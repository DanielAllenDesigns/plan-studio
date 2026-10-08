//! plan-framing: Chief-style "Build Framing" for walls and floor platforms.
//!
//! * [`frame_wall`] frames a wall: plates, studs and the king/trimmer/header/
//!   sill/cripple assembly around each opening.
//! * [`frame_floor`] frames a floor platform: joists, rim joists and blocking.
//! * [`Member::mesh`] gives each member a box mesh for the 3D view.
//! * [`wall_detail`] draws the 2D framing elevation of a wall.
//! * [`takeoff`] counts members and totals lumber.
//!
//! Lengths are inches. The 3D frame matches `plan-3d`: X right, Y up,
//! Z = -plan y.

mod defaults;
mod detail;
mod floor;
mod lumber;
mod member;
mod takeoff;
mod wall;

pub use defaults::FramingDefaults;
pub use detail::{wall_detail, Stroke};
pub use floor::{frame_floor, JoistDirection};
pub use lumber::{
    format_inches, Lumber, TWO_BY_EIGHT, TWO_BY_FOUR, TWO_BY_SIX, TWO_BY_TEN, TWO_BY_TWELVE,
};
pub use member::{Member, MemberKind, Transform3, Vec3};
pub use takeoff::{takeoff, Takeoff};
pub use wall::frame_wall;

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::{Point, Wall, WallKind};

    #[test]
    fn every_member_mesh_is_a_closed_box_with_unit_normals() {
        let wall = Wall {
            id: 5,
            start: Point::new(0.0, 0.0),
            end: Point::new(0.0, 144.0),
            thickness: 6.5,
            height: 109.125,
            kind: WallKind::Exterior,
            layer: "Walls, Normal".into(),
        };
        let door = plan_core::Opening::default_door(1, wall.id, 72.0);
        let members = frame_wall(&wall, &[&door], 0.0, &FramingDefaults::default());
        assert!(members.len() > 10);
        for m in &members {
            let mesh = m.mesh();
            assert_eq!(mesh.triangle_count(), 12, "{}", m.label);
            assert_eq!(mesh.vertices.len(), 24);
            assert_eq!(mesh.object_id, Some(5));
            for v in &mesh.vertices {
                let n = v.normal;
                let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                assert!((len - 1.0).abs() < 1e-5);
            }
            // Triangle winding agrees with the stored normal, and the mesh
            // bounds equal the corner bounds.
            for tri in mesh.indices.as_chunks::<3>().0 {
                let p = |i: u32| mesh.vertices[i as usize].position.map(f64::from);
                let (a, b, c) = (p(tri[0]), p(tri[1]), p(tri[2]));
                let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                let n = mesh.vertices[tri[0] as usize].normal.map(f64::from);
                let cr = member::cross(e1, e2);
                assert!(member::dot(cr, n) > 0.0);
            }
            let (lo, hi) = mesh.bounds().unwrap();
            let corners = m.corners();
            for k in 0..3 {
                let cmin = corners.iter().map(|c| c[k]).fold(f64::INFINITY, f64::min);
                let cmax = corners
                    .iter()
                    .map(|c| c[k])
                    .fold(f64::NEG_INFINITY, f64::max);
                assert!((f64::from(lo[k]) - cmin).abs() < 1e-4);
                assert!((f64::from(hi[k]) - cmax).abs() < 1e-4);
            }
        }
    }
}
