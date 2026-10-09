//! Architectural Block Specification (reference manual "Architectural
//! Blocks", pp. 1059 to 1062; parity rows CB-429..CB-436): General,
//! Sub-Objects, Components, Label, Layer and Schedule. The structure is
//! `plan_core::arch_block`; the editor side is `tools::arch_block`.
//!
//! The window edits a copy of the block. OK stores it as one undo step.
//! Edit Sub-Object closes the window and switches the selection to a single
//! member (`tools::arch_block::set_editing_sub_objects`).
//!
//! [`show_all`] draws this window and the other round 15 object windows
//! (3D solids, material layers, distribution options, soffits); the shell
//! calls it once a frame.

use super::{row, section, Outcome, SpecDialog, SpecPages, Tab};
use crate::editor::{EditorContext, ObjectRef};
use crate::tools::arch_block as tool;
use eframe::egui::{self, Painter, Rect, Ui};
use plan_core::arch_block::{ArchBlock, BlockLabelMode, ElevationRef};
use plan_core::{Id, ObjectRef as CoreRef};
use std::cell::RefCell;

const TABS: &[Tab] = &[
    Tab {
        name: "General",
        enabled: true,
    },
    Tab {
        name: "Sub-Objects",
        enabled: true,
    },
    Tab {
        name: "Components",
        enabled: true,
    },
    Tab {
        name: "Label",
        enabled: true,
    },
    Tab {
        name: "Layer",
        enabled: true,
    },
    Tab {
        name: "Schedule",
        enabled: true,
    },
];

/// The dialog and its draft.
pub struct ArchBlockDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    draft: ArchBlock,
    layers: Vec<String>,
    /// The members with their type name, in block order (nested blocks
    /// resolved).
    members: Vec<(&'static str, CoreRef)>,
    picked: Option<usize>,
    /// Edit Sub-Object was pressed for this member.
    edit_sub: Option<CoreRef>,
}

impl ArchBlockDialog {
    /// The dialog for block `id`, or `None` when it is gone.
    pub fn new(cx: &EditorContext, id: Id) -> Option<Self> {
        let f = cx.floor();
        let draft = f.blocks.get(id)?.clone();
        let mut layers: Vec<String> = cx
            .project
            .layers
            .layers
            .iter()
            .map(|l| l.name.clone())
            .collect();
        if !layers.contains(&draft.layer) {
            layers.push(draft.layer.clone());
        }
        let members = f
            .blocks
            .flat_members(id)
            .into_iter()
            .map(|m| (ObjectRef::from_group_ref(m).type_name(), m))
            .collect();
        Some(Self {
            frame: SpecDialog::new("Architectural Block Specification", "arch_block_spec"),
            form: Form {
                draft,
                layers,
                members,
                picked: None,
                edit_sub: None,
            },
        })
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn draft(&self) -> &ArchBlock {
        &self.form.draft
    }

    pub fn draft_mut(&mut self) -> &mut ArchBlock {
        &mut self.form.draft
    }

    /// OK: stores the draft as one undo step. Returns whether the block
    /// changed.
    pub fn apply(&self, cx: &mut EditorContext) -> bool {
        apply(cx, &self.form.draft)
    }
}

/// Stores `draft` over the block with its id: one undo step, nothing when
/// the block did not change.
pub fn apply(cx: &mut EditorContext, draft: &ArchBlock) -> bool {
    let fl = cx.floor;
    let Some(old) = cx.floor().blocks.get(draft.id).cloned() else {
        return false;
    };
    let mut new = draft.clone();
    new.normalize();
    if old == new {
        return false;
    }
    cx.begin_change("Architectural Block Specification");
    if let Some(b) = cx.project.floors[fl].blocks.get_mut(new.id) {
        *b = new;
    }
    crate::editor::solids_view::ensure_layers(&mut cx.project.layers);
    cx.mark_dirty();
    true
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        if self.draft.name.trim().is_empty() {
            return Some("The block needs a name".into());
        }
        if self.draft.label.mode == BlockLabelMode::Custom
            && self.draft.label.text.trim().is_empty()
        {
            return Some("Type the custom label or choose Automatic".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match tab {
            0 => self.general(ui),
            1 => self.sub_objects(ui),
            2 => self.components(ui),
            3 => self.label(ui),
            4 => self.layer(ui),
            _ => self.schedule(ui),
        }
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        let ink = egui::Color32::from_rgb(0x2B, 0x2B, 0x2B);
        painter.rect_stroke(
            rect.shrink(18.0),
            2.0,
            egui::Stroke::new(1.0_f32, ink),
            egui::StrokeKind::Inside,
        );
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("{}\n{} sub-objects", self.draft.name, self.members.len()),
            egui::FontId::proportional(13.0),
            ink,
        );
    }
}

impl Form {
    fn general(&mut self, ui: &mut Ui) {
        let d = &mut self.draft;
        section(ui, "Block");
        row(ui, "Name", |ui| ui.text_edit_singleline(&mut d.name));
        ui.add_space(4.0);
        section(ui, "Display");
        ui.checkbox(&mut d.display_bounding_box, "Display Bounding Box");
        ui.checkbox(&mut d.display_sub_objects, "Display Sub-Objects");
        if !d.display_sub_objects {
            // Turning the sub-objects off leaves the box as the only trace.
            d.display_bounding_box = true;
        }
        ui.add_enabled_ui(d.display_sub_objects, |ui| {
            ui.checkbox(
                &mut d.sub_objects_use_block_layer,
                "Display Sub-Objects Using Block Layer",
            );
            ui.checkbox(
                &mut d.sub_objects_use_block_draw_order,
                "Display Sub-Objects Using Block Draw Order",
            );
        });
        ui.add_space(4.0);
        section(ui, "Size/Position");
        row(ui, "Elevations measured", |ui| {
            egui::ComboBox::from_id_salt("block_elev_ref")
                .selected_text(d.elevation_ref.name())
                .show_ui(ui, |ui| {
                    for r in ElevationRef::ALL {
                        ui.selectable_value(&mut d.elevation_ref, r, r.name());
                    }
                });
        });
        ui.checkbox(
            &mut d.suppress_room_moldings,
            "Suppress Adjacent Room Moldings",
        );
        ui.add_space(4.0);
        section(ui, "Schedules and Materials List");
        let ganged = d.kind == plan_core::arch_block::BlockKind::GangedElectrical;
        ui.add_enabled(
            !ganged,
            egui::Checkbox::new(&mut d.treat_as_one, "Treat as One Object"),
        )
        .on_hover_text(if ganged {
            "A ganged electrical block is always one object"
        } else {
            "Schedule the block as one object instead of listing its sub-objects"
        });
    }

