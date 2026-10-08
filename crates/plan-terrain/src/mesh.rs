//! 3D meshes: the terrain surface and roads draped on it.
//!
//! Output uses plan-3d's frame: X right, Y up, Z = -plan y; UVs are in feet.

use plan_3d::{Material, Mesh, Vertex};
use plan_core::Point;

use crate::geom::strip_edges;
use crate::landscape::dash_path;
use crate::landscape_mesh::named_material;
use crate::model::{RoadKind, RoadStrip, Terrain, TerrainSurface, MARKING_DASH, MARKING_GAP};
use crate::query::elevation_at;

/// Height roads sit above the terrain to avoid z-fighting, inches.
const ROAD_LIFT: f64 = 0.5;
/// Curb width, inches.
const CURB_WIDTH: f64 = 6.0;
/// Height a painted marking sits above the surface it is laid on, inches.
const MARKING_LIFT: f64 = 0.3;

/// Plan point and elevation to a plan-3d position.
pub(crate) fn to_scene(p: Point, z: f64) -> [f64; 3] {
    [p.x, z, -p.y]
}

/// Accumulates triangles and produces a [`Mesh`] with area-weighted vertex normals.
#[derive(Default)]
pub(crate) struct MeshBuilder {
    pub(crate) positions: Vec<[f64; 3]>,
    pub(crate) uvs: Vec<[f32; 2]>,
    pub(crate) triangles: Vec<[u32; 3]>,
}

impl MeshBuilder {
    pub(crate) fn push(&mut self, position: [f64; 3], uv: [f64; 2]) -> u32 {
        self.positions.push(position);
        self.uvs.push([uv[0] as f32, uv[1] as f32]);
        (self.positions.len() - 1) as u32
    }

    /// Add a triangle, flipping its winding if needed so it faces `facing`.
    pub(crate) fn push_facing(&mut self, tri: [u32; 3], facing: [f64; 3]) {
        let n = self.face_normal(tri);
        let dot: f64 = (0..3).map(|k| n[k] * facing[k]).sum();
        self.triangles.push(if dot < 0.0 {
            [tri[0], tri[2], tri[1]]
        } else {
            tri
        });
    }

    fn face_normal(&self, tri: [u32; 3]) -> [f64; 3] {
        let [a, b, c] = tri.map(|i| self.positions[i as usize]);
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ]
    }

    pub(crate) fn finish(self, material: Material) -> Mesh {
        let mut normals = vec![[0.0f64; 3]; self.positions.len()];
        for &tri in &self.triangles {
            let n = self.face_normal(tri);
            for i in tri {
                for k in 0..3 {
                    normals[i as usize][k] += n[k];
                }
            }
        }
        let vertices = self
            .positions
            .iter()
            .zip(&normals)
            .zip(&self.uvs)
            .map(|((p, n), uv)| {
                let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                let n = if len > 1e-12 {
                    [n[0] / len, n[1] / len, n[2] / len]
                } else {
                    [0.0, 1.0, 0.0]
                };
                Vertex {
                    position: [p[0] as f32, p[1] as f32, p[2] as f32],
                    normal: [n[0] as f32, n[1] as f32, n[2] as f32],
                    uv: *uv,
                }
            })
            .collect();
        Mesh {
            vertices,
            indices: self.triangles.into_iter().flatten().collect(),
            material,
            object_id: None,
            color: None,
        }
    }
}

/// Base of the mesh `object_id`s of terrain objects. They have no ids
/// of their own (they are addressed by index), so their meshes carry
/// `TERRAIN_ID_BASE + part * 2^32 + index`, far above any plan id.
pub const TERRAIN_ID_BASE: u64 = 1 << 56;

/// Which list of the [`Terrain`] a mesh `object_id` indexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainPart {
    Feature,
    Wall,
    Landscape,
    Road,
}

