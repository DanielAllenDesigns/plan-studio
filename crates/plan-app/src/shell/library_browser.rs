//! Chief's Library Browser (docs/parity CB-53..CB-61): a live search field, the
//! category tree from [`Library::tree`] with item counts, and a results list
//! whose rows paint a small preview of the item's [`Symbol2d`].
//!
//! The panel never touches the plan. Clicking a result returns
//! [`LibraryEvent::Activate`]; [`apply_event`] turns that into "make it the
//! active library item and switch to the Library tool".
//!
//! Below the built-in tree sit the user's Chief Architect catalogs
//! ([`chief_ui`]): Core, Bonus, Manufacturer and User nodes that scan and load
//! on background threads, thumbnails ([`png`]), a cross-catalog search and an
//! Open Object window. A clicked Chief object is bridged into the transient
//! catalog of `tools::library::chief` and activated like a built-in item.

mod chief_ui;
pub mod png;

use crate::editor::EditorContext;
use crate::tools::library::chief::{self, ChiefSettings};
use crate::tools::ToolId;
use chief_ui::{ChiefAction, ChiefBrowser};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Shape, Stroke, Vec2};
use plan_library::{CatalogItem, CategoryNode, Library, Stroke as SymStroke, Symbol2d};
use std::sync::Arc;

/// Edge of the square each result's preview is drawn in.
pub const PREVIEW_PX: f32 = 48.0;
/// Space kept around the drawing inside the preview square.
const PREVIEW_MARGIN: f32 = 2.0;
/// Most rows listed at once; the rest is summarized.
pub const RESULT_CAP: usize = 200;

/// What the user did in the panel this frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LibraryEvent {
    /// A result was clicked: make this id the active library item.
    Activate(String),
    /// Something for the status bar (placeholder menu entries).
    Message(String),
}

pub struct LibraryBrowserState {
    /// The built-in library, without the user's items.
    base: Library,
    /// `base` plus the user library (User > Images).
    library: Library,
    /// The user items `library` was built with.
    user_items: Vec<Arc<CatalogItem>>,
    tree: CategoryNode,
    /// The search field.
    pub query: String,
    /// The selected category path; empty means the whole library.
    pub category: Vec<String>,
    /// The item the Library tool places.
    pub active_item: Option<String>,
    /// The Chief Architect catalogs section.
    pub chief: ChiefBrowser,
}

impl Default for LibraryBrowserState {
    /// The built-in library plus the saved Chief catalog preference.
    fn default() -> Self {
        let mut st = LibraryBrowserState::new(Library::with_all_core());
        st.chief = ChiefBrowser::new(ChiefSettings::load());
        st
    }
}

impl LibraryBrowserState {
    pub fn new(library: Library) -> Self {
        let tree = library.tree();
        let mut st = LibraryBrowserState {
            base: library.clone(),
            library,
            user_items: Vec::new(),
            tree,
            query: String::new(),
            category: Vec::new(),
            active_item: None,
            // Off until the saved preference is loaded (see `Default`).
            chief: ChiefBrowser::new(ChiefSettings::off()),
        };
        st.sync_user_items();
        st
    }

    /// Lists the user library (`tools::images::user_items`, category
    /// User > Images) next to the built-in items; rebuilt when it changes.
    pub fn sync_user_items(&mut self) {
        let items = crate::tools::images::user_items();
        let same = items.len() == self.user_items.len()
            && items
                .iter()
                .zip(&self.user_items)
                .all(|(a, b)| Arc::ptr_eq(a, b));
        if same {
            return;
        }
        let mut library = self.base.clone();
        if !items.is_empty() {
            let owned: Vec<CatalogItem> = items.iter().map(|i| (**i).clone()).collect();
            library.add(plan_library::user::user_catalog(&owned));
        }
        self.tree = library.tree();
        self.library = library;
        self.user_items = items;
    }

    /// Items matching the search field inside the selected category: ranked
    /// search hits when there is a query, else the category in name order.
    /// Empty when there is neither a query nor a category.
    #[cfg(test)]
    pub fn results(&self) -> Vec<&CatalogItem> {
        results_of(&self.library, &self.query, &self.category)
    }

