//! Scenario 97 (round 16, brief 26): dynamic defaults and Set as Default,
//! Multiple Saved Defaults, Default Sets, Saved Plan Views with rotation,
//! Reverse Plan, templates and Import Settings.
//!
//! The brief called this s85; that number was taken, so the file is s97.

use super::{draw_shell, Sim};
use crate::dialogs::{
    default_sets::{self, Pick},
    import_settings, plan_views, saved_defaults, template_chooser,
};
use crate::shell::view_commands as vc;
use crate::templates::{self, PlanSeed};
use crate::toolbar::{Action, ViewFlag};
use plan_chiefplan::TemplateDimensionDefaults;
use plan_core::cad::CadItem;
use plan_core::defaults::import::{items, Clash, ImportCategory, ImportItem, ImportSource};
use plan_core::defaults::saved::SavedKind;
use plan_core::defaults::template::PurgeCategory;
use plan_core::defaults::PlanDefaults;
use plan_core::geometry::Point;
use plan_core::layer_sets::DEFAULT_PLAN_VIEW_NAME;
use plan_core::{OpeningKind, Project};
use std::collections::BTreeSet;

fn custom(sim: &mut Sim, id: &'static str) {
    sim.action(Action::Custom(id));
}

/// A decode shaped like Daniel's x17 working template: 34 layer sets, 14
/// dimension default sets, 13 Rich Text default sets and 20 saved plan views.
fn x17_seed() -> PlanSeed {
    let layer_sets: Vec<String> = [
        "Camera View",
        "Kitchen & Bath Elevation",
        "Elevation View",
        "Section View",
        "3D Framing",
        "Electrical",
        "Framing",
        "HVAC",
        "Plot Plan",
        "Roof Plan",
        "Kitchen & Bath",
        "Steel Framing",
        "Presentation",
        "Reference Display",
        "Working",
        "Floor Plan Dimensioned",
        "Floor Plan Shell",
        "Foundation",
        "Framing, Ceiling",
        "Framing Porch",
        "Framing, Roof",
        "Square Footage",
        "Window Schedule",
        "DWG EXPORT",
        "Presentation Elevation View",
        "Kitchen and Bath Elevation",
        "Framing, Floor",
        "Foundation Plan Dimensioned",
        "Detail",
        "DWG Export",
        "Terrain Plan",
        "Kitchen and Bath",
        "3D Steel Framing",
        "FINISH FLOOR",
    ]
    .iter()
    .map(|n| format!("{n} Layer Set"))
    .collect();
    assert_eq!(layer_sets.len(), 34);
    let dims = [
        "Plot Plan",
        "1/2\" Scale",
        "1/4\" Scale",
        "1/8\" Scale",
        "Electrical",
        "Framing",
        "HVAC",
        "Roof",
        "NKBA",
        "1\" Scale",
        "Foundation",
        "Kitchen and Bath",
        "Legacy NKBA",
        "Structural Steel",
    ];
    let rich = [
        "1/4\" Scale",
        "Plot Plan",
        "1/8\" Scale",
        "Electrical",
        "Framing",
        "HVAC",
        "Roof",
        "1/2\" Scale",
        "NKBA",
        "1\" Scale",
        "Foundation",
        "Kitchen and Bath",
        "Structural Steel",
    ];
    PlanSeed {
        path: "/t/x17.plan".into(),
        file_name: "x17 Working Template.plan".into(),
        layer_set_names: layer_sets,
        plan_view_names: plan_core::layer_sets::TEMPLATE_PLAN_VIEWS
            .iter()
            .map(|(v, _)| v.to_string())
            .collect(),
        rich_text_defaults: rich
            .iter()
            .map(|n| format!("{n} Rich Text Defaults"))
            .collect(),
        dimension_defaults: dims
            .iter()
            .map(|n| TemplateDimensionDefaults {
                name: format!("{n} Dimension Defaults"),
                // Each scale has its own arrow size, so the sets differ.
                arrow_size_in: Some(match *n {
                    "1/8\" Scale" => 6.0,
                    "1/4\" Scale" => 3.0,
                    _ => 4.5,
                }),
                ..TemplateDimensionDefaults::default()
            })
            .collect(),
        ..PlanSeed::default()
    }
}

fn x17_sim() -> Sim {
    let mut sim = Sim::new();
    let d = templates::overlay(crate::plan_defaults::embedded(), &x17_seed());
    let project = Project::from_defaults("Untitled", &d);
    sim.app.cx.defaults = d;
    sim.app.cx.set_project(project);
    sim
}

// ===================================================================
// Item 3: Default Sets over Daniel's inventory
// ===================================================================

