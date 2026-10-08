//! The Chief Architect catalogs inside the Library Browser.
//!
//! Four top-level nodes (Core, Bonus, Manufacturer, User) list the catalogs
//! `plan_calib::ChiefLibrary::discover()` found. The scan runs on a background
//! thread the first time a node is expanded; each catalog loads its category
//! tree and object list on another thread when it is expanded. Rows show the
//! catalog's own PNG thumbnails (decoded by [`super::png`], cached as egui
//! textures). Clicking a row bridges the object into the transient catalog
//! (`tools::library::chief`) and activates it; the context menu opens an info
//! window. Licensed content is read in place and never copied.

use super::png;
use crate::tools::library::chief::{self, ChiefSettings, LICENSE_NOTE};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, TextureHandle, Vec2};
use plan_calib::{
    CatalogKind, CategoryNode as ChiefNode, ChiefCatalog, ChiefLibrary, ObjectSummary, SearchHit,
};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

/// Thumbnail edge kept in memory (the rows draw 48 px, 2x for sharp screens).
const THUMB_PX: usize = 96;
/// Thumbnails decoded per frame.
const THUMBS_PER_FRAME: usize = 4;
/// Object sizes decoded per frame.
const SIZES_PER_FRAME: usize = 2;
/// Most hits a cross-catalog search returns.
pub const SEARCH_MAX: usize = 200;
/// The search starts this long after the last keystroke.
const SEARCH_DEBOUNCE: f64 = 0.35;
const ROW_H: f32 = 54.0;

/// One top-level node of the Chief section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub title: &'static str,
    pub kind: CatalogKind,
    /// Indices into `ChiefLibrary::catalogs()` (installed catalogs only).
    pub catalogs: Vec<usize>,
}

/// The four top-level nodes for `lib`, in Chief's order. Uninstalled and
/// deleted registry entries are left out.
pub fn groups(lib: &ChiefLibrary) -> Vec<Group> {
    let spec = [
        ("Chief Architect Core Catalogs", CatalogKind::Core),
        ("Bonus Catalogs", CatalogKind::Bonus),
        ("Manufacturer Catalogs", CatalogKind::Manufacturer),
        ("User Catalog", CatalogKind::User),
    ];
    spec.iter()
        .map(|&(title, kind)| Group {
            title,
            kind,
            catalogs: lib
                .catalogs()
                .iter()
                .enumerate()
                .filter(|(_, e)| e.kind == kind && e.path.is_some())
                .map(|(i, _)| i)
                .collect(),
        })
        .collect()
}

/// Titles for the placeholder shown before the scan has finished.
pub fn group_titles() -> [&'static str; 4] {
    [
        "Chief Architect Core Catalogs",
        "Bonus Catalogs",
        "Manufacturer Catalogs",
        "User Catalog",
    ]
}

/// A catalog's category tree and object list.
pub struct CatalogView {
    pub tree: ChiefNode,
    pub objects: Vec<ObjectSummary>,
}

impl CatalogView {
    /// Reads both from an open catalog.
    pub fn load(cat: &ChiefCatalog) -> Result<CatalogView, String> {
        let tree = cat.category_tree().map_err(|e| e.to_string())?;
        let mut it = cat.objects().map_err(|e| e.to_string())?;
        let objects: Vec<ObjectSummary> = it.by_ref().collect();
        if let Some(e) = it.take_error() {
            return Err(e.to_string());
        }
        Ok(CatalogView { tree, objects })
    }

    /// Objects under `path` (names from the catalog root), by name. A query
    /// further narrows by name or keyword.
    pub fn objects_in(&self, path: &[String], query: &str) -> Vec<&ObjectSummary> {
        let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        let mut v: Vec<&ObjectSummary> = self
            .objects
            .iter()
            .filter(|o| o.category_path.starts_with(path))
            .filter(|o| {
                let name = o.name.to_lowercase();
                terms.iter().all(|t| {
                    name.contains(t) || o.keywords.iter().any(|k| k.to_lowercase().contains(t))
                })
            })
            .collect();
        v.sort_by_key(|o| (o.name.to_lowercase(), o.library_object_id));
        v
    }
}

/// Lines of the "Open Object" window.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectInfo {
    pub catalog_name: String,
    pub object: ObjectSummary,
    /// Width, depth, height in inches when decoded.
    pub size: Option<[f64; 3]>,
}

