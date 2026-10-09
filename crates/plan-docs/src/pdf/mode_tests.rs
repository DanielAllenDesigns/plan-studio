//! Print modes: grayscale / black and white, uniform line width, the
//! transformation matrix, bookmarks and the standard-font metrics.

use super::tests::{check_xref, count};
use super::*;

fn text(pdf: &[u8]) -> String {
    pdf.iter().map(|&b| b as char).collect()
}

fn drawn(mode: PdfColorMode) -> Vec<u8> {
    let mut d = PdfDoc::new(200.0, 200.0);
    d.set_color_mode(mode);
    d.set_rgb_stroke(200, 30, 30);
    d.set_rgb_fill(30, 30, 200);
    d.line(0.0, 0.0, 100.0, 100.0, 1.0);
    d.text(10.0, 10.0, 10.0, "Hello");
    d.fill_rect(0.0, 0.0, 50.0, 50.0, PdfColor::Rgb(10, 200, 10));
    d.polygon(
        &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)],
        Some(PdfColor::Rgb(250, 248, 244)),
        None,
    );
    d.image_rgb(0.0, 0.0, 10.0, 10.0, 1, 1, &[255, 0, 0]);
    d.finish()
}

#[test]
fn colour_mode_keeps_rgb_operators() {
    let t = text(&drawn(PdfColorMode::Color));
    assert!(t.contains(" rg\n") && t.contains(" RG\n"));
}

#[test]
fn grayscale_has_no_rgb_colour_operators() {
    let pdf = drawn(PdfColorMode::Grayscale);
    let t = text(&pdf);
    assert!(!t.contains(" rg") && !t.contains(" RG"), "{t}");
    assert!(t.contains(" g\n") && t.contains(" G\n"));
    // The red pixel became its luminance, 0.299 * 255 = 76.
    assert!(pdf.windows(3).any(|w| w == [76, 76, 76]));
}

#[test]
fn black_and_white_uses_only_black_and_white() {
    let pdf = drawn(PdfColorMode::BlackWhite);
    let t = text(&pdf);
    assert!(!t.contains(" rg") && !t.contains(" RG"));
    // The paper-white fill stays white, the green fill (luma 0.59) turns white
    // and every ink is black.
    for line in t.lines() {
        for op in [" g", " G"] {
            if let Some(v) = line.strip_suffix(op) {
                let v = v.split_whitespace().last().unwrap();
                assert!(v == "0" || v == "1", "gray {v} in {line}");
            }
        }
    }
    assert!(pdf.windows(3).any(|w| w == [0, 0, 0]));
}

#[test]
fn uniform_line_width_replaces_every_stroke_width() {
    let mut d = PdfDoc::new(100.0, 100.0);
    d.set_uniform_line_width(Some(0.4));
    d.line(0.0, 0.0, 10.0, 10.0, 3.0);
    d.polyline(&[(0.0, 0.0), (5.0, 0.0), (5.0, 5.0)], true, 2.0);
    d.rect(1.0, 1.0, 5.0, 5.0, 1.5);
    d.set_line_width(2.5);
    let t = text(&d.finish());
    assert!(t.contains("q 0.4 w 0 0 m 10 10 l S Q"));
    assert!(t.contains("q 0.4 w 0 0 m 5 0 l 5 5 l s Q"));
    assert!(t.contains("q 0.4 w 1 1 5 5 re S Q"));
    assert!(!t.contains("3 w") && !t.contains("2 w") && !t.contains("2.5 w"));
}

#[test]
fn transform_writes_a_cm_matrix() {
    let mut d = PdfDoc::new(100.0, 100.0);
    d.save_state();
    d.transform([0.5, 0.0, 0.0, 0.5, 12.0, -3.0]);
    d.line(0.0, 0.0, 10.0, 0.0, 1.0);
    d.restore_state();
    assert!(text(&d.finish()).contains("0.5 0 0 0.5 12 -3 cm\n"));
}

