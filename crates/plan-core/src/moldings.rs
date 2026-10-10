//! The shared molding model (Chief Reference Manual ch. 29, pp. 964 to 989).
//!
//! Every object with a Moldings panel (molding polylines, rooms, the Floor
//! Defaults, cabinets, tray ceilings, wall caps, soffits) edits the same data:
//!
//! * a [`ProfileDef`] is a cross section drawn at actual size as one or more
//!   closed polylines ([`ProfilePart`]; several parts with their own
//!   materials make a stacked profile). The back of the molding is the
//!   vertical edge at `x = 0`, the section projects toward `+x` and its
//!   bottom is at `y = 0`; a section is counter-clockwise;
//! * a [`MoldingEntry`] is one row of the Moldings table: the profile and
//!   its Width, Height, Repeat Distance, offsets, rotation and so on;
//! * a [`MoldingTable`] is the table itself with the buttons of the panel as
//!   methods (Add New, Make Copy, Delete, Move Up/Down, Make Stack, Explode
//!   Stack, Replace, Default).
//!
//! # Rooms
//!
//! [`RoomMoldings`] carries a room's own table (Use Floor Default, No Change
//! or its own rows) and the edges without molding;
//! [`room_molding_lines`] turns the effective table into molding lines along
//! the interior surfaces, leaving out openings, walls that are not drawn,
//! cabinets that stand in the way ([`cabinet_obstacles`]) and edges switched
//! off. [`legacy_room_lines`] does the same for the base, chair rail and
//! crown of the older Moldings tab (`RoomName.moldings`), which is what the
//! Make Room Molding Polyline edit button converts.
//!
//! # Reporting
//!
//! [`trim_takeoff`] lists linear lengths under "Interior Trim" and
//! "Exterior Trim" for the Materials List.

use crate::details::{DetailsLayer, MoldingLine, MoldingProfile, MoldingSource, MOLDING_LAYER};
use crate::extras::{MoldingKind, MoldingRef};
use crate::geometry::{point_in_polygon, polygon_area, segment_intersection, Point};
use crate::model::{Floor, Id, Project, WallKind};
use crate::rooms::{
    edge_openings, edge_wall, molding_def, molding_defs, MoldingDef, Room, CHAIR_RAIL_HEIGHT,
    MOLDING_LIBRARY,
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Smallest extent of a profile, inches.
pub const MIN_EXTENT: f64 = 0.01;
/// Name of the default square profile of the Molding Polyline tool.
pub const SQUARE_PROFILE: &str = "Square 1 x 1";
/// Materials List category of interior moldings.
pub const INTERIOR_TRIM: &str = "Interior Trim";
/// Materials List category of exterior moldings and corner trim.
pub const EXTERIOR_TRIM: &str = "Exterior Trim";

// ===================================================================
// Types
// ===================================================================

/// What a profile is for (the Type of the Selected Profile Options).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum MoldingType {
    #[default]
    Base,
    ChairRail,
    Crown,
    Casing,
    WallCap,
    EdgeProfile,
    Rail,
    StripProfile,
    Eave,
    Gable,
    Other,
}

impl MoldingType {
    pub const ALL: [MoldingType; 11] = [
        MoldingType::Base,
        MoldingType::ChairRail,
        MoldingType::Crown,
        MoldingType::Casing,
        MoldingType::WallCap,
        MoldingType::EdgeProfile,
        MoldingType::Rail,
        MoldingType::StripProfile,
        MoldingType::Eave,
        MoldingType::Gable,
        MoldingType::Other,
    ];

    pub fn name(self) -> &'static str {
        match self {
            MoldingType::Base => "Base",
            MoldingType::ChairRail => "Chair Rail",
            MoldingType::Crown => "Crown",
            MoldingType::Casing => "Casing",
            MoldingType::WallCap => "Wall Cap",
            MoldingType::EdgeProfile => "Edge Profile",
            MoldingType::Rail => "Rail",
            MoldingType::StripProfile => "Strip Profile",
            MoldingType::Eave => "Eave",
            MoldingType::Gable => "Gable",
            MoldingType::Other => "Other",
        }
    }

    pub fn from_kind(kind: MoldingKind) -> MoldingType {
        match kind {
            MoldingKind::Base => MoldingType::Base,
            MoldingKind::Chair => MoldingType::ChairRail,
            MoldingKind::Crown => MoldingType::Crown,
        }
    }

    /// The room molding kind of a base, chair rail or crown.
    pub fn kind(self) -> Option<MoldingKind> {
        match self {
            MoldingType::Base => Some(MoldingKind::Base),
            MoldingType::ChairRail => Some(MoldingKind::Chair),
            MoldingType::Crown => Some(MoldingKind::Crown),
            _ => None,
        }
    }

    /// Eave and gable moldings are exterior trim; the rest is interior.
    pub fn is_exterior(self) -> bool {
        matches!(self, MoldingType::Eave | MoldingType::Gable)
    }

    /// Elevation of the bottom of a molding of this type and `height` above
    /// the floor for a ceiling at `ceiling`, inches (base on the floor, chair
    /// rail 32 inches up, crown hanging from the ceiling).
    pub fn default_bottom(self, ceiling: f64, height: f64) -> f64 {
        match self {
            MoldingType::Crown => (ceiling - height).max(0.0),
            MoldingType::ChairRail => CHAIR_RAIL_HEIGHT,
            _ => 0.0,
        }
    }
}

/// On Selected Edge of the Moldings panel (ridge caps, gutters, shadow
/// boards and per-edge moldings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EdgeMode {
    #[default]
    Automatic,
    On,
    Off,
}

impl EdgeMode {
    pub const ALL: [EdgeMode; 3] = [EdgeMode::Automatic, EdgeMode::On, EdgeMode::Off];

    pub fn name(self) -> &'static str {
        match self {
            EdgeMode::Automatic => "Automatic",
            EdgeMode::On => "On",
            EdgeMode::Off => "Off",
        }
    }
}

/// Where a table's rows come from: its own, the floor's defaults, or left
/// as they are when several selected objects differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TableSource {
    #[default]
    Own,
    UseFloorDefault,
    NoChange,
}

impl TableSource {
    pub fn name(self) -> &'static str {
        match self {
            TableSource::Own => "Own moldings",
            TableSource::UseFloorDefault => "Use Floor Default",
            TableSource::NoChange => "No Change",
        }
    }
}

// ===================================================================
// Profiles
// ===================================================================

/// Why a polyline cannot be a molding profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileError {
    TooFewPoints,
    NoArea,
    SelfIntersecting,
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ProfileError::TooFewPoints => "A profile needs a closed polyline of at least 3 points",
            ProfileError::NoArea => "The polyline encloses no area",
            ProfileError::SelfIntersecting => "The polyline crosses itself",
        })
    }
}

/// A closed section polyline with the material it is made of ("" follows the
/// molding's).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfilePart {
    pub section: Vec<Point>,
    pub material: String,
}

/// A molding profile: a name, a type and one or more section polylines.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileDef {
    pub name: String,
    pub kind: MoldingType,
    pub parts: Vec<ProfilePart>,
}

/// A closed polyline without the repeated closing point and repeated points,
/// counter-clockwise; an error when it cannot enclose an area.
pub fn clean_section(pts: &[Point]) -> Result<Vec<Point>, ProfileError> {
    let mut v: Vec<Point> = Vec::with_capacity(pts.len());
    for p in pts {
        if v.last().is_none_or(|q| q.dist(*p) > 1e-9) {
            v.push(*p);
        }
    }
    while v.len() > 1 && v[0].dist(v[v.len() - 1]) <= 1e-9 {
        v.pop();
    }
    if v.len() < 3 {
        return Err(ProfileError::TooFewPoints);
    }
    let n = v.len();
    for i in 0..n {
        for j in i + 1..n {
            // Edges that share a vertex touch by construction.
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            if segment_intersection(v[i], v[(i + 1) % n], v[j], v[(j + 1) % n]).is_some() {
                return Err(ProfileError::SelfIntersecting);
            }
        }
    }
    let area = polygon_area(&v);
    if area.abs() < 1e-9 {
        return Err(ProfileError::NoArea);
    }
    if area < 0.0 {
        v.reverse();
    }
    Ok(v)
}

