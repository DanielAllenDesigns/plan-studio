//! Dependency-free ASCII DXF writer (R12 / AC1009).
//!
//! Output is plain group-code / value line pairs. Units are inches
//! (`$INSUNITS` = 1). Walls are closed POLYLINEs using the joined outlines from
//! [`crate::joins`]; openings are jamb LINEs plus a door-swing ARC; dimensions
//! are exploded into LINEs and TEXT (no DIMENSION entity), so they open the
//! same everywhere.

use crate::cad::{CadItem, CadObject};
use crate::dimension::{DimFormat, Dimension, DimensionKind};
use crate::geometry::Point;
use crate::joins::wall_outlines;
use crate::model::{OpeningKind, Project};
use crate::rooms::Room;
use std::collections::BTreeSet;

const WALL_JOIN_TOL: f64 = 0.5;
const DIM_TEXT_HEIGHT: f64 = 4.0;
const ROOM_TEXT_HEIGHT: f64 = 6.0;
const AREA_TEXT_HEIGHT: f64 = 4.0;

/// Group-code / value pair writer.
struct Dxf {
    out: String,
}

impl Dxf {
    fn new() -> Self {
        Self { out: String::new() }
    }

    fn pair(&mut self, code: i32, value: impl std::fmt::Display) {
        self.out.push_str(&format!("{code}\n{value}\n"));
    }

    fn num(&mut self, code: i32, v: f64) {
        self.pair(code, fmt_num(v));
    }

    fn text_value(&mut self, code: i32, s: &str) {
        let clean: String = s.chars().filter(|c| !c.is_control()).collect();
        self.pair(code, clean);
    }

    fn section(&mut self, name: &str) {
        self.pair(0, "SECTION");
        self.pair(2, name);
    }

    fn end_section(&mut self) {
        self.pair(0, "ENDSEC");
    }

    fn entity(&mut self, kind: &str, layer: &str) {
        self.pair(0, kind);
        self.text_value(8, layer);
    }

    fn point(&mut self, base: i32, p: Point) {
        self.num(base, p.x);
        self.num(base + 10, p.y);
        self.num(base + 20, 0.0);
    }

    fn line(&mut self, layer: &str, a: Point, b: Point) {
        self.entity("LINE", layer);
        self.point(10, a);
        self.point(11, b);
    }

    fn arc(&mut self, layer: &str, c: Point, r: f64, start_deg: f64, end_deg: f64) {
        self.entity("ARC", layer);
        self.point(10, c);
        self.num(40, r);
        self.num(50, start_deg.rem_euclid(360.0));
        self.num(51, end_deg.rem_euclid(360.0));
    }

    fn circle(&mut self, layer: &str, c: Point, r: f64) {
        self.entity("CIRCLE", layer);
        self.point(10, c);
        self.num(40, r);
    }

    fn polyline(&mut self, layer: &str, pts: &[Point], closed: bool) {
        self.entity("POLYLINE", layer);
        self.pair(66, 1);
        self.pair(70, if closed { 1 } else { 0 });
        for p in pts {
            self.entity("VERTEX", layer);
            self.point(10, *p);
        }
        self.entity("SEQEND", layer);
    }

    /// A filled quadrilateral (SOLID) in AutoCAD colour `color`; `q` runs
    /// round the outline, which SOLID wants in the order 1, 2, 4, 3.
    fn solid(&mut self, layer: &str, q: [Point; 4], color: i32) {
        self.entity("SOLID", layer);
        self.pair(62, color);
        self.point(10, q[0]);
        self.point(11, q[1]);
        self.point(12, q[3]);
        self.point(13, q[2]);
    }

    /// TEXT centered on `at` (horizontal center, vertical middle).
    fn text_centered(&mut self, layer: &str, at: Point, height: f64, rot_deg: f64, text: &str) {
        self.entity("TEXT", layer);
        self.point(10, at);
        self.num(40, height);
        self.text_value(1, text);
        self.num(50, rot_deg);
        self.pair(72, 1);
        self.pair(73, 2);
        self.point(11, at);
    }

