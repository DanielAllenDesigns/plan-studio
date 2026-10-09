//! What a layout box remembers about the view it shows: Box Scale, the
//! linked view, Plot Lines, the box label and its border and fill (Chief's
//! Layout Box Specification, manual pp. 1409 to 1413).
//!
//! All of it lives in [`BoxView`], one `#[serde(default)]` slot of
//! [`LayoutBox`], so layouts saved before it load unchanged. The operations
//! on it (pan, rescale, update, label text, plot-line editing) are in
//! `boxops`.

use crate::model::LayoutBox;
use plan_core::callout::{CalloutShape, ViewLink};
use plan_core::fill_styles::FillStyle;
use plan_core::{Id, LineStyle, Point};
use plan_docs::Scale;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

fn yes() -> bool {
    true
}

/// How a box's contents are scaled (the Box Scale panel).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum ScaleMode {
    /// The box's named [`LayoutBox::scale`].
    #[default]
    Named,
    /// A scale that is on no list (Scale Layout Box Contents to Fit, a typed
    /// ratio): paper inches per foot of the building.
    Custom(f64),
    /// "No Scale" (Fit to Sheet): the view has a size on the sheet but no
    /// scale; paper inches per foot as it is drawn now. Resizing the box by
    /// its corner resizes the view with it.
    NoScale(f64),
}

/// Camera View Options of the Linked View panel: how a cross section,
/// elevation or camera view is linked to the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CameraLink {
    /// Live View, Always Update: dynamic.
    #[default]
    Always,
    /// Live View, Update on Demand: semi-dynamic.
    OnDemand,
    /// Plot Lines: a semi-dynamic Vector View of edge and pattern lines that
    /// only you update.
    PlotLines,
}

impl CameraLink {
    pub const ALL: [CameraLink; 3] = [
        CameraLink::Always,
        CameraLink::OnDemand,
        CameraLink::PlotLines,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CameraLink::Always => "Live View, Always Update",
            CameraLink::OnDemand => "Live View, Update on Demand",
            CameraLink::PlotLines => "Plot Lines",
        }
    }
}

/// The four kinds of layout view (manual p. 1401).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateKind {
    /// Follows the plan at once.
    Dynamic,
    /// Fully updates when printed or when you ask.
    SemiDynamic,
    /// A snapshot: replaced, never updated.
    Static,
    /// Plot Line views: linked, but only updated by you.
    PlotLine,
}

impl UpdateKind {
    pub fn label(self) -> &'static str {
        match self {
            UpdateKind::Dynamic => "Dynamic",
            UpdateKind::SemiDynamic => "Semi-Dynamic",
            UpdateKind::Static => "Static",
            UpdateKind::PlotLine => "Plot Line",
        }
    }
}

/// Edge Line or Pattern Line (Layout Line Specification).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineType {
    /// An edge of a surface of a 3D object.
    Edge,
    /// A line of a material pattern (brick, siding, shingles).
    Pattern,
}

impl LineType {
    pub fn label(self) -> &'static str {
        match self {
            LineType::Edge => "Edge Line",
            LineType::Pattern => "Pattern Line",
        }
    }
}

/// How heavily a generated edge line is drawn by default (the weight class
/// the vector view gave it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tier {
    Light,
    Medium,
    Heavy,
    Cut,
    /// A hidden edge (dashed).
    Hidden,
}

/// One line of a Plot Lines view, in drawing space (inches of the building,
/// as the vector view had it). `None` properties follow the view's defaults
/// ("Use Default Weight / Style / Color").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlotLine {
    pub id: Id,
    pub a: Point,
    pub b: Point,
    pub kind: LineType,
    pub tier: Tier,
    /// The color a pattern line was made with (its material's); edge lines
    /// have none.
    #[serde(default)]
    pub base_color: Option<[u8; 3]>,
    #[serde(default)]
    pub weight_pt: Option<f64>,
    #[serde(default)]
    pub style: Option<LineStyle>,
    #[serde(default)]
    pub color: Option<[u8; 3]>,
    /// Drawn by you with Edit Layout Lines (the view's update removes it
    /// with the rest).
    #[serde(default)]
    pub added: bool,
}

/// What a [`PlotFill`] stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FillRole {
    /// The poche of a section cut.
    Cut,
    /// A cast shadow.
    Shadow,
    /// A surface in its material's color: drawn only when Color Fill is on.
    Material,
}

/// A filled area of a Plot Lines view: the poche of a cut, a shadow, or a
/// material's color when Color Fill is on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlotFill {
    pub polygon: Vec<Point>,
    pub rgb: [u8; 3],
    pub role: FillRole,
}

