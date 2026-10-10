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
use plan_core::walls::spec::{default_cap_profiles, min_thickness};
use plan_core::walls::{
    scale_opening_offset, ArcLock, CapPosition, CeilingPlatform, FloorPlatform,
    DEFAULT_HALF_WALL_HEIGHT, DEFAULT_PONY_SPLIT,
};
use plan_core::{
    FenceStyle, Id, Opening, Project, ResizeAbout, Wall, WallClass, WallCurve, WallKind,
    WallTypeDef,
};
use std::collections::{HashMap, HashSet};

mod tabs;

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

/// The tabs of a placed wall, exterior or interior: its Roof tab is live
/// (an interior wall can be a gable or knee wall too).
const WALL_TABS_EXTERIOR: &[Tab] = &[
    on("General"),
    on("Structure"),
    on("Roof"),
    on("Foundation"),
    on("Wall Types"),
    on("Wall Cap"),
    on("Wall Covering"),
    on("Rail Style"),
    on("Newels/Balusters"),
    on("Rails"),
    on("Layer"),
    on("Materials"),
    on("Label"),
    on("Components"),
    on("Object Information"),
    on("Schedule"),
];

/// The tabs of the Wall Specification over several walls (W-83): the fields
/// every wall has.
const WALL_TABS_MULTI: &[Tab] = &[
    on("General"),
    on("Structure"),
    on("Roof"),
    on("Foundation"),
    on("Wall Types"),
    on("Wall Cap"),
    on("Layer"),
];

const WALL_TABS: &[Tab] = &[
    on("General"),
    on("Structure"),
    off("Roof"),
    on("Foundation"),
    on("Wall Types"),
    on("Wall Cap"),
    on("Wall Covering"),
    on("Rail Style"),
    on("Newels/Balusters"),
    on("Rails"),
    on("Layer"),
    on("Materials"),
    on("Label"),
    on("Components"),
    on("Object Information"),
    on("Schedule"),
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

/// The reference lines the Radius field can measure to (W-66).
const RADIUS_TO: [(ResizeAbout, &str); 5] = [
    (ResizeAbout::OuterSurface, "Outer Surface"),
    (ResizeAbout::MainLayerOutside, "Main Layer Outside"),
    (ResizeAbout::WallCenter, "Wall Center"),
    (ResizeAbout::MainLayerInside, "Main Layer Inside"),
    (ResizeAbout::InnerSurface, "Inner Surface"),
];

/// Which wall point stays put when the length is edited.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum WallLock {
    Start,
    Center,
    End,
}

/// The Retain Wall Framing box: Build Framing leaves the framing of retained
/// walls as it is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Retain {
    value: bool,
    /// The walls disagree and the box was not clicked yet.
    mixed: bool,
    touched: bool,
}

/// What a dialog over several walls tracks (W-83).
struct Multi {
    ids: Vec<Id>,
    originals: Vec<Wall>,
    /// Keys whose value is not the same in every wall.
    mixed: HashSet<&'static str>,
    /// Keys the user edited: only these are written to the walls.
    touched: HashSet<&'static str>,
    /// Text of the mixed length fields being typed.
    bufs: HashMap<&'static str, String>,
    default_heights: [f64; 2],
}

/// The fields a multi-wall dialog edits, by key, as comparable text.
fn field_values(w: &Wall) -> Vec<(&'static str, String)> {
    let (st, f, c) = (&w.spec.structure, &w.spec.foundation, &w.spec.cap);
    let n = |v: f64| format!("{v:.4}");
    vec![
        ("thickness", n(w.thickness)),
        ("wall_type", format!("{:?}", w.wall_type)),
        ("bottom", n(w.bottom_offset)),
        ("height", n(w.height)),
        ("default_top", format!("{}", !st.custom_top)),
        (
            "default_bottom",
            format!("{}", st.default_bottom(w.bottom_offset)),
        ),
        ("invisible", format!("{}", w.flags.invisible)),
        (
            "no_room_definition",
            format!("{}", w.flags.no_room_definition),
        ),
        ("no_locate", format!("{}", w.flags.no_locate)),
        ("layer", w.layer.clone()),
        ("gen_between", format!("{}", st.generate_between_platforms)),
        ("ceiling_platform", format!("{:?}", st.ceiling_platform)),
        ("floor_platform", format!("{:?}", st.floor_platform)),
        ("through_start", format!("{}", st.through_at_start)),
        ("through_end", format!("{}", st.through_at_end)),
        ("bearing_wall", format!("{}", st.bearing_wall)),
        ("roof", format!("{:?}", w.roof)),
        ("slab_footing", format!("{}", f.slab_footing)),
        ("footing", format!("{}", f.footing)),
        ("footing_width", n(f.footing_width)),
        ("footing_height", n(f.footing_height)),
        ("footing_auto", format!("{}", f.auto_bottom)),
        ("footing_bottom", n(f.footing_bottom)),
        ("footing_vertical", format!("{}", f.vertical_footing)),
        ("footing_offset", n(f.footing_offset)),
        ("footing_center", format!("{}", f.center_on_main_layer)),
        ("footing_outside", format!("{}", f.align_on_outside)),
        ("chamfer_mono", format!("{}", f.chamfer_monolithic)),
        ("chamfer_regular", format!("{}", f.chamfer_regular)),
        ("chamfer_width", n(f.chamfer_width)),
        ("chamfer_height", n(f.chamfer_height)),
        ("pour", format!("{}", f.pour_number)),
        ("sill_plate", format!("{}", f.sill_plate)),
        ("cap", format!("{}", c.enabled)),
        ("cap_profile", c.profile.clone()),
        ("cap_full", format!("{}", c.full_wall_width)),
        ("cap_position", format!("{:?}", c.position)),
        ("cap_split_pony", format!("{}", c.split_pony_wall)),
    ]
}

/// The keys whose values differ between `walls`.
fn mixed_keys(walls: &[Wall]) -> HashSet<&'static str> {
    let mut out = HashSet::new();
    let Some((first, rest)) = walls.split_first() else {
        return out;
    };
    let base = field_values(first);
    for w in rest {
        for ((key, a), (_, b)) in base.iter().zip(field_values(w)) {
            if *a != b {
                out.insert(*key);
            }
        }
    }
    out
}

