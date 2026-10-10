//! Commands of the contextual Edit toolbar, shared by every tool: Open
//! Object, Delete, Cut, Copy, Paste in Place, Reverse Swing, Fix Wall
//! Connections. The clipboard lives in `clipboard.rs`; Paste, Duplicate,
//! Group, the context menu and the other `edit.*` commands in
//! `edit_commands.rs`.

use super::{ops, placed};

use super::selection::{layer_of, ObjectRef};
use super::{EditorContext, EditorRequest};
use plan_core::geometry::Point;
use plan_core::{Id, OpeningKind};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditActionKind {
    OpenObject,
    Delete,
    Cut,
    Copy,
    PasteInPlace,
    ReverseSwing,
    FixWallConnections,
    /// A command that works on any selection (Group, Select Same Type,
    /// Transform/Replicate, Layer, Lock ...): an `edit.*` id of
    /// `edit_commands`, run by `EditorContext::run_edit_command`.
    Command {
        id: &'static str,
        label: &'static str,
    },
    /// A command of one object type (Auto Stairwell, Join Roof Planes, ...),
    /// run by `EditorContext::run_custom` with its `id`.
    Custom {
        id: &'static str,
        label: &'static str,
        /// An icon id from `icons.rs`; empty when none fits.
        icon: &'static str,
    },
}

/// One button of the Edit toolbar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EditAction {
    pub kind: EditActionKind,
    pub label: &'static str,
    /// An icon id from `icons.rs`, when one fits.
    pub icon: Option<&'static str>,
    pub enabled: bool,
}