#[test]
fn bookmarks_make_an_outline_that_points_at_the_pages() {
    let mut d = PdfDoc::new(100.0, 100.0);
    d.add_bookmark("A-1 Floor Plan");
    d.new_page();
    d.add_bookmark("A-2 (Roof)");
    let pdf = d.finish();
    check_xref(&pdf);
    let t = text(&pdf);
    assert_eq!(count(&pdf, "/Type/Outlines"), 1);
    assert!(t.contains("/Outlines "));
    assert!(t.contains("/Title (A-1 Floor Plan)"));
    assert!(t.contains("/Title (A-2 \\(Roof\\))"));
    assert!(t.contains("/Dest ["));
    assert_eq!(count(&pdf, "/Type /Page "), 2);
}

#[test]
fn standard_fonts_carry_widths_and_a_descriptor() {
    let mut d = PdfDoc::new(100.0, 100.0);
    d.text(1.0, 1.0, 10.0, "x");
    let pdf = d.finish();
    check_xref(&pdf);
    let t = text(&pdf);
    assert_eq!(count(&pdf, "/Type /FontDescriptor"), 2);
    assert_eq!(count(&pdf, "/FirstChar 32 /LastChar 126 /Widths ["), 2);
    assert!(t.contains("/FontDescriptor "));
}

// ------------------------------------------------------------ opacity --

#[test]
fn set_alpha_adds_an_ext_gstate_to_the_page_that_uses_it() {
    let mut d = PdfDoc::new(200.0, 200.0);
    d.line(0.0, 0.0, 10.0, 10.0, 1.0);
    d.new_page();
    d.save_state();
    d.set_alpha(0.3);
    d.set_alpha(0.3);
    d.text(10.0, 10.0, 12.0, "DRAFT");
    d.restore_state();
    let pdf = d.finish();
    let t = text(&pdf);
    // One resource for the level, named in the page that sets it.
    assert_eq!(
        count(&pdf, "/GSA30 << /Type/ExtGState /ca 0.3 /CA 0.3 >>"),
        1
    );
    assert_eq!(count(&pdf, "/GSA30 gs"), 2);
    let first = t.find("/Type /Page ").unwrap();
    let second = t[first + 10..].find("/Type /Page ").unwrap() + first + 10;
    assert!(
        !t[first..second].contains("ExtGState"),
        "page 1 sets no opacity"
    );
    assert!(t[second..].contains("ExtGState"));
    check_xref(&pdf);
}

#[test]
fn a_picture_with_transparent_parts_gets_a_soft_mask() {
    let mut d = PdfDoc::new(200.0, 200.0);
    // Two pixels: one solid red, one half transparent green.
    let rgba = [255u8, 0, 0, 255, 0, 255, 0, 128];
    assert!(d.image_rgba_placed(100.0, 100.0, 40.0, 20.0, 0.0, 2, 1, &rgba));
    // All-opaque pictures need no mask.
    assert!(d.image_rgba_placed(50.0, 50.0, 10.0, 10.0, 0.0, 1, 1, &[1, 2, 3, 255]));
    assert!(!d.image_rgba_placed(0.0, 0.0, 1.0, 1.0, 0.0, 2, 2, &[0; 4]));
    let pdf = d.finish();
    let t = text(&pdf);
    assert_eq!(count(&pdf, "/ColorSpace/DeviceGray"), 1);
    assert_eq!(count(&pdf, "/SMask "), 1);
    // The unturned picture: width 40, height 20, lower-left at (80, 90).
    assert!(t.contains("q 40 0 0 20 80 90 cm /Im1 Do Q"), "{t}");
    // The mask is the alpha plane.
    assert!(pdf.windows(2).any(|w| w == [255u8, 128]));
    check_xref(&pdf);
}

#[test]
fn a_turned_picture_is_placed_by_its_centre() {
    let mut d = PdfDoc::new(200.0, 200.0);
    let quarter = std::f64::consts::FRAC_PI_2;
    assert!(d.image_rgba_placed(100.0, 100.0, 40.0, 20.0, quarter, 1, 1, &[9, 9, 9, 255]));
    let t = text(&d.finish());
    // Turned a quarter: the x axis points up, the y axis points left; the
    // lower-left corner of the image is at (110, 80).
    assert!(t.contains("q 0 40 -20 0 110 80 cm /Im1 Do Q"), "{t}");
}
