//! The line style library (CAD-82..CAD-86; manual pp. 230-233).
//!
//! A line style is a named, repeating run of components: a Dash (a stroke of
//! a given length), a Dot (a single point) or a Text (a word such as "GAS" or
//! "EX." set along the line). Every component is followed by a gap
//! (`spacing`). Lengths and heights are paper inches, so a style keeps its
//! look at any drawing scale: the stroker takes the scale as "plan inches per
//! paper inch" (`12 / inches-per-foot`).
//!
//! * [`LineStyleDef`] / [`LineStyleLibrary`]: the styles saved in the file
//!   (what Line Style Management lists) with new, copy, move, purge, delete
//!   and merge.
//! * [`catalog`]: the styles the Library Browser offers.
//! * [`stroke_path`]: the one stroker every renderer shares. It turns a
//!   polyline into [`Piece`]s (dash runs that follow corners, dots, text) and
//!   keeps the pattern's phase continuous round corners.
//! * [`parse_lin`]: File > Import > Import Line Styles (.lin).
//! * [`usage`]: the Used column (red plus, wrench, S).

use crate::geometry::Point;
use crate::layers::LineStyle;
use crate::model::Id;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What one component of a line style draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComponentKind {
    /// A stroke of `length`.
    Dash,
    /// A single point.
    Dot,
    /// A word set along the line.
    Text,
}

impl ComponentKind {
    pub fn label(self) -> &'static str {
        match self {
            ComponentKind::Dash => "Dash",
            ComponentKind::Dot => "Dot",
            ComponentKind::Text => "Text",
        }
    }
}

/// One component and the gap after it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LineComponent {
    pub kind: ComponentKind,
    /// Dash length, paper inches (ignored for a Dot and a Text).
    pub length: f64,
    /// The gap that follows, paper inches.
    pub spacing: f64,
    /// The text of a Text component.
    pub text: String,
    /// Text height, paper inches.
    pub height: f64,
    /// Font name of a Text component; empty is the layer's text style.
    pub font: String,
}

impl Default for LineComponent {
    fn default() -> Self {
        Self {
            kind: ComponentKind::Dash,
            length: 0.125,
            spacing: 0.0625,
            text: String::new(),
            height: 0.09,
            font: String::new(),
        }
    }
}

/// Characters are this wide relative to their height (the stroker cannot
/// measure a font; the renderer draws the word centred on the slot).
const TEXT_ADVANCE: f64 = 0.6;

impl LineComponent {
    pub fn dash(length: f64, spacing: f64) -> Self {
        Self {
            kind: ComponentKind::Dash,
            length,
            spacing,
            ..Self::default()
        }
    }

    pub fn dot(spacing: f64) -> Self {
        Self {
            kind: ComponentKind::Dot,
            length: 0.0,
            spacing,
            ..Self::default()
        }
    }

    pub fn text(text: &str, height: f64, spacing: f64) -> Self {
        Self {
            kind: ComponentKind::Text,
            length: 0.0,
            spacing,
            text: text.to_string(),
            height,
            ..Self::default()
        }
    }

    /// The length the component itself takes along the line, paper inches.
    pub fn advance(&self) -> f64 {
        match self.kind {
            ComponentKind::Dash => self.length.max(0.0),
            ComponentKind::Dot => 0.0,
            ComponentKind::Text => self.text.chars().count() as f64 * self.height * TEXT_ADVANCE,
        }
    }
}

/// A named line style. No components is a solid line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LineStyleDef {
    pub name: String,
    pub components: Vec<LineComponent>,
    /// A system style (the S icon): the built-ins the layers and objects use
    /// when nothing else is chosen.
    pub system: bool,
}

impl Default for LineStyleDef {
    fn default() -> Self {
        Self {
            name: "New Line Style".into(),
            components: vec![LineComponent::dash(0.125, 0.0625)],
            system: false,
        }
    }
}

impl LineStyleDef {
    pub fn new(name: &str, components: Vec<LineComponent>) -> Self {
        Self {
            name: name.to_string(),
            components,
            system: false,
        }
    }

    pub fn solid(name: &str) -> Self {
        Self::new(name, Vec::new())
    }

    pub fn is_solid(&self) -> bool {
        self.components.is_empty()
    }

    /// One repeat of the pattern, paper inches.
    pub fn period(&self) -> f64 {
        self.components
            .iter()
            .map(|c| c.advance() + c.spacing.max(0.0))
            .sum()
    }

    /// Where each component starts within one repeat, paper inches.
    pub fn offsets(&self) -> Vec<f64> {
        let mut at = 0.0;
        self.components
            .iter()
            .map(|c| {
                let o = at;
                at += c.advance() + c.spacing.max(0.0);
                o
            })
            .collect()
    }

