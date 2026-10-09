//! Watermark (View > Watermark, Edit > Default Settings > Watermark): a text
//! or picture mark laid over a view and over every layout page, shown on
//! screen, in Print Preview and in the printed PDF when Include Watermark is
//! on (manual pp. 1437-1440).
//!
//! The mark is file-specific: [`WatermarkSettings`] lives in the plan
//! (`Project::print_setup`) with the list of views it is switched on in. This
//! module holds the settings and the placement maths: where the marks of a
//! Tile, Border or Fit to Sheet layout sit on a sheet and how they turn. The
//! drawing is done by whoever has the sheet (the PDF printer, the preview and
//! the plan canvas), so every one of them puts the marks in the same places.
//!
//! Units: paper inches, origin at the sheet's lower-left corner, y up.
//! Margins are `[top, bottom, left, right]`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Height of a capital A in Helvetica as a fraction of the font size: the
/// Print Size of a text watermark is measured baseline to the top of an A.
pub const CAP_HEIGHT_EM: f64 = 0.718;

/// Smallest and largest Print Size of a text mark, paper inches.
pub const MIN_PRINT_SIZE_IN: f64 = 0.05;
pub const MAX_PRINT_SIZE_IN: f64 = 24.0;
/// Most marks per row or column.
pub const MAX_MARKS: u32 = 40;

/// Text or a picture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatermarkKind {
    #[default]
    Text,
    Image,
}

impl WatermarkKind {
    pub const ALL: [WatermarkKind; 2] = [WatermarkKind::Text, WatermarkKind::Image];

    pub fn label(self) -> &'static str {
        match self {
            WatermarkKind::Text => "Text",
            WatermarkKind::Image => "Image",
        }
    }
}

/// How the marks are spread over the sheet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatermarkLayout {
    /// A grid of marks, Marks per Row by Marks per Column.
    #[default]
    Tile,
    /// Marks along the four edges of the area inside the margins.
    Border,
    /// One mark, grown or shrunk to fill the area inside the margins.
    FitToSheet,
}

impl WatermarkLayout {
    pub const ALL: [WatermarkLayout; 3] = [
        WatermarkLayout::Tile,
        WatermarkLayout::Border,
        WatermarkLayout::FitToSheet,
    ];

    pub fn label(self) -> &'static str {
        match self {
            WatermarkLayout::Tile => "Tile",
            WatermarkLayout::Border => "Border",
            WatermarkLayout::FitToSheet => "Fit to Sheet",
        }
    }
}

/// The Watermark Defaults dialog's answers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WatermarkSpec {
    /// Watermark Type.
    pub kind: WatermarkKind,
    /// Text: the words.
    pub text: String,
    /// Text: the colour.
    pub color: [u8; 3],
    /// Text: Print Size, baseline to the top of a capital A, paper inches.
    pub print_size_in: f64,
    /// Text: the font family (empty: Helvetica).
    pub font: String,
    /// Image: the picture file.
    pub image_path: String,
    /// Image: Ratio to Sheet, how far the picture reaches across the area
    /// inside the margins (0.01 to 1).
    pub image_ratio: f64,
    /// General: Layout.
    pub layout: WatermarkLayout,
    /// General: Angle from a horizontal line pointing right, degrees counter-clockwise.
    pub angle_deg: f64,
    /// General: Transparency, percent (0 solid, 100 invisible).
    pub transparency: f64,
    /// General: Marks per Row, across the width.
    pub marks_per_row: u32,
    /// General: Marks per Column, from top to bottom.
    pub marks_per_column: u32,
    /// Margins: Use Drawing Sheet Margin.
    pub use_sheet_margin: bool,
    /// Margins: Top, Bottom, Left, Right, paper inches.
    pub margins_in: [f64; 4],
    /// Preview: Update Automatically.
    pub update_automatically: bool,
}

impl Default for WatermarkSpec {
    fn default() -> Self {
        Self {
            kind: WatermarkKind::Text,
            text: "DRAFT".to_string(),
            color: [150, 150, 150],
            print_size_in: 0.5,
            font: String::new(),
            image_path: String::new(),
            image_ratio: 0.5,
            layout: WatermarkLayout::Tile,
            angle_deg: 30.0,
            transparency: 70.0,
            marks_per_row: 3,
            marks_per_column: 3,
            use_sheet_margin: true,
            margins_in: [0.0; 4],
            update_automatically: true,
        }
    }
}

