//! Scaled floor-plan sheet: border, title block, walls, openings, room labels.

use super::PdfDoc;
use crate::schedule::room_name;
use plan_core::{Floor, Opening, OpeningKind, Point, Project, Room, Wall};

/// Standard landscape sheet sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetSize {
    /// Architectural D, 24 x 36 in.
    ArchD,
    /// Architectural C, 18 x 24 in.
    ArchC,
    /// US Letter, 8.5 x 11 in.
    Letter,
    /// Tabloid / ledger, 11 x 17 in.
    Tabloid,
}

impl SheetSize {
    /// Landscape `(width, height)` in inches.
    pub fn inches(self) -> (f64, f64) {
        match self {
            SheetSize::ArchD => (36.0, 24.0),
            SheetSize::ArchC => (24.0, 18.0),
            SheetSize::Letter => (11.0, 8.5),
            SheetSize::Tabloid => (17.0, 11.0),
        }
    }

    /// Landscape `(width, height)` in PDF points.
    pub fn points(self) -> (f64, f64) {
        let (w, h) = self.inches();
        (w * 72.0, h * 72.0)
    }
}

/// Architectural drawing scales (paper inches per foot of plan).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    /// 1/2" = 1'-0"
    HalfInch,
    /// 1/4" = 1'-0"
    QuarterInch,
    /// 3/16" = 1'-0"
    ThreeSixteenths,
    /// 1/8" = 1'-0"
    EighthInch,
}

impl Scale {
    /// All scales, largest drawing first.
    pub const DESCENDING: [Scale; 4] = [
        Scale::HalfInch,
        Scale::QuarterInch,
        Scale::ThreeSixteenths,
        Scale::EighthInch,
    ];

    /// Paper inches per plan foot.
    pub fn inches_per_foot(self) -> f64 {
        match self {
            Scale::HalfInch => 0.5,
            Scale::QuarterInch => 0.25,
            Scale::ThreeSixteenths => 0.1875,
            Scale::EighthInch => 0.125,
        }
    }

    /// Points of paper per inch of plan: `72 * scale_in_per_ft / 12`.
    pub fn points_per_inch(self) -> f64 {
        72.0 * self.inches_per_foot() / 12.0
    }

    /// Scale note text, e.g. `1/4" = 1'-0"`.
    pub fn label(self) -> &'static str {
        match self {
            Scale::HalfInch => "1/2\" = 1'-0\"",
            Scale::QuarterInch => "1/4\" = 1'-0\"",
            Scale::ThreeSixteenths => "3/16\" = 1'-0\"",
            Scale::EighthInch => "1/8\" = 1'-0\"",
        }
    }

    /// The next smaller drawing scale, if any.
    pub fn smaller(self) -> Option<Scale> {
        let i = Scale::DESCENDING.iter().position(|s| *s == self)?;
        Scale::DESCENDING.get(i + 1).copied()
    }
}

/// Text for the title block strip.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TitleBlock {
    pub project_name: String,
    pub sheet_title: String,
    pub sheet_number: String,
    pub date: String,
    pub designer: String,
}

/// Result of [`plan_sheet`].
#[derive(Debug, Clone)]
pub struct PlanSheetResult {
    /// The finished one-page PDF.
    pub pdf: Vec<u8>,
    /// The scale actually drawn (the requested one, or the largest smaller
    /// one that fits).
    pub scale_used: Scale,
    /// `true` when the plan fit at the *requested* scale. When `false`,
    /// `scale_used` is smaller than requested; if even 1/8" = 1'-0" overflows
    /// the sheet the plan is still drawn at 1/8" and runs past the margins.
    pub fitted: bool,
}

