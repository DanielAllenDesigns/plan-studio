//! Plan View Specification (Tools > Plan Views): a saved plan view's name,
//! layer set, floor, reference display, default dimension and text styles and
//! zoom, with New, Duplicate and Delete, plus the Save Plan View and Reset
//! Plan View commands.
//!
//! `run_command` opens the window (or runs Save / Reset) from a menu id;
//! `show_all` draws it (the shell calls it from `docks::show_dialogs`).

use crate::editor::plan_tabs;
use crate::editor::EditorContext;
use eframe::egui::{self, Align2, Vec2};
use plan_core::geometry::Point;
use plan_core::SavedPlanView;
use std::cell::RefCell;

/// Menu id: Tools > Plan Views > Plan View Specification...
pub const OPEN: &str = "views.specification";
/// Menu id: Tools > Plan Views > Save Plan View.
pub const SAVE: &str = "views.save";
/// Menu id: Tools > Plan Views > Reset Plan View.
pub const RESET: &str = "views.reset";
/// Menu id: Tools > Plan Views > Add Template Plan Views.
pub const SEED: &str = "views.add_template";

// ----- operations -----

/// The edits of the specification form.
#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    pub name: String,
    pub layer_set: String,
    pub floor: Option<usize>,
    pub reference_display: bool,
    /// Floor shown as reference, relative to the viewed floor.
    pub reference_floor: Option<i32>,
    pub dimension_defaults: String,
    pub text_style: String,
    /// Zoom in screen pixels per plan inch; `None` keeps the camera as saved.
    pub zoom: Option<f64>,
}

impl Spec {
    /// The form of view `name`.
    pub fn of(cx: &EditorContext, name: &str) -> Option<Spec> {
        let v = cx.project.plan_view(name)?;
        Some(Spec {
            name: v.name.clone(),
            layer_set: v.layer_set.clone(),
            floor: v.floor,
            reference_display: v.reference_display,
            reference_floor: v.reference_floor,
            dimension_defaults: v.dimension_defaults.clone(),
            text_style: v.text_style.clone(),
            zoom: v.camera.map(|c| c.1),
        })
    }
}

/// OK / Apply: writes `spec` into the plan view `original` (one undo step).
/// The name may change while it stays unique; the layer set must exist; the
/// floor must exist. When the view is the one shown, it is shown again so the
/// plan follows the new floor, reference display and defaults.
pub fn apply_spec(cx: &mut EditorContext, original: &str, spec: &Spec) -> Result<(), String> {
    if cx.project.plan_view(original).is_none() {
        return Err(format!("There is no plan view \"{original}\""));
    }
    let name = spec.name.trim();
    if name.is_empty() {
        return Err("Type a name for the plan view".into());
    }
    if name != original && cx.project.plan_view(name).is_some() {
        return Err(format!("A plan view named \"{name}\" exists already"));
    }
    if cx.project.layer_sets.get(&spec.layer_set).is_none() {
        return Err(format!("There is no layer set \"{}\"", spec.layer_set));
    }
    if spec.floor.is_some_and(|f| f >= cx.project.floors.len()) {
        return Err("That floor does not exist".into());
    }
    cx.begin_change("Plan View Specification");
    cx.project.rename_plan_view(original, name);
    if let Some(v) = cx.project.plan_views.iter_mut().find(|v| v.name == name) {
        v.layer_set = spec.layer_set.clone();
        v.floor = spec.floor;
        v.reference_display = spec.reference_display;
        v.reference_floor = spec.reference_floor;
        v.dimension_defaults = spec.dimension_defaults.clone();
        v.text_style = spec.text_style.clone();
        if let Some(z) = spec.zoom {
            let center = v.camera.map_or(Point::ZERO, |c| c.0);
            v.camera = Some((center, z.clamp(0.05, 50.0)));
        }
    }
    if cx.project.active_plan_view == name {
        cx.show_plan_view(name);
    }
    cx.mark_dirty();
    Ok(())
}

/// New: a view that starts as a copy of the active one, named "Plan View N";
/// it becomes a tab and the shown view. Returns its name.
pub fn new_view(cx: &mut EditorContext) -> String {
    let name = crate::dialogs::app_info::new_plan_view(cx);
    plan_tabs::with_tabs(|t| t.open_view(cx, &name));
    name
}

