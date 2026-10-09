//! The Plan Check report as a table, for the layout and PDF report page.
//!
//! plan-check does not know about plan-docs or plan-layout; the application
//! turns a [`ReportTable`] into a `plan_docs::Schedule` (same title, columns
//! and rows) and prints it.

use plan_core::units::fmt_ft_in;

use crate::{Finding, Target};

/// A titled table of strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportTable {
    /// Page title.
    pub title: String,
    /// Column headings.
    pub columns: Vec<String>,
    /// One row per finding, most severe first.
    pub rows: Vec<Vec<String>>,
}

impl Finding {
    /// What the finding is about, e.g. `Opening 12` or `Room 3`.
    pub fn target_label(&self) -> String {
        match self.object {
            Some(Target::Wall(i)) => format!("Wall {i}"),
            Some(Target::Opening(i)) => format!("Opening {i}"),
            Some(Target::Room(i)) => format!("Room {}", i + 1),
            Some(Target::Stair(i)) => format!("Stair {i}"),
            Some(Target::Symbol(i)) => format!("Fixture {i}"),
            Some(Target::Cabinet(i)) => format!("Cabinet {i}"),
            Some(Target::Roof(i)) => format!("Roof plane {i}"),
            Some(Target::Detail(i)) => format!("Deck {i}"),
            Some(Target::Foundation(i)) => format!("Foundation {i}"),
            None => "Plan".to_string(),
        }
    }

    /// The object and, when known, where it is on the plan.
    pub fn where_text(&self) -> String {
        match self.location {
            Some(p) => format!(
                "{} at {}, {}",
                self.target_label(),
                fmt_ft_in(p.x),
                fmt_ft_in(p.y)
            ),
            None => self.target_label(),
        }
    }
}

/// The findings as a table: number, severity, code reference, where, the
/// finding and its fix.
pub fn report_table(floor_name: &str, findings: &[Finding]) -> ReportTable {
    ReportTable {
        title: format!("Plan Check - {floor_name}"),
        columns: [
            "No.",
            "Severity",
            "Code reference",
            "Where",
            "Finding",
            "Fix",
        ]
        .iter()
        .map(|c| (*c).to_string())
        .collect(),
        rows: findings
            .iter()
            .enumerate()
            .map(|(i, f)| {
                vec![
                    (i + 1).to_string(),
                    f.severity.singular().to_string(),
                    f.rule.to_string(),
                    f.where_text(),
                    f.message.clone(),
                    f.fix.clone(),
                ]
            })
            .collect(),
    }
}
