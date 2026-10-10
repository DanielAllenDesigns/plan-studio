//! The Build Roof dialog (RF-1, RF-2), the Roof Plane Specification (RF-36,
//! with its holes list and Build Roof edge fields) and the Dormer
//! Specification (RF-48), on the shared dialog frame.
//!
//! Both edit a cloned draft; OK hands it back through `draft()`.

#![allow(dead_code)]

use super::assembly_def::AssemblyDefDialog;
use super::{
    fmt_short, on, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_ACCENT, PV_INK,
};
use crate::editor::roof_view::{
    pitch_label, AllPlanesEdit, CeilingFraming, CeilingRecord, FillKind, RoofFraming,
    RoofPlaneRecord, RoofSettings, RoofStructure, RoofStyle, LOCKS, ROOF_MATERIALS,
};
use eframe::egui::{self, Align2, FontId, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::assemblies::{AssemblyKind, AssemblyLibrary, RoofLayers};
use plan_core::defaults::{EaveCut, RoofDetailDefaults};
use plan_core::LineStyle;
use plan_roof::{
    BaselineOver, DormerKind, DormerSpec, HeightLock, HeightSettings, LengthEntry, ReturnKind,
    ReturnSpec, NO_BIRDSMOUTH_RAISE,
};

const MIN_PITCH: f64 = 0.5;
const MAX_PITCH: f64 = 24.0;

fn pitch_row(ui: &mut Ui, label: &str, pitch: &mut f64) {
    // Rise per 12, or degrees with Pitch in Degrees on (RF-72).
    row(ui, label, |ui| {
        super::roof_baseline::pitch_drag(ui, pitch);
    });
}

fn material_combo(ui: &mut Ui, salt: &str, value: &mut String) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(value.clone())
        .show_ui(ui, |ui| {
            for m in ROOF_MATERIALS {
                ui.selectable_value(value, m.to_string(), m);
            }
        });
}

/// The combo of a default / on / off choice.
fn tri_combo(ui: &mut Ui, salt: &str, value: &mut Option<bool>) {
    let shown = match value {
        None => "Roof Default",
        Some(true) => "On",
        Some(false) => "Off",
    };
    egui::ComboBox::from_id_salt(salt)
        .selected_text(shown)
        .show_ui(ui, |ui| {
            ui.selectable_value(value, None, "Roof Default");
            ui.selectable_value(value, Some(true), "On");
            ui.selectable_value(value, Some(false), "Off");
        });
}

/// A default / on / off choice for an option a plane can set for itself.
fn tri_state(ui: &mut Ui, salt: &str, label: &str, value: &mut Option<bool>) {
    row(ui, label, |ui| tri_combo(ui, salt, value));
}

/// A wall type for the roof detail: a combo over `types`, or a text field
/// when the caller has no list. Empty keeps the wall's own type.
fn wall_type_row(ui: &mut Ui, salt: &str, label: &str, value: &mut String, types: &[String]) {
    row(ui, label, |ui| {
        if types.is_empty() {
            ui.add(
                egui::TextEdit::singleline(value)
                    .hint_text("the wall's own type")
                    .desired_width(160.0),
            );
        } else {
            let shown = if value.is_empty() {
                "(the wall's own type)".to_string()
            } else {
                value.clone()
            };
            egui::ComboBox::from_id_salt(salt)
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    ui.selectable_value(value, String::new(), "(the wall's own type)");
                    for t in types {
                        ui.selectable_value(value, t.clone(), t);
                    }
                });
        }
    });
}

/// The roof detail form shared by Default Settings > Roof Defaults and the
/// Build Roof dialog (RF-14, RF-15, RF-28, RF-31): structure, eave cut,
/// fascia, soffit, frieze, ridge caps, gutters, rafter tails, attic walls
/// and the Build Roof baseline rule. `types` are the wall types to choose
/// from (empty: type the name).
pub fn detail_form(ui: &mut Ui, fields: &mut Fields, d: &mut RoofDetailDefaults, types: &[String]) {
    section(ui, "Roof Structure");
    fields.length_row(ui, "Thickness", "rd_thickness", &mut d.thickness);
    ui.checkbox(&mut d.baseline_at_plate, "Roof baseline at top plate")
        .on_hover_text(
            "Build Roof seats the underside of the structure on the top plate at the wall, so gable corners meet the plate with no gap",
        );
    section(ui, "Eaves");
    row(ui, "Eave Cut", |ui| {
        egui::ComboBox::from_id_salt("rd_eave_cut")
            .selected_text(d.eave_cut.label())
            .show_ui(ui, |ui| {
                for c in EaveCut::ALL {
                    ui.selectable_value(&mut d.eave_cut, c, c.label());
                }
            });
    });
    ui.checkbox(&mut d.fascia, "Fascia");
    if d.fascia {
        fields.length_row(ui, "Fascia Height", "rd_fascia_h", &mut d.fascia_height);
        fields.length_row(
            ui,
            "Fascia Thickness",
            "rd_fascia_t",
            &mut d.fascia_thickness,
        );
    }
    ui.checkbox(&mut d.rake_fascia, "Rake Fascia");
    ui.checkbox(&mut d.soffit, "Soffit");
    ui.add_enabled(
        d.soffit,
        egui::Checkbox::new(&mut d.sloped_soffit, "Sloped Soffit"),
    );
    ui.checkbox(&mut d.frieze, "Frieze Board");
    ui.checkbox(&mut d.ridge_caps, "Ridge and Hip Caps")
        .on_hover_text("Drawn on planes with Include Ridge Caps on");
    ui.checkbox(&mut d.gutters, "Gutters");
    if d.gutters {
        fields.length_row(ui, "Gutter Size", "rd_gutter", &mut d.gutter_size);
    }
    ui.checkbox(&mut d.flashing, "Flashing at Butting Roofs");
    section(ui, "Rafter Tails");
    ui.checkbox(&mut d.rafter_tails, "Exposed Rafter Tails")
        .on_hover_text("Tails replace the soffit under the eaves");
    if d.rafter_tails {
        fields.length_row(
            ui,
            "Spacing (on center)",
            "rd_rafter_sp",
            &mut d.rafter_spacing,
        );
        fields.length_row(ui, "Rafter Width", "rd_rafter_w", &mut d.rafter_width);
        fields.length_row(ui, "Rafter Depth", "rd_rafter_d", &mut d.rafter_depth);
    }
    section(ui, "Walls Under the Roof");
    ui.checkbox(&mut d.auto_attic_walls, "Auto Attic Walls");
    wall_type_row(
        ui,
        "rd_attic_type",
        "Attic Wall Type",
        &mut d.attic_wall_type,
        types,
    );
    wall_type_row(
        ui,
        "rd_lower_type",
        "Lower Wall Type if Split by Butting Roof",
        &mut d.lower_wall_type,
        types,
    );
    ui.checkbox(&mut d.roof_cuts_wall_at_bottom, "Roof Cuts Wall at Bottom")
        .on_hover_text("A wall standing over a lower roof is cut along it");
}

/// Why `d` cannot be used, if it cannot.
pub fn detail_error(d: &RoofDetailDefaults) -> Option<String> {
    if d.thickness <= 0.0 {
        Some("The roof thickness must be more than zero".into())
    } else if d.fascia && (d.fascia_height < 0.0 || d.fascia_thickness < 0.0) {
        Some("The fascia size cannot be negative".into())
    } else if d.rafter_tails
        && (d.rafter_spacing < 1.0 || d.rafter_width <= 0.0 || d.rafter_depth <= 0.0)
    {
        Some("Rafter spacing must be at least 1 inch, with a positive width and depth".into())
    } else {
        None
    }
}

// ===================================================================
// Build Roof
// ===================================================================

const BUILD_TABS: &[Tab] = &[on("Roof"), on("Options"), on("Materials"), on("Detail")];

struct BuildPages {
    s: RoofSettings,
    fields: Fields,
    /// Which floor the roof goes on, for the Roof page.
    note: String,
    /// The Roof Styles button pressed (RF-3): OK writes its directives to the
    /// exterior walls before building. `None` builds from the walls as they are.
    style: Option<RoofStyle>,
}

impl BuildPages {
    /// The Roof Height group of the Build Roof dialog (manual pp. 829, 830):
    /// Heel Height or the birdsmouth fields by framing method, and the three
    /// eave switches.
    fn height_group(&mut self, ui: &mut egui::Ui) {
        use plan_roof::RoofFraming;
        section(ui, "Roof Height");
        match self.s.heights.framing {
            RoofFraming::Trusses => {
                self.fields.length_row(
                    ui,
                    "Heel Height",
                    "heel_height",
                    &mut self.s.heights.heel_height,
                );
            }
            RoofFraming::Rafters => {
                ui.checkbox(
                    &mut self.s.heights.auto_birdsmouth,
                    "Automatic Birdsmouth Cut",
                )
                .on_hover_text(
                    "On, the cut follows the pitch and the seat is the plate; off, \
                     enter the Raise Off Plate / Birdsmouth Cut (negative cuts)",
                );
                let cut = self.s.heights.birdsmouth_cut;
                if self.s.heights.auto_birdsmouth {
                    ui.weak("Birdsmouth Cut and Seat follow the pitch and the plate");
                } else {
                    self.fields.length_row(
                        ui,
                        "Raise Off Plate / Birdsmouth Cut",
                        "birdsmouth_cut",
                        &mut self.s.heights.birdsmouth_cut,
                    );
                    if cut < 0.0 {
                        let seat = plan_roof::birdsmouth_seat_for_cut(-cut, self.s.pitch);
                        ui.weak(format!("Birdsmouth Seat {}", fmt_short(seat)));
                    }
                }
            }
        }
        let depth = plan_roof::vertical_structure_depth(self.s.detail.thickness, self.s.pitch);
        ui.weak(format!(
            "Vertical Structure Depth {} (Structure panel)",
            fmt_short(depth)
        ));
        let h = &mut self.s.heights;
        ui.checkbox(
            &mut h.same_roof_height,
            "Same Roof Height at Exterior Walls",
        )
        .on_hover_text(
            "Bearing walls stay the same height; overhangs change so eaves meet \
                 (wall overhangs are ignored)",
        );
        ui.checkbox(&mut h.same_height_eaves, "Same Height Eaves")
            .on_hover_text(
                "Every eave is at the height of a default-pitch plane; planes are \
                 raised or lowered to meet it",
            );
        ui.checkbox(&mut h.allow_low_planes, "Allow Low Roof Planes")
            .on_hover_text("Uncheck only when an upper floor overhangs the roofs below");
    }
}

