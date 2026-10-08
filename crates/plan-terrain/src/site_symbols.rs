//! Site plan symbols: the North Pointer and the Scale Bar, built as CAD items
//! for the "Site Plan" layer, and the compass arithmetic that goes with the
//! north angle.
//!
//! The *north angle* is the rotation of the plan: degrees clockwise from the
//! top of the plan to true north (0 = north is up the page). It is stored in
//! [`crate::Terrain::north_angle`]; the sun angle turns a true compass azimuth
//! into a direction on the plan with [`plan_azimuth`], and labels name the
//! side a plan direction faces with [`facing_label`].

use plan_core::cad::CadItem;
use plan_core::units::fmt_ft_in;
use plan_core::Point;
use serde::{Deserialize, Serialize};

/// The layer the North Pointer and the Scale Bar are drawn on.
pub const SITE_PLAN_LAYER: &str = "Site Plan";

/// Default radius of the North Pointer, inches.
pub const DEFAULT_POINTER_RADIUS: f64 = 24.0;

/// A placed site symbol and the CAD objects that draw it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SiteMark {
    pub center: Point,
    /// Radius of a North Pointer, or the length of a Scale Bar, inches.
    pub size: f64,
    /// Ids of the CAD objects of the symbol (so it can be replaced).
    pub ids: Vec<u64>,
}

/// The unit vector on the plan that points to true north.
pub fn north_vector(north_angle_deg: f64) -> Point {
    let a = north_angle_deg.to_radians();
    Point::new(a.sin(), a.cos())
}

/// The north angle that makes the pointer from `center` toward `toward` point north.
pub fn north_angle_toward(center: Point, toward: Point) -> f64 {
    let d = toward.sub(center);
    if d.length() < 1e-9 {
        return 0.0;
    }
    d.x.atan2(d.y).to_degrees().rem_euclid(360.0)
}

/// The direction on the plan, as degrees clockwise from the top of the plan,
/// of a true compass azimuth (0 = north, 90 = east).
pub fn plan_azimuth(true_azimuth_deg: f64, north_angle_deg: f64) -> f64 {
    (true_azimuth_deg + north_angle_deg).rem_euclid(360.0)
}

/// The true compass azimuth of a direction on the plan (degrees clockwise from
/// the top of the plan).
pub fn true_azimuth(plan_azimuth_deg: f64, north_angle_deg: f64) -> f64 {
    (plan_azimuth_deg - north_angle_deg).rem_euclid(360.0)
}

/// The compass azimuth a plan vector faces (`None` for a zero vector).
pub fn azimuth_of_plan_vector(v: Point, north_angle_deg: f64) -> Option<f64> {
    (v.length() > 1e-9).then(|| {
        true_azimuth(
            v.x.atan2(v.y).to_degrees().rem_euclid(360.0),
            north_angle_deg,
        )
    })
}

/// The eight-point compass name of an azimuth: "N", "NE", "E", ...
pub fn facing_label(azimuth_deg: f64) -> &'static str {
    const NAMES: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    let idx = ((azimuth_deg.rem_euclid(360.0) + 22.5) / 45.0) as usize % 8;
    NAMES[idx]
}

/// The CAD items of a North Pointer: a circle with an arrow toward north and
/// an "N" beyond its tip.
pub fn north_pointer_items(center: Point, radius: f64, north_angle_deg: f64) -> Vec<CadItem> {
    let r = radius.max(1.0);
    let d = north_vector(north_angle_deg);
    let n = d.perp();
    let at = |along: f64, across: f64| center.add(d.scale(along * r)).add(n.scale(across * r));
    let text_height = r * 0.5;
    let label_at = at(1.0 + 0.45, 0.0);
    vec![
        CadItem::Circle { center, radius: r },
        CadItem::Polyline {
            points: vec![
                at(1.0, 0.0),
                at(-0.55, 0.38),
                at(-0.2, 0.0),
                at(-0.55, -0.38),
            ],
            closed: true,
        },
        CadItem::Line {
            a: at(-0.2, 0.0),
            b: at(1.0, 0.0),
        },
        CadItem::Text {
            // Anchored bottom-left: shift so the letter is centered on the label point.
            pos: Point::new(
                label_at.x - text_height * 0.3,
                label_at.y - text_height * 0.5,
            ),
            text: "N".into(),
            height: text_height,
            angle: 0.0,
        },
    ]
}

/// The CAD items of a Scale Bar: a bar `length` inches long from `start` along
/// `angle` (radians, counter-clockwise from +X), in `divisions` equal parts
/// with a tick and a length label at each division and the alternate parts
/// hatched. The labels read upright.
pub fn scale_bar_items(start: Point, angle: f64, length: f64, divisions: usize) -> Vec<CadItem> {
    let length = length.max(1.0);
    let divisions = divisions.clamp(1, 20);
    let u = Point::new(angle.cos(), angle.sin());
    let n = u.perp();
    let bar_h = (length / 40.0).clamp(3.0, 12.0);
    let at = |along: f64, across: f64| start.add(u.scale(along)).add(n.scale(across));
    let mut items = vec![CadItem::Polyline {
        points: vec![
            at(0.0, 0.0),
            at(length, 0.0),
            at(length, bar_h),
            at(0.0, bar_h),
        ],
        closed: true,
    }];
    let part = length / divisions as f64;
    let text_h = (bar_h * 1.4).max(4.0);
    for k in 0..=divisions {
        let x = k as f64 * part;
        if k > 0 && k < divisions {
            items.push(CadItem::Line {
                a: at(x, 0.0),
                b: at(x, bar_h),
            });
        }
        let label = if k == 0 {
            "0".to_string()
        } else {
            fmt_ft_in(x)
        };
        let anchor = at(x, bar_h + text_h * 0.4);
        items.push(CadItem::Text {
            pos: Point::new(
                anchor.x - label.chars().count() as f64 * text_h * 0.3,
                anchor.y,
            ),
            text: label,
            height: text_h,
            angle: 0.0,
        });
        // Hatch every other part with a diagonal.
        if k < divisions && k % 2 == 0 {
            items.push(CadItem::Line {
                a: at(x, 0.0),
                b: at(x + part, bar_h),
            });
        }
    }
    items
}