impl ObjectInfo {
    /// Label and value rows, ending with the licensing note.
    pub fn lines(&self) -> Vec<(&'static str, String)> {
        let o = &self.object;
        vec![
            ("Name", o.name.clone()),
            ("Category", o.category_path.join(" \u{25B8} ")),
            ("Keywords", o.keywords.join(", ")),
            (
                "Size",
                match self.size {
                    Some([w, d, h]) => format!(
                        "{} \u{00D7} {} \u{00D7} {} in (W \u{00D7} D \u{00D7} H)",
                        trim(w),
                        trim(d),
                        trim(h)
                    ),
                    None => "not decoded".into(),
                },
            ),
            ("Source catalog", self.catalog_name.clone()),
            ("Licence", LICENSE_NOTE.to_string()),
        ]
    }
}

fn trim(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

enum Scan {
    Idle,
    Running(Receiver<ChiefLibrary>),
    Done,
}

enum Slot {
    Loading(Receiver<Result<CatalogView, String>>),
    Ready(Arc<CatalogView>),
    Failed(String),
}

struct SearchJob {
    query: String,
    rx: Receiver<Vec<SearchHit>>,
}

/// Everything the Chief section of the browser keeps.
pub struct ChiefBrowser {
    pub settings: ChiefSettings,
    /// "Search Chief catalogs".
    pub search_chief: bool,
    /// The category (or whole catalog, path empty) whose objects are listed.
    pub selected: Option<(usize, Vec<String>)>,
    pub info: Option<ObjectInfo>,
    scan: Scan,
    library: Option<Arc<ChiefLibrary>>,
    catalogs: HashMap<usize, Slot>,
    opened: HashMap<usize, Arc<ChiefCatalog>>,
    thumbs: HashMap<(usize, i64), Option<TextureHandle>>,
    sizes: HashMap<(usize, i64), Option<[f64; 3]>>,
    hits: Vec<SearchHit>,
    hits_query: String,
    job: Option<SearchJob>,
    edited_at: f64,
    last_query: String,
    folder_window: Option<String>,
    configured: bool,
    notice: String,
}

impl ChiefBrowser {
    pub fn new(settings: ChiefSettings) -> Self {
        Self {
            settings,
            search_chief: false,
            selected: None,
            info: None,
            scan: Scan::Idle,
            library: None,
            catalogs: HashMap::new(),
            opened: HashMap::new(),
            thumbs: HashMap::new(),
            sizes: HashMap::new(),
            hits: Vec::new(),
            hits_query: String::new(),
            job: None,
            edited_at: 0.0,
            last_query: String::new(),
            folder_window: None,
            configured: false,
            notice: String::new(),
        }
    }

    /// Whether the user wants Chief rows (checkbox on).
    pub fn enabled(&self) -> bool {
        self.settings.enabled
    }

    /// The text shown under the nodes.
    pub fn status_line(&self) -> String {
        match (&self.scan, &self.library) {
            (Scan::Running(_), _) => "Scanning Chief catalogs\u{2026}".into(),
            (_, Some(lib)) => {
                let st = lib.stats();
                if st.missing > 0 {
                    format!(
                        "{} Chief catalogs found ({} not installed)",
                        st.installed, st.missing
                    )
                } else {
                    format!("{} Chief catalogs found", st.installed)
                }
            }
            _ => String::new(),
        }
    }

    /// Replaces the scan result (also used by tests and the background
    /// thread's hand-over).
    pub fn set_library(&mut self, lib: ChiefLibrary) {
        let lib = Arc::new(lib);
        chief::set_library(lib.clone());
        self.library = Some(lib);
        self.scan = Scan::Done;
        self.catalogs.clear();
        self.opened.clear();
        self.thumbs.clear();
        self.sizes.clear();
        self.selected = None;
        self.hits.clear();
        self.hits_query.clear();
    }

    /// The loaded library, if the scan finished.
    #[cfg(test)]
    pub fn library(&self) -> Option<&Arc<ChiefLibrary>> {
        self.library.as_ref()
    }

    /// Starts the background scan unless one ran or is running.
    fn ensure_scan(&mut self, ctx: &egui::Context) {
        if matches!(self.scan, Scan::Idle) && self.settings.enabled {
            let folder = self.settings.folder.clone();
            self.scan = Scan::Running(spawn(ctx, move || chief::discover(folder.as_deref())));
        }
    }

    fn reset_scan(&mut self) {
        self.scan = Scan::Idle;
        self.library = None;
        self.catalogs.clear();
        self.opened.clear();
        self.thumbs.clear();
        self.sizes.clear();
        self.selected = None;
        self.hits.clear();
        self.hits_query.clear();
        self.job = None;
    }

    /// Applies new settings: stores them, informs the backend, saves.
    fn apply_settings(&mut self, new: ChiefSettings) -> Option<String> {
        let folder_changed = new.folder != self.settings.folder;
        let toggled = new.enabled != self.settings.enabled;
        self.settings = new;
        chief::configure(&self.settings);
        if folder_changed || toggled {
            self.reset_scan();
        }
        self.settings
            .save()
            .err()
            .map(|e| format!("Could not save the Chief catalog setting: {e}"))
    }

    /// Collects finished background jobs.
    fn poll(&mut self) {
        if let Scan::Running(rx) = &self.scan {
            match rx.try_recv() {
                Ok(lib) => self.set_library(lib),
                Err(TryRecvError::Disconnected) => self.scan = Scan::Done,
                Err(TryRecvError::Empty) => {}
            }
        }
        for slot in self.catalogs.values_mut() {
            if let Slot::Loading(rx) = slot {
                match rx.try_recv() {
                    Ok(Ok(v)) => *slot = Slot::Ready(Arc::new(v)),
                    Ok(Err(e)) => *slot = Slot::Failed(e),
                    Err(TryRecvError::Disconnected) => *slot = Slot::Failed("load stopped".into()),
                    Err(TryRecvError::Empty) => {}
                }
            }
        }
        if let Some(job) = &self.job {
            match job.rx.try_recv() {
                Ok(hits) => {
                    self.hits = hits;
                    self.hits_query = job.query.clone();
                    self.job = None;
                }
                Err(TryRecvError::Disconnected) => self.job = None,
                Err(TryRecvError::Empty) => {}
            }
        }
    }

    fn busy(&self) -> bool {
        matches!(self.scan, Scan::Running(_))
            || self.job.is_some()
            || self
                .catalogs
                .values()
                .any(|s| matches!(s, Slot::Loading(_)))
    }

    fn catalog_name(&self, idx: usize) -> String {
        self.library
            .as_ref()
            .and_then(|l| l.catalogs().get(idx))
            .map_or_else(String::new, |e| e.name.clone())
    }

    fn open_catalog(&mut self, idx: usize) -> Option<Arc<ChiefCatalog>> {
        if let Some(c) = self.opened.get(&idx) {
            return Some(c.clone());
        }
        let c = Arc::new(self.library.as_ref()?.open(idx).ok()?);
        self.opened.insert(idx, c.clone());
        Some(c)
    }

    fn start_catalog(&mut self, ctx: &egui::Context, idx: usize) {
        if self.catalogs.contains_key(&idx) {
            return;
        }
        let Some(lib) = self.library.clone() else {
            return;
        };
        let rx = spawn(ctx, move || {
            let cat = lib.open(idx).map_err(|e| e.to_string())?;
            CatalogView::load(&cat)
        });
        self.catalogs.insert(idx, Slot::Loading(rx));
    }

    /// The decoded size of an object, a little at a time.
    fn size_of(&mut self, idx: usize, id: i64, budget: &mut usize) -> Option<[f64; 3]> {
        if let Some(s) = self.sizes.get(&(idx, id)) {
            return *s;
        }
        if *budget == 0 {
            return None;
        }
        *budget -= 1;
        let size = self.open_catalog(idx).and_then(|cat| {
            let blobs = cat.object_blobs(id).ok()?;
            plan_calib::decode::decode_object(&blobs)
                .size
                .map(|s| [s.width, s.depth, s.height])
        });
        self.sizes.insert((idx, id), size);
        size
    }

    fn thumb_of(
        &mut self,
        ctx: &egui::Context,
        idx: usize,
        obj: &ObjectSummary,
        budget: &mut usize,
    ) -> Option<TextureHandle> {
        if let Some(t) = self.thumbs.get(&(idx, obj.library_object_id)) {
            return t.clone();
        }
        if *budget == 0 || !obj.has_thumbnail {
            if !obj.has_thumbnail {
                self.thumbs.insert((idx, obj.library_object_id), None);
            }
            return None;
        }
        *budget -= 1;
        let tex = self
            .open_catalog(idx)
            .and_then(|cat| cat.thumbnail(obj.library_object_id).ok().flatten())
            .and_then(|bytes| png::decode(&bytes).ok())
            .map(|img| {
                ctx.load_texture(
                    format!("chief-thumb-{idx}-{}", obj.library_object_id),
                    img.downscaled(THUMB_PX).to_color_image(),
                    egui::TextureOptions::LINEAR,
                )
            });
        self.thumbs
            .insert((idx, obj.library_object_id), tex.clone());
        tex
    }

    /// Bridges the clicked object and returns the id to activate.
    fn activate(&mut self, idx: usize, obj: &ObjectSummary) -> Result<String, String> {
        let cat = self
            .open_catalog(idx)
            .ok_or_else(|| format!("Could not open {}", self.catalog_name(idx)))?;
        chief::install_object(&cat, obj)
            .map(|ci| ci.item.id.clone())
            .map_err(|e| format!("Could not read {}: {e}", obj.name))
    }

    /// Opens the info window for an object.
    fn open_info(&mut self, idx: usize, obj: &ObjectSummary) {
        let mut budget = 1;
        let size = self.size_of(idx, obj.library_object_id, &mut budget);
        self.info = Some(ObjectInfo {
            catalog_name: self.catalog_name(idx),
            object: obj.clone(),
            size,
        });
    }

    fn kick_search(&mut self, ctx: &egui::Context, query: &str) {
        let q = query.trim().to_owned();
        if q != self.last_query {
            self.last_query = q.clone();
            self.edited_at = ctx.input(|i| i.time);
        }
        if !self.search_chief || self.job.is_some() {
            return;
        }
        if q.is_empty() {
            self.hits.clear();
            self.hits_query.clear();
            return;
        }
        if q == self.hits_query {
            return;
        }
        let wait = SEARCH_DEBOUNCE - (ctx.input(|i| i.time) - self.edited_at);
        if wait > 0.0 {
            ctx.request_repaint_after(Duration::from_secs_f64(wait));
            return;
        }
        let Some(lib) = self.library.clone() else {
            return;
        };
        let query = q.clone();
        self.job = Some(SearchJob {
            query: q,
            rx: spawn(ctx, move || lib.search(&query, SEARCH_MAX)),
        });
    }
}

fn spawn<T: Send + 'static>(
    ctx: &egui::Context,
    f: impl FnOnce() -> T + Send + 'static,
) -> Receiver<T> {
    let (tx, rx) = channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let _ = tx.send(f());
        ctx.request_repaint();
    });
    rx
}

