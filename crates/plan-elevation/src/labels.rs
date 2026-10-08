//! Elevation annotations: title, grade line, level callouts and roof pitch symbols.
//!
//! Text is `(anchor, string)` with the anchor at the left end of the baseline,
//! sized [`TEXT_H`] drawing units with an estimated advance of
//! [`TEXT_CHAR_W`] per character; consumers re-flow it with real font metrics.

use crate::drawing::{Drawing, EdgeKind, Line2, LineWeight, TEXT_CHAR_W, TEXT_H};
use crate::projection::{Projection, ViewDir};
use plan_3d::{Material, Scene};
use plan_core::units::fmt_ft_in_frac;
use plan_core::{FloorKind, Point, Project};
use std::collections::HashMap;

/// The grade line runs this far past the building on each side, inches.
const GRADE_OVERHANG: f64 = 36.0;
/// Gap between the level callouts' right end and the building, inches.
const CALLOUT_GAP: f64 = 48.0;
/// Pitch triangle leg run, inches.
const SYMBOL_RUN: f64 = 18.0;
/// Shortest sloped roof edge that gets a pitch symbol, inches.
const MIN_EDGE: f64 = 36.0;
/// At most this many pitch symbols per drawing.
const MAX_SYMBOLS: usize = 12;
/// Pitch symbols closer than this to one of the same pitch are skipped, inches.
const SYMBOL_SPACING: f64 = 48.0;

fn text_width(s: &str) -> f64 {
    TEXT_CHAR_W * s.chars().count() as f64
}

fn title(dir: ViewDir) -> &'static str {
    match dir {
        ViewDir::Front => "FRONT ELEVATION",
        ViewDir::Back => "BACK ELEVATION",
        ViewDir::Left => "LEFT ELEVATION",
        ViewDir::Right => "RIGHT ELEVATION",
        ViewDir::Top => "PLAN VIEW",
    }
}

fn level_text(name: &str, inches: f64) -> String {
    format!("{name} {}", fmt_ft_in_frac(inches, 8))
}

/// Add the annotations to `drawing`, the elevation of `scene` seen from `dir`.
///
/// * the title below the drawing ("FRONT ELEVATION"...);
/// * for side views, a Medium grade line (the lowest normal floor's subfloor
///   level) running 36" past the building, labelled "GRADE" at its right end;
/// * level callouts left of the building, relative to the first floor's
///   subfloor: "T.O. SUBFLOOR" at each floor's elevation and "T.O. PLATE" at
///   the top of its tallest drawn wall, in feet-inches to 1/8";
/// * a pitch symbol ("8:12" with a small right triangle) on each sloped roof
///   edge that shows as a sloped line in the view, when the scene has
///   [`Material::Roof`] meshes.
///
/// Lines are added as [`EdgeKind::Annotation`]. An empty drawing is left as is.
pub fn annotate(drawing: &mut Drawing, scene: &Scene, project: &Project, dir: ViewDir) {
    if drawing.lines.is_empty() {
        return;
    }
    let (lo, hi) = drawing.bounds;
    let mut low = lo.y;

    if dir != ViewDir::Top {
        let grade = project
            .floors
            .iter()
            .filter(|f| f.kind == FloorKind::Normal)
            .map(|f| f.elevation)
            .fold(f64::INFINITY, f64::min);
        let grade = if grade.is_finite() { grade } else { lo.y };
        drawing.lines.push(Line2 {
            a: Point::new(lo.x - GRADE_OVERHANG, grade),
            b: Point::new(hi.x + GRADE_OVERHANG, grade),
            weight: LineWeight::Medium,
            kind: EdgeKind::Annotation,
        });
        drawing.texts.push((
            Point::new(hi.x + GRADE_OVERHANG + 6.0, grade),
            "GRADE".into(),
        ));
        low = low.min(grade);

        let mut seen: Vec<(String, i64)> = Vec::new();
        for floor in &project.floors {
            let plate = floor
                .walls
                .iter()
                .filter(|w| !w.flags.invisible && !w.flags.railing)
                .map(|w| w.height)
                .fold(f64::NEG_INFINITY, f64::max);
            if !plate.is_finite() {
                continue;
            }
            for (name, y) in [
                ("T.O. SUBFLOOR", floor.elevation),
                ("T.O. PLATE", floor.elevation + plate),
            ] {
                let text = level_text(name, y);
                let key = (text.clone(), (y * 8.0).round() as i64);
                if seen.contains(&key) {
                    continue;
                }
                seen.push(key);
                let x = lo.x - CALLOUT_GAP - text_width(&text);
                drawing.texts.push((Point::new(x, y), text));
            }
        }
    }

    let label = title(dir);
    drawing.texts.push((
        Point::new(
            0.5 * (lo.x + hi.x) - 0.5 * text_width(label),
            low - 3.0 * TEXT_H,
        ),
        label.into(),
    ));

    if dir != ViewDir::Top {
        roof_symbols(drawing, scene, dir);
    }
    drawing.update_bounds();
}

