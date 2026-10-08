//! Phase C decoders: typed objects inside a template body.
//!
//! Phase A lists names and Phase B decodes layer tables from their record
//! frame. This module decodes the *object stream*: Chief writes most template
//! content as objects introduced by the marker `[01] CD AB <class> <version>
//! <u32 size>`, where `size` counts itself and the payload (so an object spans
//! `[size_pos, size_pos + size)`). Strings inside payloads are a `u32` length,
//! the bytes, and a NUL.
//!
//! Decoded (see `docs/chief-template-format.md`, section 8, for offsets,
//! counts and confidence):
//!
//! * class 57 materials: id, name, colour
//! * class 215 wall types: name and the layer stack (material id, thickness,
//!   main/framing flags), 518 bytes per layer record
//! * class 139 text styles: font, style (`Book`/`Heavy`), height, GUID
//! * class 64 rich text defaults: font, size, text and background colour
//! * class 129 dimension defaults: arrow, extension, line separation, number
//!   format, text style (by GUID)
//! * class 23 room types: the default ceiling height
//! * paper sizes: the two `f64`s before a sheet-size name
//!
//! Every reader is bounds-checked and returns `None` or skips an object when
//! the bytes do not fit the expected shape; nothing here panics on junk.

use crate::scan::{walk_strings, TemplateKind};
use serde::{Deserialize, Serialize};

/// `CD AB`, the object marker (bytes 0-1 of the "CDAB record" in
/// `docs/chief-library-format.md`).
pub const OBJECT_MARKER: [u8; 2] = [0xCD, 0xAB];

/// Class ids (the byte after the marker) the decoders understand.
pub mod class {
    pub const MATERIAL: u8 = 57;
    pub const ROOM_TYPE: u8 = 23;
    pub const TEXT_STYLE: u8 = 139;
    pub const DIMENSION_DEFAULTS: u8 = 129;
    pub const RICH_TEXT_DEFAULTS: u8 = 64;
    pub const WALL_TYPE: u8 = 215;
}

/// Bytes per wall layer record inside a wall type object.
pub const WALL_LAYER_STRIDE: usize = 518;

/// One object found by [`find_objects`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawObject {
    pub class: u8,
    pub version: u8,
    /// The byte before the marker (1 for most top-level objects, 0 or other
    /// values for nested ones); not interpreted.
    pub flag: u8,
    /// Offset of the `CD` byte.
    pub marker: usize,
    /// Offset of the `u32` size field (`marker + 4`).
    pub size_pos: usize,
    /// One past the last byte of the object.
    pub end: usize,
}

impl RawObject {
    /// First payload byte (just after the size field).
    pub fn payload(&self) -> usize {
        self.size_pos + 4
    }
}

fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        b.get(o..o.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        b.get(o..o.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn f64_at(b: &[u8], o: usize) -> Option<f64> {
    Some(f64::from_le_bytes(
        b.get(o..o.checked_add(8)?)?.try_into().ok()?,
    ))
}

/// A finite `f64` within `[lo, hi]`. Non-zero values smaller than 1e-4 are
/// rejected: in these files they are text or flag bytes read as a double
/// (the smallest real value in a template is a 0.01" housewrap).
fn f64_in(b: &[u8], o: usize, lo: f64, hi: f64) -> Option<f64> {
    f64_at(b, o)
        .filter(|v| v.is_finite() && *v >= lo && *v <= hi)
        .filter(|v| *v == 0.0 || v.abs() >= 1e-4)
}

/// A length stored as an inch value: zero or at least 1/64".
fn length_in(b: &[u8], o: usize, hi: f64) -> Option<f64> {
    f64_in(b, o, 0.0, hi).filter(|v| *v == 0.0 || *v >= 1.0 / 64.0)
}

/// Rounds away binary noise (`0.010000000000000009` is 0.01).
fn tidy(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

fn printable(s: &[u8]) -> bool {
    s.iter().all(|&c| (0x20..=0x7E).contains(&c))
}

/// Reads a `u32`-length-prefixed string at `o` (length at most `max`). Returns
/// the text and the offset just after the bytes (the NUL is not skipped).
fn read_str(b: &[u8], o: usize, max: usize) -> Option<(String, usize)> {
    let len = u32_at(b, o)? as usize;
    if len > max {
        return None;
    }
    let start = o + 4;
    let bytes = b.get(start..start.checked_add(len)?)?;
    printable(bytes).then(|| (String::from_utf8_lossy(bytes).into_owned(), start + len))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Every plausible object in `bytes`, in file order. An object is plausible
/// when its declared size is at least 4 and stays inside the file. Objects can
/// nest, so the list is flat and spans may overlap.
pub fn find_objects(bytes: &[u8]) -> Vec<RawObject> {
    let mut out = Vec::new();
    let len = bytes.len();
    let mut i = 1;
    while i + 8 <= len {
        if bytes[i] == OBJECT_MARKER[0] && bytes[i + 1] == OBJECT_MARKER[1] {
            let size_pos = i + 4;
            if let Some(size) = u32_at(bytes, size_pos) {
                let end = size_pos.saturating_add(size as usize);
                if size >= 4 && end <= len {
                    out.push(RawObject {
                        class: bytes[i + 2],
                        version: bytes[i + 3],
                        flag: bytes[i - 1],
                        marker: i,
                        size_pos,
                        end,
                    });
                }
            }
        }
        i += 1;
    }
    out
}

// ---------------------------------------------------------------- materials

/// Marker that follows a material's colour: `FF` (alpha) and `88 13 00 00`
/// (5000, a constant in every sample).
const MATERIAL_ANCHOR: [u8; 5] = [0xFF, 0x88, 0x13, 0x00, 0x00];

/// One entry of the template's material table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateMaterial {
    /// The id wall layers refer to (`u16` before the colour).
    pub id: u32,
    /// Empty for unnamed entries (colour-palette swatches).
    pub name: String,
    pub color: [u8; 3],
    /// Offset of the object marker.
    pub offset: u64,
}

fn decode_material(b: &[u8], o: &RawObject) -> Option<TemplateMaterial> {
    let payload = o.payload();
    let seg = b.get(payload..o.end)?;
    let k = seg.windows(5).position(|w| w == MATERIAL_ANCHOR)?;
    if k < 8 {
        return None;
    }
    let anchor = payload + k;
    let id = u32::from(u16_at(b, anchor - 7)?);
    let color = [b[anchor - 3], b[anchor - 2], b[anchor - 1]];
    // The name ends one NUL before the id; find the shortest length prefix
    // that lines up with it.
    let nul = anchor - 8;
    let mut name = String::new();
    if b.get(nul) == Some(&0) {
        for len in 1..=120usize {
            let Some(start) = nul.checked_sub(len + 4) else {
                break;
            };
            if start < payload {
                break;
            }
            if u32_at(b, start) == Some(len as u32) && printable(&b[start + 4..nul]) {
                name = String::from_utf8_lossy(&b[start + 4..nul]).into_owned();
                break;
            }
        }
    }
    Some(TemplateMaterial {
        id,
        name,
        color,
        offset: o.marker as u64,
    })
}

/// The material table (class 57, version 0).
pub fn decode_materials(b: &[u8], objs: &[RawObject]) -> Vec<TemplateMaterial> {
    objs.iter()
        .filter(|o| o.class == class::MATERIAL && o.version == 0)
        .filter_map(|o| decode_material(b, o))
        .collect()
}

// --------------------------------------------------------------- wall types

/// One layer of a decoded wall type, exterior face first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateWallLayer {
    /// Material name (`material #<id>` when the id is not in the table).
    pub material: String,
    pub material_id: u32,
    pub thickness_in: f64,
    /// Byte `+0x15` of the layer record (High confidence: exactly the studs
    /// or the concrete/CMU core are flagged in Daniel's types).
    pub is_main: bool,
    /// Byte `+0x16` (Medium: set on framing members, clear on `Interior-4`).
    pub is_framing: bool,
    /// Byte `+0x17` (Low: set on air-gap and `Opening (no material)` layers).
    pub is_gap: bool,
    /// The `f64` at `+0x18` (Medium): 16 or 24 on studs (on-centre spacing),
    /// 96 on sheet goods (sheet length), 7 or 8 on siding and brick (course
    /// exposure), 0 where the material has no module.
    pub spacing_in: f64,
}

/// A wall type definition from the template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateWallType {
    pub name: String,
    /// `(material, thickness in inches, is main layer)`, exterior face first.
    pub layers: Vec<(String, f64, bool)>,
    /// The same layers with every decoded flag.
    pub layer_details: Vec<TemplateWallLayer>,
    /// Sum of the layer thicknesses, inches.
    pub total_thickness_in: f64,
    /// Offset of the object marker.
    pub offset: u64,
}

fn material_label(id: u32, materials: &[TemplateMaterial]) -> String {
    materials
        .iter()
        .find(|m| m.id == id && !m.name.is_empty())
        .map_or_else(|| format!("material #{id}"), |m| m.name.clone())
}

fn decode_wall_type(
    b: &[u8],
    o: &RawObject,
    materials: &[TemplateMaterial],
) -> Option<TemplateWallType> {
    let p = o.payload();
    let (name, name_end) = read_str(b, p, 160)?;
    // NUL, then the record count (layers plus one trailing sentinel record).
    let count = u32_at(b, name_end + 1)? as usize;
    if !(1..=40).contains(&count) {
        return None;
    }
    let first = name_end + 5;
    let last = first.checked_add(count.checked_mul(WALL_LAYER_STRIDE)?)?;
    if last > o.end {
        return None;
    }
    let mut details = Vec::new();
    for k in 0..count - 1 {
        let r = first + k * WALL_LAYER_STRIDE;
        let thickness = f64_in(b, r, 0.0, 1000.0)?;
        let id = u32_at(b, r + 8)?;
        details.push(TemplateWallLayer {
            material: material_label(id, materials),
            material_id: id,
            thickness_in: tidy(thickness),
            is_main: *b.get(r + 0x15)? == 1,
            is_framing: *b.get(r + 0x16)? == 1,
            is_gap: *b.get(r + 0x17)? == 1,
            spacing_in: tidy(f64_in(b, r + 0x18, 0.0, 1000.0).unwrap_or(0.0)),
        });
    }
    let total = tidy(details.iter().map(|l| l.thickness_in).sum());
    Some(TemplateWallType {
        name,
        layers: details
            .iter()
            .map(|l| (l.material.clone(), l.thickness_in, l.is_main))
            .collect(),
        layer_details: details,
        total_thickness_in: total,
        offset: o.marker as u64,
    })
}

/// Wall types (class 215, version 0), in file order.
pub fn decode_wall_types(
    b: &[u8],
    objs: &[RawObject],
    materials: &[TemplateMaterial],
) -> Vec<TemplateWallType> {
    objs.iter()
        .filter(|o| o.class == class::WALL_TYPE && o.version == 0)
        .filter_map(|o| decode_wall_type(b, o, materials))
        .collect()
}

// -------------------------------------------------------------- text styles

/// A text style from the template (class 139).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateTextStyle {
    pub name: String,
    pub font: String,
    /// The font style string: `Book` or `Heavy` in Daniel's templates.
    pub font_style: String,
    /// The leading `f64`: plan inches (`1/4" Text Style` is 4.5, which prints
    /// 3/32" tall at 1/4" scale; `1" Text Style` is 1.125).
    pub height_in: f64,
    /// `font_style` is `Heavy`/`Bold`.
    pub bold: bool,
    /// `font_style` contains `Italic`/`Oblique`.
    pub italic: bool,
    /// Not found in the object (None); text colour lives elsewhere.
    pub color: Option<[u8; 3]>,
    /// The last 16 bytes of the object, hex. Dimension defaults refer to a
    /// style by this id.
    pub guid: String,
    pub offset: u64,
}

