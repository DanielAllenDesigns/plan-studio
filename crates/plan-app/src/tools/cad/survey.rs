//! Survey-style drafting (Round 16 brief 06; CAD-109, CAD-112 to CAD-114,
//! CAD-118, CAD-134): the Number Style in force, the Current Point, the
//! models behind New CAD Line and New CAD Point, live Show Length / Show
//! Angle labels, and building a plot plan from a traverse of bearings and
//! distances.

use super::arcs::{input_arc, input_arc_chord, Curve, Extent, InputArc};
use super::{add_cad_items, point_items};
use crate::editor::site_view;
use crate::editor::{EditorContext, ObjectRef};
use plan_core::bearing::{
    azimuth_dir, azimuth_of, math_to_azimuth, parse_angle, AngleStyle, Course, NumberStyle,
};
use plan_core::cad::{CadItem, CadLabels};
use plan_core::geometry::Point;
use plan_core::Id;
use std::cell::{Cell, RefCell};

thread_local! {
    static STYLE: RefCell<NumberStyle> = RefCell::new(NumberStyle::default());
    static CURRENT: Cell<Option<Point>> = const { Cell::new(None) };
    /// Temporary points in creation order, each with the CAD objects drawn.
    static POINTS: RefCell<Vec<(Point, Vec<Id>)>> = const { RefCell::new(Vec::new()) };
    static PREVIOUS: Cell<Option<(Point, Point)>> = const { Cell::new(None) };
}

// ----- commands (CAD > Survey Entry) -----

pub const NUMBER_STYLE: &str = "cad.survey.number_style";
pub const NEW_LINE: &str = "cad.survey.new_line";
pub const NEW_ARC: &str = "cad.survey.new_arc";
pub const DELETE_POINT: &str = "cad.survey.delete_point";

/// The CAD > Survey Entry menu: `(label, command)`.
pub const MENU: [(&str, &str); 4] = [
    ("New CAD Line\u{2026}", NEW_LINE),
    ("New CAD Arc\u{2026}", NEW_ARC),
    ("Delete Last Temporary Point", DELETE_POINT),
    ("Number Style\u{2026}", NUMBER_STYLE),
];

pub fn is_command(id: &str) -> bool {
    id.starts_with("cad.survey.")
}

pub fn run_command(cx: &mut EditorContext, id: &str) {
    match id {
        NUMBER_STYLE => crate::dialogs::number_style::open(),
        NEW_LINE => crate::dialogs::input_line::open(),
        NEW_ARC => crate::dialogs::input_arc::open(),
        DELETE_POINT => {
            if !delete_current_point(cx) {
                cx.status = "There is no temporary point to delete".into();
            }
        }
        _ => {}
    }
}

// ----- Number Style in force -----

/// The Number Style and Angle Style (Preferences > Number Style).
pub fn number_style() -> NumberStyle {
    STYLE.with(|s| s.borrow().clone())
}

pub fn set_number_style(style: NumberStyle) {
    STYLE.with(|s| *s.borrow_mut() = style);
}

thread_local! {
    /// The style of the plan as of the last [`sync_number_style`].
    static SEEN: RefCell<NumberStyle> = RefCell::new(NumberStyle::default());
}

/// Brings the style in force up to date with the plan's own: called every
/// frame (`EditorContext::refresh`), so opening a plan, a new plan, Undo and
/// Redo change it. A style set only for the session ([`set_number_style`])
/// stays until the plan's own changes.
pub fn sync_number_style(project: &plan_core::Project) {
    let changed = SEEN.with(|s| {
        let mut seen = s.borrow_mut();
        let changed = *seen != project.number_style;
        if changed {
            seen.clone_from(&project.number_style);
        }
        changed
    });
    if changed {
        set_number_style(project.number_style.clone());
    }
}

/// Stores `style` in the plan (one undo step) and puts it in force.
pub fn store_number_style(cx: &mut EditorContext, style: NumberStyle) -> bool {
    if cx.project.number_style == style && number_style() == style {
        return false;
    }
    cx.begin_change("Number Style");
    cx.project.number_style = style.clone();
    cx.mark_dirty();
    SEEN.with(|s| s.borrow_mut().clone_from(&style));
    set_number_style(style);
    true
}

