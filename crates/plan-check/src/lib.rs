//! plan-check: Chief's Tools > Checks menu as an extensible rule engine.
//!
//! [`plan_check`] walks a floor of a [`Project`] and returns [`Finding`]s
//! (severity, location, the rule text and a fix suggestion) for residential
//! code concerns based on the 2021 IRC. [`door_window_check`] is the focused
//! Door and Window check, [`plan_footprint`] is Plan Footprint and
//! [`report_markdown`] formats findings for a report.
//!
//! All lengths are inches. Rooms are measured to wall centerlines because
//! [`plan_core::Room`] carries no inner polygon, so clear (finished)
//! dimensions are slightly smaller than the ones checked here.

mod ctx;
mod footprint;
mod geom;
mod rules;
#[cfg(test)]
mod tests;

use plan_core::{detect_rooms, Id, Point, Project, Room};
use plan_stairs::Stair;
use serde::{Deserialize, Serialize};

use ctx::Ctx;
pub use footprint::{plan_footprint, Footprint};

/// How serious a finding is. Ordered most severe first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Severity {
    /// A code violation or a broken model.
    Error,
    /// Likely a problem; needs a look.
    Warning,
    /// Advice or a note.
    Info,
}

impl Severity {
    /// Heading text used in reports.
    pub fn label(self) -> &'static str {
        match self {
            Severity::Error => "Errors",
            Severity::Warning => "Warnings",
            Severity::Info => "Info",
        }
    }
}

/// The plan object a finding is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    /// A wall, by id.
    Wall(Id),
    /// A door or window, by id.
    Opening(Id),
    /// A room, by index into the room list given to [`plan_check`].
    Room(usize),
    /// A stair, by id.
    Stair(Id),
}

/// One result of a check.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Finding {
    /// The rule text, e.g. `"IRC R310.2 egress"`.
    pub rule: &'static str,
    /// How serious it is.
    pub severity: Severity,
    /// What is wrong, with the measured values.
    pub message: String,
    /// Where to look in the plan, when there is a place.
    pub location: Option<Point>,
    /// The object concerned.
    pub object: Option<Target>,
    /// A suggested fix.
    pub fix: String,
}

/// The limits the checks compare against (inches and square feet).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CheckOptions {
    /// Code edition (IRC year).
    pub code_year: u32,
    /// IECC climate zone, when known.
    pub climate_zone: Option<u8>,
    /// Minimum habitable room area, sq ft (R304.1).
    pub min_room_area: f64,
    /// Minimum horizontal room dimension (R304.2).
    pub min_room_dim: f64,
    /// Minimum ceiling height (R305.1).
    pub min_ceiling: f64,
    /// Minimum hallway width (R311.6).
    pub min_hall_width: f64,
    /// Minimum egress opening area, sq ft (R310.2.1; 5.0 is used on the grade floor).
    pub egress_min_area: f64,
    /// Minimum egress opening width (R310.2.1).
    pub egress_min_w: f64,
    /// Minimum egress opening height (R310.2.1).
    pub egress_min_h: f64,
    /// Maximum egress sill height above the floor (R310.2.2).
    pub egress_max_sill: f64,
    /// Clear width of the main entry door (R311.2); the leaf is 4" more.
    pub min_door_width: f64,
    /// Minimum bathroom door width.
    pub bath_door: f64,
    /// Minimum stair width (R311.7.1).
    pub stair_min_width: f64,
    /// Maximum riser height (R311.7.5.1).
    pub riser_max: f64,
    /// Minimum tread depth (R311.7.5.2).
    pub tread_min: f64,
    /// Minimum stair headroom (R311.7.2).
    pub headroom: f64,
    /// Minimum landing depth (R311.7.6).
    pub landing_min: f64,
    /// Minimum clear width of the garage-to-house door (R302.5.1).
    pub garage_house_door_min: f64,
    /// Glazing to exterior wall area ratio below which natural light is flagged.
    pub window_to_wall_ratio_warn: f64,
}

impl Default for CheckOptions {
    fn default() -> Self {
        Self {
            code_year: 2021,
            climate_zone: None,
            min_room_area: 70.0,
            min_room_dim: 84.0,
            min_ceiling: 84.0,
            min_hall_width: 36.0,
            egress_min_area: 5.7,
            egress_min_w: 20.0,
            egress_min_h: 24.0,
            egress_max_sill: 44.0,
            min_door_width: 32.0,
            bath_door: 24.0,
            stair_min_width: 36.0,
            riser_max: 7.75,
            tread_min: 10.0,
            headroom: 80.0,
            landing_min: 36.0,
            garage_house_door_min: 32.0,
            window_to_wall_ratio_warn: 0.15,
        }
    }
}

