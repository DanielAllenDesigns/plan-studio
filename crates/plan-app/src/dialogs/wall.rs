//! Wall Specification (docs/chief-x18-dialogs.md, "Wall Specification").

use super::opening::OPENING_MARGIN;
use super::{
    dis_check, dis_combo, dis_radio, layer_stack, off, on, pv_text, row, section, session_check,
    wall_plan_sketch, Fields, Outcome, SpecDialog, SpecPages, Tab, WallTypeDialog, PV_FAINT,
};
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::extras::WallExtras as StoredExtras;
use plan_core::geometry::Point;
use plan_core::units::fmt_ft_in;
use plan_core::walls::{DEFAULT_HALF_WALL_HEIGHT, DEFAULT_PONY_SPLIT};
use plan_core::{FenceStyle, Id, Opening, Wall, WallClass, WallKind, WallTypeDef};

/// Values a Roof tab control starts with when it is switched on.
const ROOF_PITCH_DEFAULT: f64 = 8.0;
const ROOF_UPPER_PITCH_DEFAULT: f64 = 12.0;
const ROOF_OVERHANG_DEFAULT: f64 = 16.0;
const ROOF_EXTEND_DEFAULT: f64 = 24.0;
const ROOF_RETURN_DEFAULT: f64 = 24.0;

/// Extras-map keys for the two default-wall dialogs (real ids count up from 1).
const DEFAULT_EXTERIOR_KEY: Id = Id::MAX;
const DEFAULT_INTERIOR_KEY: Id = Id::MAX - 1;
const DEFAULT_FOUNDATION_KEY: Id = Id::MAX - 5;

/// What a wall dialog is bound to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WallTarget {
    Wall(Id),
    /// Edit > Default Settings > Walls > Exterior Wall.
    DefaultExterior,
    DefaultInterior,
    DefaultFoundation,
}

impl WallTarget {
    /// Key of this target in the app's per-session extras map.
    pub fn key(self) -> Id {
        match self {
            WallTarget::Wall(id) => id,
            WallTarget::DefaultExterior => DEFAULT_EXTERIOR_KEY,
            WallTarget::DefaultInterior => DEFAULT_INTERIOR_KEY,
            WallTarget::DefaultFoundation => DEFAULT_FOUNDATION_KEY,
        }
    }

    fn is_default(self) -> bool {
        !matches!(self, WallTarget::Wall(_))
    }
}

const WALL_LAYERS: [&str; 6] = [
    "Walls, Normal",
    "Walls, Invisible",
    "Walls, Labels",
    "Footings",
    "Deck Railing",
    "Fencing",
];

/// The wall classes of the General tab's Wall Class list, in flyout order.
const CLASS_NAMES: [&str; 11] = [
    "Standard",
    "Foundation",
    "Pony Wall",
    "Glass Wall",
    "Glass Pony Wall",
    "Half-Wall",
    "Room Divider",
    "Railing",
    "Deck Railing",
    "Deck Edge",
    "Fencing",
];

fn class_index(c: &WallClass) -> usize {
    match c {
        WallClass::Standard => 0,
        WallClass::Foundation => 1,
        WallClass::Pony { .. } => 2,
        WallClass::Glass => 3,
        WallClass::GlassPony { .. } => 4,
        WallClass::HalfWall { .. } => 5,
        WallClass::RoomDivider => 6,
        WallClass::Railing => 7,
        WallClass::DeckRailing => 8,
        WallClass::DeckEdge => 9,
        WallClass::Fencing { .. } => 10,
    }
}

/// Whether a wall type is one of the special ones the class lists keep to
/// their own wall classes.
fn special_type(t: &WallTypeDef) -> Option<&'static str> {
    let has = |s: &str| {
        t.layers.iter().any(|l| {
            l.material.to_ascii_lowercase().contains(s) || l.name.to_ascii_lowercase().contains(s)
        })
    };
    if t.name.starts_with("Deck Railing") {
        Some("Deck Railing")
    } else if t.name.starts_with("Deck Edge") {
        Some("Deck Edge")
    } else if t.name.starts_with("Railing") {
        Some("Railing")
    } else if t.name.starts_with("Fence") {
        Some("Fencing")
    } else if has("glass") {
        Some("Glass")
    } else {
        None
    }
}

/// The tabs of an exterior wall: its Roof tab is live.
const WALL_TABS_EXTERIOR: &[Tab] = &[
    on("General"),
    on("Structure"),
    on("Roof"),
    off("Foundation"),
    on("Wall Types"),
    off("Wall Cap"),
    off("Wall Covering"),
    on("Rail Style"),
    off("Newels/Balusters"),
    off("Rails"),
    on("Layer"),
    off("Materials"),
    on("Label"),
    off("Components"),
    off("Object Information"),
    off("Schedule"),
];

const WALL_TABS: &[Tab] = &[
    on("General"),
    on("Structure"),
    off("Roof"),
    off("Foundation"),
    on("Wall Types"),
    off("Wall Cap"),
    off("Wall Covering"),
    on("Rail Style"),
    off("Newels/Balusters"),
    off("Rails"),
    on("Layer"),
    off("Materials"),
    on("Label"),
    off("Components"),
    off("Object Information"),
    off("Schedule"),
];

/// Wall dialog values. The last picked wall type, the plan label switch and
/// the specified label text are stored with the wall (`Wall.extras`, see
/// [`WallExtras::with_stored`] and [`WallExtras::to_stored`]); the app keeps
/// the rest per wall id for the session only. (Invisible, No Room Definition
/// and No Locate live in `Wall.flags`.)
#[derive(Clone, Debug, PartialEq)]
pub struct WallExtras {
    /// Name of the wall type last picked in the Wall Types tab.
    wall_type: Option<String>,
    suppress_label: bool,
    display_in_plan: bool,
    specify_label: bool,
    label_text: String,
}

impl Default for WallExtras {
    fn default() -> Self {
        Self {
            wall_type: None,
            suppress_label: false,
            display_in_plan: true,
            specify_label: false,
            label_text: String::new(),
        }
    }
}

impl WallExtras {
    /// These extras with the values stored with a wall laid over them.
    pub fn with_stored(mut self, stored: &StoredExtras) -> Self {
        self.display_in_plan = stored.display_label;
        self.specify_label = stored.label_text.is_some();
        if let Some(t) = &stored.label_text {
            self.label_text = t.clone();
        }
        if stored.last_wall_type.is_some() {
            self.wall_type = stored.last_wall_type.clone();
        }
        self
    }