thread_local! {
    /// The style the live labels use: the Number Style, with the CAD
    /// Defaults' "Displayed Line Length" and "Display Line Angles As" laid
    /// over it when they were changed.
    static LABEL_STYLE: RefCell<Option<NumberStyle>> = const { RefCell::new(None) };
}

/// The style the live labels are drawn in.
pub fn label_style() -> NumberStyle {
    LABEL_STYLE
        .with(|s| s.borrow().clone())
        .unwrap_or_else(number_style)
}

/// The Number Style with the CAD Defaults page's line length and angle
/// choices laid over it (CAD-119, Default Settings > CAD > General CAD).
/// A choice left at the built-in value follows the Number Style.
pub fn overlay_cad_defaults(base: &NumberStyle, d: &plan_core::PlanDefaults) -> NumberStyle {
    use plan_core::units::LengthUnit;
    let mut s = base.clone();
    if let Some(v) = d.page_value("cad.general.length_format") {
        s.length.unit = match v.text().as_str() {
            "Feet" => LengthUnit::DecimalFeet,
            "Inches" => LengthUnit::Inches,
            "Meters" => LengthUnit::Meters,
            "Centimeters" => LengthUnit::Centimeters,
            "Millimeters" => LengthUnit::Millimeters,
            _ => LengthUnit::FeetInches,
        };
    }
    if let Some(v) = d.page_value("cad.general.length_accuracy") {
        let t = v.text();
        let denom = t.strip_prefix("1/").and_then(|n| n.parse::<u32>().ok());
        s.length.fraction_denominator = denom.unwrap_or(1);
    }
    if let Some(v) = d.page_value("cad.general.angle_format") {
        let style = match v.text().as_str() {
            "Bearing" => AngleStyle::Quadrant,
            "Azimuth" => AngleStyle::Azimuth,
            _ => AngleStyle::Degrees,
        };
        if s.angle.style != style {
            s.angle.style = style;
            s.angle.decimals = if style == AngleStyle::Degrees { 2 } else { 0 };
        }
    }
    s
}

/// A length typed in a CAD field, in inches.
pub fn parse_length(text: &str) -> Option<f64> {
    number_style().parse_length(text)
}

/// An angle typed in a CAD field (a degree value, a quadrant bearing or an
/// azimuth), as degrees counter-clockwise from east.
pub fn parse_angle_text(text: &str) -> Option<f64> {
    parse_angle(text, number_style().angle.style)
}

// ----- the Current Point (CAD-113) -----

/// The latest created or selected temporary point.
pub fn current_point() -> Option<Point> {
    CURRENT.with(|c| c.get())
}

pub fn set_current_point(p: Option<Point>) {
    CURRENT.with(|c| c.set(p));
}

/// The previous line entered in New CAD Line, for "relative to previous".
pub fn previous_line() -> Option<(Point, Point)> {
    PREVIOUS.with(|c| c.get())
}

/// Forgets the Current Point and the points kept for Delete (a new plan).
pub fn reset() {
    set_current_point(None);
    POINTS.with(|p| p.borrow_mut().clear());
    PREVIOUS.with(|c| c.set(None));
}

/// Marks a temporary point the user selected as the Current Point.
pub fn select_point(p: Point) {
    set_current_point(Some(p));
}

/// Once a frame (`EditorContext::refresh`): the plan's Number Style, and the
/// Current Point follows a temporary point the user selected.
pub fn on_refresh(cx: &EditorContext) {
    sync_number_style(&cx.project);
    let label = overlay_cad_defaults(&number_style(), &cx.defaults);
    LABEL_STYLE.with(|s| {
        let mut s = s.borrow_mut();
        if s.as_ref() != Some(&label) {
            *s = Some(label);
        }
    });
    if let Some(ObjectRef::Cad(id)) = cx.selection.single() {
        let hit = POINTS.with(|v| {
            v.borrow()
                .iter()
                .find(|(_, ids)| ids.contains(&id))
                .map(|(p, _)| *p)
        });
        if let Some(p) = hit {
            if current_point().is_none_or(|c| c.dist(p) > 1e-6) {
                select_point(p);
            }
        }
    }
}

