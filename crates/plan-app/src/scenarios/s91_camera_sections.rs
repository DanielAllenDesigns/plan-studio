//! Scenario 91 (round 16, brief 32): clipping, annotations and navigation
//! steps of section cameras (C-5, C-62, C-121, C-129, C-136, C-137, C-152).
//!
//! Draw a shell, cut a wall section across it, clip it to one bay, step the
//! cutting plane, add a note and a dimension, save and reopen the plan, send
//! the view to a layout and see the annotations there; change a camera's
//! incremental distance and angle and see the steps follow. Every edit of the
//! camera specification is one undo step.

use super::{draw_shell, Sim};
use crate::dialogs::camera::{camera_drawing, elevation_options, render_elevation_with};
use crate::shell::view3d_panel::nudge::Steps;
use crate::tools::camera::set_section_geometry;
use plan_core::camera::{DEFAULT_EYE_HEIGHT, DEFAULT_FOV_DEG};
use plan_core::camera_view::{AnnotKind, DrawSurface, ViewAnnotation};
use plan_core::geometry::Point;
use plan_core::{CameraKind, CameraObject, Id};

const W: f64 = 480.0;
const H: f64 = 360.0;

/// A cross section through the middle of the shell, 120 in wide.
fn section(sim: &mut Sim) -> Id {
    let mut cam = CameraObject::new(
        CameraKind::CrossSection { back_clip: None },
        Point::new(W / 2.0, 20.0),
        90.0,
        "Section A",
        0,
    );
    set_section_geometry(&mut cam, Point::new(W / 2.0, 20.0), 90.0, 120.0, None);
    sim.app.cx.project.add_camera(cam)
}

fn edit(sim: &mut Sim, id: Id, label: &str, f: impl FnOnce(&mut CameraObject)) {
    sim.app.cx.begin_change(label);
    let c = sim
        .app
        .cx
        .project
        .cameras
        .iter_mut()
        .find(|c| c.id == id)
        .expect("camera");
    f(c);
}

fn camera(sim: &Sim, id: Id) -> &CameraObject {
    sim.app.cx.project.camera(id).expect("camera")
}

/// Width of the model's own lines (annotations such as the grade line are
/// laid over the clipped drawing and are not counted).
fn width(sim: &Sim, id: Id) -> f64 {
    let c = camera(sim, id);
    let mut opts = elevation_options(c);
    opts.raster_px = 160;
    let d = render_elevation_with(&sim.app.cx.project, c, &opts);
    let xs = d
        .lines
        .iter()
        .filter(|l| l.kind != plan_elevation::EdgeKind::Annotation)
        .flat_map(|l| [l.a.x, l.b.x]);
    let (lo, hi) = xs.fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
    if hi < lo {
        0.0
    } else {
        hi - lo
    }
}

fn note(id: Id, at: [f64; 2], text: &str) -> ViewAnnotation {
    ViewAnnotation {
        id,
        kind: AnnotKind::Note {
            at,
            text: text.into(),
            note_type: "General".into(),
            size: 6.0,
        },
        surface: DrawSurface::drawing(),
        layer: "Notes".into(),
        weight: None,
    }
}

fn dimension(id: Id) -> ViewAnnotation {
    ViewAnnotation {
        id,
        kind: AnnotKind::Dimension {
            a: [-40.0, 20.0],
            b: [40.0, 20.0],
            offset: [0.0, 12.0],
            a_cut: None,
            b_cut: None,
            text: None,
        },
        surface: DrawSurface::drawing(),
        layer: "Dimensions".into(),
        weight: None,
    }
}

#[test]
fn new_cameras_start_at_60_inches_and_55_degrees() {
    let c = CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "Den", 0);
    assert_eq!(c.eye_height, 60.0);
    assert_eq!(c.fov_deg, 55.0);
    assert_eq!((DEFAULT_EYE_HEIGHT, DEFAULT_FOV_DEG), (60.0, 55.0));
    assert!(c.view.shadows, "Full Camera views start with shadows on");
}