// ----- drawing -----

/// What the Chief section asks the browser to do.
pub enum ChiefAction {
    /// Make this bridged id the active item.
    Activate(String),
    Message(String),
}

/// The settings row and the four nodes (inside the tree scroll area).
pub fn tree(ui: &mut egui::Ui, st: &mut ChiefBrowser) -> Option<ChiefAction> {
    let mut action = None;
    if !st.configured {
        chief::configure(&st.settings);
        st.configured = true;
    }
    st.poll();
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        let mut on = st.settings.enabled;
        if ui
            .checkbox(&mut on, "Use Chief Architect catalogs")
            .on_hover_text(LICENSE_NOTE)
            .changed()
        {
            let new = ChiefSettings {
                enabled: on,
                ..st.settings.clone()
            };
            if let Some(m) = st.apply_settings(new) {
                action = Some(ChiefAction::Message(m));
            }
        }
        if ui.small_button("Catalog folders\u{2026}").clicked() {
            st.folder_window = Some(
                st.settings
                    .folder
                    .as_ref()
                    .map_or_else(String::new, |p| p.to_string_lossy().into_owned()),
            );
        }
    });
    if !st.settings.enabled {
        return action;
    }

    let library = st.library.clone();
    let node_groups = library.as_ref().map(|l| groups(l));
    for (n, title) in group_titles().iter().enumerate() {
        let group = node_groups.as_ref().map(|g| g[n].clone());
        let label = match &group {
            Some(g) => format!("{title} ({})", g.catalogs.len()),
            None => (*title).to_string(),
        };
        egui::CollapsingHeader::new(label)
            .id_salt(("library_chief_group", n))
            .default_open(false)
            .show(ui, |ui| {
                st.ensure_scan(ui.ctx());
                match &group {
                    None => {
                        ui.weak("Scanning Chief catalogs\u{2026}");
                    }
                    Some(g) if g.catalogs.is_empty() => {
                        ui.weak("No catalogs installed.");
                    }
                    Some(g) => {
                        for &idx in &g.catalogs {
                            catalog_node(ui, st, idx);
                        }
                    }
                }
            });
    }
    let status = st.status_line();
    if !status.is_empty() {
        ui.weak(status);
    }
    if !st.notice.is_empty() {
        ui.weak(st.notice.clone());
    }
    if st.busy() {
        ui.ctx().request_repaint_after(Duration::from_millis(120));
    }
    action
}

