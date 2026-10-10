//! Schedules placed in the plan: storage, editing, picking and drawing
//! (`plan_core::schedules`, `plan_docs::schedule_kinds`, the Schedule flyout).
//!
//! # Storage
//!
//! A floor's [`ScheduleLayer`] lives in the typed slot `Floor.schedules`;
//! [`save`] also makes sure the layer the tables are drawn on exists. Undo and
//! redo restore it with the rest of the project. Every edit is one undo step.
//! The plan-wide Schedule Defaults and custom categories live in
//! `Project::schedule_setup`.
//!
//! # Drawing
//!
//! [`draw_schedules`] is called once from `render::draw_plan`. Each schedule
//! is a table (title row, heading row, one row per object, a Totals row) whose
//! rows come from the plan, so it is always live: the built tables, their
//! layouts and the callout labels are kept in a cache that is dropped on
//! every change signal of the editor context (`EditorContext::cache_key`), so
//! a frame that changed nothing only draws. Text sizes are the text styles'
//! character heights in plan inches (the same scale as every other
//! annotation). The same call draws the callout labels (D01, W03, C-01 ...)
//! next to doors, windows, cabinets and fixtures when a schedule of that kind
//! on the floor has Show Labels on.
//!
//! The submodules split the work: [`layout`] places the cells (Swap
//! Rows/Columns, Wrapping, column widths, the turn), [`paint`] draws them and
//! the callout shapes, [`handles`] holds the edit handles of a selected
//! schedule and [`ops`] the Edit toolbar commands (Renumber Schedule, Move
//! Row, Open Row Object(s), Schedule to Text, Find in Plan, ...).
//!
//! # Selection
//!
//! A schedule is `ObjectRef::Schedule(id)`: Select Objects and the Schedule
//! tool share `cx.selection`, and [`draw_schedules`] highlights the selected
//! ones.

pub mod handles;
pub mod layout;
mod ops;
mod paint;

pub use handles::HandleKind;
pub use layout::{Frame, Layout, Metrics};
pub use ops::*;

use super::{Camera, EditorContext, ObjectRef};
use eframe::egui::{self, Color32, Pos2, Rect, Shape, Stroke};
use plan_core::geometry::Point;
use plan_core::schedules::{
    ensure_layer, LabelOptions, RoomRef, Schedule, ScheduleKind, ScheduleLayer,
    SCHEDULE_LABEL_STYLE,
};
use plan_core::{Id, Project, TextStyle};
use plan_docs::schedule_kinds::{self, Callout};
use plan_docs::Schedule as Table;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

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

/// One schedule that shows callouts: the layer they are drawn on, the
/// callouts and how they look.
struct LabelSource {
    layer: String,
    kind: ScheduleKind,
    callouts: Vec<Callout>,
    opts: LabelOptions,
}

type LabelSources = Vec<LabelSource>;

thread_local! {
    static CACHE: RefCell<Option<FloorCache>> = const { RefCell::new(None) };
    /// The look of the callouts of each kind, for the callouts other views
    /// draw (the door and window marks over the openings).
    static LABEL_OPTS: RefCell<Vec<(ScheduleKind, LabelOptions)>> = const { RefCell::new(Vec::new()) };
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
    if let Some(l) = with_cache(cx, |c| {
        c.layouts
            .get(&(floor, def.id))
            .filter(|(d, _)| d == def)
            .map(|(_, l)| l.clone())
    }) {
        return l;
    }
    // Built outside the cache borrow: it reads the plan, not the cache.
    let l = Rc::new(build_layout(cx, def, floor));
    with_cache(cx, |c| {
        c.layouts.insert((floor, def.id), (def.clone(), l.clone()));
    });
    l
}

fn active_rooms<'a>(
    cx: &'a EditorContext,
    floor: usize,
) -> plan_docs::schedule_kinds::ActiveRooms<'a> {
    (floor == cx.floor).then_some((floor, cx.rooms.as_slice()))
}

