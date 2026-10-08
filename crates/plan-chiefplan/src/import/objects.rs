//! The object stage of the import: cabinets, symbols, electrical devices,
//! stairs, roof planes and room labels.
//!
//! Each decoder lives in its own module ([`cabinets`], [`symbols`],
//! [`electrical`], [`stairs`], [`roofs`], [`labels`]); this module walks the
//! object tree, places what they decode on the right floor of the project
//! and counts it in the report. Cabinets, stairs, electrical data and roofs
//! are stored in the floor's opaque slots as the JSON their owning crates
//! serialize (`plan_cabinets::Cabinet`, `plan_stairs::Stair`,
//! `plan_electrical::ElectricalLayer`, the roof record of
//! `plan-app/src/editor/roof_view.rs`): this crate cannot depend on them, and
//! the shapes are pinned by the tests of each decoder.

use super::cabinets::{self, cabinet_json, countertop_json, BoxKind};
use super::electrical::{self, default_height, kind_is_wall_mounted, kind_of};
use super::floors::{floor_of, ChiefFloor};
use super::labels::{self, is_area_note};
use super::roofs::{self, plane_json, same_outline};
use super::stairs::{self, flight_json, landing_json};
use super::symbols::{self, fallback_catalog_id};
use super::tree::ObjectTree;
use super::{ImportOptions, ImportReport};
use plan_core::geometry::{dist_to_segment, Point};
use plan_core::model::{Project, RoomName, Wall};
use plan_core::PlacedSymbol;
use serde_json::{json, Value};

/// Floor class id (the parent of everything placed).
const FLOOR_CLASS: u8 = 30;

/// A room decoded in the room stage, for matching labels.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RoomBox {
    pub center: (f64, f64),
    pub size: (f64, f64),
    /// The room already shows a name.
    pub named: bool,
}

fn parent_class(tree: &ObjectTree, i: usize) -> Option<u8> {
    tree.node(i).parent.map(|p| tree.node(p).class)
}

/// The walls' bounding box of a floor, `(min x, min y, max x, max y)`.
fn wall_bounds(project: &Project, fi: usize) -> Option<(f64, f64, f64, f64)> {
    let mut it = project.floors[fi]
        .walls
        .iter()
        .flat_map(|w| [w.start, w.end]);
    let first = it.next()?;
    let mut b = (first.x, first.y, first.x, first.y);
    for p in it {
        b = (b.0.min(p.x), b.1.min(p.y), b.2.max(p.x), b.3.max(p.y));
    }
    Some(b)
}

/// The wall a wall-mounted device sits on: the nearest centreline within half
/// the wall's thickness plus 4", and the angle that faces out of the wall on
/// the device's side.
fn host_of(walls: &[Wall], p: Point) -> Option<(u64, f64)> {
    let mut best: Option<(f64, &Wall)> = None;
    for w in walls {
        if w.start.dist(w.end) < 1.0 {
            continue;
        }
        let d = dist_to_segment(p, w.start, w.end);
        if d <= w.thickness / 2.0 + 4.0 && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, w));
        }
    }
    let (_, w) = best?;
    let dir = w.end.sub(w.start);
    let side = dir.x * (p.y - w.start.y) - dir.y * (p.x - w.start.x);
    let base = dir.y.atan2(dir.x);
    let angle = if side >= 0.0 {
        base + std::f64::consts::FRAC_PI_2
    } else {
        base - std::f64::consts::FRAC_PI_2
    };
    Some((w.id, angle))
}

fn contains_point(b: &RoomBox, p: (f64, f64)) -> bool {
    (p.0 - b.center.0).abs() <= b.size.0 / 2.0 && (p.1 - b.center.1).abs() <= b.size.1 / 2.0
}

