//! Customize Toolbars (Tools > Toolbars and Hotkeys), after Chief's Toolbar
//! Customization dialog: pick the toolbar set of a view type (floor plan, 3D
//! view, vector elevation, layout), choose which of the available buttons
//! each row shows and in which order, add rows, lock the toolbars, reset to
//! Daniel's Chief set, import a Chief `.toolbar` file and export the
//! configuration as JSON.
//!
//! The dialog edits a copy of the [`ToolbarConfig`]. OK or Apply makes it the
//! live configuration and saves `~/.plan-studio/toolbars.json`; Cancel drops
//! it. The sets are the ones [`crate::toolbar::config`] draws the bars from.

use super::Outcome;
use crate::toolbar::config::{
    self, BarLayout, ImportReport, ToolbarConfig, ViewKind, ViewSet, ROW1, ROW2, SEPARATOR,
    VIEW_BAR,
};
use eframe::egui::{self, Align, Align2, Color32, Layout, RichText, Vec2};

const WARN: Color32 = Color32::from_rgb(0xE0, 0xA0, 0x30);

pub struct ToolbarDialog {
    draft: ToolbarConfig,
    view: ViewKind,
    /// The row being edited (a [`BarLayout::id`]).
    bar: String,
    search: String,
    /// The picked entry of the row, by index.
    selected: Option<usize>,
    new_row_name: String,
    message: String,
}

impl ToolbarDialog {
    /// Opens on the set of the view that is on screen.
    pub fn new(cfg: &ToolbarConfig, view: ViewKind) -> Self {
        ToolbarDialog {
            draft: cfg.clone(),
            view,
            bar: ROW2.to_string(),
            search: String::new(),
            selected: None,
            new_row_name: String::new(),
            message: String::new(),
        }
    }

    #[cfg(test)]
    pub fn draft(&self) -> &ToolbarConfig {
        &self.draft
    }

    #[cfg(test)]
    pub fn view(&self) -> ViewKind {
        self.view
    }

    #[cfg(test)]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Switches the set being edited.
    pub fn set_view(&mut self, v: ViewKind) {
        self.view = v;
        self.selected = None;
        if self.set().bar(&self.bar).is_none() {
            self.bar = ROW2.to_string();
        }
    }

    pub fn set(&self) -> &ViewSet {
        // `normalize` guarantees every view has its set.
        self.draft
            .views
            .get(&self.view)
            .expect("every view type has a toolbar set")
    }

    pub fn bar(&self) -> &BarLayout {
        self.set()
            .bar(&self.bar)
            .or_else(|| self.set().bars.first())
            .expect("a view set has bars")
    }

    pub fn select_bar(&mut self, id: &str) {
        if self.set().bar(id).is_some() {
            self.bar = id.to_string();
            self.selected = None;
        }
    }

    fn bar_mut(&mut self) -> Option<&mut BarLayout> {
        let id = self.bar.clone();
        self.draft.view_mut(self.view).bar_mut(&id)
    }

    /// Edits are refused while the toolbars are locked.
    fn unlocked(&mut self) -> bool {
        if self.draft.locked {
            self.message = "The toolbars are locked; clear Lock Toolbars to change them".into();
            false
        } else {
            true
        }
    }

    pub fn set_locked(&mut self, on: bool) {
        self.draft.locked = on;
    }

    // ----- editing the buttons of the row -----

    /// Adds `key` to the row (after the picked entry, else at the end), or
    /// takes it off when it is already there.
    pub fn toggle(&mut self, key: &str) -> bool {
        if !self.unlocked() {
            return false;
        }
        let after = self.selected.map(|i| i + 1);
        let Some(bar) = self.bar_mut() else {
            return false;
        };
        if bar.contains(key) {
            bar.remove_key(key);
            self.selected = None;
            true
        } else {
            let ok = bar.add(key, after);
            if ok {
                self.selected = after.map(|i| i.min(bar.items.len().saturating_sub(1)));
            }
            ok
        }
    }

