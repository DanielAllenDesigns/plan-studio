//! Door and Window Specification (docs/chief-x18-dialogs.md).

use super::{
    dis_check, dis_combo, dis_radio, fmt_short, off, on, pv_text, row, section, session_check,
    Fields, Outcome, SpecDialog, SpecPages, Tab, PV_ACCENT, PV_BG, PV_FAINT, PV_GLASS, PV_INK,
    PV_WALL,
};
use eframe::egui::{
    self, Align2, Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Ui, Vec2,
};
use plan_core::defaults::{OpeningDefaults, WindowDefaults};
use plan_core::extras::OpeningExtras as StoredExtras;
use plan_core::opening_symbol::{bifold_panels, plan_symbol, sliding_panels, PartKind};
use plan_core::openings::mull::MulledSpec;
use plan_core::openings::types::{DefaultKey, DynGroup};
use plan_core::openings::{
    door_panel_count, ArchType, CasingProfile, HandleStyle, LintelStyle, LiteStyle, OpenMode,
    OpeningSpec, RecessTo, ShutterSides, ShutterStyle, StandardWidths, WindowType,
};
use plan_core::{
    Casing, Id, LabelMode, LabelPlacement, LabelSettings, Opening, OpeningKind,
    OpeningLabelDefaults, OpeningStyle, OpeningVariantDefaults, Project, SizeFormat, SizeStyle,
    Wall, WallKind,
};

#[path = "bay_window.rs"]
mod bay_window;
#[path = "mulled_unit.rs"]
mod mulled_unit;
mod tabs;

/// Minimum clear distance between an opening jamb and a wall end or another
/// opening. Mirrors the private constant `Project::add_opening` uses.
pub const OPENING_MARGIN: f64 = 2.0;

const DEFAULT_DOOR_KEY: Id = Id::MAX - 2;
const DEFAULT_WINDOW_KEY: Id = Id::MAX - 3;
const DEFAULT_EXTERIOR_DOOR_KEY: Id = Id::MAX - 4;
/// Keys of the Defaults dialogs of the door and window types count down from
/// here.
const TYPE_KEY_BASE: Id = Id::MAX - 1000;

/// The index of `t` in the Window Type list ([`WindowType::ALL`]).
fn type_index(t: WindowType) -> usize {
    WindowType::ALL.iter().position(|x| *x == t).unwrap_or(2)
}

/// The Window Type of a stored or typed name; a double hung window when the
/// name is unknown.
fn type_of_name(name: &str) -> usize {
    type_index(WindowType::from_name(name).unwrap_or_default())
}

const DOOR_TABS: &[Tab] = &[
    on("General"),
    on("Options"),
    on("Casing"),
    on("Lintel"),
    on("Sill/Threshold"),
    on("Lites"),
    on("Jamb"),
    on("Arch"),
    on("Hardware"),
    on("Shutters"),
    on("Opening Indicators"),
    on("Rough Opening"),
    on("Framing"),
    on("Energy Values"),
    on("Layer"),
    on("Materials"),
    on("Label"),
    off("Components"),
    on("Object Information"),
    on("Schedule"),
];

const WINDOW_TABS: &[Tab] = &[
    on("General"),
    on("Options"),
    on("Casing"),
    on("Lintel"),
    on("Sill/Threshold"),
    on("Sash"),
    on("Frame"),
    on("Lites"),
    on("Shape"),
    on("Arch"),
    on("Treatments"),
    on("Shutters"),
    on("Opening Indicators"),
    on("Rough Opening"),
    on("Framing"),
    on("Energy Values"),
    on("Layer"),
    on("Materials"),
    on("Label"),
    off("Components"),
    on("Object Information"),
    on("Schedule"),
];

/// What an opening dialog is bound to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpeningTarget {
    Placed(Id),
    /// Edit > Default Settings > Doors (the template new doors are cloned from).
    DefaultDoor,
    /// Edit > Default Settings > Doors > Exterior Door.
    DefaultExteriorDoor,
    DefaultWindow,
    /// The Defaults dialog of one door or window type (Default Settings, or
    /// a double-click on the tool's button): the opening that type places
    /// (manual pp. 103, 571, 603).
    DefaultType(DefaultKey),
}

impl OpeningTarget {
    /// Key of this target in the app's per-session extras map.
    pub fn key(self) -> Id {
        match self {
            OpeningTarget::Placed(id) => id,
            OpeningTarget::DefaultDoor => DEFAULT_DOOR_KEY,
            OpeningTarget::DefaultExteriorDoor => DEFAULT_EXTERIOR_DOOR_KEY,
            OpeningTarget::DefaultWindow => DEFAULT_WINDOW_KEY,
            OpeningTarget::DefaultType(k) => {
                let slot = DefaultKey::doors()
                    .into_iter()
                    .chain(DefaultKey::windows())
                    .position(|d| d == k)
                    .unwrap_or(0);
                TYPE_KEY_BASE - slot as Id
            }
        }
    }

    /// The Window Defaults (the main one, or a window type's): they hold the
    /// Minimum Separation and the Mulled Unit Defaults.
    pub fn is_window_defaults(self) -> bool {
        match self {
            OpeningTarget::DefaultWindow => true,
            OpeningTarget::DefaultType(k) => k.kind == OpeningKind::Window,
            _ => false,
        }
    }
}

thread_local! {
    static TYPE_DEFAULTS: std::cell::Cell<Option<DefaultKey>> = const { std::cell::Cell::new(None) };
}

/// Asks for the Defaults dialog of a door or window type (the double-click
/// on a Door or Window Tools button, manual p. 603). The app opens it in its
/// next frame.
pub fn request_type_defaults(key: DefaultKey) {
    TYPE_DEFAULTS.with(|c| c.set(Some(key)));
}

/// The type whose Defaults dialog was asked for, once.
pub fn take_type_defaults_request() -> Option<DefaultKey> {
    TYPE_DEFAULTS.with(|c| c.take())
}

/// Door and window dialog values. The style name, thickness, swing angle,
/// jamb/frame width and "show open in plan" are stored with the opening
/// (`Opening.extras`, see [`OpeningExtras::with_stored`] and
/// [`OpeningExtras::to_stored`]); the door or window style, the label and the
/// swing sides live on the [`Opening`] itself; the rest is kept per session by
/// the app. One struct serves both kinds.
#[derive(Clone, Debug, PartialEq)]
pub struct OpeningExtras {
    /// Library style name (e.g. "Door P04"); shown read-only.
    style_name: String,
    window_type: usize,
    thickness: f64,
    swing_angle: f64,
    show_open_2d: bool,
    egress: bool,
    tempered: bool,
    lites_across: u32,
    lites_vertical: u32,
    muntin_width: f64,
    casing_interior: bool,
    casing_interior_width: f64,
    casing_interior_depth: f64,
    casing_interior_reveal: f64,
    casing_exterior: bool,
    casing_exterior_width: f64,
    casing_exterior_depth: f64,
    casing_exterior_reveal: f64,
    has_jamb: bool,
    size_includes_jamb: bool,
    jamb_side: f64,
    jamb_top: f64,
    jamb_bottom: f64,
    fit_to_wall: bool,
    jamb_depth: f64,
    jamb_inset: f64,
    mitered_corners: bool,
    /// Minimum Separation of the Window Defaults, inches.
    min_separation: f64,
    /// The Mulled Unit Defaults of the Window Defaults.
    mulled: MulledSpec,
    /// Ignore Casing for Opening Resize.
    ignore_casing: bool,
    /// The Specification tabs of a default door or window (a placed opening
    /// keeps them on itself, `Opening.extras.spec`).
    spec: OpeningSpec,
    /// Manufacturer widths and the snap option (Default Settings only);
    /// `None` until the dialog changed them, so the plan defaults govern.
    widths: Option<StandardWidths>,
}

impl Default for OpeningExtras {
    fn default() -> Self {
        Self {
            style_name: String::new(),
            window_type: type_index(WindowType::DoubleHung),
            thickness: 1.375,
            swing_angle: 90.0,
            show_open_2d: true,
            egress: false,
            tempered: true,
            lites_across: 1,
            lites_vertical: 1,
            muntin_width: 0.875,
            casing_interior: true,
            casing_interior_width: 3.5,
            casing_interior_depth: 0.75,
            casing_interior_reveal: 0.25,
            casing_exterior: true,
            casing_exterior_width: 3.25,
            casing_exterior_depth: 1.0,
            casing_exterior_reveal: 0.25,
            has_jamb: true,
            size_includes_jamb: false,
            jamb_side: 0.75,
            jamb_top: 0.75,
            jamb_bottom: 0.75,
            fit_to_wall: true,
            jamb_depth: 6.0,
            jamb_inset: 0.0,
            mitered_corners: false,
            min_separation: plan_core::openings::placement::DEFAULT_MIN_SEPARATION,
            mulled: MulledSpec::default(),
            ignore_casing: false,
            spec: OpeningSpec::default(),
            widths: None,
        }
    }
}

impl OpeningExtras {
    /// Extras for a door seeded from the plan defaults. `exterior` selects
    /// which casing (interior or exterior) the defaults' casing values fill.
    pub fn from_door_defaults(d: &OpeningDefaults, exterior: bool) -> Self {
        let mut e = Self {
            style_name: d.style.clone(),
            thickness: d.thickness,
            swing_angle: d.swing_angle,
            jamb_side: d.jamb_width,
            jamb_top: d.jamb_width,
            jamb_bottom: d.jamb_width,
            ..Self::default()
        };
        if exterior {
            e.casing_exterior_width = d.casing_width;
            e.casing_exterior_depth = d.casing_depth;
            e.casing_exterior_reveal = d.reveal;
        } else {
            e.casing_interior_width = d.casing_width;
            e.casing_interior_depth = d.casing_depth;
            e.casing_interior_reveal = d.reveal;
        }
        e
    }

    /// Extras for a window seeded from the plan defaults.
    pub fn from_window_defaults(d: &WindowDefaults) -> Self {
        Self {
            window_type: type_of_name(&d.window_type),
            style_name: d.window_type.clone(),
            min_separation: d.min_separation,
            mulled: d.mulled.clone(),
            ignore_casing: d.ignore_casing,
            egress: d.egress,
            tempered: d.tempered,
            lites_across: d.lites_across,
            lites_vertical: d.lites_vertical,
            jamb_side: d.frame_width,
            jamb_top: d.frame_width,
            jamb_bottom: d.frame_width,
            size_includes_jamb: true,
            ..Self::default()
        }
    }

    /// These extras with the values stored with an opening of `kind` laid
    /// over them.
    pub fn with_stored(mut self, stored: &StoredExtras, kind: OpeningKind) -> Self {
        if let Some(name) = &stored.style_name {
            self.style_name = name.clone();
            if kind == OpeningKind::Window {
                if let Some(t) = WindowType::from_name(name) {
                    self.window_type = type_index(t);
                }
            }
        }
        if kind == OpeningKind::Door {
            if let Some(t) = stored.thickness {
                self.thickness = t;
            }
        }
        // A door's swing angle; a casement, awning or hopper window's too.
        if let Some(a) = stored.swing_angle_deg {
            self.swing_angle = a;
        }
        let jamb = match kind {
            OpeningKind::Door => stored.jamb_width,
            OpeningKind::Window => stored.frame_width,
        };
        if let Some(j) = jamb {
            self.jamb_side = j;
            self.jamb_top = j;
            self.jamb_bottom = j;
        }
        self.show_open_2d = stored.show_open_in_plan;
        self
    }

    /// The stored form of these values for an opening of `kind`; fields the
    /// dialog does not edit (the sash width) come from `keep`.
    pub fn to_stored(&self, kind: OpeningKind, keep: &StoredExtras) -> StoredExtras {
        let mut out = keep.clone();
        match kind {
            OpeningKind::Door => {
                out.style_name = (!self.style_name.is_empty()).then(|| self.style_name.clone());
                out.thickness = Some(self.thickness);
                out.swing_angle_deg = Some(self.swing_angle);
                out.jamb_width = Some(self.jamb_side);
            }
            OpeningKind::Window => {
                out.style_name = Some(WindowType::ALL[self.window_type].name().to_string());
                out.frame_width = Some(self.jamb_side);
                if keep.swing_angle_deg.is_some() || self.swing_angle != 90.0 {
                    out.swing_angle_deg = Some(self.swing_angle);
                }
            }
        }
        out.show_open_in_plan = self.show_open_2d;
        out
    }

    /// The door defaults after editing: `base` with everything these extras
    /// and the edited `draft` can express written over it.
    pub fn to_door_defaults(
        &self,
        draft: &Opening,
        base: &OpeningDefaults,
        exterior: bool,
    ) -> OpeningDefaults {
        let (cw, cd, cr) = if exterior {
            (
                self.casing_exterior_width,
                self.casing_exterior_depth,
                self.casing_exterior_reveal,
            )
        } else {
            (
                self.casing_interior_width,
                self.casing_interior_depth,
                self.casing_interior_reveal,
            )
        };
        OpeningDefaults {
            width: draft.width,
            height: draft.height,
            thickness: self.thickness,
            style: base.style.clone(),
            casing_width: cw,
            casing_depth: cd,
            reveal: cr,
            jamb_width: self.jamb_side,
            swing_angle: self.swing_angle,
            sill_height: draft.sill_height,
        }
    }

    /// The window defaults after editing (see [`Self::to_door_defaults`]).
    pub fn to_window_defaults(&self, draft: &Opening, base: &WindowDefaults) -> WindowDefaults {
        WindowDefaults {
            width: draft.width,
            height: draft.height,
            sill_height: draft.sill_height,
            window_type: WindowType::ALL[self.window_type].name().to_string(),
            min_separation: self.min_separation.max(0.0),
            mulled: self.mulled.clone(),
            ignore_casing: self.ignore_casing,
            frame_width: self.jamb_side,
            sash_width: base.sash_width,
            lites_across: self.lites_across,
            lites_vertical: self.lites_vertical,
            egress: self.egress,
            tempered: self.tempered,
        }
    }
}

/// Facts about the host wall the dialog needs for position rules.
struct HostWall {
    length: f64,
    thickness: f64,
    kind: WallKind,
}

pub struct OpeningDialog {
    frame: SpecDialog,
    form: OpeningForm,
}

struct OpeningForm {
    target: OpeningTarget,
    draft: Opening,
    extras: OpeningExtras,
    /// The label settings being edited (the opening's own, else the
    /// Default Settings of its kind).
    label: LabelSettings,
    /// What `label` started as: only a change is stored with the opening, so
    /// a later change of the defaults still reaches an untouched label.
    label_start: LabelSettings,
    /// The settings the opening had of its own when the dialog opened.
    label_own: Option<LabelSettings>,
    wall: Option<HostWall>,
    /// The other openings on the same wall (for the overlap rule).
    others: Vec<Opening>,
    fields: Fields,
    /// The custom lite dividers as typed (percent, comma separated).
    custom_across: String,
    custom_up: String,
    /// The style whose standard widths the General tab edits, and the text.
    widths_style: OpeningStyle,
    widths_text: String,
    /// The Materials tab's search text and the library it lists (read when
    /// the tab is first shown).
    material_filter: String,
    material_lib: Option<plan_materials::MaterialLibrary>,
    /// The layers the Layer tab lists (empty: the usual ones).
    layer_choices: Vec<String>,
    /// The opening as the dialog first showed it: what an edit is measured
    /// against to tell which groups stop using the default.
    original: Opening,
    /// The Use Default boxes the user touched (they are not released by the
    /// edit that follows).
    dyn_touched: Vec<DynGroup>,
    /// The revision of the mulled unit's specification this dialog started
    /// from.
    unit_revision: u32,
}

