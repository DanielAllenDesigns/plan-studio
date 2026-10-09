//! The Saved Defaults dialog of a tool (manual p. 105): "Available Saved
//! Defaults" (Edit, Copy, Rename, Delete, Select All, Clear All; several can
//! be selected with Shift or Ctrl/Cmd) and "Currently Active Saved Defaults"
//! (a drop-down and an Edit button). Cancel drops every change made in the
//! dialog.
//!
//! Double-clicking the line of Manual Dimensions, Text, Callouts, Markers (and
//! the other multi-default tools) in Default Settings opens it; so does a
//! double-click on the tool's toolbar button when Edit Active Default on
//! Double-Click is off ([`double_click`]).

use super::default_sets::{self, PromptFor};
use crate::editor::EditorContext;
use crate::toolbar::Action;
use crate::tools::ToolId;
use eframe::egui::{self, Align, Align2, Layout};
use plan_core::callout::AnnotDefaults;
use plan_core::defaults::saved::{SavedDefaults, SavedKind};
use plan_core::defaults::{DimensionDefaults, PageValue};
use plan_core::SavedPlanView;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};

/// Command id prefix: `defaults.saved.<kind id>` opens the dialog of a kind.
pub const COMMAND_PREFIX: &str = "defaults.saved.";

/// The command id that opens the dialog of `kind` (a static string, so a
/// menu row, a toolbar button or a Default Settings leaf can carry it).
pub const fn command_id(kind: SavedKind) -> &'static str {
    match kind {
        SavedKind::ManualDimensions => "defaults.saved.dimensions",
        SavedKind::Text => "defaults.saved.text",
        SavedKind::RichText => "defaults.saved.rich_text",
        SavedKind::RevisionClouds => "defaults.saved.revision_clouds",
        SavedKind::Callouts => "defaults.saved.callouts",
        SavedKind::Markers => "defaults.saved.markers",
        SavedKind::Notes => "defaults.saved.notes",
        SavedKind::Arrows => "defaults.saved.arrows",
        SavedKind::RoomFunctions => "defaults.saved.room_functions",
        SavedKind::StructuralMemberReporting => "defaults.saved.structural_reporting",
        SavedKind::FramingTypes => "defaults.saved.framing_types",
    }
}

thread_local! {
    static EDIT_ACTIVE: Cell<bool> = const { Cell::new(true) };
    static WINDOW: RefCell<Option<Window>> = const { RefCell::new(None) };
}

/// Preferences > General > Edit Active Default on Double-Click (on by
/// default). Off: a double-click on a tool's button opens its Saved Defaults
/// dialog instead of the active default's dialog.
pub fn edit_active_on_double_click() -> bool {
    EDIT_ACTIVE.with(Cell::get)
}

pub fn set_edit_active_on_double_click(on: bool) {
    EDIT_ACTIVE.with(|c| c.set(on));
}

/// Everything the dialog can change, kept so Cancel can put it back.
struct Snapshot {
    saved: SavedDefaults,
    annot: AnnotDefaults,
    pages: BTreeMap<String, PageValue>,
    dimension_sets: Vec<plan_core::defaults::DimensionDefaultSet>,
    active_dimension_set: String,
    dimensions: DimensionDefaults,
    views: Vec<SavedPlanView>,
}

impl Snapshot {
    fn take(cx: &mut EditorContext) -> Snapshot {
        cx.project.saved_commit_all(&mut cx.defaults);
        Snapshot {
            saved: cx.project.saved_defaults.clone(),
            annot: cx.project.annot_defaults.clone(),
            pages: cx.defaults.pages.clone(),
            dimension_sets: cx.defaults.dimension_sets.clone(),
            active_dimension_set: cx.defaults.active_dimension_set.clone(),
            dimensions: cx.defaults.dimensions.clone(),
            views: cx.project.plan_views.clone(),
        }
    }

    fn restore(self, cx: &mut EditorContext) {
        cx.project.saved_defaults = self.saved;
        cx.project.annot_defaults = self.annot;
        cx.defaults.pages = self.pages;
        cx.defaults.dimension_sets = self.dimension_sets;
        cx.defaults.active_dimension_set = self.active_dimension_set;
        cx.defaults.dimensions = self.dimensions;
        cx.project.plan_views = self.views;
    }
}

