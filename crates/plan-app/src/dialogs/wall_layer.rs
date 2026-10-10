//! Wall Layer Specification (Round 16, brief 12; manual p. 392): the dialog
//! "Edit Layer" in the Wall Type Definitions opens for one layer, with the
//! General, Line Style, Fill Style and Materials tabs.
//!
//! [`WallLayerDialog`] edits a copy of the layer; OK hands it back. The
//! pure rules (what a role allows, what an Exterior layer may extend) are in
//! `plan_core::wall_types` and are used here, not repeated.

use super::{fill_style, row, section, Outcome};
use eframe::egui::{self, Align2, Key, Modifiers, RichText};
use plan_core::assemblies::LayerRole;
use plan_core::defaults::WallLayer;
use plan_core::fill_styles::FillStyle;
use plan_core::wall_types::{LayerFraming, LayerGroup, MIN_LAYER};

/// The roles the Role drop-down offers, in Chief's order.
pub const ROLES: [LayerRole; 5] = [
    LayerRole::Framing,
    LayerRole::AirGap,
    LayerRole::Standard,
    LayerRole::Cladding,
    LayerRole::Finish,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    General,
    LineStyle,
    FillStyle,
    Materials,
}

const PAGES: [(Page, &str); 4] = [
    (Page::General, "General"),
    (Page::LineStyle, "Line Style"),
    (Page::FillStyle, "Fill Style"),
    (Page::Materials, "Materials"),
];

/// Wall Layer Specification for one layer of a wall type.
pub struct WallLayerDialog {
    id: egui::Id,
    layer: WallLayer,
    group: LayerGroup,
    page: Page,
    preview_width: f64,
}

impl WallLayerDialog {
    /// `group` is the section of the table the layer is in; only an Exterior
    /// layer can be extended.
    pub fn new(layer: WallLayer, group: LayerGroup, index: usize) -> Self {
        Self {
            id: egui::Id::new(("wall_layer_spec", index)),
            layer,
            group,
            page: Page::General,
            preview_width: 12.0,
        }
    }

    pub fn layer(&self) -> &WallLayer {
        &self.layer
    }

    /// Opens on the Fill Style tab (the Fill column of the table).
    pub fn on_fill_page(mut self) -> Self {
        self.page = Page::FillStyle;
        self
    }

    /// Why the layer cannot be accepted, if it cannot.
    pub fn error(&self) -> Option<String> {
        if self.layer.name.trim().is_empty() {
            return Some("The layer needs a name".into());
        }
        if self.layer.thickness < MIN_LAYER - 1e-9 && !self.layer.is_main {
            return Some("The layer needs a thickness".into());
        }
        if self.layer.is_main && self.layer.is_air_gap() {
            return Some("An air gap cannot be a main layer".into());
        }
        None
    }

    /// Picks the role (a Framing layer gets its framing options).
    pub fn set_role(&mut self, role: LayerRole) {
        self.layer.set_role(role);
    }