/// `0.25, 0.5` as `25, 50`.
fn percent_text(fractions: &[f64]) -> String {
    fractions
        .iter()
        .map(|f| format!("{}", (f * 1000.0).round() / 10.0))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Numbers separated by commas or spaces; anything else is skipped.
fn parse_numbers(text: &str) -> Vec<f64> {
    text.split(|c: char| c == ',' || c.is_whitespace())
        .filter_map(|t| t.trim().parse::<f64>().ok())
        .collect()
}

fn widths_text(widths: &StandardWidths, kind: OpeningKind, style: OpeningStyle) -> String {
    widths
        .for_style(kind, style)
        .iter()
        .map(|w| format!("{}", (w * 100.0).round() / 100.0))
        .collect::<Vec<_>>()
        .join(", ")
}

impl OpeningDialog {
    /// A dialog for an opening placed in `wall`; `others` are the wall's
    /// other openings.
    pub fn for_opening(
        opening: Opening,
        wall: &Wall,
        others: Vec<Opening>,
        extras: OpeningExtras,
    ) -> Self {
        let target = OpeningTarget::Placed(opening.id);
        let host = HostWall {
            length: wall.path_length(),
            thickness: wall.thickness,
            kind: wall.kind,
        };
        Self::build(target, opening, Some(host), others, extras)
    }

    /// A dialog for the default door or window template.
    pub fn for_default(target: OpeningTarget, template: Opening, extras: OpeningExtras) -> Self {
        Self::build(target, template, None, Vec::new(), extras)
    }

    fn build(
        target: OpeningTarget,
        draft: Opening,
        wall: Option<HostWall>,
        others: Vec<Opening>,
        extras: OpeningExtras,
    ) -> Self {
        // A placed opening's own stored values win over the session's.
        let mut extras = match target {
            OpeningTarget::Placed(_) => extras.with_stored(&draft.extras, draft.kind),
            _ => extras,
        };
        let mut draft = draft;
        match target {
            OpeningTarget::Placed(_) => {
                // What the opening holds shows in the tabs.
                extras.lites_across = draft.lites.0.max(1);
                extras.lites_vertical = draft.lites.1.max(1);
                extras.muntin_width = draft.extras.spec.muntin_width;
                extras.casing_interior = draft.extras.spec.casing_interior;
                extras.casing_exterior = draft.extras.spec.casing_exterior;
                if let Some(c) = draft.casing {
                    extras.casing_interior_width = c.width;
                    extras.casing_interior_depth = c.depth;
                    extras.casing_interior_reveal = c.reveal;
                }
                // The exterior casing is stored with the opening too; until
                // it is, the one casing serves both faces.
                if let Some(c) = draft.extras.spec.casing_exterior_size.or(draft.casing) {
                    extras.casing_exterior_width = c.width;
                    extras.casing_exterior_depth = c.depth;
                    extras.casing_exterior_reveal = c.reveal;
                }
            }
            _ => {
                // A default door or window starts from the tab values the
                // session's dialog kept.
                if extras.spec != OpeningSpec::default() {
                    draft.extras.spec = extras.spec.clone();
                }
                extras.muntin_width = draft.extras.spec.muntin_width;
            }
        }
        // The Window Type shown is the one the opening holds; one that
        // disagrees with its style (an old plan) takes the style's.
        if draft.kind == OpeningKind::Window {
            let held = draft.extras.spec.window_type;
            if draft.extras.style_name.is_none()
                || extras.window_type == type_index(WindowType::DoubleHung)
            {
                extras.window_type = type_index(held);
            }
            if WindowType::ALL[extras.window_type].style() != draft.style {
                if let Some(t) = WindowType::for_style(draft.style) {
                    extras.window_type = type_index(t);
                }
            }
        }
        let label = draft.extras.label.clone().unwrap_or_default();
        let title = match draft.kind {
            OpeningKind::Door => "Door Specification",
            OpeningKind::Window => "Window Specification",
        };
        let key = match draft.kind {
            OpeningKind::Door => "door",
            OpeningKind::Window => "window",
        };
        let kind = draft.kind;
        let (custom_across, custom_up) = (
            percent_text(&draft.extras.spec.custom_across),
            percent_text(&draft.extras.spec.custom_up),
        );
        let widths_text = widths_text(
            &extras.widths.clone().unwrap_or_default(),
            kind,
            OpeningStyle::default_for(kind),
        );
        let original = draft.clone();
        let unit_revision = draft.extras.spec.mulled.as_ref().map_or(0, |m| m.revision);
        let me = Self {
            frame: SpecDialog::new(title, key),
            form: OpeningForm {
                target,
                label_own: draft.extras.label.clone(),
                label: label.clone(),
                label_start: label,
                draft,
                extras,
                wall,
                others,
                fields: Fields::default(),
                custom_across,
                custom_up,
                widths_style: OpeningStyle::default_for(kind),
                widths_text,
                material_filter: String::new(),
                material_lib: None,
                layer_choices: Vec::new(),
                original,
                dyn_touched: Vec::new(),
                unit_revision,
            },
        };
        // What the first sync writes (the stored values) is not an edit: the
        // Use Default flags stay as the opening has them, and the opening as it
        // stands then is what edits are measured against.
        let mut me = me;
        let flags = me.form.draft.extras.spec.dynamic;
        me.sync_stored();
        me.form.draft.extras.spec.dynamic = flags;
        me.form.original = me.form.draft.clone();
        me
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let outcome = self.frame.show(ctx, &mut self.form);
        self.sync_stored();
        outcome
    }

    /// Copies the values that persist with the opening into the draft, so
    /// accepting the dialog stores them in `Opening.extras`.
    pub(crate) fn sync_stored(&mut self) {
        let f = &mut self.form;
        f.draft.extras = f.extras.to_stored(f.draft.kind, &f.draft.extras);
        // The Lites and Casing tabs act on the opening itself.
        f.draft.lites = (f.extras.lites_across.max(1), f.extras.lites_vertical.max(1));
        let spec = &mut f.draft.extras.spec;
        spec.muntin_width = f.extras.muntin_width.max(0.125);
        spec.casing_interior = f.extras.casing_interior;
        spec.casing_exterior = f.extras.casing_exterior;
        let casing = Casing {
            width: f.extras.casing_interior_width,
            depth: f.extras.casing_interior_depth,
            reveal: f.extras.casing_interior_reveal,
        };
        if casing != Casing::default() || f.draft.casing.is_some() {
            f.draft.casing = Some(casing);
        }
        let outside = Casing {
            width: f.extras.casing_exterior_width,
            depth: f.extras.casing_exterior_depth,
            reveal: f.extras.casing_exterior_reveal,
        };
        if f.draft.extras.spec.casing_exterior_size.is_some()
            || (outside != casing && matches!(f.target, OpeningTarget::Placed(_)))
        {
            f.draft.extras.spec.casing_exterior_size = Some(outside);
        }
        if !matches!(f.target, OpeningTarget::Placed(_)) {
            f.extras.spec = f.draft.extras.spec.clone();
        }
        // The Window Type the list shows is the one the window holds.
        if f.draft.kind == OpeningKind::Window {
            f.draft.extras.spec.window_type = WindowType::ALL[f.extras.window_type];
        }
        // A lowered ceiling or raised floor of 0 is none (manual p. 640).
        let bay = &mut f.draft.extras.spec.bay;
        if bay.lowered_ceiling.is_some_and(|c| c.height <= 0.0) {
            bay.lowered_ceiling = None;
        }
        if bay.raised_floor.is_some_and(|r| r.height <= 0.0) {
            bay.raised_floor = None;
        }
        // A group of settings the user edited stops using the default
        // (manual p. 103), unless the Use Default box was touched.
        if matches!(f.target, OpeningTarget::Placed(_)) {
            for g in DynGroup::of_kind(f.draft.kind) {
                if f.dyn_touched.contains(g) {
                    continue;
                }
                if f.original.extras.spec.dynamic.get(*g)
                    && f.draft.extras.spec.dynamic.get(*g)
                    && !plan_core::openings::group_eq(&f.original, &f.draft, *g)
                {
                    f.draft.extras.spec.dynamic.set(*g, false);
                }
            }
        }
        // The label is stored with a placed opening once the Label tab
        // changed it; until then the Default Settings keep governing it.
        f.draft.extras.label = if f.label != f.label_start {
            Some(f.label.clone())
        } else {
            f.label_own.clone()
        };
    }

    /// The label settings new labels follow (Default Settings > Labels): a
    /// placed opening without its own settings starts from these. Call before
    /// the dialog is shown.
    pub fn with_label_defaults(mut self, defaults: &OpeningLabelDefaults) -> Self {
        let f = &mut self.form;
        let base = defaults.for_kind(f.draft.kind).clone();
        f.label = f.label_own.clone().unwrap_or(base);
        f.label_start = f.label.clone();
        self
    }

    /// The layers of the plan, for the Layer tab to list. Call before the
    /// dialog is shown; without it the tab lists the usual opening layers.
    #[allow(dead_code)] // main.rs calls it where the dialog opens (docs/integration-queue.md)
    pub fn with_layer_choices(mut self, layers: Vec<String>) -> Self {
        self.form.layer_choices = layers;
        self
    }

    /// The label settings as edited (what the Default Settings dialogs store).
    pub fn label_settings(&self) -> &LabelSettings {
        &self.form.label
    }

    /// Writes what a Default Settings dialog edited beyond the door and window
    /// templates into the variant defaults: the standard widths and snap
    /// option, and the tab values new doors or windows start with.
    pub fn apply_to_variants(&self, v: &mut OpeningVariantDefaults) {
        let f = &self.form;
        if matches!(f.target, OpeningTarget::Placed(_)) {
            return;
        }
        if let Some(w) = &f.extras.widths {
            v.widths = w.clone();
        }
        match f.draft.kind {
            OpeningKind::Door => v.door_spec = f.draft.extras.spec.clone(),
            OpeningKind::Window => v.window_spec = f.draft.extras.spec.clone(),
        }
        // The default of this door or window type: the opening the Defaults
        // dialog shows, so new ones start from it and the placed ones that
        // use the default follow it (manual pp. 103, 571, 603).
        let key = match f.target {
            OpeningTarget::DefaultDoor => DefaultKey::new(OpeningKind::Door, f.draft.style, false),
            OpeningTarget::DefaultExteriorDoor => {
                DefaultKey::new(OpeningKind::Door, f.draft.style, true)
            }
            OpeningTarget::DefaultType(k) => k,
            _ => DefaultKey::main_window(),
        };
        // Only once the dialog changed something (the opening, or the
        // settings kept beside it such as Minimum Separation and the Mulled
        // Unit Defaults), or the type already has a default: opening and
        // closing it leaves the plan defaults alone.
        if f.draft != f.original
            || f.extras != OpeningExtras::default()
            || v.type_default(key).is_some()
        {
            v.set_type_default(key, f.draft.clone());
        }
    }

    pub fn target(&self) -> OpeningTarget {
        self.form.target
    }

    pub fn draft(&self) -> &Opening {
        &self.form.draft
    }

    /// Runs the copy the dialog does after every frame (tests of other
    /// modules call it after editing the draft).
    #[cfg(test)]
    pub fn sync_stored_for_test(&mut self) {
        self.sync_stored();
    }

    /// Tests edit the draft as the form's controls would.
    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut Opening {
        &mut self.form.draft
    }

    pub fn extras(&self) -> &OpeningExtras {
        &self.form.extras
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
}

impl OpeningExtras {
    /// The standard widths and snap option a Default Settings dialog set
    /// this session; `None` while the plan defaults govern.
    pub fn standard_widths(&self) -> Option<&StandardWidths> {
        self.widths.as_ref()
    }

    /// The tab values a Default Settings dialog set this session, when they
    /// differ from a plain door or window.
    pub fn default_spec(&self) -> Option<&OpeningSpec> {
        (self.spec != OpeningSpec::default()).then_some(&self.spec)
    }
}

/// Places a new opening on `wall_id` the way `Project::add_opening` does
/// (clamp into the wall, reject overlaps and walls that are too short), but
/// starting from `template` so the Default Settings sizes apply.
pub fn place_from_template(
    project: &mut Project,
    floor: usize,
    wall_id: Id,
    center_offset: f64,
    template: &Opening,
) -> Option<Id> {
    let wall_len = project.floors[floor].wall(wall_id)?.path_length();
    let half = template.width * 0.5;
    if wall_len < template.width + 2.0 * OPENING_MARGIN {
        return None;
    }
    let id = project.alloc_id();
    let mut opening = template.clone();
    opening.id = id;
    opening.wall_id = wall_id;
    opening.center_offset =
        center_offset.clamp(half + OPENING_MARGIN, wall_len - half - OPENING_MARGIN);
    let separation = project.opening_display.min_separation;
    let f = &mut project.floors[floor];
    let overlaps = f
        .openings_on(wall_id)
        .any(|o| plan_core::openings::placement::conflict_with(&opening, o, separation));
    if overlaps {
        return None;
    }
    f.openings.push(opening);
    // The "Doors, Labels" and "Windows, Labels" layers exist from the first
    // opening on, also in plans saved before them (DW-63).
    project.ensure_opening_label_layers();
    Some(id)
}

/// Whether an arch shapes a unit of this kind and style in 3D (the plan marks
/// it for every style).
fn plan_3d_arch_applies(kind: OpeningKind, style: OpeningStyle) -> bool {
    match kind {
        OpeningKind::Window => matches!(
            style,
            OpeningStyle::Window | OpeningStyle::Fixed | OpeningStyle::Casement
        ),
        OpeningKind::Door => matches!(
            style,
            OpeningStyle::Hinged
                | OpeningStyle::DoubleDoor
                | OpeningStyle::Fixed
                | OpeningStyle::Doorway
        ),
    }
}

fn overlap(a: &Opening, b: &Opening) -> bool {
    // The shared placement rules: a window standing over a door shares no
    // height with it, and two windows may touch (DW-4).
    plan_core::openings::placement::conflict(a, b)
}

impl OpeningForm {
    fn is_door(&self) -> bool {
        self.draft.kind == OpeningKind::Door
    }

    fn clamp_center(&mut self) {
        if let Some(w) = &self.wall {
            let half = self.draft.width * 0.5;
            let (lo, hi) = (half + OPENING_MARGIN, w.length - half - OPENING_MARGIN);
            if hi >= lo {
                self.draft.center_offset = self.draft.center_offset.clamp(lo, hi);
            }
        }
    }

    fn size_changed(&mut self) {
        let max_w = self
            .wall
            .as_ref()
            .map_or(480.0, |w| (w.length - 2.0 * OPENING_MARGIN).max(6.0));
        self.draft.width = self.draft.width.clamp(6.0, max_w);
        self.draft.height = self.draft.height.max(6.0);
        self.clamp_center();
    }

    fn general(&mut self, ui: &mut Ui) {
        let door = self.is_door();
        section(ui, "General");
        if door {
            row(ui, "Door Style", |ui| {
                egui::ComboBox::from_id_salt("door_style")
                    .selected_text(self.draft.style.name(OpeningKind::Door))
                    .show_ui(ui, |ui| {
                        for s in OpeningStyle::DOORS {
                            ui.selectable_value(
                                &mut self.draft.style,
                                s,
                                s.name(OpeningKind::Door),
                            );
                        }
                    })
                    .response
                    .on_hover_text("The style sets the plan symbol and the 3D door");
            });
            if !self.extras.style_name.is_empty() {
                row(ui, "Library Style", |ui| {
                    ui.label(self.extras.style_name.as_str());
                });
            }
            row(ui, "Door Type", |ui| dis_combo(ui, "door_type", "Hinged"));
        } else {
            let before = self.draft.style;
            row(ui, "Window Style", |ui| {
                egui::ComboBox::from_id_salt("window_style")
                    .selected_text(self.draft.style.name(OpeningKind::Window))
                    .show_ui(ui, |ui| {
                        for s in OpeningStyle::WINDOWS {
                            ui.selectable_value(
                                &mut self.draft.style,
                                s,
                                s.name(OpeningKind::Window),
                            );
                        }
                    })
                    .response
                    .on_hover_text("The style sets the plan symbol and the 3D window");
            });
            if self.draft.style != before {
                // The type follows the style it is drawn by.
                if let Some(t) = WindowType::for_style(self.draft.style) {
                    if WindowType::ALL[self.extras.window_type].style() != self.draft.style {
                        self.extras.window_type = type_index(t);
                        self.draft.extras.spec.window_type = t;
                    }
                }
            }
            if self.draft.style.projects() {
                self.bay_general(ui);
            } else {
                self.window_type_rows(ui);
            }
        }
        row(ui, "Window Level", |ui| {
            let mut level = i32::from(self.draft.extras.spec.level);
            if ui
                .add(egui::DragValue::new(&mut level).range(0..=9))
                .on_hover_text(
                    "Level 0 draws in the layer colour and is picked first; the others draw light grey",
                )
                .changed()
            {
                self.draft.extras.spec.level = level.clamp(0, 9) as u8;
            }
        });
        if door
            && matches!(
                self.draft.style,
                OpeningStyle::Hinged | OpeningStyle::Sliding
            )
        {
            row(ui, "Door Location", |ui| {
                let e = &mut self.draft.extras.spec.exterior_door;
                ui.radio_value(e, None, "From wall");
                ui.radio_value(e, Some(false), "Interior");
                ui.radio_value(e, Some(true), "Exterior");
            });
        }
        if self.target.is_window_defaults() {
            row(ui, "Minimum Separation", |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.extras.min_separation)
                        .range(0.0..=48.0)
                        .speed(0.25)
                        .suffix(" in"),
                )
                .on_hover_text(
                    "How close window and door units may stand, and the width of the casing windows share",
                );
            });
            ui.checkbox(
                &mut self.extras.ignore_casing,
                "Ignore Casing for Opening Resize",
            )
            .on_hover_text(
                "Doors and windows may run right up to an intersecting wall instead of stopping where their casing meets it",
            );
        }

        section(ui, "Size and Position");
        if self
            .fields
            .length_row(ui, "Width", "width", &mut self.draft.width)
        {
            self.size_changed();
        }
        if self
            .fields
            .length_row(ui, "Height", "height", &mut self.draft.height)
        {
            self.size_changed();
        }
        if door
            && self
                .fields
                .length_row(ui, "Thickness", "thickness", &mut self.extras.thickness)
        {
            self.extras.thickness = self.extras.thickness.max(0.25);
        }
        super::elevation_ref::row(ui, "Elevation Reference");
        let mut top = self.draft.sill_height + self.draft.height;
        if self.fields.length_row(ui, "Floor to Top", "top", &mut top) {
            self.draft.height = (top - self.draft.sill_height).max(6.0);
        }
        let mut bottom = self.draft.sill_height;
        if self
            .fields
            .length_row(ui, "Floor to Bottom", "bottom", &mut bottom)
        {
            self.draft.sill_height = bottom.max(0.0);
        }
        // Egress size of a bedroom window / the required exit door (code minimums).
        super::code_notice::opening_notices(ui, &mut self.draft);

        section(ui, "Position");
        match self.wall.as_ref().map(|w| w.length) {
            Some(len) => {
                let mut center = self.draft.center_offset;
                if self
                    .fields
                    .length_row(ui, "Distance from Wall Start", "center", &mut center)
                {
                    self.draft.center_offset = center;
                    self.clamp_center();
                }
                ui.horizontal(|ui| {
                    ui.add_space(super::LABEL_WIDTH + ui.spacing().item_spacing.x);
                    if ui.button("Center on wall").clicked() {
                        self.draft.center_offset = len * 0.5;
                        self.clamp_center();
                    }
                });
                ui.weak(format!(
                    "To the opening center, along a {} wall. Kept at least {} from the wall ends.",
                    fmt_short(len),
                    fmt_short(OPENING_MARGIN)
                ));
            }
            None => {
                ui.add_enabled_ui(false, |ui| {
                    row(ui, "Distance from Wall Start", |ui| {
                        ui.label("set when placed");
                    });
                });
            }
        }
        if !matches!(self.target, OpeningTarget::Placed(_)) {
            self.standard_widths(ui);
        }
    }

    /// The Window Type list with Use Default first, then Percent Open or
    /// Swing Angle by the type, the Component Options and the Louver Size
    /// (manual pp. 619, 620).
    fn window_type_rows(&mut self, ui: &mut Ui) {
        let placed = matches!(self.target, OpeningTarget::Placed(_));
        let use_default = placed && self.draft.extras.spec.dynamic.window_type;
        let current = WindowType::ALL[self.extras.window_type];
        let text = if use_default {
            "Use Default"
        } else {
            current.name()
        };
        row(ui, "Window Type", |ui| {
            egui::ComboBox::from_id_salt("window_type")
                .selected_text(text)
                .show_ui(ui, |ui| {
                    if placed
                        && ui
                            .selectable_label(use_default, "Use Default")
                            .on_hover_text("Follows the type of the Window Defaults")
                            .clicked()
                    {
                        self.draft.extras.spec.dynamic.window_type = true;
                        if !self.dyn_touched.contains(&DynGroup::Type) {
                            self.dyn_touched.push(DynGroup::Type);
                        }
                    }
                    for (i, t) in WindowType::ALL.iter().enumerate() {
                        let on = !use_default && self.extras.window_type == i;
                        if ui.selectable_label(on, t.name()).clicked() {
                            self.extras.window_type = i;
                            self.draft.extras.spec.window_type = *t;
                            self.draft.extras.spec.dynamic.window_type = false;
                            if !self.draft.style.projects()
                                && self.draft.style != OpeningStyle::WallNiche
                            {
                                self.draft.style = t.style();
                            }
                        }
                    }
                })
                .response
                .on_hover_text("The window type is stored with the window");
        });
        let wt = WindowType::ALL[self.extras.window_type];
        match wt.open_mode() {
            OpenMode::Percent => {
                row(ui, "Percent Open", |ui| {
                    let spec = &mut self.draft.extras.spec;
                    let mut pct = (spec.open_fraction * 100.0).round();
                    if ui
                        .add(
                            egui::DragValue::new(&mut pct)
                                .range(0.0..=100.0)
                                .suffix(" %"),
                        )
                        .on_hover_text("How far open the window is in 3D views")
                        .changed()
                    {
                        spec.open_fraction = pct / 100.0;
                    }
                });
            }
            OpenMode::Angle => {
                row(ui, "Swing Angle", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.extras.swing_angle)
                            .range(0.0..=180.0)
                            .speed(1.0)
                            .suffix("\u{B0}"),
                    )
                    .on_hover_text("How far open the sash swings in 3D views");
                });
            }
            OpenMode::Never => {}
        }
        if let Some(label) = wt.component_size_label() {
            let mut size = self.draft.extras.spec.component_size;
            if self
                .fields
                .length_row(ui, label, "component_size", &mut size)
            {
                self.draft.extras.spec.component_size = size.max(0.0);
            }
            ui.weak("A size of 0 makes the components the same size.");
        }
        if wt.is_louvered() {
            let mut size = self.draft.extras.spec.louver_size;
            if self
                .fields
                .length_row(ui, "Louver Size", "louver_size", &mut size)
            {
                self.draft.extras.spec.louver_size = size.max(0.25);
            }
        }
    }

    /// The Use Default box of a dynamic group of a placed opening (manual
    /// p. 103). Editing the group's values releases it; ticking it again
    /// makes the opening follow the default once more.
    fn use_default_row(&mut self, ui: &mut Ui, g: DynGroup) {
        if !matches!(self.target, OpeningTarget::Placed(_))
            || !DynGroup::of_kind(self.draft.kind).contains(&g)
        {
            return;
        }
        let mut on = self.draft.extras.spec.dynamic.get(g);
        if ui
            .checkbox(&mut on, "Use Default")
            .on_hover_text(
                "Follows the default of this type: when the default changes, so does this opening",
            )
            .changed()
        {
            self.draft.extras.spec.dynamic.set(g, on);
            if !self.dyn_touched.contains(&g) {
                self.dyn_touched.push(g);
            }
        }
    }

    /// Swing side, hinge side (when there is one) and swing angle.
    fn swing_controls(&mut self, ui: &mut Ui, hinge: bool) {
        section(ui, "Swing");
        row(ui, "Swing side", |ui| {
            ui.radio_value(&mut self.draft.swing_flipped, false, "Left")
                .on_hover_text("Swings to the wall's normal side");
            ui.radio_value(&mut self.draft.swing_flipped, true, "Right")
                .on_hover_text("Swings to the other side of the wall (flipped)");
        });
        if hinge {
            row(ui, "Hinge side", |ui| {
                ui.radio_value(&mut self.draft.hinge_at_end, false, "Start")
                    .on_hover_text("Hinge on the wall-start jamb");
                ui.radio_value(&mut self.draft.hinge_at_end, true, "End")
                    .on_hover_text("Hinge on the wall-end jamb");
            });
        }
        row(ui, "Swing Angle", |ui| {
            ui.add(
                egui::DragValue::new(&mut self.extras.swing_angle)
                    .range(0.0..=180.0)
                    .speed(1.0)
                    .suffix("\u{B0}"),
            );
        });
    }

    /// A start/end choice stored in `hinge_at_end`.
    fn toward_row(&mut self, ui: &mut Ui, label: &str) {
        row(ui, label, |ui| {
            ui.radio_value(&mut self.draft.hinge_at_end, false, "Wall start");
            ui.radio_value(&mut self.draft.hinge_at_end, true, "Wall end");
        });
    }

    /// A side-of-the-wall choice stored in `swing_flipped`.
    fn side_row(&mut self, ui: &mut Ui, label: &str) {
        row(ui, label, |ui| {
            ui.radio_value(&mut self.draft.swing_flipped, false, "Left");
            ui.radio_value(&mut self.draft.swing_flipped, true, "Right");
        });
    }

    fn door_options(&mut self, ui: &mut Ui) {
        let style = self.draft.style;
        let width = self.draft.width;
        match style {
            OpeningStyle::Hinged | OpeningStyle::Shower => self.swing_controls(ui, true),
            OpeningStyle::DoubleDoor => self.swing_controls(ui, false),
            OpeningStyle::Sliding => {
                section(ui, "Sliding Door");
                ui.label(format!(
                    "{} panels, calculated from the width",
                    sliding_panels(width)
                ));
                self.toward_row(ui, "Opens toward");
            }
            OpeningStyle::Pocket => {
                section(ui, "Pocket Door");
                self.toward_row(ui, "Pocket on");
                ui.weak("The leaf slides into a pocket in the wall beside the opening.");
            }
            OpeningStyle::Bifold => {
                section(ui, "Bifold Door");
                let n = bifold_panels(width);
                ui.label(format!("{n} panels"));
                self.side_row(ui, "Folds toward");
                if n == 2 {
                    self.toward_row(ui, "Hinged at");
                }
            }
            OpeningStyle::Garage => {
                section(ui, "Garage Door");
                self.side_row(ui, "Overhead tracks on");
            }
            OpeningStyle::Barn => {
                section(ui, "Barn Door");
                self.side_row(ui, "Hung on");
                self.toward_row(ui, "Slides toward");
            }
            OpeningStyle::Doorway => {
                section(ui, "Doorway");
                ui.weak("A cased opening: no leaf and no swing.");
            }
            _ => {}
        }
        self.mulled_unit_section(ui);
        section(ui, "Open/Close Display");
        ui.checkbox(&mut self.extras.show_open_2d, "Show Open in 2D")
            .on_hover_text("Draws the open leaf and swing; unchecked draws the door closed");
        self.show_open_3d(ui);
        if matches!(style, OpeningStyle::Hinged | OpeningStyle::DoubleDoor) {
            section(ui, "Door Panels");
            let calc = self.draft.extras.spec.calc_panels;
            row(ui, "Panels", |ui| {
                if ui
                    .radio(!calc && style == OpeningStyle::Hinged, "Single Door Only")
                    .clicked()
                {
                    self.draft.style = OpeningStyle::Hinged;
                    self.draft.extras.spec.calc_panels = false;
                }
                if ui
                    .radio(
                        !calc && style == OpeningStyle::DoubleDoor,
                        "Double Door Only",
                    )
                    .clicked()
                {
                    self.draft.style = OpeningStyle::DoubleDoor;
                    self.draft.extras.spec.calc_panels = false;
                }
                if ui.radio(calc, "Calculate from Width").clicked() {
                    self.draft.extras.spec.calc_panels = true;
                }
            });
            if calc {
                let n = door_panel_count(self.draft.effective_style(), width);
                ui.label(format!(
                    "{} from the width ({})",
                    if n == 2 { "Double door" } else { "Single door" },
                    fmt_short(width)
                ));
            }
        }
        if self.draft.effective_style() == OpeningStyle::DoubleDoor {
            self.door_swing_group(ui);
        }
        if matches!(style, OpeningStyle::Hinged | OpeningStyle::DoubleDoor) {
            ui.checkbox(
                &mut self.draft.extras.spec.swings_both,
                "Swings Both Directions",
            )
            .on_hover_text("A double-acting door: the plan draws the swing arc on both sides");
        }
        self.recessed_into_wall(ui);
        ui.add_enabled_ui(false, |ui| {
            dis_check(ui, "All Glass", false);
            section(ui, "Plan Display");
            row(ui, "Top Edge", |ui| {
                dis_radio(ui, "Automatic", true);
                dis_radio(ui, "Show Top Edge", false);
                dis_radio(ui, "Hide Top Edge", false);
            });
            section(ui, "Safety");
            dis_check(ui, "Tempered Glass", false);
            dis_check(ui, "Fire Door", false);
            section(ui, "Plinth Blocks");
            dis_check(ui, "Interior Plinth Block", false);
            dis_check(ui, "Exterior Plinth Block", false);
        });
    }

    /// Options tab, Bay Roof: the roof over a bay, box or bow window.
    fn bay_roof(&mut self, ui: &mut Ui) {
        section(ui, "Bay Roof");
        let box_window = self.draft.style == OpeningStyle::BoxWindow;
        let roof = &mut self.draft.extras.spec.bay_roof;
        row(ui, "Roof", |ui| {
            egui::ComboBox::from_id_salt("bay_roof_kind")
                .selected_text(roof.kind.name())
                .show_ui(ui, |ui| {
                    for k in plan_core::openings::BayRoofKind::ALL {
                        ui.selectable_value(&mut roof.kind, k, k.name());
                    }
                });
        });
        let sloped = matches!(
            roof.kind.resolved(box_window),
            plan_core::openings::BayRoofKind::Hip | plan_core::openings::BayRoofKind::Shed
        );
        ui.add_enabled_ui(sloped, |ui| {
            row(ui, "Pitch (rise per 12)", |ui| {
                ui.add(
                    egui::DragValue::new(&mut roof.pitch)
                        .range(0.0..=24.0)
                        .speed(0.1),
                )
            });
        });
        let mut overhang = roof.overhang;
        let changed = ui
            .add_enabled_ui(sloped, |ui| {
                self.fields
                    .length_row(ui, "Overhang", "bay_roof_overhang", &mut overhang)
            })
            .inner;
        if changed {
            self.draft.extras.spec.bay_roof.overhang = overhang.max(0.0);
        }
    }

    fn window_options(&mut self, ui: &mut Ui) {
        match self.draft.style {
            OpeningStyle::Casement => self.swing_controls(ui, true),
            OpeningStyle::SlidingWindow => {
                section(ui, "Sliding Window");
                ui.label("Two overlapping sashes on separate tracks");
                self.toward_row(ui, "Opens toward");
            }
            OpeningStyle::Awning | OpeningStyle::Hopper => {
                let awning = self.draft.style == OpeningStyle::Awning;
                section(
                    ui,
                    if awning {
                        "Awning Window"
                    } else {
                        "Hopper Window"
                    },
                );
                ui.checkbox(&mut self.draft.swing_flipped, "Open to the other side")
                    .on_hover_text("An awning opens outward and a hopper inward; this reverses it");
            }
            OpeningStyle::BayWindow | OpeningStyle::BowWindow | OpeningStyle::BoxWindow => {
                section(ui, "Projecting Window");
                ui.label(format!(
                    "Projects {} from the exterior face with its own seat and roof",
                    fmt_short(self.draft.extras.spec.bay.depth_for(self.draft.style))
                ));
                ui.checkbox(&mut self.draft.swing_flipped, "Project to the other side");
                self.bay_options(ui);
            }
            OpeningStyle::PassThrough => {
                section(ui, "Pass-Through");
                ui.weak("An opening through the wall with no glazing.");
            }
            OpeningStyle::WallNiche => {
                section(ui, "Wall Niche");
                ui.weak("A recess cut from the room side; the wall behind it stays.");
                let thick = self.wall.as_ref().map_or(4.5, |w| w.thickness);
                let mut depth = self.draft.extras.spec.niche_depth;
                if self
                    .fields
                    .length_row(ui, "Niche Depth", "niche_depth", &mut depth)
                {
                    self.draft.extras.spec.niche_depth = depth.max(0.5);
                }
                ui.weak(format!(
                    "Cut {} deep (3 1/2\" by default), always leaving 1\" of wall behind it.",
                    fmt_short(self.draft.niche_depth(thick))
                ));
            }
            _ => {}
        }
        self.mulled_unit_section(ui);
        section(ui, "Options");
        dis_check(ui, "Interior Corner Block", false);
        dis_check(ui, "Exterior Corner Block", false);
        session_check(ui, &mut self.extras.egress, "Egress");
        session_check(ui, &mut self.extras.tempered, "Tempered Glass");
        section(ui, "Display");
        ui.checkbox(&mut self.extras.show_open_2d, "Show Open in 2D");
        self.show_open_3d(ui);
        ui.add_enabled_ui(false, |ui| {
            section(ui, "Recessed into Wall");
            dis_check(ui, "Recessed To Layer", true);
        });
    }

    /// "Show Open in 3D" and its Open slider (`0..=100 %` of the swing angle
    /// or of the way a sliding, pocket, bifold, barn or garage door travels).
    fn show_open_3d(&mut self, ui: &mut Ui) {
        let spec = &mut self.draft.extras.spec;
        ui.checkbox(&mut spec.show_open_in_3d, "Show Open in 3D")
            .on_hover_text("Builds this opening's leaf or panels open in the 3D view");
        ui.add_enabled_ui(spec.show_open_in_3d, |ui| {
            row(ui, "Open", |ui| {
                let mut pct = (spec.open_fraction * 100.0).round();
                if ui
                    .add(egui::Slider::new(&mut pct, 0.0..=100.0).suffix(" %"))
                    .changed()
                {
                    spec.open_fraction = pct / 100.0;
                }
            });
        });
    }

    fn casing(&mut self, ui: &mut Ui) {
        self.use_default_row(ui, DynGroup::Casing);
        let e = &mut self.extras;
        let exterior_ok = self
            .wall
            .as_ref()
            .is_none_or(|w| w.kind == WallKind::Exterior);
        section(ui, "Interior Casing");
        ui.checkbox(&mut e.casing_interior, "Use Interior Casing")
            .on_hover_text("Casing is drawn in 3D when Casing is on in the 3D view");
        ui.add_enabled_ui(e.casing_interior, |ui| {
            self.fields
                .length_row(ui, "Width", "ci_width", &mut e.casing_interior_width);
            self.fields
                .length_row(ui, "Depth", "ci_depth", &mut e.casing_interior_depth);
            self.fields
                .length_row(ui, "Reveal", "ci_reveal", &mut e.casing_interior_reveal);
        });
        section(ui, "Exterior Casing");
        ui.add_enabled_ui(exterior_ok, |ui| {
            ui.checkbox(&mut e.casing_exterior, "Use Exterior Casing")
                .on_hover_text("Casing is drawn in 3D when Casing is on in the 3D view");
            ui.add_enabled_ui(e.casing_exterior, |ui| {
                self.fields
                    .length_row(ui, "Width", "ce_width", &mut e.casing_exterior_width);
                self.fields
                    .length_row(ui, "Depth", "ce_depth", &mut e.casing_exterior_depth);
                self.fields
                    .length_row(ui, "Reveal", "ce_reveal", &mut e.casing_exterior_reveal);
            });
        });
        if !exterior_ok {
            ui.weak("Exterior casing is unavailable in an interior wall.");
        }
        section(ui, "Profile");
        let spec = &mut self.draft.extras.spec;
        row(ui, "Casing profile", |ui| {
            egui::ComboBox::from_id_salt("casing_profile")
                .selected_text(spec.casing_profile.name())
                .show_ui(ui, |ui| {
                    for p in CasingProfile::ALL {
                        ui.selectable_value(&mut spec.casing_profile, p, p.name());
                    }
                });
        });
        section(ui, "Plan Display");
        ui.checkbox(
            &mut self.draft.extras.spec.casing_in_plan,
            "Show Casing in Plan",
        )
        .on_hover_text("Small rectangles on the wall faces beside the jambs; a mulled unit has one pair for the whole unit");
        ui.add_enabled_ui(false, |ui| {
            section(ui, "Double Wall Options");
            row(ui, "Double wall", |ui| {
                dis_radio(ui, "Through", true);
                dis_radio(ui, "Enlarged", false);
                dis_radio(ui, "Double", false);
            });
        });
        section(ui, "Curved Wall Casing");
        row(ui, "Curved wall", |ui| {
            for m in plan_core::openings::spec::CurvedCasing::ALL {
                ui.radio_value(&mut self.draft.extras.spec.curved_casing, m, m.name());
            }
        });
        ui.weak("Radial boards stand square to the wall where each one is; straight boards are square at the middle of the opening; parallel boards bend with the curve.");
    }

    /// The Jamb tab (doors) and Frame tab (windows) share their controls.
    fn jamb_or_frame(&mut self, ui: &mut Ui) {
        let group = if self.is_door() {
            DynGroup::Jamb
        } else {
            DynGroup::Frame
        };
        self.use_default_row(ui, group);
        let door = self.is_door();
        let e = &mut self.extras;
        let (has, positioning) = if door {
            ("Has Jamb", "Door Size Includes Jamb")
        } else {
            ("Has Frame", "Window Size Includes Frame")
        };
        section(ui, if door { "Jamb" } else { "Frame" });
        ui.checkbox(&mut e.has_jamb, has)
            .on_hover_text(super::SESSION_NOTE);
        ui.add_enabled_ui(e.has_jamb, |ui| {
            // Stored with the opening: the plan clears a wider opening in the
            // wall when the size leaves the frame out (DW-82).
            let spec = &mut self.draft.extras.spec;
            row(ui, "Positioning", |ui| {
                ui.radio_value(&mut spec.size_includes_frame, true, positioning);
                ui.radio_value(
                    &mut spec.size_includes_frame,
                    false,
                    positioning.replace("Includes", "Excludes"),
                );
            });
            if door {
                ui.checkbox(&mut spec.jamb_in_plan, "Show Jamb in Plan")
                    .on_hover_text("The jamb blocks beside each jamb line of the plan symbol");
            }
            self.fields
                .length_row(ui, "Sides Width", "j_side", &mut e.jamb_side);
            self.fields
                .length_row(ui, "Top Width", "j_top", &mut e.jamb_top);
            if !door {
                self.fields
                    .length_row(ui, "Bottom Width", "j_bottom", &mut e.jamb_bottom);
            }
            ui.checkbox(
                &mut e.fit_to_wall,
                if door {
                    "Fit Jamb to Wall"
                } else {
                    "Fit Frame to Wall"
                },
            );
            ui.add_enabled_ui(!e.fit_to_wall, |ui| {
                self.fields
                    .length_row(ui, "Depth", "j_depth", &mut e.jamb_depth);
            });
            self.fields
                .length_row(ui, "Inset", "j_inset", &mut e.jamb_inset);
        });
        if !door {
            section(ui, "Options");
            row(ui, "Corner Join", |ui| {
                ui.radio_value(&mut e.mitered_corners, false, "Post");
                ui.radio_value(&mut e.mitered_corners, true, "Mitered");
            });
        }
    }

    fn lites(&mut self, ui: &mut Ui) {
        let e = &mut self.extras;
        let spec = &mut self.draft.extras.spec;
        section(ui, "Lites");
        row(ui, "Type", |ui| {
            egui::ComboBox::from_id_salt("lite_type")
                .selected_text(spec.lite_style.name())
                .show_ui(ui, |ui| {
                    for st in LiteStyle::ALL {
                        ui.selectable_value(&mut spec.lite_style, st, st.name());
                    }
                });
        });
        let custom = spec.lite_style == LiteStyle::Custom;
        let diamond = spec.lite_style == LiteStyle::Diamond;
        let prairie = spec.lite_style == LiteStyle::Prairie;
        ui.add_enabled_ui(!custom && !prairie, |ui| {
            row(
                ui,
                if diamond {
                    "Diamonds Across"
                } else {
                    "Lites Across"
                },
                |ui| {
                    ui.add(egui::DragValue::new(&mut e.lites_across).range(1..=12));
                },
            );
            row(
                ui,
                if diamond {
                    "Diamonds Vertical"
                } else {
                    "Lites Vertical"
                },
                |ui| {
                    ui.add(egui::DragValue::new(&mut e.lites_vertical).range(1..=12));
                },
            );
        });
        self.fields
            .length_row(ui, "Muntin Width", "muntin", &mut e.muntin_width);
        if custom {
            row(ui, "Dividers Across %", |ui| {
                if ui
                    .add(egui::TextEdit::singleline(&mut self.custom_across).desired_width(160.0))
                    .changed()
                {
                    spec.custom_across = parse_numbers(&self.custom_across)
                        .into_iter()
                        .map(|p| (p / 100.0).clamp(0.0, 1.0))
                        .collect();
                }
            });
            row(ui, "Dividers Up %", |ui| {
                if ui
                    .add(egui::TextEdit::singleline(&mut self.custom_up).desired_width(160.0))
                    .changed()
                {
                    spec.custom_up = parse_numbers(&self.custom_up)
                        .into_iter()
                        .map(|p| (p / 100.0).clamp(0.0, 1.0))
                        .collect();
                }
            });
            ui.weak(
                "Where the muntins sit, as percents of the width and of the height, e.g. 33, 66",
            );
        }
        if prairie {
            ui.weak("A border of small lites round one large pane.");
        }
        ui.add_enabled_ui(false, |ui| {
            dis_check(ui, "Lites in Fixed", true);
            dis_check(ui, "Lites in Movable", true);
            dis_check(ui, "Muntin in Corner", false);
            dis_check(ui, "Auto Adjust Lites for Component Size", true);
            section(ui, "Round Top Arch");
            dis_check(ui, "Concentric", false);
        });
    }

    /// The Sash tab (windows): the sash widths and the post between sashes.
    fn sash(&mut self, ui: &mut Ui) {
        self.use_default_row(ui, DynGroup::Sash);
        section(ui, "Sash");
        ui.checkbox(&mut self.draft.extras.spec.has_sash, "Has Sash")
            .on_hover_text("A window without a sash is glass straight in the frame");
        let has = self.draft.extras.spec.has_sash;
        ui.add_enabled_ui(has, |ui| {
            let mut side = self.draft.extras.sash_width.unwrap_or(1.5);
            if self
                .fields
                .length_row(ui, "Side Width", "sash_side", &mut side)
            {
                self.draft.extras.sash_width = Some(side.max(0.0));
            }
            let spec = &mut self.draft.extras.spec;
            self.fields
                .length_row(ui, "Top Width", "sash_top", &mut spec.sash_top);
            self.fields
                .length_row(ui, "Bottom Width", "sash_bottom", &mut spec.sash_bottom);
        });
        let spec = &mut self.draft.extras.spec;
        self.fields
            .length_row(ui, "Middle Width", "sash_mullion", &mut spec.mullion_width);
        ui.weak(
            "Middle width is the post between two sashes and between mulled windows. \
             Frame widths are on the Frame tab.",
        );
        spec.sash_top = spec.sash_top.max(0.0);
        spec.sash_bottom = spec.sash_bottom.max(0.0);
        spec.mullion_width = spec.mullion_width.max(0.0);
    }

    /// The Lintel tab: trim over the head, and the exterior sill of a window.
    fn lintel(&mut self, ui: &mut Ui) {
        self.use_default_row(ui, DynGroup::Lintel);
        let window = !self.is_door();
        let spec = &mut self.draft.extras.spec;
        section(ui, "Lintel");
        ui.checkbox(&mut spec.lintel.exterior, "Use Exterior Lintel");
        ui.checkbox(&mut spec.lintel.interior, "Use Interior Lintel");
        let any = spec.lintel.any();
        ui.add_enabled_ui(any, |ui| {
            row(ui, "Style", |ui| {
                egui::ComboBox::from_id_salt("lintel_style")
                    .selected_text(spec.lintel.style.name())
                    .show_ui(ui, |ui| {
                        for st in LintelStyle::ALL {
                            ui.selectable_value(&mut spec.lintel.style, st, st.name());
                        }
                    });
            });
            self.fields
                .length_row(ui, "Height", "lintel_h", &mut spec.lintel.height);
            self.fields
                .length_row(ui, "Depth", "lintel_d", &mut spec.lintel.depth);
            self.fields
                .length_row(ui, "Extend", "lintel_x", &mut spec.lintel.extend);
        });
        spec.lintel.height = spec.lintel.height.max(0.25);
        spec.lintel.depth = spec.lintel.depth.max(0.25);
        spec.lintel.extend = spec.lintel.extend.max(0.0);
        if window {
            section(ui, "Exterior Sill");
            ui.checkbox(&mut spec.sill.enabled, "Use Exterior Sill");
            ui.add_enabled_ui(spec.sill.enabled, |ui| {
                self.fields
                    .length_row(ui, "Projection", "sill_d", &mut spec.sill.depth);
                self.fields
                    .length_row(ui, "Thickness", "sill_h", &mut spec.sill.height);
                self.fields
                    .length_row(ui, "Extend", "sill_x", &mut spec.sill.extend);
            });
            spec.sill.depth = spec.sill.depth.max(0.25);
            spec.sill.height = spec.sill.height.max(0.25);
            spec.sill.extend = spec.sill.extend.max(0.0);
        }
        ui.weak("Drawn in 3D on the outside face (inside for an interior lintel).");
    }

    /// The Arch tab: the shape of the head, in 3D and in plan.
    fn arch(&mut self, ui: &mut Ui) {
        let (w, h) = (self.draft.width, self.draft.height);
        let door_style = self.draft.effective_style();
        let applies = plan_3d_arch_applies(self.draft.kind, door_style);
        let spec = &mut self.draft.extras.spec;
        section(ui, "Arch");
        row(ui, "Type", |ui| {
            egui::ComboBox::from_id_salt("arch_type")
                .selected_text(spec.arch.kind.name())
                .show_ui(ui, |ui| {
                    for t in ArchType::ALL {
                        ui.selectable_value(&mut spec.arch.kind, t, t.name());
                    }
                });
        });
        ui.add_enabled_ui(spec.arch.is_arched(), |ui| {
            self.fields
                .length_row(ui, "Height", "arch_h", &mut spec.arch.height);
        });
        spec.arch.height = spec.arch.height.max(0.0);
        if spec.arch.is_arched() {
            let rise = spec.arch.rise(w, h);
            ui.label(format!(
                "Rise {} over a {} opening{}",
                fmt_short(rise),
                fmt_short(w),
                if spec.arch.height > 0.0 {
                    ""
                } else {
                    " (automatic)"
                }
            ));
            if !applies {
                ui.weak("This style keeps a square head; the arch is only marked in plan.");
            }
        } else {
            ui.weak("Height 0 takes the type's own rise.");
        }
    }

    /// The Hardware tab (doors): handle and hinges, drawn as simple shapes.
    fn hardware(&mut self, ui: &mut Ui) {
        self.use_default_row(ui, DynGroup::Hardware);
        let hw = &mut self.draft.extras.spec.hardware;
        section(ui, "Hardware");
        ui.checkbox(&mut hw.enabled, "Show Hardware in 3D");
        ui.add_enabled_ui(hw.enabled, |ui| {
            row(ui, "Handle", |ui| {
                egui::ComboBox::from_id_salt("handle_style")
                    .selected_text(hw.handle.name())
                    .show_ui(ui, |ui| {
                        for st in HandleStyle::ALL {
                            ui.selectable_value(&mut hw.handle, st, st.name());
                        }
                    });
            });
            self.fields
                .length_row(ui, "Up from Bottom", "hw_up", &mut hw.handle_height);
            self.fields
                .length_row(ui, "In from Door Edge", "hw_in", &mut hw.in_from_edge);
            section(ui, "Hinges");
            row(ui, "Number of Hinges", |ui| {
                ui.add(egui::DragValue::new(&mut hw.hinges).range(0..=6));
            });
            self.fields
                .length_row(ui, "In from Top/Bottom", "hw_inset", &mut hw.hinge_inset);
        });
        hw.handle_height = hw.handle_height.max(0.0);
        hw.in_from_edge = hw.in_from_edge.max(0.0);
        hw.hinge_inset = hw.hinge_inset.max(0.0);
        ui.weak("Hinged, double, fixed and pocket doors. Off keeps a plain slab.");
    }

    /// The Shutters tab: exterior shutters beside or over the opening.
    fn shutters(&mut self, ui: &mut Ui) {
        let exterior = self
            .wall
            .as_ref()
            .is_none_or(|w| w.kind == WallKind::Exterior);
        let sh = &mut self.draft.extras.spec.shutters;
        section(ui, "Shutters");
        row(ui, "Type", |ui| {
            egui::ComboBox::from_id_salt("shutter_style")
                .selected_text(sh.style.name())
                .show_ui(ui, |ui| {
                    for st in ShutterStyle::ALL {
                        ui.selectable_value(&mut sh.style, st, st.name());
                    }
                });
        });
        ui.add_enabled_ui(sh.present(), |ui| {
            row(ui, "Sides", |ui| {
                egui::ComboBox::from_id_salt("shutter_sides")
                    .selected_text(sh.sides.name())
                    .show_ui(ui, |ui| {
                        for st in ShutterSides::ALL {
                            ui.selectable_value(&mut sh.sides, st, st.name());
                        }
                    });
            });
            self.fields
                .length_row(ui, "Width (0 = half)", "shutter_w", &mut sh.width);
            row(ui, "Color", |ui| {
                ui.color_edit_button_srgb(&mut sh.color);
            });
            ui.checkbox(&mut sh.closed, "Show Closed");
            ui.checkbox(&mut sh.outside_casing, "Outside Casing");
            if sh.style == ShutterStyle::Louver {
                self.fields
                    .length_row(ui, "Louver Size", "shutter_louver", &mut sh.louver_size);
            }
        });
        sh.width = sh.width.max(0.0);
        sh.louver_size = sh.louver_size.max(0.25);
        if !exterior {
            ui.weak("Shutters are drawn on exterior walls only.");
        } else {
            ui.weak("On the outside face, in 3D, in elevations and as small rectangles in plan.");
        }
    }

    /// Default Settings only: the standard widths a jamb handle can snap to.
    fn standard_widths(&mut self, ui: &mut Ui) {
        let kind = self.draft.kind;
        section(ui, "Standard Widths");
        let mut widths = self.extras.widths.clone().unwrap_or_default();
        let start = widths.clone();
        ui.checkbox(&mut widths.snap, "Snap to standard widths")
            .on_hover_text(
                "Dragging a jamb handle lands on the nearest manufacturer width; Alt skips it",
            );
        let styles = OpeningStyle::for_kind_list(kind);
        let before = self.widths_style;
        row(ui, "Style", |ui| {
            egui::ComboBox::from_id_salt("widths_style")
                .selected_text(self.widths_style.name(kind))
                .show_ui(ui, |ui| {
                    for st in styles {
                        ui.selectable_value(&mut self.widths_style, *st, st.name(kind));
                    }
                });
        });
        if before != self.widths_style {
            self.widths_text = widths_text(&widths, kind, self.widths_style);
        }
        row(ui, "Widths (in)", |ui| {
            if ui
                .add(egui::TextEdit::singleline(&mut self.widths_text).desired_width(220.0))
                .changed()
            {
                let list = parse_numbers(&self.widths_text);
                let style = self.widths_style;
                match widths.lists.iter_mut().find(|l| l.style == style) {
                    Some(l) => l.widths = list,
                    None => widths.lists.push(plan_core::openings::StyleWidths {
                        style,
                        widths: list,
                    }),
                }
            }
        });
        let shown: Vec<String> = widths
            .for_style(kind, self.widths_style)
            .iter()
            .map(|w| fmt_short(*w))
            .collect();
        ui.weak(shown.join("   "));
        if widths != start {
            self.extras.widths = Some(widths);
        }
    }

    /// "Recessed into Wall" (DW-57): the leaf stands this far in from the
    /// exterior face instead of on the wall centerline, in plan.
    fn recessed_into_wall(&mut self, ui: &mut Ui) {
        section(ui, "Recessed into Wall");
        let thick = self.wall.as_ref().map_or(4.5, |w| w.thickness);
        let spec = &mut self.draft.extras.spec;
        let mut on = spec.recess_depth.is_some() || spec.recess_to != RecessTo::Depth;
        if ui
            .checkbox(&mut on, "Recessed To Layer")
            .on_hover_text("Stands the door leaf and its swing in from the exterior face")
            .changed()
        {
            spec.recess_depth = on.then_some(thick * 0.5);
            if !on {
                spec.recess_to = RecessTo::Depth;
            }
        }
        if on {
            row(ui, "Recessed To", |ui| {
                ui.radio_value(&mut spec.recess_to, RecessTo::Depth, "Depth");
                ui.radio_value(&mut spec.recess_to, RecessTo::MainLayer, "Main Layer")
                    .on_hover_text("The exterior side of the wall's main (structural) layer");
                ui.radio_value(
                    &mut spec.recess_to,
                    RecessTo::SheathingLayer,
                    "Sheathing Layer",
                );
            });
        }
        if let Some(d) = spec.recess_depth {
            let mut depth = d;
            let typed = spec.recess_to == RecessTo::Depth;
            let changed = ui
                .add_enabled_ui(typed, |ui| {
                    self.fields
                        .length_row(ui, "Depth from Exterior Face", "recess_d", &mut depth)
                })
                .inner;
            if changed {
                spec.recess_depth = Some(depth.clamp(0.0, thick));
            }
        }
    }

    /// The Sill/Threshold tab (DW-81, DW-85): the threshold line of an
    /// exterior door, the exterior sill of a window.
    fn sill_threshold(&mut self, ui: &mut Ui) {
        if self.is_door() {
            let spec = &mut self.draft.extras.spec;
            section(ui, "Threshold");
            ui.checkbox(&mut spec.threshold, "Show Threshold in Plan")
                .on_hover_text("A thin line across the opening of a door in an exterior wall");
            ui.weak("Hinged, double, sliding and fixed doors in exterior walls.");
        } else {
            let spec = &mut self.draft.extras.spec;
            section(ui, "Exterior Sill");
            ui.checkbox(&mut spec.sill.enabled, "Use Exterior Sill")
                .on_hover_text("Also under Lintel; the plan draws it past the exterior face");
            ui.add_enabled_ui(spec.sill.enabled, |ui| {
                self.fields
                    .length_row(ui, "Projection", "sill_d", &mut spec.sill.depth);
                self.fields
                    .length_row(ui, "Extend", "sill_x", &mut spec.sill.extend);
            });
            spec.sill.depth = spec.sill.depth.max(0.25);
            spec.sill.extend = spec.sill.extend.max(0.0);
        }
    }

    /// The Opening Indicators tab (DW-83).
    fn indicators(&mut self, ui: &mut Ui) {
        let swings = matches!(
            self.draft.effective_style(),
            OpeningStyle::Hinged
                | OpeningStyle::DoubleDoor
                | OpeningStyle::Shower
                | OpeningStyle::Casement
        );
        let ind = &mut self.draft.extras.spec.indicators;
        section(ui, "Plan Display");
        ui.checkbox(&mut ind.show_in_plan, "Show Opening Indicators in Plan")
            .on_hover_text(
                "An X over a fixed unit, an arrow for the way an awning or hopper opens",
            );
        ui.add_enabled_ui(swings, |ui| {
            ui.checkbox(&mut ind.swing_arrows, "Swing Direction Arrows")
                .on_hover_text("An arrowhead at the free end of the swing arc");
        });
        if !swings {
            ui.weak("This style has no swing arc.");
        }
    }

    /// The Schedule tab (L-29, DW-61): the mark and the supplier data the
    /// door and window schedules list.
    fn schedule_tab(&mut self, ui: &mut Ui) {
        section(ui, "Schedule");
        ui.checkbox(
            &mut self.draft.extras.spec.schedule.include,
            "Include in Schedule",
        )
        .on_hover_text("A cleared box leaves it out of the schedule and its numbering");
        let mut mark = self.draft.schedule_number.clone().unwrap_or_default();
        row(ui, "Mark", |ui| {
            ui.add(egui::TextEdit::singleline(&mut mark).desired_width(120.0))
                .on_hover_text("Blank numbers it automatically (D01, W03); Renumber Schedule sets them in draw order");
        });
        self.draft.schedule_number = (!mark.trim().is_empty()).then(|| mark.trim().to_string());
        let sch = &mut self.draft.extras.spec.schedule;
        section(ui, "Product");
        for (label, text) in [
            ("Manufacturer", &mut sch.manufacturer),
            ("Model", &mut sch.model),
            ("Supplier", &mut sch.supplier),
        ] {
            row(ui, label, |ui| {
                ui.add(egui::TextEdit::singleline(text).desired_width(260.0));
            });
        }
        row(ui, "Comment", |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut sch.comment)
                    .desired_rows(3)
                    .desired_width(260.0),
            );
        });
        ui.weak("Add the Manufacturer, Model, Supplier and Comment columns in the schedule's Specification.");
    }

    fn label(&mut self, ui: &mut Ui) {
        let door = self.is_door();
        section(ui, "Display Options");
        let mut suppress = self.label.mode == LabelMode::Suppress;
        if ui
            .checkbox(&mut suppress, "Suppress Label in All Views")
            .changed()
        {
            self.label.mode = if suppress {
                LabelMode::Suppress
            } else {
                LabelMode::Automatic
            };
        }
        ui.checkbox(&mut self.label.display_in_plan, "Display in Plan View");
        section(ui, "Label Content");
        let mut specify = self.draft.label_override.is_some();
        if ui
            .radio_value(&mut specify, false, "Automatic Label")
            .changed()
            && !specify
        {
            self.draft.label_override = None;
        }
        if ui
            .radio_value(&mut specify, true, "Specify Label")
            .changed()
            && self.draft.label_override.is_none()
        {
            self.draft.label_override = Some(String::new());
        }
        if let Some(text) = &mut self.draft.label_override {
            ui.add(egui::TextEdit::singleline(text).desired_width(260.0));
            ui.weak("Macros: %automatic_label% %schedule_number% %width% %height% %type%");
        }
        ui.add_enabled_ui(!specify, |ui| {
            row(ui, "Size Format", |ui| {
                ui.radio_value(&mut self.label.size_format, SizeFormat::HeightWidth, "Height/Width");
                ui.radio_value(&mut self.label.size_format, SizeFormat::WidthHeight, "Width/Height");
                ui.radio_value(&mut self.label.size_format, SizeFormat::WidthOnly, "Width Only");
            });
            row(ui, "Size Style", |ui| {
                ui.radio_value(&mut self.label.size_style, SizeStyle::Shorthand, "3068");
                ui.radio_value(
                    &mut self.label.size_style,
                    SizeStyle::Architectural,
                    "2'-6\" x 6'-8\"",
                );
            });
            ui.checkbox(
                &mut self.label.include_schedule_number,
                "Include Schedule Number",
            )
            .on_hover_text("Shows the schedule mark (D01) instead of the size once a schedule numbers this opening");
            ui.checkbox(&mut self.label.include_type, "Include Type");
        });
        row(ui, "Placement", |ui| {
            ui.radio_value(
                &mut self.label.placement,
                LabelPlacement::Center,
                "Over opening",
            );
            ui.radio_value(
                &mut self.label.placement,
                LabelPlacement::Interior,
                "Interior",
            );
            ui.radio_value(
                &mut self.label.placement,
                LabelPlacement::Exterior,
                "Exterior",
            );
        });
        // What the plan will show now, and with a schedule.
        let mut shown = self.draft.clone();
        shown.extras.label = Some(self.label.clone());
        let defaults = OpeningLabelDefaults::default();
        let size = shown.plan_label(&defaults, None);
        let mark = shown.plan_label(&defaults, Some(if door { "D01" } else { "W01" }));
        ui.add_space(4.0);
        ui.label(format!(
            "Label: {}    with a schedule: {}",
            size.as_deref().unwrap_or("(none)"),
            mark.as_deref().unwrap_or("(none)")
        ));
        ui.add_enabled_ui(false, |ui| {
            section(ui, "Label Layer");
            dis_radio(
                ui,
                if door {
                    "Use System Layer (Doors, Labels)"
                } else {
                    "Use System Layer (Windows, Labels)"
                },
                true,
            );
        });
    }
}