#[test]
fn daniels_inventory_arrives_as_layer_sets_saved_defaults_default_sets_and_views() {
    let sim = x17_sim();
    let p = &sim.app.cx.project;
    let d = &sim.app.cx.defaults;
    // 34 layer sets (plus the plan's own Default Set), 14 dimension sets.
    assert!(p.layer_sets.sets.len() >= 34);
    for n in [
        "Plot Plan Layer Set",
        "FINISH FLOOR Layer Set",
        "Working Layer Set",
    ] {
        assert!(p.layer_sets.get(n).is_some(), "{n}");
    }
    assert!(d.dimension_sets.len() >= 14);
    // 13 Rich Text defaults beside "Default".
    let rich = p.saved_defaults.list(SavedKind::RichText).unwrap();
    assert_eq!(rich.items.len(), 14);
    assert!(rich.items.iter().any(|s| s.name == "1/8\" Scale"));
    // 20 saved plan views, each on a layer set of the plan.
    assert_eq!(p.plan_views.len(), 20);
    for v in &p.plan_views {
        assert!(p.layer_sets.get(&v.layer_set).is_some(), "{}", v.name);
    }
    assert_eq!(
        p.plan_view("Plot Plan View").unwrap().layer_set,
        "Plot Plan Layer Set"
    );
    // A Default Set for every scale that has both a dimension and a Rich Text
    // default, on the layer set of its name when there is one.
    for n in ["1/4\" Scale", "1/8\" Scale", "Plot Plan", "NKBA", "Roof"] {
        let s = p.saved_defaults.set(n).unwrap_or_else(|| panic!("set {n}"));
        assert_eq!(s.member(SavedKind::ManualDimensions), Some(n));
        assert_eq!(s.member(SavedKind::RichText), Some(n));
    }
    assert_eq!(
        p.saved_defaults.set("Plot Plan").unwrap().layer_set,
        "Plot Plan Layer Set"
    );
    // A dimension set with no Rich Text default of its name makes no Default Set.
    assert!(p.saved_defaults.set("Legacy NKBA").is_none());
    // "Working Plan View" opens first, like the template's working view.
    assert_eq!(p.active_plan_view, "Working Plan View");
}

#[test]
fn switching_the_default_set_gives_new_dimensions_the_one_eighth_scale_settings() {
    let mut sim = x17_sim();
    let before = sim.app.cx.defaults.dimensions.arrow_size;
    default_sets::request_pick(Pick::DefaultSet("1/8\" Scale".into()));
    custom(&mut sim, default_sets::PICK);
    let cx = &sim.app.cx;
    assert_eq!(cx.defaults.active_dimension_set, "1/8\" Scale");
    assert_eq!(cx.defaults.dimensions.arrow_size, 6.0, "the 1/8 set's");
    assert_ne!(before, 6.0);
    let mut p = cx.project.clone();
    let mut d = cx.defaults.clone();
    assert_eq!(p.using_default_set(&mut d), Some("1/8\" Scale".to_string()));
    // The Rich Text default follows with the set.
    assert_eq!(p.saved_active(&d, SavedKind::RichText), "1/8\" Scale");
    // And back to a quarter-inch set.
    default_sets::request_pick(Pick::DefaultSet("1/4\" Scale".into()));
    custom(&mut sim, default_sets::PICK);
    assert_eq!(sim.app.cx.defaults.dimensions.arrow_size, 3.0);
    // A dimension the Dimension tool draws now uses the set in force: its
    // text format is the active set's.
    assert_eq!(sim.app.cx.defaults.active_dimension_set, "1/4\" Scale");
    // Not an undo step: it changed no object.
    assert!(!sim.app.cx.can_undo());
}

#[test]
fn a_saved_view_remembers_its_default_set_and_brings_it_back_on_opening() {
    let mut sim = x17_sim();
    // The Plot Plan View uses the "Plot Plan" Default Set.
    let mut spec = plan_views::Spec::of(&sim.app.cx, "Plot Plan View").unwrap();
    spec.default_set = "Plot Plan".into();
    spec.picks.insert("dimensions".into(), "Plot Plan".into());
    spec.picks.insert("rich_text".into(), "Plot Plan".into());
    plan_views::apply_spec(&mut sim.app.cx, "Plot Plan View", &spec).unwrap();
    // The Working Plan View uses the 1/4" set.
    let mut spec = plan_views::Spec::of(&sim.app.cx, "Working Plan View").unwrap();
    spec.default_set = "1/4\" Scale".into();
    spec.picks.insert("dimensions".into(), "1/4\" Scale".into());
    spec.picks.insert("rich_text".into(), "1/4\" Scale".into());
    plan_views::apply_spec(&mut sim.app.cx, "Working Plan View", &spec).unwrap();
    // Open the views from the toolbar's selector: the defaults follow.
    let i = sim
        .app
        .cx
        .project
        .plan_views
        .iter()
        .position(|v| v.name == "Plot Plan View")
        .unwrap();
    sim.action(Action::PlanView(i));
    assert_eq!(sim.app.cx.defaults.active_dimension_set, "Plot Plan");
    assert_eq!(sim.app.cx.project.saved_defaults.active_set, "Plot Plan");
    let j = sim
        .app
        .cx
        .project
        .plan_views
        .iter()
        .position(|v| v.name == "Working Plan View")
        .unwrap();
    sim.action(Action::PlanView(j));
    assert_eq!(sim.app.cx.defaults.active_dimension_set, "1/4\" Scale");
    // And the same through the tabs.
    crate::editor::plan_tabs::with_tabs(|t| t.open_view(&mut sim.app.cx, "Plot Plan View"));
    assert_eq!(sim.app.cx.defaults.active_dimension_set, "Plot Plan");
}

