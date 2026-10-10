//! Dimensions: manual, automatic exterior strings and temporary (transient) ones.

use crate::geometry::Point;
use crate::model::{Id, Opening, Wall, WallKind};
use crate::units::{fmt_ft_in_frac, format_length, LengthFormat, LengthUnit};
use serde::{Deserialize, Serialize};

mod geom;
mod label;
mod layout;
mod line;
mod pole;
mod seg;
mod settings;

pub use geom::CurveGeom;
pub use label::{
    angle_text, grid_round, indicators, round_to_step, step_inches, DimLabelOptions, LabelParts,
    SecondFormat, TolMode, Tolerance,
};
pub use layout::{upright, LabelLayout, LabelLine, LabelParams, LeaderDefaults};
pub use line::{
    migrate_dimension_strings, DimLine, ElevMark, ExtLen, ExtLine, ExtProps, ExtReach, MIN_SEG_LEN,
};
pub use pole::{
    floor_marks, pole_marks, pole_strings, roof_marks, roof_slope_dimensions, section_profile,
    section_roof_marks, ElevationMark, RoofMark,
};
pub use seg::{CurveKind, DimCurve, DimSeg, LeaderStyle};
pub use settings::{
    exterior_strings_for, mark_default, DimSetup, DimView, LocateTool, MarkKind, OffsetFrom,
    PoleMark, PoleSetup, RoundMethod, TempWalls, TextPos, ToolLocate, ToolLocates, LOCATE_MARKS,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DimensionKind {
    Manual,
    AutoExterior,
    Temporary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dimension {
    pub id: Id,
    pub kind: DimensionKind,
    /// First measured point.
    pub start: Point,
    /// Second measured point.
    pub end: Point,
    /// Signed perpendicular offset of the dimension line from the start-end
    /// line. Positive is the left side when walking start to end.
    pub offset: f64,
    #[serde(default)]
    pub text_override: Option<String>,
    /// What the start and end are tied to: a dimension tied to a wall point
    /// follows the wall when it moves or stretches (see [`crate::dim_assoc`]).
    #[serde(default)]
    pub anchors: [Option<crate::dim_assoc::DimAnchor>; 2],
    /// Extension lines switched off per measured point (Dimension
    /// Specification: Show Extension Line).
    #[serde(default)]
    pub hide_ext: [bool; 2],
    /// Which automatic run made this dimension; cleared when it is edited
    /// and becomes manual (DIM-25, DIM-33).
    #[serde(default)]
    pub auto_group: AutoGroup,
    /// The text style of the dimension number; `None` is the dimension
    /// set's (Dimension Defaults, Text Style).
    #[serde(default)]
    pub text_style: Option<String>,
    /// What this dimension sets itself instead of taking the Dimension
    /// Defaults' (Dimension Specification: Primary Format and Arrow tabs,
    /// extension line settings; DIM-31, DIM-39).
    #[serde(default, skip_serializing_if = "DimOverrides::is_default")]
    pub look: DimOverrides,
}

/// The end mark of a dimension line (the Arrow tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DimArrow {
    /// A short stroke across the line (architectural tick).
    #[default]
    Tick,
    /// An arrowhead; `filled` decides between a solid and an open one.
    Arrow,
    Dot,
    /// A long diagonal stroke across the line (drafting slash).
    Slash,
    None,
}

impl DimArrow {
    pub const ALL: [DimArrow; 5] = [
        DimArrow::Tick,
        DimArrow::Arrow,
        DimArrow::Dot,
        DimArrow::Slash,
        DimArrow::None,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DimArrow::Tick => "Tick",
            DimArrow::Arrow => "Arrow",
            DimArrow::Dot => "Dot",
            DimArrow::Slash => "Slash",
            DimArrow::None => "None",
        }
    }

    /// The mark a Dimension Defaults arrow style name asks for (Chief's
    /// names vary; anything unrecognised is the tick).
    pub fn from_name(name: &str) -> DimArrow {
        let n = name.to_ascii_lowercase();
        if n.contains("arrow") {
            DimArrow::Arrow
        } else if n.contains("dot") {
            DimArrow::Dot
        } else if n.contains("slash") {
            DimArrow::Slash
        } else if n == "none" {
            DimArrow::None
        } else {
            DimArrow::Tick
        }
    }
}

/// The settings one dimension keeps of its own. Every field is `None` until
/// the Dimension Specification sets it; `None` follows the Dimension
/// Defaults in force.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DimOverrides {
    // --- Primary Format ---
    pub units: Option<LengthUnit>,
    /// Smallest fraction denominator of feet-inches and inches.
    pub fraction: Option<u32>,
    /// Decimal places of decimal feet and metric units.
    pub decimals: Option<u32>,
    pub unit_indicators: Option<bool>,
    pub trailing_zeroes: Option<bool>,
    /// Feet-inches without the zero feet: `6"` rather than `0'-6"`.
    pub suppress_zero_feet: Option<bool>,
    // --- Arrow ---
    pub arrow: Option<DimArrow>,
    /// Length of the end mark, plan inches.
    pub arrow_size: Option<f64>,
    /// Solid arrowhead or dot.
    pub arrow_filled: Option<bool>,
    // --- Extension lines ---
    /// Gap between the measured point and the extension line.
    pub ext_gap: Option<f64>,
    /// How far the extension line runs past the dimension line.
    pub ext_past: Option<f64>,
    /// A fixed extension line length, measured back from the dimension line
    /// toward the measured point (0 or `None`: the whole way).
    pub ext_length: Option<f64>,
    // --- Secondary Format and tolerance ---
    /// This dimension's own second format (Secondary Format tab).
    pub second: Option<SecondFormat>,
    pub tolerance: Option<Tolerance>,
    /// Rounded value indicators `(+ or - after, ~ before)`.
    pub indicators: Option<(bool, bool)>,
    pub text_pos: Option<TextPos>,
    // --- Dimension panel (Dimension Line Specification, manual pp. 514 to 515) ---
    /// The name of the Saved Dimension Default this line inherits from
    /// (None: the active one).
    pub inherits: Option<String>,
    /// The height of the numbers, plan inches (None: the text style's).
    pub number_height: Option<f64>,
    /// Display Wall Widths: the segments that measure across one wall show
    /// (None: yes, except for Interior Dimensions, which set it off).
    pub wall_widths: Option<bool>,
    /// Display Gaps Between Cabinet Face Items (elevation views).
    pub cabinet_gaps: Option<bool>,
    /// An angular dimension's own angle style `(decimals, degrees-minutes-
    /// seconds)`; None is Use Default Angle Style.
    pub angle_style: Option<(u32, bool)>,
    // --- Layer panel ---
    /// The layer the line is drawn on (None: the default for its kind).
    pub layer: Option<String>,
    // --- Marker Format panel ---
    /// The number format of an elevation marker's height line (None: Use
    /// Default Formatting, the primary format).
    pub marker_format: Option<LengthFormat>,
    // --- Segment, string, label and curve ---
    pub seg: DimSeg,
}

impl DimOverrides {
    pub fn is_default(&self) -> bool {
        *self == DimOverrides::default()
    }

    /// Does the dimension set any part of its number format?
    pub fn has_format(&self) -> bool {
        self.units.is_some()
            || self.fraction.is_some()
            || self.decimals.is_some()
            || self.unit_indicators.is_some()
            || self.trailing_zeroes.is_some()
            || self.suppress_zero_feet.is_some()
    }

    /// The number format in force for the dimension: its own settings laid
    /// over the Dimension Defaults' format `base`.
    pub fn effective_format(&self, base: &DimFormat) -> LengthFormat {
        let mut f = base.effective();
        if let Some(u) = self.units {
            f.unit = u;
        }
        if let Some(d) = self.fraction {
            f.fraction_denominator = d.max(1);
        }
        if let Some(d) = self.decimals {
            f.decimals = d;
        }
        if let Some(i) = self.unit_indicators {
            f.unit_indicators = i;
        }
        if let Some(t) = self.trailing_zeroes {
            f.trailing_zeroes = t;
        }
        f
    }

    /// `inches` as dimension text: the dimension's own format laid over
    /// `base` (the Dimension Defaults' format).
    pub fn format_len(&self, base: &DimFormat, inches: f64) -> String {
        if !self.has_format() {
            return base.fmt_len(inches);
        }
        let f = self.effective_format(base);
        let s = format_length(inches, &f);
        if self.suppress_zero_feet == Some(true) && f.unit == LengthUnit::FeetInches {
            suppress_zero_feet(&s, f.unit_indicators)
        } else {
            s
        }
    }

    /// The label options in force: the defaults' with this dimension's own
    /// second format, tolerance, indicators and text position laid over.
    pub fn label_options(&self, base: &DimFormat) -> DimLabelOptions {
        let mut o = base.label;
        if let Some(s) = self.second {
            o.second = s;
        }
        if let Some(t) = self.tolerance {
            o.tolerance = t;
        }
        if let Some((after, tilde)) = self.indicators {
            o.plus_minus_after = after;
            o.tilde_before = tilde;
        }
        if let Some(p) = self.text_pos {
            o.position = p;
        }
        o
    }
}

/// `0'-6"` as `6"` (and `0-6` as `6` without unit marks).
pub fn suppress_zero_feet(s: &str, unit_indicators: bool) -> String {
    let (sign, body) = match s.strip_prefix('-') {
        Some(b) => ("-", b),
        None => ("", s),
    };
    let prefix = if unit_indicators { "0'-" } else { "0-" };
    match body.strip_prefix(prefix) {
        Some(rest) => format!("{sign}{rest}"),
        None => s.to_string(),
    }
}

/// One extension line from the measured point `m` toward the end `e` it
/// reaches on the dimension line: it starts `gap` off the point, runs `past`
/// the dimension line, and with `length` set covers only that much back from
/// the dimension line. `None` when the two coincide.
pub fn extension_segment(
    m: Point,
    e: Point,
    gap: f64,
    past: f64,
    length: Option<f64>,
) -> Option<(Point, Point)> {
    let v = e.sub(m);
    let len = v.length();
    if len < 1e-6 {
        return None;
    }
    let u = v.scale(1.0 / len);
    let mut start = if len > gap { gap.max(0.0) } else { 0.0 };
    if let Some(l) = length.filter(|l| *l > 0.0) {
        start = start.max(len - l);
    }
    Some((m.add(u.scale(start)), e.add(u.scale(past.max(0.0)))))
}

