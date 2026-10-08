//! 3D meshes of the landscape objects, draped on the terrain surface.
//!
//! Output uses plan-3d's frame (X right, Y up, Z = -plan y); UVs are in feet.
//!
//! Materials: grass regions are [`Material::Grass`], garden beds
//! [`Material::Mulch`] (or the bed's named material), canopies
//! [`Material::Foliage`], water [`Material::Water`]; trunks use
//! [`Material::Framing`] and edging, basins and stepping stones
//! [`Material::Stone`]. Walls pick [`Material::Concrete`], [`Material::Stone`]
//! or [`Material::Brick`] by name.
//!
//! Every mesh carries the id of its terrain object ([`crate::terrain_object_id`])
//! so the 3D view can pick it.

use std::f64::consts::PI;

use plan_3d::triangulate::ear_clip;
use plan_3d::{Material, Mesh};
use plan_core::geometry::{point_in_polygon, polygon_area};
use plan_core::Point;

use crate::delaunay::triangulate;
use crate::geom::{bounds, dedup_points, densify, dist_to_boundary, offset_polygon, strip_edges};
use crate::landscape::{Landscape, LandscapeKind, TerrainWall};
use crate::mesh::{terrain_object_id, to_scene, MeshBuilder, TerrainPart, UP};
use crate::model::{Feature, FeatureKind, Terrain, TerrainSurface};
use crate::query::elevation_at;

/// Longest span of a draped wall or ring segment, inches.
const SPAN: f64 = 24.0;
/// Lift of flat overlays above the ground to avoid z-fighting, inches.
const LIFT: f64 = 0.5;
/// Thickness of bed edging and the basin wall of a water feature, inches.
const EDGING_THICKNESS: f64 = 2.0;
/// A plant at least this tall (inches) is drawn as a tree on a trunk.
const TREE_HEIGHT: f64 = 96.0;
const SPRINKLER_RADIUS: f64 = 1.5;

/// Ground elevation lookups with a fallback for points off the surface.
pub(crate) struct Ground<'a> {
    surface: Option<&'a TerrainSurface>,
    default_z: f64,
}

impl<'a> Ground<'a> {
    pub(crate) fn new(surface: Option<&'a TerrainSurface>) -> Self {
        let default_z = surface.filter(|s| !s.vertices.is_empty()).map_or(0.0, |s| {
            s.vertices.iter().map(|v| v[1]).sum::<f64>() / s.vertices.len() as f64
        });
        Ground { surface, default_z }
    }

    pub(crate) fn z(&self, p: Point) -> f64 {
        self.surface
            .and_then(|s| elevation_at(s, p))
            .unwrap_or(self.default_z)
    }
}

/// Every landscape mesh of the terrain: walls and curbs, flat feature slabs,
/// beds, grass, water, stepping stones, plants and sprinkler heads. Holes and
/// break lines have no mesh of their own (they shape the surface itself).
pub fn landscape_meshes(t: &Terrain, surface: Option<&TerrainSurface>) -> Vec<Mesh> {
    let ground = Ground::new(surface);
    let mut out = wall_meshes(t, surface);
    for (i, f) in t
        .features
        .iter()
        .enumerate()
        .filter(|(_, f)| f.kind != FeatureKind::Hole)
    {
        let mut ms = feature_meshes(f, &ground);
        tag(&mut ms, TerrainPart::Feature, i);
        out.extend(ms);
    }
    for (i, l) in t.landscape.iter().enumerate() {
        let mut ms = object_meshes(l, &ground);
        tag(&mut ms, TerrainPart::Landscape, i);
        out.extend(ms);
    }
    out
}

/// One mesh per terrain wall or curb: the top is `height` above the ground at
/// every cross-section, the bottom `depth` below it.
pub fn wall_meshes(t: &Terrain, surface: Option<&TerrainSurface>) -> Vec<Mesh> {
    let ground = Ground::new(surface);
    t.walls
        .iter()
        .enumerate()
        .filter_map(|(i, w)| {
            let mut m = wall_mesh(w, &ground)?;
            m.object_id = Some(terrain_object_id(TerrainPart::Wall, i));
            Some(m)
        })
        .collect()
}