fn decode_text_style(b: &[u8], o: &RawObject) -> Option<TemplateTextStyle> {
    let p = o.payload();
    let height = f64_in(b, p, 0.0, 1000.0)?;
    let (font, font_end) = read_str(b, p + 8, 80)?;
    let (style, style_end) = read_str(b, font_end + 1, 40)?;
    // 12 flag bytes (weight bit, then unknown flags), then the name.
    let (name, name_end) = read_str(b, style_end + 1 + 12, 160)?;
    if name.is_empty() || name_end + 1 + 1 + 16 > o.end {
        return None;
    }
    let guid = b.get(o.end - 16..o.end)?;
    let lower = style.to_ascii_lowercase();
    Some(TemplateTextStyle {
        name,
        font,
        bold: lower.contains("heavy") || lower.contains("bold"),
        italic: lower.contains("italic") || lower.contains("oblique"),
        font_style: style,
        height_in: height,
        color: None,
        guid: hex(guid),
        offset: o.marker as u64,
    })
}

/// Text styles (class 139, version 0).
pub fn decode_text_styles(b: &[u8], objs: &[RawObject]) -> Vec<TemplateTextStyle> {
    objs.iter()
        .filter(|o| o.class == class::TEXT_STYLE && o.version == 0)
        .filter_map(|o| decode_text_style(b, o))
        .collect()
}

// ------------------------------------------------------- rich text defaults

/// A `... Rich Text Defaults` entry (class 64): the text settings stored as
/// strings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateRichText {
    pub name: String,
    pub font: String,
    /// Empty for fonts without a style string (`Chief Blueprint`).
    pub font_style: String,
    /// Printed text size, inches (the string `0.166666666666667` is 1/6").
    pub size_in: f64,
    pub color: [u8; 3],
    pub background: [u8; 3],
    pub offset: u64,
}