    /// The stored form of these values.
    pub fn to_stored(&self) -> StoredExtras {
        StoredExtras {
            label_text: self.specify_label.then(|| self.label_text.clone()),
            display_label: self.display_in_plan,
            last_wall_type: self.wall_type.clone(),
        }
    }
}

/// Which wall point stays put when the length is edited.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum WallLock {
    Start,
    Center,
    End,
}

pub struct WallDialog {
    frame: SpecDialog,
    form: WallForm,
}

struct WallForm {
    target: WallTarget,
    draft: Wall,
    extras: WallExtras,
    /// The wall's openings as they were when the dialog opened.
    openings: Vec<Opening>,
    orig_start: Point,
    lock: WallLock,
    default_height: f64,
    default_top: bool,
    fields: Fields,
    /// The wall types offered in the Wall Types tab (`PlanDefaults::wall_types`).
    types: Vec<WallTypeDef>,
    /// The Wall Type Definitions dialog, while open.
    define: Option<WallTypeDialog>,
    /// Wall types edited or created in that dialog (to store with the plan
    /// and the defaults on OK).
    edited_types: Vec<WallTypeDef>,
}

impl WallDialog {
    /// `default_height` is the app's default wall height (for the Structure
    /// tab's "Default Wall Top Height" box).
    pub fn new(
        target: WallTarget,
        wall: Wall,
        openings: Vec<Opening>,
        extras: WallExtras,
        default_height: f64,
        types: Vec<WallTypeDef>,
    ) -> Self {
        let title = match target {
            WallTarget::Wall(_) => "Wall Specification",
            WallTarget::DefaultExterior => "Wall Specification (Exterior Wall Defaults)",
            WallTarget::DefaultInterior => "Wall Specification (Interior Wall Defaults)",
            WallTarget::DefaultFoundation => "Wall Specification (Foundation Wall Defaults)",
        };
        let default_top = (wall.height - default_height).abs() < 1e-6;
        // A placed wall's own stored values win over the session's.
        let extras = if target.is_default() {
            extras
        } else {
            extras.with_stored(&wall.extras)
        };
        Self {
            frame: SpecDialog::new(title, "wall"),
            form: WallForm {
                target,
                orig_start: wall.start,
                draft: wall,
                extras,
                openings,
                lock: WallLock::Start,
                default_height,
                default_top,
                fields: Fields::default(),
                types,
                define: None,
                edited_types: Vec::new(),
            },
        }
    }

    /// A dialog for one of the default wall settings; `type_name` is the
    /// wall type currently set in the defaults.
    pub fn for_default(
        target: WallTarget,
        thickness: f64,
        height: f64,
        mut extras: WallExtras,
        types: Vec<WallTypeDef>,
        type_name: &str,
    ) -> Self {
        extras.wall_type = Some(type_name.to_string());
        let kind = if target == WallTarget::DefaultInterior {
            WallKind::Interior
        } else {
            WallKind::Exterior
        };
        // A sample wall for the preview; length and angle are not editable.
        let wall = crate::editor::ops::make_wall(
            0,
            Point::ZERO,
            Point::new(144.0, 0.0),
            thickness,
            height,
            kind,
        );
        Self::new(target, wall, Vec::new(), extras, height, types)
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        // The definitions dialog is drawn first so it takes Esc and Enter.
        if let Some(mut d) = self.form.define.take() {
            match d.show(ctx) {
                Outcome::Open => self.form.define = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => self.form.apply_defined(&mut d),
            }
        }
        let outcome = self.frame.show(ctx, &mut self.form);
        self.sync_stored();
        outcome
    }

    /// Copies the values that persist with the wall into the draft, so
    /// accepting the dialog stores them in `Wall.extras`.
    pub(crate) fn sync_stored(&mut self) {
        self.form.draft.extras = self.form.extras.to_stored();
    }

    /// Wall types edited or created through "Define...".
    pub fn edited_types(&self) -> &[WallTypeDef] {
        &self.form.edited_types
    }

    pub fn target(&self) -> WallTarget {
        self.form.target
    }

    pub fn draft(&self) -> &Wall {
        &self.form.draft
    }

    /// Tests edit the draft as the form's controls would.
    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut Wall {
        &mut self.form.draft
    }

    pub fn extras(&self) -> &WallExtras {
        &self.form.extras
    }

    /// The wall type picked in the Wall Types tab, if any.
    pub fn picked_type(&self) -> Option<&str> {
        self.form.extras.wall_type.as_deref()
    }

    /// The wall's openings with their offsets adjusted to the edited start
    /// point, so they stay where they were along the wall.
    pub fn adjusted_openings(&self) -> Vec<Opening> {
        self.form.adjusted_openings()
    }
}

impl WallForm {
    /// Takes the result of the Wall Type Definitions dialog.
    fn apply_defined(&mut self, d: &mut WallTypeDialog) {
        for t in d.changed_types() {
            match self.types.iter_mut().find(|x| x.name == t.name) {
                Some(slot) => *slot = t.clone(),
                None => self.types.push(t.clone()),
            }
            match self.edited_types.iter_mut().find(|x| x.name == t.name) {
                Some(slot) => *slot = t,
                None => self.edited_types.push(t),
            }
        }
        self.draft.resize_about = d.resize_about();
        if let Some(def) = d
            .selected_name()
            .and_then(|n| self.types.iter().find(|t| t.name == n))
        {
            self.draft.thickness = def.thickness();
            if !self.target.is_default() {
                self.draft.kind = def.kind;
            }
            self.draft.wall_type = Some(def.name.clone());
            self.extras.wall_type = Some(def.name.clone());
        }
    }

    fn adjusted_openings(&self) -> Vec<Opening> {
        let dir = self.draft.direction();
        let shift = self.orig_start.sub(self.draft.start).dot(dir);
        self.openings
            .iter()
            .map(|o| {
                let mut o = o.clone();
                o.center_offset += shift;
                o
            })
            .collect()
    }

    /// Direction of the draft; +x for a degenerate wall.
    fn dir(&self) -> Point {
        let d = self.draft.direction();
        if d.length() < 0.5 {
            Point::new(1.0, 0.0)
        } else {
            d
        }
    }

    fn angle_deg(&self) -> f64 {
        let a = self.dir().angle().to_degrees().rem_euclid(360.0);
        let a = (a * 100.0).round() / 100.0;
        if a >= 360.0 {
            0.0
        } else {
            a
        }
    }

