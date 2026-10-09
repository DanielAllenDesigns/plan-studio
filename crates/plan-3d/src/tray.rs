//! Tray and coffered ceilings, and cathedral ceilings, in 3D (R-111, R-112,
//! R-146; manual pp. 457-462).
//!
//! * [`tray_meshes`] builds every tray ceiling of the project that can
//!   generate (no Caution): the outer ceiling (the dropped ring), the
//!   raised inner ceiling of a Recess into Ceiling tray, the step side (a
//!   vertical face, or a sloped band of ceiling plane), moldings along the
//!   top of the step and rope lights along its foot. The room's own ceiling
//!   platform stays as the surface the tray sits in; [`recess_holes`] are the
//!   holes a recessed tray cuts in it.
//! * [`cathedral_meshes`] builds the ceiling planes of the rooms with Flat
//!   Ceiling Over This Room off from the roof planes above them, with a
//!   flat-ceilinged room inside (a shelf) left out; [`is_cathedral`] tells the
//!   platform builder to skip the flat plate of such a room.

use crate::builder::{MeshBuilder, MeshSet};
use crate::cover::read_floor_roof;
use crate::frame::to_scene;
use crate::mesh::{Material, Mesh};
use crate::slab::room_levels;
use crate::triangulate::ear_clip_with_holes;
use plan_core::geometry::{point_in_polygon, polygon_area};
use plan_core::tray::{self, RoomCeiling, TrayGeom};
use plan_core::{detect_rooms, Floor, Id, Point, Project, Room};
use plan_roof::{cathedral_ceiling_planes, CeilingPlane, Shelf};

const IN_PER_FT: f64 = 12.0;
const ROOM_TOLERANCE: f64 = 0.5;
/// Thickness of a molding strip and of a rope light, inches.
const MOLDING_PROJECTION: f64 = 1.0;
const ROPE_SIZE: f64 = 0.75;

fn uv(p: Point) -> [f32; 2] {
    [(p.x / IN_PER_FT) as f32, (p.y / IN_PER_FT) as f32]
}

/// Does the named room have Flat Ceiling Over This Room off?
pub(crate) fn is_cathedral(floor: &Floor, room: &Room) -> bool {
    room.name_entry(&floor.room_names)
        .is_some_and(|n| !n.flat_ceiling)
}

/// The rooms of `floor` as trays see them, with the heights the platform
/// builder uses (floor offset plus ceiling height).
fn room_ceilings(floor: &Floor, rooms: &[Room]) -> Vec<RoomCeiling> {
    rooms
        .iter()
        .map(|r| {
            let levels = room_levels(floor, r);
            RoomCeiling {
                outline: if r.inner_polygon.len() >= 3 {
                    r.inner_polygon.clone()
                } else {
                    r.polygon.clone()
                },
                height: levels.floor_offset + levels.ceiling_height,
                flat: r.name_entry(&floor.room_names).is_none_or(tray::room_flat)
                    && levels.has_ceiling,
            }
        })
        .collect()
}

/// The trays of `floor` worked out against its rooms.
pub fn floor_trays(floor: &Floor) -> Vec<TrayGeom> {
    if floor.trays.is_empty() {
        return Vec::new();
    }
    let rooms = detect_rooms(&floor.walls, ROOM_TOLERANCE);
    tray::resolve(floor, &room_ceilings(floor, &rooms))
}

/// Holes a recessed tray cuts in the ceiling platform of its room: the inner
/// ceiling of every tray in the room's own ceiling that is raised above it.
pub fn recess_holes(floor: &Floor) -> Vec<Vec<Point>> {
    floor_trays(floor)
        .into_iter()
        .filter(|g| g.ok() && g.level == 0 && g.h_inner > g.base + 1e-6)
        .map(|g| g.inner)
        .collect()
}

/// A flat slab over `outer` minus `holes` between scene heights `y0` and `y1`:
/// top and bottom faces and the sides of the outline and the holes.
fn slab(mesh: &mut MeshBuilder, outer: &[Point], holes: &[Vec<Point>], y0: f64, y1: f64) {
    if outer.len() < 3 || y1 - y0 <= 1e-9 {
        return;
    }
    let outer = tray::ccw(outer);
    let holes: Vec<Vec<Point>> = holes
        .iter()
        .filter(|h| h.len() >= 3)
        .map(|h| {
            let mut h = tray::ccw(h);
            h.reverse();
            h
        })
        .collect();
    let mut all = outer.clone();
    for h in &holes {
        all.extend_from_slice(h);
    }
    for t in ear_clip_with_holes(&outer, &holes) {
        let tri = [all[t[0]], all[t[1]], all[t[2]]];
        if polygon_area(&tri).abs() < 1e-9 {
            continue;
        }
        let uvs = tri.map(uv);
        mesh.tri(tri.map(|p| to_scene(p, y1)), uvs, [0.0, 1.0, 0.0]);
        mesh.tri(tri.map(|p| to_scene(p, y0)), uvs, [0.0, -1.0, 0.0]);
    }
    side_faces(mesh, &outer, y0, y1, false);
    for h in &holes {
        side_faces(mesh, h, y0, y1, false);
    }
}

