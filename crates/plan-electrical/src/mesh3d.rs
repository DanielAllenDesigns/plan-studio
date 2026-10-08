//! 3D fixtures: cover plates with their toggles and receptacle faces, recessed
//! can trims, flush ceiling fixtures, pendants with a cord and a shade, ceiling
//! fans, sconces, detectors, panels, floor boxes and rope-light strips.
//!
//! Scene space matches `plan-3d`: X = plan x, Y = up (offset by the floor
//! elevation), Z = -plan y. Plates and fixture bodies take the device's
//! finish ([`finish_material`], white by default); toggles, slots and can
//! apertures are dark ([`Material::Asphalt`]).
//!
//! [`meshes`] builds one layer; [`electrical_meshes`] walks every floor of a
//! project (the electrical layer must be visible).

use crate::device::{Device, DeviceKind};
use crate::layer::ElectricalLayer;
use plan_3d::{Material, Mesh, Vertex};
use plan_core::geometry::{polygon_area, project_on_segment};
use plan_core::{Point, Project, Wall, DEFAULT_CEILING_HEIGHT};

/// Cover plate size: 2.75" wide, 4.5" tall, 0.25" proud of the wall.
const PLATE: (f64, f64, f64) = (2.75, 4.5, 0.25);
/// Panel box: 14" wide, 20" tall, 4" deep.
const PANEL: (f64, f64, f64) = (14.0, 20.0, 4.0);
/// Flush ceiling fixture disc: 12" across, 2" thick, hanging from its height.
const DISC_RADIUS: f64 = 6.0;
const DISC_THICKNESS: f64 = 2.0;
const DISC_SEGMENTS: usize = 16;
/// Recessed can trim ring (radius, thickness) and its dark aperture radius.
const CAN_TRIM: (f64, f64) = (4.0, 0.5);
const CAN_APERTURE: f64 = 2.6;
/// Pendant shade: bottom radius, top radius and height; the cord is 0.4" wide.
const SHADE: (f64, f64, f64) = (7.0, 3.5, 9.0);
const CORD: f64 = 0.4;
/// Ceiling fan hub (radius, height) and blade (length from the hub edge,
/// width, thickness); the hub hangs 6" below the ceiling.
const FAN_HUB: (f64, f64) = (2.5, 3.0);
const FAN_BLADE: (f64, f64, f64) = (12.0, 5.0, 0.5);
const FAN_DROP: f64 = 6.0;
/// Smoke detector puck: radius and thickness.
const PUCK: (f64, f64) = (4.5, 1.5);
/// Wall sconce body: width, height, depth.
const SCONCE: (f64, f64, f64) = (5.0, 8.0, 4.0);
/// Thermostat body: width, height, depth.
const THERMOSTAT: (f64, f64, f64) = (3.5, 3.5, 1.0);
/// Floor box edge and thickness.
const FLOOR_BOX: (f64, f64) = (4.0, 0.25);
/// Rope light cross-section edge.
const ROPE_SIZE: f64 = 0.75;
/// Dark details (toggle, slots) stand out from the plate by this much, inches.
const DETAIL: f64 = 0.1;

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

    /// A frustum of circular section from `y0` (radius `r0`) up to `y1`
    /// (radius `r1`), with flat caps.
    fn frustum(&mut self, center: Point, (r0, r1): (f64, f64), (y0, y1): (f64, f64)) {
        let ring = |r: f64, a: f64| center + Point::new(a.cos(), a.sin()) * r;
        let h = (y1 - y0).max(1e-6);
        let slope = ((r0 - r1) / h) as f32;
        for i in 0..DISC_SEGMENTS {
            let a = std::f64::consts::TAU * i as f64 / DISC_SEGMENTS as f64;
            let b = std::f64::consts::TAU * (i + 1) as f64 / DISC_SEGMENTS as f64;
            let mid = (a + b) * 0.5;
            let n = [mid.cos() as f32, slope, -(mid.sin() as f32)];
            let (a0, b0) = (scene(ring(r0, a), y0), scene(ring(r0, b), y0));
            let (a1, b1) = (scene(ring(r1, a), y1), scene(ring(r1, b), y1));
            self.tri([a0, b0, b1], n);
            self.tri([a0, b1, a1], n);
            if r1 > 1e-6 {
                self.tri([scene(center, y1), a1, b1], [0.0, 1.0, 0.0]);
            }
            if r0 > 1e-6 {
                self.tri([scene(center, y0), a0, b0], [0.0, -1.0, 0.0]);
            }
        }
    }
}

