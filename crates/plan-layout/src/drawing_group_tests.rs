//! Drawing Groups in the Layout window: a plan box draws its CAD in drawing
//! group order (LAY-36).

use crate::canvas::Prim;
use crate::extent::SceneSource;
use crate::tests::two_room_house;
use crate::*;
use plan_core::{CadItem, ObjectRef, Point};
use plan_docs::Scale;

fn text_order(p: &plan_core::Project) -> (usize, usize) {
    let mut b = LayoutBox::new(
        1,
        (Point::new(1.0, 1.0), Point::new(16.0, 14.0)),
        BoxSource::PlanView {
            floor: 0,
            layer_set: "All".into(),
        },
        Scale::QuarterInch,
    );
    b.border = false;
    let cx = LayoutRenderContext::new(p);
    let prims = crate::render::box_prims_for_test(&b, &cx, &SceneSource::new(None));
    let at = |word: &str| {
        prims
            .iter()
            .position(|q| matches!(q, Prim::Text { text, .. } if text == word))
            .unwrap_or_else(|| panic!("{word} is drawn"))
    };
    (at("FIRSTNOTE"), at("SECONDNOTE"))
}

#[test]
fn a_plan_box_draws_cad_in_drawing_group_order() {
    let mut p = two_room_house();
    let ids: Vec<u64> = ["FIRSTNOTE", "SECONDNOTE"]
        .iter()
        .enumerate()
        .map(|(i, t)| {
            p.add_cad(
                0,
                "CAD, Default",
                CadItem::Text {
                    pos: Point::new(60.0 + 40.0 * i as f64, 60.0),
                    text: (*t).into(),
                    height: 6.0,
                    angle: 0.0,
                },
            )
        })
        .collect();
    let (a, b) = text_order(&p);
    assert!(a < b, "drawn in the order they were made");
    p.drawing_group_to_front(0, &[ObjectRef::Cad(ids[0])]);
    let (a, b) = text_order(&p);
    assert!(b < a, "the first note is in front of the second now");
    // A kind default moves every CAD object: both under the walls, same order.
    p.drawing_group_defaults.set("CAD", 10);
    p.drawing_group_defaults.set("Text", 10);
    p.set_drawing_groups(0, &[ObjectRef::Cad(ids[0])], None);
    let (a, b) = text_order(&p);
    assert!(a < b, "back in creation order within one group");
}
