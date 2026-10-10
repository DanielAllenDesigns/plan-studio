//! Operations on a layout box's scale and placement: scale text and ratios,
//! the too-big-for-the-sheet check, the content map (source space to paper),
//! and the edit tools Pan/Scale, Recenter, Scale to Fit and Rescale Layout
//! View (manual pp. 1405 to 1407, 1412).

use crate::boxview::{nominal_scale, CameraLink, PlotOptions, ScaleMode, UpdateKind, ViewArt};
use crate::canvas::turn_point;
use crate::extent::plan_target;
use crate::model::{BoxSource, Layout, LayoutBox, LABEL_GAP_IN};
use plan_core::{Point, Project};
use plan_docs::Scale;

// --------------------------------------------------------------- scales --

/// Paper inches per inch of the building at `scale`: 1 in = 100 ft is
/// exactly 1/1200.
pub fn paper_per_model(scale: Scale) -> f64 {
    scale.inches_per_foot() / 12.0
}

/// Inches of the building one inch of paper stands for (the ratio of a
/// `1:n` scale): 1 in = 100 ft is 1200.
pub fn model_per_paper(scale: Scale) -> f64 {
    12.0 / scale.inches_per_foot()
}

fn fraction(t: &str) -> Option<f64> {
    let (n, d) = t.split_once('/')?;
    let (n, d) = (n.trim().parse::<f64>().ok()?, d.trim().parse::<f64>().ok()?);
    (d != 0.0).then_some(n / d)
}

/// `1/4`, `0.25`, `1-1/2` or `1 1/2` as a number.
fn number(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Some((whole, f)) = s.rsplit_once(['-', ' ']) {
        if f.contains('/') {
            return Some(whole.trim().parse::<f64>().ok()? + fraction(f)?);
        }
    }
    if s.contains('/') {
        fraction(s)
    } else {
        s.parse().ok()
    }
}

/// A length such as `1/4"`, `3 in`, `50'`, `1'-6"`, `20 ft`, `1 m` or
/// `50 mm`, in inches. A bare number counts as inches.
fn length_in(s: &str) -> Option<f64> {
    let s = s.trim();
    if let Some((feet, rest)) = s.split_once(['\'', '\u{2019}', '\u{2032}']) {
        let feet = number(feet)?;
        let rest = rest.trim().trim_start_matches('-').trim();
        let inches = if rest.is_empty() {
            0.0
        } else {
            length_in(rest)?
        };
        return Some(feet * 12.0 + inches);
    }
    const UNITS: [(&str, f64); 11] = [
        ("inches", 1.0),
        ("inch", 1.0),
        ("in", 1.0),
        ("\"", 1.0),
        ("\u{201d}", 1.0),
        ("feet", 12.0),
        ("foot", 12.0),
        ("ft", 12.0),
        ("mm", 1.0 / 25.4),
        ("cm", 1.0 / 2.54),
        ("m", 1000.0 / 25.4),
    ];
    for (u, f) in UNITS {
        if let Some(num) = s.strip_suffix(u) {
            return Some(number(num)? * f);
        }
    }
    number(s)
}

/// The paper inches per foot a typed scale means: `1:1200`, a bare `1200`
/// (a ratio), `1/4" = 1'`, `1 in = 80 ft`, `1 m = 50 m`. `None` for text that
/// is none of these or is not positive.
pub fn parse_scale_text(text: &str) -> Option<f64> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let ipf = if let Some((l, r)) = t.split_once(':') {
        let (l, r) = (l.trim().parse::<f64>().ok()?, r.trim().parse::<f64>().ok()?);
        if l <= 0.0 || r <= 0.0 {
            return None;
        }
        12.0 * l / r
    } else if let Some((l, r)) = t.split_once('=') {
        let (paper, model) = (length_in(l)?, length_in(r)?);
        if paper <= 0.0 || model <= 0.0 {
            return None;
        }
        paper / (model / 12.0)
    } else {
        let n: f64 = t.parse().ok()?;
        if n <= 0.0 {
            return None;
        }
        12.0 / n
    };
    (ipf.is_finite() && (1e-4..=1e3).contains(&ipf)).then_some(ipf)
}

