//! DXF export options (L-44, L-45): what File > Export > DXF asks before it
//! writes.
//!
//! [`build`] takes the plain R12 text [`super::dxf`] wrote for each floor
//! (the plan, roofs, framing and schedule tables) and writes one file from
//! them with the options applied:
//!
//! * **Units**: inches, feet, millimetres, centimetres or metres; every
//!   length and coordinate is converted and `$INSUNITS` says which.
//! * **Layer names**: Chief's own, the AIA national CAD standard names
//!   (A-WALL, A-DOOR, A-GLAZ, ...), or a custom map; with several floors in
//!   one file each floor's layers can carry the floor's name.
//! * **Line weights**: the LAYER table gets the plotted weight of each layer
//!   (group 370, hundredths of a millimetre, the DXF convention).
//! * **Text**: written as TEXT entities, or drawn as lines (a single-stroke
//!   font), for programs that lack the fonts.
//! * **Floors**: any set of floors; in 3D every floor sits at its elevation.
//! * **2D or 3D**: 3D adds the model as 3DFACE entities on layers named after
//!   the materials.

use super::dxf::fmt_num;
use crate::geometry::Point;
use crate::layers::LayerSet;
use std::collections::BTreeMap;

/// The unit the file is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DxfUnits {
    #[default]
    Inches,
    Feet,
    Millimeters,
    Centimeters,
    Meters,
}

impl DxfUnits {
    pub const ALL: [DxfUnits; 5] = [
        DxfUnits::Inches,
        DxfUnits::Feet,
        DxfUnits::Millimeters,
        DxfUnits::Centimeters,
        DxfUnits::Meters,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DxfUnits::Inches => "Inches",
            DxfUnits::Feet => "Feet",
            DxfUnits::Millimeters => "Millimeters",
            DxfUnits::Centimeters => "Centimeters",
            DxfUnits::Meters => "Meters",
        }
    }

    /// Drawing units per inch of plan.
    pub fn per_inch(self) -> f64 {
        match self {
            DxfUnits::Inches => 1.0,
            DxfUnits::Feet => 1.0 / 12.0,
            DxfUnits::Millimeters => 25.4,
            DxfUnits::Centimeters => 2.54,
            DxfUnits::Meters => 0.0254,
        }
    }

    /// The `$INSUNITS` code.
    pub fn insunits(self) -> i32 {
        match self {
            DxfUnits::Inches => 1,
            DxfUnits::Feet => 2,
            DxfUnits::Millimeters => 4,
            DxfUnits::Centimeters => 5,
            DxfUnits::Meters => 6,
        }
    }
}

/// How layers are named in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayerNaming {
    /// Chief's names ("Walls, Normal").
    #[default]
    Chief,
    /// The AIA CAD layer standard ("A-WALL").
    Aia,
}

/// How text is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextMode {
    #[default]
    Text,
    /// Drawn with lines.
    Lines,
}

/// The export options.
#[derive(Debug, Clone, PartialEq)]
pub struct DxfOptions {
    pub units: DxfUnits,
    pub naming: LayerNaming,
    /// Renames applied after `naming`: `(layer in the plan, name in the file)`.
    pub layer_map: Vec<(String, String)>,
    /// Put the plotted weight of every layer in the LAYER table.
    pub line_weights: bool,
    pub text: TextMode,
    /// With more than one floor, put the floor's name in front of its layers.
    pub floor_layers: bool,
    /// Every floor at its elevation, and the model as 3DFACEs.
    pub three_d: bool,
}

impl Default for DxfOptions {
    fn default() -> Self {
        Self {
            units: DxfUnits::Inches,
            naming: LayerNaming::Chief,
            layer_map: Vec::new(),
            line_weights: false,
            text: TextMode::Text,
            floor_layers: true,
            three_d: false,
        }
    }
}

/// The R12 text of one floor, with what the file needs to place it.
#[derive(Debug, Clone, PartialEq)]
pub struct FloorDxf {
    pub name: String,
    /// Finished-floor elevation, inches.
    pub elevation: f64,
    pub text: String,
}

/// A mesh of the 3D model in plan coordinates (inches, Z up).
#[derive(Debug, Clone, PartialEq)]
pub struct SolidDxf {
    /// The layer name: the material.
    pub layer: String,
    pub triangles: Vec<[[f64; 3]; 3]>,
}

