//! Construction Lines and the Reference Display's table of rows (manual pp.
//! 83-91; CAD-62..CAD-67, LAY-39..LAY-45).
//!
//! # Construction lines
//!
//! A construction line is a CAD line (a [`CadItem::Line`] on the
//! "Construction Lines" layer, so it selects, moves, copies and deletes like
//! any CAD object) plus a [`ConstructionLine`] record kept in
//! [`Floor::construction`], keyed by the CAD object's id. The record holds
//! what a plain line cannot: infinite in plan and elevation, shown on every
//! floor, taking part in automatic ordering, the callouts and their style.
//! A record whose CAD object is gone is ignored ([`Floor::construction_line`]
//! looks the object up) and pruned by [`Floor::prune_construction`].
//!
//! Automatic ordering ([`order_labels`]) numbers the lines of one angle by
//! their position with the plan's rule sets ([`RuleSet`], Construction Line
//! Order Management); collinear lines share a number. New lines start from
//! the plan's defaults ([`ConstructionSettings::defaults`]).
//!
//! # Reference rows
//!
//! The Change Floor/Reference dialog's table: reference rows in draw order
//! (front first) with the Current line somewhere among them
//! ([`ReferenceTable`]). A row names a floor of this plan or of another plan
//! file, the layer set it draws with, whether fill patterns show, an
//! X/Y/Z offset and an angle for the other plan ([`ReferenceRow::to_world`]).

use crate::cad::{CadItem, CadObject};
use crate::geometry::Point;
use crate::layers::{Layer, LineStyle};
use crate::model::{Floor, Id, Project};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The layer construction lines are drawn on.
pub const LAYER: &str = "Construction Lines";
/// The drawing group a new construction line is put in (manual p. 84).
pub const DEFAULT_GROUP: i32 = 21;
/// Half the length of the segment an infinite line is stood for by (inches).
/// A snap segment longer than [`LONG_SEGMENT`] is an infinite line.
pub const INFINITE_REACH: f64 = 1.0e7;
/// Segments longer than this (inches) stand for infinite lines.
pub const LONG_SEGMENT: f64 = 1.0e6;
/// Two lines whose angles differ by less than this many degrees are parallel
/// for ordering.
pub const ANGLE_TOLERANCE_DEG: f64 = 0.5;
/// Two lines closer than this (inches) are collinear for ordering.
pub const COLLINEAR_TOLERANCE: f64 = 0.25;

// ---------------------------------------------------------------------------
// Callouts
// ---------------------------------------------------------------------------

/// Which ends of a construction line carry a callout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CalloutEnds {
    Both,
    Start,
    End,
    #[default]
    None,
}

impl CalloutEnds {
    pub const ALL: [CalloutEnds; 4] = [
        CalloutEnds::Both,
        CalloutEnds::Start,
        CalloutEnds::End,
        CalloutEnds::None,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CalloutEnds::Both => "Both",
            CalloutEnds::Start => "Start",
            CalloutEnds::End => "End",
            CalloutEnds::None => "None",
        }
    }

    pub fn at_start(self) -> bool {
        matches!(self, CalloutEnds::Both | CalloutEnds::Start)
    }

    pub fn at_end(self) -> bool {
        matches!(self, CalloutEnds::Both | CalloutEnds::End)
    }
}

/// The outline of a construction line callout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CalloutShape {
    #[default]
    Circle,
    Ellipse,
    Square,
    Rectangle,
    Capsule,
    Diamond,
    Hexagon,
    Octagon,
}

impl CalloutShape {
    pub const ALL: [CalloutShape; 8] = [
        CalloutShape::Circle,
        CalloutShape::Ellipse,
        CalloutShape::Square,
        CalloutShape::Rectangle,
        CalloutShape::Capsule,
        CalloutShape::Diamond,
        CalloutShape::Hexagon,
        CalloutShape::Octagon,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CalloutShape::Circle => "Circle",
            CalloutShape::Ellipse => "Ellipse",
            CalloutShape::Square => "Square",
            CalloutShape::Rectangle => "Rectangle",
            CalloutShape::Capsule => "Capsule",
            CalloutShape::Diamond => "Diamond",
            CalloutShape::Hexagon => "Hexagon",
            CalloutShape::Octagon => "Octagon",
        }
    }

    /// The Angle box is not available for these (manual p. 88).
    pub fn has_angle(self) -> bool {
        !matches!(
            self,
            CalloutShape::Ellipse | CalloutShape::Capsule | CalloutShape::Rectangle
        )
    }
}

/// The Callouts panel of the Construction Line Specification (manual pp.
/// 87-88).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CalloutSpec {
    /// Display Callout on, in plan views.
    pub plan: CalloutEnds,
    /// Display Callout on, in cross section/elevation views.
    pub elevation: CalloutEnds,
    /// The text inside the callout ("Text Above Line").
    pub label: String,
    /// "Text Below Line": the bottom row, separated by a line.
    pub text_below: String,
    /// Automatic beside the label: report the ordering number or letter.
    pub label_auto: bool,
    /// Automatic beside the text below.
    pub below_auto: bool,
    pub shape: CalloutShape,
    pub filled: bool,
    /// Use the color of the layer for the fill.
    pub fill_by_layer: bool,
    pub fill_color: [u8; 3],
    /// 0 opaque to 100 clear (percent).
    pub transparency: u8,
    /// Automatic size: the callout encompasses its label.
    pub auto_size: bool,
    /// Size when not automatic (inches).
    pub size: f64,
    /// Angle of the shape, degrees.
    pub angle_deg: f64,
    /// Custom Callout Line Options.
    pub custom_outline: bool,
    pub outline_color: [u8; 3],
    pub outline_color_by_layer: bool,
    pub outline_style: LineStyle,
    pub outline_style_by_layer: bool,
    /// Hundredths of a millimetre.
    pub outline_weight: u32,
    pub outline_weight_by_layer: bool,
}

impl Default for CalloutSpec {
    fn default() -> Self {
        Self {
            plan: CalloutEnds::None,
            elevation: CalloutEnds::None,
            label: String::new(),
            text_below: String::new(),
            label_auto: true,
            below_auto: false,
            shape: CalloutShape::Circle,
            filled: false,
            fill_by_layer: true,
            fill_color: [255, 255, 255],
            transparency: 0,
            auto_size: true,
            size: 6.0,
            angle_deg: 0.0,
            custom_outline: false,
            outline_color: [0, 0, 0],
            outline_color_by_layer: true,
            outline_style: LineStyle::Solid,
            outline_style_by_layer: true,
            outline_weight: 25,
            outline_weight_by_layer: true,
        }
    }
}

impl CalloutSpec {
    /// The two rows of the callout text: the label (the ordering label when
    /// Automatic is on and the line has one) and the text below the line.
    pub fn texts(&self, auto: Option<&str>) -> (String, String) {
        let pick = |auto_on: bool, manual: &str| {
            if auto_on {
                auto.unwrap_or("").to_string()
            } else {
                manual.to_string()
            }
        };
        (
            pick(self.label_auto, &self.label),
            pick(self.below_auto, &self.text_below),
        )
    }

    /// The ends that carry a callout in `view`.
    pub fn ends(&self, view: ViewType) -> CalloutEnds {
        match view {
            ViewType::Plan => self.plan,
            ViewType::Elevation => self.elevation,
        }
    }
}

