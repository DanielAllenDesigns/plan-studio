//! Meshes for slabs, footings, square pads, round piers and the holes in the
//! floor and ceiling platforms (`plan_core::foundation`).
//!
//! * [`foundation_meshes`] builds every slab (a polygon-with-holes extrusion),
//!   its footings, the pads and the piers of a project, all in
//!   [`Material::Concrete`] unless the object's material names another one.
//! * [`cut_platform`] triangulates a platform polygon with holes cut out, and
//!   [`build_platform`] builds the floor and ceiling platforms over the rooms
//!   with the platform holes cut: `build_scene` calls it with
//!   `FoundationLayer::platform_hole_outlines(kind)` for every floor.

use crate::builder::MeshBuilder;
use crate::frame::to_scene;
use crate::mesh::{Material, Mesh};
use crate::triangulate::ear_clip_with_holes;
use plan_core::foundation::{
    hole_inside, offset_ring, BoxSolid, FoundationLayer, Pad, Pier, PlatformKind, Slab,
};
use plan_core::geometry::{polygon_area, Point};
use plan_core::{Floor, Id, Project, Room};
use std::f64::consts::TAU;

/// A round pier is a prism with this many sides.
pub const PIER_SIDES: usize = 16;

const IN_PER_FT: f64 = 12.0;

fn uv(p: Point) -> [f32; 2] {
    [(p.x / IN_PER_FT) as f32, (p.y / IN_PER_FT) as f32]
}

fn oriented(pts: &[Point], ccw: bool) -> Vec<Point> {
    let mut v = pts.to_vec();
    if (polygon_area(&v) > 0.0) != ccw {
        v.reverse();
    }
    v
}

/// Holes that can be cut from `outer`: inside it and clear of the holes kept
/// before them. Anything else would not triangulate cleanly and is skipped.
fn usable_holes(outer: &[Point], holes: &[Vec<Point>]) -> Vec<Vec<Point>> {
    let mut kept: Vec<Vec<Point>> = Vec::new();
    for h in holes {
        if !hole_inside(h, outer) {
            continue;
        }
        let clear = kept.iter().all(|k| {
            let (lo, hi) = plan_core::foundation::bounds(k);
            let (hl, hh) = plan_core::foundation::bounds(h);
            hh.x < lo.x || hl.x > hi.x || hh.y < lo.y || hl.y > hi.y
        });
        if clear {
            kept.push(h.clone());
        }
    }
    kept
}

/// The triangles (counter-clockwise in plan) that cover `poly` minus `holes`.
///
/// Holes that are not entirely inside `poly`, or that overlap an earlier hole,
/// are ignored. This is what the floor-platform mesher calls to cut a platform
/// hole.
pub fn cut_platform(poly: &[Point], holes: &[Vec<Point>]) -> Vec<[Point; 3]> {
    let holes = usable_holes(poly, holes);
    triangulate(poly, &holes)
}

fn triangulate(outer: &[Point], holes: &[Vec<Point>]) -> Vec<[Point; 3]> {
    let mut all: Vec<Point> = outer.to_vec();
    for h in holes {
        all.extend_from_slice(h);
    }
    ear_clip_with_holes(outer, holes)
        .into_iter()
        .map(|[a, b, c]| [all[a], all[b], all[c]])
        .filter(|t| polygon_area(t).abs() > 1e-9)
        .collect()
}

/// Adds the prism between `y_bottom` and `y_top` (scene inches) over `outer`
/// with `holes` cut out: top and bottom faces, outer walls and hole walls.
fn add_prism(
    mesh: &mut MeshBuilder,
    outer: &[Point],
    holes: &[Vec<Point>],
    y_bottom: f64,
    y_top: f64,
) {
    let outer = oriented(outer, true);
    let holes: Vec<Vec<Point>> = usable_holes(&outer, holes)
        .iter()
        .map(|h| oriented(h, false))
        .collect();
    for tri in triangulate(&outer, &holes) {
        let uvs = tri.map(uv);
        mesh.tri(tri.map(|p| to_scene(p, y_top)), uvs, [0.0, 1.0, 0.0]);
        mesh.tri(tri.map(|p| to_scene(p, y_bottom)), uvs, [0.0, -1.0, 0.0]);
    }
    // Counter-clockwise outer ring and clockwise hole rings both keep the
    // material on the left, so the side normal is always the right-hand one.
    add_sides(mesh, &outer, y_bottom, y_top);
    for h in &holes {
        add_sides(mesh, h, y_bottom, y_top);
    }
}

