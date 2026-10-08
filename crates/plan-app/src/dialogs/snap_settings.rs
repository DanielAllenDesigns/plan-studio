//! Edit > Snap Settings (S-68..S-72, S-74, CAD-40): the object snaps on and
//! off and one by one, grid snaps, angle snaps with the list of allowed
//! angles, the bumping/pushing distance and the snap tolerance in pixels.
//! The values live in `PlanDefaults::editing` (and `grid`), so they save with
//! the template; the snap engine reads them on every pointer move.

use crate::editor::EditorContext;
use eframe::egui;
use plan_core::EditingDefaults;

/// Angle increments the combo offers, degrees.
pub const INCREMENTS: [f64; 6] = [5.0, 10.0, 15.0, 30.0, 45.0, 90.0];

/// What the dialog edits: a copy of the snap values, written back on OK.
#[derive(Clone, Debug, PartialEq)]
pub struct SnapDraft {
    pub editing: EditingDefaults,
    /// Grid snap unit, inches.
    pub grid_snap: f64,
    /// The allowed-angles text as typed (`0, 45, 90`).
    pub angles_text: String,
}

/// `0, 45, 90` as degrees; `None` when a piece is not a number.
pub fn parse_angles(text: &str) -> Option<Vec<f64>> {
    let mut out = Vec::new();
    for piece in text
        .split([',', ';', ' '])
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        let v: f64 = piece.trim_end_matches('\u{b0}').parse().ok()?;
        out.push(v.rem_euclid(360.0));
    }
    Some(out)
}

fn format_angles(angles: &[f64]) -> String {
    angles
        .iter()
        .map(|a| format!("{a}"))
        .collect::<Vec<_>>()
        .join(", ")
}

impl SnapDraft {
    pub fn from_context(cx: &EditorContext) -> Self {
        Self {
            editing: cx.defaults.editing.clone(),
            grid_snap: cx.defaults.grid.snap,
            angles_text: format_angles(&cx.defaults.editing.snap_angles),
        }
    }

    /// Why OK is not allowed, if so.
    pub fn error(&self) -> Option<String> {
        if parse_angles(&self.angles_text).is_none() {
            return Some("The allowed angles are degrees separated by commas".into());
        }
        if self.grid_snap < 0.0 {
            return Some("The grid snap cannot be negative".into());
        }
        None
    }

    /// Stores the draft in the plan defaults (the snap engine reads them from
    /// there). Returns false while the draft has an error.
    pub fn apply(&self, cx: &mut EditorContext) -> bool {
        let Some(angles) = parse_angles(&self.angles_text) else {
            return false;
        };
        if self.error().is_some() {
            return false;
        }
        let mut e = self.editing.clone();
        e.snap_angles = angles;
        // Both places carry the angle increment (the engine reads the grid's).
        cx.defaults.grid.angle_snap_deg = e.angle_snap_deg;
        cx.defaults.grid.snap = self.grid_snap;
        cx.defaults.editing = e;
        cx.status = "Snap settings updated".into();
        true
    }
}

pub struct SnapSettingsDialog {
    pub draft: SnapDraft,
    fields: super::Fields,
}

impl SnapSettingsDialog {
    pub fn new(cx: &EditorContext) -> Self {
        Self {
            draft: SnapDraft::from_context(cx),
            fields: super::Fields::default(),
        }
    }

