//! Structural Member Reporting (manual pp. 905-908, 932): how framing members
//! are calculated in the Materials List.
//!
//! A [`ReportingDefault`] is one saved default: a [`ReportMethod`], the
//! length units, the Board Sizes table in priority order, the saw Kerf Width
//! and the long-run option. A [`ReportingSet`] keeps
//! any number of them, one active, and at most one Mixed.
//!
//! * **Buy List** matches the pieces of each size to the boards of the table
//!   (the highest-priority board long enough, several pieces cut from one
//!   board with the kerf between them) and counts the boards to buy. Pieces
//!   longer than every board listed, and sizes the table does not list, are
//!   the Other category and report their actual lengths.
//! * **Cut List** counts the pieces by size and cut length.
//! * **Linear Length** totals the linear feet of each size.
//! * **Mixed** counts studs, joists, rafters, posts and beams as pieces and
//!   totals plates, blocking, rim joists, ridges, fascia and (unless List Cut
//!   Header Lengths is on) headers in linear feet.
//!
//! Every line carries its Materials List category (Framing, Subfloor,
//! Roofing, Decks-Walks) from the member's definition or Role.

use crate::catalog::{FramingCatalog, MaterialsCategory, Role};
use crate::lumber::format_inches;
use crate::manual::{FramingMember, LumberSize};
use crate::member::Member;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const EPS: f64 = 1e-6;
/// Dimensions closer than this are the same size (a hundredth of an inch).
const SIZE_EPS: f64 = 0.01;

/// The four Reporting Methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ReportMethod {
    #[default]
    BuyList,
    CutList,
    LinearLength,
    Mixed,
}

impl ReportMethod {
    pub const ALL: [ReportMethod; 4] = [
        ReportMethod::BuyList,
        ReportMethod::CutList,
        ReportMethod::LinearLength,
        ReportMethod::Mixed,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ReportMethod::BuyList => "Buy List",
            ReportMethod::CutList => "Cut List",
            ReportMethod::LinearLength => "Linear Length",
            ReportMethod::Mixed => "Mixed Reporting",
        }
    }

    /// Does the Board Sizes table have a Length column?
    pub fn has_lengths(self) -> bool {
        self != ReportMethod::LinearLength
    }
}

/// The Length Units of the dialog and of the Materials List.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ReportUnits {
    #[default]
    Feet,
    Inches,
    Meters,
    Millimeters,
}

impl ReportUnits {
    pub const ALL: [ReportUnits; 4] = [
        ReportUnits::Feet,
        ReportUnits::Inches,
        ReportUnits::Meters,
        ReportUnits::Millimeters,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ReportUnits::Feet => "Feet",
            ReportUnits::Inches => "Inches",
            ReportUnits::Meters => "Meters",
            ReportUnits::Millimeters => "Millimeters",
        }
    }

    /// A length in inches as text: `8'`, `8'-6"`, `96 1/2"`, `2.44 m`, `2438 mm`.
    pub fn format(self, inches: f64) -> String {
        match self {
            ReportUnits::Feet => {
                let sixteenths = (inches * 16.0).round() as i64;
                let (ft, rest) = (sixteenths.div_euclid(192), sixteenths.rem_euclid(192));
                if rest == 0 {
                    format!("{ft}'")
                } else {
                    format!("{ft}'-{}\"", format_inches(rest as f64 / 16.0))
                }
            }
            ReportUnits::Inches => format!("{}\"", format_inches(inches)),
            ReportUnits::Meters => format!("{:.2} m", inches * 0.0254),
            ReportUnits::Millimeters => format!("{:.0} mm", inches * 25.4),
        }
    }

    /// Inches from a typed number in these units.
    pub fn to_inches(self, v: f64) -> f64 {
        match self {
            ReportUnits::Feet => v * 12.0,
            ReportUnits::Inches => v,
            ReportUnits::Meters => v / 0.0254,
            ReportUnits::Millimeters => v / 25.4,
        }
    }

    /// The number to show for a length in inches.
    pub fn from_inches(self, inches: f64) -> f64 {
        match self {
            ReportUnits::Feet => inches / 12.0,
            ReportUnits::Inches => inches,
            ReportUnits::Meters => inches * 0.0254,
            ReportUnits::Millimeters => inches * 25.4,
        }
    }
}

/// How a Buy List reports framing that is typically one long run of several
/// collinear boards (plates, girts, deck planking, rim joists) and the boards
/// that are longer than any board listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LongRun {
    /// Report Actual Lengths: each run on its own line.
    ActualLengths,
    /// Report Total Linear Length: the runs of a size added into one line.
    TotalLinearLength,
    /// Use Longest Buy List Board: the total divided by the longest board.
    #[default]
    LongestBoard,
}

impl LongRun {
    pub const ALL: [LongRun; 3] = [
        LongRun::ActualLengths,
        LongRun::TotalLinearLength,
        LongRun::LongestBoard,
    ];

    pub fn name(self) -> &'static str {
        match self {
            LongRun::ActualLengths => "Report Actual Lengths",
            LongRun::TotalLinearLength => "Report Total Linear Length",
            LongRun::LongestBoard => "Use Longest Buy List Board",
        }
    }
}

/// One row of the Board Sizes table (the Board Specification dialog).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BoardSpec {
    /// Actual thickness and depth, inches.
    pub thickness: f64,
    pub depth: f64,
    /// Board length, inches (`0` for Linear Length defaults).
    pub length: f64,
    /// The Framing Type, by name; empty matches any type.
    pub type_name: String,
    pub treated: bool,
    /// The Materials List formula of the line (stored; the Materials List
    /// formula editor evaluates it).
    pub formula: String,
}

impl Default for BoardSpec {
    fn default() -> Self {
        Self {
            thickness: 1.5,
            depth: 3.5,
            length: 96.0,
            type_name: "Lumber".into(),
            treated: false,
            formula: String::new(),
        }
    }
}

impl BoardSpec {
    pub fn new(thickness: f64, depth: f64, length_ft: f64) -> Self {
        Self {
            thickness,
            depth,
            length: length_ft * 12.0,
            ..Self::default()
        }
    }

