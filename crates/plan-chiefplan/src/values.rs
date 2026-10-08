//! Phase B (best effort): decode per-layer values and wall layer stacks.
//!
//! Findings (details and offsets in the crate README):
//!
//! * A layer record is a length-prefixed name with a fixed frame around it.
//!   Before the length prefix (`o` = offset of the prefix):
//!   `o-8..o-6` = `E0 3F` (tail of an `f64` 0.5), `o-6` = flags byte
//!   (bit 0 display, bit 1 lock), `o-5` = a per-layer constant byte,
//!   `o-4..o` = an `i32` layer id (-140..-1 and 0..214 in the default plan).
//!   After the name (`e` = end of name): `e` = 0, `e+1..e+4` = RGB colour,
//!   `e+4` = `FF` (alpha), `e+5..e+7` = line weight (`u16`, 1/100 mm),
//!   `e+84..e+88` = a second RGBA value.
//! * Every layer set stores its own copy of ~354 layer records, so the same
//!   layer name appears once per set (47 times in the default plan template).
//! * Wall layer stacks were **not** found next to wall type names in any
//!   sample: the names sit in repeated pick lists. The search below stays in
//!   place so a template that does store them name-adjacent is picked up.

use crate::classify::is_wall_type_name;
use crate::scan::TemplateScan;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Minimum layer records for a run of records to count as a layer set.
const MIN_LAYERS_PER_SET: usize = 20;
/// Largest byte gap between two consecutive records of the same set (a set's
/// table is contiguous; sets are separated by tens of kilobytes).
const MAX_RECORD_GAP: u64 = 30_000;
/// How far before its first layer record a set's name may appear.
const MAX_NAME_DISTANCE: u64 = 60_000;
/// A string starting this close after a wall type name means a list entry.
const LIST_GAP: u64 = 8;
/// Bytes after a wall type name searched for a thickness run.
const WALL_WINDOW: usize = 256;

/// One decoded layer record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerValues {
    pub name: String,
    /// Offset of the name's length prefix.
    pub offset: u64,
    /// Per-layer id (negative and non-negative ranges both occur).
    pub id: i32,
    /// Flags bit 0.
    pub display: bool,
    /// Flags bit 1.
    pub locked: bool,
    /// The raw flags byte.
    pub flags: u8,
    /// The raw per-layer constant byte before the flags (meaning unknown).
    pub class_byte: u8,
    pub color: [u8; 3],
    /// Plotted line weight, hundredths of a millimetre.
    pub line_weight: u16,
    /// The second colour-like value at `e+84` (RGBA; meaning unknown).
    pub secondary_color: [u8; 4],
}

/// The layer table of one layer set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerSetData {
    /// `Name Layer Set` as found before the table, or `(unnamed set N)`.
    pub name: String,
    pub named: bool,
    /// Offset of the first record.
    pub offset: u64,
    pub layers: Vec<LayerValues>,
}

impl LayerSetData {
    /// Names of the layers whose display flag is on, in table order.
    pub fn visible_layers(&self) -> Vec<String> {
        self.layers
            .iter()
            .filter(|l| l.display)
            .map(|l| l.name.clone())
            .collect()
    }
}

/// A run of layer thicknesses found next to a wall type name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WallStack {
    pub name: String,
    pub offset: u64,
    pub thicknesses: Vec<f64>,
    pub total: f64,
    /// 8 for `f64`, 4 for `f32`.
    pub float_bytes: u8,
}

/// Decoded values of one file.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ValueReport {
    pub layer_sets: Vec<LayerSetData>,
    pub wall_stacks: Vec<WallStack>,
    /// Records matching the id frame (named like layers).
    pub candidate_records: usize,
    /// Of those, records that also matched the full post-name frame.
    pub structural_records: usize,
}

/// The fixed frame before a layer name: `E0 3F` (tail of an `f64` 0.5), two
/// flag bytes, then a small `i32` layer id.
fn frame_before(bytes: &[u8], o: usize) -> bool {
    o >= 8 && bytes[o - 8..o - 6] == [0xE0, 0x3F] && (-4096..4096).contains(&le_i32(bytes, o - 4))
}