/// The AIA layer for a Chief layer, by name.
pub fn aia_layer(name: &str) -> String {
    let n = name.trim();
    let lower = n.to_lowercase();
    let table: [(&str, &str); 22] = [
        ("walls", "A-WALL"),
        ("doors", "A-DOOR"),
        ("windows", "A-GLAZ"),
        ("dimensions", "A-ANNO-DIMS"),
        ("room labels", "A-AREA-IDEN"),
        ("rooms", "A-AREA"),
        ("roof", "A-ROOF"),
        ("framing", "S-FRAM"),
        ("cabinets", "A-FLOR-CASE"),
        ("stairs", "A-FLOR-STRS"),
        ("electrical", "E-POWR"),
        ("lights", "E-LITE"),
        ("plumbing", "P-FIXT"),
        ("foundation", "S-FNDN"),
        ("site", "C-SITE"),
        ("terrain", "C-TOPO"),
        ("text", "A-ANNO-TEXT"),
        ("notes", "A-ANNO-NOTE"),
        ("schedule", "A-ANNO-SCHD"),
        ("cad", "A-ANNO-NPLT"),
        ("deck", "A-FLOR-DECK"),
        ("symbols", "A-FLOR-FIXT"),
    ];
    for (prefix, aia) in table {
        if lower.starts_with(prefix) {
            return aia.to_string();
        }
    }
    n.to_string()
}

/// A layer name the DXF format accepts.
fn clean_layer(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | '/' | '\\' | '"' | ':' | ';' | '?' | '*' | '|' | '=' | '`' => '-',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let s = s.trim().to_string();
    if s.is_empty() {
        "0".to_string()
    } else {
        s
    }
}

impl DxfOptions {
    /// The name `layer` of floor `floor_name` has in the file.
    pub fn layer_name(&self, layer: &str, floor_name: &str, floors: usize) -> String {
        let mapped = self
            .layer_map
            .iter()
            .find(|(from, _)| from == layer)
            .map(|(_, to)| to.clone())
            .unwrap_or_else(|| match self.naming {
                LayerNaming::Chief => layer.to_string(),
                LayerNaming::Aia => aia_layer(layer),
            });
        let base = if mapped == "0" || floors < 2 || !self.floor_layers {
            mapped
        } else {
            format!("{floor_name} - {mapped}")
        };
        clean_layer(&base)
    }
}

type Pair = (i32, String);

fn parse(text: &str) -> Vec<Pair> {
    let lines: Vec<&str> = text.lines().collect();
    lines
        .chunks(2)
        .filter(|c| c.len() == 2)
        .filter_map(|c| {
            Some((
                c[0].trim().parse::<i32>().ok()?,
                c[1].trim_end().to_string(),
            ))
        })
        .collect()
}

/// The LAYER table records and the entity records of a DXF.
#[derive(Default)]
struct Parts {
    layers: Vec<Vec<Pair>>,
    entities: Vec<Vec<Pair>>,
}

fn split(pairs: &[Pair]) -> Parts {
    let mut parts = Parts::default();
    let mut section = String::new();
    let mut table = String::new();
    let mut current: Vec<Pair> = Vec::new();
    let flush = |current: &mut Vec<Pair>, section: &str, table: &str, parts: &mut Parts| {
        if current.is_empty() {
            return;
        }
        let rec = std::mem::take(current);
        match (section, table, rec[0].1.as_str()) {
            ("ENTITIES", _, _) => parts.entities.push(rec),
            ("TABLES", "LAYER", "LAYER") => parts.layers.push(rec),
            _ => {}
        }
    };
    let mut i = 0;
    while i < pairs.len() {
        let (code, value) = &pairs[i];
        if *code == 0 {
            flush(&mut current, &section, &table, &mut parts);
            match value.as_str() {
                "SECTION" => {
                    section = pairs
                        .get(i + 1)
                        .filter(|p| p.0 == 2)
                        .map(|p| p.1.clone())
                        .unwrap_or_default();
                    i += 2;
                    continue;
                }
                "ENDSEC" => {
                    section.clear();
                    table.clear();
                }
                "TABLE" => {
                    table = pairs
                        .get(i + 1)
                        .filter(|p| p.0 == 2)
                        .map(|p| p.1.clone())
                        .unwrap_or_default();
                    i += 2;
                    continue;
                }
                "ENDTAB" => table.clear(),
                _ => current.push(pairs[i].clone()),
            }
        } else if !current.is_empty() {
            current.push(pairs[i].clone());
        }
        i += 1;
    }
    flush(&mut current, &section, &table, &mut parts);
    parts
}

fn value_of(rec: &[Pair], code: i32) -> Option<&str> {
    rec.iter().find(|p| p.0 == code).map(|p| p.1.as_str())
}

