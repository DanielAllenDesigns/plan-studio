//! The Edit menu's commands (S-8, S-32..S-36, S-52..S-54, S-79, S-81..S-84,
//! S-88, S-93, S-101..S-106): Cut, Copy, Paste, Duplicate, Select All, Select
//! Same Type, Group, Lock, Send to Layer, Transform/Replicate, Align, the
//! click-driven modes, the Action History and the right-click menu.
//!
//! Every command has an id (`edit.cut`, `edit.align.left`, ...) so a menu
//! row, a toolbar button, a hotkey and the context menu all reach it through
//! [`EditorContext::run_custom`] (the ids start with `edit.`).

use super::actions::{EditAction, EditActionKind};
use super::selection::{self, layer_of, ObjectRef};
use super::transform::{self, Mode};
use super::{Clipboard, EditorContext, EditorRequest};
use crate::toolbar::Action;
use crate::tools::ToolId;
use plan_core::geometry::Point;
use plan_core::groups::ObjectRef as CoreRef;
use plan_core::transform::{AlignMode, Axis};
use plan_core::OpeningKind;

/// Duplicate puts the copy this far from the original (Cmd+D): 12" right and
/// 12" down on the screen.
pub const DUPLICATE_OFFSET: Point = Point::new(12.0, -12.0);

/// Command ids.
pub mod ids {
    pub const CUT: &str = "edit.cut";
    pub const COPY: &str = "edit.copy";
    pub const PASTE: &str = "edit.paste";
    pub const PASTE_HOLD: &str = "edit.paste_hold";
    pub const PASTE_GROUP: &str = "edit.paste_group";
    pub const COPY_PASTE_IN_PLACE: &str = "edit.copy_paste_in_place";
    pub const DUPLICATE: &str = "edit.duplicate";
    pub const DELETE: &str = "edit.delete";
    pub const DELETE_OBJECTS: &str = "edit.delete_objects";
    pub const SELECT_ALL: &str = "edit.select_all";
    pub const SELECT_SAME: &str = "edit.select_same";
    pub const GROUP: &str = "edit.group";
    pub const UNGROUP: &str = "edit.ungroup";
    pub const EXPLODE: &str = "edit.explode";
    pub const LOCK: &str = "edit.lock";
    pub const UNLOCK: &str = "edit.unlock";
    pub const LAYER: &str = "edit.layer";
    pub const TRANSFORM: &str = "edit.transform";
    pub const ROTATE: &str = "edit.rotate";
    pub const REFLECT: &str = "edit.reflect";
    pub const REFLECT_COPY: &str = "edit.reflect_copy";
    pub const POINT_TO_POINT: &str = "edit.point_to_point";
    pub const CENTER: &str = "edit.center";
    pub const POINT_TO_POINT_CENTER: &str = "edit.point_to_point_center";
    pub const PARALLEL: &str = "edit.parallel";
    pub const PERPENDICULAR: &str = "edit.perpendicular";
    pub const DISTRIBUTE_H: &str = "edit.distribute.h";
    pub const DISTRIBUTE_V: &str = "edit.distribute.v";
    pub const ALIGN_DIALOG: &str = "edit.align_dialog";
    pub const FRONT: &str = "edit.front";
    pub const BACK: &str = "edit.back";
    pub const HISTORY: &str = "edit.history";
    pub const OPEN: &str = "edit.open";
    pub const REVERSE_SWING: &str = "edit.reverse_swing";
    pub const FLIP_HINGE: &str = "edit.flip_hinge";
    pub const REVERSE_LAYERS: &str = "edit.reverse_layers";
    pub const FIX_WALLS: &str = "edit.fix_walls";
}

/// One row of the right-click menu.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextEntry {
    pub label: String,
    pub action: Action,
    pub enabled: bool,
    /// Draw a separator above the row.
    pub sep_before: bool,
}

impl ContextEntry {
    fn new(label: impl Into<String>, action: Action) -> Self {
        Self {
            label: label.into(),
            action,
            enabled: true,
            sep_before: false,
        }
    }

    fn cmd(label: impl Into<String>, id: &'static str) -> Self {
        Self::new(label, Action::Custom(id))
    }

