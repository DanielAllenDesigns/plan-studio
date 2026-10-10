//! Roof Truss, Girder Truss and Floor/Ceiling Truss Specification (reference
//! manual pp. 955 to 963).
//!
//! The panels of the truss kinds of the Framing Member Specification
//! (`dialogs/framing.rs` picks them for a truss member):
//!
//! * **General** (roof): Truss Type, Pitch, Heel Height and Overhang; Member
//!   Sizing (Top Chord, Bottom Chord, Webbing, and for a girder the Ply
//!   Thickness and Ply Count); Maximum Horizontal Span (Top and Bottom
//!   Chord); Horizontal Blocking (Vertical Spacing, Rollout Offset with
//!   Automatic); Roof Directives (Require Kingpost, End Truss, Energy Heel,
//!   Drop Hip Truss, Reduced Gable, Attic Truss, Sloping Flat Truss);
//!   Options (Automatically Generated Truss, Force Truss Rebuild, Lock Truss
//!   Envelope and Webbing, Use Special Snapping, Calculate Chords/Webbing in
//!   Materials List, Show Multi-Ply Lines).
//! * **General** (floor / ceiling): Member Depth, Thickness, Maximum Span,
//!   Horizontal Blocking, Truss Directives (Vertical Supports) and Options.
//! * Line Style, Fill Style and Label, with the Components, Object
//!   Information and Schedule panels the shared frame adds.
//!
//! The panels edit the member's [`FramingMember::truss`] spec in place; the
//! span is always the member's length. Chief does not engineer trusses: the
//! dialog says so.

use super::{dis_combo, fmt_short, on, row, section, Fields, Tab};
use eframe::egui::{self, Ui};
use plan_framing::{FramingMember, Lumber, ManualMemberKind, Truss, TrussSpec, TrussType};

/// Tabs of a Roof or Girder Truss.
pub const ROOF_TABS: &[Tab] = &[
    on("General"),
    on("Line Style"),
    on("Fill Style"),
    on("Materials"),
    on("Label"),
    on("Layer"),
];

/// Tabs of a Floor/Ceiling Truss.
pub const FLOOR_TABS: &[Tab] = ROOF_TABS;

/// The truss types the Truss Type list offers.
pub const TYPES: [TrussType; 8] = [
    TrussType::Fink,
    TrussType::Howe,
    TrussType::KingPost,
    TrussType::DoubleFink,
    TrussType::DoubleHowe,
    TrussType::Scissor,
    TrussType::Attic,
    TrussType::Mono,
];

/// Member depths offered for chords and webbing, actual inches.
const DEPTHS: [(&str, f64); 5] = [
    ("2x4", 3.5),
    ("2x6", 5.5),
    ("2x8", 7.25),
    ("2x10", 9.25),
    ("2x12", 11.25),
];

const MAX_PITCH: f64 = 24.0;
const MAX_PLIES: u32 = 6;

fn depth_name(depth: f64) -> String {
    DEPTHS
        .iter()
        .find(|(_, d)| (d - depth).abs() < 0.01)
        .map_or_else(|| fmt_short(depth), |(n, _)| (*n).to_string())
}

/// A combo over the standard depths; `value` is the depth in inches.
fn depth_combo(ui: &mut Ui, salt: &str, value: &mut f64) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(depth_name(*value))
        .show_ui(ui, |ui| {
            for (name, d) in DEPTHS {
                ui.selectable_value(value, d, name);
            }
        });
}

fn lumber(depth: f64, like: Lumber) -> Lumber {
    Lumber {
        thickness: like.thickness,
        depth,
    }
}

/// What the Truss panel's non-spec switches hold while the dialog is open.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Switches {
    /// Automatically Generated Truss: on while the truss is a laid-out one
    /// and the box stays checked.
    pub auto: bool,
    /// The truss was laid out by Build Framing (the box can be unchecked).
    pub was_auto: bool,
}

