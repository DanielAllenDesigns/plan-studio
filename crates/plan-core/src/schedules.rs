//! Schedules as plan objects, and the Project Information record.
//!
//! In Chief a schedule is placed in the plan as a table that updates live.
//! Here a [`Schedule`] stores only the *definition* (which kind of object it
//! lists, its columns, sort, text style, label options and where it sits);
//! the rows are generated from the plan by `plan_docs::schedule_kinds`.
//!
//! # Storage
//!
//! A floor's schedules live in the typed slot `Floor.schedules` as a
//! [`ScheduleLayer`] ([`ScheduleLayer::load`] / [`ScheduleLayer::store`]).
//! Undo and redo restore it with the rest of the project.
//!
//! # Project Information
//!
//! [`ProjectInfo`] (client, designer, job number, revisions, ...) is stored in
//! `Project.info`. [`ProjectInfo::macro_pairs`] lists the layout title-block
//! macros it fills in.

use crate::geometry::Point;
use crate::layers::{Layer, LayerSet};
use crate::model::{Floor, Id};
use serde::{Deserialize, Serialize};

/// Layer schedules (and their callout labels) are drawn on.
pub const SCHEDULE_LAYER: &str = "Schedules";
/// Text style schedule tables use when the plan has no other choice.
pub const SCHEDULE_TEXT_STYLE: &str = "Schedule Style";
/// Text style of the callout labels, when the plan defines it.
pub const SCHEDULE_LABEL_STYLE: &str = "Schedule Label";

// ===================================================================
// Kinds and fields
// ===================================================================

/// What a schedule lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScheduleKind {
    Door,
    Window,
    Room,
    Wall,
    Cabinet,
    Electrical,
    Framing,
    Fixture,
    Furniture,
    Plant,
    /// A custom schedule: every placed object, narrowed by the filter text.
    General,
}

/// One column the plan data can supply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    /// Stable id the row builder keys on.
    pub id: &'static str,
    /// Heading a new column starts with.
    pub title: &'static str,
    /// Shown in a new schedule of the kind.
    pub default: bool,
}

const fn f(id: &'static str, title: &'static str, default: bool) -> Field {
    Field { id, title, default }
}

const DOOR_FIELDS: &[Field] = &[
    f("mark", "Mark", true),
    f("floor", "Floor", true),
    f("width", "Width", true),
    f("height", "Height", true),
    f("type", "Type", true),
    f("wall", "Wall", true),
    f("swing", "Swing", true),
];
const WINDOW_FIELDS: &[Field] = &[
    f("mark", "Mark", true),
    f("width", "Width", true),
    f("height", "Height", true),
    f("sill", "Sill", true),
    f("head", "Head", true),
    f("type", "Type", true),
    f("wall", "Wall", true),
    f("floor", "Floor", false),
];
const ROOM_FIELDS: &[Field] = &[
    f("mark", "Number", true),
    f("name", "Name", true),
    f("area", "Area sq ft", true),
    f("perimeter", "Perimeter ft", true),
    f("ceiling_height", "Ceiling height", true),
    f("floor_finish", "Floor Finish", false),
    f("ceiling_finish", "Ceiling Finish", false),
    f("floor", "Floor", false),
];
const WALL_FIELDS: &[Field] = &[
    f("mark", "Number", true),
    f("type", "Type", true),
    f("length", "Length", true),
    f("thickness", "Thickness", true),
    f("height", "Height", true),
    f("area", "Area sq ft", true),
    f("openings", "Openings", true),
    f("floor", "Floor", false),
];
const CABINET_FIELDS: &[Field] = &[
    f("mark", "Mark", true),
    f("label", "Label", true),
    f("type", "Type", true),
    f("width", "Width", true),
    f("depth", "Depth", true),
    f("height", "Height", true),
    f("elevation", "Elevation", false),
    f("countertop", "Countertop", false),
    f("floor", "Floor", false),
];
const ELECTRICAL_FIELDS: &[Field] = &[
    f("mark", "Mark", true),
    f("type", "Type", true),
    f("label", "Label", true),
    f("height", "Mount Height", true),
    f("circuit", "Circuit", true),
    f("wall", "Wall", false),
    f("floor", "Floor", false),
];
const FRAMING_FIELDS: &[Field] = &[
    f("mark", "Mark", true),
    f("type", "Member", true),
    f("size", "Size", true),
    f("length", "Length", true),
    f("qty", "Qty", true),
    f("floor", "Floor", false),
];
const SYMBOL_FIELDS: &[Field] = &[
    f("mark", "Mark", true),
    f("name", "Name", true),
    f("category", "Category", true),
    f("width", "Width", true),
    f("depth", "Depth", true),
    f("height", "Height", true),
    f("elevation", "Elevation", false),
    f("floor", "Floor", false),
];
const GENERAL_FIELDS: &[Field] = &[
    f("mark", "Mark", true),
    f("category", "Category", true),
    f("name", "Name", true),
    f("size", "Size", true),
    f("floor", "Floor", true),
];

