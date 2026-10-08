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
    /// Floors and Rooms > Floor Defaults (R-56).
    FloorDefaults,
    TextStyles,
    /// Preferences > Templates (the Templates page window).
    Templates,
}

/// A leaf of the tree: a dialog `main` opens for the entry, or a page this
/// module draws itself.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Leaf {
    Entry(DefaultsEntry),
    /// Roofs > Roof Defaults (RF-14, RF-15, RF-28, RF-31).
    RoofDefaults,
    /// Cabinets > Cabinet Defaults.
    CabinetDefaults,
    /// Framing > Framing Defaults.
    FramingDefaults,
}

const TREE: &[(&str, &[(&str, Leaf)])] = &[
    (
        "Walls",
        &[
            ("Exterior Wall", Leaf::Entry(DefaultsEntry::ExteriorWall)),
            ("Interior Wall", Leaf::Entry(DefaultsEntry::InteriorWall)),
            (
                "Foundation Wall",
                Leaf::Entry(DefaultsEntry::FoundationWall),
            ),
        ],
    ),
    (
        "Doors",
        &[
            ("Interior Door", Leaf::Entry(DefaultsEntry::InteriorDoor)),
            ("Exterior Door", Leaf::Entry(DefaultsEntry::ExteriorDoor)),
        ],
    ),
    ("Windows", &[("Window", Leaf::Entry(DefaultsEntry::Window))]),
    (
        "Dimension",
        &[("Dimensions", Leaf::Entry(DefaultsEntry::Dimensions))],
    ),
    (
        "Text",
        &[("Text Styles", Leaf::Entry(DefaultsEntry::TextStyles))],
    ),
    (
        "Floors and Rooms",
        &[
            ("Floor Defaults", Leaf::Entry(DefaultsEntry::FloorDefaults)),
            ("Room Types", Leaf::Entry(DefaultsEntry::RoomTypes)),
        ],
    ),
    ("Roofs", &[("Roof Defaults", Leaf::RoofDefaults)]),
    (
        "Cabinets",
        &[("Cabinet Defaults", Leaf::CabinetDefaults)],
    ),
    ("Framing", &[("Framing Defaults", Leaf::FramingDefaults)]),
    (
        "Preferences",
        &[("Templates", Leaf::Entry(DefaultsEntry::Templates))],
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
    selected: Option<Leaf>,
}

/// What editing a leaf does: the entries go to `main`, the Roof Defaults
/// page opens here.
fn edit_outcome(leaf: Leaf) -> DefaultsOutcome {
    match leaf {
        Leaf::Entry(e) => DefaultsOutcome::Edit(e),
        Leaf::RoofDefaults => {
            open_roof_defaults();
            DefaultsOutcome::Open
        }
        Leaf::CabinetDefaults => {
            OPEN_CABINETS.with(|c| c.set(true));
            DefaultsOutcome::Open
        }
        Leaf::FramingDefaults => {
            OPEN_FRAMING.with(|c| c.set(true));
            DefaultsOutcome::Open
        }
    }
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
                                            outcome = edit_outcome(*entry);
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
                                outcome = edit_outcome(e);
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

/// Draws the Templates page and the Roof Defaults page if they are open;
/// call once a frame.
pub fn show_templates_page(ctx: &egui::Context, cx: &mut EditorContext) {
    show_roof_defaults(ctx, cx);
    show_cabinet_defaults(ctx, cx);
    show_framing_defaults(ctx, cx);
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

// ===================================================================
// Roofs > Roof Defaults
// ===================================================================

/// The Roof Defaults page: a draft of [`plan_core::defaults::RoofDetailDefaults`]
/// (eave cut, fascia, soffit, rafter tails, attic walls, baseline rule) that
/// Build Roof copies into the roof settings of its floor, and the 3D view
/// draws from.
pub struct RoofDefaultsPage {
    pub draft: plan_core::defaults::RoofDetailDefaults,
    /// Also give the roofs of this plan the new detail.
    pub apply_to_plan: bool,
    fields: super::Fields,
    types: Vec<String>,
}

impl RoofDefaultsPage {
    pub fn new(cx: &EditorContext) -> Self {
        Self {
            draft: cx.defaults.roof_detail.clone(),
            apply_to_plan: true,
            fields: super::Fields::default(),
            types: cx
                .defaults
                .wall_types
                .iter()
                .map(|t| t.name.clone())
                .collect(),
        }
    }

    /// Why OK is off, if it is.
    pub fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            Some("Enter a valid length".into())
        } else {
            super::roof::detail_error(&self.draft)
        }
    }

    /// Makes the draft the defaults and, when asked, the detail of the
    /// plan's roofs (one undo step). Returns the floors changed.
    pub fn apply(&self, cx: &mut EditorContext) -> usize {
        cx.defaults.roof_detail = self.draft.clone();
        let mut changed = 0;
        if self.apply_to_plan {
            cx.begin_change("Roof Defaults");
            changed = crate::editor::roof_view::apply_detail(&mut cx.project, &self.draft);
        }
        cx.mark_dirty();
        changed
    }
}

thread_local! {
    static ROOF_OPEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ROOF_PAGE: RefCell<Option<RoofDefaultsPage>> = const { RefCell::new(None) };
}

/// Opens the Roof Defaults page (drawn by [`show_templates_page`]'s caller
/// once a frame).
pub fn open_roof_defaults() {
    ROOF_OPEN.with(|c| c.set(true));
}

/// Is the Roof Defaults page open?
#[cfg_attr(not(test), allow(dead_code))]
pub fn roof_defaults_open() -> bool {
    ROOF_OPEN.with(|c| c.get()) || ROOF_PAGE.with(|p| p.borrow().is_some())
}

fn show_roof_defaults(ctx: &egui::Context, cx: &mut EditorContext) {
    if ROOF_OPEN.with(|c| c.replace(false)) {
        ROOF_PAGE.with(|p| *p.borrow_mut() = Some(RoofDefaultsPage::new(cx)));
    }
    let Some(mut page) = ROOF_PAGE.with(|p| p.borrow_mut().take()) else {
        return;
    };
    let mut open = true;
    let mut outcome = super::Outcome::Open;
    egui::Window::new("Roof Defaults")
        .id(egui::Id::new("roof_defaults_page"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([460.0, 560.0])
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            let height = (ui.available_height() - 80.0).max(120.0);
            egui::ScrollArea::vertical()
                .id_salt("roof_defaults_scroll")
                .max_height(height)
                .show(ui, |ui| {
                    super::roof::detail_form(ui, &mut page.fields, &mut page.draft, &page.types);
                });
            ui.separator();
            ui.checkbox(
                &mut page.apply_to_plan,
                "Also use for the roofs in this plan",
            );
            let error = page.error();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_enabled(error.is_none(), egui::Button::new("   OK   "))
                    .clicked()
                {
                    outcome = super::Outcome::Ok;
                }
                if ui.button("Cancel").clicked() {
                    outcome = super::Outcome::Cancel;
                }
                if let Some(e) = &error {
                    ui.colored_label(egui::Color32::from_rgb(0xE0, 0x4B, 0x4B), e);
                }
            });
        });
    if !open || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        outcome = super::Outcome::Cancel;
    }
    match outcome {
        super::Outcome::Open => ROOF_PAGE.with(|p| *p.borrow_mut() = Some(page)),
        super::Outcome::Cancel => {}
        super::Outcome::Ok => {
            let n = page.apply(cx);
            cx.status = if n > 0 {
                format!("Saved the roof defaults; {n} roof(s) in this plan use them")
            } else {
                "Saved the roof defaults".into()
            };
        }
    }
}

