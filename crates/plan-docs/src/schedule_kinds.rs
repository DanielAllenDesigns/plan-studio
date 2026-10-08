//! Rows, columns and callout labels for the schedules placed in the plan.
//!
//! A [`plan_core::schedules::Schedule`] only says *what* to list and how the
//! table looks. This module reads the plan and produces:
//!
//! * [`table`]: the schedule as a [`crate::Schedule`] of strings (title,
//!   visible columns in order, sorted and filtered rows) for drawing, CSV and
//!   the schedule window;
//! * [`callouts`] / [`floor_callouts`]: the label (`D01`, `W03`, `C-12`...)
//!   of every door, window, cabinet and fixture, with the point to draw it at;
//! * [`entries`]: the raw objects of a kind, one per row, with every field of
//!   [`ScheduleKind::fields`].
//!
//! # Numbering
//!
//! Objects are ordered by floor, then in reading order across the plan (x
//! then y of the object's centre). A mark is the schedule's prefix and a
//! two-digit number (`D01`); [`Numbering::ByFloor`] restarts on every floor,
//! [`Numbering::Whole`] keeps counting up through the floors. The numbers are
//! always counted over the whole plan, so a schedule that lists one floor
//! still shows the right marks. A door or window whose own schedule number is
//! set (`Opening::schedule_number`) shows that instead.
//!
//! Fixtures, furniture and plants are the placed library symbols, told apart
//! by their library category (`Plants`, `Furniture`, `Plumbing`, `Bath &
//! Kitchen`, `Lighting`).

use crate::schedule::{
    kind_label, ordered_openings, perimeter, reading_key, room_area_sq_ft, wall_number_of,
    wall_numbers,
};
use crate::Schedule as Table;
use plan_cabinets::{auto_label, Cabinet, CabinetKind};
use plan_core::schedules::{
    FloorScope, Numbering, Schedule, ScheduleKind, ScheduleLayer, SortSpec,
};
use plan_core::units::fmt_ft_in;
use plan_core::{detect_rooms, Id, MoldingKind, OpeningKind, PlacedSymbol, Point, Project, Room};
use plan_electrical::ElectricalLayer;
use plan_framing::{FramingMember, ManualMemberKind, Member};
use plan_library::Library;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// Rooms of the floor being edited, already detected by the caller, so the
/// Room schedule matches what the plan shows. Other floors are detected here.
pub type ActiveRooms<'a> = Option<(usize, &'a [Room])>;

/// Distance a door or window callout sits off the wall face, inches.
const CALLOUT_GAP: f64 = 9.0;

/// One scheduled object (or one grouped line, for framing).
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub floor: usize,
    /// The object's id; `0` for lines made of several objects (framing).
    pub id: Id,
    /// Where the object is (its centre), plan inches.
    pub position: Point,
    /// Where its callout label is drawn.
    pub callout: Point,
    /// The mark the object's own dialog sets, which replaces the number.
    pub mark_override: Option<String>,
    /// `(field id, text)` for every field of the kind. `mark` is filled in by
    /// [`number`].
    pub cells: Vec<(&'static str, String)>,
    /// The kind of object, which differs from the schedule's kind in a
    /// General schedule.
    pub kind: ScheduleKind,
    /// Short name and size, for the General schedule.
    pub name: String,
    pub size: String,
}

impl Entry {
    /// The text of field `id` (empty when the entry has no such field).
    pub fn cell(&self, id: &str) -> &str {
        self.cells
            .iter()
            .find(|(k, _)| *k == id)
            .map_or("", |(_, v)| v.as_str())
    }

    fn set(&mut self, id: &'static str, text: String) {
        match self.cells.iter_mut().find(|(k, _)| *k == id) {
            Some((_, v)) => *v = text,
            None => self.cells.push((id, text)),
        }
    }
}

/// A label to draw next to an object.
#[derive(Debug, Clone, PartialEq)]
pub struct Callout {
    pub kind: ScheduleKind,
    pub floor: usize,
    /// Id of the door, window, cabinet or placed symbol.
    pub object: Id,
    pub text: String,
    /// Where the label's centre goes, plan inches.
    pub at: Point,
}

// ===================================================================
// Collecting the objects
// ===================================================================

fn new_entry(floor: usize, id: Id, position: Point, kind: ScheduleKind) -> Entry {
    Entry {
        floor,
        id,
        position,
        callout: position,
        mark_override: None,
        cells: Vec::new(),
        kind,
        name: String::new(),
        size: String::new(),
    }
}

fn size_text(w: f64, h: f64) -> String {
    format!("{} x {}", fmt_ft_in(w), fmt_ft_in(h))
}

fn by_position(v: &mut [Entry]) {
    v.sort_by_key(|e| (e.floor, reading_key(e.position), e.id));
}

/// The label the plan shows for an opening that no schedule numbers: its
/// size (`3068`) or its own text.
fn opening_label(o: &plan_core::Opening) -> String {
    o.plan_label(&plan_core::OpeningLabelDefaults::default(), None)
        .unwrap_or_else(|| o.label())
}

fn door_window(project: &Project, kind: ScheduleKind) -> Vec<Entry> {
    let ok = if kind == ScheduleKind::Door {
        OpeningKind::Door
    } else {
        OpeningKind::Window
    };
    let mut out = Vec::new();
    for (fi, f) in project.floors.iter().enumerate() {
        let numbers = wall_numbers(f);
        for (o, w) in ordered_openings(f, ok) {
            let centre = w.point_at(o.center_offset);
            let mut e = new_entry(fi, o.id, centre, kind);
            e.callout = centre.add(w.normal().scale(w.thickness * 0.5 + CALLOUT_GAP));
            e.mark_override = o.schedule_number.clone().filter(|s| !s.trim().is_empty());
            e.name = kind.name().to_string();
            e.size = size_text(o.width, o.height);
            let wall = wall_number_of(&numbers, w.id);
            e.cells = if kind == ScheduleKind::Door {
                vec![
                    ("mark", String::new()),
                    ("floor", f.name.clone()),
                    ("width", fmt_ft_in(o.width)),
                    ("height", fmt_ft_in(o.height)),
                    ("type", kind_label(w.kind).to_string()),
                    ("wall", wall),
                    (
                        "swing",
                        if o.swing_flipped {
                            "Flipped"
                        } else {
                            "Standard"
                        }
                        .to_string(),
                    ),
                    ("style", o.type_name().to_string()),
                    ("label", opening_label(o)),
                ]
            } else {
                vec![
                    ("mark", String::new()),
                    ("width", fmt_ft_in(o.width)),
                    ("height", fmt_ft_in(o.height)),
                    ("sill", fmt_ft_in(o.sill_height)),
                    ("head", fmt_ft_in(o.sill_height + o.height)),
                    ("type", kind_label(w.kind).to_string()),
                    ("wall", wall),
                    ("floor", f.name.clone()),
                    ("style", o.type_name().to_string()),
                    ("label", opening_label(o)),
                ]
            };
            out.push(e);
        }
    }
    out
}

fn walls(project: &Project) -> Vec<Entry> {
    let mut out = Vec::new();
    for (fi, f) in project.floors.iter().enumerate() {
        for (w, _) in wall_numbers(f) {
            let mid = w.start.add(w.end).scale(0.5);
            let mut e = new_entry(fi, w.id, mid, ScheduleKind::Wall);
            e.name = "Wall".to_string();
            e.size = format!("{} x {}", fmt_ft_in(w.length()), fmt_ft_in(w.height));
            e.cells = vec![
                ("mark", String::new()),
                ("type", kind_label(w.kind).to_string()),
                ("length", fmt_ft_in(w.length())),
                ("thickness", fmt_ft_in(w.thickness)),
                ("height", fmt_ft_in(w.height)),
                ("area", format!("{:.1}", w.length() * w.height / 144.0)),
                ("openings", f.openings_on(w.id).count().to_string()),
                ("floor", f.name.clone()),
            ];
            out.push(e);
        }
    }
    out
}

