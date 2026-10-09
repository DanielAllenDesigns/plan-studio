//! Door and window extensions for Chief parity (`docs/parity/doors-windows.md`).
//!
//! * DW-31..DW-33: swing side and hinge end are independent
//!   (`Opening::swing_flipped` = swings to the other side of the wall,
//!   `Opening::hinge_at_end` = hinge on the wall-end jamb).
//! * DW-38..DW-58: the opening style ([`OpeningStyle`]); plan symbols are in
//!   [`crate::opening_symbol`].
//! * DW-59..DW-63: plan label ([`LabelSettings`]), size shorthand and
//!   schedule number.
//! * DW-31..DW-37 editing: [`Project::flip_swing`], [`Project::flip_hinge`],
//!   [`Project::slide_opening`].
//! * DW-24, DW-26..DW-28, DW-51, DW-52: [`Project::resize_opening`],
//!   [`Project::center_opening`], [`Project::mull_openings`] and
//!   [`Project::unmull_openings`].

use crate::geometry::{point_in_polygon, Point};
use crate::model::{Id, Opening, OpeningKind, Project, Wall, WallKind};
use crate::rooms::Room;
use crate::units::fmt_ft_in;
use serde::{Deserialize, Serialize};

pub mod bay;
pub mod mull;
pub mod placement;
pub mod spec;
pub mod types;
pub use bay::{BayUnit, MIN_UNIT_WIDTH};
pub use mull::{MulledArrangement, MulledLabel, MulledSpec};
pub use spec::{
    door_panel_count, Arch, ArchType, BayRoof, BayRoofKind, CasingProfile, ExteriorSill,
    HandleStyle, Hardware, Lintel, LintelStyle, LiteStyle, OpeningSpec, OpeningView3d, RecessTo,
    ShutterSides, ShutterStyle, Shutters, StandardWidths, StyleWidths,
};
pub use types::{
    copy_group, group_eq, release_edited_groups, DefaultKey, DynGroup, OpenMode, TypeDefault,
    UseDefault, WindowType,
};

/// Minimum clear distance between an opening jamb and a wall end or another
/// opening (same as `Project::add_opening`).
const OPENING_MARGIN: f64 = 2.0;
/// Narrowest an opening can be resized to, inches.
pub const MIN_OPENING_WIDTH: f64 = 6.0;
/// The shortest transom, inches.
pub const MIN_TRANSOM_HEIGHT: f64 = 6.0;
/// Windows farther apart than this cannot be mulled, inches.
pub const MULL_MAX_GAP: f64 = 12.0;
/// The door styles that can be mulled with sidelite windows (DW-52).
pub const MULL_DOOR_STYLES: [OpeningStyle; 4] = [
    OpeningStyle::Hinged,
    OpeningStyle::DoubleDoor,
    OpeningStyle::Doorway,
    OpeningStyle::Fixed,
];

/// Drawing/behaviour style of an opening (DW-38..DW-58).
///
/// `Window` is the single/double hung window; `Fixed` and `Sliding` serve
/// doors and windows alike (the opening's kind picks the symbol).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OpeningStyle {
    #[default]
    Hinged,
    Sliding,
    Pocket,
    Bifold,
    Garage,
    Doorway,
    Barn,
    Shower,
    Fixed,
    Window,
    BayWindow,
    BowWindow,
    BoxWindow,
    PassThrough,
    WallNiche,
    /// Two hinged leaves (double or French door).
    DoubleDoor,
    /// Hinged sash with swing arcs in plan.
    Casement,
    /// Sliding window: two overlapping sashes.
    SlidingWindow,
    /// Sash hinged at the head, opening outward.
    Awning,
    /// Sash hinged at the sill, opening inward.
    Hopper,
}

impl OpeningStyle {
    /// Every door style, in flyout order.
    pub const DOORS: [OpeningStyle; 10] = [
        OpeningStyle::Hinged,
        OpeningStyle::DoubleDoor,
        OpeningStyle::Doorway,
        OpeningStyle::Sliding,
        OpeningStyle::Pocket,
        OpeningStyle::Bifold,
        OpeningStyle::Barn,
        OpeningStyle::Fixed,
        OpeningStyle::Garage,
        OpeningStyle::Shower,
    ];

    /// Every window style, in flyout order.
    pub const WINDOWS: [OpeningStyle; 11] = [
        OpeningStyle::Window,
        OpeningStyle::Casement,
        OpeningStyle::Fixed,
        OpeningStyle::SlidingWindow,
        OpeningStyle::Awning,
        OpeningStyle::Hopper,
        OpeningStyle::BayWindow,
        OpeningStyle::BowWindow,
        OpeningStyle::BoxWindow,
        OpeningStyle::PassThrough,
        OpeningStyle::WallNiche,
    ];

    /// The styles offered for an opening of `kind`.
    pub fn for_kind_list(kind: OpeningKind) -> &'static [OpeningStyle] {
        match kind {
            OpeningKind::Door => &Self::DOORS,
            OpeningKind::Window => &Self::WINDOWS,
        }
    }

    /// The style a new opening of `kind` starts with.
    pub fn default_for(kind: OpeningKind) -> OpeningStyle {
        match kind {
            OpeningKind::Door => OpeningStyle::Hinged,
            OpeningKind::Window => OpeningStyle::Window,
        }
    }

    /// Whether this style belongs to doors (as opposed to windows/niches).
    pub fn is_door_style(self) -> bool {
        matches!(
            self,
            OpeningStyle::Hinged
                | OpeningStyle::Sliding
                | OpeningStyle::Pocket
                | OpeningStyle::Bifold
                | OpeningStyle::Garage
                | OpeningStyle::Doorway
                | OpeningStyle::Barn
                | OpeningStyle::Shower
                | OpeningStyle::DoubleDoor
        )
    }

    /// Bay, bow and box windows project outside the wall.
    pub fn projects(self) -> bool {
        matches!(
            self,
            OpeningStyle::BayWindow | OpeningStyle::BowWindow | OpeningStyle::BoxWindow
        )
    }

    /// Chief's name of the style as a tool and in schedules.
    pub fn name(self, kind: OpeningKind) -> &'static str {
        match (self, kind) {
            (OpeningStyle::Hinged, _) => "Hinged Door",
            (OpeningStyle::Sliding, OpeningKind::Door) => "Sliding Door",
            (OpeningStyle::Sliding, OpeningKind::Window) => "Sliding Window",
            (OpeningStyle::Pocket, _) => "Pocket Door",
            (OpeningStyle::Bifold, _) => "Bifold Door",
            (OpeningStyle::Garage, _) => "Garage Door",
            (OpeningStyle::Doorway, _) => "Doorway",
            (OpeningStyle::Barn, _) => "Barn Door",
            (OpeningStyle::Shower, _) => "Shower Door",
            (OpeningStyle::Fixed, OpeningKind::Door) => "Fixed Door",
            (OpeningStyle::Fixed, OpeningKind::Window) => "Fixed Window",
            (OpeningStyle::Window, _) => "Window",
            (OpeningStyle::BayWindow, _) => "Bay Window",
            (OpeningStyle::BowWindow, _) => "Bow Window",
            (OpeningStyle::BoxWindow, _) => "Box Window",
            (OpeningStyle::PassThrough, _) => "Pass-Through",
            (OpeningStyle::WallNiche, _) => "Wall Niche",
            (OpeningStyle::DoubleDoor, _) => "Double Door",
            (OpeningStyle::Casement, _) => "Casement Window",
            (OpeningStyle::SlidingWindow, _) => "Sliding Window",
            (OpeningStyle::Awning, _) => "Awning Window",
            (OpeningStyle::Hopper, _) => "Hopper Window",
        }
    }
}

/// A default size for one style (the Default Settings of each door and
/// window tool); `None` keeps the plain door or window default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariantSize {
    pub style: OpeningStyle,
    pub width: f64,
    pub height: Option<f64>,
    pub sill_height: Option<f64>,
}

/// Default sizes of the door and window variants (DW-6, DW-46).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OpeningVariantDefaults {
    pub sizes: Vec<VariantSize>,
    /// The Defaults dialog of each door and window type (DW-124, DW-154):
    /// the opening a type places, looks and all. See [`types`].
    pub types: Vec<types::TypeDefault>,
    /// Counts the changes to `types`, so an opening using a default knows when
    /// to look again.
    pub revision: u32,
    /// The revision the placed openings were last brought up to (not saved).
    #[serde(skip)]
    pub followed: types::Followed,

    /// Manufacturer widths per style and the snap-to-standard-widths option
    /// of the jamb handles (DW-27).
    pub widths: StandardWidths,
    /// Tab values (sash, lintel, arch, hardware, shutters...) new doors and
    /// windows start with.
    pub door_spec: OpeningSpec,
    pub window_spec: OpeningSpec,
}

impl Default for OpeningVariantDefaults {
    fn default() -> Self {
        let v = |style, width, height: Option<f64>, sill: Option<f64>| VariantSize {
            style,
            width,
            height,
            sill_height: sill,
        };
        Self {
            widths: StandardWidths::default(),
            door_spec: OpeningSpec::default(),
            window_spec: OpeningSpec::default(),
            types: Vec::new(),
            revision: 0,
            followed: types::Followed::default(),
            sizes: vec![
                v(OpeningStyle::Doorway, 36.0, None, None),
                v(OpeningStyle::DoubleDoor, 60.0, None, None),
                v(OpeningStyle::Sliding, 72.0, None, None),
                v(OpeningStyle::Pocket, 30.0, None, None),
                v(OpeningStyle::Bifold, 48.0, None, None),
                v(OpeningStyle::Barn, 36.0, None, None),
                v(OpeningStyle::Garage, 108.0, Some(96.0), None),
                v(OpeningStyle::Shower, 28.0, Some(72.0), None),
                v(OpeningStyle::SlidingWindow, 60.0, Some(48.0), Some(36.0)),
                v(OpeningStyle::Awning, 36.0, Some(24.0), Some(60.0)),
                v(OpeningStyle::Hopper, 36.0, Some(18.0), Some(72.0)),
                v(OpeningStyle::BayWindow, 50.0, Some(60.0), None),
                v(OpeningStyle::BowWindow, 70.0, Some(60.0), None),
                v(OpeningStyle::BoxWindow, 50.0, Some(60.0), None),
                v(OpeningStyle::PassThrough, 48.0, Some(36.0), Some(42.0)),
                v(OpeningStyle::WallNiche, 24.0, Some(36.0), Some(36.0)),
            ],
        }
    }
}

