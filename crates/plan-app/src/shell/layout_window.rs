//! Chief's layout view: the project's layout drawn at zoom in the main area,
//! with page tabs at the bottom, layout boxes you select, move and resize,
//! Send to Layout, the Layout Box Specification, Page Setup, Project
//! Information, the Layout Page Table and Print / PDF.
//!
//! * The layout lives in `Project::layout` as the JSON of a
//!   `plan_layout::Layout` ([`load`] / [`store`]), so it is saved and opened
//!   with the `.psplan`. The title block macros read `Project::info`
//!   (Tools > Project Information); see [`macro_context`].
//! * [`LayoutView`] holds the window state (current page, selection, zoom,
//!   a local undo history of the layout, the drawing cache) and every edit as a
//!   plain method on `&mut Project`, so the model logic runs without egui.
//! * The app owns one [`LayoutView`] per thread; `main.rs` calls [`dispatch`],
//!   [`show_central`] and [`show_dialogs`].
//!
//! Layout space is paper inches with the origin at the sheet's bottom-left.
//!
//! Undo: the layout has its own history (pushed and popped as whole-layout
//! snapshots), used while the layout view is showing (Edit > Undo, Cmd+Z and
//! the Undo button). The plan's history snapshots the whole project, layout
//! included, so undoing a plan edit also restores the layout JSON it held at
//! that moment.

use crate::dialogs::camera as cam;
use crate::dialogs::layout::{
    BoxSpec, BoxSpecDialog, PageChoice, PageRow, PageSetup, PageSetupDialog, PageTableDialog,
    Placement, PrintDialog, SendDialog, SendSource, SendSpec,
};
use crate::dialogs::Outcome;
use crate::editor::EditorContext;
use eframe::egui::{
    self, Align2, Color32, CursorIcon, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
};
use plan_core::{CadItem, CadObject, Id, Point, Project};
use plan_docs::{Scale, CHIEF_SHEET_BACKGROUND};
use plan_elevation::{Line2, LineWeight};
use plan_layout::{
    fit_largest_scale, render_pdf, send_to_layout, source_size_in, BoxSource, Layout, LayoutBox,
    LayoutPage, LayoutRenderContext, MacroContext, TitleBlockStyle, AUTO_SCALE_CEILING,
};
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::f64::consts::TAU;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// Undo steps kept for the layout.
const HISTORY_CAP: usize = 100;
/// Smallest side of a layout box, paper inches.
const MIN_BOX_IN: f64 = 0.25;
/// Drag snap, paper inches (1/16").
const SNAP_IN: f64 = 1.0 / 16.0;
const MIN_ZOOM: f32 = 2.0;
const MAX_ZOOM: f32 = 400.0;
const HANDLE_PX: f32 = 7.0;

fn st(width: f32, color: Color32) -> Stroke {
    Stroke::new(width, color)
}

const INK: Color32 = Color32::from_gray(0x22);
const SURROUND: Color32 = Color32::from_gray(0x3A);
const SELECT_BLUE: Color32 = Color32::from_rgb(0x2F, 0x6C, 0xB3);

// ------------------------------------------------------------- storage --

/// The project's layout, if it has one and its JSON reads back.
pub fn load(project: &Project) -> Option<Layout> {
    serde_json::from_value(project.layout.clone()?).ok()
}

/// Writes `layout` into the project.
pub fn store(project: &mut Project, layout: &Layout) {
    project.layout = serde_json::to_value(layout).ok();
}

/// The values the title block macros expand to: the project's name and its
/// Project Information (Tools > Project Information, `Project::info`).
pub fn macro_context(project: &Project) -> MacroContext {
    let pairs = project.info.macro_pairs();
    let get = |name: &str| {
        pairs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    MacroContext {
        project_name: project.name.clone(),
        client: get("%client%"),
        address: get("%address%"),
        designer: get("%designer%"),
        date: get("%date%"),
        project_number: get("%project.number%"),
        revision: get("%revision%"),
        revisions: project.info.revisions.clone(),
        ..MacroContext::default()
    }
}

/// Today as `2026-10-08` (UTC).
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    crate::templates::format_time(secs)
        .chars()
        .take(10)
        .collect()
}

/// The scale a page announces: its boxes' common scale, `AS NOTED` when they
/// differ, `NTS` when no box is scaled.
fn page_scale_label(page: &LayoutPage) -> String {
    let mut labels: Vec<&str> = page
        .boxes
        .iter()
        .filter(|b| is_scaled(&b.source))
        .map(|b| b.scale.label())
        .collect();
    labels.sort_unstable();
    labels.dedup();
    match labels.as_slice() {
        [] => "NTS".to_string(),
        [one] => (*one).to_string(),
        _ => "AS NOTED".to_string(),
    }
}

fn is_scaled(s: &BoxSource) -> bool {
    matches!(
        s,
        BoxSource::PlanView { .. }
            | BoxSource::Elevation { .. }
            | BoxSource::Section { .. }
            | BoxSource::Camera { .. }
            | BoxSource::CadDetail { .. }
    )
}

// ---------------------------------------------------------- page logic --

/// Content pages are numbered consecutively from the first one's number, in
/// page order; template pages keep their number.
pub fn renumber_pages(layout: &mut Layout) {
    let Some(start) = layout
        .pages
        .iter()
        .find(|p| !p.template_page)
        .map(|p| p.number)
    else {
        return;
    };
    for (n, p) in (start..).zip(layout.pages.iter_mut().filter(|p| !p.template_page)) {
        p.number = n;
    }
}

fn next_box_id(layout: &Layout) -> Id {
    layout
        .pages
        .iter()
        .flat_map(|p| p.boxes.iter().map(|b| b.id))
        .max()
        .map_or(1, |m| m + 1)
}

fn next_page_number(layout: &Layout) -> u32 {
    layout
        .pages
        .iter()
        .map(|p| p.number)
        .max()
        .map_or(1, |m| m + 1)
}

/// Inserts an empty page before or after `index` and returns its index.
pub fn insert_page(layout: &mut Layout, index: usize, before: bool) -> usize {
    let at = if before { index } else { index + 1 }.min(layout.pages.len());
    let number = next_page_number(layout);
    layout.add_page(number, format!("Page {number}"));
    let page = layout.pages.pop().expect("just added");
    layout.pages.insert(at, page);
    renumber_pages(layout);
    at
}

/// Copies page `index` (with new box ids) right after it; returns the copy's index.
pub fn duplicate_page(layout: &mut Layout, index: usize) -> Option<usize> {
    let mut copy = layout.pages.get(index)?.clone();
    let first = next_box_id(layout);
    for (id, b) in (first..).zip(copy.boxes.iter_mut()) {
        b.id = id;
    }
    copy.title = format!("{} copy", copy.title);
    copy.number = next_page_number(layout);
    layout.pages.insert(index + 1, copy);
    renumber_pages(layout);
    Some(index + 1)
}

/// Deletes page `index`. The last printed page cannot be deleted.
pub fn delete_page(layout: &mut Layout, index: usize) -> Result<(), &'static str> {
    let Some(page) = layout.pages.get(index) else {
        return Err("No such page");
    };
    if !page.template_page && layout.content_pages().len() <= 1 {
        return Err("A layout keeps at least one page");
    }
    layout.pages.remove(index);
    renumber_pages(layout);
    Ok(())
}

/// Swaps page `index` with its neighbour (`forward` = the next one) and
/// returns the page's new index.
pub fn exchange_page(layout: &mut Layout, index: usize, forward: bool) -> Option<usize> {
    let other = if forward {
        index.checked_add(1).filter(|o| *o < layout.pages.len())?
    } else {
        index.checked_sub(1)?
    };
    layout.pages.swap(index, other);
    renumber_pages(layout);
    Some(other)
}

/// Removes pages outside `range` (1-based ordinals among the printed pages);
/// template pages stay so their content still repeats.
pub fn pages_in_range(layout: &Layout, range: Option<(usize, usize)>) -> Layout {
    let mut out = layout.clone();
    if let Some((from, to)) = range {
        let mut ordinal = 0;
        out.pages.retain(|p| {
            if p.template_page {
                return true;
            }
            ordinal += 1;
            (from..=to).contains(&ordinal)
        });
    }
    out
}

/// The layout as a PDF: every printed page, or the pages in `range`, with the
/// Project Information in the title blocks and camera boxes drawn.
pub fn print_bytes(layout: &Layout, project: &Project, range: Option<(usize, usize)>) -> Vec<u8> {
    let layout = pages_in_range(layout, range);
    let mut rcx = cam::layout_context(project);
    rcx.macros = macro_context(project);
    render_pdf(&layout, &rcx)
}

/// Writes an edited box back (moving it to its new page when that changed).
/// False when the box no longer exists.
pub fn apply_box_spec(layout: &mut Layout, spec: &BoxSpec) -> bool {
    let id = spec.layout_box.id;
    let Some((pi, bi)) = layout
        .pages
        .iter()
        .enumerate()
        .find_map(|(pi, p)| p.boxes.iter().position(|b| b.id == id).map(|bi| (pi, bi)))
    else {
        return false;
    };
    if layout.pages[pi].number == spec.page || layout.page(spec.page).is_none() {
        layout.pages[pi].boxes[bi] = spec.layout_box.clone();
    } else {
        layout.pages[pi].boxes.remove(bi);
        if let Some(p) = layout.page_mut(spec.page) {
            p.boxes.push(spec.layout_box.clone());
        }
    }
    true
}

// -------------------------------------------------------------- geometry --

/// `[x_min, y_min, x_max, y_max]` of a box, paper inches.
pub fn bounds(b: &LayoutBox) -> [f64; 4] {
    let (a, c) = b.rect_in;
    [a.x.min(c.x), a.y.min(c.y), a.x.max(c.x), a.y.max(c.y)]
}

fn set_bounds(b: &mut LayoutBox, r: [f64; 4]) {
    b.rect_in = (Point::new(r[0], r[1]), Point::new(r[2], r[3]));
}

/// The topmost box of `page` containing the paper point `(x, y)`.
pub fn box_at(page: &LayoutPage, x: f64, y: f64) -> Option<Id> {
    page.boxes.iter().rev().find_map(|b| {
        let r = bounds(b);
        (x >= r[0] && x <= r[2] && y >= r[1] && y <= r[3]).then_some(b.id)
    })
}

/// A resize handle of a selected box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
}

impl Handle {
    const ALL: [Handle; 8] = [
        Handle::N,
        Handle::S,
        Handle::E,
        Handle::W,
        Handle::NE,
        Handle::NW,
        Handle::SE,
        Handle::SW,
    ];

    /// The handle's position on the rectangle `r`, paper inches.
    fn position(self, r: [f64; 4]) -> (f64, f64) {
        let (mx, my) = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
        match self {
            Handle::N => (mx, r[3]),
            Handle::S => (mx, r[1]),
            Handle::E => (r[2], my),
            Handle::W => (r[0], my),
            Handle::NE => (r[2], r[3]),
            Handle::NW => (r[0], r[3]),
            Handle::SE => (r[2], r[1]),
            Handle::SW => (r[0], r[1]),
        }
    }

    fn cursor(self) -> CursorIcon {
        match self {
            Handle::N | Handle::S => CursorIcon::ResizeVertical,
            Handle::E | Handle::W => CursorIcon::ResizeHorizontal,
            Handle::NE | Handle::SW => CursorIcon::ResizeNeSw,
            Handle::NW | Handle::SE => CursorIcon::ResizeNwSe,
        }
    }
}

/// The handle of `r` within `tol` paper inches of `(x, y)`.
pub fn handle_at(r: [f64; 4], x: f64, y: f64, tol: f64) -> Option<Handle> {
    Handle::ALL.into_iter().find(|h| {
        let (hx, hy) = h.position(r);
        (hx - x).abs() <= tol && (hy - y).abs() <= tol
    })
}

fn snap(v: f64, on: bool) -> f64 {
    if on {
        (v / SNAP_IN).round() * SNAP_IN
    } else {
        v
    }
}

/// `r` with handle `h` dragged by `(dx, dy)` paper inches (edges snap when
/// `snapping`); the box never gets smaller than [`MIN_BOX_IN`].
pub fn resized(r: [f64; 4], h: Handle, dx: f64, dy: f64, snapping: bool) -> [f64; 4] {
    let mut o = r;
    let (west, east) = (
        matches!(h, Handle::W | Handle::NW | Handle::SW),
        matches!(h, Handle::E | Handle::NE | Handle::SE),
    );
    let (south, north) = (
        matches!(h, Handle::S | Handle::SE | Handle::SW),
        matches!(h, Handle::N | Handle::NE | Handle::NW),
    );
    if west {
        o[0] = snap(r[0] + dx, snapping).min(r[2] - MIN_BOX_IN);
    }
    if east {
        o[2] = snap(r[2] + dx, snapping).max(r[0] + MIN_BOX_IN);
    }
    if south {
        o[1] = snap(r[1] + dy, snapping).min(r[3] - MIN_BOX_IN);
    }
    if north {
        o[3] = snap(r[3] + dy, snapping).max(r[1] + MIN_BOX_IN);
    }
    o
}