impl ScheduleKind {
    pub const ALL: [ScheduleKind; 11] = [
        ScheduleKind::Door,
        ScheduleKind::Window,
        ScheduleKind::Room,
        ScheduleKind::Wall,
        ScheduleKind::Cabinet,
        ScheduleKind::Electrical,
        ScheduleKind::Framing,
        ScheduleKind::Fixture,
        ScheduleKind::Furniture,
        ScheduleKind::Plant,
        ScheduleKind::General,
    ];

    /// The menu / flyout name: "Door Schedule".
    pub fn title(self) -> &'static str {
        match self {
            ScheduleKind::Door => "Door Schedule",
            ScheduleKind::Window => "Window Schedule",
            ScheduleKind::Room => "Room Schedule",
            ScheduleKind::Wall => "Wall Schedule",
            ScheduleKind::Cabinet => "Cabinet Schedule",
            ScheduleKind::Electrical => "Electrical Schedule",
            ScheduleKind::Framing => "Framing Schedule",
            ScheduleKind::Fixture => "Fixture Schedule",
            ScheduleKind::Furniture => "Furniture Schedule",
            ScheduleKind::Plant => "Plant Schedule",
            ScheduleKind::General => "Schedule",
        }
    }

    /// The short name: "Door".
    pub fn name(self) -> &'static str {
        match self {
            ScheduleKind::General => "General",
            k => k.title().trim_end_matches(" Schedule"),
        }
    }

    /// Mark prefix of a new schedule (`D` gives D01, D02, ...).
    pub fn default_prefix(self) -> &'static str {
        match self {
            ScheduleKind::Door => "D",
            ScheduleKind::Window => "W",
            ScheduleKind::Room => "R",
            ScheduleKind::Wall => "WL",
            ScheduleKind::Cabinet => "C-",
            ScheduleKind::Electrical => "E-",
            ScheduleKind::Framing => "FR-",
            ScheduleKind::Fixture => "F-",
            ScheduleKind::Furniture => "FU-",
            ScheduleKind::Plant => "P-",
            ScheduleKind::General => "G-",
        }
    }

    /// Every column the kind can show, in the order a new schedule lists them.
    pub fn fields(self) -> &'static [Field] {
        match self {
            ScheduleKind::Door => DOOR_FIELDS,
            ScheduleKind::Window => WINDOW_FIELDS,
            ScheduleKind::Room => ROOM_FIELDS,
            ScheduleKind::Wall => WALL_FIELDS,
            ScheduleKind::Cabinet => CABINET_FIELDS,
            ScheduleKind::Electrical => ELECTRICAL_FIELDS,
            ScheduleKind::Framing => FRAMING_FIELDS,
            ScheduleKind::Fixture | ScheduleKind::Furniture | ScheduleKind::Plant => SYMBOL_FIELDS,
            ScheduleKind::General => GENERAL_FIELDS,
        }
    }

    /// Does the plan show a callout label next to each object of this kind
    /// (Chief's Schedule Number Label)?
    pub fn has_labels(self) -> bool {
        matches!(
            self,
            ScheduleKind::Door
                | ScheduleKind::Window
                | ScheduleKind::Cabinet
                | ScheduleKind::Fixture
        )
    }

    /// The default columns: every field, the default ones visible.
    pub fn default_columns(self) -> Vec<ColumnSpec> {
        self.fields()
            .iter()
            .map(|fd| ColumnSpec::new(fd.id, fd.title, fd.default))
            .collect()
    }
}