/// Plan-space bounding box of all wall footprints: `(min, max)` in inches.
fn plan_bounds(f: &Floor) -> (Point, Point) {
    let mut it = f.walls.iter().flat_map(|w| w.footprint());
    let Some(first) = it.next() else {
        return (Point::ZERO, Point::ZERO);
    };
    it.fold((first, first), |(lo, hi), p| {
        (
            Point::new(lo.x.min(p.x), lo.y.min(p.y)),
            Point::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    })
}

/// Shorten `s` until it fits `max_w` points at `size`.
fn fit_text(s: &str, size: f64, max_w: f64) -> String {
    let mut t = s.to_string();
    while !t.is_empty() && PdfDoc::text_width(&t, size) > max_w {
        t.pop();
    }
    t
}

/// Page-space layout shared by the drawing routines.
struct Layout {
    margin: f64,
    title_h: f64,
    title_w: f64,
    sheet_w: f64,
    sheet_h: f64,
    pad: f64,
}

impl Layout {
    fn new(sheet: SheetSize) -> Self {
        let (w, h) = sheet.points();
        let large = matches!(sheet, SheetSize::ArchD | SheetSize::ArchC);
        Layout {
            margin: if large { 36.0 } else { 18.0 },
            title_h: (h * 0.06).clamp(40.0, 72.0),
            title_w: (w * 0.45).max(300.0).min(w * 0.8),
            sheet_w: w,
            sheet_h: h,
            pad: if large { 24.0 } else { 12.0 },
        }
    }

    /// Space available to the plan: `(x, y, w, h)` in points.
    fn plan_area(&self) -> (f64, f64, f64, f64) {
        (
            self.margin + self.pad,
            self.margin + self.title_h + self.pad,
            self.sheet_w - 2.0 * (self.margin + self.pad),
            self.sheet_h - 2.0 * self.margin - self.title_h - 2.0 * self.pad,
        )
    }
}

/// Print one floor of a project as a scaled PDF sheet.
///
/// The plan is centred in the area above the title block. If it does not fit
/// at `scale`, successively smaller scales are tried; see [`PlanSheetResult`]
/// for how the outcome is reported. `floor` is an index into
/// `project.floors`; an out-of-range index prints an empty sheet.
pub fn plan_sheet(
    project: &Project,
    floor: usize,
    rooms: &[Room],
    sheet: SheetSize,
    scale: Scale,
    title_block: &TitleBlock,
) -> PlanSheetResult {
    let empty = Floor::new("", 0.0);
    let f = project.floors.get(floor).unwrap_or(&empty);
    let lay = Layout::new(sheet);
    let (lo, hi) = plan_bounds(f);
    let (plan_w, plan_h) = (hi.x - lo.x, hi.y - lo.y);
    let (ax, ay, aw, ah) = lay.plan_area();
    let fits = |s: Scale| {
        let k = s.points_per_inch();
        plan_w * k <= aw && plan_h * k <= ah
    };

    let mut used = scale;
    while !fits(used) {
        match used.smaller() {
            Some(s) => used = s,
            None => break,
        }
    }
    let fitted = used == scale;

    let k = used.points_per_inch();
    // Centre the plan bounding box in the plan area.
    let ox = ax + (aw - plan_w * k) * 0.5;
    let oy = ay + (ah - plan_h * k) * 0.5;
    let tp = |p: Point| (ox + (p.x - lo.x) * k, oy + (p.y - lo.y) * k);

    let mut doc = PdfDoc::new(lay.sheet_w, lay.sheet_h);
    draw_border(&mut doc, &lay);
    draw_title_block(&mut doc, &lay, title_block, used);

    // Scale note, bottom-left of the sheet.
    let note_size = (lay.title_h * 0.26).clamp(8.0, 14.0);
    let mut note = used.label().to_string();
    if !fitted {
        note.push_str("  (reduced to fit sheet)");
    }
    doc.set_gray(0.0);
    doc.text(lay.margin + 12.0, lay.margin + 12.0, note_size, &note);

    draw_walls(&mut doc, f, &tp);
    for o in &f.openings {
        if let Some(w) = f.wall(o.wall_id) {
            draw_opening(&mut doc, w, o, &tp, k);
        }
    }
    draw_room_labels(&mut doc, f, rooms, &tp, sheet);

    PlanSheetResult {
        pdf: doc.finish(),
        scale_used: used,
        fitted,
    }
}

fn draw_border(doc: &mut PdfDoc, lay: &Layout) {
    doc.rect(
        lay.margin,
        lay.margin,
        lay.sheet_w - 2.0 * lay.margin,
        lay.sheet_h - 2.0 * lay.margin,
        2.0,
    );
}

/// Chief-like title block: two rows of labelled boxes along the bottom-right.
fn draw_title_block(doc: &mut PdfDoc, lay: &Layout, tb: &TitleBlock, scale: Scale) {
    let x0 = lay.sheet_w - lay.margin - lay.title_w;
    let y0 = lay.margin;
    let row_h = lay.title_h * 0.5;
    let label_size = (row_h * 0.22).clamp(4.5, 7.0);
    let value_size = (row_h * 0.38).clamp(7.0, 12.0);

    let sheet_text = match (tb.sheet_number.is_empty(), tb.sheet_title.is_empty()) {
        (false, false) => format!("{}  {}", tb.sheet_number, tb.sheet_title),
        (false, true) => tb.sheet_number.clone(),
        _ => tb.sheet_title.clone(),
    };
    // (label, value, x fraction start, x fraction end), top row then bottom row.
    let top = [
        ("PROJECT", tb.project_name.as_str(), 0.0, 0.55),
        ("SHEET", sheet_text.as_str(), 0.55, 1.0),
    ];
    let bottom = [
        ("SCALE", scale.label(), 0.0, 0.34),
        ("DATE", tb.date.as_str(), 0.34, 0.62),
        ("DRAWN BY", tb.designer.as_str(), 0.62, 1.0),
    ];
    doc.rect(x0, y0, lay.title_w, lay.title_h, 1.5);
    for (row, y) in [(&top[..], y0 + row_h), (&bottom[..], y0)] {
        for (label, value, f0, f1) in row {
            let bx = x0 + f0 * lay.title_w;
            let bw = (f1 - f0) * lay.title_w;
            doc.rect(bx, y, bw, row_h, 0.75);
            doc.set_gray(0.35);
            doc.text(bx + 3.0, y + row_h - label_size - 2.0, label_size, label);
            doc.set_gray(0.0);
            let v = fit_text(value, value_size, bw - 8.0);
            doc.text(bx + 4.0, y + 4.0, value_size, &v);
        }
    }
}

fn pt_list(pts: &[Point], tp: &impl Fn(Point) -> (f64, f64)) -> Vec<(f64, f64)> {
    pts.iter().map(|p| tp(*p)).collect()
}

/// Walls as a gray union with a black outline.
///
/// Pass 1 strokes every footprint with a double-width black line; pass 2
/// fills every footprint gray on top. Edges shared between joined walls are
/// covered by the neighbouring fill, leaving one clean outline around the
/// union without computing mitred outlines.
fn draw_walls(doc: &mut PdfDoc, f: &Floor, tp: &impl Fn(Point) -> (f64, f64)) {
    for w in &f.walls {
        doc.polyline(&pt_list(&w.footprint(), tp), true, 1.4);
    }
    for w in &f.walls {
        doc.filled_polygon(&pt_list(&w.footprint(), tp), 0.8);
    }
}

/// Clear the wall under an opening and draw jambs plus the door swing or
/// window lines.
fn draw_opening(
    doc: &mut PdfDoc,
    w: &Wall,
    o: &Opening,
    tp: &impl Fn(Point) -> (f64, f64),
    k: f64,
) {
    let (n, half) = (w.normal(), w.thickness * 0.5);
    let (s, e) = (o.start_offset(), o.end_offset());
    // The gap overshoots the faces a little so the outline strokes vanish.
    let g = half + 1.5 / k;
    let at = |off: f64, side: f64| w.point_at(off).add(n.scale(side));
    doc.filled_polygon(
        &pt_list(&[at(s, g), at(e, g), at(e, -g), at(s, -g)], tp),
        1.0,
    );
    doc.set_gray(0.0);
    for off in [s, e] {
        let (a, b) = (tp(at(off, half)), tp(at(off, -half)));
        doc.line(a.0, a.1, b.0, b.1, 0.8);
    }
    match o.kind {
        OpeningKind::Window => {
            for (side, width) in [(half, 0.8), (0.0, 0.4), (-half, 0.8)] {
                let (a, b) = (tp(at(s, side)), tp(at(e, side)));
                doc.line(a.0, a.1, b.0, b.1, width);
            }
        }
        OpeningKind::Door => {
            // Standard: hinge at the wall-start jamb, swing to the left.
            let (hinge_off, other_off, sign) = if o.swing_flipped {
                (e, s, -1.0)
            } else {
                (s, e, 1.0)
            };
            let swing = n.scale(sign);
            let hinge = w.point_at(hinge_off).add(swing.scale(half));
            let closed_dir = w
                .point_at(other_off)
                .sub(w.point_at(hinge_off))
                .normalized();
            let leaf_end = hinge.add(swing.scale(o.width));
            let (a, b) = (tp(hinge), tp(leaf_end));
            doc.line(a.0, a.1, b.0, b.1, 0.6);
            let a0 = closed_dir.angle().to_degrees();
            let mut sweep = swing.angle().to_degrees() - a0;
            while sweep > 180.0 {
                sweep -= 360.0;
            }
            while sweep <= -180.0 {
                sweep += 360.0;
            }
            doc.set_line_width(0.4);
            doc.arc(a.0, a.1, o.width * k, a0, a0 + sweep);
        }
    }
}

fn draw_room_labels(
    doc: &mut PdfDoc,
    f: &Floor,
    rooms: &[Room],
    tp: &impl Fn(Point) -> (f64, f64),
    sheet: SheetSize,
) {
    let size = if matches!(sheet, SheetSize::ArchD | SheetSize::ArchC) {
        10.0
    } else {
        7.0
    };
    doc.set_gray(0.0);
    for r in rooms {
        let (cx, cy) = tp(r.centroid);
        doc.text_centered(cx, cy + size * 0.2, size, &room_name(f, r));
        let area = format!("{:.0} SF", r.area_sq_ft());
        doc.text_centered(cx, cy - size * 1.0, size * 0.85, &area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::tests::{check_xref, count};
    use crate::test_support::{house, rect_walls};
    use plan_core::{detect_rooms, WallKind};

    fn tb() -> TitleBlock {
        TitleBlock {
            project_name: "Smith Residence".into(),
            sheet_title: "First Floor Plan".into(),
            sheet_number: "A-101".into(),
            date: "2026-10-07".into(),
            designer: "DAD".into(),
        }
    }

    fn text_of(pdf: &[u8]) -> String {
        pdf.iter().map(|&b| b as char).collect()
    }

    #[test]
    fn house_fits_arch_d_quarter_inch() {
        let p = house();
        let rooms = detect_rooms(&p.floors[0].walls, 1.0);
        let r = plan_sheet(&p, 0, &rooms, SheetSize::ArchD, Scale::QuarterInch, &tb());
        assert!(r.fitted);
        assert_eq!(r.scale_used, Scale::QuarterInch);
        assert!(r.pdf.starts_with(b"%PDF-1.4"));
        assert!(r.pdf.ends_with(b"%%EOF"));
        assert_eq!(count(&r.pdf, "/Type /Page"), 1);
        check_xref(&r.pdf);
        let t = text_of(&r.pdf);
        for label in ["PROJECT", "SHEET", "SCALE", "DATE", "DRAWN BY"] {
            assert!(t.contains(label), "missing {label}");
        }
        assert!(t.contains("Smith Residence"));
        assert!(t.contains("1/4\" = 1'-0\""));
        assert!(t.contains("/MediaBox [0 0 2592 1728]"));
        // Room label and area, drawn at the centroid: 40' x 30' centerline.
        assert!(t.contains("1200 SF"));
    }

    #[test]
    fn oversize_plan_steps_down_scale() {
        let mut p = plan_core::Project::new("big");
        // 100' x 60' overflows Letter at every scale, so it lands on 1/8".
        rect_walls(&mut p, 1200.0, 720.0, 6.5, WallKind::Exterior);
        let r = plan_sheet(
            &p,
            0,
            &[],
            SheetSize::Letter,
            Scale::QuarterInch,
            &TitleBlock::default(),
        );
        assert!(!r.fitted);
        assert_eq!(r.scale_used, Scale::EighthInch);
        assert!(text_of(&r.pdf).contains("reduced to fit sheet"));
        check_xref(&r.pdf);
    }

    #[test]
    fn scale_math() {
        assert!((Scale::QuarterInch.points_per_inch() - 1.5).abs() < 1e-12);
        assert!((Scale::EighthInch.points_per_inch() - 0.75).abs() < 1e-12);
        assert_eq!(Scale::EighthInch.smaller(), None);
        assert_eq!(Scale::HalfInch.smaller(), Some(Scale::QuarterInch));
        assert_eq!(SheetSize::ArchD.points(), (2592.0, 1728.0));
    }

    #[test]
    fn empty_or_bad_floor_still_produces_pdf() {
        let p = plan_core::Project::new("empty");
        let r = plan_sheet(
            &p,
            5,
            &[],
            SheetSize::Tabloid,
            Scale::HalfInch,
            &TitleBlock::default(),
        );
        assert!(r.fitted);
        check_xref(&r.pdf);
    }
}