/// Delete with nothing selected: removes the latest temporary point (the
/// Current Point), as Chief does. Returns whether it removed one.
pub fn delete_key(cx: &mut EditorContext) -> bool {
    cx.selection.is_empty() && delete_current_point(cx)
}

// ----- New CAD Point (CAD-112) -----

/// How a point is placed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointSpec {
    Absolute {
        x: f64,
        y: f64,
    },
    /// Offset from the Current Point.
    Relative {
        dx: f64,
        dy: f64,
    },
    /// Distance and angle (degrees counter-clockwise from east) from the
    /// Current Point; the angle comes from [`parse_angle_text`].
    Polar {
        dist: f64,
        angle: f64,
    },
}

/// Place Point clicks a temporary point, Input Point types one (this dialog)
/// and a Point Marker is a temporary point with a circle (DECISIONS 126).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointKind {
    Place,
    Input,
    Marker,
}

pub fn resolve_point(spec: PointSpec, current: Option<Point>) -> Result<Point, &'static str> {
    let base = |what| current.ok_or(what);
    match spec {
        PointSpec::Absolute { x, y } => Ok(Point::new(x, y)),
        PointSpec::Relative { dx, dy } => {
            Ok(base("There is no Current Point yet")?.add(Point::new(dx, dy)))
        }
        PointSpec::Polar { dist, angle } => {
            let a = angle.to_radians();
            Ok(
                base("There is no Current Point yet")?
                    .add(Point::new(a.cos(), a.sin()).scale(dist)),
            )
        }
    }
}

/// Creates a temporary point (one undo step) and makes it the Current Point.
pub fn enter_point(
    cx: &mut EditorContext,
    spec: PointSpec,
    kind: PointKind,
) -> Result<Point, String> {
    let p = resolve_point(spec, current_point()).map_err(String::from)?;
    let items = point_items(p, kind == PointKind::Marker);
    let label = match kind {
        PointKind::Marker => "Draw Point Marker",
        _ => "Draw Point",
    };
    super::ensure_layer(&mut cx.project, super::TEMP_POINT_LAYER, [200, 0, 200], 13);
    let ids = add_cad_items(cx, super::TEMP_POINT_LAYER, items, label)
        .ok_or_else(|| "The Temporary Points layer is locked".to_string())?;
    POINTS.with(|v| v.borrow_mut().push((p, ids)));
    set_current_point(Some(p));
    Ok(p)
}

/// Drops the points Undo took away and re-reads the places of those it moved
/// (a point's cross is centred on it).
fn sync_points(cx: &EditorContext) {
    POINTS.with(|v| {
        v.borrow_mut().retain_mut(|(p, ids)| {
            let Some(first) = ids.first() else {
                return false;
            };
            match cx.floor().cad.iter().find(|o| o.id == *first).map(|o| &o.item) {
                Some(CadItem::Line { a, b }) => {
                    *p = Point::lerp(*a, *b, 0.5);
                    true
                }
                _ => false,
            }
        });
        let last = v.borrow().last().map(|(p, _)| *p);
        if let (Some(cur), Some(last)) = (current_point(), last) {
            // The Current Point followed a point that no longer exists.
            if !v.borrow().iter().any(|(p, _)| p.dist(cur) < 1e-6) {
                set_current_point(Some(last));
            }
        }
    });
}

/// Delete with a temporary point current: removes the points one at a time
/// in reverse order of creation (one undo step each). Returns whether one
/// was removed.
pub fn delete_current_point(cx: &mut EditorContext) -> bool {
    sync_points(cx);
    let Some((_, ids)) = POINTS.with(|v| v.borrow_mut().pop()) else {
        return false;
    };
    cx.begin_change("Delete Point");
    let fl = cx.floor;
    for id in ids {
        cx.project.remove_cad(fl, id);
    }
    cx.selection.clear();
    cx.mark_dirty();
    set_current_point(POINTS.with(|v| v.borrow().last().map(|(p, _)| *p)));
    true
}

// ----- New CAD Line (CAD-112, CAD-134) -----

/// Where the line starts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StartSpec {
    Absolute {
        x: f64,
        y: f64,
    },
    /// Offset from the Current Point.
    Relative {
        dx: f64,
        dy: f64,
    },
    Polar {
        dist: f64,
        angle: f64,
    },
}

