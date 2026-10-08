//! Schedule Specification: the dialog behind a double-click on a placed
//! schedule. Tabs: General (title, kind, floors, filter, the column list with
//! show/hide, rename and up/down, sort), Labels (show callout labels,
//! numbering, prefix), Text Style and Layer.
//!
//! The dialog edits a clone of the [`Schedule`]; OK hands it back and the
//! host stores it as one undo step. The General page also has *Export CSV*
//! and *Open in Window*; the dialog cannot reach the plan, so it only raises
//! those as [`SpecActions`] for the host to carry out.

use super::{on, row, section, Outcome, SpecDialog, SpecPages, Tab, PV_ACCENT, PV_FAINT, PV_INK};
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Stroke, StrokeKind, Ui};
use plan_core::schedules::{FloorScope, Numbering, Schedule, ScheduleKind};
use plan_core::Id;

const TABS: &[Tab] = &[on("General"), on("Labels"), on("Text Style"), on("Layer")];

/// What the user asked for besides editing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpecActions {
    pub export_csv: bool,
    pub open_window: bool,
}

pub struct ScheduleSpecDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    /// Floor the schedule is placed on.
    floor: usize,
    def: Schedule,
    text_styles: Vec<String>,
    layers: Vec<String>,
    actions: SpecActions,
}

impl ScheduleSpecDialog {
    /// `text_styles` and `layers` are the plan's names for the pickers.
    pub fn new(floor: usize, def: Schedule, text_styles: Vec<String>, layers: Vec<String>) -> Self {
        let mut def = def;
        def.reconcile_columns();
        Self {
            frame: SpecDialog::new("Schedule Specification", "schedule_spec"),
            form: Form {
                floor,
                def,
                text_styles,
                layers,
                actions: SpecActions::default(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn floor(&self) -> usize {
        self.form.floor
    }

    pub fn id(&self) -> Id {
        self.form.def.id
    }

    pub fn draft(&self) -> &Schedule {
        &self.form.def
    }

    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut Schedule {
        &mut self.form.def
    }

    #[cfg(test)]
    pub fn raise_open_window(&mut self) {
        self.form.actions.open_window = true;
    }

    /// The export / open-window requests since the last call.
    pub fn take_actions(&mut self) -> SpecActions {
        std::mem::take(&mut self.form.actions)
    }
}

fn combo<T: PartialEq + Copy>(ui: &mut Ui, salt: &str, value: &mut T, options: &[(T, &str)]) {
    let current = options
        .iter()
        .find(|(v, _)| v == value)
        .map_or("", |(_, n)| *n);
    egui::ComboBox::from_id_salt(salt)
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (v, name) in options {
                ui.selectable_value(value, *v, *name);
            }
        });
}

impl Form {
    fn general(&mut self, ui: &mut Ui) {
        section(ui, "General");
        row(ui, "Title", |ui| {
            let hint = self.def.kind.title();
            ui.add(
                egui::TextEdit::singleline(&mut self.def.title)
                    .hint_text(hint)
                    .desired_width(240.0),
            )
        });
        row(ui, "Schedule type", |ui| {
            let mut kind = self.def.kind;
            let options: Vec<(ScheduleKind, &str)> = ScheduleKind::ALL
                .iter()
                .map(|k| {
                    (
                        *k,
                        if *k == ScheduleKind::General {
                            "General (custom)"
                        } else {
                            k.name()
                        },
                    )
                })
                .collect();
            combo(ui, "schedule_kind", &mut kind, &options);
            if kind != self.def.kind {
                self.def.set_kind(kind);
            }
        });
        row(ui, "Floors", |ui| {
            combo(
                ui,
                "schedule_scope",
                &mut self.def.floor_scope,
                &[
                    (FloorScope::ThisFloor, "This floor only"),
                    (FloorScope::All, "All floors"),
                ],
            )
        });
        row(ui, "Filter", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.def.filter)
                    .hint_text("only rows containing...")
                    .desired_width(240.0),
            )
        });