fn parse_hex_rgb(s: &str) -> Option<[u8; 3]> {
    if s.len() != 6 || !s.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

fn decode_rich_text(b: &[u8], o: &RawObject) -> Option<TemplateRichText> {
    let seg = b.get(o.payload()..o.end)?;
    let strings = walk_strings(seg, 0);
    let (_, name) = strings.first()?;
    if !name.ends_with("Rich Text Defaults")
        && !name.contains("Rich Text Defaults,")
        && !name.contains("Rich Text Default")
    {
        return None;
    }
    // After the name: font [style] size colour background.
    let rest: Vec<&str> = strings.iter().skip(1).map(|(_, s)| s.as_str()).collect();
    let size_at = rest
        .iter()
        .position(|s| s.parse::<f64>().is_ok() && s.chars().any(|c| c.is_ascii_digit()))?;
    if size_at == 0 || size_at > 2 {
        return None;
    }
    let size_in = rest[size_at].parse::<f64>().ok()?;
    let color = parse_hex_rgb(rest.get(size_at + 1)?)?;
    let background = parse_hex_rgb(rest.get(size_at + 2)?)?;
    Some(TemplateRichText {
        name: name.clone(),
        font: rest[0].to_string(),
        font_style: if size_at == 2 {
            rest[1].to_string()
        } else {
            String::new()
        },
        size_in,
        color,
        background,
        offset: o.marker as u64,
    })
}

/// Rich text defaults (class 64, version 1 in Daniel's files; the version is
/// not checked because the name test is stricter). Where a name repeats (the
/// template stores a second, label-specific copy), the first one wins.
pub fn decode_rich_text_defaults(b: &[u8], objs: &[RawObject]) -> Vec<TemplateRichText> {
    let mut out: Vec<TemplateRichText> = Vec::new();
    for o in objs.iter().filter(|o| o.class == class::RICH_TEXT_DEFAULTS) {
        if let Some(r) = decode_rich_text(b, o) {
            if !out.iter().any(|x| x.name == r.name) {
                out.push(r);
            }
        }
    }
    out
}

// ------------------------------------------------------ dimension defaults

/// What the decoder reads from a dimension default set (class 129). Values
/// are inches. A field is `None` when the bytes do not look plausible for that
/// set (the sets written by older Chief versions have a different shape
/// after the format block).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TemplateDimensionDefaults {
    /// Full name, `1/4" Scale Dimension Defaults`.
    pub name: String,
    /// Name of the text style the set points at (resolved through the style's
    /// GUID); `None` when the set has no matching style.
    pub text_style: Option<String>,
    pub text_font: Option<String>,
    /// The text height stored in the set (a copy of the style's height).
    pub text_height_in: Option<f64>,
    pub arrow_size_in: Option<f64>,
    /// Extensions tab: Length Away From Marked Object.
    pub extension_length_away_in: Option<f64>,
    /// Extensions tab: Fixed Gap From Marked Object.
    pub extension_fixed_gap_in: Option<f64>,
    /// Extensions tab: Length Towards Marked Object.
    pub extension_length_towards_in: Option<f64>,
    /// Extensions tab: Fixed Proximity, Distance To Marked Object.
    pub extension_proximity_in: Option<f64>,
    /// Setup Automatic: Line Separation.
    pub line_separation_in: Option<f64>,
    /// Reach (24" at 1/4" scale).
    pub reach_in: Option<f64>,
    /// General: Baseline Line Separation.
    pub baseline_separation_in: Option<f64>,
    /// Setup Automatic: 1st Line Offset.
    pub first_line_offset_in: Option<f64>,
    /// Setup Automatic, Exterior: Reach.
    pub exterior_reach_in: Option<f64>,
    /// Primary Format, Accuracy: Decimal Places.
    pub decimal_places: Option<u32>,
    /// Primary Format, Accuracy: Smallest Fraction 1/N.
    pub smallest_fraction: Option<u32>,
    /// Primary Format: Fraction Text Size, percent.
    pub fraction_text_size_pct: Option<u32>,
    /// The unit strings stored in the format blocks (`ft`, `in`).
    pub unit_labels: Vec<String>,
    pub offset: u64,
}

/// Offsets below are from the end of the name (length prefix + bytes + NUL),
/// which is 0x22 in the `1/4" Scale` set the numbers were first read from.
const DIM_ARROW: usize = 0x4b - 0x22;
const DIM_DECIMALS: usize = 0x5d - 0x22;
const DIM_FRACTION: usize = 0x61 - 0x22;
const DIM_FRACTION_PCT: usize = 0xc0 - 0x22;
/// Offsets below are from the `Avenir`/`Arial` string that follows the text
/// height `f64` (`F`; 0x1c1 in the `1/4" Scale` set).
const DIM_TEXT_HEIGHT: isize = -8;
const DIM_STYLE_GUID: usize = 0x1e;
const DIM_EXT_AWAY: usize = 0x32;
const DIM_EXT_GAP: usize = 0x3a;
const DIM_EXT_TOWARDS: usize = 0x42;
const DIM_EXT_PROXIMITY: usize = 0x4b;
const DIM_LINE_SEPARATION: usize = 0x54;
const DIM_REACH: usize = 0x5c;
const DIM_BASELINE: usize = 0x64;
const DIM_FIRST_OFFSET: usize = 0x6c;
/// From the font anchor to the exterior reach (`0x385 - 0x1c1`).
const DIM_EXT_REACH: usize = 0x385 - 0x1c1;

fn power_of_two_denominator(v: u32) -> bool {
    (1..=256).contains(&v) && v.is_power_of_two()
}

fn decode_dimension_defaults(
    b: &[u8],
    o: &RawObject,
    styles: &[TemplateTextStyle],
) -> Option<TemplateDimensionDefaults> {
    let p = o.payload();
    let (name, name_end) = read_str(b, p, 160)?;
    let after = name_end + 1;
    let seg = b.get(p..o.end)?;
    let at = |rel: usize| -> Option<usize> { after.checked_add(rel).filter(|&x| x + 8 <= o.end) };
    let dim = |rel: usize, hi: f64| -> Option<f64> { length_in(b, at(rel)?, hi) };

    // Font anchor: the first text font string well past the name.
    let font_at = ["\u{6}\0\0\0Avenir\0", "\u{5}\0\0\0Arial\0"]
        .iter()
        .filter_map(|pat| {
            let pat = pat.as_bytes();
            seg.windows(pat.len())
                .enumerate()
                .find(|(i, w)| *i > 0x80 && *w == pat)
                .map(|(i, _)| p + i)
        })
        .min();

    let mut d = TemplateDimensionDefaults {
        name,
        offset: o.marker as u64,
        arrow_size_in: dim(DIM_ARROW, 100.0),
        decimal_places: at(DIM_DECIMALS)
            .and_then(|x| u32_at(b, x))
            .filter(|v| *v <= 8),
        smallest_fraction: at(DIM_FRACTION)
            .and_then(|x| u32_at(b, x))
            .filter(|v| power_of_two_denominator(*v)),
        fraction_text_size_pct: at(DIM_FRACTION_PCT)
            .and_then(|x| u32_at(b, x))
            .filter(|v| (1..=200).contains(v)),
        ..TemplateDimensionDefaults::default()
    };

    // The unit strings inside the format blocks.
    for (_, s) in walk_strings(seg, 0) {
        if (s == "ft" || s == "in") && !d.unit_labels.contains(&s) {
            d.unit_labels.push(s);
        }
    }

    if let Some(f) = font_at {
        d.text_font = read_str(b, f, 40).map(|(s, _)| s);
        let rel = |off: usize| -> Option<usize> { Some(f + off).filter(|&x| x + 8 <= o.end) };
        d.text_height_in = f
            .checked_add_signed(DIM_TEXT_HEIGHT)
            .and_then(|x| length_in(b, x, 200.0));
        d.extension_length_away_in = rel(DIM_EXT_AWAY).and_then(|x| length_in(b, x, 200.0));
        d.extension_fixed_gap_in = rel(DIM_EXT_GAP).and_then(|x| length_in(b, x, 200.0));
        d.extension_length_towards_in = rel(DIM_EXT_TOWARDS).and_then(|x| length_in(b, x, 200.0));
        d.extension_proximity_in = rel(DIM_EXT_PROXIMITY).and_then(|x| length_in(b, x, 200.0));
        d.line_separation_in = rel(DIM_LINE_SEPARATION).and_then(|x| length_in(b, x, 500.0));
        d.reach_in = rel(DIM_REACH).and_then(|x| length_in(b, x, 1000.0));
        d.baseline_separation_in = rel(DIM_BASELINE).and_then(|x| length_in(b, x, 500.0));
        d.first_line_offset_in = rel(DIM_FIRST_OFFSET).and_then(|x| length_in(b, x, 500.0));
        d.exterior_reach_in = rel(DIM_EXT_REACH).and_then(|x| length_in(b, x, 5000.0));
        if let Some(guid) = f.checked_add(DIM_STYLE_GUID).and_then(|x| b.get(x..x + 16)) {
            let g = hex(guid);
            d.text_style = styles.iter().find(|s| s.guid == g).map(|s| s.name.clone());
        }
    }
    Some(d)
}