impl WatermarkSpec {
    /// Opacity, 0 to 1.
    pub fn alpha(&self) -> f64 {
        (1.0 - self.transparency / 100.0).clamp(0.0, 1.0)
    }

    /// Font size of a text mark, points: the size at which a capital A is
    /// [`print_size_in`](Self::print_size_in) tall.
    pub fn font_size_pt(&self) -> f64 {
        self.print_size_in * 72.0 / CAP_HEIGHT_EM
    }

    /// Does the mark have anything to show (words or a picture file)?
    pub fn has_content(&self) -> bool {
        match self.kind {
            WatermarkKind::Text => !self.text.trim().is_empty(),
            WatermarkKind::Image => !self.image_path.trim().is_empty(),
        }
    }

    /// Why the answers cannot be used, if they cannot.
    pub fn problem(&self) -> Option<&'static str> {
        if self.kind == WatermarkKind::Text && self.text.trim().is_empty() {
            return Some("Type the watermark text");
        }
        if self.kind == WatermarkKind::Image && self.image_path.trim().is_empty() {
            return Some("Choose a watermark picture");
        }
        if !(MIN_PRINT_SIZE_IN..=MAX_PRINT_SIZE_IN).contains(&self.print_size_in) {
            return Some("The print size must be between 0.05 and 24 inches");
        }
        if !(0.01..=1.0).contains(&self.image_ratio) {
            return Some("The ratio to sheet must be between 1% and 100%");
        }
        if !(0.0..=100.0).contains(&self.transparency) {
            return Some("The transparency must be between 0 and 100 percent");
        }
        if self.marks_per_row == 0
            || self.marks_per_column == 0
            || self.marks_per_row > MAX_MARKS
            || self.marks_per_column > MAX_MARKS
        {
            return Some("Marks per row and per column must be between 1 and 40");
        }
        if self.margins_in.iter().any(|m| !(0.0..=100.0).contains(m)) {
            return Some("Margins must be zero or more");
        }
        None
    }
}

/// The file's watermark: the settings and where it is switched on.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WatermarkSettings {
    pub spec: WatermarkSpec,
    /// View keys View > Watermark is on in: `plan:<saved view name>`,
    /// `elevation`, `detail`, `layout` (see the `*_key` functions).
    pub on: BTreeSet<String>,
}

/// The key of a saved plan view.
pub fn plan_key(view: &str) -> String {
    format!("plan:{view}")
}

/// The key of the cross section / elevation views.
pub const ELEVATION_KEY: &str = "elevation";
/// The key of the CAD detail views.
pub const DETAIL_KEY: &str = "detail";
/// The key of the layout: on, the watermark shows on every page.
pub const LAYOUT_KEY: &str = "layout";

impl WatermarkSettings {
    /// Is the watermark on in the view `key`?
    pub fn is_on(&self, key: &str) -> bool {
        self.on.contains(key)
    }

    /// Switches the watermark in view `key`; returns the new state.
    pub fn set_on(&mut self, key: &str, on: bool) -> bool {
        if on {
            self.on.insert(key.to_string());
        } else {
            self.on.remove(key);
        }
        on
    }

    /// View > Watermark: flips the view `key`; returns the new state.
    pub fn toggle(&mut self, key: &str) -> bool {
        let now = !self.is_on(key);
        self.set_on(key, now)
    }

    /// Is this the state of a plan that never touched the watermark?
    pub fn is_default(&self) -> bool {
        *self == WatermarkSettings::default()
    }
}

/// One mark laid on a sheet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedMark {
    /// Centre, paper inches from the sheet's lower-left corner.
    pub cx: f64,
    pub cy: f64,
    /// Counter-clockwise degrees about the centre.
    pub angle_deg: f64,
    /// Size before turning, paper inches (a text mark: its width by the
    /// height of a capital A).
    pub w: f64,
    pub h: f64,
}

impl PlacedMark {
    /// Width and height of the box that holds the turned mark.
    pub fn bounds(&self) -> (f64, f64) {
        turned_bounds(self.w, self.h, self.angle_deg)
    }

