//! Auto Detail, CAD Detail From View and the CAD Detail Management
//! operations.
//!
//! **Auto Detail** turns a section or elevation camera into a CAD detail
//! (`plan_core::details::CadDetailInfo`, a detail floor):
//!
//! * the camera's 2D drawing is made with the material hatch on at the
//!   detail's print scale and its lines are copied as CAD lines on the detail
//!   layers: section cut lines on `Detail, Cut`, silhouettes and material
//!   seams on `Detail, Lines`, the material hatch on `Detail, Hatch`,
//!   annotations on `Detail, Notes`;
//! * every wall the cut passes through is drawn as its assembly: the lines
//!   between the wall type's layers, each layer's material pattern as hatch
//!   (the framing layer is left to the framing and insulation), the wall's
//!   layer names as notes beside the first cut wall;
//! * the framing layer shows the framing members `plan-framing` built for the
//!   wall (Build Framing) that the cut plane passes through, each a box with
//!   the X that marks cut lumber; a wall without built framing gets its
//!   bottom plate and double top plate;
//! * the stud cavity between the plates gets an insulation batt on
//!   `Detail, Insulation` (exterior walls, and any wall whose type has an
//!   insulation layer) unless the cut passes through a stud, which then
//!   stands there instead.
//!
//! The detail's name follows the camera name and callout number until the
//! user renames it. **CAD Detail From View** on a plan view copies that
//! floor's lines into a detail; with a section or elevation selected it is
//! Auto Detail.

use crate::dialogs::camera as cam;
use crate::editor::{details_view as dv, framing_view, plan_tabs, EditorContext, ObjectRef};
use plan_core::cad::{insulation_items, CadItem};
use plan_core::defaults::{PlanDefaults, WallLayer};
use plan_core::details::{
    auto_detail_name, normalize_to_origin, CadDetailInfo, DetailSource, DETAIL_CUT_LAYER,
    DETAIL_FRAMING_LAYER, DETAIL_HATCH_LAYER, DETAIL_INSULATION_LAYER, DETAIL_LINES_LAYER,
    DETAIL_NOTES_LAYER,
};
use plan_core::geometry::Point;
use plan_core::walls::Side;
use plan_core::{CameraObject, Id, Project, SavedPlanView, Wall, WallKind};
use plan_elevation::{EdgeKind, RegionKind};

/// Text height on paper, inches (1/8").
const TEXT_PAPER_IN: f64 = 0.125;
/// Thickness of a bottom plate and of each top plate, inches.
const PLATE: f64 = 1.5;

/// What Auto Detail draws.
#[derive(Clone, Debug, PartialEq)]
pub struct AutoDetailOptions {
    /// Print scale, paper inches per foot of the detail.
    pub scale: f64,
    /// The camera drawing's material hatch.
    pub hatch: bool,
    /// Insulation batts in wall cavities.
    pub insulation: bool,
    /// Framing members (or assumed plates) in cut walls.
    pub framing: bool,
    /// The title, scale note and layer names.
    pub notes: bool,
}

impl Default for AutoDetailOptions {
    fn default() -> Self {
        Self {
            scale: plan_core::details::DEFAULT_DETAIL_SCALE,
            hatch: true,
            insulation: true,
            framing: true,
            notes: true,
        }
    }
}

/// The items Auto Detail made, and what is in them.
#[derive(Clone, Debug, Default)]
pub struct Built {
    pub items: Vec<(String, CadItem)>,
    /// Cut wall pieces that were drawn as an assembly.
    pub walls: usize,
    pub batts: usize,
    /// Cut framing members drawn (assumed plates included).
    pub members: usize,
    pub hatch_lines: usize,
}

/// `1 1/2" = 1'-0"` for a scale in paper inches per foot.
pub fn scale_label(scale: f64) -> String {
    let whole = scale.floor();
    let frac = scale - whole;
    let fractions = [
        (0.0, ""),
        (0.125, "1/8"),
        (0.25, "1/4"),
        (0.375, "3/8"),
        (0.5, "1/2"),
        (0.625, "5/8"),
        (0.75, "3/4"),
        (0.875, "7/8"),
    ];
    let text = fractions
        .iter()
        .find(|(f, _)| (frac - f).abs() < 1e-6)
        .map(|(_, s)| match (whole as i64, *s) {
            (0, s) => s.to_string(),
            (w, "") => format!("{w}"),
            (w, s) => format!("{w} {s}"),
        })
        .unwrap_or_else(|| format!("{scale}"));
    format!("{text}\" = 1'-0\"")
}

/// The section and elevation cameras a detail can be made from.
pub fn eligible_cameras(project: &Project) -> Vec<(Id, String)> {
    project
        .cameras
        .iter()
        .filter(|c| cam::is_elevation_camera(c))
        .map(|c| (c.id, c.name.clone()))
        .collect()
}

// ----- the wall assembly -----

/// The layers of `wall`'s type, exterior face first.
fn wall_layers(project: &Project, defaults: &PlanDefaults, wall: &Wall) -> Vec<WallLayer> {
    let typed = wall.wall_type.as_deref().and_then(|n| {
        project
            .wall_type_def(n)
            .or_else(|| defaults.wall_type(n))
            .map(|t| t.layers.clone())
    });
    if let Some(l) = typed.filter(|l| !l.is_empty()) {
        return l;
    }
    let t = wall.thickness;
    if t < 2.0 {
        return vec![WallLayer::new("Wall", t, true, "Fir Framing")];
    }
    if wall.kind == WallKind::Exterior {
        let core = (t - 1.5).max(0.5);
        vec![
            WallLayer::new("Siding", 0.5, false, "Siding"),
            WallLayer::new("Sheathing", 0.5, false, "OSB-Hrz"),
            WallLayer::new("Framing", core, true, "Fir Framing"),
            WallLayer::new("Drywall", 0.5, false, "Drywall"),
        ]
    } else {
        let core = (t - 1.0).max(0.5);
        vec![
            WallLayer::new("Drywall", 0.5, false, "Drywall"),
            WallLayer::new("Framing", core, true, "Fir Framing"),
            WallLayer::new("Drywall", 0.5, false, "Drywall"),
        ]
    }
}

