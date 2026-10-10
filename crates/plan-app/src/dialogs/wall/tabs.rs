//! The Wall Specification tabs added in Round 15: Wall Covering (W-115),
//! Newels/Balusters (W-116), Rails (W-117), Materials, Components, Object
//! Information (W-118) and Schedule. The values are stored in `Wall.spec`
//! (`plan_core::walls::spec_tabs`); this file is the pages.

use super::{row, section, WallForm};
use eframe::egui::{self, Ui};
use plan_core::extras::MoldingKind;
use plan_core::rooms::{molding_def, molding_defs};
use plan_core::units::fmt_ft_in;
use plan_core::walls::{
    drawing_group_name, wall_components, BalusterStyle, CoveringSide, NewelStyle, RailFill,
    RailProfile, WallClass, WallMaterials, DEFAULT_DRAWING_GROUP, DRAWING_GROUPS,
};

/// Library materials a combo lists at most.
const MATERIAL_ROWS: usize = 60;

/// A combo of library materials: "None" first, then the matches of `filter`.
/// Returns the picked material and its color, or `Some(None)` for none.
#[allow(clippy::type_complexity)]
fn material_pick(
    ui: &mut Ui,
    salt: &str,
    current: &str,
    filter: &mut String,
    lib: &plan_materials::MaterialLibrary,
    none_label: &str,
) -> Option<Option<(String, [u8; 3])>> {
    let mut picked = None;
    let shown = if current.is_empty() {
        none_label.to_string()
    } else {
        current.to_string()
    };
    egui::ComboBox::from_id_salt(salt.to_string())
        .selected_text(shown)
        .width(240.0)
        .show_ui(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(filter)
                    .hint_text("Search materials")
                    .desired_width(220.0),
            );
            if ui
                .selectable_label(current.is_empty(), none_label)
                .clicked()
            {
                picked = Some(None);
            }
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .show(ui, |ui| {
                    for def in lib.search(filter).into_iter().take(MATERIAL_ROWS) {
                        if ui
                            .selectable_label(current == def.name, &def.name)
                            .clicked()
                        {
                            picked = Some(Some((def.name.clone(), def.color)));
                        }
                    }
                });
        });
    picked
}

/// A combo over the molding library entries of one kind, by name.
fn molding_combo(ui: &mut Ui, salt: &str, kind: MoldingKind, value: &mut String) {
    egui::ComboBox::from_id_salt(salt.to_string())
        .selected_text(if value.is_empty() {
            "None".to_string()
        } else {
            value.clone()
        })
        .width(220.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(value, String::new(), "None");
            for d in molding_defs(kind) {
                ui.selectable_value(value, d.name.to_string(), d.name);
            }
        });
    if let Some(d) = molding_def(value) {
        ui.weak(format!(
            "{} high, {} out",
            fmt_ft_in(d.height()),
            fmt_ft_in(d.projection())
        ));
    }
}

impl WallForm {
    /// Wall Covering tab (W-115): what each face wears.
    pub(super) fn wall_covering(&mut self, ui: &mut Ui) {
        ui.weak(
            "Coverings and moldings sit on the face of the wall, break at doors and \
             windows that reach into their height, and show in 3D.",
        );
        for side in CoveringSide::ALL {
            self.side_covering(ui, side);
        }
    }