/// The `object_id` of terrain object `index` of `part`.
pub fn terrain_object_id(part: TerrainPart, index: usize) -> u64 {
    let p = match part {
        TerrainPart::Feature => 0_u64,
        TerrainPart::Wall => 1,
        TerrainPart::Landscape => 2,
        TerrainPart::Road => 3,
    };
    TERRAIN_ID_BASE + (p << 32) + index as u64
}

/// The terrain object a mesh `object_id` stands for, if it is one of
/// [`terrain_object_id`]'s.
pub fn terrain_object_of(id: u64) -> Option<(TerrainPart, usize)> {
    let rest = id.checked_sub(TERRAIN_ID_BASE)?;
    let part = match rest >> 32 {
        0 => TerrainPart::Feature,
        1 => TerrainPart::Wall,
        2 => TerrainPart::Landscape,
        3 => TerrainPart::Road,
        _ => return None,
    };
    Some((part, (rest & 0xFFFF_FFFF) as usize))
}

/// The terrain surface as a mesh in [`Material::Grass`]. Normals are the
/// triangle normals averaged (area-weighted) at the vertices; UVs are plan x/y
/// in feet.
pub fn terrain_mesh(surface: &TerrainSurface) -> Mesh {
    surface_mesh(surface, Material::Grass)
}

/// [`terrain_mesh`] in the material the terrain's specification names
/// ([`Terrain::ground_material`]; "Grass" when blank or unknown).
pub fn terrain_mesh_for(t: &Terrain, surface: &TerrainSurface) -> Mesh {
    surface_mesh(surface, named_material(&t.ground_material, Material::Grass))
}

fn surface_mesh(surface: &TerrainSurface, material: Material) -> Mesh {
    let mut b = MeshBuilder::default();
    for v in &surface.vertices {
        b.push(
            to_scene(Point::new(v[0], v[2]), v[1]),
            [v[0] / 12.0, v[2] / 12.0],
        );
    }
    // Plan-CCW triangles face up in the scene frame, so indices copy straight across.
    b.triangles = surface.triangles.clone();
    b.finish(material)
}

/// One mesh per road strip (plus a curb mesh when `curb` is set), draped on `surface`.
///
/// The centerline is offset by half the width to each side with mitered corners,
/// subdivided so no span exceeds half the grid spacing, and each sample is placed on
/// the surface plus 0.5". Samples off the surface (holes, outside the perimeter) take
/// the nearest elevation along the strip.
///
/// The material is the strip's own ([`RoadStrip::material_name`]): asphalt for
/// roads and driveways, concrete for sidewalks and curbs, paint for a road
/// marking. Each carries its road's [`terrain_object_id`]. A strip with a
/// `crown` is `crown` inches higher along its centerline than along its edges;
/// a curb is `curb_height` tall. A marking lies on whatever is under it (the
/// ground, or the crown of a road), and a dashed one is cut into dashes.
pub fn road_meshes(t: &Terrain, surface: &TerrainSurface) -> Vec<Mesh> {
    let step = (t.grid_spacing / 2.0).max(6.0);
    let default_z = if surface.vertices.is_empty() {
        0.0
    } else {
        surface.vertices.iter().map(|v| v[1]).sum::<f64>() / surface.vertices.len() as f64
    };
    let mut meshes = Vec::new();
    for (index, road) in t.roads.iter().enumerate() {
        let id = Some(terrain_object_id(TerrainPart::Road, index));
        if road.kind == RoadKind::Marking {
            let mut mesh = marking_mesh(t, road, surface, step, default_z);
            if let Some(m) = mesh.as_mut() {
                m.object_id = id;
            }
            meshes.extend(mesh);
            continue;
        }
        let Some(rows) = draped_rows(road, surface, step, default_z, &|_| 0.0) else {
            continue;
        };
        let material = named_material(road.material_name(), Material::Asphalt);
        let mut strip = strip_mesh(&rows, material, road.crown.max(0.0));
        strip.object_id = id;
        meshes.push(strip);
        if road.curb {
            let mut curb = curb_mesh(&rows, road.curb_height.max(0.0));
            curb.object_id = id;
            meshes.push(curb);
        }
    }
    meshes
}