/// Where the line ends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EndSpec {
    Absolute {
        x: f64,
        y: f64,
    },
    /// Offset from the start.
    Relative {
        dx: f64,
        dy: f64,
    },
    /// Distance and angle from the start (degrees counter-clockwise from
    /// east).
    Polar {
        dist: f64,
        angle: f64,
    },
    /// Distance, turning `turn` degrees counter-clockwise from the direction
    /// of the previous line.
    FromPrevious {
        dist: f64,
        turn: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineSpec {
    pub start: StartSpec,
    pub end: EndSpec,
}

impl LineSpec {
    /// A course from the Current Point: an azimuth and a distance.
    pub fn course(c: Course) -> Self {
        Self {
            start: StartSpec::Relative { dx: 0.0, dy: 0.0 },
            end: EndSpec::Polar {
                dist: c.distance,
                angle: plan_core::bearing::azimuth_to_math(c.azimuth),
            },
        }
    }
}

/// Where a line or arc starts.
pub fn resolve_start(start: StartSpec, current: Option<Point>) -> Result<Point, &'static str> {
    match start {
        StartSpec::Absolute { x, y } => Ok(Point::new(x, y)),
        StartSpec::Relative { dx, dy } => resolve_point(PointSpec::Relative { dx, dy }, current),
        StartSpec::Polar { dist, angle } => {
            resolve_point(PointSpec::Polar { dist, angle }, current)
        }
    }
}

pub fn resolve_line(
    spec: LineSpec,
    current: Option<Point>,
    previous: Option<(Point, Point)>,
) -> Result<(Point, Point), &'static str> {
    let start = resolve_start(spec.start, current)?;
    let polar = |dist: f64, angle: f64| {
        let a = angle.to_radians();
        start.add(Point::new(a.cos(), a.sin()).scale(dist))
    };
    let end = match spec.end {
        EndSpec::Absolute { x, y } => Point::new(x, y),
        EndSpec::Relative { dx, dy } => start.add(Point::new(dx, dy)),
        EndSpec::Polar { dist, angle } => polar(dist, angle),
        EndSpec::FromPrevious { dist, turn } => {
            let (a, b) = previous.ok_or("There is no previous line yet")?;
            polar(dist, b.sub(a).angle().to_degrees() + turn)
        }
    };
    if start.dist(end) < 1e-6 {
        return Err("The line has no length");
    }
    Ok((start, end))
}

/// Draws the line a New CAD Line dialog describes (one undo step) and makes
/// its end the Current Point, so Next can continue from it.
pub fn enter_line(cx: &mut EditorContext, spec: LineSpec) -> Result<(Point, Point), String> {
    let (a, b) = resolve_line(spec, current_point(), previous_line()).map_err(String::from)?;
    let layer = cx.project.layers.current_cad_layer();
    add_cad_items(cx, &layer, vec![CadItem::Line { a, b }], "Draw Line")
        .ok_or_else(|| "The CAD layer is locked".to_string())?;
    set_current_point(Some(b));
    PREVIOUS.with(|c| c.set(Some((a, b))));
    Ok((a, b))
}

// ----- New CAD Arc (CAD-8) -----

/// The direction an arc leaves its start in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DirSpec {
    /// Degrees counter-clockwise from east (a bearing is read by
    /// [`parse_angle_text`]).
    Angle(f64),
    /// The direction the previous line or arc ended in, turned `turn`
    /// degrees counter-clockwise.
    Previous { turn: f64 },
}

/// What a New CAD Arc dialog describes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArcSpec {
    pub start: StartSpec,
    pub direction: DirSpec,
    pub radius: f64,
    pub curve: Curve,
    pub extent: Extent,
    /// The direction is the arc's chord direction, not its start tangent
    /// (only for [`DirSpec::Angle`]).
    pub chord: bool,
}