    /// TEXT anchored at its bottom-left.
    fn text_left(&mut self, layer: &str, at: Point, height: f64, rot_deg: f64, text: &str) {
        self.entity("TEXT", layer);
        self.point(10, at);
        self.num(40, height);
        self.text_value(1, text);
        self.num(50, rot_deg);
    }
}

pub(super) fn fmt_num(v: f64) -> String {
    let s = format!("{v:.4}");
    if s == "-0.0000" {
        "0.0000".to_string()
    } else {
        s
    }
}

/// Nearest AutoCAD Color Index among the basic palette.
fn aci_color(rgb: [u8; 3]) -> i32 {
    const PALETTE: [(i32, [u8; 3]); 10] = [
        (1, [255, 0, 0]),
        (2, [255, 255, 0]),
        (3, [0, 255, 0]),
        (4, [0, 255, 255]),
        (5, [0, 0, 255]),
        (6, [255, 0, 255]),
        (7, [0, 0, 0]),
        (8, [128, 128, 128]),
        (9, [192, 192, 192]),
        (30, [255, 127, 0]),
    ];
    let dist = |c: [u8; 3]| -> i32 {
        (0..3)
            .map(|i| {
                let d = i32::from(rgb[i]) - i32::from(c[i]);
                d * d
            })
            .sum()
    };
    PALETTE
        .iter()
        .min_by_key(|(_, c)| dist(*c))
        .map(|(n, _)| *n)
        .unwrap_or(7)
}

fn dim_layer(kind: DimensionKind) -> &'static str {
    match kind {
        DimensionKind::Manual => "Dimensions, Manual",
        DimensionKind::AutoExterior | DimensionKind::Temporary => "Dimensions, Automatic",
    }
}

/// Angle in degrees, flipped if needed so text never reads upside down.
fn readable_angle_deg(v: Point) -> f64 {
    let mut a = v.angle().to_degrees();
    if a > 90.0 + 1e-9 {
        a -= 180.0;
    } else if a <= -90.0 + 1e-9 {
        a += 180.0;
    }
    a
}

/// Width of a text in plan inches at `height`, estimated (DXF carries no
/// font metrics): the average glyph is 0.55 of the height wide.
fn estimated_width(text: &str, height: f64) -> f64 {
    text.chars().count() as f64 * height * 0.55
}

/// A text with a box (TXT-1, TXT-16) as the entities that make it up: the
/// background (SOLID), the frame (closed POLYLINE) and one TEXT per line
/// run, wrapped and aligned as the plan shows them, with the glyph widths
/// estimated. `None` when the text has no box.
fn write_text_box(
    d: &mut Dxf,
    c: &CadObject,
    attrs: &crate::cad::CadAttrs,
    height: f64,
) -> Option<()> {
    let CadItem::Text {
        pos, text, angle, ..
    } = &c.item
    else {
        return None;
    };
    let item = CadItem::Text {
        pos: *pos,
        text: text.clone(),
        height,
        angle: *angle,
    };
    let pb = crate::text_box::placed(&item, attrs)?;
    let draw = pb.draw_plan(&|r, h| estimated_width(&r.text, h));
    let layer = c.layer.as_str();
    if let Some((quad, rgb)) = draw.fill {
        d.solid(layer, quad, aci_color(rgb));
    }
    if let Some(quad) = draw.border {
        d.polyline(layer, &quad, true);
    }
    for r in &draw.runs {
        d.text_left(layer, r.at, r.height, pb.angle.to_degrees(), &r.run.text);
    }
    Some(())
}