// ===================================================================
// Item 2: Saved Defaults
// ===================================================================

#[test]
fn saved_defaults_dialog_adds_renames_and_refuses_to_delete_what_a_set_uses() {
    let mut sim = Sim::new();
    custom(&mut sim, "defaults.saved.callouts");
    assert_eq!(saved_defaults::open_kind(), Some(SavedKind::Callouts));
    sim.dialog_frame(false);
    // Copy through the name prompt.
    default_sets::prompt(
        "New Callouts Default Name",
        default_sets::PromptFor::AddSaved(SavedKind::Callouts, "Default".into()),
        "",
    );
    default_sets::answer_prompt(&mut sim.app.cx, "Section Callout").unwrap();
    sim.dialog_frame(false);
    let names = {
        let cx = &mut sim.app.cx;
        cx.project.saved_names(&cx.defaults, SavedKind::Callouts)
    };
    assert_eq!(names, vec!["Default", "Section Callout"]);
    // A Default Set that uses "Default" blocks its deletion.
    let cx = &mut sim.app.cx;
    cx.project
        .saved_activate(&mut cx.defaults, SavedKind::Callouts, "Default");
    cx.project
        .default_set_save_new(&mut cx.defaults, "Plan Set")
        .unwrap();
    let why = cx
        .project
        .saved_delete(&mut cx.defaults, SavedKind::Callouts, "Default")
        .unwrap_err();
    assert!(why.contains("Plan Set"), "{why}");
    // Cancel (Escape) drops every change the dialog made: the first Escape
    // closes the defaults dialog the copy opened, the second the dialog.
    sim.dialog_frame_key(Some(eframe::egui::Key::Escape));
    sim.dialog_frame_key(Some(eframe::egui::Key::Escape));
    let cx = &mut sim.app.cx;
    assert_eq!(saved_defaults::open_kind(), None);
    assert_eq!(
        cx.project.saved_names(&cx.defaults, SavedKind::Callouts),
        vec!["Default"]
    );
    assert!(cx.project.saved_defaults.set("Plan Set").is_none());
}

#[test]
fn each_kind_has_its_own_saved_defaults_and_double_click_opens_the_active_one() {
    let mut sim = Sim::new();
    for kind in SavedKind::ALL {
        let cx = &mut sim.app.cx;
        let n = cx.project.saved_names(&cx.defaults, kind);
        assert!(!n.is_empty(), "{kind:?}");
        cx.project
            .saved_copy(&mut cx.defaults, kind, &n[0], "Second")
            .unwrap();
        assert!(cx.project.saved_activate(&mut cx.defaults, kind, "Second"));
        assert_eq!(cx.project.saved_active(&cx.defaults, kind), "Second");
    }
    // The tools that have them: a double-click on the button.
    saved_defaults::set_edit_active_on_double_click(true);
    use crate::tools::text::TextMode;
    use crate::tools::ToolId;
    assert!(saved_defaults::double_click(
        &mut sim.app.cx,
        Action::SetTool(ToolId::TextVariant(TextMode::Callout))
    ));
    assert!(
        default_sets::editor_is_open(),
        "the callout defaults dialog"
    );
    sim.dialog_frame(false);
    sim.dialog_frame_key(Some(eframe::egui::Key::Escape));
    default_sets::reset_state();
}

#[test]
fn pasting_an_object_into_another_file_recreates_its_saved_default() {
    let mut sim = Sim::new();
    let cx = &mut sim.app.cx;
    cx.project
        .saved_copy(&mut cx.defaults, SavedKind::Markers, "Default", "Elevation")
        .unwrap();
    cx.project
        .saved_activate(&mut cx.defaults, SavedKind::Markers, "Elevation");
    cx.project.annot_defaults.marker.items.clear();
    let value = cx
        .project
        .saved_value(&mut cx.defaults, SavedKind::Markers, "Elevation")
        .unwrap();
    // The other file lacks it; pasting stores it once, a second paste reuses it.
    let mut other = Sim::new();
    let o = &mut other.app.cx;
    assert!(!o
        .project
        .saved_names(&o.defaults, SavedKind::Markers)
        .contains(&"Elevation".to_string()));
    o.project.saved_store(
        &mut o.defaults,
        SavedKind::Markers,
        "Elevation",
        value.clone(),
        true,
    );
    o.project.saved_store(
        &mut o.defaults,
        SavedKind::Markers,
        "Elevation",
        value,
        true,
    );
    assert_eq!(
        o.project
            .saved_names(&o.defaults, SavedKind::Markers)
            .iter()
            .filter(|n| *n == "Elevation")
            .count(),
        1
    );
}

