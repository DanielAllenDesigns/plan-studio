//! What is selected, and picking objects with the pointer (S-1..S-6).
//!
//! [`ObjectRef`] is the one object model of the editor: every kind the plan
//! can hold has a variant, and `exists`, `layer_of`, [`hit_test_cx`] and
//! [`extra_in_rect`] ask the view module that owns the kind.

use super::{placed, roof_view, rooms_edit, site_view, stairs_view, EditorContext};
use crate::tools::camera as camera_tool;
use plan_core::cad::CadItem;
use plan_core::geometry::{dist_to_segment, project_on_segment, Point};
use plan_core::{CadObject, DimensionKind, Floor, Id, LayerSet, OpeningKind, Project};
use std::collections::HashSet;
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
            | ObjectRef::Device(i) => i,
            ObjectRef::Room(i) => i as Id,
            ObjectRef::Terrain => 0,
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
            ObjectRef::Camera(_) | ObjectRef::Room(_) | ObjectRef::Terrain => false,
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
            ObjectRef::Device(i) => site_view::electrical_layer(fl, floor).device(i).is_some(),
            other => other.exists(floor),
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

    /// Drops objects that no longer exist on floor `fl` of `project`.
    pub fn retain_existing(&mut self, project: &Project, fl: usize) {
        self.items.retain(|o| o.exists_in(project, fl));
    }
}

/// The layer an object lives on.
pub fn layer_of(floor: &Floor, o: ObjectRef) -> Option<String> {
    match o {
        ObjectRef::Wall(i) => floor.wall(i).map(|w| w.layer.clone()),
        ObjectRef::Opening(i) => floor.openings.iter().find(|x| x.id == i).map(|x| {
            match x.kind {
                OpeningKind::Door => "Doors",
                OpeningKind::Window => "Windows",
            }
            .to_string()
        }),
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
        ObjectRef::RoofPlane(i) => roof_view::load(floor).plane(i).map(|r| r.layer.clone()),
        ObjectRef::Device(_) => Some(site_view::ELECTRICAL_LAYER.to_string()),
        ObjectRef::Camera(_) => Some(camera_tool::CAMERA_LAYER.to_string()),
        ObjectRef::Terrain => Some(site_view::TERRAIN_LAYER.to_string()),
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
        let perp = p.sub(w.start).dot(w.normal()).abs();
        if perp > w.thickness * 0.5 + slop {
            continue;
        }
        let (t, _) = project_on_segment(p, w.start, w.end);
        let along = t * w.length();
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
        let perp = p.sub(w.start).dot(w.normal()).abs();
        if perp > w.thickness * 0.5 + tol * 0.5 {
            continue;
        }
        let (t, _) = project_on_segment(p, w.start, w.end);
        let along = t * w.length();
        for o in floor.openings_on(w.id) {
            let r = ObjectRef::Opening(o.id);
            if along >= o.start_offset() && along <= o.end_offset() && visible(r) {
                out.push(r);
            }
        }
    }
    out
}

/// Dimensions, then CAD (newest first); CAD objects in `skip` (the outlines
/// and records of roof planes) are not pickable as CAD.
fn dim_cad_hits(
    floor: &Floor,
    p: Point,
    tol: f64,
    skip: &HashSet<Id>,
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
        if !skip.contains(&c.id) && cad_distance(&c.item, p) <= tol && visible(r) {
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
    out.extend(dim_cad_hits(floor, p, tol, &HashSet::new(), &visible));
    out.extend(wall_hits(floor, p, tol, &visible));
    out
}

/// CAD objects that are really the storage of a roof plane (its outline and
/// its record); they are picked as [`ObjectRef::RoofPlane`] instead.
fn roof_storage_ids(floor: &Floor) -> HashSet<Id> {
    let mut ids = HashSet::new();
    for r in &roof_view::load(floor).planes {
        ids.insert(r.id);
        if r.outline_id != 0 {
            ids.insert(r.outline_id);
        }
    }
    ids
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
    let skip = roof_storage_ids(floor);
    out.extend(dim_cad_hits(floor, p, tol, &skip, &visible));
    out.extend(wall_hits(floor, p, tol, &visible));

    let roofs = roof_view::load(floor);
    let mut interior = None;
    for r in roofs.planes.iter().rev() {
        let r_ref = ObjectRef::RoofPlane(r.id);
        if !visible(r_ref) {
            continue;
        }
        let poly = r.plan_polygon();
        let n = poly.len();
        let on_edge =
            n >= 2 && (0..n).any(|i| dist_to_segment(p, poly[i], poly[(i + 1) % n]) <= tol);
        if on_edge {
            out.push(r_ref);
        } else if interior.is_none() && r.contains(p) {
            interior = Some(r_ref);
        }
    }
    if layers.is_visible("Rooms") {
        if let Some(i) = rooms_edit::room_index_at(cx, p) {
            out.push(ObjectRef::Room(i));
        }
    }
    out.extend(interior);
    if layers.is_visible(site_view::TERRAIN_LAYER) {
        if let Some(view) = site_view::terrain_view(&cx.project) {
            if site_view::hit_terrain(&view.record.terrain, p, tol).is_some() {
                out.push(ObjectRef::Terrain);
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
    for r in &roof_view::load(floor).planes {
        let o = ObjectRef::RoofPlane(r.id);
        let poly = r.plan_polygon();
        if !poly.is_empty() && usable(o) && hit(&poly) {
            out.push(o);
        }
    }
    for c in cx.project.cameras_on(cx.floor) {
        let r = ObjectRef::Camera(c.id);
        if usable(r) && hit(&[c.position]) {
            out.push(r);
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
    use plan_core::{Project, WallKind};

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
}
