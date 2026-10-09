//! Import of survey points into the terrain's elevation data.
//!
//! Three text formats are read, all by hand (no parser crates):
//!
//! * **DXF** (ASCII): `POINT`, `3DFACE`, the `VERTEX` records of a `POLYLINE`
//!   and the vertices of an `LWPOLYLINE` (all at its elevation, group 38), as a
//!   survey or a civil package exports spot heights. The drawing unit comes
//!   from `$INSUNITS` when the file has one;
//! * **GPX**: way points (`<wpt>`) with `lat`, `lon` and `<ele>` in meters.
//!   Route and track points carry no elevation and are not read here (the GPS
//!   assistant in `import_assistant` makes markers, polylines and perimeters of
//!   them). They are projected onto a local flat plan around the first point
//!   (east is plan x, north is plan y);
//! * **XYZ text**: one point per line, `x y z` separated by spaces, commas,
//!   semicolons or tabs. Lines that do not start with numbers (headers,
//!   comments) are skipped.
//!
//! The result is a list of [`ElevationPoint`]s in inches, ready to append to a
//! [`Terrain`] ([`Terrain::add_elevation_points`]).

use plan_core::Point;

use crate::model::{ElevationPoint, Terrain};

/// Meters in an inch.
pub(crate) const METERS_PER_INCH: f64 = 0.0254;
/// Mean radius of the earth, meters (the GPX projection).
pub(crate) const EARTH_RADIUS_M: f64 = 6_371_008.8;
/// Points closer than this to an existing point count as the same point, inches.
const SAME_POINT: f64 = 0.01;

/// The unit of the coordinates in a DXF or XYZ file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImportUnit {
    /// DXF: the drawing's `$INSUNITS` (feet when it has none). XYZ: feet.
    #[default]
    Auto,
    Inches,
    Feet,
    Meters,
    Millimeters,
}

impl ImportUnit {
    pub const ALL: [ImportUnit; 5] = [
        ImportUnit::Auto,
        ImportUnit::Feet,
        ImportUnit::Inches,
        ImportUnit::Meters,
        ImportUnit::Millimeters,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ImportUnit::Auto => "From the file (feet if unknown)",
            ImportUnit::Inches => "Inches",
            ImportUnit::Feet => "Feet",
            ImportUnit::Meters => "Meters",
            ImportUnit::Millimeters => "Millimeters",
        }
    }

    /// Inches in one of the unit (`Auto` is feet).
    pub fn inches(self) -> f64 {
        match self {
            ImportUnit::Inches => 1.0,
            ImportUnit::Auto | ImportUnit::Feet => 12.0,
            ImportUnit::Meters => 1.0 / METERS_PER_INCH,
            ImportUnit::Millimeters => 1.0 / (METERS_PER_INCH * 1000.0),
        }
    }

    /// The unit a DXF `$INSUNITS` code names.
    fn from_dxf_code(code: i64) -> Option<ImportUnit> {
        Some(match code {
            1 => ImportUnit::Inches,
            2 => ImportUnit::Feet,
            4 => ImportUnit::Millimeters,
            5 | 6 => ImportUnit::Meters,
            _ => return None,
        })
    }
}

/// The file formats the importer reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFormat {
    Dxf,
    Gpx,
    Xyz,
}

impl ImportFormat {
    pub fn name(self) -> &'static str {
        match self {
            ImportFormat::Dxf => "DXF",
            ImportFormat::Gpx => "GPX",
            ImportFormat::Xyz => "XYZ text",
        }
    }
}

/// What an import read.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedPoints {
    pub format: ImportFormat,
    /// The points, inches (elevation in `z`).
    pub points: Vec<ElevationPoint>,
    /// Records or lines that held no usable point.
    pub skipped: usize,
}

impl ImportedPoints {
    /// Corners of the box around the points, or `None` without points.
    pub fn bounds(&self) -> Option<(Point, Point)> {
        let first = self.points.first()?.pos;
        Some(self.points.iter().fold((first, first), |(lo, hi), e| {
            (
                Point::new(lo.x.min(e.pos.x), lo.y.min(e.pos.y)),
                Point::new(hi.x.max(e.pos.x), hi.y.max(e.pos.y)),
            )
        }))
    }

