//! Distribution Region and Path options (reference manual "Distributed
//! Objects", pp. 1091 to 1098; parity rows CB-468..CB-474): the options
//! window of a selected distribution record and Explode Distributed Object.
//!
//! The record is a library symbol with a `plan_core::images::Distribution`
//! (drawn by `tools::images`); the options and the copy maker are
//! `plan_core::distribution`.

use crate::editor::actions::{EditAction, EditActionKind};
use crate::editor::{EditorContext, ObjectRef};
use plan_core::Id;

pub const OPTIONS: &str = "distribution.options";
pub const EXPLODE: &str = "distribution.explode";

fn button(id: &'static str, label: &'static str) -> EditAction {
    EditAction {
        kind: EditActionKind::Custom {
            id,
            label,
            icon: "",
        },
        label,
        icon: None,
        enabled: true,
    }
}

/// The distribution record the selection means: a selected record, or the
/// owner of a selected copy. `None` unless exactly one record is meant.
pub fn selected_record(cx: &EditorContext) -> Option<Id> {
    let mut found: Option<Id> = None;
    for o in &cx.selection.items {
        let ObjectRef::Symbol(id) = *o else {
            continue;
        };
        let s = cx.floor().symbol(id)?;
        let rec = if s.distribution.is_some() {
            id
        } else {
            s.owner?
        };
        match found {
            None => found = Some(rec),
            Some(f) if f == rec => {}
            Some(_) => return None,
        }
    }
    found
}

/// Explode Distributed Object: the copies stay as ordinary symbols, the
/// record goes (one undo step). Returns how many copies were released.
pub fn explode(cx: &mut EditorContext) -> Result<usize, String> {
    let Some(rec) = selected_record(cx) else {
        return Err("Select a distribution region or path first".into());
    };
    let fl = cx.floor;
    let copies: Vec<Id> = cx
        .floor()
        .symbols
        .iter()
        .filter(|s| s.owner == Some(rec))
        .map(|s| s.id)
        .collect();
    cx.begin_change("Explode Distributed Object");
    match cx.project.explode_distribution(fl, rec) {
        Some(n) => {
            cx.selection.items = copies.into_iter().map(ObjectRef::Symbol).collect();
            cx.mark_dirty();
            cx.status = format!("Exploded into {n} objects");
            Ok(n)
        }
        None => {
            cx.cancel_change();
            Err("That is not a distribution record".into())
        }
    }
}

/// Runs the distribution commands.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        OPTIONS => match selected_record(cx) {
            Some(r) => crate::dialogs::distribution::open(cx, r),
            None => cx.status = "Select a distribution region or path first".into(),
        },
        EXPLODE => {
            if let Err(e) = explode(cx) {
                cx.status = e;
            }
        }
        _ => return false,
    }
    true
}

/// The Edit toolbar buttons for the selection.
pub fn edit_actions(cx: &EditorContext) -> Vec<EditAction> {
    if selected_record(cx).is_none() {
        return Vec::new();
    }
    vec![
        button(OPTIONS, "Distribution Options"),
        button(EXPLODE, "Explode Distributed Object"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::geometry::Point;
    use plan_core::images::{DistKind, Distribution};

    fn cx_with_region() -> (EditorContext, Id) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let mut d = Distribution::new(
            DistKind::Region,
            false,
            vec![
                Point::new(0.0, 0.0),
                Point::new(120.0, 0.0),
                Point::new(120.0, 120.0),
                Point::new(0.0, 120.0),
            ],
            "Shrub",
            [12.0, 12.0, 24.0],
        );
        d.spacing = 30.0;
        let id = cx.project.add_distribution(0, d);
        (cx, id)
    }

    #[test]
    fn explode_keeps_the_copies_and_drops_the_record_for_good() {
        let (mut cx, rec) = cx_with_region();
        let copies = cx.project.distribution_copies(0, rec);
        assert!(copies > 4);
        cx.selection.items = vec![ObjectRef::Symbol(rec)];
        assert_eq!(selected_record(&cx), Some(rec));
        let n = explode(&mut cx).unwrap();
        assert_eq!(n, copies);
        assert!(cx.floor().symbol(rec).is_none());
        assert_eq!(cx.floor().symbols.len(), copies);
        assert_eq!(cx.undo_label(), Some("Explode Distributed Object"));
        cx.undo();
        assert!(cx.floor().symbol(rec).is_some());
        assert_eq!(cx.project.distribution_copies(0, rec), copies);
    }

    #[test]
    fn a_selected_copy_means_its_record() {
        let (mut cx, rec) = cx_with_region();
        let copy = cx
            .floor()
            .symbols
            .iter()
            .find(|s| s.owner == Some(rec))
            .unwrap()
            .id;
        cx.selection.items = vec![ObjectRef::Symbol(copy)];
        assert_eq!(selected_record(&cx), Some(rec));
        assert_eq!(edit_actions(&cx).len(), 2);
    }
}