    /// `2x4` for a nominal-sized board of the usual dressed sizes, else the
    /// actual dimensions.
    pub fn size_name(&self) -> String {
        for (t, d, name) in NOMINALS {
            if (self.thickness - t).abs() < SIZE_EPS && (self.depth - d).abs() < SIZE_EPS {
                return (*name).to_string();
            }
        }
        format!(
            "{}x{}",
            format_inches(self.thickness),
            format_inches(self.depth)
        )
    }

    fn matches(&self, k: &SizeKey) -> bool {
        (self.thickness - k.thickness).abs() < SIZE_EPS
            && (self.depth - k.depth).abs() < SIZE_EPS
            && (self.type_name.is_empty() || self.type_name == k.type_name)
            && self.treated == k.treated
    }
}

/// Actual (dressed) sizes of the usual nominal ones.
const NOMINALS: &[(f64, f64, &str)] = &[
    (0.75, 3.5, "1x4"),
    (0.75, 5.5, "1x6"),
    (1.5, 3.5, "2x4"),
    (1.5, 5.5, "2x6"),
    (1.5, 7.25, "2x8"),
    (1.5, 9.25, "2x10"),
    (1.5, 11.25, "2x12"),
    (3.5, 3.5, "4x4"),
    (3.5, 5.5, "4x6"),
    (3.5, 7.25, "4x8"),
    (3.5, 9.25, "4x10"),
    (3.5, 11.25, "4x12"),
    (5.5, 5.5, "6x6"),
];

/// One saved Structural Member Reporting default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReportingDefault {
    pub name: String,
    pub method: ReportMethod,
    pub units: ReportUnits,
    /// The Board Sizes table, highest priority first.
    pub boards: Vec<BoardSpec>,
    /// Material lost to the saw blade at each cut, inches (Buy and Cut List).
    pub kerf: f64,
    /// Buy List option for long runs and for boards longer than any listed.
    pub long_runs: LongRun,
}

impl Default for ReportingDefault {
    fn default() -> Self {
        Self::buy_list("Buy List")
    }
}

impl ReportingDefault {
    /// A Buy List with the usual board sizes: 2x4 to 2x12 in 8' to 20'
    /// (every 2'), 4x4, 4x6, 4x10 and 6x6 in 8', 10' and 12' and a few
    /// longer beams.
    pub fn buy_list(name: &str) -> Self {
        let mut boards = Vec::new();
        for (t, d) in [
            (1.5, 3.5),
            (1.5, 5.5),
            (1.5, 7.25),
            (1.5, 9.25),
            (1.5, 11.25),
        ] {
            for ft in [8.0, 10.0, 12.0, 14.0, 16.0, 18.0, 20.0] {
                boards.push(BoardSpec::new(t, d, ft));
            }
        }
        for (t, d) in [(3.5, 3.5), (3.5, 5.5), (3.5, 9.25), (5.5, 5.5)] {
            for ft in [8.0, 10.0, 12.0, 16.0] {
                boards.push(BoardSpec::new(t, d, ft));
            }
        }
        Self {
            name: name.into(),
            method: ReportMethod::BuyList,
            units: ReportUnits::Feet,
            boards,
            kerf: 0.125,
            long_runs: LongRun::LongestBoard,
        }
    }

    /// A default of `method` that starts from this default's boards (Copy /
    /// Convert; the lengths are dropped for Linear Length).
    pub fn converted(&self, name: &str, method: ReportMethod) -> Self {
        let mut d = self.clone();
        d.name = name.into();
        d.method = method;
        if method == ReportMethod::LinearLength {
            // One line per size: the lengths mean nothing.
            let mut seen: Vec<(f64, f64, String, bool)> = Vec::new();
            d.boards.retain(|b| {
                let key = (b.thickness, b.depth, b.type_name.clone(), b.treated);
                if seen.contains(&key) {
                    false
                } else {
                    seen.push(key);
                    true
                }
            });
            for b in &mut d.boards {
                b.length = 0.0;
            }
        } else if self.method == ReportMethod::LinearLength {
            d.boards = ReportingDefault::buy_list("").boards;
        }
        d
    }

    /// Moves the board at `i` one line up (higher priority); false at the top.
    pub fn priority_up(&mut self, i: usize) -> bool {
        if i == 0 || i >= self.boards.len() {
            return false;
        }
        self.boards.swap(i, i - 1);
        true
    }

    /// Moves the board at `i` one line down; false at the bottom.
    pub fn priority_down(&mut self, i: usize) -> bool {
        if i + 1 >= self.boards.len() {
            return false;
        }
        self.boards.swap(i, i + 1);
        true
    }

    /// The longest board listed for a size.
    fn longest_for(&self, k: &SizeKey) -> Option<f64> {
        self.boards
            .iter()
            .filter(|b| b.matches(k))
            .map(|b| b.length)
            .fold(None, |a, l| Some(a.map_or(l, |a: f64| a.max(l))))
    }
}

/// Why a saved default edit was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportingError {
    /// Empty or already used (names are case-sensitive and unique).
    BadName,
    Missing,
    /// Only one Mixed Reporting default may exist.
    SecondMixed,
    /// The last default cannot be deleted.
    Last,
    /// The default is the one in use by a Materials List.
    InUse,
}

impl std::fmt::Display for ReportingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ReportingError::BadName => "The name is empty or already used.",
            ReportingError::Missing => "There is no such default.",
            ReportingError::SecondMixed => "Only one Mixed Reporting default can exist in a plan.",
            ReportingError::Last => "The last default cannot be deleted.",
            ReportingError::InUse => "The default is in use by a saved Materials List.",
        })
    }
}

/// The saved Structural Member Reporting defaults of a plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReportingSet {
    pub defaults: Vec<ReportingDefault>,
    /// The Currently Active Default, by name.
    pub active: String,
    /// Defaults saved Materials Lists name; they cannot be deleted.
    pub used_by_lists: Vec<String>,
}