/// The scale to keep for a factor of `ipf` paper inches per foot: a scale on
/// the lists when it is one, a whole `1:n` ratio next, else the nearest
/// named scale with the exact factor kept as [`ScaleMode::Custom`].
pub fn scale_for_ipf(ipf: f64) -> (Scale, ScaleMode) {
    if let Some(s) = Scale::choices()
        .into_iter()
        .find(|s| (s.inches_per_foot() - ipf).abs() < 1e-9)
    {
        return (s, ScaleMode::Named);
    }
    let n = 12.0 / ipf.max(1e-9);
    if n >= 1.0 && (n - n.round()).abs() < 1e-6 {
        return (Scale::Ratio(n.round() as u32), ScaleMode::Named);
    }
    (nominal_scale(ipf), ScaleMode::Custom(ipf))
}

/// The scales Send to Layout steps through when a view does not fit, largest
/// drawing first: the architectural scales, then the site scales
/// (1 in = 30 ft down to 1 in = 100 ft).
pub fn fit_scales() -> Vec<Scale> {
    Scale::ALL.into_iter().chain(Scale::SITE).collect()
}

/// The next smaller scale of [`fit_scales`].
pub fn next_smaller(s: Scale) -> Option<Scale> {
    let all = fit_scales();
    let i = all.iter().position(|x| *x == s)?;
    all.get(i + 1).copied()
}

/// Does a box of `size_in` fit the page's drawing area?
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitCheck {
    /// The box, paper inches.
    pub size_in: (f64, f64),
    /// The drawing area it goes into (less the caption strip), paper inches.
    pub area_in: (f64, f64),
    /// Wider or taller than the area: the "view is too big for the sheet"
    /// warning.
    pub too_large: bool,
}

/// Checks a box of `size_in` against the drawing area of sheet number `page`
/// (the layout's when the page does not exist yet), leaving room for the
/// caption when `labelled`.
pub fn fit_check(
    layout: &Layout,
    page: Option<u32>,
    size_in: (f64, f64),
    labelled: bool,
) -> FitCheck {
    let (lo, hi) = page
        .and_then(|n| layout.page(n))
        .map_or_else(|| layout.drawing_area(), |p| layout.page_drawing_area(p));
    let gap = if labelled { LABEL_GAP_IN } else { 0.0 };
    let area_in = (hi.x - lo.x, hi.y - lo.y - gap);
    FitCheck {
        size_in,
        area_in,
        too_large: size_in.0 > area_in.0 + 1e-9 || size_in.1 > area_in.1 + 1e-9,
    }
}

/// The largest scale of [`fit_scales`], no larger than `max`, at which a
/// view `frame_in` inches of the building across fits the page's drawing
/// area; the smallest site scale when none does.
pub fn largest_scale_that_fits(
    layout: &Layout,
    page: Option<u32>,
    frame_in: (f64, f64),
    max: Scale,
    labelled: bool,
) -> Scale {
    let mut s = max;
    loop {
        let k = paper_per_model(s);
        let check = fit_check(layout, page, (frame_in.0 * k, frame_in.1 * k), labelled);
        if !check.too_large {
            return s;
        }
        match next_smaller(s) {
            Some(n) => s = n,
            None => return s,
        }
    }
}

// -------------------------------------------------------- content map --

/// Where a box puts its contents: maps points of the view's own space (inches
/// of the building) to paper and back, with the box's scale, pan and quarter
/// turn. The renderer, the hit tests and the layout line tools all use it, so
/// they agree to the point.
#[derive(Debug, Clone, Copy)]
pub struct ContentMap {
    lo: Point,
    /// PDF points per inch of the view.
    k: f64,
    /// Where `lo` lands before the quarter turn, PDF points.
    origin: (f64, f64),
    /// The box centre, PDF points (the turn is about it).
    centre: (f64, f64),
    turns: u8,
    /// The box rectangle as the contents see it (a quarter turn swaps its
    /// sides about the centre), PDF points.
    pub rect: [f64; 4],
    /// The box rectangle on the page, PDF points.
    pub real: [f64; 4],
    /// Size of the contents, PDF points.
    pub content: (f64, f64),
}

