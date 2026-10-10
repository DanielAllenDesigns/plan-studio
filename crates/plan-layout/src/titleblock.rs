//! Title block templates and text macros.

use plan_core::CadObject;
use serde::{Deserialize, Serialize};

/// Where the title block sits on the sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TitleBlockStyle {
    /// A tall strip down the right edge (Chief's presentation default).
    RightStrip,
    /// A short strip along the bottom edge.
    BottomStrip,
    /// Hand-drawn CAD in paper inches. Text items may contain macros.
    Custom(Vec<CadObject>),
}

/// The page macros of one sheet: `%layout.label%`, `%layout.title%`,
/// `%layout.description%`, `%layout.comments%`, `%page%`, `%page.print%`,
/// `%numpages%` and `%lastpage%`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageMacros {
    pub label: String,
    pub title: String,
    pub description: String,
    pub comments: String,
    /// The absolute layout page number.
    pub page: u32,
    /// The printed page number (pages without data are not counted).
    pub print: u32,
    /// How many pages print.
    pub num_pages: u32,
    /// The layout page number of the last page that prints.
    pub last_page: u32,
}

/// Values the macros expand to. The per-sheet fields (`sheet_number`,
/// `sheet_title`, `scale`, `page_count`) are filled in by [`crate::render_pdf`]
/// for each page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroContext {
    pub project_name: String,
    pub client: String,
    pub address: String,
    pub designer: String,
    pub date: String,
    pub sheet_number: String,
    pub sheet_title: String,
    pub scale: String,
    /// Job number (`%project.number%`).
    #[serde(default)]
    pub project_number: String,
    /// Current revision label (`%revision%`).
    #[serde(default)]
    pub revision: String,
    /// Number of printed pages (`%page.count%`).
    #[serde(default)]
    pub page_count: usize,
    /// Revision table rows `(number, date, description)`, oldest first.
    #[serde(default)]
    pub revisions: Vec<(String, String, String)>,
    /// More macros as `("%company%", value)`: Project Information's
    /// `%company%`, `%client.phone%`, `%client.email%`, `%drawn.by%`,
    /// `%checked.by%`, `%project.address%`, `%client.address%` and
    /// `%custom.<key>%`.
    #[serde(default)]
    pub extra: Vec<(String, String)>,
    /// The page macros of the sheet being drawn; without them
    /// `%layout.label%`, `%page%` and the rest stay as written.
    #[serde(default)]
    pub page_info: Option<PageMacros>,
}

/// `2026-10-07` (or `10/7/2026`) as `October 7, 2026`; other text is returned as is.
pub fn long_date(date: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let t = date.trim();
    let nums = |sep: char| -> Option<Vec<u32>> {
        let v: Option<Vec<u32>> = t.split(sep).map(|p| p.trim().parse().ok()).collect();
        v.filter(|v| v.len() == 3)
    };
    let (y, m, d) = if let Some(v) = nums('-').filter(|v| v[0] > 31) {
        (v[0], v[1], v[2])
    } else if let Some(v) = nums('/').filter(|v| v[2] > 31) {
        (v[2], v[0], v[1])
    } else {
        return date.to_string();
    };
    if (1..=12).contains(&m) && (1..=31).contains(&d) {
        format!("{} {d}, {y}", MONTHS[m as usize - 1])
    } else {
        date.to_string()
    }
}

impl MacroContext {
    /// Is `key` (`"%client%"`) one of the macros with a field of its own?
    pub fn is_builtin(key: &str) -> bool {
        matches!(
            key,
            "%project.name%"
                | "%project.number%"
                | "%client%"
                | "%address%"
                | "%designer%"
                | "%date.long%"
                | "%date%"
                | "%revision%"
                | "%sheet.number%"
                | "%sheet.title%"
                | "%scale%"
                | "%page.count%"
        )
    }