    /// Lowest and highest elevation, or `None` without points.
    pub fn elevation_range(&self) -> Option<(f64, f64)> {
        let first = self.points.first()?.z;
        Some(
            self.points
                .iter()
                .fold((first, first), |(lo, hi), e| (lo.min(e.z), hi.max(e.z))),
        )
    }

    /// Moves the points so the middle of their box lies on `center`.
    pub fn center_on(&mut self, center: Point) {
        let Some((lo, hi)) = self.bounds() else {
            return;
        };
        let shift = center - Point::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
        for e in &mut self.points {
            e.pos = e.pos + shift;
        }
    }

    /// Lowers every elevation by the lowest one, so the lowest point is at 0.
    pub fn zero_lowest(&mut self) {
        if let Some((lo, _)) = self.elevation_range() {
            for e in &mut self.points {
                e.z -= lo;
            }
        }
    }

    /// A one-line summary for the dialog and the status bar.
    pub fn summary(&self) -> String {
        let mut s = format!(
            "{} points read from {}",
            self.points.len(),
            self.format.name()
        );
        if let Some((lo, hi)) = self.elevation_range() {
            s.push_str(&format!(
                "; elevations {} to {}",
                plan_core::units::fmt_ft_in_frac(lo, 2),
                plan_core::units::fmt_ft_in_frac(hi, 2)
            ));
        }
        if self.skipped > 0 {
            s.push_str(&format!("; {} records skipped", self.skipped));
        }
        s
    }
}

/// Reads survey points from `text` in whichever of the three formats it is.
/// `unit` applies to DXF and XYZ coordinates (GPX is degrees and meters).
pub fn import_points(text: &str, unit: ImportUnit) -> Result<ImportedPoints, String> {
    let lower_head: String = text.chars().take(4096).collect::<String>().to_lowercase();
    let result = if lower_head.contains("<gpx") {
        parse_gpx(text)
    } else if looks_like_dxf(text) {
        parse_dxf(text, unit)
    } else {
        parse_xyz(text, unit)
    };
    if result.points.is_empty() {
        Err(format!(
            "No elevation points found in the {} text",
            result.format.name()
        ))
    } else {
        Ok(result)
    }
}

fn looks_like_dxf(text: &str) -> bool {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    matches!((lines.next(), lines.next()), (Some("0"), Some("SECTION")))
}

// ----- DXF -----

/// One DXF entity as the group codes it carries.
#[derive(Default)]
struct DxfEntity {
    kind: String,
    /// (code, value) pairs in file order.
    groups: Vec<(i64, String)>,
}

fn dxf_entities(text: &str) -> (Vec<DxfEntity>, Option<ImportUnit>) {
    let mut lines = text.lines();
    let mut entities: Vec<DxfEntity> = Vec::new();
    let mut units = None;
    let mut last_variable = String::new();
    while let (Some(code), Some(value)) = (lines.next(), lines.next()) {
        let Ok(code) = code.trim().parse::<i64>() else {
            continue;
        };
        let value = value.trim();
        match code {
            0 => entities.push(DxfEntity {
                kind: value.to_ascii_uppercase(),
                groups: Vec::new(),
            }),
            9 => last_variable = value.to_ascii_uppercase(),
            70 if last_variable == "$INSUNITS"
                && entities.last().is_some_and(|e| e.kind == "SECTION") =>
            {
                units = value
                    .parse::<i64>()
                    .ok()
                    .and_then(ImportUnit::from_dxf_code);
                last_variable.clear();
            }
            _ => {
                if let Some(e) = entities.last_mut() {
                    e.groups.push((code, value.to_string()));
                }
            }
        }
    }
    (entities, units)
}

fn number(groups: &[(i64, String)], code: i64) -> Option<f64> {
    groups
        .iter()
        .find(|(c, _)| *c == code)
        .and_then(|(_, v)| v.parse::<f64>().ok())
        .filter(|v| v.is_finite())
}