fn rooms(project: &Project, active: ActiveRooms) -> Vec<Entry> {
    let mut out = Vec::new();
    for (fi, f) in project.floors.iter().enumerate() {
        let detected;
        let list: &[Room] = match active {
            Some((af, r)) if af == fi => r,
            _ => {
                detected = detect_rooms(&f.walls, 1.0);
                &detected
            }
        };
        for r in list {
            let spec = r.name_entry(&f.room_names);
            let name = spec.map_or_else(|| r.label.clone(), |n| n.name.clone());
            let ceiling = spec
                .and_then(|n| n.ceiling_height)
                .unwrap_or(f.ceiling_height);
            let mut e = new_entry(fi, 0, r.centroid, ScheduleKind::Room);
            e.name = name.clone();
            e.size = format!("{:.1} sq ft", room_area_sq_ft(r));
            e.cells = vec![
                ("mark", String::new()),
                ("name", name),
                // The Interior Area, like the plan label (QA-03); the
                // centerline ("Standard") area is kept as its own value.
                ("area", format!("{:.1}", room_area_sq_ft(r))),
                ("standard_area", format!("{:.1}", r.area_sq_ft())),
                ("perimeter", format!("{:.1}", perimeter(&r.polygon) / 12.0)),
                ("ceiling_height", fmt_ft_in(ceiling)),
                (
                    "floor_finish",
                    spec.and_then(|n| n.floor_finish.clone())
                        .unwrap_or_default(),
                ),
                (
                    "ceiling_finish",
                    spec.and_then(|n| n.ceiling_finish.clone())
                        .unwrap_or_default(),
                ),
                ("floor", f.name.clone()),
                // The Room Finish schedule's columns.
                (
                    "base",
                    spec.map(|n| molding_profile(n, MoldingKind::Base))
                        .unwrap_or_default(),
                ),
                (
                    "crown",
                    spec.map(|n| molding_profile(n, MoldingKind::Crown))
                        .unwrap_or_default(),
                ),
                (
                    "wall_finish",
                    spec.and_then(|n| n.misc.as_ref())
                        .map(|m| m.wall_covering.clone())
                        .unwrap_or_default(),
                ),
            ];
            out.push(e);
        }
    }
    // Rooms keep the order the caller (or room detection) gave them.
    out
}

/// The profile name of the room's `kind` molding ("" = none).
fn molding_profile(n: &plan_core::RoomName, kind: MoldingKind) -> String {
    n.moldings
        .iter()
        .find(|m| m.kind == kind)
        .map(|m| m.profile.clone())
        .unwrap_or_default()
}

/// The Room Finish schedule: the rooms again, with the finish columns.
fn room_finishes(project: &Project, active: ActiveRooms) -> Vec<Entry> {
    let mut out = rooms(project, active);
    for e in &mut out {
        e.kind = ScheduleKind::RoomFinish;
    }
    out
}

/// The stairs and ramps of every floor (landings are not listed): the
/// solved riser and tread of each, from `plan_stairs::solve`.
fn stairs(project: &Project) -> Vec<Entry> {
    use plan_stairs::{solve, Stair, StairShape};
    let mut out = Vec::new();
    for (fi, f) in project.floors.iter().enumerate() {
        for v in &f.stairs {
            let Ok(s) = serde_json::from_value::<Stair>(v.clone()) else {
                continue;
            };
            let p = &s.params;
            let kind = match p.shape {
                StairShape::Landing { .. } => continue,
                StairShape::Straight => "Straight",
                StairShape::LShaped { .. } => "L-Shaped",
                StairShape::UShaped { .. } => "U-Shaped",
                StairShape::Winder { .. } => "Winder",
                StairShape::Ramp { .. } => "Ramp",
                StairShape::Curved { .. } => "Curved",
            };
            let sol = solve(p);
            let ramp = matches!(p.shape, StairShape::Ramp { .. });
            let mut e = new_entry(fi, s.id, s.origin, ScheduleKind::Stair);
            e.name = kind.to_string();
            e.size = size_text(p.width, p.total_rise);
            e.cells = vec![
                ("mark", String::new()),
                ("type", kind.to_string()),
                ("width", fmt_ft_in(p.width)),
                ("rise", fmt_ft_in(p.total_rise)),
                (
                    "risers",
                    if ramp {
                        String::new()
                    } else {
                        sol.risers.to_string()
                    },
                ),
                (
                    "riser",
                    if ramp {
                        String::new()
                    } else {
                        format!("{:.3}\"", sol.riser_height)
                    },
                ),
                (
                    "tread",
                    if ramp {
                        String::new()
                    } else {
                        format!("{:.2}\"", sol.tread_depth)
                    },
                ),
                ("run", fmt_ft_in(sol.total_run)),
                ("floor", f.name.clone()),
            ];
            out.push(e);
        }
    }
    by_position(&mut out);
    out
}

/// The numbered notes of the plan: text objects reading `Note 3: ...` (or
/// the prefix of another note type), ordered by note type, then number.
fn notes(project: &Project) -> Vec<Entry> {
    let types = project.note_types();
    let mut keyed: Vec<((usize, u32, usize), Entry)> = Vec::new();
    for (fi, f) in project.floors.iter().enumerate() {
        for c in &f.cad {
            let plan_core::CadItem::Text { pos, text, .. } = &c.item else {
                continue;
            };
            let Some((type_name, n)) = types.parse(text) else {
                continue;
            };
            let prefix = types.get(type_name).map_or("Note", |t| t.prefix.as_str());
            let body = text
                .split_once(':')
                .map_or(text.as_str(), |(_, b)| b)
                .trim()
                .to_string();
            let mut e = new_entry(fi, c.id, *pos, ScheduleKind::Note);
            e.mark_override = Some(format!("{prefix} {n}"));
            e.name = body.clone();
            e.size = String::new();
            e.cells = vec![
                ("mark", String::new()),
                ("type", type_name.to_string()),
                ("note", body),
                ("floor", f.name.clone()),
            ];
            let type_order = types
                .types
                .iter()
                .position(|t| t.name == type_name)
                .unwrap_or(0);
            keyed.push(((type_order, n, fi), e));
        }
    }
    keyed.sort_by_key(|(k, e)| (*k, e.id));
    keyed.into_iter().map(|(_, e)| e).collect()
}

