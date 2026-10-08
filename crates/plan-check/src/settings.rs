//! Plan Check Settings: which rules run, the limits they compare against (the
//! jurisdiction preset), and the findings the user chose to ignore.
//!
//! Both are kept with the plan so they travel with the file. plan-core has no
//! field for them, so they are stored as two reserved entries of
//! `Project::info.custom` (`plancheck.settings` and `plancheck.ignored`), JSON
//! in a string. [`CheckSettings::load`] and [`CheckSettings::store`],
//! [`ignored_keys`] and [`set_ignored_keys`] are the only code that touches
//! them, so moving the storage later is a change in this file.

use std::collections::BTreeSet;

use plan_core::{Project, Room};
use plan_stairs::Stair;
use serde::{Deserialize, Serialize};

use crate::{plan_check, CheckOptions, Finding, Severity, Target};

/// The jurisdiction presets of the settings dialog. `Custom` is what the
/// dialog shows once a limit has been edited.
pub const JURISDICTIONS: [&str; 2] = ["IRC 2021 residential", "Custom"];

const PRESET_IRC_2021: &str = "IRC 2021 residential";
const CUSTOM: &str = "Custom";
const SETTINGS_KEY: &str = "plancheck.settings";
const IGNORED_KEY: &str = "plancheck.ignored";

/// One rule of the Plan Check Settings list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleInfo {
    /// The `rule` string of the findings it produces (the code reference).
    pub id: &'static str,
    /// Heading in the settings list.
    pub group: &'static str,
    /// The usual (highest) severity of its findings.
    pub severity: Severity,
    /// One line about what it checks.
    pub summary: &'static str,
}

const fn r(
    id: &'static str,
    group: &'static str,
    severity: Severity,
    summary: &'static str,
) -> RuleInfo {
    RuleInfo {
        id,
        group,
        severity,
        summary,
    }
}

use Severity::{Error as E, Info as I, Warning as W};