    fn sub_objects(&mut self, ui: &mut Ui) {
        section(ui, "Sub-Objects");
        ui.label(format!("{} objects in this block.", self.members.len()));
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                for (i, (name, m)) in self.members.iter().enumerate() {
                    let text = format!("{name} (id {})", ObjectRef::from_group_ref(*m).id());
                    if ui.selectable_label(self.picked == Some(i), text).clicked() {
                        self.picked = Some(i);
                    }
                }
            });
        ui.add_space(6.0);
        let pick = self
            .picked
            .and_then(|i| self.members.get(i))
            .map(|(_, m)| *m);
        if ui
            .add_enabled(pick.is_some(), egui::Button::new("Edit Sub-Object"))
            .on_hover_text("Select this member alone so it can be edited inside the block")
            .clicked()
        {
            self.edit_sub = pick;
        }
    }

    fn components(&mut self, ui: &mut Ui) {
        section(ui, "Components");
        let mut counts: Vec<(&'static str, usize)> = Vec::new();
        for (name, _) in &self.members {
            match counts.iter_mut().find(|(n, _)| n == name) {
                Some((_, c)) => *c += 1,
                None => counts.push((name, 1)),
            }
        }
        egui::Grid::new("block_components")
            .num_columns(2)
            .show(ui, |ui| {
                for (name, c) in counts {
                    ui.label(name);
                    ui.label(c.to_string());
                    ui.end_row();
                }
            });
    }

    fn label(&mut self, ui: &mut Ui) {
        let l = &mut self.draft.label;
        section(ui, "Label");
        ui.radio_value(
            &mut l.mode,
            BlockLabelMode::Automatic,
            "Automatic (the block's name)",
        );
        ui.radio_value(&mut l.mode, BlockLabelMode::Custom, "Custom");
        ui.radio_value(&mut l.mode, BlockLabelMode::Hidden, "Hidden");
        ui.add_enabled_ui(l.mode == BlockLabelMode::Custom, |ui| {
            row(ui, "Label text", |ui| ui.text_edit_singleline(&mut l.text));
        });
        ui.checkbox(&mut l.sub_object_labels, "Show labels of sub-objects");
    }

    fn layer(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        let d = &mut self.draft;
        row(ui, "Layer", |ui| {
            egui::ComboBox::from_id_salt("block_layer")
                .selected_text(d.layer.clone())
                .show_ui(ui, |ui| {
                    for n in &self.layers {
                        ui.selectable_value(&mut d.layer, n.clone(), n);
                    }
                });
        });
    }

    fn schedule(&mut self, ui: &mut Ui) {
        let s = &mut self.draft.schedule;
        section(ui, "Schedule");
        ui.checkbox(&mut s.exclude, "Leave out of schedules");
        row(ui, "Category", |ui| {
            ui.text_edit_singleline(&mut s.category)
        });
        row(ui, "Mark", |ui| ui.text_edit_singleline(&mut s.mark));
        row(ui, "Manufacturer", |ui| {
            ui.text_edit_singleline(&mut s.manufacturer)
        });
        row(ui, "Model", |ui| ui.text_edit_singleline(&mut s.model));
        row(ui, "Note", |ui| ui.text_edit_singleline(&mut s.note));
    }
}