    pub fn move_selected(&mut self, down: bool) -> bool {
        let Some(i) = self.selected else {
            return false;
        };
        if !self.unlocked() {
            return false;
        }
        let Some(bar) = self.bar_mut() else {
            return false;
        };
        let to = if down {
            if i + 1 >= bar.items.len() {
                return false;
            }
            i + 1
        } else {
            if i == 0 {
                return false;
            }
            i - 1
        };
        let moved = bar.move_item(i, to);
        if moved {
            self.selected = Some(to);
        }
        moved
    }

    pub fn remove_selected(&mut self) -> bool {
        let Some(i) = self.selected else {
            return false;
        };
        if !self.unlocked() {
            return false;
        }
        if let Some(bar) = self.bar_mut() {
            bar.remove_at(i);
        }
        self.selected = None;
        true
    }

    pub fn add_separator(&mut self) -> bool {
        if !self.unlocked() {
            return false;
        }
        let at = self.selected.map(|i| i + 1);
        let Some(bar) = self.bar_mut() else {
            return false;
        };
        let at = at.unwrap_or(bar.items.len());
        bar.add_separator(at);
        true
    }

    pub fn pick(&mut self, index: usize) {
        self.selected = Some(index);
    }

    pub fn set_visible(&mut self, id: &str, on: bool) -> bool {
        if !self.unlocked() {
            return false;
        }
        match self.draft.view_mut(self.view).bar_mut(id) {
            Some(b) => {
                b.visible = on;
                true
            }
            None => false,
        }
    }

    // ----- rows -----

    pub fn add_row(&mut self, name: &str) -> Option<String> {
        if !self.unlocked() {
            return None;
        }
        let id = self.draft.view_mut(self.view).add_row(name);
        self.bar = id.clone();
        self.selected = None;
        Some(id)
    }

    pub fn delete_row(&mut self) -> bool {
        if !self.unlocked() {
            return false;
        }
        let id = self.bar.clone();
        let removed = self.draft.view_mut(self.view).remove_row(&id);
        if removed {
            self.bar = ROW2.to_string();
            self.selected = None;
        } else {
            self.message = "The standard rows cannot be deleted; hide them instead".into();
        }
        removed
    }

    // ----- reset, import, export -----

    /// Reset to Daniel's Chief set, for the view being edited.
    pub fn reset_view(&mut self) -> bool {
        if !self.unlocked() {
            return false;
        }
        self.draft.reset_view(self.view);
        self.bar = ROW2.to_string();
        self.selected = None;
        self.message = format!("{} is back to Daniel's Chief set", self.view.label());
        true
    }

    pub fn reset_all(&mut self) -> bool {
        if !self.unlocked() {
            return false;
        }
        self.draft.reset_all();
        self.bar = ROW2.to_string();
        self.selected = None;
        self.message = "Every view type is back to Daniel's Chief set".into();
        true
    }

    /// Reads a Chief `.toolbar` file's text and replaces the standard rows of
    /// the view being edited with its buttons.
    pub fn import_chief_text(&mut self, text: &str) -> Result<ImportReport, String> {
        if !self.unlocked() {
            return Err(self.message.clone());
        }
        let set = plan_config::parse_toolbar(text).map_err(|e| e.to_string())?;
        let report = config::import_chief(&set, self.view);
        if report.total == 0 {
            return Err(format!(
                "That file has no toolbars for the {} view type",
                self.view.label()
            ));
        }
        self.draft.apply_import(&report);
        self.selected = None;
        self.message = report.summary();
        Ok(report)
    }

    /// Daniel's embedded Default Configuration, through the same import.
    pub fn import_daniels_default(&mut self) -> Result<ImportReport, String> {
        if !self.unlocked() {
            return Err(self.message.clone());
        }
        let cfg = plan_config::load_daniel_config();
        let set = cfg
            .toolbars
            .first()
            .ok_or("Daniel's Chief toolbars are missing")?;
        let report = config::import_chief(set, self.view);
        self.draft.apply_import(&report);
        self.selected = None;
        self.message = report.summary();
        Ok(report)
    }

    pub fn export_json(&self) -> String {
        self.draft.to_json()
    }

    /// Replaces the draft with an exported file's configuration.
    pub fn import_json(&mut self, text: &str) -> Result<(), String> {
        if !self.unlocked() {
            return Err(self.message.clone());
        }
        let cfg = ToolbarConfig::from_json(text)?;
        self.draft = cfg;
        self.set_view(self.view);
        self.message = "Toolbar file loaded".into();
        Ok(())
    }