/// Runs the stage. Returns, per tree node, whether an imported object covers it
/// (the object itself or one of its ancestors), for the skipped-class listing.
#[allow(clippy::too_many_arguments)]
pub(crate) fn import_objects(
    bytes: &[u8],
    tree: &ObjectTree,
    chief_floors: &[ChiefFloor],
    project: &mut Project,
    opts: &ImportOptions,
    report: &mut ImportReport,
    rooms: &[Vec<RoomBox>],
) -> Vec<bool> {
    let mut imported = vec![false; tree.len()];
    for k in [
        "cabinets",
        "cabinet_soffits",
        "countertops",
        "symbols",
        "electrical_devices",
        "stairs",
        "stair_landings",
        "roof_planes",
        "room_labels",
        "room_labels_applied",
    ] {
        report.counts.entry(k.to_string()).or_insert(0);
    }

    if opts.cabinets {
        import_cabinets(bytes, tree, chief_floors, project, report, &mut imported);
    }
    if opts.symbols {
        import_symbols(
            bytes,
            tree,
            chief_floors,
            project,
            opts,
            report,
            &mut imported,
        );
    }
    if opts.electrical {
        import_electrical(bytes, tree, chief_floors, project, report, &mut imported);
    }
    if opts.stairs {
        import_stairs(bytes, tree, chief_floors, project, report, &mut imported);
    }
    if opts.roofs {
        import_roofs(bytes, tree, chief_floors, project, report, &mut imported);
    }
    if opts.room_labels {
        import_labels(
            bytes,
            tree,
            chief_floors,
            project,
            rooms,
            report,
            &mut imported,
        );
    }

    // Coverage: an imported node covers its descendants (nodes come in file
    // order, so a parent precedes its children).
    let mut covered = imported;
    for i in 0..covered.len() {
        if !covered[i] {
            if let Some(p) = tree.node(i).parent {
                covered[i] = covered[p];
            }
        }
    }
    covered
}

fn import_cabinets(
    bytes: &[u8],
    tree: &ObjectTree,
    chief_floors: &[ChiefFloor],
    project: &mut Project,
    report: &mut ImportReport,
    imported: &mut [bool],
) {
    let (mut placed, mut soffits, mut tops, mut failed) = (0usize, 0usize, 0usize, 0usize);
    let mut kinds = std::collections::BTreeMap::<&'static str, usize>::new();
    for cn in tree.of_kind(cabinets::CABINET, 0) {
        let Some(fi) = floor_of(tree, chief_floors, cn) else {
            continue;
        };
        if !matches!(
            parent_class(tree, cn),
            Some(FLOOR_CLASS) | Some(cabinets::GROUP)
        ) {
            continue;
        }
        let Some(c) = cabinets::decode_cabinet(bytes, tree, cn) else {
            failed += 1;
            continue;
        };
        let id = project.alloc_id();
        project.floors[fi].cabinets.push(cabinet_json(&c, id));
        imported[cn] = true;
        if let Some(p) = tree.node(cn).parent {
            if tree.node(p).class == cabinets::GROUP {
                imported[p] = true;
            }
        }
        placed += 1;
        if c.kind == BoxKind::Soffit {
            soffits += 1;
        }
        *kinds.entry(c.kind.label()).or_insert(0) += 1;
    }
    for tn in tree.of_kind(cabinets::COUNTERTOP, 0) {
        let Some(fi) = floor_of(tree, chief_floors, tn) else {
            continue;
        };
        if parent_class(tree, tn) != Some(FLOOR_CLASS) {
            continue;
        }
        let Some(t) = cabinets::decode_countertop(bytes, tree, tn) else {
            continue;
        };
        let id = project.alloc_id();
        project.floors[fi].cabinets.push(countertop_json(&t, id));
        imported[tn] = true;
        tops += 1;
    }
    report.count("cabinets", placed);
    report.count("cabinet_soffits", soffits);
    report.count("countertops", tops);
    if placed > 0 {
        let by_kind: Vec<String> = kinds.iter().map(|(k, n)| format!("{n} {k}")).collect();
        report.warnings.push(format!(
            "{placed} cabinet box(es) imported ({}) with position, size and kind; door and drawer layouts are inferred from the style names, and corner and blind cabinets come in as plain boxes",
            by_kind.join(", ")
        ));
    }
    if failed > 0 {
        report.warnings.push(format!(
            "{failed} cabinet object(s) had no placement record and were skipped"
        ));
    }
}

