//! Edit Wall Intersections (W-135, W-144; manual pp. 388-389, 402): each wall
//! end shows a handle in the middle of every structural layer and the Auto
//! Connect magnet. Dragging a layer handle slides that one layer along the
//! wall so it meets a layer line of the wall it runs into; Reset Wall Layer
//! Intersections puts the layers back where the join rules put them.
//!
//! The slide is stored per wall as a [`LayerJoin`] in `WallSpec::layer_joins`
//! and read by [`crate::joins::wall_layer_outlines`].

use crate::defaults::WallTypeDef;
use crate::geometry::Point;
use crate::joins::{wall_layer_bands, wall_layer_outlines, WallLayerOutline};
use crate::model::{Id, Project, Wall, WallEnd};
use serde::{Deserialize, Serialize};

/// Slides closer to nothing than this are no slide.
const NONE: f64 = 1e-6;
/// The farthest a layer may be slid from its joined position, as a multiple of
/// the wall's thickness (plus a foot).
const REACH_THICKNESSES: f64 = 4.0;

/// One layer of one wall end slid along the wall.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LayerJoin {
    /// The end of the wall (false: start).
    pub at_end: bool,
    /// Index of the layer in the wall's layers, exterior to interior.
    pub layer: usize,
    /// How far the layer reaches past (positive) or stops short of (negative)
    /// its joined position, inches.
    pub shift: f64,
}

/// The slide of layer `layer` at the given end of `w`, zero when none is set.
pub fn layer_shift(w: &Wall, at_end: bool, layer: usize) -> f64 {
    w.spec
        .layer_joins
        .iter()
        .find(|j| j.at_end == at_end && j.layer == layer)
        .map_or(0.0, |j| j.shift)
}

fn is_end(e: WallEnd) -> bool {
    e == WallEnd::End
}

/// The slides at which layer `layer` of wall `id` meets a layer line of a
/// wall touching that end (the magnet positions of the handle), nearest the
/// joined position first. `types` resolve wall type names to layers.
pub fn layer_snap_candidates(
    walls: &[Wall],
    types: &[WallTypeDef],
    id: Id,
    end: WallEnd,
    layer: usize,
    tol: f64,
) -> Vec<f64> {
    let Some(w) = walls.iter().find(|w| w.id == id) else {
        return Vec::new();
    };
    if w.is_curved() || w.length() < NONE {
        return Vec::new();
    }
    let ty_of = |x: &Wall| {
        x.wall_type
            .as_deref()
            .and_then(|n| types.iter().find(|t| t.name == n))
    };
    let bands = wall_layer_bands(w, ty_of(w));
    let Some(band) = bands.get(layer) else {
        return Vec::new();
    };
    let d = w.direction();
    let (p, outward) = match end {
        WallEnd::Start => (w.start, -d),
        WallEnd::End => (w.end, d),
    };
    let mid = p + w.normal() * ((band.outer + band.inner) * 0.5);
    let reach = REACH_THICKNESSES * w.thickness + 12.0;
    let mut out: Vec<f64> = Vec::new();
    for o in walls {
        if o.id == id || o.is_curved() || o.length() < NONE {
            continue;
        }
        // Only walls that touch the end (a corner or a tee).
        if o.closest_point(p).0.dist(p) > tol.max(o.thickness * 0.5 + 0.5) {
            continue;
        }
        let od = o.direction();
        let cross = outward.cross(od);
        if cross.abs() < 1e-3 {
            continue;
        }
        // The other wall's centerline and every face of its layers.
        let mut lats = vec![0.0];
        for ob in wall_layer_bands(o, ty_of(o)) {
            lats.push(ob.outer);
            lats.push(ob.inner);
        }
        for lat in lats {
            let q = o.start + o.normal() * lat;
            // Distance along `outward` from `mid` to the line through q.
            let t = (q - mid).cross(od) / cross;
            if t.abs() <= reach {
                out.push(t);
            }
        }
    }
    out.sort_by(|a, b| a.abs().total_cmp(&b.abs()));
    out.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
    out
}

/// The candidate nearest to `want`, or `want` itself when there is none
/// within `snap` inches.
pub fn snap_slide(candidates: &[f64], want: f64, snap: f64) -> f64 {
    candidates
        .iter()
        .copied()
        .filter(|c| (c - want).abs() <= snap)
        .min_by(|a, b| (a - want).abs().total_cmp(&(b - want).abs()))
        .unwrap_or(want)
}