        section(ui, "Columns");
        let mut pending: Option<(usize, bool)> = None;
        egui::Grid::new("schedule_columns")
            .num_columns(5)
            .striped(true)
            .show(ui, |ui| {
                ui.strong("Show");
                ui.strong("Heading");
                ui.strong("Field");
                ui.label("");
                ui.label("");
                ui.end_row();
                let n = self.def.columns.len();
                for (i, c) in self.def.columns.iter_mut().enumerate() {
                    ui.checkbox(&mut c.visible, "");
                    ui.add(egui::TextEdit::singleline(&mut c.title).desired_width(130.0));
                    ui.weak(&c.field);
                    if ui
                        .add_enabled(i > 0, egui::Button::new("\u{25B2}").small())
                        .on_hover_text("Move up")
                        .clicked()
                    {
                        pending = Some((i, true));
                    }
                    if ui
                        .add_enabled(i + 1 < n, egui::Button::new("\u{25BC}").small())
                        .on_hover_text("Move down")
                        .clicked()
                    {
                        pending = Some((i, false));
                    }
                    ui.end_row();
                }
            });
        if let Some((i, up)) = pending {
            self.def.move_column(i, up);
        }
        if !self.def.columns.iter().any(|c| c.visible) {
            ui.colored_label(super::ERROR_RED, "Show at least one column");
        }

