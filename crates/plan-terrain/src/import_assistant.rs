//! The Import Terrain Assistant and the Import GPS Data Assistant (manual
//! pp. 1341-1347): which column is which, filtering the points, scaling and
//! rotating them, and what a GPX file becomes.

use plan_core::Point;

use crate::import::{
    attribute, element, next_point_tag, tag_name, ImportFormat, ImportUnit, ImportedPoints,
    EARTH_RADIUS_M, METERS_PER_INCH,
};
use crate::model::{ElevationPoint, Terrain};

// ===================================================================
// Import Terrain Assistant
// ===================================================================

/// How the columns of a text file are arranged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnOrder {
    /// Guessed per line: three numbers are X Y Z, four are a point number
    /// then X Y Z.
    #[default]
    Auto,
    Xyz,
    NXyz,
    NXyzDescription,
    Yxz,
    NYxz,
    NYxzDescription,
}

impl ColumnOrder {
    pub const ALL: [ColumnOrder; 7] = [
        ColumnOrder::Auto,
        ColumnOrder::Xyz,
        ColumnOrder::NXyz,
        ColumnOrder::NXyzDescription,
        ColumnOrder::Yxz,
        ColumnOrder::NYxz,
        ColumnOrder::NYxzDescription,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ColumnOrder::Auto => "Detect from the file",
            ColumnOrder::Xyz => "XYZ",
            ColumnOrder::NXyz => "#XYZ",
            ColumnOrder::NXyzDescription => "#XYZ Description",
            ColumnOrder::Yxz => "YXZ",
            ColumnOrder::NYxz => "#YXZ",
            ColumnOrder::NYxzDescription => "#YXZ Description",
        }
    }

    /// Does the first column hold a point number?
    pub fn has_number(self) -> bool {
        !matches!(self, ColumnOrder::Xyz | ColumnOrder::Yxz)
    }

    /// Does the last column hold a description?
    pub fn has_description(self) -> bool {
        matches!(
            self,
            ColumnOrder::NXyzDescription | ColumnOrder::NYxzDescription
        )
    }

    /// Is the first coordinate column Y (north) rather than X (east)?
    pub fn y_first(self) -> bool {
        matches!(
            self,
            ColumnOrder::Yxz | ColumnOrder::NYxz | ColumnOrder::NYxzDescription
        )
    }
}

/// What separates the columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Delimiter {
    /// Whatever the line uses: commas, semicolons, tabs, then spaces.
    #[default]
    Auto,
    Comma,
    Space,
    Tab,
    Semicolon,
}

impl Delimiter {
    pub const ALL: [Delimiter; 5] = [
        Delimiter::Auto,
        Delimiter::Comma,
        Delimiter::Space,
        Delimiter::Tab,
        Delimiter::Semicolon,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Delimiter::Auto => "Automatic",
            Delimiter::Comma => "Comma",
            Delimiter::Space => "Space",
            Delimiter::Tab => "Tab",
            Delimiter::Semicolon => "Semicolon",
        }
    }

    fn split(self, line: &str) -> Vec<String> {
        let cut = |c: char| match self {
            Delimiter::Auto => c == ',' || c == ';' || c == '\t',
            Delimiter::Comma => c == ',',
            Delimiter::Space => c.is_whitespace(),
            Delimiter::Tab => c == '\t',
            Delimiter::Semicolon => c == ';',
        };
        let auto_space = self == Delimiter::Auto && !line.contains([',', ';', '\t']);
        let parts: Vec<String> = if auto_space {
            line.split_whitespace().map(str::to_string).collect()
        } else {
            line.split(cut).map(|f| f.trim().to_string()).collect()
        };
        parts
    }
}

/// Step 1 of the Import Terrain Assistant: the file's layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextLayout {
    pub order: ColumnOrder,
    pub delimiter: Delimiter,
    /// Header lines to skip before the first point.
    pub skip_lines: usize,
}

/// One row of a survey file, in the file's own units.
#[derive(Debug, Clone, PartialEq)]
pub struct RawPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub number: String,
    pub description: String,
}

