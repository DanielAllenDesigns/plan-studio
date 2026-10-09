//! The Materials List lines of a room's floor and ceiling platforms
//! (Round 16, brief 13): one row per layer of the room's layered Floor and
//! Ceiling Structure and Finish definitions, in the categories Chief lists
//! them under.
//!
//! * a Sheathing layer of a floor structure is the **Subfloor** (4x8 sheets);
//! * a Finish layer of a floor finish is the **Flooring** (square feet), a
//!   plain layer under it (backerboard, mortar) is Underlayment;
//! * a Framing layer is **Framing**: floor or ceiling framing, counted by
//!   the area it spans (square feet) with its spacing;
//! * a Finish or Standard layer of a ceiling finish is **Wallboard** (4x8
//!   sheets) when it is drywall, else the layer under its own name;
//! * an Air Gap (a plenum) is no purchase.
//!
//! A platform that has no layered definition keeps the rows it always had:
//! one Flooring row and one 1/2 in ceiling drywall row.

use super::{emit, Acc, Raw, Rule};
use plan_core::assemblies::{resolve, Assembly, AssemblyKind, AssemblyLayer, LayerRole};
use plan_core::{Floor, Room};

/// A thickness as inches and a fraction, "3/4\"", "1 1/8\"", "12\"".
pub fn inch_text(t: f64) -> String {
    let sixteenths = (t * 16.0).round() as i64;
    let (whole, rest) = (sixteenths / 16, sixteenths % 16);
    if rest == 0 {
        return format!("{whole}\"");
    }
    let g = {
        let (mut a, mut b) = (rest, 16);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a
    };
    let frac = format!("{}/{}", rest / g, 16 / g);
    if whole == 0 {
        format!("{frac}\"")
    } else {
        format!("{whole} {frac}\"")
    }
}

fn is_drywall(l: &AssemblyLayer) -> bool {
    let n = l.material.to_ascii_lowercase();
    ["drywall", "gypsum", "wallboard", "gwb"]
        .iter()
        .any(|k| n.contains(k))
}

/// Rows of one layered definition.
fn layer_rows(out: &mut Vec<Raw>, kind: AssemblyKind, a: &Assembly, room: &str, acc: &Acc) {
    let ceiling = matches!(
        kind,
        AssemblyKind::CeilingStructure | AssemblyKind::CeilingFinish
    );
    for l in &a.layers {
        if l.role == LayerRole::AirGap || l.thickness <= 0.0 {
            continue;
        }
        let thick = inch_text(l.thickness);
        let material = if l.material.trim().is_empty() {
            "Layer"
        } else {
            l.material.trim()
        };
        match (kind, l.role) {
            (_, LayerRole::Framing) => {
                let spacing = l
                    .framing
                    .map(|f| format!(" @ {} o.c.", inch_text(f.spacing)))
                    .unwrap_or_default();
                let key = if ceiling {
                    "Ceiling framing"
                } else {
                    "Floor framing"
                };
                emit(
                    out,
                    "Framing",
                    key,
                    format!("{key} {material} {thick}{spacing} - {room}"),
                    &thick,
                    "sq ft",
                    acc,
                    Rule::Ceil,
                );
            }
            (AssemblyKind::FloorStructure, LayerRole::Sheathing) => emit(
                out,
                "Interior Finishes",
                "Subfloor",
                format!("Subfloor {material} {thick} 4x8 sheet - {room}"),
                "4x8",
                "sheet",
                acc,
                Rule::Sheets,
            ),
            (AssemblyKind::FloorFinish, LayerRole::Finish | LayerRole::Cladding) => emit(
                out,
                "Interior Finishes",
                "Flooring",
                format!("Flooring - {material} {thick} - {room}"),
                "",
                "sq ft",
                acc,
                Rule::Ceil,
            ),
            (AssemblyKind::FloorFinish, _) => emit(
                out,
                "Interior Finishes",
                "Underlayment",
                format!("Underlayment - {material} {thick} - {room}"),
                "",
                "sq ft",
                acc,
                Rule::Ceil,
            ),
            (AssemblyKind::CeilingFinish | AssemblyKind::CeilingStructure, _) if is_drywall(l) => {
                emit(
                    out,
                    "Interior Finishes",
                    "Wallboard",
                    format!("Wallboard {thick} 4x8 sheet - {room}"),
                    "4x8",
                    "sheet",
                    acc,
                    Rule::Sheets,
                )
            }
            (_, LayerRole::Sheathing) => emit(
                out,
                "Interior Finishes",
                "Sheathing",
                format!("Sheathing {material} {thick} 4x8 sheet - {room}"),
                "4x8",
                "sheet",
                acc,
                Rule::Sheets,
            ),
            _ => emit(
                out,
                "Interior Finishes",
                material,
                format!("{material} {thick} - {room}"),
                "",
                "sq ft",
                acc,
                Rule::Ceil,
            ),
        }
    }
}

/// The rows of `room`'s platforms: its layered definitions layer by layer, the
/// old Flooring and ceiling drywall rows for a finish with none.
pub(super) fn emit_room(out: &mut Vec<Raw>, floor: &Floor, room: &Room, name: &str, acc: &Acc) {
    let misc = room
        .name_entry(&floor.room_names)
        .and_then(|n| n.misc.as_ref());
    let def = |k: AssemblyKind| resolve(k, &floor.settings, misc);
    let ff = def(AssemblyKind::FloorFinish);
    if ff.is_layered() {
        layer_rows(out, AssemblyKind::FloorFinish, &ff.assembly, name, acc);
    } else {
        emit(
            out,
            "Interior Finishes",
            "Flooring",
            format!("Flooring - {name}"),
            "",
            "sq ft",
            acc,
            Rule::Ceil,
        );
    }
    let cf = def(AssemblyKind::CeilingFinish);
    if cf.is_layered() {
        layer_rows(out, AssemblyKind::CeilingFinish, &cf.assembly, name, acc);
    } else {
        emit(
            out,
            "Interior Finishes",
            "Ceiling drywall 1/2\" 4x8 sheet",
            format!("Ceiling drywall 1/2\" 4x8 sheet - {name}"),
            "4x8",
            "sheet",
            acc,
            Rule::Sheets,
        );
    }
    for kind in [AssemblyKind::FloorStructure, AssemblyKind::CeilingStructure] {
        let r = def(kind);
        if r.is_layered() {
            layer_rows(out, kind, &r.assembly, name, acc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thicknesses_read_as_fractions_of_an_inch() {
        assert_eq!(inch_text(0.75), "3/4\"");
        assert_eq!(inch_text(11.25), "11 1/4\"");
        assert_eq!(inch_text(12.0), "12\"");
        assert_eq!(inch_text(0.375), "3/8\"");
        assert_eq!(inch_text(1.125), "1 1/8\"");
    }
}