#[test]
fn a_section_clipped_stepped_and_annotated_is_saved_sent_and_undone() {
    let mut sim = Sim::new();
    sim.app.cx.defaults.grid.snap = 0.0;
    sim.app.cx.defaults.grid.angle_snap_deg = 0.0;
    draw_shell(&mut sim, W, H);
    let id = section(&mut sim);
    // Clip Sides on (the default of a section): one bay, 120 in wide.
    assert!(camera(&sim, id).view.clip.clip_sides);
    let clipped = width(&sim, id);
    assert!(clipped > 1.0 && clipped <= 120.0 + 1e-6, "{clipped}");
    // Clip Sides off draws the whole width of the shell.
    edit(&mut sim, id, "Clip Sides", |c| {
        c.view.clip.clip_sides = false
    });
    assert!(width(&sim, id) > 400.0, "the whole shell is drawn");
    sim.undo();
    assert!(camera(&sim, id).view.clip.clip_sides, "one undo step");
    assert!(width(&sim, id) <= 120.0 + 1e-6);
    // Clip Elevation keeps the drawing between two heights.
    edit(&mut sim, id, "Clip Elevation", |c| {
        c.view.clip.clip_elevation = true;
        c.view.clip.bottom = 0.0;
        c.view.clip.top = 48.0;
    });
    {
        let c = camera(&sim, id);
        let d = render_elevation_with(&sim.app.cx.project, c, &elevation_options(c));
        assert!(d.bounds.1.y <= 48.0 + 1e-6, "top {}", d.bounds.1.y);
    }
    // Poche off drops the cut fill.
    edit(&mut sim, id, "Poche", |c| {
        c.view.clip.clip_elevation = false;
        c.view.clip.poche = false;
    });
    {
        let c = camera(&sim, id);
        let d = render_elevation_with(&sim.app.cx.project, c, &elevation_options(c));
        assert_eq!(d.cut_regions().count(), 0);
    }
    // A stepped cutting plane: break the line and push the right piece back.
    edit(&mut sim, id, "Add Break", |c| {
        c.view.clip.poche = true;
        c.view.clip.plane.add_break(0.0).unwrap();
        c.view.clip.plane.set_offset(1, 24.0);
    });
    assert!(width(&sim, id) > 1.0);
    // A note and a dimension are drawn on the view.
    let before = {
        let c = camera(&sim, id);
        render_elevation_with(&sim.app.cx.project, c, &elevation_options(c))
    };
    edit(&mut sim, id, "Add Annotations", |c| {
        c.view
            .annotations
            .push(note(1, [-30.0, 90.0], "FIELD VERIFY"));
        c.view.annotations.push(dimension(2));
    });
    let after = {
        let c = camera(&sim, id);
        render_elevation_with(&sim.app.cx.project, c, &elevation_options(c))
    };
    assert!(after.lines.len() > before.lines.len());
    assert!(after.texts.iter().any(|(_, t)| t == "FIELD VERIFY"));
    // Save, close and reopen the plan: the clip and the annotations return.
    let json = sim.app.cx.project.to_json().unwrap();
    let back = plan_core::Project::from_json(&json).unwrap();
    let c = back.camera(id).unwrap();
    assert_eq!(c.view.annotations.len(), 2);
    assert!(c.view.clip.plane.is_stepped());
    assert_eq!(c.view.clip.plane.offsets, vec![0.0, 24.0]);
    // Send the view to a layout: the annotations are in the box's drawing.
    let drawing = camera_drawing(&back, id).expect("a drawing");
    assert!(drawing.texts.iter().any(|(_, t)| t == "FIELD VERIFY"));
    let mut layout = plan_layout::Layout::new("L", plan_docs::SheetSize::ArchC);
    crate::dialogs::camera::send_camera_to_layout(&mut layout, &back, id, 1).expect("a box");
    // Undo takes the annotations off in one step.
    sim.undo();
    assert!(camera(&sim, id).view.annotations.is_empty());
    sim.redo();
    assert_eq!(camera(&sim, id).view.annotations.len(), 2);
}