impl Default for ReportingSet {
    fn default() -> Self {
        Self {
            defaults: vec![ReportingDefault::buy_list("Buy List")],
            active: "Buy List".into(),
            used_by_lists: Vec::new(),
        }
    }
}

impl ReportingSet {
    pub fn get(&self, name: &str) -> Option<&ReportingDefault> {
        self.defaults.iter().find(|d| d.name == name)
    }

    /// The Currently Active Default (the first when the name is stale).
    pub fn active_default(&self) -> &ReportingDefault {
        self.get(&self.active)
            .or_else(|| self.defaults.first())
            .expect("a reporting set always has a default")
    }

    pub fn has_mixed(&self) -> bool {
        self.defaults
            .iter()
            .any(|d| d.method == ReportMethod::Mixed)
    }

    fn name_ok(&self, name: &str) -> bool {
        !name.trim().is_empty() && self.get(name.trim()).is_none()
    }

    /// New default (the New dialog or Copy/Convert): `method` may not be Mixed
    /// when one exists; `copy_from` seeds the boards of a Buy List.
    pub fn add(
        &mut self,
        name: &str,
        method: ReportMethod,
        copy_from: Option<&str>,
    ) -> Result<(), ReportingError> {
        let name = name.trim();
        if !self.name_ok(name) {
            return Err(ReportingError::BadName);
        }
        if method == ReportMethod::Mixed && self.has_mixed() {
            return Err(ReportingError::SecondMixed);
        }
        let mut d = match copy_from.and_then(|n| self.get(n)) {
            Some(src) if method == ReportMethod::BuyList => src.converted(name, method),
            _ => ReportingDefault {
                name: name.into(),
                method,
                boards: if method.has_lengths() {
                    ReportingDefault::buy_list("").boards
                } else {
                    ReportingDefault::buy_list("")
                        .converted("", ReportMethod::LinearLength)
                        .boards
                },
                ..ReportingDefault::buy_list(name)
            },
        };
        d.name = name.into();
        d.method = method;
        self.defaults.push(d);
        Ok(())
    }

    /// Copy/Convert of the default `src`: a new default of `method`.
    pub fn copy_convert(
        &mut self,
        src: &str,
        name: &str,
        method: ReportMethod,
    ) -> Result<(), ReportingError> {
        let name = name.trim();
        let base = self.get(src).cloned().ok_or(ReportingError::Missing)?;
        if !self.name_ok(name) {
            return Err(ReportingError::BadName);
        }
        if method == ReportMethod::Mixed && self.has_mixed() {
            return Err(ReportingError::SecondMixed);
        }
        self.defaults.push(base.converted(name, method));
        Ok(())
    }

    pub fn rename(&mut self, old: &str, new: &str) -> Result<(), ReportingError> {
        let new = new.trim();
        if old == new {
            return Ok(());
        }
        if self.get(old).is_none() {
            return Err(ReportingError::Missing);
        }
        if !self.name_ok(new) {
            return Err(ReportingError::BadName);
        }
        for d in &mut self.defaults {
            if d.name == old {
                d.name = new.into();
            }
        }
        if self.active == old {
            self.active = new.into();
        }
        for n in &mut self.used_by_lists {
            if n == old {
                *n = new.into();
            }
        }
        Ok(())
    }

    pub fn delete(&mut self, name: &str) -> Result<(), ReportingError> {
        if self.get(name).is_none() {
            return Err(ReportingError::Missing);
        }
        if self.defaults.len() == 1 {
            return Err(ReportingError::Last);
        }
        if self.used_by_lists.iter().any(|n| n == name) {
            return Err(ReportingError::InUse);
        }
        self.defaults.retain(|d| d.name != name);
        if self.active == name {
            self.active = self.defaults[0].name.clone();
        }
        Ok(())
    }

    /// Replaces the default called `name` (Edit); the name may change. The
    /// result may not make a second Mixed.
    pub fn replace(&mut self, name: &str, edited: ReportingDefault) -> Result<(), ReportingError> {
        let i = self
            .defaults
            .iter()
            .position(|d| d.name == name)
            .ok_or(ReportingError::Missing)?;
        if edited.method == ReportMethod::Mixed
            && self
                .defaults
                .iter()
                .enumerate()
                .any(|(k, d)| k != i && d.method == ReportMethod::Mixed)
        {
            return Err(ReportingError::SecondMixed);
        }
        let new_name = edited.name.trim().to_string();
        if new_name != name {
            self.rename(name, &new_name)?;
        }
        let i = self
            .defaults
            .iter()
            .position(|d| d.name == new_name)
            .ok_or(ReportingError::Missing)?;
        self.defaults[i] = ReportingDefault {
            name: new_name,
            ..edited
        };
        Ok(())
    }

    /// Makes `name` the Currently Active Default.
    pub fn set_active(&mut self, name: &str) -> bool {
        if self.get(name).is_some() {
            self.active = name.into();
            true
        } else {
            false
        }
    }
}

// ----- the pieces -----

/// What a piece's size is: dressed dimensions, the Framing Type and whether
/// it is treated.
#[derive(Debug, Clone, PartialEq)]
struct SizeKey {
    thickness: f64,
    depth: f64,
    type_name: String,
    treated: bool,
}

impl SizeKey {
    fn same(&self, o: &SizeKey) -> bool {
        (self.thickness - o.thickness).abs() < SIZE_EPS
            && (self.depth - o.depth).abs() < SIZE_EPS
            && self.type_name == o.type_name
            && self.treated == o.treated
    }
}

/// One piece of framing to report: what it is, its size and cut length.
#[derive(Debug, Clone, PartialEq)]
pub struct ReportInput {
    pub category: MaterialsCategory,
    pub role: Role,
    /// Member type name for descriptions, e.g. `"stud"`.
    pub label: String,
    /// The size as the list shows it (`2x10`, `I-Joist 1 1/2x9 1/4`).
    pub size_text: String,
    pub thickness: f64,
    pub depth: f64,
    pub type_name: String,
    pub treated: bool,
    pub length: f64,
    /// Long runs of collinear boards: plates, girts, planking, rim joists.
    pub long_run: bool,
    /// Index of the object the piece belongs to, for the caller's own use
    /// (the Materials List attributes lines to objects with it).
    pub source: usize,
}