// ---------------------------------------------------------------------------
// The record of one construction line
// ---------------------------------------------------------------------------

/// The settings of one construction line (the Construction Line, Callouts,
/// Line Style and Text Style panels). `id` is the CAD line's id; the
/// plan-wide defaults ([`ConstructionSettings::defaults`]) carry id 0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConstructionLine {
    pub id: Id,
    /// Draw Infinite Line in Plan View.
    pub infinite_plan: bool,
    /// Draw Infinite Line in Elevation View.
    pub infinite_elevation: bool,
    /// Display on All Floors in Plan View.
    pub all_floors: bool,
    /// Include in Automatic Ordering.
    pub in_ordering: bool,
    pub callouts: CalloutSpec,
    /// Line Style panel: the color, style and weight of the line itself;
    /// `None` takes the layer's (By Layer).
    pub color: Option<[u8; 3]>,
    pub line_style: Option<LineStyle>,
    pub line_weight: Option<u32>,
    /// Text Style panel: the text style of the callouts; empty is the
    /// layer's.
    pub text_style: String,
}

impl Default for ConstructionLine {
    fn default() -> Self {
        Self {
            id: 0,
            infinite_plan: true,
            infinite_elevation: true,
            all_floors: false,
            in_ordering: true,
            callouts: CalloutSpec::default(),
            color: None,
            line_style: None,
            line_weight: None,
            text_style: String::new(),
        }
    }
}

impl ConstructionLine {
    /// The default record made over for the CAD line `id`.
    pub fn for_line(&self, id: Id) -> ConstructionLine {
        ConstructionLine {
            id,
            ..self.clone()
        }
    }

    /// Is the line drawn infinite in `view`?
    pub fn infinite_in(&self, view: ViewType) -> bool {
        match view {
            ViewType::Plan => self.infinite_plan,
            ViewType::Elevation => self.infinite_elevation,
        }
    }
}

/// The construction line records of a floor.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConstructionLayer {
    pub lines: Vec<ConstructionLine>,
}

impl ConstructionLayer {
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn get(&self, id: Id) -> Option<&ConstructionLine> {
        self.lines.iter().find(|l| l.id == id)
    }

    pub fn get_mut(&mut self, id: Id) -> Option<&mut ConstructionLine> {
        self.lines.iter_mut().find(|l| l.id == id)
    }

    pub fn contains(&self, id: Id) -> bool {
        self.get(id).is_some()
    }

    /// Sets (adds or replaces) the record of `line.id`.
    pub fn set(&mut self, line: ConstructionLine) {
        match self.get_mut(line.id) {
            Some(slot) => *slot = line,
            None => self.lines.push(line),
        }
    }
}

/// A construction line with its geometry, ready to draw, snap and order.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedLine {
    /// The floor the line was drawn on.
    pub floor: usize,
    pub id: Id,
    pub a: Point,
    pub b: Point,
    /// The layer of the CAD line.
    pub layer: String,
    pub spec: ConstructionLine,
}

impl ResolvedLine {
    pub fn angle_deg(&self) -> f64 {
        line_angle_deg(self.a, self.b)
    }
}

impl Floor {
    /// The record of the construction line `id`, when the CAD line exists.
    pub fn construction_line(&self, id: Id) -> Option<&ConstructionLine> {
        self.construction
            .get(id)
            .filter(|_| self.cad.iter().any(|c| c.id == id))
    }

    /// Is the CAD object `id` a construction line?
    pub fn is_construction_line(&self, id: Id) -> bool {
        self.construction_line(id).is_some()
    }

    /// The ids of the construction lines of this floor (those with a CAD line).
    pub fn construction_ids(&self) -> std::collections::HashSet<Id> {
        self.construction
            .lines
            .iter()
            .map(|l| l.id)
            .filter(|id| self.cad.iter().any(|c| c.id == *id))
            .collect()
    }

    /// Forgets the records whose CAD line is gone. Returns how many.
    pub fn prune_construction(&mut self) -> usize {
        let before = self.construction.lines.len();
        let ids: std::collections::HashSet<Id> = self.cad.iter().map(|c| c.id).collect();
        self.construction.lines.retain(|l| ids.contains(&l.id));
        before - self.construction.lines.len()
    }

    /// The construction lines drawn on this floor with their geometry.
    pub fn resolved_construction(&self, floor_index: usize) -> Vec<ResolvedLine> {
        self.construction
            .lines
            .iter()
            .filter_map(|rec| {
                let c = self.cad.iter().find(|c| c.id == rec.id)?;
                let CadItem::Line { a, b } = c.item else {
                    return None;
                };
                Some(ResolvedLine {
                    floor: floor_index,
                    id: c.id,
                    a,
                    b,
                    layer: c.layer.clone(),
                    spec: rec.clone(),
                })
            })
            .collect()
    }
}

impl Project {
    /// The construction lines shown on floor `floor`: the ones drawn there
    /// and the ones of other floors set to Display on All Floors in Plan
    /// View.
    pub fn construction_lines_on(&self, floor: usize) -> Vec<ResolvedLine> {
        let mut out = Vec::new();
        for (i, f) in self.floors.iter().enumerate() {
            if f.construction.is_empty() {
                continue;
            }
            for line in f.resolved_construction(i) {
                if i == floor || line.spec.all_floors {
                    out.push(line);
                }
            }
        }
        out
    }

    /// Makes sure the "Construction Lines" layer exists. Returns true when
    /// it was added.
    pub fn ensure_construction_layer(&mut self) -> bool {
        if self.layers.get(LAYER).is_some() {
            return false;
        }
        let mut layer = Layer::new(LAYER, [0, 110, 200], 13);
        layer.line_style = LineStyle::Dashed;
        self.layers.add(layer);
        true
    }

    /// Adds a construction line from `a` to `b` on floor `floor`, on `layer`
    /// (the Construction Lines layer when `None`), from the plan's defaults,
    /// in drawing group [`DEFAULT_GROUP`]. Returns the id of the CAD line.
    pub fn add_construction_line(
        &mut self,
        floor: usize,
        a: Point,
        b: Point,
        layer: Option<&str>,
    ) -> Option<Id> {
        if floor >= self.floors.len() {
            return None;
        }
        let layer_name = match layer {
            Some(l) => l.to_string(),
            None => {
                self.ensure_construction_layer();
                LAYER.to_string()
            }
        };
        let id = self.alloc_id();
        let rec = self.construction.defaults.for_line(id);
        let f = &mut self.floors[floor];
        f.cad.push(CadObject {
            id,
            layer: layer_name,
            item: CadItem::Line { a, b },
        });
        f.construction.set(rec);
        f.set_drawing_group(crate::groups::ObjectRef::Cad(id), Some(DEFAULT_GROUP));
        Some(id)
    }

