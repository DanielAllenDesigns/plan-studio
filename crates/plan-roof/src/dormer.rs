//! Auto Dormer and Explode Dormer (Chief RF-48..RF-51).
//!
//! A dormer is built on one roof plane from a handful of dimensions. The
//! result is plain geometry: a front wall, two cheek (side) walls, the dormer
//! roof planes, the hole to cut in the main roof, and an optional window
//! opening on the front wall. An overhang (`DormerSpec::overhang`) lengthens
//! the roof planes past the walls ([`Dormer::overhang_planes`]); the soffit and
//! fascia are the 3D builder's (`plan-3d` eave detail).
//!
//! Frame: the main plane's eave (`baseline`) runs from `a` to `b`; `u` is the
//! unit eave direction and `w = u.perp()` points up the slope. A dormer point
//! is addressed by `(along, d)`: `along` is the distance from `a` along `u`,
//! `d` the plan distance from the eave line along `w`.

use crate::geom::{self, V3};
use crate::hole::{HoleKind, RoofHole};
use crate::RoofPlane;
use plan_core::Point;
use serde::{Deserialize, Serialize};

/// Dormer roof shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DormerKind {
    /// Two planes meeting in a horizontal ridge; gable front wall.
    #[default]
    Gable,
    /// One plane, flatter than the main roof.
    Shed,
    /// Three planes (front hip and two sides).
    Hip,
}

/// Dormer dimensions, inches.
///
/// `height_to_ridge` and `pitch` both describe the slope of a gable or hip
/// roof. When `height_to_ridge > wall_height` it wins (the roof pitch is then
/// derived and reported on the roof planes); otherwise `pitch` is used. A shed
/// dormer always uses `pitch` (halved when not flatter than the main roof) and
/// ignores `height_to_ridge`: its roof runs until it meets the main roof.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DormerSpec {
    pub kind: DormerKind,
    /// Outside width of the front wall, along the eave direction.
    pub width: f64,
    /// Ridge height above the main roof surface at the front wall base.
    pub height_to_ridge: f64,
    /// Height of the front wall and cheek-wall tops above the same base.
    pub wall_height: f64,
    /// Distance from the baseline's first point to the dormer centre, along the
    /// eave direction.
    pub position_along_eave: f64,
    /// Plan distance from the eave line up the slope to the front wall.
    pub setback_from_eave: f64,
    /// Dormer roof pitch (rise in 12), see above.
    pub pitch: f64,
    /// Window `(width, height)` centred on the front wall.
    pub window: Option<(f64, f64)>,
    /// Horizontal overhang of the dormer roof past its walls (eaves and
    /// rakes), inches. `0` keeps the roof flush with the walls. The roof
    /// planes in [`Dormer::overhang_planes`] carry it; the hole in the main
    /// roof is always the wall footprint.
    #[serde(default)]
    pub overhang: f64,
}

impl Default for DormerSpec {
    /// 48" gable dormer, 36" walls, 8:12, centred at the origin, 36" back from
    /// the eave, no window.
    fn default() -> Self {
        Self {
            kind: DormerKind::Gable,
            width: 48.0,
            height_to_ridge: 0.0,
            wall_height: 36.0,
            position_along_eave: 0.0,
            setback_from_eave: 36.0,
            pitch: 8.0,
            window: None,
            overhang: 0.0,
        }
    }
}

/// A dormer wall as a vertical polygon plus the plan line it stands on.
#[derive(Debug, Clone, PartialEq)]
pub struct DormerWall {
    /// Roof-space outline (`X = x`, `Y` up, `Z = -y`); the Newell normal points
    /// out of the dormer.
    pub polygon3d: Vec<V3>,
    /// Unit outward normal (horizontal).
    pub normal: V3,
    /// Plan base line.
    pub start: Point,
    pub end: Point,
    /// Elevation of the wall base at `start` (the main roof surface under the
    /// front wall), inches.
    pub base_elevation: f64,
    /// Full wall height above `base_elevation` at its tallest point.
    pub height: f64,
}

/// A rectangular window opening in the front wall.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowOpening {
    /// Four corners on the front wall plane, outward-facing winding.
    pub polygon3d: Vec<V3>,
    pub width: f64,
    pub height: f64,
    /// Sill height above the wall base.
    pub sill_height: f64,
}

/// A dormer built on a main roof plane.
#[derive(Debug, Clone, PartialEq)]
pub struct Dormer {
    pub kind: DormerKind,
    pub spec: DormerSpec,
    pub front_wall: DormerWall,
    /// The two cheek walls: left (toward the baseline start) then right.
    pub side_walls: Vec<DormerWall>,
    /// Gable 2, shed 1, hip 3. Each polygon starts with its eave edge. They
    /// cover the hole in the main roof exactly (no overhang).
    pub roof_planes: Vec<RoofPlane>,
    /// The roof planes with the spec's overhang: eaves and rakes pushed out by
    /// `spec.overhang`, valleys, ridges and hips kept on their lines. The same
    /// planes as `roof_planes` (same order, same edge indices) when the
    /// overhang is zero.
    pub overhang_planes: Vec<RoofPlane>,
    /// `(plane, edge)` of every edge of `overhang_planes` that meets the main
    /// roof (a valley): no fascia or soffit goes there.
    pub valley_edges: Vec<(usize, usize)>,
    /// The footprint to cut in the main roof (`kind` is [`HoleKind::Hole`]).
    pub hole_in_main_roof: RoofHole,
    pub window_opening: Option<WindowOpening>,
    /// Elevation of the dormer ridge (gable/hip) or of the shed roof where it
    /// meets the main roof, inches.
    pub ridge_elevation: f64,
    /// Plan depth of the dormer from the front wall to the back of its roof.
    pub depth: f64,
}

/// The dormer's parts as plain planes and walls, no longer tied to a spec.
#[derive(Debug, Clone, PartialEq)]
pub struct ExplodedDormer {
    /// Front wall first, then the cheek walls.
    pub walls: Vec<DormerWall>,
    pub roof_planes: Vec<RoofPlane>,
    pub hole: RoofHole,
    pub window_opening: Option<WindowOpening>,
}

/// Split a dormer into independent parts (Chief "Explode Dormer").
pub fn explode_dormer(dormer: &Dormer) -> ExplodedDormer {
    let mut walls = vec![dormer.front_wall.clone()];
    walls.extend(dormer.side_walls.iter().cloned());
    ExplodedDormer {
        walls,
        roof_planes: dormer.overhang_planes.clone(),
        hole: dormer.hole_in_main_roof.clone(),
        window_opening: dormer.window_opening.clone(),
    }
}

/// Horizontal plan vector as a roof-space direction.
fn dir3(v: Point) -> V3 {
    [v.x, 0.0, -v.y]
}