    fn when(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    fn sep(mut self) -> Self {
        self.sep_before = true;
        self
    }
}

fn custom_button(id: &'static str, label: &'static str) -> EditAction {
    EditAction {
        kind: EditActionKind::Command { id, label },
        label,
        icon: None,
        enabled: true,
    }
}

impl EditorContext {
    // ----- clipboard -----

    /// Cut: copies the selection and deletes it, as one undo step (S-81).
    pub fn cut_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let items = self.selection.items.clone();
        if items.iter().any(|o| self.is_object_locked(*o)) {
            self.status = "Those objects are on a locked layer".into();
            return;
        }
        let clip = Clipboard::capture(self);
        if clip.is_empty() {
            self.status = "Nothing to cut".into();
            return;
        }
        if !clip.skipped.is_empty() {
            // Deleting what the clipboard cannot hold would lose it.
            self.status = format!(
                "Cannot cut {}: copy or delete the rest separately",
                clip.skipped.join(", ")
            );
            return;
        }
        let n = clip.len();
        self.store_clipboard(clip);
        self.as_one_step("Cut", |cx| cx.delete_selection());
        self.status = format!("Cut {n} object{}", if n == 1 { "" } else { "s" });
    }

    /// Pastes the clipboard shifted by `offset`; the copies become the
    /// selection. One undo step named `label`. Returns how many objects.
    pub fn paste_at(&mut self, offset: Point, label: &str, as_group: bool) -> usize {
        self.paste_core(offset, label, as_group, false)
    }

    /// Drops the clipboard at the end of the cursor-attached paste: pasted
    /// walls join the walls they touch (S-82).
    pub fn paste_attached(&mut self, offset: Point, as_group: bool) -> usize {
        self.paste_core(offset, "Paste", as_group, true)
    }

    fn paste_core(&mut self, offset: Point, label: &str, as_group: bool, join: bool) -> usize {
        let Some(clip) = self.clipboard.clone().filter(|c| !c.is_empty()) else {
            self.status = "Nothing to paste".into();
            return 0;
        };
        self.begin_change(label);
        let sel = clip.paste(self, offset, as_group);
        if sel.is_empty() {
            self.cancel_change();
            self.status = "Nothing to paste".into();
            return 0;
        }
        if join {
            for o in &sel {
                if let ObjectRef::Wall(id) = *o {
                    super::connect::auto_connect(self, id);
                }
            }
        }
        self.selection.items = sel;
        self.selection.retain_existing(&self.project, self.floor);
        self.mark_dirty();
        let n = self.selection.len();
        self.status = if offset.length() < 1e-9 {
            "Pasted in place".into()
        } else {
            format!("Pasted {n} object{}", if n == 1 { "" } else { "s" })
        };
        n
    }

    /// Paste (Cmd+V): the clipboard follows the pointer until a click drops
    /// it; Esc cancels (S-82). The tool changes to Select Objects.
    pub fn begin_paste(&mut self, as_group: bool) {
        if !self.has_clipboard() {
            self.status = "Nothing to paste".into();
            return;
        }
        transform::begin_mode(self, Mode::Paste { as_group, at: None });
        self.requests.push(EditorRequest::SetTool(ToolId::Select));
    }

    /// Duplicate (Cmd+D): a copy of the selection 12" over and down, one undo
    /// step; the clipboard is left alone.
    pub fn duplicate_selection(&mut self) {
        let clip = Clipboard::capture(self);
        if clip.is_empty() {
            self.status = "Nothing to duplicate".into();
            return;
        }
        self.begin_change("Duplicate");
        let sel = clip.paste(self, DUPLICATE_OFFSET, false);
        if sel.is_empty() {
            self.cancel_change();
            return;
        }
        self.selection.items = sel;
        self.mark_dirty();
        self.status = "Duplicated the selection".into();
    }

    /// How many undo steps there are (cancelled steps do not count). A
    /// dialog host compares it before and after an apply to see whether the
    /// apply made a step.
    pub fn undo_depth(&self) -> usize {
        self.history.depth()
    }