    /// Converts the CAD polyline `id` of `floor` into construction lines,
    /// one per segment (the first keeps the polyline's id). Returns the ids
    /// made; empty when `id` is not an open or closed polyline of 2+ points.
    pub fn polyline_to_construction(&mut self, floor: usize, id: Id) -> Vec<Id> {
        let Some(f) = self.floors.get(floor) else {
            return Vec::new();
        };
        let Some(CadObject {
            item: CadItem::Polyline { points, closed },
            layer,
            ..
        }) = f.cad.iter().find(|c| c.id == id).cloned()
        else {
            return Vec::new();
        };
        let mut segs: Vec<(Point, Point)> = points.windows(2).map(|w| (w[0], w[1])).collect();
        if closed && points.len() > 2 {
            segs.push((points[points.len() - 1], points[0]));
        }
        if segs.is_empty() {
            return Vec::new();
        }
        let layer = if layer == crate::cad::DEFAULT_CAD_LAYER {
            self.ensure_construction_layer();
            LAYER.to_string()
        } else {
            layer
        };
        let mut ids = vec![id];
        for _ in 1..segs.len() {
            ids.push(self.alloc_id());
        }
        let recs: Vec<ConstructionLine> = ids
            .iter()
            .map(|i| self.construction.defaults.for_line(*i))
            .collect();
        let f = &mut self.floors[floor];
        if let Some(slot) = f.cad.iter_mut().find(|c| c.id == id) {
            slot.item = CadItem::Line {
                a: segs[0].0,
                b: segs[0].1,
            };
            slot.layer = layer.clone();
        }
        for (i, (s, rec)) in segs.iter().zip(recs).enumerate() {
            if i > 0 {
                f.cad.push(CadObject {
                    id: ids[i],
                    layer: layer.clone(),
                    item: CadItem::Line { a: s.0, b: s.1 },
                });
            }
            f.construction.set(rec);
            f.set_drawing_group(crate::groups::ObjectRef::Cad(ids[i]), Some(DEFAULT_GROUP));
        }
        ids
    }

    /// Converts the construction line `id` of `floor` into a plain CAD
    /// polyline of two points (its record and drawing group are dropped).
    pub fn construction_to_polyline(&mut self, floor: usize, id: Id) -> bool {
        let Some(f) = self.floors.get_mut(floor) else {
            return false;
        };
        if !f.is_construction_line(id) {
            return false;
        }
        let Some(c) = f.cad.iter_mut().find(|c| c.id == id) else {
            return false;
        };
        let CadItem::Line { a, b } = c.item else {
            return false;
        };
        c.item = CadItem::Polyline {
            points: vec![a, b],
            closed: false,
        };
        if c.layer == LAYER {
            c.layer = crate::cad::DEFAULT_CAD_LAYER.to_string();
        }
        f.construction.lines.retain(|l| l.id != id);
        f.set_drawing_group(crate::groups::ObjectRef::Cad(id), None);
        true
    }
}

// ---------------------------------------------------------------------------
// Plan-wide settings: defaults and the order rule sets
// ---------------------------------------------------------------------------

/// Which kind of view a rule set applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ViewType {
    #[default]
    Plan,
    Elevation,
}

impl ViewType {
    pub fn label(self) -> &'static str {
        match self {
            ViewType::Plan => "Plan View",
            ViewType::Elevation => "Elevation View",
        }
    }
}

/// The numbering convention of an order rule set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CountFormat {
    /// 1, 2, 3
    #[default]
    Numbers,
    /// 01, 02, 03
    PaddedNumbers,
    /// A, B, C ... Z, AA
    UpperLetters,
    /// a, b, c ... z, aa
    LowerLetters,
    /// I, II, III
    UpperRoman,
    /// i, ii, iii
    LowerRoman,
}

impl CountFormat {
    pub const ALL: [CountFormat; 6] = [
        CountFormat::Numbers,
        CountFormat::PaddedNumbers,
        CountFormat::UpperLetters,
        CountFormat::LowerLetters,
        CountFormat::UpperRoman,
        CountFormat::LowerRoman,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CountFormat::Numbers => "1, 2, 3",
            CountFormat::PaddedNumbers => "01, 02, 03",
            CountFormat::UpperLetters => "A, B, C",
            CountFormat::LowerLetters => "a, b, c",
            CountFormat::UpperRoman => "I, II, III",
            CountFormat::LowerRoman => "i, ii, iii",
        }
    }

    /// The label of the `n`th line (1-based).
    pub fn format(self, n: usize) -> String {
        match self {
            CountFormat::Numbers => n.to_string(),
            CountFormat::PaddedNumbers => format!("{n:02}"),
            CountFormat::UpperLetters => letters(n, b'A'),
            CountFormat::LowerLetters => letters(n, b'a'),
            CountFormat::UpperRoman => roman(n),
            CountFormat::LowerRoman => roman(n).to_lowercase(),
        }
    }
}

