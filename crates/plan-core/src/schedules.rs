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

pub mod numfmt;
pub use numfmt::{
    format_value, Accuracy, FractionFormat, FractionStyle, NumFormat, NumKind, NumUnit, Reduce,
    Thousands,
};

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
    /// Stairs and ramps (landings are not listed).
    Stair,
    /// Rooms with their floor, wall, base, crown and ceiling finishes.
    RoomFinish,
    /// The numbered notes of the plan (`Note 3: ...`, `E 1: ...`).
    Note,
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

/// The four object-preview columns (manual p. 717). Their cells are pictures
/// drawn by the plan, so the table text of a preview column is empty.
pub const PREVIEW_FIELDS: [&str; 4] = [
    "callout_symbol",
    "symbol_2d",
    "elevation_3d",
    "perspective_3d",
];

/// Is `id` one of the object-preview columns?
pub fn is_preview_field(id: &str) -> bool {
    PREVIEW_FIELDS.contains(&id)
}

const DOOR_FIELDS: &[Field] = &[
    f("mark", "Mark", true),
    f("floor", "Floor", true),
    f("width", "Width", true),
    f("height", "Height", true),
    f("type", "Type", true),
    f("wall", "Wall", true),
    f("swing", "Swing", true),
    f("style", "Style", false),
    f("label", "Plan Label", false),
    f("manufacturer", "Manufacturer", false),
    f("model", "Model", false),
    f("supplier", "Supplier", false),
    f("comment", "Comment", false),
    f("rough", "Rough Opening", false),
    f("u_factor", "U-Factor", false),
    f("shgc", "SHGC", false),
    f("description", "Description", false),
    f("object_id", "ID", false),
    f("area", "Area", false),
    f("quantity", "Quantity", false),
    f("callout_symbol", "Callout Symbol", false),
    f("symbol_2d", "2D Symbol", false),
    f("elevation_3d", "3D Elevation", false),
    f("perspective_3d", "3D Perspective", false),
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
    f("style", "Style", false),
    f("label", "Plan Label", false),
    f("manufacturer", "Manufacturer", false),
    f("model", "Model", false),
    f("supplier", "Supplier", false),
    f("comment", "Comment", false),
    f("rough", "Rough Opening", false),
    f("u_factor", "U-Factor", false),
    f("shgc", "SHGC", false),
    f("description", "Description", false),
    f("object_id", "ID", false),
    f("area", "Area", false),
    f("quantity", "Quantity", false),
    f("callout_symbol", "Callout Symbol", false),
    f("symbol_2d", "2D Symbol", false),
    f("elevation_3d", "3D Elevation", false),
    f("perspective_3d", "3D Perspective", false),
];
const ROOM_FIELDS: &[Field] = &[
    f("mark", "Number", true),
    f("name", "Name", true),
    f("area", "Area sq ft", true),
    f("standard_area", "Standard Area", false),
    f("perimeter", "Perimeter ft", true),
    f("ceiling_height", "Ceiling height", true),
    f("floor_finish", "Floor Finish", false),
    f("ceiling_finish", "Ceiling Finish", false),
    f("floor", "Floor", false),
    f("volume", "Volume", false),
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
    // Wall Specification tabs: Wall Types, Wall Covering, Object
    // Information and Schedule.
    f("wall_type", "Wall Type", false),
    f("interior_covering", "Interior Covering", false),
    f("exterior_covering", "Exterior Covering", false),
    f("code", "Code", false),
    f("description", "Description", false),
    f("manufacturer", "Manufacturer", false),
    f("model", "Model", false),
    f("supplier", "Supplier", false),
    f("comment", "Comment", false),
    // The wall legend columns (L-235).
    f("total_width", "Total Width", false),
    f("construction_upper", "Wall Construction, Upper", false),
    f("construction_lower", "Wall Construction, Lower", false),
    f("quantity", "Quantity", false),
    f("callout_symbol", "Callout Symbol", false),
    f("symbol_2d", "2D Symbol", false),
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
    f("door_style", "Door Style", false),
    f("drawer_style", "Drawer Style", false),
    f("finish", "Finish", false),
    f("hardware", "Hardware", false),
    f("category", "Category", false),
    f("quantity", "Quantity", false),
    f("callout_symbol", "Callout Symbol", false),
    f("symbol_2d", "2D Symbol", false),
    f("elevation_3d", "3D Elevation", false),
    f("perspective_3d", "3D Perspective", false),
];
const ELECTRICAL_FIELDS: &[Field] = &[
    f("mark", "Mark", true),
    f("type", "Type", true),
    f("count", "Count", true),
    f("label", "Label", true),
    f("height", "Mount Height", true),
    f("circuit", "Circuit", true),
    f("voltage", "Voltage", false),
    f("flags", "Flags", false),
    f("wall", "Wall", false),
    f("floor", "Floor", false),
    f("callout_symbol", "Callout Symbol", false),
    f("symbol_2d", "2D Symbol", false),
    f("elevation_3d", "3D Elevation", false),
    f("perspective_3d", "3D Perspective", false),
];
const FRAMING_FIELDS: &[Field] = &[
    f("mark", "Mark", true),
    f("type", "Member", true),
    f("size", "Size", true),
    f("length", "Length", true),
    f("qty", "Qty", true),
    f("linear", "Linear ft", false),
    f("board_feet", "Board ft", false),
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
    f("manufacturer", "Manufacturer", false),
    f("model", "Model", false),
    f("comment", "Comment", false),
    f("quantity", "Quantity", false),
    f("callout_symbol", "Callout Symbol", false),
    f("symbol_2d", "2D Symbol", false),
    f("elevation_3d", "3D Elevation", false),
    f("perspective_3d", "3D Perspective", false),
];
const STAIR_FIELDS: &[Field] = &[
    f("mark", "Mark", true),
    f("type", "Type", true),
    f("treads", "Treads", true),
    f("risers", "Risers", true),
    f("riser", "Riser height", true),
    f("tread", "Tread depth", true),
    f("rise", "Total rise", true),
    f("run", "Total run", true),
    f("width", "Width", true),
    f("headroom", "Headroom", true),
    f("floor", "Floor", false),
];
const ROOM_FINISH_FIELDS: &[Field] = &[
    f("mark", "Number", true),
    f("name", "Name", true),
    f("floor_finish", "Floor Finish", true),
    f("base", "Base", true),
    f("wall_finish", "Wall Finish", true),
    f("crown", "Crown", false),
    f("ceiling_finish", "Ceiling Finish", true),
    f("area", "Area sq ft", false),
    f("ceiling_height", "Ceiling height", false),
    f("floor", "Floor", false),
    f("volume", "Volume", false),
];
const NOTE_FIELDS: &[Field] = &[
    f("mark", "No.", true),
    f("type", "Type", true),
    f("note", "Note", true),
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
    pub const ALL: [ScheduleKind; 14] = [
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
        ScheduleKind::Stair,
        ScheduleKind::RoomFinish,
        ScheduleKind::Note,
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
            ScheduleKind::Stair => "Stair Schedule",
            ScheduleKind::RoomFinish => "Room Finish Schedule",
            ScheduleKind::Note => "Note Schedule",
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
            ScheduleKind::Stair => "S",
            ScheduleKind::RoomFinish => "RF",
            ScheduleKind::Note => "N",
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
            ScheduleKind::Stair => STAIR_FIELDS,
            ScheduleKind::RoomFinish => ROOM_FINISH_FIELDS,
            ScheduleKind::Note => NOTE_FIELDS,
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

    /// The default columns: every field, the default ones visible. The
    /// Area columns of the Door, Window and Room Finish schedules (and the
    /// Volume of the Room Finish) start with "Calculate Total" on (p. 717).
    pub fn default_columns(self) -> Vec<ColumnSpec> {
        self.fields()
            .iter()
            .map(|fd| ColumnSpec::for_field(self, fd))
            .collect()
    }

    /// What a numeric column of this kind measures (`None` for text and
    /// preview columns). Only these columns can carry a total, a sum of
    /// similar rows and a number format.
    pub fn num_kind(self, field: &str) -> Option<NumKind> {
        use NumKind::*;
        use ScheduleKind as K;
        Some(match (self, field) {
            (_, "quantity" | "count" | "qty") => Count,
            (K::Door | K::Window, "width" | "height" | "sill" | "head") => Length,
            (K::Door | K::Window, "area") => Area,
            (K::Wall, "length" | "thickness" | "height" | "total_width") => Length,
            (K::Wall, "area") => Area,
            (K::Wall, "openings") => Count,
            (K::Room | K::RoomFinish, "area" | "standard_area") => Area,
            (K::Room | K::RoomFinish, "perimeter") => Feet,
            (K::Room | K::RoomFinish, "ceiling_height") => Length,
            (K::Room | K::RoomFinish, "volume") => Volume,
            (K::Cabinet, "width" | "depth" | "height" | "elevation") => Length,
            (K::Electrical, "height") => Length,
            (K::Framing, "length") => Length,
            (K::Framing, "linear") => Feet,
            (K::Framing, "board_feet") => BoardFeet,
            (K::Fixture | K::Furniture | K::Plant, "width" | "depth" | "height" | "elevation") => {
                Length
            }
            (K::Stair, "treads" | "risers") => Count,
            (K::Stair, "width" | "headroom" | "rise" | "run" | "riser" | "tread") => Length,
            _ => return None,
        })
    }

    /// Does the plan draw the object-preview columns for this kind?
    pub fn has_previews(self) -> bool {
        self.fields().iter().any(|fd| is_preview_field(fd.id))
    }

    /// Does the Totals Row apply (the schedule lists Door, Window or Room
    /// Finish objects; manual p. 722)?
    pub fn has_totals_row(self) -> bool {
        matches!(
            self,
            ScheduleKind::Door | ScheduleKind::Window | ScheduleKind::RoomFinish
        )
    }
}

// ===================================================================
// The schedule object
// ===================================================================

/// Horizontal text alignment inside a column (Align Left, Center, Align
/// Right, Justify edit tools; Attributes > Alignment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

impl TextAlign {
    pub const ALL: [TextAlign; 4] = [
        TextAlign::Left,
        TextAlign::Center,
        TextAlign::Right,
        TextAlign::Justify,
    ];

    pub fn name(self) -> &'static str {
        match self {
            TextAlign::Left => "Left",
            TextAlign::Center => "Center",
            TextAlign::Right => "Right",
            TextAlign::Justify => "Justify",
        }
    }
}

