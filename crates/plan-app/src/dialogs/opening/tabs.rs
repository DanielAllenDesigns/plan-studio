//! The Door and Window Specification tabs of round 15: Rough Opening,
//! Framing, Energy Values, Layer, Materials, Object Information, and for a
//! window Shape and Treatments (docs/chief-x18-dialogs.md). Their values live
//! in `Opening.extras.spec`, so accepting the dialog stores them in the one
//! undo step the Specification already is.

use super::{row, section, Fields, OpeningForm};
use eframe::egui::{self, Ui};
use plan_core::openings::spec::{
    BlindStyle, CornerCut, CurtainStyle, DoorSwing, HeaderMaterial, InteriorShutterStyle,
    MillworkStyle, RoughMode, ShapeKind, WindowShape, DOOR_ENERGY_TYPES, DOOR_PARTS,
    WINDOW_ENERGY_TYPES, WINDOW_PARTS,
};
use plan_core::OpeningKind;

/// The layers the Layer tab lists when the host gave it none.
const DEFAULT_LAYERS: [&str; 6] = [
    "Doors",
    "Windows",
    "Doors, Labels",
    "Windows, Labels",
    "Walls, Main",
    "Walls, Normal",
];

/// The most materials a picker lists at once.
const MATERIAL_ROWS: usize = 60;

/// The energy type picked: its name and its usual U-factor and SHGC are
/// written to the Energy Values.
pub(super) fn pick_energy_type(
    energy: &mut plan_core::openings::spec::EnergyValues,
    name: &str,
    u: f64,
    shgc: f64,
) {
    energy.construction = name.to_string();
    energy.u_factor = u;
    energy.shgc = shgc;
}

/// The Shape combo's choice: a preset fills the values it stands for on a
/// window `w` x `h`; Custom keeps what is there.
pub(super) fn pick_shape(shape: &mut WindowShape, kind: ShapeKind, w: f64, h: f64) {
    if kind == ShapeKind::Custom {
        shape.kind = kind;
    } else {
        *shape = WindowShape::preset(kind, w, h);
    }
}

/// A count with a "Framing Defaults" checkbox: `None` follows the defaults.
fn optional_count(ui: &mut Ui, label: &str, value: &mut Option<u32>, start: u32) {
    row(ui, label, |ui| {
        let mut follow = value.is_none();
        if ui.checkbox(&mut follow, "Framing Defaults").changed() {
            *value = if follow { None } else { Some(start) };
        }
        if let Some(v) = value {
            ui.add(egui::DragValue::new(v).range(0..=6));
        }
    });
}

/// A top or bottom corner cut: a checkbox, its height and its offset.
fn corner_rows(
    fields: &mut Fields,
    ui: &mut Ui,
    title: &str,
    keys: (&'static str, &'static str),
    cut: &mut CornerCut,
) {
    ui.checkbox(&mut cut.on, title);
    ui.add_enabled_ui(cut.on, |ui| {
        fields.length_row(ui, "Height", keys.0, &mut cut.height);
        fields.length_row(ui, "Offset", keys.1, &mut cut.offset);
    });
    cut.height = cut.height.max(0.0);
    cut.offset = cut.offset.max(0.0);
}

fn color_row(ui: &mut Ui, label: &str, rgb: &mut [u8; 3]) {
    row(ui, label, |ui| {
        ui.color_edit_button_srgb(rgb);
    });
}