fn num_of(rec: &[Pair], code: i32) -> f64 {
    value_of(rec, code)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0.0)
}

/// Writes the DXF of `floors` (and the model `solids` in 3D) with `opts`.
/// `layers` supplies the colour and weight of the layers.
pub fn build(
    floors: &[FloorDxf],
    solids: &[SolidDxf],
    layers: &LayerSet,
    opts: &DxfOptions,
) -> String {
    let k = opts.units.per_inch();
    let n_floors = floors.len();
    let mut out: Vec<Pair> = Vec::new();
    let mut layer_records: BTreeMap<String, (i32, u32, bool)> = BTreeMap::new();
    let mut entities: Vec<Pair> = Vec::new();

    for fl in floors {
        let parts = split(&parse(&fl.text));
        let elevation = if opts.three_d { fl.elevation } else { 0.0 };
        for rec in &parts.layers {
            let orig = value_of(rec, 2).unwrap_or("0").to_string();
            let name = opts.layer_name(&orig, &fl.name, n_floors);
            let color = value_of(rec, 62)
                .and_then(|v| v.trim().parse::<i32>().ok())
                .unwrap_or(7);
            let weight = layers.get(&orig).map_or(0, |l| l.line_weight);
            layer_records
                .entry(name)
                .or_insert((color.abs().max(1), weight, color < 0));
        }
        for rec in &parts.entities {
            let kind = rec[0].1.clone();
            let layer_orig = value_of(rec, 8).unwrap_or("0").to_string();
            let layer = opts.layer_name(&layer_orig, &fl.name, n_floors);
            layer_records.entry(layer.clone()).or_insert((7, 0, false));
            if kind == "TEXT" && opts.text == TextMode::Lines {
                for (a, b) in text_lines(rec) {
                    let line = [
                        (0, "LINE".to_string()),
                        (8, layer.clone()),
                        (10, fmt_num((a.x) * k)),
                        (20, fmt_num((a.y) * k)),
                        (30, fmt_num(elevation * k)),
                        (11, fmt_num((b.x) * k)),
                        (21, fmt_num((b.y) * k)),
                        (31, fmt_num(elevation * k)),
                    ];
                    entities.extend(line);
                }
                continue;
            }
            for (code, value) in rec {
                let scaled = match *code {
                    8 => layer.clone(),
                    10..=13 | 20..=23 => scale(value, k),
                    30..=33 => {
                        let z = value.trim().parse::<f64>().unwrap_or(0.0) + elevation;
                        fmt_num(z * k)
                    }
                    40 if matches!(kind.as_str(), "ARC" | "CIRCLE" | "TEXT") => scale(value, k),
                    _ => value.clone(),
                };
                entities.push((*code, scaled));
            }
        }
    }

    // The model in 3D: one 3DFACE per triangle.
    if opts.three_d {
        for s in solids {
            let layer = clean_layer(&s.layer);
            layer_records.entry(layer.clone()).or_insert((8, 0, false));
            for t in &s.triangles {
                entities.push((0, "3DFACE".to_string()));
                entities.push((8, layer.clone()));
                // The fourth corner repeats the third: a triangle.
                for (i, v) in [t[0], t[1], t[2], t[2]].iter().enumerate() {
                    entities.push((10 + i as i32, fmt_num(v[0] * k)));
                    entities.push((20 + i as i32, fmt_num(v[1] * k)));
                    entities.push((30 + i as i32, fmt_num(v[2] * k)));
                }
            }
        }
    }

    // HEADER
    out.push((0, "SECTION".into()));
    out.push((2, "HEADER".into()));
    out.push((9, "$ACADVER".into()));
    out.push((1, "AC1009".into()));
    out.push((9, "$INSUNITS".into()));
    out.push((70, opts.units.insunits().to_string()));
    out.push((0, "ENDSEC".into()));
    // TABLES
    out.push((0, "SECTION".into()));
    out.push((2, "TABLES".into()));
    for (c, v) in [
        (0, "TABLE"),
        (2, "LTYPE"),
        (70, "1"),
        (0, "LTYPE"),
        (2, "CONTINUOUS"),
        (70, "0"),
        (3, "Solid line"),
        (72, "65"),
        (73, "0"),
        (40, "0.0000"),
        (0, "ENDTAB"),
        (0, "TABLE"),
        (2, "LAYER"),
    ] {
        out.push((c, v.to_string()));
    }
    out.push((70, (layer_records.len() + 1).to_string()));
    let write_layer = |out: &mut Vec<Pair>, name: &str, color: i32, weight: u32| {
        out.push((0, "LAYER".into()));
        out.push((2, name.to_string()));
        out.push((70, "0".into()));
        out.push((62, color.to_string()));
        out.push((6, "CONTINUOUS".into()));
        if opts.line_weights && weight > 0 {
            out.push((370, weight.to_string()));
        }
    };
    write_layer(&mut out, "0", 7, 0);
    for (name, (color, weight, off)) in &layer_records {
        if name != "0" {
            write_layer(&mut out, name, if *off { -*color } else { *color }, *weight);
        }
    }
    out.push((0, "ENDTAB".into()));
    out.push((0, "ENDSEC".into()));
    // ENTITIES
    out.push((0, "SECTION".into()));
    out.push((2, "ENTITIES".into()));
    out.extend(entities);
    out.push((0, "ENDSEC".into()));
    out.push((0, "EOF".into()));

    let mut text = String::new();
    for (c, v) in out {
        text.push_str(&format!("{c}\n{v}\n"));
    }
    text
}