/// Run every rule on `floor` of `project` (Tools > Checks > Plan Check).
///
/// `rooms` are the detected rooms of that floor; `room_types` gives the type
/// of room `index` (e.g. `"Bedroom"`). A room without an entry takes the type
/// of a [`plan_core::RoomName`] anchored inside it, and failing that stays
/// unnamed. `stairs` are the stairs on this floor. Findings come back most
/// severe first, otherwise in rule order.
///
/// # Panics
/// Panics if `floor` is not a valid floor index.
pub fn plan_check(
    project: &Project,
    floor: usize,
    rooms: &[Room],
    room_types: &[(usize, String)],
    stairs: &[Stair],
    opts: &CheckOptions,
) -> Vec<Finding> {
    let ctx = Ctx::new(&project.floors[floor], rooms, room_types, stairs, opts);
    let mut out = Vec::new();
    rules::habitable_room_size(&ctx, &mut out);
    rules::bedroom_egress(&ctx, &mut out);
    rules::ventilation(&ctx, &mut out);
    rules::door_widths(&ctx, &mut out);
    rules::door_swings(&ctx, &mut out);
    rules::hallway_width(&ctx, &mut out);
    rules::stairs(&ctx, &mut out);
    rules::garage(&ctx, &mut out);
    rules::wall_geometry(&ctx, &mut out);
    rules::opening_geometry(&ctx, &mut out);
    rules::room_access(&ctx, &mut out);
    rules::natural_light(&ctx, &mut out);
    out.sort_by_key(|f| f.severity);
    out
}

/// Tools > Checks > Door/Window Check: the opening rules only (door widths and
/// opening geometry). Rooms are detected from the walls and typed from the
/// floor's room names.
///
/// # Panics
/// Panics if `floor` is not a valid floor index.
pub fn door_window_check(project: &Project, floor: usize) -> Vec<Finding> {
    let f = &project.floors[floor];
    let rooms = detect_rooms(&f.walls, 1.0);
    let opts = CheckOptions::default();
    let ctx = Ctx::new(f, &rooms, &[], &[], &opts);
    let mut out = Vec::new();
    rules::door_widths(&ctx, &mut out);
    rules::opening_geometry(&ctx, &mut out);
    out.sort_by_key(|f| f.severity);
    out
}

/// Format findings as Markdown: a count line, then one section per severity.
pub fn report_markdown(findings: &[Finding]) -> String {
    let count = |s: Severity| findings.iter().filter(|f| f.severity == s).count();
    let mut md = String::from("# Plan Check Report\n\n");
    if findings.is_empty() {
        md.push_str("No findings.\n");
        return md;
    }
    md.push_str(&format!(
        "{} findings: {} errors, {} warnings, {} info\n",
        findings.len(),
        count(Severity::Error),
        count(Severity::Warning),
        count(Severity::Info)
    ));
    for sev in [Severity::Error, Severity::Warning, Severity::Info] {
        let group: Vec<&Finding> = findings.iter().filter(|f| f.severity == sev).collect();
        if group.is_empty() {
            continue;
        }
        md.push_str(&format!("\n## {} ({})\n\n", sev.label(), group.len()));
        for f in group {
            md.push_str(&format!("- **{}**: {}", f.rule, f.message));
            if let Some(p) = f.location {
                md.push_str(&format!(" (at {:.0}, {:.0})", p.x, p.y));
            }
            md.push_str(&format!("\n  - Fix: {}\n", f.fix));
        }
    }
    md
}

/// Build a finding with no location or object (rules fill those in).
pub(crate) fn finding(
    rule: &'static str,
    severity: Severity,
    message: String,
    fix: &str,
) -> Finding {
    Finding {
        rule,
        severity,
        message,
        location: None,
        object: None,
        fix: fix.to_string(),
    }
}

impl Finding {
    /// Attach a plan location.
    pub(crate) fn at(mut self, p: Point) -> Self {
        self.location = Some(p);
        self
    }

    /// Attach the object concerned.
    pub(crate) fn on(mut self, t: Target) -> Self {
        self.object = Some(t);
        self
    }
}
