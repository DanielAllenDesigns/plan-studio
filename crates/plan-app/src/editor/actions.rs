//! Commands of the contextual Edit toolbar, shared by every tool: Open
//! Object, Delete, Copy, Paste in Place, Reverse Swing, Fix Wall Connections.

use super::{ops, placed};

use super::selection::{layer_of, ObjectRef};
use super::{EditorContext, EditorRequest};
use plan_core::{CadObject, Dimension, Id, Opening, OpeningKind, Wall};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditActionKind {
    OpenObject,
    Delete,
    Copy,
    PasteInPlace,
    ReverseSwing,
    FixWallConnections,
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

/// Objects copied with Copy; pasted back with new ids.
#[derive(Clone, Debug, Default)]
pub struct Clipboard {
    pub walls: Vec<Wall>,
    pub openings: Vec<Opening>,
    pub dimensions: Vec<Dimension>,
    pub cad: Vec<CadObject>,
}

impl Clipboard {
    pub fn is_empty(&self) -> bool {
        self.walls.is_empty()
            && self.openings.is_empty()
            && self.dimensions.is_empty()
            && self.cad.is_empty()
    }

    pub fn len(&self) -> usize {
        self.walls.len() + self.openings.len() + self.dimensions.len() + self.cad.len()
    }
}

impl EditorContext {
    /// The buttons every object type shares, for the current selection.
    pub fn common_edit_actions(&self) -> Vec<EditAction> {
        if self.selection.is_empty() {
            return Vec::new();
        }
        let mut v = Vec::new();
        if self.selection.single().is_some() {
            v.push(EditAction::new(EditActionKind::OpenObject));
        }
        v.push(EditAction::new(EditActionKind::Delete));
        v.push(EditAction::new(EditActionKind::Copy));
        let paste = EditAction::new(EditActionKind::PasteInPlace);
        v.push(if self.has_clipboard() {
            paste
        } else {
            paste.disabled()
        });
        v
    }

    /// Runs an Edit toolbar command on the current selection.
    pub fn apply_edit_action(&mut self, kind: EditActionKind) {
        match kind {
            EditActionKind::OpenObject => {
                if let Some(o) = self.selection.single() {
                    self.requests.push(EditorRequest::OpenSpec(o));
                }
            }
            EditActionKind::Delete => self.delete_selection(),
            EditActionKind::Copy => self.copy_selection(),
            EditActionKind::PasteInPlace => self.paste_in_place(),
            EditActionKind::ReverseSwing => self.reverse_swing(),
            EditActionKind::FixWallConnections => {
                super::connect::fix_wall_connections_action(self);
            }
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
        self.mark_dirty();
        self.status.clear();
    }

    pub fn copy_selection(&mut self) {
        let f = self.floor();
        let mut clip = Clipboard::default();
        for o in &self.selection.items {
            match *o {
                ObjectRef::Wall(id) => {
                    if let Some(w) = f.wall(id) {
                        clip.walls.push(w.clone());
                        clip.openings.extend(f.openings_on(id).cloned());
                    }
                }
                ObjectRef::Opening(id) => {
                    if let Some(op) = f.openings.iter().find(|x| x.id == id) {
                        if !clip.openings.iter().any(|x| x.id == id) {
                            clip.openings.push(op.clone());
                        }
                    }
                }
                ObjectRef::Dimension(id) => {
                    clip.dimensions
                        .extend(f.dimensions.iter().find(|d| d.id == id).cloned());
                }
                ObjectRef::Cad(id) | ObjectRef::Text(id) => {
                    clip.cad.extend(f.cad.iter().find(|c| c.id == id).cloned());
                }
                _ => {}
            }
        }
        if clip.is_empty() {
            // Cabinets and symbols have their own clipboard.
            if self.copy_extra() == 0 {
                self.status = "Nothing to copy".into();
            } else {
                self.clipboard = None;
            }
            return;
        }
        placed::clear_clipboard();
        let n = clip.len() + self.copy_extra();
        self.clipboard = Some(clip);
        self.status = format!("Copied {n} object{}", if n == 1 { "" } else { "s" });
    }

    /// Pastes the clipboard at its original coordinates; the copies become
    /// the selection. Openings come along only with their host wall.
    pub fn paste_in_place(&mut self) {
        let Some(clip) = self.clipboard.clone().filter(|c| !c.is_empty()) else {
            if self.paste_extra() == 0 {
                self.status = "Nothing to paste".into();
            } else {
                self.status = "Pasted in place".into();
            }
            return;
        };
        self.begin_change("Paste in Place");
        let fl = self.floor;
        let mut sel = Vec::new();
        let mut wall_map: HashMap<Id, Id> = HashMap::new();
        for w in &clip.walls {
            let id = self.project.alloc_id();
            let mut copy = w.clone();
            copy.id = id;
            self.project.floors[fl].walls.push(copy);
            wall_map.insert(w.id, id);
            sel.push(ObjectRef::Wall(id));
        }
        for o in &clip.openings {
            let Some(host) = wall_map.get(&o.wall_id) else {
                continue;
            };
            let id = self.project.alloc_id();
            let mut copy = o.clone();
            copy.id = id;
            copy.wall_id = *host;
            self.project.floors[fl].openings.push(copy);
            if let Some(extras) = self.extras.openings.get(&o.id).cloned() {
                self.extras.openings.insert(id, extras);
            }
        }
        for d in &clip.dimensions {
            let id = self.project.add_dimension(fl, d.clone());
            sel.push(ObjectRef::Dimension(id));
        }
        for c in &clip.cad {
            let id = self.project.add_cad(fl, c.layer.clone(), c.item.clone());
            sel.push(ObjectRef::Cad(id));
        }
        self.selection.items = sel;
        let core_sel = self.selection.items.clone();
        if self.paste_extra() > 0 {
            let mut all = core_sel;
            all.extend(self.selection.items.iter().copied());
            self.selection.items = all;
        }
        self.mark_dirty();
        self.status = "Pasted in place".into();
    }

    /// Flips the swing of the selected doors.
    pub fn reverse_swing(&mut self) {
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
