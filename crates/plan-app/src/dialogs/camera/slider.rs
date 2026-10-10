//! The Cross Section Slider dialog (manual pp. 1176, 1177; C-141): several
//! cutting planes, each with a check box, a Position slider and a typed
//! Position measured from the edge of the model the plane cuts first. It
//! stays open while the user keeps working in the view; Done closes it.

use super::{row, Fields};
use eframe::egui;
use plan_core::camera_view::{CrossSectionSlider, SliderSide};

/// Draws the dialog for `planes` over a model that spans `lo` to `hi` (plan
/// X, plan Y, height). Returns `(done, changed)`.
pub fn show(
    ctx: &egui::Context,
    fields: &mut Fields,
    planes: &mut CrossSectionSlider,
    lo: [f64; 3],
    hi: [f64; 3],
) -> (bool, bool) {
    let mut done = false;
    let mut changed = false;
    egui::Window::new("Cross Section Slider")
        .id(egui::Id::new("cross_section_slider_window"))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.weak("Check a cutting plane, then move its Position.");
            const KEYS: [&str; 6] = [
                "slide_left",
                "slide_right",
                "slide_front",
                "slide_back",
                "slide_top",
                "slide_bottom",
            ];
            for (side, key) in SliderSide::ALL.iter().zip(KEYS) {
                let max = CrossSectionSlider::max_position(*side, lo, hi);
                let Some(plane) = planes.planes.iter().find(|p| p.side == *side).copied() else {
                    continue;
                };
                let (mut on, mut pos) = (plane.on, plane.position.min(max));
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut on, side.label()).changed() {
                        planes.set_on(*side, on);
                        changed = true;
                    }
                });
                ui.add_enabled_ui(on, |ui| {
                    row(ui, "Position", |ui| {
                        if ui
                            .add(egui::Slider::new(&mut pos, 0.0..=max).show_value(false))
                            .changed()
                        {
                            planes.set_position(*side, pos);
                            changed = true;
                        }
                        if fields.length(ui, key, &mut pos) {
                            planes.set_position(*side, pos.clamp(0.0, max));
                            changed = true;
                        }
                    });
                });
            }
            ui.separator();
            if ui.button("Done").clicked() {
                done = true;
            }
        });
    (done, changed)
}
