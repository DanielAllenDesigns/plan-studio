//! Stepped and raked wall edges (W-141, manual p. 401). A wall's top and
//! bottom edge are lists of straight pieces along the wall: a break adds a
//! piece, a piece lifted as a whole makes a step, a corner moved on its own
//! makes a rake (two pieces meeting at a moved corner make a compound rake).
//! Heights are measured up from the wall's bottom, like `Wall::height`; the
//! top of a plain wall is `[]` and stands level at `height`. The 3D builder
//! reads the pieces as its top and bottom profiles, the plan marks each step
//! with an S.

use serde::{Deserialize, Serialize};

/// Edges closer than this to a piece end are the same place.
const EPS: f64 = 1e-6;

/// One straight stretch of an edge: from `s0` to `s1` along the wall, rising
/// from `h0` to `h1` (equal for a level piece).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EdgePiece {
    pub s0: f64,
    pub s1: f64,
    pub h0: f64,
    pub h1: f64,
}

impl EdgePiece {
    /// Height of the piece at `s` (clamped to the piece).
    pub fn at(&self, s: f64) -> f64 {
        let l = self.s1 - self.s0;
        if l < EPS {
            return self.h0;
        }
        let t = ((s - self.s0) / l).clamp(0.0, 1.0);
        self.h0 + (self.h1 - self.h0) * t
    }

    /// Whether the piece slopes.
    pub fn is_raked(&self) -> bool {
        (self.h1 - self.h0).abs() > 1e-6
    }
}

/// Which edge of the wall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
}

/// Which end of the wall the edge handle belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeEnd {
    Start,
    End,
}

/// The stepped and raked edges of a wall.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WallProfile {
    /// The top edge; empty is level at the wall's height.
    pub top: Vec<EdgePiece>,
    /// The bottom edge, heights above the wall's bottom; empty is level at 0.
    pub bottom: Vec<EdgePiece>,
}

impl WallProfile {
    /// A plain wall: both edges level.
    pub fn is_plain(&self) -> bool {
        self.top.is_empty() && self.bottom.is_empty()
    }

    fn edge(&self, e: Edge) -> &Vec<EdgePiece> {
        match e {
            Edge::Top => &self.top,
            Edge::Bottom => &self.bottom,
        }
    }

    fn edge_mut(&mut self, e: Edge) -> &mut Vec<EdgePiece> {
        match e {
            Edge::Top => &mut self.top,
            Edge::Bottom => &mut self.bottom,
        }
    }

    /// The edge as pieces over `0..length`, level at `level` when it has none.
    pub fn pieces(&self, e: Edge, length: f64, level: f64) -> Vec<EdgePiece> {
        let p = self.edge(e);
        if p.is_empty() {
            vec![EdgePiece {
                s0: 0.0,
                s1: length,
                h0: level,
                h1: level,
            }]
        } else {
            p.clone()
        }
    }

    /// Height of the edge at `s` (the higher side of a step).
    pub fn height_at(&self, e: Edge, s: f64, length: f64, level: f64) -> f64 {
        if self.edge(e).is_empty() {
            return level;
        }
        self.edge(e)
            .iter()
            .filter(|p| s >= p.s0 - EPS && s <= p.s1 + EPS)
            .map(|p| p.at(s))
            .fold(f64::MIN, f64::max)
            .max(if length > 0.0 { f64::MIN } else { level })
    }

    /// Add Break on an edge (W-141): splits the piece under `s`. A fresh edge
    /// starts as one level piece at `level`. Returns false at the wall ends
    /// or where a break already is.
    pub fn add_break(&mut self, e: Edge, s: f64, length: f64, level: f64) -> bool {
        if s <= EPS || s >= length - EPS {
            return false;
        }
        if self.edge(e).is_empty() {
            *self.edge_mut(e) = self.pieces(e, length, level);
        }
        let v = self.edge_mut(e);
        let Some(i) = v.iter().position(|p| s > p.s0 + EPS && s < p.s1 - EPS) else {
            return false;
        };
        let p = v[i];
        let mid = p.at(s);
        v[i] = EdgePiece {
            s0: p.s0,
            s1: s,
            h0: p.h0,
            h1: mid,
        };
        v.insert(
            i + 1,
            EdgePiece {
                s0: s,
                s1: p.s1,
                h0: mid,
                h1: p.h1,
            },
        );
        true
    }