/// The automatic run a dimension came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AutoGroup {
    /// Manual, or an automatic dimension from before runs were recorded.
    #[default]
    None,
    /// Auto Exterior Dimensions.
    Exterior,
    /// Auto Interior Dimensions.
    Interior,
    /// Auto Elevation and Auto Story Pole dimensions (heights, not plan
    /// distances).
    Levels,
    /// Auto NKBA Dimensions: strings along the kitchen and bath cabinet runs
    /// (cabinet faces, appliance centers, overall).
    Nkba,
}

/// Which wall surface a dimension locates (Dimension Defaults, Locate
/// Objects: Walls).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WallLocate {
    /// The wall's outer and inner surfaces (its full thickness).
    Surfaces,
    /// The main layer's surfaces.
    #[default]
    MainLayer,
    /// The wall's centerline.
    Centers,
}

impl WallLocate {
    pub const ALL: [WallLocate; 3] = [
        WallLocate::Surfaces,
        WallLocate::MainLayer,
        WallLocate::Centers,
    ];

    pub fn label(self) -> &'static str {
        match self {
            WallLocate::Surfaces => "Surfaces",
            WallLocate::MainLayer => "Main Layer",
            WallLocate::Centers => "Centers",
        }
    }
}

/// How a dimension locates a door or window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OpeningLocate {
    /// The opening's sides (jambs).
    #[default]
    Sides,
    Centers,
    /// Openings are not located: the wall behind is.
    None,
}

impl OpeningLocate {
    pub const ALL: [OpeningLocate; 3] = [
        OpeningLocate::Sides,
        OpeningLocate::Centers,
        OpeningLocate::None,
    ];

    pub fn label(self) -> &'static str {
        match self {
            OpeningLocate::Sides => "Sides",
            OpeningLocate::Centers => "Centers",
            OpeningLocate::None => "None",
        }
    }
}

/// Whether a dimension locates cabinets, or fixtures, at their sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ObjectLocate {
    #[default]
    Sides,
    None,
}

impl ObjectLocate {
    pub const ALL: [ObjectLocate; 2] = [ObjectLocate::Sides, ObjectLocate::None];

    pub fn label(self) -> &'static str {
        match self {
            ObjectLocate::Sides => "Sides",
            ObjectLocate::None => "None",
        }
    }
}

/// One string of the Auto Exterior Dimensions set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutoString {
    /// Door and window sides (or centers) along the wall.
    Openings,
    /// Wall to wall: the corners and the walls that meet the side.
    WallToWall,
    /// Corner to corner.
    Overall,
}

impl AutoString {
    pub fn label(self) -> &'static str {
        match self {
            AutoString::Openings => "Openings",
            AutoString::WallToWall => "Wall to Wall",
            AutoString::Overall => "Overall",
        }
    }
}

/// Chief's default set, nearest the wall first.
pub const DEFAULT_AUTO_STRINGS: [AutoString; 3] = [
    AutoString::Openings,
    AutoString::WallToWall,
    AutoString::Overall,
];

/// One set of Locate Objects choices (Dimension Defaults, Locate Objects).
/// A dimension set keeps its main group in its own fields; the temporary and
/// the elevation dimensions each have a group of their own (DIM-40).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocateGroup {
    pub walls: WallLocate,
    pub openings: OpeningLocate,
    pub cabinets: ObjectLocate,
    pub fixtures: ObjectLocate,
}

impl Default for LocateGroup {
    /// Wall surfaces and opening sides: how temporary dimensions have always
    /// measured (face to face, jamb to jamb).
    fn default() -> Self {
        Self {
            walls: WallLocate::Surfaces,
            openings: OpeningLocate::Sides,
            cabinets: ObjectLocate::Sides,
            fixtures: ObjectLocate::Sides,
        }
    }
}

/// How dimension text is formatted.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DimFormat {
    /// Smallest fraction denominator shown: 8 or 16.
    pub smallest_fraction: u32,
    /// Show `'` and `"` unit indicators.
    pub unit_indicators: bool,
    /// Full length format (imperial or metric). When set it takes over
    /// from `smallest_fraction` / `unit_indicators`.
    #[serde(default)]
    pub length: Option<LengthFormat>,
    /// The second format, tolerance, rounding method and indicators.
    #[serde(default)]
    pub label: DimLabelOptions,
}

impl Default for DimFormat {
    fn default() -> Self {
        Self {
            smallest_fraction: 16,
            unit_indicators: true,
            length: None,
            label: DimLabelOptions::default(),
        }
    }
}

impl DimFormat {
    /// The length format this describes: `length`, else feet-inches at
    /// `smallest_fraction`.
    pub fn effective(&self) -> LengthFormat {
        self.length.unwrap_or(LengthFormat {
            unit: LengthUnit::FeetInches,
            fraction_denominator: self.smallest_fraction.max(1),
            decimals: 2,
            unit_indicators: self.unit_indicators,
            trailing_zeroes: false,
        })
    }

    /// Whole-millimetre dimension text with no unit marks, e.g. `3048`.
    pub fn metric_mm() -> Self {
        Self {
            length: Some(LengthFormat::metric_mm()),
            ..Self::default()
        }
    }

    /// Feet-inches text rounded to the chosen fraction, e.g. `12'-6 1/2"`
    /// (or `12-6 1/2` without unit indicators).
    pub fn fmt_len(&self, inches: f64) -> String {
        if let Some(l) = &self.length {
            return format_length(inches, l);
        }
        let s = fmt_ft_in_frac(inches, self.smallest_fraction);
        if self.unit_indicators {
            s
        } else {
            s.replace(['\'', '"'], "")
        }
    }
}

impl Dimension {
    pub fn new(id: Id, kind: DimensionKind, start: Point, end: Point, offset: f64) -> Self {
        Self {
            id,
            kind,
            start,
            end,
            offset,
            text_override: None,
            anchors: [None, None],
            hide_ext: [false; 2],
            auto_group: AutoGroup::None,
            text_style: None,
            look: DimOverrides::default(),
        }
    }

    pub fn length(&self) -> f64 {
        self.start.dist(self.end)
    }

    fn offset_vec(&self) -> Point {
        self.end
            .sub(self.start)
            .normalized()
            .perp()
            .scale(self.offset)
    }

    /// Endpoints of the dimension line (the measured line pushed out by `offset`).
    pub fn line_points(&self) -> (Point, Point) {
        let o = self.offset_vec();
        (self.start.add(o), self.end.add(o))
    }

    /// Extension lines from each measured point out to the dimension line.
    pub fn extension_lines(&self) -> [(Point, Point); 2] {
        let o = self.offset_vec();
        [(self.start, self.start.add(o)), (self.end, self.end.add(o))]
    }

    /// The extension lines that are shown (see `hide_ext`).
    pub fn visible_extension_lines(&self) -> Vec<(Point, Point)> {
        self.extension_lines()
            .into_iter()
            .zip(self.hide_ext)
            .filter(|(_, hidden)| !hidden)
            .map(|(l, _)| l)
            .collect()
    }

    /// The label on one line: the number with any additional text, tolerance
    /// and indicators, and the second format in parentheses
    /// ([`Dimension::label_parts`] has the lines apart).
    pub fn label(&self, fmt: &DimFormat) -> String {
        self.label_parts(fmt).one_line()
    }

    /// Reverse Dimension (DIM-37): the measured points swap ends (with their
    /// ties and extension-line switches), which turns the left-hand side the
    /// offset is measured to around: the dimension line lands on the other
    /// side of the objects it measures, the same distance away.
    pub fn reverse(&mut self) {
        std::mem::swap(&mut self.start, &mut self.end);
        self.anchors.swap(0, 1);
        self.hide_ext.swap(0, 1);
    }

    /// An automatic dimension becomes manual (Convert to Manual); false when
    /// it already is.
    pub fn convert_to_manual(&mut self) -> bool {
        let was = self.kind != DimensionKind::Manual || self.auto_group != AutoGroup::None;
        if self.kind == DimensionKind::AutoExterior {
            self.kind = DimensionKind::Manual;
        }
        self.auto_group = AutoGroup::None;
        was && self.kind == DimensionKind::Manual
    }

    /// The unit left-hand normal of the measuring direction.
    fn normal(&self) -> Point {
        self.end.sub(self.start).normalized().perp()
    }

    /// Where the dimension line lies along `n`, measured from `origin`.
    fn line_pos(&self, origin: Point, n: Point) -> f64 {
        self.start.sub(origin).dot(n) + self.offset * self.normal().dot(n)
    }

    /// Puts the dimension line `pos` along `n` from `origin` by changing the
    /// offset; false when the dimension is not parallel to the reference.
    fn set_line_pos(&mut self, origin: Point, n: Point, pos: f64) -> bool {
        let k = self.normal().dot(n);
        if k.abs() < 0.9 || self.length() < 1e-9 {
            return false;
        }
        self.offset = (pos - self.start.sub(origin).dot(n)) / k;
        true
    }
}

/// Align Dimensions (DIM-36): every dimension parallel to the first has its
/// dimension line moved onto the first one's line. Returns how many moved.
pub fn align_dimensions(dims: &mut [Dimension]) -> usize {
    let Some((first, rest)) = dims.split_first_mut() else {
        return 0;
    };
    if first.length() < 1e-9 {
        return 0;
    }
    let (origin, n) = (first.start, first.normal());
    let pos = first.line_pos(origin, n);
    let mut moved = 0;
    for d in rest {
        if d.set_line_pos(origin, n, pos) {
            moved += 1;
        }
    }
    moved
}

