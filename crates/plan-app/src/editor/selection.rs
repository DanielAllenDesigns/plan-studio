//! What is selected, and picking objects with the pointer (S-1..S-6).
//!
//! [`ObjectRef`] is the one object model of the editor: every kind the plan
//! can hold has a variant, and `exists`, `layer_of`, [`hit_test_cx`] and
//! [`extra_in_rect`] ask the view module that owns the kind.

use super::{
    details_view, foundation_view, framing_view, placed, roof_view, rooms_edit, schedule_view,
    site_view, stairs_view, EditorContext,
};
use crate::tools::camera as camera_tool;
use plan_core::cad::CadItem;
use plan_core::details::DetailsLayer;
use plan_core::foundation::FoundationLayer;
use plan_core::geometry::{dist_to_segment, point_in_polygon, Point};
use plan_core::{CadObject, DimensionKind, Floor, Id, LayerSet, Project};
use std::f64::consts::TAU;

/// Addresses one object of the plan.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum ObjectRef {
    Wall(Id),
    Opening(Id),
    Dimension(Id),
    Cad(Id),
    Cabinet(Id),
    Stair(Id),
    /// A roof record: a roof plane, a ceiling plane or a dormer
    /// (`roof_view::RoofSet::kind_of` tells which).
    RoofPlane(Id),
    Symbol(Id),
    Camera(Id),
    Text(Id),
    /// An electrical device (`site_view`).
    Device(Id),
    /// Index into the detected rooms of the active floor (`EditorContext::rooms`).
    /// Room selection itself lives in `rooms_edit`; this ref addresses a room
    /// for hit tests and `EditorRequest::OpenSpec`.
    Room(usize),
    /// The project's terrain (one per plan).
    Terrain,
    /// One element of the terrain: a break, wall, curb, feature, road,
    /// landscape object, elevation point, line, region or modifier (the
    /// perimeter is the whole [`ObjectRef::Terrain`]). Addressed by index
    /// into the terrain record, so it is only valid until the next edit
    /// that removes terrain elements.
    TerrainObject(site_view::TerrainHit),
    /// A slab, slab hole, pad, pier or platform hole
    /// (`FoundationLayer::find(id)` tells which).
    Foundation(Id),
    /// A manually placed framing object: a member or a layout line
    /// (`framing_view::Record`).
    Framing(Id),
    /// A corner board, quoin, molding, material region, wall hatch, deck or
    /// 3D solid (`DetailsLayer::find(id)` tells which).
    Detail(Id),
    /// A schedule table placed on the plan (`schedule_view`).
    Schedule(Id),
    /// An architectural block (`Floor::blocks`). A click on a block member
    /// selects the members; this addresses the block itself for its
    /// specification and the Materials List.
    Block(Id),
    /// A compound 3D solid, the result of Union, Subtract or Intersect
    /// (`Floor::solid_layer`).
    Solid(Id),
}

impl ObjectRef {
    pub fn id(self) -> Id {
        match self {
            ObjectRef::Wall(i)
            | ObjectRef::Opening(i)
            | ObjectRef::Dimension(i)
            | ObjectRef::Cad(i)
            | ObjectRef::Cabinet(i)
            | ObjectRef::Stair(i)
            | ObjectRef::RoofPlane(i)
            | ObjectRef::Symbol(i)
            | ObjectRef::Camera(i)
            | ObjectRef::Text(i)
            | ObjectRef::Device(i)
            | ObjectRef::Foundation(i)
            | ObjectRef::Framing(i)
            | ObjectRef::Detail(i)
            | ObjectRef::Block(i)
            | ObjectRef::Solid(i)
            | ObjectRef::Schedule(i) => i,
            ObjectRef::Room(i) => i as Id,
            ObjectRef::Terrain => 0,
            // The id the object's 3D meshes carry (0: no mesh).
            ObjectRef::TerrainObject(h) => site_view::hit_mesh_id(h),
        }
    }