impl SpecPages for BuildPages {
    fn tabs(&self) -> &'static [Tab] {
        BUILD_TABS
    }

    fn error(&self) -> Option<String> {
        let sw = &self.s.switches;
        if self.fields.any_invalid() {
            Some("Enter a valid length".into())
        } else if sw.pitch_in_degrees
            && (self.s.pitch < MIN_PITCH || self.s.pitch > 12.0 * 89f64.to_radians().tan())
        {
            Some("Pitch must be between 1 and 89 degrees".into())
        } else if !sw.pitch_in_degrees && (self.s.pitch < MIN_PITCH || self.s.pitch > MAX_PITCH) {
            Some("Pitch must be between 0.5 and 24 in 12".into())
        } else if sw.segment_angle < plan_roof::SEGMENT_ANGLE_RANGE.0 - 1e-9
            || sw.segment_angle > plan_roof::SEGMENT_ANGLE_RANGE.1 + 1e-9
        {
            Some("Segment Angle at Curved Wall must be between 6 and 90 degrees".into())
        } else {
            detail_error(&self.s.detail)
        }
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match tab {
            0 => {
                section(ui, "Roof Styles");
                ui.horizontal_wrapped(|ui| {
                    for st in RoofStyle::ALL {
                        let on = self.style == Some(st);
                        if ui
                            .selectable_label(on, st.label())
                            .on_hover_text("Writes the roof directive of every exterior wall")
                            .clicked()
                        {
                            self.style = if on { None } else { Some(st) };
                        }
                    }
                });
                ui.weak(match self.style {
                    Some(st) => style_note(st),
                    None => "No style: the roof follows each wall's own Roof tab.",
                });
                section(ui, "Roof");
                ui.checkbox(&mut self.s.build_planes, "Build Roof Planes");
                ui.checkbox(&mut self.s.auto_rebuild, "Auto Rebuild Roofs")
                    .on_hover_text("Rebuild the automatic planes when the walls change");
                self.build_switches(ui);
                ui.checkbox(&mut self.s.ignore_top_floor, "Ignore Top Floor")
                    .on_hover_text("Build the roof over the floor below the top one");
                ui.checkbox(
                    &mut self.s.build_ceiling_planes,
                    "Build ceiling planes for vaulted rooms",
                )
                .on_hover_text(
                    "Rooms with Ceiling Over This Room turned off get ceiling planes that follow the roof",
                );
                ui.checkbox(&mut self.s.build_attic_floor, "Build attic floor")
                    .on_hover_text(
                        "Also add the attic floor (attic walls and an Attic room) under the roof",
                    );
                section(ui, "Defaults for walls without their own roof settings");
                pitch_row(ui, "Pitch", &mut self.s.pitch);
                if ui
                    .checkbox(&mut self.s.switches.pitch_in_degrees, "Pitch in Degrees")
                    .on_hover_text("Dialogs and roof plane labels show degrees (-89 to 89)")
                    .changed()
                {
                    plan_roof::set_pitch_display_degrees(self.s.switches.pitch_in_degrees);
                }
                self.fields
                    .length_row(ui, "Overhang", "overhang", &mut self.s.overhang);
                self.fields.degrees_row(
                    ui,
                    "Segment Angle at Curved Wall",
                    "deg_segment_angle",
                    &mut self.s.switches.segment_angle,
                );
                self.fields.length_row(
                    ui,
                    "Minimum Alcove Size",
                    "min_alcove",
                    &mut self.s.switches.min_alcove,
                );
                self.fields.length_row(
                    ui,
                    "Raise Roof Off Plate",
                    "raise",
                    &mut self.s.raise_off_plate,
                );
                self.height_group(ui);
                ui.add_space(6.0);
                ui.weak(&self.note);
            }
            1 => {
                section(ui, "Framing");
                let mut v = self.s.build_framing;
                if ui
                    .checkbox(&mut v, "Build Framing")
                    .on_hover_text("Stored; roof framing is not generated yet")
                    .changed()
                {
                    self.s.build_framing = v;
                }
                let h = &mut self.s.heights;
                ui.horizontal(|ui| {
                    ui.label("Framing Method");
                    ui.radio_value(&mut h.framing, plan_roof::RoofFraming::Rafters, "Rafters");
                    ui.radio_value(&mut h.framing, plan_roof::RoofFraming::Trusses, "Trusses");
                });
                ui.weak("Roof framing is a placeholder until plan-framing exists.");
                section(ui, "3D Display");
                ui.checkbox(&mut self.s.switches.show_all_ridges, "Show All Ridges")
                    .on_hover_text(
                        "Draw a line along each hip over a curved wall in plan and vector views",
                    );
            }
            2 => {
                section(ui, "Roofing");
                row(ui, "Material", |ui| {
                    material_combo(ui, "build_roof_material", &mut self.s.material);
                });
            }
            _ => detail_form(ui, &mut self.fields, &mut self.s.detail, &[]),
        }
    }

    fn preview(&self, p: &Painter, area: Rect) {
        // The roof from above: outline, ridge and hips of the chosen style.
        let r = Rect::from_center_size(area.center(), area.size() * egui::vec2(0.8, 0.5));
        let ink = Stroke::new(1.5_f32, PV_INK);
        let line = Stroke::new(1.0_f32, PV_ACCENT);
        p.rect_stroke(r, 0.0, ink, egui::StrokeKind::Inside);
        let inset = r.height() * 0.5;
        let style = self.style.unwrap_or(RoofStyle::Hip);
        let (l, rr) = match style {
            // Hip: the ridge is shortened by the hips at both ends.
            RoofStyle::Hip | RoofStyle::Gambrel => (
                Pos2::new(r.min.x + inset, r.center().y),
                Pos2::new(r.max.x - inset, r.center().y),
            ),
            // Half hip and Dutch gable: a short hip at each end.
            RoofStyle::HalfHip | RoofStyle::DutchGable => (
                Pos2::new(r.min.x + inset * 0.4, r.center().y),
                Pos2::new(r.max.x - inset * 0.4, r.center().y),
            ),
            RoofStyle::Gable => (
                Pos2::new(r.min.x, r.center().y),
                Pos2::new(r.max.x, r.center().y),
            ),
            RoofStyle::Shed => (
                Pos2::new(r.min.x, r.min.y + 1.0),
                Pos2::new(r.max.x, r.min.y + 1.0),
            ),
        };
        p.line_segment([l, rr], ink);
        match style {
            RoofStyle::Hip | RoofStyle::HalfHip | RoofStyle::DutchGable | RoofStyle::Gambrel => {
                for (c, e) in [
                    (l, r.left_top()),
                    (l, r.left_bottom()),
                    (rr, r.right_top()),
                    (rr, r.right_bottom()),
                ] {
                    p.line_segment([c, e], line);
                }
            }
            RoofStyle::Gable | RoofStyle::Shed => {}
        }
        if style == RoofStyle::Gambrel {
            // The break line of the steeper lower slopes.
            for y in [r.min.y + r.height() * 0.25, r.max.y - r.height() * 0.25] {
                p.line_segment([Pos2::new(l.x, y), Pos2::new(rr.x, y)], line);
            }
        }
        p.text(
            Pos2::new(area.center().x, r.max.y + 18.0),
            Align2::CENTER_CENTER,
            match self.style {
                Some(st) => format!("{}, {}", st.label(), pitch_label(self.s.pitch)),
                None => pitch_label(self.s.pitch),
            },
            FontId::proportional(13.0),
            PV_INK,
        );
    }
}

impl BuildPages {
    /// Make Roof Baseline Polylines, the two Retain switches and Use Existing
    /// Roof Baselines (RF-70, RF-71): the Build group of the Roof panel.
    fn build_switches(&mut self, ui: &mut Ui) {
        let planes = self.s.build_planes;
        let sw = &mut self.s.switches;
        // Make Roof Baseline Polylines is not available with Build Roof Planes.
        ui.add_enabled_ui(!planes, |ui| {
            ui.checkbox(&mut sw.make_baselines, "Make Roof Baseline Polylines")
                .on_hover_text(
                    "Delete the roof and make baseline polylines along the outside of the exterior walls",
                );
        });
        if planes {
            sw.make_baselines = false;
        }
        // The retain boxes work with Build Roof Planes or with Make Roof
        // Baseline Polylines.
        ui.add_enabled_ui(planes || sw.make_baselines, |ui| {
            ui.checkbox(&mut sw.retain_manual, "Retain Manually Drawn Roof Planes")
                .on_hover_text("A rebuild keeps the planes drawn by hand");
            ui.checkbox(&mut sw.retain_edited, "Retain Edited Automatic Roof Planes")
                .on_hover_text(
                    "A rebuild keeps automatic planes that were moved or re-pitched; a new plane coplanar with one is dropped",
                );
        });
        ui.add_enabled_ui(planes, |ui| {
            ui.checkbox(
                &mut sw.use_existing_baselines,
                "Use Existing Roof Baselines",
            )
            .on_hover_text(
                "Build the planes from the roof baseline polylines instead of the walls",
            );
        });
    }
}

/// What a style writes to the walls, for the Build Roof dialog.
fn style_note(style: RoofStyle) -> &'static str {
    match style {
        RoofStyle::Hip => "Every exterior wall becomes a Hip Wall.",
        RoofStyle::Gable => "The walls across the ridge become Full Gable Walls.",
        RoofStyle::Shed => {
            "One long wall becomes the High Shed/Gable Wall; the ends are Full Gable Walls."
        }
        RoofStyle::Gambrel => {
            "Gable ends; the long walls get a steep lower pitch and a shallow upper pitch."
        }
        RoofStyle::DutchGable => "The walls across the ridge become Dutch Gable Walls.",
        RoofStyle::HalfHip => "Gable ends whose peak is clipped by a small hip (Starts at Height).",
    }
}

/// Build Roof dialog (one click on the tool opens it).
pub struct BuildRoofDialog {
    frame: SpecDialog,
    pages: BuildPages,
}

impl BuildRoofDialog {
    /// `note` tells which floor the roof is built over.
    pub fn new(settings: RoofSettings, note: impl Into<String>) -> Self {
        Self {
            frame: SpecDialog::new("Build Roof", "build_roof"),
            pages: BuildPages {
                s: settings,
                fields: Fields::default(),
                note: note.into(),
                style: None,
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.pages)
    }

    pub fn settings(&self) -> &RoofSettings {
        &self.pages.s
    }

    /// The Roof Styles button chosen, if any (RF-3).
    pub fn style(&self) -> Option<RoofStyle> {
        self.pages.style
    }

    /// Presses a style button as a click would (tests and the shell).
    pub fn set_style(&mut self, style: Option<RoofStyle>) {
        self.pages.style = style;
    }
}

// ===================================================================
// Roof Plane Specification
// ===================================================================

const PLANE_TABS: &[Tab] = &[
    on("General"),
    on("Holes"),
    on("Build Roof Edge"),
    on("Options"),
    on("Structure"),
    on("Materials"),
    on("Layer"),
    on("Label"),
    on("Line Style"),
    on("Fill Style"),
    on("Arrow"),
    on("Polyline"),
    on("Schedule"),
];

/// Field key of a height lock's length field.
fn lock_key(lock: HeightLock) -> &'static str {
    match lock {
        HeightLock::RidgeTop => "plane_ridge_top",
        HeightLock::Baseline => "baseline",
        HeightLock::FasciaTop => "plane_fascia_top",
        HeightLock::ShadowBoardTop => "plane_shadow_top",
        HeightLock::TopOfPlate => "plane_top_of_plate",
    }
}