/// `r` moved by `(dx, dy)`, its lower-left corner snapped when `snapping`.
pub fn moved(r: [f64; 4], dx: f64, dy: f64, snapping: bool) -> [f64; 4] {
    let (w, h) = (r[2] - r[0], r[3] - r[1]);
    let x = snap(r[0] + dx, snapping);
    let y = snap(r[1] + dy, snapping);
    [x, y, x + w, y + h]
}

// ---------------------------------------------------------------- state --

/// Everything the layout window needs to ask the application for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LayoutCommand {
    /// File > Open Layout / Window > Layout: show the layout view.
    ShowLayout,
    /// Window > Floor Plan View: back to the plan.
    ShowPlan,
    SendToLayout,
    SendAllFloors,
    PageSetup,
    ProjectInfo,
    PageTable,
    InsertPageBefore,
    InsertPageAfter,
    DuplicatePage,
    DeletePage,
    ExchangeWithPrevious,
    ExchangeWithNext,
    NextPage,
    PreviousPage,
    GoToPage(usize),
    BoxSpecification,
    DeleteBox,
    UpdateViews,
    FitPage,
    Print,
    ExportPdf,
    Undo,
    Redo,
}

#[derive(Clone)]
enum Drag {
    Pan,
    Move {
        id: Id,
        start: (f64, f64),
        orig: [f64; 4],
        before: Box<Layout>,
    },
    Resize {
        id: Id,
        handle: Handle,
        start: (f64, f64),
        orig: [f64; 4],
        before: Box<Layout>,
    },
}

/// A Send to Layout waiting for the click that places it.
struct Placing {
    source: BoxSource,
    scale: Option<Scale>,
    page: PageChoice,
    /// The size of the box that will be placed, paper inches (the ghost).
    size: (f64, f64),
}

#[derive(Default)]
struct BoxCache {
    map: HashMap<Id, (u64, Rc<Vec<Line2>>)>,
}

#[derive(Default)]
struct Dialogs {
    send: Option<SendDialog>,
    spec: Option<BoxSpecDialog>,
    setup: Option<PageSetupDialog>,
    table: Option<PageTableDialog>,
    print: Option<PrintDialog>,
}

impl Dialogs {
    fn any(&self) -> bool {
        self.send.is_some()
            || self.spec.is_some()
            || self.setup.is_some()
            || self.table.is_some()
            || self.print.is_some()
    }
}

/// The layout window's state.
pub struct LayoutView {
    /// The layout view replaces the plan canvas.
    pub active: bool,
    layout: Option<Layout>,
    /// The JSON last read from or written to the project.
    stored: Option<serde_json::Value>,
    /// Index of the page shown (into `Layout::pages`).
    pub page: usize,
    pub selected: Option<Id>,
    past: Vec<(String, Layout)>,
    future: Vec<(String, Layout)>,
    /// Pixels per paper inch.
    zoom: f32,
    /// Screen offset of the sheet's top-left from the view's top-left.
    offset: Vec2,
    fit_pending: bool,
    drag: Option<Drag>,
    placing: Option<Placing>,
    renaming: Option<(usize, String)>,
    cache: BoxCache,
    sig: u64,
    dialogs: Dialogs,
    /// The paper-to-screen mapping of the last frame (hit tests, tests).
    last_xf: Option<Xf>,
    /// The layout sheet the plan's Drawing Sheet was last matched to.
    synced_sheet: Option<plan_docs::SheetSize>,
}

impl Default for LayoutView {
    fn default() -> Self {
        Self {
            active: false,
            layout: None,
            stored: None,
            page: 0,
            selected: None,
            past: Vec::new(),
            future: Vec::new(),
            zoom: 20.0,
            offset: Vec2::ZERO,
            fit_pending: true,
            drag: None,
            placing: None,
            renaming: None,
            cache: BoxCache::default(),
            sig: 0,
            dialogs: Dialogs::default(),
            last_xf: None,
            synced_sheet: None,
        }
    }
}

/// Hash of what plan views, elevations and camera drawings depend on.
fn project_sig(p: &Project) -> u64 {
    let mut h = DefaultHasher::new();
    let parts = [
        serde_json::to_string(&p.floors),
        serde_json::to_string(&p.layers),
        serde_json::to_string(&p.layer_sets),
        serde_json::to_string(&p.cameras),
        serde_json::to_string(&p.text_styles),
        serde_json::to_string(&p.terrain),
    ];
    for s in parts {
        s.unwrap_or_default().hash(&mut h);
    }
    h.finish()
}

fn box_key(b: &LayoutBox, sig: u64) -> u64 {
    let mut h = DefaultHasher::new();
    sig.hash(&mut h);
    for p in [b.rect_in.0, b.rect_in.1] {
        p.x.to_bits().hash(&mut h);
        p.y.to_bits().hash(&mut h);
    }
    b.scale.hash(&mut h);
    b.border.hash(&mut h);
    b.clip.hash(&mut h);
    b.line_weight_scale.to_bits().hash(&mut h);
    b.hatch_materials.hash(&mut h);
    match &b.source {
        BoxSource::ImageData {
            width,
            height,
            rgba,
        } => (width, height, rgba.len()).hash(&mut h),
        other => format!("{other:?}").hash(&mut h),
    }
    h.finish()
}

impl LayoutView {
    // ----- model access -----

    /// Re-reads the layout when the project's JSON changed under us (a file
    /// was opened, or the plan's undo restored an older snapshot).
    pub fn sync(&mut self, project: &Project) {
        if project.layout == self.stored {
            return;
        }
        self.layout = load(project);
        self.stored = project.layout.clone();
        self.past.clear();
        self.future.clear();
        self.cache.map.clear();
        self.drag = None;
        self.placing = None;
        self.clamp_page();
    }

    pub fn layout(&self) -> Option<&Layout> {
        self.layout.as_ref()
    }

    pub fn current_page(&self) -> Option<&LayoutPage> {
        self.layout.as_ref()?.pages.get(self.page)
    }

    fn clamp_page(&mut self) {
        let n = self.layout.as_ref().map_or(0, |l| l.pages.len());
        self.page = self.page.min(n.saturating_sub(1));
        if let Some(id) = self.selected {
            let there = self
                .current_page()
                .is_some_and(|p| p.boxes.iter().any(|b| b.id == id));
            if !there {
                self.selected = None;
            }
        }
    }

    fn write_back(&mut self, project: &mut Project) {
        if let Some(l) = &self.layout {
            store(project, l);
            self.stored = project.layout.clone();
        }
    }

    /// Replaces the layout with `new`, recording the step for undo.
    fn commit(&mut self, project: &mut Project, label: &str, new: Layout) {
        if let Some(old) = self.layout.take() {
            self.past.push((label.to_string(), old));
            if self.past.len() > HISTORY_CAP {
                self.past.remove(0);
            }
        }
        self.future.clear();
        self.layout = Some(new);
        self.write_back(project);
        self.clamp_page();
    }

    /// Runs `f` on a copy of the layout; when it returns true the copy
    /// becomes the layout, as the undo step `label`.
    pub fn edit(
        &mut self,
        project: &mut Project,
        label: &str,
        f: impl FnOnce(&mut Layout) -> bool,
    ) -> bool {
        let Some(mut copy) = self.layout.clone() else {
            return false;
        };
        if !f(&mut copy) {
            return false;
        }
        self.commit(project, label, copy);
        true
    }

    /// Steps back; returns the undone step's label.
    pub fn undo(&mut self, project: &mut Project) -> Option<String> {
        let (label, prev) = self.past.pop()?;
        if let Some(cur) = self.layout.replace(prev) {
            self.future.push((label.clone(), cur));
        }
        self.write_back(project);
        self.clamp_page();
        Some(label)
    }

    pub fn redo(&mut self, project: &mut Project) -> Option<String> {
        let (label, next) = self.future.pop()?;
        if let Some(cur) = self.layout.replace(next) {
            self.past.push((label.clone(), cur));
        }
        self.write_back(project);
        self.clamp_page();
        Some(label)
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.past.last().map(|(l, _)| l.as_str())
    }

    // ----- creating -----

    /// Makes the project's layout from the template (Daniel's title block, a
    /// template page 0 and an empty page 1) unless it has one. True when a
    /// layout was created.
    pub fn create(
        &mut self,
        project: &mut Project,
        seed: Option<&crate::templates::LayoutInfoSeed>,
    ) -> bool {
        self.sync(project);
        if self.layout.is_some() {
            return false;
        }
        let layout = crate::templates::new_layout(&format!("{} Layout", project.name), seed);
        self.past.clear();
        self.future.clear();
        self.layout = Some(layout);
        self.write_back(project);
        // %date% needs a date; fill it in when Project Information has none.
        if project.info.date.trim().is_empty() {
            project.info.date = today();
        }
        self.page = self
            .layout
            .as_ref()
            .and_then(|l| l.pages.iter().position(|p| !p.template_page))
            .unwrap_or(0);
        self.selected = None;
        self.fit_pending = true;
        true
    }

    // ----- pages -----

    pub fn set_page(&mut self, index: usize) {
        let n = self.layout.as_ref().map_or(0, |l| l.pages.len());
        if index < n && index != self.page {
            self.page = index;
            self.selected = None;
            self.renaming = None;
        }
    }

    pub fn add_page(&mut self, project: &mut Project, before: bool) -> bool {
        let at = self.page;
        let mut new_index = at;
        let done = self.edit(project, "Insert Page", |l| {
            new_index = insert_page(l, at, before);
            true
        });
        if done {
            self.set_page(new_index);
        }
        done
    }

    pub fn duplicate_current_page(&mut self, project: &mut Project) -> bool {
        let at = self.page;
        let mut new_index = None;
        let done = self.edit(project, "Duplicate Page", |l| {
            new_index = duplicate_page(l, at);
            new_index.is_some()
        });
        if let Some(i) = new_index.filter(|_| done) {
            self.set_page(i);
        }
        done
    }

