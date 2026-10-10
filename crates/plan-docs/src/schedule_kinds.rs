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
use plan_core::props::{PropDef, PropKey, PropKind};
use plan_core::schedules::{
    format_value, NumFormat, NumKind, NumRec, Numbering, Schedule, ScheduleKind, ScheduleLayer,
    SortSpec,
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
    /// `(column id, text)` of the custom properties of the object
    /// (`prop:Fire Rating`), for the kinds that take them.
    pub props: Vec<(String, String)>,
    /// The system schedule category of the object (`Door/Hinged Door`,
    /// `Wall/Siding-6`); the Categories to Include tree ticks these.
    pub category: String,
    /// The numbers behind the numeric cells, in the raw unit of the field
    /// (inches, square feet, a count); columns with a Number Format are set
    /// from these rather than from the cell text.
    pub nums: Vec<(&'static str, f64)>,
    /// The number the schedule gave the object (counted from 1 on its
    /// floor or in the plan; the mark shows it with prefix and style).
    pub n: u32,
}

impl Entry {
    /// The text of field `id` (empty when the entry has no such field). A
    /// `prop:` id reads the object's custom property.
    pub fn cell(&self, id: &str) -> &str {
        if id.starts_with(plan_core::props::COLUMN_PREFIX) {
            return self
                .props
                .iter()
                .find(|(k, _)| k == id)
                .map_or("", |(_, v)| v.as_str());
        }
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

    /// Records the number behind a numeric cell.
    fn num(&mut self, id: &'static str, v: f64) {
        match self.nums.iter_mut().find(|(k, _)| *k == id) {
            Some((_, x)) => *x = v,
            None => self.nums.push((id, v)),
        }
    }

    /// The number field `id` holds, in the raw unit of `kind`: the recorded
    /// one, else the cell text read back.
    pub fn value(&self, id: &str, kind: NumKind) -> Option<f64> {
        if let Some((_, v)) = self.nums.iter().find(|(k, _)| *k == id) {
            return Some(*v);
        }
        let text = self.cell(id).trim();
        if text.is_empty() {
            return None;
        }
        match kind {
            NumKind::Length => plan_core::units::parse_ft_in(text),
            _ => text
                .trim_end_matches(|c: char| !c.is_ascii_digit())
                .parse::<f64>()
                .ok(),
        }
    }

    /// A General-schedule row stands for an object of another kind: its
    /// cells are those of that kind, so the fields a General or foreign
    /// schedule asks for fall back to the object's name and size.
    fn fallback(&self, id: &str) -> Option<String> {
        match id {
            "label" | "name" => Some(self.name.clone()),
            "size" => Some(self.size.clone()),
            "category" => Some(self.category.clone()),
            _ => None,
        }
    }

    /// The text of `id`, with the fallbacks of [`Entry::fallback`] for a
    /// field the object's own kind lacks.
    pub fn text(&self, id: &str) -> String {
        if self.cells.iter().any(|(k, _)| *k == id) || id.starts_with("prop:") {
            return self.cell(id).to_string();
        }
        self.fallback(id).unwrap_or_default()
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
        props: Vec::new(),
        category: String::new(),
        nums: Vec::new(),
        n: 0,
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
                    ("manufacturer", o.extras.spec.schedule.manufacturer.clone()),
                    ("model", o.extras.spec.schedule.model.clone()),
                    ("supplier", o.extras.spec.schedule.supplier.clone()),
                    ("comment", o.extras.spec.schedule.comment.clone()),
                    ("rough", size_text(o.rough_width(), o.rough_height())),
                    ("u_factor", format!("{:.2}", o.extras.spec.energy.u_factor)),
                    ("shgc", format!("{:.2}", o.extras.spec.energy.shgc)),
                    ("description", o.extras.spec.info.description.clone()),
                    ("object_id", o.extras.spec.info.id.clone()),
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
                    ("manufacturer", o.extras.spec.schedule.manufacturer.clone()),
                    ("model", o.extras.spec.schedule.model.clone()),
                    ("supplier", o.extras.spec.schedule.supplier.clone()),
                    ("comment", o.extras.spec.schedule.comment.clone()),
                    ("rough", size_text(o.rough_width(), o.rough_height())),
                    ("u_factor", format!("{:.2}", o.extras.spec.energy.u_factor)),
                    ("shgc", format!("{:.2}", o.extras.spec.energy.shgc)),
                    ("description", o.extras.spec.info.description.clone()),
                    ("object_id", o.extras.spec.info.id.clone()),
                ]
            };
            e.category = format!("{}/{}", group_of(kind), o.type_name());
            // The Area column (width times height, square feet) and the
            // Quantity of the row.
            let area = o.width * o.height / 144.0;
            e.set("area", format!("{area:.1}"));
            e.set("quantity", "1".to_string());
            e.num("width", o.width);
            e.num("height", o.height);
            e.num("area", area);
            e.num("quantity", 1.0);
            if kind == ScheduleKind::Window {
                e.num("sill", o.sill_height);
                e.num("head", o.sill_height + o.height);
            }
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
                ("wall_type", w.wall_type.clone().unwrap_or_default()),
                (
                    "interior_covering",
                    covering_text(&w.spec.covering.interior),
                ),
                (
                    "exterior_covering",
                    covering_text(&w.spec.covering.exterior),
                ),
                ("code", w.spec.info.id.clone()),
                ("description", w.spec.info.description.clone()),
                ("manufacturer", w.spec.schedule.manufacturer.clone()),
                ("model", w.spec.schedule.model.clone()),
                ("supplier", w.spec.schedule.supplier.clone()),
                ("comment", w.spec.schedule.comment.clone()),
            ];
            // The wall's category is its wall type; a wall with none is
            // listed by its kind.
            e.category = format!(
                "Wall/{}",
                w.wall_type
                    .clone()
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| kind_label(w.kind).to_string())
            );
            // Wall legend columns (L-235). Total Width is the thickness of
            // the whole assembly, the construction columns list the layers of
            // the upper and the lower wall (they differ for a pony wall).
            let (upper, lower) = wall_assemblies(project, w);
            let total = [&upper, &lower]
                .iter()
                .filter_map(|t| t.as_ref().map(|d| d.thickness()))
                .fold(0.0_f64, f64::max);
            let total = if total > 0.0 { total } else { w.thickness };
            e.set("total_width", fmt_ft_in(total));
            e.set("construction_upper", construction_text(upper.as_ref()));
            e.set("construction_lower", construction_text(lower.as_ref()));
            e.set("quantity", "1".to_string());
            e.num("length", w.length());
            e.num("thickness", w.thickness);
            e.num("height", w.height);
            e.num("area", w.length() * w.height / 144.0);
            e.num("total_width", total);
            e.num("openings", f.openings_on(w.id).count() as f64);
            e.num("quantity", 1.0);
            out.push(e);
        }
    }
    out
}

/// The wall types of the upper and the lower part of a wall: both its own
/// type, except for a pony wall (upper type over lower type) and a glass pony
/// wall (the lower type under glass).
fn wall_assemblies(
    project: &Project,
    w: &plan_core::Wall,
) -> (
    Option<plan_core::defaults::WallTypeDef>,
    Option<plan_core::defaults::WallTypeDef>,
) {
    use plan_core::walls::WallClass;
    let def = |name: &str| project.wall_type_def(name).cloned();
    let own = w.wall_type.as_deref().filter(|n| !n.is_empty());
    match &w.class {
        WallClass::Pony {
            upper_type,
            lower_type,
            ..
        } => (def(upper_type), def(lower_type)),
        WallClass::GlassPony { lower_type, .. } => (None, def(lower_type)),
        _ => {
            let d = own.and_then(def);
            (d.clone(), d)
        }
    }
}

/// The layers of a wall type as a schedule cell: `Siding 1", Framing 3 1/2"`,
/// exterior face first. Empty when the wall has no such part.
fn construction_text(t: Option<&plan_core::defaults::WallTypeDef>) -> String {
    let Some(t) = t else {
        return String::new();
    };
    t.layers
        .iter()
        .map(|l| {
            let name = if l.material.is_empty() {
                &l.name
            } else {
                &l.material
            };
            let inches = NumFormat {
                units: plan_core::schedules::NumUnit::Inches,
                ..NumFormat::default()
            };
            format!(
                "{} {}",
                name,
                format_value(l.thickness, NumKind::Length, &inches)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The first part of a category id: the group the Categories to Include
/// tree lists a kind's categories under.
pub fn group_of(kind: ScheduleKind) -> &'static str {
    match kind {
        ScheduleKind::RoomFinish => "Room",
        k => k.name(),
    }
}

/// The coverings on one face of a wall as a schedule cell: the materials
/// and molding profile names, covering first.
fn covering_text(side: &plan_core::walls::SideCovering) -> String {
    [
        &side.covering,
        &side.wainscot,
        &side.chair_rail,
        &side.base,
        &side.crown,
    ]
    .into_iter()
    .filter(|n| !n.is_empty())
    .cloned()
    .collect::<Vec<_>>()
    .join(", ")
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
            // The room type is its category; an unnamed room has the type
            // "Room".
            let room_type = spec
                .map(|n| n.room_type.clone())
                .filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| "Room".to_string());
            e.category = format!("Room/{room_type}");
            let area = room_area_sq_ft(r);
            let volume = area * ceiling / 12.0;
            e.set("volume", format!("{volume:.1}"));
            e.num("area", area);
            e.num("standard_area", r.area_sq_ft());
            e.num("perimeter", perimeter(&r.polygon) / 12.0);
            e.num("ceiling_height", ceiling);
            e.num("volume", volume);
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
        e.set("quantity", "1".to_string());
        e.num("quantity", 1.0);
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
                StairShape::Curved { .. } if p.spiral => "Spiral",
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
                (
                    "treads",
                    if ramp {
                        String::new()
                    } else {
                        sol.treads.to_string()
                    },
                ),
                ("width", fmt_ft_in(p.width)),
                ("headroom", fmt_ft_in(p.headroom_min)),
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
            e.category = format!("Stair/{kind}");
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
            e.category = format!("Note/{type_name}");
            let type_order = types
                .types
                .iter()
                .position(|t| t.name == type_name)
                .unwrap_or(0);
            keyed.push(((type_order, n, fi), e));
        }
    }
    // The Note objects (callouts tied to the schedule), numbered per type
    // in draw order (`Project::note_rows`).
    for r in project.note_rows() {
        let mut e = new_entry(r.floor, r.id, r.pos, ScheduleKind::Note);
        e.mark_override = Some(r.mark.clone());
        e.name = r.text.clone();
        e.size = String::new();
        e.cells = vec![
            ("mark", String::new()),
            ("type", r.note_type.clone()),
            ("note", r.text.clone()),
            ("floor", project.floors[r.floor].name.clone()),
        ];
        e.category = format!("Note/{}", r.note_type);
        let type_order = types
            .types
            .iter()
            .position(|t| t.name == r.note_type)
            .unwrap_or(0);
        keyed.push(((type_order, r.number, r.floor), e));
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
            // Schedule tab of the Cabinet Specification: off keeps it out,
            // and the fillers the program makes are never listed (p. 655).
            if !c.in_schedule || c.auto_filler {
                continue;
            }
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
            let category = plan_cabinets::schedule_category(&c)
                .map_or("Other", |k| k.name())
                .to_string();
            e.category = format!("Cabinet/{category}");
            e.set("category", category);
            e.set("quantity", "1".to_string());
            e.num("width", c.width);
            e.num("depth", c.depth);
            e.num("height", c.height);
            e.num("elevation", c.elevation);
            e.num("quantity", 1.0);
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
                // Hidden columns: the voltage (outlets) and the flags the
                // device schedule shows after the type ("110V, GFCI, WP").
                (
                    "voltage",
                    d.kind
                        .voltage()
                        .map(|v| format!("{v}V"))
                        .unwrap_or_default(),
                ),
                ("flags", d.kind.flags().join(", ")),
                (
                    "wall",
                    d.wall_id
                        .map(|w| wall_number_of(&numbers, w))
                        .unwrap_or_default(),
                ),
                ("floor", f.name.clone()),
            ];
            e.category = format!("Electrical/{}", d.kind.name());
            e.num("count", 1.0);
            e.num("height", d.height);
            out.push(e);
        }
        // Rope lights set to Treat as One Object are one line each (E-22).
        for r in layer.ropes.iter().filter(|r| r.spec.treat_as_object) {
            let at = r.points.first().copied().unwrap_or(Point::ZERO);
            let mut e = new_entry(fi, ROPE_ENTRY_BASE + r.id, at, ScheduleKind::Electrical);
            let height = r.top_elevation(plan_core::DEFAULT_CEILING_HEIGHT);
            e.name = "Rope Light".to_string();
            e.size = fmt_ft_in(r.length());
            e.cells = vec![
                ("mark", String::new()),
                ("type", "Rope Light".to_string()),
                ("count", "1".to_string()),
                ("label", r.label.clone()),
                ("height", fmt_ft_in(height)),
                ("circuit", String::new()),
                ("voltage", String::new()),
                ("flags", String::new()),
                ("wall", String::new()),
                ("floor", f.name.clone()),
            ];
            e.category = "Electrical/Rope Light".to_string();
            e.num("count", 1.0);
            e.num("height", height);
            out.push(e);
        }
    }
    by_position(&mut out);
    out
}