/// The picture a semi-dynamic or Plot Lines view keeps: it is drawn from
/// this until the view is updated.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ViewArt {
    pub lines: Vec<PlotLine>,
    pub fills: Vec<PlotFill>,
    /// Annotation texts of the vector view: `(anchor, text)`.
    #[serde(default)]
    pub texts: Vec<(Point, String)>,
    /// `(min, max)` of everything in the picture, drawing inches.
    pub bounds: (Point, Point),
    /// The id the next line drawn gets.
    #[serde(default)]
    pub next_id: Id,
    /// Counts every change, so screen caches know the picture moved on.
    #[serde(default)]
    pub rev: u64,
}

impl ViewArt {
    /// The line with `id`.
    pub fn line(&self, id: Id) -> Option<&PlotLine> {
        self.lines.iter().find(|l| l.id == id)
    }

    /// Recomputes [`bounds`](Self::bounds) from the lines and fills.
    pub fn update_bounds(&mut self) {
        let pts = self
            .lines
            .iter()
            .flat_map(|l| [l.a, l.b])
            .chain(self.fills.iter().flat_map(|f| f.polygon.iter().copied()))
            .chain(self.texts.iter().map(|t| t.0));
        let mut lo = Point::new(f64::MAX, f64::MAX);
        let mut hi = Point::new(f64::MIN, f64::MIN);
        let mut any = false;
        for p in pts {
            any = true;
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        self.bounds = if any {
            (lo, hi)
        } else {
            (Point::ZERO, Point::ZERO)
        };
    }
}

/// Edge Line Defaults and Pattern Line Defaults of the Linked View panel,
/// and Color Fill.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlotOptions {
    /// Colors that correspond to the materials used in the model.
    pub color_fill: bool,
    /// Use the weight and color below for every edge line (unchecked: the
    /// layer and object settings of the vector view).
    pub use_edge_defaults: bool,
    pub edge_weight_pt: f64,
    pub edge_color: [u8; 3],
    pub use_pattern_defaults: bool,
    pub pattern_weight_pt: f64,
    pub pattern_color: [u8; 3],
}

impl Default for PlotOptions {
    fn default() -> Self {
        Self {
            color_fill: false,
            use_edge_defaults: false,
            edge_weight_pt: 0.7,
            edge_color: [0, 0, 0],
            use_pattern_defaults: false,
            pattern_weight_pt: 0.15,
            pattern_color: [90, 90, 90],
        }
    }
}

/// Where a box label sits against its box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LabelPos {
    #[default]
    BottomLeft,
    BottomCenter,
    BottomRight,
    TopLeft,
    TopCenter,
    TopRight,
}

impl LabelPos {
    pub const ALL: [LabelPos; 6] = [
        LabelPos::BottomLeft,
        LabelPos::BottomCenter,
        LabelPos::BottomRight,
        LabelPos::TopLeft,
        LabelPos::TopCenter,
        LabelPos::TopRight,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LabelPos::BottomLeft => "Bottom Left",
            LabelPos::BottomCenter => "Bottom Center",
            LabelPos::BottomRight => "Bottom Right",
            LabelPos::TopLeft => "Top Left",
            LabelPos::TopCenter => "Top Center",
            LabelPos::TopRight => "Top Right",
        }
    }

    /// Is the label above the box?
    pub fn on_top(self) -> bool {
        matches!(
            self,
            LabelPos::TopLeft | LabelPos::TopCenter | LabelPos::TopRight
        )
    }
}

/// The Label panel beyond the text itself ([`LayoutBox::label`]): a callout
/// or marker shape, a link to a view or layout page, and where it sits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoxLabel {
    /// `None`: a plain text label. Any other shape draws the shape on the
    /// Layout Box Labels layer with [`callout_text`](Self::callout_text)
    /// inside it and the label text beside it.
    pub shape: CalloutShape,
    /// Text inside the shape; the callout macros expand in it.
    pub callout_text: String,
    /// The view or layout page the label refers to; the label macros report
    /// it.
    pub link: Option<ViewLink>,
    pub position: LabelPos,
    /// Moved by its edit handle, paper inches from [`position`](Self::position).
    pub offset_in: (f64, f64),
    /// The scale note under a scaled view's label.
    pub show_scale: bool,
}

impl Default for BoxLabel {
    fn default() -> Self {
        Self {
            shape: CalloutShape::None,
            callout_text: "%referenced_view_callout_label%".to_string(),
            link: None,
            position: LabelPos::BottomLeft,
            offset_in: (0.0, 0.0),
            show_scale: true,
        }
    }
}

/// The border of one box beyond its layer: `None` follows the Layout Box
/// Borders layer.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct BorderStyle {
    pub color: Option<[u8; 3]>,
    pub weight_pt: Option<f64>,
    pub dash: Option<LineStyle>,
}

