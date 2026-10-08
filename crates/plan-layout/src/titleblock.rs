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

/// Values the macros expand to. The per-sheet fields are filled in by
/// [`crate::render_pdf`] for each page.
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
}

impl MacroContext {
    /// Replace every known macro in `text`:
    /// `%project.name%`, `%client%`, `%address%`, `%designer%`, `%date%`,
    /// `%sheet.number%`, `%sheet.title%`, `%scale%`. Unknown `%...%` stay as written.
    pub fn expand(&self, text: &str) -> String {
        [
            ("%project.name%", &self.project_name),
            ("%client%", &self.client),
            ("%address%", &self.address),
            ("%designer%", &self.designer),
            ("%date%", &self.date),
            ("%sheet.number%", &self.sheet_number),
            ("%sheet.title%", &self.sheet_title),
            ("%scale%", &self.scale),
        ]
        .into_iter()
        .fold(text.to_string(), |acc, (k, v)| acc.replace(k, v))
    }
}

/// A title block: a style and its labelled fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TitleBlockTemplate {
    pub style: TitleBlockStyle,
    /// `(label, macro text)` pairs, top to bottom (or left to right).
    pub fields: Vec<(String, String)>,
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
        };
        let out = TitleBlockTemplate::presentation_18x24().expand_macros(&ctx);
        assert_eq!(out.len(), 8);
        assert!(out.contains(&("SHEET NO.".to_string(), "A-3".to_string())));
        assert!(out.contains(&("PROJECT".to_string(), "Smith Residence".to_string())));
        assert!(out.iter().all(|(_, v)| !v.contains('%')));
        assert_eq!(ctx.expand("%unknown% %date%"), "%unknown% 2026-10-07");
    }
}