fn rect_poly(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
    vec![
        Point::new(x0, y0),
        Point::new(x1, y0),
        Point::new(x1, y1),
        Point::new(x0, y1),
    ]
}

fn push_rect(out: &mut Vec<(String, CadItem)>, layer: &str, x0: f64, y0: f64, x1: f64, y1: f64) {
    out.push((
        layer.to_string(),
        CadItem::Polyline {
            points: rect_poly(x0, y0, x1, y1),
            closed: true,
        },
    ));
}

fn push_cross(out: &mut Vec<(String, CadItem)>, layer: &str, x0: f64, y0: f64, x1: f64, y1: f64) {
    out.push((
        layer.to_string(),
        CadItem::Line {
            a: Point::new(x0, y0),
            b: Point::new(x1, y1),
        },
    ));
    out.push((
        layer.to_string(),
        CadItem::Line {
            a: Point::new(x0, y1),
            b: Point::new(x1, y0),
        },
    ));
}

/// A cut framing member in drawing space.
#[derive(Clone, Copy, Debug)]
struct CutMember {
    kind: plan_framing::MemberKind,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

/// The members of `wall` (on floor `fi`) that the cut plane of `view` passes
/// through, as boxes in drawing space.
fn cut_members(
    project: &Project,
    fi: usize,
    wall: &Wall,
    view: &plan_elevation::FreeView,
    clip: (f64, f64, f64, f64),
) -> (usize, Vec<CutMember>) {
    let members: Vec<plan_framing::Member> = framing_view::load(&project.floors[fi])
        .into_iter()
        .filter(|m| m.wall_id == Some(wall.id))
        .collect();
    let total = members.len();
    let mut out = Vec::new();
    for m in members {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        let (mut d0, mut d1) = (f64::MAX, f64::MIN);
        for c in m.corners() {
            // Scene axes: X right, Y up, Z = -plan y.
            let v = view.to_view(Point::new(c[0], -c[2]));
            x0 = x0.min(v.x);
            x1 = x1.max(v.x);
            y0 = y0.min(c[1]);
            y1 = y1.max(c[1]);
            d0 = d0.min(v.y);
            d1 = d1.max(v.y);
        }
        let cut = d0 <= 1e-6 && d1 >= -1e-6;
        let inside =
            x1 >= clip.0 - 0.5 && x0 <= clip.2 + 0.5 && y1 >= clip.1 - 0.5 && y0 <= clip.3 + 0.5;
        if cut && inside {
            out.push(CutMember {
                kind: m.kind,
                x0,
                y0,
                x1,
                y1,
            });
        }
    }
    (total, out)
}

fn is_stud(k: plan_framing::MemberKind) -> bool {
    use plan_framing::MemberKind as K;
    matches!(
        k,
        K::Stud | K::KingStud | K::TrimmerStud | K::CrippleStud | K::CornerStud | K::TeeStud
    )
}

/// Draws one cut piece of a wall (`region` is the bounding box of its cut
/// polygon in drawing space) as its assembly.
#[allow(clippy::too_many_arguments)]
fn assembly(
    project: &Project,
    defaults: &PlanDefaults,
    fi: usize,
    wall: &Wall,
    view: &plan_elevation::FreeView,
    region: (f64, f64, f64, f64),
    opts: &AutoDetailOptions,
    built: &mut Built,
    labels: &mut Vec<(f64, String)>,
) {
    let (x0, y0, x1, y1) = region;
    let width = x1 - x0;
    if width < 0.5 || y1 - y0 < 2.0 {
        return;
    }
    let ext = if wall.exterior_side == Side::Left {
        wall.normal()
    } else {
        wall.normal().scale(-1.0)
    };
    let ext_x = view.tangent().dot(ext);
    if ext_x.abs() < 0.2 {
        // The cut runs along the wall: there is no thickness to show.
        return;
    }
    let layers = wall_layers(project, defaults, wall);
    let total: f64 = layers.iter().map(|l| l.thickness).sum();
    if total <= 0.0 {
        return;
    }
    // Strips from the exterior face inward, in drawing x.
    let (mut at, dir) = if ext_x > 0.0 { (x1, -1.0) } else { (x0, 1.0) };
    let paper = opts.scale;
    let mut main: Option<(f64, f64)> = None;
    let mut has_insulation_layer = false;
    let mut strips = Vec::new();
    for l in &layers {
        let w = l.thickness / total * width;
        let (a, b) = (at, at + dir * w);
        let (lo, hi) = (a.min(b), a.max(b));
        strips.push((l.clone(), lo, hi));
        if l.is_main {
            main = Some((lo, hi));
        }
        let lname = l.name.to_ascii_lowercase();
        has_insulation_layer |=
            lname.contains("insul") || l.material.to_ascii_lowercase().contains("insul");
        at = b;
    }
    built.walls += 1;
    for (i, (l, lo, hi)) in strips.iter().enumerate() {
        // The line between this layer and the next.
        if i + 1 < strips.len() {
            let edge = if dir < 0.0 { *lo } else { *hi };
            built.items.push((
                DETAIL_LINES_LAYER.to_string(),
                CadItem::Line {
                    a: Point::new(edge, y0),
                    b: Point::new(edge, y1),
                },
            ));
        }
        if opts.hatch && !l.is_main {
            let (pattern, _) = dv::material_look(&l.material);
            if pattern != plan_materials::Pattern::None {
                for (a, b) in dv::strokes_in(&pattern, &rect_poly(*lo, y0, *hi, y1), paper) {
                    built
                        .items
                        .push((DETAIL_HATCH_LAYER.to_string(), CadItem::Line { a, b }));
                    built.hatch_lines += 1;
                }
            }
        }
    }
    // The layer names, for the notes beside the first cut wall.
    for (l, lo, hi) in &strips {
        labels.push(((lo + hi) * 0.5, l.name.clone()));
    }
    let Some((fx0, fx1)) = main else {
        return;
    };
    // Framing and the cavity.
    let (total_members, cut) = if opts.framing {
        cut_members(project, fi, wall, view, (fx0, y0, fx1, y1))
    } else {
        (0, Vec::new())
    };
    let mut bottom = y0 + PLATE;
    let mut top = y1 - 2.0 * PLATE;
    if opts.framing && total_members == 0 {
        // No built framing: the assumed plates.
        push_rect(
            &mut built.items,
            DETAIL_FRAMING_LAYER,
            fx0,
            y0,
            fx1,
            y0 + PLATE,
        );
        push_cross(
            &mut built.items,
            DETAIL_FRAMING_LAYER,
            fx0,
            y0,
            fx1,
            y0 + PLATE,
        );
        for k in 0..2 {
            let ya = y1 - PLATE * f64::from(k + 1);
            push_rect(
                &mut built.items,
                DETAIL_FRAMING_LAYER,
                fx0,
                ya,
                fx1,
                ya + PLATE,
            );
            push_cross(
                &mut built.items,
                DETAIL_FRAMING_LAYER,
                fx0,
                ya,
                fx1,
                ya + PLATE,
            );
        }
        built.members += 3;
    }
    let mut stud_in_the_way = false;
    for m in &cut {
        use plan_framing::MemberKind as K;
        match m.kind {
            K::BottomPlate => bottom = bottom.max(m.y1),
            K::TopPlate => top = top.min(m.y0),
            k if is_stud(k) => stud_in_the_way = true,
            _ => {}
        }
        // Boxes that run along the cut plane are clipped to the wall's cut.
        let (bx0, bx1) = (m.x0.max(fx0 - 0.01), m.x1.min(fx1 + 0.01));
        if bx1 - bx0 < 0.05 {
            continue;
        }
        push_rect(&mut built.items, DETAIL_FRAMING_LAYER, bx0, m.y0, bx1, m.y1);
        push_cross(&mut built.items, DETAIL_FRAMING_LAYER, bx0, m.y0, bx1, m.y1);
        built.members += 1;
    }
    let insulated = wall.kind == WallKind::Exterior || has_insulation_layer;
    if opts.insulation && insulated && !stud_in_the_way && top - bottom > 4.0 {
        let corners = [
            Point::new(fx0, bottom),
            Point::new(fx0, top),
            Point::new(fx1, top),
            Point::new(fx1, bottom),
        ];
        for item in insulation_items(&corners) {
            built
                .items
                .push((DETAIL_INSULATION_LAYER.to_string(), item));
        }
        built.batts += 1;
    }
}

fn bbox(poly: &[Point]) -> (f64, f64, f64, f64) {
    let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in poly {
        b = (b.0.min(p.x), b.1.min(p.y), b.2.max(p.x), b.3.max(p.y));
    }
    b
}

/// Builds the detail of `camera` (see the module docs). The items are in
/// building inches with the lower-left of the drawing at the origin.
pub fn build(
    project: &Project,
    defaults: &PlanDefaults,
    camera: &CameraObject,
    opts: &AutoDetailOptions,
) -> Built {
    let mut el = cam::elevation_options(camera);
    el.hatch = opts.hatch;
    el.hatch_scale = opts.scale;
    el.regions = true;
    let mut drawing = cam::render_elevation_with(project, camera, &el);
    drawing.merge_collinear();
    let mut built = Built::default();
    if drawing.lines.is_empty() {
        // Nothing in view: no title or scale note either.
        return built;
    }
    for l in &drawing.lines {
        let layer = match l.kind {
            EdgeKind::Cut => DETAIL_CUT_LAYER,
            EdgeKind::Hatch => {
                if !opts.hatch {
                    continue;
                }
                built.hatch_lines += 1;
                DETAIL_HATCH_LAYER
            }
            EdgeKind::Annotation => DETAIL_NOTES_LAYER,
            EdgeKind::Hidden => continue,
            _ => DETAIL_LINES_LAYER,
        };
        built
            .items
            .push((layer.to_string(), CadItem::Line { a: l.a, b: l.b }));
    }
    let text_h = TEXT_PAPER_IN * 12.0 / opts.scale.max(0.01);
    if opts.notes {
        for (pos, text) in &drawing.texts {
            built.items.push((
                DETAIL_NOTES_LAYER.to_string(),
                CadItem::Text {
                    pos: *pos,
                    text: text.clone(),
                    height: text_h,
                    angle: 0.0,
                },
            ));
        }
    }
    // The cut walls as assemblies.
    let view = cam::free_view(camera);
    let mut names: Vec<(f64, String)> = Vec::new();
    let mut label_top = f64::MIN;
    let mut right = f64::MIN;
    for r in drawing.regions_of(RegionKind::Cut) {
        let Some(id) = r.object_id else {
            continue;
        };
        let Some((fi, wall)) = project
            .floors
            .iter()
            .enumerate()
            .find_map(|(i, f)| f.wall(id).map(|w| (i, w)))
        else {
            continue;
        };
        let region = bbox(&r.polygon);
        // The layer names are written once, for the first wall piece.
        let first = built.walls == 0;
        let mut these = Vec::new();
        assembly(
            project, defaults, fi, wall, &view, region, opts, &mut built, &mut these,
        );
        if first && !these.is_empty() {
            label_top = region.3;
            names = these;
        }
        right = right.max(region.2);
    }
    if opts.notes {
        if !names.is_empty() {
            // Names beside the drawing, each with a leader to its layer.
            let x_text = drawing.bounds.1.x.max(right) + text_h * 3.0;
            for (k, (x, name)) in names.iter().enumerate() {
                let y = label_top - text_h * 2.0 * (k as f64 + 1.0);
                built.items.push((
                    DETAIL_NOTES_LAYER.to_string(),
                    CadItem::Line {
                        a: Point::new(*x, y + text_h * 0.5),
                        b: Point::new(x_text - text_h * 0.5, y + text_h * 0.5),
                    },
                ));
                built.items.push((
                    DETAIL_NOTES_LAYER.to_string(),
                    CadItem::Text {
                        pos: Point::new(x_text, y),
                        text: name.clone(),
                        height: text_h,
                        angle: 0.0,
                    },
                ));
            }
        }
        // Title and scale under the drawing.
        let (lo, _) = drawing.bounds;
        let title = auto_detail_name(&camera.name, project.callout_number(camera.id));
        built.items.push((
            DETAIL_NOTES_LAYER.to_string(),
            CadItem::Text {
                pos: Point::new(lo.x, lo.y - text_h * 3.0),
                text: title.to_uppercase(),
                height: text_h * 1.5,
                angle: 0.0,
            },
        ));
        built.items.push((
            DETAIL_NOTES_LAYER.to_string(),
            CadItem::Text {
                pos: Point::new(lo.x, lo.y - text_h * 5.0),
                text: format!("SCALE: {}", scale_label(opts.scale)),
                height: text_h,
                angle: 0.0,
            },
        ));
    }
    normalize_to_origin(&mut built.items);
    built
}

// ----- making details -----

fn fill_detail(cx: &mut EditorContext, idx: usize, items: Vec<(String, CadItem)>) {
    for (layer, item) in items {
        cx.project.add_cad(idx, layer, item);
    }
}

/// Auto Detail: makes a CAD detail from the section or elevation camera
/// `camera` as one undo step. Returns the new detail's floor index.
pub fn auto_detail(
    cx: &mut EditorContext,
    camera: Id,
    opts: &AutoDetailOptions,
) -> Result<usize, String> {
    let Some(c) = cx.project.camera(camera).cloned() else {
        return Err("That view is not in the plan any more".into());
    };
    if !cam::is_elevation_camera(&c) {
        return Err(format!(
            "{} is not a section or elevation: pick one of those",
            c.name
        ));
    }
    let built = build(&cx.project, &cx.defaults, &c, opts);
    if built.items.is_empty() {
        return Err(format!(
            "{} shows nothing to make a detail from: draw walls first",
            c.name
        ));
    }
    cx.begin_change("Auto Detail");
    let name = auto_detail_name(&c.name, cx.project.callout_number(camera));
    let mut info = CadDetailInfo::from_source(DetailSource::Camera { camera });
    info.scale = opts.scale;
    let idx = cx.project.add_cad_detail(&name, info);
    let objects = built.items.len();
    fill_detail(cx, idx, built.items);
    cx.mark_dirty();
    cx.status = format!(
        "Made detail \"{}\": {objects} objects, {} wall pieces, {} hatch lines, {} insulation batts, {} framing members",
        cx.project.floors[idx].name, built.walls, built.hatch_lines, built.batts, built.members
    );
    Ok(idx)
}

/// CAD Detail From View on a plan: the lines of the active floor (wall
/// outlines and visible CAD) and its dimensions become a detail. One undo
/// step; returns the detail's floor index.
pub fn detail_from_plan(cx: &mut EditorContext) -> Result<usize, String> {
    let fl = cx.floor;
    let floor = &cx.project.floors[fl];
    if floor.is_cad_detail() {
        return Err("This is already a detail".into());
    }
    let items = plan_core::cad::detail_items(floor, cx.layers(), DETAIL_CUT_LAYER);
    let dims = floor.dimensions.clone();
    if items.is_empty() && dims.is_empty() {
        return Err("There are no lines in this view to copy".into());
    }
    let source = floor.name.clone();
    cx.begin_change("CAD Detail From View");
    let mut info = CadDetailInfo::from_source(DetailSource::PlanView {
        floor: source.clone(),
    });
    info.scale = 0.25;
    let idx = cx.project.add_cad_detail(&format!("{source} Detail"), info);
    let n = items.len() + dims.len();
    fill_detail(cx, idx, items);
    for d in dims {
        cx.project.add_dimension(idx, d);
    }
    cx.mark_dirty();
    cx.status = format!(
        "Copied {n} objects into \"{}\"",
        cx.project.floors[idx].name
    );
    Ok(idx)
}

/// CAD Detail From View: a selected section or elevation becomes a detail by
/// Auto Detail; otherwise the plan lines of the active floor are copied.
pub fn detail_from_view(cx: &mut EditorContext) -> Result<usize, String> {
    let selected = cx.selection.items.iter().find_map(|o| match o {
        ObjectRef::Camera(id) => cx
            .project
            .camera(*id)
            .filter(|c| cam::is_elevation_camera(c)),
        _ => None,
    });
    match selected.map(|c| c.id) {
        Some(id) => auto_detail(cx, id, &AutoDetailOptions::default()),
        None => detail_from_plan(cx),
    }
}

/// A blank detail called "CAD Detail" (CAD Detail Management > New).
pub fn new_detail(cx: &mut EditorContext) -> usize {
    cx.begin_change("New CAD Detail");
    let idx = cx
        .project
        .add_cad_detail("CAD Detail", CadDetailInfo::default());
    cx.mark_dirty();
    cx.status = format!("Added detail \"{}\"", cx.project.floors[idx].name);
    idx
}

// ----- management -----

/// The name of the plan view (tab) that shows detail `name`.
pub fn tab_name(name: &str) -> String {
    format!("Detail: {name}")
}

/// Shows detail `idx` in a plan view tab of its own and switches to it.
pub fn open_detail(cx: &mut EditorContext, idx: usize) -> bool {
    let Some(f) = cx.project.floors.get(idx).filter(|f| f.is_cad_detail()) else {
        return false;
    };
    let view = tab_name(&f.name);
    let layer_set = cx
        .project
        .current_plan_view()
        .map(|v| v.layer_set.clone())
        .or_else(|| cx.project.plan_views.first().map(|v| v.layer_set.clone()))
        .unwrap_or_default();
    match cx.project.plan_views.iter_mut().find(|v| v.name == view) {
        Some(v) => v.floor = Some(idx),
        None => {
            let mut v = SavedPlanView::new(view.clone(), layer_set);
            v.floor = Some(idx);
            cx.project.plan_views.push(v);
        }
    }
    let opened = plan_tabs::with_tabs(|t| t.open_view(cx, &view));
    if opened {
        cx.floor = idx.min(cx.project.floors.len() - 1);
        cx.selection.clear();
        cx.refresh();
        cx.status = format!("Detail \"{}\"", cx.project.floors[idx].name);
    }
    opened
}

/// Renames a detail (one undo step); its tab follows. False for an empty
/// name or a floor that is not a detail.
pub fn rename_detail(cx: &mut EditorContext, idx: usize, name: &str) -> bool {
    let Some(old) = cx
        .project
        .floors
        .get(idx)
        .filter(|f| f.is_cad_detail())
        .map(|f| f.name.clone())
    else {
        return false;
    };
    if name.trim().is_empty() {
        return false;
    }
    cx.begin_change("Rename CAD Detail");
    if !cx.project.rename_cad_detail(idx, name) {
        cx.cancel_change();
        return false;
    }
    let new = cx.project.floors[idx].name.clone();
    let (old_tab, new_tab) = (tab_name(&old), tab_name(&new));
    if let Some(v) = cx.project.plan_views.iter_mut().find(|v| v.name == old_tab) {
        v.name = new_tab.clone();
    }
    if cx.project.active_plan_view == old_tab {
        cx.project.active_plan_view = new_tab;
    }
    cx.mark_dirty();
    true
}

/// Duplicates a detail (one undo step). Returns the copy's floor index.
pub fn duplicate_detail(cx: &mut EditorContext, idx: usize) -> Option<usize> {
    cx.begin_change("Duplicate CAD Detail");
    match cx.project.duplicate_cad_detail(idx) {
        Some(i) => {
            cx.mark_dirty();
            cx.status = format!("Duplicated as \"{}\"", cx.project.floors[i].name);
            Some(i)
        }
        None => {
            cx.cancel_change();
            None
        }
    }
}

/// Deletes a detail and its tab (one undo step). The editor leaves the floor
/// if it was showing it.
pub fn delete_detail(cx: &mut EditorContext, idx: usize) -> bool {
    let Some(name) = cx
        .project
        .floors
        .get(idx)
        .filter(|f| f.is_cad_detail())
        .map(|f| f.name.clone())
    else {
        return false;
    };
    cx.begin_change("Delete CAD Detail");
    if !cx.project.delete_cad_detail(idx) {
        cx.cancel_change();
        return false;
    }
    let tab = tab_name(&name);
    let others: Vec<String> = cx
        .project
        .plan_views
        .iter()
        .filter(|v| v.name != tab)
        .map(|v| v.name.clone())
        .collect();
    if cx.project.active_plan_view == tab {
        if let Some(first) = others.first() {
            cx.project.active_plan_view = first.clone();
        }
    }
    if !others.is_empty() {
        cx.project.plan_views.retain(|v| v.name != tab);
    }
    match cx.floor.cmp(&idx) {
        std::cmp::Ordering::Equal => cx.floor = 0,
        std::cmp::Ordering::Greater => cx.floor -= 1,
        std::cmp::Ordering::Less => {}
    }
    cx.selection.clear();
    cx.mark_dirty();
    cx.refresh();
    cx.status = format!("Deleted detail \"{name}\"");
    true
}

/// The layout scale nearest to a detail's print scale.
fn layout_scale(scale: f64) -> plan_docs::Scale {
    use plan_docs::Scale as S;
    if scale >= 2.5 {
        S::ThreeInch
    } else if scale >= 1.25 {
        S::OneAndHalfInch
    } else if scale >= 0.9 {
        S::OneInch
    } else if scale >= 0.6 {
        S::ThreeQuarterInch
    } else if scale >= 0.4 {
        S::HalfInch
    } else {
        S::QuarterInch
    }
}

/// Send to Layout for a detail: a detail box on the last page of the plan's
/// layout, at the detail's print scale (one undo step). Returns the page.
pub fn send_to_layout(cx: &mut EditorContext, idx: usize) -> Result<u32, String> {
    use crate::shell::layout_window as lw;
    let Some(f) = cx.project.floors.get(idx).filter(|f| f.is_cad_detail()) else {
        return Err("That is not a CAD detail".into());
    };
    if f.cad.is_empty() {
        return Err(format!(
            "\"{}\" is empty: draw or place something in it",
            f.name
        ));
    }
    let name = f.name.clone();
    let scale = f.detail.as_ref().map_or(1.5, |d| d.scale);
    let items = f.cad.clone();
    let Some(mut layout) = lw::load(&cx.project) else {
        return Err("Start a layout first (File > New Layout), then send the detail".into());
    };
    let page = layout.content_pages().last().map_or(1, |p| p.number);
    let ctx = plan_layout::LayoutRenderContext::new(&cx.project);
    plan_layout::send_to_layout(
        &mut layout,
        &ctx,
        page,
        plan_layout::BoxSource::CadDetail {
            name: name.clone(),
            items,
        },
        layout_scale(scale),
        None,
    );
    drop(ctx);
    cx.begin_change("Send to Layout");
    lw::store(&mut cx.project, &layout);
    cx.mark_dirty();
    cx.status = format!("Detail \"{name}\" sent to layout page A-{page}");
    Ok(page)
}

/// Keeps the names of camera-made details in step with their cameras'
/// names and callout numbers. Not an undo step of its own: the name is
/// derived from the camera. True when a name changed.
pub fn sync_names(cx: &mut EditorContext) -> bool {
    if !cx.project.detail_names_stale() {
        return false;
    }
    let changed = cx.project.sync_detail_names();
    if changed > 0 {
        cx.mark_dirty();
    }
    changed > 0
}

// ----- commands -----

/// Command id: CAD > Auto Detail.
pub const AUTO_DETAIL: &str = "details.auto";
/// Command id: CAD > CAD Detail From View.
pub const DETAIL_FROM_VIEW: &str = "details.from_view";
/// Command id: CAD > CAD Detail Management.
pub const MANAGEMENT: &str = "details.management";
/// Command id: CAD > Detail Components.
pub const COMPONENTS: &str = "details.components";

/// Runs a detail command by id; false when the id is not one of ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        AUTO_DETAIL => {
            // A selected section or elevation is the source; otherwise ask.
            let selected = cx.selection.items.iter().find_map(|o| match o {
                ObjectRef::Camera(c) => cx
                    .project
                    .camera(*c)
                    .filter(|c| cam::is_elevation_camera(c))
                    .map(|c| c.id),
                _ => None,
            });
            match selected {
                Some(c) => {
                    cx.status = auto_detail(cx, c, &AutoDetailOptions::default())
                        .map_or_else(|e| e, |i| format!("Made \"{}\"", cx.project.floors[i].name));
                }
                None => crate::dialogs::details::open_auto_detail(cx),
            }
            true
        }
        DETAIL_FROM_VIEW => {
            if let Err(e) = detail_from_view(cx) {
                cx.status = e;
            }
            true
        }
        MANAGEMENT => {
            crate::dialogs::details::open_management();
            true
        }
        COMPONENTS => {
            crate::dialogs::details::open_components();
            true
        }
        _ => false,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::walls::Side;
    use plan_core::CameraKind;

    pub(crate) fn new_cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    /// A 20' x 15' house of Siding-6 exterior walls drawn clockwise, so the
    /// exterior is the left side of every wall.
    pub(crate) fn house(cx: &mut EditorContext) -> Vec<Id> {
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(0.0, 180.0),
            Point::new(240.0, 180.0),
            Point::new(240.0, 0.0),
        ];
        let mut ids = Vec::new();
        for i in 0..4 {
            let id =
                cx.project
                    .add_wall(0, pts[i], pts[(i + 1) % 4], 7.0, 96.0, WallKind::Exterior);
            let w = cx.project.floors[0].wall_mut(id).unwrap();
            w.wall_type = Some("Siding-6".into());
            w.exterior_side = Side::Left;
            ids.push(id);
        }
        cx.refresh();
        ids
    }

    /// A section looking east at x = `x`: the cut plane crosses the south
    /// wall (the wall along y = 0 is walls[3], from (240, 0) to (0, 0)).
    pub(crate) fn section_at(cx: &mut EditorContext, x: f64) -> Id {
        let mut c = plan_core::CameraObject::new(
            CameraKind::CrossSection { back_clip: None },
            Point::new(x, 0.0),
            0.0,
            "Section A",
            0,
        );
        crate::tools::camera::set_section_geometry(&mut c, Point::new(x, 0.0), 0.0, 120.0, None);
        cx.project.add_camera(c)
    }

    fn layer_count(b: &Built, layer: &str) -> usize {
        b.items.iter().filter(|(l, _)| l == layer).count()
    }

    fn texts(b: &Built) -> Vec<String> {
        b.items
            .iter()
            .filter_map(|(_, i)| match i {
                CadItem::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn scale_labels_read_like_the_title_block() {
        assert_eq!(scale_label(1.5), "1 1/2\" = 1'-0\"");
        assert_eq!(scale_label(3.0), "3\" = 1'-0\"");
        assert_eq!(scale_label(0.75), "3/4\" = 1'-0\"");
        assert_eq!(scale_label(1.0), "1\" = 1'-0\"");
        assert_eq!(scale_label(0.25), "1/4\" = 1'-0\"");
    }

    #[test]
    fn a_section_through_a_wall_becomes_a_detail_with_assembly_and_insulation() {
        let mut cx = new_cx();
        house(&mut cx);
        let cam = section_at(&mut cx, 120.0);
        let c = cx.project.camera(cam).unwrap().clone();
        let built = build(&cx.project, &cx.defaults, &c, &AutoDetailOptions::default());
        assert!(built.walls >= 1, "a wall piece was cut: {built:?}");
        // The section's cut lines, the assembly's hatch, the batt and the
        // assumed plates all sit on their own layers.
        assert!(layer_count(&built, DETAIL_CUT_LAYER) > 0);
        assert!(layer_count(&built, DETAIL_HATCH_LAYER) > 0, "{built:?}");
        assert_eq!(built.batts, 1);
        assert!(
            layer_count(&built, DETAIL_INSULATION_LAYER) >= 2,
            "box and wave"
        );
        assert!(
            layer_count(&built, DETAIL_FRAMING_LAYER) >= 9,
            "three plates"
        );
        assert_eq!(built.members, 3);
        // The notes name the wall's layers, the view and the scale.
        let t = texts(&built);
        for want in ["Siding", "Sheathing", "Framing", "Drywall"] {
            assert!(t.iter().any(|x| x == want), "{want} in {t:?}");
        }
        assert!(t.iter().any(|x| x.starts_with("SCALE: 1 1/2")), "{t:?}");
        assert!(t.iter().any(|x| x.contains("SECTION A")), "{t:?}");
        // Detail space starts at the origin.
        let (mut lo_x, mut lo_y) = (f64::MAX, f64::MAX);
        for (_, i) in &built.items {
            let (a, _) = i.bounds();
            lo_x = lo_x.min(a.x);
            lo_y = lo_y.min(a.y);
        }
        assert!(lo_x.abs() < 1e-6 && lo_y.abs() < 1e-6, "{lo_x} {lo_y}");
    }

    #[test]
    fn the_options_leave_things_out() {
        let mut cx = new_cx();
        house(&mut cx);
        let cam = section_at(&mut cx, 120.0);
        let c = cx.project.camera(cam).unwrap().clone();
        let none = AutoDetailOptions {
            hatch: false,
            insulation: false,
            framing: false,
            notes: false,
            ..AutoDetailOptions::default()
        };
        let b = build(&cx.project, &cx.defaults, &c, &none);
        assert_eq!(b.batts, 0);
        assert_eq!(b.members, 0);
        assert_eq!(layer_count(&b, DETAIL_INSULATION_LAYER), 0);
        assert_eq!(layer_count(&b, DETAIL_FRAMING_LAYER), 0);
        assert_eq!(layer_count(&b, DETAIL_HATCH_LAYER), 0);
        assert!(texts(&b).is_empty());
        assert!(layer_count(&b, DETAIL_CUT_LAYER) > 0, "the cut stays");
    }

    #[test]
    fn built_framing_replaces_the_assumed_plates_and_a_cut_stud_replaces_the_batt() {
        let mut cx = new_cx();
        house(&mut cx);
        let summary = framing_view::build(&mut cx, false);
        assert!(summary.walls > 0, "the walls were framed");
        let mut with_batt = None;
        let mut with_stud = None;
        let mut x = 80.0;
        while x < 160.0 && (with_batt.is_none() || with_stud.is_none()) {
            let cam = section_at(&mut cx, x);
            let c = cx.project.camera(cam).unwrap().clone();
            let b = build(&cx.project, &cx.defaults, &c, &AutoDetailOptions::default());
            if b.batts == 1 && with_batt.is_none() {
                with_batt = Some((x, b.clone()));
            }
            if b.batts == 0 && b.members > 3 && with_stud.is_none() {
                with_stud = Some((x, b));
            }
            cx.project.cameras.clear();
            x += 1.0;
        }
        let (_, batt) = with_batt.expect("a cut between two studs");
        // Real plates: the bottom plate and the double top plate, cut.
        assert!(batt.members >= 3, "{}", batt.members);
        assert!(layer_count(&batt, DETAIL_INSULATION_LAYER) >= 2);
        let (_, stud) = with_stud.expect("a cut through a stud");
        assert!(stud.members > batt.members, "the stud is drawn as well");
        assert_eq!(layer_count(&stud, DETAIL_INSULATION_LAYER), 0);
    }

    #[test]
    fn auto_detail_is_one_undo_step_and_a_cameraless_plan_says_why() {
        let mut cx = new_cx();
        assert!(auto_detail(&mut cx, 99, &AutoDetailOptions::default())
            .unwrap_err()
            .contains("not in the plan"));
        house(&mut cx);
        // An empty view is refused.
        let empty = {
            let mut c = plan_core::CameraObject::new(
                CameraKind::CrossSection { back_clip: None },
                Point::new(5000.0, 5000.0),
                0.0,
                "Nowhere",
                0,
            );
            crate::tools::camera::set_section_geometry(
                &mut c,
                Point::new(5000.0, 5000.0),
                0.0,
                60.0,
                None,
            );
            cx.project.add_camera(c)
        };
        assert!(auto_detail(&mut cx, empty, &AutoDetailOptions::default()).is_err());
        let cam = section_at(&mut cx, 120.0);
        let floors = cx.project.floors.len();
        let i = auto_detail(&mut cx, cam, &AutoDetailOptions::default()).unwrap();
        assert_eq!(i, floors);
        let f = &cx.project.floors[i];
        assert!(f.is_cad_detail() && !f.cad.is_empty());
        assert_eq!(
            f.detail.as_ref().unwrap().source,
            DetailSource::Camera { camera: cam }
        );
        assert!(f.name.contains("Section A"), "{}", f.name);
        assert!(cx.project.layers.get(DETAIL_INSULATION_LAYER).is_some());
        assert_eq!(cx.undo().as_deref(), Some("Auto Detail"));
        assert_eq!(cx.project.floors.len(), floors);
        // A plan view camera is not a section.
        let mut cam2 =
            plan_core::CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "Cam", 0);
        cam2.id = 0;
        let fid = cx.project.add_camera(cam2);
        assert!(auto_detail(&mut cx, fid, &AutoDetailOptions::default())
            .unwrap_err()
            .contains("not a section"));
    }

    #[test]
    fn the_detail_follows_its_camera_until_it_is_renamed() {
        let mut cx = new_cx();
        house(&mut cx);
        let cam = section_at(&mut cx, 120.0);
        let i = auto_detail(&mut cx, cam, &AutoDetailOptions::default()).unwrap();
        let n = cx.project.callout_number(cam).unwrap();
        assert_eq!(cx.project.floors[i].name, format!("{n} - Section A"));
        cx.project
            .cameras
            .iter_mut()
            .find(|c| c.id == cam)
            .unwrap()
            .name = "Wall Section".into();
        assert!(sync_names(&mut cx));
        assert_eq!(cx.project.floors[i].name, format!("{n} - Wall Section"));
        assert!(!sync_names(&mut cx), "nothing left to do");
        assert!(rename_detail(&mut cx, i, "Typical Wall"));
        cx.project
            .cameras
            .iter_mut()
            .find(|c| c.id == cam)
            .unwrap()
            .name = "Other".into();
        assert!(!sync_names(&mut cx));
        assert_eq!(cx.project.floors[i].name, "Typical Wall");
    }

    #[test]
    fn management_renames_duplicates_opens_and_deletes_with_undo() {
        let mut cx = new_cx();
        house(&mut cx);
        let cam = section_at(&mut cx, 120.0);
        let i = auto_detail(&mut cx, cam, &AutoDetailOptions::default()).unwrap();
        let objects = cx.project.floors[i].cad.len();
        // Open: the detail gets a tab of its own and the editor shows it.
        assert!(open_detail(&mut cx, i));
        assert_eq!(cx.floor, i);
        let tab = tab_name(&cx.project.floors[i].name);
        assert_eq!(cx.project.active_plan_view, tab);
        assert_eq!(cx.project.plan_view(&tab).unwrap().floor, Some(i));
        // Rename: the tab follows.
        assert!(rename_detail(&mut cx, i, "Sill Section"));
        assert_eq!(cx.project.floors[i].name, "Sill Section");
        assert_eq!(cx.project.active_plan_view, tab_name("Sill Section"));
        assert!(!rename_detail(&mut cx, i, "  "));
        assert!(!rename_detail(&mut cx, 0, "Not a detail"));
        assert_eq!(cx.undo().as_deref(), Some("Rename CAD Detail"));
        // Duplicate.
        let j = duplicate_detail(&mut cx, i).unwrap();
        assert_eq!(cx.project.floors[j].cad.len(), objects);
        assert_ne!(cx.project.floors[j].name, cx.project.floors[i].name);
        assert!(
            duplicate_detail(&mut cx, 0).is_none(),
            "a floor is not a detail"
        );
        // Delete the original: the copy moves down, the editor leaves.
        cx.floor = j;
        assert!(delete_detail(&mut cx, i));
        assert_eq!(cx.floor, j - 1);
        assert_eq!(cx.project.cad_detail_floors(), vec![j - 1]);
        assert!(!delete_detail(&mut cx, 0));
        assert_eq!(cx.undo().as_deref(), Some("Delete CAD Detail"));
        assert_eq!(cx.project.cad_detail_floors().len(), 2);
        assert_eq!(cx.undo().as_deref(), Some("Duplicate CAD Detail"));
        assert_eq!(cx.project.cad_detail_floors().len(), 1);
    }

    #[test]
    fn a_detail_goes_to_the_layout_as_a_detail_box_at_its_scale() {
        let mut cx = new_cx();
        house(&mut cx);
        let cam = section_at(&mut cx, 120.0);
        let i = auto_detail(&mut cx, cam, &AutoDetailOptions::default()).unwrap();
        // No layout yet.
        assert!(send_to_layout(&mut cx, i).unwrap_err().contains("layout"));
        assert!(send_to_layout(&mut cx, 0)
            .unwrap_err()
            .contains("not a CAD detail"));
        let mut layout = plan_layout::Layout::new("L", plan_docs::SheetSize::ArchC);
        layout.add_page(5, "Details");
        crate::shell::layout_window::store(&mut cx.project, &layout);
        let page = send_to_layout(&mut cx, i).unwrap();
        let layout = crate::shell::layout_window::load(&cx.project).unwrap();
        let b = layout.page(page).unwrap().boxes.last().unwrap();
        let plan_layout::BoxSource::CadDetail { name, items } = &b.source else {
            panic!("a detail box: {:?}", b.source);
        };
        assert_eq!(name, &cx.project.floors[i].name);
        assert_eq!(items.len(), cx.project.floors[i].cad.len());
        assert_eq!(b.scale, plan_docs::Scale::OneAndHalfInch);
        assert_eq!(cx.undo().as_deref(), Some("Send to Layout"));
        // An empty detail has nothing to send.
        let blank = new_detail(&mut cx);
        assert!(send_to_layout(&mut cx, blank)
            .unwrap_err()
            .contains("empty"));
    }

    #[test]
    fn cad_detail_from_view_copies_plan_lines_or_uses_the_selected_section() {
        let mut cx = new_cx();
        // Nothing to copy yet.
        assert!(detail_from_view(&mut cx).unwrap_err().contains("no lines"));
        house(&mut cx);
        let i = detail_from_view(&mut cx).unwrap();
        let f = &cx.project.floors[i];
        assert!(f.is_cad_detail());
        assert_eq!(
            f.detail.as_ref().unwrap().source,
            DetailSource::PlanView {
                floor: "1st Floor".into()
            }
        );
        assert_eq!(f.cad.len(), 4, "one outline per wall");
        assert!(f.cad.iter().all(|o| o.layer == DETAIL_CUT_LAYER));
        assert_eq!(cx.undo().as_deref(), Some("CAD Detail From View"));
        // A selected section makes an Auto Detail instead.
        let cam = section_at(&mut cx, 120.0);
        cx.selection.items = vec![ObjectRef::Camera(cam)];
        let j = detail_from_view(&mut cx).unwrap();
        assert!(matches!(
            cx.project.floors[j].detail.as_ref().unwrap().source,
            DetailSource::Camera { .. }
        ));
        // A detail cannot be copied into a detail.
        cx.floor = j;
        cx.selection.clear();
        assert!(detail_from_view(&mut cx).unwrap_err().contains("already"));
    }

    #[test]
    fn the_commands_run_and_unknown_ids_pass() {
        let mut cx = new_cx();
        assert!(!run_command(&mut cx, "nope"));
        assert!(run_command(&mut cx, DETAIL_FROM_VIEW));
        assert!(cx.status.contains("no lines"));
        assert!(run_command(&mut cx, AUTO_DETAIL));
        assert!(cx.status.contains("cross section"), "{}", cx.status);
        house(&mut cx);
        let cam = section_at(&mut cx, 120.0);
        cx.selection.items = vec![ObjectRef::Camera(cam)];
        assert!(run_command(&mut cx, AUTO_DETAIL));
        assert_eq!(cx.project.cad_detail_floors().len(), 1);
        assert!(run_command(&mut cx, MANAGEMENT));
        assert!(crate::dialogs::details::management_open());
        assert!(run_command(&mut cx, COMPONENTS));
        assert!(crate::dialogs::details::components_open());
    }
}
