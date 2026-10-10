//! The Library Browser dock body (CB-53 to CB-61): what the Library dock
//! shows below its tab strip.
//!
//! The panel is [`show`]: the Objects / Materials switch, a header with the
//! Library menu (New Folder, Add Selection, Convert to Symbol, Replace From
//! Library, Import and Export Library, thumbnails, Trash) and the Library
//! Preferences link, then the browser itself ([`super::library_browser`]: the
//! search field with the type filter, the tree of catalogs and the Trash, the
//! results and the preview pane). What a click asked of the editor (Open
//! Object, Replace Selected, the file pickers) is carried out after the browser
//! drew ([`handle_requests`]).
//!
//! This module also owns the small pieces the browser shares: the folder and
//! file icons, the type filter, the Trash node and list, and the path-traced
//! thumbnails ([`ThumbService`]).

mod thumbs;

pub use thumbs::{Thumb, ThumbService};

use super::docks::DockRequest;
use super::library_browser::{self, LibraryBrowserState, PanelRequest, UserAction};
use crate::editor::EditorContext;
use crate::toolbar::Action;
use crate::tools::library::user as store;
use crate::tools::library::{convert, set_active_item};
use eframe::egui::{self, Color32, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use plan_library::manage::USER_ROOT;
use plan_library::LibType;

/// Draws the Library dock body.
pub fn show(
    ui: &mut Ui,
    st: &mut LibraryBrowserState,
    cx: &mut EditorContext,
    requests: &mut Vec<DockRequest>,
) {
    // The dock lists library objects, or (filtered to Materials) the material
    // library.
    if crate::tools::materials::browser::filter_switch(ui) {
        crate::tools::materials::browser::panel(ui, cx);
        return;
    }
    header(ui, st, cx, requests);
    ui.add_space(2.0);
    if let Some(ev) = library_browser::show(ui, st) {
        if let Some(tool) = library_browser::apply_event(ev, st, cx) {
            requests.push(DockRequest::SetTool(tool));
        }
    }
    handle_requests(st, cx);
}

/// The Library menu button and the Preferences link.
fn header(
    ui: &mut Ui,
    st: &mut LibraryBrowserState,
    cx: &mut EditorContext,
    requests: &mut Vec<DockRequest>,
) {
    ui.horizontal(|ui| {
        ui.menu_button("Library", |ui| {
            if ui.button("New Folder\u{2026}").clicked() {
                st.user
                    .perform(UserAction::NewFolder(vec![USER_ROOT.to_string()]));
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Add Selection to Library").clicked() {
                store::run_command(cx, store::ADD_SELECTION);
                ui.close_menu();
            }
            let convertible = convert::can_convert(cx);
            if ui
                .add_enabled(convertible, egui::Button::new("Convert to Symbol"))
                .on_hover_text("Turns the selected 3D solids into a User Catalog symbol")
                .clicked()
            {
                convert::run_command(cx, convert::CONVERT_TO_SYMBOL);
                ui.close_menu();
            }
            if ui
                .button("Replace From Library")
                .on_hover_text("The selected objects take the active library item")
                .clicked()
            {
                convert::replace_selected(cx);
                ui.close_menu();
            }
            let mut keep = convert::keep_size();
            if ui.checkbox(&mut keep, "Replace keeps size").changed() {
                convert::set_keep_size(keep);
            }
            ui.separator();
            if ui.button("Import Library\u{2026}").clicked() {
                store::run_command(cx, store::IMPORT_LIBRARY);
                ui.close_menu();
            }
            if ui.button("Export Library\u{2026}").clicked() {
                store::run_command(cx, store::EXPORT_LIBRARY);
                ui.close_menu();
            }
            if ui.button("Import 3D Model\u{2026}").clicked() {
                store::run_command(cx, store::IMPORT_MODEL);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Rebuild Thumbnails").clicked() {
                let n = st.user.thumbs.rebuild();
                cx.status = format!("Deleted {n} cached thumbnail(s); they render again as needed");
                ui.close_menu();
            }
            if ui.button("Empty Trash\u{2026}").clicked() {
                if let Some(m) = st.user.perform(UserAction::EmptyTrash) {
                    cx.status = m;
                }
                ui.close_menu();
            }
        });
        if ui
            .small_button("Preferences\u{2026}")
            .on_hover_text("Library Browser preferences and the Chief catalog folders")
            .clicked()
        {
            requests.push(DockRequest::Run(Action::Custom(
                crate::dialogs::app_info::PREFS_LIBRARY,
            )));
        }
    });
}

/// Carries out what the browser asked of the editor this frame.
pub fn handle_requests(st: &mut LibraryBrowserState, cx: &mut EditorContext) {
    for req in std::mem::take(&mut st.user.requests) {
        match req {
            PanelRequest::Run(id) => {
                store::run_command(cx, id);
            }
            PanelRequest::OpenObject(id) => {
                crate::dialogs::library_object::open_item(cx, &id);
            }
            PanelRequest::ReplaceSelected(id) => {
                if set_active_item(cx, &id) {
                    st.active_item = Some(id);
                    convert::replace_selected(cx);
                }
            }
            PanelRequest::RescanChief => st.chief.rescan(),
        }
    }
}

// ----- the type filter -----

/// The Type filter row: a drop-down of the twelve browser types (cabinets,
/// doors, windows, fixtures, furniture, plants, materials, backdrops,
/// moldings, images, electrical, hardware); none ticked keeps everything.
pub fn type_filter(ui: &mut Ui, types: &mut Vec<LibType>) {
    ui.horizontal(|ui| {
        ui.label("Type");
        let text = match types.as_slice() {
            [] => "All".to_string(),
            [t] => t.label().to_string(),
            ts => format!("{} types", ts.len()),
        };
        egui::ComboBox::from_id_salt("library_browser_types")
            .selected_text(text)
            .show_ui(ui, |ui| {
                if ui.selectable_label(types.is_empty(), "All").clicked() {
                    types.clear();
                }
                for t in LibType::ALL {
                    let mut on = types.contains(&t);
                    if ui.checkbox(&mut on, t.label()).changed() {
                        if on {
                            types.push(t);
                        } else {
                            types.retain(|x| *x != t);
                        }
                    }
                }
            });
        if !types.is_empty() && ui.small_button("Clear").clicked() {
            types.clear();
        }
    });
}

// ----- the Trash -----

/// The Trash node at the bottom of the tree; true when it was clicked.
pub fn trash_node(ui: &mut Ui, selected: bool) -> bool {
    let n = store::trash_len();
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(15.0, 14.0), Sense::hover());
        paint_trash_icon(ui.painter(), rect, n > 0);
        ui.selectable_label(selected, format!("Trash ({n})"))
            .on_hover_text("Deleted User Catalog items wait here until the Trash is emptied")
            .clicked()
    })
    .inner
}