    /// The display name of the active item (a built-in or a bridged Chief
    /// object).
    pub fn active_name(&self) -> Option<String> {
        let id = self.active_item.as_deref()?;
        match self.library.get(id) {
            Some(i) => Some(i.name.clone()),
            None => chief::installed(id).map(|c| c.item.name.clone()),
        }
    }

    /// Makes `id` the active item when the library (or the transient Chief
    /// catalog) has it.
    pub fn activate(&mut self, id: &str) -> bool {
        if self.library.get(id).is_some() || chief::installed(id).is_some() {
            self.active_item = Some(id.to_string());
            true
        } else {
            false
        }
    }
}

fn results_of<'a>(library: &'a Library, query: &str, category: &[String]) -> Vec<&'a CatalogItem> {
    let query = query.trim();
    if query.is_empty() && category.is_empty() {
        return Vec::new();
    }
    let mut items = library.search(query);
    items.retain(|i| i.category.starts_with(category));
    if query.is_empty() {
        items.sort_by_key(|i| (i.name.to_lowercase(), i.id.clone()));
    }
    items
}

/// Applies what the panel reported: activating an item makes it the Library
/// tool's item and asks for that tool; messages go to the status bar.
pub fn apply_event(
    ev: LibraryEvent,
    st: &mut LibraryBrowserState,
    cx: &mut EditorContext,
) -> Option<ToolId> {
    match ev {
        LibraryEvent::Activate(id) => {
            // The Library tool keeps the id it places; the browser keeps its
            // own copy to highlight the row.
            (st.activate(&id) && crate::tools::library::set_active_item(cx, &id))
                .then_some(ToolId::Library)
        }
        LibraryEvent::Message(m) => {
            cx.status = m;
            None
        }
    }
}

/// The preview shapes of `symbol` scaled to fit `rect` (Y flipped so the
/// room side points up the screen). Empty for an empty symbol.
pub fn preview_shapes(symbol: &Symbol2d, rect: Rect, stroke: Stroke) -> Vec<Shape> {
    let Some(b) = symbol.bounds() else {
        return Vec::new();
    };
    let inner = rect.shrink(PREVIEW_MARGIN);
    let extent = b.width().max(b.height()).max(1e-6) as f32;
    let scale = inner.width().min(inner.height()) / extent;
    let c = b.center();
    let map = |p: plan_core::geometry::Point| -> Pos2 {
        Pos2::new(
            rect.center().x + (p.x - c.x) as f32 * scale,
            rect.center().y - (p.y - c.y) as f32 * scale,
        )
    };
    let mut shapes = Vec::new();
    for s in &symbol.strokes {
        match s {
            SymStroke::Polyline { points, closed } => {
                let pts: Vec<Pos2> = points.iter().map(|p| map(*p)).collect();
                if pts.len() >= 2 {
                    shapes.push(if *closed {
                        Shape::closed_line(pts, stroke)
                    } else {
                        Shape::line(pts, stroke)
                    });
                }
            }
            SymStroke::Circle { center, radius } => {
                shapes.push(Shape::circle_stroke(
                    map(*center),
                    *radius as f32 * scale,
                    stroke,
                ));
            }
            SymStroke::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => {
                let sweep = if end_deg - start_deg >= 360.0 {
                    360.0
                } else {
                    (end_deg - start_deg).rem_euclid(360.0)
                };
                let steps = ((sweep / 10.0).ceil() as usize).max(4);
                let pts: Vec<Pos2> = (0..=steps)
                    .map(|i| {
                        let a = (start_deg + sweep * i as f64 / steps as f64).to_radians();
                        map(plan_core::geometry::Point::new(
                            center.x + radius * a.cos(),
                            center.y + radius * a.sin(),
                        ))
                    })
                    .collect();
                shapes.push(Shape::line(pts, stroke));
            }
        }
    }
    shapes
}