    /// Moves the end point (Start lock), start point (End lock) or both
    /// (Center lock) along the wall direction.
    fn set_length(&mut self, len: f64) {
        let len = len.max(1.0);
        let d = self.dir();
        let (s, e) = (self.draft.start, self.draft.end);
        match self.lock {
            WallLock::Start => self.draft.end = s.add(d.scale(len)),
            WallLock::End => self.draft.start = e.sub(d.scale(len)),
            WallLock::Center => {
                let mid = Point::lerp(s, e, 0.5);
                self.draft.start = mid.sub(d.scale(len * 0.5));
                self.draft.end = mid.add(d.scale(len * 0.5));
            }
        }
    }

    /// Rotates the wall about its start point, keeping the length.
    fn set_angle(&mut self, deg: f64) {
        let a = deg.to_radians();
        let len = self.draft.length().max(1.0);
        self.draft.end = self
            .draft
            .start
            .add(Point::new(a.cos(), a.sin()).scale(len));
    }

    /// The type matching the draft: the one picked last if it still fits,
    /// else the first with the same kind and thickness.
    fn current_type(&self) -> Option<usize> {
        let fits = |t: &WallTypeDef| (t.thickness() - self.draft.thickness).abs() < 1e-6;
        let picked = self.extras.wall_type.as_deref().and_then(|n| {
            self.types
                .iter()
                .position(|t| t.name == n)
                .filter(|i| fits(&self.types[*i]))
        });
        picked.or_else(|| {
            self.types
                .iter()
                .position(|t| fits(t) && t.kind == self.draft.kind)
        })
    }

    /// Layers to draw for the draft: the type's, or a generic stack.
    fn layers(&self) -> Vec<(&str, f64)> {
        if let Some(i) = self.current_type() {
            return self.types[i]
                .layers
                .iter()
                .map(|l| (l.name.as_str(), l.thickness))
                .collect();
        }
        let t = self.draft.thickness.max(0.1);
        if t >= 1.5 {
            vec![("Drywall", 0.5), ("Framing", t - 1.0), ("Drywall", 0.5)]
        } else {
            vec![("Drywall", t)]
        }
    }

    /// Makes the draft wall `class`, with the layer, heights and types the
    /// class goes with.
    fn change_class(&mut self, class: WallClass) {
        if let Some(old) = self.draft.class.default_layer() {
            if self.draft.layer == old {
                self.draft.layer = plan_core::model::DEFAULT_WALL_LAYER.to_string();
            }
        }
        // The wall type that goes with the class.
        let wanted = match &class {
            WallClass::Glass | WallClass::GlassPony { .. } => Some("Glass"),
            WallClass::Railing => Some("Railing"),
            WallClass::DeckRailing => Some("Deck Railing"),
            WallClass::DeckEdge => Some("Deck Edge"),
            WallClass::Fencing { .. } => Some("Fencing"),
            _ => None,
        };
        let current = self
            .draft
            .wall_type
            .as_deref()
            .and_then(|n| self.types.iter().find(|t| t.name == n));
        let new_type = match wanted {
            Some(w) if current.and_then(special_type) != Some(w) => self
                .types
                .iter()
                .find(|t| special_type(t) == Some(w))
                .map(|t| t.name.clone()),
            None if class == WallClass::Foundation => self
                .types
                .iter()
                .find(|t| t.name == "Foundation-8")
                .map(|t| t.name.clone()),
            None if current.and_then(special_type).is_some() => Some(self.ordinary_type(None)),
            _ => None,
        };
        if let Some(name) = new_type.filter(|n| !n.is_empty()) {
            if let Some(t) = self.types.iter().find(|t| t.name == name) {
                self.draft.thickness = t.thickness();
            }
            self.extras.wall_type = Some(name.clone());
            self.draft.wall_type = Some(name);
        }
        self.draft.set_class(class);
        if let Some(l) = self.draft.class.default_layer() {
            self.draft.layer = l.to_string();
        }
        if let WallClass::HalfWall { height } = self.draft.class {
            self.draft.height = height;
        }
    }

    /// Whether the Wall Type list of this wall's class offers `t`.
    fn offers_type(&self, t: &WallTypeDef) -> bool {
        let special = special_type(t);
        match &self.draft.class {
            WallClass::Glass | WallClass::GlassPony { .. } => special == Some("Glass"),
            WallClass::Foundation => t
                .layers
                .iter()
                .any(|l| l.material.to_ascii_lowercase().contains("concrete")),
            WallClass::Railing => special == Some("Railing"),
            WallClass::DeckRailing => special == Some("Deck Railing"),
            WallClass::DeckEdge => special == Some("Deck Edge"),
            WallClass::Fencing { .. } => special == Some("Fencing"),
            _ => special.is_none(),
        }
    }

    /// A wall type name for a class that needs one: the wall's own if it is
    /// an ordinary type, else the first ordinary one.
    fn ordinary_type(&self, prefer: Option<&str>) -> String {
        let ok = |n: &str| {
            self.types
                .iter()
                .any(|t| t.name == n && special_type(t).is_none())
        };
        prefer
            .filter(|n| ok(n))
            .or(self.draft.wall_type.as_deref().filter(|n| ok(n)))
            .map(str::to_string)
            .or_else(|| {
                self.types
                    .iter()
                    .find(|t| special_type(t).is_none() && t.kind == WallKind::Exterior)
                    .map(|t| t.name.clone())
            })
            .unwrap_or_default()
    }

    /// The class picked by `CLASS_NAMES[i]` with defaults for its fields.
    fn class_from_index(&self, i: usize) -> WallClass {
        let lower = self.ordinary_type(Some("Foundation-8"));
        match i {
            1 => WallClass::Foundation,
            2 => WallClass::Pony {
                upper_type: self.ordinary_type(None),
                lower_type: lower,
                split_height: DEFAULT_PONY_SPLIT,
                upper_sets_plan_display: false,
            },
            3 => WallClass::Glass,
            4 => WallClass::GlassPony {
                lower_type: lower,
                split_height: DEFAULT_PONY_SPLIT,
            },
            5 => WallClass::HalfWall {
                height: DEFAULT_HALF_WALL_HEIGHT,
            },
            6 => WallClass::RoomDivider,
            7 => WallClass::Railing,
            8 => WallClass::DeckRailing,
            9 => WallClass::DeckEdge,
            10 => WallClass::Fencing {
                style: FenceStyle::Picket,
            },
            _ => WallClass::Standard,
        }
    }