impl SpecPages for OpeningForm {
    fn tabs(&self) -> &'static [Tab] {
        if self.is_door() {
            DOOR_TABS
        } else {
            WINDOW_TABS
        }
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        if self.draft.width <= 0.0 || self.draft.height <= 0.0 {
            return Some("Width and height must be greater than zero".into());
        }
        if let Some(w) = &self.wall {
            if w.length < self.draft.width + 2.0 * OPENING_MARGIN {
                return Some("Opening is wider than the wall".into());
            }
            if self.others.iter().any(|o| overlap(&self.draft, o)) {
                return Some("Opening overlaps another opening".into());
            }
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        let name = self.tabs()[tab].name;
        match name {
            "General" => self.general(ui),
            "Options" if self.is_door() => self.door_options(ui),
            "Options" => self.window_options(ui),
            "Casing" => self.casing(ui),
            "Jamb" | "Frame" => self.jamb_or_frame(ui),
            "Lites" => self.lites(ui),
            "Sash" => self.sash(ui),
            "Lintel" => self.lintel(ui),
            "Arch" => self.arch(ui),
            "Hardware" => self.hardware(ui),
            "Shutters" => self.shutters(ui),
            "Label" => self.label(ui),
            "Sill/Threshold" => self.sill_threshold(ui),
            "Opening Indicators" => self.indicators(ui),
            "Schedule" => self.schedule_tab(ui),
            "Rough Opening" => self.rough_opening(ui),
            "Framing" => self.framing(ui),
            "Energy Values" => self.energy_values(ui),
            "Layer" => self.layer_tab(ui),
            "Materials" => self.materials_tab(ui),
            "Object Information" => self.object_information(ui),
            "Shape" => self.shape_tab(ui),
            "Treatments" => self.treatments_tab(ui),
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        let split = rect.min.y + rect.height() * 0.58;
        let top = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, split));
        let bottom = Rect::from_min_max(Pos2::new(rect.min.x, split + 4.0), rect.max);
        let o = &self.draft;
        let (label, e) = if self.is_door() {
            ("Door elevation", &self.extras)
        } else {
            ("Window elevation", &self.extras)
        };
        pv_text(
            p,
            top.min + Vec2::new(0.0, 4.0),
            Align2::LEFT_CENTER,
            label,
            11.0,
        );
        let elev = Rect::from_min_max(top.min + Vec2::new(0.0, 10.0), top.max);
        if self.is_door() {
            door_elevation(p, elev, o);
        } else {
            window_elevation(p, elev, o, e);
        }
        p.hline(rect.x_range(), split + 1.0, Stroke::new(0.8_f32, PV_FAINT));
        pv_text(
            p,
            bottom.min + Vec2::new(0.0, 6.0),
            Align2::LEFT_CENTER,
            "Plan view",
            11.0,
        );
        // A short stretch of the host wall, centered on the opening.
        let thick = self.wall.as_ref().map_or(4.5, |w| w.thickness);
        let len = o.width + 36.0;
        let mut sample = o.clone();
        sample.center_offset = len * 0.5;
        sample.extras.swing_angle_deg = Some(e.swing_angle);
        sample.extras.show_open_in_plan = e.show_open_2d;
        plan_preview(
            p,
            Rect::from_min_max(bottom.min + Vec2::new(0.0, 8.0), bottom.max),
            len,
            thick,
            &sample,
        );
    }
}