    /// The repeat can be stroked: every part has a size and the whole is
    /// longer than a hair.
    pub fn strokable(&self) -> bool {
        self.period() > 1e-6
    }

    /// The component order as the dialog's list shows it ("Dash 1/8",
    /// "Dot", "Text GAS").
    pub fn component_labels(&self) -> Vec<String> {
        self.components
            .iter()
            .map(|c| match c.kind {
                ComponentKind::Dash => format!("Dash {:.3}", c.length),
                ComponentKind::Dot => "Dot".to_string(),
                ComponentKind::Text => format!("Text {}", c.text),
            })
            .collect()
    }
}

/// Name of the built-in style a layer or object `LineStyle` stands for.
pub fn enum_name(s: LineStyle) -> &'static str {
    match s {
        LineStyle::Solid => "Solid",
        LineStyle::Dashed => "Dashed",
        LineStyle::Dotted => "Dotted",
        LineStyle::DashDot => "Dash Dot",
    }
}

/// The four system styles every file carries (the `LineStyle` variants).
pub fn system_styles() -> Vec<LineStyleDef> {
    let mk = |name: &str, c: Vec<LineComponent>| LineStyleDef {
        name: name.to_string(),
        components: c,
        system: true,
    };
    vec![
        mk("Solid", Vec::new()),
        mk("Dashed", vec![LineComponent::dash(0.14, 0.07)]),
        mk("Dotted", vec![LineComponent::dot(0.05)]),
        mk(
            "Dash Dot",
            vec![LineComponent::dash(0.17, 0.05), LineComponent::dot(0.05)],
        ),
    ]
}

/// The styles of the Library Browser's Line Styles folder: the system four
/// and the usual drafting styles, including the ones with text.
pub fn catalog() -> Vec<LineStyleDef> {
    let mut v = system_styles();
    let mut add = |name: &str, c: Vec<LineComponent>| v.push(LineStyleDef::new(name, c));
    add("Long Dash", vec![LineComponent::dash(0.375, 0.09)]);
    add(
        "Center",
        vec![
            LineComponent::dash(0.5, 0.09),
            LineComponent::dash(0.09, 0.09),
        ],
    );
    add(
        "Phantom",
        vec![
            LineComponent::dash(0.5, 0.07),
            LineComponent::dash(0.07, 0.07),
            LineComponent::dash(0.07, 0.07),
        ],
    );
    add("Hidden", vec![LineComponent::dash(0.09, 0.05)]);
    add(
        "Dash Dot Dot",
        vec![
            LineComponent::dash(0.17, 0.05),
            LineComponent::dot(0.05),
            LineComponent::dot(0.05),
        ],
    );
    add(
        "Gas Line",
        vec![
            LineComponent::dash(0.4, 0.08),
            LineComponent::text("GAS", 0.09, 0.08),
        ],
    );
    add(
        "Existing",
        vec![
            LineComponent::dash(0.4, 0.08),
            LineComponent::text("EX.", 0.09, 0.08),
        ],
    );
    add(
        "Property Line",
        vec![
            LineComponent::dash(0.6, 0.08),
            LineComponent::dot(0.08),
            LineComponent::dash(0.6, 0.08),
            LineComponent::dot(0.08),
            LineComponent::dot(0.08),
        ],
    );
    v
}

/// The styles saved in the file, in the order Line Style Management shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LineStyleLibrary {
    pub styles: Vec<LineStyleDef>,
    /// Drop a style from the file when its last use goes away.
    pub auto_purge: bool,
}

impl Default for LineStyleLibrary {
    fn default() -> Self {
        Self {
            styles: system_styles(),
            auto_purge: false,
        }
    }
}

impl LineStyleLibrary {
    pub fn get(&self, name: &str) -> Option<&LineStyleDef> {
        self.styles.iter().find(|s| s.name == name)
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.styles.iter().position(|s| s.name == name)
    }

    /// `base`, or `base 2`, `base 3`... when taken.
    pub fn unique_name(&self, base: &str) -> String {
        if self.get(base).is_none() {
            return base.to_string();
        }
        (2..)
            .map(|n| format!("{base} {n}"))
            .find(|n| self.get(n).is_none())
            .unwrap_or_else(|| base.to_string())
    }

    /// Adds `def` under a free name; returns its index.
    pub fn add(&mut self, mut def: LineStyleDef) -> usize {
        def.name = self.unique_name(&def.name);
        def.system = false;
        self.styles.push(def);
        self.styles.len() - 1
    }

    /// New: a fresh dash style.
    pub fn add_new(&mut self) -> usize {
        self.add(LineStyleDef::default())
    }