    /// General tab: the wall class and the fields only some classes have.
    fn class_rows(&mut self, ui: &mut Ui) {
        let current = class_index(&self.draft.class);
        row(ui, "Wall Class", |ui| {
            egui::ComboBox::from_id_salt("wall_class")
                .selected_text(CLASS_NAMES[current])
                .show_ui(ui, |ui| {
                    for (i, name) in CLASS_NAMES.iter().enumerate() {
                        if ui.selectable_label(i == current, *name).clicked() && i != current {
                            let class = self.class_from_index(i);
                            self.change_class(class);
                        }
                    }
                });
        });
        match self.draft.class.clone() {
            WallClass::Foundation => {
                self.fields.length_row(
                    ui,
                    "Foundation Height",
                    "foundation_height",
                    &mut self.draft.foundation_height,
                );
            }
            WallClass::HalfWall { height } => {
                let mut h = height;
                if self
                    .fields
                    .length_row(ui, "Half-Wall Height", "half_height", &mut h)
                    && h > 0.0
                {
                    self.draft.class = WallClass::HalfWall { height: h };
                    self.draft.height = h;
                    self.default_top = false;
                }
            }
            WallClass::Fencing { style } => {
                row(ui, "Fence Style", |ui| {
                    let mut st = style;
                    for (v, name) in [
                        (FenceStyle::Picket, "Picket"),
                        (FenceStyle::Privacy, "Privacy"),
                        (FenceStyle::Rail, "Rail"),
                    ] {
                        ui.radio_value(&mut st, v, name);
                    }
                    if st != style {
                        self.draft.class = WallClass::Fencing { style: st };
                    }
                });
            }
            _ => {}
        }
    }

    /// A combo that picks one of the wall types `offered` and returns the
    /// pick, if it changed.
    fn type_combo(
        &self,
        ui: &mut Ui,
        id: &'static str,
        current: &str,
        offered: impl Fn(&WallTypeDef) -> bool,
    ) -> Option<String> {
        let mut picked = None;
        egui::ComboBox::from_id_salt(id)
            .selected_text(current.to_string())
            .show_ui(ui, |ui| {
                for t in self.types.iter().filter(|t| offered(t)) {
                    if ui.selectable_label(t.name == current, &t.name).clicked() {
                        picked = Some(t.name.clone());
                    }
                }
            });
        picked
    }

    /// Wall Types tab: the pony wall's upper and lower types, split height
    /// and which type the plan shows.
    fn pony_section(&mut self, ui: &mut Ui) {
        section(ui, "Pony Wall");
        let mut is_pony = matches!(
            self.draft.class,
            WallClass::Pony { .. } | WallClass::GlassPony { .. }
        );
        let was = is_pony;
        ui.checkbox(&mut is_pony, "Pony Wall");
        if is_pony != was {
            self.change_class(self.class_from_index(if is_pony { 2 } else { 0 }));
        }
        let ordinary = |t: &WallTypeDef| special_type(t).is_none();
        match self.draft.class.clone() {
            WallClass::Pony {
                upper_type,
                lower_type,
                split_height,
                upper_sets_plan_display,
            } => {
                let (mut upper, mut lower) = (upper_type.clone(), lower_type.clone());
                let mut split = split_height;
                let mut show_upper = upper_sets_plan_display;
                row(ui, "Upper Wall Type", |ui| {
                    if let Some(n) = self.type_combo(ui, "pony_upper", &upper_type, ordinary) {
                        upper = n;
                    }
                });
                row(ui, "Lower Wall Type", |ui| {
                    if let Some(n) = self.type_combo(ui, "pony_lower", &lower_type, ordinary) {
                        lower = n;
                    }
                });
                self.fields
                    .length_row(ui, "Elevation of Lower Wall Top", "pony_split", &mut split);
                row(ui, "Display in Plan View", |ui| {
                    ui.radio_value(&mut show_upper, true, "Upper");
                    ui.radio_value(&mut show_upper, false, "Lower");
                });
                let split = split.clamp(0.0, self.draft.height.max(0.0));
                if upper != upper_type {
                    if let Some(t) = self.types.iter().find(|t| t.name == upper) {
                        self.draft.thickness = t.thickness();
                    }
                    self.draft.wall_type = Some(upper.clone());
                    self.extras.wall_type = Some(upper.clone());
                }
                let class = WallClass::Pony {
                    upper_type: upper,
                    lower_type: lower,
                    split_height: split,
                    upper_sets_plan_display: show_upper,
                };
                if class != self.draft.class {
                    self.draft.set_class(class);
                }
            }
            WallClass::GlassPony {
                lower_type,
                split_height,
            } => {
                let mut lower = lower_type.clone();
                let mut split = split_height;
                row(ui, "Upper Wall Type", |ui| {
                    ui.label("Glass");
                });
                row(ui, "Lower Wall Type", |ui| {
                    if let Some(n) = self.type_combo(ui, "pony_lower", &lower_type, ordinary) {
                        lower = n;
                    }
                });
                self.fields
                    .length_row(ui, "Elevation of Lower Wall Top", "pony_split", &mut split);
                let class = WallClass::GlassPony {
                    lower_type: lower,
                    split_height: split.clamp(0.0, self.draft.height.max(0.0)),
                };
                if class != self.draft.class {
                    self.draft.set_class(class);
                }
            }
            _ => {
                ui.add_enabled_ui(false, |ui| {
                    row(ui, "Lower Wall Type", |ui| {
                        dis_combo(ui, "pony_type", "Foundation-8")
                    });
                    row(ui, "Elevation of Lower Wall Top", |ui| {
                        ui.label(fmt_ft_in(DEFAULT_PONY_SPLIT));
                    });
                });
            }
        }
    }

    /// Rail Style tab: what a railing wall builds.
    fn rail_style(&mut self, ui: &mut Ui) {
        section(ui, "Rail Style");
        if !self.draft.class.is_railing() && self.draft.class != WallClass::Railing {
            ui.weak("Draw a Railing or Deck Railing wall, or tick Railing on the General tab.");
            return;
        }
        self.fields
            .length_row(ui, "Railing Height", "rail_height", &mut self.draft.height);
        let len = self.draft.length();
        ui.weak(format!(
            "Posts {}: one every 8' at most and one at each end",
            plan_3d::railing_post_count(len)
        ));
        ui.weak("Top rail, bottom rail and 3/4\" balusters about 4\" apart");
    }