/// The Shadow Board Top row of the General panel, listed after the four
/// heights of `LOCKS` when the roof has shadow boards.
const SHADOW_LOCK: (HeightLock, &str) = (HeightLock::ShadowBoardTop, "Shadow Board Top Height");

/// Where the pivot of `lock` sits in the lock diagram, as fractions of its
/// box (x to the right, y down): the side of a wall with the plane rising
/// from the eave at the left to the ridge at the right (RF-117).
pub fn lock_point(lock: HeightLock, trusses: bool) -> (f32, f32) {
    match lock {
        HeightLock::RidgeTop => (0.90, 0.14),
        HeightLock::Baseline => (0.30, 0.50),
        HeightLock::FasciaTop => (0.08, 0.62),
        HeightLock::ShadowBoardTop => (0.08, 0.54),
        // Rafters bear on the inside edge of the plate with the automatic
        // birdsmouth; trusses sit on the plate at the heel.
        HeightLock::TopOfPlate => {
            if trusses {
                (0.30, 0.70)
            } else {
                (0.38, 0.70)
            }
        }
    }
}

/// The diagram beside the Height/Pitch settings: the wall, the plate, the
/// plane with its structure, and a dot on the point the chosen lock holds.
fn lock_diagram(ui: &mut Ui, lock: HeightLock, trusses: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(180.0, 96.0), egui::Sense::hover());
    let p = ui.painter_at(rect);
    let at = |x: f32, y: f32| {
        Pos2::new(
            rect.left() + x * rect.width(),
            rect.top() + y * rect.height(),
        )
    };
    let ink = Stroke::new(1.5_f32, PV_INK);
    // Wall and top plate.
    p.line_segment([at(0.30, 0.70), at(0.30, 1.0)], ink);
    p.line_segment([at(0.45, 0.70), at(0.45, 1.0)], ink);
    p.rect_stroke(
        Rect::from_min_max(at(0.30, 0.66), at(0.45, 0.70)),
        0.0,
        ink,
        egui::StrokeKind::Inside,
    );
    // Top surface of the plane, eave to ridge, and its underside.
    p.line_segment([at(0.08, 0.62), at(0.90, 0.14)], ink);
    p.line_segment(
        [at(0.08, 0.68), at(0.90, 0.20)],
        Stroke::new(1.0_f32, PV_INK),
    );
    p.line_segment([at(0.08, 0.62), at(0.08, 0.68)], ink);
    if trusses {
        // A truss: bottom chord and a web.
        p.line_segment(
            [at(0.30, 0.68), at(0.90, 0.20)],
            Stroke::new(0.8_f32, PV_INK),
        );
        p.line_segment(
            [at(0.30, 0.68), at(0.62, 0.20)],
            Stroke::new(0.8_f32, PV_INK),
        );
    }
    let (x, y) = lock_point(lock, trusses);
    p.circle_filled(at(x, y), 4.0, PV_ACCENT);
    ui.weak(if trusses {
        "Diagram: trusses. The dot marks the locked height."
    } else {
        "Diagram: rafters. The dot marks the locked height."
    });
}

struct PlanePages {
    draft: RoofPlaneRecord,
    layers: Vec<String>,
    fields: Fields,
    /// The structure Roof Defaults give a plane, which Define starts from.
    default_structure: RoofStructure,
    /// The Define Roof Structure window while it is open (the all-planes
    /// sheet keeps it; a single plane uses the layer definitions below).
    define: Option<RoofStructure>,
    /// The Material Layers Definition window of a single plane's Roof
    /// Surface, Roof Structure or Roof Ceiling Finish while it is open.
    layers_edit: Option<AssemblyDefDialog>,
    /// The height a pitch change keeps fixed (RF-104, manual p. 840).
    lock: HeightLock,
    /// The roof's Roof Height settings: framing and birdsmouth.
    heights: HeightSettings,
    /// How typed edge lengths are read (RF-97).
    entry: LengthEntry,
    /// The roof's trim has shadow boards, so the Shadow Board Top height
    /// applies (manual p. 846).
    shadow_boards: bool,
}

impl PlanePages {
    fn new(draft: RoofPlaneRecord, layers: Vec<String>) -> Self {
        Self {
            draft,
            layers,
            fields: Fields::default(),
            default_structure: RoofStructure::default(),
            define: None,
            layers_edit: None,
            lock: HeightLock::Baseline,
            heights: HeightSettings::default(),
            entry: LengthEntry::Projected,
            shadow_boards: false,
        }
    }

    /// Structure thickness square to the slope: the plane's own structure,
    /// else the Roof Defaults'.
    fn thickness(&self) -> f64 {
        self.draft
            .structure
            .unwrap_or(self.default_structure)
            .thickness()
    }

    fn plane_heights(&self) -> plan_roof::PlaneHeights {
        self.draft.plane_heights(self.thickness())
    }

    /// A pitch typed in the specification: the plane pivots about the locked
    /// height.
    fn set_pitch_locked(&mut self, pitch: f64) {
        let h = self
            .plane_heights()
            .with_pitch(pitch, self.lock, self.heights.auto_birdsmouth);
        self.draft.apply_heights(&h);
    }

    /// A height typed in the specification: the plane keeps its pitch and
    /// moves (or, for Top of Plate, the plate does).
    fn set_height(&mut self, lock: HeightLock, value: f64) {
        let h = self.plane_heights().with_height(lock, value);
        self.draft.apply_heights(&h);
    }

    /// The General panel (RF-36, RF-115..RF-118).
    fn general_page(&mut self, ui: &mut Ui) {
        section(ui, "3D Orientation");
        let mut pitch = self.draft.pitch;
        pitch_row(ui, "Pitch", &mut pitch);
        if (pitch - self.draft.pitch).abs() > 1e-9 {
            self.set_pitch_locked(pitch);
        }
        ui.weak(
            "The radio button picks the height a pitch change keeps fixed. Heights are elevations.",
        );
        let shown = self.plane_heights();
        for (lock, label) in LOCKS {
            let mut v = shown.height(lock);
            let key = lock_key(lock);
            let radio = &mut self.lock;
            let fields = &mut self.fields;
            if row(ui, label, |ui| {
                ui.radio_value(radio, lock, "");
                fields.length(ui, key, &mut v)
            }) {
                self.set_height(lock, v);
            }
        }
        if self.shadow_boards {
            let (lock, label) = SHADOW_LOCK;
            let mut v = shown.height(lock);
            let radio = &mut self.lock;
            let fields = &mut self.fields;
            if row(ui, label, |ui| {
                ui.radio_value(radio, lock, "");
                fields.length(ui, lock_key(lock), &mut v)
            }) {
                self.set_height(lock, v);
            }
        } else if self.lock == HeightLock::ShadowBoardTop {
            self.lock = HeightLock::Baseline;
        }
        lock_diagram(
            ui,
            self.lock,
            self.heights.framing == plan_roof::RoofFraming::Trusses,
        );
        section(ui, "Measurements");
        let ph = self.plane_heights();
        row(ui, "Structure Thickness", |ui| {
            ui.label(fmt_short(ph.thickness));
        });
        row(ui, "Vertical Structure Depth", |ui| {
            ui.label(fmt_short(ph.vertical_depth()));
        });
        let depth = ph.birdsmouth_depth();
        if self.heights.framing == plan_roof::RoofFraming::Trusses {
            row(ui, "Birdsmouth", |ui| {
                ui.label("None (trusses)");
            });
            row(ui, "Heel Height", |ui| {
                ui.label(fmt_short(self.heights.heel_height));
            });
        } else if -depth >= NO_BIRDSMOUTH_RAISE {
            row(ui, "Birdsmouth", |ui| {
                ui.label(format!("None (raised {} off the plate)", fmt_short(-depth)));
            });
        } else {
            row(ui, "Birdsmouth Depth", |ui| {
                ui.label(fmt_short(depth.max(0.0)));
            });
            row(ui, "Birdsmouth Seat", |ui| {
                ui.label(fmt_short(ph.birdsmouth_seat().max(0.0)));
            });
        }
        row(ui, "Overhang from Baseline", |ui| {
            ui.label(fmt_short(ph.overhang));
        });
        row(ui, "Surface Area", |ui| {
            ui.label(format!("{:.1} sq ft", self.draft.area() / 144.0));
        });
        section(ui, "Options");
        let mut edited = !self.draft.auto;
        let can_clear = self.draft.source.is_some();
        if ui
            .add_enabled(
                can_clear || self.draft.auto,
                egui::Checkbox::new(&mut edited, "Mark as Edited"),
            )
            .on_hover_text("An edited plane is kept when the roof is rebuilt")
            .changed()
        {
            self.draft.auto = !edited;
        }
        ui.checkbox(&mut self.draft.special_snapping, "Use Special Snapping")
            .on_hover_text("Plane edges snap to the outside of a parallel wall nearby");
        super::roof_baseline::curved_section(ui, &mut self.draft, &mut self.fields);
        section(ui, "Vertices");
        egui::Grid::new("roof_vertices")
            .striped(true)
            .show(ui, |ui| {
                ui.strong("#");
                ui.strong("X");
                ui.strong("Y");
                ui.strong("Elevation");
                ui.end_row();
                for (i, v) in self.draft.polygon3d.iter().enumerate() {
                    ui.label(format!("{}", i + 1));
                    ui.label(fmt_short(v[0]));
                    ui.label(fmt_short(-v[2]));
                    ui.label(fmt_short(v[1]));
                    ui.end_row();
                }
            });
    }