/// Everything a layout box remembers about its view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoxView {
    pub scale_mode: ScaleMode,
    /// Use Layout Line Scaling: lines look like lines of the same weight
    /// drawn on the page. Unchecked, weights and dashes scale with the view.
    #[serde(default = "yes")]
    pub layout_line_scaling: bool,
    /// Scale Layout Box Contents Only: changing the scale resizes the box to
    /// match (unchecked, the box keeps its size).
    #[serde(default = "yes")]
    pub resize_with_scale: bool,
    /// Pan/Scale Layout Box: the contents moved from where the box places
    /// them, paper inches (content space, before a quarter turn).
    pub pan_in: (f64, f64),
    /// Current Screen: the part of the view to show, as `[x0, y0, x1, y1]`
    /// in the view's own units; `None` shows the whole view.
    pub extent: Option<[f64; 4]>,
    /// Entire Plan/View: the extent of everything the plan shows (Fill
    /// Window) instead of the walls and dimensions.
    pub fill_window: bool,
    /// Link Saved Plan View: the box follows this saved plan view's floor
    /// and layer set. `None` uses the box's own floor and layer set.
    pub saved_view: Option<String>,
    pub camera: CameraLink,
    pub plot: PlotOptions,
    /// Show Color; unchecked prints the view in grays.
    #[serde(default = "yes")]
    pub show_color: bool,
    /// Poché: a dark fill on the tops of the walls in a plan view.
    pub poche: bool,
    /// Current Default Set of an unsaved plan view (name).
    pub default_set: String,
    /// Ignore Invalid Links: no caution symbol on a missing view.
    pub ignore_missing: bool,
    /// The picture a semi-dynamic or Plot Lines view keeps.
    pub art: Option<ViewArt>,
    /// Fill Style of the box (shows when the borders show).
    pub fill: Option<FillStyle>,
    pub border: BorderStyle,
    pub label: BoxLabel,
}

impl Default for BoxView {
    fn default() -> Self {
        Self {
            scale_mode: ScaleMode::Named,
            layout_line_scaling: true,
            resize_with_scale: true,
            pan_in: (0.0, 0.0),
            extent: None,
            fill_window: false,
            saved_view: None,
            camera: CameraLink::Always,
            plot: PlotOptions::default(),
            show_color: true,
            poche: false,
            default_set: String::new(),
            ignore_missing: false,
            art: None,
            fill: None,
            border: BorderStyle::default(),
            label: BoxLabel::default(),
        }
    }
}

impl BoxView {
    /// A hash that changes whenever anything that is drawn changes (the
    /// picture counts by its revision, not line by line).
    pub fn cache_key(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let small = (
            &self.scale_mode,
            self.layout_line_scaling,
            self.pan_in,
            &self.extent,
            self.fill_window,
            &self.saved_view,
            self.camera,
            &self.plot,
        );
        let rest = (
            self.show_color,
            self.poche,
            &self.default_set,
            &self.fill,
            &self.border,
            &self.label,
        );
        format!("{small:?}{rest:?}").hash(&mut h);
        self.art
            .as_ref()
            .map(|a| (a.rev, a.lines.len(), a.fills.len()))
            .hash(&mut h);
        h.finish()
    }

    /// Does the view keep a picture (semi-dynamic and Plot Lines views)?
    pub fn has_art(&self) -> bool {
        self.art.is_some()
    }
}

impl LayoutBox {
    /// Paper inches per foot of the building the contents are drawn at:
    /// the named scale, or the custom / unscaled factor.
    pub fn effective_ipf(&self) -> f64 {
        match self.view.scale_mode {
            ScaleMode::Named => self.scale.inches_per_foot(),
            ScaleMode::Custom(i) | ScaleMode::NoScale(i) => i,
        }
    }

    /// PDF points of paper per inch of the building.
    pub fn points_per_inch(&self) -> f64 {
        72.0 * self.effective_ipf() / 12.0
    }

    /// Is the view drawn at No Scale?
    pub fn is_no_scale(&self) -> bool {
        matches!(self.view.scale_mode, ScaleMode::NoScale(_))
    }

    /// The scale note under the label: `1/4" = 1'-0"`, `1:37`, or
    /// `NOT TO SCALE`.
    pub fn scale_note(&self) -> String {
        match self.view.scale_mode {
            ScaleMode::Named => self.scale.label().to_string(),
            ScaleMode::Custom(ipf) => custom_label(ipf),
            ScaleMode::NoScale(_) => "NOT TO SCALE".to_string(),
        }
    }
}

/// The note of a scale that is on no list: `1:n` when the ratio is whole,
/// else paper inches per foot.
pub fn custom_label(ipf: f64) -> String {
    let n = 12.0 / ipf.max(1e-9);
    if (n - n.round()).abs() < 1e-6 {
        format!("1:{}", n.round() as u64)
    } else {
        let t = format!("{ipf:.4}");
        let t = t.trim_end_matches('0').trim_end_matches('.');
        format!("{t}\" = 1'-0\"")
    }
}

/// The nearest named scale to a custom factor (the box keeps one as its
/// nominal [`LayoutBox::scale`]).
pub fn nominal_scale(ipf: f64) -> Scale {
    Scale::from_inches_per_foot(ipf)
}
