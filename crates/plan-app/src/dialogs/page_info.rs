//! Layout Page Information (manual p. 1419 to 1421): the dialog that
//! replaces Round 14's Page Specification. It edits the Page Information of
//! every page of the layout in one go (the Selected Page list switches
//! between them), the Page Template Options, the Page Revisions table with
//! its New / Edit / Delete / Move buttons, and the two Plan Studio settings
//! that were on Page Specification: the page's own sheet and "no border or
//! title block".
//!
//! One OK is one undo step: `shell::layout_window::LayoutView::apply_page_info`
//! writes everything the dialog changed.

use super::layout::frame;
use super::layout_revisions::RevisionDialog;
use super::{row, section, Outcome};
use eframe::egui::{self, Ui};
use plan_docs::SheetSize;
use plan_layout::{
    resolve_labels, CustomSheetSize, Layout, LayoutPage, PageInfo, PageRevision, SheetChoice,
};

/// A page's own sheet and title block switch (Plan Studio's, kept from
/// Page Specification).
#[derive(Clone, Debug, PartialEq)]
pub struct PageSheet {
    /// The page's own sheet; `None` follows the layout.
    pub sheet: Option<SheetChoice>,
    /// The page's own sheet turned upright.
    pub portrait: bool,
    /// The page prints without the border and title block.
    pub no_title_block: bool,
}

impl PageSheet {
    /// The sheet settings of `page` in `layout`.
    pub fn of(page: &LayoutPage, layout: &Layout) -> Self {
        let (sheet, portrait) = match page.size_override_in {
            None => (None, false),
            Some((w, h)) => {
                let portrait = h > w;
                let (long, short) = (w.max(h), w.min(h));
                let known = layout.size_choices().into_iter().find(|c| {
                    let (cl, cs) = c.inches();
                    (cl - long).abs() < 1e-6 && (cs - short).abs() < 1e-6
                });
                let choice = known.unwrap_or_else(|| {
                    SheetChoice::Custom(CustomSheetSize::new("This page", long, short))
                });
                (Some(choice), portrait)
            }
        };
        Self {
            sheet,
            portrait,
            no_title_block: page.no_title_block,
        }
    }
}

/// One page in the dialog: its key, the Page Information being edited and
/// its sheet settings.
#[derive(Clone, Debug, PartialEq)]
pub struct PageEntry {
    pub number: u32,
    pub info: PageInfo,
    pub sheet: PageSheet,
}

/// The Layout Page Information dialog.
pub struct PageInfoDialog {
    entries: Vec<PageEntry>,
    /// The layout's pages without their content: the flags before the edit,
    /// to check what OK would change.
    original: Layout,
    selected: usize,
    choices: Vec<SheetChoice>,
    layout_sheet: String,
    rev_sel: Option<usize>,
    /// The Revision Specification being edited: which revision (`None` = a
    /// new one) and the dialog.
    editor: Option<(Option<usize>, RevisionDialog)>,
    today: String,
    designer: String,
}

impl PageInfoDialog {
    /// The dialog on `layout`, with page `selected` (an index) chosen.
    /// `today` and `designer` fill a new revision's Date and Revised By.
    pub fn new(layout: &Layout, selected: usize, today: &str, designer: &str) -> Self {
        let entries: Vec<PageEntry> = layout
            .pages
            .iter()
            .map(|p| PageEntry {
                number: p.number,
                info: PageInfo::of(p),
                sheet: PageSheet::of(p, layout),
            })
            .collect();
        let mut original = Layout::new("pages", SheetSize::ArchC);
        for p in &layout.pages {
            let q = original.add_page(p.number, p.title.clone());
            q.template_page = p.template_page;
            q.template = p.template;
            q.in_layout_table = p.in_layout_table;
        }
        let mut choices = layout.size_choices();
        for e in &entries {
            if let Some(c) = &e.sheet.sheet {
                if !choices.contains(c) {
                    choices.insert(0, c.clone());
                }
            }
        }
        Self {
            selected: selected.min(entries.len().saturating_sub(1)),
            entries,
            original,
            choices,
            layout_sheet: layout.sheet_choice().label(),
            rev_sel: None,
            editor: None,
            today: today.to_string(),
            designer: designer.to_string(),
        }
    }

    /// Every page as edited.
    pub fn entries(&self) -> &[PageEntry] {
        &self.entries
    }

    /// The edited page (an index into the entries).
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Chooses the page to edit (the Selected Page list).
    pub fn select(&mut self, index: usize) {
        if index < self.entries.len() {
            self.selected = index;
            self.rev_sel = None;
        }
    }

    /// The Page Information of the page being edited.
    pub fn info_mut(&mut self) -> Option<&mut PageInfo> {
        self.entries.get_mut(self.selected).map(|e| &mut e.info)
    }