// ===================================================================
// The schedule object
// ===================================================================

/// One column of a schedule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnSpec {
    /// Field id (see [`ScheduleKind::fields`]).
    pub field: String,
    /// Heading drawn in the table.
    pub title: String,
    pub visible: bool,
    /// Column width in plan inches; `0` sizes the column to its text.
    pub width: f64,
}

impl ColumnSpec {
    pub fn new(field: &str, title: &str, visible: bool) -> Self {
        Self {
            field: field.to_string(),
            title: title.to_string(),
            visible,
            width: 0.0,
        }
    }
}

/// How rows are ordered; an empty `field` keeps the natural order (floor,
/// then reading order across the plan).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SortSpec {
    pub field: String,
    pub descending: bool,
}

/// Where callout numbers restart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Numbering {
    /// Every floor starts again at 01.
    #[default]
    ByFloor,
    /// One running count from the lowest floor up.
    Whole,
}

/// Which floors a schedule lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FloorScope {
    /// Only the floor the schedule is placed on.
    #[default]
    ThisFloor,
    All,
}

/// A schedule placed in the plan: a table that updates live.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Schedule {
    pub id: Id,
    pub kind: ScheduleKind,
    /// Upper-left corner of the table, plan inches.
    pub position: Point,
    pub columns: Vec<ColumnSpec>,
    pub sort: SortSpec,
    /// Name of the text style the table is set in.
    pub text_style: String,
    /// Title row text; empty uses the kind's name ("Door Schedule").
    pub title: String,
    pub numbering: Numbering,
    /// Draw a callout label (D01, W03, C-12...) next to each listed object.
    pub show_labels: bool,
    pub floor_scope: FloorScope,
    /// Text before the number in marks and labels.
    pub label_prefix: String,
    pub layer: String,
    /// Keeps only rows with a cell containing this text (any case). A
    /// `General` schedule lists everything, so this is what narrows it.
    pub filter: String,
}

impl Default for Schedule {
    fn default() -> Self {
        Schedule::new(ScheduleKind::Door, Point::ZERO)
    }
}

impl Schedule {
    /// A schedule of `kind` with Chief's default columns (id assigned by
    /// [`ScheduleLayer::add`]).
    pub fn new(kind: ScheduleKind, position: Point) -> Self {
        Self {
            id: 0,
            kind,
            position,
            columns: kind.default_columns(),
            sort: SortSpec::default(),
            text_style: SCHEDULE_TEXT_STYLE.to_string(),
            title: String::new(),
            numbering: Numbering::ByFloor,
            show_labels: kind.has_labels(),
            floor_scope: FloorScope::ThisFloor,
            label_prefix: kind.default_prefix().to_string(),
            layer: SCHEDULE_LAYER.to_string(),
            filter: String::new(),
        }
    }

    /// The title row text.
    pub fn display_title(&self) -> String {
        if self.title.trim().is_empty() {
            self.kind.title().to_string()
        } else {
            self.title.clone()
        }
    }

    /// The columns that show, in order.
    pub fn visible_columns(&self) -> impl Iterator<Item = &ColumnSpec> {
        self.columns.iter().filter(|c| c.visible)
    }

    /// Shows or hides column `i`. Returns `false` for a bad index.
    pub fn set_column_visible(&mut self, i: usize, visible: bool) -> bool {
        match self.columns.get_mut(i) {
            Some(c) => {
                c.visible = visible;
                true
            }
            None => false,
        }
    }

    /// Moves column `i` one place up (`up`) or down. Returns the new index,
    /// or `None` when it is already at that end.
    pub fn move_column(&mut self, i: usize, up: bool) -> Option<usize> {
        let j = if up { i.checked_sub(1)? } else { i + 1 };
        if i >= self.columns.len() || j >= self.columns.len() {
            return None;
        }
        self.columns.swap(i, j);
        Some(j)
    }