/// The Trash view: the deleted items with Restore and Delete Permanently, and
/// Empty Trash. Returns the actions to carry out.
pub fn trash_list(ui: &mut Ui) -> Vec<UserAction> {
    let mut actions = Vec::new();
    let trash = store::trash();
    ui.horizontal(|ui| {
        ui.strong("Trash");
        if ui
            .add_enabled(
                !trash.is_empty(),
                egui::Button::new("Empty Trash\u{2026}").small(),
            )
            .clicked()
        {
            actions.push(UserAction::EmptyTrash);
        }
    });
    if trash.is_empty() {
        ui.weak("The Trash is empty. Deleted User Catalog items wait here until you empty it.");
        return actions;
    }
    ui.weak(format!(
        "{} item{}",
        trash.len(),
        if trash.len() == 1 { "" } else { "s" }
    ));
    egui::ScrollArea::vertical()
        .id_salt("library_trash")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for e in trash.entries.iter().rev() {
                let item = &e.item;
                let resp = ui
                    .horizontal(|ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::new(11.0, 14.0), Sense::hover());
                        paint_file_icon(ui.painter(), rect);
                        let r = ui.label(&item.name);
                        if ui.small_button("Restore").clicked() {
                            actions.push(UserAction::Restore(item.id.clone()));
                        }
                        r
                    })
                    .inner;
                resp.on_hover_text(format!("From {}", item.category.join(" \u{25B8} ")))
                    .context_menu(|ui| {
                        if ui.button("Restore").clicked() {
                            actions.push(UserAction::Restore(item.id.clone()));
                            ui.close_menu();
                        }
                        if ui.button("Delete Permanently\u{2026}").clicked() {
                            actions.push(UserAction::Purge(item.id.clone()));
                            ui.close_menu();
                        }
                    });
            }
        });
    actions
}