fn add_sides(mesh: &mut MeshBuilder, ring: &[Point], y_bottom: f64, y_top: f64) {
    let (v0, v1) = ((y_bottom / IN_PER_FT) as f32, (y_top / IN_PER_FT) as f32);
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        let len = a.dist(b);
        if len <= 1e-9 {
            continue;
        }
        let d = (b - a).normalized();
        let out = [d.y as f32, 0.0, d.x as f32];
        let quad = [
            to_scene(a, y_bottom),
            to_scene(b, y_bottom),
            to_scene(b, y_top),
            to_scene(a, y_top),
        ];
        let u = (len / IN_PER_FT) as f32;
        mesh.quad(quad, [[0.0, v0], [u, v0], [u, v1], [0.0, v1]], out);
    }
}

/// An axis-aligned box in scene space.
fn add_box(mesh: &mut MeshBuilder, b: &BoxSolid, floor_elev: f64) {
    add_prism(
        mesh,
        &b.corners(),
        &[],
        floor_elev + b.bottom,
        floor_elev + b.top,
    );
}

/// A footing of `width` along `ring` on its material side, between `y_bottom`
/// and `y_top`. `ring` must keep the material on its left (counter-clockwise
/// outer ring, clockwise hole ring).
fn add_footing_ring(mesh: &mut MeshBuilder, ring: &[Point], width: f64, y0: f64, y1: f64) {
    let moved = offset_ring(ring, width);
    let (a_ring, a_moved) = (polygon_area(ring).abs(), polygon_area(&moved).abs());
    let same_turn = polygon_area(ring).signum() == polygon_area(&moved).signum();
    if !same_turn || a_moved < 1e-6 {
        // Wider than the shape: the footing fills it.
        add_prism(mesh, ring, &[], y0, y1);
        return;
    }
    if a_moved < a_ring {
        add_prism(mesh, ring, std::slice::from_ref(&moved), y0, y1);
    } else {
        add_prism(mesh, &moved, std::slice::from_ref(&ring.to_vec()), y0, y1);
    }
}

fn material_of(name: &str) -> Material {
    Material::from_layer_name(name).unwrap_or(Material::Concrete)
}

/// The mesh of one slab on a floor at `floor_elev`: the body with its holes
/// (`holes` are the outlines to cut: its own and the layer's slab holes that
/// fall inside it), the footing under the outer edge and a footing around
/// every hole in `footed_holes`.
pub fn slab_mesh(
    slab: &Slab,
    holes: &[Vec<Point>],
    footed_holes: &[Vec<Point>],
    floor_elev: f64,
) -> Option<Mesh> {
    if slab.outline.len() < 3 || slab.thickness <= 0.0 {
        return None;
    }
    let mut mesh = MeshBuilder::new(material_of(&slab.material));
    let y_top = floor_elev + slab.top_elevation;
    let y_bottom = y_top - slab.thickness;
    add_prism(&mut mesh, &slab.outline, holes, y_bottom, y_top);

    let size = slab.footing.unwrap_or_default();
    let (f_bottom, f_top) = (y_bottom - size.depth, y_bottom);
    if slab.footing.is_some() {
        let outer = oriented(&slab.outline, true);
        // Footing Offset: the footing reaches out past the slab's edges.
        let ring = if slab.footing_offset > 0.0 {
            offset_ring(&outer, -slab.footing_offset)
        } else {
            outer
        };
        add_footing_ring(&mut mesh, &ring, size.width, f_bottom, f_top);
    }
    let usable = usable_holes(&slab.outline, holes);
    for h in footed_holes {
        if usable.iter().any(|u| same_ring(u, h)) {
            add_footing_ring(&mut mesh, &oriented(h, false), size.width, f_bottom, f_top);
        }
    }
    (!mesh.is_empty()).then(|| mesh.finish(Some(slab.id)))
}

fn same_ring(a: &[Point], b: &[Point]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(p, q)| p.dist(*q) < 0.01)
}

/// The mesh of a square pad.
pub fn pad_mesh(pad: &Pad, floor_elev: f64) -> Option<Mesh> {
    if pad.size <= 0.0 || pad.thickness <= 0.0 {
        return None;
    }
    let mut mesh = MeshBuilder::new(material_of(&pad.material));
    add_box(&mut mesh, &pad.solid(), floor_elev);
    Some(mesh.finish(Some(pad.id)))
}