// ----- plan sketch -----

/// Plan view of a short stretch of wall (start at the left, its left side up)
/// with the opening drawn from its plan symbol, so the preview shows what the
/// plan will: swing, sliding panels, pocket, projecting unit.
fn plan_preview(p: &Painter, area: Rect, len: f64, thick: f64, o: &Opening) {
    use plan_core::geometry::Point;
    let wall = Wall::new(
        Point::new(0.0, 0.0),
        Point::new(len, 0.0),
        thick,
        96.0,
        WallKind::Exterior,
    );
    let sym = plan_symbol(&wall, o, 1.0);
    let (mut y0, mut y1) = (-thick * 0.5, thick * 0.5);
    for q in sym.parts.iter().flat_map(|part| part.points.iter()) {
        y0 = y0.min(q.y);
        y1 = y1.max(q.y);
    }
    let s_x = (area.width() as f64 - 12.0) / len.max(1.0);
    let s_y = (area.height() as f64 - 18.0).max(10.0) / (y1 - y0).max(1.0);
    let s = s_x.min(s_y).clamp(0.02, 6.0) as f32;
    let (xc, yc) = (area.center().x, area.center().y - 4.0);
    let ymid = ((y0 + y1) * 0.5) as f32;
    let to = |q: Point| {
        Pos2::new(
            xc + (q.x as f32 - len as f32 * 0.5) * s,
            yc - (q.y as f32 - ymid) * s,
        )
    };
    let ink = Stroke::new(1.0_f32, PV_INK);
    let th = (thick as f32 * s).max(5.0);
    // The wall body: centerline at t = 0.
    let c0 = to(Point::new(0.0, 0.0)).y;
    let body = Rect::from_min_max(
        Pos2::new(to(Point::new(0.0, 0.0)).x, c0 - th * 0.5),
        Pos2::new(to(Point::new(len, 0.0)).x, c0 + th * 0.5),
    );
    p.rect_filled(body, 0.0, PV_WALL);
    p.rect_stroke(body, 0.0, ink, StrokeKind::Inside);
    // The cut across the opening.
    let (a, b) = (
        to(Point::new(sym.span.0, 0.0)).x,
        to(Point::new(sym.span.1, 0.0)).x,
    );
    let (lo, hi) = (sym.cut.0 as f32, sym.cut.1 as f32);
    let (top, bottom) = (
        c0 - hi * s
            - if hi >= thick as f32 * 0.5 - 1e-4 {
                1.0
            } else {
                0.0
            },
        c0 - lo * s
            + if lo <= -thick as f32 * 0.5 + 1e-4 {
                1.0
            } else {
                0.0
            },
    );
    p.rect_filled(
        Rect::from_min_max(Pos2::new(a, top), Pos2::new(b, bottom)),
        0.0,
        PV_BG,
    );
    for part in &sym.parts {
        let mut pts: Vec<Pos2> = part.points.iter().map(|q| to(*q)).collect();
        if pts.len() < 2 {
            continue;
        }
        let stroke = match part.kind {
            PartKind::Swing => Stroke::new(0.8_f32, PV_FAINT),
            PartKind::Glass | PartKind::Frame => Stroke::new(0.8_f32, PV_INK),
            PartKind::Hidden | PartKind::Track => Stroke::new(0.8_f32, PV_FAINT),
            _ => ink,
        };
        match part.kind {
            PartKind::Hidden | PartKind::Track => {
                if part.closed {
                    pts.push(pts[0]);
                }
                p.extend(Shape::dashed_line(&pts, stroke, 4.0, 3.0));
            }
            _ if part.closed => {
                p.add(Shape::closed_line(pts, stroke));
            }
            _ => {
                p.add(Shape::line(pts, stroke));
            }
        }
    }
    pv_text(
        p,
        Pos2::new(area.center().x, area.max.y - 6.0),
        Align2::CENTER_CENTER,
        fmt_short(o.width),
        11.0,
    );
}