impl OpeningVariantDefaults {
    /// The size entry of `style`, when it has one.
    pub fn size_for(&self, style: OpeningStyle) -> Option<&VariantSize> {
        self.sizes.iter().find(|s| s.style == style)
    }

    /// `template` made into an opening of `style`: the style's own size where
    /// the defaults have one, the template's otherwise.
    pub fn apply(&self, template: &Opening, style: OpeningStyle) -> Opening {
        let mut o = template.clone();
        o.style = style;
        if o.extras.spec == OpeningSpec::default() {
            o.extras.spec = match o.kind {
                OpeningKind::Door => self.door_spec.clone(),
                OpeningKind::Window => self.window_spec.clone(),
            };
        }
        if let Some(s) = self.size_for(style) {
            o.width = s.width;
            if let Some(h) = s.height {
                o.height = h;
            }
            if let Some(sill) = s.sill_height {
                o.sill_height = sill;
            }
            // Defaults saved before the manual's sizes carry the old stock
            // widths of the projecting windows; those read as unset.
            if style.projects() && (s.width - bay::legacy_unit_width(style)).abs() < 1e-9 {
                o.width = bay::default_unit_width(style);
            }
        }
        // A bay, box or bow window starts with the angle, depth and sections
        // of its style.
        if style.projects() && o.extras.spec.bay == bay::BayUnit::default() {
            o.extras.spec.bay = bay::BayUnit::for_style(style);
        }
        o
    }
}

// ----- labels (DW-59..DW-63) -----

/// Order of the numbers in an automatic size label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SizeFormat {
    #[default]
    WidthHeight,
    HeightWidth,
    WidthOnly,
}

/// How a size is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SizeStyle {
    /// Feet and inches run together: `3068`.
    #[default]
    Shorthand,
    /// `2'-6" x 6'-8"`.
    Architectural,
}

/// Where a label's text comes from (Default Settings > Door/Window Labels).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LabelMode {
    #[default]
    Automatic,
    /// The custom text (with macros) of the settings, or the opening's own.
    Custom,
    Suppress,
}

/// Where the label sits relative to the opening.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LabelPlacement {
    /// Centered over the opening, on the wall.
    Center,
    /// Beside the wall on the room side.
    #[default]
    Interior,
    /// Beside the wall on the outside.
    Exterior,
}

/// How a door or window label reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LabelSettings {
    pub mode: LabelMode,
    /// Custom text for [`LabelMode::Custom`]: macros `%automatic_label%`,
    /// `%schedule_number%`, `%width%`, `%height%` and `%type%`.
    pub custom_text: String,
    pub size_format: SizeFormat,
    pub size_style: SizeStyle,
    /// Show the schedule mark instead of the size when there is one.
    pub include_schedule_number: bool,
    /// Put the style name before the label.
    pub include_type: bool,
    pub placement: LabelPlacement,
    /// Draw the label in plan views at all.
    pub display_in_plan: bool,
}

impl Default for LabelSettings {
    fn default() -> Self {
        Self {
            mode: LabelMode::Automatic,
            custom_text: String::new(),
            size_format: SizeFormat::WidthHeight,
            size_style: SizeStyle::Shorthand,
            include_schedule_number: true,
            include_type: false,
            placement: LabelPlacement::Interior,
            display_in_plan: true,
        }
    }
}

/// Default label settings of doors and windows.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OpeningLabelDefaults {
    pub door: LabelSettings,
    pub window: LabelSettings,
}

impl OpeningLabelDefaults {
    pub fn for_kind(&self, kind: OpeningKind) -> &LabelSettings {
        match kind {
            OpeningKind::Door => &self.door,
            OpeningKind::Window => &self.window,
        }
    }
}

/// Casing around an opening (DW defaults: width, depth, reveal), inches.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Casing {
    pub width: f64,
    pub depth: f64,
    pub reveal: f64,
}

impl Default for Casing {
    fn default() -> Self {
        Self {
            width: 3.5,
            depth: 0.75,
            reveal: 0.25,
        }
    }
}

pub(crate) fn default_lites() -> (u32, u32) {
    (1, 1)
}

/// On-disk shape of [`Opening`]; fills `style` from the kind when absent so
/// old files get `Window` for windows, not the door default.
#[derive(Deserialize)]
pub(crate) struct OpeningDe {
    id: Id,
    wall_id: Id,
    center_offset: f64,
    width: f64,
    height: f64,
    sill_height: f64,
    kind: OpeningKind,
    #[serde(default)]
    swing_flipped: bool,
    #[serde(default)]
    hinge_at_end: bool,
    #[serde(default)]
    style: Option<OpeningStyle>,
    #[serde(default)]
    label_override: Option<String>,
    #[serde(default)]
    schedule_number: Option<String>,
    #[serde(default)]
    casing: Option<Casing>,
    #[serde(default = "default_lites")]
    lites: (u32, u32),
    #[serde(default)]
    egress: bool,
    #[serde(default)]
    tempered: bool,
    #[serde(default)]
    extras: crate::extras::OpeningExtras,
    #[serde(default)]
    mull_group: Option<Id>,
}

impl From<OpeningDe> for Opening {
    fn from(d: OpeningDe) -> Opening {
        Opening {
            id: d.id,
            wall_id: d.wall_id,
            center_offset: d.center_offset,
            width: d.width,
            height: d.height,
            sill_height: d.sill_height,
            kind: d.kind,
            swing_flipped: d.swing_flipped,
            hinge_at_end: d.hinge_at_end,
            style: d.style.unwrap_or_else(|| OpeningStyle::default_for(d.kind)),
            label_override: d.label_override,
            schedule_number: d.schedule_number,
            casing: d.casing,
            lites: d.lites,
            egress: d.egress,
            tempered: d.tempered,
            extras: d.extras,
            mull_group: d.mull_group,
        }
    }
}

/// Feet and inches concatenated, rounding down to a whole inch (DW-59):
/// 36" is `30`, 80" is `68`, 32" is `28`.
pub fn size_shorthand(inches: f64) -> String {
    let total = (inches + 1e-6).floor().max(0.0) as i64;
    format!("{}{}", total / 12, total % 12)
}

/// A width and height written as `format` and `style` say (DW-59).
pub fn size_text(width: f64, height: f64, format: SizeFormat, style: SizeStyle) -> String {
    let one = |v: f64| match style {
        SizeStyle::Shorthand => size_shorthand(v),
        SizeStyle::Architectural => fmt_ft_in(v),
    };
    let join = |a: String, b: String| match style {
        SizeStyle::Shorthand => format!("{a}{b}"),
        SizeStyle::Architectural => format!("{a} x {b}"),
    };
    match format {
        SizeFormat::WidthHeight => join(one(width), one(height)),
        SizeFormat::HeightWidth => join(one(height), one(width)),
        SizeFormat::WidthOnly => one(width),
    }
}

impl Opening {
    pub fn new(
        wall_id: Id,
        center_offset: f64,
        kind: OpeningKind,
        width: f64,
        height: f64,
        sill_height: f64,
    ) -> Opening {
        Opening {
            id: 0,
            wall_id,
            center_offset,
            width,
            height,
            sill_height,
            kind,
            swing_flipped: false,
            hinge_at_end: false,
            style: OpeningStyle::default_for(kind),
            label_override: None,
            schedule_number: None,
            casing: None,
            lites: default_lites(),
            egress: false,
            tempered: false,
            extras: crate::extras::OpeningExtras::default(),
            mull_group: None,
        }
    }

    /// Chief's automatic label (DW-59): width then height as concatenated
    /// feet-and-inches digits, e.g. `3068` for a 36" x 80" door.
    pub fn auto_label(&self) -> String {
        size_text(
            self.width,
            self.height,
            SizeFormat::WidthHeight,
            SizeStyle::Shorthand,
        )
    }

    /// The label to draw: the user's override (DW-62) or the automatic one.
    pub fn label(&self) -> String {
        self.label_override
            .clone()
            .unwrap_or_else(|| self.auto_label())
    }

    /// The name of this opening's style (`Hinged Door`, `Bay Window`).
    pub fn type_name(&self) -> &'static str {
        self.style.name(self.kind)
    }

    /// The label settings in force: the opening's own, else `defaults`.
    pub fn label_settings<'a>(&'a self, defaults: &'a OpeningLabelDefaults) -> &'a LabelSettings {
        self.extras
            .label
            .as_ref()
            .unwrap_or_else(|| defaults.for_kind(self.kind))
    }

    /// The text of the plan label (DW-59..DW-63), or `None` when the label is
    /// suppressed or hidden in plan. `mark` is the schedule mark (`D01`) when a
    /// schedule numbers this opening; without one the size is shown. The
    /// opening's own text (`label_override`) wins over the settings' custom
    /// text, and either may use the macros `%automatic_label%` (the size),
    /// `%schedule_number%` (the mark), `%width%`, `%height%` and `%type%`.
    pub fn plan_label(
        &self,
        defaults: &OpeningLabelDefaults,
        mark: Option<&str>,
    ) -> Option<String> {
        let st = self.label_settings(defaults);
        if st.mode == LabelMode::Suppress || !st.display_in_plan {
            return None;
        }
        let mark = mark.map(str::trim).filter(|m| !m.is_empty()).or(self
            .schedule_number
            .as_deref()
            .map(str::trim)
            .filter(|m| !m.is_empty()));
        let own = self
            .label_override
            .as_deref()
            .filter(|t| !t.trim().is_empty());
        let template = match (own, st.mode) {
            (Some(t), _) => t.to_string(),
            (None, LabelMode::Custom) if !st.custom_text.trim().is_empty() => {
                st.custom_text.clone()
            }
            _ if mark.is_some() && st.include_schedule_number => "%schedule_number%".into(),
            _ => "%automatic_label%".into(),
        };
        let size = size_text(self.width, self.height, st.size_format, st.size_style);
        let text = template
            .replace("%automatic_label%", &size)
            .replace("%schedule_number%", mark.unwrap_or(""))
            .replace("%width%", &fmt_ft_in(self.width))
            .replace("%height%", &fmt_ft_in(self.height))
            .replace("%type%", self.type_name());
        let text = if st.include_type && own.is_none() {
            format!("{} {}", self.type_name(), text)
        } else {
            text
        };
        let text = text.trim().to_string();
        (!text.is_empty()).then_some(text)
    }
}

impl Default for Opening {
    fn default() -> Self {
        Opening::new(0, 0.0, OpeningKind::Door, 36.0, 80.0, 0.0)
    }
}

