//! Embedded TrueType fonts in the PDF writer.

use super::tests::{check_xref, count};
use super::truetype::{face_count, synthetic_font, Font};
use super::*;
use std::collections::HashMap;

#[derive(Debug)]
struct Fonts(HashMap<String, Arc<Vec<u8>>>);

impl FontSource for Fonts {
    fn face(&self, spec: &FontSpec) -> Option<FontFace> {
        let data = self.0.get(&spec.family.to_lowercase())?.clone();
        Some(FontFace { data, index: 0 })
    }
}

fn source(fonts: &[(&str, Vec<u8>)]) -> Option<Arc<dyn FontSource>> {
    Some(Arc::new(Fonts(
        fonts
            .iter()
            .map(|(n, d)| (n.to_lowercase(), Arc::new(d.clone())))
            .collect(),
    )))
}

fn doc_with(fonts: &[(&str, Vec<u8>)]) -> PdfDoc {
    let mut d = PdfDoc::new(200.0, 100.0);
    d.set_font_source(source(fonts));
    d
}

/// The bytes between `stream\n` and `\nendstream` of the first object whose
/// dictionary contains `key`.
fn stream_of(pdf: &[u8], key: &str) -> Vec<u8> {
    let at = pdf
        .windows(key.len())
        .position(|w| w == key.as_bytes())
        .expect("key");
    let start = at + pdf[at..].windows(7).position(|w| w == b"stream\n").unwrap() + 7;
    let end = start
        + pdf[start..]
            .windows(10)
            .position(|w| w == b"\nendstream")
            .unwrap();
    pdf[start..end].to_vec()
}

#[test]
fn text_in_an_installed_font_embeds_a_truetype_subset() {
    let mut d = doc_with(&[("Test Sans", synthetic_font("Test Sans", "Regular", 400, 0))]);
    assert!(d.use_font(Some(&FontSpec::new("Test Sans", false, false))));
    d.text(10.0, 20.0, 12.0, "AB");
    d.text(10.0, 40.0, 12.0, "A (B)");
    d.use_font(None);
    d.set_font_bold(false);
    d.text(10.0, 60.0, 12.0, "plain");
    assert_eq!(d.embedded_font_count(), 1);
    let pdf = d.finish();
    check_xref(&pdf);
    let t = String::from_utf8_lossy(&pdf);
    assert!(t.contains("/FontFile2"));
    assert!(t.contains("/Subtype/TrueType"));
    assert!(t.contains("-Test-Sans-Regular") || t.contains("+Test-Sans-Regular"));
    assert!(t.contains("BT /E1 12 Tf 10 20 Td (AB) Tj ET"));
    assert!(t.contains("BT /F1 12 Tf 10 60 Td (plain) Tj ET"));
    assert!(
        t.contains("/E1 "),
        "the page resources list the embedded font"
    );
    assert_eq!(count(&pdf, "/FontFile2"), 1, "one subset per font");
    // The embedded file is a valid font holding A and B only.
    let file = stream_of(&pdf, "/Length1");
    assert_eq!(face_count(&file), 1);
    let f = Font::parse(Arc::new(file), 0).expect("the subset parses");
    assert!(f.glyph_index('A').is_some() && f.glyph_index('B').is_some());
    assert_eq!(f.glyph_index('C'), None);
    // Widths cover FirstChar..LastChar of the codes used: ' ' '(' ')' 'A' 'B'.
    assert!(t.contains("/FirstChar 32 /LastChar 66"));
}

#[test]
fn a_missing_family_falls_back_to_helvetica_with_one_note() {
    let mut d = doc_with(&[("Test Sans", synthetic_font("Test Sans", "Regular", 400, 0))]);
    let missing = FontSpec::new("Avenir", true, false);
    assert!(!d.use_font(Some(&missing)));
    d.text(5.0, 5.0, 10.0, "bold fallback");
    assert!(!d.use_font(Some(&missing)));
    assert_eq!(d.font_notes().len(), 1);
    assert!(d.font_notes()[0].contains("\"Avenir\" is not installed"));
    assert_eq!(d.embedded_font_count(), 0);
    assert!(d.is_bold());
    let pdf = d.finish();
    check_xref(&pdf);
    let t = String::from_utf8_lossy(&pdf);
    assert!(t.contains("BT /F2 10 Tf 5 5 Td (bold fallback)"));
    assert_eq!(count(&pdf, "/FontFile2"), 0);
}