    /// OK / Apply: the draft becomes the live configuration and is saved.
    pub fn commit(&mut self) -> Result<(), String> {
        let r = config::set_current(self.draft.clone());
        self.message = match &r {
            Ok(()) => "Toolbars saved".into(),
            Err(e) => format!("Toolbars changed but not saved: {e}"),
        };
        r
    }

    // ----- what the lists show -----

    /// The available buttons by group, filtered by the search text (which
    /// matches the button, its group and its flyout members).
    pub fn available(&self) -> Vec<(String, Vec<&'static config::CatalogEntry>)> {
        let needle = self.search.trim().to_lowercase();
        let mut out = Vec::new();
        for g in config::groups() {
            let entries: Vec<&config::CatalogEntry> = config::catalog()
                .iter()
                .filter(|e| e.group == g)
                .filter(|e| {
                    needle.is_empty()
                        || e.key.to_lowercase().contains(&needle)
                        || e.group.to_lowercase().contains(&needle)
                        || e.detail.to_lowercase().contains(&needle)
                })
                .collect();
            if !entries.is_empty() {
                out.push((g, entries));
            }
        }
        out
    }

    #[cfg(test)]
    pub fn set_search(&mut self, text: &str) {
        self.search = text.to_string();
    }

    /// Names of the other rows of this set that also carry `key`.
    fn elsewhere(&self, key: &str) -> Vec<&str> {
        self.set()
            .bars
            .iter()
            .filter(|b| b.id != self.bar && b.contains(key))
            .map(|b| b.name.as_str())
            .collect()
    }

    // ----- drawing -----

