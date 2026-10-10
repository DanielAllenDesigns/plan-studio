//! Tools > Project Information: Owners and their Name-Value Pairs, and the
//! revision table of the plan (`plan_core::schedules::ProjectInfo`; manual
//! pp. 569, 570).
//!
//! The left side lists the **Owners**: the system owners Project, Designer
//! and Client, which cannot be renamed or deleted, and custom owners (shown
//! in italics) that can be added, duplicated, renamed and deleted. The right
//! side lists the selected owner's **Name-Value Pairs**: the system names
//! cannot be edited or deleted; custom names (italic) can be added, renamed
//! (double-click) and deleted; Clear Values blanks every value. Every pair is
//! a text macro in any text object, `%<owner>.<name>%` (`%client.name%`,
//! `%builder.license_no%`), and the Client and Designer pairs go to the
//! REScheck export.
//!
//! The dialog edits a clone; OK stores it with [`apply`] as one undo step
//! ("Project Information"). The layout title blocks read the values through
//! `ProjectInfo::macro_pairs` (`%client%`, `%project.number%`, `%revision%`...).
//! The custom owners travel inside `ProjectInfo::custom` under a reserved key
//! (`plan_core::macros::OWNERS_KEY`).

// The Owners buttons are also calls for the scenarios and the shell.
#![allow(dead_code)]

use super::{on, pv_text, section, Outcome, SpecDialog, SpecPages, Tab, PV_FAINT, PV_INK};
use crate::editor::EditorContext;
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Stroke, StrokeKind, Ui};
use plan_core::macros::{MacroOwners, OWNER_CLIENT, SYSTEM_OWNERS};
use plan_core::schedules::ProjectInfo;
use std::time::{SystemTime, UNIX_EPOCH};

const TABS: &[Tab] = &[on("Owners"), on("Revisions")];

/// Stores `info` on the project as one undo step. Returns whether it changed.
pub fn apply(cx: &mut EditorContext, info: ProjectInfo) -> bool {
    if cx.project.info == info {
        return false;
    }
    cx.begin_change("Project Information");
    cx.project.info = info;
    cx.mark_dirty();
    cx.status = "Project information updated".into();
    true
}

/// `YYYY-MM-DD` for a day count since 1970-01-01 (proleptic Gregorian).
pub fn civil_date(days_since_epoch: i64) -> String {
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// Today's date (UTC) as `YYYY-MM-DD`.
pub fn today() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    civil_date((secs / 86_400) as i64)
}

pub struct ProjectInfoDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    info: ProjectInfo,
    /// The client address as typed (one line per address line).
    address: String,
    /// The custom owners and extra pairs being edited.
    owners: MacroOwners,
    selected: String,
    selected_field: Option<String>,
    /// Text typed for Add (owner), Rename (owner) and Add Field.
    new_owner: String,
    rename_owner: String,
    new_field: String,
    /// A custom name being renamed (double-click): `(old name, new text)`.
    renaming: Option<(String, String)>,
    error: Option<String>,
}