/// Dimension default sets (class 129, version 0). `styles` resolves the text
/// style GUIDs.
pub fn decode_dimension_defaults_all(
    b: &[u8],
    objs: &[RawObject],
    styles: &[TemplateTextStyle],
) -> Vec<TemplateDimensionDefaults> {
    objs.iter()
        .filter(|o| o.class == class::DIMENSION_DEFAULTS && o.version == 0)
        .filter_map(|o| decode_dimension_defaults(b, o, styles))
        .collect()
}

// ----------------------------------------------------------- default heights

/// Heights found in the room type definitions (class 23).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DefaultHeights {
    /// 109.125 (9'-1 1/8") in Daniel's plan template: an `f64` at the same
    /// place in every full room type definition. Stored once per room type.
    /// Medium confidence on the role (ceiling/wall height; Chief's Flat Roof
    /// preview shows 121 1/8" for the same type, so the dialog may add a
    /// 12" platform).
    pub room_type_height_in: Option<f64>,
    /// Room type objects that carry a height.
    pub room_types_with_height: u32,
    /// Of those, how many agree with `room_type_height_in`.
    pub room_types_agreeing: u32,
    /// Floor, foundation, rough ceiling and stem wall heights: not located.
    pub floor_foundation_rough_ceiling_stem_found: bool,
}

/// The first `f64` in the 0x200..0x300 window of a room type payload that is
/// a multiple of 1/8" between 90 and 150 inches.
fn room_type_height(b: &[u8], o: &RawObject) -> Option<f64> {
    let p = o.payload();
    let lo = p + 0x200;
    let hi = (p + 0x300).min(o.end.saturating_sub(8));
    (lo..hi).find_map(|j| {
        f64_in(b, j, 90.0, 150.0).filter(|v| ((v * 8.0).round() - v * 8.0).abs() < 1e-9)
    })
}

/// Reads [`DefaultHeights`] from the room types.
pub fn decode_default_heights(b: &[u8], objs: &[RawObject]) -> DefaultHeights {
    let mut counts: Vec<(f64, u32)> = Vec::new();
    let mut with = 0;
    for o in objs
        .iter()
        .filter(|o| o.class == class::ROOM_TYPE && o.version == 0)
    {
        if let Some(v) = room_type_height(b, o) {
            with += 1;
            match counts.iter_mut().find(|(x, _)| *x == v) {
                Some((_, n)) => *n += 1,
                None => counts.push((v, 1)),
            }
        }
    }
    let best = counts.iter().max_by_key(|(_, n)| *n).copied();
    DefaultHeights {
        room_type_height_in: best.map(|(v, _)| v),
        room_types_with_height: with,
        room_types_agreeing: best.map_or(0, |(_, n)| n),
        floor_foundation_rough_ceiling_stem_found: false,
    }
}

// --------------------------------------------------------------- paper sizes

/// A paper / sheet size entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaperSize {
    /// `ARCH C (18" x 24")` with inner double spaces collapsed.
    pub name: String,
    /// The two `f64`s before the name (one NUL byte between), inches.
    pub width_in: f64,
    pub height_in: f64,
    /// In the plan template the four `f64`s before those: two margins then
    /// the printable width and height. `None` in the layout.
    pub printable_width_in: Option<f64>,
    pub printable_height_in: Option<f64>,
    pub offset: u64,
}

fn looks_like_sheet_name(s: &str) -> bool {
    s.contains('"') && s.contains(" x ") && s.contains('(')
}

/// Sheet-size entries, wherever a name is preceded by a plausible size pair.
pub fn decode_paper_sizes(b: &[u8], strings: &[(u64, String)]) -> Vec<PaperSize> {
    let mut out: Vec<PaperSize> = Vec::new();
    for (off, s) in strings {
        if !looks_like_sheet_name(s) {
            continue;
        }
        let o = *off as usize;
        // A NUL byte separates the last f64 from the name's length prefix.
        let at = |back: usize| o.checked_sub(back);
        let (Some(w), Some(h)) = (
            at(17).and_then(|x| f64_in(b, x, 1.0, 400.0)),
            at(9).and_then(|x| f64_in(b, x, 1.0, 400.0)),
        ) else {
            continue;
        };
        let printable = at(33)
            .and_then(|x| f64_in(b, x, 1.0, 400.0))
            .zip(at(25).and_then(|x| f64_in(b, x, 1.0, 400.0)))
            // Margins (< 2") precede a printable area that is not the paper.
            .filter(|_| at(49).and_then(|x| f64_in(b, x, 0.01, 2.0)).is_some());
        let name = s.split_whitespace().collect::<Vec<_>>().join(" ");
        if out.iter().any(|p| p.name == name) {
            continue;
        }
        out.push(PaperSize {
            name,
            width_in: w,
            height_in: h,
            printable_width_in: printable.map(|(a, _)| a),
            printable_height_in: printable.map(|(_, c)| c),
            offset: *off,
        });
    }
    out
}

// ------------------------------------------------------------------ layout

/// What the layout template stores about its page(s).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutInfo {
    /// A `Page Template` entry exists.
    pub page_template_found: bool,
    /// Layout page objects with boxes: none identified (Low confidence that
    /// the template really has none). The 4 "layout pages" Phase A reports
    /// are style and layer names.
    pub page_count: Option<u32>,
    /// Layout boxes and their scales: not stored in the template.
    pub box_scales: Vec<String>,
    /// The printer named in the page setup (`EPSON_WF_7610_Series`).
    pub printer: Option<String>,
    /// Whether an embedded JPEG (the logo) sits in the body, and its length.
    pub embedded_jpeg_bytes: Option<u64>,
    /// Title block macro strings with the offset of each.
    pub title_block_macros: Vec<(String, u64)>,
    /// Field names of the project-information block (Project, Designer,
    /// Client, ...), with offsets. Their positions on a page are not stored.
    pub project_info_fields: Vec<(String, u64)>,
    /// `18x24` from the file name when the numbers are not in the template.
    pub sheet_from_file_name: Option<(f64, f64)>,
}

/// `18x24` (or `24x36`) at the start of a file name, as inches.
pub fn sheet_from_file_name(file_name: &str) -> Option<(f64, f64)> {
    let head: String = file_name
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == 'x' || *c == '.')
        .collect();
    let (a, b) = head.split_once('x')?;
    let (a, b) = (a.parse::<f64>().ok()?, b.parse::<f64>().ok()?);
    (a > 0.0 && b > 0.0 && a <= 200.0 && b <= 200.0).then_some((a, b))
}

const PROJECT_INFO_FIELDS: [&str; 21] = [
    "Project",
    "Project Name",
    "Street",
    "City",
    "State/Province",
    "Zip/Postal Code",
    "Country/Region",
    "APN",
    "Property Zone",
    "Occupancy Group",
    "Construction Type",
    "Designer",
    "Name",
    "Company Name",
    "Phone Number 1",
    "Phone Number 2",
    "Cell Phone Number",
    "Fax Number",
    "Web Site",
    "E-mail Address",
    "Client",
];