fn bounds_of<'a>(pts: impl IntoIterator<Item = &'a Point>) -> Option<(Point, Point)> {
    let mut it = pts.into_iter();
    let first = *it.next()?;
    let (mut lo, mut hi) = (first, first);
    for p in it {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    Some((lo, hi))
}

impl ProfileDef {
    /// A profile from one closed polyline, moved so its back-bottom corner is
    /// the origin (Add to Library on a closed polyline).
    pub fn from_polyline(
        name: impl Into<String>,
        kind: MoldingType,
        pts: &[Point],
    ) -> Result<ProfileDef, ProfileError> {
        ProfileDef::stacked(name, kind, vec![(pts.to_vec(), String::new())])
    }

    /// A stacked profile from several polylines with their materials (Add to
    /// Library as Stacked Molding). The parts keep their positions relative
    /// to each other; together they are moved to the origin.
    pub fn stacked(
        name: impl Into<String>,
        kind: MoldingType,
        parts: Vec<(Vec<Point>, String)>,
    ) -> Result<ProfileDef, ProfileError> {
        if parts.is_empty() {
            return Err(ProfileError::TooFewPoints);
        }
        let mut cleaned = Vec::with_capacity(parts.len());
        for (pts, material) in parts {
            cleaned.push(ProfilePart {
                section: clean_section(&pts)?,
                material,
            });
        }
        let lo = bounds_of(cleaned.iter().flat_map(|p| p.section.iter()))
            .map(|b| b.0)
            .unwrap_or(Point::ZERO);
        for part in &mut cleaned {
            for p in &mut part.section {
                *p = *p - lo;
            }
        }
        Ok(ProfileDef {
            name: name.into(),
            kind,
            parts: cleaned,
        })
    }

    /// A built-in library profile.
    pub fn from_def(def: &MoldingDef) -> ProfileDef {
        ProfileDef {
            name: def.name.to_string(),
            kind: MoldingType::from_kind(def.kind),
            parts: vec![ProfilePart {
                section: def.points(),
                material: String::new(),
            }],
        }
    }

    /// `(lowest corner, highest corner)` over all parts.
    pub fn bounds(&self) -> (Point, Point) {
        bounds_of(self.parts.iter().flat_map(|p| p.section.iter()))
            .unwrap_or((Point::ZERO, Point::ZERO))
    }

    /// How far the profile projects, inches.
    pub fn width(&self) -> f64 {
        let (lo, hi) = self.bounds();
        hi.x - lo.x
    }

    /// Vertical size, inches.
    pub fn height(&self) -> f64 {
        let (lo, hi) = self.bounds();
        hi.y - lo.y
    }

    pub fn is_stack(&self) -> bool {
        self.parts.len() > 1
    }

    /// Every part is a usable closed section.
    pub fn is_valid(&self) -> bool {
        !self.parts.is_empty()
            && self.parts.iter().all(|p| clean_section(&p.section).is_ok())
            && self.width() > MIN_EXTENT
            && self.height() > MIN_EXTENT
    }

    /// The first part's section (the profile of a single-polyline molding).
    pub fn section(&self) -> &[Point] {
        self.parts.first().map_or(&[], |p| &p.section)
    }
}

/// The default square profile of the Molding Polyline tool.
pub fn square_profile() -> ProfileDef {
    let sq = vec![
        Point::new(0.0, 0.0),
        Point::new(1.0, 0.0),
        Point::new(1.0, 1.0),
        Point::new(0.0, 1.0),
    ];
    ProfileDef {
        name: SQUARE_PROFILE.to_string(),
        kind: MoldingType::Other,
        parts: vec![ProfilePart {
            section: sq,
            material: String::new(),
        }],
    }
}

fn rect_profile(name: &str, kind: MoldingType, projection: f64, height: f64) -> ProfileDef {
    ProfileDef {
        name: name.to_string(),
        kind,
        parts: vec![ProfilePart {
            section: vec![
                Point::new(0.0, 0.0),
                Point::new(projection, 0.0),
                Point::new(projection, height),
                Point::new(0.0, height),
            ],
            material: String::new(),
        }],
    }
}

/// The profiles every plan has: the room molding library, the square
/// profile and a few casing, wall cap, rail and strip profiles.
pub fn builtin_profiles() -> Vec<ProfileDef> {
    let mut v: Vec<ProfileDef> = MOLDING_LIBRARY.iter().map(ProfileDef::from_def).collect();
    v.push(square_profile());
    v.push(rect_profile(
        "Casing - Flat 3 1/2",
        MoldingType::Casing,
        0.75,
        3.5,
    ));
    v.push(rect_profile(
        "Wall Cap - Flat 1 1/2",
        MoldingType::WallCap,
        4.0,
        1.5,
    ));
    v.push(rect_profile("Rail - Flat 2", MoldingType::Rail, 2.0, 1.5));
    v.push(rect_profile(
        "Strip - Rope Light 1/2",
        MoldingType::StripProfile,
        0.5,
        0.5,
    ));
    v
}

/// The profile with this name (case-insensitive): the plan's own profiles
/// first, then the built-in ones.
pub fn find_profile(name: &str, own: &[ProfileDef]) -> Option<ProfileDef> {
    let n = name.trim();
    own.iter()
        .find(|p| p.name.eq_ignore_ascii_case(n))
        .cloned()
        .or_else(|| {
            builtin_profiles()
                .into_iter()
                .find(|p| p.name.eq_ignore_ascii_case(n))
        })
}

// ===================================================================
// The Moldings table
// ===================================================================

/// One row of the Moldings table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MoldingEntry {
    pub profile: ProfileDef,
    /// Width of the profile as used (how far it projects), inches.
    pub width: f64,
    /// Height of the profile as used, inches.
    pub height: f64,
    /// 3D molding: distance between the repeated symbols, inches (0 is a
    /// continuous molding).
    pub repeat_distance: f64,
    /// 3D molding: length of one repeated symbol along the path, inches
    /// (0 is half the repeat distance).
    pub element_length: f64,
    /// Horizontal Offset (away from the wall; negative recesses), inches.
    pub h_offset: f64,
    /// Vertical Offset, inches.
    pub v_offset: f64,
    pub retain_aspect: bool,
    /// A stacked row sits on top of the row before it.
    pub auto_offset: bool,
    /// Rails: runs the whole width of the wall.
    pub full_wall_width: bool,
    /// Wall caps: splits the profile on a pony wall.
    pub split_profile: bool,
    pub edge: EdgeMode,
    /// Bottom of the profile above the floor; `None` follows the type.
    pub vertical_position: Option<f64>,
    pub kind: MoldingType,
    /// Profile Rotation, degrees.
    pub rotation: f64,
    pub reflect_h: bool,
    pub reflect_v: bool,
    pub texture_up: bool,
    pub count_components: bool,
    /// Rows with the same non-zero number are one stack.
    pub stack: u32,
}

impl Default for MoldingEntry {
    fn default() -> Self {
        MoldingEntry::new(square_profile())
    }
}

/// Rotates `p` by `deg` degrees about `c`.
fn rotate_about(p: Point, c: Point, deg: f64) -> Point {
    let (s, k) = deg.to_radians().sin_cos();
    let d = p - c;
    Point::new(c.x + d.x * k - d.y * s, c.y + d.x * s + d.y * k)
}

impl MoldingEntry {
    pub fn new(profile: ProfileDef) -> MoldingEntry {
        MoldingEntry {
            width: profile.width().max(MIN_EXTENT),
            height: profile.height().max(MIN_EXTENT),
            kind: profile.kind,
            profile,
            repeat_distance: 0.0,
            element_length: 0.0,
            h_offset: 0.0,
            v_offset: 0.0,
            retain_aspect: true,
            auto_offset: true,
            full_wall_width: false,
            split_profile: false,
            edge: EdgeMode::Automatic,
            vertical_position: None,
            rotation: 0.0,
            reflect_h: false,
            reflect_v: false,
            texture_up: false,
            count_components: false,
            stack: 0,
        }
    }

    /// Sets the Width; the Height follows when Retain Aspect Ratio is on.
    pub fn set_width(&mut self, w: f64) {
        let w = w.max(MIN_EXTENT);
        if self.retain_aspect && self.width > MIN_EXTENT {
            self.height = (self.height * w / self.width).max(MIN_EXTENT);
        }
        self.width = w;
    }

    /// Sets the Height; the Width follows when Retain Aspect Ratio is on.
    pub fn set_height(&mut self, h: f64) {
        let h = h.max(MIN_EXTENT);
        if self.retain_aspect && self.height > MIN_EXTENT {
            self.width = (self.width * h / self.height).max(MIN_EXTENT);
        }
        self.height = h;
    }

    /// Back to the profile's own size and type (the Default button).
    pub fn reset_to_profile(&mut self) {
        self.width = self.profile.width().max(MIN_EXTENT);
        self.height = self.profile.height().max(MIN_EXTENT);
        self.kind = self.profile.kind;
        self.h_offset = 0.0;
        self.v_offset = 0.0;
        self.rotation = 0.0;
        self.reflect_h = false;
        self.reflect_v = false;
        self.vertical_position = None;
    }