/// Path prefix of a catalog's objects: the root tag's name, unless the tree
/// has a synthetic top node.
fn root_prefix(view: &CatalogView) -> Vec<String> {
    if view.tree.uuid.is_empty() {
        Vec::new()
    } else {
        vec![view.tree.name.clone()]
    }
}

fn catalog_node(ui: &mut egui::Ui, st: &mut ChiefBrowser, idx: usize) {
    let name = st.catalog_name(idx);
    let id = ui.make_persistent_id(("library_chief_catalog", idx));
    let state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);
    let open = state.is_open();
    let ctx = ui.ctx().clone();
    let prefix = match st.catalogs.get(&idx) {
        Some(Slot::Ready(v)) => root_prefix(v),
        _ => Vec::new(),
    };
    let selected_here = st
        .selected
        .as_ref()
        .is_some_and(|(i, p)| *i == idx && *p == prefix);
    let mut pick: Option<(usize, Vec<String>)> = None;
    let header = state.show_header(ui, |ui| {
        if ui.selectable_label(selected_here, &name).clicked() {
            pick = Some((idx, prefix.clone()));
        }
    });
    if open {
        st.start_catalog(&ctx, idx);
    }
    header.body(|ui| match st.catalogs.get(&idx) {
        Some(Slot::Ready(view)) => {
            let view = view.clone();
            let mut path = root_prefix(&view);
            for child in &view.tree.children {
                path.push(child.name.clone());
                chief_category(ui, idx, child, &mut path, &st.selected, &mut pick);
                path.pop();
            }
            if view.tree.children.is_empty() {
                ui.weak(format!("{} objects", view.objects.len()));
            }
        }
        Some(Slot::Failed(e)) => {
            ui.weak(format!("Could not read this catalog: {e}"));
        }
        _ => {
            ui.weak("Loading\u{2026}");
        }
    });
    if let Some(sel) = pick {
        st.start_catalog(&ctx, sel.0);
        st.selected = Some(sel);
    }
}

