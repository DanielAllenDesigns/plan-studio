//! Import of a Chief Architect project `.plan` into a Plan Studio
//! [`Project`].
//!
//! ```no_run
//! use plan_chiefplan::import::{import_plan, ImportOptions};
//! let r = import_plan("House.plan", &ImportOptions::default()).unwrap();
//! println!("{} walls", r.report.counts["walls"]);
//! let project = r.project; // hand to the editor's open_project
//! ```
//!
//! The format notes (which class is which, with offsets and confidence) are
//! in `docs/chief-plan-format.md`. In short a plan file is a stream of
//! `[01] CD AB <class> <version> <u32 size> <payload>` objects that nest
//! (see [`tree`]); the importer reads
//!
//! | Plan Studio | Chief class | Confidence |
//! |---|---|---|
//! | floors | 30 | High (count, order, elevation, ceiling) / Low (names) |
//! | walls | 6 (+ child 31/40 line, 215 type) | High (line, type), Medium (height, side, arcs) |
//! | doors, windows | 9, 10 (children of a wall) | High (centre, width, height), Low (style) |
//! | room names | 23 inside a floor | Medium |
//! | dimensions | 24 | Medium (points), Low (line offset) |
//! | text notes | 25 | Medium (position, string), Low (size) |
//!
//! Everything else is counted in [`ImportReport::skipped_classes`].

pub mod dims;
pub mod floors;
pub mod openings;
pub mod rooms;
pub mod texts;
pub mod tree;
pub mod walls;

use crate::bridge::{apply_seed, collapse, seed_defaults, wall_type_def};
use crate::error::{Error, Result};
use crate::scan::TemplateKind;
use floors::{decode_floors, floor_of, Generation};
use plan_core::dimension::{Dimension, DimensionKind};
use plan_core::floors::FloorKind;
use plan_core::geometry::Point;
use plan_core::model::{Floor, Id, Opening, Project, RoomName};
use plan_core::PlanDefaults;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use tree::ObjectTree;
use walls::{decode_wall, plan_wall, WALL};

/// Ceiling value used when a floor's header triple is not found.
pub const DEFAULT_CEILING: f64 = plan_core::model::DEFAULT_CEILING_HEIGHT;

/// Import settings.
#[derive(Debug, Clone)]
pub struct ImportOptions {
    /// Project name; default is the file stem.
    pub project_name: Option<String>,
    /// Seed layers, layer sets, text styles and wall types from the file
    /// itself (one extra pass over the file's strings).
    pub seed_from_file: bool,
    /// Import dimension strings (their line offset is assumed).
    pub dimensions: bool,
    /// Import text notes as CAD text (their size is assumed).
    pub text: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        ImportOptions {
            project_name: None,
            seed_from_file: true,
            dimensions: true,
            text: true,
        }
    }
}

/// Per-floor counts.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FloorReport {
    pub name: String,
    pub elevation: f64,
    pub ceiling_height: f64,
    pub walls: usize,
    pub openings: usize,
    pub rooms: usize,
    pub dimensions: usize,
}

/// A class found on a floor that the importer does not turn into objects.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SkippedClass {
    pub class: u8,
    pub version: u8,
    pub label: String,
    /// Confidence of `label` ("High", "Medium", "Low", "None").
    pub confidence: String,
    pub count: usize,
}

/// What the import found and what it left out.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ImportReport {
    pub file_name: String,
    pub generation: String,
    /// Objects accepted into the tree / rejected as chance matches.
    pub objects: usize,
    pub rejected_objects: usize,
    /// `floors`, `walls`, `doors`, `windows`, `rooms`, `rooms_named`,
    /// `dimensions`, `wall_types`.
    pub counts: BTreeMap<String, usize>,
    pub floors: Vec<FloorReport>,
    pub skipped_classes: Vec<SkippedClass>,
    pub warnings: Vec<String>,
}

impl ImportReport {
    fn count(&mut self, key: &str, n: usize) {
        *self.counts.entry(key.to_string()).or_insert(0) += n;
    }