struct Frame {
    a: Point,
    u: Point,
    w: Point,
    /// Elevation of the main plane at the front wall base.
    yb: f64,
    /// Front wall distance from the eave line.
    s: f64,
}

impl Frame {
    fn plan(&self, along: f64, d: f64) -> Point {
        self.a.add(self.u.scale(along)).add(self.w.scale(d))
    }
    /// Roof-space point at `(along, d)` and height `rel` above the front base.
    fn pt(&self, along: f64, d: f64, rel: f64) -> V3 {
        geom::lift(self.plan(along, d), self.yb + rel)
    }
}

/// Build a dormer on `main_plane`.
///
/// Returns `None` when the dormer cannot be built: a degenerate dimension, a
/// flat or reversed main plane, or a footprint that does not lie completely
/// inside the plane (too wide, too close to the eave, or reaching past the
/// ridge). Deviation from a bare `Dormer` return: the failure cases need a
/// value.
pub fn auto_dormer(main_plane: &RoofPlane, spec: DormerSpec) -> Option<Dormer> {
    if spec.width <= 1e-6 || spec.wall_height <= 1e-6 || main_plane.polygon3d.len() < 3 {
        return None;
    }
    let (a, b) = main_plane.baseline;
    let u = b.sub(a).normalized();
    if u == Point::ZERO {
        return None;
    }
    let w = u.perp();
    let h0 = main_plane.height_at(a)?;
    let tm = main_plane.height_at(a.add(w))? - h0;
    if tm <= 1e-6 {
        return None;
    }
    let c = spec.position_along_eave;
    let mut f = Frame {
        a,
        u,
        w,
        yb: 0.0,
        s: spec.setback_from_eave,
    };
    f.yb = main_plane.height_at(f.plan(c, f.s))?;

    let (hw, wh) = (spec.width * 0.5, spec.wall_height);
    let (ul, ur) = (c - hw, c + hw);
    let s = f.s;

    // Roof-plane polygons start with their eave edge; walls are oriented below.
    let roof_polys: Vec<Vec<V3>>;
    let hole_outline: Vec<Point>;
    let front_poly: Vec<V3>;
    let (cheek_depth, ridge_rel, depth): (f64, f64, f64);
    match spec.kind {
        DormerKind::Gable | DormerKind::Hip => {
            let rise = if spec.height_to_ridge > wh {
                spec.height_to_ridge - wh
            } else {
                hw * spec.pitch / 12.0
            };
            if rise <= 1e-6 {
                return None;
            }
            let hr = wh + rise;
            let dw = wh / tm;
            let dr = hr / tm;
            let gable = spec.kind == DormerKind::Gable;
            if !gable && dr <= hw + 1e-6 {
                return None; // ridge would collapse into a point
            }
            let left = if gable {
                vec![
                    f.pt(ul, s, wh),
                    f.pt(ul, s + dw, wh),
                    f.pt(c, s + dr, hr),
                    f.pt(c, s, hr),
                ]
            } else {
                vec![
                    f.pt(ul, s, wh),
                    f.pt(ul, s + dw, wh),
                    f.pt(c, s + dr, hr),
                    f.pt(c, s + hw, hr),
                ]
            };
            let right = if gable {
                vec![
                    f.pt(ur, s, wh),
                    f.pt(ur, s + dw, wh),
                    f.pt(c, s + dr, hr),
                    f.pt(c, s, hr),
                ]
            } else {
                vec![
                    f.pt(ur, s, wh),
                    f.pt(ur, s + dw, wh),
                    f.pt(c, s + dr, hr),
                    f.pt(c, s + hw, hr),
                ]
            };
            let mut polys = vec![left, right];
            if gable {
                front_poly = vec![
                    f.pt(ul, s, 0.0),
                    f.pt(ur, s, 0.0),
                    f.pt(ur, s, wh),
                    f.pt(c, s, hr),
                    f.pt(ul, s, wh),
                ];
            } else {
                polys.push(vec![f.pt(ul, s, wh), f.pt(ur, s, wh), f.pt(c, s + hw, hr)]);
                front_poly = vec![
                    f.pt(ul, s, 0.0),
                    f.pt(ur, s, 0.0),
                    f.pt(ur, s, wh),
                    f.pt(ul, s, wh),
                ];
            }
            roof_polys = polys;
            hole_outline = vec![
                f.plan(ul, s),
                f.plan(ur, s),
                f.plan(ur, s + dw),
                f.plan(c, s + dr),
                f.plan(ul, s + dw),
            ];
            cheek_depth = dw;
            ridge_rel = hr;
            depth = dr;
        }
        DormerKind::Shed => {
            let raw = if spec.pitch > 0.0 {
                spec.pitch / 12.0
            } else {
                tm * 0.5
            };
            let ts = if raw >= tm * 0.95 { tm * 0.5 } else { raw };
            let d_shed = wh / (tm - ts);
            let meet = tm * d_shed;
            roof_polys = vec![vec![
                f.pt(ul, s, wh),
                f.pt(ur, s, wh),
                f.pt(ur, s + d_shed, meet),
                f.pt(ul, s + d_shed, meet),
            ]];
            front_poly = vec![
                f.pt(ul, s, 0.0),
                f.pt(ur, s, 0.0),
                f.pt(ur, s, wh),
                f.pt(ul, s, wh),
            ];
            hole_outline = vec![
                f.plan(ul, s),
                f.plan(ur, s),
                f.plan(ur, s + d_shed),
                f.plan(ul, s + d_shed),
            ];
            cheek_depth = d_shed;
            ridge_rel = meet;
            depth = d_shed;
        }
    }

    // The whole footprint must sit inside the main plane.
    let outline = geom::ccw(&main_plane.plan_polygon());
    if !hole_outline
        .iter()
        .all(|&p| geom::strictly_inside(p, &outline, 1e-6))
    {
        return None;
    }

    let roof_planes: Vec<RoofPlane> = roof_polys
        .into_iter()
        .map(|poly| {
            let poly = geom::up_eave_first(poly);
            let pitch = roof_pitch(&poly);
            RoofPlane {
                baseline: (geom::to_plan(poly[0]), geom::to_plan(poly[1])),
                polygon3d: poly,
                pitch_in_12: pitch,
                source_edge: main_plane.source_edge,
            }
        })
        .collect();

    let (overhang_planes, valley_edges) =
        overhang_roof(&roof_planes, main_plane, spec.overhang.max(0.0));

    let front_out = f.w.scale(-1.0);
    let front_wall = DormerWall {
        polygon3d: geom::orient_toward(front_poly, dir3(front_out)),
        normal: dir3(front_out),
        start: f.plan(ul, s),
        end: f.plan(ur, s),
        base_elevation: f.yb,
        height: if spec.kind == DormerKind::Gable {
            ridge_rel
        } else {
            wh
        },
    };
    let cheek = |along: f64, out: Point| {
        let tri = vec![
            f.pt(along, s, 0.0),
            f.pt(along, s, wh),
            f.pt(along, s + cheek_depth, tm * cheek_depth),
        ];
        DormerWall {
            polygon3d: geom::orient_toward(tri, dir3(out)),
            normal: dir3(out),
            start: f.plan(along, s),
            end: f.plan(along, s + cheek_depth),
            base_elevation: f.yb,
            height: wh,
        }
    };
    let side_walls = vec![cheek(ul, f.u.scale(-1.0)), cheek(ur, f.u)];

    let window_opening = spec.window.and_then(|(ww, wht)| {
        let ww = ww.min(spec.width - 6.0);
        let wht = wht.min(wh);
        (ww > 1e-6 && wht > 1e-6).then(|| {
            let sill = (wh - wht) * 0.5;
            let (x0, x1) = (c - ww * 0.5, c + ww * 0.5);
            let poly = vec![
                f.pt(x0, s, sill),
                f.pt(x1, s, sill),
                f.pt(x1, s, sill + wht),
                f.pt(x0, s, sill + wht),
            ];
            WindowOpening {
                polygon3d: geom::orient_toward(poly, dir3(front_out)),
                width: ww,
                height: wht,
                sill_height: sill,
            }
        })
    });

    Some(Dormer {
        kind: spec.kind,
        spec,
        front_wall,
        side_walls,
        roof_planes,
        overhang_planes,
        valley_edges,
        hole_in_main_roof: RoofHole {
            outline: hole_outline,
            kind: HoleKind::Hole,
            skylight: None,
        },
        window_opening,
        ridge_elevation: f.yb + ridge_rel,
        depth,
    })
}

