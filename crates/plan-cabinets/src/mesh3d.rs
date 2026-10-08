//! 3D meshes for a cabinet: carcass, toe kick, countertop, fronts, handles.
//!
//! `plan-3d` has no wood or laminate materials, so the carcass and fronts use
//! [`Material::WallInterior`], the countertop uses [`Material::Floor`] as a
//! stand-in and handles use [`Material::WindowFrame`] as a metal stand-in.

use plan_3d::{Material, Mesh, Vertex};
use plan_core::Id;

use crate::cabinet::{Cabinet, CabinetKind, HandleStyle, Overlay};
use crate::face::FaceItem;

/// Side, back, top and bottom panel thickness, inches.
const PANEL: f64 = 0.75;
/// Face-frame thickness, inches.
const FRAME: f64 = 0.75;
/// How far handles stand proud of the front, inches.
const HANDLE_PROJECTION: f64 = 1.0;

type V3 = [f64; 3];

/// The six faces of a box: outward normal and the four `(ix, iy, iz)` corner
/// selectors (0 = min, 1 = max), wound counter-clockwise seen from outside.
const FACES: [([f64; 3], [[u8; 3]; 4]); 6] = [
    (
        [1.0, 0.0, 0.0],
        [[1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1]],
    ),
    (
        [-1.0, 0.0, 0.0],
        [[0, 1, 0], [0, 0, 0], [0, 0, 1], [0, 1, 1]],
    ),
    (
        [0.0, 1.0, 0.0],
        [[1, 1, 0], [0, 1, 0], [0, 1, 1], [1, 1, 1]],
    ),
    (
        [0.0, -1.0, 0.0],
        [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]],
    ),
    (
        [0.0, 0.0, 1.0],
        [[0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]],
    ),
    (
        [0.0, 0.0, -1.0],
        [[0, 1, 0], [1, 1, 0], [1, 0, 0], [0, 0, 0]],
    ),
];

/// Local-frame to scene-frame transform plus the output mesh list.
struct Builder {
    id: Id,
    x: f64,
    y: f64,
    cos: f64,
    sin: f64,
    elevation: f64,
    meshes: Vec<Mesh>,
}

impl Builder {
    fn new(cab: &Cabinet) -> Self {
        let (sin, cos) = cab.angle.sin_cos();
        Self {
            id: cab.id,
            x: cab.position.x,
            y: cab.position.y,
            cos,
            sin,
            elevation: cab.elevation,
            meshes: Vec::new(),
        }
    }

    /// Local `(x, y, z-up)` to scene `(X, Y-up, Z = -plan y)`.
    fn point(&self, p: V3) -> [f32; 3] {
        let px = self.x + p[0] * self.cos - p[1] * self.sin;
        let py = self.y + p[0] * self.sin + p[1] * self.cos;
        [px as f32, (self.elevation + p[2]) as f32, (-py) as f32]
    }

    fn direction(&self, n: V3) -> [f32; 3] {
        let nx = n[0] * self.cos - n[1] * self.sin;
        let ny = n[0] * self.sin + n[1] * self.cos;
        [nx as f32, n[2] as f32, (-ny) as f32]
    }