    /// Runs `f`, which may open several undo steps, and leaves them as one
    /// step named `label`.
    pub fn as_one_step(&mut self, label: &str, f: impl FnOnce(&mut Self)) {
        // Inside an undo group the steps are not pushed until the group ends:
        // the group's single step takes the label instead.
        if self.history.group_is_empty() {
            f(self);
            if self.history.group_has_change() {
                self.history.relabel_last(label);
            }
            return;
        }
        let before = self.history.depth();
        f(self);
        let added = self.history.depth().saturating_sub(before);
        match added {
            0 => {}
            1 => self.history.relabel_last(label),
            n => {
                let after = self.project.clone();
                for _ in 0..n {
                    self.undo();
                }
                self.begin_change(label);
                self.project = after;
                self.selection.retain_existing(&self.project, self.floor);
                self.mark_dirty();
            }
        }
    }

    fn is_object_locked(&self, o: ObjectRef) -> bool {
        layer_of(self.floor(), o).is_some_and(|l| self.project.layers.is_locked(&l))
    }

    // ----- selection -----

    /// Select All (Cmd+A, S-33).
    pub fn select_all_objects(&mut self) {
        let n = selection::select_all(self);
        self.status = format!("Selected {n} object{}", if n == 1 { "" } else { "s" });
    }

    /// Select Same Type (S-32).
    pub fn select_same_type(&mut self) {
        let n = selection::select_same_type(self);
        self.status = if n == 0 {
            "Select an object first".into()
        } else {
            format!(
                "Selected {n} object{} of the same type",
                if n == 1 { "" } else { "s" }
            )
        };
    }

    // ----- groups -----

    /// The members of the selection that can join a group.
    fn groupable(&self) -> Vec<CoreRef> {
        let mut out: Vec<CoreRef> = Vec::new();
        for o in &self.selection.items {
            // A hosted opening travels with its wall.
            if matches!(o, ObjectRef::Opening(_)) {
                continue;
            }
            if let Some(g) = o.to_group_ref() {
                if !out.contains(&g) {
                    out.push(g);
                }
            }
        }
        out
    }

    /// Does the selection touch a group (that is not a CAD block)?
    fn selection_in_group(&self) -> bool {
        let f = self.floor();
        self.selection
            .items
            .iter()
            .filter_map(|o| o.to_group_ref())
            .any(|g| f.group_of(g).is_some())
    }

    fn selection_in_cad_block(&self) -> bool {
        let f = self.floor();
        self.selection
            .items
            .iter()
            .filter_map(|o| o.to_group_ref())
            .any(|g| f.group_of(g).is_some_and(|gr| f.cad_block(gr.id).is_some()))
    }

    /// Group Selected Objects (S-35): the objects select and move together
    /// from now on.
    pub fn group_selection(&mut self) {
        let refs = self.groupable();
        if refs.len() < 2 {
            self.status = "Select two or more objects to group".into();
            return;
        }
        if self.selection_in_cad_block() {
            self.status = "Explode the CAD block first".into();
            return;
        }
        self.begin_change("Group");
        let fl = self.floor;
        if self.project.make_group(fl, &refs).is_some() {
            self.mark_dirty();
            self.status = format!("Grouped {} objects", refs.len());
        } else {
            self.cancel_change();
        }
    }

    /// Ungroup / Explode: dissolves every group (and CAD block) the selection
    /// belongs to; the members stay selected.
    pub fn ungroup_selection(&mut self) {
        let f = self.floor();
        let mut groups: Vec<plan_core::Id> = Vec::new();
        for g in self.selection.items.iter().filter_map(|o| o.to_group_ref()) {
            if let Some(gr) = f.group_of(g) {
                if !groups.contains(&gr.id) {
                    groups.push(gr.id);
                }
            }
        }
        if groups.is_empty() {
            self.status = "Nothing selected is grouped".into();
            return;
        }
        self.begin_change("Ungroup");
        let fl = self.floor;
        for g in &groups {
            if self.floor().cad_block(*g).is_some() {
                self.project.explode_cad_block(fl, *g);
            } else {
                self.project.explode_group(fl, *g);
            }
        }
        self.mark_dirty();
        self.status = format!(
            "Ungrouped {} group{}",
            groups.len(),
            if groups.len() == 1 { "" } else { "s" }
        );
    }

