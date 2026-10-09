//! The active layout's sheet: its paper size and drawing scale, and where it
//! sits on the plan (View > Drawing Sheet, View > Print Preview).

use plan_core::geometry::Point;
use plan_core::Floor;
use plan_docs::{Scale, SheetSize};

/// The sheet size and scale the plan is drawn for. The project stores no
/// layouts yet, so the app keeps this one "active layout" per session; the
/// Project Browser's Layout section edits it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SheetSetup {
    pub size: SheetSize,
    pub scale: Scale,
    /// Drawing Sheet Setup (round 16): the sheet in hundredths of an inch,
    /// `(width, height)`, when it is not exactly `size` landscape: a custom
    /// size or a sheet turned upright. Kept with the `size` it was made for;
    /// choosing another size elsewhere drops it.
    pub paper_cin: Option<(SheetSize, (u32, u32))>,
}

impl Default for SheetSetup {
    /// Architectural D at 1/4" = 1'-0", the construction set default.
    fn default() -> Self {
        Self {
            size: SheetSize::ArchD,
            scale: Scale::QuarterInch,
            paper_cin: None,
        }
    }
}

impl SheetSetup {
    /// The sheet on paper, `(width, height)` inches, as the Drawing Sheet
    /// Setup left it.
    pub fn paper_inches(&self) -> (f64, f64) {
        match self.paper_cin {
            Some((size, (w, h))) if size == self.size => {
                (f64::from(w) / 100.0, f64::from(h) / 100.0)
            }
            _ => self.size.inches(),
        }
    }

    /// The sheet on the ground, `(width, height)` in plan inches: paper
    /// inches divided by the scale (1/4" = 1' puts 48 plan inches on each
    /// paper inch).
    pub fn world_size(&self) -> (f64, f64) {
        let (w, h) = self.paper_inches();
        let plan_in_per_paper_in = 12.0 / self.scale.inches_per_foot();
        (w * plan_in_per_paper_in, h * plan_in_per_paper_in)
    }

    /// The sheet centered on `center`: `(min, max)` corners.
    pub fn rect_around(&self, center: Point) -> (Point, Point) {
        let (w, h) = self.world_size();
        (
            Point::new(center.x - w * 0.5, center.y - h * 0.5),
            Point::new(center.x + w * 0.5, center.y + h * 0.5),
        )
    }

    /// `ARCH D (24 x 36)  1/4" = 1'-0"`.
    pub fn caption(&self) -> String {
        match self.paper_cin {
            Some((size, (w, h))) if size == self.size => {
                let side = |v: u32| {
                    if v % 100 == 0 {
                        format!("{}", v / 100)
                    } else {
                        format!("{}", f64::from(v) / 100.0)
                    }
                };
                format!("{} x {} in  {}", side(w), side(h), self.scale.label())
            }
            _ => format!("{}  {}", self.size.label(), self.scale.label()),
        }
    }
}

thread_local! {
    /// The colour mode View > Print Preview shows: the Print dialog's Color,
    /// Grayscale or Black and white, as last previewed.
    static PREVIEW_COLOR: std::cell::Cell<plan_layout::PrintColor> =
        const { std::cell::Cell::new(plan_layout::PrintColor::Color) };
}

/// Sets the colour mode Print Preview shows.
pub fn set_preview_color(c: plan_layout::PrintColor) {
    PREVIEW_COLOR.with(|p| p.set(c));
}

/// The colour mode Print Preview shows.
pub fn preview_color() -> plan_layout::PrintColor {
    PREVIEW_COLOR.with(std::cell::Cell::get)
}

/// What the preview's caption says about the colour mode; empty for colour.
pub fn preview_color_label(c: plan_layout::PrintColor) -> &'static str {
    match c {
        plan_layout::PrintColor::Color => "",
        plan_layout::PrintColor::Grayscale => "Grayscale",
        plan_layout::PrintColor::BlackWhite => "Black and white",
    }
}

/// The center of the floor's walls (the middle of their bounding box), or
/// the origin when there are none.
pub fn plan_center(floor: &Floor) -> Point {
    // Center Sheet / dragging the sheet put it elsewhere for this floor.
    if let Some(c) = floor.sheet_center {
        return c;
    }
    let mut pts = floor.walls.iter().flat_map(|w| w.footprint());
    let Some(first) = pts.next() else {
        return Point::ZERO;
    };
    let (mut lo, mut hi) = (first, first);
    for p in pts {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::WallKind;

    #[test]
    fn arch_d_at_quarter_inch_is_144_by_96_feet() {
        let (w, h) = SheetSetup::default().world_size();
        assert_eq!((w, h), (36.0 * 48.0, 24.0 * 48.0));
        let s = SheetSetup {
            size: SheetSize::Letter,
            scale: Scale::EighthInch,
            ..SheetSetup::default()
        };
        assert_eq!(s.world_size(), (11.0 * 96.0, 8.5 * 96.0));
    }

    #[test]
    fn the_sheet_is_centered_on_the_walls() {
        let mut p = plan_core::Project::new("t");
        assert_eq!(plan_center(&p.floors[0]), Point::ZERO);
        p.add_wall(
            0,
            Point::new(100.0, 100.0),
            Point::new(300.0, 100.0),
            0.0001,
            100.0,
            WallKind::Exterior,
        );
        let c = plan_center(&p.floors[0]);
        assert!((c.x - 200.0).abs() < 1e-3 && (c.y - 100.0).abs() < 1e-3);
        let (lo, hi) = SheetSetup::default().rect_around(c);
        assert!(((lo.x + hi.x) * 0.5 - c.x).abs() < 1e-9);
        assert!((hi.x - lo.x - 1728.0).abs() < 1e-9);
    }
}