fn le_i32(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().expect("4 bytes"))
}

/// Decodes the record whose name length prefix is at `o`, if the frame fits.
pub(crate) fn decode_layer_record(bytes: &[u8], o: usize, name: &str) -> Option<LayerValues> {
    // One byte per character (MacRoman symbols decode to single chars).
    let name_len = name.chars().count();
    let end = o.checked_add(4 + name_len)?;
    if o < 8 || end + 88 > bytes.len() {
        return None;
    }
    if !frame_before(bytes, o) {
        return None;
    }
    if bytes[end] != 0 || bytes[end + 4] != 0xFF {
        return None;
    }
    let flags = bytes[o - 6];
    Some(LayerValues {
        name: name.to_string(),
        offset: o as u64,
        id: le_i32(bytes, o - 4),
        display: flags & 1 != 0,
        locked: flags & 2 != 0,
        flags,
        class_byte: bytes[o - 5],
        color: [bytes[end + 1], bytes[end + 2], bytes[end + 3]],
        line_weight: u16::from_le_bytes([bytes[end + 5], bytes[end + 6]]),
        secondary_color: [
            bytes[end + 84],
            bytes[end + 85],
            bytes[end + 86],
            bytes[end + 87],
        ],
    })
}

/// Decodes layer sets and wall stacks from `bytes` using the strings already
/// found by [`crate::scan::scan_bytes`].
pub fn decode(bytes: &[u8], scan: &TemplateScan) -> ValueReport {
    let mut report = ValueReport::default();

    // Layer records, in file order.
    let mut records: Vec<LayerValues> = Vec::new();
    for (off, raw) in &scan.strings {
        let o = *off as usize;
        let name = raw.trim();
        if name.chars().count() < 2 || name.ends_with("Layer Set") {
            continue;
        }
        if o + 4 > bytes.len() || !frame_before(bytes, o) {
            continue;
        }
        report.candidate_records += 1;
        if let Some(rec) = decode_layer_record(bytes, o, raw) {
            report.structural_records += 1;
            records.push(LayerValues {
                name: name.to_string(),
                ..rec
            });
        }
    }

    // Layer-set names: strings ending in "Layer Set" that are not themselves
    // layer-style records.
    let record_offsets: HashSet<u64> = records.iter().map(|r| r.offset).collect();
    let set_names: Vec<(u64, &str)> = scan
        .strings
        .iter()
        .filter(|(o, s)| s.trim().ends_with("Layer Set") && !record_offsets.contains(o))
        .map(|(o, s)| (*o, s.trim()))
        .collect();

    // Group consecutive records into sets.
    // A set ends at a large byte gap or when a layer name repeats (the next
    // set's table starts again with the same layers).
    let mut groups: Vec<Vec<LayerValues>> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for rec in records {
        let continues = groups.last().is_some_and(|g| {
            rec.offset - g.last().map_or(0, |l| l.offset) <= MAX_RECORD_GAP
                && !seen.contains(&rec.name)
        });
        if continues {
            seen.insert(rec.name.clone());
            groups.last_mut().expect("checked above").push(rec);
        } else {
            seen.clear();
            seen.insert(rec.name.clone());
            groups.push(vec![rec]);
        }
    }
    groups.retain(|g| g.len() >= MIN_LAYERS_PER_SET);

    let mut prev_end = 0u64;
    let mut unnamed = 0;
    for g in groups {
        let first = g[0].offset;
        let name = set_names
            .iter()
            .rfind(|(o, _)| *o > prev_end && *o < first && first - *o <= MAX_NAME_DISTANCE)
            .map(|(_, n)| (*n).to_string());
        prev_end = g.last().map_or(first, |l| l.offset);
        let named = name.is_some();
        let name = name.unwrap_or_else(|| {
            unnamed += 1;
            format!("(unnamed set {unnamed})")
        });
        report.layer_sets.push(LayerSetData {
            name,
            named,
            offset: first,
            layers: g,
        });
    }

    // Wall stacks next to wall type names. A name followed within a few
    // bytes by another string is a pick-list entry, not a definition.
    let string_offsets: HashSet<u64> = scan.strings.iter().map(|(o, _)| *o).collect();
    let mut seen = HashSet::new();
    for (off, raw) in &scan.strings {
        let name = raw.trim();
        if !is_wall_type_name(name) || !seen.insert(name.to_string()) {
            continue;
        }
        let end = *off + 4 + raw.chars().count() as u64;
        if (end..end + LIST_GAP).any(|o| string_offsets.contains(&o)) {
            continue;
        }
        if let Some(stack) = find_wall_stack(bytes, end as usize, name, *off) {
            report.wall_stacks.push(stack);
        }
    }
    report
}