    /// Draws the window and reports OK / Cancel.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        egui::Window::new("Customize Toolbars")
            .id(egui::Id::new("customize_toolbars"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(820.0, 600.0))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| self.body(ui, &mut outcome));
        if !open && outcome == Outcome::Open {
            outcome = Outcome::Cancel;
        }
        outcome
    }

    fn body(&mut self, ui: &mut egui::Ui, outcome: &mut Outcome) {
        ui.horizontal(|ui| {
            ui.label("Toolbars for:");
            for v in ViewKind::ALL {
                if ui.selectable_label(self.view == v, v.label()).clicked() {
                    self.set_view(v);
                }
            }
            ui.weak(format!("(showing now: {})", config::active_view().label()));
        });
        ui.separator();
        let list_height = (ui.available_height() - 120.0).max(180.0);
        ui.columns(2, |cols| {
            self.available_panel(&mut cols[0], list_height);
            self.rows_panel(&mut cols[1], list_height);
        });
        ui.separator();
        self.footer(ui, outcome);
    }

    fn available_panel(&mut self, ui: &mut egui::Ui, height: f32) {
        ui.strong("Available buttons");
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.add(egui::TextEdit::singleline(&mut self.search).desired_width(f32::INFINITY));
        });
        let searching = !self.search.trim().is_empty();
        let bar_name = self.bar().name.clone();
        ui.weak(format!("Tick a button to put it on \"{bar_name}\"."));
        let mut toggled: Option<String> = None;
        egui::ScrollArea::vertical()
            .id_salt("tb_available")
            .max_height(height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (group, entries) in self.available() {
                    egui::CollapsingHeader::new(&group)
                        .id_salt(("tb_group", &group))
                        .default_open(searching)
                        .show(ui, |ui| {
                            for e in entries {
                                let mut on = self.bar().contains(e.key);
                                let elsewhere = self.elsewhere(e.key);
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::Image::new(crate::icons::icon(e.icon))
                                            .fit_to_exact_size(Vec2::splat(16.0)),
                                    );
                                    let r = ui.checkbox(&mut on, e.key).on_hover_text(format!(
                                        "{} ({})",
                                        e.detail,
                                        e.kind.label()
                                    ));
                                    if r.changed() {
                                        toggled = Some(e.key.to_string());
                                    }
                                    if !elsewhere.is_empty() {
                                        ui.weak(format!("(also on {})", elsewhere.join(", ")));
                                    }
                                });
                            }
                        });
                }
            });
        if let Some(key) = toggled {
            self.toggle(&key);
        }
    }

    fn rows_panel(&mut self, ui: &mut egui::Ui, height: f32) {
        ui.strong("Rows");
        let mut pick_bar: Option<String> = None;
        let mut visible_change: Option<(String, bool)> = None;
        for b in &self.set().bars {
            ui.horizontal(|ui| {
                let mut vis = b.visible;
                if ui
                    .checkbox(&mut vis, "")
                    .on_hover_text("Show this row")
                    .changed()
                {
                    visible_change = Some((b.id.clone(), vis));
                }
                let label = match b.id.as_str() {
                    ROW1 | ROW2 | VIEW_BAR => format!("{} (standard)", b.name),
                    _ => b.name.clone(),
                };
                if ui.selectable_label(self.bar == b.id, label).clicked() {
                    pick_bar = Some(b.id.clone());
                }
                ui.weak(format!("{} buttons", b.keys().count()));
            });
        }
        if let Some(id) = pick_bar {
            self.select_bar(&id);
        }
        if let Some((id, on)) = visible_change {
            self.set_visible(&id, on);
        }
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.new_row_name)
                    .hint_text("Name of the new row")
                    .desired_width(150.0),
            );
            if ui.button("Add Row").clicked() {
                let name = std::mem::take(&mut self.new_row_name);
                self.add_row(&name);
            }
            let custom = !self.bar().is_builtin();
            if ui
                .add_enabled(custom, egui::Button::new("Delete Row"))
                .clicked()
            {
                self.delete_row();
            }
        });
        ui.separator();
        ui.strong(format!("Buttons on \"{}\"", self.bar().name));
        let mut pick: Option<usize> = None;
        let items = self.bar().items.clone();
        egui::ScrollArea::vertical()
            .id_salt("tb_items")
            .max_height((height - 140.0).max(100.0))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if items.is_empty() {
                    ui.weak("Empty. Tick buttons on the left to add them.");
                }
                for (i, key) in items.iter().enumerate() {
                    let text = if key == SEPARATOR {
                        RichText::new("\u{2014}\u{2014}  separator  \u{2014}\u{2014}").weak()
                    } else {
                        RichText::new(key)
                    };
                    if ui
                        .selectable_label(self.selected == Some(i), text)
                        .clicked()
                    {
                        pick = Some(i);
                    }
                }
            });
        if let Some(i) = pick {
            self.pick(i);
        }
        let has = self.selected.is_some();
        let n = self.bar().items.len();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    has && self.selected != Some(0),
                    egui::Button::new("Move Up"),
                )
                .clicked()
            {
                self.move_selected(false);
            }
            if ui
                .add_enabled(
                    has && self.selected.is_some_and(|i| i + 1 < n),
                    egui::Button::new("Move Down"),
                )
                .clicked()
            {
                self.move_selected(true);
            }
            if ui.add_enabled(has, egui::Button::new("Remove")).clicked() {
                self.remove_selected();
            }
            if ui.button("Add Separator").clicked() {
                self.add_separator();
            }
        });
    }

    fn footer(&mut self, ui: &mut egui::Ui, outcome: &mut Outcome) {
        let mut locked = self.draft.locked;
        if ui
            .checkbox(&mut locked, "Lock Toolbars")
            .on_hover_text("Refuses changes to the rows until it is cleared")
            .changed()
        {
            self.set_locked(locked);
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .button("Reset This View to Daniel's Chief Set")
                .on_hover_text(
                    "Floor Plan: the toolbars as Chief's Default Configuration lays them out",
                )
                .clicked()
            {
                self.reset_view();
            }
            if ui.button("Reset All Views").clicked() {
                self.reset_all();
            }
            if ui
                .button("Import Chief Toolbar File\u{2026}")
                .on_hover_text("Reads a Chief .toolbar file into this view type")
                .clicked()
            {
                self.import_file_dialog();
            }
            if ui
                .button("Import Daniel's Chief Toolbars")
                .on_hover_text("Re-reads the Default Configuration that ships with Plan Studio")
                .clicked()
            {
                if let Err(e) = self.import_daniels_default() {
                    self.message = e;
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button("Export\u{2026}").clicked() {
                self.export_file_dialog();
            }
            if ui.button("Load Exported File\u{2026}").clicked() {
                self.load_file_dialog();
            }
        });
        if !self.message.is_empty() {
            ui.label(RichText::new(&self.message).color(if self.draft.locked {
                WARN
            } else {
                ui.visuals().text_color()
            }));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("OK").clicked() {
                // A failed save keeps the window open with the reason shown.
                if self.commit().is_ok() {
                    *outcome = Outcome::Ok;
                }
            }
            if ui.button("Cancel").clicked() {
                *outcome = Outcome::Cancel;
            }
            if ui.button("Apply").clicked() {
                let _ = self.commit();
            }
        });
    }

    fn import_file_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import a Chief toolbar file")
            .add_filter("Chief toolbar", &["toolbar"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                if let Err(e) = self.import_chief_text(&text) {
                    self.message = e;
                }
            }
            Err(e) => self.message = format!("Could not read {}: {e}", path.display()),
        }
    }

    fn export_file_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Export the toolbar configuration")
            .set_file_name(config::FILE_NAME)
            .add_filter("JSON", &["json"])
            .save_file()
        else {
            return;
        };
        self.message = match std::fs::write(&path, self.export_json()) {
            Ok(()) => format!("Exported to {}", path.display()),
            Err(e) => format!("Could not write {}: {e}", path.display()),
        };
    }

    fn load_file_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Load an exported toolbar configuration")
            .add_filter("JSON", &["json"])
            .pick_file()
        else {
            return;
        };
        let r = std::fs::read_to_string(&path)
            .map_err(|e| format!("Could not read {}: {e}", path.display()))
            .and_then(|t| self.import_json(&t));
        if let Err(e) = r {
            self.message = e;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT_TOOLBAR: &str =
        include_str!("../../../../docs/chief-config-raw/Default Configuration.toolbar");

    fn dialog() -> ToolbarDialog {
        ToolbarDialog::new(&ToolbarConfig::daniel_default(), ViewKind::Plan)
    }

    #[test]
    fn ticking_a_button_adds_it_after_the_pick_and_unticking_removes_it() {
        let mut d = dialog();
        d.select_bar(ROW2);
        assert!(!d.bar().contains("Road"));
        assert!(d.toggle("Road"));
        assert_eq!(d.bar().items.last().unwrap(), "Road");
        // With an entry picked, the new button goes right after it.
        d.pick(0);
        assert!(d.toggle("Sidewalk"));
        assert_eq!(d.bar().items[1], "Sidewalk");
        assert!(d.toggle("Sidewalk"));
        assert!(!d.bar().contains("Sidewalk"));
        // The draft changed, the live configuration did not.
        assert_ne!(&ToolbarConfig::daniel_default(), d.draft());
    }

    #[test]
    fn move_remove_and_separators_act_on_the_picked_entry() {
        let mut d = dialog();
        d.select_bar(ROW1);
        d.pick(0);
        let first = d.bar().items[0].clone();
        assert!(!d.move_selected(false), "already first");
        assert!(d.move_selected(true));
        assert_eq!(d.bar().items[1], first);
        assert_eq!(d.selected, Some(1));
        assert!(d.remove_selected());
        assert!(!d.bar().contains(&first));
        assert!(d.bar().hidden.contains(&first));
        assert!(!d.remove_selected(), "nothing picked any more");
        let n = d.bar().items.len();
        d.pick(0);
        assert!(d.add_separator());
        assert_eq!(d.bar().items.len(), n + 1);
        assert_eq!(d.bar().items[1], SEPARATOR);
    }

    #[test]
    fn rows_are_added_named_hidden_and_deleted() {
        let mut d = dialog();
        let id = d.add_row("Terrain Tools").unwrap();
        assert_eq!(d.bar().name, "Terrain Tools");
        assert!(d.toggle("Road"));
        assert!(d.set_visible(&id, false));
        assert!(!d.bar().visible);
        assert!(d.delete_row());
        assert!(d.set().bar(&id).is_none());
        d.select_bar(ROW1);
        assert!(!d.delete_row(), "standard rows stay");
        assert!(d.message().contains("cannot be deleted"));
    }

    #[test]
    fn views_have_separate_sets() {
        let mut d = dialog();
        d.select_bar(ROW2);
        assert!(d.bar().contains("Lines"));
        d.set_view(ViewKind::View3d);
        assert!(!d.bar().contains("Lines"));
        assert!(d.toggle("Lines"));
        d.set_view(ViewKind::Plan);
        assert!(d.bar().contains("Lines"));
        d.set_view(ViewKind::Layout);
        assert!(d.bar().contains("Page Setup"));
        assert_eq!(d.view(), ViewKind::Layout);
    }

    #[test]
    fn locking_refuses_every_edit_until_it_is_cleared() {
        let mut d = dialog();
        d.set_locked(true);
        assert!(!d.toggle("Road"));
        d.pick(0);
        assert!(!d.remove_selected());
        assert!(!d.move_selected(true));
        assert!(!d.add_separator());
        assert!(d.add_row("x").is_none());
        assert!(!d.reset_view());
        assert!(!d.reset_all());
        assert!(d.import_chief_text(DEFAULT_TOOLBAR).is_err());
        assert!(d.message().contains("locked"));
        d.set_locked(false);
        assert!(d.toggle("Road"));
    }

    #[test]
    fn reset_returns_to_daniels_set() {
        let mut d = dialog();
        d.select_bar(ROW2);
        d.toggle("Road");
        d.pick(0);
        d.remove_selected();
        d.set_view(ViewKind::Layout);
        d.select_bar(ROW2);
        d.toggle("Road");
        assert!(d.reset_view());
        assert_eq!(d.set(), &ToolbarConfig::default_view(ViewKind::Layout));
        // Only the view being edited was reset.
        d.set_view(ViewKind::Plan);
        assert_ne!(d.set(), &ToolbarConfig::default_view(ViewKind::Plan));
        assert!(d.reset_all());
        assert_eq!(d.draft(), &ToolbarConfig::daniel_default());
    }

    #[test]
    fn importing_a_chief_toolbar_file_replaces_the_standard_rows() {
        let mut d = dialog();
        let r = d.import_chief_text(DEFAULT_TOOLBAR).unwrap();
        assert!(r.mapped * 10 >= r.total * 8, "{}", r.summary());
        assert_eq!(d.bar().items[0], "Select Objects");
        assert!(d.message().contains("Chief buttons"));
        // The other view types are untouched.
        assert_eq!(
            d.draft().view(ViewKind::Layout),
            ToolbarConfig::daniel_default().view(ViewKind::Layout)
        );
        // A file that is not a toolbar file says so.
        assert!(d.import_chief_text("hello").is_err());
        // The embedded copy imports the same way.
        let mut e = dialog();
        let r2 = e.import_daniels_default().unwrap();
        assert_eq!(r2.mapped, r.mapped);
    }

    #[test]
    fn export_then_load_round_trips_and_ok_saves_into_the_live_configuration() {
        let mut d = dialog();
        d.select_bar(ROW1);
        d.toggle("Road");
        let text = d.export_json();
        let mut e = dialog();
        e.import_json(&text).unwrap();
        assert_eq!(e.draft(), d.draft());
        assert!(e.import_json("not json").is_err());
        d.set_locked(true);
        d.commit().unwrap();
        let live = config::current();
        assert!(live.locked);
        assert!(live
            .view(ViewKind::Plan)
            .unwrap()
            .bar(ROW1)
            .unwrap()
            .contains("Road"));
        config::set_current_unsaved(ToolbarConfig::default());
    }

    #[test]
    fn the_search_filters_the_available_buttons_by_name_group_and_members() {
        let mut d = dialog();
        let all: usize = d.available().iter().map(|(_, e)| e.len()).sum();
        assert!(all > 120);
        d.set_search("hinged");
        let hits = d.available();
        assert!(hits
            .iter()
            .any(|(_, es)| es.iter().any(|e| e.key == "Door")));
        d.set_search("terrain");
        assert!(d
            .available()
            .iter()
            .any(|(g, _)| g == "Terrain and Landscape"));
        d.set_search("zzzz");
        assert!(d.available().is_empty());
    }

    #[test]
    fn the_window_draws_headlessly() {
        let mut d = dialog();
        let ctx = egui::Context::default();
        let mut outcome = Outcome::Open;
        let _ = ctx.run(egui::RawInput::default(), |ctx| outcome = d.show(ctx));
        assert_eq!(outcome, Outcome::Open);
    }
}