    /// Line Style panel (RF-88).
    fn line_style_page(&mut self, ui: &mut Ui) {
        section(ui, "Line Style");
        let st = &mut self.draft.style;
        let mut own = st.line_color.is_some();
        if ui.checkbox(&mut own, "Own line color").changed() {
            st.line_color = own.then_some([0, 0, 0]);
        }
        if let Some(c) = st.line_color.as_mut() {
            row(ui, "Color", |ui| ui.color_edit_button_srgb(c));
        } else {
            ui.weak("Drawn in the color of the plane's layer.");
        }
        row(ui, "Line Weight", |ui| {
            ui.add(
                egui::DragValue::new(&mut st.line_weight)
                    .range(0.25..=8.0)
                    .speed(0.05)
                    .max_decimals(2)
                    .suffix(" x"),
            )
        });
        row(ui, "Dash", |ui| {
            for d in [
                LineStyle::Solid,
                LineStyle::Dashed,
                LineStyle::Dotted,
                LineStyle::DashDot,
            ] {
                ui.radio_value(&mut st.dash, d, format!("{d:?}"));
            }
        });
    }

    /// Fill Style panel (RF-89).
    fn fill_style_page(&mut self, ui: &mut Ui) {
        section(ui, "Fill Style");
        let f = &mut self.draft.style.fill;
        row(ui, "Fill", |ui| {
            for k in FillKind::ALL {
                ui.radio_value(&mut f.kind, k, k.label());
            }
        });
        if f.kind != FillKind::None {
            row(ui, "Color", |ui| ui.color_edit_button_srgb(&mut f.color));
            row(ui, "Opacity", |ui| {
                ui.add(egui::Slider::new(&mut f.opacity, 0..=255).show_value(false))
            });
        }
        if f.kind == FillKind::Hatch {
            self.fields
                .length_row(ui, "Line Spacing", "plane_hatch", &mut f.spacing);
            f.spacing = f.spacing.max(1.0);
        }
        ui.weak("Drawn inside the plane's outline in the plan view.");
    }

    /// Arrow panel (RF-91): the slope arrow.
    fn arrow_page(&mut self, ui: &mut Ui) {
        section(ui, "Slope Arrow");
        let a = &mut self.draft.style.arrow;
        ui.checkbox(&mut a.show, "Show Slope Arrow");
        ui.checkbox(&mut a.show_text, "Show Pitch and Label");
        if a.show {
            let mut own = a.length > 0.0;
            if ui.checkbox(&mut own, "Specify Length").changed() {
                a.length = if own { 48.0 } else { 0.0 };
            }
            if own {
                self.fields
                    .length_row(ui, "Length", "plane_arrow_len", &mut a.length);
                a.length = a.length.max(1.0);
            }
            let mut color = a.color.is_some();
            if ui.checkbox(&mut color, "Own color").changed() {
                a.color = color.then_some([0, 0, 0]);
            }
            if let Some(c) = a.color.as_mut() {
                row(ui, "Color", |ui| ui.color_edit_button_srgb(c));
            }
        }
        ui.weak("The arrow points down the slope from the middle of the plane.");
    }

    /// Polyline panel (RF-92, RF-97): perimeter, areas and volume, and the
    /// edge lengths, typed as projected or actual.
    fn polyline_page(&mut self, ui: &mut Ui) {
        let rep = self.draft.report(self.thickness());
        section(ui, "Roof Plane Polyline");
        let sq = |a: f64| format!("{:.1} sq ft", a / 144.0);
        for (label, text) in [
            ("Perimeter (projected)", fmt_short(rep.perimeter_projected)),
            ("Perimeter (actual)", fmt_short(rep.perimeter_actual)),
            ("Surface Area", sq(rep.area_surface)),
            ("Projected Area", sq(rep.area_projected)),
            ("Framing Area", sq(rep.area_framing)),
            ("Volume", format!("{:.1} cu ft", rep.volume / 1728.0)),
        ] {
            row(ui, label, |ui| {
                ui.label(text);
            });
        }
        section(ui, "Edge Lengths");
        row(ui, "Enter lengths as", |ui| {
            for e in [LengthEntry::Projected, LengthEntry::Actual] {
                ui.radio_value(&mut self.entry, e, e.label());
            }
        });
        const KEYS: [&str; 12] = [
            "plane_edge0",
            "plane_edge1",
            "plane_edge2",
            "plane_edge3",
            "plane_edge4",
            "plane_edge5",
            "plane_edge6",
            "plane_edge7",
            "plane_edge8",
            "plane_edge9",
            "plane_edge10",
            "plane_edge11",
        ];
        #[allow(clippy::needless_range_loop)]
        for i in 0..self.draft.polygon3d.len().min(KEYS.len()) {
            let Some((plan, actual)) = self.draft.edge_lengths(i) else {
                continue;
            };
            let mut shown = match self.entry {
                LengthEntry::Projected => plan,
                LengthEntry::Actual => actual,
            };
            let label = format!("Edge {}", i + 1);
            let entry = self.entry;
            let fields = &mut self.fields;
            if row(ui, &label, |ui| fields.length(ui, KEYS[i], &mut shown)) {
                self.draft.set_edge_length(i, shown, entry);
            }
        }
        if self.draft.polygon3d.len() > KEYS.len() {
            ui.weak("Only the first twelve edges are listed.");
        }
    }

    /// Schedule panel (RF-101): the plane's row in the roof schedule.
    fn schedule_page(&mut self, ui: &mut Ui) {
        section(ui, "Roof Plane Schedule");
        ui.checkbox(
            &mut self.draft.in_schedule,
            "List this plane in the roof schedule",
        );
        let rep = self.draft.report(self.thickness());
        section(ui, "Schedule Row");
        egui::Grid::new("plane_schedule_row")
            .striped(true)
            .show(ui, |ui| {
                for h in ["Label", "Pitch", "Material", "Surface Area", "Layer"] {
                    ui.strong(h);
                }
                ui.end_row();
                ui.label(if self.draft.label.is_empty() {
                    "(none)"
                } else {
                    &self.draft.label
                });
                ui.label(self.draft.pitch_label());
                ui.label(&self.draft.material);
                ui.label(format!("{:.1} sq ft", rep.area_surface / 144.0));
                ui.label(&self.draft.layer);
                ui.end_row();
            });
        if !self.draft.in_schedule {
            ui.weak("Left out of the schedule.");
        }
    }
}

/// The Define Roof Structure window (RF-36): framing, member size and
/// spacing, layers and ceiling framing. Returns `true` once OK was pressed
/// (the structure is then in `st`) and `false`/`None` states are kept by the
/// caller: `Some(true)` OK, `Some(false)` Cancel, `None` still open.
fn define_window(ctx: &egui::Context, st: &mut RoofStructure, fields: &mut Fields) -> Option<bool> {
    let mut result = None;
    egui::Window::new("Define Roof Structure")
        .id(egui::Id::new("roof_define_structure"))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            section(ui, "Roof Framing");
            row(ui, "Framing", |ui| {
                egui::ComboBox::from_id_salt("define_framing")
                    .selected_text(st.framing.label())
                    .show_ui(ui, |ui| {
                        for f in RoofFraming::ALL {
                            ui.selectable_value(&mut st.framing, f, f.label());
                        }
                    });
            });
            fields.length_row(ui, "Member Width", "struct_width", &mut st.member_width);
            fields.length_row(ui, "Member Depth", "struct_depth", &mut st.member_depth);
            fields.length_row(ui, "Spacing On Center", "struct_spacing", &mut st.spacing);
            section(ui, "Layers Over the Framing");
            fields.length_row(ui, "Sheathing", "struct_sheathing", &mut st.sheathing);
            fields.length_row(ui, "Roofing", "struct_roofing", &mut st.roofing);
            section(ui, "Ceiling Framing");
            row(ui, "Ceiling", |ui| {
                egui::ComboBox::from_id_salt("define_ceiling")
                    .selected_text(st.ceiling.label())
                    .show_ui(ui, |ui| {
                        for c in CeilingFraming::ALL {
                            ui.selectable_value(&mut st.ceiling, c, c.label());
                        }
                    });
            });
            ui.add_space(6.0);
            ui.weak(format!("Total thickness {}", fmt_short(st.thickness())));
            let err = if fields.any_invalid() {
                Some("Enter a valid length".to_string())
            } else {
                st.error()
            };
            if let Some(e) = &err {
                ui.colored_label(egui::Color32::from_rgb(0xC0, 0x30, 0x30), e);
            }
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    result = Some(false);
                }
                if ui
                    .add_enabled(err.is_none(), egui::Button::new("OK"))
                    .clicked()
                {
                    result = Some(true);
                }
            });
        });
    result
}

impl PlanePages {
    /// The holes and skylights of the plane: sizes, skylight construction,
    /// and a Delete button each.
    fn holes_page(&mut self, ui: &mut Ui) {
        section(ui, "Holes and Skylights");
        if self.draft.holes.is_empty() {
            ui.weak("This plane has no holes. Use the Roof Hole and Skylight tools.");
            return;
        }
        let mut remove = None;
        for (i, h) in self.draft.holes.iter_mut().enumerate() {
            let (w, l) = h.size();
            ui.horizontal(|ui| {
                ui.strong(if h.is_skylight() { "Skylight" } else { "Hole" });
                ui.label(format!("{} x {}", fmt_short(w), fmt_short(l)));
                if ui.button("Delete").clicked() {
                    remove = Some(i);
                }
            });
            if let Some(spec) = &mut h.skylight {
                for (label, value, max) in [
                    ("Curb Height", &mut spec.curb_height, 48.0),
                    ("Glass Thickness", &mut spec.glass_thickness, 6.0),
                    ("Frame Width", &mut spec.frame_width, 12.0),
                ] {
                    row(ui, label, |ui| {
                        ui.add(
                            egui::DragValue::new(value)
                                .range(0.25..=max)
                                .speed(0.1)
                                .suffix(" in"),
                        );
                    });
                }
            }
            ui.add_space(4.0);
        }
        if let Some(i) = remove {
            self.draft.holes.remove(i);
        }
    }

    /// The layered definitions the plane has now: its own, or ones made from
    /// the structure it follows (its own numbers or Roof Defaults).
    fn roof_layers(&self) -> RoofLayers {
        self.draft.layers.clone().unwrap_or_else(|| {
            let st = self.draft.structure.unwrap_or(self.default_structure);
            RoofLayers::from_numbers(
                st.roofing,
                st.sheathing,
                st.member_depth,
                st.member_width,
                st.spacing,
                st.framing == RoofFraming::Trusses,
            )
        })
    }

