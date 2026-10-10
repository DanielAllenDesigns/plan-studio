//! Shaping an imported model for the Import 3D Symbol dialog: which way it
//! faces, and its size from the 3D bounding box.
//!
//! All operations work on a model already in the import frame (inches, Y up,
//! front toward +Z) and return a new model.

use crate::model::{ImportedModel, ImportedPart, UpAxis};

/// The direction the imported object's front points, in the import frame
/// (after units and the up-axis fix).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Facing {
    /// Toward +Z: already the symbol's front.
    #[default]
    Front,
    /// Toward -Z.
    Back,
    /// Toward +X.
    Right,
    /// Toward -X.
    Left,
}

impl Facing {
    /// All four, in the order the dialog lists them.
    pub const ALL: [Facing; 4] = [Facing::Front, Facing::Back, Facing::Right, Facing::Left];

    /// The degrees about the vertical axis that bring this direction to
    /// +Z (positive is counter-clockwise seen from above).
    pub fn rotation_deg(self) -> f32 {
        match self {
            Facing::Front => 0.0,
            Facing::Back => 180.0,
            Facing::Right => -90.0,
            Facing::Left => 90.0,
        }
    }

    /// The choice as the file's own axes read: for a Y-up file the front is
    /// +Z, -Z, +X or -X; for a Z-up file (front toward -Y by convention) it
    /// is -Y, +Y, +X or -X.
    pub fn label(self, up: UpAxis) -> &'static str {
        match (up, self) {
            (UpAxis::Y, Facing::Front) => "+Z (default)",
            (UpAxis::Y, Facing::Back) => "-Z",
            (UpAxis::Z, Facing::Front) => "-Y (default)",
            (UpAxis::Z, Facing::Back) => "+Y",
            (_, Facing::Right) => "+X",
            (_, Facing::Left) => "-X",
        }
    }
}

fn map_positions(model: &ImportedModel, f: impl Fn([f32; 3]) -> [f32; 3]) -> ImportedModel {
    ImportedModel {
        parts: model
            .parts
            .iter()
            .map(|p| ImportedPart {
                positions: p.positions.iter().map(|q| f(*q)).collect(),
                ..p.clone()
            })
            .collect(),
    }
}

/// The model turned `deg` about the vertical (Y) axis through the origin.
pub fn rotated_about_y(model: &ImportedModel, deg: f32) -> ImportedModel {
    if deg.rem_euclid(360.0) == 0.0 {
        return model.clone();
    }
    let (s, c) = deg.to_radians().sin_cos();
    map_positions(model, |q| [q[0] * c + q[2] * s, q[1], -q[0] * s + q[2] * c])
}

/// The model turned so that `facing` points to +Z.
pub fn faced(model: &ImportedModel, facing: Facing) -> ImportedModel {
    rotated_about_y(model, facing.rotation_deg())
}

/// The model scaled by `k` along x, y and z.
pub fn scaled(model: &ImportedModel, k: [f32; 3]) -> ImportedModel {
    map_positions(model, |q| [q[0] * k[0], q[1] * k[1], q[2] * k[2]])
}

/// Size of the 3D bounding box as `[width (x), height (y), depth (z)]`.
pub fn size_of(model: &ImportedModel) -> Option<[f64; 3]> {
    model.extent().map(|e| e.map(f64::from))
}

/// The model resized to the given width (x), height (y) and depth (z) in
/// inches. A `None` side is left as it is, or follows the others when
/// `keep_proportions`: then one uniform factor comes from the first given
/// side (width, height, depth in that order). Sides of zero or less are
/// ignored. A model with no extent comes back unchanged.
pub fn resized(
    model: &ImportedModel,
    width: Option<f64>,
    height: Option<f64>,
    depth: Option<f64>,
    keep_proportions: bool,
) -> ImportedModel {
    let Some(size) = size_of(model) else {
        return model.clone();
    };
    let want = [width, height, depth];
    let factor = |i: usize| match want[i] {
        Some(w) if w > 0.0 && size[i] > 1e-9 => Some(w / size[i]),
        _ => None,
    };
    let k = if keep_proportions {
        let u = (0..3).find_map(factor).unwrap_or(1.0);
        [u; 3]
    } else {
        [0, 1, 2].map(|i| factor(i).unwrap_or(1.0))
    };
    scaled(model, k.map(|v| v as f32))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4 (x) x 2 (y) x 1 (z) slab with its "front" marker vertex at +X.
    fn slab() -> ImportedModel {
        ImportedModel {
            parts: vec![ImportedPart {
                positions: vec![
                    [0.0, 0.0, 0.0],
                    [4.0, 0.0, 0.0],
                    [4.0, 2.0, 1.0],
                    [0.0, 2.0, 1.0],
                ],
                indices: vec![0, 1, 2, 0, 2, 3],
                ..ImportedPart::default()
            }],
        }
    }

    #[test]
    fn facing_turns_the_chosen_direction_to_plus_z() {
        let m = slab();
        // The +X marker (4, 0, 0): facing Right brings +X to +Z.
        let r = faced(&m, Facing::Right);
        let p = r.parts[0].positions[1];
        assert!(p[0].abs() < 1e-5 && (p[2] - 4.0).abs() < 1e-5, "{p:?}");
        let l = faced(&m, Facing::Left);
        let q = l.parts[0].positions[1];
        assert!(q[0].abs() < 1e-5 && (q[2] + 4.0).abs() < 1e-5, "{q:?}");
        let b = faced(&m, Facing::Back);
        let o = b.parts[0].positions[1];
        assert!((o[0] + 4.0).abs() < 1e-5 && o[2].abs() < 1e-5, "{o:?}");
        assert_eq!(faced(&m, Facing::Front), m);
    }

    #[test]
    fn right_and_left_swap_width_and_depth() {
        let e = size_of(&faced(&slab(), Facing::Right)).unwrap();
        assert!(
            (e[0] - 1.0).abs() < 1e-5 && (e[2] - 4.0).abs() < 1e-5,
            "{e:?}"
        );
    }

    #[test]
    fn resize_per_axis_or_proportionally() {
        let m = slab();
        let free = size_of(&resized(&m, Some(8.0), None, Some(3.0), false)).unwrap();
        assert!(
            (free[0] - 8.0).abs() < 1e-5
                && (free[1] - 2.0).abs() < 1e-5
                && (free[2] - 3.0).abs() < 1e-5
        );
        let keep = size_of(&resized(&m, None, Some(6.0), None, true)).unwrap();
        assert!(
            (keep[0] - 12.0).abs() < 1e-5
                && (keep[1] - 6.0).abs() < 1e-5
                && (keep[2] - 3.0).abs() < 1e-5
        );
        // Nothing asked, nothing changed; a zero side is ignored.
        assert_eq!(resized(&m, None, None, None, true), m);
        assert_eq!(resized(&m, Some(0.0), None, None, false), m);
    }

    #[test]
    fn labels_follow_the_files_axes() {
        assert_eq!(Facing::Front.label(UpAxis::Z), "-Y (default)");
        assert_eq!(Facing::Back.label(UpAxis::Y), "-Z");
        assert_eq!(Facing::Right.label(UpAxis::Z), "+X");
    }
}