    /// The parts scaled to Width x Height, reflected and rotated, with the
    /// back-bottom corner of their box at the origin (offsets not applied),
    /// and the size of that box.
    fn shaped(&self) -> (Vec<ProfilePart>, f64, f64) {
        let (lo, hi) = self.profile.bounds();
        let (pw, ph) = (hi.x - lo.x, hi.y - lo.y);
        let kx = if pw > MIN_EXTENT {
            self.width / pw
        } else {
            1.0
        };
        let ky = if ph > MIN_EXTENT {
            self.height / ph
        } else {
            1.0
        };
        let (w, h) = (pw * kx, ph * ky);
        let centre = Point::new(w * 0.5, h * 0.5);
        let mut parts: Vec<ProfilePart> = self
            .profile
            .parts
            .iter()
            .map(|part| {
                let mut section: Vec<Point> = part
                    .section
                    .iter()
                    .map(|p| {
                        let mut q = Point::new((p.x - lo.x) * kx, (p.y - lo.y) * ky);
                        if self.reflect_h {
                            q.x = w - q.x;
                        }
                        if self.reflect_v {
                            q.y = h - q.y;
                        }
                        if self.rotation.abs() > 1e-9 {
                            q = rotate_about(q, centre, self.rotation);
                        }
                        q
                    })
                    .collect();
                if polygon_area(&section) < 0.0 {
                    section.reverse();
                }
                ProfilePart {
                    section,
                    material: part.material.clone(),
                }
            })
            .collect();
        let (nlo, nhi) = bounds_of(parts.iter().flat_map(|p| p.section.iter()))
            .unwrap_or((Point::ZERO, Point::ZERO));
        for part in &mut parts {
            for p in &mut part.section {
                *p = *p - nlo;
            }
        }
        (parts, nhi.x - nlo.x, nhi.y - nlo.y)
    }

    /// The final sections of the row: scaled, reflected, rotated and moved by
    /// the offsets (`x` away from the wall, `y` up from the row's bottom).
    pub fn shaped_parts(&self) -> Vec<ProfilePart> {
        let (mut parts, _, _) = self.shaped();
        let d = Point::new(self.h_offset, self.v_offset);
        for part in &mut parts {
            for p in &mut part.section {
                *p = *p + d;
            }
        }
        parts
    }

    /// Height of the shaped profile (what a stack adds), inches.
    pub fn shaped_height(&self) -> f64 {
        self.shaped().2
    }

    /// Projection of the shaped profile with its horizontal offset, inches.
    pub fn shaped_width(&self) -> f64 {
        self.shaped().1
    }
}

/// A row placed in space: final sections, where its bottom is and what it
/// is.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedEntry {
    /// Index of the row in the table.
    pub index: usize,
    pub name: String,
    pub kind: MoldingType,
    pub parts: Vec<ProfilePart>,
    /// Elevation of the profile's bottom above the floor, inches.
    pub bottom: f64,
    /// Elevation of its top (with the vertical offset), inches.
    pub top: f64,
    pub repeat_distance: f64,
    pub element_length: f64,
    pub edge: EdgeMode,
    pub full_wall_width: bool,
    pub split_profile: bool,
}

impl ResolvedEntry {
    /// Length of a repeated element along the path, inches.
    pub fn element(&self) -> f64 {
        if self.element_length > 0.0 {
            self.element_length
                .min(self.repeat_distance.max(self.element_length))
        } else {
            self.repeat_distance * 0.5
        }
    }
}

/// The Moldings table of an object.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MoldingTable {
    pub source: TableSource,
    pub rows: Vec<MoldingEntry>,
}

impl MoldingTable {
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// A table of one row.
    pub fn single(profile: ProfileDef) -> MoldingTable {
        MoldingTable {
            source: TableSource::Own,
            rows: vec![MoldingEntry::new(profile)],
        }
    }

    /// Add New: appends a row for `profile` (Select Library Object); returns
    /// its index.
    pub fn add_new(&mut self, profile: ProfileDef) -> usize {
        self.rows.push(MoldingEntry::new(profile));
        self.rows.len() - 1
    }

    /// Make Copy: a copy of row `i` right after it; returns its index.
    pub fn make_copy(&mut self, i: usize) -> Option<usize> {
        let mut copy = self.rows.get(i)?.clone();
        copy.stack = 0;
        self.rows.insert(i + 1, copy);
        Some(i + 1)
    }

    /// Delete: removes row `i`.
    pub fn delete(&mut self, i: usize) -> bool {
        if i < self.rows.len() {
            self.rows.remove(i);
            true
        } else {
            false
        }
    }

    pub fn move_up(&mut self, i: usize) -> bool {
        if i == 0 || i >= self.rows.len() {
            return false;
        }
        self.rows.swap(i, i - 1);
        true
    }

    pub fn move_down(&mut self, i: usize) -> bool {
        if i + 1 >= self.rows.len() {
            return false;
        }
        self.rows.swap(i, i + 1);
        true
    }

    /// Replace: row `i` takes another profile and its size, keeping the
    /// offsets and switches.
    pub fn replace(&mut self, i: usize, profile: ProfileDef) -> bool {
        let Some(row) = self.rows.get_mut(i) else {
            return false;
        };
        row.width = profile.width().max(MIN_EXTENT);
        row.height = profile.height().max(MIN_EXTENT);
        row.kind = profile.kind;
        row.profile = profile;
        true
    }

    /// Default: row `i` back to its profile's size and type.
    pub fn set_default(&mut self, i: usize) -> bool {
        match self.rows.get_mut(i) {
            Some(r) => {
                r.reset_to_profile();
                true
            }
            None => false,
        }
    }

    /// Make Stack: the rows at `indices` become one stack, kept in table
    /// order and gathered at the first of them. Returns the stack number.
    pub fn make_stack(&mut self, indices: &[usize]) -> Option<u32> {
        let mut picked: Vec<usize> = indices
            .iter()
            .copied()
            .filter(|i| *i < self.rows.len())
            .collect();
        picked.sort_unstable();
        picked.dedup();
        if picked.len() < 2 {
            return None;
        }
        let number = self.rows.iter().map(|r| r.stack).max().unwrap_or(0) + 1;
        let at = picked[0];
        let mut moved: Vec<MoldingEntry> = Vec::new();
        for i in picked.iter().rev() {
            moved.push(self.rows.remove(*i));
        }
        moved.reverse();
        for r in &mut moved {
            r.stack = number;
        }
        for (k, r) in moved.into_iter().enumerate() {
            self.rows.insert(at + k, r);
        }
        Some(number)
    }

    /// Explode Stack: the stack row `i` belongs to becomes single rows.
    pub fn explode_stack(&mut self, i: usize) -> bool {
        let Some(n) = self.rows.get(i).map(|r| r.stack).filter(|s| *s != 0) else {
            return false;
        };
        for r in self.rows.iter_mut().filter(|r| r.stack == n) {
            r.stack = 0;
        }
        true
    }

    /// The rows with the same stack number as row `i` (just `i` when it is
    /// on its own).
    pub fn stack_of(&self, i: usize) -> Vec<usize> {
        match self.rows.get(i).map(|r| r.stack) {
            Some(n) if n != 0 => (0..self.rows.len())
                .filter(|k| self.rows[*k].stack == n)
                .collect(),
            _ => vec![i],
        }
    }

    /// The groups of consecutive rows that are stacked (a row on its own is a
    /// group of one).
    fn groups(&self) -> Vec<std::ops::Range<usize>> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.rows.len() {
            let mut j = i + 1;
            if self.rows[i].stack != 0 {
                while j < self.rows.len() && self.rows[j].stack == self.rows[i].stack {
                    j += 1;
                }
            }
            out.push(i..j);
            i = j;
        }
        out
    }

    fn resolve_with(&self, base_of: impl Fn(&MoldingEntry, f64) -> f64) -> Vec<ResolvedEntry> {
        let mut out = Vec::new();
        for g in self.groups() {
            let heights: Vec<f64> = self.rows[g.clone()]
                .iter()
                .map(MoldingEntry::shaped_height)
                .collect();
            let total: f64 = heights.iter().sum();
            let first = &self.rows[g.start];
            let base = first
                .vertical_position
                .unwrap_or_else(|| base_of(first, total));
            let mut cursor = base;
            for (k, idx) in g.clone().enumerate() {
                let row = &self.rows[idx];
                let bottom = if k == 0 {
                    base
                } else if row.auto_offset {
                    cursor
                } else {
                    row.vertical_position.unwrap_or(base)
                };
                cursor = bottom + heights[k];
                out.push(ResolvedEntry {
                    index: idx,
                    name: row.profile.name.clone(),
                    kind: row.kind,
                    parts: row.shaped_parts(),
                    bottom,
                    top: bottom + row.v_offset + heights[k],
                    repeat_distance: row.repeat_distance,
                    element_length: row.element_length,
                    edge: row.edge,
                    full_wall_width: row.full_wall_width,
                    split_profile: row.split_profile,
                });
            }
        }
        out
    }

    /// The rows placed on a wall of height `ceiling`: each stack where its
    /// type belongs (base on the floor, chair rail 32 inches up, crown
    /// hanging from the ceiling).
    pub fn resolve(&self, ceiling: f64) -> Vec<ResolvedEntry> {
        self.resolve_with(|row, total| row.kind.default_bottom(ceiling, total))
    }

    /// The rows placed from `base` up (a molding polyline: every stack
    /// starts at the molding's own height).
    pub fn resolve_from(&self, base: f64) -> Vec<ResolvedEntry> {
        self.resolve_with(|_, _| base)
    }

    /// Does the table put a molding on an edge in `mode`?
    pub fn has_rows(&self) -> bool {
        !self.rows.is_empty()
    }
}

// ===================================================================
// Paths
// ===================================================================

/// Length of a polyline, inches.
pub fn path_length(pts: &[Point]) -> f64 {
    pts.windows(2).map(|s| s[0].dist(s[1])).sum()
}