struct Window {
    kind: SavedKind,
    selected: BTreeSet<String>,
    anchor: Option<String>,
    snapshot: Snapshot,
    /// The active default the drop-down shows.
    active: String,
    message: String,
}

/// Opens the Saved Defaults dialog of `kind`.
pub fn open(cx: &mut EditorContext, kind: SavedKind) {
    let snapshot = Snapshot::take(cx);
    let active = cx.project.saved_active(&mut cx.defaults, kind);
    let mut selected = BTreeSet::new();
    selected.insert(active.clone());
    WINDOW.with(|w| {
        *w.borrow_mut() = Some(Window {
            kind,
            selected,
            anchor: Some(active.clone()),
            snapshot,
            active,
            message: String::new(),
        })
    });
}

/// Is the dialog open (and for which kind)?
pub fn open_kind() -> Option<SavedKind> {
    WINDOW.with(|w| w.borrow().as_ref().map(|w| w.kind))
}

/// Runs `defaults.saved.<kind>`; false when the id is not one of ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    let Some(kind) = id.strip_prefix(COMMAND_PREFIX).and_then(SavedKind::from_id) else {
        return false;
    };
    open(cx, kind);
    true
}

/// The kind of saved default a tool button belongs to, if it has any.
pub fn kind_of_tool(tool: ToolId) -> Option<SavedKind> {
    use crate::tools::cad::CadMode;
    use crate::tools::text::TextMode;
    Some(match tool {
        ToolId::Dimension | ToolId::DimensionVariant(_) => SavedKind::ManualDimensions,
        ToolId::Text | ToolId::TextVariant(TextMode::Text) => SavedKind::Text,
        ToolId::TextVariant(TextMode::RichText) => SavedKind::RichText,
        ToolId::TextVariant(TextMode::Callout) => SavedKind::Callouts,
        ToolId::TextVariant(TextMode::Marker) => SavedKind::Markers,
        ToolId::TextVariant(TextMode::Note) => SavedKind::Notes,
        ToolId::TextVariant(TextMode::ArrowLine) => SavedKind::Arrows,
        ToolId::CadVariant(CadMode::RevisionCloud) => SavedKind::RevisionClouds,
        _ => return None,
    })
}

/// A double-click on a tool's toolbar button. A tool with multiple saved
/// defaults opens the defaults dialog of the active one, or its Saved
/// Defaults dialog when Edit Active Default on Double-Click is off. Returns
/// whether the action was one of those tools.
pub fn double_click(cx: &mut EditorContext, action: Action) -> bool {
    let Action::SetTool(tool) = action else {
        return false;
    };
    let Some(kind) = kind_of_tool(tool) else {
        return false;
    };
    if edit_active_on_double_click() {
        let name = cx.project.saved_active(&mut cx.defaults, kind);
        default_sets::edit_saved(cx, kind, &name);
    } else {
        open(cx, kind);
    }
    true
}

