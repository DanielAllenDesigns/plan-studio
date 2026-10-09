//! DXF sections to a [`DxfDrawing`]: header, tables, blocks and entities.

use super::geom::{ellipse_points, ellipse_range, sample_polyline, Nurbs};
use super::model::*;
use super::text::{clean_text, mtext_runs};
use super::tokens::Pair;
use plan_core::Point;
use std::collections::BTreeMap;
use std::f64::consts::TAU;

/// A `0`-group record: the entity type and the pairs that follow it.
pub(super) struct Rec<'p> {
    pub kind: &'p str,
    pub pairs: &'p [Pair<'p>],
}

impl<'p> Rec<'p> {
    pub fn get(&self, code: i32) -> Option<&'p str> {
        self.pairs
            .iter()
            .find(|(c, _)| *c == code)
            .map(|(_, v)| v.as_ref())
    }

    pub fn f(&self, code: i32) -> Option<f64> {
        self.get(code).and_then(parse_f64)
    }

    pub fn int(&self, code: i32) -> Option<i32> {
        self.get(code).and_then(parse_i)
    }

    pub fn all(&self, code: i32) -> impl Iterator<Item = &'p str> + '_ {
        self.pairs
            .iter()
            .filter(move |(c, _)| *c == code)
            .map(|(_, v)| v.as_ref())
    }

    /// Point from group codes `base` (x) and `base + 10` (y).
    pub fn point(&self, base: i32) -> Point {
        Point::new(
            self.f(base).unwrap_or(0.0),
            self.f(base + 10).unwrap_or(0.0),
        )
    }

    pub fn has(&self, code: i32) -> bool {
        self.pairs.iter().any(|(c, _)| *c == code)
    }

    /// Points written as repeated `base` / `base + 10` pairs.
    pub fn points(&self, base: i32) -> Vec<Point> {
        let mut out: Vec<Point> = Vec::new();
        for (code, v) in self.pairs {
            if *code == base {
                out.push(Point::new(parse_f64(v).unwrap_or(0.0), 0.0));
            } else if *code == base + 10 {
                if let Some(p) = out.last_mut() {
                    p.y = parse_f64(v).unwrap_or(0.0);
                }
            }
        }
        out
    }

    fn layer(&self) -> String {
        self.get(8).map_or("0", str::trim).to_string()
    }

    /// The extrusion direction points down (the OCS x axis is mirrored).
    fn flipped(&self) -> bool {
        let z = self.f(230).unwrap_or(1.0);
        let (x, y) = (self.f(210).unwrap_or(0.0), self.f(220).unwrap_or(0.0));
        z < 0.0 && x.abs() < 1e-9 && y.abs() < 1e-9
    }
}

pub(super) fn parse_f64(s: &str) -> Option<f64> {
    s.trim().parse().ok()
}

fn parse_i(s: &str) -> Option<i32> {
    let t = s.trim();
    t.parse().ok().or_else(|| t.parse::<f64>().ok().map(|f| f as i32))
}

