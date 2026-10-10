//! Plan View Specification (Tools > Plan Views, manual pp. 177 to 181): a
//! saved plan view's General panel (name, Saved, floor with "Use Any Floor",
//! Remember Zoom/Rotation, Show Color, Show Watermark, Link to Layout, Poché,
//! Save Options), its Selected Defaults panel (Default Set, the saved default
//! of each annotation kind, Layer Set, Current CAD Layer) and its Reference
//! Display panel, with New, Duplicate and Delete, the New Saved Plan View
//! dialog (name and Copy Layer Set), and the Save Plan View and Reset Plan
//! View commands.
//!
//! `run_command` opens the window (or runs Save / Reset) from a menu id;
//! `show_all` draws it (the shell calls it from `docks::show_dialogs`).
//! [`view_shown`] and [`view_stored`] are what `editor::plan_tabs` calls when
//! a view is shown and when the view being left is stored.

use super::default_sets::{self, Selected};
use crate::editor::plan_tabs;
use crate::editor::EditorContext;
use crate::toolbar::ViewFlag;
use eframe::egui::{self, Align2, Vec2};
use plan_core::defaults::saved::SavedKind;
use plan_core::defaults::views::{normalize_deg, PlanViewSpec, SaveOption};
use plan_core::fill_styles::PocheView;
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
/// Menu id: Tools > Plan Views > Add Starter Plan Views (the dozen working views).
pub const STARTER: &str = "views.add_starter";
/// Menu id: Tools > Plan Views > New Saved Plan View...
pub const NEW_SAVED: &str = "views.new_saved";
/// Menu id: Tools > Plan Views > Save Active View As...
pub const SAVE_AS: &str = "views.save_as";

thread_local! {
    /// Save Plan View is running (not a view that is being left).
    static EXPLICIT_SAVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Save Plan View: stores the floor, reference display, zoom, rotation and
/// defaults the plan shows now in the shown view (one undo step), whatever
/// its Save Options say.
pub fn save_plan_view(cx: &mut EditorContext) -> bool {
    EXPLICIT_SAVE.with(|c| c.set(true));
    let done = plan_tabs::with_tabs(|t| t.save_active(cx));
    EXPLICIT_SAVE.with(|c| c.set(false));
    done
}

// ----- operations -----

/// The edits of the specification form.
#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    pub name: String,
    /// All views of a plan are saved; kept for the check box.
    pub saved: bool,
    pub layer_set: String,
    /// `None` is "Use Any Floor".
    pub floor: Option<usize>,
    pub remember_zoom_rotation: bool,
    pub show_color: bool,
    pub show_watermark: bool,
    pub link_to_layout: bool,
    pub poche: bool,
    pub save_option: SaveOption,
    pub reference_display: bool,
    /// Floor shown as reference, relative to the viewed floor.
    pub reference_floor: Option<i32>,
    pub dimension_defaults: String,
    pub text_style: String,
    /// The Selected Defaults panel: Default Set, picks and CAD layer.
    pub default_set: String,
    pub picks: std::collections::BTreeMap<String, String>,
    pub cad_layer: String,
    /// Zoom in screen pixels per plan inch; `None` keeps the camera as saved.
    pub zoom: Option<f64>,
    /// Rotation in degrees, -180 to 180.
    pub rotation_deg: f64,
}

impl Spec {
    /// The form of view `name`.
    pub fn of(cx: &EditorContext, name: &str) -> Option<Spec> {
        let v = cx.project.plan_view(name)?;
        Some(Spec {
            name: v.name.clone(),
            saved: v.spec.saved,
            layer_set: v.layer_set.clone(),
            floor: v.floor,
            remember_zoom_rotation: v.spec.remember_zoom_rotation,
            show_color: v.spec.show_color,
            show_watermark: cx
                .project
                .print_setup
                .watermark
                .is_on(&plan_core::watermark::plan_key(&v.name)),
            link_to_layout: v.spec.link_to_layout,
            poche: cx.project.styles.poche.is_on(&v.name, PocheView::Plan),
            save_option: v.spec.save_option,
            reference_display: v.reference_display,
            reference_floor: v.reference_floor,
            dimension_defaults: v.dimension_defaults.clone(),
            text_style: v.text_style.clone(),
            default_set: v.spec.default_set.clone(),
            picks: v.spec.selected.clone(),
            cad_layer: v.spec.cad_layer.clone(),
            zoom: v.camera.map(|c| c.1),
            rotation_deg: v.spec.rotation_deg,
        })
    }