// ===================================================================
// Item 1: dynamic defaults and Set as Default
// ===================================================================

fn shell_with_door(sim: &mut Sim) -> (plan_core::Id, plan_core::Id) {
    draw_shell(sim, 240.0, 180.0);
    let wall = sim.wall_ids()[0];
    let a = sim
        .app
        .cx
        .project
        .add_opening(0, wall, 60.0, OpeningKind::Door)
        .expect("a door");
    let b = sim
        .app
        .cx
        .project
        .add_opening(0, wall, 160.0, OpeningKind::Door)
        .expect("a second door");
    (a, b)
}

#[test]
fn a_use_default_door_follows_a_changed_door_default_and_an_overridden_one_does_not() {
    use plan_core::openings::{types::UseDefault, DefaultKey};
    use plan_core::OpeningStyle;
    let mut sim = Sim::new();
    let (follower, own) = shell_with_door(&mut sim);
    let hinges = |sim: &Sim, id| {
        sim.app
            .cx
            .floor()
            .openings
            .iter()
            .find(|o| o.id == id)
            .unwrap()
            .extras
            .spec
            .hardware
            .hinges
    };
    {
        let f = &mut sim.app.cx.project.floors[0];
        for o in &mut f.openings {
            if o.id == follower {
                o.extras.spec.dynamic = UseDefault::all(OpeningKind::Door);
            } else if o.id == own {
                // Edited: it gave up Use Default for its hardware.
                o.extras.spec.dynamic = UseDefault::all(OpeningKind::Door);
                o.extras.spec.dynamic.hardware = false;
            }
        }
    }
    sim.dialog_frame(false); // the tracker remembers the defaults
    let old_hinges = hinges(&sim, follower);
    // The door default changes to four hinges.
    // The door stands in an exterior wall: its default is the exterior one.
    let key = {
        let o = sim
            .app
            .cx
            .floor()
            .openings
            .iter()
            .find(|o| o.id == follower)
            .unwrap();
        DefaultKey::of(o, plan_core::WallKind::Exterior)
    };
    let _ = OpeningStyle::Hinged;
    let mut template = plan_core::Opening::default_door(0, 0, 0.0);
    template.extras.spec.hardware.hinges = old_hinges + 1;
    sim.app
        .cx
        .defaults
        .opening_variants
        .set_type_default(key, template);
    sim.dialog_frame(false);
    assert_eq!(
        hinges(&sim, follower),
        old_hinges + 1,
        "Use Default follows"
    );
    assert_eq!(hinges(&sim, own), old_hinges, "an overridden door does not");
    assert_eq!(sim.app.cx.undo_label(), Some("Default Settings"));
    // One undo step puts it back.
    sim.undo();
    assert_eq!(hinges(&sim, follower), old_hinges);
}

#[test]
fn walls_on_the_default_follow_a_changed_wall_default_and_edited_walls_keep_their_own() {
    let mut sim = Sim::new();
    draw_shell(&mut sim, 240.0, 180.0);
    let ids = sim.wall_ids();
    let default_height = sim.app.cx.defaults.exterior_wall.height;
    // One wall was edited to its own height.
    sim.app.cx.project.floors[0]
        .wall_mut(ids[0])
        .unwrap()
        .height = default_height + 12.0;
    sim.dialog_frame(false);
    sim.app.cx.defaults.exterior_wall.height = default_height + 24.0;
    sim.dialog_frame(false);
    let h = |sim: &Sim, i: usize| sim.app.cx.floor().wall(ids[i]).unwrap().height;
    assert_eq!(
        h(&sim, 0),
        default_height + 12.0,
        "the edited wall keeps its height"
    );
    #[allow(clippy::needless_range_loop)]
    for i in 1..ids.len() {
        assert_eq!(
            h(&sim, i),
            default_height + 24.0,
            "wall {i} uses the default"
        );
    }
    assert_eq!(sim.app.cx.undo_label(), Some("Default Settings"));
    // Use Default, set on the edited wall, makes it follow the next change.
    assert!(sim.app.cx.project.set_group_follows(
        plan_core::defaults::dynamic::DefaultKind::ExteriorWall,
        ids[0],
        "height",
        true
    ));
    sim.app.cx.defaults.exterior_wall.height = default_height + 36.0;
    sim.dialog_frame(false);
    assert_eq!(h(&sim, 0), default_height + 36.0);
}