/// Schedule entry ids of rope lights start here, so they never meet a device's.
const ROPE_ENTRY_BASE: Id = 1 << 40;

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
            e.category = format!("Framing/{}", e.cell("type"));
            e.num("length", length);
            e.num("qty", f64::from(qty));
            e.num("linear", length * f64::from(qty) / 12.0);
            e.num("board_feet", board);
            out.push(e);
        }
    }
    out
}

/// The category of a placed symbol. Fixtures split into Plumbing, Appliances,
/// HVAC and Lighting (Multiple Fixture Schedules can serve as a plumbing,
/// appliance or HVAC schedule, p. 718); furniture and plants use their library
/// sub-category, and every plant is one category.
fn symbol_category(kind: ScheduleKind, name: &str, path: Option<&[String]>) -> String {
    let group = group_of(kind);
    let top = path.and_then(|p| p.first()).map_or("", String::as_str);
    let sub = path.and_then(|p| p.get(1)).map_or("", String::as_str);
    let n = name.to_lowercase();
    let item = match kind {
        ScheduleKind::Plant => "Plants".to_string(),
        ScheduleKind::Fixture => {
            let has = |words: &[&str]| {
                words
                    .iter()
                    .any(|w| n.contains(w) || sub.to_lowercase().contains(w))
            };
            if top == "Lighting" {
                "Lighting".to_string()
            } else if has(&[
                "refrigerator",
                "fridge",
                "range",
                "oven",
                "cooktop",
                "dishwasher",
                "microwave",
                "washer",
                "dryer",
                "vent hood",
                "icemaker",
                "trash compactor",
            ]) {
                "Appliances".to_string()
            } else if has(&[
                "furnace",
                "water heater",
                "boiler",
                "condenser",
                "hvac",
                "air handler",
                "heat pump",
            ]) {
                "HVAC".to_string()
            } else {
                "Plumbing".to_string()
            }
        }
        _ if !sub.is_empty() => sub.to_string(),
        _ if !top.is_empty() => top.to_string(),
        _ => group.to_string(),
    };
    format!("{group}/{item}")
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
            e.category = symbol_category(k, &e.name, item.map(|i| i.category.as_slice()));
            e.set("quantity", "1".to_string());
            e.num("width", s.width);
            e.num("depth", s.depth);
            e.num("height", s.height);
            e.num("elevation", s.elevation);
            e.num("quantity", 1.0);
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
            e.category = "Plant/Plants".to_string();
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
    out.extend(terrain_objects(project));
    out.sort_by_key(|e| (e.floor, reading_key(e.position), e.id));
    out
}

/// The terrain and road objects of the site (Terrain Perimeter, Driveways,
/// Medians, Roads, Road Markings, Terrain Paths and Terrain Features, each
/// under the category its Schedule panel names), one row per object. They
/// belong to the site, so they sit on the first floor's list.
fn terrain_objects(project: &Project) -> Vec<Entry> {
    let Some(t) = crate::terrain_report::terrain_of(project) else {
        return Vec::new();
    };
    let floor_name = project
        .floors
        .first()
        .map(|f| f.name.clone())
        .unwrap_or_default();
    plan_terrain::terrain_schedule(&t)
        .into_iter()
        .map(|row| {
            let at = plan_terrain::anchor_of(&t, row.key).unwrap_or(Point::ZERO);
            let size = if row.area > 0.0 {
                format!("{:.0} sq ft", row.area / 144.0)
            } else {
                fmt_ft_in(row.length)
            };
            let mut e = new_entry(0, 0, at, ScheduleKind::General);
            e.name = row.name.clone();
            e.size = size.clone();
            e.cells = vec![
                ("mark", String::new()),
                ("category", row.category.name().to_string()),
                ("name", row.name),
                ("size", size),
                ("floor", floor_name.clone()),
            ];
            e.category = format!("General/{}", row.category.name());
            e
        })
        .collect()
}

/// The objects of `kind` on every floor, floor by floor, in reading order
/// (rooms in detection order, framing grouped). Marks are empty; see
/// [`number`]. `active` lets the Room schedule use the editor's own room
/// detection for that floor.
pub fn entries(project: &Project, kind: ScheduleKind, active: ActiveRooms) -> Vec<Entry> {
    let mut out = entries_raw(project, kind, active);
    // Every field of the kind has a cell (the object preview columns are
    // pictures the plan draws, so their text is empty).
    for e in &mut out {
        for fd in e.kind.fields() {
            if e.kind == kind && !e.cells.iter().any(|(k, _)| *k == fd.id) {
                e.cells.push((fd.id, String::new()));
            }
        }
    }
    attach_props(project, kind, &mut out);
    apply_object_pages(project, &mut out);
    out
}

/// The shared panels' effect on the schedule rows: an object whose Schedule
/// panel has Include in Schedule cleared is left out, and what Object
/// Information says (Code, Comment, Description, Manufacturer, Supplier)
/// replaces the cell of the same column.
fn apply_object_pages(project: &Project, entries: &mut Vec<Entry>) {
    if project.props.pages.is_empty() && project.materials.objects.is_empty() {
        return;
    }
    let key_of = |e: &Entry| prop_key(e.kind, e.floor, e.id, e.position);
    entries.retain(|e| match key_of(e) {
        Some(k) => !project.props.schedule_excluded(k.as_str()),
        None => true,
    });
    for e in entries.iter_mut() {
        let Some(k) = key_of(e) else { continue };
        let Some(info) = project.materials.info(k.as_str()) else {
            continue;
        };
        for (id, text) in [
            ("code", &info.code),
            ("comment", &info.comment),
            ("description", &info.description),
            ("manufacturer", &info.manufacturer),
            ("supplier", &info.supplier),
        ] {
            if !text.is_empty() && e.cells.iter().any(|(c, _)| *c == id) {
                e.set(id, text.clone());
            }
        }
    }
}

/// The kind of custom property a schedule of `kind` lists (none for the
/// Note and General schedules).
pub fn prop_kind_of(kind: ScheduleKind) -> Option<PropKind> {
    match kind {
        ScheduleKind::Door => Some(PropKind::Door),
        ScheduleKind::Window => Some(PropKind::Window),
        ScheduleKind::Room | ScheduleKind::RoomFinish => Some(PropKind::Room),
        ScheduleKind::Wall => Some(PropKind::Wall),
        ScheduleKind::Cabinet => Some(PropKind::Cabinet),
        ScheduleKind::Electrical => Some(PropKind::Electrical),
        ScheduleKind::Framing => Some(PropKind::Framing),
        ScheduleKind::Fixture | ScheduleKind::Furniture | ScheduleKind::Plant => {
            Some(PropKind::Symbol)
        }
        ScheduleKind::Stair => Some(PropKind::Stair),
        ScheduleKind::Note | ScheduleKind::General => None,
    }
}

/// Who owns the custom property values of a scheduled object: `None` for the
/// lines that stand for no single object (grouped framing, terrain plants).
pub fn prop_key(kind: ScheduleKind, floor: usize, id: Id, position: Point) -> Option<PropKey> {
    match kind {
        ScheduleKind::Room | ScheduleKind::RoomFinish => Some(PropKey::room(floor, position)),
        _ if id == 0 => None,
        ScheduleKind::Door => Some(PropKey::door(id)),
        ScheduleKind::Window => Some(PropKey::window(id)),
        ScheduleKind::Wall => Some(PropKey::wall(id)),
        ScheduleKind::Cabinet => Some(PropKey::cabinet(id)),
        ScheduleKind::Electrical => Some(PropKey::device(floor, id)),
        ScheduleKind::Fixture | ScheduleKind::Furniture | ScheduleKind::Plant => {
            Some(PropKey::symbol(id))
        }
        ScheduleKind::Stair => Some(PropKey::stair(id)),
        ScheduleKind::Framing => Some(PropKey::framing(id)),
        ScheduleKind::Note | ScheduleKind::General => None,
    }
}

/// Fills in each entry's custom property texts (stored value, else default).
fn attach_props(project: &Project, kind: ScheduleKind, entries: &mut [Entry]) {
    let Some(pk) = prop_kind_of(kind) else {
        return;
    };
    let defs: Vec<&PropDef> = project.props.defs_for(pk).collect();
    if defs.is_empty() {
        return;
    }
    for e in entries {
        let key = prop_key(e.kind, e.floor, e.id, e.position);
        e.props = defs
            .iter()
            .map(|d| {
                let text = key
                    .as_ref()
                    .map(|k| project.props.text(k, d))
                    .unwrap_or_default();
                (d.column_id(), text)
            })
            .collect();
    }
}

