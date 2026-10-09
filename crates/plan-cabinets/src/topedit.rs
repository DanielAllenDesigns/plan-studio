//! Editing a custom countertop's outline and edges (Custom Countertop
//! Specification, reference manual pp. 688 to 690): the polyline figures,
//! the selected line, waterfalls and molding per edge, and turning one top
//! into a hole of another.

use plan_core::geometry::{point_in_polygon, Point};

use crate::cabinet::{Cabinet, CabinetKind};
use crate::geom;
use crate::options::EdgeMolding;
use crate::top::{Cutout, CutoutKind, EdgeProfile};

impl Cabinet {
    fn top_ring_points(&self) -> Option<&[Point]> {
        if self.kind != CabinetKind::CustomCountertop {
            return None;
        }
        self.custom.as_ref().filter(|c| c.closed).map(|c| &c.outline[..])
    }

    /// Number of lines in the outline of a custom countertop (Polyline
    /// panel, Number of Lines); 0 for anything else.
    pub fn top_edge_count(&self) -> usize {
        self.top_ring_points().map_or(0, <[Point]>::len)
    }

    /// Edge `i` of the outline, local frame, from point `i` to point `i + 1`.
    pub fn top_edge(&self, i: usize) -> Option<(Point, Point)> {
        let ring = self.top_ring_points()?;
        let n = ring.len();
        (i < n).then(|| (ring[i], ring[(i + 1) % n]))
    }

    /// Length of edge `i`, inches.
    pub fn top_edge_length(&self, i: usize) -> f64 {
        self.top_edge(i).map_or(0.0, |(a, b)| a.dist(b))
    }

    /// Angle of edge `i` in degrees counter-clockwise from the local +x
    /// axis, in `[0, 360)`.
    pub fn top_edge_angle(&self, i: usize) -> f64 {
        self.top_edge(i).map_or(0.0, |(a, b)| {
            b.sub(a).y.atan2(b.sub(a).x).to_degrees().rem_euclid(360.0)
        })
    }

    /// Perimeter of the outline (Polyline panel, Perimeter), inches.
    pub fn top_perimeter(&self) -> f64 {
        (0..self.top_edge_count()).map(|i| self.top_edge_length(i)).sum()
    }

    /// Area of the top less its holes, square inches (Polyline panel).
    pub fn top_area(&self) -> f64 {
        match self.top_ring_points() {
            Some(ring) => geom::region_area(ring, &self.holes_local()),
            None => 0.0,
        }
    }

    /// Moves outline point `i` to `to` (local frame); the start of one line
    /// and the end of the previous one are the same point. Returns false
    /// when there is no such point or the outline would collapse.
    pub fn move_top_point(&mut self, i: usize, to: Point) -> bool {
        if self.kind != CabinetKind::CustomCountertop {
            return false;
        }
        let Some(c) = self.custom.as_mut().filter(|c| c.closed) else {
            return false;
        };
        if i >= c.outline.len() {
            return false;
        }
        let old = c.outline[i];
        c.outline[i] = to;
        if geom::area(&c.outline).abs() < 1e-6 {
            c.outline[i] = old;
            return false;
        }
        self.refit_top();
        true
    }

    /// Gives edge `i` the length `len`, keeping its start and direction, so
    /// its end (and the start of the next line) moves. Returns false for a
    /// length that is not positive or an unknown edge.
    pub fn set_top_edge_length(&mut self, i: usize, len: f64) -> bool {
        let Some((a, b)) = self.top_edge(i) else {
            return false;
        };
        let cur = a.dist(b);
        if len <= 1e-6 || cur < 1e-9 {
            return false;
        }
        let to = a.add(b.sub(a).scale(len / cur));
        let n = self.top_edge_count();
        self.move_top_point((i + 1) % n, to)
    }

    /// Turns edge `i` to `deg` degrees about its start, keeping its length.
    pub fn set_top_edge_angle(&mut self, i: usize, deg: f64) -> bool {
        let Some((a, b)) = self.top_edge(i) else {
            return false;
        };
        let len = a.dist(b);
        let th = deg.to_radians();
        let to = Point::new(a.x + len * th.cos(), a.y + len * th.sin());
        let n = self.top_edge_count();
        self.move_top_point((i + 1) % n, to)
    }