fn import_symbols(
    bytes: &[u8],
    tree: &ObjectTree,
    chief_floors: &[ChiefFloor],
    project: &mut Project,
    opts: &ImportOptions,
    report: &mut ImportReport,
    imported: &mut [bool],
) {
    let (mut placed, mut in_cabinets, mut failed, mut unresolved) =
        (0usize, 0usize, 0usize, 0usize);
    for sn in tree.of_kind(symbols::SYMBOL, 0) {
        let Some(fi) = floor_of(tree, chief_floors, sn) else {
            continue;
        };
        match parent_class(tree, sn) {
            Some(FLOOR_CLASS) | Some(cabinets::GROUP) => {}
            Some(_) => {
                in_cabinets += 1;
                continue;
            }
            None => continue,
        }
        let Some(s) = symbols::decode_symbol(bytes, tree, sn) else {
            failed += 1;
            continue;
        };
        let resolved = opts.symbol_resolver.and_then(|f| f(&s.name, &s.tags));
        if resolved.is_none() {
            unresolved += 1;
        }
        let catalog = resolved.unwrap_or_else(|| fallback_catalog_id(&s.name));
        let b = &s.block;
        let mut sym = PlacedSymbol::new(catalog, Point::new(b.x, b.y), b.width, b.depth, b.height);
        sym.angle = b.angle().to_degrees();
        sym.elevation = b.elevation();
        sym.label = s.name.clone();
        project.add_symbol(fi, sym);
        imported[sn] = true;
        if let Some(p) = tree.node(sn).parent {
            if tree.node(p).class == cabinets::GROUP {
                imported[p] = true;
            }
        }
        placed += 1;
    }
    report.count("symbols", placed);
    report.count("symbols_in_cabinets", in_cabinets);
    if placed > 0 {
        report.warnings.push(format!(
            "{placed} library object(s) imported by name: the plan holds its own copy of each and no catalog link, so {unresolved} import as \"chief-plan.<name>\" boxes until a catalog match is supplied"
        ));
    }
    if failed > 0 {
        report.warnings.push(format!(
            "{failed} library object(s) had no placement record or name and were skipped"
        ));
    }
}

fn import_electrical(
    bytes: &[u8],
    tree: &ObjectTree,
    chief_floors: &[ChiefFloor],
    project: &mut Project,
    report: &mut ImportReport,
    imported: &mut [bool],
) {
    let mut per_floor: Vec<Vec<Value>> = vec![Vec::new(); project.floors.len()];
    let (mut placed, mut unknown, mut undecoded, mut on_wall) = (0usize, 0usize, 0usize, 0usize);
    for dn in tree.of_kind(electrical::DEVICE, 0) {
        let Some(fi) = floor_of(tree, chief_floors, dn) else {
            continue;
        };
        if parent_class(tree, dn) != Some(FLOOR_CLASS) {
            continue;
        }
        let bounds = wall_bounds(project, fi).unwrap_or((-1.0e5, -1.0e5, 1.0e5, 1.0e5));
        let Some(d) = electrical::decode_device(bytes, tree, dn, bounds) else {
            undecoded += 1;
            continue;
        };
        let Some(kind) = kind_of(&d.name, &d.tags, d.mount) else {
            unknown += 1;
            continue;
        };
        let wall_mounted = kind_is_wall_mounted(kind);
        let host = if wall_mounted {
            host_of(&project.floors[fi].walls, Point::new(d.x, d.y))
        } else {
            None
        };
        if host.is_some() {
            on_wall += 1;
        }
        let id = project.alloc_id();
        let ceiling = project.floors[fi].ceiling_height;
        per_floor[fi].push(json!({
            "id": id,
            "kind": kind,
            "position": {"x": d.x, "y": d.y},
            "angle": host.map_or(0.0, |h| h.1),
            "height": default_height(kind, ceiling),
            "wall_id": host.map(|h| h.0),
            "circuit": null,
            "label": d.name,
            "switched_by": [],
            "finish": "",
            "hide_label": true,
        }));
        imported[dn] = true;
        placed += 1;
    }
    for (fi, devices) in per_floor.into_iter().enumerate() {
        if !devices.is_empty() {
            project.floors[fi].electrical = Some(json!({"devices": devices, "connections": []}));
        }
    }
    report.count("electrical_devices", placed);
    report.count("electrical_on_wall", on_wall);
    if placed > 0 {
        report.warnings.push(format!(
            "{placed} electrical device(s) imported with position and kind; heights come from the kind, wall devices face out of the nearest wall, and circuits and switch connections were not decoded"
        ));
    }
    if unknown > 0 {
        report.warnings.push(format!(
            "{unknown} electrical device(s) of a kind Plan Studio has no equivalent for were skipped"
        ));
    }
    if undecoded > 0 {
        report.warnings.push(format!(
            "{undecoded} electrical object(s) had no position inside the walls' box and were skipped"
        ));
    }
}