#[test]
fn set_as_default_copies_the_selected_walls_spec_and_the_others_follow() {
    let mut sim = Sim::new();
    draw_shell(&mut sim, 240.0, 180.0);
    let ids = sim.wall_ids();
    sim.dialog_frame(false);
    sim.app.cx.project.floors[0]
        .wall_mut(ids[0])
        .unwrap()
        .height = 120.0;
    sim.app
        .cx
        .selection
        .set(crate::editor::ObjectRef::Wall(ids[0]));
    assert!(crate::plan_defaults::can_set_as_default(&sim.app.cx));
    assert!(sim
        .app
        .cx
        .selection_edit_actions()
        .iter()
        .any(|a| a.label == "Set as Default"));
    custom(&mut sim, crate::plan_defaults::SET_AS_DEFAULT);
    assert_eq!(sim.app.cx.defaults.exterior_wall.height, 120.0);
    assert!(
        sim.app.cx.status.contains("Exterior Wall"),
        "{}",
        sim.app.cx.status
    );
    // The walls on the default follow it; the next frame must not redo it.
    for (i, id) in ids.iter().enumerate().skip(1) {
        assert_eq!(
            sim.app.cx.floor().wall(*id).unwrap().height,
            120.0,
            "wall {i}"
        );
    }
    let depth = sim.app.cx.undo_depth();
    sim.dialog_frame(false);
    assert_eq!(
        sim.app.cx.undo_depth(),
        depth,
        "the change was handled once"
    );
    // Terrain paths and nothing selected are refused.
    sim.app.cx.selection.clear();
    custom(&mut sim, crate::plan_defaults::SET_AS_DEFAULT);
    assert!(sim.app.cx.status.contains("Select one object"));
}

#[test]
fn set_as_default_on_a_door_updates_the_door_defaults() {
    let mut sim = Sim::new();
    let (door, _) = shell_with_door(&mut sim);
    {
        let o = sim.app.cx.project.floors[0]
            .openings
            .iter_mut()
            .find(|o| o.id == door)
            .unwrap();
        o.width = 36.0;
    }
    sim.app
        .cx
        .selection
        .set(crate::editor::ObjectRef::Opening(door));
    custom(&mut sim, crate::plan_defaults::SET_AS_DEFAULT);
    let key = plan_core::openings::DefaultKey::new(
        OpeningKind::Door,
        plan_core::OpeningStyle::Hinged,
        true,
    );
    let t =
        sim.app
            .cx
            .defaults
            .opening_variants
            .type_default(key)
            .or_else(|| {
                sim.app.cx.defaults.opening_variants.type_default(
                    plan_core::openings::DefaultKey::new(
                        OpeningKind::Door,
                        plan_core::OpeningStyle::Hinged,
                        false,
                    ),
                )
            })
            .expect("the door default exists");
    assert_eq!(t.template.width, 36.0);
}

// ===================================================================
// Item 4: Saved Plan Views, rotation and Reverse Plan
// ===================================================================

#[test]
fn a_saved_view_with_rotation_restores_it_on_reopen_and_through_the_file() {
    let mut sim = Sim::new();
    sim.app.cx.project.insert_floor_above(0).unwrap();
    // Rotate to a typed angle (relative to north up, 270 reads -90).
    vc::open_rotate_dialog(270.0);
    assert!(vc::rotate_dialog_open());
    assert_eq!(vc::parse_angle(" -90\u{b0} "), Some(-90.0));
    vc::request_rotation(270.0);
    let rad = vc::take_pending_rotation().unwrap();
    sim.app.camera.rotation = rad;
    assert!((vc::reported_rotation_deg() + 90.0).abs() < 1e-9);
    // Rotating by 90 twice is 90, not 180: the dialog sets, it does not add.
    vc::request_rotation(90.0);
    vc::request_rotation(90.0);
    assert!((vc::take_pending_rotation().unwrap().to_degrees() - 90.0).abs() < 1e-9);
    // Save the view with the rotation of the plan as shown.
    vc::report_rotation(rad);
    custom(&mut sim, plan_views::SAVE);
    let v = sim.app.cx.project.current_plan_view().unwrap();
    assert!(
        (v.spec.rotation_deg + 90.0).abs() < 1e-9,
        "{}",
        v.spec.rotation_deg
    );
    // Leave for another view and come back: the rotation returns.
    custom(&mut sim, plan_views::NEW_SAVED);
    assert!(plan_views::new_dialog_open());
    plan_views::answer_new_dialog(&mut sim.app.cx, "Elsewhere", None).unwrap();
    let rot = vc::take_pending_rotation();
    assert!(rot.is_some(), "the new view brings a rotation (the copy's)");
    vc::request_rotation(0.0);
    vc::take_pending_rotation();
    crate::editor::plan_tabs::with_tabs(|t| t.open_view(&mut sim.app.cx, DEFAULT_PLAN_VIEW_NAME));
    let back = vc::take_pending_rotation().unwrap();
    assert!(
        (back.to_degrees() - 270.0).abs() < 1e-9,
        "{}",
        back.to_degrees()
    );
    // And through the file.
    let json = sim.app.cx.project.to_json().unwrap();
    let loaded = Project::from_json(&json).unwrap();
    assert!(
        (loaded
            .plan_view(DEFAULT_PLAN_VIEW_NAME)
            .unwrap()
            .spec
            .rotation_deg
            + 90.0)
            .abs()
            < 1e-9
    );
    // Reset Plan View shows it as saved again.
    custom(&mut sim, plan_views::RESET);
    assert!(vc::take_pending_rotation().is_some());
}