/// Vertical text alignment inside a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum VAlign {
    Top,
    #[default]
    Middle,
    Bottom,
}

impl VAlign {
    pub const ALL: [VAlign; 3] = [VAlign::Top, VAlign::Middle, VAlign::Bottom];

    pub fn name(self) -> &'static str {
        match self {
            VAlign::Top => "Top",
            VAlign::Middle => "Middle",
            VAlign::Bottom => "Bottom",
        }
    }
}

/// One column of a schedule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColumnSpec {
    /// Field id (see [`ScheduleKind::fields`]).
    pub field: String,
    /// Heading drawn in the table.
    pub title: String,
    pub visible: bool,
    /// Column width in plan inches; `0` sizes the column to its text.
    pub width: f64,
    /// "Calculate Total": the Totals Row adds this column up.
    pub calc_total: bool,
    /// "Sum Similar Rows": a row of several similar objects reports the
    /// total of all of them in this column.
    pub sum_similar: bool,
    /// Number Formatting of the column (`None`: the plan's length text).
    pub format: Option<NumFormat>,
    /// Alignment of this column (`None` follows the schedule's).
    pub align: Option<TextAlign>,
}

impl Default for ColumnSpec {
    fn default() -> Self {
        Self::new("", "", true)
    }
}

impl ColumnSpec {
    pub fn new(field: &str, title: &str, visible: bool) -> Self {
        Self {
            field: field.to_string(),
            title: title.to_string(),
            visible,
            width: 0.0,
            calc_total: false,
            sum_similar: false,
            format: None,
            align: None,
        }
    }