impl Project {
    /// Slides layer `layer` at `end` of wall `id` by `shift` inches (zero
    /// removes the slide). Returns false when the wall does not exist.
    pub fn set_layer_join(
        &mut self,
        floor: usize,
        id: Id,
        end: WallEnd,
        layer: usize,
        shift: f64,
    ) -> bool {
        let Some(w) = self.floors.get_mut(floor).and_then(|f| f.wall_mut(id)) else {
            return false;
        };
        let at_end = is_end(end);
        let list = &mut w.spec.layer_joins;
        list.retain(|j| !(j.at_end == at_end && j.layer == layer));
        if shift.abs() > NONE {
            list.push(LayerJoin {
                at_end,
                layer,
                shift,
            });
        }
        true
    }

    /// Reset Wall Layer Intersections: every slid layer of the walls goes
    /// back (all walls of the floor when `ids` is `None`). The Auto Connect
    /// locks go too when `locks` is set (the dialog's Reset to Defaults).
    /// Returns how many walls changed.
    pub fn reset_layer_joins(&mut self, floor: usize, ids: Option<&[Id]>, locks: bool) -> usize {
        let Some(f) = self.floors.get_mut(floor) else {
            return 0;
        };
        let mut n = 0;
        for w in &mut f.walls {
            if ids.is_some_and(|l| !l.contains(&w.id)) {
                continue;
            }
            let had = !w.spec.layer_joins.is_empty()
                || (locks && (w.flags.lock_start || w.flags.lock_end));
            w.spec.layer_joins.clear();
            if locks {
                w.flags.lock_start = false;
                w.flags.lock_end = false;
            }
            n += usize::from(had);
        }
        n
    }
}

/// Where the handle of a layer sits in the plan: the middle of the layer at
/// the wall end, pushed by its slide.
pub fn layer_handle(w: &Wall, types: &[WallTypeDef], end: WallEnd, layer: usize) -> Option<Point> {
    let ty = w
        .wall_type
        .as_deref()
        .and_then(|n| types.iter().find(|t| t.name == n));
    let bands = wall_layer_bands(w, ty);
    let band = bands.get(layer)?;
    let d = w.direction();
    let (p, outward) = match end {
        WallEnd::Start => (w.start, -d),
        WallEnd::End => (w.end, d),
    };
    let shift = layer_shift(w, is_end(end), layer);
    Some(p + w.normal() * ((band.outer + band.inner) * 0.5) + outward * shift)
}

/// The plan outline of layer `layer` of wall `id`, joined to the walls around
/// it exactly as the plan draws it (start-left, end-left, end-right,
/// start-right; left = +normal), with Edit Wall Intersections slides applied
/// (`spec.layer_joins`). `None` for an unknown wall or layer, or a curved wall.
pub fn layer_end_polygon(
    walls: &[Wall],
    types: &[WallTypeDef],
    id: Id,
    layer: usize,
    tol: f64,
) -> Option<[Point; 4]> {
    let outlines = wall_layer_outlines(walls, types, tol);
    outline_of(&outlines, id, layer)
}

/// [`layer_end_polygon`] from outlines already computed for the whole floor.
pub fn outline_of(outlines: &[WallLayerOutline], id: Id, layer: usize) -> Option<[Point; 4]> {
    let o = outlines
        .iter()
        .find(|o| o.wall_id == id && o.layer_index == layer)?;
    <[Point; 4]>::try_from(o.polygon.as_slice()).ok()
}