// ----- elevation sketches -----

const PV_DOOR: Color32 = Color32::from_rgb(0xD9, 0xC3, 0x9A);
const PV_TRIM: Color32 = Color32::from_rgb(0xF6, 0xF4, 0xEE);

/// Maps unit fractions (0..1 across, 0..1 down) of `r` to a screen rectangle.
fn frac(r: Rect, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    Rect::from_min_max(
        Pos2::new(r.min.x + r.width() * x0, r.min.y + r.height() * y0),
        Pos2::new(r.min.x + r.width() * x1, r.min.y + r.height() * y1),
    )
}

/// Screen point at unit fractions (across, down) of `r`.
fn pt(r: Rect, fx: f32, fy: f32) -> Pos2 {
    Pos2::new(r.min.x + r.width() * fx, r.min.y + r.height() * fy)
}

fn outlined(p: &Painter, r: Rect, fill: Color32) {
    p.rect_filled(r, 0.0, fill);
    p.rect_stroke(r, 0.0, Stroke::new(1.0_f32, PV_INK), StrokeKind::Inside);
}

/// Scaled rect for an `w` x `h` inch object sitting on a floor line near the
/// bottom of `area`, leaving room for dimension text.
fn fit_on_floor(area: Rect, w: f64, h: f64, lift: f64) -> (Rect, f32) {
    let (w, h) = (w.max(1.0) as f32, h.max(1.0) as f32);
    let lift = lift.max(0.0) as f32;
    let s = ((area.width() - 64.0) / w)
        .min((area.height() - 34.0) / (h + lift))
        .max(0.05);
    let size = Vec2::new(w * s, h * s);
    let floor_y = area.max.y - 18.0;
    let min = Pos2::new(
        area.center().x - size.x * 0.5 - 12.0,
        floor_y - lift * s - size.y,
    );
    (Rect::from_min_size(min, size), floor_y)
}