fn entries_raw(project: &Project, kind: ScheduleKind, active: ActiveRooms) -> Vec<Entry> {
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
// Categories to Include
// ===================================================================

/// One category of the Categories to Include tree.
#[derive(Debug, Clone, PartialEq)]
pub struct CategoryNode {
    /// `Wall/Siding-6`: the key in `Schedule::categories`.
    pub id: String,
    pub title: String,
    /// A custom schedule category (`Custom/<name>`).
    pub custom: bool,
}

/// A heading of the tree (Wall, Room, Fixture, Electrical, ...) and the
/// categories under it.
#[derive(Debug, Clone, PartialEq)]
pub struct CategoryGroup {
    pub id: String,
    pub title: String,
    pub items: Vec<CategoryNode>,
}

fn node(group: &str, item: &str) -> CategoryNode {
    CategoryNode {
        id: format!("{group}/{item}"),
        title: item.to_string(),
        custom: false,
    }
}

/// The categories a kind lists that the plan declares whether or not any
/// object uses them yet.
fn declared_categories(project: &Project, kind: ScheduleKind) -> Vec<String> {
    use plan_core::OpeningStyle;
    match kind {
        ScheduleKind::Door => OpeningStyle::DOORS
            .iter()
            .map(|s| s.name(OpeningKind::Door).to_string())
            .collect(),
        ScheduleKind::Window => OpeningStyle::WINDOWS
            .iter()
            .map(|s| s.name(OpeningKind::Window).to_string())
            .collect(),
        ScheduleKind::Wall => project.wall_types.iter().map(|t| t.name.clone()).collect(),
        ScheduleKind::Cabinet => plan_cabinets::ScheduleCategory::ALL
            .iter()
            .map(|c| c.name().to_string())
            .chain(["Other".to_string()])
            .collect(),
        ScheduleKind::Electrical => plan_electrical::DeviceKind::all()
            .iter()
            .map(|k| k.name().to_string())
            .collect(),
        ScheduleKind::Fixture => ["Plumbing", "Appliances", "HVAC", "Lighting"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        ScheduleKind::Plant => vec!["Plants".to_string()],
        ScheduleKind::Stair => [
            "Straight", "L-Shaped", "U-Shaped", "Winder", "Ramp", "Spiral", "Curved",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
        ScheduleKind::Note => project
            .note_types()
            .types
            .iter()
            .map(|t| t.name.clone())
            .collect(),
        _ => Vec::new(),
    }
}

/// The Categories to Include tree of a schedule of `kind`: a heading per kind
/// of object (a General schedule has all of them) with the categories the
/// plan declares and those its objects use, then the custom categories.
pub fn category_tree(project: &Project, kind: ScheduleKind) -> Vec<CategoryGroup> {
    let kinds: Vec<ScheduleKind> = if kind == ScheduleKind::General {
        GENERAL_KINDS.to_vec()
    } else {
        vec![kind]
    };
    let mut groups: Vec<CategoryGroup> = Vec::new();
    for k in kinds {
        let g = group_of(k);
        let mut names = declared_categories(project, k);
        let prefix = format!("{g}/");
        let mut present: Vec<String> = entries_raw(project, k, None)
            .into_iter()
            .filter_map(|e| e.category.strip_prefix(&prefix).map(str::to_string))
            .collect();
        present.sort();
        present.dedup();
        for p in present {
            if !names.contains(&p) {
                names.push(p);
            }
        }
        groups.push(CategoryGroup {
            id: g.to_string(),
            title: g.to_string(),
            items: names.iter().map(|n| node(g, n)).collect(),
        });
    }
    if kind == ScheduleKind::General {
        let mut site: Vec<String> = terrain_objects(project)
            .into_iter()
            .filter_map(|e| e.category.strip_prefix("General/").map(str::to_string))
            .collect();
        site.sort();
        site.dedup();
        if !site.is_empty() {
            groups.push(CategoryGroup {
                id: "General".to_string(),
                title: "Site".to_string(),
                items: site.iter().map(|n| node("General", n)).collect(),
            });
        }
    }
    if !project.schedule_setup.categories.is_empty() {
        groups.push(CategoryGroup {
            id: "Custom".to_string(),
            title: "Custom Categories".to_string(),
            items: project
                .schedule_setup
                .categories
                .iter()
                .map(|c| CategoryNode {
                    id: plan_core::schedules::custom_category_id(&c.name),
                    title: c.name.clone(),
                    custom: true,
                })
                .collect(),
        });
    }
    groups
}

/// The ids of every category in the tree of `kind`.
pub fn category_ids(project: &Project, kind: ScheduleKind) -> Vec<String> {
    category_tree(project, kind)
        .into_iter()
        .flat_map(|g| g.items.into_iter().map(|n| n.id))
        .collect()
}

/// Do new types of this heading join a schedule that exists (Wall, Room and
/// Note types; p. 720)?
fn grows(id: &str) -> bool {
    matches!(id.split('/').next(), Some("Wall" | "Room" | "Note"))
}

/// Is the system category `id` ticked in `def`? A schedule that never
/// recorded a choice lists everything; a type made later is listed when the
/// schedule takes new types (Room and Wall Schedules do, Note Schedules do
/// not).
pub fn category_ticked(def: &Schedule, id: &str) -> bool {
    if id.is_empty() {
        return true;
    }
    let default = def.categories.is_empty() || !grows(id) || def.new_types_included;
    def.category_on(id, default)
}

/// The key a custom category member is stored under for `e`.
fn entry_key(e: &Entry) -> Option<String> {
    prop_key(e.kind, e.floor, e.id, e.position).map(|k| k.0)
}

/// Is `e` in a custom category that `def` ticks?
fn in_ticked_custom(project: &Project, def: &Schedule, e: &Entry) -> bool {
    if !def
        .categories
        .iter()
        .any(|(k, on)| *on && k.starts_with("Custom/"))
    {
        return false;
    }
    let Some(key) = entry_key(e) else {
        return false;
    };
    // The categories an object names on its Schedule panel (Include in
    // Schedule As), and those the Manage dialog put it in.
    let on_page: &[String] = project
        .props
        .pages_of(&key)
        .and_then(|p| p.schedule.as_ref())
        .map_or(&[], |s| s.categories.as_slice());
    project
        .schedule_setup
        .categories_of(&key)
        .into_iter()
        .chain(on_page.iter().map(String::as_str))
        .any(|c| def.category_on(&plan_core::schedules::custom_category_id(c), false))
}

/// The custom categories object `key` is in: those its Schedule panel names
/// and those the Manage dialog put it in.
pub fn categories_of_object(project: &Project, key: &str) -> Vec<String> {
    let mut out: Vec<String> = project
        .schedule_setup
        .categories_of(key)
        .into_iter()
        .map(str::to_string)
        .collect();
    if let Some(page) = project
        .props
        .pages_of(key)
        .and_then(|p| p.schedule.as_ref())
    {
        for c in &page.categories {
            if !out.contains(c) {
                out.push(c.clone());
            }
        }
    }
    out
}

/// The polygons of the rooms "Include Objects from Room" picked.
fn scope_polygons(
    project: &Project,
    def: &Schedule,
    active: ActiveRooms,
) -> Vec<(usize, Vec<Point>)> {
    def.rooms
        .iter()
        .filter_map(|r| {
            let f = project.floors.get(r.floor)?;
            let detected;
            let list: &[Room] = match active {
                Some((af, rs)) if af == r.floor => rs,
                _ => {
                    detected = detect_rooms(&f.walls, 1.0);
                    &detected
                }
            };
            list.iter()
                .find(|room| plan_core::geometry::point_in_polygon(r.point(), &room.polygon))
                .map(|room| (r.floor, room.polygon.clone()))
        })
        .collect()
}

/// Is `p` inside `poly` or on its edge (a door sits on the wall centerline)?
fn inside_or_on(poly: &[Point], p: Point) -> bool {
    if plan_core::geometry::point_in_polygon(p, poly) {
        return true;
    }
    let n = poly.len();
    (0..n).any(|i| {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let ab = b - a;
        let len2 = ab.x * ab.x + ab.y * ab.y;
        if len2 < 1e-12 {
            return a.dist(p) < 0.5;
        }
        let t = (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2).clamp(0.0, 1.0);
        Point::new(a.x + ab.x * t, a.y + ab.y * t).dist(p) < 0.5
    })
}

/// The rooms of every floor a schedule can be limited to (Include Objects
/// from Room): `(floor, room name, a point inside it)`.
pub fn room_choices(project: &Project, active: ActiveRooms) -> Vec<(usize, String, Point)> {
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
            let name = r
                .name_entry(&f.room_names)
                .map_or_else(|| r.label.clone(), |n| n.name.clone());
            out.push((fi, name, r.centroid));
        }
    }
    out
}

// ===================================================================
// Marks
// ===================================================================

/// The columns whose values tell two objects apart for Group Similar Objects:
/// the object preview columns, the Quantity and the number columns that sum
/// similar rows do not count.
fn identity_key(e: &Entry, columns: &[plan_core::schedules::ColumnSpec], def: &Schedule) -> String {
    let mut key = String::new();
    if let Some(m) = &e.mark_override {
        key.push_str(m);
    }
    key.push('\u{1}');
    for c in columns {
        if c.field == "mark"
            || c.field == "quantity"
            || plan_core::schedules::is_preview_field(&c.field)
            || (c.sum_similar && def.kind.num_kind(&c.field).is_some())
        {
            continue;
        }
        key.push_str(&e.text(&c.field));
        key.push('\u{1}');
    }
    if def.numbering == Numbering::ByFloor {
        key.push_str(&e.floor.to_string());
    }
    key
}

/// The numbering scope of an entry: its floor when every floor restarts at
/// the first number, else the whole plan.
fn scope_of(e: &Entry, numbering: Numbering) -> usize {
    match numbering {
        Numbering::ByFloor => e.floor,
        Numbering::Whole => 0,
    }
}

/// Fills in each entry's `mark` (entries are in floor order): `prefix` and a
/// two-digit number, or the object's own schedule number. This is the plain
/// numbering by position; [`number_schedule`] is the one a schedule uses.
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
        e.n = n as u32;
        let mark = e
            .mark_override
            .clone()
            .unwrap_or_else(|| format!("{prefix}{n:02}"));
        e.set("mark", mark);
    }
}

