//! Library Symbol placement (CB-55..CB-57 in
//! `docs/parity/cabinets-stairs-framing-terrain-library.md`).
//!
//! * the active library item is set from the Library Browser
//!   ([`set_active_item`]) and a click in the plan places a copy (CB-55); the
//!   tool stays active for repeats;
//! * wall-mounted items rotate to face away from the nearest wall and snap
//!   flush within the auto-rotate distance (CB-56); ceiling items center on
//!   the cursor at the ceiling; countertop items snap onto the nearest
//!   cabinet top;
//! * a placed symbol is picked with a click; its handles are Move, Rotate and
//!   Resize (width on both sides, depth at the front); double-click opens the
//!   Symbol Specification; the Edit toolbar offers Open Object, Delete, Copy
//!   and a placeholder Replace From Library (CB-57).
//!
//! Chief Architect catalog objects (`chief.<catalog-uuid>.<id>`) are placed the
//! same way: the Library Browser bridges the clicked object into the transient
//! catalog of [`chief`] and [`find_item`] returns it like a built-in item.
//!
//! The active item id is kept per thread (the shared `SessionExtras` has no
//! slot for it and `editor/mod.rs` is not this tool's to change).

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::handles::{self, hit_handle, Handle, HandleKind};
use crate::editor::placed::{
    self, hit_symbol, load_cabinets, placed_handles, placed_symbol_strokes, symbol_axes,
    symbol_center, PlacedRef,
};
use crate::editor::{Camera, EditAction, EditorContext, EditorRequest, ObjectRef};
use eframe::egui::{self, Key, Pos2};
use plan_cabinets::Cabinet;
use plan_core::geometry::{dist_to_segment, Point};
use plan_core::{Id, PlacedSymbol};
use plan_library::{CatalogItem, Library, Placement};
use std::cell::RefCell;
use std::ops::Deref;
use std::sync::{Arc, OnceLock};

pub mod chief;
pub mod make;
pub mod user;

/// Pixels the pointer must travel before a press becomes a drag.
const DRAG_THRESHOLD_PX: f32 = 3.0;
/// A countertop item snaps to a cabinet top within this distance, inches.
const COUNTER_SNAP: f64 = 12.0;

/// The built-in library, built once.
pub fn library_catalog() -> &'static Library {
    static LIB: OnceLock<Library> = OnceLock::new();
    LIB.get_or_init(Library::with_all_core)
}

/// A library item: built in (static) or a bridged Chief object.
pub enum ItemRef {
    Core(&'static CatalogItem),
    Chief(Arc<CatalogItem>),
    /// An item of the user library (saved pictures).
    User(Arc<CatalogItem>),
}

impl Deref for ItemRef {
    type Target = CatalogItem;