/// `FullHeight` as "Full Height": the cabinet kinds spelled out, whatever
/// kinds the cabinet crate has.
fn cabinet_kind_name(k: CabinetKind) -> String {
    let raw = format!("{k:?}");
    let mut out = String::with_capacity(raw.len() + 2);
    for (i, c) in raw.chars().enumerate() {
        if i > 0 && c.is_uppercase() {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

fn cabinets(project: &Project) -> Vec<Entry> {
    let mut out = Vec::new();
    for (fi, f) in project.floors.iter().enumerate() {
        for v in &f.cabinets {
            let Ok(c) = serde_json::from_value::<Cabinet>(v.clone()) else {
                continue;
            };
            // The local origin is the back-left corner; the centre is half
            // the width along X and half the depth along Y, rotated.
            let (s, co) = c.angle.sin_cos();
            let (lx, ly) = (c.width * 0.5, c.depth * 0.5);
            let centre = Point::new(
                c.position.x + lx * co - ly * s,
                c.position.y + lx * s + ly * co,
            );
            let label = if c.label.trim().is_empty() {
                auto_label(&c)
            } else {
                c.label.clone()
            };
            let mut e = new_entry(fi, c.id, centre, ScheduleKind::Cabinet);
            e.name = label.clone();
            e.size = format!("{} x {}", fmt_ft_in(c.width), fmt_ft_in(c.height));
            e.cells = vec![
                ("mark", String::new()),
                ("label", label),
                (
                    "type",
                    c.preset
                        .map_or_else(|| cabinet_kind_name(c.kind), |p| p.name().to_string()),
                ),
                ("width", fmt_ft_in(c.width)),
                ("depth", fmt_ft_in(c.depth)),
                ("height", fmt_ft_in(c.height)),
                ("elevation", fmt_ft_in(c.elevation)),
                (
                    "countertop",
                    if c.countertop.is_some() { "Yes" } else { "No" }.to_string(),
                ),
                ("floor", f.name.clone()),
                ("door_style", c.door_style.name.clone()),
                ("drawer_style", c.drawer_style.name.clone()),
                ("finish", c.finish_name()),
                ("hardware", c.hardware_name()),
            ];
            out.push(e);
        }
    }
    by_position(&mut out);
    out
}

fn electrical(project: &Project) -> Vec<Entry> {
    let mut out = Vec::new();
    for (fi, f) in project.floors.iter().enumerate() {
        let Some(v) = f.electrical.as_ref() else {
            continue;
        };
        let Ok(layer) = serde_json::from_value::<ElectricalLayer>(v.clone()) else {
            continue;
        };
        let numbers = wall_numbers(f);
        for d in &layer.devices {
            let mut e = new_entry(fi, d.id, d.position, ScheduleKind::Electrical);
            e.name = d.kind.name().to_string();
            e.size = fmt_ft_in(d.height);
            e.cells = vec![
                ("mark", String::new()),
                ("type", d.kind.name().to_string()),
                ("count", "1".to_string()),
                ("label", d.label.clone()),
                ("height", fmt_ft_in(d.height)),
                (
                    "circuit",
                    d.circuit.map(|c| c.to_string()).unwrap_or_default(),
                ),
                (
                    "wall",
                    d.wall_id
                        .map(|w| wall_number_of(&numbers, w))
                        .unwrap_or_default(),
                ),
                ("floor", f.name.clone()),
            ];
            out.push(e);
        }
    }
    by_position(&mut out);
    out
}

/// One grouped framing line: members of one kind, size and length.
fn framing(project: &Project) -> Vec<Entry> {
    let mut out = Vec::new();
    for (fi, f) in project.floors.iter().enumerate() {
        // (member kind, size, length in 1/8") -> count
        let mut groups: BTreeMap<(String, String, i64), u32> = BTreeMap::new();
        let mut add = |kind: &str, size: String, length: f64| {
            *groups
                .entry((kind.to_string(), size, (length * 8.0).round() as i64))
                .or_default() += 1;
        };
        for v in &f.framing {
            let record = v.as_object().filter(|o| o.len() == 1).and_then(|o| {
                o.get("Manual")
                    .or_else(|| o.get("Built"))
                    .and_then(|m| serde_json::from_value::<FramingMember>(m.clone()).ok())
            });
            if let Some(m) = record {
                if matches!(
                    m.kind,
                    ManualMemberKind::BearingLine | ManualMemberKind::TrussBase
                ) {
                    continue;
                }
                add(m.kind.name(), m.lumber.name(), m.plan_length());
            } else if let Ok(m) = serde_json::from_value::<Member>(v.clone()) {
                add(m.kind.name(), m.lumber.nominal_name(), m.length);
            }
        }
        for ((kind, size, eighths), qty) in groups {
            let length = eighths as f64 / 8.0;
            let mut e = new_entry(fi, 0, Point::ZERO, ScheduleKind::Framing);
            e.name = format!("{kind} {size}");
            e.size = format!("{size} x {}", fmt_ft_in(length));
            // Nominal "2x6" gives the board feet: T x W x length / 144 per piece.
            let board = size
                .split_once('x')
                .and_then(|(t, w)| Some(t.parse::<f64>().ok()? * w.parse::<f64>().ok()?))
                .map_or(0.0, |tw| tw * length / 144.0 * f64::from(qty));
            e.cells = vec![
                ("mark", String::new()),
                ("type", kind),
                ("size", size),
                ("length", fmt_ft_in(length)),
                ("qty", qty.to_string()),
                ("linear", format!("{:.1}", length * f64::from(qty) / 12.0)),
                ("board_feet", format!("{board:.1}")),
                ("floor", f.name.clone()),
            ];
            out.push(e);
        }
    }
    out
}

fn library() -> &'static Library {
    static LIB: OnceLock<Library> = OnceLock::new();
    LIB.get_or_init(Library::with_all_core)
}

/// Fixture, furniture or plant, by library category; `None` for other
/// symbols (electrical, exterior, unknown).
pub fn symbol_kind(s: &PlacedSymbol) -> Option<ScheduleKind> {
    let id = s.catalog_id.to_lowercase();
    let path: Vec<String> = match library().get(&s.catalog_id) {
        Some(item) => item.category.clone(),
        None => Vec::new(),
    };
    let in_path = |names: &[&str]| path.iter().any(|p| names.contains(&p.as_str()));
    if in_path(&["Plants"]) || (path.is_empty() && id.contains("plant")) {
        Some(ScheduleKind::Plant)
    } else if in_path(&["Furniture"]) || (path.is_empty() && id.contains("furniture")) {
        Some(ScheduleKind::Furniture)
    } else if in_path(&["Plumbing", "Bath & Kitchen", "Lighting"])
        || (path.is_empty() && (id.contains("plumbing") || id.contains("lighting")))
    {
        Some(ScheduleKind::Fixture)
    } else {
        None
    }
}

fn symbols(project: &Project, kind: Option<ScheduleKind>) -> Vec<Entry> {
    let mut out = Vec::new();
    for (fi, f) in project.floors.iter().enumerate() {
        for s in &f.symbols {
            let Some(k) = symbol_kind(s) else { continue };
            if kind.is_some_and(|want| want != k) {
                continue;
            }
            let item = library().get(&s.catalog_id);
            let name = if !s.label.trim().is_empty() {
                s.label.clone()
            } else {
                item.map_or_else(|| s.catalog_id.clone(), |i| i.name.clone())
            };
            let fp = s.footprint();
            let centre = fp
                .iter()
                .fold(Point::ZERO, |a, p| a.add(*p))
                .scale(1.0 / fp.len() as f64);
            let mut e = new_entry(fi, s.id, centre, k);
            e.name = name.clone();
            e.size = size_text(s.width, s.depth);
            e.cells = vec![
                ("mark", String::new()),
                ("name", name),
                (
                    "category",
                    item.map(|i| i.category.join(" > ")).unwrap_or_default(),
                ),
                ("width", fmt_ft_in(s.width)),
                ("depth", fmt_ft_in(s.depth)),
                ("height", fmt_ft_in(s.height)),
                ("elevation", fmt_ft_in(s.elevation)),
                ("floor", f.name.clone()),
            ];
            out.push(e);
        }
    }
    if kind == Some(ScheduleKind::Plant) {
        out.extend(terrain_plants(project));
    }
    by_position(&mut out);
    out
}

/// The plants of the terrain's landscaping runs (Plant tool), one row per
/// plant. They belong to the site, so they sit on the first floor's list.
fn terrain_plants(project: &Project) -> Vec<Entry> {
    let Some(runs) = project
        .terrain
        .as_ref()
        .and_then(|t| t.get("terrain"))
        .and_then(|t| t.get("landscape"))
        .and_then(|l| l.as_array())
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for run in runs {
        if run.get("kind").and_then(|k| k.as_str()) != Some("Plants") {
            continue;
        }
        let num = |k: &str| {
            run.get(k)
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0)
        };
        let points: Vec<Point> = run
            .get("points")
            .and_then(|p| p.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|p| Some(Point::new(p.get("x")?.as_f64()?, p.get("y")?.as_f64()?)))
                    .collect()
            })
            .unwrap_or_default();
        let Some(first) = points.first().copied() else {
            continue;
        };
        let (spacing, canopy, height) = (num("spacing"), num("size"), num("height"));
        let length: f64 = points.windows(2).map(|w| w[0].dist(w[1])).sum();
        let count = if spacing > 0.0 && length > 0.0 {
            (length / spacing + 1e-9).floor() as usize + 1
        } else {
            1
        };
        let id = run.get("plant").and_then(|p| p.as_str()).unwrap_or("");
        let item = library().get(id);
        let name = item.map_or_else(
            || {
                if id.is_empty() {
                    "Plant".to_string()
                } else {
                    id.to_string()
                }
            },
            |i| i.name.clone(),
        );
        let floor_name = project
            .floors
            .first()
            .map(|f| f.name.clone())
            .unwrap_or_default();
        for _ in 0..count {
            let mut e = new_entry(0, 0, first, ScheduleKind::Plant);
            e.name = name.clone();
            e.size = size_text(canopy, canopy);
            e.cells = vec![
                ("mark", String::new()),
                ("name", name.clone()),
                (
                    "category",
                    item.map(|i| i.category.join(" > "))
                        .unwrap_or_else(|| "Plants".to_string()),
                ),
                ("width", fmt_ft_in(canopy)),
                ("depth", fmt_ft_in(canopy)),
                ("height", fmt_ft_in(height)),
                ("elevation", fmt_ft_in(0.0)),
                ("floor", floor_name.clone()),
            ];
            out.push(e);
        }
    }
    out
}

