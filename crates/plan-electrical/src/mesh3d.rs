//! 3D stand-ins: cover plates, ceiling discs and strips as plain meshes.
//!
//! Scene space matches `plan-3d`: X = plan x, Y = up (offset by the floor
//! elevation), Z = -plan y. All stand-ins use [`Material::WindowFrame`] (white).

use crate::device::{Device, DeviceKind};
use crate::layer::ElectricalLayer;
use plan_3d::{Material, Mesh, Vertex};
use plan_core::geometry::{polygon_area, project_on_segment};
use plan_core::{Point, Wall};

/// Cover plate size: 2.75" wide, 4.5" tall, 0.25" proud of the wall.
const PLATE: (f64, f64, f64) = (2.75, 4.5, 0.25);
/// Panel box: 14" wide, 20" tall, 4" deep.
const PANEL: (f64, f64, f64) = (14.0, 20.0, 4.0);
/// Ceiling fixture disc: 6" across, 1" thick, hanging from its height.
const DISC_RADIUS: f64 = 3.0;
const DISC_THICKNESS: f64 = 1.0;
const DISC_SEGMENTS: usize = 16;
/// Floor box edge and thickness.
const FLOOR_BOX: (f64, f64) = (4.0, 0.25);
/// Rope light cross-section edge.
const ROPE_SIZE: f64 = 0.75;

fn scene(p: Point, y: f64) -> [f32; 3] {
    [p.x as f32, y as f32, -p.y as f32]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Triangle soup with flat normals and wound to face the stated normal.
#[derive(Default)]
struct Soup {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Soup {
    fn tri(&mut self, p: [[f32; 3]; 3], normal: [f32; 3]) {
        let c = cross(sub(p[1], p[0]), sub(p[2], p[0]));
        let flip = c[0] * normal[0] + c[1] * normal[1] + c[2] * normal[2] < 0.0;
        let order = if flip { [0, 2, 1] } else { [0, 1, 2] };
        let base = self.vertices.len() as u32;
        for k in order {
            self.vertices.push(Vertex {
                position: p[k],
                normal,
                uv: [p[k][0] / 12.0, p[k][2] / 12.0],
            });
        }
        self.indices.extend([base, base + 1, base + 2]);
    }

    /// Extrude a convex plan outline between heights `y0` and `y1`.
    fn prism(&mut self, outline: &[Point], y0: f64, y1: f64) {
        let mut pts = outline.to_vec();
        if polygon_area(&pts) < 0.0 {
            pts.reverse();
        }
        for k in 1..pts.len() - 1 {
            self.tri(
                [scene(pts[0], y1), scene(pts[k], y1), scene(pts[k + 1], y1)],
                [0.0, 1.0, 0.0],
            );
            self.tri(
                [scene(pts[0], y0), scene(pts[k], y0), scene(pts[k + 1], y0)],
                [0.0, -1.0, 0.0],
            );
        }
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            let d = b.sub(a).normalized();
            // Outward (right-hand) plan normal (dy, -dx) in scene space.
            let n = [d.y as f32, 0.0, d.x as f32];
            let (a0, b0, a1, b1) = (scene(a, y0), scene(b, y0), scene(a, y1), scene(b, y1));
            self.tri([a0, b0, b1], n);
            self.tri([a0, b1, a1], n);
        }
    }

    fn finish(self, device: &Device) -> Mesh {
        Mesh {
            vertices: self.vertices,
            indices: self.indices,
            material: Material::WindowFrame,
            object_id: Some(device.id),
        }
    }
}

/// Plan rectangle from `origin`, `width` along `u` (centered) and `depth` along `n`.
fn rect(origin: Point, u: Point, n: Point, width: f64, depth: f64) -> [Point; 4] {
    let half = u * (width * 0.5);
    [
        origin - half,
        origin + half,
        origin + half + n * depth,
        origin - half + n * depth,
    ]
}

/// Wall face position and outward unit normal, re-derived from the host wall
/// when it is available so plates follow thickness changes.
fn mount(d: &Device, walls: &[Wall]) -> (Point, Point) {
    let facing = Point::new(d.angle.cos(), d.angle.sin());
    let host = d.wall_id.and_then(|id| walls.iter().find(|w| w.id == id));
    match host {
        Some(w) => {
            let n = if facing.dot(w.normal()) >= 0.0 {
                w.normal()
            } else {
                -w.normal()
            };
            let on_center = project_on_segment(d.position, w.start, w.end).1;
            (on_center + n * (w.thickness * 0.5), n)
        }
        None => (d.position, facing),
    }
}

fn device_mesh(d: &Device, walls: &[Wall], elevation: f64) -> Option<Mesh> {
    let mut soup = Soup::default();
    let y = elevation + d.height;
    if d.kind.is_wall_mounted() {
        let (w, h, t) = if d.kind == DeviceKind::Panel {
            PANEL
        } else {
            PLATE
        };
        let (face, n) = mount(d, walls);
        soup.prism(&rect(face, n.perp(), n, w, t), y - h * 0.5, y + h * 0.5);
    } else if d.kind.is_ceiling() {
        let disc: Vec<Point> = (0..DISC_SEGMENTS)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / DISC_SEGMENTS as f64;
                d.position + Point::new(a.cos(), a.sin()) * DISC_RADIUS
            })
            .collect();
        soup.prism(&disc, y - DISC_THICKNESS, y);
    } else if d.kind == DeviceKind::OutletFloor {
        let (edge, thick) = FLOOR_BOX;
        let u = Point::new(1.0, 0.0);
        let origin = d.position - Point::new(0.0, edge * 0.5);
        soup.prism(
            &rect(origin, u, Point::new(0.0, 1.0), edge, edge),
            elevation,
            elevation + thick,
        );
    } else if let DeviceKind::RopeLight { length } = d.kind {
        // Runs along the symbol's local Y axis, from the device position.
        let facing = Point::new(d.angle.cos(), d.angle.sin());
        let run = facing.perp();
        soup.prism(
            &rect(d.position, facing, run, ROPE_SIZE, length),
            y - ROPE_SIZE * 0.5,
            y + ROPE_SIZE * 0.5,
        );
    } else {
        return None;
    }
    Some(soup.finish(d))
}

/// One white box or disc per device: cover plates (2.75" x 4.5") on walls,
/// 6" discs on ceilings, a 14" x 20" box for panels, thin floor boxes and
/// rope-light strips. Wall plates are placed on the host wall face.
pub fn meshes(layer: &ElectricalLayer, walls: &[Wall], floor_elevation: f64) -> Vec<Mesh> {
    layer
        .devices
        .iter()
        .filter_map(|d| device_mesh(d, walls, floor_elevation))
        .collect()
}