/// Gives every mesh the id of terrain object `index` of `part`.
fn tag(meshes: &mut [Mesh], part: TerrainPart, index: usize) {
    for m in meshes {
        m.object_id = Some(terrain_object_id(part, index));
    }
}

fn named_material(name: &str, default: Material) -> Material {
    match name.trim().to_ascii_lowercase().as_str() {
        "stone" | "flagstone" | "fieldstone" => Material::Stone,
        "brick" => Material::Brick,
        "concrete" | "cement" => Material::Concrete,
        "water" => Material::Water,
        "grass" | "lawn" | "turf" => Material::Grass,
        "mulch" | "soil" | "dirt" | "bark" => Material::Mulch,
        "gravel" | "pea gravel" | "crushed stone" => Material::Gravel,
        "asphalt" | "blacktop" => Material::Asphalt,
        _ => default,
    }
}

// ----- sweeps (walls, curbs, edging) -----

struct WRow {
    l: Point,
    r: Point,
    along: f64,
    top: f64,
    bottom: f64,
}

fn quad(b: &mut MeshBuilder, q: [u32; 4], facing: [f64; 3]) {
    b.push_facing([q[0], q[1], q[2]], facing);
    b.push_facing([q[0], q[2], q[3]], facing);
}

/// A ribbon of rectangular sections: left face, top and right face between
/// consecutive rows, plus end caps when open.
fn sweep(rows: &[WRow], closed: bool, material: Material) -> Option<Mesh> {
    if rows.len() < 2 {
        return None;
    }
    let mut b = MeshBuilder::default();
    let ids: Vec<[u32; 4]> = rows
        .iter()
        .map(|r| {
            let x = r.along / 12.0;
            [
                b.push(to_scene(r.l, r.bottom), [x, r.bottom / 12.0]),
                b.push(to_scene(r.l, r.top), [x, r.top / 12.0]),
                b.push(to_scene(r.r, r.top), [x + 0.5, r.top / 12.0]),
                b.push(to_scene(r.r, r.bottom), [x + 0.5, r.bottom / 12.0]),
            ]
        })
        .collect();
    let n = rows.len();
    let pairs = if closed { n } else { n - 1 };
    for i in 0..pairs {
        let j = (i + 1) % n;
        let (a, c) = (ids[i], ids[j]);
        let (l, r) = (rows[i].l, rows[i].r);
        let out = [l.x - r.x, 0.0, -(l.y - r.y)];
        let inward = [-out[0], 0.0, -out[2]];
        quad(&mut b, [a[0], c[0], c[1], a[1]], out);
        quad(&mut b, [a[1], c[1], c[2], a[2]], UP);
        quad(&mut b, [a[2], c[2], c[3], a[3]], inward);
    }
    if !closed {
        let dir = |p: Point, q: Point| [q.x - p.x, 0.0, -(q.y - p.y)];
        let mid = |r: &WRow| Point::lerp(r.l, r.r, 0.5);
        let fwd = dir(mid(&rows[0]), mid(&rows[1]));
        let back = dir(mid(&rows[n - 2]), mid(&rows[n - 1]));
        quad(&mut b, ids[0], [-fwd[0], 0.0, -fwd[2]]);
        quad(
            &mut b,
            [ids[n - 1][3], ids[n - 1][2], ids[n - 1][1], ids[n - 1][0]],
            back,
        );
    }
    Some(b.finish(material))
}