#[test]
fn reverse_plan_twice_is_the_identity_with_hidden_and_locked_layers_and_each_is_one_undo_step() {
    let mut sim = Sim::new();
    draw_shell(&mut sim, 240.0, 180.0);
    let cx = &mut sim.app.cx;
    // Objects on a hidden layer and on a locked layer.
    let hidden = cx.project.add_cad(
        0,
        "CAD, Hidden Notes",
        CadItem::Line {
            a: Point::new(10.0, 10.0),
            b: Point::new(60.0, 30.0),
        },
    );
    let locked = cx.project.add_cad(
        0,
        "CAD, Locked Notes",
        CadItem::Line {
            a: Point::new(20.0, 100.0),
            b: Point::new(90.0, 120.0),
        },
    );
    for (name, hide, lock) in [
        ("CAD, Hidden Notes", true, false),
        ("CAD, Locked Notes", false, true),
    ] {
        if cx.project.layers.get(name).is_none() {
            cx.project
                .layers
                .add(plan_core::Layer::new(name, [128, 128, 128], 25));
        }
        if let Some(l) = cx.project.layers.get_mut(name) {
            l.display = !hide;
            l.locked = lock;
        }
    }
    cx.refresh_layer_view();
    cx.refresh();
    let line_of = |sim: &Sim, id| {
        sim.app
            .cx
            .floor()
            .cad
            .iter()
            .find(|c| c.id == id)
            .and_then(|c| match &c.item {
                CadItem::Line { a, .. } => Some(*a),
                _ => None,
            })
            .unwrap()
    };
    let original = sim.app.cx.project.clone();
    let (h0, l0) = (line_of(&sim, hidden), line_of(&sim, locked));
    custom(&mut sim, vc::REVERSE_PLAN);
    assert_eq!(sim.app.cx.undo_label(), Some("Reverse Plan"));
    let (lo, hi) = vc::building_bounds(&sim.app.cx).unwrap();
    let mid = (lo.x + hi.x) / 2.0;
    // Hidden and locked layers turn with the rest of the plan (DECISIONS DS7).
    assert!((line_of(&sim, hidden).x - (2.0 * mid - h0.x)).abs() < 1e-6);
    assert!((line_of(&sim, locked).x - (2.0 * mid - l0.x)).abs() < 1e-6);
    // The layers are as they were.
    assert!(
        !sim.app
            .cx
            .project
            .layers
            .get("CAD, Hidden Notes")
            .unwrap()
            .display
    );
    assert!(
        sim.app
            .cx
            .project
            .layers
            .get("CAD, Locked Notes")
            .unwrap()
            .locked
    );
    // Twice is the identity.
    custom(&mut sim, vc::REVERSE_PLAN);
    assert!((line_of(&sim, hidden).x - h0.x).abs() < 1e-6);
    assert!((line_of(&sim, locked).x - l0.x).abs() < 1e-6);
    for (a, b) in sim.app.cx.project.floors[0]
        .walls
        .iter()
        .zip(&original.floors[0].walls)
    {
        assert!(a.start.dist(b.start) < 1e-6 && a.end.dist(b.end) < 1e-6);
    }
    // Each is one undo step.
    sim.undo();
    assert!((line_of(&sim, hidden).x - (2.0 * mid - h0.x)).abs() < 1e-6);
    sim.undo();
    assert!((line_of(&sim, hidden).x - h0.x).abs() < 1e-6);
}

#[test]
fn rotate_plan_view_dialog_draws_and_cancel_keeps_the_rotation() {
    let mut sim = Sim::new();
    vc::take_pending_rotation();
    custom(&mut sim, vc::ROTATE_DIALOG);
    assert!(vc::rotate_dialog_open());
    sim.app.cx.status.clear();
    let ctx = eframe::egui::Context::default();
    let _ = ctx.run(eframe::egui::RawInput::default(), |ctx| {
        vc::show_rotate_dialog(ctx, &mut sim.app.cx)
    });
    assert!(vc::rotate_dialog_open());
    sim.dialog_frame_key(Some(eframe::egui::Key::Escape));
    let ctx2 = eframe::egui::Context::default();
    let mut input = eframe::egui::RawInput::default();
    input.events.push(eframe::egui::Event::Key {
        key: eframe::egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: eframe::egui::Modifiers::NONE,
    });
    let _ = ctx2.run(input, |ctx| vc::show_rotate_dialog(ctx, &mut sim.app.cx));
    assert!(!vc::rotate_dialog_open());
    assert_eq!(vc::take_pending_rotation(), None);
}

