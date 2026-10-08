//! Site plan symbols in the plan: the North Pointer and the Scale Bar are CAD
//! objects on the "Site Plan" layer. The North Pointer also stores the plan's
//! *north angle* in the terrain record, which the sun angle ([`plan_sun_azimuth`])
//! and compass labels use.

use super::{load_terrain, save_terrain};
use crate::editor::EditorContext;
use plan_core::{Layer, Point, Project};
use plan_terrain::{
    north_angle_toward, north_pointer_items, plan_azimuth, scale_bar_items, SiteMark,
    SITE_PLAN_LAYER,
};

/// Smallest North Pointer radius, inches.
const MIN_POINTER_RADIUS: f64 = 12.0;
/// Longest radius a click can make, inches.
const MAX_POINTER_RADIUS: f64 = 1200.0;

/// Adds the Site Plan layer to the plan when it is missing (no undo step of
/// its own: call it inside the edit).
pub fn ensure_site_plan_layer(project: &mut Project) {
    project
        .layers
        .add(Layer::new(SITE_PLAN_LAYER, [70, 70, 78], 18));
}

/// The plan's north angle: degrees clockwise from the top of the plan to true
/// north (0 until a North Pointer is placed).
pub fn north_angle(project: &Project) -> f64 {
    load_terrain(project).map_or(0.0, |r| r.terrain.north_angle)
}

/// The direction on the plan, degrees clockwise from its top, of the true
/// compass azimuth `true_azimuth_deg` once the north angle is applied. The
/// sun angle uses this to cast shadows the way the real sun does.
pub fn plan_sun_azimuth(project: &Project, true_azimuth_deg: f64) -> f64 {
    plan_azimuth(true_azimuth_deg, north_angle(project))
}

/// Terrain > North Pointer: draws the pointer centered on `center` with its
/// arrow toward `toward`, sized by the distance between the two, and makes that
/// direction true north. An earlier pointer is replaced. One undo step.
pub fn place_north_pointer(cx: &mut EditorContext, center: Point, toward: Point) -> bool {
    if center.dist(toward) < 1.0 {
        cx.status = "Click away from the center to point north".into();
        return false;
    }
    let radius = center
        .dist(toward)
        .clamp(MIN_POINTER_RADIUS, MAX_POINTER_RADIUS);
    let angle = (north_angle_toward(center, toward) * 10.0).round() / 10.0;
    let floor = cx.floor;
    cx.begin_change("North Pointer");
    ensure_site_plan_layer(&mut cx.project);
    let mut rec = load_terrain(&cx.project).unwrap_or_default();
    if let Some(old) = rec.terrain.north_pointer.take() {
        for id in old.ids {
            for f in 0..cx.project.floors.len() {
                cx.project.remove_cad(f, id);
            }
        }
    }
    let ids = north_pointer_items(center, radius, angle)
        .into_iter()
        .map(|item| cx.project.add_cad(floor, SITE_PLAN_LAYER, item))
        .collect();
    rec.terrain.north_angle = angle;
    rec.terrain.north_pointer = Some(SiteMark {
        center,
        size: radius,
        ids,
    });
    save_terrain(&mut cx.project, &rec);
    cx.mark_dirty();
    true
}

/// Terrain > Scale Bar: draws a bar from `start` to `end`, rounded to whole
/// feet, in four parts (fewer when short). One undo step.
pub fn place_scale_bar(cx: &mut EditorContext, start: Point, end: Point) -> bool {
    let feet = (start.dist(end) / 12.0).round();
    if feet < 1.0 {
        cx.status = "The scale bar needs at least a foot".into();
        return false;
    }
    let angle = end.sub(start).angle();
    let divisions = if feet >= 8.0 { 4 } else { feet as usize };
    let floor = cx.floor;
    cx.begin_change("Scale Bar");
    ensure_site_plan_layer(&mut cx.project);
    for item in scale_bar_items(start, angle, feet * 12.0, divisions) {
        cx.project.add_cad(floor, SITE_PLAN_LAYER, item);
    }
    cx.mark_dirty();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::cad::CadItem;

    fn cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    fn site_items(cx: &EditorContext) -> Vec<&CadItem> {
        cx.project.floors[0]
            .cad
            .iter()
            .filter(|c| c.layer == SITE_PLAN_LAYER)
            .map(|c| &c.item)
            .collect()
    }

    #[test]
    fn the_north_pointer_sets_the_north_angle_the_sun_uses() {
        let mut cx = cx();
        assert_eq!(north_angle(&cx.project), 0.0);
        assert_eq!(plan_sun_azimuth(&cx.project, 180.0), 180.0);
        // North points to the right of the page (east on the plan).
        assert!(place_north_pointer(
            &mut cx,
            Point::new(100.0, 100.0),
            Point::new(160.0, 100.0)
        ));
        assert_eq!(north_angle(&cx.project), 90.0);
        // Sun due south now shines from the left of the page.
        assert_eq!(plan_sun_azimuth(&cx.project, 180.0), 270.0);
        assert_eq!(plan_sun_azimuth(&cx.project, 90.0), 180.0);
        assert!(cx.project.layers.get(SITE_PLAN_LAYER).is_some());
        let items = site_items(&cx);
        assert_eq!(items.len(), 4);
        assert!(items
            .iter()
            .any(|i| matches!(i, CadItem::Circle { radius, .. } if (*radius - 60.0).abs() < 1e-9)));

        // Placing again replaces the pointer: still four items, new angle.
        assert!(place_north_pointer(
            &mut cx,
            Point::new(100.0, 100.0),
            Point::new(100.0, 40.0)
        ));
        assert_eq!(site_items(&cx).len(), 4);
        assert_eq!(north_angle(&cx.project), 180.0);

        // One undo step puts the first pointer and angle back.
        assert_eq!(cx.undo().as_deref(), Some("North Pointer"));
        assert_eq!(north_angle(&cx.project), 90.0);
        assert_eq!(site_items(&cx).len(), 4);
        // A click on the center is refused.
        assert!(!place_north_pointer(
            &mut cx,
            Point::new(5.0, 5.0),
            Point::new(5.0, 5.0)
        ));
    }

    #[test]
    fn a_scale_bar_is_cad_on_the_site_plan_layer() {
        let mut cx = cx();
        assert!(place_scale_bar(
            &mut cx,
            Point::new(0.0, 0.0),
            Point::new(485.0, 0.0)
        ));
        let items = site_items(&cx);
        let labels: Vec<&str> = items
            .iter()
            .filter_map(|i| match i {
                CadItem::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        // 485" rounds to 40 feet, in four parts.
        assert_eq!(labels, ["0", "10'-0\"", "20'-0\"", "30'-0\"", "40'-0\""]);
        assert!(!place_scale_bar(
            &mut cx,
            Point::new(0.0, 0.0),
            Point::new(3.0, 0.0)
        ));
        assert_eq!(cx.undo().as_deref(), Some("Scale Bar"));
        assert!(site_items(&cx).is_empty());
    }
}
