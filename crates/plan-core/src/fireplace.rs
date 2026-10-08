//! Fireplaces and chimneys (CB-87): the specification stored per floor, the
//! plan geometry of the firebox, hearth, mantel and chimney, and the 3-2-10
//! rule that decides how high a chimney goes.
//!
//! A fireplace is a [`PlacedSymbol`] (so the Select tool moves, rotates,
//! sizes, copies and deletes it like any placed object) plus a
//! [`Fireplace`] record in [`Floor::fireplaces`] with the same id that holds
//! everything the Fireplace Specification edits. The symbol's `position` is
//! the middle of the fireplace's back, its `width` and `depth` are the size
//! of the body, and its front faces the room (see [`PlacedSymbol`]). A symbol
//! with no record behaves as a default fireplace, so a pasted copy still
//! works.
//!
//! Local axes of a fireplace: `x` across (to the right of the front), `y`
//! forward from the back, in inches. [`Frame`] maps them to the plan.

use crate::deck::clip_line;
use crate::geometry::{point_in_polygon, polygon_area, Point};
use crate::model::{Floor, Id, Project, Wall};
use crate::symbols::PlacedSymbol;
use serde::{Deserialize, Serialize};

/// Catalog id of a fireplace symbol (a firebox with its hearth and mantel).
pub const FIREPLACE_CATALOG_ID: &str = "plan-studio.fireplace";
/// Catalog id of a stand-alone chimney or chase (no firebox).
pub const CHIMNEY_CATALOG_ID: &str = "plan-studio.chimney";
/// Layer fireplaces are drawn on.
pub const FIREPLACE_LAYER: &str = "Fireplaces";
/// Elevation of the symbol of a fireplace relative to its floor, inches: below
/// the floor platform, so the stand-in block the viewer draws for any placed
/// symbol is hidden.
pub const STAND_IN_ELEVATION: f64 = -2.0;
/// Greatest distance at which another roof counts for the 2-ft rule: 10 ft.
pub const NEARBY_REACH: f64 = 120.0;

/// Is `sym` the symbol of a fireplace or chimney?
pub fn is_fireplace_symbol(sym: &PlacedSymbol) -> bool {
    sym.catalog_id == FIREPLACE_CATALOG_ID || sym.catalog_id == CHIMNEY_CATALOG_ID
}

/// The kind of fireplace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FireplaceKind {
    /// A site-built masonry firebox and chimney.
    #[default]
    Masonry,
    /// A factory-built firebox in a framed chase.
    Prefab,
    /// A chimney or chase on its own, no firebox.
    ChimneyOnly,
}

impl FireplaceKind {
    pub const ALL: [FireplaceKind; 3] = [
        FireplaceKind::Masonry,
        FireplaceKind::Prefab,
        FireplaceKind::ChimneyOnly,
    ];

    pub fn name(self) -> &'static str {
        match self {
            FireplaceKind::Masonry => "Masonry",
            FireplaceKind::Prefab => "Prefab",
            FireplaceKind::ChimneyOnly => "Chimney Only",
        }
    }

    pub fn has_firebox(self) -> bool {
        self != FireplaceKind::ChimneyOnly
    }

    /// The catalog id of the symbol that stands for this kind.
    pub fn catalog_id(self) -> &'static str {
        match self {
            FireplaceKind::ChimneyOnly => CHIMNEY_CATALOG_ID,
            _ => FIREPLACE_CATALOG_ID,
        }
    }
}

/// What the fireplace burns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Fuel {
    #[default]
    Wood,
    Gas,
    Electric,
}

impl Fuel {
    pub const ALL: [Fuel; 3] = [Fuel::Wood, Fuel::Gas, Fuel::Electric];

    pub fn name(self) -> &'static str {
        match self {
            Fuel::Wood => "Wood Burning",
            Fuel::Gas => "Gas",
            Fuel::Electric => "Electric",
        }
    }
}

/// The opening of the firebox.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Firebox {
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    /// Height of the bottom of the opening above the hearth top.
    pub raise: f64,
}

impl Default for Firebox {
    fn default() -> Self {
        Self {
            width: 36.0,
            height: 30.0,
            depth: 18.0,
            raise: 0.0,
        }
    }
}

