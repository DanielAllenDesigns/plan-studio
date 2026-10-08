//! Tolerant ASCII DXF reader.
//!
//! The file is read as group-code / value line pairs (CRLF or LF, padded or
//! unpadded codes). Supported content: `HEADER` (`$INSUNITS`, `$EXTMIN`,
//! `$EXTMAX`), `TABLES` ▸ `LAYER`, `BLOCKS` and `ENTITIES` (LINE, LWPOLYLINE,
//! POLYLINE/VERTEX/SEQEND, CIRCLE, ARC, TEXT, MTEXT, INSERT). Anything else is
//! skipped and tallied in [`DxfDrawing::skipped`].

use crate::ImportError;
use plan_core::cad::TEXT_WIDTH_FACTOR;
use plan_core::Point;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Drawing units from `$INSUNITS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DxfUnits {
    /// No units declared (`0`, absent, or a unit this crate does not know).
    #[default]
    Unitless,
    Inches,
    Feet,
    Millimeters,
    Centimeters,
    Meters,
}

impl DxfUnits {
    /// Map an `$INSUNITS` code; unknown codes are treated as unitless.
    pub fn from_code(code: i32) -> DxfUnits {
        match code {
            1 => DxfUnits::Inches,
            2 => DxfUnits::Feet,
            4 => DxfUnits::Millimeters,
            5 => DxfUnits::Centimeters,
            6 => DxfUnits::Meters,
            _ => DxfUnits::Unitless,
        }
    }
}

/// A layer from the `LAYER` table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DxfLayer {
    pub name: String,
    /// AutoCAD Color Index (always non-negative; the sign is folded into `visible`).
    pub color_aci: i32,
    /// `false` when the layer is off (negative colour) or frozen.
    pub visible: bool,
}

/// One drawing entity, in raw drawing units. Angles are degrees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DxfEntity {
    Line {
        a: Point,
        b: Point,
        layer: String,
    },
    /// LWPOLYLINE or POLYLINE. `bulges[i]` belongs to the segment from
    /// vertex `i` to the next vertex (the first one, when `closed` and `i` is
    /// last); it always has the same length as `points`.
    Polyline {
        points: Vec<Point>,
        closed: bool,
        layer: String,
        bulges: Vec<f64>,
    },
    Circle {
        center: Point,
        radius: f64,
        layer: String,
    },
    /// Counter-clockwise arc from `start_deg` to `end_deg`.
    Arc {
        center: Point,
        radius: f64,
        start_deg: f64,
        end_deg: f64,
        layer: String,
    },
    /// TEXT or MTEXT; `pos` is the bottom-left anchor.
    Text {
        pos: Point,
        text: String,
        height: f64,
        angle_deg: f64,
        layer: String,
    },
    Insert {
        block: String,
        pos: Point,
        scale: (f64, f64),
        rotation_deg: f64,
        layer: String,
    },
}

/// A block definition from the `BLOCKS` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DxfBlock {
    pub name: String,
    /// Block base point; INSERT positions refer to it.
    pub base: Point,
    pub entities: Vec<DxfEntity>,
}

/// A parsed DXF file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DxfDrawing {
    pub layers: Vec<DxfLayer>,
    /// Model-space entities, with INSERTs still unexpanded.
    pub entities: Vec<DxfEntity>,
    pub units: DxfUnits,
    /// `($EXTMIN, $EXTMAX)` when the header carries sane values.
    pub extents: Option<(Point, Point)>,
    /// Block definitions keyed by upper-cased name.
    pub blocks: BTreeMap<String, DxfBlock>,
    /// Entity types that were not imported, with how many of each.
    pub skipped: Vec<(String, usize)>,
}

impl DxfDrawing {
    /// Look up a block by name (DXF block names are case-insensitive).
    pub fn block(&self, name: &str) -> Option<&DxfBlock> {
        self.blocks.get(&name.to_uppercase())
    }

    /// Model-space entities with every INSERT replaced by the (transformed)
    /// contents of its block, recursively. INSERTs of unknown blocks are
    /// dropped. Block entities on layer `0` take the INSERT's layer.
    pub fn explode_inserts(&self) -> Vec<DxfEntity> {
        let mut out = Vec::new();
        self.expand(&self.entities, 0, &mut out);
        out
    }

    fn expand(&self, entities: &[DxfEntity], depth: usize, out: &mut Vec<DxfEntity>) {
        for e in entities {
            let DxfEntity::Insert {
                block,
                pos,
                scale,
                rotation_deg,
                layer,
            } = e
            else {
                out.push(e.clone());
                continue;
            };
            if depth >= MAX_INSERT_DEPTH {
                continue;
            }
            let Some(def) = self.block(block) else {
                continue;
            };
            let xf = Transform::new(def.base, *pos, *scale, *rotation_deg);
            let inner: Vec<DxfEntity> = def
                .entities
                .iter()
                .map(|ent| xf.entity(ent, layer))
                .collect();
            self.expand(&inner, depth + 1, out);
        }
    }
}

/// Guard against self-referencing blocks.
const MAX_INSERT_DEPTH: usize = 16;

/// Block-to-world transform of an INSERT: `p' = pos + R * S * (p - base)`.
struct Transform {
    base: Point,
    pos: Point,
    sx: f64,
    sy: f64,
    rot_deg: f64,
    cos: f64,
    sin: f64,
}

