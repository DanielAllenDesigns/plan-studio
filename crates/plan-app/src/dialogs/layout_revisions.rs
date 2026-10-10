//! Revision Specification (manual p. 1421): one revision of one or more
//! layout pages. Tools > Layout > Add Layout Revision opens it with the list
//! of Revised Pages; the New and Edit buttons of Layout Page Information open
//! it for the page being edited (no page list then).

use super::layout::{frame, PageList};
use super::{row, section, Outcome};
use eframe::egui::{self, Ui};
use plan_layout::PageRevision;

/// The Revision Specification dialog.
pub struct RevisionDialog {
    rev: PageRevision,
    /// Revised Pages: the pages to choose from and which are chosen. `None`
    /// when the revision belongs to the page being edited.
    pages: Option<(PageList, Vec<u32>)>,
    title: &'static str,
}

impl RevisionDialog {
    /// Add Layout Revision: a new revision for the pages of `pages` (number,
    /// name) ticked in `chosen`; the date is today's and Revised By the
    /// designer.
    pub fn add(pages: PageList, chosen: Vec<u32>, today: &str, designer: &str) -> Self {
        Self {
            rev: PageRevision::new("", today, designer, ""),
            pages: Some((pages, chosen)),
            title: "Revision Specification",
        }
    }

    /// A new revision of the page being edited in Page Information.
    pub fn new_for_page(today: &str, designer: &str) -> Self {
        Self {
            rev: PageRevision::new("", today, designer, ""),
            pages: None,
            title: "Revision Specification",
        }
    }

    /// Edit one revision of the page being edited.
    pub fn edit(rev: PageRevision) -> Self {
        Self {
            rev,
            pages: None,
            title: "Revision Specification",
        }
    }

    /// The revision as filled in.
    pub fn revision(&self) -> PageRevision {
        PageRevision::new(
            &self.rev.label,
            &self.rev.date,
            &self.rev.revised_by,
            &self.rev.description,
        )
        .with_include(self.rev.include)
    }

    /// The page numbers ticked under Revised Pages (empty without a list).
    pub fn revised_pages(&self) -> Vec<u32> {
        self.pages
            .as_ref()
            .map(|(_, c)| c.clone())
            .unwrap_or_default()
    }

    fn error(&self) -> Option<&'static str> {
        if self.rev.label.trim().is_empty() {
            Some("Give the revision a label")
        } else if self.pages.as_ref().is_some_and(|(_, c)| c.is_empty()) {
            Some("Choose the pages this revision applies to")
        } else {
            None
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let error = self.error();
        let title = self.title;
        let Self { rev, pages, .. } = self;
        frame(ctx, title, 420.0, error, |ui| body(ui, rev, pages))
    }
}

fn body(ui: &mut Ui, rev: &mut PageRevision, pages: &mut Option<(PageList, Vec<u32>)>) {
    if let Some((list, chosen)) = pages {
        section(ui, "Revised Pages");
        egui::ScrollArea::vertical()
            .id_salt("revision_pages")
            .max_height(140.0)
            .show(ui, |ui| {
                for (number, name) in list.iter() {
                    let mut on = chosen.contains(number);
                    if ui.checkbox(&mut on, name).changed() {
                        if on {
                            chosen.push(*number);
                        } else {
                            chosen.retain(|n| n != number);
                        }
                    }
                }
            });
    }
    section(ui, "Revision Information");
    row(ui, "Label", |ui| {
        ui.add(egui::TextEdit::singleline(&mut rev.label).desired_width(120.0));
    });
    row(ui, "Date", |ui| {
        ui.add(egui::TextEdit::singleline(&mut rev.date).desired_width(120.0));
    });
    row(ui, "Revised By", |ui| {
        ui.add(egui::TextEdit::singleline(&mut rev.revised_by).desired_width(220.0));
    });
    row(ui, "Description", |ui| {
        ui.add(
            egui::TextEdit::multiline(&mut rev.description)
                .desired_rows(3)
                .desired_width(260.0),
        );
    });
    ui.checkbox(&mut rev.include, "Include in Revision Table");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_revision_starts_with_todays_date_and_the_designer() {
        let d = RevisionDialog::add(
            vec![(1, "A-1  Plan".into()), (2, "A-2  Elevations".into())],
            vec![2],
            "2026-10-08",
            "Daniel Sievers",
        );
        let r = d.revision();
        assert_eq!(
            (r.date.as_str(), r.revised_by.as_str(), r.include),
            ("2026-10-08", "Daniel Sievers", true)
        );
        assert_eq!(d.revised_pages(), [2]);
        // No label yet: refused.
        assert!(d.error().is_some());
    }

    #[test]
    fn a_revision_needs_a_label_and_a_page() {
        let mut d = RevisionDialog::add(vec![(1, "A-1".into())], vec![], "d", "b");
        d.rev.label = "1".into();
        assert_eq!(d.error(), Some("Choose the pages this revision applies to"));
        if let Some((_, c)) = &mut d.pages {
            c.push(1);
        }
        assert_eq!(d.error(), None);
        let e = RevisionDialog::edit(PageRevision::new("B", "d", "b", "x"));
        assert_eq!(e.error(), None);
        assert!(e.revised_pages().is_empty());
    }

    #[test]
    fn the_dialog_draws() {
        let ctx = egui::Context::default();
        let mut d = RevisionDialog::new_for_page("2026-10-08", "DAD");
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            assert_eq!(d.show(ctx), Outcome::Open);
        });
    }
}
