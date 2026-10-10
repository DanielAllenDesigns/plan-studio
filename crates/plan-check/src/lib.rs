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
mod minimums;
mod report;
mod rules;
mod rules_code;
mod rules_fixtures;
mod rules_irc;
mod rules_mep;
mod rules_nkba;
mod settings;
pub mod tables;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_code;
#[cfg(test)]
mod tests_irc;
#[cfg(test)]
mod tests_nkba;

use plan_core::{detect_rooms, Id, Point, Project, Room};
use plan_stairs::Stair;
use serde::{Deserialize, Serialize};

use ctx::Ctx;
pub use footprint::{plan_footprint, Footprint};
pub use minimums::CodeMinimums;
pub use report::{report_table, ReportTable};
pub use rules_nkba::{nkba_report, NkbaReport, NkbaRow, NkbaStatus};
pub use settings::{
    filter_findings, finding_key, ignored_keys, rule_catalog, run_plan_check, set_ignored_keys,
    summary_line, CheckRun, CheckSettings, PlanCheckSettings, RuleInfo, JURISDICTIONS,
};

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
    /// One finding's severity as a word.
    pub fn singular(self) -> &'static str {
        match self {
            Severity::Error => "Error",
            Severity::Warning => "Warning",
            Severity::Info => "Info",
        }
    }

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
    /// A placed library symbol (a plumbing fixture), by id.
    Symbol(Id),
    /// A cabinet, by id.
    Cabinet(Id),
    /// A roof plane, by id.
    Roof(Id),
    /// A detail object (a deck), by id.
    Detail(Id),
    /// A slab, square pad or round pier of the foundation layer, by id
    /// (`plan_core::foundation::FoundationRef`).
    Foundation(Id),
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
    /// Minimum ceiling height of a bathroom or laundry (R305.1 exception).
    pub min_ceiling_bath: f64,
    /// Drop above which a walking surface needs a guard (R312.1.1).
    pub guard_drop: f64,
    /// Minimum guard height (R312.1.2).
    pub guard_height: f64,
    /// Minimum guard height on the open side of a stair (R312.1.2 exception).
    pub stair_guard_height: f64,
    /// Flights with this many risers or more need a handrail (R311.7.8).
    pub handrail_risers: u32,
    /// Nominal height of an egress door (R311.2).
    pub entry_door_height: f64,
    /// Depth of the landing outside an exterior door (R311.3).
    pub exit_landing: f64,
    /// Openable glazing as a share of the floor area of a habitable room (R303.1).
    pub vent_ratio: f64,
    /// Water closet: centerline to side wall or fixture (P2705.1).
    pub wc_side_clear: f64,
    /// Water closet: clear space in front (P2705.1).
    pub wc_front_clear: f64,
    /// Shower and tub: smallest side (P2708.1).
    pub shower_min_dim: f64,
    /// Shower: smallest floor area, sq in (P2708.1).
    pub shower_min_area: f64,
    /// Kitchen walkway width (NKBA).
    pub kitchen_walkway: f64,
    /// Kitchen work aisle width, one cook (NKBA).
    pub kitchen_work_aisle: f64,
    /// Smallest counter depth including the overhangs.
    pub counter_depth_min: f64,
    /// Roof pitch (rise per 12) under which shingles are not allowed (R905.2.2).
    pub roof_pitch_min: f64,
    /// Roof pitch (rise per 12) under which double underlayment is required (R905.1.1).
    pub roof_pitch_underlay: f64,
    /// Roof pitch (rise per 12) over which a steep-slope advisory is shown.
    pub roof_pitch_steep: f64,
    /// Frost line depth below finished grade (R403.1.4.1); 12" in Georgia.
    pub frost_depth: f64,
    /// How far the first floor's finished floor sits above finished grade,
    /// inches (the plan has no grade of its own, so the footing rules assume it).
    pub grade_below_floor: f64,
    /// Thinnest footing (R403.1.1).
    pub footing_min_thickness: f64,
    /// Highest handrail (R311.7.8.1).
    pub handrail_max: f64,
    /// Sphere that must not pass through a guard (R312.1.3).
    pub guard_sphere: f64,
    /// Clear space in front of a shower or tub entrance (R307.1).
    pub shower_front_clear: f64,
    /// Thinnest gypsum board on the garage side of the house wall (R302.6).
    pub garage_gypsum_min: f64,
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
            min_ceiling_bath: 80.0,
            guard_drop: 30.0,
            guard_height: 36.0,
            stair_guard_height: 34.0,
            handrail_risers: 4,
            entry_door_height: 80.0,
            exit_landing: 36.0,
            vent_ratio: 0.04,
            wc_side_clear: 15.0,
            wc_front_clear: 21.0,
            shower_min_dim: 30.0,
            shower_min_area: 900.0,
            kitchen_walkway: 36.0,
            kitchen_work_aisle: 42.0,
            counter_depth_min: 24.0,
            roof_pitch_min: 2.0,
            roof_pitch_underlay: 4.0,
            roof_pitch_steep: 12.0,
            frost_depth: 12.0,
            grade_below_floor: 6.0,
            footing_min_thickness: 6.0,
            handrail_max: 38.0,
            guard_sphere: 4.0,
            shower_front_clear: 24.0,
            garage_gypsum_min: 0.5,
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
    let ctx =
        Ctx::new(&project.floors[floor], rooms, room_types, stairs, opts).with_project(project);
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
    rules_mep::electrical(&ctx, &mut out);
    rules_mep::framing(&ctx, &mut out);
    rules_code::run(&ctx, &mut out);
    rules_fixtures::run(&ctx, &mut out);
    rules_irc::run(&ctx, &mut out);
    rules_nkba::run(&ctx, &mut out);
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