    /// Chief's name for the object type.
    pub fn type_name(self) -> &'static str {
        match self {
            ObjectRef::Wall(_) => "Wall",
            ObjectRef::Opening(_) => "Opening",
            ObjectRef::Dimension(_) => "Dimension",
            ObjectRef::Cad(_) => "CAD Object",
            ObjectRef::Cabinet(_) => "Cabinet",
            ObjectRef::Stair(_) => "Stairs",
            ObjectRef::RoofPlane(_) => "Roof Plane",
            ObjectRef::Symbol(_) => "Symbol",
            ObjectRef::Camera(_) => "Camera",
            ObjectRef::Text(_) => "Text",
            ObjectRef::Device(_) => "Electrical Device",
            ObjectRef::Room(_) => "Room",
            ObjectRef::Terrain => "Terrain",
            ObjectRef::TerrainObject(h) => site_view::hit_type_name(None, h),
            ObjectRef::Foundation(_) => "Foundation Object",
            ObjectRef::Framing(_) => "Framing Object",
            ObjectRef::Detail(_) => "Detail Object",
            ObjectRef::Schedule(_) => "Schedule",
            ObjectRef::Block(_) => "Architectural Block",
            ObjectRef::Solid(_) => "3D Solid",
        }
    }

    /// Does the object still exist on `floor`? Only kinds stored on the
    /// floor itself can answer; cameras, rooms and terrain need
    /// [`exists_in`](Self::exists_in) and are `false` here.
    pub fn exists(self, floor: &Floor) -> bool {
        match self {
            ObjectRef::Wall(i) => floor.wall(i).is_some(),
            ObjectRef::Opening(i) => floor.openings.iter().any(|o| o.id == i),
            ObjectRef::Dimension(i) => floor.dimensions.iter().any(|d| d.id == i),
            ObjectRef::Cad(i) | ObjectRef::Text(i) => floor.cad.iter().any(|c| c.id == i),
            ObjectRef::Cabinet(i) => placed::exists(floor, placed::PlacedRef::Cabinet(i)),
            ObjectRef::Symbol(i) => placed::exists(floor, placed::PlacedRef::Symbol(i)),
            ObjectRef::Stair(i) => stairs_view::exists(floor, i),
            ObjectRef::RoofPlane(i) => roof_view::exists(floor, i),
            ObjectRef::Device(i) => site_view::load_electrical(floor).device(i).is_some(),
            ObjectRef::Foundation(i) => FoundationLayer::load(floor).find(i).is_some(),
            ObjectRef::Framing(i) => framing_view::find(floor, i).is_some(),
            ObjectRef::Detail(i) => DetailsLayer::load(floor).find(i).is_some(),
            ObjectRef::Schedule(i) => schedule_view::exists(floor, i),
            ObjectRef::Block(i) => floor.blocks.get(i).is_some(),
            ObjectRef::Solid(i) => floor.solid_layer.compound(i).is_some(),
            ObjectRef::Camera(_)
            | ObjectRef::Room(_)
            | ObjectRef::Terrain
            | ObjectRef::TerrainObject(_) => false,
        }
    }

    /// Does the object still exist on floor `fl` of `project`? Rooms never
    /// do: they are not kept in a [`Selection`].
    pub fn exists_in(self, project: &Project, fl: usize) -> bool {
        let Some(floor) = project.floors.get(fl) else {
            return false;
        };
        match self {
            ObjectRef::Camera(i) => project.camera(i).is_some_and(|c| c.floor == fl),
            ObjectRef::Terrain => site_view::load_terrain(project).is_some(),
            ObjectRef::TerrainObject(h) => site_view::terrain_view(project)
                .is_some_and(|v| site_view::hit_exists(&v.record.terrain, h)),
            ObjectRef::Device(i) => site_view::electrical_layer(fl, floor).device(i).is_some(),
            other => other.exists(floor),
        }
    }
}

impl ObjectRef {
    /// The reference as a group member (`plan_core::groups`); rooms and
    /// terrain cannot be grouped.
    pub fn to_group_ref(self) -> Option<plan_core::ObjectRef> {
        use plan_core::ObjectRef as G;
        Some(match self {
            ObjectRef::Wall(i) => G::Wall(i),
            ObjectRef::Opening(i) => G::Opening(i),
            ObjectRef::Dimension(i) => G::Dimension(i),
            ObjectRef::Cad(i) | ObjectRef::Text(i) => G::Cad(i),
            ObjectRef::Symbol(i) => G::Symbol(i),
            ObjectRef::Camera(i) => G::Camera(i),
            ObjectRef::Cabinet(i) => G::Cabinet(i),
            ObjectRef::Stair(i) => G::Stair(i),
            ObjectRef::Device(i) => G::Device(i),
            ObjectRef::RoofPlane(i) => G::RoofPlane(i),
            ObjectRef::Foundation(i) => G::Foundation(i),
            ObjectRef::Framing(i) => G::Framing(i),
            ObjectRef::Detail(i) => G::Detail(i),
            ObjectRef::Schedule(i) => G::Schedule(i),
            ObjectRef::Block(i) => G::Block(i),
            ObjectRef::Solid(i) => G::Solid(i),
            ObjectRef::Room(_) | ObjectRef::Terrain | ObjectRef::TerrainObject(_) => return None,
        })
    }

    /// The editor reference of a group member.
    pub fn from_group_ref(r: plan_core::ObjectRef) -> ObjectRef {
        use plan_core::ObjectRef as G;
        match r {
            G::Wall(i) => ObjectRef::Wall(i),
            G::Opening(i) => ObjectRef::Opening(i),
            G::Dimension(i) => ObjectRef::Dimension(i),
            G::Cad(i) => ObjectRef::Cad(i),
            G::Symbol(i) => ObjectRef::Symbol(i),
            G::Camera(i) => ObjectRef::Camera(i),
            G::Cabinet(i) => ObjectRef::Cabinet(i),
            G::Stair(i) => ObjectRef::Stair(i),
            G::Device(i) => ObjectRef::Device(i),
            G::RoofPlane(i) => ObjectRef::RoofPlane(i),
            G::Foundation(i) => ObjectRef::Foundation(i),
            G::Framing(i) => ObjectRef::Framing(i),
            G::Detail(i) => ObjectRef::Detail(i),
            G::Schedule(i) => ObjectRef::Schedule(i),
            G::Block(i) => ObjectRef::Block(i),
            G::Solid(i) => ObjectRef::Solid(i),
        }
    }