/// Clamp `center` like `Project::add_opening`: the jambs stay at least the
/// margin away from the wall ends. `None` if the wall is too short.
pub fn clamp_opening_center(wall_len: f64, width: f64, center: f64) -> Option<f64> {
    if wall_len < width + 2.0 * OPENING_MARGIN {
        return None;
    }
    let half = width * 0.5;
    Some(center.clamp(half + OPENING_MARGIN, wall_len - half - OPENING_MARGIN))
}

/// Door placement defaults from the pointer (DW-8, DW-76): returns
/// `(swing_flipped, hinge_at_end)`. The door swings toward the side of the
/// wall the pointer is on (`swing_flipped` is false for the wall's left/normal
/// side, true for the right; a pointer on the centerline keeps the left), and
/// the hinge goes to the jamb nearer the closer wall end.
pub fn door_defaults_for_pointer(
    wall: &crate::model::Wall,
    pointer: Point,
    center_offset: f64,
) -> (bool, bool) {
    let (_, side) = wall.locate(pointer);
    (side < 0.0, center_offset > wall.path_length() * 0.5)
}

/// The side of `wall` that is the outside: `1.0` for the left (normal) side,
/// `-1.0` for the right. An exterior wall's room side is the interior; with no
/// room beside it the left face is the exterior. Interior walls report the
/// right side as "outside" so the left reads as the room, like the 3D view.
pub fn exterior_sign(wall: &Wall, rooms: &[Room]) -> f64 {
    if wall.kind == WallKind::Interior {
        return -1.0;
    }
    let s = wall.path_length() * 0.5;
    let mid = wall.point_along(s);
    let reach = wall.thickness * 0.5 + 1.0;
    let probe = mid + wall.normal_along(s) * reach;
    if rooms.iter().any(|r| point_in_polygon(probe, &r.polygon)) {
        -1.0
    } else {
        1.0
    }
}

/// Whether two openings share no height: one starts at or above the top of
/// the other, so they can stand over each other on the same stretch of wall
/// (a window over a door, DW-52).
pub fn vertically_apart(a: &Opening, b: &Opening) -> bool {
    let (a0, a1) = (a.sill_height, a.sill_height + a.height);
    let (b0, b1) = (b.sill_height, b.sill_height + b.height);
    a0 >= b1 - 1e-6 || b0 >= a1 - 1e-6
}

/// Whether `a` and `b` (on the same wall) get in each other's way: they come
/// closer than `margin` along the wall and share part of the wall face's
/// height. Openings stacked one over the other never do.
pub fn openings_conflict(a: &Opening, b: &Opening, margin: f64) -> bool {
    a.start_offset() < b.end_offset() + margin
        && a.end_offset() > b.start_offset() - margin
        && !vertically_apart(a, b)
}

/// Whether `upper` stands directly over `lower`: they share stretch of wall
/// and `upper` begins at or above the top of `lower`.
pub fn stands_over(upper: &Opening, lower: &Opening) -> bool {
    upper.id != lower.id
        && upper.wall_id == lower.wall_id
        && upper.start_offset() < lower.end_offset() - 1e-6
        && upper.end_offset() > lower.start_offset() + 1e-6
        && upper.sill_height >= lower.sill_height + lower.height - 1e-6
}

/// The member of `row` that `upper` stands over most (the widest overlap).
fn row_member_under<'a>(row: &'a [Opening], upper: &Opening) -> Option<&'a Opening> {
    row.iter().filter(|v| stands_over(upper, v)).max_by(|a, b| {
        let ov = |v: &Opening| {
            upper.end_offset().min(v.end_offset()) - upper.start_offset().max(v.start_offset())
        };
        ov(a).total_cmp(&ov(b))
    })
}

/// The curve of a wall as openings see it: positions along the wall are arc
/// lengths from its start, and a point `t` inches off the centerline is
/// radial (DW-88).
impl Wall {
    /// The centerline point `s` inches along the wall (arc length).
    pub fn point_along(&self, s: f64) -> Point {
        if self.is_curved() {
            self.frame_at(s).0
        } else {
            self.point_at(s)
        }
    }

    /// The unit direction of travel at `s`.
    pub fn tangent_along(&self, s: f64) -> Point {
        if self.is_curved() {
            self.frame_at(s).1
        } else {
            self.direction()
        }
    }

    /// The unit left normal at `s`.
    pub fn normal_along(&self, s: f64) -> Point {
        self.tangent_along(s).perp()
    }

    /// The point `s` along the wall and `t` to its left.
    pub fn point_offset(&self, s: f64, t: f64) -> Point {
        self.point_along(s).add(self.normal_along(s).scale(t))
    }

    /// Where `p` stands against the wall: `(s, t)`, the arc length along the
    /// centerline (clamped to the wall) and the signed distance to the left.
    pub fn locate(&self, p: Point) -> (f64, f64) {
        let len = self.path_length();
        if !self.is_curved() {
            let rel = p.sub(self.start);
            return (
                rel.dot(self.direction()).clamp(0.0, len),
                rel.dot(self.normal()),
            );
        }
        let (q, tangent) = self.closest_point(p);
        let s = match self.arc_center_radius() {
            Some((c, r)) => {
                let sweep = self.curve.map_or(0.0, |k| k.sweep(self.start, self.end));
                let a0 = self.start.sub(c).angle();
                let a = q.sub(c).angle();
                let round =
                    if sweep >= 0.0 { a - a0 } else { a0 - a }.rem_euclid(std::f64::consts::TAU);
                (round.min(sweep.abs()) * r).clamp(0.0, len)
            }
            None => 0.0,
        };
        (s, p.sub(q).dot(tangent.perp()))
    }

    /// The band between `t_lo` and `t_hi` off the centerline over `s0..s1` as
    /// convex quads `[hi start, hi end, lo end, lo start]`: one when the wall is
    /// straight, a strip following the arc when it is curved.
    pub fn band_quads(&self, s0: f64, s1: f64, t_lo: f64, t_hi: f64) -> Vec<[Point; 4]> {
        let n = if self.is_curved() {
            let sweep_deg = self.arc_readout().map_or(0.0, |a| a.sweep_deg);
            let share = (s1 - s0) / self.path_length().max(1e-9);
            ((sweep_deg * share / 4.0).ceil() as usize).clamp(1, 64)
        } else {
            1
        };
        let at = |i: usize| s0 + (s1 - s0) * i as f64 / n as f64;
        (0..n)
            .map(|i| {
                [
                    self.point_offset(at(i), t_hi),
                    self.point_offset(at(i + 1), t_hi),
                    self.point_offset(at(i + 1), t_lo),
                    self.point_offset(at(i), t_lo),
                ]
            })
            .collect()
    }

    /// Outline of the band between `t_lo` and `t_hi` off the centerline over
    /// `s0..s1`, following the arc of a curved wall (four corners when
    /// straight). Used to clear the wall fill and to highlight an opening.
    pub fn band(&self, s0: f64, s1: f64, t_lo: f64, t_hi: f64) -> Vec<Point> {
        let quads = self.band_quads(s0, s1, t_lo, t_hi);
        let mut out: Vec<Point> = quads.iter().map(|q| q[0]).collect();
        if let Some(last) = quads.last() {
            out.push(last[1]);
            out.push(last[2]);
        }
        out.extend(quads.iter().rev().map(|q| q[3]));
        out
    }
}

/// Which jamb of an opening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Jamb {
    /// The jamb nearer the wall start.
    Start,
    /// The jamb nearer the wall end.
    End,
}

impl Project {
    /// Reverse Swing (DW-32): the door opens to the other side of the wall,
    /// hinge unchanged. Returns `false` for an unknown opening.
    pub fn flip_swing(&mut self, floor: usize, id: Id) -> bool {
        match self.floors[floor].openings.iter_mut().find(|o| o.id == id) {
            Some(o) => {
                o.swing_flipped = !o.swing_flipped;
                true
            }
            None => false,
        }
    }

    /// Flip Hinge (DW-32): move the hinge to the other jamb, swing side
    /// unchanged. Returns `false` for an unknown opening.
    pub fn flip_hinge(&mut self, floor: usize, id: Id) -> bool {
        match self.floors[floor].openings.iter_mut().find(|o| o.id == id) {
            Some(o) => {
                o.hinge_at_end = !o.hinge_at_end;
                true
            }
            None => false,
        }
    }

    /// The openings that make up the mulled unit of `id` on its wall, ordered
    /// along the wall. A window on its own is its own unit.
    pub fn mull_members(&self, floor: usize, id: Id) -> Vec<Id> {
        let f = &self.floors[floor];
        let Some(cur) = f.openings.iter().find(|o| o.id == id) else {
            return Vec::new();
        };
        let Some(group) = cur.mull_group else {
            return vec![id];
        };
        let mut members: Vec<&Opening> = f
            .openings
            .iter()
            .filter(|o| o.wall_id == cur.wall_id && o.mull_group == Some(group))
            .collect();
        members.sort_by(|a, b| a.center_offset.total_cmp(&b.center_offset));
        members.into_iter().map(|o| o.id).collect()
    }