    fn general(&mut self, ui: &mut Ui) {
        let is_default = self.target.is_default();
        section(ui, "General");
        ui.horizontal_wrapped(|ui| {
            let mut foundation = self.draft.class == WallClass::Foundation;
            let changed = ui
                .add_enabled(
                    !is_default,
                    egui::Checkbox::new(&mut foundation, "Foundation Wall"),
                )
                .changed();
            if changed {
                self.change_class(if foundation {
                    WallClass::Foundation
                } else {
                    WallClass::Standard
                });
            }
            let mut railing = self.draft.class == WallClass::Railing;
            let changed = ui
                .add_enabled(!is_default, egui::Checkbox::new(&mut railing, "Railing"))
                .changed();
            if changed {
                self.change_class(if railing {
                    WallClass::Railing
                } else {
                    WallClass::Standard
                });
            }
            dis_check(ui, "Terrain Retaining Wall", false);
            dis_check(ui, "Attic Wall", false);
        });
        if !is_default {
            self.class_rows(ui);
        }
        self.fields
            .length_row(ui, "Thickness", "thickness", &mut self.draft.thickness);
        // Chief's wall "Bottom" value: where the wall starts above its floor.
        ui.add_enabled_ui(!is_default, |ui| {
            self.fields.length_row(
                ui,
                "Bottom Height",
                "bottom_offset",
                &mut self.draft.bottom_offset,
            );
        });
        ui.add_enabled_ui(!is_default, |ui| {
            let mut len = self.draft.length();
            if self
                .fields
                .length_row(ui, "Wall Length", "length", &mut len)
            {
                self.set_length(len);
            }
            let mut angle = self.angle_deg();
            if self
                .fields
                .degrees_row(ui, "Wall Angle", "deg_angle", &mut angle)
            {
                self.set_angle(angle);
            }
            let tip = "The point that stays fixed when the wall length changes";
            row(ui, "Lock", |ui| {
                ui.radio_value(&mut self.lock, WallLock::Start, "Start")
                    .on_hover_text(tip);
                ui.radio_value(&mut self.lock, WallLock::Center, "Center")
                    .on_hover_text(tip);
                ui.radio_value(&mut self.lock, WallLock::End, "End")
                    .on_hover_text(tip);
            });
        });
        if !is_default {
            ui.weak("The angle rotates about the start point. Connected walls are not moved.");
        }

        section(ui, "Options");
        if is_default {
            for l in ["Invisible", "No Room Definition", "No Locate"] {
                dis_check(ui, l, false);
            }
        } else {
            let f = &mut self.draft.flags;
            ui.checkbox(&mut f.invisible, "Invisible");
            ui.checkbox(&mut f.no_room_definition, "No Room Definition");
            ui.checkbox(&mut f.no_locate, "No Locate");
        }
        for l in [
            "Lock Center",
            "No Room Moldings Exterior",
            "No Room Moldings Interior",
            "Automatically Generated Wall",
            "Ignored by Hide Exterior Walls",
        ] {
            dis_check(ui, l, false);
        }

        ui.add_enabled_ui(false, |ui| {
            section(ui, "Curved Wall");
            row(ui, "Radius to", |ui| {
                dis_radio(ui, "Outer Surface", false);
                dis_radio(ui, "Main Layer Outside", true);
            });
            row(ui, "Lock", |ui| {
                dis_radio(ui, "Arc Center", false);
                dis_radio(ui, "Ends", true);
            });
            dis_check(ui, "Automatic Facet Angle", true);
        });
    }

    fn structure(&mut self, ui: &mut Ui) {
        section(ui, "Default Wall Heights");
        if ui
            .checkbox(&mut self.default_top, "Default Wall Top Height")
            .changed()
            && self.default_top
        {
            self.draft.height = self.default_height;
        }
        ui.add_enabled_ui(!self.default_top, |ui| {
            self.fields
                .length_row(ui, "Wall Height", "height", &mut self.draft.height);
        });
        dis_check(ui, "Default Wall Bottom Height", true);

        ui.add_enabled_ui(false, |ui| {
            section(ui, "Platform Intersections");
            dis_check(
                ui,
                "Invisible Walls and Railings: Generate Between Platforms",
                true,
            );
            row(ui, "Ceiling Platform", |ui| {
                dis_radio(ui, "Automatic", true);
                dis_radio(ui, "Stop at Ceiling Above", false);
            });
            row(ui, "Floor Platform", |ui| {
                dis_radio(ui, "Automatic", true);
                dis_radio(ui, "Stop at Floor Below", false);
            });
            section(ui, "Wall Intersections");
            dis_check(ui, "Through Wall At End", false);
            dis_check(ui, "Through Wall At Start", false);
            section(ui, "Rim Joist");
            row(ui, "Rim Joist", |ui| {
                dis_radio(ui, "Automatic", true);
                dis_radio(ui, "Double", false);
                dis_radio(ui, "Single", false);
            });
            section(ui, "Double Wall");
            row(ui, "Double Wall", |ui| {
                dis_radio(ui, "Furred Wall", false);
                dis_radio(ui, "Split Framing", false);
                dis_radio(ui, "Frame Through", true);
            });
            section(ui, "Stud Layout");
            dis_check(ui, "Use Framing Reference", true);
            dis_check(ui, "Reverse Stud Rollout Direction", false);
            section(ui, "Framing");
            dis_check(ui, "Retain Wall Framing", false);
            dis_check(ui, "Bearing Wall", false);
            dis_check(ui, "Stagger Multiple Framing Layers", true);
            dis_check(ui, "Create Wall/Footing Below", false);
            dis_check(ui, "Insert Floor Framing Below", true);
        });
    }