    /// Do both name the same object type (Select Same Type)? CAD lines and
    /// text are one type.
    pub fn same_type(self, other: ObjectRef) -> bool {
        match (self, other) {
            (ObjectRef::Cad(_) | ObjectRef::Text(_), ObjectRef::Cad(_) | ObjectRef::Text(_)) => {
                true
            }
            (a, b) => std::mem::discriminant(&a) == std::mem::discriminant(&b),
        }
    }
}

/// The selected objects, in selection order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selection {
    pub items: Vec<ObjectRef>,
}

impl Selection {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn contains(&self, o: ObjectRef) -> bool {
        self.items.contains(&o)
    }

    pub fn set(&mut self, o: ObjectRef) {
        self.items.clear();
        self.items.push(o);
    }

    pub fn add(&mut self, o: ObjectRef) {
        if !self.contains(o) {
            self.items.push(o);
        }
    }

    pub fn toggle(&mut self, o: ObjectRef) {
        if let Some(i) = self.items.iter().position(|x| *x == o) {
            self.items.remove(i);
        } else {
            self.items.push(o);
        }
    }

    /// The only selected object, if exactly one is selected.
    pub fn single(&self) -> Option<ObjectRef> {
        (self.items.len() == 1).then(|| self.items[0])
    }

    /// Whether two or more objects are selected and every one is a wall.
    pub fn all_walls(&self) -> bool {
        self.items.len() >= 2 && self.items.iter().all(|o| matches!(o, ObjectRef::Wall(_)))
    }

    /// Drops objects that no longer exist on floor `fl` of `project`.
    pub fn retain_existing(&mut self, project: &Project, fl: usize) {
        self.items.retain(|o| o.exists_in(project, fl));
    }
}

/// The layer an object lives on.
pub fn layer_of(floor: &Floor, o: ObjectRef) -> Option<String> {
    match o {
        ObjectRef::Wall(i) => floor.wall(i).map(|w| w.layer.clone()),
        ObjectRef::Opening(i) => floor
            .openings
            .iter()
            .find(|x| x.id == i)
            .map(|x| x.layer_name().to_string()),
        ObjectRef::Dimension(i) => floor.dimensions.iter().find(|d| d.id == i).map(|d| {
            match d.kind {
                DimensionKind::AutoExterior => "Dimensions, Automatic",
                _ => "Dimensions, Manual",
            }
            .to_string()
        }),
        ObjectRef::Cad(i) | ObjectRef::Text(i) => floor
            .cad
            .iter()
            .find(|c| c.id == i)
            .map(|c| c.layer.clone()),
        ObjectRef::Cabinet(i) => {
            placed::cabinet_by_id(floor, i).map(|c| placed::cabinet_layer(c.kind).to_string())
        }
        ObjectRef::Symbol(i) => floor.symbol(i).map(|s| s.layer.clone()),
        ObjectRef::Stair(i) => {
            stairs_view::exists(floor, i).then(|| stairs_view::LAYER.to_string())
        }
        ObjectRef::RoofPlane(i) => roof_view::load(floor).layer_of(i),
        ObjectRef::Foundation(i) => {
            let layer = FoundationLayer::load(floor);
            layer.find(i).and_then(|r| layer.layer_of(r))
        }
        ObjectRef::Framing(i) => framing_view::find(floor, i).map(|r| r.layer().to_string()),
        ObjectRef::Detail(i) => {
            let layer = DetailsLayer::load(floor);
            layer.find(i).and_then(|r| layer.layer_of(r))
        }
        ObjectRef::Schedule(i) => schedule_view::layer_of(floor, i),
        ObjectRef::Block(i) => floor.blocks.get(i).map(|b| b.layer.clone()),
        ObjectRef::Solid(i) => floor.solid_layer.compound(i).map(|c| c.layer.clone()),
        ObjectRef::Device(_) => Some(site_view::ELECTRICAL_LAYER.to_string()),
        ObjectRef::Camera(_) => Some(camera_tool::CAMERA_LAYER.to_string()),
        // The terrain's own layer; an element's layer of its own is read from
        // the terrain record by the pickers (`hit_test_cx`, `extra_in_rect`).
        ObjectRef::Terrain | ObjectRef::TerrainObject(_) => {
            Some(site_view::TERRAIN_LAYER.to_string())
        }
        ObjectRef::Room(_) => None,
    }
}