fn wall_mesh(w: &TerrainWall, ground: &Ground) -> Option<Mesh> {
    if w.thickness <= 0.0 {
        return None;
    }
    let edges = strip_edges(&w.points, w.thickness / 2.0);
    if edges.center.len() < 2 {
        return None;
    }
    let mut rows = Vec::new();
    let mut along = 0.0;
    // The surface is read just outside each face: a wall that cuts the terrain
    // has the retained grade on its left and the cut grade on its right. The top
    // stands `height` above the retained side, the bottom `depth` below the
    // lower one.
    let row = |l: Point, r: Point, along: f64| {
        let across = r.sub(l).normalized();
        let gl = ground.z(l.sub(across));
        let gr = ground.z(r.add(across));
        WRow {
            l,
            r,
            along,
            top: gl + w.height,
            bottom: gl.min(gr) - w.depth,
        }
    };
    for i in 0..edges.center.len() - 1 {
        let len = edges.center[i].dist(edges.center[i + 1]);
        let parts = (len / SPAN).ceil().max(1.0) as usize;
        for k in 0..parts {
            let s = k as f64 / parts as f64;
            rows.push(row(
                Point::lerp(edges.left[i], edges.left[i + 1], s),
                Point::lerp(edges.right[i], edges.right[i + 1], s),
                along + len * s,
            ));
        }
        along += len;
    }
    let last = edges.center.len() - 1;
    rows.push(row(edges.left[last], edges.right[last], along));
    sweep(
        &rows,
        false,
        named_material(&w.material, Material::Concrete),
    )
}

/// Rows of a closed ring around `poly`: the left edge `outer` inches off the
/// outline and the right edge `inner` inches off it (positive grows the
/// shape). Heights are set afterwards by [`reground`].
fn ring_rows(poly: &[Point], outer: f64, inner: f64) -> Vec<WRow> {
    let pts = densify(&dedup_points(poly, true), SPAN, true);
    let (o, i) = (offset_polygon(&pts, outer), offset_polygon(&pts, inner));
    if pts.len() < 3 || o.len() != pts.len() || i.len() != pts.len() {
        return Vec::new();
    }
    let mut along = 0.0;
    (0..pts.len())
        .map(|k| {
            if k > 0 {
                along += pts[k - 1].dist(pts[k]);
            }
            WRow {
                l: o[k],
                r: i[k],
                along,
                top: 0.0,
                bottom: 0.0,
            }
        })
        .collect()
}

// ----- flat and draped regions -----

fn flat_polygon(poly: &[Point], z: f64, material: Material) -> Option<Mesh> {
    let pts = dedup_points(poly, true);
    let tris = ear_clip(&pts);
    if tris.is_empty() {
        return None;
    }
    let mut b = MeshBuilder::default();
    let ids: Vec<u32> = pts
        .iter()
        .map(|p| b.push(to_scene(*p, z), [p.x / 12.0, p.y / 12.0]))
        .collect();
    for [a, c, d] in tris {
        b.push_facing([ids[a], ids[c], ids[d]], UP);
    }
    Some(b.finish(material))
}

/// A region lying on the ground (`lift` inches above it).
fn draped_region(poly: &[Point], ground: &Ground, lift: f64, material: Material) -> Option<Mesh> {
    let outline = dedup_points(poly, true);
    if outline.len() < 3 {
        return None;
    }
    let area = polygon_area(&outline).abs();
    if area < 1.0 {
        return None;
    }
    let step = SPAN.max((area / 2500.0).sqrt());
    let mut pts = densify(&outline, step, true);
    if let Some((lo, hi)) = bounds(&outline) {
        let mut y = lo.y + step / 2.0;
        while y < hi.y {
            let mut x = lo.x + step / 2.0;
            while x < hi.x {
                let p = Point::new(x, y);
                if point_in_polygon(p, &outline) && dist_to_boundary(p, &outline) > step * 0.35 {
                    pts.push(p);
                }
                x += step;
            }
            y += step;
        }
    }
    let mut b = MeshBuilder::default();
    let ids: Vec<u32> = pts
        .iter()
        .map(|p| b.push(to_scene(*p, ground.z(*p) + lift), [p.x / 12.0, p.y / 12.0]))
        .collect();
    let mut any = false;
    for [a, c, d] in triangulate(&pts) {
        let centroid = pts[a].add(pts[c]).add(pts[d]).scale(1.0 / 3.0);
        if !point_in_polygon(centroid, &outline) {
            continue;
        }
        b.push_facing([ids[a], ids[c], ids[d]], UP);
        any = true;
    }
    any.then(|| b.finish(material))
}

// ----- features -----