impl ReportInput {
    fn key(&self) -> SizeKey {
        SizeKey {
            thickness: self.thickness,
            depth: self.depth,
            type_name: self.type_name.clone(),
            treated: self.treated,
        }
    }

    fn is_header(&self) -> bool {
        self.role == Role::Header
    }
}

fn is_long_run(role: Role) -> bool {
    matches!(
        role,
        Role::Plate
            | Role::RimJoist
            | Role::Ledger
            | Role::Ridge
            | Role::Fascia
            | Role::DeckPlank
            | Role::Sill
    )
}

/// Pieces that Mixed Reporting totals in linear feet instead of counting.
fn mixed_linear(role: Role) -> bool {
    is_long_run(role)
        || matches!(
            role,
            Role::WallBlocking
                | Role::JoistBlocking
                | Role::RoofBlocking
                | Role::Header
                | Role::GeneralFraming
                | Role::Hip
                | Role::Valley
                | Role::Purlin
        )
}

/// The pieces of automatic members. `first_source` is the `source` index of
/// the first member; the others follow.
pub fn auto_inputs(
    members: &[Member],
    catalog: &FramingCatalog,
    first_source: usize,
) -> Vec<ReportInput> {
    members
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let ty = catalog.type_of_member(m);
            let role = FramingCatalog::role_of_member(m);
            ReportInput {
                category: catalog.category_of_member(m),
                role,
                label: m.kind.name().to_string(),
                size_text: catalog.size_text(&ty, &m.lumber),
                thickness: m.lumber.thickness,
                depth: m.lumber.depth,
                type_name: ty.name,
                treated: false,
                length: m.length,
                long_run: is_long_run(role),
                source: first_source + i,
            }
        })
        .collect()
}

/// The pieces of manual members, one per ply (a truss yields its chords and
/// webs). Layout lines yield none.
pub fn manual_inputs(
    members: &[FramingMember],
    catalog: &FramingCatalog,
    first_source: usize,
) -> Vec<ReportInput> {
    let mut out = Vec::new();
    for (i, m) in members.iter().enumerate() {
        let Some(role) = m.role.or_else(|| Role::of_manual(m.kind)) else {
            continue;
        };
        let category = catalog
            .category_of_manual(m)
            .unwrap_or_else(|| role.category());
        let type_name = if !m.framing_type.is_empty() {
            m.framing_type.clone()
        } else {
            catalog.type_for_role(role).name
        };
        let ty = catalog.type_named(&type_name).cloned().unwrap_or_default();
        for p in m.pieces() {
            // Engineered sections keep their own dimensions; dimensional
            // lumber (and truss chords) use the dressed size of the name.
            let (thickness, depth) = match parse_dim(&p.size) {
                Some((t, d)) => (LumberSize::dim(t, d).width(), LumberSize::dim(t, d).depth()),
                None => (m.lumber.width(), m.lumber.depth()),
            };
            let lumber = crate::Lumber { thickness, depth };
            let size_text = if parse_dim(&p.size).is_some() {
                catalog.size_text(&ty, &lumber)
            } else {
                p.size.clone()
            };
            out.push(ReportInput {
                category,
                role,
                label: p.kind.to_string(),
                size_text,
                thickness,
                depth,
                type_name: ty.name.clone(),
                treated: m.treated,
                length: p.length,
                long_run: is_long_run(role),
                source: first_source + i,
            });
        }
    }
    out
}

/// `(2, 10)` from `"2x10"`; `None` for names like `"GLB 5 1/8x12"`.
fn parse_dim(name: &str) -> Option<(u32, u32)> {
    let (t, d) = name.split_once('x')?;
    Some((t.parse().ok()?, d.parse().ok()?))
}

// ----- the report -----

/// One line of the report.
#[derive(Debug, Clone, PartialEq)]
pub struct ReportLine {
    pub category: MaterialsCategory,
    /// `2x4 x 8'`, `2x4 stud x 92 5/8"`, `2x6 plate`.
    pub description: String,
    pub size: String,
    /// The board or cut length, inches; `None` for a total.
    pub length: Option<f64>,
    /// Pieces (`ea`) or feet (`lf`).
    pub qty: f64,
    pub unit: &'static str,
    /// True for the Other category of a Buy List (actual lengths).
    pub other: bool,
    /// `(source, share)` pairs: the objects behind the line and their share
    /// of its quantity (the shares add up to `qty`).
    pub sources: Vec<(usize, f64)>,
}

/// A report: its lines and the totals the dialog shows.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Report {
    pub lines: Vec<ReportLine>,
}

impl Report {
    /// Pieces counted (`ea` lines).
    pub fn pieces(&self) -> f64 {
        self.lines
            .iter()
            .filter(|l| l.unit == "ea")
            .map(|l| l.qty)
            .sum()
    }

    /// Feet of lumber the lines stand for: boards times length, cut pieces
    /// times length, and linear totals.
    pub fn linear_feet(&self) -> f64 {
        self.lines
            .iter()
            .map(|l| match (l.unit, l.length) {
                ("ea", Some(len)) => l.qty * len / 12.0,
                ("lf", _) => l.qty,
                _ => 0.0,
            })
            .sum()
    }

    /// Linear feet by category.
    pub fn linear_feet_in(&self, c: MaterialsCategory) -> f64 {
        Report {
            lines: self
                .lines
                .iter()
                .filter(|l| l.category == c)
                .cloned()
                .collect(),
        }
        .linear_feet()
    }
}

fn add_source(into: &mut Vec<(usize, f64)>, source: usize, share: f64) {
    match into.iter_mut().find(|(s, _)| *s == source) {
        Some((_, q)) => *q += share,
        None => into.push((source, share)),
    }
}