    /// A short multi-line summary for a dialog or a log.
    pub fn summary(&self) -> String {
        let c = |k: &str| self.counts.get(k).copied().unwrap_or(0);
        let mut s = format!(
            "{}: {} floors, {} walls, {} doors, {} windows, {} named rooms, {} dimensions, {} texts",
            self.file_name,
            c("floors"),
            c("walls"),
            c("doors"),
            c("windows"),
            c("rooms_named"),
            c("dimensions"),
            c("texts")
        );
        for w in &self.warnings {
            s.push_str("\n  - ");
            s.push_str(w);
        }
        s
    }
}

/// The imported project and the report.
#[derive(Debug, Clone)]
pub struct ImportResult {
    pub project: Project,
    pub report: ImportReport,
}

/// Reads the plan at `path` into a project.
pub fn import_plan(path: impl AsRef<Path>, opts: &ImportOptions) -> Result<ImportResult> {
    let path = path.as_ref();
    if TemplateKind::from_extension(path) == Some(TemplateKind::Layout) {
        return Err(Error::Format(
            "a .layout file holds sheets, not plan geometry".into(),
        ));
    }
    let bytes = std::fs::read(path)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    import_bytes(&bytes, &name, opts)
}

fn ordinal(n: usize) -> String {
    let suffix = match (n % 100, n % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    format!("{n}{suffix} Floor")
}

/// The exact name of a layer already in the project that equals `wanted`
/// after whitespace collapsing, else `wanted`.
fn canonical_layer(project: &Project, wanted: &str) -> String {
    let key = collapse(wanted);
    project
        .layers
        .layers
        .iter()
        .find(|l| collapse(&l.name) == key)
        .map_or_else(|| wanted.to_string(), |l| l.name.clone())
}

/// Internal parts of walls, rooms and dimensions that are never reported as
/// skipped.
const PART_CLASSES: [u8; 12] = [4, 31, 40, 81, 120, 93, 122, 54, 68, 114, 55, 215];

/// Reads the plan held in `bytes` (the whole file) into a project.
pub fn import_bytes(bytes: &[u8], file_name: &str, opts: &ImportOptions) -> Result<ImportResult> {
    let mut report = ImportReport {
        file_name: file_name.to_string(),
        ..ImportReport::default()
    };
    for k in [
        "floors",
        "walls",
        "doors",
        "windows",
        "rooms",
        "rooms_named",
        "dimensions",
        "wall_types",
        "curved_walls",
        "wall_ends_joined",
        "texts",
    ] {
        report.counts.insert(k.to_string(), 0);
    }
    // The string-level inventory seeds layers and layer sets; it needs the X18
    // header, so an X17 file imports without that seed.
    let inventory = crate::build_inventory_from_bytes(bytes, file_name, Some(TemplateKind::Plan));
    let tree = ObjectTree::build(bytes);
    let generation = Generation::of(bytes);
    report.generation = generation.label().to_string();
    report.objects = tree.len();
    report.rejected_objects = tree.rejected;

    // Project shell: Daniel's defaults, seeded with the file's own layers,
    // layer sets and wall types.
    let mut defaults = PlanDefaults::default();
    if opts.seed_from_file {
        match &inventory {
            Ok((inv, _)) => {
                let seed = seed_defaults(inv, defaults.clone());
                apply_seed(&mut defaults, &seed);
            }
            Err(e) => report.warnings.push(format!(
                "layers and layer sets were not read from the file ({e}); Daniel's defaults are used"
            )),
        }
    }
    let name = opts
        .project_name
        .clone()
        .unwrap_or_else(|| file_name.trim_end_matches(".plan").to_string());
    let mut project = Project::from_defaults(name, &defaults);

    let wall_types = walls::collect_wall_types(bytes, &tree);
    report.count("wall_types", wall_types.types.len());

    let chief_floors = decode_floors(bytes, &tree, generation);
    if chief_floors.is_empty() {
        report
            .warnings
            .push("no floor objects (class 30) found; nothing to import".into());
        report.count("floors", project.floors.len());
        report.count("wall_types", 0);
        return Ok(ImportResult { project, report });
    }
    if chief_floors.iter().any(|f| !f.found) {
        report.warnings.push(
            "floor elevation/ceiling header not found on some floors; default ceiling used".into(),
        );
    }

    // Floors.
    let mut floors: Vec<Floor> = Vec::new();
    let roof_floor: Vec<bool> = chief_floors
        .iter()
        .map(|f| tree.of_kind(18, 1).any(|i| tree.contains(f.node, i)))
        .collect();
    let mut normal_count = 0;
    for (i, f) in chief_floors.iter().enumerate() {
        let last = i + 1 == chief_floors.len();
        let (fname, kind) = if f.elevation < -1.0 {
            ("Foundation".to_string(), FloorKind::Foundation)
        } else if last && roof_floor[i] && i > 0 {
            ("Attic".to_string(), FloorKind::Attic)
        } else {
            normal_count += 1;
            (ordinal(normal_count), FloorKind::Normal)
        };
        let mut floor = Floor::new(fname, f.elevation);
        floor.ceiling_height = f.ceiling;
        floor.kind = kind;
        floors.push(floor);
    }
    for (i, f) in floors.iter().enumerate() {
        if let Some(prev) = floors[..i].iter().find(|g| g.elevation == f.elevation) {
            report.warnings.push(format!(
                "floors \"{}\" and \"{}\" have the same elevation ({} in): the file's floor header differs from the usual layout",
                prev.name, f.name, f.elevation
            ));
        }
    }
    project.floors = floors;
    // Floors are only named by position: say so.
    report.warnings.push(
        "floor names are positional (Foundation, 1st, 2nd, ...): the file stores none".into(),
    );

    // Walls and openings.
    let mut wall_ids: HashMap<usize, (usize, Id)> = HashMap::new();
    let mut used_types: Vec<String> = Vec::new();
    let mut approximated = 0;
    let mut curved = 0;
    let mut untyped = 0;
    let mut undecoded_openings = 0;
    let mut bad_openings = 0;
    let (mut doors, mut windows) = (0usize, 0usize);
    let wall_nodes: Vec<usize> = tree
        .of_kind(WALL, 0)
        .filter(|&i| tree.ancestor_of_class(i, WALL).is_none())
        .collect();
    for &wn in &wall_nodes {
        let Some(fi) = floor_of(&tree, &chief_floors, wn) else {
            continue; // the default wall specs of the template sit outside floors
        };
        let Some(cw) = decode_wall(bytes, &tree, wn, &wall_types) else {
            continue;
        };
        if cw.approximated {
            approximated += 1;
        }
        if cw.type_id.is_none() || cw.type_id.and_then(|id| wall_types.get(id)).is_none() {
            untyped += 1;
        }
        let (mut wall, ty) = plan_wall(&cw, &chief_floors[fi], fi, &wall_types);
        if wall.curve.is_some() {
            curved += 1;
        }
        wall.layer = canonical_layer(&project, &wall.layer);
        if let Some(t) = ty {
            if !used_types.contains(&t) {
                used_types.push(t);
            }
        }
        let id = project.alloc_id();
        wall.id = id;
        let wall_len = wall.length();
        project.floors[fi].walls.push(wall);
        wall_ids.insert(wn, (fi, id));

        undecoded_openings += openings::undecoded(bytes, &tree, wn);
        for o in openings::decode_openings(bytes, &tree, wn) {
            if !(0.0..=wall_len + 0.5).contains(&o.center) {
                bad_openings += 1;
                continue;
            }
            let oid = project.alloc_id();
            let mut op = Opening::new(
                id,
                o.center,
                o.kind,
                o.width,
                o.height,
                openings::sill_of(&o),
            );
            op.id = oid;
            op.style = openings::style_of(&o);
            if let Some(h) = o.hinge_at_end {
                op.hinge_at_end = h;
            }
            if let Some(r) = o.swing_right {
                op.swing_flipped = r;
            }
            match o.kind {
                plan_core::model::OpeningKind::Door => doors += 1,
                plan_core::model::OpeningKind::Window => windows += 1,
            }
            project.floors[fi].openings.push(op);
        }
    }
    // Chief joins walls at their reference lines; Plan Studio at centrelines.
    let mut healed = 0;
    for f in &mut project.floors {
        healed += walls::heal_joins(&mut f.walls, &mut f.openings);
    }
    report.count("wall_ends_joined", healed);
    // The file's own wall type definitions replace same-named defaults.
    for name in &used_types {
        if let Some(t) = wall_types.by_name(name) {
            let def = wall_type_def(t);
            match project.wall_types.iter_mut().find(|d| d.name == def.name) {
                Some(slot) => *slot = def,
                None => project.wall_types.push(def),
            }
        }
    }
    report.count("walls", wall_ids.len());
    report.count("curved_walls", curved);
    report.count("doors", doors);
    report.count("windows", windows);
    if approximated > 0 {
        report.warnings.push(format!(
            "{approximated} wall(s) without a line object were reduced to their first and last point (curved walls are not decoded)"
        ));
    }
    if untyped > 0 {
        report.warnings.push(format!(
            "{untyped} wall(s) without a resolvable wall type got a 5.5\" thickness"
        ));
    }
    if undecoded_openings > 0 {
        report.warnings.push(format!(
            "{undecoded_openings} door/window object(s) use a layout variant that is not decoded and were skipped"
        ));
    }
    if bad_openings > 0 {
        report.warnings.push(format!(
            "{bad_openings} opening(s) lie outside their wall's length and were skipped"
        ));
    }
    if doors > 0 {
        report.warnings.push(
            "door style (hinged, doorway, double, slider) is guessed from the width; hinge side and swing come from two flag bytes verified on one project"
                .into(),
        );
    }

    // Rooms.
    let (mut rooms_total, mut rooms_named, mut rooms_failed) = (0usize, 0usize, 0usize);
    for rn in tree.of_kind(rooms::ROOM, 0) {
        let Some(fi) = floor_of(&tree, &chief_floors, rn) else {
            continue;
        };
        rooms_total += 1;
        match rooms::decode_room(bytes, &tree, rn) {
            Some(r) => {
                if let Some(label) = r.display_name() {
                    rooms_named += 1;
                    let ty = if r.room_type == "Default" {
                        String::new()
                    } else {
                        r.room_type.clone()
                    };
                    project.floors[fi].room_names.push(RoomName::new(
                        Point::new(r.center.0, r.center.1),
                        label,
                        ty,
                    ));
                }
            }
            None => rooms_failed += 1,
        }
    }
    report.count("rooms", rooms_total);
    report.count("rooms_named", rooms_named);
    if rooms_failed > 0 {
        report.warnings.push(format!(
            "{rooms_failed} room object(s) could not be decoded"
        ));
    }
    if rooms_total > rooms_named {
        report.warnings.push(format!(
            "{} room(s) have no name in the room object (their label is a separate text object, not decoded)",
            rooms_total - rooms_named
        ));
    }

    // Dimensions.
    let mut dim_count = 0usize;
    let mut dims_skipped = 0usize;
    if opts.dimensions {
        let centres: Vec<Option<(f64, f64)>> = project
            .floors
            .iter()
            .map(|f| {
                let mut it = f.walls.iter().flat_map(|w| [w.start, w.end]);
                let first = it.next()?;
                let (mut lo, mut hi) = (first, first);
                for p in it {
                    lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
                    hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
                }
                Some(((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0))
            })
            .collect();
        for dn in tree.of_kind(dims::DIMENSION, 0) {
            let Some(fi) = floor_of(&tree, &chief_floors, dn) else {
                continue;
            };
            let Some(d) = dims::decode_dimension(bytes, &tree, dn) else {
                dims_skipped += 1;
                continue;
            };
            let centre = centres[fi].unwrap_or((0.0, 0.0));
            for pair in d.points.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                if (a.0 - b.0).hypot(a.1 - b.1) < 1.0 {
                    continue;
                }
                let off = dims::assumed_offset(a, b, centre);
                let dim = Dimension::new(
                    0,
                    DimensionKind::Manual,
                    Point::new(a.0, a.1),
                    Point::new(b.0, b.1),
                    off,
                );
                project.add_dimension(fi, dim);
                dim_count += 1;
            }
        }
        if dim_count > 0 {
            report.warnings.push(
                "dimension line offsets are assumed (36\" away from the walls): the file's value was not found"
                    .into(),
            );
        }
        if dims_skipped > 0 {
            report.warnings.push(format!(
                "{dims_skipped} dimension object(s) are not linear strings (angular, radial, elevation) and were skipped"
            ));
        }
    }
    report.count("dimensions", dim_count);

    // Text notes.
    let mut text_count = 0usize;
    if opts.text {
        let layer_names: HashMap<i32, String> = inventory
            .as_ref()
            .ok()
            .and_then(|(_, v)| v.layer_sets.first())
            .map(|set| set.layers.iter().map(|l| (l.id, l.name.clone())).collect())
            .unwrap_or_default();
        for tn in tree.of_kind(texts::TEXT, 0) {
            let Some(fi) = floor_of(&tree, &chief_floors, tn) else {
                continue;
            };
            if [6u8, 79, 9, 10, 23, 24, 15, 21, 68]
                .iter()
                .any(|&c| tree.ancestor_of_class(tn, c).is_some())
            {
                continue;
            }
            let Some(t) = texts::decode_text(bytes, &tree, tn) else {
                continue;
            };
            let height = texts::assumed_height(&t);
            let width = 0.6 * height * t.text.chars().count() as f64;
            let layer = t
                .layer_id
                .and_then(|id| layer_names.get(&id))
                .filter(|n| n.as_str() != "Default")
                .map_or_else(|| "Text".to_string(), |n| canonical_layer(&project, n));
            project.add_cad(
                fi,
                layer,
                plan_core::CadItem::Text {
                    pos: Point::new(t.center_x - width / 2.0, t.top_y - height),
                    text: t.text,
                    height,
                    angle: 0.0,
                },
            );
            text_count += 1;
        }
        if text_count > 0 {
            report.warnings.push(
                "text notes import with an assumed height (4.5\" regular, 8\" bold), no rotation and a centred width estimate"
                    .into(),
            );
        }
    }
    report.count("texts", text_count);

    // Skipped classes: what sits on a floor that no importer handled.
    let mut skipped: BTreeMap<(u8, u8), usize> = BTreeMap::new();
    for (i, n) in tree.nodes.iter().enumerate() {
        if floor_of(&tree, &chief_floors, i).is_none() {
            continue;
        }
        if matches!(n.class, 6 | 9 | 10 | 23 | 24 | 25 | 30) || PART_CLASSES.contains(&n.class) {
            continue;
        }
        if [6u8, 23, 24, 9, 10, 15, 21]
            .iter()
            .any(|&c| tree.ancestor_of_class(i, c).is_some())
        {
            continue;
        }
        *skipped.entry((n.class, n.version)).or_insert(0) += 1;
    }
    let mut list: Vec<SkippedClass> = skipped
        .into_iter()
        .map(|((class, version), count)| {
            let (label, conf) =
                floors::class_label(class, version).unwrap_or(("unidentified", "None"));
            SkippedClass {
                class,
                version,
                label: label.to_string(),
                confidence: conf.to_string(),
                count,
            }
        })
        .collect();
    list.sort_by(|a, b| b.count.cmp(&a.count).then(a.class.cmp(&b.class)));
    report.skipped_classes = list;

    // Drop trailing floors with nothing on them (the template always
    // carries four), keeping at least one.
    let used: Vec<usize> = project
        .floors
        .iter()
        .map(|f| {
            f.walls.len() + f.openings.len() + f.room_names.len() + f.dimensions.len() + f.cad.len()
        })
        .collect();
    // The first floor above grade is always kept.
    let keep = project
        .floors
        .iter()
        .position(|f| f.elevation >= -1.0)
        .map_or(1, |i| i + 1);
    while project.floors.len() > keep && used[project.floors.len() - 1] == 0 {
        project.floors.pop();
    }
    for f in &project.floors {
        report.floors.push(FloorReport {
            name: f.name.clone(),
            elevation: f.elevation,
            ceiling_height: f.ceiling_height,
            walls: f.walls.len(),
            openings: f.openings.len(),
            rooms: f.room_names.len(),
            dimensions: f.dimensions.len(),
        });
    }
    report.count("floors", project.floors.len());
    Ok(ImportResult { project, report })
}

#[cfg(test)]
mod tests;
