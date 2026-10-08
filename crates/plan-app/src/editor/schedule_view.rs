//! Schedules placed in the plan: storage, editing, picking and drawing
//! (`plan_core::schedules`, `plan_docs::schedule_kinds`, the Schedule flyout).
//!
//! # Storage
//!
//! A floor's [`ScheduleLayer`] lives in the typed slot `Floor.schedules`;
//! [`save`] also makes sure the layer the tables are drawn on exists. Undo and
//! redo restore it with the rest of the project. Every edit is one undo step.
//!
//! # Drawing
//!
//! [`draw_schedules`] is called once from `render::draw_plan`. Each schedule
//! is a table (title row, header row, one row per object) whose rows come
//! from the plan, so it is always live: the built tables, their sizes and the
//! callout labels are kept in a cache that is dropped on every change signal
//! of the editor context (`EditorContext::cache_key`), so a frame that
//! changed nothing only draws. Text sizes are the plan
//! text style's character height in plan inches (the same scale as every
//! other annotation). The same call draws the callout labels (D01, W03, C-01
//! ...) next to doors, windows, cabinets and fixtures when a schedule of that
//! kind on the floor has Show Labels on.
//!
//! # Selection
//!
//! A schedule is `ObjectRef::Schedule(id)`: Select Objects and the Schedule
//! tool share `cx.selection`, and [`draw_schedules`] highlights the selected
//! ones.

use super::{Camera, EditorContext, ObjectRef};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Shape, Stroke, StrokeKind};
use plan_core::geometry::Point;
use plan_core::schedules::{
    ensure_layer, Schedule, ScheduleKind, ScheduleLayer, SCHEDULE_LABEL_STYLE,
};
use plan_core::{Id, Project, TextStyle};
use plan_docs::schedule_kinds::{self, Callout};
use plan_docs::Schedule as Table;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Characters narrower than this many character heights are rare; a column is
/// sized as `chars * CHAR_W * height`.
const CHAR_W: f64 = 0.56;
/// Horizontal padding on each side of a cell, in character heights.
const PAD_X: f64 = 0.45;
/// Row height in character heights.
const ROW_H: f64 = 1.55;
const TITLE_H: f64 = 2.1;
/// Text smaller than this many screen pixels is not drawn.
const MIN_TEXT_PX: f32 = 3.0;

/// Is schedule `id` placed on `floor`?
pub fn exists(floor: &plan_core::Floor, id: Id) -> bool {
    ScheduleLayer::load(floor).find(id).is_some()
}

/// The layer schedule `id` of `floor` is drawn on.
pub fn layer_of(floor: &plan_core::Floor, id: Id) -> Option<String> {
    ScheduleLayer::load(floor).find(id).map(|s| s.layer.clone())
}

/// The selected schedule, if exactly one schedule is selected.
pub fn selected(cx: &EditorContext) -> Option<Id> {
    let mut it = cx.selection.items.iter().filter_map(|o| match o {
        ObjectRef::Schedule(id) => Some(*id),
        _ => None,
    });
    let first = it.next()?;
    it.next().is_none().then_some(first)
}

/// Selects schedule `id` (and nothing else).
pub fn select(cx: &mut EditorContext, id: Id) {
    cx.selection.set(ObjectRef::Schedule(id));
}

/// Drops the schedules from the selection.
pub fn clear_selection(cx: &mut EditorContext) {
    cx.selection
        .items
        .retain(|o| !matches!(o, ObjectRef::Schedule(_)));
}

/// Is schedule `id` selected?
pub fn is_selected(cx: &EditorContext, id: Id) -> bool {
    cx.selection.contains(ObjectRef::Schedule(id))
}

// ===================================================================
// Storage and editing
// ===================================================================

/// The schedules of the active floor.
pub fn load(cx: &EditorContext) -> ScheduleLayer {
    ScheduleLayer::clone(&layer_rc(cx))
}

// ===================================================================
// Cache
// ===================================================================

/// What was built for the active floor since the last change signal.
struct FloorCache {
    key: (u64, u64),
    floor: usize,
    layer: Rc<ScheduleLayer>,
    /// The built layout of a schedule (by floor and id), with the definition
    /// it was built from.
    layouts: HashMap<(usize, Id), (Schedule, Rc<Layout>)>,
    /// Callouts per label source.
    labels: Option<Rc<LabelSources>>,
}

