//! DIMENSION entities: the linear dimension the plan can hold as an object,
//! and the lines, arcs and text of the others (or of any dimension whose
//! drawing block is missing).

use crate::dxf::geom::Xf;
use crate::dxf::*;
use plan_core::Point;
use std::f64::consts::{PI, TAU};

/// The start, end and signed dimension-line offset of a linear or aligned
/// dimension, in the dimension's own units. The offset is positive on the
/// left of start to end, like [`plan_core::dimension::Dimension::offset`].
/// `None` for the other kinds and for a zero-length measurement.
pub fn linear_dimension(d: &DxfDimension) -> Option<(Point, Point, f64)> {
    if d.dtype > 1 {
        return None;
    }
    let a = d.p13;
    let dir = if d.dtype == 0 {
        let t = d.angle_deg.to_radians();
        Point::new(t.cos(), t.sin())
    } else {
        d.p14.sub(a).normalized()
    };
    let len = d.p14.sub(a).dot(dir);
    if dir.length() < 0.5 || len.abs() < 1e-9 {
        return None;
    }
    // Walk from the start toward the end along the dimension direction.
    let (start, end, dir) = if len < 0.0 {
        (a, a.add(dir.scale(len)), dir.scale(-1.0))
    } else {
        (a, a.add(dir.scale(len)), dir)
    };
    let offset = d.def_pt.sub(start).dot(dir.perp());
    Some((start, end, offset))
}

/// The text a dimension shows: the override with `<>` replaced by the
/// measurement, or `None` when the dimension shows its own measurement.
pub fn text_override(d: &DxfDimension) -> Option<String> {
    let t = d.text.trim_end();
    if t.is_empty() || t == "<>" {
        return None;
    }
    if t.contains("<>") {
        let m = d.measurement.map_or_else(String::new, fmt_measure);
        return Some(t.replace("<>", &m));
    }
    Some(t.to_string())
}

