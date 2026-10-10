//! The panels of the Camera Specification that follow the manual's Camera,
//! Positioning, Below Grade, Selected Defaults, Plan Display and Layer panels
//! (pp. 1186 to 1200; `docs/parity/3d-views-cameras.md` C-121, C-139,
//! C-140, C-146 to C-151, C-153, C-156, C-158).
//!
//! These are more `impl CameraDialog` pages: they edit `CameraObject::view`
//! on the dialog's draft and nothing else.

use super::*;
use crate::dialogs::default_sets::{self, Selected};
use plan_core::camera_view::spec::LightChoice;
use plan_core::camera_view::{BelowGradeLimit, CalloutArrow, CalloutPlacement, SuperResolution};

/// The line styles the Below Grade and Plan Display panels offer.
pub const LINE_STYLES: [&str; 4] = ["Solid", "Dashed", "Dotted", "Dash-Dot"];

/// The Selected Defaults of a view as the panel's draft: the view's own, or
/// the plan's active defaults when the view follows them.
pub fn selected_of(view: &plan_core::camera_view::ViewDefaults, plan: &Selected) -> Selected {
    if view.is_plan_default() {
        return plan.clone();
    }
    Selected {
        default_set: view.default_set.clone(),
        picks: view.picks.clone(),
        layer_set: view.layer_set.clone(),
        cad_layer: view.cad_layer.clone(),
    }
}