/// Draws the dialog when it is open; call once a frame.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut w) = WINDOW.with(|w| w.borrow_mut().take()) else {
        return;
    };
    let names = cx.project.saved_names(&mut cx.defaults, w.kind);
    w.selected.retain(|n| names.contains(n));
    if !names.contains(&w.active) {
        w.active = cx.project.saved_active(&mut cx.defaults, w.kind);
    }
    let mut ok = false;
    let mut cancel = false;
    let mut open = true;
    let mut action: Option<Act> = None;
    let mut active_pick = w.active.clone();
    let modifiers = ctx.input(|i| i.modifiers);
    egui::Window::new(format!("Saved {} Defaults", w.kind.label()))
        .id(egui::Id::new("saved_defaults_dialog"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.strong("Available Saved Defaults");
            egui::ScrollArea::vertical()
                .id_salt("saved_defaults_list")
                .max_height(220.0)
                .min_scrolled_height(120.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for (i, n) in names.iter().enumerate() {
                        let on = w.selected.contains(n);
                        let r = ui.selectable_label(on, n);
                        if r.clicked() {
                            if modifiers.shift {
                                // A range from the anchor.
                                let a = w
                                    .anchor
                                    .as_ref()
                                    .and_then(|a| names.iter().position(|x| x == a))
                                    .unwrap_or(i);
                                let (lo, hi) = (a.min(i), a.max(i));
                                w.selected = names[lo..=hi].iter().cloned().collect();
                            } else if modifiers.command || modifiers.ctrl {
                                if !w.selected.remove(n) {
                                    w.selected.insert(n.clone());
                                }
                                w.anchor = Some(n.clone());
                            } else {
                                w.selected.clear();
                                w.selected.insert(n.clone());
                                w.anchor = Some(n.clone());
                            }
                        }
                        if r.double_clicked() {
                            action = Some(Act::Edit);
                        }
                    }
                });
            ui.horizontal(|ui| {
                let one = w.selected.len() == 1;
                let any = !w.selected.is_empty();
                if ui.add_enabled(any, egui::Button::new("Edit")).clicked() {
                    action = Some(Act::Edit);
                }
                if ui.add_enabled(any, egui::Button::new("Copy")).clicked() {
                    action = Some(Act::Copy);
                }
                if ui.add_enabled(one, egui::Button::new("Rename")).clicked() {
                    action = Some(Act::Rename);
                }
                if ui.add_enabled(any, egui::Button::new("Delete")).clicked() {
                    action = Some(Act::Delete);
                }
                if ui.button("Select All").clicked() {
                    w.selected = names.iter().cloned().collect();
                }
                if ui.button("Clear All").clicked() {
                    w.selected.clear();
                }
            });
            ui.add_space(6.0);
            ui.strong("Currently Active Saved Defaults");
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("saved_defaults_active")
                    .selected_text(active_pick.clone())
                    .width(260.0)
                    .show_ui(ui, |ui| {
                        for n in &names {
                            ui.selectable_value(&mut active_pick, n.clone(), n);
                        }
                    });
                if ui.button("Edit").clicked() {
                    action = Some(Act::EditActive);
                }
            });
            ui.add_space(4.0);
            let mut on = edit_active_on_double_click();
            if ui
                .checkbox(&mut on, "Edit Active Default on Double-Click")
                .on_hover_text("Off: double-clicking the tool's button opens this dialog")
                .changed()
            {
                set_edit_active_on_double_click(on);
            }
            if !w.message.is_empty() {
                ui.label(&w.message);
            }
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("   OK   ").clicked() {
                    ok = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
    if active_pick != w.active {
        cx.project
            .saved_activate(&mut cx.defaults, w.kind, &active_pick);
        w.active = active_pick;
    }
    if let Some(a) = action {
        let first = w.selected.iter().next().cloned();
        match a {
            Act::Edit => {
                if let Some(n) = first {
                    default_sets::edit_saved(cx, w.kind, &n);
                    w.active = n;
                }
            }
            Act::EditActive => {
                let n = w.active.clone();
                default_sets::edit_saved(cx, w.kind, &n);
            }
            Act::Copy => {
                if w.selected.len() == 1 {
                    if let Some(n) = first {
                        default_sets::prompt(
                            &format!("New {} Default Name", w.kind.label()),
                            PromptFor::AddSaved(w.kind, n.clone()),
                            &format!("{n} Copy"),
                        );
                    }
                } else {
                    let mut made = 0;
                    for n in w.selected.clone() {
                        let name = format!("{n} 2");
                        let unique =
                            unique_name(&name, &cx.project.saved_names(&mut cx.defaults, w.kind));
                        if cx
                            .project
                            .saved_copy(&mut cx.defaults, w.kind, &n, &unique)
                            .is_ok()
                        {
                            made += 1;
                        }
                    }
                    w.message = format!("Copied {made} saved defaults");
                }
            }
            Act::Rename => {
                if let Some(n) = first {
                    default_sets::prompt(
                        "Rename Current Default",
                        PromptFor::RenameSaved(w.kind, n.clone()),
                        &n,
                    );
                }
            }
            Act::Delete => {
                let mut msgs = Vec::new();
                for n in w.selected.clone() {
                    match cx.project.saved_delete(&mut cx.defaults, w.kind, &n) {
                        Ok(()) => {
                            w.selected.remove(&n);
                        }
                        Err(e) => msgs.push(e),
                    }
                }
                w.active = cx.project.saved_active(&mut cx.defaults, w.kind);
                w.message = msgs.join("; ");
            }
        }
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape))
        && !default_sets::prompt_is_open()
        && !default_sets::editor_is_open()
    {
        cancel = true;
    }
    if ok {
        cx.project.saved_commit(&mut cx.defaults, w.kind);
        cx.mark_dirty();
        cx.status = format!(
            "{} defaults: \"{}\" is active",
            w.kind.label(),
            cx.project.saved_active(&mut cx.defaults, w.kind)
        );
    } else if cancel || !open {
        w.snapshot.restore(cx);
        cx.mark_dirty();
    } else {
        WINDOW.with(|slot| *slot.borrow_mut() = Some(w));
    }
}