fn fmt_measure(v: f64) -> String {
    let s = format!("{v:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn ent(kind: DxfKind, props: &DxfProps) -> DxfEntity {
    DxfEntity {
        kind,
        props: props.clone(),
    }
}

fn line(a: Point, b: Point, props: &DxfProps) -> DxfEntity {
    ent(DxfKind::Line { a, b }, props)
}

/// A filled arrowhead at `tip` pointing along `dir` (toward the tip).
fn arrow(tip: Point, dir: Point, size: f64, props: &DxfProps) -> DxfEntity {
    let back = tip.sub(dir.scale(size));
    let w = dir.perp().scale(size / 6.0);
    ent(
        DxfKind::Face {
            points: vec![tip, back.add(w), back.sub(w)],
            filled: true,
        },
        props,
    )
}

fn label(pos: Point, text: &str, height: f64, angle_deg: f64, props: &DxfProps) -> DxfEntity {
    ent(
        DxfKind::Text(Box::new(DxfText {
            pos,
            h: HJust::Center,
            v: VJust::Middle,
            text: text.to_string(),
            runs: Vec::new(),
            height,
            angle_deg,
            width_factor: 1.0,
            oblique_deg: 0.0,
            style: String::new(),
            wrap_width: 0.0,
            mtext: false,
            tag: String::new(),
            attdef: false,
            invisible: false,
        })),
        props,
    )
}

/// A reading angle in (-90, 90].
fn readable(deg: f64) -> f64 {
    let mut a = deg.rem_euclid(360.0);
    if a > 90.0 && a <= 270.0 {
        a -= 180.0;
    } else if a > 270.0 {
        a -= 360.0;
    }
    a
}

/// Lines, arrowheads, arcs and text that draw `d` without a block.
pub fn dimension_entities(d: &DxfDimension, st: &DxfDimStyle, props: &DxfProps) -> Vec<DxfEntity> {
    let mut out = Vec::new();
    let txt_h = st.sized(st.text_height).max(1e-9);
    let asz = st.sized(st.arrow);
    let exo = st.sized(st.ext_offset);
    let exe = st.sized(st.ext_extend);
    let shown = |m: f64, prefix: &str, suffix: &str| -> String {
        text_override(d).unwrap_or_else(|| format!("{prefix}{}{suffix}", fmt_measure(m)))
    };
    match d.dtype {
        0 | 1 => {
            let a = d.p13;
            let b = d.p14;
            let dir = if d.dtype == 0 {
                let t = d.angle_deg.to_radians();
                Point::new(t.cos(), t.sin())
            } else {
                b.sub(a).normalized()
            };
            let n = dir.perp();
            let s = if d.def_pt.sub(a).dot(n) >= 0.0 {
                1.0
            } else {
                -1.0
            };
            let foot = |p: Point| p.add(n.scale(d.def_pt.sub(p).dot(n)));
            let (fa, fb) = (foot(a), foot(b));
            for (p, f) in [(a, fa), (b, fb)] {
                let sgn = if d.def_pt.sub(p).dot(n) >= 0.0 {
                    1.0
                } else {
                    -1.0
                };
                out.push(line(
                    p.add(n.scale(exo * sgn)),
                    f.add(n.scale(exe * sgn)),
                    props,
                ));
            }
            out.push(line(fa, fb, props));
            let span = fb.sub(fa);
            if span.length() > 1e-9 {
                let u = span.normalized();
                out.push(arrow(fa, u.scale(-1.0), asz, props));
                out.push(arrow(fb, u, asz, props));
            }
            let m = b.sub(a).dot(dir).abs() * st.linear_factor;
            let at = if d.text_pt == Point::ZERO {
                Point::lerp(fa, fb, 0.5).add(n.scale(s * (txt_h * 0.5 + st.sized(st.gap))))
            } else {
                d.text_pt
            };
            out.push(label(
                at,
                &shown(d.measurement.unwrap_or(m), "", ""),
                txt_h,
                readable(dir.angle().to_degrees()),
                props,
            ));
        }
        3 | 4 => {
            // Diameter: 10 and 15 are opposite points on the circle. Radius:
            // 10 is the centre and 15 the point on the circle.
            let (p, q) = (d.def_pt, d.p15);
            out.push(line(p, q, props));
            let dir = q.sub(p).normalized();
            out.push(arrow(q, dir, asz, props));
            let (m, pre) = if d.dtype == 3 {
                (p.dist(q), "\u{d8}")
            } else {
                (p.dist(q), "R")
            };
            let at = if d.text_pt == Point::ZERO {
                q.add(dir.scale(txt_h * 2.0))
            } else {
                d.text_pt
            };
            out.push(label(
                at,
                &shown(d.measurement.unwrap_or(m), pre, ""),
                txt_h,
                readable(dir.angle().to_degrees()),
                props,
            ));
        }
        2 | 5 => {
            // 2-line: lines 13-14 and 15-10, arc through 16. 3-point: the
            // vertex is 15, the ends are 13 and 14 and the arc passes 10.
            let (vertex, e1, e2, arc_pt) = if d.dtype == 5 {
                (d.p15, d.p13, d.p14, d.def_pt)
            } else {
                let v = line_meet(d.p13, d.p14, d.p15, d.def_pt).unwrap_or(d.p14);
                (v, d.p14, d.def_pt, d.p16)
            };
            let r = arc_pt.dist(vertex);
            let (a1, a2) = (e1.sub(vertex).angle(), e2.sub(vertex).angle());
            let mid = arc_pt.sub(vertex).angle();
            // Pick the sweep (ccw from a_start to a_end) that holds the arc point.
            let ccw12 = (mid - a1).rem_euclid(TAU) <= (a2 - a1).rem_euclid(TAU);
            let (s, e) = if ccw12 { (a1, a2) } else { (a2, a1) };
            if r > 1e-9 {
                out.push(ent(
                    DxfKind::Arc {
                        center: vertex,
                        radius: r,
                        start_deg: s.to_degrees(),
                        end_deg: e.to_degrees(),
                    },
                    props,
                ));
                out.push(line(
                    e1,
                    vertex.add(Point::new(a1.cos(), a1.sin()).scale(r)),
                    props,
                ));
                out.push(line(
                    e2,
                    vertex.add(Point::new(a2.cos(), a2.sin()).scale(r)),
                    props,
                ));
            }
            let sweep = (e - s).rem_euclid(TAU);
            let deg = d.measurement.map_or(sweep.to_degrees(), f64::to_degrees);
            let m = if deg > 360.0 { sweep.to_degrees() } else { deg };
            let at = if d.text_pt == Point::ZERO {
                let t = s + sweep / 2.0;
                vertex.add(Point::new(t.cos(), t.sin()).scale(r + txt_h))
            } else {
                d.text_pt
            };
            let tang = (mid + PI / 2.0).to_degrees();
            out.push(label(
                at,
                &shown(m, "", "\u{b0}"),
                txt_h,
                readable(tang),
                props,
            ));
        }
        _ => {
            // Ordinate: a leader from the feature point to the text.
            out.push(line(d.p13, d.p14, props));
            let at = if d.text_pt == Point::ZERO {
                d.p14
            } else {
                d.text_pt
            };
            let m = if d.p13.x != d.p14.x { d.p13.x } else { d.p13.y };
            out.push(label(
                at,
                &shown(d.measurement.unwrap_or(m), "", ""),
                txt_h,
                0.0,
                props,
            ));
        }
    }
    out
}

/// Intersection of the infinite lines `a b` and `c d`.
fn line_meet(a: Point, b: Point, c: Point, d: Point) -> Option<Point> {
    plan_core::cad::line_intersection(a, b, c, d)
}

/// Apply `xf` to the points of a dimension.
pub fn xform_dimension(xf: &Xf, d: &DxfDimension) -> DxfDimension {
    DxfDimension {
        def_pt: xf.point(d.def_pt),
        text_pt: if d.text_pt == Point::ZERO {
            d.text_pt
        } else {
            xf.point(d.text_pt)
        },
        p13: xf.point(d.p13),
        p14: xf.point(d.p14),
        p15: xf.point(d.p15),
        p16: xf.point(d.p16),
        angle_deg: xf.angle(d.angle_deg),
        measurement: d.measurement,
        ..d.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dim(dtype: i32, p10: Point, p13: Point, p14: Point, angle: f64) -> DxfDimension {
        DxfDimension {
            dtype,
            block: String::new(),
            def_pt: p10,
            text_pt: Point::ZERO,
            p13,
            p14,
            p15: Point::ZERO,
            p16: Point::ZERO,
            angle_deg: angle,
            text: String::new(),
            measurement: None,
            style: String::new(),
        }
    }

    #[test]
    fn aligned_and_rotated_dimensions_have_start_end_and_offset() {
        // Horizontal 100 long, dimension line 20 above.
        let d = dim(
            0,
            Point::new(50.0, 20.0),
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            0.0,
        );
        let (s, e, off) = linear_dimension(&d).unwrap();
        assert_eq!((s, e), (Point::new(0.0, 0.0), Point::new(100.0, 0.0)));
        assert!((off - 20.0).abs() < 1e-9);
        // Below the line: negative offset (the left side walking start to end is up).
        let below = dim(
            1,
            Point::new(50.0, -8.0),
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            0.0,
        );
        assert!((linear_dimension(&below).unwrap().2 + 8.0).abs() < 1e-9);
        // A horizontal dimension of a slanted pair measures the x run.
        let slanted = dim(
            0,
            Point::new(0.0, 30.0),
            Point::new(0.0, 0.0),
            Point::new(60.0, 20.0),
            0.0,
        );
        let (s, e, _) = linear_dimension(&slanted).unwrap();
        assert!((e.sub(s).length() - 60.0).abs() < 1e-9 && e.y.abs() < 1e-9);
        // A pair walked backwards reads start to end along the dimension.
        let back = dim(
            0,
            Point::new(0.0, 5.0),
            Point::new(100.0, 0.0),
            Point::new(0.0, 0.0),
            0.0,
        );
        let (s, e, _) = linear_dimension(&back).unwrap();
        assert!((s.x - 100.0).abs() < 1e-9 && e.x.abs() < 1e-9);
        assert!(linear_dimension(&dim(2, Point::ZERO, Point::ZERO, Point::ZERO, 0.0)).is_none());
    }

    #[test]
    fn dimension_text_replaces_the_marker() {
        let mut d = dim(0, Point::ZERO, Point::ZERO, Point::new(10.0, 0.0), 0.0);
        assert_eq!(text_override(&d), None);
        d.text = "<>".into();
        assert_eq!(text_override(&d), None);
        d.text = "EQ".into();
        assert_eq!(text_override(&d).as_deref(), Some("EQ"));
        d.text = "<> TYP".into();
        d.measurement = Some(12.5);
        assert_eq!(text_override(&d).as_deref(), Some("12.5 TYP"));
    }

    #[test]
    fn a_missing_block_is_drawn_from_the_definition_points() {
        let d = dim(
            1,
            Point::new(50.0, 20.0),
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            0.0,
        );
        let st = DxfDimStyle::default();
        let es = dimension_entities(&d, &st, &DxfProps::default());
        let lines = es
            .iter()
            .filter(|e| matches!(e.kind, DxfKind::Line { .. }))
            .count();
        let texts: Vec<&DxfText> = es
            .iter()
            .filter_map(|e| match &e.kind {
                DxfKind::Text(t) => Some(t.as_ref()),
                _ => None,
            })
            .collect();
        assert_eq!(lines, 3, "two extension lines and the dimension line");
        assert_eq!(texts[0].text, "100");
        // An angular dimension gets its arc and two extension lines.
        let mut a = dim(
            5,
            Point::new(7.0, 7.0),
            Point::new(10.0, 0.0),
            Point::new(0.0, 10.0),
            0.0,
        );
        a.p15 = Point::ZERO;
        let es = dimension_entities(&a, &st, &DxfProps::default());
        let arc = es.iter().find_map(|e| match e.kind {
            DxfKind::Arc {
                radius,
                start_deg,
                end_deg,
                ..
            } => Some((radius, start_deg, end_deg)),
            _ => None,
        });
        let (r, s, e) = arc.unwrap();
        assert!((r - 7.0 * 2f64.sqrt()).abs() < 1e-9);
        assert!(s.abs() < 1e-9 && (e - 90.0).abs() < 1e-9);
        let label = es.iter().find_map(|e| match &e.kind {
            DxfKind::Text(t) => Some(t.text.clone()),
            _ => None,
        });
        assert_eq!(label.as_deref(), Some("90\u{b0}"));
    }
}
