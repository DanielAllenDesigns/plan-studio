//! Corner boards, quoins, moldings, material regions, wall hatching, polygon
//! decks and 3D solids: storage, editing, picking, plan drawing and the 3D
//! meshes (`plan_core::details`, the Trim flyout, Floor/Wall Material Region,
//! Wall Hatching, Polygon Shaped Deck and the 3D Solid flyout).
//!
//! # Storage
//!
//! A floor's [`DetailsLayer`] lives in the typed slot `Floor.details`;
//! [`save`] also makes sure the layers the objects are drawn on exist in the
//! plan. Undo and redo restore it with the rest of the project.
//!
//! # Selection
//!
//! The objects are `ObjectRef::Detail(id)` in `cx.selection`, shared by the
//! Select tool and the details tool ([`selected`], [`select`],
//! [`clear_selection`] take the context). `editor::selection` hit-tests with
//! [`pick_all`], box-selects with [`in_rect`], and `editor::dispatch` moves
//! and deletes with [`translate_ids`] and [`delete_ids`]. The corners the
//! Select tool drags are [`vertices`] / [`move_vertex_in`]. Wall material
//! regions and hatches belong to their wall: [`drop_orphans`] removes them
//! with it and [`follow_walls`] keeps corner boards and quoins on their
//! corners when walls move.
//!
//! # Drawing and 3D
//!
//! `render::draw_plan` calls [`draw_under`] (floor regions, decks and solids,
//! under the walls) and [`draw_over`] (wall hatching and wall regions, corner
//! trim and moldings, over the walls). The 3D view adds
//! [`detail_meshes`] to its scene.

use super::{Camera, EditorContext, ObjectRef};
use eframe::egui::{self, Color32, Pos2, Shape, Stroke};
use plan_3d::triangulate::ear_clip;
use plan_core::details::{
    self, bounds, circle_points, corner_near, exterior_corners, wall_strip, CornerBoard,
    DeckPolygon, DetailRef, DetailStyle, DetailsLayer, ExteriorCorner, MaterialRegion, MoldingLine,
    MoldingProfile, Quoin, RegionKind, Solid3d, SolidKind, WallHatch, CORNER_TRIM_LAYER,
    DECK_LAYER, MOLDING_LAYER, REGION_LAYER, SOLID_LAYER,
};
use plan_core::geometry::{
    dist_to_segment, point_in_polygon, polygon_area, project_on_segment, Point,
};
use plan_core::moldings::ProfileDef;
use plan_core::walls::Side;
use plan_core::{Id, Layer, LayerSet, LineStyle, Project, Wall, WallClass};
use plan_materials::{
    clip_strokes_to_polygon, core_library, pattern_strokes, MaterialLibrary, Pattern,
};
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::OnceLock;

/// Smallest outline area a drawn shape needs, square inches.
pub const MIN_AREA: f64 = 1.0;
/// Hatch lines of a polygon deck: the decking board pitch, inches.
const BOARD_PITCH: f64 = 5.5;
/// Hatching is skipped when it would cover less than this many pixels per inch.
const MIN_HATCH_PX_PER_IN: f64 = 0.04;

/// The layers the detail objects use, with Chief-like colors and weights.
fn layer_defaults() -> [Layer; 5] {
    [
        Layer::new(CORNER_TRIM_LAYER, [150, 110, 70], 25),
        Layer::new(MOLDING_LAYER, [160, 90, 50], 18),
        Layer::new(REGION_LAYER, [110, 110, 140], 18),
        Layer::new(DECK_LAYER, [150, 110, 60], 25),
        Layer::new(SOLID_LAYER, [90, 90, 90], 18),
    ]
}

// ===================================================================
// Storage and editing
// ===================================================================

/// The details layer of the active floor.
pub fn load(cx: &EditorContext) -> DetailsLayer {
    DetailsLayer::load(cx.floor())
}

/// Stores `layer` on floor `fi` and adds the layers it needs to the plan.
pub fn save(project: &mut Project, fi: usize, layer: &DetailsLayer) {
    layer.store(&mut project.floors[fi]);
    if !layer.is_empty() {
        ensure_layers(&mut project.layers);
    }
}

/// Adds the detail layers when the plan lacks them.
pub fn ensure_layers(set: &mut LayerSet) {
    for l in layer_defaults() {
        if set.get(&l.name).is_none() {
            set.layers.push(l);
        }
    }
}

/// Runs `edit` on the active floor's details layer as one undo step.
pub fn edit(cx: &mut EditorContext, label: &str, edit: impl FnOnce(&mut DetailsLayer)) {
    cx.begin_change(label);
    let mut layer = load(cx);
    edit(&mut layer);
    let fl = cx.floor;
    save(&mut cx.project, fl, &layer);
    cx.mark_dirty();
}

/// Adds a corner board on `corner`; returns its id.
pub fn add_corner_board(cx: &mut EditorContext, corner: &ExteriorCorner) -> Id {
    let id = cx.project.alloc_id();
    edit(cx, "Corner Board", |l| {
        l.corner_boards.push(CornerBoard::at(id, corner));
    });
    id
}

/// Adds a quoin stack on `corner`; returns its id.
pub fn add_quoin(cx: &mut EditorContext, corner: &ExteriorCorner) -> Id {
    let id = cx.project.alloc_id();
    edit(cx, "Quoins", |l| l.quoins.push(Quoin::at(id, corner)));
    id
}

/// Auto Place Corner Boards: a board on every convex exterior corner of the
/// active floor that has none. One undo step, none when nothing was added.
/// Returns how many were placed.
pub fn auto_corner_boards(cx: &mut EditorContext) -> usize {
    auto_place(
        cx,
        "Auto Place Corner Boards",
        |layer, floor, rooms, alloc| layer.auto_corner_boards(floor, rooms, alloc),
    )
}

/// Auto Place Quoins: a stack on every convex exterior corner that has none.
pub fn auto_quoins(cx: &mut EditorContext) -> usize {
    auto_place(cx, "Auto Place Quoins", |layer, floor, rooms, alloc| {
        layer.auto_quoins(floor, rooms, alloc)
    })
}

fn auto_place(
    cx: &mut EditorContext,
    label: &str,
    place: impl FnOnce(
        &mut DetailsLayer,
        &plan_core::Floor,
        &[plan_core::Room],
        &mut dyn FnMut() -> Id,
    ) -> usize,
) -> usize {
    let rooms = cx.rooms_now().to_vec();
    let floor = cx.floor().clone();
    let mut layer = load(cx);
    let n = place(&mut layer, &floor, &rooms, &mut || cx.project.alloc_id());
    if n > 0 {
        cx.begin_change(label);
        let fl = cx.floor;
        save(&mut cx.project, fl, &layer);
        cx.mark_dirty();
    }
    n
}

/// Adds a molding along `polyline`; returns its id.
pub fn add_molding(cx: &mut EditorContext, polyline: Vec<Point>, profile: MoldingProfile) -> Id {
    let id = cx.project.alloc_id();
    let ceiling = cx.floor().ceiling_height;
    let label = if polyline.len() > 2 {
        "Molding Polyline"
    } else {
        "Molding Line"
    };
    edit(cx, label, |l| {
        l.moldings
            .push(MoldingLine::new(id, polyline, profile, ceiling));
    });
    id
}

/// Adds a molding polyline with a library profile (the Molding Line and
/// Molding Polyline tools); returns its id. The molding starts at the height
/// its profile type belongs at (a crown at the ceiling, a base on the floor,
/// anything else on the floor).
pub fn add_molding_with(cx: &mut EditorContext, polyline: Vec<Point>, profile: ProfileDef) -> Id {
    let id = cx.project.alloc_id();
    let ceiling = cx.floor().ceiling_height;
    let label = if polyline.len() > 2 {
        "Molding Polyline"
    } else {
        "Molding Line"
    };
    let bottom = profile.kind.default_bottom(ceiling, profile.height());
    edit(cx, label, |l| {
        l.moldings
            .push(MoldingLine::with_profile(id, polyline, profile, bottom));
    });
    id
}