    pub fn delete_current_page(&mut self, project: &mut Project) -> Result<(), &'static str> {
        let at = self.page;
        let mut result = Ok(());
        self.edit(project, "Delete Page", |l| {
            result = delete_page(l, at);
            result.is_ok()
        });
        self.clamp_page();
        self.selected = None;
        result
    }

    pub fn exchange_current_page(&mut self, project: &mut Project, forward: bool) -> bool {
        let at = self.page;
        let mut new_index = None;
        let done = self.edit(project, "Exchange Pages", |l| {
            new_index = exchange_page(l, at, forward);
            new_index.is_some()
        });
        if let Some(i) = new_index.filter(|_| done) {
            self.page = i;
        }
        done
    }

    pub fn rename_page(&mut self, project: &mut Project, index: usize, title: &str) -> bool {
        let title = title.trim().to_string();
        self.edit(project, "Rename Page", |l| {
            match l.pages.get_mut(index).filter(|p| p.title != title) {
                Some(p) => {
                    p.title = title;
                    true
                }
                None => false,
            }
        })
    }

    pub fn apply_page_table(&mut self, project: &mut Project, rows: &[PageRow]) -> bool {
        self.edit(project, "Layout Page Table", |l| {
            let mut changed = false;
            for (p, r) in l.pages.iter_mut().zip(rows) {
                if p.title != r.title || p.template_page != r.template_page {
                    p.title = r.title.clone();
                    p.template_page = r.template_page;
                    changed = true;
                }
            }
            if changed {
                renumber_pages(l);
            }
            changed
        })
    }

    pub fn page_rows(&self) -> Vec<PageRow> {
        self.layout
            .as_ref()
            .map(|l| {
                l.pages
                    .iter()
                    .map(|p| PageRow {
                        number: p.number,
                        title: p.title.clone(),
                        template_page: p.template_page,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    // ----- page setup and project information -----

    pub fn page_setup(&self) -> Option<PageSetup> {
        let l = self.layout.as_ref()?;
        Some(PageSetup {
            sheet: l.sheet,
            margins_in: l.margins_in,
            page_background: l.page_background,
            edge_line_weight: l.edge_line_weight,
            sheet_index: l.sheet_index,
        })
    }

    pub fn apply_page_setup(&mut self, project: &mut Project, s: &PageSetup) -> bool {
        self.edit(project, "Page Setup", |l| {
            let changed = l.sheet != s.sheet
                || (l.margins_in - s.margins_in).abs() > 1e-9
                || l.page_background != s.page_background
                || l.edge_line_weight != s.edge_line_weight
                || l.sheet_index != s.sheet_index;
            l.sheet = s.sheet;
            l.margins_in = s.margins_in;
            l.page_background = s.page_background;
            l.edge_line_weight = s.edge_line_weight;
            l.sheet_index = s.sheet_index;
            changed
        })
    }

    // ----- boxes -----

    fn selected_box(&self) -> Option<&LayoutBox> {
        let id = self.selected?;
        self.current_page()?.boxes.iter().find(|b| b.id == id)
    }

    /// Moves a box to `r` (paper inches) as one undo step.
    pub fn set_box_bounds(
        &mut self,
        project: &mut Project,
        id: Id,
        r: [f64; 4],
        label: &str,
    ) -> bool {
        self.edit(project, label, |l| {
            for p in &mut l.pages {
                if let Some(b) = p.boxes.iter_mut().find(|b| b.id == id) {
                    if bounds(b) == r {
                        return false;
                    }
                    set_bounds(b, r);
                    return true;
                }
            }
            false
        })
    }

    pub fn delete_selected(&mut self, project: &mut Project) -> bool {
        let Some(id) = self.selected else {
            return false;
        };
        let done = self.edit(project, "Delete Layout Box", |l| {
            let mut found = false;
            for p in &mut l.pages {
                let n = p.boxes.len();
                p.boxes.retain(|b| b.id != id);
                found |= p.boxes.len() != n;
            }
            found
        });
        if done {
            self.selected = None;
        }
        done
    }

    pub fn apply_spec(&mut self, project: &mut Project, spec: &BoxSpec) -> bool {
        let done = self.edit(project, "Layout Box Specification", |l| {
            let before = l.clone();
            apply_box_spec(l, spec) && *l != before
        });
        if done {
            if let Some(i) = self
                .layout
                .as_ref()
                .and_then(|l| l.pages.iter().position(|p| p.number == spec.page))
            {
                self.page = i;
            }
            self.cache.map.remove(&spec.layout_box.id);
        }
        done
    }

    // ----- Send to Layout -----

    /// Sends a view to the layout (L-1..L-3): a new box on the chosen page at
    /// the chosen scale, "largest that fits" when `spec.scale` is `None`.
    /// `center` is the paper point a [`Placement::Click`] box is centered on.
    pub fn send(
        &mut self,
        project: &mut Project,
        spec: &SendSpec,
        center: Option<(f64, f64)>,
    ) -> Result<Id, String> {
        let Some(mut layout) = self.layout.clone() else {
            return Err("There is no layout to send to".into());
        };
        let source = match &spec.source {
            SendSource::Plan { floor, layer_set } => {
                if *floor >= project.floors.len() {
                    return Err("That floor does not exist".into());
                }
                BoxSource::PlanView {
                    floor: *floor,
                    layer_set: layer_set.clone(),
                }
            }
            SendSource::Camera { id, .. } => {
                if project.camera(*id).is_none() {
                    return Err("That camera does not exist".into());
                }
                BoxSource::Camera { camera_id: *id }
            }
        };
        let page_no = match spec.page {
            PageChoice::Existing(n) if layout.page(n).is_some() => n,
            _ => {
                let n = next_page_number(&layout);
                layout.add_page(n, format!("Page {n}"));
                n
            }
        };
        let rcx = cam::layout_context(project);
        let scale = spec
            .scale
            .unwrap_or_else(|| fit_largest_scale(&layout, &rcx, &source, AUTO_SCALE_CEILING));
        let centre = match (spec.placement, center) {
            (Placement::FirstFree, _) | (Placement::Click, None) => None,
            (Placement::Centered, _) => {
                let (lo, hi) = layout.drawing_area();
                Some(((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0))
            }
            (Placement::Click, Some(c)) => Some(c),
        };
        let at = centre.map(|(cx, cy)| {
            let (w, h) = source_size_in(&source, scale, &rcx);
            Point::new(cx - w / 2.0, cy - h / 2.0)
        });
        let id = send_to_layout(&mut layout, &rcx, page_no, source, scale, at);
        drop(rcx);
        self.commit(project, "Send to Layout", layout);
        if let Some(i) = self
            .layout
            .as_ref()
            .and_then(|l| l.pages.iter().position(|p| p.number == page_no))
        {
            self.page = i;
        }
        self.selected = Some(id);
        Ok(id)
    }

    /// Sends every floor plan, one per page: the first onto the current page
    /// when it is empty, the rest onto new pages titled by their floor.
    pub fn send_all_floors(&mut self, project: &mut Project) -> usize {
        let layer_set = project.layer_sets.active.clone();
        let mut sent = 0;
        for floor in 0..project.floors.len() {
            let empty_here = sent == 0
                && self
                    .current_page()
                    .is_some_and(|p| !p.template_page && p.boxes.is_empty());
            let page = if empty_here {
                PageChoice::Existing(self.current_page().map_or(1, |p| p.number))
            } else {
                PageChoice::New
            };
            let spec = SendSpec {
                source: SendSource::Plan {
                    floor,
                    layer_set: layer_set.clone(),
                },
                page,
                scale: None,
                placement: Placement::FirstFree,
            };
            if self.send(project, &spec, None).is_ok() {
                let title = plan_layout::plan_label(project, floor);
                let at = self.page;
                self.rename_page_quiet(at, &title);
                sent += 1;
            }
        }
        self.write_back(project);
        sent
    }

    /// Renames a page without a history step (part of a bigger one).
    fn rename_page_quiet(&mut self, index: usize, title: &str) {
        if let Some(p) = self.layout.as_mut().and_then(|l| l.pages.get_mut(index)) {
            p.title = title.to_string();
        }
    }

    // ----- viewing -----

    fn fit(&mut self, area: Rect) {
        let Some(l) = &self.layout else { return };
        let (w, h) = l.sheet.inches();
        let z = ((area.width() - 40.0) / w as f32).min((area.height() - 40.0) / h as f32);
        self.zoom = z.clamp(MIN_ZOOM, MAX_ZOOM);
        self.offset = Vec2::new(
            (area.width() - w as f32 * self.zoom) / 2.0,
            (area.height() - h as f32 * self.zoom) / 2.0,
        );
        self.fit_pending = false;
    }

    fn zoom_about(&mut self, area: Rect, pointer: Pos2, factor: f32) {
        let z0 = self.zoom;
        let z1 = (z0 * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let origin = area.min + self.offset;
        // The paper point under the pointer stays under it.
        let px = (pointer.x - origin.x) / z0;
        let py = (pointer.y - origin.y) / z0;
        self.zoom = z1;
        self.offset = Vec2::new(
            pointer.x - px * z1 - area.min.x,
            pointer.y - py * z1 - area.min.y,
        );
    }
}

// ---------------------------------------------------------------- thread --

thread_local! {
    static VIEW: RefCell<LayoutView> = RefCell::new(LayoutView::default());
}

fn with_view<R>(f: impl FnOnce(&mut LayoutView) -> R) -> R {
    VIEW.with(|v| f(&mut v.borrow_mut()))
}

/// Is the layout view showing (instead of the plan or the 3D view)?
pub fn is_active() -> bool {
    VIEW.with(|v| v.try_borrow().map(|v| v.active).unwrap_or(true))
}

/// Is a layout dialog open (the canvas keys must not reach the tools)?
pub fn dialog_open() -> bool {
    VIEW.with(|v| v.try_borrow().map(|v| v.dialogs.any()).unwrap_or(true))
}

/// Leaves the layout view (the plan or 3D view takes over).
pub fn deactivate() {
    with_view(|v| v.active = false);
}

/// `(index, "A-1  Title", template page)` of every page, read from the project's
/// layout JSON (for the Project Browser).
pub fn page_list(project: &Project) -> Vec<(usize, String, bool)> {
    let Some(pages) = project
        .layout
        .as_ref()
        .and_then(|l| l.get("pages"))
        .and_then(|p| p.as_array())
    else {
        return Vec::new();
    };
    pages
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let n = p.get("number").and_then(|n| n.as_u64()).unwrap_or(0);
            let title = p.get("title").and_then(|t| t.as_str()).unwrap_or("");
            let template = p
                .get("template_page")
                .and_then(|t| t.as_bool())
                .unwrap_or(false);
            (i, format!("A-{n}  {title}"), template)
        })
        .collect()
}

/// The page the layout view shows.
pub fn current_page_index() -> usize {
    with_view(|v| v.page)
}

/// File > New Layout: makes the project's layout from Daniel's template and
/// shows it. A project keeps one layout; an existing one is opened instead.
pub fn new_layout(cx: &mut EditorContext) {
    let settings = crate::templates::load_settings();
    let seed = crate::templates::refresh(&settings, false);
    let made = with_view(|v| {
        let made = v.create(&mut cx.project, seed.cache.layout.as_ref());
        v.active = true;
        made
    });
    cx.status = if made {
        "New layout: page template and page 1".into()
    } else {
        "This plan already has a layout; opened it".into()
    };
    sync_sheet(cx);
}

/// Keeps the plan's Drawing Sheet outline on the layout's sheet size.
fn sync_sheet(cx: &mut EditorContext) {
    let changed = with_view(|v| {
        let now = v.layout().map(|l| l.sheet);
        (now != v.synced_sheet).then(|| {
            v.synced_sheet = now;
            now
        })
    });
    if let Some(Some(size)) = changed {
        cx.sheet.size = size;
    }
}

/// Runs a layout command. `camera` is the elevation or section camera of the
/// open 3D view, which Send to Layout sends instead of the plan.
pub fn dispatch(cmd: LayoutCommand, cx: &mut EditorContext, camera: Option<Id>) {
    let mut view = with_view(std::mem::take);
    view.run(cx, cmd, camera);
    with_view(|slot| *slot = view);
    sync_sheet(cx);
}

/// Send to Layout for the camera `id` (what the 3D panel's buttons call):
/// opens the Send to Layout dialog on that camera.
#[allow(dead_code)] // The 3D panel moves to this once it drops its own session layout.
pub fn send_camera(cx: &mut EditorContext, id: Id) {
    dispatch(LayoutCommand::SendToLayout, cx, Some(id));
}

/// The project's layout as PDF bytes, if it has any printed pages.
pub fn layout_pdf(project: &Project) -> Option<Vec<u8>> {
    let layout = load(project)?;
    (!layout.content_pages().is_empty()).then(|| print_bytes(&layout, project, None))
}

/// Edit > Undo while the layout view shows.
pub fn undo(cx: &mut EditorContext) -> Option<String> {
    with_view(|v| v.undo(&mut cx.project))
}

pub fn redo(cx: &mut EditorContext) -> Option<String> {
    with_view(|v| v.redo(&mut cx.project))
}

// ------------------------------------------------------------- commands --

impl LayoutView {
    fn ensure(&mut self, cx: &mut EditorContext) {
        self.sync(&cx.project);
        if self.layout.is_none() {
            let settings = crate::templates::load_settings();
            let seed = crate::templates::refresh(&settings, false);
            self.create(&mut cx.project, seed.cache.layout.as_ref());
        }
    }

    fn page_list(&self) -> Vec<(u32, String)> {
        self.layout
            .as_ref()
            .map(|l| {
                l.pages
                    .iter()
                    .filter(|p| !p.template_page)
                    .map(|p| (p.number, p.title.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Executes `cmd`; the result is reported in the status bar.
    pub fn run(&mut self, cx: &mut EditorContext, cmd: LayoutCommand, camera: Option<Id>) {
        use LayoutCommand as C;
        if cmd == C::ShowPlan {
            self.active = false;
            return;
        }
        self.ensure(cx);
        let stays = matches!(
            cmd,
            C::SendToLayout
                | C::Print
                | C::ExportPdf
                | C::PageSetup
                | C::ProjectInfo
                | C::PageTable
                | C::Undo
                | C::Redo
                | C::FitPage
        );
        if !stays {
            self.active = true;
        }
        let project = &mut cx.project;
        let status: String = match cmd {
            C::ShowLayout => String::new(),
            C::ShowPlan => String::new(),
            C::SendToLayout => {
                let source = self.send_source(project, camera, cx.floor);
                let floors = project.floors.iter().map(|f| f.name.clone()).collect();
                let sets = project
                    .layer_sets
                    .sets
                    .iter()
                    .map(|s| s.name.clone())
                    .collect();
                let current = self
                    .current_page()
                    .filter(|p| !p.template_page)
                    .map(|p| p.number);
                self.dialogs.send = Some(SendDialog::new(
                    source,
                    self.page_list(),
                    current,
                    floors,
                    sets,
                ));
                String::new()
            }
            C::SendAllFloors => {
                let n = self.send_all_floors(project);
                format!("Sent {n} floor plan(s) to the layout")
            }
            C::PageSetup => {
                self.dialogs.setup = self.page_setup().map(PageSetupDialog::new);
                String::new()
            }
            C::ProjectInfo => {
                // Tools > Project Information (the schedules builder's dialog).
                cx.requests.push(crate::editor::EditorRequest::SetTool(
                    crate::tools::ToolId::ProjectInfo,
                ));
                String::new()
            }
            C::PageTable => {
                self.dialogs.table = Some(PageTableDialog::new(self.page_rows()));
                String::new()
            }
            C::InsertPageBefore | C::InsertPageAfter => {
                self.add_page(project, cmd == C::InsertPageBefore);
                "Inserted a page".into()
            }
            C::DuplicatePage => {
                self.duplicate_current_page(project);
                "Duplicated the page".into()
            }
            C::DeletePage => match self.delete_current_page(project) {
                Ok(()) => "Deleted the page".into(),
                Err(e) => e.to_string(),
            },
            C::ExchangeWithPrevious | C::ExchangeWithNext => {
                if self.exchange_current_page(project, cmd == C::ExchangeWithNext) {
                    "Exchanged pages".into()
                } else {
                    "There is no page to exchange with".into()
                }
            }
            C::NextPage => {
                self.set_page(self.page + 1);
                String::new()
            }
            C::PreviousPage => {
                self.set_page(self.page.saturating_sub(1));
                String::new()
            }
            C::GoToPage(i) => {
                self.set_page(i);
                String::new()
            }
            C::BoxSpecification => match self.selected_box().cloned() {
                Some(b) => {
                    self.open_spec(project, &b);
                    String::new()
                }
                None => "Select a layout box first".into(),
            },
            C::DeleteBox => {
                if self.delete_selected(project) {
                    "Deleted the layout box".into()
                } else {
                    "Select a layout box first".into()
                }
            }
            C::UpdateViews => {
                self.cache.map.clear();
                "Updated the layout views".into()
            }
            C::FitPage => {
                self.fit_pending = true;
                String::new()
            }
            C::Print => {
                let n = self.layout.as_ref().map_or(0, |l| l.content_pages().len());
                self.dialogs.print = Some(PrintDialog::new(n));
                String::new()
            }
            C::ExportPdf => self.export_pdf(project, None),
            C::Undo => match self.undo(project) {
                Some(l) => format!("Undid {l}"),
                None => "Nothing to undo".into(),
            },
            C::Redo => match self.redo(project) {
                Some(l) => format!("Redid {l}"),
                None => "Nothing to redo".into(),
            },
        };
        if !status.is_empty() {
            cx.status = status;
        }
    }

    fn send_source(&self, project: &Project, camera: Option<Id>, floor: usize) -> SendSource {
        if let Some(c) = camera
            .and_then(|id| project.camera(id))
            .filter(|c| cam::is_elevation_camera(c))
        {
            return SendSource::Camera {
                id: c.id,
                name: c.name.clone(),
            };
        }
        SendSource::Plan {
            floor: floor.min(project.floors.len().saturating_sub(1)),
            layer_set: project.layer_sets.active.clone(),
        }
    }

    fn open_spec(&mut self, project: &Project, b: &LayoutBox) {
        let Some(page) = self.current_page() else {
            return;
        };
        let floors = project.floors.iter().map(|f| f.name.clone()).collect();
        let sets = project
            .layer_sets
            .sets
            .iter()
            .map(|s| s.name.clone())
            .collect();
        self.dialogs.spec = Some(BoxSpecDialog::new(
            b,
            page.number,
            self.page_list(),
            floors,
            sets,
        ));
    }

    /// Asks for a file and saves the layout as a PDF; returns the status text.
    fn export_pdf(&self, project: &Project, range: Option<(usize, usize)>) -> String {
        let Some(layout) = &self.layout else {
            return "There is no layout".into();
        };
        if layout.content_pages().is_empty() {
            return "The layout has no pages to print".into();
        }
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(format!("{}.pdf", layout.name))
            .add_filter("PDF", &["pdf"])
            .save_file()
        else {
            return "Print cancelled".into();
        };
        let bytes = print_bytes(layout, project, range);
        match std::fs::write(&path, bytes) {
            Ok(()) => format!("Saved {}", path.display()),
            Err(e) => format!("Could not save: {e}"),
        }
    }
}

// --------------------------------------------------------------- dialogs --

/// Shows the layout dialogs and applies what was accepted.
pub fn show_dialogs(ctx: &egui::Context, cx: &mut EditorContext) {
    let mut view = with_view(std::mem::take);
    view.show_dialogs(ctx, cx);
    with_view(|slot| *slot = view);
    sync_sheet(cx);
}

impl LayoutView {
    fn show_dialogs(&mut self, ctx: &egui::Context, cx: &mut EditorContext) {
        self.sync(&cx.project);
        if let Some(mut d) = self.dialogs.send.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.send = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => self.finish_send(cx, d.spec().clone()),
            }
        }
        if let Some(mut d) = self.dialogs.spec.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.spec = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    if self.apply_spec(&mut cx.project, &d.result()) {
                        cx.status = "Updated the layout box".into();
                    }
                }
            }
        }
        if let Some(mut d) = self.dialogs.setup.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.setup = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    if self.apply_page_setup(&mut cx.project, d.setup()) {
                        self.fit_pending = true;
                        cx.status = "Page setup changed".into();
                    }
                }
            }
        }
        if let Some(mut d) = self.dialogs.table.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.table = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    if self.apply_page_table(&mut cx.project, d.rows()) {
                        cx.status = "Layout page table updated".into();
                    }
                }
            }
        }
        if let Some(mut d) = self.dialogs.print.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.print = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => cx.status = self.export_pdf(&cx.project, d.range()),
            }
        }
    }

    /// The Send to Layout dialog was accepted.
    fn finish_send(&mut self, cx: &mut EditorContext, spec: SendSpec) {
        if spec.placement == Placement::Click {
            let source = match &spec.source {
                SendSource::Plan { floor, layer_set } => BoxSource::PlanView {
                    floor: *floor,
                    layer_set: layer_set.clone(),
                },
                SendSource::Camera { id, .. } => BoxSource::Camera { camera_id: *id },
            };
            let rcx = cam::layout_context(&cx.project);
            let scale = match (spec.scale, &self.layout) {
                (Some(s), _) => s,
                (None, Some(l)) => fit_largest_scale(l, &rcx, &source, AUTO_SCALE_CEILING),
                (None, None) => AUTO_SCALE_CEILING,
            };
            let size = source_size_in(&source, scale, &rcx);
            drop(rcx);
            self.placing = Some(Placing {
                source,
                scale: spec.scale,
                page: spec.page,
                size,
            });
            // Pages are chosen now so the click lands on the right sheet.
            if let PageChoice::Existing(n) = spec.page {
                if let Some(i) = self
                    .layout
                    .as_ref()
                    .and_then(|l| l.pages.iter().position(|p| p.number == n))
                {
                    self.set_page(i);
                }
            }
            self.active = true;
            cx.status = "Click on the page to place the layout box (Esc cancels)".into();
            return;
        }
        match self.send(&mut cx.project, &spec, None) {
            Ok(_) => {
                self.active = true;
                cx.status = "Sent to layout".into();
            }
            Err(e) => cx.status = e,
        }
    }

    /// The click that places a pending Send to Layout.
    fn place_at(&mut self, cx: &mut EditorContext, x: f64, y: f64) {
        let Some(p) = self.placing.take() else { return };
        let page = match p.page {
            PageChoice::Existing(_) => self
                .current_page()
                .map_or(PageChoice::New, |pg| PageChoice::Existing(pg.number)),
            PageChoice::New => PageChoice::New,
        };
        let source = match &p.source {
            BoxSource::PlanView { floor, layer_set } => SendSource::Plan {
                floor: *floor,
                layer_set: layer_set.clone(),
            },
            BoxSource::Camera { camera_id } => SendSource::Camera {
                id: *camera_id,
                name: String::new(),
            },
            _ => return,
        };
        let spec = SendSpec {
            source,
            page,
            scale: p.scale,
            placement: Placement::Click,
        };
        cx.status = match self.send(&mut cx.project, &spec, Some((x, y))) {
            Ok(_) => "Sent to layout".into(),
            Err(e) => e,
        };
    }
}