/// 1 -> A, 26 -> Z, 27 -> AA.
fn letters(mut n: usize, base: u8) -> String {
    let mut out = Vec::new();
    while n > 0 {
        n -= 1;
        out.push(base + (n % 26) as u8);
        n /= 26;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

fn roman(mut n: usize) -> String {
    const T: [(usize, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for (v, s) in T {
        while n >= v {
            out.push_str(s);
            n -= v;
        }
    }
    out
}

/// One rule set of Construction Line Order Management.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RuleSet {
    pub name: String,
    /// The check box beside the name: the set can be used in applicable
    /// views.
    pub enabled: bool,
    /// System rule sets cannot be deleted or renamed, and their view type
    /// and angle cannot be changed.
    pub system: bool,
    pub view: ViewType,
    /// The angle (degrees, direction-agnostic) a line must be drawn at.
    pub angle_deg: f64,
    pub format: CountFormat,
    /// Number from right to left and down to up instead of left to right
    /// and up to down.
    pub reverse: bool,
}

impl Default for RuleSet {
    fn default() -> Self {
        Self {
            name: "New Rule Set".into(),
            enabled: true,
            system: false,
            view: ViewType::Plan,
            angle_deg: 0.0,
            format: CountFormat::Numbers,
            reverse: false,
        }
    }
}

impl RuleSet {
    /// Does a line at `angle_deg` (any direction) fall under this set?
    pub fn matches_angle(&self, angle_deg: f64) -> bool {
        angle_diff(self.angle_deg, angle_deg) <= ANGLE_TOLERANCE_DEG
    }
}

/// The plan's Construction Line Defaults and order rule sets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConstructionSettings {
    /// What a new construction line starts as (Construction Line Defaults,
    /// "Set as Default").
    pub defaults: ConstructionLine,
    /// The rule sets, highest priority first.
    pub rule_sets: Vec<RuleSet>,
}

impl Default for ConstructionSettings {
    fn default() -> Self {
        let sys = |name: &str, view, angle_deg, format| RuleSet {
            name: name.into(),
            enabled: true,
            system: true,
            view,
            angle_deg,
            format,
            reverse: false,
        };
        Self {
            defaults: ConstructionLine::default(),
            rule_sets: vec![
                sys("Plan Vertical", ViewType::Plan, 90.0, CountFormat::Numbers),
                sys(
                    "Plan Horizontal",
                    ViewType::Plan,
                    0.0,
                    CountFormat::UpperLetters,
                ),
                sys(
                    "Elevation Vertical",
                    ViewType::Elevation,
                    90.0,
                    CountFormat::Numbers,
                ),
                sys(
                    "Elevation Horizontal",
                    ViewType::Elevation,
                    0.0,
                    CountFormat::UpperLetters,
                ),
            ],
        }
    }
}

impl ConstructionSettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// A name for a new rule set that no other set has.
    pub fn unique_name(&self, base: &str) -> String {
        if !self.rule_sets.iter().any(|r| r.name == base) {
            return base.to_string();
        }
        (2..)
            .map(|n| format!("{base} {n}"))
            .find(|n| !self.rule_sets.iter().any(|r| &r.name == n))
            .unwrap_or_else(|| base.to_string())
    }

    /// Adds a new rule set below the others. Returns its index.
    pub fn add_rule_set(&mut self) -> usize {
        let name = self.unique_name("New Rule Set");
        self.rule_sets.push(RuleSet {
            name,
            ..RuleSet::default()
        });
        self.rule_sets.len() - 1
    }

    /// Copies rule set `i` right below it. Returns the copy's index.
    pub fn copy_rule_set(&mut self, i: usize) -> Option<usize> {
        let src = self.rule_sets.get(i)?.clone();
        let copy = RuleSet {
            name: self.unique_name(&format!("{} Copy", src.name)),
            system: false,
            ..src
        };
        self.rule_sets.insert(i + 1, copy);
        Some(i + 1)
    }

    /// Deletes rule set `i`; system rule sets stay.
    pub fn delete_rule_set(&mut self, i: usize) -> bool {
        match self.rule_sets.get(i) {
            Some(r) if !r.system => {
                self.rule_sets.remove(i);
                true
            }
            _ => false,
        }
    }

    /// Increase Priority: moves rule set `i` up one place.
    pub fn raise_priority(&mut self, i: usize) -> Option<usize> {
        if i == 0 || i >= self.rule_sets.len() {
            return None;
        }
        self.rule_sets.swap(i, i - 1);
        Some(i - 1)
    }

    /// Decrease Priority: moves rule set `i` down one place.
    pub fn lower_priority(&mut self, i: usize) -> Option<usize> {
        if i + 1 >= self.rule_sets.len() {
            return None;
        }
        self.rule_sets.swap(i, i + 1);
        Some(i + 1)
    }

    /// Rule set names must be unique and not empty.
    pub fn name_error(&self, i: usize) -> Option<String> {
        let r = self.rule_sets.get(i)?;
        if r.name.trim().is_empty() {
            return Some("A rule set needs a name".into());
        }
        if self
            .rule_sets
            .iter()
            .enumerate()
            .any(|(j, o)| j != i && o.name == r.name)
        {
            return Some(format!("Another rule set is named \"{}\"", r.name));
        }
        None
    }

    /// The first (highest priority) enabled rule set of `view` that takes
    /// lines at `angle_deg`.
    pub fn rule_for(&self, view: ViewType, angle_deg: f64) -> Option<usize> {
        self.rule_sets
            .iter()
            .position(|r| r.enabled && r.view == view && r.matches_angle(angle_deg))
    }
}

// ---------------------------------------------------------------------------
// Line geometry
// ---------------------------------------------------------------------------

/// The angle of the line a-b in degrees, `0 <= angle < 180` (a line has no
/// direction here).
pub fn line_angle_deg(a: Point, b: Point) -> f64 {
    let d = b.sub(a);
    if d.length() < 1e-9 {
        return 0.0;
    }
    let deg = d.angle().to_degrees().rem_euclid(180.0);
    if deg > 179.999_999 {
        0.0
    } else {
        deg
    }
}

/// The difference between two line angles, `0..=90` degrees.
pub fn angle_diff(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(180.0);
    d.min(180.0 - d)
}

/// The point where the infinite lines a1-a2 and b1-b2 cross; `None` when
/// they are parallel (or a line has no length).
pub fn intersect_lines(a1: Point, a2: Point, b1: Point, b2: Point) -> Option<Point> {
    let r = a2.sub(a1);
    let s = b2.sub(b1);
    let denom = r.cross(s);
    if r.length() < 1e-9 || s.length() < 1e-9 {
        return None;
    }
    // Parallel: the sine of the angle between them is ~0.
    if denom.abs() < 1e-9 * r.length() * s.length() {
        return None;
    }
    let t = b1.sub(a1).cross(s) / denom;
    Some(a1.add(r.scale(t)))
}

/// The point of the infinite line a-b nearest `p`.
pub fn nearest_on_line(p: Point, a: Point, b: Point) -> Point {
    let d = b.sub(a);
    let len2 = d.dot(d);
    if len2 < 1e-18 {
        return a;
    }
    let t = p.sub(a).dot(d) / len2;
    a.add(d.scale(t))
}

/// Distance from `p` to the infinite line a-b.
pub fn dist_to_line(p: Point, a: Point, b: Point) -> f64 {
    p.dist(nearest_on_line(p, a, b))
}

/// Are the infinite lines parallel (within [`ANGLE_TOLERANCE_DEG`])?
pub fn is_parallel(a1: Point, a2: Point, b1: Point, b2: Point) -> bool {
    angle_diff(line_angle_deg(a1, a2), line_angle_deg(b1, b2)) <= ANGLE_TOLERANCE_DEG
}

/// Are the infinite lines perpendicular (within [`ANGLE_TOLERANCE_DEG`])?
pub fn is_perpendicular(a1: Point, a2: Point, b1: Point, b2: Point) -> bool {
    (angle_diff(line_angle_deg(a1, a2), line_angle_deg(b1, b2)) - 90.0).abs()
        <= ANGLE_TOLERANCE_DEG
}

/// The segment that stands for the infinite line a-b: very long, centered on
/// the drawn middle. Snaps and the overlay treat segments longer than
/// [`LONG_SEGMENT`] as infinite lines.
pub fn infinite_segment(a: Point, b: Point) -> (Point, Point) {
    let d = b.sub(a);
    let u = if d.length() < 1e-9 {
        Point::new(1.0, 0.0)
    } else {
        d.normalized()
    };
    let mid = Point::lerp(a, b, 0.5);
    (
        mid.sub(u.scale(INFINITE_REACH)),
        mid.add(u.scale(INFINITE_REACH)),
    )
}

/// Does this snap segment stand for an infinite line?
pub fn is_infinite_segment(a: Point, b: Point) -> bool {
    a.dist(b) > LONG_SEGMENT
}

/// The part of the infinite line a-b inside the rectangle `lo`..`hi`, as the
/// points where it enters (towards the start, `t -> -inf`) and leaves
/// (towards the end). `None` when the line misses the rectangle.
pub fn clip_line_to_rect(a: Point, b: Point, lo: Point, hi: Point) -> Option<(Point, Point)> {
    let d = b.sub(a);
    if d.length() < 1e-9 {
        return None;
    }
    let (mut t0, mut t1) = (f64::NEG_INFINITY, f64::INFINITY);
    for (p, q, l, h) in [(a.x, d.x, lo.x, hi.x), (a.y, d.y, lo.y, hi.y)] {
        if q.abs() < 1e-12 {
            if p < l || p > h {
                return None;
            }
        } else {
            let (mut ta, mut tb) = ((l - p) / q, (h - p) / q);
            if ta > tb {
                std::mem::swap(&mut ta, &mut tb);
            }
            t0 = t0.max(ta);
            t1 = t1.min(tb);
        }
    }
    (t0 <= t1).then(|| (a.add(d.scale(t0)), a.add(d.scale(t1))))
}

// ---------------------------------------------------------------------------
// Automatic ordering
// ---------------------------------------------------------------------------

/// A line to number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrderLine {
    pub id: Id,
    pub a: Point,
    pub b: Point,
}