    fn deref(&self) -> &CatalogItem {
        match self {
            ItemRef::Core(i) => i,
            ItemRef::Chief(i) | ItemRef::User(i) => i,
        }
    }
}

/// The library item with this id: a built-in one, or a Chief object (bridged
/// on first use when only its id is known, as after a plan was reopened).
pub fn find_item(id: &str) -> Option<ItemRef> {
    match library_catalog().get(id) {
        Some(i) => Some(ItemRef::Core(i)),
        None => match super::images::user_item(id) {
            Some(i) => Some(ItemRef::User(i)),
            None => chief::resolve_item(id).map(|c| ItemRef::Chief(c.item)),
        },
    }
}

thread_local! {
    static ACTIVE: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// The catalog id the Library tool places, if one is chosen.
pub fn active_item() -> Option<String> {
    ACTIVE.with(|a| a.borrow().clone())
}

/// Makes `catalog_id` the item the Library tool places. Returns `false` (and
/// says so in the status bar) for an unknown id.
pub fn set_active_item(cx: &mut EditorContext, catalog_id: &str) -> bool {
    match find_item(catalog_id) {
        Some(item) => {
            ACTIVE.with(|a| *a.borrow_mut() = Some(item.id.clone()));
            user::touch_recent(&item.id);
            cx.status = format!("Library Symbol: click to place {}", item.name);
            true
        }
        None => {
            cx.status = format!("Unknown library item: {catalog_id}");
            false
        }
    }
}

pub fn clear_active_item() {
    ACTIVE.with(|a| *a.borrow_mut() = None);
}

/// A small search-and-pick list for the active item; the shell may show it
/// in a popup or panel.
pub fn library_quick_pick(ui: &mut egui::Ui, cx: &mut EditorContext) {
    let id = ui.id().with("library_quick_pick_query");
    let mut query: String = ui.data_mut(|d| d.get_temp(id).unwrap_or_default());
    ui.add(egui::TextEdit::singleline(&mut query).hint_text("Search the library"));
    ui.data_mut(|d| d.insert_temp(id, query.clone()));
    let active = active_item();
    egui::ScrollArea::vertical()
        .id_salt("library_quick_pick_list")
        .max_height(240.0)
        .show(ui, |ui| {
            for item in library_catalog().search(&query).into_iter().take(80) {
                let selected = active.as_deref() == Some(item.id.as_str());
                let text = format!("{}   {}\" x {}\"", item.name, item.width, item.depth);
                if ui.selectable_label(selected, text).clicked() {
                    set_active_item(cx, &item.id);
                }
            }
        });
}

// ----- placement -----

/// The cabinet whose top the point is nearest to (within [`COUNTER_SNAP`]).
fn nearest_counter(cabs: &[Cabinet], p: Point) -> Option<&Cabinet> {
    cabs.iter()
        .filter(|c| c.countertop.is_some())
        .map(|c| {
            let k = c.corners();
            let d = (0..4)
                .map(|i| dist_to_segment(p, k[i], k[(i + 1) % 4]))
                .fold(f64::INFINITY, f64::min);
            let local = p.sub(c.position);
            let (u, v) = (
                Point::new(c.angle.cos(), c.angle.sin()),
                Point::new(-c.angle.sin(), c.angle.cos()),
            );
            let inside =
                (0.0..=c.width).contains(&local.dot(u)) && (0.0..=c.depth).contains(&local.dot(v));
            (if inside { 0.0 } else { d }, c)
        })
        .filter(|(d, _)| *d <= COUNTER_SNAP)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, c)| c)
}

/// The symbol that a click at `p` would place for `item` (nothing is stored).
pub fn placement_for(cx: &EditorContext, item: &CatalogItem, p: &PointerEvent) -> PlacedSymbol {
    let mut s = PlacedSymbol::new(
        item.id.clone(),
        p.world,
        item.width,
        item.depth,
        item.height,
    );
    s.elevation = item.elevation;
    s.layer = user::layer_of(item);
    // A saved picture places the picture.
    s.image = super::images::spec_from_item(item);
    // Free placement: the symbol's center sits on the snapped point, so the
    // back-center is half a depth behind it.
    let free = |s: &mut PlacedSymbol, center: Point| {
        s.position = Point::new(center.x, center.y - item.depth * 0.5);
    };
    // What the item snaps to (its placement, or its own auto-rotate choice).
    let rules = plan_library::rules::placement_rules(item);
    match item.placement {
        Placement::WallMounted => {
            if !(rules.auto_rotate && s.auto_rotate_to_wall(&cx.floor().walls)) {
                s.position = p.snapped;
                s.angle = 0.0;
            }
        }
        Placement::FreeStanding => {
            // A free-standing item that turns to the wall (a bookcase):
            // try the wall with the cursor as the back-center first.
            s.position = p.snapped;
            if !(rules.auto_rotate && s.auto_rotate_to_wall(&cx.floor().walls)) {
                s.angle = 0.0;
                free(&mut s, p.snapped);
            }
        }
        Placement::Ceiling => {
            free(&mut s, p.snapped);
            s.elevation = (cx.floor().ceiling_height - item.height).max(0.0);
        }
        Placement::Countertop => {
            let cabs = load_cabinets(cx.floor());
            match nearest_counter(&cabs, p.world) {
                Some(c) => {
                    let (hw, hd) = (item.width * 0.5, item.depth * 0.5);
                    let (u, v) = (
                        Point::new(c.angle.cos(), c.angle.sin()),
                        Point::new(-c.angle.sin(), c.angle.cos()),
                    );
                    let rel = p.world.sub(c.position);
                    let clamp = |x: f64, half: f64, len: f64| {
                        if half * 2.0 >= len {
                            len * 0.5
                        } else {
                            x.clamp(half, len - half)
                        }
                    };
                    let (lx, ly) = (
                        clamp(rel.dot(u), hw, c.width),
                        clamp(rel.dot(v), hd, c.depth),
                    );
                    s.angle = c.angle.to_degrees().rem_euclid(360.0);
                    s.position = c.to_plan(Point::new(lx, ly - hd));
                    s.elevation = c.elevation + c.height;
                }
                None => free(&mut s, p.snapped),
            }
        }
    }
    s
}