/// `path` is the full category path of `node` (root name first).
fn chief_category(
    ui: &mut egui::Ui,
    idx: usize,
    node: &ChiefNode,
    path: &mut Vec<String>,
    selected: &Option<(usize, Vec<String>)>,
    pick: &mut Option<(usize, Vec<String>)>,
) {
    let here = selected
        .as_ref()
        .is_some_and(|(i, p)| *i == idx && p == path);
    let label = format!("{} ({})", node.name, node.total_count());
    if node.children.is_empty() {
        if ui.selectable_label(here, label).clicked() {
            *pick = Some((idx, path.clone()));
        }
        return;
    }
    let id = ui.make_persistent_id(("library_chief_cat", idx, path.clone()));
    let state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);
    state
        .show_header(ui, |ui| {
            if ui.selectable_label(here, label).clicked() {
                *pick = Some((idx, path.clone()));
            }
        })
        .body(|ui| {
            for child in &node.children {
                path.push(child.name.clone());
                chief_category(ui, idx, child, path, selected, pick);
                path.pop();
            }
        });
}

/// The Chief part of the results area. Returns `true` when it drew the
/// whole list (a Chief category is selected, no built-in rows to show).
pub fn category_list(
    ui: &mut egui::Ui,
    st: &mut ChiefBrowser,
    query: &str,
    active: Option<&str>,
) -> (bool, Option<ChiefAction>) {
    let Some((idx, path)) = st.selected.clone() else {
        return (false, None);
    };
    let mut action = None;
    ui.horizontal(|ui| {
        ui.strong(format!(
            "{} \u{25B8} {}",
            st.catalog_name(idx),
            path.join(" \u{25B8} ")
        ));
        if ui.small_button("Show all").clicked() {
            st.selected = None;
        }
    });
    let view = match st.catalogs.get(&idx) {
        Some(Slot::Ready(v)) => v.clone(),
        Some(Slot::Failed(e)) => {
            ui.weak(format!("Could not read this catalog: {e}"));
            return (true, None);
        }
        _ => {
            ui.weak("Loading\u{2026}");
            return (true, None);
        }
    };
    let objs = view.objects_in(&path, query);
    ui.weak(format!(
        "{} object{}",
        objs.len(),
        if objs.len() == 1 { "" } else { "s" }
    ));
    let mut thumbs = THUMBS_PER_FRAME;
    let mut sizes = SIZES_PER_FRAME;
    egui::ScrollArea::vertical()
        .id_salt("library_chief_results")
        .auto_shrink([false, false])
        .show_rows(ui, ROW_H, objs.len(), |ui, range| {
            for o in &objs[range] {
                let catalog = st.catalog_name(idx);
                if let Some(a) = object_row(
                    ui,
                    st,
                    idx,
                    o,
                    &catalog,
                    false,
                    active,
                    &mut thumbs,
                    &mut sizes,
                ) {
                    action = Some(a);
                }
            }
        });
    (true, action)
}