fn feature_meshes(f: &Feature, ground: &Ground) -> Vec<Mesh> {
    let outline = densify(&dedup_points(&f.polygon, true), SPAN, true);
    if outline.len() < 3 {
        return Vec::new();
    }
    if f.pad {
        // The graded surface already is the flat pad with its sloped sides:
        // only the paving lies on top of it.
        let top = ground.z(centroid_of(&outline)).max(
            outline
                .iter()
                .map(|p| ground.z(*p))
                .fold(f64::NEG_INFINITY, f64::max),
        );
        let material = named_material(&f.material, Material::Concrete);
        return flat_polygon(&outline, top + LIFT, material)
            .into_iter()
            .collect();
    }
    let zs: Vec<f64> = outline.iter().map(|p| ground.z(*p)).collect();
    let mean = zs.iter().sum::<f64>() / zs.len() as f64;
    let top = mean + f.height.max(LIFT);
    let material = named_material(&f.material, Material::Concrete);
    let mut out: Vec<Mesh> = flat_polygon(&outline, top, material).into_iter().collect();
    // Skirt down to the ground all round.
    let rows: Vec<WRow> = {
        let mut along = 0.0;
        outline
            .iter()
            .zip(&zs)
            .enumerate()
            .map(|(i, (p, g))| {
                if i > 0 {
                    along += outline[i - 1].dist(*p);
                }
                WRow {
                    l: *p,
                    r: *p,
                    along,
                    top,
                    bottom: g.min(top - LIFT),
                }
            })
            .collect()
    };
    out.extend(skirt(&rows, material));
    out
}

/// A vertical wall along the closed row outline (zero thickness).
fn skirt(rows: &[WRow], material: Material) -> Option<Mesh> {
    let n = rows.len();
    if n < 3 {
        return None;
    }
    let mut b = MeshBuilder::default();
    let ids: Vec<[u32; 2]> = rows
        .iter()
        .map(|r| {
            [
                b.push(to_scene(r.l, r.bottom), [r.along / 12.0, r.bottom / 12.0]),
                b.push(to_scene(r.l, r.top), [r.along / 12.0, r.top / 12.0]),
            ]
        })
        .collect();
    let ccw = polygon_area(&rows.iter().map(|r| r.l).collect::<Vec<_>>()) >= 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        let (p, q) = (rows[i].l, rows[j].l);
        // The outward side of a CCW outline is the right of travel.
        let d = q.sub(p);
        let s = if ccw { 1.0 } else { -1.0 };
        let out = [d.y * s, 0.0, d.x * s];
        quad(&mut b, [ids[i][0], ids[j][0], ids[j][1], ids[i][1]], out);
    }
    Some(b.finish(material))
}

fn centroid_of(pts: &[Point]) -> Point {
    let n = pts.len().max(1) as f64;
    let sum = pts.iter().fold(Point::ZERO, |a, p| a.add(*p));
    Point::new(sum.x / n, sum.y / n)
}

// ----- landscape objects -----

fn object_meshes(l: &Landscape, ground: &Ground) -> Vec<Mesh> {
    match l.kind {
        LandscapeKind::GardenBed => bed_meshes(l, ground),
        LandscapeKind::GrassRegion => {
            draped_region(&l.points, ground, l.height.max(LIFT), Material::Grass)
                .into_iter()
                .collect()
        }
        LandscapeKind::WaterFeature => water_meshes(l, ground),
        LandscapeKind::SteppingStones => stone_meshes(l, ground).into_iter().collect(),
        LandscapeKind::Plants => plant_meshes(l, ground),
        LandscapeKind::Sprinklers => sprinkler_mesh(l, ground).into_iter().collect(),
    }
}

fn bed_meshes(l: &Landscape, ground: &Ground) -> Vec<Mesh> {
    let mut out: Vec<Mesh> = draped_region(
        &l.points,
        ground,
        l.height.max(LIFT),
        named_material(&l.material, Material::Mulch),
    )
    .into_iter()
    .collect();
    if l.edging && l.size > 0.0 {
        let rows = ring_rows(&l.points, EDGING_THICKNESS, 0.0);
        let rows = reground(rows, ground, |g| (g + l.size, g - EDGING_THICKNESS));
        out.extend(sweep(&rows, true, Material::Stone));
    }
    out
}