/// The automatic-ordering label of each line (manual pp. 84-85): the lines
/// under one rule set are numbered by position, from left to right and up to
/// down (or the other way round with Reverse Order Direction), and collinear
/// lines share a number. Lines no enabled rule set of `view` takes get no
/// label.
pub fn order_labels(lines: &[OrderLine], view: ViewType, sets: &[RuleSet]) -> HashMap<Id, String> {
    let settings = ConstructionSettings {
        defaults: ConstructionLine::default(),
        rule_sets: sets.to_vec(),
    };
    let mut groups: HashMap<usize, Vec<(Id, f64)>> = HashMap::new();
    for l in lines {
        let angle = line_angle_deg(l.a, l.b);
        let Some(rule) = settings.rule_for(view, angle) else {
            continue;
        };
        // The key runs left to right for upright lines and up to down for
        // flat ones: the position along the line's right-hand normal.
        let rad = angle.to_radians();
        let normal = Point::new(rad.sin(), -rad.cos());
        let mid = Point::lerp(l.a, l.b, 0.5);
        groups.entry(rule).or_default().push((l.id, mid.dot(normal)));
    }
    let mut out = HashMap::new();
    for (rule, mut members) in groups {
        let set = &sets[rule];
        members.sort_by(|x, y| x.1.total_cmp(&y.1).then(x.0.cmp(&y.0)));
        if set.reverse {
            members.reverse();
        }
        let mut rank = 0usize;
        let mut last: Option<f64> = None;
        for (id, key) in members {
            if last.is_none_or(|k| (k - key).abs() > COLLINEAR_TOLERANCE) {
                rank += 1;
                last = Some(key);
            }
            out.insert(id, set.format.format(rank));
        }
    }
    out
}

impl Project {
    /// The ordering labels of the construction lines shown on `floor` in
    /// `view`, for the lines that take part in automatic ordering.
    pub fn construction_order(&self, floor: usize, view: ViewType) -> HashMap<Id, String> {
        let lines: Vec<OrderLine> = self
            .construction_lines_on(floor)
            .into_iter()
            .filter(|l| l.spec.in_ordering)
            .map(|l| OrderLine {
                id: l.id,
                a: l.a,
                b: l.b,
            })
            .collect();
        order_labels(&lines, view, &self.construction.rule_sets)
    }
}

// ---------------------------------------------------------------------------
// Reference rows
// ---------------------------------------------------------------------------

/// Which plan a reference row shows.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ReferenceSource {
    /// The plan being edited.
    #[default]
    ThisPlan,
    /// Another plan file (read-only; the path as chosen).
    File(String),
}

impl ReferenceSource {
    pub fn is_this_plan(&self) -> bool {
        matches!(self, ReferenceSource::ThisPlan)
    }

    pub fn label(&self) -> String {
        match self {
            ReferenceSource::ThisPlan => "Current Plan".into(),
            ReferenceSource::File(p) => std::path::Path::new(p)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.clone()),
        }
    }
}

/// Which floor a reference row shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RowFloor {
    /// The floor below the current one (nothing under the lowest floor).
    #[default]
    Below,
    /// The floor above the current one.
    Above,
    /// The floor below; the floor above when there is none (the manual's
    /// "Automatic").
    Automatic,
    /// A fixed floor, by index.
    Fixed(usize),
    /// The same floor level as the current plan (for another plan file).
    MatchCurrent,
}

impl RowFloor {
    /// The floor index this row shows while `current` is the active floor
    /// of a plan with `count` floors; `other_plan` says the row refers to
    /// another plan (which may show the same level). `count` is the number
    /// of floors of the plan the row shows.
    pub fn resolve(self, current: usize, count: usize, other_plan: bool) -> Option<usize> {
        let idx = match self {
            RowFloor::Below => current.checked_sub(1)?,
            RowFloor::Above => current + 1,
            RowFloor::Automatic => match current.checked_sub(1) {
                Some(b) if b < count => b,
                _ => current + 1,
            },
            RowFloor::Fixed(i) => i,
            RowFloor::MatchCurrent => current,
        };
        (idx < count && (other_plan || idx != current)).then_some(idx)
    }

    pub fn label(self) -> String {
        match self {
            RowFloor::Below => "Floor Below".into(),
            RowFloor::Above => "Floor Above".into(),
            RowFloor::Automatic => "Automatic".into(),
            RowFloor::Fixed(i) => format!("Floor {}", i + 1),
            RowFloor::MatchCurrent => "Match Current".into(),
        }
    }
}

/// One reference row of the Change Floor/Reference table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReferenceRow {
    pub source: ReferenceSource,
    pub floor: RowFloor,
    /// The layer set the row draws with; `None` is the active view's layers
    /// ("Reference Display Layer Set" of the referenced plan when it has
    /// one).
    pub layer_set: Option<String>,
    /// The Details column: fill patterns show, not only edge lines.
    pub details: bool,
    /// X, Y, Z offset of another plan (inches).
    pub offset: [f64; 3],
    /// The Angle of another plan (degrees, counter-clockwise).
    pub angle_deg: f64,
    /// The color the row's linework is drawn in.
    pub color: [u8; 3],
}

impl Default for ReferenceRow {
    fn default() -> Self {
        Self {
            source: ReferenceSource::ThisPlan,
            floor: RowFloor::Automatic,
            layer_set: None,
            details: true,
            offset: [0.0; 3],
            angle_deg: 0.0,
            color: [128, 128, 128],
        }
    }
}

impl ReferenceRow {
    /// A row for another plan file: it shows the same floor level as the
    /// current plan ("Match Current").
    pub fn for_file(path: impl Into<String>) -> Self {
        Self {
            source: ReferenceSource::File(path.into()),
            floor: RowFloor::MatchCurrent,
            ..Self::default()
        }
    }

    /// Maps a point of the referenced plan into the current plan: rotated
    /// by the Angle about the other plan's origin, then moved by the X/Y
    /// offset. Rows of this plan are not moved.
    pub fn to_world(&self, p: Point) -> Point {
        if self.source.is_this_plan() {
            return p;
        }
        let (s, c) = self.angle_deg.to_radians().sin_cos();
        Point::new(
            p.x * c - p.y * s + self.offset[0],
            p.x * s + p.y * c + self.offset[1],
        )
    }

    /// The inverse of [`ReferenceRow::to_world`].
    pub fn from_world(&self, w: Point) -> Point {
        if self.source.is_this_plan() {
            return w;
        }
        let (s, c) = self.angle_deg.to_radians().sin_cos();
        let q = Point::new(w.x - self.offset[0], w.y - self.offset[1]);
        Point::new(q.x * c + q.y * s, -q.x * s + q.y * c)
    }

    /// Turns the other plan by `new_angle_deg` about the world point
    /// `pivot` (the marquee's center) so that point stays where it is:
    /// changes the offset and the angle.
    pub fn rotate_about(&mut self, pivot: Point, new_angle_deg: f64) {
        let local = self.from_world(pivot);
        self.angle_deg = new_angle_deg;
        let (s, c) = new_angle_deg.to_radians().sin_cos();
        self.offset[0] = pivot.x - (local.x * c - local.y * s);
        self.offset[1] = pivot.y - (local.x * s + local.y * c);
    }

    /// Moves the other plan by `delta` (the Move handle).
    pub fn move_by(&mut self, delta: Point) {
        self.offset[0] += delta.x;
        self.offset[1] += delta.y;
    }
}