/// Reads the rows of `text` as `layout` says. Returns the points and how many
/// lines held no usable point (header and comment lines that do not start with
/// a digit or a sign are not counted).
pub fn read_columns(text: &str, layout: &TextLayout) -> (Vec<RawPoint>, usize) {
    let mut out = Vec::new();
    let mut skipped = 0;
    for line in text
        .lines()
        .skip(layout.skip_lines)
        .map(str::trim)
        .filter(|l| !l.is_empty())
    {
        let fields = layout.delimiter.split(line);
        let o = match layout.order {
            ColumnOrder::Auto => {
                let numeric = fields
                    .iter()
                    .filter(|f| f.parse::<f64>().is_ok())
                    .count();
                if numeric >= 4 && fields.len() == numeric {
                    ColumnOrder::NXyz
                } else {
                    ColumnOrder::Xyz
                }
            }
            o => o,
        };
        let first = usize::from(o.has_number());
        let need = first + 3;
        let num = |i: usize| {
            fields
                .get(i)
                .and_then(|f| f.parse::<f64>().ok())
                .filter(|v| v.is_finite())
        };
        let (a, b, z) = (num(first), num(first + 1), num(first + 2));
        let (Some(a), Some(b), Some(z)) = (a, b, z) else {
            if line.starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '+') {
                skipped += 1;
            }
            continue;
        };
        let (x, y) = if o.y_first() { (b, a) } else { (a, b) };
        let description = if o.has_description() {
            fields.get(need..).map(|f| f.join(" ")).unwrap_or_default()
        } else {
            String::new()
        };
        out.push(RawPoint {
            x,
            y,
            z,
            number: if o.has_number() {
                fields.first().cloned().unwrap_or_default()
            } else {
                String::new()
            },
            description,
        });
    }
    (out, skipped)
}

/// Step 2: which points to keep.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RangeFilter {
    /// Keep points with `lo <= value <= hi` (file units).
    pub x: Option<(f64, f64)>,
    pub y: Option<(f64, f64)>,
    pub z: Option<(f64, f64)>,
    /// Reduce to at most this many evenly spread points.
    pub max_points: Option<usize>,
}

fn within(r: Option<(f64, f64)>, v: f64) -> bool {
    r.is_none_or(|(a, b)| v >= a.min(b) && v <= a.max(b))
}

/// Point count above which the assistant warns that the terrain gets slow.
pub const MANY_POINTS: usize = 2000;

/// Applies the range limits, then thins what is left to `max_points`.
pub fn filter_points(points: &[RawPoint], f: &RangeFilter) -> Vec<RawPoint> {
    let kept: Vec<RawPoint> = points
        .iter()
        .filter(|p| within(f.x, p.x) && within(f.y, p.y) && within(f.z, p.z))
        .cloned()
        .collect();
    match f.max_points {
        Some(n) if n > 0 && kept.len() > n => thin(&kept, n),
        _ => kept,
    }
}

/// Keeps at most `n` points spread evenly over the area they cover: the area
/// is cut into square cells and each cell keeps the point nearest its middle;
/// the cells are made as small as the limit allows.
pub fn thin(points: &[RawPoint], n: usize) -> Vec<RawPoint> {
    if points.len() <= n || n == 0 {
        return points.to_vec();
    }
    let (mut lo, mut hi) = (
        Point::new(f64::INFINITY, f64::INFINITY),
        Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
    );
    for p in points {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let span = (hi.x - lo.x).max(hi.y - lo.y).max(1e-9);
    let pick = |cell: f64| -> Vec<usize> {
        let mut best: std::collections::HashMap<(i64, i64), (f64, usize)> = Default::default();
        for (i, p) in points.iter().enumerate() {
            let key = (
                ((p.x - lo.x) / cell).floor() as i64,
                ((p.y - lo.y) / cell).floor() as i64,
            );
            let cx = lo.x + (key.0 as f64 + 0.5) * cell;
            let cy = lo.y + (key.1 as f64 + 0.5) * cell;
            let d = (p.x - cx).hypot(p.y - cy);
            let e = best.entry(key).or_insert((d, i));
            if d < e.0 {
                *e = (d, i);
            }
        }
        let mut v: Vec<usize> = best.values().map(|(_, i)| *i).collect();
        v.sort_unstable();
        v
    };
    // Smallest cell that keeps at most n points (bisection on the cell size).
    let (mut a, mut b) = (span / (n as f64).sqrt() / 8.0, span * 2.0);
    for _ in 0..40 {
        let mid = (a + b) / 2.0;
        if pick(mid).len() > n {
            a = mid;
        } else {
            b = mid;
        }
    }
    pick(b).into_iter().map(|i| points[i].clone()).collect()
}

/// Step 3: units, origin, relief and rotation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaleOptions {
    pub unit_x: ImportUnit,
    pub unit_y: ImportUnit,
    pub unit_z: ImportUnit,
    /// A file point (file units) that becomes the plan origin.
    pub map_to_origin: Option<(f64, f64)>,
    /// Scale factor for the relief (elevations); 1 keeps them.
    pub relief_scale: f64,
    /// Turns the data counterclockwise about the origin, degrees (rotates
    /// north counterclockwise).
    pub rotate_ccw: f64,
}