fn parse_dxf(text: &str, unit: ImportUnit) -> ImportedPoints {
    let (entities, header_unit) = dxf_entities(text);
    let unit = match unit {
        ImportUnit::Auto => header_unit.unwrap_or(ImportUnit::Feet),
        u => u,
    };
    let k = unit.inches();
    let mut out = ImportedPoints {
        format: ImportFormat::Dxf,
        points: Vec::new(),
        skipped: 0,
    };
    let mut push = |x: f64, y: f64, z: f64| {
        out.points.push(ElevationPoint {
            pos: Point::new(x * k, y * k),
            z: z * k,
        });
    };
    let mut skipped = 0;
    for e in &entities {
        match e.kind.as_str() {
            "POINT" | "VERTEX" => match (number(&e.groups, 10), number(&e.groups, 20)) {
                // A polyline's spline-frame and control records have flag bits 16/128; the
                // plain vertices of a 3D polyline carry 32 or 0.
                (Some(x), Some(y)) if vertex_is_plain(e) => {
                    push(x, y, number(&e.groups, 30).unwrap_or(0.0));
                }
                (Some(_), Some(_)) => {}
                _ => skipped += 1,
            },
            "3DFACE" => {
                let mut any = false;
                for i in 0..4 {
                    if let (Some(x), Some(y), Some(z)) = (
                        number(&e.groups, 10 + i),
                        number(&e.groups, 20 + i),
                        number(&e.groups, 30 + i),
                    ) {
                        push(x, y, z);
                        any = true;
                    }
                }
                if !any {
                    skipped += 1;
                }
            }
            "LWPOLYLINE" => {
                let z = number(&e.groups, 38).unwrap_or(0.0);
                let xs = e.groups.iter().filter(|(c, _)| *c == 10);
                let ys = e.groups.iter().filter(|(c, _)| *c == 20);
                let mut any = false;
                for ((_, x), (_, y)) in xs.zip(ys) {
                    if let (Ok(x), Ok(y)) = (x.parse::<f64>(), y.parse::<f64>()) {
                        push(x, y, z);
                        any = true;
                    }
                }
                if !any {
                    skipped += 1;
                }
            }
            _ => {}
        }
    }
    out.skipped = skipped;
    dedup(&mut out.points);
    out
}

/// A VERTEX that is a polyline's own point, not a spline control or frame record.
fn vertex_is_plain(e: &DxfEntity) -> bool {
    let flags = number(&e.groups, 70).unwrap_or(0.0) as i64;
    flags & (16 | 128) == 0
}

// ----- GPX -----

fn parse_gpx(text: &str) -> ImportedPoints {
    let mut raw: Vec<(f64, f64, Option<f64>)> = Vec::new();
    let mut skipped = 0;
    let lower = text.to_ascii_lowercase();
    let mut at = 0;
    while let Some(rel) = next_point_tag(&lower[at..]) {
        let start = at + rel;
        let Some(open_end) = lower[start..].find('>') else {
            break;
        };
        let open = &text[start..start + open_end];
        let self_closing = open.ends_with('/');
        // Only way points carry elevation; route points are ignored and track
        // points are markers, polylines or a perimeter (the GPS assistant).
        let is_way = tag_name(&lower[start + 1..]) == "wpt";
        at = start + open_end + 1;
        let body = if self_closing {
            ""
        } else {
            let name = tag_name(&lower[start + 1..]);
            let close = format!("</{name}");
            let end = lower[at..].find(&close).map_or(lower.len(), |e| at + e);
            let b = &text[at..end];
            at = end;
            b
        };
        if !is_way {
            continue;
        }
        let (lat, lon) = (attribute(open, "lat"), attribute(open, "lon"));
        match (lat, lon) {
            (Some(lat), Some(lon)) if lat.abs() <= 90.0 && lon.abs() <= 180.0 => {
                raw.push((lat, lon, element(body, "ele")));
            }
            _ => skipped += 1,
        }
    }
    let mut out = ImportedPoints {
        format: ImportFormat::Gpx,
        points: Vec::new(),
        skipped,
    };
    let Some(&(lat0, lon0, _)) = raw.first() else {
        return out;
    };
    let cos_lat = lat0.to_radians().cos();
    for (lat, lon, ele) in raw {
        let east = (lon - lon0).to_radians() * cos_lat * EARTH_RADIUS_M;
        let north = (lat - lat0).to_radians() * EARTH_RADIUS_M;
        out.points.push(ElevationPoint {
            pos: Point::new(east / METERS_PER_INCH, north / METERS_PER_INCH),
            z: ele.unwrap_or(0.0) / METERS_PER_INCH,
        });
    }
    dedup(&mut out.points);
    out
}