        section(ui, "Sort");
        row(ui, "Sort by", |ui| {
            let current = self
                .def
                .columns
                .iter()
                .find(|c| c.field == self.def.sort.field)
                .map_or("(order in plan)".to_string(), |c| c.title.clone());
            egui::ComboBox::from_id_salt("schedule_sort")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.def.sort.field, String::new(), "(order in plan)");
                    for c in &self.def.columns {
                        ui.selectable_value(&mut self.def.sort.field, c.field.clone(), &c.title);
                    }
                });
            ui.checkbox(&mut self.def.sort.descending, "Descending");
        });

        section(ui, "Output");
        ui.horizontal(|ui| {
            if ui.button("Export CSV\u{2026}").clicked() {
                self.actions.export_csv = true;
            }
            if ui.button("Open in Window").clicked() {
                self.actions.open_window = true;
            }
        });
    }

    fn labels(&mut self, ui: &mut Ui) {
        section(ui, "Labels");
        let supported = self.def.kind.has_labels();
        ui.add_enabled_ui(supported, |ui| {
            ui.checkbox(
                &mut self.def.show_labels,
                "Show schedule number labels in the plan",
            );
            row(ui, "Numbering", |ui| {
                combo(
                    ui,
                    "schedule_numbering",
                    &mut self.def.numbering,
                    &[
                        (Numbering::ByFloor, "By floor (each floor starts at 01)"),
                        (Numbering::Whole, "Whole plan (keeps counting)"),
                    ],
                )
            });
        });
        row(ui, "Prefix", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.def.label_prefix).desired_width(80.0));
            if ui.button("Default").clicked() {
                self.def.label_prefix = self.def.kind.default_prefix().to_string();
            }
        });
        ui.add_space(6.0);
        if supported {
            ui.weak(format!(
                "Marks read {}01, {}02, ... in order across the floor. A door or window with its own schedule number shows that instead.",
                self.def.label_prefix, self.def.label_prefix
            ));
        } else {
            ui.weak(format!(
                "{} objects have no callout label in the plan; the prefix only sets the Mark column.",
                self.def.kind.name()
            ));
        }
        section(ui, "Callouts by kind");
        egui::Grid::new("schedule_prefixes").show(ui, |ui| {
            for k in ScheduleKind::ALL.into_iter().filter(|k| k.has_labels()) {
                ui.label(k.name());
                ui.weak(format!("{}01", k.default_prefix()));
                ui.end_row();
            }
        });
    }

    fn text_style(&mut self, ui: &mut Ui) {
        section(ui, "Text Style");
        row(ui, "Table text style", |ui| {
            egui::ComboBox::from_id_salt("schedule_text_style")
                .selected_text(self.def.text_style.clone())
                .show_ui(ui, |ui| {
                    for name in &self.text_styles {
                        ui.selectable_value(&mut self.def.text_style, name.clone(), name);
                    }
                });
        });
        ui.add_space(6.0);
        ui.weak("Callout labels use the \"Schedule Label\" style when the plan has one.");
    }

    fn layer(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        row(ui, "Layer", |ui| {
            egui::ComboBox::from_id_salt("schedule_layer")
                .selected_text(self.def.layer.clone())
                .show_ui(ui, |ui| {
                    for name in &self.layers {
                        ui.selectable_value(&mut self.def.layer, name.clone(), name);
                    }
                });
        });
    }
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        (!self.def.columns.iter().any(|c| c.visible)).then(|| "Show at least one column".into())
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match tab {
            0 => self.general(ui),
            1 => self.labels(ui),
            2 => self.text_style(ui),
            _ => self.layer(ui),
        }
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        // The table's skeleton: title bar, heading row and a few rows.
        let cols: Vec<&str> = self
            .def
            .visible_columns()
            .map(|c| c.title.as_str())
            .collect();
        if cols.is_empty() {
            return;
        }
        let title_h: f32 = 22.0;
        let row_h: f32 = 16.0;
        let body = Rect::from_min_size(
            rect.min,
            egui::vec2(rect.width(), (title_h + row_h * 5.0).min(rect.height())),
        );
        painter.rect_stroke(body, 0.0, Stroke::new(1.5_f32, PV_INK), StrokeKind::Inside);
        painter.text(
            Pos2::new(body.center().x, body.min.y + title_h * 0.5),
            Align2::CENTER_CENTER,
            self.def.display_title(),
            egui::FontId::proportional(11.0),
            PV_INK,
        );
        painter.hline(
            body.x_range(),
            body.min.y + title_h,
            Stroke::new(1.0_f32, PV_INK),
        );
        let cw = body.width() / cols.len() as f32;
        for (i, c) in cols.iter().enumerate() {
            let x = body.min.x + cw * i as f32;
            if i > 0 {
                painter.vline(
                    x,
                    (body.min.y + title_h)..=body.max.y,
                    Stroke::new(1.0_f32, PV_FAINT),
                );
            }
            painter.text(
                Pos2::new(x + 3.0, body.min.y + title_h + row_h * 0.5),
                Align2::LEFT_CENTER,
                c,
                egui::FontId::proportional(8.0),
                PV_ACCENT,
            );
            for r in 1..4 {
                painter.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(x + 3.0, body.min.y + title_h + row_h * (r as f32 + 0.4)),
                        egui::vec2((cw - 8.0).max(4.0), 4.0),
                    ),
                    1.0,
                    PV_FAINT,
                );
            }
        }
        painter.hline(
            body.x_range(),
            body.min.y + title_h + row_h,
            Stroke::new(1.0_f32, PV_INK),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::geometry::Point;

    fn dialog() -> ScheduleSpecDialog {
        let mut def = Schedule::new(ScheduleKind::Door, Point::ZERO);
        def.id = 7;
        ScheduleSpecDialog::new(
            0,
            def,
            vec!["Schedule Style".into(), "Default Text Style".into()],
            vec!["Schedules".into(), "Text".into()],
        )
    }

    #[test]
    fn toggling_a_column_hides_it_in_the_draft() {
        let mut d = dialog();
        assert!(d.draft().visible_columns().any(|c| c.field == "swing"));
        let i = d
            .draft()
            .columns
            .iter()
            .position(|c| c.field == "swing")
            .unwrap();
        d.draft_mut().set_column_visible(i, false);
        assert!(!d.draft().visible_columns().any(|c| c.field == "swing"));
        assert_eq!(d.id(), 7);
        assert_eq!(d.floor(), 0);
    }

    #[test]
    fn hiding_every_column_blocks_ok() {
        let mut d = dialog();
        for c in &mut d.draft_mut().columns {
            c.visible = false;
        }
        assert!(d.form.error().is_some());
        d.draft_mut().columns[0].visible = true;
        assert!(d.form.error().is_none());
    }

    #[test]
    fn every_page_and_the_preview_draw() {
        let mut d = dialog();
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
                        ui.allocate_painter(egui::vec2(220.0, 300.0), egui::Sense::hover());
                    d.form.preview(&painter, painter.clip_rect());
                });
            });
        }
        assert_eq!(d.take_actions(), SpecActions::default());
    }

    #[test]
    fn actions_are_raised_once() {
        let mut d = dialog();
        d.form.actions.export_csv = true;
        assert!(d.take_actions().export_csv);
        assert!(!d.take_actions().export_csv);
    }
}
