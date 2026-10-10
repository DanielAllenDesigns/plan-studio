//! Terrain Specification types of round 15 (manual pp. 1322-1327): Absolute
//! Elevation, the Terrain Elevation Reference Point, the skirt, smoothing
//! levels, triangle count and the contour presentation options, plus the
//! per-object extras (label, schedule category, object information) every
//! terrain object carries.

use std::collections::BTreeMap;

use plan_core::units::fmt_ft_in_frac;
use plan_core::Point;
use serde::{Deserialize, Serialize};

use crate::landscape::ObjectStyle;
use crate::model::Terrain;

/// Distance from the terrain to Floor 1's subfloor that "Automatic" uses, inches
/// (Chief takes 6" for a slab or a 6" stem wall, 8" for a deeper foundation).
pub const AUTOMATIC_SUBFLOOR_DISTANCE: f64 = 6.0;
/// Default vertical distance from the Floor 1 subfloor to the surface at the
/// reference point or at contour 0 (negative: the surface is below the floor).
pub const DEFAULT_SURFACE_OFFSET: f64 = -6.0;
/// Default thickness of the terrain skirt, inches.
pub const DEFAULT_SKIRT_THICKNESS: f64 = 24.0;
/// Custom triangle count a fresh Terrain Specification offers.
pub const DEFAULT_CUSTOM_TRIANGLES: u32 = 3000;
/// Default number of Chaikin passes of the 2D contour smoothing.
pub const DEFAULT_CONTOUR_SMOOTH_PASSES: u32 = 2;

/// Terrain Specification > General > Absolute Elevation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AbsoluteElevation {
    /// Chief sets the distance between Floor 1 and the terrain.
    #[default]
    Automatic,
    /// Retain the surface elevation at the Terrain Elevation Reference Point.
    ReferencePoint,
    /// Retain the surface elevation at contour 0.
    ContourZero,
}

impl AbsoluteElevation {
    pub const ALL: [AbsoluteElevation; 3] = [
        AbsoluteElevation::Automatic,
        AbsoluteElevation::ReferencePoint,
        AbsoluteElevation::ContourZero,
    ];

    pub fn name(self) -> &'static str {
        match self {
            AbsoluteElevation::Automatic => "Automatic",
            AbsoluteElevation::ReferencePoint => "Retain surface elevation at Reference Point",
            AbsoluteElevation::ContourZero => "Retain surface elevation at Contour 0",
        }
    }
}

/// How the skirt's bottom is found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SkirtMode {
    /// A flat base below the lowest point of the surface.
    #[default]
    FlatBase,
    /// A constant distance below the surface all round.
    FollowTerrain,
}

impl SkirtMode {
    pub fn name(self) -> &'static str {
        match self {
            SkirtMode::FlatBase => "Flat base",
            SkirtMode::FollowTerrain => "Follow terrain",
        }
    }
}

/// The 3D skirt around the edge of the terrain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Skirt {
    pub enabled: bool,
    /// Height (thickness) of the skirt, inches.
    pub thickness: f64,
    pub mode: SkirtMode,
    /// Material by name; empty takes the bare-ground material.
    pub material: String,
}

impl Default for Skirt {
    fn default() -> Self {
        Skirt {
            enabled: false,
            thickness: DEFAULT_SKIRT_THICKNESS,
            mode: SkirtMode::FlatBase,
            material: String::new(),
        }
    }
}

/// Terrain Surface Smoothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SmoothingLevel {
    /// The number of passes typed in `Terrain::smoothing`.
    #[default]
    Passes,
    /// Flat triangles between the data points (no resampling), for dense data.
    Linear,
    Low,
    Medium,
    High,
}

impl SmoothingLevel {
    pub const ALL: [SmoothingLevel; 5] = [
        SmoothingLevel::Linear,
        SmoothingLevel::Low,
        SmoothingLevel::Medium,
        SmoothingLevel::High,
        SmoothingLevel::Passes,
    ];

    pub fn name(self) -> &'static str {
        match self {
            SmoothingLevel::Passes => "Custom passes",
            SmoothingLevel::Linear => "Linear",
            SmoothingLevel::Low => "Low",
            SmoothingLevel::Medium => "Medium",
            SmoothingLevel::High => "High",
        }
    }
}