    /// Add an axis-aligned (in the local frame) box as one mesh. Degenerate
    /// boxes are skipped.
    fn add_box(&mut self, min: V3, max: V3, material: Material) {
        if (0..3).any(|k| max[k] - min[k] <= 1e-9) {
            return;
        }
        let mut vertices = Vec::with_capacity(24);
        let mut indices = Vec::with_capacity(36);
        for (normal, corners) in FACES {
            let axis = (0..3).find(|&k| normal[k] != 0.0).unwrap_or(0);
            let (ua, va) = match axis {
                0 => (1, 2),
                1 => (0, 2),
                _ => (0, 1),
            };
            let base = vertices.len() as u32;
            for sel in corners {
                let p: V3 = std::array::from_fn(|k| if sel[k] == 1 { max[k] } else { min[k] });
                vertices.push(Vertex {
                    position: self.point(p),
                    normal: self.direction(normal),
                    uv: [(p[ua] / 12.0) as f32, (p[va] / 12.0) as f32],
                });
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        self.meshes.push(Mesh {
            vertices,
            indices,
            material,
            object_id: Some(self.id),
        });
    }
}

/// Build the cabinet's 3D meshes.
///
/// * Carcass: side, back and bottom panels (plus a top panel unless it is a
///   base cabinet, whose countertop acts as the top), 3/4" thick.
/// * Base/full-height toe kick: a 3/4" board whose front face is `depth` back
///   from the cabinet front, spanning the toe kick height.
/// * Countertop slab with its overhangs, and an optional backsplash.
/// * Framed cabinets: stiles at both ends and a rail for every separation.
/// * Each drawer, door and appliance front as a panel (door/drawer thickness)
///   adjusted by the [`Overlay`], with a small box handle. Openings and
///   separations in frameless cabinets get no geometry.
///
/// Shelves are a single board and partitions a single 3/4" vertical panel at
/// the local origin end. If the face layout cannot be resolved (fixed items
/// exceed the face) only the carcass is produced.
pub fn meshes(cabinet: &Cabinet) -> Vec<Mesh> {
    let mut b = Builder::new(cabinet);
    let (w, d, h) = (cabinet.width, cabinet.depth, cabinet.height);

    match cabinet.kind {
        CabinetKind::Shelf => {
            b.add_box(
                [0.0, 0.0, (h - PANEL).max(0.0)],
                [w, d, h],
                Material::WallInterior,
            );
            return b.meshes;
        }
        CabinetKind::Partition => {
            b.add_box(
                [0.0, 0.0, 0.0],
                [PANEL.min(w), d, h],
                Material::WallInterior,
            );
            return b.meshes;
        }
        _ => {}
    }

    let toe = cabinet.toe_kick.filter(|t| t.height > 0.0);
    let z0 = toe.map_or(0.0, |t| t.height);
    let top = cabinet.countertop;
    let z1 = h - top.map_or(0.0, |c| c.thickness);
    let door_t = cabinet.door_style.thickness;
    let front_t = door_t.max(cabinet.drawer_style.thickness);
    // Front of the carcass: inset fronts sit flush with it, others stand proud
    // of it so the overall depth stays `depth`.
    let cf = match cabinet.overlay {
        Overlay::Inset { .. } => d,
        _ => d - front_t,
    };

    // Carcass.
    let wi = Material::WallInterior;
    b.add_box([0.0, 0.0, z0], [PANEL, cf, z1], wi);
    b.add_box([w - PANEL, 0.0, z0], [w, cf, z1], wi);
    b.add_box([PANEL, 0.0, z0], [w - PANEL, PANEL, z1], wi);
    b.add_box([PANEL, PANEL, z0], [w - PANEL, cf, z0 + PANEL], wi);
    if cabinet.kind != CabinetKind::Base {
        b.add_box([PANEL, PANEL, z1 - PANEL], [w - PANEL, cf, z1], wi);
    }

    // Toe kick board, front face `depth` back from the cabinet front.
    if let Some(tk) = toe {
        let front = d - tk.depth;
        b.add_box(
            [0.0, (front - FRAME).max(0.0), 0.0],
            [w, front, tk.height],
            wi,
        );
    }

    // Countertop and backsplash.
    if let Some(t) = top {
        b.add_box(
            [-t.overhang_sides, -t.overhang_back, z1],
            [w + t.overhang_sides, d + t.overhang_front, h],
            Material::Floor,
        );
        if let Some(bs) = cabinet.backsplash.filter(|s| s.height > 0.0) {
            b.add_box(
                [-t.overhang_sides, -t.overhang_back, h],
                [
                    w + t.overhang_sides,
                    -t.overhang_back + bs.thickness,
                    h + bs.height,
                ],
                Material::Floor,
            );
        }
    }

    // Face frame and fronts.
    let fw = if cabinet.framed {
        cabinet.face.frame_width
    } else {
        0.0
    };
    let face_w = w - 2.0 * fw;
    if face_w <= 0.0 || z1 <= z0 {
        return b.meshes;
    }
    if cabinet.framed {
        b.add_box([0.0, cf - FRAME, z0], [fw, cf, z1], wi);
        b.add_box([w - fw, cf - FRAME, z0], [w, cf, z1], wi);
    }
    let Ok(items) = cabinet.face.resolve(z1 - z0, face_w) else {
        return b.meshes;
    };
    for r in items {
        let (x, y, iw, ih) = r.rect;
        let (rx0, rx1, rz0, rz1) = (fw + x, fw + x + iw, z0 + y, z0 + y + ih);
        match &r.item {
            FaceItem::Separation { .. } => {
                if cabinet.framed {
                    b.add_box([rx0, cf - FRAME, rz0], [rx1, cf, rz1], wi);
                }
            }
            FaceItem::Opening { .. } => {}
            FaceItem::Drawer { .. } => {
                let t = cabinet.drawer_style.thickness;
                front_panel(&mut b, cabinet, (rx0, rx1, rz0, rz1), t);
                let (cx, cz) = ((rx0 + rx1) / 2.0, (rz0 + rz1) / 2.0);
                handle(&mut b, cabinet.drawer_style.handle, d, cx, cz, true);
            }
            FaceItem::Appliance { .. } => {
                front_panel(&mut b, cabinet, (rx0, rx1, rz0, rz1), door_t);
            }
            FaceItem::DoorLeft { .. } => door(&mut b, cabinet, (rx0, rx1, rz0, rz1), Hinge::Left),
            FaceItem::DoorRight { .. } => door(&mut b, cabinet, (rx0, rx1, rz0, rz1), Hinge::Right),
            FaceItem::DoorAuto { .. } => {
                // Hinge toward the nearer end of the cabinet; centred doors hinge right.
                let hinge = if (rx0 + rx1) / 2.0 < w / 2.0 - 1e-9 {
                    Hinge::Left
                } else {
                    Hinge::Right
                };
                door(&mut b, cabinet, (rx0, rx1, rz0, rz1), hinge);
            }
            FaceItem::DoubleDoor { .. } => {
                let mid = (rx0 + rx1) / 2.0;
                door(&mut b, cabinet, (rx0, mid, rz0, rz1), Hinge::Left);
                door(&mut b, cabinet, (mid, rx1, rz0, rz1), Hinge::Right);
            }
            FaceItem::HorizontalLayout { .. } => {}
        }
    }
    b.meshes
}

#[derive(Clone, Copy)]
enum Hinge {
    Left,
    Right,
}

/// Panel rect `(x0, x1, z0, z1)` adjusted for the overlay and clamped to the cabinet.
fn overlay_rect(cab: &Cabinet, r: (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
    let grow = match cab.overlay {
        Overlay::Full { reveal } => -reveal / 2.0,
        Overlay::Traditional { overlap } => overlap,
        Overlay::Inset { clearance } => -clearance,
    };
    let z0 = cab.toe_kick.map_or(0.0, |t| t.height);
    let z1 = cab.height - cab.countertop.map_or(0.0, |c| c.thickness);
    (
        (r.0 - grow).max(0.0).min(cab.width),
        (r.1 + grow).max(0.0).min(cab.width),
        (r.2 - grow).max(z0),
        (r.3 + grow).min(z1),
    )
}

/// A slab front flush with the cabinet's front plane (`y = depth`).
fn front_panel(b: &mut Builder, cab: &Cabinet, r: (f64, f64, f64, f64), thickness: f64) {
    let (x0, x1, z0, z1) = overlay_rect(cab, r);
    b.add_box(
        [x0, cab.depth - thickness, z0],
        [x1, cab.depth, z1],
        Material::WallInterior,
    );
}

/// A door panel with its handle on the free edge.
fn door(b: &mut Builder, cab: &Cabinet, r: (f64, f64, f64, f64), hinge: Hinge) {
    let style = &cab.door_style;
    front_panel(b, cab, r, style.thickness);
    let (x0, x1, z0, z1) = overlay_rect(cab, r);
    let cx = match hinge {
        Hinge::Left => x1 - style.handle_from_edge,
        Hinge::Right => x0 + style.handle_from_edge,
    };
    // Handles sit near the top of base/tall doors and the bottom of wall doors.
    let cz = if cab.kind == CabinetKind::Wall {
        z0 + style.handle_from_top
    } else {
        z1 - style.handle_from_top
    };
    handle(b, style.handle, cab.depth, cx, cz, false);
}

/// A tiny box handle centred at `(cx, cz)` on the front plane.
fn handle(b: &mut Builder, style: HandleStyle, depth: f64, cx: f64, cz: f64, horizontal: bool) {
    let (hw, hh) = match (style, horizontal) {
        (HandleStyle::None, _) => return,
        (HandleStyle::Knob, _) => (1.0, 1.0),
        (HandleStyle::Pull, true) => (4.0, 0.5),
        (HandleStyle::Pull, false) => (0.5, 4.0),
    };
    b.add_box(
        [cx - hw / 2.0, depth, cz - hh / 2.0],
        [cx + hw / 2.0, depth + HANDLE_PROJECTION, cz + hh / 2.0],
        Material::WindowFrame,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::geometry::Point;

    fn max_y(m: &Mesh) -> f32 {
        m.bounds().unwrap().1[1]
    }

    #[test]
    fn base_has_countertop_at_36_and_recessed_toe_kick() {
        let ms = meshes(&Cabinet::base(24.0));
        let top = ms
            .iter()
            .find(|m| m.material == Material::Floor)
            .expect("countertop");
        assert!((max_y(top) - 36.0).abs() < 1e-4);
        let (lo, hi) = top.bounds().unwrap();
        assert!((lo[1] - 34.5).abs() < 1e-4);
        // Front overhang of 1": plan y = 25, scene z = -25.
        assert!((lo[2] + 25.0).abs() < 1e-4);
        assert!((hi[2] - 0.0).abs() < 1e-4);

        // Toe kick board: bottom at 0, top at 4, front face 3" back (z = -21).
        let kick = ms
            .iter()
            .find(|m| {
                let (lo, hi) = m.bounds().unwrap();
                lo[1].abs() < 1e-4 && (hi[1] - 4.0).abs() < 1e-4
            })
            .expect("toe kick board");
        assert!((kick.bounds().unwrap().0[2] + 21.0).abs() < 1e-4);
    }

    #[test]
    fn all_triangles_face_outward() {
        let mut c = Cabinet::sink_base(36.0);
        c.angle = 0.7;
        c.position = Point::new(5.0, 9.0);
        c.backsplash = Some(crate::Backsplash {
            height: 4.0,
            thickness: 0.5,
        });
        for m in meshes(&c) {
            for t in m.indices.chunks(3) {
                let v: Vec<[f32; 3]> = t.iter().map(|&i| m.vertices[i as usize].position).collect();
                let n = m.vertices[t[0] as usize].normal;
                let e1 = [v[1][0] - v[0][0], v[1][1] - v[0][1], v[1][2] - v[0][2]];
                let e2 = [v[2][0] - v[0][0], v[2][1] - v[0][1], v[2][2] - v[0][2]];
                let cr = [
                    e1[1] * e2[2] - e1[2] * e2[1],
                    e1[2] * e2[0] - e1[0] * e2[2],
                    e1[0] * e2[1] - e1[1] * e2[0],
                ];
                let dot = cr[0] * n[0] + cr[1] * n[1] + cr[2] * n[2];
                assert!(dot > 0.0, "inward triangle in {:?}", m.material);
            }
        }
    }

    #[test]
    fn wall_cabinet_sits_at_54_with_fronts_and_no_countertop() {
        let ms = meshes(&Cabinet::wall(30.0));
        assert!(ms.iter().all(|m| m.material != Material::Floor));
        let (lo, hi) = ms.iter().flat_map(|m| m.vertices.iter()).fold(
            ([f32::MAX; 3], [f32::MIN; 3]),
            |(mut lo, mut hi), v| {
                for k in 0..3 {
                    lo[k] = lo[k].min(v.position[k]);
                    hi[k] = hi[k].max(v.position[k]);
                }
                (lo, hi)
            },
        );
        assert!((lo[1] - 54.0).abs() < 1e-4 && (hi[1] - 84.0).abs() < 1e-4);
        // A door handle (WindowFrame) exists.
        assert!(ms.iter().any(|m| m.material == Material::WindowFrame));
    }

    #[test]
    fn opening_gets_no_front_and_bad_layout_keeps_carcass() {
        let mut c = Cabinet::base(24.0);
        let with_door = meshes(&c).len();
        c.face.items = vec![FaceItem::Opening { height: 0.0 }];
        assert!(meshes(&c).len() < with_door);
        c.face.items = vec![FaceItem::Drawer { height: 99.0 }];
        let carcass_only = meshes(&c);
        assert!(!carcass_only.is_empty());
        assert!(carcass_only
            .iter()
            .all(|m| m.material != Material::WindowFrame));
    }

    #[test]
    fn shelf_and_partition_are_single_panels() {
        assert_eq!(meshes(&Cabinet::new(CabinetKind::Shelf, 36.0)).len(), 1);
        assert_eq!(meshes(&Cabinet::new(CabinetKind::Partition, 0.75)).len(), 1);
    }
}