/// The dormer roof planes with `overhang` inches of eave and rake, and the
/// `(plane, edge)` pairs that meet `main` (valleys).
///
/// Each plane is offset edge by edge in plan: an edge no other dormer plane
/// shares and that is not on the main roof (an eave or a rake) moves outward
/// by `overhang`; ridges, hips and valleys stay on their lines, so a
/// neighbouring pair of planes still meets along the same hip (mitred) and the
/// valley stretches along the line where the plane meets the main roof. The
/// new corners are the intersections of neighbouring edge lines, lifted onto
/// the plane. The result has the same vertex and edge order as the input.
fn overhang_roof(
    planes: &[RoofPlane],
    main: &RoofPlane,
    overhang: f64,
) -> (Vec<RoofPlane>, Vec<(usize, usize)>) {
    let on_main = |p: V3| {
        main.height_at(geom::to_plan(p))
            .is_some_and(|y| (y - p[1]).abs() < 1e-4)
    };
    let same = |a: V3, b: V3| geom::sub3(a, b).iter().all(|c| c.abs() < 1e-4);
    let mut valleys = Vec::new();
    let mut out = Vec::with_capacity(planes.len());
    for (k, pl) in planes.iter().enumerate() {
        let poly = &pl.polygon3d;
        let n = poly.len();
        // Per edge: offset it (eave or rake)?
        let moves: Vec<bool> = (0..n)
            .map(|i| {
                let (a, b) = (poly[i], poly[(i + 1) % n]);
                if on_main(a) && on_main(b) {
                    valleys.push((k, i));
                    return false;
                }
                let shared = planes.iter().enumerate().any(|(l, q)| {
                    l != k && {
                        let m = q.polygon3d.len();
                        (0..m).any(|j| {
                            let (c, d) = (q.polygon3d[j], q.polygon3d[(j + 1) % m]);
                            (same(a, c) && same(b, d)) || (same(a, d) && same(b, c))
                        })
                    }
                });
                !shared
            })
            .collect();
        if overhang <= 1e-9 || n < 3 || !moves.iter().any(|&m| m) {
            out.push(pl.clone());
            continue;
        }
        let pts: Vec<Point> = poly.iter().map(|&p| geom::to_plan(p)).collect();
        // Eaves and rakes move out of the (counter-clockwise) polygon.
        let shifts: Vec<f64> = moves
            .iter()
            .map(|&m| if m { overhang } else { 0.0 })
            .collect();
        let new_pts = geom::offset_edges(&pts, &shifts);
        let lifted: Option<Vec<V3>> = new_pts
            .iter()
            .map(|&p| pl.height_at(p).map(|y| geom::lift(p, y)))
            .collect();
        match lifted {
            Some(polygon3d) => out.push(RoofPlane {
                baseline: (geom::to_plan(polygon3d[0]), geom::to_plan(polygon3d[1])),
                polygon3d,
                pitch_in_12: pl.pitch_in_12,
                source_edge: pl.source_edge,
            }),
            None => out.push(pl.clone()),
        }
    }
    (out, valleys)
}

/// Pitch (rise in 12) of an upward-facing polygon from its normal.
fn roof_pitch(poly: &[V3]) -> f64 {
    let n = geom::unit3(geom::newell(poly)).unwrap_or([0.0, 1.0, 0.0]);
    if n[1] <= 1e-9 {
        return 0.0;
    }
    (n[0] * n[0] + n[2] * n[2]).sqrt() / n[1] * 12.0
}

// ===================================================================
// Second pitch (gambrel), dormer rooms and returns, crickets (RF-84..RF-87)
// ===================================================================

/// The second pitch of a dormer roof: the roof rises at the dormer's own
/// pitch for `in_from_eave` inches (measured across, from the eave), then at
/// `pitch2` to the ridge. A gable dormer with a second pitch is a gambrel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SecondPitch {
    /// Rise per 12 of the upper part.
    pub pitch2: f64,
    /// Plan distance from the eave to the break, inches.
    pub in_from_eave: f64,
}