/// Spreads `qty` over the pieces in proportion to their lengths.
fn spread(pieces: &[&ReportInput], qty: f64) -> Vec<(usize, f64)> {
    let total: f64 = pieces.iter().map(|p| p.length).sum();
    let mut out = Vec::new();
    for p in pieces {
        let share = if total > EPS {
            qty * p.length / total
        } else {
            qty / pieces.len().max(1) as f64
        };
        add_source(&mut out, p.source, share);
    }
    out
}

/// Computes the report of `inputs` for `d`. `list_cut_headers` is List Cut
/// Header Lengths of the Openings panel (Mixed Reporting only).
pub fn report(inputs: &[ReportInput], d: &ReportingDefault, list_cut_headers: bool) -> Report {
    let mut lines = Vec::new();
    match d.method {
        ReportMethod::BuyList => buy_list(inputs, d, &mut lines),
        ReportMethod::CutList => cut_list(inputs.iter().collect(), d, &mut lines),
        ReportMethod::LinearLength => linear(inputs.iter().collect(), &mut lines),
        ReportMethod::Mixed => {
            let (lin, cut): (Vec<&ReportInput>, Vec<&ReportInput>) = inputs
                .iter()
                .partition(|p| mixed_linear(p.role) && !(p.is_header() && list_cut_headers));
            cut_list(cut, d, &mut lines);
            linear(lin, &mut lines);
        }
    }
    lines.sort_by(|a, b| {
        a.category
            .cmp(&b.category)
            .then(size_order(&a.size).cmp(&size_order(&b.size)))
            .then(a.other.cmp(&b.other))
            .then(a.length.unwrap_or(0.0).total_cmp(&b.length.unwrap_or(0.0)))
    });
    Report { lines }
}

/// Sort key of a size text: thickness, depth by the digits found.
fn size_order(size: &str) -> (u32, u32, String) {
    let nums: Vec<u32> = size
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    (
        nums.first().copied().unwrap_or(u32::MAX),
        nums.get(1).copied().unwrap_or(0),
        size.to_string(),
    )
}

/// Groups pieces by category and size key, keeping the first appearance order.
#[allow(clippy::type_complexity)]
fn group<'a>(
    pieces: &[&'a ReportInput],
) -> Vec<((MaterialsCategory, SizeKey, String), Vec<&'a ReportInput>)> {
    let mut groups: Vec<((MaterialsCategory, SizeKey, String), Vec<&ReportInput>)> = Vec::new();
    for p in pieces {
        let key = p.key();
        match groups
            .iter_mut()
            .find(|((c, k, s), _)| *c == p.category && k.same(&key) && *s == p.size_text)
        {
            Some((_, v)) => v.push(p),
            None => groups.push(((p.category, key, p.size_text.clone()), vec![p])),
        }
    }
    groups
}

fn cut_list(pieces: Vec<&ReportInput>, d: &ReportingDefault, out: &mut Vec<ReportLine>) {
    for ((category, _, size), members) in group(&pieces) {
        let mut by_len: BTreeMap<i64, Vec<&ReportInput>> = BTreeMap::new();
        for p in &members {
            by_len
                .entry((p.length * 16.0).round() as i64)
                .or_default()
                .push(p);
        }
        for (sixteenths, ps) in by_len {
            let len = sixteenths as f64 / 16.0;
            let label = ps[0].label.clone();
            let qty = ps.len() as f64;
            out.push(ReportLine {
                category,
                description: format!("{size} {label} x {}", d.units.format(len)),
                size: size.clone(),
                length: Some(len),
                qty,
                unit: "ea",
                other: false,
                sources: spread(&ps, qty),
            });
        }
    }
}

fn linear(pieces: Vec<&ReportInput>, out: &mut Vec<ReportLine>) {
    for ((category, _, size), members) in group(&pieces) {
        let feet: f64 = members.iter().map(|p| p.length).sum::<f64>() / 12.0;
        out.push(ReportLine {
            category,
            description: format!("{size} linear feet"),
            size,
            length: None,
            qty: feet,
            unit: "lf",
            other: false,
            sources: spread(&members, feet),
        });
    }
}

/// A board bought, with the pieces cut from it.
struct Bought<'a> {
    spec: usize,
    used: f64,
    pieces: Vec<&'a ReportInput>,
}