// ----- icons -----

const FOLDER_FILL: Color32 = Color32::from_rgb(0xE6, 0xB9, 0x50);
const FOLDER_BACK: Color32 = Color32::from_rgb(0xC8, 0x95, 0x2E);
const FOLDER_LINE: Color32 = Color32::from_rgb(0x8F, 0x68, 0x1C);
const FILE_FILL: Color32 = Color32::from_rgb(0xF4, 0xF4, 0xF2);
const FILE_LINE: Color32 = Color32::from_rgb(0x7A, 0x7A, 0x78);

/// A folder icon (open or closed) in the line, before a tree label.
pub fn folder_icon(ui: &mut Ui, open: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(15.0, 12.0), Sense::hover());
    paint_folder_icon(ui.painter(), rect, open);
}

/// Paints a folder in `rect`.
pub fn paint_folder_icon(p: &Painter, rect: Rect, open: bool) {
    let (x0, y0, x1, y1) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    let stroke = Stroke::new(0.8_f32, FOLDER_LINE);
    // The back panel with its tab.
    let tab_w = (x1 - x0) * 0.42;
    p.add(Shape::convex_polygon(
        vec![
            Pos2::new(x0, y0 + 1.0),
            Pos2::new(x0 + tab_w, y0 + 1.0),
            Pos2::new(x0 + tab_w + 1.5, y0 + 3.0),
            Pos2::new(x1, y0 + 3.0),
            Pos2::new(x1, y1),
            Pos2::new(x0, y1),
        ],
        FOLDER_BACK,
        stroke,
    ));
    // The front panel: square when closed, slanted when open.
    let front = if open {
        vec![
            Pos2::new(x0 + 2.5, y0 + 5.0),
            Pos2::new(x1 + 0.5, y0 + 5.0),
            Pos2::new(x1 - 2.0, y1),
            Pos2::new(x0, y1),
        ]
    } else {
        vec![
            Pos2::new(x0, y0 + 4.0),
            Pos2::new(x1, y0 + 4.0),
            Pos2::new(x1, y1),
            Pos2::new(x0, y1),
        ]
    };
    p.add(Shape::convex_polygon(front, FOLDER_FILL, stroke));
}

/// Paints a document with a folded corner in `rect`.
pub fn paint_file_icon(p: &Painter, rect: Rect) {
    let (x0, y0, x1, y1) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    let fold = ((x1 - x0) * 0.35).max(2.0);
    let stroke = Stroke::new(0.8_f32, FILE_LINE);
    p.add(Shape::convex_polygon(
        vec![
            Pos2::new(x0, y0),
            Pos2::new(x1 - fold, y0),
            Pos2::new(x1, y0 + fold),
            Pos2::new(x1, y1),
            Pos2::new(x0, y1),
        ],
        FILE_FILL,
        stroke,
    ));
    p.add(Shape::line(
        vec![
            Pos2::new(x1 - fold, y0),
            Pos2::new(x1 - fold, y0 + fold),
            Pos2::new(x1, y0 + fold),
        ],
        stroke,
    ));
}

