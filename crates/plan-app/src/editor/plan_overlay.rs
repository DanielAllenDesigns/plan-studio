//! What a plan box on a layout page (and the DXF export) shows besides the
//! walls, openings, rooms, dimensions and CAD: the floor's cabinets with
//! their Fill Style, merged countertops, placed symbols and stairs, the
//! lower floor's stairs with the treads seen through a stairwell.
//!
//! `plan-layout` cannot build these (no `plan-cabinets` / `plan-stairs`), so
//! [`overlay`] turns what the screen draws (`placed::draw_placed`,
//! `stairs_view::draw_stairs`) into the neutral
//! [`plan_layout::PlanOverlayItem`] list the layout renderer takes through
//! `LayoutRenderContext::with_plan_overlay`.

use super::placed::{
    cabinet_layer, load_cabinets, merged_countertops, placed_symbol_strokes, stroke_polylines,
    symbol_center,
};
use super::stairs_view as sv;
use crate::tools::library::find_item;
use plan_cabinets::{Cabinet, FillPattern, Stroke as CabStroke};
use plan_core::cad::CadItem;
use plan_core::geometry::Point;
use plan_core::{Floor, Project};
use plan_layout::{OverlayShape, PlanOverlayItem};
use plan_stairs::Stroke as StairStroke;

/// The layer the dashed treads under a stairwell and beyond the break line
/// go on in a DXF (it has no dashes of its own here).
pub const DXF_HIDDEN_STAIRS_LAYER: &str = "Stairs, Hidden";

/// Samples a circular arc, degrees counter-clockwise.
fn arc(center: Point, radius: f64, start_deg: f64, end_deg: f64) -> Vec<Point> {
    let sweep = if end_deg - start_deg >= 360.0 {
        360.0
    } else {
        (end_deg - start_deg).rem_euclid(360.0)
    };
    (0..=32)
        .map(|i| {
            let a = (start_deg + sweep * f64::from(i) / 32.0).to_radians();
            Point::new(center.x + radius * a.cos(), center.y + radius * a.sin())
        })
        .collect()
}

fn text(layer: &str, at: Point, text: &str, height: f64, angle: f64) -> PlanOverlayItem {
    PlanOverlayItem {
        layer: layer.to_string(),
        shape: OverlayShape::Text {
            at,
            text: text.to_string(),
            height,
            angle,
        },
        dashed: false,
        weight: 1.0,
    }
}

/// Text anchors: the screen centres a label on its point; a page text starts
/// at its baseline, so move it back by half the (estimated) width.
fn centred(at: Point, label: &str, height: f64, angle: f64) -> Point {
    let half = label.chars().count() as f64 * height * 0.5 * 0.55;
    at - Point::new(angle.cos(), angle.sin()) * half
        - Point::new(-angle.sin(), angle.cos()) * (height * 0.35)
}

/// A cabinet's Fill Style: the translucent solid or hatch lines clipped to
/// its outline (`placed::draw_cabinet_fill`).
fn cabinet_fill(cab: &Cabinet, out: &mut Vec<PlanOverlayItem>) {
    let fill = &cab.fill;
    if !fill.is_visible() {
        return;
    }
    let layer = cabinet_layer(cab.kind);
    let ring = cab.footprint();
    if fill.pattern == FillPattern::Solid {
        for t in plan_cabinets::triangulate(&ring, &[]) {
            out.push(PlanOverlayItem {
                layer: layer.to_string(),
                shape: OverlayShape::Fill {
                    points: t.to_vec(),
                    rgb: fill.color,
                    alpha: fill.alpha,
                },
                dashed: false,
                weight: 1.0,
            });
        }
        return;
    }
    for [a, b] in fill.hatch_lines(&ring) {
        out.push(PlanOverlayItem::line(layer, vec![a, b], false).weighted(0.4));
    }
}

/// One cabinet's plan symbol (the countertop outline is left to the merged
/// tops) and label.
fn cabinet_items(cab: &Cabinet, out: &mut Vec<PlanOverlayItem>) {
    let layer = cabinet_layer(cab.kind);
    for (i, k) in plan_cabinets::plan_symbol(cab).iter().enumerate() {
        match k {
            CabStroke::Line(a, b) => out.push(PlanOverlayItem::line(layer, vec![*a, *b], false)),
            CabStroke::Polyline(pts, closed) => {
                if cab.countertop.is_some() && i == 1 {
                    continue;
                }
                out.push(PlanOverlayItem::line(layer, pts.clone(), *closed));
            }
            CabStroke::Arc {
                center,
                radius,
                start,
                end,
            } => out.push(PlanOverlayItem::line(
                layer,
                arc(*center, *radius, start.to_degrees(), end.to_degrees()),
                false,
            )),
            CabStroke::Text {
                at,
                text: label,
                height,
                angle,
            } => out.push(text(
                plan_core::layers::CABINET_LABEL_LAYER,
                centred(*at, label, *height, *angle),
                label,
                *height,
                *angle,
            )),
        }
    }
}

fn stair_stroke(
    layer: &str,
    s: &StairStroke,
    dashed: bool,
    weight: f64,
    out: &mut Vec<PlanOverlayItem>,
) {
    let mut push = |item: PlanOverlayItem| {
        let mut item = item.weighted(weight);
        item.dashed = dashed;
        out.push(item);
    };
    match s {
        StairStroke::Line(a, b) => push(PlanOverlayItem::line(layer, vec![*a, *b], false)),
        StairStroke::Polyline(pts, closed) => {
            push(PlanOverlayItem::line(layer, pts.clone(), *closed));
        }
        StairStroke::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => push(PlanOverlayItem::line(
            layer,
            arc(*center, *radius, *start_deg, *end_deg),
            false,
        )),
        StairStroke::Text {
            pos,
            text: label,
            height,
            angle,
        } => out.push(text(
            layer,
            centred(*pos, label, *height, *angle),
            label,
            *height,
            *angle,
        )),
    }
}