    fn selected(&self) -> Selected {
        Selected {
            default_set: self.default_set.clone(),
            picks: self.picks.clone(),
            layer_set: self.layer_set.clone(),
            cad_layer: self.cad_layer.clone(),
        }
    }

    fn take_selected(&mut self, s: Selected) {
        self.default_set = s.default_set;
        self.picks = s.picks;
        self.layer_set = s.layer_set;
        self.cad_layer = s.cad_layer;
        // The view's dimension defaults follow its Manual Dimensions pick.
        if let Some(d) = self.picks.get(SavedKind::ManualDimensions.id()) {
            self.dimension_defaults = d.clone();
        }
    }
}

/// OK / Apply: writes `spec` into the plan view `original` (one undo step).
/// The name may change while it stays unique; the layer set must exist; the
/// floor must exist; the Default Set and saved defaults it names must exist.
/// When the view is the one shown, it is shown again so the plan follows the
/// new floor, reference display, defaults and rotation.
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
    if !spec.default_set.is_empty() && cx.project.saved_defaults.set(&spec.default_set).is_none() {
        return Err(format!("There is no Default Set \"{}\"", spec.default_set));
    }
    for (kid, saved) in &spec.picks {
        let Some(kind) = SavedKind::from_id(kid) else {
            continue;
        };
        if !cx
            .project
            .saved_names(&cx.defaults, kind)
            .iter()
            .any(|n| n == saved)
        {
            return Err(format!("There is no saved default \"{saved}\""));
        }
    }
    cx.begin_change("Plan View Specification");
    cx.project.rename_plan_view(original, name);
    if name != original {
        // The switches kept by view name follow the rename.
        let wm = &mut cx.project.print_setup.watermark;
        let was_on = wm.is_on(&plan_core::watermark::plan_key(original));
        wm.set_on(&plan_core::watermark::plan_key(original), false);
        wm.set_on(&plan_core::watermark::plan_key(name), was_on);
        let p = &mut cx.project.styles.poche;
        for (n, _) in p.views.iter_mut().filter(|(n, _)| n == original) {
            *n = name.to_string();
        }
    }
    cx.project
        .print_setup
        .watermark
        .set_on(&plan_core::watermark::plan_key(name), spec.show_watermark);
    cx.project.styles.poche.set(name, spec.poche);
    if let Some(v) = cx.project.plan_views.iter_mut().find(|v| v.name == name) {
        v.layer_set = spec.layer_set.clone();
        v.floor = spec.floor;
        v.reference_display = spec.reference_display;
        v.reference_floor = spec.reference_floor;
        v.dimension_defaults = spec.dimension_defaults.clone();
        v.text_style = spec.text_style.clone();
        v.spec = PlanViewSpec {
            saved: true,
            remember_zoom_rotation: spec.remember_zoom_rotation,
            rotation_deg: normalize_deg(spec.rotation_deg),
            show_color: spec.show_color,
            link_to_layout: spec.link_to_layout,
            save_option: spec.save_option,
            default_set: spec.default_set.clone(),
            selected: spec.picks.clone(),
            cad_layer: spec.cad_layer.clone(),
            dirty: false,
        };
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

/// Called by `EditorContext::show_plan_view` when view `view` is shown: the
/// Show Color switch, the Selected Defaults of the view (its Default Set or
/// picks and CAD layer) and, for a view that remembers it, its rotation.
/// Switching defaults is navigation, not an undo step.
pub fn view_shown(cx: &mut EditorContext, view: &SavedPlanView) {
    if view.spec.show_color {
        cx.view_flags.insert(ViewFlag::Color);
    } else {
        cx.view_flags.remove(&ViewFlag::Color);
    }
    cx.project.view_apply_defaults(&mut cx.defaults, &view.name);
    // The view's own layer set wins over the set a Default Set brought.
    if cx.project.layer_sets.get(&view.layer_set).is_some() {
        cx.project.show_layer_set(&view.layer_set);
    }
    crate::shell::view_commands::request_rotation(if view.spec.remember_zoom_rotation {
        view.spec.rotation_deg
    } else {
        0.0
    });
}

/// Called by `EditorContext::store_active` for the view being left or saved:
/// keeps the defaults in force and, when it remembers it, the rotation shown.
/// A view set to Never Save keeps them only when saved on purpose (Save
/// Plan View).
pub fn view_stored(cx: &mut EditorContext, name: &str) {
    let Some(v) = cx.project.plan_view(name) else {
        return;
    };
    let explicit = EXPLICIT_SAVE.with(std::cell::Cell::get);
    if v.spec.save_option == SaveOption::Never && !explicit {
        return;
    }
    let remember = v.spec.remember_zoom_rotation;
    cx.project.view_capture_defaults(&mut cx.defaults, name);
    if remember {
        let rot = crate::shell::view_commands::reported_rotation_deg();
        if let Some(v) = cx.project.plan_views.iter_mut().find(|v| v.name == name) {
            v.spec.rotation_deg = rot;
        }
    }
}

/// New: a view that starts as a copy of the active one, named "Plan View N";
/// it becomes a tab and the shown view. Returns its name.
pub fn new_view(cx: &mut EditorContext) -> String {
    let name = crate::dialogs::app_info::new_plan_view(cx);
    plan_tabs::with_tabs(|t| t.open_view(cx, &name));
    name
}

/// The New Saved Plan View dialog's OK: a saved view named `name` with the
/// attributes of `from` (the floor, reference display, zoom, defaults and
/// rotation of the view as it is now). With `copy_layer_set` the new view
/// gets a copy of the original's layer set under that name; without, it
/// shares the original's. One undo step; the view is shown. Returns its name.
pub fn create_saved_view(
    cx: &mut EditorContext,
    from: &str,
    name: &str,
    copy_layer_set: Option<&str>,
) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Type a name for the plan view".into());
    }
    if cx.project.plan_view(name).is_some() {
        return Err(format!("A plan view named \"{name}\" exists already"));
    }
    let base = cx
        .project
        .plan_view(from)
        .cloned()
        .ok_or_else(|| format!("There is no plan view \"{from}\""))?;
    let set_name = copy_layer_set.map(str::trim);
    if let Some(n) = set_name {
        if n.is_empty() {
            return Err("Type a name for the new layer set".into());
        }
        if cx.project.layer_sets.get(n).is_some() {
            return Err(format!("A layer set named \"{n}\" exists already"));
        }
    }
    cx.begin_change("New Saved Plan View");
    let mut view = base.clone();
    view.name = name.to_string();
    if let Some(n) = set_name {
        let src = base.layer_set.clone();
        if !cx.project.layer_sets.copy_set(&src, n) {
            cx.cancel_change();
            return Err("The layer set could not be copied".into());
        }
        view.layer_set = n.to_string();
    }
    view.spec.dirty = false;
    // The switches kept by view name start as the original's.
    let was_wm = cx
        .project
        .print_setup
        .watermark
        .is_on(&plan_core::watermark::plan_key(from));
    cx.project
        .print_setup
        .watermark
        .set_on(&plan_core::watermark::plan_key(name), was_wm);
    let was_poche = cx.project.styles.poche.is_on(from, PocheView::Plan);
    cx.project.styles.poche.set(name, was_poche);
    cx.project.plan_views.push(view);
    cx.mark_dirty();
    // The new view starts with the defaults in force now.
    if cx.project.active_plan_view == from {
        cx.project.view_capture_defaults(&mut cx.defaults, name);
    }
    plan_tabs::with_tabs(|t| t.open_view(cx, name));
    cx.status = format!("Made the saved plan view \"{name}\"");
    Ok(name.to_string())
}

