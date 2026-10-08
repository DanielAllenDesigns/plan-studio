//! A minimal, dependency-free PDF 1.4 writer and the plan-sheet renderer.
//!
//! [`PdfDoc`] collects drawing operators per page (uncompressed content
//! streams, one shared Helvetica font resource) and [`PdfDoc::finish`]
//! serialises them with a correct cross-reference table. Coordinates are PDF
//! points (1/72"), origin at the bottom-left, Y up, which matches plan space.
//!
//! [`plan_sheet`] builds on it to print a floor at an architectural scale.

mod sheet;

pub use sheet::{plan_sheet, PlanSheetResult, Scale, SheetSize, TitleBlock};

use std::fmt::Write as _;

/// Helvetica advance widths (1/1000 em) for ASCII 32..=126.
const HELVETICA_WIDTHS: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278,
    278, // space .. /
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, // 0-9
    278, 278, 584, 584, 584, 556, 1015, // : .. @
    667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, // A-M
    722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, // N-Z
    278, 278, 278, 469, 556, 333, // [ \ ] ^ _ `
    556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, // a-m
    556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, // n-z
    334, 260, 334, 584, // { | } ~
];

/// Format a number for a content stream: up to 3 decimals, no trailing zeros.
fn num(v: f64) -> String {
    let mut s = format!("{v:.3}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".into();
    }
    s
}

/// Map a char to its WinAnsi byte (`?` when unrepresentable).
fn winansi(c: char) -> u8 {
    match c {
        '\u{20}'..='\u{7e}' | '\u{a0}'..='\u{ff}' => c as u32 as u8,
        '\u{2022}' => 0x95,
        '\u{2013}' => 0x96,
        '\u{2014}' => 0x97,
        '\u{2018}' => 0x91,
        '\u{2019}' => 0x92,
        '\u{201c}' => 0x93,
        '\u{201d}' => 0x94,
        '\u{2026}' => 0x85,
        '\u{20ac}' => 0x80,
        _ => b'?',
    }
}

/// Escape a string as a PDF literal string body (without the parentheses).
fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.chars().map(winansi) {
        match b {
            b'(' | b')' | b'\\' => {
                out.push('\\');
                out.push(b as char);
            }
            0x20..=0x7e => out.push(b as char),
            _ => {
                let _ = write!(out, "\\{b:03o}");
            }
        }
    }
    out
}

/// A multi-page PDF under construction.
#[derive(Debug, Clone)]
pub struct PdfDoc {
    width: f64,
    height: f64,
    pages: Vec<String>,
    gray: f64,
    line_width: f64,
}

impl PdfDoc {
    /// A new document whose pages are `width_pt` x `height_pt`, with one
    /// blank page already started. Initial state: black, 0.5 pt lines.
    pub fn new(width_pt: f64, height_pt: f64) -> Self {
        let mut doc = Self {
            width: width_pt,
            height: height_pt,
            pages: vec![String::new()],
            gray: 0.0,
            line_width: 0.5,
        };
        doc.emit_state();
        doc
    }