fn write_cad(
    d: &mut Dxf,
    c: &CadObject,
    attrs: Option<&crate::cad::CadAttrs>,
    text_height: impl Fn(&CadObject, f64) -> f64,
) {
    let layer = c.layer.as_str();
    if let (CadItem::Text { height, .. }, Some(a)) = (&c.item, attrs) {
        if a.text_box.needs_layout() && write_text_box(d, c, a, text_height(c, *height)).is_some()
        {
            return;
        }
    }
    match &c.item {
        CadItem::Line { a, b } => d.line(layer, *a, *b),
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => d.arc(
            layer,
            *center,
            *radius,
            start_angle.to_degrees(),
            end_angle.to_degrees(),
        ),
        CadItem::Circle { center, radius } => d.circle(layer, *center, *radius),
        CadItem::Polyline { points, closed } => d.polyline(layer, points, *closed),
        CadItem::Text {
            pos,
            text,
            height,
            angle,
        } => d.text_left(
            layer,
            *pos,
            text_height(c, *height),
            angle.to_degrees(),
            text,
        ),
    }
}

/// How the export sizes annotation (DXF has no printed size: the text height
/// is written in plan inches, so a style that holds its size on paper needs
/// the sheet's scale to turn it into plan inches).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DxfAnnotation {
    /// Paper inches per foot of plan (0.25 for 1/4" scale); `0` writes the
    /// stored character heights.
    pub inches_per_foot: f64,
    /// The dimension set's text style (empty: "Dimension Text Style").
    pub dim_text_style: String,
    /// The dimension set holds its text size on paper at any scale.
    pub dim_printed_size: bool,
    /// How dimension numbers read; `None` is the default feet-inches format.
    pub dim_format: Option<DimFormat>,
}

impl DxfAnnotation {
    /// The plan height of the number of `dim`: its own text style, else the
    /// set's, else "Dimension Text Style"; the stored plan height of the
    /// style, or its size on paper at `inches_per_foot` when the style or the
    /// set holds a printed size. `DIM_TEXT_HEIGHT` when no style is found.
    fn dim_height(&self, project: &Project, dim: &Dimension) -> f64 {
        let name = dim
            .text_style
            .as_deref()
            .filter(|n| !n.is_empty())
            .or(Some(self.dim_text_style.as_str()).filter(|n| !n.is_empty()))
            .unwrap_or("Dimension Text Style");
        project
            .text_styles
            .resolve(name)
            .map_or(DIM_TEXT_HEIGHT, |st| {
                st.plan_height_at(self.inches_per_foot, self.dim_printed_size)
            })
    }
}

/// Export `floor` of `project` as an R12 ASCII DXF string. `rooms` are the
/// detected rooms for that floor (used for room labels).
pub fn write_dxf(project: &Project, floor: usize, rooms: &[Room]) -> String {
    DxfExport::new(project, floor, rooms).write()
}

/// One extra polyline group of a [`DxfExport`].
#[derive(Debug, Clone, PartialEq)]
struct ExtraPolylines {
    layer: String,
    polylines: Vec<Vec<Point>>,
    closed: bool,
}

/// A DXF export of one floor with additional polylines for data the model
/// keeps outside the typed fields (roof plane outlines, manual framing).
///
/// ```
/// use plan_core::{export::dxf::DxfExport, Point, Project};
/// let p = Project::new("x");
/// let sq = vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(10.0, 10.0)];
/// let dxf = DxfExport::new(&p, 0, &[])
///     .with_extra_polylines("Roof Planes", vec![sq])
///     .write();
/// assert!(dxf.contains("Roof Planes"));
/// ```
pub struct DxfExport<'a> {
    project: &'a Project,
    floor: usize,
    rooms: &'a [Room],
    extra: Vec<ExtraPolylines>,
    annotation: DxfAnnotation,
}

impl<'a> DxfExport<'a> {
    pub fn new(project: &'a Project, floor: usize, rooms: &'a [Room]) -> Self {
        Self {
            project,
            floor,
            rooms,
            extra: Vec::new(),
            annotation: DxfAnnotation::default(),
        }
    }