/// A gambrel dormer: a gable dormer whose roof planes break at
/// `second.in_from_eave` from each eave. `spec.pitch` is the lower pitch (the
/// first, from the eave); the ridge stands where the upper pitch reaches the
/// centre, and `height_to_ridge` is ignored. Four roof planes: lower left,
/// upper left, lower right, upper right. The front wall follows the broken
/// profile and the opening in the main roof is the plan of the planes.
///
/// `None` for a degenerate dimension, a break not between the eave and the
/// ridge, a flat or reversed main plane or a footprint outside the plane.
pub fn gambrel_dormer(
    main_plane: &RoofPlane,
    spec: DormerSpec,
    second: SecondPitch,
) -> Option<Dormer> {
    if spec.width <= 1e-6 || spec.wall_height <= 1e-6 || main_plane.polygon3d.len() < 3 {
        return None;
    }
    let hw = spec.width * 0.5;
    if second.pitch2 <= 1e-6
        || spec.pitch <= 1e-6
        || second.in_from_eave <= 1e-6
        || second.in_from_eave >= hw - 1e-6
    {
        return None;
    }
    let (a, b) = main_plane.baseline;
    let u = b.sub(a).normalized();
    if u == Point::ZERO {
        return None;
    }
    let w = u.perp();
    let h0 = main_plane.height_at(a)?;
    let tm = main_plane.height_at(a.add(w))? - h0;
    if tm <= 1e-6 {
        return None;
    }
    let c = spec.position_along_eave;
    let mut f = Frame {
        a,
        u,
        w,
        yb: 0.0,
        s: spec.setback_from_eave,
    };
    f.yb = main_plane.height_at(f.plan(c, f.s))?;
    let (wh, s) = (spec.wall_height, f.s);
    let (ul, ur) = (c - hw, c + hw);
    let inn = second.in_from_eave;
    let h1 = wh + inn * spec.pitch / 12.0;
    let hr = h1 + (hw - inn) * second.pitch2 / 12.0;
    let (dw, d1, dr) = (wh / tm, h1 / tm, hr / tm);
    let (bl, br) = (ul + inn, ur - inn);
    let polys = vec![
        // Lower left, upper left, lower right, upper right.
        vec![
            f.pt(ul, s, wh),
            f.pt(ul, s + dw, wh),
            f.pt(bl, s + d1, h1),
            f.pt(bl, s, h1),
        ],
        vec![
            f.pt(bl, s, h1),
            f.pt(bl, s + d1, h1),
            f.pt(c, s + dr, hr),
            f.pt(c, s, hr),
        ],
        vec![
            f.pt(ur, s, wh),
            f.pt(ur, s + dw, wh),
            f.pt(br, s + d1, h1),
            f.pt(br, s, h1),
        ],
        vec![
            f.pt(br, s, h1),
            f.pt(br, s + d1, h1),
            f.pt(c, s + dr, hr),
            f.pt(c, s, hr),
        ],
    ];
    let hole_outline = vec![
        f.plan(ul, s),
        f.plan(ur, s),
        f.plan(ur, s + dw),
        f.plan(br, s + d1),
        f.plan(c, s + dr),
        f.plan(bl, s + d1),
        f.plan(ul, s + dw),
    ];
    let outline = geom::ccw(&main_plane.plan_polygon());
    if !hole_outline
        .iter()
        .all(|&p| geom::strictly_inside(p, &outline, 1e-6))
    {
        return None;
    }
    let roof_planes: Vec<RoofPlane> = polys
        .into_iter()
        .map(|poly| {
            let poly = geom::up_eave_first(poly);
            let pitch = roof_pitch(&poly);
            RoofPlane {
                baseline: (geom::to_plan(poly[0]), geom::to_plan(poly[1])),
                polygon3d: poly,
                pitch_in_12: pitch,
                source_edge: main_plane.source_edge,
            }
        })
        .collect();
    let (overhang_planes, valley_edges) =
        overhang_roof(&roof_planes, main_plane, spec.overhang.max(0.0));
    let front_out = f.w.scale(-1.0);
    let front_poly = vec![
        f.pt(ul, s, 0.0),
        f.pt(ur, s, 0.0),
        f.pt(ur, s, wh),
        f.pt(br, s, h1),
        f.pt(c, s, hr),
        f.pt(bl, s, h1),
        f.pt(ul, s, wh),
    ];
    let front_wall = DormerWall {
        polygon3d: geom::orient_toward(front_poly, dir3(front_out)),
        normal: dir3(front_out),
        start: f.plan(ul, s),
        end: f.plan(ur, s),
        base_elevation: f.yb,
        height: hr,
    };
    let cheek = |along: f64, out: Point| {
        let tri = vec![
            f.pt(along, s, 0.0),
            f.pt(along, s, wh),
            f.pt(along, s + dw, wh),
            f.pt(along, s + dw, tm * dw),
        ];
        DormerWall {
            polygon3d: geom::orient_toward(tri, dir3(out)),
            normal: dir3(out),
            start: f.plan(along, s),
            end: f.plan(along, s + dw),
            base_elevation: f.yb,
            height: wh,
        }
    };
    let side_walls = vec![cheek(ul, f.u.scale(-1.0)), cheek(ur, f.u)];
    Some(Dormer {
        kind: DormerKind::Gable,
        spec,
        front_wall,
        side_walls,
        roof_planes,
        overhang_planes,
        valley_edges,
        hole_in_main_roof: RoofHole {
            outline: hole_outline,
            kind: HoleKind::Hole,
            skylight: None,
        },
        window_opening: None,
        ridge_elevation: f.yb + hr,
        depth: dr,
    })
}

/// Dormer Room options of a floating dormer (the dormer stands on the roof
/// without a hole through it).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DormerRoom {
    /// Create Shaft to Room Below: vertical walls from the dormer opening
    /// down to the floor of the room below.
    pub create_shaft: bool,
    /// Form Room: the dormer interior becomes a room of its own.
    pub form_room: bool,
    /// Wall type of the shaft and room walls; empty is the default wall type.
    pub wall_type: String,
    /// Set to Existing Ceiling: the room's ceiling is the ceiling of the room
    /// below instead of the top of the dormer walls.
    pub set_to_existing_ceiling: bool,
}

/// The shaft walls under a dormer opening: plumb walls from the main roof
/// surface down to `floor_y` (the floor of the room below), one per edge of
/// the opening. Empty unless the room asks for a shaft.
pub fn dormer_shaft(
    main_plane: &RoofPlane,
    dormer: &Dormer,
    room: &DormerRoom,
    floor_y: f64,
) -> Vec<crate::hole::RimWall> {
    if !room.create_shaft {
        return Vec::new();
    }
    crate::hole::rim_walls(
        main_plane,
        &dormer.hole_in_main_roof.outline,
        crate::hole::HoleRim::Plumb,
        floor_y,
    )
}

/// The ceiling of a dormer room, elevation: `existing_ceiling` when Set to
/// Existing Ceiling is on and there is one, else the top of the dormer walls.
pub fn dormer_room_ceiling(
    dormer: &Dormer,
    room: &DormerRoom,
    existing_ceiling: Option<f64>,
) -> f64 {
    match (room.set_to_existing_ceiling, existing_ceiling) {
        (true, Some(y)) => y,
        _ => dormer.front_wall.base_elevation + dormer.spec.wall_height,
    }
}

