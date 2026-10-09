//! Custom Countertop Specification (reference manual pp. 688 to 690): the
//! panels of a free-form countertop. Polyline (outline figures and the Hole
//! in Countertop switch), Selected Line (length, angle, Molding on Selected
//! Edge, Add or Remove Waterfall), Moldings (thickness, edge profile and
//! size, Display Molding Edges) and the Mitre / Height options of
//! waterfalls.
//!
//! The page is a function on `plan_cabinets::Cabinet`; the Cabinet
//! Specification embeds it for the Custom Countertop kind and everything it
//! does is a method on the cabinet (`set_top_edge_length`,
//! `set_top_edge_molding`, `set_waterfall`), so tests drive the same calls.

use super::{fmt_short, row, section, Fields};
use eframe::egui::{self, Ui};
use plan_cabinets::{Cabinet, CabinetKind, EdgeMolding};

/// Why a length or angle could not be applied to the selected edge.
pub const EDGE_REFUSED: &str = "That edit would collapse the outline.";

/// The selected line, clamped to the outline.
pub fn selected_edge(d: &Cabinet) -> usize {
    let n = d.top_edge_count();
    if n == 0 {
        0
    } else {
        d.top_spec.selected.min(n - 1)
    }
}

/// Selects line `i` (clamped).
pub fn select_edge(d: &mut Cabinet, i: usize) {
    let n = d.top_edge_count();
    d.top_spec.selected = if n == 0 { 0 } else { i.min(n - 1) };
}

/// Apply to All Edges: the same molding choice on every edge.
pub fn apply_molding_to_all(d: &mut Cabinet, molding: EdgeMolding) {
    d.set_top_edge_molding(0, molding, true);
}

/// Add or Remove Waterfall from Selected Edge. Returns whether anything
/// changed.
pub fn toggle_selected_waterfall(d: &mut Cabinet, on: bool) -> bool {
    let i = selected_edge(d);
    let was = d.top_spec.edge(i).waterfall;
    if was == on {
        return false;
    }
    d.set_waterfall(i, on)
}

/// The panels of the Custom Countertop Specification. Returns an error
/// message to show when an edit was refused.
pub fn custom_top_ui(ui: &mut Ui, f: &mut Fields, d: &mut Cabinet) -> Option<&'static str> {
    let mut refused = None;
    d.custom.as_ref()?;
    let is_top = d.kind == CabinetKind::CustomCountertop;

    section(ui, "Polyline");
    let closed = d.custom.as_ref().is_some_and(|c| c.closed);
    row(ui, "Outline", |ui| {
        ui.label(if closed { "Closed" } else { "Open" });
        ui.label(format!(
            "{} lines, perimeter {}",
            d.top_edge_count(),
            fmt_short(d.top_perimeter())
        ));
    });
    if is_top {
        row(ui, "Area", |ui| {
            ui.label(format!("{:.1} sq ft", d.top_area() / 144.0));
        });
        ui.checkbox(&mut d.top_spec.hole, "Hole in Countertop");
        ui.checkbox(
            &mut d.top_spec.thickness_from_cabinet,
            "Thickness from the cabinet below",
        );
        ui.checkbox(
            &mut d.top_spec.height_from_cabinet,
            "Height from the cabinet below",
        );
    }

    let n = d.top_edge_count();
    if n == 0 {
        return refused;
    }
    section(ui, "Selected Line");
    let mut sel = selected_edge(d);
    row(ui, "Line", |ui| {
        egui::ComboBox::from_id_salt("ctop_line")
            .selected_text(format!("Line {} of {}", sel + 1, n))
            .show_ui(ui, |ui| {
                for i in 0..n {
                    ui.selectable_value(&mut sel, i, format!("Line {}", i + 1));
                }
            });
    });
    if sel != d.top_spec.selected {
        select_edge(d, sel);
    }
    let mut len = d.top_edge_length(sel);
    if f.length_row(ui, "Length", "ctop_len", &mut len) && !d.set_top_edge_length(sel, len) {
        refused = Some(EDGE_REFUSED);
    }
    let mut deg = d.top_edge_angle(sel).to_degrees();
    if f.degrees_row(ui, "Angle", "deg_ctop_angle", &mut deg)
        && !d.set_top_edge_angle(sel, deg.to_radians())
    {
        refused = Some(EDGE_REFUSED);
    }

    if is_top {
        section(ui, "Molding on Selected Edge");
        let cur = d.top_spec.edge(sel).molding;
        let mut pick = cur;
        ui.horizontal(|ui| {
            for m in EdgeMolding::ALL {
                ui.radio_value(&mut pick, m, m.name());
            }
        });
        if pick != cur {
            d.set_top_edge_molding(sel, pick, false);
        }
        if ui.button("Apply to All Edges").clicked() {
            apply_molding_to_all(d, pick);
        }

        section(ui, "Waterfall");
        let has = d.top_spec.edge(sel).waterfall;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!has, egui::Button::new("Add Waterfall to Selected Edge"))
                .clicked()
            {
                toggle_selected_waterfall(d, true);
            }
            if ui
                .add_enabled(has, egui::Button::new("Remove Waterfall"))
                .clicked()
            {
                toggle_selected_waterfall(d, false);
            }
        });
        if d.top_spec.has_waterfall() {
            ui.checkbox(&mut d.top_spec.mitre_waterfall, "Mitre All Waterfall Edges");
            ui.checkbox(
                &mut d.top_spec.waterfall_auto_height,
                "Waterfall runs to the floor",
            );
            if !d.top_spec.waterfall_auto_height {
                f.length_row(
                    ui,
                    "Waterfall Height",
                    "ctop_wf_h",
                    &mut d.top_spec.waterfall_height,
                );
            }
        }

        // The profile and its size are on the General panel's Custom Top
        // section; the per-edge choice above decides which edges carry it.
        section(ui, "Moldings");
        ui.checkbox(
            &mut d.top_spec.display_molding_edges,
            "Display Molding Edges in Plan Views",
        );
    }
    refused
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::geometry::Point;

    fn square_top() -> Cabinet {
        let mut c = Cabinet::custom_countertop(
            &[
                Point::new(0.0, 0.0),
                Point::new(48.0, 0.0),
                Point::new(48.0, 24.0),
                Point::new(0.0, 24.0),
            ],
            1.5,
            36.0,
        )
        .unwrap();
        c.top_spec.selected = 0;
        c
    }

    #[test]
    fn waterfall_and_molding_follow_the_selected_edge() {
        let mut c = square_top();
        assert_eq!(c.top_edge_count(), 4);
        select_edge(&mut c, 9);
        assert_eq!(selected_edge(&c), 3, "selection clamps");
        select_edge(&mut c, 1);
        assert!(toggle_selected_waterfall(&mut c, true));
        assert!(!toggle_selected_waterfall(&mut c, true), "already on");
        assert_eq!(c.waterfall_edges(), vec![1]);
        assert!(toggle_selected_waterfall(&mut c, false));
        assert!(c.waterfall_edges().is_empty());
        c.set_top_edge_molding(2, EdgeMolding::NoMolding, false);
        assert!(!c.top_edge_has_molding(2));
        apply_molding_to_all(&mut c, EdgeMolding::HasMolding);
        assert!((0..4).all(|i| c.top_edge_has_molding(i)));
    }

    #[test]
    fn the_page_draws_and_edits_the_selected_line() {
        let ctx = egui::Context::default();
        let mut f = Fields::default();
        let mut c = square_top();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(custom_top_ui(ui, &mut f, &mut c).is_none());
            });
        });
        assert!(c.top_edge_length(0) > 1.0);
    }
}