impl EditAction {
    pub fn new(kind: EditActionKind) -> Self {
        let (label, icon) = match kind {
            EditActionKind::OpenObject => ("Open Object", Some("select")),
            EditActionKind::Delete => ("Delete Objects", Some("delete_surface")),
            EditActionKind::Cut => ("Cut", None),
            EditActionKind::Command { label, .. } => (label, None),
            EditActionKind::Copy => ("Copy Selected Objects", None),
            EditActionKind::PasteInPlace => ("Paste in Place", Some("paste_hold")),
            EditActionKind::ReverseSwing => ("Reverse Swing", Some("door_hinged")),
            EditActionKind::FixWallConnections => ("Fix Wall Connections", Some("wall_fix")),
            EditActionKind::Custom { label, icon, .. } => {
                (label, (!icon.is_empty()).then_some(icon))
            }
        };
        Self {
            kind,
            label,
            icon,
            enabled: true,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

pub use super::clipboard::Clipboard;

impl EditorContext {
    /// The buttons every object type shares, for the current selection.
    pub fn common_edit_actions(&self) -> Vec<EditAction> {
        if self.selection.is_empty() {
            return Vec::new();
        }
        let mut v = Vec::new();
        if self.selection.single().is_some()
            || self.selection.all_walls()
            || self.selection.all_cabinets()
            || crate::tools::text::selected_annot(self).is_some()
        {
            v.push(EditAction::new(EditActionKind::OpenObject));
        }
        v.push(EditAction::new(EditActionKind::Delete));
        v.push(EditAction::new(EditActionKind::Cut));
        v.push(EditAction::new(EditActionKind::Copy));
        let paste = EditAction::new(EditActionKind::PasteInPlace);
        v.push(if self.has_clipboard() {
            paste
        } else {
            paste.disabled()
        });
        v.extend(self.selection_edit_actions());
        v
    }

    /// Runs an Edit toolbar command on the current selection.
    pub fn apply_edit_action(&mut self, kind: EditActionKind) {
        self.undo_group(|cx| cx.apply_edit_action_ungrouped(kind));
    }

    fn apply_edit_action_ungrouped(&mut self, kind: EditActionKind) {
        match kind {
            EditActionKind::OpenObject => {
                // Several walls open one dialog over all of them (W-83).
                if let Some(o) = self
                    .selection
                    .single()
                    .or_else(|| self.selection.all_walls().then(|| self.selection.items[0]))
                    .or_else(|| {
                        self.selection
                            .all_cabinets()
                            .then(|| self.selection.items[0])
                    })
                    .or_else(|| {
                        crate::tools::text::selected_annot(self).map(|(_, id)| ObjectRef::Cad(id))
                    })
                {
                    self.requests.push(EditorRequest::OpenSpec(o));
                }
            }
            EditActionKind::Delete => self.delete_selection(),
            EditActionKind::Cut => self.cut_selection(),
            EditActionKind::Copy => self.copy_selection(),
            EditActionKind::PasteInPlace => self.paste_in_place(),
            EditActionKind::ReverseSwing => self.reverse_swing(),
            EditActionKind::FixWallConnections => {
                super::connect::fix_wall_connections_action(self);
            }
            EditActionKind::Command { id, .. } => self.run_edit_command(id),
            EditActionKind::Custom { id, .. } => self.run_custom(id),
        }
    }

    fn is_locked(&self, o: ObjectRef) -> bool {
        layer_of(self.floor(), o).is_some_and(|l| self.project.layers.is_locked(&l))
    }

    /// True if `o` may be edited (its layer is not locked); sets the status
    /// message otherwise (S-5, S-89).
    pub fn check_unlocked(&mut self, o: ObjectRef) -> bool {
        if self.is_locked(o) {
            self.status = "That object is on a locked layer".into();
            false
        } else {
            true
        }
    }

    /// Delete / Backspace: removes the selection (one undo step). Objects on
    /// locked layers are refused (S-89).
    pub fn delete_selection(&mut self) {
        // Walls, doors, cabinets and stairs each open their own step; a Delete
        // is one (QA-24).
        self.undo_group(|cx| cx.delete_selection_ungrouped());
    }

    fn delete_selection_ungrouped(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let (locked, free): (Vec<ObjectRef>, Vec<ObjectRef>) = self
            .selection
            .items
            .iter()
            .partition(|o| self.is_locked(**o));
        if free.is_empty() {
            self.status = "Those objects are on a locked layer".into();
            return;
        }
        // The selection holds only the free objects while the owners of each
        // kind delete theirs; the locked ones come back afterwards.
        self.selection.items = free.clone();
        let fl = self.floor;
        if self.selection_has_core() {
            self.begin_change("Delete");
            ops::delete_objects(&mut self.project, fl, &free);
        }
        self.delete_extra();
        self.selection.items = locked;
        self.selection.retain_existing(&self.project, fl);
        // A group left with fewer than two members goes (S-35).
        self.prune_dead_groups();
        crate::tools::arch_block::prune_dead_blocks(self);
        self.mark_dirty();
        self.status.clear();
    }

    /// Copy: the selection goes to the clipboard (S-81).
    pub fn copy_selection(&mut self) {
        let clip = Clipboard::capture(self);
        if clip.is_empty() {
            self.status = if clip.skipped.is_empty() {
                "Nothing to copy".into()
            } else {
                format!("Cannot copy {}", clip.skipped.join(", "))
            };
            return;
        }
        let n = clip.len();
        let note = if clip.skipped.is_empty() {
            String::new()
        } else {
            format!(" (not copied: {})", clip.skipped.join(", "))
        };
        self.store_clipboard(clip);
        self.status = format!("Copied {n} object{}{note}", if n == 1 { "" } else { "s" });
    }

    /// Pastes the clipboard at its original coordinates (Paste Hold
    /// Position, `C, P, P`); the copies become the selection (S-83, S-84).
    pub fn paste_in_place(&mut self) {
        self.paste_at(Point::ZERO, "Paste in Place", false);
    }

    /// Flips the swing of the selected doors.
    pub fn reverse_swing(&mut self) {
        self.undo_group(|cx| cx.reverse_swing_ungrouped());
    }

    fn reverse_swing_ungrouped(&mut self) {
        placed::reverse_door_swing(self);
        let ids: Vec<Id> = self
            .selection
            .items
            .iter()
            .filter_map(|o| match o {
                ObjectRef::Opening(id) => Some(*id),
                _ => None,
            })
            .filter(|id| {
                self.floor()
                    .openings
                    .iter()
                    .any(|o| o.id == *id && o.kind == OpeningKind::Door)
            })
            .collect();
        if ids.is_empty() {
            return;
        }
        self.begin_change("Reverse Swing");
        let fl = self.floor;
        for o in self.project.floors[fl]
            .openings
            .iter_mut()
            .filter(|o| ids.contains(&o.id))
        {
            o.swing_flipped = !o.swing_flipped;
        }
        self.mark_dirty();
    }
}