/// Auto Roof Return on a gable dormer: a return at each front eave corner of
/// the dormer roof, wrapping the corner along the eave line and up the front
/// rake. Hip and shed dormers have no gable end: nothing comes back. The
/// returns are made on the planes with the overhang, so they wrap the
/// corners of the finished roof.
pub fn dormer_returns(
    dormer: &Dormer,
    spec: crate::gable::ReturnSpec,
) -> Vec<crate::gable::RoofReturn> {
    if dormer.kind != DormerKind::Gable {
        return Vec::new();
    }
    let front = Point::new(dormer.front_wall.normal[0], -dormer.front_wall.normal[2]);
    dormer
        .overhang_planes
        .iter()
        .filter_map(|pl| {
            let p = &pl.polygon3d;
            if p.len() < 3 {
                return None;
            }
            // Only the eave of a plane that has the front rake: its eave
            // edge ends at the front wall line.
            let (a, b) = (geom::to_plan(p[0]), geom::to_plan(p[1]));
            let at_start = a.dot(front) > b.dot(front);
            let reaches_front = (a.dot(front) - b.dot(front)).abs() > 1e-6;
            reaches_front.then(|| crate::gable::roof_return_at(pl, 0, at_start, spec))?
        })
        .collect()
}

/// A cricket: the small saddle roof behind a chimney or an up-slope wall
/// that sheds water round it.
#[derive(Debug, Clone, PartialEq)]
pub struct Cricket {
    /// The two triangular planes, left then right as seen looking up-slope.
    pub planes: [RoofPlane; 2],
    /// The level ridge, from the wall face up-slope to where it meets the
    /// roof.
    pub ridge: (V3, V3),
    /// Height of the ridge above the roof at the wall, inches.
    pub height: f64,
    /// Plan length of the ridge, inches.
    pub length: f64,
    /// Pitch of the two planes, rise in 12.
    pub pitch_in_12: f64,
}

