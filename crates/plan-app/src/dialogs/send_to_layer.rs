//! Edit toolbar > Layer: a small window that lists the layers; picking one
//! moves the selected walls, CAD, text and symbols onto it (one undo step).

use crate::editor::EditorContext;
use eframe::egui;
use std::cell::RefCell;

#[derive(Clone, Debug, Default)]
pub struct LayerPicker {
    pub chosen: Option<String>,
    pub filter: String,
}

thread_local! {
    static DIALOG: RefCell<Option<LayerPicker>> = const { RefCell::new(None) };
}

/// Opens the picker for the selection (nothing happens without one).
pub fn open(cx: &mut EditorContext) {
    if cx.selection.is_empty() {
        cx.status = "Select objects first".into();
        return;
    }
    // The layer the objects are on starts chosen.
    let first = cx.selection_layers().into_iter().next();
    DIALOG.with(|d| {
        let mut d = d.borrow_mut();
        if d.is_none() {
            *d = Some(LayerPicker {
                chosen: first,
                filter: String::new(),
            });
        }
    });
}

impl LayerPicker {
    /// Draws the window; false once it is closed.
    pub fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) -> bool {
        let mut open = true;
        let mut ok = false;
        let mut cancel = false;
        egui::Window::new("Layer")
            .id(egui::Id::new("send_to_layer"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Move the selected objects to layer:");
                ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("Filter"));
                let filter = self.filter.to_lowercase();
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        for l in &cx.project.layers.layers {
                            if !filter.is_empty() && !l.name.to_lowercase().contains(&filter) {
                                continue;
                            }
                            let on = self.chosen.as_deref() == Some(l.name.as_str());
                            if ui.selectable_label(on, &l.name).clicked() {
                                self.chosen = Some(l.name.clone());
                            }
                        }
                    });
                ui.horizontal(|ui| {
                    ok = ui
                        .add_enabled(self.chosen.is_some(), egui::Button::new("OK"))
                        .clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if ok {
            if let Some(l) = self.chosen.clone() {
                cx.send_selection_to_layer(&l);
            }
            return false;
        }
        open && !cancel
    }
}

pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    if let Some(mut d) = DIALOG.with(|d| d.borrow_mut().take()) {
        if d.show(ctx, cx) {
            DIALOG.with(|slot| *slot.borrow_mut() = Some(d));
        }
    }
}