/// The view's stored defaults for a panel draft: nothing when the draft is
/// the plan's own active choice, so the view keeps following the plan.
pub fn view_defaults_of(sel: &Selected, plan: &Selected) -> plan_core::camera_view::ViewDefaults {
    // The panel adds an empty pick for every kind it lists; those are not a
    // choice.
    let picks = |s: &Selected| {
        s.picks
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    if sel.default_set == plan.default_set
        && sel.layer_set == plan.layer_set
        && sel.cad_layer == plan.cad_layer
        && picks(sel) == picks(plan)
    {
        return plan_core::camera_view::ViewDefaults::default();
    }
    plan_core::camera_view::ViewDefaults {
        default_set: sel.default_set.clone(),
        picks: sel
            .picks
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        layer_set: sel.layer_set.clone(),
        cad_layer: sel.cad_layer.clone(),
    }
}

fn pen_row(ui: &mut egui::Ui, label: &str, value: &mut f64, range: std::ops::RangeInclusive<f64>) {
    row(ui, label, |ui| {
        ui.add(egui::Slider::new(value, range).fixed_decimals(2))
    });
}

fn color_row(ui: &mut egui::Ui, label: &str, color: &mut [u8; 3]) {
    row(ui, label, |ui| ui.color_edit_button_srgb(color));
}

impl CameraDialog {
    /// Show Color and Show Watermark, the end of the General group.
    pub(super) fn general_options(&mut self, ui: &mut egui::Ui) {
        if self.draft.kind == CameraKind::Walkthrough {
            return;
        }
        let o = &mut self.draft.view.options;
        ui.checkbox(&mut o.show_color, "Show Color")
            .on_hover_text("Uncheck to draw the view in grey");
        ui.checkbox(&mut o.show_watermark, "Show Watermark")
            .on_hover_text("Shows the plan's watermark over the view (View > Watermark)");
    }

    /// What the Rendering, Upscaling, Depth of Field, Lighting and Options
    /// groups add to the technique and shadows rows (manual pp. 1188, 1189,
    /// 1195, 1196). A section or elevation only has Ray Casted Sun Shadows,
    /// Sharpening and Use Sunlight.
    pub(super) fn render_extras(&mut self, ui: &mut egui::Ui) {
        let section_cam = is_elevation_camera(&self.draft);
        let walkthrough = self.draft.kind == CameraKind::Walkthrough;
        let o = &mut self.draft.view.options;
        ui.checkbox(&mut o.ray_sun_shadows, "Ray Casted Sun Shadows")
            .on_hover_text("Better sun shadows; they refresh when the camera is not moving");
        if !section_cam {
            ui.checkbox(&mut o.reflections, "Reflections");
            ui.checkbox(&mut o.animate_water, "Animate Water");
            ui.checkbox(&mut o.light_bloom, "Light Bloom")
                .on_hover_text("Standard, Duotone and Watercolor views only");
            pen_row(ui, "Ambient Occlusion", &mut o.ambient_occlusion, 0.0..=1.0);
        }
        section(ui, "Upscaling");
        pen_row(ui, "Sharpening", &mut o.sharpening, 0.0..=1.0);
        if !section_cam {
            row(ui, "Super Resolution", |ui| {
                egui::ComboBox::from_id_salt("camera_super_res")
                    .selected_text(o.super_resolution.label())
                    .show_ui(ui, |ui| {
                        for s in SuperResolution::ALL {
                            ui.selectable_value(&mut o.super_resolution, s, s.label());
                        }
                    })
            });
            if !walkthrough {
                section(ui, "Depth of Field");
                ui.checkbox(&mut o.dof_on, "Depth of Field Enabled");
                ui.add_enabled_ui(o.dof_on, |ui| {
                    row(ui, "F-Stop", |ui| {
                        ui.add(egui::Slider::new(&mut o.f_stop, 1.0..=32.0).fixed_decimals(1))
                    });
                    self.fields.length_row(
                        ui,
                        "Focus Distance",
                        "dof_focus",
                        &mut o.focus_distance,
                    );
                });
            }
        }
        section(ui, "Lighting (Sun and Lights)");
        ui.checkbox(&mut o.use_sunlight, "Use Sunlight")
            .on_hover_text("Uncheck to switch the sun off in this view");
        if !section_cam {
            row(ui, "Lights", |ui| {
                ui.radio_value(&mut o.light_choice, LightChoice::Automatic, "Automatic");
                ui.radio_value(&mut o.light_choice, LightChoice::LightSet, "Light Set");
            });
            if o.light_choice == LightChoice::Automatic {
                row(ui, "Maximum Number", |ui| {
                    ui.add(egui::DragValue::new(&mut o.max_lights).range(1..=64))
                });
            }
        }
        o.clamp();
    }

    /// The Options group: Clip Surfaces Within and Hide Camera-Facing
    /// Exterior Walls (manual p. 1189).
    pub(super) fn view_options(&mut self, ui: &mut egui::Ui) {
        if is_elevation_camera(&self.draft) {
            return;
        }
        section(ui, "Options");
        if self.draft.kind != CameraKind::Walkthrough {
            self.fields
                .degrees_row(ui, "Field of View", "deg_fov", &mut self.draft.fov_deg);
        }
        let o = &mut self.draft.view.options;
        self.fields.length_row(
            ui,
            "Clip Surfaces Within",
            "clip_within",
            &mut o.clip_surfaces_within,
        );
        ui.checkbox(
            &mut o.hide_facing_walls,
            "Hide Camera-Facing Exterior Walls",
        )
        .on_hover_text(
            "Leaves out the exterior walls that face a camera outside the house, \
                 with their doors and windows, and the interior walls of an attic",
        );
        o.clamp();
    }

    /// Depth Cue for a section or elevation (manual pp. 1175, 1176).
    pub(super) fn depth_cue_group(&mut self, ui: &mut egui::Ui) {
        if !is_elevation_camera(&self.draft) {
            return;
        }
        section(ui, "Depth Cue");
        let d = &mut self.draft.view.depth_cue;
        ui.checkbox(&mut d.on, "Use Depth Cue");
        ui.add_enabled_ui(d.on, |ui| {
            let mut sync = d.keep_in_sync;
            if ui.checkbox(&mut sync, "Keep Start/End in Sync").changed() {
                d.set_keep_in_sync(sync);
            }
            let (mut start, mut end) = (d.start, d.end);
            if self.fields.length_row(ui, "Start", "cue_start", &mut start) {
                d.set_start(start);
            }
            if self.fields.length_row(ui, "End", "cue_end", &mut end) {
                d.set_end(end);
            }
            row(ui, "Fog Opacity", |ui| {
                let mut pct = d.opacity * 100.0;
                let r = ui.add(egui::Slider::new(&mut pct, 0.0..=100.0).suffix("%"));
                d.opacity = (pct / 100.0).clamp(0.0, 1.0);
                r
            });
            color_row(ui, "Fog Color", &mut d.color);
        });
    }

    /// The Positioning panel: the camera's place and direction, then the
    /// Navigation group.
    pub(super) fn positioning_tab(&mut self, ui: &mut egui::Ui) {
        if self.draft.kind == CameraKind::Walkthrough {
            section(ui, "Position");
            ui.weak("The path of a walkthrough is edited on the Camera tab.");
            self.navigation(ui);
            return;
        }
        let section_cam = self.is_section();
        section(
            ui,
            if section_cam {
                "Cut Line"
            } else {
                "Camera Position"
            },
        );
        let f = &mut self.fields;
        let d = &mut self.draft;
        f.length_row(
            ui,
            if section_cam {
                "Center X"
            } else {
                "X Position"
            },
            "x",
            &mut d.position.x,
        );
        f.length_row(
            ui,
            if section_cam {
                "Center Y"
            } else {
                "Y Position"
            },
            "y",
            &mut d.position.y,
        );
        f.degrees_row(
            ui,
            if section_cam {
                "View Direction"
            } else {
                "Camera Angle"
            },
            "deg_dir",
            &mut d.direction_deg,
        );
        if section_cam {
            f.length_row(ui, "Section Length", "width", &mut self.section_len);
        } else {
            f.length_row(ui, "Height Above Floor", "eye", &mut d.eye_height);
            let level = d.kind.is_eye_level();
            ui.add_enabled_ui(level, |ui| {
                f.degrees_row(ui, "Tilt Angle (up is +)", "deg_tilt", &mut d.view.tilt_deg);
            });
            if !level {
                ui.weak("An overview orbits the building; only a Full or Floor Camera tilts.");
            }
        }
        self.navigation(ui);
    }

    /// The Below Grade panel (manual pp. 1190, 1197, 1198).
    pub(super) fn below_grade_tab(&mut self, ui: &mut egui::Ui) {
        section(ui, "Below Grade Line Settings");
        ui.weak("Affects Vector Views and Technical Illustration views.");
        let b = &mut self.draft.view.below_grade;
        ui.horizontal(|ui| {
            ui.checkbox(&mut b.override_color, "Override Color");
            ui.add_enabled_ui(b.override_color, |ui| {
                ui.color_edit_button_srgb(&mut b.color)
            });
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut b.override_style, "Override Style");
            ui.add_enabled_ui(b.override_style, |ui| {
                egui::ComboBox::from_id_salt("below_grade_style")
                    .selected_text(b.style.clone())
                    .show_ui(ui, |ui| {
                        for s in LINE_STYLES {
                            ui.selectable_value(&mut b.style, s.to_string(), s);
                        }
                    })
            });
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut b.override_weight, "Override Weight");
            ui.add_enabled_ui(b.override_weight, |ui| {
                ui.add(
                    egui::DragValue::new(&mut b.weight)
                        .range(0.05..=6.0)
                        .speed(0.05)
                        .suffix(" pt"),
                )
            });
        });
        section(ui, "Affects Lines Below");
        let mut terrain = b.limit == BelowGradeLimit::Terrain;
        row(ui, "Limit", |ui| {
            ui.radio_value(&mut terrain, true, "Terrain Perimeter");
            ui.radio_value(&mut terrain, false, "Absolute Height")
        });
        match (terrain, b.limit) {
            (true, BelowGradeLimit::Absolute(_)) => b.limit = BelowGradeLimit::Terrain,
            (false, BelowGradeLimit::Terrain) => b.limit = BelowGradeLimit::Absolute(0.0),
            _ => {}
        }
        if let BelowGradeLimit::Absolute(h) = &mut b.limit {
            self.fields.length_row(ui, "Height", "below_height", h);
        }
        section(ui, "Affected Object Types");
        for name in plan_core::camera_view::spec::BELOW_GRADE_OBJECTS {
            ui.label(name);
        }
    }

    /// The Selected Defaults panel: the same lists as the Active Defaults
    /// dialog, kept for this view.
    pub(super) fn selected_defaults_tab(&mut self, ui: &mut egui::Ui) {
        let Some(env) = self.defaults_env.as_ref() else {
            ui.weak("The plan's lists are not available here.");
            return;
        };
        let plan = self.plan_selected.clone();
        let mut sel = selected_of(&self.draft.view.selected, &plan);
        let events = sel.panel(ui, env, true, "camera_selected");
        self.draft.view.selected = view_defaults_of(&sel, &plan);
        self.defaults_events.extend(events);
        ui.add_space(6.0);
        ui.weak(
            "Text, dimensions and notes drawn in this view use these defaults, \
             and their CAD layer.",
        );
    }

    /// The Plan Display panel (manual pp. 1190, 1191, 1198 to 1200).
    pub(super) fn plan_display_tab(&mut self, ui: &mut egui::Ui) {
        let section_cam = self.is_section();
        let walkthrough = self.draft.kind == CameraKind::Walkthrough;
        let name = self.draft.name.clone();
        section(ui, "Display on All Floors");
        let p = &mut self.draft.view.plan;
        ui.checkbox(&mut p.all_floors, "Display on All Floors")
            .on_hover_text(
                "Otherwise the symbol only shows on the floor the camera was created on",
            );
        section(ui, "Display as Callout");
        let c = &mut self.draft.callout;
        ui.checkbox(&mut c.show, "Display as Callout")
            .on_hover_text("A camera must be a callout to print, export or go to layout");
        ui.add_enabled_ui(c.show, |ui| {
            if section_cam {
                row(ui, "Placement", |ui| {
                    egui::ComboBox::from_id_salt("callout_placement")
                        .selected_text(p.placement.label())
                        .show_ui(ui, |ui| {
                            for pl in CalloutPlacement::ALL {
                                ui.selectable_value(&mut p.placement, pl, pl.label());
                            }
                        })
                });
                if p.placement == CalloutPlacement::Custom {
                    self.fields
                        .length_row(ui, "Offset from Center", "callout_offset", &mut p.offset);
                }
            }
            row(ui, "Callout Label", |ui| {
                ui.add(egui::TextEdit::singleline(&mut p.callout_label).desired_width(160.0))
            });
            row(ui, "Text Below Line", |ui| {
                ui.add_enabled(
                    !p.text_below_auto,
                    egui::TextEdit::singleline(&mut p.text_below).desired_width(160.0),
                )
            });
            ui.checkbox(&mut p.text_below_auto, "Automatic Text Below Line")
                .on_hover_text(format!(
                    "The camera's name, {name}, or the layout page label once the view is on a sheet"
                ));
            let mut auto = p.callout_size.is_none();
            ui.checkbox(&mut auto, "Automatic Callout Size");
            if auto {
                p.callout_size = None;
            } else {
                let mut size = p.callout_size.unwrap_or(24.0);
                self.fields
                    .length_row(ui, "Callout Size", "callout_size", &mut size);
                p.callout_size = Some(size.max(1.0));
            }
            row(ui, "Callout Arrow", |ui| {
                egui::ComboBox::from_id_salt("callout_arrow")
                    .selected_text(p.arrow.label())
                    .show_ui(ui, |ui| {
                        for a in CalloutArrow::ALL {
                            ui.selectable_value(&mut p.arrow, a, a.label());
                        }
                    })
            });
            ui.add_enabled(
                p.arrow != CalloutArrow::None,
                egui::Checkbox::new(&mut p.arrow_filled, "Filled"),
            );
            if section_cam && (p.placement.draws_line() || self.draft.view.clip.plane.is_stepped())
            {
                let mut by_layer = p.line_style.is_none();
                ui.checkbox(&mut by_layer, "Cross Section Line Style By Layer");
                if by_layer {
                    p.line_style = None;
                } else {
                    let mut cur = p.line_style.clone().unwrap_or_else(|| "Solid".into());
                    row(ui, "Cross Section Line Style", |ui| {
                        egui::ComboBox::from_id_salt("callout_line_style")
                            .selected_text(cur.clone())
                            .show_ui(ui, |ui| {
                                for s in LINE_STYLES {
                                    ui.selectable_value(&mut cur, s.to_string(), s);
                                }
                            })
                    });
                    p.line_style = Some(cur);
                }
                let mut by_layer = p.line_weight.is_none();
                ui.checkbox(&mut by_layer, "Cross Section Line Weight By Layer");
                if by_layer {
                    p.line_weight = None;
                } else {
                    let mut w = p.line_weight.unwrap_or(0.5);
                    pen_row(ui, "Cross Section Line Weight", &mut w, 0.05..=6.0);
                    p.line_weight = Some(w);
                }
            }
        });
        if !section_cam && !walkthrough {
            section(ui, "Standard Options");
            let mut default_size = p.symbol_size.is_none();
            ui.checkbox(&mut default_size, "Default Camera Symbol Size");
            if default_size {
                p.symbol_size = None;
            } else {
                let mut size = p.symbol_size.unwrap_or(24.0);
                self.fields
                    .length_row(ui, "Camera Symbol Size", "symbol_size", &mut size);
                p.symbol_size = Some(size.max(1.0));
            }
            ui.checkbox(&mut p.show_focal_point, "Show Camera Focal Point");
            ui.checkbox(&mut p.show_fov_indicators, "Show Field of View Indicators");
            let mut default_len = p.fov_length.is_none();
            ui.checkbox(&mut default_len, "Default FOV Indicator Length");
            if default_len {
                p.fov_length = None;
            } else {
                let mut len = p.fov_length.unwrap_or(DEFAULT_CONE_LENGTH);
                self.fields
                    .length_row(ui, "FOV Indicator Length", "fov_length", &mut len);
                p.fov_length = Some(len.max(1.0));
            }
        }
    }

    /// The Layer panel: the layer of the camera symbol, and its Drawing
    /// Group.
    pub(super) fn layer_tab(&mut self, ui: &mut egui::Ui) {
        section(ui, "Layer");
        let names: Vec<String> = if self.layer_names.is_empty() {
            vec![plan_core::camera_view::spec::CAMERA_LAYER.to_string()]
        } else {
            self.layer_names.clone()
        };
        let l = &mut self.draft.view.layer;
        row(ui, "Layer", |ui| {
            egui::ComboBox::from_id_salt("camera_layer")
                .selected_text(l.layer.clone())
                .width(220.0)
                .show_ui(ui, |ui| {
                    for n in &names {
                        ui.selectable_value(&mut l.layer, n.clone(), n);
                    }
                })
        });
        section(ui, "Drawing Group");
        let mut own = l.drawing_group.is_some();
        ui.checkbox(&mut own, "Set Drawing Group")
            .on_hover_text("Objects in a lower group draw behind those in a higher one");
        if own {
            let mut g = l.drawing_group.unwrap_or(95);
            row(ui, "Drawing Group", |ui| {
                ui.add(egui::DragValue::new(&mut g).range(0..=999))
            });
            l.drawing_group = Some(g);
        } else {
            l.drawing_group = None;
            ui.weak("The camera uses the group of cameras in Default Settings > Drawing Groups.");
        }
    }
}