    /// Re-anchors the cabinet on its outline after a point moved: the
    /// bounding box origin and size follow, and holes and waterfall flags
    /// stay in place.
    fn refit_top(&mut self) {
        let Some(c) = self.custom.as_mut() else { return };
        let Some((lo, hi)) = geom::bbox(&c.outline) else {
            return;
        };
        if lo.x.abs() > 1e-9 || lo.y.abs() > 1e-9 {
            let shift = lo;
            for p in &mut c.outline {
                *p = p.sub(shift);
            }
            for cut in &mut self.cutouts {
                for p in &mut cut.outline {
                    *p = p.sub(shift);
                }
            }
            self.position = self.position.add(rotate(shift, self.angle));
        }
        self.width = hi.x - lo.x;
        self.depth = hi.y - lo.y;
    }

    /// Does edge `i` carry the edge profile? Automatic edges have it (the
    /// walls a top stands against are not known to the cabinet); No Molding
    /// removes it, Has Molding keeps it.
    pub fn top_edge_has_molding(&self, i: usize) -> bool {
        self.top_spec.edge(i).molding != EdgeMolding::NoMolding
    }

    /// Is the profile on at least one edge? With no molding on any edge the
    /// top is built square.
    pub fn top_has_any_molding(&self) -> bool {
        let n = self.top_edge_count();
        n == 0 || (0..n).any(|i| self.top_edge_has_molding(i))
    }

    /// The edge profile the slab is built with: the top's own, or square
    /// when every edge turns the molding off.
    pub fn top_effective_profile(&self) -> (EdgeProfile, f64) {
        match &self.custom {
            Some(c) if self.top_has_any_molding() => (c.edge, c.edge_size),
            _ => (EdgeProfile::Square, 0.0),
        }
    }

    /// Sets the molding of edge `i`, or of every edge (Apply to All Edges).
    pub fn set_top_edge_molding(&mut self, i: usize, molding: EdgeMolding, all: bool) {
        let n = self.top_edge_count();
        if all {
            for k in 0..n {
                self.top_spec.edge_mut(k).molding = molding;
            }
        } else if i < n {
            self.top_spec.edge_mut(i).molding = molding;
        }
    }

    /// Adds or removes a waterfall on edge `i` (Add or Remove Waterfall
    /// from Selected Edge). Returns false for an unknown edge.
    pub fn set_waterfall(&mut self, i: usize, on: bool) -> bool {
        if i >= self.top_edge_count() {
            return false;
        }
        self.top_spec.edge_mut(i).waterfall = on;
        true
    }

    /// The edges that have a waterfall.
    pub fn waterfall_edges(&self) -> Vec<usize> {
        (0..self.top_edge_count())
            .filter(|&i| self.top_spec.edge(i).waterfall)
            .collect()
    }

    /// The vertical slab of a waterfall on edge `i`: a ring in the local
    /// frame (the edge pushed inward by the top's thickness) and the
    /// elevation range it spans relative to the bottom of the horizontal
    /// top (`z` below is negative): from the bottom of the top down to the
    /// floor or by the specified height.
    pub fn waterfall_slab(&self, i: usize) -> Option<(Vec<Point>, f64)> {
        let (a, b) = self.top_edge(i)?;
        let c = self.custom.as_ref()?;
        let t = c.thickness.max(0.05);
        let e = b.sub(a);
        if e.dist(Point::ZERO) < 1e-9 {
            return None;
        }
        // The outline is counter-clockwise, so the inside is to the left.
        let inward = Point::new(-e.y, e.x).normalized().scale(t);
        let ring = vec![a, b, b.add(inward), a.add(inward)];
        let spec = &self.top_spec;
        // The bottom of the horizontal top is at `elevation` above the floor.
        let drop = if spec.waterfall_auto_height {
            self.elevation.max(0.0)
        } else {
            spec.waterfall_height.max(0.0)
        };
        Some((ring, drop))
    }

