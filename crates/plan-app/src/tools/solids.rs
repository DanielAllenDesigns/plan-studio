//! 3D solid commands beyond the Build > 3D Solid flyout (reference manual
//! "3D Solid Tools", pp. 1063 to 1081; parity rows CB-439..CB-458): Boolean
//! Union, Subtract and Intersect of the selected solids, Convert Polyline to
//! Solid, and the 3D Solid Options window (rotation about X and Y, elevation
//! reference, label, pyramid shape, surface quality).
//!
//! The primitives themselves are drawn by the Details tool (`tools::details`);
//! the geometry is `plan_core::solids`.

use crate::editor::actions::{EditAction, EditActionKind};
use crate::editor::{details_view, EditorContext, ObjectRef};
use plan_core::details::DetailsLayer;
use plan_core::geometry::Point;
use plan_core::solids::{boolean_floor, polyline_to_solid, BoolOp, INITIAL_SOLID_HEIGHT};
use plan_core::{Id, ObjectRef as CoreRef};

pub const UNION: &str = "solids.union";
pub const SUBTRACT: &str = "solids.subtract";
pub const INTERSECT: &str = "solids.intersect";
pub const FROM_POLYLINE: &str = "solids.from_polyline";
pub const OPTIONS: &str = "solids.options";

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

/// The selected 3D solids (primitives other than faces, and compound
/// solids) in selection order.
pub fn selected_solids(cx: &EditorContext) -> Vec<CoreRef> {
    let layer = DetailsLayer::load(cx.floor());
    cx.selection
        .items
        .iter()
        .filter_map(|o| match *o {
            ObjectRef::Detail(id) => layer
                .solid(id)
                .filter(|s| !matches!(s.kind, plan_core::details::SolidKind::Face { .. }))
                .map(|_| CoreRef::Detail(id)),
            ObjectRef::Solid(id) => Some(CoreRef::Solid(id)),
            _ => None,
        })
        .collect()
}

/// The closed CAD polylines in the selection.
pub fn selected_closed_polylines(cx: &EditorContext) -> Vec<(Id, Vec<Point>)> {
    cx.selection
        .items
        .iter()
        .filter_map(|o| match *o {
            ObjectRef::Cad(id) => cx.floor().cad.iter().find(|c| c.id == id),
            _ => None,
        })
        .filter_map(|c| match &c.item {
            plan_core::cad::CadItem::Polyline { points, closed }
                if *closed && points.len() >= 3 =>
            {
                Some((c.id, points.clone()))
            }
            _ => None,
        })
        .collect()
}

/// Union, Subtract or Intersect of the selected solids: one undo step. The
/// result is a compound solid and becomes the selection.
pub fn boolean(cx: &mut EditorContext, op: BoolOp) -> Result<Id, String> {
    let operands = selected_solids(cx);
    if operands.len() < 2 {
        return Err("Select two or more 3D solids".into());
    }
    if operands
        .iter()
        .any(|r| cx.floor().blocks.parent_of(*r).is_some())
    {
        return Err("Explode the architectural block first".into());
    }
    let fl = cx.floor;
    cx.begin_change(match op {
        BoolOp::Union => "Union Solids",
        BoolOp::Subtract => "Subtract Solids",
        BoolOp::Intersect => "Intersect Solids",
    });
    let new_id = cx.project.alloc_id();
    match boolean_floor(&mut cx.project.floors[fl], op, &operands, new_id) {
        Ok(id) => {
            crate::editor::solids_view::ensure_layers(&mut cx.project.layers);
            details_view::ensure_layers(&mut cx.project.layers);
            cx.selection.items = vec![ObjectRef::Solid(id)];
            cx.mark_dirty();
            cx.status = format!("{} of {} solids", op.name(), operands.len());
            Ok(id)
        }
        Err(e) => {
            cx.cancel_change();
            Err(e.to_string())
        }
    }
}

/// Convert Polyline to Solid: every selected closed CAD polyline becomes a
/// 3D Solid (a polyline solid of the initial height). One undo step.
pub fn convert_polylines(cx: &mut EditorContext) -> Result<usize, String> {
    let lines = selected_closed_polylines(cx);
    if lines.is_empty() {
        return Err("Select a closed CAD polyline to convert to a 3D solid".into());
    }
    let fl = cx.floor;
    let mut made: Vec<Id> = Vec::new();
    let mut solids = Vec::new();
    for (_, pts) in &lines {
        let id = cx.project.alloc_id();
        if let Some(s) = polyline_to_solid(id, pts, INITIAL_SOLID_HEIGHT) {
            made.push(id);
            solids.push(s);
        }
    }
    if solids.is_empty() {
        return Err("The polyline encloses no area".into());
    }
    details_view::edit(cx, "Convert Polyline to Solid", |layer| {
        layer.solids.extend(solids)
    });
    for (cad, _) in &lines {
        cx.project.remove_cad(fl, *cad);
    }
    cx.selection.items = made.iter().map(|i| ObjectRef::Detail(*i)).collect();
    cx.status = format!("Converted {} polyline(s) to 3D solids", made.len());
    Ok(made.len())
}