/// Adds a floor material region; returns its id.
pub fn add_floor_region(cx: &mut EditorContext, outline: Vec<Point>) -> Id {
    let id = cx.project.alloc_id();
    edit(cx, "Floor Material Region", |l| {
        l.regions.push(MaterialRegion::floor(id, outline));
    });
    id
}

/// Adds a wall material region; returns its id.
pub fn add_wall_region(
    cx: &mut EditorContext,
    wall: Id,
    side: Side,
    (u0, u1): (f64, f64),
    (v0, v1): (f64, f64),
) -> Id {
    let id = cx.project.alloc_id();
    edit(cx, "Wall Material Region", |l| {
        l.regions
            .push(MaterialRegion::wall(id, wall, side, u0, u1, v0, v1));
    });
    id
}

/// The hatch of a wall, adding the default one when it has none; returns its
/// id and whether it was added.
pub fn wall_hatch(cx: &mut EditorContext, wall: Id) -> (Id, bool) {
    if let Some(h) = load(cx).hatches.iter().find(|h| h.wall_id == wall) {
        return (h.id, false);
    }
    let id = cx.project.alloc_id();
    edit(cx, "Wall Hatching", |l| {
        l.hatches.push(WallHatch {
            id,
            wall_id: wall,
            ..WallHatch::default()
        });
    });
    (id, true)
}

/// Adds a polygon deck; returns its id.
pub fn add_deck(cx: &mut EditorContext, outline: Vec<Point>) -> Id {
    let id = cx.project.alloc_id();
    edit(cx, "Polygon Shaped Deck", |l| {
        l.decks.push(DeckPolygon::new(id, outline));
    });
    id
}

/// Adds a 3D solid; returns its id.
pub fn add_solid(cx: &mut EditorContext, kind: SolidKind, position: Point) -> Id {
    let id = cx.project.alloc_id();
    let label = match &kind {
        SolidKind::PolylineSolid { .. } => "3D Solid",
        k => k.name(),
    };
    edit(cx, label, |l| {
        l.solids.push(Solid3d::new(id, kind, position))
    });
    id
}

/// Deletes the object as one undo step; returns whether it existed.
pub fn delete(cx: &mut EditorContext, r: DetailRef) -> bool {
    if !exists(cx, r) {
        return false;
    }
    edit(cx, &format!("Delete {}", r.name()), |l| {
        l.remove(r);
    });
    forget(cx, &[r.id()]);
    true
}

/// Moves the object by `d` as one undo step; returns whether it existed.
pub fn move_by(cx: &mut EditorContext, r: DetailRef, d: Point) -> bool {
    if !exists(cx, r) {
        return false;
    }
    edit(cx, &format!("Move {}", r.name()), |l| {
        l.translate(r, d);
    });
    true
}

/// Deletes the objects with these ids as one undo step; returns how many went.
pub fn delete_ids(cx: &mut EditorContext, ids: &[Id]) -> usize {
    let layer = load(cx);
    let refs: Vec<DetailRef> = ids.iter().filter_map(|i| layer.find(*i)).collect();
    if refs.is_empty() {
        return 0;
    }
    let label = match refs.as_slice() {
        [one] => format!("Delete {}", one.name()),
        _ => "Delete Details".to_string(),
    };
    edit(cx, &label, |l| {
        for r in &refs {
            l.remove(*r);
        }
    });
    forget(cx, ids);
    refs.len()
}

/// Deletes the wall material regions and wall hatches whose wall is gone,
/// inside the caller's undo step (deleting walls). Returns how many went.
pub fn drop_orphans(cx: &mut EditorContext) -> usize {
    if cx.floor().details.is_none() {
        return 0;
    }
    let mut layer = load(cx);
    let gone: Vec<Id> = layer
        .regions
        .iter()
        .filter(|r| r.wall_id().is_some_and(|w| cx.floor().wall(w).is_none()))
        .map(|r| r.id)
        .chain(
            layer
                .hatches
                .iter()
                .filter(|h| cx.floor().wall(h.wall_id).is_none())
                .map(|h| h.id),
        )
        .collect();
    if gone.is_empty() {
        return 0;
    }
    let n = layer.drop_orphans(cx.floor());
    let fl = cx.floor;
    save(&mut cx.project, fl, &layer);
    forget(cx, &gone);
    cx.mark_dirty();
    n
}

/// Makes the corner boards and quoins of floor `fi` follow their walls after
/// the walls changed from `before` (the floor's walls as they were): the trim
/// slides to where the faces of its two walls meet now. Wall material regions
/// and hatches are measured along their wall and follow on their own.
/// Returns whether any trim moved.
pub fn follow_walls(project: &mut Project, fi: usize, before: &[Wall]) -> bool {
    let Some(floor) = project.floors.get(fi) else {
        return false;
    };
    if floor.details.is_none() {
        return false;
    }
    let mut layer = DetailsLayer::load(floor);
    if layer.corner_boards.is_empty() && layer.quoins.is_empty() {
        return false;
    }
    let moved = layer.follow_walls(before, &floor.walls);
    // Trim that is not held by Set Top / Set Bottom follows the height of
    // its walls too.
    let taller = before.iter().any(|b| {
        floor
            .wall(b.id)
            .is_some_and(|w| (w.height - b.height).abs() > 1e-9)
    });
    let resized = taller && {
        let rooms = plan_core::detect_rooms(&floor.walls, 0.5);
        layer.refresh_trim_heights(floor, &rooms)
    };
    if !moved && !resized {
        return false;
    }
    save(project, fi, &layer);
    true
}

/// Translates the objects with these ids by `d` inside the caller's undo
/// step (group drags and nudges); returns how many moved.
pub fn translate_ids(cx: &mut EditorContext, ids: &[Id], d: Point) -> usize {
    let mut layer = load(cx);
    let mut n = 0;
    for id in ids {
        if let Some(r) = layer.find(*id) {
            n += usize::from(layer.translate(r, d));
        }
    }
    if n > 0 {
        let fl = cx.floor;
        save(&mut cx.project, fl, &layer);
        cx.mark_dirty();
    }
    n
}

/// Does the active floor have the object?
pub fn exists(cx: &EditorContext, r: DetailRef) -> bool {
    load(cx).find(r.id()) == Some(r)
}

// ===================================================================
// Selection
// ===================================================================

/// The selected detail object, if any: the first selected `Detail` that
/// still exists.
pub fn selected(cx: &EditorContext) -> Option<DetailRef> {
    let layer = load(cx);
    cx.selection.items.iter().find_map(|o| match o {
        ObjectRef::Detail(id) => layer.find(*id),
        _ => None,
    })
}

/// Selects `r` alone.
pub fn select(cx: &mut EditorContext, r: DetailRef) {
    cx.selection.set(ObjectRef::Detail(r.id()));
}

/// Drops the detail objects from the selection.
pub fn clear_selection(cx: &mut EditorContext) {
    cx.selection
        .items
        .retain(|o| !matches!(o, ObjectRef::Detail(_)));
}

/// Drops the objects with these ids from the selection.
fn forget(cx: &mut EditorContext, ids: &[Id]) {
    cx.selection
        .items
        .retain(|o| !matches!(o, ObjectRef::Detail(i) if ids.contains(i)));
    // The layer tables and spec extras of regions and solids that went.
    let fl = cx.floor;
    let layer = DetailsLayer::load(&cx.project.floors[fl]);
    let floor = &mut cx.project.floors[fl];
    floor.drop_region_orphans();
    floor.solid_layer.drop_orphans(&layer);
    floor.prune_blocks(|m| !matches!(m, plan_core::ObjectRef::Detail(i) if ids.contains(&i)));
}

// ===================================================================
// Picking
// ===================================================================

fn near_edge(outline: &[Point], p: Point, tol: f64) -> bool {
    let n = outline.len();
    n >= 2 && (0..n).any(|i| dist_to_segment(p, outline[i], outline[(i + 1) % n]) <= tol)
}

fn near_or_inside(outline: &[Point], p: Point, tol: f64) -> bool {
    outline.len() >= 3 && (point_in_polygon(p, outline) || near_edge(outline, p, tol))
}

fn visible(cx: &EditorContext, layer: &str) -> bool {
    cx.layers().is_visible(layer)
}

