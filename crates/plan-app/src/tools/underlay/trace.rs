//! Image tracing helpers: the two moves a scanned survey or a photographed
//! plan needs before it can be traced, for underlays and for pictures placed
//! with the Image tools.
//!
//! * **Point to Point Resize**: click two points on the picture that are a
//!   known distance apart in the real building, type that distance; the
//!   picture is scaled about the first point so the two points are exactly
//!   that far apart ([`scale_factor`], [`resize_underlay`], [`resize_picture`]).
//! * **Rotate to Align**: click two points along an edge that should be
//!   level (or plumb); the picture is turned about the first point to the
//!   nearest axis ([`level_angle`], [`align_underlay`], [`align_picture`]).
//!
//! Each is one undo step. Locked underlays and pictures on locked layers are
//! left alone.

use crate::editor::EditorContext;
use plan_core::geometry::Point;
use plan_core::Id;

/// Smallest real distance a resize accepts, inches.
pub const MIN_DISTANCE: f64 = 0.01;
/// Two clicks closer than this are the same point, inches.
pub const MIN_SEPARATION: f64 = 1e-6;

/// The factor that makes the points `a` and `b` `real` inches apart; `None`
/// for coincident points or a distance that is not positive.
pub fn scale_factor(a: Point, b: Point, real: f64) -> Option<f64> {
    let drawn = a.dist(b);
    (drawn >= MIN_SEPARATION && real.is_finite() && real >= MIN_DISTANCE).then(|| real / drawn)
}

/// The turn, degrees counter-clockwise, that brings the line from `a` to `b`
/// to the nearest axis (level, or plumb when it is nearer to that). Within
/// +-45 degrees. Zero for coincident points.
pub fn level_angle(a: Point, b: Point) -> f64 {
    let d = b.sub(a);
    if d.length() < MIN_SEPARATION {
        return 0.0;
    }
    let theta = d.y.atan2(d.x).to_degrees();
    let off = theta - (theta / 90.0).round() * 90.0;
    -off
}

/// `p` turned `deg` degrees counter-clockwise about `about`.
pub fn rotate_about(p: Point, about: Point, deg: f64) -> Point {
    let (s, c) = deg.to_radians().sin_cos();
    let v = p.sub(about);
    Point::new(about.x + v.x * c - v.y * s, about.y + v.x * s + v.y * c)
}

// ----- underlays -----

/// Turns the underlay `id` about `a` so the line `a`-`b` is level (or plumb),
/// as one undo step. False (changing nothing) for a missing or locked
/// underlay and for equal points.
pub fn align_underlay(cx: &mut EditorContext, id: Id, a: Point, b: Point) -> bool {
    let fl = cx.floor;
    let Some(u) = cx.floor().underlay(id) else {
        return false;
    };
    if super::locked(cx, u) {
        cx.status = "That underlay is locked".into();
        return false;
    }
    if a.dist(b) < MIN_SEPARATION {
        cx.status = "Rotate to Align needs two different points".into();
        return false;
    }
    let delta = level_angle(a, b);
    let mut n = u.clone();
    n.origin = rotate_about(n.origin, a, delta);
    n.rotation += delta.to_radians();
    cx.begin_change("Rotate Underlay to Align");
    if let Some(slot) = cx.project.floors[fl].underlay_mut(id) {
        *slot = n;
    }
    cx.mark_dirty();
    cx.status = format!("Underlay turned {delta:.2} degrees");
    true
}

/// Point to Point Resize of an underlay: the same as its two-point
/// calibration, kept here under its Chief name.
pub fn resize_underlay(cx: &mut EditorContext, id: Id, a: Point, b: Point, real: f64) -> bool {
    super::apply_calibration(cx, id, a, b, real)
}

// ----- pictures -----

fn picture_locked(cx: &EditorContext, id: Id) -> Option<bool> {
    let s = cx.floor().symbol(id).filter(|s| s.image.is_some())?;
    Some(cx.layers().is_locked(&s.layer))
}

/// Point to Point Resize of a picture: scales it about `a` so `a` and `b` are
/// `real` inches apart. One undo step. False for something that is not a
/// picture, a picture on a locked layer, equal points or a bad distance.
pub fn resize_picture(cx: &mut EditorContext, id: Id, a: Point, b: Point, real: f64) -> bool {
    let fl = cx.floor;
    match picture_locked(cx, id) {
        None => return false,
        Some(true) => {
            cx.status = "That picture is on a locked layer".into();
            return false;
        }
        Some(false) => {}
    }
    let Some(k) = scale_factor(a, b, real) else {
        cx.status = "Point to Point Resize needs two different points and a distance".into();
        return false;
    };
    cx.begin_change("Point to Point Resize");
    if let Some(s) = cx.project.floors[fl]
        .symbols
        .iter_mut()
        .find(|s| s.id == id)
    {
        s.position = a.add(s.position.sub(a).scale(k));
        s.width *= k;
        s.depth *= k;
        s.height *= k;
    }
    cx.mark_dirty();
    cx.status = format!(
        "Picture scaled by {k:.4}: those points are now {} apart",
        cx.fmt_dim(real)
    );
    true
}

