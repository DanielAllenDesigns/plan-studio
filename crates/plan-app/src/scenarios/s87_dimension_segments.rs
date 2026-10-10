//! Scenario 87 (round 16): the dimension leftovers of round 15.
//!
//! The Dimension Defaults' Layer panel names the layer new dimensions draw
//! on (DIM-57); Auto Refresh replaces the automatic strings when the model
//! changes and leaves an unchanged run alone (DIM-52); Auto Story Pole puts
//! an Elevation Marker on every mark (DIM-64); Join, Select String and Take
//! Out of String run from the Edit toolbar's command path.

use super::Sim;
use crate::editor::selection::layer_of;
use crate::editor::ObjectRef;
use crate::tools::dimension::{edit_actions, DimMode, CMD_JOIN, CMD_LEAVE, CMD_SELECT_STRING};
use crate::tools::ToolId;
use plan_core::callout::MarkerKind;
use plan_core::geometry::Point;
use plan_core::{Dimension, DimensionKind, Id, OpeningKind, WallKind};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn shell(sim: &mut Sim) -> [Id; 4] {
    let cx = &mut sim.app.cx;
    let c = [p(0.0, 0.0), p(480.0, 0.0), p(480.0, 360.0), p(0.0, 360.0)];
    let mut ids = [0; 4];
    for i in 0..4 {
        ids[i] = cx
            .project
            .add_wall(0, c[i], c[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior);
        cx.project
            .add_opening(0, ids[i], 120.0, OpeningKind::Window)
            .unwrap();
    }
    cx.refresh();
    ids
}

fn ids_of(sim: &Sim) -> Vec<Id> {
    sim.app.cx.floor().dimensions.iter().map(|d| d.id).collect()
}

#[test]
fn the_layer_panel_names_the_layers_new_dimensions_draw_on() {
    let mut sim = Sim::new();
    let st = &mut sim.app.cx.defaults.dimensions.setup;
    st.layer_manual = "Plot Dims".into();
    st.layer_automatic = "Auto Dims".into();
    sim.app.cx.begin_change("Dimension");
    let manual = sim.app.cx.project.add_dimension(
        0,
        Dimension::new(0, DimensionKind::Manual, p(0.0, 0.0), p(100.0, 0.0), 24.0),
    );
    let auto = sim.app.cx.project.add_dimension(
        0,
        Dimension::new(
            0,
            DimensionKind::AutoExterior,
            p(0.0, 0.0),
            p(100.0, 0.0),
            24.0,
        ),
    );
    let floor = sim.app.cx.floor();
    assert_eq!(
        layer_of(floor, ObjectRef::Dimension(manual)).as_deref(),
        Some("Plot Dims")
    );
    assert_eq!(
        layer_of(floor, ObjectRef::Dimension(auto)).as_deref(),
        Some("Auto Dims")
    );
    // A run of the automatic tool lands on the automatic layer too.
    let mut sim = Sim::new();
    sim.app.cx.defaults.dimensions.setup.layer_automatic = "Auto Dims".into();
    shell(&mut sim);
    sim.tool(ToolId::DimensionVariant(DimMode::AutoExterior));
    sim.click(-100.0, -100.0);
    let floor = sim.app.cx.floor();
    assert!(!floor.dimensions.is_empty());
    for d in &floor.dimensions {
        assert_eq!(
            layer_of(floor, ObjectRef::Dimension(d.id)).as_deref(),
            Some("Auto Dims")
        );
    }
}

#[test]
fn without_a_named_layer_dimensions_keep_chiefs_own_layers() {
    let mut sim = Sim::new();
    sim.app.cx.begin_change("Dimension");
    let m = sim.app.cx.project.add_dimension(
        0,
        Dimension::new(0, DimensionKind::Manual, p(0.0, 0.0), p(100.0, 0.0), 24.0),
    );
    assert_eq!(
        layer_of(sim.app.cx.floor(), ObjectRef::Dimension(m)).as_deref(),
        Some("Dimensions, Manual")
    );
    assert!(sim.app.cx.floor().dimensions[0].look.layer.is_none());
}

#[test]
fn auto_refresh_replaces_the_run_when_the_model_changes_and_is_idempotent() {
    let mut sim = Sim::new();
    sim.app.cx.px_per_in = 2.0;
    let walls = shell(&mut sim);
    sim.tool(ToolId::DimensionVariant(DimMode::AutoExterior));
    sim.click(-100.0, -100.0);
    let before = sim.app.cx.floor().dimensions.len();
    assert!(before > 0);

    // Off: a new window leaves the run as it was.
    sim.app.cx.begin_change("Add window");
    sim.app
        .cx
        .project
        .add_opening(0, walls[0], 300.0, OpeningKind::Window)
        .unwrap();
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    assert_eq!(sim.app.cx.floor().dimensions.len(), before);
    sim.undo();

    // On: the run is deleted and replaced, once.
    sim.app.cx.defaults.dimensions.setup.exterior_auto_refresh = true;
    let old = ids_of(&sim);
    sim.app.cx.begin_change("Add window");
    sim.app
        .cx
        .project
        .add_opening(0, walls[0], 300.0, OpeningKind::Window)
        .unwrap();
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    let new = ids_of(&sim);
    assert!(new.len() > before, "{} > {before}", new.len());
    assert!(new.iter().all(|i| !old.contains(i)), "all replaced");
    // Refreshing again with nothing changed keeps the very same strings.
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    assert_eq!(ids_of(&sim), new);
    // The edit and its refresh are one undo step.
    sim.undo();
    assert_eq!(sim.app.cx.floor().dimensions.len(), before);
}

#[test]
fn auto_refresh_does_nothing_when_no_run_was_made() {
    let mut sim = Sim::new();
    shell(&mut sim);
    sim.app.cx.defaults.dimensions.setup.exterior_auto_refresh = true;
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    assert!(sim.app.cx.floor().dimensions.is_empty());
}

#[test]
fn a_story_pole_puts_an_elevation_marker_on_every_mark() {
    let mut sim = Sim::new();
    sim.app.cx.px_per_in = 2.0;
    shell(&mut sim);
    sim.tool(ToolId::DimensionVariant(DimMode::AutoStoryPole));
    sim.click(-100.0, 0.0);
    let markers = sim.app.cx.floor().annots.markers.clone();
    assert!(markers.len() >= 2, "{}", markers.len());
    assert!(markers.iter().all(|m| m.kind == MarkerKind::Elevation));
    assert!(markers.iter().all(|m| (m.center.x + 100.0).abs() < 1e-6));
    // A new run on the same line replaces the markers.
    sim.click(-100.0, 0.0);
    assert_eq!(sim.app.cx.floor().annots.markers.len(), markers.len());
    // One undo step takes the whole run back.
    sim.undo();
    assert_eq!(sim.app.cx.floor().annots.markers.len(), markers.len());
    sim.undo();
    assert!(sim.app.cx.floor().annots.markers.is_empty());
}

#[test]
fn join_select_string_and_take_out_run_from_the_edit_toolbar() {
    let mut sim = Sim::new();
    sim.app.cx.begin_change("Dimensions");
    let mut ids = Vec::new();
    for i in 0..3 {
        let a = 100.0 * f64::from(i);
        ids.push(sim.app.cx.project.add_dimension(
            0,
            Dimension::new(0, DimensionKind::Manual, p(a, 0.0), p(a + 100.0, 0.0), 24.0),
        ));
    }
    sim.app.cx.selection.items = ids.iter().map(|i| ObjectRef::Dimension(*i)).collect();
    let labels: Vec<_> = edit_actions(&sim.app.cx).iter().map(|a| a.label).collect();
    for want in [
        "Join Into One Dimension String",
        "Select Dimension String",
        "Take Out of Dimension String",
    ] {
        assert!(labels.contains(&want), "{labels:?}");
    }
    sim.app.cx.run_custom(CMD_JOIN);
    assert_eq!(sim.app.cx.floor().string_members(ids[0]).len(), 3);
    sim.app.cx.selection.set(ObjectRef::Dimension(ids[1]));
    sim.app.cx.run_custom(CMD_LEAVE);
    assert_eq!(sim.app.cx.floor().string_members(ids[1]).len(), 1);
    assert_eq!(sim.app.cx.floor().string_members(ids[0]).len(), 2);
    sim.app.cx.selection.set(ObjectRef::Dimension(ids[2]));
    sim.app.cx.run_custom(CMD_SELECT_STRING);
    assert_eq!(sim.app.cx.selection.items.len(), 2);
    // Each command was one undo step: the leave goes back to the full string.
    sim.undo();
    assert_eq!(sim.app.cx.floor().string_members(ids[0]).len(), 3);
}