fn arc_contains(angle: f64, start: f64, end: f64) -> bool {
    let sweep = (end - start).rem_euclid(TAU);
    (angle - start).rem_euclid(TAU) <= sweep
}

/// Distance from `p` to the stroke of a CAD item (0 inside a text box).
pub fn cad_distance(item: &CadItem, p: Point) -> f64 {
    match item {
        CadItem::Line { a, b } => dist_to_segment(p, *a, *b),
        CadItem::Circle { center, radius } => (p.dist(*center) - radius).abs(),
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let a = p.sub(*center).angle();
            if arc_contains(a, *start_angle, *end_angle) {
                (p.dist(*center) - radius).abs()
            } else {
                let at =
                    |t: f64| Point::new(center.x + radius * t.cos(), center.y + radius * t.sin());
                p.dist(at(*start_angle)).min(p.dist(at(*end_angle)))
            }
        }
        CadItem::Polyline { points, closed } => {
            let mut best = f64::INFINITY;
            for pair in points.windows(2) {
                best = best.min(dist_to_segment(p, pair[0], pair[1]));
            }
            if *closed && points.len() > 2 {
                best = best.min(dist_to_segment(p, points[points.len() - 1], points[0]));
            }
            best
        }
        CadItem::Text { .. } => {
            let (lo, hi) = item.bounds();
            let dx = (lo.x - p.x).max(0.0).max(p.x - hi.x);
            let dy = (lo.y - p.y).max(0.0).max(p.y - hi.y);
            dx.hypot(dy)
        }
    }
}

/// The opening under `p`: inside its extent along the wall and the wall
/// thickness (plus `slop`).
pub fn hit_opening(floor: &Floor, p: Point, slop: f64) -> Option<Id> {
    for w in &floor.walls {
        // Along the wall is the arc length on a curved wall (DW-88).
        let (along, lateral) = w.locate(p);
        if lateral.abs() > w.thickness * 0.5 + slop {
            continue;
        }
        if let Some(o) = floor
            .openings_on(w.id)
            .find(|o| along >= o.start_offset() && along <= o.end_offset())
        {
            return Some(o.id);
        }
    }
    None
}

fn open_hits(
    floor: &Floor,
    p: Point,
    tol: f64,
    visible: &dyn Fn(ObjectRef) -> bool,
) -> Vec<ObjectRef> {
    let mut out = Vec::new();
    // Openings win over their host wall (S-4).
    for w in &floor.walls {
        let (along, lateral) = w.locate(p);
        if lateral.abs() > w.thickness * 0.5 + tol * 0.5 {
            continue;
        }
        for o in floor.openings_on(w.id) {
            let r = ObjectRef::Opening(o.id);
            if along >= o.start_offset() && along <= o.end_offset() && visible(r) {
                out.push(r);
            }
        }
    }
    out
}

/// Dimensions, then CAD (newest first).
fn dim_cad_hits(
    floor: &Floor,
    p: Point,
    tol: f64,
    visible: &dyn Fn(ObjectRef) -> bool,
) -> Vec<ObjectRef> {
    let mut out = Vec::new();
    for d in &floor.dimensions {
        let (a, b) = d.line_points();
        let near = dist_to_segment(p, a, b) <= tol
            || d.extension_lines()
                .iter()
                .any(|(s, e)| dist_to_segment(p, *s, *e) <= tol);
        let r = ObjectRef::Dimension(d.id);
        if near && visible(r) {
            out.push(r);
        }
    }
    for c in floor.cad.iter().rev() {
        let r = ObjectRef::Cad(c.id);
        if cad_distance(&c.item, p) <= tol && visible(r) {
            out.push(r);
        }
    }
    out
}

fn wall_hits(
    floor: &Floor,
    p: Point,
    tol: f64,
    visible: &dyn Fn(ObjectRef) -> bool,
) -> Vec<ObjectRef> {
    let mut walls: Vec<(f64, Id)> = floor
        .walls
        .iter()
        .filter_map(|w| {
            let d = dist_to_segment(p, w.start, w.end);
            let reach = tol.max(w.thickness * 0.5 + tol * 0.5);
            (d <= reach && visible(ObjectRef::Wall(w.id))).then_some((d, w.id))
        })
        .collect();
    walls.sort_by(|a, b| a.0.total_cmp(&b.0));
    walls
        .into_iter()
        .map(|(_, id)| ObjectRef::Wall(id))
        .collect()
}

/// Objects under `p`, topmost first: openings, dimensions, CAD (newest
/// first), then walls nearest-first. `tol` is the pick distance in inches.
/// Objects on hidden layers are skipped (S-5); locked ones can be picked.
/// Kinds that need the whole editor state (cabinets, stairs, roofs, devices,
/// cameras, rooms) are found by [`hit_test_cx`].
pub fn hit_test(floor: &Floor, layers: &LayerSet, p: Point, tol: f64) -> Vec<ObjectRef> {
    let visible = |o: ObjectRef| layer_of(floor, o).is_none_or(|l| layers.is_visible(&l));
    let mut out = open_hits(floor, p, tol, &visible);
    out.extend(dim_cad_hits(floor, p, tol, &visible));
    out.extend(wall_hits(floor, p, tol, &visible));
    out
}