/// A line of the Change Floor/Reference table: the Current floor or one of
/// the reference rows.
#[derive(Debug, Clone, PartialEq)]
pub enum TableLine {
    Current,
    Row(ReferenceRow),
}

/// The table of reference rows in draw order (front first) with the Current
/// line among them. Empty `rows` means the default single row (the floor
/// below, in the Reference Display's session choices).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReferenceTable {
    pub rows: Vec<ReferenceRow>,
    /// How many rows are drawn in front of the Current line.
    pub current_at: usize,
    /// XOR drawing: reference lines over identical lines of the current
    /// floor are not drawn, and the rest change color.
    pub xor: bool,
}

impl Default for ReferenceTable {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            current_at: 0,
            xor: false,
        }
    }
}

impl ReferenceTable {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The table as the dialog lists it: top (front) to bottom, the Current
    /// line included.
    pub fn lines(&self) -> Vec<TableLine> {
        let at = self.current_at.min(self.rows.len());
        let mut out: Vec<TableLine> = self.rows.iter().cloned().map(TableLine::Row).collect();
        out.insert(at, TableLine::Current);
        out
    }

    /// Rebuilds the table from its lines (exactly one Current line).
    pub fn from_lines(lines: Vec<TableLine>, xor: bool) -> ReferenceTable {
        let at = lines
            .iter()
            .position(|l| *l == TableLine::Current)
            .unwrap_or(0);
        ReferenceTable {
            rows: lines
                .into_iter()
                .filter_map(|l| match l {
                    TableLine::Row(r) => Some(r),
                    TableLine::Current => None,
                })
                .collect(),
            current_at: at,
            xor,
        }
    }

    /// Insert Above: a new row (Automatic floor, the Reference Floor Layer
    /// Set) directly above line `sel`. Returns the new line's index.
    pub fn insert_above(&mut self, sel: usize, row: ReferenceRow) -> usize {
        let mut lines = self.lines();
        let at = sel.min(lines.len());
        lines.insert(at, TableLine::Row(row));
        *self = Self::from_lines(lines, self.xor);
        at
    }

    /// Insert Below: a new row directly below line `sel`.
    pub fn insert_below(&mut self, sel: usize, row: ReferenceRow) -> usize {
        let mut lines = self.lines();
        let at = (sel + 1).min(lines.len());
        lines.insert(at, TableLine::Row(row));
        *self = Self::from_lines(lines, self.xor);
        at
    }

    /// Move Up: line `sel` trades places with the one above. Returns its
    /// new index.
    pub fn move_up(&mut self, sel: usize) -> Option<usize> {
        let mut lines = self.lines();
        if sel == 0 || sel >= lines.len() {
            return None;
        }
        lines.swap(sel, sel - 1);
        *self = Self::from_lines(lines, self.xor);
        Some(sel - 1)
    }

    /// Move Down.
    pub fn move_down(&mut self, sel: usize) -> Option<usize> {
        let mut lines = self.lines();
        if sel + 1 >= lines.len() {
            return None;
        }
        lines.swap(sel, sel + 1);
        *self = Self::from_lines(lines, self.xor);
        Some(sel + 1)
    }

    /// Can line `sel` be deleted? Not the Current line, and not the only
    /// reference row.
    pub fn can_delete(&self, sel: usize) -> bool {
        matches!(self.lines().get(sel), Some(TableLine::Row(_))) && self.rows.len() > 1
    }

    /// Delete: removes line `sel` when allowed.
    pub fn delete(&mut self, sel: usize) -> bool {
        if !self.can_delete(sel) {
            return false;
        }
        let mut lines = self.lines();
        lines.remove(sel);
        *self = Self::from_lines(lines, self.xor);
        true
    }

    /// The reference rows other than the Current line.
    pub fn reference_rows(&self) -> &[ReferenceRow] {
        &self.rows
    }

