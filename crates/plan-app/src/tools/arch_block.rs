//! Architectural blocks in the editor (reference manual "Architectural
//! Blocks", pp. 1057 to 1062; parity rows CB-427..CB-438).
//!
//! Make Architectural Block turns the selected cabinets, symbols, devices,
//! slabs, 3D solids and other blocks into one block; a click on any member
//! then selects the whole block, so it moves, turns, mirrors, copies and
//! deletes as one. Explode Architectural Block dissolves it. Edit Sub-Objects
//! switches the selection back to single members until it is switched off.
//! The structure and the rules live in `plan_core::arch_block`; this module
//! is the editor's side: the commands on the Edit toolbar, the plan box and
//! the hooks the selection and delete code call.
//!
//! This module also gathers the Edit toolbar buttons and commands of the
//! other round 15 object modules ([`super::solids`], [`super::regions`],
//! [`super::distribution`]) so the editor needs only one hook.

use crate::editor::actions::{EditAction, EditActionKind};
use crate::editor::{selection, transform, EditorContext, ObjectRef};
use plan_core::arch_block::BlockKind;
use plan_core::geometry::Point;
use plan_core::{Floor, Id, ObjectRef as CoreRef};
use std::cell::Cell;

pub const MAKE_BLOCK: &str = "blocks.make";
pub const MAKE_GANGED: &str = "blocks.make_ganged";
pub const EXPLODE_BLOCK: &str = "blocks.explode";
pub const BLOCK_SPEC: &str = "blocks.spec";
pub const EDIT_SUB_OBJECTS: &str = "blocks.edit_sub";

thread_local! {
    /// Edit Sub-Objects is on: a click selects one member, not its block.
    static SUB_EDIT: Cell<bool> = const { Cell::new(false) };
}

/// Is Edit Sub-Objects on?
pub fn editing_sub_objects() -> bool {
    SUB_EDIT.with(|s| s.get())
}

/// Switches Edit Sub-Objects on or off.
pub fn set_editing_sub_objects(on: bool) {
    SUB_EDIT.with(|s| s.set(on));
}

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

/// The selected objects as group references, each once.
fn selected_core(cx: &EditorContext) -> Vec<CoreRef> {
    let mut out: Vec<CoreRef> = Vec::new();
    for r in cx.selection.items.iter().filter_map(|o| o.to_group_ref()) {
        if !out.contains(&r) {
            out.push(r);
        }
    }
    out
}

/// The outermost blocks that hold the selected objects, each once.
pub fn selected_blocks(cx: &EditorContext) -> Vec<Id> {
    let f = cx.floor();
    let mut out = Vec::new();
    for r in selected_core(cx) {
        if let Some(b) = f.block_for_pick(r) {
            if !out.contains(&b) {
                out.push(b);
            }
        }
    }
    out
}

/// The one block the selection is (all of its members and nothing else).
pub fn selected_block(cx: &EditorContext) -> Option<Id> {
    let blocks = selected_blocks(cx);
    let [id] = blocks.as_slice() else {
        return None;
    };
    let f = cx.floor();
    let sel = selected_core(cx);
    let flat = f.blocks.flat_members(*id);
    (flat.iter().all(|m| sel.contains(m)) && sel.iter().all(|m| flat.contains(m))).then_some(*id)
}

/// The members of a block picked through `o`: what a click on `o` selects
/// when it belongs to a block (empty when it does not or when sub-objects
/// are being edited).
pub fn block_members_of(floor: &Floor, o: ObjectRef) -> Vec<ObjectRef> {
    if editing_sub_objects() {
        return Vec::new();
    }
    let Some(g) = o.to_group_ref() else {
        return Vec::new();
    };
    let Some(b) = floor.block_for_pick(g) else {
        return Vec::new();
    };
    floor
        .blocks
        .flat_members(b)
        .into_iter()
        .map(ObjectRef::from_group_ref)
        .collect()
}

/// The plan box of block `id`: the box around its members.
pub fn block_bounds(cx: &EditorContext, id: Id) -> Option<(Point, Point)> {
    let members: Vec<ObjectRef> = cx
        .floor()
        .blocks
        .flat_members(id)
        .into_iter()
        .map(ObjectRef::from_group_ref)
        .collect();
    transform::items_bounds(cx, &members)
}

/// What the selection would make: the group references, with a block the
/// selection holds whole standing for its members (so blocks nest).
fn make_members(cx: &EditorContext) -> Vec<CoreRef> {
    let f = cx.floor();
    let sel = selected_core(cx);
    let mut out: Vec<CoreRef> = Vec::new();
    for r in &sel {
        let mapped = match f.blocks.root_of(*r) {
            Some(root)
                if f.blocks
                    .flat_members(root.id)
                    .iter()
                    .all(|m| sel.contains(m)) =>
            {
                CoreRef::Block(root.id)
            }
            _ => *r,
        };
        if !out.contains(&mapped) {
            out.push(mapped);
        }
    }
    out
}