impl OpeningForm {
    /// The Rough Opening tab (DW-56): the opening the framer leaves, the
    /// concrete cutout under a door, and its plan display.
    pub(super) fn rough_opening(&mut self, ui: &mut Ui) {
        self.use_default_row(ui, super::DynGroup::Rough);
        let door = self.is_door();
        section(ui, "Rough Opening");
        let (w, h, head) = (
            self.draft.rough_width(),
            self.draft.rough_height(),
            self.draft.header_bottom(),
        );
        row(ui, "Total Width", |ui| ui.label(super::fmt_short(w)));
        row(ui, "Total Height", |ui| ui.label(super::fmt_short(h)));
        row(ui, "Header Bottom Height", |ui| {
            ui.label(super::fmt_short(head))
        });
        let r = &mut self.draft.extras.spec.rough;
        row(ui, "Calculate", |ui| {
            for m in RoughMode::ALL {
                ui.radio_value(&mut r.mode, m, m.name());
            }
        });
        match r.mode {
            RoughMode::AdditionalSpace => {
                self.fields
                    .length_row(ui, "Additional Width", "ro_addw", &mut r.add_width);
                self.fields
                    .length_row(ui, "Additional Height", "ro_addh", &mut r.add_height);
            }
            RoughMode::ClearanceGap => {
                self.fields
                    .length_row(ui, "Side Gap", "ro_gs", &mut r.gap_side);
                self.fields
                    .length_row(ui, "Top Gap", "ro_gt", &mut r.gap_top);
                self.fields
                    .length_row(ui, "Bottom Gap", "ro_gb", &mut r.gap_bottom);
            }
        }
        for v in [
            &mut r.add_width,
            &mut r.add_height,
            &mut r.gap_side,
            &mut r.gap_top,
            &mut r.gap_bottom,
        ] {
            *v = v.max(0.0);
        }
        if door {
            section(ui, "Add for Concrete Cutout");
            self.fields
                .length_row(ui, "Each Side", "ro_conc", &mut r.concrete_each_side);
            r.concrete_each_side = r.concrete_each_side.max(0.0);
            ui.add_enabled(
                r.concrete_each_side > 0.0,
                egui::Checkbox::new(&mut r.concrete_show_below, "Show In Floor Below"),
            );
        }
        section(ui, "Plan Display");
        ui.checkbox(&mut r.show_in_plan, "Show Rough Opening in Plan")
            .on_hover_text("Dashed lines across the wall at the rough opening's edges");
        ui.weak("The framing stands around the rough opening; the schedule lists it in its Rough Opening column.");
    }

    /// The Framing tab (DW-114): header, trimmers, king studs and sill.
    pub(super) fn framing(&mut self, ui: &mut Ui) {
        self.use_default_row(ui, super::DynGroup::Framing);
        let door = self.is_door();
        let fr = &mut self.draft.extras.spec.framing;
        section(ui, "Header");
        ui.checkbox(&mut fr.include_header, "Include Header");
        ui.add_enabled_ui(fr.include_header, |ui| {
            row(ui, "Construction", |ui| {
                egui::ComboBox::from_id_salt("fr_material")
                    .selected_text(fr.header_material.name())
                    .show_ui(ui, |ui| {
                        for m in HeaderMaterial::ALL {
                            ui.selectable_value(&mut fr.header_material, m, m.name());
                        }
                    });
            });
            optional_count(ui, "Count", &mut fr.header_plies, 2);
            let mut calc = fr.header_depth.is_none();
            row(ui, "Depth", |ui| {
                if ui.checkbox(&mut calc, "Calculate from Width").changed() {
                    fr.header_depth = if calc { None } else { Some(5.5) };
                }
            });
            if let Some(mut d) = fr.header_depth {
                if self
                    .fields
                    .length_row(ui, "Header Depth", "fr_depth", &mut d)
                {
                    fr.header_depth = Some(d.max(1.5));
                }
            }
        });
        section(ui, "Supports");
        optional_count(ui, "Trimmer Count", &mut fr.trimmers, 1);
        optional_count(ui, "King Stud Count", &mut fr.king_studs, 1);
        if !door {
            section(ui, "Sills");
            ui.checkbox(&mut fr.sill, "Include Sill");
        }
        ui.weak("Where a count follows the Framing Defaults, Build > Framing sets it.");
    }