    /// The sheet settings of the page being edited.
    pub fn sheet_mut(&mut self) -> Option<&mut PageSheet> {
        self.entries.get_mut(self.selected).map(|e| &mut e.sheet)
    }

    /// `(page number, information)` of every page, to apply with
    /// [`Layout::set_page_infos`].
    pub fn infos(&self) -> Vec<(u32, PageInfo)> {
        self.entries
            .iter()
            .map(|e| (e.number, e.info.clone()))
            .collect()
    }

    /// The layout with the drafts' flags, for questions about templates.
    fn shadow(&self) -> Layout {
        let mut l = Layout::new("pages", SheetSize::ArchC);
        for e in &self.entries {
            let p = l.add_page(e.number, e.info.title.clone());
            p.template_page = e.info.template_page;
            p.template = e.info.template;
        }
        l
    }

    /// The label each page will show.
    pub fn labels(&self) -> Vec<String> {
        resolve_labels(
            self.entries
                .iter()
                .map(|e| (e.info.label.as_str(), e.info.template_page, e.number)),
        )
    }

    /// Why OK is refused, if it is.
    pub fn error(&self) -> Option<String> {
        self.original.clone().set_page_infos(&self.infos()).err()
    }

    /// Do other pages use the page template at `index`? Then it cannot stop
    /// being a template.
    pub fn is_assigned(&self, index: usize) -> bool {
        self.shadow().assigned_count(index) > 0
    }

    /// New revision on the selected page (the New button).
    pub fn new_revision(&mut self) {
        self.editor = Some((
            None,
            RevisionDialog::new_for_page(&self.today, &self.designer),
        ));
    }

    /// Edit the selected revision (the Edit button).
    pub fn edit_revision(&mut self) {
        let Some(i) = self.rev_sel else { return };
        if let Some(r) = self
            .entries
            .get(self.selected)
            .and_then(|e| e.info.revisions.get(i))
        {
            self.editor = Some((Some(i), RevisionDialog::edit(r.clone())));
        }
    }

    /// Delete the selected revision.
    pub fn delete_revision(&mut self) {
        let Some(i) = self.rev_sel else { return };
        if let Some(e) = self.entries.get_mut(self.selected) {
            if i < e.info.revisions.len() {
                e.info.revisions.remove(i);
                self.rev_sel = None;
            }
        }
    }

    /// Move the selected revision up (`-1`) or down (`1`).
    pub fn move_revision(&mut self, delta: i32) {
        let Some(i) = self.rev_sel else { return };
        let Some(e) = self.entries.get_mut(self.selected) else {
            return;
        };
        let j = i as i64 + i64::from(delta);
        if j >= 0 && (j as usize) < e.info.revisions.len() && i < e.info.revisions.len() {
            e.info.revisions.swap(i, j as usize);
            self.rev_sel = Some(j as usize);
        }
    }

    /// Takes the revision the Revision Specification produced.
    fn finish_revision(&mut self, which: Option<usize>, rev: PageRevision) {
        if let Some(e) = self.entries.get_mut(self.selected) {
            match which {
                Some(i) if i < e.info.revisions.len() => e.info.revisions[i] = rev,
                _ => {
                    e.info.revisions.push(rev);
                    self.rev_sel = Some(e.info.revisions.len() - 1);
                }
            }
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        if let Some((which, dialog)) = &mut self.editor {
            let which = *which;
            match dialog.show(ctx) {
                Outcome::Ok => {
                    let rev = dialog.revision();
                    self.editor = None;
                    self.finish_revision(which, rev);
                }
                Outcome::Cancel => self.editor = None,
                Outcome::Open => {}
            }
            return Outcome::Open;
        }
        let error = self.error();
        let labels = self.labels();
        let assigned = self.is_assigned(self.selected);
        let templates: Vec<(u32, String)> = self
            .entries
            .iter()
            .filter(|e| e.info.template_page)
            .map(|e| (e.number, e.info.title.clone()))
            .collect();
        let names: Vec<String> = self
            .entries
            .iter()
            .zip(&labels)
            .map(|(e, l)| format!("{}  {}", l, e.info.title))
            .collect();
        let mut selected = self.selected;
        let mut action = RevAction::None;
        let outcome = {
            let Self {
                entries,
                choices,
                layout_sheet,
                rev_sel,
                selected: sel_field,
                ..
            } = self;
            frame(ctx, "Layout Page Information", 520.0, error.as_deref(), |ui| {
                if let Some(e) = entries.get_mut(*sel_field) {
                    body(
                        ui,
                        e,
                        BodyCtx {
                            names: &names,
                            templates: &templates,
                            assigned,
                            choices,
                            layout_sheet,
                        },
                        &mut selected,
                        rev_sel,
                        &mut action,
                    );
                }
            })
        };
        if selected != self.selected {
            self.select(selected);
        }
        match action {
            RevAction::None => {}
            RevAction::New => self.new_revision(),
            RevAction::Edit => self.edit_revision(),
            RevAction::Delete => self.delete_revision(),
            RevAction::Move(d) => self.move_revision(d),
        }
        outcome
    }
}

enum RevAction {
    None,
    New,
    Edit,
    Delete,
    Move(i32),
}

struct BodyCtx<'a> {
    names: &'a [String],
    templates: &'a [(u32, String)],
    assigned: bool,
    choices: &'a [SheetChoice],
    layout_sheet: &'a str,
}