/// The hearth in front of the firebox (IRC R1001.10: 16" in front and 8" at
/// each side of an opening under 6 sq ft).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hearth {
    pub enabled: bool,
    /// Distance the hearth reaches in front of the body.
    pub projection: f64,
    /// Distance it reaches past each side of the firebox opening.
    pub side_extension: f64,
    /// Height of the hearth top above the floor (raised hearth).
    pub height: f64,
    pub thickness: f64,
}

impl Default for Hearth {
    fn default() -> Self {
        Self {
            enabled: true,
            projection: 16.0,
            side_extension: 8.0,
            height: 0.0,
            thickness: 4.0,
        }
    }
}

/// The mantel: a shelf over the opening, optionally with legs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Mantel {
    pub enabled: bool,
    /// Height of the top of the shelf above the floor.
    pub height: f64,
    /// Width of the shelf; `0` follows the opening plus the overhangs.
    pub width: f64,
    pub depth: f64,
    pub thickness: f64,
    /// Width of the legs under each end; `0` is a floating shelf.
    pub leg_width: f64,
}

impl Default for Mantel {
    fn default() -> Self {
        Self {
            enabled: true,
            height: 54.0,
            width: 0.0,
            depth: 8.0,
            thickness: 3.0,
            leg_width: 0.0,
        }
    }
}

/// What tops the chimney.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CapKind {
    None,
    /// A concrete crown with the flue tile showing.
    #[default]
    Crown,
    /// A metal rain cap on legs.
    RainCap,
    /// A stone slab cap.
    Stone,
}

impl CapKind {
    pub const ALL: [CapKind; 4] = [
        CapKind::None,
        CapKind::Crown,
        CapKind::RainCap,
        CapKind::Stone,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CapKind::None => "None",
            CapKind::Crown => "Concrete Crown",
            CapKind::RainCap => "Metal Rain Cap",
            CapKind::Stone => "Stone Cap",
        }
    }
}

/// How the top of the chimney is found.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum ChimneyTop {
    /// By the 3-2-10 rule: 3 ft above the roof it passes through and 2 ft
    /// above anything within 10 ft.
    #[default]
    Auto,
    /// A fixed height above the floor the fireplace stands on, inches.
    Height(f64),
}

/// The chimney above the fireplace (Chimney tab).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Chimney {
    pub enabled: bool,
    /// Outside size of the shaft.
    pub width: f64,
    pub depth: f64,
    pub top: ChimneyTop,
    /// Rise above the roof the chimney passes through, inches (36).
    pub above_roof: f64,
    /// Rise above anything within 10 ft, inches (24).
    pub above_nearby: f64,
    pub cap: CapKind,
    /// How far the cap reaches past the shaft.
    pub cap_overhang: f64,
    pub cap_thickness: f64,
    /// Flue liner, showing above the cap.
    pub flue_width: f64,
    pub flue_depth: f64,
    pub flue_rise: f64,
    /// Flashing where the shaft meets the roof.
    pub flashing: bool,
    /// The chimney continues up through the floors above as a chase.
    pub chase_through_floors: bool,
}

impl Default for Chimney {
    fn default() -> Self {
        Self {
            enabled: true,
            width: 48.0,
            depth: 24.0,
            top: ChimneyTop::Auto,
            above_roof: 36.0,
            above_nearby: 24.0,
            cap: CapKind::Crown,
            cap_overhang: 2.0,
            cap_thickness: 4.0,
            flue_width: 12.0,
            flue_depth: 12.0,
            flue_rise: 8.0,
            flashing: true,
            chase_through_floors: true,
        }
    }
}

/// Materials tab: a surface material name per part ("" = the kind's own).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FireplaceMaterials {
    pub body: String,
    pub firebox: String,
    pub hearth: String,
    pub mantel: String,
    pub chimney: String,
    pub cap: String,
}

/// Label tab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FireplaceLabel {
    pub show: bool,
    /// Text shown in the plan; empty shows the fireplace's name.
    pub text: String,
}

impl Default for FireplaceLabel {
    fn default() -> Self {
        Self {
            show: true,
            text: String::new(),
        }
    }
}