/// Every object under `p`, topmost first, over the whole editor model:
/// cameras, devices, openings, symbols and cabinets, stairs, dimensions, CAD,
/// walls, roof plane edges, the room, roof plane interiors and the terrain.
/// Hidden layers are skipped; locked ones can be picked.
pub fn hit_test_cx(cx: &EditorContext, p: Point, tol: f64) -> Vec<ObjectRef> {
    let floor = cx.floor();
    let layers = cx.layers();
    let visible = |o: ObjectRef| layer_of(floor, o).is_none_or(|l| layers.is_visible(&l));
    let mut out = Vec::new();

    if layers.is_visible(camera_tool::CAMERA_LAYER) {
        out.extend(
            cx.project
                .cameras_on(cx.floor)
                .filter(|c| camera_tool::hit_symbol(c, p, tol))
                .map(|c| ObjectRef::Camera(c.id)),
        );
    }
    if layers.is_visible(site_view::ELECTRICAL_LAYER) {
        let layer = site_view::electrical_layer(cx.floor, floor);
        if let Some(id) = site_view::device_at(&layer, p, tol) {
            out.push(ObjectRef::Device(id));
        }
    }
    out.extend(open_hits(floor, p, tol, &visible));
    if let Some(r) = placed::hit_placed(cx, p, tol) {
        out.push(r.object());
    }
    if layers.is_visible(stairs_view::LAYER) {
        if let Some(id) = stairs_view::pick(floor, p, tol) {
            out.push(ObjectRef::Stair(id));
        }
    }
    // Printed-size text is picked by the box it is drawn in at the sheet's scale.
    out.extend(super::render::settle_text_hits(
        cx,
        dim_cad_hits(floor, p, tol, &visible),
        p,
        tol,
    ));
    // A schedule table is an opaque rectangle drawn over the plan.
    if let Some(id) = schedule_view::pick(cx, p) {
        out.push(ObjectRef::Schedule(id));
    }

    // Details: trim, moldings and solids sit above the walls, wall regions
    // and hatching belong to their wall (below it), floor regions and decks
    // lie under the rooms like slabs.
    let details = details_view::pick_all(cx, p, tol);
    let tier = |t: details_view::Tier| {
        details
            .iter()
            .filter(move |(_, x)| *x == t)
            .map(|(r, _)| ObjectRef::Detail(r.id()))
            .collect::<Vec<_>>()
    };
    out.extend(super::solids_view::pick(cx, p, tol));
    out.extend(tier(details_view::Tier::Above));
    out.extend(wall_hits(floor, p, tol, &visible));
    out.extend(tier(details_view::Tier::Wall));

    // Foundation objects: edges (and pads and piers) win over the room; a
    // slab picked only by its interior comes after it.
    let mut slab_interior = None;
    if let Some(r) = foundation_view::pick(cx, p, tol) {
        if foundation_view::picked_by_interior(cx, r, p, tol) {
            slab_interior = Some(ObjectRef::Foundation(r.id()));
        } else {
            out.push(ObjectRef::Foundation(r.id()));
        }
    }

    // Placed framing members and layout lines (the topmost one).
    if let Some(id) = framing_view::pick(floor, p, tol) {
        let r = ObjectRef::Framing(id);
        if visible(r) {
            out.push(r);
        }
    }

    let roofs = roof_view::load(floor);
    let mut interior = None;
    for (id, _, polys) in roofs.pick_polys().iter().rev() {
        let r_ref = ObjectRef::RoofPlane(*id);
        if !visible(r_ref) {
            continue;
        }
        let on_edge = polys.iter().any(|poly| {
            let n = poly.len();
            n >= 2 && (0..n).any(|i| dist_to_segment(p, poly[i], poly[(i + 1) % n]) <= tol)
        });
        if on_edge {
            out.push(r_ref);
        } else if interior.is_none() && polys.iter().any(|poly| point_in_polygon(p, poly)) {
            interior = Some(r_ref);
        }
    }
    if layers.is_visible("Rooms") {
        if let Some(i) = rooms_edit::room_index_at(cx, p) {
            out.push(ObjectRef::Room(i));
        }
    }
    out.extend(interior);
    out.extend(tier(details_view::Tier::Below));
    out.extend(slab_interior);
    if let Some(view) = site_view::terrain_view(&cx.project) {
        let t = &view.record.terrain;
        if let Some(hit) = site_view::hit_terrain(t, p, tol) {
            let layer = site_view::hit_layer(t, hit).unwrap_or_else(|| view.record.layer.clone());
            if layers.is_visible(&layer) {
                out.push(match hit {
                    site_view::TerrainHit::Perimeter => ObjectRef::Terrain,
                    other => ObjectRef::TerrainObject(other),
                });
            }
        }
    }
    out
}