// --------------------------------------------------------------- painting --

/// Paper-to-screen mapping of the sheet being drawn.
#[derive(Clone, Copy)]
struct Xf {
    origin: Pos2,
    z: f32,
    h: f64,
}

impl Xf {
    fn pt(&self, x: f64, y: f64) -> Pos2 {
        Pos2::new(
            self.origin.x + x as f32 * self.z,
            self.origin.y + (self.h - y) as f32 * self.z,
        )
    }

    fn paper(&self, p: Pos2) -> (f64, f64) {
        (
            f64::from((p.x - self.origin.x) / self.z),
            self.h - f64::from((p.y - self.origin.y) / self.z),
        )
    }

    fn rect(&self, r: [f64; 4]) -> Rect {
        Rect::from_two_pos(self.pt(r[0], r[1]), self.pt(r[2], r[3]))
    }
}

fn line_width(w: LineWeight, z: f32) -> f32 {
    let k = (z / 20.0).clamp(0.7, 1.8);
    k * match w {
        LineWeight::Heavy => 1.5,
        LineWeight::Medium => 1.0,
        LineWeight::Light => 0.6,
    }
}

/// The lines of a box, from the cache when nothing it depends on changed.
fn box_lines<'a>(
    cache: &mut BoxCache,
    rcx: &mut Option<LayoutRenderContext<'a>>,
    project: &'a Project,
    sig: u64,
    b: &LayoutBox,
) -> Rc<Vec<Line2>> {
    let key = box_key(b, sig);
    if let Some((k, lines)) = cache.map.get(&b.id) {
        if *k == key {
            return lines.clone();
        }
    }
    let rcx = rcx.get_or_insert_with(|| cam::layout_context(project));
    let lines = Rc::new(plan_layout::render_box_lines(b, rcx));
    cache.map.insert(b.id, (key, lines.clone()));
    lines
}

fn paint_box(painter: &egui::Painter, xf: &Xf, b: &LayoutBox, lines: &[Line2], faint: bool) {
    let clip = painter.clip_rect().expand(20.0);
    let tint = |c: Color32| if faint { c.gamma_multiply(0.45) } else { c };
    let shapes: Vec<egui::Shape> = lines
        .iter()
        .filter_map(|l| {
            let (a, c) = (xf.pt(l.a.x, l.a.y), xf.pt(l.b.x, l.b.y));
            Rect::from_two_pos(a, c).intersects(clip).then(|| {
                egui::Shape::line_segment([a, c], st(line_width(l.weight, xf.z), tint(INK)))
            })
        })
        .collect();
    painter.extend(shapes);
    let r = xf.rect(bounds(b));
    match &b.source {
        BoxSource::Text { text, height_pt } => {
            let px = (*height_pt as f32 * xf.z / 72.0).max(1.0);
            if px >= 3.0 {
                painter.with_clip_rect(r).text(
                    r.left_top() + Vec2::splat(2.0),
                    Align2::LEFT_TOP,
                    text,
                    FontId::proportional(px),
                    tint(INK),
                );
            }
        }
        BoxSource::Schedule { kind } => {
            painter.text(
                r.center(),
                Align2::CENTER_CENTER,
                format!("{kind:?} schedule"),
                FontId::proportional((xf.z * 0.35).clamp(8.0, 18.0)),
                tint(Color32::GRAY),
            );
        }
        BoxSource::Image { path } => {
            painter.text(
                r.center(),
                Align2::CENTER_CENTER,
                path,
                FontId::proportional(11.0),
                tint(Color32::GRAY),
            );
        }
        BoxSource::ImageData { .. } => {
            painter.text(
                r.center(),
                Align2::CENTER_CENTER,
                "image",
                FontId::proportional(11.0),
                tint(Color32::GRAY),
            );
        }
        _ => {}
    }
    if let Some(label) = &b.label {
        let pos = xf.pt(bounds(b)[0], bounds(b)[1]) + Vec2::new(0.0, 4.0);
        let size = (10.0 * xf.z / 20.0).clamp(7.0, 22.0);
        painter.text(
            pos,
            Align2::LEFT_TOP,
            label,
            FontId::proportional(size),
            tint(INK),
        );
        if is_scaled(&b.source) {
            painter.text(
                pos + Vec2::new(0.0, size + 2.0),
                Align2::LEFT_TOP,
                format!("SCALE: {}", b.scale.label()),
                FontId::proportional((size * 0.7).max(6.0)),
                tint(Color32::GRAY),
            );
        }
    }
}