/// The specification of one fireplace or chimney.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Fireplace {
    /// Id of the placed symbol this record belongs to.
    pub id: Id,
    pub kind: FireplaceKind,
    pub name: String,
    pub fuel: Fuel,
    /// The body replaces the stretch of wall it stands in.
    pub in_wall: bool,
    /// Height of the bottom of the body above the floor.
    pub elevation: f64,
    /// Height of the body; `None` reaches the ceiling.
    pub height: Option<f64>,
    pub firebox: Firebox,
    pub hearth: Hearth,
    pub mantel: Mantel,
    pub chimney: Chimney,
    pub materials: FireplaceMaterials,
    pub label: FireplaceLabel,
}

impl Default for Fireplace {
    fn default() -> Self {
        Fireplace::new(0, FireplaceKind::Masonry)
    }
}

impl Fireplace {
    /// A fireplace of `kind` with Chief-like defaults.
    pub fn new(id: Id, kind: FireplaceKind) -> Self {
        let (name, chimney) = match kind {
            FireplaceKind::Masonry => ("Fireplace", Chimney::default()),
            FireplaceKind::Prefab => (
                "Prefab Fireplace",
                Chimney {
                    width: 30.0,
                    depth: 30.0,
                    cap: CapKind::RainCap,
                    ..Chimney::default()
                },
            ),
            FireplaceKind::ChimneyOnly => ("Chimney", Chimney::default()),
        };
        Self {
            id,
            kind,
            name: name.into(),
            fuel: if kind == FireplaceKind::Prefab {
                Fuel::Gas
            } else {
                Fuel::Wood
            },
            in_wall: false,
            elevation: 0.0,
            height: None,
            firebox: Firebox::default(),
            hearth: Hearth::default(),
            mantel: Mantel::default(),
            chimney,
            materials: FireplaceMaterials::default(),
            label: FireplaceLabel::default(),
        }
    }

    /// The width and depth a new fireplace of this kind is placed with.
    pub fn default_size(kind: FireplaceKind) -> (f64, f64) {
        match kind {
            FireplaceKind::Masonry => (72.0, 24.0),
            FireplaceKind::Prefab => (54.0, 30.0),
            FireplaceKind::ChimneyOnly => (48.0, 24.0),
        }
    }

    /// The text of the plan label.
    pub fn label_text(&self) -> String {
        if self.label.text.trim().is_empty() {
            self.name.clone()
        } else {
            self.label.text.clone()
        }
    }

    /// Keeps the parts inside the body: the firebox no wider than the body
    /// less two 4" jambs, the chimney no larger than the body (a chimney only
    /// has its own size), positive sizes.
    pub fn fit_to(&mut self, width: f64, depth: f64) {
        let w = width.max(6.0);
        let d = depth.max(6.0);
        if self.kind.has_firebox() {
            self.firebox.width = self.firebox.width.clamp(6.0, (w - 8.0).max(6.0));
            self.firebox.depth = self.firebox.depth.clamp(2.0, d);
            self.firebox.height = self.firebox.height.max(6.0);
            self.firebox.raise = self.firebox.raise.max(0.0);
            self.chimney.width = self.chimney.width.clamp(6.0, w);
            self.chimney.depth = self.chimney.depth.clamp(6.0, d);
        } else {
            self.chimney.width = w;
            self.chimney.depth = d;
        }
        let (max_fw, max_fd) = (
            (self.chimney.width - 4.0).max(4.0),
            (self.chimney.depth - 4.0).max(4.0),
        );
        self.chimney.flue_width = self.chimney.flue_width.clamp(4.0, max_fw);
        self.chimney.flue_depth = self.chimney.flue_depth.clamp(4.0, max_fd);
        self.hearth.projection = self.hearth.projection.max(0.0);
        self.hearth.side_extension = self.hearth.side_extension.max(0.0);
        self.hearth.height = self.hearth.height.max(0.0);
        self.hearth.thickness = self.hearth.thickness.max(0.5);
        self.mantel.depth = self.mantel.depth.max(1.0);
        self.mantel.thickness = self.mantel.thickness.max(0.5);
    }