impl CameraDialog {
    /// The lists the Selected Defaults panel draws, and the plan's own
    /// choice, handed over by the host before each frame.
    pub fn set_defaults_env(&mut self, env: default_sets::Env, plan: Selected) {
        self.defaults_env = Some(env);
        self.plan_selected = plan;
    }

    /// Does the open tab need the plan's lists (the host fills them on
    /// demand)?
    pub fn wants_defaults_env(&self) -> bool {
        self.last_tab == super::TAB_SELECTED_DEFAULTS
    }

    /// What the Selected Defaults panel asked for since the last call (Add,
    /// Edit, Rename and Delete act on the plan's lists, so the host runs
    /// them).
    pub fn take_defaults_events(&mut self) -> Vec<default_sets::Ev> {
        std::mem::take(&mut self.defaults_events)
    }

    /// The Selected Defaults draft, for the host to hand to
    /// `default_sets::handle_event`.
    pub fn selected_draft(&self) -> Selected {
        selected_of(&self.draft.view.selected, &self.plan_selected)
    }

    /// Takes the draft back after the host ran an event on it.
    pub fn set_selected_draft(&mut self, sel: &Selected) {
        self.draft.view.selected = view_defaults_of(sel, &self.plan_selected);
    }

    /// The names of the plan's layers, for the Layer panel.
    pub fn with_layer_names(mut self, names: Vec<String>) -> Self {
        self.layer_names = names;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::camera_view::ViewDefaults;
    use plan_core::geometry::Point;

    fn plan() -> Selected {
        Selected {
            default_set: String::new(),
            picks: [("text".to_string(), "Standard".to_string())].into(),
            layer_set: "Working".into(),
            cad_layer: "CAD, Default".into(),
        }
    }

    #[test]
    fn a_view_that_follows_the_plan_stores_nothing() {
        let plan = plan();
        let shown = selected_of(&ViewDefaults::default(), &plan);
        assert_eq!(shown, plan, "the panel shows what the plan uses");
        assert!(view_defaults_of(&shown, &plan).is_plan_default());
        // A change is kept, and empty picks are not.
        let mut own = shown;
        own.cad_layer = "Detail".into();
        own.picks.insert("notes".into(), String::new());
        let stored = view_defaults_of(&own, &plan);
        assert_eq!(stored.cad_layer, "Detail");
        assert_eq!(stored.picks.len(), 1);
        assert_eq!(selected_of(&stored, &plan).cad_layer, "Detail");
    }

    fn draw(d: &mut CameraDialog) {
        let ctx = egui::Context::default();
        for tab in 0..TABS.len() {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    use super::super::super::SpecPages;
                    d.page(ui, tab)
                });
            });
        }
    }

    #[test]
    fn the_new_panels_draw_for_every_camera_and_keep_what_was_set() {
        let mut cams = vec![
            CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "full", 0),
            CameraObject::new(CameraKind::PerspectiveOverview, Point::ZERO, 0.0, "ov", 0),
            CameraObject::new(
                CameraKind::CrossSection { back_clip: None },
                Point::ZERO,
                0.0,
                "x",
                0,
            ),
            CameraObject::walkthrough(vec![Point::ZERO, Point::new(100.0, 0.0)], 60.0, "walk", 0),
        ];
        for c in &mut cams {
            let v = &mut c.view;
            v.options.hide_facing_walls = true;
            v.options.dof_on = true;
            v.depth_cue.on = true;
            v.below_grade.override_weight = true;
            v.plan.all_floors = true;
            v.layer.drawing_group = Some(40);
            let mut d = CameraDialog::new(c, "1st Floor", CameraExtras::default())
                .with_layer_names(vec!["Cameras".into(), "Detail".into()]);
            d.set_defaults_env(
                default_sets::Env {
                    names: Default::default(),
                    sets: Vec::new(),
                    layer_sets: vec!["Working".into()],
                    cad_layers: vec!["Detail".into()],
                },
                plan(),
            );
            draw(&mut d);
            let v = &d.draft().view;
            assert!(v.options.hide_facing_walls && v.depth_cue.on && v.plan.all_floors);
            assert_eq!(v.layer.drawing_group, Some(40));
            assert!(
                v.selected.is_plan_default(),
                "drawing the panel changes nothing"
            );
            assert!(d.take_defaults_events().is_empty());
        }
    }
}