/// Distribute Dimensions (DIM-36): the dimension lines of the dimensions
/// parallel to the first (it included) are spaced evenly between the two
/// outermost ones. Returns how many lines changed (the two ends stay).
pub fn distribute_dimensions(dims: &mut [Dimension]) -> usize {
    let Some(first) = dims.first() else { return 0 };
    if first.length() < 1e-9 {
        return 0;
    }
    let (origin, n) = (first.start, first.normal());
    let mut order: Vec<(usize, f64)> = dims
        .iter()
        .enumerate()
        .filter(|(_, d)| d.length() > 1e-9 && d.normal().dot(n).abs() >= 0.9)
        .map(|(i, d)| (i, d.line_pos(origin, n)))
        .collect();
    if order.len() < 3 {
        return 0;
    }
    order.sort_by(|a, b| a.1.total_cmp(&b.1));
    let (lo, hi) = (order[0].1, order[order.len() - 1].1);
    let last = (order.len() - 1) as f64;
    let mut moved = 0;
    for (k, (i, _)) in order.iter().enumerate().skip(1).take(order.len() - 2) {
        let pos = lo + (hi - lo) * k as f64 / last;
        if dims[*i].set_line_pos(origin, n, pos) {
            moved += 1;
        }
    }
    moved
}

/// Which axis-aligned side of the building a string runs along.
#[derive(Clone, Copy)]
enum Side {
    Bottom,
    Right,
    Top,
    Left,
}

const AXIS_TOL: f64 = 1e-3;
const SIDE_TOL: f64 = 1.0;
const BREAK_TOL: f64 = 0.5;

fn is_horizontal(w: &Wall) -> bool {
    w.direction().y.abs() < AXIS_TOL && w.length() > 0.0
}

fn is_vertical(w: &Wall) -> bool {
    w.direction().x.abs() < AXIS_TOL && w.length() > 0.0
}

/// Automatic exterior dimensions.
///
/// Uses the Exterior walls (or all walls when none are Exterior). For each of
/// the four axis-aligned sides it emits one overall dimension, `offset` inches
/// outside the outer wall face and measuring wall centerline to centerline,
/// and, when walls break the side, a string of dimensions between consecutive
/// centerline breakpoints half-way between the wall and the overall line.
/// The four overall dimensions come first (bottom, right, top, left), then
/// the strings. Ids are `0`; the caller assigns real ids when adding them.
pub fn auto_exterior_dimensions(walls: &[Wall], offset: f64) -> Vec<Dimension> {
    let mut outer: Vec<&Wall> = walls
        .iter()
        .filter(|w| w.kind == WallKind::Exterior)
        .collect();
    if outer.is_empty() {
        outer = walls.iter().collect();
    }
    outer.retain(|w| is_horizontal(w) || is_vertical(w));
    if outer.is_empty() {
        return Vec::new();
    }

    // Extreme centerline coordinates of the outer boundary.
    let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
    for w in &outer {
        for p in [w.start, w.end] {
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
        }
    }

    let mut overall = Vec::new();
    let mut strings = Vec::new();
    for side in [Side::Bottom, Side::Right, Side::Top, Side::Left] {
        // Walls lying along this side: (along-axis interval, thickness).
        let (horizontal, line) = match side {
            Side::Bottom => (true, min_y),
            Side::Top => (true, max_y),
            Side::Left => (false, min_x),
            Side::Right => (false, max_x),
        };
        let on_side: Vec<&&Wall> = outer
            .iter()
            .filter(|w| {
                if horizontal {
                    is_horizontal(w) && (w.start.y - line).abs() <= SIDE_TOL
                } else {
                    is_vertical(w) && (w.start.x - line).abs() <= SIDE_TOL
                }
            })
            .collect();
        if on_side.is_empty() {
            continue;
        }
        let along = |p: Point| if horizontal { p.x } else { p.y };
        let half_t = on_side.iter().map(|w| w.thickness).fold(0.0_f64, f64::max) * 0.5;
        let lo = on_side
            .iter()
            .flat_map(|w| [along(w.start), along(w.end)])
            .fold(f64::INFINITY, f64::min);
        let hi = on_side
            .iter()
            .flat_map(|w| [along(w.start), along(w.end)])
            .fold(f64::NEG_INFINITY, f64::max);

        // Outer face coordinate on the perpendicular axis.
        let face = match side {
            Side::Bottom | Side::Left => line - half_t,
            Side::Top | Side::Right => line + half_t,
        };
        // Direction chosen so the left-hand perpendicular points outward.
        let (s, e) = match side {
            Side::Bottom => (Point::new(hi, face), Point::new(lo, face)),
            Side::Top => (Point::new(lo, face), Point::new(hi, face)),
            Side::Right => (Point::new(face, hi), Point::new(face, lo)),
            Side::Left => (Point::new(face, lo), Point::new(face, hi)),
        };
        overall.push(Dimension::new(0, DimensionKind::AutoExterior, s, e, offset));

        // Breakpoints: side wall ends plus walls butting into the side line.
        let mut breaks: Vec<f64> = on_side
            .iter()
            .flat_map(|w| [along(w.start), along(w.end)])
            .collect();
        for w in walls {
            let perpendicular = if horizontal {
                is_vertical(w)
            } else {
                is_horizontal(w)
            };
            if !perpendicular {
                continue;
            }
            for p in [w.start, w.end] {
                let across = if horizontal { p.y } else { p.x };
                if (across - line).abs() <= half_t + SIDE_TOL {
                    let a = along(p);
                    if a > lo - BREAK_TOL && a < hi + BREAK_TOL {
                        breaks.push(a);
                    }
                }
            }
        }
        breaks.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        breaks.dedup_by(|a, b| (*a - *b).abs() <= BREAK_TOL);
        if breaks.len() > 2 {
            for pair in breaks.windows(2) {
                let pt = |a: f64| {
                    if horizontal {
                        Point::new(a, face)
                    } else {
                        Point::new(face, a)
                    }
                };
                // Same direction convention as the overall dimension.
                let (a, b) = if (e.sub(s)).dot(if horizontal {
                    Point::new(1.0, 0.0)
                } else {
                    Point::new(0.0, 1.0)
                }) >= 0.0
                {
                    (pair[0], pair[1])
                } else {
                    (pair[1], pair[0])
                };
                strings.push(Dimension::new(
                    0,
                    DimensionKind::AutoExterior,
                    pt(a),
                    pt(b),
                    offset * 0.5,
                ));
            }
        }
    }
    overall.extend(strings);
    overall
}

// ===== the Auto Exterior Dimensions set =====

/// Settings of [`auto_exterior_set`]: the strings (nearest the wall first),
/// where the first sits and how far apart they are, and what the dimensions
/// locate (Dimension Defaults, Setup and Locate Objects).
pub struct ExteriorSetup<'a> {
    pub strings: &'a [AutoString],
    /// From the wall's outer surface to the nearest string, inches.
    pub first_offset: f64,
    /// Between strings, inches.
    pub spacing: f64,
    pub walls: WallLocate,
    pub openings: OpeningLocate,
    /// The main layer's lateral span across a wall (offsets from its
    /// centerline along the wall's normal), where wall types are known.
    pub main_span: &'a dyn Fn(&Wall) -> (f64, f64),
}

impl ExteriorSetup<'_> {
    /// The lateral span a wall is located at.
    pub fn span(&self, w: &Wall) -> (f64, f64) {
        match self.walls {
            WallLocate::Centers => (0.0, 0.0),
            WallLocate::Surfaces => (-w.thickness * 0.5, w.thickness * 0.5),
            WallLocate::MainLayer => (self.main_span)(w),
        }
    }
}

/// Walls within this many radians (about 2 degrees) of one direction (modulo
/// 90 degrees) share a frame, so a hand-drawn side a few inches off plumb is
/// still one side of the building and gets one set of strings (QA-08).
const FRAME_TOL: f64 = 0.035;
/// Segments shorter than this are not dimensioned, inches.
const MIN_SEGMENT: f64 = 0.5;

/// The direction of a wall modulo 90 degrees, in `[0, pi/2)`.
fn frame_of(w: &Wall) -> f64 {
    let a = w
        .direction()
        .angle()
        .rem_euclid(std::f64::consts::FRAC_PI_2);
    if a < FRAME_TOL || a > std::f64::consts::FRAC_PI_2 - FRAME_TOL {
        0.0
    } else {
        a
    }
}

/// Automatic exterior dimensions as Chief's default set: up to three strings
/// on each side of the building, as configured, nearest the wall first (by
/// default the openings, then wall to wall, then the overall dimension).
///
/// The exterior walls (all walls when none are Exterior) are grouped by
/// direction (modulo 90 degrees) and each group is dimensioned in its own
/// frame, so a building turned off the axes, or a wing at 45 degrees, gets
/// strings parallel to its walls. A string that would repeat another (one
/// segment equal to the overall) or that has nothing to show (no openings) is
/// left out. Points are placed on the wall surface `setup` locates and each
/// string sits `first_offset` (plus `spacing` per slot) beyond the outer
/// face. Ids are `0`; every dimension is [`AutoGroup::Exterior`].
///
/// A curved exterior wall takes part by its chord when the chord lies in a
/// frame the straight walls already make: its tangent points (the chord
/// ends) are breaks of the wall-to-wall string and its bulge pushes the
/// strings out and counts toward the overall.
pub fn auto_exterior_set(
    walls: &[Wall],
    openings: &[Opening],
    setup: &ExteriorSetup,
) -> Vec<Dimension> {
    // Curved walls take part by their chord (they join the frame their chord
    // lies in, but never start one) and their bulge clears the strings.
    let straight: Vec<&Wall> = walls.iter().filter(|w| w.length() > 0.0).collect();
    let mut outer: Vec<&Wall> = straight
        .iter()
        .copied()
        .filter(|w| w.kind == WallKind::Exterior)
        .collect();
    if outer.is_empty() {
        outer = straight.clone();
    }
    let mut frames: Vec<f64> = Vec::new();
    for w in outer.iter().filter(|w| !w.is_curved()) {
        let a = frame_of(w);
        if !frames.iter().any(|f| (f - a).abs() <= FRAME_TOL) {
            frames.push(a);
        }
    }
    frames.sort_by(f64::total_cmp);
    let mut out = Vec::new();
    for theta in frames {
        out.extend(frame_set(theta, &straight, &outer, openings, setup));
    }
    out
}