/// How far the surface of the roads under `p` stands above the ground there,
/// inches: the road lift plus the crown, which fades to nothing at the edges.
fn road_lift_at(t: &Terrain, p: Point) -> f64 {
    t.roads
        .iter()
        .filter(|r| r.kind != RoadKind::Marking && r.width > 0.0)
        .filter_map(|r| {
            let d = r
                .centerline
                .windows(2)
                .map(|w| plan_core::geometry::dist_to_segment(p, w[0], w[1]))
                .fold(f64::INFINITY, f64::min);
            let half = r.width / 2.0;
            (d <= half).then(|| ROAD_LIFT + r.crown.max(0.0) * (1.0 - d / half))
        })
        .fold(0.0, f64::max)
}

/// The painted strip of a road marking (all its dashes in one mesh).
fn marking_mesh(
    t: &Terrain,
    road: &RoadStrip,
    surface: &TerrainSurface,
    step: f64,
    default_z: f64,
) -> Option<Mesh> {
    let pieces = if road.dashed {
        dash_path(&road.centerline, MARKING_DASH, MARKING_GAP)
    } else {
        vec![road.centerline.clone()]
    };
    let lift = |p: Point| (road_lift_at(t, p) - ROAD_LIFT).max(0.0) + MARKING_LIFT;
    let mut b = MeshBuilder::default();
    let mut any = false;
    for piece in pieces {
        let strip = RoadStrip {
            centerline: piece,
            ..road.clone()
        };
        if let Some(rows) = draped_rows(&strip, surface, step.min(60.0), default_z, &lift) {
            push_strip(&mut b, &rows, 0.0);
            any = true;
        }
    }
    any.then(|| b.finish(named_material(road.material_name(), Material::Trim)))
}

/// A cross-section of a draped strip.
struct Row {
    left: [f64; 3],
    right: [f64; 3],
    /// Distance along the centerline, inches.
    along: f64,
}

fn draped_rows(
    road: &RoadStrip,
    surface: &TerrainSurface,
    step: f64,
    default_z: f64,
    extra_lift: &dyn Fn(Point) -> f64,
) -> Option<Vec<Row>> {
    if road.width <= 0.0 {
        return None;
    }
    let edges = strip_edges(&road.centerline, road.width / 2.0);
    if edges.center.len() < 2 {
        return None;
    }
    // Subdivide every span, keeping the original vertices.
    let mut plan: Vec<(Point, Point, f64)> = Vec::new();
    let mut along = 0.0;
    for i in 0..edges.center.len() - 1 {
        let len = edges.center[i].dist(edges.center[i + 1]);
        let parts = (len / step).ceil().max(1.0) as usize;
        for k in 0..parts {
            let s = k as f64 / parts as f64;
            plan.push((
                Point::lerp(edges.left[i], edges.left[i + 1], s),
                Point::lerp(edges.right[i], edges.right[i + 1], s),
                along + len * s,
            ));
        }
        along += len;
    }
    let last = edges.center.len() - 1;
    plan.push((edges.left[last], edges.right[last], along));

    // Elevation of left/right at every row, with gaps filled from neighbors.
    let mut zs: Vec<Option<f64>> = plan
        .iter()
        .flat_map(|&(l, r, _)| [elevation_at(surface, l), elevation_at(surface, r)])
        .collect();
    let mut carry = None;
    for z in zs.iter_mut() {
        match *z {
            Some(v) => carry = Some(v),
            None => *z = carry,
        }
    }
    let mut carry = None;
    for z in zs.iter_mut().rev() {
        match *z {
            Some(v) => carry = Some(v),
            None => *z = carry,
        }
    }
    let z = |i: usize| zs[i].unwrap_or(default_z) + ROAD_LIFT;
    Some(
        plan.iter()
            .enumerate()
            .map(|(i, &(l, r, along))| Row {
                left: to_scene(l, z(2 * i) + extra_lift(l)),
                right: to_scene(r, z(2 * i + 1) + extra_lift(r)),
                along,
            })
            .collect(),
    )
}