fn door_elevation(p: &Painter, area: Rect, o: &Opening) {
    let style = o.style;
    let (r, floor_y) = fit_on_floor(area, o.width, o.height, o.sill_height);
    p.hline(area.x_range(), floor_y, Stroke::new(1.0_f32, PV_INK));
    // Casing/jamb around the leaf.
    outlined(p, r.expand(3.0), PV_TRIM);
    let hinge_left = !o.hinge_at_end;
    match style {
        // Sliding: two overlapping leaves with lites and an arrow.
        OpeningStyle::Sliding => {
            outlined(p, frac(r, 0.0, 0.0, 0.55, 1.0), PV_DOOR);
            outlined(p, frac(r, 0.45, 0.0, 1.0, 1.0), PV_DOOR);
            outlined(p, frac(r, 0.08, 0.08, 0.47, 0.8), PV_GLASS);
            outlined(p, frac(r, 0.53, 0.08, 0.92, 0.8), PV_GLASS);
        }
        // Pocket: the leaf half-way out of the wall pocket.
        OpeningStyle::Pocket => {
            p.rect_filled(frac(r, 0.0, 0.0, 0.45, 1.0), 0.0, PV_BG);
            p.rect_stroke(
                frac(r, 0.0, 0.0, 0.45, 1.0),
                0.0,
                Stroke::new(1.0_f32, PV_FAINT),
                StrokeKind::Inside,
            );
            outlined(p, frac(r, 0.45, 0.0, 1.0, 1.0), PV_DOOR);
            p.circle_filled(pt(r, 0.55, 0.5), 2.5, PV_INK);
        }
        // Bifold: four narrow folded leaves.
        OpeningStyle::Bifold => {
            for i in 0..4 {
                let x = i as f32 * 0.25;
                outlined(p, frac(r, x, 0.0, x + 0.25, 1.0), PV_DOOR);
            }
        }
        // Garage: horizontal sections, lites in the top one.
        OpeningStyle::Garage => {
            for i in 0..4 {
                let y = i as f32 * 0.25;
                outlined(p, frac(r, 0.0, y, 1.0, y + 0.25), PV_DOOR);
            }
            for i in 0..4 {
                let x = 0.06 + i as f32 * 0.22;
                outlined(p, frac(r, x, 0.05, x + 0.18, 0.2), PV_GLASS);
            }
        }
        // Doorway: just the cased opening.
        OpeningStyle::Doorway => {
            p.rect_filled(r, 0.0, PV_BG);
        }
        // Barn: plank leaf with a Z brace, hanging on a rail.
        OpeningStyle::Barn => {
            outlined(p, r, PV_DOOR);
            for i in 1..6 {
                let x = r.min.x + r.width() * i as f32 / 6.0;
                p.line_segment(
                    [Pos2::new(x, r.min.y), Pos2::new(x, r.max.y)],
                    Stroke::new(0.6_f32, PV_FAINT),
                );
            }
            let brace = Stroke::new(1.5_f32, PV_INK);
            p.line_segment([pt(r, 0.1, 0.1), pt(r, 0.9, 0.9)], brace);
            p.line_segment(
                [
                    Pos2::new(r.min.x - 6.0, r.min.y - 5.0),
                    Pos2::new(r.max.x + 6.0, r.min.y - 5.0),
                ],
                Stroke::new(2.0_f32, PV_INK),
            );
        }
        // Double: two leaves meeting in the middle, each with a lite and a knob.
        OpeningStyle::DoubleDoor => {
            for (x0, x1, kx) in [(0.0, 0.5, 0.42), (0.5, 1.0, 0.58)] {
                outlined(p, frac(r, x0, 0.0, x1, 1.0), PV_DOOR);
                outlined(p, frac(r, x0 + 0.07, 0.07, x1 - 0.07, 0.42), PV_GLASS);
                outlined(
                    p,
                    frac(r, x0 + 0.07, 0.5, x1 - 0.07, 0.93),
                    PV_DOOR.gamma_multiply(0.9),
                );
                p.circle_filled(pt(r, kx, 0.5), 3.0, PV_INK);
            }
        }
        // Fixed and shower doors are glass in a frame.
        OpeningStyle::Fixed | OpeningStyle::Shower => {
            outlined(p, r, PV_GLASS);
            p.line_segment(
                [pt(r, 0.0, 0.0), pt(r, 1.0, 1.0)],
                Stroke::new(0.6_f32, PV_FAINT),
            );
        }
        // Hinged: two raised panels, a lite and a handle opposite the hinge.
        _ => {
            outlined(p, r, PV_DOOR);
            outlined(p, frac(r, 0.14, 0.07, 0.86, 0.42), PV_GLASS);
            outlined(
                p,
                frac(r, 0.14, 0.5, 0.86, 0.93),
                PV_DOOR.gamma_multiply(0.9),
            );
            let hx = if hinge_left { 0.0 } else { 1.0 };
            for y in [0.12, 0.5, 0.88] {
                p.rect_filled(
                    Rect::from_center_size(pt(r, hx, y), Vec2::new(4.0, 7.0)),
                    0.0,
                    PV_INK,
                );
            }
            let kx = if hinge_left { 0.9 } else { 0.1 };
            p.circle_filled(pt(r, kx, 0.5), 3.0, PV_INK);
        }
    }
    dimension_text(p, r, floor_y, o);
}

fn dimension_text(p: &Painter, r: Rect, floor_y: f32, o: &Opening) {
    pv_text(
        p,
        Pos2::new(r.center().x, floor_y + 10.0),
        Align2::CENTER_CENTER,
        fmt_short(o.width),
        11.0,
    );
    pv_text(
        p,
        Pos2::new(r.max.x + 8.0, r.center().y),
        Align2::LEFT_CENTER,
        fmt_short(o.height),
        11.0,
    );
}

/// A window with a Shape tab outline: the outline filled with glass and the
/// lite dividers clipped to it.
fn shaped_window_elevation(p: &Painter, r: Rect, floor_y: f32, o: &Opening, e: &OpeningExtras) {
    let s = r.width() / o.width.max(1.0) as f32;
    let at = |q: (f64, f64)| Pos2::new(r.min.x + q.0 as f32 * s, r.max.y - q.1 as f32 * s);
    let shape = &o.extras.spec.shape;
    let pts: Vec<Pos2> = shape
        .outline(o.width, o.height)
        .into_iter()
        .map(at)
        .collect();
    p.add(Shape::convex_polygon(
        pts,
        PV_GLASS,
        Stroke::new(2.0_f32, PV_INK),
    ));
    let muntin = Stroke::new((e.muntin_width as f32 * s).clamp(0.8, 3.0), PV_INK);
    for (a, b) in shape.lite_lines(
        &o.extras.spec,
        (e.lites_across, e.lites_vertical),
        o.width,
        o.height,
        1.0,
    ) {
        p.line_segment([at(a), at(b)], muntin);
    }
    p.rect_filled(
        Rect::from_min_max(
            Pos2::new(r.min.x - 6.0, r.max.y + 3.0),
            Pos2::new(r.max.x + 6.0, r.max.y + 6.0),
        ),
        0.0,
        PV_WALL,
    );
    dimension_text(p, r, floor_y, o);
}

