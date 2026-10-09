//! Drawing Sheet Setup per view (File > Print > Drawing Sheet Setup, manual
//! pp. 1425-1432) and the print-related slots of a plan.
//!
//! Chief keeps a Drawing Sheet Setup for each kind of view: the plan views,
//! the cross section / elevation views, the CAD Details, the layout and the
//! Materials List each have their own sheet size, orientation, Drawing Scale,
//! Drawing Margins, printer and line weight scale; a new view starts from the
//! plan view's. [`PrintSetup`] is that table, plus the file's
//! [`WatermarkSettings`]. The sheet sizes the lists offer are program-wide
//! (`~/.plan-studio/sheetsizes.json`, see `plan_layout::SheetSizeFile`), so a
//! setup names its size and keeps the two sides with it.
//!
//! The Drawing Scale is two parts, `1/4" = 1'` or `1 mm = 50 mm`, with the
//! U.S. and metric units chosen independently.
//!
//! Units: paper inches except where a field says otherwise.

use crate::watermark::WatermarkSettings;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The kinds of view that keep a Drawing Sheet Setup and Print View settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ViewType {
    #[default]
    Plan,
    /// Cross section / elevation views.
    Elevation,
    CadDetail,
    Layout,
    MaterialsList,
}

impl ViewType {
    pub const ALL: [ViewType; 5] = [
        ViewType::Plan,
        ViewType::Elevation,
        ViewType::CadDetail,
        ViewType::Layout,
        ViewType::MaterialsList,
    ];

    /// Storage key.
    pub fn key(self) -> &'static str {
        match self {
            ViewType::Plan => "plan",
            ViewType::Elevation => "elevation",
            ViewType::CadDetail => "detail",
            ViewType::Layout => "layout",
            ViewType::MaterialsList => "materials",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ViewType::Plan => "Plan view",
            ViewType::Elevation => "Cross section / elevation",
            ViewType::CadDetail => "CAD Detail",
            ViewType::Layout => "Layout",
            ViewType::MaterialsList => "Materials List",
        }
    }

    /// Does the view scale (so a Drawing Scale means something)?
    pub fn scaled(self) -> bool {
        !matches!(self, ViewType::MaterialsList)
    }
}

/// A unit of the two-part Drawing Scale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LenUnit {
    Inch,
    #[default]
    Foot,
    Millimeter,
    Meter,
}

impl LenUnit {
    pub const ALL: [LenUnit; 4] = [
        LenUnit::Inch,
        LenUnit::Foot,
        LenUnit::Millimeter,
        LenUnit::Meter,
    ];

    pub fn to_inches(self) -> f64 {
        match self {
            LenUnit::Inch => 1.0,
            LenUnit::Foot => 12.0,
            LenUnit::Millimeter => 1.0 / 25.4,
            LenUnit::Meter => 1000.0 / 25.4,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            LenUnit::Inch => "in",
            LenUnit::Foot => "ft",
            LenUnit::Millimeter => "mm",
            LenUnit::Meter => "m",
        }
    }

    pub fn is_metric(self) -> bool {
        matches!(self, LenUnit::Millimeter | LenUnit::Meter)
    }
}

/// The Drawing Scale of a view, in two parts: `paper` of `paper_unit` on the
/// sheet stands for `real` of `real_unit` in the building.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrawingScale {
    pub paper: f64,
    pub paper_unit: LenUnit,
    pub real: f64,
    pub real_unit: LenUnit,
}

impl Default for DrawingScale {
    /// 1/4 in = 1 ft, the plan default.
    fn default() -> Self {
        Self::per_foot(0.25)
    }
}

impl DrawingScale {
    /// `inches` of paper per foot of building: 1/4 in = 1 ft is `per_foot(0.25)`.
    pub fn per_foot(inches: f64) -> Self {
        Self {
            paper: inches,
            paper_unit: LenUnit::Inch,
            real: 1.0,
            real_unit: LenUnit::Foot,
        }
    }

    /// 1:`n` in millimeters: `1 mm = n mm`.
    pub fn ratio_mm(n: f64) -> Self {
        Self {
            paper: 1.0,
            paper_unit: LenUnit::Millimeter,
            real: n,
            real_unit: LenUnit::Millimeter,
        }
    }

    /// Layouts: 1 in = 1 in (views carry their own scales).
    pub fn layout() -> Self {
        Self {
            paper: 1.0,
            paper_unit: LenUnit::Inch,
            real: 1.0,
            real_unit: LenUnit::Inch,
        }
    }