/// Paints a small bin (full or empty) in `rect`.
fn paint_trash_icon(p: &Painter, rect: Rect, full: bool) {
    let (x0, y0, x1, y1) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    let stroke = Stroke::new(0.9_f32, FILE_LINE);
    let fill = if full {
        Color32::from_rgb(0xC9, 0xD2, 0xDA)
    } else {
        FILE_FILL
    };
    p.add(Shape::convex_polygon(
        vec![
            Pos2::new(x0 + 2.0, y0 + 4.0),
            Pos2::new(x1 - 2.0, y0 + 4.0),
            Pos2::new(x1 - 3.0, y1),
            Pos2::new(x0 + 3.0, y1),
        ],
        fill,
        stroke,
    ));
    p.add(Shape::line_segment(
        [Pos2::new(x0 + 1.0, y0 + 3.0), Pos2::new(x1 - 1.0, y0 + 3.0)],
        stroke,
    ));
    p.add(Shape::line_segment(
        [
            Pos2::new(x0 + (x1 - x0) * 0.4, y0 + 1.5),
            Pos2::new(x0 + (x1 - x0) * 0.6, y0 + 1.5),
        ],
        stroke,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use crate::tools::library::user::tests_support::fresh;

    fn frame(
        ctx: &egui::Context,
        st: &mut LibraryBrowserState,
        cx: &mut EditorContext,
    ) -> Vec<DockRequest> {
        let mut requests = Vec::new();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(420.0, 900.0))),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| show(ui, st, cx, &mut requests));
        });
        requests
    }

    #[test]
    fn the_dock_body_draws_with_the_header_filter_and_trash_node() {
        fresh(false);
        let ctx = egui::Context::default();
        let mut st = LibraryBrowserState::new(plan_library::Library::with_all_core());
        let mut cx = EditorContext::new(plan_defaults::embedded());
        for _ in 0..3 {
            let r = frame(&ctx, &mut st, &mut cx);
            assert!(r.is_empty(), "no request without a click");
        }
        // The type filter narrows the list even without a query.
        st.user.filter.types = vec![LibType::Cabinets];
        assert!(st.user.narrows());
        frame(&ctx, &mut st, &mut cx);
        assert_eq!(st.chief.types, vec![LibType::Cabinets]);
        let shown = st.results();
        assert!(!shown.is_empty());
        assert!(shown
            .iter()
            .all(|i| plan_library::types::classify(i) == Some(LibType::Cabinets)));
    }

    #[test]
    fn icons_paint_shapes_in_every_state() {
        let ctx = egui::Context::default();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                folder_icon(ui, false);
                folder_icon(ui, true);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(11.0, 14.0), Sense::hover());
                paint_file_icon(ui.painter(), rect);
                trash_node(ui, false);
            });
        });
        assert!(out.shapes.len() >= 4);
    }

    #[test]
    fn the_trash_view_restores_and_the_panel_carries_out_requests() {
        fresh(false);
        let ctx = egui::Context::default();
        let mut st = LibraryBrowserState::new(plan_library::Library::with_all_core());
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let (item, model) = crate::tools::library::make::item_from_material(
            "Slate",
            [10, 20, 30],
            &store::new_id(plan_library::ItemKind::Material),
            &["User".to_string(), "Materials".to_string()],
        );
        let id = item.id.clone();
        store::add(item, Some(&model)).unwrap();
        assert!(store::delete(&id).unwrap());
        // Open Object on a built-in item queues the request; handling it opens
        // the dialog.
        let core = crate::tools::library::library_catalog()
            .all_items()
            .next()
            .unwrap()
            .id
            .clone();
        st.user.perform(UserAction::OpenObject(core.clone()));
        handle_requests(&mut st, &mut cx);
        assert!(crate::dialogs::library_object::is_open());
        crate::dialogs::library_object::close();
        // The Trash list offers Restore for the deleted item.
        let mut acts = Vec::new();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| acts = trash_list(ui));
        });
        assert!(acts.is_empty(), "nothing clicked yet");
        assert!(st
            .user
            .perform(UserAction::Restore(id.clone()))
            .unwrap()
            .contains("Restored"));
        assert!(store::item(&id).is_some());
        assert!(store::trash().is_empty());
    }
}