/// Duplicate: copies `name` under a free name. Returns the new name.
pub fn duplicate_view(cx: &mut EditorContext, name: &str) -> Option<String> {
    cx.project.plan_view(name)?;
    cx.begin_change("Duplicate Plan View");
    cx.project.duplicate_plan_view(name)
}

/// Delete: removes the view (the last one stays).
pub fn delete_view(cx: &mut EditorContext, name: &str) -> Result<(), String> {
    if cx.project.plan_view(name).is_none() {
        return Err(format!("There is no plan view \"{name}\""));
    }
    if cx.project.plan_views.len() <= 1 {
        return Err("The last plan view cannot be deleted".into());
    }
    cx.begin_change("Delete Plan View");
    cx.project.delete_plan_view(name);
    if let Some(v) = cx.project.current_plan_view().map(|v| v.name.clone()) {
        cx.show_plan_view(&v);
    }
    cx.mark_dirty();
    Ok(())
}

/// Renames a view (the Project Browser's Rename).
pub fn rename_view(cx: &mut EditorContext, old: &str, new_name: &str) -> Result<(), String> {
    let new_name = new_name.trim();
    if new_name.is_empty() {
        return Err("Type a name for the plan view".into());
    }
    if cx.project.plan_view(old).is_none() {
        return Err(format!("There is no plan view \"{old}\""));
    }
    if new_name != old && cx.project.plan_view(new_name).is_some() {
        return Err(format!("A plan view named \"{new_name}\" exists already"));
    }
    cx.begin_change("Rename Plan View");
    cx.project.rename_plan_view(old, new_name);
    cx.mark_dirty();
    Ok(())
}

/// Drag to reorder in the Project Browser: moves the view at `from` to `to`
/// (one undo step).
pub fn move_view(cx: &mut EditorContext, from: usize, to: usize) -> bool {
    let n = cx.project.plan_views.len();
    if from >= n || to >= n || from == to {
        return false;
    }
    cx.begin_change("Move Plan View");
    cx.project.move_plan_view(from, to);
    cx.mark_dirty();
    true
}

/// Adds Daniel's template plan views the plan lacks (one undo step); returns
/// how many.
pub fn add_template_views(cx: &mut EditorContext) -> usize {
    let mut probe = cx.project.clone();
    if probe.seed_template_plan_views() == 0 {
        return 0;
    }
    cx.begin_change("Add Template Plan Views");
    let n = cx.project.seed_template_plan_views();
    cx.mark_dirty();
    n
}

// ----- the window -----

#[derive(Default)]
struct State {
    open: bool,
    /// The view being edited (its name when the window last synced).
    original: Option<String>,
    draft: Option<Spec>,
    message: String,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Opens the window on the shown view.
pub fn open() {
    state(|s| {
        s.open = true;
        s.original = None;
        s.draft = None;
        s.message.clear();
    });
}

/// Is the window open?
pub fn is_open() -> bool {
    state(|s| s.open)
}

/// Opens the window on the view `name` (the Project Browser's Specification).
pub fn open_on(name: &str) {
    state(|s| {
        s.open = true;
        s.original = Some(name.to_string());
        s.draft = None;
        s.message.clear();
    });
}

/// Runs a menu command by id; false when the id is not ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        OPEN => open(),
        SAVE => {
            plan_tabs::with_tabs(|t| t.save_active(cx));
        }
        RESET => {
            plan_tabs::with_tabs(|t| t.reset_active(cx));
        }
        SEED => {
            let n = add_template_views(cx);
            cx.status = if n == 0 {
                "The template plan views are already in this plan".into()
            } else {
                format!("Added {n} template plan views")
            };
        }
        _ => return false,
    }
    true
}

/// Draws the window when it is open.
pub fn show_all(ctx: &egui::Context, cx: &mut EditorContext) {
    if !is_open() {
        return;
    }
    let mut open = true;
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        egui::Window::new("Plan View Specification")
            .id(egui::Id::new("plan_view_specification"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(560.0, 440.0))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| body(ui, cx, &mut st));
    });
    if !open {
        state(|s| s.open = false);
    }
}