fn body(
    ui: &mut Ui,
    e: &mut PageEntry,
    c: BodyCtx<'_>,
    selected: &mut usize,
    rev_sel: &mut Option<usize>,
    action: &mut RevAction,
) {
    section(ui, "Page Information");
    row(ui, "Selected Page", |ui| {
        egui::ComboBox::from_id_salt("page_info_selected")
            .width(280.0)
            .selected_text(c.names.get(*selected).cloned().unwrap_or_default())
            .show_ui(ui, |ui| {
                for (i, n) in c.names.iter().enumerate() {
                    ui.selectable_value(selected, i, n);
                }
            });
    });
    let info = &mut e.info;
    row(ui, "Label", |ui| {
        ui.add(egui::TextEdit::singleline(&mut info.label).desired_width(160.0))
            .on_hover_text("A # becomes the next number among the pages with the same label, as in A-# or A0.#");
    });
    row(ui, "Title", |ui| {
        ui.add(egui::TextEdit::singleline(&mut info.title).desired_width(300.0));
    });
    row(ui, "Description", |ui| {
        ui.add(egui::TextEdit::singleline(&mut info.description).desired_width(300.0));
    });
    row(ui, "Comments", |ui| {
        ui.add(
            egui::TextEdit::multiline(&mut info.comments)
                .desired_rows(2)
                .desired_width(300.0),
        );
    });
    ui.add_enabled_ui(!info.template_page, |ui| {
        let mut on = info.in_layout_table && !info.template_page;
        if ui.checkbox(&mut on, "Include in Layout Table").changed() {
            info.in_layout_table = on;
        }
    });

    section(ui, "Page Template Options");
    ui.add_enabled_ui(!c.assigned, |ui| {
        let r = ui.checkbox(
            &mut info.template_page,
            "Use as Page Template (not printed; its content repeats on the pages that use it)",
        );
        if r.changed() && info.template_page {
            info.in_layout_table = false;
            info.template = None;
        }
    });
    if c.assigned {
        ui.weak("Other pages use this page template: assign them another first.");
    }
    ui.add_enabled_ui(!info.template_page && !c.templates.is_empty(), |ui| {
        row(ui, "Assign Page Template", |ui| {
            let shown = match info.template {
                None => c
                    .templates
                    .first()
                    .map_or("(none)".to_string(), |t| format!("Default ({})", t.1)),
                Some(n) => c
                    .templates
                    .iter()
                    .find(|t| t.0 == n)
                    .map_or_else(|| "(none)".to_string(), |t| t.1.clone()),
            };
            egui::ComboBox::from_id_salt("page_info_template")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    if let Some(first) = c.templates.first() {
                        ui.selectable_value(
                            &mut info.template,
                            None,
                            format!("Default ({})", first.1),
                        );
                    }
                    for (n, name) in c.templates {
                        ui.selectable_value(&mut info.template, Some(*n), name);
                    }
                });
        });
    });

    section(ui, "Page Revisions");
    egui::ScrollArea::vertical()
        .id_salt("page_info_revisions")
        .max_height(110.0)
        .show(ui, |ui| {
            egui::Grid::new("page_info_revision_grid")
                .striped(true)
                .show(ui, |ui| {
                    for h in ["Label", "Date", "Revised By", "Description", "In Table"] {
                        ui.strong(h);
                    }
                    ui.end_row();
                    for (i, r) in info.revisions.iter().enumerate() {
                        ui.selectable_value(rev_sel, Some(i), &r.label);
                        ui.label(&r.date);
                        ui.label(&r.revised_by);
                        ui.label(&r.description);
                        ui.label(if r.include { "Yes" } else { "No" });
                        ui.end_row();
                    }
                });
        });
    ui.horizontal(|ui| {
        if ui.button("New").clicked() {
            *action = RevAction::New;
        }
        let has = rev_sel.is_some_and(|i| i < info.revisions.len());
        ui.add_enabled_ui(has, |ui| {
            if ui.button("Edit").clicked() {
                *action = RevAction::Edit;
            }
            if ui.button("Delete").clicked() {
                *action = RevAction::Delete;
            }
            if ui.button("Move Up").clicked() {
                *action = RevAction::Move(-1);
            }
            if ui.button("Move Down").clicked() {
                *action = RevAction::Move(1);
            }
        });
    });

    section(ui, "Sheet");
    let sheet = &mut e.sheet;
    row(ui, "Sheet size", |ui| {
        let shown = match &sheet.sheet {
            None => format!("Same as the layout ({})", c.layout_sheet),
            Some(s) => s.label(),
        };
        egui::ComboBox::from_id_salt("page_info_sheet")
            .selected_text(shown)
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut sheet.sheet,
                    None,
                    format!("Same as the layout ({})", c.layout_sheet),
                );
                for s in c.choices {
                    ui.selectable_value(&mut sheet.sheet, Some(s.clone()), s.label());
                }
            });
    });
    ui.add_enabled_ui(sheet.sheet.is_some(), |ui| {
        row(ui, "Orientation", |ui| {
            ui.radio_value(&mut sheet.portrait, false, "Landscape");
            ui.radio_value(&mut sheet.portrait, true, "Portrait");
        });
    });
    ui.checkbox(
        &mut sheet.no_title_block,
        "No border or title block on this page",
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::Point;

    fn layout() -> Layout {
        let mut l = Layout::new("t", SheetSize::ArchC);
        l.add_page(0, "Standard Template").template_page = true;
        for (n, t) in [(1, "Cover"), (2, "Plans"), (3, "Elevations")] {
            let p = l.add_page(n, t);
            p.label = "A-#".into();
            p.add_line(Point::new(1.0, 1.0), Point::new(2.0, 1.0));
        }
        l
    }

    #[test]
    fn it_starts_on_the_chosen_page_with_its_information() {
        let l = layout();
        let d = PageInfoDialog::new(&l, 2, "2026-10-08", "DAD");
        assert_eq!(d.selected(), 2);
        assert_eq!(d.entries().len(), 4);
        assert_eq!(d.entries()[2].info.title, "Plans");
        assert_eq!(d.labels(), ["A-0", "A-1", "A-2", "A-3"]);
        assert_eq!(d.error(), None);
        assert!(d.is_assigned(0), "the default template is used by the pages");
    }

    #[test]
    fn editing_two_pages_applies_in_one_go() {
        let mut l = layout();
        let mut d = PageInfoDialog::new(&l, 1, "2026-10-08", "DAD");
        d.info_mut().unwrap().label = "A0.#".into();
        d.select(2);
        d.info_mut().unwrap().label = "A0.#".into();
        d.info_mut().unwrap().description = "Main floor".into();
        assert_eq!(l.set_page_infos(&d.infos()), Ok(true));
        assert_eq!(l.page_labels(), ["A-0", "A0.1", "A0.2", "A-1"]);
        assert_eq!(l.pages[2].description, "Main floor");
    }

    #[test]
    fn a_used_template_cannot_be_dropped_and_the_dialog_says_why() {
        let l = layout();
        let mut d = PageInfoDialog::new(&l, 0, "d", "b");
        d.info_mut().unwrap().template_page = false;
        let e = d.error().expect("refused");
        assert!(e.contains("page template of other pages"), "{e}");
    }

    #[test]
    fn revisions_are_added_edited_moved_and_deleted() {
        let mut d = PageInfoDialog::new(&layout(), 1, "2026-10-08", "DAD");
        d.new_revision();
        let (_, dialog) = d.editor.take().expect("the Revision Specification opens");
        let r = dialog.revision();
        assert_eq!((r.date.as_str(), r.revised_by.as_str()), ("2026-10-08", "DAD"));
        d.finish_revision(None, PageRevision::new("1", "2026-10-08", "DAD", "First"));
        d.finish_revision(None, PageRevision::new("2", "2026-10-09", "DAD", "Second"));
        assert_eq!(d.rev_sel, Some(1));
        d.move_revision(-1);
        let labels: Vec<&str> = d.entries()[1]
            .info
            .revisions
            .iter()
            .map(|r| r.label.as_str())
            .collect();
        assert_eq!(labels, ["2", "1"]);
        d.edit_revision();
        assert!(d.editor.is_some());
        d.finish_revision(
            Some(0),
            PageRevision::new("2", "2026-10-09", "DAD", "Second, revised"),
        );
        assert_eq!(d.entries()[1].info.revisions[0].description, "Second, revised");
        d.delete_revision();
        assert_eq!(d.entries()[1].info.revisions.len(), 1);
    }

    #[test]
    fn the_dialog_draws_with_and_without_the_revision_editor() {
        let ctx = egui::Context::default();
        let mut d = PageInfoDialog::new(&layout(), 1, "2026-10-08", "DAD");
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            assert_eq!(d.show(ctx), Outcome::Open);
        });
        d.new_revision();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            assert_eq!(d.show(ctx), Outcome::Open);
        });
    }
}