pub(crate) const UP: [f64; 3] = [0.0, 1.0, 0.0];

fn strip_mesh(rows: &[Row], material: Material, crown: f64) -> Mesh {
    let mut b = MeshBuilder::default();
    push_strip(&mut b, rows, crown);
    b.finish(material)
}

/// Adds the strip through `rows` to `b`.
fn push_strip(b: &mut MeshBuilder, rows: &[Row], crown: f64) {
    let width_ft = {
        let (l, r) = (rows[0].left, rows[0].right);
        ((l[0] - r[0]).powi(2) + (l[2] - r[2]).powi(2)).sqrt() / 12.0
    };
    // Per row: left edge, centerline (raised by the crown), right edge.
    let ids: Vec<[u32; 3]> = rows
        .iter()
        .map(|r| {
            let center = [
                (r.left[0] + r.right[0]) / 2.0,
                (r.left[1] + r.right[1]) / 2.0 + crown,
                (r.left[2] + r.right[2]) / 2.0,
            ];
            [
                b.push(r.left, [0.0, r.along / 12.0]),
                b.push(center, [width_ft / 2.0, r.along / 12.0]),
                b.push(r.right, [width_ft, r.along / 12.0]),
            ]
        })
        .collect();
    for w in ids.windows(2) {
        // The two halves of the strip, each as two triangles.
        for (a, c) in [(0, 1), (1, 2)] {
            let (l0, r0, l1, r1) = (w[0][a], w[0][c], w[1][a], w[1][c]);
            b.push_facing([r0, r1, l1], UP);
            b.push_facing([r0, l1, l0], UP);
        }
    }
}

/// Raised curb blocks along both edges of the strip, inside the strip width.
fn curb_mesh(rows: &[Row], curb_height: f64) -> Mesh {
    let mut b = MeshBuilder::default();
    for side in [1.0, -1.0] {
        // Per row: outer bottom, outer top, inner top, inner bottom.
        let ids: Vec<[u32; 4]> = rows
            .iter()
            .map(|r| {
                let (edge, other) = if side > 0.0 {
                    (r.left, r.right)
                } else {
                    (r.right, r.left)
                };
                let (dx, dz) = (other[0] - edge[0], other[2] - edge[2]);
                let len = (dx * dx + dz * dz).sqrt().max(1e-9);
                let inner = [
                    edge[0] + dx / len * CURB_WIDTH,
                    edge[1],
                    edge[2] + dz / len * CURB_WIDTH,
                ];
                let up = |p: [f64; 3]| [p[0], p[1] + curb_height, p[2]];
                let uv = [0.0, r.along / 12.0];
                [
                    b.push(edge, uv),
                    b.push(up(edge), uv),
                    b.push(up(inner), uv),
                    b.push(inner, uv),
                ]
            })
            .collect();
        for w in ids.windows(2) {
            // Outward direction in the scene is away from the strip center.
            let out = {
                let (e, o) = (rows[0].left, rows[0].right);
                let (dx, dz) = ((e[0] - o[0]) * side, (e[2] - o[2]) * side);
                [dx, 0.0, dz]
            };
            let inward = [-out[0], 0.0, -out[2]];
            let faces: [(usize, usize, [f64; 3]); 3] = [(0, 1, out), (1, 2, UP), (2, 3, inward)];
            for (a, c, facing) in faces {
                b.push_facing([w[0][a], w[1][a], w[1][c]], facing);
                b.push_facing([w[0][a], w[1][c], w[0][c]], facing);
            }
        }
    }
    b.finish(Material::Concrete)
}