impl Transform {
    fn new(base: Point, pos: Point, scale: (f64, f64), rot_deg: f64) -> Self {
        let r = rot_deg.to_radians();
        Self {
            base,
            pos,
            sx: scale.0,
            sy: scale.1,
            rot_deg,
            cos: r.cos(),
            sin: r.sin(),
        }
    }

    fn rotate(&self, v: Point) -> Point {
        Point::new(
            v.x * self.cos - v.y * self.sin,
            v.x * self.sin + v.y * self.cos,
        )
    }

    fn point(&self, p: Point) -> Point {
        let d = p.sub(self.base);
        self.pos
            .add(self.rotate(Point::new(d.x * self.sx, d.y * self.sy)))
    }

    /// Angle (degrees) of a direction after scale and rotation.
    fn angle(&self, deg: f64) -> f64 {
        let r = deg.to_radians();
        self.rotate(Point::new(r.cos() * self.sx, r.sin() * self.sy))
            .angle()
            .to_degrees()
    }

    fn mirrored(&self) -> bool {
        self.sx * self.sy < 0.0
    }

    fn radius(&self, r: f64) -> f64 {
        r * (self.sx.abs() + self.sy.abs()) * 0.5
    }

    /// Transform `e`; entities on layer `0` move to `insert_layer`.
    fn entity(&self, e: &DxfEntity, insert_layer: &str) -> DxfEntity {
        let lay = |l: &String| {
            if l == "0" {
                insert_layer.to_string()
            } else {
                l.clone()
            }
        };
        match e {
            DxfEntity::Line { a, b, layer } => DxfEntity::Line {
                a: self.point(*a),
                b: self.point(*b),
                layer: lay(layer),
            },
            DxfEntity::Polyline {
                points,
                closed,
                layer,
                bulges,
            } => DxfEntity::Polyline {
                points: points.iter().map(|p| self.point(*p)).collect(),
                closed: *closed,
                layer: lay(layer),
                bulges: bulges
                    .iter()
                    .map(|b| if self.mirrored() { -b } else { *b })
                    .collect(),
            },
            DxfEntity::Circle {
                center,
                radius,
                layer,
            } => DxfEntity::Circle {
                center: self.point(*center),
                radius: self.radius(*radius),
                layer: lay(layer),
            },
            DxfEntity::Arc {
                center,
                radius,
                start_deg,
                end_deg,
                layer,
            } => {
                let (s, e) = (self.angle(*start_deg), self.angle(*end_deg));
                // A mirror reverses the sweep direction, so swap the ends.
                let (start_deg, end_deg) = if self.mirrored() { (e, s) } else { (s, e) };
                DxfEntity::Arc {
                    center: self.point(*center),
                    radius: self.radius(*radius),
                    start_deg,
                    end_deg,
                    layer: lay(layer),
                }
            }
            DxfEntity::Text {
                pos,
                text,
                height,
                angle_deg,
                layer,
            } => DxfEntity::Text {
                pos: self.point(*pos),
                text: text.clone(),
                height: height * self.sy.abs(),
                angle_deg: angle_deg + self.rot_deg,
                layer: lay(layer),
            },
            DxfEntity::Insert {
                block,
                pos,
                scale,
                rotation_deg,
                layer,
            } => DxfEntity::Insert {
                block: block.clone(),
                pos: self.point(*pos),
                scale: (scale.0 * self.sx, scale.1 * self.sy),
                rotation_deg: rotation_deg + self.rot_deg,
                layer: lay(layer),
            },
        }
    }
}