fn valid_thickness(v: f64) -> bool {
    v.is_finite()
        && (1.0 / 32.0..=24.0).contains(&v)
        && ((v * 32.0).round() - v * 32.0).abs() < 1e-9
}

fn plausible_total(t: f64) -> bool {
    (2.0..=30.0).contains(&t) && ((t * 16.0).round() - t * 16.0).abs() < 1e-9
}

/// Searches the bytes after a wall type name for two or more (not all equal)
/// consecutive
/// `f64` (then `f32`) layer thicknesses (multiples of 1/32") whose sum is a
/// plausible wall thickness (a multiple of 1/16" between 2" and 30").
pub fn find_wall_stack(
    bytes: &[u8],
    name_end: usize,
    name: &str,
    offset: u64,
) -> Option<WallStack> {
    let window = &bytes[name_end.min(bytes.len())..(name_end + WALL_WINDOW).min(bytes.len())];
    for width in [8usize, 4] {
        for start in 0..window.len().saturating_sub(width * 2 - 1) {
            let mut run = Vec::new();
            let mut p = start;
            while p + width <= window.len() {
                let v = if width == 8 {
                    f64::from_le_bytes(window[p..p + 8].try_into().expect("8 bytes"))
                } else {
                    f64::from(f32::from_le_bytes(
                        window[p..p + 4].try_into().expect("4 bytes"),
                    ))
                };
                if !valid_thickness(v) {
                    break;
                }
                run.push(v);
                p += width;
            }
            let total: f64 = run.iter().sum();
            // Repeated identical values are structure bytes (`00 00 80 3F`
            // runs, 2.0 padding), not a layer stack.
            let varied = run.iter().any(|&v| (v - run[0]).abs() > 1e-9);
            if run.len() >= 2 && varied && plausible_total(total) {
                return Some(WallStack {
                    name: name.to_string(),
                    offset,
                    thicknesses: run,
                    total,
                    float_bytes: width as u8,
                });
            }
        }
    }
    None
}

// ----- calibration across files -----

/// How much to trust a decoded field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Confidence {
    None,
    Low,
    Medium,
    High,
}

/// Evidence for one field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldReport {
    pub field: String,
    pub location: String,
    pub confidence: Confidence,
    pub records: usize,
    /// P(equal) for the same layer in the same-named set of two different
    /// files. High means the field is a stable property of (set, layer).
    pub same_set_agreement: Option<f64>,
    /// P(equal) for the same layer in two differently named sets of one file.
    /// Clearly lower than the same-set value means the field varies by set.
    pub other_set_agreement: Option<f64>,
    pub note: String,
}

/// Cross-file calibration summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    pub files: usize,
    pub layer_sets: usize,
    pub candidate_records: usize,
    pub structural_records: usize,
    pub fields: Vec<FieldReport>,
    pub wall_stacks_found: usize,
}

/// (equal pairs, total pairs) over the values of one group.
fn pairs<T: Eq + std::hash::Hash>(values: &[T]) -> (u64, u64) {
    let n = values.len() as u64;
    if n < 2 {
        return (0, 0);
    }
    let mut counts: HashMap<&T, u64> = HashMap::new();
    for v in values {
        *counts.entry(v).or_default() += 1;
    }
    let eq = counts.values().map(|c| c * (c - 1) / 2).sum();
    (eq, n * (n - 1) / 2)
}