#[test]
fn a_camera_keeps_its_own_incremental_steps() {
    let mut sim = Sim::new();
    let id = sim.app.cx.project.add_camera(CameraObject::new(
        CameraKind::FullCamera,
        Point::ZERO,
        0.0,
        "Den",
        0,
    ));
    let s = Steps::of(&camera(&sim, id).view);
    assert_eq!(s, Steps::default(), "24 in and 15 degrees to begin with");
    edit(&mut sim, id, "Navigation", |c| {
        c.view.move_step = 6.0;
        c.view.rotate_step = 45.0;
    });
    let s = Steps::of(&camera(&sim, id).view);
    assert_eq!(s.move_in, 6.0);
    assert!((s.rotate - 45f32.to_radians()).abs() < 1e-6);
    // A camera of another view is not touched.
    let other = sim.app.cx.project.add_camera(CameraObject::new(
        CameraKind::FullCamera,
        Point::ZERO,
        0.0,
        "Kitchen",
        0,
    ));
    assert_eq!(Steps::of(&camera(&sim, other).view), Steps::default());
    // The settings survive a save, and one undo takes them back.
    let back = plan_core::Project::from_json(&sim.app.cx.project.to_json().unwrap()).unwrap();
    assert_eq!(back.camera(id).unwrap().view.move_step, 6.0);
    sim.undo();
    assert_eq!(camera(&sim, id).view.move_step, 24.0);
}

// ----- Cross Section Lines, Point Markers, Depth Cue, Below Grade, Plan Display -----

/// A cross section whose cut line runs north to south at x = W / 2, so the
/// south and north walls are cut (the view looks east).
fn through_walls(sim: &mut Sim) -> Id {
    let mut cam = CameraObject::new(
        CameraKind::CrossSection { back_clip: None },
        Point::new(W / 2.0, H / 2.0),
        0.0,
        "Section B",
        0,
    );
    set_section_geometry(&mut cam, Point::new(W / 2.0, H / 2.0), 0.0, H + 60.0, None);
    sim.app.cx.project.add_camera(cam)
}

fn lines_of(sim: &Sim, id: Id) -> Vec<plan_elevation::CutLine> {
    crate::dialogs::camera::cross_section_lines_of(&sim.app.cx.project, camera(sim, id))
}

#[test]
fn cut_walls_get_cross_section_lines_and_a_dimension_to_one_stays_on_it() {
    let mut sim = Sim::new();
    sim.app.cx.defaults.grid.snap = 0.0;
    sim.app.cx.defaults.grid.angle_snap_deg = 0.0;
    draw_shell(&mut sim, W, H);
    let id = through_walls(&mut sim);
    let lines = lines_of(&sim, id);
    assert!(
        lines.len() >= 8,
        "four lines for each of the two cut walls: {}",
        lines.len()
    );
    let face = lines
        .iter()
        .find(|l| l.edge == 0)
        .copied()
        .expect("a face line");
    // A dimension from that face to a point 40 in away.
    edit(&mut sim, id, "Add Dimension", |c| {
        c.view.annotations.push(ViewAnnotation {
            id: 1,
            kind: AnnotKind::Dimension {
                a: [face.position() + 0.5, 30.0],
                b: [face.position() + 40.0, 30.0],
                offset: [0.0, 10.0],
                a_cut: None,
                b_cut: None,
                text: None,
            },
            surface: DrawSurface::drawing(),
            layer: "Dimensions".into(),
            weight: None,
        });
    });
    let n = {
        let lines = lines_of(&sim, id);
        let c = sim
            .app
            .cx
            .project
            .cameras
            .iter_mut()
            .find(|c| c.id == id)
            .unwrap();
        crate::dialogs::camera::attach_cut_markers(c, &lines)
    };
    assert_eq!(n, 1, "only the end that meets the line is attached");
    let c = camera(&sim, id);
    assert_eq!(
        c.view.annotations.len(),
        2,
        "the dimension and a Point Marker"
    );
    assert!(c
        .view
        .annotations
        .iter()
        .any(|a| matches!(a.kind, AnnotKind::PointMarker { .. })));
    // Thicken the wall: its face moves and the dimension follows it.
    let wall = face.object;
    let before = lines_of(&sim, id);
    let was = plan_elevation::find_cut(&before, wall, 0)
        .unwrap()
        .position();
    sim.app.cx.begin_change("Thicken");
    for f in &mut sim.app.cx.project.floors {
        for w in &mut f.walls {
            if w.id == wall {
                w.thickness += 8.0;
            }
        }
    }
    let after = lines_of(&sim, id);
    let now = plan_elevation::find_cut(&after, wall, 0)
        .unwrap()
        .position();
    assert!((now - was).abs() > 1.0, "the face moved: {was} -> {now}");
    let c = camera(&sim, id);
    let drawing = render_elevation_with(&sim.app.cx.project, c, &elevation_options(c));
    let moved = plan_core::units::fmt_ft_in_frac((face.position() + 40.0 - now).abs(), 8);
    assert!(
        drawing.texts.iter().any(|(_, t)| *t == moved),
        "the dimension now reads {moved}: {:?}",
        drawing.texts
    );
    // Cross Section Lines are in the view's coloured lines, not in the layout.
    assert!(!drawing.styled.is_empty());
}

