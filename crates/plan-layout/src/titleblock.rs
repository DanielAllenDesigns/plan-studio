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
    /// Replace every known macro in `text`:
    /// `%project.name%`, `%project.number%`, `%client%`, `%address%`,
    /// `%designer%`, `%date%`, `%date.long%`, `%revision%`, `%sheet.number%`,
    /// `%sheet.title%`, `%scale%`, `%page.count%`. Unknown `%...%` stay as written.
    pub fn expand(&self, text: &str) -> String {
        let page_count = self.page_count.to_string();
        let date_long = long_date(&self.date);
        [
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
        .fold(text.to_string(), |acc, (k, v)| acc.replace(k, v))
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
}