impl ContentMap {
    /// The map of box `b` showing the view space rectangle `lo`..`hi`.
    pub fn new(b: &LayoutBox, lo: Point, hi: Point) -> ContentMap {
        let real = b.bounds_in().map(|v| v * 72.0);
        let turns = b.quarter_turns();
        let centre = ((real[0] + real[2]) * 0.5, (real[1] + real[3]) * 0.5);
        let rect = if turns % 2 == 1 {
            let (hw, hh) = ((real[3] - real[1]) * 0.5, (real[2] - real[0]) * 0.5);
            [centre.0 - hw, centre.1 - hh, centre.0 + hw, centre.1 + hh]
        } else {
            real
        };
        let k = b.points_per_inch();
        let (bw, bh) = (rect[2] - rect[0], rect[3] - rect[1]);
        let (cw, ch) = ((hi.x - lo.x) * k, (hi.y - lo.y) * k);
        let (ox, oy) = if cw <= bw + 1e-6 && ch <= bh + 1e-6 {
            (rect[0] + (bw - cw) * 0.5, rect[1] + (bh - ch) * 0.5)
        } else {
            (rect[0], rect[3] - ch)
        };
        let pan = b.view.pan_in;
        ContentMap {
            lo,
            k,
            origin: (ox + pan.0 * 72.0, oy + pan.1 * 72.0),
            centre,
            turns,
            rect,
            real,
            content: (cw, ch),
        }
    }

    /// PDF points per inch of the view.
    pub fn k(&self) -> f64 {
        self.k
    }

    /// A view point before the box turns its contents, PDF points.
    pub fn unturned(&self, p: Point) -> (f64, f64) {
        (
            self.origin.0 + (p.x - self.lo.x) * self.k,
            self.origin.1 + (p.y - self.lo.y) * self.k,
        )
    }

    /// A view point on the page, PDF points.
    pub fn to_pt(&self, p: Point) -> (f64, f64) {
        turn_point(self.unturned(p), self.centre, self.turns)
    }

    /// A view point on the page, paper inches.
    pub fn to_paper(&self, p: Point) -> Point {
        let q = self.to_pt(p);
        Point::new(q.0 / 72.0, q.1 / 72.0)
    }

    /// The view point under a page point (paper inches).
    pub fn to_source(&self, paper: Point) -> Point {
        let back = turn_point(
            (paper.x * 72.0, paper.y * 72.0),
            self.centre,
            (4 - self.turns % 4) % 4,
        );
        Point::new(
            self.lo.x + (back.0 - self.origin.0) / self.k,
            self.lo.y + (back.1 - self.origin.1) / self.k,
        )
    }
}

/// A page-space vector (paper inches) as the box's contents see it, before a
/// quarter turn: what a drag on the page means to the contents.
pub fn unturn_vector(turns: u8, v: (f64, f64)) -> (f64, f64) {
    match turns % 4 {
        0 => v,
        1 => (v.1, -v.0),
        2 => (-v.0, -v.1),
        _ => (-v.1, v.0),
    }
}

/// The box's size as its contents see it (sides swapped by a quarter turn),
/// paper inches.
pub fn content_rect_size(b: &LayoutBox) -> (f64, f64) {
    let (w, h) = b.size_in();
    if b.quarter_turns() % 2 == 1 {
        (h, w)
    } else {
        (w, h)
    }
}

// ----------------------------------------------------------- the tools --

/// Pan/Scale Layout Box, panning: moves the contents inside the box by a
/// drag of `d` paper inches on the page.
pub fn pan_by(b: &mut LayoutBox, d: (f64, f64)) {
    let (dx, dy) = unturn_vector(b.quarter_turns(), d);
    b.view.pan_in.0 += dx;
    b.view.pan_in.1 += dy;
}

/// How a rescale sets the scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NewScale {
    /// No Scale: the view keeps its size on the sheet but has no scale.
    NoScale,
    /// A named scale.
    Named(Scale),
    /// Paper inches per foot, as typed or fitted.
    PerFoot(f64),
}