/// Per label source: the layer its schedule is drawn on and its callouts.
type LabelSources = Vec<(String, Vec<Callout>)>;

thread_local! {
    static CACHE: RefCell<Option<FloorCache>> = const { RefCell::new(None) };
}

/// Runs `f` on the cache of `cx`'s current state, starting it afresh when a
/// change signal or a floor switch happened since it was filled. `f` must not
/// call back into this module.
fn with_cache<R>(cx: &EditorContext, f: impl FnOnce(&mut FloorCache) -> R) -> R {
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        let key = cx.cache_key();
        if c.as_ref()
            .is_none_or(|x| x.key != key || x.floor != cx.floor)
        {
            *c = Some(FloorCache {
                key,
                floor: cx.floor,
                layer: Rc::new(ScheduleLayer::load(cx.floor())),
                layouts: HashMap::new(),
                labels: None,
            });
        }
        f(c.as_mut().expect("just filled"))
    })
}

fn layer_rc(cx: &EditorContext) -> Rc<ScheduleLayer> {
    with_cache(cx, |c| c.layer.clone())
}

/// [`layout_of`] without the copy: the cached layout when `def` is the
/// schedule it was built from, else built (and kept) now.
pub fn layout_rc(cx: &EditorContext, def: &Schedule, floor: usize) -> Rc<Layout> {
    with_cache(cx, |c| {
        if let Some((d, l)) = c.layouts.get(&(floor, def.id)) {
            if d == def {
                return l.clone();
            }
        }
        let l = Rc::new(layout(
            table_for(cx, def, floor),
            def,
            text_height(&cx.project, def),
        ));
        c.layouts.insert((floor, def.id), (def.clone(), l.clone()));
        l
    })
}

/// Stores `layer` on floor `fi` and adds the layer the tables are drawn on.
pub fn save(project: &mut Project, fi: usize, layer: &ScheduleLayer) {
    layer.store(&mut project.floors[fi]);
    if !layer.is_empty() {
        ensure_layer(&mut project.layers);
        for s in &layer.schedules {
            if project.layers.get(&s.layer).is_none() {
                project.layers.layers.push(plan_core::Layer::new(
                    s.layer.clone(),
                    [60, 60, 60],
                    18,
                ));
            }
        }
    }
}

/// Runs `edit` on floor `fi`'s schedules as one undo step named `label`.
pub fn edit_floor(
    cx: &mut EditorContext,
    fi: usize,
    label: &str,
    edit: impl FnOnce(&mut ScheduleLayer),
) {
    cx.begin_change(label);
    let mut layer = ScheduleLayer::load(&cx.project.floors[fi]);
    edit(&mut layer);
    save(&mut cx.project, fi, &layer);
    cx.mark_dirty();
}

/// Places a new schedule of `kind` with its upper-left corner at `at` on the
/// active floor. Returns its id.
pub fn add(cx: &mut EditorContext, kind: ScheduleKind, at: Point) -> Id {
    let id = cx.project.alloc_id();
    let mut s = Schedule::new(kind, at);
    s.id = id;
    let fl = cx.floor;
    edit_floor(cx, fl, &format!("Place {}", kind.title()), |l| {
        l.add(s);
    });
    id
}

/// The schedule `id` of the active floor.
pub fn find(cx: &EditorContext, id: Id) -> Option<Schedule> {
    load(cx).find(id).cloned()
}

/// Replaces schedule `id` of floor `fi` with `def` (the Schedule
/// Specification dialog's OK). Returns whether it exists.
pub fn replace(cx: &mut EditorContext, fi: usize, def: Schedule) -> bool {
    let exists = cx
        .project
        .floors
        .get(fi)
        .is_some_and(|f| ScheduleLayer::load(f).find(def.id).is_some());
    if !exists {
        return false;
    }
    let mut def = def;
    def.reconcile_columns();
    edit_floor(cx, fi, "Schedule Specification", |l| {
        if let Some(s) = l.find_mut(def.id) {
            *s = def;
        }
    });
    true
}