// ----- the tool -----

#[derive(Clone, Copy, Debug)]
struct Drag {
    op: HandleKind,
    start: Point,
    screen: Pos2,
    begun: bool,
}

#[derive(Default)]
pub struct LibraryTool {
    ghost: Option<PlacedSymbol>,
    original: Option<PlacedSymbol>,
    drag: Option<Drag>,
}

fn op_label(op: HandleKind) -> &'static str {
    match op {
        HandleKind::Rotate => "Rotate Symbol",
        HandleKind::ResizeStart | HandleKind::ResizeEnd | HandleKind::Reshape(_) => "Resize Symbol",
        _ => "Move Symbol",
    }
}

fn snap_to(v: f64, unit: f64, alt: bool) -> f64 {
    if alt {
        v
    } else {
        (v / unit).round() * unit
    }
}

/// The symbol after dragging handle `op` from `start` to `p`, starting from
/// `orig`.
pub fn apply_drag(
    cx: &EditorContext,
    op: HandleKind,
    orig: &PlacedSymbol,
    start: Point,
    p: &PointerEvent,
) -> PlacedSymbol {
    let mut s = orig.clone();
    let alt = p.modifiers.alt;
    let unit = cx.snap_unit();
    let (u, v) = symbol_axes(orig);
    match op {
        HandleKind::Move => {
            let delta = p.world.sub(start);
            s.position = Point::new(
                snap_to(orig.position.x + delta.x, unit, alt),
                snap_to(orig.position.y + delta.y, unit, alt),
            );
            let ctrl = p.modifiers.command || p.modifiers.ctrl;
            if !ctrl && placed::symbol_placement(orig) == Placement::WallMounted {
                s.auto_rotate_to_wall(&cx.floor().walls);
            }
        }
        HandleKind::ResizeEnd | HandleKind::ResizeStart => {
            let t = p.world.sub(orig.position).dot(u);
            let half = orig.width * 0.5;
            let end = op == HandleKind::ResizeEnd;
            let w = if end { t + half } else { half - t };
            let w = snap_to(w, unit, alt).max(1.0);
            let shift = (w - orig.width) * 0.5;
            s.width = w;
            s.position = orig.position + u * if end { shift } else { -shift };
        }
        HandleKind::Reshape(_) => {
            let t = p.world.sub(orig.position).dot(v);
            s.depth = snap_to(t, unit, alt).max(1.0);
        }
        HandleKind::Rotate => {
            let center = symbol_center(orig);
            let d = p.world.sub(center);
            if d.length() > 1e-6 {
                let step = if cx.defaults.grid.angle_snap_deg > 0.0 {
                    cx.defaults.grid.angle_snap_deg
                } else {
                    15.0
                };
                let a = d.angle().to_degrees() - 90.0;
                s.angle = snap_to(a, step, alt).rem_euclid(360.0);
                let nv = Point::new(-s.angle.to_radians().sin(), s.angle.to_radians().cos());
                s.position = center - nv * (s.depth * 0.5);
            }
        }
        _ => {}
    }
    s
}

fn write_symbol(cx: &mut EditorContext, s: &PlacedSymbol) {
    let fl = cx.floor;
    if let Some(slot) = cx.project.floors[fl]
        .symbols
        .iter_mut()
        .find(|x| x.id == s.id)
    {
        *slot = s.clone();
    }
    cx.mark_dirty();
}

impl LibraryTool {
    fn selected_symbol(cx: &EditorContext) -> Option<Id> {
        match cx.selection.single()? {
            ObjectRef::Symbol(id) => Some(id),
            _ => None,
        }
    }

    fn handles(cx: &EditorContext) -> Vec<Handle> {
        Self::selected_symbol(cx)
            .map(|id| placed_handles(cx.floor(), PlacedRef::Symbol(id), cx.px_per_in))
            .unwrap_or_default()
    }

