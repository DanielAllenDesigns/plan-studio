//! Scenario 17: dimensions. The Locate Objects settings (surfaces, main
//! layer, centers), associative strings that follow a wall and an opening,
//! Auto Exterior with a window on each side of a 40 x 30 shell, Auto Interior,
//! Auto NKBA on a cabinet run and printed-size text at two sheet scales
//! (DIM-3, DIM-4, DIM-7, DIM-24, DIM-26, DIM-27, DIM-29, DIM-35).

use super::{draw_shell, Sim};
use crate::editor::render::DimLook;
use crate::editor::ObjectRef;
use crate::tools::dimension::DimMode;
use crate::tools::ToolId;
use eframe::egui;
use plan_cabinets::CabinetKind;
use plan_core::dim_assoc::AnchorTarget;
use plan_core::dimension::{AutoGroup, WallLocate};
use plan_core::geometry::Point;
use plan_core::{Dimension, DimensionKind, OpeningLocate, WallKind};
use plan_docs::Scale;

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim
}

fn dims(sim: &Sim) -> Vec<Dimension> {
    sim.app.cx.floor().dimensions.clone()
}

/// A 40 x 30 shell drawn with exact corners (the walls close in a loop).
fn exact_house() -> Sim {
    let mut sim = Sim::new();
    sim.tool(ToolId::Wall {
        kind: WallKind::Exterior,
    });
    sim.drag((0.0, 0.0), (W, 0.0));
    sim.drag((W, 0.0), (W, H));
    sim.drag((W, H), (0.0, H));
    sim.drag((0.0, H), (0.0, 0.0));
    sim.app.cx.refresh();
    assert_eq!(sim.floor_walls(), 4);
    sim
}

/// Places the manual dimension across the shell between the faces nearest
/// the two clicks, with the line 40" to the side.
fn manual_across(sim: &mut Sim) -> Dimension {
    sim.tool(ToolId::DimensionVariant(DimMode::Manual));
    sim.click(200.0, 4.0);
    sim.click(200.0, H - 3.0);
    let r = sim.click(260.0, 180.0);
    assert_eq!(
        r.commit.as_deref(),
        Some("Manual Dimension"),
        "{}",
        sim.app.cx.status
    );
    sim.tool(ToolId::Select);
    dims(sim).pop().unwrap()
}

#[test]
fn locate_objects_walls_decides_which_surface_a_manual_dimension_measures() {
    let t = {
        let sim = house();
        sim.app.cx.wall_thickness(WallKind::Exterior)
    };
    let mut lens = Vec::new();
    for mode in [
        WallLocate::Surfaces,
        WallLocate::MainLayer,
        WallLocate::Centers,
    ] {
        let mut sim = house();
        sim.app.cx.defaults.dimensions.locate_walls = mode;
        let d = manual_across(&mut sim);
        lens.push((mode, d.length()));
    }
    let get = |m: WallLocate| lens.iter().find(|(k, _)| *k == m).unwrap().1;
    let (surf, main, cent) = (
        get(WallLocate::Surfaces),
        get(WallLocate::MainLayer),
        get(WallLocate::Centers),
    );
    // Both clicks are on the room side of the wall: surfaces measure the
    // clear span, centers the centerline distance, the main layer between.
    assert!((cent - (H + 1.0)).abs() < 1.5, "centers {cent}");
    assert!((surf - (H + 1.0 - t)).abs() < 1.5, "surfaces {surf}");
    assert!(surf < main && main < cent, "{surf} < {main} < {cent}");
}

#[test]
fn locate_objects_openings_measure_to_the_sides_or_the_center_of_a_window() {
    let mut out = Vec::new();
    for mode in [OpeningLocate::Sides, OpeningLocate::Centers] {
        let mut sim = house();
        sim.tool(ToolId::Window);
        sim.click(240.0, 0.0);
        sim.app.cx.defaults.dimensions.set_opening_locate(mode);
        sim.tool(ToolId::DimensionVariant(DimMode::Manual));
        // From the wall corner to the window.
        sim.click(2.0, -4.0);
        sim.click(236.0, -4.0);
        sim.click(120.0, -40.0);
        let d = dims(&sim).pop().expect("a dimension");
        out.push((mode, d.length(), d.anchors));
    }
    let (sides, centers) = (&out[0], &out[1]);
    // The window is 32" wide: the center is 16" farther than its near side.
    assert!(
        (centers.1 - sides.1 - 16.0).abs() < 2.0,
        "sides {} centers {}",
        sides.1,
        centers.1
    );
    // The measured point is tied to the window either way.
    for o in &out {
        assert!(
            o.2.iter()
                .flatten()
                .any(|a| a.target == AnchorTarget::Opening),
            "{:?} not tied to the opening: {:?}",
            o.0,
            o.2
        );
    }
}