fn buy_list(inputs: &[ReportInput], d: &ReportingDefault, out: &mut Vec<ReportLine>) {
    let pieces: Vec<&ReportInput> = inputs.iter().collect();
    for ((category, key, size), members) in group(&pieces) {
        let listed = d.boards.iter().any(|b| b.matches(&key));
        let longest = d.longest_for(&key).unwrap_or(0.0);
        // Long runs: handled by the long-run option when the size is listed.
        let (runs, singles): (Vec<&ReportInput>, Vec<&ReportInput>) = members
            .iter()
            .copied()
            .partition(|p| p.long_run && listed && d.long_runs != LongRun::ActualLengths);
        if !runs.is_empty() {
            let total: f64 = runs.iter().map(|p| p.length).sum();
            if d.long_runs == LongRun::LongestBoard && longest > EPS {
                // Count of the longest board; the kerf of the joints between
                // boards is added to the run.
                let n = ((total + d.kerf * (total / longest).floor()) / longest - 1e-9).ceil();
                out.push(ReportLine {
                    category,
                    description: format!("{size} x {}", d.units.format(longest)),
                    size: size.clone(),
                    length: Some(longest),
                    qty: n,
                    unit: "ea",
                    other: false,
                    sources: spread(&runs, n),
                });
            } else {
                out.push(ReportLine {
                    category,
                    description: format!("{size} linear feet"),
                    size: size.clone(),
                    length: None,
                    qty: total / 12.0,
                    unit: "lf",
                    other: false,
                    sources: spread(&runs, total / 12.0),
                });
            }
        }
        // Everything else is packed into boards, longest piece first.
        let mut sorted = singles;
        sorted.sort_by(|a, b| b.length.total_cmp(&a.length));
        let mut bought: Vec<Bought> = Vec::new();
        let mut other: Vec<&ReportInput> = Vec::new();
        for p in sorted {
            if !listed {
                other.push(p);
                continue;
            }
            if let Some(b) = bought
                .iter_mut()
                .find(|b| b.used + p.length <= d.boards[b.spec].length + EPS)
            {
                b.used += p.length + d.kerf;
                b.pieces.push(p);
                continue;
            }
            // The first board in priority order that is long enough.
            match d
                .boards
                .iter()
                .position(|b| b.matches(&key) && b.length + EPS >= p.length)
            {
                Some(spec) => bought.push(Bought {
                    spec,
                    used: p.length + d.kerf,
                    pieces: vec![p],
                }),
                None => other.push(p),
            }
        }
        // One line per board length.
        let mut specs: Vec<usize> = bought.iter().map(|b| b.spec).collect();
        specs.sort_unstable();
        specs.dedup();
        for spec in specs {
            let boards: Vec<&Bought> = bought.iter().filter(|b| b.spec == spec).collect();
            let qty = boards.len() as f64;
            let covered: Vec<&ReportInput> = boards
                .iter()
                .flat_map(|b| b.pieces.iter().copied())
                .collect();
            let len = d.boards[spec].length;
            out.push(ReportLine {
                category,
                description: format!("{size} x {}", d.units.format(len)),
                size: size.clone(),
                length: Some(len),
                qty,
                unit: "ea",
                other: false,
                sources: spread(&covered, qty),
            });
        }
        // The Other category: actual lengths, or one total.
        if !other.is_empty() {
            if d.long_runs == LongRun::TotalLinearLength {
                let total: f64 = other.iter().map(|p| p.length).sum();
                out.push(ReportLine {
                    category,
                    description: format!("{size} linear feet (other)"),
                    size: size.clone(),
                    length: None,
                    qty: total / 12.0,
                    unit: "lf",
                    other: true,
                    sources: spread(&other, total / 12.0),
                });
            } else {
                let mut by_len: BTreeMap<i64, Vec<&ReportInput>> = BTreeMap::new();
                for p in &other {
                    by_len
                        .entry((p.length * 16.0).round() as i64)
                        .or_default()
                        .push(p);
                }
                for (sixteenths, ps) in by_len {
                    let len = sixteenths as f64 / 16.0;
                    let qty = ps.len() as f64;
                    out.push(ReportLine {
                        category,
                        description: format!("{size} x {} (other)", d.units.format(len)),
                        size: size.clone(),
                        length: Some(len),
                        qty,
                        unit: "ea",
                        other: true,
                        sources: spread(&ps, qty),
                    });
                }
            }
        }
    }
}

// ----- entry points -----

/// The report of automatic and manual members under the catalog's Currently
/// Active Default.
pub fn report_members(
    auto: &[Member],
    manual: &[FramingMember],
    catalog: &FramingCatalog,
    list_cut_headers: bool,
) -> Report {
    let mut inputs = auto_inputs(auto, catalog, 0);
    inputs.extend(manual_inputs(manual, catalog, auto.len()));
    report(
        &inputs,
        catalog.reporting.active_default(),
        list_cut_headers,
    )
}

impl ReportingDefault {
    /// The default as a `.calumber` file (JSON), for Export.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// A default read from a `.calumber` file; `None` when it is not one.
    pub fn from_json(text: &str) -> Option<Self> {
        let d: ReportingDefault = serde_json::from_str(text).ok()?;
        (!d.name.trim().is_empty()).then_some(d)
    }
}

