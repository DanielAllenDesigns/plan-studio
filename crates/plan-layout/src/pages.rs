//! Layout page management (manual p. 1416 to 1422): the Page Information a
//! page carries, `#` label numbering, page numbers for macros, page
//! templates assigned per page, page revisions, and the rows of the Layout
//! Page Table and Layout Revision Table.
//!
//! A page keeps its `number`: Chief's absolute page number, which is its
//! position and its key. The `label` is the text the sheet shows; it never
//! has to be unique.

use crate::model::{Layout, LayoutPage};
use serde::{Deserialize, Serialize};

fn yes() -> bool {
    true
}

/// One revision of a page (Page Information > Page Revisions).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageRevision {
    pub label: String,
    /// The date as typed (the dialog fills in today's).
    pub date: String,
    pub revised_by: String,
    pub description: String,
    /// Include in Revision Table.
    #[serde(default = "yes")]
    pub include: bool,
}

impl PageRevision {
    pub fn new(label: &str, date: &str, revised_by: &str, description: &str) -> Self {
        Self {
            label: label.trim().to_string(),
            date: date.trim().to_string(),
            revised_by: revised_by.trim().to_string(),
            description: description.trim().to_string(),
            include: true,
        }
    }

    /// The same revision with Include in Revision Table set to `include`.
    pub fn with_include(mut self, include: bool) -> Self {
        self.include = include;
        self
    }
}

/// The numbers a page's macros show, fixed for one print
/// ([`Layout::bake_numbering`]) or worked out from the layout
/// ([`Layout::page_numbers`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PageNumbers {
    /// `%layout.label%`: the label with its `#` numbered.
    pub label: String,
    /// `%page%`: the absolute layout page number.
    pub page: u32,
    /// `%page.print%`: the printed page number (blank pages not counted).
    pub print: u32,
    /// `%numpages%`: how many pages print.
    pub num_pages: u32,
    /// `%lastpage%`: the layout page number of the last printed page.
    pub last_page: u32,
}

/// Everything the Page Information dialog edits on one page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageInfo {
    pub title: String,
    pub label: String,
    pub description: String,
    pub comments: String,
    pub in_layout_table: bool,
    /// Use as Page Template.
    pub template_page: bool,
    /// Assign Page Template: the number of a template page (`None` is the
    /// default template).
    pub template: Option<u32>,
    pub revisions: Vec<PageRevision>,
}

impl PageInfo {
    /// The information of `page`.
    pub fn of(page: &LayoutPage) -> Self {
        Self {
            title: page.title.clone(),
            label: page.label.clone(),
            description: page.description.clone(),
            comments: page.comments.clone(),
            in_layout_table: page.in_layout_table,
            template_page: page.template_page,
            template: page.template,
            revisions: page.revisions.clone(),
        }
    }
}

/// The labels of pages given as `(label, is a template, number)` in page
/// order. A label with `#` takes the next number (from 1) among the pages
/// that have the identical label (templates count apart from the pages, as a
/// template's label does not pass to them); a fixed label stays as typed; an
/// empty one is `A-{number}`.
pub fn resolve_labels<'a>(pages: impl IntoIterator<Item = (&'a str, bool, u32)>) -> Vec<String> {
    let mut counts: Vec<((String, bool), u32)> = Vec::new();
    pages
        .into_iter()
        .map(|(label, template, number)| {
            let label = label.trim();
            if label.is_empty() {
                return format!("A-{number}");
            }
            if !label.contains('#') {
                return label.to_string();
            }
            let key = (label.to_string(), template);
            let n = match counts.iter_mut().find(|(k, _)| *k == key) {
                Some((_, c)) => {
                    *c += 1;
                    *c
                }
                None => {
                    counts.push((key, 1));
                    1
                }
            };
            label.replace('#', &n.to_string())
        })
        .collect()
}

impl Layout {
    /// The label of every page, index for index with [`pages`](Self::pages).
    pub fn page_labels(&self) -> Vec<String> {
        resolve_labels(
            self.pages
                .iter()
                .map(|p| (p.label.as_str(), p.template_page, p.number)),
        )
    }