#[test]
fn a_manual_dimension_is_tied_to_its_walls_and_follows_when_one_moves() {
    let mut sim = house();
    let d = manual_across(&mut sim);
    let len = d.length();
    assert!(d.anchors.iter().all(Option::is_some), "{:?}", d.anchors);
    assert!(d
        .anchors
        .iter()
        .flatten()
        .all(|a| a.target == AnchorTarget::Wall));
    let offset = d.offset;
    // Drag the north wall 48" further out with the Select tool.
    sim.tool(ToolId::Select);
    sim.click(400.0, H);
    let ObjectRef::Wall(north) = sim.app.cx.selection.single().expect("north wall") else {
        panic!("a wall")
    };
    let nw = sim.app.cx.floor().wall(north).unwrap().clone();
    assert!(nw.start.y > H - 5.0 && nw.end.y > H - 5.0);
    sim.drag((400.0, H), (400.0, H + 48.0));
    let moved = sim.app.cx.floor().wall(north).unwrap().clone();
    assert!((moved.start.y - nw.start.y - 48.0).abs() < 1.0, "{moved:?}");
    // The dimension stretched with it (DIM-29), its line stayed put (DIM-35).
    let after = dims(&sim).pop().unwrap();
    assert!(
        (after.length() - len - 48.0).abs() < 1.5,
        "{} -> {}",
        len,
        after.length()
    );
    // A hand-drawn west wall is 2" off plumb, so the anchor slides a hair.
    assert!(
        (after.offset - offset).abs() < 0.5,
        "{} -> {}",
        offset,
        after.offset
    );
    assert_eq!(after.id, d.id);
    // Undo brings both back together.
    sim.undo();
    let back = dims(&sim).pop().unwrap();
    assert!(
        (back.length() - len).abs() < 1.0,
        "{} vs {len}",
        back.length()
    );
}

#[test]
fn a_dimension_to_a_window_follows_the_window_when_it_slides() {
    let mut sim = house();
    sim.tool(ToolId::Window);
    sim.click(240.0, 0.0);
    let win = sim.app.cx.floor().openings[0].clone();
    sim.tool(ToolId::DimensionVariant(DimMode::Manual));
    sim.click(2.0, -4.0);
    sim.click(win.start_offset() + 6.0, -4.0);
    sim.click(120.0, -40.0);
    sim.tool(ToolId::Select);
    let d = dims(&sim).pop().expect("a dimension");
    assert!(d
        .anchors
        .iter()
        .flatten()
        .any(|a| a.target == AnchorTarget::Opening && a.wall == win.id));
    // Slide the window 60" along the wall by dragging its body.
    sim.app.cx.selection.set(ObjectRef::Opening(win.id));
    sim.drag((240.0, 0.0), (300.0, 0.0));
    let moved = sim.app.cx.floor().openings[0].clone();
    assert!(
        (moved.center_offset - win.center_offset - 60.0).abs() < 2.0,
        "{moved:?}"
    );
    let after = dims(&sim).pop().unwrap();
    assert!(
        (after.length() - d.length() - 60.0).abs() < 2.5,
        "{} -> {}",
        d.length(),
        after.length()
    );
}

/// The side of the shell a dimension line is on.
fn side_of(d: &Dimension) -> &'static str {
    let (a, b) = d.line_points();
    let c = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    if (a.y - b.y).abs() < 1.0 {
        if c.y < 0.0 {
            "south"
        } else if c.y > H {
            "north"
        } else {
            "inside"
        }
    } else if c.x < 0.0 {
        "west"
    } else if c.x > W {
        "east"
    } else {
        "inside"
    }
}

fn three_strings_per_side(sim: &mut Sim) -> Vec<Dimension> {
    sim.tool(ToolId::Window);
    for (x, y) in [(240.0, 0.0), (240.0, H), (0.0, 180.0), (W, 180.0)] {
        sim.click(x, y);
    }
    assert_eq!(sim.app.cx.floor().openings.len(), 4);
    sim.tool(ToolId::DimensionVariant(DimMode::AutoExterior));
    let r = sim.click(240.0, 180.0);
    assert!(r.commit.is_some(), "{}", sim.app.cx.status);
    dims(sim)
}