/// Group pairs into records at each `0` code; `EOF` ends the file.
pub(super) fn records<'p>(pairs: &'p [Pair<'p>]) -> Vec<Rec<'p>> {
    let mut out: Vec<Rec<'p>> = Vec::new();
    let mut start: Option<(usize, &'p str)> = None;
    for (i, (code, value)) in pairs.iter().enumerate() {
        if *code != 0 {
            continue;
        }
        if let Some((s, kind)) = start.take() {
            out.push(Rec {
                kind,
                pairs: &pairs[s + 1..i],
            });
        }
        let kind = value.trim();
        if kind == "EOF" {
            return out;
        }
        start = Some((i, kind));
    }
    if let Some((s, kind)) = start {
        out.push(Rec {
            kind,
            pairs: &pairs[s + 1..],
        });
    }
    out
}

/// Counts of entity types that were not imported.
#[derive(Default)]
pub(super) struct Skipped(pub BTreeMap<String, usize>);

impl Skipped {
    fn add(&mut self, what: &str) {
        *self.0.entry(what.to_string()).or_default() += 1;
    }
}

/// Fills `drawing` from the sections of a record list.
pub(super) fn read_drawing(recs: &[Rec<'_>], drawing: &mut DxfDrawing) {
    let mut skipped = Skipped::default();
    let mut i = 0;
    while i < recs.len() {
        if recs[i].kind != "SECTION" {
            i += 1;
            continue;
        }
        let name = recs[i].get(2).map_or("", str::trim);
        let end = recs[i + 1..]
            .iter()
            .position(|r| r.kind == "ENDSEC" || r.kind == "SECTION")
            .map_or(recs.len(), |p| i + 1 + p);
        let body = &recs[i + 1..end];
        match name {
            "HEADER" => read_header(&recs[i], drawing),
            "TABLES" => read_tables(body, drawing),
            "BLOCKS" => read_blocks(body, drawing, &mut skipped),
            "ENTITIES" => {
                let mut model = Vec::new();
                for e in read_entities(body, &mut skipped) {
                    if e.props.paper {
                        drawing.paper_entities.push(e);
                    } else {
                        model.push(e);
                    }
                }
                model.append(&mut drawing.entities);
                drawing.entities = model;
            }
            _ => {}
        }
        // Resume at a SECTION that followed a missing ENDSEC; otherwise skip ENDSEC.
        i = if recs.get(end).is_some_and(|r| r.kind == "SECTION") {
            end
        } else {
            end + 1
        };
    }
    drawing.skipped = skipped.0.into_iter().collect();
}

fn read_header(section: &Rec<'_>, d: &mut DxfDrawing) {
    let mut var = "";
    let (mut min, mut max) = ((None, None), (None, None));
    let hd = &mut d.header_dim;
    for (code, value) in section.pairs {
        let value = value.as_ref();
        if *code == 9 {
            var = value.trim();
            continue;
        }
        match (var, *code) {
            ("$ACADVER", 1) => d.version = value.trim().to_string(),
            ("$INSUNITS", 70) => {
                if let Some(n) = parse_i(value) {
                    d.units = DxfUnits::from_code(n);
                }
            }
            ("$MEASUREMENT", 70) => d.metric = parse_i(value) == Some(1),
            ("$LTSCALE", 40) => d.ltscale = parse_f64(value).unwrap_or(1.0),
            ("$DIMSCALE", 40) => {
                let s = parse_f64(value).unwrap_or(1.0);
                d.dim_scale = s;
                hd.scale = s;
            }
            ("$DIMASZ", 40) => hd.arrow = parse_f64(value).unwrap_or(hd.arrow),
            ("$DIMEXO", 40) => hd.ext_offset = parse_f64(value).unwrap_or(hd.ext_offset),
            ("$DIMEXE", 40) => hd.ext_extend = parse_f64(value).unwrap_or(hd.ext_extend),
            ("$DIMTXT", 40) => hd.text_height = parse_f64(value).unwrap_or(hd.text_height),
            ("$DIMGAP", 40) => hd.gap = parse_f64(value).unwrap_or(hd.gap),
            ("$DIMLFAC", 40) => hd.linear_factor = parse_f64(value).unwrap_or(1.0),
            ("$EXTMIN", 10) => min.0 = parse_f64(value),
            ("$EXTMIN", 20) => min.1 = parse_f64(value),
            ("$EXTMAX", 10) => max.0 = parse_f64(value),
            ("$EXTMAX", 20) => max.1 = parse_f64(value),
            _ => {}
        }
    }
    if let ((Some(x0), Some(y0)), (Some(x1), Some(y1))) = (min, max) {
        // AutoCAD writes +/-1e20 for an empty drawing.
        if [x0, y0, x1, y1].iter().all(|v| v.abs() < 1e19) {
            d.extents = Some((Point::new(x0, y0), Point::new(x1, y1)));
        }
    }
}

fn true_color(v: i32) -> DxfColor {
    let u = v as u32;
    DxfColor::Rgb([((u >> 16) & 255) as u8, ((u >> 8) & 255) as u8, (u & 255) as u8])
}

fn read_tables(body: &[Rec<'_>], d: &mut DxfDrawing) {
    for r in body {
        match r.kind {
            "LAYER" => {
                let Some(name) = r.get(2).map(|n| n.trim().to_string()) else {
                    continue;
                };
                if d.layers.iter().any(|l| l.name == name) {
                    continue;
                }
                let raw = r.int(62).unwrap_or(7);
                let flags = r.int(70).unwrap_or(0);
                let color = match r.int(420) {
                    Some(t) => true_color(t),
                    None => DxfColor::Aci(raw.unsigned_abs().clamp(1, 255) as u8),
                };
                d.layers.push(DxfLayer {
                    name,
                    color,
                    visible: raw >= 0 && flags & 1 == 0,
                    frozen: flags & 1 != 0,
                    locked: flags & 4 != 0,
                    linetype: r.get(6).map_or("CONTINUOUS", str::trim).to_string(),
                    weight: r.int(370).unwrap_or(WEIGHT_DEFAULT),
                    plot: r.int(290).unwrap_or(1) != 0,
                });
            }
            "LTYPE" => {
                let Some(name) = r.get(2).map(|n| n.trim().to_string()) else {
                    continue;
                };
                d.linetypes.push(DxfLinetype {
                    name,
                    description: r.get(3).unwrap_or("").trim().to_string(),
                    pattern: r.all(49).filter_map(parse_f64).collect(),
                });
            }
            "STYLE" => {
                let Some(name) = r.get(2).map(|n| n.trim().to_string()) else {
                    continue;
                };
                d.text_styles.push(DxfTextStyle {
                    name,
                    font: r.get(3).unwrap_or("").trim().to_string(),
                    width_factor: r.f(41).filter(|w| *w > 0.0).unwrap_or(1.0),
                    oblique_deg: r.f(50).unwrap_or(0.0),
                    height: r.f(40).unwrap_or(0.0),
                });
            }
            "DIMSTYLE" => {
                let Some(name) = r.get(2).map(|n| n.trim().to_string()) else {
                    continue;
                };
                let base = DxfDimStyle::default();
                d.dim_styles.push(DxfDimStyle {
                    name,
                    scale: r.f(40).unwrap_or(base.scale),
                    arrow: r.f(41).unwrap_or(base.arrow),
                    ext_offset: r.f(42).unwrap_or(base.ext_offset),
                    ext_extend: r.f(44).unwrap_or(base.ext_extend),
                    text_height: r.f(140).unwrap_or(base.text_height),
                    gap: r.f(147).unwrap_or(base.gap),
                    linear_factor: r.f(144).unwrap_or(base.linear_factor),
                });
            }
            _ => {}
        }
    }
}

fn read_blocks(body: &[Rec<'_>], d: &mut DxfDrawing, skipped: &mut Skipped) {
    let mut i = 0;
    while i < body.len() {
        if body[i].kind != "BLOCK" {
            i += 1;
            continue;
        }
        let head = &body[i];
        let end = body[i + 1..]
            .iter()
            .position(|r| r.kind == "ENDBLK" || r.kind == "BLOCK")
            .map_or(body.len(), |p| i + 1 + p);
        let name = head
            .get(2)
            .or_else(|| head.get(3))
            .map_or("", str::trim)
            .to_string();
        let flags = head.int(70).unwrap_or(0);
        let xref = if flags & (4 | 8) != 0 {
            let path = head.get(1).map_or("", str::trim);
            if path.is_empty() { name.clone() } else { path.to_string() }
        } else {
            String::new()
        };
        let lower = name.to_ascii_lowercase();
        let mut entities = read_entities(&body[i + 1..end], skipped);
        if lower == "*paper_space" {
            for mut e in entities {
                e.props.paper = true;
                d.paper_entities.push(e);
            }
        } else if lower == "*model_space" {
            d.entities.append(&mut entities);
        } else if lower.starts_with("*paper_space") {
            // Later pages of paper space are not recognised.
        } else {
            if !xref.is_empty() && !d.xrefs.contains(&xref) {
                d.xrefs.push(xref.clone());
            }
            let block = DxfBlock {
                base: head.point(10),
                entities,
                anonymous: flags & 1 != 0 || name.starts_with('*'),
                xref,
                name,
            };
            d.blocks.insert(block.name.to_uppercase(), block);
        }
        i = end;
    }
}

fn props_of(r: &Rec<'_>) -> DxfProps {
    let color = match (r.int(420), r.int(62)) {
        (Some(t), _) => true_color(t),
        (None, Some(0)) => DxfColor::ByBlock,
        (None, Some(256) | None) => DxfColor::ByLayer,
        (None, Some(n)) => DxfColor::Aci(n.unsigned_abs().clamp(1, 255) as u8),
    };
    DxfProps {
        layer: r.layer(),
        color,
        weight: r.int(370).unwrap_or(WEIGHT_BY_LAYER),
        linetype: r.get(6).map_or("", str::trim).to_string(),
        paper: r.int(67).unwrap_or(0) == 1,
    }
}

/// Convert a run of entity records. Unknown types are counted in `skipped`.
pub(super) fn read_entities(recs: &[Rec<'_>], skipped: &mut Skipped) -> Vec<DxfEntity> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < recs.len() {
        let r = &recs[i];
        i += 1;
        // Invisible entities are not drawn.
        if r.int(60).unwrap_or(0) == 1 {
            if r.kind == "POLYLINE" || r.kind == "INSERT" {
                while i < recs.len() && matches!(recs[i].kind, "VERTEX" | "ATTRIB") {
                    i += 1;
                }
                if i < recs.len() && recs[i].kind == "SEQEND" {
                    i += 1;
                }
            }
            continue;
        }
        let props = props_of(r);
        let mut push = |kind: DxfKind| {
            out.push(DxfEntity {
                kind,
                props: props.clone(),
            });
        };
        match r.kind {
            "LINE" => push(DxfKind::Line {
                a: r.point(10),
                b: r.point(11),
            }),
            "LWPOLYLINE" => match lwpolyline(r) {
                Some(k) => push(k),
                None => skipped.add(r.kind),
            },
            "POLYLINE" => {
                let start = i;
                while i < recs.len() && recs[i].kind == "VERTEX" {
                    i += 1;
                }
                let vertices = &recs[start..i];
                if i < recs.len() && recs[i].kind == "SEQEND" {
                    i += 1;
                }
                match polyline(r, vertices) {
                    Some(kinds) => kinds.into_iter().for_each(&mut push),
                    None => skipped.add("POLYLINE (3D mesh)"),
                }
            }
            "CIRCLE" => {
                let mut c = r.point(10);
                if r.flipped() {
                    c.x = -c.x;
                }
                push(DxfKind::Circle {
                    center: c,
                    radius: r.f(40).unwrap_or(0.0),
                });
            }
            "ARC" => {
                let mut c = r.point(10);
                let (mut a0, mut a1) = (r.f(50).unwrap_or(0.0), r.f(51).unwrap_or(360.0));
                if r.flipped() {
                    c.x = -c.x;
                    (a0, a1) = (180.0 - a1, 180.0 - a0);
                }
                push(DxfKind::Arc {
                    center: c,
                    radius: r.f(40).unwrap_or(0.0),
                    start_deg: a0,
                    end_deg: a1,
                });
            }
            "ELLIPSE" => push(ellipse(r)),
            "SPLINE" => match spline(r) {
                Some(k) => push(k),
                None => skipped.add(r.kind),
            },
            "HATCH" => match hatch(r) {
                Some(h) => push(DxfKind::Hatch(Box::new(h))),
                None => skipped.add(r.kind),
            },
            "SOLID" | "TRACE" => push(DxfKind::Face {
                points: face_points(r, true),
                filled: true,
            }),
            "3DFACE" => push(DxfKind::Face {
                points: face_points(r, false),
                filled: false,
            }),
            "POINT" => push(DxfKind::Marker { pos: r.point(10) }),
            "TEXT" => push(DxfKind::Text(Box::new(text_entity(r, false)))),
            "ATTRIB" => push(DxfKind::Text(Box::new(text_entity(r, true)))),
            "ATTDEF" => {
                let mut t = text_entity(r, true);
                t.attdef = true;
                push(DxfKind::Text(Box::new(t)));
            }
            "MTEXT" => push(DxfKind::Text(Box::new(mtext_entity(r)))),
            "DIMENSION" => push(DxfKind::Dimension(Box::new(dimension(r)))),
            "LEADER" => {
                let points = r.points(10);
                if points.len() >= 2 {
                    push(DxfKind::Leader {
                        points,
                        arrow: r.int(71).unwrap_or(1) != 0,
                    });
                } else {
                    skipped.add(r.kind);
                }
            }
            "MULTILEADER" | "MLEADER" => match mleader(r) {
                Some(m) => push(DxfKind::MLeader(Box::new(m))),
                None => skipped.add("MULTILEADER"),
            },
            "INSERT" => {
                let mut attribs = Vec::new();
                while i < recs.len() && recs[i].kind == "ATTRIB" {
                    let a = &recs[i];
                    i += 1;
                    if a.int(60).unwrap_or(0) == 1 {
                        continue;
                    }
                    let mut text = text_entity(a, true);
                    // An invisible attribute (flag 1) is not shown.
                    text.invisible = a.int(70).unwrap_or(0) & 1 != 0;
                    attribs.push(DxfEntity {
                        kind: DxfKind::Text(Box::new(text)),
                        props: props_of(a),
                    });
                }
                if i < recs.len() && recs[i].kind == "SEQEND" {
                    i += 1;
                }
                push(insert(r, attribs));
            }
            // Stray terminators are not content.
            "SEQEND" | "ENDBLK" | "VERTEX" | "BLOCK" => {}
            other => skipped.add(other),
        }
    }
    out
}

fn insert(r: &Rec<'_>, attribs: Vec<DxfEntity>) -> DxfKind {
    let mut pos = r.point(10);
    let mut rot = r.f(50).unwrap_or(0.0);
    let (sx, mut sy) = (r.f(41).unwrap_or(1.0), r.f(42).unwrap_or(1.0));
    if r.flipped() {
        // x mirrored: M R(a) S(sx, sy) = R(180 - a) S(sx, -sy).
        pos.x = -pos.x;
        rot = 180.0 - rot;
        sy = -sy;
    }
    DxfKind::Insert(Box::new(DxfInsert {
        block: r.get(2).map_or("", str::trim).to_string(),
        pos,
        scale: (sx, sy),
        rotation_deg: rot,
        attribs,
        columns: r.int(70).unwrap_or(1).max(1) as u32,
        rows: r.int(71).unwrap_or(1).max(1) as u32,
        col_spacing: r.f(44).unwrap_or(0.0),
        row_spacing: r.f(45).unwrap_or(0.0),
    }))
}

fn lwpolyline(r: &Rec<'_>) -> Option<DxfKind> {
    let mut points: Vec<Point> = Vec::new();
    let mut bulges: Vec<f64> = Vec::new();
    for (code, value) in r.pairs {
        let v = parse_f64(value);
        match (*code, v) {
            (10, Some(x)) => {
                points.push(Point::new(x, 0.0));
                bulges.push(0.0);
            }
            (20, Some(y)) => {
                if let Some(p) = points.last_mut() {
                    p.y = y;
                }
            }
            (42, Some(b)) => {
                if let Some(slot) = bulges.last_mut() {
                    *slot = b;
                }
            }
            _ => {}
        }
    }
    if points.len() < 2 {
        return None;
    }
    if r.flipped() {
        points.iter_mut().for_each(|p| p.x = -p.x);
        bulges.iter_mut().for_each(|b| *b = -*b);
    }
    Some(DxfKind::Polyline {
        points,
        closed: r.int(70).unwrap_or(0) & 1 != 0,
        bulges,
    })
}

fn polyline(head: &Rec<'_>, vertices: &[Rec<'_>]) -> Option<Vec<DxfKind>> {
    let flags = head.int(70).unwrap_or(0);
    // 3D meshes (16) are not outlines.
    if flags & 16 != 0 {
        return None;
    }
    if flags & 64 != 0 {
        return Some(polyface(vertices));
    }
    let mut points = Vec::new();
    let mut bulges = Vec::new();
    for v in vertices {
        // Spline frame control points (16) are not part of the curve.
        if v.int(70).unwrap_or(0) & 16 != 0 {
            continue;
        }
        points.push(v.point(10));
        bulges.push(v.f(42).unwrap_or(0.0));
    }
    if points.len() < 2 {
        return Some(Vec::new());
    }
    if head.flipped() {
        points.iter_mut().for_each(|p| p.x = -p.x);
        bulges.iter_mut().for_each(|b| *b = -*b);
    }
    Some(vec![DxfKind::Polyline {
        points,
        closed: flags & 1 != 0,
        bulges,
    }])
}

/// A polyface mesh: position vertices (flag 64) then face records (flag 128
/// with 1-based corner indices in groups 71 to 74, negative for a hidden
/// edge). Each face is an outline.
fn polyface(vertices: &[Rec<'_>]) -> Vec<DxfKind> {
    let mut pos: Vec<Point> = Vec::new();
    let mut faces: Vec<Vec<i32>> = Vec::new();
    for v in vertices {
        let f = v.int(70).unwrap_or(0);
        if f & 64 != 0 {
            pos.push(v.point(10));
        } else if f & 128 != 0 {
            faces.push(
                [71, 72, 73, 74]
                    .iter()
                    .filter_map(|c| v.int(*c))
                    .filter(|i| *i != 0)
                    .collect(),
            );
        }
    }
    faces
        .into_iter()
        .filter_map(|idx| {
            let pts: Vec<Point> = idx
                .iter()
                .filter_map(|i| pos.get(i.unsigned_abs() as usize - 1).copied())
                .collect();
            (pts.len() >= 3).then_some(DxfKind::Face {
                points: pts,
                filled: false,
            })
        })
        .collect()
}

fn ellipse(r: &Rec<'_>) -> DxfKind {
    let u = r.point(11);
    let ratio = r.f(40).unwrap_or(1.0);
    let sign = if r.f(230).unwrap_or(1.0) < 0.0 { -1.0 } else { 1.0 };
    let (t0, t1) = ellipse_range(r.f(41).unwrap_or(0.0), r.f(42).unwrap_or(TAU));
    DxfKind::Ellipse {
        center: r.point(10),
        u,
        v: u.perp().scale(ratio * sign),
        t0,
        t1,
    }
}

fn spline(r: &Rec<'_>) -> Option<DxfKind> {
    let flags = r.int(70).unwrap_or(0);
    let control = r.points(10);
    let fit = r.points(11);
    if control.len() < 2 && fit.len() < 2 {
        return None;
    }
    Some(DxfKind::Spline {
        degree: r.int(71).unwrap_or(3).max(1) as usize,
        closed: flags & 1 != 0,
        knots: r.all(40).filter_map(parse_f64).collect(),
        weights: r.all(41).filter_map(parse_f64).collect(),
        control,
        fit,
    })
}

fn face_points(r: &Rec<'_>, solid: bool) -> Vec<Point> {
    let (p0, p1, p2, p3) = (r.point(10), r.point(11), r.point(12), r.point(13));
    let tri = !r.has(13) || p2 == p3;
    if solid {
        // SOLID corners run 1, 2, 4, 3 around the outline.
        if tri {
            vec![p0, p1, p2]
        } else {
            vec![p0, p1, p3, p2]
        }
    } else if tri {
        vec![p0, p1, p2]
    } else {
        vec![p0, p1, p2, p3]
    }
}

fn text_entity(r: &Rec<'_>, attrib: bool) -> DxfText {
    let height = r.f(40).unwrap_or(0.0);
    let mut angle_deg = r.f(50).unwrap_or(0.0);
    let text = clean_text(r.get(1).unwrap_or(""));
    // ATTRIB and ATTDEF carry the vertical justification in 74.
    let (hj, vj) = (r.int(72).unwrap_or(0), r.int(if attrib { 74 } else { 73 }).unwrap_or(0));
    let p10 = r.point(10);
    let p11 = if r.has(11) { r.point(11) } else { p10 };
    let (h, v, mut pos) = match (hj, vj) {
        // Aligned (3) and fit (5) text runs from the first to the second point.
        (3 | 5, _) => {
            let d = p11.sub(p10);
            if d.length() > 1e-12 {
                angle_deg = d.angle().to_degrees();
            }
            (HJust::Left, VJust::Baseline, p10)
        }
        (4, _) => (HJust::Center, VJust::Middle, p11),
        (0, 0) => (HJust::Left, VJust::Baseline, p10),
        (hj, vj) => (
            match hj {
                1 => HJust::Center,
                2 => HJust::Right,
                _ => HJust::Left,
            },
            match vj {
                1 => VJust::Bottom,
                2 => VJust::Middle,
                3 => VJust::Top,
                _ => VJust::Baseline,
            },
            p11,
        ),
    };
    if r.flipped() {
        pos.x = -pos.x;
        angle_deg = 180.0 - angle_deg;
    }
    DxfText {
        pos,
        h,
        v,
        text,
        runs: Vec::new(),
        height,
        angle_deg,
        width_factor: r.f(41).filter(|w| *w > 0.0).unwrap_or(1.0),
        oblique_deg: r.f(51).unwrap_or(0.0),
        style: r.get(7).map_or("", str::trim).to_string(),
        wrap_width: 0.0,
        mtext: false,
        tag: r.get(2).map_or("", str::trim).to_string(),
        attdef: false,
        invisible: false,
    }
}

fn mtext_entity(r: &Rec<'_>) -> DxfText {
    // Long text is split: any number of group-3 chunks, then the final group 1.
    let mut raw = String::new();
    for (code, value) in r.pairs {
        if *code == 3 {
            raw.push_str(value);
        }
    }
    raw.push_str(r.get(1).unwrap_or(""));
    let height = r.f(40).unwrap_or(0.0);
    // The direction vector (11/21) overrides the rotation angle when present.
    let angle_deg = match (r.f(11), r.f(21)) {
        (Some(x), Some(y)) if x != 0.0 || y != 0.0 => y.atan2(x).to_degrees(),
        _ => r.f(50).unwrap_or(0.0),
    };
    let attach = r.int(71).unwrap_or(1).clamp(1, 9) - 1;
    let (runs, text) = mtext_runs(&raw, height);
    DxfText {
        pos: r.point(10),
        h: match attach % 3 {
            1 => HJust::Center,
            2 => HJust::Right,
            _ => HJust::Left,
        },
        v: match attach / 3 {
            0 => VJust::Top,
            1 => VJust::Middle,
            _ => VJust::Bottom,
        },
        text,
        runs,
        height,
        angle_deg,
        width_factor: 1.0,
        oblique_deg: 0.0,
        style: r.get(7).map_or("", str::trim).to_string(),
        wrap_width: r.f(41).unwrap_or(0.0).max(0.0),
        mtext: true,
        tag: String::new(),
        attdef: false,
        invisible: false,
    }
}

fn dimension(r: &Rec<'_>) -> DxfDimension {
    DxfDimension {
        dtype: r.int(70).unwrap_or(0) & 7,
        block: r.get(2).map_or("", str::trim).to_string(),
        def_pt: r.point(10),
        text_pt: r.point(11),
        p13: r.point(13),
        p14: r.point(14),
        p15: r.point(15),
        p16: r.point(16),
        angle_deg: r.f(50).unwrap_or(0.0),
        text: clean_text(r.get(1).unwrap_or("")),
        measurement: r.f(42),
        style: r.get(3).map_or("", str::trim).to_string(),
    }
}

/// MULTILEADER: leader lines inside `LEADER_LINE{ ... }` groups of the
/// context data, and the text content.
fn mleader(r: &Rec<'_>) -> Option<DxfMLeader> {
    let mut lines: Vec<Vec<Point>> = Vec::new();
    let mut cur: Option<Vec<Point>> = None;
    let mut text = String::new();
    let mut text_pos = None;
    let mut height = 0.0;
    let mut angle = 0.0;
    let arrow = true;
    let mut x: Option<f64> = None;
    for (code, value) in r.pairs {
        let v = value.trim();
        match *code {
            304 if v.starts_with("LEADER_LINE") => cur = Some(Vec::new()),
            304 if v == "DEFAULT_TEXT_CONTENT" => {}
            304 => text = super::text::mtext_runs(value, 0.0).1,
            305 => {
                if let Some(l) = cur.take() {
                    if l.len() >= 2 {
                        lines.push(l);
                    }
                }
            }
            10 if cur.is_some() => x = parse_f64(v),
            20 if cur.is_some() => {
                if let (Some(px), Some(py), Some(l)) = (x.take(), parse_f64(v), cur.as_mut()) {
                    l.push(Point::new(px, py));
                }
            }
            12 => x = parse_f64(v),
            22 => {
                if let (Some(px), Some(py)) = (x.take(), parse_f64(v)) {
                    text_pos = Some(Point::new(px, py));
                }
            }
            44 | 41 if height == 0.0 => height = parse_f64(v).unwrap_or(0.0),
            42 => angle = parse_f64(v).unwrap_or(0.0).to_degrees(),
            _ => {}
        }
    }
    // Leaders drawn without context data (older files) list their vertices
    // with plain codes.
    if lines.is_empty() {
        let pts = r.points(10);
        if pts.len() >= 2 {
            lines.push(pts);
        }
    }
    if lines.is_empty() && text.is_empty() {
        return None;
    }
    Some(DxfMLeader {
        lines,
        text,
        text_pos,
        height,
        angle_deg: angle,
        arrow,
    })
}

// ----- HATCH -----

fn hatch(r: &Rec<'_>) -> Option<DxfHatch> {
    let p = r.pairs;
    let mut i = 0;
    let mut pattern = String::new();
    let mut solid = false;
    while i < p.len() && p[i].0 != 91 {
        match p[i].0 {
            2 => pattern = p[i].1.trim().to_string(),
            70 => solid = parse_i(&p[i].1) == Some(1),
            _ => {}
        }
        i += 1;
    }
    let n_paths = p.get(i).and_then(|x| parse_i(&x.1)).unwrap_or(0).max(0) as usize;
    i += 1;
    let mut loops: Vec<HatchLoop> = Vec::new();
    for _ in 0..n_paths {
        while i < p.len() && p[i].0 != 92 {
            i += 1;
        }
        let Some(flags) = p.get(i).and_then(|x| parse_i(&x.1)) else {
            break;
        };
        i += 1;
        let pts = if flags & 2 != 0 {
            hatch_polyline_path(p, &mut i)
        } else {
            hatch_edge_path(p, &mut i)
        };
        // Source boundary objects (97 and 330 handles).
        if p.get(i).is_some_and(|x| x.0 == 97) {
            i += 1;
            while p.get(i).is_some_and(|x| x.0 == 330) {
                i += 1;
            }
        }
        if pts.len() >= 3 {
            loops.push(HatchLoop {
                points: pts,
                external: flags & (1 | 16) != 0,
            });
        }
    }
    if loops.is_empty() {
        return None;
    }
    if !loops.iter().any(|l| l.external) {
        let big = loops
            .iter()
            .enumerate()
            .max_by(|a, b| area(&a.1.points).total_cmp(&area(&b.1.points)))
            .map(|(k, _)| k)
            .unwrap_or(0);
        loops[big].external = true;
    }
    // Pattern data after the paths: 52 angle, 41 scale, 78 line count, then
    // the lines (53 angle, 43/44 base, 45/46 offset, 79 dashes).
    let (mut angle, mut scale, mut spacing) = (0.0, 1.0, None);
    let mut first_line_angle = None;
    let mut offset = (None, None);
    let mut seeds = false;
    for (code, v) in &p[i.min(p.len())..] {
        match *code {
            52 => angle = parse_f64(v).unwrap_or(0.0),
            41 => scale = parse_f64(v).unwrap_or(1.0),
            98 => seeds = true,
            53 if first_line_angle.is_none() && !seeds => first_line_angle = parse_f64(v),
            45 if first_line_angle.is_some() && offset.0.is_none() => offset.0 = parse_f64(v),
            46 if first_line_angle.is_some() && offset.1.is_none() => offset.1 = parse_f64(v),
            _ => {}
        }
    }
    if let (Some(a), (Some(ox), Some(oy))) = (first_line_angle, offset) {
        let t = a.to_radians();
        let d = (-ox * t.sin() + oy * t.cos()).abs() * scale;
        if d > 1e-9 {
            spacing = Some(d);
        }
    }
    let name_solid = pattern.eq_ignore_ascii_case("SOLID") || pattern.is_empty();
    Some(DxfHatch {
        loops,
        pattern,
        solid: solid || name_solid,
        angle_deg: angle,
        scale,
        spacing,
    })
}

fn area(pts: &[Point]) -> f64 {
    let n = pts.len();
    (0..n)
        .map(|i| pts[i].cross(pts[(i + 1) % n]))
        .sum::<f64>()
        .abs()
        * 0.5
}

fn hatch_polyline_path(p: &[Pair<'_>], i: &mut usize) -> Vec<Point> {
    let (mut has_bulge, mut closed, mut n) = (false, true, 0usize);
    while *i < p.len() && p[*i].0 != 93 {
        match p[*i].0 {
            72 => has_bulge = parse_i(&p[*i].1) != Some(0),
            73 => closed = parse_i(&p[*i].1) != Some(0),
            _ => {}
        }
        *i += 1;
    }
    if *i < p.len() {
        n = parse_i(&p[*i].1).unwrap_or(0).max(0) as usize;
        *i += 1;
    }
    let mut pts = Vec::with_capacity(n);
    let mut bulges = Vec::with_capacity(n);
    for _ in 0..n {
        if p.get(*i).map(|x| x.0) != Some(10) {
            break;
        }
        let x = parse_f64(&p[*i].1).unwrap_or(0.0);
        *i += 1;
        let y = match p.get(*i) {
            Some((20, v)) => {
                *i += 1;
                parse_f64(v).unwrap_or(0.0)
            }
            _ => 0.0,
        };
        let mut b = 0.0;
        if has_bulge {
            if let Some((42, v)) = p.get(*i) {
                b = parse_f64(v).unwrap_or(0.0);
                *i += 1;
            }
        }
        pts.push(Point::new(x, y));
        bulges.push(b);
    }
    sample_polyline(&pts, &bulges, closed)
}

/// Takes pairs while their code is in `allowed`.
fn take_codes<'a>(p: &'a [Pair<'a>], i: &mut usize, allowed: &[i32]) -> Vec<(i32, &'a str)> {
    let mut out = Vec::new();
    while let Some((c, v)) = p.get(*i) {
        if !allowed.contains(c) {
            break;
        }
        out.push((*c, v.as_ref()));
        *i += 1;
    }
    out
}

fn code_f(v: &[(i32, &str)], code: i32) -> Option<f64> {
    v.iter().find(|(c, _)| *c == code).and_then(|(_, s)| parse_f64(s))
}

fn hatch_edge_path(p: &[Pair<'_>], i: &mut usize) -> Vec<Point> {
    // 93: number of edges.
    while *i < p.len() && p[*i].0 != 93 {
        *i += 1;
    }
    let n = p.get(*i).and_then(|x| parse_i(&x.1)).unwrap_or(0).max(0) as usize;
    *i += 1;
    let mut chain: Vec<Point> = Vec::new();
    for _ in 0..n {
        if p.get(*i).map(|x| x.0) != Some(72) {
            break;
        }
        let kind = parse_i(&p[*i].1).unwrap_or(0);
        *i += 1;
        let pts: Vec<Point> = match kind {
            1 => {
                let v = take_codes(p, i, &[10, 20, 11, 21]);
                vec![
                    Point::new(code_f(&v, 10).unwrap_or(0.0), code_f(&v, 20).unwrap_or(0.0)),
                    Point::new(code_f(&v, 11).unwrap_or(0.0), code_f(&v, 21).unwrap_or(0.0)),
                ]
            }
            2 => {
                let v = take_codes(p, i, &[10, 20, 40, 50, 51, 73]);
                let c = Point::new(code_f(&v, 10).unwrap_or(0.0), code_f(&v, 20).unwrap_or(0.0));
                let rad = code_f(&v, 40).unwrap_or(0.0);
                let (a0, a1) = (code_f(&v, 50).unwrap_or(0.0), code_f(&v, 51).unwrap_or(360.0));
                let ccw = code_f(&v, 73).unwrap_or(1.0) != 0.0;
                let u = Point::new(rad, 0.0);
                let w = Point::new(0.0, rad);
                if ccw {
                    let (t0, t1) = ellipse_range(a0.to_radians(), a1.to_radians());
                    ellipse_points(c, u, w, t0, t1)
                } else {
                    let (t0, t1) = ellipse_range(a1.to_radians(), a0.to_radians());
                    let mut pts = ellipse_points(c, u, w, t0, t1);
                    pts.reverse();
                    pts
                }
            }
            3 => {
                let v = take_codes(p, i, &[10, 20, 11, 21, 40, 50, 51, 73]);
                let c = Point::new(code_f(&v, 10).unwrap_or(0.0), code_f(&v, 20).unwrap_or(0.0));
                let major = Point::new(code_f(&v, 11).unwrap_or(1.0), code_f(&v, 21).unwrap_or(0.0));
                let ratio = code_f(&v, 40).unwrap_or(1.0);
                let (a0, a1) = (code_f(&v, 50).unwrap_or(0.0), code_f(&v, 51).unwrap_or(360.0));
                let ccw = code_f(&v, 73).unwrap_or(1.0) != 0.0;
                let minor = major.perp().scale(ratio);
                if ccw {
                    let (t0, t1) = ellipse_range(a0.to_radians(), a1.to_radians());
                    ellipse_points(c, major, minor, t0, t1)
                } else {
                    let (t0, t1) = ellipse_range(a1.to_radians(), a0.to_radians());
                    let mut pts = ellipse_points(c, major, minor, t0, t1);
                    pts.reverse();
                    pts
                }
            }
            4 => hatch_spline_edge(p, i),
            _ => break,
        };
        append_edge(&mut chain, pts);
    }
    chain
}

fn hatch_spline_edge(p: &[Pair<'_>], i: &mut usize) -> Vec<Point> {
    let head = take_codes(p, i, &[94, 73, 74]);
    let degree = code_f(&head, 94).unwrap_or(3.0).max(1.0) as usize;
    let rational = code_f(&head, 73).unwrap_or(0.0) != 0.0;
    let mut knots = Vec::new();
    let mut control: Vec<Point> = Vec::new();
    let mut weights = Vec::new();
    let (mut n_knots, mut n_ctrl) = (0usize, 0usize);
    if p.get(*i).map(|x| x.0) == Some(95) {
        n_knots = parse_i(&p[*i].1).unwrap_or(0).max(0) as usize;
        *i += 1;
    }
    if p.get(*i).map(|x| x.0) == Some(96) {
        n_ctrl = parse_i(&p[*i].1).unwrap_or(0).max(0) as usize;
        *i += 1;
    }
    for _ in 0..n_knots {
        if let Some((40, v)) = p.get(*i) {
            knots.push(parse_f64(v).unwrap_or(0.0));
            *i += 1;
        }
    }
    for _ in 0..n_ctrl {
        let (Some((10, x)), Some((20, y))) = (p.get(*i), p.get(*i + 1)) else {
            break;
        };
        control.push(Point::new(parse_f64(x).unwrap_or(0.0), parse_f64(y).unwrap_or(0.0)));
        *i += 2;
        if rational {
            if let Some((42, w)) = p.get(*i) {
                weights.push(parse_f64(w).unwrap_or(1.0));
                *i += 1;
            }
        }
    }
    // Optional fit data: 97 count, 11/21 points, 12/22 and 13/23 tangents.
    if p.get(*i).map(|x| x.0) == Some(97) {
        let nf = parse_i(&p[*i].1).unwrap_or(0).max(0) as usize;
        *i += 1;
        for _ in 0..nf {
            take_codes(p, i, &[11, 21]);
        }
        take_codes(p, i, &[12, 22, 13, 23]);
    }
    let nurbs = Nurbs {
        degree,
        knots: &knots,
        weights: &weights,
        control: &control,
    };
    nurbs.sample(8)
}

fn append_edge(chain: &mut Vec<Point>, mut edge: Vec<Point>) {
    if edge.is_empty() {
        return;
    }
    if let Some(&last) = chain.last() {
        // Edges may run either way round the loop.
        if edge.first().is_some_and(|f| f.dist(last) > edge.last().map_or(f64::MAX, |l| l.dist(last))) {
            edge.reverse();
        }
        if edge[0].dist(last) < 1e-6 * (1.0 + last.length()) {
            edge.remove(0);
        }
    }
    chain.extend(edge);
    // A closed loop repeats its first point at the end.
    if chain.len() > 2 && chain[0].dist(chain[chain.len() - 1]) < 1e-9 * (1.0 + chain[0].length()) {
        chain.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_split_at_group_zero() {
        let pairs: Vec<Pair> = vec![
            (0, "SECTION".into()),
            (2, "ENTITIES".into()),
            (0, "LINE".into()),
            (8, "A".into()),
            (0, "EOF".into()),
            (0, "LINE".into()),
        ];
        let r = records(&pairs);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].kind, "SECTION");
        assert_eq!(r[1].get(8), Some("A"));
    }
}