    /// Roof tab (RF-18..RF-27): what Build Roof does at this wall.
    fn roof(&mut self, ui: &mut Ui) {
        use plan_core::defaults::RoofWallKind as K;
        section(ui, "Roof Options");
        let r = &mut self.draft.roof;
        for (kind, label) in [
            (K::Hip, "Hip Wall"),
            (K::FullGable, "Full Gable Wall"),
            (K::DutchGable, "Dutch Gable Wall"),
            (K::HighShedGable, "High Shed/Gable Wall"),
            (K::KneeWall, "Knee Wall"),
            (K::ExtendSlopeDownward, "Extend Slope Downward"),
        ] {
            ui.radio_value(&mut r.kind, kind, label);
        }
        if r.kind == K::ExtendSlopeDownward {
            let mut drop = r.extend_drop.unwrap_or(ROOF_EXTEND_DEFAULT);
            if self
                .fields
                .length_row(ui, "Drop below Eave", "roof_extend", &mut drop)
            {
                r.extend_drop = Some(drop.max(0.0));
            }
        }
        if r.kind == K::KneeWall {
            ui.weak("A knee wall makes no roof plane of its own; the roof passes over it.");
        }

        section(ui, "Pitch Options");
        let mut own = r.pitch_in_12.is_some();
        if ui.checkbox(&mut own, "Specify Pitch").changed() {
            r.pitch_in_12 = own.then_some(ROOF_PITCH_DEFAULT);
        }
        if let Some(p) = &mut r.pitch_in_12 {
            row(ui, "Pitch", |ui| {
                ui.add(
                    egui::DragValue::new(p)
                        .range(0.5..=24.0)
                        .speed(0.1)
                        .max_decimals(2)
                        .suffix(" : 12"),
                );
            });
        }
        let mut upper = r.upper_pitch.is_some();
        let label = if r.kind == K::DutchGable {
            "Starts Dutch Gable at Height"
        } else {
            "Upper Pitch"
        };
        if ui.checkbox(&mut upper, label).changed() {
            r.upper_pitch = upper.then_some((ROOF_UPPER_PITCH_DEFAULT, self.draft.height + 48.0));
        }
        if let Some((rise, start)) = &mut r.upper_pitch {
            if r.kind != K::DutchGable {
                row(ui, "Upper Pitch", |ui| {
                    ui.add(
                        egui::DragValue::new(rise)
                            .range(0.5..=60.0)
                            .speed(0.1)
                            .max_decimals(2)
                            .suffix(" : 12"),
                    );
                });
            }
            self.fields
                .length_row(ui, "Starts at Height", "roof_upper_start", start);
            ui.weak("Height above the floor of this wall.");
        }

        section(ui, "Overhang");
        let mut own = r.overhang.is_some();
        if ui.checkbox(&mut own, "Specify Overhang").changed() {
            r.overhang = own.then_some(ROOF_OVERHANG_DEFAULT);
        }
        if let Some(o) = &mut r.overhang {
            self.fields.length_row(ui, "Length", "roof_overhang", o);
            *o = o.max(0.0);
        }

        section(ui, "Auto Roof Return");
        ui.checkbox(&mut r.auto_roof_return, "Auto Roof Return");
        if r.auto_roof_return {
            let mut len = r.return_length.unwrap_or(ROOF_RETURN_DEFAULT);
            if self
                .fields
                .length_row(ui, "Length", "roof_return_length", &mut len)
            {
                r.return_length = Some(len.max(2.0));
            }
            ui.weak("Returns are made where this wall is a gable end.");
        }
    }

    fn wall_types(&mut self, ui: &mut Ui) {
        section(ui, "General");
        let current = self.current_type();
        let default_kind = self.target.is_default().then_some(self.draft.kind);
        row(ui, "Wall Type", |ui| {
            let text = match current {
                Some(i) => format!(
                    "{} ({})",
                    self.types[i].name,
                    super::fmt_short(self.types[i].thickness())
                ),
                None => format!("Custom ({})", super::fmt_short(self.draft.thickness)),
            };
            egui::ComboBox::from_id_salt("wall_type")
                .selected_text(text)
                .show_ui(ui, |ui| {
                    for (i, t) in self.types.iter().enumerate() {
                        // A default dialog keeps its own wall kind.
                        if default_kind.is_some_and(|k| k != t.kind) {
                            continue;
                        }
                        if current != Some(i) && !self.offers_type(t) {
                            continue;
                        }
                        let label = format!("{} ({})", t.name, super::fmt_short(t.thickness()));
                        if ui.selectable_label(current == Some(i), label).clicked() {
                            self.draft.thickness = t.thickness();
                            self.draft.kind = t.kind;
                            self.draft.wall_type = Some(t.name.clone());
                            self.extras.wall_type = Some(t.name.clone());
                            // A pony wall's picked type is its upper type.
                            if let WallClass::Pony { upper_type, .. } = &mut self.draft.class {
                                *upper_type = t.name.clone();
                            }
                        }
                    }
                });
            if ui.button("Define\u{2026}").clicked() && self.define.is_none() {
                let current = self
                    .current_type()
                    .and_then(|i| self.types.get(i))
                    .map(|t| t.name.clone());
                self.define = Some(WallTypeDialog::new(
                    self.types.clone(),
                    current.as_deref(),
                    self.draft.resize_about,
                ));
            }
            ui.add_enabled(false, egui::Button::new("Library\u{2026}"));
        });
        if let Some(t) = current.map(|i| &self.types[i]) {
            ui.weak(format!(
                "Main layer starts {} in from the exterior face",
                super::fmt_short(t.main_layer_offset())
            ));
        }
        ui.add_space(6.0);
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width().min(420.0), 120.0),
            egui::Sense::hover(),
        );
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 3.0, super::PV_BG);
        layer_stack(&painter, rect.shrink(8.0), &self.layers());

        if self.target.is_default() {
            ui.add_enabled_ui(false, |ui| {
                section(ui, "Pony Wall");
                dis_check(ui, "Pony Wall", false);
            });
        } else {
            self.pony_section(ui);
        }
    }

    fn layer(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        row(ui, "Layer", |ui| {
            dis_check(ui, "Default", true);
            egui::ComboBox::from_id_salt("wall_layer")
                .selected_text(self.draft.layer.clone())
                .show_ui(ui, |ui| {
                    for name in WALL_LAYERS {
                        ui.selectable_value(&mut self.draft.layer, name.to_string(), name);
                    }
                });
        });
        row(ui, "Drawing Group", |ui| {
            dis_combo(ui, "wall_group", "Default: 29 \u{2013} Wall")
        });
    }

    fn label(&mut self, ui: &mut Ui) {
        section(ui, "Display Options");
        let e = &mut self.extras;
        session_check(ui, &mut e.suppress_label, "Suppress Label in All Views");
        ui.checkbox(&mut e.display_in_plan, "Display in Plan View");
        section(ui, "Label Content");
        ui.radio_value(&mut e.specify_label, false, "Automatic Label");
        ui.radio_value(&mut e.specify_label, true, "Specify Label");
        ui.add_enabled(
            e.specify_label,
            egui::TextEdit::singleline(&mut e.label_text).desired_width(260.0),
        );
        if !e.specify_label {
            let name = self
                .current_type()
                .map_or("Custom", |i| self.types[i].name.as_str());
            ui.weak(format!(
                "Automatic label: Wall \u{2013} {name} \u{2013} {}",
                fmt_ft_in(self.draft.length())
            ));
        }
        ui.add_enabled_ui(false, |ui| {
            section(ui, "Appearance");
            dis_check(ui, "Display Border", false);
            row(ui, "Text Style", |ui| {
                dis_combo(ui, "wall_text_style", "Use Layer Text Style")
            });
            row(ui, "Alignment", |ui| dis_combo(ui, "wall_align", "Left"));
            dis_check(ui, "Auto Adjust Text Direction", true);
            section(ui, "Label Layer");
            dis_radio(ui, "Use System Layer (Walls, Labels)", true);
            dis_radio(ui, "Use Object Layer (Walls, Normal)", false);
        });
    }
}