/// Make Architectural Block (or, with `kind`, the Ganged Electrical Block)
/// from the selection: one undo step; returns the new block's id.
pub fn make_block(cx: &mut EditorContext, kind: BlockKind) -> Result<Id, String> {
    let members = make_members(cx);
    if members.len() < 2 {
        return Err("Select two or more architectural objects to make a block".into());
    }
    let fl = cx.floor;
    // The layers of the members, looked up before the floor is borrowed
    // mutably.
    let layers: Vec<(CoreRef, Option<String>)> = members
        .iter()
        .map(|m| {
            (
                *m,
                selection::layer_of(cx.floor(), ObjectRef::from_group_ref(*m)),
            )
        })
        .collect();
    let label = if kind == BlockKind::GangedElectrical {
        "Make Ganged Electrical Block"
    } else {
        "Make Architectural Block"
    };
    cx.begin_change(label);
    let id = cx.project.alloc_id();
    let layer_of = |r: CoreRef| {
        layers
            .iter()
            .find(|(m, _)| *m == r)
            .and_then(|(_, l)| l.clone())
    };
    match cx.project.floors[fl].make_block(id, &members, kind, &layer_of) {
        Ok(id) => {
            crate::editor::solids_view::ensure_layers(&mut cx.project.layers);
            cx.selection.items = cx
                .floor()
                .blocks
                .flat_members(id)
                .into_iter()
                .map(ObjectRef::from_group_ref)
                .collect();
            cx.mark_dirty();
            cx.status = format!("Made an architectural block of {} objects", members.len());
            Ok(id)
        }
        Err(e) => {
            cx.cancel_change();
            Err(e.to_string())
        }
    }
}

/// Explode Architectural Block: the outermost block of every selected
/// object goes, the members stay selected. Returns how many blocks went.
pub fn explode_blocks(cx: &mut EditorContext) -> Result<usize, String> {
    let blocks = selected_blocks(cx);
    if blocks.is_empty() {
        return Err("Nothing selected is in an architectural block".into());
    }
    let fl = cx.floor;
    cx.begin_change("Explode Architectural Block");
    let mut members: Vec<ObjectRef> = Vec::new();
    for b in &blocks {
        if let Some(m) = cx.project.floors[fl].explode_block(*b) {
            members.extend(m.into_iter().map(ObjectRef::from_group_ref));
        }
    }
    cx.selection
        .items
        .retain(|o| !matches!(o, ObjectRef::Block(_)));
    for m in members {
        if !matches!(m, ObjectRef::Block(_)) && !cx.selection.items.contains(&m) {
            cx.selection.items.push(m);
        }
    }
    cx.mark_dirty();
    cx.status = format!("Exploded {} block(s)", blocks.len());
    Ok(blocks.len())
}

/// Drops dead members from every block, and the blocks left empty (after a
/// delete).
pub fn prune_dead_blocks(cx: &mut EditorContext) {
    if cx.floor().blocks.is_empty() {
        return;
    }
    let fl = cx.floor;
    let project = &cx.project;
    let dead: Vec<CoreRef> = project.floors[fl]
        .blocks
        .blocks
        .iter()
        .flat_map(|b| b.members.iter().copied())
        .filter(|m| !matches!(m, CoreRef::Block(_)))
        .filter(|m| !ObjectRef::from_group_ref(*m).exists_in(project, fl))
        .collect();
    if !dead.is_empty() {
        cx.project.floors[fl].prune_blocks(|m| !dead.contains(&m));
    }
}

/// Moves and turns the compound solids and keeps the blocks consistent;
/// the part of an Edit command that runs `id`.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        MAKE_BLOCK => {
            let r = make_block(cx, BlockKind::Standard).map(|_| ());
            report(cx, r);
        }
        MAKE_GANGED => {
            let r = make_block(cx, BlockKind::GangedElectrical).map(|_| ());
            report(cx, r);
        }
        EXPLODE_BLOCK => {
            let r = explode_blocks(cx).map(|_| ());
            report(cx, r);
        }
        BLOCK_SPEC => match selected_blocks(cx).first() {
            Some(b) => crate::dialogs::arch_block::open(cx, *b),
            None => cx.status = "Select an architectural block first".into(),
        },
        EDIT_SUB_OBJECTS => {
            let on = !editing_sub_objects();
            set_editing_sub_objects(on);
            cx.status = if on {
                "Edit Sub-Objects: click a single member of a block".into()
            } else {
                "Edit Sub-Objects is off: a click selects the whole block".into()
            };
        }
        _ => {
            return super::solids::run_command(cx, id)
                || super::regions::run_command(cx, id)
                || super::distribution::run_command(cx, id)
                || crate::dialogs::soffit::run_command(cx, id);
        }
    }
    true
}