    /// Replace every known macro in `text`:
    /// `%project.name%`, `%project.number%`, `%client%`, `%address%`,
    /// `%designer%`, `%date%`, `%date.long%`, `%revision%`, `%sheet.number%`,
    /// `%sheet.title%`, `%scale%`, `%page.count%`, then those of
    /// [`MacroContext::extra`] (`%company%`, `%client.phone%`...). Unknown
    /// `%...%` stay as written.
    pub fn expand(&self, text: &str) -> String {
        let page_count = self.page_count.to_string();
        let date_long = long_date(&self.date);
        let builtin = [
            ("%project.name%", &self.project_name),
            ("%project.number%", &self.project_number),
            ("%client%", &self.client),
            ("%address%", &self.address),
            ("%designer%", &self.designer),
            ("%date.long%", &date_long),
            ("%date%", &self.date),
            ("%revision%", &self.revision),
            ("%sheet.number%", &self.sheet_number),
            ("%sheet.title%", &self.sheet_title),
            ("%scale%", &self.scale),
            ("%page.count%", &page_count),
        ]
        .into_iter()
        .fold(text.to_string(), |acc, (k, v)| acc.replace(k, v));
        let builtin = match &self.page_info {
            Some(p) => {
                let (page, print) = (p.page.to_string(), p.print.to_string());
                let (num, last) = (p.num_pages.to_string(), p.last_page.to_string());
                [
                    ("%layout.label%", &p.label),
                    ("%layout.title%", &p.title),
                    ("%layout.description%", &p.description),
                    ("%layout.comments%", &p.comments),
                    ("%page.print%", &print),
                    ("%page%", &page),
                    ("%numpages%", &num),
                    ("%lastpage%", &last),
                ]
                .into_iter()
                .fold(builtin, |acc, (k, v)| acc.replace(k, v))
            }
            None => builtin,
        };
        self.extra
            .iter()
            .fold(builtin, |acc, (k, v)| acc.replace(k.as_str(), v))
    }
}

impl MacroContext {
    /// Fills the per-sheet fields for `page` of `layout`: the sheet number
    /// (its label), title, the page macros, and the REVISIONS table. A page
    /// with revisions of its own lists those (the ones included in the
    /// table); a page without any takes Project Information's rows plus the
    /// revision clouds of the layout ([`add_cloud_revisions`](Self::add_cloud_revisions)).
    pub fn apply_page(&mut self, layout: &crate::model::Layout, page: &crate::model::LayoutPage) {
        let Some(index) = layout.pages.iter().position(|p| p.number == page.number) else {
            self.sheet_number = page.sheet_number();
            self.sheet_title = page.title.clone();
            return;
        };
        let nums = layout.page_numbers(index).unwrap_or_default();
        self.sheet_number = nums.label.clone();
        self.sheet_title = page.title.clone();
        self.page_count = layout.content_pages().len();
        self.page_info = Some(PageMacros {
            label: nums.label,
            title: page.title.clone(),
            description: page.description.clone(),
            comments: page.comments.clone(),
            page: nums.page,
            print: nums.print,
            num_pages: nums.num_pages,
            last_page: nums.last_page,
        });
        if page.revisions.is_empty() {
            self.add_cloud_revisions(layout);
        } else {
            self.revisions = page
                .revisions
                .iter()
                .filter(|r| r.include)
                .map(|r| (r.label.clone(), r.date.clone(), r.description.clone()))
                .collect();
            self.revision = self
                .revisions
                .last()
                .map(|r| r.0.clone())
                .unwrap_or_default();
        }
    }