/// One group-code / value pair.
type Pair<'a> = (i32, &'a str);

/// A `0`-group record: entity type plus the pairs that follow it.
struct Record<'a> {
    kind: &'a str,
    pairs: Vec<Pair<'a>>,
}

impl<'a> Record<'a> {
    fn get(&self, code: i32) -> Option<&'a str> {
        self.pairs.iter().find(|(c, _)| *c == code).map(|(_, v)| *v)
    }

    fn f(&self, code: i32) -> Option<f64> {
        self.get(code).and_then(parse_f64)
    }

    fn int(&self, code: i32) -> Option<i32> {
        self.get(code).and_then(|v| v.trim().parse().ok())
    }

    fn layer(&self) -> String {
        self.get(8).map_or("0", str::trim).to_string()
    }

    /// Point from group codes `base` (x) and `base + 10` (y).
    fn point(&self, base: i32) -> Point {
        Point::new(
            self.f(base).unwrap_or(0.0),
            self.f(base + 10).unwrap_or(0.0),
        )
    }
}

fn parse_f64(s: &str) -> Option<f64> {
    s.trim().parse().ok()
}

/// Split text into group-code / value pairs, skipping comments (999) and
/// resynchronising past lines that are not group codes.
fn tokenize(text: &str) -> Vec<Pair<'_>> {
    let mut lines = text.lines();
    let mut out = Vec::new();
    while let Some(code_line) = lines.next() {
        let Ok(code) = code_line.trim().parse::<i32>() else {
            continue;
        };
        let Some(value) = lines.next() else {
            break;
        };
        if code != 999 {
            out.push((code, value));
        }
    }
    out
}

/// Group pairs into records at each `0` code; pairs before the first `0` are
/// dropped and `EOF` ends the file.
fn records<'a>(pairs: &[Pair<'a>]) -> Vec<Record<'a>> {
    let mut out: Vec<Record<'a>> = Vec::new();
    for &(code, value) in pairs {
        if code == 0 {
            let kind = value.trim();
            if kind == "EOF" {
                break;
            }
            out.push(Record {
                kind,
                pairs: Vec::new(),
            });
        } else if let Some(r) = out.last_mut() {
            r.pairs.push((code, value));
        }
    }
    out
}

/// Parse an ASCII DXF document.
///
/// # Errors
/// [`ImportError::BinaryDxf`] for binary DXF and [`ImportError::NotDxf`] when
/// no `SECTION` can be found.
pub fn parse_dxf(text: &str) -> Result<DxfDrawing, ImportError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.starts_with("AutoCAD Binary DXF") {
        return Err(ImportError::BinaryDxf);
    }
    let recs = records(&tokenize(text));
    if !recs.iter().any(|r| r.kind == "SECTION") {
        return Err(ImportError::NotDxf);
    }

    let mut drawing = DxfDrawing::default();
    let mut skipped: BTreeMap<String, usize> = BTreeMap::new();
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
            "HEADER" => read_header(&recs[i], &mut drawing),
            "TABLES" => read_layers(body, &mut drawing),
            "BLOCKS" => read_blocks(body, &mut drawing, &mut skipped),
            "ENTITIES" => drawing.entities = convert_entities(body, &mut skipped),
            _ => {}
        }
        // Resume at a SECTION that followed a missing ENDSEC; otherwise skip ENDSEC.
        i = if recs.get(end).is_some_and(|r| r.kind == "SECTION") {
            end
        } else {
            end + 1
        };
    }
    drawing.skipped = skipped.into_iter().collect();
    Ok(drawing)
}