fn paint_cad(painter: &egui::Painter, xf: &Xf, o: &CadObject, ctx: &MacroContext) {
    let stroke = st(1.0, INK);
    let p = |pt: Point| xf.pt(pt.x, pt.y);
    match &o.item {
        CadItem::Line { a, b } => {
            painter.line_segment([p(*a), p(*b)], stroke);
        }
        CadItem::Polyline { points, closed } => {
            let pts: Vec<Pos2> = points.iter().map(|q| p(*q)).collect();
            if *closed {
                painter.add(egui::Shape::closed_line(pts, stroke));
            } else {
                painter.add(egui::Shape::line(pts, stroke));
            }
        }
        CadItem::Circle { center, radius } => {
            painter.circle_stroke(p(*center), *radius as f32 * xf.z, stroke);
        }
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let sweep = (end_angle - start_angle).rem_euclid(TAU);
            let n = 32;
            let pts: Vec<Pos2> = (0..=n)
                .map(|i| {
                    let a = start_angle + sweep * f64::from(i) / f64::from(n);
                    p(Point::new(
                        center.x + radius * a.cos(),
                        center.y + radius * a.sin(),
                    ))
                })
                .collect();
            painter.add(egui::Shape::line(pts, stroke));
        }
        CadItem::Text {
            pos, text, height, ..
        } => {
            let px = (*height as f32 * xf.z).max(1.0);
            if px >= 3.0 {
                painter.text(
                    p(*pos),
                    Align2::LEFT_BOTTOM,
                    ctx.expand(text),
                    FontId::proportional(px),
                    INK,
                );
            }
        }
    }
}

fn paint_field(painter: &egui::Painter, r: Rect, label: &str, value: &str) {
    painter.rect_stroke(r, 0.0, st(0.75, INK), StrokeKind::Inside);
    let small = (r.height() * 0.14).clamp(5.0, 9.0);
    painter.text(
        r.left_top() + Vec2::new(3.0, 2.0),
        Align2::LEFT_TOP,
        label,
        FontId::proportional(small),
        Color32::GRAY,
    );
    let big = (r.height() * 0.28).clamp(7.0, 15.0);
    painter.with_clip_rect(r).text(
        r.left_bottom() + Vec2::new(4.0, -3.0),
        Align2::LEFT_BOTTOM,
        value,
        FontId::proportional(big),
        INK,
    );
}

/// The border and title block of a page, macros expanded for that sheet.
fn paint_title_block(painter: &egui::Painter, xf: &Xf, layout: &Layout, ctx: &MacroContext) {
    let (w, h) = layout.sheet.inches();
    let m = layout.margins_in;
    let edge =
        (f32::from(u16::try_from(layout.edge_line_weight).unwrap_or(18)) / 100.0 * xf.z * 0.04)
            .clamp(1.0, 3.0);
    painter.rect_stroke(
        xf.rect([m, m, w - m, h - m]),
        0.0,
        st(edge, INK),
        StrokeKind::Middle,
    );
    let fields = layout.title_block.expand_macros(ctx);
    let (lo, hi) = layout.drawing_area();
    match &layout.title_block.style {
        TitleBlockStyle::RightStrip => {
            let x0 = hi.x;
            let rows = layout.title_block.revision_rows;
            let rev_h = if rows > 0 {
                0.45 + 0.25 * rows as f64
            } else {
                0.0
            };
            let avail = (h - 2.0 * m - rev_h).max(0.0);
            let each = avail / fields.len().max(1) as f64;
            let mut top = h - m;
            for (label, value) in &fields {
                paint_field(painter, xf.rect([x0, top - each, w - m, top]), label, value);
                top -= each;
            }
            if rows > 0 {
                let table = xf.rect([x0, m, w - m, m + rev_h]);
                painter.rect_stroke(table, 0.0, st(0.75, INK), StrokeKind::Inside);
                painter.text(
                    table.left_top() + Vec2::new(3.0, 2.0),
                    Align2::LEFT_TOP,
                    "REVISIONS",
                    FontId::proportional(8.0),
                    Color32::GRAY,
                );
                for (i, (n, d, t)) in ctx.revisions.iter().rev().take(rows).enumerate() {
                    painter.text(
                        xf.pt(x0 + 0.05, m + rev_h - 0.45 - 0.25 * i as f64),
                        Align2::LEFT_TOP,
                        format!("{n}  {d}  {t}"),
                        FontId::proportional((xf.z * 0.13).clamp(6.0, 11.0)),
                        INK,
                    );
                }
            }
        }
        TitleBlockStyle::BottomStrip => {
            let each = (w - 2.0 * m) / fields.len().max(1) as f64;
            for (i, (label, value)) in fields.iter().enumerate() {
                let x = m + each * i as f64;
                paint_field(painter, xf.rect([x, m, x + each, lo.y]), label, value);
            }
        }
        TitleBlockStyle::Custom(items) => {
            for o in items {
                paint_cad(painter, xf, o, ctx);
            }
        }
    }
}

impl LayoutView {
    /// Draws page `index`: background, template pages, boxes, page CAD, border
    /// and title block.
    fn paint_page(&mut self, painter: &egui::Painter, xf: &Xf, project: &Project, index: usize) {
        let Some(layout) = self.layout.take() else {
            return;
        };
        self.paint_layout_page(&layout, painter, xf, project, index);
        self.layout = Some(layout);
    }

    fn paint_layout_page(
        &mut self,
        layout: &Layout,
        painter: &egui::Painter,
        xf: &Xf,
        project: &Project,
        index: usize,
    ) {
        let (w, h) = layout.sheet.inches();
        let sheet = xf.rect([0.0, 0.0, w, h]);
        painter.rect_filled(
            sheet.translate(Vec2::new(3.0, 3.0)),
            0.0,
            Color32::from_black_alpha(90),
        );
        let bg = if layout.page_background {
            let (r, g, b) = CHIEF_SHEET_BACKGROUND;
            Color32::from_rgb(r, g, b)
        } else {
            Color32::WHITE
        };
        painter.rect_filled(sheet, 0.0, bg);
        let Some(page) = layout.pages.get(index) else {
            return;
        };
        let mut ctx = macro_context(project);
        ctx.sheet_number = page.sheet_number();
        ctx.sheet_title = page.title.clone();
        ctx.scale = page_scale_label(page);
        ctx.page_count = layout.content_pages().len();
        let sig = self.sig;
        let mut cache = std::mem::take(&mut self.cache);
        let mut rcx = None;
        let painter = painter.with_clip_rect(painter.clip_rect().intersect(sheet.expand(4.0)));
        for t in layout.template_pages() {
            if !std::ptr::eq(t, page) {
                for b in &t.boxes {
                    let lines = box_lines(&mut cache, &mut rcx, project, sig, b);
                    paint_box(&painter, xf, b, &lines, true);
                }
                for o in &t.cad {
                    paint_cad(&painter, xf, o, &ctx);
                }
            }
        }
        for b in &page.boxes {
            let lines = box_lines(&mut cache, &mut rcx, project, sig, b);
            paint_box(&painter, xf, b, &lines, false);
        }
        for o in &page.cad {
            paint_cad(&painter, xf, o, &ctx);
        }
        self.cache = cache;
        paint_title_block(&painter, xf, layout, &ctx);
        if page.template_page {
            painter.text(
                sheet.center(),
                Align2::CENTER_CENTER,
                "PAGE TEMPLATE",
                FontId::proportional((xf.z * 1.2).clamp(14.0, 80.0)),
                Color32::from_black_alpha(28),
            );
        }
    }

    fn paint_selection(&self, painter: &egui::Painter, xf: &Xf) {
        let Some(b) = self.selected_box() else { return };
        let r = bounds(b);
        painter.rect_stroke(xf.rect(r), 0.0, st(1.5, SELECT_BLUE), StrokeKind::Outside);
        for h in Handle::ALL {
            let (x, y) = h.position(r);
            let c = xf.pt(x, y);
            let hr = Rect::from_center_size(c, Vec2::splat(HANDLE_PX));
            painter.rect_filled(hr, 0.0, Color32::WHITE);
            painter.rect_stroke(hr, 0.0, st(1.0, SELECT_BLUE), StrokeKind::Inside);
        }
    }
}

// ------------------------------------------------------------------ view --

/// Draws the layout view in the main area.
pub fn show_central(ctx: &egui::Context, ui: &mut Ui, cx: &mut EditorContext) {
    let mut view = with_view(std::mem::take);
    view.show(ctx, ui, cx);
    with_view(|slot| *slot = view);
}