/// Offset of the next `<wpt`, `<rtept` or `<trkpt` tag in `lower`.
pub(crate) fn next_point_tag(lower: &str) -> Option<usize> {
    ["<wpt", "<rtept", "<trkpt"]
        .iter()
        .filter_map(|tag| {
            let mut from = 0;
            while let Some(i) = lower[from..].find(tag) {
                let end = from + i + tag.len();
                // The tag name must end here (`<wptx` is something else).
                if lower[end..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_whitespace() || c == '>' || c == '/')
                {
                    return Some(from + i);
                }
                from = end;
            }
            None
        })
        .min()
}

/// The element name that starts at the beginning of `s` (after the `<`).
pub(crate) fn tag_name(s: &str) -> &str {
    let end = s
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .unwrap_or(s.len());
    &s[..end]
}

/// The numeric value of attribute `name` in a start tag.
pub(crate) fn attribute(tag: &str, name: &str) -> Option<f64> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find(name) {
        let at = from + i;
        let before_ok = at == 0
            || lower[..at]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace);
        let rest = lower[at + name.len()..].trim_start();
        if before_ok && rest.starts_with('=') {
            let value = &tag[tag.len() - rest.len() + 1..];
            let value = value.trim_start();
            let quote = value.chars().next().filter(|c| *c == '"' || *c == '\'')?;
            let inner = &value[1..];
            let end = inner.find(quote)?;
            return inner[..end]
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite());
        }
        from = at + name.len();
    }
    None
}

/// The number inside `<name>..</name>` in `body`.
pub(crate) fn element(body: &str, name: &str) -> Option<f64> {
    let lower = body.to_ascii_lowercase();
    let open = format!("<{name}>");
    let start = lower.find(&open)? + open.len();
    let end = lower[start..].find(&format!("</{name}"))? + start;
    body[start..end]
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

// ----- XYZ text -----

fn parse_xyz(text: &str, unit: ImportUnit) -> ImportedPoints {
    let k = unit.inches();
    let mut out = ImportedPoints {
        format: ImportFormat::Xyz,
        points: Vec::new(),
        skipped: 0,
    };
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let nums: Vec<f64> = line
            .split(|c: char| c == ',' || c == ';' || c == '\t' || c.is_whitespace())
            .filter(|f| !f.is_empty())
            .map(|f| f.parse::<f64>().ok().filter(|v| v.is_finite()))
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default();
        // A numbered point list has a leading id: `1,x,y,z`.
        let (x, y, z) = match nums.as_slice() {
            [x, y, z] | [_, x, y, z] => (*x, *y, *z),
            [x, y] => (*x, *y, 0.0),
            _ => {
                // Header and comment lines are not counted as skipped data.
                if line
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit() || c == '-')
                {
                    out.skipped += 1;
                }
                continue;
            }
        };
        out.points.push(ElevationPoint {
            pos: Point::new(x * k, y * k),
            z: z * k,
        });
    }
    dedup(&mut out.points);
    out
}

/// Drops points at the same position as an earlier one.
fn dedup(points: &mut Vec<ElevationPoint>) {
    let mut kept: Vec<ElevationPoint> = Vec::with_capacity(points.len());
    for p in points.drain(..) {
        if !kept.iter().any(|q| q.pos.dist(p.pos) < SAME_POINT) {
            kept.push(p);
        }
    }
    *points = kept;
}