/// Duplicate: copies `name` under a free name, at once (the scripted form;
/// the Project Browser asks with [`request_duplicate`]). Returns the new name.
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

/// Adds the dozen starter views a Residential template carries (one undo
/// step); returns how many were new.
pub fn add_starter_views(cx: &mut EditorContext) -> usize {
    let mut probe = cx.project.clone();
    if probe.seed_starter_plan_views() == 0 {
        return 0;
    }
    cx.begin_change("Add Starter Plan Views");
    let n = cx.project.seed_starter_plan_views();
    cx.mark_dirty();
    n
}

// ----- the window -----

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    General,
    SelectedDefaults,
    ReferenceDisplay,
}

/// The New Saved Plan View dialog.
#[derive(Clone)]
struct NewDialog {
    from: String,
    name: String,
    copy_layer_set: bool,
    layer_set_name: String,
    error: String,
}

#[derive(Default)]
struct State {
    open: bool,
    /// The view being edited (its name when the window last synced).
    original: Option<String>,
    draft: Option<Spec>,
    message: String,
    tab: Tab,
    new_dialog: Option<NewDialog>,
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

/// Opens the New Saved Plan View dialog for a copy of `from`.
pub fn open_new_saved(cx: &EditorContext, from: &str) {
    let mut n = cx.project.plan_views.len() + 1;
    let name = loop {
        let c = format!("Plan View {n}");
        if cx.project.plan_view(&c).is_none() {
            break c;
        }
        n += 1;
    };
    let layer_set_name = cx
        .project
        .plan_view(from)
        .map_or_else(String::new, |v| format!("{} Copy", v.layer_set));
    state(|s| {
        s.new_dialog = Some(NewDialog {
            from: from.to_string(),
            name,
            copy_layer_set: false,
            layer_set_name,
            error: String::new(),
        })
    });
}

/// Duplicate from the Project Browser: the New Saved Plan View dialog opens
/// for a copy of `name`.
pub fn request_duplicate(cx: &EditorContext, name: &str) {
    open_new_saved(cx, name);
    state(|s| {
        if let Some(d) = s.new_dialog.as_mut() {
            d.name = format!("{name} (2)");
        }
    });
}

/// Is the New Saved Plan View dialog open?
pub fn new_dialog_open() -> bool {
    state(|s| s.new_dialog.is_some())
}

/// Types the name (and the layer set copy) into the open New Saved Plan View
/// dialog and answers OK. Returns the error text, if any.
pub fn answer_new_dialog(
    cx: &mut EditorContext,
    name: &str,
    copy_layer_set: Option<&str>,
) -> Result<String, String> {
    let Some(d) = state(|s| s.new_dialog.take()) else {
        return Err("No dialog".into());
    };
    match create_saved_view(cx, &d.from, name, copy_layer_set) {
        Ok(n) => Ok(n),
        Err(e) => {
            state(|s| {
                s.new_dialog = Some(NewDialog {
                    error: e.clone(),
                    ..d
                })
            });
            Err(e)
        }
    }
}

/// Runs a menu command by id; false when the id is not ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        OPEN => open(),
        SAVE => {
            save_plan_view(cx);
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
        STARTER => {
            let n = add_starter_views(cx);
            cx.status = if n == 0 {
                "The starter plan views are already in this plan".into()
            } else {
                format!("Added {n} starter plan views")
            };
        }
        NEW_SAVED | SAVE_AS => {
            // Save Active View As makes a copy of the current view; so does
            // New Saved Plan View.
            let from = cx.project.active_plan_view.clone();
            open_new_saved(cx, &from);
        }
        _ => return false,
    }
    true
}