    /// Copy: the same style named after the original with a number.
    pub fn copy(&mut self, index: usize) -> Option<usize> {
        let def = self.styles.get(index)?.clone();
        let name = self.unique_name(&def.name);
        let at = index + 1;
        self.styles.insert(
            at,
            LineStyleDef {
                name,
                system: false,
                ..def
            },
        );
        Some(at)
    }

    /// Makes sure `def` (a catalog style chosen with the Library button) is
    /// saved in the file; returns the name to refer to it by. A style of the
    /// same name and look is reused; the same name with another look is saved
    /// under a numbered name.
    pub fn ensure(&mut self, def: &LineStyleDef) -> String {
        if let Some(have) = self.get(&def.name) {
            if have.components == def.components {
                return have.name.clone();
            }
        }
        let i = self.add(def.clone());
        self.styles[i].name.clone()
    }

    /// Moves the styles at `rows` up one position (Move Up); returns where
    /// the selection is now.
    pub fn move_up(&mut self, rows: &[usize]) -> Vec<usize> {
        let mut rows: Vec<usize> = rows.to_vec();
        rows.sort_unstable();
        rows.dedup();
        if rows.first().is_none_or(|r| *r == 0) {
            return rows;
        }
        for r in &mut rows {
            if *r < self.styles.len() {
                self.styles.swap(*r, *r - 1);
                *r -= 1;
            }
        }
        rows
    }

    /// Moves the styles at `rows` down one position (Move Down).
    pub fn move_down(&mut self, rows: &[usize]) -> Vec<usize> {
        let mut rows: Vec<usize> = rows.to_vec();
        rows.sort_unstable();
        rows.dedup();
        if rows.last().is_none_or(|r| *r + 1 >= self.styles.len()) {
            return rows;
        }
        for r in rows.iter_mut().rev() {
            self.styles.swap(*r, *r + 1);
            *r += 1;
        }
        rows
    }

    /// Edits the components and name of style `index`; a system style keeps
    /// its name and its look (Chief's system styles are fixed).
    pub fn edit(&mut self, index: usize, def: LineStyleDef) -> Result<(), String> {
        let Some(old) = self.styles.get(index) else {
            return Err("No such line style".into());
        };
        if old.system {
            return Err("A system line style cannot be edited".into());
        }
        if def.name.trim().is_empty() {
            return Err("A line style needs a name".into());
        }
        if self
            .styles
            .iter()
            .enumerate()
            .any(|(i, s)| i != index && s.name == def.name)
        {
            return Err(format!("There is already a line style named {}", def.name));
        }
        self.styles[index] = LineStyleDef {
            system: false,
            ..def
        };
        Ok(())
    }

    /// Delete: removes the unused, non-system styles at `rows`. Returns the
    /// names removed.
    pub fn delete(&mut self, rows: &[usize], in_use: &dyn Fn(&str) -> bool) -> Vec<String> {
        let mut gone = Vec::new();
        let mut keep = Vec::new();
        for (i, s) in self.styles.drain(..).enumerate() {
            if rows.contains(&i) && !s.system && !in_use(&s.name) {
                gone.push(s.name);
            } else {
                keep.push(s);
            }
        }
        self.styles = keep;
        gone
    }

    /// Purge: removes every style that is not in use (the system ones stay).
    pub fn purge(&mut self, in_use: &dyn Fn(&str) -> bool) -> Vec<String> {
        let all: Vec<usize> = (0..self.styles.len()).collect();
        self.delete(&all, in_use)
    }

    /// Merge: keeps the topmost style at `rows` and drops the others;
    /// returns `(kept name, dropped names)` for the caller to repoint the
    /// instances of the dropped styles. A system style may only be the
    /// topmost selection, and only one system style may be in the selection.
    pub fn merge(&mut self, rows: &[usize]) -> Result<(String, Vec<String>), String> {
        let mut rows: Vec<usize> = rows
            .iter()
            .copied()
            .filter(|r| *r < self.styles.len())
            .collect();
        rows.sort_unstable();
        rows.dedup();
        if rows.len() < 2 {
            return Err("Select two or more line styles to merge".into());
        }
        if rows[1..].iter().any(|r| self.styles[*r].system) {
            return Err("A system line style can only be the topmost one of a merge".into());
        }
        let kept = self.styles[rows[0]].name.clone();
        let dropped: Vec<String> = rows[1..]
            .iter()
            .map(|r| self.styles[*r].name.clone())
            .collect();
        self.styles.retain(|s| !dropped.contains(&s.name));
        Ok((kept, dropped))
    }
}

// ---------------------------------------------------------------------------
// The stroker
// ---------------------------------------------------------------------------