    /// Drops group members that no longer exist and groups left with fewer
    /// than two (after objects were deleted).
    pub fn prune_dead_groups(&mut self) {
        if self.floor().groups.is_empty() {
            return;
        }
        let fl = self.floor;
        let dead: Vec<CoreRef> = self
            .floor()
            .groups
            .iter()
            .flat_map(|g| g.members.iter().copied())
            .filter(|m| !ObjectRef::from_group_ref(*m).exists_in(&self.project, fl))
            .collect();
        if !dead.is_empty() {
            self.project.floors[fl].prune_groups(|g| !dead.contains(&g));
        }
    }

    // ----- layers -----

    /// The layers the selected objects are on.
    pub fn selection_layers(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for o in &self.selection.items {
            if let Some(l) = layer_of(self.floor(), *o) {
                if !out.contains(&l) {
                    out.push(l);
                }
            }
        }
        out
    }

    /// Lock / Unlock: the layers of the selected objects are locked (or
    /// unlocked), so nothing on them can be edited until unlocked (S-5).
    pub fn set_selection_locked(&mut self, lock: bool) {
        let layers = self.selection_layers();
        if layers.is_empty() {
            self.status = "Select objects first".into();
            return;
        }
        // Nothing to do (already so, or a layer the plan lacks): say so and
        // leave no undo step (QA-26).
        let needs = |l: &String| {
            self.project
                .layers
                .get(l)
                .is_some_and(|layer| layer.locked != lock)
        };
        if !layers.iter().any(needs) {
            self.status = if layers.iter().all(|l| self.project.layers.get(l).is_none()) {
                "These objects are not on a layer that can be locked".into()
            } else if lock {
                "Already locked".into()
            } else {
                "Nothing is locked".into()
            };
            return;
        }
        self.begin_change(if lock { "Lock" } else { "Unlock" });
        for l in &layers {
            self.project.layers.set_locked(l, lock);
        }
        if lock {
            // Locked objects cannot be edited, so they leave the selection.
            self.selection.clear();
        }
        self.mark_dirty();
        self.status = format!(
            "{} layer{}: {}",
            if lock { "Locked" } else { "Unlocked" },
            if layers.len() == 1 { "" } else { "s" },
            layers.join(", ")
        );
    }

    /// Send to Layer: moves walls, CAD, text and symbols onto `layer` (the
    /// other kinds live on the layer of their kind). One undo step.
    pub fn send_selection_to_layer(&mut self, layer: &str) -> usize {
        let items = self.selection.items.clone();
        self.move_objects_to_layer(&items, layer, "Send to Layer")
    }

    /// Moves `items` (walls, CAD, text and symbols of the active floor) onto
    /// `layer` as one undo step called `label`; the Layer Painter calls this
    /// for every click. Returns how many moved.
    pub fn move_objects_to_layer(
        &mut self,
        items: &[ObjectRef],
        layer: &str,
        label: &str,
    ) -> usize {
        if self.project.layers.get(layer).is_none() {
            self.status = format!("There is no layer named {layer}");
            return 0;
        }
        if items.iter().any(|o| self.is_object_locked(*o)) {
            self.status = "Those objects are on a locked layer".into();
            return 0;
        }
        self.begin_change(label);
        let fl = self.floor;
        let mut n = 0;
        for o in items {
            match *o {
                ObjectRef::Wall(id) => {
                    if let Some(w) = self.project.floors[fl].wall_mut(id) {
                        w.layer = layer.to_string();
                        n += 1;
                    }
                }
                ObjectRef::Cad(id) | ObjectRef::Text(id) => {
                    if let Some(c) = self.project.floors[fl].cad.iter_mut().find(|c| c.id == id) {
                        c.layer = layer.to_string();
                        n += 1;
                    }
                }
                ObjectRef::Symbol(id) => {
                    if let Some(s) = self.project.floors[fl]
                        .symbols
                        .iter_mut()
                        .find(|s| s.id == id)
                    {
                        s.layer = layer.to_string();
                        n += 1;
                    }
                }
                ObjectRef::Cabinet(id) => {
                    if let Some(mut c) =
                        crate::editor::placed::cabinet_by_id(&self.project.floors[fl], id)
                    {
                        c.layer = Some(layer.to_string());
                        if crate::editor::placed::replace_cabinet(&mut self.project, fl, &c) {
                            n += 1;
                        }
                    }
                }
                _ => {}
            }
        }
        if n == 0 {
            self.cancel_change();
            self.status = "Those objects stay on the layer of their kind".into();
            return 0;
        }
        self.mark_dirty();
        self.status = format!(
            "Moved {n} object{} to {layer}",
            if n == 1 { "" } else { "s" }
        );
        n
    }