    /// Width of the mantel shelf: its own, else the opening plus 12" each
    /// side, never wider than the body less nothing (it may overhang).
    pub fn mantel_width(&self) -> f64 {
        if self.mantel.width > 0.0 {
            self.mantel.width
        } else {
            self.firebox.width + 24.0
        }
    }

    /// Top of the body above the floor: its own height, else the ceiling.
    pub fn body_top(&self, ceiling_height: f64) -> f64 {
        self.elevation + self.height.unwrap_or(ceiling_height).max(12.0)
    }
}

/// The plan frame of a fireplace symbol.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// Middle of the back.
    pub origin: Point,
    /// Unit vector across the front (local +x).
    pub u: Point,
    /// Unit vector from the back to the front (local +y).
    pub v: Point,
}

impl Frame {
    pub fn of(sym: &PlacedSymbol) -> Self {
        let a = sym.angle.to_radians();
        let u = Point::new(a.cos(), a.sin());
        Self {
            origin: sym.position,
            u,
            v: u.perp(),
        }
    }

    /// The plan point at local `(x, y)`.
    pub fn at(&self, x: f64, y: f64) -> Point {
        self.origin + self.u * x + self.v * y
    }

    /// The plan rectangle `x0..x1` by `y0..y1`, counter-clockwise.
    pub fn rect(&self, x0: f64, x1: f64, y0: f64, y1: f64) -> Vec<Point> {
        vec![
            self.at(x0, y0),
            self.at(x1, y0),
            self.at(x1, y1),
            self.at(x0, y1),
        ]
    }

    /// Local coordinates of a plan point.
    pub fn local(&self, p: Point) -> (f64, f64) {
        let d = p - self.origin;
        (d.dot(self.u), d.dot(self.v))
    }
}

/// Plan outline of the body.
pub fn body_poly(sym: &PlacedSymbol) -> Vec<Point> {
    let f = Frame::of(sym);
    f.rect(-sym.width * 0.5, sym.width * 0.5, 0.0, sym.depth)
}

/// Plan outline of the firebox opening: the recess cut into the front of
/// the body.
pub fn firebox_poly(fp: &Fireplace, sym: &PlacedSymbol) -> Vec<Point> {
    let f = Frame::of(sym);
    let w = fp.firebox.width * 0.5;
    let d = fp.firebox.depth.min(sym.depth);
    f.rect(-w, w, sym.depth - d, sym.depth)
}

/// Plan outline of the hearth, in front of the body.
pub fn hearth_poly(fp: &Fireplace, sym: &PlacedSymbol) -> Vec<Point> {
    if !fp.hearth.enabled || !fp.kind.has_firebox() {
        return Vec::new();
    }
    let f = Frame::of(sym);
    let w = fp.firebox.width * 0.5 + fp.hearth.side_extension;
    f.rect(-w, w, sym.depth, sym.depth + fp.hearth.projection)
}

/// Plan outline of the mantel shelf, which stands off the front of the body.
pub fn mantel_poly(fp: &Fireplace, sym: &PlacedSymbol) -> Vec<Point> {
    if !fp.mantel.enabled || !fp.kind.has_firebox() {
        return Vec::new();
    }
    let f = Frame::of(sym);
    let w = fp.mantel_width() * 0.5;
    f.rect(-w, w, sym.depth, sym.depth + fp.mantel.depth)
}

/// Plan outline of the chimney shaft: centered across the body and flush
/// with its back.
pub fn chimney_poly(fp: &Fireplace, sym: &PlacedSymbol) -> Vec<Point> {
    let f = Frame::of(sym);
    let (w, d) = if fp.kind.has_firebox() {
        (
            fp.chimney.width.min(sym.width),
            fp.chimney.depth.min(sym.depth),
        )
    } else {
        (sym.width, sym.depth)
    };
    f.rect(-w * 0.5, w * 0.5, 0.0, d)
}