impl Default for ScaleOptions {
    fn default() -> Self {
        ScaleOptions::uniform(ImportUnit::Feet)
    }
}

impl ScaleOptions {
    /// The same unit on all three axes.
    pub fn uniform(unit: ImportUnit) -> Self {
        ScaleOptions {
            unit_x: unit,
            unit_y: unit,
            unit_z: unit,
            map_to_origin: None,
            relief_scale: 1.0,
            rotate_ccw: 0.0,
        }
    }
}

/// Converts file points to plan elevation points (inches): the units per axis,
/// the mapped origin, the rotation about it and the relief factor.
pub fn scale_points(points: &[RawPoint], o: &ScaleOptions) -> Vec<ElevationPoint> {
    let (kx, ky, kz) = (o.unit_x.inches(), o.unit_y.inches(), o.unit_z.inches());
    let (ox, oy) = o.map_to_origin.map_or((0.0, 0.0), |(x, y)| (x * kx, y * ky));
    let (sin, cos) = o.rotate_ccw.to_radians().sin_cos();
    let relief = if o.relief_scale.is_finite() && o.relief_scale > 0.0 {
        o.relief_scale
    } else {
        1.0
    };
    points
        .iter()
        .map(|p| {
            let (x, y) = (p.x * kx - ox, p.y * ky - oy);
            ElevationPoint {
                pos: Point::new(x * cos - y * sin, x * sin + y * cos),
                z: p.z * kz * relief,
            }
        })
        .collect()
}

/// Everything the Import Terrain Assistant collects.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TerrainImport {
    pub layout: TextLayout,
    pub filter: RangeFilter,
    pub scale: ScaleOptions,
}

/// Runs the assistant on `text`: columns, filter, scale. DXF drawings and GPX
/// files go through [`crate::import_points`] instead (their layout is fixed).
pub fn import_terrain_text(text: &str, job: &TerrainImport) -> Result<ImportedPoints, String> {
    let (raw, skipped) = read_columns(text, &job.layout);
    if raw.is_empty() {
        return Err("No points found: check the data organization".into());
    }
    let kept = filter_points(&raw, &job.filter);
    if kept.is_empty() {
        return Err("The filter leaves no points".into());
    }
    Ok(ImportedPoints {
        format: ImportFormat::Xyz,
        points: scale_points(&kept, &job.scale),
        skipped,
    })
}

/// Counts for the Filter Data step: points read, kept and the extents of the
/// file (min and max of each coordinate).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DataRanges {
    pub count: usize,
    pub x: (f64, f64),
    pub y: (f64, f64),
    pub z: (f64, f64),
}

/// The extents of `points`, or `None` without points.
pub fn ranges_of(points: &[RawPoint]) -> Option<DataRanges> {
    let first = points.first()?;
    let mut r = DataRanges {
        count: points.len(),
        x: (first.x, first.x),
        y: (first.y, first.y),
        z: (first.z, first.z),
    };
    for p in points {
        r.x = (r.x.0.min(p.x), r.x.1.max(p.x));
        r.y = (r.y.0.min(p.y), r.y.1.max(p.y));
        r.z = (r.z.0.min(p.z), r.z.1.max(p.z));
    }
    Some(r)
}

/// A rectangle around `points`, `margin` inches off their box: the Terrain
/// Perimeter created when an import finds none.
pub fn perimeter_around(points: &[ElevationPoint], margin: f64) -> Vec<Point> {
    let Some(first) = points.first() else {
        return Vec::new();
    };
    let (mut lo, mut hi) = (first.pos, first.pos);
    for p in points {
        lo = Point::new(lo.x.min(p.pos.x), lo.y.min(p.pos.y));
        hi = Point::new(hi.x.max(p.pos.x), hi.y.max(p.pos.y));
    }
    let m = margin.max(0.0);
    vec![
        Point::new(lo.x - m, lo.y - m),
        Point::new(hi.x + m, lo.y - m),
        Point::new(hi.x + m, hi.y + m),
        Point::new(lo.x - m, hi.y + m),
    ]
}

