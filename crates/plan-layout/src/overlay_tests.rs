//! The plan overlay hook: cabinets, stairs and symbols in plan boxes.

use crate::canvas::{Dash, Prim};
use crate::extent::SceneSource;
use crate::tests::two_room_house;
use crate::*;
use plan_core::Point;
use plan_docs::Scale;

fn plan_box() -> LayoutBox {
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
    b
}

fn prims_with(cx: &LayoutRenderContext) -> Vec<Prim> {
    crate::render::box_prims_for_test(&plan_box(), cx, &SceneSource::new(None))
}

fn some_items() -> Vec<PlanOverlayItem> {
    let sq = vec![
        Point::new(20.0, 20.0),
        Point::new(60.0, 20.0),
        Point::new(60.0, 40.0),
        Point::new(20.0, 40.0),
    ];
    vec![
        PlanOverlayItem {
            layer: "Cabinets, Base".into(),
            shape: OverlayShape::Fill {
                points: sq.clone(),
                rgb: [200, 0, 0],
                alpha: 0.5,
            },
            dashed: false,
            weight: 1.0,
        },
        PlanOverlayItem::line("Cabinets, Base", sq, true),
        PlanOverlayItem::line(
            "Stairs",
            vec![Point::new(70.0, 20.0), Point::new(70.0, 80.0)],
            false,
        )
        .dashed()
        .weighted(0.75),
        PlanOverlayItem {
            layer: "Stairs".into(),
            shape: OverlayShape::Text {
                at: Point::new(75.0, 30.0),
                text: "UP".into(),
                height: 4.0,
                angle: 0.0,
            },
            dashed: false,
            weight: 1.0,
        },
    ]
}

#[test]
fn a_plan_box_without_an_overlay_hook_draws_none_of_it() {
    let p = two_room_house();
    let cx = LayoutRenderContext::new(&p);
    let prims = prims_with(&cx);
    assert!(!prims
        .iter()
        .any(|q| matches!(q, Prim::Text { text, .. } if text == "UP")));
}

#[test]
fn the_overlay_draws_fills_lines_dashes_and_text_in_the_plan_box() {
    let p = two_room_house();
    let plain = prims_with(&LayoutRenderContext::new(&p));
    let cx = LayoutRenderContext::new(&p).with_plan_overlay(|floor| {
        if floor == 0 {
            some_items()
        } else {
            Vec::new()
        }
    });
    let prims = prims_with(&cx);
    let strokes = |v: &[Prim]| {
        v.iter()
            .filter(|q| matches!(q, Prim::Stroke { .. }))
            .count()
    };
    let fills = |v: &[Prim]| v.iter().filter(|q| matches!(q, Prim::Fill { .. })).count();
    assert_eq!(strokes(&prims), strokes(&plain) + 2);
    assert_eq!(fills(&prims), fills(&plain) + 1);
    assert!(prims
        .iter()
        .any(|q| matches!(q, Prim::Stroke { pen, .. } if pen.dash == Dash::Dashed)));
    assert!(prims
        .iter()
        .any(|q| matches!(q, Prim::Text { text, .. } if text == "UP")));
    // The half-transparent red mixes with the white sheet.
    let fill = prims
        .iter()
        .find_map(|q| match q {
            Prim::Fill {
                color: plan_docs::PdfColor::Rgb(r, g, b),
                ..
            } if *g > 100 => Some((*r, *g, *b)),
            _ => None,
        })
        .expect("the cabinet fill");
    assert_eq!(fill, (228, 128, 128));
}

#[test]
fn a_hidden_layer_hides_its_overlay_items() {
    let mut p = two_room_house();
    p.layers.add(plan_core::Layer::new("Stairs", [0, 0, 0], 25));
    p.layers.set_display("Stairs", false);
    let cx = LayoutRenderContext::new(&p).with_plan_overlay(|_| some_items());
    // The layer set "All" shows every layer, so use a named set.
    let mut b = plan_box();
    b.source = BoxSource::PlanView {
        floor: 0,
        layer_set: "Working".into(),
    };
    let prims = crate::render::box_prims_for_test(&b, &cx, &SceneSource::new(None));
    assert!(!prims
        .iter()
        .any(|q| matches!(q, Prim::Text { text, .. } if text == "UP")));
    assert!(prims.iter().any(|q| matches!(q, Prim::Fill { .. })));
}

#[test]
fn a_cad_detail_box_wraps_and_frames_its_text_boxes() {
    use plan_core::cad::CadAttrs;
    use plan_core::{CadItem, CadObject};
    let mut p = two_room_house();
    let mut f = plan_core::Floor::new("Eave Detail", 0.0);
    f.detail = Some(plan_core::details::CadDetailInfo::default());
    let item = CadObject {
        id: 900,
        layer: "CAD, Default".into(),
        item: CadItem::Text {
            pos: Point::new(0.0, 40.0),
            text: "Fascia board over the rafter tail with a drip edge".into(),
            height: 2.0,
            angle: 0.0,
        },
    };
    f.cad.push(item.clone());
    let mut a = CadAttrs::new(900);
    a.text_box.width = 30.0;
    a.text_box.border = true;
    f.cad_attrs.push(a);
    p.floors.push(f);
    let mut b = LayoutBox::new(
        1,
        (Point::new(1.0, 1.0), Point::new(10.0, 8.0)),
        BoxSource::CadDetail {
            name: "Eave Detail".into(),
            items: vec![item],
        },
        Scale::OneInch,
    );
    b.border = false;
    {
        let cx = LayoutRenderContext::new(&p);
        let prims = crate::render::box_prims_for_test(&b, &cx, &SceneSource::new(None));
        let words = prims
            .iter()
            .filter(|q| matches!(q, Prim::Text { .. }))
            .count();
        let frames = prims
            .iter()
            .filter(|q| matches!(q, Prim::Stroke { closed: true, .. }))
            .count();
        assert!(words >= 2, "the text wraps onto several lines: {words}");
        assert_eq!(frames, 1, "and has its frame");
    }
    // Without the detail's attributes it is one plain line, no frame.
    p.floors.last_mut().unwrap().cad_attrs.clear();
    let cx = LayoutRenderContext::new(&p);
    let plain = crate::render::box_prims_for_test(&b, &cx, &SceneSource::new(None));
    assert_eq!(
        plain
            .iter()
            .filter(|q| matches!(q, Prim::Text { .. }))
            .count(),
        1
    );
    assert!(!plain
        .iter()
        .any(|q| matches!(q, Prim::Stroke { closed: true, .. })));
}