static CATALOG: &[RuleInfo] = &[
    r(
        "IRC R304.1 minimum room area",
        "Rooms",
        E,
        "Habitable rooms are at least 70 sq ft",
    ),
    r(
        "IRC R304.2 minimum room dimension",
        "Rooms",
        E,
        "Habitable rooms are at least 7' across",
    ),
    r(
        "IRC R305.1 ceiling height",
        "Rooms",
        E,
        "Ceilings are at least 7' (6'-8\" in baths)",
    ),
    r(
        "IRC R311.6 hallway width",
        "Rooms",
        E,
        "Hallways are at least 36\" wide",
    ),
    r(
        "IRC R311.1 means of egress",
        "Rooms",
        E,
        "Every room can be reached through a door",
    ),
    r(
        "IRC R311.1 access through bathroom",
        "Rooms",
        W,
        "No room is entered only through a bathroom",
    ),
    r(
        "Plan hygiene: unnamed room",
        "Rooms",
        I,
        "Rooms have a name or type",
    ),
    r(
        "IRC R310.2 egress",
        "Doors and windows",
        E,
        "Bedroom egress window or exterior door",
    ),
    r(
        "IRC R303.3 ventilation",
        "Doors and windows",
        I,
        "Baths, kitchens and bedrooms without a window",
    ),
    r(
        "IRC R303.1 natural light",
        "Doors and windows",
        I,
        "Glazing is 8% of the floor area",
    ),
    r(
        "IRC R303.1 ventilation area",
        "Doors and windows",
        W,
        "Openable glazing is 4% of the floor area",
    ),
    r(
        "IRC R311.2 door width",
        "Doors and windows",
        W,
        "Entry, interior and bath door widths",
    ),
    r(
        "IRC R311.2 egress door height",
        "Doors and windows",
        W,
        "Exterior doors are 80\" high",
    ),
    r(
        "IRC R311.2 bedroom door swing",
        "Doors and windows",
        I,
        "Bedroom doors swing into the room",
    ),
    r(
        "IRC R311.3 landing at door",
        "Doors and windows",
        W,
        "A landing between an exterior door and a stair",
    ),
    r(
        "IRC R312.1.1 door to a drop",
        "Doors and windows",
        E,
        "Upper-floor exterior doors open onto a deck",
    ),
    r(
        "IRC R312.2 window fall protection",
        "Doors and windows",
        I,
        "Low windows on upper floors",
    ),
    r(
        "IRC R311.7.1 stair width",
        "Stairs and guards",
        E,
        "Stairs are at least 36\" wide",
    ),
    r(
        "IRC R311.7.2 headroom",
        "Stairs and guards",
        E,
        "Stair headroom is 6'-8\"",
    ),
    r(
        "IRC R311.7.3 vertical rise",
        "Stairs and guards",
        E,
        "No flight rises more than 12'-7\"",
    ),
    r(
        "IRC R311.7.5.1 riser height",
        "Stairs and guards",
        E,
        "Risers are 7 3/4\" or less",
    ),
    r(
        "IRC R311.7.5.2 tread depth",
        "Stairs and guards",
        E,
        "Treads are at least 10\"",
    ),
    r(
        "IRC R311.7.5 stair comfort (2R+T)",
        "Stairs and guards",
        I,
        "2R+T falls between 24\" and 25\"",
    ),
    r(
        "IRC R311.7.6 landings",
        "Stairs and guards",
        E,
        "Landings are at least 36\" deep",
    ),
    r(
        "IRC R311.7.6 door swing over stair",
        "Stairs and guards",
        W,
        "A door does not swing into a stair",
    ),
    r(
        "IRC R311.7.8 handrails",
        "Stairs and guards",
        W,
        "Flights of 4 or more risers have a handrail",
    ),
    r(
        "IRC R312.1.2 guard height",
        "Stairs and guards",
        E,
        "Stair railings are at least 34\" high",
    ),
    r(
        "IRC R312.1.1 guards",
        "Stairs and guards",
        E,
        "Decks over 30\" high have a guard",
    ),
    r(
        "IRC R311.8 ramps",
        "Stairs and guards",
        E,
        "Ramp slope and landings",
    ),
    r(
        "IRC R307.1 water closet clearance",
        "Bath and kitchen",
        W,
        "15\" to each side and 21\" in front of a toilet",
    ),
    r(
        "IRC P2708.1 shower and tub size",
        "Bath and kitchen",
        W,
        "Showers and tubs are at least 30\" x 30\"",
    ),
    r(
        "IRC R307.1 door swing into fixture",
        "Bath and kitchen",
        W,
        "A bath door does not swing across a fixture",
    ),
    r(
        "NKBA kitchen aisle width",
        "Bath and kitchen",
        W,
        "Kitchen aisles are 36\" (42\" work aisle)",
    ),
    r(
        "NKBA kitchen counter depth",
        "Bath and kitchen",
        I,
        "Counters are at least 24\" deep",
    ),
    r(
        "IRC R309.1 garage floor",
        "Garage",
        I,
        "The garage floor sits below the house",
    ),
    r(
        "IRC R302.5.1 garage opening",
        "Garage",
        E,
        "The garage does not open into a bedroom",
    ),
    r(
        "IRC R302.5.1 garage door width",
        "Garage",
        E,
        "The door to the house is at least 32\" wide",
    ),
    r(
        "IRC R302.5.1 garage door rating",
        "Garage",
        I,
        "The door to the house is solid or 20-minute rated",
    ),
    r(
        "IRC R905.2.2 roof slope",
        "Roof",
        W,
        "Shingle roofs are 2:12 or steeper",
    ),
    r(
        "IRC R905.1.1 underlayment",
        "Roof",
        I,
        "Roofs under 4:12 need double underlayment",
    ),
    r(
        "Roof pitch: steep slope",
        "Roof",
        I,
        "Roofs steeper than 12:12",
    ),
    r(
        "IRC R314.3 smoke alarms",
        "Electrical",
        W,
        "A smoke alarm in each bedroom",
    ),
    r(
        "IRC R314.3 smoke alarm on every level",
        "Electrical",
        W,
        "A smoke alarm on every level",
    ),
    r(
        "IRC R315.2 carbon monoxide alarms",
        "Electrical",
        I,
        "A CO alarm with bedrooms and an attached garage",
    ),
    r(
        "NEC 210.8(A) GFCI protection",
        "Electrical",
        W,
        "GFCI receptacles in wet areas",
    ),
    r(
        "NEC 210.52(A) receptacle spacing",
        "Electrical",
        W,
        "A receptacle within 6' along every wall",
    ),
    r(
        "IRC R602.7 header size",
        "Framing",
        W,
        "Header sizes against the span table",
    ),
    r("IRC R502.3.1 joist span", "Framing", W, "Floor joist spans"),
    r(
        "Plan geometry: tiny wall",
        "Plan geometry",
        W,
        "Walls under 6\" long",
    ),
    r(
        "Plan geometry: dangling exterior wall",
        "Plan geometry",
        W,
        "Exterior walls with a free end",
    ),
    r(
        "Plan geometry: duplicate wall",
        "Plan geometry",
        W,
        "Walls drawn twice on one line",
    ),
    r(
        "Plan geometry: opening",
        "Plan geometry",
        E,
        "Openings fit and do not overlap",
    ),
];