/// How a picked object ranks against the rest of the plan: `Above` the walls
/// (trim, moldings, solids), `Wall` objects belong to a wall and rank just
/// after it (wall regions, hatching) and `Below` the rooms (floor regions,
/// decks).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tier {
    Above,
    Wall,
    Below,
}

/// Every object at `p` within `tol` inches, topmost first, with its [`Tier`].
/// Corner trim and moldings first (they are small), then solids, wall regions
/// and hatches, decks and last the floor regions; among shapes the smallest
/// wins.
pub fn pick_all(cx: &EditorContext, p: Point, tol: f64) -> Vec<(DetailRef, Tier)> {
    if cx.floor().details.is_none() {
        return Vec::new();
    }
    let layer = load(cx);
    let floor = cx.floor();
    let mut out = Vec::new();
    for b in &layer.corner_boards {
        if visible(cx, &b.layer) && near_or_inside(&b.outline(), p, tol) {
            out.push((DetailRef::CornerBoard(b.id), Tier::Above));
        }
    }
    for q in &layer.quoins {
        if visible(cx, &q.layer) && near_or_inside(&q.outline(), p, tol) {
            out.push((DetailRef::Quoin(q.id), Tier::Above));
        }
    }
    for m in &layer.moldings {
        let hit = m
            .polyline
            .windows(2)
            .any(|s| dist_to_segment(p, s[0], s[1]) <= tol + m.width);
        if visible(cx, &m.layer) && hit {
            out.push((DetailRef::Molding(m.id), Tier::Above));
        }
    }
    let smallest_first = |mut c: Vec<(DetailRef, f64)>| {
        c.sort_by(|a, b| a.1.total_cmp(&b.1));
        c.into_iter().map(|x| x.0).collect::<Vec<_>>()
    };
    let solids = layer
        .solids
        .iter()
        .filter(|s| visible(cx, &s.layer) && near_or_inside(&s.footprint(), p, tol))
        .map(|s| (DetailRef::Solid(s.id), polygon_area(&s.footprint()).abs()))
        .collect();
    out.extend(smallest_first(solids).into_iter().map(|r| (r, Tier::Above)));
    for r in layer.regions.iter().filter(|r| !r.is_floor()) {
        let hit = r
            .plan_polygon(floor)
            .is_some_and(|poly| near_or_inside(&poly, p, tol));
        if visible(cx, &r.layer) && hit {
            out.push((DetailRef::Region(r.id), Tier::Wall));
        }
    }
    for h in &layer.hatches {
        let hit = floor
            .wall(h.wall_id)
            .is_some_and(|w| near_or_inside(&w.footprint(), p, tol));
        if visible(cx, &h.layer) && hit {
            out.push((DetailRef::Hatch(h.id), Tier::Wall));
        }
    }
    let decks = layer
        .decks
        .iter()
        .filter(|d| visible(cx, &d.layer) && near_or_inside(&d.outline, p, tol))
        .map(|d| (DetailRef::Deck(d.id), d.area()))
        .collect();
    out.extend(smallest_first(decks).into_iter().map(|r| (r, Tier::Below)));
    let regions = layer
        .regions
        .iter()
        .filter(|r| r.is_floor() && visible(cx, &r.layer) && near_or_inside(&r.outline, p, tol))
        .map(|r| (DetailRef::Region(r.id), r.area()))
        .collect();
    out.extend(
        smallest_first(regions)
            .into_iter()
            .map(|r| (r, Tier::Below)),
    );
    out
}

/// The object at `p` within `tol` inches (the first of [`pick_all`]).
pub fn pick(cx: &EditorContext, p: Point, tol: f64) -> Option<DetailRef> {
    pick_all(cx, p, tol).first().map(|x| x.0)
}

/// The corners of the object the Select tool can drag, in plan coordinates
/// (see `DetailsLayer::vertices`).
pub fn vertices(cx: &EditorContext, r: DetailRef) -> Option<Vec<Point>> {
    load(cx).vertices(r)
}

/// Moves corner `i` of `r` in `layer` to `to`; returns whether it moved.
pub fn move_vertex_in(layer: &mut DetailsLayer, r: DetailRef, i: usize, to: Point) -> bool {
    layer.move_vertex(r, i, to)
}

/// Every object whose plan bounds lie inside the box `lo`..`hi`, or, with
/// `crossing`, touch it.
pub fn in_rect(cx: &EditorContext, lo: Point, hi: Point, crossing: bool) -> Vec<DetailRef> {
    let layer = load(cx);
    let floor = cx.floor();
    let mut out = Vec::new();
    let mut check = |r: DetailRef, lay: &str| {
        if !visible(cx, lay) {
            return;
        }
        if let Some((a, b)) = layer.plan_bounds(floor, r) {
            let hit = if crossing {
                a.x <= hi.x && b.x >= lo.x && a.y <= hi.y && b.y >= lo.y
            } else {
                a.x >= lo.x && a.y >= lo.y && b.x <= hi.x && b.y <= hi.y
            };
            if hit {
                out.push(r);
            }
        }
    };
    for x in &layer.corner_boards {
        check(DetailRef::CornerBoard(x.id), &x.layer);
    }
    for x in &layer.quoins {
        check(DetailRef::Quoin(x.id), &x.layer);
    }
    for x in &layer.moldings {
        check(DetailRef::Molding(x.id), &x.layer);
    }
    for x in &layer.regions {
        check(DetailRef::Region(x.id), &x.layer);
    }
    for x in &layer.hatches {
        check(DetailRef::Hatch(x.id), &x.layer);
    }
    for x in &layer.decks {
        check(DetailRef::Deck(x.id), &x.layer);
    }
    for x in &layer.solids {
        check(DetailRef::Solid(x.id), &x.layer);
    }
    out
}

/// A wall under the pointer: where along it and on which side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallHit {
    pub wall: Id,
    /// Distance from the wall start, inches.
    pub u: f64,
    pub side: Side,
    pub length: f64,
    pub height: f64,
}

/// The straight wall at `p` (within its thickness plus `tol`).
pub fn wall_at(cx: &EditorContext, p: Point, tol: f64) -> Option<WallHit> {
    let usable = |w: &Wall| {
        !w.is_curved()
            && !w.flags.invisible
            && w.length() > 1e-6
            && !matches!(w.class, WallClass::RoomDivider | WallClass::Fencing { .. })
            && cx.layers().is_visible(&w.layer)
    };
    cx.floor()
        .walls
        .iter()
        .filter(|w| usable(w))
        .filter_map(|w| {
            let (t, q) = project_on_segment(p, w.start, w.end);
            let d = q.dist(p);
            (d <= w.thickness * 0.5 + tol).then_some((d, w, t, q))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, w, t, q)| WallHit {
            wall: w.id,
            u: t * w.length(),
            side: if (p - q).dot(w.normal()) >= 0.0 {
                Side::Left
            } else {
                Side::Right
            },
            length: w.length(),
            height: w.height,
        })
}

/// The exterior corner near `p`, if any (corner boards and quoins are placed
/// on these).
pub fn corner_at(cx: &mut EditorContext, p: Point, tol: f64) -> Option<ExteriorCorner> {
    let rooms = cx.rooms_now().to_vec();
    corner_near(&exterior_corners(cx.floor(), &rooms), p, tol)
}

// ===================================================================
// Materials and patterns
// ===================================================================

fn library() -> &'static MaterialLibrary {
    static LIB: OnceLock<MaterialLibrary> = OnceLock::new();
    LIB.get_or_init(core_library)
}

/// The names of the library materials, for the dialogs' material combo.
pub fn material_names() -> Vec<String> {
    library().materials.iter().map(|m| m.name.clone()).collect()
}

/// Plan look of a material: its 2D pattern and base color. Names the
/// library does not know match the first material containing the name; the
/// rest draw a plain gray fill.
pub fn material_look(name: &str) -> (Pattern, [u8; 3]) {
    let lib = library();
    let def = lib
        .find(name)
        .or_else(|| lib.search(name).into_iter().next());
    def.map_or((Pattern::None, [180, 180, 180]), |m| {
        (m.pattern.clone(), m.color)
    })
}