/// One drawn part of a styled line. Positions are in the path's own units.
#[derive(Debug, Clone, PartialEq)]
pub enum Piece {
    /// A stroke along the path; it keeps the corners it passes (a dash that
    /// turns a corner is one run, so the corner is joined).
    Run(Vec<Point>),
    /// A dot.
    Dot(Point),
    /// A word centred at `at`, turned `angle` radians (kept upright).
    Text {
        at: Point,
        angle: f64,
        text: String,
        /// Text height in path units.
        height: f64,
        /// The slot the word takes along the line, path units.
        width: f64,
        font: String,
    },
}

/// Cap on pieces per path, so a hairline pattern on a long line cannot run
/// away.
pub const MAX_PIECES: usize = 60_000;

struct Path {
    pts: Vec<Point>,
    /// Cumulative length at each point.
    cum: Vec<f64>,
}

impl Path {
    fn new(pts: &[Point], closed: bool) -> Option<Path> {
        let mut v: Vec<Point> = Vec::with_capacity(pts.len() + 1);
        for p in pts {
            if v.last().is_none_or(|q| q.dist(*p) > 1e-9) {
                v.push(*p);
            }
        }
        if closed && v.len() > 2 {
            let first = v[0];
            if v.last().is_some_and(|l| l.dist(first) > 1e-9) {
                v.push(first);
            }
        }
        if v.len() < 2 {
            return None;
        }
        let mut cum = vec![0.0];
        for w in v.windows(2) {
            cum.push(cum[cum.len() - 1] + w[0].dist(w[1]));
        }
        Some(Path { pts: v, cum })
    }

    fn total(&self) -> f64 {
        self.cum[self.cum.len() - 1]
    }

    /// Index of the segment containing distance `s`.
    fn seg_at(&self, s: f64) -> usize {
        let i = self.cum.partition_point(|c| *c <= s);
        i.clamp(1, self.pts.len() - 1) - 1
    }

    fn point_at(&self, s: f64) -> Point {
        let i = self.seg_at(s);
        let len = self.cum[i + 1] - self.cum[i];
        let t = if len > 1e-12 {
            ((s - self.cum[i]) / len).clamp(0.0, 1.0)
        } else {
            0.0
        };
        Point::lerp(self.pts[i], self.pts[i + 1], t)
    }

    fn dir_at(&self, s: f64) -> Point {
        let i = self.seg_at(s);
        self.pts[i + 1].sub(self.pts[i]).normalized()
    }

    /// The stroke from `s0` to `s1` with the corners between.
    fn run(&self, s0: f64, s1: f64) -> Vec<Point> {
        let mut out = vec![self.point_at(s0)];
        let first = self.cum.partition_point(|c| *c <= s0 + 1e-12);
        for k in first..self.pts.len() {
            if self.cum[k] < s1 - 1e-12 {
                out.push(self.pts[k]);
            } else {
                break;
            }
        }
        out.push(self.point_at(s1));
        out
    }
}

/// Strokes `pts` (closed when `closed`) with `def`. `scale` is path units
/// per paper inch (plan inches per paper inch for a plan). A solid style
/// returns the whole path as one run.
pub fn stroke_path(def: &LineStyleDef, pts: &[Point], closed: bool, scale: f64) -> Vec<Piece> {
    let Some(path) = Path::new(pts, closed) else {
        return Vec::new();
    };
    if def.is_solid() || !def.strokable() || !(scale.is_finite() && scale > 0.0) {
        return vec![Piece::Run(path.pts.clone())];
    }
    let total = path.total();
    let mut out: Vec<Piece> = Vec::new();
    let mut s = 0.0;
    'outer: loop {
        for c in &def.components {
            if s > total + 1e-9 || out.len() >= MAX_PIECES {
                break 'outer;
            }
            match c.kind {
                ComponentKind::Dash => {
                    let len = c.length.max(0.0) * scale;
                    if len > 1e-9 {
                        if s < total - 1e-9 {
                            out.push(Piece::Run(path.run(s, (s + len).min(total))));
                        }
                    } else {
                        out.push(Piece::Dot(path.point_at(s.min(total))));
                    }
                    s += len;
                }
                ComponentKind::Dot => out.push(Piece::Dot(path.point_at(s.min(total)))),
                ComponentKind::Text => {
                    let width = c.advance() * scale;
                    if width > 1e-9 && s + width <= total + 1e-9 {
                        let mid = s + width / 2.0;
                        let d = path.dir_at(mid);
                        // Text reads left to right: a path heading left turns
                        // the word round (the angle stays within +-90 degrees).
                        let mut angle = d.angle();
                        if angle > std::f64::consts::FRAC_PI_2 + 1e-9 {
                            angle -= std::f64::consts::PI;
                        } else if angle <= -std::f64::consts::FRAC_PI_2 - 1e-9 {
                            angle += std::f64::consts::PI;
                        }
                        out.push(Piece::Text {
                            at: path.point_at(mid),
                            angle,
                            text: c.text.clone(),
                            height: c.height * scale,
                            width,
                            font: c.font.clone(),
                        });
                    }
                    s += width;
                }
            }
            s += c.spacing.max(0.0) * scale;
        }
    }
    out
}