    /// Sizes dimension and CAD text from the text styles at the sheet's
    /// scale (see [`DxfAnnotation`]).
    pub fn with_annotation(mut self, annotation: DxfAnnotation) -> Self {
        self.annotation = annotation;
        self
    }

    /// Adds closed polylines on `layer` (a layer unknown to the project is
    /// still declared in the LAYER table). Polylines with fewer than two
    /// points are dropped.
    pub fn with_extra_polylines(mut self, layer: &str, polylines: Vec<Vec<Point>>) -> Self {
        self.push_extra(layer, polylines, true);
        self
    }

    /// Like [`DxfExport::with_extra_polylines`], but the polylines stay open.
    pub fn with_extra_open_polylines(mut self, layer: &str, polylines: Vec<Vec<Point>>) -> Self {
        self.push_extra(layer, polylines, false);
        self
    }

    fn push_extra(&mut self, layer: &str, polylines: Vec<Vec<Point>>, closed: bool) {
        let polylines: Vec<Vec<Point>> = polylines.into_iter().filter(|p| p.len() >= 2).collect();
        if !polylines.is_empty() {
            self.extra.push(ExtraPolylines {
                layer: layer.to_string(),
                polylines,
                closed,
            });
        }
    }

    /// The R12 ASCII DXF text.
    pub fn write(&self) -> String {
        write_dxf_with(
            self.project,
            self.floor,
            self.rooms,
            &self.extra,
            &self.annotation,
        )
    }
}