/// Every rule the settings dialog lists, in dialog order. A finding whose
/// rule is not in the list is always shown.
pub fn rule_catalog() -> &'static [RuleInfo] {
    CATALOG
}

/// Which rules run, and the limits they use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CheckSettings {
    /// A name of [`JURISDICTIONS`].
    pub jurisdiction: String,
    /// The limits.
    pub options: CheckOptions,
    /// Rules switched off, by [`RuleInfo::id`].
    pub disabled: BTreeSet<String>,
}

impl Default for CheckSettings {
    fn default() -> Self {
        Self::irc_2021()
    }
}

impl CheckSettings {
    /// The "IRC 2021 residential" preset: its limits and every rule on.
    pub fn irc_2021() -> Self {
        Self {
            jurisdiction: PRESET_IRC_2021.to_string(),
            options: CheckOptions::default(),
            disabled: BTreeSet::new(),
        }
    }

    /// Whether the rule runs.
    pub fn is_enabled(&self, rule: &str) -> bool {
        !self.disabled.contains(rule)
    }

    /// Switch a rule on or off.
    pub fn set_enabled(&mut self, rule: &str, on: bool) {
        if on {
            self.disabled.remove(rule);
        } else {
            self.disabled.insert(rule.to_string());
        }
    }

    /// Names the settings after the limits: the preset while they match it,
    /// Custom once one was edited. Call after changing `options`.
    pub fn name_from_limits(&mut self) {
        self.jurisdiction = if self.options == CheckOptions::default() {
            PRESET_IRC_2021.to_string()
        } else {
            CUSTOM.to_string()
        };
    }