    /// Lifts or lowers the whole piece number `i` to the level `h` (a step
    /// against its neighbours).
    pub fn set_step(&mut self, e: Edge, i: usize, h: f64) -> bool {
        let v = self.edge_mut(e);
        let Some(p) = v.get_mut(i) else {
            return false;
        };
        p.h0 = h;
        p.h1 = h;
        true
    }

    /// Moves the corner between piece `i` and piece `i + 1` to height `h`
    /// without a step: both pieces slope to it (a rake, or a compound rake
    /// when the pieces on either side already slope).
    pub fn set_corner(&mut self, e: Edge, i: usize, h: f64) -> bool {
        let v = self.edge_mut(e);
        if i + 1 >= v.len() {
            return false;
        }
        v[i].h1 = h;
        v[i + 1].h0 = h;
        true
    }

    /// Moves the end corner of the edge (the handle at a wall end) to `h`;
    /// the outer piece slopes. A plain edge becomes one raked piece.
    pub fn set_end(&mut self, e: Edge, end: EdgeEnd, h: f64, length: f64, level: f64) -> bool {
        if self.edge(e).is_empty() {
            *self.edge_mut(e) = self.pieces(e, length, level);
        }
        let v = self.edge_mut(e);
        let Some(p) = (match end {
            EdgeEnd::Start => v.first_mut(),
            EdgeEnd::End => v.last_mut(),
        }) else {
            return false;
        };
        match end {
            EdgeEnd::Start => p.h0 = h,
            EdgeEnd::End => p.h1 = h,
        }
        true
    }

    /// Removes the break at `s` (the piece before and after become one when
    /// they line up; otherwise the lower of the two heights is kept level).
    pub fn remove_break(&mut self, e: Edge, s: f64) -> bool {
        let v = self.edge_mut(e);
        let Some(i) = v.iter().position(|p| (p.s1 - s).abs() <= 1e-3) else {
            return false;
        };
        if i + 1 >= v.len() {
            return false;
        }
        let (a, b) = (v[i], v[i + 1]);
        v[i] = EdgePiece {
            s0: a.s0,
            s1: b.s1,
            h0: a.h0,
            h1: b.h1,
        };
        v.remove(i + 1);
        true
    }

    /// Positions where the edge jumps (steps). The plan marks these with S.
    pub fn steps(&self, e: Edge) -> Vec<f64> {
        let v = self.edge(e);
        v.windows(2)
            .filter(|w| (w[0].h1 - w[1].h0).abs() > 1e-6)
            .map(|w| w[0].s1)
            .collect()
    }

    /// Whether any piece of either edge slopes.
    pub fn is_raked(&self) -> bool {
        self.top.iter().chain(&self.bottom).any(EdgePiece::is_raked)
    }

    /// Whether the wall has a step on either edge.
    pub fn is_stepped(&self) -> bool {
        !self.steps(Edge::Top).is_empty() || !self.steps(Edge::Bottom).is_empty()
    }

    /// Drops pieces shorter than a hair, merges level pieces of one height
    /// that meet, and goes back to the plain wall when nothing is left to
    /// shape. Call after editing.
    pub fn tidy(&mut self, length: f64, top_level: f64) {
        for (e, level) in [(Edge::Top, top_level), (Edge::Bottom, 0.0)] {
            let v = self.edge_mut(e);
            v.retain(|p| p.s1 - p.s0 > 1e-4);
            let mut out: Vec<EdgePiece> = Vec::new();
            for p in v.drain(..) {
                match out.last_mut() {
                    Some(l)
                        if !l.is_raked()
                            && !p.is_raked()
                            && (l.h1 - p.h0).abs() < 1e-6
                            && (l.s1 - p.s0).abs() < 1e-6 =>
                    {
                        l.s1 = p.s1;
                    }
                    _ => out.push(p),
                }
            }
            *v = out;
            let plain = v.len() == 1
                && (v[0].s0).abs() < 1e-6
                && (v[0].s1 - length).abs() < 1e-3
                && !v[0].is_raked()
                && (v[0].h0 - level).abs() < 1e-6;
            if plain {
                v.clear();
            }
        }
    }

