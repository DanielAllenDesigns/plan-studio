//! Edit-toolbar commands of a selected section camera (brief 32, C-137):
//! Add Break on its cutting plane, and Make Parallel / Make Perpendicular on
//! the break handle that was last clicked (manual p. 1171-1172).

use std::cell::Cell;

use plan_core::Id;

use super::{EditAction, EditActionKind, EditorContext, ObjectRef};
use crate::tools::camera::{is_section, section_width};

pub const ADD_BREAK: &str = "camera.add_break";
pub const MAKE_PARALLEL: &str = "camera.make_parallel";
pub const MAKE_PERPENDICULAR: &str = "camera.make_perpendicular";

thread_local! {
    /// The break handle (camera, break index) the Select tool last pressed.
    static SELECTED_BREAK: Cell<Option<(Id, usize)>> = const { Cell::new(None) };
}

/// Remembers the break handle that was pressed (the Select tool calls this).
pub fn select_break(camera: Id, break_index: usize) {
    SELECTED_BREAK.with(|s| s.set(Some((camera, break_index))));
}

/// The selected section camera, when exactly one camera is selected.
fn selected_section(cx: &EditorContext) -> Option<Id> {
    match cx.selection.single()? {
        ObjectRef::Camera(id) if cx.project.camera(id).is_some_and(is_section) => Some(id),
        _ => None,
    }
}

/// The break handle that is still valid for the selected camera.
fn selected_break(cx: &EditorContext) -> Option<(Id, usize)> {
    let id = selected_section(cx)?;
    let (cam, i) = SELECTED_BREAK.with(Cell::get)?;
    let c = cx.project.camera(id)?;
    (cam == id && i < c.view.clip.plane.breaks.len()).then_some((id, i))
}

/// The Edit toolbar buttons for the current selection.
pub fn edit_buttons(cx: &EditorContext) -> Vec<EditAction> {
    let button = |id: &'static str, label: &'static str| {
        EditAction::new(EditActionKind::Custom {
            id,
            label,
            icon: "",
        })
    };
    let mut v = Vec::new();
    if selected_section(cx).is_some() {
        v.push(button(ADD_BREAK, "Add Break"));
    }
    if selected_break(cx).is_some() {
        v.push(button(MAKE_PARALLEL, "Make Parallel"));
        v.push(button(MAKE_PERPENDICULAR, "Make Perpendicular"));
    }
    v
}

/// Runs one of this module's commands; false when `id` is not one of them.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        ADD_BREAK => {
            let Some(cam) = selected_section(cx) else {
                cx.status = "Select a section camera first".into();
                return true;
            };
            cx.begin_change("Add Break");
            let mut added = None;
            cx.project.update_camera(cam, |c| {
                // The break goes in the middle of the widest piece.
                let half = section_width(c) * 0.5;
                let (x0, x1, _) = c
                    .view
                    .clip
                    .plane
                    .spans(-half, half)
                    .into_iter()
                    .max_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0)))
                    .unwrap_or((-half, half, 0.0));
                added = c.view.clip.plane.add_break((x0 + x1) * 0.5);
            });
            if let Some(i) = added {
                select_break(cam, i);
                cx.status = "Added a break to the cutting plane".into();
            } else {
                cx.status = "No room for another break on the cutting plane".into();
            }
            true
        }
        MAKE_PARALLEL | MAKE_PERPENDICULAR => {
            let Some((cam, i)) = selected_break(cx) else {
                cx.status = "Select a break handle first".into();
                return true;
            };
            cx.begin_change(if id == MAKE_PARALLEL {
                "Make Parallel"
            } else {
                "Make Perpendicular"
            });
            cx.project.update_camera(cam, |c| {
                if id == MAKE_PARALLEL {
                    c.view.clip.plane.make_parallel(i);
                } else {
                    c.view.clip.plane.make_perpendicular(i);
                }
            });
            true
        }
        _ => false,
    }
}