    /// Turns the Fill Style on (a Use Layer fill) or off.
    pub fn set_fill(&mut self, fill: Option<FillStyle>) {
        self.layer.spec.fill = fill;
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        let error = self.error();
        egui::Window::new("Wall Layer Specification")
            .id(self.id)
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([430.0, 420.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (p, label) in PAGES {
                        ui.selectable_value(&mut self.page, p, label);
                    }
                });
                ui.separator();
                match self.page {
                    Page::General => self.general(ui),
                    Page::LineStyle => self.line_style(ui),
                    Page::FillStyle => self.fill_page(ui),
                    Page::Materials => self.materials(ui),
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if let Some(e) = &error {
                        ui.colored_label(super::ERROR_RED, e);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let ok = egui::Button::new(RichText::new("   OK   ").strong());
                        if ui.add_enabled(error.is_none(), ok).clicked() {
                            outcome = Outcome::Ok;
                        }
                        if ui.button("Cancel").clicked() {
                            outcome = Outcome::Cancel;
                        }
                    });
                });
            });
        if !open {
            outcome = Outcome::Cancel;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            outcome = Outcome::Cancel;
        }
        outcome
    }

    fn general(&mut self, ui: &mut egui::Ui) {
        section(ui, "General");
        row(ui, "Name", |ui| {
            ui.text_edit_singleline(&mut self.layer.name)
        });
        row(ui, "Thickness", |ui| {
            ui.add(
                egui::DragValue::new(&mut self.layer.thickness)
                    .speed(0.0625)
                    .range(0.0..=48.0)
                    .suffix("\""),
            )
        });
        let mut role = self.layer.resolved_role();
        row(ui, "Role", |ui| {
            egui::ComboBox::from_id_salt("wl_role")
                .selected_text(role.label())
                .show_ui(ui, |ui| {
                    for r in ROLES {
                        ui.selectable_value(&mut role, r, r.label());
                    }
                })
        });
        if role != self.layer.resolved_role() {
            self.set_role(role);
        }
        let exterior = self.group == LayerGroup::Exterior;
        let mut extend = self.layer.spec.extension > 0.0;
        ui.add_enabled_ui(exterior, |ui| {
            if ui.checkbox(&mut extend, "Extend Layer").changed() {
                self.layer.spec.extension = if extend { 4.0 } else { 0.0 };
            }
            if extend {
                row(ui, "Extension", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.layer.spec.extension)
                            .speed(0.25)
                            .range(0.0..=240.0)
                            .suffix("\""),
                    )
                });
            }
        });
        ui.checkbox(
            &mut self.layer.spec.auto_detail_insulation,
            "Auto Detail as Insulation",
        );
        match self.layer.resolved_role() {
            LayerRole::Framing => self.framing_options(ui),
            LayerRole::Cladding => {
                section(ui, "3D Cladding");
                row(ui, "Profile", |ui| {
                    ui.text_edit_singleline(&mut self.layer.spec.cladding)
                });
            }
            _ => {}
        }
    }

    fn framing_options(&mut self, ui: &mut egui::Ui) {
        section(ui, "Framing");
        let f = self
            .layer
            .spec
            .framing
            .get_or_insert_with(LayerFraming::default);
        row(ui, "Stud Construction", |ui| {
            ui.text_edit_singleline(&mut f.stud_construction)
        });
        opt_inches(ui, "Stud Width", &mut f.stud_width, 3.5);
        opt_inches(ui, "Stud Spacing", &mut f.stud_spacing, 16.0);
        row(ui, "Top Plate", |ui| {
            ui.text_edit_singleline(&mut f.top_plate_construction)
        });
        opt_count(ui, "Top Plates", &mut f.top_plate_count, 2);
        row(ui, "Bottom Plate", |ui| {
            ui.text_edit_singleline(&mut f.bottom_plate_construction)
        });
        opt_count(ui, "Bottom Plates", &mut f.bottom_plate_count, 1);
        ui.checkbox(&mut f.horizontal, "Horizontal Framing (Girts)");
    }

    fn line_style(&mut self, ui: &mut egui::Ui) {
        section(ui, "Line Style");
        let line = &mut self.layer.spec.line;
        row(ui, "Display Layer", |ui| {
            let mut name = line.display_layer.clone().unwrap_or_default();
            if ui.text_edit_singleline(&mut name).changed() {
                line.display_layer = (!name.trim().is_empty()).then_some(name);
            }
        });
        let mut by_layer = line.color.is_none();
        if ui.checkbox(&mut by_layer, "Color By Layer").changed() {
            line.color = if by_layer { None } else { Some([0, 0, 0]) };
        }
        if let Some(c) = &mut line.color {
            row(ui, "Color", |ui| ui.color_edit_button_srgb(c));
        }
        row(ui, "Style", |ui| {
            let mut s = line.style.clone().unwrap_or_default();
            if ui.text_edit_singleline(&mut s).changed() {
                line.style = (!s.trim().is_empty()).then_some(s);
            }
        });
        let mut weight_by_layer = line.weight.is_none();
        if ui
            .checkbox(&mut weight_by_layer, "Weight By Layer")
            .changed()
        {
            line.weight = if weight_by_layer { None } else { Some(18) };
        }
        if let Some(w) = &mut line.weight {
            row(ui, "Weight", |ui| {
                ui.add(egui::DragValue::new(w).range(1..=200).suffix(" (0.01 mm)"))
            });
        }
    }

    fn fill_page(&mut self, ui: &mut egui::Ui) {
        section(ui, "Fill Style");
        let mut on = self.layer.spec.fill.is_some();
        if ui.checkbox(&mut on, "Fill the layer in plan").changed() {
            self.layer.spec.fill = on.then(FillStyle::use_layer);
        }
        if let Some(style) = &mut self.layer.spec.fill {
            fill_style::panel(
                ui,
                "wall_layer",
                style,
                &[],
                [200, 200, 200],
                &mut self.preview_width,
                false,
            );
        }
    }

    fn materials(&mut self, ui: &mut egui::Ui) {
        section(ui, "Materials");
        row(ui, "Material", |ui| {
            ui.text_edit_singleline(&mut self.layer.material)
        });
        if self.layer.resolved_role() == LayerRole::Framing {
            let f = self
                .layer
                .spec
                .framing
                .get_or_insert_with(LayerFraming::default);
            row(ui, "Studs", |ui| {
                ui.text_edit_singleline(&mut f.stud_material)
            });
            row(ui, "Plates", |ui| {
                ui.text_edit_singleline(&mut f.plate_material)
            });
            row(ui, "Treated Framing", |ui| {
                ui.text_edit_singleline(&mut f.treated_material)
            });
        }
    }
}