impl LayoutView {
    fn show(&mut self, ctx: &egui::Context, ui: &mut Ui, cx: &mut EditorContext) {
        self.sync(&cx.project);
        if self.layout.is_none() {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.heading("This plan has no layout yet");
                if ui.button("New Layout").clicked() {
                    self.ensure(cx);
                }
                if ui.button("Back to the plan").clicked() {
                    self.active = false;
                }
            });
            return;
        }
        let mut cmds: Vec<LayoutCommand> = Vec::new();
        egui::TopBottomPanel::top("layout_tools").show_inside(ui, |ui| {
            self.toolbar(ui, cx, &mut cmds);
        });
        egui::TopBottomPanel::bottom("layout_tabs").show_inside(ui, |ui| {
            self.page_tabs(ui, cx, &mut cmds);
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(ui, |ui| self.sheet_view(ctx, ui, cx));
        if !cmds.is_empty() {
            ctx.request_repaint();
        }
        for c in cmds {
            self.run(cx, c, None);
        }
    }

    fn toolbar(&mut self, ui: &mut Ui, cx: &EditorContext, out: &mut Vec<LayoutCommand>) {
        use LayoutCommand as C;
        let _ = cx;
        egui::ScrollArea::horizontal()
            .id_salt("layout_toolbar")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut btn = |ui: &mut Ui, text: &str, tip: &str, c: C, on: bool| {
                        if ui
                            .add_enabled(on, egui::Button::new(text))
                            .on_hover_text(tip)
                            .on_disabled_hover_text(tip)
                            .clicked()
                        {
                            out.push(c);
                        }
                    };
                    btn(ui, "Plan", "Back to the floor plan", C::ShowPlan, true);
                    ui.separator();
                    btn(
                        ui,
                        "Send to Layout",
                        "Send the current plan view",
                        C::SendToLayout,
                        true,
                    );
                    btn(
                        ui,
                        "Box Specification",
                        "Layout Box Specification (double-click a box)",
                        C::BoxSpecification,
                        self.selected.is_some(),
                    );
                    btn(
                        ui,
                        "Delete Box",
                        "Delete the selected layout box",
                        C::DeleteBox,
                        self.selected.is_some(),
                    );
                    ui.separator();
                    btn(
                        ui,
                        "Page Before",
                        "Insert Page Before",
                        C::InsertPageBefore,
                        true,
                    );
                    btn(
                        ui,
                        "Page After",
                        "Insert Page After",
                        C::InsertPageAfter,
                        true,
                    );
                    btn(ui, "Duplicate", "Duplicate Page", C::DuplicatePage, true);
                    btn(ui, "Delete Page", "Delete Page", C::DeletePage, true);
                    btn(
                        ui,
                        "\u{25C0}",
                        "Exchange With Previous Page",
                        C::ExchangeWithPrevious,
                        self.page > 0,
                    );
                    btn(
                        ui,
                        "\u{25B6}",
                        "Exchange With Next Page",
                        C::ExchangeWithNext,
                        self.layout
                            .as_ref()
                            .is_some_and(|l| self.page + 1 < l.pages.len()),
                    );
                    ui.separator();
                    btn(ui, "Page Table", "Layout Page Table", C::PageTable, true);
                    btn(
                        ui,
                        "Update Views",
                        "Update Layout Views",
                        C::UpdateViews,
                        true,
                    );
                    btn(ui, "Page Setup", "Page Setup", C::PageSetup, true);
                    btn(
                        ui,
                        "Project Info",
                        "Project Information",
                        C::ProjectInfo,
                        true,
                    );
                    ui.separator();
                    btn(
                        ui,
                        "Undo",
                        "Undo the last layout edit",
                        C::Undo,
                        !self.past.is_empty(),
                    );
                    btn(ui, "Redo", "Redo", C::Redo, !self.future.is_empty());
                    btn(ui, "Fit", "Fit the page in the window", C::FitPage, true);
                    btn(ui, "Print", "Print Layout", C::Print, true);
                    ui.label(format!("{:.0}%", self.zoom / 0.96));
                });
            });
    }

    fn page_tabs(&mut self, ui: &mut Ui, cx: &mut EditorContext, out: &mut Vec<LayoutCommand>) {
        use LayoutCommand as C;
        let titles: Vec<(String, bool)> = self
            .layout
            .as_ref()
            .map(|l| {
                l.pages
                    .iter()
                    .map(|p| {
                        let name = if p.template_page {
                            format!("{} (template)", p.title)
                        } else {
                            format!("A-{}  {}", p.number, p.title)
                        };
                        (name, p.template_page)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut rename: Option<(usize, String)> = None;
        egui::ScrollArea::horizontal()
            .id_salt("layout_page_tabs")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (i, (name, template)) in titles.iter().enumerate() {
                        if let Some((ri, text)) = self.renaming.as_mut().filter(|r| r.0 == i) {
                            let _ = ri;
                            let r = ui.add(egui::TextEdit::singleline(text).desired_width(140.0));
                            r.request_focus();
                            if r.lost_focus() {
                                rename = Some((i, text.clone()));
                            }
                            continue;
                        }
                        let mut label = egui::RichText::new(name);
                        if *template {
                            label = label.italics();
                        }
                        let r = ui.selectable_label(i == self.page, label);
                        if r.clicked() {
                            out.push(C::GoToPage(i));
                        }
                        if r.double_clicked() {
                            let title = self
                                .layout
                                .as_ref()
                                .and_then(|l| l.pages.get(i))
                                .map(|p| p.title.clone())
                                .unwrap_or_default();
                            self.renaming = Some((i, title));
                        }
                        r.context_menu(|ui| {
                            for (text, c) in [
                                ("Insert Page Before", C::InsertPageBefore),
                                ("Insert Page After", C::InsertPageAfter),
                                ("Duplicate Page", C::DuplicatePage),
                                ("Exchange With Previous Page", C::ExchangeWithPrevious),
                                ("Exchange With Next Page", C::ExchangeWithNext),
                                ("Delete Page", C::DeletePage),
                            ] {
                                if ui.button(text).clicked() {
                                    out.push(C::GoToPage(i));
                                    out.push(c);
                                    ui.close_menu();
                                }
                            }
                        });
                    }
                    if ui.button("+").on_hover_text("Add a page").clicked() {
                        let last = titles.len().saturating_sub(1);
                        out.push(C::GoToPage(last));
                        out.push(C::InsertPageAfter);
                    }
                });
            });
        if let Some((i, title)) = rename {
            self.renaming = None;
            if self.rename_page(&mut cx.project, i, &title) {
                cx.status = "Renamed the page".into();
            }
        }
    }

    fn sheet_view(&mut self, ctx: &egui::Context, ui: &mut Ui, cx: &mut EditorContext) {
        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let Some(layout) = &self.layout else { return };
        let (_, sheet_h) = layout.sheet.inches();
        if self.fit_pending {
            self.fit(rect);
        }
        let pointer = resp.hover_pos();
        // Zoom: wheel / pinch about the pointer.
        if let Some(pos) = pointer {
            let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = (scroll * 0.005).exp() * pinch;
            if (factor - 1.0).abs() > 1e-4 {
                self.zoom_about(rect, pos, factor);
            }
        }
        let xf = Xf {
            origin: rect.min + self.offset,
            z: self.zoom,
            h: sheet_h,
        };
        self.last_xf = Some(xf);
        if self.drag.is_none() {
            self.sig = project_sig(&cx.project);
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, SURROUND);
        let page = self.page;
        self.paint_page(&painter, &xf, &cx.project, page);
        self.paint_selection(&painter, &xf);
        self.interact(ctx, ui, cx, &resp, &xf, &painter);
    }

    fn interact(
        &mut self,
        ctx: &egui::Context,
        ui: &mut Ui,
        cx: &mut EditorContext,
        resp: &egui::Response,
        xf: &Xf,
        painter: &egui::Painter,
    ) {
        let snapping = !ui.input(|i| i.modifiers.alt);
        let tol = f64::from(HANDLE_PX) / f64::from(xf.z);

        // Cursor feedback and the placement ghost.
        if let Some(pos) = resp.hover_pos() {
            if let Some(b) = self.selected_box() {
                let (x, y) = xf.paper(pos);
                if let Some(h) = handle_at(bounds(b), x, y, tol) {
                    ctx.set_cursor_icon(h.cursor());
                }
            }
            if self.placing.is_some() {
                ctx.set_cursor_icon(CursorIcon::Crosshair);
            }
        }

        // Starting a drag.
        if resp.drag_started() {
            let origin = ui.input(|i| i.pointer.press_origin());
            let primary = ui.input(|i| i.pointer.primary_down());
            if let (Some(pos), true) = (origin, primary && self.placing.is_none()) {
                let (x, y) = xf.paper(pos);
                let handle = self
                    .selected_box()
                    .and_then(|b| handle_at(bounds(b), x, y, tol).map(|h| (b.id, bounds(b), h)));
                if let (Some((id, orig, h)), Some(before)) = (handle, self.layout.clone()) {
                    self.drag = Some(Drag::Resize {
                        id,
                        handle: h,
                        start: (x, y),
                        orig,
                        before: Box::new(before),
                    });
                } else if let (Some(id), Some(before)) = (self.hit(xf, pos), self.layout.clone()) {
                    self.selected = Some(id);
                    let orig = self
                        .current_page()
                        .and_then(|p| p.boxes.iter().find(|b| b.id == id))
                        .map_or([0.0; 4], bounds);
                    self.drag = Some(Drag::Move {
                        id,
                        start: (x, y),
                        orig,
                        before: Box::new(before),
                    });
                } else {
                    self.drag = Some(Drag::Pan);
                }
            } else {
                self.drag = Some(Drag::Pan);
            }
        }
        // Dragging.
        if resp.dragged() {
            let cur = ui.input(|i| i.pointer.interact_pos());
            match (self.drag.clone(), cur) {
                (Some(Drag::Pan), _) => self.offset += ui.input(|i| i.pointer.delta()),
                (
                    Some(Drag::Move {
                        id, start, orig, ..
                    }),
                    Some(pos),
                ) => {
                    let (x, y) = xf.paper(pos);
                    self.live_bounds(
                        &mut cx.project,
                        id,
                        moved(orig, x - start.0, y - start.1, snapping),
                    );
                }
                (
                    Some(Drag::Resize {
                        id,
                        handle,
                        start,
                        orig,
                        ..
                    }),
                    Some(pos),
                ) => {
                    let (x, y) = xf.paper(pos);
                    self.live_bounds(
                        &mut cx.project,
                        id,
                        resized(orig, handle, x - start.0, y - start.1, snapping),
                    );
                }
                _ => {}
            }
        }
        if resp.drag_stopped() {
            match self.drag.take() {
                Some(Drag::Move { before, .. }) => {
                    self.finish_drag(&mut cx.project, *before, "Move Layout Box")
                }
                Some(Drag::Resize { before, .. }) => {
                    self.finish_drag(&mut cx.project, *before, "Resize Layout Box");
                }
                _ => {}
            }
            if let Some(l) = self.undo_label().map(str::to_string) {
                cx.status = l;
            }
        }
        // Clicks.
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let (x, y) = xf.paper(pos);
                if self.placing.is_some() {
                    self.place_at(cx, x, y);
                } else {
                    self.selected = self.hit(xf, pos);
                }
            }
        }
        if resp.double_clicked() {
            if let Some(b) = resp.interact_pointer_pos().and_then(|p| self.hit(xf, p)) {
                self.selected = Some(b);
                if let Some(bx) = self.selected_box().cloned() {
                    self.open_spec(&cx.project, &bx);
                }
            }
        }
        // Placement ghost.
        if let (Some(p), Some(pos)) = (&self.placing, resp.hover_pos()) {
            let (w, h) = p.size;
            let (x, y) = xf.paper(pos);
            let r = xf.rect([x - w / 2.0, y - h / 2.0, x + w / 2.0, y + h / 2.0]);
            painter.rect_stroke(r, 0.0, st(1.5, SELECT_BLUE), StrokeKind::Middle);
        }
        // Keys.
        if !ctx.wants_keyboard_input() && !self.dialogs.any() {
            let (del, esc, nudge) = ui.input(|i| {
                let big = if i.modifiers.shift { 4.0 } else { 1.0 } * SNAP_IN;
                let mut n = (0.0, 0.0);
                if i.key_pressed(egui::Key::ArrowLeft) {
                    n.0 -= big;
                }
                if i.key_pressed(egui::Key::ArrowRight) {
                    n.0 += big;
                }
                if i.key_pressed(egui::Key::ArrowUp) {
                    n.1 += big;
                }
                if i.key_pressed(egui::Key::ArrowDown) {
                    n.1 -= big;
                }
                (
                    i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
                    i.key_pressed(egui::Key::Escape),
                    n,
                )
            });
            if del && self.delete_selected(&mut cx.project) {
                cx.status = "Deleted the layout box".into();
            }
            if esc {
                self.placing = None;
                self.selected = None;
            }
            if nudge != (0.0, 0.0) {
                if let Some(b) = self.selected_box() {
                    let r = moved(bounds(b), nudge.0, nudge.1, false);
                    let id = b.id;
                    self.set_box_bounds(&mut cx.project, id, r, "Move Layout Box");
                }
            }
        }
    }

    /// The topmost box under the screen point.
    fn hit(&self, xf: &Xf, pos: Pos2) -> Option<Id> {
        let (x, y) = xf.paper(pos);
        self.current_page().and_then(|p| box_at(p, x, y))
    }

    /// Moves a box without a history step (mid-drag); the drag's start
    /// snapshot becomes the step when the mouse is released.
    fn live_bounds(&mut self, project: &mut Project, id: Id, r: [f64; 4]) {
        if let Some(l) = &mut self.layout {
            for p in &mut l.pages {
                if let Some(b) = p.boxes.iter_mut().find(|b| b.id == id) {
                    set_bounds(b, r);
                }
            }
        }
        self.write_back(project);
    }

    fn finish_drag(&mut self, project: &mut Project, before: Layout, label: &str) {
        let changed = self.layout.as_ref().is_some_and(|l| *l != before);
        if changed {
            self.past.push((label.to_string(), before));
            if self.past.len() > HISTORY_CAP {
                self.past.remove(0);
            }
            self.future.clear();
            self.write_back(project);
        }
    }
}

