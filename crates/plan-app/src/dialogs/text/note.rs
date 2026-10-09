//! Note Specification (TXT-54, TXT-55; manual pp. 559 to 563). Panels: Note,
//! Line Style, Text Style, Object Information and Schedule. The Note panel
//! holds the schedule text and Note Type (with New Note Type), the label
//! above and below the line (`%simple_schedule_number%` by default), the
//! shape, fill, size and angles, and the position.

use super::annot::{
    color_row, draw_preview, insert_menu, line_style_page, lock_check, text_style_page,
    transparency_row, AnnotDialog, Env, Macros, Mode,
};
use crate::dialogs::{on, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab};
use crate::editor::EditorContext;
use eframe::egui::{self, Painter, Rect, Ui};
use plan_core::callout::{note_items, CalloutShape, Note, Vars};
use plan_core::Id;

pub const NOTE_TABS: &[Tab] = &[
    on("Note"),
    on("Line Style"),
    on("Text Style"),
    on("Object Information"),
    on("Schedule"),
];

pub struct NoteDialog {
    frame: SpecDialog,
    form: NoteForm,
}

struct NoteForm {
    spec: Note,
    orig: Note,
    mode: Mode,
    env: Env,
    fields: Fields,
    /// Note types made with New Note Type: `(name, prefix)`.
    new_types: Vec<(String, String)>,
    new_name: String,
    new_prefix: String,
    /// The number the preview shows.
    number: u32,
}