fn write_dxf_with(
    project: &Project,
    floor: usize,
    rooms: &[Room],
    extra_polylines: &[ExtraPolylines],
    ann: &DxfAnnotation,
) -> String {
    let fl = &project.floors[floor];
    let ipf = ann.inches_per_foot;
    let mut d = Dxf::new();

    // HEADER
    d.section("HEADER");
    d.pair(9, "$ACADVER");
    d.pair(1, "AC1009");
    d.pair(9, "$INSUNITS");
    d.pair(70, 1);
    d.end_section();

    // TABLES
    let mut layer_names: BTreeSet<&str> = BTreeSet::new();
    for c in &fl.cad {
        layer_names.insert(c.layer.as_str());
    }
    for w in &fl.walls {
        layer_names.insert(w.layer.as_str());
    }
    for e in extra_polylines {
        layer_names.insert(e.layer.as_str());
    }
    let extra: Vec<&str> = layer_names
        .into_iter()
        .filter(|n| *n != "0" && project.layers.get(n).is_none())
        .collect();
    d.section("TABLES");
    d.pair(0, "TABLE");
    d.pair(2, "LTYPE");
    d.pair(70, 1);
    d.pair(0, "LTYPE");
    d.pair(2, "CONTINUOUS");
    d.pair(70, 0);
    d.pair(3, "Solid line");
    d.pair(72, 65);
    d.pair(73, 0);
    d.num(40, 0.0);
    d.pair(0, "ENDTAB");
    d.pair(0, "TABLE");
    d.pair(2, "LAYER");
    d.pair(70, 1 + project.layers.layers.len() + extra.len());
    let write_layer = |d: &mut Dxf, name: &str, color: i32| {
        d.pair(0, "LAYER");
        d.text_value(2, name);
        d.pair(70, 0);
        d.pair(62, color);
        d.pair(6, "CONTINUOUS");
    };
    write_layer(&mut d, "0", 7);
    for l in &project.layers.layers {
        let c = aci_color(l.color);
        write_layer(&mut d, &l.name, if l.display { c } else { -c });
    }
    for name in extra {
        write_layer(&mut d, name, 7);
    }
    d.pair(0, "ENDTAB");
    d.end_section();

    // ENTITIES
    d.section("ENTITIES");

    // Walls
    for outline in wall_outlines(&fl.walls, WALL_JOIN_TOL) {
        let layer = fl
            .wall(outline.wall_id)
            .map_or("Walls, Normal", |w| w.layer.as_str());
        d.polyline(layer, &outline.polygon, true);
    }

    // Openings
    for o in &fl.openings {
        let Some(w) = fl.wall(o.wall_id) else {
            continue;
        };
        let dir = w.direction();
        let n = w.normal();
        let half_t = w.thickness * 0.5;
        let layer = match o.kind {
            OpeningKind::Door => "Doors",
            OpeningKind::Window => "Windows",
        };
        let jamb_a = w.point_at(o.start_offset());
        let jamb_b = w.point_at(o.end_offset());
        for j in [jamb_a, jamb_b] {
            d.line(layer, j.sub(n.scale(half_t)), j.add(n.scale(half_t)));
        }
        match o.kind {
            OpeningKind::Door => {
                let theta = dir.angle().to_degrees();
                // Hinge on the near jamb normally; far jamb and opposite side when flipped.
                let (hinge, closed, open, start_deg) = if o.swing_flipped {
                    (jamb_b, dir.scale(-1.0), n.scale(-1.0), theta + 180.0)
                } else {
                    (jamb_a, dir, n, theta)
                };
                d.line(layer, hinge, hinge.add(open.scale(o.width)));
                d.arc(layer, hinge, o.width, start_deg, start_deg + 90.0);
                // Closed-leaf position across the opening.
                d.line(layer, hinge, hinge.add(closed.scale(o.width)));
            }
            OpeningKind::Window => {
                d.line(layer, jamb_a, jamb_b);
            }
        }
    }

    // Dimensions
    let fmt = ann.dim_format.unwrap_or_default();
    for dim in fl
        .dimensions
        .iter()
        .filter(|x| x.kind != DimensionKind::Temporary)
    {
        let layer = dim_layer(dim.kind);
        // Extension lines switched off per point (Show Extension Line) stay
        // off in the drawing.
        for (a, b) in dim.visible_extension_lines() {
            d.line(layer, a, b);
        }
        let (a, b) = dim.line_points();
        d.line(layer, a, b);
        let v = b.sub(a);
        let rot = readable_angle_deg(v);
        // Lift the text just above the line (relative to the reading direction).
        let text_h = ann.dim_height(project, dim);
        let up = Point::new(rot.to_radians().cos(), rot.to_radians().sin()).perp();
        let at = Point::lerp(a, b, 0.5).add(up.scale(text_h * 0.8));
        d.text_centered(layer, at, text_h, rot, &dim.label(&fmt));
    }

    // Room labels
    let label_h = project.text_styles.drawn_height(
        &project.layers,
        "Room Labels",
        None,
        ROOM_TEXT_HEIGHT,
        ipf,
    );
    let area_h = label_h * AREA_TEXT_HEIGHT / ROOM_TEXT_HEIGHT;
    for r in rooms {
        let name = r
            .name_entry(&fl.room_names)
            .map_or(r.label.as_str(), |n| n.name.as_str());
        d.text_centered(
            "Room Labels",
            r.centroid.add(Point::new(0.0, label_h * 0.6)),
            label_h,
            0.0,
            name,
        );
        d.text_centered(
            "Room Labels",
            r.centroid.sub(Point::new(0.0, area_h * 1.0)),
            area_h,
            0.0,
            &format!("{:.0} SF", r.area_sq_ft()),
        );
    }

    // CAD
    // Text is written at the height it is drawn at: a printed-size style at
    // its size on paper for the sheet's scale.
    let attrs = fl.cad_attr_map();
    let text_height = |c: &CadObject, h: f64| {
        project.text_styles.drawn_height(
            &project.layers,
            &c.layer,
            attrs.get(&c.id).and_then(|a| a.text_style.as_deref()),
            h,
            ipf,
        )
    };
    for c in &fl.cad {
        write_cad(&mut d, c, attrs.get(&c.id), text_height);
    }

    // Extra polylines (roof planes, manual framing)
    for e in extra_polylines {
        for pl in &e.polylines {
            d.polyline(&e.layer, pl, e.closed);
        }
    }

    d.end_section();
    d.pair(0, "EOF");
    d.out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dimension::Dimension;
    use crate::model::{WallKind, DEFAULT_CEILING_HEIGHT};
    use crate::rooms::detect_rooms;

    fn sample() -> Project {
        let mut p = Project::new("dxf");
        let corners = [
            (0.0, 0.0, 240.0, 0.0),
            (240.0, 0.0, 240.0, 120.0),
            (240.0, 120.0, 0.0, 120.0),
            (0.0, 120.0, 0.0, 0.0),
        ];
        let mut first = 0;
        for (i, (x0, y0, x1, y1)) in corners.into_iter().enumerate() {
            let id = p.add_wall(
                0,
                Point::new(x0, y0),
                Point::new(x1, y1),
                6.5,
                DEFAULT_CEILING_HEIGHT,
                WallKind::Exterior,
            );
            if i == 0 {
                first = id;
            }
        }
        p.add_opening(0, first, 60.0, OpeningKind::Door).unwrap();
        p.add_opening(0, first, 180.0, OpeningKind::Window).unwrap();
        p.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::new(0.0, 0.0),
                Point::new(240.0, 0.0),
                -36.0,
            ),
        );
        p.add_cad(
            0,
            "CAD, Default",
            CadItem::Circle {
                center: Point::new(50.0, 50.0),
                radius: 10.0,
            },
        );
        p.add_cad(
            0,
            "Custom Layer",
            CadItem::Text {
                pos: Point::new(0.0, 0.0),
                text: "Note".into(),
                height: 4.0,
                angle: 0.0,
            },
        );
        p
    }

    #[test]
    fn dxf_structure() {
        let p = sample();
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        assert_eq!(rooms.len(), 1);
        let s = write_dxf(&p, 0, &rooms);
        assert!(s.contains("SECTION") && s.contains("ENTITIES") && s.contains("EOF"));
        assert!(s.contains("AC1009") && s.contains("$INSUNITS"));
        // One closed POLYLINE per wall plus none from CAD here.
        assert_eq!(s.lines().filter(|l| *l == "POLYLINE").count(), 4);
        assert_eq!(s.lines().filter(|l| *l == "SEQEND").count(), 4);
        assert_eq!(s.lines().filter(|l| *l == "ARC").count(), 1);
        assert_eq!(s.lines().filter(|l| *l == "CIRCLE").count(), 1);
        assert!(s.contains("Dimensions, Manual"));
        assert!(s.contains("Room Labels"));
        assert!(s.contains("Custom Layer"));
        assert!(s.contains("20'-0\""));

        // Even number of lines, codes are integers, ends with 0/EOF.
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines.len() % 2, 0);
        for pair in lines.chunks(2) {
            assert!(
                pair[0].trim().parse::<i32>().is_ok(),
                "bad group code {:?}",
                pair[0]
            );
        }
        assert_eq!(&lines[lines.len() - 2..], ["0", "EOF"]);
        // Sections balance.
        let secs = lines
            .chunks(2)
            .filter(|p| p[0] == "0" && p[1] == "SECTION")
            .count();
        let ends = lines
            .chunks(2)
            .filter(|p| p[0] == "0" && p[1] == "ENDSEC")
            .count();
        assert_eq!(secs, 3);
        assert_eq!(secs, ends);
    }

    fn texts(s: &str) -> Vec<(String, f64)> {
        // (value, height) of every TEXT entity.
        let lines: Vec<&str> = s.lines().collect();
        let mut out = Vec::new();
        let mut i = 0;
        while i + 1 < lines.len() {
            if lines[i] == "0" && lines[i + 1] == "TEXT" {
                let (mut h, mut v) = (0.0, String::new());
                let mut j = i + 2;
                while j + 1 < lines.len() && lines[j] != "0" {
                    match lines[j] {
                        "40" => h = lines[j + 1].parse().unwrap(),
                        "1" => v = lines[j + 1].to_string(),
                        _ => {}
                    }
                    j += 2;
                }
                out.push((v, h));
                i = j;
            } else {
                i += 2;
            }
        }
        out
    }

    #[test]
    fn hidden_extension_lines_are_not_written() {
        let mut p = sample();
        let line_count = |p: &Project| {
            write_dxf(p, 0, &[])
                .lines()
                .filter(|l| *l == "LINE")
                .count()
        };
        let all = line_count(&p);
        p.floors[0].dimensions[0].hide_ext = [true, false];
        assert_eq!(line_count(&p), all - 1);
        p.floors[0].dimensions[0].hide_ext = [true, true];
        assert_eq!(line_count(&p), all - 2);
    }

    #[test]
    fn text_heights_come_from_the_styles_at_the_sheet_scale() {
        let mut p = sample();
        // Without a scale: the stored CAD height and the Dimension Text
        // Style's 4.5" character height.
        let t = texts(&write_dxf(&p, 0, &[]));
        assert!(t.contains(&("Note".to_string(), 4.0)), "{t:?}");
        assert!(t.contains(&("20'-0\"".to_string(), 4.5)), "{t:?}");
        // A printed-size layer style (Custom Layer is not a project layer,
        // so it uses the Default Text Style) and a printed-size dimension
        // set, written for 1/8" scale: twice the 1/4" plan height.
        let i = p
            .text_styles
            .styles
            .iter()
            .position(|s| s.name == "Default Text Style")
            .unwrap();
        p.text_styles.styles[i].use_printed_size(true);
        let ann = DxfAnnotation {
            inches_per_foot: 0.125,
            dim_printed_size: true,
            ..DxfAnnotation::default()
        };
        let s = DxfExport::new(&p, 0, &[])
            .with_annotation(ann.clone())
            .write();
        let t = texts(&s);
        // 4" stored against a 6" style: 4 * 12 / 6 = 8.
        assert!(t.contains(&("Note".to_string(), 8.0)), "{t:?}");
        // 4.5" Dimension Text Style held to its printed size (0.1875")
        // at 1/8" scale: 18".
        let dim_h = t.iter().find(|(v, _)| v == "20'-0\"").unwrap().1;
        let want = p
            .text_styles
            .get("Dimension Text Style")
            .unwrap()
            .plan_height_at(0.125, true);
        assert!((dim_h - want).abs() < 1e-3, "{dim_h} vs {want}");
        // The same printed size at 1/4" is half the plan height.
        let q = DxfExport::new(&p, 0, &[])
            .with_annotation(DxfAnnotation {
                inches_per_foot: 0.25,
                ..ann
            })
            .write();
        let note = texts(&q).iter().find(|(v, _)| v == "Note").unwrap().1;
        assert!((note - 4.0).abs() < 1e-3, "{note}");
    }

    #[test]
    fn extra_polylines_land_on_their_own_declared_layer() {
        let p = sample();
        let sq = vec![
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 50.0),
            Point::new(0.0, 50.0),
        ];
        let base = write_dxf(&p, 0, &[]);
        let s = DxfExport::new(&p, 0, &[])
            .with_extra_polylines("Roof Planes", vec![sq.clone(), vec![Point::ZERO]])
            .with_extra_open_polylines("Framing", vec![vec![Point::ZERO, Point::new(5.0, 0.0)]])
            .write();
        // Two polylines added (the one-point polyline is dropped).
        assert_eq!(
            s.lines().filter(|l| *l == "POLYLINE").count(),
            base.lines().filter(|l| *l == "POLYLINE").count() + 2
        );
        // Declared in the layer table, then used by the entity.
        let lines: Vec<&str> = s.lines().collect();
        let decl = lines
            .chunks(2)
            .position(|c| c[0] == "2" && c[1] == "Roof Planes")
            .expect("layer declared");
        let used = lines
            .chunks(2)
            .rposition(|c| c[0] == "8" && c[1] == "Roof Planes")
            .unwrap();
        assert!(decl < used);
        // The closed flag is written (group 70 = 1) for the roof outline.
        assert!(s.contains("Framing"));
        assert_eq!(write_dxf(&p, 0, &[]), DxfExport::new(&p, 0, &[]).write());
    }

    #[test]
    fn hidden_layer_gets_negative_color_and_temporary_dims_skipped() {
        let mut p = sample();
        p.layers.set_display("Doors", false);
        p.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::Temporary,
                Point::ZERO,
                Point::new(10.0, 0.0),
                5.0,
            ),
        );
        let s = write_dxf(&p, 0, &[]);
        let lines: Vec<&str> = s.lines().collect();
        let idx = lines
            .chunks(2)
            .position(|c| c[0] == "2" && c[1] == "Doors")
            .unwrap();
        // LAYER record: 2 name, 70 flags, 62 color.
        let color = lines.chunks(2).nth(idx + 2).unwrap();
        assert_eq!(color[0], "62");
        assert!(color[1].starts_with('-'));
        assert!(!s.contains("Dimensions, Automatic\n10"));
    }

    #[test]
    fn room_labels_name_an_island_and_the_room_around_it_apart() {
        use crate::model::RoomName;
        let mut p = Project::new("island");
        let ring = |p: &mut Project, x0: f64, y0: f64, x1: f64, y1: f64, kind| {
            let c = [
                Point::new(x0, y0),
                Point::new(x1, y0),
                Point::new(x1, y1),
                Point::new(x0, y1),
            ];
            for i in 0..4 {
                p.add_wall(0, c[i], c[(i + 1) % 4], 4.5, 96.0, kind);
            }
        };
        ring(&mut p, 0.0, 0.0, 480.0, 360.0, WallKind::Exterior);
        ring(&mut p, 200.0, 150.0, 280.0, 210.0, WallKind::Interior);
        // The island's name first: the room around it must not take it.
        p.floors[0]
            .room_names
            .push(RoomName::new(Point::new(240.0, 180.0), "Pantry", "Pantry"));
        p.floors[0].room_names.push(RoomName::new(
            Point::new(60.0, 60.0),
            "Great Room",
            "Family",
        ));
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        assert_eq!(rooms.len(), 2);
        let s = write_dxf(&p, 0, &rooms);
        assert_eq!(s.matches("Pantry").count(), 1, "one label for the pantry");
        assert_eq!(s.matches("Great Room").count(), 1);
    }

    #[test]
    fn a_boxed_text_is_wrapped_framed_and_filled() {
        use crate::cad::CadAttrs;
        let mut p = Project::new("boxed");
        let id = p.add_cad(
            0,
            "Text",
            CadItem::Text {
                pos: Point::new(10.0, 10.0),
                text: "Verify the fascia depth and the drip edge at site".into(),
                height: 3.0,
                angle: 0.0,
            },
        );
        let plain = write_dxf(&p, 0, &[]);
        assert_eq!(texts(&plain).len(), 1, "one TEXT for a plain text");
        assert!(!plain.contains("SOLID"));
        let mut a = CadAttrs::new(id);
        a.text_box.width = 40.0;
        a.text_box.border = true;
        a.text_box.background = Some([255, 255, 0]);
        p.floors[0].cad_attrs.push(a);
        let boxed = write_dxf(&p, 0, &[]);
        let lines = texts(&boxed);
        assert!(lines.len() >= 2, "wrapped into several TEXTs: {lines:?}");
        assert!(lines.iter().all(|(_, h)| (*h - 3.0).abs() < 1e-6));
        let joined = lines
            .iter()
            .map(|(t, _)| t.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(joined.contains("fascia") && joined.contains("drip edge"));
        // The frame is a closed POLYLINE and the background a SOLID.
        assert_eq!(boxed.lines().filter(|l| *l == "POLYLINE").count(), 1);
        assert_eq!(boxed.lines().filter(|l| *l == "SOLID").count(), 1);
    }
}