    /// Edit Reference Document Offset and Swap Floor/Reference: the rows
    /// that refer to another plan file.
    pub fn file_rows(&self) -> Vec<usize> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, r)| !r.source.is_this_plan())
            .map(|(i, _)| i)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    // ----- line geometry -----

    #[test]
    fn two_infinite_lines_cross_beyond_their_drawn_ends() {
        // Drawn short, they still meet at (100, 50).
        let x = intersect_lines(p(0.0, 50.0), p(10.0, 50.0), p(100.0, 0.0), p(100.0, 10.0));
        assert_eq!(x, Some(p(100.0, 50.0)));
        // Slanted against upright.
        let x = intersect_lines(p(0.0, 0.0), p(10.0, 10.0), p(30.0, 0.0), p(30.0, 5.0)).unwrap();
        assert!(x.dist(p(30.0, 30.0)) < 1e-9);
    }

    #[test]
    fn parallel_lines_never_meet() {
        assert_eq!(
            intersect_lines(p(0.0, 0.0), p(10.0, 0.0), p(0.0, 5.0), p(10.0, 5.0)),
            None
        );
        assert!(is_parallel(
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(3.0, 5.0),
            p(-40.0, 5.0)
        ));
        assert!(is_perpendicular(
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(3.0, 5.0),
            p(3.0, 50.0)
        ));
        assert!(!is_perpendicular(
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(0.0, 0.0),
            p(10.0, 10.0)
        ));
    }

    #[test]
    fn nearest_point_and_distance_use_the_infinite_line() {
        let n = nearest_on_line(p(500.0, 7.0), p(0.0, 0.0), p(10.0, 0.0));
        assert_eq!(n, p(500.0, 0.0));
        assert!((dist_to_line(p(500.0, 7.0), p(0.0, 0.0), p(10.0, 0.0)) - 7.0).abs() < 1e-9);
    }

    #[test]
    fn line_angles_ignore_direction() {
        assert!(line_angle_deg(p(0.0, 0.0), p(1.0, 0.0)).abs() < 1e-9);
        assert!(line_angle_deg(p(1.0, 0.0), p(0.0, 0.0)).abs() < 1e-9);
        assert!((line_angle_deg(p(0.0, 0.0), p(0.0, 5.0)) - 90.0).abs() < 1e-9);
        assert!((line_angle_deg(p(0.0, 5.0), p(0.0, 0.0)) - 90.0).abs() < 1e-9);
        assert!((line_angle_deg(p(0.0, 0.0), p(-1.0, 1.0)) - 135.0).abs() < 1e-9);
        assert!(angle_diff(179.9, 0.1) < 0.3);
    }

    #[test]
    fn an_infinite_line_clips_to_the_view_edges() {
        let (s, e) = clip_line_to_rect(p(10.0, 10.0), p(20.0, 10.0), p(0.0, 0.0), p(100.0, 50.0))
            .unwrap();
        assert_eq!((s, e), (p(0.0, 10.0), p(100.0, 10.0)));
        // Direction decides which end is the start.
        let (s, e) = clip_line_to_rect(p(20.0, 10.0), p(10.0, 10.0), p(0.0, 0.0), p(100.0, 50.0))
            .unwrap();
        assert_eq!((s, e), (p(100.0, 10.0), p(0.0, 10.0)));
        assert!(
            clip_line_to_rect(p(0.0, 80.0), p(10.0, 80.0), p(0.0, 0.0), p(100.0, 50.0)).is_none()
        );
    }

    #[test]
    fn the_infinite_stand_in_is_long_and_recognised() {
        let (a, b) = infinite_segment(p(0.0, 0.0), p(10.0, 0.0));
        assert!(is_infinite_segment(a, b));
        assert!(!is_infinite_segment(p(0.0, 0.0), p(10_000.0, 0.0)));
        assert!(dist_to_line(p(5.0, 0.0), a, b) < 1e-6);
    }

    // ----- ordering -----

    fn line(id: Id, a: (f64, f64), b: (f64, f64)) -> OrderLine {
        OrderLine {
            id,
            a: p(a.0, a.1),
            b: p(b.0, b.1),
        }
    }

    #[test]
    fn count_formats_count() {
        assert_eq!(CountFormat::Numbers.format(3), "3");
        assert_eq!(CountFormat::PaddedNumbers.format(3), "03");
        assert_eq!(CountFormat::UpperLetters.format(1), "A");
        assert_eq!(CountFormat::UpperLetters.format(26), "Z");
        assert_eq!(CountFormat::UpperLetters.format(27), "AA");
        assert_eq!(CountFormat::LowerLetters.format(28), "ab");
        assert_eq!(CountFormat::UpperRoman.format(14), "XIV");
        assert_eq!(CountFormat::LowerRoman.format(9), "ix");
    }

    #[test]
    fn rule_sets_number_vertical_lines_left_to_right_and_flat_ones_top_down() {
        let sets = ConstructionSettings::default().rule_sets;
        let lines = [
            line(1, (300.0, 0.0), (300.0, 10.0)),
            line(2, (100.0, 0.0), (100.0, 10.0)),
            line(3, (200.0, 0.0), (200.0, 10.0)),
            line(4, (0.0, 50.0), (10.0, 50.0)),
            line(5, (0.0, 400.0), (10.0, 400.0)),
        ];
        let labels = order_labels(&lines, ViewType::Plan, &sets);
        // Vertical: numbers from left to right.
        assert_eq!(labels[&2], "1");
        assert_eq!(labels[&3], "2");
        assert_eq!(labels[&1], "3");
        // Horizontal: letters from the top (largest y) down.
        assert_eq!(labels[&5], "A");
        assert_eq!(labels[&4], "B");
    }

    #[test]
    fn collinear_lines_share_a_number_and_reverse_flips_the_direction() {
        let mut sets = ConstructionSettings::default().rule_sets;
        let lines = [
            line(1, (100.0, 0.0), (100.0, 10.0)),
            line(2, (100.0, 500.0), (100.0, 600.0)),
            line(3, (250.0, 0.0), (250.0, 10.0)),
        ];
        let labels = order_labels(&lines, ViewType::Plan, &sets);
        assert_eq!(labels[&1], "1");
        assert_eq!(labels[&2], "1", "collinear");
        assert_eq!(labels[&3], "2");
        sets[0].reverse = true;
        let labels = order_labels(&lines, ViewType::Plan, &sets);
        assert_eq!(labels[&3], "1");
        assert_eq!(labels[&1], "2");
        assert_eq!(labels[&2], "2");
    }

    #[test]
    fn lines_at_other_angles_and_disabled_sets_get_no_label() {
        let mut sets = ConstructionSettings::default().rule_sets;
        let lines = [line(1, (0.0, 0.0), (100.0, 100.0))];
        assert!(order_labels(&lines, ViewType::Plan, &sets).is_empty());
        // A rule for 45 degrees takes it.
        sets.push(RuleSet {
            name: "Diagonal".into(),
            angle_deg: 45.0,
            format: CountFormat::LowerRoman,
            ..RuleSet::default()
        });
        assert_eq!(order_labels(&lines, ViewType::Plan, &sets)[&1], "i");
        sets[4].enabled = false;
        assert!(order_labels(&lines, ViewType::Plan, &sets).is_empty());
        // The elevation rules do not number plan lines.
        let vertical = [line(2, (0.0, 0.0), (0.0, 10.0))];
        let mut only_elev = ConstructionSettings::default().rule_sets;
        only_elev.retain(|r| r.view == ViewType::Elevation);
        assert!(order_labels(&vertical, ViewType::Plan, &only_elev).is_empty());
        assert_eq!(order_labels(&vertical, ViewType::Elevation, &only_elev)[&2], "1");
    }

    #[test]
    fn the_higher_priority_set_wins_when_two_take_a_line() {
        let mut s = ConstructionSettings::default();
        let mine = s.add_rule_set();
        s.rule_sets[mine].angle_deg = 90.0;
        s.rule_sets[mine].format = CountFormat::UpperRoman;
        let lines = [line(1, (0.0, 0.0), (0.0, 10.0))];
        // Below the system set: the system set numbers it.
        assert_eq!(order_labels(&lines, ViewType::Plan, &s.rule_sets)[&1], "1");
        // Raised to the top: the roman set does.
        for _ in 0..mine {
            let i = s
                .rule_sets
                .iter()
                .position(|r| r.name == "New Rule Set")
                .unwrap();
            s.raise_priority(i);
        }
        assert_eq!(order_labels(&lines, ViewType::Plan, &s.rule_sets)[&1], "I");
    }

    #[test]
    fn rule_set_management_follows_the_manual() {
        let mut s = ConstructionSettings::default();
        assert!(!s.delete_rule_set(0), "system sets stay");
        let n = s.add_rule_set();
        assert_eq!(s.rule_sets[n].name, "New Rule Set");
        let c = s.copy_rule_set(n).unwrap();
        assert_eq!(s.rule_sets[c].name, "New Rule Set Copy");
        assert!(s.name_error(c).is_none());
        s.rule_sets[c].name = "New Rule Set".into();
        assert!(s.name_error(c).is_some(), "names are unique");
        assert!(s.delete_rule_set(c));
        assert_eq!(s.raise_priority(0), None);
        assert_eq!(s.lower_priority(0), Some(1));
        assert_eq!(s.rule_sets[1].name, "Plan Vertical");
        assert!(!s.is_default());
    }

    #[test]
    fn callout_text_reports_the_order_label_when_automatic() {
        let mut c = CalloutSpec::default();
        assert_eq!(c.texts(Some("B")), ("B".to_string(), String::new()));
        assert_eq!(c.texts(None), (String::new(), String::new()));
        c.label_auto = false;
        c.label = "Grid".into();
        c.text_below = "N".into();
        assert_eq!(c.texts(Some("B")), ("Grid".to_string(), "N".to_string()));
        c.below_auto = true;
        assert_eq!(c.texts(Some("B")).1, "B");
    }

    // ----- the model -----

    #[test]
    fn a_new_construction_line_gets_the_layer_group_and_defaults() {
        let mut proj = Project::new("t");
        proj.construction.defaults.all_floors = true;
        let id = proj
            .add_construction_line(0, p(0.0, 0.0), p(100.0, 0.0), None)
            .unwrap();
        assert!(proj.layers.get(LAYER).is_some());
        let f = &proj.floors[0];
        assert!(f.is_construction_line(id));
        assert_eq!(f.construction.get(id).unwrap().id, id);
        assert!(f.construction.get(id).unwrap().all_floors, "from the defaults");
        assert_eq!(
            f.drawing_group_override(crate::groups::ObjectRef::Cad(id)),
            Some(DEFAULT_GROUP)
        );
        // Gone with the CAD line.
        proj.floors[0].cad.clear();
        assert!(!proj.floors[0].is_construction_line(id));
        assert_eq!(proj.floors[0].prune_construction(), 1);
    }

    #[test]
    fn lines_set_to_all_floors_show_on_the_other_floors() {
        let mut proj = Project::new("t");
        proj.floors.push(Floor::new("2nd Floor", 108.0));
        let here = proj
            .add_construction_line(0, p(0.0, 0.0), p(10.0, 0.0), None)
            .unwrap();
        let everywhere = proj
            .add_construction_line(0, p(0.0, 5.0), p(10.0, 5.0), None)
            .unwrap();
        proj.floors[0]
            .construction
            .get_mut(everywhere)
            .unwrap()
            .all_floors = true;
        let upstairs: Vec<Id> = proj
            .construction_lines_on(1)
            .into_iter()
            .map(|l| l.id)
            .collect();
        assert_eq!(upstairs, vec![everywhere]);
        let down: Vec<Id> = proj
            .construction_lines_on(0)
            .into_iter()
            .map(|l| l.id)
            .collect();
        assert_eq!(down, vec![here, everywhere]);
    }

    #[test]
    fn a_polyline_converts_to_construction_lines_and_back() {
        let mut proj = Project::new("t");
        let id = proj.alloc_id();
        proj.floors[0].cad.push(CadObject {
            id,
            layer: crate::cad::DEFAULT_CAD_LAYER.into(),
            item: CadItem::Polyline {
                points: vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 80.0)],
                closed: false,
            },
        });
        let ids = proj.polyline_to_construction(0, id);
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], id);
        for i in &ids {
            assert!(proj.floors[0].is_construction_line(*i));
        }
        assert_eq!(proj.floors[0].cad.len(), 2);
        assert!(proj.construction_to_polyline(0, ids[1]));
        assert!(!proj.floors[0].is_construction_line(ids[1]));
        let back = proj.floors[0].cad.iter().find(|c| c.id == ids[1]).unwrap();
        assert!(matches!(back.item, CadItem::Polyline { .. }));
        assert!(!proj.construction_to_polyline(0, ids[1]), "already plain");
        assert!(proj.polyline_to_construction(0, 999).is_empty());
    }

    #[test]
    fn the_records_round_trip_through_json() {
        let mut proj = Project::new("t");
        let id = proj
            .add_construction_line(0, p(0.0, 0.0), p(10.0, 0.0), None)
            .unwrap();
        proj.floors[0].construction.get_mut(id).unwrap().callouts.plan = CalloutEnds::Both;
        proj.construction.rule_sets[0].reverse = true;
        proj.reference_table.rows.push(ReferenceRow::for_file("/tmp/x.psplan"));
        let json = proj.to_json().unwrap();
        let back = Project::from_json(&json).unwrap();
        assert_eq!(
            back.floors[0].construction.get(id).unwrap().callouts.plan,
            CalloutEnds::Both
        );
        assert!(back.construction.rule_sets[0].reverse);
        assert_eq!(back.reference_table.rows.len(), 1);
        // A plan without the slots reads as the defaults.
        let plain = Project::new("p").to_json().unwrap();
        assert!(!plain.contains("construction"));
        assert!(!plain.contains("reference_table"));
    }

    // ----- reference rows -----

    #[test]
    fn row_floors_resolve_against_the_current_floor() {
        use RowFloor::*;
        assert_eq!(Below.resolve(0, 3, false), None);
        assert_eq!(Below.resolve(2, 3, false), Some(1));
        assert_eq!(Above.resolve(2, 3, false), None);
        assert_eq!(Above.resolve(0, 3, false), Some(1));
        assert_eq!(Automatic.resolve(2, 3, false), Some(1));
        assert_eq!(Automatic.resolve(0, 3, false), Some(1), "none below: above");
        assert_eq!(Automatic.resolve(0, 1, false), None);
        assert_eq!(Fixed(2).resolve(0, 3, false), Some(2));
        assert_eq!(Fixed(0).resolve(0, 3, false), None, "not its own reference");
        assert_eq!(Fixed(0).resolve(0, 3, true), Some(0), "another plan can");
        assert_eq!(MatchCurrent.resolve(1, 3, true), Some(1));
        assert_eq!(MatchCurrent.resolve(2, 2, true), None, "no such level there");
    }

    #[test]
    fn another_plan_is_rotated_then_offset_into_this_one() {
        let mut row = ReferenceRow::for_file("/x/other.psplan");
        row.offset = [100.0, 50.0, 0.0];
        row.angle_deg = 90.0;
        let w = row.to_world(p(10.0, 0.0));
        assert!(w.dist(p(100.0, 60.0)) < 1e-9, "{w:?}");
        assert!(row.from_world(w).dist(p(10.0, 0.0)) < 1e-9);
        // Rows of this plan are not moved.
        let mut here = ReferenceRow::default();
        here.offset = [5.0, 5.0, 0.0];
        assert_eq!(here.to_world(p(1.0, 2.0)), p(1.0, 2.0));
    }

    #[test]
    fn rotating_about_the_marquee_center_keeps_the_center_put() {
        let mut row = ReferenceRow::for_file("/x/other.psplan");
        row.offset = [100.0, 0.0, 0.0];
        let local_center = p(50.0, 20.0);
        let center = row.to_world(local_center);
        row.rotate_about(center, 30.0);
        assert!((row.angle_deg - 30.0).abs() < 1e-12);
        assert!(row.to_world(local_center).dist(center) < 1e-9);
        row.move_by(p(10.0, -4.0));
        assert!(row.to_world(local_center).dist(p(center.x + 10.0, center.y - 4.0)) < 1e-9);
    }

    #[test]
    fn the_table_keeps_the_current_line_among_the_rows() {
        let mut t = ReferenceTable::default();
        t.rows.push(ReferenceRow::default());
        // [Current, row0]
        assert_eq!(t.lines().len(), 2);
        assert_eq!(t.lines()[0], TableLine::Current);
        // Insert Above the first row: [Current, new, row0].
        let mut b = ReferenceRow::for_file("b.psplan");
        b.layer_set = Some("B".into());
        let at = t.insert_above(1, b.clone());
        assert_eq!(at, 1);
        assert_eq!(t.rows[0], b);
        assert_eq!(t.current_at, 0);
        // Insert Above the Current line puts a row in front of it.
        t.insert_above(0, ReferenceRow::for_file("front.psplan"));
        assert_eq!(t.current_at, 1);
        // Move Down the Current line: it passes the next row.
        assert_eq!(t.move_down(1), Some(2));
        assert_eq!(t.current_at, 2);
        assert_eq!(t.move_up(2), Some(1));
        assert_eq!(t.move_up(0), None);
        assert_eq!(t.move_down(t.lines().len() - 1), None);
        // Insert Below the last line.
        let last = t.lines().len() - 1;
        t.insert_below(last, ReferenceRow::default());
        assert_eq!(t.lines().len(), 5);
    }

    #[test]
    fn a_row_deletes_unless_it_is_current_or_the_only_one() {
        let mut t = ReferenceTable::default();
        t.rows.push(ReferenceRow::default());
        assert!(!t.can_delete(0), "the Current line");
        assert!(!t.can_delete(1), "the only reference row");
        t.insert_below(1, ReferenceRow::for_file("o.psplan"));
        assert!(t.can_delete(2));
        assert!(t.delete(2));
        assert_eq!(t.rows.len(), 1);
        assert!(!t.delete(5));
    }
}