    /// Structure panel (RF-36, Round 16): the plane's Roof Surface, Roof
    /// Structure and Roof Ceiling Finish, each a Material Layers Definition.
    fn structure_page(&mut self, ui: &mut Ui) {
        section(ui, "Structure");
        let shown = self.draft.structure.unwrap_or(self.default_structure);
        let layers = self.roof_layers();
        row(ui, "Source", |ui| {
            ui.label(if self.draft.structure.is_some() {
                "This plane"
            } else {
                "Roof Defaults"
            });
        });
        row(ui, "Framing", |ui| ui.label(shown.framing.label()));
        row(ui, "Members", |ui| {
            ui.label(format!(
                "{} x {} at {} o.c.",
                fmt_short(shown.member_width),
                fmt_short(shown.member_depth),
                fmt_short(shown.spacing)
            ))
        });
        row(ui, "Thickness", |ui| ui.label(fmt_short(shown.thickness())));
        row(ui, "Ceiling Framing", |ui| ui.label(shown.ceiling.label()));
        ui.add_space(6.0);
        let mut open = None;
        for kind in [
            AssemblyKind::RoofSurface,
            AssemblyKind::RoofStructure,
            AssemblyKind::RoofCeilingFinish,
        ] {
            row(ui, kind.label(), |ui| {
                let a = layers.get(kind);
                ui.label(format!(
                    "{} layer{}, {}",
                    a.layers.len(),
                    if a.layers.len() == 1 { "" } else { "s" },
                    fmt_short(a.total_thickness())
                ));
                if ui.button("Edit\u{2026}").clicked() {
                    open = Some(kind);
                }
            });
        }
        if let Some(kind) = open {
            self.layers_edit = Some(AssemblyDefDialog::new(
                kind,
                layers.get(kind).clone(),
                &AssemblyLibrary::default(),
            ));
        }
        if self.draft.structure.is_some() && ui.button("Use Roof Defaults").clicked() {
            self.draft.set_structure(None);
        }
        if let Some(mut d) = self.layers_edit.take() {
            match d.show(ui.ctx()) {
                Outcome::Open => self.layers_edit = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    let mut l = layers;
                    l.set(d.kind(), d.into_assembly());
                    self.draft.set_layers(Some(l), shown);
                }
            }
        }
    }

    /// The Build Roof overrides of the edge this plane rises from.
    fn edge_page(&mut self, ui: &mut Ui) {
        section(ui, "Edge (used by Build Roof)");
        if self.draft.source.is_none() {
            ui.weak("Only planes made by Build Roof rise from a wall edge.");
            return;
        }
        let mut pitch_on = self.draft.edge.pitch.is_some();
        let mut pitch = self.draft.edge.pitch.unwrap_or(self.draft.pitch);
        ui.horizontal(|ui| {
            ui.checkbox(&mut pitch_on, "Pitch");
            pitch_row(ui, "", &mut pitch);
        });
        self.draft.edge.pitch = pitch_on.then_some(pitch);
        let mut over_on = self.draft.edge.overhang.is_some();
        let mut over = self.draft.edge.overhang.unwrap_or(self.draft.overhang);
        ui.checkbox(&mut over_on, "Overhang");
        if over_on {
            self.fields
                .length_row(ui, "Overhang from wall face", "edge_overhang", &mut over);
        }
        self.draft.edge.overhang = over_on.then_some(over);
        ui.checkbox(
            &mut self.draft.edge.gable,
            "Gable end (no plane rises from this edge)",
        )
        .on_hover_text("Rebuilds the roof; the plane disappears");
        ui.add_space(6.0);
        ui.weak("OK rebuilds the automatic roof with these edge settings.");
    }
}

impl SpecPages for PlanePages {
    fn tabs(&self) -> &'static [Tab] {
        PLANE_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            Some("Enter a valid length".into())
        } else if self.draft.pitch < MIN_PITCH || self.draft.pitch > MAX_PITCH {
            Some("Pitch must be between 0.5 and 24 in 12".into())
        } else {
            None
        }
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match tab {
            0 => self.general_page(ui),
            1 => self.holes_page(ui),
            2 => self.edge_page(ui),
            3 => {
                section(ui, "Eaves and Ridge");
                ui.checkbox(&mut self.draft.ridge_caps, "Include Ridge Caps")
                    .on_hover_text(
                        "Ridge and hip caps along this plane (Roof Defaults must allow them)",
                    );
                row(ui, "Eave Cut", |ui| {
                    let shown = self
                        .draft
                        .eave
                        .eave_cut
                        .map_or("Roof Default", EaveCut::label);
                    egui::ComboBox::from_id_salt("plane_eave_cut")
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.draft.eave.eave_cut,
                                None,
                                "Roof Default",
                            );
                            for c in EaveCut::ALL {
                                ui.selectable_value(
                                    &mut self.draft.eave.eave_cut,
                                    Some(c),
                                    c.label(),
                                );
                            }
                        });
                });
                tri_state(
                    ui,
                    "plane_tails",
                    "Rafter Tails",
                    &mut self.draft.eave.rafter_tails,
                );
                tri_state(ui, "plane_fascia", "Fascia", &mut self.draft.eave.fascia);
                tri_state(ui, "plane_soffit", "Soffit", &mut self.draft.eave.soffit);
                tri_state(ui, "plane_frieze", "Frieze", &mut self.draft.eave.frieze);
                tri_state(ui, "plane_gutters", "Gutters", &mut self.draft.eave.gutters);
                self.draft.gutters = self.draft.eave.gutters == Some(true);
            }
            4 => self.structure_page(ui),
            5 => {
                section(ui, "Roofing");
                row(ui, "Material", |ui| {
                    material_combo(ui, "plane_material", &mut self.draft.material);
                });
            }
            6 => {
                section(ui, "Layer");
                row(ui, "Layer", |ui| {
                    egui::ComboBox::from_id_salt("plane_layer")
                        .selected_text(self.draft.layer.clone())
                        .show_ui(ui, |ui| {
                            for l in &self.layers {
                                ui.selectable_value(&mut self.draft.layer, l.clone(), l);
                            }
                        });
                });
            }
            7 => {
                section(ui, "Label");
                row(ui, "Label", |ui| {
                    ui.text_edit_singleline(&mut self.draft.label);
                });
                ui.weak("The pitch is always shown next to the label.");
            }
            8 => self.line_style_page(ui),
            9 => self.fill_style_page(ui),
            10 => self.arrow_page(ui),
            11 => self.polyline_page(ui),
            _ => self.schedule_page(ui),
        }
    }

    fn preview(&self, p: &Painter, area: Rect) {
        let poly = self.draft.plan_polygon();
        if poly.len() < 3 {
            return;
        }
        let (mut lo, mut hi) = (poly[0], poly[0]);
        for q in &poly {
            lo.x = lo.x.min(q.x);
            lo.y = lo.y.min(q.y);
            hi.x = hi.x.max(q.x);
            hi.y = hi.y.max(q.y);
        }
        let (w, h) = ((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0));
        let s = ((area.width() as f64 / w).min(area.height() as f64 * 0.8 / h)).max(1e-6);
        let c = area.center();
        let to = |q: plan_core::Point| {
            Pos2::new(
                c.x + ((q.x - (lo.x + hi.x) * 0.5) * s) as f32,
                c.y - ((q.y - (lo.y + hi.y) * 0.5) * s) as f32,
            )
        };
        let pts: Vec<Pos2> = poly.iter().map(|q| to(*q)).collect();
        p.add(egui::Shape::closed_line(
            pts.clone(),
            Stroke::new(1.5_f32, PV_INK),
        ));
        p.line_segment([pts[0], pts[1]], Stroke::new(3.0_f32, PV_ACCENT));
        p.text(
            Pos2::new(c.x, area.max.y - 8.0),
            Align2::CENTER_CENTER,
            pitch_label(self.draft.pitch),
            FontId::proportional(13.0),
            PV_INK,
        );
    }
}

/// Roof Plane Specification (RF-36).
pub struct RoofPlaneDialog {
    frame: SpecDialog,
    pages: PlanePages,
}

impl RoofPlaneDialog {
    pub fn new(record: RoofPlaneRecord, layers: Vec<String>) -> Self {
        let key = record.id;
        Self {
            frame: SpecDialog::new("Roof Plane Specification", ("roof_plane", key)),
            pages: PlanePages::new(record, layers),
        }
    }

    /// The roof's Roof Height settings: the framing and birdsmouth the
    /// General panel's lock arithmetic and read-outs follow.
    /// Shadow boards are on in the roof's trim: the Shadow Board Top height
    /// can be typed and locked.
    pub fn with_shadow_boards(mut self, on: bool) -> Self {
        self.pages.shadow_boards = on;
        self
    }

    pub fn with_heights(mut self, heights: HeightSettings) -> Self {
        self.pages.heights = heights;
        self
    }

    /// Structure > Define starts from the structure these Roof Defaults give.
    pub fn with_detail(mut self, detail: &plan_core::defaults::RoofDetailDefaults) -> Self {
        self.pages.default_structure = RoofStructure::from_detail(detail);
        self
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.pages)
    }

    /// The edited copy.
    pub fn draft(&self) -> &RoofPlaneRecord {
        &self.pages.draft
    }
}

// ===================================================================
// Edit All Roof Planes
// ===================================================================

const ALL_TABS: &[Tab] = &[
    on("General"),
    on("Options"),
    on("Structure"),
    on("Materials"),
];

/// One setting of Edit All Roof Planes: a "Change" check box and the control.
fn change_row(ui: &mut Ui, label: &str, on_: &mut bool, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.checkbox(on_, "Change");
        row(ui, label, |ui| {
            ui.add_enabled_ui(*on_, add);
        });
    });
}

struct AllPages {
    /// How many planes the change reaches.
    count: usize,
    layers: Vec<String>,
    fields: Fields,
    pitch: Option<f64>,
    overhang: Option<f64>,
    material: Option<String>,
    layer: Option<String>,
    ridge_caps: Option<bool>,
    eave_cut: Option<Option<EaveCut>>,
    rafter_tails: Option<Option<bool>>,
    fascia: Option<Option<bool>>,
    soffit: Option<Option<bool>>,
    frieze: Option<Option<bool>>,
    gutters: Option<Option<bool>>,
    structure: Option<Option<RoofStructure>>,
    default_structure: RoofStructure,
    define: Option<RoofStructure>,
}

impl AllPages {
    fn edit(&self) -> AllPlanesEdit {
        AllPlanesEdit {
            pitch: self.pitch,
            overhang: self.overhang,
            material: self.material.clone(),
            layer: self.layer.clone(),
            ridge_caps: self.ridge_caps,
            eave_cut: self.eave_cut,
            rafter_tails: self.rafter_tails,
            fascia: self.fascia,
            soffit: self.soffit,
            frieze: self.frieze,
            gutters: self.gutters,
            structure: self.structure,
        }
    }
}