/// Cross-catalog search results (when "Search Chief catalogs" is on).
pub fn search_results(
    ui: &mut egui::Ui,
    st: &mut ChiefBrowser,
    query: &str,
    active: Option<&str>,
) -> Option<ChiefAction> {
    if !st.settings.enabled || !st.search_chief || query.trim().is_empty() {
        return None;
    }
    st.kick_search(ui.ctx(), query);
    let mut action = None;
    ui.separator();
    let searching = st.job.is_some() || st.hits_query != query.trim();
    if st.library.is_none() {
        ui.weak("Open a Chief node first to scan the catalogs.");
        st.ensure_scan(ui.ctx());
        return None;
    }
    if searching && st.hits.is_empty() {
        ui.weak("Searching Chief catalogs\u{2026}");
        return None;
    }
    ui.strong(format!(
        "Chief catalogs: {} hit{}{}",
        st.hits.len(),
        if st.hits.len() == 1 { "" } else { "s" },
        if st.hits.len() >= SEARCH_MAX {
            " (first 200)"
        } else {
            ""
        }
    ));
    let hits = st.hits.clone();
    let mut thumbs = THUMBS_PER_FRAME;
    let mut sizes = SIZES_PER_FRAME;
    for h in &hits {
        if let Some(a) = object_row(
            ui,
            st,
            h.catalog,
            &h.object,
            &h.catalog_name,
            true,
            active,
            &mut thumbs,
            &mut sizes,
        ) {
            action = Some(a);
        }
    }
    action
}