pub fn resolve_arc(
    spec: ArcSpec,
    current: Option<Point>,
    previous: Option<(Point, Point)>,
) -> Result<InputArc, &'static str> {
    let start = resolve_start(spec.start, current)?;
    let dir = match spec.direction {
        DirSpec::Angle(a) => a,
        DirSpec::Previous { turn } => {
            let (a, b) = previous.ok_or("There is no previous line or arc yet")?;
            b.sub(a).angle().to_degrees() + turn
        }
    };
    if spec.chord && matches!(spec.direction, DirSpec::Angle(_)) {
        return input_arc_chord(start, dir, spec.radius, spec.curve, spec.extent);
    }
    input_arc(start, dir, spec.radius, spec.curve, spec.extent)
}

/// Draws the arc a New CAD Arc dialog describes (one undo step) and makes
/// its end the Current Point, with the direction it ends in the "previous"
/// direction of the next line or arc.
pub fn enter_arc(cx: &mut EditorContext, spec: ArcSpec) -> Result<InputArc, String> {
    let arc = resolve_arc(spec, current_point(), previous_line()).map_err(String::from)?;
    let layer = cx.project.layers.current_cad_layer();
    add_cad_items(cx, &layer, vec![arc.item.clone()], "Draw Arc")
        .ok_or_else(|| "The CAD layer is locked".to_string())?;
    set_current_point(Some(arc.end));
    let heading = Point::new(arc.end_dir.to_radians().cos(), arc.end_dir.to_radians().sin());
    PREVIOUS.with(|c| c.set(Some((arc.end.sub(heading), arc.end))));
    Ok(arc)
}

// ----- Move Point (CAD-112) -----

/// The temporary point within `tol` inches of `at`, nearest first.
pub fn temporary_point_at(cx: &EditorContext, at: Point, tol: f64) -> Option<Point> {
    sync_points(cx);
    POINTS.with(|v| {
        v.borrow()
            .iter()
            .map(|(p, _)| *p)
            .filter(|p| p.dist(at) <= tol)
            .min_by(|a, b| a.dist(at).total_cmp(&b.dist(at)))
    })
}

/// The temporary point that CAD object `id` is part of (its cross or
/// marker circle), if any.
pub fn temporary_point_of(cx: &EditorContext, id: Id) -> Option<Point> {
    sync_points(cx);
    POINTS.with(|v| {
        v.borrow()
            .iter()
            .find(|(_, ids)| ids.contains(&id))
            .map(|(p, _)| *p)
    })
}

/// Where the Move Point dialog puts the point `from`: Absolute is the new
/// location, Relative and Polar are steps from where it is.
pub fn resolve_move(from: Point, spec: PointSpec) -> Point {
    match spec {
        PointSpec::Absolute { x, y } => Point::new(x, y),
        PointSpec::Relative { dx, dy } => from.add(Point::new(dx, dy)),
        PointSpec::Polar { dist, angle } => {
            let a = angle.to_radians();
            from.add(Point::new(a.cos(), a.sin()).scale(dist))
        }
    }
}

/// Moves the temporary point `from` (one undo step); it becomes the Current
/// Point.
pub fn move_point(cx: &mut EditorContext, from: Point, spec: PointSpec) -> Result<Point, String> {
    sync_points(cx);
    let idx = POINTS
        .with(|v| v.borrow().iter().position(|(p, _)| p.dist(from) < 1e-6))
        .ok_or("That is not a temporary point")?;
    let to = resolve_move(from, spec);
    if cx.layers().is_locked(super::TEMP_POINT_LAYER) {
        return Err("The Temporary Points layer is locked".into());
    }
    let delta = to.sub(from);
    let ids = POINTS.with(|v| v.borrow()[idx].1.clone());
    cx.begin_change("Move Point");
    let fl = cx.floor;
    for o in cx.project.floors[fl].cad.iter_mut() {
        if ids.contains(&o.id) {
            o.item = plan_core::cad::translated(&o.item, delta);
        }
    }
    POINTS.with(|v| v.borrow_mut()[idx].0 = to);
    set_current_point(Some(to));
    cx.mark_dirty();
    Ok(to)
}

/// Reads one traverse course typed as a bearing (or angle) and a distance.
pub fn parse_course(bearing: &str, distance: &str) -> Result<Course, String> {
    let math = parse_angle_text(bearing).ok_or_else(|| format!("\"{bearing}\" is not an angle"))?;
    let dist = parse_length(distance).ok_or_else(|| format!("\"{distance}\" is not a length"))?;
    if dist <= 0.0 {
        return Err("A course needs a distance above zero".into());
    }
    Ok(Course {
        azimuth: math_to_azimuth(math),
        distance: dist,
    })
}