/// Runs the solid commands.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    let result = match id {
        UNION => boolean(cx, BoolOp::Union).map(|_| ()),
        SUBTRACT => boolean(cx, BoolOp::Subtract).map(|_| ()),
        INTERSECT => boolean(cx, BoolOp::Intersect).map(|_| ()),
        FROM_POLYLINE => convert_polylines(cx).map(|_| ()),
        OPTIONS => match selected_solids(cx).as_slice() {
            [one] => {
                crate::dialogs::solids::open(cx, *one);
                Ok(())
            }
            _ => Err("Select one 3D solid".into()),
        },
        _ => return false,
    };
    if let Err(e) = result {
        cx.status = e;
    }
    true
}

/// The Edit toolbar buttons for the selection.
pub fn edit_actions(cx: &EditorContext) -> Vec<EditAction> {
    let mut v = Vec::new();
    let solids = selected_solids(cx);
    if solids.len() >= 2 {
        v.push(button(UNION, "Union Solids"));
        v.push(button(SUBTRACT, "Subtract Solids"));
        v.push(button(INTERSECT, "Intersect Solids"));
    }
    if solids.len() == 1 {
        v.push(button(OPTIONS, "3D Solid Options"));
    }
    if !selected_closed_polylines(cx).is_empty() {
        v.push(button(FROM_POLYLINE, "Convert Polyline to Solid"));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::details::SolidKind;

    fn cx_with_boxes() -> (EditorContext, Id, Id) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let a = details_view::add_solid(
            &mut cx,
            SolidKind::Box {
                w: 24.0,
                d: 24.0,
                h: 24.0,
            },
            Point::new(0.0, 0.0),
        );
        let b = details_view::add_solid(
            &mut cx,
            SolidKind::Box {
                w: 24.0,
                d: 24.0,
                h: 24.0,
            },
            Point::new(12.0, 0.0),
        );
        (cx, a, b)
    }

    #[test]
    fn union_replaces_two_solids_with_one_compound_in_one_undo_step() {
        let (mut cx, a, b) = cx_with_boxes();
        cx.selection.items = vec![ObjectRef::Detail(a), ObjectRef::Detail(b)];
        let id = boolean(&mut cx, BoolOp::Union).unwrap();
        assert_eq!(cx.undo_label(), Some("Union Solids"));
        assert!(DetailsLayer::load(cx.floor()).solids.is_empty());
        let vol = cx.floor().solid_layer.compound(id).unwrap().volume();
        assert!(
            (vol - (24.0 * 24.0 * 24.0 * 2.0 - 12.0 * 24.0 * 24.0)).abs() < 1.0,
            "{vol}"
        );
        assert_eq!(cx.selection.items, vec![ObjectRef::Solid(id)]);
        cx.undo();
        assert_eq!(DetailsLayer::load(cx.floor()).solids.len(), 2);
        assert!(cx.floor().solid_layer.compounds.is_empty());
    }

    #[test]
    fn subtract_and_intersect_use_the_first_solid_as_the_base() {
        let (mut cx, a, b) = cx_with_boxes();
        cx.selection.items = vec![ObjectRef::Detail(a), ObjectRef::Detail(b)];
        let id = boolean(&mut cx, BoolOp::Subtract).unwrap();
        let vol = cx.floor().solid_layer.compound(id).unwrap().volume();
        assert!((vol - 12.0 * 24.0 * 24.0).abs() < 1.0, "{vol}");
        cx.undo();
        cx.selection.items = vec![ObjectRef::Detail(a), ObjectRef::Detail(b)];
        let id = boolean(&mut cx, BoolOp::Intersect).unwrap();
        let vol = cx.floor().solid_layer.compound(id).unwrap().volume();
        assert!((vol - 12.0 * 24.0 * 24.0).abs() < 1.0, "{vol}");
    }

    #[test]
    fn one_solid_cannot_be_combined_and_leaves_no_undo_step() {
        let (mut cx, a, _) = cx_with_boxes();
        let before = cx.undo_label().map(str::to_string);
        cx.selection.items = vec![ObjectRef::Detail(a)];
        assert!(boolean(&mut cx, BoolOp::Union).is_err());
        assert_eq!(cx.undo_label().map(str::to_string), before);
    }

    #[test]
    fn a_closed_polyline_converts_to_a_solid_and_the_polyline_goes() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let cad = cx.project.add_cad(
            0,
            "CAD, Default",
            plan_core::cad::CadItem::Polyline {
                points: vec![
                    Point::new(0.0, 0.0),
                    Point::new(48.0, 0.0),
                    Point::new(48.0, 24.0),
                    Point::new(0.0, 24.0),
                ],
                closed: true,
            },
        );
        cx.selection.items = vec![ObjectRef::Cad(cad)];
        assert_eq!(convert_polylines(&mut cx), Ok(1));
        assert!(cx.floor().cad.is_empty());
        assert_eq!(DetailsLayer::load(cx.floor()).solids.len(), 1);
        assert_eq!(cx.undo_label(), Some("Convert Polyline to Solid"));
        cx.undo();
        assert_eq!(cx.floor().cad.len(), 1);
        assert!(DetailsLayer::load(cx.floor()).solids.is_empty());
    }
}