/// Surface triangle detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TriangleDetail {
    /// The grid spacing and subdivision of the specification.
    #[default]
    Grid,
    Low,
    Medium,
    High,
    /// `Terrain::custom_triangles`.
    Custom,
}

impl TriangleDetail {
    pub const ALL: [TriangleDetail; 5] = [
        TriangleDetail::Grid,
        TriangleDetail::Low,
        TriangleDetail::Medium,
        TriangleDetail::High,
        TriangleDetail::Custom,
    ];

    pub fn name(self) -> &'static str {
        match self {
            TriangleDetail::Grid => "From the grid spacing",
            TriangleDetail::Low => "Low (1000)",
            TriangleDetail::Medium => "Medium (2000)",
            TriangleDetail::High => "High (4000)",
            TriangleDetail::Custom => "Custom",
        }
    }
}

/// Units of the contour elevation labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LabelUnits {
    /// Feet and inches like the rest of the plan.
    #[default]
    FeetInches,
    Inches,
    DecimalFeet,
}

impl LabelUnits {
    pub const ALL: [LabelUnits; 3] = [
        LabelUnits::FeetInches,
        LabelUnits::Inches,
        LabelUnits::DecimalFeet,
    ];

    pub fn name(self) -> &'static str {
        match self {
            LabelUnits::FeetInches => "Feet and inches",
            LabelUnits::Inches => "Inches",
            LabelUnits::DecimalFeet => "Decimal feet",
        }
    }

    /// The label text of an elevation (inches).
    pub fn format(self, z: f64) -> String {
        match self {
            LabelUnits::FeetInches => fmt_ft_in_frac(z, 2),
            LabelUnits::Inches => format!("{}\"", trim_zeros(z, 1)),
            LabelUnits::DecimalFeet => format!("{}'", trim_zeros(z / 12.0, 2)),
        }
    }
}

fn trim_zeros(v: f64, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

/// What a finished Build Terrain made (shown in the specification; cleared by
/// Clear Terrain).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildStats {
    pub triangles: u32,
    pub contour_levels: u32,
}

/// A label on a terrain object (the "Terrain Labels" layer).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ObjectLabel {
    pub shown: bool,
    /// Custom text; empty takes the automatic label.
    pub text: String,
    /// Offset of the label from its object's anchor point, inches.
    pub offset: Point,
}

/// The Object Information panel.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ObjectInfo {
    pub manufacturer: String,
    pub supplier: String,
    pub code: String,
    pub comment: String,
    pub url: String,
}

impl ObjectInfo {
    pub fn is_empty(&self) -> bool {
        *self == ObjectInfo::default()
    }
}

/// Everything beyond its geometry a terrain object keeps: the Label,
/// Object Information and Schedule panels, and the options of the elevation
/// data specifications.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ObjectExtras {
    pub label: ObjectLabel,
    /// Schedule category chosen by hand; empty takes the object's own.
    pub schedule_category: String,
    pub info: ObjectInfo,
    /// Names on the Components tab (listed in the Materials List).
    pub components: Vec<String>,
    /// Elevation Region: only the outline is held, not the interior (the
    /// "Interior is Flat" box unchecked).
    pub interior_open: bool,
    /// Elevation Region with an open interior: flatten the surface as it
    /// approaches the edge ("Interpolate Tangent to Edge").
    pub tangent_to_edge: bool,
    /// Elevation Point: note text beside the point (`%elevation%` is replaced
    /// by the point's elevation).
    pub note: String,
    /// Elevation Point: radius of the marker, inches (0 takes the default).
    pub marker_radius: f64,
    /// Line style, layer and fill of the objects that have no style of their own.
    pub style: ObjectStyle,
}

/// Default radius of an Elevation Point marker, inches.
pub const DEFAULT_MARKER_RADIUS: f64 = 3.0;

/// A terrain object, addressed by kind and index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObjectKey {
    Perimeter,
    Point(usize),
    Line(usize),
    Region(usize),
    Modifier(usize),
    Feature(usize),
    Break(usize),
    Wall(usize),
    Landscape(usize),
    Road(usize),
}

