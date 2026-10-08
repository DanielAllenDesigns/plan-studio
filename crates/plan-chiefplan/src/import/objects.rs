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
use super::{ImportOptions, ImportReport, SymbolQuery};
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
        "cabinet_corners",
        "symbols",
        "symbols_with_guid",
        "symbols_linked",
        "electrical_devices",
        "electrical_in_groups",
        "electrical_connections",
        "stairs",
        "stair_landings",
        "stairs_stacked",
        "roof_planes",
        "roof_edges_joined",
        "roof_gable_edges",
        "roof_overhangs",
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
    let mut corners = 0usize;
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
        let mut json = cabinet_json(&c, id);
        if let Some(at) = cabinets::corner_at(&c, &project.floors[fi].walls) {
            cabinets::make_corner(&mut json, &c, at);
            corners += 1;
        }
        project.floors[fi].cabinets.push(json);
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
    report.count("cabinet_corners", corners);
    if placed > 0 {
        let by_kind: Vec<String> = kinds.iter().map(|(k, n)| format!("{n} {k}")).collect();
        report.warnings.push(format!(
            "{placed} cabinet box(es) imported ({}) with position, size and kind; door and drawer layouts are inferred from the style names, {corners} square box(es) standing in a wall corner became corner cabinets (guessed from where they stand) and blind cabinets come in as plain boxes",
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
    let (mut placed, mut in_cabinets, mut failed, mut unresolved, mut with_guid) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
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
        if s.catalog_guid.is_some() {
            with_guid += 1;
        }
        let resolved = opts.symbol_resolver.and_then(|f| {
            f(&SymbolQuery {
                name: s.name.clone(),
                tags: s.tags.clone(),
                unique_id: s.catalog_guid.clone(),
                candidates: s.guid_candidates.clone(),
            })
        });
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
    report.count("symbols_with_guid", with_guid);
    report.count("symbols_linked", placed - unresolved);
    if placed > 0 {
        let how = if opts.symbol_resolver.is_some() {
            format!(
                "{} linked to a catalog item and {unresolved} left as \"chief-plan.<name>\" boxes",
                placed - unresolved
            )
        } else {
            "all as \"chief-plan.<name>\" boxes (no catalog resolver given)".to_string()
        };
        report.warnings.push(format!(
            "{placed} library object(s) imported by name; {with_guid} carry the library item's GUID: {how}"
        ));
    }
    if failed > 0 {
        report.warnings.push(format!(
            "{failed} library object(s) had no placement record or name and were skipped"
        ));
    }
}

/// A device placed on a floor, kept until the connections are resolved.
struct PlacedDevice {
    id: u64,
    kind: &'static str,
    pos: (f64, f64),
    json: Value,
}

/// How far from a device's point a connection arc may end: a wall switch's arc
/// leaves its symbol 10" to 18" off the point.
const CONNECTION_REACH: f64 = 20.0;

fn is_switch(kind: &str) -> bool {
    matches!(
        kind,
        "Switch" | "Switch3Way" | "Switch4Way" | "SwitchDimmer"
    )
}

fn nearest_device(devs: &[PlacedDevice], p: (f64, f64)) -> Option<usize> {
    devs.iter()
        .enumerate()
        .map(|(i, d)| (i, (d.pos.0 - p.0).hypot(d.pos.1 - p.1)))
        .filter(|(_, d)| *d <= CONNECTION_REACH)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

fn import_electrical(
    bytes: &[u8],
    tree: &ObjectTree,
    chief_floors: &[ChiefFloor],
    project: &mut Project,
    report: &mut ImportReport,
    imported: &mut [bool],
) {
    let mut per_floor: Vec<Vec<PlacedDevice>> =
        (0..project.floors.len()).map(|_| Vec::new()).collect();
    let (mut placed, mut unknown, mut undecoded, mut on_wall, mut in_groups) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    for dn in tree.of_kind(electrical::DEVICE, 0) {
        let Some(fi) = floor_of(tree, chief_floors, dn) else {
            continue;
        };
        // Loose devices, and the devices of a multi-gang box.
        let in_group = match parent_class(tree, dn) {
            Some(FLOOR_CLASS) => false,
            Some(electrical::GANG_BOX) => tree
                .node(dn)
                .parent
                .is_some_and(|g| parent_class(tree, g) == Some(FLOOR_CLASS)),
            _ => continue,
        };
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
        per_floor[fi].push(PlacedDevice {
            id,
            kind,
            pos: (d.x, d.y),
            json: json!({
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
            }),
        });
        imported[dn] = true;
        if in_group {
            if let Some(g) = tree.node(dn).parent {
                imported[g] = true;
            }
            in_groups += 1;
        }
        placed += 1;
    }

    // The dashed arcs between devices.
    let mut connections_total = 0usize;
    let mut connections: Vec<Vec<Value>> = vec![Vec::new(); project.floors.len()];
    let mut switched: Vec<Vec<(u64, u64)>> = vec![Vec::new(); project.floors.len()];
    for cn in tree.of_kind(electrical::CONNECTION, 1) {
        let Some(fi) = floor_of(tree, chief_floors, cn) else {
            continue;
        };
        if parent_class(tree, cn) != Some(FLOOR_CLASS) || per_floor[fi].is_empty() {
            continue;
        }
        let Some(c) = electrical::decode_connection(bytes, tree, cn) else {
            continue;
        };
        let devs = &per_floor[fi];
        let (Some(a), Some(b)) = (nearest_device(devs, c.start), nearest_device(devs, c.end))
        else {
            continue;
        };
        if a == b {
            continue;
        }
        // The switch end controls the other: `from` is the switch.
        let (from, to, bulge) = if !is_switch(devs[a].kind) && is_switch(devs[b].kind) {
            (b, a, -c.bulge())
        } else {
            (a, b, c.bulge())
        };
        if is_switch(devs[from].kind) && !is_switch(devs[to].kind) {
            switched[fi].push((devs[to].id, devs[from].id));
        }
        connections[fi].push(json!({
            "from": devs[from].id,
            "to": devs[to].id,
            "arc_bulge": bulge,
        }));
        imported[cn] = true;
        connections_total += 1;
    }

    for (fi, devs) in per_floor.into_iter().enumerate() {
        if devs.is_empty() {
            continue;
        }
        let list: Vec<Value> = devs
            .into_iter()
            .map(|d| {
                let mut v = d.json;
                let by: Vec<u64> = switched[fi]
                    .iter()
                    .filter(|(load, _)| *load == d.id)
                    .map(|(_, sw)| *sw)
                    .collect();
                v["switched_by"] = json!(by);
                v
            })
            .collect();
        project.floors[fi].electrical =
            Some(json!({"devices": list, "connections": connections[fi]}));
    }
    report.count("electrical_devices", placed);
    report.count("electrical_on_wall", on_wall);
    report.count("electrical_in_groups", in_groups);
    report.count("electrical_connections", connections_total);
    if placed > 0 {
        report.warnings.push(format!(
            "{placed} electrical device(s) imported with position and kind ({in_groups} from multi-gang boxes); heights come from the kind, wall devices face out of the nearest wall, {connections_total} connection arc(s) link devices (the switch end controls the other), circuits were not decoded"
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
    let mut stacked = 0usize;
    for (fi, f) in &flights {
        let id = project.alloc_id();
        let elev = project.floors[*fi].elevation;
        if f.base_above(elev).is_some_and(|b| b > 0.0) {
            stacked += 1;
        }
        project.floors[*fi].stairs.push(flight_json(f, id, elev));
        imported[f.node] = true;
    }
    let mut landing_exact = 0usize;
    for (fi, l) in &landings {
        let id = project.alloc_id();
        let elev = project.floors[*fi].elevation;
        // The landing is where a flight starts or ends: its surface is that
        // flight's bottom, or the top of the flight that arrives plus a riser.
        let from_flights = flights
            .iter()
            .filter(|(f, _)| f == fi)
            .find_map(|(_, f)| {
                let start = (f.x, f.y);
                let h = if near_outline(&l.outline, start, 8.0) {
                    f.bottom
                } else if near_outline(&l.outline, f.end_point(), 8.0) {
                    f.top.map(|t| t + f.riser)
                } else {
                    None
                };
                h.map(|h| h - stairs::FLOOR_FINISH - elev)
            })
            .filter(|h| *h > 1.0);
        let rise = match from_flights {
            Some(h) => {
                landing_exact += 1;
                h
            }
            None => {
                // Not stored for this landing: half the rise of the floor's flights.
                let rise = flights
                    .iter()
                    .filter(|(f, _)| f == fi)
                    .map(|(_, f)| f.rise)
                    .fold(0.0f64, f64::max);
                if rise > 0.0 {
                    rise / 2.0
                } else {
                    project.floors[*fi].ceiling_height / 2.0
                }
            }
        };
        project.floors[*fi]
            .stairs
            .push(landing_json(l, id, elev, rise));
        imported[l.node] = true;
    }
    report.count("stairs", flights.len());
    report.count("stair_landings", landings.len());
    report.count("stairs_stacked", stacked);
    if !flights.is_empty() || !landings.is_empty() {
        report.warnings.push(format!(
            "{} stair flight(s) and {} landing(s) imported as separate straight stairs and landings; each flight gets its own riser count from its run, {stacked} stand on a landing (height from the file) and {landing_exact} of the landings have the height of the flight that leaves them (the others are half the rise); rail sides were not decoded",
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

/// Whether `p` is inside `outline` or within `tol` of one of its edges.
fn near_outline(outline: &[(f64, f64)], p: (f64, f64), tol: f64) -> bool {
    let n = outline.len();
    let mut inside = false;
    for k in 0..n {
        let (a, b) = (outline[k], outline[(k + 1) % n]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
            inside = !inside;
        }
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l2 = dx * dx + dy * dy;
        let t = if l2 < 1e-12 {
            0.0
        } else {
            (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0)
        };
        if (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy) <= tol {
            return true;
        }
    }
    inside
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
    let (mut joined, mut gables, mut overhangs) = (0usize, 0usize, 0usize);
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
        joined += p.edges.iter().filter(|e| e.link.is_some()).count();
        gables += p
            .edges
            .iter()
            .filter(|e| e.role == roofs::EdgeRole::Rake)
            .count();
        overhangs += usize::from(p.overhang > 0.0);
        seen[fi].push(p);
        placed += 1;
    }
    report.count("roof_planes", placed);
    report.count("roof_duplicates", duplicates);
    report.count("roof_edges_joined", joined);
    report.count("roof_gable_edges", gables);
    report.count("roof_overhangs", overhangs);
    if placed > 0 {
        report.warnings.push(format!(
            "{placed} roof plane(s) imported with outline (eave edge first), pitch and baseline height; {joined} edge(s) are joined to a neighbouring plane (ridge, hip, valley), {gables} are gable ends, {overhangs} plane(s) carry an eave overhang measured from the baseline; fascia sizes, holes and materials were not decoded"
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
