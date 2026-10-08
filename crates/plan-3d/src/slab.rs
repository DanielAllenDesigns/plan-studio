//! Floor and ceiling slabs triangulated from detected room polygons.

use crate::builder::MeshBuilder;
use crate::mesh::{Material, Mesh};
use crate::triangulate::ear_clip;
use plan_core::Point;

/// Slab thickness, inches.
pub const SLAB_THICKNESS: f64 = 1.0;
/// Finish thickness added above the floor elevation, inches.
pub const FLOOR_FINISH: f64 = 0.75;

const IN_PER_FT: f64 = 12.0;

type V3d = [f64; 3];

fn sub3(a: V3d, b: V3d) -> V3d {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross3(a: V3d, b: V3d) -> V3d {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit3(a: V3d) -> Option<V3d> {
    let len = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    (len > 1e-9).then(|| [a[0] / len, a[1] / len, a[2] / len])
}

fn f32v(a: V3d) -> [f32; 3] {
    a.map(|c| c as f32)
}

/// A slab of `thickness` inches below a planar polygon in scene space
/// (`x`, `y` up, `z`; inches), for roof planes and ceilings.
///
/// `polygon3d` is the upper surface, in either winding; the slab extends
/// against the plane normal that points up (or the Newell normal when the
/// plane is vertical). Returns an empty mesh for degenerate input.
pub fn slab_from_polygon(polygon3d: &[[f64; 3]], thickness: f64, material: Material) -> Mesh {
    let mut mesh = MeshBuilder::new(material);
    let n = polygon3d.len();
    // Newell normal: CCW orientation when viewed from its tip.
    let mut nn = [0.0; 3];
    for i in 0..n {
        let (a, b) = (polygon3d[i], polygon3d[(i + 1) % n]);
        nn[0] += (a[1] - b[1]) * (a[2] + b[2]);
        nn[1] += (a[2] - b[2]) * (a[0] + b[0]);
        nn[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let Some(nn) = (n >= 3).then(|| unit3(nn)).flatten() else {
        return mesh.finish(None);
    };
    let up = if nn[1] < -1e-9 { nn.map(|c| -c) } else { nn };
    let drop = thickness.max(0.0);
    let bottom = |p: V3d| {
        [
            p[0] - up[0] * drop,
            p[1] - up[1] * drop,
            p[2] - up[2] * drop,
        ]
    };

    // Triangulate in the plane projected along its dominant axis.
    let axis = (0..3)
        .max_by(|&a, &b| nn[a].abs().total_cmp(&nn[b].abs()))
        .unwrap_or(1);
    let (ia, ib) = match axis {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    };
    let flat: Vec<Point> = polygon3d.iter().map(|p| Point::new(p[ia], p[ib])).collect();
    let uv = |p: V3d| [(p[ia] / IN_PER_FT) as f32, (p[ib] / IN_PER_FT) as f32];
    for [a, b, c] in ear_clip(&flat) {
        let tri = [polygon3d[a], polygon3d[b], polygon3d[c]];
        let uvs = tri.map(uv);
        mesh.tri(tri.map(f32v), uvs, f32v(up));
        if drop > 0.0 {
            mesh.tri(tri.map(|p| f32v(bottom(p))), uvs, f32v(up.map(|c| -c)));
        }
    }
    if drop > 0.0 {
        for i in 0..n {
            let (a, b) = (polygon3d[i], polygon3d[(i + 1) % n]);
            let Some(out) = unit3(cross3(sub3(b, a), nn)) else {
                continue;
            };
            let len =
                ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
            let u = (len / IN_PER_FT) as f32;
            let quad = [a, b, bottom(b), bottom(a)].map(f32v);
            mesh.quad(
                quad,
                [[0.0, 0.0], [u, 0.0], [u, 1.0], [0.0, 1.0]],
                f32v(out),
            );
        }
    }
    mesh.finish(None)
}

/// Per-room vertical settings that reach the 3D platforms (R-23..R-30, R-33):
/// the room name entry whose anchor lies in the room supplies a floor height
/// offset, a ceiling height override, the Floor and Ceiling Structure, the
/// finish thickness and the Floor/Ceiling Under/Over This Room switches;
/// rooms without a named entry keep the floor's defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RoomLevels {
    /// Raise of the room's floor above the floor datum, inches.
    pub floor_offset: f64,
    /// Ceiling height measured from the room's own floor, inches.
    pub ceiling_height: f64,
    /// Floor finish above the platform, inches.
    pub floor_finish: f64,
    /// Thickness of the floor platform, inches.
    pub floor_thickness: f64,
    /// Thickness of the ceiling platform, inches.
    pub ceiling_thickness: f64,
    /// Build the floor platform under the room (R-30, Open Below).
    pub has_floor: bool,
    /// Build the ceiling platform over the room (R-30, Deck and Porch).
    pub has_ceiling: bool,
}

/// The levels of `room` on `floor`.
pub(crate) fn room_levels(floor: &plan_core::Floor, room: &plan_core::Room) -> RoomLevels {
    let named = room.name_entry(&floor.room_names);
    let misc = named.and_then(|n| n.misc.as_ref());
    let layered = |layers: Option<&Vec<plan_core::extras::StructureLayer>>| {
        layers
            .filter(|l| !l.is_empty())
            .map(|l| plan_core::extras::structure_thickness(l))
    };
    RoomLevels {
        floor_offset: named.map_or(0.0, |n| n.floor_height_offset),
        ceiling_height: named
            .and_then(|n| n.ceiling_height)
            .unwrap_or(floor.ceiling_height),
        floor_finish: misc.map_or(floor.settings.floor_finish_thickness, |m| {
            m.floor_finish_thickness
        }),
        floor_thickness: layered(misc.map(|m| &m.floor_structure)).unwrap_or(SLAB_THICKNESS),
        ceiling_thickness: layered(misc.map(|m| &m.ceiling_structure)).unwrap_or(SLAB_THICKNESS),
        has_floor: named.is_none_or(|n| n.has_floor),
        has_ceiling: named.is_none_or(|n| n.has_ceiling),
    }
}

/// Top of the ceiling platform of `room` on `floor`, scene elevation: where
/// a flat roof over the room sits (the Flat Roof directive).
pub fn room_ceiling_top(floor: &plan_core::Floor, room: &plan_core::Room) -> f64 {
    let l = room_levels(floor, room);
    floor.elevation + l.floor_offset + l.ceiling_height + l.ceiling_thickness
}

/// Stem walls under a room (R-26, R-40): concrete walls along the room's
/// exterior walls from the underside of the room's floor platform up to the
/// floor datum, where the walls above begin. A garage floor dropped below
/// the house floor gets them from its drop; a room with a Stem Wall height
/// gets them that deep below the datum. They stop at garage doors.
/// `thickness` is the foundation wall type's (the wall's own when unknown).
/// One mesh per run, tagged with its wall.
pub(crate) fn stem_walls(
    floor: &plan_core::Floor,
    room: &plan_core::Room,
    levels: &RoomLevels,
    thickness: Option<f64>,
) -> Vec<Mesh> {
    use crate::builder::MeshSet;
    use crate::frame::Frame;
    use plan_core::geometry::dist_to_segment;
    use plan_core::{OpeningStyle, WallKind};
    let explicit = room
        .name_entry(&floor.room_names)
        .and_then(|n| n.stem_wall_height)
        .filter(|h| *h > 0.5);
    let dropped = levels.floor_offset < -0.5;
    let datum = floor.elevation;
    let platform = datum + levels.floor_offset - levels.floor_thickness;
    let y0 = match (dropped, explicit) {
        (true, Some(h)) => platform.min(datum - h),
        (true, None) => platform,
        (false, Some(h)) => datum - h,
        (false, None) => return Vec::new(),
    };
    if datum - y0 < 0.5 || room.polygon.len() < 3 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let n = room.polygon.len();
    for i in 0..n {
        let (p, q) = (room.polygon[i], room.polygon[(i + 1) % n]);
        let len = p.dist(q);
        if len < 1.0 {
            continue;
        }
        let dir = q.sub(p).normalized();
        let mid = Point::lerp(p, q, 0.5);
        let Some(wall) = floor.walls.iter().find(|w| {
            w.kind == WallKind::Exterior
                && !w.flags.invisible
                && w.length() > 1e-6
                && dist_to_segment(mid, w.start, w.end) <= w.thickness * 0.5 + 1.0
                && w.direction().cross(dir).abs() < 0.02
        }) else {
            continue;
        };
        // Garage doors in the run leave the stem open; the rest is split.
        let along = |pt: Point| pt.sub(p).dot(dir);
        let mut gaps: Vec<(f64, f64)> = floor
            .openings_on(wall.id)
            .filter(|o| o.style == OpeningStyle::Garage)
            .map(|o| {
                let c = along(wall.point_at(o.center_offset));
                (c - o.width * 0.5, c + o.width * 0.5)
            })
            .collect();
        gaps.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut runs = Vec::new();
        let mut cursor = 0.0;
        for (g0, g1) in gaps {
            if g0 > cursor {
                runs.push((cursor, g0.min(len)));
            }
            cursor = cursor.max(g1);
        }
        if cursor < len {
            runs.push((cursor, len));
        }
        let t = thickness.unwrap_or(wall.thickness).max(1.0);
        let mut synthetic = wall.clone();
        synthetic.start = p;
        synthetic.end = q;
        synthetic.thickness = t;
        synthetic.curve = None;
        synthetic.bottom_offset = 0.0;
        let frame = Frame::new(&synthetic, y0);
        for (s0, s1) in runs.into_iter().filter(|r| r.1 - r.0 > 1.0) {
            let mut set = MeshSet::default();
            frame.cuboid(
                set.material(Material::Concrete),
                (s0, s1),
                (-t * 0.5, t * 0.5),
                (0.0, datum - y0),
            );
            out.extend(set.finish(Some(wall.id)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_square_slab_is_a_closed_box() {
        let sq = [
            [0.0, 100.0, 0.0],
            [120.0, 100.0, 0.0],
            [120.0, 100.0, -120.0],
            [0.0, 100.0, -120.0],
        ];
        for poly in [sq.to_vec(), sq.iter().rev().copied().collect::<Vec<_>>()] {
            let m = slab_from_polygon(&poly, 6.0, Material::Roof);
            assert_eq!(m.triangle_count(), 12);
            let (lo, hi) = m.bounds().unwrap();
            assert!((hi[1] - 100.0).abs() < 1e-4 && (lo[1] - 94.0).abs() < 1e-4);
            // Normals agree with winding and the box faces outward.
            let center = [60.0, 97.0, -60.0];
            for t in m.indices.chunks(3) {
                let p = |i: u32| m.vertices[i as usize].position.map(f64::from);
                let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                let g = cross3(sub3(b, a), sub3(c, a));
                let out = sub3(a, center);
                assert!(g[0] * out[0] + g[1] * out[1] + g[2] * out[2] > 0.0);
            }
        }
    }

    #[test]
    fn sloped_slab_hangs_below_the_plane_and_degenerate_is_empty() {
        let tilted = [
            [0.0, 0.0, 0.0],
            [120.0, 0.0, 0.0],
            [120.0, 60.0, -120.0],
            [0.0, 60.0, -120.0],
        ];
        let m = slab_from_polygon(&tilted, 4.0, Material::Roof);
        assert_eq!(m.triangle_count(), 12);
        let (lo, _) = m.bounds().unwrap();
        assert!(lo[1] < 0.0);
        assert_eq!(
            slab_from_polygon(&tilted[..2], 4.0, Material::Roof).triangle_count(),
            0
        );
    }
}