    /// The column for `fd` as a new schedule of `kind` starts it.
    pub fn for_field(kind: ScheduleKind, fd: &Field) -> Self {
        let mut c = Self::new(fd.id, fd.title, fd.default);
        c.calc_total = matches!(
            (kind, fd.id),
            (
                ScheduleKind::Door | ScheduleKind::Window | ScheduleKind::RoomFinish,
                "area"
            ) | (ScheduleKind::RoomFinish, "volume")
        );
        c
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
    /// Only the floor the schedule is placed on, or the floors picked in
    /// [`Schedule::floors`].
    #[default]
    ThisFloor,
    All,
}

/// A room picked in "Include Objects from Room": rooms have no id, so the
/// room is the one that holds this point on that floor.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RoomRef {
    pub floor: usize,
    pub x: f64,
    pub y: f64,
}

impl RoomRef {
    pub fn at(floor: usize, p: Point) -> Self {
        Self {
            floor,
            x: p.x,
            y: p.y,
        }
    }

    pub fn point(&self) -> Point {
        Point::new(self.x, self.y)
    }
}

/// How big a wrapped table may be (Wrapping).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum WrapBy {
    /// Entries per Table: the most rows (or columns, when swapped).
    Entries(usize),
    /// Max Table Size: the longest table, plan inches.
    MaxSize(f64),
}

/// Where a shorter wrapped table sits against the first (Table Alignment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TableAlign {
    /// Top (Left when swapped).
    #[default]
    Start,
    Centered,
    /// Bottom (Right when swapped).
    End,
}

impl TableAlign {
    pub fn name(self, swapped: bool) -> &'static str {
        match (self, swapped) {
            (TableAlign::Start, false) => "Top",
            (TableAlign::Start, true) => "Left",
            (TableAlign::Centered, _) => "Centered",
            (TableAlign::End, false) => "Bottom",
            (TableAlign::End, true) => "Right",
        }
    }
}

/// The Wrapping group of the Columns/Rows panel: one schedule shown as
/// several tables side by side (or stacked, when swapped).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WrapSpec {
    pub enabled: bool,
    pub by: WrapBy,
    /// Wrapped Schedule Offset: the gap between tables, plan inches.
    pub offset: f64,
    /// Justify Wrapped Tables: stretch rows (or columns) so every table is
    /// the same size.
    pub justify: bool,
    pub align: TableAlign,
    /// Display Title on Wrapped Tables.
    pub title_each: bool,
    /// Display Column Headings in Wrapped Tables.
    pub headings_each: bool,
}

impl Default for WrapSpec {
    fn default() -> Self {
        Self {
            enabled: false,
            by: WrapBy::Entries(10),
            offset: 12.0,
            justify: false,
            align: TableAlign::Start,
            title_each: true,
            headings_each: true,
        }
    }
}

/// Which colour the object previews use (Show Color).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ColorFrom {
    /// The colour assigned to each object.
    #[default]
    Plan,
    /// The colour assigned to the schedule.
    Schedule,
}

/// Object Preview Options.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PreviewOptions {
    pub show_color: bool,
    pub color_from: ColorFrom,
    /// Scale Images: previews share one scale, so sizes compare.
    pub scale_images: bool,
    /// Use Plan View Scale for the 2D Symbol column.
    pub plan_view_scale: bool,
    pub opening_indicators: bool,
    pub casing: bool,
    pub treatments: bool,
}

impl Default for PreviewOptions {
    fn default() -> Self {
        Self {
            show_color: false,
            color_from: ColorFrom::Plan,
            scale_images: true,
            plan_view_scale: false,
            opening_indicators: false,
            casing: true,
            treatments: false,
        }
    }
}

/// Which labels an object listed in a schedule shows (Label Format).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LabelFormat {
    #[default]
    Both,
    /// Callouts only; the object's own label is hidden.
    Callout,
    /// The object's own label only; no callouts.
    LabelOnly,
}

impl LabelFormat {
    pub const ALL: [LabelFormat; 3] = [
        LabelFormat::Both,
        LabelFormat::Callout,
        LabelFormat::LabelOnly,
    ];

    pub fn name(self) -> &'static str {
        match self {
            LabelFormat::Both => "Use Both Callout and Label",
            LabelFormat::Callout => "Use Callout",
            LabelFormat::LabelOnly => "Use Label",
        }
    }
}

/// The ten callout shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CalloutShape {
    /// Text without a shape.
    None,
    Circle,
    Ellipse,
    Rectangle,
    Capsule,
    Diamond,
    Triangle,
    Pentagon,
    Hexagon,
    Octagon,
}