#[test]
fn auto_exterior_puts_three_strings_on_each_side_of_a_shell_with_a_window_per_side() {
    let mut sim = exact_house();
    let all = three_strings_per_side(&mut sim);
    assert!(all.iter().all(|d| d.kind == DimensionKind::AutoExterior));
    assert!(all.iter().all(|d| d.auto_group == AutoGroup::Exterior));
    for side in ["south", "north", "west", "east"] {
        let on: Vec<&Dimension> = all.iter().filter(|d| side_of(d) == side).collect();
        // Openings, wall to wall and overall, each farther from the wall.
        let mut lines: Vec<f64> = on
            .iter()
            .map(|d| {
                let (a, _) = d.line_points();
                if side == "south" || side == "north" {
                    a.y
                } else {
                    a.x
                }
            })
            .collect();
        lines.sort_by(f64::total_cmp);
        lines.dedup_by(|a, b| (*a - *b).abs() < 1.0);
        assert_eq!(lines.len(), 3, "{side}: {lines:?}");
    }
    assert!(all.iter().all(|d| side_of(d) != "inside"));
    // The overall strings span the shell, outside face to outside face.
    let t = sim.app.cx.wall_thickness(WallKind::Exterior);
    let longest = |side: &str| {
        all.iter()
            .filter(|d| side_of(d) == side)
            .map(Dimension::length)
            .fold(0.0, f64::max)
    };
    assert!(
        (longest("south") - (W + t)).abs() < 2.0,
        "{}",
        longest("south")
    );
    assert!(
        (longest("west") - (H + t)).abs() < 2.0,
        "{}",
        longest("west")
    );
    // A second run replaces the first; one undo step each.
    let n = all.len();
    sim.click(240.0, 180.0);
    assert_eq!(dims(&sim).len(), n, "re-running replaces the strings");
    sim.undo();
    assert_eq!(dims(&sim).len(), n);
    sim.undo();
    assert!(dims(&sim).is_empty());
}

#[test]
fn auto_exterior_on_a_hand_drawn_shell_keeps_every_string_outside_the_house() {
    // The same shell drawn with ends a few inches off: the west wall ends
    // up 2" off plumb.
    let mut sim = house();
    let all = three_strings_per_side(&mut sim);
    let inside: Vec<_> = all.iter().filter(|d| side_of(d) == "inside").collect();
    assert!(
        inside.is_empty(),
        "{} strings run through the house",
        inside.len()
    );
    // Three strings (openings, wall to wall, overall) are three lines at
    // different distances; a string is several segments (QA-08: the old
    // assertion counted segments, which are 6 on a side with a window).
    let mut lines: Vec<i64> = all
        .iter()
        .filter(|d| side_of(d) == "west")
        .map(|d| (d.line_points().0.x / 6.0).round() as i64)
        .collect();
    lines.sort_unstable();
    lines.dedup();
    assert_eq!(lines.len(), 3, "{lines:?}");
}

#[test]
fn auto_exterior_keeps_a_manual_dimension_and_follows_the_locate_setting() {
    let mut sim = house();
    let manual = manual_across(&mut sim);
    sim.tool(ToolId::DimensionVariant(DimMode::AutoExterior));
    sim.app.cx.defaults.dimensions.locate_walls = WallLocate::Centers;
    sim.click(240.0, 180.0);
    let all = dims(&sim);
    assert!(
        all.iter().any(|d| d.id == manual.id),
        "the manual one stays"
    );
    // Centers: the overall string runs centerline to centerline.
    let longest = all
        .iter()
        .filter(|d| d.kind == DimensionKind::AutoExterior && side_of(d) == "south")
        .map(Dimension::length)
        .fold(0.0, f64::max);
    assert!((longest - (W + 1.0)).abs() < 8.0, "{longest}");
}

#[test]
fn auto_interior_dimensions_measure_the_clear_spans_of_the_room() {
    let mut sim = house();
    sim.tool(ToolId::Wall {
        kind: WallKind::Interior,
    });
    sim.drag((240.0, 0.0), (240.0, H));
    sim.tool(ToolId::DimensionVariant(DimMode::AutoInterior));
    assert_eq!(sim.app.tools.active().name(), "Auto Interior Dimensions");
    let r = sim.click(120.0, 180.0);
    assert!(r.commit.is_some(), "{}", sim.app.cx.status);
    let all = dims(&sim);
    assert!(!all.is_empty());
    assert!(all.iter().all(|d| d.auto_group == AutoGroup::Interior));
    // The west room is about 240" wide between the faces.
    let t = sim.app.cx.wall_thickness(WallKind::Exterior);
    let clear_w = 240.0 - t / 2.0 - 3.0;
    assert!(
        all.iter().any(|d| (d.length() - clear_w).abs() < 8.0),
        "no string near {clear_w}: {:?}",
        all.iter().map(Dimension::length).collect::<Vec<_>>()
    );
    // They sit inside the shell.
    assert!(all.iter().all(|d| side_of(d) == "inside"));
    assert_eq!(sim.undo().as_deref(), Some("Auto Interior Dimensions"));
    assert!(dims(&sim).is_empty());
}