    /// The sheet number `page` shows: its frozen label while a range prints,
    /// else the label with its `#` numbered among this layout's pages.
    pub fn sheet_number_of(&self, page: &LayoutPage) -> String {
        if let Some(b) = &page.baked {
            return b.label.clone();
        }
        match self.pages.iter().position(|p| p.number == page.number) {
            Some(i) => self.page_labels().swap_remove(i),
            None => page.sheet_number(),
        }
    }

    /// The numbers page `index`'s macros show.
    pub fn page_numbers(&self, index: usize) -> Option<PageNumbers> {
        let page = self.pages.get(index)?;
        if let Some(b) = &page.baked {
            return Some(b.clone());
        }
        let printed: Vec<&LayoutPage> = self
            .pages
            .iter()
            .filter(|p| !p.template_page && p.has_data())
            .collect();
        let print = if page.template_page {
            0
        } else {
            1 + self.pages[..index]
                .iter()
                .filter(|p| !p.template_page && p.has_data())
                .count() as u32
        };
        Some(PageNumbers {
            label: self.page_labels().swap_remove(index),
            page: page.number,
            print,
            num_pages: printed.len() as u32,
            last_page: printed.last().map_or(0, |p| p.number),
        })
    }

    /// Freezes every page's label and numbers, so a copy with pages taken
    /// out (a print range) still shows the numbers of the whole layout.
    pub fn bake_numbering(&mut self) {
        let numbers: Vec<Option<PageNumbers>> = (0..self.pages.len())
            .map(|i| self.page_numbers(i))
            .collect();
        for (p, n) in self.pages.iter_mut().zip(numbers) {
            p.baked = n;
        }
    }

    /// Content pages are numbered consecutively in page order, from the lowest
    /// number any of them has (so moving the cover, sheet 0, away from the
    /// front does not shift the whole set up by one); template pages keep
    /// their number. Every `#` label follows the new order by itself.
    pub fn renumber_pages(&mut self) {
        let Some(start) = self
            .pages
            .iter()
            .filter(|p| !p.template_page)
            .map(|p| p.number)
            .min()
        else {
            return;
        };
        for (n, p) in (start..).zip(self.pages.iter_mut().filter(|p| !p.template_page)) {
            p.number = n;
        }
        self.fix_template_refs();
    }

    /// Drags the page at `from` to position `to` (its index after the move),
    /// like dragging it in the Project Browser. Returns where it went.
    pub fn move_page(&mut self, from: usize, to: usize) -> Option<usize> {
        if from >= self.pages.len() {
            return None;
        }
        let to = to.min(self.pages.len() - 1);
        // Template numbers are the keys of assignments: remember who used
        // which template page before numbers change.
        let page = self.pages.remove(from);
        self.pages.insert(to, page);
        self.renumber_pages();
        Some(to)
    }

    /// Template pages keep their numbers when pages are renumbered, so the
    /// assignments (by number) stay valid; this drops one that points at a
    /// page that is not a template any more.
    fn fix_template_refs(&mut self) {
        let templates: Vec<u32> = self
            .pages
            .iter()
            .filter(|p| p.template_page)
            .map(|p| p.number)
            .collect();
        for p in &mut self.pages {
            if p.template.is_some_and(|n| !templates.contains(&n)) {
                p.template = None;
            }
        }
    }

    /// The template page that shows on page `index`: the one assigned to it,
    /// else the default (the first template page). Template pages have none.
    pub fn template_index_for(&self, index: usize) -> Option<usize> {
        let page = self.pages.get(index)?;
        if page.template_page {
            return None;
        }
        let assigned = page.template.and_then(|n| {
            self.pages
                .iter()
                .position(|p| p.template_page && p.number == n)
        });
        assigned.or_else(|| self.pages.iter().position(|p| p.template_page))
    }

    /// The template pages whose contents repeat on page `index` (none or
    /// one).
    pub fn templates_for(&self, index: usize) -> Vec<&LayoutPage> {
        self.template_index_for(index)
            .map(|t| &self.pages[t])
            .into_iter()
            .collect()
    }