impl CalloutShape {
    pub const ALL: [CalloutShape; 10] = [
        CalloutShape::None,
        CalloutShape::Circle,
        CalloutShape::Ellipse,
        CalloutShape::Rectangle,
        CalloutShape::Capsule,
        CalloutShape::Diamond,
        CalloutShape::Triangle,
        CalloutShape::Pentagon,
        CalloutShape::Hexagon,
        CalloutShape::Octagon,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CalloutShape::None => "None",
            CalloutShape::Circle => "Circle",
            CalloutShape::Ellipse => "Ellipse",
            CalloutShape::Rectangle => "Rectangle",
            CalloutShape::Capsule => "Capsule",
            CalloutShape::Diamond => "Diamond",
            CalloutShape::Triangle => "Triangle",
            CalloutShape::Pentagon => "Pentagon",
            CalloutShape::Hexagon => "Hexagon",
            CalloutShape::Octagon => "Octagon",
        }
    }

    /// The polygon sides of a regular shape (0 for the round and box ones).
    pub fn sides(self) -> usize {
        match self {
            CalloutShape::Diamond => 4,
            CalloutShape::Triangle => 3,
            CalloutShape::Pentagon => 5,
            CalloutShape::Hexagon => 6,
            CalloutShape::Octagon => 8,
            _ => 0,
        }
    }
}

/// How callout numbers are written (Schedule Numbers Format).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum NumberStyle {
    #[default]
    Numeric,
    UpperAlpha,
    LowerAlpha,
    UpperRoman,
    LowerRoman,
}

impl NumberStyle {
    pub const ALL: [NumberStyle; 5] = [
        NumberStyle::Numeric,
        NumberStyle::UpperAlpha,
        NumberStyle::LowerAlpha,
        NumberStyle::UpperRoman,
        NumberStyle::LowerRoman,
    ];

    pub fn name(self) -> &'static str {
        match self {
            NumberStyle::Numeric => "1, 2, 3",
            NumberStyle::UpperAlpha => "A, B, C",
            NumberStyle::LowerAlpha => "a, b, c",
            NumberStyle::UpperRoman => "I, II, III",
            NumberStyle::LowerRoman => "i, ii, iii",
        }
    }
}

fn alpha_number(n: u32, upper: bool) -> String {
    let base = if upper { b'A' } else { b'a' };
    let mut n = n.max(1) - 1;
    let mut out = Vec::new();
    loop {
        out.push(base + (n % 26) as u8);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

fn roman_number(mut n: u32, upper: bool) -> String {
    const TABLE: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for (v, s) in TABLE {
        while n >= v {
            out.push_str(s);
            n -= v;
        }
    }
    if upper {
        out
    } else {
        out.to_lowercase()
    }
}

/// Which layer the callouts of a schedule are placed on (Callout Layer).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum CalloutLayer {
    /// Each callout goes on the label layer of its object.
    ObjectLabel,
    /// Every callout goes on the layer of the schedule.
    #[default]
    Schedule,
    /// Every callout goes on the named layer.
    Custom(String),
}

impl CalloutLayer {
    /// The layer the callouts of `schedule` are on, `None` when each object's
    /// own label layer decides.
    pub fn layer_of<'a>(&'a self, schedule: &'a Schedule) -> Option<&'a str> {
        match self {
            CalloutLayer::ObjectLabel => None,
            CalloutLayer::Schedule => Some(schedule.layer.as_str()),
            CalloutLayer::Custom(n) => Some(n.as_str()),
        }
    }
}

/// The Labels panel beyond the prefix and numbering: what the callouts look
/// like (manual pp. 726 to 728).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LabelOptions {
    pub format: LabelFormat,
    /// Schedule Start Number.
    pub start_number: u32,
    pub number_style: NumberStyle,
    /// Include Leading Zeroes: `D01` rather than `D1`.
    pub leading_zeros: bool,
    /// The shape; `None` takes the kind's own (a circle for doors, a hexagon
    /// for windows).
    pub shape: Option<CalloutShape>,
    pub filled: bool,
    pub fill_color: [u8; 3],
    /// Use the colour of the layer instead of `fill_color`.
    pub fill_by_layer: bool,
    /// 0 to 100 percent.
    pub transparency: u8,
    /// Size the shape to the text.
    pub auto_size: bool,
    /// Shape size when not automatic, plan inches.
    pub size: f64,
    /// Degrees.
    pub shape_angle: f64,
    pub text_angle: f64,
    /// The text angle follows the shape angle.
    pub auto_text_angle: bool,
    pub follow_label: bool,
    /// Callout Layer.
    pub layer: CalloutLayer,
}

impl Default for LabelOptions {
    fn default() -> Self {
        Self {
            format: LabelFormat::Both,
            start_number: 1,
            number_style: NumberStyle::Numeric,
            leading_zeros: true,
            shape: None,
            filled: false,
            fill_color: [255, 255, 255],
            fill_by_layer: true,
            transparency: 0,
            auto_size: true,
            size: 8.0,
            shape_angle: 0.0,
            text_angle: 0.0,
            auto_text_angle: true,
            follow_label: false,
            layer: CalloutLayer::default(),
        }
    }
}

/// The number a schedule gave one object, kept so numbers stay where they
/// are when other objects come and go (manual p. 715).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NumRec {
    /// The kind of the object (a General schedule lists several).
    pub kind: ScheduleKind,
    pub floor: usize,
    pub id: Id,
    /// Its place in the schedule, counted from 1.
    pub n: u32,
}