    /// Plan inches per paper inch (48 for 1/4 in = 1 ft).
    pub fn ratio(&self) -> f64 {
        (self.real * self.real_unit.to_inches()) / (self.paper * self.paper_unit.to_inches())
    }

    /// Paper inches per foot of building (the way `plan_docs::Scale` counts).
    pub fn inches_per_foot(&self) -> f64 {
        12.0 / self.ratio()
    }

    pub fn is_valid(&self) -> bool {
        self.paper.is_finite()
            && self.real.is_finite()
            && self.paper > 0.0
            && self.real > 0.0
            && self.ratio() >= 0.01
            && self.ratio() <= 100_000.0
    }

    /// The scale that draws `inches` of paper per foot of building, in inches
    /// and feet (or, when `metric`, as `1 mm = n mm`).
    pub fn from_inches_per_foot(inches: f64, metric: bool) -> Self {
        if metric {
            Self::ratio_mm(12.0 / inches.max(1e-6))
        } else {
            Self::per_foot(inches)
        }
    }

    /// `1/4" = 1'`, `1" = 10'`, `1 mm = 50 mm`, `1/4" = 1 m`.
    pub fn label(&self) -> String {
        format!(
            "{} = {}",
            part_text(self.paper, self.paper_unit),
            part_text(self.real, self.real_unit)
        )
    }
}

fn part_text(v: f64, unit: LenUnit) -> String {
    match unit {
        LenUnit::Inch => format!("{}\"", inch_text(v)),
        LenUnit::Foot => format!("{}'", plain_text(v)),
        LenUnit::Millimeter | LenUnit::Meter => format!("{} {}", plain_text(v), unit.label()),
    }
}

fn plain_text(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.3}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

/// Inches as a whole number and a fraction of 64ths when it is one:
/// `1-1/2`, `3/16`; a decimal otherwise.
fn inch_text(v: f64) -> String {
    let whole = v.floor();
    let frac = v - whole;
    for den in [2.0_f64, 4.0, 8.0, 16.0, 32.0, 64.0] {
        let n = (frac * den).round();
        if (frac * den - n).abs() < 1e-9 && n > 0.0 && n < den {
            let mut d = den as u32;
            let mut nn = n as u32;
            while nn % 2 == 0 && d % 2 == 0 {
                nn /= 2;
                d /= 2;
            }
            return if whole > 0.0 {
                format!("{}-{nn}/{d}", whole as i64)
            } else {
                format!("{nn}/{d}")
            };
        }
    }
    plain_text(v)
}

/// A sheet size by name with its sides (landscape: the long side first).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SheetDims {
    pub name: String,
    pub long_in: f64,
    pub short_in: f64,
}

impl Default for SheetDims {
    /// ARCH D (24 x 36), the construction set default.
    fn default() -> Self {
        Self::new("ARCH D (24 x 36)", 36.0, 24.0)
    }
}

impl SheetDims {
    pub fn new(name: impl Into<String>, a: f64, b: f64) -> Self {
        Self {
            name: name.into(),
            long_in: a.max(b),
            short_in: a.min(b),
        }
    }
}

/// Printing margins of the sheet, `[top, bottom, left, right]`, paper inches.
pub type Margins = [f64; 4];

/// The Advanced Line Weights of a view.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LineWeightSetup {
    /// Use 1 for all line weights (Home Designer compatibility): every line
    /// prints at one thickness, the same at any drawing scale.
    pub single_weight: bool,
    /// Line Weight Scale: weight 1 is 1/`denominator` of `unit`. Chief's
    /// default is 1 = 1/100 mm.
    pub denominator: f64,
    pub unit_mm: bool,
}

impl Default for LineWeightSetup {
    fn default() -> Self {
        Self {
            single_weight: false,
            denominator: 100.0,
            unit_mm: true,
        }
    }
}

impl LineWeightSetup {
    /// Millimeters one weight number stands for.
    pub fn weight_mm(&self) -> f64 {
        let unit = if self.unit_mm { 1.0 } else { 25.4 };
        unit / self.denominator.max(1.0)
    }

    /// The factor on a pen weight made for Chief's default scale (1 = 1/100 mm).
    pub fn factor(&self) -> f64 {
        self.weight_mm() / 0.01
    }