#[test]
fn depth_cue_fades_the_far_lines_and_below_grade_restyles_the_low_ones() {
    let mut sim = Sim::new();
    sim.app.cx.defaults.grid.snap = 0.0;
    sim.app.cx.defaults.grid.angle_snap_deg = 0.0;
    draw_shell(&mut sim, W, H);
    let id = through_walls(&mut sim);
    // A free-standing partition well behind the cutting plane.
    sim.app.cx.project.add_wall(
        0,
        Point::new(400.0, 100.0),
        Point::new(400.0, 260.0),
        6.0,
        96.0,
        plan_core::WallKind::Interior,
    );
    // Heavy lines count 2, Medium 1, Light 0: the sum falls when lines fade.
    let heavy = |sim: &Sim| {
        use plan_elevation::LineWeight::{Heavy, Light, Medium};
        let c = camera(sim, id);
        let mut o = elevation_options(c);
        o.raster_px = 400;
        render_elevation_with(&sim.app.cx.project, c, &o)
            .lines
            .iter()
            .filter(|l| l.kind != plan_elevation::EdgeKind::Annotation)
            .map(|l| match l.weight {
                Heavy => 2,
                Medium => 1,
                Light => 0,
            })
            .sum::<usize>()
    };
    let plain = heavy(&sim);
    edit(&mut sim, id, "Depth Cue", |c| {
        c.view.depth_cue.on = true;
        c.view.depth_cue.set_keep_in_sync(true);
        c.view.depth_cue.set_start(60.0);
        c.view.depth_cue.opacity = 1.0;
    });
    assert!(
        heavy(&sim) < plain,
        "the far lines fade: {plain} -> {}",
        heavy(&sim)
    );
    // Below Grade: everything under 48 in is dashed and red.
    edit(&mut sim, id, "Below Grade", |c| {
        c.view.depth_cue.on = false;
        let b = &mut c.view.below_grade;
        b.override_color = true;
        b.color = [200, 0, 0];
        b.override_style = true;
        b.limit = plan_core::camera_view::BelowGradeLimit::Absolute(48.0);
    });
    let c = camera(&sim, id);
    let d = render_elevation_with(&sim.app.cx.project, c, &elevation_options(c));
    assert!(d.styled.iter().any(|s| s.color == [200, 0, 0]
        && s.dashed
        && s.a.y <= 48.0 + 1e-6
        && s.b.y <= 48.0 + 1e-6));
    assert!(d
        .lines
        .iter()
        .any(|l| l.kind == plan_elevation::EdgeKind::Hidden && l.a.y.max(l.b.y) <= 48.0 + 1e-6));
    sim.undo();
    assert!(!camera(&sim, id).view.below_grade.override_color);
}