    /// The lower-left origin of the unturned mark, the point text is drawn
    /// from (its baseline start) once it is turned about the centre.
    pub fn origin(&self) -> (f64, f64) {
        let (s, c) = self.angle_deg.to_radians().sin_cos();
        let (hx, hy) = (self.w * 0.5, self.h * 0.5);
        (self.cx - (hx * c - hy * s), self.cy - (hx * s + hy * c))
    }
}

/// Width and height of the axis-aligned box around a `w` x `h` rectangle
/// turned `angle_deg`.
pub fn turned_bounds(w: f64, h: f64, angle_deg: f64) -> (f64, f64) {
    let (s, c) = angle_deg.to_radians().sin_cos();
    (w * c.abs() + h * s.abs(), w * s.abs() + h * c.abs())
}

/// The area the marks fill, `[x0, y0, x1, y1]`: the sheet inside the margins
/// (the Drawing Sheet margins, or the watermark's own when Use Drawing Sheet
/// Margin is off). Never smaller than a tenth of the sheet.
pub fn mark_area(spec: &WatermarkSpec, sheet: (f64, f64), sheet_margins: [f64; 4]) -> [f64; 4] {
    let m = if spec.use_sheet_margin {
        sheet_margins
    } else {
        spec.margins_in
    };
    let [top, bottom, left, right] = m;
    let mut x0 = left.max(0.0);
    let mut x1 = sheet.0 - right.max(0.0);
    let mut y0 = bottom.max(0.0);
    let mut y1 = sheet.1 - top.max(0.0);
    if x1 - x0 < sheet.0 * 0.1 {
        x0 = 0.0;
        x1 = sheet.0;
    }
    if y1 - y0 < sheet.1 * 0.1 {
        y0 = 0.0;
        y1 = sheet.1;
    }
    [x0, y0, x1, y1]
}