/// Copies the field `key` of `from` onto `to` (thickness and wall type are
/// the project's business: they hold reference lines).
fn apply_field(key: &str, from: &Wall, to: &mut Wall, default_heights: &[f64; 2]) {
    let (fs, ts) = (&from.spec, &mut to.spec);
    match key {
        "bottom" => {
            to.bottom_offset = from.bottom_offset;
            ts.structure.custom_bottom = fs.structure.custom_bottom;
        }
        "height" => to.height = from.height,
        "default_top" => {
            ts.structure.custom_top = fs.structure.custom_top;
            if !ts.structure.custom_top {
                to.height = default_heights[usize::from(to.kind == WallKind::Interior)];
            }
        }
        "default_bottom" => {
            ts.structure.custom_bottom = fs.structure.custom_bottom;
            if !ts.structure.custom_bottom {
                to.bottom_offset = 0.0;
            }
        }
        "invisible" => to.flags.invisible = from.flags.invisible,
        "no_room_definition" => to.flags.no_room_definition = from.flags.no_room_definition,
        "no_locate" => to.flags.no_locate = from.flags.no_locate,
        "layer" => to.layer = from.layer.clone(),
        "gen_between" => {
            ts.structure.generate_between_platforms = fs.structure.generate_between_platforms
        }
        "ceiling_platform" => ts.structure.ceiling_platform = fs.structure.ceiling_platform,
        "floor_platform" => ts.structure.floor_platform = fs.structure.floor_platform,
        "through_start" => ts.structure.through_at_start = fs.structure.through_at_start,
        "through_end" => ts.structure.through_at_end = fs.structure.through_at_end,
        "bearing_wall" => ts.structure.bearing_wall = fs.structure.bearing_wall,
        "roof" => to.roof = from.roof.clone(),
        "slab_footing" => ts.foundation.slab_footing = fs.foundation.slab_footing,
        "footing" => ts.foundation.footing = fs.foundation.footing,
        "footing_width" => ts.foundation.footing_width = fs.foundation.footing_width,
        "footing_height" => ts.foundation.footing_height = fs.foundation.footing_height,
        "footing_auto" => ts.foundation.auto_bottom = fs.foundation.auto_bottom,
        "footing_bottom" => ts.foundation.footing_bottom = fs.foundation.footing_bottom,
        "footing_vertical" => ts.foundation.vertical_footing = fs.foundation.vertical_footing,
        "footing_offset" => ts.foundation.footing_offset = fs.foundation.footing_offset,
        "footing_center" => ts.foundation.center_on_main_layer = fs.foundation.center_on_main_layer,
        "footing_outside" => ts.foundation.align_on_outside = fs.foundation.align_on_outside,
        "chamfer_mono" => ts.foundation.chamfer_monolithic = fs.foundation.chamfer_monolithic,
        "chamfer_regular" => ts.foundation.chamfer_regular = fs.foundation.chamfer_regular,
        "chamfer_width" => ts.foundation.chamfer_width = fs.foundation.chamfer_width,
        "chamfer_height" => ts.foundation.chamfer_height = fs.foundation.chamfer_height,
        "pour" => ts.foundation.pour_number = fs.foundation.pour_number,
        "sill_plate" => ts.foundation.sill_plate = fs.foundation.sill_plate,
        "cap" => ts.cap.enabled = fs.cap.enabled,
        "cap_profile" => ts.cap.profile = fs.cap.profile.clone(),
        "cap_full" => ts.cap.full_wall_width = fs.cap.full_wall_width,
        "cap_position" => ts.cap.position = fs.cap.position,
        "cap_split_pony" => ts.cap.split_pony_wall = fs.cap.split_pony_wall,
        _ => {}
    }
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
    /// How far the length and angle edits so far moved the wall's start along
    /// the wall, so its openings keep their distance from the locked point.
    open_shift: f64,
    lock: WallLock,
    /// The reference line the Radius field measures to ("Radius to").
    radius_to: ResizeAbout,
    /// What a radius or arc angle edit holds fixed ("Lock", W-66).
    arc_lock: ArcLock,
    /// How much the arc edits made so far changed the path length by; the
    /// openings keep their proportion along the wall.
    arc_scale: f64,
    default_height: f64,
    default_top: bool,
    /// The thickness the wall had when the dialog opened (a wall already
    /// thinner than its type allows may keep it).
    orig_thickness: f64,
    /// Set when the dialog edits several walls at once (W-83).
    multi: Option<Multi>,
    /// Retain Wall Framing: lives in the plan's framing settings, not in the
    /// wall, so the dialog tracks it on its own.
    retain: Retain,
    fields: Fields,
    /// The library the Materials and Wall Covering tabs pick from, loaded when
    /// first needed, and their search text.
    material_lib: Option<plan_materials::MaterialLibrary>,
    material_filter: String,
    /// The wall types offered in the Wall Types tab (`PlanDefaults::wall_types`).
    types: Vec<WallTypeDef>,
    /// The Wall Type Definitions dialog, while open.
    define: Option<WallTypeDialog>,
    /// Wall types edited or created in that dialog (to store with the plan
    /// and the defaults on OK).
    edited_types: Vec<WallTypeDef>,
    /// An attic wall stands above this wall: Combine with Above Wall is
    /// offered (manual p. 429).
    attic_above: bool,
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
        let default_top =
            !wall.spec.structure.custom_top && (wall.height - default_height).abs() < 1e-6;
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
                orig_thickness: wall.thickness,
                open_shift: 0.0,
                draft: wall,
                extras,
                openings,
                lock: WallLock::Start,
                radius_to: ResizeAbout::WallCenter,
                arc_lock: ArcLock::Ends,
                arc_scale: 1.0,
                default_height,
                default_top,
                multi: None,
                retain: Retain::default(),
                fields: Fields::default(),
                material_lib: None,
                material_filter: String::new(),
                types,
                define: None,
                edited_types: Vec::new(),
                attic_above: false,
            },
        }
    }

    /// Tells the dialog an attic wall stands above the wall, so Combine with
    /// Above Wall on the Roof tab can be used.
    pub fn set_attic_above(&mut self, above: bool) {
        self.form.attic_above = above;
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

    /// The Wall Specification over several walls (W-83, Open Object with a
    /// selection of walls). `default_heights` are the default top heights of
    /// exterior and interior walls. Fields whose values differ between the
    /// walls show Chief's indeterminate state, and only the fields edited
    /// are written back by [`WallDialog::apply_multi`].
    pub fn multi(walls: Vec<Wall>, default_heights: [f64; 2], types: Vec<WallTypeDef>) -> Self {
        let first = walls.first().cloned().expect("at least one wall");
        let mut d = Self::new(
            WallTarget::Wall(first.id),
            first,
            Vec::new(),
            WallExtras::default(),
            default_heights[0],
            types,
        );
        let mixed = mixed_keys(&walls);
        d.frame = SpecDialog::new("Wall Specification (Multiple Walls)", "wall_multi");
        d.form.multi = Some(Multi {
            ids: walls.iter().map(|w| w.id).collect(),
            originals: walls,
            mixed,
            touched: HashSet::new(),
            bufs: HashMap::new(),
            default_heights,
        });
        d
    }

    /// The walls a multi-wall dialog edits (`None` for a one-wall dialog).
    pub fn multi_ids(&self) -> Option<&[Id]> {
        self.form.multi.as_ref().map(|m| m.ids.as_slice())
    }

    /// Keys of the fields the user edited so far (multi-wall dialogs).
    #[cfg(test)]
    pub fn touched(&self) -> Vec<&'static str> {
        let mut v: Vec<_> = self
            .form
            .multi
            .iter()
            .flat_map(|m| m.touched.iter().copied())
            .collect();
        v.sort_unstable();
        v
    }

    /// Tests edit a field of the draft and mark it edited, as the page does.
    #[cfg(test)]
    pub fn edit_field(&mut self, key: &'static str, edit: impl FnOnce(&mut Wall)) {
        edit(&mut self.form.draft);
        self.form.touch(key);
    }

    /// Whether the field `key` shows the mixed state (multi-wall dialogs).
    #[cfg(test)]
    pub fn is_mixed(&self, key: &str) -> bool {
        self.form.mixed(key)
    }

    /// Writes the edited fields of a multi-wall dialog to every wall of floor
    /// `floor`; the caller opens the undo step. A thickness edit keeps each
    /// wall's reference line (W-27) and a wall type change goes through
    /// [`Project::set_wall_type`]. Returns how many walls changed.
    pub fn apply_multi(&self, project: &mut Project, floor: usize) -> usize {
        let Some(m) = &self.form.multi else {
            return 0;
        };
        if m.touched.is_empty() {
            return 0;
        }
        let draft = &self.form.draft;
        let mut changed = 0;
        for id in &m.ids {
            let Some(before) = project.floors[floor].wall(*id).cloned() else {
                continue;
            };
            if let (true, Some(name)) = (m.touched.contains("wall_type"), &draft.wall_type) {
                if let Some(def) = self.form.types.iter().find(|t| &t.name == name) {
                    let about = before.resize_about;
                    project.set_wall_type(floor, *id, def, about);
                    if let Some(w) = project.floors[floor].wall_mut(*id) {
                        w.kind = def.kind;
                    }
                }
            }
            if m.touched.contains("thickness") {
                project.set_wall_thickness_about(floor, *id, draft.thickness);
            }
            if let Some(w) = project.floors[floor].wall_mut(*id) {
                for key in &m.touched {
                    apply_field(key, draft, w, &m.default_heights);
                }
            }
            if project.floors[floor]
                .wall(*id)
                .is_some_and(|w| !plan_core::joins::walls_equal(std::slice::from_ref(w), &[before]))
            {
                changed += 1;
            }
        }
        changed
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
        // A wall whose "Default Wall Top Height" box is off keeps its own
        // height when the floor's ceiling height changes (W-60).
        if self.form.multi.is_none() {
            self.form.draft.spec.structure.custom_top = !self.form.default_top;
        }
    }

    /// Hands the dialog the Retain Wall Framing state of each wall it edits
    /// (from the plan's framing settings).
    pub fn set_framing_retained(&mut self, states: &[bool]) {
        let first = states.first().copied().unwrap_or(false);
        self.form.retain = Retain {
            value: first,
            mixed: states.iter().any(|s| *s != first),
            touched: false,
        };
    }

    /// The Retain Wall Framing value to write to the dialog's walls, when the
    /// box was clicked.
    pub fn retain_framing_change(&self) -> Option<bool> {
        self.form.retain.touched.then_some(self.form.retain.value)
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

    /// Draws the tab called `tab` in a headless frame, so a test runs that
    /// page's code; false when the dialog has no live tab of that name.
    #[cfg(test)]
    pub fn draw_tab_for_test(&mut self, ctx: &egui::Context, tab: &str) -> bool {
        let Some(i) = self
            .form
            .tabs()
            .iter()
            .position(|t| t.name == tab && t.enabled)
        else {
            return false;
        };
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| self.form.page(ui, i));
        });
        self.sync_stored();
        true
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

    /// The openings after the edits: a straight wall keeps their distance
    /// from the locked end; a curved one scales them with the path length
    /// (making a wall curved, or changing its radius, stretches the
    /// centerline) so each keeps its proportion along the wall.
    fn adjusted_openings(&self) -> Vec<Opening> {
        let shift = if self.draft.is_curved() {
            0.0
        } else {
            self.open_shift
        };
        let k = self.arc_scale;
        let len = self.draft.path_length();
        self.openings
            .iter()
            .map(|o| {
                let mut o = o.clone();
                o.center_offset += shift;
                if (k - 1.0).abs() > 1e-9 {
                    scale_opening_offset(&mut o, k, len);
                }
                o
            })
            .collect()
    }

    /// The wall type the draft stands on, for the layer positions.
    fn draft_type(&self) -> Option<&WallTypeDef> {
        let name = self.draft.wall_type.as_deref()?;
        self.types.iter().find(|t| t.name == name)
    }

    /// Notes that an arc edit changed the path length from `old`.
    fn track_path(&mut self, old: f64) {
        if old > 1e-9 {
            self.arc_scale *= self.draft.path_length() / old;
        }
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
        // The openings keep their distance from the locked point: the start
        // moves back by the change (End lock) or half of it (Center lock).
        let delta = len - self.draft.length();
        self.open_shift += match self.lock {
            WallLock::Start => 0.0,
            WallLock::Center => delta * 0.5,
            WallLock::End => delta,
        };
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

    /// Rotates the wall about its Lock point (the start, the center or the
    /// end), keeping the length (W-76). The walls joined to the ends follow
    /// when the dialog is accepted (`wall_edit::follow_moved_ends`).
    fn set_angle(&mut self, deg: f64) {
        let a = deg.to_radians();
        let len = self.draft.length().max(1.0);
        let d = Point::new(a.cos(), a.sin());
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
        if self
            .fields
            .length_row(ui, "Railing Height", "rail_height", &mut self.draft.height)
        {
            // The Rails tab's top of rail follows the height set here.
            self.draft.spec.railing.top_rail_top = Some(self.draft.height);
        }
        super::code_notice::code_notice(
            ui,
            "IRC R312.1.2 guard height",
            &mut self.draft.height,
            crate::editor::code::active().guard_height,
            super::code_notice::LimitKind::Min,
        );
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
            if self.fields.length_row(
                ui,
                "Bottom Height",
                "bottom_offset",
                &mut self.draft.bottom_offset,
            ) {
                // A bottom off the floor is no longer the default one.
                self.draft.spec.structure.custom_bottom = self.draft.bottom_offset.abs() > 1e-9;
            }
        });
        ui.add_enabled_ui(!is_default, |ui| {
            let mut len = self.draft.length();
            let len_label = if self.draft.is_curved() {
                "Chord Length"
            } else {
                "Wall Length"
            };
            if self.fields.length_row(ui, len_label, "length", &mut len) {
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
            ui.weak("The angle turns the wall about its Lock point; the walls joined to its ends follow.");
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
            "Ignored by Hide Exterior Walls",
        ] {
            dis_check(ui, l, false);
        }
        // Set by the program on the invisible walls between platforms (W-25,
        // W-63); it cannot be turned on by hand.
        dis_check(
            ui,
            "Automatically Generated Wall",
            self.draft.flags.auto_generated,
        );

        self.arc_section(ui, is_default);
    }

    /// The Arc section (W-64..W-67): Change Line/Arc as a check box, then the
    /// arc by number. Radius, arc angle and rise each recompute the others
    /// from the chord, which the start and end points fix.
    fn arc_section(&mut self, ui: &mut Ui, is_default: bool) {
        section(ui, "Arc");
        ui.add_enabled_ui(!is_default, |ui| {
            let mut curved = self.draft.is_curved();
            if ui
                .checkbox(&mut curved, "Curved Wall (Change Line/Arc)")
                .changed()
            {
                self.set_curved(curved);
            }
            let Some(c) = self.draft.curve.filter(|c| !c.is_straight()) else {
                return;
            };
            let chord = self.draft.length();
            let mut radius = self
                .draft
                .radius_to(self.draft_type(), self.radius_to)
                .unwrap_or(0.0);
            if self
                .fields
                .length_row(ui, "Radius", "arc_radius", &mut radius)
            {
                self.set_radius(radius);
            }
            let mut sweep = c.sweep_abs(chord).to_degrees();
            if self
                .fields
                .degrees_row(ui, "Arc Angle", "deg_arc_angle", &mut sweep)
            {
                self.set_sweep_deg(sweep);
            }
            let mut rise = c.bulge.abs();
            if self
                .fields
                .length_row(ui, "Arc Rise", "arc_rise", &mut rise)
            {
                self.set_rise(rise);
            }
            row(ui, "Bulges", |ui| {
                let mut left = c.bulge > 0.0;
                let a = ui.radio_value(&mut left, true, "Left of start to end");
                let b = ui.radio_value(&mut left, false, "Right");
                if a.changed() || b.changed() {
                    self.set_side(left);
                }
            });
            if let Some((center, _)) = self.draft.arc_center_radius() {
                ui.weak(format!(
                    "Arc length {}   Center {}, {}",
                    fmt_ft_in(self.draft.path_length()),
                    fmt_ft_in(center.x),
                    fmt_ft_in(center.y)
                ));
            }
            ui.weak("The ends stay put; Wall Length is the chord.");
        });
        ui.add_enabled_ui(!is_default && self.draft.is_curved(), |ui| {
            row(ui, "Radius to", |ui| {
                for (about, label) in RADIUS_TO {
                    ui.radio_value(&mut self.radius_to, about, label);
                }
            });
            row(ui, "Lock", |ui| {
                ui.radio_value(&mut self.arc_lock, ArcLock::Center, "Arc Center")
                    .on_hover_text("A new radius or arc angle keeps the arc center; the ends move");
                ui.radio_value(&mut self.arc_lock, ArcLock::Ends, "Ends")
                    .on_hover_text("A new radius or arc angle keeps the wall's ends");
            });
        });
        ui.add_enabled_ui(false, |ui| {
            dis_check(ui, "Automatic Facet Angle", true);
        });
    }

    /// Straight to arc (a quarter of the chord for the rise) and back. The
    /// openings keep their proportion along the longer or shorter wall.
    fn set_curved(&mut self, on: bool) {
        let old = self.draft.path_length();
        self.draft.curve = on.then(|| WallCurve {
            bulge: self.draft.length() * 0.25,
        });
        self.track_path(old);
    }

    fn arc_left(&self) -> bool {
        self.draft.curve.is_none_or(|c| c.bulge >= 0.0)
    }

    /// The radius to the chosen reference line; below half the chord it
    /// stays at half (a semicircle). With the Arc Center lock the ends move.
    fn set_radius(&mut self, radius: f64) {
        let old = self.draft.path_length();
        let ty = self.draft_type().cloned();
        let (about, lock) = (self.radius_to, self.arc_lock);
        if self
            .draft
            .set_radius_to(ty.as_ref(), about, radius, lock)
            .is_some()
        {
            self.track_path(old);
        }
    }

    /// The arc angle in degrees, 1 to 340.
    fn set_sweep_deg(&mut self, deg: f64) {
        let old = self.draft.path_length();
        let lock = self.arc_lock;
        if self
            .draft
            .set_sweep_locked(deg.to_radians(), lock)
            .is_some()
        {
            self.track_path(old);
        }
    }

    /// The rise (sagitta) over the chord: 0 makes the wall straight.
    fn set_rise(&mut self, rise: f64) {
        let old = self.draft.path_length();
        let rise = rise.max(0.0);
        self.draft.curve = (rise > 1e-9).then(|| WallCurve {
            bulge: if self.arc_left() { rise } else { -rise },
        });
        self.track_path(old);
    }

    fn set_side(&mut self, left: bool) {
        if let Some(c) = &mut self.draft.curve {
            c.bulge = if left { c.bulge.abs() } else { -c.bulge.abs() };
        }
    }

    // ----- fields shared by the one-wall and the multi-wall dialog -----

    /// Does `key` show the mixed state: several walls disagree and the user
    /// has not edited it yet (W-83)?
    fn mixed(&self, key: &str) -> bool {
        self.multi
            .as_ref()
            .is_some_and(|m| m.mixed.contains(key) && !m.touched.contains(key))
    }

    /// Notes that the user edited `key`; only edited fields are written to
    /// the walls of a multi-wall dialog.
    fn touch(&mut self, key: &'static str) {
        if let Some(m) = &mut self.multi {
            m.touched.insert(key);
        }
    }

    /// A check box over a `bool` of the draft (the mixed state shows as
    /// indeterminate until it is clicked).
    fn chk(
        &mut self,
        ui: &mut Ui,
        key: &'static str,
        label: &str,
        get: fn(&mut Wall) -> &mut bool,
    ) -> bool {
        let mut v = *get(&mut self.draft);
        let resp = ui.add(egui::Checkbox::new(&mut v, label).indeterminate(self.mixed(key)));
        if resp.changed() {
            *get(&mut self.draft) = v;
            self.touch(key);
            return true;
        }
        false
    }

    /// A radio button; `selected` is whether the draft has this choice. A mixed
    /// field selects none. Returns true when it was clicked.
    fn pick(&mut self, ui: &mut Ui, key: &'static str, label: &str, selected: bool) -> bool {
        let selected = selected && !self.mixed(key);
        let clicked = ui.radio(selected, label).clicked();
        if clicked {
            self.touch(key);
        }
        clicked
    }

    /// A length field over an `f64` of the draft. A mixed field is blank
    /// until something is typed in it.
    fn len(
        &mut self,
        ui: &mut Ui,
        label: &str,
        key: &'static str,
        get: fn(&mut Wall) -> &mut f64,
    ) -> bool {
        if !self.mixed(key) {
            let changed = self.fields.length_row(ui, label, key, get(&mut self.draft));
            if changed {
                self.touch(key);
            }
            return changed;
        }
        let mut changed = false;
        row(ui, label, |ui| {
            let buf = self
                .multi
                .as_mut()
                .map(|m| m.bufs.entry(key).or_default())
                .expect("a mixed field exists only in a multi-wall dialog");
            let resp = ui.add(
                egui::TextEdit::singleline(buf)
                    .desired_width(100.0)
                    .hint_text("(varies)"),
            );
            if resp.changed() {
                if let Some(v) = plan_core::units::parse_ft_in(buf) {
                    *get(&mut self.draft) = v;
                    changed = true;
                }
            }
        });
        if changed {
            self.touch(key);
        }
        changed
    }

    /// The thinnest the wall may be made: the layers of its type other than
    /// the main layer (W-30).
    fn min_thickness(&self, wall: &Wall) -> f64 {
        let ty = wall
            .wall_type
            .as_deref()
            .and_then(|n| self.types.iter().find(|t| t.name == n));
        min_thickness(ty)
    }

    fn structure(&mut self, ui: &mut Ui) {
        let is_default = self.target.is_default();
        section(ui, "Default Wall Heights");
        let mixed = self.mixed("default_top");
        let before = self.default_top;
        let resp = ui.add(
            egui::Checkbox::new(&mut self.default_top, "Default Wall Top Height")
                .indeterminate(mixed),
        );
        if resp.changed() && self.default_top != before {
            self.touch("default_top");
            self.draft.spec.structure.custom_top = !self.default_top;
            if self.default_top {
                self.draft.height = self.default_height;
            }
        }
        ui.add_enabled_ui(!self.default_top || mixed, |ui| {
            if self.len(ui, "Wall Height", "height", |w| &mut w.height) {
                self.default_top = false;
                self.draft.spec.structure.custom_top = true;
                self.touch("default_top");
            }
        });
        ui.add_enabled_ui(!is_default, |ui| {
            let mixed = self.mixed("default_bottom");
            let mut on = self
                .draft
                .spec
                .structure
                .default_bottom(self.draft.bottom_offset);
            let resp = ui.add(
                egui::Checkbox::new(&mut on, "Default Wall Bottom Height").indeterminate(mixed),
            );
            if resp.changed() {
                self.touch("default_bottom");
                self.draft.spec.structure.custom_bottom = !on;
                if on {
                    self.draft.bottom_offset = 0.0;
                }
            }
        });

        ui.add_enabled_ui(!is_default, |ui| {
            section(ui, "Platform Intersections");
            self.chk(
                ui,
                "gen_between",
                "Invisible Walls and Railings: Generate Between Platforms",
                |w| &mut w.spec.structure.generate_between_platforms,
            );
            let ceiling = self.draft.spec.structure.ceiling_platform;
            row(ui, "Ceiling Platform", |ui| {
                ui.vertical(|ui| {
                    let c = [
                        ("Automatic", CeilingPlatform::Automatic),
                        ("Stop at Ceiling Above", CeilingPlatform::StopAtCeilingAbove),
                        (
                            "Balloon Through Ceiling Above",
                            CeilingPlatform::BalloonThroughCeilingAbove,
                        ),
                    ];
                    for (label, choice) in c {
                        if self.pick(ui, "ceiling_platform", label, ceiling == choice) {
                            self.draft.spec.structure.ceiling_platform = choice;
                        }
                    }
                    let hang = matches!(ceiling, CeilingPlatform::HangFloorPlatformAbove { .. });
                    let (mut sub, mut ledger) = match ceiling {
                        CeilingPlatform::HangFloorPlatformAbove {
                            subfloor_to_interior,
                            include_ledger,
                        } => (subfloor_to_interior, include_ledger),
                        _ => (false, false),
                    };
                    if self.pick(
                        ui,
                        "ceiling_platform",
                        "Hang Floor Platform Above on Wall",
                        hang,
                    ) {
                        self.draft.spec.structure.ceiling_platform =
                            CeilingPlatform::HangFloorPlatformAbove {
                                subfloor_to_interior: sub,
                                include_ledger: ledger,
                            };
                    }
                    ui.add_enabled_ui(hang, |ui| {
                        let a = ui
                            .checkbox(&mut sub, "Subflooring to Wall Interior")
                            .changed();
                        let b = ui.checkbox(&mut ledger, "Include Ledger").changed();
                        if a || b {
                            self.draft.spec.structure.ceiling_platform =
                                CeilingPlatform::HangFloorPlatformAbove {
                                    subfloor_to_interior: sub,
                                    include_ledger: ledger,
                                };
                            self.touch("ceiling_platform");
                        }
                    });
                });
            });
            let floor = self.draft.spec.structure.floor_platform;
            row(ui, "Floor Platform", |ui| {
                ui.vertical(|ui| {
                    let c = [
                        ("Automatic", FloorPlatform::Automatic),
                        ("Stop at Floor Below", FloorPlatform::StopAtFloorBelow),
                        (
                            "Balloon/Extend Through Floor Below",
                            FloorPlatform::BalloonThroughFloorBelow,
                        ),
                    ];
                    for (label, choice) in c {
                        if self.pick(ui, "floor_platform", label, floor == choice) {
                            self.draft.spec.structure.floor_platform = choice;
                        }
                    }
                });
            });
            section(ui, "Wall Intersections");
            self.chk(ui, "through_end", "Through Wall At End", |w| {
                &mut w.spec.structure.through_at_end
            });
            self.chk(ui, "through_start", "Through Wall At Start", |w| {
                &mut w.spec.structure.through_at_start
            });
        });
        ui.add_enabled_ui(false, |ui| {
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
        });
        // Framing: Retain Wall Framing is read by Build Framing; Bearing Wall
        // is stored with the wall.
        ui.add_enabled_ui(!is_default, |ui| {
            section(ui, "Framing");
            let r = self.retain;
            let mut v = r.value;
            let resp = ui.add(
                egui::Checkbox::new(&mut v, "Retain Wall Framing")
                    .indeterminate(r.mixed && !r.touched),
            );
            if resp.changed() {
                // The first click on the mixed state checks the box.
                self.retain = Retain {
                    value: v || (r.mixed && !r.touched),
                    mixed: r.mixed,
                    touched: true,
                };
            }
            self.chk(ui, "bearing_wall", "Bearing Wall", |w| {
                &mut w.spec.structure.bearing_wall
            });
        });
        ui.add_enabled_ui(false, |ui| {
            dis_check(ui, "Stagger Multiple Framing Layers", true);
            dis_check(ui, "Create Wall/Footing Below", false);
            dis_check(ui, "Insert Floor Framing Below", true);
        });
    }

    /// Foundation tab (W-52): the footing under the wall, slab chamfers and
    /// the sill plate on top.
    fn foundation(&mut self, ui: &mut Ui) {
        let is_default = self.target.is_default();
        ui.add_enabled_ui(!is_default, |ui| {
            section(ui, "Foundation");
            ui.horizontal_wrapped(|ui| {
                let mut foundation = self.draft.class == WallClass::Foundation;
                let single = self.multi.is_none();
                let changed = ui
                    .add_enabled(
                        single,
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
                self.chk(ui, "slab_footing", "Slab Footing", |w| {
                    &mut w.spec.foundation.slab_footing
                });
            });
            self.len(ui, "Wall Thickness", "thickness", |w| &mut w.thickness);

            section(ui, "Footing");
            self.chk(ui, "footing", "Footing", |w| &mut w.spec.foundation.footing);
            let on = self.draft.spec.foundation.footing || self.mixed("footing");
            ui.add_enabled_ui(on, |ui| {
                self.len(ui, "Width", "footing_width", |w| {
                    &mut w.spec.foundation.footing_width
                });
                self.len(ui, "Height", "footing_height", |w| {
                    &mut w.spec.foundation.footing_height
                });
                self.chk(ui, "footing_auto", "Automatic Footing Bottom Height", |w| {
                    &mut w.spec.foundation.auto_bottom
                });
                ui.add_enabled_ui(!self.draft.spec.foundation.auto_bottom, |ui| {
                    self.len(ui, "Footing Bottom", "footing_bottom", |w| {
                        &mut w.spec.foundation.footing_bottom
                    });
                });
                self.chk(ui, "footing_vertical", "Vertical Footing", |w| {
                    &mut w.spec.foundation.vertical_footing
                });
                self.len(ui, "Footing Offset", "footing_offset", |w| {
                    &mut w.spec.foundation.footing_offset
                });
                self.chk(ui, "footing_center", "Center Footing on Main Layer", |w| {
                    &mut w.spec.foundation.center_on_main_layer
                });
                self.chk(ui, "footing_outside", "Align Footing on Outside", |w| {
                    &mut w.spec.foundation.align_on_outside
                });
            });

            section(ui, "Slab");
            self.chk(ui, "chamfer_mono", "Add Chamfer on Monolithic Slab", |w| {
                &mut w.spec.foundation.chamfer_monolithic
            });
            self.chk(ui, "chamfer_regular", "Add Chamfer on Regular Slab", |w| {
                &mut w.spec.foundation.chamfer_regular
            });
            self.len(ui, "Chamfer Width", "chamfer_width", |w| {
                &mut w.spec.foundation.chamfer_width
            });
            self.len(ui, "Chamfer Height", "chamfer_height", |w| {
                &mut w.spec.foundation.chamfer_height
            });
            row(ui, "Monolithic Slab Pour Number", |ui| {
                let mut n = self.draft.spec.foundation.pour_number;
                if ui.add(egui::DragValue::new(&mut n).range(1..=99)).changed() {
                    self.draft.spec.foundation.pour_number = n;
                    self.touch("pour");
                }
            });

            section(ui, "Sill Plate");
            self.chk(ui, "sill_plate", "Sill Plate", |w| {
                &mut w.spec.foundation.sill_plate
            });
            row(ui, "Construction", |ui| {
                dis_combo(
                    ui,
                    "wall_sill",
                    &self.draft.spec.foundation.sill_construction.clone(),
                )
            });
        });
        if is_default {
            ui.weak("The foundation options are set on a placed wall.");
        }
    }

    /// Wall Cap tab: the cap profile laid on the top of the wall (usually a
    /// half wall).
    fn cap(&mut self, ui: &mut Ui) {
        let is_default = self.target.is_default();
        ui.add_enabled_ui(!is_default, |ui| {
            section(ui, "Wall Cap Profile");
            self.chk(ui, "cap", "Wall Cap", |w| &mut w.spec.cap.enabled);
            let on = self.draft.spec.cap.enabled || self.mixed("cap");
            ui.add_enabled_ui(on, |ui| {
                egui::Grid::new("wall_cap_table")
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong("Name");
                        ui.strong("Width");
                        ui.strong("Height");
                        ui.end_row();
                        for p in default_cap_profiles() {
                            let selected =
                                self.draft.spec.cap.profile == p.name && !self.mixed("cap_profile");
                            if ui.selectable_label(selected, &p.name).clicked() {
                                self.draft.spec.cap.profile = p.name.clone();
                                self.touch("cap_profile");
                            }
                            ui.label(super::fmt_short(p.width));
                            ui.label(super::fmt_short(p.height));
                            ui.end_row();
                        }
                    });
                self.chk(ui, "cap_full", "Full Wall Width", |w| {
                    &mut w.spec.cap.full_wall_width
                });
                self.chk(ui, "cap_split_pony", "Split Pony Wall", |w| {
                    &mut w.spec.cap.split_pony_wall
                });
                section(ui, "Selected Profile Options");
                let pos = self.draft.spec.cap.position;
                row(ui, "Horizontal Position", |ui| {
                    for (label, choice) in [
                        ("Inside Wall", CapPosition::Inside),
                        ("Wall Center", CapPosition::Center),
                        ("Outside Wall", CapPosition::Outside),
                    ] {
                        if self.pick(ui, "cap_position", label, pos == choice) {
                            self.draft.spec.cap.position = choice;
                        }
                    }
                });
            });
        });
        if is_default {
            ui.weak("The wall cap is set on a placed wall.");
        }
    }

    /// General tab over several walls (W-83): the fields every wall has.
    fn general_multi(&mut self, ui: &mut Ui) {
        section(ui, "General");
        self.len(ui, "Thickness", "thickness", |w| &mut w.thickness);
        if self.len(ui, "Bottom Height", "bottom", |w| &mut w.bottom_offset) {
            self.draft.spec.structure.custom_bottom = self.draft.bottom_offset.abs() > 1e-9;
            self.touch("default_bottom");
        }
        section(ui, "Options");
        self.chk(ui, "invisible", "Invisible", |w| &mut w.flags.invisible);
        self.chk(ui, "no_room_definition", "No Room Definition", |w| {
            &mut w.flags.no_room_definition
        });
        self.chk(ui, "no_locate", "No Locate", |w| &mut w.flags.no_locate);
        let n = self.multi.as_ref().map_or(0, |m| m.ids.len());
        ui.weak(format!(
            "{n} walls are open. A blank or indeterminate field differs between them and \
             stays as it is unless you edit it."
        ));
    }

    /// Wall Types tab over several walls: one wall type for all of them.
    fn wall_types_multi(&mut self, ui: &mut Ui) {
        section(ui, "General");
        let mixed = self.mixed("wall_type");
        let text = if mixed {
            "(varies)".to_string()
        } else {
            self.draft
                .wall_type
                .clone()
                .unwrap_or_else(|| "Custom".into())
        };
        let mut picked: Option<WallTypeDef> = None;
        row(ui, "Wall Type", |ui| {
            egui::ComboBox::from_id_salt("wall_type_multi")
                .selected_text(text)
                .show_ui(ui, |ui| {
                    for t in &self.types {
                        let label = format!("{} ({})", t.name, super::fmt_short(t.thickness()));
                        let on = !mixed && self.draft.wall_type.as_deref() == Some(t.name.as_str());
                        if ui.selectable_label(on, label).clicked() {
                            picked = Some(t.clone());
                        }
                    }
                });
        });
        if let Some(t) = picked {
            self.draft.wall_type = Some(t.name.clone());
            self.draft.thickness = t.thickness();
            self.extras.wall_type = Some(t.name);
            self.touch("wall_type");
            self.touch("thickness");
        }
    }

    /// Layer tab over several walls.
    fn layer_multi(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        let mixed = self.mixed("layer");
        let text = if mixed {
            "(varies)".to_string()
        } else {
            self.draft.layer.clone()
        };
        let mut picked = None;
        row(ui, "Layer", |ui| {
            egui::ComboBox::from_id_salt("wall_layer_multi")
                .selected_text(text)
                .show_ui(ui, |ui| {
                    for name in WALL_LAYERS {
                        let on = !mixed && self.draft.layer == name;
                        if ui.selectable_label(on, name).clicked() {
                            picked = Some(name);
                        }
                    }
                });
        });
        if let Some(name) = picked {
            self.draft.layer = name.to_string();
            self.touch("layer");
        }
    }

    /// The Roof tab; over several walls a change marks the directive as
    /// edited so it is written to all of them, and a directive that differs
    /// between the walls shows as "(varies)" until then.
    fn roof_tracked(&mut self, ui: &mut Ui) {
        if self.mixed("roof") {
            ui.weak("(varies) The walls have different roof settings; editing sets them all.");
        }
        let before = self.draft.roof.clone();
        self.roof(ui);
        if self.draft.roof != before {
            self.touch("roof");
        }
    }

    /// Roof tab (RF-18..RF-27): what Build Roof does at this wall.
    fn roof(&mut self, ui: &mut Ui) {
        use plan_core::defaults::RoofWallKind as K;
        section(ui, "Roof Options");
        let r = &mut self.draft.roof;
        let kind_before = r.kind;
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
        // A Full Gable Wall checks Include Automatic End Truss Above for you.
        if r.kind == K::FullGable && kind_before != K::FullGable {
            r.end_truss_above = true;
        }
        let mut cuts = r.cuts_wall_at_bottom.unwrap_or(true);
        if ui
            .checkbox(&mut cuts, "Roof Cuts Wall at Bottom")
            .on_hover_text("The part of the wall below an intersecting roof plane is not built")
            .changed()
        {
            r.cuts_wall_at_bottom = Some(cuts);
        }
        ui.checkbox(&mut r.include_frieze, "Include Frieze")
            .on_hover_text(
                "The frieze molding of Build Roof runs along this wall at the roof line",
            );
        ui.checkbox(&mut r.end_truss_above, "Include Automatic End Truss Above")
            .on_hover_text(
                "An attic wall above gets a Reduced Gable End Truss with automatic trusses",
            );
        ui.add_enabled(
            self.attic_above,
            egui::Checkbox::new(&mut r.combine_with_above, "Combine with Above Wall"),
        )
        .on_hover_text("Balloon-frame this wall with the attic wall above it");
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
            // In From Baseline is the same break measured in plan from the
            // baseline; the two follow each other (manual p. 429).
            let lower = r.pitch_in_12.unwrap_or(ROOF_PITCH_DEFAULT);
            let wall_h = self.draft.height;
            let mut inward = plan_roof::in_from_baseline_for_start_height(wall_h, lower, *start);
            if self
                .fields
                .length_row(ui, "In From Baseline", "roof_in_from_baseline", &mut inward)
            {
                *start = plan_roof::start_height_for_in_from_baseline(wall_h, lower, inward);
            }
            ui.weak("Starts at Height is above the floor of this wall; In From Baseline is the plan distance from the baseline. Each updates the other.");
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
            super::select_layer::layer_field(
                ui,
                "wall_layer",
                &mut self.draft.layer,
                WALL_LAYERS.iter().copied(),
            );
        });
        self.drawing_group_row(ui);
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
        if self.multi.is_some() {
            WALL_TABS_MULTI
        } else if !self.target.is_default() {
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
        if let Some(m) = &self.multi {
            // Every wall that takes the new thickness must keep its layers.
            if m.touched.contains("thickness") {
                for w in &m.originals {
                    let least = self.min_thickness(w);
                    if self.draft.thickness < least - 1e-9 {
                        return Some(format!(
                            "Thickness cannot be less than {} (the layers of {})",
                            fmt_ft_in(least),
                            w.wall_type.as_deref().unwrap_or("the wall")
                        ));
                    }
                }
            }
        } else if (self.draft.thickness - self.orig_thickness).abs() > 1e-9 {
            let least = self.min_thickness(&self.draft);
            if self.draft.thickness < least - 1e-9 {
                return Some(format!(
                    "Thickness cannot be less than {} (the layers of the wall type)",
                    fmt_ft_in(least)
                ));
            }
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
        let len = self.draft.path_length();
        let too_short = self.adjusted_openings().iter().any(|o| {
            o.start_offset() < OPENING_MARGIN - 1e-6 || o.end_offset() > len - OPENING_MARGIN + 1e-6
        });
        too_short.then(|| "Wall is too short for its openings".to_string())
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        if self.multi.is_some() {
            match self.tabs()[tab].name {
                "General" => self.general_multi(ui),
                "Structure" => self.structure(ui),
                "Roof" => self.roof_tracked(ui),
                "Foundation" => self.foundation(ui),
                "Wall Types" => self.wall_types_multi(ui),
                "Wall Cap" => self.cap(ui),
                "Layer" => self.layer_multi(ui),
                _ => {}
            }
            return;
        }
        match self.tabs()[tab].name {
            "General" => self.general(ui),
            "Structure" => self.structure(ui),
            "Foundation" => self.foundation(ui),
            "Wall Cap" => self.cap(ui),
            "Roof" => self.roof_tracked(ui),
            "Wall Types" => self.wall_types(ui),
            "Rail Style" => self.rail_style(ui),
            "Layer" => self.layer(ui),
            "Label" => self.label(ui),
            "Wall Covering" => self.wall_covering(ui),
            "Newels/Balusters" => self.newels_balusters(ui),
            "Rails" => self.rails(ui),
            "Materials" => self.materials_tab(ui),
            "Components" => self.components_tab(ui),
            "Object Information" => self.object_information(ui),
            "Schedule" => self.schedule_tab(ui),
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
            open_shift: 0.0,
            draft: wall,
            extras: WallExtras::default(),
            openings: vec![opening],
            lock,
            radius_to: ResizeAbout::WallCenter,
            arc_lock: ArcLock::Ends,
            arc_scale: 1.0,
            default_height: 109.125,
            default_top: true,
            orig_thickness: 4.5,
            multi: None,
            retain: Retain::default(),
            fields: Fields::default(),
            material_lib: None,
            material_filter: String::new(),
            types: plan_core::PlanDefaults::chief_x18_daniel().wall_types,
            define: None,
            edited_types: Vec::new(),
            attic_above: false,
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
        // An interior wall can be a gable or knee wall: its Roof tab is live.
        f.draft.kind = WallKind::Interior;
        assert!(f.tabs().iter().any(|t| t.name == "Roof" && t.enabled));
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

    #[test]
    fn the_arc_section_round_trips_radius_angle_and_rise() {
        let mut f = form(WallLock::Start);
        let chord = f.draft.length();
        assert!(!f.draft.is_curved());
        f.set_curved(true);
        let c = f.draft.curve.unwrap();
        assert!((c.bulge - chord * 0.25).abs() < 1e-9);
        // The radius reads back, and typing it again changes nothing.
        let r = c.radius(chord).unwrap();
        f.set_radius(r);
        assert!((f.draft.curve.unwrap().bulge - chord * 0.25).abs() < 1e-9);
        f.set_radius(chord);
        let c = f.draft.curve.unwrap();
        assert!((c.radius(chord).unwrap() - chord).abs() < 1e-9);
        // A radius below half the chord falls back to the semicircle.
        f.set_radius(1.0);
        let c = f.draft.curve.unwrap();
        assert!((c.bulge - chord * 0.5).abs() < 1e-9);
        f.set_sweep_deg(90.0);
        let c = f.draft.curve.unwrap();
        assert!((c.sweep_abs(chord).to_degrees() - 90.0).abs() < 1e-9);
        // The side flips the sign and keeps the size; the rise sets it.
        f.set_side(false);
        assert!(f.draft.curve.unwrap().bulge < 0.0);
        f.set_rise(20.0);
        assert!((f.draft.curve.unwrap().bulge + 20.0).abs() < 1e-9);
        f.set_side(true);
        assert!((f.draft.curve.unwrap().bulge - 20.0).abs() < 1e-9);
        // Through the JSON the arc stays an arc.
        let back: Wall = serde_json::from_str(&serde_json::to_string(&f.draft).unwrap()).unwrap();
        assert_eq!(back.curve, f.draft.curve);
        assert!(back.is_curved());
        f.set_rise(0.0);
        assert!(f.draft.curve.is_none());
        f.set_curved(true);
        f.set_curved(false);
        assert!(f.draft.curve.is_none());
    }

    #[test]
    fn making_a_wall_curved_rescales_its_openings_and_straightening_restores_them() {
        let mut f = form(WallLock::Start);
        assert_eq!(f.adjusted_openings()[0].center_offset, 50.0);
        f.set_curved(true);
        let len = f.draft.path_length();
        assert!(len > 100.0);
        let o = &f.adjusted_openings()[0];
        assert!((o.center_offset - 50.0 * len / 100.0).abs() < 1e-9);
        // A deeper bend stretches them further; the proportion stays 1/2.
        f.set_rise(40.0);
        let o = &f.adjusted_openings()[0];
        assert!((o.center_offset / f.draft.path_length() - 0.5).abs() < 1e-9);
        assert!(f.error().is_none());
        // Back to a straight wall: the original offsets return.
        f.set_curved(false);
        assert!((f.adjusted_openings()[0].center_offset - 50.0).abs() < 1e-9);
    }

    #[test]
    fn the_radius_is_measured_to_the_chosen_reference_line() {
        let mut f = form(WallLock::Start);
        f.set_curved(true);
        let (_, r) = f.draft.arc_center_radius().unwrap();
        f.radius_to = ResizeAbout::OuterSurface;
        let outer = f.draft.radius_to(f.draft_type(), f.radius_to).unwrap();
        // The default wall's exterior is its left side, the bulge side.
        assert!((outer - (r + 2.25)).abs() < 1e-9);
        f.set_radius(outer + 50.0);
        let (_, r2) = f.draft.arc_center_radius().unwrap();
        assert!((r2 - (r + 50.0)).abs() < 1e-9);
        assert_eq!(f.draft.start, Point::new(10.0, 10.0), "ends locked");
    }

    #[test]
    fn the_arc_center_lock_moves_the_ends_and_the_openings_follow() {
        let mut f = form(WallLock::Start);
        f.set_curved(true);
        let (c0, r0) = f.draft.arc_center_radius().unwrap();
        f.arc_lock = ArcLock::Center;
        f.set_radius(r0 * 2.0);
        let (c1, r1) = f.draft.arc_center_radius().unwrap();
        assert!(c1.dist(c0) < 1e-9 && (r1 - 2.0 * r0).abs() < 1e-9);
        assert_ne!(f.draft.start, Point::new(10.0, 10.0));
        let o = &f.adjusted_openings()[0];
        assert!((o.center_offset / f.draft.path_length() - 0.5).abs() < 1e-9);
        // The arc angle swings the end about the center.
        let sweep = f.draft.curve.unwrap().sweep_abs(f.draft.length());
        f.set_sweep_deg(sweep.to_degrees() / 2.0);
        let (c2, r2) = f.draft.arc_center_radius().unwrap();
        assert!(c2.dist(c0) < 1e-6 && (r2 - 2.0 * r0).abs() < 1e-6);
    }

    // ----- Structure and Foundation tabs, multi-wall Open Object (W-83) -----

    /// Draws `tab` of `f` and clicks the text `label` (the last such text on
    /// the page, so a checkbox wins over a section title of the same name).
    fn click_label(f: &mut WallForm, ctx: &egui::Context, tab: usize, label: &str) {
        use egui::{Event, PointerButton, Pos2, RawInput};
        fn find(s: &egui::Shape, label: &str, out: &mut Option<Pos2>) {
            match s {
                egui::Shape::Text(t) if t.galley.text() == label => {
                    *out = Some(t.pos + egui::vec2(4.0, 4.0))
                }
                egui::Shape::Vec(v) => v.iter().for_each(|x| find(x, label, out)),
                _ => {}
            }
        }
        let frame = |f: &mut WallForm, input: RawInput| {
            ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| f.page(ui, tab));
            })
        };
        let out = frame(f, RawInput::default());
        let mut at = None;
        for c in &out.shapes {
            find(&c.shape, label, &mut at);
        }
        let at = at.unwrap_or_else(|| panic!("no text {label:?} on the page"));
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        for input in [
            vec![Event::PointerMoved(at)],
            vec![button(true)],
            vec![button(false)],
        ] {
            frame(
                f,
                RawInput {
                    events: input,
                    ..Default::default()
                },
            );
        }
    }

    fn tab_index(f: &WallForm, name: &str) -> usize {
        f.tabs().iter().position(|t| t.name == name).unwrap()
    }

    #[test]
    fn the_structure_tab_stores_platform_intersections_and_through_walls() {
        let mut f = form(WallLock::Start);
        let ctx = egui::Context::default();
        let tab = tab_index(&f, "Structure");
        let st = |f: &WallForm| f.draft.spec.structure.clone();
        assert_eq!(st(&f).ceiling_platform, CeilingPlatform::Automatic);
        click_label(&mut f, &ctx, tab, "Balloon Through Ceiling Above");
        assert_eq!(
            st(&f).ceiling_platform,
            CeilingPlatform::BalloonThroughCeilingAbove
        );
        click_label(&mut f, &ctx, tab, "Stop at Ceiling Above");
        assert_eq!(st(&f).ceiling_platform, CeilingPlatform::StopAtCeilingAbove);
        click_label(&mut f, &ctx, tab, "Hang Floor Platform Above on Wall");
        assert!(matches!(
            st(&f).ceiling_platform,
            CeilingPlatform::HangFloorPlatformAbove { .. }
        ));
        click_label(&mut f, &ctx, tab, "Include Ledger");
        assert!(matches!(
            st(&f).ceiling_platform,
            CeilingPlatform::HangFloorPlatformAbove {
                include_ledger: true,
                ..
            }
        ));
        click_label(&mut f, &ctx, tab, "Stop at Floor Below");
        assert_eq!(st(&f).floor_platform, FloorPlatform::StopAtFloorBelow);
        click_label(&mut f, &ctx, tab, "Balloon/Extend Through Floor Below");
        assert_eq!(
            st(&f).floor_platform,
            FloorPlatform::BalloonThroughFloorBelow
        );
        assert!(!st(&f).through_at_end);
        click_label(&mut f, &ctx, tab, "Through Wall At End");
        click_label(&mut f, &ctx, tab, "Through Wall At Start");
        assert!(st(&f).through_at_end && st(&f).through_at_start);
        assert!(st(&f).generate_between_platforms);
        click_label(
            &mut f,
            &ctx,
            tab,
            "Invisible Walls and Railings: Generate Between Platforms",
        );
        assert!(!st(&f).generate_between_platforms);
        // Unchecking the default heights marks the wall as keeping its own.
        assert!(f.default_top && !st(&f).custom_top);
        click_label(&mut f, &ctx, tab, "Default Wall Top Height");
        assert!(!f.default_top && st(&f).custom_top);
        assert!(st(&f).default_bottom(0.0));
        click_label(&mut f, &ctx, tab, "Default Wall Bottom Height");
        assert!(st(&f).custom_bottom && !st(&f).default_bottom(0.0));
        // And it all survives the file.
        let back: Wall = serde_json::from_str(&serde_json::to_string(&f.draft).unwrap()).unwrap();
        assert_eq!(back.spec.structure, st(&f));
    }

    #[test]
    fn the_foundation_and_cap_tabs_store_their_fields() {
        let mut f = form(WallLock::Start);
        let ctx = egui::Context::default();
        let tab = tab_index(&f, "Foundation");
        assert!(!f.draft.spec.foundation.footing);
        click_label(&mut f, &ctx, tab, "Footing");
        assert!(f.draft.spec.foundation.footing);
        click_label(&mut f, &ctx, tab, "Vertical Footing");
        click_label(&mut f, &ctx, tab, "Align Footing on Outside");
        click_label(&mut f, &ctx, tab, "Automatic Footing Bottom Height");
        let fd = &f.draft.spec.foundation;
        assert!(fd.vertical_footing && fd.align_on_outside && !fd.auto_bottom);
        click_label(&mut f, &ctx, tab, "Slab Footing");
        assert!(f.draft.spec.foundation.slab_footing);
        let cap = tab_index(&f, "Wall Cap");
        click_label(&mut f, &ctx, cap, "Wall Cap");
        assert!(f.draft.spec.cap.enabled);
        click_label(&mut f, &ctx, cap, "Overhanging Cap");
        click_label(&mut f, &ctx, cap, "Full Wall Width");
        click_label(&mut f, &ctx, cap, "Wall Center");
        let c = &f.draft.spec.cap;
        assert_eq!(c.profile, "Overhanging Cap");
        assert!(!c.full_wall_width);
        assert_eq!(c.position, CapPosition::Center);
        // The tabs are live, not dimmed.
        assert!(f.tabs().iter().any(|t| t.name == "Foundation" && t.enabled));
        assert!(f.tabs().iter().any(|t| t.name == "Wall Cap" && t.enabled));
    }

    #[test]
    fn several_walls_edit_the_roof_tab_and_bearing_wall_and_retain_framing() {
        use plan_core::defaults::RoofWallKind;
        let (mut p, ids) = three_walls();
        p.floors[0].wall_mut(ids[0]).unwrap().roof.kind = RoofWallKind::KneeWall;
        let mut d = multi_for(&p, &ids);
        d.set_framing_retained(&[true, false, false]);
        assert!(d.form.tabs().iter().any(|t| t.name == "Roof" && t.enabled));
        assert!(d.is_mixed("roof"));
        assert!(d.retain_framing_change().is_none() && d.form.retain.mixed);
        // Click Full Gable Wall on the Roof tab, Bearing Wall and Retain Wall
        // Framing on the Structure tab.
        let ctx = egui::Context::default();
        let roof = tab_index(&d.form, "Roof");
        click_label(&mut d.form, &ctx, roof, "Full Gable Wall");
        assert_eq!(d.touched(), vec!["roof"]);
        let structure = tab_index(&d.form, "Structure");
        click_label(&mut d.form, &ctx, structure, "Bearing Wall");
        click_label(&mut d.form, &ctx, structure, "Retain Wall Framing");
        assert_eq!(d.retain_framing_change(), Some(true));
        assert_eq!(d.apply_multi(&mut p, 0), 3);
        for id in &ids {
            let w = p.floors[0].wall(*id).unwrap();
            assert_eq!(w.roof.kind, RoofWallKind::FullGable);
            assert!(w.spec.structure.bearing_wall);
        }
    }

    #[test]
    fn the_structure_page_draws_the_live_framing_checkboxes() {
        let mut f = form(WallLock::Start);
        let structure = f.tabs().iter().position(|t| t.name == "Structure").unwrap();
        let drawn = page_texts(&mut f, structure);
        for want in ["Retain Wall Framing", "Bearing Wall"] {
            assert!(drawn.iter().any(|t| t == want), "{want} in {drawn:?}");
        }
        f.retain = Retain {
            value: true,
            mixed: false,
            touched: true,
        };
        f.draft.spec.structure.bearing_wall = true;
        let back: Wall = serde_json::from_str(&serde_json::to_string(&f.draft).unwrap()).unwrap();
        assert!(back.spec.structure.bearing_wall);
    }

    fn three_walls() -> (Project, Vec<Id>) {
        let mut p = Project::new("t");
        let ids: Vec<Id> = [(4.5, 96.0), (6.0, 96.0), (6.0, 96.0)]
            .iter()
            .enumerate()
            .map(|(k, (t, h))| {
                p.add_wall(
                    0,
                    Point::new(0.0, 100.0 * k as f64),
                    Point::new(120.0, 100.0 * k as f64),
                    *t,
                    *h,
                    WallKind::Interior,
                )
            })
            .collect();
        p.floors[0].wall_mut(ids[1]).unwrap().flags.invisible = true;
        (p, ids)
    }

    fn multi_for(p: &Project, ids: &[Id]) -> WallDialog {
        WallDialog::multi(
            ids.iter()
                .map(|i| p.floors[0].wall(*i).unwrap().clone())
                .collect(),
            [109.125, 96.0],
            plan_core::PlanDefaults::chief_x18_daniel().wall_types,
        )
    }

    #[test]
    fn several_walls_show_mixed_fields_and_write_only_the_edited_ones() {
        let (mut p, ids) = three_walls();
        let mut d = multi_for(&p, &ids);
        assert_eq!(d.multi_ids().unwrap(), ids.as_slice());
        // Thickness and the invisible flag differ; the height does not.
        assert!(d.is_mixed("thickness") && d.is_mixed("invisible"));
        assert!(!d.is_mixed("height") && !d.is_mixed("no_locate"));
        assert!(d.touched().is_empty());
        // Nothing edited: nothing written.
        assert_eq!(d.apply_multi(&mut p, 0), 0);
        // Edit No Locate through the page and leave the rest.
        let ctx = egui::Context::default();
        let tab = tab_index(&d.form, "General");
        click_label(&mut d.form, &ctx, tab, "No Locate");
        assert_eq!(d.touched(), vec!["no_locate"]);
        let before: Vec<(f64, bool)> = ids
            .iter()
            .map(|i| {
                let w = p.floors[0].wall(*i).unwrap();
                (w.thickness, w.flags.invisible)
            })
            .collect();
        assert_eq!(d.apply_multi(&mut p, 0), 3);
        for (k, id) in ids.iter().enumerate() {
            let w = p.floors[0].wall(*id).unwrap();
            assert!(w.flags.no_locate, "wall {k}");
            assert_eq!((w.thickness, w.flags.invisible), before[k], "wall {k}");
        }
    }

    #[test]
    fn a_thickness_typed_over_several_walls_goes_to_all_of_them() {
        let (mut p, ids) = three_walls();
        let mut d = multi_for(&p, &ids);
        {
            let m = d.form.multi.as_mut().unwrap();
            m.touched.insert("thickness");
        }
        d.draft_mut().thickness = 8.0;
        assert!(d.form.error().is_none());
        assert_eq!(d.apply_multi(&mut p, 0), 3);
        for id in &ids {
            assert_eq!(p.floors[0].wall(*id).unwrap().thickness, 8.0);
        }
        // One undo step is the host's business; the walls' other fields stay.
        assert!(p.floors[0].wall(ids[1]).unwrap().flags.invisible);
    }

    #[test]
    fn thickness_cannot_go_below_the_layers_of_the_wall_type() {
        let ty = plan_core::PlanDefaults::chief_x18_daniel()
            .wall_type("Stucco-6")
            .unwrap()
            .clone();
        let fixed = plan_core::walls::spec::fixed_layers_thickness(&ty);
        assert!(fixed > 0.5);
        let mut p = Project::new("t");
        let mut ids = Vec::new();
        for k in 0..2 {
            let id = p.add_wall(
                0,
                Point::new(0.0, 100.0 * k as f64),
                Point::new(120.0, 100.0 * k as f64),
                ty.thickness(),
                96.0,
                WallKind::Exterior,
            );
            p.floors[0].wall_mut(id).unwrap().wall_type = Some(ty.name.clone());
            ids.push(id);
        }
        let mut d = multi_for(&p, &ids);
        d.form.multi.as_mut().unwrap().touched.insert("thickness");
        d.draft_mut().thickness = fixed;
        let msg = d.form.error().expect("too thin for its layers");
        assert!(msg.starts_with("Thickness cannot be less than"), "{msg}");
        d.draft_mut().thickness = fixed + 0.25;
        assert!(d.form.error().is_none());

        // The same rule for one wall.
        let mut f = form(WallLock::Start);
        f.types = vec![ty.clone()];
        f.draft.wall_type = Some(ty.name.clone());
        f.draft.thickness = fixed;
        assert!(f.error().unwrap().starts_with("Thickness cannot be less"));
        // A wall that was already thinner may keep its thickness.
        f.orig_thickness = fixed;
        assert!(f.error().is_none());
    }

    #[test]
    fn mixed_platform_choices_select_no_radio_until_one_is_clicked() {
        let (mut p, ids) = three_walls();
        {
            use plan_core::walls::CeilingPlatform as C;
            p.floors[0]
                .wall_mut(ids[0])
                .unwrap()
                .spec
                .structure
                .ceiling_platform = C::BalloonThroughCeilingAbove;
        }
        let mut d = multi_for(&p, &ids);
        assert!(d.is_mixed("ceiling_platform"));
        let ctx = egui::Context::default();
        let tab = tab_index(&d.form, "Structure");
        click_label(&mut d.form, &ctx, tab, "Stop at Ceiling Above");
        assert!(!d.is_mixed("ceiling_platform"));
        click_label(&mut d.form, &ctx, tab, "Through Wall At End");
        assert_eq!(d.touched(), vec!["ceiling_platform", "through_end"]);
        assert_eq!(d.apply_multi(&mut p, 0), 3);
        for id in &ids {
            let st = &p.floors[0].wall(*id).unwrap().spec.structure;
            assert_eq!(st.ceiling_platform, CeilingPlatform::StopAtCeilingAbove);
            assert!(st.through_at_end && !st.through_at_start);
        }
    }

    #[test]
    fn a_wall_type_picked_for_several_walls_resizes_each_about_its_reference() {
        let (mut p, ids) = three_walls();
        let d_types = plan_core::PlanDefaults::chief_x18_daniel().wall_types;
        let i6 = d_types
            .iter()
            .find(|t| t.name == "Interior-6")
            .unwrap()
            .clone();
        let mut d = multi_for(&p, &ids);
        {
            let form = &mut d.form;
            form.draft.wall_type = Some(i6.name.clone());
            form.draft.thickness = i6.thickness();
            let m = form.multi.as_mut().unwrap();
            m.touched.insert("wall_type");
            m.touched.insert("thickness");
        }
        assert_eq!(d.apply_multi(&mut p, 0), 3);
        for id in &ids {
            let w = p.floors[0].wall(*id).unwrap();
            assert_eq!(w.wall_type.as_deref(), Some("Interior-6"));
            assert!((w.thickness - i6.thickness()).abs() < 1e-9);
        }
    }

    #[test]
    fn the_covering_materials_components_info_and_schedule_tabs_are_live() {
        let mut f = form(WallLock::Start);
        let ctx = egui::Context::default();
        for name in [
            "Wall Covering",
            "Materials",
            "Components",
            "Object Information",
            "Schedule",
        ] {
            assert!(
                f.tabs().iter().any(|t| t.name == name && t.enabled),
                "{name}"
            );
        }
        let cover = tab_index(&f, "Wall Covering");
        let drawn = page_texts(&mut f, cover);
        for want in [
            "Interior Side",
            "Exterior Side",
            "Wainscot",
            "Chair Rail",
            "Base Molding",
            "Crown Molding",
        ] {
            assert!(drawn.iter().any(|t| t == want), "{want} in {drawn:?}");
        }
        // The heights follow the molding library once a profile is picked.
        f.draft.spec.covering.interior.base = "Base 5 1/4".into();
        f.draft.spec.covering.interior.wainscot = "Beadboard".into();
        let drawn = page_texts(&mut f, cover);
        assert!(drawn.iter().any(|t| t == "Wainscot Height"), "{drawn:?}");
        // Components list the layers and the wainscot.
        let comp = tab_index(&f, "Components");
        let drawn = page_texts(&mut f, comp);
        assert!(drawn.iter().any(|t| t.starts_with("Wainscot")), "{drawn:?}");
        // Object Information and Schedule store through the draft.
        let info = tab_index(&f, "Object Information");
        let drawn = page_texts(&mut f, info);
        for want in ["Identification", "Manufacturer", "Supplier", "Description"] {
            assert!(drawn.iter().any(|t| t == want), "{want} in {drawn:?}");
        }
        let sched = tab_index(&f, "Schedule");
        assert!(f.draft.spec.schedule.include);
        click_label(&mut f, &ctx, sched, "Include in Schedule");
        assert!(!f.draft.spec.schedule.include);
        let back: Wall = serde_json::from_str(&serde_json::to_string(&f.draft).unwrap()).unwrap();
        assert_eq!(back.spec.covering.interior.base, "Base 5 1/4");
        assert!(!back.spec.schedule.include);
    }

    #[test]
    fn the_newels_and_rails_tabs_belong_to_railing_walls() {
        let mut f = form(WallLock::Start);
        let newels = tab_index(&f, "Newels/Balusters");
        let rails = tab_index(&f, "Rails");
        // An interior wall gets the hint only.
        let drawn = page_texts(&mut f, newels);
        assert!(
            drawn.iter().any(|t| t.starts_with("Draw a Railing")),
            "{drawn:?}"
        );
        f.change_class(WallClass::Railing);
        let drawn = page_texts(&mut f, newels);
        for want in [
            "Newel Size",
            "Greatest Newel Spacing",
            "Between Newels",
            "Baluster Size",
            "Greatest Baluster Spacing",
        ] {
            assert!(drawn.iter().any(|t| t == want), "{want} in {drawn:?}");
        }
        let drawn = page_texts(&mut f, rails);
        for want in [
            "Top Rail",
            "Bottom Rail",
            "Top of Top Rail",
            "Rail Height",
            "Rail Width",
        ] {
            assert!(drawn.iter().any(|t| t == want), "{want} in {drawn:?}");
        }
        // Glass panels replace the balusters.
        f.draft.spec.railing.fill = plan_core::walls::RailFill::GlassPanel;
        let drawn = page_texts(&mut f, newels);
        assert!(drawn.iter().any(|t| t == "Panel Thickness"), "{drawn:?}");
        assert!(
            !drawn.iter().any(|t| t == "Greatest Baluster Spacing"),
            "{drawn:?}"
        );
    }
}