    /// The Energy Values tab (DW-115): stored for the schedules.
    pub(super) fn energy_values(&mut self, ui: &mut Ui) {
        let door = self.is_door();
        let types: &[(&str, f64, f64)] = if door {
            &DOOR_ENERGY_TYPES
        } else {
            &WINDOW_ENERGY_TYPES
        };
        let e = &mut self.draft.extras.spec.energy;
        section(ui, "Assembly");
        row(ui, if door { "Door Type" } else { "Glazing" }, |ui| {
            let shown = if e.construction.is_empty() {
                "Custom".to_string()
            } else {
                e.construction.clone()
            };
            egui::ComboBox::from_id_salt("energy_type")
                .selected_text(shown)
                .width(220.0)
                .show_ui(ui, |ui| {
                    for (name, u, g) in types {
                        if ui
                            .selectable_label(e.construction == *name, *name)
                            .clicked()
                        {
                            pick_energy_type(e, name, *u, *g);
                        }
                    }
                });
        });
        row(ui, "U-Factor", |ui| {
            ui.add(
                egui::DragValue::new(&mut e.u_factor)
                    .speed(0.01)
                    .range(0.0..=5.0)
                    .fixed_decimals(2),
            )
            .on_hover_text("Btu / (h ft2 F); lower insulates better");
        });
        row(ui, "SHGC", |ui| {
            ui.add(
                egui::DragValue::new(&mut e.shgc)
                    .speed(0.01)
                    .range(0.0..=1.0)
                    .fixed_decimals(2),
            )
            .on_hover_text("Solar heat gain coefficient, 0 to 1");
        });
        ui.weak("Listed in the U-Factor and SHGC columns of the schedule.");
    }

    /// The Layer tab (DW-116): the layer the opening is on.
    pub(super) fn layer_tab(&mut self, ui: &mut Ui) {
        let kind_layer = match self.draft.kind {
            OpeningKind::Door => "Doors",
            OpeningKind::Window => "Windows",
        };
        let current = self.draft.layer_name().to_string();
        let choices: Vec<String> = if self.layer_choices.is_empty() {
            DEFAULT_LAYERS.iter().map(|s| s.to_string()).collect()
        } else {
            self.layer_choices.clone()
        };
        section(ui, "Layer");
        let spec = &mut self.draft.extras.spec;
        row(ui, "Layer", |ui| {
            egui::ComboBox::from_id_salt("op_layer")
                .selected_text(&current)
                .width(220.0)
                .show_ui(ui, |ui| {
                    for name in &choices {
                        if ui.selectable_label(&current == name, name).clicked() {
                            spec.layer = (name != kind_layer).then(|| name.clone());
                        }
                    }
                });
            if ui
                .small_button("Define\u{2026}")
                .on_hover_text("Open Layer Display Options")
                .clicked()
            {
                crate::dialogs::layer_display::open_define();
            }
        });
        row(ui, "Other Layer", |ui| {
            let mut text = spec.layer.clone().unwrap_or_default();
            if ui
                .add(egui::TextEdit::singleline(&mut text).desired_width(220.0))
                .changed()
            {
                spec.layer = (!text.trim().is_empty()).then_some(text);
            }
        });
        if ui
            .add_enabled(
                spec.layer.is_some(),
                egui::Button::new("Use the Default Layer"),
            )
            .clicked()
        {
            spec.layer = None;
        }
        ui.weak(format!(
            "A {} sits on the \"{kind_layer}\" layer unless this says otherwise.",
            match self.draft.kind {
                OpeningKind::Door => "door",
                OpeningKind::Window => "window",
            }
        ));
    }