/// Enters a traverse from `start` as one CAD line per course, each its own
/// undo step (as in Next chaining), and returns the line ids.
pub fn enter_traverse(
    cx: &mut EditorContext,
    start: Point,
    courses: &[(&str, &str)],
) -> Result<Vec<Id>, String> {
    let parsed: Vec<Course> = courses
        .iter()
        .map(|(b, d)| parse_course(b, d))
        .collect::<Result<_, _>>()?;
    set_current_point(Some(start));
    let mut ids = Vec::new();
    for c in parsed {
        let (a, b) = resolve_line(LineSpec::course(c), current_point(), previous_line())
            .map_err(String::from)?;
        let layer = cx.project.layers.current_cad_layer();
        let made = add_cad_items(cx, &layer, vec![CadItem::Line { a, b }], "Draw Line")
            .ok_or_else(|| "The CAD layer is locked".to_string())?;
        ids.extend(made);
        set_current_point(Some(b));
        PREVIOUS.with(|p| p.set(Some((a, b))));
    }
    Ok(ids)
}

/// The corners of the lines `ids` in order (each line must start where the
/// last ended, within `tol` inches).
pub fn chain_points(cx: &EditorContext, ids: &[Id], tol: f64) -> Option<Vec<Point>> {
    let mut pts: Vec<Point> = Vec::new();
    for id in ids {
        let CadItem::Line { a, b } = cx.floor().cad.iter().find(|c| c.id == *id)?.item.clone()
        else {
            return None;
        };
        match pts.last() {
            None => pts.push(a),
            Some(last) if last.dist(a) > tol => return None,
            _ => {}
        }
        pts.push(b);
    }
    Some(pts)
}

/// Convert to Terrain Perimeter: the closed chain of lines `ids` becomes the
/// terrain perimeter (one undo step). The CAD lines stay.
pub fn convert_to_terrain_perimeter(cx: &mut EditorContext, ids: &[Id]) -> Result<f64, String> {
    let mut pts = chain_points(cx, ids, 0.01).ok_or("The lines do not form one chain")?;
    let miss = pts.first().zip(pts.last()).map_or(0.0, |(a, b)| a.dist(*b));
    if miss > 0.5 {
        return Err(format!("The traverse does not close (off by {miss:.2} in)"));
    }
    if pts.len() > 3 {
        pts.pop();
    }
    if pts.len() < 3 {
        return Err("A perimeter needs at least three corners".into());
    }
    site_view::edit_terrain(cx, "Convert to Terrain Perimeter", |r| {
        r.terrain.perimeter = pts;
    });
    Ok(miss)
}

// ----- Show Length / Show Angle (CAD-118) -----

/// One live label: where it sits (plan inches), its text and its angle.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeLabel {
    pub pos: Point,
    pub text: String,
    pub rotation: f64,
}

fn line_labels(a: Point, b: Point, l: &CadLabels, st: &NumberStyle, out: &mut Vec<EdgeLabel>) {
    let len = a.dist(b);
    if len < 1e-9 {
        return;
    }
    let dir = b.sub(a).normalized();
    // Keep the text upright: it reads left to right, or bottom to top on a
    // vertical edge, so its angle stays in (-90, 90] degrees.
    let half = std::f64::consts::FRAC_PI_2;
    let ang = dir.angle();
    let (u, rot) = if ang > half + 1e-9 {
        (dir.scale(-1.0), ang - std::f64::consts::PI)
    } else if ang <= -half + 1e-9 {
        (dir.scale(-1.0), ang + std::f64::consts::PI)
    } else {
        (dir, ang)
    };
    let mid = Point::lerp(a, b, 0.5);
    let nudge = u.perp().scale(6.0);
    if l.show_length {
        out.push(EdgeLabel {
            pos: mid.add(nudge),
            text: st.format_length(len),
            rotation: rot,
        });
    }
    if l.show_angle {
        let from_far = l.reverse_angle;
        let deg = if from_far {
            a.sub(b).angle().to_degrees()
        } else {
            b.sub(a).angle().to_degrees()
        };
        out.push(EdgeLabel {
            pos: mid.sub(nudge),
            text: st.format_angle(deg),
            rotation: rot,
        });
    }
}

