//! Railing walls: posts, rails and balusters instead of a solid wall. The
//! Newels/Balusters and Rails tabs (`Wall.spec.railing`) shape them; the
//! default spec builds the railing these constants describe.

use crate::builder::MeshSet;
use crate::frame::Frame;
use crate::mesh::{Material, Mesh};
use plan_core::Wall;

/// Posts of a railing run with the default spec: one every 96" at most, plus
/// one at each end.
pub fn post_count(length: f64) -> usize {
    plan_core::walls::WallRailing::default().newel_count(length)
}

/// A regular octagon of circumradius `r` about `(s, t)`, for round posts and
/// balusters.
fn octagon(s: f64, t: f64, r: f64) -> Vec<(f64, f64)> {
    (0..8)
        .map(|k| {
            let a = std::f64::consts::TAU * (f64::from(k) + 0.5) / 8.0;
            (s + r * a.cos(), t + r * a.sin())
        })
        .collect()
}

/// A square of side `size` with its corners cut by a quarter of it.
fn chamfered(s: f64, t: f64, size: f64) -> Vec<(f64, f64)> {
    let (h, c) = (size * 0.5, size * 0.25);
    vec![
        (s - h + c, t - h),
        (s + h - c, t - h),
        (s + h, t - h + c),
        (s + h, t + h - c),
        (s + h - c, t + h),
        (s - h + c, t + h),
        (s - h, t + h - c),
        (s - h, t - h + c),
    ]
}