type LookCache = (u64, HashMap<String, (Pattern, [u8; 3])>);

thread_local! {
    /// [`material_look_in`]'s answers, kept until the material library changes
    /// (looking a material up copies the whole library).
    static LOOKS: RefCell<LookCache> =
        RefCell::new((u64::MAX, HashMap::new()));
}

/// [`material_look`] for the plan: the user's materials (and their copies of
/// the core ones) count, so a pattern or colour edited in the Material
/// Specification shows in the plan too.
pub fn material_look_in(project: &Project, name: &str) -> (Pattern, [u8; 3]) {
    let revision = crate::tools::materials::library_revision();
    let hit = LOOKS.with(|l| {
        let mut l = l.borrow_mut();
        if l.0 != revision {
            *l = (revision, HashMap::new());
        }
        l.1.get(name).cloned()
    });
    if let Some(hit) = hit {
        return hit;
    }
    let look = crate::tools::materials::hatch_material(project, name)
        .map_or_else(|| material_look(name), |m| (m.pattern, m.color));
    LOOKS.with(|l| l.borrow_mut().1.insert(name.to_string(), look.clone()));
    look
}

/// The strokes that fill a floor region with its material's pattern at the
/// Pattern tab's scale and angle (the user's materials count), clipped to
/// `polygon`; see [`region_strokes`] for the core library alone.
pub fn region_strokes_in(
    project: &Project,
    region: &MaterialRegion,
    polygon: &[Point],
    paper: f64,
) -> Vec<(Point, Point)> {
    crate::tools::materials::hatch_strokes(project, &region.material, polygon, paper)
}

/// The pattern names a wall hatch can use.
pub const PATTERN_NAMES: [&str; 13] = [
    "Lines",
    "Cross Hatch",
    "Brick",
    "Block",
    "Shingle",
    "Lap Siding",
    "Board and Batten",
    "Tile",
    "Herringbone",
    "Insulation",
    "Concrete",
    "Earth",
    "Grass",
];

/// The `plan_materials` pattern called `name`, `scale` times its usual size
/// (line patterns also take `angle` degrees). Unknown names give
/// [`Pattern::None`].
pub fn pattern_named(name: &str, scale: f64, angle: f64) -> Pattern {
    let k = if scale > 0.0 { scale } else { 1.0 };
    match name {
        "Lines" => Pattern::Lines {
            angle_deg: angle,
            spacing: 6.0 * k,
        },
        "Cross Hatch" => Pattern::CrossHatch {
            angle_deg: angle,
            spacing: 6.0 * k,
        },
        "Brick" => Pattern::Brick {
            length: 8.0 * k,
            height: 2.25 * k,
        },
        "Block" => Pattern::Block {
            length: 16.0 * k,
            height: 8.0 * k,
        },
        "Shingle" => Pattern::Shingle {
            exposure: 5.0 * k,
            width: 12.0 * k,
        },
        "Lap Siding" => Pattern::LapSiding { exposure: 6.0 * k },
        "Board and Batten" => Pattern::BoardAndBatten { spacing: 12.0 * k },
        "Tile" => Pattern::Tile {
            w: 12.0 * k,
            h: 12.0 * k,
        },
        "Herringbone" => Pattern::Herringbone {
            length: 12.0 * k,
            width: 3.0 * k,
        },
        "Insulation" => Pattern::Insulation,
        "Concrete" => Pattern::Concrete,
        "Earth" => Pattern::Earth,
        "Grass" => Pattern::Grass,
        _ => Pattern::None,
    }
}

/// The pattern strokes of `pattern` over `polygon`, clipped to it. `paper`
/// is the drawing scale in paper inches per foot (1/4" = 1' is `0.25`).
pub fn strokes_in(pattern: &Pattern, polygon: &[Point], paper: f64) -> Vec<(Point, Point)> {
    if polygon.len() < 3 {
        return Vec::new();
    }
    let (lo, hi) = bounds(polygon);
    let raw = pattern_strokes(pattern, (lo, hi), paper);
    clip_strokes_to_polygon(&raw, polygon)
}

/// The strokes that fill a floor region with its material's pattern, clipped
/// to the region's outline.
pub fn region_strokes(
    region: &MaterialRegion,
    polygon: &[Point],
    paper: f64,
) -> Vec<(Point, Point)> {
    let (pattern, _) = material_look(&region.material);
    strokes_in(&pattern, polygon, paper)
}

/// The polygon a wall's hatching is drawn in: the main layer's outline when
/// the join cache has it, else the wall's drawn outline.
pub fn hatch_polygon(cx: &EditorContext, wall: &Wall) -> Vec<Point> {
    // A curved wall's main layer outline follows the arc, like the straight
    // one follows the wall.
    cx.layer_outlines
        .iter()
        .find(|l| l.wall_id == wall.id && l.is_main)
        .map(|l| l.polygon.clone())
        .or_else(|| {
            cx.outlines
                .iter()
                .filter(|_| !wall.is_curved())
                .find(|o| o.wall_id == wall.id)
                .map(|o| o.polygon.clone())
        })
        .unwrap_or_else(|| wall.plan_polygon())
}

/// The strokes of a wall hatch inside `polygon`.
pub fn hatch_strokes(h: &WallHatch, polygon: &[Point], paper: f64) -> Vec<(Point, Point)> {
    strokes_in(&pattern_named(&h.pattern, h.scale, h.angle), polygon, paper)
}

type Strokes = Rc<Vec<(Point, Point)>>;

thread_local! {
    static STROKES: RefCell<HashMap<u64, Strokes>> = RefCell::new(HashMap::new());
}

/// Pattern strokes cached by what they depend on, so a frame does not clip
/// thousands of segments again.
fn cached_strokes(key: impl Hash, make: impl FnOnce() -> Vec<(Point, Point)>) -> Strokes {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    let k = hasher.finish();
    STROKES.with(|c| {
        let mut map = c.borrow_mut();
        if let Some(s) = map.get(&k) {
            return s.clone();
        }
        if map.len() > 96 {
            map.clear();
        }
        let s = Rc::new(make());
        map.insert(k, s.clone());
        s
    })
}

fn hash_points(pts: &[Point]) -> Vec<(u64, u64)> {
    pts.iter().map(|p| (p.x.to_bits(), p.y.to_bits())).collect()
}

// ===================================================================
// 3D
// ===================================================================

/// Every corner board, quoin, molding, material region, deck and solid of the
/// project as 3D meshes; the 3D view extends its scene with it.
pub fn detail_meshes(project: &Project) -> Vec<plan_3d::Mesh> {
    plan_3d::details::detail_meshes(project)
}

// ===================================================================
// Plan drawing
// ===================================================================

fn sc(cam: &Camera, p: Point) -> Pos2 {
    cam.world_to_screen(p)
}