/// The part of the polyline between arc lengths `a` and `b`, with the
/// vertices that lie between them.
pub fn sub_path(pts: &[Point], a: f64, b: f64) -> Vec<Point> {
    let (a, b) = (a.min(b).max(0.0), a.max(b));
    let mut out: Vec<Point> = Vec::new();
    let mut at = 0.0;
    for s in pts.windows(2) {
        let len = s[0].dist(s[1]);
        let (s0, s1) = (at, at + len);
        at = s1;
        if len < 1e-9 || s1 <= a || s0 >= b {
            continue;
        }
        let ta = ((a - s0) / len).clamp(0.0, 1.0);
        let tb = ((b - s0) / len).clamp(0.0, 1.0);
        let (pa, pb) = (Point::lerp(s[0], s[1], ta), Point::lerp(s[0], s[1], tb));
        if out.last().is_none_or(|q| q.dist(pa) > 1e-9) {
            out.push(pa);
        }
        if out.last().is_none_or(|q| q.dist(pb) > 1e-9) {
            out.push(pb);
        }
    }
    out
}

/// The centres of repeated elements along a path of `length`: one every
/// `repeat`, the first half a repeat in, so the run starts and ends with a
/// gap of half a repeat. Empty when `repeat` is not positive.
pub fn repeat_centers(length: f64, repeat: f64) -> Vec<f64> {
    if repeat <= 1e-9 || length <= 1e-9 {
        return Vec::new();
    }
    let n = ((length / repeat) + 1e-9).floor() as usize;
    if n == 0 {
        return Vec::new();
    }
    let margin = (length - repeat * (n as f64 - 1.0)) * 0.5;
    (0..n).map(|k| margin + repeat * k as f64).collect()
}

/// Points along the arc of `radius` about `centre` from `start` degrees to
/// `end` degrees (counter-clockwise when `end > start`), `segments` pieces.
pub fn arc_points(centre: Point, radius: f64, start: f64, end: f64, segments: usize) -> Vec<Point> {
    let n = segments.max(1);
    (0..=n)
        .map(|k| {
            let a = (start + (end - start) * k as f64 / n as f64).to_radians();
            Point::new(centre.x + radius * a.cos(), centre.y + radius * a.sin())
        })
        .collect()
}

// ===================================================================
// Room moldings
// ===================================================================

/// The moldings of one room, keyed by the anchor of its [`crate::RoomName`].
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RoomMoldings {
    pub anchor: Point,
    pub table: MoldingTable,
    /// Edges of the room's interior outline (by index) without molding.
    pub off_edges: Vec<usize>,
}

/// Something a molding stops at: a footprint in plan and the heights it
/// occupies.
#[derive(Debug, Clone, PartialEq)]
pub struct Obstacle {
    pub footprint: Vec<Point>,
    pub bottom: f64,
    pub top: f64,
}

fn json_f64(v: &serde_json::Value, key: &str) -> Option<f64> {
    v.get(key).and_then(serde_json::Value::as_f64)
}

/// The cabinets of `floor` as obstacles (read from their stored form, so
/// plan-core needs no cabinet types): the outline box of each, between its
/// bottom and top. Fillers, soffits, shelves, partitions and countertop and
/// backsplash pieces are not obstacles.
pub fn cabinet_obstacles(floor: &Floor) -> Vec<Obstacle> {
    let mut out = Vec::new();
    for c in &floor.cabinets {
        // Cut Room Moldings off: room moldings run behind this cabinet.
        if c.get("cut_room_moldings")
            .and_then(serde_json::Value::as_bool)
            == Some(false)
        {
            continue;
        }
        let kind = c.get("kind").and_then(|k| k.as_str()).unwrap_or("Base");
        if matches!(
            kind,
            "Soffit"
                | "SoffitPolygon"
                | "Shelf"
                | "Partition"
                | "CustomCountertop"
                | "CustomBacksplash"
                | "CounterHole"
        ) {
            continue;
        }
        let (Some(w), Some(d)) = (json_f64(c, "width"), json_f64(c, "depth")) else {
            continue;
        };
        let pos = c.get("position");
        let px = pos.and_then(|p| json_f64(p, "x")).unwrap_or(0.0);
        let py = pos.and_then(|p| json_f64(p, "y")).unwrap_or(0.0);
        let angle = json_f64(c, "angle").unwrap_or(0.0);
        let (s, k) = angle.sin_cos();
        let origin = Point::new(px, py);
        let to_plan = |p: Point| origin + Point::new(p.x * k - p.y * s, p.x * s + p.y * k);
        let footprint = [
            Point::new(0.0, 0.0),
            Point::new(w, 0.0),
            Point::new(w, d),
            Point::new(0.0, d),
        ]
        .map(to_plan)
        .to_vec();
        let bottom = json_f64(c, "elevation").unwrap_or(0.0);
        let top = bottom + json_f64(c, "height").unwrap_or(34.5);
        out.push(Obstacle {
            footprint,
            bottom,
            top,
        });
    }
    out
}

/// Does `ob` stand against the edge `p`-`q`: close enough to the wall and
/// overlapping it along the edge, and up in the range `lo..hi`?
fn obstacle_span(ob: &Obstacle, p: Point, q: Point, lo: f64, hi: f64) -> Option<(f64, f64)> {
    if ob.top <= lo || ob.bottom >= hi || ob.footprint.len() < 3 {
        return None;
    }
    let len = p.dist(q);
    if len < 1e-6 {
        return None;
    }
    let dir = q.sub(p).normalized();
    let left = dir.perp();
    let mut s_lo = f64::MAX;
    let mut s_hi = f64::MIN;
    let mut near = f64::MAX;
    for v in &ob.footprint {
        let rel = *v - p;
        s_lo = s_lo.min(rel.dot(dir));
        s_hi = s_hi.max(rel.dot(dir));
        near = near.min(rel.dot(left));
    }
    // The room is on the left of the edge: the cabinet's nearest corner must
    // be within a couple of inches of the wall surface.
    if near > 2.0 || s_hi <= 0.0 || s_lo >= len {
        return None;
    }
    // And it must reach into the room at least a little.
    let reach = ob
        .footprint
        .iter()
        .map(|v| (*v - p).dot(left))
        .fold(f64::MIN, f64::max);
    (reach > 1.0).then_some((s_lo.max(0.0), s_hi.min(len)))
}

/// The pieces of the edge `p`-`q` (as distances along it) a molding between
/// `lo` and `hi` above the floor datum runs on: not crossed by an opening,
/// not behind an obstacle.
fn edge_runs(
    floor: &Floor,
    p: Point,
    q: Point,
    (lo, hi): (f64, f64),
    obstacles: &[Obstacle],
) -> Vec<(f64, f64)> {
    let len = p.dist(q);
    let mut blocks: Vec<(f64, f64)> = edge_openings(floor, p, q)
        .into_iter()
        .filter(|o| o.sill < hi && o.head > lo)
        .map(|o| (o.from, o.to))
        .collect();
    blocks.extend(
        obstacles
            .iter()
            .filter_map(|ob| obstacle_span(ob, p, q, lo, hi)),
    );
    blocks.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = Vec::new();
    let mut cursor = 0.0;
    for (a, b) in blocks {
        if a > cursor {
            out.push((cursor, a.min(len)));
        }
        cursor = cursor.max(b);
    }
    if cursor < len {
        out.push((cursor, len));
    }
    out.retain(|(a, b)| b - a > 0.5);
    out
}

/// Is a wall behind the edge not drawn (so no molding stands on it)?
fn wall_without_molding(floor: &Floor, p: Point, q: Point, suppressed: &[Id]) -> bool {
    edge_wall(floor, p, q).is_some_and(|w| {
        w.flags.invisible
            || w.flags.room_divider
            || w.flags.railing
            || (w.kind == WallKind::Interior && w.height <= 0.0)
            || suppressed.contains(&w.id)
    })
}