/// Objects of the kinds `objects_in_rect` of the select tool does not know,
/// inside (`crossing` false) or touching (`crossing` true) the rectangle
/// `lo..hi`. Hidden and locked layers are skipped.
pub fn extra_in_rect(cx: &EditorContext, lo: Point, hi: Point, crossing: bool) -> Vec<ObjectRef> {
    let floor = cx.floor();
    let layers = cx.layers();
    let usable = |o: ObjectRef| {
        layer_of(floor, o).is_none_or(|l| layers.is_visible(&l) && !layers.is_locked(&l))
    };
    let hit = |pts: &[Point]| {
        let (mut blo, mut bhi) = (pts[0], pts[0]);
        for q in pts {
            blo = Point::new(blo.x.min(q.x), blo.y.min(q.y));
            bhi = Point::new(bhi.x.max(q.x), bhi.y.max(q.y));
        }
        if crossing {
            blo.x <= hi.x && bhi.x >= lo.x && blo.y <= hi.y && bhi.y >= lo.y
        } else {
            blo.x >= lo.x && bhi.x <= hi.x && blo.y >= lo.y && bhi.y <= hi.y
        }
    };
    let mut out = Vec::new();
    for s in stairs_view::load(floor) {
        let r = ObjectRef::Stair(s.id());
        if usable(r) && hit(&s.footprint()) {
            out.push(r);
        }
    }
    for c in placed::load_cabinets(floor) {
        let r = ObjectRef::Cabinet(c.id);
        if usable(r) && hit(&c.corners()) {
            out.push(r);
        }
    }
    for s in &floor.symbols {
        let r = ObjectRef::Symbol(s.id);
        if usable(r) && hit(&s.footprint()) {
            out.push(r);
        }
    }
    for d in &site_view::electrical_layer(cx.floor, floor).devices {
        let r = ObjectRef::Device(d.id);
        if usable(r) && hit(&[d.position]) {
            out.push(r);
        }
    }
    for (id, _, polys) in roof_view::load(floor).pick_polys() {
        let o = ObjectRef::RoofPlane(id);
        let pts: Vec<Point> = polys.into_iter().flatten().collect();
        if !pts.is_empty() && usable(o) && hit(&pts) {
            out.push(o);
        }
    }
    for r in foundation_view::in_rect_or_touching(cx, lo, hi, crossing) {
        let o = ObjectRef::Foundation(r.id());
        if usable(o) {
            out.push(o);
        }
    }
    for rec in framing_view::load_records(floor) {
        let r = ObjectRef::Framing(rec.id());
        let pts = rec.extent();
        if !pts.is_empty() && usable(r) && hit(&pts) {
            out.push(r);
        }
    }
    for r in details_view::in_rect(cx, lo, hi, crossing) {
        let o = ObjectRef::Detail(r.id());
        if usable(o) {
            out.push(o);
        }
    }
    // Compound 3D solids (the results of Union, Subtract and Intersect).
    for o in super::solids_view::in_rect(cx, lo, hi, crossing) {
        if usable(o) {
            out.push(o);
        }
    }
    for c in cx.project.cameras_on(cx.floor) {
        let r = ObjectRef::Camera(c.id);
        if usable(r) && hit(&[c.position]) {
            out.push(r);
        }
    }
    for (id, lo, hi) in schedule_view::extents(cx) {
        let r = ObjectRef::Schedule(id);
        if usable(r) && hit(&[lo, hi]) {
            out.push(r);
        }
    }
    // Terrain elements, each on its own layer.
    if let Some(view) = site_view::terrain_view(&cx.project) {
        let t = &view.record.terrain;
        for h in site_view::all_hits(t) {
            let layer = site_view::hit_layer(t, h).unwrap_or_else(|| view.record.layer.clone());
            if !(layers.is_visible(&layer) && !layers.is_locked(&layer)) {
                continue;
            }
            let pts = site_view::hit_points(t, h);
            if !pts.is_empty() && hit(&pts) {
                out.push(ObjectRef::TerrainObject(h));
            }
        }
    }
    out
}

/// The object a 3D mesh was built from: the floor it is on and its
/// [`ObjectRef`], from the mesh's `object_id` (ids are unique across the
/// project). `None` for ids that belong to no selectable object (floor
/// slabs, roof shells).
pub fn object_for_mesh_id(project: &Project, id: Id) -> Option<(usize, ObjectRef)> {
    let kinds: [fn(Id) -> ObjectRef; 10] = [
        ObjectRef::Opening,
        ObjectRef::Wall,
        ObjectRef::Symbol,
        ObjectRef::Cabinet,
        ObjectRef::Stair,
        ObjectRef::RoofPlane,
        ObjectRef::Foundation,
        ObjectRef::Framing,
        ObjectRef::Detail,
        ObjectRef::Device,
    ];
    (0..project.floors.len()).find_map(|fl| {
        kinds
            .iter()
            .map(|k| k(id))
            .find(|r| r.exists_in(project, fl))
            .map(|r| (fl, r))
    })
}

