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
use crate::joins::wall_layer_bands;
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
}