    /// Draws the window; false once it is closed (OK or Cancel).
    pub fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) -> bool {
        let mut open = true;
        let mut ok = false;
        let mut cancel = false;
        let mut defaults = false;
        egui::Window::new("Snap Settings")
            .id(egui::Id::new("snap_settings"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                let e = &mut self.draft.editing;
                ui.checkbox(&mut e.object_snaps, "Object Snaps");
                ui.add_enabled_ui(e.object_snaps, |ui| {
                    ui.indent("object_snaps", |ui| {
                        egui::Grid::new("object_snap_kinds")
                            .num_columns(2)
                            .show(ui, |ui| {
                                ui.checkbox(&mut e.snap_endpoint, "Endpoint");
                                ui.checkbox(&mut e.snap_midpoint, "Midpoint");
                                ui.end_row();
                                ui.checkbox(&mut e.snap_intersection, "Intersection");
                                ui.checkbox(&mut e.snap_on_object, "On Object");
                                ui.end_row();
                                ui.checkbox(&mut e.snap_center, "Center");
                                ui.checkbox(&mut e.snap_quadrant, "Quadrant");
                                ui.end_row();
                                ui.checkbox(&mut e.snap_perpendicular, "Perpendicular");
                                ui.checkbox(&mut e.snap_tangent, "Tangent");
                                ui.end_row();
                                ui.checkbox(&mut e.snap_extension, "Extension");
                                ui.checkbox(&mut e.snap_markers, "Points/Markers");
                                ui.end_row();
                            });
                    });
                });
                ui.separator();
                ui.checkbox(&mut e.grid_snaps, "Grid Snaps");
                ui.add_enabled_ui(e.grid_snaps, |ui| {
                    super::row(ui, "Grid Snap Unit", |ui| {
                        self.fields
                            .length(ui, "grid_snap", &mut self.draft.grid_snap)
                    });
                });
                ui.separator();
                ui.checkbox(&mut e.angle_snaps, "Angle Snaps");
                ui.add_enabled_ui(e.angle_snaps, |ui| {
                    super::row(ui, "Angle Increment", |ui| {
                        let label = format!("{}\u{b0}", e.angle_snap_deg);
                        egui::ComboBox::from_id_salt("angle_increment")
                            .selected_text(label)
                            .show_ui(ui, |ui| {
                                for inc in INCREMENTS {
                                    ui.selectable_value(
                                        &mut e.angle_snap_deg,
                                        inc,
                                        format!("{inc}\u{b0}"),
                                    );
                                }
                            });
                    });
                    super::row(ui, "Allowed Angles", |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.draft.angles_text)
                                .hint_text("every increment")
                                .desired_width(140.0),
                        );
                    });
                    ui.weak("Degrees counter-clockwise from east; each also allows its opposite.");
                });
                ui.separator();
                ui.checkbox(&mut e.bumping, "Bumping/Pushing");
                ui.add_enabled_ui(e.bumping, |ui| {
                    super::row(ui, "Bumping Distance", |ui| {
                        self.fields.length(ui, "bumping", &mut e.bumping_distance)
                    });
                });
                super::row(ui, "Snap Distance (pixels)", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut e.snap_distance_px)
                            .range(2.0..=40.0)
                            .speed(0.25),
                    );
                });
                ui.weak("Hold Alt while drawing or dragging to suspend every snap.");
                if let Some(err) = self.draft.error() {
                    ui.colored_label(egui::Color32::from_rgb(0xC0, 0x30, 0x30), err);
                }
                ui.horizontal(|ui| {
                    let valid = self.draft.error().is_none() && !self.fields.any_invalid();
                    ok = ui.add_enabled(valid, egui::Button::new("OK")).clicked();
                    cancel = ui.button("Cancel").clicked();
                    defaults = ui.button("Reset").clicked();
                });
            });
        // Enter is OK and Esc is Cancel, as in the other dialogs.
        let (enter, esc) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
            )
        });
        let ok = ok || (enter && self.draft.error().is_none() && !self.fields.any_invalid());
        let cancel = cancel || esc;
        if defaults {
            let mut fresh = SnapDraft::from_context(cx);
            fresh.editing = EditingDefaults {
                behavior: fresh.editing.behavior.clone(),
                ..EditingDefaults::default()
            };
            fresh.angles_text.clear();
            self.draft = fresh;
        }
        if ok {
            self.draft.apply(cx);
        }
        open && !ok && !cancel
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::snap::SnapKind;
    use crate::plan_defaults;
    use plan_core::geometry::Point;
    use plan_core::{CadItem, WallKind};

    fn cx() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        cx
    }

    #[test]
    fn angles_parse_and_normalize() {
        assert_eq!(parse_angles("0, 45; 90"), Some(vec![0.0, 45.0, 90.0]));
        assert_eq!(parse_angles(" -45 360 "), Some(vec![315.0, 0.0]));
        assert_eq!(parse_angles(""), Some(vec![]));
        assert_eq!(parse_angles("0, x"), None);
    }

    #[test]
    fn changing_the_object_snaps_changes_what_the_engine_returns() {
        let mut cx = cx();
        let raw = Point::new(121.3, 2.2);
        assert_eq!(cx.snap_at(raw, None, false, &[]).kind, SnapKind::Endpoint);
        let mut d = SnapDraft::from_context(&cx);
        d.editing.snap_endpoint = false;
        assert!(d.apply(&mut cx));
        assert_eq!(cx.snap_at(raw, None, false, &[]).kind, SnapKind::OnObject);
        d.editing.object_snaps = false;
        assert!(d.apply(&mut cx));
        assert_eq!(cx.snap_at(raw, None, false, &[]).kind, SnapKind::Grid);
        d.editing.grid_snaps = false;
        assert!(d.apply(&mut cx));
        assert_eq!(cx.snap_at(raw, None, false, &[]).kind, SnapKind::Free);
    }

    #[test]
    fn tolerance_in_pixels_sets_the_reach() {
        let mut cx = cx();
        let raw = Point::new(124.0, 0.0);
        // 10 px at 2 px/in reaches 5": the endpoint is 4" away.
        assert_eq!(cx.snap_at(raw, None, false, &[]).kind, SnapKind::Endpoint);
        let mut d = SnapDraft::from_context(&cx);
        d.editing.snap_distance_px = 4.0;
        d.apply(&mut cx);
        assert_eq!(cx.snap_at(raw, None, false, &[]).kind, SnapKind::Grid);
    }

    #[test]
    fn center_and_tangent_snaps_follow_their_switches() {
        let mut cx = cx();
        cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Circle {
                center: Point::new(300.0, 300.0),
                radius: 40.0,
            },
        );
        let raw = Point::new(301.0, 299.0);
        assert_eq!(cx.snap_at(raw, None, false, &[]).kind, SnapKind::Center);
        let mut d = SnapDraft::from_context(&cx);
        d.editing.snap_center = false;
        d.apply(&mut cx);
        assert_eq!(cx.snap_at(raw, None, false, &[]).kind, SnapKind::Grid);
    }

    #[test]
    fn the_allowed_angles_list_and_increment_are_stored_and_used() {
        let mut cx = cx();
        let start = Some(Point::new(0.0, 100.0));
        let raw = Point::new(100.0, 100.0 + 100.0 * 40.0f64.to_radians().tan());
        let by_inc = cx.snap_at(raw, start, false, &[]);
        assert_eq!(by_inc.kind, SnapKind::Angle);
        assert!(
            (by_inc.point.y - 100.0 - by_inc.point.x).abs() < 1e-6,
            "45 degrees"
        );
        let mut d = SnapDraft::from_context(&cx);
        d.angles_text = "0, 90".into();
        d.editing.angle_snap_deg = 30.0;
        assert!(d.apply(&mut cx));
        assert_eq!(cx.defaults.editing.snap_angles, vec![0.0, 90.0]);
        assert_eq!(cx.defaults.grid.angle_snap_deg, 30.0);
        let by_list = cx.snap_at(raw, start, false, &[]);
        assert!(
            (by_list.point.y - 100.0).abs() < 1e-6,
            "{:?}",
            by_list.point
        );
        // Angle snaps off: the grid takes over.
        d.editing.angle_snaps = false;
        d.apply(&mut cx);
        assert_eq!(cx.snap_at(raw, start, false, &[]).kind, SnapKind::Grid);
    }

    #[test]
    fn a_bad_angle_list_blocks_apply() {
        let mut cx = cx();
        let mut d = SnapDraft::from_context(&cx);
        d.angles_text = "zero".into();
        assert!(d.error().is_some());
        let before = cx.defaults.editing.clone();
        assert!(!d.apply(&mut cx));
        assert_eq!(cx.defaults.editing, before);
        d.angles_text.clear();
        d.grid_snap = -1.0;
        assert!(d.error().is_some());
    }

    #[test]
    fn the_window_draws_and_ok_applies() {
        let mut cx = cx();
        let mut dlg = SnapSettingsDialog::new(&cx);
        dlg.draft.editing.snap_midpoint = false;
        let ctx = egui::Context::default();
        let mut open = true;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            open = dlg.show(ctx, &mut cx);
        });
        assert!(open);
        // Nothing is stored until OK.
        assert!(cx.defaults.editing.snap_midpoint);
    }
}
