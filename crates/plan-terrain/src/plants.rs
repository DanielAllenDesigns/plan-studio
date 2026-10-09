//! Plants as images (Plant Image Specification, manual pp. 1362-1366), Grow
//! Plants, Garden Bed distributed plants and the grass-blade options of a
//! Grass Region (pp. 1335-1336).

use plan_core::geometry::point_in_polygon;
use plan_core::Point;
use serde::{Deserialize, Serialize};

use crate::geom::{bounds, dist_to_boundary};
use crate::landscape::{Landscape, LandscapeKind};

/// The season the plan is shown in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Season {
    Spring,
    #[default]
    Summer,
    Autumn,
    Winter,
}

impl Season {
    pub const ALL: [Season; 4] = [
        Season::Spring,
        Season::Summer,
        Season::Autumn,
        Season::Winter,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Season::Spring => "Spring",
            Season::Summer => "Summer",
            Season::Autumn => "Autumn",
            Season::Winter => "Winter",
        }
    }

    fn index(self) -> usize {
        match self {
            Season::Spring => 0,
            Season::Summer => 1,
            Season::Autumn => 2,
            Season::Winter => 3,
        }
    }
}

/// How a plant image looks in one season.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SeasonLook {
    /// Tint of the foliage, RGB.
    pub tint: [u8; 3],
    /// Share of the full foliage that is on the plant: 1 in leaf, 0 bare.
    pub foliage: f64,
}

impl Default for SeasonLook {
    fn default() -> Self {
        SeasonLook {
            tint: [0x4C, 0x8A, 0x3C],
            foliage: 1.0,
        }
    }
}

/// The four looks of a deciduous plant image (a conifer keeps its foliage).
pub fn default_seasons(evergreen: bool) -> [SeasonLook; 4] {
    if evergreen {
        let green = SeasonLook {
            tint: [0x2F, 0x6B, 0x3A],
            foliage: 1.0,
        };
        return [green; 4];
    }
    [
        SeasonLook {
            tint: [0x7C, 0xB3, 0x5A],
            foliage: 0.7,
        },
        SeasonLook {
            tint: [0x4C, 0x8A, 0x3C],
            foliage: 1.0,
        },
        SeasonLook {
            tint: [0xC8, 0x7A, 0x2A],
            foliage: 0.8,
        },
        SeasonLook {
            tint: [0x8A, 0x78, 0x62],
            foliage: 0.0,
        },
    ]
}

/// Plant Image Specification: a plant drawn from an image, standing as a
/// billboard in 3D. The run keeps one image for all its plants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlantImage {
    /// Image file (a library image path or a file on disk).
    pub file: String,
    /// 2D plant symbol drawn in plan (a library symbol or CAD block name).
    pub symbol_2d: String,
    /// Size of the image, inches.
    pub width: f64,
    pub height: f64,
    /// Keep the aspect ratio when one size changes.
    pub retain_aspect: bool,
    /// The image's own height-to-width ratio (Reset Original Aspect Ratio).
    pub original_aspect: f64,
    /// Elevation reference: the elevation is measured to the top of the image
    /// (true) or to its bottom (false).
    pub elevation_to_top: bool,
    /// Elevation of that reference above the ground, inches.
    pub elevation: f64,
    /// Center point of the image within the plant's footprint, inches.
    pub center: Point,
    /// Mirror the image.
    pub reverse: bool,
    /// The image turns to face the camera.
    pub faces_camera: bool,
    pub copyright: String,
    /// Colour that is transparent in the image, RGB (`None` = none).
    pub transparent_color: Option<[u8; 3]>,
    /// The look in spring, summer, autumn and winter.
    pub seasons: [SeasonLook; 4],
}

impl Default for PlantImage {
    fn default() -> Self {
        PlantImage {
            file: String::new(),
            symbol_2d: String::new(),
            width: 36.0,
            height: 36.0,
            retain_aspect: true,
            original_aspect: 1.0,
            elevation_to_top: false,
            elevation: 0.0,
            center: Point::new(0.0, 0.0),
            reverse: false,
            faces_camera: true,
            copyright: String::new(),
            transparent_color: None,
            seasons: default_seasons(false),
        }
    }
}