/// Draws the whole panel body (below the dock heading).
pub fn show(ui: &mut egui::Ui, st: &mut LibraryBrowserState) -> Option<LibraryEvent> {
    st.sync_user_items();
    let mut event = None;
    let chief_action = |a: ChiefAction, event: &mut Option<LibraryEvent>| {
        *event = Some(match a {
            ChiefAction::Activate(id) => LibraryEvent::Activate(id),
            ChiefAction::Message(m) => LibraryEvent::Message(m),
        });
    };

    ui.horizontal(|ui| {
        let edit = egui::TextEdit::singleline(&mut st.query)
            .hint_text("Search the library")
            .desired_width(ui.available_width() - 28.0);
        ui.add(edit);
        if ui
            .add_enabled(!st.query.is_empty(), egui::Button::new("\u{2715}").small())
            .on_hover_text("Clear the search")
            .clicked()
        {
            st.query.clear();
        }
    });
    if st.chief.enabled() {
        ui.checkbox(&mut st.chief.search_chief, "Search Chief catalogs");
    }
    match st.active_name() {
        Some(n) => ui.label(format!("Active item: {n}")),
        None => ui.weak("No active item"),
    };
    if let Some(a) = chief_ui::windows(ui.ctx(), &mut st.chief) {
        chief_action(a, &mut event);
    }
    ui.separator();

    let tree_height = (ui.available_height() * 0.4).max(80.0);
    egui::ScrollArea::vertical()
        .id_salt("library_tree")
        .max_height(tree_height)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let mut picked = None;
            let root = st.tree.clone();
            category_node(ui, &root, &mut Vec::new(), &st.category, &mut picked, true);
            if let Some(path) = picked {
                st.category = path;
                st.chief.selected = None;
            }
            let before = st.chief.selected.clone();
            if let Some(a) = chief_ui::tree(ui, &mut st.chief) {
                chief_action(a, &mut event);
            }
            if st.chief.selected != before && st.chief.selected.is_some() {
                st.category.clear();
            }
        });
    ui.separator();

    let active = st.active_item.clone();

    // A Chief category is selected: list its objects.
    if st.chief.selected.is_some() {
        let query = st.query.clone();
        let (_, a) = chief_ui::category_list(ui, &mut st.chief, &query, active.as_deref());
        if let Some(a) = a {
            chief_action(a, &mut event);
        }
        return event;
    }

    let path = if st.category.is_empty() {
        "All categories".to_string()
    } else {
        st.category.join(" \u{25B8} ")
    };
    ui.horizontal(|ui| {
        ui.strong(path);
        if !st.category.is_empty() && ui.small_button("Show all").clicked() {
            st.category.clear();
        }
    });

    let results = results_of(&st.library, &st.query, &st.category);
    let chief_search = st.chief.enabled() && st.chief.search_chief && !st.query.trim().is_empty();
    let total = results.len();
    if total == 0 && !chief_search {
        if st.query.trim().is_empty() && st.category.is_empty() {
            ui.weak("Search above or pick a category.");
        } else {
            ui.weak("No library items match.");
        }
        return event;
    }
    if total > 0 {
        ui.weak(format!("{total} item{}", if total == 1 { "" } else { "s" }));
    }
    let query = st.query.clone();
    egui::ScrollArea::vertical()
        .id_salt("library_results")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for item in results.iter().take(RESULT_CAP) {
                let is_active = active.as_deref() == Some(item.id.as_str());
                if let Some(e) = result_row(ui, item, is_active) {
                    event = Some(e);
                }
            }
            if total > RESULT_CAP {
                ui.weak(format!(
                    "{} more; narrow the search to see them.",
                    total - RESULT_CAP
                ));
            }
            if let Some(a) = chief_ui::search_results(ui, &mut st.chief, &query, active.as_deref())
            {
                chief_action(a, &mut event);
            }
        });
    event
}

/// One tree node: a header (or a plain label for a leaf) that selects the
/// category when its label is clicked.
fn category_node(
    ui: &mut egui::Ui,
    node: &CategoryNode,
    path: &mut Vec<String>,
    selected: &[String],
    picked: &mut Option<Vec<String>>,
    is_root: bool,
) {
    let here_selected = selected == path.as_slice();
    let label = format!("{} ({})", node.name, node.count);
    if node.children.is_empty() {
        if ui.selectable_label(here_selected, label).clicked() {
            *picked = Some(path.clone());
        }
        return;
    }
    let id = ui.make_persistent_id(("library_category", path.clone()));
    let state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, is_root);
    state
        .show_header(ui, |ui| {
            if ui.selectable_label(here_selected, label).clicked() {
                *picked = Some(path.clone());
            }
        })
        .body(|ui| {
            for child in &node.children {
                path.push(child.name.clone());
                category_node(ui, child, path, selected, picked, false);
                path.pop();
            }
        });
}

