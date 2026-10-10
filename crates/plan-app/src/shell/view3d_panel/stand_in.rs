//! Labels on the stand-in blocks of the 3D view: a placed symbol whose
//! catalog item is not known (the Chief importer's `chief-plan.<name>` ids, a
//! Chief object whose catalog is not installed, an id from another machine)
//! is drawn as a box (`symbol_meshes`); this paints its name over the block so
//! it reads as "something is here" rather than an anonymous slab.

use crate::tools::library::find_item;
use eframe::egui;
use plan_core::{PlacedSymbol, Project};
use plan_view3d::math;
use plan_view3d::Camera;

/// Text size of a block label, points.
const LABEL_PX: f32 = 12.0;

/// Is this symbol drawn as a stand-in block (no catalog item, no picture,
/// distribution record or 3D solid)?
pub fn is_stand_in(s: &PlacedSymbol) -> bool {
    s.image.is_none() && s.distribution.is_none() && !s.solid && find_item(&s.catalog_id).is_none()
}

/// The label and the scene point (top centre of the block) of every stand-in
/// block of the project.
pub fn labels(project: &Project) -> Vec<(String, [f32; 3])> {
    let mut out = Vec::new();
    for floor in &project.floors {
        for s in floor.symbols.iter().filter(|s| is_stand_in(s)) {
            let foot = s.footprint();
            let (cx, cy) = (
                foot.iter().map(|p| p.x).sum::<f64>() / 4.0,
                foot.iter().map(|p| p.y).sum::<f64>() / 4.0,
            );
            let top = floor.elevation + s.elevation + s.height.max(0.0);
            let name = if s.label.is_empty() {
                plan_library::standin::stand_in_label(&s.catalog_id)
            } else {
                s.label.clone()
            };
            out.push((name, [cx as f32, top as f32, -cy as f32]));
        }
    }
    out
}

/// The pixel of scene point `p` in the viewport `rect`; None behind the eye.
pub fn to_screen(cam: &Camera, rect: egui::Rect, p: [f32; 3]) -> Option<egui::Pos2> {
    let aspect = rect.width().max(1.0) / rect.height().max(1.0);
    let clip = math::transform_vec4(&cam.view_projection(aspect), [p[0], p[1], p[2], 1.0]);
    if !cam.mode.is_orthographic() && clip[3] <= 1e-6 {
        return None;
    }
    let ndc = math::transform_point(&cam.view_projection(aspect), p);
    if !(-1.2..=1.2).contains(&ndc[0]) || !(-1.2..=1.2).contains(&ndc[1]) {
        return None;
    }
    Some(egui::pos2(
        rect.min.x + (ndc[0] + 1.0) * 0.5 * rect.width(),
        rect.min.y + (1.0 - ndc[1]) * 0.5 * rect.height(),
    ))
}

/// Paints the labels of the stand-in blocks over the 3D view.
pub fn paint(painter: &egui::Painter, cam: &Camera, rect: egui::Rect, project: &Project) {
    for (name, at) in labels(project) {
        if let Some(pos) = to_screen(cam, rect, at) {
            let galley = painter.layout_no_wrap(
                name,
                egui::FontId::proportional(LABEL_PX),
                egui::Color32::from_gray(0x20),
            );
            let r = egui::Align2::CENTER_BOTTOM.anchor_size(pos, galley.size());
            painter.rect_filled(
                r.expand(2.0),
                2.0,
                egui::Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, 0xC8),
            );
            painter.galley(r.min, galley, egui::Color32::BLACK);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::geometry::Point;

    fn project_with(id: &str) -> Project {
        let mut p = Project::new("T");
        p.add_symbol(
            0,
            PlacedSymbol::new(id, Point::new(100.0, 50.0), 20.0, 20.0, 30.0),
        );
        p
    }

    #[test]
    fn an_unknown_catalog_item_gets_a_label_at_the_top_of_its_block() {
        let p = project_with("chief-plan.dining-chair");
        let l = labels(&p);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].0, "Dining Chair");
        // 30" tall on a floor at elevation 0; plan y runs into -z.
        assert!((l[0].1[1] - 30.0).abs() < 1e-3);
        assert!((l[0].1[0] - 100.0).abs() < 1e-3);
    }

    #[test]
    fn a_known_catalog_item_and_a_picture_get_no_stand_in_label() {
        let p = project_with("core.plumbing.toilet_elongated");
        assert!(labels(&p).is_empty());
    }

    #[test]
    fn the_symbol_label_wins_over_the_id() {
        let mut p = project_with("chief-plan.x");
        p.floors[0].symbols[0].label = "Armchair".into();
        assert_eq!(labels(&p)[0].0, "Armchair");
    }

    #[test]
    fn a_point_in_front_of_the_camera_has_a_pixel_and_one_behind_does_not() {
        let mut cam = Camera::default();
        cam.set_mode(plan_view3d::CameraMode::Orbit);
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let target = cam.target;
        assert!(to_screen(&cam, rect, target).is_some());
        let eye = cam.eye();
        let behind = math::add(eye, math::sub(eye, target));
        assert!(to_screen(&cam, rect, behind).is_none());
    }
}