    fn side_covering(&mut self, ui: &mut Ui, side: CoveringSide) {
        let int = side == CoveringSide::Interior;
        let pick = |a: &'static str, b: &'static str| if int { a } else { b };
        section(ui, &format!("{} Side", side.name()));
        let lib = self
            .material_lib
            .get_or_insert_with(crate::tools::materials::library)
            .clone();
        let c = self.draft.spec.covering.side_mut(side);
        row(ui, "Wall Covering", |ui| {
            let cur = c.covering.clone();
            if let Some(p) = material_pick(
                ui,
                pick("wc_cov_i", "wc_cov_e"),
                &cur,
                &mut self.material_filter,
                &lib,
                "None",
            ) {
                c.covering = p.map(|(n, _)| n).unwrap_or_default();
            }
        });
        if !c.covering.is_empty() {
            self.fields.length_row(
                ui,
                "Covering Thickness",
                pick("wc_cov_t_i", "wc_cov_t_e"),
                &mut c.covering_thickness,
            );
        }
        row(ui, "Wainscot", |ui| {
            let cur = c.wainscot.clone();
            if let Some(p) = material_pick(
                ui,
                pick("wc_wain_i", "wc_wain_e"),
                &cur,
                &mut self.material_filter,
                &lib,
                "None",
            ) {
                c.wainscot = p.map(|(n, _)| n).unwrap_or_default();
            }
        });
        if !c.wainscot.is_empty() {
            self.fields.length_row(
                ui,
                "Wainscot Height",
                pick("wc_wain_h_i", "wc_wain_h_e"),
                &mut c.wainscot_height,
            );
            self.fields.length_row(
                ui,
                "Wainscot Thickness",
                pick("wc_wain_t_i", "wc_wain_t_e"),
                &mut c.wainscot_thickness,
            );
        }
        row(ui, "Chair Rail", |ui| {
            molding_combo(
                ui,
                pick("wc_chair_i", "wc_chair_e"),
                MoldingKind::Chair,
                &mut c.chair_rail,
            )
        });
        if !c.chair_rail.is_empty() {
            self.fields.length_row(
                ui,
                "Chair Rail Height",
                pick("wc_chair_h_i", "wc_chair_h_e"),
                &mut c.chair_rail_height,
            );
        }
        row(ui, "Base Molding", |ui| {
            molding_combo(
                ui,
                pick("wc_base_i", "wc_base_e"),
                MoldingKind::Base,
                &mut c.base,
            )
        });
        row(ui, "Crown Molding", |ui| {
            molding_combo(
                ui,
                pick("wc_crown_i", "wc_crown_e"),
                MoldingKind::Crown,
                &mut c.crown,
            )
        });
        if !(c.chair_rail.is_empty() && c.base.is_empty() && c.crown.is_empty()) {
            row(ui, "Molding Material", |ui| {
                let cur = c.molding_material.clone();
                if let Some(p) = material_pick(
                    ui,
                    pick("wc_mmat_i", "wc_mmat_e"),
                    &cur,
                    &mut self.material_filter,
                    &lib,
                    "Trim",
                ) {
                    c.molding_material = p.map(|(n, _)| n).unwrap_or_default();
                }
            });
        }
    }

    /// Whether the draft is a railing wall (a Railing or Deck Railing class,
    /// or the Railing option).
    fn is_railing_wall(&self) -> bool {
        self.draft.class.is_railing()
            || self.draft.class == WallClass::Railing
            || self.draft.flags.railing
    }