/// The paths a molding between `lo` and `hi` runs on around `room`: pieces
/// of the interior outline joined where they meet at corners. `off` are
/// edges without molding, `suppressed` walls that stop molding.
pub fn room_runs(
    floor: &Floor,
    room: &Room,
    (lo, hi): (f64, f64),
    obstacles: &[Obstacle],
    off: &[usize],
    suppressed: &[Id],
) -> Vec<Vec<Point>> {
    let poly = if room.inner_polygon.len() >= 3 {
        &room.inner_polygon
    } else {
        &room.polygon
    };
    let n = poly.len();
    if n < 3 {
        return Vec::new();
    }
    // (from, to, starts at a corner, ends at a corner)
    let mut pieces: Vec<(Point, Point, bool, bool)> = Vec::new();
    let mut gap_before_first = false;
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let len = p.dist(q);
        if len < 1e-6 {
            continue;
        }
        if off.contains(&i) || wall_without_molding(floor, p, q, suppressed) {
            if i == 0 {
                gap_before_first = true;
            }
            // A skipped edge breaks the chain: a sentinel piece of nothing.
            pieces.push((p, p, false, false));
            continue;
        }
        let dir = q.sub(p).normalized();
        for (a, b) in edge_runs(floor, p, q, (lo, hi), obstacles) {
            pieces.push((p + dir * a, p + dir * b, a < 1e-6, len - b < 1e-6));
        }
    }
    let _ = gap_before_first;
    let mut runs: Vec<Vec<Point>> = Vec::new();
    let mut prev_open_end = false;
    for &(pa, pb, at_start, at_end) in &pieces {
        if pa.dist(pb) < 1e-9 {
            prev_open_end = false;
            continue;
        }
        if at_start && prev_open_end && !runs.is_empty() {
            if let Some(last) = runs.last_mut() {
                last.push(pb);
            }
        } else {
            runs.push(vec![pa, pb]);
        }
        prev_open_end = at_end;
    }
    let first_ok = pieces.first().is_some_and(|f| f.2 && f.0.dist(f.1) > 1e-9);
    let last_ok = pieces.last().is_some_and(|l| l.3 && l.0.dist(l.1) > 1e-9);
    let wraps = first_ok && last_ok;
    if wraps && runs.len() > 1 {
        let first = runs.remove(0);
        if let Some(last) = runs.last_mut() {
            last.extend(first.into_iter().skip(1));
        }
    } else if wraps && runs.len() == 1 {
        let r = &mut runs[0];
        if r[0].dist(r[r.len() - 1]) > 1e-6 {
            let p0 = r[0];
            r.push(p0);
        }
    }
    runs
}

/// Room types that get no default moldings: stairwells and Open Below rooms,
/// porches and exterior rooms, garages and other hybrid rooms (manual p. 462).
/// A room of one of them has moldings only from a table of its own or of its
/// type.
pub const NO_DEFAULT_MOLDING_TYPES: [&str; 8] = [
    "Stairwell",
    "Open Below",
    "Porch",
    "Deck",
    "Garage",
    "Courtyard",
    "Flat Roof",
    "Attic",
];

static NO_TABLE: MoldingTable = MoldingTable {
    source: TableSource::Own,
    rows: Vec::new(),
};

/// The table a room of `room_type` gets when it has none of its own: the one
/// its room type's defaults set, else the floor's default table (not for
/// the types of [`NO_DEFAULT_MOLDING_TYPES`]).
pub fn default_table<'a>(layer: &'a DetailsLayer, room_type: &str) -> &'a MoldingTable {
    if let Some(t) = layer
        .type_moldings
        .iter()
        .find(|t| t.room_type.eq_ignore_ascii_case(room_type.trim()))
        .filter(|t| t.table.source != TableSource::UseFloorDefault)
    {
        return &t.table;
    }
    if NO_DEFAULT_MOLDING_TYPES
        .iter()
        .any(|n| n.eq_ignore_ascii_case(room_type.trim()))
    {
        return &NO_TABLE;
    }
    &layer.floor_moldings
}

/// The effective table of the room whose anchor is `anchor`: its own rows,
/// or the default of its room type and floor when it says Use Floor Default
/// (or has no record).
pub fn effective_table<'a>(
    layer: &'a DetailsLayer,
    anchor: Point,
    room_type: &str,
) -> &'a MoldingTable {
    match layer.room_moldings_at(anchor) {
        Some(r) if r.table.source != TableSource::UseFloorDefault => &r.table,
        _ => default_table(layer, room_type),
    }
}

/// The molding defaults of one room type (Room Type Defaults, Moldings).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TypeMoldings {
    pub room_type: String,
    pub table: MoldingTable,
}

/// The molding lines of `room` from `table` (the effective table): one line
/// per run per row, ready to sweep. `ceiling` is the finished ceiling above
/// the finished floor; `datum` is added to every elevation (the finished
/// floor above the floor datum). Ids are 0: the caller assigns them.
#[allow(clippy::too_many_arguments)]
pub fn room_molding_lines(
    floor: &Floor,
    room: &Room,
    table: &MoldingTable,
    off_edges: &[usize],
    suppressed: &[Id],
    ceiling: f64,
    datum: f64,
    anchor: Point,
) -> Vec<MoldingLine> {
    let obstacles = cabinet_obstacles(floor);
    let mut out = Vec::new();
    for r in table.resolve(ceiling) {
        if r.edge == EdgeMode::Off {
            continue;
        }
        let row = &table.rows[r.index];
        let runs = room_runs(
            floor,
            room,
            (datum + r.bottom, datum + r.top),
            &obstacles,
            off_edges,
            suppressed,
        );
        for run in runs {
            let mut entry = row.clone();
            // The line's own elevation is the bottom of the stack: pin the
            // row there (vertical positions are absolute, so `dz` stays 0).
            entry.vertical_position = Some(datum + r.bottom);
            entry.stack = 0;
            out.push(MoldingLine {
                polyline: run,
                profile: MoldingProfile::Custom(row.profile.section().to_vec()),
                height: row.shaped_height(),
                width: row.shaped_width(),
                elevation: datum + r.bottom,
                table: MoldingTable {
                    source: TableSource::Own,
                    rows: vec![entry],
                },
                side: crate::details::MoldingSide::Left,
                automatic: true,
                source: MoldingSource::Room {
                    anchor,
                    kind: r.kind,
                },
                layer: MOLDING_LAYER.to_string(),
                ..MoldingLine::default()
            });
        }
    }
    out
}

/// The lines of the older Moldings tab of a room (`RoomName.moldings`: a
/// base, a chair rail and a crown by library name): what the Make Room
/// Molding Polyline edit button converts.
pub fn legacy_room_lines(
    floor: &Floor,
    room: &Room,
    refs: &[MoldingRef],
    ceiling: f64,
    datum: f64,
    anchor: Point,
) -> Vec<MoldingLine> {
    let mut table = MoldingTable::default();
    for m in refs {
        if m.profile.trim().is_empty() {
            continue;
        }
        let Some(def) = molding_def(&m.profile).or_else(|| molding_defs(m.kind).first().copied())
        else {
            continue;
        };
        let mut entry = MoldingEntry::new(ProfileDef::from_def(def));
        if m.height > 0.0 {
            entry.set_height(m.height);
        }
        entry.kind = MoldingType::from_kind(m.kind);
        table.rows.push(entry);
    }
    room_molding_lines(floor, room, &table, &[], &[], ceiling, datum, anchor)
}

/// The room whose label anchor is `anchor` (or that contains it).
pub fn room_at(rooms: &[Room], anchor: Point) -> Option<&Room> {
    rooms.iter().find(|r| {
        let poly = if r.inner_polygon.len() >= 3 {
            &r.inner_polygon
        } else {
            &r.polygon
        };
        point_in_polygon(anchor, poly)
    })
}

// ===================================================================
// Reporting
// ===================================================================

/// One line of the trim take-off.
#[derive(Debug, Clone, PartialEq)]
pub struct TrimLine {
    /// [`INTERIOR_TRIM`] or [`EXTERIOR_TRIM`].
    pub category: &'static str,
    pub item: String,
    pub material: String,
    /// Linear length, inches.
    pub length: f64,
    /// How many pieces (quoin blocks) when the item is counted.
    pub count: usize,
    /// The owners behind the line: (floor, key, length in inches).
    pub sources: Vec<(usize, String, f64)>,
}

struct TrimAdd<'a> {
    category: &'static str,
    item: &'a str,
    material: &'a str,
    length: f64,
    count: usize,
    owner: (usize, &'a str),
}

fn add_trim(out: &mut Vec<TrimLine>, a: TrimAdd) {
    let (floor, key) = a.owner;
    let source = |l: &mut TrimLine| {
        if let Some(s) = l.sources.iter_mut().find(|s| s.0 == floor && s.1 == key) {
            s.2 += a.length;
        } else {
            l.sources.push((floor, key.to_string(), a.length));
        }
    };
    if let Some(l) = out
        .iter_mut()
        .find(|l| l.category == a.category && l.item == a.item && l.material == a.material)
    {
        l.length += a.length;
        l.count += a.count;
        source(l);
    } else {
        let mut l = TrimLine {
            category: a.category,
            item: a.item.to_string(),
            material: a.material.to_string(),
            length: a.length,
            count: a.count,
            sources: Vec::new(),
        };
        source(&mut l);
        out.push(l);
    }
}

/// Linear lengths of the moldings, corner boards and quoins of the whole
/// plan, grouped by category, profile name and material: molding polylines
/// and the room moldings the Moldings tabs generate, under Interior Trim and
/// Exterior Trim (corner boards and quoins are exterior).
pub fn trim_takeoff(project: &Project) -> Vec<TrimLine> {
    let floors: Vec<usize> = (0..project.floors.len()).collect();
    trim_takeoff_for(project, &floors, |_, _, _| true)
}