impl SpecPages for AllPages {
    fn tabs(&self) -> &'static [Tab] {
        ALL_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            Some("Enter a valid length".into())
        } else if self
            .pitch
            .is_some_and(|p| !(MIN_PITCH..=MAX_PITCH).contains(&p))
        {
            Some("Pitch must be between 0.5 and 24 in 12".into())
        } else if self.overhang.is_some_and(|o| o < 0.0) {
            Some("The overhang cannot be negative".into())
        } else if self.edit().is_empty() {
            Some("Choose what to change".into())
        } else {
            None
        }
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match tab {
            0 => {
                section(ui, &format!("Edit All Roof Planes ({} planes)", self.count));
                let mut on_ = self.pitch.is_some();
                let mut pitch = self.pitch.unwrap_or(8.0);
                change_row(ui, "Pitch", &mut on_, |ui| {
                    ui.add(
                        egui::DragValue::new(&mut pitch)
                            .range(MIN_PITCH..=MAX_PITCH)
                            .speed(0.1)
                            .max_decimals(2)
                            .suffix(" : 12"),
                    );
                });
                self.pitch = on_.then_some(pitch);
                let mut on_ = self.overhang.is_some();
                let mut over = self.overhang.unwrap_or(16.0);
                ui.horizontal(|ui| {
                    ui.checkbox(&mut on_, "Change");
                    self.fields
                        .length_row(ui, "Overhang", "all_overhang", &mut over);
                });
                self.overhang = on_.then_some(over);
                ui.weak(
                    "Pitch and overhang rebuild the automatic planes so hips and ridges follow.",
                );
            }
            1 => {
                section(ui, "Eaves and Ridge");
                let mut caps_on = self.ridge_caps.is_some();
                let mut caps = self.ridge_caps.unwrap_or(false);
                change_row(ui, "Ridge Caps", &mut caps_on, |ui| {
                    ui.checkbox(&mut caps, "Include Ridge Caps");
                });
                self.ridge_caps = caps_on.then_some(caps);
                let mut on_ = self.eave_cut.is_some();
                let mut cut = self.eave_cut.flatten();
                change_row(ui, "Eave Cut", &mut on_, |ui| {
                    egui::ComboBox::from_id_salt("all_eave_cut")
                        .selected_text(cut.map_or("Roof Default", EaveCut::label))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut cut, None, "Roof Default");
                            for c in EaveCut::ALL {
                                ui.selectable_value(&mut cut, Some(c), c.label());
                            }
                        });
                });
                self.eave_cut = on_.then_some(cut);
                for (salt, label, slot) in [
                    ("all_tails", "Rafter Tails", &mut self.rafter_tails),
                    ("all_fascia", "Fascia", &mut self.fascia),
                    ("all_soffit", "Soffit", &mut self.soffit),
                    ("all_frieze", "Frieze", &mut self.frieze),
                    ("all_gutters", "Gutters", &mut self.gutters),
                ] {
                    let mut on_ = slot.is_some();
                    let mut v = slot.flatten();
                    change_row(ui, label, &mut on_, |ui| tri_combo(ui, salt, &mut v));
                    *slot = on_.then_some(v);
                }
            }
            2 => {
                section(ui, "Structure");
                let mut on_ = self.structure.is_some();
                let shown = self.structure.flatten().unwrap_or(self.default_structure);
                ui.checkbox(&mut on_, "Change the structure of every plane");
                if on_ && self.structure.is_none() {
                    self.structure = Some(Some(shown));
                } else if !on_ {
                    self.structure = None;
                }
                if self.structure.is_some() {
                    row(ui, "Framing", |ui| ui.label(shown.framing.label()));
                    row(ui, "Members", |ui| {
                        ui.label(format!(
                            "{} x {} at {} o.c.",
                            fmt_short(shown.member_width),
                            fmt_short(shown.member_depth),
                            fmt_short(shown.spacing)
                        ))
                    });
                    row(ui, "Thickness", |ui| ui.label(fmt_short(shown.thickness())));
                    ui.horizontal(|ui| {
                        if ui.button("Define...").clicked() {
                            self.define = Some(shown);
                        }
                        if ui.button("Use Roof Defaults").clicked() {
                            self.structure = Some(None);
                        }
                    });
                }
                if let Some(mut st) = self.define {
                    match define_window(ui.ctx(), &mut st, &mut self.fields) {
                        Some(true) => {
                            self.structure = Some(Some(st));
                            self.define = None;
                        }
                        Some(false) => self.define = None,
                        None => self.define = Some(st),
                    }
                }
            }
            _ => {
                section(ui, "Roofing");
                let mut on_ = self.material.is_some();
                let mut m = self
                    .material
                    .clone()
                    .unwrap_or_else(|| ROOF_MATERIALS[0].to_string());
                change_row(ui, "Material", &mut on_, |ui| {
                    material_combo(ui, "all_material", &mut m);
                });
                self.material = on_.then_some(m);
                section(ui, "Layer");
                let mut on_ = self.layer.is_some();
                let mut l = self.layer.clone().unwrap_or_default();
                change_row(ui, "Layer", &mut on_, |ui| {
                    egui::ComboBox::from_id_salt("all_layer")
                        .selected_text(l.clone())
                        .show_ui(ui, |ui| {
                            for name in &self.layers {
                                ui.selectable_value(&mut l, name.clone(), name);
                            }
                        });
                });
                self.layer = (on_ && !l.is_empty()).then_some(l);
            }
        }
    }

    fn preview(&self, p: &Painter, area: Rect) {
        // A roof from above with every plane marked.
        let r = Rect::from_center_size(area.center(), area.size() * egui::vec2(0.8, 0.5));
        let ink = Stroke::new(1.5_f32, PV_INK);
        p.rect_stroke(r, 0.0, ink, egui::StrokeKind::Inside);
        let inset = r.height() * 0.5;
        let (l, rr) = (
            Pos2::new(r.min.x + inset, r.center().y),
            Pos2::new(r.max.x - inset, r.center().y),
        );
        p.line_segment([l, rr], ink);
        for (c, e) in [
            (l, r.left_top()),
            (l, r.left_bottom()),
            (rr, r.right_top()),
            (rr, r.right_bottom()),
        ] {
            p.line_segment([c, e], Stroke::new(1.0_f32, PV_ACCENT));
        }
        p.text(
            Pos2::new(area.center().x, r.max.y + 18.0),
            Align2::CENTER_CENTER,
            format!("{} planes", self.count),
            FontId::proportional(13.0),
            PV_INK,
        );
    }
}

/// Edit All Roof Planes (RF-39): one dialog whose settings reach every plane
/// of the roof.
pub struct AllPlanesDialog {
    frame: SpecDialog,
    pages: AllPages,
}

impl AllPlanesDialog {
    pub fn new(count: usize, layers: Vec<String>) -> Self {
        Self {
            frame: SpecDialog::new("Edit All Roof Planes", "roof_all_planes"),
            pages: AllPages {
                count,
                layers,
                fields: Fields::default(),
                pitch: None,
                overhang: None,
                material: None,
                layer: None,
                ridge_caps: None,
                eave_cut: None,
                rafter_tails: None,
                fascia: None,
                soffit: None,
                frieze: None,
                gutters: None,
                structure: None,
                default_structure: RoofStructure::default(),
                define: None,
            },
        }
    }

    pub fn with_detail(mut self, detail: &plan_core::defaults::RoofDetailDefaults) -> Self {
        self.pages.default_structure = RoofStructure::from_detail(detail);
        self
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.pages)
    }

    /// What OK changes.
    pub fn edit(&self) -> AllPlanesEdit {
        self.pages.edit()
    }

    /// Sets the pitch to change, as typing in the dialog does (tests).
    pub fn set_pitch(&mut self, pitch: Option<f64>) {
        self.pages.pitch = pitch;
    }
}

// ===================================================================
// Ceiling Plane Specification
// ===================================================================

const CEILING_TABS: &[Tab] = &[on("General"), on("Line Style"), on("Layer")];

const LINE_STYLES: [(LineStyle, &str); 4] = [
    (LineStyle::Solid, "Solid"),
    (LineStyle::Dashed, "Dashed"),
    (LineStyle::Dotted, "Dotted"),
    (LineStyle::DashDot, "Dash Dot"),
];

struct CeilingPages {
    draft: CeilingRecord,
    layers: Vec<String>,
    fields: Fields,
}

impl SpecPages for CeilingPages {
    fn tabs(&self) -> &'static [Tab] {
        CEILING_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            Some("Enter a valid length".into())
        } else if self.draft.pitch < 0.0 || self.draft.pitch > MAX_PITCH {
            Some("Pitch must be between 0 and 24 in 12".into())
        } else if self.draft.thickness < 0.0 {
            Some("The thickness cannot be negative".into())
        } else {
            None
        }
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match tab {
            0 => {
                section(ui, "General");
                self.fields.length_row(
                    ui,
                    "Height at Baseline",
                    "ceiling_height",
                    &mut self.draft.height_at_baseline,
                );
                row(ui, "Pitch", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.draft.pitch)
                            .range(0.0..=MAX_PITCH)
                            .speed(0.1)
                            .max_decimals(2)
                            .suffix(" : 12"),
                    );
                });
                self.fields.length_row(
                    ui,
                    "Thickness",
                    "ceiling_thickness",
                    &mut self.draft.thickness,
                );
                row(ui, "Origin", |ui| {
                    ui.label(if self.draft.auto {
                        "Built by Build Roof (replaced when the roof is rebuilt)"
                    } else {
                        "Manual (kept when the roof is rebuilt)"
                    });
                });
            }
            1 => {
                section(ui, "Line Style");
                row(ui, "Line style", |ui| {
                    let current = LINE_STYLES
                        .iter()
                        .find(|(s, _)| *s == self.draft.line_style)
                        .map_or("Solid", |(_, n)| *n);
                    egui::ComboBox::from_id_salt("ceiling_line_style")
                        .selected_text(current)
                        .show_ui(ui, |ui| {
                            for (s, name) in LINE_STYLES {
                                ui.selectable_value(&mut self.draft.line_style, s, name);
                            }
                        });
                });
            }
            _ => {
                section(ui, "Layer");
                row(ui, "Layer", |ui| {
                    egui::ComboBox::from_id_salt("ceiling_layer")
                        .selected_text(self.draft.layer.clone())
                        .show_ui(ui, |ui| {
                            for l in &self.layers {
                                ui.selectable_value(&mut self.draft.layer, l.clone(), l);
                            }
                        });
                });
            }
        }
    }

    fn preview(&self, p: &Painter, area: Rect) {
        // Side view: a sloped line over the baseline.
        let ink = Stroke::new(1.5_f32, PV_INK);
        let a = Pos2::new(area.min.x + area.width() * 0.15, area.center().y + 20.0);
        let rise = (area.width() * 0.7) * (self.draft.pitch / 12.0).min(1.0) as f32;
        let b = Pos2::new(area.max.x - area.width() * 0.15, a.y - rise);
        p.line_segment([a, Pos2::new(b.x, a.y)], Stroke::new(1.0_f32, PV_ACCENT));
        p.line_segment([a, b], ink);
        p.text(
            Pos2::new(area.center().x, area.max.y - 8.0),
            Align2::CENTER_CENTER,
            format!("Ceiling {}", pitch_label(self.draft.pitch)),
            FontId::proportional(13.0),
            PV_INK,
        );
    }
}