/// Deletes schedule `id` from the active floor.
pub fn delete(cx: &mut EditorContext, id: Id) -> bool {
    if find(cx, id).is_none() {
        return false;
    }
    let fl = cx.floor;
    edit_floor(cx, fl, "Delete Schedule", |l| {
        l.remove(id);
    });
    cx.selection.items.retain(|o| *o != ObjectRef::Schedule(id));
    true
}

/// Deletes every schedule in `ids` from the active floor as one undo step.
/// Returns how many went.
pub fn delete_ids(cx: &mut EditorContext, ids: &[Id]) -> usize {
    let n = {
        let layer = load(cx);
        ids.iter().filter(|id| layer.find(**id).is_some()).count()
    };
    if n == 0 {
        return 0;
    }
    let fl = cx.floor;
    edit_floor(cx, fl, "Delete Schedule", |l| {
        for id in ids {
            l.remove(*id);
        }
    });
    cx.selection
        .items
        .retain(|o| !matches!(o, ObjectRef::Schedule(i) if ids.contains(i)));
    n
}

/// Moves schedules by `d` (a group drag or nudge; the caller owns the undo
/// step).
pub fn translate_ids(cx: &mut EditorContext, ids: &[Id], d: Point) {
    let fl = cx.floor;
    let mut layer = ScheduleLayer::load(&cx.project.floors[fl]);
    let mut any = false;
    for id in ids {
        if let Some(s) = layer.find_mut(*id) {
            s.position = s.position + d;
            any = true;
        }
    }
    if any {
        layer.store(&mut cx.project.floors[fl]);
        // The tables and labels drawn so far were built for the old place.
        cx.mark_dirty();
    }
}

/// The screen-independent extent `(id, lower-left, upper-right)` of every
/// schedule on the active floor (for box selection).
pub fn extents(cx: &EditorContext) -> Vec<(Id, Point, Point)> {
    layer_rc(cx)
        .schedules
        .iter()
        .map(|s| {
            let (lo, hi) = layout_rc(cx, s, cx.floor).bounds(s.position);
            (s.id, lo, hi)
        })
        .collect()
}

/// Moves schedule `id` so its upper-left corner is `to`.
pub fn move_to(cx: &mut EditorContext, id: Id, to: Point) -> bool {
    if find(cx, id).is_none() {
        return false;
    }
    let fl = cx.floor;
    edit_floor(cx, fl, "Move Schedule", |l| {
        if let Some(s) = l.find_mut(id) {
            s.position = to;
        }
    });
    true
}

/// The schedule as a table of strings (what is drawn, exported and shown in
/// the schedule window). `floor` is the floor the schedule is placed on.
pub fn table_for(cx: &EditorContext, def: &Schedule, floor: usize) -> Table {
    let rooms = (floor == cx.floor).then_some((floor, cx.rooms.as_slice()));
    schedule_kinds::table(&cx.project, def, floor, rooms)
}

/// File name and CSV text of schedule `id` on the active floor.
pub fn export_csv(cx: &EditorContext, id: Id) -> Option<(String, String)> {
    let def = find(cx, id)?;
    let t = table_for(cx, &def, cx.floor);
    Some((format!("{}.csv", t.title.replace(' ', "_")), t.to_csv()))
}

// ===================================================================
// Layout
// ===================================================================

/// Where a table's cells fall, in plan inches.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub table: Table,
    /// Character height of the table text.
    pub h: f64,
    /// Width of each column.
    pub col_w: Vec<f64>,
    pub width: f64,
    pub title_h: f64,
    pub row_h: f64,
    pub height: f64,
}

fn text_w(s: &str, h: f64, bold: bool) -> f64 {
    s.chars().count() as f64 * h * if bold { CHAR_W * 1.1 } else { CHAR_W }
}