fn layer_color(cx: &EditorContext, name: &str, fallback: [u8; 3]) -> Color32 {
    let [r, g, b] = cx.layers().get(name).map_or(fallback, |l| l.color);
    Color32::from_rgb(r, g, b)
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// The ink of a detail: its own color (Line Style page), else its layer's.
fn ink_of(cx: &EditorContext, s: &DetailStyle, layer: &str, fallback: [u8; 3]) -> Color32 {
    s.color
        .map(rgb)
        .unwrap_or_else(|| layer_color(cx, layer, fallback))
}

/// Pixel width of a detail's main line: `base` px, or its own plotted weight
/// (1/100 mm) scaled from the 0.18 mm that `base` stands for.
fn width_of(s: &DetailStyle, base: f32) -> f32 {
    s.weight
        .map_or(base, |w| (w as f32 / 18.0 * base).clamp(0.3, 8.0))
}

/// The dash of a detail's main line: its own, else `default`.
fn dash_of(s: &DetailStyle, default: LineStyle) -> LineStyle {
    s.dash.unwrap_or(default)
}

/// A world outline in `stroke`, optionally closed, in the given line style
/// (also the tool's rubber band).
pub fn stroke_line(
    painter: &egui::Painter,
    cam: &Camera,
    pts: &[Point],
    closed: bool,
    stroke: Stroke,
    style: LineStyle,
) {
    if pts.len() < 2 {
        return;
    }
    let mut screen: Vec<Pos2> = pts.iter().map(|p| sc(cam, *p)).collect();
    if closed {
        screen.push(screen[0]);
    }
    match style {
        LineStyle::Solid => {
            painter.add(Shape::line(screen, stroke));
        }
        LineStyle::Dashed => painter.extend(Shape::dashed_line(&screen, stroke, 9.0, 5.0)),
        LineStyle::Dotted => painter.extend(Shape::dashed_line(&screen, stroke, 2.0, 4.0)),
        LineStyle::DashDot => painter.extend(Shape::dashed_line(&screen, stroke, 10.0, 8.0)),
    }
}

/// Fills a simple polygon with a flat color.
fn fill_polygon(painter: &egui::Painter, cam: &Camera, outline: &[Point], color: Color32) {
    if outline.len() < 3 || color.a() == 0 {
        return;
    }
    let mut mesh = egui::Mesh::default();
    for p in outline {
        mesh.colored_vertex(sc(cam, *p), color);
    }
    for [a, b, c] in ear_clip(outline) {
        mesh.add_triangle(a as u32, b as u32, c as u32);
    }
    painter.add(Shape::mesh(mesh));
}

fn draw_strokes(painter: &egui::Painter, cam: &Camera, strokes: &[(Point, Point)], stroke: Stroke) {
    for (a, b) in strokes {
        painter.line_segment([sc(cam, *a), sc(cam, *b)], stroke);
    }
}

fn paper(cx: &EditorContext) -> f64 {
    cx.sheet.scale.inches_per_foot()
}

fn draw_floor_region(
    cx: &EditorContext,
    painter: &egui::Painter,
    cam: &Camera,
    r: &MaterialRegion,
) {
    if r.outline.len() < 3 {
        return;
    }
    let ink = ink_of(cx, &r.style, &r.layer, [110, 110, 140]);
    let (pattern, color) = material_look_in(&cx.project, &r.material);
    fill_polygon(painter, cam, &r.outline, rgb(color).gamma_multiply(0.35));
    if cam.px_per_in >= MIN_HATCH_PX_PER_IN && pattern != Pattern::None {
        let scale = paper(cx);
        let strokes = cached_strokes(
            (
                "floor",
                &r.material,
                hash_points(&r.outline),
                scale.to_bits(),
                crate::tools::materials::library_revision(),
            ),
            || region_strokes_in(&cx.project, r, &r.outline, scale),
        );
        draw_strokes(
            painter,
            cam,
            &strokes,
            Stroke::new(0.75_f32, ink.gamma_multiply(0.8)),
        );
    }
    stroke_line(
        painter,
        cam,
        &r.outline,
        true,
        Stroke::new(width_of(&r.style, 1.2), ink),
        dash_of(&r.style, LineStyle::Solid),
    );
}

fn draw_deck(cx: &EditorContext, painter: &egui::Painter, cam: &Camera, d: &DeckPolygon) {
    if d.outline.len() < 3 {
        return;
    }
    let ink = ink_of(cx, &d.style, &d.layer, [150, 110, 60]);
    fill_polygon(painter, cam, &d.outline, ink.gamma_multiply(0.25));
    if cam.px_per_in >= MIN_HATCH_PX_PER_IN {
        let pattern = Pattern::Lines {
            angle_deg: 0.0,
            spacing: BOARD_PITCH,
        };
        let scale = paper(cx);
        let strokes = cached_strokes(("deck", hash_points(&d.outline), scale.to_bits()), || {
            strokes_in(&pattern, &d.outline, scale)
        });
        draw_strokes(
            painter,
            cam,
            &strokes,
            Stroke::new(0.6_f32, ink.gamma_multiply(0.7)),
        );
    }
    let width = width_of(&d.style, if d.railing { 2.8_f32 } else { 1.6_f32 });
    stroke_line(
        painter,
        cam,
        &d.outline,
        true,
        Stroke::new(width, ink),
        dash_of(&d.style, LineStyle::Solid),
    );
    if d.railing {
        // Posts at the corners.
        for p in &d.outline {
            painter.circle_filled(sc(cam, *p), 2.5, ink);
        }
    }
}

fn draw_solid(cx: &EditorContext, painter: &egui::Painter, cam: &Camera, s: &Solid3d) {
    let ink = ink_of(cx, &s.style, &s.layer, [90, 90, 90]);
    let foot = s.footprint();
    let style = dash_of(
        &s.style,
        match s.kind {
            SolidKind::Face { .. } => LineStyle::Dotted,
            _ => LineStyle::Dashed,
        },
    );
    fill_polygon(painter, cam, &foot, ink.gamma_multiply(0.12));
    stroke_line(
        painter,
        cam,
        &foot,
        true,
        Stroke::new(width_of(&s.style, 1.3), ink),
        style,
    );
    // The apex or center of round and pointed solids.
    let mark = matches!(
        s.kind,
        SolidKind::Sphere { .. } | SolidKind::Cone { .. } | SolidKind::Cylinder { .. }
    );
    if mark {
        let c = sc(cam, s.position);
        painter.line_segment(
            [c - egui::vec2(4.0, 0.0), c + egui::vec2(4.0, 0.0)],
            Stroke::new(1.0_f32, ink),
        );
        painter.line_segment(
            [c - egui::vec2(0.0, 4.0), c + egui::vec2(0.0, 4.0)],
            Stroke::new(1.0_f32, ink),
        );
    }
}

/// Floor material regions, decks and 3D solids: drawn under the walls.
pub fn draw_under(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let layer = load(cx);
    if layer.is_empty() {
        return;
    }
    for r in layer.regions.iter().filter(|r| r.is_floor()) {
        if visible(cx, &r.layer) {
            draw_floor_region(cx, painter, cam, r);
        }
    }
    for d in &layer.decks {
        if visible(cx, &d.layer) {
            draw_deck(cx, painter, cam, d);
        }
    }
    for s in &layer.solids {
        if visible(cx, &s.layer) {
            draw_solid(cx, painter, cam, s);
        }
    }
}

fn draw_wall_region(
    cx: &EditorContext,
    painter: &egui::Painter,
    cam: &Camera,
    r: &MaterialRegion,
    wall: &Wall,
) {
    let Some((u0, u1, _, _)) = r.uv_bounds() else {
        return;
    };
    let strip = wall_strip(wall, u0, u1);
    let ink = ink_of(cx, &r.style, &r.layer, [110, 110, 140]);
    let (pattern, color) = material_look_in(&cx.project, &r.material);
    fill_polygon(painter, cam, &strip, rgb(color).gamma_multiply(0.5));
    if cam.px_per_in >= MIN_HATCH_PX_PER_IN && pattern != Pattern::None {
        let scale = paper(cx);
        let strokes = cached_strokes(
            (
                "wall",
                &r.material,
                hash_points(&strip),
                scale.to_bits(),
                crate::tools::materials::library_revision(),
            ),
            || crate::tools::materials::hatch_strokes(&cx.project, &r.material, &strip, scale),
        );
        draw_strokes(painter, cam, &strokes, Stroke::new(0.75_f32, ink));
    }
    stroke_line(
        painter,
        cam,
        &strip,
        true,
        Stroke::new(width_of(&r.style, 1.0), ink),
        dash_of(&r.style, LineStyle::Solid),
    );
}

fn draw_hatch(
    cx: &EditorContext,
    painter: &egui::Painter,
    cam: &Camera,
    h: &WallHatch,
    wall: &Wall,
) {
    if cam.px_per_in < MIN_HATCH_PX_PER_IN {
        return;
    }
    let poly = hatch_polygon(cx, wall);
    let ink = ink_of(cx, &h.style, &h.layer, [110, 110, 140]);
    let scale = paper(cx);
    let strokes = cached_strokes(
        (
            "hatch",
            &h.pattern,
            h.scale.to_bits(),
            h.angle.to_bits(),
            hash_points(&poly),
            scale.to_bits(),
        ),
        || hatch_strokes(h, &poly, scale),
    );
    draw_strokes(
        painter,
        cam,
        &strokes,
        Stroke::new(width_of(&h.style, 0.75), ink),
    );
}

fn draw_corner_board(cx: &EditorContext, painter: &egui::Painter, cam: &Camera, b: &CornerBoard) {
    let ink = ink_of(cx, &b.style, &b.layer, [150, 110, 70]);
    let outline = b.outline();
    fill_polygon(painter, cam, &outline, Color32::from_rgb(250, 248, 240));
    stroke_line(
        painter,
        cam,
        &outline,
        true,
        Stroke::new(width_of(&b.style, 1.2), ink),
        dash_of(&b.style, LineStyle::Solid),
    );
}

fn draw_quoin(cx: &EditorContext, painter: &egui::Painter, cam: &Camera, q: &Quoin) {
    let ink = ink_of(cx, &q.style, &q.layer, [150, 110, 70]);
    let (la, lb) = q.course_lengths(0);
    let top = q.axes.l_polygon(q.built_corner(), la, lb, q.depth);
    fill_polygon(painter, cam, &top, ink.gamma_multiply(0.3));
    stroke_line(
        painter,
        cam,
        &top,
        true,
        Stroke::new(width_of(&q.style, 1.2), ink),
        dash_of(&q.style, LineStyle::Solid),
    );
    // The next course (long and short swap) as ticks past the top block.
    if q.quoin_style() != details::QuoinStyle::Uniform && q.courses() > 1 {
        let (a1, b1) = q.course_lengths(1);
        let (a, b) = (q.axes.dir_a, q.axes.dir_b);
        let (na, nb) = (q.axes.out_a * q.depth, q.axes.out_b * q.depth);
        let tick = Stroke::new(1.0_f32, ink);
        for (len, len0, dir, off) in [(a1, la, a, na), (b1, lb, b, nb)] {
            if len > len0 {
                let corner = q.built_corner();
                let from = corner + dir * len0;
                painter.line_segment([sc(cam, from), sc(cam, from + off)], tick);
                painter.line_segment(
                    [
                        sc(cam, corner + dir * len + off),
                        sc(cam, corner + dir * len),
                    ],
                    tick,
                );
            }
        }
    }
}

/// How far a molding projects from its path, inches (the widest part of its
/// profiles with their offsets).
fn reach_of(m: &MoldingLine) -> f64 {
    m.placed_parts()
        .iter()
        .flat_map(|p| p.section.iter().map(|q| q.x))
        .fold(0.0_f64, f64::max)
        .max(0.0)
}

/// The side the profile lies on: `+1` is left of the drawing direction.
fn side_sign(m: &MoldingLine) -> f64 {
    if m.extrude_inside && m.is_closed() {
        let n = m.polyline.len();
        if polygon_area(&m.polyline[..n - 1]) >= 0.0 {
            1.0
        } else {
            -1.0
        }
    } else if m.side == details::MoldingSide::Right {
        -1.0
    } else {
        1.0
    }
}

/// A molding in plan: the path (the back of the molding) and the front line
/// the projection adds, edge by edge. An edge the molding is not on is a
/// thin dotted line, the selected edge of a selected molding is drawn in the
/// selection color.
fn draw_molding(cx: &EditorContext, painter: &egui::Painter, cam: &Camera, m: &MoldingLine) {
    if m.polyline.len() < 2 {
        return;
    }
    let ink = ink_of(cx, &m.style, &m.layer, [160, 90, 50]);
    let reach = reach_of(m) * side_sign(m);
    let selected_edge = cx
        .selection
        .items
        .contains(&ObjectRef::Detail(m.id))
        .then(|| crate::tools::molding::selected_edge_of(m.id));
    for (i, s) in m.polyline.windows(2).enumerate() {
        if !m.edge_on(i) {
            stroke_line(
                painter,
                cam,
                s,
                false,
                Stroke::new(0.8_f32, ink.gamma_multiply(0.5)),
                LineStyle::Dotted,
            );
            continue;
        }
        let color = if selected_edge == Some(i) {
            cx.palette.selection
        } else {
            ink
        };
        stroke_line(
            painter,
            cam,
            s,
            false,
            Stroke::new(width_of(&m.style, 1.6), color),
            dash_of(&m.style, LineStyle::Solid),
        );
        let n = (s[1] - s[0]).normalized().perp() * reach;
        stroke_line(
            painter,
            cam,
            &[s[0] + n, s[1] + n],
            false,
            Stroke::new(0.9_f32, ink.gamma_multiply(0.8)),
            LineStyle::Dashed,
        );
    }
    if m.show_label {
        let text = crate::tools::molding::label_of(m);
        let mid = Point::lerp(m.polyline[0], m.polyline[m.polyline.len() - 1], 0.5);
        painter.text(
            sc(cam, mid),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(11.0),
            ink,
        );
    }
}

fn draw_selection(cx: &EditorContext, painter: &egui::Painter, cam: &Camera, layer: &DetailsLayer) {
    let refs: Vec<DetailRef> = cx
        .selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Detail(id) => layer.find(*id),
            _ => None,
        })
        .collect();
    let stroke = Stroke::new(2.6_f32, cx.palette.selection);
    for r in refs {
        let Some((lo, hi)) = layer.plan_bounds(cx.floor(), r) else {
            continue;
        };
        let pad = 2.0 / cam.px_per_in.max(1e-6);
        let box_pts = [
            Point::new(lo.x - pad, lo.y - pad),
            Point::new(hi.x + pad, lo.y - pad),
            Point::new(hi.x + pad, hi.y + pad),
            Point::new(lo.x - pad, hi.y + pad),
        ];
        stroke_line(painter, cam, &box_pts, true, stroke, LineStyle::Solid);
    }
}