/// The strings of one direction frame (walls turned by `-theta` so they run
/// along the axes, the results turned back).
fn frame_set(
    theta: f64,
    all: &[&Wall],
    outer: &[&Wall],
    openings: &[Opening],
    s: &ExteriorSetup,
) -> Vec<Dimension> {
    let (c, sn) = (theta.cos(), theta.sin());
    let rot = |p: Point| Point::new(p.x * c + p.y * sn, -p.x * sn + p.y * c);
    let unrot = |p: Point| Point::new(p.x * c - p.y * sn, p.x * sn + p.y * c);
    let turned = |ws: &[&Wall]| -> Vec<Wall> {
        ws.iter()
            .filter(|w| (frame_of(w) - theta).abs() <= FRAME_TOL)
            .map(|w| {
                let mut w = (*w).clone();
                w.start = rot(w.start);
                w.end = rot(w.end);
                w
            })
            .collect()
    };
    let all_f = turned(all);
    let outer_f = turned(outer);
    let horizontal_wall = |w: &Wall| w.direction().y.abs() < FRAME_TOL;
    let vertical_wall = |w: &Wall| w.direction().x.abs() < FRAME_TOL;
    let outer_f: Vec<&Wall> = outer_f
        .iter()
        .filter(|w| horizontal_wall(w) || vertical_wall(w))
        .collect();
    if outer_f.is_empty() {
        return Vec::new();
    }
    let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
    for w in &outer_f {
        for p in [w.start, w.end] {
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
        }
    }

    let mut dims = Vec::new();
    for side in [Side::Bottom, Side::Right, Side::Top, Side::Left] {
        let (horizontal, line, out) = match side {
            Side::Bottom => (true, min_y, Point::new(0.0, -1.0)),
            Side::Top => (true, max_y, Point::new(0.0, 1.0)),
            Side::Left => (false, min_x, Point::new(-1.0, 0.0)),
            Side::Right => (false, max_x, Point::new(1.0, 0.0)),
        };
        // A wall slightly off the axis lies on a side by its outermost end.
        let outermost = |w: &Wall| {
            let (a, b) = if horizontal {
                (w.start.y, w.end.y)
            } else {
                (w.start.x, w.end.x)
            };
            let k = if horizontal { out.y } else { out.x };
            if k > 0.0 {
                a.max(b)
            } else {
                a.min(b)
            }
        };
        let on_side: Vec<&Wall> = outer_f
            .iter()
            .copied()
            .filter(|w| {
                if horizontal {
                    horizontal_wall(w) && (outermost(w) - line).abs() <= SIDE_TOL
                } else {
                    vertical_wall(w) && (outermost(w) - line).abs() <= SIDE_TOL
                }
            })
            .collect();
        if on_side.is_empty() {
            continue;
        }
        let along = |p: Point| if horizontal { p.x } else { p.y };
        let axis = if horizontal {
            Point::new(1.0, 0.0)
        } else {
            Point::new(0.0, 1.0)
        };
        // How far the located surface and the physical face lie outward of
        // the centerline.
        let outward = |w: &Wall, (lo, hi): (f64, f64)| {
            let k = w.normal().dot(out);
            (lo * k).max(hi * k)
        };
        // Curved walls on this side: the bulge of the arc pushes the strings
        // outward and can reach past the ends of the run.
        let arcs: Vec<&Wall> = on_side.iter().copied().filter(|w| w.is_curved()).collect();
        let out_k = if horizontal { out.y } else { out.x };
        let (mut arc_out, mut arc_lo, mut arc_hi) = (0.0_f64, f64::INFINITY, f64::NEG_INFINITY);
        for w in &arcs {
            for p in w.sample_points(24) {
                let across = if horizontal { p.y } else { p.x };
                arc_out = arc_out.max((across - line) * out_k + w.thickness * 0.5);
                arc_lo = arc_lo.min(along(p));
                arc_hi = arc_hi.max(along(p));
            }
        }
        let phys = on_side
            .iter()
            .map(|w| w.thickness * 0.5)
            .fold(arc_out, f64::max);
        let meas = on_side
            .iter()
            .map(|w| outward(w, s.span(w)))
            .fold(f64::NEG_INFINITY, f64::max);
        let lo = on_side
            .iter()
            .flat_map(|w| [along(w.start), along(w.end)])
            .fold(f64::INFINITY, f64::min)
            - meas.max(0.0);
        let lo = lo.min(arc_lo);
        let hi = on_side
            .iter()
            .flat_map(|w| [along(w.start), along(w.end)])
            .fold(f64::NEG_INFINITY, f64::max)
            + meas.max(0.0);
        let hi = hi.max(arc_hi);
        let face = line + (if horizontal { out.y } else { out.x }) * meas;
        let base_offset = (phys - meas).max(0.0) + s.first_offset;
        // Direction chosen so the left-hand perpendicular points outward.
        let forward = match side {
            Side::Bottom | Side::Right => false,
            Side::Top | Side::Left => true,
        };
        let pt = |a: f64| {
            if horizontal {
                Point::new(a, face)
            } else {
                Point::new(face, a)
            }
        };
        let emit = |breaks: &mut Vec<f64>, slot: usize, group: &mut Vec<Dimension>| {
            breaks.sort_by(f64::total_cmp);
            breaks.dedup_by(|a, b| (*a - *b).abs() <= BREAK_TOL);
            for pair in breaks.windows(2) {
                if pair[1] - pair[0] < MIN_SEGMENT {
                    continue;
                }
                let (a, b) = if forward {
                    (pair[0], pair[1])
                } else {
                    (pair[1], pair[0])
                };
                let mut d = Dimension::new(
                    0,
                    DimensionKind::AutoExterior,
                    unrot(pt(a)),
                    unrot(pt(b)),
                    base_offset + slot as f64 * s.spacing,
                );
                d.auto_group = AutoGroup::Exterior;
                group.push(d);
            }
        };

        for (slot, kind) in s.strings.iter().enumerate() {
            let mut group = Vec::new();
            match kind {
                AutoString::Overall => {
                    emit(&mut vec![lo, hi], slot, &mut group);
                }
                AutoString::Openings => {
                    if s.openings == OpeningLocate::None {
                        continue;
                    }
                    let mut breaks = vec![lo, hi];
                    let mut found = false;
                    for o in openings {
                        let Some(w) = on_side.iter().find(|w| w.id == o.wall_id && !w.is_curved())
                        else {
                            continue;
                        };
                        found = true;
                        match s.openings {
                            OpeningLocate::Centers => {
                                breaks.push(along(w.point_along(o.center_offset)));
                            }
                            _ => {
                                breaks.push(along(w.point_along(o.start_offset())));
                                breaks.push(along(w.point_along(o.end_offset())));
                            }
                        }
                    }
                    if found {
                        emit(&mut breaks, slot, &mut group);
                    }
                }
                AutoString::WallToWall => {
                    let mut breaks = vec![lo, hi];
                    // A curved wall is dimensioned between its tangent
                    // points (the chord ends), the way a bay reads.
                    for w in &arcs {
                        for p in [w.start, w.end] {
                            breaks.push(along(p).clamp(lo, hi));
                        }
                    }
                    for w in &all_f {
                        let perpendicular = if horizontal {
                            vertical_wall(w)
                        } else {
                            horizontal_wall(w)
                        };
                        if !perpendicular {
                            continue;
                        }
                        let n_along = w.normal().dot(axis);
                        let (a_lo, a_hi) = s.span(w);
                        for p in [w.start, w.end] {
                            let across = if horizontal { p.y } else { p.x };
                            if (across - line).abs() > phys + SIDE_TOL {
                                continue;
                            }
                            let a = along(p);
                            if a > lo - BREAK_TOL && a < hi + BREAK_TOL {
                                // The wall's two surfaces: its thickness is
                                // a segment of the chain.
                                breaks.push((a + a_lo * n_along).clamp(lo, hi));
                                if (a_hi - a_lo).abs() > 0.01 {
                                    breaks.push((a + a_hi * n_along).clamp(lo, hi));
                                }
                            }
                        }
                    }
                    breaks.sort_by(f64::total_cmp);
                    breaks.dedup_by(|a, b| (*a - *b).abs() <= BREAK_TOL);
                    // Nothing but the corners (located at the same point):
                    // it would repeat the overall.
                    if breaks.len() > 2 {
                        emit(&mut breaks, slot, &mut group);
                    }
                }
            }
            dims.extend(group);
        }
    }
    dims
}

// ===== Auto NKBA dimensions =====

/// What an NKBA string measures to inside a cabinet or appliance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NkbaKind {
    /// The center of a sink (its cutout or bowl).
    Sink,
    /// The center of a cooktop or range.
    Cooktop,
    /// The center of an appliance bay or free-standing appliance
    /// (dishwasher, refrigerator, oven).
    Appliance,
}

impl NkbaKind {
    pub fn label(self) -> &'static str {
        match self {
            NkbaKind::Sink => "Sink",
            NkbaKind::Cooktop => "Cooktop",
            NkbaKind::Appliance => "Appliance",
        }
    }
}

/// One thing in a kitchen or bath run: a base or tall cabinet, or a
/// free-standing appliance standing in the run. Its frame is a cabinet's:
/// `origin` is the back-left corner, `angle` turns the width axis, the front
/// faces the left-hand perpendicular of that axis.
#[derive(Debug, Clone, PartialEq)]
pub struct NkbaItem {
    pub origin: Point,
    /// Radians, counter-clockwise.
    pub angle: f64,
    pub width: f64,
    pub depth: f64,
    /// Centers (along the width, from the left edge) the run is also
    /// dimensioned to: the sink, cooktop and appliance bays in this item.
    pub centers: Vec<(NkbaKind, f64)>,
}

impl NkbaItem {
    fn dir(&self) -> Point {
        Point::new(self.angle.cos(), self.angle.sin())
    }
}

/// Settings of [`auto_nkba_dimensions`].
pub struct NkbaSetup<'a> {
    /// From the cabinet front to the nearest string, inches.
    pub first_offset: f64,
    /// Between strings, inches.
    pub spacing: f64,
    /// The walls a run is dimensioned back to (its ends stop at their
    /// surfaces).
    pub walls: &'a [Wall],
    /// Farthest a wall may stand from the end of a run and still be
    /// dimensioned to, inches.
    pub reach: f64,
}

/// Cabinets whose fronts differ by less than this are one run, inches.
const NKBA_FRONT_TOL: f64 = 1.0;
/// Items closer than this along a run touch (one run), inches.
const NKBA_GAP: f64 = 1.5;
/// Directions closer than this are the same, radians.
const NKBA_ANGLE_TOL: f64 = 0.01;