#[test]
fn no_source_means_helvetica_without_a_note() {
    let mut d = PdfDoc::new(100.0, 100.0);
    d.set_font_source(None);
    assert!(!d.use_font(Some(&FontSpec::new("Arial", false, false))));
    assert!(d.font_notes().is_empty());
    d.text(1.0, 1.0, 9.0, "x");
    let pdf = d.finish();
    assert!(String::from_utf8_lossy(&pdf).contains("BT /F1 9 Tf"));
}

#[test]
fn a_font_that_forbids_embedding_is_not_embedded() {
    let mut d = doc_with(&[("Locked", synthetic_font("Locked", "Regular", 400, 0x0002))]);
    assert!(!d.use_font(Some(&FontSpec::new("Locked", false, false))));
    assert!(d.font_notes()[0].contains("does not allow embedding"));
}

#[test]
fn an_unreadable_font_file_falls_back() {
    let mut d = doc_with(&[("Junk", b"definitely not a font".to_vec())]);
    assert!(!d.use_font(Some(&FontSpec::new("Junk", false, false))));
    assert!(d.font_notes()[0].contains("could not be read"));
}

#[test]
fn widths_follow_the_embedded_font_and_two_specs_share_one_subset() {
    let mut d = doc_with(&[("Test Sans", synthetic_font("Test Sans", "Regular", 400, 0))]);
    let helv = d.current_text_width("AB", 10.0);
    d.use_font(Some(&FontSpec::new("Test Sans", false, false)));
    // A is 600 and B 700 units of 1000 in the test font.
    assert!((d.current_text_width("AB", 10.0) - 13.0).abs() < 1e-9);
    assert!((helv - d.current_text_width("AB", 10.0)).abs() > 0.1);
    d.text_centered(100.0, 10.0, 10.0, "AB");
    let mut again = FontSpec::new("Test Sans", false, false);
    again.style = "Regular".into();
    d.use_font(Some(&again));
    d.text(0.0, 0.0, 8.0, "A");
    assert_eq!(d.embedded_font_count(), 1);
    let pdf = d.finish();
    assert_eq!(count(&pdf, "/FontFile2"), 1);
    assert!(String::from_utf8_lossy(&pdf).contains("Td (AB) Tj"));
}

#[test]
fn rotated_text_and_bookmarks_keep_the_file_valid_with_embedded_fonts() {
    let mut d = doc_with(&[("Test Sans", synthetic_font("Test Sans", "Regular", 400, 0))]);
    d.add_bookmark("First");
    d.use_font(Some(&FontSpec::new("Test Sans", false, false)));
    d.text_rotated(10.0, 10.0, 9.0, 90.0, "AC");
    d.new_page();
    d.add_bookmark("Second");
    d.text(10.0, 10.0, 9.0, "B");
    let pdf = d.finish();
    check_xref(&pdf);
    assert_eq!(count(&pdf, "/Type /Page "), 2);
    assert_eq!(count(&pdf, "/Type/Outlines"), 1);
    assert_eq!(count(&pdf, "/FontFile2"), 1);
}