/// The General panel of a roof, girder or floor/ceiling truss.
pub fn general(m: &mut FramingMember, sw: &mut Switches, fields: &mut Fields, ui: &mut Ui) {
    let floor = m.kind == ManualMemberKind::FloorCeilingTruss;
    let girder = m.kind == ManualMemberKind::GirderTruss;
    let span = m.plan_length();
    let plies = m.plies.max(1);
    let Some(spec) = m.truss.as_mut() else {
        ui.weak("This member is not a truss.");
        return;
    };
    ui.weak("Trusses are shown for illustration only and are not engineered: have a licensed engineer approve every truss design.");
    section(ui, "Truss Shape");
    row(ui, "Truss Type", |ui| {
        egui::ComboBox::from_id_salt("truss_type")
            .selected_text(spec.kind.name())
            .show_ui(ui, |ui| {
                for t in TYPES {
                    ui.selectable_value(&mut spec.kind, t, t.name());
                }
            });
    });
    row(ui, "Span", |ui| ui.label(fmt_short(span)));
    if !floor {
        row(ui, "Pitch (rise per 12)", |ui| {
            ui.add(
                egui::DragValue::new(&mut spec.pitch)
                    .range(0.0..=MAX_PITCH)
                    .speed(0.1)
                    .suffix(":12"),
            )
        });
        fields.length_row(ui, "Heel height", "tr_heel", &mut spec.heel_height);
        fields.length_row(ui, "Overhang", "tr_overhang", &mut spec.overhang);
    }

    if floor {
        section(ui, "Member Depth");
        let mut top = spec.chord.depth;
        row(ui, "Top Chord", |ui| depth_combo(ui, "tr_top", &mut top));
        spec.chord = lumber(top, spec.chord);
        let mut bottom = spec.bottom_lumber().depth;
        row(ui, "Bottom Chord", |ui| {
            depth_combo(ui, "tr_bot", &mut bottom)
        });
        spec.bottom_chord_depth = if (bottom - top).abs() < 1e-9 {
            0.0
        } else {
            bottom
        };
        let mut web = spec.web.depth;
        row(ui, "Webbing", |ui| depth_combo(ui, "tr_web", &mut web));
        spec.web = lumber(web, spec.web);
        section(ui, "Thickness");
        fields.length_row(
            ui,
            "Overall Thickness",
            "tr_overall",
            &mut spec.chord.thickness,
        );
        fields.length_row(ui, "Webbing", "tr_web_t", &mut spec.web_thickness);
        fields.length_row(ui, "Overall Depth", "tr_depth", &mut spec.flat_depth);
        section(ui, "Maximum Span");
        fields.length_row(
            ui,
            "Top and Bottom Chord",
            "tr_span",
            &mut spec.max_span_bottom,
        );
        spec.max_span_top = spec.max_span_bottom;
    } else {
        section(ui, "Member Sizing");
        let mut top = spec.chord.depth;
        row(ui, "Top Chord", |ui| depth_combo(ui, "tr_top", &mut top));
        spec.chord = lumber(top, spec.chord);
        let mut bottom = spec.bottom_lumber().depth;
        row(ui, "Bottom Chord", |ui| {
            depth_combo(ui, "tr_bot", &mut bottom)
        });
        spec.bottom_chord_depth = if (bottom - top).abs() < 1e-9 {
            0.0
        } else {
            bottom
        };
        let mut web = spec.web.depth;
        row(ui, "Webbing", |ui| depth_combo(ui, "tr_web", &mut web));
        spec.web = lumber(web, spec.web);
        if girder {
            let mut p = plies;
            row(ui, "Ply Count", |ui| {
                ui.add(egui::DragValue::new(&mut p).range(1..=MAX_PLIES))
            });
            spec.plies = p;
            row(ui, "Ply Thickness", |ui| {
                ui.label(fmt_short(spec.chord.thickness))
            });
            row(ui, "Total Thickness", |ui| {
                ui.label(fmt_short(spec.thickness()))
            });
        }
        section(ui, "Maximum Horizontal Span");
        fields.length_row(ui, "Top Chord", "tr_span_top", &mut spec.max_span_top);
        fields.length_row(ui, "Bottom Chord", "tr_span_bot", &mut spec.max_span_bottom);
        ui.weak("0 leaves the webbing of the truss type; a smaller span gives a finer web.");
    }

    section(ui, "Horizontal Blocking");
    ui.checkbox(&mut spec.horizontal_blocking, "Horizontal Blocking");
    ui.add_enabled_ui(spec.horizontal_blocking, |ui| {
        fields.length_row(ui, "Vertical Spacing", "tr_block", &mut spec.block_spacing);
        ui.checkbox(&mut spec.rollout_auto, "Automatic Rollout Offset");
        ui.add_enabled_ui(!spec.rollout_auto, |ui| {
            fields.length_row(ui, "Rollout Offset", "tr_rollout", &mut spec.rollout_offset);
        });
    });

    if floor {
        section(ui, "Truss Directives");
        ui.checkbox(&mut spec.vertical_supports, "Vertical Supports");
    } else {
        section(ui, "Roof Directives");
        ui.checkbox(&mut spec.require_kingpost, "Require Kingpost");
        ui.checkbox(&mut spec.end_truss, "End Truss")
            .on_hover_text("Vertical members spaced like the wall studs below replace the webbing");
        ui.add_enabled_ui(spec.end_truss, |ui| {
            fields.length_row(ui, "Stud Spacing", "tr_studs", &mut spec.stud_spacing);
        });
        ui.checkbox(&mut spec.energy_heel, "Energy Heel");
        ui.checkbox(&mut spec.drop_hip, "Drop Hip Truss");
        ui.checkbox(&mut spec.reduced_gable, "Reduced Gable");
        let mut attic = spec.kind == TrussType::Attic;
        if ui.checkbox(&mut attic, "Attic Truss").changed() {
            spec.kind = if attic {
                TrussType::Attic
            } else {
                TrussType::Fink
            };
        }
        ui.checkbox(&mut spec.sloping_flat, "Sloping Flat Truss");
        ui.add_enabled_ui(spec.sloping_flat, |ui| {
            fields.length_row(ui, "Structure Depth", "tr_flat", &mut spec.flat_depth);
        });
    }

    section(ui, "Options");
    ui.add_enabled_ui(sw.was_auto, |ui| {
        ui.checkbox(&mut sw.auto, "Automatically Generated Truss")
            .on_hover_text("Uncheck to treat the truss as drawn by hand from now on");
    });
    ui.add_enabled_ui(!spec.locked, |ui| {
        ui.checkbox(&mut spec.force_rebuild, "Force Truss Rebuild")
            .on_hover_text("Makes the envelope and webbing again for where the truss stands");
    });
    if spec.locked {
        spec.force_rebuild = false;
    }
    ui.checkbox(&mut spec.locked, "Lock Truss Envelope and Webbing");
    ui.checkbox(&mut spec.special_snapping, "Use Special Snapping");
    ui.checkbox(
        &mut spec.calc_chords,
        "Calculate Chords/Webbing in Materials List",
    )
    .on_hover_text("Counts the chord and web boards instead of the truss as one object");
    if girder {
        ui.checkbox(&mut spec.show_ply_lines, "Show Multi-Ply Lines");
    }
}

/// How many members the truss gets for its current inputs.
pub fn member_count(m: &FramingMember) -> usize {
    m.truss.as_ref().map_or(0, |t| {
        let mut s: TrussSpec = t.clone();
        s.span = m.plan_length();
        Truss::generate(&s).members.len()
    })
}

/// The Line Style panel (line style and weight follow the layer).
pub fn line_style(ui: &mut Ui) {
    section(ui, "Line Style");
    row(ui, "Line style", |ui| dis_combo(ui, "truss_ls", "By layer"));
    row(ui, "Line weight", |ui| {
        dis_combo(ui, "truss_lw", "By layer")
    });
    ui.add_space(6.0);
    ui.weak("Line style and weight follow the truss's layer.");
}