/// The cricket behind the wall face `a` to `b` (plan points, for a chimney
/// its up-slope side) on `main_plane`. The ridge starts at the middle of the
/// face at `width / 2 * pitch / 12` above the roof and runs level up-slope
/// until the rising roof meets it; each plane is a triangle from the ridge to
/// one end of the face. `pitch` is the cricket's rise in 12; `None` uses half
/// the roof's, the usual cricket. `None` comes back for a face under 6
/// inches, a flat roof or a point off the plane.
pub fn cricket_behind(
    main_plane: &RoofPlane,
    a: Point,
    b: Point,
    pitch: Option<f64>,
) -> Option<Cricket> {
    let width = a.dist(b);
    if width < 6.0 {
        return None;
    }
    let n = main_plane.normal();
    let flat = (n[0] * n[0] + n[2] * n[2]).sqrt();
    if n[1] < 1e-9 || flat < 1e-9 {
        return None;
    }
    // Up the slope in plan; the gradient of the roof along it.
    let up = Point::new(-n[0] / flat, n[2] / flat);
    let slope = flat / n[1];
    let cp = pitch.unwrap_or(main_plane.pitch_in_12 * 0.5);
    if cp <= 1e-6 || cp / 12.0 >= slope {
        // A cricket as steep as the roof never meets it.
        return None;
    }
    let mid = Point::lerp(a, b, 0.5);
    let h = width * 0.5 * cp / 12.0;
    let length = h / slope;
    let y0 = main_plane.height_at(mid)?;
    let top = y0 + h;
    let end = mid.add(up.scale(length));
    let r0 = geom::lift(mid, top);
    let r1 = geom::lift(end, top);
    let corner = |p: Point| main_plane.height_at(p).map(|y| geom::lift(p, y));
    let (va, vb) = (corner(a)?, corner(b)?);
    // Left as seen looking up-slope: the corner with the larger cross product.
    let (left, right) = if up.cross(a.sub(mid)) > 0.0 {
        (va, vb)
    } else {
        (vb, va)
    };
    let make = |corner: V3| {
        let poly = geom::up_eave_first(vec![corner, r0, r1]);
        let pitch = roof_pitch(&poly);
        RoofPlane {
            baseline: (geom::to_plan(poly[0]), geom::to_plan(poly[1])),
            polygon3d: poly,
            pitch_in_12: pitch,
            source_edge: main_plane.source_edge,
        }
    };
    Some(Cricket {
        planes: [make(left), make(right)],
        ridge: (r0, r1),
        height: h,
        length,
        pitch_in_12: cp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hole::tests::gable_roof_planes;
    use plan_core::geometry::polygon_area;

    fn south() -> RoofPlane {
        gable_roof_planes()
            .into_iter()
            .find(|p| p.source_edge == 0)
            .unwrap()
    }

    fn spec(kind: DormerKind) -> DormerSpec {
        DormerSpec {
            kind,
            width: 72.0,
            height_to_ridge: 0.0,
            wall_height: 36.0,
            position_along_eave: 240.0,
            setback_from_eave: 30.0,
            pitch: 8.0,
            window: Some((30.0, 24.0)),
            overhang: 0.0,
        }
    }

    fn hole_area(d: &Dormer) -> f64 {
        polygon_area(&d.hole_in_main_roof.outline).abs()
    }

    fn roofs_plan_area(d: &Dormer) -> f64 {
        d.roof_planes.iter().map(RoofPlane::projected_area).sum()
    }

    #[test]
    fn gable_dormer_has_two_planes_horizontal_ridge_vertical_front() {
        let main = south();
        assert!((main.pitch_in_12 - 8.0).abs() < 1e-9);
        let d = auto_dormer(&main, spec(DormerKind::Gable)).unwrap();
        assert_eq!(d.roof_planes.len(), 2);
        assert_eq!(d.side_walls.len(), 2);
        // Ridge: the two highest vertices of the roof planes share a height.
        let top = d
            .roof_planes
            .iter()
            .flat_map(|p| p.polygon3d.iter())
            .fold(f64::MIN, |m, p| m.max(p[1]));
        let ridge: Vec<V3> = d.roof_planes[0]
            .polygon3d
            .iter()
            .copied()
            .filter(|p| (p[1] - top).abs() < 1e-6)
            .collect();
        assert_eq!(ridge.len(), 2);
        assert!((ridge[0][1] - ridge[1][1]).abs() < 1e-9);
        assert!((top - d.ridge_elevation).abs() < 1e-6);
        // Pitch 8:12 from the dormer pitch field.
        for p in &d.roof_planes {
            assert!((p.pitch_in_12 - 8.0).abs() < 1e-6, "{}", p.pitch_in_12);
            assert!(p.normal()[1] > 0.0);
            assert_eq!(p.baseline.0, geom::to_plan(p.polygon3d[0]));
        }
        // Front wall is vertical: horizontal normal, shared plan line.
        let f = &d.front_wall;
        assert!(f.normal[1].abs() < 1e-12);
        let nn = geom::unit3(geom::newell(&f.polygon3d)).unwrap();
        assert!(nn[1].abs() < 1e-9);
        assert!(geom::dot3(nn, f.normal) > 0.999);
        // Faces down the slope (toward the eave at y = 0, so plan -y).
        assert!(f.normal[2] > 0.99, "{:?}", f.normal);
        let plan_pts: Vec<Point> = f.polygon3d.iter().map(|&p| geom::to_plan(p)).collect();
        assert!(plan_pts.iter().all(|p| (p.y - 30.0).abs() < 1e-6));
        // Base sits on the main roof.
        for p in &f.polygon3d {
            if (p[1] - f.base_elevation).abs() < 1e-9 {
                let y = main.height_at(geom::to_plan(*p)).unwrap();
                assert!((y - p[1]).abs() < 1e-6);
            }
        }
        assert!(f.height > 36.0);
    }

    #[test]
    fn height_to_ridge_overrides_pitch_for_gable() {
        let main = south();
        let mut sp = spec(DormerKind::Gable);
        sp.height_to_ridge = 36.0 + 36.0; // rise 36 over half-width 36: 12:12
        let d = auto_dormer(&main, sp).unwrap();
        for p in &d.roof_planes {
            assert!((p.pitch_in_12 - 12.0).abs() < 1e-6);
        }
        assert!((d.ridge_elevation - d.front_wall.base_elevation - 72.0).abs() < 1e-6);
    }

    #[test]
    fn hole_matches_the_dormer_footprint_for_every_kind() {
        let main = south();
        for kind in [DormerKind::Gable, DormerKind::Hip, DormerKind::Shed] {
            let d = auto_dormer(&main, spec(kind)).unwrap();
            assert_eq!(d.hole_in_main_roof.kind, HoleKind::Hole);
            // The dormer roofs cover exactly the hole in plan.
            assert!(
                (hole_area(&d) - roofs_plan_area(&d)).abs() < 1e-6,
                "{kind:?}: hole {} roofs {}",
                hole_area(&d),
                roofs_plan_area(&d)
            );
            // Every roof-plane vertex lies in or on the hole outline.
            let hole = geom::ccw(&d.hole_in_main_roof.outline);
            for p in d.roof_planes.iter().flat_map(|p| p.polygon3d.iter()) {
                let q = geom::to_plan(*p);
                assert!(
                    plan_core::geometry::point_in_polygon(q, &hole)
                        || geom::boundary_dist(q, &hole) < 1e-6
                );
            }
            // The roof planes meet the main roof along the hole (valleys): the
            // wall/roof vertices on the main plane agree with it.
            for p in d.roof_planes.iter().flat_map(|p| p.polygon3d.iter()) {
                let q = geom::to_plan(*p);
                let on_main = main.height_at(q).unwrap();
                assert!(
                    on_main <= p[1] + 1e-6,
                    "dormer roof dips below the main roof"
                );
            }
            // Applying the hole to the main plane works.
            let r = crate::roof_plane_with_holes(&main, std::slice::from_ref(&d.hole_in_main_roof));
            assert_eq!(r.holes.len(), 1, "{kind:?}");
        }
    }

    #[test]
    fn hip_and_shed_plane_counts() {
        let main = south();
        let hip = auto_dormer(&main, spec(DormerKind::Hip)).unwrap();
        assert_eq!(hip.roof_planes.len(), 3);
        let shed = auto_dormer(&main, spec(DormerKind::Shed)).unwrap();
        assert_eq!(shed.roof_planes.len(), 1);
        // Shed roof is flatter than the main roof.
        assert!(shed.roof_planes[0].pitch_in_12 < main.pitch_in_12);
        // An 8:12 request is not flatter than the 8:12 main roof: halved.
        assert!((shed.roof_planes[0].pitch_in_12 - 4.0).abs() < 1e-6);
    }

    #[test]
    fn window_sits_on_the_front_wall_and_explode_returns_all_parts() {
        let main = south();
        let d = auto_dormer(&main, spec(DormerKind::Gable)).unwrap();
        let win = d.window_opening.clone().unwrap();
        assert_eq!(win.polygon3d.len(), 4);
        assert!((win.sill_height - 6.0).abs() < 1e-9);
        for p in &win.polygon3d {
            assert!((geom::to_plan(*p).y - 30.0).abs() < 1e-6);
        }
        let x = explode_dormer(&d);
        assert_eq!(x.walls.len(), 3);
        assert_eq!(x.roof_planes.len(), 2);
        assert_eq!(x.hole, d.hole_in_main_roof);
        assert!(x.window_opening.is_some());
    }

    #[test]
    fn a_dormer_stored_before_overhangs_loads_flush() {
        let old = r#"{"kind":"Gable","width":72.0,"height_to_ridge":0.0,"wall_height":36.0,"position_along_eave":240.0,"setback_from_eave":30.0,"pitch":8.0,"window":null}"#;
        let spec: DormerSpec = serde_json::from_str(old).unwrap();
        assert_eq!(spec.overhang, 0.0);
        let back: DormerSpec = serde_json::from_str(
            &serde_json::to_string(&DormerSpec {
                overhang: 9.0,
                ..spec
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(back.overhang, 9.0);
    }

    #[test]
    fn no_overhang_leaves_the_planes_as_they_are() {
        let main = south();
        for kind in [DormerKind::Gable, DormerKind::Hip, DormerKind::Shed] {
            let d = auto_dormer(&main, spec(kind)).unwrap();
            assert_eq!(d.overhang_planes, d.roof_planes, "{kind:?}");
        }
    }

    #[test]
    fn a_gable_dormer_overhangs_its_eaves_and_front_rake() {
        let main = south();
        let mut sp = spec(DormerKind::Gable);
        sp.overhang = 12.0;
        let d = auto_dormer(&main, sp).unwrap();
        assert_eq!(d.overhang_planes.len(), 2);
        let plan = |p: &RoofPlane| p.plan_polygon();
        let xs = |p: &RoofPlane| plan(p).iter().map(|q| q.x).collect::<Vec<_>>();
        // The dormer is 72 wide centred on 240: walls at x = 204 and 276.
        let min_x = d
            .overhang_planes
            .iter()
            .flat_map(xs)
            .fold(f64::MAX, f64::min);
        let max_x = d
            .overhang_planes
            .iter()
            .flat_map(xs)
            .fold(f64::MIN, f64::max);
        assert!((min_x - 192.0).abs() < 1e-6, "{min_x}");
        assert!((max_x - 288.0).abs() < 1e-6, "{max_x}");
        // The front rake and ridge reach 12" past the front wall (y = 30).
        let min_y = d
            .overhang_planes
            .iter()
            .flat_map(|p| plan(p).into_iter().map(|q| q.y))
            .fold(f64::MAX, f64::min);
        assert!((min_y - 18.0).abs() < 1e-6, "{min_y}");
        // Every corner stays on its plane, which keeps its pitch, and the
        // ridge is still level and shared.
        for (o, n) in d.roof_planes.iter().zip(&d.overhang_planes) {
            assert!((o.pitch_in_12 - n.pitch_in_12).abs() < 1e-6);
            let normal = n.normal();
            for v in &n.polygon3d {
                let off = geom::sub3(*v, o.polygon3d[0]);
                assert!(geom::dot3(off, normal).abs() < 1e-6);
            }
            assert!(n.normal()[1] > 0.0);
            assert!(n.projected_area() > o.projected_area());
        }
        // One valley edge per plane, on the main roof.
        assert_eq!(d.valley_edges.len(), 2);
        for &(k, e) in &d.valley_edges {
            let p = &d.overhang_planes[k];
            for v in [p.polygon3d[e], p.polygon3d[(e + 1) % p.polygon3d.len()]] {
                let y = main.height_at(geom::to_plan(v)).unwrap();
                assert!((y - v[1]).abs() < 1e-6, "valley corner off the main roof");
            }
        }
        // The hole is still the wall footprint.
        let plain = auto_dormer(&main, spec(DormerKind::Gable)).unwrap();
        assert_eq!(d.hole_in_main_roof, plain.hole_in_main_roof);
    }

    #[test]
    fn hip_and_shed_dormers_overhang_on_every_free_edge() {
        let main = south();
        for kind in [DormerKind::Hip, DormerKind::Shed] {
            let mut sp = spec(kind);
            sp.overhang = 10.0;
            let d = auto_dormer(&main, sp).unwrap();
            let plain = auto_dormer(&main, spec(kind)).unwrap();
            let area = |d: &Dormer| d.overhang_planes.iter().map(RoofPlane::area).sum::<f64>();
            assert!(area(&d) > area(&plain) + 100.0, "{kind:?}");
            assert!(d.overhang_planes.iter().all(|p| p.normal()[1] > 0.0));
            // Hip planes still meet along the same hip lines: planes share
            // the corners the overhang left on them.
            if kind == DormerKind::Hip {
                let front = &d.overhang_planes[2];
                let left = &d.overhang_planes[0];
                let shared = front
                    .polygon3d
                    .iter()
                    .filter(|v| {
                        left.polygon3d
                            .iter()
                            .any(|w| geom::sub3(**v, *w).iter().all(|c| c.abs() < 1e-6))
                    })
                    .count();
                assert_eq!(shared, 2, "the hip is one shared edge");
            }
        }
    }

    #[test]
    fn dormers_that_do_not_fit_are_rejected() {
        let main = south();
        let mut sp = spec(DormerKind::Gable);
        sp.setback_from_eave = 0.0; // front wall on the eave edge
        assert!(auto_dormer(&main, sp).is_none());
        let mut sp = spec(DormerKind::Gable);
        sp.position_along_eave = 10.0; // hangs off the end
        assert!(auto_dormer(&main, sp).is_none());
        let mut sp = spec(DormerKind::Gable);
        sp.setback_from_eave = 170.0; // over the ridge
        assert!(auto_dormer(&main, sp).is_none());
        let mut sp = spec(DormerKind::Gable);
        sp.width = 0.0;
        assert!(auto_dormer(&main, sp).is_none());
    }

    // ----- second pitch, room, returns, crickets -----

    fn gambrel_spec() -> DormerSpec {
        DormerSpec {
            pitch: 12.0,
            window: None,
            ..spec(DormerKind::Gable)
        }
    }

    const SECOND: SecondPitch = SecondPitch {
        pitch2: 4.0,
        in_from_eave: 18.0,
    };

    #[test]
    fn a_gambrel_dormer_has_two_pitches_and_a_ridge_where_the_upper_one_ends() {
        let main = south();
        let d = gambrel_dormer(&main, gambrel_spec(), SECOND).expect("fits the plane");
        assert_eq!(d.roof_planes.len(), 4);
        let mut pitches: Vec<f64> = d.roof_planes.iter().map(|p| p.pitch_in_12).collect();
        pitches.sort_by(f64::total_cmp);
        for (got, want) in pitches.iter().zip([4.0, 4.0, 12.0, 12.0]) {
            assert!((got - want).abs() < 1e-6, "{pitches:?}");
        }
        // Wall 36 + 18 at 12:12 + 18 at 4:12 = 60 above the front base; the
        // main roof is 108 + 30 * 8/12 = 128 there.
        assert!(
            (d.ridge_elevation - (128.0 + 60.0)).abs() < 1e-6,
            "{}",
            d.ridge_elevation
        );
        // Lower and upper planes meet at the same height along the break.
        let top = |p: &RoofPlane| p.polygon3d.iter().map(|v| v[1]).fold(f64::MIN, f64::max);
        let lower_top = top(&d.roof_planes[0]);
        assert!((lower_top - (128.0 + 54.0)).abs() < 1e-6);
        assert!((top(&d.roof_planes[1]) - d.ridge_elevation).abs() < 1e-6);
        // Each side meets the main roof along two valleys.
        assert_eq!(d.valley_edges.len(), 4);
        for (k, e) in &d.valley_edges {
            let p = &d.roof_planes[*k].polygon3d;
            for v in [p[*e], p[(*e + 1) % p.len()]] {
                let y = main.height_at(geom::to_plan(v)).unwrap();
                assert!((y - v[1]).abs() < 1e-6, "valley point off the main roof");
            }
        }
        // The opening is the plan of the dormer: 7 corners, inside the plane.
        assert_eq!(d.hole_in_main_roof.outline.len(), 7);
        assert!(d.front_wall.polygon3d.len() == 7);
    }

    #[test]
    fn the_gambrel_break_must_lie_between_the_eave_and_the_ridge() {
        let main = south();
        for inn in [0.0, 36.0, 50.0, -3.0] {
            let second = SecondPitch {
                in_from_eave: inn,
                ..SECOND
            };
            assert!(
                gambrel_dormer(&main, gambrel_spec(), second).is_none(),
                "{inn}"
            );
        }
        let flat = SecondPitch {
            pitch2: 0.0,
            ..SECOND
        };
        assert!(gambrel_dormer(&main, gambrel_spec(), flat).is_none());
        // Too far up the slope: the upper part would pass the ridge of the main roof.
        let mut high = gambrel_spec();
        high.setback_from_eave = 160.0;
        assert!(gambrel_dormer(&main, high, SECOND).is_none());
    }

    #[test]
    fn the_dormer_shaft_runs_plumb_to_the_floor_below_only_when_asked() {
        let main = south();
        let d = auto_dormer(&main, spec(DormerKind::Gable)).unwrap();
        assert!(dormer_shaft(&main, &d, &DormerRoom::default(), 0.0).is_empty());
        let room = DormerRoom {
            create_shaft: true,
            ..DormerRoom::default()
        };
        let walls = dormer_shaft(&main, &d, &room, 0.0);
        assert_eq!(walls.len(), d.hole_in_main_roof.outline.len());
        for w in &walls {
            assert!(
                (w[3][1]).abs() < 1e-9 && (w[2][1]).abs() < 1e-9,
                "to the floor"
            );
            assert!((w[0][0] - w[3][0]).abs() < 1e-9 && (w[0][2] - w[3][2]).abs() < 1e-9);
            assert!(w[0][1] > 100.0, "starts on the roof");
        }
    }

    #[test]
    fn a_dormer_room_takes_the_existing_ceiling_when_told_to() {
        let main = south();
        let d = auto_dormer(&main, spec(DormerKind::Gable)).unwrap();
        let own = d.front_wall.base_elevation + 36.0;
        let mut room = DormerRoom::default();
        assert_eq!(dormer_room_ceiling(&d, &room, Some(96.0)), own);
        room.set_to_existing_ceiling = true;
        assert_eq!(dormer_room_ceiling(&d, &room, Some(96.0)), 96.0);
        assert_eq!(
            dormer_room_ceiling(&d, &room, None),
            own,
            "no ceiling to follow"
        );
    }

    #[test]
    fn auto_roof_return_wraps_the_front_corners_of_a_gable_dormer_only() {
        use crate::gable::{ReturnKind, ReturnSpec};
        let main = south();
        let ret = ReturnSpec {
            kind: ReturnKind::Full,
            length: 12.0,
        };
        let mut sp = spec(DormerKind::Gable);
        sp.overhang = 8.0;
        let d = auto_dormer(&main, sp).unwrap();
        let rs = dormer_returns(&d, ret);
        assert_eq!(rs.len(), 2, "one at each front eave corner");
        let front = Point::new(d.front_wall.normal[0], -d.front_wall.normal[2]);
        for r in &rs {
            // The corner is at the front of the dormer roof and the return
            // projects out past it.
            let c = geom::to_plan(r.corner);
            assert!(r.plane.polygon3d.iter().all(|v| v[1] >= r.corner[1] - 1e-6));
            assert!(r
                .plane
                .plan_polygon()
                .iter()
                .any(|q| q.sub(c).dot(front) > 1.0));
        }
        let boxed = dormer_returns(
            &d,
            ReturnSpec {
                kind: ReturnKind::Boxed,
                length: 12.0,
            },
        );
        assert!(boxed.iter().all(|r| r.plane.pitch_in_12 == 0.0));
        for k in [DormerKind::Hip, DormerKind::Shed] {
            let other = auto_dormer(&main, spec(k)).unwrap();
            assert!(dormer_returns(&other, ret).is_empty(), "{k:?}");
        }
    }

    #[test]
    fn a_cricket_is_two_triangles_on_a_level_ridge_that_meets_the_roof() {
        let main = south();
        // A 48" wide chimney face across the slope at y = 60.
        let (a, b) = (Point::new(216.0, 60.0), Point::new(264.0, 60.0));
        let c = cricket_behind(&main, a, b, None).expect("a cricket fits");
        // Half the roof pitch: 4:12 over 24" half width is 8" high.
        assert!((c.pitch_in_12 - 4.0).abs() < 1e-9);
        assert!((c.height - 8.0).abs() < 1e-9);
        // The ridge is level and the roof (8:12) climbs 8" in 12".
        assert!((c.length - 12.0).abs() < 1e-9);
        assert!((c.ridge.0[1] - c.ridge.1[1]).abs() < 1e-9);
        let at_wall = main.height_at(Point::new(240.0, 60.0)).unwrap();
        assert!((c.ridge.0[1] - (at_wall + 8.0)).abs() < 1e-9);
        let end = geom::to_plan(c.ridge.1);
        assert!(
            (end.sub(Point::new(240.0, 72.0)).length()) < 1e-9,
            "12\" up-slope"
        );
        let roof_there = main.height_at(end).unwrap();
        assert!(
            (roof_there - c.ridge.1[1]).abs() < 1e-9,
            "the ridge ends on the roof"
        );
        for pl in &c.planes {
            assert_eq!(pl.polygon3d.len(), 3);
            assert!(pl.normal()[1] > 0.0);
            assert!((pl.pitch_in_12 - 4.0).abs() < 1e-6, "{}", pl.pitch_in_12);
        }
        // One plane each side of the ridge.
        let side = |pl: &RoofPlane| {
            let cx = pl.polygon3d.iter().map(|v| v[0]).sum::<f64>() / 3.0;
            cx - 240.0
        };
        assert!(side(&c.planes[0]) * side(&c.planes[1]) < 0.0);
        // The two outer corners are on the roof at the ends of the face.
        for (pl, x) in c.planes.iter().zip([216.0, 264.0]) {
            let on_roof = pl.polygon3d.iter().any(|v| {
                (v[0] - x).abs() < 1e-9
                    && (main.height_at(geom::to_plan(*v)).unwrap() - v[1]).abs() < 1e-9
            });
            assert!(on_roof, "corner at x = {x}");
        }
    }

    #[test]
    fn a_cricket_needs_a_wide_enough_face_and_a_gentler_pitch_than_the_roof() {
        let main = south();
        let (a, b) = (Point::new(216.0, 60.0), Point::new(264.0, 60.0));
        assert!(cricket_behind(&main, a, Point::new(218.0, 60.0), None).is_none());
        assert!(
            cricket_behind(&main, a, b, Some(8.0)).is_none(),
            "as steep as the roof"
        );
        assert!(cricket_behind(&main, a, b, Some(0.0)).is_none());
        // Steeper than half the roof pitch makes a taller, shorter cricket.
        let c = cricket_behind(&main, a, b, Some(6.0)).unwrap();
        assert!((c.height - 12.0).abs() < 1e-9 && (c.length - 18.0).abs() < 1e-9);
        // Outside the plane's slope direction there is nothing to build on.
        let flat = RoofPlane {
            polygon3d: vec![
                [0.0, 100.0, 0.0],
                [100.0, 100.0, 0.0],
                [100.0, 100.0, -100.0],
                [0.0, 100.0, -100.0],
            ],
            pitch_in_12: 0.0,
            baseline: (Point::new(0.0, 0.0), Point::new(100.0, 0.0)),
            source_edge: 0,
        };
        assert!(
            cricket_behind(&flat, Point::new(20.0, 50.0), Point::new(60.0, 50.0), None).is_none()
        );
    }
}