/// A welded roof edge with the triangles meeting at it.
struct RoofEdge {
    a: [f64; 3],
    b: [f64; 3],
    /// Unit normals of the adjacent triangles.
    normals: Vec<[f64; 3]>,
    /// `tan` of the inclination of the sloped adjacent triangles, if any.
    tan: Option<f64>,
    /// Whether any adjacent triangle faces the camera.
    front: bool,
}

fn quant(p: [f64; 3]) -> [i64; 3] {
    p.map(|c| (c / 0.05).round() as i64)
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `rise:12` text for a slope given as `tan` of the inclination.
fn pitch_text(tan: f64) -> String {
    let rise = tan * 12.0;
    if (rise - rise.round()).abs() < 0.05 {
        format!("{}:12", rise.round() as i64)
    } else {
        format!("{rise:.1}:12")
    }
}

fn roof_symbols(drawing: &mut Drawing, scene: &Scene, dir: ViewDir) {
    let Some(bounds) = scene.bounds() else {
        return;
    };
    let proj = Projection::for_view(dir, bounds);
    let mut edges: HashMap<([i64; 3], [i64; 3]), RoofEdge> = HashMap::new();
    for mesh in scene.meshes.iter().filter(|m| m.material == Material::Roof) {
        let pos = |i: u32| mesh.vertices[i as usize].position.map(f64::from);
        for t in mesh.indices.chunks(3).filter(|t| t.len() == 3) {
            let p = [pos(t[0]), pos(t[1]), pos(t[2])];
            let c = cross(sub(p[1], p[0]), sub(p[2], p[0]));
            let len = dot(c, c).sqrt();
            if len < 1e-9 {
                continue;
            }
            let n = [c[0] / len, c[1] / len, c[2] / len];
            let tan = n[0].hypot(n[2]) / n[1].abs().max(1e-9);
            let sloped = (0.05..=6.0).contains(&tan);
            let v = p.map(|q| {
                let (pt, d) = proj.project(q);
                [pt.x, pt.y, d]
            });
            let front = cross(sub(v[1], v[0]), sub(v[2], v[0]))[2] > 1e-6;
            for k in 0..3 {
                let (a, b) = (p[k], p[(k + 1) % 3]);
                let key = if quant(a) <= quant(b) {
                    (quant(a), quant(b))
                } else {
                    (quant(b), quant(a))
                };
                let e = edges.entry(key).or_insert_with(|| RoofEdge {
                    a,
                    b,
                    normals: Vec::new(),
                    tan: None,
                    front: false,
                });
                e.normals.push(n);
                e.front |= front;
                if sloped && e.tan.is_none() {
                    e.tan = Some(tan);
                }
            }
        }
    }

    // Outline and crease edges that read as sloped lines in this view.
    let mut found: Vec<(f64, Point, Point, f64)> = Vec::new();
    for e in edges.values() {
        let Some(tan) = e.tan else { continue };
        let outline = e.normals.len() == 1
            || e.normals
                .iter()
                .skip(1)
                .any(|n| dot(*n, e.normals[0]) < 0.97);
        if !(outline && e.front) {
            continue;
        }
        let (pa, _) = proj.project(e.a);
        let (pb, _) = proj.project(e.b);
        let (dx, dy) = ((pb.x - pa.x).abs(), (pb.y - pa.y).abs());
        let len = pa.dist(pb);
        if len >= MIN_EDGE && dx >= 6.0 && (0.04..=4.0).contains(&(dy / dx)) {
            let (l, r) = if pa.x <= pb.x { (pa, pb) } else { (pb, pa) };
            found.push((len, l, r, tan));
        }
    }
    found.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.x.total_cmp(&b.1.x)));

    let mut placed: Vec<(Point, String)> = Vec::new();
    for (_, l, r, tan) in found {
        if placed.len() >= MAX_SYMBOLS {
            break;
        }
        let text = pitch_text(tan);
        let mid = Point::lerp(l, r, 0.5);
        if placed
            .iter()
            .any(|(p, t)| *t == text && p.dist(mid) < SYMBOL_SPACING)
        {
            continue;
        }
        let slope = (r.y - l.y) / (r.x - l.x);
        let y_at = |x: f64| l.y + slope * (x - l.x);
        let (xa, xb) = (mid.x - 0.5 * SYMBOL_RUN, mid.x + 0.5 * SYMBOL_RUN);
        let (a, b) = (Point::new(xa, y_at(xa)), Point::new(xb, y_at(xb)));
        // Right angle above the edge: at the upper end's height, over the lower end.
        let corner = if slope >= 0.0 {
            Point::new(xa, b.y)
        } else {
            Point::new(xb, a.y)
        };
        for (p, q) in [(a, b), (a, corner), (corner, b)] {
            drawing.lines.push(Line2 {
                a: p,
                b: q,
                weight: LineWeight::Light,
                kind: EdgeKind::Annotation,
            });
        }
        let at = Point::new(corner.x - 0.5 * text_width(&text), corner.y + 0.5 * TEXT_H);
        drawing.texts.push((at, text.clone()));
        placed.push((mid, text));
    }
}