/// What a click on mesh `id` of the 3D view selects: the floor to show and
/// the object. Plan objects come from [`object_for_mesh_id`]; terrain
/// elements (whose meshes carry `plan_terrain::terrain_object_id`s) belong to
/// no floor, so `floor` is kept.
pub fn pick_for_mesh_id(project: &Project, floor: usize, id: Id) -> Option<(usize, ObjectRef)> {
    if let Some(hit) = site_view::hit_for_mesh_id(id) {
        let r = ObjectRef::TerrainObject(hit);
        return r.exists_in(project, floor).then_some((floor, r));
    }
    object_for_mesh_id(project, id)
}

/// Every object of the active floor that a click could select and an edit
/// could change: on a displayed, unlocked layer (Select All, S-33). Rooms
/// are not objects of the selection.
pub fn all_selectable(cx: &EditorContext) -> Vec<ObjectRef> {
    let floor = cx.floor();
    let layers = cx.layers();
    let usable = |o: ObjectRef| {
        layer_of(floor, o).is_none_or(|l| layers.is_visible(&l) && !layers.is_locked(&l))
    };
    let mut out = Vec::new();
    for w in &floor.walls {
        let r = ObjectRef::Wall(w.id);
        if usable(r) {
            out.push(r);
        }
    }
    for o in &floor.openings {
        let r = ObjectRef::Opening(o.id);
        if usable(r) {
            out.push(r);
        }
    }
    for d in &floor.dimensions {
        let r = ObjectRef::Dimension(d.id);
        if usable(r) {
            out.push(r);
        }
    }
    for c in &floor.cad {
        let r = ObjectRef::Cad(c.id);
        if usable(r) {
            out.push(r);
        }
    }
    let far = 1.0e12;
    out.extend(extra_in_rect(
        cx,
        Point::new(-far, -far),
        Point::new(far, far),
        true,
    ));
    out
}

/// Select All (Cmd+A): everything [`all_selectable`] names becomes the
/// selection. Returns how many objects.
pub fn select_all(cx: &mut EditorContext) -> usize {
    let all = all_selectable(cx);
    rooms_edit::clear_room_selection();
    cx.selection.items = all;
    cx.selection.len()
}

/// Select Same Type: with objects selected, selects every object of the
/// same types on the active floor (displayed, unlocked layers). Returns the
/// new selection size; 0 when nothing was selected.
pub fn select_same_type(cx: &mut EditorContext) -> usize {
    let kinds: Vec<ObjectRef> = cx.selection.items.clone();
    if kinds.is_empty() {
        return 0;
    }
    let same: Vec<ObjectRef> = all_selectable(cx)
        .into_iter()
        .filter(|o| kinds.iter().any(|k| k.same_type(*o)))
        .collect();
    cx.selection.items = same;
    cx.selection.len()
}

/// `items` plus the rest of every group they belong to (a click on a group
/// member selects the group, S-35).
pub fn expand_groups(cx: &EditorContext, items: &[ObjectRef]) -> Vec<ObjectRef> {
    let floor = cx.floor();
    let mut out: Vec<ObjectRef> = Vec::new();
    for o in items {
        // A member of an architectural block selects the whole block.
        let block = crate::tools::arch_block::block_members_of(floor, *o);
        // A segment of a dimension string selects the whole string.
        let strand: Vec<ObjectRef> = match o {
            ObjectRef::Dimension(id) => floor
                .string_members(*id)
                .into_iter()
                .map(ObjectRef::Dimension)
                .collect(),
            _ => Vec::new(),
        };
        let members = match o.to_group_ref() {
            _ if !block.is_empty() => block,
            _ if strand.len() > 1 => strand,
            Some(g) => floor
                .group_members_of(g)
                .into_iter()
                .map(ObjectRef::from_group_ref)
                .collect(),
            None => vec![*o],
        };
        for m in members {
            // A group names the same object under either CAD variant.
            let known = out
                .iter()
                .any(|x| x.to_group_ref() == m.to_group_ref() && m.to_group_ref().is_some())
                || out.contains(&m);
            if !known && (m == *o || m.exists_in(&cx.project, cx.floor)) {
                out.push(m);
            }
        }
    }
    out
}