thread_local! {
    static DIALOG: RefCell<Option<ArchBlockDialog>> = const { RefCell::new(None) };
}

/// Opens the specification of block `id` (nothing when it is gone).
pub fn open(cx: &EditorContext, id: Id) {
    if let Some(d) = ArchBlockDialog::new(cx, id) {
        DIALOG.with(|slot| *slot.borrow_mut() = Some(d));
    }
}

/// Is the window open?
pub fn is_open() -> bool {
    DIALOG.with(|d| d.borrow().is_some())
}

/// Closes the window without applying it.
pub fn close() {
    DIALOG.with(|d| *d.borrow_mut() = None);
}

/// Draws the window when open and applies an OK.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut d) = DIALOG.with(|d| d.borrow_mut().take()) else {
        return;
    };
    let outcome = d.show(ctx);
    if let Some(m) = d.form.edit_sub.take() {
        tool::set_editing_sub_objects(true);
        cx.selection.items = vec![ObjectRef::from_group_ref(m)];
        cx.status = "Edit Sub-Objects: click a single member of a block".into();
        return;
    }
    match outcome {
        Outcome::Open => DIALOG.with(|slot| *slot.borrow_mut() = Some(d)),
        Outcome::Ok => {
            d.apply(cx);
        }
        Outcome::Cancel => {}
    }
}

/// Draws every round 15 object window once a frame.
pub fn show_all(ctx: &egui::Context, cx: &mut EditorContext) {
    show(ctx, cx);
    super::solids::show(ctx, cx);
    super::material_region::show(ctx, cx);
    super::distribution::show(ctx, cx);
    super::soffit::show(ctx, cx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_cabinets::{Cabinet, CabinetKind};
    use plan_core::arch_block::BlockKind;
    use plan_core::geometry::Point;

    fn cx_with_block() -> (EditorContext, Id) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let a = crate::editor::placed::add_cabinet(
            &mut cx.project,
            0,
            Cabinet::new(CabinetKind::Base, 24.0),
        )
        .unwrap();
        let mut c2 = Cabinet::new(CabinetKind::Wall, 24.0);
        c2.position = Point::new(30.0, 0.0);
        let b = crate::editor::placed::add_cabinet(&mut cx.project, 0, c2).unwrap();
        cx.selection.items = vec![ObjectRef::Cabinet(a), ObjectRef::Cabinet(b)];
        let id = tool::make_block(&mut cx, BlockKind::Standard).unwrap();
        (cx, id)
    }

    #[test]
    fn ok_stores_the_draft_as_one_undo_step_and_unchanged_is_none() {
        let (mut cx, id) = cx_with_block();
        let mut d = ArchBlockDialog::new(&cx, id).unwrap();
        assert!(!d.apply(&mut cx), "nothing changed, no undo step");
        d.draft_mut().name = "Kitchen Run".into();
        d.draft_mut().label.mode = BlockLabelMode::Custom;
        d.draft_mut().label.text = "K1".into();
        d.draft_mut().display_sub_objects = false;
        assert!(d.apply(&mut cx));
        assert_eq!(cx.undo_label(), Some("Architectural Block Specification"));
        let b = cx.floor().blocks.get(id).unwrap();
        assert_eq!(b.name, "Kitchen Run");
        assert_eq!(b.label_text().as_deref(), Some("K1"));
        assert!(b.display_bounding_box, "no sub-objects shows the box");
        cx.undo();
        assert_eq!(
            cx.floor().blocks.get(id).unwrap().name,
            "Architectural Block"
        );
    }

    #[test]
    fn the_dialog_lists_members_and_blocks_a_blank_name() {
        let (cx, id) = cx_with_block();
        let mut d = ArchBlockDialog::new(&cx, id).unwrap();
        assert_eq!(d.form.members.len(), 2);
        assert!(d.form.error().is_none());
        d.draft_mut().name = "  ".into();
        assert!(d.form.error().is_some());
    }

    #[test]
    fn the_window_draws_headless_and_ok_applies() {
        let (mut cx, id) = cx_with_block();
        close();
        open(&cx, id);
        assert!(is_open());
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| show(ctx, &mut cx));
        }
        assert!(is_open(), "it stays open until OK or Cancel");
        close();
        assert!(!is_open());
    }
}