/// Numbers the listed objects of `def` (manual p. 715). A schedule that
/// recorded numbers keeps them, whatever else changes; an object it has no
/// number for yet goes below the last one, in the order the objects were
/// placed (their ids). A schedule with no record numbers by position. With
/// Group Similar Objects the objects of one row share its number.
fn number_schedule(
    entries: &mut Vec<Entry>,
    def: &Schedule,
    columns: &[plan_core::schedules::ColumnSpec],
) {
    let scope = |e: &Entry| scope_of(e, def.numbering);
    let mut n_of: Vec<u32> = vec![0; entries.len()];
    if def.group_similar {
        let mut seen: BTreeMap<(usize, String), u32> = BTreeMap::new();
        let mut next: BTreeMap<usize, u32> = BTreeMap::new();
        for (i, e) in entries.iter().enumerate() {
            let key = (scope(e), identity_key(e, columns, def));
            let n = *seen.entry(key).or_insert_with(|| {
                let c = next.entry(scope(e)).or_insert(0);
                *c += 1;
                *c
            });
            n_of[i] = n;
        }
    } else if def.numbers.is_empty() {
        let mut next: BTreeMap<usize, u32> = BTreeMap::new();
        for (i, e) in entries.iter().enumerate() {
            let c = next.entry(scope(e)).or_insert(0);
            *c += 1;
            n_of[i] = *c;
        }
    } else {
        let mut top: BTreeMap<usize, u32> = BTreeMap::new();
        let mut fresh: Vec<usize> = Vec::new();
        for (i, e) in entries.iter().enumerate() {
            match (e.id != 0)
                .then(|| def.number_of(e.kind, e.floor, e.id))
                .flatten()
            {
                Some(n) => {
                    n_of[i] = n;
                    let t = top.entry(scope(e)).or_insert(0);
                    *t = (*t).max(n);
                }
                None => fresh.push(i),
            }
        }
        // New objects in placement order; lines that stand for no single
        // object (rooms, framing) keep their order after those.
        fresh.sort_by_key(|&i| (entries[i].id == 0, entries[i].id, i));
        for i in fresh {
            let t = top.entry(scope(&entries[i])).or_insert(0);
            *t += 1;
            n_of[i] = *t;
        }
    }
    for (e, n) in entries.iter_mut().zip(&n_of) {
        e.n = *n;
    }
    // Rows follow their numbers.
    let mut order: Vec<usize> = (0..entries.len()).collect();
    order.sort_by_key(|&i| {
        let e = &entries[i];
        (
            match def.numbering {
                Numbering::ByFloor => e.floor,
                Numbering::Whole => 0,
            },
            e.n,
            i,
        )
    });
    let mut slots: Vec<Option<Entry>> = std::mem::take(entries).into_iter().map(Some).collect();
    for i in order {
        if let Some(mut e) = slots[i].take() {
            let mark = e
                .mark_override
                .clone()
                .unwrap_or_else(|| def.mark_text(e.n));
            e.set("mark", mark);
            entries.push(e);
        }
    }
}

/// The objects `def` lists on any floor, numbered: its categories, custom
/// categories and rooms applied, floors not yet. Numbers are counted over
/// the whole plan, so a schedule that lists one floor shows the right marks.
fn listed(project: &Project, def: &Schedule, active: ActiveRooms) -> Vec<Entry> {
    let mut all: Vec<Entry> = entries(project, def.kind, active)
        .into_iter()
        .filter(|e| category_ticked(def, &e.category) || in_ticked_custom(project, def, e))
        .collect();
    // Objects of another kind that a ticked custom category adds
    // ("a dishwasher in the cabinet schedule").
    if def.kind != ScheduleKind::General
        && def
            .categories
            .iter()
            .any(|(k, on)| *on && k.starts_with("Custom/"))
    {
        for k in ScheduleKind::ALL {
            if k == def.kind || k == ScheduleKind::General {
                continue;
            }
            all.extend(
                entries(project, k, active)
                    .into_iter()
                    .filter(|e| in_ticked_custom(project, def, e)),
            );
        }
    }
    all.sort_by_key(|e| e.floor);
    if !def.rooms.is_empty() {
        let polys = scope_polygons(project, def, active);
        all.retain(|e| {
            polys
                .iter()
                .any(|(f, poly)| *f == e.floor && inside_or_on(poly, e.position))
        });
    }
    let columns = effective_columns(project, def);
    number_schedule(&mut all, def, &columns);
    all
}

/// The label of every object `def` numbers on `floor` (empty unless the
/// schedule shows labels and its kind has them).
pub fn callouts(project: &Project, floor: usize, def: &Schedule) -> Vec<Callout> {
    if !def.show_labels
        || !def.kind.has_labels()
        || def.label.format == plan_core::schedules::LabelFormat::LabelOnly
    {
        return Vec::new();
    }
    listed(project, def, None)
        .into_iter()
        .filter(|e| e.floor == floor)
        // Show Schedule Callout cleared on the object's Schedule panel.
        .filter(|e| {
            prop_key(e.kind, e.floor, e.id, e.position).is_none_or(|k| {
                project
                    .props
                    .pages_of(k.as_str())
                    .and_then(|p| p.schedule.as_ref())
                    .is_none_or(|s| s.show_callout)
            })
        })
        .map(|e| Callout {
            kind: e.kind,
            floor,
            object: e.id,
            text: e.cell("mark").to_string(),
            at: e.callout,
        })
        .collect()
}

/// Every callout on `floor`: for each of door, window, cabinet and fixture,
/// the labels of every schedule of that kind on the floor that shows them
/// (an object listed in two schedules shows a callout for each, p. 719).
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
        for def in layer.label_sources(kind) {
            out.extend(callouts(project, floor, def));
        }
    }
    out
}

/// The numbers `def` would record for the objects it lists as they stand:
/// existing objects in ascending order of their label (manual p. 715), the
/// way a new schedule starts. Lines that stand for no single object have none.
pub fn snapshot_numbers(project: &Project, def: &Schedule, active: ActiveRooms) -> Vec<NumRec> {
    let mut fresh = def.clone();
    fresh.numbers.clear();
    fresh.group_similar = false;
    let mut all = listed(project, &fresh, active);
    all.sort_by(|a, b| {
        scope_of(a, def.numbering)
            .cmp(&scope_of(b, def.numbering))
            .then_with(|| natural_cmp(&a.text("label"), &b.text("label")))
            .then_with(|| reading_key(a.position).cmp(&reading_key(b.position)))
            .then_with(|| a.id.cmp(&b.id))
    });
    records_in_order(&all, def.numbering)
}

/// Consecutive records for `entries` in the order given, per numbering scope.
fn records_in_order(entries: &[Entry], numbering: Numbering) -> Vec<NumRec> {
    let mut next: BTreeMap<usize, u32> = BTreeMap::new();
    let mut out = Vec::new();
    for e in entries {
        let c = next.entry(scope_of(e, numbering)).or_insert(0);
        *c += 1;
        if e.id != 0 {
            out.push(NumRec {
                kind: e.kind,
                floor: e.floor,
                id: e.id,
                n: *c,
            });
        }
    }
    out
}

/// Renumber Schedule: the numbers close up, keeping the order the schedule
/// lists its objects in (p. 715).
pub fn renumbered(project: &Project, def: &Schedule, active: ActiveRooms) -> Vec<NumRec> {
    let mut plain = def.clone();
    plain.group_similar = false;
    let all = listed(project, &plain, active);
    records_in_order(&all, def.numbering)
}

/// Moves the row at `from` to `to` among the rows of `def` (the Move Row
/// handles, Move Up / Move Down in Schedule): the objects in between shift by
/// one number. `None` when either row is missing or they are on different
/// numbering scopes.
pub fn moved_numbers(
    project: &Project,
    def: &Schedule,
    home_floor: usize,
    active: ActiveRooms,
    from: usize,
    to: usize,
) -> Option<Vec<NumRec>> {
    let mut plain = def.clone();
    plain.group_similar = false;
    let all = listed(project, &plain, active);
    let shown = rows(project, &plain, home_floor, active);
    let (a, b) = (shown.get(from)?, shown.get(to)?);
    if scope_of(a, def.numbering) != scope_of(b, def.numbering) || a.id == 0 {
        return None;
    }
    // The entries of the scope in number order; take `a` out and put it at
    // the place `b` has.
    let scope = scope_of(a, def.numbering);
    let mut order: Vec<&Entry> = all
        .iter()
        .filter(|e| scope_of(e, def.numbering) == scope)
        .collect();
    let at = |order: &[&Entry], e: &Entry| {
        order
            .iter()
            .position(|x| x.kind == e.kind && x.floor == e.floor && x.id == e.id)
    };
    let ia = at(&order, a)?;
    let ib = at(&order, b)?;
    let moving = order.remove(ia);
    order.insert(ib, moving);
    let mut records = records_in_order(&all, def.numbering)
        .into_iter()
        .filter(|r| {
            all.iter()
                .find(|e| e.kind == r.kind && e.floor == r.floor && e.id == r.id)
                .is_some_and(|e| scope_of(e, def.numbering) != scope)
        })
        .collect::<Vec<_>>();
    let owned: Vec<Entry> = order.iter().map(|e| (*e).clone()).collect();
    records.extend(records_in_order(&owned, def.numbering));
    Some(records)
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
        let o = natural_cmp(&a.text(&sort.field), &b.text(&sort.field));
        if sort.descending {
            o.reverse()
        } else {
            o
        }
    });
}