#[test]
fn a_new_saved_plan_view_can_copy_its_layer_set_and_duplicate_opens_the_same_dialog() {
    let mut sim = x17_sim();
    let sets = sim.app.cx.project.layer_sets.sets.len();
    custom(&mut sim, plan_views::NEW_SAVED);
    assert!(plan_views::new_dialog_open());
    assert!(
        plan_views::answer_new_dialog(&mut sim.app.cx, "Plot Copy", Some("Plot Copy Layers"))
            .is_ok()
    );
    assert_eq!(sim.app.cx.project.layer_sets.sets.len(), sets + 1);
    assert_eq!(sim.app.cx.project.plan_views.len(), 21);
    assert_eq!(sim.app.cx.undo_label(), Some("New Saved Plan View"));
    sim.undo();
    assert_eq!(sim.app.cx.project.plan_views.len(), 20);
    // Duplicate (the Project Browser) opens the New Saved Plan View dialog.
    plan_views::request_duplicate(&sim.app.cx, "Roof Plan View");
    assert!(plan_views::new_dialog_open());
    sim.dialog_frame(false);
    plan_views::answer_new_dialog(&mut sim.app.cx, "Roof Plan View (2)", None).unwrap();
    assert!(sim.app.cx.project.plan_view("Roof Plan View (2)").is_some());
}