#[test]
fn the_default_source_serves_new_documents_and_measures_text() {
    // A family no other test asks for, so a parallel test is not disturbed.
    let font = synthetic_font("Default Only", "Regular", 400, 0);
    set_default_font_source(source(&[("Default Only", font)]));
    let spec = FontSpec::new("Default Only", false, false);
    let w = text_width_in(&spec, "AB", 10.0).expect("measured");
    assert!((w - 13.0).abs() < 1e-9);
    assert_eq!(
        text_width_in(&FontSpec::new("Absent", false, false), "AB", 10.0),
        None
    );
    let mut d = PdfDoc::new(50.0, 50.0);
    assert!(d.use_font(Some(&spec)));
    d.text(1.0, 1.0, 9.0, "A");
    assert_eq!(
        d.finish()
            .windows(10)
            .filter(|w| *w == b"/FontFile2")
            .count(),
        1
    );
    set_default_font_source(None);
    assert_eq!(text_width_in(&spec, "AB", 10.0), None);
}

#[test]
fn a_text_style_names_its_font_request() {
    let st = plan_core::TextStyle::plan_sized("x", 6.0, true)
        .with_font("Avenir Book")
        .with_italic(true);
    let spec = FontSpec::of_style(&st).unwrap();
    assert_eq!(
        (
            spec.family.as_str(),
            spec.style.as_str(),
            spec.bold,
            spec.italic
        ),
        ("Avenir", "Book", true, true)
    );
    let none = plan_core::TextStyle::plan_sized("x", 6.0, false).with_font(" ");
    assert_eq!(FontSpec::of_style(&none), None);
}

/// Real system fonts (macOS paths), written out for inspection with external
/// tools when `PLAN_FONT_TEST_OUT` names a directory.
#[test]
#[ignore = "needs the macOS system fonts"]
fn real_system_fonts_embed() {
    let arial = std::fs::read("/System/Library/Fonts/Supplemental/Arial.ttf").unwrap();
    let avenir = std::fs::read("/System/Library/Fonts/Avenir.ttc").unwrap();
    #[derive(Debug)]
    struct Real(Vec<u8>, Vec<u8>);
    impl FontSource for Real {
        fn face(&self, spec: &FontSpec) -> Option<FontFace> {
            match spec.family.as_str() {
                "Arial" => Some(FontFace {
                    data: Arc::new(self.0.clone()),
                    index: 0,
                }),
                // Avenir.ttc: face 0 Book, face 4 Heavy... find by name.
                "Avenir" => {
                    let data = Arc::new(self.1.clone());
                    let want = if spec.bold { "Heavy" } else { "Book" };
                    (0..truetype::face_count(&data)).find_map(|i| {
                        let f = truetype::Font::parse(data.clone(), i)?;
                        (f.info().style == want).then(|| FontFace {
                            data: data.clone(),
                            index: i,
                        })
                    })
                }
                _ => None,
            }
        }
    }
    let mut d = PdfDoc::new(400.0, 200.0);
    d.set_font_source(Some(Arc::new(Real(arial, avenir))));
    d.use_font(Some(&FontSpec::new("Arial", false, false)));
    d.text(
        10.0,
        170.0,
        18.0,
        "Arial: Kitchen 12'-6\" x 14'-0\" (Plan) \u{2022} \u{2013} \u{e9}",
    );
    d.use_font(Some(&FontSpec::new("Avenir", false, false)));
    d.text(10.0, 140.0, 18.0, "Avenir Book: Great Room 20'-0\"");
    d.use_font(Some(&FontSpec::new("Avenir", true, false)));
    d.text(10.0, 110.0, 18.0, "Avenir Heavy: MASTER BEDROOM");
    d.text_rotated(380.0, 20.0, 12.0, 90.0, "rotated");
    d.use_font(Some(&FontSpec::new("Nonesuch", false, false)));
    d.text(10.0, 60.0, 12.0, "fallback Helvetica");
    assert_eq!(d.embedded_font_count(), 3);
    assert_eq!(d.font_notes().len(), 1);
    let pdf = d.finish();
    check_xref(&pdf);
    eprintln!("pdf bytes: {}", pdf.len());
    assert!(pdf.len() < 150_000, "subsets keep the file small");
    if let Ok(dir) = std::env::var("PLAN_FONT_TEST_OUT") {
        std::fs::write(format!("{dir}/fonts.pdf"), &pdf).unwrap();
    }
}