/// [`trim_takeoff`] for some floors and some owners. `keep(floor, key,
/// centre)` is asked for every molding (`molding:<id>`), corner board
/// (`corner_board:<id>`), quoin set (`quoin:<id>`) and room (the key
/// [`room_trim_key`] makes, for its generated moldings); `centre` is a point
/// of the owner. Each line lists its owners in `sources`.
pub fn trim_takeoff_for(
    project: &Project,
    floors: &[usize],
    mut keep: impl FnMut(usize, &str, Point) -> bool,
) -> Vec<TrimLine> {
    let mut out: Vec<TrimLine> = Vec::new();
    for (fi, floor) in project.floors.iter().enumerate() {
        if !floors.contains(&fi) {
            continue;
        }
        let layer = DetailsLayer::load(floor);
        for m in layer.moldings.iter().filter(|m| m.edge_lengths_on() > 0.0) {
            let key = format!("molding:{}", m.id);
            let centre = m.polyline.first().copied().unwrap_or_default();
            if !keep(fi, &key, centre) {
                continue;
            }
            for (name, material, category, length) in m.takeoff_rows() {
                let owner = (fi, key.as_str());
                add_trim(
                    &mut out,
                    TrimAdd {
                        category,
                        item: &name,
                        material: &material,
                        length,
                        count: 0,
                        owner,
                    },
                );
            }
        }
        for b in &layer.corner_boards {
            let key = format!("corner_board:{}", b.id);
            if keep(fi, &key, b.wall_corner) {
                let a = TrimAdd {
                    category: EXTERIOR_TRIM,
                    item: "Corner Board",
                    material: &b.material,
                    length: b.height * 2.0,
                    count: 0,
                    owner: (fi, &key),
                };
                add_trim(&mut out, a);
            }
        }
        for q in &layer.quoins {
            let key = format!("quoin:{}", q.id);
            if keep(fi, &key, q.corner) {
                let a = TrimAdd {
                    category: EXTERIOR_TRIM,
                    item: "Quoin",
                    material: &q.material,
                    length: q.total_height,
                    count: q.block_count(),
                    owner: (fi, &key),
                };
                add_trim(&mut out, a);
            }
        }
        // Room moldings the tabs generate.
        let rooms = crate::rooms::detect_rooms(&floor.walls, 0.5);
        for room in &rooms {
            let key = room_trim_key(fi, room);
            if !keep(fi, &key, room.centroid) {
                continue;
            }
            let ceiling = room
                .name_entry(&floor.room_names)
                .and_then(|n| n.ceiling_height)
                .unwrap_or(floor.ceiling_height);
            for line in generated_room_lines(floor, &layer, room, ceiling, 0.0) {
                for (name, material, category, length) in line.takeoff_rows() {
                    let a = TrimAdd {
                        category,
                        item: &name,
                        material: &material,
                        length,
                        count: 0,
                        owner: (fi, &key),
                    };
                    add_trim(&mut out, a);
                }
            }
        }
    }
    out.sort_by(|a, b| (a.category, &a.item).cmp(&(b.category, &b.item)));
    out
}

/// The owner key of a room's generated moldings: the same text as the
/// Materials List key of the room (`room:<floor>:<x>,<y>` in whole inches).
pub fn room_trim_key(floor: usize, room: &Room) -> String {
    crate::props::PropKey::room(floor, room.centroid).0
}

/// The lines the new Moldings panel generates for `room`: its own table,
/// or the floor defaults, whichever is in force. Empty for a room that only
/// has the older Moldings tab (plan-3d draws that one itself) and for a room
/// with no table at all. `ceiling` is the finished ceiling above the
/// finished floor, `datum` the finished floor above the floor datum.
pub fn extended_room_lines(
    floor: &Floor,
    layer: &DetailsLayer,
    room: &Room,
    ceiling: f64,
    datum: f64,
) -> Vec<MoldingLine> {
    let named = room.name_entry(&floor.room_names);
    let anchor = named.map_or_else(|| room.interior_point(), |n| n.anchor);
    let record = layer.room_moldings_at(anchor);
    if record.is_none() && named.is_some_and(|n| !n.moldings.is_empty()) {
        return Vec::new();
    }
    let room_type = named.map_or("", |n| n.room_type.as_str());
    let table = effective_table(layer, anchor, room_type);
    if table.is_empty() || table.source == TableSource::NoChange {
        return Vec::new();
    }
    let off: &[usize] = record.map_or(&[], |r| r.off_edges.as_slice());
    room_molding_lines(
        floor,
        room,
        table,
        off,
        &layer.molding_free_walls,
        ceiling,
        datum,
        anchor,
    )
}