    /// Feeds the REVISIONS table from the revision clouds of `layout`: every
    /// mark a cloud carries that Project Information has no row for gets one
    /// (mark, no date, "Revision cloud on A-2, A-5"), after the existing rows
    /// and in mark order (numbers before letters). With no current revision
    /// set, `%revision%` becomes the last row's mark.
    pub fn add_cloud_revisions(&mut self, layout: &crate::model::Layout) {
        let mut found: Vec<(String, Vec<String>)> = Vec::new();
        for page in layout.content_pages() {
            for c in &page.clouds {
                let mark = c.revision.trim();
                if mark.is_empty() || self.revisions.iter().any(|(n, _, _)| n.trim() == mark) {
                    continue;
                }
                let sheet = layout.sheet_number_of(page);
                match found.iter_mut().find(|(m, _)| m == mark) {
                    Some((_, sheets)) => {
                        if !sheets.contains(&sheet) {
                            sheets.push(sheet);
                        }
                    }
                    None => found.push((mark.to_string(), vec![sheet])),
                }
            }
        }
        found.sort_by(
            |(a, _), (b, _)| match (a.parse::<u32>(), b.parse::<u32>()) {
                (Ok(x), Ok(y)) => x.cmp(&y),
                (Ok(_), Err(_)) => std::cmp::Ordering::Less,
                (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
                (Err(_), Err(_)) => a.cmp(b),
            },
        );
        for (mark, sheets) in found {
            self.revisions.push((
                mark,
                String::new(),
                format!("Revision cloud on {}", sheets.join(", ")),
            ));
        }
        if self.revision.trim().is_empty() {
            if let Some((n, _, _)) = self.revisions.last() {
                self.revision = n.clone();
            }
        }
    }
}

/// Rows in the REVISIONS table of [`TitleBlockTemplate::from_daniel_18x24`].
pub const DANIEL_REVISION_ROWS: usize = 5;

/// A title block: a style and its labelled fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TitleBlockTemplate {
    pub style: TitleBlockStyle,
    /// `(label, macro text)` pairs, top to bottom (or left to right).
    pub fields: Vec<(String, String)>,
    /// Rows of the REVISIONS table at the foot of a [`TitleBlockStyle::RightStrip`]
    /// (0 = no table). Rows come from [`MacroContext::revisions`].
    #[serde(default)]
    pub revision_rows: usize,
}

impl TitleBlockTemplate {
    /// The 18x24 presentation sheet: a 2.5" right strip with PROJECT, CLIENT,
    /// ADDRESS, SHEET TITLE, SHEET NO., DATE, SCALE and DRAWN BY boxes.
    pub fn presentation_18x24() -> Self {
        let f = |l: &str, m: &str| (l.to_string(), m.to_string());
        Self {
            style: TitleBlockStyle::RightStrip,
            fields: vec![
                f("PROJECT", "%project.name%"),
                f("CLIENT", "%client%"),
                f("ADDRESS", "%address%"),
                f("SHEET TITLE", "%sheet.title%"),
                f("SHEET NO.", "%sheet.number%"),
                f("DATE", "%date%"),
                f("SCALE", "%scale%"),
                f("DRAWN BY", "%designer%"),
            ],
            revision_rows: 0,
        }
    }

    /// Daniel's 18x24 presentation sheet: the right strip with PROJECT,
    /// CLIENT, ADDRESS, SHEET TITLE, SHEET NO., DATE, SCALE and DRAWN BY
    /// boxes (in that order) above a REVISIONS table of 5 rows.
    pub fn from_daniel_18x24() -> Self {
        Self {
            revision_rows: DANIEL_REVISION_ROWS,
            ..Self::presentation_18x24()
        }
    }