/// A vertical [`PIER_SIDES`]-sided prism between two scene elevations.
fn add_cylinder(mesh: &mut MeshBuilder, center: Point, radius: f64, y0: f64, y1: f64) {
    let ring: Vec<Point> = (0..PIER_SIDES)
        .map(|i| {
            let a = TAU * i as f64 / PIER_SIDES as f64;
            Point::new(center.x + radius * a.cos(), center.y + radius * a.sin())
        })
        .collect();
    // Caps as a fan around the center.
    for i in 0..PIER_SIDES {
        let tri = [center, ring[i], ring[(i + 1) % PIER_SIDES]];
        let uvs = tri.map(uv);
        mesh.tri(tri.map(|p| to_scene(p, y1)), uvs, [0.0, 1.0, 0.0]);
        mesh.tri(tri.map(|p| to_scene(p, y0)), uvs, [0.0, -1.0, 0.0]);
    }
    add_sides(mesh, &ring, y0, y1);
}

/// The mesh of a round pier: a 16-sided shaft and, when it has one, a square
/// footing box under it.
pub fn pier_mesh(pier: &Pier, floor_elev: f64) -> Option<Mesh> {
    if pier.diameter <= 0.0 || pier.height <= 0.0 {
        return None;
    }
    let mut mesh = MeshBuilder::new(material_of(&pier.material));
    add_cylinder(
        &mut mesh,
        pier.center,
        pier.diameter * 0.5,
        floor_elev + pier.bottom_elevation(),
        floor_elev + pier.elevation,
    );
    if let Some(b) = pier.footing_solid() {
        add_box(&mut mesh, &b, floor_elev);
    }
    Some(mesh.finish(Some(pier.id)))
}

/// Every foundation mesh on one floor.
pub fn floor_foundation_meshes(floor: &Floor) -> Vec<Mesh> {
    let layer = FoundationLayer::load(floor);
    if layer.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for slab in &layer.slabs {
        let holes = layer.all_hole_outlines(slab);
        let footed: Vec<Vec<Point>> = layer
            .holes_in(slab)
            .into_iter()
            .filter(|h| h.with_footing)
            .map(|h| h.outline.clone())
            .collect();
        out.extend(slab_mesh(slab, &holes, &footed, floor.elevation));
    }
    out.extend(
        layer
            .pads
            .iter()
            .filter_map(|p| pad_mesh(p, floor.elevation)),
    );
    out.extend(
        layer
            .piers
            .iter()
            .filter_map(|p| pier_mesh(p, floor.elevation)),
    );
    out
}

/// Every slab, footing, pad and pier of the project, ready to extend a scene:
/// `scene.meshes.extend(foundation_meshes(&project))`.
pub fn foundation_meshes(project: &Project) -> Vec<Mesh> {
    project
        .floors
        .iter()
        .flat_map(floor_foundation_meshes)
        .collect()
}

/// A floor or ceiling platform over `rooms` between `y_bottom` and `y_top`
/// (scene inches) with `holes` cut out of the rooms they lie in: a drop-in
/// for the room slab builder. `object_id` tags the mesh.
pub fn build_platform(
    material: Material,
    rooms: &[Room],
    holes: &[Vec<Point>],
    y_bottom: f64,
    y_top: f64,
    object_id: Option<Id>,
) -> Option<Mesh> {
    let mut mesh = MeshBuilder::new(material);
    for room in rooms {
        add_prism(&mut mesh, &room.polygon, holes, y_bottom, y_top);
    }
    (!mesh.is_empty()).then(|| mesh.finish(object_id))
}

