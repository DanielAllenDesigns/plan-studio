//! Molding sweeps: a profile swept along a 3D polyline with mitred joints
//! (Molding Line, Molding Polyline, the room moldings converted to
//! polylines, stacked and recessed profiles, 3D moldings that repeat an
//! element along the path).
//!
//! # The sweep
//!
//! The path is a list of 3D points (plan x, plan y, elevation of the
//! molding's bottom edge). Every edge has a frame: `d` along the edge, `n`
//! the horizontal vector to its left (the side the profile projects to) and
//! `u = d x n`, which points up for a level edge and stays square to the
//! edge for a sloped one. A profile point `(x, y)` stands at
//! `vertex + n x + u y`, lifted by the part's vertical offset.
//!
//! At a joint the two sections are brought into the mitre plane, the plane
//! through the vertex whose normal is the bisector of the two edge
//! directions: each section point slides along its own edge until it lies in
//! that plane. For a corner in a level plan this is the usual mitre; for
//! edges that rise and fall in one vertical plane the two sections agree
//! exactly. At a twisted joint (edges not in one plane) the points differ
//! and are averaged (Auto Calc Orientation at Twisted Joints), kept as the
//! previous edge has them (auto-calc off) or cut square with no mitre
//! (Mitre Molding at Twisted Joints off).
//!
//! An edge switched off ends the run before it; with Mitre Molding If Next
//! Edge Turned Off the end is still cut on the mitre plane.

use crate::builder::{MeshBuilder, MeshSet, V3};
use crate::mesh::{Material, Mesh};
use crate::triangulate::ear_clip;
use plan_core::details::{DetailsLayer, MoldingLine, SweepPath};
use plan_core::geometry::{polygon_area, Point};
use plan_core::moldings::{extended_room_lines, repeat_centers};
use plan_core::{Floor, Room};

const IN_PER_FT: f64 = 12.0;
/// The least cosine between a joint's bisector and an edge direction the
/// mitre follows; sharper corners are cut off there.
const MITER_LIMIT: f64 = 0.25;
/// Two points of a joint closer than this are the same, inches.
const TWIST_TOLERANCE: f64 = 0.05;

type V = [f64; 3];

fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: V, k: f64) -> V {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn len(a: V) -> f64 {
    dot(a, a).sqrt()
}
fn norm(a: V) -> V {
    let l = len(a);
    if l < 1e-12 {
        [0.0; 3]
    } else {
        scale(a, 1.0 / l)
    }
}

/// Plan-space (x, y, up) to scene space (x, up, -y).
fn scene(p: V) -> V3 {
    [p[0] as f32, p[2] as f32, (-p[1]) as f32]
}

fn scene_dir(p: V) -> V3 {
    scene(p)
}

/// How joints behave.
#[derive(Debug, Clone, Copy)]
struct Joints {
    auto_orient: bool,
    mitre_twisted: bool,
    mitre_if_next_off: bool,
}

/// A run of consecutive edges the molding is on.
#[derive(Debug, Clone, PartialEq)]
struct Run {
    pts: Vec<V>,
    closed: bool,
    /// Direction of the edge arriving at the start when it is switched off.
    before: Option<V>,
    /// Direction of the edge leaving the end when it is switched off.
    after: Option<V>,
}

struct Frame {
    d: V,
    n: V,
    u: V,
}

fn frames(pts: &[V], closed: bool) -> Vec<Frame> {
    let m = pts.len();
    let segs = if closed { m } else { m - 1 };
    let mut ds: Vec<V> = Vec::with_capacity(segs);
    let mut ns: Vec<Option<V>> = Vec::with_capacity(segs);
    for i in 0..segs {
        let d = norm(sub(pts[(i + 1) % m], pts[i]));
        let flat = (d[0] * d[0] + d[1] * d[1]).sqrt();
        ns.push((flat > 1e-9).then(|| [-d[1] / flat, d[0] / flat, 0.0]));
        ds.push(d);
    }
    // A vertical edge has no left of its own: it takes its neighbour's.
    let known: Vec<usize> = (0..segs).filter(|i| ns[*i].is_some()).collect();
    let fallback = [1.0, 0.0, 0.0];
    let resolved: Vec<V> = (0..segs)
        .map(|i| {
            ns[i].unwrap_or_else(|| {
                let prev = (1..=segs).map(|k| (i + segs - k) % segs).find(|j| known.contains(j));
                prev.and_then(|j| ns[j]).unwrap_or(fallback)
            })
        })
        .collect();
    (0..segs)
        .map(|i| Frame {
            d: ds[i],
            n: resolved[i],
            u: norm(cross(ds[i], resolved[i])),
        })
        .collect()
}

/// The section point `(x, y)` at `vertex` in the frame `f`, lifted by `dz`.
fn at_vertex(vertex: V, f: &Frame, p: Point, dz: f64) -> V {
    add(
        add(vertex, [0.0, 0.0, dz]),
        add(scale(f.n, p.x), scale(f.u, p.y)),
    )
}

/// `pt` moved along `d` into the plane through `origin` with normal `m`.
fn into_plane(pt: V, d: V, origin: V, m: V) -> V {
    let denom = dot(d, m);
    let denom = if denom >= 0.0 {
        denom.max(MITER_LIMIT)
    } else {
        denom.min(-MITER_LIMIT)
    };
    sub(pt, scale(d, dot(sub(pt, origin), m) / denom))
}

/// Sweeps `section` along `run`, adding to `mb`.
fn sweep_run(mb: &mut MeshBuilder, run: &Run, section: &[Point], dz: f64, joints: Joints) {
    let mut pts: Vec<V> = Vec::with_capacity(run.pts.len());
    for p in &run.pts {
        if pts.last().is_none_or(|q| len(sub(*p, *q)) > 1e-9) {
            pts.push(*p);
        }
    }
    let mut closed = run.closed;
    if closed && pts.len() > 1 && len(sub(pts[0], pts[pts.len() - 1])) < 1e-9 {
        pts.pop();
    }
    if pts.len() < 2 || (closed && pts.len() < 3) {
        closed = false;
        if pts.len() < 2 {
            return;
        }
    }
    let m = pts.len();
    let fr = frames(&pts, closed);
    let segs = fr.len();
    let k = section.len();
    // Rings: section points of segment s at its start and at its end.
    let mut start_ring: Vec<Vec<V>> = vec![Vec::new(); segs];
    let mut end_ring: Vec<Vec<V>> = vec![Vec::new(); segs];
    let mut cap_start = vec![None::<V>; segs];
    let mut cap_end = vec![None::<V>; segs];
    let ring_of = |s: usize, vertex: V, m_plane: V| -> Vec<V> {
        section
            .iter()
            .map(|p| {
                let raw = at_vertex(vertex, &fr[s], *p, dz);
                into_plane(raw, fr[s].d, add(vertex, [0.0, 0.0, dz]), m_plane)
            })
            .collect()
    };
    for j in 0..m {
        let prev = if j > 0 {
            Some(j - 1)
        } else if closed {
            Some(segs - 1)
        } else {
            None
        };
        let next = if j < segs { Some(j) } else { None };
        let v = pts[j];
        match (prev, next) {
            (Some(a), Some(b)) => {
                let sum = add(fr[a].d, fr[b].d);
                if len(sum) < 1e-6 {
                    // A reversal: both ends are cut square.
                    end_ring[a] = ring_of(a, v, fr[a].d);
                    start_ring[b] = ring_of(b, v, fr[b].d);
                    cap_end[a] = Some(fr[a].d);
                    cap_start[b] = Some(scale(fr[b].d, -1.0));
                    continue;
                }
                let mp = norm(sum);
                let ra = ring_of(a, v, mp);
                let rb = ring_of(b, v, mp);
                let twisted = ra
                    .iter()
                    .zip(&rb)
                    .any(|(x, y)| len(sub(*x, *y)) > TWIST_TOLERANCE);
                if twisted && !joints.mitre_twisted {
                    end_ring[a] = ring_of(a, v, fr[a].d);
                    start_ring[b] = ring_of(b, v, fr[b].d);
                    cap_end[a] = Some(fr[a].d);
                    cap_start[b] = Some(scale(fr[b].d, -1.0));
                } else if twisted && joints.auto_orient {
                    let avg: Vec<V> = ra
                        .iter()
                        .zip(&rb)
                        .map(|(x, y)| scale(add(*x, *y), 0.5))
                        .collect();
                    end_ring[a] = avg.clone();
                    start_ring[b] = avg;
                } else {
                    end_ring[a] = ra.clone();
                    start_ring[b] = if twisted { ra } else { rb };
                }
            }
            (None, Some(b)) => {
                let mp = match run.before.filter(|_| joints.mitre_if_next_off) {
                    Some(e) if len(add(e, fr[b].d)) > 1e-6 => norm(add(e, fr[b].d)),
                    _ => fr[b].d,
                };
                start_ring[b] = ring_of(b, v, mp);
                cap_start[b] = Some(scale(mp, -1.0));
            }
            (Some(a), None) => {
                let mp = match run.after.filter(|_| joints.mitre_if_next_off) {
                    Some(e) if len(add(e, fr[a].d)) > 1e-6 => norm(add(e, fr[a].d)),
                    _ => fr[a].d,
                };
                end_ring[a] = ring_of(a, v, mp);
                cap_end[a] = Some(mp);
            }
            (None, None) => {}
        }
    }
    let tris = ear_clip(section);
    for s in 0..segs {
        let (r0, r1) = (&start_ring[s], &end_ring[s]);
        if r0.len() != k || r1.len() != k {
            continue;
        }
        let seg_len = len(sub(pts[(s + 1) % m], pts[s]));
        let u = (seg_len / IN_PER_FT) as f32;
        for i in 0..k {
            let j = (i + 1) % k;
            let e = section[j] - section[i];
            let l = e.length();
            if l <= 1e-9 {
                continue;
            }
            // Outward normal of a counter-clockwise section edge.
            let (nx, ny) = (e.y / l, -e.x / l);
            let normal = norm(add(scale(fr[s].n, nx), scale(fr[s].u, ny)));
            let (v0, v1) = (
                ((section[i].y + dz) / IN_PER_FT) as f32,
                ((section[j].y + dz) / IN_PER_FT) as f32,
            );
            mb.quad(
                [scene(r0[i]), scene(r1[i]), scene(r1[j]), scene(r0[j])],
                [[0.0, v0], [u, v0], [u, v1], [0.0, v1]],
                scene_dir(normal),
            );
        }
        for (ring, cap) in [(r0, cap_start[s]), (r1, cap_end[s])] {
            let Some(normal) = cap else { continue };
            for t in &tris {
                let p = t.map(|i| scene(ring[i]));
                let uv = t.map(|i| {
                    [
                        (section[i].x / IN_PER_FT) as f32,
                        ((section[i].y + dz) / IN_PER_FT) as f32,
                    ]
                });
                mb.tri(p, uv, scene_dir(normal));
            }
        }
    }
}

/// The edges of `path` the molding is on, as runs of consecutive edges.
fn runs_of(path: &SweepPath, floor_elev: f64) -> Vec<Run> {
    let n = path.points.len();
    if n < 2 {
        return Vec::new();
    }
    let p3: Vec<V> = (0..n)
        .map(|i| [path.points[i].x, path.points[i].y, floor_elev + path.bottoms[i]])
        .collect();
    let edges = n - 1;
    let on = |i: usize| path.on.get(i).copied().unwrap_or(true);
    if (0..edges).all(on) {
        return vec![Run {
            pts: if path.closed {
                p3[..n - 1].to_vec()
            } else {
                p3
            },
            closed: path.closed,
            before: None,
            after: None,
        }];
    }
    // A closed path starts just after an edge that is off, so no run wraps.
    let start = if path.closed {
        (0..edges).find(|i| !on(*i)).map_or(0, |f| f + 1)
    } else {
        0
    };
    let order: Vec<usize> = (0..edges).map(|k| (start + k) % edges).collect();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut group: Vec<usize> = Vec::new();
    for &e in &order {
        if on(e) {
            group.push(e);
        } else if !group.is_empty() {
            groups.push(std::mem::take(&mut group));
        }
    }
    if !group.is_empty() {
        groups.push(group);
    }
    let edge_dir = |e: usize| norm(sub(p3[e + 1], p3[e]));
    groups
        .into_iter()
        .map(|g| {
            let first = g[0];
            let last = g[g.len() - 1];
            let mut pts = vec![p3[first]];
            pts.extend(g.iter().map(|e| p3[e + 1]));
            let before = if first == 0 && !path.closed {
                None
            } else {
                Some(edge_dir((first + edges - 1) % edges))
            };
            let after = if last + 1 >= edges && !path.closed {
                None
            } else {
                Some(edge_dir((last + 1) % edges))
            };
            Run {
                pts,
                closed: false,
                before,
                after,
            }
        })
        .collect()
}

/// The part of a run's polyline between arc lengths `a` and `b`.
fn sub_path3(pts: &[V], closed: bool, a: f64, b: f64) -> Vec<V> {
    let mut all = pts.to_vec();
    if closed {
        all.push(pts[0]);
    }
    let (a, b) = (a.min(b).max(0.0), a.max(b));
    let mut out: Vec<V> = Vec::new();
    let mut at = 0.0;
    for s in all.windows(2) {
        let l = len(sub(s[1], s[0]));
        let (s0, s1) = (at, at + l);
        at = s1;
        if l < 1e-9 || s1 <= a || s0 >= b {
            continue;
        }
        let lerp = |t: f64| add(s[0], scale(sub(s[1], s[0]), t));
        let (pa, pb) = (lerp(((a - s0) / l).clamp(0.0, 1.0)), lerp(((b - s0) / l).clamp(0.0, 1.0)));
        if out.last().is_none_or(|q| len(sub(*q, pa)) > 1e-9) {
            out.push(pa);
        }
        if out.last().is_none_or(|q| len(sub(*q, pb)) > 1e-9) {
            out.push(pb);
        }
    }
    out
}

fn run_length(run: &Run) -> f64 {
    let mut total: f64 = run.pts.windows(2).map(|s| len(sub(s[1], s[0]))).sum();
    if run.closed && run.pts.len() > 1 {
        total += len(sub(run.pts[0], run.pts[run.pts.len() - 1]));
    }
    total
}

/// Everything a molding line adds to the scene: one mesh per material.
/// `floor_elev` is the elevation of the floor the line stands on (the line's
/// own heights are above it).
pub fn molding_meshes(m: &MoldingLine, floor_elev: f64) -> Vec<Mesh> {
    let path = m.sweep_path();
    let parts: Vec<_> = m
        .placed_parts()
        .into_iter()
        .filter(|p| p.section.len() >= 3 && polygon_area(&p.section).abs() > 1e-9)
        .collect();
    if path.points.len() < 2 || parts.is_empty() || m.height <= 0.0 && m.table.is_empty() {
        return Vec::new();
    }
    let joints = Joints {
        auto_orient: m.auto_orient,
        mitre_twisted: m.mitre_twisted,
        mitre_if_next_off: m.mitre_if_next_off,
    };
    let runs = runs_of(&path, floor_elev);
    let mut set = MeshSet::default();
    for part in &parts {
        let material = crate::details::material_of(&part.material, Material::Trim);
        let mb = set.material(material);
        for run in &runs {
            if part.repeat > 1e-9 {
                let total = run_length(run);
                let e = part.element.clamp(0.01, part.repeat.max(0.01));
                for c in repeat_centers(total, part.repeat) {
                    let sub = sub_path3(&run.pts, run.closed, c - e * 0.5, c + e * 0.5);
                    if sub.len() >= 2 {
                        let piece = Run {
                            pts: sub,
                            closed: false,
                            before: None,
                            after: None,
                        };
                        sweep_run(mb, &piece, &part.section, part.dz, joints);
                    }
                }
            } else {
                sweep_run(mb, run, &part.section, part.dz, joints);
            }
        }
    }
    set.finish(Some(m.id))
}

/// The finished floor above the floor datum and the finished ceiling above
/// the finished floor of `room` on `floor`, inches: where its moldings stand.
pub fn room_datum_and_ceiling(floor: &Floor, room: &Room) -> (f64, f64) {
    let levels = crate::slab::room_levels(floor, room);
    (
        levels.floor_offset + levels.floor_finish,
        levels.ceiling_height - levels.floor_finish,
    )
}

/// The meshes of the moldings the Moldings panel generates for the rooms of
/// `floor` (their own tables and the Floor Defaults; the older Moldings tab
/// is built with the room).
pub(crate) fn generated_room_meshes(floor: &Floor, layer: &DetailsLayer) -> Vec<Mesh> {
    if layer.floor_moldings.is_empty()
        && layer.room_moldings.is_empty()
        && layer.type_moldings.is_empty()
    {
        return Vec::new();
    }
    let mut out = Vec::new();
    for room in plan_core::detect_rooms(&floor.walls, 0.5) {
        let levels = crate::slab::room_levels(floor, &room);
        if !levels.has_ceiling && layer.floor_moldings.is_empty() {
            continue;
        }
        let datum = levels.floor_offset + levels.floor_finish;
        let ceiling = levels.ceiling_height - levels.floor_finish;
        for line in extended_room_lines(floor, layer, &room, ceiling, datum) {
            out.extend(molding_meshes(&line, floor.elevation));
        }
    }
    out
}

/// Sweeps one section along a plain path (a helper for callers that hold a
/// polyline and a profile but no [`MoldingLine`]): level points `(x, y)` at
/// `elevation`, the section projecting to the left.
pub fn sweep_mesh(path: &[Point], closed: bool, section: &[Point], elevation: f64) -> Option<Mesh> {
    if path.len() < 2 || section.len() < 3 {
        return None;
    }
    let mut b = MeshBuilder::new(Material::Trim);
    let run = Run {
        pts: path.iter().map(|p| [p.x, p.y, elevation]).collect(),
        closed,
        before: None,
        after: None,
    };
    let joints = Joints {
        auto_orient: true,
        mitre_twisted: true,
        mitre_if_next_off: false,
    };
    sweep_run(&mut b, &run, section, 0.0, joints);
    (!b.is_empty()).then(|| b.finish(None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::details::{MoldingProfile, MoldingSide};
    use plan_core::moldings::{
        arc_points, builtin_profiles, EdgeMode, MoldingEntry, MoldingTable, ProfileDef,
    };

    fn pt(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    /// Signed volume of a closed mesh with outward faces, cubic inches.
    fn volume(meshes: &[Mesh]) -> f64 {
        let mut v = 0.0;
        for m in meshes {
            for t in m.indices.chunks(3) {
                let p: Vec<V> = t
                    .iter()
                    .map(|i| {
                        let p = m.vertices[*i as usize].position;
                        [p[0] as f64, p[1] as f64, p[2] as f64]
                    })
                    .collect();
                v += dot(p[0], cross(p[1], p[2])) / 6.0;
            }
        }
        v
    }

    fn bounds(meshes: &[Mesh]) -> ([f32; 3], [f32; 3]) {
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for m in meshes {
            for v in &m.vertices {
                for k in 0..3 {
                    lo[k] = lo[k].min(v.position[k]);
                    hi[k] = hi[k].max(v.position[k]);
                }
            }
        }
        (lo, hi)
    }

    fn box_profile(w: f64, h: f64) -> ProfileDef {
        ProfileDef::from_polyline(
            "box",
            plan_core::moldings::MoldingType::Base,
            &[pt(0.0, 0.0), pt(w, 0.0), pt(w, h), pt(0.0, h)],
        )
        .unwrap()
    }

    fn line(points: Vec<Point>, p: ProfileDef) -> MoldingLine {
        let mut m = MoldingLine::with_profile(1, points, p, 0.0);
        m.side = MoldingSide::Left;
        m
    }

    fn rect(w: f64, d: f64) -> Vec<Point> {
        vec![pt(0.0, 0.0), pt(w, 0.0), pt(w, d), pt(0.0, d), pt(0.0, 0.0)]
    }

    #[test]
    fn a_rectangle_is_swept_with_mitred_corners() {
        // 100 x 60, section 2 x 4, projecting inside (counter-clockwise).
        let m = line(rect(100.0, 60.0), box_profile(2.0, 4.0));
        let meshes = molding_meshes(&m, 0.0);
        assert_eq!(meshes.len(), 1);
        let want = (100.0 * 60.0 - 96.0 * 56.0) * 4.0;
        assert!((volume(&meshes) - want).abs() < 1e-3 * want, "{}", volume(&meshes));
        // No end caps on a closed ring: 4 segments x 4 section edges x 2.
        assert_eq!(meshes[0].triangle_count(), 4 * 8);
        // The inner mitre points are at (2, 2) and (98, 58) and the molding
        // stays inside the path.
        let (lo, hi) = bounds(&meshes);
        assert!(lo[0].abs() < 1e-4 && (hi[0] - 100.0).abs() < 1e-4);
        assert!(hi[2].abs() < 1e-4 && (lo[2] + 60.0).abs() < 1e-4);
        assert!((hi[1] - 4.0).abs() < 1e-4 && lo[1].abs() < 1e-4);
    }

    #[test]
    fn an_open_run_has_end_caps_and_the_volume_of_a_straight_piece() {
        let m = line(vec![pt(0.0, 0.0), pt(96.0, 0.0)], box_profile(1.0, 5.0));
        let meshes = molding_meshes(&m, 0.0);
        let want = 96.0 * 5.0;
        assert!((volume(&meshes) - want).abs() < 1e-3, "{}", volume(&meshes));
        // Left of +x is +plan y, which is -z in the scene.
        let (lo, hi) = bounds(&meshes);
        assert!((lo[2] + 1.0).abs() < 1e-4 && hi[2].abs() < 1e-4);
    }

    #[test]
    fn a_profile_with_a_notch_is_capped_correctly() {
        // An L-shaped (concave) section: area 3 x 1 + 1 x 2 = 5.
        let l = ProfileDef::from_polyline(
            "L",
            plan_core::moldings::MoldingType::Crown,
            &[
                pt(0.0, 0.0),
                pt(3.0, 0.0),
                pt(3.0, 1.0),
                pt(1.0, 1.0),
                pt(1.0, 3.0),
                pt(0.0, 3.0),
            ],
        )
        .unwrap();
        let m = line(vec![pt(0.0, 0.0), pt(10.0, 0.0)], l);
        let meshes = molding_meshes(&m, 0.0);
        assert!((volume(&meshes) - 5.0 * 10.0).abs() < 1e-3, "{}", volume(&meshes));
    }

    #[test]
    fn a_sweep_along_an_arc_follows_the_radius() {
        // Counter-clockwise arc of radius 48, 90 degrees; the section
        // projects toward the centre.
        let arc = arc_points(pt(0.0, 0.0), 48.0, 0.0, 90.0, 48);
        let m = line(arc, box_profile(2.0, 3.0));
        let meshes = molding_meshes(&m, 0.0);
        let want = 3.0 * (std::f64::consts::FRAC_PI_2 / 2.0) * (48.0 * 48.0 - 46.0 * 46.0);
        let v = volume(&meshes);
        assert!((v - want).abs() < 0.01 * want, "{v} vs {want}");
        // The far side of the molding is the arc itself, never beyond it.
        let (lo, hi) = bounds(&meshes);
        assert!(hi[0] <= 48.0 + 1e-3 && lo[0] >= -0.1, "{lo:?} {hi:?}");
        assert!(hi[1] <= 3.0 + 1e-4);
    }

    #[test]
    fn reverse_direction_and_extrude_inside_flip_the_side() {
        // Drawn clockwise, the default side (right) puts the profile inside.
        let cw = vec![pt(0.0, 0.0), pt(0.0, 60.0), pt(100.0, 60.0), pt(100.0, 0.0), pt(0.0, 0.0)];
        let mut m = MoldingLine::with_profile(1, cw.clone(), box_profile(2.0, 4.0), 0.0);
        let inside = (100.0 * 60.0 - 96.0 * 56.0) * 4.0;
        let v = volume(&molding_meshes(&m, 0.0));
        assert!((v - inside).abs() < 1e-3 * inside, "{v}");
        let (lo, hi) = bounds(&molding_meshes(&m, 0.0));
        assert!(lo[0] >= -1e-4 && hi[0] <= 100.0 + 1e-4);
        // Reverse Direction puts it outside.
        m.reverse_direction();
        let (lo, hi) = bounds(&molding_meshes(&m, 0.0));
        assert!(lo[0] <= -1.9 && hi[0] >= 101.9, "{lo:?} {hi:?}");
        // Extrude Inside Polyline puts it inside again, either way.
        m.extrude_inside = true;
        let (lo, hi) = bounds(&molding_meshes(&m, 0.0));
        assert!(lo[0] >= -1e-4 && hi[0] <= 100.0 + 1e-4);
        let mut again = MoldingLine::with_profile(1, cw, box_profile(2.0, 4.0), 0.0);
        again.extrude_inside = true;
        again.side = MoldingSide::Left;
        let (lo, hi) = bounds(&molding_meshes(&again, 0.0));
        assert!(lo[0] >= -1e-4 && hi[0] <= 100.0 + 1e-4);
    }

    #[test]
    fn stacked_and_recessed_profiles_use_their_offsets_and_materials() {
        let mut table = MoldingTable::default();
        table.add_new(box_profile(1.0, 2.0));
        let b = table.add_new(box_profile(1.0, 1.0));
        table.rows[b].h_offset = -0.25;
        table.rows[b].v_offset = 0.5;
        table.rows[b].profile.parts[0].material = "Oak Flooring".into();
        table.make_stack(&[0, 1]);
        let mut m = line(vec![pt(0.0, 0.0), pt(10.0, 0.0)], box_profile(1.0, 2.0));
        m.table = table;
        m.elevation = 30.0;
        let meshes = molding_meshes(&m, 0.0);
        // Two materials: the second keeps its own.
        assert_eq!(meshes.len(), 2);
        assert!(meshes.iter().any(|x| x.material != Material::Trim));
        let (lo, hi) = bounds(&meshes);
        // Starts at the molding's height; the recessed part goes 1/4 behind
        // the back line (toward +z, away from the room) and sits 2.5 up.
        assert!((lo[1] - 30.0).abs() < 1e-4);
        assert!((hi[1] - 33.5).abs() < 1e-4, "{}", hi[1]);
        assert!((lo[2] + 1.0).abs() < 1e-4 && (hi[2] - 0.25).abs() < 1e-4, "{lo:?} {hi:?}");
        // Off for one row removes it.
        m.table.rows[b].edge = EdgeMode::Off;
        assert_eq!(molding_meshes(&m, 0.0).len(), 1);
    }

    #[test]
    fn a_3d_line_rises_and_keeps_its_section_square_to_the_edge() {
        let mut m = line(vec![pt(0.0, 0.0), pt(96.0, 0.0)], box_profile(1.0, 5.0));
        m.set_vertex_bottom(0, 10.0);
        m.set_vertex_bottom(1, 34.0);
        assert!(m.is_sloped());
        let meshes = molding_meshes(&m, 0.0);
        let len3 = (96.0f64 * 96.0 + 24.0 * 24.0).sqrt();
        assert!((volume(&meshes) - 5.0 * len3).abs() < 1e-2, "{}", volume(&meshes));
        let (lo, hi) = bounds(&meshes);
        assert!((lo[1] - 10.0).abs() < 1.5 && hi[1] > 34.0, "{lo:?} {hi:?}");
        // 3D length and angles of the edge.
        assert!((m.edge_length_3d(0) - len3).abs() < 1e-9);
        let (xy, from) = m.edge_angles(0);
        assert!(xy.abs() < 1e-9 && (from - 14.036).abs() < 1e-2, "{from}");
    }

    #[test]
    fn an_edge_switched_off_splits_the_run() {
        let mut m = line(
            vec![pt(0.0, 0.0), pt(50.0, 0.0), pt(100.0, 0.0), pt(150.0, 0.0)],
            box_profile(1.0, 2.0),
        );
        assert!(m.set_edge_on(1, false));
        let meshes = molding_meshes(&m, 0.0);
        let want = 100.0 * 2.0;
        assert!((volume(&meshes) - want).abs() < 1e-3, "{}", volume(&meshes));
        // The gap is empty.
        let n = meshes[0]
            .vertices
            .iter()
            .filter(|v| v.position[0] > 50.5 && v.position[0] < 99.5)
            .count();
        assert_eq!(n, 0);
        assert!((m.edge_lengths_on() - 100.0).abs() < 1e-9);
        // Turning it on again restores the whole line.
        assert!(m.set_edge_on(1, true));
        assert!((volume(&molding_meshes(&m, 0.0)) - 300.0).abs() < 1e-3);
    }

    #[test]
    fn a_repeated_element_is_placed_every_repeat_distance() {
        let mut table = MoldingTable::default();
        let i = table.add_new(box_profile(2.0, 2.0));
        table.rows[i].repeat_distance = 10.0;
        table.rows[i].element_length = 4.0;
        let mut m = line(vec![pt(0.0, 0.0), pt(100.0, 0.0)], box_profile(2.0, 2.0));
        m.table = table;
        let meshes = molding_meshes(&m, 0.0);
        // Ten elements of 4 x 2 x 2.
        assert!((volume(&meshes) - 10.0 * 4.0 * 4.0).abs() < 1e-3, "{}", volume(&meshes));
        // The first sits 3 inches in (half the 6 inch gap), the last ends 3
        // inches short of the end.
        let (lo, hi) = bounds(&meshes);
        assert!((lo[0] - 3.0).abs() < 1e-3 && (hi[0] - 97.0).abs() < 1e-3, "{lo:?} {hi:?}");
        // Without an element length the element is half the repeat distance.
        m.table.rows[0].element_length = 0.0;
        assert!((volume(&molding_meshes(&m, 0.0)) - 10.0 * 5.0 * 4.0).abs() < 1e-3);
    }

    #[test]
    fn a_corner_element_is_mitred_at_forty_five_degrees() {
        let mut table = MoldingTable::default();
        let i = table.add_new(box_profile(1.0, 1.0));
        table.rows[i].repeat_distance = 40.0;
        table.rows[i].element_length = 20.0;
        let mut m = line(vec![pt(0.0, 0.0), pt(20.0, 0.0), pt(20.0, 20.0)], box_profile(1.0, 1.0));
        m.table = table;
        let meshes = molding_meshes(&m, 0.0);
        // One element centred on the corner (the path is 40 long): 10 along
        // each leg, mitred; the volume is the two legs minus the shared
        // corner square.
        let want = 19.0;
        assert!((volume(&meshes) - want).abs() < 0.05, "{}", volume(&meshes));
    }

    #[test]
    fn a_built_in_profile_sweeps_a_closed_mesh() {
        let crown = builtin_profiles()
            .into_iter()
            .find(|p| p.kind == plan_core::moldings::MoldingType::Crown)
            .unwrap();
        let area = polygon_area(crown.section());
        let entry = MoldingEntry::new(crown.clone());
        let m = line(rect(120.0, 96.0), crown);
        let meshes = molding_meshes(&m, 0.0);
        let v = volume(&meshes);
        // Ring of the section's area around the rectangle (mitres included).
        assert!(v > area * 2.0 * (120.0 + 96.0) * 0.9 && v < area * 2.0 * (120.0 + 96.0), "{v}");
        assert!(entry.width > 0.0);
        let _ = MoldingProfile::Base;
    }

    #[test]
    fn a_vertical_edge_does_not_break_the_frame() {
        let mut m = line(vec![pt(0.0, 0.0), pt(0.0, 0.0)], box_profile(1.0, 1.0));
        m.polyline = vec![pt(0.0, 0.0), pt(0.0, 0.0), pt(50.0, 0.0)];
        m.heights = vec![0.0, 24.0, 24.0];
        let meshes = molding_meshes(&m, 0.0);
        assert!(!meshes.is_empty());
        assert!(volume(&meshes).is_finite());
    }

    #[test]
    fn twisted_joints_are_averaged_or_cut_square() {
        // Level then climbing at 45 degrees in a bent plan.
        let mut m = line(
            vec![pt(0.0, 0.0), pt(50.0, 0.0), pt(50.0, 50.0)],
            box_profile(1.0, 2.0),
        );
        m.heights = vec![0.0, 0.0, 30.0];
        let mitred = volume(&molding_meshes(&m, 0.0));
        m.mitre_twisted = false;
        let square = volume(&molding_meshes(&m, 0.0));
        assert!(mitred.is_finite() && square.is_finite());
        assert!(mitred > 0.0 && square > 0.0);
        m.mitre_twisted = true;
        m.auto_orient = false;
        assert!(volume(&molding_meshes(&m, 0.0)) > 0.0);
    }
}