fn stairs_of_floor(floor: &Floor, out: &mut Vec<PlanOverlayItem>) {
    for o in sv::load(floor) {
        let fp = o.footprint();
        if o.x.fill && fp.len() >= 3 {
            let g = (f64::from(o.x.fill_gray) * 0.45 + 255.0 * 0.55).round() as u8;
            for t in plan_cabinets::triangulate(&fp, &[]) {
                out.push(PlanOverlayItem {
                    layer: sv::LAYER.to_string(),
                    shape: OverlayShape::Fill {
                        points: t.to_vec(),
                        rgb: [g, g, g],
                        alpha: 1.0,
                    },
                    dashed: false,
                    weight: 1.0,
                });
            }
        }
        let weight = (f64::from(o.x.line_weight) / 0.5).clamp(0.5, 3.0);
        for s in sv::symbol_strokes(&o) {
            stair_stroke(sv::LAYER, &s, o.x.dashed, weight, out);
        }
        for s in sv::hidden_strokes(&o) {
            stair_stroke(sv::LAYER, &s, true, weight * 0.75, out);
        }
        if o.x.show_label && !o.x.label.is_empty() {
            let c = plan_core::geometry::polygon_centroid(&fp);
            out.push(text(
                sv::LAYER,
                centred(c, &o.x.label, 6.0, 0.0),
                &o.x.label,
                6.0,
                0.0,
            ));
        }
    }
}

/// The stairs of the floor below as this floor sees them: the part beyond
/// their break line and, through an opening, the treads below it (dashed).
fn stairs_from_below(project: &Project, floor: usize, out: &mut Vec<PlanOverlayItem>) {
    let Some(below) = floor.checked_sub(1).and_then(|f| project.floors.get(f)) else {
        return;
    };
    for o in sv::load(below) {
        for s in sv::upper_strokes(&o) {
            stair_stroke(sv::LAYER, &s, true, 0.75, out);
        }
        if sv::open_to_floor_above(project, floor - 1, &o) {
            for s in sv::well_strokes(&o) {
                stair_stroke(sv::LAYER, &s, true, 0.6, out);
            }
        }
    }
}

fn symbol_items(floor: &Floor, out: &mut Vec<PlanOverlayItem>) {
    for s in &floor.symbols {
        // Pictures and distributed objects are drawn by their own tools.
        if s.image.is_some() || s.distribution.is_some() {
            continue;
        }
        match placed_symbol_strokes(s) {
            Some(sym) => {
                for (pts, closed) in stroke_polylines(&sym) {
                    out.push(PlanOverlayItem::line(s.layer.clone(), pts, closed));
                }
            }
            None => out.push(PlanOverlayItem::line(
                s.layer.clone(),
                s.footprint().to_vec(),
                true,
            )),
        }
        let unknown = find_item(&s.catalog_id).is_none();
        let label = if s.label.is_empty() && unknown {
            plan_library::standin::stand_in_label(&s.catalog_id)
        } else {
            s.label.clone()
        };
        if !label.is_empty() {
            let angle = s.angle.to_radians();
            out.push(text(
                &s.layer,
                centred(symbol_center(s), &label, 3.0, angle),
                &label,
                3.0,
                angle,
            ));
        }
    }
}

/// The overlay of `floor`: cabinet fills first (so lines and text draw on
/// top), then cabinets, merged countertops (dashed), symbols and stairs.
pub fn overlay(project: &Project, floor: usize) -> Vec<PlanOverlayItem> {
    let Some(f) = project.floors.get(floor) else {
        return Vec::new();
    };
    let cabs = load_cabinets(f);
    let mut out = Vec::new();
    for c in &cabs {
        cabinet_fill(c, &mut out);
    }
    for c in &cabs {
        cabinet_items(c, &mut out);
    }
    let tops: Vec<Cabinet> = cabs
        .iter()
        .filter(|c| c.countertop.is_some())
        .cloned()
        .collect();
    for poly in merged_countertops(&tops) {
        out.push(PlanOverlayItem::line("Cabinets, Base", poly, true).dashed());
    }
    symbol_items(f, &mut out);
    stairs_from_below(project, floor, &mut out);
    stairs_of_floor(f, &mut out);
    out
}

/// The overlay as CAD for a DXF export: lines on their layers, dashed ones
/// on `"<layer>, Hidden"` ("Stairs, Hidden"), texts as texts. Fills have no
/// DXF form here and are left out.
pub fn dxf_items(project: &Project, floor: usize) -> Vec<(String, CadItem)> {
    let mut out = Vec::new();
    for it in overlay(project, floor) {
        let layer = if it.dashed {
            if it.layer == sv::LAYER {
                DXF_HIDDEN_STAIRS_LAYER.to_string()
            } else {
                format!("{}, Hidden", it.layer)
            }
        } else {
            it.layer.clone()
        };
        match it.shape {
            OverlayShape::Polyline { points, closed } if points.len() >= 2 => {
                out.push((layer, CadItem::Polyline { points, closed }));
            }
            OverlayShape::Text {
                at,
                text,
                height,
                angle,
            } => out.push((
                layer,
                CadItem::Text {
                    pos: at,
                    text,
                    height,
                    angle,
                },
            )),
            _ => {}
        }
    }
    out
}
