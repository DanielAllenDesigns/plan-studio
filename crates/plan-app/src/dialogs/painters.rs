//! The painter bar and the Object Painter Modes dialog (LAY-18, S-116).
//!
//! While a painter tool is active a small bar floats at the top of the
//! drawing area: for the Layer Painter the layer to paint with and the scope
//! (Component / Object); for the Object Painter the attributes loaded by the
//! Object Eyedropper, the scope and a button for the Modes dialog. Both bars
//! switch between painting and eyedropping. The Modes dialog holds the Object
//! Painter's Component / Object / Room / Floor / Plan scope and the Apply to
//! all of type switch.

use crate::editor::EditorContext;
use crate::tools::painters::{with_state, LayerScope, ObjectScope, PainterMode};
use crate::tools::ToolId;
use eframe::egui::{self, Align2, Vec2};
use std::cell::Cell;

thread_local! {
    /// The painter the bar asked to switch to (taken by the tool's frame).
    static SWITCH: Cell<Option<ToolId>> = const { Cell::new(None) };
}

/// The painter tool the bar asked for since the last call.
pub fn take_switch() -> Option<ToolId> {
    SWITCH.with(Cell::take)
}

/// Draws the bar at the top of the drawing area `painter` paints into and,
/// when open, the Modes dialog. A click on Paint or Eyedropper leaves the
/// tool to switch to in [`take_switch`].
pub fn show(painter: &egui::Painter, cx: &EditorContext, mode: PainterMode) {
    let ctx = painter.ctx();
    let mut switch = None;
    let top = painter.clip_rect().center_top() + Vec2::new(0.0, 6.0);
    egui::Area::new(egui::Id::new("painter_bar"))
        .order(egui::Order::Foreground)
        .fixed_pos(top)
        .pivot(Align2::CENTER_TOP)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| bar(ui, cx, mode, &mut switch));
            });
        });
    modes_window(ctx);
    if switch.is_some() {
        SWITCH.with(|s| s.set(switch));
    }
}

fn bar(ui: &mut egui::Ui, cx: &EditorContext, mode: PainterMode, switch: &mut Option<ToolId>) {
    ui.strong(mode.name());
    let (paint, dropper) = if mode.is_layer() {
        (PainterMode::LayerPaint, PainterMode::LayerEyedropper)
    } else {
        (PainterMode::ObjectPaint, PainterMode::ObjectEyedropper)
    };
    for m in [paint, dropper] {
        let label = if m == paint { "Paint" } else { "Eyedropper" };
        if ui.selectable_label(mode == m, label).clicked() && mode != m {
            *switch = Some(ToolId::PainterVariant(m));
        }
    }
    ui.separator();
    if mode.is_layer() {
        layer_controls(ui, cx);
    } else {
        object_controls(ui);
    }
}

fn layer_controls(ui: &mut egui::Ui, cx: &EditorContext) {
    let mut layer = with_state(|s| s.layer.clone());
    ui.label("Layer:");
    egui::ComboBox::from_id_salt("painter_layer")
        .selected_text(layer.clone().unwrap_or_else(|| "(none)".into()))
        .width(170.0)
        .show_ui(ui, |ui| {
            for l in &cx.project.layers.layers {
                ui.selectable_value(&mut layer, Some(l.name.clone()), &l.name);
            }
        });
    with_state(|s| s.layer = layer);
    ui.separator();
    ui.label("Scope:");
    let mut scope = with_state(|s| s.layer_scope);
    for sc in LayerScope::ALL {
        ui.selectable_value(&mut scope, sc, sc.name());
    }
    with_state(|s| s.layer_scope = scope);
}

fn object_controls(ui: &mut egui::Ui) {
    let source = with_state(|s| s.source.as_ref().map(|a| a.summary.clone()));
    ui.label(match source {
        Some(s) => format!("Loaded: {s}"),
        None => "Loaded: nothing (use the Object Eyedropper)".to_string(),
    });
    ui.separator();
    let mut scope = with_state(|s| s.scope);
    ui.label("Scope:");
    egui::ComboBox::from_id_salt("painter_scope")
        .selected_text(scope.name())
        .show_ui(ui, |ui| {
            for sc in ObjectScope::ALL {
                ui.selectable_value(&mut scope, sc, sc.name());
            }
        });
    with_state(|s| s.scope = scope);
    if ui.button("Modes\u{2026}").clicked() {
        with_state(|s| s.modes_open = true);
    }
}

/// The Object Painter Modes dialog.
fn modes_window(ctx: &egui::Context) {
    if !with_state(|s| s.modes_open) {
        return;
    }
    let mut open = true;
    let mut close = false;
    egui::Window::new("Object Painter Modes")
        .id(egui::Id::new("object_painter_modes"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            let mut scope = with_state(|s| s.scope);
            let mut all = with_state(|s| s.all_of_type);
            ui.label("Paint the attributes loaded by the Object Eyedropper onto:");
            for sc in ObjectScope::ALL {
                ui.radio_value(&mut scope, sc, sc.name())
                    .on_hover_text(sc.describe());
                ui.indent(sc.name(), |ui| {
                    ui.weak(sc.describe());
                });
            }
            ui.separator();
            ui.checkbox(&mut all, "Apply to all of type").on_hover_text(
                "On: Room, Floor and Plan reach every object of the kind. Off: only objects \
                     like the one clicked (same wall type, symbol, cabinet kind, ...).",
            );
            ui.weak("Position, size and identity (marks, labels, text) are never copied.");
            with_state(|s| {
                s.scope = scope;
                s.all_of_type = all;
            });
            ui.separator();
            close = ui.button("Close").clicked();
        });
    if !open || close {
        with_state(|s| s.modes_open = false);
    }
}
