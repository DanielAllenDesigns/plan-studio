//! Railing walls: posts, rails and balusters instead of a solid wall.

use crate::builder::MeshSet;
use crate::frame::Frame;
use crate::mesh::{Material, Mesh};
use plan_core::Wall;

/// Maximum post spacing, 8'.
const POST_SPACING: f64 = 96.0;
/// Square post size.
const POST_SIZE: f64 = 3.5;
/// Top of the top rail above the floor, 36".
pub const RAIL_TOP: f64 = 36.0;
/// Top rail height.
const RAIL_HEIGHT: f64 = 1.5;
/// Bottom rail: gap above floor and height.
const BOTTOM_RAIL: (f64, f64) = (3.0, 4.5);
/// Baluster center spacing, 4".
const BALUSTER_SPACING: f64 = 4.0;
/// Square baluster size.
const BALUSTER_SIZE: f64 = 0.75;

/// Posts of a railing run: one every 96" at most, plus one at each end.
pub fn post_count(length: f64) -> usize {
    if length <= 0.0 {
        return 0;
    }
    (length / POST_SPACING).ceil().max(1.0) as usize + 1
}

/// Build the post-and-rail set for `wall`. Posts are [`Material::Trim`]; the
/// rails and balusters are [`Material::Metal`].
pub fn build_railing(wall: &Wall, elevation: f64) -> Vec<Mesh> {
    let length = wall.length();
    let posts = post_count(length);
    if posts == 0 {
        return Vec::new();
    }
    let frame = Frame::new(wall, elevation);
    let mut set = MeshSet::default();
    let half = POST_SIZE * 0.5;
    let t_post = (-half, half);
    let rail_bottom = RAIL_TOP - RAIL_HEIGHT;

    let centers: Vec<f64> = (0..posts)
        .map(|i| (length * i as f64 / (posts - 1) as f64).clamp(half, (length - half).max(half)))
        .collect();
    for &c in &centers {
        frame.cuboid(
            set.material(Material::Trim),
            (c - half, c + half),
            t_post,
            (0.0, rail_bottom),
        );
    }
    let rail_t = (-half * 0.6, half * 0.6);
    let metal = set.material(Material::Metal);
    frame.cuboid(metal, (0.0, length), rail_t, (rail_bottom, RAIL_TOP));
    frame.cuboid(
        metal,
        (0.0, length),
        (-0.5, 0.5),
        (BOTTOM_RAIL.0, BOTTOM_RAIL.1),
    );
    let clear = half + BALUSTER_SIZE * 0.5;
    let b = BALUSTER_SIZE * 0.5;
    let mut s = BALUSTER_SPACING * 0.5;
    while s < length {
        if centers.iter().all(|c| (c - s).abs() >= clear) {
            frame.cuboid(metal, (s - b, s + b), (-b, b), (BOTTOM_RAIL.1, rail_bottom));
        }
        s += BALUSTER_SPACING;
    }
    set.finish(Some(wall.id))
}