/// Where the marks of `spec` go on a `sheet` (width, height) inches, given
/// the size `mark` (width, height) inches of one mark at its own size and the
/// Drawing Sheet's margins.
///
/// * Tile: Marks per Row by Marks per Column cells divide the area; a mark
///   sits at the centre of each cell, row 0 at the top.
/// * Border: the first and last rows run along the top and bottom of the area
///   and the middle rows along its left and right edges, marks centred in
///   their cells and kept inside the area.
/// * Fit to Sheet: one mark at the middle of the area, scaled so the turned
///   mark just fits.
pub fn place_marks(
    spec: &WatermarkSpec,
    sheet: (f64, f64),
    sheet_margins: [f64; 4],
    mark: (f64, f64),
) -> Vec<PlacedMark> {
    let (mw, mh) = (mark.0.max(1e-6), mark.1.max(1e-6));
    let [x0, y0, x1, y1] = mark_area(spec, sheet, sheet_margins);
    let (aw, ah) = (x1 - x0, y1 - y0);
    let angle = spec.angle_deg;
    let nx = spec.marks_per_row.clamp(1, MAX_MARKS) as usize;
    let ny = spec.marks_per_column.clamp(1, MAX_MARKS) as usize;
    let at = |cx: f64, cy: f64, w: f64, h: f64| PlacedMark {
        cx,
        cy,
        angle_deg: angle,
        w,
        h,
    };
    match spec.layout {
        WatermarkLayout::FitToSheet => {
            let (bw, bh) = turned_bounds(mw, mh, angle);
            let k = (aw / bw).min(ah / bh);
            vec![at((x0 + x1) * 0.5, (y0 + y1) * 0.5, mw * k, mh * k)]
        }
        WatermarkLayout::Tile => {
            let (cw, ch) = (aw / nx as f64, ah / ny as f64);
            let mut out = Vec::with_capacity(nx * ny);
            for j in 0..ny {
                for i in 0..nx {
                    out.push(at(
                        x0 + (i as f64 + 0.5) * cw,
                        y1 - (j as f64 + 0.5) * ch,
                        mw,
                        mh,
                    ));
                }
            }
            out
        }
        WatermarkLayout::Border => {
            let (bw, bh) = turned_bounds(mw, mh, angle);
            let (cw, ch) = (aw / nx as f64, ah / ny as f64);
            let top = y1 - bh * 0.5;
            let bottom = y0 + bh * 0.5;
            let left = x0 + bw * 0.5;
            let right = x1 - bw * 0.5;
            let mut out = Vec::new();
            for i in 0..nx {
                let x = x0 + (i as f64 + 0.5) * cw;
                out.push(at(x, top, mw, mh));
                if ny >= 2 {
                    out.push(at(x, bottom, mw, mh));
                }
            }
            for j in 1..ny.saturating_sub(1) {
                let y = y1 - (j as f64 + 0.5) * ch;
                out.push(at(left, y, mw, mh));
                out.push(at(right, y, mw, mh));
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(layout: WatermarkLayout, nx: u32, ny: u32, angle: f64) -> WatermarkSpec {
        WatermarkSpec {
            layout,
            marks_per_row: nx,
            marks_per_column: ny,
            angle_deg: angle,
            use_sheet_margin: false,
            margins_in: [0.0; 4],
            ..WatermarkSpec::default()
        }
    }

    #[test]
    fn tile_puts_a_mark_at_the_middle_of_each_cell_top_row_first() {
        let s = spec(WatermarkLayout::Tile, 3, 2, 30.0);
        let m = place_marks(&s, (30.0, 20.0), [0.0; 4], (2.0, 0.5));
        assert_eq!(m.len(), 6);
        // Cells are 10 x 10: the first row is the upper one.
        assert_eq!((m[0].cx, m[0].cy), (5.0, 15.0));
        assert_eq!((m[1].cx, m[1].cy), (15.0, 15.0));
        assert_eq!((m[2].cx, m[2].cy), (25.0, 15.0));
        assert_eq!((m[3].cx, m[3].cy), (5.0, 5.0));
        assert!(m
            .iter()
            .all(|k| k.angle_deg == 30.0 && k.w == 2.0 && k.h == 0.5));
    }

    #[test]
    fn tile_respects_the_margins_of_the_drawing_sheet_or_its_own() {
        let mut s = spec(WatermarkLayout::Tile, 2, 1, 0.0);
        s.use_sheet_margin = true;
        // Top, bottom, left, right of the drawing sheet.
        let m = place_marks(&s, (20.0, 10.0), [1.0, 1.0, 2.0, 2.0], (1.0, 1.0));
        // The area is x 2..18, y 1..9: two cells 8 wide, centres 6 and 14.
        assert_eq!((m[0].cx, m[0].cy), (6.0, 5.0));
        assert_eq!((m[1].cx, m[1].cy), (14.0, 5.0));
        s.use_sheet_margin = false;
        s.margins_in = [0.0, 0.0, 5.0, 0.0];
        let m = place_marks(&s, (20.0, 10.0), [1.0, 1.0, 2.0, 2.0], (1.0, 1.0));
        // Left margin 5: the area is x 5..20, two cells 7.5 wide.
        assert_eq!(m[0].cx, 8.75);
        assert_eq!(m[1].cx, 16.25);
    }

    #[test]
    fn border_runs_along_the_four_edges_with_the_corners_counted_once() {
        let s = spec(WatermarkLayout::Border, 4, 4, 0.0);
        let m = place_marks(&s, (40.0, 20.0), [0.0; 4], (4.0, 2.0));
        // 4 across the top, 4 along the bottom, 2 down each side.
        assert_eq!(m.len(), 4 + 4 + 2 + 2);
        let top: Vec<_> = m.iter().filter(|k| k.cy > 18.9).collect();
        assert_eq!(top.len(), 4);
        // Cells are 10 wide: centres 5, 15, 25, 35; a mark 2 tall hangs from the top.
        assert_eq!(top[0].cx, 5.0);
        assert!((top[0].cy - 19.0).abs() < 1e-9);
        let left: Vec<_> = m.iter().filter(|k| k.cx < 2.1).collect();
        assert_eq!(left.len(), 2, "the middle rows only, not the corners");
        assert!((left[0].cx - 2.0).abs() < 1e-9, "inside the left edge");
        // One column: only the top row.
        let one = place_marks(
            &spec(WatermarkLayout::Border, 3, 1, 0.0),
            (30.0, 20.0),
            [0.0; 4],
            (2.0, 1.0),
        );
        assert_eq!(one.len(), 3);
    }

    #[test]
    fn fit_to_sheet_scales_one_turned_mark_to_the_area() {
        let s = spec(WatermarkLayout::FitToSheet, 5, 5, 0.0);
        let m = place_marks(&s, (30.0, 10.0), [0.0; 4], (3.0, 1.0));
        assert_eq!(m.len(), 1);
        // Width limits: 30 / 3 = 10x; height 10 / 1 = 10x.
        assert!((m[0].w - 30.0).abs() < 1e-9 && (m[0].h - 10.0).abs() < 1e-9);
        assert_eq!((m[0].cx, m[0].cy), (15.0, 5.0));
        // Turned 90 degrees the mark is 1 wide by 3 tall: the height limits.
        let s = spec(WatermarkLayout::FitToSheet, 1, 1, 90.0);
        let m = place_marks(&s, (30.0, 10.0), [0.0; 4], (3.0, 1.0));
        let (bw, bh) = m[0].bounds();
        assert!((bh - 10.0).abs() < 1e-9 && bw < 30.0);
        assert!((m[0].w - 10.0).abs() < 1e-9, "{}", m[0].w);
    }

    #[test]
    fn a_turned_mark_has_its_origin_where_the_text_starts() {
        // Unturned: the origin is the lower-left corner.
        let m = PlacedMark {
            cx: 10.0,
            cy: 5.0,
            angle_deg: 0.0,
            w: 4.0,
            h: 2.0,
        };
        assert_eq!(m.origin(), (8.0, 4.0));
        // Turned a quarter: the text reads upward, so its start is below the centre.
        let q = PlacedMark {
            angle_deg: 90.0,
            ..m
        };
        let (x, y) = q.origin();
        assert!((x - 11.0).abs() < 1e-9 && (y - 3.0).abs() < 1e-9, "{x} {y}");
        let (bw, bh) = q.bounds();
        assert!((bw - 2.0).abs() < 1e-9 && (bh - 4.0).abs() < 1e-9);
    }

    #[test]
    fn toggle_and_serde_round_trip() {
        let mut w = WatermarkSettings::default();
        assert!(w.is_default());
        assert!(w.toggle(&plan_key("Working Plan View")));
        assert!(w.is_on("plan:Working Plan View"));
        assert!(!w.is_on(LAYOUT_KEY));
        w.spec.text = "NOT FOR CONSTRUCTION".into();
        let json = serde_json::to_string(&w).unwrap();
        let back: WatermarkSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, w);
        assert!(!w.toggle(&plan_key("Working Plan View")));
        // Missing keys fall back to the defaults.
        let old: WatermarkSettings = serde_json::from_str(r#"{"spec":{"text":"X"}}"#).unwrap();
        assert_eq!(old.spec.text, "X");
        assert_eq!(old.spec.layout, WatermarkLayout::Tile);
    }

    #[test]
    fn the_answers_are_checked() {
        let mut s = WatermarkSpec::default();
        assert!(s.problem().is_none());
        s.text = "  ".into();
        assert!(s.problem().is_some() && !s.has_content());
        s.text = "DRAFT".into();
        s.transparency = 120.0;
        assert!(s.problem().is_some());
        s.transparency = 50.0;
        assert!((s.alpha() - 0.5).abs() < 1e-12);
        s.kind = WatermarkKind::Image;
        assert!(s.problem().is_some());
        s.image_path = "logo.png".into();
        assert!(s.problem().is_none());
        // A capital A of 0.718 em: a 1 inch print size is a 100.28 pt font.
        s.print_size_in = 1.0;
        assert!((s.font_size_pt() - 72.0 / 0.718).abs() < 1e-9);
    }

    #[test]
    fn a_margin_that_eats_the_sheet_is_ignored() {
        let mut s = spec(WatermarkLayout::FitToSheet, 1, 1, 0.0);
        s.margins_in = [0.0, 0.0, 19.0, 0.0];
        assert_eq!(
            mark_area(&s, (20.0, 10.0), [0.0; 4]),
            [0.0, 0.0, 20.0, 10.0]
        );
    }
}
