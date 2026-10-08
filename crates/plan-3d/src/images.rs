//! 3D geometry of pictures (Create Image / Create Billboard Image).
//!
//! The GL path has no textures yet, so a picture is a flat-coloured quad: its
//! average colour ([`plan_core::ImageSpec::color`]) mapped to the nearest
//! [`Material`]. A flat picture lies on its footprint at the symbol's
//! elevation; a billboard stands upright over its base point and turns to
//! face the camera ([`plan_core::images::billboard_angle`]).
//!
//! Scene axes as everywhere in this crate: X = plan x, Y = up, Z = -plan y.
//! Every quad is two-sided (a front and a back face) so a picture is visible
//! from underneath and behind.

use crate::mesh::{Material, Mesh, Vertex};
use plan_core::geometry::Point;
use plan_core::images::billboard_angle;
use plan_core::{PlacedSymbol, Project};

/// Lift of a flat picture above the surface under it, inches (avoids
/// z-fighting with a floor drawn at the same height).
pub const FLAT_LIFT: f64 = 0.1;

/// The material whose colour is closest to `rgb`. Never a Floor, Ceiling or
/// Roof: the Doll House and overview views hide those, and a picture must
/// stay visible and pickable in every view (QA-11).
pub fn nearest_material(rgb: [u8; 3]) -> Material {
    let c = [
        f32::from(rgb[0]) / 255.0,
        f32::from(rgb[1]) / 255.0,
        f32::from(rgb[2]) / 255.0,
    ];
    Material::ALL
        .iter()
        .copied()
        .filter(|m| m.color()[3] >= 1.0)
        .filter(|m| !matches!(m, Material::Floor | Material::Ceiling | Material::Roof))
        .min_by(|a, b| {
            let d = |m: &Material| {
                let k = m.color();
                (0..3).map(|i| (k[i] - c[i]).powi(2)).sum::<f32>()
            };
            d(a).total_cmp(&d(b))
        })
        .unwrap_or(Material::Trim)
}

/// Where the camera is, in scene axes, or `None` when billboards should keep
/// the angle stored on the symbol.
pub type Eye = Option<[f32; 3]>;

/// The angle (degrees) a billboard at `sym` is drawn at for camera `eye`
/// (scene axes); the stored angle when there is no eye or the eye is on top
/// of it.
pub fn billboard_orientation(sym: &PlacedSymbol, eye: Eye) -> f64 {
    let Some(e) = eye else {
        return sym.angle;
    };
    let base = base_point(sym);
    billboard_angle(base, Point::new(f64::from(e[0]), -f64::from(e[2]))).unwrap_or(sym.angle)
}

/// The point a billboard stands on: the middle of its footprint.
fn base_point(sym: &PlacedSymbol) -> Point {
    let a = sym.angle.to_radians();
    let v = Point::new(-a.sin(), a.cos());
    sym.position + v * (sym.depth * 0.5)
}

fn quad(mesh: &mut Mesh, c: [[f32; 3]; 4], n: [f32; 3]) {
    // Front face with normal `n` (counter-clockwise seen from `n`), then the
    // back face with the reverse winding.
    let a = c[0];
    let e1 = [c[1][0] - a[0], c[1][1] - a[1], c[1][2] - a[2]];
    let e2 = [c[2][0] - a[0], c[2][1] - a[1], c[2][2] - a[2]];
    let cross = [
        e1[1] * e2[2] - e1[2] * e2[1],
        e1[2] * e2[0] - e1[0] * e2[2],
        e1[0] * e2[1] - e1[1] * e2[0],
    ];
    let flip = cross[0] * n[0] + cross[1] * n[1] + cross[2] * n[2] < 0.0;
    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    for (side, sign) in [(0, 1.0f32), (1, -1.0f32)] {
        let base = mesh.vertices.len() as u32;
        for (k, p) in c.iter().enumerate() {
            mesh.vertices.push(Vertex {
                position: *p,
                normal: [n[0] * sign, n[1] * sign, n[2] * sign],
                uv: uvs[k],
            });
        }
        let front_order: [u32; 6] = if flip {
            [0, 2, 1, 0, 3, 2]
        } else {
            [0, 1, 2, 0, 2, 3]
        };
        let order: [u32; 6] = if side == 0 {
            front_order
        } else {
            [
                front_order[0],
                front_order[2],
                front_order[1],
                front_order[3],
                front_order[5],
                front_order[4],
            ]
        };
        mesh.indices.extend(order.iter().map(|i| base + i));
    }
}