/// A schedule placed in the plan: a table that updates live.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Schedule {
    pub id: Id,
    pub kind: ScheduleKind,
    /// Upper-left corner of the table, plan inches (before it is turned).
    pub position: Point,
    pub columns: Vec<ColumnSpec>,
    pub sort: SortSpec,
    /// Name of the text style the table body is set in (Main Text Style).
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
    /// Field id rows are grouped by: rows with the same value (and the same
    /// values in the other visible columns) are counted together in one
    /// line. Empty lists every object on its own line.
    pub group_by: String,
    /// Adds a last line with the number of objects listed.
    pub totals: bool,
    // ----- Round 16 (Schedule Specification, manual pp. 719 to 728) -----
    /// "Display" beside the Main Title.
    pub show_title: bool,
    /// Display Column Headings.
    pub show_headings: bool,
    /// Include Objects from Floor: the floors listed when the schedule is not
    /// on All Floors. Empty means the floor it is placed on.
    pub floors: Vec<usize>,
    /// Include Objects from Room: only objects whose centre lies in one of
    /// these rooms. Empty lists the whole floor.
    pub rooms: Vec<RoomRef>,
    /// Ticked and unticked categories of the Categories to Include tree,
    /// by category id (`Wall/Siding`); a category not listed here follows
    /// [`Schedule::category_on`]'s default.
    pub categories: std::collections::BTreeMap<String, bool>,
    /// New Room and Wall types are ticked in a schedule that already exists
    /// (a Note schedule does not).
    pub new_types_included: bool,
    /// Group Similar Objects: objects that share every shown value are one
    /// row with a Quantity.
    pub group_similar: bool,
    /// Display Totals Row (the columns with Calculate Total are added up).
    pub totals_row: bool,
    /// The Totals Row label.
    pub totals_label: String,
    /// Blank rows pad the table to at least this many lines.
    pub min_rows: usize,
    /// Swap Rows/Columns: objects across, attributes down.
    pub swap: bool,
    pub wrap: WrapSpec,
    pub previews: PreviewOptions,
    pub fraction: FractionFormat,
    /// Attributes > Box/Grid.
    pub border: bool,
    pub grid_lines: bool,
    pub h_align: TextAlign,
    pub v_align: VAlign,
    /// Margins between the text and the cell border: left, right, top,
    /// bottom, plan inches.
    pub margins: [f64; 4],
    /// Rotation about the table centre, degrees counter-clockwise.
    pub angle: f64,
    /// Line Style panel: the table's lines (`None` follows the text colour).
    pub line_color: Option<[u8; 3]>,
    pub line_weight: f32,
    /// Fill Style panel: paint the table's background.
    pub fill: bool,
    pub fill_color: Option<[u8; 3]>,
    /// Title Text Style and Header Text Style (empty follows the main one).
    pub title_style: String,
    pub header_style: String,
    pub label: LabelOptions,
    /// The numbers given to objects so far.
    pub numbers: Vec<NumRec>,
    /// The schedule keeps a number record (a schedule made before Round 16
    /// has none and numbers by position until its first Renumber or move).
    /// A new schedule keeps one even while it lists nothing, so an object
    /// placed later goes below the last number and a deleted one leaves a gap.
    pub numbers_recorded: bool,
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
            group_by: String::new(),
            totals: false,
            show_title: true,
            show_headings: true,
            floors: Vec::new(),
            rooms: Vec::new(),
            categories: std::collections::BTreeMap::new(),
            new_types_included: kind != ScheduleKind::Note,
            group_similar: false,
            totals_row: true,
            totals_label: "Totals".to_string(),
            min_rows: 0,
            swap: false,
            wrap: WrapSpec::default(),
            previews: PreviewOptions::default(),
            fraction: FractionFormat::default(),
            border: true,
            grid_lines: true,
            h_align: TextAlign::Left,
            v_align: VAlign::Middle,
            margins: [2.0, 2.0, 1.2, 1.2],
            angle: 0.0,
            line_color: None,
            line_weight: 1.0,
            fill: true,
            fill_color: None,
            title_style: String::new(),
            header_style: String::new(),
            label: LabelOptions::default(),
            numbers: Vec::new(),
            numbers_recorded: false,
        }
    }

    /// Keeps `numbers` as the schedule's record of its objects' numbers.
    pub fn record_numbers(&mut self, numbers: Vec<NumRec>) {
        self.numbers = numbers;
        self.numbers_recorded = true;
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
        // Custom property columns (`prop:Name`) are the project's to define.
        self.columns.retain(|c| {
            c.field.starts_with(crate::props::COLUMN_PREFIX)
                || fields.iter().any(|fd| fd.id == c.field)
        });
        for fd in fields {
            if !self.columns.iter().any(|c| c.field == fd.id) {
                let mut c = ColumnSpec::for_field(self.kind, fd);
                c.visible = false;
                self.columns.push(c);
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
        self.group_by.clear();
        self.categories.clear();
        self.numbers.clear();
        self.numbers_recorded = false;
        self.new_types_included = kind != ScheduleKind::Note;
        if was_default_prefix {
            self.label_prefix = kind.default_prefix().to_string();
        }
        self.show_labels = kind.has_labels();
    }

    /// Is the category `id` ticked? `default_on` is what it is when the
    /// schedule never recorded a choice (system categories on, custom ones
    /// off; a Note type follows [`Schedule::new_types_included`]).
    pub fn category_on(&self, id: &str, default_on: bool) -> bool {
        self.categories.get(id).copied().unwrap_or(default_on)
    }

    /// Ticks or unticks category `id`.
    pub fn set_category(&mut self, id: &str, on: bool) {
        self.categories.insert(id.to_string(), on);
    }

    /// The number the schedule gave an object, if it recorded one.
    pub fn number_of(&self, kind: ScheduleKind, floor: usize, id: Id) -> Option<u32> {
        self.numbers
            .iter()
            .find(|r| r.kind == kind && r.floor == floor && r.id == id)
            .map(|r| r.n)
    }

    /// The text of the `n`th schedule number (counted from 1): the start
    /// number, the number style and the leading zero applied, without the
    /// prefix.
    pub fn number_text(&self, n: u32) -> String {
        let v = n.saturating_sub(1) + self.label.start_number;
        match self.label.number_style {
            NumberStyle::Numeric if self.label.leading_zeros => format!("{v:02}"),
            NumberStyle::Numeric => v.to_string(),
            NumberStyle::UpperAlpha => alpha_number(v, true),
            NumberStyle::LowerAlpha => alpha_number(v, false),
            NumberStyle::UpperRoman => roman_number(v, true),
            NumberStyle::LowerRoman => roman_number(v, false),
        }
    }

    /// The mark of the `n`th object: prefix and number.
    pub fn mark_text(&self, n: u32) -> String {
        format!("{}{}", self.label_prefix, self.number_text(n))
    }

    /// The rooms picked for "Include Objects from Room" on `floor`.
    pub fn rooms_on(&self, floor: usize) -> impl Iterator<Item = &RoomRef> {
        self.rooms.iter().filter(move |r| r.floor == floor)
    }

    /// Is the schedule limited to chosen floors or rooms?
    pub fn has_scope(&self) -> bool {
        self.floor_scope == FloorScope::ThisFloor && !self.floors.is_empty()
            || !self.rooms.is_empty()
    }

    /// Does the schedule list objects from `floor`, placed on `home`?
    pub fn lists_floor(&self, floor: usize, home: usize) -> bool {
        match self.floor_scope {
            FloorScope::All => true,
            FloorScope::ThisFloor if self.floors.is_empty() => floor == home,
            FloorScope::ThisFloor => self.floors.contains(&floor),
        }
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
        self.label_sources(kind).next()
    }

    /// Every schedule of `kind` that shows labels. An object listed in more
    /// than one of them shows a callout for each (manual p. 719).
    pub fn label_sources(&self, kind: ScheduleKind) -> impl Iterator<Item = &Schedule> {
        self.schedules
            .iter()
            .filter(move |s| s.kind == kind && s.show_labels)
    }

    /// The layer stored on `floor` (empty when it has none or the data does
    /// not parse).
    pub fn load(floor: &Floor) -> Self {
        let mut layer: ScheduleLayer = floor
            .schedules
            .as_ref()
            .map(crate::foreign::read_layer)
            .unwrap_or_default();
        for s in &mut layer.schedules {
            s.reconcile_columns();
        }
        layer
    }

    /// Stores the layer on `floor`; an empty layer clears the slot.
    pub fn store(&self, floor: &mut Floor) {
        // Records this build cannot read stay in the slot (QA-28).
        floor.schedules =
            crate::foreign::layer_slot(self, self.is_empty(), floor.schedules.as_ref());
    }
}

// ===================================================================
// Schedule defaults and custom categories
// ===================================================================

/// A custom schedule category (Tools > Schedules > Manage Custom Schedule
/// Categories) and the objects assigned to it. Objects are named by their
/// [`crate::props::PropKey`] string (`door:12`, `cabinet:7`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomCategory {
    pub name: String,
    pub members: Vec<String>,
}