fn ratio((eq, total): (u64, u64)) -> Option<f64> {
    (total > 0).then(|| eq as f64 / total as f64)
}

/// Agreement of one field extracted by `get`.
fn agreement<T: Eq + std::hash::Hash + Clone>(
    reports: &[ValueReport],
    get: impl Fn(&LayerValues) -> T,
) -> (Option<f64>, Option<f64>) {
    // Same set name + layer, across files (first set of that name per file).
    let mut same: HashMap<(String, String), Vec<T>> = HashMap::new();
    let mut other = (0u64, 0u64);
    for rep in reports {
        let mut seen_sets = HashSet::new();
        let mut per_layer: HashMap<&str, Vec<T>> = HashMap::new();
        for set in &rep.layer_sets {
            if !set.named || !seen_sets.insert(set.name.to_lowercase()) {
                continue;
            }
            for l in &set.layers {
                let v = get(l);
                same.entry((set.name.to_lowercase(), l.name.clone()))
                    .or_default()
                    .push(v.clone());
                per_layer.entry(l.name.as_str()).or_default().push(v);
            }
        }
        for vals in per_layer.values() {
            let (e, t) = pairs(vals);
            other.0 += e;
            other.1 += t;
        }
    }
    let mut same_total = (0u64, 0u64);
    for vals in same.values() {
        let (e, t) = pairs(vals);
        same_total.0 += e;
        same_total.1 += t;
    }
    (ratio(same_total), ratio(other))
}

fn grade(same: Option<f64>, cap: Confidence) -> Confidence {
    let c = match same {
        Some(s) if s >= 0.9 => Confidence::High,
        Some(s) if s >= 0.7 => Confidence::Medium,
        Some(_) => Confidence::Low,
        None => Confidence::None,
    };
    c.min(cap)
}