fn report(cx: &mut EditorContext, r: Result<(), String>) {
    if let Err(e) = r {
        cx.status = e;
    }
}

/// The Edit toolbar buttons of the round 15 object modules for the
/// selection.
pub fn edit_actions(cx: &EditorContext) -> Vec<EditAction> {
    let mut v = Vec::new();
    let f = cx.floor();
    let members = make_members(cx);
    let already = selected_block(cx).is_some();
    if members.len() >= 2 && !already && members.iter().all(|m| f.block_eligible(*m).is_ok()) {
        let devices = members
            .iter()
            .filter(|m| matches!(m, CoreRef::Device(_)))
            .count();
        if devices == members.len() {
            v.push(button(MAKE_GANGED, "Make Ganged Electrical Block"));
        } else if devices == 0 {
            v.push(button(MAKE_BLOCK, "Make Architectural Block"));
        }
    }
    if !selected_blocks(cx).is_empty() {
        v.push(button(EXPLODE_BLOCK, "Explode Architectural Block"));
        v.push(button(BLOCK_SPEC, "Architectural Block Specification"));
        v.push(button(
            EDIT_SUB_OBJECTS,
            if editing_sub_objects() {
                "Finish Editing Sub-Objects"
            } else {
                "Edit Sub-Objects"
            },
        ));
    } else if editing_sub_objects() {
        v.push(button(EDIT_SUB_OBJECTS, "Finish Editing Sub-Objects"));
    }
    v.extend(super::solids::edit_actions(cx));
    v.extend(super::regions::edit_actions(cx));
    v.extend(super::distribution::edit_actions(cx));
    v.extend(crate::dialogs::soffit::edit_actions(cx));
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_cabinets::{Cabinet, CabinetKind};

    fn cx_with_two_cabinets() -> (EditorContext, Id, Id) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let mut a = Cabinet::new(CabinetKind::Base, 24.0);
        a.position = Point::new(0.0, 0.0);
        let mut b = Cabinet::new(CabinetKind::Base, 24.0);
        b.position = Point::new(48.0, 0.0);
        let ia = crate::editor::placed::add_cabinet(&mut cx.project, 0, a).unwrap();
        let ib = crate::editor::placed::add_cabinet(&mut cx.project, 0, b).unwrap();
        (cx, ia, ib)
    }

    #[test]
    fn make_and_explode_are_single_undo_steps() {
        let (mut cx, a, b) = cx_with_two_cabinets();
        cx.selection.items = vec![ObjectRef::Cabinet(a), ObjectRef::Cabinet(b)];
        let id = make_block(&mut cx, BlockKind::Standard).unwrap();
        assert_eq!(cx.undo_label(), Some("Make Architectural Block"));
        assert_eq!(cx.floor().blocks.get(id).unwrap().members.len(), 2);
        assert_eq!(selected_block(&cx), Some(id));
        let (lo, hi) = block_bounds(&cx, id).unwrap();
        assert!(hi.x - lo.x >= 70.0, "the box spans both cabinets");
        explode_blocks(&mut cx).unwrap();
        assert!(cx.floor().blocks.is_empty());
        assert_eq!(cx.undo_label(), Some("Explode Architectural Block"));
        cx.undo();
        assert_eq!(cx.floor().blocks.len(), 1);
        cx.undo();
        assert!(cx.floor().blocks.is_empty());
    }

    #[test]
    fn a_click_on_a_member_selects_the_block_unless_editing_sub_objects() {
        let (mut cx, a, b) = cx_with_two_cabinets();
        cx.selection.items = vec![ObjectRef::Cabinet(a), ObjectRef::Cabinet(b)];
        make_block(&mut cx, BlockKind::Standard).unwrap();
        let clicked = selection::expand_groups(&cx, &[ObjectRef::Cabinet(a)]);
        assert_eq!(clicked.len(), 2);
        set_editing_sub_objects(true);
        let single = selection::expand_groups(&cx, &[ObjectRef::Cabinet(a)]);
        set_editing_sub_objects(false);
        assert_eq!(single, vec![ObjectRef::Cabinet(a)]);
    }

    #[test]
    fn one_object_or_a_wall_cannot_make_a_block() {
        let (mut cx, a, _) = cx_with_two_cabinets();
        cx.selection.items = vec![ObjectRef::Cabinet(a)];
        assert!(make_block(&mut cx, BlockKind::Standard).is_err());
        assert!(
            cx.undo_label().is_none(),
            "a refused command leaves no undo step"
        );
        let w = cx.project.add_wall(
            0,
            Point::ZERO,
            Point::new(100.0, 0.0),
            6.0,
            96.0,
            plan_core::WallKind::Interior,
        );
        cx.selection.items = vec![ObjectRef::Cabinet(a), ObjectRef::Wall(w)];
        assert!(make_block(&mut cx, BlockKind::Standard).is_err());
    }
}