impl NoteDialog {
    pub fn new(spec: Note, mode: Mode, env: Env) -> Self {
        let title = super::annot::title(super::annot::AnnotKind::Note, mode, &env.saved_name);
        Self {
            frame: SpecDialog::new(title, "note"),
            form: NoteForm {
                orig: spec.clone(),
                spec,
                mode,
                env,
                fields: Fields::default(),
                new_types: Vec::new(),
                new_name: String::new(),
                new_prefix: String::new(),
                number: 1,
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn mode(&self) -> Mode {
        self.form.mode
    }

    pub fn title(&self) -> String {
        super::annot::title(
            super::annot::AnnotKind::Note,
            self.form.mode,
            &self.form.env.saved_name,
        )
    }

    pub fn tab_names(&self) -> Vec<&'static str> {
        NOTE_TABS.iter().map(|t| t.name).collect()
    }

    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut Note {
        &mut self.form.spec
    }

    pub fn draft(&self) -> &Note {
        &self.form.spec
    }

    pub fn apply(&self, cx: &mut EditorContext) -> Option<Id> {
        let mut spec = self.form.spec.clone();
        let at = spec.center;
        spec.text = crate::tools::text::TextTool::expand(cx, &spec.text, at);
        spec.text_below = crate::tools::text::TextTool::expand(cx, &spec.text_below, at);
        let fl = cx.floor;
        let new_types = &self.form.new_types;
        let same = self.form.spec == self.form.orig && new_types.is_empty();
        let add_types = |cx: &mut EditorContext| {
            if !new_types.is_empty() {
                let mut types = cx.project.note_types();
                for (n, p) in new_types {
                    types.add(n, p);
                }
                cx.project.set_note_types(&types);
            }
        };
        match self.form.mode {
            Mode::New => {
                if cx.layers().is_locked(&spec.layer) {
                    cx.status = format!("The layer \"{}\" is locked", spec.layer);
                    return None;
                }
                cx.begin_change("Place Note");
                add_types(cx);
                let id = cx.project.add_note(fl, spec);
                AnnotDialog::select(cx, id);
                cx.mark_dirty();
                Some(id)
            }
            Mode::Edit(id) => {
                if same || !lock_check(cx, id) {
                    return None;
                }
                let idx = match cx.floor().annot_of(id) {
                    Some(plan_core::callout::AnnotRef::Note(i)) => i,
                    _ => return None,
                };
                cx.begin_change("Change Note");
                add_types(cx);
                let cur = cx.project.floors[fl].annots.notes[idx].clone();
                cx.project.floors[fl].annots.notes[idx] = Note {
                    items: cur.items,
                    pose_idx: cur.pose_idx,
                    pose: cur.pose,
                    center: cur.center,
                    ignore_no_schedule: cur.ignore_no_schedule,
                    ..spec
                };
                cx.project.sync_annotations();
                cx.mark_dirty();
                Some(id)
            }
            Mode::Defaults => {
                if same {
                    return None;
                }
                cx.begin_change("Change Note Defaults");
                add_types(cx);
                cx.project.annot_defaults.note = Note {
                    items: Vec::new(),
                    pose: None,
                    center: plan_core::geometry::Point::ZERO,
                    ..spec
                };
                cx.mark_dirty();
                Some(0)
            }
        }
    }
}

impl NoteForm {
    fn note_page(&mut self, ui: &mut Ui) {
        let types: Vec<String> = self
            .env
            .note_types
            .iter()
            .cloned()
            .chain(self.new_types.iter().map(|(n, _)| n.clone()))
            .collect();
        let n = &mut self.spec;
        section(ui, "Schedule");
        row(ui, "Text", |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut n.text)
                    .desired_rows(3)
                    .desired_width(260.0),
            );
        });
        row(ui, "", |ui| {
            insert_menu(ui, "note_text", &mut n.text, Macros::Note);
            crate::dialogs::spell_check::local_controls(ui, &mut n.text, false);
        });
        row(ui, "Type", |ui| {
            egui::ComboBox::from_id_salt("note_type")
                .selected_text(n.note_type.clone())
                .show_ui(ui, |ui| {
                    for t in &types {
                        ui.selectable_value(&mut n.note_type, t.clone(), t.as_str());
                    }
                });
            ui.menu_button("New Note Type", |ui| {
                ui.add(egui::TextEdit::singleline(&mut self.new_name).hint_text("Name"));
                ui.add(egui::TextEdit::singleline(&mut self.new_prefix).hint_text("Prefix"));
                if ui.button("Add").clicked()
                    && !self.new_name.trim().is_empty()
                    && !self.new_prefix.is_empty()
                    && !types.contains(&self.new_name.trim().to_string())
                {
                    let name = self.new_name.trim().to_string();
                    n.note_type.clone_from(&name);
                    self.new_types
                        .push((name, std::mem::take(&mut self.new_prefix)));
                    self.new_name.clear();
                    ui.close_menu();
                }
            });
        });
        section(ui, "Label");
        row(ui, "Text Above Line", |ui| {
            ui.add(egui::TextEdit::singleline(&mut n.label).desired_width(160.0));
            insert_menu(ui, "note_label", &mut n.label, Macros::Note);
        });
        row(ui, "Text Below Line", |ui| {
            ui.add(egui::TextEdit::singleline(&mut n.text_below).desired_width(160.0));
            insert_menu(ui, "note_below", &mut n.text_below, Macros::Note);
        });
        section(ui, "Shape");
        ui.checkbox(&mut n.generate_shape, "Generate Shape from Schedule");
        ui.add_enabled_ui(!n.generate_shape, |ui| {
            ui.horizontal_wrapped(|ui| {
                for s in CalloutShape::ALL {
                    ui.radio_value(&mut n.shape, s, s.label());
                }
            });
        });
        section(ui, "Fill Color");
        ui.checkbox(&mut n.generate_fill, "Generate Fill Color from Schedule");
        ui.add_enabled_ui(!n.generate_fill, |ui| {
            ui.checkbox(&mut n.filled, "Filled");
            if n.filled {
                color_row(ui, "Color", &mut n.fill_color);
                transparency_row(ui, &mut n.transparency);
            }
        });
        section(ui, "Size/Orientation");
        ui.checkbox(&mut n.generate_size, "Generate Size from Schedule");
        ui.add_enabled_ui(!n.generate_size, |ui| {
            ui.checkbox(&mut n.auto_size, "Automatic");
            if !n.auto_size {
                self.fields.length_row(ui, "Size", "note_size", &mut n.size);
            }
        });
        ui.checkbox(&mut n.generate_angles, "Generate Angles from Schedule");
        ui.add_enabled_ui(!n.generate_angles, |ui| {
            self.fields
                .degrees_row(ui, "Shape Angle", "deg_note_shape", &mut n.shape_angle);
            row(ui, "Text Angle", |ui| {
                let mut auto = n.text_angle.is_none();
                if ui.checkbox(&mut auto, "Automatic").changed() {
                    n.text_angle = if auto { None } else { Some(n.shape_angle) };
                }
                if let Some(a) = &mut n.text_angle {
                    self.fields.degrees(ui, "deg_note_text", a);
                }
            });
        });
        ui.checkbox(&mut n.auto_adjust_text, "Auto Adjust Text Direction");
        section(ui, "Position");
        self.fields
            .length_row(ui, "X Position", "note_x", &mut n.center.x);
        self.fields
            .length_row(ui, "Y Position", "note_y", &mut n.center.y);
        self.fields.length_row(ui, "Z Position", "note_z", &mut n.z);
        ui.checkbox(&mut n.auto_height, "Auto Adjust Height in Plan View");
    }

    fn info_page(&mut self, ui: &mut Ui) {
        section(ui, "Object Information");
        let n = &mut self.spec;
        let mut remove = None;
        for (i, (k, v)) in n.info.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(k).desired_width(140.0));
                ui.add(egui::TextEdit::singleline(v).desired_width(200.0));
                if ui.small_button("Delete").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            n.info.remove(i);
        }
        if ui.button("Add Field").clicked() {
            n.info.push((String::new(), String::new()));
        }
        ui.weak("Names and values a Note Schedule can use as columns.");
    }

    fn schedule_page(&mut self, ui: &mut Ui) {
        section(ui, "Schedule");
        let n = &mut self.spec;
        ui.checkbox(&mut n.include_in_schedule, "Include in Schedule");
        ui.add_enabled_ui(n.include_in_schedule, |ui| {
            let mut auto = n.category.is_none();
            if ui.checkbox(&mut auto, "Auto Schedule Category").changed() {
                n.category = if auto {
                    None
                } else {
                    Some(n.note_type.clone())
                };
            }
            if let Some(c) = &mut n.category {
                row(ui, "Include in Schedule As", |ui| {
                    ui.add(egui::TextEdit::singleline(c).desired_width(180.0));
                });
            }
        });
        ui.weak("A note is listed in the Note Schedules that include its Note Type.");
    }
}

impl SpecPages for NoteForm {
    fn tabs(&self) -> &'static [Tab] {
        NOTE_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        if self.spec.height <= 0.0 {
            return Some("The text height must be greater than zero".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match NOTE_TABS[tab].name {
            "Note" => self.note_page(ui),
            "Line Style" => {
                let layers = self.env.layers.clone();
                line_style_page(ui, &mut self.spec.line, &mut self.spec.layer, &layers);
            }
            "Text Style" => {
                let styles = self.env.styles.clone();
                text_style_page(
                    ui,
                    &mut self.spec.text_style,
                    &mut self.spec.height,
                    &styles,
                    &mut self.fields,
                );
            }
            "Object Information" => self.info_page(ui),
            "Schedule" => self.schedule_page(ui),
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        // The schedule number is not shown in the preview (as in Chief).
        let v = Vars {
            number: Some(self.number),
            ..Vars::default()
        };
        let mut n = self.spec.clone();
        n.label = n.label.replace("%simple_schedule_number%", "#");
        let g = note_items(&n, &v, false);
        draw_preview(p, rect, &g);
    }
}