/// Automatic NKBA dimensions for the kitchen and bath cabinet runs: the
/// strings the NKBA Graphic and Presentation Standards ask for on a plan.
/// Items that stand side by side with their fronts on one line make a run;
/// each run gets, on its front side and nearest the cabinets first,
///
/// 1. a string to every cabinet face, from the wall at one end (when one
///    stands within `reach`) to the wall at the other,
/// 2. a string to the centers of the sinks, cooktops and appliances, over the
///    same extent (left out when the run has none), and
/// 3. the overall length of the run (left out when it would repeat the
///    first string).
///
/// Dimensions run from the lower end of the run to the higher one along its
/// width axis and sit on the front side. Ids are `0` and every dimension is
/// [`AutoGroup::Nkba`].
pub fn auto_nkba_dimensions(items: &[NkbaItem], setup: &NkbaSetup) -> Vec<Dimension> {
    // (item index, direction angle) groups: one per direction and front line.
    struct Run {
        angle: f64,
        front: f64,
        members: Vec<usize>,
    }
    let across = |i: &NkbaItem| i.origin.dot(i.dir().perp()) + i.depth;
    let mut groups: Vec<Run> = Vec::new();
    for (k, it) in items.iter().enumerate() {
        if it.width <= 0.0 || it.depth <= 0.0 {
            continue;
        }
        let (a, f) = (it.angle, across(it));
        match groups.iter_mut().find(|g| {
            let da = (g.angle - a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI;
            da.abs() <= NKBA_ANGLE_TOL && (g.front - f).abs() <= NKBA_FRONT_TOL
        }) {
            Some(g) => g.members.push(k),
            None => groups.push(Run {
                angle: a,
                front: f,
                members: vec![k],
            }),
        }
    }
    let mut out = Vec::new();
    for g in groups {
        let u = Point::new(g.angle.cos(), g.angle.sin());
        let n = u.perp();
        let span = |k: usize| {
            let s0 = items[k].origin.dot(u);
            (s0, s0 + items[k].width)
        };
        let mut members = g.members.clone();
        members.sort_by(|a, b| span(*a).0.total_cmp(&span(*b).0));
        // Split the group where the cabinets stop touching.
        let mut runs: Vec<Vec<usize>> = Vec::new();
        let mut reach = f64::NEG_INFINITY;
        for k in members {
            let (s0, s1) = span(k);
            match runs.last_mut() {
                Some(r) if s0 <= reach + NKBA_GAP => r.push(k),
                _ => runs.push(vec![k]),
            }
            reach = if runs.last().is_some_and(|r| r.len() == 1) {
                s1
            } else {
                reach.max(s1)
            };
        }
        for run in runs {
            out.extend(nkba_run(items, &run, u, n, g.front, setup));
        }
    }
    out
}

/// The distance from `from` along `dir` to the nearest wall surface, if it
/// is within `reach`.
fn wall_surface_ahead(walls: &[Wall], from: Point, dir: Point, reach: f64) -> Option<f64> {
    let mut best: Option<f64> = None;
    for w in walls {
        if w.length() < 1e-6 {
            continue;
        }
        let e = w.end.sub(w.start);
        let denom = dir.x * e.y - dir.y * e.x;
        let sin_theta = denom / e.length();
        if sin_theta.abs() < 0.05 {
            continue;
        }
        let r = w.start.sub(from);
        let t = (r.x * e.y - r.y * e.x) / denom;
        let q = (r.x * dir.y - r.y * dir.x) / denom;
        if !(-1e-9..=1.0 + 1e-9).contains(&q) {
            continue;
        }
        // The near surface: the centerline hit pulled back by the half
        // thickness along the ray.
        let face = t - w.thickness * 0.5 / sin_theta.abs();
        if face >= -NKBA_FRONT_TOL && face <= reach {
            best = Some(best.map_or(face.max(0.0), |b| b.min(face.max(0.0))));
        }
    }
    best
}

fn nkba_run(
    items: &[NkbaItem],
    run: &[usize],
    u: Point,
    n: Point,
    front: f64,
    setup: &NkbaSetup,
) -> Vec<Dimension> {
    let at = |s: f64| u.scale(s).add(n.scale(front));
    let mut faces: Vec<f64> = Vec::new();
    let mut centers: Vec<f64> = Vec::new();
    let mut deepest = 0.0_f64;
    for &k in run {
        let it = &items[k];
        let s0 = it.origin.dot(u);
        faces.push(s0);
        faces.push(s0 + it.width);
        deepest = deepest.max(it.depth);
        for (_, c) in &it.centers {
            centers.push(s0 + c);
        }
    }
    let lo_cab = faces.iter().copied().fold(f64::INFINITY, f64::min);
    let hi_cab = faces.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    // The walls the run ends against, measured through the middle of the
    // cabinets' depth.
    let mid = at(0.0).sub(n.scale(deepest * 0.5));
    let ahead =
        |s: f64, dir: Point| wall_surface_ahead(setup.walls, mid.add(u.scale(s)), dir, setup.reach);
    let lo = lo_cab - ahead(lo_cab, u.scale(-1.0)).unwrap_or(0.0);
    let hi = hi_cab + ahead(hi_cab, u).unwrap_or(0.0);

    let string = |breaks: &[f64], slot: usize| -> Vec<Dimension> {
        let mut b: Vec<f64> = breaks.to_vec();
        b.sort_by(f64::total_cmp);
        b.dedup_by(|x, y| (*x - *y).abs() <= BREAK_TOL);
        b.windows(2)
            .filter(|p| p[1] - p[0] >= MIN_SEGMENT)
            .map(|p| {
                let mut d = Dimension::new(
                    0,
                    DimensionKind::AutoExterior,
                    at(p[0]),
                    at(p[1]),
                    setup.first_offset + slot as f64 * setup.spacing,
                );
                d.auto_group = AutoGroup::Nkba;
                d
            })
            .collect()
    };

    let mut out = Vec::new();
    let mut slot = 0;
    let mut face_breaks = faces;
    face_breaks.extend([lo, hi]);
    let face_dims = string(&face_breaks, slot);
    let repeats_overall = face_dims.len() <= 1;
    if !face_dims.is_empty() {
        out.extend(face_dims);
        slot += 1;
    }
    if !centers.is_empty() {
        let mut b = centers;
        b.extend([lo, hi]);
        let d = string(&b, slot);
        if !d.is_empty() {
            out.extend(d);
            slot += 1;
        }
    }
    if !repeats_overall {
        out.extend(string(&[lo, hi], slot));
    }
    out
}

// ===== elevation and story pole dimensions =====

/// A height of the building above the first floor's finished floor.
#[derive(Debug, Clone, PartialEq)]
pub struct Level {
    pub name: String,
    /// Inches above the datum (the first normal floor's finished floor).
    pub elevation: f64,
}

/// The levels of a plan, lowest first: each floor's finished floor and its
/// ceiling. Equal elevations keep the first name.
pub fn story_levels(floors: &[crate::model::Floor]) -> Vec<Level> {
    let mut out: Vec<Level> = Vec::new();
    for f in floors {
        out.push(Level {
            name: format!("{} Floor", f.name),
            elevation: f.elevation,
        });
        out.push(Level {
            name: format!("{} Ceiling", f.name),
            elevation: f.elevation + f.ceiling_height,
        });
    }
    out.sort_by(|a, b| a.elevation.total_cmp(&b.elevation));
    out.dedup_by(|b, a| (a.elevation - b.elevation).abs() < 1e-6);
    out
}

/// A story pole: one vertical dimension between each pair of neighbouring
/// levels (ceiling heights and floor platforms) on the line `x`, with an
/// overall dimension from the lowest to the highest level outside them. The
/// vertical axis of the pole is the plan's Y axis, so lengths read as
/// heights. Ids are `0`.
pub fn story_pole_dimensions(levels: &[Level], x: f64, offset: f64) -> Vec<Dimension> {
    let at = |l: &Level| Point::new(x, l.elevation);
    let mut out: Vec<Dimension> = levels
        .windows(2)
        .filter(|w| w[1].elevation - w[0].elevation >= 0.5)
        .map(|w| Dimension::new(0, DimensionKind::AutoExterior, at(&w[0]), at(&w[1]), offset))
        .collect();
    if let (Some(first), Some(last)) = (levels.first(), levels.last()) {
        if levels.len() > 2 && last.elevation - first.elevation >= 0.5 {
            out.push(Dimension::new(
                0,
                DimensionKind::AutoExterior,
                at(first),
                at(last),
                offset * 2.0,
            ));
        }
    }
    out
}

/// Elevation dimensions: the height of every level above the datum level
/// (the first normal floor's finished floor), as stacked baseline
/// dimensions on the line `x`, `separation` apart. Levels below the datum
/// run downward. Ids are `0`.
pub fn elevation_dimensions(
    levels: &[Level],
    datum: f64,
    x: f64,
    separation: f64,
) -> Vec<Dimension> {
    let mut above: Vec<&Level> = levels
        .iter()
        .filter(|l| l.elevation - datum > 0.5)
        .collect();
    let mut below: Vec<&Level> = levels
        .iter()
        .filter(|l| datum - l.elevation > 0.5)
        .collect();
    above.sort_by(|a, b| a.elevation.total_cmp(&b.elevation));
    below.sort_by(|a, b| b.elevation.total_cmp(&a.elevation));
    let mut out = Vec::new();
    for group in [above, below] {
        for (i, l) in group.iter().enumerate() {
            out.push(Dimension::new(
                0,
                DimensionKind::AutoExterior,
                Point::new(x, datum),
                Point::new(x, l.elevation),
                separation * (i + 1) as f64,
            ));
        }
    }
    out
}

/// The datum of a plan's elevations: the finished floor of the first floor
/// that is not a foundation.
pub fn elevation_datum(floors: &[crate::model::Floor]) -> f64 {
    floors
        .iter()
        .find(|f| f.kind != crate::floors::FloorKind::Foundation)
        .map_or(0.0, |f| f.elevation)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall(id: u64, x0: f64, y0: f64, x1: f64, y1: f64, kind: WallKind) -> Wall {
        Wall {
            id,
            ..Wall::new(Point::new(x0, y0), Point::new(x1, y1), 6.0, 109.125, kind)
        }
    }

    fn rect() -> Vec<Wall> {
        let e = WallKind::Exterior;
        vec![
            wall(1, 0.0, 0.0, 240.0, 0.0, e),
            wall(2, 240.0, 0.0, 240.0, 120.0, e),
            wall(3, 240.0, 120.0, 0.0, 120.0, e),
            wall(4, 0.0, 120.0, 0.0, 0.0, e),
        ]
    }

    #[test]
    fn label_formatting() {
        let f = DimFormat::default();
        assert_eq!(f.fmt_len(150.5), "12'-6 1/2\"");
        assert_eq!(f.fmt_len(150.07), "12'-6 1/16\"");
        let eighths = DimFormat {
            smallest_fraction: 8,
            unit_indicators: true,
            length: None,
            label: DimLabelOptions::default(),
        };
        assert_eq!(eighths.fmt_len(150.07), "12'-6 1/8\"");
        assert_eq!(eighths.fmt_len(150.0), "12'-6\"");
        let bare = DimFormat {
            smallest_fraction: 16,
            unit_indicators: false,
            length: None,
            label: DimLabelOptions::default(),
        };
        assert_eq!(bare.fmt_len(150.5), "12-6 1/2");
        let mm = DimFormat::metric_mm();
        assert_eq!(mm.fmt_len(120.0), "3048");
        assert_eq!(mm.fmt_len(1234.5 / 25.4), "1235");
        // Old JSON without the field still loads and stays imperial.
        let old: DimFormat =
            serde_json::from_str(r#"{"smallest_fraction":8,"unit_indicators":true}"#).unwrap();
        assert_eq!(old.length, None);
        assert_eq!(old.fmt_len(150.0), "12'-6\"");
        let mut d = Dimension::new(
            1,
            DimensionKind::Manual,
            Point::ZERO,
            Point::new(150.5, 0.0),
            12.0,
        );
        assert_eq!(d.label(&f), "12'-6 1/2\"");
        d.text_override = Some("EQ".into());
        assert_eq!(d.label(&f), "EQ");
    }

    #[test]
    fn line_and_extension_geometry() {
        let d = Dimension::new(
            1,
            DimensionKind::Manual,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            24.0,
        );
        assert!((d.length() - 100.0).abs() < 1e-9);
        let (a, b) = d.line_points();
        assert!(a.dist(Point::new(0.0, 24.0)) < 1e-9 && b.dist(Point::new(100.0, 24.0)) < 1e-9);
        let ext = d.extension_lines();
        assert!(ext[0].0.dist(d.start) < 1e-9 && ext[0].1.dist(a) < 1e-9);
        assert!(ext[1].0.dist(d.end) < 1e-9 && ext[1].1.dist(b) < 1e-9);
    }

    #[test]
    fn auto_exterior_on_rectangle() {
        let dims = auto_exterior_dimensions(&rect(), 36.0);
        assert_eq!(dims.len(), 4);
        let lens: Vec<f64> = dims.iter().map(|d| d.length()).collect();
        assert_eq!(
            lens.iter().filter(|l| (**l - 240.0).abs() < 1e-9).count(),
            2
        );
        assert_eq!(
            lens.iter().filter(|l| (**l - 120.0).abs() < 1e-9).count(),
            2
        );
        // Dimension lines sit outside the building (outer face + offset).
        let top = dims
            .iter()
            .find(|d| (d.start.y - 123.0).abs() < 1e-9)
            .unwrap();
        assert!((top.line_points().0.y - 159.0).abs() < 1e-9);
        let bottom = dims
            .iter()
            .find(|d| (d.start.y + 3.0).abs() < 1e-9)
            .unwrap();
        assert!((bottom.line_points().0.y + 39.0).abs() < 1e-9);
    }

    #[test]
    fn auto_exterior_adds_strings_at_breakpoints() {
        let mut walls = rect();
        // Interior wall meeting the bottom wall at x = 100.
        walls.push(wall(5, 100.0, 0.0, 100.0, 120.0, WallKind::Interior));
        let dims = auto_exterior_dimensions(&walls, 36.0);
        // 4 overall + 2 segments on the bottom + 2 on the top.
        assert_eq!(dims.len(), 4 + 2 + 2);
        let mut seg: Vec<f64> = dims[4..]
            .iter()
            .filter(|d| (d.offset - 18.0).abs() < 1e-9)
            .map(|d| d.length())
            .collect();
        seg.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(seg, vec![100.0, 100.0, 140.0, 140.0]);
    }

    #[test]
    fn story_pole_and_elevation_dimensions_read_as_heights() {
        use crate::floors::FloorKind;
        use crate::model::Floor;
        let mut found = Floor::new("Foundation", -36.0);
        found.kind = FloorKind::Foundation;
        found.ceiling_height = 36.0;
        let mut first = Floor::new("1st Floor", 0.0);
        first.ceiling_height = 108.0;
        let mut second = Floor::new("2nd Floor", 118.25);
        second.ceiling_height = 96.0;
        let floors = [found, first, second];
        let levels = story_levels(&floors);
        let heights: Vec<f64> = levels.iter().map(|l| l.elevation).collect();
        // Foundation ceiling and first floor share an elevation (0).
        assert_eq!(heights, vec![-36.0, 0.0, 108.0, 118.25, 214.25]);
        assert_eq!(levels[1].name, "Foundation Ceiling");

        let pole = story_pole_dimensions(&levels, 500.0, 24.0);
        // Four gaps and one overall.
        assert_eq!(pole.len(), 5);
        let lens: Vec<f64> = pole.iter().map(|d| d.length()).collect();
        assert_eq!(lens, vec![36.0, 108.0, 10.25, 96.0, 250.25]);
        assert!(pole.iter().all(|d| d.start.x == 500.0 && d.end.x == 500.0));
        assert_eq!(pole[4].offset, 48.0);
        // The ceiling-height dimension reads 9'-0".
        assert_eq!(pole[1].label(&DimFormat::default()), "9'-0\"");

        let datum = elevation_datum(&floors);
        assert_eq!(datum, 0.0);
        let elev = elevation_dimensions(&levels, datum, 500.0, 18.0);
        // Above the datum: 108, 118.25, 214.25; below: 36.
        assert_eq!(elev.len(), 4);
        assert_eq!(elev[0].length(), 108.0);
        assert_eq!(elev[2].length(), 214.25);
        assert_eq!(elev[2].offset, 54.0);
        assert_eq!(elev[3].end.y, -36.0);
        assert!(story_pole_dimensions(&[], 0.0, 1.0).is_empty());
    }

    // ----- the Auto Exterior set -----

    fn shell_40x30() -> (Vec<Wall>, Vec<Opening>) {
        let e = WallKind::Exterior;
        let walls = vec![
            wall(1, 0.0, 0.0, 480.0, 0.0, e),
            wall(2, 480.0, 0.0, 480.0, 360.0, e),
            wall(3, 480.0, 360.0, 0.0, 360.0, e),
            wall(4, 0.0, 360.0, 0.0, 0.0, e),
            wall(5, 200.0, 0.0, 200.0, 360.0, WallKind::Interior),
        ];
        let openings = [
            (10, 1, 120.0),
            (11, 2, 180.0),
            (12, 3, 300.0),
            (13, 4, 120.0),
        ]
        .iter()
        .map(|&(id, w, c)| Opening::default_window(id, w, c))
        .collect();
        (walls, openings)
    }

    fn setup<'a>(
        strings: &'a [AutoString],
        walls: WallLocate,
        openings: OpeningLocate,
        main: &'a dyn Fn(&Wall) -> (f64, f64),
    ) -> ExteriorSetup<'a> {
        ExteriorSetup {
            strings,
            first_offset: 32.0,
            spacing: 18.0,
            walls,
            openings,
            main_span: main,
        }
    }

    fn full(w: &Wall) -> (f64, f64) {
        (-w.thickness * 0.5, w.thickness * 0.5)
    }

    #[test]
    fn the_shell_gets_three_strings_per_side_with_openings_nearest() {
        let (walls, openings) = shell_40x30();
        let s = setup(
            &DEFAULT_AUTO_STRINGS,
            WallLocate::Surfaces,
            OpeningLocate::Sides,
            &full,
        );
        let dims = auto_exterior_set(&walls, &openings, &s);
        assert!(dims.iter().all(|d| d.auto_group == AutoGroup::Exterior));
        let bottom: Vec<&Dimension> = dims
            .iter()
            .filter(|d| (d.start.y + 3.0).abs() < 1e-9 && (d.end.y + 3.0).abs() < 1e-9)
            .collect();
        // Three distinct strings, nearest first: 32, 50, 68 beyond the face.
        let mut offsets: Vec<f64> = bottom.iter().map(|d| d.offset).collect();
        offsets.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        offsets.sort_by(f64::total_cmp);
        offsets.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        assert_eq!(offsets, vec![32.0, 50.0, 68.0]);
        let lens = |off: f64| -> Vec<f64> {
            let mut v: Vec<f64> = bottom
                .iter()
                .filter(|d| (d.offset - off).abs() < 1e-9)
                .map(|d| d.length())
                .collect();
            v.sort_by(f64::total_cmp);
            v
        };
        // Openings: the corner, the window sides (102..138), the other corner.
        assert_eq!(lens(32.0), vec![36.0, 105.0, 345.0]);
        // Wall to wall: the corner walls' thickness, the interior wall's
        // two faces (197..203) and the spans between.
        assert_eq!(lens(50.0), vec![6.0, 6.0, 6.0, 194.0, 274.0]);
        // Overall: outer surface to outer surface.
        assert_eq!(lens(68.0), vec![486.0]);
        // The left side: the same three strings.
        let left: Vec<&Dimension> = dims
            .iter()
            .filter(|d| (d.start.x + 3.0).abs() < 1e-9 && (d.end.x + 3.0).abs() < 1e-9)
            .collect();
        let mut left_offsets: Vec<f64> = left.iter().map(|d| d.offset).collect();
        left_offsets.sort_by(f64::total_cmp);
        left_offsets.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        assert_eq!(left_offsets, vec![32.0, 50.0, 68.0]);
        // Lines sit outside: the left-hand side of each string faces away
        // from the building.
        for d in &dims {
            let (a, b) = d.line_points();
            let mid = Point::lerp(a, b, 0.5);
            assert!(
                mid.x < -3.0 || mid.x > 483.0 || mid.y < -3.0 || mid.y > 363.0,
                "{mid:?}"
            );
        }
    }

    #[test]
    fn locate_settings_change_where_the_overall_lands() {
        let (walls, _) = shell_40x30();
        let overall = [AutoString::Overall];
        let len_at = |mode: WallLocate| -> f64 {
            let s = setup(&overall, mode, OpeningLocate::Sides, &|w: &Wall| {
                (-w.thickness * 0.25, w.thickness * 0.25)
            });
            let dims = auto_exterior_set(&walls, &[], &s);
            assert_eq!(dims.len(), 4);
            dims.iter().map(Dimension::length).fold(0.0, f64::max)
        };
        assert_eq!(len_at(WallLocate::Surfaces), 486.0);
        assert_eq!(len_at(WallLocate::MainLayer), 483.0);
        assert_eq!(len_at(WallLocate::Centers), 480.0);
        // Opening centers instead of sides.
        let (walls, openings) = shell_40x30();
        let strings = [AutoString::Openings];
        let s = setup(&strings, WallLocate::Centers, OpeningLocate::Centers, &full);
        let dims = auto_exterior_set(&walls, &openings, &s);
        let mut bottom: Vec<f64> = dims
            .iter()
            .filter(|d| d.start.y.abs() < 1e-9 && d.end.y.abs() < 1e-9)
            .map(Dimension::length)
            .collect();
        bottom.sort_by(f64::total_cmp);
        assert_eq!(bottom, vec![120.0, 360.0]);
        // No locate of openings: no openings string.
        let s = setup(&strings, WallLocate::Centers, OpeningLocate::None, &full);
        assert!(auto_exterior_set(&walls, &openings, &s).is_empty());
    }

    #[test]
    fn a_plain_rectangle_has_wall_to_wall_and_overall_but_no_openings_string() {
        let s = setup(
            &DEFAULT_AUTO_STRINGS,
            WallLocate::Surfaces,
            OpeningLocate::Sides,
            &full,
        );
        let dims = auto_exterior_set(&rect(), &[], &s);
        let at = |off: f64| {
            dims.iter()
                .filter(|d| (d.offset - off).abs() < 1e-9)
                .count()
        };
        assert_eq!((at(32.0), at(50.0), at(68.0)), (0, 12, 4));
        // Centerlines only: the corners coincide, so wall to wall would
        // repeat the overall and is left out.
        let s = setup(
            &DEFAULT_AUTO_STRINGS,
            WallLocate::Centers,
            OpeningLocate::Sides,
            &full,
        );
        assert_eq!(auto_exterior_set(&rect(), &[], &s).len(), 4);
    }

    #[test]
    fn a_turned_shell_gets_strings_parallel_to_its_walls() {
        let (walls, openings) = shell_40x30();
        let theta = 30.0_f64.to_radians();
        let (c, sn) = (theta.cos(), theta.sin());
        let turn = |p: Point| Point::new(p.x * c - p.y * sn, p.x * sn + p.y * c);
        let turned: Vec<Wall> = walls
            .iter()
            .map(|w| Wall {
                start: turn(w.start),
                end: turn(w.end),
                ..w.clone()
            })
            .collect();
        let s = setup(
            &DEFAULT_AUTO_STRINGS,
            WallLocate::Surfaces,
            OpeningLocate::Sides,
            &full,
        );
        let straight = auto_exterior_set(&walls, &openings, &s);
        let dims = auto_exterior_set(&turned, &openings, &s);
        assert_eq!(dims.len(), straight.len());
        // Every string runs parallel to a wall of the building.
        for d in &dims {
            let a = d
                .end
                .sub(d.start)
                .angle()
                .rem_euclid(std::f64::consts::FRAC_PI_2);
            assert!((a - theta).abs() < 1e-6, "{a}");
        }
        // The same lengths and offsets as the unturned shell.
        let key = |ds: &[Dimension]| -> Vec<(i64, i64)> {
            let mut v: Vec<(i64, i64)> = ds
                .iter()
                .map(|d| {
                    (
                        (d.length() * 100.0).round() as i64,
                        (d.offset * 100.0).round() as i64,
                    )
                })
                .collect();
            v.sort();
            v
        };
        assert_eq!(key(&dims), key(&straight));
        // And they sit outside the turned building's center.
        let center = turn(Point::new(240.0, 180.0));
        for d in &dims {
            let (a, b) = d.line_points();
            let mid = Point::lerp(a, b, 0.5);
            let n = d.end.sub(d.start).normalized().perp();
            assert!(mid.sub(center).dot(n) > 0.0, "{mid:?}");
        }
    }

    #[test]
    fn a_curved_bay_is_dimensioned_by_its_chord_and_clears_the_strings() {
        use crate::walls::WallCurve;
        let e = WallKind::Exterior;
        let house = |bulge: f64| {
            let mut bay = wall(2, 150.0, 0.0, 330.0, 0.0, e);
            bay.curve = Some(WallCurve { bulge });
            vec![
                wall(1, 0.0, 0.0, 150.0, 0.0, e),
                bay,
                wall(3, 330.0, 0.0, 480.0, 0.0, e),
                wall(4, 480.0, 0.0, 480.0, 360.0, e),
                wall(5, 480.0, 360.0, 0.0, 360.0, e),
                wall(6, 0.0, 360.0, 0.0, 0.0, e),
            ]
        };
        let strings = [AutoString::WallToWall, AutoString::Overall];
        let s = setup(&strings, WallLocate::Surfaces, OpeningLocate::Sides, &full);
        let bottom = |walls: &[Wall]| -> Vec<Dimension> {
            auto_exterior_set(walls, &[], &s)
                .into_iter()
                .filter(|d| (d.start.y + 3.0).abs() < 1e-9 && (d.end.y + 3.0).abs() < 1e-9)
                .collect()
        };
        // Which sign bulges outward (below the wall)?
        let sag = |bulge: f64| {
            let w = &house(bulge)[1];
            w.sample_points(24)
                .iter()
                .map(|p| -p.y)
                .fold(f64::NEG_INFINITY, f64::max)
        };
        let out = if sag(40.0) > sag(-40.0) { 40.0 } else { -40.0 };
        let (sag_out, sag_in) = (sag(out), sag(-out));
        assert!(sag_out > 10.0 && sag_in < 1e-6, "{sag_out} {sag_in}");

        let first_offset =
            |ds: &[Dimension]| ds.iter().map(|d| d.offset).fold(f64::INFINITY, f64::min);
        let bay_out = bottom(&house(out));
        let bay_in = bottom(&house(-out));
        // The wall-to-wall string breaks at the bay's tangent points: the
        // corner wall's thickness twice, the walls either side and the bay's
        // chord.
        let mut lens: Vec<f64> = bay_out
            .iter()
            .filter(|d| d.offset < 33.0 + sag_out)
            .map(|d| d.length())
            .collect();
        lens.sort_by(f64::total_cmp);
        assert_eq!(lens, vec![6.0, 6.0, 147.0, 147.0, 180.0]);
        // Bulging inward changes nothing; bulging out pushes every string
        // out by the sagitta.
        assert!((first_offset(&bay_in) - 32.0).abs() < 1e-9);
        assert!((first_offset(&bay_out) - (32.0 + sag_out)).abs() < 1e-6);
        // The overall still runs corner to corner.
        let overall: Vec<f64> = bay_out
            .iter()
            .filter(|d| d.offset > 33.0 + sag_out)
            .map(|d| d.length())
            .collect();
        assert_eq!(overall, vec![486.0]);
    }

    fn cab(x: f64, y: f64, width: f64, centers: Vec<(NkbaKind, f64)>) -> NkbaItem {
        NkbaItem {
            origin: Point::new(x, y),
            angle: 0.0,
            width,
            depth: 24.0,
            centers,
        }
    }

    #[test]
    fn nkba_dimensions_a_kitchen_run_to_cabinet_faces_and_the_sink_center() {
        // A 240" wall along y = 0 (faces at +-3), a left return wall and a
        // right return wall 114" past the last cabinet.
        let e = WallKind::Exterior;
        let walls = vec![
            wall(1, 0.0, 0.0, 240.0, 0.0, e),
            wall(2, 0.0, 0.0, 0.0, 144.0, e),
            wall(3, 240.0, 0.0, 240.0, 144.0, e),
        ];
        // Base cabinets against the wall face, from the left return's face.
        let items = vec![
            cab(3.0, 3.0, 30.0, vec![]),
            cab(33.0, 3.0, 36.0, vec![(NkbaKind::Sink, 18.0)]),
            cab(69.0, 3.0, 24.0, vec![(NkbaKind::Appliance, 12.0)]),
            cab(93.0, 3.0, 30.0, vec![]),
        ];
        let setup = NkbaSetup {
            first_offset: 12.0,
            spacing: 12.0,
            walls: &walls,
            reach: 150.0,
        };
        let dims = auto_nkba_dimensions(&items, &setup);
        assert!(dims.iter().all(|d| d.auto_group == AutoGroup::Nkba));
        // Every string runs along the cabinet fronts (y = 27) and is placed
        // on the front side.
        for d in &dims {
            assert!((d.start.y - 27.0).abs() < 1e-9 && (d.end.y - 27.0).abs() < 1e-9);
            assert!(d.end.x > d.start.x);
            let (a, _) = d.line_points();
            assert!(a.y > 27.0);
        }
        let string = |off: f64| -> Vec<(f64, f64)> {
            let mut v: Vec<(f64, f64)> = dims
                .iter()
                .filter(|d| (d.offset - off).abs() < 1e-9)
                .map(|d| (d.start.x, d.end.x))
                .collect();
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
            v
        };
        // Cabinet faces, then on to the right wall's surface (237).
        assert_eq!(
            string(12.0),
            vec![
                (3.0, 33.0),
                (33.0, 69.0),
                (69.0, 93.0),
                (93.0, 123.0),
                (123.0, 237.0)
            ]
        );
        // The sink center (33 + 18) and the dishwasher bay center (69 + 12),
        // measured from the walls.
        assert_eq!(string(24.0), vec![(3.0, 51.0), (51.0, 81.0), (81.0, 237.0)]);
        // The overall, wall face to wall face.
        assert_eq!(string(36.0), vec![(3.0, 237.0)]);
        // Out of reach of the right wall: the string stops at the last cabinet.
        let near = NkbaSetup {
            reach: 60.0,
            ..setup
        };
        let dims = auto_nkba_dimensions(&items, &near);
        let mut ends: Vec<f64> = dims.iter().map(|d| d.end.x).collect();
        ends.sort_by(f64::total_cmp);
        assert_eq!(ends.last().copied(), Some(123.0));
    }

    #[test]
    fn nkba_runs_split_by_gaps_and_turn_with_the_cabinets() {
        let setup = NkbaSetup {
            first_offset: 12.0,
            spacing: 12.0,
            walls: &[],
            reach: 60.0,
        };
        // Two cabinets with a 40" gap are two runs, each with its own
        // faces string (one segment: no separate overall).
        let items = vec![cab(0.0, 0.0, 30.0, vec![]), cab(70.0, 0.0, 30.0, vec![])];
        let dims = auto_nkba_dimensions(&items, &setup);
        assert_eq!(dims.len(), 2);
        assert!(dims.iter().all(|d| (d.length() - 30.0).abs() < 1e-9));
        // A run turned 90 degrees (against a wall on the right) dimensions
        // along y, on its front side (+x... rotated +y front -> -x).
        let turned = |x: f64, y: f64, w: f64| NkbaItem {
            origin: Point::new(x, y),
            angle: std::f64::consts::FRAC_PI_2,
            width: w,
            depth: 24.0,
            centers: vec![],
        };
        let items = vec![turned(100.0, 0.0, 30.0), turned(100.0, 30.0, 36.0)];
        let dims = auto_nkba_dimensions(&items, &setup);
        // Faces (two) and the overall.
        assert_eq!(dims.len(), 3);
        for d in &dims {
            assert!((d.start.x - 76.0).abs() < 1e-9, "{:?}", d.start);
            assert!(d.end.y > d.start.y);
            let (a, _) = d.line_points();
            assert!(a.x < 76.0, "front side is -x: {a:?}");
        }
    }

    #[test]
    fn reverse_aligns_and_distributes_dimension_lines() {
        let dim = |x0: f64, x1: f64, y: f64, off: f64| {
            Dimension::new(
                1,
                DimensionKind::Manual,
                Point::new(x0, y),
                Point::new(x1, y),
                off,
            )
        };
        // Reverse: the line lands on the other side, the same distance away.
        let mut d = dim(0.0, 100.0, 0.0, 24.0);
        d.hide_ext = [true, false];
        let before = d.line_points().0.y;
        d.reverse();
        assert_eq!((d.start.x, d.end.x), (100.0, 0.0));
        assert_eq!(d.hide_ext, [false, true]);
        assert!(
            (d.line_points().0.y + before).abs() < 1e-9,
            "{:?}",
            d.line_points()
        );
        assert!((d.length() - 100.0).abs() < 1e-9);
        // Align: three parallel dimensions, one of them reversed, all land on
        // the first one's line (y = 24).
        let mut v = vec![
            dim(0.0, 100.0, 0.0, 24.0),
            dim(10.0, 60.0, 5.0, 40.0),
            dim(80.0, 20.0, -10.0, -30.0),
        ];
        assert_eq!(align_dimensions(&mut v), 2);
        for d in &v {
            let (a, b) = d.line_points();
            assert!(
                (a.y - 24.0).abs() < 1e-9 && (b.y - 24.0).abs() < 1e-9,
                "{a:?}"
            );
        }
        // Not parallel: left alone.
        let mut turn = vec![
            dim(0.0, 100.0, 0.0, 24.0),
            Dimension::new(
                2,
                DimensionKind::Manual,
                Point::ZERO,
                Point::new(0.0, 50.0),
                10.0,
            ),
        ];
        assert_eq!(align_dimensions(&mut turn), 0);
        assert_eq!(turn[1].offset, 10.0);
        // Distribute: lines at 24, 30, 90 and 120 spread evenly (24, 56, 88,
        // 120 with the ends fixed; the order along the normal decides).
        let mut v = vec![
            dim(0.0, 100.0, 0.0, 24.0),
            dim(0.0, 40.0, 0.0, 120.0),
            dim(0.0, 60.0, 0.0, 30.0),
            dim(0.0, 80.0, 0.0, 90.0),
        ];
        assert_eq!(distribute_dimensions(&mut v), 2);
        let ys: Vec<f64> = v.iter().map(|d| d.line_points().0.y).collect();
        assert!((ys[0] - 24.0).abs() < 1e-9 && (ys[1] - 120.0).abs() < 1e-9);
        assert!(
            (ys[2] - 56.0).abs() < 1e-9 && (ys[3] - 88.0).abs() < 1e-9,
            "{ys:?}"
        );
        // Fewer than three: nothing to distribute.
        assert_eq!(distribute_dimensions(&mut v[..2]), 0);
        // Convert to Manual.
        let mut a = Dimension::new(
            1,
            DimensionKind::AutoExterior,
            Point::ZERO,
            Point::new(10.0, 0.0),
            1.0,
        );
        a.auto_group = AutoGroup::Nkba;
        assert!(a.convert_to_manual());
        assert_eq!(
            (a.kind, a.auto_group),
            (DimensionKind::Manual, AutoGroup::None)
        );
        assert!(!a.convert_to_manual());
    }

    #[test]
    fn extension_lines_can_be_hidden_per_point() {
        let mut d = Dimension::new(
            1,
            DimensionKind::Manual,
            Point::ZERO,
            Point::new(100.0, 0.0),
            24.0,
        );
        assert_eq!(d.visible_extension_lines().len(), 2);
        d.hide_ext = [true, false];
        let v = d.visible_extension_lines();
        assert_eq!(v.len(), 1);
        assert!(v[0].0.dist(Point::new(100.0, 0.0)) < 1e-9);
        // Old files without the new fields load.
        let json = serde_json::to_string(&d).unwrap();
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        for k in ["hide_ext", "auto_group", "text_style"] {
            v.as_object_mut().unwrap().remove(k);
        }
        let back: Dimension = serde_json::from_value(v).unwrap();
        assert_eq!(back.hide_ext, [false, false]);
        assert_eq!(back.auto_group, AutoGroup::None);
    }
    #[test]
    fn a_dimension_follows_the_defaults_until_it_sets_its_own_format() {
        let fmt = DimFormat::default();
        let mut d = Dimension::new(
            1,
            DimensionKind::Manual,
            Point::ZERO,
            Point::new(78.25, 0.0),
            12.0,
        );
        assert_eq!(d.label(&fmt), "6'-6 1/4\"");
        d.look.fraction = Some(2);
        assert_eq!(d.label(&fmt), "6'-6 1/2\"");
        d.look.fraction = None;
        d.look.unit_indicators = Some(false);
        assert_eq!(d.label(&fmt), "6-6 1/4");
        d.look.unit_indicators = Some(true);
        d.look.units = Some(LengthUnit::Inches);
        assert_eq!(d.label(&fmt), "78 1/4\"");
        d.look.units = Some(LengthUnit::DecimalFeet);
        d.look.decimals = Some(3);
        assert_eq!(d.label(&fmt), "6.521'");
        d.look.units = Some(LengthUnit::Millimeters);
        d.look.decimals = Some(0);
        d.look.unit_indicators = Some(false);
        assert_eq!(d.label(&fmt), "1988");
        // The typed text still wins.
        d.text_override = Some("EQ".into());
        assert_eq!(d.label(&fmt), "EQ");
    }

    #[test]
    fn zero_feet_can_be_suppressed() {
        let fmt = DimFormat::default();
        let mut d = Dimension::new(
            1,
            DimensionKind::Manual,
            Point::ZERO,
            Point::new(6.5, 0.0),
            12.0,
        );
        assert_eq!(d.label(&fmt), "0'-6 1/2\"");
        d.look.suppress_zero_feet = Some(true);
        assert_eq!(d.label(&fmt), "6 1/2\"");
        d.look.unit_indicators = Some(false);
        assert_eq!(d.label(&fmt), "6 1/2");
        // Whole feet keep their feet.
        d.end = Point::new(30.0, 0.0);
        d.look.unit_indicators = None;
        assert_eq!(d.label(&fmt), "2'-6\"");
        assert_eq!(suppress_zero_feet("-0'-3\"", true), "-3\"");
        assert_eq!(suppress_zero_feet("1'-3\"", true), "1'-3\"");
    }

    #[test]
    fn extension_segments_honor_gap_overshoot_and_a_fixed_length() {
        let m = Point::new(0.0, 0.0);
        let e = Point::new(0.0, 24.0);
        let (a, b) = extension_segment(m, e, 2.0, 3.0, None).unwrap();
        assert!(a.dist(Point::new(0.0, 2.0)) < 1e-9 && b.dist(Point::new(0.0, 27.0)) < 1e-9);
        // A fixed length counts back from the dimension line.
        let (a, b) = extension_segment(m, e, 2.0, 3.0, Some(10.0)).unwrap();
        assert!(a.dist(Point::new(0.0, 14.0)) < 1e-9 && b.dist(Point::new(0.0, 27.0)) < 1e-9);
        // A length longer than the run changes nothing.
        let (a, _) = extension_segment(m, e, 2.0, 0.0, Some(100.0)).unwrap();
        assert!(a.dist(Point::new(0.0, 2.0)) < 1e-9);
        assert!(extension_segment(m, m, 2.0, 3.0, None).is_none());
    }

    #[test]
    fn overrides_round_trip_and_old_files_load() {
        let mut d = Dimension::new(
            1,
            DimensionKind::Manual,
            Point::ZERO,
            Point::new(10.0, 0.0),
            1.0,
        );
        let plain = serde_json::to_string(&d).unwrap();
        assert!(!plain.contains("look"), "{plain}");
        d.look.arrow = Some(DimArrow::Arrow);
        d.look.arrow_filled = Some(true);
        d.look.ext_length = Some(8.0);
        let back: Dimension = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        assert_eq!(back, d);
        let old: Dimension = serde_json::from_str(&plain).unwrap();
        assert!(old.look.is_default());
        assert_eq!(DimArrow::from_name("Filled Arrow"), DimArrow::Arrow);
        assert_eq!(DimArrow::from_name(""), DimArrow::Tick);
        assert_eq!(DimArrow::from_name("Dot"), DimArrow::Dot);
    }
}