    /// The settings stored with the plan, or the 2021 IRC preset.
    pub fn load(project: &Project) -> Self {
        read_entry(project, SETTINGS_KEY)
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Keep the settings with the plan (nothing is stored for the defaults).
    pub fn store(&self, project: &mut Project) {
        if *self == Self::default() {
            write_entry(project, SETTINGS_KEY, None);
        } else if let Ok(s) = serde_json::to_string(self) {
            write_entry(project, SETTINGS_KEY, Some(s));
        }
    }
}

fn read_entry(project: &Project, key: &str) -> Option<String> {
    project
        .info
        .custom
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
}

fn write_entry(project: &mut Project, key: &str, value: Option<String>) {
    let custom = &mut project.info.custom;
    match (value, custom.iter().position(|(k, _)| k == key)) {
        (Some(v), Some(i)) => custom[i].1 = v,
        (Some(v), None) => custom.push((key.to_string(), v)),
        (None, Some(i)) => {
            custom.remove(i);
        }
        (None, None) => {}
    }
}

/// The identity of a finding for the Ignore list: floor, rule, object and
/// place (whole inches). Room findings leave the room index out because it
/// shifts when rooms are added; their place is the room's centre.
pub fn finding_key(floor: usize, f: &Finding) -> String {
    let who = match f.object {
        Some(Target::Wall(i)) => format!("wall{i}"),
        Some(Target::Opening(i)) => format!("opening{i}"),
        Some(Target::Stair(i)) => format!("stair{i}"),
        Some(Target::Symbol(i)) => format!("symbol{i}"),
        Some(Target::Cabinet(i)) => format!("cabinet{i}"),
        Some(Target::Roof(i)) => format!("roof{i}"),
        Some(Target::Detail(i)) => format!("detail{i}"),
        Some(Target::Room(_)) => "room".to_string(),
        None => "-".to_string(),
    };
    let at = f
        .location
        .map_or("-".to_string(), |p| format!("{:.0},{:.0}", p.x, p.y));
    format!("{floor}|{}|{who}|{at}", f.rule)
}

/// The findings the user ignored in this plan.
pub fn ignored_keys(project: &Project) -> BTreeSet<String> {
    read_entry(project, IGNORED_KEY)
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default()
        .into_iter()
        .collect()
}

/// Replace the ignore list stored with the plan.
pub fn set_ignored_keys(project: &mut Project, keys: &BTreeSet<String>) {
    let value = (!keys.is_empty())
        .then(|| serde_json::to_string(&keys.iter().collect::<Vec<_>>()).ok())
        .flatten();
    write_entry(project, IGNORED_KEY, value);
}

/// The result of a Plan Check with the plan's settings applied.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CheckRun {
    /// Findings to walk through, most severe first.
    pub findings: Vec<Finding>,
    /// Findings the user ignored.
    pub ignored: Vec<Finding>,
    /// How many findings the switched-off rules would have produced.
    pub switched_off: usize,
}

impl CheckRun {
    /// `(errors, warnings, info)` of the findings still to look at.
    pub fn counts(&self) -> (usize, usize, usize) {
        let n = |s| self.findings.iter().filter(|f| f.severity == s).count();
        (n(Severity::Error), n(Severity::Warning), n(Severity::Info))
    }

    /// The status-bar line, e.g. `Plan Check: 2 errors, 3 warnings, 1 info (4 ignored)`.
    pub fn summary(&self) -> String {
        summary_line("Plan Check", &self.findings, self.ignored.len())
    }
}

/// The status-bar line for `findings` still to look at and `ignored` ones set
/// aside, e.g. `Plan Check: 2 errors, 3 warnings, 1 info (4 ignored)`.
pub fn summary_line(title: &str, findings: &[Finding], ignored: usize) -> String {
    let n = |s| findings.iter().filter(|f| f.severity == s).count();
    let plural =
        |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let mut s = if findings.is_empty() {
        format!("{title}: no findings")
    } else {
        format!(
            "{title}: {}, {}, {} info",
            plural(n(Severity::Error), "error", "errors"),
            plural(n(Severity::Warning), "warning", "warnings"),
            n(Severity::Info)
        )
    };
    if ignored > 0 {
        s.push_str(&format!(" ({ignored} ignored)"));
    }
    s
}

/// Applies the plan's settings and ignore list to `findings` of `floor`:
/// findings of rules that are switched off are dropped, ignored ones are set
/// aside.
pub fn filter_findings(project: &Project, floor: usize, findings: Vec<Finding>) -> CheckRun {
    let settings = CheckSettings::load(project);
    let ignored = ignored_keys(project);
    let mut run = CheckRun::default();
    for f in findings {
        if !settings.is_enabled(f.rule) {
            run.switched_off += 1;
        } else if ignored.contains(&finding_key(floor, &f)) {
            run.ignored.push(f);
        } else {
            run.findings.push(f);
        }
    }
    run
}

/// Plan Check with the settings and ignore list stored in `project`: rules
/// that are switched off are dropped, ignored findings are set aside.
///
/// # Panics
/// Panics if `floor` is not a valid floor index.
pub fn run_plan_check(
    project: &Project,
    floor: usize,
    rooms: &[Room],
    room_types: &[(usize, String)],
    stairs: &[Stair],
) -> CheckRun {
    let settings = CheckSettings::load(project);
    let all = plan_check(project, floor, rooms, room_types, stairs, &settings.options);
    filter_findings(project, floor, all)
}