// ===================================================================
// Cabinets > Cabinet Defaults, Framing > Framing Defaults
// ===================================================================

/// Command id: opens the Cabinet Defaults window (Edit menu).
pub const CABINETS: &str = "defaults.cabinets";
/// Command id: opens the Framing Defaults window (Build > Framing).
pub const FRAMING: &str = "defaults.framing";
/// Command id: switches to the Framing Overview, or back (Build > Framing).
pub const FRAMING_OVERVIEW: &str = "defaults.framing_overview";

thread_local! {
    static OPEN_CABINETS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static OPEN_FRAMING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static CABINET_PAGE: RefCell<Option<super::cabinet::CabinetDefaultsDialog>> =
        const { RefCell::new(None) };
    static FRAMING_PAGE: RefCell<Option<super::framing::FramingDefaultsDialog>> =
        const { RefCell::new(None) };
}

/// Runs the menu commands of this module; false for any other id.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        CABINETS => OPEN_CABINETS.with(|c| c.set(true)),
        FRAMING => OPEN_FRAMING.with(|c| c.set(true)),
        FRAMING_OVERVIEW => {
            if crate::editor::framing_view::in_overview(&cx.project) {
                crate::editor::framing_view::leave_overview(cx);
            } else {
                crate::editor::framing_view::activate_overview(cx);
            }
        }
        _ => return false,
    }
    true
}