/// Ceiling Plane Specification (RF-45): height at the baseline, pitch,
/// thickness, line style and layer of a vaulted ceiling plane.
pub struct CeilingDialog {
    frame: SpecDialog,
    pages: CeilingPages,
}

impl CeilingDialog {
    pub fn new(record: CeilingRecord, layers: Vec<String>) -> Self {
        let key = record.id;
        Self {
            frame: SpecDialog::new("Ceiling Plane Specification", ("ceiling_plane", key)),
            pages: CeilingPages {
                draft: record,
                layers,
                fields: Fields::default(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.pages)
    }

    /// The edited copy.
    pub fn draft(&self) -> &CeilingRecord {
        &self.pages.draft
    }

    pub fn draft_mut(&mut self) -> &mut CeilingRecord {
        &mut self.pages.draft
    }
}

// ===================================================================
// Dormer Specification
// ===================================================================

const DORMER_TABS: &[Tab] = &[on("General"), on("Roof"), on("Window")];

struct DormerPages {
    spec: DormerSpec,
    fields: Fields,
    window_on: bool,
    window: (f64, f64),
}

fn kind_name(k: DormerKind) -> &'static str {
    match k {
        DormerKind::Gable => "Gable",
        DormerKind::Shed => "Shed",
        DormerKind::Hip => "Hip",
    }
}

impl SpecPages for DormerPages {
    fn tabs(&self) -> &'static [Tab] {
        DORMER_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            Some("Enter a valid length".into())
        } else if self.spec.width < 6.0 {
            Some("The dormer must be at least 6 inches wide".into())
        } else if self.spec.wall_height < 6.0 {
            Some("The dormer walls must be at least 6 inches tall".into())
        } else if self.spec.pitch < MIN_PITCH || self.spec.pitch > MAX_PITCH {
            Some("Pitch must be between 0.5 and 24 in 12".into())
        } else if self.spec.overhang < 0.0 {
            Some("The overhang cannot be negative".into())
        } else {
            None
        }
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match tab {
            0 => {
                section(ui, "Dormer");
                row(ui, "Type", |ui| {
                    egui::ComboBox::from_id_salt("dormer_kind")
                        .selected_text(kind_name(self.spec.kind))
                        .show_ui(ui, |ui| {
                            for k in [DormerKind::Gable, DormerKind::Shed, DormerKind::Hip] {
                                ui.selectable_value(&mut self.spec.kind, k, kind_name(k));
                            }
                        });
                });
                self.fields
                    .length_row(ui, "Width", "dormer_width", &mut self.spec.width);
                self.fields.length_row(
                    ui,
                    "Wall Height",
                    "dormer_wall",
                    &mut self.spec.wall_height,
                );
                section(ui, "Position on the roof plane");
                self.fields.length_row(
                    ui,
                    "Along the Eave",
                    "dormer_along",
                    &mut self.spec.position_along_eave,
                );
                self.fields.length_row(
                    ui,
                    "Setback from Eave",
                    "dormer_setback",
                    &mut self.spec.setback_from_eave,
                );
            }
            1 => {
                section(ui, "Dormer Roof");
                pitch_row(ui, "Pitch", &mut self.spec.pitch);
                self.fields.length_row(
                    ui,
                    "Height to Ridge",
                    "dormer_ridge",
                    &mut self.spec.height_to_ridge,
                );
                ui.weak("A ridge higher than the walls sets the pitch; 0 uses the pitch above.");
                self.fields
                    .length_row(ui, "Overhang", "dormer_overhang", &mut self.spec.overhang);
                ui.weak("The roof reaches this far past the walls; the Roof Defaults give it fascia and soffit.");
            }
            _ => {
                section(ui, "Window");
                ui.checkbox(&mut self.window_on, "Window in the front wall");
                if self.window_on {
                    self.fields
                        .length_row(ui, "Width", "dormer_win_w", &mut self.window.0);
                    self.fields
                        .length_row(ui, "Height", "dormer_win_h", &mut self.window.1);
                }
                self.spec.window = self.window_on.then_some(self.window);
            }
        }
    }

    fn preview(&self, p: &Painter, area: Rect) {
        // Front elevation: walls and the roof shape.
        let w = area.width() * 0.7;
        let h = area.height() * 0.3;
        let base = Pos2::new(area.center().x, area.center().y + h * 0.5);
        let left = Pos2::new(base.x - w * 0.5, base.y);
        let right = Pos2::new(base.x + w * 0.5, base.y);
        let ink = Stroke::new(1.5_f32, PV_INK);
        p.rect_stroke(
            Rect::from_two_pos(left, Pos2::new(right.x, base.y - h)),
            0.0,
            ink,
            egui::StrokeKind::Inside,
        );
        let eave_l = Pos2::new(left.x, base.y - h);
        let eave_r = Pos2::new(right.x, base.y - h);
        match self.spec.kind {
            DormerKind::Gable | DormerKind::Hip => {
                let ridge = Pos2::new(base.x, base.y - h - area.height() * 0.2);
                p.line_segment([eave_l, ridge], ink);
                p.line_segment([ridge, eave_r], ink);
            }
            DormerKind::Shed => {
                p.line_segment(
                    [eave_l, Pos2::new(eave_r.x, eave_r.y - area.height() * 0.1)],
                    ink,
                );
            }
        }
        if self.window_on {
            let c = Pos2::new(base.x, base.y - h * 0.5);
            p.rect_stroke(
                Rect::from_center_size(c, egui::vec2(w * 0.3, h * 0.5)),
                0.0,
                Stroke::new(1.0_f32, PV_ACCENT),
                egui::StrokeKind::Inside,
            );
        }
        p.text(
            Pos2::new(area.center().x, area.max.y - 8.0),
            Align2::CENTER_CENTER,
            format!("{} dormer", kind_name(self.spec.kind)),
            FontId::proportional(13.0),
            PV_INK,
        );
    }
}

/// Dormer Specification (RF-48): the dormer's dimensions. Used after an Auto
/// Dormer click and to edit a placed dormer.
pub struct DormerDialog {
    frame: SpecDialog,
    pages: DormerPages,
}

impl DormerDialog {
    pub fn new(spec: DormerSpec) -> Self {
        Self {
            frame: SpecDialog::new("Dormer Specification", "dormer"),
            pages: DormerPages {
                window_on: spec.window.is_some(),
                window: spec.window.unwrap_or((24.0, 36.0)),
                spec,
                fields: Fields::default(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.pages)
    }

    /// The edited dimensions.
    pub fn spec(&self) -> DormerSpec {
        self.pages.spec
    }

    /// Test access to the dimensions the Dormer Specification edits.
    #[cfg(test)]
    pub fn spec_mut(&mut self) -> &mut DormerSpec {
        &mut self.pages.spec
    }
}

// ----- Roof Return (RF-27) -----

const RETURN_TABS: &[Tab] = &[on("General")];
/// The shortest return the dialog accepts, inches.
pub const MIN_RETURN_LENGTH: f64 = 2.0;

struct ReturnPages {
    spec: ReturnSpec,
    fields: Fields,
}

fn return_kind_name(k: ReturnKind) -> &'static str {
    match k {
        ReturnKind::Full => "Full",
        ReturnKind::Half => "Half",
        ReturnKind::Boxed => "Boxed",
    }
}

impl SpecPages for ReturnPages {
    fn tabs(&self) -> &'static [Tab] {
        RETURN_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            Some("Enter a valid length".into())
        } else if self.spec.length < MIN_RETURN_LENGTH {
            Some("The return must be at least 2 inches long".into())
        } else {
            None
        }
    }

    fn page(&mut self, ui: &mut Ui, _tab: usize) {
        section(ui, "Roof Return");
        row(ui, "Type", |ui| {
            for k in [ReturnKind::Full, ReturnKind::Half, ReturnKind::Boxed] {
                ui.radio_value(&mut self.spec.kind, k, return_kind_name(k));
            }
        });
        self.fields
            .length_row(ui, "Length", "return_length", &mut self.spec.length);
        ui.weak("Used by the next clicks of the Roof Return tool. Shift makes a half return, Alt a boxed one.");
    }

    fn preview(&self, p: &Painter, area: Rect) {
        // Plan sketch: the eave, and the return wrapping the corner.
        let w = area.width() * 0.6;
        let corner = Pos2::new(area.center().x + w * 0.25, area.center().y);
        let len = (self.spec.length as f32 / 48.0).clamp(0.1, 1.0) * w * 0.4;
        let stroke = Stroke::new(1.5_f32, PV_INK);
        p.line_segment([Pos2::new(corner.x - w * 0.5, corner.y), corner], stroke);
        let tip = Pos2::new(corner.x + len, corner.y);
        let dash = Stroke::new(1.5_f32, PV_ACCENT);
        p.line_segment([corner, tip], dash);
        if self.spec.kind == ReturnKind::Boxed {
            p.line_segment([tip, Pos2::new(tip.x, tip.y + 14.0)], dash);
        }
        p.text(
            Pos2::new(area.center().x, area.min.y + 12.0),
            Align2::CENTER_CENTER,
            return_kind_name(self.spec.kind),
            FontId::proportional(11.0),
            PV_INK,
        );
    }
}

/// The Roof Return settings: type and length of the returns the tool makes.
pub struct ReturnDialog {
    frame: SpecDialog,
    pages: ReturnPages,
}