impl PlantImage {
    /// An image of `width` by `height` inches whose original aspect ratio is
    /// that of the two.
    pub fn sized(file: &str, width: f64, height: f64, evergreen: bool) -> Self {
        PlantImage {
            file: file.to_string(),
            width,
            height,
            original_aspect: if width > 0.0 { height / width } else { 1.0 },
            seasons: default_seasons(evergreen),
            ..PlantImage::default()
        }
    }

    /// Sets the width, following the height when the aspect ratio is retained.
    pub fn set_width(&mut self, width: f64) {
        let aspect = if self.width > 0.0 {
            self.height / self.width
        } else {
            self.original_aspect
        };
        self.width = width;
        if self.retain_aspect {
            self.height = width * aspect;
        }
    }

    /// Sets the height, following the width when the aspect ratio is retained.
    pub fn set_height(&mut self, height: f64) {
        let aspect = if self.height > 0.0 {
            self.width / self.height
        } else {
            1.0 / self.original_aspect.max(1e-6)
        };
        self.height = height;
        if self.retain_aspect {
            self.width = height * aspect;
        }
    }

    /// Back to the image's original aspect ratio, keeping the width.
    pub fn reset_aspect(&mut self) {
        self.height = self.width * self.original_aspect;
    }

    /// The look in `season`.
    pub fn look(&self, season: Season) -> SeasonLook {
        self.seasons[season.index()]
    }

    /// Height of the bottom of the image above the ground, inches.
    pub fn bottom_above_ground(&self) -> f64 {
        if self.elevation_to_top {
            self.elevation - self.height
        } else {
            self.elevation
        }
    }
}

/// Garden Bed distributed plants: copies of a plant spread over the bed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Distribution {
    /// Catalog id of the plant.
    pub plant: String,
    /// Center to center, inches.
    pub spacing: f64,
    /// Canopy width and height of each plant, inches.
    pub size: f64,
    pub height: f64,
    /// Stay this far inside the bed's edge, inches.
    pub margin: f64,
    /// Stagger alternate rows by half a spacing.
    pub stagger: bool,
}

impl Default for Distribution {
    fn default() -> Self {
        Distribution {
            plant: String::new(),
            spacing: 36.0,
            size: 30.0,
            height: 30.0,
            margin: 12.0,
            stagger: true,
        }
    }
}

/// Positions of the plants spread over a bed outline: a lattice `spacing`
/// apart (alternate rows offset when `stagger`), kept `margin` inside the edge.
pub fn distribute_in(outline: &[Point], d: &Distribution) -> Vec<Point> {
    if outline.len() < 3 || d.spacing < 1.0 {
        return Vec::new();
    }
    let Some((lo, hi)) = bounds(outline) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut y = lo.y + d.spacing / 2.0;
    let mut row = 0;
    while y < hi.y {
        let shift = if d.stagger && row % 2 == 1 {
            d.spacing / 2.0
        } else {
            0.0
        };
        let mut x = lo.x + d.spacing / 2.0 + shift;
        while x < hi.x {
            let p = Point::new(x, y);
            if point_in_polygon(p, outline) && dist_to_boundary(p, outline) >= d.margin {
                out.push(p);
            }
            x += d.spacing;
        }
        y += if d.stagger {
            d.spacing * 0.866
        } else {
            d.spacing
        };
        row += 1;
    }
    out
}

impl Landscape {
    /// Plants spread over a garden bed by its Distributed Plant panel.
    pub fn distributed_positions(&self) -> Vec<Point> {
        match (&self.distribution, self.kind) {
            (Some(d), LandscapeKind::GardenBed) if !d.plant.is_empty() => {
                distribute_in(&self.points, d)
            }
            _ => Vec::new(),
        }
    }
}

/// Months of growth to maturity of a plant `height` tall at maturity.
pub fn default_age_at_maturity(mature_height: f64) -> f64 {
    if mature_height >= 96.0 {
        240.0
    } else if mature_height >= 36.0 {
        84.0
    } else {
        36.0
    }
}

/// Share of its mature size a plant has reached at `age_months`, starting from
/// `start_fraction` of it at age 0 and growing evenly to 1 at `maturity_months`.
pub fn growth_fraction(age_months: f64, maturity_months: f64, start_fraction: f64) -> f64 {
    if maturity_months <= 0.0 {
        return 1.0;
    }
    let start = start_fraction.clamp(0.05, 1.0);
    (start + (1.0 - start) * (age_months / maturity_months)).clamp(start, 1.0)
}