    /// The Materials tab (DW-117): a library material per component.
    pub(super) fn materials_tab(&mut self, ui: &mut Ui) {
        self.use_default_row(ui, super::DynGroup::Materials);
        let parts: &[&str] = if self.is_door() {
            &DOOR_PARTS
        } else {
            &WINDOW_PARTS
        };
        let Self {
            draft,
            material_filter,
            material_lib,
            ..
        } = self;
        let lib = material_lib.get_or_insert_with(crate::tools::materials::library);
        section(ui, "Materials");
        for part in parts {
            let paint = draft.extras.spec.materials.get(part).cloned();
            row(ui, part, |ui| {
                if let Some(p) = &paint {
                    egui::color_picker::show_color(
                        ui,
                        egui::Color32::from_rgb(p.rgb[0], p.rgb[1], p.rgb[2]),
                        egui::vec2(18.0, 18.0),
                    );
                }
                let shown = paint
                    .as_ref()
                    .map_or("Default".to_string(), |p| p.material.clone());
                egui::ComboBox::from_id_salt(("op_mat", *part))
                    .selected_text(shown)
                    .width(240.0)
                    .show_ui(ui, |ui| {
                        ui.add(
                            egui::TextEdit::singleline(material_filter)
                                .hint_text("Search materials")
                                .desired_width(220.0),
                        );
                        if ui
                            .selectable_label(paint.is_none(), "Default (usual look)")
                            .clicked()
                        {
                            draft.extras.spec.materials.set(part, None);
                        }
                        egui::ScrollArea::vertical()
                            .max_height(220.0)
                            .show(ui, |ui| {
                                for def in
                                    lib.search(material_filter).into_iter().take(MATERIAL_ROWS)
                                {
                                    let on = paint.as_ref().is_some_and(|p| p.material == def.name);
                                    if ui.selectable_label(on, &def.name).clicked() {
                                        draft
                                            .extras
                                            .spec
                                            .materials
                                            .set(part, Some((def.name.as_str(), def.color)));
                                    }
                                }
                            });
                    });
            });
        }
        ui.weak("A component without a material keeps its usual look. The Material Painter's library is the list.");
    }

    /// The Object Information tab (DW-118).
    pub(super) fn object_information(&mut self, ui: &mut Ui) {
        let o = &self.draft;
        section(ui, "Opening");
        let rows = [
            (
                "Object",
                match o.kind {
                    OpeningKind::Door => "Door",
                    OpeningKind::Window => "Window",
                }
                .to_string(),
            ),
            ("Style", o.type_name().to_string()),
            (
                "Size (W x H)",
                format!(
                    "{} x {}",
                    super::fmt_short(o.width),
                    super::fmt_short(o.height)
                ),
            ),
            ("Rough Opening", o.rough_text()),
            ("Layer", o.layer_name().to_string()),
        ];
        egui::Grid::new("op_info").striped(true).show(ui, |ui| {
            for (k, v) in rows {
                ui.label(k);
                ui.label(v);
                ui.end_row();
            }
        });
        let spec = &mut self.draft.extras.spec;
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
        ui.weak("Manufacturer, model, supplier and notes are the Schedule tab's fields; the schedule lists them.");
    }