// ===================================================================
// Item 5: Templates and Import Settings
// ===================================================================

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("plan-studio-s97-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

#[test]
fn save_as_template_purges_sets_the_default_and_new_plan_starts_from_it_or_asks() {
    let dir = tmp("templates");
    templates::set_plan_templates_dir_for_tests(Some(dir.clone()));
    let mut sim = Sim::new();
    draw_shell(&mut sim, 240.0, 180.0);
    shell_with_door_in_place(&mut sim);
    let cx = &mut sim.app.cx;
    cx.project.name = "Smith House".into();
    cx.project
        .saved_copy(&mut cx.defaults, SavedKind::RichText, "Default", "Plot")
        .unwrap();
    let purge: BTreeSet<PurgeCategory> = PurgeCategory::ALL.iter().copied().collect();
    let path = template_chooser::save_template(cx, "Smith Template", &purge, true).unwrap();
    assert!(path.is_file());
    assert!(
        !sim.app.cx.floor().walls.is_empty(),
        "the open plan keeps its data"
    );
    // New Plan starts from the default template: no walls, but the saved
    // default and the defaults came along.
    sim.app.new_project();
    assert_eq!(sim.app.cx.floor().walls.len(), 0);
    assert_eq!(sim.app.cx.project.name, "Untitled");
    let names = {
        let cx = &mut sim.app.cx;
        cx.project.saved_names(&cx.defaults, SavedKind::RichText)
    };
    assert!(names.contains(&"Plot".to_string()));
    // The default template goes missing: New Plan asks instead of guessing.
    std::fs::remove_file(&path).unwrap();
    draw_shell(&mut sim, 120.0, 120.0);
    sim.app.new_project();
    assert!(template_chooser::missing_open());
    assert!(
        !sim.app.cx.floor().walls.is_empty(),
        "nothing was replaced yet"
    );
    // Load Installed starts a plan from the installed defaults.
    template_chooser::load_installed_for_tests();
    sim.app.poll_template_requests();
    assert_eq!(sim.app.cx.floor().walls.len(), 0);
    templates::set_plan_templates_dir_for_tests(None);
    let _ = std::fs::remove_dir_all(&dir);
    template_chooser::reset_state();
}

fn shell_with_door_in_place(sim: &mut Sim) {
    let wall = sim.wall_ids()[0];
    sim.app
        .cx
        .project
        .add_opening(0, wall, 60.0, OpeningKind::Door)
        .expect("a door");
}

#[test]
fn import_settings_applies_the_picked_categories_once_as_one_undo_step() {
    import_settings_reset();
    let mut sim = Sim::new();
    let mut d = PlanDefaults::chief_x18_daniel();
    let mut p = Project::from_defaults("Source", &d);
    p.layer_sets.copy_set("Default Set", "Imported Layers");
    p.saved_copy(&mut d, SavedKind::Callouts, "Default", "Imported Callout")
        .unwrap();
    p.default_set_save_new(&mut d, "Imported Set").unwrap();
    p.note_types.add("Imported Note", "IN");
    p.plan_views.push(plan_core::SavedPlanView::new(
        "Imported View",
        "Imported Layers",
    ));
    let src = ImportSource {
        project: p,
        defaults: Some(d),
        layout: false,
        imperial: true,
    };
    let all = items(&src);
    for c in [
        ImportCategory::SavedDefaults,
        ImportCategory::LayerSets,
        ImportCategory::NoteTypes,
        ImportCategory::SavedPlanViews,
        ImportCategory::WallTypes,
    ] {
        assert!(all.iter().any(|i| i.category == c), "{c:?}");
    }
    import_settings::open_source(src.clone(), "Source.psplan");
    sim.dialog_frame(false);
    assert!(import_settings::is_open());
    let picked: Vec<ImportItem> = all
        .iter()
        .filter(|i| {
            matches!(
                i.name.as_str(),
                "Imported Layers"
                    | "Imported Callout"
                    | "Imported Set"
                    | "Imported Note"
                    | "Imported View"
            )
        })
        .cloned()
        .collect();
    assert_eq!(picked.len(), 5);
    import_settings::pick(&picked, true);
    import_settings::set_clash(Clash::Rename);
    let before = sim.app.cx.project.layer_sets.sets.len();
    let status = import_settings::accept(&mut sim.app.cx).unwrap();
    assert!(status.starts_with("Imported 5"), "{status}");
    assert_eq!(sim.app.cx.project.layer_sets.sets.len(), before + 1);
    assert!(sim.app.cx.project.note_types.get("Imported Note").is_some());
    assert!(sim.app.cx.project.plan_view("Imported View").is_some());
    assert!(sim
        .app
        .cx
        .project
        .saved_defaults
        .set("Imported Set")
        .is_some());
    assert_eq!(sim.app.cx.undo_label(), Some("Import Settings"));
    // Applied once: the window is closed and accepting again does nothing.
    assert!(import_settings::accept(&mut sim.app.cx).is_none());
    sim.undo();
    assert_eq!(sim.app.cx.project.layer_sets.sets.len(), before);
    assert!(sim.app.cx.project.plan_view("Imported View").is_none());
}

fn import_settings_reset() {
    import_settings::reset_state();
}

// ===================================================================
// The leaves the earlier builders left for this brief
// ===================================================================

#[test]
fn the_tray_ceiling_defaults_leaf_edits_the_tray_a_new_tray_starts_from() {
    use crate::dialogs::tray_ceiling as tray;
    let mut sim = Sim::new();
    assert!(sim.app.cx.defaults.tray_ceiling.is_none());
    custom(&mut sim, tray::DEFAULTS);
    assert!(tray::dialog_open());
    sim.dialog_frame(false);
    tray::with_dialog(|d| d.draft_mut().depth = 9.0).unwrap();
    // OK stores the plan defaults; no tray is made.
    let made = tray::accept_dialog(&mut sim.app.cx);
    assert!(made.is_none());
    assert_eq!(
        sim.app.cx.defaults.tray_ceiling.as_ref().unwrap().depth,
        9.0
    );
    assert_eq!(sim.app.cx.defaults.tray_default().depth, 9.0);
    assert!(sim.app.cx.floor().trays.trays.is_empty());
    // Reopening shows the stored values.
    custom(&mut sim, tray::DEFAULTS);
    assert_eq!(tray::with_dialog(|d| d.draft().depth).unwrap(), 9.0);
    let _ = tray::accept_dialog(&mut sim.app.cx);
}

#[test]
fn the_watermark_and_default_sets_leaves_open_their_windows() {
    let mut sim = Sim::new();
    custom(&mut sim, crate::dialogs::watermark::DEFAULTS);
    sim.dialog_frame(false);
    assert!(crate::dialogs::watermark::is_open());
    custom(&mut sim, default_sets::DEFAULT_SETS);
    sim.dialog_frame(false);
    assert!(default_sets::sets_is_open());
    custom(&mut sim, default_sets::ACTIVE_DEFAULTS);
    sim.dialog_frame(false);
    assert!(default_sets::active_is_open());
    default_sets::reset_state();
    // And both are leaves of the Default Settings tree.
    let leaves = crate::dialogs::defaults::all_leaves();
    for want in [
        "Watermark",
        "Tray Ceiling",
        "Default Sets",
        "Rich Text",
        "Revision Clouds",
        "Framing Types",
    ] {
        assert!(leaves.iter().any(|(_, n, _)| *n == want), "{want}");
    }
}

#[test]
fn the_toolbar_controls_are_catalog_buttons_and_show_color_follows_the_view() {
    for want in [
        "Active Layer Set",
        "Active Dimension Defaults",
        "Active Default Set",
    ] {
        assert!(
            crate::toolbar::config::catalog()
                .iter()
                .any(|e| e.key == want),
            "{want}"
        );
    }
    let mut sim = Sim::new();
    let mut spec = plan_views::Spec::of(&sim.app.cx, DEFAULT_PLAN_VIEW_NAME).unwrap();
    spec.show_color = false;
    plan_views::apply_spec(&mut sim.app.cx, DEFAULT_PLAN_VIEW_NAME, &spec).unwrap();
    assert!(!sim.app.cx.view_flags.contains(&ViewFlag::Color));
}