fn body(ui: &mut egui::Ui, cx: &mut EditorContext, st: &mut State) {
    // Which view the form shows.
    let original = match st.original.clone() {
        Some(n) if cx.project.plan_view(&n).is_some() => n,
        _ => cx.project.active_plan_view.clone(),
    };
    if st.original.as_deref() != Some(original.as_str()) || st.draft.is_none() {
        st.original = Some(original.clone());
        st.draft = Spec::of(cx, &original);
    }
    let names: Vec<String> = cx
        .project
        .plan_views
        .iter()
        .map(|v| v.name.clone())
        .collect();
    ui.columns(2, |cols| {
        // Left: the views.
        let ui = &mut cols[0];
        ui.strong("Plan Views");
        egui::ScrollArea::vertical()
            .id_salt("plan_view_list")
            .max_height(300.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for n in &names {
                    let on = *n == original;
                    if ui.selectable_label(on, n).clicked() && !on {
                        st.original = Some(n.clone());
                        st.draft = Spec::of(cx, n);
                    }
                }
            });
        ui.horizontal_wrapped(|ui| {
            if ui.button("New").clicked() {
                let n = new_view(cx);
                st.original = Some(n);
                st.draft = None;
            }
            if ui.button("Duplicate").clicked() {
                if let Some(n) = duplicate_view(cx, &original) {
                    st.original = Some(n);
                    st.draft = None;
                }
            }
            if ui.button("Delete").clicked() {
                st.message = match delete_view(cx, &original) {
                    Ok(()) => {
                        st.original = None;
                        st.draft = None;
                        format!("Deleted {original}")
                    }
                    Err(e) => e,
                };
            }
        });

        // Right: the form.
        let ui = &mut cols[1];
        let Some(draft) = st.draft.as_mut() else {
            return;
        };
        ui.strong("Specification");
        egui::Grid::new("plan_view_form")
            .num_columns(2)
            .spacing(Vec2::new(8.0, 5.0))
            .show(ui, |ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut draft.name).desired_width(200.0));
                ui.end_row();

                ui.label("Layer Set");
                egui::ComboBox::from_id_salt("pv_layer_set")
                    .selected_text(draft.layer_set.clone())
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        for s in cx.project.layer_sets.names() {
                            ui.selectable_value(&mut draft.layer_set, s.to_string(), s);
                        }
                    });
                ui.end_row();

                ui.label("Floor");
                let floor_text = match draft.floor {
                    Some(f) => cx
                        .project
                        .floors
                        .get(f)
                        .map_or_else(|| format!("Floor {}", f + 1), |fl| fl.name.clone()),
                    None => "Current floor".into(),
                };
                egui::ComboBox::from_id_salt("pv_floor")
                    .selected_text(floor_text)
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut draft.floor, None, "Current floor");
                        for (i, f) in cx.project.floors.iter().enumerate() {
                            ui.selectable_value(&mut draft.floor, Some(i), &f.name);
                        }
                    });
                ui.end_row();

                ui.label("Reference Display");
                ui.checkbox(&mut draft.reference_display, "Show the reference floor");
                ui.end_row();

                ui.label("Reference Floor");
                let rf = match draft.reference_floor {
                    None | Some(-1) => "Floor below",
                    Some(1) => "Floor above",
                    Some(_) => "Other floor",
                };
                ui.add_enabled_ui(draft.reference_display, |ui| {
                    egui::ComboBox::from_id_salt("pv_ref_floor")
                        .selected_text(rf)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut draft.reference_floor,
                                Some(-1),
                                "Floor below",
                            );
                            ui.selectable_value(&mut draft.reference_floor, Some(1), "Floor above");
                        });
                });
                ui.end_row();

                ui.label("Dimension Defaults");
                egui::ComboBox::from_id_salt("pv_dim_defaults")
                    .selected_text(if draft.dimension_defaults.is_empty() {
                        "(unchanged)".to_string()
                    } else {
                        draft.dimension_defaults.clone()
                    })
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut draft.dimension_defaults,
                            String::new(),
                            "(unchanged)",
                        );
                        for s in &cx.defaults.dimension_sets {
                            ui.selectable_value(
                                &mut draft.dimension_defaults,
                                s.name.clone(),
                                &s.name,
                            );
                        }
                    });
                ui.end_row();

                ui.label("Text Style");
                egui::ComboBox::from_id_salt("pv_text_style")
                    .selected_text(if draft.text_style.is_empty() {
                        "(unchanged)".to_string()
                    } else {
                        draft.text_style.clone()
                    })
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut draft.text_style, String::new(), "(unchanged)");
                        for s in &cx.project.text_styles.styles {
                            ui.selectable_value(&mut draft.text_style, s.name.clone(), &s.name);
                        }
                    });
                ui.end_row();

                ui.label("Zoom");
                let mut z = draft.zoom.unwrap_or(cx.px_per_in.max(0.05));
                if ui
                    .add(
                        egui::DragValue::new(&mut z)
                            .speed(0.01)
                            .range(0.05..=50.0)
                            .suffix(" px/in"),
                    )
                    .changed()
                {
                    draft.zoom = Some(z);
                }
                ui.end_row();
            });
    });
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        if ui
            .button("Save Plan View")
            .on_hover_text("Store the floor, reference display, zoom and pan the plan shows now in the shown view")
            .clicked()
        {
            plan_tabs::with_tabs(|t| t.save_active(cx));
            st.draft = None;
        }
        if ui
            .button("Reset Plan View")
            .on_hover_text("Show the shown view as it was saved")
            .clicked()
        {
            plan_tabs::with_tabs(|t| t.reset_active(cx));
            st.draft = None;
        }
        if ui.button("Open as Tab").clicked() {
            plan_tabs::with_tabs(|t| t.open_view(cx, &original));
        }
    });
    ui.horizontal(|ui| {
        let mut apply = ui.button("Apply").clicked();
        let ok = ui.button("OK").clicked();
        apply |= ok;
        if apply {
            if let Some(d) = st.draft.clone() {
                match apply_spec(cx, &original, &d) {
                    Ok(()) => {
                        st.original = Some(d.name.trim().to_string());
                        st.draft = None;
                        st.message = "Plan view updated".into();
                        if ok {
                            st.open = false;
                        }
                    }
                    Err(e) => st.message = e,
                }
            }
        }
        if ui.button("Close").clicked() {
            st.open = false;
        }
    });
    if !st.message.is_empty() {
        ui.label(&st.message);
    }
}