/// The material a plate or fixture finish stands for (white when unknown).
pub fn finish_material(finish: &str) -> Material {
    match finish.trim().to_ascii_lowercase().as_str() {
        "ivory" | "light almond" => Material::Trim,
        "brown" => Material::DoorPanel,
        "black" => Material::Asphalt,
        "stainless steel" | "stainless" => Material::Metal,
        _ => Material::WindowFrame,
    }
}

/// The parts of one device: one soup per material.
struct Parts<'a> {
    device: &'a Device,
    soups: Vec<(Material, Soup)>,
}

impl<'a> Parts<'a> {
    fn new(device: &'a Device) -> Self {
        Self {
            device,
            soups: Vec::new(),
        }
    }

    fn soup(&mut self, material: Material) -> &mut Soup {
        if let Some(i) = self.soups.iter().position(|(m, _)| *m == material) {
            return &mut self.soups[i].1;
        }
        self.soups.push((material, Soup::default()));
        &mut self.soups.last_mut().expect("just pushed").1
    }

    /// The soup for the plate or body, in the device's finish.
    fn body(&mut self) -> &mut Soup {
        let m = finish_material(&self.device.finish);
        self.soup(m)
    }

    fn finish(self) -> Vec<Mesh> {
        let id = self.device.id;
        self.soups
            .into_iter()
            .filter(|(_, s)| !s.indices.is_empty())
            .map(|(material, s)| Mesh {
                vertices: s.vertices,
                indices: s.indices,
                material,
                object_id: Some(id),
            })
            .collect()
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

/// Plan disc of `radius` around `center`.
fn disc(center: Point, radius: f64) -> Vec<Point> {
    (0..DISC_SEGMENTS)
        .map(|i| {
            let a = std::f64::consts::TAU * i as f64 / DISC_SEGMENTS as f64;
            center + Point::new(a.cos(), a.sin()) * radius
        })
        .collect()
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

/// Dark receptacle slots, a toggle or a jack opening on a plate.
fn plate_details(parts: &mut Parts, face: Point, n: Point, y: f64) {
    let u = n.perp();
    let kind = parts.device.kind;
    let mut dark = |dy: f64, w: f64, h: f64| {
        let o = face + n * (PLATE.2 - 0.01);
        parts.soup(Material::Asphalt).prism(
            &rect(o, u, n, w, DETAIL + 0.01),
            y + dy - h * 0.5,
            y + dy + h * 0.5,
        );
    };
    match kind {
        DeviceKind::Outlet110 | DeviceKind::Gfci | DeviceKind::OutletFloor => {
            dark(1.1, 1.0, 1.3);
            dark(-1.1, 1.0, 1.3);
        }
        DeviceKind::Outlet110Quad => {
            dark(1.6, 1.0, 1.0);
            dark(-1.6, 1.0, 1.0);
            dark(0.4, 0.6, 0.4);
            dark(-0.4, 0.6, 0.4);
        }
        DeviceKind::Outlet220 => dark(0.0, 1.6, 1.6),
        k if k.is_switch() => dark(0.0, 0.6, 1.6),
        DeviceKind::DataJack | DeviceKind::PhoneJack | DeviceKind::TvJack => dark(0.0, 0.9, 0.9),
        _ => {}
    }
}

fn device_parts<'a>(
    d: &'a Device,
    walls: &[Wall],
    elevation: f64,
    ceiling: f64,
) -> Option<Parts<'a>> {
    let mut parts = Parts::new(d);
    let y = elevation + d.height;
    let top = elevation + ceiling;
    match d.kind {
        k if k.is_wall_mounted() => {
            let (face, n) = mount(d, walls);
            let u = n.perp();
            match k {
                DeviceKind::Panel => {
                    let (w, h, t) = PANEL;
                    parts
                        .body()
                        .prism(&rect(face, u, n, w, t), y - h * 0.5, y + h * 0.5);
                }
                DeviceKind::WallSconce => {
                    let (w, h, t) = SCONCE;
                    parts
                        .body()
                        .prism(&rect(face, u, n, w, t), y - h * 0.5, y + h * 0.5);
                }
                DeviceKind::Thermostat => {
                    let (w, h, t) = THERMOSTAT;
                    parts
                        .body()
                        .prism(&rect(face, u, n, w, t), y - h * 0.5, y + h * 0.5);
                }
                _ => {
                    let (w, h, t) = PLATE;
                    parts
                        .body()
                        .prism(&rect(face, u, n, w, t), y - h * 0.5, y + h * 0.5);
                    plate_details(&mut parts, face, n, y);
                }
            }
        }
        DeviceKind::CeilingLight => {
            parts
                .body()
                .prism(&disc(d.position, DISC_RADIUS), y - DISC_THICKNESS, y);
        }
        DeviceKind::RecessedCan => {
            let (r, t) = CAN_TRIM;
            parts.body().prism(&disc(d.position, r), y - t, y);
            parts.soup(Material::Asphalt).prism(
                &disc(d.position, CAN_APERTURE),
                y - t - DETAIL,
                y - t,
            );
        }
        DeviceKind::PendantLight => {
            let (rb, rt, h) = SHADE;
            parts.body().frustum(d.position, (rb, rt), (y, y + h));
            let cord_from = y + h;
            if top > cord_from {
                let c = d.position;
                parts.soup(Material::Metal).prism(
                    &[
                        c + Point::new(-CORD * 0.5, -CORD * 0.5),
                        c + Point::new(CORD * 0.5, -CORD * 0.5),
                        c + Point::new(CORD * 0.5, CORD * 0.5),
                        c + Point::new(-CORD * 0.5, CORD * 0.5),
                    ],
                    cord_from,
                    top,
                );
            }
        }
        DeviceKind::CeilingFan => {
            let (hr, hh) = FAN_HUB;
            let hub_top = y - FAN_DROP;
            parts
                .soup(Material::Metal)
                .prism(&disc(d.position, hr), hub_top - hh, hub_top);
            let (len, width, thick) = FAN_BLADE;
            let body = parts.body();
            for k in 0..4 {
                let a = d.angle + f64::from(k) * std::f64::consts::FRAC_PI_2;
                let dir = Point::new(a.cos(), a.sin());
                let start = d.position + dir * hr;
                body.prism(
                    &rect(start, dir.perp(), dir, width, len),
                    hub_top - hh * 0.5 - thick,
                    hub_top - hh * 0.5,
                );
            }
            // The down-rod from the ceiling to the hub.
            let c = d.position;
            parts.soup(Material::Metal).prism(
                &[
                    c + Point::new(-0.5, -0.5),
                    c + Point::new(0.5, -0.5),
                    c + Point::new(0.5, 0.5),
                    c + Point::new(-0.5, 0.5),
                ],
                hub_top,
                top.max(hub_top),
            );
        }
        DeviceKind::SmokeDetector => {
            let (r, t) = PUCK;
            parts.body().prism(&disc(d.position, r), y - t, y);
        }
        DeviceKind::OutletFloor => {
            let (edge, thick) = FLOOR_BOX;
            let u = Point::new(1.0, 0.0);
            let origin = d.position - Point::new(0.0, edge * 0.5);
            parts.body().prism(
                &rect(origin, u, Point::new(0.0, 1.0), edge, edge),
                elevation,
                elevation + thick,
            );
        }
        DeviceKind::RopeLight { length } => {
            // Runs along the symbol's local Y axis, from the device position.
            let facing = Point::new(d.angle.cos(), d.angle.sin());
            let run = facing.perp();
            parts.body().prism(
                &rect(d.position, facing, run, ROPE_SIZE, length),
                y - ROPE_SIZE * 0.5,
                y + ROPE_SIZE * 0.5,
            );
        }
        _ => return None,
    }
    Some(parts)
}

/// The meshes of one layer, split by material: cover plates (2.75" x 4.5")
/// with dark slots or toggles on walls, trim rings with a dark aperture for
/// cans, 12" discs for flush lights, a shade and cord for pendants, hub and
/// blades for fans, pucks for detectors, a 14" x 20" box for panels, thin
/// floor boxes and rope-light strips. Wall devices sit on the host wall face;
/// ceiling devices hang from the tallest of `walls` (the default ceiling when
/// there are none).
pub fn meshes(layer: &ElectricalLayer, walls: &[Wall], floor_elevation: f64) -> Vec<Mesh> {
    let ceiling = walls
        .iter()
        .map(|w| w.height)
        .fold(f64::NEG_INFINITY, f64::max);
    let ceiling = if ceiling.is_finite() && ceiling > 0.0 {
        ceiling
    } else {
        DEFAULT_CEILING_HEIGHT
    };
    layer
        .devices
        .iter()
        .filter_map(|d| device_parts(d, walls, floor_elevation, ceiling))
        .flat_map(Parts::finish)
        .collect()
}

/// Every floor's electrical devices as 3D meshes, for the 3D view. Nothing is
/// returned when the project's `Electrical` layer is hidden. Floors without an
/// electrical record are skipped.
pub fn electrical_meshes(project: &Project) -> Vec<Mesh> {
    if !project.layers.is_visible("Electrical") {
        return Vec::new();
    }
    let mut out = Vec::new();
    for floor in &project.floors {
        let Ok(Some(layer)) = floor.electrical_as::<ElectricalLayer>() else {
            continue;
        };
        out.extend(meshes(&layer, &floor.walls, floor.elevation));
    }
    out
}
