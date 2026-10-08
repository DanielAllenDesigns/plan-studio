//! Dependency-free ASCII DXF writer (R12 / AC1009).
//!
//! Output is plain group-code / value line pairs. Units are inches
//! (`$INSUNITS` = 1). Walls are closed POLYLINEs using the joined outlines from
//! [`crate::joins`]; openings are jamb LINEs plus a door-swing ARC; dimensions
//! are exploded into LINEs and TEXT (no DIMENSION entity), so they open the
//! same everywhere.

use crate::cad::{CadItem, CadObject};
use crate::dimension::{DimFormat, DimensionKind};
use crate::geometry::{point_in_polygon, Point};
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

fn fmt_num(v: f64) -> String {
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

fn write_cad(d: &mut Dxf, c: &CadObject) {
    let layer = c.layer.as_str();
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
        } => d.text_left(layer, *pos, *height, angle.to_degrees(), text),
    }
}

/// Export `floor` of `project` as an R12 ASCII DXF string. `rooms` are the
/// detected rooms for that floor (used for room labels).
pub fn write_dxf(project: &Project, floor: usize, rooms: &[Room]) -> String {
    let fl = &project.floors[floor];
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
    let fmt = DimFormat::default();
    for dim in fl
        .dimensions
        .iter()
        .filter(|x| x.kind != DimensionKind::Temporary)
    {
        let layer = dim_layer(dim.kind);
        for (a, b) in dim.extension_lines() {
            d.line(layer, a, b);
        }
        let (a, b) = dim.line_points();
        d.line(layer, a, b);
        let v = b.sub(a);
        let rot = readable_angle_deg(v);
        // Lift the text just above the line (relative to the reading direction).
        let up = Point::new(rot.to_radians().cos(), rot.to_radians().sin()).perp();
        let at = Point::lerp(a, b, 0.5).add(up.scale(DIM_TEXT_HEIGHT * 0.8));
        d.text_centered(layer, at, DIM_TEXT_HEIGHT, rot, &dim.label(&fmt));
    }

    // Room labels
    for r in rooms {
        let name = fl
            .room_names
            .iter()
            .find(|n| point_in_polygon(n.anchor, &r.polygon))
            .map_or(r.label.as_str(), |n| n.name.as_str());
        d.text_centered(
            "Room Labels",
            r.centroid.add(Point::new(0.0, ROOM_TEXT_HEIGHT * 0.6)),
            ROOM_TEXT_HEIGHT,
            0.0,
            name,
        );
        d.text_centered(
            "Room Labels",
            r.centroid.sub(Point::new(0.0, AREA_TEXT_HEIGHT * 1.0)),
            AREA_TEXT_HEIGHT,
            0.0,
            &format!("{:.0} SF", r.area_sq_ft()),
        );
    }

    // CAD
    for c in &fl.cad {
        write_cad(&mut d, c);
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
}