fn show_cabinet_defaults(ctx: &egui::Context, cx: &mut EditorContext) {
    if OPEN_CABINETS.with(|c| c.replace(false)) {
        let dlg = super::cabinet::CabinetDefaultsDialog::new(&cx.defaults.cabinets);
        CABINET_PAGE.with(|p| *p.borrow_mut() = Some(dlg));
    }
    let Some(mut dlg) = CABINET_PAGE.with(|p| p.borrow_mut().take()) else {
        return;
    };
    match dlg.show(ctx) {
        super::Outcome::Open => CABINET_PAGE.with(|p| *p.borrow_mut() = Some(dlg)),
        super::Outcome::Cancel => {}
        super::Outcome::Ok => {
            cx.defaults.cabinets = dlg.draft().clone();
            cx.mark_dirty();
            cx.status = "Saved the cabinet defaults".into();
        }
    }
}

fn show_framing_defaults(ctx: &egui::Context, cx: &mut EditorContext) {
    if OPEN_FRAMING.with(|c| c.replace(false)) {
        let settings = crate::editor::framing_view::settings(&cx.project);
        let dlg = super::framing::FramingDefaultsDialog::new(&settings);
        FRAMING_PAGE.with(|p| *p.borrow_mut() = Some(dlg));
    }
    let Some(mut dlg) = FRAMING_PAGE.with(|p| p.borrow_mut().take()) else {
        return;
    };
    match dlg.show(ctx) {
        super::Outcome::Open => FRAMING_PAGE.with(|p| *p.borrow_mut() = Some(dlg)),
        super::Outcome::Cancel => {}
        super::Outcome::Ok => {
            crate::editor::framing_view::set_settings(cx, dlg.draft().clone());
            cx.status = "Saved the framing defaults".into();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::defaults::EaveCut;

    #[test]
    fn the_tree_lists_roof_defaults_and_it_opens_its_own_page() {
        let leaves: Vec<_> = TREE
            .iter()
            .flat_map(|(_, l)| l.iter())
            .map(|(n, l)| (*n, *l))
            .collect();
        assert!(leaves.contains(&("Roof Defaults", Leaf::RoofDefaults)));
        // Every other leaf still goes to the entry's dialog.
        assert_eq!(
            edit_outcome(Leaf::Entry(DefaultsEntry::Templates)),
            DefaultsOutcome::Edit(DefaultsEntry::Templates)
        );
        ROOF_OPEN.with(|c| c.set(false));
        assert_eq!(edit_outcome(Leaf::RoofDefaults), DefaultsOutcome::Open);
        assert!(roof_defaults_open());
        ROOF_OPEN.with(|c| c.set(false));
    }

    #[test]
    fn ok_on_the_roof_defaults_page_feeds_the_defaults_and_the_plan() {
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        let mut page = RoofDefaultsPage::new(&cx);
        assert!(
            page.draft.baseline_at_plate,
            "Daniel's template seats the roof on the plate"
        );
        page.draft.eave_cut = EaveCut::Square;
        page.draft.rafter_tails = true;
        assert!(page.error().is_none());
        // A plan with a stored roof settings entry takes the detail too.
        cx.project.floors[0].roofs = vec![serde_json::json!({"kind": "settings"})];
        assert_eq!(page.apply(&mut cx), 1);
        assert_eq!(cx.defaults.roof_detail.eave_cut, EaveCut::Square);
        let stored = crate::editor::roof_view::load(&cx.project.floors[0])
            .settings
            .unwrap();
        assert!(stored.detail.rafter_tails);
        // A bad size blocks OK.
        page.draft.thickness = 0.0;
        assert!(page.error().is_some());
    }
}
