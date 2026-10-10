//! The Truss Detail window: every truss configuration of the plan once, with
//! its label, quantity and web layout, and the Find Trusses, Open Truss
//! Detail and Force Truss Rebuild buttons (reference manual pp. 941 to 944).
//!
//! The same data fills the Truss Detail CAD detail
//! (`framing_view::details`) and the truss rows of the framing schedule.

use crate::editor::{framing_view, EditorContext};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use plan_framing::TrussConfig;
use std::cell::Cell;

thread_local! {
    static OPEN: Cell<bool> = const { Cell::new(false) };
}

/// Opens the window.
pub fn open() {
    OPEN.with(|o| o.set(true));
}

/// Closes the window.
pub fn close() {
    OPEN.with(|o| o.set(false));
}

/// Whether the window is open.
pub fn is_open() -> bool {
    OPEN.with(Cell::get)
}

/// Paints the diagram of `c` inside `rect`.
pub fn paint_config(painter: &egui::Painter, rect: Rect, c: &TrussConfig, ink: Color32) {
    let (w, h) = (c.overall_width().max(1.0), c.height().max(1.0));
    let min_x = c
        .members
        .iter()
        .flat_map(|m| [m.a.x, m.b.x])
        .fold(0.0, f64::min);
    let scale = f64::from(rect.width() - 8.0) / w;
    let scale = scale.min(f64::from(rect.height() - 8.0) / h);
    let to = |x: f64, y: f64| {
        Pos2::new(
            rect.min.x + 4.0 + ((x - min_x) * scale) as f32,
            rect.max.y - 4.0 - (y * scale) as f32,
        )
    };
    for m in &c.members {
        painter.line_segment(
            [to(m.a.x, m.a.y), to(m.b.x, m.b.y)],
            Stroke::new(1.1_f32, ink),
        );
    }
}

/// Draws the window when it is open. Called every frame with the other
/// framing dialogs (`dialogs::framing::host_frame`).
pub fn host_frame(cx: &mut EditorContext, ctx: &egui::Context) {
    if !is_open() {
        return;
    }
    let configs = framing_view::truss_configs_of(&cx.project);
    let mut open = true;
    let mut find: Option<String> = None;
    let mut detail: Option<String> = None;
    let mut rebuild: Option<String> = None;
    egui::Window::new("Truss Detail")
        .open(&mut open)
        .default_width(560.0)
        .show(ctx, |ui| {
            if configs.is_empty() {
                ui.label("There are no trusses in the plan.");
                return;
            }
            ui.weak("Each truss configuration is drawn once. Chief Architect does not engineer trusses: have a licensed engineer approve every design.");
            let ink = ui.visuals().text_color();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for c in &configs {
                    ui.separator();
                    ui.horizontal(|ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(200.0, 64.0), egui::Sense::hover());
                        paint_config(&ui.painter_at(rect), rect, c, ink);
                        ui.vertical(|ui| {
                            let title = if c.count > 1 {
                                format!("{} ({})", c.label, c.count)
                            } else {
                                c.label.clone()
                            };
                            ui.strong(title);
                            ui.label(format!(
                                "Span {}, {} members",
                                crate::dialogs::fmt_short(c.span),
                                c.members.len()
                            ));
                            ui.horizontal(|ui| {
                                if ui.button("Find Trusses").clicked() {
                                    find = Some(c.label.clone());
                                }
                                if ui.button("Open Truss Detail").clicked() {
                                    detail = Some(c.label.clone());
                                }
                                if ui
                                    .add_enabled(
                                        !c.ids.is_empty(),
                                        egui::Button::new("Force Truss Rebuild"),
                                    )
                                    .clicked()
                                {
                                    rebuild = Some(c.label.clone());
                                }
                            });
                        });
                    });
                }
            });
        });
    if !open {
        close();
    }
    if let Some(label) = find {
        framing_view::find_config(cx, &label);
    }
    if let Some(label) = detail {
        let first = configs
            .iter()
            .find(|c| c.label == label)
            .and_then(|c| c.ids.first().copied());
        framing_view::open_truss_detail(cx, first);
    }
    if let Some(label) = rebuild {
        if framing_view::find_config(cx, &label) > 0 {
            framing_view::build_selected(cx, None);
        }
    }
}