/// The live labels of a CAD item for the flags in `l` (length centered
/// above, angle or radius centered below).
pub fn edge_labels(item: &CadItem, l: &CadLabels, st: &NumberStyle) -> Vec<EdgeLabel> {
    let mut out = Vec::new();
    if !l.any() {
        return out;
    }
    match item {
        CadItem::Line { a, b } => line_labels(*a, *b, l, st, &mut out),
        CadItem::Polyline { points, closed } => {
            let n = points.len();
            let edges = if *closed { n } else { n.saturating_sub(1) };
            for i in 0..edges {
                line_labels(points[i], points[(i + 1) % n], l, st, &mut out);
            }
        }
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let mut sweep = end_angle - start_angle;
            if sweep <= 0.0 {
                sweep += std::f64::consts::TAU;
            }
            let mid_a = start_angle + sweep / 2.0;
            let mid = center.add(Point::new(mid_a.cos(), mid_a.sin()).scale(*radius));
            if l.show_length {
                out.push(EdgeLabel {
                    pos: mid.add(Point::new(mid_a.cos(), mid_a.sin()).scale(6.0)),
                    text: st.format_length(radius * sweep),
                    rotation: 0.0,
                });
            }
            if l.show_radius || l.show_angle {
                out.push(EdgeLabel {
                    pos: mid.sub(Point::new(mid_a.cos(), mid_a.sin()).scale(6.0)),
                    text: format!("R {}", st.format_length(*radius)),
                    rotation: 0.0,
                });
            }
        }
        _ => {}
    }
    out
}

/// Paints the live labels of `c` (called for objects that have attributes).
pub fn draw_labels(
    painter: &eframe::egui::Painter,
    cam: &crate::editor::Camera,
    c: &plan_core::cad::CadObject,
    labels: &CadLabels,
    color: eframe::egui::Color32,
) {
    if !labels.any() {
        return;
    }
    for l in edge_labels(&c.item, labels, &label_style()) {
        let galley =
            painter.layout_no_wrap(l.text, eframe::egui::FontId::proportional(11.0), color);
        let at = cam.world_to_screen(l.pos);
        // The plan's y axis points up the screen, so the text turns the other
        // way; it is rotated about its top-left corner, so that corner is put
        // where the text's middle lands on `at`.
        let a = -l.rotation as f32;
        let (s, co) = a.sin_cos();
        let h = galley.size() / 2.0;
        let turned = eframe::egui::vec2(h.x * co - h.y * s, h.x * s + h.y * co);
        painter.add(eframe::egui::epaint::TextShape::new(at - turned, galley, color).with_angle(a));
    }
}

/// The azimuth text a line shows, for tests and the status line.
pub fn azimuth_text(a: Point, b: Point, st: &NumberStyle) -> String {
    let mut s = st.clone();
    if s.angle.style != AngleStyle::Azimuth && s.angle.style != AngleStyle::Quadrant {
        s.angle.style = AngleStyle::Quadrant;
    }
    s.format_angle(azimuth_to_math_of(a, b))
}

fn azimuth_to_math_of(a: Point, b: Point) -> f64 {
    plan_core::bearing::azimuth_to_math(azimuth_of(a, b))
}

/// Direction check helper kept for the dialogs: unit vector of an azimuth.
pub fn direction(az: f64) -> Point {
    azimuth_dir(az)
}