impl ProjectInfoDialog {
    pub fn new(info: &ProjectInfo) -> Self {
        Self {
            frame: SpecDialog::new("Project Information", "project_info"),
            form: Form {
                address: info.client_address.join("\n"),
                owners: MacroOwners::from_info(info),
                info: info.clone(),
                selected: SYSTEM_OWNERS[0].to_string(),
                selected_field: None,
                new_owner: String::new(),
                rename_owner: String::new(),
                new_field: String::new(),
                renaming: None,
                error: None,
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    /// The edited information (address lines and empty rows cleaned up, the
    /// custom owners stored in it).
    pub fn draft(&self) -> ProjectInfo {
        self.form.cleaned()
    }

    #[cfg(test)]
    pub fn info_mut(&mut self) -> &mut ProjectInfo {
        &mut self.form.info
    }

    /// The Owners side, for tests and scenarios.
    pub fn owners_mut(&mut self) -> (&mut MacroOwners, &mut ProjectInfo) {
        (&mut self.form.owners, &mut self.form.info)
    }

    /// Selects an owner in the list.
    pub fn select_owner(&mut self, name: &str) {
        self.form.selected = name.to_string();
        self.form.selected_field = None;
    }

    /// The Add / Duplicate / Rename / Delete buttons of the Owners list and
    /// the Add Field / Delete Field / Clear Values buttons of the pairs, as
    /// calls. Each returns the error to show, if any.
    pub fn add_owner(&mut self, name: &str) -> Result<(), String> {
        self.form.add_owner(name)
    }

    pub fn duplicate_owner(&mut self) -> Result<(), String> {
        self.form.duplicate_owner()
    }

    pub fn rename_owner(&mut self, to: &str) -> Result<(), String> {
        self.form.rename_selected_owner(to)
    }

    pub fn delete_owner(&mut self) -> bool {
        self.form.delete_selected_owner()
    }

    pub fn add_field(&mut self, name: &str) -> Result<(), String> {
        self.form.add_field(name)
    }

    pub fn set_value(&mut self, name: &str, value: &str) -> bool {
        let owner = self.form.selected.clone();
        self.form.set_value(&owner, name, value)
    }

    pub fn delete_field(&mut self, name: &str) -> bool {
        let owner = self.form.selected.clone();
        self.form
            .owners
            .delete_field(&mut self.form.info, &owner, name)
    }

    pub fn clear_values(&mut self) {
        let owner = self.form.selected.clone();
        self.form.owners.clear_values(&mut self.form.info, &owner);
        if owner == OWNER_CLIENT {
            self.form.address.clear();
        }
    }
}

impl Form {
    /// The info with the address text split into lines, blank revision and
    /// custom rows dropped, and the custom owners stored.
    fn cleaned(&self) -> ProjectInfo {
        let mut info = self.info.clone();
        info.client_address = self
            .address
            .lines()
            .map(|l| l.trim_end().to_string())
            .filter(|l| !l.trim().is_empty())
            .collect();
        info.revisions
            .retain(|(n, d, t)| ![n, d, t].iter().all(|s| s.trim().is_empty()));
        info.custom.retain(|(k, _)| !k.trim().is_empty());
        self.owners.store(&mut info);
        info
    }

    fn set_value(&mut self, owner: &str, name: &str, value: &str) -> bool {
        if owner == OWNER_CLIENT && name == "Address" {
            self.address = value.to_string();
            return true;
        }
        self.owners.set_value(&mut self.info, owner, name, value)
    }

    fn add_owner(&mut self, name: &str) -> Result<(), String> {
        let n = self.owners.add_owner(name)?;
        self.selected = n;
        self.selected_field = None;
        Ok(())
    }

    fn duplicate_owner(&mut self) -> Result<(), String> {
        // The address buffer is the Client address while editing.
        self.info.client_address = self
            .address
            .lines()
            .map(|l| l.trim_end().to_string())
            .filter(|l| !l.trim().is_empty())
            .collect();
        let n = self.owners.duplicate_owner(&self.info, &self.selected)?;
        self.selected = n;
        self.selected_field = None;
        Ok(())
    }

    fn rename_selected_owner(&mut self, to: &str) -> Result<(), String> {
        self.owners.rename_owner(&self.selected, to)?;
        self.selected = to.trim().to_string();
        Ok(())
    }

    fn delete_selected_owner(&mut self) -> bool {
        let gone = self.owners.delete_owner(&self.selected);
        if gone {
            self.selected = SYSTEM_OWNERS[0].to_string();
            self.selected_field = None;
        }
        gone
    }

    fn add_field(&mut self, name: &str) -> Result<(), String> {
        let n = self
            .owners
            .add_field(&mut self.info, &self.selected.clone(), name)?;
        self.selected_field = Some(n);
        Ok(())
    }

    /// The Owners panel: the list and its four buttons.
    fn owners_list(&mut self, ui: &mut Ui) {
        section(ui, "Owners");
        let views = self.owners.views(&self.info);
        for v in &views {
            let mut text = egui::RichText::new(&v.name);
            if !v.system {
                text = text.italics();
            }
            if ui.selectable_label(self.selected == v.name, text).clicked() {
                self.selected = v.name.clone();
                self.selected_field = None;
                self.rename_owner = v.name.clone();
            }
        }
        if !views.iter().any(|v| v.name == self.selected) {
            self.selected = SYSTEM_OWNERS[0].to_string();
        }
        ui.add_space(4.0);
        let custom = !SYSTEM_OWNERS.contains(&self.selected.as_str());
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.new_owner).desired_width(110.0));
            if ui.button("Add").clicked() {
                let n = std::mem::take(&mut self.new_owner);
                self.error = self.add_owner(&n).err();
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Duplicate").clicked() {
                self.error = self.duplicate_owner().err();
            }
            if ui
                .add_enabled(custom, egui::Button::new("Delete"))
                .clicked()
            {
                self.delete_selected_owner();
            }
        });
        ui.horizontal(|ui| {
            ui.add_enabled(
                custom,
                egui::TextEdit::singleline(&mut self.rename_owner).desired_width(110.0),
            );
            if ui
                .add_enabled(custom, egui::Button::new("Rename"))
                .clicked()
            {
                let to = self.rename_owner.clone();
                self.error = self.rename_selected_owner(&to).err();
            }
        });
    }

    /// The Name-Value Pairs of the selected owner.
    fn pairs_panel(&mut self, ui: &mut Ui) {
        let owner = self.selected.clone();
        section(ui, &format!("Name-Value Pairs: {owner}"));
        let Some(view) = self
            .owners
            .views(&self.info)
            .into_iter()
            .find(|v| v.name == owner)
        else {
            return;
        };
        let mut changes: Vec<(String, String)> = Vec::new();
        let mut finish_rename: Option<(String, String)> = None;
        egui::Grid::new("project_info_pairs")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.strong("Name");
                ui.strong("Value");
                ui.end_row();
                for p in &view.pairs {
                    let is_sel = self.selected_field.as_deref() == Some(p.name.as_str());
                    // The name: system names are plain, custom names are
                    // italic and rename on a double-click.
                    let renaming_here = !p.system
                        && self
                            .renaming
                            .as_ref()
                            .is_some_and(|(old, _)| *old == p.name);
                    if renaming_here {
                        if let Some((old, buf)) = self.renaming.as_mut() {
                            let r = ui.add(egui::TextEdit::singleline(buf).desired_width(110.0));
                            if r.lost_focus() {
                                finish_rename = Some((old.clone(), buf.clone()));
                            }
                        }
                    } else {
                        let mut t = egui::RichText::new(&p.name);
                        if !p.system {
                            t = t.italics();
                        }
                        let r = ui.selectable_label(is_sel, t);
                        if r.clicked() {
                            self.selected_field = Some(p.name.clone());
                        }
                        if r.double_clicked() && !p.system {
                            self.renaming = Some((p.name.clone(), p.name.clone()));
                        }
                    }
                    let mut v = if owner == OWNER_CLIENT && p.name == "Address" {
                        self.address.clone()
                    } else {
                        p.value.clone()
                    };
                    let changed = if owner == OWNER_CLIENT && p.name == "Address" {
                        ui.add(
                            egui::TextEdit::multiline(&mut v)
                                .desired_rows(3)
                                .desired_width(240.0),
                        )
                        .changed()
                    } else {
                        ui.add(egui::TextEdit::singleline(&mut v).desired_width(240.0))
                            .changed()
                    };
                    if changed {
                        changes.push((p.name.clone(), v));
                    }
                    ui.end_row();
                }
            });
        for (name, v) in changes {
            self.set_value(&owner, &name, &v);
        }
        if let Some((old, new)) = finish_rename {
            self.renaming = None;
            if old != new.trim() {
                self.error = self
                    .owners
                    .rename_field(&mut self.info, &owner, &old, &new)
                    .err();
            }
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.new_field).desired_width(110.0));
            if ui.button("Add Field").clicked() {
                let n = std::mem::take(&mut self.new_field);
                self.error = self.add_field(&n).err();
            }
            let deletable = self
                .selected_field
                .as_deref()
                .is_some_and(|f| view.pairs.iter().any(|p| p.name == f && !p.system));
            if ui
                .add_enabled(deletable, egui::Button::new("Delete Field"))
                .clicked()
            {
                if let Some(f) = self.selected_field.take() {
                    self.owners.delete_field(&mut self.info, &owner, &f);
                }
            }
            if ui.button("Clear Values").clicked() {
                self.owners.clear_values(&mut self.info, &owner);
                if owner == OWNER_CLIENT {
                    self.address.clear();
                }
            }
        });
        ui.weak(format!(
            "Use a name in any text as %{}.<name>%; the Client and Designer pairs go to REScheck.",
            plan_core::macros::slug(&owner)
        ));
    }

    fn owners_page(&mut self, ui: &mut Ui) {
        ui.columns(2, |cols| {
            self.owners_list(&mut cols[0]);
            self.pairs_panel(&mut cols[1]);
        });
        if let Some(e) = &self.error {
            ui.colored_label(egui::Color32::from_rgb(0xE0, 0x4B, 0x4B), e);
        }
    }

    fn revisions(&mut self, ui: &mut Ui) {
        section(ui, "Revisions");
        let mut remove = None;
        egui::Grid::new("project_info_revisions")
            .num_columns(4)
            .striped(true)
            .show(ui, |ui| {
                ui.strong("No.");
                ui.strong("Date");
                ui.strong("Description");
                ui.label("");
                ui.end_row();
                for (i, (n, d, t)) in self.info.revisions.iter_mut().enumerate() {
                    ui.add(egui::TextEdit::singleline(n).desired_width(36.0));
                    ui.add(egui::TextEdit::singleline(d).desired_width(90.0));
                    ui.add(egui::TextEdit::singleline(t).desired_width(170.0));
                    if ui
                        .small_button("\u{2715}")
                        .on_hover_text("Remove")
                        .clicked()
                    {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
        if let Some(i) = remove {
            self.info.revisions.remove(i);
        }
        ui.add_space(4.0);
        if ui.button("Add Revision").clicked() {
            let next = self.info.revisions.len() + 1;
            self.info
                .revisions
                .push((next.to_string(), today(), String::new()));
            self.info.revision = next.to_string();
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Date");
            ui.add(egui::TextEdit::singleline(&mut self.info.date).desired_width(120.0));
            if ui.button("Today").clicked() {
                self.info.date = today();
            }
        });
    }
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match tab {
            0 => self.owners_page(ui),
            _ => self.revisions(ui),
        }
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        // A title block strip with the main values.
        painter.rect_stroke(rect, 2.0, Stroke::new(1.0_f32, PV_INK), StrokeKind::Inside);
        let info = self.cleaned();
        let mut y = rect.min.y + 12.0;
        let mut line = |label: &str, value: &str| {
            pv_text(
                painter,
                Pos2::new(rect.min.x + 6.0, y),
                Align2::LEFT_CENTER,
                label,
                9.0,
            );
            let shown = if value.is_empty() { "\u{2014}" } else { value };
            pv_text(
                painter,
                Pos2::new(rect.min.x + 6.0, y + 12.0),
                Align2::LEFT_CENTER,
                shown,
                12.0,
            );
            y += 34.0;
            painter.hline(rect.x_range(), y - 8.0, Stroke::new(1.0_f32, PV_FAINT));
        };
        line("CLIENT", &info.client_name);
        line("ADDRESS", &info.client_address.join(", "));
        line("PROJECT NO.", &info.project_number);
        line("DATE", &info.date);
        line("REVISION", &info.revision);
        line("DRAWN BY", &info.drawn_by);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;

    #[test]
    fn civil_dates_are_right() {
        assert_eq!(civil_date(0), "1970-01-01");
        assert_eq!(civil_date(20_000), "2024-10-04");
        assert_eq!(civil_date(19_782), "2024-02-29");
        assert_eq!(civil_date(-1), "1969-12-31");
        assert_eq!(today().len(), 10);
    }

    #[test]
    fn the_draft_splits_the_address_and_drops_blank_rows() {
        let mut d = ProjectInfoDialog::new(&ProjectInfo::default());
        d.form.address = "12 Oak St\n\nAtlanta, GA  \n".into();
        d.info_mut().revisions = vec![
            ("1".into(), "2026-10-01".into(), "Issued".into()),
            (String::new(), " ".into(), String::new()),
        ];
        d.info_mut().custom = vec![("lot".into(), "14".into()), (" ".into(), "ignored".into())];
        let out = d.draft();
        assert_eq!(out.client_address, ["12 Oak St", "Atlanta, GA"]);
        assert_eq!(out.revisions.len(), 1);
        assert_eq!(out.custom, [("lot".to_string(), "14".to_string())]);
    }

    #[test]
    fn apply_is_one_undo_step_and_round_trips() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let mut info = cx.project.info.clone();
        assert!(!apply(&mut cx, info.clone()), "no change, no step");
        assert!(cx.undo_label().is_none());
        info.client_name = "Pat Smith".into();
        info.project_number = "26-014".into();
        info.revisions = vec![("A".into(), "2026-10-08".into(), "Permit set".into())];
        info.custom = vec![("lot".into(), "14".into())];
        assert!(apply(&mut cx, info.clone()));
        assert_eq!(cx.undo_label(), Some("Project Information"));
        let back = plan_core::Project::from_json(&cx.project.to_json().unwrap()).unwrap();
        assert_eq!(back.info, info);
        assert_eq!(back.info.expand("%client% %custom.lot%"), "Pat Smith 14");
        cx.undo();
        assert_eq!(cx.project.info, ProjectInfo::default());
    }

    #[test]
    fn the_dialog_draws_every_tab() {
        let mut d = ProjectInfoDialog::new(&ProjectInfo {
            client_name: "Pat".into(),
            client_address: vec!["12 Oak St".into()],
            revisions: vec![("1".into(), "d".into(), "t".into())],
            custom: vec![("k".into(), "v".into())],
            ..ProjectInfo::default()
        });
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                assert_eq!(d.show(ctx), Outcome::Open);
            });
        }
        for tab in 0..TABS.len() {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    d.form.page(ui, tab);
                    let (_, painter) =
                        ui.allocate_painter(egui::vec2(200.0, 300.0), egui::Sense::hover());
                    d.form.preview(&painter, painter.clip_rect());
                });
            });
        }
    }

    #[test]
    fn owners_and_pairs_follow_the_buttons() {
        use plan_core::macros::MacroOwners;
        let mut d = ProjectInfoDialog::new(&ProjectInfo::default());
        // The system owners cannot be renamed or deleted.
        d.select_owner("Client");
        assert!(d.rename_owner("Buyer").is_err());
        assert!(!d.delete_owner());
        // A custom owner with a field, duplicated and deleted.
        d.add_owner("Builder").unwrap();
        assert!(d.add_owner("builder").is_err());
        d.add_field("License No").unwrap();
        assert!(d.add_field("license no").is_err());
        assert!(d.set_value("License No", "GA-5521"));
        d.duplicate_owner().unwrap();
        assert!(d.rename_owner("Framer").is_ok());
        assert!(d.delete_owner());
        // The Client's address is typed in lines; Clear Values blanks it.
        d.select_owner("Client");
        assert!(d.set_value("Name", "Pat Smith"));
        assert!(d.set_value("Address", "12 Oak St\nAtlanta, GA"));
        let out = d.draft();
        assert_eq!(out.client_name, "Pat Smith");
        assert_eq!(out.client_address, ["12 Oak St", "Atlanta, GA"]);
        // The owners ride in the information and survive the store.
        let owners = MacroOwners::from_info(&out);
        assert_eq!(owners.value(&out, "Builder", "License No"), "GA-5521");
        assert!(owners.views(&out).iter().all(|v| v.name != "Framer"));
        let mut cx = EditorContext::new(plan_defaults::embedded());
        assert!(apply(&mut cx, out.clone()));
        let back = plan_core::Project::from_json(&cx.project.to_json().unwrap()).unwrap();
        assert_eq!(
            MacroOwners::from_info(&back.info).value(&back.info, "Builder", "License No"),
            "GA-5521"
        );
        d.clear_values();
        let cleared = d.draft();
        assert_eq!(cleared.client_name, "");
        assert!(cleared.client_address.is_empty());
        // Both sides of the window draw with a custom owner selected.
        d.select_owner("Builder");
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| d.form.page(ui, 0));
        });
    }
}