/// Builds the layout of `def` placed on `floor`.
pub fn build_layout(cx: &EditorContext, def: &Schedule, floor: usize) -> Layout {
    let rooms = active_rooms(cx, floor);
    let built = schedule_kinds::built(&cx.project, def, floor, rooms);
    let cols = schedule_kinds::effective_columns(&cx.project, def);
    layout::build(layout::Input {
        table: built.table,
        def,
        cols: &cols,
        previews: built.previews,
        metrics: metrics(&cx.project, def),
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

/// A new schedule of `kind` as the Schedule Defaults make it, with its
/// upper-left corner at `at`, its id and the numbers its objects start with
/// (existing objects in order of their labels, p. 715).
pub fn new_schedule(cx: &mut EditorContext, kind: ScheduleKind, at: Point) -> Schedule {
    let id = cx.project.alloc_id();
    let mut s = cx.project.schedule_setup.template(kind, at);
    s.id = id;
    let rooms = active_rooms(cx, cx.floor);
    s.numbers = schedule_kinds::snapshot_numbers(&cx.project, &s, rooms);
    // A schedule that does not take new types (Notes) records the types it
    // has now as ticked.
    if !s.new_types_included && s.categories.is_empty() {
        for c in schedule_kinds::category_ids(&cx.project, kind) {
            s.set_category(&c, true);
        }
    }
    s
}

/// Places a new schedule of `kind` with its upper-left corner at `at` on the
/// active floor. Returns its id.
pub fn add(cx: &mut EditorContext, kind: ScheduleKind, at: Point) -> Id {
    let s = new_schedule(cx, kind, at);
    let id = s.id;
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
    replace_as(cx, fi, def, "Schedule Specification")
}

/// [`replace`] as the undo step `label`.
pub fn replace_as(cx: &mut EditorContext, fi: usize, def: Schedule, label: &str) -> bool {
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
    // A custom category the dialog made joins the plan with the schedule that
    // ticks it.
    let unknown: Vec<String> = def
        .categories
        .keys()
        .filter_map(|k| k.strip_prefix("Custom/"))
        .filter(|n| cx.project.schedule_setup.category(n).is_none())
        .map(str::to_string)
        .collect();
    if !unknown.is_empty() {
        cx.begin_change(label);
        for n in &unknown {
            let _ = cx.project.schedule_setup.add_category(n);
        }
        let mut layer = ScheduleLayer::load(&cx.project.floors[fi]);
        if let Some(s) = layer.find_mut(def.id) {
            *s = def;
        }
        save(&mut cx.project, fi, &layer);
        cx.mark_dirty();
        return true;
    }
    edit_floor(cx, fi, label, |l| {
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
    schedule_kinds::table(&cx.project, def, floor, active_rooms(cx, floor))
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

/// The character heights of a schedule's three text styles.
pub fn metrics(project: &Project, def: &Schedule) -> Metrics {
    let body = text_height(project, def);
    let of = |name: &str, fallback: f64| {
        if name.is_empty() {
            fallback
        } else {
            project
                .text_styles
                .resolve(name)
                .map_or(fallback, |s| s.height_in)
        }
    };
    Metrics {
        body,
        title: of(&def.title_style, body * 1.15),
        head: of(&def.header_style, body),
    }
}

/// Sizes a table for text of character height `h` and the columns' own width
/// settings (the plain layout: no previews, one text size).
pub fn layout(table: Table, def: &Schedule, h: f64) -> Layout {
    let cols: Vec<plan_core::schedules::ColumnSpec> = def
        .visible_columns()
        .filter(|c| {
            def.kind.fields().iter().any(|f| f.id == c.field)
                || c.field.starts_with(plan_core::props::COLUMN_PREFIX)
        })
        .cloned()
        .collect();
    layout::build(layout::Input {
        previews: vec![None; table.rows.len()],
        table,
        def,
        cols: &cols,
        metrics: Metrics::uniform(h),
    })
}

/// The character height of a schedule's text style.
pub fn text_height(project: &Project, def: &Schedule) -> f64 {
    project
        .text_styles
        .resolve(&def.text_style)
        .map_or(4.5, |s| s.height_in)
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
        .find(|s| layout_rc(cx, s, cx.floor).contains(s.position, p))
        .map(|s| s.id)
}

// ===================================================================
// Callout labels
// ===================================================================

/// The labels to draw on the active floor: for each schedule that shows
/// callouts, those of its objects on the floor (when the layer is visible).
pub fn labels(cx: &EditorContext) -> Vec<Callout> {
    let sources = label_sources(cx);
    sources
        .iter()
        .filter(|s| cx.layers().is_visible(&s.layer))
        .flat_map(|s| s.callouts.iter().cloned())
        .collect()
}

/// The callouts of each label source on the active floor, whatever the
/// visibility of its layer (cached with the tables).
fn label_sources(cx: &EditorContext) -> Rc<LabelSources> {
    if let Some(l) = with_cache(cx, |c| c.labels.clone()) {
        return l;
    }
    let layer = layer_rc(cx);
    let mut out: LabelSources = Vec::new();
    for kind in [
        ScheduleKind::Door,
        ScheduleKind::Window,
        ScheduleKind::Cabinet,
        ScheduleKind::Fixture,
    ] {
        for def in layer.label_sources(kind) {
            let callouts = schedule_kinds::callouts(&cx.project, cx.floor, def);
            out.push(LabelSource {
                layer: def
                    .label
                    .layer
                    .layer_of(def)
                    .unwrap_or(def.layer.as_str())
                    .to_string(),
                kind,
                callouts,
                opts: def.label.clone(),
            });
        }
    }
    LABEL_OPTS.with(|o| {
        let mut o = o.borrow_mut();
        o.clear();
        for s in &out {
            if !o.iter().any(|(k, _)| *k == s.kind) {
                o.push((s.kind, s.opts.clone()));
            }
        }
    });
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

/// Draws one callout label in the shape the schedule that numbers it asks
/// for (a circle for doors, a hexagon for windows unless it says otherwise).
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
    if font_px < paint::MIN_TEXT_PX {
        return;
    }
    let at = cam.world_to_screen(c.at);
    if !cam.rect.expand(40.0).contains(at) {
        return;
    }
    let opts = LABEL_OPTS
        .with(|o| {
            o.borrow()
                .iter()
                .find(|(k, _)| *k == c.kind)
                .map(|(_, o)| o.clone())
        })
        .unwrap_or_default();
    paint::draw_callout_shape(
        painter, at, &c.text, font_px, &opts, c.kind, ink, paper, 0.0,
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
        let body = cx.project.text_styles.resolve(&s.text_style);
        let title = if s.title_style.is_empty() {
            body
        } else {
            cx.project.text_styles.resolve(&s.title_style)
        };
        let head = if s.header_style.is_empty() {
            body
        } else {
            cx.project.text_styles.resolve(&s.header_style)
        };
        let colors = paint::Colors {
            body: color_of(body, pal.text),
            title: color_of(title, pal.text),
            head: color_of(head, pal.text),
            paper: pal.background,
        };
        paint::draw_schedule(painter, cam, s, &l, colors);
        if is_selected(cx, s.id) {
            let f = l.frame(s.position);
            let pts: Vec<Pos2> = [
                (0.0, 0.0),
                (l.width, 0.0),
                (l.width, l.height),
                (0.0, l.height),
            ]
            .iter()
            .map(|(x, y)| cam.world_to_screen(f.to_plan(*x, *y)))
            .collect();
            painter.add(Shape::closed_line(pts, Stroke::new(3.0_f32, pal.selection)));
            paint::draw_handles(
                painter,
                cam,
                &handles::handles(s, &l),
                pal.selection,
                pal.background,
            );
            if let Some(row) = selected_row_of(s.id, cx.floor) {
                ops::draw_row_highlight(painter, cam, s, &l, row, pal.selection);
            }
        }
    }
    let style = label_style(&cx.project);
    let h = style.map_or(4.5, |s| s.height_in);
    let ink = color_of(style, pal.text);
    // Doors and windows are labelled over the opening itself (the mark when
    // a schedule numbers them, else the size): see `opening_view`.
    let sources = label_sources(cx);
    for src in sources
        .iter()
        .filter(|s| cx.layers().is_visible(&s.layer))
        .filter(|s| !matches!(s.kind, ScheduleKind::Door | ScheduleKind::Window))
    {
        for c in &src.callouts {
            let px = cam.px_per_in as f32;
            let font_px = (h as f32 * px).clamp(1.0, 300.0);
            if font_px < paint::MIN_TEXT_PX {
                continue;
            }
            let at = cam.world_to_screen(c.at);
            if !cam.rect.expand(40.0).contains(at) {
                continue;
            }
            paint::draw_callout_shape(
                painter,
                at,
                &c.text,
                font_px,
                &src.opts,
                c.kind,
                ink,
                pal.background,
                0.0,
            );
        }
    }
}

/// The rooms of the active floor as `(RoomRef, name)` for "Include Objects
/// from Room".
pub fn room_refs(cx: &EditorContext) -> Vec<(RoomRef, String)> {
    schedule_kinds::room_choices(&cx.project, Some((cx.floor, cx.rooms.as_slice())))
        .into_iter()
        .map(|(f, name, p)| (RoomRef::at(f, p), name))
        .collect()
}

#[allow(dead_code)]
fn _keep(_: Rect) {}

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