/// Every molding of `room` as lines: [`extended_room_lines`], or the lines of
/// the older Moldings tab for a room that has only that.
pub fn generated_room_lines(
    floor: &Floor,
    layer: &DetailsLayer,
    room: &Room,
    ceiling: f64,
    datum: f64,
) -> Vec<MoldingLine> {
    let lines = extended_room_lines(floor, layer, room, ceiling, datum);
    if !lines.is_empty() {
        return lines;
    }
    match room.name_entry(&floor.room_names) {
        Some(named) if layer.room_moldings_at(named.anchor).is_none() => legacy_room_lines(
            floor,
            room,
            &named.moldings,
            named.ceiling_height.unwrap_or(ceiling),
            datum,
            named.anchor,
        ),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::details::MoldingSide;
    use crate::model::Wall;

    fn pt(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn crown_polyline() -> Vec<Point> {
        // Drawn clockwise and with the first point repeated at the end.
        vec![
            pt(5.0, 8.0),
            pt(5.0, 12.0),
            pt(7.0, 12.0),
            pt(9.0, 10.0),
            pt(9.0, 8.0),
            pt(5.0, 8.0),
        ]
    }

    #[test]
    fn a_closed_polyline_becomes_a_profile_at_the_origin() {
        let p = ProfileDef::from_polyline("My Crown", MoldingType::Crown, &crown_polyline())
            .expect("a profile");
        assert_eq!(p.name, "My Crown");
        let (lo, hi) = p.bounds();
        assert_eq!((lo.x, lo.y), (0.0, 0.0));
        assert!((hi.x - 4.0).abs() < 1e-9 && (hi.y - 4.0).abs() < 1e-9);
        // Counter-clockwise and without the closing point.
        assert!(polygon_area(p.section()) > 0.0);
        assert_eq!(p.section().len(), 5);
        assert!(p.is_valid());
    }

    #[test]
    fn bad_polylines_are_refused() {
        let line = [pt(0.0, 0.0), pt(1.0, 0.0)];
        assert_eq!(
            ProfileDef::from_polyline("x", MoldingType::Base, &line).unwrap_err(),
            ProfileError::TooFewPoints
        );
        let flat = [pt(0.0, 0.0), pt(1.0, 0.0), pt(2.0, 0.0)];
        assert_eq!(
            ProfileDef::from_polyline("x", MoldingType::Base, &flat).unwrap_err(),
            ProfileError::NoArea
        );
        let bow = [pt(0.0, 0.0), pt(2.0, 2.0), pt(2.0, 0.0), pt(0.0, 2.0)];
        assert_eq!(
            ProfileDef::from_polyline("x", MoldingType::Base, &bow).unwrap_err(),
            ProfileError::SelfIntersecting
        );
    }

    #[test]
    fn a_stacked_profile_keeps_the_parts_relative_positions() {
        let lower = vec![
            pt(10.0, 20.0),
            pt(12.0, 20.0),
            pt(12.0, 21.0),
            pt(10.0, 21.0),
        ];
        let upper = vec![
            pt(10.0, 21.0),
            pt(11.0, 21.0),
            pt(11.0, 23.0),
            pt(10.0, 23.0),
        ];
        let p = ProfileDef::stacked(
            "Built-up",
            MoldingType::Crown,
            vec![(lower, "Oak".into()), (upper, "Painted White Trim".into())],
        )
        .unwrap();
        assert!(p.is_stack());
        assert_eq!(p.parts[0].material, "Oak");
        let (_, hi) = p.bounds();
        assert!((hi.x - 2.0).abs() < 1e-9 && (hi.y - 3.0).abs() < 1e-9);
        // The upper part sits on the lower one.
        assert!((p.parts[1].section[0].y - 1.0).abs() < 1e-9);
    }

    #[test]
    fn builtin_profiles_cover_the_room_library_and_are_valid() {
        let all = builtin_profiles();
        assert!(all.len() > MOLDING_LIBRARY.len());
        assert!(all.iter().all(ProfileDef::is_valid));
        assert!(find_profile(SQUARE_PROFILE, &[]).is_some());
        assert!(find_profile("crown - cove 3 5/8", &[]).is_some());
        // A plan's own profile wins over a built-in one of the same name.
        let mut own = square_profile();
        own.kind = MoldingType::Crown;
        assert_eq!(
            find_profile(SQUARE_PROFILE, &[own]).unwrap().kind,
            MoldingType::Crown
        );
    }

    #[test]
    fn retain_aspect_ratio_links_width_and_height() {
        let mut e = MoldingEntry::new(
            ProfileDef::from_polyline(
                "w",
                MoldingType::Base,
                &[pt(0.0, 0.0), pt(2.0, 0.0), pt(2.0, 4.0), pt(0.0, 4.0)],
            )
            .unwrap(),
        );
        assert_eq!((e.width, e.height), (2.0, 4.0));
        e.set_width(1.0);
        assert!((e.height - 2.0).abs() < 1e-9);
        e.retain_aspect = false;
        e.set_height(10.0);
        assert!((e.width - 1.0).abs() < 1e-9);
        let (parts, w, h) = e.shaped();
        assert!((w - 1.0).abs() < 1e-9 && (h - 10.0).abs() < 1e-9);
        assert_eq!(parts.len(), 1);
    }

    #[test]
    fn offsets_move_the_section_and_negative_recesses() {
        let e = MoldingEntry {
            h_offset: -0.25,
            v_offset: 2.0,
            ..Default::default()
        };
        let parts = e.shaped_parts();
        let (lo, _) = bounds_of(parts[0].section.iter()).unwrap();
        assert!((lo.x + 0.25).abs() < 1e-9 && (lo.y - 2.0).abs() < 1e-9);
    }

    #[test]
    fn reflection_and_rotation_keep_the_box_at_the_origin() {
        let tri = ProfileDef::from_polyline(
            "tri",
            MoldingType::Base,
            &[pt(0.0, 0.0), pt(3.0, 0.0), pt(0.0, 1.0)],
        )
        .unwrap();
        let mut e = MoldingEntry::new(tri);
        e.reflect_h = true;
        let (parts, w, h) = e.shaped();
        assert!((w - 3.0).abs() < 1e-9 && (h - 1.0).abs() < 1e-9);
        // The long leg is still on the bottom; the point is now at x = 3.
        assert!(parts[0]
            .section
            .iter()
            .any(|p| (p.x - 3.0).abs() < 1e-9 && p.y > 0.9));
        assert!(polygon_area(&parts[0].section) > 0.0);
        e.reflect_h = false;
        e.rotation = 90.0;
        let (_, w, h) = e.shaped();
        assert!((w - 1.0).abs() < 1e-9 && (h - 3.0).abs() < 1e-9);
    }

    #[test]
    fn table_buttons() {
        let mut t = MoldingTable::default();
        let a = t.add_new(square_profile());
        let b = t.add_new(builtin_profiles()[0].clone());
        let c = t.add_new(builtin_profiles()[3].clone());
        assert_eq!((a, b, c), (0, 1, 2));
        assert_eq!(t.make_copy(1), Some(2));
        assert_eq!(t.len(), 4);
        assert!(t.move_up(2) && t.move_down(0));
        assert!(!t.move_up(0) && !t.move_down(3));
        assert!(t.delete(3) && !t.delete(9));
        let n = t.make_stack(&[0, 2]).unwrap();
        // The two rows were gathered at the first one.
        assert_eq!(t.rows[0].stack, n);
        assert_eq!(t.rows[1].stack, n);
        assert_eq!(t.stack_of(1), vec![0, 1]);
        assert!(t.explode_stack(0));
        assert_eq!(t.stack_of(0), vec![0]);
        assert!(t.make_stack(&[0]).is_none());
        // Replace keeps the offsets, Default clears them.
        t.rows[0].h_offset = 0.5;
        assert!(t.replace(0, builtin_profiles()[4].clone()));
        assert_eq!(t.rows[0].h_offset, 0.5);
        assert!(t.set_default(0));
        assert_eq!(t.rows[0].h_offset, 0.0);
    }

    #[test]
    fn stacks_sit_on_each_other_and_crowns_hang_from_the_ceiling() {
        let mut t = MoldingTable::default();
        t.add_new(
            ProfileDef::from_polyline(
                "c1",
                MoldingType::Crown,
                &[pt(0.0, 0.0), pt(2.0, 0.0), pt(2.0, 2.0), pt(0.0, 2.0)],
            )
            .unwrap(),
        );
        t.add_new(
            ProfileDef::from_polyline(
                "c2",
                MoldingType::Crown,
                &[pt(0.0, 0.0), pt(1.0, 0.0), pt(1.0, 1.0), pt(0.0, 1.0)],
            )
            .unwrap(),
        );
        // Unstacked, each crown hangs from the ceiling.
        let r = t.resolve(96.0);
        assert!((r[0].bottom - 94.0).abs() < 1e-9 && (r[1].bottom - 95.0).abs() < 1e-9);
        // Stacked, the two hang from the ceiling together.
        t.make_stack(&[0, 1]);
        let r = t.resolve(96.0);
        assert!((r[0].bottom - 93.0).abs() < 1e-9, "{}", r[0].bottom);
        assert!((r[1].bottom - 95.0).abs() < 1e-9);
        assert!((r[1].top - 96.0).abs() < 1e-9);
        // A polyline starts its stacks at its own height.
        let r = t.resolve_from(40.0);
        assert!((r[0].bottom - 40.0).abs() < 1e-9 && (r[1].bottom - 42.0).abs() < 1e-9);
        // A base goes on the floor, a chair rail 32 inches up.
        let mut base = MoldingTable::single(builtin_profiles()[0].clone());
        base.rows[0].kind = MoldingType::ChairRail;
        assert_eq!(base.resolve(96.0)[0].bottom, CHAIR_RAIL_HEIGHT);
        base.rows[0].vertical_position = Some(10.0);
        assert_eq!(base.resolve(96.0)[0].bottom, 10.0);
    }

    #[test]
    fn repeat_placement_and_sub_paths() {
        let c = repeat_centers(100.0, 20.0);
        assert_eq!(c.len(), 5);
        assert!((c[0] - 10.0).abs() < 1e-9 && (c[4] - 90.0).abs() < 1e-9);
        assert!(repeat_centers(100.0, 0.0).is_empty());
        assert_eq!(repeat_centers(10.0, 20.0).len(), 0);
        // A run of 105 leaves 2.5 on each side of five centres.
        let c = repeat_centers(105.0, 20.0);
        assert_eq!(c.len(), 5);
        assert!((c[0] - 12.5).abs() < 1e-9);
        let path = [pt(0.0, 0.0), pt(10.0, 0.0), pt(10.0, 10.0)];
        assert!((path_length(&path) - 20.0).abs() < 1e-9);
        // An element across the corner keeps the corner vertex (the 45
        // degree miter).
        let around = sub_path(&path, 8.0, 12.0);
        assert_eq!(around.len(), 3);
        assert!(around[1].dist(pt(10.0, 0.0)) < 1e-9);
        let straight = sub_path(&path, 2.0, 4.0);
        assert_eq!(straight.len(), 2);
    }

    #[test]
    fn arcs_have_the_radius() {
        let a = arc_points(pt(0.0, 0.0), 24.0, 0.0, 90.0, 12);
        assert_eq!(a.len(), 13);
        assert!(a.iter().all(|p| (p.length() - 24.0).abs() < 1e-9));
        assert!((a[12].x).abs() < 1e-9 && (a[12].y - 24.0).abs() < 1e-9);
    }

    fn room_floor() -> (Floor, Vec<Room>) {
        let mut f = Floor::new("1", 0.0);
        let w = |id, a: Point, b: Point| Wall {
            id,
            ..Wall::new(a, b, 6.0, 96.0, WallKind::Exterior)
        };
        f.walls = vec![
            w(1, pt(0.0, 0.0), pt(120.0, 0.0)),
            w(2, pt(120.0, 0.0), pt(120.0, 96.0)),
            w(3, pt(120.0, 96.0), pt(0.0, 96.0)),
            w(4, pt(0.0, 96.0), pt(0.0, 0.0)),
        ];
        let rooms = crate::rooms::detect_rooms(&f.walls, 1.0);
        (f, rooms)
    }

    #[test]
    fn room_runs_stop_at_cabinets_and_skip_off_edges() {
        let (mut f, rooms) = room_floor();
        assert_eq!(rooms.len(), 1);
        let room = &rooms[0];
        let all = room_runs(&f, room, (0.0, 5.0), &[], &[], &[]);
        assert_eq!(all.len(), 1);
        assert!(all[0][0].dist(all[0][all[0].len() - 1]) < 1e-6, "closed");
        // An edge switched off opens the run.
        let no_edge = room_runs(&f, room, (0.0, 5.0), &[], &[1], &[]);
        assert_eq!(no_edge.len(), 1);
        assert!(no_edge[0].dist_ends() > 1.0);
        // A base cabinet along the bottom wall (the inner outline starts at
        // x = 3, y = 3) cuts the base molding out behind it.
        f.cabinets.push(serde_json::json!({
            "kind": "Base", "position": {"x": 40.0, "y": 3.0}, "angle": 0.0,
            "width": 30.0, "depth": 24.0, "height": 34.5, "elevation": 0.0
        }));
        let obstacles = cabinet_obstacles(&f);
        assert_eq!(obstacles.len(), 1);
        // Cut Room Moldings off lets the molding run behind the cabinet.
        let mut free = f.clone();
        free.cabinets[0]["cut_room_moldings"] = serde_json::json!(false);
        assert!(cabinet_obstacles(&free).is_empty());
        let base = room_runs(&f, room, (0.0, 5.0), &obstacles, &[], &[]);
        let len: f64 = base.iter().map(|r| path_length(r)).sum();
        let full: f64 = all.iter().map(|r| path_length(r)).sum();
        assert!((full - len - 30.0).abs() < 1.0, "{full} {len}");
        // Crown, up at the ceiling, passes over the base cabinet.
        let crown = room_runs(&f, room, (92.0, 96.0), &obstacles, &[], &[]);
        let crown_len: f64 = crown.iter().map(|r| path_length(r)).sum();
        assert!((crown_len - full).abs() < 1e-6);
        // A wall that is not drawn takes its molding with it.
        f.walls[0].flags.invisible = true;
        let gone = room_runs(&f, room, (0.0, 5.0), &[], &[], &[]);
        let gone_len: f64 = gone.iter().map(|r| path_length(r)).sum();
        assert!((full - gone_len - 114.0).abs() < 1.0, "{full} {gone_len}");
        // Suppressing a wall by id does the same.
        f.walls[0].flags.invisible = false;
        let supp = room_runs(&f, room, (0.0, 5.0), &[], &[], &[1]);
        let supp_len: f64 = supp.iter().map(|r| path_length(r)).sum();
        assert!((gone_len - supp_len).abs() < 1e-6);
    }

    trait Ends {
        fn dist_ends(&self) -> f64;
    }
    impl Ends for Vec<Point> {
        fn dist_ends(&self) -> f64 {
            self[0].dist(self[self.len() - 1])
        }
    }

    #[test]
    fn room_lines_follow_the_effective_table() {
        let (f, rooms) = room_floor();
        let room = &rooms[0];
        let mut table = MoldingTable::default();
        let crown = builtin_profiles()
            .into_iter()
            .find(|p| p.kind == MoldingType::Crown)
            .unwrap();
        table.add_new(builtin_profiles()[0].clone());
        table.add_new(crown);
        let lines = room_molding_lines(&f, room, &table, &[], &[], 96.0, 0.0, pt(60.0, 48.0));
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].elevation, 0.0);
        assert!(lines[1].elevation > 90.0);
        // The sweep stands at the line's elevation, not at the floor.
        for l in &lines {
            assert!(l.placed_parts().iter().all(|p| p.dz.abs() < 1e-9));
        }
        assert!(lines.iter().all(|l| l.automatic));
        assert_eq!(lines[0].side, MoldingSide::Left);
        // Off for one row removes its lines.
        table.rows[0].edge = EdgeMode::Off;
        let lines = room_molding_lines(&f, room, &table, &[], &[], 96.0, 0.0, pt(60.0, 48.0));
        assert_eq!(lines.len(), 1);
    }

    fn box_project() -> Project {
        let mut p = Project::new("t");
        let c = [
            pt(0.0, 0.0),
            pt(240.0, 0.0),
            pt(240.0, 180.0),
            pt(0.0, 180.0),
        ];
        for i in 0..4 {
            p.add_wall(0, c[i], c[(i + 1) % 4], 6.5, 96.0, WallKind::Exterior);
        }
        p.floors[0]
            .room_names
            .push(crate::model::RoomName::new(pt(120.0, 90.0), "Den", "Den"));
        p
    }

    #[test]
    fn the_takeoff_reports_lengths_under_interior_and_exterior_trim() {
        let mut p = box_project();
        let rooms = crate::rooms::detect_rooms(&p.floors[0].walls, 0.5);
        let inner = {
            let mut ring = rooms[0].inner_polygon.clone();
            let first = ring[0];
            ring.push(first);
            path_length(&ring)
        };
        let mut layer = DetailsLayer::default();
        // The room has a base and a crown from its table.
        let crown = builtin_profiles()
            .into_iter()
            .find(|x| x.kind == MoldingType::Crown)
            .unwrap();
        let mut table = MoldingTable::default();
        table.add_new(builtin_profiles()[0].clone());
        table.add_new(crown.clone());
        layer.room_moldings_mut(pt(120.0, 90.0)).table = table;
        // A hand-drawn molding polyline of 100 + 50 inches, one edge off.
        let mut line = MoldingLine::with_profile(
            50,
            vec![
                pt(10.0, 10.0),
                pt(110.0, 10.0),
                pt(110.0, 60.0),
                pt(10.0, 60.0),
            ],
            crown.clone(),
            30.0,
        );
        line.set_edge_on(2, false);
        layer.moldings.push(line);
        let floor = p.floors[0].clone();
        let mut next = 100;
        let mut alloc = || {
            next += 1;
            next
        };
        layer.auto_corner_boards(&floor, &rooms, &mut alloc);
        layer.auto_quoins(&floor, &rooms, &mut alloc);
        layer.store(&mut p.floors[0]);
        let lines = trim_takeoff(&p);
        let sum = |cat: &str, item: &str| -> f64 {
            lines
                .iter()
                .filter(|l| l.category == cat && l.item.starts_with(item))
                .map(|l| l.length)
                .sum()
        };
        // Base and crown run all the way round the room; the polyline adds
        // 100 + 50 inches of crown.
        assert!((sum(INTERIOR_TRIM, &builtin_profiles()[0].name) - inner).abs() < 1.0);
        assert!(
            (sum(INTERIOR_TRIM, &crown.name) - inner - 150.0).abs() < 1.0,
            "{lines:?}"
        );
        // Four corner boards of two 96" boards, four quoin stacks of 96".
        assert!((sum(EXTERIOR_TRIM, "Corner Board") - 4.0 * 2.0 * 96.0).abs() < 1e-6);
        assert!((sum(EXTERIOR_TRIM, "Quoin") - 4.0 * 96.0).abs() < 1e-6);
        assert!(lines.iter().any(|l| l.item == "Quoin" && l.count == 4 * 24));
        // The older Moldings tab is counted too (no record, a name list).
        let mut p2 = box_project();
        p2.floors[0].room_names[0].moldings.push(MoldingRef {
            kind: MoldingKind::Base,
            profile: "Base - Colonial 5 1/4".into(),
            height: 5.25,
        });
        let legacy = trim_takeoff(&p2);
        assert_eq!(legacy.len(), 1);
        assert_eq!(legacy[0].category, INTERIOR_TRIM);
        assert!((legacy[0].length - inner).abs() < 1.0);
    }

    #[test]
    fn use_floor_default_shares_one_table_across_rooms() {
        let mut p = box_project();
        let crown = builtin_profiles()
            .into_iter()
            .find(|x| x.kind == MoldingType::Crown)
            .unwrap();
        let mut layer = DetailsLayer::default();
        layer.floor_moldings.add_new(crown.clone());
        layer.store(&mut p.floors[0]);
        let floor = p.floors[0].clone();
        let rooms = crate::rooms::detect_rooms(&floor.walls, 0.5);
        let layer = DetailsLayer::load(&floor);
        // No record: the floor default applies.
        let lines = extended_room_lines(&floor, &layer, &rooms[0], 96.0, 0.0);
        assert_eq!(lines.len(), 1);
        // A record that says Use Floor Default is the same; an own table wins.
        let mut layer = layer;
        layer.room_moldings_mut(pt(120.0, 90.0)).table.source = TableSource::UseFloorDefault;
        assert_eq!(
            extended_room_lines(&floor, &layer, &rooms[0], 96.0, 0.0).len(),
            1
        );
        let mut own = MoldingTable::single(builtin_profiles()[0].clone());
        own.add_new(builtin_profiles()[1].clone());
        layer.room_moldings_mut(pt(120.0, 90.0)).table = own;
        assert_eq!(
            extended_room_lines(&floor, &layer, &rooms[0], 96.0, 0.0).len(),
            2
        );
        // An empty own table: the room has no molding at all.
        layer.room_moldings_mut(pt(120.0, 90.0)).table = MoldingTable::default();
        assert!(extended_room_lines(&floor, &layer, &rooms[0], 96.0, 0.0).is_empty());
        // A room type can set its own table; porches and garages get none
        // from the floor.
        let mut typed = DetailsLayer::default();
        typed.floor_moldings.add_new(crown.clone());
        typed.type_moldings.push(TypeMoldings {
            room_type: "Den".into(),
            table: MoldingTable::single(builtin_profiles()[0].clone()),
        });
        assert_eq!(
            default_table(&typed, "den").rows[0].profile.name,
            builtin_profiles()[0].name
        );
        assert_eq!(
            default_table(&typed, "Bedroom").rows[0].profile.name,
            crown.name
        );
        assert!(default_table(&typed, "Garage").is_empty());
        assert!(default_table(&typed, "porch").is_empty());
        typed.type_moldings.push(TypeMoldings {
            room_type: "Garage".into(),
            table: MoldingTable::single(crown.clone()),
        });
        assert_eq!(
            default_table(&typed, "Garage").len(),
            1,
            "its own type table applies"
        );
        // A room with the older tab and no record is left to plan-3d.
        let mut floor = floor;
        floor.room_names[0].moldings.push(MoldingRef {
            kind: MoldingKind::Crown,
            profile: "Crown - Cove 3 5/8".into(),
            height: 3.625,
        });
        let layer = DetailsLayer::default();
        assert!(extended_room_lines(&floor, &layer, &rooms[0], 96.0, 0.0).is_empty());
        assert!(!generated_room_lines(&floor, &layer, &rooms[0], 96.0, 0.0).is_empty());
    }
}