fn import_stairs(
    bytes: &[u8],
    tree: &ObjectTree,
    chief_floors: &[ChiefFloor],
    project: &mut Project,
    report: &mut ImportReport,
    imported: &mut [bool],
) {
    let mut flights: Vec<(usize, stairs::ChiefFlight)> = Vec::new();
    let mut landings: Vec<(usize, stairs::ChiefLanding)> = Vec::new();
    let mut failed = 0usize;
    for fnode in tree.of_kind(stairs::FLIGHT, 0) {
        let Some(fi) = floor_of(tree, chief_floors, fnode) else {
            continue;
        };
        if parent_class(tree, fnode) != Some(FLOOR_CLASS) {
            continue;
        }
        match stairs::decode_flight(bytes, tree, fnode) {
            Some(f) => flights.push((fi, f)),
            None => failed += 1,
        }
    }
    for lnode in tree.of_kind(stairs::LANDING, 0) {
        let Some(fi) = floor_of(tree, chief_floors, lnode) else {
            continue;
        };
        if parent_class(tree, lnode) != Some(FLOOR_CLASS) {
            continue;
        }
        match stairs::decode_landing(bytes, tree, lnode) {
            Some(l) => landings.push((fi, l)),
            None => failed += 1,
        }
    }
    for (fi, f) in &flights {
        let id = project.alloc_id();
        let elev = project.floors[*fi].elevation;
        project.floors[*fi].stairs.push(flight_json(f, id, elev));
        imported[f.node] = true;
    }
    for (fi, l) in &landings {
        let id = project.alloc_id();
        let elev = project.floors[*fi].elevation;
        // The landing's height is not stored: half the rise of the floor's flights.
        let rise = flights
            .iter()
            .filter(|(f, _)| f == fi)
            .map(|(_, f)| f.rise)
            .fold(0.0f64, f64::max);
        let rise = if rise > 0.0 {
            rise / 2.0
        } else {
            project.floors[*fi].ceiling_height / 2.0
        };
        project.floors[*fi]
            .stairs
            .push(landing_json(l, id, elev, rise));
        imported[l.node] = true;
    }
    report.count("stairs", flights.len());
    report.count("stair_landings", landings.len());
    if !flights.is_empty() || !landings.is_empty() {
        report.warnings.push(format!(
            "{} stair flight(s) and {} landing(s) imported as separate straight stairs and landings; each flight gets its own riser count from its run, and landing heights are half the rise (approximate)",
            flights.len(),
            landings.len()
        ));
    }
    if failed > 0 {
        report.warnings.push(format!(
            "{failed} stair or landing object(s) had no readable line or spec and were skipped"
        ));
    }
}