    // ----- door and wall commands -----

    /// Flip Hinge: moves the hinge of the selected doors to the other jamb.
    pub fn flip_hinge_selected(&mut self) {
        let ids: Vec<plan_core::Id> = self
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
        if ids.is_empty()
            || ids
                .iter()
                .any(|i| self.is_object_locked(ObjectRef::Opening(*i)))
        {
            return;
        }
        self.begin_change("Flip Hinge");
        let fl = self.floor;
        for id in ids {
            self.project.flip_hinge(fl, id);
        }
        self.mark_dirty();
    }

    /// Reverse Layers: the wall's layers swap sides (the existing wall
    /// command, `wall_edit::reverse_layers`).
    pub fn reverse_layers_selected(&mut self) {
        super::wall_edit::reverse_layers(self);
    }

    // ----- Action History -----

    /// The undo steps (oldest first) and the redo steps (next first).
    pub fn action_history(&self) -> (Vec<String>, Vec<String>) {
        (self.history.past_labels(), self.history.future_labels())
    }

    /// Goes to the state after undo step `index` (0 = the oldest): undoes
    /// the later ones. Returns how many steps went.
    pub fn jump_back_to(&mut self, index: usize) -> usize {
        let steps = self.history.depth().saturating_sub(index + 1);
        for _ in 0..steps {
            self.undo();
        }
        steps
    }

    /// Redoes `steps` steps (the redo entry `steps - 1` was clicked).
    pub fn jump_forward(&mut self, steps: usize) -> usize {
        let mut n = 0;
        for _ in 0..steps {
            if self.redo().is_some() {
                n += 1;
            }
        }
        n
    }

    // ----- toolbar buttons and the context menu -----

    /// The contextual Edit toolbar buttons that work on any selection.
    pub fn selection_edit_actions(&self) -> Vec<EditAction> {
        let mut v = Vec::new();
        if self.groupable().len() >= 2 {
            v.push(custom_button(ids::GROUP, "Group Selected Objects"));
        }
        if self.selection_in_group() {
            v.push(custom_button(ids::UNGROUP, "Ungroup"));
        }
        v.push(custom_button(ids::SELECT_SAME, "Select Same Type"));
        if crate::plan_defaults::can_set_as_default(self) {
            v.push(custom_button(
                crate::plan_defaults::SET_AS_DEFAULT,
                "Set as Default",
            ));
        }
        v.extend(crate::dialogs::materials_list::edit_buttons(self));
        v.extend(super::camera_edit::edit_buttons(self));
        v.extend(super::stairs_view::staircase::edit_buttons(self));
        if crate::tools::painters::can_match(self) {
            v.push(custom_button(
                crate::tools::painters::MATCH_PROPERTIES,
                "Match Properties",
            ));
        }
        v.push(custom_button(ids::TRANSFORM, "Transform/Replicate Object"));
        v.push(custom_button(ids::REFLECT, "Reflect About Object"));
        v.push(custom_button(ids::POINT_TO_POINT, "Point to Point Move"));
        v.push(custom_button(ids::CENTER, "Center Object"));
        v.push(custom_button(
            ids::POINT_TO_POINT_CENTER,
            "Point to Point Center",
        ));
        // Chief orders the group Transform, Reflect, Point to Point Move,
        // Center Object, Align/Distribute, then the line tools.
        if self.selection.len() >= 2 {
            v.push(custom_button(ids::ALIGN_DIALOG, "Align/Distribute"));
        }
        let straight = self.selection.items.iter().any(|o| match o {
            ObjectRef::Wall(_) => true,
            ObjectRef::Cad(id) | ObjectRef::Text(id) => self
                .floor()
                .cad
                .iter()
                .any(|c| c.id == *id && matches!(c.item, plan_core::cad::CadItem::Line { .. })),
            _ => false,
        });
        if straight {
            v.push(custom_button(ids::PARALLEL, "Make Parallel"));
            v.push(custom_button(ids::PERPENDICULAR, "Make Perpendicular"));
        }
        v.push(custom_button(ids::LAYER, "Layer"));
        if self.selection_locks_something() {
            v.push(custom_button(ids::LOCK, "Lock"));
        } else {
            v.push(custom_button(ids::UNLOCK, "Unlock"));
        }
        v
    }