/// Rescale Layout View (Change Scale) and the typed scale of Pan/Scale: sets
/// the scale, and, when the box's contents follow the scale (the default
/// `resize_with_scale`), resizes the box about its centre by the same factor
/// so it keeps showing the same part of the view. Returns whether anything
/// changed.
pub fn rescale(b: &mut LayoutBox, to: NewScale) -> bool {
    let old = b.effective_ipf();
    let before = (b.scale, b.view.scale_mode);
    let new = match to {
        NewScale::NoScale => {
            b.view.scale_mode = ScaleMode::NoScale(old);
            return before != (b.scale, b.view.scale_mode);
        }
        NewScale::Named(s) => {
            b.scale = s;
            b.view.scale_mode = ScaleMode::Named;
            s.inches_per_foot()
        }
        NewScale::PerFoot(ipf) => {
            let (s, mode) = scale_for_ipf(ipf);
            b.scale = s;
            b.view.scale_mode = mode;
            match mode {
                ScaleMode::Custom(v) => v,
                _ => s.inches_per_foot(),
            }
        }
    };
    let f = new / old.max(1e-12);
    if (f - 1.0).abs() > 1e-12 {
        if b.view.resize_with_scale {
            scale_rect(b, f);
        }
        b.view.pan_in = (b.view.pan_in.0 * f, b.view.pan_in.1 * f);
    }
    before != (b.scale, b.view.scale_mode) || (f - 1.0).abs() > 1e-12
}

/// Resizes the box about its centre by `f`.
fn scale_rect(b: &mut LayoutBox, f: f64) {
    let [x0, y0, x1, y1] = b.bounds_in();
    let (cx, cy) = ((x0 + x1) * 0.5, (y0 + y1) * 0.5);
    let (hw, hh) = ((x1 - x0) * 0.5 * f, (y1 - y0) * 0.5 * f);
    b.rect_in = (Point::new(cx - hw, cy - hh), Point::new(cx + hw, cy + hh));
}

/// Recenter Layout Box Contents: centers the contents in the box without
/// changing the scale. `frame_in` is the size of the view, inches of the
/// building.
pub fn recenter(b: &mut LayoutBox, frame_in: (f64, f64)) {
    let (rw, rh) = content_rect_size(b);
    let k = b.effective_ipf() / 12.0;
    let (cw, ch) = (frame_in.0 * k, frame_in.1 * k);
    // Where the box places contents by default, against where centred is.
    let fits = cw * 72.0 <= rw * 72.0 + 1e-6 && ch * 72.0 <= rh * 72.0 + 1e-6;
    let legacy = if fits {
        ((rw - cw) * 0.5, (rh - ch) * 0.5)
    } else {
        (0.0, rh - ch)
    };
    let centred = ((rw - cw) * 0.5, (rh - ch) * 0.5);
    b.view.pan_in = (centred.0 - legacy.0, centred.1 - legacy.1);
}

/// Scale Layout Box Contents to Fit: the scale at which the whole view fills
/// the box's present extents (a scale on no list is fine), centred.
/// `frame_in` is the size of the view, inches of the building. Returns the
/// factor chosen, paper inches per foot.
pub fn scale_to_fit(b: &mut LayoutBox, frame_in: (f64, f64)) -> f64 {
    let (rw, rh) = content_rect_size(b);
    let (fw, fh) = (frame_in.0.max(1e-6) / 12.0, frame_in.1.max(1e-6) / 12.0);
    let ipf = (rw / fw).min(rh / fh);
    let keep = b.view.resize_with_scale;
    b.view.resize_with_scale = false;
    rescale(b, NewScale::PerFoot(ipf));
    b.view.resize_with_scale = keep;
    b.view.pan_in = (0.0, 0.0);
    recenter(b, frame_in);
    ipf
}