impl ObjectKey {
    /// Key of the side table (`Terrain::side_extras`), for the kinds that
    /// keep their extras there.
    fn side_key(self) -> Option<String> {
        match self {
            ObjectKey::Point(i) => Some(format!("point:{i}")),
            ObjectKey::Region(i) => Some(format!("region:{i}")),
            ObjectKey::Modifier(i) => Some(format!("modifier:{i}")),
            _ => None,
        }
    }
}

/// Which side-table kinds exist: used to re-index after a removal.
const SIDE_KINDS: [&str; 3] = ["point", "region", "modifier"];

impl Terrain {
    /// The extras of `key` (default when it has none or does not exist).
    pub fn extras(&self, key: ObjectKey) -> ObjectExtras {
        match key {
            ObjectKey::Perimeter => Some(self.perimeter_extras.clone()),
            ObjectKey::Line(i) => self.elevation_lines.get(i).map(|o| o.extras.clone()),
            ObjectKey::Feature(i) => self.features.get(i).map(|o| o.extras.clone()),
            ObjectKey::Break(i) => self.breaks.get(i).map(|o| o.extras.clone()),
            ObjectKey::Wall(i) => self.walls.get(i).map(|o| o.extras.clone()),
            ObjectKey::Landscape(i) => self.landscape.get(i).map(|o| o.extras.clone()),
            ObjectKey::Road(i) => self.roads.get(i).map(|o| o.extras.clone()),
            other => other
                .side_key()
                .and_then(|k| self.side_extras.get(&k).cloned()),
        }
        .unwrap_or_default()
    }

    /// Stores `extras` on `key` (nothing happens when the object does not exist).
    pub fn set_extras(&mut self, key: ObjectKey, extras: ObjectExtras) {
        match key {
            ObjectKey::Perimeter => self.perimeter_extras = extras,
            ObjectKey::Line(i) => {
                if let Some(o) = self.elevation_lines.get_mut(i) {
                    o.extras = extras;
                }
            }
            ObjectKey::Feature(i) => {
                if let Some(o) = self.features.get_mut(i) {
                    o.extras = extras;
                }
            }
            ObjectKey::Break(i) => {
                if let Some(o) = self.breaks.get_mut(i) {
                    o.extras = extras;
                }
            }
            ObjectKey::Wall(i) => {
                if let Some(o) = self.walls.get_mut(i) {
                    o.extras = extras;
                }
            }
            ObjectKey::Landscape(i) => {
                if let Some(o) = self.landscape.get_mut(i) {
                    o.extras = extras;
                }
            }
            ObjectKey::Road(i) => {
                if let Some(o) = self.roads.get_mut(i) {
                    o.extras = extras;
                }
            }
            other => {
                let exists = match other {
                    ObjectKey::Point(i) => i < self.elevation_points.len(),
                    ObjectKey::Region(i) => i < self.elevation_regions.len(),
                    ObjectKey::Modifier(i) => i < self.modifiers.len(),
                    _ => false,
                };
                if let (true, Some(k)) = (exists, other.side_key()) {
                    if extras == ObjectExtras::default() {
                        self.side_extras.remove(&k);
                    } else {
                        self.side_extras.insert(k, extras);
                    }
                }
            }
        }
    }

    /// Re-keys the side table after the object `key` was removed from its list
    /// (the objects after it moved down one place). Call it with the key of the
    /// removed object; it does nothing for the kinds that keep their extras on
    /// the object itself.
    pub fn forget_extras_of_removed(&mut self, key: ObjectKey) {
        let (kind, index) = match key {
            ObjectKey::Point(i) => ("point", i),
            ObjectKey::Region(i) => ("region", i),
            ObjectKey::Modifier(i) => ("modifier", i),
            _ => return,
        };
        let old = std::mem::take(&mut self.side_extras);
        for (k, v) in old {
            let Some((kk, num)) = k.split_once(':') else {
                continue;
            };
            let Ok(n) = num.parse::<usize>() else {
                continue;
            };
            if kk != kind || !SIDE_KINDS.contains(&kk) || n < index {
                self.side_extras.insert(k, v);
            } else if n > index {
                self.side_extras.insert(format!("{kind}:{}", n - 1), v);
            }
        }
    }