    /// Thickness of a line under "Use 1 for all line weights": weight 1 on a
    /// 1/300 inch scale, in points.
    pub const SINGLE_WEIGHT_PT: f64 = 72.0 / 300.0;
}

/// Drawing Sheet Setup of one kind of view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewSheet {
    pub sheet: SheetDims,
    pub portrait: bool,
    pub scale: DrawingScale,
    /// Drawing Margins `[top, bottom, left, right]`.
    pub margins_in: Margins,
    /// Printer for View (`None`: the system default).
    pub printer: Option<String>,
    /// Remember Print Settings after Printing.
    pub remember_print_settings: bool,
    pub weights: LineWeightSetup,
}

impl Default for ViewSheet {
    fn default() -> Self {
        Self {
            sheet: SheetDims::default(),
            portrait: false,
            scale: DrawingScale::default(),
            margins_in: [0.25; 4],
            printer: None,
            remember_print_settings: true,
            weights: LineWeightSetup::default(),
        }
    }
}

impl ViewSheet {
    /// The sheet's width and height as drawn: turned upright when portrait.
    pub fn inches(&self) -> (f64, f64) {
        if self.portrait {
            (self.sheet.short_in, self.sheet.long_in)
        } else {
            (self.sheet.long_in, self.sheet.short_in)
        }
    }

    /// The sheet's footprint on the ground at the Drawing Scale, plan inches.
    pub fn world_size(&self) -> (f64, f64) {
        let (w, h) = self.inches();
        let k = self.scale.ratio();
        (w * k, h * k)
    }

    /// Why the answers cannot be used, if they cannot.
    pub fn problem(&self) -> Option<&'static str> {
        if self.sheet.long_in < 2.0 || self.sheet.short_in < 2.0 {
            return Some("The sheet must be at least 2 inches each way");
        }
        if !self.scale.is_valid() {
            return Some("The Drawing Scale needs a number above zero on both sides");
        }
        let (w, h) = self.inches();
        let [t, b, l, r] = self.margins_in;
        if self.margins_in.iter().any(|m| !(0.0..=100.0).contains(m)) {
            return Some("Margins must be zero or more");
        }
        if l + r >= w || t + b >= h {
            return Some("The margins leave no printable area");
        }
        if self.weights.denominator < 1.0 {
            return Some("The line weight scale needs a denominator of 1 or more");
        }
        None
    }
}

/// The print-related settings of a plan: a Drawing Sheet Setup for each kind
/// of view and the Watermark.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrintSetup {
    /// Setups by [`ViewType::key`]; a missing view type uses the plan's.
    pub views: BTreeMap<String, ViewSheet>,
    pub watermark: WatermarkSettings,
}

impl PrintSetup {
    /// The setup the plan has not touched.
    pub fn is_default(&self) -> bool {
        *self == PrintSetup::default()
    }

    /// The Drawing Sheet Setup of a kind of view: its own, else (a new view
    /// takes the plan view's) the plan's, with the Drawing Scale a layout
    /// always has: 1 in = 1 in.
    pub fn sheet_for(&self, view: ViewType) -> ViewSheet {
        let base = self
            .views
            .get(view.key())
            .or_else(|| self.views.get(ViewType::Plan.key()))
            .cloned()
            .unwrap_or_default();
        match (view, self.views.contains_key(view.key())) {
            (ViewType::Layout, false) => ViewSheet {
                scale: DrawingScale::layout(),
                ..base
            },
            _ => base,
        }
    }

    /// Does the kind of view have a setup of its own?
    pub fn has_own(&self, view: ViewType) -> bool {
        self.views.contains_key(view.key())
    }