/// Resizing a non-scaled view by its corner (the Alternate edit behavior):
/// the box takes `new_rect` and the view grows or shrinks with it. Other
/// handles only change the box, cropping the view.
pub fn resize_no_scale(b: &mut LayoutBox, new_rect: [f64; 4]) {
    let (w0, _) = b.size_in();
    let w1 = new_rect[2] - new_rect[0];
    b.rect_in = (
        Point::new(new_rect[0], new_rect[1]),
        Point::new(new_rect[2], new_rect[3]),
    );
    if let ScaleMode::NoScale(ipf) = b.view.scale_mode {
        if w0 > 1e-9 && w1 > 1e-9 {
            let f = w1 / w0;
            b.view.scale_mode = ScaleMode::NoScale(ipf * f);
            b.view.pan_in = (b.view.pan_in.0 * f, b.view.pan_in.1 * f);
        }
    }
}

// ------------------------------------------------------------ kinds --

impl LayoutBox {
    /// Dynamic, semi-dynamic, static or Plot Line (manual p. 1401).
    pub fn update_kind(&self) -> UpdateKind {
        match &self.source {
            BoxSource::ImageData { .. } | BoxSource::Image { .. } => UpdateKind::Static,
            BoxSource::Perspective { .. } => UpdateKind::SemiDynamic,
            BoxSource::Elevation { .. } | BoxSource::Section { .. } | BoxSource::Camera { .. } => {
                match self.view.camera {
                    CameraLink::Always => UpdateKind::Dynamic,
                    CameraLink::OnDemand => UpdateKind::SemiDynamic,
                    CameraLink::PlotLines => UpdateKind::PlotLine,
                }
            }
            _ => UpdateKind::Dynamic,
        }
    }

    /// Is the box a Plot Lines view?
    pub fn is_plot_lines(&self) -> bool {
        self.update_kind() == UpdateKind::PlotLine
    }

    /// Camera View Options apply to cross sections, elevations and camera
    /// views.
    pub fn has_camera_options(&self) -> bool {
        matches!(
            self.source,
            BoxSource::Elevation { .. } | BoxSource::Section { .. } | BoxSource::Camera { .. }
        )
    }
}

// ------------------------------------------------------ missing views --

/// What is wrong with a box whose view has gone (manual p. 1405, "Missing
/// Layout Views").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Missing {
    /// The floor, camera or saved plan view the box is linked to is gone.
    View(String),
    /// The layer set the box uses is gone (the plan's own set shows in its
    /// place).
    LayerSet(String),
}

impl Missing {
    pub fn text(&self) -> String {
        match self {
            Missing::View(what) => format!("Missing layout view: {what}"),
            Missing::LayerSet(name) => format!("Missing layer set: {name}"),
        }
    }
}

/// Is the view the box was sent from still in the plan?
pub fn missing_view(b: &LayoutBox, project: &Project) -> Option<Missing> {
    match &b.source {
        BoxSource::PlanView { floor, .. } => {
            if let Some(name) = &b.view.saved_view {
                if project.plan_view(name).is_none() {
                    return Some(Missing::View(format!("saved plan view {name}")));
                }
            }
            if plan_target(&b.source, &b.view, project).is_none() {
                return Some(Missing::View(format!("floor {}", floor + 1)));
            }
            // A box's own layer set is a label as much as a link (plans
            // are sent with the name of the layer display they were drawn
            // with); only the layer set of a linked saved plan view can be
            // missing.
            if let Some(sv) = b
                .view
                .saved_view
                .as_deref()
                .and_then(|n| project.plan_view(n))
            {
                if !sv.layer_set.is_empty() && project.layer_sets.get(&sv.layer_set).is_none() {
                    return Some(Missing::LayerSet(sv.layer_set.clone()));
                }
            }
            None
        }
        BoxSource::Camera { camera_id } | BoxSource::Perspective { camera_id } => project
            .camera(*camera_id)
            .is_none()
            .then(|| Missing::View(format!("camera {camera_id}"))),
        _ => None,
    }
}

// ------------------------------------------------------ send options --

/// What part of the view Send to Layout sends.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SendExtent {
    /// Entire Plan/View: everything the view shows (Fill Window).
    #[default]
    EntireView,
    /// Current Screen: only what is on screen, `[x0, y0, x1, y1]` in the
    /// view's own units.
    CurrentScreen([f64; 4]),
    /// Current Screen As Image: a static picture of what is on screen.
    AsImage,
}