/// The rows `def` lists, marked, scoped to its floors and rooms, filtered
/// and sorted. `home_floor` is the floor the schedule is placed on.
pub fn rows(
    project: &Project,
    def: &Schedule,
    home_floor: usize,
    active: ActiveRooms,
) -> Vec<Entry> {
    let all = listed(project, def, active);
    let needle = def.filter.trim().to_lowercase();
    let mut shown: Vec<Entry> = all
        .into_iter()
        .filter(|e| def.lists_floor(e.floor, home_floor))
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

/// Fields whose cells are numbers the legacy Totals line adds up.
const SUMMED: [&str; 7] = [
    "area",
    "standard_area",
    "perimeter",
    "qty",
    "count",
    "linear",
    "board_feet",
];

/// The columns `def` shows: its visible columns the kind (or a custom
/// property of the kind) supplies, then any custom property flagged "show in
/// schedule" that the schedule has no column for.
pub fn effective_columns(
    project: &Project,
    def: &Schedule,
) -> Vec<plan_core::schedules::ColumnSpec> {
    let pk = prop_kind_of(def.kind);
    let prop_def = |field: &str| -> Option<&PropDef> {
        let name = field.strip_prefix(plan_core::props::COLUMN_PREFIX)?;
        project.props.def(pk?, name)
    };
    let mut cols: Vec<plan_core::schedules::ColumnSpec> = def
        .visible_columns()
        .filter(|c| {
            def.kind.fields().iter().any(|f| f.id == c.field) || prop_def(&c.field).is_some()
        })
        .cloned()
        .collect();
    if let Some(pk) = pk {
        for d in project.props.defs_for(pk).filter(|d| d.show_in_schedule) {
            if !def.columns.iter().any(|c| c.field == d.column_id()) {
                cols.push(plan_core::schedules::ColumnSpec::new(
                    &d.column_id(),
                    &d.name,
                    true,
                ));
            }
        }
    }
    cols
}

/// A column's heading: its own title, else the kind's field title, else the
/// custom property's name.
fn column_title(def: &Schedule, c: &plan_core::schedules::ColumnSpec) -> String {
    if !c.title.trim().is_empty() {
        return c.title.clone();
    }
    def.kind
        .fields()
        .iter()
        .find(|f| f.id == c.field)
        .map(|f| f.title.to_string())
        .or_else(|| {
            c.field
                .strip_prefix(plan_core::props::COLUMN_PREFIX)
                .map(str::to_string)
        })
        .unwrap_or_default()
}

/// The text of a number: the plain form a column without Number Formatting
/// shows for a sum (a grouped row or a total).
fn plain_number(v: f64, kind: NumKind) -> String {
    match kind {
        NumKind::Length => fmt_ft_in(v),
        NumKind::Count => format!("{v:.0}"),
        _ => format!("{v:.1}"),
    }
}

/// The text `format` (or the plain form) gives the number `v` of a column.
fn number_text(v: f64, kind: NumKind, format: Option<&NumFormat>) -> String {
    match format {
        Some(f) => format_value(v, kind, f),
        None => plain_number(v, kind),
    }
}

/// One column's number over the objects of a row: the sum when the column
/// sums similar rows, else the first object's value; the Quantity is the
/// number of objects.
fn column_value(
    members: &[&Entry],
    c: &plan_core::schedules::ColumnSpec,
    kind: ScheduleKind,
) -> Option<f64> {
    let nk = kind.num_kind(&c.field)?;
    if c.field == "quantity" {
        return Some(members.len() as f64);
    }
    if c.sum_similar {
        let vals: Vec<f64> = members
            .iter()
            .filter_map(|m| m.value(&c.field, nk))
            .collect();
        (!vals.is_empty()).then(|| vals.iter().sum())
    } else {
        members[0].value(&c.field, nk)
    }
}

/// The cell of column `c` for the objects of one row.
fn cell_of(members: &[&Entry], c: &plan_core::schedules::ColumnSpec, kind: ScheduleKind) -> String {
    let first = members[0];
    let Some(nk) = kind.num_kind(&c.field) else {
        return first.text(&c.field);
    };
    let grouped = members.len() > 1;
    match column_value(members, c, kind) {
        Some(v) if c.format.is_some() || (grouped && (c.sum_similar || c.field == "quantity")) => {
            number_text(v, nk, c.format.as_ref())
        }
        Some(v) if c.field == "quantity" => number_text(v, nk, None),
        _ => first.text(&c.field),
    }
}

/// What the object preview columns draw for one row: the first object of the
/// row (the previews do not decide whether objects share a row).
#[derive(Debug, Clone, PartialEq)]
pub struct Preview {
    pub kind: ScheduleKind,
    /// The object's category (`Door/Hinged Door`), which picks the picture.
    pub category: String,
    /// Plan width, depth and height of the object, inches.
    pub width: f64,
    pub depth: f64,
    pub height: f64,
    /// The mark the Callout Symbol column shows.
    pub mark: String,
}

fn preview_of(e: &Entry) -> Preview {
    let num = |id: &str, kind: NumKind| e.value(id, kind).unwrap_or(0.0);
    let width = num("width", NumKind::Length);
    Preview {
        kind: e.kind,
        category: e.category.clone(),
        width,
        depth: match e.kind {
            ScheduleKind::Door | ScheduleKind::Window => 6.0,
            ScheduleKind::Wall => num("thickness", NumKind::Length),
            _ => num("depth", NumKind::Length),
        },
        height: num("height", NumKind::Length),
        mark: e.cell("mark").to_string(),
    }
}

/// One line of the table with the objects behind it.
struct DisplayRow {
    cells: Vec<String>,
    targets: Vec<RowTarget>,
    preview: Option<Preview>,
}

impl DisplayRow {
    fn blank(cells: Vec<String>) -> Self {
        Self {
            cells,
            targets: Vec::new(),
            preview: None,
        }
    }
}

/// The rows as listed (one per object), as groups of similar objects
/// (Group Similar Objects, or `def.group_by`), padded to the Minimum Rows,
/// with a Totals row and the legacy Totals line when the schedule asks for
/// them, and the objects behind each row. The totals and blank rows have no
/// targets.
fn display_rows(
    project: &Project,
    def: &Schedule,
    home_floor: usize,
    active: ActiveRooms,
) -> Vec<DisplayRow> {
    let shown = rows(project, def, home_floor, active);
    let columns = effective_columns(project, def);
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
    // (objects of the row)
    let mut body: Vec<(
        Vec<String>,
        Vec<RowTarget>,
        Vec<Option<f64>>,
        Option<Preview>,
    )> = Vec::new();
    let calc: Vec<usize> = columns
        .iter()
        .enumerate()
        .filter(|(_, c)| c.calc_total && def.kind.num_kind(&c.field).is_some())
        .map(|(i, _)| i)
        .collect();
    if grouping {
        // Groups keep the order of their first member.
        let mut groups: Vec<(String, Vec<&Entry>)> = Vec::new();
        for e in &shown {
            let key = e.text(&def.group_by);
            match groups.iter_mut().find(|(k, _)| *k == key) {
                Some((_, v)) => v.push(e),
                None => groups.push((key, vec![e])),
            }
        }
        for (_, members) in groups {
            let cells = columns
                .iter()
                .map(|c| {
                    let first = members[0].text(&c.field);
                    let same = members.iter().all(|m| m.text(&c.field) == first);
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
                        first
                    } else {
                        "*".to_string()
                    }
                })
                .collect();
            let values = calc
                .iter()
                .map(|&i| column_value(&members, &columns[i], def.kind))
                .collect();
            let preview = Some(preview_of(members[0]));
            body.push((
                cells,
                members.iter().map(|m| target(m)).collect(),
                values,
                preview,
            ));
        }
    } else if def.group_similar {
        let mut groups: Vec<(String, Vec<&Entry>)> = Vec::new();
        for e in &shown {
            let key = identity_key(e, &columns, def);
            match groups.iter_mut().find(|(k, _)| *k == key) {
                Some((_, v)) => v.push(e),
                None => groups.push((key, vec![e])),
            }
        }
        for (_, members) in groups {
            let cells = columns
                .iter()
                .map(|c| cell_of(&members, c, def.kind))
                .collect();
            let values = calc
                .iter()
                .map(|&i| column_value(&members, &columns[i], def.kind))
                .collect();
            let preview = Some(preview_of(members[0]));
            body.push((
                cells,
                members.iter().map(|m| target(m)).collect(),
                values,
                preview,
            ));
        }
    } else {
        for e in &shown {
            let members = [e];
            let cells = columns
                .iter()
                .map(|c| cell_of(&members, c, def.kind))
                .collect();
            let values = calc
                .iter()
                .map(|&i| column_value(&members, &columns[i], def.kind))
                .collect();
            body.push((cells, vec![target(e)], values, Some(preview_of(e))));
        }
    }
    let mut out: Vec<DisplayRow> = Vec::new();
    let totals_row = (def.totals_row && !calc.is_empty() && !columns.is_empty()).then(|| {
        let mut cells = vec![String::new(); columns.len()];
        for (slot, &i) in calc.iter().enumerate() {
            let Some(nk) = def.kind.num_kind(&columns[i].field) else {
                continue;
            };
            let sum: f64 = body.iter().filter_map(|(_, _, v, _)| v[slot]).sum();
            cells[i] = number_text(sum, nk, columns[i].format.as_ref());
        }
        // The label sits in the leftmost column unless that column totals.
        if !calc.contains(&0) {
            cells[0] = def.totals_label.clone();
        }
        cells
    });
    let n_body = body.len();
    for (cells, targets, _, preview) in body {
        out.push(DisplayRow {
            cells,
            targets,
            preview,
        });
    }
    for _ in n_body..def.min_rows {
        out.push(DisplayRow::blank(vec![String::new(); columns.len()]));
    }
    if let Some(cells) = totals_row {
        out.push(DisplayRow::blank(cells));
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
        out.push(DisplayRow::blank(cells));
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
        .map(|r| r.targets)
        .collect()
}

/// The table as the schedule shows it on a sheet: [`table`] with Swap
/// Rows/Columns applied (objects across, attributes down). Wrapping into
/// several tables is the view's: it needs the row heights.
pub fn display_table(
    project: &Project,
    def: &Schedule,
    home_floor: usize,
    active: ActiveRooms,
) -> Table {
    let t = table(project, def, home_floor, active);
    if def.swap {
        t.transposed()
    } else {
        t
    }
}

/// A built schedule: the table, what each row stands for and the pictures of
/// the preview columns, all from one pass over the plan.
#[derive(Debug, Clone)]
pub struct Built {
    pub table: Table,
    pub targets: Vec<Vec<RowTarget>>,
    pub previews: Vec<Option<Preview>>,
}

/// [`table`], [`row_targets`] and [`row_previews`] in one pass.
pub fn built(project: &Project, def: &Schedule, home_floor: usize, active: ActiveRooms) -> Built {
    let columns = effective_columns(project, def);
    let rows = display_rows(project, def, home_floor, active);
    let mut b = Built {
        table: Table {
            title: def.display_title(),
            columns: columns.iter().map(|c| column_title(def, c)).collect(),
            rows: Vec::with_capacity(rows.len()),
        },
        targets: Vec::with_capacity(rows.len()),
        previews: Vec::with_capacity(rows.len()),
    };
    for r in rows {
        b.table.rows.push(r.cells);
        b.targets.push(r.targets);
        b.previews.push(r.preview);
    }
    b
}

/// The pictures the object preview columns (Callout Symbol, 2D Symbol, 3D
/// Elevation, 3D Perspective) draw for each row of [`table`], in the same
/// order; `None` for the blank and Totals rows.
pub fn row_previews(
    project: &Project,
    def: &Schedule,
    home_floor: usize,
    active: ActiveRooms,
) -> Vec<Option<Preview>> {
    display_rows(project, def, home_floor, active)
        .into_iter()
        .map(|r| r.preview)
        .collect()
}

/// The schedule as a table: title, the visible columns in their order, and
/// one row per object (or per group of similar ones), padded to the Minimum
/// Rows and ended by the Totals row when the schedule asks for them. Swap
/// Rows/Columns and Wrapping are the view's: this is the data.
pub fn table(project: &Project, def: &Schedule, home_floor: usize, active: ActiveRooms) -> Table {
    let columns = effective_columns(project, def);
    Table {
        title: def.display_title(),
        columns: columns.iter().map(|c| column_title(def, c)).collect(),
        rows: display_rows(project, def, home_floor, active)
            .into_iter()
            .map(|r| r.cells)
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{house, rect_walls};
    use plan_core::schedules::{ColumnSpec, FloorScope};
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
    fn a_cabinet_switched_out_of_the_schedule_is_not_listed() {
        let mut p = rich();
        let mut cabs = p.floors[0].cabinets_as::<Cabinet>().unwrap();
        let before = table(&p, &def(ScheduleKind::Cabinet), 0, None).rows.len();
        assert!(before >= 1);
        cabs[0].in_schedule = false;
        p.floors[0].set_cabinets(&cabs).unwrap();
        let t = table(&p, &def(ScheduleKind::Cabinet), 0, None);
        assert_eq!(t.rows.len(), before - 1);
        let targets = row_targets(&p, &def(ScheduleKind::Cabinet), 0, None);
        assert!(targets.iter().flatten().all(|e| e.id != cabs[0].id));
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
    fn the_wall_schedule_follows_the_object_information_and_schedule_tabs() {
        let mut p = Project::new("W");
        rect_walls(&mut p, 240.0, 120.0, 4.5, WallKind::Exterior);
        let ids: Vec<_> = p.floors[0].walls.iter().map(|w| w.id).collect();
        {
            let w = p.floors[0].wall_mut(ids[0]).unwrap();
            w.spec.info.id = "W-1".into();
            w.spec.info.description = "Garage wall".into();
            w.spec.schedule.supplier = "Acme".into();
            w.spec.covering.interior.wainscot = "Beadboard".into();
            w.spec.covering.interior.chair_rail = "Chair Rail".into();
        }
        let mut d = def(ScheduleKind::Wall);
        for id in [
            "code",
            "description",
            "supplier",
            "interior_covering",
            "wall_type",
        ] {
            d.columns.push(ColumnSpec::new(id, id, true));
        }
        let t = table(&p, &d, 0, None);
        assert_eq!(t.rows.len(), 4);
        let ix = |name: &str| t.columns.iter().position(|c| c == name).unwrap();
        let row = t.rows.iter().find(|r| r[ix("code")] == "W-1").unwrap();
        assert_eq!(row[ix("description")], "Garage wall");
        assert_eq!(row[ix("supplier")], "Acme");
        assert_eq!(row[ix("interior_covering")], "Beadboard, Chair Rail");
        // A cleared Include in Schedule leaves the wall out.
        p.floors[0].wall_mut(ids[1]).unwrap().spec.schedule.include = false;
        assert_eq!(table(&p, &d, 0, None).rows.len(), 3);
        // So does a generated invisible wall.
        p.floors[0].wall_mut(ids[2]).unwrap().flags.auto_generated = true;
        assert_eq!(table(&p, &d, 0, None).rows.len(), 2);
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
    fn a_spiral_stair_is_named_spiral_and_a_plain_curved_one_curved() {
        use plan_stairs::{Stair, StairParams, StairShape};
        let mut p = rich();
        let mut mk = |spiral: bool, x: f64| {
            Stair::new(
                p.alloc_id(),
                Point::new(x, 40.0),
                0.0,
                StairParams {
                    shape: StairShape::Curved { inner_radius: 6.0 },
                    spiral,
                    ..StairParams::default()
                },
            )
        };
        let (a, b) = (mk(true, 300.0), mk(false, 400.0));
        let mut stairs = p.floors[0].stairs_as::<Stair>().unwrap();
        stairs.extend([a, b]);
        p.floors[0].set_stairs(&stairs).unwrap();
        let t = table(&p, &def(ScheduleKind::Stair), 0, None);
        let types: Vec<&str> = t.rows.iter().map(|r| r[1].as_str()).collect();
        assert!(types.contains(&"Spiral"), "{types:?}");
        assert!(types.contains(&"Curved"), "{types:?}");
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
                "Treads",
                "Risers",
                "Riser height",
                "Tread depth",
                "Total rise",
                "Total run",
                "Width",
                "Headroom"
            ]
        );
        assert_eq!(t.rows.len(), 1, "{:?}", t.rows);
        let row = &t.rows[0];
        assert_eq!(row[0], "S01");
        assert_eq!(row[1], "Straight");
        let params = StairParams::default();
        let sol = plan_stairs::solve(&params);
        assert_eq!(row[2], sol.treads.to_string());
        assert_eq!(row[3], sol.risers.to_string());
        assert_eq!(row[4], format!("{:.3}\"", sol.riser_height));
        assert_eq!(row[8], fmt_ft_in(params.width));
        assert_eq!(row[9], fmt_ft_in(params.headroom_min));
        let targets = row_targets(&p, &def(ScheduleKind::Stair), 0, None);
        assert_eq!(targets[0][0].kind, ScheduleKind::Stair);
        assert_ne!(targets[0][0].id, 0);
    }
    #[test]
    fn schedule_tab_data_reaches_the_columns_and_include_leaves_a_door_out() {
        let mut p = house();
        let doors: Vec<plan_core::Id> = p.floors[0]
            .openings
            .iter()
            .filter(|o| o.kind == OpeningKind::Door)
            .map(|o| o.id)
            .collect();
        assert!(!doors.is_empty());
        let first = doors[0];
        {
            let o = p.floors[0]
                .openings
                .iter_mut()
                .find(|o| o.id == first)
                .unwrap();
            let sch = &mut o.extras.spec.schedule;
            sch.supplier = "Acme Millwork".into();
            sch.manufacturer = "Therma".into();
            sch.model = "TD-3068".into();
            sch.comment = "Primed".into();
        }
        let mut d = def(ScheduleKind::Door);
        for field in ["manufacturer", "model", "supplier", "comment"] {
            d.columns
                .iter_mut()
                .find(|c| c.field == field)
                .unwrap_or_else(|| panic!("no {field} column"))
                .visible = true;
        }
        let t = table(&p, &d, 0, None);
        let row = t
            .rows
            .iter()
            .find(|r| r.iter().any(|c| c == "Acme Millwork"))
            .expect("the supplier is listed");
        assert!(row.iter().any(|c| c == "Therma"));
        assert!(row.iter().any(|c| c == "TD-3068"));
        assert!(row.iter().any(|c| c == "Primed"));
        // "Include in Schedule" cleared: the door is not a row and takes no mark.
        let before = table(&p, &d, 0, None).rows.len();
        p.floors[0]
            .openings
            .iter_mut()
            .find(|o| o.id == first)
            .unwrap()
            .extras
            .spec
            .schedule
            .include = false;
        let after = table(&p, &d, 0, None);
        assert_eq!(after.rows.len(), before - 1);
        assert!(after
            .rows
            .iter()
            .all(|r| !r.iter().any(|c| c == "Acme Millwork")));
        assert!(callouts(&p, 0, &d).iter().all(|c| c.object != first));
    }

    #[test]
    fn rough_opening_energy_and_object_information_reach_the_schedule() {
        let mut p = house();
        let doors: Vec<plan_core::Id> = p.floors[0]
            .openings
            .iter()
            .filter(|o| o.kind == OpeningKind::Door)
            .map(|o| o.id)
            .collect();
        let first = doors[0];
        {
            let o = p.floors[0]
                .openings
                .iter_mut()
                .find(|o| o.id == first)
                .unwrap();
            let spec = &mut o.extras.spec;
            spec.rough.add_width = 2.0;
            spec.rough.add_height = 2.5;
            spec.energy.u_factor = 0.27;
            spec.energy.shgc = 0.2;
            spec.info.description = "Entry door".into();
            spec.info.id = "E-1".into();
        }
        let mut d = def(ScheduleKind::Door);
        for field in ["rough", "u_factor", "shgc", "description", "object_id"] {
            d.columns
                .iter_mut()
                .find(|c| c.field == field)
                .unwrap_or_else(|| panic!("no {field} column"))
                .visible = true;
        }
        let t = table(&p, &d, 0, None);
        let row = t
            .rows
            .iter()
            .find(|r| r.iter().any(|c| c == "Entry door"))
            .expect("the description is listed");
        let o = p.floors[0].openings.iter().find(|o| o.id == first).unwrap();
        assert!(row
            .iter()
            .any(|c| *c == size_text(o.width + 2.0, o.height + 2.5)));
        assert!(row.iter().any(|c| c == "0.27"));
        assert!(row.iter().any(|c| c == "0.20"));
        assert!(row.iter().any(|c| c == "E-1"));
        // The window schedule has the same columns.
        let w = def(ScheduleKind::Window);
        for field in ["rough", "u_factor", "shgc", "description", "object_id"] {
            assert!(w.columns.iter().any(|c| c.field == field), "{field}");
        }
    }
    #[test]
    fn object_pages_leave_objects_out_and_fill_the_information_columns() {
        use plan_core::object_pages::{ObjectPages, SchedulePage};
        let mut p = rich();
        let before = entries(&p, ScheduleKind::Cabinet, None);
        assert_eq!(before.len(), 2);
        let key = prop_key(
            ScheduleKind::Cabinet,
            before[0].floor,
            before[0].id,
            before[0].position,
        )
        .unwrap();
        // Object Information replaces the Comment cell of a wall.
        let walls = entries(&p, ScheduleKind::Wall, None);
        let wkey = prop_key(
            ScheduleKind::Wall,
            walls[0].floor,
            walls[0].id,
            walls[0].position,
        )
        .unwrap();
        let info = plan_core::materials_data::ObjectInfo {
            comment: "Match the island".into(),
            ..Default::default()
        };
        p.materials.objects.insert(wkey.0.clone(), info);
        let walls = entries(&p, ScheduleKind::Wall, None);
        assert_eq!(walls[0].cell("comment"), "Match the island");
        assert_eq!(walls[1].cell("comment"), "");
        // Include in Schedule cleared leaves the cabinet out; a cleared
        // callout keeps the row but drops the label.
        let pages = ObjectPages {
            schedule: Some(SchedulePage {
                include: false,
                ..SchedulePage::default()
            }),
            ..ObjectPages::default()
        };
        assert!(p.props.set_pages(&key.0, pages));
        let e = entries(&p, ScheduleKind::Cabinet, None);
        assert_eq!(e.len(), 1);
        assert_ne!(e[0].id, before[0].id);
        let pages = ObjectPages {
            schedule: Some(SchedulePage {
                show_callout: false,
                ..SchedulePage::default()
            }),
            ..ObjectPages::default()
        };
        assert!(p.props.set_pages(&key.0, pages));
        assert_eq!(entries(&p, ScheduleKind::Cabinet, None).len(), 2);
        let mut d = def(ScheduleKind::Cabinet);
        d.show_labels = true;
        let on_plan: Vec<Id> = callouts(&p, 0, &d).iter().map(|c| c.object).collect();
        assert!(!on_plan.contains(&before[0].id));
    }
}

#[cfg(test)]
mod r16_tests {
    //! Round 16, brief 04: Schedule Specification scope, tables and numbers.
    use super::*;
    use crate::test_support::{house, rect_walls};
    use plan_core::schedules::{
        Accuracy, ColumnSpec, FloorScope, NumFormat, NumUnit, RoomRef, WrapBy,
    };
    use plan_core::WallKind;

    fn def(kind: ScheduleKind) -> Schedule {
        Schedule::new(kind, Point::ZERO)
    }

    fn show(d: &mut Schedule, fields: &[&str]) {
        for f in fields {
            d.columns
                .iter_mut()
                .find(|c| c.field == *f)
                .unwrap_or_else(|| panic!("no {f} column"))
                .visible = true;
        }
    }

    fn col<'a>(d: &'a mut Schedule, field: &str) -> &'a mut ColumnSpec {
        d.columns.iter_mut().find(|c| c.field == field).unwrap()
    }

    fn at(t: &Table, row: usize, heading: &str) -> String {
        let i = t.columns.iter().position(|c| c == heading).unwrap();
        t.rows[row][i].clone()
    }

    #[test]
    fn the_window_schedule_totals_its_area_column_by_default() {
        let p = house();
        let mut d = def(ScheduleKind::Window);
        show(&mut d, &["area"]);
        assert!(col(&mut d, "area").calc_total, "Area totals by default");
        let t = table(&p, &d, 0, None);
        assert_eq!(t.rows.len(), 3, "two windows and the Totals row");
        let each: Vec<f64> = (0..2)
            .map(|r| at(&t, r, "Area").parse::<f64>().unwrap())
            .collect();
        let total: f64 = at(&t, 2, "Area").parse().unwrap();
        assert!(
            (total - (each[0] + each[1])).abs() < 0.11,
            "{total} {each:?}"
        );
        assert_eq!(t.rows[2][0], "Totals", "the label sits in the first column");
        // Display Totals Row off, or a changed label.
        d.totals_label = "Sum".into();
        assert_eq!(table(&p, &d, 0, None).rows[2][0], "Sum");
        d.totals_row = false;
        assert_eq!(table(&p, &d, 0, None).rows.len(), 2);
        // A Totals row only when a column calculates a total.
        d.totals_row = true;
        col(&mut d, "area").calc_total = false;
        assert_eq!(table(&p, &d, 0, None).rows.len(), 2);
    }

    #[test]
    fn calculate_total_works_on_other_columns_and_the_label_gives_way() {
        let p = house();
        let mut d = def(ScheduleKind::Window);
        // The first column is the total: no label then.
        show(&mut d, &["width"]);
        col(&mut d, "width").calc_total = true;
        let i = d.columns.iter().position(|c| c.field == "width").unwrap();
        let c = d.columns.remove(i);
        d.columns.insert(0, c);
        let t = table(&p, &d, 0, None);
        let last = t.rows.last().unwrap();
        assert_ne!(last[0], "Totals");
        // Two 3'-0" windows add up to 6'-0".
        assert_eq!(last[0], fmt_ft_in(2.0 * 36.0));
    }

    #[test]
    fn group_similar_objects_counts_a_row_and_sum_similar_adds_it_up() {
        let p = house();
        let mut d = def(ScheduleKind::Window);
        // Two identical windows on different walls: hide the Wall column so
        // they are similar.
        d.columns.iter_mut().for_each(|c| {
            if c.field == "wall" {
                c.visible = false;
            }
        });
        show(&mut d, &["quantity", "area"]);
        assert_eq!(table(&p, &d, 0, None).rows.len(), 3, "one row per object");
        d.group_similar = true;
        let t = table(&p, &d, 0, None);
        assert_eq!(t.rows.len(), 2, "one group and the Totals row: {t:?}");
        assert_eq!(at(&t, 0, "Quantity"), "2");
        // Both windows share the row's number.
        assert_eq!(at(&t, 0, "Mark"), "W01");
        let one = at(&t, 0, "Area").parse::<f64>().unwrap();
        // Without Sum Similar Rows the row shows one window's area, the
        // Totals row the sum of the rows.
        let total = at(&t, 1, "Area").parse::<f64>().unwrap();
        assert!((total - one).abs() < 1e-9);
        col(&mut d, "area").sum_similar = true;
        let t = table(&p, &d, 0, None);
        let both = at(&t, 0, "Area").parse::<f64>().unwrap();
        assert!((both - 2.0 * one).abs() < 0.11, "{both} vs {one}");
        let total = at(&t, 1, "Area").parse::<f64>().unwrap();
        assert!((total - both).abs() < 1e-9, "the Totals row adds the rows");
        // The targets of the group are both windows.
        assert_eq!(row_targets(&p, &d, 0, None)[0].len(), 2);
    }

    #[test]
    fn minimum_rows_pad_the_table_above_the_totals_row() {
        let p = house();
        let mut d = def(ScheduleKind::Window);
        show(&mut d, &["area"]);
        d.min_rows = 5;
        let t = table(&p, &d, 0, None);
        assert_eq!(t.rows.len(), 6, "five lines and the Totals row");
        assert!(t.rows[2].iter().all(String::is_empty));
        assert!(t.rows[4].iter().all(String::is_empty));
        assert_eq!(t.rows[5][0], "Totals");
        let targets = row_targets(&p, &d, 0, None);
        assert!(targets[2].is_empty() && targets[5].is_empty());
        assert_eq!(targets[0].len(), 1);
    }

    #[test]
    fn number_formatting_sets_a_columns_units_and_fractions() {
        let mut p = house();
        p.floors[0]
            .openings
            .iter_mut()
            .filter(|o| o.kind == OpeningKind::Window)
            .for_each(|o| o.width = 36.3);
        let mut d = def(ScheduleKind::Window);
        let plain = at(&table(&p, &d, 0, None), 0, "Width");
        assert_eq!(plain, fmt_ft_in(36.3));
        // To the nearest quarter inch.
        col(&mut d, "width").format = Some(NumFormat {
            accuracy: Accuracy::Fraction(4),
            ..NumFormat::default()
        });
        let quarter = at(&table(&p, &d, 0, None), 0, "Width");
        assert_eq!(quarter, "3'-0 1/4\"");
        // Decimal inches.
        col(&mut d, "width").format = Some(NumFormat {
            units: NumUnit::Inches,
            accuracy: Accuracy::Decimal(2),
            ..NumFormat::default()
        });
        assert_eq!(at(&table(&p, &d, 0, None), 0, "Width"), "36.30\"");
        // The totals row follows the format.
        col(&mut d, "width").calc_total = true;
        let t = table(&p, &d, 0, None);
        assert_eq!(at(&t, 2, "Width"), "72.60\"");
    }

    #[test]
    fn swap_rows_and_columns_lists_objects_across() {
        let p = house();
        let d = def(ScheduleKind::Window);
        let t = table(&p, &d, 0, None);
        let s = t.transposed();
        assert_eq!(s.columns.len(), 1 + t.rows.len());
        assert_eq!(s.columns[1], "W01");
        assert_eq!(s.columns[2], "W02");
        assert_eq!(s.rows.len(), t.columns.len() - 1);
        assert_eq!(s.rows[0][0], t.columns[1]);
        assert_eq!(s.rows[0][1], t.rows[0][1]);
        assert_eq!(s.rows[0][2], t.rows[1][1]);
    }

    #[test]
    fn wrap_breaks_by_entries_and_by_size() {
        use super::super::schedule::wrap_counts;
        assert_eq!(
            wrap_counts(&[1.0; 5], 0.0, 0.0, WrapBy::Entries(2)),
            [2, 2, 1]
        );
        assert_eq!(wrap_counts(&[1.0; 4], 0.0, 0.0, WrapBy::Entries(10)), [4]);
        // Max Table Size: 25 spent on the title and headings of the first
        // table, 15 on each later one; rows 10 high; 55 at most.
        assert_eq!(
            wrap_counts(&[10.0; 10], 25.0, 15.0, WrapBy::MaxSize(55.0)),
            [3, 4, 3]
        );
        // Headings only on the first table leave more room for rows.
        assert_eq!(
            wrap_counts(&[10.0; 10], 25.0, 0.0, WrapBy::MaxSize(55.0)),
            [3, 5, 2]
        );
        // A table holds at least one row, however tall.
        assert_eq!(
            wrap_counts(&[100.0; 2], 0.0, 0.0, WrapBy::MaxSize(10.0)),
            [1, 1]
        );
        // The wrapped tables share the heading; the Totals row ends the last.
        let p = house();
        let mut d = def(ScheduleKind::Window);
        show(&mut d, &["area"]);
        d.min_rows = 5;
        let t = table(&p, &d, 0, None);
        let parts = t.wrapped(&[2, 2, 1], 1);
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].rows.len(), 2);
        assert_eq!(parts[2].rows.len(), 2, "one row and the Totals row");
        assert_eq!(parts[2].rows[1][0], "Totals");
        assert!(parts.iter().all(|x| x.columns == t.columns));
    }

    #[test]
    fn schedule_to_text_is_tab_delimited() {
        let p = house();
        let d = def(ScheduleKind::Door);
        let t = table(&p, &d, 0, None);
        let text = t.to_tsv(false, true);
        let mut lines = text.lines();
        assert!(lines.next().unwrap().starts_with("Mark\tFloor\tWidth"));
        assert!(lines.next().unwrap().starts_with("D01\t"));
        assert!(t.to_tsv(true, false).starts_with("Door Schedule\n"));
    }

    #[test]
    fn the_categories_tree_filters_the_walls_by_type() {
        let mut p = Project::new("W");
        let ids = rect_walls(&mut p, 240.0, 120.0, 4.5, WallKind::Exterior);
        p.floors[0].wall_mut(ids[0]).unwrap().wall_type = Some("Siding-6".into());
        p.floors[0].wall_mut(ids[1]).unwrap().wall_type = Some("Siding-6".into());
        let tree = category_tree(&p, ScheduleKind::Wall);
        let wall = tree.iter().find(|g| g.id == "Wall").unwrap();
        let ids_of: Vec<&str> = wall.items.iter().map(|n| n.id.as_str()).collect();
        assert!(ids_of.contains(&"Wall/Siding-6"));
        assert!(ids_of.contains(&"Wall/Exterior"), "{ids_of:?}");
        let mut d = def(ScheduleKind::Wall);
        assert_eq!(table(&p, &d, 0, None).rows.len(), 4);
        d.set_category("Wall/Siding-6", false);
        let t = table(&p, &d, 0, None);
        assert_eq!(t.rows.len(), 2, "the siding walls are unticked");
        d.set_category("Wall/Siding-6", true);
        d.set_category("Wall/Exterior", false);
        assert_eq!(table(&p, &d, 0, None).rows.len(), 2);
    }

    #[test]
    fn new_wall_and_room_types_join_a_schedule_but_note_types_do_not() {
        let mut p = Project::new("W");
        let ids = rect_walls(&mut p, 240.0, 120.0, 4.5, WallKind::Exterior);
        let mut wall = def(ScheduleKind::Wall);
        // The user ticked Exterior and nothing else.
        wall.set_category("Wall/Exterior", true);
        p.floors[0].wall_mut(ids[0]).unwrap().wall_type = Some("Stucco-6".into());
        assert!(wall.new_types_included);
        assert_eq!(table(&p, &wall, 0, None).rows.len(), 4, "Stucco-6 joins");
        wall.new_types_included = false;
        assert_eq!(table(&p, &wall, 0, None).rows.len(), 3, "it does not");
        assert!(!def(ScheduleKind::Note).new_types_included);
        assert!(def(ScheduleKind::Wall).new_types_included);
    }

    #[test]
    fn a_fixture_schedule_splits_into_plumbing_appliances_and_hvac() {
        let mut p = Project::new("F");
        rect_walls(&mut p, 240.0, 120.0, 4.5, WallKind::Exterior);
        for (cat, x) in [
            ("core.plumbing.toilet_elongated", 60.0),
            ("core.kitchen.refrigerator", 100.0),
        ] {
            p.add_symbol(
                0,
                PlacedSymbol::new(cat, Point::new(x, 60.0), 20.0, 30.0, 30.0),
            );
        }
        let all = def(ScheduleKind::Fixture);
        let n = table(&p, &all, 0, None).rows.len();
        assert!(n >= 1);
        let cats: Vec<String> = entries(&p, ScheduleKind::Fixture, None)
            .into_iter()
            .map(|e| e.category)
            .collect();
        assert!(cats.iter().all(|c| c.starts_with("Fixture/")), "{cats:?}");
        let mut plumbing_only = def(ScheduleKind::Fixture);
        for id in category_ids(&p, ScheduleKind::Fixture) {
            plumbing_only.set_category(&id, id == "Fixture/Plumbing");
        }
        let kept = table(&p, &plumbing_only, 0, None).rows.len();
        let plumbing = cats.iter().filter(|c| *c == "Fixture/Plumbing").count();
        assert_eq!(kept, plumbing);
    }

    #[test]
    fn a_custom_category_adds_an_object_of_another_kind() {
        let mut p = house();
        let mut cab = Cabinet::base(24.0);
        cab.id = p.alloc_id();
        cab.position = Point::new(20.0, 20.0);
        p.floors[0].set_cabinets(&[cab.clone()]).unwrap();
        p.schedule_setup.add_category("Glazing").unwrap();
        let win = p.floors[0]
            .openings
            .iter()
            .find(|o| o.kind == OpeningKind::Window)
            .unwrap()
            .id;
        // A cabinet and a door listed under Glazing.
        assert!(p
            .schedule_setup
            .assign("Glazing", &PropKey::cabinet(cab.id).0));
        assert!(p.schedule_setup.assign("Glazing", &PropKey::window(win).0));
        // The Door schedule gains the window and the cabinet only when it
        // ticks the category.
        let mut d = def(ScheduleKind::Door);
        assert_eq!(table(&p, &d, 0, None).rows.len(), 1);
        d.set_category("Custom/Glazing", true);
        let t = table(&p, &d, 0, None);
        assert_eq!(t.rows.len(), 3, "{t:?}");
        let targets = row_targets(&p, &d, 0, None);
        let kinds: Vec<ScheduleKind> = targets.iter().map(|t| t[0].kind).collect();
        assert!(kinds.contains(&ScheduleKind::Cabinet));
        assert!(kinds.contains(&ScheduleKind::Window));
        // Unticking a system category leaves the custom one.
        d.set_category("Door/Hinged Door", false);
        assert_eq!(table(&p, &d, 0, None).rows.len(), 2);
    }

    #[test]
    fn include_objects_from_room_lists_what_stands_in_that_room() {
        let mut p = Project::new("Rooms");
        rect_walls(&mut p, 480.0, 240.0, 4.5, WallKind::Exterior);
        // A partition makes two rooms.
        p.add_wall(
            0,
            Point::new(240.0, 0.0),
            Point::new(240.0, 240.0),
            4.5,
            109.0,
            WallKind::Interior,
        );
        let mut a = Cabinet::base(24.0);
        a.id = p.alloc_id();
        a.position = Point::new(20.0, 20.0);
        let mut b = Cabinet::base(24.0);
        b.id = p.alloc_id();
        b.position = Point::new(340.0, 20.0);
        p.floors[0].set_cabinets(&[a.clone(), b.clone()]).unwrap();
        let mut d = def(ScheduleKind::Cabinet);
        assert_eq!(table(&p, &d, 0, None).rows.len(), 2);
        let rooms = room_choices(&p, None);
        assert_eq!(rooms.len(), 2, "{rooms:?}");
        let left = rooms.iter().find(|r| r.2.x < 240.0).unwrap();
        d.rooms = vec![RoomRef::at(left.0, left.2)];
        let t = table(&p, &d, 0, None);
        assert_eq!(t.rows.len(), 1);
        assert_eq!(row_targets(&p, &d, 0, None)[0][0].id, a.id);
        // Both rooms: both cabinets again.
        let right = rooms.iter().find(|r| r.2.x > 240.0).unwrap();
        d.rooms.push(RoomRef::at(right.0, right.2));
        assert_eq!(table(&p, &d, 0, None).rows.len(), 2);
        // Callouts follow the room scope too.
        d.show_labels = true;
        d.rooms.truncate(1);
        assert_eq!(callouts(&p, 0, &d).len(), 1);
    }

    #[test]
    fn several_floors_can_be_chosen_without_all_floors() {
        let mut p = house();
        p.floors.push(plan_core::Floor::new("2nd Floor", 109.0));
        rect_walls_on(&mut p, 1);
        let mut d = def(ScheduleKind::Wall);
        assert_eq!(table(&p, &d, 0, None).rows.len(), 4, "this floor");
        d.floor_scope = FloorScope::All;
        assert_eq!(table(&p, &d, 0, None).rows.len(), 8);
        d.floor_scope = FloorScope::ThisFloor;
        d.floors = vec![1];
        assert_eq!(table(&p, &d, 0, None).rows.len(), 4, "the chosen floor");
        d.floors = vec![0, 1];
        assert_eq!(table(&p, &d, 0, None).rows.len(), 8);
    }

    fn rect_walls_on(p: &mut Project, floor: usize) {
        let c = [
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(0.0, 100.0),
        ];
        for i in 0..4 {
            p.add_wall(floor, c[i], c[(i + 1) % 4], 4.5, 109.0, WallKind::Interior);
        }
    }

    #[test]
    #[ignore = "R16-04 in progress"]
    fn numbers_follow_placement_order_and_renumber_closes_gaps() {
        let mut p = Project::new("N");
        let ids = rect_walls(&mut p, 480.0, 360.0, 6.5, WallKind::Exterior);
        // Three doors placed right to left: ids ascend, x descends.
        let mut doors = Vec::new();
        for off in [400.0, 300.0, 100.0] {
            doors.push(p.add_opening(0, ids[0], off, OpeningKind::Door).unwrap());
        }
        let mut d = def(ScheduleKind::Door);
        // A schedule starts with the objects there are, in label order; all
        // three doors are the same size, so ties go by placement.
        d.numbers = snapshot_numbers(&p, &d, None);
        assert_eq!(d.numbers.len(), 3);
        let mark_of = |p: &Project, d: &Schedule, id: Id| {
            let e = rows(p, d, 0, None)
                .into_iter()
                .find(|e| e.id == id)
                .unwrap();
            e.cell("mark").to_string()
        };
        assert_eq!(mark_of(&p, &d, doors[0]), "D01");
        assert_eq!(mark_of(&p, &d, doors[2]), "D03");
        // A door placed afterwards goes to the bottom, however far left.
        let late = p.add_opening(0, ids[2], 20.0, OpeningKind::Door).unwrap();
        assert_eq!(mark_of(&p, &d, late), "D04");
        // The row order follows the numbers.
        let order: Vec<Id> = rows(&p, &d, 0, None).iter().map(|e| e.id).collect();
        assert_eq!(order, [doors[0], doors[1], doors[2], late]);
        // Deleting the second door leaves a gap: the others keep their numbers.
        p.floors[0].openings.retain(|o| o.id != doors[1]);
        assert_eq!(mark_of(&p, &d, doors[2]), "D03");
        assert_eq!(mark_of(&p, &d, late), "D04");
        // Renumber Schedule closes the gap and keeps the order.
        d.numbers = renumbered(&p, &d, None);
        assert_eq!(mark_of(&p, &d, doors[0]), "D01");
        assert_eq!(mark_of(&p, &d, doors[2]), "D02");
        assert_eq!(mark_of(&p, &d, late), "D03");
    }

    #[test]
    fn moving_a_row_renumbers_the_rows_between() {
        let mut p = Project::new("M");
        let ids = rect_walls(&mut p, 480.0, 360.0, 6.5, WallKind::Exterior);
        let doors: Vec<Id> = [100.0, 200.0, 300.0]
            .iter()
            .map(|off| p.add_opening(0, ids[0], *off, OpeningKind::Door).unwrap())
            .collect();
        let mut d = def(ScheduleKind::Door);
        d.numbers = snapshot_numbers(&p, &d, None);
        // The last door moves to the top.
        d.numbers = moved_numbers(&p, &d, 0, None, 2, 0).unwrap();
        let order: Vec<Id> = rows(&p, &d, 0, None).iter().map(|e| e.id).collect();
        assert_eq!(order, [doors[2], doors[0], doors[1]]);
        assert_eq!(rows(&p, &d, 0, None)[0].cell("mark"), "D01");
        assert!(moved_numbers(&p, &d, 0, None, 0, 9).is_none());
    }

    #[test]
    fn prefix_start_and_number_style_shape_the_marks() {
        let p = house();
        let mut d = def(ScheduleKind::Window);
        d.label_prefix = "WN-".into();
        d.label.start_number = 5;
        d.label.leading_zeros = false;
        let t = table(&p, &d, 0, None);
        assert_eq!(t.rows[0][0], "WN-5");
        assert_eq!(t.rows[1][0], "WN-6");
        d.label.number_style = plan_core::schedules::NumberStyle::UpperAlpha;
        assert_eq!(table(&p, &d, 0, None).rows[0][0], "WN-E");
    }

    #[test]
    fn the_wall_legend_has_total_width_and_upper_and_lower_construction() {
        let mut p = Project::new("L");
        let ids = rect_walls(&mut p, 240.0, 120.0, 4.5, WallKind::Exterior);
        p.register_wall_type(plan_core::defaults::WallTypeDef {
            name: "Stucco".into(),
            layers: vec![
                plan_core::defaults::WallLayer::new("Stucco", 1.0, false, "Stucco"),
                plan_core::defaults::WallLayer::new("Framing", 3.5, true, "Framing"),
            ],
            kind: WallKind::Exterior,
        });
        p.register_wall_type(plan_core::defaults::WallTypeDef {
            name: "Foundation".into(),
            layers: vec![plan_core::defaults::WallLayer::new(
                "Concrete", 8.0, true, "Concrete",
            )],
            kind: WallKind::Exterior,
        });
        {
            let w = p.floors[0].wall_mut(ids[0]).unwrap();
            w.wall_type = Some("Stucco".into());
        }
        {
            let w = p.floors[0].wall_mut(ids[1]).unwrap();
            w.set_class(plan_core::walls::WallClass::Pony {
                upper_type: "Stucco".into(),
                lower_type: "Foundation".into(),
                split_height: 36.0,
                upper_sets_plan_display: true,
            });
        }
        let mut d = def(ScheduleKind::Wall);
        show(
            &mut d,
            &["total_width", "construction_upper", "construction_lower"],
        );
        let t = table(&p, &d, 0, None);
        let ix = |h: &str| t.columns.iter().position(|c| c == h).unwrap();
        let stucco = t
            .rows
            .iter()
            .find(|r| r[ix("Wall Construction, Upper")].contains("Stucco 1\""))
            .unwrap_or_else(|| panic!("the stucco wall: {t:?}"));
        assert_eq!(
            stucco[ix("Wall Construction, Upper")],
            stucco[ix("Wall Construction, Lower")]
        );
        assert_eq!(stucco[ix("Total Width")], fmt_ft_in(4.5));
        let pony = t
            .rows
            .iter()
            .find(|r| r[ix("Wall Construction, Lower")].contains("Concrete"))
            .expect("the pony wall");
        assert!(pony[ix("Wall Construction, Upper")].contains("Stucco"));
        assert_eq!(pony[ix("Total Width")], fmt_ft_in(8.0));
    }

    #[test]
    fn room_finish_schedules_total_area_and_volume() {
        let mut p = Project::new("RF");
        rect_walls(&mut p, 240.0, 120.0, 4.5, WallKind::Exterior);
        let mut d = def(ScheduleKind::RoomFinish);
        show(&mut d, &["area", "volume"]);
        let t = table(&p, &d, 0, None);
        assert_eq!(t.rows.len(), 2, "the room and the Totals row");
        let area: f64 = at(&t, 0, "Area sq ft").parse().unwrap();
        let vol: f64 = at(&t, 0, "Volume").parse().unwrap();
        assert!(area > 100.0);
        assert!(vol > area * 8.0, "ceiling is over 8 feet");
        assert_eq!(at(&t, 1, "Area sq ft"), at(&t, 0, "Area sq ft"));
        assert_eq!(at(&t, 1, "Volume"), at(&t, 0, "Volume"));
    }
}
