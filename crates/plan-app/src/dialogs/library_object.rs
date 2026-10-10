//! Library Object Specification for a library object: the window the Library
//! Browser's **Open Object** opens (CB-57, CB-60).
//!
//! It is the same dialog a placed library object opens
//! (`dialogs::symbol::SymbolDialog`, tabs General, Options, Materials, Label,
//! Layer, Object Information, Schedule) but it edits the library item's own
//! defaults. OK on a User Catalog item saves the new size, elevation, layer,
//! label, schedule and options into it; built-in and Chief objects are
//! read-only (the dialog says so and offers Add to Library, which saves an
//! editable copy). One dialog is open at a time, drawn every frame by the
//! shell ([`show_all`]).

use super::symbol::SymbolDialog;
use super::Outcome;
use crate::editor::EditorContext;
use eframe::egui;
use std::cell::RefCell;

thread_local! {
    static OPEN: RefCell<Option<SymbolDialog>> = const { RefCell::new(None) };
}

fn layer_names(cx: &EditorContext) -> Vec<String> {
    cx.layers().layers.iter().map(|l| l.name.clone()).collect()
}

/// Opens the Library Object Specification of library item `id`; false (and a
/// status message) for an unknown id.
pub fn open_item(cx: &mut EditorContext, id: &str) -> bool {
    match SymbolDialog::for_library_item(id, layer_names(cx)) {
        Some(d) => {
            OPEN.with(|o| *o.borrow_mut() = Some(d));
            true
        }
        None => {
            cx.status = format!("Unknown library item: {id}");
            false
        }
    }
}

#[cfg(test)]
/// Is the dialog open?
pub fn is_open() -> bool {
    OPEN.with(|o| o.borrow().is_some())
}

#[cfg(test)]
/// The id of the item the open dialog edits.
pub fn open_id() -> Option<String> {
    OPEN.with(|o| {
        o.borrow()
            .as_ref()
            .and_then(|d| d.library_item_id().map(str::to_owned))
    })
}

#[cfg(test)]
/// Closes the dialog without saving.
pub fn close() {
    OPEN.with(|o| *o.borrow_mut() = None);
}

/// Saves what the dialog edits into the User Catalog; the status message says
/// what happened.
fn save(cx: &mut EditorContext, dlg: &SymbolDialog) {
    let Some(updated) = dlg.library_update() else {
        cx.status = "That library object is read-only".into();
        return;
    };
    cx.status = match crate::tools::library::user::update(updated) {
        Ok(i) => format!("Saved \"{}\" in the User Catalog", i.name),
        Err(e) => e,
    };
}

/// Draws the dialog when it is open and carries out OK.
pub fn show_all(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut dlg) = OPEN.with(|o| o.borrow_mut().take()) else {
        return;
    };
    match dlg.show(ctx) {
        Outcome::Open => OPEN.with(|o| *o.borrow_mut() = Some(dlg)),
        Outcome::Cancel => {}
        Outcome::Ok => save(cx, &dlg),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use crate::tools::library::user::{self as store, tests_support};
    use plan_library::{CatalogItem, Placement, Symbol2d};

    fn user_item(name: &str) -> CatalogItem {
        let mut i = CatalogItem::new(
            store::new_id(plan_library::ItemKind::Symbol),
            name,
            Placement::FreeStanding,
            Symbol2d::new(vec![plan_library::Stroke::Polyline {
                points: vec![
                    plan_core::geometry::Point::new(-10.0, -10.0),
                    plan_core::geometry::Point::new(10.0, -10.0),
                    plan_core::geometry::Point::new(10.0, 10.0),
                    plan_core::geometry::Point::new(-10.0, 10.0),
                ],
                closed: true,
            }]),
        )
        .with_category(&["User", "Symbols"])
        .with_size(20.0, 20.0, 30.0);
        i.tags = vec!["chair".into()];
        i
    }

    #[test]
    fn a_user_item_opens_edits_and_saves_its_defaults() {
        tests_support::fresh(false);
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let item = store::add(user_item("Stool"), None).unwrap();
        assert!(open_item(&mut cx, &item.id));
        assert!(is_open());
        assert_eq!(open_id().as_deref(), Some(item.id.as_str()));
        let mut dlg = OPEN.with(|o| o.borrow_mut().take()).unwrap();
        // Edit through the dialog's draft, as typing in the fields would.
        dlg.edit_draft(|d| {
            d.width = 40.0;
            d.height = 18.0;
            d.elevation = 2.0;
            d.flip = true;
            d.label = "S1".into();
            d.options.insert("hardware".into(), "Matte Black".into());
        });
        save(&mut cx, &dlg);
        let back = store::item(&item.id).unwrap();
        assert_eq!((back.width, back.depth, back.height), (40.0, 20.0, 18.0));
        assert_eq!(back.elevation, 2.0);
        let b = back.symbol.bounds().unwrap();
        assert!((b.width() - 40.0).abs() < 1e-9, "the drawing is stretched");
        let d = back.defaults.clone().unwrap();
        assert!(d.flip);
        assert_eq!(d.label, "S1");
        assert_eq!(
            d.options.get("hardware").map(String::as_str),
            Some("Matte Black")
        );
        // Reopening shows the saved defaults; undoing them leaves none.
        assert!(open_item(&mut cx, &item.id));
        let mut dlg = OPEN.with(|o| o.borrow_mut().take()).unwrap();
        assert_eq!(dlg.draft().label, "S1");
        dlg.edit_draft(|d| {
            d.flip = false;
            d.label.clear();
            d.options.clear();
        });
        save(&mut cx, &dlg);
        assert!(store::item(&item.id).unwrap().defaults.is_none());
    }

    #[test]
    fn built_in_objects_open_read_only() {
        tests_support::fresh(false);
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let core = crate::tools::library::library_catalog()
            .all_items()
            .next()
            .unwrap()
            .id
            .clone();
        assert!(open_item(&mut cx, &core));
        let dlg = OPEN.with(|o| o.borrow_mut().take()).unwrap();
        assert!(dlg.library_update().is_none());
        save(&mut cx, &dlg);
        assert!(cx.status.contains("read-only"));
        assert!(!open_item(&mut cx, "nope.nothing"));
        assert!(cx.status.contains("Unknown library item"));
        close();
    }
}