    /// Number of pages so far.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    fn emit(&mut self, s: &str) {
        if let Some(p) = self.pages.last_mut() {
            p.push_str(s);
        }
    }

    fn emit_state(&mut self) {
        let s = format!(
            "{g} g {g} G {w} w\n",
            g = num(self.gray),
            w = num(self.line_width)
        );
        self.emit(&s);
    }

    /// Start a new page, carrying over the current gray and line width.
    pub fn new_page(&mut self) {
        self.pages.push(String::new());
        self.emit_state();
    }

    /// Set the fill and stroke colour (0 = black, 1 = white).
    pub fn set_gray(&mut self, g: f64) {
        self.gray = g.clamp(0.0, 1.0);
        let s = format!("{g} g {g} G\n", g = num(self.gray));
        self.emit(&s);
    }

    /// Set the line width used by [`PdfDoc::arc`] (other strokes pass their own).
    pub fn set_line_width(&mut self, width_pt: f64) {
        self.line_width = width_pt.max(0.0);
        let s = format!("{} w\n", num(self.line_width));
        self.emit(&s);
    }

    /// Stroke a straight line.
    pub fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, width_pt: f64) {
        let s = format!(
            "q {} w {} {} m {} {} l S Q\n",
            num(width_pt),
            num(x1),
            num(y1),
            num(x2),
            num(y2)
        );
        self.emit(&s);
    }

    fn path(points: &[(f64, f64)]) -> String {
        let mut s = String::new();
        for (i, (x, y)) in points.iter().enumerate() {
            let op = if i == 0 { "m" } else { "l" };
            let _ = write!(s, "{} {} {op} ", num(*x), num(*y));
        }
        s
    }

    /// Stroke a polyline, optionally closed. Fewer than 2 points draws nothing.
    pub fn polyline(&mut self, points: &[(f64, f64)], closed: bool, width_pt: f64) {
        if points.len() < 2 {
            return;
        }
        let s = format!(
            "q {} w {}{} Q\n",
            num(width_pt),
            Self::path(points),
            if closed { "s" } else { "S" }
        );
        self.emit(&s);
    }

    /// Fill a polygon with a gray level (no outline). The current colour is
    /// unchanged afterwards.
    pub fn filled_polygon(&mut self, points: &[(f64, f64)], gray: f64) {
        if points.len() < 3 {
            return;
        }
        let s = format!(
            "q {} g {}f Q\n",
            num(gray.clamp(0.0, 1.0)),
            Self::path(points)
        );
        self.emit(&s);
    }

    /// Stroke an axis-aligned rectangle with its lower-left corner at (x, y).
    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, width_pt: f64) {
        let s = format!(
            "q {} w {} {} {} {} re S Q\n",
            num(width_pt),
            num(x),
            num(y),
            num(w),
            num(h)
        );
        self.emit(&s);
    }

    /// Draw text with its baseline-left at (x, y) in Helvetica, using the
    /// current gray as fill. Characters are mapped to WinAnsi; parentheses and
    /// backslashes are escaped.
    pub fn text(&mut self, x: f64, y: f64, size_pt: f64, text: &str) {
        let s = format!(
            "BT /F1 {} Tf {} {} Td ({}) Tj ET\n",
            num(size_pt),
            num(x),
            num(y),
            escape_text(text)
        );
        self.emit(&s);
    }

    /// Approximate width of `text` in points (Helvetica metrics for ASCII,
    /// a mid-width guess for other characters).
    pub fn text_width(text: &str, size_pt: f64) -> f64 {
        let units: u32 = text
            .chars()
            .map(|c| match c {
                ' '..='~' => u32::from(HELVETICA_WIDTHS[c as usize - 32]),
                _ => 556,
            })
            .sum();
        f64::from(units) * size_pt / 1000.0
    }

    /// Draw text horizontally centred on `cx`.
    pub fn text_centered(&mut self, cx: f64, y: f64, size_pt: f64, text: &str) {
        let w = Self::text_width(text, size_pt);
        self.text(cx - w * 0.5, y, size_pt, text);
    }

    /// Stroke a circular arc centred at (cx, cy) from `start_deg` to
    /// `end_deg` (counter-clockwise when end > start, clockwise otherwise),
    /// approximated with cubic Bezier segments of at most 90 degrees, using
    /// the current line width.
    pub fn arc(&mut self, cx: f64, cy: f64, r: f64, start_deg: f64, end_deg: f64) {
        let sweep = end_deg - start_deg;
        if sweep == 0.0 || r <= 0.0 {
            return;
        }
        let segs = (sweep.abs() / 90.0).ceil().max(1.0) as usize;
        let step = (sweep / segs as f64).to_radians();
        let k = 4.0 / 3.0 * (step / 4.0).tan();
        let mut a = start_deg.to_radians();
        let at = |ang: f64| (cx + r * ang.cos(), cy + r * ang.sin());
        let (x0, y0) = at(a);
        let mut s = format!("{} {} m ", num(x0), num(y0));
        for _ in 0..segs {
            let b = a + step;
            let (p0x, p0y) = (a.cos(), a.sin());
            let (p3x, p3y) = (b.cos(), b.sin());
            let c1 = (cx + r * (p0x - k * p0y), cy + r * (p0y + k * p0x));
            let c2 = (cx + r * (p3x + k * p3y), cy + r * (p3y - k * p3x));
            let e = at(b);
            let _ = write!(
                s,
                "{} {} {} {} {} {} c ",
                num(c1.0),
                num(c1.1),
                num(c2.0),
                num(c2.1),
                num(e.0),
                num(e.1)
            );
            a = b;
        }
        s.push_str("S\n");
        self.emit(&s);
    }

    /// Serialise the document: header, objects, xref table and trailer.
    ///
    /// Object layout: 1 catalog, 2 page tree, 3 Helvetica, then a page object
    /// and a content stream per page. Non-page dictionaries write `/Type/X`
    /// without a space so a search for `/Type /Page` matches only pages.
    pub fn finish(self) -> Vec<u8> {
        let n_pages = self.pages.len();
        let mut objs: Vec<Vec<u8>> = Vec::new();
        objs.push(b"<< /Type/Catalog /Pages 2 0 R >>".to_vec());
        let kids: Vec<String> = (0..n_pages).map(|i| format!("{} 0 R", 4 + 2 * i)).collect();
        objs.push(
            format!(
                "<< /Kids [{}] /Count {n_pages} /Type/Pages >>",
                kids.join(" ")
            )
            .into_bytes(),
        );
        objs.push(
            b"<< /Type/Font /Subtype/Type1 /BaseFont/Helvetica /Encoding/WinAnsiEncoding >>"
                .to_vec(),
        );
        for (i, content) in self.pages.iter().enumerate() {
            objs.push(
                format!(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents {} 0 R \
                     /Resources << /Font << /F1 3 0 R >> >> >>",
                    num(self.width),
                    num(self.height),
                    5 + 2 * i
                )
                .into_bytes(),
            );
            let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
            stream.extend_from_slice(content.as_bytes());
            stream.extend_from_slice(b"endstream");
            objs.push(stream);
        }

        let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
        let mut offsets = Vec::with_capacity(objs.len());
        for (i, body) in objs.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n", objs.len() + 1).as_bytes());
        out.extend_from_slice(b"0000000000 65535 f \n");
        for off in offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF",
                objs.len() + 1
            )
            .as_bytes(),
        );
        out
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn count(hay: &[u8], needle: &str) -> usize {
        let n = needle.as_bytes();
        hay.windows(n.len()).filter(|w| *w == n).count()
    }

    /// Parse the xref table and check every offset lands on "N 0 obj".
    pub(crate) fn check_xref(pdf: &[u8]) {
        // Latin-1 view so byte offsets equal char offsets.
        let text: String = pdf.iter().map(|&b| b as char).collect();
        let sx = text.rfind("startxref\n").expect("startxref");
        let xref_off: usize = text[sx + 10..]
            .lines()
            .next()
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        // Char offsets above 127 take two bytes in `text`; map via a byte index.
        let bytes = pdf;
        assert!(bytes[xref_off..].starts_with(b"xref\n"));
        let tail = &text[text.rfind("xref\n0 ").unwrap()..];
        let mut lines = tail.lines().skip(1);
        let header: Vec<usize> = lines
            .next()
            .unwrap()
            .split_whitespace()
            .map(|t| t.parse().unwrap())
            .collect();
        assert_eq!(header[0], 0);
        let total = header[1];
        assert!(lines.next().unwrap().starts_with("0000000000 65535 f"));
        for obj in 1..total {
            let entry = lines.next().unwrap();
            assert_eq!(entry.len(), 19, "xref entries are 20 bytes with newline");
            assert!(entry.ends_with(" 00000 n "));
            let off: usize = entry[..10].parse().unwrap();
            let want = format!("{obj} 0 obj\n");
            assert!(
                bytes[off..].starts_with(want.as_bytes()),
                "object {obj} offset {off} wrong"
            );
        }
        assert!(text.contains(&format!("/Size {total}")));
    }

    #[test]
    fn structure_and_xref() {
        let mut d = PdfDoc::new(612.0, 792.0);
        d.line(0.0, 0.0, 100.0, 100.0, 1.0);
        d.new_page();
        d.text(10.0, 10.0, 12.0, "Hi (there) \\ ok");
        d.new_page();
        d.filled_polygon(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], 0.5);
        let pdf = d.finish();
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF"));
        assert_eq!(count(&pdf, "/Type /Page"), 3);
        assert_eq!(count(&pdf, "/Type/Pages"), 1);
        assert_eq!(count(&pdf, "Hi \\(there\\) \\\\ ok"), 1);
        check_xref(&pdf);
    }

    #[test]
    fn stream_length_matches() {
        let mut d = PdfDoc::new(100.0, 100.0);
        d.polyline(&[(0.0, 0.0), (5.5, 5.25)], true, 2.0);
        let pdf = d.finish();
        let text: String = pdf.iter().map(|&b| b as char).collect();
        let l: usize = text
            .split("/Length ")
            .nth(1)
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let start = text.find("stream\n").unwrap() + 7;
        let end = text.find("endstream").unwrap();
        assert_eq!(end - start, l);
    }

    #[test]
    fn winansi_and_widths() {
        assert_eq!(escape_text("5\u{b0}"), "5\\260");
        assert_eq!(escape_text("\u{2019}"), "\\222");
        assert_eq!(escape_text("\u{4e2d}"), "?");
        assert!((PdfDoc::text_width("AA", 10.0) - 13.34).abs() < 1e-9);
    }

    #[test]
    fn arc_uses_bezier_segments() {
        let mut d = PdfDoc::new(100.0, 100.0);
        d.arc(50.0, 50.0, 10.0, 0.0, 90.0);
        d.arc(50.0, 50.0, 10.0, 0.0, 180.0);
        let text = d.pages[0].clone();
        assert_eq!(text.matches(" c ").count(), 3);
        assert!(text.contains("60 50 m"));
        // Quarter-circle control points: k = 0.5523 * 10.
        assert!(text.contains("60 55.523 55.523 60 50 60 c"));
    }
}