fn read_header(section: &Record<'_>, drawing: &mut DxfDrawing) {
    let mut var = "";
    let (mut min, mut max) = ((None, None), (None, None));
    for &(code, value) in &section.pairs {
        if code == 9 {
            var = value.trim();
            continue;
        }
        match (var, code) {
            ("$INSUNITS", 70) => {
                if let Ok(n) = value.trim().parse() {
                    drawing.units = DxfUnits::from_code(n);
                }
            }
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
            drawing.extents = Some((Point::new(x0, y0), Point::new(x1, y1)));
        }
    }
}

fn read_layers(body: &[Record<'_>], drawing: &mut DxfDrawing) {
    for r in body.iter().filter(|r| r.kind == "LAYER") {
        let Some(name) = r.get(2) else {
            continue;
        };
        let color = r.int(62).unwrap_or(7);
        let frozen = r.int(70).unwrap_or(0) & 1 != 0;
        drawing.layers.push(DxfLayer {
            name: name.trim().to_string(),
            color_aci: color.abs(),
            visible: color >= 0 && !frozen,
        });
    }
}

fn read_blocks(
    body: &[Record<'_>],
    drawing: &mut DxfDrawing,
    skipped: &mut BTreeMap<String, usize>,
) {
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
        let name = head.get(2).map_or("", str::trim).to_string();
        let block = DxfBlock {
            base: head.point(10),
            entities: convert_entities(&body[i + 1..end], skipped),
            name,
        };
        drawing.blocks.insert(block.name.to_uppercase(), block);
        i = end;
    }
}

/// Convert a run of entity records. Unknown types are counted in `skipped`.
fn convert_entities(recs: &[Record<'_>], skipped: &mut BTreeMap<String, usize>) -> Vec<DxfEntity> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < recs.len() {
        let r = &recs[i];
        i += 1;
        match r.kind {
            "LINE" => out.push(DxfEntity::Line {
                a: r.point(10),
                b: r.point(11),
                layer: r.layer(),
            }),
            "LWPOLYLINE" => match lwpolyline(r) {
                Some(e) => out.push(e),
                None => *skipped.entry(r.kind.to_string()).or_default() += 1,
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
                    Some(e) => out.push(e),
                    None => *skipped.entry(r.kind.to_string()).or_default() += 1,
                }
            }
            "CIRCLE" => out.push(DxfEntity::Circle {
                center: r.point(10),
                radius: r.f(40).unwrap_or(0.0),
                layer: r.layer(),
            }),
            "ARC" => out.push(DxfEntity::Arc {
                center: r.point(10),
                radius: r.f(40).unwrap_or(0.0),
                start_deg: r.f(50).unwrap_or(0.0),
                end_deg: r.f(51).unwrap_or(360.0),
                layer: r.layer(),
            }),
            "TEXT" => out.push(text_entity(r)),
            "MTEXT" => out.push(mtext_entity(r)),
            "INSERT" => out.push(DxfEntity::Insert {
                block: r.get(2).map_or("", str::trim).to_string(),
                pos: r.point(10),
                scale: (r.f(41).unwrap_or(1.0), r.f(42).unwrap_or(1.0)),
                rotation_deg: r.f(50).unwrap_or(0.0),
                layer: r.layer(),
            }),
            // Stray terminators (e.g. after INSERT attributes) are not content.
            "SEQEND" | "ENDBLK" | "VERTEX" => {}
            other => *skipped.entry(other.to_string()).or_default() += 1,
        }
    }
    out
}

