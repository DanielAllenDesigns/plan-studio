//! What is selected, and picking objects with the pointer (S-1..S-6).

use plan_core::cad::CadItem;
use plan_core::geometry::{dist_to_segment, project_on_segment, Point};
use plan_core::{CadObject, DimensionKind, Floor, Id, LayerSet, OpeningKind};
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
            | ObjectRef::Text(i) => i,
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
        }
    }

    /// Does the object still exist on `floor`? Kinds the model has no storage
    /// for yet never do.
    pub fn exists(self, floor: &Floor) -> bool {
        match self {
            ObjectRef::Wall(i) => floor.wall(i).is_some(),
            ObjectRef::Opening(i) => floor.openings.iter().any(|o| o.id == i),
            ObjectRef::Dimension(i) => floor.dimensions.iter().any(|d| d.id == i),
            ObjectRef::Cad(i) => floor.cad.iter().any(|c| c.id == i),
            _ => false,
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

    /// Drops objects that no longer exist.
    pub fn retain_existing(&mut self, floor: &Floor) {
        self.items.retain(|o| o.exists(floor));
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
        ObjectRef::Cad(i) => floor
            .cad
            .iter()
            .find(|c| c.id == i)
            .map(|c| c.layer.clone()),
        _ => None,
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

/// Objects under `p`, topmost first: openings, dimensions, CAD (newest
/// first), then walls nearest-first. `tol` is the pick distance in inches.
/// Objects on hidden layers are skipped (S-5); locked ones can be picked.
pub fn hit_test(floor: &Floor, layers: &LayerSet, p: Point, tol: f64) -> Vec<ObjectRef> {
    let mut out = Vec::new();
    let visible = |o: ObjectRef| layer_of(floor, o).is_none_or(|l| layers.is_visible(&l));

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
    out.extend(walls.into_iter().map(|(_, id)| ObjectRef::Wall(id)));
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