/// Used by the Project Browser's tooltip: what a view shows.
pub fn summary(v: &SavedPlanView) -> String {
    format!(
        "Layer set: {}{}{}",
        v.layer_set,
        match v.floor {
            Some(f) => format!("; floor {}", f + 1),
            None => String::new(),
        },
        if v.reference_display {
            "; reference display"
        } else {
            ""
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::layer_sets::DEFAULT_PLAN_VIEW_NAME;

    fn cx() -> EditorContext {
        let mut cx = crate::editor::plan_tabs::plain_cx();
        cx.project.insert_floor_above(0).unwrap();
        cx
    }

    #[test]
    fn the_specification_round_trips_through_the_plan() {
        let mut cx = cx();
        cx.project.layer_sets.copy_set("Default Set", "Dimmed");
        let mut spec = Spec::of(&cx, DEFAULT_PLAN_VIEW_NAME).unwrap();
        assert_eq!(spec.layer_set, "Default Set");
        assert_eq!(spec.floor, None);
        spec.name = "Upstairs Plan".into();
        spec.layer_set = "Dimmed".into();
        spec.floor = Some(1);
        spec.reference_display = true;
        spec.reference_floor = Some(-1);
        spec.dimension_defaults = cx.defaults.dimension_sets[1].name.clone();
        spec.text_style = cx.project.text_styles.styles[1].name.clone();
        spec.zoom = Some(2.5);
        apply_spec(&mut cx, DEFAULT_PLAN_VIEW_NAME, &spec).unwrap();
        assert_eq!(cx.undo_label(), Some("Plan View Specification"));
        // The form reads back what was written.
        assert_eq!(Spec::of(&cx, "Upstairs Plan"), Some(spec.clone()));
        assert!(cx.project.plan_view(DEFAULT_PLAN_VIEW_NAME).is_none());
        // It was the shown view: the plan follows.
        assert_eq!(cx.project.active_plan_view, "Upstairs Plan");
        assert_eq!(cx.project.layer_sets.active, "Dimmed");
        assert_eq!(cx.floor, 1);
        assert_eq!(cx.defaults.active_dimension_set, spec.dimension_defaults);
        // And through the file.
        let back = plan_core::Project::from_json(&cx.project.to_json().unwrap()).unwrap();
        let v = back.plan_view("Upstairs Plan").unwrap();
        assert_eq!(v.camera.map(|c| c.1), Some(2.5));
        assert_eq!(v.text_style, spec.text_style);
        cx.undo();
        assert!(cx.project.plan_view(DEFAULT_PLAN_VIEW_NAME).is_some());
    }

    #[test]
    fn the_specification_is_validated() {
        let mut cx = cx();
        let name = DEFAULT_PLAN_VIEW_NAME;
        let ok = Spec::of(&cx, name).unwrap();
        let bad = |f: &dyn Fn(&mut Spec)| {
            let mut s = ok.clone();
            f(&mut s);
            s
        };
        let before = cx.project.plan_views.clone();
        assert!(apply_spec(&mut cx, "nope", &ok).is_err());
        assert!(apply_spec(&mut cx, name, &bad(&|s| s.name = "  ".into())).is_err());
        assert!(apply_spec(&mut cx, name, &bad(&|s| s.layer_set = "nope".into())).is_err());
        assert!(apply_spec(&mut cx, name, &bad(&|s| s.floor = Some(9))).is_err());
        cx.project
            .add_plan_view(SavedPlanView::new("Taken", "Default Set"));
        assert!(apply_spec(&mut cx, name, &bad(&|s| s.name = "Taken".into())).is_err());
        assert_eq!(
            cx.project.plan_views.len(),
            before.len() + 1,
            "nothing was applied"
        );
        assert!(
            cx.undo_label().is_none(),
            "a refused form leaves no undo step"
        );
    }

    #[test]
    fn views_are_added_duplicated_renamed_moved_and_deleted() {
        let mut cx = cx();
        let new = new_view(&mut cx);
        assert_eq!(cx.project.active_plan_view, new);
        let dup = duplicate_view(&mut cx, &new).unwrap();
        assert_eq!(dup, format!("{new} (2)"));
        assert!(duplicate_view(&mut cx, "nope").is_none());
        assert!(rename_view(&mut cx, &dup, "Renamed").is_ok());
        assert!(rename_view(&mut cx, "Renamed", DEFAULT_PLAN_VIEW_NAME).is_err());
        assert!(rename_view(&mut cx, "Renamed", " ").is_err());
        assert!(rename_view(&mut cx, "zzz", "Q").is_err());
        let order = |cx: &EditorContext| {
            cx.project
                .plan_views
                .iter()
                .map(|v| v.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(order(&cx), vec![DEFAULT_PLAN_VIEW_NAME, &new, "Renamed"]);
        assert!(move_view(&mut cx, 2, 0));
        assert_eq!(order(&cx)[0], "Renamed");
        assert!(!move_view(&mut cx, 0, 0));
        cx.undo();
        assert_eq!(order(&cx)[0], DEFAULT_PLAN_VIEW_NAME);
        // Deleting the shown view shows the first remaining one.
        assert!(delete_view(&mut cx, &new).is_ok());
        assert_eq!(cx.project.active_plan_view, DEFAULT_PLAN_VIEW_NAME);
        assert!(delete_view(&mut cx, "zzz").is_err());
        assert!(delete_view(&mut cx, "Renamed").is_ok());
        assert!(delete_view(&mut cx, DEFAULT_PLAN_VIEW_NAME).is_err());
    }

    #[test]
    fn template_plan_views_are_added_once_and_undone_in_one_step() {
        let mut cx = cx();
        let before = cx.project.plan_views.len();
        assert_eq!(add_template_views(&mut cx), 20);
        assert_eq!(cx.project.plan_views.len(), before + 20);
        assert_eq!(add_template_views(&mut cx), 0);
        cx.undo();
        assert_eq!(cx.project.plan_views.len(), before);
        assert!(run_command(&mut cx, SEED));
        assert!(cx.status.contains("20"));
        assert!(run_command(&mut cx, SEED));
        assert!(cx.status.contains("already"));
    }

    #[test]
    fn save_and_reset_commands_work_on_the_shown_view() {
        let mut cx = cx();
        cx.floor = 1;
        assert!(run_command(&mut cx, SAVE));
        assert_eq!(cx.project.current_plan_view().unwrap().floor, Some(1));
        cx.floor = 0;
        assert!(run_command(&mut cx, RESET));
        assert_eq!(cx.floor, 1);
        assert!(!run_command(&mut cx, "nope"));
        assert!(summary(cx.project.current_plan_view().unwrap()).contains("floor 2"));
    }

    #[test]
    fn the_window_draws() {
        let ctx = egui::Context::default();
        let mut cx = cx();
        run_command(&mut cx, OPEN);
        assert!(is_open());
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| show_all(ctx, &mut cx));
        }
        open_on(DEFAULT_PLAN_VIEW_NAME);
        let _ = ctx.run(egui::RawInput::default(), |ctx| show_all(ctx, &mut cx));
        state(|s| s.open = false);
    }
}