/// A rail along the whole run: a box, or an octagonal tube of the same depth.
fn rail(
    frame: &Frame,
    mesh: &mut crate::builder::MeshBuilder,
    profile: plan_core::walls::RailProfile,
    length: f64,
    width: f64,
    height: f64,
    top: f64,
) {
    let (lo, hi) = (top - height, top);
    match profile {
        plan_core::walls::RailProfile::Rectangular => {
            frame.cuboid(mesh, (0.0, length), (-width * 0.5, width * 0.5), (lo, hi));
        }
        plan_core::walls::RailProfile::Round => {
            // An octagonal tube along the run.
            let (r_t, r_h) = (width * 0.5, height * 0.5);
            let (ct, ch) = (0.0, (lo + hi) * 0.5);
            let ring: Vec<(f64, f64)> = (0..8)
                .map(|k| {
                    let a = std::f64::consts::TAU * (f64::from(k) + 0.5) / 8.0;
                    (ct + r_t * a.cos(), ch + r_h * a.sin())
                })
                .collect();
            for k in 0..8 {
                let (a, b) = (ring[k], ring[(k + 1) % 8]);
                let quad = [
                    frame.point(0.0, a.0, a.1),
                    frame.point(length, a.0, a.1),
                    frame.point(length, b.0, b.1),
                    frame.point(0.0, b.0, b.1),
                ];
                let o = frame.point(0.0, ct, ch);
                let mid = frame.point(0.0, (a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
                let n = [mid[0] - o[0], mid[1] - o[1], mid[2] - o[2]];
                let uv = [
                    [0.0, 0.0],
                    [(length / 12.0) as f32, 0.0],
                    [(length / 12.0) as f32, 0.1],
                    [0.0, 0.1],
                ];
                mesh.quad(quad, uv, n);
            }
        }
    }
}

/// Build the post-and-rail set for `wall` from its Newels/Balusters and Rails
/// tabs. Posts are [`Material::Trim`]; the rails and balusters are
/// [`Material::Metal`]; a glass panel is [`Material::Glass`].
pub fn build_railing(wall: &Wall, elevation: f64) -> Vec<Mesh> {
    use plan_core::walls::{BalusterStyle, NewelStyle, RailFill};
    let spec = &wall.spec.railing;
    let length = wall.length();
    let centers = spec.newel_centers(length);
    if length <= 0.0 {
        return Vec::new();
    }
    let frame = Frame::new(wall, elevation);
    let mut set = MeshSet::default();
    let size = spec.newel_size.max(0.25);
    let half = size * 0.5;
    let rail_top = spec.rail_top();
    let rail_bottom = rail_top - spec.top_rail_height;
    let post_bottom = if spec.post_to_beam {
        -spec.beam_depth.max(0.0)
    } else {
        0.0
    };
    let post_top = spec.post_top();
    for &c in &centers {
        let mesh = set.material(Material::Trim);
        match spec.newel_style {
            NewelStyle::Square => frame.cuboid(
                mesh,
                (c - half, c + half),
                (-half, half),
                (
                    post_bottom,
                    post_top
                        - if spec.newel_cap {
                            plan_core::walls::spec_tabs::NEWEL_CAP_HEIGHT
                        } else {
                            0.0
                        },
                ),
            ),
            NewelStyle::Round => frame.prism(
                mesh,
                &octagon(c, 0.0, half),
                (
                    post_bottom,
                    post_top
                        - if spec.newel_cap {
                            plan_core::walls::spec_tabs::NEWEL_CAP_HEIGHT
                        } else {
                            0.0
                        },
                ),
            ),
            NewelStyle::Chamfered => frame.prism(
                mesh,
                &chamfered(c, 0.0, size),
                (
                    post_bottom,
                    post_top
                        - if spec.newel_cap {
                            plan_core::walls::spec_tabs::NEWEL_CAP_HEIGHT
                        } else {
                            0.0
                        },
                ),
            ),
        }
        if spec.newel_cap {
            let cap = half * 1.25;
            frame.cuboid(
                mesh,
                (c - cap, c + cap),
                (-cap, cap),
                (
                    post_top - plan_core::walls::spec_tabs::NEWEL_CAP_HEIGHT,
                    post_top,
                ),
            );
        }
    }
    let metal = set.material(Material::Metal);
    rail(
        &frame,
        metal,
        spec.top_rail_profile,
        length,
        spec.top_rail_width,
        spec.top_rail_height,
        rail_top,
    );
    let fill_bottom = if spec.bottom_rail {
        rail(
            &frame,
            metal,
            spec.bottom_rail_profile,
            length,
            spec.bottom_rail_width,
            spec.bottom_rail_height,
            spec.bottom_rail_gap + spec.bottom_rail_height,
        );
        spec.bottom_rail_gap + spec.bottom_rail_height
    } else {
        spec.bottom_rail_gap
    };
    match spec.fill {
        RailFill::Balusters => {
            let b = spec.baluster_size.max(0.1) * 0.5;
            for s in spec.baluster_centers(length, &centers) {
                let metal = set.material(Material::Metal);
                let h = (fill_bottom, rail_bottom);
                match spec.baluster_style {
                    BalusterStyle::Square => {
                        frame.cuboid(metal, (s - b, s + b), (-b, b), h);
                    }
                    BalusterStyle::Round => frame.prism(metal, &octagon(s, 0.0, b), h),
                    BalusterStyle::FlatBar => {
                        frame.cuboid(metal, (s - b * 2.0, s + b * 2.0), (-b * 0.5, b * 0.5), h);
                    }
                }
            }
        }
        RailFill::GlassPanel | RailFill::SolidPanel => {
            let material = if spec.fill == RailFill::GlassPanel {
                Material::Glass
            } else {
                Material::Trim
            };
            let mut edges = vec![0.0];
            edges.extend(centers.iter().copied());
            edges.push(length);
            let t = spec.panel_thickness.max(0.05) * 0.5;
            for w in edges.windows(2) {
                let a = w[0]
                    + if w[0] == 0.0 && !spec.newel_at_ends {
                        0.0
                    } else {
                        half
                    };
                let b = w[1]
                    - if w[1] == length && !spec.newel_at_ends {
                        0.0
                    } else {
                        half
                    };
                if b - a > 0.5 {
                    frame.cuboid(
                        set.material(material),
                        (a, b),
                        (-t, t),
                        (fill_bottom, rail_bottom),
                    );
                }
            }
        }
    }
    set.finish(Some(wall.id))
}