    /// Is some selected object on an unlocked layer (so Lock is the command)?
    fn selection_locks_something(&self) -> bool {
        self.selection
            .items
            .iter()
            .any(|o| !self.is_object_locked(*o))
    }

    /// The rows of the right-click menu for the current selection (S-8,
    /// DW-109). `toolbar` is the Edit toolbar of the object, whose buttons
    /// head the menu; with nothing selected the menu is the view/paste one.
    /// `ghost_tool` is true while a door or window tool shows its ghost: the
    /// menu then starts with Select Objects.
    pub fn context_entries(&self, toolbar: &[EditAction], ghost_tool: bool) -> Vec<ContextEntry> {
        let mut v: Vec<ContextEntry> = Vec::new();
        if ghost_tool {
            v.push(ContextEntry::new(
                "Select Objects",
                Action::SetTool(ToolId::Select),
            ));
        }
        if self.selection.is_empty() {
            let sep = ghost_tool;
            let mut paste = ContextEntry::cmd("Paste", ids::PASTE).when(self.has_clipboard());
            paste.sep_before = sep;
            v.push(paste);
            v.push(
                ContextEntry::cmd("Paste Hold Position", ids::PASTE_HOLD)
                    .when(self.has_clipboard()),
            );
            v.push(ContextEntry::cmd("Select All", ids::SELECT_ALL).sep());
            v.push(
                ContextEntry::new(
                    match self.undo_label() {
                        Some(l) => format!("Undo {l}"),
                        None => "Undo".into(),
                    },
                    Action::Undo,
                )
                .when(self.can_undo())
                .sep(),
            );
            v.push(
                ContextEntry::new(
                    match self.redo_label() {
                        Some(l) => format!("Redo {l}"),
                        None => "Redo".into(),
                    },
                    Action::Redo,
                )
                .when(self.can_redo()),
            );
            v.push(ContextEntry::new("Zoom In", Action::ZoomIn).sep());
            v.push(ContextEntry::new("Zoom Out", Action::ZoomOut));
            v.push(ContextEntry::new("Fill Window", Action::FillWindow));
            return v;
        }

        // The Edit toolbar's own buttons of this object type come first.
        let mut first = true;
        let mut push_first = |v: &mut Vec<ContextEntry>, e: ContextEntry| {
            if !v.iter().any(|x| x.action == e.action || x.label == e.label) {
                let mut e = e;
                e.sep_before = first && !v.is_empty();
                first = false;
                v.push(e);
            }
        };
        if self.selection.single().is_some() {
            push_first(&mut v, ContextEntry::cmd("Open Object", ids::OPEN));
        }
        // A schedule can be exported for editing in Excel and imported back.
        if matches!(self.selection.single(), Some(ObjectRef::Schedule(_))) {
            push_first(
                &mut v,
                ContextEntry::cmd(
                    "Export for Editing (XLSX)\u{2026}",
                    crate::dialogs::property_manager::EXPORT_SELECTED,
                ),
            );
            push_first(
                &mut v,
                ContextEntry::cmd(
                    "Import Property Data (XLSX)\u{2026}",
                    crate::dialogs::property_manager::IMPORT,
                ),
            );
        }
        for a in toolbar {
            let action = match a.kind {
                EditActionKind::Custom { id, .. } | EditActionKind::Command { id, .. } => {
                    Action::Custom(id)
                }
                EditActionKind::ReverseSwing => Action::Custom(ids::REVERSE_SWING),
                EditActionKind::FixWallConnections => Action::Custom(ids::FIX_WALLS),
                // The shared buttons have rows of their own below.
                _ => continue,
            };
            // Group and layer buttons repeat below.
            if matches!(
                action,
                Action::Custom(
                    ids::GROUP
                        | ids::UNGROUP
                        | ids::SELECT_SAME
                        | ids::TRANSFORM
                        | ids::LAYER
                        | ids::LOCK
                        | ids::UNLOCK
                )
            ) {
                continue;
            }
            push_first(&mut v, ContextEntry::new(a.label, action).when(a.enabled));
        }
        let has_door = self.selection.items.iter().any(|o| match o {
            ObjectRef::Opening(id) => self
                .floor()
                .openings
                .iter()
                .any(|x| x.id == *id && x.kind == OpeningKind::Door),
            _ => false,
        });
        if has_door {
            push_first(
                &mut v,
                ContextEntry::cmd("Reverse Swing", ids::REVERSE_SWING),
            );
            push_first(&mut v, ContextEntry::cmd("Flip Hinge", ids::FLIP_HINGE));
        }
        if self
            .selection
            .items
            .iter()
            .any(|o| matches!(o, ObjectRef::Wall(_)))
        {
            push_first(
                &mut v,
                ContextEntry::cmd("Reverse Layers", ids::REVERSE_LAYERS),
            );
        }
        if self.selection_in_cad_block() {
            push_first(&mut v, ContextEntry::cmd("Explode", ids::EXPLODE));
        }
        if self
            .selection
            .items
            .iter()
            .any(|o| matches!(o, ObjectRef::RoofPlane(_)))
        {
            push_first(
                &mut v,
                ContextEntry::cmd("Rebuild Roofs", super::dispatch::cmd::ROOF_REBUILD),
            );
        }

        // Clipboard and delete.
        let mut cut = ContextEntry::cmd("Cut", ids::CUT).sep();
        cut.sep_before = !v.is_empty();
        v.push(cut);
        v.push(ContextEntry::cmd("Copy", ids::COPY));
        v.push(ContextEntry::cmd("Paste", ids::PASTE).when(self.has_clipboard()));
        v.push(ContextEntry::cmd("Delete", ids::DELETE));

        // Selection and arrangement.
        v.push(ContextEntry::cmd("Select Same Type", ids::SELECT_SAME).sep());
        if self.groupable().len() >= 2 {
            v.push(ContextEntry::cmd("Group", ids::GROUP));
        }
        if self.selection_in_group() {
            v.push(ContextEntry::cmd("Ungroup", ids::UNGROUP));
        }
        if self.selection_locks_something() {
            v.push(ContextEntry::cmd("Lock", ids::LOCK));
        } else {
            v.push(ContextEntry::cmd("Unlock", ids::UNLOCK));
        }
        v.push(ContextEntry::cmd("Send to Layer\u{2026}", ids::LAYER));
        v.push(ContextEntry::cmd(
            "Transform/Replicate Object\u{2026}",
            ids::TRANSFORM,
        ));
        v
    }