    /// The Shape tab of a window (DW-121).
    pub(super) fn shape_tab(&mut self, ui: &mut Ui) {
        let (w, h) = (self.draft.width, self.draft.height);
        ui.label(format!("Window width is: {}", super::fmt_short(w)));
        let shape = &mut self.draft.extras.spec.shape;
        section(ui, "Shape");
        row(ui, "Shape", |ui| {
            let mut kind = shape.kind;
            egui::ComboBox::from_id_salt("win_shape")
                .selected_text(kind.name())
                .show_ui(ui, |ui| {
                    for k in ShapeKind::ALL {
                        ui.selectable_value(&mut kind, k, k.name());
                    }
                });
            if kind != shape.kind {
                pick_shape(shape, kind, w, h);
            }
            if ui.button("Revert All").clicked() {
                *shape = WindowShape::default();
            }
        });
        if shape.kind == ShapeKind::Custom {
            section(ui, "Sides");
            let mut lh = shape.left_height.unwrap_or(h);
            if self.fields.length_row(ui, "Height Left", "sh_lh", &mut lh) {
                shape.left_height = Some(lh.clamp(0.0, h));
            }
            let mut rh = shape.right_height.unwrap_or(h);
            if self.fields.length_row(ui, "Height Right", "sh_rh", &mut rh) {
                shape.right_height = Some(rh.clamp(0.0, h));
            }
            section(ui, "Top Inside Corners");
            corner_rows(
                &mut self.fields,
                ui,
                "Left",
                ("sh_tl_h", "sh_tl_o"),
                &mut shape.top_left,
            );
            corner_rows(
                &mut self.fields,
                ui,
                "Right",
                ("sh_tr_h", "sh_tr_o"),
                &mut shape.top_right,
            );
            section(ui, "Bottom Corners");
            corner_rows(
                &mut self.fields,
                ui,
                "Left",
                ("sh_bl_h", "sh_bl_o"),
                &mut shape.bottom_left,
            );
            corner_rows(
                &mut self.fields,
                ui,
                "Right",
                ("sh_br_h", "sh_br_o"),
                &mut shape.bottom_right,
            );
        } else if shape.is_shaped() {
            ui.weak("Choose Custom to type the sides and corners.");
        }
        let spec = &mut self.draft.extras.spec;
        section(ui, "Lite Pattern");
        row(ui, "Type", |ui| {
            egui::ComboBox::from_id_salt("shape_lite_type")
                .selected_text(spec.lite_style.name())
                .show_ui(ui, |ui| {
                    for st in plan_core::openings::LiteStyle::ALL {
                        ui.selectable_value(&mut spec.lite_style, st, st.name());
                    }
                });
        });
        let e = &mut self.extras;
        row(ui, "Lites Across", |ui| {
            ui.add(egui::DragValue::new(&mut e.lites_across).range(1..=16));
        });
        row(ui, "Lites Vertical", |ui| {
            ui.add(egui::DragValue::new(&mut e.lites_vertical).range(1..=16));
        });
        self.fields
            .length_row(ui, "Muntin Width", "sh_muntin", &mut e.muntin_width);
        self.fields
            .length_row(ui, "Mullion Width", "sh_mullion", &mut spec.mullion_width);
        spec.mullion_width = spec.mullion_width.max(0.0);
        ui.weak(
            "The dividers run inside the shape. The same values are on the Lites and Sash tabs.",
        );
    }