fn decode_layout(
    b: &[u8],
    strings: &[(u64, String)],
    file_name: &str,
    resource_start: usize,
) -> LayoutInfo {
    let mut info = LayoutInfo {
        page_template_found: strings.iter().any(|(_, s)| s == "Page Template"),
        page_count: Some(0),
        ..LayoutInfo::default()
    };
    info.printer = strings
        .iter()
        .find(|(_, s)| s.contains("_Series") || s.starts_with("EPSON") || s.contains("LaserJet"))
        .map(|(_, s)| s.clone());
    // The project block: from `Project` through `Country/Region` of the client.
    let start = strings.iter().position(|(_, s)| s == "Project");
    if let Some(start) = start {
        for (off, s) in strings.iter().skip(start).take(30) {
            if PROJECT_INFO_FIELDS.contains(&s.as_str()) {
                info.project_info_fields.push((s.clone(), *off));
            }
        }
    }
    for (off, s) in strings {
        let t = s.trim();
        let is_macro = (t.starts_with('%') && t.ends_with('%'))
            || t.contains("%floor.name%")
            || t.contains("%room.name%");
        if is_macro
            && t != "%automatic_description%"
            && !info.title_block_macros.iter().any(|(m, _)| m == t)
        {
            info.title_block_macros.push((t.to_string(), *off));
        }
    }
    // An embedded JPEG (the logo): the first SOI past the first 1/16 of the
    // file, through the last EOI before the resource table.
    let limit = resource_start.min(b.len());
    let first_search = b.len() / 16;
    if let Some(soi) = b
        .get(first_search..limit)
        .and_then(|w| w.windows(3).position(|x| x == [0xFF, 0xD8, 0xFF]))
        .map(|i| i + first_search)
    {
        if let Some(eoi) = b[soi..limit].windows(2).rposition(|w| w == [0xFF, 0xD9]) {
            if eoi > 1024 {
                info.embedded_jpeg_bytes = Some((eoi + 2) as u64);
            }
        }
    }
    info.sheet_from_file_name = sheet_from_file_name(file_name);
    info
}

// ----------------------------------------------------------------- summary

/// A role-to-material pairing derived from a decoded wall type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DefaultMaterial {
    /// `exterior wall finish`, `exterior wall framing`, `interior wall
    /// finish`, ...
    pub role: String,
    pub wall_type: String,
    pub material: String,
}

/// Everything the Phase C decoders read from one template. All fields default
/// to empty so a summary of a file without the structure is valid.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TemplateSummary {
    pub file_name: String,
    pub kind: Option<TemplateKind>,
    /// Plausible objects found in the body.
    pub object_count: usize,
    pub materials: Vec<TemplateMaterial>,
    pub wall_types: Vec<TemplateWallType>,
    pub text_styles: Vec<TemplateTextStyle>,
    pub rich_text_defaults: Vec<TemplateRichText>,
    pub dimension_defaults: Vec<TemplateDimensionDefaults>,
    pub default_heights: DefaultHeights,
    pub paper_sizes: Vec<PaperSize>,
    /// Roof, floor finish and similar defaults are not located; this lists the
    /// wall-derived ones.
    pub default_materials: Vec<DefaultMaterial>,
    /// Only for layouts.
    pub layout: Option<LayoutInfo>,
}

impl TemplateSummary {
    pub fn wall_type(&self, name: &str) -> Option<&TemplateWallType> {
        self.wall_types.iter().find(|w| w.name == name)
    }

    pub fn text_style(&self, name: &str) -> Option<&TemplateTextStyle> {
        self.text_styles.iter().find(|s| s.name == name)
    }

    pub fn dimension_set(&self, name: &str) -> Option<&TemplateDimensionDefaults> {
        self.dimension_defaults.iter().find(|d| d.name == name)
    }

    pub fn material(&self, id: u32) -> Option<&TemplateMaterial> {
        self.materials.iter().find(|m| m.id == id)
    }
}

/// Wall types Daniel's setup names as the exterior and interior defaults
/// (`docs/daniel-chief-setup.md`); the roles below read their layers.
const DEFAULT_EXTERIOR_WALL: &str = "Stucco-6";
const DEFAULT_INTERIOR_WALL: &str = "Interior-4";

fn derive_default_materials(walls: &[TemplateWallType]) -> Vec<DefaultMaterial> {
    let mut out = Vec::new();
    let mut push = |role: &str, wall: &str, material: Option<String>| {
        if let Some(material) = material {
            out.push(DefaultMaterial {
                role: role.into(),
                wall_type: wall.into(),
                material,
            });
        }
    };
    if let Some(w) = walls.iter().find(|w| w.name == DEFAULT_EXTERIOR_WALL) {
        push(
            "exterior wall finish",
            &w.name,
            w.layers.first().map(|l| l.0.clone()),
        );
        push(
            "exterior wall framing",
            &w.name,
            w.layers.iter().find(|l| l.2).map(|l| l.0.clone()),
        );
        push(
            "exterior wall interior finish",
            &w.name,
            w.layers.last().map(|l| l.0.clone()),
        );
    }
    if let Some(w) = walls.iter().find(|w| w.name == DEFAULT_INTERIOR_WALL) {
        push(
            "interior wall finish",
            &w.name,
            w.layers.first().map(|l| l.0.clone()),
        );
        push(
            "interior wall framing",
            &w.name,
            w.layers.iter().find(|l| l.2).map(|l| l.0.clone()),
        );
    }
    out
}