    /// The template pages to pick from: `(number, name)`, the name being the
    /// page's title (its label when untitled).
    pub fn template_choices(&self) -> Vec<(u32, String)> {
        self.pages
            .iter()
            .filter(|p| p.template_page)
            .map(|p| {
                let name = if p.title.trim().is_empty() {
                    p.sheet_number()
                } else {
                    p.title.clone()
                };
                (p.number, name)
            })
            .collect()
    }

    /// How many other pages use the template page at `index`.
    pub fn assigned_count(&self, index: usize) -> usize {
        if !self.pages.get(index).is_some_and(|p| p.template_page) {
            return 0;
        }
        (0..self.pages.len())
            .filter(|&i| i != index && self.template_index_for(i) == Some(index))
            .count()
    }

    /// Does page `index` print without the layout's border and title block:
    /// its own switch, or the one of the template it uses (a cover template
    /// has none).
    pub fn effective_no_title_block(&self, index: usize) -> bool {
        let Some(page) = self.pages.get(index) else {
            return false;
        };
        page.no_title_block
            || self
                .template_index_for(index)
                .is_some_and(|t| self.pages[t].no_title_block)
    }

    /// Why page `index` cannot be deleted, if it cannot: a page template that
    /// other pages use, and page zero when it is the template.
    pub fn delete_blocker(&self, index: usize) -> Option<&'static str> {
        let page = self.pages.get(index)?;
        if !page.template_page {
            return None;
        }
        if page.number == 0 {
            Some("Layout page zero is the default page template and cannot be deleted")
        } else if self.assigned_count(index) > 0 {
            Some("A page template that other pages use cannot be deleted")
        } else {
            None
        }
    }

    /// Applies the Page Information of several pages at once, `(page number,
    /// information)`. Nothing changes when one is refused: a template that
    /// pages still use cannot stop being a template.
    pub fn set_page_infos(&mut self, infos: &[(u32, PageInfo)]) -> Result<bool, String> {
        let mut next = self.clone();
        for (number, info) in infos {
            let Some(p) = next.pages.iter_mut().find(|p| p.number == *number) else {
                return Err("There is no such page".to_string());
            };
            p.title = info.title.trim().to_string();
            p.label = info.label.trim().to_string();
            p.description = info.description.clone();
            p.comments = info.comments.clone();
            p.template_page = info.template_page;
            p.in_layout_table = info.in_layout_table && !info.template_page;
            p.template = if info.template_page {
                None
            } else {
                info.template
            };
            p.revisions = info.revisions.clone();
        }
        // A template other pages are assigned to cannot stop being one.
        for (i, was) in self.pages.iter().enumerate() {
            if !was.template_page || next.pages[i].template_page {
                continue;
            }
            let explicit = next
                .pages
                .iter()
                .any(|p| !p.template_page && p.template == Some(was.number));
            let last = !next.pages.iter().any(|p| p.template_page) && self.assigned_count(i) > 0;
            if explicit || last {
                return Err(format!(
                    "\"{}\" is the page template of other pages: assign them another first",
                    was.title
                ));
            }
        }
        next.fix_template_refs();
        let changed = next != *self;
        *self = next;
        Ok(changed)
    }

    /// Adds `revision` to every page in `numbers` (Add Layout Revision).
    /// Returns how many pages got it.
    pub fn add_revision(&mut self, numbers: &[u32], revision: &PageRevision) -> usize {
        let mut n = 0;
        for p in &mut self.pages {
            if numbers.contains(&p.number) {
                p.revisions.push(revision.clone());
                n += 1;
            }
        }
        n
    }

    /// Copies the border and drawings of page `from` (its CAD, leaders and
    /// revision clouds) onto page `to`, as new objects. Returns how many.
    pub fn copy_page_drawings(&mut self, from: usize, to: usize) -> usize {
        if from == to || from >= self.pages.len() || to >= self.pages.len() {
            return 0;
        }
        let src = self.pages[from].clone();
        let dst = &mut self.pages[to];
        let mut n = 0;
        for o in &src.cad {
            let mut o = o.clone();
            o.id = dst.next_cad_id();
            dst.cad.push(o);
            n += 1;
        }
        for l in &src.leaders {
            let mut l = l.clone();
            l.id = dst.next_cad_id();
            dst.leaders.push(l);
            n += 1;
        }
        for c in &src.clouds {
            let mut c = c.clone();
            c.id = dst.next_cad_id();
            dst.clouds.push(c);
            n += 1;
        }
        n
    }

    /// The rows of a Layout Page Table: label, title and description of every
    /// page that is listed (Include in Layout Table) and has data on it.
    pub fn page_table_rows(&self) -> Vec<Vec<String>> {
        let labels = self.page_labels();
        self.pages
            .iter()
            .zip(labels)
            .filter(|(p, _)| !p.template_page && p.in_layout_table && p.has_data())
            .map(|(p, label)| vec![label, p.title.clone(), p.description.clone()])
            .collect()
    }

    /// The rows of the Layout Revision Table of the page numbered `number`:
    /// label, date, revised by and description of its revisions that are
    /// included, in the order they are kept.
    pub fn revision_table_rows(&self, number: u32) -> Vec<Vec<String>> {
        revision_rows(
            self.page(number)
                .map(|p| p.revisions.as_slice())
                .unwrap_or(&[]),
        )
    }
}

