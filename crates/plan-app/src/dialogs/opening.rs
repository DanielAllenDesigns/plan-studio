//! Door and Window Specification (docs/chief-x18-dialogs.md).

use super::{
    dis_check, dis_combo, dis_radio, fmt_short, off, on, pv_text, row, section, session_check,
    wall_plan_sketch, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_ACCENT, PV_BG, PV_FAINT,
    PV_GLASS, PV_INK, PV_WALL,
};
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};
use plan_core::defaults::{OpeningDefaults, WindowDefaults};
use plan_core::{Id, Opening, OpeningKind, Project, Wall, WallKind};

/// Minimum clear distance between an opening jamb and a wall end or another
/// opening. Mirrors the private constant `Project::add_opening` uses.
pub const OPENING_MARGIN: f64 = 2.0;

const DEFAULT_DOOR_KEY: Id = Id::MAX - 2;
const DEFAULT_WINDOW_KEY: Id = Id::MAX - 3;
const DEFAULT_EXTERIOR_DOOR_KEY: Id = Id::MAX - 4;

const DOOR_STYLES: [&str; 7] = [
    "Hinged", "Sliding", "Pocket", "Bifold", "Garage", "Doorway", "Barn",
];
const WINDOW_TYPES: [&str; 6] = [
    "Single Casement",
    "Double Hung",
    "Slider",
    "Fixed Glass",
    "Awning",
    "Picture",
];

const DOOR_TABS: &[Tab] = &[
    on("General"),
    on("Options"),
    on("Casing"),
    off("Lintel"),
    off("Sill/Threshold"),
    off("Lites"),
    on("Jamb"),
    off("Arch"),
    off("Hardware"),
    off("Shutters"),
    off("Opening Indicators"),
    off("Rough Opening"),
    off("Framing"),
    off("Energy Values"),
    off("Layer"),
    off("Materials"),
    on("Label"),
    off("Components"),
    off("Object Information"),
    off("Schedule"),
];

const WINDOW_TABS: &[Tab] = &[
    on("General"),
    on("Options"),
    off("Casing"),
    off("Lintel"),
    off("Sill/Threshold"),
    off("Sash"),
    on("Frame"),
    on("Lites"),
    off("Shape"),
    off("Arch"),
    off("Treatments"),
    off("Shutters"),
    off("Opening Indicators"),
    off("Rough Opening"),
    off("Framing"),
    off("Energy Values"),
    off("Layer"),
    off("Materials"),
    on("Label"),
    off("Components"),
    off("Object Information"),
    off("Schedule"),
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
}

impl OpeningTarget {
    /// Key of this target in the app's per-session extras map.
    pub fn key(self) -> Id {
        match self {
            OpeningTarget::Placed(id) => id,
            OpeningTarget::DefaultDoor => DEFAULT_DOOR_KEY,
            OpeningTarget::DefaultExteriorDoor => DEFAULT_EXTERIOR_DOOR_KEY,
            OpeningTarget::DefaultWindow => DEFAULT_WINDOW_KEY,
        }
    }
}

/// Door and window settings the model has no fields for yet, kept per
/// session by the app. One struct serves both kinds.
#[derive(Clone, Debug, PartialEq)]
pub struct OpeningExtras {
    /// Library style name (e.g. "Door P04"); shown read-only.
    style_name: String,
    door_style: usize,
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
    display_in_plan: bool,
    suppress_label: bool,
    specify_label: bool,
    label_text: String,
    size_format: usize,
    include_schedule_number: bool,
    include_type: bool,
}