impl ReportingSet {
    /// Import: adds `d` under a name that is not taken yet (`name 2`, ...); a
    /// Mixed default is refused when one exists. Returns the name used.
    pub fn import(&mut self, mut d: ReportingDefault) -> Result<String, ReportingError> {
        if d.method == ReportMethod::Mixed && self.has_mixed() {
            return Err(ReportingError::SecondMixed);
        }
        let base = d.name.trim().to_string();
        let mut name = base.clone();
        let mut n = 2;
        while self.get(&name).is_some() {
            name = format!("{base} {n}");
            n += 1;
        }
        d.name = name.clone();
        self.defaults.push(d);
        Ok(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::FramingCatalog;
    use crate::lumber::{TWO_BY_FOUR, TWO_BY_SIX, TWO_BY_TEN};
    use crate::manual::MemberKind as ManualKind;
    use crate::member::{Member, MemberKind, Transform3};
    use plan_core::Point;

    fn tf() -> Transform3 {
        Transform3 {
            origin: [0.0; 3],
            axis_x: [1.0, 0.0, 0.0],
            axis_y: [0.0, 1.0, 0.0],
        }
    }

    fn piece(kind: MemberKind, lumber: crate::Lumber, len: f64) -> Member {
        Member::new(kind, lumber, len, tf(), None)
    }

    /// Eight 92 5/8" studs, two 192" plates and three 100" joists.
    fn floor() -> Vec<Member> {
        let mut v = Vec::new();
        for _ in 0..8 {
            v.push(piece(MemberKind::Stud, TWO_BY_FOUR, 92.625));
        }
        for _ in 0..2 {
            v.push(piece(MemberKind::TopPlate, TWO_BY_FOUR, 192.0));
        }
        for _ in 0..3 {
            v.push(piece(MemberKind::Joist, TWO_BY_TEN, 100.0));
        }
        v
    }

    fn run(d: &ReportingDefault) -> Report {
        let c = FramingCatalog::default();
        report(&auto_inputs(&floor(), &c, 0), d, false)
    }

    #[test]
    fn a_cut_list_counts_pieces_and_its_footage_equals_the_takeoff() {
        let mut d = ReportingDefault::buy_list("Cuts");
        d.method = ReportMethod::CutList;
        let r = run(&d);
        assert_eq!(r.pieces(), 13.0);
        let t = crate::takeoff(&floor());
        let take_lf: f64 = t.linear_feet_by_size.iter().map(|(_, lf)| lf).sum();
        assert!(
            (r.linear_feet() - take_lf).abs() < 0.01,
            "{} vs {take_lf}",
            r.linear_feet()
        );
        // Studs are 8 pieces of one cut length, categorised Framing; joists
        // fall under Subfloor.
        let studs = r
            .lines
            .iter()
            .find(|l| l.description.contains("stud"))
            .unwrap();
        assert_eq!(
            (studs.qty, studs.category),
            (8.0, MaterialsCategory::Framing)
        );
        let joists = r
            .lines
            .iter()
            .find(|l| l.description.contains("joist"))
            .unwrap();
        assert_eq!(joists.category, MaterialsCategory::Subfloor);
    }

    #[test]
    fn linear_length_totals_each_size() {
        let mut d = ReportingDefault::buy_list("Lin");
        d.method = ReportMethod::LinearLength;
        let r = run(&d);
        assert_eq!(r.pieces(), 0.0);
        let two_by_four = r
            .lines
            .iter()
            .find(|l| l.size == "2x4" && l.category == MaterialsCategory::Framing)
            .unwrap();
        let want = (8.0 * 92.625 + 2.0 * 192.0) / 12.0;
        assert!((two_by_four.qty - want).abs() < 1e-9);
        let t = crate::takeoff(&floor());
        let take_lf: f64 = t.linear_feet_by_size.iter().map(|(_, lf)| lf).sum();
        assert!((r.linear_feet() - take_lf).abs() < 0.01);
    }

    #[test]
    fn a_buy_list_packs_pieces_into_boards_and_counts_the_boards() {
        let mut d = ReportingDefault::buy_list("Buy");
        d.long_runs = LongRun::ActualLengths;
        let r = run(&d);
        // Studs: the shortest 2x4 that holds one stud is the 8'... no: 92 5/8"
        // is longer than 8' = 96"? It is not (92.6 < 96), so one stud per 8'
        // board, except that no second stud fits: 8 boards.
        let stud_boards: f64 = r
            .lines
            .iter()
            .filter(|l| l.size == "2x4" && l.category == MaterialsCategory::Framing && !l.other)
            .map(|l| l.qty)
            .sum();
        // 8 studs plus two 192" plates (a 16' board holds a 192" plate, 16' =
        // 192", exactly) -> boards bought are at least the 10 pieces.
        assert!(stud_boards >= 10.0, "{stud_boards}");
        // Three 100" joists need 3 x 10' (120") boards, one piece each,
        // unless two fit a longer board: 100 + 100 + kerf > 20' -> no, 200 <
        // 240 so two joists fit one 20' board.
        let joist_lines: Vec<_> = r.lines.iter().filter(|l| l.size == "2x10").collect();
        let boards: f64 = joist_lines.iter().map(|l| l.qty).sum();
        assert!(boards <= 3.0);
        // Bought footage covers the pieces cut.
        assert!(
            r.linear_feet()
                >= crate::takeoff(&floor())
                    .linear_feet_by_size
                    .iter()
                    .map(|(_, f)| f)
                    .sum::<f64>()
        );
    }

    #[test]
    fn buy_list_chooses_the_highest_priority_board_that_is_long_enough() {
        let mut d = ReportingDefault::buy_list("Buy");
        d.long_runs = LongRun::ActualLengths;
        d.boards = vec![
            BoardSpec::new(1.5, 9.25, 12.0),
            BoardSpec::new(1.5, 9.25, 10.0),
        ];
        let c = FramingCatalog::default();
        let m = [piece(MemberKind::Joist, TWO_BY_TEN, 100.0)];
        let r = report(&auto_inputs(&m, &c, 0), &d, false);
        // 100" fits the 10' (120") board but the 12' board is higher in the
        // table, so it is the one bought.
        assert_eq!(r.lines[0].length, Some(144.0));
        d.priority_down(0);
        let r = report(&auto_inputs(&m, &c, 0), &d, false);
        assert_eq!(r.lines[0].length, Some(120.0));
    }

    #[test]
    fn kerf_decides_whether_two_pieces_share_a_board() {
        let c = FramingCatalog::default();
        let mut d = ReportingDefault::buy_list("Buy");
        d.boards = vec![BoardSpec::new(1.5, 3.5, 8.0)];
        // Two 47 15/16" studs make 95.875" in a 96" board with the kerf in
        // between only when the kerf is under 1/8".
        let m = [
            piece(MemberKind::Stud, TWO_BY_FOUR, 47.9375),
            piece(MemberKind::Stud, TWO_BY_FOUR, 47.9375),
        ];
        d.kerf = 0.0625;
        assert_eq!(report(&auto_inputs(&m, &c, 0), &d, false).lines[0].qty, 1.0);
        d.kerf = 0.25;
        assert_eq!(report(&auto_inputs(&m, &c, 0), &d, false).lines[0].qty, 2.0);
    }

    #[test]
    fn long_runs_follow_the_long_run_option_and_other_reports_actual_lengths() {
        let c = FramingCatalog::default();
        let mut d = ReportingDefault::buy_list("Buy");
        d.boards = vec![BoardSpec::new(1.5, 3.5, 16.0)];
        let plates: Vec<Member> = (0..3)
            .map(|_| piece(MemberKind::TopPlate, TWO_BY_FOUR, 300.0))
            .collect();
        // 900" of plate on a 192" longest board -> 5 boards.
        d.long_runs = LongRun::LongestBoard;
        let r = report(&auto_inputs(&plates, &c, 0), &d, false);
        assert_eq!(r.lines.len(), 1);
        assert_eq!((r.lines[0].qty, r.lines[0].length), (5.0, Some(192.0)));
        d.long_runs = LongRun::TotalLinearLength;
        let r = report(&auto_inputs(&plates, &c, 0), &d, false);
        assert_eq!((r.lines[0].qty, r.lines[0].unit), (75.0, "lf"));
        d.long_runs = LongRun::ActualLengths;
        let r = report(&auto_inputs(&plates, &c, 0), &d, false);
        assert!(r.lines[0].other, "300\" is longer than the 16' board");
        assert_eq!((r.lines[0].qty, r.lines[0].length), (3.0, Some(300.0)));
        // A size the table does not list is Other too.
        let odd = [piece(MemberKind::Joist, TWO_BY_SIX, 80.0)];
        let r = report(&auto_inputs(&odd, &c, 0), &d, false);
        assert!(r.lines[0].other);
    }

    #[test]
    fn mixed_counts_studs_and_totals_plates_and_headers_until_cut_headers_are_listed() {
        let c = FramingCatalog::default();
        let mut ms = floor();
        ms.push(piece(MemberKind::Header, TWO_BY_SIX, 48.0));
        ms.push(piece(MemberKind::Header, TWO_BY_SIX, 72.0));
        let mut d = ReportingDefault::buy_list("Mixed");
        d.method = ReportMethod::Mixed;
        let r = report(&auto_inputs(&ms, &c, 0), &d, false);
        let studs = r
            .lines
            .iter()
            .find(|l| l.description.contains("stud"))
            .unwrap();
        assert_eq!((studs.unit, studs.qty), ("ea", 8.0));
        let plate = r
            .lines
            .iter()
            .find(|l| l.size == "2x4" && l.unit == "lf")
            .unwrap();
        assert_eq!(plate.qty, 32.0);
        let header_lf = r
            .lines
            .iter()
            .find(|l| l.size == "2x6" && l.unit == "lf")
            .unwrap();
        assert_eq!(header_lf.qty, 10.0);
        let r = report(&auto_inputs(&ms, &c, 0), &d, true);
        let headers: Vec<_> = r.lines.iter().filter(|l| l.size == "2x6").collect();
        assert_eq!(headers.len(), 2);
        assert!(headers.iter().all(|l| l.unit == "ea"));
    }

    #[test]
    fn the_set_keeps_one_mixed_and_refuses_to_delete_the_last_or_a_used_default() {
        let mut s = ReportingSet::default();
        s.add("Cuts", ReportMethod::CutList, None).unwrap();
        s.add("Legacy", ReportMethod::Mixed, None).unwrap();
        assert_eq!(
            s.add("Legacy 2", ReportMethod::Mixed, None),
            Err(ReportingError::SecondMixed)
        );
        assert_eq!(
            s.add("Cuts", ReportMethod::CutList, None),
            Err(ReportingError::BadName)
        );
        assert_eq!(
            s.add("cuts", ReportMethod::CutList, None),
            Ok(()),
            "names are case-sensitive"
        );
        s.rename("Cuts", "Job cuts").unwrap();
        s.set_active("Job cuts");
        s.rename("Job cuts", "Cutting").unwrap();
        assert_eq!(s.active, "Cutting");
        s.used_by_lists.push("Legacy".into());
        assert_eq!(s.delete("Legacy"), Err(ReportingError::InUse));
        s.copy_convert("Buy List", "Linear", ReportMethod::LinearLength)
            .unwrap();
        let lin = s.get("Linear").unwrap();
        assert!(lin.boards.iter().all(|b| b.length == 0.0));
        s.delete("cuts").unwrap();
        s.delete("Cutting").unwrap();
        assert_eq!(s.active, "Buy List");
        s.delete("Linear").unwrap();
        let mut one = ReportingSet::default();
        assert_eq!(one.delete("Buy List"), Err(ReportingError::Last));
    }

    #[test]
    fn manual_members_report_each_ply_with_their_category_and_treated_flag() {
        let c = FramingCatalog::default();
        let mut beam = FramingMember::new(
            1,
            ManualKind::FloorCeilingBeam,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
        );
        beam.plies = 3;
        beam.treated = true;
        let post = FramingMember::new(
            2,
            ManualKind::Post,
            Point::new(0.0, 0.0),
            Point::new(0.0, 0.0),
        );
        let line = FramingMember::new(
            3,
            ManualKind::BearingLine,
            Point::new(0.0, 0.0),
            Point::new(50.0, 0.0),
        );
        let inputs = manual_inputs(&[beam, post, line], &c, 0);
        assert_eq!(
            inputs.len(),
            4,
            "3 plies + 1 post; the bearing line is layout"
        );
        assert!(inputs.iter().filter(|i| i.treated).count() == 3);
        assert_eq!(inputs[0].category, MaterialsCategory::Subfloor);
        assert_eq!(inputs[3].category, MaterialsCategory::Framing);
    }

    #[test]
    fn a_default_exports_and_imports_as_json_under_a_free_name() {
        let d = ReportingDefault::buy_list("Lumber yard");
        let text = d.to_json();
        let back = ReportingDefault::from_json(&text).unwrap();
        assert_eq!(back, d);
        assert!(ReportingDefault::from_json("{not json").is_none());
        let mut s = ReportingSet::default();
        assert_eq!(s.import(back.clone()).unwrap(), "Lumber yard");
        assert_eq!(s.import(back).unwrap(), "Lumber yard 2");
        let mut mixed = d.clone();
        mixed.method = ReportMethod::Mixed;
        mixed.name = "M".into();
        s.import(mixed.clone()).unwrap();
        assert_eq!(s.import(mixed), Err(ReportingError::SecondMixed));
    }

    #[test]
    fn units_format_lengths() {
        assert_eq!(ReportUnits::Feet.format(96.0), "8'");
        assert_eq!(ReportUnits::Feet.format(102.0), "8'-6\"");
        assert_eq!(ReportUnits::Inches.format(92.625), "92 5/8\"");
        assert_eq!(ReportUnits::Millimeters.format(96.0), "2438 mm");
        assert_eq!(ReportUnits::Meters.format(96.0), "2.44 m");
        assert!((ReportUnits::Feet.to_inches(8.0) - 96.0).abs() < 1e-9);
    }

    #[test]
    fn board_spec_names_standard_sizes_and_falls_back_to_actual() {
        assert_eq!(BoardSpec::new(1.5, 9.25, 12.0).size_name(), "2x10");
        assert_eq!(
            BoardSpec::new(1.75, 11.875, 24.0).size_name(),
            "1 3/4x11 7/8"
        );
    }
}