/// The path (plan inches) of a CAD item that a line style follows, and
/// whether it is closed. Circles and arcs are faceted finely enough that a
/// dash follows the curve. Text has no path.
pub fn cad_item_path(item: &crate::cad::CadItem) -> Option<(Vec<Point>, bool)> {
    use crate::cad::CadItem;
    use std::f64::consts::TAU;
    match item {
        CadItem::Line { a, b } => Some((vec![*a, *b], false)),
        CadItem::Polyline { points, closed } => Some((points.clone(), *closed)),
        CadItem::Circle { center, radius } => Some((
            (0..96)
                .map(|i| {
                    let t = TAU * f64::from(i) / 96.0;
                    Point::new(center.x + radius * t.cos(), center.y + radius * t.sin())
                })
                .collect(),
            true,
        )),
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let sweep = (end_angle - start_angle).rem_euclid(TAU);
            Some((
                (0..=64)
                    .map(|i| {
                        let t = start_angle + sweep * f64::from(i) / 64.0;
                        Point::new(center.x + radius * t.cos(), center.y + radius * t.sin())
                    })
                    .collect(),
                false,
            ))
        }
        CadItem::Text { .. } => None,
    }
}

// ---------------------------------------------------------------------------
// Import (.lin)
// ---------------------------------------------------------------------------

/// What a `.lin` import found.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LinImport {
    pub styles: Vec<LineStyleDef>,
    /// Names of definitions that could not be read.
    pub skipped: Vec<String>,
}