/// Whether the end face of a layer slab is hidden against another wall's
/// layer: the end edge `a`-`b` lies along an edge of some other wall's layer
/// outline (a mitre shares the whole edge; a butt stops on the surface of the
/// layer it meets). Such a face is not drawn, so the two materials meet along
/// one edge with nothing coincident to fight over.
pub fn end_edge_is_shared(outlines: &[WallLayerOutline], id: Id, a: Point, b: Point) -> bool {
    const EDGE: f64 = 1e-4;
    let len = a.dist(b);
    if len <= EDGE {
        return false;
    }
    let on_line = |q: Point, p0: Point, p1: Point| {
        let e = p1 - p0;
        let l = e.length();
        l > EDGE && ((q - p0).cross(e * (1.0 / l))).abs() <= EDGE
    };
    outlines.iter().filter(|o| o.wall_id != id).any(|o| {
        let n = o.polygon.len();
        (0..n).any(|i| {
            let (p0, p1) = (o.polygon[i], o.polygon[(i + 1) % n]);
            let e = p1 - p0;
            if !on_line(a, p0, p1) || !on_line(b, p0, p1) {
                return false;
            }
            // Both ends inside the other edge's span.
            let l = e.length();
            let u = e * (1.0 / l);
            let (ta, tb) = ((a - p0).dot(u), (b - p0).dot(u));
            ta >= -EDGE && ta <= l + EDGE && tb >= -EDGE && tb <= l + EDGE
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defaults::PlanDefaults;
    use crate::joins::wall_layer_outlines;
    use crate::model::WallKind;

    fn corner() -> (Project, Id, Id, Vec<WallTypeDef>) {
        let mut p = Project::new("t");
        let a = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            8.0,
            96.0,
            WallKind::Exterior,
        );
        let b = p.add_wall(
            0,
            Point::new(120.0, 0.0),
            Point::new(120.0, 100.0),
            8.0,
            96.0,
            WallKind::Exterior,
        );
        let types = PlanDefaults::default().wall_types;
        (p, a, b, types)
    }

    #[test]
    fn a_slid_layer_reaches_past_the_join_and_reset_puts_it_back() {
        let (mut p, a, _b, types) = corner();
        let walls = p.floors[0].walls.clone();
        let base = wall_layer_outlines(&walls, &types, 0.5);
        let a_base = base.iter().find(|o| o.wall_id == a).unwrap();

        assert!(p.set_layer_join(0, a, WallEnd::End, 0, 3.0));
        let slid = p.floors[0].walls.clone();
        let out = wall_layer_outlines(&slid, &types, 0.5);
        let o = out
            .iter()
            .find(|o| o.wall_id == a && o.layer_index == 0)
            .unwrap();
        // The corners of that layer on the end side moved 3 in along the wall.
        let moved = o
            .polygon
            .iter()
            .zip(&a_base.polygon)
            .any(|(n, b)| (n.x - b.x - 3.0).abs() < 1e-6);
        assert!(moved, "{:?} vs {:?}", o.polygon, a_base.polygon);
        // Other layers of the wall did not move.
        if let Some(l1) = out.iter().find(|o| o.wall_id == a && o.layer_index == 1) {
            let b1 = base
                .iter()
                .find(|o| o.wall_id == a && o.layer_index == 1)
                .unwrap();
            assert_eq!(l1.polygon, b1.polygon);
        }

        assert_eq!(p.reset_layer_joins(0, Some(&[a]), false), 1);
        assert!(p.floors[0].wall(a).unwrap().spec.layer_joins.is_empty());
        assert_eq!(p.reset_layer_joins(0, None, false), 0);
    }

    #[test]
    fn the_magnet_positions_are_where_the_layer_meets_the_other_walls_lines() {
        let (p, a, _b, types) = corner();
        let walls = &p.floors[0].walls;
        // The wall has a single main layer here, so the candidates are the
        // two faces and the centerline distances of the wall it meets.
        let c = layer_snap_candidates(walls, &types, a, WallEnd::End, 0, 0.5);
        assert!(!c.is_empty());
        // Nearest first: the layer's own mid-line meets the wall's centerline
        // at the corner itself (no slide).
        assert!(c[0].abs() < 1e-6, "{c:?}");
        assert!(c.iter().any(|t| (t.abs() - 4.0).abs() < 1e-6), "{c:?}");
        assert_eq!(snap_slide(&c, 3.6, 1.0), 4.0);
        assert_eq!(snap_slide(&c, 9.0, 1.0), 9.0);
    }

    #[test]
    fn reset_to_defaults_clears_the_locks_with_the_slides() {
        let (mut p, a, b, _) = corner();
        p.set_layer_join(0, a, WallEnd::Start, 0, -2.0);
        p.floors[0].wall_mut(b).unwrap().flags.lock_start = true;
        assert_eq!(p.reset_layer_joins(0, None, false), 1);
        assert!(p.floors[0].wall(b).unwrap().flags.lock_start, "locks stay");
        assert_eq!(p.reset_layer_joins(0, None, true), 1);
        assert!(!p.floors[0].wall(b).unwrap().flags.lock_start);
    }

    // ----- 3D layer outlines (brief 40) -----

    fn typed(id: u64, a: (f64, f64), b: (f64, f64)) -> (Wall, WallTypeDef) {
        let ty = PlanDefaults::chief_x18_daniel()
            .wall_type("Stucco-6")
            .unwrap()
            .clone();
        let mut w = Wall::new(
            Point::new(a.0, a.1),
            Point::new(b.0, b.1),
            ty.thickness(),
            96.0,
            WallKind::Exterior,
        );
        w.id = id;
        w.wall_type = Some(ty.name.clone());
        (w, ty)
    }

    fn area(poly: &[Point]) -> f64 {
        let n = poly.len();
        (0..n)
            .map(|i| poly[i].cross(poly[(i + 1) % n]))
            .sum::<f64>()
            .abs()
            * 0.5
    }

    /// Total overlap area between layers of different walls, and the
    /// difference between each wall's layer areas and its whole outline.
    fn overlap_and_gap(walls: &[Wall], ty: &WallTypeDef) -> (f64, f64) {
        let types = vec![ty.clone()];
        let outs = wall_layer_outlines(walls, &types, 0.5);
        let mut overlap = 0.0;
        for (i, a) in outs.iter().enumerate() {
            for b in outs.iter().skip(i + 1).filter(|b| b.wall_id != a.wall_id) {
                overlap += area(&crate::joins::convex_overlap(&a.polygon, &b.polygon));
            }
        }
        let whole = crate::joins::wall_outlines(walls, 0.5);
        let mut gap = 0.0;
        for w in walls {
            let layers: f64 = outs
                .iter()
                .filter(|o| o.wall_id == w.id)
                .map(|o| area(&o.polygon))
                .sum();
            let o = whole.iter().find(|o| o.wall_id == w.id).unwrap();
            gap += (layers - area(&o.polygon)).abs();
        }
        (overlap, gap)
    }

    #[test]
    fn l_corner_layers_meet_edge_to_edge() {
        let (a, ty) = typed(1, (0.0, 0.0), (120.0, 0.0));
        let (b, _) = typed(2, (120.0, 0.0), (120.0, 100.0));
        let walls = vec![a, b];
        let types = vec![ty.clone()];
        let (overlap, gap) = overlap_and_gap(&walls, &ty);
        assert!(overlap < 1e-6, "layers overlap by {overlap}");
        assert!(gap < 1e-6, "layers leave a gap of {gap}");
        let outs = wall_layer_outlines(&walls, &types, 0.5);
        // The end edge of every layer of the first wall lies along an edge of
        // the second wall's layers, so no end face is needed there.
        for k in 0..ty.layers.len() {
            let p = layer_end_polygon(&walls, &types, 1, k, 0.5).unwrap();
            assert!(end_edge_is_shared(&outs, 1, p[1], p[2]), "layer {k}");
        }
        // The far end is free.
        let p = layer_end_polygon(&walls, &types, 1, 0, 0.5).unwrap();
        assert!(!end_edge_is_shared(&outs, 1, p[0], p[3]));
        assert!(layer_end_polygon(&walls, &types, 1, 99, 0.5).is_none());
    }

    #[test]
    fn forty_five_degree_corner_layers_meet_edge_to_edge() {
        let (a, ty) = typed(1, (0.0, 0.0), (100.0, 0.0));
        let (b, _) = typed(2, (100.0, 0.0), (170.7, 70.7));
        let walls = vec![a, b];
        let (overlap, gap) = overlap_and_gap(&walls, &ty);
        assert!(overlap < 1e-6, "layers overlap by {overlap}");
        assert!(gap < 1e-6, "layers leave a gap of {gap}");
        let types = vec![ty.clone()];
        let outs = wall_layer_outlines(&walls, &types, 0.5);
        for k in 0..ty.layers.len() {
            let p = layer_end_polygon(&walls, &types, 1, k, 0.5).unwrap();
            assert!(end_edge_is_shared(&outs, 1, p[1], p[2]), "layer {k}");
        }
    }

    #[test]
    fn tee_abutting_layers_stop_on_the_through_wall() {
        let (a, ty) = typed(1, (0.0, 0.0), (240.0, 0.0));
        let (b, _) = typed(2, (120.0, 0.0), (120.0, 100.0));
        let walls = vec![a, b];
        let (overlap, _) = overlap_and_gap(&walls, &ty);
        assert!(overlap < 1e-6, "layers overlap by {overlap}");
        let types = vec![ty.clone()];
        let outs = wall_layer_outlines(&walls, &types, 0.5);
        for k in 0..ty.layers.len() {
            let p = layer_end_polygon(&walls, &types, 2, k, 0.5).unwrap();
            assert!(end_edge_is_shared(&outs, 2, p[0], p[3]), "layer {k}");
        }
        // The through wall's layers are whole and uncut.
        let p = layer_end_polygon(&walls, &types, 1, 0, 0.5).unwrap();
        assert!((p[0].dist(p[1]) - 240.0).abs() < 1e-6);
    }

    #[test]
    fn cross_of_four_walls_keeps_both_main_layers_whole() {
        let (a, ty) = typed(1, (0.0, 0.0), (120.0, 0.0));
        let (b, _) = typed(2, (120.0, 0.0), (240.0, 0.0));
        let (c, _) = typed(3, (120.0, -100.0), (120.0, 0.0));
        let (d, _) = typed(4, (120.0, 0.0), (120.0, 100.0));
        let walls = vec![a, b, c, d];
        let (overlap, gap) = overlap_and_gap(&walls, &ty);
        assert!(overlap < 1e-6, "layers overlap by {overlap}");
        assert!(gap < 1e-6, "layers leave a gap of {gap}");
    }
}