/// Table rows of `revisions` that are included in the table.
pub fn revision_rows(revisions: &[PageRevision]) -> Vec<Vec<String>> {
    revisions
        .iter()
        .filter(|r| r.include)
        .map(|r| {
            vec![
                r.label.clone(),
                r.date.clone(),
                r.revised_by.clone(),
                r.description.clone(),
            ]
        })
        .collect()
}

/// Column heads of the Layout Page Table.
pub const PAGE_TABLE_COLUMNS: [&str; 3] = ["LABEL", "TITLE", "DESCRIPTION"];
/// Column heads of the Layout Revision Table.
pub const REVISION_TABLE_COLUMNS: [&str; 4] = ["REV", "DATE", "BY", "DESCRIPTION"];

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::Point;
    use plan_docs::SheetSize;

    fn labelled(labels: &[&str]) -> Layout {
        let mut l = Layout::new("t", SheetSize::ArchC);
        for (i, label) in labels.iter().enumerate() {
            let p = l.add_page(i as u32 + 1, format!("Page {}", i + 1));
            p.label = (*label).to_string();
        }
        l
    }

    #[test]
    fn hash_takes_the_next_free_number_among_pages_with_the_same_pattern() {
        let l = labelled(&["A0.#", "A1.#", "A1.#", "", "A0.#", "E1.#", "Cover", "A1.#"]);
        assert_eq!(
            l.page_labels(),
            ["A0.1", "A1.1", "A1.2", "A-4", "A0.2", "E1.1", "Cover", "A1.3"]
        );
        // A fixed label is allowed, even twice (duplicates are legal).
        let d = labelled(&["Cover", "Cover", "A-#", "A-#"]);
        assert_eq!(d.page_labels(), ["Cover", "Cover", "A-1", "A-2"]);
    }

    #[test]
    fn a_templates_label_does_not_number_the_pages() {
        let mut l = labelled(&["A-#", "A-#", "A-#"]);
        l.pages[0].template_page = true;
        assert_eq!(l.page_labels(), ["A-1", "A-1", "A-2"]);
    }

    #[test]
    fn dragging_a_page_renumbers_every_hash_label() {
        let mut l = labelled(&["A-#", "A-#", "A-#", "S-#"]);
        for (i, t) in ["Plan", "Elevations", "Sections", "Notes"]
            .iter()
            .enumerate()
        {
            l.pages[i].title = (*t).to_string();
        }
        assert_eq!(l.page_labels(), ["A-1", "A-2", "A-3", "S-1"]);
        assert_eq!(l.move_page(2, 0), Some(0));
        let titles: Vec<&str> = l.pages.iter().map(|p| p.title.as_str()).collect();
        assert_eq!(titles, ["Sections", "Plan", "Elevations", "Notes"]);
        assert_eq!(l.page_labels(), ["A-1", "A-2", "A-3", "S-1"]);
        // The page numbers follow the position too.
        let numbers: Vec<u32> = l.pages.iter().map(|p| p.number).collect();
        assert_eq!(numbers, [1, 2, 3, 4]);
        assert_eq!(l.sheet_number_of(&l.pages[0].clone()), "A-1");
        assert_eq!(l.move_page(9, 0), None);
    }

    #[test]
    fn page_numbers_skip_blank_pages() {
        let mut l = labelled(&["A-#", "A-#", "A-#", "A-#"]);
        l.pages[0].add_line(Point::new(1.0, 1.0), Point::new(2.0, 1.0));
        l.pages[2].add_line(Point::new(1.0, 1.0), Point::new(2.0, 1.0));
        l.pages[3].add_line(Point::new(1.0, 1.0), Point::new(2.0, 1.0));
        let n = |i| l.page_numbers(i).unwrap();
        assert_eq!((n(0).page, n(0).print), (1, 1));
        assert_eq!((n(2).page, n(2).print), (3, 2));
        assert_eq!((n(3).page, n(3).print), (4, 3));
        assert_eq!((n(2).num_pages, n(2).last_page), (3, 4));
        assert_eq!(n(2).label, "A-3");
        // Freezing keeps them when pages are taken out.
        let mut b = l.clone();
        b.bake_numbering();
        b.pages.retain(|p| p.number >= 3);
        assert_eq!(b.page_numbers(0).unwrap().print, 2);
        assert_eq!(b.sheet_number_of(&b.pages[0]), "A-3");
    }

    #[test]
    fn templates_are_assigned_per_page_and_guarded() {
        let mut l = Layout::new("t", SheetSize::ArchC);
        l.add_page(0, "Standard Template").template_page = true;
        l.add_page(1, "Cover Template").template_page = true;
        l.pages[1].no_title_block = true;
        l.add_page(2, "Cover");
        l.add_page(3, "Plan");
        l.pages[0].add_line(Point::new(0.0, 0.0), Point::new(1.0, 0.0));
        // The default template is the first one; the cover picks the second.
        assert_eq!(l.template_index_for(3), Some(0));
        l.pages[2].template = Some(1);
        assert_eq!(l.template_index_for(2), Some(1));
        assert_eq!(l.template_index_for(0), None, "a template has none");
        assert!(l.effective_no_title_block(2));
        assert!(!l.effective_no_title_block(3));
        assert_eq!(l.assigned_count(0), 1);
        assert_eq!(l.assigned_count(1), 1);
        assert_eq!(l.templates_for(2)[0].title, "Cover Template");
        // A template that is assigned cannot be deleted; page zero never.
        assert!(l.delete_blocker(1).is_some());
        assert!(l.delete_blocker(0).unwrap().contains("zero"));
        assert_eq!(l.delete_blocker(3), None);
        l.pages[2].template = None;
        assert_eq!(
            l.delete_blocker(1),
            None,
            "nobody uses the cover template now"
        );
        // It cannot stop being a template while a page is assigned to it.
        l.pages[3].template = Some(1);
        let mut info = PageInfo::of(&l.pages[1]);
        info.template_page = false;
        let r = l.set_page_infos(&[(1, info)]);
        assert!(r.is_err(), "{r:?}");
        assert!(l.pages[1].template_page, "nothing changed");
    }

    #[test]
    fn page_information_round_trips_and_clears_the_table_flag_on_templates() {
        let mut l = labelled(&["A-#", "A-#"]);
        let mut info = PageInfo::of(&l.pages[0]);
        info.title = " Floor Plans ".into();
        info.description = "Main and upper".into();
        info.comments = "check scale".into();
        info.in_layout_table = true;
        info.revisions = vec![PageRevision::new("A", "2026-10-01", "DAD", "Issued")];
        assert_eq!(l.set_page_infos(&[(1, info)]), Ok(true));
        let p = &l.pages[0];
        assert_eq!(
            (
                p.title.as_str(),
                p.description.as_str(),
                p.comments.as_str()
            ),
            ("Floor Plans", "Main and upper", "check scale")
        );
        assert_eq!(p.revisions.len(), 1);
        let mut t = PageInfo::of(&l.pages[1]);
        t.template_page = true;
        t.in_layout_table = true;
        t.template = Some(1);
        assert_eq!(l.set_page_infos(&[(2, t)]), Ok(true));
        assert!(!l.pages[1].in_layout_table && l.pages[1].template.is_none());
        // Applying the same thing again changes nothing.
        let same = PageInfo::of(&l.pages[0]);
        assert_eq!(l.set_page_infos(&[(1, same)]), Ok(false));
        let stray = PageInfo::of(&l.pages[0]);
        assert!(l.set_page_infos(&[(99, stray)]).is_err());
    }

    #[test]
    fn page_and_revision_table_rows() {
        let mut l = labelled(&["A-#", "A-#", "A-#"]);
        for i in 0..3 {
            l.pages[i].title = format!("T{}", i + 1);
            l.pages[i].add_line(Point::new(0.0, 0.0), Point::new(1.0, 0.0));
        }
        l.pages[1].description = "Elevations".into();
        l.pages[2].in_layout_table = false;
        l.add_page(9, "Blank");
        let rows = l.page_table_rows();
        assert_eq!(
            rows,
            vec![
                vec!["A-1".to_string(), "T1".into(), String::new()],
                vec!["A-2".to_string(), "T2".into(), "Elevations".into()],
            ]
        );
        let rev = PageRevision::new("1", "2026-10-02", "DAD", "Moved the stair");
        let mut hidden = PageRevision::new("2", "2026-10-03", "DAD", "Draft");
        hidden.include = false;
        assert_eq!(l.add_revision(&[1, 3, 77], &rev), 2);
        l.pages[0].revisions.push(hidden);
        assert_eq!(
            l.revision_table_rows(1),
            vec![vec![
                "1".to_string(),
                "2026-10-02".into(),
                "DAD".into(),
                "Moved the stair".into()
            ]]
        );
        assert_eq!(l.revision_table_rows(3).len(), 1);
        assert!(l.revision_table_rows(2).is_empty());
        assert!(l.revision_table_rows(55).is_empty());
    }

    #[test]
    fn copying_a_pages_drawings_makes_new_objects() {
        let mut l = labelled(&["", ""]);
        l.pages[0].add_line(Point::new(0.0, 0.0), Point::new(1.0, 0.0));
        l.pages[0].add_text(Point::new(1.0, 1.0), "TITLE", 0.2);
        l.pages[0].add_cloud(Point::new(1.0, 1.0), Point::new(3.0, 3.0), "A");
        l.pages[1].add_line(Point::new(5.0, 5.0), Point::new(6.0, 5.0));
        assert_eq!(l.copy_page_drawings(0, 1), 3);
        let ids: Vec<plan_core::Id> = l.pages[1]
            .cad
            .iter()
            .map(|o| o.id)
            .chain(l.pages[1].clouds.iter().map(|c| c.id))
            .collect();
        let mut u = ids.clone();
        u.sort_unstable();
        u.dedup();
        assert_eq!(u.len(), ids.len(), "ids are unique on the page");
        assert_eq!(l.copy_page_drawings(0, 0), 0);
        assert_eq!(l.copy_page_drawings(0, 9), 0);
    }

    #[test]
    fn older_pages_open_with_the_new_fields_defaulted() {
        let l = labelled(&["A-#"]);
        let mut v = serde_json::to_value(&l).unwrap();
        let page = &mut v["pages"][0];
        for k in [
            "label",
            "description",
            "comments",
            "in_layout_table",
            "template",
            "revisions",
        ] {
            page.as_object_mut().unwrap().remove(k);
        }
        for k in ["snap_grid", "snap_unit_in"] {
            v.as_object_mut().unwrap().remove(k);
        }
        let old: Layout = serde_json::from_value(v).unwrap();
        assert_eq!(old.pages[0].label, "");
        assert!(old.pages[0].in_layout_table && old.pages[0].revisions.is_empty());
        assert!(old.snap_grid && (old.snap_unit_in - 1.0 / 16.0).abs() < 1e-12);
        assert_eq!(old.pages[0].sheet_number(), "A-1");
        // New fields survive a round trip; the frozen numbers are not stored.
        let mut l = labelled(&["A0.#"]);
        l.pages[0].description = "d".into();
        l.pages[0]
            .revisions
            .push(PageRevision::new("1", "d", "b", "x"));
        l.bake_numbering();
        let json = serde_json::to_string(&l).unwrap();
        assert!(!json.contains("baked"));
        let mut back: Layout = serde_json::from_str(&json).unwrap();
        l.pages[0].baked = None;
        back.pages[0].baked = None;
        assert_eq!(back, l);
    }

    // ---- macros, rendering and the two tables ----

    use crate::model::{
        BoxSource, LayoutBox, DEFAULT_SNAP_UNIT_IN, MAX_SNAP_UNIT_IN, MIN_SNAP_UNIT_IN,
    };
    use crate::tests::two_room_house;
    use crate::titleblock::MacroContext;
    use crate::{render_pdf, LayoutRenderContext};
    use plan_docs::Scale;

    fn pdf_text(l: &Layout) -> String {
        let p = two_room_house();
        let cx = LayoutRenderContext::new(&p);
        render_pdf(l, &cx).iter().map(|&b| b as char).collect()
    }

    fn drawn(labels: &[&str]) -> Layout {
        let mut l = labelled(labels);
        for p in &mut l.pages {
            p.add_line(Point::new(1.0, 1.0), Point::new(2.0, 1.0));
        }
        l
    }

    #[test]
    fn page_macros_evaluate_per_page() {
        let mut l = drawn(&["A0.#", "A0.#", "A0.#"]);
        l.pages[1].title = "Elevations".into();
        l.pages[1].description = "All four sides".into();
        l.pages[1].comments = "check heights".into();
        // The middle page is blank so it does not print: the third page is
        // printed page 2 of 2 but layout page 3.
        l.pages[1].cad.clear();
        let mut ctx = MacroContext::default();
        ctx.apply_page(&l, &l.pages[2].clone());
        assert_eq!(
            ctx.expand("%layout.label% %page% %page.print% %numpages% %lastpage%"),
            "A0.3 3 2 2 3"
        );
        ctx.apply_page(&l, &l.pages[1].clone());
        assert_eq!(
            ctx.expand("%layout.title% | %layout.description% | %layout.comments%"),
            "Elevations | All four sides | check heights"
        );
        assert_eq!(ctx.sheet_number, "A0.2");
        // Without a page the macros stay as written.
        assert_eq!(MacroContext::default().expand("%page%"), "%page%");
    }

    #[test]
    fn macros_in_layout_text_and_the_title_block_reach_the_pdf() {
        let mut l = Layout::new("t", SheetSize::ArchC);
        l.add_page(0, "Standard Template").template_page = true;
        l.pages[0].add_text(
            Point::new(1.0, 1.0),
            "SHEET %layout.label% OF %lastpage%",
            0.2,
        );
        for (n, t) in [(1, "Floor Plans"), (2, "Roof Plan")] {
            let p = l.add_page(n, t);
            p.label = "A1.#".into();
            p.add_text(
                Point::new(3.0, 3.0),
                "THIS IS %layout.title% %page.print%/%numpages%",
                0.2,
            );
        }
        let t = pdf_text(&l);
        assert!(t.contains("SHEET A1.1 OF 2"), "template text on page one");
        assert!(t.contains("SHEET A1.2 OF 2"), "template text on page two");
        assert!(t.contains("THIS IS Floor Plans 1/2"));
        assert!(t.contains("THIS IS Roof Plan 2/2"));
        assert!(!t.contains("%layout.label%"));
        // Text boxes expand them too.
        let mut b = LayoutBox::new(
            1,
            (Point::new(4.0, 4.0), Point::new(9.0, 5.0)),
            BoxSource::text("PAGE %page% OF %numpages%", 10.0),
            Scale::QuarterInch,
        );
        b.border = false;
        l.pages[2].boxes.push(b);
        let t = pdf_text(&l);
        assert!(t.contains("PAGE 2 OF 2"), "text box macros");
    }

    #[test]
    fn page_revisions_feed_the_title_block_and_clouds_only_when_there_are_none() {
        let mut l = drawn(&["A-#", "A-#"]);
        l.pages[0].add_cloud(Point::new(1.0, 1.0), Point::new(3.0, 3.0), "7");
        l.pages[1].add_cloud(Point::new(1.0, 1.0), Point::new(3.0, 3.0), "9");
        l.pages[1].revisions = vec![
            PageRevision::new("1", "2026-10-01", "DAD", "Issued"),
            PageRevision::new("2", "2026-10-05", "DAD", "Stair moved").with_include(false),
        ];
        let mut ctx = MacroContext::default();
        ctx.apply_page(&l, &l.pages[0].clone());
        assert!(
            ctx.revisions.iter().any(|r| r.0 == "7"),
            "no page revisions: the cloud feeds the table {:?}",
            ctx.revisions
        );
        let mut ctx = MacroContext::default();
        ctx.apply_page(&l, &l.pages[1].clone());
        assert_eq!(ctx.revisions.len(), 1, "{:?}", ctx.revisions);
        assert_eq!(ctx.revisions[0].0, "1");
        assert_eq!(ctx.revision, "1");
    }

    #[test]
    fn the_two_tables_draw_their_rows_and_the_revision_table_follows_its_page() {
        let mut l = Layout::new("t", SheetSize::ArchC);
        l.add_page(0, "Standard Template").template_page = true;
        let rect = (Point::new(1.0, 1.0), Point::new(8.0, 5.0));
        l.pages[0].boxes.push(LayoutBox::new(
            9,
            (Point::new(1.0, 6.0), Point::new(8.0, 9.0)),
            BoxSource::RevisionTable,
            Scale::QuarterInch,
        ));
        for (n, t, d) in [(1, "Floor Plans", "Main and upper"), (2, "Roof Plan", "")] {
            let p = l.add_page(n, t);
            p.label = "A-#".into();
            p.description = d.into();
            p.add_line(Point::new(1.0, 1.0), Point::new(2.0, 1.0));
        }
        l.pages[1].revisions.push(PageRevision::new(
            "1",
            "2026-10-02",
            "DAD",
            "Moved the stair",
        ));
        l.pages[2].revisions.push(PageRevision::new(
            "B",
            "2026-10-03",
            "DAD",
            "Added a skylight",
        ));
        l.pages[2].boxes.push(LayoutBox::new(
            1,
            rect,
            BoxSource::PageTable,
            Scale::QuarterInch,
        ));
        let t = pdf_text(&l);
        assert!(
            t.contains("Floor Plans") && t.contains("Main and upper"),
            "page table"
        );
        assert!(t.contains("Roof Plan"));
        // The Revision Table sits on the template and shows each page's own rows.
        assert!(t.contains("Moved the stair"), "page one's revision");
        assert!(t.contains("Added a skylight"), "page two's revision");
        // Pages move: the table follows by itself.
        l.move_page(2, 1);
        assert_eq!(l.page_table_rows()[0][1], "Roof Plan");
        assert_eq!(l.page_table_rows()[0][0], "A-1");
    }

    #[test]
    fn layout_defaults_hold_the_snap_unit() {
        let l = Layout::new("t", SheetSize::ArchC);
        assert!(l.snap_grid);
        assert!((l.snap_unit_in - DEFAULT_SNAP_UNIT_IN).abs() < 1e-12);
        assert!(MIN_SNAP_UNIT_IN < DEFAULT_SNAP_UNIT_IN && DEFAULT_SNAP_UNIT_IN < MAX_SNAP_UNIT_IN);
    }
}