impl Terrain {
    /// Appends survey points to the elevation data, skipping any that fall on
    /// an existing point. Returns how many were added.
    pub fn add_elevation_points(&mut self, points: &[ElevationPoint]) -> usize {
        let before = self.elevation_points.len();
        for p in points {
            if !self
                .elevation_points
                .iter()
                .any(|q| q.pos.dist(p.pos) < SAME_POINT)
            {
                self.elevation_points.push(p.clone());
            }
        }
        self.elevation_points.len() - before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DXF: &str =
        "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n2\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n\
0\nPOINT\n8\n0\n10\n10.0\n20\n20.0\n30\n5.0\n\
0\nPOINT\n10\n30.0\n20\n20.0\n30\n6.5\n\
0\nPOINT\n10\n30.0\n20\n20.0\n30\n7.0\n\
0\nLWPOLYLINE\n90\n2\n38\n8.0\n10\n0.0\n20\n0.0\n10\n10.0\n20\n0.0\n\
0\nPOLYLINE\n70\n8\n0\nVERTEX\n10\n50.0\n20\n50.0\n30\n9.0\n70\n32\n\
0\nVERTEX\n10\n60.0\n20\n50.0\n30\n9.5\n70\n16\n0\nSEQEND\n\
0\nPOINT\n8\n0\n0\nENDSEC\n0\nEOF\n";

    #[test]
    fn dxf_points_polylines_and_units() {
        let r = import_points(DXF, ImportUnit::Auto).unwrap();
        assert_eq!(r.format, ImportFormat::Dxf);
        // POINT x2 (the third sits on the second and is dropped), LWPOLYLINE x2, one plain VERTEX.
        assert_eq!(r.points.len(), 5);
        // $INSUNITS 2 = feet.
        assert_eq!(r.points[0].pos, Point::new(120.0, 240.0));
        assert_eq!(r.points[0].z, 60.0);
        assert_eq!(r.points[2].z, 96.0);
        // The POINT without coordinates is counted, not read.
        assert_eq!(r.skipped, 1);
        let m = import_points(DXF, ImportUnit::Meters).unwrap();
        assert!((m.points[0].pos.x - 10.0 / METERS_PER_INCH).abs() < 1e-6);
    }

    const GPX: &str = r#"<?xml version="1.0"?>
<gpx version="1.1"><wpt lat="33.7490" lon="-84.3880"><ele>320.0</ele><name>A</name></wpt>
<wpt lat='33.7500' lon='-84.3880'><ele>321.5</ele></wpt>
<wpt lat="33.7490" lon="-84.3870"/>
<wpt lat="95" lon="0"><ele>1</ele></wpt>
<rte><rtept lat="33.8" lon="-84.3"><ele>9</ele></rtept></rte>
<trk><trkseg>
<trkpt lat="33.7000" lon="-84.3000"><ele>5</ele></trkpt>
</trkseg></trk></gpx>"#;

    #[test]
    fn gpx_waypoints_project_onto_a_local_plan_and_other_points_are_not_elevation() {
        let r = import_points(GPX, ImportUnit::Feet).unwrap();
        assert_eq!(r.format, ImportFormat::Gpx);
        assert_eq!(r.points.len(), 3);
        assert_eq!(r.skipped, 1);
        // The first point is the origin; 0.001 degrees north is about 111 m.
        assert_eq!(r.points[0].pos, Point::new(0.0, 0.0));
        let north_m = r.points[1].pos.y * METERS_PER_INCH;
        assert!((north_m - 111.19).abs() < 0.5, "{north_m}");
        // 0.001 degrees east at 33.7 degrees is about 92.5 m.
        let east_m = r.points[2].pos.x * METERS_PER_INCH;
        assert!((east_m - 92.5).abs() < 0.5, "{east_m}");
        // Elevations are meters; a point without one is at 0.
        assert!((r.points[1].z * METERS_PER_INCH - 321.5).abs() < 1e-9);
        assert_eq!(r.points[2].z, 0.0);
    }

    #[test]
    fn xyz_text_with_headers_ids_and_separators() {
        let text = "# survey\nPoint,X,Y,Z\n1,0,0,10\n2, 10, 0, 11\n3;10;10;12\n4\t0\t10\t13\nbad line 7\n12 x 4\n";
        let r = import_points(text, ImportUnit::Feet).unwrap();
        assert_eq!(r.format, ImportFormat::Xyz);
        assert_eq!(r.points.len(), 4);
        assert_eq!(r.points[1].pos, Point::new(120.0, 0.0));
        assert_eq!(r.points[3].z, 156.0);
        // `12 x 4` starts with a number but is not a point.
        assert_eq!(r.skipped, 1);
        assert!(import_points("hello\nworld", ImportUnit::Feet).is_err());
    }

    #[test]
    fn centering_zeroing_and_merging() {
        let mut r = import_points("0 0 100\n10 0 104\n10 10 102", ImportUnit::Inches).unwrap();
        r.center_on(Point::new(600.0, 480.0));
        let (lo, hi) = r.bounds().unwrap();
        assert_eq!((lo.x + hi.x) / 2.0, 600.0);
        assert_eq!((lo.y + hi.y) / 2.0, 480.0);
        r.zero_lowest();
        assert_eq!(r.elevation_range(), Some((0.0, 4.0)));
        let mut t = Terrain::default();
        assert_eq!(t.add_elevation_points(&r.points), 3);
        assert_eq!(t.add_elevation_points(&r.points), 0);
        assert!(r.summary().starts_with("3 points read from XYZ text"));
    }
}