/// The Send Options, Camera View Options and Scaling answers of the Send to
/// Layout dialog beyond the view, page and scale.
#[derive(Debug, Clone, PartialEq)]
pub struct SendOptions {
    pub extent: SendExtent,
    /// Link Saved Plan View: the saved plan view the box follows.
    pub link_saved_view: Option<String>,
    pub camera: CameraLink,
    pub plot: PlotOptions,
    /// Use Layout Line Scaling.
    pub layout_line_scaling: bool,
    /// Fit to Sheet (No Scale): about half the sheet, no scale.
    pub fit_to_sheet: bool,
    /// Snap to Active CAD Point: a box placed by a click snaps to the
    /// nearest corner or end point already on the page.
    pub snap_to_point: bool,
}

impl Default for SendOptions {
    fn default() -> Self {
        Self {
            extent: SendExtent::EntireView,
            link_saved_view: None,
            camera: CameraLink::Always,
            plot: PlotOptions::default(),
            layout_line_scaling: true,
            fit_to_sheet: false,
            snap_to_point: false,
        }
    }
}

impl SendOptions {
    /// Writes the options onto the new box `b` (its source decides which
    /// apply).
    pub fn apply_to(&self, b: &mut LayoutBox) {
        let v = &mut b.view;
        v.layout_line_scaling = self.layout_line_scaling;
        match self.extent {
            SendExtent::EntireView => {
                v.fill_window = matches!(b.source, BoxSource::PlanView { .. })
            }
            SendExtent::CurrentScreen(e) => v.extent = Some(e),
            SendExtent::AsImage => {}
        }
        if matches!(b.source, BoxSource::PlanView { .. }) {
            v.saved_view = self.link_saved_view.clone();
        }
        if matches!(
            b.source,
            BoxSource::Elevation { .. } | BoxSource::Section { .. } | BoxSource::Camera { .. }
        ) {
            v.camera = self.camera;
            v.plot = self.plot.clone();
        }
    }
}

/// Snap to Active CAD Point: the corner, end or vertex already on page
/// `page` nearest to `at` within `reach` paper inches, else `at`.
pub fn snap_point(layout: &Layout, page: u32, at: Point, reach: f64) -> Point {
    snap_point_except(layout, page, at, reach, None)
}

/// [`snap_point`] leaving out the corners of box `skip` (the box being
/// placed must not snap to itself).
pub fn snap_point_except(
    layout: &Layout,
    page: u32,
    at: Point,
    reach: f64,
    skip: Option<plan_core::Id>,
) -> Point {
    let Some(p) = layout.page(page) else {
        return at;
    };
    let mut best: Option<(f64, Point)> = None;
    let mut consider = |q: Point| {
        let d = q.dist(at);
        if d <= reach && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, q));
        }
    };
    for b in p.boxes.iter().filter(|b| Some(b.id) != skip) {
        let [x0, y0, x1, y1] = b.bounds_in();
        for q in [
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ] {
            consider(q);
        }
    }
    for o in &p.cad {
        for q in cad_points(&o.item) {
            consider(q);
        }
    }
    best.map_or(at, |(_, q)| q)
}

fn cad_points(item: &plan_core::CadItem) -> Vec<Point> {
    use plan_core::CadItem as C;
    match item {
        C::Line { a, b } => vec![*a, *b],
        C::Polyline { points, .. } => points.clone(),
        C::Text { pos, .. } => vec![*pos],
        other => {
            let (a, b) = plan_core::CadObject {
                id: 0,
                layer: String::new(),
                item: other.clone(),
            }
            .bounds();
            vec![a, b]
        }
    }
}

/// The floor and layer set a saved plan view gives its layout boxes (the floor
/// is `None` for a view that shows whichever floor is current): what Unlink
/// Saved Plan View leaves a box showing.
pub fn saved_view_target(project: &Project, name: &str) -> Option<(Option<usize>, String)> {
    let v = project.plan_view(name)?;
    Some((v.floor, v.layer_set.clone()))
}

/// Removes the picture a semi-dynamic or Plot Lines view keeps, so it draws
/// live again.
pub fn drop_art(b: &mut LayoutBox) -> Option<ViewArt> {
    b.view.art.take()
}