impl Terrain {
    /// Adds imported points and, when `create_perimeter` is set and the
    /// terrain has no perimeter, a perimeter around the data extents. Returns
    /// how many points were new.
    pub fn import_elevation_points(
        &mut self,
        points: &[ElevationPoint],
        create_perimeter: bool,
    ) -> usize {
        let added = self.add_elevation_points(points);
        if create_perimeter && self.perimeter.len() < 3 {
            let margin = (self.grid_spacing / 2.0).max(60.0);
            self.perimeter = perimeter_around(points, margin);
        }
        added
    }
}

// ===================================================================
// Import GPS Data Assistant
// ===================================================================

/// What a GPX item becomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GpsImportAs {
    /// Elevation points (way points only: track points carry no elevation).
    #[default]
    ElevationData,
    /// A marker at each point.
    Marker,
    /// A polyline through the points.
    Polyline,
    /// A closed Terrain Perimeter through the points.
    Perimeter,
}

impl GpsImportAs {
    pub const ALL: [GpsImportAs; 4] = [
        GpsImportAs::ElevationData,
        GpsImportAs::Marker,
        GpsImportAs::Polyline,
        GpsImportAs::Perimeter,
    ];

    pub fn name(self) -> &'static str {
        match self {
            GpsImportAs::ElevationData => "Elevation Data",
            GpsImportAs::Marker => "Marker",
            GpsImportAs::Polyline => "Polyline",
            GpsImportAs::Perimeter => "Terrain Perimeter",
        }
    }
}

/// The kinds of GPX point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpsKind {
    Way,
    Route,
    Track,
}

/// One GPX point.
#[derive(Debug, Clone, PartialEq)]
pub struct GpsPoint {
    pub kind: GpsKind,
    pub lat: f64,
    pub lon: f64,
    /// Elevation, meters (way points only).
    pub ele: Option<f64>,
    pub name: String,
}

/// Reads the points of a GPX 1.1 file (`<wpt>`, `<rtept>`, `<trkpt>`).
pub fn parse_gpx_points(text: &str) -> Result<Vec<GpsPoint>, String> {
    let lower = text.to_ascii_lowercase();
    if !lower.contains("<gpx") {
        return Err("This is not a GPX file".into());
    }
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(rel) = next_point_tag(&lower[at..]) {
        let start = at + rel;
        let Some(open_end) = lower[start..].find('>') else {
            break;
        };
        let open = &text[start..start + open_end];
        let self_closing = open.ends_with('/');
        let name_tag = tag_name(&lower[start + 1..]).to_string();
        at = start + open_end + 1;
        let body = if self_closing {
            ""
        } else {
            let close = format!("</{name_tag}");
            let end = lower[at..].find(&close).map_or(lower.len(), |e| at + e);
            let b = &text[at..end];
            at = end;
            b
        };
        let kind = match name_tag.as_str() {
            "wpt" => GpsKind::Way,
            "rtept" => GpsKind::Route,
            _ => GpsKind::Track,
        };
        let (Some(lat), Some(lon)) = (attribute(open, "lat"), attribute(open, "lon")) else {
            continue;
        };
        if lat.abs() > 90.0 || lon.abs() > 180.0 {
            continue;
        }
        out.push(GpsPoint {
            kind,
            lat,
            lon,
            ele: element(body, "ele"),
            name: text_element(body, "name"),
        });
    }
    Ok(out)
}

fn text_element(body: &str, name: &str) -> String {
    let lower = body.to_ascii_lowercase();
    let open = format!("<{name}>");
    let Some(start) = lower.find(&open).map(|i| i + open.len()) else {
        return String::new();
    };
    let Some(end) = lower[start..].find(&format!("</{name}")).map(|e| e + start) else {
        return String::new();
    };
    body[start..end].trim().to_string()
}

/// Transform Coordinates step of the GPS assistant.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GpsTransform {
    /// Lowers the elevation data by this much, inches (a survey at 1,000 ft
    /// above sea level, say).
    pub lower_by: f64,
    /// Turns the data counterclockwise about the origin, degrees.
    pub rotate_ccw: f64,
    /// The latitude and longitude that become the plan origin (the first point
    /// when `None`).
    pub origin: Option<(f64, f64)>,
}