    fn arm(&mut self, cx: &EditorContext, id: Id, op: HandleKind, p: &PointerEvent) {
        self.original = cx.floor().symbol(id).cloned();
        self.drag = Some(Drag {
            op,
            start: p.world,
            screen: p.screen,
            begun: false,
        });
    }

    fn cancel_drag(&mut self, cx: &mut EditorContext) -> bool {
        let Some(d) = self.drag.take() else {
            return false;
        };
        if d.begun {
            if let Some(o) = self.original.take() {
                write_symbol(cx, &o);
            }
            cx.cancel_change();
        }
        self.original = None;
        true
    }

    fn open_spec(cx: &mut EditorContext, id: Id) {
        cx.selection.set(ObjectRef::Symbol(id));
        cx.requests
            .push(EditorRequest::OpenSpec(ObjectRef::Symbol(id)));
    }
}

impl Tool for LibraryTool {
    fn id(&self) -> ToolId {
        ToolId::Library
    }

    fn name(&self) -> &'static str {
        "Library Symbol"
    }

    fn hint(&self) -> String {
        match active_item().and_then(|id| find_item(&id).map(|i| i.name.clone())) {
            Some(n) => format!("Library Symbol: click to place {n}"),
            None => {
                "Library Symbol: pick an item in the Library Browser, then click in the plan".into()
            }
        }
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        cx.status = self.hint();
    }

    fn deactivate(&mut self, cx: &mut EditorContext) {
        self.cancel_drag(cx);
        self.ghost = None;
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let (Some(d), true) = (self.drag, p.down) {
            let Some(orig) = self.original.clone() else {
                return ToolResult::ignored();
            };
            let moved = (p.screen - d.screen).length() >= DRAG_THRESHOLD_PX;
            if !d.begun && !moved {
                return ToolResult::consumed();
            }
            if !d.begun {
                cx.begin_change(op_label(d.op));
                self.drag = Some(Drag { begun: true, ..d });
            }
            let next = apply_drag(cx, d.op, &orig, d.start, &p);
            write_symbol(cx, &next);
            return ToolResult::consumed();
        }
        self.ghost = active_item()
            .and_then(|id| find_item(&id))
            .map(|item| placement_for(cx, &item, &p));
        ToolResult {
            repaint: true,
            ..ToolResult::default()
        }
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let tol = cx.pick_tol();
        // A handle of the selected symbol.
        if let Some(h) = hit_handle(&Self::handles(cx), p.world, tol) {
            if let Some(id) = Self::selected_symbol(cx) {
                self.arm(cx, id, h.kind, &p);
                return ToolResult::consumed();
            }
        }
        // An existing symbol: select it (and arm a move).
        if let Some(id) = hit_symbol(cx, p.world, 0.0) {
            if p.modifiers.shift {
                cx.selection.toggle(ObjectRef::Symbol(id));
            } else {
                cx.selection.set(ObjectRef::Symbol(id));
                self.arm(cx, id, HandleKind::Move, &p);
            }
            return ToolResult::consumed();
        }
        // Otherwise place the active item (CB-55).
        let Some(item) = active_item().and_then(|id| find_item(&id)) else {
            cx.status = self.hint();
            return ToolResult::consumed();
        };
        let mut sym = placement_for(cx, &item, &p);
        // An appliance dropped near its bay turns and sits in the cabinet's bay.
        placed::snap_symbol_to_bay(cx.floor(), &mut sym, placed::BAY_SNAP_REACH);
        cx.begin_change("Place Symbol");
        let fl = cx.floor;
        // Cabinets, CAD blocks and text come back as real plan objects.
        let obj = match user::place_payload(cx, &item, &sym) {
            Some(o) => o,
            None => {
                user::ensure_layer(cx, &sym.layer);
                ObjectRef::Symbol(cx.project.add_symbol(fl, sym))
            }
        };
        cx.selection.set(obj);
        user::touch_recent(&item.id);
        cx.mark_dirty();
        cx.status = format!("Placed {}", item.name);
        ToolResult::committed("Place Symbol")
    }

    fn pointer_up(&mut self, _cx: &mut EditorContext, _p: PointerEvent) -> ToolResult {
        let Some(d) = self.drag.take() else {
            return ToolResult::ignored();
        };
        self.original = None;
        if d.begun {
            ToolResult::committed(op_label(d.op))
        } else {
            ToolResult::consumed()
        }
    }

    fn double_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        match hit_symbol(cx, p.world, cx.pick_tol()) {
            Some(id) => {
                Self::open_spec(cx, id);
                ToolResult::consumed()
            }
            None => ToolResult::ignored(),
        }
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if k.is(Key::Escape) {
            return if self.cancel_drag(cx) {
                ToolResult::consumed()
            } else {
                ToolResult::ignored()
            };
        }
        if k.is(Key::Delete) || k.is(Key::Backspace) {
            if placed::delete_placed(cx) > 0 {
                return ToolResult::committed("Delete");
            }
            return ToolResult::ignored();
        }
        if k.is(Key::Enter) {
            if let Some(id) = Self::selected_symbol(cx) {
                Self::open_spec(cx, id);
                return ToolResult::consumed();
            }
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let pal = &cx.palette;
        if let Some(g) = &self.ghost {
            if self.drag.is_none() {
                let stroke = egui::Stroke::new(1.5_f32, pal.ghost_stroke);
                match placed_symbol_strokes(g) {
                    Some(sym) => placed::draw_library_strokes(painter, cam, &sym, stroke),
                    None => {
                        let pts = g
                            .footprint()
                            .iter()
                            .map(|q| cam.world_to_screen(*q))
                            .collect();
                        painter.add(egui::Shape::closed_line(pts, stroke));
                    }
                }
            }
        }
        handles::draw(&Self::handles(cx), painter, cam, pal);
    }

    fn edit_toolbar(&self, cx: &EditorContext) -> Vec<EditAction> {
        let mut v = cx.common_edit_actions();
        // Replace From Library (CB-57) and friends.
        v.extend(cx.extra_edit_actions());
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::WallKind;

    fn setup() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        cx
    }

    fn click(t: &mut LibraryTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let p = PointerEvent::at(cx, Point::new(x, y));
        t.pointer_move(cx, p);
        let r = t.pointer_down(cx, p.with_down(true));
        t.pointer_up(cx, p);
        r
    }

    fn first_of(placement: Placement) -> &'static CatalogItem {
        library_catalog()
            .all_items()
            .find(|i| i.placement == placement)
            .unwrap()
    }

    #[test]
    fn catalog_is_built_once_and_has_every_placement() {
        assert!(std::ptr::eq(library_catalog(), library_catalog()));
        for p in [
            Placement::WallMounted,
            Placement::FreeStanding,
            Placement::Ceiling,
            Placement::Countertop,
        ] {
            let _ = first_of(p);
        }
    }

    #[test]
    fn wall_mounted_items_rotate_flush_to_the_nearest_wall() {
        let mut cx = setup();
        let item = first_of(Placement::WallMounted);
        assert!(set_active_item(&mut cx, &item.id));
        let mut t = LibraryTool::default();
        // 4" above the wall's upper face (face at y = 3): faces +y.
        let r = click(&mut t, &mut cx, 100.0, 7.0);
        assert_eq!(r.commit.as_deref(), Some("Place Symbol"));
        let s = &cx.floor().symbols[0];
        assert!((s.angle - 0.0).abs() < 1e-9);
        assert!((s.position.y - 3.0).abs() < 1e-9, "{:?}", s.position);
        assert_eq!((s.width, s.depth), (item.width, item.depth));
        // Below the wall: faces -y, rotated 180.
        click(&mut t, &mut cx, 160.0, -6.0);
        let s2 = &cx.floor().symbols[1];
        assert!((s2.angle - 180.0).abs() < 1e-9);
        assert!((s2.position.y + 3.0).abs() < 1e-9);
        // Far from any wall it stays where it was clicked.
        click(&mut t, &mut cx, 100.0, 100.0);
        let s3 = &cx.floor().symbols[2];
        assert_eq!(s3.angle, 0.0);
        assert_eq!(s3.position, Point::new(100.0, 100.0));
    }

    #[test]
    fn the_auto_rotate_option_decides_whether_an_item_snaps_to_the_wall() {
        let cx = setup();
        // 4" above the wall's upper face (face at y = 3).
        let p = PointerEvent::at(&cx, Point::new(100.0, 7.0));
        // A free-standing item that turns to the wall snaps flush.
        let mut shelf = first_of(Placement::FreeStanding).clone();
        shelf.auto_rotate = Some(true);
        let s = placement_for(&cx, &shelf, &p);
        assert!((s.position.y - 3.0).abs() < 1e-9 && s.angle.abs() < 1e-9, "{:?}", s.position);
        // Without the option it just centers on the click.
        shelf.auto_rotate = None;
        let s = placement_for(&cx, &shelf, &p);
        assert!((symbol_center(&s).y - p.snapped.y).abs() < 1e-9);
        // A wall-mounted item told never to turn stays where it was clicked.
        let mut vanity = first_of(Placement::WallMounted).clone();
        vanity.auto_rotate = Some(false);
        let s = placement_for(&cx, &vanity, &p);
        assert_eq!((s.position, s.angle), (p.snapped, 0.0));
        // Far from walls a snapping free item is placed freely.
        shelf.auto_rotate = Some(true);
        let far = PointerEvent::at(&cx, Point::new(100.0, 100.0));
        let s = placement_for(&cx, &shelf, &far);
        assert!((symbol_center(&s).y - far.snapped.y).abs() < 1e-9);
        // The item's default layer follows its category or its own choice.
        shelf.layer = Some("Furniture".into());
        assert_eq!(placement_for(&cx, &shelf, &far).layer, "Furniture");
    }

    #[test]
    fn free_standing_and_ceiling_items_center_on_the_click() {
        let mut cx = setup();
        let free = first_of(Placement::FreeStanding);
        let p = PointerEvent::at(&cx, Point::new(120.0, 80.0));
        let s = placement_for(&cx, free, &p);
        let c = symbol_center(&s);
        assert!((c.x - p.snapped.x).abs() < 1e-9 && (c.y - p.snapped.y).abs() < 1e-9);
        assert_eq!(s.angle, 0.0);
        let ceil = first_of(Placement::Ceiling);
        let s = placement_for(&cx, ceil, &p);
        assert!((s.elevation - (cx.floor().ceiling_height - ceil.height).max(0.0)).abs() < 1e-9);
        cx.floor_mut().ceiling_height = 108.0;
        let s = placement_for(&cx, ceil, &p);
        assert!((s.elevation - (108.0 - ceil.height).max(0.0)).abs() < 1e-9);
    }

    #[test]
    fn countertop_items_sit_on_the_nearest_cabinet_top() {
        let mut cx = setup();
        let mut cab = Cabinet::base(36.0);
        cab.position = Point::new(50.0, 3.0);
        placed::add_cabinet(&mut cx.project, 0, cab).unwrap();
        let item = first_of(Placement::Countertop);
        let p = PointerEvent::at(&cx, Point::new(68.0, 14.0));
        let s = placement_for(&cx, item, &p);
        assert!((s.elevation - 36.0).abs() < 1e-9);
        let c = symbol_center(&s);
        assert!(
            c.x >= 50.0 && c.x <= 86.0 && c.y >= 3.0 && c.y <= 27.0,
            "{c:?}"
        );
        // Out of reach: free placement at the default elevation.
        let far = PointerEvent::at(&cx, Point::new(200.0, 120.0));
        let s = placement_for(&cx, item, &far);
        assert_eq!(s.elevation, item.elevation);
    }

    #[test]
    fn no_active_item_places_nothing() {
        clear_active_item();
        let mut cx = setup();
        let mut t = LibraryTool::default();
        click(&mut t, &mut cx, 100.0, 100.0);
        assert!(cx.floor().symbols.is_empty());
        assert!(cx.status.contains("Library"));
        assert!(!set_active_item(&mut cx, "no.such.item"));
    }

    #[test]
    fn undo_removes_a_placed_symbol_and_selection_picks_it_back() {
        let mut cx = setup();
        let item = first_of(Placement::FreeStanding);
        set_active_item(&mut cx, &item.id);
        let mut t = LibraryTool::default();
        click(&mut t, &mut cx, 100.0, 100.0);
        assert_eq!(cx.floor().symbols.len(), 1);
        cx.selection.clear();
        // Clicking the symbol selects it instead of placing another.
        let c = symbol_center(&cx.floor().symbols[0]);
        click(&mut t, &mut cx, c.x, c.y);
        assert_eq!(cx.floor().symbols.len(), 1);
        assert!(matches!(cx.selection.single(), Some(ObjectRef::Symbol(_))));
        assert_eq!(cx.undo().as_deref(), Some("Place Symbol"));
        assert!(cx.floor().symbols.is_empty());
    }

    #[test]
    fn handles_move_resize_and_rotate_with_one_undo_step() {
        let mut cx = setup();
        let item = first_of(Placement::FreeStanding);
        set_active_item(&mut cx, &item.id);
        let mut t = LibraryTool::default();
        click(&mut t, &mut cx, 120.0, 120.0);
        let id = cx.floor().symbols[0].id;
        let orig = cx.floor().symbols[0].clone();

        // Resize: drag the right-edge handle 10" outward.
        let handle = placed_handles(cx.floor(), PlacedRef::Symbol(id), cx.px_per_in)
            .into_iter()
            .find(|h| h.kind == HandleKind::ResizeEnd)
            .unwrap();
        let mut down = PointerEvent::at(&cx, handle.pos).with_down(true);
        down.screen = Pos2::new(0.0, 0.0);
        t.pointer_down(&mut cx, down);
        let mut mv = PointerEvent::at(&cx, handle.pos + Point::new(10.0, 0.0)).with_down(true);
        mv.screen = Pos2::new(40.0, 0.0);
        t.pointer_move(&mut cx, mv);
        let r = t.pointer_up(&mut cx, mv);
        assert_eq!(r.commit.as_deref(), Some("Resize Symbol"));
        let s = &cx.floor().symbols[0];
        assert!((s.width - (orig.width + 10.0)).abs() < 1e-6, "{}", s.width);
        assert!((s.position.x - (orig.position.x + 5.0)).abs() < 1e-6);
        assert_eq!(cx.undo().as_deref(), Some("Resize Symbol"));
        assert_eq!(cx.floor().symbols[0].width, orig.width);

        // (Undo re-checks the selection; keep it for the next drag.)
        cx.selection.set(ObjectRef::Symbol(id));
        // Rotate to the right: handle sits above the front; drag it to +x.
        let rot = placed_handles(cx.floor(), PlacedRef::Symbol(id), cx.px_per_in)
            .into_iter()
            .find(|h| h.kind == HandleKind::Rotate)
            .unwrap();
        let mut down = PointerEvent::at(&cx, rot.pos).with_down(true);
        down.screen = Pos2::new(0.0, 0.0);
        t.pointer_down(&mut cx, down);
        let center = symbol_center(&cx.floor().symbols[0]);
        let mut mv = PointerEvent::at(&cx, center + Point::new(0.0, -50.0)).with_down(true);
        mv.screen = Pos2::new(40.0, 40.0);
        t.pointer_move(&mut cx, mv);
        t.pointer_up(&mut cx, mv);
        assert!((cx.floor().symbols[0].angle - 180.0).abs() < 1e-6);
        let c2 = symbol_center(&cx.floor().symbols[0]);
        assert!(c2.dist(center) < 1e-6, "rotation is about the center");
    }

    #[test]
    fn double_click_requests_the_spec_and_toolbar_lists_replace() {
        let mut cx = setup();
        let item = first_of(Placement::FreeStanding);
        set_active_item(&mut cx, &item.id);
        let mut t = LibraryTool::default();
        click(&mut t, &mut cx, 100.0, 100.0);
        let c = symbol_center(&cx.floor().symbols[0]);
        let id = cx.floor().symbols[0].id;
        let ev = PointerEvent::at(&cx, c);
        t.double_click(&mut cx, ev);
        assert!(cx
            .requests
            .contains(&EditorRequest::OpenSpec(ObjectRef::Symbol(id))));
        let labels: Vec<&str> = t.edit_toolbar(&cx).iter().map(|a| a.label).collect();
        assert!(labels.contains(&"Replace From Library"));
        assert!(labels.contains(&"Open Object"));
        // Delete key removes it.
        let r = t.key(&mut cx, KeyEvent::key(Key::Delete));
        assert_eq!(r.commit.as_deref(), Some("Delete"));
        assert!(cx.floor().symbols.is_empty());
    }
}