    /// Adds back any field of the kind the columns lack (hidden), and drops
    /// columns whose field the kind does not have. Run after loading a plan
    /// or changing the kind.
    pub fn reconcile_columns(&mut self) {
        let fields = self.kind.fields();
        self.columns
            .retain(|c| fields.iter().any(|fd| fd.id == c.field));
        for fd in fields {
            if !self.columns.iter().any(|c| c.field == fd.id) {
                self.columns.push(ColumnSpec::new(fd.id, fd.title, false));
            }
        }
    }

    /// Changes the kind: columns, prefix and label default start over.
    pub fn set_kind(&mut self, kind: ScheduleKind) {
        if self.kind == kind {
            return;
        }
        let was_default_prefix = self.label_prefix == self.kind.default_prefix();
        self.kind = kind;
        self.columns = kind.default_columns();
        self.sort = SortSpec::default();
        if was_default_prefix {
            self.label_prefix = kind.default_prefix().to_string();
        }
        self.show_labels = kind.has_labels();
    }
}

/// All schedules of one floor.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScheduleLayer {
    pub schedules: Vec<Schedule>,
}

impl ScheduleLayer {
    pub fn is_empty(&self) -> bool {
        self.schedules.is_empty()
    }

    pub fn find(&self, id: Id) -> Option<&Schedule> {
        self.schedules.iter().find(|s| s.id == id)
    }

    pub fn find_mut(&mut self, id: Id) -> Option<&mut Schedule> {
        self.schedules.iter_mut().find(|s| s.id == id)
    }

    /// Adds `schedule`; a zero or used id is replaced with the next free one.
    /// Returns the id.
    pub fn add(&mut self, mut schedule: Schedule) -> Id {
        if schedule.id == 0 || self.find(schedule.id).is_some() {
            schedule.id = self.schedules.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        }
        let id = schedule.id;
        self.schedules.push(schedule);
        id
    }

    pub fn remove(&mut self, id: Id) -> bool {
        let n = self.schedules.len();
        self.schedules.retain(|s| s.id != id);
        self.schedules.len() != n
    }

    /// The first schedule of `kind` that shows labels: the one that governs
    /// the callouts of that kind of object on this floor.
    pub fn label_source(&self, kind: ScheduleKind) -> Option<&Schedule> {
        self.schedules
            .iter()
            .find(|s| s.kind == kind && s.show_labels)
    }

    /// The layer stored on `floor` (empty when it has none or the data does
    /// not parse).
    pub fn load(floor: &Floor) -> Self {
        let mut layer: ScheduleLayer = floor
            .schedules
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        for s in &mut layer.schedules {
            s.reconcile_columns();
        }
        layer
    }

    /// Stores the layer on `floor`; an empty layer clears the slot.
    pub fn store(&self, floor: &mut Floor) {
        floor.schedules = if self.is_empty() {
            None
        } else {
            serde_json::to_value(self).ok()
        };
    }
}

/// Adds the schedule layer to `layers` when the plan lacks it.
pub fn ensure_layer(layers: &mut LayerSet) {
    if layers.get(SCHEDULE_LAYER).is_none() {
        layers
            .layers
            .push(Layer::new(SCHEDULE_LAYER, [60, 60, 60], 18));
    }
}

// ===================================================================
// Project Information
// ===================================================================

/// Tools > Project Information: who the job is for and who drew it. The
/// layout title blocks read these through [`ProjectInfo::macro_pairs`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectInfo {
    pub client_name: String,
    /// Address lines, top to bottom.
    pub client_address: Vec<String>,
    pub client_phone: String,
    pub client_email: String,
    pub designer: String,
    pub company: String,
    pub project_number: String,
    pub project_address: String,
    pub date: String,
    /// Current revision label.
    pub revision: String,
    /// Revision table rows `(number, date, description)`, oldest first.
    pub revisions: Vec<(String, String, String)>,
    pub drawn_by: String,
    pub checked_by: String,
    /// Extra `(key, value)` pairs, available as `%custom.<key>%`.
    pub custom: Vec<(String, String)>,
}