/// What the GPS assistant makes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GpsResult {
    pub elevation_points: Vec<ElevationPoint>,
    pub markers: Vec<(Point, String)>,
    pub polyline: Vec<Point>,
    pub perimeter: Vec<Point>,
    /// Route points left out (Chief ignores them).
    pub route_ignored: usize,
    /// Way points that carried no elevation.
    pub no_elevation: usize,
}

impl GpsResult {
    pub fn is_empty(&self) -> bool {
        self.elevation_points.is_empty()
            && self.markers.is_empty()
            && self.polyline.is_empty()
            && self.perimeter.is_empty()
    }

    /// A one-line summary.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.elevation_points.is_empty() {
            parts.push(format!("{} elevation points", self.elevation_points.len()));
        }
        if !self.markers.is_empty() {
            parts.push(format!("{} markers", self.markers.len()));
        }
        if !self.polyline.is_empty() {
            parts.push(format!("a {}-point polyline", self.polyline.len()));
        }
        if !self.perimeter.is_empty() {
            parts.push(format!("a {}-point perimeter", self.perimeter.len()));
        }
        let mut s = if parts.is_empty() {
            "Nothing to import".to_string()
        } else {
            parts.join(", ")
        };
        if self.route_ignored > 0 {
            s.push_str(&format!("; {} route points ignored", self.route_ignored));
        }
        if self.no_elevation > 0 {
            s.push_str(&format!("; {} way points without elevation", self.no_elevation));
        }
        s
    }
}

/// Runs the Import GPS Data Assistant. Way points go where `waypoints_as`
/// says, track points where `tracks_as` says (`ElevationData` is not possible
/// for them: they carry no elevation, so they are left out). Route points are
/// ignored.
pub fn import_gps(
    text: &str,
    waypoints_as: GpsImportAs,
    tracks_as: GpsImportAs,
    tr: &GpsTransform,
) -> Result<GpsResult, String> {
    let pts = parse_gpx_points(text)?;
    let mut out = GpsResult::default();
    out.route_ignored = pts.iter().filter(|p| p.kind == GpsKind::Route).count();
    let origin = tr
        .origin
        .or_else(|| {
            pts.iter()
                .find(|p| p.kind != GpsKind::Route)
                .map(|p| (p.lat, p.lon))
        })
        .ok_or("The file has no way points or track points")?;
    let cos_lat = origin.0.to_radians().cos();
    let (sin, cos) = tr.rotate_ccw.to_radians().sin_cos();
    let plan = |p: &GpsPoint| {
        let east = (p.lon - origin.1).to_radians() * cos_lat * EARTH_RADIUS_M / METERS_PER_INCH;
        let north = (p.lat - origin.0).to_radians() * EARTH_RADIUS_M / METERS_PER_INCH;
        Point::new(east * cos - north * sin, east * sin + north * cos)
    };
    for (kind, as_) in [(GpsKind::Way, waypoints_as), (GpsKind::Track, tracks_as)] {
        let group: Vec<&GpsPoint> = pts.iter().filter(|p| p.kind == kind).collect();
        match as_ {
            GpsImportAs::ElevationData => {
                for p in group {
                    if kind != GpsKind::Way {
                        continue;
                    }
                    match p.ele {
                        Some(e) => out.elevation_points.push(ElevationPoint {
                            pos: plan(p),
                            z: e / METERS_PER_INCH - tr.lower_by,
                        }),
                        None => out.no_elevation += 1,
                    }
                }
            }
            GpsImportAs::Marker => {
                out.markers
                    .extend(group.iter().map(|p| (plan(p), p.name.clone())));
            }
            GpsImportAs::Polyline => {
                out.polyline.extend(group.iter().map(|p| plan(p)));
            }
            GpsImportAs::Perimeter => {
                out.perimeter.extend(group.iter().map(|p| plan(p)));
            }
        }
    }
    // Several points at one spot are one point.
    out.elevation_points.dedup_by(|a, b| a.pos.dist(b.pos) < 0.01);
    if out.perimeter.len() < 3 {
        out.perimeter.clear();
    }
    if out.polyline.len() < 2 {
        out.polyline.clear();
    }
    if out.is_empty() {
        return Err(format!("{}; nothing to import", out.summary()));
    }
    Ok(out)
}