// ----------------------------------------------------------------- tests --

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::WallKind;
    use plan_docs::SheetSize;

    fn project() -> Project {
        let mut p = Project::new("Smith Residence");
        for (a, b) in [
            ((0.0, 0.0), (480.0, 0.0)),
            ((480.0, 0.0), (480.0, 360.0)),
            ((480.0, 360.0), (0.0, 360.0)),
            ((0.0, 360.0), (0.0, 0.0)),
        ] {
            p.add_wall(
                0,
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                6.0,
                96.0,
                WallKind::Exterior,
            );
        }
        p
    }

    fn view_with_layout() -> (LayoutView, Project) {
        let mut p = project();
        let mut v = LayoutView::default();
        assert!(v.create(&mut p, None));
        (v, p)
    }

    fn plan_spec(floor: usize) -> SendSpec {
        SendSpec {
            source: SendSource::Plan {
                floor,
                layer_set: "Default Set".into(),
            },
            page: PageChoice::Existing(1),
            scale: None,
            placement: Placement::FirstFree,
        }
    }

    #[test]
    fn layout_round_trips_through_project_json() {
        let (mut v, mut p) = view_with_layout();
        v.send(&mut p, &plan_spec(0), None).unwrap();
        let json = p.to_json().unwrap();
        let back = Project::from_json(&json).unwrap();
        assert_eq!(load(&back).unwrap(), *v.layout().unwrap());
        assert!(load(&Project::new("x")).is_none());
        // A fresh view reads the opened project's layout.
        let mut v2 = LayoutView::default();
        v2.sync(&back);
        assert_eq!(v2.layout(), v.layout());
    }

    #[test]
    fn new_layout_has_a_template_page_and_page_one_with_the_title_block() {
        let (v, mut p) = view_with_layout();
        let l = v.layout().unwrap();
        assert_eq!(l.pages.len(), 2);
        assert!(l.pages[0].template_page && l.pages[0].number == 0);
        assert_eq!(l.pages[1].number, 1);
        assert_eq!(
            l.title_block,
            plan_layout::TitleBlockTemplate::from_daniel_18x24()
        );
        assert_eq!(l.sheet, SheetSize::ArchC);
        assert_eq!(v.page, 1, "the first printed page is showing");
        // The title block fields expand with the Project Information.
        p.info.client_name = "J. Smith".into();
        let ctx = macro_context(&p);
        let fields = l.title_block.expand_macros(&ctx);
        assert!(fields.contains(&("PROJECT".to_string(), "Smith Residence".to_string())));
        assert!(fields.contains(&("CLIENT".to_string(), "J. Smith".to_string())));
        // Today's date is filled in for the DATE field.
        assert_eq!(p.info.date.len(), 10);
        // A second New Layout opens the same one.
        let mut v2 = LayoutView::default();
        assert!(!v2.create(&mut p.clone(), None));
    }

    #[test]
    fn send_to_layout_adds_a_box_at_the_auto_scale() {
        let (mut v, mut p) = view_with_layout();
        let id = v.send(&mut p, &plan_spec(0), None).unwrap();
        let l = v.layout().unwrap();
        let b = l.pages[1].boxes.iter().find(|b| b.id == id).unwrap();
        // A 40' x 30' plan on 18x24: 1/4" is the auto ceiling and it fits.
        assert_eq!(b.scale, Scale::QuarterInch);
        assert_eq!(b.label.as_deref(), Some("1ST FLOOR PLAN"));
        assert!(matches!(b.source, BoxSource::PlanView { floor: 0, .. }));
        assert_eq!(v.selected, Some(id));
        assert_eq!(v.undo_label(), Some("Send to Layout"));
        // A chosen scale is used as is.
        let spec = SendSpec {
            scale: Some(Scale::EighthInch),
            ..plan_spec(0)
        };
        let id2 = v.send(&mut p, &spec, None).unwrap();
        let l = v.layout().unwrap();
        assert_eq!(
            l.pages[1].boxes.iter().find(|b| b.id == id2).unwrap().scale,
            Scale::EighthInch
        );
        // Undo takes it away again.
        assert_eq!(v.undo(&mut p).as_deref(), Some("Send to Layout"));
        assert_eq!(v.layout().unwrap().pages[1].boxes.len(), 1);
    }

    #[test]
    fn send_centered_and_clicked_and_new_page() {
        let (mut v, mut p) = view_with_layout();
        let centered = SendSpec {
            placement: Placement::Centered,
            ..plan_spec(0)
        };
        let id = v.send(&mut p, &centered, None).unwrap();
        let (lo, hi) = v.layout().unwrap().drawing_area();
        let b = v.layout().unwrap().pages[1]
            .boxes
            .iter()
            .find(|b| b.id == id)
            .unwrap()
            .clone();
        let r = bounds(&b);
        assert!(((r[0] + r[2]) / 2.0 - (lo.x + hi.x) / 2.0).abs() < 1e-9);
        assert!(((r[1] + r[3]) / 2.0 - (lo.y + hi.y) / 2.0).abs() < 1e-9);
        let clicked = SendSpec {
            placement: Placement::Click,
            page: PageChoice::New,
            ..plan_spec(0)
        };
        let id2 = v.send(&mut p, &clicked, Some((10.0, 8.0))).unwrap();
        let l = v.layout().unwrap();
        assert_eq!(l.pages.len(), 3, "a new page was added");
        assert_eq!(v.page, 2);
        let b2 = l.pages[2].boxes.iter().find(|b| b.id == id2).unwrap();
        let r2 = bounds(b2);
        assert!(((r2[0] + r2[2]) / 2.0 - 10.0).abs() < 1e-9);
        assert!(((r2[1] + r2[3]) / 2.0 - 8.0).abs() < 1e-9);
        assert!(v.send(&mut p, &plan_spec(9), None).is_err());
    }

    #[test]
    fn box_move_and_resize_update_the_model_and_undo_restores() {
        let (mut v, mut p) = view_with_layout();
        let id = v.send(&mut p, &plan_spec(0), None).unwrap();
        let before = bounds(&v.layout().unwrap().pages[1].boxes[0]);
        let moved_to = moved(before, 1.0, 2.0, true);
        assert!(v.set_box_bounds(&mut p, id, moved_to, "Move Layout Box"));
        assert_eq!(bounds(&v.layout().unwrap().pages[1].boxes[0]), moved_to);
        // The project holds the move too.
        assert_eq!(bounds(&load(&p).unwrap().pages[1].boxes[0]), moved_to);
        let grown = resized(moved_to, Handle::NE, 1.0, 1.0, true);
        assert!(v.set_box_bounds(&mut p, id, grown, "Resize Layout Box"));
        assert!(grown[2] > moved_to[2] && grown[3] > moved_to[3]);
        assert_eq!(v.undo(&mut p).as_deref(), Some("Resize Layout Box"));
        assert_eq!(bounds(&v.layout().unwrap().pages[1].boxes[0]), moved_to);
        assert_eq!(v.undo(&mut p).as_deref(), Some("Move Layout Box"));
        assert_eq!(bounds(&v.layout().unwrap().pages[1].boxes[0]), before);
        assert_eq!(bounds(&load(&p).unwrap().pages[1].boxes[0]), before);
        assert_eq!(v.redo(&mut p).as_deref(), Some("Move Layout Box"));
        assert_eq!(bounds(&v.layout().unwrap().pages[1].boxes[0]), moved_to);
    }

    #[test]
    fn resize_handles_keep_a_minimum_size_and_hit_tests_work() {
        let r = [2.0, 2.0, 6.0, 5.0];
        assert_eq!(resized(r, Handle::E, 1.0, 9.0, false), [2.0, 2.0, 7.0, 5.0]);
        assert_eq!(resized(r, Handle::W, 10.0, 0.0, false)[0], 6.0 - MIN_BOX_IN);
        assert_eq!(
            resized(r, Handle::N, 0.0, -10.0, false)[3],
            2.0 + MIN_BOX_IN
        );
        assert_eq!(moved(r, 0.3, 0.3, true)[0], 2.3125);
        assert_eq!(handle_at(r, 6.0, 5.0, 0.1), Some(Handle::NE));
        assert_eq!(handle_at(r, 4.0, 2.0, 0.1), Some(Handle::S));
        assert_eq!(handle_at(r, 4.0, 3.5, 0.1), None);
        let (v, _) = view_with_layout();
        let page = &v.layout().unwrap().pages[1];
        assert_eq!(box_at(page, 1.0, 1.0), None);
    }

    #[test]
    fn delete_box_and_page_operations() {
        let (mut v, mut p) = view_with_layout();
        let id = v.send(&mut p, &plan_spec(0), None).unwrap();
        assert_eq!(v.selected, Some(id));
        assert!(v.delete_selected(&mut p));
        assert!(v.layout().unwrap().pages[1].boxes.is_empty());
        assert!(!v.delete_selected(&mut p));
        v.undo(&mut p);
        assert_eq!(v.layout().unwrap().pages[1].boxes.len(), 1);

        // Pages: add, rename, duplicate, exchange, delete.
        assert!(v.add_page(&mut p, false));
        let l = v.layout().unwrap();
        assert_eq!(l.pages.len(), 3);
        assert_eq!(v.page, 2);
        assert_eq!(l.pages[2].number, 2);
        assert!(v.rename_page(&mut p, 2, " Elevations "));
        assert_eq!(v.layout().unwrap().pages[2].title, "Elevations");
        assert!(v.add_page(&mut p, true));
        let l = v.layout().unwrap();
        assert_eq!(l.pages.len(), 4);
        // Inserting before renumbers: the old page 2 became page 3.
        assert_eq!(
            l.pages.iter().map(|x| x.number).collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
        assert_eq!(l.pages[3].title, "Elevations");
        v.set_page(1);
        assert!(v.duplicate_current_page(&mut p));
        let l = v.layout().unwrap();
        assert_eq!(l.pages.len(), 5);
        assert_eq!(l.pages[2].boxes.len(), 1);
        assert_ne!(
            l.pages[2].boxes[0].id, l.pages[1].boxes[0].id,
            "new box ids"
        );
        assert!(v.exchange_current_page(&mut p, true));
        assert_eq!(v.page, 3);
        assert!(v.exchange_current_page(&mut p, false));
        assert_eq!(v.page, 2);
        assert!(v.delete_current_page(&mut p).is_ok());
        assert_eq!(v.layout().unwrap().pages.len(), 4);
        // The template page can go, the last printed page cannot.
        v.set_page(0);
        assert!(v.delete_current_page(&mut p).is_ok());
        while v.layout().unwrap().content_pages().len() > 1 {
            v.set_page(0);
            assert!(v.delete_current_page(&mut p).is_ok());
        }
        v.set_page(0);
        assert_eq!(
            v.delete_current_page(&mut p),
            Err("A layout keeps at least one page")
        );
        // Everything was undoable step by step.
        assert!(v.undo(&mut p).is_some());
    }

    #[test]
    fn page_setup_changes_the_sheet() {
        let (mut v, mut p) = view_with_layout();
        let mut s = v.page_setup().unwrap();
        assert_eq!(s.sheet, SheetSize::ArchC);
        s.sheet = SheetSize::Tabloid;
        s.margins_in = 0.25;
        s.page_background = false;
        s.edge_line_weight = 35;
        assert!(v.apply_page_setup(&mut p, &s));
        let l = load(&p).unwrap();
        assert_eq!(l.sheet, SheetSize::Tabloid);
        assert_eq!(l.margins_in, 0.25);
        assert!(!l.page_background);
        assert_eq!(l.edge_line_weight, 35);
        assert!(!v.apply_page_setup(&mut p, &s), "no change, no step");
        v.undo(&mut p);
        assert_eq!(v.layout().unwrap().sheet, SheetSize::ArchC);
    }

    #[test]
    fn title_block_macros_read_the_project_information() {
        let (mut v, mut p) = view_with_layout();
        // A new layout fills in today's date when Project Information has none.
        assert_eq!(p.info.date.len(), 10);
        p.info.client_name = "J. Smith".into();
        p.info.client_address = vec!["1 Main St".into(), "Atlanta".into()];
        p.info.project_number = "26-014".into();
        p.info.revision = "B".into();
        p.info.date = "2026-10-08".into();
        p.info.drawn_by = "DAD".into();
        p.info.revisions = vec![("1".into(), "2026-10-08".into(), "Issued".into())];
        let ctx = macro_context(&p);
        assert_eq!(
            ctx.expand(
                "%project.name%|%client%|%address%|%project.number%|%revision%|%designer%|%date%"
            ),
            "Smith Residence|J. Smith|1 Main St, Atlanta|26-014|B|DAD|2026-10-08"
        );
        assert_eq!(ctx.revisions.len(), 1);
        // The layout and the information live side by side in the project.
        v.add_page(&mut p, false);
        assert_eq!(p.info.project_number, "26-014");
        assert!(load(&p).is_some());
        let back = Project::from_json(&p.to_json().unwrap()).unwrap();
        assert_eq!(macro_context(&back), ctx);
    }

    #[test]
    fn box_specification_edits_and_moves_a_box() {
        let (mut v, mut p) = view_with_layout();
        let id = v.send(&mut p, &plan_spec(0), None).unwrap();
        v.add_page(&mut p, false);
        let mut b = v.layout().unwrap().pages[1].boxes[0].clone();
        b.scale = Scale::EighthInch;
        b.border = false;
        b.label = Some("MAIN LEVEL".into());
        let spec = BoxSpec {
            layout_box: b.clone(),
            page: 2,
        };
        assert!(v.apply_spec(&mut p, &spec));
        let l = v.layout().unwrap();
        assert!(l.pages[1].boxes.is_empty());
        let moved = l.pages[2].boxes.iter().find(|x| x.id == id).unwrap();
        assert_eq!(moved.scale, Scale::EighthInch);
        assert!(!moved.border);
        assert_eq!(moved.label.as_deref(), Some("MAIN LEVEL"));
        // A box that no longer exists is refused.
        let gone = BoxSpec {
            layout_box: LayoutBox { id: 999, ..b },
            page: 1,
        };
        assert!(!v.apply_spec(&mut p, &gone));
    }

    #[test]
    fn page_table_renames_and_flags_templates() {
        let (mut v, mut p) = view_with_layout();
        let mut rows = v.page_rows();
        assert_eq!(rows.len(), 2);
        rows[1].title = "Main Level".into();
        assert!(v.apply_page_table(&mut p, &rows));
        assert_eq!(v.layout().unwrap().pages[1].title, "Main Level");
        assert!(!v.apply_page_table(&mut p, &rows));
        assert_eq!(page_list(&p)[1].1, "A-1  Main Level");
        assert!(page_list(&p)[0].2, "page 0 is the template");
    }

    #[test]
    fn print_renders_the_printed_pages_only_and_ranges() {
        let (mut v, mut p) = view_with_layout();
        v.send(&mut p, &plan_spec(0), None).unwrap();
        v.add_page(&mut p, false);
        v.add_page(&mut p, false);
        let layout = v.layout().unwrap().clone();
        assert_eq!(layout.content_pages().len(), 3);
        let count = |bytes: &[u8]| {
            String::from_utf8_lossy(bytes)
                .matches("/Type /Page")
                .count()
        };
        let all = print_bytes(&layout, &p, None);
        assert!(all.starts_with(b"%PDF"));
        assert_eq!(count(&all), 3, "the template page is not printed");
        let range = print_bytes(&layout, &p, Some((2, 3)));
        assert_eq!(count(&range), 2);
        let one = print_bytes(&layout, &p, Some((1, 1)));
        assert_eq!(count(&one), 1);
        assert_eq!(layout_pdf(&p).map(|b| count(&b)), Some(3));
    }

    #[test]
    fn external_changes_reload_the_layout_and_drop_history() {
        let (mut v, mut p) = view_with_layout();
        v.send(&mut p, &plan_spec(0), None).unwrap();
        assert!(v.undo_label().is_some());
        let mut other = Project::new("Other");
        let mut v2 = LayoutView::default();
        v2.create(&mut other, None);
        // The project's layout is replaced (a file was opened).
        p.layout = other.layout.clone();
        v.sync(&p);
        assert_eq!(v.layout().unwrap().pages[1].boxes.len(), 0);
        assert!(v.undo_label().is_none());
        // And removed.
        p.layout = None;
        v.sync(&p);
        assert!(v.layout().is_none());
    }

    #[test]
    fn send_all_floors_makes_one_page_per_floor() {
        let mut p = project();
        p.build_new_floor(false);
        let mut v = LayoutView::default();
        v.create(&mut p, None);
        assert_eq!(v.send_all_floors(&mut p), 2);
        let l = v.layout().unwrap();
        assert_eq!(l.content_pages().len(), 2);
        assert_eq!(l.pages[1].title, "FIRST FLOOR PLAN");
        assert_eq!(l.pages[2].title, "SECOND FLOOR PLAN");
    }

    #[test]
    fn dispatch_commands_drive_the_view() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let mut v = LayoutView::default();
        v.create(&mut cx.project, None);
        v.run(&mut cx, LayoutCommand::ShowLayout, None);
        assert!(v.active);
        v.run(&mut cx, LayoutCommand::InsertPageAfter, None);
        assert_eq!(v.layout().unwrap().pages.len(), 3);
        v.run(&mut cx, LayoutCommand::Undo, None);
        assert_eq!(cx.status, "Undid Insert Page");
        v.run(&mut cx, LayoutCommand::SendToLayout, None);
        assert!(v.dialogs.send.is_some());
        v.run(&mut cx, LayoutCommand::PageSetup, None);
        v.run(&mut cx, LayoutCommand::ProjectInfo, None);
        v.run(&mut cx, LayoutCommand::PageTable, None);
        v.run(&mut cx, LayoutCommand::Print, None);
        assert!(v.dialogs.any());
        v.run(&mut cx, LayoutCommand::BoxSpecification, None);
        assert_eq!(cx.status, "Select a layout box first");
        v.run(&mut cx, LayoutCommand::ShowPlan, None);
        assert!(!v.active);
    }

    #[test]
    fn the_view_and_its_dialogs_draw_headless() {
        let ctx = egui::Context::default();
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = project();
        let mut v = LayoutView::default();
        v.create(&mut cx.project, None);
        v.send(&mut cx.project, &plan_spec(0), None).unwrap();
        v.active = true;
        for cmd in [
            LayoutCommand::SendToLayout,
            LayoutCommand::PageSetup,
            LayoutCommand::ProjectInfo,
            LayoutCommand::PageTable,
            LayoutCommand::Print,
            LayoutCommand::BoxSpecification,
        ] {
            v.run(&mut cx, cmd, None);
        }
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| v.show(ctx, ui, &mut cx));
                v.show_dialogs(ctx, &mut cx);
            });
        }
        assert!(v.zoom > MIN_ZOOM);
        // The page tabs and a template page draw too.
        v.set_page(0);
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| v.show(ctx, ui, &mut cx));
            });
        }
        // No layout: the prompt draws.
        let mut empty = LayoutView::default();
        let mut cx2 = EditorContext::new(plan_defaults::embedded());
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| empty.show(ctx, ui, &mut cx2));
        });
    }

    #[test]
    fn a_camera_view_is_sent_and_drawn_through_the_hook() {
        use plan_core::{CameraKind, CameraObject};
        let (mut v, mut p) = view_with_layout();
        let id = p.add_camera(CameraObject::new(
            CameraKind::Elevation,
            Point::new(240.0, -200.0),
            90.0,
            "Front Elevation",
            0,
        ));
        let spec = SendSpec {
            source: SendSource::Camera {
                id,
                name: "Front Elevation".into(),
            },
            ..plan_spec(0)
        };
        let box_id = v.send(&mut p, &spec, None).unwrap();
        let b = v.layout().unwrap().pages[1]
            .boxes
            .iter()
            .find(|b| b.id == box_id)
            .unwrap()
            .clone();
        assert!(matches!(b.source, BoxSource::Camera { camera_id } if camera_id == id));
        assert_eq!(b.label.as_deref(), Some("FRONT ELEVATION"));
        // The drawing cache asks the camera hook; a plain context would draw
        // only the 4 border edges.
        let mut cache = BoxCache::default();
        let mut rcx = None;
        let lines = box_lines(&mut cache, &mut rcx, &p, 1, &b);
        assert!(lines.len() > 4, "{} lines", lines.len());
        let again = box_lines(&mut cache, &mut rcx, &p, 1, &b);
        assert!(Rc::ptr_eq(&lines, &again), "cached");
        let changed = box_lines(&mut cache, &mut rcx, &p, 2, &b);
        assert!(
            !Rc::ptr_eq(&lines, &changed),
            "a new project signature redraws"
        );
        drop(rcx);
        // A camera that is gone is refused.
        let gone = SendSpec {
            source: SendSource::Camera {
                id: 999,
                name: String::new(),
            },
            ..plan_spec(0)
        };
        assert!(v.send(&mut p, &gone, None).is_err());
        // The send source picks the open 3D view's elevation camera, else the plan.
        assert!(matches!(
            v.send_source(&p, Some(id), 0),
            SendSource::Camera { .. }
        ));
        assert!(matches!(
            v.send_source(&p, None, 0),
            SendSource::Plan { .. }
        ));
    }

    // ----- pointer interaction through egui -----

    fn frame(
        ctx: &egui::Context,
        v: &mut LayoutView,
        cx: &mut EditorContext,
        events: Vec<egui::Event>,
        t: f64,
    ) {
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 900.0))),
            time: Some(t),
            events,
            ..egui::RawInput::default()
        };
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| v.show(ctx, ui, cx));
        });
    }

    fn press(pos: Pos2, down: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: down,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Drags from `from` to `to` over several frames; returns the next time.
    fn drag(
        ctx: &egui::Context,
        v: &mut LayoutView,
        cx: &mut EditorContext,
        from: Pos2,
        to: Pos2,
        mut t: f64,
    ) -> f64 {
        frame(ctx, v, cx, vec![egui::Event::PointerMoved(from)], t);
        t += 0.05;
        frame(ctx, v, cx, vec![press(from, true)], t);
        for i in 1..=6 {
            t += 0.05;
            let p = from + (to - from) * (i as f32 / 6.0);
            frame(ctx, v, cx, vec![egui::Event::PointerMoved(p)], t);
        }
        t += 0.05;
        frame(ctx, v, cx, vec![press(to, false)], t);
        t + 0.05
    }

    fn interactive() -> (egui::Context, LayoutView, EditorContext, Id) {
        let ctx = egui::Context::default();
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = project();
        let mut v = LayoutView::default();
        v.create(&mut cx.project, None);
        let id = v.send(&mut cx.project, &plan_spec(0), None).unwrap();
        v.selected = None;
        v.active = true;
        frame(&ctx, &mut v, &mut cx, vec![], 0.0);
        frame(&ctx, &mut v, &mut cx, vec![], 0.1);
        assert!(v.last_xf.is_some());
        (ctx, v, cx, id)
    }

    fn center_of(v: &LayoutView, id: Id) -> (Pos2, [f64; 4]) {
        let b = v
            .current_page()
            .unwrap()
            .boxes
            .iter()
            .find(|b| b.id == id)
            .unwrap();
        let r = bounds(b);
        let xf = v.last_xf.unwrap();
        (xf.pt((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0), r)
    }

    #[test]
    fn dragging_a_box_moves_it_and_undo_restores() {
        let (ctx, mut v, mut cx, id) = interactive();
        let (c, before) = center_of(&v, id);
        let t = drag(&ctx, &mut v, &mut cx, c, c + Vec2::new(60.0, -40.0), 1.0);
        let (_, after) = center_of(&v, id);
        assert_eq!(v.selected, Some(id), "dragging selects the box");
        assert!(
            after[0] > before[0] && after[1] > before[1],
            "{before:?} -> {after:?}"
        );
        assert!(
            ((after[2] - after[0]) - (before[2] - before[0])).abs() < 1e-9,
            "size kept"
        );
        // The project holds the move and one undo step covers the whole drag.
        assert_eq!(bounds(&load(&cx.project).unwrap().pages[1].boxes[0]), after);
        assert_eq!(v.past.len(), 2, "send + move");
        assert_eq!(v.undo(&mut cx.project).as_deref(), Some("Move Layout Box"));
        assert_eq!(bounds(&v.layout().unwrap().pages[1].boxes[0]), before);
        let _ = t;
    }

    #[test]
    fn dragging_a_handle_resizes_and_clicking_empty_space_deselects() {
        let (ctx, mut v, mut cx, id) = interactive();
        let (c, _) = center_of(&v, id);
        // Click selects.
        frame(
            &ctx,
            &mut v,
            &mut cx,
            vec![egui::Event::PointerMoved(c)],
            1.0,
        );
        frame(&ctx, &mut v, &mut cx, vec![press(c, true)], 1.05);
        frame(&ctx, &mut v, &mut cx, vec![press(c, false)], 1.1);
        assert_eq!(v.selected, Some(id));
        assert!(v.past.len() == 1, "a click is not an edit");
        // Drag the NE handle.
        let (_, before) = center_of(&v, id);
        let xf = v.last_xf.unwrap();
        let ne = xf.pt(before[2], before[3]);
        let t = drag(&ctx, &mut v, &mut cx, ne, ne + Vec2::new(-50.0, 40.0), 2.0);
        let (_, after) = center_of(&v, id);
        assert!(after[2] < before[2], "narrower: {before:?} -> {after:?}");
        assert!(after[3] < before[3], "shorter, dragged down on screen");
        assert_eq!(
            (after[0], after[1]),
            (before[0], before[1]),
            "SW corner fixed"
        );
        assert_eq!(
            v.undo(&mut cx.project).as_deref(),
            Some("Resize Layout Box")
        );
        assert_eq!(center_of(&v, id).1, before);
        // Click on bare sheet deselects.
        let empty = xf.pt(1.0, 1.0);
        frame(
            &ctx,
            &mut v,
            &mut cx,
            vec![egui::Event::PointerMoved(empty)],
            t,
        );
        frame(&ctx, &mut v, &mut cx, vec![press(empty, true)], t + 0.05);
        frame(&ctx, &mut v, &mut cx, vec![press(empty, false)], t + 0.1);
        assert_eq!(v.selected, None);
    }

    #[test]
    fn double_click_opens_the_box_specification_and_delete_removes_the_box() {
        let (ctx, mut v, mut cx, id) = interactive();
        let (c, _) = center_of(&v, id);
        frame(
            &ctx,
            &mut v,
            &mut cx,
            vec![egui::Event::PointerMoved(c)],
            1.0,
        );
        for k in 0..2 {
            let t = 1.05 + 0.1 * f64::from(k);
            frame(&ctx, &mut v, &mut cx, vec![press(c, true)], t);
            frame(&ctx, &mut v, &mut cx, vec![press(c, false)], t + 0.05);
        }
        assert!(v.dialogs.spec.is_some(), "double-click opens the spec");
        v.dialogs.spec = None;
        let key = egui::Event::Key {
            key: egui::Key::Delete,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&ctx, &mut v, &mut cx, vec![key], 2.0);
        assert!(v.current_page().unwrap().boxes.is_empty());
        assert_eq!(v.undo_label(), Some("Delete Layout Box"));
    }

    #[test]
    fn a_click_places_a_pending_send() {
        let (ctx, mut v, mut cx, _) = interactive();
        v.past.clear();
        let spec = SendSpec {
            placement: Placement::Click,
            scale: Some(Scale::EighthInch),
            ..plan_spec(0)
        };
        v.finish_send(&mut cx, spec);
        assert!(v.placing.is_some());
        let xf = v.last_xf.unwrap();
        let at = xf.pt(8.0, 9.0);
        frame(
            &ctx,
            &mut v,
            &mut cx,
            vec![egui::Event::PointerMoved(at)],
            1.0,
        );
        frame(&ctx, &mut v, &mut cx, vec![press(at, true)], 1.05);
        frame(&ctx, &mut v, &mut cx, vec![press(at, false)], 1.1);
        assert!(v.placing.is_none());
        let page = v.current_page().unwrap();
        assert_eq!(page.boxes.len(), 2);
        let r = bounds(page.boxes.last().unwrap());
        assert!((((r[0] + r[2]) / 2.0) - 8.0).abs() < 0.05, "{r:?}");
        assert!((((r[1] + r[3]) / 2.0) - 9.0).abs() < 0.05, "{r:?}");
    }
}