/// Sizes the table for text of character height `h` and the columns' own
/// width settings.
pub fn layout(table: Table, def: &Schedule, h: f64) -> Layout {
    let h = h.max(0.5);
    let visible: Vec<_> = def
        .visible_columns()
        .filter(|c| {
            def.kind.fields().iter().any(|f| f.id == c.field)
                || c.field.starts_with(plan_core::props::COLUMN_PREFIX)
        })
        .collect();
    let col_w: Vec<f64> = table
        .columns
        .iter()
        .enumerate()
        .map(|(i, title)| {
            let own = visible.get(i).map_or(0.0, |c| c.width);
            let content = table
                .rows
                .iter()
                .map(|r| text_w(r.get(i).map_or("", String::as_str), h, false))
                .fold(text_w(title, h, true), f64::max)
                + 2.0 * PAD_X * h;
            if own > 0.0 {
                own
            } else {
                content
            }
        })
        .collect();
    let cols_w: f64 = col_w.iter().sum();
    let width = cols_w.max(text_w(&table.title, h, true) + 2.0 * PAD_X * h);
    let title_h = TITLE_H * h;
    let row_h = ROW_H * h;
    let height = title_h + row_h * (1 + table.rows.len()) as f64;
    Layout {
        table,
        h,
        col_w,
        width,
        title_h,
        row_h,
        height,
    }
}

/// The character height of a schedule's text style.
pub fn text_height(project: &Project, def: &Schedule) -> f64 {
    project
        .text_styles
        .resolve(&def.text_style)
        .map_or(4.5, |s| s.height_in)
}

impl Layout {
    /// The table's rectangle with the upper-left corner at `top_left`:
    /// `(min, max)` in plan inches (Y up).
    pub fn bounds(&self, top_left: Point) -> (Point, Point) {
        (
            Point::new(top_left.x, top_left.y - self.height),
            Point::new(top_left.x + self.width, top_left.y),
        )
    }
}

/// The layout of schedule `def` placed on floor `floor`.
pub fn layout_of(cx: &EditorContext, def: &Schedule, floor: usize) -> Layout {
    Layout::clone(&layout_rc(cx, def, floor))
}

/// The schedule of the active floor under `p` (the topmost one when tables
/// overlap).
pub fn pick(cx: &EditorContext, p: Point) -> Option<Id> {
    let layer = layer_rc(cx);
    layer
        .schedules
        .iter()
        .rev()
        .filter(|s| cx.layers().is_visible(&s.layer))
        .find(|s| {
            let l = layout_rc(cx, s, cx.floor);
            let (lo, hi) = l.bounds(s.position);
            p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y
        })
        .map(|s| s.id)
}

// ===================================================================
// Callout labels
// ===================================================================

/// The labels to draw on the active floor: for each kind with labels, the
/// first schedule on the floor that shows them (when its layer is visible).
pub fn labels(cx: &EditorContext) -> Vec<Callout> {
    let sources = label_sources(cx);
    sources
        .iter()
        .filter(|(layer, _)| cx.layers().is_visible(layer))
        .flat_map(|(_, callouts)| callouts.iter().cloned())
        .collect()
}

/// The callouts of each label source on the active floor, whatever the
/// visibility of its layer (cached with the tables).
fn label_sources(cx: &EditorContext) -> Rc<LabelSources> {
    if let Some(l) = with_cache(cx, |c| c.labels.clone()) {
        return l;
    }
    let layer = layer_rc(cx);
    let mut out = Vec::new();
    for kind in [
        ScheduleKind::Door,
        ScheduleKind::Window,
        ScheduleKind::Cabinet,
        ScheduleKind::Fixture,
    ] {
        if let Some(def) = layer.label_source(kind) {
            out.push((
                def.layer.clone(),
                schedule_kinds::callouts(&cx.project, cx.floor, def),
            ));
        }
    }
    let out = Rc::new(out);
    with_cache(cx, |c| c.labels = Some(out.clone()));
    out
}

/// The style the callout labels are set in: "Schedule Label" when the plan
/// has it, else "Default Label Style", else the schedule table's style.
pub fn label_style(project: &Project) -> Option<&TextStyle> {
    project
        .text_styles
        .get(SCHEDULE_LABEL_STYLE)
        .or_else(|| project.text_styles.get("Default Label Style"))
        .or_else(|| {
            project
                .text_styles
                .get(plan_core::schedules::SCHEDULE_TEXT_STYLE)
        })
        .or_else(|| project.text_styles.resolve(""))
}

// ===================================================================
// Drawing
// ===================================================================

fn color_of(style: Option<&TextStyle>, fallback: Color32) -> Color32 {
    match style {
        Some(s) if s.color != [0, 0, 0] => Color32::from_rgb(s.color[0], s.color[1], s.color[2]),
        _ => fallback,
    }
}