/// One result: preview, name and size. A click activates it; the context menu
/// holds Chief's Open Object and Add to User Library (placeholders).
fn result_row(ui: &mut egui::Ui, item: &CatalogItem, is_active: bool) -> Option<LibraryEvent> {
    let size = Vec2::new(ui.available_width(), PREVIEW_PX + 6.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let visuals = ui.visuals();
    if is_active {
        ui.painter()
            .rect_filled(rect, 3.0, visuals.selection.bg_fill);
    } else if resp.hovered() {
        ui.painter()
            .rect_filled(rect, 3.0, visuals.widgets.hovered.weak_bg_fill);
    }
    let preview = Rect::from_min_size(rect.min + Vec2::splat(3.0), Vec2::splat(PREVIEW_PX));
    ui.painter()
        .rect_filled(preview, 2.0, Color32::from_gray(0xEC));
    let ink = Stroke::new(1.0_f32, Color32::from_gray(0x2B));
    for shape in preview_shapes(&item.symbol, preview, ink) {
        ui.painter().add(shape);
    }
    let text_x = preview.right() + 8.0;
    let name_color = visuals.strong_text_color();
    let weak = visuals.weak_text_color();
    ui.painter().text(
        Pos2::new(text_x, rect.top() + 10.0),
        egui::Align2::LEFT_CENTER,
        &item.name,
        egui::FontId::proportional(13.0),
        name_color,
    );
    ui.painter().text(
        Pos2::new(text_x, rect.top() + 28.0),
        egui::Align2::LEFT_CENTER,
        format!(
            "{} \u{00D7} {} in",
            trim_num(item.width),
            trim_num(item.depth)
        ),
        egui::FontId::proportional(11.0),
        weak,
    );
    let mut event = None;
    if resp.clicked() {
        event = Some(LibraryEvent::Activate(item.id.clone()));
    }
    resp.context_menu(|ui| {
        if ui.button("Open Object").clicked() {
            event = Some(LibraryEvent::Message(format!(
                "Open Object: {} (coming)",
                item.name
            )));
            ui.close_menu();
        }
        if ui.button("Add to User Library").clicked() {
            event = Some(LibraryEvent::Message(format!(
                "Add to User Library: {} (coming)",
                item.name
            )));
            ui.close_menu();
        }
    });
    event
}

fn trim_num(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> LibraryBrowserState {
        LibraryBrowserState::default()
    }

    #[test]
    fn empty_state_lists_nothing_until_a_query_or_category() {
        let mut st = state();
        assert!(st.results().is_empty());
        st.query = "toilet".into();
        let hits = st.results();
        assert!(!hits.is_empty());
        assert!(hits[0].name.to_lowercase().contains("toilet"));
    }

    #[test]
    fn search_filters_live_and_category_narrows() {
        let mut st = state();
        st.query = "toilet".into();
        let all = st.results().len();
        st.query = "toilet elongated".into();
        let fewer = st.results().len();
        assert!(fewer >= 1 && fewer <= all);

        // Picking a category lists exactly its subtree.
        st.query.clear();
        let root = st.tree.clone();
        let top = root.children.first().expect("a top-level category");
        st.category = vec![top.name.clone()];
        assert_eq!(st.results().len(), top.count);

        // A query inside a category only returns that category's items.
        st.query = "a".into();
        assert!(st
            .results()
            .iter()
            .all(|i| i.category.first() == Some(&top.name)));
    }

    #[test]
    fn saved_pictures_are_listed_under_user_images() {
        use crate::tools::images;
        images::set_user_library_path(Some(None));
        let mut st = LibraryBrowserState::new(Library::with_core());
        st.category = vec!["User".into(), "Images".into()];
        assert!(st.results().is_empty());
        images::register_user_item(plan_library::user::image_item(
            "user.image.t1",
            "Front Rug",
            "/pics/rug.png",
            96.0,
            60.0,
        ))
        .unwrap();
        st.sync_user_items();
        let hits = st.results();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "user.image.t1");
        assert!(st.tree.child("User").is_some());
        assert!(st.activate("user.image.t1"));
        assert_eq!(st.active_name().as_deref(), Some("Front Rug"));
        images::set_user_library_path(None);
    }

    #[test]
    fn tree_counts_add_up() {
        let st = state();
        let root = st.tree;
        assert_eq!(root.count, st.library.len());
        let kids: usize =
            root.children.iter().map(|c| c.count).sum::<usize>() + root.item_ids.len();
        assert_eq!(kids, root.count);
    }

    #[test]
    fn activating_checks_the_id() {
        let mut st = state();
        assert!(!st.activate("no.such.item"));
        assert!(st.active_item.is_none());
        let id = st.library.all_items().next().unwrap().id.clone();
        assert!(st.activate(&id));
        assert_eq!(st.active_item.as_deref(), Some(id.as_str()));
        assert!(st.active_name().is_some());
    }

    #[test]
    fn previews_fit_the_48px_box() {
        let st = state();
        let rect = Rect::from_min_size(Pos2::new(100.0, 200.0), Vec2::splat(PREVIEW_PX));
        let mut drawn = 0;
        for item in st.library.all_items().take(300) {
            let shapes = preview_shapes(&item.symbol, rect, Stroke::new(1.0_f32, Color32::BLACK));
            assert_eq!(shapes.is_empty(), item.symbol.is_empty(), "{}", item.id);
            drawn += shapes.len();
            for s in shapes {
                let r = s.visual_bounding_rect();
                // Allow the stroke width on each side.
                assert!(
                    rect.expand(1.0).contains_rect(r),
                    "{} preview {r:?} leaves {rect:?}",
                    item.id
                );
            }
        }
        assert!(drawn > 0);
    }

    #[test]
    fn activating_a_chief_object_selects_it_and_asks_for_the_library_tool() {
        use crate::plan_defaults;
        use crate::tools::library::{active_item, clear_active_item};
        use plan_library::{CatalogItem, Placement};

        clear_active_item();
        let id = "chief.cafe-0002.77";
        chief::install_item(
            CatalogItem::new(
                id,
                "Round Tank Toilet",
                Placement::FreeStanding,
                Symbol2d::default(),
            )
            .with_size(15.5, 28.0, 30.0),
            "Core Interiors",
        );
        let mut st = state();
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let tool = apply_event(LibraryEvent::Activate(id.into()), &mut st, &mut cx);
        assert_eq!(tool, Some(ToolId::Library));
        assert_eq!(active_item().as_deref(), Some(id));
        assert_eq!(st.active_item.as_deref(), Some(id));
        assert_eq!(st.active_name().as_deref(), Some("Round Tank Toilet"));
        assert!(cx.status.contains("Round Tank Toilet"), "{}", cx.status);

        // An id nobody bridged does nothing.
        assert_eq!(
            apply_event(
                LibraryEvent::Activate("chief.nope.1".into()),
                &mut st,
                &mut cx
            ),
            None
        );
        assert_eq!(st.active_item.as_deref(), Some(id));
        apply_event(LibraryEvent::Message("hello".into()), &mut st, &mut cx);
        assert_eq!(cx.status, "hello");
    }

    #[test]
    fn the_panel_draws_with_chief_catalogs_on_and_off() {
        let ctx = egui::Context::default();
        let mut st = state();
        st.chief = ChiefBrowser::new(ChiefSettings::off());
        let run = |st: &mut LibraryBrowserState| {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    assert!(show(ui, st).is_none());
                });
            });
        };
        run(&mut st);
        st.chief.settings.enabled = true;
        st.chief.search_chief = true;
        st.query = "toilet".into();
        st.chief
            .set_library(plan_calib::ChiefLibrary::from_entries(Vec::new()));
        run(&mut st);
    }
}