    /// Turns `hole` (a custom countertop lying inside this one) into a
    /// countertop hole of this top: its outline becomes an opening in this
    /// top's local frame. Both must be closed custom countertops. Returns
    /// false, changing nothing, when they are not or `hole` is not wholly
    /// inside this top.
    pub fn absorb_hole(&mut self, hole: &Cabinet) -> bool {
        let (Some(outer), Some(inner)) = (self.top_polygon(), hole.top_polygon()) else {
            return false;
        };
        if self.kind != CabinetKind::CustomCountertop
            || hole.kind != CabinetKind::CustomCountertop
            || self.custom.as_ref().is_none_or(|c| !c.closed)
            || hole.custom.as_ref().is_none_or(|c| !c.closed)
            || !inner.iter().all(|p| point_in_polygon(*p, &outer))
        {
            return false;
        }
        let outline = geom::ccw(&inner)
            .into_iter()
            .map(|p| to_local(self, p))
            .collect();
        self.cutouts.push(Cutout {
            kind: CutoutKind::Custom,
            name: "Opening".to_string(),
            outline,
        });
        true
    }

    /// The Make Cabinet Molding Polyline path: the closed outline, in plan
    /// coordinates, that the molding `index` of this cabinet follows (the
    /// footprint pushed out by its projection).
    pub fn molding_path(&self, index: usize) -> Option<Vec<Point>> {
        let m = self.moldings.get(index)?;
        let ring = geom::offset_ring(&self.footprint_local(), m.projection);
        Some(ring.into_iter().map(|p| self.to_plan(p)).collect())
    }

    /// Takes molding `index` off the cabinet (Make Cabinet Molding
    /// Polyline replaces it with a polyline of its own).
    pub fn take_molding(&mut self, index: usize) -> Option<crate::cabinet::Molding> {
        (index < self.moldings.len()).then(|| self.moldings.remove(index))
    }
}

fn rotate(p: Point, angle: f64) -> Point {
    let (s, c) = angle.sin_cos();
    Point::new(p.x * c - p.y * s, p.x * s + p.y * c)
}