#[test]
fn a_stepped_plane_has_handles_in_the_plan_that_move_and_step_it() {
    use crate::tools::camera::{
        add_section_break, apply_handle, handles_of, plane_point, section_width, CamHandle,
    };
    let mut sim = Sim::new();
    let id = through_walls(&mut sim);
    let mut c = camera(&sim, id).clone();
    assert!(!handles_of(&c)
        .iter()
        .any(|(h, _)| matches!(h, CamHandle::Break(_))));
    // Add Break at a plan point on the line.
    let at = plane_point(&c, 30.0, 0.0).unwrap();
    assert!(add_section_break(&mut c, at));
    let hs = handles_of(&c);
    assert_eq!(
        hs.iter()
            .filter(|(h, _)| matches!(h, CamHandle::Break(_)))
            .count(),
        1
    );
    assert_eq!(
        hs.iter()
            .filter(|(h, _)| matches!(h, CamHandle::Step(_)))
            .count(),
        2
    );
    // Drag the second piece's handle square to the line: a step of 24 in.
    let to = plane_point(&c, 100.0, 24.0).unwrap();
    apply_handle(&mut c, CamHandle::Step(1), to, false);
    assert!((c.view.clip.plane.offsets[1] - 24.0).abs() < 1e-6);
    // Drag the break along the line.
    let to = plane_point(&c, -50.0, 0.0).unwrap();
    apply_handle(&mut c, CamHandle::Break(0), to, false);
    assert!((c.view.clip.plane.breaks[0] + 50.0).abs() < 1e-6);
    // Too near the end of the line: no break.
    let far = plane_point(&c, section_width(&c), 0.0).unwrap();
    assert!(!add_section_break(&mut c, far));
}

#[test]
fn plan_display_places_labels_and_shows_the_camera_on_every_floor() {
    use crate::tools::camera::{callout_style, callouts_on};
    use plan_core::camera_view::CalloutPlacement;
    let mut sim = Sim::new();
    let id = through_walls(&mut sim);
    let style = callout_style();
    assert_eq!(callouts_on(&sim.app.cx.project, 0, &style).len(), 1);
    edit(&mut sim, id, "Plan Display", |c| {
        c.view.plan.placement = CalloutPlacement::BothSides;
        c.view.plan.callout_label = "A".into();
        c.view.plan.text_below_auto = false;
        c.view.plan.text_below = "SEE A-3".into();
        c.view.plan.callout_size = Some(30.0);
    });
    let cos = callouts_on(&sim.app.cx.project, 0, &style);
    assert_eq!(cos.len(), 2, "a callout at each end");
    assert!(cos.iter().all(|c| c.number == "A" && c.radius == 15.0));
    assert!(cos.iter().all(|c| c.below.as_deref() == Some("SEE A-3")));
    assert!(
        cos.iter().all(|c| c.arrows.len() == 1),
        "an arrow at each end"
    );
    // Display on All Floors puts the symbol on the other floors too.
    assert!(callouts_on(&sim.app.cx.project, 1, &style).is_empty());
    edit(&mut sim, id, "Display on All Floors", |c| {
        c.view.plan.all_floors = true
    });
    assert_eq!(sim.app.cx.project.cameras_on(1).count(), 1);
    sim.undo();
    assert_eq!(sim.app.cx.project.cameras_on(1).count(), 0);
}

#[test]
fn auto_elevations_can_be_made_one_side_at_a_time() {
    use crate::tools::camera::{add_auto_elevations_for, AutoSide};
    let mut sim = Sim::new();
    sim.app.cx.defaults.grid.snap = 0.0;
    sim.app.cx.defaults.grid.angle_snap_deg = 0.0;
    draw_shell(&mut sim, W, H);
    let p = &mut sim.app.cx.project;
    let front = add_auto_elevations_for(p, 0, false, Some(AutoSide::Front));
    assert_eq!(front.len(), 1);
    assert_eq!(p.camera(front[0]).unwrap().name, "South Elevation");
    assert_eq!(
        p.camera(front[0]).unwrap().direction_deg,
        90.0,
        "looks north at the front"
    );
    let right = add_auto_elevations_for(p, 0, false, Some(AutoSide::Right));
    assert_eq!(p.camera(right[0]).unwrap().name, "East Elevation");
    assert_eq!(p.cameras.len(), 2);
    // Again: the side is updated, not duplicated.
    let again = add_auto_elevations_for(p, 0, false, Some(AutoSide::Front));
    assert_eq!(again, front);
    assert_eq!(p.cameras.len(), 2);
    // All Elevations fills in the other two.
    assert_eq!(add_auto_elevations_for(p, 0, false, None).len(), 4);
    assert_eq!(p.cameras.len(), 4);
    // The section layers exist for the views to draw.
    assert!(p
        .layers
        .get(plan_core::camera_view::clip::CROSS_SECTION_LAYER)
        .is_some_and(|l| l.locked));
    assert!(p
        .layers
        .get(plan_core::camera_view::clip::CLIP_LINES_LAYER)
        .is_some_and(|l| !l.display));
}

