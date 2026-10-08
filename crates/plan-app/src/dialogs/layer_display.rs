//! Layer Display Options as a modal window: the same table and "Properties
//! for Selected Layer" as the dock panel, plus New Layer and Copy Layer Set.

use crate::editor::EditorContext;
use crate::shell::docks::{add_layer, layer_panel, LayerPanelState};
use eframe::egui::{self, Align, Align2, Key, Layout, Modifiers, Vec2};

#[derive(Default)]
pub struct LayerDisplayDialog {
    panel: LayerPanelState,
}

impl LayerDisplayDialog {
    /// New Layer...: adds a layer and selects it.
    pub fn new_layer(&mut self, cx: &mut EditorContext) -> String {
        let name = add_layer(cx);
        self.panel.filter.clear();
        self.panel.selected = Some(name.clone());
        name
    }

    /// Draws the window; returns false once it should close.
    pub fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) -> bool {
        let mut open = true;
        let mut close = false;
        egui::Window::new("Layer Display Options")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(420.0, 560.0))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                layer_panel(ui, cx, &mut self.panel, 0.5);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("New Layer\u{2026}").clicked() {
                        let name = self.new_layer(cx);
                        cx.status = format!("Added layer {name}");
                    }
                    if ui.button("Copy Layer Set\u{2026}").clicked() {
                        cx.status = "Copy Layer Set: coming".into();
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });
                });
            });
        if !ctx.wants_keyboard_input()
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            close = true;
        }
        open && !close
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;

    #[test]
    fn new_layer_is_selected_and_listed() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let mut d = LayerDisplayDialog::default();
        assert!(d.panel.selected.is_none());
        let name = d.new_layer(&mut cx);
        assert_eq!(d.panel.selected.as_deref(), Some(name.as_str()));
        assert!(cx.project.layers.get(&name).is_some());
    }
}