fn lwpolyline(r: &Record<'_>) -> Option<DxfEntity> {
    let mut points: Vec<Point> = Vec::new();
    let mut bulges: Vec<f64> = Vec::new();
    for &(code, value) in &r.pairs {
        let v = parse_f64(value);
        match (code, v) {
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
    Some(DxfEntity::Polyline {
        points,
        closed: r.int(70).unwrap_or(0) & 1 != 0,
        layer: r.layer(),
        bulges,
    })
}

fn polyline(head: &Record<'_>, vertices: &[Record<'_>]) -> Option<DxfEntity> {
    let flags = head.int(70).unwrap_or(0);
    // 3D meshes (16) and polyface meshes (64) are not outlines.
    if flags & (16 | 64) != 0 {
        return None;
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
        return None;
    }
    Some(DxfEntity::Polyline {
        points,
        closed: flags & 1 != 0,
        layer: head.layer(),
        bulges,
    })
}

fn text_entity(r: &Record<'_>) -> DxfEntity {
    let height = r.f(40).unwrap_or(0.0);
    let angle_deg = r.f(50).unwrap_or(0.0);
    let text = clean_text(r.get(1).unwrap_or(""));
    let (h_align, v_align) = (r.int(72).unwrap_or(0), r.int(73).unwrap_or(0));
    // Aligned text is positioned by the second point (11/21). Convert it back
    // to the bottom-left anchor using a rough text width.
    let aligned = matches!(h_align, 1 | 2 | 4) || matches!(v_align, 2 | 3);
    let pos = if aligned && r.get(11).is_some() {
        let width = text.chars().count() as f64 * height * TEXT_WIDTH_FACTOR;
        let dx = match h_align {
            1 | 4 => width * 0.5,
            2 => width,
            _ => 0.0,
        };
        let dy = match (h_align, v_align) {
            (_, 2) | (4, _) => height * 0.5,
            (_, 3) => height,
            _ => 0.0,
        };
        let a = angle_deg.to_radians();
        let (dx, dy) = (-dx, -dy);
        r.point(11).add(Point::new(
            dx * a.cos() - dy * a.sin(),
            dx * a.sin() + dy * a.cos(),
        ))
    } else {
        r.point(10)
    };
    DxfEntity::Text {
        pos,
        text,
        height,
        angle_deg,
        layer: r.layer(),
    }
}

fn mtext_entity(r: &Record<'_>) -> DxfEntity {
    // Long text is split: any number of group-3 chunks, then the final group 1.
    let mut raw = String::new();
    for &(code, value) in &r.pairs {
        if code == 3 {
            raw.push_str(value);
        }
    }
    raw.push_str(r.get(1).unwrap_or(""));
    // The direction vector (11/21) overrides the rotation angle when present.
    let angle_deg = match (r.f(11), r.f(21)) {
        (Some(x), Some(y)) if x != 0.0 || y != 0.0 => y.atan2(x).to_degrees(),
        _ => r.f(50).unwrap_or(0.0),
    };
    DxfEntity::Text {
        pos: r.point(10),
        text: clean_text(&strip_mtext(&raw)),
        height: r.f(40).unwrap_or(0.0),
        angle_deg,
        layer: r.layer(),
    }
}

/// Replace AutoCAD `%%` control sequences with their characters.
fn clean_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' || chars.peek() != Some(&'%') {
            out.push(c);
            continue;
        }
        chars.next();
        match chars.next() {
            Some('d' | 'D') => out.push('°'),
            Some('p' | 'P') => out.push('±'),
            Some('c' | 'C') => out.push('Ø'),
            Some('%') => out.push('%'),
            // Underline / overline / strike toggles carry no text.
            Some('u' | 'U' | 'o' | 'O' | 'k' | 'K') => {}
            Some(other) => {
                out.push_str("%%");
                out.push(other);
            }
            None => out.push_str("%%"),
        }
    }
    out
}