fn scale(value: &str, k: f64) -> String {
    match value.trim().parse::<f64>() {
        Ok(v) => fmt_num(v * k),
        Err(_) => value.to_string(),
    }
}

// ----- text as lines -----

/// Height of a glyph cell, in glyph units (a glyph is 3 wide).
const GLYPH_H: f64 = 6.0;
/// Advance of one character as a fraction of the text height (the factor
/// the plan uses for a rough text width).
const ADVANCE: f64 = crate::cad::TEXT_WIDTH_FACTOR;

type Stroke = &'static [(f64, f64)];

/// The strokes of `c` (letters are drawn upper case).
fn glyph(c: char) -> Option<Vec<Stroke>> {
    const A: Stroke = &[(0.0, 0.0), (0.0, 4.0), (1.5, 6.0), (3.0, 4.0), (3.0, 0.0)];
    const A2: Stroke = &[(0.0, 3.0), (3.0, 3.0)];
    const B1: Stroke = &[
        (0.0, 0.0),
        (0.0, 6.0),
        (2.0, 6.0),
        (3.0, 5.0),
        (3.0, 4.0),
        (2.0, 3.0),
        (0.0, 3.0),
    ];
    const B2: Stroke = &[(2.0, 3.0), (3.0, 2.0), (3.0, 1.0), (2.0, 0.0), (0.0, 0.0)];
    const C: Stroke = &[
        (3.0, 5.0),
        (2.0, 6.0),
        (1.0, 6.0),
        (0.0, 5.0),
        (0.0, 1.0),
        (1.0, 0.0),
        (2.0, 0.0),
        (3.0, 1.0),
    ];
    const D: Stroke = &[
        (0.0, 0.0),
        (0.0, 6.0),
        (2.0, 6.0),
        (3.0, 5.0),
        (3.0, 1.0),
        (2.0, 0.0),
        (0.0, 0.0),
    ];
    const E1: Stroke = &[(3.0, 6.0), (0.0, 6.0), (0.0, 0.0), (3.0, 0.0)];
    const E2: Stroke = &[(0.0, 3.0), (2.0, 3.0)];
    const F1: Stroke = &[(3.0, 6.0), (0.0, 6.0), (0.0, 0.0)];
    const G: Stroke = &[
        (3.0, 5.0),
        (2.0, 6.0),
        (1.0, 6.0),
        (0.0, 5.0),
        (0.0, 1.0),
        (1.0, 0.0),
        (2.0, 0.0),
        (3.0, 1.0),
        (3.0, 3.0),
        (1.5, 3.0),
    ];
    const H1: Stroke = &[(0.0, 0.0), (0.0, 6.0)];
    const H2: Stroke = &[(3.0, 0.0), (3.0, 6.0)];
    const H3: Stroke = &[(0.0, 3.0), (3.0, 3.0)];
    const I1: Stroke = &[(0.5, 6.0), (2.5, 6.0)];
    const I2: Stroke = &[(1.5, 6.0), (1.5, 0.0)];
    const I3: Stroke = &[(0.5, 0.0), (2.5, 0.0)];
    const J: Stroke = &[(3.0, 6.0), (3.0, 1.0), (2.0, 0.0), (1.0, 0.0), (0.0, 1.0)];
    const K2: Stroke = &[(3.0, 6.0), (0.0, 3.0), (3.0, 0.0)];
    const L: Stroke = &[(0.0, 6.0), (0.0, 0.0), (3.0, 0.0)];
    const M: Stroke = &[(0.0, 0.0), (0.0, 6.0), (1.5, 3.0), (3.0, 6.0), (3.0, 0.0)];
    const N: Stroke = &[(0.0, 0.0), (0.0, 6.0), (3.0, 0.0), (3.0, 6.0)];
    const O: Stroke = &[
        (1.0, 0.0),
        (0.0, 1.0),
        (0.0, 5.0),
        (1.0, 6.0),
        (2.0, 6.0),
        (3.0, 5.0),
        (3.0, 1.0),
        (2.0, 0.0),
        (1.0, 0.0),
    ];
    const P: Stroke = &[
        (0.0, 0.0),
        (0.0, 6.0),
        (2.0, 6.0),
        (3.0, 5.0),
        (3.0, 4.0),
        (2.0, 3.0),
        (0.0, 3.0),
    ];
    const Q2: Stroke = &[(2.0, 1.5), (3.0, 0.0)];
    const R2: Stroke = &[(1.5, 3.0), (3.0, 0.0)];
    const S: Stroke = &[
        (3.0, 5.0),
        (2.0, 6.0),
        (1.0, 6.0),
        (0.0, 5.0),
        (0.0, 4.0),
        (1.0, 3.0),
        (2.0, 3.0),
        (3.0, 2.0),
        (3.0, 1.0),
        (2.0, 0.0),
        (1.0, 0.0),
        (0.0, 1.0),
    ];
    const T1: Stroke = &[(0.0, 6.0), (3.0, 6.0)];
    const U: Stroke = &[
        (0.0, 6.0),
        (0.0, 1.0),
        (1.0, 0.0),
        (2.0, 0.0),
        (3.0, 1.0),
        (3.0, 6.0),
    ];
    const V: Stroke = &[(0.0, 6.0), (1.5, 0.0), (3.0, 6.0)];
    const W: Stroke = &[(0.0, 6.0), (0.75, 0.0), (1.5, 3.0), (2.25, 0.0), (3.0, 6.0)];
    const X1: Stroke = &[(0.0, 6.0), (3.0, 0.0)];
    const X2: Stroke = &[(0.0, 0.0), (3.0, 6.0)];
    const Y1: Stroke = &[(0.0, 6.0), (1.5, 3.0), (3.0, 6.0)];
    const Y2: Stroke = &[(1.5, 3.0), (1.5, 0.0)];
    const Z: Stroke = &[(0.0, 6.0), (3.0, 6.0), (0.0, 0.0), (3.0, 0.0)];
    const D1: Stroke = &[(0.5, 5.0), (1.5, 6.0), (1.5, 0.0)];
    const D2: Stroke = &[
        (0.0, 5.0),
        (1.0, 6.0),
        (2.0, 6.0),
        (3.0, 5.0),
        (3.0, 4.0),
        (0.0, 0.0),
        (3.0, 0.0),
    ];
    const D3A: Stroke = &[
        (0.0, 5.0),
        (1.0, 6.0),
        (2.0, 6.0),
        (3.0, 5.0),
        (3.0, 4.0),
        (2.0, 3.0),
        (1.0, 3.0),
    ];
    const D3B: Stroke = &[
        (2.0, 3.0),
        (3.0, 2.0),
        (3.0, 1.0),
        (2.0, 0.0),
        (1.0, 0.0),
        (0.0, 1.0),
    ];
    const D4: Stroke = &[(2.5, 0.0), (2.5, 6.0), (0.0, 2.0), (3.0, 2.0)];
    const D5: Stroke = &[
        (3.0, 6.0),
        (0.0, 6.0),
        (0.0, 3.0),
        (2.0, 3.0),
        (3.0, 2.0),
        (3.0, 1.0),
        (2.0, 0.0),
        (1.0, 0.0),
        (0.0, 1.0),
    ];
    const D6: Stroke = &[
        (3.0, 5.0),
        (2.0, 6.0),
        (1.0, 6.0),
        (0.0, 5.0),
        (0.0, 1.0),
        (1.0, 0.0),
        (2.0, 0.0),
        (3.0, 1.0),
        (3.0, 2.0),
        (2.0, 3.0),
        (0.0, 3.0),
    ];
    const D7: Stroke = &[(0.0, 6.0), (3.0, 6.0), (1.0, 0.0)];
    const D8: Stroke = &[
        (1.0, 3.0),
        (0.0, 4.0),
        (0.0, 5.0),
        (1.0, 6.0),
        (2.0, 6.0),
        (3.0, 5.0),
        (3.0, 4.0),
        (2.0, 3.0),
        (1.0, 3.0),
        (0.0, 2.0),
        (0.0, 1.0),
        (1.0, 0.0),
        (2.0, 0.0),
        (3.0, 1.0),
        (3.0, 2.0),
        (2.0, 3.0),
    ];
    const D9: Stroke = &[
        (0.0, 1.0),
        (1.0, 0.0),
        (2.0, 0.0),
        (3.0, 1.0),
        (3.0, 5.0),
        (2.0, 6.0),
        (1.0, 6.0),
        (0.0, 5.0),
        (0.0, 4.0),
        (1.0, 3.0),
        (3.0, 3.0),
    ];
    const D0: Stroke = &[(0.5, 0.8), (2.5, 5.2)];
    const DASH: Stroke = &[(0.5, 3.0), (2.5, 3.0)];
    const DOT: Stroke = &[(1.3, 0.0), (1.7, 0.0), (1.7, 0.4), (1.3, 0.4), (1.3, 0.0)];
    const COMMA: Stroke = &[(1.7, 0.5), (1.2, -1.0)];
    const TICK: Stroke = &[(1.5, 6.0), (1.5, 4.5)];
    const QT1: Stroke = &[(1.0, 6.0), (1.0, 4.5)];
    const QT2: Stroke = &[(2.0, 6.0), (2.0, 4.5)];
    const SLASH: Stroke = &[(0.0, 0.0), (3.0, 6.0)];
    const LP: Stroke = &[(2.0, 6.0), (1.0, 5.0), (1.0, 1.0), (2.0, 0.0)];
    const RP: Stroke = &[(1.0, 6.0), (2.0, 5.0), (2.0, 1.0), (1.0, 0.0)];
    const COLON1: Stroke = &[(1.3, 1.0), (1.7, 1.0), (1.7, 1.4), (1.3, 1.4), (1.3, 1.0)];
    const COLON2: Stroke = &[(1.3, 4.0), (1.7, 4.0), (1.7, 4.4), (1.3, 4.4), (1.3, 4.0)];
    const PLUS1: Stroke = &[(0.5, 3.0), (2.5, 3.0)];
    const PLUS2: Stroke = &[(1.5, 2.0), (1.5, 4.0)];
    const EQ1: Stroke = &[(0.5, 2.0), (2.5, 2.0)];
    const EQ2: Stroke = &[(0.5, 4.0), (2.5, 4.0)];
    const PCT1: Stroke = &[(0.2, 5.0), (0.8, 5.0), (0.8, 6.0), (0.2, 6.0), (0.2, 5.0)];
    const PCT2: Stroke = &[(2.2, 0.0), (2.8, 0.0), (2.8, 1.0), (2.2, 1.0), (2.2, 0.0)];
    const HASH1: Stroke = &[(1.0, 0.0), (1.0, 6.0)];
    const HASH2: Stroke = &[(2.0, 0.0), (2.0, 6.0)];
    const HASH3: Stroke = &[(0.0, 2.0), (3.0, 2.0)];
    const HASH4: Stroke = &[(0.0, 4.0), (3.0, 4.0)];
    const UNKNOWN: Stroke = &[(0.0, 0.0), (3.0, 0.0), (3.0, 6.0), (0.0, 6.0), (0.0, 0.0)];
    let c = c.to_ascii_uppercase();
    Some(match c {
        ' ' => return None,
        'A' => vec![A, A2],
        'B' => vec![B1, B2],
        'C' => vec![C],
        'D' => vec![D],
        'E' => vec![E1, E2],
        'F' => vec![F1, E2],
        'G' => vec![G],
        'H' => vec![H1, H2, H3],
        'I' => vec![I1, I2, I3],
        'J' => vec![J],
        'K' => vec![H1, K2],
        'L' => vec![L],
        'M' => vec![M],
        'N' => vec![N],
        'O' => vec![O],
        'P' => vec![P],
        'Q' => vec![O, Q2],
        'R' => vec![P, R2],
        'S' => vec![S],
        'T' => vec![T1, I2],
        'U' => vec![U],
        'V' => vec![V],
        'W' => vec![W],
        'X' => vec![X1, X2],
        'Y' => vec![Y1, Y2],
        'Z' => vec![Z],
        '0' => vec![O, D0],
        '1' => vec![D1, I3],
        '2' => vec![D2],
        '3' => vec![D3A, D3B],
        '4' => vec![D4],
        '5' => vec![D5],
        '6' => vec![D6],
        '7' => vec![D7],
        '8' => vec![D8],
        '9' => vec![D9],
        '-' => vec![DASH],
        '.' => vec![DOT],
        ',' => vec![COMMA],
        '\'' => vec![TICK],
        '"' => vec![QT1, QT2],
        '/' => vec![SLASH],
        '(' => vec![LP],
        ')' => vec![RP],
        ':' => vec![COLON1, COLON2],
        '+' => vec![PLUS1, PLUS2],
        '=' => vec![EQ1, EQ2],
        '%' => vec![SLASH, PCT1, PCT2],
        '#' => vec![HASH1, HASH2, HASH3, HASH4],
        _ => vec![UNKNOWN],
    })
}

