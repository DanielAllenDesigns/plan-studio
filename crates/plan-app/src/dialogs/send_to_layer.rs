//! Edit toolbar > Layer: a small window that lists the layers; picking one
//! moves the selected walls, CAD, text and symbols onto it (one undo step).

use crate::editor::EditorContext;
use eframe::egui;
use std::cell::RefCell;

use super::select_layer::{send_selection, SelectLayer};

/// The Send to Layer window: the Select Layer panel with Use Default Layer.
pub type LayerPicker = SelectLayer;

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
            *d = Some(SelectLayer::new(first));
        }
    });
}

impl SelectLayer {
    /// Draws the window; false once it is closed.
    pub fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) -> bool {
        let mut open = true;
        let mut ok = false;
        let mut cancel = false;
        egui::Window::new("Select Layer")
            .id(egui::Id::new("send_to_layer"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Move the selected objects to layer:");
                self.panel(ui, cx, true);
                ui.horizontal(|ui| {
                    ok = ui
                        .add_enabled(self.ready(), egui::Button::new("OK"))
                        .clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if ok {
            send_selection(cx, self);
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