/// The hole outlines of one platform of `floor`.
pub fn platform_holes(floor: &Floor, kind: PlatformKind) -> Vec<Vec<Point>> {
    FoundationLayer::load(floor).platform_hole_outlines(kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::foundation::{rect_outline, Footing, SlabHole};

    fn square(x: f64, y: f64, s: f64) -> Vec<Point> {
        rect_outline(Point::new(x, y), Point::new(x + s, y + s))
    }

    /// Signed volume of a closed mesh by the divergence theorem.
    fn volume(m: &Mesh) -> f64 {
        let p = |i: u32| m.vertices[i as usize].position.map(f64::from);
        m.indices
            .chunks(3)
            .map(|t| {
                let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                let cr = [
                    b[1] * c[2] - b[2] * c[1],
                    b[2] * c[0] - b[0] * c[2],
                    b[0] * c[1] - b[1] * c[0],
                ];
                (a[0] * cr[0] + a[1] * cr[1] + a[2] * cr[2]) / 6.0
            })
            .sum::<f64>()
            .abs()
    }

    fn tri_area_sum(tris: &[[Point; 3]]) -> f64 {
        tris.iter().map(|t| polygon_area(t).abs()).sum()
    }

    #[test]
    fn cut_platform_removes_the_hole_area() {
        let poly = square(0.0, 0.0, 200.0);
        let hole = square(50.0, 60.0, 40.0);
        let tris = cut_platform(&poly, std::slice::from_ref(&hole));
        assert!((tri_area_sum(&tris) - (40_000.0 - 1_600.0)).abs() < 1e-6);
        // No triangle centroid falls inside the hole.
        for t in &tris {
            let c = plan_core::geometry::polygon_centroid(t);
            assert!(!plan_core::geometry::point_in_polygon(c, &hole));
        }
        // A hole that leaves the polygon is ignored; none changes nothing.
        let out = square(180.0, 0.0, 40.0);
        assert!((tri_area_sum(&cut_platform(&poly, &[out])) - 40_000.0).abs() < 1e-6);
        assert!((tri_area_sum(&cut_platform(&poly, &[])) - 40_000.0).abs() < 1e-6);
        // Two holes, either winding.
        let mut h2 = square(120.0, 120.0, 30.0);
        h2.reverse();
        let tris = cut_platform(&poly, &[hole, h2]);
        assert!((tri_area_sum(&tris) - (40_000.0 - 1_600.0 - 900.0)).abs() < 1e-6);
    }

    #[test]
    fn a_slab_with_a_hole_is_a_closed_solid_of_the_right_volume() {
        let mut slab = Slab::new(7, square(0.0, 0.0, 240.0));
        slab.top_elevation = 2.0;
        let hole = square(100.0, 100.0, 40.0);
        let m = slab_mesh(&slab, std::slice::from_ref(&hole), &[], 0.0).unwrap();
        assert_eq!(m.material, Material::Concrete);
        assert_eq!(m.object_id, Some(7));
        let expect = (240.0 * 240.0 - 40.0 * 40.0) * 4.0;
        assert!((volume(&m) - expect).abs() < 1e-3, "{}", volume(&m));
        let (lo, hi) = m.bounds().unwrap();
        assert!((hi[1] - 2.0).abs() < 1e-5 && (lo[1] + 2.0).abs() < 1e-5);
        // The hole walls exist: more triangles than the plain slab.
        let plain = slab_mesh(&slab, &[], &[], 0.0).unwrap();
        assert_eq!(plain.triangle_count(), 12);
        assert!(m.triangle_count() > plain.triangle_count());
        assert!((volume(&plain) - 240.0 * 240.0 * 4.0).abs() < 1e-3);
    }

    #[test]
    fn footings_hang_below_the_slab_edge() {
        let mut slab = Slab::new(1, square(0.0, 0.0, 240.0));
        slab.footing = Some(Footing {
            width: 16.0,
            depth: 8.0,
        });
        let m = slab_mesh(&slab, &[], &[], 0.0).unwrap();
        let body = 240.0 * 240.0 * 4.0;
        let ring = (240.0 * 240.0 - 208.0 * 208.0) * 8.0;
        assert!((volume(&m) - (body + ring)).abs() < 1e-2, "{}", volume(&m));
        let (lo, _) = m.bounds().unwrap();
        assert!((lo[1] + 12.0).abs() < 1e-5, "4\" slab + 8\" footing");
    }

    #[test]
    fn a_hole_with_footing_gets_a_ring_around_it() {
        let slab = Slab::new(1, square(0.0, 0.0, 240.0));
        let hole = square(100.0, 100.0, 40.0);
        let h = SlabHole::new(2, hole.clone(), true);
        let plain = slab_mesh(&slab, std::slice::from_ref(&hole), &[], 0.0).unwrap();
        let footed = slab_mesh(
            &slab,
            std::slice::from_ref(&hole),
            std::slice::from_ref(&h.outline),
            0.0,
        )
        .unwrap();
        // Default footing 16" x 8" around a 40" hole: (72^2 - 40^2) * 8.
        let ring = (72.0_f64.powi(2) - 40.0_f64.powi(2)) * 8.0;
        assert!((volume(&footed) - volume(&plain) - ring).abs() < 1e-2);
    }

    #[test]
    fn a_pad_is_a_box_and_a_pier_has_sixteen_sides() {
        let pad = Pad::new(3, Point::new(50.0, 50.0));
        let m = pad_mesh(&pad, 0.0).unwrap();
        assert_eq!(m.triangle_count(), 12);
        assert!((volume(&m) - 24.0 * 24.0 * 12.0).abs() < 1e-3);
        let (lo, hi) = m.bounds().unwrap();
        assert_eq!((lo[1], hi[1]), (-12.0, 0.0));

        let pier = Pier::new(4, Point::ZERO);
        let m = pier_mesh(&pier, 0.0).unwrap();
        // 16 side quads + 16 top and 16 bottom fan triangles.
        assert_eq!(m.triangle_count(), PIER_SIDES * 2 + PIER_SIDES * 2);
        let side_normals: std::collections::HashSet<[i32; 3]> = m
            .vertices
            .iter()
            .filter(|v| v.normal[1].abs() < 1e-6)
            .map(|v| v.normal.map(|c| (c * 1000.0).round() as i32))
            .collect();
        assert_eq!(side_normals.len(), 16, "sixteen distinct side faces");
        let area = 0.5 * 16.0 * 6.0_f64.powi(2) * (TAU / 16.0).sin();
        assert!((volume(&m) - area * 36.0).abs() < 1e-2);
        let (lo, hi) = m.bounds().unwrap();
        assert_eq!((lo[1], hi[1]), (-36.0, 0.0));

        let mut footed = pier.clone();
        footed.footing = Some(Footing::default());
        let m2 = pier_mesh(&footed, 0.0).unwrap();
        assert_eq!(m2.triangle_count(), m.triangle_count() + 12);
        assert_eq!(m2.bounds().unwrap().0[1], -44.0);
    }

    #[test]
    fn floor_meshes_follow_the_stored_layer_and_elevation() {
        let mut project = Project::new("t");
        project.floors[0].elevation = -30.0;
        assert!(foundation_meshes(&project).is_empty());
        let layer = FoundationLayer {
            slabs: vec![Slab::new(1, square(0.0, 0.0, 120.0))],
            holes: vec![SlabHole::new(2, square(20.0, 20.0, 20.0), false)],
            pads: vec![Pad::new(3, Point::new(200.0, 0.0))],
            piers: vec![Pier::new(4, Point::new(300.0, 0.0))],
            platform_holes: Vec::new(),
        };
        layer.store(&mut project.floors[0]);
        let meshes = foundation_meshes(&project);
        assert_eq!(meshes.len(), 3);
        let slab = meshes.iter().find(|m| m.object_id == Some(1)).unwrap();
        let (lo, hi) = slab.bounds().unwrap();
        assert_eq!((lo[1], hi[1]), (-34.0, -30.0));
        // The stored slab hole is cut.
        let net = (120.0 * 120.0 - 400.0) * 4.0;
        assert!((volume(slab) - net).abs() < 1e-3);
    }

    #[test]
    fn a_platform_with_a_hole_keeps_its_rooms_and_cuts_the_hole() {
        let room = Room {
            polygon: square(0.0, 0.0, 120.0),
            ..sample_room()
        };
        let hole = square(40.0, 40.0, 30.0);
        let m = build_platform(
            Material::Floor,
            std::slice::from_ref(&room),
            std::slice::from_ref(&hole),
            -1.0,
            0.0,
            None,
        )
        .unwrap();
        assert!((volume(&m) - (120.0 * 120.0 - 900.0)).abs() < 1e-3);
        // A hole outside the room leaves it whole.
        let far = square(500.0, 500.0, 30.0);
        let m = build_platform(Material::Floor, &[room], &[far], -1.0, 0.0, None).unwrap();
        assert!((volume(&m) - 120.0 * 120.0).abs() < 1e-3);
    }

    fn sample_room() -> Room {
        let walls = {
            let mut p = Project::new("r");
            for (a, b) in [
                ((0.0, 0.0), (100.0, 0.0)),
                ((100.0, 0.0), (100.0, 100.0)),
                ((100.0, 100.0), (0.0, 100.0)),
                ((0.0, 100.0), (0.0, 0.0)),
            ] {
                p.add_wall(
                    0,
                    Point::new(a.0, a.1),
                    Point::new(b.0, b.1),
                    4.5,
                    96.0,
                    plan_core::WallKind::Interior,
                );
            }
            p.floors[0].walls.clone()
        };
        plan_core::detect_rooms(&walls, 0.5)
            .into_iter()
            .next()
            .expect("a closed loop of walls is one room")
    }
}