#[test]
fn auto_nkba_dimensions_a_base_cabinet_run_to_its_faces_and_overall() {
    let mut sim = house();
    for x in [60.0, 84.0, 108.0] {
        sim.tool(ToolId::CabinetVariant(CabinetKind::Base));
        sim.app.cx.selection.clear();
        sim.click(x, 10.0);
    }
    assert!(
        sim.app.cx.floor().symbols.len()
            + crate::editor::placed::load_cabinets(sim.app.cx.floor()).len()
            >= 3
    );
    sim.tool(ToolId::DimensionVariant(DimMode::AutoNkba));
    let r = sim.click(120.0, 100.0);
    assert_eq!(
        r.commit.as_deref(),
        Some("Auto NKBA Dimensions"),
        "{}",
        sim.app.cx.status
    );
    let all = dims(&sim);
    assert!(!all.is_empty());
    assert!(all.iter().all(|d| d.auto_group == AutoGroup::Nkba));
    // The overall string is the longest and covers all three cabinets.
    let cabs = crate::editor::placed::load_cabinets(sim.app.cx.floor());
    let run: f64 = cabs.iter().map(|c| c.width).sum();
    let overall = all.iter().map(Dimension::length).fold(0.0, f64::max);
    assert!(overall >= run - 1e-6, "overall {overall} vs run {run}");
    // Tied to the cabinets: moving one moves the string (checked by anchors).
    assert!(all
        .iter()
        .flat_map(|d| d.anchors.iter().flatten())
        .any(|a| a.target == AnchorTarget::Cabinet));
    // Running again replaces; undo takes the run away in one step.
    sim.click(120.0, 100.0);
    assert_eq!(dims(&sim).len(), all.len());
    sim.undo();
    sim.undo();
    assert!(dims(&sim).is_empty());
}

/// The tallest dimension number drawn on the plan canvas.
fn dim_text_px(sim: &mut Sim) -> f32 {
    sim.plan_shapes()
        .into_iter()
        .filter_map(|s| match s {
            egui::Shape::Text(t) if t.galley.text().contains('\'') => Some(t.galley.size().y),
            _ => None,
        })
        .fold(0.0, f32::max)
}

#[test]
fn printed_size_dimension_text_holds_its_size_on_paper_at_two_scales() {
    let mut sim = house();
    manual_across(&mut sim);
    let d = dims(&sim).pop().unwrap();
    sim.app.cx.sheet.scale = Scale::QuarterInch;
    let plain_quarter = DimLook::of(&sim.app.cx, &d).text_h;
    sim.app.cx.sheet.scale = Scale::EighthInch;
    let plain_eighth = DimLook::of(&sim.app.cx, &d).text_h;
    // Without Printed Size the plan inches stay and the sheet shrinks them.
    assert_eq!(plain_quarter, plain_eighth);

    sim.app.cx.defaults.dimensions.printed_size = true;
    sim.app.cx.sheet.scale = Scale::QuarterInch;
    let look_q = DimLook::of(&sim.app.cx, &d);
    sim.app.cx.sheet.scale = Scale::EighthInch;
    let look_e = DimLook::of(&sim.app.cx, &d);
    // Same height on paper: twice the plan inches at half the scale.
    assert!(
        (look_e.text_h / look_q.text_h - 2.0).abs() < 1e-9,
        "{look_q:?} {look_e:?}"
    );
    assert!((look_e.arrow / look_q.arrow - 2.0).abs() < 1e-9);
    // And it is what the canvas draws.
    sim.app.cx.sheet.scale = Scale::QuarterInch;
    let px_q = dim_text_px(&mut sim);
    sim.app.cx.sheet.scale = Scale::EighthInch;
    let px_e = dim_text_px(&mut sim);
    assert!(
        px_q > 0.0 && px_e > 0.0,
        "no dimension text drawn: {px_q} {px_e}"
    );
    assert!(
        px_e > px_q * 1.5,
        "text did not grow with the plan size: {px_q} -> {px_e}"
    );
}