fn window_elevation(p: &Painter, area: Rect, o: &Opening, e: &OpeningExtras) {
    let (r, floor_y) = fit_on_floor(area, o.width, o.height, o.sill_height);
    p.hline(area.x_range(), floor_y, Stroke::new(1.0_f32, PV_INK));
    if o.is_shaped() {
        shaped_window_elevation(p, r, floor_y, o, e);
        return;
    }
    outlined(p, r.expand(3.0), PV_TRIM);
    let glass = r.shrink(3.0);
    outlined(p, glass, PV_GLASS);
    // Lite grid.
    let s = r.width() / o.width.max(1.0) as f32;
    let muntin = Stroke::new((e.muntin_width as f32 * s).clamp(0.8, 3.0), PV_INK);
    for i in 1..e.lites_across {
        let x = glass.min.x + glass.width() * i as f32 / e.lites_across as f32;
        p.line_segment(
            [Pos2::new(x, glass.min.y), Pos2::new(x, glass.max.y)],
            muntin,
        );
    }
    for j in 1..e.lites_vertical {
        let y = glass.min.y + glass.height() * j as f32 / e.lites_vertical as f32;
        p.line_segment(
            [Pos2::new(glass.min.x, y), Pos2::new(glass.max.x, y)],
            muntin,
        );
    }
    // Operation symbol.
    let sym = Stroke::new(0.8_f32, PV_ACCENT);
    let c = glass.center();
    match o.style {
        OpeningStyle::Casement => {
            // Casement: lines converge on the hinge (left) side.
            p.line_segment(
                [
                    Pos2::new(glass.max.x, glass.min.y),
                    Pos2::new(glass.min.x, c.y),
                ],
                sym,
            );
            p.line_segment(
                [
                    Pos2::new(glass.max.x, glass.max.y),
                    Pos2::new(glass.min.x, c.y),
                ],
                sym,
            );
        }
        OpeningStyle::Window => {
            p.line_segment(
                [Pos2::new(glass.min.x, c.y), Pos2::new(glass.max.x, c.y)],
                Stroke::new(2.5_f32, PV_INK),
            );
        }
        OpeningStyle::SlidingWindow | OpeningStyle::Sliding => {
            p.line_segment(
                [Pos2::new(c.x, glass.min.y), Pos2::new(c.x, glass.max.y)],
                Stroke::new(2.5_f32, PV_INK),
            );
            p.line_segment([Pos2::new(c.x - 8.0, c.y), Pos2::new(c.x + 8.0, c.y)], sym);
        }
        OpeningStyle::Awning => {
            p.line_segment(
                [
                    Pos2::new(glass.min.x, glass.max.y),
                    Pos2::new(c.x, glass.min.y),
                ],
                sym,
            );
            p.line_segment(
                [
                    Pos2::new(glass.max.x, glass.max.y),
                    Pos2::new(c.x, glass.min.y),
                ],
                sym,
            );
        }
        OpeningStyle::Hopper => {
            p.line_segment(
                [
                    Pos2::new(glass.min.x, glass.min.y),
                    Pos2::new(c.x, glass.max.y),
                ],
                sym,
            );
            p.line_segment(
                [
                    Pos2::new(glass.max.x, glass.min.y),
                    Pos2::new(c.x, glass.max.y),
                ],
                sym,
            );
        }
        _ => {}
    }
    // Sill.
    p.rect_filled(
        Rect::from_min_max(
            Pos2::new(r.min.x - 6.0, r.max.y + 3.0),
            Pos2::new(r.max.x + 6.0, r.max.y + 6.0),
        ),
        0.0,
        PV_WALL,
    );
    dimension_text(p, r, floor_y, o);
    if o.sill_height > 0.0 {
        pv_text(
            p,
            Pos2::new(r.min.x - 8.0, floor_y - o.sill_height as f32 * s * 0.5),
            Align2::RIGHT_CENTER,
            fmt_short(o.sill_height),
            10.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::geometry::Point;

    fn host() -> Wall {
        Wall::new(
            Point::ZERO,
            Point::new(240.0, 0.0),
            6.0,
            109.0,
            WallKind::Exterior,
        )
    }

    #[test]
    fn door_values_persist_with_the_opening() {
        let door = Opening::default_door(5, 1, 100.0);
        let mut d = OpeningDialog::for_opening(door, &host(), Vec::new(), OpeningExtras::default());
        {
            let e = &mut d.form.extras;
            e.style_name = "Door P09".into();
            e.thickness = 1.75;
            e.swing_angle = 110.0;
            e.jamb_side = 1.0;
            e.show_open_2d = false;
        }
        d.sync_stored();
        let saved = d.draft().clone();
        let x = &saved.extras;
        assert_eq!(x.style_name.as_deref(), Some("Door P09"));
        assert_eq!(x.thickness, Some(1.75));
        assert_eq!(x.swing_angle_deg, Some(110.0));
        assert_eq!(x.jamb_width, Some(1.0));
        assert!(!x.show_open_in_plan);
        assert_eq!(x.frame_width, None);

        let back: Opening = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
        assert_eq!(back.extras, saved.extras);
        // A new session (default extras) shows what the file holds.
        let d2 = OpeningDialog::for_opening(back, &host(), Vec::new(), OpeningExtras::default());
        let e = d2.extras();
        assert_eq!(e.style_name, "Door P09");
        assert_eq!(
            (e.thickness, e.swing_angle, e.jamb_side),
            (1.75, 110.0, 1.0)
        );
        assert!(!e.show_open_2d);
    }

    #[test]
    fn window_values_persist_and_the_sash_width_is_kept() {
        let mut win = Opening::default_window(6, 1, 100.0);
        win.extras.sash_width = Some(1.25);
        let mut d = OpeningDialog::for_opening(win, &host(), Vec::new(), OpeningExtras::default());
        d.form.extras.window_type = type_index(WindowType::DoubleHung);
        d.form.extras.jamb_side = 0.5;
        d.sync_stored();
        let x = d.draft().extras.clone();
        assert_eq!(x.style_name.as_deref(), Some("Double Hung"));
        assert_eq!(x.frame_width, Some(0.5));
        assert_eq!(x.sash_width, Some(1.25));
        assert_eq!((x.thickness, x.jamb_width), (None, None));
        let d2 = OpeningDialog::for_opening(
            d.draft().clone(),
            &host(),
            Vec::new(),
            OpeningExtras::default(),
        );
        assert_eq!(d2.extras().window_type, type_index(WindowType::DoubleHung));
        assert_eq!(d2.extras().jamb_side, 0.5);
    }

    #[test]
    fn editing_a_group_releases_use_default_and_the_other_groups_keep_it() {
        use plan_core::openings::UseDefault;
        let mut win = Opening::default_window(6, 1, 100.0);
        win.extras.spec.dynamic = UseDefault::all(OpeningKind::Window);
        let mut d = OpeningDialog::for_opening(win, &host(), Vec::new(), OpeningExtras::default());
        // Showing the dialog is not an edit.
        d.sync_stored_for_test();
        let dynamic = d.draft().extras.spec.dynamic;
        assert_eq!(dynamic, UseDefault::all(OpeningKind::Window));
        // The Lintel tab's values changed: that group stops following.
        d.draft_mut().extras.spec.lintel.exterior = true;
        d.sync_stored_for_test();
        let dynamic = d.draft().extras.spec.dynamic;
        assert!(!dynamic.lintel && dynamic.casing && dynamic.sash && dynamic.window_type);
        // Every tab with a Use Default box draws.
        let ctx = egui::Context::default();
        for tab in [
            "Casing",
            "Lintel",
            "Sash",
            "Frame",
            "Treatments",
            "Framing",
            "Rough Opening",
            "Materials",
        ] {
            assert!(d.draw_tab_for_test(&ctx, tab), "{tab}");
        }
    }

    #[test]
    fn the_window_defaults_hold_the_separation_and_make_the_main_window_default() {
        let d = plan_core::PlanDefaults::chief_x18_daniel();
        let mut e = OpeningExtras::from_window_defaults(&d.window);
        assert_eq!(e.min_separation, 2.0);
        e.min_separation = 5.0;
        e.ignore_casing = true;
        e.mulled.single_hole = true;
        let w = Opening::default_window(0, 0, 0.0);
        let back = e.to_window_defaults(&w, &d.window);
        assert_eq!(back.min_separation, 5.0);
        assert!(back.ignore_casing && back.mulled.single_hole);
        // OK on the Window Defaults stores the main window default.
        let dialog = OpeningDialog::for_default(OpeningTarget::DefaultWindow, w, e);
        let mut v = OpeningVariantDefaults::default();
        dialog.apply_to_variants(&mut v);
        assert!(v
            .type_default(plan_core::openings::DefaultKey::main_window())
            .is_some());
        assert!(v.needs_follow());
    }

    #[test]
    fn the_general_panel_of_a_window_draws_for_every_type() {
        let ctx = egui::Context::default();
        for t in WindowType::ALL {
            let mut win = Opening::default_window(6, 1, 100.0);
            win.style = t.style();
            win.extras.spec.window_type = t;
            let mut d =
                OpeningDialog::for_opening(win, &host(), Vec::new(), OpeningExtras::default());
            assert!(d.draw_tab_for_test(&ctx, "General"), "{t:?}");
            // The dialog shows the type the window holds.
            assert_eq!(d.extras().window_type, type_index(t), "{t:?}");
        }
    }

    #[test]
    fn an_untouched_label_stays_on_the_defaults_and_a_changed_one_is_stored() {
        let defaults = OpeningLabelDefaults::default();
        let door = Opening::default_door(5, 1, 100.0);
        let mut d = OpeningDialog::for_opening(door, &host(), Vec::new(), OpeningExtras::default())
            .with_label_defaults(&defaults);
        d.sync_stored();
        assert_eq!(d.draft().extras.label, None, "nothing changed");
        // Change the format on the Label tab: now it is the opening's own.
        d.form.label.size_style = SizeStyle::Architectural;
        d.form.label.placement = LabelPlacement::Exterior;
        d.form.draft.label_override = None;
        d.sync_stored();
        let own = d.draft().extras.label.clone().expect("stored");
        assert_eq!(own.size_style, SizeStyle::Architectural);
        let saved = d.draft().clone();
        assert_eq!(
            saved.plan_label(&defaults, None).as_deref(),
            Some("3'-0\" x 6'-8\"")
        );
        // Reopened (with other defaults) it still shows its own settings.
        let mut other = OpeningLabelDefaults::default();
        other.door.size_format = SizeFormat::WidthOnly;
        let d2 = OpeningDialog::for_opening(saved, &host(), Vec::new(), OpeningExtras::default())
            .with_label_defaults(&other);
        assert_eq!(d2.label_settings(), &own);
        // A fresh opening starts from the defaults it is given.
        let d3 = OpeningDialog::for_opening(
            Opening::default_door(6, 1, 100.0),
            &host(),
            Vec::new(),
            OpeningExtras::default(),
        )
        .with_label_defaults(&other);
        assert_eq!(d3.label_settings().size_format, SizeFormat::WidthOnly);
    }

    #[test]
    fn suppress_and_specify_label_reach_the_plan_label() {
        let defaults = OpeningLabelDefaults::default();
        let door = Opening::default_door(5, 1, 100.0);
        let mut d = OpeningDialog::for_opening(door, &host(), Vec::new(), OpeningExtras::default())
            .with_label_defaults(&defaults);
        d.form.label.mode = LabelMode::Suppress;
        d.sync_stored();
        assert_eq!(d.draft().plan_label(&defaults, None), None);
        d.form.label.mode = LabelMode::Automatic;
        d.form.draft.label_override = Some("A%width%".into());
        d.sync_stored();
        assert_eq!(
            d.draft().plan_label(&defaults, None).as_deref(),
            Some("A3'-0\"")
        );
        // The Default Settings dialog hands back the label settings it edited.
        let mut dd = OpeningDialog::for_default(
            OpeningTarget::DefaultWindow,
            Opening::default_window(0, 0, 0.0),
            OpeningExtras::default(),
        )
        .with_label_defaults(&defaults);
        dd.form.label.include_type = true;
        assert!(dd.label_settings().include_type);
    }

    #[test]
    fn the_style_combo_edits_the_openings_style_and_size_defaults_apply() {
        let mut d = OpeningDialog::for_opening(
            Opening::default_door(5, 1, 100.0),
            &host(),
            Vec::new(),
            OpeningExtras::default(),
        );
        d.draft_mut().style = OpeningStyle::Barn;
        d.sync_stored();
        assert_eq!(d.draft().style, OpeningStyle::Barn);
        let back: Opening =
            serde_json::from_str(&serde_json::to_string(d.draft()).unwrap()).unwrap();
        assert_eq!(back.style, OpeningStyle::Barn);
        // Every style of both lists has a plan symbol the preview can draw.
        let wall = host();
        for s in OpeningStyle::DOORS.into_iter().chain(OpeningStyle::WINDOWS) {
            let kind = if s.is_door_style() {
                OpeningKind::Door
            } else {
                OpeningKind::Window
            };
            let mut o = Opening::new(1, 100.0, kind, 48.0, 80.0, 0.0);
            o.style = s;
            assert!(!plan_symbol(&wall, &o, 1.0).parts.is_empty(), "{s:?}");
        }
    }

    #[test]
    fn defaults_dialogs_ignore_the_template_extras() {
        let d = plan_core::PlanDefaults::chief_x18_daniel();
        let extras = OpeningExtras::from_door_defaults(&d.interior_door, false);
        let template = Opening::default_door(0, 0, 0.0);
        let dialog =
            OpeningDialog::for_default(OpeningTarget::DefaultDoor, template, extras.clone());
        assert_eq!(dialog.extras(), &extras);
    }

    #[test]
    fn template_sizes_apply_and_overlap_is_rejected() {
        let mut p = Project::new("t");
        let w = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            4.5,
            109.125,
            WallKind::Interior,
        );
        let mut door = Opening::default_door(0, 0, 0.0);
        door.width = 60.0;
        let id = place_from_template(&mut p, 0, w, 5.0, &door).unwrap();
        let o = p.floors[0].openings.iter().find(|o| o.id == id).unwrap();
        assert!((o.width - 60.0).abs() < 1e-9);
        // Clamped to half-width plus the margin.
        assert!((o.center_offset - 32.0).abs() < 1e-9);
        assert!(place_from_template(&mut p, 0, w, 50.0, &door).is_none());
        assert!(place_from_template(&mut p, 0, w, 180.0, &door).is_some());
    }

    #[test]
    fn extras_round_trip_through_plan_defaults() {
        let d = plan_core::PlanDefaults::chief_x18_daniel();
        let door = OpeningExtras::from_door_defaults(&d.interior_door, false);
        assert_eq!(door.style_name, "Door P04");
        assert_eq!(door.casing_interior_width, 3.5);
        let o = Opening::default_door(0, 0, 0.0);
        let back = door.to_door_defaults(&o, &d.interior_door, false);
        assert_eq!(back.thickness, 1.375);
        assert_eq!(back.casing_width, 3.5);
        assert_eq!(back.jamb_width, 0.75);
        assert_eq!(back.swing_angle, 90.0);

        let ext = OpeningExtras::from_door_defaults(&d.exterior_door, true);
        assert_eq!(ext.casing_exterior_depth, 1.0);

        let win = OpeningExtras::from_window_defaults(&d.window);
        assert!(win.egress && win.tempered);
        assert_eq!(win.window_type, type_index(WindowType::SingleCasement));
        let w = Opening::default_window(0, 0, 0.0);
        let back = win.to_window_defaults(&w, &d.window);
        assert_eq!(back.window_type, "Single Casement");
        assert_eq!(back.frame_width, 0.75);
        assert_eq!(back.sash_width, 1.5);
        assert!(back.egress);
    }

    #[test]
    fn the_lites_tab_acts_on_the_opening_and_comes_back_when_reopened() {
        let win = Opening::default_window(6, 1, 100.0);
        let mut d = OpeningDialog::for_opening(win, &host(), Vec::new(), OpeningExtras::default());
        d.form.extras.lites_across = 3;
        d.form.extras.lites_vertical = 2;
        d.form.extras.muntin_width = 1.5;
        d.form.draft.extras.spec.lite_style = LiteStyle::Prairie;
        d.sync_stored();
        assert_eq!(d.draft().lites, (3, 2));
        assert_eq!(d.draft().extras.spec.muntin_width, 1.5);
        assert_eq!(d.draft().extras.spec.lite_style, LiteStyle::Prairie);
        // Reopened on the saved opening with a session that knows nothing.
        let d2 = OpeningDialog::for_opening(
            d.draft().clone(),
            &host(),
            Vec::new(),
            OpeningExtras::default(),
        );
        assert_eq!(
            (
                d2.extras().lites_across,
                d2.extras().lites_vertical,
                d2.extras().muntin_width
            ),
            (3, 2, 1.5)
        );
        assert_eq!(d2.draft().extras.spec.lite_style, LiteStyle::Prairie);
    }

    #[test]
    fn sash_lintel_arch_hardware_and_shutter_values_persist_with_the_opening() {
        let mut win = Opening::default_window(6, 1, 100.0);
        win.extras.sash_width = Some(2.0);
        let mut d = OpeningDialog::for_opening(win, &host(), Vec::new(), OpeningExtras::default());
        {
            let spec = &mut d.form.draft.extras.spec;
            spec.has_sash = false;
            spec.sash_top = 2.5;
            spec.mullion_width = 3.0;
            spec.lintel.exterior = true;
            spec.lintel.style = LintelStyle::Keystone;
            spec.sill.enabled = true;
            spec.arch.kind = ArchType::Tudor;
            spec.arch.height = 9.0;
            spec.shutters.style = ShutterStyle::Louver;
            spec.shutters.sides = ShutterSides::Left;
            spec.shutters.color = [10, 20, 30];
            spec.hardware.enabled = true;
            spec.hardware.handle = HandleStyle::Knob;
            spec.niche_depth = 2.0;
        }
        d.sync_stored();
        // Editing the dialog's own fields keeps the sash width the Sash tab
        // writes on the opening.
        assert_eq!(d.draft().extras.sash_width, Some(2.0));
        let saved = d.draft().clone();
        let back: Opening = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
        assert_eq!(back.extras.spec, saved.extras.spec);
        assert_eq!(back.extras.spec.arch.kind, ArchType::Tudor);
        // Reopened with another session's extras: the opening's values win.
        let d2 = OpeningDialog::for_opening(back, &host(), Vec::new(), OpeningExtras::default());
        assert_eq!(d2.draft().extras.spec, saved.extras.spec);
        // Hardware and shutters have a tab on a door; the window has a Sash tab.
        let door = DOOR_TABS
            .iter()
            .filter(|t| t.enabled)
            .map(|t| t.name)
            .collect::<Vec<_>>();
        for tab in ["Lintel", "Lites", "Arch", "Hardware", "Shutters"] {
            assert!(door.contains(&tab), "{tab}");
        }
        let window = WINDOW_TABS
            .iter()
            .filter(|t| t.enabled)
            .map(|t| t.name)
            .collect::<Vec<_>>();
        for tab in ["Casing", "Lintel", "Sash", "Arch", "Shutters"] {
            assert!(window.contains(&tab), "{tab}");
        }
    }

    #[test]
    fn the_casing_tab_reaches_the_opening_but_only_when_it_changed() {
        let mut d = OpeningDialog::for_opening(
            Opening::default_door(5, 1, 100.0),
            &host(),
            Vec::new(),
            OpeningExtras::default(),
        );
        d.sync_stored();
        assert_eq!(d.draft().casing, None, "untouched stays on the defaults");
        d.form.extras.casing_interior_width = 5.5;
        d.form.extras.casing_interior_depth = 1.0;
        d.form.extras.casing_exterior = false;
        d.sync_stored();
        let c = d.draft().casing.expect("stored");
        assert_eq!((c.width, c.depth, c.reveal), (5.5, 1.0, 0.25));
        assert!(d.draft().extras.spec.casing_interior && !d.draft().extras.spec.casing_exterior);
        let d2 = OpeningDialog::for_opening(
            d.draft().clone(),
            &host(),
            Vec::new(),
            OpeningExtras::default(),
        );
        assert_eq!(d2.extras().casing_interior_width, 5.5);
        assert!(!d2.extras().casing_exterior);
    }

    #[test]
    fn calculate_from_width_makes_the_door_single_or_double_by_its_width() {
        let mut door = Opening::default_door(5, 1, 100.0);
        door.width = 60.0;
        let mut d = OpeningDialog::for_opening(door, &host(), Vec::new(), OpeningExtras::default());
        assert_eq!(d.draft().effective_style(), OpeningStyle::Hinged);
        d.form.draft.extras.spec.calc_panels = true;
        d.sync_stored();
        assert!(d.draft().extras.spec.calc_panels);
        assert_eq!(d.draft().effective_style(), OpeningStyle::DoubleDoor);
        assert_eq!(door_panel_count(d.draft().effective_style(), 60.0), 2);
        d.form.draft.width = 30.0;
        assert_eq!(d.draft().effective_style(), OpeningStyle::Hinged);
    }

    #[test]
    fn a_default_dialog_hands_widths_and_tab_values_to_the_variant_defaults() {
        let mut d = OpeningDialog::for_default(
            OpeningTarget::DefaultWindow,
            Opening::default_window(0, 0, 0.0),
            OpeningExtras::default(),
        );
        let mut v = OpeningVariantDefaults::default();
        // Nothing changed: the plan defaults stay as they are.
        d.apply_to_variants(&mut v);
        assert_eq!(v, OpeningVariantDefaults::default());
        let mut widths = StandardWidths {
            snap: true,
            ..StandardWidths::default()
        };
        widths.lists.retain(|l| l.style != OpeningStyle::Window);
        widths.window_fallback = vec![30.0, 40.0];
        d.form.extras.widths = Some(widths);
        d.form.draft.extras.spec.shutters.style = ShutterStyle::Panel;
        d.sync_stored();
        d.apply_to_variants(&mut v);
        assert!(v.widths.snap);
        assert_eq!(
            v.widths
                .for_style(OpeningKind::Window, OpeningStyle::Window),
            vec![30.0, 40.0]
        );
        assert_eq!(v.window_spec.shutters.style, ShutterStyle::Panel);
        assert_eq!(v.door_spec, OpeningSpec::default());
        // The session copy keeps them for the next time the dialog opens.
        assert_eq!(
            d.extras().default_spec().unwrap().shutters.style,
            ShutterStyle::Panel
        );
        assert!(d.extras().standard_widths().unwrap().snap);
        // A placed opening's dialog does not touch the defaults.
        let mut placed = OpeningDialog::for_opening(
            Opening::default_door(5, 1, 100.0),
            &host(),
            Vec::new(),
            OpeningExtras::default(),
        );
        placed.form.extras.widths = Some(StandardWidths::default());
        let mut v2 = OpeningVariantDefaults::default();
        v2.widths.snap = true;
        placed.apply_to_variants(&mut v2);
        assert!(v2.widths.snap);
    }

    #[test]
    fn niche_depth_comes_from_the_opening_and_defaults_to_three_and_a_half() {
        let mut n = Opening::new(1, 100.0, OpeningKind::Window, 24.0, 36.0, 36.0);
        n.style = OpeningStyle::WallNiche;
        assert_eq!(n.niche_depth(6.0), 3.5);
        assert_eq!(n.niche_depth(4.5), 3.5);
        n.extras.spec.niche_depth = 2.0;
        assert_eq!(n.niche_depth(6.0), 2.0);
        // At most the wall less an inch.
        n.extras.spec.niche_depth = 9.0;
        assert_eq!(n.niche_depth(6.0), 5.0);
    }

    #[test]
    fn every_enabled_tab_draws() {
        let ctx = egui::Context::default();
        for (opening, host_kind) in [
            (Opening::default_door(5, 1, 100.0), WallKind::Exterior),
            (Opening::default_window(6, 1, 100.0), WallKind::Interior),
        ] {
            for style in [
                opening.style,
                OpeningStyle::WallNiche,
                OpeningStyle::Casement,
            ] {
                let mut o = opening.clone();
                o.style = style;
                o.extras.spec.arch.kind = ArchType::Gothic;
                o.extras.spec.shutters.style = ShutterStyle::Panel;
                o.extras.spec.lite_style = LiteStyle::Custom;
                let mut wall = host();
                wall.kind = host_kind;
                let mut d =
                    OpeningDialog::for_opening(o, &wall, Vec::new(), OpeningExtras::default());
                for tab in 0..d.form.tabs().len() {
                    if !d.form.tabs()[tab].enabled {
                        continue;
                    }
                    let _ = ctx.run(egui::RawInput::default(), |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| d.form.page(ui, tab));
                    });
                }
            }
        }
        // The default dialogs show the standard widths on General.
        let mut d = OpeningDialog::for_default(
            OpeningTarget::DefaultDoor,
            Opening::default_door(0, 0, 0.0),
            OpeningExtras::default(),
        );
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| d.form.page(ui, 0));
        });
    }
    #[test]
    fn the_schedule_indicator_and_sill_threshold_tabs_are_live() {
        for o in [
            Opening::default_door(5, 1, 100.0),
            Opening::default_window(6, 1, 100.0),
        ] {
            let d = OpeningDialog::for_opening(o, &host(), Vec::new(), OpeningExtras::default());
            for name in ["Schedule", "Opening Indicators", "Sill/Threshold"] {
                let tab = d
                    .form
                    .tabs()
                    .iter()
                    .find(|t| t.name == name)
                    .unwrap_or_else(|| panic!("no {name} tab"));
                assert!(tab.enabled, "{name} is dimmed");
            }
        }
    }

    #[test]
    fn schedule_plan_detail_values_are_stored_with_the_opening() {
        let mut d = OpeningDialog::for_opening(
            Opening::default_door(5, 1, 100.0),
            &host(),
            Vec::new(),
            OpeningExtras::default(),
        );
        {
            let o = d.draft_mut();
            o.schedule_number = Some("A7".into());
            let spec = &mut o.extras.spec;
            spec.schedule.include = false;
            spec.schedule.supplier = "Acme".into();
            spec.schedule.comment = "Primed".into();
            spec.swings_both = true;
            spec.threshold = false;
            spec.recess_depth = Some(1.5);
            spec.size_includes_frame = false;
            spec.indicators.swing_arrows = true;
        }
        d.sync_stored();
        let back: Opening =
            serde_json::from_str(&serde_json::to_string(d.draft()).unwrap()).unwrap();
        assert_eq!(format!("{back:?}"), format!("{:?}", d.draft()));
        assert_eq!(back.schedule_number.as_deref(), Some("A7"));
        assert!(!back.extras.spec.schedule.include);
        assert_eq!(back.extras.spec.schedule.supplier, "Acme");
        assert_eq!(back.extras.spec.recess_depth, Some(1.5));
        // A plan from before the tabs reads with their defaults.
        let mut v: serde_json::Value = serde_json::to_value(&back).unwrap();
        let spec = v["extras"]["spec"].as_object_mut().unwrap();
        for k in [
            "swings_both",
            "threshold",
            "jamb_in_plan",
            "size_includes_frame",
            "recess_depth",
            "indicators",
            "schedule",
        ] {
            spec.remove(k);
        }
        let old: Opening = serde_json::from_value(v).unwrap();
        let spec = &old.extras.spec;
        assert!(spec.schedule.include && spec.threshold && spec.jamb_in_plan);
        assert!(spec.size_includes_frame && !spec.swings_both);
        assert_eq!(spec.recess_depth, None);
        assert_eq!(
            spec.indicators,
            plan_core::openings::spec::OpeningIndicators::default()
        );
    }

    #[test]
    fn two_windows_keep_the_minimum_separation_and_so_does_a_door() {
        let mut p = Project::new("t");
        let w = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(300.0, 0.0),
            6.0,
            109.125,
            WallKind::Exterior,
        );
        let mut win = Opening::default_window(0, 0, 0.0);
        win.width = 36.0;
        place_from_template(&mut p, 0, w, 100.0, &win).unwrap();
        // Flush against the first (82..118) is closer than the 2" separation;
        // 2" clear is fine: 120..156, centered 138.
        assert!(place_from_template(&mut p, 0, w, 136.0, &win).is_none());
        assert!(place_from_template(&mut p, 0, w, 138.0, &win).is_some());
        // The third one: 1" short of the separation is refused, 2" is fine.
        assert!(place_from_template(&mut p, 0, w, 175.0, &win).is_none());
        assert!(place_from_template(&mut p, 0, w, 176.0, &win).is_some());
        // A door beside a window keeps the same distance.
        let mut door = Opening::default_door(0, 0, 0.0);
        door.width = 30.0;
        door.sill_height = 0.0;
        assert!(place_from_template(&mut p, 0, w, 120.0, &door).is_none());
        // Touching the last window (ending at 194) is too close, 2" clear is fine.
        assert!(place_from_template(&mut p, 0, w, 209.0, &door).is_none());
        assert!(place_from_template(&mut p, 0, w, 211.0, &door).is_some());
        // The plan's Minimum Separation moves the stop: flush against that
        // door (196..226) is refused with 2", fine with none.
        assert!(place_from_template(&mut p, 0, w, 241.0, &door).is_none());
        p.opening_display.min_separation = 0.0;
        assert!(place_from_template(&mut p, 0, w, 241.0, &door).is_some());
    }

    #[test]
    fn the_round_15_tabs_are_live_and_their_values_survive_the_dialog() {
        use plan_core::openings::spec::{CurtainStyle, ShapeKind, WindowShape};
        let ctx = egui::Context::default();
        let door_tabs = [
            "Rough Opening",
            "Framing",
            "Energy Values",
            "Layer",
            "Materials",
            "Object Information",
        ];
        let mut d = OpeningDialog::for_opening(
            Opening::default_door(5, 1, 100.0),
            &host(),
            Vec::new(),
            OpeningExtras::default(),
        );
        for tab in door_tabs {
            assert!(d.draw_tab_for_test(&ctx, tab), "door {tab}");
        }
        // Only a window has a Shape and a Treatments tab.
        assert!(!d.draw_tab_for_test(&ctx, "Shape"));
        assert!(!d.draw_tab_for_test(&ctx, "Treatments"));
        // The Components tab is still dimmed.
        assert!(!d.draw_tab_for_test(&ctx, "Components"));
        let mut w = OpeningDialog::for_opening(
            Opening::default_window(6, 1, 100.0),
            &host(),
            Vec::new(),
            OpeningExtras::default(),
        );
        for tab in door_tabs.iter().copied().chain(["Shape", "Treatments"]) {
            assert!(w.draw_tab_for_test(&ctx, tab), "window {tab}");
        }
        // What the pages edit stays on the draft through the per-frame sync.
        {
            let (width, height) = (w.draft().width, w.draft().height);
            let spec = &mut w.draft_mut().extras.spec;
            spec.shape = WindowShape::preset(ShapeKind::Triangle, width, height);
            spec.treatments.curtain = CurtainStyle::Pleated;
            spec.rough.add_width = 2.0;
            spec.framing.trimmers = Some(2);
            spec.energy.u_factor = 0.25;
            spec.layer = Some("Windows, Labels".into());
            spec.materials.set("Frame", Some(("Oak", [1, 2, 3])));
            spec.info.id = "W-9".into();
        }
        for tab in ["Shape", "Treatments", "Rough Opening", "Materials"] {
            assert!(w.draw_tab_for_test(&ctx, tab));
        }
        let spec = &w.draft().extras.spec;
        assert!(spec.shape.is_shaped());
        assert_eq!(spec.treatments.curtain, CurtainStyle::Pleated);
        assert_eq!(spec.rough.add_width, 2.0);
        assert_eq!(spec.framing.trimmers, Some(2));
        assert_eq!(spec.energy.u_factor, 0.25);
        assert_eq!(w.draft().layer_name(), "Windows, Labels");
        assert_eq!(spec.materials.color("Frame"), Some([1, 2, 3]));
        assert_eq!(spec.info.id, "W-9");
        // A double door shows its Door Swing group on the Options tab.
        let mut dd = Opening::default_door(7, 1, 100.0);
        dd.style = OpeningStyle::DoubleDoor;
        dd.width = 60.0;
        let mut dd = OpeningDialog::for_opening(dd, &host(), Vec::new(), OpeningExtras::default());
        assert!(dd.draw_tab_for_test(&ctx, "Options"));
        // The default window dialog hands the new values to the variants.
        let mut defaults = OpeningDialog::for_default(
            OpeningTarget::DefaultWindow,
            Opening::default_window(0, 0, 0.0),
            OpeningExtras::default(),
        );
        defaults.draft_mut().extras.spec.treatments.curtain = CurtainStyle::Panels;
        defaults.draft_mut().extras.spec.energy.u_factor = 0.2;
        defaults.sync_stored();
        let mut v = OpeningVariantDefaults::default();
        defaults.apply_to_variants(&mut v);
        assert_eq!(v.window_spec.treatments.curtain, CurtainStyle::Panels);
        assert_eq!(v.window_spec.energy.u_factor, 0.2);
        assert_eq!(v.door_spec, OpeningSpec::default());
    }
}