/// The line segments that draw the TEXT entity `rec` (anchored left or
/// centred as the entity says), in plan inches.
fn text_lines(rec: &[Pair]) -> Vec<(Point, Point)> {
    let text = value_of(rec, 1).unwrap_or("").to_string();
    let height = num_of(rec, 40).max(1e-6);
    let rot = num_of(rec, 50).to_radians();
    let centered = value_of(rec, 72).is_some_and(|v| v.trim() == "1");
    let anchor = if centered {
        Point::new(num_of(rec, 11), num_of(rec, 21))
    } else {
        Point::new(num_of(rec, 10), num_of(rec, 20))
    };
    let chars: Vec<char> = text.chars().collect();
    let width = chars.len() as f64 * ADVANCE * height;
    let (ox, oy) = if centered {
        (-width / 2.0, -height / 2.0)
    } else {
        (0.0, 0.0)
    };
    let unit = height / GLYPH_H;
    let (s, c) = rot.sin_cos();
    let place = |gx: f64, gy: f64, i: usize| -> Point {
        let x = ox + i as f64 * ADVANCE * height + gx * unit;
        let y = oy + gy * unit;
        Point::new(anchor.x + x * c - y * s, anchor.y + x * s + y * c)
    };
    let mut out = Vec::new();
    for (i, ch) in chars.iter().enumerate() {
        for stroke in glyph(*ch).unwrap_or_default() {
            for w in stroke.windows(2) {
                out.push((place(w[0].0, w[0].1, i), place(w[1].0, w[1].1, i)));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::dxf::write_dxf;
    use crate::model::{Project, WallKind};

    fn plan() -> Project {
        let mut p = Project::new("o");
        let pts = [(0.0, 0.0), (240.0, 0.0), (240.0, 120.0), (0.0, 120.0)];
        for i in 0..4 {
            let (a, b) = (pts[i], pts[(i + 1) % 4]);
            p.add_wall(
                0,
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                6.0,
                96.0,
                WallKind::Exterior,
            );
        }
        p.add_cad(
            0,
            "CAD, Default",
            crate::cad::CadItem::Text {
                pos: Point::new(10.0, 10.0),
                text: "Kitchen 12'".into(),
                height: 6.0,
                angle: 0.0,
            },
        );
        p.add_cad(
            0,
            "CAD, Default",
            crate::cad::CadItem::Circle {
                center: Point::new(50.0, 50.0),
                radius: 10.0,
            },
        );
        p
    }

    fn floor_text(p: &Project) -> FloorDxf {
        FloorDxf {
            name: "1st Floor".into(),
            elevation: 0.0,
            text: write_dxf(p, 0, &[]),
        }
    }

    fn count(s: &str, kind: &str) -> usize {
        s.lines().filter(|l| *l == kind).count()
    }

    fn pairs_ok(s: &str) {
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines.len() % 2, 0);
        for c in lines.chunks(2) {
            assert!(c[0].trim().parse::<i32>().is_ok(), "bad code {:?}", c[0]);
        }
        assert_eq!(&lines[lines.len() - 2..], ["0", "EOF"]);
    }

    #[test]
    fn default_options_keep_the_geometry_and_the_names() {
        let p = plan();
        let f = floor_text(&p);
        let s = build(
            std::slice::from_ref(&f),
            &[],
            &p.layers,
            &DxfOptions::default(),
        );
        pairs_ok(&s);
        assert!(s.contains("Walls, Normal") && s.contains("CAD, Default"));
        assert_eq!(count(&s, "POLYLINE"), count(&f.text, "POLYLINE"));
        assert_eq!(count(&s, "TEXT"), 1);
        assert_eq!(count(&s, "CIRCLE"), 1);
    }

    #[test]
    fn units_scale_every_length_and_set_insunits() {
        let p = plan();
        let f = floor_text(&p);
        let opts = DxfOptions {
            units: DxfUnits::Millimeters,
            ..DxfOptions::default()
        };
        let s = build(std::slice::from_ref(&f), &[], &p.layers, &opts);
        let lines: Vec<&str> = s.lines().collect();
        let i = lines.iter().position(|l| *l == "$INSUNITS").unwrap();
        assert_eq!(lines[i + 1], "70");
        assert_eq!(lines[i + 2], "4");
        // The circle's radius: 10 in = 254 mm; its centre 50 in = 1270 mm.
        let at = lines.iter().position(|l| *l == "CIRCLE").unwrap();
        let rec: Vec<&str> = lines[at..].iter().take(14).copied().collect();
        assert!(rec.contains(&"1270.0000"), "{rec:?}");
        assert!(rec.contains(&"254.0000"), "{rec:?}");
        let feet = DxfOptions {
            units: DxfUnits::Feet,
            ..DxfOptions::default()
        };
        let s = build(std::slice::from_ref(&f), &[], &p.layers, &feet);
        assert!(s.contains("4.1667"), "50 in is 4.1667 ft");
    }

    #[test]
    fn aia_names_and_the_custom_map() {
        let p = plan();
        let f = floor_text(&p);
        let opts = DxfOptions {
            naming: LayerNaming::Aia,
            layer_map: vec![("CAD, Default".into(), "MY-CAD".into())],
            ..DxfOptions::default()
        };
        let s = build(std::slice::from_ref(&f), &[], &p.layers, &opts);
        assert!(s.contains("A-WALL"));
        assert!(s.contains("MY-CAD"));
        assert!(!s.contains("Walls, Normal"));
        assert_eq!(aia_layer("Doors"), "A-DOOR");
        assert_eq!(aia_layer("Windows"), "A-GLAZ");
        assert_eq!(aia_layer("Dimensions, Manual"), "A-ANNO-DIMS");
        assert_eq!(aia_layer("Something Else"), "Something Else");
    }

    #[test]
    fn line_weights_go_in_the_layer_table() {
        let p = plan();
        let f = floor_text(&p);
        let off = build(
            std::slice::from_ref(&f),
            &[],
            &p.layers,
            &DxfOptions::default(),
        );
        assert!(!off.lines().any(|l| l == "370"));
        let on = build(
            std::slice::from_ref(&f),
            &[],
            &p.layers,
            &DxfOptions {
                line_weights: true,
                ..DxfOptions::default()
            },
        );
        let lines: Vec<&str> = on.lines().collect();
        let at = lines.iter().position(|l| *l == "Walls, Normal").unwrap();
        let rec: Vec<&str> = lines[at..].iter().take(12).copied().collect();
        let w = p
            .layers
            .get("Walls, Normal")
            .unwrap()
            .line_weight
            .to_string();
        assert!(
            rec.windows(2).any(|w2| w2[0] == "370" && w2[1] == w),
            "{rec:?}"
        );
    }

    #[test]
    fn text_can_be_drawn_as_lines() {
        let p = plan();
        let f = floor_text(&p);
        let before = count(&f.text, "LINE");
        let s = build(
            std::slice::from_ref(&f),
            &[],
            &p.layers,
            &DxfOptions {
                text: TextMode::Lines,
                ..DxfOptions::default()
            },
        );
        pairs_ok(&s);
        assert_eq!(count(&s, "TEXT"), 0);
        assert!(count(&s, "LINE") > before + 20, "the letters are strokes");
    }

    #[test]
    fn several_floors_get_their_own_layers_and_elevations_in_3d() {
        let p = plan();
        let mut up = floor_text(&p);
        up.name = "2nd Floor".into();
        up.elevation = 108.0;
        let f1 = floor_text(&p);
        let opts = DxfOptions {
            three_d: true,
            ..DxfOptions::default()
        };
        let tri = SolidDxf {
            layer: "Wall Exterior".into(),
            triangles: vec![[[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 5.0]]],
        };
        let s = build(&[f1, up], &[tri], &p.layers, &opts);
        pairs_ok(&s);
        assert!(s.contains("1st Floor - Walls, Normal"));
        assert!(s.contains("2nd Floor - Walls, Normal"));
        assert_eq!(count(&s, "3DFACE"), 1);
        assert!(
            s.contains("108.0000"),
            "the upper floor sits at its elevation"
        );
        // In 2D everything is flat.
        let flat = build(&[floor_text(&p)], &[], &p.layers, &DxfOptions::default());
        assert!(!flat.contains("108.0000"));
        assert_eq!(count(&flat, "3DFACE"), 0);
    }

    #[test]
    fn layer_names_are_made_safe() {
        assert_eq!(clean_layer("A/B:C"), "A-B-C");
        assert_eq!(clean_layer("  "), "0");
    }
}