    /// The Treatments tab of a window (DW-123).
    pub(super) fn treatments_tab(&mut self, ui: &mut Ui) {
        self.use_default_row(ui, super::DynGroup::Treatments);
        let t = &mut self.draft.extras.spec.treatments;
        section(ui, "Curtains");
        row(ui, "Style", |ui| {
            egui::ComboBox::from_id_salt("tr_curtain")
                .selected_text(t.curtain.name())
                .show_ui(ui, |ui| {
                    for s in CurtainStyle::ALL {
                        ui.selectable_value(&mut t.curtain, s, s.name());
                    }
                });
        });
        ui.add_enabled_ui(t.curtain != CurtainStyle::None, |ui| {
            color_row(ui, "Color", &mut t.curtain_color);
            self.fields
                .length_row(ui, "Height Off Floor", "tr_off", &mut t.curtain_off_floor);
            self.fields.length_row(
                ui,
                "Height Above Casing",
                "tr_above",
                &mut t.curtain_above_casing,
            );
        });
        section(ui, "Blinds");
        row(ui, "Style", |ui| {
            egui::ComboBox::from_id_salt("tr_blind")
                .selected_text(t.blind.name())
                .show_ui(ui, |ui| {
                    for s in BlindStyle::ALL {
                        ui.selectable_value(&mut t.blind, s, s.name());
                    }
                });
        });
        ui.add_enabled_ui(t.blind != BlindStyle::None, |ui| {
            color_row(ui, "Color", &mut t.blind_color);
            row(ui, "Lowered", |ui| {
                let mut pct = (t.blind_lowered * 100.0).round();
                if ui
                    .add(egui::Slider::new(&mut pct, 0.0..=100.0).suffix(" %"))
                    .changed()
                {
                    t.blind_lowered = pct / 100.0;
                }
            });
        });
        section(ui, "Interior Shutters");
        row(ui, "Style", |ui| {
            egui::ComboBox::from_id_salt("tr_shutter")
                .selected_text(t.shutter.name())
                .show_ui(ui, |ui| {
                    for s in InteriorShutterStyle::ALL {
                        ui.selectable_value(&mut t.shutter, s, s.name());
                    }
                });
        });
        ui.add_enabled_ui(t.shutter != InteriorShutterStyle::None, |ui| {
            color_row(ui, "Color", &mut t.shutter_color);
            ui.checkbox(&mut t.shutter_closed, "Show Closed");
        });
        section(ui, "Exterior Millwork Above Casing");
        row(ui, "Style", |ui| {
            egui::ComboBox::from_id_salt("tr_mill_up")
                .selected_text(t.millwork_above.name())
                .show_ui(ui, |ui| {
                    for s in MillworkStyle::ABOVE {
                        ui.selectable_value(&mut t.millwork_above, s, s.name());
                    }
                });
        });
        ui.add_enabled_ui(t.millwork_above != MillworkStyle::None, |ui| {
            self.fields
                .length_row(ui, "Height", "tr_mill_uh", &mut t.millwork_above_height);
            self.fields
                .length_row(ui, "Width", "tr_mill_uw", &mut t.millwork_above_width);
        });
        section(ui, "Exterior Millwork Below Casing");
        row(ui, "Style", |ui| {
            egui::ComboBox::from_id_salt("tr_mill_down")
                .selected_text(t.millwork_below.name())
                .show_ui(ui, |ui| {
                    for s in MillworkStyle::BELOW {
                        ui.selectable_value(&mut t.millwork_below, s, s.name());
                    }
                });
        });
        ui.add_enabled_ui(t.millwork_below != MillworkStyle::None, |ui| {
            self.fields
                .length_row(ui, "Height", "tr_mill_dh", &mut t.millwork_below_height);
            self.fields
                .length_row(ui, "Extend", "tr_mill_de", &mut t.millwork_below_extend);
        });
        for v in [
            &mut t.curtain_off_floor,
            &mut t.curtain_above_casing,
            &mut t.millwork_above_height,
            &mut t.millwork_above_width,
            &mut t.millwork_below_height,
            &mut t.millwork_below_extend,
        ] {
            *v = v.max(0.0);
        }
        ui.weak("Treatments show in 3D and the elevations; the plan does not draw them.");
    }

    /// The Door Swing group of the Options tab, for a double door (DW-35).
    pub(super) fn door_swing_group(&mut self, ui: &mut Ui) {
        section(ui, "Door Swing");
        let spec = &mut self.draft.extras.spec;
        row(ui, "Swing", |ui| {
            for s in DoorSwing::ALL {
                ui.radio_value(&mut spec.door_swing, s, s.name());
            }
        });
        ui.checkbox(&mut spec.swings_from_center, "Swings from Center")
            .on_hover_text("Both leaves hinge in the middle of the opening");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shape_preset_fills_its_values_and_custom_keeps_them() {
        let mut s = WindowShape::default();
        pick_shape(&mut s, ShapeKind::Triangle, 36.0, 48.0);
        assert!(s.is_shaped() && s.top_left.on && s.top_left.height == 48.0);
        pick_shape(&mut s, ShapeKind::Custom, 36.0, 48.0);
        assert_eq!(s.kind, ShapeKind::Custom);
        assert!(s.top_left.on, "the preset's values stay for editing");
        pick_shape(&mut s, ShapeKind::Rectangle, 36.0, 48.0);
        assert!(!s.is_shaped());
    }

    #[test]
    fn an_energy_type_writes_its_usual_values() {
        let mut e = plan_core::openings::spec::EnergyValues::default();
        pick_energy_type(&mut e, "Triple Pane Low-E", 0.2, 0.25);
        assert_eq!(e.construction, "Triple Pane Low-E");
        assert_eq!((e.u_factor, e.shgc), (0.2, 0.25));
    }
}