    /// Stores the setup of a kind of view.
    pub fn set_sheet(&mut self, view: ViewType, sheet: ViewSheet) {
        self.views.insert(view.key().to_string(), sheet);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_part_scales_give_the_plan_inches_per_paper_inch() {
        assert!((DrawingScale::default().ratio() - 48.0).abs() < 1e-9);
        assert!((DrawingScale::default().inches_per_foot() - 0.25).abs() < 1e-12);
        let eighth = DrawingScale::per_foot(0.125);
        assert!((eighth.ratio() - 96.0).abs() < 1e-9);
        let metric = DrawingScale::ratio_mm(50.0);
        assert!((metric.ratio() - 50.0).abs() < 1e-9);
        // 1 mm = 50 mm is 12/50 paper inch per foot.
        assert!((metric.inches_per_foot() - 0.24).abs() < 1e-9);
        // Units are independent: 1/4 in = 1 m.
        let mixed = DrawingScale {
            paper: 0.25,
            paper_unit: LenUnit::Inch,
            real: 1.0,
            real_unit: LenUnit::Meter,
        };
        assert!((mixed.ratio() - 1000.0 / 25.4 / 0.25).abs() < 1e-9);
        assert!((DrawingScale::layout().ratio() - 1.0).abs() < 1e-12);
        assert!(!DrawingScale {
            paper: 0.0,
            ..DrawingScale::default()
        }
        .is_valid());
    }

    #[test]
    fn scale_labels_read_like_the_drawing_notes() {
        assert_eq!(DrawingScale::default().label(), "1/4\" = 1'");
        assert_eq!(DrawingScale::per_foot(0.1875).label(), "3/16\" = 1'");
        assert_eq!(DrawingScale::per_foot(1.5).label(), "1-1/2\" = 1'");
        let ten = DrawingScale {
            paper: 1.0,
            paper_unit: LenUnit::Inch,
            real: 10.0,
            real_unit: LenUnit::Foot,
        };
        assert_eq!(ten.label(), "1\" = 10'");
        assert_eq!(DrawingScale::ratio_mm(50.0).label(), "1 mm = 50 mm");
        assert_eq!(
            DrawingScale::from_inches_per_foot(0.25, false),
            DrawingScale::default()
        );
        assert!((DrawingScale::from_inches_per_foot(0.24, true).ratio() - 50.0).abs() < 1e-9);
    }

    #[test]
    fn a_new_view_takes_the_plan_setup_and_a_layout_is_one_to_one() {
        let mut s = PrintSetup::default();
        let plan = ViewSheet {
            sheet: SheetDims::new("ARCH C (18 x 24)", 24.0, 18.0),
            scale: DrawingScale::per_foot(0.125),
            ..ViewSheet::default()
        };
        s.set_sheet(ViewType::Plan, plan.clone());
        let elev = s.sheet_for(ViewType::Elevation);
        assert_eq!(elev, plan, "inherits the plan view's");
        assert!(!s.has_own(ViewType::Elevation));
        let lay = s.sheet_for(ViewType::Layout);
        assert_eq!(lay.sheet, plan.sheet);
        assert!(
            (lay.scale.ratio() - 1.0).abs() < 1e-12,
            "layouts are 1 in = 1 in"
        );
        // Once it has its own, that wins.
        let own = ViewSheet {
            scale: DrawingScale::per_foot(0.5),
            ..plan.clone()
        };
        s.set_sheet(ViewType::Elevation, own.clone());
        assert_eq!(s.sheet_for(ViewType::Elevation), own);
        assert_eq!(s.sheet_for(ViewType::Plan), plan);
    }

    #[test]
    fn the_sheet_turns_upright_and_has_a_footprint_on_the_ground() {
        let mut v = ViewSheet::default();
        assert_eq!(v.inches(), (36.0, 24.0));
        // 36 x 24 at 1/4 in = 1 ft covers 144 x 96 feet.
        assert_eq!(v.world_size(), (36.0 * 48.0, 24.0 * 48.0));
        v.portrait = true;
        assert_eq!(v.inches(), (24.0, 36.0));
        assert!(v.problem().is_none());
        v.margins_in = [20.0, 20.0, 0.0, 0.0];
        assert!(v.problem().is_some());
    }

    #[test]
    fn line_weight_scale_defaults_to_one_hundredth_of_a_millimeter() {
        let w = LineWeightSetup::default();
        assert!((w.weight_mm() - 0.01).abs() < 1e-12);
        assert!((w.factor() - 1.0).abs() < 1e-12);
        // 1 = 1/1000 in is 0.0254 mm: 2.54x the default thickness.
        let inch = LineWeightSetup {
            single_weight: false,
            denominator: 1000.0,
            unit_mm: false,
        };
        assert!((inch.factor() - 2.54).abs() < 1e-9);
    }

    #[test]
    fn the_setup_round_trips_and_old_files_load() {
        let mut s = PrintSetup::default();
        assert!(s.is_default());
        s.set_sheet(ViewType::CadDetail, ViewSheet::default());
        s.watermark.toggle("layout");
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<PrintSetup>(&json).unwrap(), s);
        assert_eq!(
            serde_json::from_str::<PrintSetup>("{}").unwrap(),
            PrintSetup::default()
        );
    }
}