    /// The fields with macros expanded: `(label, value)`.
    pub fn expand_macros(&self, ctx: &MacroContext) -> Vec<(String, String)> {
        self.fields
            .iter()
            .map(|(label, text)| (label.clone(), ctx.expand(text)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macros_expand_in_every_field() {
        let ctx = MacroContext {
            project_name: "Smith Residence".into(),
            client: "J. Smith".into(),
            address: "1 Main St".into(),
            designer: "DAD".into(),
            date: "2026-10-07".into(),
            sheet_number: "A-3".into(),
            sheet_title: "Elevations".into(),
            scale: "1/4\" = 1'-0\"".into(),
            ..MacroContext::default()
        };
        let out = TitleBlockTemplate::presentation_18x24().expand_macros(&ctx);
        assert_eq!(out.len(), 8);
        assert!(out.contains(&("SHEET NO.".to_string(), "A-3".to_string())));
        assert!(out.contains(&("PROJECT".to_string(), "Smith Residence".to_string())));
        assert!(out.iter().all(|(_, v)| !v.contains('%')));
        assert_eq!(ctx.expand("%unknown% %date%"), "%unknown% 2026-10-07");
    }

    #[test]
    fn new_macros_expand() {
        let ctx = MacroContext {
            client: "J. Smith".into(),
            address: "1 Main St".into(),
            project_number: "26-014".into(),
            revision: "C".into(),
            page_count: 7,
            date: "2026-10-07".into(),
            ..MacroContext::default()
        };
        assert_eq!(
            ctx.expand(
                "%client%|%address%|%project.number%|%revision%|%page.count%|%date.long%|%date%"
            ),
            "J. Smith|1 Main St|26-014|C|7|October 7, 2026|2026-10-07"
        );
        assert_eq!(long_date("10/7/2026"), "October 7, 2026");
        assert_eq!(long_date("sometime"), "sometime");
        assert_eq!(long_date("2026-13-40"), "2026-13-40");
    }

    #[test]
    fn project_information_macros_expand() {
        let ctx = MacroContext {
            client: "J. Smith".into(),
            extra: vec![
                ("%company%".into(), "Daniel Allen Designs".into()),
                ("%client.phone%".into(), "404-555-0100".into()),
                ("%custom.permit%".into(), "BP-22".into()),
            ],
            ..MacroContext::default()
        };
        assert_eq!(
            ctx.expand("%company%|%client%|%client.phone%|%custom.permit%|%custom.nope%"),
            "Daniel Allen Designs|J. Smith|404-555-0100|BP-22|%custom.nope%"
        );
        assert!(MacroContext::is_builtin("%client%"));
        assert!(!MacroContext::is_builtin("%company%"));
    }

    #[test]
    fn daniel_block_field_order_and_revision_table() {
        let t = TitleBlockTemplate::from_daniel_18x24();
        let labels: Vec<&str> = t.fields.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(
            labels,
            [
                "PROJECT",
                "CLIENT",
                "ADDRESS",
                "SHEET TITLE",
                "SHEET NO.",
                "DATE",
                "SCALE",
                "DRAWN BY"
            ]
        );
        assert_eq!(t.revision_rows, 5);
        assert_eq!(t.style, TitleBlockStyle::RightStrip);
    }

    #[test]
    fn revision_clouds_add_rows_to_the_revision_table() {
        use crate::model::Layout;
        use plan_core::Point;
        let mut l = Layout::new("t", plan_docs::SheetSize::ArchC);
        l.add_page(1, "Plan")
            .add_cloud(Point::new(1.0, 1.0), Point::new(3.0, 3.0), "B");
        l.add_page(2, "Elevations");
        {
            let p = &mut l.pages[1];
            p.add_cloud(Point::new(1.0, 1.0), Point::new(3.0, 3.0), "2");
            p.add_cloud(Point::new(4.0, 1.0), Point::new(6.0, 3.0), "B");
            p.add_cloud(Point::new(7.0, 1.0), Point::new(9.0, 3.0), "1");
            p.add_cloud(Point::new(7.0, 5.0), Point::new(9.0, 7.0), " ");
        }
        let mut ctx = MacroContext {
            revisions: vec![("1".into(), "2026-10-01".into(), "Issued for permit".into())],
            ..MacroContext::default()
        };
        ctx.add_cloud_revisions(&l);
        let marks: Vec<&str> = ctx.revisions.iter().map(|r| r.0.as_str()).collect();
        // Project Information's row stays as it was; the cloud marks follow
        // (numbers, then letters), each with the sheets that carry it.
        assert_eq!(marks, ["1", "2", "B"]);
        assert_eq!(ctx.revisions[0].2, "Issued for permit");
        assert_eq!(ctx.revisions[1].2, "Revision cloud on A-2");
        assert_eq!(ctx.revisions[2].2, "Revision cloud on A-1, A-2");
        assert_eq!(ctx.revision, "B", "the latest mark is the current revision");
        // Running it again adds nothing; a set revision is kept.
        let before = ctx.clone();
        ctx.add_cloud_revisions(&l);
        assert_eq!(ctx, before);
        let mut set = MacroContext {
            revision: "A".into(),
            ..MacroContext::default()
        };
        set.add_cloud_revisions(&l);
        assert_eq!(set.revision, "A");
        assert_eq!(set.revisions.len(), 3);
    }
}
