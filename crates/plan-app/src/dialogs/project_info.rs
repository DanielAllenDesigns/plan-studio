//! Tools > Project Information: the client, designer, job number, date,
//! revision table and custom fields of the plan (`plan_core::schedules::ProjectInfo`).
//!
//! The dialog edits a clone; OK stores it with [`apply`] as one undo step
//! ("Project Information"). The layout title blocks read the values through
//! `ProjectInfo::macro_pairs` (`%client%`, `%project.number%`, `%revision%`...).

use super::{on, pv_text, row, section, Outcome, SpecDialog, SpecPages, Tab, PV_FAINT, PV_INK};
use crate::editor::EditorContext;
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Stroke, StrokeKind, Ui};
use plan_core::schedules::ProjectInfo;
use std::time::{SystemTime, UNIX_EPOCH};

const TABS: &[Tab] = &[
    on("Client"),
    on("Project"),
    on("Designer"),
    on("Revisions"),
    on("Custom Fields"),
];

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
}

impl ProjectInfoDialog {
    pub fn new(info: &ProjectInfo) -> Self {
        Self {
            frame: SpecDialog::new("Project Information", "project_info"),
            form: Form {
                address: info.client_address.join("\n"),
                info: info.clone(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    /// The edited information (address lines and empty rows cleaned up).
    pub fn draft(&self) -> ProjectInfo {
        self.form.cleaned()
    }

    #[cfg(test)]
    pub fn info_mut(&mut self) -> &mut ProjectInfo {
        &mut self.form.info
    }
}

fn text_row(ui: &mut Ui, label: &str, value: &mut String) {
    row(ui, label, |ui| {
        ui.add(egui::TextEdit::singleline(value).desired_width(260.0))
    });
}

impl Form {
    /// The info with the address text split into lines and blank revision
    /// and custom rows dropped.
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
        info
    }

    fn client(&mut self, ui: &mut Ui) {
        section(ui, "Client");
        text_row(ui, "Name", &mut self.info.client_name);
        row(ui, "Address", |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut self.address)
                    .desired_rows(3)
                    .desired_width(260.0),
            )
        });
        text_row(ui, "Phone", &mut self.info.client_phone);
        text_row(ui, "Email", &mut self.info.client_email);
    }

    fn project(&mut self, ui: &mut Ui) {
        section(ui, "Project");
        text_row(ui, "Project number", &mut self.info.project_number);
        text_row(ui, "Project address", &mut self.info.project_address);
        row(ui, "Date", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.info.date).desired_width(120.0));
            if ui.button("Today").clicked() {
                self.info.date = today();
            }
        });
        text_row(ui, "Current revision", &mut self.info.revision);
    }

    fn designer(&mut self, ui: &mut Ui) {
        section(ui, "Designer");
        text_row(ui, "Designer", &mut self.info.designer);
        text_row(ui, "Company", &mut self.info.company);
        text_row(ui, "Drawn by", &mut self.info.drawn_by);
        text_row(ui, "Checked by", &mut self.info.checked_by);
        ui.add_space(6.0);
        ui.weak("Drawn by fills the title block's %designer% (DRAWN BY) box when set.");
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
    }

    fn custom(&mut self, ui: &mut Ui) {
        section(ui, "Custom Fields");
        ui.weak("Each field is available in layout text as %custom.name%.");
        let mut remove = None;
        egui::Grid::new("project_info_custom")
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                ui.strong("Name");
                ui.strong("Value");
                ui.label("");
                ui.end_row();
                for (i, (k, v)) in self.info.custom.iter_mut().enumerate() {
                    ui.add(egui::TextEdit::singleline(k).desired_width(110.0));
                    ui.add(egui::TextEdit::singleline(v).desired_width(190.0));
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
            self.info.custom.remove(i);
        }
        ui.add_space(4.0);
        if ui.button("Add Field").clicked() {
            self.info.custom.push((String::new(), String::new()));
        }
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
            0 => self.client(ui),
            1 => self.project(ui),
            2 => self.designer(ui),
            3 => self.revisions(ui),
            _ => self.custom(ui),
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
}