/// Strip basic MTEXT formatting: `\P` becomes a newline, font/colour/height
/// switches (`\f...;` and friends) and grouping braces are removed.
pub(crate) fn strip_mtext(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' | '}' => {}
            '\\' => match chars.next() {
                Some('P' | 'p') => out.push('\n'),
                Some('~') => out.push(' '),
                Some('\\') => out.push('\\'),
                Some('{') => out.push('{'),
                Some('}') => out.push('}'),
                // Switches terminated by ';' (font, height, colour, width, ...).
                Some('f' | 'F' | 'H' | 'C' | 'c' | 'Q' | 'T' | 'W' | 'A') => {
                    for n in chars.by_ref() {
                        if n == ';' {
                            break;
                        }
                    }
                }
                // Stacked fractions: keep "a/b" and drop the terminator.
                Some('S') => {
                    for n in chars.by_ref() {
                        match n {
                            ';' => break,
                            '^' | '#' => out.push('/'),
                            _ => out.push(n),
                        }
                    }
                }
                // Underline / overline / strike toggles.
                Some('L' | 'l' | 'O' | 'o' | 'K' | 'k') => {}
                Some(other) => out.push(other),
                None => {}
            },
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::{CadItem, Project, WallKind};

    const MINIMAL: &str = "\
  0\r\nSECTION\r\n  2\r\nHEADER\r\n  9\r\n$INSUNITS\r\n 70\r\n     4\r\n\
  9\r\n$EXTMIN\r\n 10\r\n0.0\r\n 20\r\n0.0\r\n 30\r\n0.0\r\n\
  9\r\n$EXTMAX\r\n 10\r\n5000.0\r\n 20\r\n3000.0\r\n 30\r\n0.0\r\n  0\r\nENDSEC\r\n\
  0\r\nSECTION\r\n  2\r\nTABLES\r\n  0\r\nTABLE\r\n  2\r\nLAYER\r\n 70\r\n 2\r\n\
  0\r\nLAYER\r\n  2\r\nA-WALL\r\n 70\r\n 0\r\n 62\r\n 1\r\n  6\r\nCONTINUOUS\r\n\
  0\r\nLAYER\r\n  2\r\nA-HIDDEN\r\n 70\r\n 0\r\n 62\r\n -3\r\n  6\r\nCONTINUOUS\r\n\
  0\r\nENDTAB\r\n  0\r\nENDSEC\r\n\
  0\r\nSECTION\r\n  2\r\nBLOCKS\r\n  0\r\nBLOCK\r\n  8\r\n0\r\n  2\r\nCHAIR\r\n 10\r\n0.0\r\n 20\r\n0.0\r\n\
  0\r\nLINE\r\n  8\r\n0\r\n 10\r\n0.0\r\n 20\r\n0.0\r\n 11\r\n10.0\r\n 21\r\n0.0\r\n\
  0\r\nENDBLK\r\n  0\r\nENDSEC\r\n\
  0\r\nSECTION\r\n  2\r\nENTITIES\r\n\
  0\r\nLINE\r\n  8\r\nA-WALL\r\n 10\r\n0.0\r\n 20\r\n0.0\r\n 11\r\n1000.0\r\n 21\r\n0.0\r\n\
  0\r\nLWPOLYLINE\r\n  8\r\nA-WALL\r\n 90\r\n3\r\n 70\r\n1\r\n 10\r\n0.0\r\n 20\r\n0.0\r\n\
 42\r\n1.0\r\n 10\r\n100.0\r\n 20\r\n0.0\r\n 10\r\n100.0\r\n 20\r\n100.0\r\n\
  0\r\nCIRCLE\r\n  8\r\n0\r\n 10\r\n50.0\r\n 20\r\n50.0\r\n 40\r\n25.0\r\n\
  0\r\nARC\r\n  8\r\n0\r\n 10\r\n0.0\r\n 20\r\n0.0\r\n 40\r\n30.0\r\n 50\r\n0.0\r\n 51\r\n90.0\r\n\
  0\r\nTEXT\r\n  8\r\n0\r\n 10\r\n5.0\r\n 20\r\n6.0\r\n 40\r\n2.5\r\n  1\r\nKitchen\r\n\
  0\r\nMTEXT\r\n  8\r\n0\r\n 10\r\n7.0\r\n 20\r\n8.0\r\n 40\r\n3.0\r\n  1\r\n{\\fArial|b0;Line one}\\PLine two\r\n\
  0\r\nINSERT\r\n  8\r\nFURN\r\n  2\r\nchair\r\n 10\r\n200.0\r\n 20\r\n300.0\r\n 41\r\n2.0\r\n 42\r\n2.0\r\n 50\r\n90.0\r\n\
  0\r\nHATCH\r\n  8\r\n0\r\n  0\r\nHATCH\r\n  8\r\n0\r\n  0\r\nSPLINE\r\n  8\r\n0\r\n\
  0\r\nENDSEC\r\n  0\r\nEOF\r\n";

    #[test]
    fn minimal_drawing_parses() {
        let d = parse_dxf(MINIMAL).unwrap();
        assert_eq!(d.units, DxfUnits::Millimeters);
        assert_eq!(
            d.extents,
            Some((Point::new(0.0, 0.0), Point::new(5000.0, 3000.0)))
        );
        assert_eq!(d.layers.len(), 2);
        assert_eq!(d.layers[0].name, "A-WALL");
        assert_eq!(d.layers[0].color_aci, 1);
        assert!(d.layers[0].visible);
        assert_eq!(d.layers[1].color_aci, 3);
        assert!(!d.layers[1].visible);
        assert_eq!(d.entities.len(), 7);
        assert_eq!(
            d.skipped,
            vec![("HATCH".to_string(), 2), ("SPLINE".to_string(), 1)]
        );

        match &d.entities[1] {
            DxfEntity::Polyline {
                points,
                closed,
                bulges,
                layer,
            } => {
                assert_eq!(points.len(), 3);
                assert!(*closed);
                assert_eq!(bulges, &vec![1.0, 0.0, 0.0]);
                assert_eq!(layer, "A-WALL");
            }
            other => panic!("expected polyline, got {other:?}"),
        }
        match &d.entities[3] {
            DxfEntity::Arc {
                radius,
                start_deg,
                end_deg,
                ..
            } => assert_eq!((*radius, *start_deg, *end_deg), (30.0, 0.0, 90.0)),
            other => panic!("expected arc, got {other:?}"),
        }
        match &d.entities[5] {
            DxfEntity::Text { text, height, .. } => {
                assert_eq!(text, "Line one\nLine two");
                assert_eq!(*height, 3.0);
            }
            other => panic!("expected text, got {other:?}"),
        }
    }

    #[test]
    fn insert_explodes_with_scale_rotation_and_layer() {
        let d = parse_dxf(MINIMAL).unwrap();
        assert!(d.block("Chair").is_some());
        let flat = d.explode_inserts();
        // 7 entities, the INSERT replaced by the block's single LINE.
        assert_eq!(flat.len(), 7);
        assert!(!flat.iter().any(|e| matches!(e, DxfEntity::Insert { .. })));
        let DxfEntity::Line { a, b, layer } = flat.last().unwrap() else {
            panic!("exploded insert should end with a line");
        };
        // (0,0)-(10,0) scaled x2, rotated 90 deg, moved to (200,300).
        assert!(a.dist(Point::new(200.0, 300.0)) < 1e-9);
        assert!(b.dist(Point::new(200.0, 320.0)) < 1e-9);
        assert_eq!(layer, "FURN");
    }

    #[test]
    fn tolerates_lf_unpadded_codes_comments_and_bom() {
        let src = "\u{feff}999\ncomment\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nX\n10\n1\n20\n2\n11\n3\n21\n4\n0\nENDSEC\n0\nEOF\n";
        let d = parse_dxf(src).unwrap();
        assert_eq!(d.entities.len(), 1);
        assert_eq!(d.units, DxfUnits::Unitless);
    }

    #[test]
    fn tolerates_padded_codes_and_crlf() {
        let src = [
            "  0", "SECTION", "  2", "ENTITIES", "  0", "CIRCLE", "  8", "Round", " 10", "1.5",
            " 20", "2.5", " 40", "3.0", "  0", "ENDSEC", "  0", "EOF", "",
        ]
        .join("\r\n");
        let d = parse_dxf(&src).unwrap();
        assert_eq!(
            d.entities,
            vec![DxfEntity::Circle {
                center: Point::new(1.5, 2.5),
                radius: 3.0,
                layer: "Round".into()
            }]
        );
    }

    #[test]
    fn old_style_polyline_with_vertices() {
        let src = "0\nSECTION\n2\nENTITIES\n0\nPOLYLINE\n8\nP\n66\n1\n70\n1\n\
0\nVERTEX\n8\nP\n10\n0\n20\n0\n0\nVERTEX\n8\nP\n10\n10\n20\n0\n42\n0.5\n\
0\nVERTEX\n8\nP\n10\n10\n20\n10\n0\nSEQEND\n8\nP\n0\nENDSEC\n0\nEOF\n";
        let d = parse_dxf(src).unwrap();
        match &d.entities[..] {
            [DxfEntity::Polyline {
                points,
                closed,
                bulges,
                ..
            }] => {
                assert_eq!(points.len(), 3);
                assert!(*closed);
                assert_eq!(bulges, &vec![0.0, 0.5, 0.0]);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn rejects_non_dxf() {
        assert_eq!(parse_dxf("hello world"), Err(ImportError::NotDxf));
        assert_eq!(
            parse_dxf("AutoCAD Binary DXF\r\n"),
            Err(ImportError::BinaryDxf)
        );
    }

    #[test]
    fn round_trips_plan_core_writer_output() {
        let mut p = Project::new("rt");
        p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        p.add_wall(
            0,
            Point::new(120.0, 0.0),
            Point::new(120.0, 96.0),
            6.5,
            96.0,
            WallKind::Exterior,
        );
        p.add_cad(
            0,
            "Notes",
            CadItem::Line {
                a: Point::new(1.0, 2.0),
                b: Point::new(3.0, 4.0),
            },
        );
        p.add_cad(
            0,
            "CAD, Default",
            CadItem::Circle {
                center: Point::new(50.0, 50.0),
                radius: 5.0,
            },
        );
        p.layers.set_display("Doors", false);

        let dxf = plan_core::write_dxf(&p, 0, &[]);
        let d = parse_dxf(&dxf).unwrap();

        assert_eq!(d.units, DxfUnits::Inches);
        // "0" + every project layer + the extra "Notes" CAD layer.
        assert_eq!(d.layers.len(), 1 + p.layers.layers.len() + 1);
        let doors = d.layers.iter().find(|l| l.name == "Doors").unwrap();
        assert!(!doors.visible);
        let walls = d.layers.iter().find(|l| l.name == "Walls, Normal").unwrap();
        assert!(walls.visible);
        assert!(d.layers.iter().any(|l| l.name == "Notes"));

        let polylines = d
            .entities
            .iter()
            .filter(|e| matches!(e, DxfEntity::Polyline { closed: true, .. }))
            .count();
        assert_eq!(polylines, 2);
        assert!(d.entities.iter().any(
            |e| matches!(e, DxfEntity::Line { layer, a, .. } if layer == "Notes" && a.dist(Point::new(1.0, 2.0)) < 1e-6)
        ));
        assert!(d.entities.iter().any(
            |e| matches!(e, DxfEntity::Circle { radius, .. } if (*radius - 5.0).abs() < 1e-6)
        ));
        assert!(d.skipped.is_empty());
    }

    #[test]
    fn mtext_formatting_is_stripped() {
        assert_eq!(
            strip_mtext("{\\fArial|b0;Hi}\\Pthere\\~now"),
            "Hi\nthere now"
        );
        assert_eq!(strip_mtext("\\H2.5;Big \\C1;red"), "Big red");
        assert_eq!(clean_text("45%%d %%p1"), "45° ±1");
    }
}
