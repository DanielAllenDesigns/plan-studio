//! Manage Custom Schedule Categories (Tools > Schedules, manual pp. 710 to
//! 711): create, rename and delete the plan's custom schedule categories and
//! put the selected objects in them. A schedule lists a custom category's
//! objects when it ticks the category on the General panel of its
//! specification; an object joins a category on its own Schedule panel
//! ("Include in Schedule As"), or here with *Add Selection*.
//!
//! The categories live in `Project::schedule_setup`. Each command is one
//! undo step.

use crate::editor::schedule_view::{self, ident_of};
use crate::editor::EditorContext;
use eframe::egui;
use plan_core::schedules::{custom_category_id, ScheduleLayer, ScheduleSetup};
use plan_docs::schedule_kinds;
use std::cell::RefCell;

#[derive(Default)]
struct State {
    open: bool,
    selected: Option<String>,
    new_name: String,
    rename: String,
    message: String,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Opens the dialog.
pub fn open() {
    STATE.with(|s| s.borrow_mut().open = true);
}

/// Is the dialog up?
pub fn is_open() -> bool {
    STATE.with(|s| s.borrow().open)
}

/// The keys of the selected objects, as the categories store them.
pub fn selection_keys(cx: &EditorContext) -> Vec<String> {
    cx.selection
        .items
        .iter()
        .filter_map(|o| {
            let i = ident_of(cx, *o)?;
            schedule_kinds::prop_key(
                i.kind,
                i.floor,
                i.id,
                i.position.unwrap_or(plan_core::geometry::Point::ZERO),
            )
            .map(|k| k.0)
        })
        .collect()
}

/// New Category: one undo step. The name must be new and not empty.
pub fn add_category(cx: &mut EditorContext, name: &str) -> Result<(), String> {
    let mut probe = cx.project.schedule_setup.clone();
    probe.add_category(name)?;
    cx.begin_change("New Schedule Category");
    cx.project.schedule_setup = probe;
    cx.mark_dirty();
    Ok(())
}

/// Rename: the schedules that tick the category follow.
pub fn rename_category(cx: &mut EditorContext, old: &str, new: &str) -> Result<(), String> {
    let mut probe = cx.project.schedule_setup.clone();
    probe.rename_category(old, new)?;
    let new = new.trim().to_string();
    cx.begin_change("Rename Schedule Category");
    cx.project.schedule_setup = probe;
    for d in &mut cx.project.schedule_setup.defaults {
        ScheduleSetup::rename_in(d, old, &new);
    }
    for p in cx.project.props.pages.values_mut() {
        if let Some(s) = p.schedule.as_mut() {
            for c in &mut s.categories {
                if c == old {
                    *c = new.clone();
                }
            }
        }
    }
    for fi in 0..cx.project.floors.len() {
        let mut layer = ScheduleLayer::load(&cx.project.floors[fi]);
        for s in &mut layer.schedules {
            ScheduleSetup::rename_in(s, old, &new);
        }
        schedule_view::save(&mut cx.project, fi, &layer);
    }
    cx.mark_dirty();
    Ok(())
}

/// Delete: the category goes, and every schedule forgets that it ticked it.
pub fn delete_category(cx: &mut EditorContext, name: &str) -> bool {
    if cx.project.schedule_setup.category(name).is_none() {
        return false;
    }
    cx.begin_change("Delete Schedule Category");
    cx.project.schedule_setup.delete_category(name);
    let id = custom_category_id(name);
    for d in &mut cx.project.schedule_setup.defaults {
        d.categories.remove(&id);
    }
    for p in cx.project.props.pages.values_mut() {
        if let Some(s) = p.schedule.as_mut() {
            s.categories.retain(|c| c != name);
        }
    }
    for fi in 0..cx.project.floors.len() {
        let mut layer = ScheduleLayer::load(&cx.project.floors[fi]);
        for s in &mut layer.schedules {
            s.categories.remove(&id);
        }
        schedule_view::save(&mut cx.project, fi, &layer);
    }
    cx.mark_dirty();
    true
}

/// Puts object `key` in (or takes it out of) custom category `name` on its
/// Schedule panel (Include in Schedule As). True when something changed.
fn set_membership(project: &mut plan_core::Project, key: &str, name: &str, member: bool) -> bool {
    let mut pages = project.props.pages_of(key).cloned().unwrap_or_default();
    let mut sched = pages.schedule.take().unwrap_or_default();
    let has = sched.categories.iter().any(|c| c == name);
    let changed = match (member, has) {
        (true, false) => {
            sched.categories.push(name.to_string());
            true
        }
        (false, true) => {
            sched.categories.retain(|c| c != name);
            true
        }
        _ => false,
    };
    // The category the Manage dialog's own list holds counts as well.
    let in_setup = if member {
        false
    } else {
        project.schedule_setup.unassign(name, key)
    };
    pages.schedule = Some(sched);
    if changed {
        project.props.set_pages(key, pages);
    }
    changed || in_setup
}

/// Add Selection: the selected objects join the category (their Schedule
/// panel lists it).
pub fn assign_selection(cx: &mut EditorContext, name: &str) -> usize {
    let keys = selection_keys(cx);
    if keys.is_empty() || cx.project.schedule_setup.category(name).is_none() {
        return 0;
    }
    cx.begin_change("Assign Schedule Category");
    let n = keys
        .iter()
        .filter(|k| set_membership(&mut cx.project, k, name, true))
        .count();
    cx.mark_dirty();
    n
}

/// Remove Selection: the selected objects leave the category.
pub fn unassign_selection(cx: &mut EditorContext, name: &str) -> usize {
    let keys = selection_keys(cx);
    if keys.is_empty() {
        return 0;
    }
    cx.begin_change("Unassign Schedule Category");
    let n = keys
        .iter()
        .filter(|k| set_membership(&mut cx.project, k, name, false))
        .count();
    cx.mark_dirty();
    n
}

/// How many objects are in category `name`.
pub fn member_count(project: &plan_core::Project, name: &str) -> usize {
    let mut keys: std::collections::BTreeSet<String> = project
        .schedule_setup
        .category(name)
        .map(|c| c.members.iter().cloned().collect())
        .unwrap_or_default();
    for (k, p) in &project.props.pages {
        if p.schedule
            .as_ref()
            .is_some_and(|s| s.categories.iter().any(|c| c == name))
        {
            keys.insert(k.clone());
        }
    }
    keys.len()
}

/// Draws the dialog when it is up.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    let mut st = STATE.with(|s| std::mem::take(&mut *s.borrow_mut()));
    if !st.open {
        STATE.with(|s| *s.borrow_mut() = st);
        return;
    }
    let mut open = true;
    let names: Vec<(String, usize)> = cx
        .project
        .schedule_setup
        .categories
        .iter()
        .map(|c| (c.name.clone(), member_count(&cx.project, &c.name)))
        .collect();
    egui::Window::new("Manage Custom Schedule Categories")
        .id(egui::Id::new("manage_schedule_categories"))
        .open(&mut open)
        .collapsible(false)
        .default_width(380.0)
        .show(ctx, |ui| {
            ui.label("Custom categories list objects in the schedules that tick them, whatever their type.");
            ui.add_space(4.0);
            egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                if names.is_empty() {
                    ui.weak("No custom categories yet");
                }
                for (n, count) in &names {
                    let sel = st.selected.as_deref() == Some(n.as_str());
                    if ui
                        .selectable_label(sel, format!("{n}   ({count} objects)"))
                        .clicked()
                    {
                        st.selected = Some(n.clone());
                        st.rename = n.clone();
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut st.new_name);
                if ui.button("New").clicked() {
                    match add_category(cx, &st.new_name) {
                        Ok(()) => {
                            st.selected = Some(st.new_name.trim().to_string());
                            st.rename = st.new_name.trim().to_string();
                            st.new_name.clear();
                            st.message.clear();
                        }
                        Err(e) => st.message = e,
                    }
                }
            });
            if let Some(sel) = st.selected.clone() {
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut st.rename);
                    if ui.button("Rename").clicked() {
                        match rename_category(cx, &sel, &st.rename) {
                            Ok(()) => {
                                st.selected = Some(st.rename.trim().to_string());
                                st.message.clear();
                            }
                            Err(e) => st.message = e,
                        }
                    }
                    if ui.button("Delete").clicked() && delete_category(cx, &sel) {
                        st.selected = None;
                        st.message.clear();
                    }
                });
                ui.horizontal(|ui| {
                    if ui
                        .button("Add Selection")
                        .on_hover_text("Put the selected objects in this category")
                        .clicked()
                    {
                        let n = assign_selection(cx, &sel);
                        st.message = format!("{n} object(s) added to {sel}");
                    }
                    if ui.button("Remove Selection").clicked() {
                        let n = unassign_selection(cx, &sel);
                        st.message = format!("{n} object(s) taken out of {sel}");
                    }
                });
            }
            if !st.message.is_empty() {
                ui.colored_label(super::ERROR_RED, st.message.clone());
            }
        });
    st.open = open;
    STATE.with(|s| *s.borrow_mut() = st);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::ObjectRef;
    use crate::plan_defaults;
    use plan_core::schedules::ScheduleKind;
    use plan_core::{OpeningKind, WallKind};

    fn cx() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let w = cx.project.add_wall(
            0,
            plan_core::geometry::Point::new(0.0, 0.0),
            plan_core::geometry::Point::new(240.0, 0.0),
            6.5,
            109.0,
            WallKind::Exterior,
        );
        cx.project
            .add_opening(0, w, 60.0, OpeningKind::Door)
            .unwrap();
        cx.project
            .add_opening(0, w, 160.0, OpeningKind::Window)
            .unwrap();
        cx.mark_dirty();
        cx.refresh();
        cx
    }

    #[test]
    fn categories_are_created_renamed_assigned_and_deleted_with_undo() {
        let mut cx = cx();
        assert!(add_category(&mut cx, "Glazing").is_ok());
        assert!(add_category(&mut cx, "Glazing").is_err());
        assert!(add_category(&mut cx, "  ").is_err());
        assert_eq!(cx.undo_label(), Some("New Schedule Category"));
        let win = cx
            .floor()
            .openings
            .iter()
            .find(|o| o.kind == OpeningKind::Window)
            .unwrap()
            .id;
        cx.selection.set(ObjectRef::Opening(win));
        assert_eq!(assign_selection(&mut cx, "Glazing"), 1);
        assert_eq!(assign_selection(&mut cx, "Glazing"), 0, "already in");
        // A Door schedule that ticks the category lists the window.
        let id = schedule_view::add(
            &mut cx,
            ScheduleKind::Door,
            plan_core::geometry::Point::ZERO,
        );
        let mut d = schedule_view::find(&cx, id).unwrap();
        d.set_category(&custom_category_id("Glazing"), true);
        assert!(schedule_view::replace(&mut cx, 0, d));
        let rows = |cx: &EditorContext| {
            let d = schedule_view::find(cx, id).unwrap();
            schedule_view::table_for(cx, &d, 0).rows.len()
        };
        assert_eq!(rows(&cx), 2);
        // Renaming keeps the tick.
        assert!(rename_category(&mut cx, "Glazing", "Glass").is_ok());
        assert_eq!(rows(&cx), 2);
        assert!(cx.project.schedule_setup.category("Glass").is_some());
        // Taking the window out, then deleting the category.
        assert_eq!(unassign_selection(&mut cx, "Glass"), 1);
        assert_eq!(rows(&cx), 1);
        assert!(delete_category(&mut cx, "Glass"));
        assert!(!delete_category(&mut cx, "Glass"));
        let d = schedule_view::find(&cx, id).unwrap();
        assert!(!d.categories.contains_key(&custom_category_id("Glass")));
        assert_eq!(cx.undo_label(), Some("Delete Schedule Category"));
        cx.undo();
        assert!(cx.project.schedule_setup.category("Glass").is_some());
    }

    #[test]
    fn the_dialog_draws() {
        let mut cx = cx();
        add_category(&mut cx, "Kitchen").unwrap();
        open();
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| show(ctx, &mut cx));
        }
        assert!(is_open());
        STATE.with(|s| s.borrow_mut().open = false);
    }
}