/// The category id of custom category `name` in [`Schedule::categories`].
pub fn custom_category_id(name: &str) -> String {
    format!("Custom/{name}")
}

/// Plan-wide schedule settings: the Schedule Defaults of each kind and the
/// custom categories. Stored in `Project.schedule_setup`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScheduleSetup {
    /// One template per kind the user changed in Default Settings > Schedules.
    pub defaults: Vec<Schedule>,
    pub categories: Vec<CustomCategory>,
}

impl ScheduleSetup {
    pub fn is_default(&self) -> bool {
        self.defaults.is_empty() && self.categories.is_empty()
    }

    /// A new schedule of `kind` as the Schedule Defaults make it (id and
    /// position are the caller's).
    pub fn template(&self, kind: ScheduleKind, position: Point) -> Schedule {
        match self.defaults.iter().find(|d| d.kind == kind) {
            Some(d) => {
                let mut s = d.clone();
                s.id = 0;
                s.position = position;
                s.numbers.clear();
                s.numbers_recorded = false;
                s.rooms.clear();
                s.reconcile_columns();
                s
            }
            None => Schedule::new(kind, position),
        }
    }

    /// Stores `def` as the default of its kind.
    pub fn set_default(&mut self, mut def: Schedule) {
        def.id = 0;
        def.position = Point::ZERO;
        def.numbers.clear();
        def.numbers_recorded = false;
        def.rooms.clear();
        match self.defaults.iter_mut().find(|d| d.kind == def.kind) {
            Some(slot) => *slot = def,
            None => self.defaults.push(def),
        }
    }

    /// Forgets the default of `kind` (back to Chief's own).
    pub fn reset_default(&mut self, kind: ScheduleKind) -> bool {
        let n = self.defaults.len();
        self.defaults.retain(|d| d.kind != kind);
        self.defaults.len() != n
    }

    pub fn category(&self, name: &str) -> Option<&CustomCategory> {
        self.categories.iter().find(|c| c.name == name)
    }