    // ----- the command router -----

    /// Runs the `edit.*` command `id` (see [`ids`]).
    pub fn run_edit_command(&mut self, id: &str) {
        self.undo_group(|cx| cx.run_edit_command_ungrouped(id));
    }

    fn run_edit_command_ungrouped(&mut self, id: &str) {
        match id {
            ids::CUT => self.cut_selection(),
            ids::COPY => self.copy_selection(),
            ids::PASTE => self.begin_paste(false),
            ids::PASTE_GROUP => self.begin_paste(true),
            ids::PASTE_HOLD => {
                self.paste_at(Point::ZERO, "Paste Hold Position", false);
            }
            ids::COPY_PASTE_IN_PLACE => {
                if self.selection.is_empty() {
                    self.status = "Nothing to copy".into();
                } else {
                    let clip = Clipboard::capture(self);
                    if !clip.is_empty() {
                        self.store_clipboard(clip);
                        self.paste_at(Point::ZERO, "Copy and Paste in Place", false);
                    }
                }
            }
            ids::DUPLICATE => self.duplicate_selection(),
            ids::DELETE => self.delete_selection(),
            ids::DELETE_OBJECTS => crate::dialogs::delete_objects::open(),
            ids::SELECT_ALL => self.select_all_objects(),
            ids::SELECT_SAME => self.select_same_type(),
            ids::GROUP => self.group_selection(),
            ids::UNGROUP | ids::EXPLODE => self.ungroup_selection(),
            ids::LOCK => self.set_selection_locked(true),
            ids::UNLOCK => self.set_selection_locked(false),
            ids::LAYER => crate::dialogs::send_to_layer::open(self),
            ids::TRANSFORM => crate::dialogs::transform::open(self),
            ids::ROTATE => {
                crate::dialogs::transform::open_rotate(self);
            }
            ids::REFLECT | ids::REFLECT_COPY => {
                if self.selection.is_empty() {
                    self.status = "Select objects to reflect".into();
                } else {
                    let copy = id == ids::REFLECT_COPY;
                    transform::begin_mode(self, Mode::Reflect { copy });
                    self.requests.push(EditorRequest::SetTool(ToolId::Select));
                }
            }
            ids::POINT_TO_POINT => {
                if self.selection.is_empty() {
                    self.status = "Select objects to move".into();
                } else {
                    transform::begin_mode(
                        self,
                        Mode::PointToPoint {
                            from: None,
                            to: None,
                        },
                    );
                    self.requests.push(EditorRequest::SetTool(ToolId::Select));
                }
            }
            ids::POINT_TO_POINT_CENTER => {
                if self.selection.is_empty() {
                    self.status = "Select objects to center".into();
                } else {
                    transform::begin_mode(self, Mode::PointToPointCenter { a: None });
                    self.requests.push(EditorRequest::SetTool(ToolId::Select));
                }
            }
            ids::CENTER => {
                if self.selection.is_empty() {
                    self.status = "Select objects to center".into();
                } else if matches!(self.selection.single(), Some(ObjectRef::Opening(_))) {
                    // A door or window centers on its wall at once.
                    if let Err(e) = transform::center_opening_in_wall(self) {
                        self.status = e;
                    }
                } else {
                    transform::begin_mode(self, Mode::Center { first_wall: None });
                    self.requests.push(EditorRequest::SetTool(ToolId::Select));
                }
            }
            ids::PARALLEL | ids::PERPENDICULAR => {
                if self.selection.is_empty() {
                    self.status = "Select walls or lines first".into();
                } else {
                    transform::begin_mode(
                        self,
                        Mode::Parallel {
                            perpendicular: id == ids::PERPENDICULAR,
                        },
                    );
                    self.requests.push(EditorRequest::SetTool(ToolId::Select));
                }
            }
            ids::DISTRIBUTE_H | ids::DISTRIBUTE_V => {
                let axis = if id == ids::DISTRIBUTE_H {
                    Axis::Horizontal
                } else {
                    Axis::Vertical
                };
                match transform::distribute_selection(self, axis, None) {
                    Ok(n) => self.status = format!("Moved {n} objects"),
                    Err(e) => self.status = e,
                }
            }
            ids::FRONT | ids::BACK => {
                let n = transform::reorder_cad(self, id == ids::FRONT);
                if n == 0 {
                    self.status = "Move to Front/Back applies to CAD objects".into();
                }
            }
            ids::HISTORY => crate::dialogs::action_history::toggle(),
            ids::ALIGN_DIALOG => crate::dialogs::transform::open_align(self),
            ids::OPEN => {
                if let Some(o) = self.selection.single() {
                    self.requests.push(EditorRequest::OpenSpec(o));
                }
            }
            ids::REVERSE_SWING => self.reverse_swing(),
            ids::FLIP_HINGE => self.flip_hinge_selected(),
            ids::REVERSE_LAYERS => self.reverse_layers_selected(),
            ids::FIX_WALLS => super::connect::fix_wall_connections_action(self),
            other => {
                if let Some(mode) = AlignMode::ALL.into_iter().find(|m| m.id() == other) {
                    match transform::align_selection(self, mode) {
                        Ok(n) => self.status = format!("Aligned {n} objects"),
                        Err(e) => self.status = e,
                    }
                }
            }
        }
    }
}