    /// Newels/Balusters tab (W-116).
    pub(super) fn newels_balusters(&mut self, ui: &mut Ui) {
        if !self.is_railing_wall() {
            ui.weak("Draw a Railing or Deck Railing wall, or tick Railing on the General tab.");
            return;
        }
        let len = self.draft.length();
        section(ui, "Newels");
        {
            let r = &mut self.draft.spec.railing;
            row(ui, "Newel Style", |ui| {
                egui::ComboBox::from_id_salt("rail_newel_style")
                    .selected_text(r.newel_style.name())
                    .show_ui(ui, |ui| {
                        for s in NewelStyle::ALL {
                            ui.selectable_value(&mut r.newel_style, s, s.name());
                        }
                    });
            });
        }
        self.fields.length_row(
            ui,
            "Newel Size",
            "rail_newel_size",
            &mut self.draft.spec.railing.newel_size,
        );
        self.fields.length_row(
            ui,
            "Newel Height (0 = to the top rail)",
            "rail_newel_height",
            &mut self.draft.spec.railing.newel_height,
        );
        self.fields.length_row(
            ui,
            "Greatest Newel Spacing",
            "rail_newel_spacing",
            &mut self.draft.spec.railing.newel_spacing,
        );
        {
            let r = &mut self.draft.spec.railing;
            ui.checkbox(&mut r.newel_at_ends, "Newel at Each End");
            ui.checkbox(&mut r.newel_cap, "Newel Cap");
            ui.checkbox(&mut r.post_to_beam, "Post to Beam")
                .on_hover_text("The posts run down to the beam under the railing");
        }
        if self.draft.spec.railing.post_to_beam {
            self.fields.length_row(
                ui,
                "Beam Depth",
                "rail_beam_depth",
                &mut self.draft.spec.railing.beam_depth,
            );
        }
        section(ui, "Balusters and Panels");
        {
            let r = &mut self.draft.spec.railing;
            row(ui, "Between Newels", |ui| {
                egui::ComboBox::from_id_salt("rail_fill")
                    .selected_text(r.fill.name())
                    .show_ui(ui, |ui| {
                        for f in RailFill::ALL {
                            ui.selectable_value(&mut r.fill, f, f.name());
                        }
                    });
            });
        }
        if self.draft.spec.railing.fill == RailFill::Balusters {
            {
                let r = &mut self.draft.spec.railing;
                row(ui, "Baluster Style", |ui| {
                    egui::ComboBox::from_id_salt("rail_baluster_style")
                        .selected_text(r.baluster_style.name())
                        .show_ui(ui, |ui| {
                            for s in BalusterStyle::ALL {
                                ui.selectable_value(&mut r.baluster_style, s, s.name());
                            }
                        });
                });
            }
            self.fields.length_row(
                ui,
                "Baluster Size",
                "rail_baluster_size",
                &mut self.draft.spec.railing.baluster_size,
            );
            self.fields.length_row(
                ui,
                "Greatest Baluster Spacing",
                "rail_baluster_spacing",
                &mut self.draft.spec.railing.baluster_spacing,
            );
            let r = &mut self.draft.spec.railing;
            let mut by_count = r.balusters_per_bay.is_some();
            if ui
                .checkbox(&mut by_count, "Set the number of balusters in each bay")
                .changed()
            {
                r.balusters_per_bay = by_count.then_some(5);
            }
            if let Some(n) = &mut r.balusters_per_bay {
                row(ui, "Balusters per Bay", |ui| {
                    ui.add(egui::DragValue::new(n).range(1..=40))
                });
            }
        } else {
            self.fields.length_row(
                ui,
                "Panel Thickness",
                "rail_panel_t",
                &mut self.draft.spec.railing.panel_thickness,
            );
        }
        let r = &self.draft.spec.railing;
        let newels = r.newel_count(len);
        let balusters = r.baluster_centers(len, &r.newel_centers(len)).len();
        ui.weak(format!(
            "This run: {newels} newel{}, {balusters} baluster{}",
            if newels == 1 { "" } else { "s" },
            if balusters == 1 { "" } else { "s" },
        ));
        let gap = r.widest_gap(len);
        if gap > plan_core::walls::spec_tabs::MAX_BALUSTER_CLEAR + 1e-9 {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!(
                    "The widest gap is {}; a 4\" sphere must not pass (IRC R312.1.3)",
                    fmt_ft_in(gap)
                ),
            );
        }
    }

    /// Rails tab (W-117).
    pub(super) fn rails(&mut self, ui: &mut Ui) {
        if !self.is_railing_wall() {
            ui.weak("Draw a Railing or Deck Railing wall, or tick Railing on the General tab.");
            return;
        }
        section(ui, "Top Rail");
        let mut top = self.draft.spec.railing.rail_top();
        if self
            .fields
            .length_row(ui, "Top of Top Rail", "rail_top", &mut top)
        {
            self.draft.spec.railing.top_rail_top = Some(top);
        }
        {
            let r = &mut self.draft.spec.railing;
            row(ui, "Profile", |ui| {
                egui::ComboBox::from_id_salt("rail_top_profile")
                    .selected_text(r.top_rail_profile.name())
                    .show_ui(ui, |ui| {
                        for p in RailProfile::ALL {
                            ui.selectable_value(&mut r.top_rail_profile, p, p.name());
                        }
                    });
            });
        }
        self.fields.length_row(
            ui,
            "Rail Height",
            "rail_top_h",
            &mut self.draft.spec.railing.top_rail_height,
        );
        self.fields.length_row(
            ui,
            "Rail Width",
            "rail_top_w",
            &mut self.draft.spec.railing.top_rail_width,
        );
        super::super::code_notice::code_notice(
            ui,
            "IRC R312.1.2 guard height",
            &mut top,
            crate::editor::code::active().guard_height,
            super::super::code_notice::LimitKind::Min,
        );
        if (top - self.draft.spec.railing.rail_top()).abs() > 1e-9 {
            self.draft.spec.railing.top_rail_top = Some(top);
        }
        section(ui, "Bottom Rail");
        ui.checkbox(&mut self.draft.spec.railing.bottom_rail, "Bottom Rail");
        if self.draft.spec.railing.bottom_rail {
            {
                let r = &mut self.draft.spec.railing;
                row(ui, "Profile", |ui| {
                    egui::ComboBox::from_id_salt("rail_bottom_profile")
                        .selected_text(r.bottom_rail_profile.name())
                        .show_ui(ui, |ui| {
                            for p in RailProfile::ALL {
                                ui.selectable_value(&mut r.bottom_rail_profile, p, p.name());
                            }
                        });
                });
            }
            self.fields.length_row(
                ui,
                "Rail Height",
                "rail_bot_h",
                &mut self.draft.spec.railing.bottom_rail_height,
            );
            self.fields.length_row(
                ui,
                "Rail Width",
                "rail_bot_w",
                &mut self.draft.spec.railing.bottom_rail_width,
            );
        }
        self.fields.length_row(
            ui,
            "Gap Above the Floor",
            "rail_bot_gap",
            &mut self.draft.spec.railing.bottom_rail_gap,
        );
    }

    /// Materials tab: a library material per layer or surface.
    pub(super) fn materials_tab(&mut self, ui: &mut Ui) {
        section(ui, "Materials");
        let parts = WallMaterials::part_names(self.draft_type());
        let lib = self
            .material_lib
            .get_or_insert_with(crate::tools::materials::library)
            .clone();
        for (i, part) in parts.iter().enumerate() {
            let paint = self.draft.spec.materials.get(part).cloned();
            row(ui, part, |ui| {
                if let Some(p) = &paint {
                    egui::color_picker::show_color(
                        ui,
                        egui::Color32::from_rgb(p.rgb[0], p.rgb[1], p.rgb[2]),
                        egui::vec2(18.0, 18.0),
                    );
                }
                let cur = paint
                    .as_ref()
                    .map(|p| p.material.clone())
                    .unwrap_or_default();
                if let Some(pick) = material_pick(
                    ui,
                    &format!("wall_mat_{i}"),
                    &cur,
                    &mut self.material_filter,
                    &lib,
                    "Default (usual look)",
                ) {
                    match pick {
                        Some((name, rgb)) => self
                            .draft
                            .spec
                            .materials
                            .set(part, Some((name.as_str(), rgb))),
                        None => self.draft.spec.materials.set(part, None),
                    }
                }
            });
        }
        ui.weak(
            "A layer or surface without a material keeps its usual look. The Material \
             Painter's library is the list.",
        );
    }

    /// Components tab: each layer with its area and volume, net of openings.
    pub(super) fn components_tab(&mut self, ui: &mut Ui) {
        section(ui, "Components");
        let rows = wall_components(&self.draft, self.draft_type(), &self.openings);
        egui::Grid::new("wall_components")
            .striped(true)
            .show(ui, |ui| {
                for h in [
                    "Component",
                    "Material",
                    "Thickness",
                    "Area (sq ft)",
                    "Volume (cu ft)",
                ] {
                    ui.strong(h);
                }
                ui.end_row();
                for r in &rows {
                    ui.label(&r.name);
                    ui.label(&r.material);
                    ui.label(fmt_ft_in(r.thickness));
                    ui.label(format!("{:.1}", r.area_sq_ft));
                    ui.label(format!("{:.2}", r.volume_cu_ft));
                    ui.end_row();
                }
            });
        ui.weak(format!(
            "Calculated for the wall shown in the preview ({} long), less its {} opening{}.",
            fmt_ft_in(self.draft.path_length()),
            self.openings.len(),
            if self.openings.len() == 1 { "" } else { "s" },
        ));
    }

    /// Object Information tab (W-118).
    pub(super) fn object_information(&mut self, ui: &mut Ui) {
        section(ui, "Wall");
        let w = &self.draft;
        let ty = w.wall_type.clone().unwrap_or_else(|| "Custom".into());
        let rows = [
            ("Object", "Wall".to_string()),
            ("Type", ty),
            ("Class", w.class.label().to_string()),
            ("Length", fmt_ft_in(w.length())),
            ("Thickness", fmt_ft_in(w.thickness)),
            ("Height", fmt_ft_in(w.height)),
            ("Layer", w.layer.clone()),
        ];
        egui::Grid::new("wall_info").striped(true).show(ui, |ui| {
            for (k, v) in rows {
                ui.label(k);
                ui.label(v);
                ui.end_row();
            }
        });
        let spec = &mut self.draft.spec;
        section(ui, "Identification");
        row(ui, "ID", |ui| {
            ui.add(egui::TextEdit::singleline(&mut spec.info.id).desired_width(240.0));
        });
        row(ui, "Description", |ui| {
            ui.add(egui::TextEdit::singleline(&mut spec.info.description).desired_width(240.0));
        });
        section(ui, "Product");
        for (label, text) in [
            ("Manufacturer", &mut spec.schedule.manufacturer),
            ("Model Number", &mut spec.schedule.model),
            ("Supplier", &mut spec.schedule.supplier),
        ] {
            row(ui, label, |ui| {
                ui.add(egui::TextEdit::singleline(text).desired_width(240.0));
            });
        }
        ui.label("Notes");
        ui.add(
            egui::TextEdit::multiline(&mut spec.schedule.comment)
                .desired_rows(3)
                .desired_width(320.0),
        );
        ui.weak("Manufacturer, model, supplier and notes are the Schedule tab's fields; the Wall schedule lists them.");
    }

    /// Schedule tab: whether the wall is in the Wall schedule and what its
    /// row reads.
    pub(super) fn schedule_tab(&mut self, ui: &mut Ui) {
        section(ui, "Wall Schedule");
        ui.checkbox(&mut self.draft.spec.schedule.include, "Include in Schedule")
            .on_hover_text(
                "A cleared box leaves the wall out of the Wall schedule and its numbering",
            );
        let w = &self.draft;
        let net = (w.path_length() * w.covering_height()
            - plan_core::walls::openings_area(w, &self.openings))
            / 144.0;
        egui::Grid::new("wall_schedule_row")
            .striped(true)
            .show(ui, |ui| {
                for h in ["Type", "Length", "Thickness", "Height", "Area (sq ft)"] {
                    ui.strong(h);
                }
                ui.end_row();
                ui.label(w.wall_type.clone().unwrap_or_else(|| "Custom".into()));
                ui.label(fmt_ft_in(w.path_length()));
                ui.label(fmt_ft_in(w.thickness));
                ui.label(fmt_ft_in(w.height));
                ui.label(format!("{net:.1}"));
                ui.end_row();
            });
        ui.weak("Add the Manufacturer, Model, Supplier, Comment, Description and Object ID columns in the schedule's Specification.");
    }

    /// The Drawing Group row of the Layer tab.
    pub(super) fn drawing_group_row(&mut self, ui: &mut Ui) {
        let cur = self
            .draft
            .spec
            .drawing_group
            .unwrap_or(DEFAULT_DRAWING_GROUP);
        row(ui, "Drawing Group", |ui| {
            let default = self.draft.spec.drawing_group.is_none();
            let text = if default {
                format!("Default: {}", drawing_group_name(DEFAULT_DRAWING_GROUP))
            } else {
                drawing_group_name(cur)
            };
            egui::ComboBox::from_id_salt("wall_group")
                .selected_text(text)
                .width(240.0)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(
                            default,
                            format!("Default: {}", drawing_group_name(DEFAULT_DRAWING_GROUP)),
                        )
                        .clicked()
                    {
                        self.draft.spec.drawing_group = None;
                    }
                    for (n, _) in DRAWING_GROUPS {
                        if ui
                            .selectable_label(!default && cur == n, drawing_group_name(n))
                            .clicked()
                        {
                            self.draft.spec.drawing_group = Some(n);
                        }
                    }
                });
        });
    }
}