/// A CAD object by id.
pub fn cad_by_id(floor: &Floor, id: Id) -> Option<&CadObject> {
    floor.cad.iter().find(|c| c.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::{OpeningKind, Project, WallKind};

    fn plan() -> (Project, Id) {
        let mut p = Project::new("t");
        let w = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            109.0,
            WallKind::Exterior,
        );
        p.add_opening(0, w, 120.0, OpeningKind::Door).unwrap();
        (p, w)
    }

    #[test]
    fn openings_beat_their_wall_and_walls_follow() {
        let (p, w) = plan();
        let hits = hit_test(&p.floors[0], &p.layers, Point::new(120.0, 1.0), 4.0);
        assert!(matches!(hits[0], ObjectRef::Opening(_)));
        assert_eq!(hits[1], ObjectRef::Wall(w));
        let hits = hit_test(&p.floors[0], &p.layers, Point::new(30.0, 2.0), 4.0);
        assert_eq!(hits, vec![ObjectRef::Wall(w)]);
        assert!(hit_test(&p.floors[0], &p.layers, Point::new(30.0, 40.0), 4.0).is_empty());
    }

    #[test]
    fn hidden_layers_cannot_be_picked() {
        let (mut p, _) = plan();
        p.layers.set_display("Walls, Normal", false);
        let hits = hit_test(&p.floors[0], &p.layers, Point::new(30.0, 0.0), 4.0);
        assert!(hits.is_empty());
    }

    #[test]
    fn cad_lines_are_picked_by_stroke() {
        let mut p = Project::new("t");
        let c = p.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::new(0.0, 0.0),
                b: Point::new(100.0, 0.0),
            },
        );
        let hits = hit_test(&p.floors[0], &p.layers, Point::new(50.0, 2.0), 3.0);
        assert_eq!(hits, vec![ObjectRef::Cad(c)]);
    }

    #[test]
    fn selection_set_operations() {
        let mut s = Selection::default();
        s.set(ObjectRef::Wall(1));
        s.add(ObjectRef::Wall(2));
        s.add(ObjectRef::Wall(2));
        assert_eq!(s.len(), 2);
        s.toggle(ObjectRef::Wall(1));
        assert_eq!(s.single(), Some(ObjectRef::Wall(2)));
    }

    #[test]
    fn a_mesh_id_resolves_to_its_object_and_floor() {
        let (mut p, w) = plan();
        let door = p.floors[0].openings[0].id;
        p.build_new_floor(false);
        let w2 = p.add_wall(
            1,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        assert_eq!(object_for_mesh_id(&p, w), Some((0, ObjectRef::Wall(w))));
        assert_eq!(
            object_for_mesh_id(&p, door),
            Some((0, ObjectRef::Opening(door)))
        );
        assert_eq!(object_for_mesh_id(&p, w2), Some((1, ObjectRef::Wall(w2))));
        assert_eq!(object_for_mesh_id(&p, 9999), None);
    }

    #[test]
    fn terrain_meshes_resolve_to_terrain_objects_without_changing_the_floor() {
        use plan_terrain::{Landscape, LandscapeKind, ShapeKind, TerrainWall};
        let (mut p, wall) = plan();
        p.build_new_floor(false);
        let mut rec = site_view::TerrainRecord::new();
        rec.terrain.walls.push(TerrainWall::new(
            plan_terrain::WallKind::Wall,
            vec![Point::new(0.0, 500.0), Point::new(300.0, 500.0)],
            false,
        ));
        rec.terrain.landscape.push(Landscape::new(
            LandscapeKind::GardenBed,
            ShapeKind::Polyline,
            vec![
                Point::new(0.0, 700.0),
                Point::new(200.0, 700.0),
                Point::new(200.0, 900.0),
                Point::new(0.0, 900.0),
            ],
        ));
        site_view::save_terrain(&mut p, &rec);
        let meshes = site_view::terrain_feature_meshes(&p);
        let id_of = |hit: site_view::TerrainHit| {
            meshes
                .iter()
                .find(|m| m.object_id == Some(site_view::hit_mesh_id(hit)))
                .and_then(|m| m.object_id)
                .unwrap_or_else(|| panic!("no mesh for {hit:?}"))
        };
        let wall_hit = site_view::TerrainHit::Wall(0);
        let bed_hit = site_view::TerrainHit::Landscape(0);
        // Whatever floor is showing stays showing.
        for floor in [0, 1] {
            assert_eq!(
                pick_for_mesh_id(&p, floor, id_of(wall_hit)),
                Some((floor, ObjectRef::TerrainObject(wall_hit)))
            );
            assert_eq!(
                pick_for_mesh_id(&p, floor, id_of(bed_hit)),
                Some((floor, ObjectRef::TerrainObject(bed_hit)))
            );
        }
        // The object's id is its meshes' id (the 3D view tints by it).
        assert_eq!(ObjectRef::TerrainObject(wall_hit).id(), id_of(wall_hit));
        // A mesh of an element that is gone selects nothing; plan ids still resolve.
        let gone = site_view::hit_mesh_id(site_view::TerrainHit::Wall(5));
        assert_eq!(pick_for_mesh_id(&p, 0, gone), None);
        assert_eq!(
            pick_for_mesh_id(&p, 1, wall),
            Some((0, ObjectRef::Wall(wall)))
        );
        // Elements without a mesh (points, lines, regions) have no id.
        assert_eq!(
            ObjectRef::TerrainObject(site_view::TerrainHit::Point(0)).id(),
            0
        );
    }
}