fn draw_table(
    painter: &egui::Painter,
    cam: &Camera,
    l: &Layout,
    top_left: Point,
    ink: Color32,
    paper: Color32,
) {
    let px = cam.px_per_in as f32;
    let tl = cam.world_to_screen(top_left);
    let size = egui::vec2(l.width as f32 * px, l.height as f32 * px);
    let outer = Rect::from_min_size(tl, size);
    if !outer.intersects(cam.rect) {
        return;
    }
    let line = Stroke::new(1.0_f32, ink);
    let faint = Stroke::new(1.0_f32, ink.gamma_multiply(0.35));
    painter.rect_filled(outer, 0.0, paper);
    let font_px = (l.h as f32 * px).clamp(1.0, 400.0);
    let title_h = l.title_h as f32 * px;
    let row_h = l.row_h as f32 * px;
    // Title row and header rule.
    painter.hline(outer.x_range(), tl.y + title_h, line);
    painter.hline(outer.x_range(), tl.y + title_h + row_h, line);
    for k in 1..=l.table.rows.len() {
        let y = tl.y + title_h + row_h * (k + 1) as f32;
        if k < l.table.rows.len() {
            painter.hline(outer.x_range(), y, faint);
        }
    }
    painter.rect_stroke(outer, 0.0, Stroke::new(1.6_f32, ink), StrokeKind::Inside);
    if font_px < MIN_TEXT_PX {
        return;
    }
    let bold = FontId::proportional(font_px);
    painter.text(
        Pos2::new(outer.center().x, tl.y + title_h * 0.5),
        Align2::CENTER_CENTER,
        &l.table.title,
        FontId::proportional(font_px * 1.15),
        ink,
    );
    let pad = (PAD_X * l.h) as f32 * px;
    let mut x = tl.x;
    for (i, w) in l.col_w.iter().enumerate() {
        let wpx = *w as f32 * px;
        if i > 0 {
            painter.vline(x, (tl.y + title_h)..=outer.bottom(), faint);
        }
        painter.text(
            Pos2::new(x + pad, tl.y + title_h + row_h * 0.5),
            Align2::LEFT_CENTER,
            &l.table.columns[i],
            bold.clone(),
            ink,
        );
        for (r, row) in l.table.rows.iter().enumerate() {
            painter.text(
                Pos2::new(x + pad, tl.y + title_h + row_h * (r as f32 + 1.5)),
                Align2::LEFT_CENTER,
                row.get(i).map_or("", String::as_str),
                FontId::proportional(font_px),
                ink,
            );
        }
        x += wpx;
    }
}

pub(crate) fn draw_label(
    painter: &egui::Painter,
    cam: &Camera,
    c: &Callout,
    h: f64,
    ink: Color32,
    paper: Color32,
) {
    let px = cam.px_per_in as f32;
    let font_px = (h as f32 * px).clamp(1.0, 300.0);
    if font_px < MIN_TEXT_PX {
        return;
    }
    let at = cam.world_to_screen(c.at);
    if !cam.rect.expand(40.0).contains(at) {
        return;
    }
    let radius = (text_w(&c.text, h, false) as f32 * 0.5 + 0.35 * h as f32) * px;
    let stroke = Stroke::new(1.2_f32, ink);
    match c.kind {
        ScheduleKind::Door => {
            painter.circle(at, radius, paper, stroke);
        }
        ScheduleKind::Window => {
            let pts: Vec<Pos2> = (0..6)
                .map(|k| {
                    let a = std::f32::consts::FRAC_PI_3 * k as f32 + std::f32::consts::FRAC_PI_6;
                    Pos2::new(at.x + radius * 1.1 * a.cos(), at.y + radius * 1.1 * a.sin())
                })
                .collect();
            painter.add(Shape::convex_polygon(pts, paper, stroke));
        }
        _ => {}
    }
    painter.text(
        at,
        Align2::CENTER_CENTER,
        &c.text,
        FontId::proportional(font_px),
        ink,
    );
}