/// Rotate to Align for a picture: turns it about `a` so the line `a`-`b`
/// is level (or plumb). One undo step.
pub fn align_picture(cx: &mut EditorContext, id: Id, a: Point, b: Point) -> bool {
    let fl = cx.floor;
    match picture_locked(cx, id) {
        None => return false,
        Some(true) => {
            cx.status = "That picture is on a locked layer".into();
            return false;
        }
        Some(false) => {}
    }
    if a.dist(b) < MIN_SEPARATION {
        cx.status = "Rotate to Align needs two different points".into();
        return false;
    }
    let delta = level_angle(a, b);
    cx.begin_change("Rotate to Align");
    if let Some(s) = cx.project.floors[fl]
        .symbols
        .iter_mut()
        .find(|s| s.id == id)
    {
        s.position = rotate_about(s.position, a, delta);
        s.angle = (s.angle + delta).rem_euclid(360.0);
    }
    cx.mark_dirty();
    cx.status = format!("Picture turned {delta:.2} degrees");
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_factor_makes_two_points_the_real_distance_apart() {
        let a = Point::new(10.0, 10.0);
        let b = Point::new(13.0, 14.0);
        assert!((scale_factor(a, b, 100.0).unwrap() - 20.0).abs() < 1e-12);
        assert!(scale_factor(a, a, 10.0).is_none());
        assert!(scale_factor(a, b, 0.0).is_none());
        assert!(scale_factor(a, b, f64::NAN).is_none());
    }

    #[test]
    fn the_level_angle_goes_to_the_nearest_axis() {
        let o = Point::ZERO;
        let at = |deg: f64| {
            let (s, c) = deg.to_radians().sin_cos();
            level_angle(o, Point::new(c * 100.0, s * 100.0))
        };
        assert!((at(3.0) + 3.0).abs() < 1e-9, "a slightly rising line");
        assert!((at(-4.0) - 4.0).abs() < 1e-9);
        assert!((at(92.0) + 2.0).abs() < 1e-9, "nearly plumb goes plumb");
        assert!((at(178.0) - 2.0).abs() < 1e-9);
        assert!(at(0.0).abs() < 1e-9 && at(90.0).abs() < 1e-9);
        assert_eq!(level_angle(o, o), 0.0);
    }

    #[test]
    fn rotating_about_a_point_keeps_it_still_and_turns_the_rest() {
        let about = Point::new(5.0, 5.0);
        assert_eq!(rotate_about(about, about, 33.0), about);
        let p = rotate_about(Point::new(15.0, 5.0), about, 90.0);
        assert!(p.dist(Point::new(5.0, 15.0)) < 1e-9);
    }

    use crate::editor::EditorContext;
    use crate::tools::images::tests::fixture_file;

    fn cx() -> EditorContext {
        crate::tools::images::set_user_library_path(Some(None));
        EditorContext::new(crate::plan_defaults::embedded())
    }

    #[test]
    fn an_underlay_turns_about_the_first_point_and_a_locked_one_refuses() {
        let mut cx = cx();
        let id = super::super::import_image(&mut cx, std::path::Path::new(&fixture_file("u.png")))
            .unwrap();
        let before = cx.floor().underlay(id).unwrap().clone();
        // A line along the picture's bottom edge, drawn 3 degrees off.
        let a = before.pixel_to_plan(0.0, 2.0);
        let b = Point::new(a.x + 100.0, a.y + 100.0 * 3f64.to_radians().tan());
        assert!(align_underlay(&mut cx, id, a, b));
        let after = cx.floor().underlay(id).unwrap().clone();
        assert!((after.rotation.to_degrees() + 3.0).abs() < 1e-9);
        // The first point stays put on the picture.
        assert!(after.pixel_to_plan(0.0, 2.0).dist(a) < 1e-9);
        assert_eq!(cx.undo().as_deref(), Some("Rotate Underlay to Align"));
        assert_eq!(cx.floor().underlay(id).unwrap().rotation, 0.0);
        // Equal points and a locked picture change nothing.
        assert!(!align_underlay(&mut cx, id, a, a));
        let fl = cx.floor;
        cx.project.floors[fl].underlay_mut(id).unwrap().locked = true;
        assert!(!align_underlay(&mut cx, id, a, b));
        assert!(!resize_underlay(&mut cx, id, a, b, 50.0));
        assert!(cx.status.contains("locked"));
        assert_eq!(cx.floor().underlay(id).unwrap().rotation, 0.0);
    }

    #[test]
    fn pictures_refuse_bad_input_and_locked_layers() {
        let mut cx = cx();
        let spec = crate::tools::images::load_spec(&fixture_file("p.png")).unwrap();
        let id = crate::tools::images::place_image(&mut cx, &spec, Point::new(0.0, 0.0), false);
        let (a, b) = (Point::new(-10.0, 5.0), Point::new(10.0, 5.0));
        // A bad distance, equal points, and something that is not a picture.
        assert!(!resize_picture(&mut cx, id, a, b, 0.0));
        assert!(!resize_picture(&mut cx, id, a, a, 40.0));
        assert!(!resize_picture(&mut cx, 9999, a, b, 40.0));
        assert!(!align_picture(&mut cx, id, a, a));
        assert!(cx.undo_label().is_none(), "nothing was changed");
        // A picture on a locked layer is left alone.
        let layer = cx.floor().symbol(id).unwrap().layer.clone();
        cx.project.layers.get_mut(&layer).unwrap().locked = true;
        assert!(!resize_picture(&mut cx, id, a, b, 40.0));
        assert!(!align_picture(&mut cx, id, a, b));
        assert!(cx.status.contains("locked"));
        cx.project.layers.get_mut(&layer).unwrap().locked = false;
        let w = cx.floor().symbol(id).unwrap().width;
        assert!(resize_picture(&mut cx, id, a, b, 40.0));
        assert!((cx.floor().symbol(id).unwrap().width - w * 2.0).abs() < 1e-9);
    }
}