impl ReturnDialog {
    pub fn new(spec: ReturnSpec) -> Self {
        Self {
            frame: SpecDialog::new("Roof Return", "roof_return"),
            pages: ReturnPages {
                spec,
                fields: Fields::default(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.pages)
    }

    pub fn spec(&self) -> ReturnSpec {
        self.pages.spec
    }
}

// ===================================================================
// Set Baseline Height
// ===================================================================

const BASELINE_TABS: &[Tab] = &[on("General")];

struct BaselinePages {
    choice: BaselineOver,
    wall_top: f64,
    existing: f64,
}

impl SpecPages for BaselinePages {
    fn tabs(&self) -> &'static [Tab] {
        BASELINE_TABS
    }

    fn error(&self) -> Option<String> {
        None
    }

    fn page(&mut self, ui: &mut Ui, _tab: usize) {
        section(ui, "Set Baseline Height");
        ui.label("The baseline starts on an existing roof plane. Where does it sit?");
        ui.radio_value(
            &mut self.choice,
            BaselineOver::WallTop,
            format!("Over Wall Top ({})", fmt_short(self.wall_top)),
        );
        ui.weak("A full-height dormer rising from the wall below.");
        ui.radio_value(
            &mut self.choice,
            BaselineOver::ExistingPlane,
            format!(
                "Over the Existing Roof Plane ({})",
                fmt_short(self.existing)
            ),
        );
        ui.weak("A dormer vent or cricket sitting on the roof surface.");
    }

    fn preview(&self, p: &Painter, area: Rect) {
        // Side sketch: the wall, the roof plane, and the new baseline.
        let c = area.center();
        let w = area.width() * 0.7;
        let wall = Stroke::new(1.5_f32, PV_INK);
        let left = c.x - w * 0.5;
        p.line_segment(
            [Pos2::new(left, c.y + 30.0), Pos2::new(left, c.y - 10.0)],
            wall,
        );
        p.line_segment(
            [Pos2::new(left, c.y - 10.0), Pos2::new(left + w, c.y - 50.0)],
            wall,
        );
        let y = match self.choice {
            BaselineOver::WallTop => c.y - 10.0,
            BaselineOver::ExistingPlane => c.y - 28.0,
        };
        let x = c.x;
        p.line_segment(
            [Pos2::new(x - 24.0, y), Pos2::new(x + 24.0, y)],
            Stroke::new(3.0_f32, PV_ACCENT),
        );
    }
}

/// Set Baseline Height (RF-114, manual p. 844): shown when a new roof plane's
/// baseline starts on an existing plane.
pub struct BaselineHeightDialog {
    frame: SpecDialog,
    pages: BaselinePages,
}

impl BaselineHeightDialog {
    pub fn new(wall_top: f64, existing: f64) -> Self {
        Self {
            frame: SpecDialog::new("Set Baseline Height", "roof_baseline_height"),
            pages: BaselinePages {
                choice: BaselineOver::WallTop,
                wall_top,
                existing,
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.pages)
    }

    pub fn choice(&self) -> BaselineOver {
        self.pages.choice
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::Point;

    #[test]
    fn build_dialog_starts_from_the_settings() {
        let s = RoofSettings::fallback();
        let d = BuildRoofDialog::new(s.clone(), "Over 1st Floor");
        assert_eq!(d.settings(), &s);
        assert!(d.pages.error().is_none());
        assert_eq!(BUILD_TABS.len(), 4);
    }

    #[test]
    fn dormer_dialog_round_trips_its_spec_and_validates() {
        let spec = DormerSpec {
            window: Some((30.0, 40.0)),
            ..DormerSpec::default()
        };
        let mut d = DormerDialog::new(spec);
        assert_eq!(d.spec(), spec);
        assert!(d.pages.error().is_none());
        d.pages.spec.width = 2.0;
        assert!(d.pages.error().is_some());
        assert_eq!(DORMER_TABS.len(), 3);
    }

    #[test]
    fn return_dialog_round_trips_its_spec_and_validates() {
        let spec = ReturnSpec {
            kind: ReturnKind::Boxed,
            length: 36.0,
        };
        let mut d = ReturnDialog::new(spec);
        assert_eq!(d.spec(), spec);
        assert!(d.pages.error().is_none());
        d.pages.spec.length = 0.5;
        assert!(d.pages.error().is_some());
    }

    #[test]
    fn plane_dialog_rejects_bad_pitch() {
        let r = RoofPlaneRecord::new(
            1,
            vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 5.0, -10.0]],
            8.0,
            (Point::new(0.0, 0.0), Point::new(10.0, 0.0)),
        );
        let mut d = RoofPlaneDialog::new(r, vec!["Roof Planes".into()]);
        assert!(d.pages.error().is_none());
        d.pages.draft.pitch = 40.0;
        assert!(d.pages.error().is_some());
        assert_eq!(PLANE_TABS.len(), 13);
    }

    /// A 20 ft by 12 ft plane at 8:12 whose eave is at 100", overhanging 16".
    fn sample_plane() -> RoofPlaneRecord {
        let mut r = RoofPlaneRecord::new(
            1,
            vec![
                [0.0, 100.0, 0.0],
                [240.0, 100.0, 0.0],
                [240.0, 196.0, -144.0],
                [0.0, 196.0, -144.0],
            ],
            8.0,
            (Point::new(0.0, 0.0), Point::new(240.0, 0.0)),
        );
        r.overhang = 16.0;
        r.plate_top = Some(98.0);
        r.plate_width = 4.5;
        r
    }

    fn sample_dialog() -> RoofPlaneDialog {
        RoofPlaneDialog::new(sample_plane(), vec!["Roof Planes".into()])
    }

    #[test]
    fn the_general_panel_reads_the_four_heights_from_the_record() {
        let d = sample_dialog();
        let h = d.pages.plane_heights();
        // The baseline is the surface over the wall face: the eave tip plus
        // the overhang climbed at 8:12.
        assert!((h.baseline - (100.0 + 16.0 * 8.0 / 12.0)).abs() < 1e-9);
        assert!((h.fascia_top() - 100.0).abs() < 1e-9);
        assert!((h.ridge_top() - (h.baseline + 128.0 * 8.0 / 12.0)).abs() < 1e-9);
        assert!((h.plate_top - 98.0).abs() < 1e-9);
        assert!((h.run - 128.0).abs() < 1e-9, "run {}", h.run);
    }

    #[test]
    fn a_pitch_change_pivots_about_the_locked_height_in_the_dialog() {
        for lock in [
            HeightLock::RidgeTop,
            HeightLock::Baseline,
            HeightLock::FasciaTop,
            HeightLock::TopOfPlate,
        ] {
            let mut d = sample_dialog();
            d.pages.lock = lock;
            let before = d.pages.plane_heights();
            d.pages.set_pitch_locked(12.0);
            let after = d.pages.plane_heights();
            assert_eq!(d.pages.draft.pitch, 12.0);
            assert!(
                (after.height(lock) - before.height(lock)).abs() < 1e-9,
                "{lock:?} moved: {} to {}",
                before.height(lock),
                after.height(lock)
            );
            // The drawn polygon follows: its ridge sits at the new ridge.
            let top = d
                .pages
                .draft
                .polygon3d
                .iter()
                .map(|v| v[1])
                .fold(f64::MIN, f64::max);
            assert!((top - after.ridge_top()).abs() < 1e-6, "{lock:?}");
        }
    }

    #[test]
    fn typing_a_height_raises_the_plane_with_its_pitch_locked() {
        let mut d = sample_dialog();
        let before = d.pages.plane_heights();
        d.pages
            .set_height(HeightLock::RidgeTop, before.ridge_top() + 6.0);
        let after = d.pages.plane_heights();
        assert_eq!(d.pages.draft.pitch, 8.0);
        assert!((after.baseline - before.baseline - 6.0).abs() < 1e-9);
        // Raised 6" off the plate: the birdsmouth is 6" shallower.
        assert!((before.birdsmouth_depth() - after.birdsmouth_depth() - 6.0).abs() < 1e-9);
        // Top of Plate is the plate itself: the plane stays.
        let mut d = sample_dialog();
        d.pages.set_height(HeightLock::TopOfPlate, 99.0);
        assert_eq!(d.pages.draft.plate_top, Some(99.0));
        assert!((d.pages.draft.baseline_height() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn edge_lengths_are_typed_as_projected_or_actual() {
        let mut d = sample_dialog();
        // Edge 2 climbs: 240 across, 0 in plan... edge 1 is the side edge,
        // 144 in plan and 96 up.
        let (plan, actual) = d.pages.draft.edge_lengths(1).unwrap();
        assert!((plan - 144.0).abs() < 1e-9);
        assert!((actual - (144.0f64.powi(2) + 96.0f64.powi(2)).sqrt()).abs() < 1e-9);
        assert!(d
            .pages
            .draft
            .set_edge_length(1, 72.0, LengthEntry::Projected));
        assert!((d.pages.draft.edge_lengths(1).unwrap().0 - 72.0).abs() < 1e-6);
        // Typed along the slope.
        let (p, a) = d.pages.draft.edge_lengths(1).unwrap();
        assert!(d
            .pages
            .draft
            .set_edge_length(1, a / 2.0, LengthEntry::Actual));
        assert!((d.pages.draft.edge_lengths(1).unwrap().0 - p / 2.0).abs() < 1e-6);
        // The report adds up.
        let rep = d.pages.draft.report(6.0);
        assert!(rep.perimeter_actual > rep.perimeter_projected);
        assert!(rep.area_surface > rep.area_projected);
        assert!((rep.volume - rep.area_framing * 6.0).abs() < 1e-6);
    }

    #[test]
    fn the_new_panels_keep_their_choices_in_the_record() {
        let mut d = sample_dialog();
        let st = &mut d.pages.draft.style;
        st.dash = LineStyle::Dashed;
        st.fill.kind = FillKind::Hatch;
        st.arrow.show = false;
        d.pages.draft.in_schedule = false;
        let json = d.pages.draft.to_json_for_test();
        let back = RoofPlaneRecord::from_json_for_test(&json).unwrap();
        assert_eq!(back.style, d.pages.draft.style);
        assert!(!back.in_schedule);
        assert_eq!(back.plate_top, Some(98.0));
        // A plane that was never styled writes no style at all.
        assert!(sample_plane().to_json_for_test().get("style").is_none());
    }

    #[test]
    fn the_lock_diagram_puts_each_pivot_in_its_own_place() {
        let locks = [
            HeightLock::RidgeTop,
            HeightLock::Baseline,
            HeightLock::FasciaTop,
            HeightLock::ShadowBoardTop,
            HeightLock::TopOfPlate,
        ];
        for (i, a) in locks.iter().enumerate() {
            for b in &locks[i + 1..] {
                assert_ne!(lock_point(*a, false), lock_point(*b, false));
            }
        }
        // The plate pivot differs between rafters and trusses.
        assert_ne!(
            lock_point(HeightLock::TopOfPlate, false),
            lock_point(HeightLock::TopOfPlate, true)
        );
    }

    #[test]
    fn the_shadow_board_top_is_typed_and_locked_only_with_shadow_boards() {
        let mut d = sample_dialog().with_shadow_boards(true);
        d.pages.draft.shadow_rise = 1.5;
        let h = d.pages.plane_heights();
        assert!((h.shadow_board_top() - (h.fascia_top() + 1.5)).abs() < 1e-9);
        d.pages
            .set_height(HeightLock::ShadowBoardTop, h.shadow_board_top() + 6.0);
        let after = d.pages.plane_heights();
        assert!((after.shadow_board_top() - (h.shadow_board_top() + 6.0)).abs() < 1e-9);
        assert!((d.pages.draft.shadow_rise - 1.5).abs() < 1e-9);
        assert!(!sample_dialog().pages.shadow_boards);
    }
}