/// Draws the active floor's schedule tables and callout labels. Called from
/// `render::draw_plan`.
pub fn draw_schedules(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let layer = layer_rc(cx);
    if layer.is_empty() {
        return;
    }
    let pal = &cx.palette;
    for s in &layer.schedules {
        if !cx.layers().is_visible(&s.layer) {
            continue;
        }
        let l = layout_rc(cx, s, cx.floor);
        let style = cx.project.text_styles.resolve(&s.text_style);
        let ink = color_of(style, pal.text);
        draw_table(painter, cam, &l, s.position, ink, pal.background);
        if is_selected(cx, s.id) {
            let (lo, hi) = l.bounds(s.position);
            let r = Rect::from_two_pos(
                cam.world_to_screen(Point::new(lo.x, hi.y)),
                cam.world_to_screen(Point::new(hi.x, lo.y)),
            );
            painter.rect_stroke(
                r.expand(2.0),
                0.0,
                Stroke::new(3.0_f32, pal.selection),
                StrokeKind::Outside,
            );
        }
    }
    let style = label_style(&cx.project);
    let h = style.map_or(4.5, |s| s.height_in);
    let ink = color_of(style, pal.text);
    // Doors and windows are labelled over the opening itself (the mark when
    // a schedule numbers them, else the size): see `opening_view`.
    for c in labels(cx)
        .iter()
        .filter(|c| !matches!(c.kind, ScheduleKind::Door | ScheduleKind::Window))
    {
        draw_label(painter, cam, c, h, ink, pal.background);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::{OpeningKind, WallKind};

    fn cx() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let w = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.5,
            109.125,
            WallKind::Exterior,
        );
        cx.project
            .add_opening(0, w, 200.0, OpeningKind::Door)
            .unwrap();
        cx.project
            .add_opening(0, w, 60.0, OpeningKind::Door)
            .unwrap();
        cx.project
            .add_opening(0, w, 130.0, OpeningKind::Window)
            .unwrap();
        cx.mark_dirty();
        cx.refresh();
        cx
    }

    #[test]
    fn placing_stores_the_schedule_and_undo_removes_it() {
        let mut cx = cx();
        clear_selection(&mut cx);
        let id = add(&mut cx, ScheduleKind::Door, Point::new(10.0, -40.0));
        let l = load(&cx);
        assert_eq!(l.schedules.len(), 1);
        assert_eq!(l.schedules[0].id, id);
        assert_eq!(l.schedules[0].kind, ScheduleKind::Door);
        assert_eq!(cx.undo_label(), Some("Place Door Schedule"));
        assert!(cx.project.layers.get("Schedules").is_some());
        assert!(cx.floor().schedules.is_some());
        cx.undo();
        assert!(load(&cx).is_empty());
        assert!(cx.floor().schedules.is_none());
        cx.redo();
        assert_eq!(load(&cx).schedules.len(), 1);
    }

    #[test]
    fn move_replace_and_delete_are_single_undo_steps() {
        let mut cx = cx();
        let id = add(&mut cx, ScheduleKind::Window, Point::ZERO);
        assert!(move_to(&mut cx, id, Point::new(50.0, 60.0)));
        assert_eq!(find(&cx, id).unwrap().position, Point::new(50.0, 60.0));
        assert_eq!(cx.undo_label(), Some("Move Schedule"));
        let mut d = find(&cx, id).unwrap();
        d.title = "Windows".into();
        assert!(replace(&mut cx, 0, d));
        assert_eq!(cx.undo_label(), Some("Schedule Specification"));
        assert_eq!(table_for(&cx, &find(&cx, id).unwrap(), 0).title, "Windows");
        select(&mut cx, id);
        assert!(delete(&mut cx, id));
        assert_eq!(selected(&cx), None);
        assert!(load(&cx).is_empty());
        assert_eq!(cx.undo().as_deref(), Some("Delete Schedule"));
        assert_eq!(cx.undo().as_deref(), Some("Schedule Specification"));
        assert!(!delete(&mut cx, 9999));
        let mut ghost = Schedule::new(ScheduleKind::Door, Point::ZERO);
        ghost.id = 9999;
        assert!(!replace(&mut cx, 0, ghost));
    }

    #[test]
    fn the_table_is_live_and_sized_from_the_text_style() {
        let mut cx = cx();
        let id = add(&mut cx, ScheduleKind::Door, Point::new(0.0, -20.0));
        let def = find(&cx, id).unwrap();
        let l = layout_of(&cx, &def, 0);
        assert_eq!(l.table.rows.len(), 2);
        assert!((l.h - 4.5).abs() < 1e-9, "Schedule Style is 4.5\"");
        assert_eq!(l.col_w.len(), l.table.columns.len());
        let expected = l.title_h + l.row_h * 3.0;
        assert!((l.height - expected).abs() < 1e-9);
        // Adding a door adds a row without touching the schedule.
        let w = cx.floor().walls[0].id;
        cx.project
            .add_opening(0, w, 20.0, OpeningKind::Door)
            .unwrap();
        cx.mark_dirty();
        cx.refresh();
        assert_eq!(layout_of(&cx, &def, 0).table.rows.len(), 3);
        // Picking: inside the rectangle, and not outside it.
        let (lo, hi) = l.bounds(def.position);
        let inside = Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5);
        assert_eq!(pick(&cx, inside), Some(id));
        assert_eq!(pick(&cx, Point::new(hi.x + 50.0, hi.y + 50.0)), None);
        cx.project.layers.set_display("Schedules", false);
        cx.refresh();
        assert_eq!(pick(&cx, inside), None);
    }

    #[test]
    fn hiding_a_column_shrinks_the_table_and_csv_has_the_header() {
        let mut cx = cx();
        let id = add(&mut cx, ScheduleKind::Door, Point::ZERO);
        let mut def = find(&cx, id).unwrap();
        let before = layout_of(&cx, &def, 0);
        let i = def.columns.iter().position(|c| c.field == "swing").unwrap();
        def.set_column_visible(i, false);
        assert!(replace(&mut cx, 0, def));
        let def = find(&cx, id).unwrap();
        let after = layout_of(&cx, &def, 0);
        assert_eq!(after.table.columns.len(), before.table.columns.len() - 1);
        assert!(after.width < before.width);
        let (name, csv) = export_csv(&cx, id).unwrap();
        assert_eq!(name, "Door_Schedule.csv");
        assert!(
            csv.starts_with("Mark,Floor,Width,Height,Type,Wall\n"),
            "{csv}"
        );
    }

    #[test]
    fn labels_need_a_schedule_with_show_labels_and_a_visible_layer() {
        let mut cx = cx();
        assert!(labels(&cx).is_empty());
        let id = add(&mut cx, ScheduleKind::Door, Point::ZERO);
        let l = labels(&cx);
        let texts: Vec<&str> = l.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts, ["D01", "D02"]);
        // The windows are unlabelled until a window schedule exists.
        add(&mut cx, ScheduleKind::Window, Point::new(0.0, -300.0));
        assert_eq!(labels(&cx).len(), 3);
        let mut def = find(&cx, id).unwrap();
        def.show_labels = false;
        replace(&mut cx, 0, def);
        assert_eq!(labels(&cx).len(), 1);
        cx.project.layers.set_display("Schedules", false);
        cx.refresh();
        assert!(labels(&cx).is_empty());
    }

    #[test]
    fn label_style_follows_schedule_label_when_present() {
        let mut cx = cx();
        assert!(label_style(&cx.project).is_some());
        let mut s = TextStyle::plan_sized(SCHEDULE_LABEL_STYLE, 9.0, true);
        s.color = [200, 0, 0];
        cx.project.text_styles.add(s);
        assert_eq!(label_style(&cx.project).unwrap().height_in, 9.0);
    }

    #[test]
    fn drawing_runs_without_panicking_at_several_zooms() {
        let mut cx = cx();
        add(&mut cx, ScheduleKind::Door, Point::new(0.0, -40.0));
        add(&mut cx, ScheduleKind::Window, Point::new(300.0, -40.0));
        add(&mut cx, ScheduleKind::General, Point::new(600.0, -40.0));
        add(&mut cx, ScheduleKind::Room, Point::new(600.0, -240.0));
        cx.refresh();
        let egui_ctx = egui::Context::default();
        let _ = egui_ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (_, painter) =
                    ui.allocate_painter(egui::vec2(800.0, 600.0), egui::Sense::hover());
                for px in [0.2, 1.0, 4.0] {
                    let mut cam = Camera::default_view();
                    cam.px_per_in = px;
                    draw_schedules(&cx, &painter, &cam);
                }
            });
        });
    }
}