/// Runs every Phase C decoder over a template. `strings` are the Phase A
/// strings (for paper sizes and the layout).
pub fn summarize_bytes(
    bytes: &[u8],
    strings: &[(u64, String)],
    kind: TemplateKind,
    file_name: &str,
    resource_start: usize,
) -> TemplateSummary {
    let objs = find_objects(bytes);
    let materials = decode_materials(bytes, &objs);
    let wall_types = decode_wall_types(bytes, &objs, &materials);
    let text_styles = decode_text_styles(bytes, &objs);
    let dimension_defaults = decode_dimension_defaults_all(bytes, &objs, &text_styles);
    let default_materials = derive_default_materials(&wall_types);
    TemplateSummary {
        file_name: file_name.to_string(),
        kind: Some(kind),
        object_count: objs.len(),
        rich_text_defaults: decode_rich_text_defaults(bytes, &objs),
        default_heights: decode_default_heights(bytes, &objs),
        paper_sizes: decode_paper_sizes(bytes, strings),
        layout: (kind == TemplateKind::Layout)
            .then(|| decode_layout(bytes, strings, file_name, resource_start)),
        materials,
        wall_types,
        text_styles,
        dimension_defaults,
        default_materials,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::testutil::{build_template, put_str};

    /// `[01] CD AB class version size payload`.
    fn obj(class: u8, version: u8, payload: &[u8]) -> Vec<u8> {
        let mut v = vec![0x01, 0xCD, 0xAB, class, version];
        v.extend_from_slice(&((payload.len() + 4) as u32).to_le_bytes());
        v.extend_from_slice(payload);
        v
    }

    /// A string as stored in object payloads: `u32` length, bytes, NUL.
    fn cstr(s: &str) -> Vec<u8> {
        let mut v = (s.len() as u32).to_le_bytes().to_vec();
        v.extend_from_slice(s.as_bytes());
        v.push(0);
        v
    }

    fn f64b(v: f64) -> [u8; 8] {
        v.to_le_bytes()
    }

    /// A material object: padding, `[id u16][hi u16][rgb][FF 88 13 00 00]`.
    fn material_obj(name: &str, id: u16, color: [u8; 3]) -> Vec<u8> {
        let mut p = vec![0u8; 24];
        p.extend(cstr(name)); // string + NUL
        p.extend_from_slice(&id.to_le_bytes());
        p.extend_from_slice(&[0, 0]);
        p.extend_from_slice(&color);
        p.extend_from_slice(&MATERIAL_ANCHOR);
        p.extend_from_slice(&[0u8; 12]);
        obj(class::MATERIAL, 0, &p)
    }

    /// One 518-byte wall layer record.
    fn layer_record(thickness: f64, id: u32, main: bool, framing: bool, gap: bool) -> Vec<u8> {
        let mut r = vec![0u8; WALL_LAYER_STRIDE];
        r[..8].copy_from_slice(&f64b(thickness));
        r[8..12].copy_from_slice(&id.to_le_bytes());
        r[0x15] = u8::from(main);
        r[0x16] = u8::from(framing);
        r[0x17] = u8::from(gap);
        let spacing = if main { 16.0 } else { 96.0 };
        r[0x18..0x20].copy_from_slice(&f64b(spacing));
        r
    }

    /// A wall type: name, NUL, count, records (the last is the sentinel).
    fn wall_obj(name: &str, layers: &[(f64, u32, bool, bool, bool)]) -> Vec<u8> {
        let mut p = cstr(name);
        p.extend_from_slice(&((layers.len() + 1) as u32).to_le_bytes());
        for (t, id, m, f, g) in layers {
            p.extend(layer_record(*t, *id, *m, *f, *g));
        }
        p.extend(layer_record(0.0, 35, false, false, false));
        p.extend_from_slice(&[0u8; 20]);
        obj(class::WALL_TYPE, 0, &p)
    }

    fn text_style_obj(name: &str, height: f64, font: &str, style: &str, guid: [u8; 16]) -> Vec<u8> {
        let mut p = f64b(height).to_vec();
        p.extend(cstr(font));
        p.extend(cstr(style));
        // The 12 flag bytes sit between the NUL of the style string and the
        // name, but `cstr` already wrote that NUL.
        p.extend_from_slice(&[0x10, 0x01, 0, 1, 1, 0, 0, 0, 0xFF, 1, 0, 1]);
        p.extend(cstr(name));
        p.push(0); // flag byte before the GUID
        p.extend_from_slice(&guid);
        obj(class::TEXT_STYLE, 0, &p)
    }

    #[test]
    fn finds_objects_and_rejects_bad_sizes() {
        let mut b = vec![0u8; 3];
        b.extend(obj(57, 0, &[1, 2, 3, 4]));
        b.extend(obj(215, 1, &[9; 10]));
        // A marker whose size runs past the end.
        b.extend_from_slice(&[0x01, 0xCD, 0xAB, 169, 0, 0xFF, 0xFF, 0xFF, 0x7F]);
        let objs = find_objects(&b);
        assert_eq!(objs.len(), 2);
        assert_eq!((objs[0].class, objs[0].version, objs[0].flag), (57, 0, 1));
        assert_eq!(objs[0].payload(), objs[0].marker + 8);
        assert_eq!(objs[1].class, 215);
        assert_eq!(objs[1].end - objs[1].payload(), 10);
        // Too short to hold a marker: no panic.
        assert!(find_objects(&[0xCD]).is_empty());
        assert!(find_objects(&[]).is_empty());
    }

    #[test]
    fn decodes_materials_named_and_unnamed() {
        let mut b = vec![0u8; 8];
        b.extend(material_obj("Drywall", 35, [0xEF, 0xE9, 0xDA]));
        b.extend(material_obj("Fir Stud 16\" OC, Teal", 169, [1, 2, 3]));
        // Unnamed: the id sits right after the padding with no string.
        let mut p = vec![0u8; 24];
        p.push(0);
        p.extend_from_slice(&70u16.to_le_bytes());
        p.extend_from_slice(&[0, 0, 0xB4, 0x7C, 0x28]);
        p.extend_from_slice(&MATERIAL_ANCHOR);
        b.extend(obj(class::MATERIAL, 0, &p));
        let objs = find_objects(&b);
        let m = decode_materials(&b, &objs);
        assert_eq!(m.len(), 3);
        assert_eq!(
            (m[0].id, m[0].name.as_str(), m[0].color),
            (35, "Drywall", [0xEF, 0xE9, 0xDA])
        );
        assert_eq!(
            (m[1].id, m[1].name.as_str()),
            (169, "Fir Stud 16\" OC, Teal")
        );
        assert_eq!(
            (m[2].id, m[2].name.as_str(), m[2].color),
            (70, "", [0xB4, 0x7C, 0x28])
        );
    }

    #[test]
    fn decodes_wall_type_layers_and_flags() {
        let mut b = Vec::new();
        b.extend(material_obj("Sand Finish - Eggshell", 156, [1, 1, 1]));
        b.extend(material_obj("Fir Stud 16\" OC, Teal", 169, [2, 2, 2]));
        b.extend(material_obj("Drywall", 35, [3, 3, 3]));
        b.extend(wall_obj(
            "Stucco-6",
            &[
                (1.125, 156, false, false, false),
                (5.5, 169, true, true, false),
                (0.5, 35, false, false, false),
                (1.0, 999, false, false, true),
            ],
        ));
        let objs = find_objects(&b);
        let mats = decode_materials(&b, &objs);
        let w = decode_wall_types(&b, &objs, &mats);
        assert_eq!(w.len(), 1);
        let w = &w[0];
        assert_eq!(w.name, "Stucco-6");
        assert_eq!(w.layers.len(), 4, "sentinel record dropped");
        assert_eq!(
            w.layers[0],
            ("Sand Finish - Eggshell".to_string(), 1.125, false)
        );
        assert_eq!(
            w.layers[1],
            ("Fir Stud 16\" OC, Teal".to_string(), 5.5, true)
        );
        assert_eq!(w.layers[2].0, "Drywall");
        // Unknown material id falls back to a label.
        assert_eq!(w.layers[3].0, "material #999");
        assert!(w.layer_details[1].is_framing && w.layer_details[3].is_gap);
        assert_eq!(w.layer_details[1].spacing_in, 16.0);
        assert_eq!(w.layer_details[2].spacing_in, 96.0);
        assert!((w.total_thickness_in - 8.125).abs() < 1e-9);
    }

    #[test]
    fn wall_type_rejects_bad_counts_and_truncation() {
        // Count larger than the object holds.
        let mut p = cstr("Bad");
        p.extend_from_slice(&5u32.to_le_bytes());
        p.extend(layer_record(1.0, 1, false, false, false));
        let b = obj(class::WALL_TYPE, 0, &p);
        let objs = find_objects(&b);
        assert!(decode_wall_types(&b, &objs, &[]).is_empty());
        // Zero count.
        let mut p = cstr("Zero");
        p.extend_from_slice(&0u32.to_le_bytes());
        let b = obj(class::WALL_TYPE, 0, &p);
        assert!(decode_wall_types(&b, &find_objects(&b), &[]).is_empty());
        // Only the sentinel: a type with no layers.
        let b = wall_obj("Empty", &[]);
        let w = decode_wall_types(&b, &find_objects(&b), &[]);
        assert_eq!(w.len(), 1);
        assert!(w[0].layers.is_empty());
    }

    #[test]
    fn decodes_text_styles() {
        let g1 = [0xAA; 16];
        let g2 = [0xBB; 16];
        let mut b = text_style_obj("1/4\" Text Style", 4.5, "Avenir", "Book", g1);
        b.extend(text_style_obj(
            "Room Label Style",
            8.0,
            "Avenir",
            "Heavy",
            g2,
        ));
        b.extend(text_style_obj("Slanted", 3.0, "Arial", "Italic", [1; 16]));
        let styles = decode_text_styles(&b, &find_objects(&b));
        assert_eq!(styles.len(), 3);
        assert_eq!(styles[0].name, "1/4\" Text Style");
        assert_eq!(
            (styles[0].font.as_str(), styles[0].height_in),
            ("Avenir", 4.5)
        );
        assert!(!styles[0].bold && !styles[0].italic);
        assert_eq!(styles[0].guid, "aa".repeat(16));
        assert!(styles[1].bold);
        assert!(styles[2].italic && !styles[2].bold);
        assert_eq!(styles[0].color, None);
    }

    #[test]
    fn decodes_rich_text_defaults_with_and_without_style() {
        let mk = |name: &str, rest: &[&str]| {
            let mut p = cstr(name);
            for s in rest {
                p.extend(cstr(s));
            }
            obj(class::RICH_TEXT_DEFAULTS, 0, &p)
        };
        let mut b = mk(
            "1/4\" Scale Rich Text Defaults",
            &[
                "Avenir",
                "Book",
                "0.166666666666667",
                "004080",
                "ffffff",
                "0002",
            ],
        );
        b.extend(mk(
            "Framing Rich Text Defaults, Roof",
            &["Chief Blueprint", "0.25", "000000", "ffffff"],
        ));
        // A repeat of the first name with different values is ignored.
        b.extend(mk(
            "1/4\" Scale Rich Text Defaults",
            &["Avenir", "Heavy", "1.5", "000000", "ffffff"],
        ));
        // Not a rich text object.
        b.extend(mk(
            "Something Else",
            &["Avenir", "Book", "1", "000000", "ffffff"],
        ));
        let r = decode_rich_text_defaults(&b, &find_objects(&b));
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].font_style, "Book");
        assert!((r[0].size_in - 1.0 / 6.0).abs() < 1e-9);
        assert_eq!(
            (r[0].color, r[0].background),
            ([0, 0x40, 0x80], [255, 255, 255])
        );
        assert_eq!(
            (r[1].font.as_str(), r[1].font_style.as_str()),
            ("Chief Blueprint", "")
        );
        assert_eq!(r[1].size_in, 0.25);
    }

    /// A dimension set with the fields at their documented offsets.
    fn dim_obj(name: &str, scale: f64, guid: [u8; 16]) -> Vec<u8> {
        let name_bytes = cstr(name);
        let after = name_bytes.len(); // 4 + len + NUL
        let font_pos = after + (0x1c1 - 0x22);
        let mut p = vec![0u8; font_pos + 0x200];
        p[..after].copy_from_slice(&name_bytes);
        let put_f = |p: &mut Vec<u8>, at: usize, v: f64| p[at..at + 8].copy_from_slice(&f64b(v));
        let put_u =
            |p: &mut Vec<u8>, at: usize, v: u32| p[at..at + 4].copy_from_slice(&v.to_le_bytes());
        put_f(&mut p, after + DIM_ARROW, 2.25 * scale);
        put_u(&mut p, after + DIM_DECIMALS, 4);
        put_u(&mut p, after + DIM_FRACTION, 8);
        put_u(&mut p, after + DIM_FRACTION_PCT, 60);
        // `ft` and `in` format strings.
        let ft = cstr("ft");
        let inn = cstr("in");
        p[after + 0x80..after + 0x80 + ft.len()].copy_from_slice(&ft);
        p[after + 0x90..after + 0x90 + inn.len()].copy_from_slice(&inn);
        // Text height, font, style GUID, extension and separation values.
        put_f(&mut p, font_pos - 8, 4.5 * scale);
        let font = cstr("Avenir");
        p[font_pos..font_pos + font.len()].copy_from_slice(&font);
        p[font_pos + DIM_STYLE_GUID..font_pos + DIM_STYLE_GUID + 16].copy_from_slice(&guid);
        put_f(&mut p, font_pos + DIM_EXT_AWAY, 3.0 * scale);
        put_f(&mut p, font_pos + DIM_EXT_GAP, 3.0 * scale);
        put_f(&mut p, font_pos + DIM_EXT_TOWARDS, 3.0);
        put_f(&mut p, font_pos + DIM_EXT_PROXIMITY, 12.0);
        put_f(&mut p, font_pos + DIM_LINE_SEPARATION, 18.0 * scale);
        put_f(&mut p, font_pos + DIM_REACH, 24.0);
        put_f(&mut p, font_pos + DIM_BASELINE, 18.0 * scale);
        put_f(&mut p, font_pos + DIM_FIRST_OFFSET, 32.0);
        put_f(&mut p, font_pos + DIM_EXT_REACH, 240.0);
        obj(class::DIMENSION_DEFAULTS, 0, &p)
    }

    #[test]
    fn decodes_dimension_defaults_and_resolves_text_style() {
        let guid = [0x37; 16];
        let mut b = text_style_obj("1/4\" Text Style", 4.5, "Avenir", "Book", guid);
        b.extend(dim_obj("1/4\" Scale Dimension Defaults", 1.0, guid));
        b.extend(dim_obj("Orphan Dimension Defaults", 0.5, [9; 16]));
        let objs = find_objects(&b);
        let styles = decode_text_styles(&b, &objs);
        let dims = decode_dimension_defaults_all(&b, &objs, &styles);
        assert_eq!(dims.len(), 2);
        let d = &dims[0];
        assert_eq!(d.name, "1/4\" Scale Dimension Defaults");
        assert_eq!(d.text_style.as_deref(), Some("1/4\" Text Style"));
        assert_eq!(d.text_font.as_deref(), Some("Avenir"));
        assert_eq!(d.text_height_in, Some(4.5));
        assert_eq!(d.arrow_size_in, Some(2.25));
        assert_eq!(
            (
                d.extension_length_away_in,
                d.extension_fixed_gap_in,
                d.extension_length_towards_in
            ),
            (Some(3.0), Some(3.0), Some(3.0))
        );
        assert_eq!(d.extension_proximity_in, Some(12.0));
        assert_eq!(
            (
                d.line_separation_in,
                d.reach_in,
                d.baseline_separation_in,
                d.first_line_offset_in
            ),
            (Some(18.0), Some(24.0), Some(18.0), Some(32.0))
        );
        assert_eq!(d.exterior_reach_in, Some(240.0));
        assert_eq!(
            (
                d.decimal_places,
                d.smallest_fraction,
                d.fraction_text_size_pct
            ),
            (Some(4), Some(8), Some(60))
        );
        assert_eq!(d.unit_labels, vec!["ft", "in"]);
        // Scaled set: values follow, the style does not resolve.
        assert_eq!(dims[1].text_style, None);
        assert_eq!(dims[1].arrow_size_in, Some(1.125));
        assert_eq!(dims[1].extension_length_away_in, Some(1.5));
    }

    #[test]
    fn dimension_defaults_reject_implausible_format_values() {
        let mut b = dim_obj("Odd Dimension Defaults", 1.0, [0; 16]);
        // Smash the fraction block: 100663296 text size, fraction 3.
        let name_len = cstr("Odd Dimension Defaults").len();
        let payload = find_objects(&b)[0].payload();
        let after = payload + name_len;
        b[after + DIM_FRACTION..after + DIM_FRACTION + 4].copy_from_slice(&3u32.to_le_bytes());
        b[after + DIM_FRACTION_PCT..after + DIM_FRACTION_PCT + 4]
            .copy_from_slice(&0x0600_0000u32.to_le_bytes());
        let d = decode_dimension_defaults_all(&b, &find_objects(&b), &[]);
        assert_eq!(d[0].smallest_fraction, None);
        assert_eq!(d[0].fraction_text_size_pct, None);
        assert_eq!(d[0].decimal_places, Some(4));
    }

    fn room_obj(name: &str, height: f64, second: f64) -> Vec<u8> {
        let mut p = vec![0u8; 0x300];
        let n = cstr(name);
        p[0x100..0x100 + n.len()].copy_from_slice(&n);
        p[0x270..0x278].copy_from_slice(&f64b(height));
        p[0x279..0x281].copy_from_slice(&f64b(second));
        obj(class::ROOM_TYPE, 0, &p)
    }

    #[test]
    fn default_height_is_the_majority_value() {
        let mut b = room_obj("Bedroom", 109.125, 23.25);
        b.extend(room_obj("Closet", 109.125, 23.25));
        b.extend(room_obj("Attic", 120.0, 23.25));
        // Too small to hold a height.
        b.extend(obj(class::ROOM_TYPE, 0, &[0u8; 40]));
        let h = decode_default_heights(&b, &find_objects(&b));
        assert_eq!(h.room_type_height_in, Some(109.125));
        assert_eq!((h.room_types_with_height, h.room_types_agreeing), (3, 2));
        assert!(!h.floor_foundation_rough_ceiling_stem_found);
        assert_eq!(decode_default_heights(&[], &[]).room_type_height_in, None);
    }

    #[test]
    fn decodes_paper_sizes_plan_and_layout_shapes() {
        let mut body = vec![0u8; 8];
        // Plan shape: margins, printable area, paper size, then the name.
        for v in [1.0 / 6.0, 1.0 / 6.0, 23.8333333, 17.8333333, 18.0, 24.0] {
            body.extend_from_slice(&f64b(v));
        }
        body.push(0);
        put_str(&mut body, "ARCH C  (18\" x 24\")");
        // Layout shape: only the paper pair (17x11, 11x17) before the name.
        body.extend_from_slice(&[0u8; 8]);
        for v in [17.0, 11.0, 11.0, 17.0] {
            body.extend_from_slice(&f64b(v));
        }
        body.push(0);
        put_str(&mut body, "ANSI B  (11\" x 17\")");
        // A sheet name with no numbers before it is skipped.
        put_str(&mut body, "Junk (1\" x 2\")");
        let bytes = build_template(&body, &[]);
        let strings = crate::scan::scan_bytes(&bytes, None).unwrap().strings;
        let p = decode_paper_sizes(&bytes, &strings);
        assert_eq!(p.len(), 2, "{p:?}");
        assert_eq!(p[0].name, "ARCH C (18\" x 24\")");
        assert_eq!((p[0].width_in, p[0].height_in), (18.0, 24.0));
        assert!((p[0].printable_width_in.unwrap() - 23.8333333).abs() < 1e-6);
        assert_eq!((p[1].width_in, p[1].height_in), (11.0, 17.0));
        assert_eq!(p[1].printable_width_in, None);
    }

    #[test]
    fn file_name_sheet_size() {
        assert_eq!(
            sheet_from_file_name("18x24 PRESENTATION LAYOUT TEMPLATE.layout"),
            Some((18.0, 24.0))
        );
        assert_eq!(sheet_from_file_name("ArchD 24x36 Layout.layout"), None);
        assert_eq!(sheet_from_file_name("Letter.layout"), None);
        assert_eq!(sheet_from_file_name("x17 Working Template.plan"), None);
    }

    #[test]
    fn summarizes_a_synthetic_plan_and_layout() {
        let g = [0x11; 16];
        let mut body = Vec::new();
        body.extend(material_obj("Drywall", 35, [9, 9, 9]));
        body.extend(material_obj("Fir Stud 16\" OC, Yellow", 168, [9, 9, 9]));
        body.extend(wall_obj(
            "Interior-4",
            &[
                (0.5, 35, false, false, false),
                (3.5, 168, true, false, false),
                (0.5, 35, false, false, false),
            ],
        ));
        body.extend(text_style_obj(
            "Default Text Style",
            4.5,
            "Avenir",
            "Book",
            g,
        ));
        body.extend(dim_obj("1/4\" Scale Dimension Defaults", 1.0, g));
        body.extend(room_obj("Bedroom", 109.125, 23.25));
        put_str(&mut body, "Page Template");
        put_str(&mut body, "Project");
        put_str(&mut body, "Client");
        put_str(&mut body, "%room.name%");
        put_str(&mut body, "Floor Finish - %floor.name%");
        let bytes = build_template(&body, &[]);
        let scan = crate::scan::scan_bytes(&bytes, Some(TemplateKind::Layout)).unwrap();
        let s = summarize_bytes(
            &bytes,
            &scan.strings,
            TemplateKind::Layout,
            "18x24 X.layout",
            scan.resource_table_start as usize,
        );
        assert_eq!(s.kind, Some(TemplateKind::Layout));
        assert_eq!(s.materials.len(), 2);
        let w = s.wall_type("Interior-4").unwrap();
        assert!((w.total_thickness_in - 4.5).abs() < 1e-9);
        assert_eq!(s.text_style("Default Text Style").unwrap().font, "Avenir");
        assert_eq!(
            s.dimension_set("1/4\" Scale Dimension Defaults")
                .unwrap()
                .text_style
                .as_deref(),
            Some("Default Text Style")
        );
        assert_eq!(s.default_heights.room_type_height_in, Some(109.125));
        assert_eq!(s.material(35).unwrap().name, "Drywall");
        // Derived roles come from Interior-4 only (no Stucco-6 here).
        assert_eq!(s.default_materials.len(), 2);
        assert_eq!(s.default_materials[1].material, "Fir Stud 16\" OC, Yellow");
        let l = s.layout.as_ref().unwrap();
        assert!(l.page_template_found);
        assert_eq!(l.page_count, Some(0));
        assert_eq!(l.sheet_from_file_name, Some((18.0, 24.0)));
        let macros: Vec<&str> = l
            .title_block_macros
            .iter()
            .map(|(m, _)| m.as_str())
            .collect();
        assert_eq!(macros, vec!["%room.name%", "Floor Finish - %floor.name%"]);
        let fields: Vec<&str> = l
            .project_info_fields
            .iter()
            .map(|(m, _)| m.as_str())
            .collect();
        assert_eq!(fields, vec!["Project", "Client"]);
        // A plan has no layout block.
        let plan = summarize_bytes(
            &bytes,
            &scan.strings,
            TemplateKind::Plan,
            "p.plan",
            bytes.len(),
        );
        assert!(plan.layout.is_none());
    }

    #[test]
    fn build_inventory_fills_the_summary_and_redaction_keeps_names() {
        let mut body = Vec::new();
        body.extend(material_obj("Drywall", 35, [9, 9, 9]));
        body.extend(wall_obj("Interior-4", &[(0.5, 35, true, false, false)]));
        put_str(&mut body, "Page Template");
        let bytes = build_template(&body, &[]);
        let dir =
            std::env::temp_dir().join(format!("plan-chiefplan-decode-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("18x24 TEST.layout");
        std::fs::write(&path, &bytes).unwrap();

        let inv = crate::build_inventory(&path).unwrap();
        assert_eq!(inv.summary.file_name, "18x24 TEST.layout");
        assert_eq!(inv.summary.kind, Some(TemplateKind::Layout));
        assert_eq!(inv.summary.wall_types.len(), 1);
        assert_eq!(inv.summary.wall_types[0].layers[0].0, "Drywall");
        assert_eq!(crate::summarize(&path).unwrap(), inv.summary);

        // The JSON writer keeps the material table for non-default files out.
        let out = dir.join("out.json");
        assert_eq!(crate::write_inventory_json(&dir, &out).unwrap(), 1);
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        let summary = &json["files"][0]["inventory"]["summary"];
        assert_eq!(summary["wall_types"][0]["name"], "Interior-4");
        assert_eq!(summary["materials"].as_array().map(Vec::len), Some(0));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn junk_does_not_panic() {
        // Random-looking bytes with embedded markers of every class.
        let mut b = Vec::new();
        for class in [57u8, 215, 139, 129, 64, 23] {
            b.extend_from_slice(&[1, 0xCD, 0xAB, class, 0, 12, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF]);
            b.extend_from_slice(&[0x7F; 32]);
        }
        let objs = find_objects(&b);
        assert!(!objs.is_empty());
        let m = decode_materials(&b, &objs);
        let _ = decode_wall_types(&b, &objs, &m);
        let t = decode_text_styles(&b, &objs);
        let _ = decode_dimension_defaults_all(&b, &objs, &t);
        let _ = decode_rich_text_defaults(&b, &objs);
        let _ = decode_default_heights(&b, &objs);
    }
}