/// Wall hatching and wall regions, corner boards, quoins and moldings: drawn
/// over the walls (and the selection box of the selected object).
pub fn draw_over(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let layer = load(cx);
    if layer.is_empty() {
        return;
    }
    let floor = cx.floor();
    for r in &layer.regions {
        if let (RegionKind::Wall(id), true) = (r.kind, visible(cx, &r.layer)) {
            if let Some(w) = floor.wall(id).filter(|w| !w.is_curved()) {
                draw_wall_region(cx, painter, cam, r, w);
            }
        }
    }
    for h in &layer.hatches {
        if visible(cx, &h.layer) {
            if let Some(w) = floor.wall(h.wall_id) {
                draw_hatch(cx, painter, cam, h, w);
            }
        }
    }
    for b in &layer.corner_boards {
        if visible(cx, &b.layer) {
            draw_corner_board(cx, painter, cam, b);
        }
    }
    for q in &layer.quoins {
        if visible(cx, &q.layer) {
            draw_quoin(cx, painter, cam, q);
        }
    }
    for m in &layer.moldings {
        if visible(cx, &m.layer) {
            draw_molding(cx, painter, cam, m);
        }
    }
    draw_selection(cx, painter, cam, &layer);
}

/// The ghost of a circle for the round solids' rubber band.
pub fn circle_outline(center: Point, r: f64) -> Vec<Point> {
    circle_points(center, r)
}