fn import_roofs(
    bytes: &[u8],
    tree: &ObjectTree,
    chief_floors: &[ChiefFloor],
    project: &mut Project,
    report: &mut ImportReport,
    imported: &mut [bool],
) {
    let mut seen: Vec<Vec<roofs::ChiefRoofPlane>> = vec![Vec::new(); project.floors.len()];
    let (mut placed, mut duplicates, mut failed) = (0usize, 0usize, 0usize);
    for pn in tree.of_kind(roofs::ROOF_PLANE, 0) {
        let Some(fi) = floor_of(tree, chief_floors, pn) else {
            continue;
        };
        if parent_class(tree, pn) != Some(FLOOR_CLASS) {
            continue;
        }
        let Some(p) = roofs::decode_plane(bytes, tree, pn) else {
            failed += 1;
            continue;
        };
        imported[pn] = true;
        if seen[fi].iter().any(|q| same_outline(q, &p)) {
            duplicates += 1;
            continue;
        }
        let id = project.alloc_id();
        project.floors[fi]
            .roofs
            .push(plane_json(&p, id, "Roof Planes"));
        seen[fi].push(p);
        placed += 1;
    }
    report.count("roof_planes", placed);
    report.count("roof_duplicates", duplicates);
    if placed > 0 {
        report.warnings.push(format!(
            "{placed} roof plane(s) imported with outline, pitch and baseline height; per-edge hips and gables, overhangs and materials were not decoded"
        ));
    }
    if duplicates > 0 {
        report.warnings.push(format!(
            "{duplicates} roof plane(s) with an outline identical to another plane of the floor were dropped"
        ));
    }
    if failed > 0 {
        report.warnings.push(format!(
            "{failed} roof object(s) had no pitch and baseline record (probably ceiling or deck surfaces) and were skipped"
        ));
    }
}

fn import_labels(
    bytes: &[u8],
    tree: &ObjectTree,
    chief_floors: &[ChiefFloor],
    project: &mut Project,
    rooms: &[Vec<RoomBox>],
    report: &mut ImportReport,
    imported: &mut [bool],
) {
    let mut taken: Vec<Vec<bool>> = rooms
        .iter()
        .map(|r| r.iter().map(|b| b.named).collect())
        .collect();
    let (mut found, mut applied, mut free) = (0usize, 0usize, 0usize);
    for ln in tree.of_kind(labels::LABEL, 0) {
        let Some(fi) = floor_of(tree, chief_floors, ln) else {
            continue;
        };
        if parent_class(tree, ln) != Some(FLOOR_CLASS) {
            continue;
        }
        let Some(l) = labels::decode_label(bytes, tree, ln) else {
            continue;
        };
        if is_area_note(&l.text) {
            continue;
        }
        found += 1;
        imported[ln] = true;
        let boxes = rooms.get(fi).map(Vec::as_slice).unwrap_or(&[]);
        // The smallest room box that contains the label's centre.
        let host = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| contains_point(b, l.center))
            .min_by(|a, b| {
                (a.1.size.0 * a.1.size.1)
                    .partial_cmp(&(b.1.size.0 * b.1.size.1))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(k, _)| k);
        match host {
            Some(k) if taken[fi][k] => continue,
            Some(k) => taken[fi][k] = true,
            None => free += 1,
        }
        project.floors[fi].room_names.push(RoomName::new(
            Point::new(l.center.0, l.center.1),
            l.text.clone(),
            String::new(),
        ));
        applied += 1;
    }
    report.count("room_labels", found);
    report.count("room_labels_applied", applied);
    if applied > 0 {
        report.warnings.push(format!(
            "{applied} room name(s) came from text labels placed at the centre of the label's frame ({free} outside every decoded room)"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::model::WallKind;

    #[test]
    fn devices_find_their_wall_and_face_the_room() {
        let mut w = Wall::new(
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        w.id = 7;
        // A device above the wall (left of +x) faces +y; below faces -y.
        let up = host_of(&[w.clone()], Point::new(50.0, 3.0)).unwrap();
        assert_eq!(up.0, 7);
        assert!((up.1 - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        let down = host_of(&[w.clone()], Point::new(50.0, -3.0)).unwrap();
        assert!((down.1 + std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!(host_of(&[w], Point::new(50.0, 30.0)).is_none());
    }

    #[test]
    fn rooms_contain_label_centres() {
        let b = RoomBox {
            center: (100.0, 100.0),
            size: (40.0, 20.0),
            named: false,
        };
        assert!(contains_point(&b, (119.0, 109.0)));
        assert!(!contains_point(&b, (121.0, 100.0)));
    }
}