/// Draws the windows when they are open.
pub fn show_all(ctx: &egui::Context, cx: &mut EditorContext) {
    show_new_dialog(ctx, cx);
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
            .default_size(Vec2::new(640.0, 520.0))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| body(ui, cx, &mut st));
    });
    if !open {
        state(|s| s.open = false);
    }
}

fn show_new_dialog(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut d) = state(|s| s.new_dialog.take()) else {
        return;
    };
    let mut ok = false;
    let mut cancel = false;
    egui::Window::new("New Saved Plan View")
        .id(egui::Id::new("new_saved_plan_view"))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name");
                let r = ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(240.0));
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    ok = true;
                }
            });
            ui.checkbox(&mut d.copy_layer_set, "Copy Layer Set");
            ui.add_enabled_ui(d.copy_layer_set, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Layer Set Name");
                    ui.add(egui::TextEdit::singleline(&mut d.layer_set_name).desired_width(200.0));
                });
            });
            if !d.error.is_empty() {
                ui.colored_label(egui::Color32::from_rgb(0xE0, 0x4B, 0x4B), &d.error);
            }
            ui.horizontal(|ui| {
                if ui.button("   OK   ").clicked() {
                    ok = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        cancel = true;
    }
    if ok {
        let set = d.copy_layer_set.then(|| d.layer_set_name.clone());
        match create_saved_view(cx, &d.from, &d.name, set.as_deref()) {
            Ok(_) => {
                state(|s| {
                    s.original = None;
                    s.draft = None;
                });
            }
            Err(e) => {
                d.error = e;
                state(|s| s.new_dialog = Some(d));
            }
        }
    } else if !cancel {
        state(|s| s.new_dialog = Some(d));
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
    let env = default_sets::Env::of(cx);
    let mut events = Vec::new();
    let mut open_new = false;
    ui.columns(2, |cols| {
        // Left: the views.
        let ui = &mut cols[0];
        ui.strong("Plan Views");
        egui::ScrollArea::vertical()
            .id_salt("plan_view_list")
            .max_height(360.0)
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
                open_new = true;
            }
            if ui.button("Duplicate").clicked() {
                request_duplicate(cx, &original);
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
        ui.horizontal(|ui| {
            ui.selectable_value(&mut st.tab, Tab::General, "General");
            ui.selectable_value(&mut st.tab, Tab::SelectedDefaults, "Selected Defaults");
            ui.selectable_value(&mut st.tab, Tab::ReferenceDisplay, "Reference Display");
        });
        ui.separator();
        match st.tab {
            Tab::General => general_panel(ui, cx, draft),
            Tab::SelectedDefaults => {
                let mut sel = draft.selected();
                events = sel.panel(ui, &env, true, "pv_selected");
                draft.take_selected(sel);
            }
            Tab::ReferenceDisplay => reference_panel(ui, cx, draft),
        }
    });
    if open_new {
        open_new_saved(cx, &original);
    }
    // Add, Edit, Rename and Delete act on the plan's lists at once.
    if let Some(draft) = st.draft.as_mut() {
        let mut sel = draft.selected();
        for ev in events {
            st.message = handle_event(cx, ev, &mut sel).unwrap_or_default();
        }
        draft.take_selected(sel);
    }
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        if ui
            .button("Save Plan View")
            .on_hover_text("Store the floor, reference display, zoom, rotation and defaults the plan shows now in the shown view")
            .clicked()
        {
            save_plan_view(cx);
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

fn handle_event(
    cx: &mut EditorContext,
    ev: default_sets::Ev,
    sel: &mut Selected,
) -> Option<String> {
    default_sets::handle_event(cx, ev, sel)
}

fn general_panel(ui: &mut egui::Ui, cx: &EditorContext, draft: &mut Spec) {
    egui::Grid::new("plan_view_form")
        .num_columns(2)
        .spacing(Vec2::new(8.0, 5.0))
        .show(ui, |ui| {
            ui.label("Name");
            ui.add(egui::TextEdit::singleline(&mut draft.name).desired_width(200.0));
            ui.end_row();

            ui.label("Saved");
            ui.add_enabled(
                false,
                egui::Checkbox::new(&mut draft.saved, "A saved view stays saved"),
            );
            ui.end_row();

            ui.label("Floor");
            let floor_text = match draft.floor {
                Some(f) => cx
                    .project
                    .floors
                    .get(f)
                    .map_or_else(|| format!("Floor {}", f + 1), |fl| fl.name.clone()),
                None => "Use Any Floor".into(),
            };
            egui::ComboBox::from_id_salt("pv_floor")
                .selected_text(floor_text)
                .width(200.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut draft.floor, None, "Use Any Floor");
                    for (i, f) in cx.project.floors.iter().enumerate() {
                        ui.selectable_value(&mut draft.floor, Some(i), &f.name);
                    }
                });
            ui.end_row();

            ui.label("");
            ui.checkbox(&mut draft.remember_zoom_rotation, "Remember Zoom/Rotation");
            ui.end_row();
            ui.label("Rotation");
            let mut rot = draft.rotation_deg;
            if ui
                .add_enabled(
                    draft.remember_zoom_rotation,
                    egui::DragValue::new(&mut rot)
                        .speed(1.0)
                        .range(-180.0..=180.0)
                        .suffix("\u{b0}"),
                )
                .changed()
            {
                draft.rotation_deg = normalize_deg(rot);
            }
            ui.end_row();
            ui.label("Zoom");
            let mut z = draft.zoom.unwrap_or(cx.px_per_in.max(0.05));
            if ui
                .add_enabled(
                    draft.remember_zoom_rotation,
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
            ui.label("");
            ui.checkbox(&mut draft.show_color, "Show Color");
            ui.end_row();
            ui.label("");
            ui.checkbox(&mut draft.show_watermark, "Show Watermark");
            ui.end_row();
            ui.label("");
            ui.checkbox(&mut draft.link_to_layout, "Link to Layout");
            ui.end_row();
        });
    ui.add_space(4.0);
    ui.strong("Wall Display Options");
    ui.checkbox(&mut draft.poche, "Poch\u{e9}");
    ui.add_space(4.0);
    ui.strong("Save Options");
    for o in SaveOption::ALL {
        ui.radio_value(&mut draft.save_option, o, o.label());
    }
}

fn reference_panel(ui: &mut egui::Ui, cx: &EditorContext, draft: &mut Spec) {
    let _ = cx;
    egui::Grid::new("plan_view_reference")
        .num_columns(2)
        .spacing(Vec2::new(8.0, 5.0))
        .show(ui, |ui| {
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
                        ui.selectable_value(&mut draft.reference_floor, Some(-1), "Floor below");
                        ui.selectable_value(&mut draft.reference_floor, Some(1), "Floor above");
                    });
            });
            ui.end_row();
        });
}