/// Re-exported so the tool and dialog share one notion of "the layer names
/// the details use".
pub fn layer_names() -> [&'static str; 5] {
    [
        details::CORNER_TRIM_LAYER,
        details::MOLDING_LAYER,
        details::REGION_LAYER,
        details::DECK_LAYER,
        details::SOLID_LAYER,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::WallKind;

    fn cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    fn box_walls(cx: &mut EditorContext) {
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 180.0),
            Point::new(0.0, 180.0),
        ];
        for i in 0..4 {
            cx.project
                .add_wall(0, c[i], c[(i + 1) % 4], 6.5, 108.0, WallKind::Exterior);
        }
        cx.mark_dirty();
    }

    fn square(x: f64, y: f64, s: f64) -> Vec<Point> {
        vec![
            Point::new(x, y),
            Point::new(x + s, y),
            Point::new(x + s, y + s),
            Point::new(x, y + s),
        ]
    }

    #[test]
    fn adding_stores_the_slot_and_the_layers_it_needs() {
        let mut cx = cx();
        let id = add_deck(&mut cx, square(0.0, 0.0, 96.0));
        let l = load(&cx);
        assert_eq!(l.decks.len(), 1);
        assert_eq!(l.decks[0].id, id);
        assert!(cx.floor().details.is_some());
        for name in layer_names() {
            assert!(cx.project.layers.get(name).is_some(), "{name}");
        }
        assert_eq!(cx.undo().as_deref(), Some("Polygon Shaped Deck"));
        assert!(load(&cx).is_empty());
        assert!(cx.floor().details.is_none());
        assert_eq!(cx.redo().as_deref(), Some("Polygon Shaped Deck"));
        assert_eq!(load(&cx).decks.len(), 1);
    }

    #[test]
    fn auto_placement_is_one_undo_step_and_repeats_add_nothing() {
        let mut cx = cx();
        box_walls(&mut cx);
        assert_eq!(auto_corner_boards(&mut cx), 4);
        assert_eq!(load(&cx).corner_boards.len(), 4);
        assert_eq!(auto_corner_boards(&mut cx), 0);
        assert_eq!(auto_quoins(&mut cx), 4);
        assert_eq!(cx.undo().as_deref(), Some("Auto Place Quoins"));
        assert!(load(&cx).quoins.is_empty());
        assert_eq!(cx.undo().as_deref(), Some("Auto Place Corner Boards"));
        assert!(load(&cx).is_empty());
    }

    #[test]
    fn manual_corners_come_from_the_exterior_walls() {
        let mut cx = cx();
        box_walls(&mut cx);
        let c = corner_at(&mut cx, Point::new(238.0, 3.0), 10.0).expect("a corner");
        let id = add_corner_board(&mut cx, &c);
        assert_eq!(load(&cx).corner_board(id).unwrap().wall_corner, c.apex);
        assert!(corner_at(&mut cx, Point::new(120.0, 90.0), 10.0).is_none());
    }

    #[test]
    fn pick_finds_each_kind_and_move_and_delete_are_undoable() {
        let mut cx = cx();
        let deck = add_deck(&mut cx, square(0.0, 0.0, 96.0));
        let region = add_floor_region(&mut cx, square(200.0, 0.0, 48.0));
        let solid = add_solid(
            &mut cx,
            SolidKind::Sphere { r: 12.0 },
            Point::new(400.0, 0.0),
        );
        let mold = add_molding(
            &mut cx,
            vec![Point::new(0.0, 300.0), Point::new(100.0, 300.0)],
            MoldingProfile::Base,
        );
        assert_eq!(
            pick(&cx, Point::new(48.0, 48.0), 4.0),
            Some(DetailRef::Deck(deck))
        );
        assert_eq!(
            pick(&cx, Point::new(224.0, 24.0), 4.0),
            Some(DetailRef::Region(region))
        );
        assert_eq!(
            pick(&cx, Point::new(402.0, 3.0), 4.0),
            Some(DetailRef::Solid(solid))
        );
        assert_eq!(
            pick(&cx, Point::new(50.0, 301.0), 4.0),
            Some(DetailRef::Molding(mold))
        );
        assert_eq!(pick(&cx, Point::new(900.0, 900.0), 4.0), None);
        // Move.
        assert!(move_by(
            &mut cx,
            DetailRef::Deck(deck),
            Point::new(10.0, 20.0)
        ));
        assert_eq!(
            load(&cx).deck(deck).unwrap().outline[0],
            Point::new(10.0, 20.0)
        );
        assert_eq!(cx.undo().as_deref(), Some("Move Deck"));
        assert_eq!(load(&cx).deck(deck).unwrap().outline[0], Point::ZERO);
        // Delete.
        select(&mut cx, DetailRef::Solid(solid));
        assert_eq!(selected(&cx), Some(DetailRef::Solid(solid)));
        assert_eq!(cx.selection.single(), Some(ObjectRef::Detail(solid)));
        assert!(delete(&mut cx, DetailRef::Solid(solid)));
        assert_eq!(selected(&cx), None);
        assert!(cx.selection.is_empty());
        assert!(load(&cx).solid(solid).is_none());
        assert_eq!(cx.undo().as_deref(), Some("Delete 3D Solid"));
        assert!(load(&cx).solid(solid).is_some());
        // Several at once is one step.
        assert_eq!(delete_ids(&mut cx, &[deck, region, 9999]), 2);
        assert_eq!(cx.undo().as_deref(), Some("Delete Details"));
    }

    #[test]
    fn rect_select_finds_objects_by_bounds() {
        let mut cx = cx();
        let d = add_deck(&mut cx, square(0.0, 0.0, 50.0));
        add_deck(&mut cx, square(500.0, 0.0, 50.0));
        let hit = in_rect(&cx, Point::new(-10.0, -10.0), Point::new(60.0, 60.0), false);
        assert_eq!(hit, vec![DetailRef::Deck(d)]);
        let crossing = in_rect(&cx, Point::new(40.0, 40.0), Point::new(60.0, 60.0), true);
        assert_eq!(crossing, vec![DetailRef::Deck(d)]);
        assert!(in_rect(&cx, Point::new(40.0, 40.0), Point::new(60.0, 60.0), false).is_empty());
    }

    #[test]
    fn walls_are_found_with_their_side_and_distance_along() {
        let mut cx = cx();
        box_walls(&mut cx);
        let hit = wall_at(&cx, Point::new(60.0, 5.0), 4.0).expect("the south wall");
        assert_eq!(hit.side, Side::Left);
        assert!((hit.u - 60.0).abs() < 1e-9);
        assert_eq!((hit.length, hit.height), (240.0, 108.0));
        let outside = wall_at(&cx, Point::new(60.0, -2.0), 4.0).unwrap();
        assert_eq!(outside.side, Side::Right);
        assert!(wall_at(&cx, Point::new(120.0, 90.0), 4.0).is_none());
    }

    #[test]
    fn a_floor_region_fills_with_pattern_strokes_clipped_inside_it() {
        // A triangle, so clipping matters.
        let tri = vec![
            Point::new(0.0, 0.0),
            Point::new(96.0, 0.0),
            Point::new(0.0, 96.0),
        ];
        let r = MaterialRegion {
            material: "Brick – Red".into(),
            ..MaterialRegion::floor(1, tri.clone())
        };
        let strokes = region_strokes(&r, &tri, 0.25);
        assert!(!strokes.is_empty(), "brick has a pattern");
        for (a, b) in &strokes {
            for p in [*a, *b, Point::lerp(*a, *b, 0.5)] {
                assert!(
                    point_in_polygon(p, &tri) || near_edge(&tri, p, 1e-6),
                    "{p:?} is outside the region"
                );
            }
        }
        // Nothing for a material with no pattern.
        let plain = MaterialRegion {
            material: "Glass".into(),
            ..r
        };
        assert!(region_strokes(&plain, &tri, 0.25).is_empty());
    }

    #[test]
    fn wall_hatch_strokes_stay_inside_the_wall_polygon() {
        let mut cx = cx();
        box_walls(&mut cx);
        cx.refresh();
        let wall = cx.floor().walls[0].clone();
        let poly = hatch_polygon(&cx, &wall);
        assert!(poly.len() >= 4);
        for pattern in ["Lines", "Cross Hatch", "Brick", "Insulation"] {
            let h = WallHatch {
                pattern: pattern.into(),
                scale: 0.5,
                ..WallHatch::default()
            };
            let strokes = hatch_strokes(&h, &poly, 0.25);
            assert!(!strokes.is_empty(), "{pattern} drew nothing");
            for (a, b) in &strokes {
                for p in [*a, *b, Point::lerp(*a, *b, 0.5)] {
                    assert!(
                        point_in_polygon(p, &poly) || near_edge(&poly, p, 1e-6),
                        "{pattern}: {p:?} is outside the wall"
                    );
                }
            }
        }
        // The angle turns line patterns.
        let flat = hatch_strokes(
            &WallHatch {
                angle: 0.0,
                ..WallHatch::default()
            },
            &poly,
            0.25,
        );
        assert!(flat.iter().all(|(a, b)| (a.y - b.y).abs() < 1e-6));
        let unknown = WallHatch {
            pattern: "Nonsense".into(),
            ..WallHatch::default()
        };
        assert!(hatch_strokes(&unknown, &poly, 0.25).is_empty());
    }

    #[test]
    fn a_wall_gets_one_hatch() {
        let mut cx = cx();
        box_walls(&mut cx);
        let wall = cx.floor().walls[0].id;
        let (id, added) = wall_hatch(&mut cx, wall);
        assert!(added);
        let (again, added) = wall_hatch(&mut cx, wall);
        assert!(!added);
        assert_eq!(id, again);
        assert_eq!(load(&cx).hatches.len(), 1);
    }

    #[test]
    fn region_polygons_and_looks_resolve() {
        let (p, c) = material_look("Brick – Red");
        assert_ne!(p, Pattern::None);
        assert_ne!(c, [180, 180, 180]);
        // Unknown names fall back to a substring match, then to gray.
        assert_eq!(
            material_look("Fieldstone").1,
            material_look("Stone Veneer – Fieldstone").1
        );
        assert_eq!(material_look("zzz").0, Pattern::None);
        assert!(material_names().len() > 20);
        for n in PATTERN_NAMES {
            assert_ne!(pattern_named(n, 1.0, 45.0), Pattern::None, "{n}");
        }
    }

    #[test]
    fn detail_meshes_reach_the_3d_scene() {
        let mut cx = cx();
        box_walls(&mut cx);
        auto_corner_boards(&mut cx);
        add_solid(
            &mut cx,
            SolidKind::Sphere { r: 12.0 },
            Point::new(500.0, 0.0),
        );
        assert_eq!(detail_meshes(&cx.project).len(), 5);
    }
    #[test]
    fn details_are_selected_through_the_shared_selection() {
        let mut cx = cx();
        let a = add_deck(&mut cx, square(0.0, 0.0, 96.0));
        let b = add_deck(&mut cx, square(300.0, 0.0, 96.0));
        select(&mut cx, DetailRef::Deck(a));
        cx.selection.add(ObjectRef::Detail(b));
        assert_eq!(cx.selection.len(), 2);
        assert!(cx.selection.items.iter().all(|o| o.exists(cx.floor())));
        clear_selection(&mut cx);
        assert!(cx.selection.is_empty());
        // Deleting one drops only it from the selection.
        select(&mut cx, DetailRef::Deck(a));
        cx.selection.add(ObjectRef::Detail(b));
        delete(&mut cx, DetailRef::Deck(a));
        assert_eq!(cx.selection.single(), Some(ObjectRef::Detail(b)));
        // A selected object that vanishes with undo is not "selected".
        cx.undo();
        cx.undo();
        assert_eq!(selected(&cx), None);
    }

    #[test]
    fn pick_all_ranks_trim_above_walls_regions_with_them_and_floor_shapes_below() {
        let mut cx = cx();
        box_walls(&mut cx);
        auto_corner_boards(&mut cx);
        let region = add_floor_region(&mut cx, square(40.0, 40.0, 100.0));
        let deck = add_deck(&mut cx, square(60.0, 60.0, 40.0));
        let wall = cx.floor().walls[0].id;
        let wr = add_wall_region(&mut cx, wall, Side::Left, (60.0, 120.0), (0.0, 96.0));
        // Inside the room: the deck (smaller) beats the region, both Below.
        let hits = pick_all(&cx, Point::new(80.0, 80.0), 2.0);
        assert_eq!(
            hits,
            vec![
                (DetailRef::Deck(deck), Tier::Below),
                (DetailRef::Region(region), Tier::Below)
            ]
        );
        // On the wall: the wall region ranks as a wall object.
        let on_wall = pick_all(&cx, Point::new(90.0, 0.0), 2.0);
        assert_eq!(on_wall, vec![(DetailRef::Region(wr), Tier::Wall)]);
        // At a corner the corner board is above everything.
        let at_corner = pick_all(&cx, Point::new(-3.0, -3.0), 2.0);
        assert!(matches!(
            at_corner[0],
            (DetailRef::CornerBoard(_), Tier::Above)
        ));
        assert_eq!(
            pick(&cx, Point::new(80.0, 80.0), 2.0),
            Some(DetailRef::Deck(deck))
        );
    }

    #[test]
    fn deleting_a_wall_drops_its_regions_and_hatches() {
        let mut cx = cx();
        box_walls(&mut cx);
        let (a, b) = (cx.floor().walls[0].id, cx.floor().walls[1].id);
        add_wall_region(&mut cx, a, Side::Left, (0.0, 60.0), (0.0, 96.0));
        let keep = add_wall_region(&mut cx, b, Side::Left, (0.0, 60.0), (0.0, 96.0));
        wall_hatch(&mut cx, a);
        let floor_region = add_floor_region(&mut cx, square(40.0, 40.0, 100.0));
        select_all(&mut cx);
        assert_eq!(drop_orphans(&mut cx), 0, "nothing orphaned yet");
        cx.selection.set(ObjectRef::Wall(a));
        cx.begin_change("Delete");
        cx.project.remove_wall(0, a);
        assert_eq!(drop_orphans(&mut cx), 2);
        let l = load(&cx);
        assert_eq!(l.regions.len(), 2);
        assert!(l.region(keep).is_some() && l.region(floor_region).is_some());
        assert!(l.hatches.is_empty());
    }

    fn select_all(cx: &mut EditorContext) {
        let ids: Vec<Id> = {
            let l = load(cx);
            l.regions
                .iter()
                .map(|r| r.id)
                .chain(l.hatches.iter().map(|h| h.id))
                .collect()
        };
        cx.selection.items = ids.into_iter().map(ObjectRef::Detail).collect();
    }

    #[test]
    fn trim_follows_a_wall_that_moves() {
        let mut cx = cx();
        box_walls(&mut cx);
        cx.refresh();
        auto_corner_boards(&mut cx);
        auto_quoins(&mut cx);
        let before = cx.floor().walls.clone();
        // The north wall (third) moves 30" south, its neighbours following.
        let ids: Vec<Id> = before.iter().map(|w| w.id).collect();
        for w in &mut cx.project.floors[0].walls {
            if w.id == ids[2] {
                w.start.y = 150.0;
                w.end.y = 150.0;
            } else if w.id == ids[1] {
                w.end.y = 150.0;
            } else if w.id == ids[3] {
                w.start.y = 150.0;
            }
        }
        assert!(follow_walls(&mut cx.project, 0, &before));
        let l = load(&cx);
        let north: Vec<f64> = l
            .corner_boards
            .iter()
            .map(|b| b.wall_corner.y)
            .filter(|y| *y > 100.0)
            .collect();
        assert_eq!(north.len(), 2);
        assert!(
            north.iter().all(|y| (*y - 153.25).abs() < 1e-6),
            "{north:?}"
        );
        assert!(l.quoins.iter().filter(|q| q.corner.y > 100.0).count() == 2);
        // Following again with the walls where they are changes nothing.
        let now = cx.floor().walls.clone();
        assert!(!follow_walls(&mut cx.project, 0, &now));
        // And nothing happens on a floor without details.
        let mut empty = Project::new("e");
        assert!(!follow_walls(&mut empty, 0, &[]));
    }

    #[test]
    fn corners_of_polygons_are_draggable_and_lines_have_two_ends() {
        let mut cx = cx();
        let deck = add_deck(&mut cx, square(0.0, 0.0, 96.0));
        let line = add_molding(
            &mut cx,
            vec![Point::new(0.0, 300.0), Point::new(100.0, 300.0)],
            MoldingProfile::Base,
        );
        assert_eq!(vertices(&cx, DetailRef::Deck(deck)).unwrap().len(), 4);
        assert_eq!(vertices(&cx, DetailRef::Molding(line)).unwrap().len(), 2);
        let mut layer = load(&cx);
        assert!(move_vertex_in(
            &mut layer,
            DetailRef::Deck(deck),
            2,
            Point::new(120.0, 130.0)
        ));
        save(&mut cx.project, 0, &layer);
        assert_eq!(
            load(&cx).deck(deck).unwrap().outline[2],
            Point::new(120.0, 130.0)
        );
    }
}