fn lin_number(tok: &str) -> Option<f64> {
    tok.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Splits a pattern line at commas that are not inside `[...]` or quotes.
fn split_lin(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let (mut depth, mut quoted) = (0i32, false);
    for ch in line.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                cur.push(ch);
            }
            '[' if !quoted => {
                depth += 1;
                cur.push(ch);
            }
            ']' if !quoted => {
                depth -= 1;
                cur.push(ch);
            }
            ',' if !quoted && depth == 0 => out.push(std::mem::take(&mut cur)),
            _ => cur.push(ch),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

/// A complex element `["GAS",STANDARD,S=.1,R=0,X=-.1,Y=-.05]` as a Text
/// component (shape elements, which name a .shx shape, are not supported).
fn lin_text(tok: &str, unit: f64) -> Option<LineComponent> {
    let inner = tok.trim().strip_prefix('[')?.strip_suffix(']')?;
    let parts = split_lin(inner);
    let text = parts.first()?.trim();
    let text = text.strip_prefix('"')?.strip_suffix('"')?;
    let mut height = 0.1 * unit;
    for p in &parts[1..] {
        let p = p.trim();
        if let Some(v) = p.strip_prefix("S=").or_else(|| p.strip_prefix("s=")) {
            if let Some(v) = lin_number(v) {
                height = v.abs() * unit;
            }
        }
    }
    Some(LineComponent::text(text, height, 0.0))
}

/// Reads AutoCAD `.lin` (and `.dat`) text: `*NAME,description` followed by
/// `A,` and the pattern - a positive number is a dash, a negative a gap and
/// zero a dot; `["TEXT",STYLE,S=h,...]` is a text. `unit` is paper inches per
/// file unit (use `1.0` for inches, `1.0 / 25.4` for millimetres).
pub fn parse_lin(text: &str, unit: f64) -> LinImport {
    let unit = if unit.is_finite() && unit > 0.0 {
        unit
    } else {
        1.0
    };
    let mut out = LinImport::default();
    let mut current: Option<String> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if let Some(head) = line.strip_prefix('*') {
            let name = head.split(',').next().unwrap_or("").trim().to_string();
            current = Some(name);
            continue;
        }
        let Some(name) = current.take() else { continue };
        let toks = split_lin(line);
        // The first element is the alignment code (A).
        let mut comps: Vec<LineComponent> = Vec::new();
        let mut ok = !name.is_empty();
        for tok in toks.iter().skip(1) {
            let t = tok.trim();
            if t.starts_with('[') {
                match lin_text(t, unit) {
                    Some(c) => comps.push(c),
                    None => ok = false,
                }
            } else if let Some(v) = lin_number(t) {
                if v > 1e-12 {
                    comps.push(LineComponent::dash(v * unit, 0.0));
                } else if v < -1e-12 {
                    match comps.last_mut() {
                        Some(last) => last.spacing += -v * unit,
                        None => comps.push(LineComponent::dot(-v * unit)),
                    }
                } else {
                    comps.push(LineComponent::dot(0.0));
                }
            } else {
                ok = false;
            }
        }
        if ok && !comps.is_empty() {
            out.styles.push(LineStyleDef::new(&name, comps));
        } else {
            out.skipped.push(name);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Assignments and the Used column
// ---------------------------------------------------------------------------

/// Where a library line style is applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineTarget {
    /// A CAD object (by id).
    Cad(Id),
    /// Every object of a layer that has no style of its own.
    Layer(String),
    /// A defaults dialog or tool ("CAD Line", "Property Line"...).
    Default(String),
}

/// The Used column of Line Style Management.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UseMark {
    /// Objects (and layers) the style is assigned to: the red plus.
    pub objects: usize,
    /// Defaults dialogs that name it: the wrench.
    pub defaults: Vec<String>,
    /// A system default: the grey S.
    pub system: bool,
}

impl UseMark {
    /// Anything keeps the style from being deleted.
    pub fn in_use(&self) -> bool {
        self.objects > 0 || !self.defaults.is_empty() || self.system
    }

    /// The tool tip over the icon.
    pub fn tip(&self) -> String {
        let mut parts = Vec::new();
        if self.objects > 0 {
            parts.push(format!(
                "Assigned to {} object(s) or layer(s)",
                self.objects
            ));
        }
        if !self.defaults.is_empty() {
            parts.push(format!("Used in defaults: {}", self.defaults.join(", ")));
        }
        if self.system {
            parts.push("System default line style".to_string());
        }
        parts.join("; ")
    }
}

/// Uses of every line style name in `project`, by name. `ids` is the set of
/// CAD ids that still exist.
pub fn usage(
    assigned: &[(LineTarget, String)],
    enum_uses: &[LineStyle],
    cad_ids: &dyn Fn(Id) -> bool,
    library: &LineStyleLibrary,
) -> BTreeMap<String, UseMark> {
    let mut m: BTreeMap<String, UseMark> = BTreeMap::new();
    for s in &library.styles {
        let e = m.entry(s.name.clone()).or_default();
        e.system = s.system;
    }
    for (t, name) in assigned {
        let e = m.entry(name.clone()).or_default();
        match t {
            LineTarget::Cad(id) => {
                if cad_ids(*id) {
                    e.objects += 1;
                }
            }
            LineTarget::Layer(_) => e.objects += 1,
            LineTarget::Default(d) => e.defaults.push(d.clone()),
        }
    }
    for s in enum_uses {
        m.entry(enum_name(*s).to_string()).or_default().objects += 1;
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn run_len(pc: &Piece) -> f64 {
        match pc {
            Piece::Run(v) => v.windows(2).map(|w| w[0].dist(w[1])).sum(),
            _ => 0.0,
        }
    }

    #[test]
    fn a_dash_style_alternates_dashes_and_gaps_along_a_line() {
        let def = LineStyleDef::new("D", vec![LineComponent::dash(1.0, 0.5)]);
        // scale 1: 10 path units, period 1.5 -> dashes start at 0, 1.5, 3...
        let v = stroke_path(&def, &[p(0.0, 0.0), p(10.0, 0.0)], false, 1.0);
        assert_eq!(v.len(), 7);
        let Piece::Run(first) = &v[0] else { panic!() };
        assert_eq!(first, &vec![p(0.0, 0.0), p(1.0, 0.0)]);
        let Piece::Run(second) = &v[1] else { panic!() };
        assert!((second[0].x - 1.5).abs() < 1e-9);
        // The last dash is cut at the end of the line.
        let Piece::Run(last) = &v[6] else { panic!() };
        assert!((last[0].x - 9.0).abs() < 1e-9 && (last[1].x - 10.0).abs() < 1e-9);
        // Scaling doubles every length.
        let v2 = stroke_path(&def, &[p(0.0, 0.0), p(10.0, 0.0)], false, 2.0);
        assert_eq!(v2.len(), 4);
        assert!((run_len(&v2[0]) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn a_dash_keeps_its_phase_and_joins_round_a_corner() {
        let def = LineStyleDef::new("D", vec![LineComponent::dash(2.0, 1.0)]);
        // 4 along x then up: the second dash (3..5) turns the corner at 4.
        let v = stroke_path(&def, &[p(0.0, 0.0), p(4.0, 0.0), p(4.0, 6.0)], false, 1.0);
        let Piece::Run(turn) = &v[1] else { panic!() };
        assert_eq!(turn, &vec![p(3.0, 0.0), p(4.0, 0.0), p(4.0, 1.0)]);
        // Total drawn length = 2 per 3 along the 10 long path: 3 whole
        // dashes (9) and the end at 9..10 = 1 short.
        let drawn: f64 = v.iter().map(run_len).sum();
        assert!((drawn - 7.0).abs() < 1e-9, "{drawn}");
        // The phase is continuous: gaps are exactly 1 apart round the corner.
        let Piece::Run(third) = &v[2] else { panic!() };
        assert!((third[0].y - 2.0).abs() < 1e-9);
    }

    #[test]
    fn a_closed_shape_strokes_its_closing_edge_and_dots_stay_on_the_path() {
        let def = LineStyleDef::new(
            "DD",
            vec![LineComponent::dash(1.0, 0.5), LineComponent::dot(0.5)],
        );
        let sq = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)];
        let v = stroke_path(&def, &sq, true, 1.0);
        // Perimeter 16, period 2: a dash then a dot every 2 along the loop.
        let dots: Vec<&Piece> = v.iter().filter(|x| matches!(x, Piece::Dot(_))).collect();
        assert_eq!(dots.len(), 8);
        // The last dash (14..15) runs on the closing edge x = 0.
        let Piece::Run(last) = v.iter().rev().find(|x| matches!(x, Piece::Run(_))).unwrap() else {
            panic!()
        };
        assert!((last[0].x).abs() < 1e-9 && (last[0].y - 2.0).abs() < 1e-9);
        assert!((last[1].y - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_text_component_sets_the_word_along_the_line_and_keeps_it_upright() {
        let def = LineStyleDef::new(
            "GAS",
            vec![
                LineComponent::dash(1.0, 0.2),
                LineComponent::text("GAS", 0.5, 0.2),
            ],
        );
        // Text width = 3 * 0.5 * 0.6 = 0.9; period = 1.2 + 1.1 = 2.3.
        assert!((def.period() - 2.3).abs() < 1e-9);
        let v = stroke_path(&def, &[p(0.0, 0.0), p(5.0, 0.0)], false, 1.0);
        let texts: Vec<&Piece> = v
            .iter()
            .filter(|x| matches!(x, Piece::Text { .. }))
            .collect();
        assert_eq!(texts.len(), 2);
        let Piece::Text {
            at,
            angle,
            text,
            width,
            ..
        } = texts[0]
        else {
            panic!()
        };
        assert_eq!(text, "GAS");
        assert!((at.x - (1.2 + 0.45)).abs() < 1e-9 && angle.abs() < 1e-9);
        assert!((width - 0.9).abs() < 1e-9);
        // Drawn right to left the word turns over to read left to right.
        let back = stroke_path(&def, &[p(5.0, 0.0), p(0.0, 0.0)], false, 1.0);
        let Piece::Text { angle, .. } = back
            .iter()
            .find(|x| matches!(x, Piece::Text { .. }))
            .unwrap()
        else {
            panic!()
        };
        assert!(angle.abs() < 1e-9, "upright, not upside down: {angle}");
        // The spacing between two words is the period.
        let Piece::Text { at: at2, .. } = texts[1] else {
            panic!()
        };
        assert!((at2.x - at.x - 2.3).abs() < 1e-9);
    }

    #[test]
    fn solid_and_degenerate_styles_stroke_the_whole_path() {
        let solid = LineStyleDef::solid("Solid");
        let v = stroke_path(&solid, &[p(0.0, 0.0), p(3.0, 0.0), p(3.0, 3.0)], false, 1.0);
        assert_eq!(
            v,
            vec![Piece::Run(vec![p(0.0, 0.0), p(3.0, 0.0), p(3.0, 3.0)])]
        );
        let zero = LineStyleDef::new("Z", vec![LineComponent::dash(0.0, 0.0)]);
        assert_eq!(
            stroke_path(&zero, &[p(0.0, 0.0), p(1.0, 0.0)], false, 1.0).len(),
            1
        );
        assert!(stroke_path(&solid, &[p(0.0, 0.0)], false, 1.0).is_empty());
    }

    #[test]
    fn the_library_creates_copies_moves_merges_purges_and_protects_system_styles() {
        let mut lib = LineStyleLibrary::default();
        assert_eq!(lib.styles.len(), 4);
        let a = lib.add_new();
        assert_eq!(lib.styles[a].name, "New Line Style");
        let b = lib.copy(a).unwrap();
        assert_eq!(lib.styles[b].name, "New Line Style 2");
        assert_eq!(b, a + 1);
        let sel = lib.move_up(&[b]);
        assert_eq!(sel, vec![a]);
        assert_eq!(lib.styles[a].name, "New Line Style 2");
        let sel = lib.move_down(&sel);
        assert_eq!(sel, vec![b]);
        // System styles cannot be edited or deleted.
        assert!(lib.edit(1, LineStyleDef::default()).is_err());
        assert!(
            lib.delete(&[0, 1], &|_| false).is_empty(),
            "system styles stay"
        );
        // Merge keeps the topmost; a system style below the top is refused.
        assert!(lib.merge(&[0, 1]).is_err());
        let (kept, dropped) = lib.merge(&[a, b]).unwrap();
        assert_eq!(kept, "New Line Style");
        assert_eq!(dropped, vec!["New Line Style 2".to_string()]);
        assert!(lib.get("New Line Style 2").is_none());
        // Purge drops only the unused ones.
        let n = lib.add(LineStyleDef::new("Mine", vec![LineComponent::dot(0.1)]));
        assert_eq!(lib.styles[n].name, "Mine");
        let gone = lib.purge(&|name| name == "Mine");
        assert_eq!(gone, vec!["New Line Style".to_string()]);
        assert!(lib.get("Mine").is_some());
        // A catalog style is saved once; a changed look gets a new name.
        let gas = catalog()
            .into_iter()
            .find(|s| s.name == "Gas Line")
            .unwrap();
        let name = lib.ensure(&gas);
        assert_eq!(lib.ensure(&gas), name);
        let mut other = gas.clone();
        other.components.pop();
        assert_eq!(lib.ensure(&other), "Gas Line 2");
    }

    #[test]
    fn the_lin_parser_reads_dashes_gaps_dots_and_text() {
        let text = "\
;; a comment
*DASHED,Dashed __ __ __
A,.5,-.25
*DASHDOT,Dash dot
A,1.0,-0.25,0,-0.25
*GAS_LINE,Gas line ----GAS----GAS----
A,.5,-.2,[\"GAS\",STANDARD,S=.1,R=0.0,X=-0.1,Y=-.05],-.25
*BROKEN,bad
A,.5,oops
";
        let imp = parse_lin(text, 1.0);
        assert_eq!(imp.styles.len(), 3);
        assert_eq!(imp.skipped, vec!["BROKEN".to_string()]);
        assert_eq!(imp.styles[0].name, "DASHED");
        assert_eq!(
            imp.styles[0].components,
            vec![LineComponent::dash(0.5, 0.25)]
        );
        let dd = &imp.styles[1].components;
        assert_eq!(dd.len(), 2);
        assert_eq!(dd[0], LineComponent::dash(1.0, 0.25));
        assert_eq!(dd[1], LineComponent::dot(0.25));
        let gas = &imp.styles[2].components;
        assert_eq!(gas.len(), 2);
        assert_eq!(gas[0], LineComponent::dash(0.5, 0.2));
        assert_eq!(gas[1].kind, ComponentKind::Text);
        assert_eq!(gas[1].text, "GAS");
        assert!((gas[1].height - 0.1).abs() < 1e-12);
        assert!((gas[1].spacing - 0.25).abs() < 1e-12);
        // Millimetre files scale to paper inches.
        let mm = parse_lin("*D,d\nA,25.4,-12.7", 1.0 / 25.4);
        assert!((mm.styles[0].components[0].length - 1.0).abs() < 1e-9);
    }

    #[test]
    fn cad_items_give_the_path_a_style_follows() {
        use crate::cad::CadItem;
        let (ring, closed) = cad_item_path(&CadItem::Circle {
            center: p(0.0, 0.0),
            radius: 10.0,
        })
        .unwrap();
        assert!(closed && ring.len() == 96);
        assert!((ring[0].x - 10.0).abs() < 1e-9);
        let (a, closed) = cad_item_path(&CadItem::Arc {
            center: p(0.0, 0.0),
            radius: 10.0,
            start_angle: 0.0,
            end_angle: std::f64::consts::FRAC_PI_2,
        })
        .unwrap();
        assert!(!closed && a.len() == 65);
        assert!((a[64].y - 10.0).abs() < 1e-9);
        assert!(cad_item_path(&CadItem::Text {
            pos: p(0.0, 0.0),
            text: "x".into(),
            height: 1.0,
            angle: 0.0
        })
        .is_none());
    }

    #[test]
    fn the_used_column_marks_objects_defaults_and_system_styles() {
        let lib = LineStyleLibrary::default();
        let assigned = vec![
            (LineTarget::Cad(5), "Solid".to_string()),
            (LineTarget::Cad(6), "Gone".to_string()),
            (LineTarget::Default("CAD Line".into()), "Dashed".to_string()),
            (LineTarget::Layer("CAD".into()), "Dotted".to_string()),
        ];
        let u = usage(&assigned, &[LineStyle::Dashed], &|id| id == 5, &lib);
        assert_eq!(u["Solid"].objects, 1);
        assert!(u["Solid"].system && u["Solid"].in_use());
        assert_eq!(u["Dashed"].defaults, vec!["CAD Line".to_string()]);
        assert_eq!(u["Dashed"].objects, 1);
        assert_eq!(u["Dotted"].objects, 1);
        assert_eq!(u["Gone"].objects, 0);
        assert!(!u["Gone"].in_use());
        assert!(u["Dashed"].tip().contains("CAD Line"));
    }
}
