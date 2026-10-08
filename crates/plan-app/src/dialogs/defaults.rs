//! Default Settings (Edit > Default Settings...): a searchable tree whose
//! leaves open the same specification dialogs objects use.

use eframe::egui::{self, Align, Key, Layout, Modifiers};

/// A leaf of the Default Settings tree.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DefaultsEntry {
    ExteriorWall,
    InteriorWall,
    FoundationWall,
    InteriorDoor,
    ExteriorDoor,
    Window,
    Dimensions,
    RoomTypes,
    TextStyles,
}

const TREE: &[(&str, &[(&str, DefaultsEntry)])] = &[
    (
        "Walls",
        &[
            ("Exterior Wall", DefaultsEntry::ExteriorWall),
            ("Interior Wall", DefaultsEntry::InteriorWall),
            ("Foundation Wall", DefaultsEntry::FoundationWall),
        ],
    ),
    (
        "Doors",
        &[
            ("Interior Door", DefaultsEntry::InteriorDoor),
            ("Exterior Door", DefaultsEntry::ExteriorDoor),
        ],
    ),
    ("Windows", &[("Window", DefaultsEntry::Window)]),
    ("Dimension", &[("Dimensions", DefaultsEntry::Dimensions)]),
    ("Text", &[("Text Styles", DefaultsEntry::TextStyles)]),
    (
        "Floors and Rooms",
        &[("Room Types", DefaultsEntry::RoomTypes)],
    ),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DefaultsOutcome {
    Open,
    Close,
    /// Edit the chosen leaf (double-click or the Edit button).
    Edit(DefaultsEntry),
}

#[derive(Default)]
pub struct DefaultsDialog {
    search: String,
    selected: Option<DefaultsEntry>,
}

impl DefaultsDialog {
    pub fn new() -> Self {
        Self::default()
    }

    /// `enabled` is false while a specification dialog is open on top.
    pub fn show(&mut self, ctx: &egui::Context, enabled: bool) -> DefaultsOutcome {
        let mut outcome = DefaultsOutcome::Open;
        let mut open = true;
        let query = self.search.trim().to_lowercase();
        egui::Window::new("Default Settings")
            .id(egui::Id::new("default_settings_dialog"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([360.0, 420.0])
            .min_size([300.0, 280.0])
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Search");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search)
                                .hint_text("Filter the tree")
                                .desired_width(f32::INFINITY),
                        );
                    });
                    ui.separator();
                    let tree_height = (ui.available_height() - 40.0).max(80.0);
                    egui::ScrollArea::vertical()
                        .id_salt("defaults_tree")
                        .max_height(tree_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let mut any = false;
                            for (group, leaves) in TREE {
                                let group_hit =
                                    query.is_empty() || group.to_lowercase().contains(&query);
                                let shown: Vec<_> = leaves
                                    .iter()
                                    .filter(|(name, _)| {
                                        group_hit || name.to_lowercase().contains(&query)
                                    })
                                    .collect();
                                if shown.is_empty() {
                                    continue;
                                }
                                any = true;
                                let mut header = egui::CollapsingHeader::new(*group)
                                    .id_salt(("defaults_group", group))
                                    .default_open(true);
                                if !query.is_empty() {
                                    header = header.open(Some(true));
                                }
                                header.show(ui, |ui| {
                                    for (name, entry) in shown {
                                        let resp = ui
                                            .selectable_label(self.selected == Some(*entry), *name);
                                        if resp.clicked() {
                                            self.selected = Some(*entry);
                                        }
                                        if resp.double_clicked() {
                                            self.selected = Some(*entry);
                                            outcome = DefaultsOutcome::Edit(*entry);
                                        }
                                    }
                                });
                            }
                            if !any {
                                ui.weak("No settings match the search");
                            }
                        });
                    ui.separator();
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            outcome = DefaultsOutcome::Close;
                        }
                        let edit = ui.add_enabled(
                            self.selected.is_some(),
                            egui::Button::new("Edit\u{2026}"),
                        );
                        if edit.clicked() {
                            if let Some(e) = self.selected {
                                outcome = DefaultsOutcome::Edit(e);
                            }
                        }
                    });
                });
            });
        if !open {
            outcome = DefaultsOutcome::Close;
        }
        if enabled && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            outcome = DefaultsOutcome::Close;
        }
        outcome
    }
}
