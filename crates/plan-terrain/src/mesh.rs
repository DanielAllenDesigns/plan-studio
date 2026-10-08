//! 3D meshes: the terrain surface and roads draped on it.
//!
//! Output uses plan-3d's frame: X right, Y up, Z = -plan y; UVs are in feet.

use plan_3d::{Material, Mesh, Vertex};
use plan_core::Point;

use crate::geom::strip_edges;
use crate::model::{RoadKind, RoadStrip, Terrain, TerrainSurface};
use crate::query::elevation_at;

/// Height roads sit above the terrain to avoid z-fighting, inches.
const ROAD_LIFT: f64 = 0.5;
/// Curb height and width, inches.
const CURB_HEIGHT: f64 = 6.0;
const CURB_WIDTH: f64 = 6.0;

/// Plan point and elevation to a plan-3d position.
fn to_scene(p: Point, z: f64) -> [f64; 3] {
    [p.x, z, -p.y]
}

/// Accumulates triangles and produces a [`Mesh`] with area-weighted vertex normals.
#[derive(Default)]
struct MeshBuilder {
    positions: Vec<[f64; 3]>,
    uvs: Vec<[f32; 2]>,
    triangles: Vec<[u32; 3]>,
}

impl MeshBuilder {
    fn push(&mut self, position: [f64; 3], uv: [f64; 2]) -> u32 {
        self.positions.push(position);
        self.uvs.push([uv[0] as f32, uv[1] as f32]);
        (self.positions.len() - 1) as u32
    }

    /// Add a triangle, flipping its winding if needed so it faces `facing`.
    fn push_facing(&mut self, tri: [u32; 3], facing: [f64; 3]) {
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

    fn finish(self, material: Material) -> Mesh {
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
        }
    }
}

/// The terrain surface as a mesh.
///
/// Deviation: plan-3d has no grass material yet, so this uses [`Material::Floor`]
/// as a stand-in. Normals are the triangle normals averaged (area-weighted) at the
/// vertices; UVs are plan x/y in feet.
pub fn terrain_mesh(surface: &TerrainSurface) -> Mesh {
    let mut b = MeshBuilder::default();
    for v in &surface.vertices {
        b.push(
            to_scene(Point::new(v[0], v[2]), v[1]),
            [v[0] / 12.0, v[2] / 12.0],
        );
    }
    // Plan-CCW triangles face up in the scene frame, so indices copy straight across.
    b.triangles = surface.triangles.clone();
    b.finish(Material::Floor)
}

/// One mesh per road strip (plus a curb mesh when `curb` is set), draped on `surface`.
///
/// The centerline is offset by half the width to each side with mitered corners,
/// subdivided so no span exceeds half the grid spacing, and each sample is placed on
/// the surface plus 0.5". Samples off the surface (holes, outside the perimeter) take
/// the nearest elevation along the strip.
///
/// Deviation: plan-3d has no pavement materials, so roads and driveways use
/// [`Material::Roof`] (charcoal, asphalt stand-in) and sidewalks and curbs use
/// [`Material::WallExterior`] (light, concrete stand-in).
pub fn road_meshes(t: &Terrain, surface: &TerrainSurface) -> Vec<Mesh> {
    let step = (t.grid_spacing / 2.0).max(6.0);
    let default_z = if surface.vertices.is_empty() {
        0.0
    } else {
        surface.vertices.iter().map(|v| v[1]).sum::<f64>() / surface.vertices.len() as f64
    };
    let mut meshes = Vec::new();
    for road in &t.roads {
        let Some(rows) = draped_rows(road, surface, step, default_z) else {
            continue;
        };
        let material = match road.kind {
            RoadKind::Road | RoadKind::Driveway => Material::Roof,
            RoadKind::Sidewalk => Material::WallExterior,
        };
        meshes.push(strip_mesh(&rows, material));
        if road.curb {
            meshes.push(curb_mesh(&rows));
        }
    }
    meshes
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
                left: to_scene(l, z(2 * i)),
                right: to_scene(r, z(2 * i + 1)),
                along,
            })
            .collect(),
    )
}

const UP: [f64; 3] = [0.0, 1.0, 0.0];

fn strip_mesh(rows: &[Row], material: Material) -> Mesh {
    let mut b = MeshBuilder::default();
    let width_ft = {
        let (l, r) = (rows[0].left, rows[0].right);
        ((l[0] - r[0]).powi(2) + (l[2] - r[2]).powi(2)).sqrt() / 12.0
    };
    let ids: Vec<(u32, u32)> = rows
        .iter()
        .map(|r| {
            (
                b.push(r.left, [0.0, r.along / 12.0]),
                b.push(r.right, [width_ft, r.along / 12.0]),
            )
        })
        .collect();
    for w in ids.windows(2) {
        let ((l0, r0), (l1, r1)) = (w[0], w[1]);
        b.push_facing([r0, r1, l1], UP);
        b.push_facing([r0, l1, l0], UP);
    }
    b.finish(material)
}

/// Raised curb blocks along both edges of the strip, inside the strip width.
fn curb_mesh(rows: &[Row]) -> Mesh {
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
                let up = |p: [f64; 3]| [p[0], p[1] + CURB_HEIGHT, p[2]];
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
    b.finish(Material::WallExterior)
}
