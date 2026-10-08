//! Text styles' fonts reach the layout PDF: an installed font is embedded as
//! a TrueType subset, a missing one falls back to Helvetica.

use crate::tests::two_room_house;
use crate::*;
use plan_core::{CadItem, Point};
use plan_docs::pdf::truetype::synthetic_font;
use plan_docs::pdf::{FontFace, FontSource, FontSpec};
use plan_docs::{Scale, SheetSize};
use std::sync::Arc;

#[derive(Debug)]
struct OneFont(Arc<Vec<u8>>);

impl FontSource for OneFont {
    fn face(&self, spec: &FontSpec) -> Option<FontFace> {
        (spec.family == "Layout Test Face").then(|| FontFace {
            data: self.0.clone(),
            index: 0,
        })
    }
}

fn text_of(pdf: &[u8]) -> String {
    pdf.iter().map(|&b| b as char).collect()
}

fn render_with_default_font(font: &str, bold: bool) -> String {
    let mut p = two_room_house();
    p.add_cad(
        0,
        "Text",
        CadItem::Text {
            pos: Point::new(100.0, 100.0),
            text: "AB".into(),
            height: 6.0,
            angle: 0.0,
        },
    );
    let st = p
        .text_styles
        .styles
        .iter_mut()
        .find(|s| s.name == "Default Text Style")
        .unwrap();
    st.font = font.into();
    st.bold = bold;
    let cx = LayoutRenderContext::new(&p);
    let mut l = Layout::new("t", SheetSize::ArchC);
    send_to_layout(
        &mut l,
        &cx,
        1,
        BoxSource::PlanView {
            floor: 0,
            layer_set: "Floor Plan".into(),
        },
        Scale::QuarterInch,
        None,
    );
    text_of(&render_pdf(&l, &cx))
}

/// The text op line that shows `(AB)`.
fn op_of_ab(t: &str) -> String {
    let at = t.find("(AB) Tj").expect("the CAD text is printed");
    let start = t[..at].rfind("BT").unwrap();
    t[start..at].to_string()
}

#[test]
fn a_text_styles_font_is_embedded_and_a_missing_one_falls_back() {
    plan_docs::pdf::set_default_font_source(Some(Arc::new(OneFont(Arc::new(synthetic_font(
        "Layout Test Face",
        "Regular",
        400,
        0,
    ))))));
    let embedded = render_with_default_font("Layout Test Face", false);
    assert!(embedded.contains("/FontFile2"));
    assert!(
        op_of_ab(&embedded).contains("/E1 "),
        "the text uses the embedded font: {}",
        op_of_ab(&embedded)
    );
    assert!(embedded.contains("/Subtype/TrueType"));
    // Other text on the page (dimension-free house, room labels...) stays
    // in the standard fonts alongside it.
    assert!(embedded.contains("/F1 ") || embedded.contains("/F2 "));

    let missing = render_with_default_font("No Such Face", false);
    assert!(!missing.contains("/FontFile2"));
    assert!(op_of_ab(&missing).contains("/F1 "));

    // A bold style in a missing family is Helvetica-Bold (it was always
    // regular before).
    let bold_missing = render_with_default_font("No Such Face", true);
    assert!(op_of_ab(&bold_missing).contains("/F2 "));
    plan_docs::pdf::set_default_font_source(None);

    // Without a source, an installed font cannot be found: Helvetica.
    let none = render_with_default_font("Layout Test Face", false);
    assert!(!none.contains("/FontFile2"));
}