enum Act {
    Edit,
    EditActive,
    Copy,
    Rename,
    Delete,
}

fn unique_name(base: &str, taken: &[String]) -> String {
    if !taken.iter().any(|t| t == base) {
        return base.to_string();
    }
    (3..)
        .map(|n| format!("{} {n}", base.trim_end_matches(" 2")))
        .find(|c| !taken.contains(c))
        .unwrap_or_else(|| base.to_string())
}

#[cfg(test)]
pub(crate) fn close() {
    WINDOW.with(|w| *w.borrow_mut() = None);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cx() -> EditorContext {
        close();
        default_sets::reset_state();
        crate::editor::plan_tabs::plain_cx()
    }

    fn frame(cx: &mut EditorContext) {
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                show(ctx, cx);
                default_sets::show_all(ctx, cx);
            });
        }
    }

    #[test]
    fn cancel_puts_back_every_change_the_dialog_made() {
        let mut cx = cx();
        assert!(run_command(&mut cx, command_id(SavedKind::Callouts)));
        assert_eq!(open_kind(), Some(SavedKind::Callouts));
        cx.project
            .saved_copy(&mut cx.defaults, SavedKind::Callouts, "Default", "Extra")
            .unwrap();
        cx.project
            .saved_activate(&mut cx.defaults, SavedKind::Callouts, "Extra");
        cx.project.annot_defaults.callout.pose_idx = 4;
        // Esc is Cancel.
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(input, |ctx| show(ctx, &mut cx));
        assert_eq!(open_kind(), None);
        assert_eq!(
            cx.project
                .saved_names(&mut cx.defaults, SavedKind::Callouts),
            vec!["Default"]
        );
        assert_eq!(cx.project.annot_defaults.callout.pose_idx, 0);
    }

    #[test]
    fn the_dialog_draws_for_every_kind() {
        let mut cx = cx();
        for kind in SavedKind::ALL {
            open(&mut cx, kind);
            frame(&mut cx);
            assert_eq!(open_kind(), Some(kind));
            close();
        }
    }

    #[test]
    fn tool_buttons_map_to_their_saved_defaults() {
        use crate::tools::text::TextMode;
        assert_eq!(
            kind_of_tool(ToolId::Dimension),
            Some(SavedKind::ManualDimensions)
        );
        assert_eq!(
            kind_of_tool(ToolId::TextVariant(TextMode::Callout)),
            Some(SavedKind::Callouts)
        );
        assert_eq!(kind_of_tool(ToolId::Select), None);
        let mut cx = cx();
        set_edit_active_on_double_click(false);
        assert!(double_click(
            &mut cx,
            Action::SetTool(ToolId::TextVariant(TextMode::Marker))
        ));
        assert_eq!(
            open_kind(),
            Some(SavedKind::Markers),
            "the Saved Defaults dialog"
        );
        close();
        set_edit_active_on_double_click(true);
        assert!(double_click(
            &mut cx,
            Action::SetTool(ToolId::TextVariant(TextMode::Marker))
        ));
        assert_eq!(open_kind(), None);
        assert!(
            default_sets::editor_is_open(),
            "the active default's own dialog"
        );
        assert!(!double_click(&mut cx, Action::SetTool(ToolId::Select)));
        default_sets::reset_state();
    }

    #[test]
    fn unique_names_count_up() {
        let taken = vec!["A".to_string(), "A 2".to_string()];
        assert_eq!(unique_name("B 2", &taken), "B 2");
        assert_eq!(unique_name("A 2", &taken), "A 3");
    }
}