fn to_local(c: &Cabinet, p: Point) -> Point {
    let d = p.sub(c.position);
    let (s, co) = c.angle.sin_cos();
    Point::new(d.x * co + d.y * s, -d.x * s + d.y * co)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(at: Point, side: f64) -> Cabinet {
        let ring = [
            at,
            Point::new(at.x + side, at.y),
            Point::new(at.x + side, at.y + side),
            Point::new(at.x, at.y + side),
        ];
        Cabinet::custom_countertop(&ring, 1.5, 36.0).unwrap()
    }

    #[test]
    fn the_polyline_panel_figures() {
        let t = square(Point::ZERO, 48.0);
        assert_eq!(t.top_edge_count(), 4);
        assert!((t.top_perimeter() - 192.0).abs() < 1e-9);
        assert!((t.top_area() - 2304.0).abs() < 1e-6);
        assert!((t.top_edge_length(2) - 48.0).abs() < 1e-9);
        assert!((t.top_edge_angle(0)).abs() < 1e-9);
        assert!((t.top_edge_angle(1) - 90.0).abs() < 1e-9);
        // A hole comes off the area.
        let mut holed = t.clone();
        holed.cutouts.push(Cutout::rect(CutoutKind::Custom, "Opening", Point::new(24.0, 24.0), 10.0, 10.0));
        assert!((holed.top_area() - 2204.0).abs() < 1e-6);
        assert_eq!(Cabinet::base(24.0).top_edge_count(), 0);
    }

    #[test]
    fn the_selected_line_changes_length_and_angle() {
        let mut t = square(Point::ZERO, 48.0);
        assert!(t.set_top_edge_length(0, 60.0));
        assert!((t.top_edge_length(0) - 60.0).abs() < 1e-9);
        // The next line starts where this one now ends.
        let (_, end0) = t.top_edge(0).unwrap();
        let (start1, _) = t.top_edge(1).unwrap();
        assert!(end0.dist(start1) < 1e-9);
        assert!((t.width - 60.0).abs() < 1e-9);
        assert!(!t.set_top_edge_length(0, 0.0));
        assert!(t.set_top_edge_angle(0, 10.0));
        assert!((t.top_edge_angle(0) - 10.0).abs() < 1e-6);
        assert!(!t.set_top_edge_length(9, 5.0));
    }

    #[test]
    fn waterfall_flags_and_the_end_panel_reach_the_floor() {
        let mut t = square(Point::ZERO, 48.0);
        assert!(t.waterfall_edges().is_empty());
        assert!(t.set_waterfall(1, true));
        assert_eq!(t.waterfall_edges(), vec![1]);
        let (ring, drop) = t.waterfall_slab(1).unwrap();
        // Automatic height: down to the floor from the bottom of the top.
        assert!((drop - 34.5).abs() < 1e-9, "{drop}");
        // The panel is as thick as the top and sits inside the edge.
        let w = geom::bbox(&ring).map(|(lo, hi)| (hi.x - lo.x, hi.y - lo.y)).unwrap();
        assert!((w.0 - 1.5).abs() < 1e-9 && (w.1 - 48.0).abs() < 1e-9, "{w:?}");
        assert!(ring.iter().all(|p| p.x <= 48.0 + 1e-9 && p.x >= 46.5 - 1e-9));
        t.top_spec.waterfall_auto_height = false;
        t.top_spec.waterfall_height = 20.0;
        assert!((t.waterfall_slab(1).unwrap().1 - 20.0).abs() < 1e-9);
        assert!(t.set_waterfall(1, false));
        assert!(t.waterfall_edges().is_empty());
        assert!(!t.set_waterfall(7, true));
    }

    #[test]
    fn molding_per_edge_and_apply_to_all() {
        let mut t = square(Point::ZERO, 48.0);
        t.custom.as_mut().unwrap().edge = EdgeProfile::Bullnose;
        assert!(t.top_has_any_molding());
        assert_eq!(t.top_effective_profile().0, EdgeProfile::Bullnose);
        t.set_top_edge_molding(2, EdgeMolding::NoMolding, false);
        assert!(!t.top_edge_has_molding(2) && t.top_edge_has_molding(0));
        t.set_top_edge_molding(0, EdgeMolding::NoMolding, true);
        assert!(!t.top_has_any_molding());
        assert_eq!(t.top_effective_profile().0, EdgeProfile::Square);
        t.set_top_edge_molding(0, EdgeMolding::HasMolding, true);
        assert_eq!(t.top_effective_profile().0, EdgeProfile::Bullnose);
    }

    #[test]
    fn a_top_inside_another_becomes_its_hole() {
        let mut outer = square(Point::ZERO, 96.0);
        let inner = square(Point::new(30.0, 30.0), 24.0);
        assert!(outer.absorb_hole(&inner));
        assert_eq!(outer.cutouts.len(), 1);
        assert!((outer.top_area() - (96.0 * 96.0 - 576.0)).abs() < 1e-6);
        // One that sticks out is refused.
        let mut outer2 = square(Point::ZERO, 96.0);
        let partly = square(Point::new(80.0, 80.0), 24.0);
        assert!(!outer2.absorb_hole(&partly));
        assert!(outer2.cutouts.is_empty());
        assert!(!Cabinet::base(24.0).absorb_hole(&inner));
    }

    #[test]
    fn a_cabinet_molding_has_a_path_and_can_be_taken() {
        let mut w = Cabinet::wall(36.0);
        w.moldings.push(crate::cabinet::Molding::crown());
        let path = w.molding_path(0).unwrap();
        assert!(path.len() >= 4);
        assert!(w.molding_path(3).is_none());
        let m = w.take_molding(0).unwrap();
        assert_eq!(m.kind, crate::cabinet::MoldingKind::Crown);
        assert!(w.moldings.is_empty());
        assert!(w.take_molding(0).is_none());
    }
}