/// Grow Plants (Terrain > Plant > Grow All Plants): sets every plant that has
/// a mature size to what it is `years` after planting. Plants without growth
/// data keep their size. Returns how many plants changed.
pub fn grow_plants(landscape: &mut [Landscape], years: f64) -> usize {
    let months = (years * 12.0).clamp(0.0, 240.0);
    let mut changed = 0;
    for l in landscape
        .iter_mut()
        .filter(|l| l.kind == LandscapeKind::Plants && l.mature_height > 0.0)
    {
        let f = growth_fraction(months, l.maturity_months, l.start_fraction);
        let (h, w) = (l.mature_height * f, l.mature_width.max(1.0) * f);
        if (l.height - h).abs() > 1e-9 || (l.size - w).abs() > 1e-9 {
            l.height = h;
            l.size = w;
            if let Some(img) = l.image.as_mut() {
                img.height = h;
                img.width = w;
            }
            changed += 1;
        }
    }
    changed
}

// ----- grass regions -----

/// Grass Region Specification > Blades.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GrassBlades {
    /// Blades per square foot.
    pub density: f64,
    /// Blade height range, inches.
    pub min_height: f64,
    pub max_height: f64,
    /// Blade width range, inches.
    pub min_width: f64,
    pub max_width: f64,
    /// Blade curve range, 0 straight to 1 bent over.
    pub min_curve: f64,
    pub max_curve: f64,
}

impl Default for GrassBlades {
    fn default() -> Self {
        GrassBlades {
            density: 20.0,
            min_height: 1.5,
            max_height: 3.0,
            min_width: 0.1,
            max_width: 0.25,
            min_curve: 0.0,
            max_curve: 0.4,
        }
    }
}

impl GrassBlades {
    /// The range fields put in order and kept in bounds.
    pub fn sanitized(&self) -> GrassBlades {
        let ord = |a: f64, b: f64| (a.min(b), a.max(b));
        let (min_height, max_height) = ord(self.min_height.max(0.0), self.max_height.max(0.0));
        let (min_width, max_width) = ord(self.min_width.max(0.0), self.max_width.max(0.0));
        let (min_curve, max_curve) = ord(
            self.min_curve.clamp(0.0, 1.0),
            self.max_curve.clamp(0.0, 1.0),
        );
        GrassBlades {
            density: self.density.clamp(0.0, 400.0),
            min_height,
            max_height,
            min_width,
            max_width,
            min_curve,
            max_curve,
        }
    }

    /// Mean blade height, inches.
    pub fn mean_height(&self) -> f64 {
        (self.min_height + self.max_height) / 2.0
    }
}

/// Mowing of a grass region: stripes of cut height.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Mow {
    pub enabled: bool,
    /// Height of the cut grass, inches.
    pub cut_height: f64,
    /// How different the stripes are, 0 to 1.
    pub line_intensity: f64,
    /// Width of one stripe, inches.
    pub line_width: f64,
    /// Direction of the stripes, degrees.
    pub angle: f64,
}

impl Default for Mow {
    fn default() -> Self {
        Mow {
            enabled: false,
            cut_height: 1.5,
            line_intensity: 0.3,
            line_width: 24.0,
            angle: 0.0,
        }
    }
}

/// Grass Region Specification > Appearance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GrassLook {
    /// Blade colours, RGB (the base of the blade first).
    pub colors: Vec<[u8; 3]>,
    pub noise_frequency: f64,
    pub roughness: f64,
    pub mow: Mow,
}

impl Default for GrassLook {
    fn default() -> Self {
        GrassLook {
            colors: vec![[0x3D, 0x7A, 0x32], [0x6F, 0xA8, 0x3E]],
            noise_frequency: 0.5,
            roughness: 0.5,
            mow: Mow::default(),
        }
    }
}

impl GrassLook {
    /// The average blade colour: what the 3D surface of the region is tinted with.
    pub fn average_color(&self) -> [u8; 3] {
        if self.colors.is_empty() {
            return [0x4C, 0x8A, 0x3C];
        }
        let n = self.colors.len() as u32;
        let sum = self.colors.iter().fold([0u32; 3], |a, c| {
            [
                a[0] + u32::from(c[0]),
                a[1] + u32::from(c[1]),
                a[2] + u32::from(c[2]),
            ]
        });
        [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8]
    }
}