    /// The span `(start, end)` a unit of openings covers along its wall.
    pub fn unit_span(&self, floor: usize, id: Id) -> Option<(f64, f64)> {
        let f = &self.floors[floor];
        let ids = self.mull_members(floor, id);
        let spans = ids
            .iter()
            .filter_map(|i| f.openings.iter().find(|o| o.id == *i))
            .map(|o| (o.start_offset(), o.end_offset()));
        spans.reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)))
    }

    /// Slide an opening (or its whole mulled unit) along its wall so the
    /// opening's center is at `new_center`, clamped like
    /// [`Project::add_opening`]. Returns `false` (and changes nothing) if the
    /// opening is unknown, the wall is too short, or the new spot overlaps
    /// another opening.
    pub fn slide_opening(&mut self, floor: usize, id: Id, new_center: f64) -> bool {
        let f = &self.floors[floor];
        let Some(cur) = f.openings.iter().find(|o| o.id == id).cloned() else {
            return false;
        };
        let Some(wall_len) = f.wall(cur.wall_id).map(|w| w.path_length()) else {
            return false;
        };
        let members = self.mull_members(floor, id);
        let Some((lo, hi)) = self.unit_span(floor, id) else {
            return false;
        };
        if wall_len < (hi - lo) + 2.0 * OPENING_MARGIN {
            return false;
        }
        let delta = (new_center - cur.center_offset)
            .clamp(OPENING_MARGIN - lo, wall_len - OPENING_MARGIN - hi);
        // The shared placement rules (`placement`): a member keeps the end
        // clearance from its neighbours (windows may touch) and stays out of
        // the bodies of walls that meet the host. A member already inside a
        // zone may move out of it but not deeper in.
        let f = &self.floors[floor];
        let Some(host) = f.wall(cur.wall_id) else {
            return false;
        };
        let rules = placement::Rules::of(self);
        let overlaps = f
            .openings
            .iter()
            .filter(|m| members.contains(&m.id))
            .any(|m| {
                let (a0, a1) = (m.start_offset(), m.end_offset());
                placement::zones_skipping_with(f, host, m, &members, &rules)
                    .iter()
                    .any(|z| {
                        let ov = |a: f64, b: f64| (b.min(z.hi) - a.max(z.lo)).max(0.0);
                        ov(a0 + delta, a1 + delta) > ov(a0, a1) + 1e-6
                    })
            });
        if overlaps {
            return false;
        }
        for o in &mut self.floors[floor].openings {
            if members.contains(&o.id) {
                o.center_offset += delta;
            }
        }
        true
    }

    /// The free range `(lo, hi)` of wall offsets the opening may occupy: from
    /// the nearer of the wall end clearance and the previous opening, to the
    /// next opening or the far wall end. Members of the opening's own mulled
    /// unit do not limit it when `as_unit` (moving the whole unit); otherwise
    /// they limit it with no gap. `None` for an unknown opening or wall.
    pub fn free_span(&self, floor: usize, id: Id, as_unit: bool) -> Option<(f64, f64)> {
        let f = &self.floors[floor];
        let cur = f.openings.iter().find(|o| o.id == id)?;
        let len = f.wall(cur.wall_id)?.path_length();
        let (mut lo, mut hi) = (OPENING_MARGIN, len - OPENING_MARGIN);
        let rules = placement::Rules::of(self);
        let members = self.mull_members(floor, id);
        for o in f.openings_on(cur.wall_id) {
            if o.id == id {
                continue;
            }
            let member = members.contains(&o.id);
            if member && as_unit {
                continue;
            }
            // An opening stacked above or below does not limit this one.
            if vertically_apart(cur, o) {
                continue;
            }
            let gap = if member { 0.0 } else { rules.min_separation };
            if o.center_offset < cur.center_offset {
                lo = lo.max(o.end_offset() + gap);
            } else {
                hi = hi.min(o.start_offset() - gap);
            }
        }
        // An opening stops where its casing meets an intersecting wall
        // (manual p. 615) unless Ignore Casing for Opening Resize is on. One
        // already inside that reach may leave it but not go deeper.
        if let Some(host) = f.wall(cur.wall_id) {
            let (mut jlo, mut jhi) = (lo, hi);
            for z in placement::junction_zones_with(f, host, rules.junction_clearance(cur)) {
                if z.core_hi <= cur.center_offset {
                    jlo = jlo.max(z.hi);
                } else if z.core_lo >= cur.center_offset {
                    jhi = jhi.min(z.lo);
                }
            }
            lo = jlo.min(cur.start_offset().max(lo));
            hi = jhi.max(cur.end_offset().min(hi));
        }
        Some((lo, hi))
    }

    /// Resize handle (DW-26..DW-28): move one jamb of an opening to the wall
    /// offset `edge`, the other jamb staying put. The result is clamped to
    /// the minimum width, the wall end clearance and the neighbouring
    /// openings. Returns whether the opening changed.
    pub fn resize_opening(&mut self, floor: usize, id: Id, jamb: Jamb, edge: f64) -> bool {
        let Some((lo, hi)) = self.free_span(floor, id, false) else {
            return false;
        };
        let f = &mut self.floors[floor];
        let Some(o) = f.openings.iter_mut().find(|o| o.id == id) else {
            return false;
        };
        let (start, end) = (o.start_offset(), o.end_offset());
        let (new_start, new_end) = match jamb {
            Jamb::Start => (edge.clamp(lo, end - MIN_OPENING_WIDTH), end),
            Jamb::End => (start, edge.clamp(start + MIN_OPENING_WIDTH, hi)),
        };
        if new_end - new_start < MIN_OPENING_WIDTH - 1e-9 {
            return false;
        }
        let changed = (new_start - start).abs() > 1e-9 || (new_end - end).abs() > 1e-9;
        o.width = new_end - new_start;
        o.center_offset = (new_start + new_end) * 0.5;
        changed
    }

    /// Set the width of an opening (a typed width, DW-12, DW-102): about its
    /// center when both jambs have room, else about the jamb that has room.
    /// Returns `false` (and changes nothing) when it fits no way.
    pub fn set_opening_width(&mut self, floor: usize, id: Id, width: f64) -> bool {
        if width < MIN_OPENING_WIDTH - 1e-9 {
            return false;
        }
        let Some((lo, hi)) = self.free_span(floor, id, false) else {
            return false;
        };
        let Some(o) = self.floors[floor].openings.iter_mut().find(|o| o.id == id) else {
            return false;
        };
        let fits = |a: f64| a >= lo - 1e-9 && a + width <= hi + 1e-9;
        let centered = o.center_offset - width * 0.5;
        let start = if fits(centered) {
            centered
        } else if fits(o.start_offset()) {
            o.start_offset()
        } else if fits(o.end_offset() - width) {
            o.end_offset() - width
        } else {
            return false;
        };
        o.width = width;
        o.center_offset = start + width * 0.5;
        true
    }

    /// Center Object on the wall segment (DW-24): put the opening (or its
    /// unit) midway in the free span between its neighbours and the wall
    /// ends. Returns `false` when it already is, or does not fit.
    pub fn center_opening(&mut self, floor: usize, id: Id) -> bool {
        let Some((lo, hi)) = self.free_span(floor, id, true) else {
            return false;
        };
        let Some((a, b)) = self.unit_span(floor, id) else {
            return false;
        };
        if b - a > hi - lo + 1e-9 {
            return false;
        }
        let delta = (lo + hi) * 0.5 - (a + b) * 0.5;
        if delta.abs() < 1e-9 {
            return false;
        }
        let members = self.mull_members(floor, id);
        for o in &mut self.floors[floor].openings {
            if members.contains(&o.id) {
                o.center_offset += delta;
            }
        }
        true
    }

    /// Mull (DW-51, DW-52): join adjacent windows of one wall, or a door and
    /// the windows beside it, into a unit that moves together and shares a
    /// frame and casing. Gaps up to [`MULL_MAX_GAP`] are
    /// closed by moving the windows after the first toward it. Returns the
    /// group id.
    pub fn mull_openings(&mut self, floor: usize, ids: &[Id]) -> Result<Id, String> {
        let f = &self.floors[floor];
        let mut all: Vec<Id> = Vec::new();
        for id in ids {
            if !f.openings.iter().any(|o| o.id == *id) {
                return Err("That window is gone".into());
            }
            for m in self.mull_members(floor, *id) {
                if !all.contains(&m) {
                    all.push(m);
                }
            }
        }
        if all.len() < 2 {
            return Err("Select at least two openings to mull".into());
        }
        let mut units: Vec<Opening> = all
            .iter()
            .filter_map(|i| f.openings.iter().find(|o| o.id == *i).cloned())
            .collect();
        // Windows mull with each other, and a door can have windows beside
        // it (sidelites): one door per unit, and only a door with a swing or
        // a cased opening, not a garage, sliding, folding or barn door.
        let doors: Vec<&Opening> = units
            .iter()
            .filter(|o| o.kind == OpeningKind::Door)
            .collect();
        if doors.len() > 1 {
            return Err("Only one door can be in a mulled unit".into());
        }
        if doors.iter().any(|d| !MULL_DOOR_STYLES.contains(&d.style)) {
            return Err("That kind of door cannot be mulled with windows".into());
        }
        let wall_id = units[0].wall_id;
        if units.iter().any(|o| o.wall_id != wall_id) {
            return Err("The openings must be on the same wall".into());
        }
        units.sort_by(|a, b| a.center_offset.total_cmp(&b.center_offset));
        // A window standing over another member (a transom over a door) is
        // part of the unit without closing any gap: only the row at the
        // bottom is laid out along the wall.
        let (row, stacked): (Vec<Opening>, Vec<Opening>) = units
            .iter()
            .cloned()
            .partition(|u| !units.iter().any(|v| stands_over(u, v)));
        if stacked.iter().any(|u| {
            !row.iter().any(|v| {
                v.start_offset() < u.end_offset() - 1e-6 && v.end_offset() > u.start_offset() + 1e-6
            })
        }) {
            return Err("A window over a door has to stand over the unit".into());
        }
        let units = row;
        // Nothing but the chosen windows may sit between them.
        let (first, last) = (units[0].start_offset(), units[units.len() - 1].end_offset());
        if f.openings_on(wall_id).any(|o| {
            !all.contains(&o.id)
                && o.end_offset() > first - 1e-9
                && o.start_offset() < last + 1e-9
                && units.iter().any(|u| !vertically_apart(u, o))
        }) {
            return Err("Another opening is in between".into());
        }
        // Close the gaps, moving each window toward the first: a window moves
        // by all the gaps before it.
        let mut shifts: Vec<(Id, f64)> = Vec::new();
        let mut prev_end = units[0].end_offset();
        let mut total = 0.0;
        for u in &units[1..] {
            let gap = u.start_offset() - prev_end;
            if gap > MULL_MAX_GAP + 1e-9 {
                return Err(format!(
                    "The openings are too far apart to mull (over {} apart)",
                    fmt_ft_in(MULL_MAX_GAP)
                ));
            }
            if gap < -1e-9 {
                return Err("The openings overlap".into());
            }
            total += gap;
            shifts.push((u.id, -total));
            prev_end = u.end_offset();
        }
        // The windows over the row move with the member they stand over.
        let carried: Vec<(Id, f64)> = stacked
            .iter()
            .filter_map(|st| {
                let under = row_member_under(&units, st)?.id;
                shifts
                    .iter()
                    .find(|(id, _)| *id == under)
                    .map(|(_, d)| (st.id, *d))
            })
            .collect();
        shifts.extend(carried);
        let group = units[0].id;
        let mut p = self.floors[floor].openings.clone();
        for (id, d) in &shifts {
            if let Some(o) = p.iter_mut().find(|o| o.id == *id) {
                o.center_offset += d;
            }
        }
        for o in &mut p {
            if all.contains(&o.id) {
                o.mull_group = Some(group);
            }
        }
        self.floors[floor].openings = p;
        Ok(group)
    }

    /// Unmull (DW-51): split the unit `id` belongs to back into separate
    /// windows. Returns how many windows were released.
    pub fn unmull_openings(&mut self, floor: usize, id: Id) -> usize {
        let members = self.mull_members(floor, id);
        let mut n = 0;
        for o in &mut self.floors[floor].openings {
            if members.contains(&o.id) && o.mull_group.take().is_some() {
                n += 1;
            }
        }
        n
    }

    /// Add a transom (DW-52): a fixed window directly over the unit `id`
    /// belongs to, as wide as the whole unit, `height` inches tall (less if
    /// the wall has no room), mulled into it so the unit shares one frame
    /// and casing. Returns the new window.
    pub fn add_transom(&mut self, floor: usize, id: Id, height: f64) -> Result<Id, String> {
        let f = &self.floors[floor];
        let Some(cur) = f.openings.iter().find(|o| o.id == id).cloned() else {
            return Err("That opening is gone".into());
        };
        let Some(wall_h) = f.wall(cur.wall_id).map(|w| w.height) else {
            return Err("That opening has no wall".into());
        };
        let members = self.mull_members(floor, id);
        let in_unit: Vec<&Opening> = f
            .openings
            .iter()
            .filter(|o| members.contains(&o.id))
            .collect();
        if in_unit
            .iter()
            .any(|o| stands_over(o, &cur) || in_unit.iter().any(|v| stands_over(o, v)))
        {
            return Err("There is a window over it already".into());
        }
        let top = in_unit
            .iter()
            .map(|o| o.sill_height + o.height)
            .fold(0.0_f64, f64::max);
        let room = wall_h - top;
        if room < MIN_TRANSOM_HEIGHT - 1e-9 {
            return Err("There is no room above it for a transom".into());
        }
        let (lo, hi) = in_unit.iter().fold((f64::MAX, f64::MIN), |(l, h), o| {
            (l.min(o.start_offset()), h.max(o.end_offset()))
        });
        let before = self.floors[floor].openings.clone();
        let new_id = self.alloc_id();
        let mut win = Opening::default_window(new_id, cur.wall_id, (lo + hi) * 0.5);
        win.width = hi - lo;
        win.height = height.clamp(MIN_TRANSOM_HEIGHT, room);
        win.sill_height = top;
        win.style = OpeningStyle::Fixed;
        win.swing_flipped = cur.swing_flipped;
        win.casing = cur.casing;
        win.extras.spec.casing_interior = cur.extras.spec.casing_interior;
        win.extras.spec.casing_exterior = cur.extras.spec.casing_exterior;
        win.extras.spec.casing_exterior_size = cur.extras.spec.casing_exterior_size;
        win.extras.spec.casing_profile = cur.extras.spec.casing_profile;
        self.floors[floor].openings.push(win);
        match self.mull_openings(floor, &[id, new_id]) {
            Ok(_) => Ok(new_id),
            Err(e) => {
                self.floors[floor].openings = before;
                Err(e)
            }
        }
    }

    /// What the casing of opening `id` is drawn around: `None` when it is
    /// drawn by another opening of its unit (a window standing over a
    /// door), else the span of the unit it belongs to (`Some(None)` for an
    /// opening on its own).
    pub fn casing_unit(&self, floor: usize, id: Id) -> Option<Option<(f64, f64)>> {
        let f = &self.floors[floor];
        let o = f.openings.iter().find(|o| o.id == id)?;
        if o.mull_group.is_none() {
            // Windows and doors whose casings touch share one (manual p. 608).
            return Some(self.casing_span(floor, id));
        }
        let members = self.mull_members(floor, id);
        if f.openings
            .iter()
            .any(|v| members.contains(&v.id) && stands_over(o, v))
        {
            return None;
        }
        Some(
            self.casing_span(floor, id)
                .or_else(|| self.unit_span(floor, id)),
        )
    }

    /// Works out the depth of every opening recessed to a wall layer (Options
    /// panel, Recessed To Layer): from the exterior face to the exterior side of
    /// the main or the sheathing layer of the wall's type. An opening whose
    /// wall has no such layer keeps its depth. Returns how many changed.
    pub fn sync_recess_depths(&mut self) -> usize {
        let types = self.wall_types.clone();
        let mut changed = 0;
        for f in &mut self.floors {
            let depths: Vec<(Id, f64)> = f
                .openings
                .iter()
                .filter(|o| o.extras.spec.recess_to != RecessTo::Depth)
                .filter_map(|o| {
                    let wt = f.wall(o.wall_id)?.wall_type.as_ref()?;
                    let def = types.iter().find(|t| &t.name == wt)?;
                    let mut offset = 0.0;
                    for l in &def.layers {
                        let hit = match o.extras.spec.recess_to {
                            RecessTo::MainLayer => l.is_main,
                            RecessTo::SheathingLayer => {
                                l.name.to_ascii_lowercase().contains("sheath")
                            }
                            RecessTo::Depth => false,
                        };
                        if hit {
                            return Some((o.id, offset));
                        }
                        offset += l.thickness;
                    }
                    None
                })
                .collect();
            for (id, d) in depths {
                if let Some(o) = f.openings.iter_mut().find(|o| o.id == id) {
                    if o.extras.spec.recess_depth != Some(d) {
                        o.extras.spec.recess_depth = Some(d);
                        changed += 1;
                    }
                }
            }
        }
        changed
    }

    /// The plan position of the opening center on its wall centerline.
    pub fn opening_position(&self, floor: usize, id: Id) -> Option<Point> {
        let f = &self.floors[floor];
        let o = f.openings.iter().find(|o| o.id == id)?;
        Some(f.wall(o.wall_id)?.point_along(o.center_offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{WallKind, DEFAULT_CEILING_HEIGHT};

    fn proj() -> (Project, Id) {
        let mut p = Project::new("o");
        let w = p.add_wall(
            0,
            Point::ZERO,
            Point::new(200.0, 0.0),
            4.5,
            DEFAULT_CEILING_HEIGHT,
            WallKind::Interior,
        );
        (p, w)
    }

    #[test]
    fn auto_label_is_chief_shorthand() {
        let d = Opening::default_door(1, 1, 50.0);
        assert_eq!(d.auto_label(), "3068");
        let o = Opening::new(1, 0.0, OpeningKind::Door, 32.0, 80.0, 0.0);
        assert_eq!(o.auto_label(), "2868");
        let w = Opening::default_window(2, 1, 50.0);
        assert_eq!(w.auto_label(), "3050");
        let g = Opening::new(1, 0.0, OpeningKind::Door, 192.0, 84.0, 0.0);
        assert_eq!(g.auto_label(), "16070");
        // Fractions round down to the next-lower inch.
        let f = Opening::new(1, 0.0, OpeningKind::Door, 35.75, 79.9, 0.0);
        assert_eq!(f.auto_label(), "21167");
        let mut l = Opening::default();
        assert_eq!(l.label(), "3068");
        l.label_override = Some("A".into());
        assert_eq!(l.label(), "A");
        assert_eq!(l.auto_label(), "3068");
    }

    #[test]
    fn style_defaults_by_kind() {
        assert_eq!(Opening::default_door(1, 1, 0.0).style, OpeningStyle::Hinged);
        assert_eq!(
            Opening::default_window(1, 1, 0.0).style,
            OpeningStyle::Window
        );
        assert!(OpeningStyle::Pocket.is_door_style());
        assert!(!OpeningStyle::BayWindow.is_door_style());
        let d = Opening::default();
        assert_eq!((d.lites, d.egress, d.tempered), ((1, 1), false, false));
    }

    #[test]
    fn old_json_gets_kind_based_style() {
        let w: Opening = serde_json::from_str(
            r#"{"id":1,"wall_id":2,"center_offset":50.0,"width":36.0,"height":60.0,
                "sill_height":24.0,"kind":"Window","swing_flipped":false}"#,
        )
        .unwrap();
        assert_eq!(w.style, OpeningStyle::Window);
        assert!(!w.hinge_at_end && w.casing.is_none());
        assert_eq!(w.lites, (1, 1));
        let d: Opening = serde_json::from_str(
            r#"{"id":1,"wall_id":2,"center_offset":50.0,"width":36.0,"height":80.0,
                "sill_height":0.0,"kind":"Door","swing_flipped":true}"#,
        )
        .unwrap();
        assert_eq!(d.style, OpeningStyle::Hinged);
        assert!(d.swing_flipped);
        // And a full round trip keeps the new fields.
        let x = Opening {
            hinge_at_end: true,
            casing: Some(Casing::default()),
            schedule_number: Some("1".into()),
            ..Opening::default()
        };
        let back: Opening = serde_json::from_str(&serde_json::to_string(&x).unwrap()).unwrap();
        assert!(back.hinge_at_end);
        assert_eq!(back.casing, Some(Casing::default()));
        assert_eq!(back.schedule_number.as_deref(), Some("1"));
    }

    #[test]
    fn flips_are_independent() {
        let (mut p, w) = proj();
        let d = p.add_opening(0, w, 100.0, OpeningKind::Door).unwrap();
        assert!(p.flip_swing(0, d));
        let o = &p.floors[0].openings[0];
        assert!(o.swing_flipped && !o.hinge_at_end);
        assert!(p.flip_hinge(0, d));
        let o = &p.floors[0].openings[0];
        assert!(o.swing_flipped && o.hinge_at_end);
        assert!(p.flip_swing(0, d));
        assert!(!p.floors[0].openings[0].swing_flipped);
        assert!(!p.flip_swing(0, 999) && !p.flip_hinge(0, 999));
    }

    #[test]
    fn slide_clamps_and_rejects_overlap() {
        let (mut p, w) = proj();
        let a = p.add_opening(0, w, 40.0, OpeningKind::Door).unwrap();
        let b = p.add_opening(0, w, 150.0, OpeningKind::Door).unwrap();
        // Clamped to half-width + margin from the start.
        assert!(p.slide_opening(0, a, -50.0));
        let oa = p.floors[0].openings.iter().find(|o| o.id == a).unwrap();
        assert!((oa.center_offset - 20.0).abs() < 1e-9);
        // Onto the other door: refused, unchanged.
        assert!(!p.slide_opening(0, a, 150.0));
        let oa = p.floors[0].openings.iter().find(|o| o.id == a).unwrap();
        assert!((oa.center_offset - 20.0).abs() < 1e-9);
        // Sliding to its own current spot is fine; past the end clamps.
        assert!(p.slide_opening(0, b, 1000.0));
        let ob = p.floors[0].openings.iter().find(|o| o.id == b).unwrap();
        assert!((ob.center_offset - 180.0).abs() < 1e-9);
        assert!(!p.slide_opening(0, 999, 10.0));
        assert!(p.opening_position(0, b).is_some());
    }

    // ----- labels (DW-59..DW-63) -----

    #[test]
    fn size_text_formats() {
        use SizeFormat as F;
        use SizeStyle as S;
        assert_eq!(size_text(36.0, 80.0, F::WidthHeight, S::Shorthand), "3068");
        assert_eq!(size_text(36.0, 80.0, F::HeightWidth, S::Shorthand), "6830");
        assert_eq!(size_text(36.0, 80.0, F::WidthOnly, S::Shorthand), "30");
        assert_eq!(
            size_text(30.0, 80.0, F::WidthHeight, S::Architectural),
            "2'-6\" x 6'-8\""
        );
        assert_eq!(
            size_text(30.0, 80.0, F::HeightWidth, S::Architectural),
            "6'-8\" x 2'-6\""
        );
        assert_eq!(
            size_text(30.0, 80.0, F::WidthOnly, S::Architectural),
            "2'-6\""
        );
    }

    #[test]
    fn plan_label_shows_the_mark_with_a_schedule_and_the_size_without() {
        let defaults = OpeningLabelDefaults::default();
        let d = Opening::default();
        assert_eq!(d.plan_label(&defaults, None).as_deref(), Some("3068"));
        assert_eq!(d.plan_label(&defaults, Some("D01")).as_deref(), Some("D01"));
        // A blank mark is no mark.
        assert_eq!(d.plan_label(&defaults, Some(" ")).as_deref(), Some("3068"));
        // The opening's own schedule number stands in for a missing schedule.
        let n = Opening {
            schedule_number: Some("A1".into()),
            ..Opening::default()
        };
        assert_eq!(n.plan_label(&defaults, None).as_deref(), Some("A1"));
        // Without "include schedule number" the size always shows.
        let mut off = OpeningLabelDefaults::default();
        off.door.include_schedule_number = false;
        assert_eq!(d.plan_label(&off, Some("D01")).as_deref(), Some("3068"));
    }

    #[test]
    fn plan_label_modes_and_macros() {
        let mut defaults = OpeningLabelDefaults::default();
        let mut o = Opening::default();
        // Suppress and hidden-in-plan give no label.
        defaults.door.mode = LabelMode::Suppress;
        assert_eq!(o.plan_label(&defaults, None), None);
        defaults.door.mode = LabelMode::Automatic;
        defaults.door.display_in_plan = false;
        assert_eq!(o.plan_label(&defaults, None), None);
        defaults.door.display_in_plan = true;
        // Architectural sizes from the Default Settings.
        defaults.door.size_style = SizeStyle::Architectural;
        assert_eq!(
            o.plan_label(&defaults, None).as_deref(),
            Some("3'-0\" x 6'-8\"")
        );
        // Custom text with macros, settings-wide.
        defaults.door.mode = LabelMode::Custom;
        defaults.door.custom_text = "%type% %automatic_label% (%schedule_number%)".into();
        assert_eq!(
            o.plan_label(&defaults, Some("D02")).as_deref(),
            Some("Hinged Door 3'-0\" x 6'-8\" (D02)")
        );
        // The opening's own text beats both.
        o.label_override = Some("EXIT %width%".into());
        assert_eq!(
            o.plan_label(&defaults, None).as_deref(),
            Some("EXIT 3'-0\"")
        );
        // Per-opening settings beat the defaults and survive a save.
        o.label_override = None;
        o.extras.label = Some(LabelSettings {
            placement: LabelPlacement::Exterior,
            size_format: SizeFormat::WidthOnly,
            ..LabelSettings::default()
        });
        assert_eq!(o.plan_label(&defaults, None).as_deref(), Some("30"));
        let back: Opening = serde_json::from_str(&serde_json::to_string(&o).unwrap()).unwrap();
        assert_eq!(back.extras.label, o.extras.label);
        // include_type puts the style name first.
        let mut with_type = OpeningLabelDefaults::default();
        with_type.window.include_type = true;
        let w = Opening::default_window(1, 1, 50.0);
        assert_eq!(
            w.plan_label(&with_type, None).as_deref(),
            Some("Window 3050")
        );
    }

    #[test]
    fn variant_defaults_size_each_style() {
        let v = OpeningVariantDefaults::default();
        let door = Opening::new(0, 0.0, OpeningKind::Door, 30.0, 96.0, 0.0);
        let g = v.apply(&door, OpeningStyle::Garage);
        assert_eq!(
            (g.width, g.height, g.style),
            (108.0, 96.0, OpeningStyle::Garage)
        );
        let dbl = v.apply(&door, OpeningStyle::DoubleDoor);
        assert_eq!((dbl.width, dbl.height), (60.0, 96.0));
        let win = Opening::default_window(0, 0, 0.0);
        let bay = v.apply(&win, OpeningStyle::BayWindow);
        // A bay is 4 ft 2 in across the wall to start with (manual p. 604).
        assert_eq!((bay.width, bay.sill_height), (50.0, 24.0));
        assert_eq!(bay.extras.spec.bay.angle_deg, 45.0);
        let bow = v.apply(&win, OpeningStyle::BowWindow);
        assert_eq!(bow.width, 70.0);
        let box_ = v.apply(&win, OpeningStyle::BoxWindow);
        assert_eq!((box_.width, box_.extras.spec.bay.angle_deg), (50.0, 90.0));
        let niche = v.apply(&win, OpeningStyle::WallNiche);
        assert_eq!(
            (niche.width, niche.height, niche.sill_height),
            (24.0, 36.0, 36.0)
        );
        // A style without an entry keeps the template.
        let h = v.apply(&door, OpeningStyle::Hinged);
        assert_eq!((h.width, h.height), (30.0, 96.0));
        // And every style has a name and sits in exactly one of the flyout lists
        // (Fixed and Sliding serve both).
        for s in OpeningStyle::DOORS {
            assert!(!s.name(OpeningKind::Door).is_empty());
        }
        for s in OpeningStyle::WINDOWS {
            assert!(!s.name(OpeningKind::Window).is_empty());
            assert!(!s.is_door_style() || s == OpeningStyle::Sliding);
        }
        assert!(OpeningStyle::DoubleDoor.is_door_style());
        assert!(OpeningStyle::BoxWindow.projects());
    }

    #[test]
    fn old_files_load_the_new_fields_with_defaults() {
        let d: Opening = serde_json::from_str(
            r#"{"id":1,"wall_id":2,"center_offset":50.0,"width":36.0,"height":80.0,
                "sill_height":0.0,"kind":"Door"}"#,
        )
        .unwrap();
        assert_eq!(d.mull_group, None);
        assert_eq!(d.extras.label, None);
        let defaults: crate::PlanDefaults = serde_json::from_str("{}").unwrap();
        assert_eq!(defaults.opening_labels, OpeningLabelDefaults::default());
        assert!(defaults
            .opening_variants
            .size_for(OpeningStyle::Garage)
            .is_some());
    }

    // ----- resize, center, mull (DW-24, DW-26..DW-28, DW-51) -----

    fn three_windows() -> (Project, Id, [Id; 3]) {
        let (mut p, w) = proj();
        let mut ids = [0; 3];
        for (i, c) in [40.0, 80.0, 130.0].into_iter().enumerate() {
            ids[i] = p.add_opening(0, w, c, OpeningKind::Window).unwrap();
            // Windows 36" wide; fix the sizes so the spans are known.
        }
        (p, w, ids)
    }

    fn span(p: &Project, id: Id) -> (f64, f64) {
        let o = p.floors[0].openings.iter().find(|o| o.id == id).unwrap();
        (o.start_offset(), o.end_offset())
    }

    #[test]
    fn resizing_a_jamb_keeps_the_other_one() {
        let (mut p, w) = proj();
        let d = p.add_opening(0, w, 100.0, OpeningKind::Door).unwrap();
        // Door spans 82..118. Drag the end jamb out to 130.
        assert!(p.resize_opening(0, d, Jamb::End, 130.0));
        assert_eq!(span(&p, d), (82.0, 130.0));
        // The start jamb in to 100.
        assert!(p.resize_opening(0, d, Jamb::Start, 100.0));
        assert_eq!(span(&p, d), (100.0, 130.0));
        let o = &p.floors[0].openings[0];
        assert_eq!((o.width, o.center_offset), (30.0, 115.0));
        // Past the other jamb: clamps to the minimum width.
        assert!(p.resize_opening(0, d, Jamb::Start, 500.0));
        assert_eq!(span(&p, d), (124.0, 130.0));
        // Past the wall end clearance: clamps there.
        p.resize_opening(0, d, Jamb::End, 5000.0);
        assert_eq!(span(&p, d).1, 198.0);
        assert!(!p.resize_opening(0, 999, Jamb::End, 10.0));
    }

    #[test]
    fn resizing_stops_at_a_neighbour() {
        let (mut p, w) = proj();
        let a = p.add_opening(0, w, 40.0, OpeningKind::Door).unwrap();
        let b = p.add_opening(0, w, 150.0, OpeningKind::Door).unwrap();
        // a: 22..58, b: 132..168. a's end jamb stops 2" short of b.
        p.resize_opening(0, a, Jamb::End, 200.0);
        assert_eq!(span(&p, a).1, 130.0);
        // b's start jamb stops 2" past a.
        p.resize_opening(0, b, Jamb::Start, 0.0);
        assert_eq!(span(&p, b).0, 132.0);
        // Start jamb of a stops at the wall end clearance.
        p.resize_opening(0, a, Jamb::Start, -50.0);
        assert_eq!(span(&p, a).0, 2.0);
    }

    #[test]
    fn typed_width_resizes_about_the_center_or_the_jamb_with_room() {
        let (mut p, w) = proj();
        let a = p.add_opening(0, w, 40.0, OpeningKind::Door).unwrap();
        let _b = p.add_opening(0, w, 100.0, OpeningKind::Door).unwrap();
        // a: 22..58. 24" wide about the center 40 -> 28..52.
        assert!(p.set_opening_width(0, a, 24.0));
        assert_eq!(span(&p, a), (28.0, 52.0));
        // 60" about the center is 10..70.
        assert!(p.set_opening_width(0, a, 60.0));
        assert_eq!(span(&p, a), (10.0, 70.0));
        // 78" would pass both the wall start (about the center) and b.
        assert!(!p.set_opening_width(0, a, 78.0));
        assert!(p.set_opening_width(0, a, 70.0));
        assert_eq!(span(&p, a), (5.0, 75.0));
        // Near the wall start a wide window grows from its start jamb instead.
        assert!(p.set_opening_width(0, a, 24.0));
        assert!(p.slide_opening(0, a, 0.0));
        assert_eq!(span(&p, a), (2.0, 26.0));
        assert!(p.slide_opening(0, a, 30.0));
        assert_eq!(span(&p, a), (18.0, 42.0));
        assert!(p.set_opening_width(0, a, 60.0));
        assert_eq!(span(&p, a), (18.0, 78.0));
        // Too wide for the gap: refused.
        assert!(!p.set_opening_width(0, a, 90.0));
        assert!(!p.set_opening_width(0, a, 2.0));
        assert_eq!(span(&p, a), (18.0, 78.0));
    }

    #[test]
    fn center_opening_centers_in_the_free_span() {
        let (mut p, w) = proj();
        let a = p.add_opening(0, w, 40.0, OpeningKind::Door).unwrap();
        let b = p.add_opening(0, w, 170.0, OpeningKind::Door).unwrap();
        // Alone it centers on the wall segment between its neighbours:
        // free span 2..(b.start - 2) -> center 0.5*(2+150) = 76... check.
        assert!(p.center_opening(0, a));
        let (s, e) = span(&p, a);
        assert!(((s + e) * 0.5 - 0.5 * (2.0 + (152.0 - 2.0))).abs() < 1e-9);
        assert!(!p.center_opening(0, a));
        // b is limited by a and the far wall end.
        assert!(p.center_opening(0, b));
        assert!(!p.center_opening(0, 999));
    }

    #[test]
    fn an_opening_stops_where_its_casing_meets_an_intersecting_wall() {
        let (mut p, w) = proj();
        // A 4 1/2" partition crossing at 100: body 97.75..102.25.
        p.add_wall(
            0,
            Point::new(100.0, -60.0),
            Point::new(100.0, 60.0),
            4.5,
            DEFAULT_CEILING_HEIGHT,
            WallKind::Interior,
        );
        let a = p.add_opening(0, w, 40.0, OpeningKind::Window).unwrap();
        // The end jamb stops 3 3/4" (the casing) short of the wall.
        p.resize_opening(0, a, Jamb::End, 150.0);
        assert_eq!(span(&p, a).1, 97.75 - 3.75);
        // Ignore Casing for Opening Resize lets it run up to the wall.
        p.opening_display.ignore_casing = true;
        p.resize_opening(0, a, Jamb::End, 150.0);
        assert_eq!(span(&p, a).1, 97.75);
        // An opening past the wall stops on the far side.
        p.opening_display.ignore_casing = false;
        let b = p.add_opening(0, w, 150.0, OpeningKind::Window).unwrap();
        p.resize_opening(0, b, Jamb::Start, 0.0);
        assert_eq!(span(&p, b).0, 102.25 + 3.75);
    }

    #[test]
    fn an_opening_recessed_to_a_layer_stands_at_that_layers_exterior_side() {
        use crate::defaults::{WallLayer, WallTypeDef};
        let (mut p, w) = proj();
        p.wall_types.push(WallTypeDef {
            name: "Brick Veneer".into(),
            kind: WallKind::Exterior,
            layers: vec![
                WallLayer::new("Brick", 4.0, false, "Brick"),
                WallLayer::new("Sheathing", 0.5, false, "OSB"),
                WallLayer::new("Framing", 5.5, true, "Fir"),
                WallLayer::new("Drywall", 0.5, false, "Drywall"),
            ],
        });
        p.floors[0].wall_mut(w).unwrap().wall_type = Some("Brick Veneer".into());
        let d = p.add_opening(0, w, 100.0, OpeningKind::Door).unwrap();
        let depth = |p: &Project| p.floors[0].openings[0].extras.spec.recess_depth;
        p.floors[0].openings[0].extras.spec.recess_to = RecessTo::MainLayer;
        assert_eq!(p.sync_recess_depths(), 1);
        assert_eq!(depth(&p), Some(4.5));
        p.floors[0].openings[0].extras.spec.recess_to = RecessTo::SheathingLayer;
        assert_eq!(p.sync_recess_depths(), 1);
        assert_eq!(depth(&p), Some(4.0));
        assert_eq!(p.sync_recess_depths(), 0);
        // A typed depth is left alone.
        p.floors[0].openings[0].extras.spec.recess_to = RecessTo::Depth;
        p.floors[0].openings[0].extras.spec.recess_depth = Some(1.0);
        assert_eq!(p.sync_recess_depths(), 0);
        assert_eq!(depth(&p), Some(1.0));
        let _ = d;
    }

    #[test]
    fn a_casement_window_has_the_sashes_of_its_type() {
        let mut w = Opening::default_window(1, 1, 100.0);
        w.style = OpeningStyle::Casement;
        w.width = 60.0;
        // An older plan: the width decides.
        assert_eq!(w.casement_sashes(), 2);
        w.width = 36.0;
        assert_eq!(w.casement_sashes(), 1);
        // A typed casement: its Single, Double or Triple.
        w.extras.spec.window_type = WindowType::SingleCasement;
        w.width = 60.0;
        assert_eq!(w.casement_sashes(), 1);
        w.extras.spec.window_type = WindowType::DoubleCasement;
        assert_eq!(w.casement_sashes(), 2);
        w.extras.spec.window_type = WindowType::TripleCasement;
        assert_eq!(w.casement_sashes(), 3);
        // The plan shows a leaf and a swing arc for each.
        let wall = crate::model::Wall::new(
            Point::ZERO,
            Point::new(200.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        let sym = crate::opening_symbol::plan_symbol(&wall, &w, 1.0);
        assert_eq!(sym.count(crate::opening_symbol::PartKind::Leaf), 3);
        assert_eq!(sym.count(crate::opening_symbol::PartKind::Swing), 3);
    }

    #[test]
    fn the_minimum_separation_limits_a_resize() {
        let (mut p, w) = proj();
        let a = p.add_opening(0, w, 40.0, OpeningKind::Window).unwrap();
        let b = p.add_opening(0, w, 150.0, OpeningKind::Window).unwrap();
        // a: 22..58, b: 132..168.
        p.resize_opening(0, a, Jamb::End, 200.0);
        assert_eq!(span(&p, a).1, 130.0, "2 in short of b");
        p.opening_display.min_separation = 7.0;
        p.resize_opening(0, a, Jamb::End, 200.0);
        assert_eq!(span(&p, a).1, 125.0);
        assert_eq!(span(&p, b), (132.0, 168.0));
    }

    #[test]
    fn windows_whose_casings_touch_share_a_casing_span() {
        let (mut p, w) = proj();
        let a = p.add_opening(0, w, 40.0, OpeningKind::Window).unwrap();
        let b = p.add_opening(0, w, 80.0, OpeningKind::Window).unwrap();
        let c = p.add_opening(0, w, 170.0, OpeningKind::Window).unwrap();
        // a: 22..58, b: 62..98 (4 in apart); c far away.
        assert_eq!(p.casing_unit(0, a), Some(Some((22.0, 98.0))));
        assert_eq!(p.casing_unit(0, b), Some(Some((22.0, 98.0))));
        assert_eq!(p.casing_unit(0, c), Some(None));
        // A blocked unit keeps working as before.
        p.mull_openings(0, &[a, b]).unwrap();
        assert!(p.casing_unit(0, a).is_some());
    }

    #[test]
    fn mull_joins_adjacent_windows_and_unmull_splits_them() {
        let (mut p, w) = proj();
        let a = p.add_opening(0, w, 40.0, OpeningKind::Window).unwrap();
        let b = p.add_opening(0, w, 80.0, OpeningKind::Window).unwrap();
        // 36" windows 4" apart: a 22..58, b 62..98.
        assert_eq!(span(&p, a), (22.0, 58.0));
        assert_eq!(span(&p, b), (62.0, 98.0));
        let group = p.mull_openings(0, &[a, b]).unwrap();
        // The gap closed: b now touches a.
        assert_eq!(span(&p, a), (22.0, 58.0));
        assert_eq!(span(&p, b), (58.0, 94.0));
        let oa = p.floors[0].openings.iter().find(|o| o.id == a).unwrap();
        let ob = p.floors[0].openings.iter().find(|o| o.id == b).unwrap();
        assert_eq!((oa.mull_group, ob.mull_group), (Some(group), Some(group)));
        assert_eq!(p.mull_members(0, b), vec![a, b]);
        assert_eq!(p.unit_span(0, a), Some((22.0, 94.0)));
        // The unit moves together.
        assert!(p.slide_opening(0, a, 100.0));
        assert_eq!(span(&p, a), (82.0, 118.0));
        assert_eq!(span(&p, b), (118.0, 154.0));
        // Past the wall end clearance the whole unit clamps (unit is 72" wide).
        assert!(p.slide_opening(0, b, 5000.0));
        assert_eq!(span(&p, b).1, 198.0);
        assert_eq!(span(&p, a).1, 162.0);
        // A member's jamb cannot shrink into... it can, but cannot grow into its twin.
        p.resize_opening(0, a, Jamb::End, 400.0);
        assert_eq!(span(&p, a).1, 162.0);
        // Center the unit.
        assert!(p.center_opening(0, a));
        assert_eq!(p.unit_span(0, a), Some((64.0, 136.0)));
        // Unmull.
        assert_eq!(p.unmull_openings(0, a), 2);
        assert_eq!(p.mull_members(0, a), vec![a]);
        assert_eq!(p.unmull_openings(0, a), 0);
        assert!(p.floors[0].openings.iter().all(|o| o.mull_group.is_none()));
    }

    #[test]
    fn mull_refuses_what_it_cannot_join() {
        let (mut p, w, ids) = three_windows();
        // 40, 80, 130 with width 36: spans 22..58, 62..98, 112..148.
        // The outer two have the middle one in between.
        assert!(p
            .mull_openings(0, &[ids[0], ids[2]])
            .unwrap_err()
            .contains("in between"));
        assert!(p.mull_openings(0, &[ids[0]]).is_err());
        assert!(p.mull_openings(0, &[ids[0], 999]).is_err());
        // Gap of 14" between the second and third is too far.
        assert!(p
            .mull_openings(0, &[ids[1], ids[2]])
            .unwrap_err()
            .contains("too far"));
        // Doors do not mull.
        let d = p.add_opening(0, w, 190.0, OpeningKind::Door);
        let _ = d;
        // All three of a mulled chain: first two join, then the third is added.
        p.mull_openings(0, &[ids[0], ids[1]]).unwrap();
        // The unit's right edge is now 94; the third starts at 112: 18" away.
        assert!(p.mull_openings(0, &[ids[0], ids[2]]).is_err());
        // A second wall is refused.
        let w2 = p.add_wall(
            0,
            Point::new(0.0, 100.0),
            Point::new(200.0, 100.0),
            4.5,
            crate::model::DEFAULT_CEILING_HEIGHT,
            WallKind::Interior,
        );
        let other = p.add_opening(0, w2, 100.0, OpeningKind::Window).unwrap();
        assert!(p
            .mull_openings(0, &[ids[0], other])
            .unwrap_err()
            .contains("same wall"));
    }

    #[test]
    fn exterior_side_follows_the_room() {
        use crate::rooms::detect_rooms;
        let mut p = Project::new("x");
        let corners = [(0.0, 0.0), (240.0, 0.0), (240.0, 180.0), (0.0, 180.0)];
        for i in 0..4 {
            let (a, b) = (corners[i], corners[(i + 1) % 4]);
            p.add_wall(
                0,
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                6.0,
                96.0,
                WallKind::Exterior,
            );
        }
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        assert_eq!(rooms.len(), 1);
        for w in &p.floors[0].walls {
            // The room is inside: the outside is the side away from the center.
            let ext = exterior_sign(w, &rooms);
            let mid = w.point_at(w.length() * 0.5);
            let out = mid + w.normal() * (ext * 10.0);
            assert!(
                !crate::geometry::point_in_polygon(out, &rooms[0].polygon),
                "{w:?} {ext}"
            );
        }
        // No rooms: the left face is the outside; interior walls say right.
        assert_eq!(exterior_sign(&p.floors[0].walls[0], &[]), 1.0);
        let iw = Wall::new(
            Point::ZERO,
            Point::new(100.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        assert_eq!(exterior_sign(&iw, &[]), -1.0);
    }

    /// A 20' wall, 96" high, with a 36" door in the middle.
    fn door_wall() -> (Project, Id, Id) {
        let mut p = Project::new("t");
        let w = p.add_wall(
            0,
            Point::ZERO,
            Point::new(240.0, 0.0),
            4.5,
            96.0,
            WallKind::Exterior,
        );
        let d = p.add_opening(0, w, 120.0, OpeningKind::Door).unwrap();
        (p, w, d)
    }

    #[test]
    fn a_window_may_stand_directly_over_a_door_but_not_into_it() {
        let (p, w, d) = door_wall();
        let door = p.floors[0].openings[0].clone();
        let mut win = Opening::default_window(99, w, 120.0);
        win.width = 36.0;
        // The default window (sill 24) runs into the door's 80" head.
        assert!(openings_conflict(&win, &door, 2.0));
        win.sill_height = 79.0;
        assert!(openings_conflict(&win, &door, 2.0));
        // From the head up the two are stacked.
        win.sill_height = 80.0;
        win.height = 16.0;
        assert!(!openings_conflict(&win, &door, 2.0));
        assert!(stands_over(&win, &door) && !stands_over(&door, &win));
        // Side by side they still keep the clearance.
        win.sill_height = 24.0;
        win.center_offset = 150.0;
        assert!(openings_conflict(&win, &door, 2.0));
        win.center_offset = 160.0;
        assert!(!openings_conflict(&win, &door, 2.0));
        let _ = (p, d);
    }

    #[test]
    fn a_transom_is_a_fixed_window_mulled_over_its_door() {
        let (mut p, _, d) = door_wall();
        let t = p.add_transom(0, d, 18.0).unwrap();
        let door = p.floors[0].openings[0].clone();
        let tr = p.floors[0].openings.iter().find(|o| o.id == t).unwrap();
        // Over the door, its width, 16" high at most (96 - 80).
        assert_eq!(tr.style, OpeningStyle::Fixed);
        assert_eq!(
            (tr.center_offset, tr.width, tr.sill_height, tr.height),
            (120.0, 36.0, 80.0, 16.0)
        );
        assert!(stands_over(tr, &door));
        assert_eq!(tr.mull_group, door.mull_group);
        assert!(tr.mull_group.is_some());
        // The unit's casing is the door's.
        assert_eq!(p.casing_unit(0, t), None);
        assert_eq!(p.casing_unit(0, d), Some(Some((102.0, 138.0))));
        // Nothing fits over it a second time, and a window cannot be put in
        // the transom's way.
        assert!(p.add_transom(0, d, 6.0).is_err());
        assert!(p.add_transom(0, t, 6.0).is_err());
        // Sliding the door slides the transom with it, and the unit moves
        // over a window beside it only if that clears the opening.
        assert!(p.slide_opening(0, d, 150.0));
        let moved = &p.floors[0].openings;
        assert_eq!(moved[0].center_offset, 150.0);
        assert_eq!(moved[1].center_offset, 150.0);
        // Unmulled, the transom is a plain window again.
        assert_eq!(p.unmull_openings(0, d), 2);
        assert_eq!(p.casing_unit(0, t), Some(None));
    }

    #[test]
    fn a_transom_needs_headroom_and_a_door_that_can_be_mulled() {
        let (mut p, w, d) = door_wall();
        p.floors[0].walls[0].height = 82.0;
        assert!(p.add_transom(0, d, 18.0).unwrap_err().contains("no room"));
        p.floors[0].walls[0].height = 96.0;
        // A garage door cannot be mulled, so no transom is left behind.
        let g = p.add_opening(0, w, 40.0, OpeningKind::Door).unwrap();
        p.floors[0]
            .openings
            .iter_mut()
            .find(|o| o.id == g)
            .unwrap()
            .style = OpeningStyle::Garage;
        let before = p.floors[0].openings.len();
        assert!(p.add_transom(0, g, 12.0).is_err());
        assert_eq!(p.floors[0].openings.len(), before);
    }

    #[test]
    fn a_door_with_a_sidelite_takes_one_transom_across_both() {
        let (mut p, w, d) = door_wall();
        // Sidelite on the right of the door, mulled with it.
        let side = p.add_opening(0, w, 200.0, OpeningKind::Window).unwrap();
        for o in &mut p.floors[0].openings {
            if o.id == side {
                o.width = 18.0;
                o.center_offset = 147.0;
                o.sill_height = 0.0;
                o.height = 80.0;
                o.style = OpeningStyle::Fixed;
            }
        }
        p.mull_openings(0, &[d, side]).unwrap();
        let t = p.add_transom(0, d, 12.0).unwrap();
        let tr = p.floors[0].openings.iter().find(|o| o.id == t).unwrap();
        // 102..156 across: the door's 36 plus the sidelite's 18.
        assert_eq!((tr.start_offset(), tr.end_offset()), (102.0, 156.0));
        // One casing for the whole unit, drawn by the door and its sidelite.
        assert_eq!(p.casing_unit(0, t), None);
        assert_eq!(p.casing_unit(0, d), Some(Some((102.0, 156.0))));
        assert_eq!(p.casing_unit(0, side), Some(Some((102.0, 156.0))));
        // Mulling the pieces by hand gives the same unit.
        p.unmull_openings(0, d);
        assert!(p.mull_openings(0, &[d, side, t]).is_ok());
        assert_eq!(p.unit_span(0, t), Some((102.0, 156.0)));
    }

    #[test]
    fn a_window_slides_over_a_door_only_at_head_height() {
        let (mut p, w, d) = door_wall();
        let win = p.add_opening(0, w, 40.0, OpeningKind::Window).unwrap();
        // At sill 24 it cannot pass over the door.
        assert!(!p.slide_opening(0, win, 120.0));
        let o = p.floors[0]
            .openings
            .iter_mut()
            .find(|o| o.id == win)
            .unwrap();
        o.sill_height = 80.0;
        o.height = 12.0;
        assert!(p.slide_opening(0, win, 120.0));
        // Neither limits the other when resized.
        assert!(p.set_opening_width(0, win, 60.0));
        let (lo, hi) = p.free_span(0, d, false).unwrap();
        assert_eq!((lo, hi), (2.0, 238.0));
    }

    /// A 20' chord bowed 60" (radius 150") with one door on it.
    fn arc_project() -> (Project, Id, Id) {
        let mut p = Project::new("arc");
        let w = p.add_wall(
            0,
            Point::ZERO,
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        p.floors[0].walls[0].curve = Some(crate::walls::WallCurve { bulge: 60.0 });
        let d = p.add_opening(0, w, 100.0, OpeningKind::Door).unwrap();
        (p, w, d)
    }

    #[test]
    fn positions_on_a_curved_wall_are_arc_lengths() {
        let (p, w, d) = arc_project();
        let wall = p.floors[0].wall(w).unwrap();
        assert!(wall.path_length() > wall.length() + 10.0);
        // An opening can sit beyond the chord's length: the wall is longer.
        let mut q = p.clone();
        q.floors[0].openings.clear();
        let far = wall.path_length() - 40.0;
        assert!(far > wall.length() - 40.0);
        let id = q.add_opening(0, w, far, OpeningKind::Door).unwrap();
        assert!((q.floors[0].openings[0].center_offset - far).abs() < 1e-9);
        let _ = id;
        // point_along / locate round-trip, on the centerline and off it.
        for s in [0.0, 37.5, 100.0, wall.path_length() - 1.0] {
            let c = wall.point_along(s);
            let (back, lateral) = wall.locate(c);
            assert!((back - s).abs() < 1e-6 && lateral.abs() < 1e-6, "{s}");
            let off = wall.point_offset(s, 2.5);
            let (back, lateral) = wall.locate(off);
            assert!((back - s).abs() < 1e-6 && (lateral - 2.5).abs() < 1e-6);
        }
        // The position of the opening is its center on the arc.
        let at = p.opening_position(0, d).unwrap();
        assert!(at.dist(wall.point_along(100.0)) < 1e-9);
        // The band over an opening follows the arc: every corner is on the
        // arc offset by the band's edge.
        let (c, r) = wall.arc_center_radius().unwrap();
        let sweep_sign = wall.curve.unwrap().sweep(wall.start, wall.end).signum();
        let quads = wall.band_quads(82.0, 118.0, -4.0, 4.0);
        assert!(quads.len() > 1);
        for q in &quads {
            for (i, corner) in q.iter().enumerate() {
                let t = if i < 2 { 4.0 } else { -4.0 };
                let want = r - sweep_sign * t;
                assert!((corner.sub(c).length() - want).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn the_pointer_side_of_a_curved_wall_picks_the_swing() {
        let (p, w, _) = arc_project();
        let wall = p.floors[0].wall(w).unwrap();
        let at = wall.point_along(120.0);
        let n = wall.normal_along(120.0);
        let (flipped_left, _) = door_defaults_for_pointer(wall, at + n * 10.0, 120.0);
        let (flipped_right, _) = door_defaults_for_pointer(wall, at - n * 10.0, 120.0);
        assert!(!flipped_left && flipped_right);
    }
}