/// Used by the Project Browser's tooltip: what a view shows.
pub fn summary(v: &SavedPlanView) -> String {
    format!(
        "Layer set: {}{}{}{}",
        v.layer_set,
        match v.floor {
            Some(f) => format!("; floor {}", f + 1),
            None => String::new(),
        },
        if v.reference_display {
            "; reference display"
        } else {
            ""
        },
        if v.spec.default_set.is_empty() {
            String::new()
        } else {
            format!("; {}", v.spec.default_set)
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

    #[test]
    fn the_specification_keeps_the_new_general_and_defaults_fields() {
        let mut cx = cx();
        cx.project
            .saved_copy(&mut cx.defaults, SavedKind::RichText, "Default", "Plot")
            .unwrap();
        cx.project
            .default_set_save_new(&mut cx.defaults, "Plot Set")
            .unwrap();
        let mut spec = Spec::of(&cx, DEFAULT_PLAN_VIEW_NAME).unwrap();
        assert!(
            spec.saved && spec.remember_zoom_rotation && spec.show_color && spec.link_to_layout
        );
        assert_eq!(spec.save_option, SaveOption::Prompt);
        spec.show_color = false;
        spec.show_watermark = true;
        spec.link_to_layout = false;
        spec.poche = true;
        spec.save_option = SaveOption::Always;
        spec.rotation_deg = 270.0;
        spec.default_set = "Plot Set".into();
        spec.picks.insert("rich_text".into(), "Plot".into());
        apply_spec(&mut cx, DEFAULT_PLAN_VIEW_NAME, &spec).unwrap();
        let back = Spec::of(&cx, DEFAULT_PLAN_VIEW_NAME).unwrap();
        assert_eq!(back.rotation_deg, -90.0, "270 reads -90");
        assert!(!back.show_color && back.show_watermark && back.poche && !back.link_to_layout);
        assert_eq!(back.save_option, SaveOption::Always);
        assert_eq!(back.default_set, "Plot Set");
        // The switches really are the ones the plan reads.
        assert!(cx
            .project
            .print_setup
            .watermark
            .is_on(&plan_core::watermark::plan_key(DEFAULT_PLAN_VIEW_NAME)));
        assert!(cx
            .project
            .styles
            .poche
            .is_on(DEFAULT_PLAN_VIEW_NAME, PocheView::Plan));
        assert!(
            !cx.view_flags.contains(&ViewFlag::Color),
            "shown again: color off"
        );
        // A Default Set or a saved default that does not exist is refused.
        let mut bad = back.clone();
        bad.default_set = "Nope".into();
        assert!(apply_spec(&mut cx, DEFAULT_PLAN_VIEW_NAME, &bad).is_err());
        let mut bad = back.clone();
        bad.picks.insert("markers".into(), "Nope".into());
        assert!(apply_spec(&mut cx, DEFAULT_PLAN_VIEW_NAME, &bad).is_err());
        // A rename carries the switches.
        let mut ren = back.clone();
        ren.name = "Renamed".into();
        apply_spec(&mut cx, DEFAULT_PLAN_VIEW_NAME, &ren).unwrap();
        let r = Spec::of(&cx, "Renamed").unwrap();
        assert!(r.show_watermark && r.poche);
        assert!(!cx
            .project
            .print_setup
            .watermark
            .is_on(&plan_core::watermark::plan_key(DEFAULT_PLAN_VIEW_NAME)));
    }

    #[test]
    fn new_saved_plan_view_copies_the_layer_set_only_when_asked() {
        let mut cx = cx();
        let sets = cx.project.layer_sets.sets.len();
        // Shares the layer set.
        let a = create_saved_view(&mut cx, DEFAULT_PLAN_VIEW_NAME, "Shares", None).unwrap();
        assert_eq!(cx.project.plan_view(&a).unwrap().layer_set, "Default Set");
        assert_eq!(cx.project.layer_sets.sets.len(), sets);
        assert_eq!(cx.undo_label(), Some("New Saved Plan View"));
        assert_eq!(
            cx.project.active_plan_view, "Shares",
            "the new view is shown"
        );
        // Copies it.
        let b = create_saved_view(&mut cx, DEFAULT_PLAN_VIEW_NAME, "Copies", Some("My Layers"))
            .unwrap();
        assert_eq!(cx.project.plan_view(&b).unwrap().layer_set, "My Layers");
        assert_eq!(cx.project.layer_sets.sets.len(), sets + 1);
        // Names are checked.
        assert!(create_saved_view(&mut cx, DEFAULT_PLAN_VIEW_NAME, "Shares", None).is_err());
        assert!(create_saved_view(&mut cx, DEFAULT_PLAN_VIEW_NAME, " ", None).is_err());
        assert!(
            create_saved_view(&mut cx, DEFAULT_PLAN_VIEW_NAME, "X", Some("My Layers")).is_err()
        );
        assert!(create_saved_view(&mut cx, DEFAULT_PLAN_VIEW_NAME, "X", Some("")).is_err());
        assert!(create_saved_view(&mut cx, "Nope", "X", None).is_err());
        cx.undo();
        assert!(
            cx.project.plan_view("Copies").is_none()
                && cx.project.layer_sets.get("My Layers").is_none()
        );
    }

    #[test]
    fn the_new_view_dialog_opens_from_the_menu_and_from_duplicate() {
        let mut cx = cx();
        state(|s| s.new_dialog = None);
        assert!(run_command(&mut cx, NEW_SAVED));
        assert!(new_dialog_open());
        assert!(answer_new_dialog(&mut cx, "", None).is_err());
        assert!(new_dialog_open(), "an error keeps the dialog open");
        assert_eq!(answer_new_dialog(&mut cx, "Mine", None).unwrap(), "Mine");
        assert!(!new_dialog_open());
        request_duplicate(&cx, "Mine");
        assert!(new_dialog_open());
        assert_eq!(
            answer_new_dialog(&mut cx, "Mine (2)", Some("Mine Layers")).unwrap(),
            "Mine (2)"
        );
        let ctx = egui::Context::default();
        request_duplicate(&cx, "Mine");
        let _ = ctx.run(egui::RawInput::default(), |ctx| show_all(ctx, &mut cx));
        state(|s| s.new_dialog = None);
    }

    #[test]
    fn the_starter_views_are_a_dozen_and_added_once() {
        let mut cx = cx();
        assert_eq!(add_starter_views(&mut cx), 12);
        assert_eq!(add_starter_views(&mut cx), 0);
        assert!(cx.project.plan_view("Working Plan View").is_some());
        cx.undo();
        assert!(cx.project.plan_view("Working Plan View").is_none());
    }

    #[test]
    fn showing_a_view_brings_its_defaults_color_and_rotation_and_saving_stores_them() {
        let mut cx = cx();
        crate::shell::view_commands::take_pending_rotation();
        cx.project
            .saved_copy(&mut cx.defaults, SavedKind::RichText, "Default", "Plot")
            .unwrap();
        let mut spec = Spec::of(&cx, DEFAULT_PLAN_VIEW_NAME).unwrap();
        spec.rotation_deg = 33.0;
        spec.show_color = false;
        spec.picks.insert("rich_text".into(), "Plot".into());
        apply_spec(&mut cx, DEFAULT_PLAN_VIEW_NAME, &spec).unwrap();
        // Leave and come back: the view brings everything back.
        cx.project
            .saved_activate(&mut cx.defaults, SavedKind::RichText, "Default");
        cx.view_flags.insert(ViewFlag::Color);
        crate::shell::view_commands::take_pending_rotation();
        cx.show_plan_view(DEFAULT_PLAN_VIEW_NAME).unwrap();
        assert_eq!(
            cx.project.saved_active(&cx.defaults, SavedKind::RichText),
            "Plot"
        );
        assert!(!cx.view_flags.contains(&ViewFlag::Color));
        let rot = crate::shell::view_commands::take_pending_rotation().unwrap();
        assert!((rot.to_degrees() - 33.0).abs() < 1e-9);
        // The rotation the shell reports is what Save Plan View keeps.
        crate::shell::view_commands::report_rotation(1.0_f64.to_radians() * 45.0);
        cx.project
            .saved_activate(&mut cx.defaults, SavedKind::RichText, "Default");
        assert!(run_command(&mut cx, SAVE));
        let v = cx.project.plan_view(DEFAULT_PLAN_VIEW_NAME).unwrap();
        assert!((v.spec.rotation_deg - 45.0).abs() < 1e-9);
        assert_eq!(
            v.spec.selected.get("rich_text").map(String::as_str),
            Some("Default")
        );
        // A view that does not remember its rotation opens north up.
        let mut spec = Spec::of(&cx, DEFAULT_PLAN_VIEW_NAME).unwrap();
        spec.remember_zoom_rotation = false;
        apply_spec(&mut cx, DEFAULT_PLAN_VIEW_NAME, &spec).unwrap();
        crate::shell::view_commands::take_pending_rotation();
        cx.show_plan_view(DEFAULT_PLAN_VIEW_NAME).unwrap();
        assert_eq!(
            crate::shell::view_commands::take_pending_rotation(),
            Some(0.0)
        );
    }

    #[test]
    fn a_view_set_to_never_save_keeps_its_stored_values_on_a_switch_but_not_on_a_save() {
        let mut cx = cx();
        let mut spec = Spec::of(&cx, DEFAULT_PLAN_VIEW_NAME).unwrap();
        spec.save_option = SaveOption::Never;
        apply_spec(&mut cx, DEFAULT_PLAN_VIEW_NAME, &spec).unwrap();
        crate::shell::view_commands::report_rotation(1.0);
        view_stored(&mut cx, DEFAULT_PLAN_VIEW_NAME);
        assert_eq!(
            cx.project
                .plan_view(DEFAULT_PLAN_VIEW_NAME)
                .unwrap()
                .spec
                .rotation_deg,
            0.0
        );
        assert!(run_command(&mut cx, SAVE));
        assert!(
            cx.project
                .plan_view(DEFAULT_PLAN_VIEW_NAME)
                .unwrap()
                .spec
                .rotation_deg
                != 0.0
        );
    }
}