    /// Makes a custom category. Names are short and unique.
    pub fn add_category(&mut self, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Type a name for the category".into());
        }
        if self.category(name).is_some() {
            return Err(format!("There is already a category named \"{name}\""));
        }
        self.categories.push(CustomCategory {
            name: name.to_string(),
            members: Vec::new(),
        });
        Ok(())
    }

    /// Renames a category; the schedules that ticked it follow when the
    /// caller runs [`ScheduleSetup::rename_in`] over them.
    pub fn rename_category(&mut self, old: &str, new: &str) -> Result<(), String> {
        let new = new.trim();
        if new.is_empty() {
            return Err("Type a name for the category".into());
        }
        if old != new && self.category(new).is_some() {
            return Err(format!("There is already a category named \"{new}\""));
        }
        match self.categories.iter_mut().find(|c| c.name == old) {
            Some(c) => {
                c.name = new.to_string();
                Ok(())
            }
            None => Err(format!("No category named \"{old}\"")),
        }
    }

    /// Moves a schedule's tick of category `old` to `new` after a rename.
    pub fn rename_in(def: &mut Schedule, old: &str, new: &str) {
        if let Some(on) = def.categories.remove(&custom_category_id(old)) {
            def.categories.insert(custom_category_id(new), on);
        }
    }

    pub fn delete_category(&mut self, name: &str) -> bool {
        let n = self.categories.len();
        self.categories.retain(|c| c.name != name);
        self.categories.len() != n
    }

    /// Lists the object `key` in category `name`.
    pub fn assign(&mut self, name: &str, key: &str) -> bool {
        match self.categories.iter_mut().find(|c| c.name == name) {
            Some(c) => {
                if !c.members.iter().any(|m| m == key) {
                    c.members.push(key.to_string());
                }
                true
            }
            None => false,
        }
    }

    pub fn unassign(&mut self, name: &str, key: &str) -> bool {
        match self.categories.iter_mut().find(|c| c.name == name) {
            Some(c) => {
                let n = c.members.len();
                c.members.retain(|m| m != key);
                c.members.len() != n
            }
            None => false,
        }
    }

    /// The custom categories object `key` is assigned to ("Include in
    /// Schedule As").
    pub fn categories_of<'a>(&'a self, key: &str) -> Vec<&'a str> {
        self.categories
            .iter()
            .filter(|c| c.members.iter().any(|m| m == key))
            .map(|c| c.name.as_str())
            .collect()
    }

    /// Drops members for which `alive` is false (deleted objects).
    pub fn purge(&mut self, alive: impl Fn(&str) -> bool) {
        for c in &mut self.categories {
            c.members.retain(|m| alive(m));
        }
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

    #[test]
    fn schedule_numbers_follow_the_start_style_and_leading_zero() {
        let mut s = Schedule::new(ScheduleKind::Door, Point::ZERO);
        assert_eq!(s.mark_text(1), "D01");
        assert_eq!(s.mark_text(12), "D12");
        s.label.start_number = 5;
        assert_eq!(s.mark_text(1), "D05");
        s.label.leading_zeros = false;
        assert_eq!(s.mark_text(1), "D5");
        s.label.number_style = NumberStyle::UpperAlpha;
        assert_eq!(s.mark_text(1), "DE");
        assert_eq!(s.mark_text(22), "DZ");
        assert_eq!(s.mark_text(23), "DAA");
        s.label.number_style = NumberStyle::LowerRoman;
        s.label.start_number = 1;
        assert_eq!(s.mark_text(3), "Diii");
        s.label.number_style = NumberStyle::UpperRoman;
        assert_eq!(s.mark_text(9), "DIX");
    }

    #[test]
    fn area_and_volume_columns_total_by_default_in_the_three_schedules() {
        for (kind, field, on) in [
            (ScheduleKind::Door, "area", true),
            (ScheduleKind::Window, "area", true),
            (ScheduleKind::RoomFinish, "area", true),
            (ScheduleKind::RoomFinish, "volume", true),
            (ScheduleKind::Room, "area", false),
            (ScheduleKind::Wall, "area", false),
            (ScheduleKind::Door, "width", false),
        ] {
            let c = kind
                .default_columns()
                .into_iter()
                .find(|c| c.field == field)
                .unwrap();
            assert_eq!(c.calc_total, on, "{kind:?} {field}");
        }
        assert!(ScheduleKind::Door.has_totals_row());
        assert!(!ScheduleKind::Cabinet.has_totals_row());
        assert_eq!(ScheduleKind::Window.num_kind("area"), Some(NumKind::Area));
        assert_eq!(
            ScheduleKind::Window.num_kind("width"),
            Some(NumKind::Length)
        );
        assert_eq!(ScheduleKind::Window.num_kind("type"), None);
        assert_eq!(
            ScheduleKind::Cabinet.num_kind("quantity"),
            Some(NumKind::Count)
        );
        assert!(ScheduleKind::Door.has_previews());
        assert!(!ScheduleKind::Framing.has_previews());
        assert!(is_preview_field("symbol_2d"));
        assert!(!is_preview_field("width"));
    }

    #[test]
    fn floors_and_rooms_scope_a_schedule() {
        let mut s = Schedule::new(ScheduleKind::Door, Point::ZERO);
        // This floor: the one it is placed on.
        assert!(s.lists_floor(2, 2) && !s.lists_floor(1, 2));
        // Chosen floors.
        s.floors = vec![0, 1];
        assert!(s.lists_floor(0, 2) && s.lists_floor(1, 2) && !s.lists_floor(2, 2));
        // All floors wins.
        s.floor_scope = FloorScope::All;
        assert!(s.lists_floor(5, 0));
        assert!(!s.has_scope());
        s.floor_scope = FloorScope::ThisFloor;
        assert!(s.has_scope());
        s.floors.clear();
        s.rooms.push(RoomRef::at(0, Point::new(10.0, 20.0)));
        assert!(s.has_scope());
        assert_eq!(s.rooms_on(0).count(), 1);
        assert_eq!(s.rooms_on(1).count(), 0);
        assert_eq!(s.rooms[0].point(), Point::new(10.0, 20.0));
    }

    #[test]
    fn categories_default_by_the_caller_and_record_the_choice() {
        let mut s = Schedule::new(ScheduleKind::Wall, Point::ZERO);
        assert!(s.category_on("Wall/Siding", true));
        assert!(!s.category_on("Custom/Glazing", false));
        s.set_category("Wall/Siding", false);
        s.set_category("Custom/Glazing", true);
        assert!(!s.category_on("Wall/Siding", true));
        assert!(s.category_on("Custom/Glazing", false));
        // Wall and Room schedules take new types; Note schedules do not.
        assert!(Schedule::new(ScheduleKind::Wall, Point::ZERO).new_types_included);
        assert!(Schedule::new(ScheduleKind::Room, Point::ZERO).new_types_included);
        assert!(!Schedule::new(ScheduleKind::Note, Point::ZERO).new_types_included);
        s.set_kind(ScheduleKind::Note);
        assert!(s.categories.is_empty(), "a new kind forgets the ticks");
        assert!(!s.new_types_included);
    }

    #[test]
    fn the_schedule_setup_keeps_defaults_and_custom_categories() {
        let mut setup = ScheduleSetup::default();
        assert!(setup.is_default());
        // Defaults: a window schedule without a border, numbers and rooms of
        // the template are not inherited.
        let mut d = Schedule::new(ScheduleKind::Window, Point::new(5.0, 5.0));
        d.border = false;
        d.id = 9;
        d.rooms.push(RoomRef::at(0, Point::ZERO));
        d.numbers.push(NumRec {
            kind: ScheduleKind::Window,
            floor: 0,
            id: 3,
            n: 1,
        });
        setup.set_default(d);
        let t = setup.template(ScheduleKind::Window, Point::new(40.0, 60.0));
        assert!(!t.border);
        assert_eq!(t.id, 0);
        assert_eq!(t.position, Point::new(40.0, 60.0));
        assert!(t.rooms.is_empty() && t.numbers.is_empty());
        assert!(setup.template(ScheduleKind::Door, Point::ZERO).border);
        assert!(setup.reset_default(ScheduleKind::Window));
        assert!(!setup.reset_default(ScheduleKind::Window));
        // Custom categories.
        assert!(setup.add_category("Glazing").is_ok());
        assert!(setup.add_category(" Glazing ").is_err());
        assert!(setup.add_category("").is_err());
        assert!(setup.assign("Glazing", "window:4"));
        assert!(!setup.assign("Nope", "window:4"));
        assert!(setup.assign("Glazing", "door:2"));
        assert!(setup.assign("Glazing", "door:2"), "twice is once");
        assert_eq!(setup.category("Glazing").unwrap().members.len(), 2);
        assert_eq!(setup.categories_of("door:2"), ["Glazing"]);
        assert!(setup.unassign("Glazing", "door:2"));
        assert!(setup.categories_of("door:2").is_empty());
        assert!(setup.rename_category("Glazing", "Glass").is_ok());
        assert!(setup.rename_category("Glazing", "x").is_err());
        let mut sched = Schedule::new(ScheduleKind::Door, Point::ZERO);
        sched.set_category(&custom_category_id("Glazing"), true);
        ScheduleSetup::rename_in(&mut sched, "Glazing", "Glass");
        assert!(sched.category_on(&custom_category_id("Glass"), false));
        setup.purge(|k| k != "window:4");
        assert!(setup.category("Glass").unwrap().members.is_empty());
        assert!(setup.delete_category("Glass"));
        assert!(setup.is_default());
    }

    #[test]
    fn a_schedule_saved_before_round_16_loads_with_the_new_settings_defaulted() {
        let old = r#"{"id":4,"kind":"Door","position":{"x":1.0,"y":2.0},
            "columns":[{"field":"mark","title":"Mark","visible":true,"width":0.0}],
            "title":"Doors"}"#;
        let s: Schedule = serde_json::from_str(old).unwrap();
        assert_eq!(s.id, 4);
        assert!(s.show_title && s.show_headings && s.border && s.grid_lines);
        assert!(!s.swap && !s.wrap.enabled && !s.group_similar);
        assert!(s.totals_row);
        assert_eq!(s.totals_label, "Totals");
        assert_eq!(s.columns[0].format, None);
        assert!(!s.columns[0].calc_total);
        assert!(s.numbers.is_empty() && s.rooms.is_empty() && s.categories.is_empty());
        // And the new settings round-trip.
        let mut t = Schedule::new(ScheduleKind::Window, Point::ZERO);
        t.swap = true;
        t.wrap.enabled = true;
        t.wrap.by = WrapBy::MaxSize(120.0);
        t.rooms.push(RoomRef::at(1, Point::new(3.0, 4.0)));
        t.columns[1].format = Some(NumFormat::default());
        t.columns[1].calc_total = true;
        t.label.shape = Some(CalloutShape::Octagon);
        t.label.layer = CalloutLayer::Custom("Labels".into());
        t.set_category("Wall/Siding", false);
        let back: Schedule = serde_json::from_value(serde_json::to_value(&t).unwrap()).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn callouts_come_from_every_schedule_that_shows_them() {
        let mut layer = ScheduleLayer::default();
        layer.add(Schedule::new(ScheduleKind::Door, Point::ZERO));
        let mut second = Schedule::new(ScheduleKind::Door, Point::ZERO);
        second.label_prefix = "FD".into();
        layer.add(second);
        let mut hidden = Schedule::new(ScheduleKind::Door, Point::ZERO);
        hidden.show_labels = false;
        layer.add(hidden);
        assert_eq!(layer.label_sources(ScheduleKind::Door).count(), 2);
        assert_eq!(
            layer.label_source(ScheduleKind::Door).unwrap().label_prefix,
            "D"
        );
        assert_eq!(layer.label_sources(ScheduleKind::Window).count(), 0);
        let layer_of = CalloutLayer::Schedule;
        assert_eq!(layer_of.layer_of(&layer.schedules[0]), Some("Schedules"));
        assert_eq!(
            CalloutLayer::ObjectLabel.layer_of(&layer.schedules[0]),
            None
        );
    }
}