#[test]
fn a_saved_overview_has_a_plan_symbol_that_places_its_view() {
    use plan_core::camera_view::ViewPose;
    let mut c = CameraObject::new(
        CameraKind::PerspectiveOverview,
        Point::new(100.0, 50.0),
        90.0,
        "Overview",
        0,
    );
    c.clip_distance = Some(200.0);
    c.view.pose = Some(ViewPose {
        eye: [100.0, 300.0, -50.0],
        target: [100.0, 40.0, -250.0],
        symbol: true,
    });
    let p = c.overview_pose().unwrap();
    // The symbol looks along +Y for 200 in: the target is 200 in north of it.
    assert!((p.eye[0] - 100.0).abs() < 1e-9 && (p.eye[2] + 50.0).abs() < 1e-9);
    assert!((p.target[2] + 250.0).abs() < 1e-9);
    assert_eq!(
        (p.eye[1], p.target[1]),
        (300.0, 40.0),
        "the heights are kept"
    );
    // Moving the symbol moves the view; a copy has its own place.
    c.position = Point::new(130.0, 80.0);
    let moved = c.overview_pose().unwrap();
    assert!((moved.eye[0] - 130.0).abs() < 1e-9 && (moved.eye[2] + 80.0).abs() < 1e-9);
    assert!((moved.target[2] + 280.0).abs() < 1e-9);
    // A pose saved before the symbol was editable is used as it is.
    c.view.pose = Some(ViewPose {
        symbol: false,
        ..c.view.pose.unwrap()
    });
    assert_eq!(c.overview_pose(), c.view.pose);
}

#[test]
fn add_break_and_make_parallel_work_from_the_edit_buttons_of_a_selected_section() {
    use crate::editor::{camera_edit, EditActionKind, ObjectRef};
    let labels = |sim: &Sim| -> Vec<&'static str> {
        sim.app
            .cx
            .selection_edit_actions()
            .into_iter()
            .filter_map(|a| match a.kind {
                EditActionKind::Custom { label, .. } => Some(label),
                _ => None,
            })
            .collect()
    };
    let mut sim = Sim::new();
    let id = section(&mut sim);
    sim.app.cx.selection.items = vec![ObjectRef::Camera(id)];
    let l = labels(&sim);
    assert!(l.contains(&"Add Break"), "{l:?}");
    assert!(!l.contains(&"Make Parallel"), "no break handle yet: {l:?}");
    sim.app.cx.run_custom(camera_edit::ADD_BREAK);
    assert_eq!(camera(&sim, id).view.clip.plane.breaks.len(), 1);
    assert_eq!(sim.app.cx.undo_label(), Some("Add Break"));
    // The new break is the selected handle: Make Parallel / Perpendicular work.
    let l = labels(&sim);
    assert!(l.contains(&"Make Parallel") && l.contains(&"Make Perpendicular"));
    sim.app.cx.run_custom(camera_edit::MAKE_PERPENDICULAR);
    let off = camera(&sim, id).view.clip.plane.offsets.clone();
    assert!((off[1] - off[0]).abs() > 1e-6, "a step was made: {off:?}");
    sim.app.cx.run_custom(camera_edit::MAKE_PARALLEL);
    let off = camera(&sim, id).view.clip.plane.offsets.clone();
    assert!((off[1] - off[0]).abs() < 1e-9, "flattened again: {off:?}");
    sim.undo();
    assert!(
        (camera(&sim, id).view.clip.plane.offsets[1] - camera(&sim, id).view.clip.plane.offsets[0])
            .abs()
            > 1e-6
    );
    // A second section is not selected: no break buttons for other objects.
    sim.app.cx.selection.items.clear();
    assert!(!labels(&sim).contains(&"Add Break"));
}