/// The mesh of one picture symbol; `None` when `sym` is not a picture.
/// `floor_elevation` is the floor's height; `eye` orients billboards.
pub fn image_mesh(sym: &PlacedSymbol, floor_elevation: f64, eye: Eye) -> Option<Mesh> {
    let spec = sym.image.as_ref()?;
    let material = nearest_material(spec.color);
    let mut mesh = Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        material,
        object_id: Some(sym.id),
    };
    let y0 = (floor_elevation + sym.elevation) as f32;
    let at = |p: Point, y: f32| [p.x as f32, y, -p.y as f32];
    if spec.billboard {
        let angle = billboard_orientation(sym, eye);
        let a = angle.to_radians();
        let u = Point::new(a.cos(), a.sin());
        let v = Point::new(-a.sin(), a.cos());
        let c = base_point(sym);
        let hw = sym.width * 0.5;
        let h = sym.height.max(1.0) as f32;
        let (l, r) = (c - u * hw, c + u * hw);
        quad(
            &mut mesh,
            [at(l, y0), at(r, y0), at(r, y0 + h), at(l, y0 + h)],
            [v.x as f32, 0.0, -v.y as f32],
        );
    } else {
        let f = sym.footprint();
        let y = y0 + FLAT_LIFT as f32;
        quad(
            &mut mesh,
            [at(f[0], y), at(f[1], y), at(f[2], y), at(f[3], y)],
            [0.0, 1.0, 0.0],
        );
    }
    Some(mesh)
}

/// Every picture of the project as meshes; `eye` is the camera in scene axes.
pub fn image_meshes(project: &Project, eye: Eye) -> Vec<Mesh> {
    let mut out = Vec::new();
    for f in &project.floors {
        for s in &f.symbols {
            if let Some(m) = image_mesh(s, f.elevation, eye) {
                out.push(m);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::ImageSpec;

    fn spec(color: [u8; 3]) -> ImageSpec {
        let mut s = ImageSpec::new("x.png", 10, 10);
        s.color = color;
        s
    }

    fn normal_of(m: &Mesh) -> [f32; 3] {
        m.vertices[0].normal
    }

    #[test]
    fn flat_pictures_lie_on_their_footprint() {
        let mut s =
            PlacedSymbol::picture(spec([230, 230, 225]), Point::new(100.0, 50.0), 40.0, 20.0);
        s.id = 3;
        s.elevation = 2.0;
        let m = image_mesh(&s, 120.0, None).unwrap();
        assert_eq!(m.triangle_count(), 4, "two-sided quad");
        assert_eq!(m.object_id, Some(3));
        let (lo, hi) = m.bounds().unwrap();
        assert!((lo[0] - 80.0).abs() < 1e-4 && (hi[0] - 120.0).abs() < 1e-4);
        assert!((lo[1] - 122.1).abs() < 1e-4 && (hi[1] - 122.1).abs() < 1e-4);
        assert!((lo[2] + 70.0).abs() < 1e-4 && (hi[2] + 50.0).abs() < 1e-4);
        assert_eq!(normal_of(&m), [0.0, 1.0, 0.0]);
        assert!(image_mesh(
            &PlacedSymbol::new("a", Point::ZERO, 1.0, 1.0, 1.0),
            0.0,
            None
        )
        .is_none());
    }

    #[test]
    fn colours_map_to_the_nearest_opaque_material() {
        assert_eq!(nearest_material([158, 77, 56]), Material::Brick);
        // QA-11: near-white used to map to Ceiling and green to Roof, which the
        // Doll House hides; pictures take the nearest non-structural material.
        for rgb in [[250, 250, 248], [100, 120, 90], [150, 150, 150]] {
            let m = nearest_material(rgb);
            assert!(!matches!(m, Material::Floor | Material::Ceiling | Material::Roof));
        }
        assert!(nearest_material([0, 0, 255]).color()[3] >= 1.0);
    }

    #[test]
    fn billboards_stand_upright_and_face_the_camera() {
        let s = PlacedSymbol::billboard(spec([100, 120, 90]), Point::new(100.0, 100.0), 40.0, 72.0);
        // Camera north-east of the billboard, 60" up (scene axes: z = -y).
        for eye_plan in [
            Point::new(400.0, 400.0),
            Point::new(-300.0, 100.0),
            Point::new(100.0, -500.0),
        ] {
            let eye = Some([eye_plan.x as f32, 60.0, -eye_plan.y as f32]);
            let m = image_mesh(&s, 0.0, eye).unwrap();
            let n = normal_of(&m);
            assert_eq!(n[1], 0.0, "upright");
            let base = base_point(&s);
            let to_eye = [
                (eye_plan.x - base.x) as f32,
                0.0,
                (-(eye_plan.y - base.y)) as f32,
            ];
            let len = (to_eye[0] * to_eye[0] + to_eye[2] * to_eye[2]).sqrt();
            let dot = (n[0] * to_eye[0] + n[2] * to_eye[2]) / len;
            assert!(dot > 0.9999, "{eye_plan:?} dot {dot}");
            let (lo, hi) = m.bounds().unwrap();
            assert!((hi[1] - lo[1] - 72.0).abs() < 1e-4);
        }
        // Without an eye the stored angle is kept.
        let m = image_mesh(&s, 0.0, None).unwrap();
        assert_eq!(normal_of(&m), [0.0, 0.0, -1.0]);
        assert_eq!(billboard_orientation(&s, None), 0.0);
        assert!((billboard_orientation(&s, Some([100.0, 5.0, -400.0])) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn project_pictures_are_collected_per_floor() {
        let mut p = Project::new("p");
        p.add_symbol(
            0,
            PlacedSymbol::picture(spec([10, 10, 10]), Point::ZERO, 10.0, 10.0),
        );
        p.add_symbol(0, PlacedSymbol::new("plain", Point::ZERO, 10.0, 10.0, 10.0));
        assert_eq!(image_meshes(&p, None).len(), 1);
    }
}