impl Default for OpeningExtras {
    fn default() -> Self {
        Self {
            style_name: String::new(),
            door_style: 0,
            window_type: 0,
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
            display_in_plan: true,
            suppress_label: false,
            specify_label: false,
            label_text: String::new(),
            size_format: 1,
            include_schedule_number: true,
            include_type: true,
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
            window_type: WINDOW_TYPES
                .iter()
                .position(|t| *t == d.window_type)
                .unwrap_or(0),
            style_name: d.window_type.clone(),
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
            window_type: WINDOW_TYPES[self.window_type].to_string(),
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
    wall: Option<HostWall>,
    /// The other openings on the same wall (for the overlap rule).
    others: Vec<Opening>,
    fields: Fields,
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
            length: wall.length(),
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
        let title = match draft.kind {
            OpeningKind::Door => "Door Specification",
            OpeningKind::Window => "Window Specification",
        };
        let key = match draft.kind {
            OpeningKind::Door => "door",
            OpeningKind::Window => "window",
        };
        Self {
            frame: SpecDialog::new(title, key),
            form: OpeningForm {
                target,
                draft,
                extras,
                wall,
                others,
                fields: Fields::default(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn target(&self) -> OpeningTarget {
        self.form.target
    }

    pub fn draft(&self) -> &Opening {
        &self.form.draft
    }

    pub fn extras(&self) -> &OpeningExtras {
        &self.form.extras
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
    let wall_len = project.floors[floor].wall(wall_id)?.length();
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
    let f = &mut project.floors[floor];
    let overlaps = f.openings_on(wall_id).any(|o| overlap(&opening, o));
    if overlaps {
        return None;
    }
    f.openings.push(opening);
    Some(id)
}

fn overlap(a: &Opening, b: &Opening) -> bool {
    a.start_offset() < b.end_offset() + OPENING_MARGIN
        && a.end_offset() > b.start_offset() - OPENING_MARGIN
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
                    .selected_text(DOOR_STYLES[self.extras.door_style])
                    .show_ui(ui, |ui| {
                        for (i, s) in DOOR_STYLES.iter().enumerate() {
                            ui.selectable_value(&mut self.extras.door_style, i, *s);
                        }
                    })
                    .response
                    .on_hover_text(
                        "Only Hinged changes the plan symbol today; the style is kept per session",
                    );
            });
            if !self.extras.style_name.is_empty() {
                row(ui, "Library Style", |ui| {
                    ui.label(self.extras.style_name.as_str());
                });
            }
            row(ui, "Door Type", |ui| dis_combo(ui, "door_type", "Hinged"));
            if self.extras.door_style != 0 {
                ui.weak("Plan and 3D symbols for this style arrive in a later phase.");
            }
        } else {
            row(ui, "Window Type", |ui| {
                egui::ComboBox::from_id_salt("window_type")
                    .selected_text(WINDOW_TYPES[self.extras.window_type])
                    .show_ui(ui, |ui| {
                        for (i, s) in WINDOW_TYPES.iter().enumerate() {
                            ui.selectable_value(&mut self.extras.window_type, i, *s);
                        }
                    })
                    .response
                    .on_hover_text("Stored per session until the model grows these fields");
            });
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
        row(ui, "Elevation Reference", |ui| {
            dis_combo(ui, "elev_ref", "From Floor")
        });
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
    }

    fn door_options(&mut self, ui: &mut Ui) {
        section(ui, "Door Swing");
        row(ui, "Hinge side", |ui| {
            ui.radio_value(&mut self.draft.swing_flipped, false, "Left")
                .on_hover_text("Hinge on the wall-start jamb");
            ui.radio_value(&mut self.draft.swing_flipped, true, "Right")
                .on_hover_text("Hinge on the wall-end jamb");
        });
        row(ui, "Swing Angle", |ui| {
            ui.add(
                egui::DragValue::new(&mut self.extras.swing_angle)
                    .range(0.0..=180.0)
                    .speed(1.0)
                    .suffix("\u{B0}"),
            )
            .on_hover_text(super::SESSION_NOTE);
        });
        section(ui, "Open/Close Display");
        session_check(ui, &mut self.extras.show_open_2d, "Show Open in 2D");
        dis_check(ui, "Show Open in 3D", false);
        ui.add_enabled_ui(false, |ui| {
            section(ui, "Door Panels");
            row(ui, "Panels", |ui| {
                dis_radio(ui, "Single Door Only", false);
                dis_radio(ui, "Double Door Only", false);
                dis_radio(ui, "Calculate from Width", true);
            });
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
            section(ui, "Recessed into Wall");
            dis_check(ui, "Recessed To Layer", true);
            section(ui, "Plinth Blocks");
            dis_check(ui, "Interior Plinth Block", false);
            dis_check(ui, "Exterior Plinth Block", false);
        });
    }

    fn window_options(&mut self, ui: &mut Ui) {
        section(ui, "Options");
        dis_check(ui, "Interior Corner Block", false);
        dis_check(ui, "Exterior Corner Block", false);
        session_check(ui, &mut self.extras.egress, "Egress");
        session_check(ui, &mut self.extras.tempered, "Tempered Glass");
        section(ui, "Display");
        session_check(ui, &mut self.extras.show_open_2d, "Show Open in 2D");
        dis_check(ui, "Show Open in 3D", false);
        ui.add_enabled_ui(false, |ui| {
            section(ui, "Recessed into Wall");
            dis_check(ui, "Recessed To Layer", true);
        });
    }

    fn casing(&mut self, ui: &mut Ui) {
        let e = &mut self.extras;
        let exterior_ok = self
            .wall
            .as_ref()
            .is_none_or(|w| w.kind == WallKind::Exterior);
        section(ui, "Interior Casing");
        ui.checkbox(&mut e.casing_interior, "Use Interior Casing")
            .on_hover_text(super::SESSION_NOTE);
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
                .on_hover_text(super::SESSION_NOTE);
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
        ui.add_enabled_ui(false, |ui| {
            section(ui, "Double Wall Options");
            row(ui, "Double wall", |ui| {
                dis_radio(ui, "Through", true);
                dis_radio(ui, "Enlarged", false);
                dis_radio(ui, "Double", false);
            });
            section(ui, "Curved Wall Casing");
            row(ui, "Curved wall", |ui| {
                dis_radio(ui, "Straight", false);
                dis_radio(ui, "Radial", true);
                dis_radio(ui, "Parallel", false);
            });
        });
    }

    /// The Jamb tab (doors) and Frame tab (windows) share their controls.
    fn jamb_or_frame(&mut self, ui: &mut Ui) {
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
            row(ui, "Positioning", |ui| {
                ui.radio_value(&mut e.size_includes_jamb, true, positioning);
                ui.radio_value(
                    &mut e.size_includes_jamb,
                    false,
                    positioning.replace("Includes", "Excludes"),
                );
            });
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
        section(ui, "Lites");
        row(ui, "Type", |ui| dis_combo(ui, "lite_type", "Normal"));
        row(ui, "Lites Across", |ui| {
            ui.add(egui::DragValue::new(&mut e.lites_across).range(1..=12));
        });
        row(ui, "Lites Vertical", |ui| {
            ui.add(egui::DragValue::new(&mut e.lites_vertical).range(1..=12));
        });
        self.fields
            .length_row(ui, "Muntin Width", "muntin", &mut e.muntin_width);
        ui.add_enabled_ui(false, |ui| {
            dis_check(ui, "Lites in Fixed", true);
            dis_check(ui, "Lites in Movable", true);
            dis_check(ui, "Muntin in Corner", false);
            dis_check(ui, "Auto Adjust Lites for Component Size", true);
            section(ui, "Round Top Arch");
            dis_check(ui, "Concentric", false);
        });
    }

    fn label(&mut self, ui: &mut Ui) {
        let door = self.is_door();
        let e = &mut self.extras;
        section(ui, "Display Options");
        session_check(ui, &mut e.suppress_label, "Suppress Label in All Views");
        session_check(ui, &mut e.display_in_plan, "Display in Plan View");
        section(ui, "Label Content");
        ui.radio_value(&mut e.specify_label, false, "Automatic Label")
            .on_hover_text(super::SESSION_NOTE);
        ui.radio_value(&mut e.specify_label, true, "Specify Label")
            .on_hover_text(super::SESSION_NOTE);
        ui.add_enabled(
            e.specify_label,
            egui::TextEdit::singleline(&mut e.label_text).desired_width(260.0),
        );
        ui.add_enabled_ui(!e.specify_label, |ui| {
            row(ui, "Size Format", |ui| {
                ui.radio_value(&mut e.size_format, 0, "Height/Width");
                ui.radio_value(&mut e.size_format, 1, "Width/Height");
                ui.radio_value(&mut e.size_format, 2, "Width Only");
            });
            ui.checkbox(&mut e.include_schedule_number, "Include Schedule Number");
            ui.checkbox(&mut e.include_type, "Include Type");
        });
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
            "Label" => self.label(ui),
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
            door_elevation(p, elev, o, e.door_style);
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
        wall_plan_sketch(
            p,
            Rect::from_min_max(bottom.min + Vec2::new(0.0, 8.0), bottom.max),
            len,
            thick,
            std::slice::from_ref(&sample),
            Some(sample.id),
            if self.is_door() { e.swing_angle } else { 90.0 },
        );
    }
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

fn door_elevation(p: &Painter, area: Rect, o: &Opening, style: usize) {
    let (r, floor_y) = fit_on_floor(area, o.width, o.height, o.sill_height);
    p.hline(area.x_range(), floor_y, Stroke::new(1.0_f32, PV_INK));
    // Casing/jamb around the leaf.
    outlined(p, r.expand(3.0), PV_TRIM);
    let hinge_left = !o.swing_flipped;
    match style {
        // Sliding: two overlapping leaves with lites and an arrow.
        1 => {
            outlined(p, frac(r, 0.0, 0.0, 0.55, 1.0), PV_DOOR);
            outlined(p, frac(r, 0.45, 0.0, 1.0, 1.0), PV_DOOR);
            outlined(p, frac(r, 0.08, 0.08, 0.47, 0.8), PV_GLASS);
            outlined(p, frac(r, 0.53, 0.08, 0.92, 0.8), PV_GLASS);
        }
        // Pocket: the leaf half-way out of the wall pocket.
        2 => {
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
        3 => {
            for i in 0..4 {
                let x = i as f32 * 0.25;
                outlined(p, frac(r, x, 0.0, x + 0.25, 1.0), PV_DOOR);
            }
        }
        // Garage: horizontal sections, lites in the top one.
        4 => {
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
        5 => {
            p.rect_filled(r, 0.0, PV_BG);
        }
        // Barn: plank leaf with a Z brace, hanging on a rail.
        6 => {
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

fn window_elevation(p: &Painter, area: Rect, o: &Opening, e: &OpeningExtras) {
    let (r, floor_y) = fit_on_floor(area, o.width, o.height, o.sill_height);
    p.hline(area.x_range(), floor_y, Stroke::new(1.0_f32, PV_INK));
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
    match e.window_type {
        0 => {
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
        1 => {
            p.line_segment(
                [Pos2::new(glass.min.x, c.y), Pos2::new(glass.max.x, c.y)],
                Stroke::new(2.5_f32, PV_INK),
            );
        }
        2 => {
            p.line_segment(
                [Pos2::new(c.x, glass.min.y), Pos2::new(c.x, glass.max.y)],
                Stroke::new(2.5_f32, PV_INK),
            );
            p.line_segment([Pos2::new(c.x - 8.0, c.y), Pos2::new(c.x + 8.0, c.y)], sym);
        }
        4 => {
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
        assert_eq!(win.window_type, 0);
        let w = Opening::default_window(0, 0, 0.0);
        let back = win.to_window_defaults(&w, &d.window);
        assert_eq!(back.window_type, "Single Casement");
        assert_eq!(back.frame_width, 0.75);
        assert_eq!(back.sash_width, 1.5);
        assert!(back.egress);
    }
}
