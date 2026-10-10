//! Floor and Wall Material Regions beyond the Details tool's polygon
//! (reference manual "Floor and Wall Material Regions", pp. 1085 to 1091;
//! parity rows CB-462..CB-467): Convert Polyline to a region, and the
//! Material Layers Definition window of a selected region.
//!
//! The region record and its drawing are in `tools::details` and
//! `editor::details_view`; the layered structure is
//! `plan_core::material_region`, drawn in 3D by `plan_3d::material_region`.

use crate::editor::actions::{EditAction, EditActionKind};
use crate::editor::{EditorContext, ObjectRef};
use plan_core::details::{DetailRef, DetailsLayer};
use plan_core::material_region::convert_polyline_to_region;
use plan_core::Id;

pub const FROM_POLYLINE: &str = "regions.from_polyline";
pub const LAYERS: &str = "regions.layers";

fn button(id: &'static str, label: &'static str) -> EditAction {
    EditAction {
        kind: EditActionKind::Custom {
            id,
            label,
            icon: "",
        },
        label,
        icon: None,
        enabled: true,
    }
}

/// The selected material region, when exactly one object is selected.
pub fn selected_region(cx: &EditorContext) -> Option<Id> {
    let [ObjectRef::Detail(id)] = cx.selection.items.as_slice() else {
        return None;
    };
    matches!(
        DetailsLayer::load(cx.floor()).find(*id),
        Some(DetailRef::Region(_))
    )
    .then_some(*id)
}

/// Convert Polyline: every selected closed CAD polyline becomes a floor
/// material region (one undo step).
pub fn convert_polylines(cx: &mut EditorContext) -> Result<usize, String> {
    let lines = super::solids::selected_closed_polylines(cx);
    if lines.is_empty() {
        return Err("Select a closed CAD polyline to convert to a Material Region".into());
    }
    let fl = cx.floor;
    cx.begin_change("Convert Polyline to Material Region");
    let mut made = Vec::new();
    for (cad, _) in &lines {
        let new_id = cx.project.alloc_id();
        match convert_polyline_to_region(&mut cx.project.floors[fl], *cad, new_id) {
            Ok(id) => made.push(id),
            Err(e) => {
                cx.cancel_change();
                return Err(e);
            }
        }
    }
    crate::editor::details_view::ensure_layers(&mut cx.project.layers);
    cx.selection.items = made.iter().map(|i| ObjectRef::Detail(*i)).collect();
    cx.mark_dirty();
    cx.status = format!("Converted {} polyline(s) to Material Regions", made.len());
    Ok(made.len())
}

/// Runs the region commands.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        FROM_POLYLINE => {
            if let Err(e) = convert_polylines(cx) {
                cx.status = e;
            }
        }
        LAYERS => match selected_region(cx) {
            Some(r) => crate::dialogs::material_region::open(cx, r),
            None => cx.status = "Select a Material Region first".into(),
        },
        _ => return false,
    }
    true
}

/// The Edit toolbar buttons for the selection.
pub fn edit_actions(cx: &EditorContext) -> Vec<EditAction> {
    let mut v = Vec::new();
    if selected_region(cx).is_some() {
        v.push(button(LAYERS, "Material Layers"));
    }
    if !super::solids::selected_closed_polylines(cx).is_empty() {
        v.push(button(FROM_POLYLINE, "Convert Polyline to Material Region"));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::geometry::Point;

    #[test]
    fn a_polyline_becomes_a_floor_region_in_one_undo_step() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let cad = cx.project.add_cad(
            0,
            "CAD, Default",
            plan_core::cad::CadItem::Polyline {
                points: vec![
                    Point::new(0.0, 0.0),
                    Point::new(60.0, 0.0),
                    Point::new(60.0, 40.0),
                    Point::new(0.0, 40.0),
                ],
                closed: true,
            },
        );
        cx.selection.items = vec![ObjectRef::Cad(cad)];
        assert_eq!(convert_polylines(&mut cx), Ok(1));
        assert_eq!(DetailsLayer::load(cx.floor()).regions.len(), 1);
        assert!(cx.floor().cad.is_empty());
        assert!(selected_region(&cx).is_some());
        cx.undo();
        assert!(DetailsLayer::load(cx.floor()).regions.is_empty());
        assert_eq!(cx.floor().cad.len(), 1);
    }

    #[test]
    fn an_open_polyline_is_refused() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let cad = cx.project.add_cad(
            0,
            "CAD, Default",
            plan_core::cad::CadItem::Polyline {
                points: vec![
                    Point::new(0.0, 0.0),
                    Point::new(60.0, 0.0),
                    Point::new(60.0, 40.0),
                ],
                closed: false,
            },
        );
        cx.selection.items = vec![ObjectRef::Cad(cad)];
        assert!(convert_polylines(&mut cx).is_err());
        assert!(cx.undo_label().is_none());
    }
}
