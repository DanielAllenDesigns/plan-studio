//! The pages of Edit > Default Settings that `defaults.rs` does not open a
//! specification dialog for: a registry of page specs ([`spec`]), the one
//! open page and the Electrical page.
//!
//! Every page has an id (`cad.arcs`, `stairs`, ...). [`request_open`] asks for
//! a page, [`show`] draws it each frame and applies OK to the defaults. A
//! page is a [`page::PageSpec`]: sections of fields that either edit a typed
//! slot of `PlanDefaults` or are stored under `PlanDefaults::pages`.

mod architectural;
mod cad;
mod camera;
mod dimension;
pub mod electrical;
pub mod page;
mod plan;

use crate::editor::EditorContext;
use eframe::egui;
pub use page::{GenericPage, PageSpec};
use std::cell::RefCell;

/// The page with id `id`, if there is one.
pub fn spec(id: &str) -> Option<PageSpec> {
    if let Some(slug) = id.strip_prefix("cad.") {
        return cad::page(slug);
    }
    if let Some(slug) = id.strip_prefix("camera.") {
        return camera::page(slug);
    }
    if let Some(slug) = id.strip_prefix("dimension.") {
        return dimension::page(slug);
    }
    if let Some(slug) = id.strip_prefix("schedules.") {
        return plan::schedule(slug);
    }
    if let Some(slug) = id.strip_prefix("text.") {
        return plan::text_page(slug);
    }
    if let Some(slug) = id.strip_prefix("walls.") {
        return architectural::special_wall(slug);
    }
    Some(match id {
        "solid3d" => architectural::solid_3d(),
        "corner_trim" => architectural::corner_trim(),
        "distributed" => architectural::distributed(),
        "image" => architectural::image(),
        "dormer" => architectural::dormer(),
        "foundation" => architectural::foundation(),
        "slab" => architectural::slab(),
        "stairs" => architectural::stairs(),
        "railing_deck" => architectural::railing_deck(),
        "garage_door" => architectural::garage_door(),
        "materials_list" => architectural::materials_list(),
        "molding_polylines" => architectural::molding_polylines(),
        "general" => plan::general(),
        "plan" => plan::plan_defaults(),
        "layout" => plan::layout(),
        "default_sets" => plan::default_sets(),
        "floor_levels" => plan::floor_levels(),
        "platforms" => plan::platforms(),
        "rooms" => plan::rooms(),
        "room_label" => plan::room_label(),
        _ => return None,
    })
}

/// The ids of every generic page, in tree order.
pub fn all_ids() -> Vec<String> {
    let mut v: Vec<String> = [
        "solid3d",
        "corner_trim",
        "distributed",
        "image",
        "dormer",
        "foundation",
        "slab",
        "stairs",
        "railing_deck",
        "garage_door",
        "materials_list",
        "molding_polylines",
        "general",
        "plan",
        "layout",
        "default_sets",
        "floor_levels",
        "platforms",
        "rooms",
        "room_label",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    v.extend(cad::LEAVES.iter().map(|(s, _)| format!("cad.{s}")));
    v.extend(camera::LEAVES.iter().map(|(s, _)| format!("camera.{s}")));
    v.extend(dimension::LEAVES.iter().map(|(s, _)| format!("dimension.{s}")));
    v.extend(plan::SCHEDULES.iter().map(|(s, _)| format!("schedules.{s}")));
    v.extend(plan::TEXT_PAGES.iter().map(|(s, _)| format!("text.{s}")));
    for s in ["railing", "fence", "pony", "half", "glass", "attic"] {
        v.push(format!("walls.{s}"));
    }
    v
}

thread_local! {
    static WANTED: RefCell<Option<String>> = const { RefCell::new(None) };
    static OPEN: RefCell<Option<GenericPage>> = const { RefCell::new(None) };
}

/// Asks for page `id`; [`show`] opens it on its next frame.
pub fn request_open(id: &str) {
    WANTED.with(|w| *w.borrow_mut() = Some(id.to_string()));
}

/// The id of the page that is open or asked for.
pub fn open_id() -> Option<String> {
    WANTED
        .with(|w| w.borrow().clone())
        .or_else(|| OPEN.with(|o| o.borrow().as_ref().map(|p| p.spec().id.clone())))
}

/// Is a generic page or the Electrical page open or asked for?
#[cfg_attr(not(test), allow(dead_code))]
pub fn is_open() -> bool {
    open_id().is_some() || electrical::is_open()
}

/// Runs `f` on the open generic page (for the tests).
#[cfg(test)]
pub fn with_open<R>(f: impl FnOnce(&mut GenericPage) -> R) -> Option<R> {
    OPEN.with(|o| o.borrow_mut().as_mut().map(f))
}

/// Draws the open page and applies OK; call once a frame.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    electrical::show(ctx, cx);
    if let Some(id) = WANTED.with(|w| w.borrow_mut().take()) {
        if let Some(spec) = spec(&id) {
            OPEN.with(|o| *o.borrow_mut() = Some(GenericPage::new(spec, &cx.defaults)));
        }
    }
    let Some(mut page) = OPEN.with(|o| o.borrow_mut().take()) else {
        return;
    };
    match page.show(ctx) {
        crate::dialogs::Outcome::Open => OPEN.with(|o| *o.borrow_mut() = Some(page)),
        crate::dialogs::Outcome::Cancel => {}
        crate::dialogs::Outcome::Ok => {
            page.apply(&mut cx.defaults);
            cx.status = format!("Saved the {} defaults", page.spec().title);
        }
    }
}