fn opt_inches(ui: &mut egui::Ui, label: &str, v: &mut Option<f64>, default: f64) {
    row(ui, label, |ui| {
        let mut follow = v.is_none();
        if ui.checkbox(&mut follow, "Defaults").changed() {
            *v = if follow { None } else { Some(default) };
        }
        if let Some(x) = v {
            ui.add(
                egui::DragValue::new(x)
                    .speed(0.25)
                    .range(0.0..=240.0)
                    .suffix("\""),
            );
        }
    });
}

fn opt_count(ui: &mut egui::Ui, label: &str, v: &mut Option<u32>, default: u32) {
    row(ui, label, |ui| {
        let mut follow = v.is_none();
        if ui.checkbox(&mut follow, "Defaults").changed() {
            *v = if follow { None } else { Some(default) };
        }
        if let Some(x) = v {
            ui.add(egui::DragValue::new(x).range(0..=6));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialog(role_main: bool) -> WallLayerDialog {
        let layer = WallLayer::new("Siding", 0.75, role_main, "Wood Siding");
        WallLayerDialog::new(layer, LayerGroup::Exterior, 0)
    }

    #[test]
    fn a_framing_role_brings_the_framing_options_and_another_role_drops_them() {
        let mut d = dialog(false);
        assert!(d.layer().spec.framing.is_none());
        d.set_role(LayerRole::Framing);
        assert!(d.layer().spec.framing.is_some());
        d.set_role(LayerRole::Cladding);
        assert!(d.layer().spec.framing.is_none());
        assert_eq!(d.layer().resolved_role(), LayerRole::Cladding);
    }

    #[test]
    fn the_layer_is_validated() {
        let mut d = dialog(false);
        assert!(d.error().is_none());
        d.layer.name = " ".into();
        assert!(d.error().is_some());
        d.layer.name = "Siding".into();
        d.layer.thickness = 0.0;
        assert!(d.error().is_some());
        d.layer.thickness = 0.75;
        let mut main = dialog(true);
        main.set_role(LayerRole::AirGap);
        assert!(main.error().is_some());
    }

    #[test]
    fn the_fill_follows_the_switch() {
        let mut d = dialog(false);
        assert!(d.layer().spec.fill.is_none());
        d.set_fill(Some(FillStyle::solid([10, 20, 30])));
        assert!(d.layer().spec.fill.is_some());
        d.set_fill(None);
        assert!(d.layer().spec.is_default());
    }
}