/// Every object a General schedule can list.
const GENERAL_KINDS: [ScheduleKind; 7] = [
    ScheduleKind::Door,
    ScheduleKind::Window,
    ScheduleKind::Cabinet,
    ScheduleKind::Electrical,
    ScheduleKind::Fixture,
    ScheduleKind::Furniture,
    ScheduleKind::Plant,
];

fn general(project: &Project) -> Vec<Entry> {
    let mut out = Vec::new();
    for k in GENERAL_KINDS {
        for e in entries(project, k, None) {
            let mut g = e.clone();
            g.cells = vec![
                ("mark", String::new()),
                ("category", k.name().to_string()),
                ("name", e.name.clone()),
                ("size", e.size.clone()),
                ("floor", e.cell("floor").to_string()),
            ];
            if g.cell("floor").is_empty() {
                let floor = project
                    .floors
                    .get(e.floor)
                    .map(|f| f.name.clone())
                    .unwrap_or_default();
                g.set("floor", floor);
            }
            out.push(g);
        }
    }
    out.sort_by_key(|e| (e.floor, reading_key(e.position), e.id));
    out
}

/// The objects of `kind` on every floor, floor by floor, in reading order
/// (rooms in detection order, framing grouped). Marks are empty; see
/// [`number`]. `active` lets the Room schedule use the editor's own room
/// detection for that floor.
pub fn entries(project: &Project, kind: ScheduleKind, active: ActiveRooms) -> Vec<Entry> {
    match kind {
        ScheduleKind::Door | ScheduleKind::Window => door_window(project, kind),
        ScheduleKind::Wall => walls(project),
        ScheduleKind::Room => rooms(project, active),
        ScheduleKind::Cabinet => cabinets(project),
        ScheduleKind::Electrical => electrical(project),
        ScheduleKind::Framing => framing(project),
        ScheduleKind::Fixture | ScheduleKind::Furniture | ScheduleKind::Plant => {
            symbols(project, Some(kind))
        }
        ScheduleKind::Stair => stairs(project),
        ScheduleKind::RoomFinish => room_finishes(project, active),
        ScheduleKind::Note => notes(project),
        ScheduleKind::General => general(project),
    }
}

// ===================================================================
// Marks
// ===================================================================

/// Fills in each entry's `mark` (entries are in floor order): `prefix` and a
/// two-digit number, or the object's own schedule number.
pub fn number(entries: &mut [Entry], numbering: Numbering, prefix: &str) {
    let mut on_floor = 0usize;
    let mut floor = usize::MAX;
    for (i, e) in entries.iter_mut().enumerate() {
        if e.floor != floor {
            floor = e.floor;
            on_floor = 0;
        }
        on_floor += 1;
        let n = match numbering {
            Numbering::ByFloor => on_floor,
            Numbering::Whole => i + 1,
        };
        let mark = e
            .mark_override
            .clone()
            .unwrap_or_else(|| format!("{prefix}{n:02}"));
        e.set("mark", mark);
    }
}

/// The label of every object `def` numbers on `floor` (empty unless the
/// schedule shows labels and its kind has them).
pub fn callouts(project: &Project, floor: usize, def: &Schedule) -> Vec<Callout> {
    if !def.show_labels || !def.kind.has_labels() {
        return Vec::new();
    }
    let mut all = entries(project, def.kind, None);
    number(&mut all, def.numbering, &def.label_prefix);
    all.into_iter()
        .filter(|e| e.floor == floor)
        .map(|e| Callout {
            kind: def.kind,
            floor,
            object: e.id,
            text: e.cell("mark").to_string(),
            at: e.callout,
        })
        .collect()
}

/// Every callout on `floor`: for each of door, window, cabinet and fixture,
/// the labels of the first schedule of that kind on the floor that shows them.
pub fn floor_callouts(project: &Project, floor: usize) -> Vec<Callout> {
    let Some(f) = project.floors.get(floor) else {
        return Vec::new();
    };
    callouts_of(project, floor, &ScheduleLayer::load(f))
}

/// [`floor_callouts`] for an already loaded layer.
pub fn callouts_of(project: &Project, floor: usize, layer: &ScheduleLayer) -> Vec<Callout> {
    let mut out = Vec::new();
    for kind in [
        ScheduleKind::Door,
        ScheduleKind::Window,
        ScheduleKind::Cabinet,
        ScheduleKind::Fixture,
    ] {
        if let Some(def) = layer.label_source(kind) {
            out.extend(callouts(project, floor, def));
        }
    }
    out
}

// ===================================================================
// The table
// ===================================================================

/// Numeric-aware comparison: `D2` before `D10`, `2'-0"` before `10'-0"`.
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    fn chunks(s: &str) -> Vec<(bool, String)> {
        let mut out: Vec<(bool, String)> = Vec::new();
        for c in s.chars() {
            let digit = c.is_ascii_digit();
            match out.last_mut() {
                Some((d, text)) if *d == digit => text.push(c.to_ascii_lowercase()),
                _ => out.push((digit, c.to_ascii_lowercase().to_string())),
            }
        }
        out
    }
    let (ca, cb) = (chunks(a), chunks(b));
    for (x, y) in ca.iter().zip(cb.iter()) {
        let ord = if x.0 && y.0 {
            let (nx, ny) = (
                x.1.trim_start_matches('0').len(),
                y.1.trim_start_matches('0').len(),
            );
            nx.cmp(&ny)
                .then_with(|| x.1.trim_start_matches('0').cmp(y.1.trim_start_matches('0')))
        } else {
            x.1.cmp(&y.1)
        };
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
    }
    ca.len().cmp(&cb.len())
}