/// Marks a CAD object as selected for the Current Point when it is a
/// temporary point's line.
pub fn select_current(cx: &mut EditorContext, o: ObjectRef) {
    cx.selection.set(o);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polar_and_relative_points_start_from_the_current_point() {
        assert!(resolve_point(PointSpec::Relative { dx: 1.0, dy: 1.0 }, None).is_err());
        let cur = Some(Point::new(10.0, 20.0));
        let p = resolve_point(
            PointSpec::Polar {
                dist: 5.0,
                angle: 90.0,
            },
            cur,
        )
        .unwrap();
        assert!(p.dist(Point::new(10.0, 25.0)) < 1e-9);
        let q = resolve_point(PointSpec::Relative { dx: -10.0, dy: 5.0 }, cur).unwrap();
        assert_eq!(q, Point::new(0.0, 25.0));
    }

    #[test]
    fn line_relative_to_previous_turns_from_its_direction() {
        let prev = Some((Point::new(0.0, 0.0), Point::new(100.0, 0.0)));
        let spec = LineSpec {
            start: StartSpec::Absolute { x: 100.0, y: 0.0 },
            end: EndSpec::FromPrevious {
                dist: 50.0,
                turn: 90.0,
            },
        };
        let (a, b) = resolve_line(spec, None, prev).unwrap();
        assert_eq!(a, Point::new(100.0, 0.0));
        assert!(b.dist(Point::new(100.0, 50.0)) < 1e-9);
        assert!(resolve_line(spec, None, None).is_err());
        let zero = LineSpec {
            start: StartSpec::Absolute { x: 1.0, y: 1.0 },
            end: EndSpec::Relative { dx: 0.0, dy: 0.0 },
        };
        assert!(resolve_line(zero, None, None).is_err());
    }

    #[test]
    fn labels_follow_the_flags_and_the_style() {
        let st = NumberStyle::survey();
        let item = CadItem::Line {
            a: Point::new(0.0, 0.0),
            b: Point::new(0.0, 1200.0),
        };
        let none = CadLabels::default();
        assert!(edge_labels(&item, &none, &st).is_empty());
        let both = CadLabels {
            show_length: true,
            show_angle: true,
            ..CadLabels::default()
        };
        let l = edge_labels(&item, &both, &st);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].text, "100'");
        assert_eq!(l[1].text, "N");
        let arc = CadItem::Arc {
            center: Point::ZERO,
            radius: 120.0,
            start_angle: 0.0,
            end_angle: std::f64::consts::FRAC_PI_2,
        };
        let r = CadLabels {
            show_radius: true,
            ..CadLabels::default()
        };
        assert_eq!(edge_labels(&arc, &r, &st)[0].text, "R 10'");
    }

    #[test]
    fn cad_defaults_lay_over_the_number_style() {
        use plan_core::defaults::PageValue;
        let mut d = plan_core::PlanDefaults::default();
        let base = NumberStyle::default();
        assert_eq!(overlay_cad_defaults(&base, &d), base);
        d.pages.insert(
            "cad.general.angle_format".into(),
            PageValue::Text("Bearing".into()),
        );
        d.pages.insert(
            "cad.general.length_format".into(),
            PageValue::Text("Feet".into()),
        );
        let s = overlay_cad_defaults(&base, &d);
        assert_eq!(s.angle.style, AngleStyle::Quadrant);
        assert_eq!(s.angle.decimals, 0);
        assert_eq!(s.format_length(120.0), "10'");
        assert_eq!(s.format_angle(45.0), "N 45\u{b0}0'0\" E");
        d.pages.insert(
            "cad.general.length_accuracy".into(),
            PageValue::Text("1/2".into()),
        );
        let mut s = overlay_cad_defaults(&base, &d);
        s.length.unit = plan_core::units::LengthUnit::FeetInches;
        assert_eq!(s.length.fraction_denominator, 2);
    }

    #[test]
    fn label_text_is_upright_whichever_way_the_line_runs() {
        let st = NumberStyle::default();
        let l = CadLabels {
            show_length: true,
            ..CadLabels::default()
        };
        let rot = |a: Point, b: Point| {
            edge_labels(&CadItem::Line { a, b }, &l, &st)[0].rotation.to_degrees()
        };
        let o = Point::ZERO;
        assert!((rot(o, Point::new(100.0, 0.0))).abs() < 1e-9);
        assert!((rot(Point::new(100.0, 0.0), o)).abs() < 1e-9);
        assert!((rot(o, Point::new(0.0, 100.0)) - 90.0).abs() < 1e-9);
        assert!((rot(Point::new(0.0, 100.0), o) - 90.0).abs() < 1e-9);
        assert!((rot(o, Point::new(-100.0, -100.0)) - 45.0).abs() < 1e-9);
    }
}
