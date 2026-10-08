//! Default Settings (Edit > Default Settings...): a searchable tree whose
//! leaves open the same specification dialogs objects use. The Preferences >
//! Templates leaf opens the Templates page: the default plan and layout
//! template paths, the seeding switch and what was decoded from them.

use crate::editor::EditorContext;
use crate::templates::{self, SeedCache, TemplateSettings};
use eframe::egui::{self, Align, Align2, Key, Layout, Modifiers};
use std::cell::RefCell;
use std::path::PathBuf;

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
    /// Preferences > Templates (the Templates page window).
    Templates,
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
    ("Preferences", &[("Templates", DefaultsEntry::Templates)]),
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

// ===================================================================
// Preferences > Templates
// ===================================================================

struct TemplatesPage {
    settings: TemplateSettings,
    plan_text: String,
    layout_text: String,
    cache: SeedCache,
    status: String,
}

thread_local! {
    static PAGE: RefCell<Option<TemplatesPage>> = const { RefCell::new(None) };
}

fn path_text(p: &Option<PathBuf>) -> String {
    p.as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn non_empty(s: &str) -> Option<PathBuf> {
    let t = s.trim();
    (!t.is_empty()).then(|| PathBuf::from(t))
}

/// Opens the Templates page window (it is drawn by [`show_templates_page`]).
pub fn open_templates_page() {
    let settings = templates::load_settings();
    let page = TemplatesPage {
        plan_text: path_text(&settings.plan),
        layout_text: path_text(&settings.layout),
        settings,
        cache: SeedCache::load(),
        status: String::new(),
    };
    PAGE.with(|p| *p.borrow_mut() = Some(page));
}

/// Re-reads the settings into the Templates page when it is open (after the
/// Import window changed them).
pub fn reload_templates_page() {
    let is_open = PAGE.with(|p| p.borrow().is_some());
    if is_open {
        open_templates_page();
    }
}

/// Draws the Templates page if it is open; call once a frame.
pub fn show_templates_page(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut page) = PAGE.with(|p| p.borrow_mut().take()) else {
        return;
    };
    if page.show(ctx, cx) {
        PAGE.with(|p| {
            let mut slot = p.borrow_mut();
            if slot.is_none() {
                *slot = Some(page);
            }
        });
    }
}

/// Forgets the per-session edits of the default dialogs so they show the
/// new defaults again (what Reset to Chief X18 Template does).
pub fn forget_dialog_edits(cx: &mut EditorContext) {
    use crate::dialogs::{OpeningTarget, WallTarget};
    for key in [
        WallTarget::DefaultExterior.key(),
        WallTarget::DefaultInterior.key(),
        WallTarget::DefaultFoundation.key(),
    ] {
        cx.extras.walls.remove(&key);
    }
    for target in [
        OpeningTarget::DefaultDoor,
        OpeningTarget::DefaultExteriorDoor,
        OpeningTarget::DefaultWindow,
    ] {
        cx.extras.openings.remove(&target.key());
    }
}

/// Saves `settings`, decodes the templates (`force` re-reads even an
/// unchanged file) and, unless the user saved their own defaults, makes the
/// result the defaults new plans start from. Returns the cache and a status
/// line.
pub fn apply_template_settings(
    cx: &mut EditorContext,
    settings: &TemplateSettings,
    force: bool,
) -> (SeedCache, String) {
    let mut parts: Vec<String> = Vec::new();
    if let Err(e) = templates::save_settings(settings) {
        parts.push(format!("Could not save the settings: {e}"));
    }
    let out = templates::refresh(settings, force);
    parts.extend(out.notes.iter().cloned());
    let own = crate::plan_defaults::user_path().is_some_and(|p| p.exists());
    if own {
        parts.push(
            "Your saved template (defaults.json) takes priority; use File > Templates > Reset to Chief X18 Template to use the Chief template".into(),
        );
    } else {
        cx.defaults =
            templates::defaults_from(settings, &out.cache, crate::plan_defaults::embedded());
        forget_dialog_edits(cx);
        match (&out.cache.plan, settings.seed_from_chief) {
            (Some(p), true) => parts.insert(
                0,
                format!(
                    "Defaults seeded from {} ({} wall types)",
                    p.file_name,
                    p.wall_types.len()
                ),
            ),
            (_, false) => parts.insert(0, "Using the shipped Chief X18 defaults".into()),
            (None, true) => {}
        }
    }
    (out.cache, parts.join("; "))
}

impl TemplatesPage {
    fn current_settings(&self) -> TemplateSettings {
        TemplateSettings {
            plan: non_empty(&self.plan_text),
            layout: non_empty(&self.layout_text),
            seed_from_chief: self.settings.seed_from_chief,
        }
    }

    fn apply(&mut self, cx: &mut EditorContext, force: bool) {
        self.settings = self.current_settings();
        let (cache, status) = apply_template_settings(cx, &self.settings, force);
        self.cache = cache;
        cx.status = status.clone();
        self.status = status;
    }

    /// Returns `false` when the window was closed.
    fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) -> bool {
        let mut open = true;
        let mut close = false;
        let mut apply = false;
        let mut force = false;
        egui::Window::new("Preferences: Templates")
            .id(egui::Id::new("preferences_templates"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(560.0)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.label("New plans and layouts start from these Chief templates.");
                ui.add_space(4.0);
                for (label, text, exts) in [
                    ("Plan template", &mut self.plan_text, &["plan", "tpl"][..]),
                    ("Layout template", &mut self.layout_text, &["layout"][..]),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(label);
                        let edit = ui.add(
                            egui::TextEdit::singleline(text)
                                .desired_width(ui.available_width() - 90.0),
                        );
                        if edit.lost_focus() && edit.changed() {
                            apply = true;
                        }
                        if ui.button("Browse\u{2026}").clicked() {
                            if let Some(p) = rfd::FileDialog::new()
                                .add_filter("Chief template", exts)
                                .pick_file()
                            {
                                *text = p.to_string_lossy().into_owned();
                                apply = true;
                            }
                        }
                    });
                }
                if ui
                    .checkbox(
                        &mut self.settings.seed_from_chief,
                        "Seed defaults from Chief template",
                    )
                    .changed()
                {
                    apply = true;
                }
                ui.horizontal(|ui| {
                    if ui.button("Re-read template now").clicked() {
                        apply = true;
                        force = true;
                    }
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
                ui.separator();
                ui.strong("Decoded from the templates");
                match &self.cache.plan {
                    Some(p) => {
                        ui.label(&p.file_name);
                        for line in templates::plan_summary_lines(p) {
                            ui.label(line);
                        }
                        ui.weak(format!("Last read {}", templates::format_time(p.read_at)));
                    }
                    None => {
                        ui.weak("No plan template has been read.");
                    }
                }
                ui.add_space(4.0);
                match &self.cache.layout {
                    Some(l) => {
                        ui.label(&l.file_name);
                        for line in templates::layout_summary_lines(l) {
                            ui.label(line);
                        }
                        ui.weak(format!("Last read {}", templates::format_time(l.read_at)));
                    }
                    None => {
                        ui.weak("No layout template has been read.");
                    }
                }
                if !self.status.is_empty() {
                    ui.separator();
                    ui.label(&self.status);
                }
            });
        if apply {
            self.apply(cx, force);
        }
        open && !close
    }
}