fn sorted(rows: &mut [Entry], sort: &SortSpec) {
    if sort.field.is_empty() {
        return;
    }
    rows.sort_by(|a, b| {
        let o = natural_cmp(a.cell(&sort.field), b.cell(&sort.field));
        if sort.descending {
            o.reverse()
        } else {
            o
        }
    });
}

/// The rows `def` lists, marked, scoped to its floor, filtered and sorted.
/// `home_floor` is the floor the schedule is placed on.
pub fn rows(
    project: &Project,
    def: &Schedule,
    home_floor: usize,
    active: ActiveRooms,
) -> Vec<Entry> {
    let mut all = entries(project, def.kind, active);
    number(&mut all, def.numbering, &def.label_prefix);
    let needle = def.filter.trim().to_lowercase();
    let mut shown: Vec<Entry> = all
        .into_iter()
        .filter(|e| def.floor_scope == FloorScope::All || e.floor == home_floor)
        .filter(|e| {
            needle.is_empty()
                || e.cells
                    .iter()
                    .any(|(_, v)| v.to_lowercase().contains(&needle))
        })
        .collect();
    sorted(&mut shown, &def.sort);
    shown
}

/// The object behind a table row (a grouped row has several).
#[derive(Debug, Clone, PartialEq)]
pub struct RowTarget {
    pub kind: ScheduleKind,
    pub floor: usize,
    /// The object's id; `0` for a room (find it by `position`) or a plant
    /// of the terrain.
    pub id: Id,
    pub position: Point,
}

/// Fields whose cells are numbers a Totals line adds up.
const SUMMED: [&str; 7] = [
    "area",
    "standard_area",
    "perimeter",
    "qty",
    "count",
    "linear",
    "board_feet",
];

fn visible_columns(def: &Schedule) -> Vec<&plan_core::schedules::ColumnSpec> {
    def.visible_columns()
        .filter(|c| def.kind.fields().iter().any(|f| f.id == c.field))
        .collect()
}

/// The rows as listed (one per object) or grouped by `def.group_by`, and a
/// Totals line when `def.totals` is on, with the objects behind each row.
/// The totals row has no targets.
fn display_rows(
    project: &Project,
    def: &Schedule,
    home_floor: usize,
    active: ActiveRooms,
) -> Vec<(Vec<String>, Vec<RowTarget>)> {
    let shown = rows(project, def, home_floor, active);
    let columns = visible_columns(def);
    let target = |e: &Entry| RowTarget {
        kind: e.kind,
        floor: e.floor,
        id: e.id,
        position: e.position,
    };
    let grouping = def
        .kind
        .fields()
        .iter()
        .any(|f| f.id == def.group_by && !def.group_by.is_empty());
    let mut out: Vec<(Vec<String>, Vec<RowTarget>)> = Vec::new();
    if grouping {
        // Groups keep the order of their first member.
        let mut groups: Vec<(String, Vec<&Entry>)> = Vec::new();
        for e in &shown {
            let key = e.cell(&def.group_by).to_string();
            match groups.iter_mut().find(|(k, _)| *k == key) {
                Some((_, v)) => v.push(e),
                None => groups.push((key, vec![e])),
            }
        }
        for (_, members) in groups {
            let cells = columns
                .iter()
                .map(|c| {
                    let first = members[0].cell(&c.field);
                    let same = members.iter().all(|m| m.cell(&c.field) == first);
                    if c.field == "mark" && members.len() > 1 {
                        let last = members[members.len() - 1].cell("mark");
                        format!("{first}-{last} ({})", members.len())
                    } else if c.field == "count" {
                        // A grouped row counts every device behind it.
                        members
                            .iter()
                            .filter_map(|m| m.cell("count").trim().parse::<u32>().ok())
                            .sum::<u32>()
                            .to_string()
                    } else if same || c.field == def.group_by {
                        first.to_string()
                    } else {
                        "*".to_string()
                    }
                })
                .collect();
            out.push((cells, members.iter().map(|m| target(m)).collect()));
        }
    } else {
        for e in &shown {
            let cells = columns
                .iter()
                .map(|c| e.cell(&c.field).to_string())
                .collect();
            out.push((cells, vec![target(e)]));
        }
    }
    if def.totals && !columns.is_empty() {
        let mut cells = vec![String::new(); columns.len()];
        cells[0] = "Total".to_string();
        if columns.len() > 1 {
            cells[1] = shown.len().to_string();
        }
        for (i, c) in columns.iter().enumerate() {
            if i < 2 || !SUMMED.contains(&c.field.as_str()) {
                continue;
            }
            let sum: f64 = shown
                .iter()
                .filter_map(|e| e.cell(&c.field).trim().parse::<f64>().ok())
                .sum();
            cells[i] = format!("{sum:.1}");
        }
        out.push((cells, Vec::new()));
    }
    out
}

/// What each row of [`table`] stands for, in the same order.
pub fn row_targets(
    project: &Project,
    def: &Schedule,
    home_floor: usize,
    active: ActiveRooms,
) -> Vec<Vec<RowTarget>> {
    display_rows(project, def, home_floor, active)
        .into_iter()
        .map(|(_, t)| t)
        .collect()
}