    /// Every object that exists, in a fixed order.
    pub fn object_keys(&self) -> Vec<ObjectKey> {
        let mut out = vec![ObjectKey::Perimeter];
        out.extend((0..self.elevation_points.len()).map(ObjectKey::Point));
        out.extend((0..self.elevation_lines.len()).map(ObjectKey::Line));
        out.extend((0..self.elevation_regions.len()).map(ObjectKey::Region));
        out.extend((0..self.modifiers.len()).map(ObjectKey::Modifier));
        out.extend((0..self.features.len()).map(ObjectKey::Feature));
        out.extend((0..self.breaks.len()).map(ObjectKey::Break));
        out.extend((0..self.walls.len()).map(ObjectKey::Wall));
        out.extend((0..self.landscape.len()).map(ObjectKey::Landscape));
        out.extend((0..self.roads.len()).map(ObjectKey::Road));
        out
    }

    // ----- absolute elevation -----

    /// The distance from the terrain to Floor 1's subfloor the building pad
    /// uses: Chief's own in Automatic, the typed one otherwise.
    pub fn effective_subfloor_distance(&self) -> f64 {
        match self.absolute_elevation {
            AbsoluteElevation::Automatic => self.subfloor_height_above_terrain,
            _ => -self.surface_offset,
        }
    }

    /// The point whose surface elevation is retained: the placed Reference
    /// Point, or the middle of the perimeter's box when none was placed.
    pub fn effective_reference_point(&self) -> Option<Point> {
        self.reference_point.or_else(|| {
            let (lo, hi) = crate::geom::bounds(&self.perimeter)?;
            Some(Point::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0))
        })
    }

    // ----- contours -----

    /// The cutting options of the contour lines for a contour `interval`.
    pub fn contour_options(&self, interval: f64) -> crate::contour::ContourOptions {
        crate::contour::ContourOptions {
            interval,
            major_every: self.contour_major_every,
            offset: self.contour_offset,
            smooth_passes: if self.contour_smoothing {
                self.contour_smooth_passes
            } else {
                0
            },
        }
    }

    // ----- surface detail -----

    /// Laplacian passes the build applies.
    pub fn effective_smoothing(&self) -> u32 {
        match self.smoothing_level {
            SmoothingLevel::Passes => self.smoothing,
            SmoothingLevel::Linear => 0,
            SmoothingLevel::Low => 1,
            SmoothingLevel::Medium => 3,
            SmoothingLevel::High => 6,
        }
    }

    /// The triangle count the detail asks for, if it names one.
    pub fn triangle_target(&self) -> Option<u32> {
        match self.triangle_detail {
            TriangleDetail::Grid => None,
            TriangleDetail::Low => Some(1000),
            TriangleDetail::Medium => Some(2000),
            TriangleDetail::High => Some(4000),
            TriangleDetail::Custom => Some(self.custom_triangles.max(50)),
        }
    }

    /// Spacing of the sample grid the build uses, inches: from the triangle
    /// target (about two triangles per grid cell) or a maximum triangle size,
    /// else the grid spacing over the subdivision.
    pub fn effective_grid_spacing(&self) -> f64 {
        let base = self.grid_spacing / f64::from(self.subdivision.max(1));
        let area = plan_core::geometry::polygon_area(&self.perimeter).abs();
        if let Some(n) = self.triangle_target() {
            if area > 1.0 {
                return (2.0 * area / f64::from(n)).sqrt().max(6.0);
            }
        }
        if self.max_triangle_size > 0.0 {
            return self.max_triangle_size.max(6.0);
        }
        base
    }

    /// Rough number of triangles the settings give for this perimeter.
    pub fn estimated_triangles(&self) -> usize {
        let area = plan_core::geometry::polygon_area(&self.perimeter).abs();
        let s = self.effective_grid_spacing().max(1.0);
        (2.0 * area / (s * s)).round() as usize
    }
}

/// Clear Terrain (manual p. 1309): removes only what Build Terrain generated.
/// The perimeter, the elevation data, features, roads and every other object
/// stay. Returns true when anything was removed.
pub fn clear_generated_only(t: &mut Terrain) -> bool {
    t.last_build.take().is_some()
}

/// Map of the extras kept off the objects (empty by default).
pub type SideExtras = BTreeMap<String, ObjectExtras>;