/// One object row: thumbnail, name, catalog (for search hits) and size.
#[allow(clippy::too_many_arguments)]
fn object_row(
    ui: &mut egui::Ui,
    st: &mut ChiefBrowser,
    idx: usize,
    obj: &ObjectSummary,
    catalog_name: &str,
    show_catalog: bool,
    active: Option<&str>,
    thumbs: &mut usize,
    sizes: &mut usize,
) -> Option<ChiefAction> {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H), Sense::click());
    let visible = ui.is_rect_visible(rect);
    let mut size = None;
    let mut tex = None;
    if visible {
        tex = st.thumb_of(ui.ctx(), idx, obj, thumbs);
        size = st.size_of(idx, obj.library_object_id, sizes);
        if (tex.is_none() && obj.has_thumbnail && *thumbs == 0) || (size.is_none() && *sizes == 0) {
            ui.ctx().request_repaint();
        }
    }
    let is_active = active.is_some_and(|a| {
        chief::parse_id(a).is_some_and(|(_, id)| id == obj.library_object_id)
            && st
                .library
                .as_ref()
                .and_then(|l| l.catalogs().get(idx))
                .is_some_and(|e| a.contains(&e.uuid))
    });
    if visible {
        let visuals = ui.visuals();
        if is_active {
            ui.painter()
                .rect_filled(rect, 3.0, visuals.selection.bg_fill);
        } else if resp.hovered() {
            ui.painter()
                .rect_filled(rect, 3.0, visuals.widgets.hovered.weak_bg_fill);
        }
        let preview =
            Rect::from_min_size(rect.min + Vec2::splat(3.0), Vec2::splat(super::PREVIEW_PX));
        ui.painter()
            .rect_filled(preview, 2.0, Color32::from_gray(0xEC));
        match &tex {
            Some(t) => {
                ui.painter().image(
                    t.id(),
                    preview,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            None => {
                let letter = obj
                    .name
                    .chars()
                    .find(|c| c.is_alphanumeric())
                    .map_or('?', |c| c.to_ascii_uppercase());
                ui.painter().text(
                    preview.center(),
                    egui::Align2::CENTER_CENTER,
                    letter,
                    egui::FontId::proportional(20.0),
                    Color32::from_gray(0x88),
                );
            }
        }
        let text_x = preview.right() + 8.0;
        let (strong, weak) = (visuals.strong_text_color(), visuals.weak_text_color());
        ui.painter().text(
            Pos2::new(text_x, rect.top() + 12.0),
            egui::Align2::LEFT_CENTER,
            &obj.name,
            egui::FontId::proportional(13.0),
            strong,
        );
        let size_text = size.map(|[w, d, _]| format!("{} \u{00D7} {} in", trim(w), trim(d)));
        let sub = match (show_catalog, size_text) {
            (true, Some(s)) => format!("{catalog_name}  \u{00B7}  {s}"),
            (true, None) => catalog_name.to_string(),
            (false, Some(s)) => s,
            (false, None) => String::new(),
        };
        ui.painter().text(
            Pos2::new(text_x, rect.top() + 31.0),
            egui::Align2::LEFT_CENTER,
            sub,
            egui::FontId::proportional(11.0),
            weak,
        );
    }
    let mut action = None;
    if resp.clicked() {
        action = Some(match st.activate(idx, obj) {
            Ok(id) => ChiefAction::Activate(id),
            Err(m) => ChiefAction::Message(m),
        });
    }
    resp.context_menu(|ui| {
        if ui.button("Open Object").clicked() {
            st.open_info(idx, obj);
            ui.close_menu();
        }
        // Chief content is licensed: it is read in place and never copied
        // into the User Catalog or an export.
        ui.add_enabled(false, egui::Button::new("Add to User Library"))
            .on_disabled_hover_text(LICENSE_NOTE);
    });
    action
}

/// The floating windows: object info and the catalog-folder override.
pub fn windows(ctx: &egui::Context, st: &mut ChiefBrowser) -> Option<ChiefAction> {
    let mut action = None;
    let mut keep = st.info.is_some();
    if let Some(info) = st.info.clone() {
        egui::Window::new(format!("Open Object \u{2014} {}", info.object.name))
            .id(egui::Id::new("chief_object_info"))
            .open(&mut keep)
            .collapsible(false)
            .default_width(360.0)
            .show(ctx, |ui| {
                egui::Grid::new("chief_object_info_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        for (k, v) in info.lines() {
                            ui.strong(k);
                            ui.add(egui::Label::new(v).wrap());
                            ui.end_row();
                        }
                    });
            });
    }
    if !keep {
        st.info = None;
    }

    if let Some(mut text) = st.folder_window.take() {
        let mut open = true;
        let mut apply = false;
        egui::Window::new("Chief catalog folders")
            .id(egui::Id::new("chief_catalog_folders"))
            .open(&mut open)
            .collapsible(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.label(
                    "Install folder holding Core Libraries, Bonus Libraries and Manufacturer \
                     Libraries. Leave empty for Chief's standard location.",
                );
                ui.add(
                    egui::TextEdit::singleline(&mut text)
                        .hint_text("/Library/Application Support/Chief Architect Premier X18")
                        .desired_width(f32::INFINITY),
                );
                let found = chief::install_folder_exists(
                    (!text.trim().is_empty()).then(|| std::path::Path::new(text.trim())),
                );
                ui.weak(if found {
                    "Folder found."
                } else {
                    "Folder not found."
                });
                ui.horizontal(|ui| {
                    if ui.button("Browse\u{2026}").clicked() {
                        if let Some(p) = rfd::FileDialog::new().pick_folder() {
                            text = p.to_string_lossy().into_owned();
                        }
                    }
                    if ui.button("Use standard location").clicked() {
                        text.clear();
                    }
                    if ui.button("Apply").clicked() {
                        apply = true;
                    }
                });
                ui.separator();
                ui.weak(LICENSE_NOTE);
            });
        if apply {
            let folder = (!text.trim().is_empty()).then(|| std::path::PathBuf::from(text.trim()));
            let new = ChiefSettings {
                enabled: st.settings.enabled,
                folder,
            };
            st.notice = match st.apply_settings(new) {
                Some(m) => m,
                None => "Chief catalog folder saved; expand a node to rescan.".into(),
            };
            action = Some(ChiefAction::Message(st.notice.clone()));
        } else if open {
            st.folder_window = Some(text);
        }
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_calib::RegistryEntry;
    use std::path::PathBuf;

    fn entry(name: &str, kind: CatalogKind, installed: bool) -> RegistryEntry {
        RegistryEntry {
            uuid: format!("uuid-{name}"),
            name: name.into(),
            kind,
            path: installed.then(|| PathBuf::from(format!("/nowhere/{name}.calib"))),
        }
    }

    fn lib() -> ChiefLibrary {
        ChiefLibrary::from_entries(vec![
            entry("Zeta", CatalogKind::Manufacturer, true),
            entry("Core B", CatalogKind::Core, true),
            entry("Core A", CatalogKind::Core, true),
            entry("Bonus One", CatalogKind::Bonus, true),
            entry("Missing", CatalogKind::Bonus, false),
            entry("Gone", CatalogKind::Deleted, true),
            entry("Mine", CatalogKind::User, true),
        ])
    }

    fn obj(id: i64, name: &str, path: &[&str], kw: &[&str]) -> ObjectSummary {
        ObjectSummary {
            library_object_id: id,
            unique_id: format!("u{id}"),
            type_code: 0,
            name: name.into(),
            category_path: path.iter().map(|s| s.to_string()).collect(),
            keywords: kw.iter().map(|s| s.to_string()).collect(),
            has_thumbnail: true,
            metric: false,
        }
    }

    #[test]
    fn four_top_level_groups_in_chief_order() {
        let library = lib();
        let g = groups(&library);
        let titles: Vec<&str> = g.iter().map(|g| g.title).collect();
        assert_eq!(titles, group_titles());
        let names = |g: &Group| -> Vec<String> {
            g.catalogs
                .iter()
                .map(|&i| library.catalogs()[i].name.clone())
                .collect()
        };
        assert_eq!(names(&g[0]), ["Core A", "Core B"]);
        assert_eq!(
            names(&g[1]),
            ["Bonus One"],
            "uninstalled entries are left out"
        );
        assert_eq!(names(&g[2]), ["Zeta"]);
        assert_eq!(names(&g[3]), ["Mine"]);
        assert_eq!(g[0].kind, CatalogKind::Core);
    }

    #[test]
    fn category_lists_filter_by_path_and_query() {
        let view = CatalogView {
            tree: ChiefNode {
                uuid: "r".into(),
                name: "Interiors".into(),
                direct_count: 0,
                children: Vec::new(),
            },
            objects: vec![
                obj(
                    1,
                    "Round Tank Toilet",
                    &["Interiors", "Bath", "Toilets"],
                    &["wc"],
                ),
                obj(2, "Bathtub", &["Interiors", "Bath", "Tubs"], &["soak"]),
                obj(3, "Sofa", &["Interiors", "Living"], &[]),
            ],
        };
        let bath = vec!["Interiors".to_string(), "Bath".to_string()];
        let names = |v: Vec<&ObjectSummary>| v.iter().map(|o| o.name.clone()).collect::<Vec<_>>();
        assert_eq!(
            names(view.objects_in(&bath, "")),
            ["Bathtub", "Round Tank Toilet"],
            "by name"
        );
        assert_eq!(names(view.objects_in(&bath, "WC")), ["Round Tank Toilet"]);
        assert_eq!(view.objects_in(&["Interiors".to_string()], "").len(), 3);
        assert!(view.objects_in(&bath, "sofa").is_empty());
    }

    #[test]
    fn info_window_lists_the_object_and_the_licence_note() {
        let info = ObjectInfo {
            catalog_name: "Core Interiors".into(),
            object: obj(
                9,
                "Round Tank Toilet",
                &["Interiors", "Bath"],
                &["wc", "toilet"],
            ),
            size: Some([15.5, 28.0, 30.0]),
        };
        let lines = info.lines();
        let get = |k: &str| lines.iter().find(|(l, _)| *l == k).unwrap().1.clone();
        assert_eq!(get("Name"), "Round Tank Toilet");
        assert_eq!(get("Category"), "Interiors \u{25B8} Bath");
        assert_eq!(get("Keywords"), "wc, toilet");
        assert!(get("Size").starts_with("15.5 \u{00D7} 28 \u{00D7} 30 in"));
        assert_eq!(get("Source catalog"), "Core Interiors");
        assert!(get("Licence").contains("Chief Architect licensed content"));
        let none = ObjectInfo { size: None, ..info };
        assert!(none
            .lines()
            .iter()
            .any(|(k, v)| *k == "Size" && v == "not decoded"));
    }

    #[test]
    fn status_line_follows_the_scan() {
        let mut b = ChiefBrowser::new(ChiefSettings {
            enabled: true,
            folder: None,
        });
        assert_eq!(b.status_line(), "");
        b.set_library(lib());
        assert_eq!(b.status_line(), "5 Chief catalogs found (2 not installed)");
        let (tx, rx) = channel::<ChiefLibrary>();
        b.scan = Scan::Running(rx);
        assert_eq!(b.status_line(), "Scanning Chief catalogs\u{2026}");
        tx.send(lib()).unwrap();
        b.poll();
        assert!(matches!(b.scan, Scan::Done));
        assert!(b.library().is_some());
    }
}