impl SpecPages for WallForm {
    fn tabs(&self) -> &'static [Tab] {
        if self.draft.kind == WallKind::Exterior && !self.target.is_default() {
            WALL_TABS_EXTERIOR
        } else {
            WALL_TABS
        }
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        if self.draft.thickness <= 0.0 {
            return Some("Thickness must be greater than zero".into());
        }
        if self.draft.height <= 0.0 {
            return Some("Wall height must be greater than zero".into());
        }
        if self.draft.bottom_offset < 0.0 {
            return Some("Bottom height cannot be negative".into());
        }
        if let Some((rise, _)) = self.draft.roof.upper_pitch {
            if rise <= 0.0 {
                return Some("Upper pitch must be greater than zero".into());
            }
        }
        let len = self.draft.length();
        let too_short = self.adjusted_openings().iter().any(|o| {
            o.start_offset() < OPENING_MARGIN - 1e-6 || o.end_offset() > len - OPENING_MARGIN + 1e-6
        });
        too_short.then(|| "Wall is too short for its openings".to_string())
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match self.tabs()[tab].name {
            "General" => self.general(ui),
            "Structure" => self.structure(ui),
            "Roof" => self.roof(ui),
            "Wall Types" => self.wall_types(ui),
            "Rail Style" => self.rail_style(ui),
            "Layer" => self.layer(ui),
            "Label" => self.label(ui),
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        let top = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.center().y - 4.0));
        let bottom = Rect::from_min_max(Pos2::new(rect.min.x, rect.center().y + 4.0), rect.max);
        pv_text(
            p,
            top.min + egui::vec2(0.0, 4.0),
            Align2::LEFT_CENTER,
            "Plan view",
            11.0,
        );
        let plan = Rect::from_min_max(top.min + egui::vec2(0.0, 10.0), top.max);
        wall_plan_sketch(
            p,
            plan,
            self.draft.length(),
            self.draft.thickness,
            &self.adjusted_openings(),
            None,
            90.0,
        );
        p.hline(
            rect.x_range(),
            rect.center().y,
            Stroke::new(0.8_f32, PV_FAINT),
        );
        pv_text(
            p,
            bottom.min + egui::vec2(0.0, 6.0),
            Align2::LEFT_CENTER,
            "Section",
            11.0,
        );
        layer_stack(
            p,
            Rect::from_min_max(bottom.min + egui::vec2(0.0, 12.0), bottom.max),
            &self.layers(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialog_for(wall: Wall) -> WallDialog {
        WallDialog::new(
            WallTarget::Wall(wall.id),
            wall,
            Vec::new(),
            WallExtras::default(),
            109.125,
            Vec::new(),
        )
    }

    #[test]
    fn label_options_and_last_type_persist_with_the_wall() {
        let wall = crate::editor::ops::make_wall(
            3,
            Point::ZERO,
            Point::new(144.0, 0.0),
            4.5,
            109.125,
            WallKind::Interior,
        );
        let mut d = dialog_for(wall);
        assert_eq!(d.draft().extras, StoredExtras::default());
        {
            let e = &mut d.form.extras;
            e.specify_label = true;
            e.label_text = "Bearing".into();
            e.display_in_plan = false;
            e.wall_type = Some("Interior-4".into());
        }
        d.sync_stored();
        let saved = d.draft().clone();
        assert_eq!(saved.extras.label_text.as_deref(), Some("Bearing"));
        assert!(!saved.extras.display_label);
        assert_eq!(saved.extras.last_wall_type.as_deref(), Some("Interior-4"));

        // Through the file and into a dialog with a fresh session.
        let back: Wall = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
        let d2 = dialog_for(back);
        let e = d2.extras();
        assert!(e.specify_label && e.label_text == "Bearing" && !e.display_in_plan);
        assert_eq!(d2.picked_type(), Some("Interior-4"));
    }

    #[test]
    fn automatic_label_stores_no_text() {
        let wall = crate::editor::ops::make_wall(
            3,
            Point::ZERO,
            Point::new(144.0, 0.0),
            4.5,
            109.125,
            WallKind::Interior,
        );
        let mut d = dialog_for(wall);
        d.form.extras.label_text = "stale".into();
        d.sync_stored();
        assert_eq!(d.draft().extras.label_text, None);
        assert!(d.draft().extras.display_label);
    }

    fn form(lock: WallLock) -> WallForm {
        let wall = crate::editor::ops::make_wall(
            1,
            Point::new(10.0, 10.0),
            Point::new(110.0, 10.0),
            4.5,
            109.125,
            WallKind::Interior,
        );
        let mut opening = Opening::default_door(2, 1, 50.0);
        opening.width = 30.0;
        WallForm {
            target: WallTarget::Wall(1),
            orig_start: wall.start,
            draft: wall,
            extras: WallExtras::default(),
            openings: vec![opening],
            lock,
            default_height: 109.125,
            default_top: true,
            fields: Fields::default(),
            types: plan_core::PlanDefaults::chief_x18_daniel().wall_types,
            define: None,
            edited_types: Vec::new(),
        }
    }

    #[test]
    fn length_lock_start_moves_end() {
        let mut f = form(WallLock::Start);
        f.set_length(150.0);
        assert!((f.draft.start.x - 10.0).abs() < 1e-9);
        assert!((f.draft.end.x - 160.0).abs() < 1e-9);
        assert!((f.adjusted_openings()[0].center_offset - 50.0).abs() < 1e-9);
    }

    #[test]
    fn length_lock_end_keeps_openings_in_place() {
        let mut f = form(WallLock::End);
        f.set_length(80.0);
        assert!((f.draft.end.x - 110.0).abs() < 1e-9);
        assert!((f.draft.start.x - 30.0).abs() < 1e-9);
        // The door was 50" from the old start (x = 60) and is now 30" from x = 30.
        assert!((f.adjusted_openings()[0].center_offset - 30.0).abs() < 1e-9);
        assert!(f.error().is_none());
        f.set_length(40.0);
        assert!(f.error().is_some());
    }

    #[test]
    fn angle_rotates_about_start() {
        let mut f = form(WallLock::Start);
        f.set_angle(90.0);
        assert!((f.draft.start.x - 10.0).abs() < 1e-9);
        assert!((f.draft.end.x - 10.0).abs() < 1e-6);
        assert!((f.draft.end.y - 110.0).abs() < 1e-6);
        assert!((f.angle_deg() - 90.0).abs() < 1e-9);
    }

    #[test]
    fn picking_a_class_sets_its_type_layer_and_flags() {
        let mut f = form(WallLock::Start);
        f.draft.kind = WallKind::Exterior;
        f.draft.wall_type = Some("Stucco-6".into());
        f.change_class(WallClass::Glass);
        assert_eq!(f.draft.wall_type.as_deref(), Some("Glass-1"));
        assert_eq!(f.draft.thickness, 1.0);
        assert_eq!(f.extras.wall_type.as_deref(), Some("Glass-1"));
        f.change_class(WallClass::Standard);
        assert_eq!(f.draft.wall_type.as_deref(), Some("Stucco-6"));
        assert_eq!(f.draft.thickness, 7.625);

        f.change_class(WallClass::RoomDivider);
        assert_eq!(f.draft.layer, "Walls, Invisible");
        assert!(f.draft.flags.room_divider && f.draft.flags.invisible);
        f.change_class(WallClass::Standard);
        assert_eq!(f.draft.layer, "Walls, Normal");
        assert!(!f.draft.flags.room_divider && !f.draft.flags.invisible);

        f.change_class(WallClass::Foundation);
        assert_eq!(f.draft.wall_type.as_deref(), Some("Foundation-8"));
        assert!(f.draft.flags.foundation);

        f.change_class(WallClass::HalfWall { height: 36.0 });
        assert_eq!(f.draft.height, 36.0);
        f.change_class(WallClass::DeckRailing);
        assert_eq!(f.draft.layer, "Deck Railing");
        assert!(f.draft.flags.railing);
    }

    #[test]
    fn a_pony_wall_starts_with_the_default_types_and_split() {
        let mut f = form(WallLock::Start);
        f.draft.kind = WallKind::Exterior;
        f.draft.wall_type = Some("Stucco-6".into());
        let class = f.class_from_index(2);
        assert_eq!(
            class,
            WallClass::Pony {
                upper_type: "Stucco-6".into(),
                lower_type: "Foundation-8".into(),
                split_height: 36.0,
                upper_sets_plan_display: false,
            }
        );
        f.change_class(class);
        assert_eq!(f.draft.flags.pony.as_ref().unwrap().lower_height, 36.0);
        // Pony types are ordinary wall types; glass and rail types are not offered.
        let glass = f
            .types
            .iter()
            .find(|t| t.name == "Glass-1")
            .unwrap()
            .clone();
        let stucco = f
            .types
            .iter()
            .find(|t| t.name == "Stucco-6")
            .unwrap()
            .clone();
        assert!(f.offers_type(&stucco) && !f.offers_type(&glass));
        f.change_class(WallClass::Glass);
        assert!(f.offers_type(&glass) && !f.offers_type(&stucco));
    }

    /// Every text the general page draws.
    fn page_texts(f: &mut WallForm, tab: usize) -> Vec<String> {
        fn texts(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|x| texts(x, out)),
                _ => {}
            }
        }
        let ctx = egui::Context::default();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| f.page(ui, tab));
        });
        let mut all = Vec::new();
        for c in &out.shapes {
            texts(&c.shape, &mut all);
        }
        all
    }

    #[test]
    fn the_general_page_has_a_bottom_height_that_must_not_be_negative() {
        let mut f = form(WallLock::Start);
        let general = WALL_TABS.iter().position(|t| t.name == "General").unwrap();
        let drawn = page_texts(&mut f, general);
        assert!(drawn.iter().any(|t| t == "Bottom Height"), "{drawn:?}");
        // Defaults to the floor; a raised wall keeps its value through the JSON.
        assert_eq!(f.draft.bottom_offset, 0.0);
        assert!(f.error().is_none());
        f.draft.bottom_offset = 42.0;
        assert!(f.error().is_none());
        let back: Wall = serde_json::from_str(&serde_json::to_string(&f.draft).unwrap()).unwrap();
        assert_eq!(back.bottom_offset, 42.0);
        f.draft.bottom_offset = -1.0;
        assert_eq!(
            f.error().as_deref(),
            Some("Bottom height cannot be negative")
        );
    }

    #[test]
    fn the_roof_tab_of_an_exterior_wall_edits_the_directive() {
        use plan_core::defaults::RoofWallKind;
        let mut f = form(WallLock::Start);
        f.draft.kind = WallKind::Interior;
        assert!(f.tabs().iter().any(|t| t.name == "Roof" && !t.enabled));
        f.draft.kind = WallKind::Exterior;
        let roof = f.tabs().iter().position(|t| t.name == "Roof").unwrap();
        assert!(f.tabs()[roof].enabled);
        f.draft.roof.kind = RoofWallKind::ExtendSlopeDownward;
        f.draft.roof.auto_roof_return = true;
        f.draft.roof.upper_pitch = Some((18.0, 120.0));
        f.draft.roof.pitch_in_12 = Some(6.0);
        let drawn = page_texts(&mut f, roof);
        for want in [
            "Dutch Gable Wall",
            "Knee Wall",
            "Extend Slope Downward",
            "Drop below Eave",
            "Upper Pitch",
            "Starts at Height",
            "Auto Roof Return",
        ] {
            assert!(drawn.iter().any(|t| t == want), "{want} in {drawn:?}");
        }
        assert!(f.error().is_none());
        f.draft.roof.upper_pitch = Some((0.0, 120.0));
        assert!(f.error().is_some());
    }

    #[test]
    fn the_class_stays_with_the_wall_through_the_json() {
        let mut f = form(WallLock::Start);
        f.change_class(WallClass::Fencing {
            style: FenceStyle::Privacy,
        });
        f.draft.foundation_height = 30.0;
        let back: Wall = serde_json::from_str(&serde_json::to_string(&f.draft).unwrap()).unwrap();
        assert_eq!(
            back.class,
            WallClass::Fencing {
                style: FenceStyle::Privacy
            }
        );
        assert_eq!(
            (back.layer.as_str(), back.foundation_height),
            ("Fencing", 30.0)
        );
    }
}