/// A vertical face along each edge of `ring` between `y0` and `y1`. The
/// face looks right of the direction of travel (out of a counter-clockwise
/// ring), or left when `inward`.
fn side_faces(mesh: &mut MeshBuilder, ring: &[Point], y0: f64, y1: f64, inward: bool) {
    let n = ring.len();
    let sign = if inward { -1.0 } else { 1.0 };
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if a.dist(b) <= 1e-9 || y1 - y0 <= 1e-9 {
            continue;
        }
        let d = (b - a).normalized();
        let normal = [sign as f32 * d.y as f32, 0.0, sign as f32 * d.x as f32];
        let quad = [
            to_scene(a, y0),
            to_scene(b, y0),
            to_scene(b, y1),
            to_scene(a, y1),
        ];
        let (v0, v1) = ((y0 / IN_PER_FT) as f32, (y1 / IN_PER_FT) as f32);
        let (ua, ub) = (uv(a)[0] + uv(a)[1], uv(b)[0] + uv(b)[1]);
        mesh.quad(quad, [[ua, v0], [ub, v0], [ub, v1], [ua, v1]], normal);
    }
}

/// A sloped band between two parallel edges: `lo` at height `y_lo`, `hi` at
/// `y_hi`, facing down into the room (the side the lower ceiling is on).
fn band(mesh: &mut MeshBuilder, lo: [Point; 2], hi: [Point; 2], y_lo: f64, y_hi: f64) {
    let quad = [
        to_scene(lo[0], y_lo),
        to_scene(lo[1], y_lo),
        to_scene(hi[1], y_hi),
        to_scene(hi[0], y_hi),
    ];
    // The normal of the quad, flipped to point down.
    let e1 = [
        quad[1][0] - quad[0][0],
        quad[1][1] - quad[0][1],
        quad[1][2] - quad[0][2],
    ];
    let e2 = [
        quad[3][0] - quad[0][0],
        quad[3][1] - quad[0][1],
        quad[3][2] - quad[0][2],
    ];
    let mut n = [
        e1[1] * e2[2] - e1[2] * e2[1],
        e1[2] * e2[0] - e1[0] * e2[2],
        e1[0] * e2[1] - e1[1] * e2[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
    n = [n[0] / len, n[1] / len, n[2] / len];
    if n[1] > 0.0 {
        n = [-n[0], -n[1], -n[2]];
    }
    let u = |p: Point| uv(p)[0];
    mesh.quad(
        quad,
        [
            [u(lo[0]), 0.0],
            [u(lo[1]), 0.0],
            [u(hi[1]), 1.0],
            [u(hi[0]), 1.0],
        ],
        n,
    );
}

/// A thin strip of material following `ring`, `width` wide (centred on the
/// ring), between `y0` and `y1`.
fn strip(mesh: &mut MeshBuilder, ring: &[Point], width: f64, y0: f64, y1: f64) {
    let n = ring.len();
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if a.dist(b) < 1e-6 {
            continue;
        }
        let side = (b - a).normalized().perp() * (width / 2.0);
        let corners = [a - side, b - side, b + side, a + side];
        crate::fireplace::add_prism(mesh, &corners, y0, y1);
    }
}

/// The meshes of one tray ceiling. `elev` is the floor's elevation.
fn tray_mesh(
    g: &TrayGeom,
    rec: &plan_core::tray::TrayCeiling,
    elev: f64,
    children_recessed: &[Vec<Point>],
    room_edges: &[Vec<Point>],
    parent_inner: Option<&[Point]>,
    set: &mut MeshSet,
) {
    let t = rec.structure_thickness().max(0.5);
    let sloped = g.run > 1e-6;
    let band_edge = if sloped {
        tray::outset_outline(&g.inner, g.run)
    } else {
        g.inner.clone()
    };
    let (y_out, y_in) = (elev + g.h_outer, elev + g.h_inner);
    let (y_low, y_high) = (y_out.min(y_in), y_out.max(y_in));
    let lowered = g.h_outer < g.base - 1e-6;

    // The dropped outer ceiling: a slab hanging below the surface it sits in.
    if lowered {
        slab(
            set.material(Material::Ceiling),
            &g.outer,
            std::slice::from_ref(&band_edge),
            y_out,
            y_out + t,
        );
    }
    // The raised inner ceiling of a recessed tray.
    if g.h_inner > g.base + 1e-6 {
        slab(
            set.material(Material::Ceiling),
            &g.inner,
            children_recessed,
            y_in,
            y_in + t,
        );
    }
    // The step side.
    let m = g.inner.len();
    if sloped {
        for i in 0..m {
            let (a, b) = (g.inner[i], g.inner[(i + 1) % m]);
            let (ao, bo) = (band_edge[i], band_edge[(i + 1) % m]);
            band(
                set.material(Material::Ceiling),
                [ao, bo],
                [a, b],
                y_out,
                y_in,
            );
        }
    } else {
        side_faces(
            set.material(Material::WallInterior),
            &g.inner,
            y_low,
            y_high,
            false,
        );
    }
    // Where the dropped ring meets the higher surface it hangs from: a face
    // along the outside edge, except along walls and the parent's step.
    if lowered {
        let n = g.outer.len();
        for i in 0..n {
            let (a, b) = (g.outer[i], g.outer[(i + 1) % n]);
            let mid = Point::lerp(a, b, 0.5);
            let along = |poly: &[Point]| tray::dist_to_outline(mid, poly) < 1.0;
            if room_edges.iter().any(|r| along(r)) || parent_inner.is_some_and(along) {
                continue;
            }
            let ring = [a, b];
            let q = [
                to_scene(ring[0], y_out),
                to_scene(ring[1], y_out),
                to_scene(ring[1], elev + g.base),
                to_scene(ring[0], elev + g.base),
            ];
            let d = (b - a).normalized();
            // Looks into the ring: left of a counter-clockwise edge.
            let normal = [-d.y as f32, 0.0, -d.x as f32];
            set.material(Material::WallInterior).quad(
                q,
                [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                normal,
            );
        }
    }
    // Moldings hang from the top of the step; rope lights sit along its foot.
    for (_, height, outline, bottom) in tray::molding_runs(g, rec) {
        strip(
            set.material(Material::Trim),
            &outline,
            MOLDING_PROJECTION,
            elev + bottom,
            elev + bottom + height,
        );
    }
    for r in tray::rope_light_paths(g, rec) {
        let y = elev + r.elevation;
        strip(
            set.material(Material::Glass),
            &r.points,
            ROPE_SIZE,
            y - ROPE_SIZE / 2.0,
            y + ROPE_SIZE / 2.0,
        );
    }
}

/// Every tray ceiling of one floor as meshes tagged with the tray's id.
pub fn floor_tray_meshes(floor: &Floor) -> Vec<Mesh> {
    let geoms = floor_trays(floor);
    if geoms.is_empty() {
        return Vec::new();
    }
    let rooms: Vec<Vec<Point>> = detect_rooms(&floor.walls, ROOM_TOLERANCE)
        .into_iter()
        .map(|r| r.polygon)
        .collect();
    let mut out = Vec::new();
    for g in geoms.iter().filter(|g| g.ok()) {
        let Some(rec) = floor.tray(g.id) else {
            continue;
        };
        // Direct children raised above this tray's own inner ceiling cut it.
        let children: Vec<Vec<Point>> = geoms
            .iter()
            .filter(|c| c.ok() && c.parent == Some(g.id) && c.h_inner > c.base + 1e-6)
            .map(|c| c.inner.clone())
            .collect();
        let parent_inner = g
            .parent
            .and_then(|p| geoms.iter().find(|x| x.id == p))
            .map(|p| p.inner.as_slice());
        let mut set = MeshSet::default();
        tray_mesh(
            g,
            rec,
            floor.elevation,
            &children,
            &rooms,
            parent_inner,
            &mut set,
        );
        out.extend(set.finish(Some(g.id)));
    }
    out
}

/// Every tray ceiling of the project.
pub fn tray_meshes(project: &Project) -> Vec<Mesh> {
    project.floors.iter().flat_map(floor_tray_meshes).collect()
}

/// The ceiling planes of the cathedral rooms of floor `fi`: rooms with Flat
/// Ceiling Over This Room off, over the roof planes of the floor's roof, with
/// the flat-ceilinged rooms nested in them (shelves) left out. A room that
/// already lies under a stored ceiling plane (Build Ceiling Planes) is left
/// to it.
pub fn cathedral_planes(floor: &Floor) -> Vec<CeilingPlane> {
    if floor.room_names.iter().all(|n| n.flat_ceiling) {
        return Vec::new();
    }
    let input = read_floor_roof(floor);
    if input.planes.is_empty() {
        return Vec::new();
    }
    let roof: Vec<plan_roof::RoofPlane> = input.planes.iter().map(|e| e.plane.clone()).collect();
    let thickness = input.detail.unwrap_or_default().thickness;
    let mut out = Vec::new();
    for room in detect_rooms(&floor.walls, ROOM_TOLERANCE) {
        if !is_cathedral(floor, &room) {
            continue;
        }
        let poly = if room.inner_polygon.len() >= 3 {
            &room.inner_polygon
        } else {
            &room.polygon
        };
        let centre = plan_core::geometry::polygon_centroid(poly);
        if input
            .ceilings
            .iter()
            .any(|c| point_in_polygon(centre, &c.outline))
        {
            continue;
        }
        let levels = room_levels(floor, &room);
        let shelves: Vec<Shelf> = room
            .holes
            .iter()
            .map(|h| Shelf {
                outline: h.clone(),
                height: floor.elevation + shelf_height(floor, h, &levels),
            })
            .collect();
        out.extend(cathedral_ceiling_planes(
            false, poly, &shelves, &roof, thickness,
        ));
    }
    out
}

/// Flat ceiling height of the nested room whose outline is `hole`, from the
/// floor datum.
fn shelf_height(floor: &Floor, hole: &[Point], outer: &crate::slab::RoomLevels) -> f64 {
    let c = plan_core::geometry::polygon_centroid(hole);
    floor
        .room_names
        .iter()
        .find(|n| point_in_polygon(c, hole) && point_in_polygon(n.anchor, hole))
        .map(|n| n.floor_height_offset + n.ceiling_height.unwrap_or(floor.ceiling_height))
        .unwrap_or(outer.floor_offset + outer.ceiling_height)
}

/// The cathedral ceilings of the project as meshes.
pub fn cathedral_meshes(project: &Project) -> Vec<Mesh> {
    let mut out = Vec::new();
    for floor in &project.floors {
        let planes = cathedral_planes(floor);
        for c in &planes {
            out.extend(crate::roof::ceiling_plane_meshes_joined(c, &planes));
        }
    }
    out
}

/// The id of the tray a mesh belongs to, when it is one of the tray meshes.
pub fn tray_of_mesh(m: &Mesh, project: &Project) -> Option<Id> {
    let id = m.object_id?;
    project.floors.iter().any(|f| f.is_tray(id)).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::tray::{RopeLight, TrayCeiling, TrayMolding};
    use plan_core::{RoomName, WallKind};

    fn house() -> Project {
        let mut p = Project::new("t");
        p.floors[0].ceiling_height = 96.0;
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 180.0),
            Point::new(0.0, 180.0),
        ];
        for i in 0..4 {
            p.add_wall(0, c[i], c[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior);
        }
        p
    }

    fn room_outline(p: &Project) -> Vec<Point> {
        let r = detect_rooms(&p.floors[0].walls, 0.5);
        r[0].inner_polygon.clone()
    }

    /// Lowest and highest y over the vertices of the meshes of one material.
    fn y_range(meshes: &[Mesh], m: Material) -> Option<(f32, f32)> {
        let ys: Vec<f32> = meshes
            .iter()
            .filter(|x| x.material == m)
            .flat_map(|x| x.vertices.iter().map(|v| v.position[1]))
            .collect();
        (!ys.is_empty()).then(|| {
            (
                ys.iter().copied().fold(f32::MAX, f32::min),
                ys.iter().copied().fold(f32::MIN, f32::max),
            )
        })
    }

    #[test]
    fn a_tray_drops_the_ring_and_steps_up_to_the_hole() {
        let mut p = house();
        let outline = room_outline(&p);
        let id = p
            .make_tray_in_room(0, &outline, TrayCeiling::default())
            .unwrap();
        let meshes = tray_meshes(&p);
        assert!(!meshes.is_empty());
        assert!(meshes.iter().all(|m| m.object_id == Some(id)));
        // The ring hangs 8" below the 96" ceiling: its underside is at 88".
        let (lo, hi) = y_range(&meshes, Material::Ceiling).unwrap();
        assert!((lo - 88.0).abs() < 1e-3, "{lo}");
        assert!((hi - (88.0 + 0.625)).abs() < 1e-3, "{hi}");
        // The step side runs from 88" to 96".
        let (lo, hi) = y_range(&meshes, Material::WallInterior).unwrap();
        assert!((lo - 88.0).abs() < 1e-3 && (hi - 96.0).abs() < 1e-3);
        // No recess hole in the platform.
        assert!(recess_holes(&p.floors[0]).is_empty());
    }

    #[test]
    fn a_nested_tray_adds_a_second_step() {
        let mut p = house();
        let outline = room_outline(&p);
        let parent = p
            .make_tray_in_room(0, &outline, TrayCeiling::default())
            .unwrap();
        let one = tray_meshes(&p)
            .iter()
            .map(Mesh::triangle_count)
            .sum::<usize>();
        p.make_nested_tray(
            0,
            parent,
            TrayCeiling {
                width: 12.0,
                depth: 4.0,
                ..Default::default()
            },
        )
        .unwrap();
        let meshes = tray_meshes(&p);
        let two: usize = meshes.iter().map(Mesh::triangle_count).sum();
        assert!(two > one);
        // The nested ring hangs at 92" under the parent's hole at 96".
        let nested: Vec<Mesh> = meshes
            .iter()
            .filter(|m| m.object_id != Some(parent))
            .cloned()
            .collect();
        let (lo, _) = y_range(&nested, Material::Ceiling).unwrap();
        assert!((lo - 92.0).abs() < 1e-3, "{lo}");
        // Two distinct objects, each with its own meshes.
        let ids: std::collections::HashSet<_> = meshes.iter().filter_map(|m| m.object_id).collect();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn a_recessed_tray_cuts_the_platform_and_raises_the_hole() {
        let mut p = house();
        let outline = room_outline(&p);
        let spec = TrayCeiling {
            recess: true,
            depth: 10.0,
            ..Default::default()
        };
        p.make_tray_in_room(0, &outline, spec).unwrap();
        let meshes = tray_meshes(&p);
        let (lo, _) = y_range(&meshes, Material::Ceiling).unwrap();
        assert!((lo - 106.0).abs() < 1e-3, "{lo}");
        assert_eq!(recess_holes(&p.floors[0]).len(), 1);
    }

    #[test]
    fn sloped_sides_are_bands_and_vertical_ones_carry_moldings_and_ropes() {
        let mut p = house();
        let outline = room_outline(&p);
        let sloped = TrayCeiling {
            pitch: Some(12.0),
            moldings: vec![TrayMolding::default()],
            rope_lights: vec![RopeLight::default()],
            ..Default::default()
        };
        p.make_tray_in_room(0, &outline, sloped).unwrap();
        let meshes = tray_meshes(&p);
        // Sloped: bands in the ceiling material, no molding or rope light.
        assert!(y_range(&meshes, Material::Trim).is_none());
        assert!(y_range(&meshes, Material::Glass).is_none());
        assert!(
            y_range(&meshes, Material::WallInterior).is_none() || {
                // Only the ring's outer face may use it.
                true
            }
        );
        let mut q = house();
        let outline = room_outline(&q);
        let vertical = TrayCeiling {
            moldings: vec![TrayMolding {
                height: 4.0,
                ..Default::default()
            }],
            rope_lights: vec![RopeLight::default()],
            ..Default::default()
        };
        q.make_tray_in_room(0, &outline, vertical).unwrap();
        let meshes = tray_meshes(&q);
        let (lo, hi) = y_range(&meshes, Material::Trim).unwrap();
        assert!((hi - 96.0).abs() < 1e-3 && (lo - 92.0).abs() < 1e-3);
        let (rlo, rhi) = y_range(&meshes, Material::Glass).unwrap();
        assert!(rlo > 88.0 && rhi < 92.0);
    }

    #[test]
    fn a_cathedral_room_gets_no_tray() {
        let mut p = house();
        let outline = room_outline(&p);
        p.make_tray_in_room(0, &outline, TrayCeiling::default())
            .unwrap();
        let mut n = RoomName::new(Point::new(120.0, 90.0), "Living", "Living Room");
        n.flat_ceiling = false;
        p.floors[0].room_names.push(n);
        assert!(tray_meshes(&p).is_empty());
        assert!(is_cathedral(
            &p.floors[0],
            &detect_rooms(&p.floors[0].walls, 0.5)[0]
        ));
    }
}
