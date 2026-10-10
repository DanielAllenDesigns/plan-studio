//! Default Settings > Terrain > Terrain Defaults (CB-51): the Terrain
//! Specification, opened from the Default Settings tree. The terrain settings
//! (contour interval, grid spacing, subdivision, smoothing, subfloor height,
//! building pad elevation, north angle, auto-rebuild, layer) are kept with the
//! plan, so this page edits the plan's terrain record and starts a record with
//! the built-in values when the plan has none yet, the same as Terrain >
//! Terrain Specification. OK is one undo step, "Terrain Defaults".

use super::terrain::TerrainDialog;
use super::Outcome;
use crate::editor::{site_view, EditorContext};
use eframe::egui;
use std::cell::{Cell, RefCell};

thread_local! {
    static OPEN: Cell<bool> = const { Cell::new(false) };
    static PAGE: RefCell<Option<TerrainDialog>> = const { RefCell::new(None) };
}

/// Asks for the page; [`show`] opens it on its next frame.
pub fn request_open() {
    OPEN.with(|c| c.set(true));
}

/// Is the page asked for or showing?
#[cfg(test)]
pub fn is_open() -> bool {
    OPEN.with(Cell::get) || PAGE.with(|p| p.borrow().is_some())
}

/// Draws the page (when open) and applies OK.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    if OPEN.with(|c| c.replace(false)) {
        let rec = site_view::load_terrain(&cx.project).unwrap_or_default();
        PAGE.with(|p| *p.borrow_mut() = Some(TerrainDialog::new(&rec)));
    }
    let Some(mut dlg) = PAGE.with(|p| p.borrow_mut().take()) else {
        return;
    };
    match dlg.show(ctx) {
        Outcome::Open => PAGE.with(|p| *p.borrow_mut() = Some(dlg)),
        Outcome::Cancel => {}
        Outcome::Ok => {
            let draft = dlg.draft().clone();
            site_view::edit_terrain(cx, "Terrain Defaults", |rec| rec.apply_spec(&draft));
            cx.status = "Saved the terrain defaults".into();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Key;

    fn frame(ctx: &egui::Context, cx: &mut EditorContext, key: Option<Key>) {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 800.0),
            )),
            ..Default::default()
        };
        if let Some(key) = key {
            input.events.push(egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
        }
        let _ = ctx.run(input, |ctx| show(ctx, cx));
    }

    #[test]
    fn the_page_opens_and_closes_with_the_keys_and_ok_is_one_undo_step() {
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        let ctx = egui::Context::default();
        assert!(!is_open());
        request_open();
        assert!(is_open());
        frame(&ctx, &mut cx, None);
        frame(&ctx, &mut cx, None);
        assert!(is_open(), "the dialog stays up until answered");
        // Escape cancels and changes nothing.
        let undo_before = cx.can_undo();
        frame(&ctx, &mut cx, Some(Key::Escape));
        frame(&ctx, &mut cx, None);
        assert!(!is_open());
        assert_eq!(cx.can_undo(), undo_before);
        assert!(site_view::load_terrain(&cx.project).is_none());
        // Enter is OK: the terrain record now exists, as one undo step.
        request_open();
        frame(&ctx, &mut cx, None);
        frame(&ctx, &mut cx, None);
        frame(&ctx, &mut cx, Some(Key::Enter));
        frame(&ctx, &mut cx, None);
        assert!(!is_open());
        assert!(site_view::load_terrain(&cx.project).is_some());
        assert_eq!(cx.status, "Saved the terrain defaults");
        assert_eq!(cx.undo().as_deref(), Some("Terrain Defaults"));
        assert!(site_view::load_terrain(&cx.project).is_none());
    }
}