    /// Rescales the pieces when the wall length changes (a stretch keeps the
    /// steps in proportion).
    pub fn scale_to(&mut self, old_len: f64, new_len: f64) {
        if old_len < EPS || (old_len - new_len).abs() < EPS {
            return;
        }
        let k = new_len / old_len;
        for p in self.top.iter_mut().chain(self.bottom.iter_mut()) {
            p.s0 *= k;
            p.s1 *= k;
        }
    }

    /// Mirrors the pieces for a wall whose start and end swap.
    pub fn flip(&mut self, length: f64) {
        for v in [&mut self.top, &mut self.bottom] {
            for p in v.iter_mut() {
                *p = EdgePiece {
                    s0: length - p.s1,
                    s1: length - p.s0,
                    h0: p.h1,
                    h1: p.h0,
                };
            }
            v.reverse();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_break_then_a_lift_makes_a_step_that_the_plan_can_find() {
        let mut p = WallProfile::default();
        assert!(p.is_plain());
        assert!(!p.add_break(Edge::Top, 0.0, 120.0, 96.0), "not at an end");
        assert!(p.add_break(Edge::Top, 48.0, 120.0, 96.0));
        assert!(!p.add_break(Edge::Top, 48.0, 120.0, 96.0), "not twice");
        assert!(p.set_step(Edge::Top, 1, 120.0));
        assert_eq!(p.steps(Edge::Top), vec![48.0]);
        assert!(p.is_stepped() && !p.is_raked());
        assert_eq!(p.height_at(Edge::Top, 10.0, 120.0, 96.0), 96.0);
        assert_eq!(p.height_at(Edge::Top, 100.0, 120.0, 96.0), 120.0);
        // Taking the break out again leaves a plain edge.
        assert!(p.set_step(Edge::Top, 1, 96.0));
        p.tidy(120.0, 96.0);
        assert!(p.is_plain());
    }

    #[test]
    fn corners_rake_and_a_second_corner_makes_a_compound_rake() {
        let mut p = WallProfile::default();
        assert!(p.set_end(Edge::Top, EdgeEnd::End, 150.0, 120.0, 96.0));
        assert!(p.is_raked() && !p.is_stepped());
        assert!((p.height_at(Edge::Top, 60.0, 120.0, 96.0) - 123.0).abs() < 1e-9);
        // A corner in the middle bends the slope.
        assert!(p.add_break(Edge::Top, 60.0, 120.0, 96.0));
        assert!(p.set_corner(Edge::Top, 0, 96.0));
        assert!((p.height_at(Edge::Top, 30.0, 120.0, 96.0) - 96.0).abs() < 1e-9);
        assert!((p.height_at(Edge::Top, 90.0, 120.0, 96.0) - 123.0).abs() < 1e-9);
        assert!(p.steps(Edge::Top).is_empty());
    }

    #[test]
    fn stretching_and_flipping_keep_the_shape() {
        let mut p = WallProfile::default();
        p.add_break(Edge::Bottom, 30.0, 120.0, 0.0);
        p.set_step(Edge::Bottom, 0, 12.0);
        p.scale_to(120.0, 240.0);
        assert_eq!(p.steps(Edge::Bottom), vec![60.0]);
        p.flip(240.0);
        assert_eq!(p.steps(Edge::Bottom), vec![180.0]);
        assert_eq!(p.height_at(Edge::Bottom, 230.0, 240.0, 0.0), 12.0);
    }

    #[test]
    fn removing_a_break_joins_the_pieces() {
        let mut p = WallProfile::default();
        p.add_break(Edge::Top, 40.0, 100.0, 96.0);
        assert!(p.remove_break(Edge::Top, 40.0));
        p.tidy(100.0, 96.0);
        assert!(p.is_plain());
        assert!(!p.remove_break(Edge::Top, 40.0));
    }
}
