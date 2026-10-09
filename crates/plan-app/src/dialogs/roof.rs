//! The Build Roof dialog (RF-1, RF-2), the Roof Plane Specification (RF-36,
//! with its holes list and Build Roof edge fields) and the Dormer
//! Specification (RF-48), on the shared dialog frame.
//!
//! Both edit a cloned draft; OK hands it back through `draft()`.

#![allow(dead_code)]

use super::{
    dis_check, fmt_short, on, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_ACCENT,
    PV_INK,
};
use crate::editor::roof_view::{
    pitch_label, AllPlanesEdit, CeilingFraming, CeilingRecord, RoofFraming, RoofPlaneRecord,
    RoofSettings, RoofStructure, RoofStyle, ROOF_MATERIALS,
};
use eframe::egui::{self, Align2, FontId, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::defaults::{EaveCut, RoofDetailDefaults};
use plan_core::LineStyle;
use plan_roof::{DormerKind, DormerSpec, ReturnKind, ReturnSpec};

const MIN_PITCH: f64 = 0.5;
const MAX_PITCH: f64 = 24.0;

fn pitch_row(ui: &mut Ui, label: &str, pitch: &mut f64) {
    row(ui, label, |ui| {
        ui.add(
            egui::DragValue::new(pitch)
                .range(MIN_PITCH..=MAX_PITCH)
                .speed(0.1)
                .max_decimals(2)
                .suffix(" : 12"),
        );
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

impl SpecPages for BuildPages {
    fn tabs(&self) -> &'static [Tab] {
        BUILD_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            Some("Enter a valid length".into())
        } else if self.s.pitch < MIN_PITCH || self.s.pitch > MAX_PITCH {
            Some("Pitch must be between 0.5 and 24 in 12".into())
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
                self.fields
                    .length_row(ui, "Overhang", "overhang", &mut self.s.overhang);
                self.fields.length_row(
                    ui,
                    "Raise Roof Off Plate",
                    "raise",
                    &mut self.s.raise_off_plate,
                );
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
                dis_check(ui, "Rafters", true);
                dis_check(ui, "Trusses", false);
                ui.weak("Roof framing is a placeholder until plan-framing exists.");
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
];

struct PlanePages {
    draft: RoofPlaneRecord,
    layers: Vec<String>,
    fields: Fields,
    /// The structure Roof Defaults give a plane, which Define starts from.
    default_structure: RoofStructure,
    /// The Define Roof Structure window while it is open.
    define: Option<RoofStructure>,
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

    /// Structure > Define (RF-36): what frames the plane.
    fn structure_page(&mut self, ui: &mut Ui) {
        section(ui, "Structure");
        let shown = self.draft.structure.unwrap_or(self.default_structure);
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
        ui.horizontal(|ui| {
            if ui.button("Define...").clicked() {
                self.define = Some(shown);
            }
            if self.draft.structure.is_some() && ui.button("Use Roof Defaults").clicked() {
                self.draft.set_structure(None);
            }
        });
        if let Some(mut st) = self.define {
            match define_window(ui.ctx(), &mut st, &mut self.fields) {
                Some(true) => {
                    self.draft.set_structure(Some(st));
                    self.define = None;
                }
                Some(false) => self.define = None,
                None => self.define = Some(st),
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
            0 => {
                section(ui, "General");
                let mut pitch = self.draft.pitch;
                pitch_row(ui, "Pitch", &mut pitch);
                if (pitch - self.draft.pitch).abs() > 1e-9 {
                    // Heights follow the new pitch right away.
                    self.draft.set_pitch(pitch);
                }
                let mut h = self.draft.baseline_height();
                if self
                    .fields
                    .length_row(ui, "Baseline Height", "baseline", &mut h)
                {
                    self.draft.set_baseline_height(h);
                }
                row(ui, "Overhang", |ui| {
                    ui.add_enabled(false, egui::Label::new(fmt_short(self.draft.overhang)));
                });
                row(ui, "Surface Area", |ui| {
                    ui.label(format!("{:.1} sq ft", self.draft.area() / 144.0));
                });
                row(ui, "Origin", |ui| {
                    ui.label(if self.draft.auto {
                        "Automatic (rebuilt with the walls)"
                    } else {
                        "Manual (kept when the roof is rebuilt)"
                    });
                });
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
            _ => {
                section(ui, "Label");
                row(ui, "Label", |ui| {
                    ui.text_edit_singleline(&mut self.draft.label);
                });
                ui.weak("The pitch is always shown next to the label.");
            }
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
            pages: PlanePages {
                draft: record,
                layers,
                fields: Fields::default(),
                default_structure: RoofStructure::default(),
                define: None,
            },
        }
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
        assert_eq!(PLANE_TABS.len(), 8);
    }
}