impl ProjectInfo {
    /// The client address on one line, lines joined with ", ".
    pub fn client_address_line(&self) -> String {
        self.client_address
            .iter()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The `(macro name, value)` pairs the layout title blocks substitute.
    ///
    /// The first group are the names `plan_layout::MacroContext::expand`
    /// knows (`%client%`, `%address%`, `%designer%`, `%date%`, `%revision%`,
    /// `%project.number%`). `%address%` is the project address, else the
    /// client's; `%designer%` is "Drawn by" when set, else the designer, which
    /// is what the title block's DRAWN BY box reads. The rest are extra names
    /// (`%client.phone%`, `%company%`, `%checked.by%`, `%custom.<key>%`, ...).
    pub fn macro_pairs(&self) -> Vec<(String, String)> {
        let address = if self.project_address.trim().is_empty() {
            self.client_address_line()
        } else {
            self.project_address.clone()
        };
        let designer = if self.drawn_by.trim().is_empty() {
            self.designer.clone()
        } else {
            self.drawn_by.clone()
        };
        let mut v: Vec<(String, String)> = vec![
            ("%client%".into(), self.client_name.clone()),
            ("%address%".into(), address),
            ("%designer%".into(), designer),
            ("%date%".into(), self.date.clone()),
            ("%revision%".into(), self.revision.clone()),
            ("%project.number%".into(), self.project_number.clone()),
            ("%project.address%".into(), self.project_address.clone()),
            ("%client.address%".into(), self.client_address_line()),
            ("%client.phone%".into(), self.client_phone.clone()),
            ("%client.email%".into(), self.client_email.clone()),
            ("%company%".into(), self.company.clone()),
            ("%drawn.by%".into(), self.drawn_by.clone()),
            ("%checked.by%".into(), self.checked_by.clone()),
        ];
        for (k, val) in &self.custom {
            let k = k.trim();
            if !k.is_empty() {
                v.push((format!("%custom.{k}%"), val.clone()));
            }
        }
        v
    }

    /// Replaces every macro of [`ProjectInfo::macro_pairs`] in `text`.
    pub fn expand(&self, text: &str) -> String {
        self.macro_pairs()
            .into_iter()
            .fold(text.to_string(), |acc, (k, v)| acc.replace(&k, &v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Project;

    #[test]
    fn default_columns_show_the_default_fields_in_order() {
        let s = Schedule::new(ScheduleKind::Cabinet, Point::ZERO);
        let shown: Vec<&str> = s.visible_columns().map(|c| c.title.as_str()).collect();
        assert_eq!(shown, ["Mark", "Label", "Type", "Width", "Depth", "Height"]);
        assert_eq!(s.columns.len(), ScheduleKind::Cabinet.fields().len());
        assert_eq!(s.display_title(), "Cabinet Schedule");
        for k in ScheduleKind::ALL {
            assert_eq!(k.fields()[0].id, "mark", "{k:?}");
            assert!(k.default_columns().iter().any(|c| c.visible), "{k:?}");
        }
    }

    #[test]
    fn columns_hide_and_move() {
        let mut s = Schedule::new(ScheduleKind::Door, Point::ZERO);
        assert!(s.set_column_visible(2, false));
        assert!(!s.visible_columns().any(|c| c.field == "width"));
        assert_eq!(s.move_column(0, true), None);
        assert_eq!(s.move_column(0, false), Some(1));
        assert_eq!(s.columns[0].field, "floor");
        assert!(!s.set_column_visible(99, true));
    }

    #[test]
    fn layer_round_trips_through_the_floor_slot() {
        let mut p = Project::new("s");
        let mut layer = ScheduleLayer::default();
        let id = layer.add(Schedule::new(ScheduleKind::Window, Point::new(10.0, 20.0)));
        assert_eq!(id, 1);
        let id2 = layer.add(Schedule::new(ScheduleKind::Door, Point::ZERO));
        assert_eq!(id2, 2);
        layer.store(&mut p.floors[0]);
        let q = Project::from_json(&p.to_json().unwrap()).unwrap();
        let back = ScheduleLayer::load(&q.floors[0]);
        assert_eq!(back, layer);
        assert!(back.label_source(ScheduleKind::Window).is_some());
        assert!(back.label_source(ScheduleKind::Cabinet).is_none());
        assert!(layer.remove(1));
        layer.store(&mut p.floors[0]);
        assert_eq!(ScheduleLayer::load(&p.floors[0]).schedules.len(), 1);
        ScheduleLayer::default().store(&mut p.floors[0]);
        assert!(p.floors[0].schedules.is_none());
    }

    #[test]
    fn loading_reconciles_columns_with_the_kind() {
        let mut p = Project::new("s");
        let mut s = Schedule::new(ScheduleKind::Door, Point::ZERO);
        s.columns.retain(|c| c.field != "swing");
        s.columns.push(ColumnSpec::new("bogus", "Bogus", true));
        let mut layer = ScheduleLayer::default();
        layer.add(s);
        layer.store(&mut p.floors[0]);
        let back = ScheduleLayer::load(&p.floors[0]);
        let cols = &back.schedules[0].columns;
        assert!(cols.iter().all(|c| c.field != "bogus"));
        let swing = cols.iter().find(|c| c.field == "swing").unwrap();
        assert!(!swing.visible);
    }

    #[test]
    fn set_kind_resets_columns_and_prefix() {
        let mut s = Schedule::new(ScheduleKind::Door, Point::ZERO);
        s.set_kind(ScheduleKind::Window);
        assert_eq!(s.label_prefix, "W");
        assert_eq!(s.columns[1].field, "width");
        let mut custom = Schedule::new(ScheduleKind::Door, Point::ZERO);
        custom.label_prefix = "DR".into();
        custom.set_kind(ScheduleKind::Window);
        assert_eq!(custom.label_prefix, "DR");
    }

    #[test]
    fn project_info_round_trips_and_old_files_load() {
        let mut p = Project::new("info");
        p.info.client_name = "Pat Smith".into();
        p.info.client_address = vec!["12 Oak St".into(), "Atlanta, GA 30301".into()];
        p.info.revisions = vec![("1".into(), "2026-10-01".into(), "Issued".into())];
        p.info.custom = vec![("lot".into(), "14".into())];
        let q = Project::from_json(&p.to_json().unwrap()).unwrap();
        assert_eq!(q.info, p.info);
        let old = r#"{"name":"old","floors":[{"name":"1st Floor","elevation":0.0,
            "ceiling_height":109.125,"walls":[],"openings":[]}],"next_id":1}"#;
        let o = Project::from_json(old).unwrap();
        assert_eq!(o.info, ProjectInfo::default());
        assert!(o.floors[0].schedules.is_none());
    }

    #[test]
    fn macro_pairs_name_the_title_block_macros() {
        let info = ProjectInfo {
            client_name: "Pat Smith".into(),
            client_address: vec!["12 Oak St".into(), "Atlanta, GA".into()],
            designer: "Daniel Allen Designs".into(),
            project_number: "26-014".into(),
            date: "2026-10-08".into(),
            revision: "B".into(),
            drawn_by: "DS".into(),
            custom: vec![("lot".into(), "14".into()), (" ".into(), "skip".into())],
            ..ProjectInfo::default()
        };
        let pairs = info.macro_pairs();
        let get = |k: &str| {
            pairs
                .iter()
                .find(|(n, _)| n == k)
                .map(|(_, v)| v.as_str())
                .unwrap()
        };
        assert_eq!(get("%client%"), "Pat Smith");
        assert_eq!(get("%address%"), "12 Oak St, Atlanta, GA");
        assert_eq!(get("%designer%"), "DS");
        assert_eq!(get("%project.number%"), "26-014");
        assert_eq!(get("%revision%"), "B");
        assert_eq!(get("%custom.lot%"), "14");
        assert!(!pairs.iter().any(|(n, _)| n.contains("skip")));
        assert_eq!(
            info.expand("%client% / %project.number%"),
            "Pat Smith / 26-014"
        );
        let blank = ProjectInfo::default();
        assert_eq!(blank.expand("%client%|"), "|");
    }
}