/// The intervals `(s0, s1)` along `wall` (from its start) that an in-wall
/// fireplace replaces: where the wall's centerline runs through the body.
pub fn wall_cuts(fp: &Fireplace, sym: &PlacedSymbol, wall: &Wall) -> Vec<(f64, f64)> {
    if !fp.in_wall || wall.length() < 1.0 {
        return Vec::new();
    }
    let body = body_poly(sym);
    let dir = wall.direction();
    clip_line(&body, wall.start, dir)
        .into_iter()
        .filter_map(|(t0, t1)| {
            let (s0, s1) = (t0.max(0.0), t1.min(wall.length()));
            (s1 - s0 > 1.0).then_some((s0, s1))
        })
        .collect()
}

/// The 3-2-10 rule. `roof` gives the height of the roof surface above a plan
/// point (scene elevation), `None` where no roof is. `floor_elevation` is
/// the floor the fireplace stands on and `ceiling_height` that floor's.
///
/// The top is the greater of 3 ft above the highest roof under the chimney
/// and 2 ft above the highest roof within 10 ft of it; with no roof at all
/// it ends 3 ft above the ceiling. A fixed height wins when set.
pub fn chimney_top(
    fp: &Fireplace,
    sym: &PlacedSymbol,
    floor_elevation: f64,
    ceiling_height: f64,
    roof: &dyn Fn(Point) -> Option<f64>,
) -> f64 {
    let base = floor_elevation + fp.elevation;
    if let ChimneyTop::Height(h) = fp.chimney.top {
        return floor_elevation + h.max(fp.body_top(ceiling_height));
    }
    let poly = chimney_poly(fp, sym);
    let under = grid_max(&poly, 0.0, roof);
    let nearby = grid_max(&poly, NEARBY_REACH, roof);
    let mut top = floor_elevation + fp.body_top(ceiling_height) + fp.chimney.above_roof;
    if let Some(h) = under {
        top = top.max(h + fp.chimney.above_roof);
    }
    if let Some(h) = nearby {
        top = top.max(h + fp.chimney.above_nearby);
    }
    top.max(base + 12.0)
}