/// Sets each ring row's top and bottom from the ground under its midpoint.
fn reground(
    mut rows: Vec<WRow>,
    ground: &Ground,
    heights: impl Fn(f64) -> (f64, f64),
) -> Vec<WRow> {
    for r in &mut rows {
        let g = ground.z(Point::lerp(r.l, r.r, 0.5));
        (r.top, r.bottom) = heights(g);
    }
    rows
}

fn water_meshes(l: &Landscape, ground: &Ground) -> Vec<Mesh> {
    let outline = dedup_points(&l.points, true);
    if outline.len() < 3 {
        return Vec::new();
    }
    let ring = densify(&outline, SPAN, true);
    let rim = ring
        .iter()
        .map(|p| ground.z(*p))
        .fold(f64::INFINITY, f64::min);
    let level = rim - l.height.max(0.0);
    let floor = level - l.depth.max(1.0);
    let mut out = Vec::new();
    out.extend(flat_polygon(&outline, level, Material::Water));
    // The basin floor looks up at the water.
    out.extend(flat_polygon(&outline, floor, Material::Stone));
    // Basin walls from grade down to the floor.
    let rows = ring_rows(&outline, EDGING_THICKNESS, 0.0);
    let rows = reground(rows, ground, |g| (g, floor));
    out.extend(sweep(&rows, true, Material::Stone));
    if l.edging && l.size > 0.0 {
        // Coping: a flat stone ring just outside the water, level with the grade.
        let rows = ring_rows(&outline, l.size, 0.0);
        let rows = reground(rows, ground, |g| (g + 1.0, g - 1.0));
        out.extend(sweep(&rows, true, Material::Stone));
    }
    out
}

/// Unit circle point.
fn on_circle(c: Point, r: f64, a: f64) -> Point {
    Point::new(c.x + r * a.cos(), c.y + r * a.sin())
}

fn stone_meshes(l: &Landscape, ground: &Ground) -> Option<Mesh> {
    const SIDES: usize = 8;
    let stones = l.stones();
    if stones.is_empty() || l.size <= 0.0 {
        return None;
    }
    let mut b = MeshBuilder::default();
    for (c, dir) in stones {
        let g = ground.z(c);
        let (top, bottom) = (g + l.height.max(LIFT), g - 1.0);
        let ring: Vec<Point> = (0..SIDES)
            .map(|i| {
                on_circle(
                    c,
                    l.size / 2.0,
                    dir + 2.0 * PI * (i as f64 + 0.5) / SIDES as f64,
                )
            })
            .collect();
        let tops: Vec<u32> = ring
            .iter()
            .map(|p| b.push(to_scene(*p, top), [p.x / 12.0, p.y / 12.0]))
            .collect();
        let center = b.push(to_scene(c, top), [c.x / 12.0, c.y / 12.0]);
        for i in 0..SIDES {
            let j = (i + 1) % SIDES;
            b.push_facing([center, tops[i], tops[j]], UP);
            let (p, q) = (ring[i], ring[j]);
            let lo_i = b.push(to_scene(p, bottom), [p.x / 12.0, bottom / 12.0]);
            let lo_j = b.push(to_scene(q, bottom), [q.x / 12.0, bottom / 12.0]);
            let mid = Point::lerp(p, q, 0.5).sub(c);
            quad(&mut b, [lo_i, lo_j, tops[j], tops[i]], [mid.x, 0.0, -mid.y]);
        }
    }
    Some(b.finish(Material::Stone))
}