/// Compares the same layer across all files and across the sets of each file
/// to grade every decoded field.
pub fn calibrate(reports: &[ValueReport]) -> Calibration {
    let all: Vec<&LayerValues> = reports
        .iter()
        .flat_map(|r| r.layer_sets.iter().flat_map(|s| s.layers.iter()))
        .collect();
    let n = all.len();
    let mut fields = Vec::new();
    let mut push = |field: &str,
                    location: &str,
                    (same, other): (Option<f64>, Option<f64>),
                    cap: Confidence,
                    note: &str| {
        fields.push(FieldReport {
            field: field.into(),
            location: location.into(),
            confidence: grade(same, cap),
            records: n,
            same_set_agreement: same,
            other_set_agreement: other,
            note: note.into(),
        });
    };
    push(
        "color (RGB)",
        "e+1..e+4, alpha 0xFF at e+4",
        agreement(reports, |l| l.color),
        Confidence::High,
        "Sanity: dimension/text layers decode navy (0,0,128), revision clouds red (185,0,0).",
    );
    push(
        "line weight",
        "e+5..e+7, u16 LE, 1/100 mm",
        agreement(reports, |l| l.line_weight),
        Confidence::High,
        "Values 0..50 (0.00 to 0.50 mm) in the samples; never checked against Chief's UI.",
    );
    push(
        "display flag",
        "flags byte o-6, bit 0",
        agreement(reports, |l| l.display),
        Confidence::Medium,
        "Varies between layer sets and is stable across files for the same set. Meaning inferred, not checked in the UI.",
    );
    push(
        "lock flag",
        "flags byte o-6, bit 1",
        agreement(reports, |l| l.locked),
        Confidence::Low,
        "Set on nearly every layer of 'Reference Display Layer Set' and a few others; meaning inferred.",
    );
    push(
        "class byte",
        "byte o-5",
        agreement(reports, |l| l.class_byte),
        Confidence::Low,
        "Constant per layer, values 0..3; meaning unknown.",
    );
    push(
        "secondary colour",
        "e+84..e+88",
        agreement(reports, |l| l.secondary_color),
        Confidence::Low,
        "Equals the primary colour for about 70% of records, 0xFFFFFFFF on label layers; meaning unknown.",
    );
    Calibration {
        files: reports.len(),
        layer_sets: reports.iter().map(|r| r.layer_sets.len()).sum(),
        candidate_records: reports.iter().map(|r| r.candidate_records).sum(),
        structural_records: reports.iter().map(|r| r.structural_records).sum(),
        fields,
        wall_stacks_found: reports.iter().map(|r| r.wall_stacks.len()).sum(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::testutil::{build_template, put_str};
    use crate::scan::{scan_bytes, TemplateKind};

    /// Writes one layer record in the observed frame.
    fn put_layer(out: &mut Vec<u8>, name: &str, id: i32, flags: u8, rgb: [u8; 3], weight: u16) {
        out.extend_from_slice(&[0xE0, 0x3F, flags, 1]);
        out.extend_from_slice(&id.to_le_bytes());
        put_str(out, name);
        let mut post = vec![0u8; 100];
        post[1..4].copy_from_slice(&rgb);
        post[4] = 0xFF;
        post[5..7].copy_from_slice(&weight.to_le_bytes());
        post[84..87].copy_from_slice(&rgb);
        post[87] = 0xFF;
        out.extend_from_slice(&post);
    }

    fn layer_set(out: &mut Vec<u8>, set: &str, display: impl Fn(usize) -> bool) {
        out.extend_from_slice(&[0; 8]);
        put_str(out, set);
        out.extend_from_slice(&[0; 40]);
        for i in 0..24 {
            let flags = u8::from(display(i)) | if i == 3 { 2 } else { 0 };
            let rgb = if i == 0 { [0, 0, 128] } else { [i as u8, 2, 3] };
            put_layer(
                out,
                &format!("Walls, Test {i}"),
                -128 + i as i32,
                flags,
                rgb,
                18 + i as u16,
            );
        }
    }

    fn two_set_template() -> Vec<u8> {
        let mut body = Vec::new();
        layer_set(&mut body, "Alpha Layer Set", |i| i % 2 == 0);
        body.extend(std::iter::repeat_n(0u8, 50_000));
        layer_set(&mut body, "Beta Layer Set", |i| i % 3 == 0);
        build_template(&body, &[])
    }

    #[test]
    fn decodes_layer_sets_and_fields() {
        let bytes = two_set_template();
        let scan = scan_bytes(&bytes, Some(TemplateKind::Plan)).unwrap();
        let rep = decode(&bytes, &scan);
        assert_eq!(rep.layer_sets.len(), 2);
        assert_eq!(rep.structural_records, 48);
        let (a, b) = (&rep.layer_sets[0], &rep.layer_sets[1]);
        assert_eq!(
            (a.name.as_str(), b.name.as_str()),
            ("Alpha Layer Set", "Beta Layer Set")
        );
        assert!(a.named && b.named);
        assert_eq!(a.layers.len(), 24);
        let l0 = &a.layers[0];
        assert_eq!(l0.name, "Walls, Test 0");
        assert_eq!(l0.id, -128);
        assert_eq!(l0.color, [0, 0, 128]);
        assert_eq!(l0.line_weight, 18);
        assert!(l0.display && !l0.locked);
        let l3 = &a.layers[3];
        assert!(!l3.display && l3.locked, "flags {:#x}", l3.flags);
        assert_eq!(a.layers[5].line_weight, 23);
        // Display differs between the two sets for the same layer.
        assert!(a.layers[2].display && !b.layers[2].display);
        assert_eq!(a.visible_layers().len(), 12);
        assert_eq!(b.visible_layers().len(), 8);
    }

    #[test]
    fn small_runs_and_set_names_are_not_layers() {
        // Ten records are below the per-set minimum.
        let mut body = Vec::new();
        body.extend_from_slice(&[0; 8]);
        put_str(&mut body, "Tiny Layer Set");
        for i in 0..10 {
            put_layer(
                &mut body,
                &format!("Walls, Few {i}"),
                -100 + i,
                1,
                [0; 3],
                10,
            );
        }
        let bytes = build_template(&body, &[]);
        let scan = scan_bytes(&bytes, None).unwrap();
        let rep = decode(&bytes, &scan);
        assert!(rep.layer_sets.is_empty());
        assert_eq!(rep.structural_records, 10);
    }

    #[test]
    fn unnamed_set_when_no_name_precedes() {
        let mut body = vec![0u8; 8];
        for i in 0..22 {
            put_layer(
                &mut body,
                &format!("CAD, Item {i}"),
                -128 + i,
                1,
                [1, 1, 1],
                18,
            );
        }
        let bytes = build_template(&body, &[]);
        let scan = scan_bytes(&bytes, None).unwrap();
        let rep = decode(&bytes, &scan);
        assert_eq!(rep.layer_sets.len(), 1);
        assert!(!rep.layer_sets[0].named);
        assert_eq!(rep.layer_sets[0].name, "(unnamed set 1)");
    }

    #[test]
    fn wall_stack_f64_and_negative_cases() {
        let mut body = Vec::new();
        put_str(&mut body, "Siding-6");
        body.extend_from_slice(&[0, 0]);
        for v in [0.5f64, 5.5, 0.5] {
            body.extend_from_slice(&v.to_le_bytes());
        }
        body.extend_from_slice(&[0; 16]);
        // A wall type followed only by text: no stack.
        put_str(&mut body, "Interior-4");
        put_str(&mut body, "Interior-6");
        let bytes = build_template(&body, &[]);
        let scan = scan_bytes(&bytes, Some(TemplateKind::Plan)).unwrap();
        let rep = decode(&bytes, &scan);
        assert_eq!(rep.wall_stacks.len(), 1);
        let s = &rep.wall_stacks[0];
        assert_eq!(s.name, "Siding-6");
        assert_eq!(s.thicknesses, vec![0.5, 5.5, 0.5]);
        assert!((s.total - 6.5).abs() < 1e-9);
        assert_eq!(s.float_bytes, 8);
    }

    #[test]
    fn wall_stack_f32() {
        let mut body = Vec::new();
        put_str(&mut body, "Stucco-6");
        body.extend_from_slice(&[0, 0, 0]);
        for v in [1.0f32, 5.5, 0.625] {
            body.extend_from_slice(&v.to_le_bytes());
        }
        body.extend_from_slice(&[0; 16]);
        let bytes = build_template(&body, &[]);
        let scan = scan_bytes(&bytes, None).unwrap();
        let rep = decode(&bytes, &scan);
        assert_eq!(rep.wall_stacks[0].float_bytes, 4);
        assert!((rep.wall_stacks[0].total - 7.125).abs() < 1e-9);
    }

    #[test]
    fn calibration_grades_stable_fields_high() {
        let bytes = two_set_template();
        let scan = scan_bytes(&bytes, None).unwrap();
        let rep = decode(&bytes, &scan);
        let cal = calibrate(&[rep.clone(), rep]);
        assert_eq!(cal.files, 2);
        let get = |n: &str| cal.fields.iter().find(|f| f.field.starts_with(n)).unwrap();
        let color = get("color");
        assert_eq!(color.confidence, Confidence::High);
        assert_eq!(color.same_set_agreement, Some(1.0));
        // Display differs between sets, identical across the two "files".
        let disp = get("display");
        assert_eq!(disp.same_set_agreement, Some(1.0));
        assert!(disp.other_set_agreement.unwrap() < 1.0);
        assert_eq!(disp.confidence, Confidence::Medium);
        assert_eq!(get("lock").confidence, Confidence::Low);
    }

    #[test]
    fn pair_math() {
        assert_eq!(pairs(&[1, 1, 2]), (1, 3));
        assert_eq!(pairs(&[1]), (0, 0));
        assert_eq!(ratio((1, 0)), None);
    }
}