/// The schedule as a table: title, the visible columns in their order, and
/// one row per object (or per group, with a Totals line, when the schedule
/// asks for them).
pub fn table(project: &Project, def: &Schedule, home_floor: usize, active: ActiveRooms) -> Table {
    let columns = visible_columns(def);
    Table {
        title: def.display_title(),
        columns: columns
            .iter()
            .map(|c| {
                if c.title.trim().is_empty() {
                    def.kind
                        .fields()
                        .iter()
                        .find(|f| f.id == c.field)
                        .map_or(String::new(), |f| f.title.to_string())
                } else {
                    c.title.clone()
                }
            })
            .collect(),
        rows: display_rows(project, def, home_floor, active)
            .into_iter()
            .map(|(cells, _)| cells)
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{house, rect_walls};
    use plan_core::schedules::ColumnSpec;
    use plan_core::{Floor, OpeningKind, WallKind};

    fn def(kind: ScheduleKind) -> Schedule {
        Schedule::new(kind, Point::ZERO)
    }

    /// A house with one of everything the kinds read.
    fn rich() -> Project {
        let mut p = house();
        let mut cab = Cabinet::base(24.0);
        cab.id = p.alloc_id();
        cab.position = Point::new(20.0, 20.0);
        let mut cab2 = Cabinet::base(30.0);
        cab2.id = p.alloc_id();
        cab2.position = Point::new(44.0, 20.0);
        p.floors[0].set_cabinets(&[cab, cab2]).unwrap();
        let mut layer = ElectricalLayer::default();
        let mut d = plan_electrical::Device {
            id: 0,
            kind: plan_electrical::DeviceKind::all()[0],
            position: Point::new(50.0, 3.0),
            angle: 0.0,
            height: 16.0,
            wall_id: None,
            circuit: Some(3),
            label: "A".into(),
            switched_by: Vec::new(),
            finish: String::new(),
            hide_label: false,
        };
        d.id = 1;
        layer.add(d);
        p.floors[0].electrical = Some(serde_json::to_value(&layer).unwrap());
        for (cat, x) in [
            ("core.plumbing.toilet_elongated", 60.0),
            ("plants.something", 100.0),
        ] {
            let mut s = PlacedSymbol::new(cat, Point::new(x, 100.0), 20.0, 30.0, 30.0);
            s.label = String::new();
            p.add_symbol(0, s);
        }
        let member = FramingMember::new(
            90,
            ManualMemberKind::Joist,
            Point::ZERO,
            Point::new(120.0, 0.0),
        );
        p.floors[0]
            .framing
            .push(serde_json::json!({ "Manual": member }));
        p.floors[0]
            .framing
            .push(serde_json::json!({ "Manual": member }));
        let stair = plan_stairs::Stair::new(
            p.alloc_id(),
            Point::new(30.0, 40.0),
            0.0,
            plan_stairs::StairParams::default(),
        );
        p.floors[0].set_stairs(&[stair]).unwrap();
        for text in [
            "Note 2: Verify at site",
            "E 1: Outlet at 16\" AFF",
            "Plain text",
        ] {
            p.add_cad(
                0,
                "CAD, Default",
                plan_core::CadItem::Text {
                    pos: Point::new(10.0, 10.0),
                    text: text.into(),
                    height: 3.0,
                    angle: 0.0,
                },
            );
        }
        p
    }

    #[test]
    fn every_kind_has_rows_where_objects_exist_and_every_field_is_filled() {
        let p = rich();
        for kind in ScheduleKind::ALL {
            let es = entries(&p, kind, None);
            if matches!(kind, ScheduleKind::Furniture) {
                // Nothing furnished in the sample.
                assert!(es.is_empty());
                continue;
            }
            assert!(!es.is_empty(), "{kind:?} has no rows");
            for e in &es {
                for fd in kind.fields() {
                    assert!(
                        e.cells.iter().any(|(k, _)| *k == fd.id),
                        "{kind:?} lacks field {}",
                        fd.id
                    );
                }
            }
        }
        // Plants: an unknown `plants.` id still counts through the id fallback.
        assert_eq!(entries(&p, ScheduleKind::Plant, None).len(), 1);
        assert_eq!(entries(&p, ScheduleKind::Fixture, None).len(), 1);
        let g = table(&p, &def(ScheduleKind::General), 0, None);
        assert!(g.rows.len() >= 8, "{:?}", g.rows);
    }

    #[test]
    fn marks_are_numbered_in_reading_order() {
        let mut p = Project::new("order");
        let w = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.5,
            109.125,
            WallKind::Exterior,
        );
        // Added right to left; numbered left to right.
        p.add_opening(0, w, 200.0, OpeningKind::Door).unwrap();
        p.add_opening(0, w, 60.0, OpeningKind::Door).unwrap();
        let t = table(&p, &def(ScheduleKind::Door), 0, None);
        assert_eq!(t.columns[0], "Mark");
        assert_eq!(t.rows[0][0], "D01");
        assert_eq!(t.rows[1][0], "D02");
        let x_of = |id: Id| {
            let o = p.floors[0].openings.iter().find(|o| o.id == id).unwrap();
            p.floors[0].wall(w).unwrap().point_at(o.center_offset).x
        };
        let calls = callouts(&p, 0, &def(ScheduleKind::Door));
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].text, "D01");
        assert_eq!(calls[1].text, "D02");
        assert!(x_of(calls[0].object) < x_of(calls[1].object));
        // The label sits off the wall face.
        assert!(calls[0].at.y.abs() > 3.25);
    }

    #[test]
    fn numbering_by_floor_restarts_and_whole_continues() {
        let mut p = house();
        p.floors.push(Floor::new("2nd Floor", 109.0));
        let w = p.add_wall(
            1,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.5,
            109.125,
            WallKind::Exterior,
        );
        p.add_opening(1, w, 100.0, OpeningKind::Door).unwrap();
        let by_floor = callouts(&p, 1, &def(ScheduleKind::Door));
        assert_eq!(by_floor[0].text, "D01");
        let mut whole = def(ScheduleKind::Door);
        whole.numbering = Numbering::Whole;
        let w2 = callouts(&p, 1, &whole);
        assert_eq!(w2[0].text, "D02");
        // A schedule on all floors lists both; this-floor lists one.
        let mut all = def(ScheduleKind::Door);
        all.floor_scope = FloorScope::All;
        assert_eq!(table(&p, &all, 0, None).rows.len(), 2);
        assert_eq!(table(&p, &def(ScheduleKind::Door), 0, None).rows.len(), 1);
        // Marks of a one-floor schedule still count over the whole plan.
        whole.floor_scope = FloorScope::ThisFloor;
        assert_eq!(table(&p, &whole, 1, None).rows[0][0], "D02");
    }

    #[test]
    fn labels_follow_the_schedule_on_the_floor() {
        let mut p = house();
        assert!(floor_callouts(&p, 0).is_empty());
        let mut layer = ScheduleLayer::default();
        layer.add(def(ScheduleKind::Window));
        let mut hidden = def(ScheduleKind::Door);
        hidden.show_labels = false;
        layer.add(hidden);
        layer.store(&mut p.floors[0]);
        let calls = floor_callouts(&p, 0);
        assert_eq!(calls.len(), 2, "two windows, no door labels");
        assert_eq!(calls[0].text, "W01");
        assert_eq!(calls[1].text, "W02");
        assert!(calls.iter().all(|c| c.kind == ScheduleKind::Window));
        // Cabinets and fixtures are labelled too.
        let mut q = rich();
        let mut layer = ScheduleLayer::default();
        layer.add(def(ScheduleKind::Cabinet));
        layer.add(def(ScheduleKind::Fixture));
        layer.store(&mut q.floors[0]);
        let texts: Vec<String> = floor_callouts(&q, 0).into_iter().map(|c| c.text).collect();
        assert_eq!(texts, ["C-01", "C-02", "F-01"]);
    }

    #[test]
    fn an_openings_own_schedule_number_replaces_the_mark() {
        let mut p = house();
        let id = p.floors[0]
            .openings
            .iter()
            .find(|o| o.kind == OpeningKind::Door)
            .unwrap()
            .id;
        p.floors[0]
            .openings
            .iter_mut()
            .find(|o| o.id == id)
            .unwrap()
            .schedule_number = Some("A1".into());
        let t = table(&p, &def(ScheduleKind::Door), 0, None);
        assert_eq!(t.rows[0][0], "A1");
    }

    #[test]
    fn style_and_plan_label_columns_name_the_flavor_and_the_size() {
        let mut p = house();
        let id = p.floors[0]
            .openings
            .iter()
            .find(|o| o.kind == OpeningKind::Door)
            .unwrap()
            .id;
        let o = p.floors[0]
            .openings
            .iter_mut()
            .find(|o| o.id == id)
            .unwrap();
        o.style = plan_core::OpeningStyle::Sliding;
        o.width = 72.0;
        o.height = 80.0;
        let mut d = def(ScheduleKind::Door);
        for field in ["style", "label"] {
            let i = d.columns.iter().position(|c| c.field == field).unwrap();
            d.set_column_visible(i, true);
        }
        let t = table(&p, &d, 0, None);
        let (si, li) = (
            t.columns.iter().position(|c| c == "Style").unwrap(),
            t.columns.iter().position(|c| c == "Plan Label").unwrap(),
        );
        let row = t
            .rows
            .iter()
            .find(|r| r[si] == "Sliding Door")
            .expect("the sliding door is listed");
        assert_eq!(row[li], "6068");
        // Hidden by default, so existing schedules keep their columns.
        assert_eq!(
            table(&p, &def(ScheduleKind::Door), 0, None).columns.len(),
            7
        );
        // The window schedule has them too.
        assert!(def(ScheduleKind::Window)
            .columns
            .iter()
            .any(|c| c.field == "style" && !c.visible));
    }

    #[test]
    fn hidden_columns_are_left_out_and_order_follows_the_list() {
        let p = house();
        let mut d = def(ScheduleKind::Window);
        let full = table(&p, &d, 0, None);
        assert_eq!(full.columns.len(), 7);
        let i = d.columns.iter().position(|c| c.field == "sill").unwrap();
        d.set_column_visible(i, false);
        d.move_column(0, false);
        let t = table(&p, &d, 0, None);
        assert_eq!(t.columns.len(), 6);
        assert!(!t.columns.contains(&"Sill".to_string()));
        assert_eq!(t.columns[0], "Width");
        assert_eq!(t.rows[0].len(), 6);
        // A renamed heading shows.
        d.columns[0].title = "Wd".into();
        assert_eq!(table(&p, &d, 0, None).columns[0], "Wd");
        // Even a schedule with all columns hidden gives an empty table.
        for c in &mut d.columns {
            c.visible = false;
        }
        assert!(table(&p, &d, 0, None).columns.is_empty());
    }

    #[test]
    fn csv_has_the_header_and_the_rows() {
        let p = rich();
        let csv = table(&p, &def(ScheduleKind::Cabinet), 0, None).to_csv();
        assert!(
            csv.starts_with("Mark,Label,Type,Width,Depth,Height\n"),
            "{csv}"
        );
        assert!(csv.contains("C-01,B24,Base,"));
        let csv = table(&p, &def(ScheduleKind::Door), 0, None).to_csv();
        assert!(csv.starts_with("Mark,Floor,Width,Height,Type,Wall,Swing\n"));
    }

    #[test]
    fn cabinet_schedule_columns_for_size_door_style_finish_and_hardware() {
        let mut p = rich();
        let mut cabs = p.floors[0].cabinets_as::<Cabinet>().unwrap();
        cabs[0].door_style.name = "Shaker Door".into();
        cabs[0].door_style.handle = plan_cabinets::HandleStyle::Pull;
        cabs[0].drawer_style.handle = plan_cabinets::HandleStyle::Cup;
        cabs[0].materials.door = plan_cabinets::MaterialChoice::Painted;
        let mut vanity = Cabinet::vanity(30.0);
        vanity.id = p.alloc_id();
        vanity.position = Point::new(80.0, 20.0);
        cabs.push(vanity);
        p.floors[0].set_cabinets(&cabs).unwrap();
        let mut d = def(ScheduleKind::Cabinet);
        let ids = [
            "width",
            "height",
            "depth",
            "door_style",
            "drawer_style",
            "finish",
            "hardware",
        ];
        for c in d.columns.iter_mut() {
            c.visible = ids.contains(&c.field.as_str()) || c.field == "type";
        }
        let t = table(&p, &d, 0, None);
        let csv = t.to_csv();
        let header = csv.lines().next().unwrap();
        for h in [
            "Width",
            "Depth",
            "Height",
            "Door Style",
            "Drawer Style",
            "Finish",
            "Hardware",
        ] {
            assert!(header.contains(h), "{header}");
        }
        assert!(csv.contains("Shaker Door"), "{csv}");
        assert!(csv.contains("Painted"), "{csv}");
        assert!(csv.contains("Pull / Cup Pull"), "{csv}");
        assert!(csv.contains("Vanity Cabinet"), "{csv}");
        assert!(csv.contains("Knob"), "{csv}");
    }

    #[test]
    fn sort_filter_and_framing_groups() {
        let p = rich();
        let mut d = def(ScheduleKind::Cabinet);
        d.sort = SortSpec {
            field: "width".into(),
            descending: true,
        };
        let t = table(&p, &d, 0, None);
        assert_eq!(t.rows[0][3], "2'-6\"");
        d.filter = "b24".into();
        assert_eq!(table(&p, &d, 0, None).rows.len(), 1);
        let f = table(&p, &def(ScheduleKind::Framing), 0, None);
        assert_eq!(f.rows.len(), 1);
        assert_eq!(
            f.rows[0][4], "2",
            "two identical joists group into one line"
        );
        assert_eq!(f.rows[0][1], "joist");
    }

    #[test]
    fn natural_order_puts_2_before_10() {
        use std::cmp::Ordering::*;
        assert_eq!(natural_cmp("D2", "D10"), Less);
        assert_eq!(natural_cmp("d02", "D2"), Equal);
        assert_eq!(natural_cmp("b", "a"), Greater);
    }

    #[test]
    fn rooms_use_the_active_floors_detection() {
        let mut p = Project::new("R");
        rect_walls(&mut p, 240.0, 120.0, 4.5, WallKind::Interior);
        let detected = detect_rooms(&p.floors[0].walls, 1.0);
        let t = table(&p, &def(ScheduleKind::Room), 0, Some((0, &detected)));
        assert_eq!(t.rows.len(), 1);
        assert_eq!(t.rows[0][0], "R01");
        // Interior area (QA-03): (240 - 4.5) x (120 - 4.5) / 144.
        assert_eq!(t.rows[0][2], "188.9");
        // Without the editor's rooms the floor is detected here.
        assert_eq!(table(&p, &def(ScheduleKind::Room), 0, None).rows.len(), 1);
        let mut d = def(ScheduleKind::Room);
        d.columns.push(ColumnSpec::new("bogus", "x", true));
        assert_eq!(
            table(&p, &d, 0, None).columns.len(),
            5,
            "unknown fields drop"
        );
    }

    #[test]
    fn sample_plans_give_rows_for_the_kinds_they_contain() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples");
        let mut seen = 0;
        for name in ["ranch-3bed", "studio-adu", "two-story-colonial"] {
            let path = format!("{dir}/{name}.psplan");
            let Ok(json) = std::fs::read_to_string(&path) else {
                continue;
            };
            let p = Project::from_json(&json).unwrap();
            seen += 1;
            let has_doors = p
                .floors
                .iter()
                .any(|f| f.openings.iter().any(|o| o.kind == OpeningKind::Door));
            let has_windows = p
                .floors
                .iter()
                .any(|f| f.openings.iter().any(|o| o.kind == OpeningKind::Window));
            let has_cabinets = p.floors.iter().any(|f| !f.cabinets.is_empty());
            for kind in ScheduleKind::ALL {
                let mut d = def(kind);
                d.floor_scope = FloorScope::All;
                let t = table(&p, &d, 0, None);
                assert!(!t.columns.is_empty(), "{name} {kind:?}");
                let expect = match kind {
                    ScheduleKind::Door => Some(has_doors),
                    ScheduleKind::Window => Some(has_windows),
                    ScheduleKind::Cabinet => Some(has_cabinets),
                    ScheduleKind::Wall => Some(true),
                    _ => None,
                };
                if let Some(true) = expect {
                    assert!(!t.rows.is_empty(), "{name} {kind:?} should have rows");
                    assert!(t.rows.iter().all(|r| r.len() == t.columns.len()));
                }
            }
            // Doors are numbered D01.. with no gaps or repeats on each floor.
            let mut d = def(ScheduleKind::Door);
            d.floor_scope = FloorScope::All;
            let marks: Vec<String> = table(&p, &d, 0, None)
                .rows
                .into_iter()
                .map(|r| r[0].clone())
                .collect();
            let mut by_floor: Vec<Vec<String>> = vec![Vec::new(); p.floors.len()];
            for (e, m) in entries(&p, ScheduleKind::Door, None).iter().zip(&marks) {
                by_floor[e.floor].push(m.clone());
            }
            for list in by_floor {
                for (i, m) in list.iter().enumerate() {
                    assert_eq!(m, &format!("D{:02}", i + 1));
                }
            }
        }
        assert!(seen > 0, "no sample plans found");
    }

    #[test]
    fn bad_floor_index_is_empty() {
        let p = house();
        assert!(table(&p, &def(ScheduleKind::Door), 7, None).rows.is_empty());
        assert!(floor_callouts(&p, 7).is_empty());
    }

    #[test]
    fn room_finish_rows_read_the_room_specification() {
        let mut p = house();
        let rooms = detect_rooms(&p.floors[0].walls, 1.0);
        assert!(!rooms.is_empty());
        let anchor = rooms[0].centroid;
        let mut name = plan_core::RoomName::new(anchor, "Kitchen", "Kitchen");
        name.floor_finish = Some("Oak".into());
        name.ceiling_finish = Some("Paint".into());
        name.moldings = vec![
            plan_core::MoldingRef {
                kind: MoldingKind::Base,
                profile: "Colonial".into(),
                height: 5.25,
            },
            plan_core::MoldingRef {
                kind: MoldingKind::Crown,
                profile: "Cove".into(),
                height: 3.5,
            },
        ];
        name.misc = Some(plan_core::extras::RoomMisc {
            wall_covering: "Wainscot".into(),
            ..plan_core::extras::RoomMisc::default()
        });
        p.floors[0].room_names.push(name);
        let t = table(&p, &def(ScheduleKind::RoomFinish), 0, None);
        assert_eq!(
            t.columns,
            vec![
                "Number",
                "Name",
                "Floor Finish",
                "Base",
                "Wall Finish",
                "Ceiling Finish"
            ]
        );
        let row = t
            .rows
            .iter()
            .find(|r| r[1] == "Kitchen")
            .expect("Kitchen row");
        assert_eq!(row[0], "RF01");
        assert_eq!(&row[2..], ["Oak", "Colonial", "Wainscot", "Paint"]);
        // Rows come from the rooms, so the Room schedule still has its own kind.
        assert!(entries(&p, ScheduleKind::Room, None)
            .iter()
            .all(|e| e.kind == ScheduleKind::Room));
    }

    #[test]
    fn note_rows_list_the_numbered_notes_by_type_and_number() {
        let p = rich();
        let t = table(&p, &def(ScheduleKind::Note), 0, None);
        assert_eq!(t.columns, vec!["No.", "Type", "Note"]);
        assert_eq!(
            t.rows,
            vec![
                vec!["Note 2", "General Note", "Verify at site"],
                vec!["E 1", "Electrical Note", "Outlet at 16\" AFF"],
            ]
        );
        let targets = row_targets(&p, &def(ScheduleKind::Note), 0, None);
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0][0].kind, ScheduleKind::Note);
        assert_ne!(targets[0][0].id, 0);
    }

    #[test]
    fn terrain_plants_are_plant_schedule_rows() {
        let mut p = house();
        p.terrain = Some(serde_json::json!({
            "terrain": {
                "landscape": [
                    {
                        "kind": "Plants",
                        "points": [{"x": 0.0, "y": 0.0}, {"x": 120.0, "y": 0.0}],
                        "plant": "plants.boxwood",
                        "size": 30.0,
                        "height": 36.0,
                        "spacing": 30.0
                    },
                    { "kind": "GardenBed", "points": [] }
                ]
            }
        }));
        let es = entries(&p, ScheduleKind::Plant, None);
        // 120" at 30" spacing: five plants, both ends included.
        assert_eq!(es.len(), 5);
        assert!(es.iter().all(|e| e.id == 0 && e.cell("height") == "3'-0\""));
        let mut d = def(ScheduleKind::Plant);
        d.group_by = "name".into();
        d.totals = true;
        let t = table(&p, &d, 0, None);
        // One group line and the totals line.
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[1][0], "Total");
        assert_eq!(t.rows[1][1], "5");
    }

    #[test]
    fn grouping_counts_equal_rows_and_totals_add_up() {
        let p = rich();
        let mut d = def(ScheduleKind::Cabinet);
        let plain = table(&p, &d, 0, None);
        assert_eq!(plain.rows.len(), 2);
        // Group the two base cabinets by their type.
        d.group_by = "type".into();
        let grouped = table(&p, &d, 0, None);
        assert_eq!(grouped.rows.len(), 1);
        assert_eq!(grouped.rows[0][0], "C-01-C-02 (2)");
        // Different widths show as "*".
        let width = grouped.columns.iter().position(|c| c == "Width").unwrap();
        assert_eq!(grouped.rows[0][width], "*");
        let targets = row_targets(&p, &d, 0, None);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].len(), 2, "both cabinets are behind the line");
        // Totals add a last line with the count of objects, none behind it.
        d.totals = true;
        let t = table(&p, &d, 0, None);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[1][0], "Total");
        assert_eq!(t.rows[1][1], "2");
        assert!(row_targets(&p, &d, 0, None)[1].is_empty());
        // Rooms: the area column is summed.
        let mut r = def(ScheduleKind::Room);
        r.totals = true;
        let t = table(&p, &r, 0, None);
        let area = t.columns.iter().position(|c| c == "Area sq ft").unwrap();
        let sum: f64 = t.rows[..t.rows.len() - 1]
            .iter()
            .filter_map(|row| row[area].parse::<f64>().ok())
            .sum();
        let shown: f64 = t.rows.last().unwrap()[area].parse().unwrap();
        assert!(
            (sum - shown).abs() < 0.11 * t.rows.len() as f64,
            "{sum} vs {shown}"
        );
        // A group field the kind does not have is ignored.
        let mut bad = def(ScheduleKind::Cabinet);
        bad.group_by = "nope".into();
        assert_eq!(table(&p, &bad, 0, None).rows.len(), 2);
    }

    #[test]
    fn the_stair_schedule_lists_stairs_but_not_landings() {
        use plan_stairs::{Stair, StairParams, StairShape};
        let mut p = rich();
        let landing = Stair::new(
            p.alloc_id(),
            Point::new(200.0, 40.0),
            0.0,
            StairParams {
                shape: StairShape::Landing { depth: 36.0 },
                ..StairParams::default()
            },
        );
        let mut stairs = p.floors[0].stairs_as::<Stair>().unwrap();
        stairs.push(landing);
        p.floors[0].set_stairs(&stairs).unwrap();
        let t = table(&p, &def(ScheduleKind::Stair), 0, None);
        assert_eq!(
            t.columns,
            vec![
                "Mark",
                "Type",
                "Width",
                "Total rise",
                "Risers",
                "Riser height",
                "Tread depth",
                "Total run"
            ]
        );
        assert_eq!(t.rows.len(), 1, "{:?}", t.rows);
        let row = &t.rows[0];
        assert_eq!(row[0], "S01");
        assert_eq!(row[1], "Straight");
        let params = StairParams::default();
        let sol = plan_stairs::solve(&params);
        assert_eq!(row[4], sol.risers.to_string());
        assert_eq!(row[5], format!("{:.3}\"", sol.riser_height));
        let targets = row_targets(&p, &def(ScheduleKind::Stair), 0, None);
        assert_eq!(targets[0][0].kind, ScheduleKind::Stair);
        assert_ne!(targets[0][0].id, 0);
    }
}