/// A closed ellipsoid cap (full when `full`) as triangles.
fn ellipsoid(b: &mut MeshBuilder, c: [f64; 3], r: [f64; 3], full: bool) {
    const SEGS: usize = 10;
    const RINGS: usize = 5;
    let lat0 = if full { -PI / 2.0 } else { 0.0 };
    let rings = if full { RINGS * 2 } else { RINGS };
    let mut grid: Vec<Vec<u32>> = Vec::new();
    for i in 0..=rings {
        let lat = lat0 + (PI / 2.0 - lat0) * i as f64 / rings as f64;
        let row: Vec<u32> = (0..SEGS)
            .map(|k| {
                let lon = 2.0 * PI * k as f64 / SEGS as f64;
                let p = [
                    c[0] + r[0] * lat.cos() * lon.cos(),
                    c[1] + r[1] * lat.sin(),
                    c[2] + r[2] * lat.cos() * lon.sin(),
                ];
                b.push(p, [k as f64 / SEGS as f64, i as f64 / rings as f64])
            })
            .collect();
        grid.push(row);
    }
    for i in 0..rings {
        for k in 0..SEGS {
            let k2 = (k + 1) % SEGS;
            let q = [grid[i][k], grid[i][k2], grid[i + 1][k2], grid[i + 1][k]];
            let mid = |ids: &[u32]| {
                let p: Vec<[f64; 3]> = ids.iter().map(|&i| b.positions[i as usize]).collect();
                let n = p.len() as f64;
                [
                    p.iter().map(|v| v[0]).sum::<f64>() / n - c[0],
                    p.iter().map(|v| v[1]).sum::<f64>() / n - c[1],
                    p.iter().map(|v| v[2]).sum::<f64>() / n - c[2],
                ]
            };
            let out = mid(&q);
            quad(b, q, out);
        }
    }
}

fn cylinder(b: &mut MeshBuilder, base: [f64; 3], radius: f64, height: f64) {
    const SIDES: usize = 8;
    let ring = |y: f64, b: &mut MeshBuilder| -> Vec<u32> {
        (0..SIDES)
            .map(|k| {
                let a = 2.0 * PI * k as f64 / SIDES as f64;
                b.push(
                    [base[0] + radius * a.cos(), y, base[2] + radius * a.sin()],
                    [k as f64 / SIDES as f64, y / 12.0],
                )
            })
            .collect()
    };
    let lo = ring(base[1], b);
    let hi = ring(base[1] + height, b);
    let center = b.push([base[0], base[1] + height, base[2]], [0.5, 0.5]);
    for k in 0..SIDES {
        let k2 = (k + 1) % SIDES;
        let a = 2.0 * PI * (k as f64 + 0.5) / SIDES as f64;
        quad(b, [lo[k], lo[k2], hi[k2], hi[k]], [a.cos(), 0.0, a.sin()]);
        b.push_facing([center, hi[k], hi[k2]], UP);
    }
}

fn plant_meshes(l: &Landscape, ground: &Ground) -> Vec<Mesh> {
    if l.size <= 0.0 || l.height <= 0.0 {
        return Vec::new();
    }
    let (mut canopy, mut trunks) = (MeshBuilder::default(), MeshBuilder::default());
    let tree = l.height >= TREE_HEIGHT;
    let positions = l.plant_positions();
    for p in &positions {
        let at = to_scene(*p, ground.z(*p));
        if tree {
            let trunk_h = l.height * 0.4;
            cylinder(&mut trunks, at, (l.size * 0.03).clamp(1.5, 8.0), trunk_h);
            let r = [l.size / 2.0, (l.height - trunk_h) / 2.0, l.size / 2.0];
            ellipsoid(&mut canopy, [at[0], at[1] + trunk_h + r[1], at[2]], r, true);
        } else {
            let r = [l.size / 2.0, l.height, l.size / 2.0];
            ellipsoid(&mut canopy, at, r, false);
        }
    }
    if positions.is_empty() {
        return Vec::new();
    }
    let mut out = vec![canopy.finish(Material::Foliage)];
    if tree {
        out.push(trunks.finish(Material::Framing));
    }
    out
}

fn sprinkler_mesh(l: &Landscape, ground: &Ground) -> Option<Mesh> {
    let heads = l.heads();
    if heads.is_empty() {
        return None;
    }
    let mut b = MeshBuilder::default();
    for (p, _) in heads {
        let at = to_scene(p, ground.z(p));
        cylinder(&mut b, at, SPRINKLER_RADIUS, l.height.max(LIFT));
    }
    Some(b.finish(Material::Metal))
}