/// The highest roof height at points of a grid (12" apart) over the polygon
/// grown by `reach`.
fn grid_max(poly: &[Point], reach: f64, roof: &dyn Fn(Point) -> Option<f64>) -> Option<f64> {
    let (mut lo, mut hi) = (
        Point::new(f64::INFINITY, f64::INFINITY),
        Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
    );
    for p in poly {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    if !lo.x.is_finite() {
        return None;
    }
    let (lo, hi) = (
        Point::new(lo.x - reach, lo.y - reach),
        Point::new(hi.x + reach, hi.y + reach),
    );
    let step = 12.0;
    let nx = (((hi.x - lo.x) / step).ceil() as usize).max(1);
    let ny = (((hi.y - lo.y) / step).ceil() as usize).max(1);
    let mut best: Option<f64> = None;
    for i in 0..=nx {
        for j in 0..=ny {
            let p = Point::new(
                lo.x + (hi.x - lo.x) * i as f64 / nx as f64,
                lo.y + (hi.y - lo.y) * j as f64 / ny as f64,
            );
            // Under the chimney itself only points on its footprint count;
            // for the reach, any point within it of the footprint.
            let counts = if reach <= 0.0 {
                point_in_polygon(p, poly)
            } else {
                point_in_polygon(p, poly) || near_polygon(poly, p, reach)
            };
            if !counts {
                continue;
            }
            if let Some(h) = roof(p) {
                best = Some(best.map_or(h, |b: f64| b.max(h)));
            }
        }
    }
    best
}

fn near_polygon(poly: &[Point], p: Point, reach: f64) -> bool {
    let n = poly.len();
    (0..n).any(|i| crate::geometry::dist_to_segment(p, poly[i], poly[(i + 1) % n]) <= reach)
}

// ----- the records on a floor -----

impl Floor {
    /// The specification of the fireplace symbol `sym`: its record, else a
    /// default one for its catalog id (a pasted copy has none yet).
    pub fn fireplace_of(&self, sym: &PlacedSymbol) -> Fireplace {
        let mut fp = self
            .fireplaces
            .iter()
            .find(|f| f.id == sym.id)
            .cloned()
            .unwrap_or_else(|| {
                let kind = if sym.catalog_id == CHIMNEY_CATALOG_ID {
                    FireplaceKind::ChimneyOnly
                } else {
                    FireplaceKind::Masonry
                };
                Fireplace::new(sym.id, kind)
            });
        fp.id = sym.id;
        fp.fit_to(sym.width, sym.depth);
        fp
    }

    /// Every fireplace symbol of the floor with its specification.
    pub fn fireplace_symbols(&self) -> Vec<(&PlacedSymbol, Fireplace)> {
        self.symbols
            .iter()
            .filter(|s| is_fireplace_symbol(s))
            .map(|s| (s, self.fireplace_of(s)))
            .collect()
    }

    /// Stores `fp` as the record of its symbol and drops the records whose
    /// symbol is gone.
    pub fn set_fireplace(&mut self, fp: Fireplace) {
        match self.fireplaces.iter_mut().find(|f| f.id == fp.id) {
            Some(slot) => *slot = fp,
            None => self.fireplaces.push(fp),
        }
        self.prune_fireplaces();
    }

    /// Drops the records of fireplaces whose symbol no longer exists.
    pub fn prune_fireplaces(&mut self) -> usize {
        let before = self.fireplaces.len();
        let symbols = &self.symbols;
        self.fireplaces.retain(|f| {
            symbols
                .iter()
                .any(|s| s.id == f.id && is_fireplace_symbol(s))
        });
        before - self.fireplaces.len()
    }
}

impl Project {
    /// Adds a fireplace of `kind` on floor `floor` with its back centered at
    /// `position`, facing `angle` degrees (the front faces +y at 0), and
    /// returns its id.
    pub fn add_fireplace(
        &mut self,
        floor: usize,
        kind: FireplaceKind,
        position: Point,
        angle: f64,
    ) -> Id {
        let (w, d) = Fireplace::default_size(kind);
        // The viewer draws a stand-in block for every placed symbol; this one
        // has no height and sits under the floor, so only the fireplace's own
        // meshes (`plan_3d::fireplace`) show.
        let mut sym = PlacedSymbol::new(kind.catalog_id(), position, w, d, 0.0);
        sym.elevation = STAND_IN_ELEVATION;
        sym.angle = angle;
        sym.layer = FIREPLACE_LAYER.to_string();
        let id = self.add_symbol(floor, sym);
        let mut fp = Fireplace::new(id, kind);
        fp.fit_to(w, d);
        self.floors[floor].set_fireplace(fp);
        id
    }
}

/// The chimney chases that pass through floor `fi`: the shafts of the
/// fireplaces on lower floors that continue upward, as `(id, outline)`.
pub fn chases_on_floor(project: &Project, fi: usize) -> Vec<(Id, Vec<Point>)> {
    let mut out = Vec::new();
    for (i, floor) in project.floors.iter().enumerate().take(fi) {
        if floor.kind != crate::floors::FloorKind::Normal || floor.is_cad_detail() {
            continue;
        }
        for (sym, fp) in floor.fireplace_symbols() {
            if fp.chimney.enabled && fp.chimney.chase_through_floors && i < fi {
                out.push((sym.id, chimney_poly(&fp, sym)));
            }
        }
    }
    out
}

/// The plan area of a polygon, positive.
pub fn area(poly: &[Point]) -> f64 {
    polygon_area(poly).abs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Wall, WallKind};

    fn flat_roof(h: f64) -> impl Fn(Point) -> Option<f64> {
        move |_| Some(h)
    }

    fn project_with_fireplace() -> (Project, Id) {
        let mut p = Project::new("fp");
        let id = p.add_fireplace(0, FireplaceKind::Masonry, Point::new(100.0, 0.0), 0.0);
        (p, id)
    }

    #[test]
    fn a_new_fireplace_has_a_symbol_and_a_record() {
        let (p, id) = project_with_fireplace();
        let f = &p.floors[0];
        let sym = f.symbol(id).unwrap();
        assert!(is_fireplace_symbol(sym));
        assert_eq!((sym.width, sym.depth), (72.0, 24.0));
        assert_eq!(sym.layer, FIREPLACE_LAYER);
        assert_eq!(f.fireplaces.len(), 1);
        assert_eq!(f.fireplaces[0].kind, FireplaceKind::Masonry);
        assert_eq!(f.fireplace_symbols().len(), 1);
    }

    #[test]
    fn a_symbol_without_a_record_behaves_as_a_default_fireplace() {
        let (mut p, id) = project_with_fireplace();
        p.floors[0].fireplaces.clear();
        let sym = p.floors[0].symbol(id).unwrap().clone();
        let fp = p.floors[0].fireplace_of(&sym);
        assert_eq!(fp.id, id);
        assert_eq!(fp.firebox.width, 36.0);
    }

    #[test]
    fn deleting_the_symbol_drops_the_record() {
        let (mut p, id) = project_with_fireplace();
        assert!(p.remove_symbol(0, id));
        assert_eq!(p.floors[0].prune_fireplaces(), 1);
        assert!(p.floors[0].fireplaces.is_empty());
    }

    #[test]
    fn the_plan_parts_follow_the_symbol_frame() {
        let (p, id) = project_with_fireplace();
        let sym = p.floors[0].symbol(id).unwrap();
        let fp = p.floors[0].fireplace_of(sym);
        // Body: 72 wide from the back (y = 0) to 24 deep.
        let body = body_poly(sym);
        assert!((area(&body) - 72.0 * 24.0).abs() < 1e-6);
        // The firebox opens in the front: 36 wide, 18 deep.
        let fb = firebox_poly(&fp, sym);
        assert!((area(&fb) - 36.0 * 18.0).abs() < 1e-6);
        assert!(fb
            .iter()
            .all(|q| q.y >= 24.0 - 18.0 - 1e-9 && q.y <= 24.0 + 1e-9));
        // The hearth is in front: 36 + 16 wide, 16 deep.
        let h = hearth_poly(&fp, sym);
        assert!((area(&h) - 52.0 * 16.0).abs() < 1e-6);
        assert!(h.iter().all(|q| q.y >= 24.0 - 1e-9));
        // The chimney is 48 x 24, centered, flush with the back.
        let c = chimney_poly(&fp, sym);
        assert!((area(&c) - 48.0 * 24.0).abs() < 1e-6);
        assert!(c.iter().all(|q| (q.x - 100.0).abs() <= 24.0 + 1e-9));
    }

    #[test]
    fn rotating_the_symbol_turns_every_part() {
        let mut p = Project::new("r");
        let id = p.add_fireplace(0, FireplaceKind::Masonry, Point::new(0.0, 0.0), 90.0);
        let sym = p.floors[0].symbol(id).unwrap();
        let fp = p.floors[0].fireplace_of(sym);
        // At 90 degrees the front faces -x: the hearth is on the -x side.
        let h = hearth_poly(&fp, sym);
        assert!(h.iter().all(|q| q.x <= -24.0 + 1e-6), "{h:?}");
    }

    #[test]
    fn an_in_wall_fireplace_replaces_the_stretch_of_wall_behind_it() {
        let mut p = Project::new("w");
        let id = p.add_fireplace(0, FireplaceKind::Masonry, Point::new(100.0, -10.0), 0.0);
        let wall = Wall::new(
            Point::new(0.0, 0.0),
            Point::new(300.0, 0.0),
            6.5,
            109.0,
            WallKind::Exterior,
        );
        let sym = p.floors[0].symbol(id).unwrap().clone();
        let mut fp = p.floors[0].fireplace_of(&sym);
        // Beside the wall (the body is y -10..14): the wall is not cut.
        assert!(wall_cuts(&fp, &sym, &wall).is_empty());
        fp.in_wall = true;
        let cuts = wall_cuts(&fp, &sym, &wall);
        assert_eq!(cuts.len(), 1);
        assert!((cuts[0].0 - 64.0).abs() < 1e-6 && (cuts[0].1 - 136.0).abs() < 1e-6);
    }

    #[test]
    fn the_chimney_rises_three_feet_above_the_roof_it_passes_through() {
        let (p, id) = project_with_fireplace();
        let sym = p.floors[0].symbol(id).unwrap();
        let fp = p.floors[0].fireplace_of(sym);
        // A flat roof at 130": 3 ft above is 166".
        let top = chimney_top(&fp, sym, 0.0, 109.125, &flat_roof(130.0));
        assert!((top - 166.0).abs() < 1e-9, "{top}");
        // A taller roof 6 ft away (beyond the footprint, within 10 ft)
        // needs 2 ft above itself: 200 + 24 = 224.
        let tall = |q: Point| {
            if (q.x - 100.0).abs() > 40.0 && (q.x - 100.0).abs() < 100.0 {
                Some(200.0)
            } else {
                Some(130.0)
            }
        };
        let top = chimney_top(&fp, sym, 0.0, 109.125, &tall);
        assert!((top - 224.0).abs() < 1e-9, "{top}");
    }

    #[test]
    fn with_no_roof_the_chimney_ends_three_feet_above_the_ceiling() {
        let (p, id) = project_with_fireplace();
        let sym = p.floors[0].symbol(id).unwrap();
        let fp = p.floors[0].fireplace_of(sym);
        let none = |_: Point| None;
        let top = chimney_top(&fp, sym, 0.0, 109.125, &none);
        assert!((top - (109.125 + 36.0)).abs() < 1e-9);
    }

    #[test]
    fn a_fixed_height_wins_but_never_ends_inside_the_body() {
        let (p, id) = project_with_fireplace();
        let sym = p.floors[0].symbol(id).unwrap();
        let mut fp = p.floors[0].fireplace_of(sym);
        fp.chimney.top = ChimneyTop::Height(200.0);
        assert_eq!(
            chimney_top(&fp, sym, 10.0, 109.125, &flat_roof(500.0)),
            210.0
        );
        fp.chimney.top = ChimneyTop::Height(20.0);
        assert!(chimney_top(&fp, sym, 0.0, 109.125, &flat_roof(0.0)) >= 109.125);
    }

    #[test]
    fn a_chimney_chase_passes_through_the_floors_above() {
        let mut p = Project::new("c");
        p.floors.push(crate::model::Floor::new("2nd Floor", 120.0));
        p.floors.push(crate::model::Floor::new("3rd Floor", 240.0));
        let id = p.add_fireplace(0, FireplaceKind::Masonry, Point::new(0.0, 0.0), 0.0);
        assert!(chases_on_floor(&p, 0).is_empty());
        assert_eq!(chases_on_floor(&p, 1).len(), 1);
        assert_eq!(chases_on_floor(&p, 2).len(), 1);
        let mut fp = p.floors[0].fireplace_of(p.floors[0].symbol(id).unwrap());
        fp.chimney.chase_through_floors = false;
        p.floors[0].set_fireplace(fp);
        assert!(chases_on_floor(&p, 1).is_empty());
    }

    #[test]
    fn fitting_keeps_the_parts_inside_the_body() {
        let mut fp = Fireplace::new(1, FireplaceKind::Masonry);
        fp.firebox.width = 100.0;
        fp.chimney.width = 200.0;
        fp.fit_to(60.0, 20.0);
        assert!(fp.firebox.width <= 52.0);
        assert!(fp.chimney.width <= 60.0 && fp.chimney.depth <= 20.0);
        let mut chimney = Fireplace::new(2, FireplaceKind::ChimneyOnly);
        chimney.fit_to(30.0, 20.0);
        assert_eq!((chimney.chimney.width, chimney.chimney.depth), (30.0, 20.0));
    }

    #[test]
    fn records_round_trip_and_sparse_json_loads() {
        let mut fp = Fireplace::new(7, FireplaceKind::Prefab);
        fp.chimney.top = ChimneyTop::Height(180.0);
        fp.mantel.leg_width = 6.0;
        let json = serde_json::to_string(&fp).unwrap();
        let back: Fireplace = serde_json::from_str(&json).unwrap();
        assert_eq!(back, fp);
        let sparse: Fireplace =
            serde_json::from_str(r#"{"id":3,"hearth":{"projection":20}}"#).unwrap();
        assert_eq!(sparse.hearth.projection, 20.0);
        assert_eq!(sparse.firebox.width, 36.0);
        let mut project = Project::new("io");
        project.add_fireplace(0, FireplaceKind::Prefab, Point::new(5.0, 5.0), 0.0);
        let s = serde_json::to_string(&project).unwrap();
        let back: Project = serde_json::from_str(&s).unwrap();
        assert_eq!(back.floors[0].fireplaces.len(), 1);
    }
}
