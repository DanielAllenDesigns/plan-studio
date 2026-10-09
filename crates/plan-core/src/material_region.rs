//! Layered Floor and Wall Material Regions (reference manual "Floor and Wall
//! Material Regions" and "Material Layers Definition Dialogs", pp. 1085 to
//! 1091; parity rows CB-462..CB-467).
//!
//! The region itself (its outline, side, "cut finish layers" switch, plan
//! line style) is a [`crate::details::MaterialRegion`] in the floor's details
//! layer. This module adds the **structure** it is made of: an ordered list
//! of [`MaterialLayer`]s, top (the exposed surface) to bottom, each with a
//! material, a fill style, a [`LayerRole`] and a thickness. A region without
//! a structure is one layer of its own `material` and `thickness`.
//!
//! * [`RegionStructure`] holds the table and its edit commands (Insert
//!   Above/Below, Move Up/Down, Delete) and the label.
//! * [`Floor::region_layers_of`] gives the layers a region has, structure or
//!   not; [`Floor::set_region_structure`] stores one and keeps the region's
//!   `material` and `thickness` in step (top layer, total).
//! * A region cuts into the parent's finish layers or sits on its surface:
//!   [`layer_bands`] gives each layer's distance range from the parent's
//!   surface for both cases (the 3D view in `plan-3d::material_region` and
//!   the take-off both read it).
//! * A wall region cuts around the wall's openings ([`wall_region_rects`]).
//! * [`r15_takeoff`] turns regions, 3D solids, compound solids, soffits and
//!   architectural blocks into Materials List rows.
//! * [`convert_polyline_to_region`] is Convert Polyline for a closed CAD
//!   polyline.

use crate::arch_block::BlockLayer;
use crate::details::{DetailRef, DetailsLayer, MaterialRegion, RegionKind, DEFAULT_REGION_MATERIAL};
use crate::geometry::{polygon_area, Point};
use crate::groups::ObjectRef;
use crate::model::{Floor, Id, Project};
use serde::{Deserialize, Serialize};

/// Smallest layer thickness the table accepts, inches.
pub const MIN_LAYER_THICKNESS: f64 = 0.001;

/// What a layer is (the Role column, p. 1089).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LayerRole {
    /// A material that fills the layer as its definition says.
    #[default]
    Standard,
    /// Generates framing or trusses in the layer.
    Framing,
    /// Not drawn in 3D and not counted in the Materials List.
    AirGap,
    /// Carries a 3D Cladding.
    Cladding3d,
}

impl LayerRole {
    pub const ALL: [LayerRole; 4] = [
        LayerRole::Standard,
        LayerRole::Framing,
        LayerRole::AirGap,
        LayerRole::Cladding3d,
    ];

    pub fn name(self) -> &'static str {
        match self {
            LayerRole::Standard => "Standard",
            LayerRole::Framing => "Framing",
            LayerRole::AirGap => "Air Gap",
            LayerRole::Cladding3d => "3D Cladding",
        }
    }

    /// Does the layer show in 3D and count in the Materials List?
    pub fn is_solid(self) -> bool {
        !matches!(self, LayerRole::AirGap)
    }
}

/// One row of the Material Layers table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MaterialLayer {
    /// Name of a library material.
    pub material: String,
    /// The Fill style Auto Detail uses in sections (a name; blank is the
    /// material's own).
    pub fill: String,
    pub role: LayerRole,
    pub thickness: f64,
}

impl Default for MaterialLayer {
    fn default() -> Self {
        Self {
            material: DEFAULT_REGION_MATERIAL.to_string(),
            fill: String::new(),
            role: LayerRole::Standard,
            thickness: 0.25,
        }
    }
}

impl MaterialLayer {
    pub fn new(material: impl Into<String>, thickness: f64) -> Self {
        Self {
            material: material.into(),
            thickness,
            ..Self::default()
        }
    }
}

/// The layers of one region and its Label panel.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RegionStructure {
    /// The id of the [`MaterialRegion`] this belongs to.
    pub region: Id,
    pub layers: Vec<MaterialLayer>,
    /// Custom label; the automatic label of a region is blank (p. 1088).
    pub label: String,
    /// Auto Detail as Insulation (p. 1091).
    pub auto_detail_insulation: bool,
}

impl RegionStructure {
    pub fn new(region: Id, layers: Vec<MaterialLayer>) -> Self {
        Self {
            region,
            layers,
            ..Self::default()
        }
    }

    /// Total thickness of every layer, inches.
    pub fn total_thickness(&self) -> f64 {
        self.layers.iter().map(|l| l.thickness).sum()
    }

    /// Insert Above row `i` (a blank layer takes the row's material);
    /// returns the new row's index.
    pub fn insert_above(&mut self, i: usize) -> usize {
        let i = i.min(self.layers.len());
        let proto = self.layers.get(i).cloned().unwrap_or_default();
        self.layers.insert(i, MaterialLayer { role: LayerRole::Standard, ..proto });
        i
    }

    /// Insert Below row `i`; returns the new row's index.
    pub fn insert_below(&mut self, i: usize) -> usize {
        if self.layers.is_empty() {
            self.layers.push(MaterialLayer::default());
            return 0;
        }
        let i = i.min(self.layers.len() - 1);
        let proto = self.layers[i].clone();
        self.layers.insert(i + 1, MaterialLayer { role: LayerRole::Standard, ..proto });
        i + 1
    }

    /// Move Up swaps row `i` with the one above; the top row stays.
    pub fn move_up(&mut self, i: usize) -> usize {
        if i == 0 || i >= self.layers.len() {
            return i;
        }
        self.layers.swap(i, i - 1);
        i - 1
    }

    /// Move Down swaps row `i` with the one below; the bottom row stays.
    pub fn move_down(&mut self, i: usize) -> usize {
        if i + 1 >= self.layers.len() {
            return i;
        }
        self.layers.swap(i, i + 1);
        i + 1
    }

    /// Delete row `i`; the last layer cannot be deleted. Returns the row
    /// that is now selected.
    pub fn delete(&mut self, i: usize) -> usize {
        if self.layers.len() <= 1 || i >= self.layers.len() {
            return i.min(self.layers.len().saturating_sub(1));
        }
        self.layers.remove(i);
        i.min(self.layers.len() - 1)
    }

    /// A reason the table cannot be accepted.
    pub fn error(&self) -> Option<String> {
        if self.layers.is_empty() {
            return Some("A material region needs at least one layer".into());
        }
        if let Some(i) = self
            .layers
            .iter()
            .position(|l| !(l.thickness.is_finite() && l.thickness >= MIN_LAYER_THICKNESS))
        {
            return Some(format!("Layer {} needs a thickness", i + 1));
        }
        None
    }
}

/// A layer's place across the parent's surface: distances measured from the
/// parent surface the region belongs to, positive away from the parent
/// (above a floor, out of a wall face). A band with `lo < 0` is cut into
/// the parent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band {
    pub layer: usize,
    pub lo: f64,
    pub hi: f64,
}

/// Where each layer of `layers` sits: a region that **cuts** the finish
/// layers has its first (exposed) layer flush with the parent surface and
/// the rest below it; one that does not sits on the surface with the first
/// layer outermost.
pub fn layer_bands(layers: &[MaterialLayer], cut: bool) -> Vec<Band> {
    let total: f64 = layers.iter().map(|l| l.thickness).sum();
    let mut out = Vec::with_capacity(layers.len());
    let mut depth = 0.0;
    for (i, l) in layers.iter().enumerate() {
        let (lo, hi) = if cut {
            (-(depth + l.thickness), -depth)
        } else {
            (total - depth - l.thickness, total - depth)
        };
        out.push(Band { layer: i, lo, hi });
        depth += l.thickness;
    }
    out
}

/// Rectangles `(u0, u1, v0, v1)` of the wall region `r` after the wall's
/// openings are cut out of it (Wall Material Regions cut around doors,
/// windows and fireplaces, p. 1086). The region's own rectangle when no
/// opening touches it; empty for a floor region or a missing wall.
pub fn wall_region_rects(floor: &Floor, r: &MaterialRegion) -> Vec<[f64; 4]> {
    let RegionKind::Wall(wid) = r.kind else {
        return Vec::new();
    };
    let Some((u0, u1, v0, v1)) = r.uv_bounds() else {
        return Vec::new();
    };
    let Some(w) = floor.wall(wid) else {
        return Vec::new();
    };
    let (u0, u1) = (u0.max(0.0), u1.min(w.length()));
    let (v0, v1) = (v0.max(0.0), v1.min(w.height));
    if u1 - u0 <= 1e-9 || v1 - v0 <= 1e-9 {
        return Vec::new();
    }
    // Opening rectangles clipped to the region.
    let holes: Vec<[f64; 4]> = floor
        .openings_on(wid)
        .filter_map(|o| {
            let (a, b) = (o.center_offset - o.width * 0.5, o.center_offset + o.width * 0.5);
            let (c, d) = (o.sill_height, o.sill_height + o.height);
            let h = [a.max(u0), b.min(u1), c.max(v0), d.min(v1)];
            (h[1] - h[0] > 1e-9 && h[3] - h[2] > 1e-9).then_some(h)
        })
        .collect();
    if holes.is_empty() {
        return vec![[u0, u1, v0, v1]];
    }
    let mut cuts: Vec<f64> = vec![u0, u1];
    for h in &holes {
        cuts.push(h[0]);
        cuts.push(h[1]);
    }
    cuts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    let mut out = Vec::new();
    for pair in cuts.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let mid = (a + b) * 0.5;
        let mut spans: Vec<(f64, f64)> = holes
            .iter()
            .filter(|h| h[0] <= mid && mid <= h[1])
            .map(|h| (h[2], h[3]))
            .collect();
        spans.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut y = v0;
        for (s, e) in spans {
            if s > y + 1e-9 {
                out.push([a, b, y, s]);
            }
            y = y.max(e);
        }
        if v1 > y + 1e-9 {
            out.push([a, b, y, v1]);
        }
    }
    out
}

/// Net surface of a region, square inches: a floor region's polygon, a wall
/// region's rectangle less the openings.
pub fn net_area(floor: &Floor, r: &MaterialRegion) -> f64 {
    match r.kind {
        RegionKind::Floor => polygon_area(&r.outline).abs(),
        RegionKind::Wall(_) => wall_region_rects(floor, r)
            .iter()
            .map(|q| (q[1] - q[0]) * (q[3] - q[2]))
            .sum(),
    }
}

/// What one layer of a region adds up to.
#[derive(Debug, Clone, PartialEq)]
pub struct RegionQuantity {
    pub region: Id,
    pub layer: usize,
    pub material: String,
    pub role: LayerRole,
    /// Square inches of surface.
    pub area: f64,
    pub thickness: f64,
    /// Cubic inches.
    pub volume: f64,
}

impl Floor {
    /// The structure stored for region `id`.
    pub fn region_structure(&self, id: Id) -> Option<&RegionStructure> {
        self.region_layers.iter().find(|s| s.region == id)
    }

    /// The layers of region `r`: its structure, else one layer of its own
    /// material and thickness.
    pub fn region_layers_of(&self, r: &MaterialRegion) -> Vec<MaterialLayer> {
        match self.region_structure(r.id) {
            Some(s) if !s.layers.is_empty() => s.layers.clone(),
            _ => vec![MaterialLayer::new(r.material.clone(), r.thickness)],
        }
    }

    /// Does region `id` have a structure of its own (so the single plate is
    /// not drawn)?
    pub fn has_region_structure(&self, id: Id) -> bool {
        self.region_structure(id).is_some_and(|s| !s.layers.is_empty())
    }

    /// Stores `s` and brings the region's `material` (the exposed layer) and
    /// `thickness` (the total) in step with it. Returns whether the region
    /// exists.
    pub fn set_region_structure(&mut self, s: RegionStructure) -> bool {
        let mut details = DetailsLayer::load(self);
        let Some(r) = details.regions.iter_mut().find(|r| r.id == s.region) else {
            return false;
        };
        if let Some(top) = s.layers.iter().find(|l| l.role.is_solid()) {
            r.material = top.material.clone();
        }
        r.thickness = s.total_thickness().max(MIN_LAYER_THICKNESS);
        let _ = self.set_details(&details);
        self.region_layers.retain(|x| x.region != s.region);
        self.region_layers.push(s);
        true
    }

    /// The quantities of region `id`, one entry per solid layer.
    pub fn region_quantities(&self, r: &MaterialRegion) -> Vec<RegionQuantity> {
        let area = net_area(self, r);
        self.region_layers_of(r)
            .iter()
            .enumerate()
            .filter(|(_, l)| l.role.is_solid())
            .map(|(i, l)| RegionQuantity {
                region: r.id,
                layer: i,
                material: l.material.clone(),
                role: l.role,
                area,
                thickness: l.thickness,
                volume: area * l.thickness,
            })
            .collect()
    }

    /// Forgets structures of regions that are gone.
    pub fn drop_region_orphans(&mut self) -> usize {
        let details = DetailsLayer::load(self);
        let n = self.region_layers.len();
        self.region_layers
            .retain(|s| details.regions.iter().any(|r| r.id == s.region));
        n - self.region_layers.len()
    }
}

/// Convert Polyline: the closed CAD polyline `cad_id` becomes a floor
/// Material Region with id `new_id` (p. 1086). The polyline goes. Fails for
/// anything but a closed polyline of three or more points.
pub fn convert_polyline_to_region(
    floor: &mut Floor,
    cad_id: Id,
    new_id: Id,
) -> Result<Id, String> {
    let item = floor
        .cad
        .iter()
        .find(|c| c.id == cad_id)
        .map(|c| c.item.clone())
        .ok_or("That object is gone")?;
    let pts = match item {
        crate::cad::CadItem::Polyline { points, closed } if closed && points.len() >= 3 => points,
        _ => return Err("Select a closed CAD polyline to convert to a Material Region".into()),
    };
    if polygon_area(&pts).abs() < 1e-6 {
        return Err("The polyline encloses no area".into());
    }
    floor.cad.retain(|c| c.id != cad_id);
    floor.cad_attrs.retain(|a| a.target != cad_id);
    let mut details = DetailsLayer::load(floor);
    details.regions.push(MaterialRegion::floor(new_id, pts));
    floor
        .set_details(&details)
        .map_err(|e| e.to_string())?;
    Ok(new_id)
}

// ===================================================================
// Materials List rows
// ===================================================================

/// One row the Materials List can take over: `qty` in `unit`, from the
/// object `key` (`detail:12`, `block:7`, `cabinet:3` ...).
#[derive(Debug, Clone, PartialEq)]
pub struct TakeoffRow {
    pub floor: usize,
    pub category: String,
    pub item: String,
    pub material: String,
    pub unit: &'static str,
    pub qty: f64,
    pub key: String,
}

const CU_IN_PER_CU_FT: f64 = 1728.0;
const SQ_IN_PER_SQ_FT: f64 = 144.0;

/// The rows of floor `fi` for what this module owns: layered material
/// regions (a row per solid layer, square feet), 3D solids and compound
/// solids (cubic feet), and architectural blocks that are not treated as one
/// object (their sub-objects are listed instead, nothing is counted twice).
/// Soffits are counted by [`soffit_surface`].
pub fn r15_takeoff(project: &Project, fi: usize) -> Vec<TakeoffRow> {
    let Some(f) = project.floors.get(fi) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let details = DetailsLayer::load(f);
    for r in &details.regions {
        let floor_region = r.is_floor();
        for q in f.region_quantities(r) {
            if q.area <= 1e-9 {
                continue;
            }
            out.push(TakeoffRow {
                floor: fi,
                category: if floor_region { "Flooring" } else { "Interior Finishes" }.into(),
                item: format!(
                    "{} region layer {}",
                    if floor_region { "Floor" } else { "Wall" },
                    q.layer + 1
                ),
                material: q.material,
                unit: "sq ft",
                qty: q.area / SQ_IN_PER_SQ_FT,
                key: format!("detail:{}", r.id),
            });
        }
    }
    for s in &details.solids {
        let ext = f.solid_layer.ext_of(s.id);
        let tris = crate::solids::solid_tris(s, ext);
        let v = crate::solids::volume(&tris);
        if v > 1e-9 {
            out.push(TakeoffRow {
                floor: fi,
                category: "Other".into(),
                item: s.kind.name().to_string(),
                material: s.material.clone(),
                unit: "cu ft",
                qty: v / CU_IN_PER_CU_FT,
                key: format!("detail:{}", s.id),
            });
        }
    }
    for c in &f.solid_layer.compounds {
        let v = c.volume();
        if v > 1e-9 {
            out.push(TakeoffRow {
                floor: fi,
                category: "Other".into(),
                item: c.name.clone(),
                material: c.material.clone(),
                unit: "cu ft",
                qty: v / CU_IN_PER_CU_FT,
                key: format!("solid:{}", c.id),
            });
        }
    }
    for b in &f.blocks.blocks {
        if b.treat_as_one && !b.schedule.exclude {
            out.push(TakeoffRow {
                floor: fi,
                category: "Other".into(),
                item: b.name.clone(),
                material: String::new(),
                unit: "ea",
                qty: 1.0,
                key: format!("block:{}", b.id),
            });
        }
    }
    out
}

/// The surface of a soffit that takes a surface material, square inches,
/// following the manual's rule (p. 1082): a shallow soffit (depth less than
/// the larger of 4 inches and 1 1/2 times the material thickness) counts its
/// front only; a deep one counts the front plus the sides, top and bottom
/// less twice the material thickness each (a soffit is hollow); back faces
/// against a wall are not counted. `attached_to_wall` says the back and the
/// top are against a wall and ceiling.
pub fn soffit_surface(
    width: f64,
    height: f64,
    depth: f64,
    material_thickness: f64,
    attached_to_wall: bool,
) -> f64 {
    let front = width * height;
    let limit = 4.0_f64.max(1.5 * material_thickness);
    if depth <= limit {
        return front;
    }
    let t2 = 2.0 * material_thickness;
    let d = (depth - t2).max(0.0);
    let sides = 2.0 * d * (height - t2).max(0.0);
    let bottom = d * (width - t2).max(0.0);
    let top = if attached_to_wall { 0.0 } else { bottom };
    let back = if attached_to_wall { 0.0 } else { (width - t2).max(0.0) * (height - t2).max(0.0) };
    front + sides + bottom + top + back
}

/// Is any block holding a member of `blocks` the given object? Used by the
/// Materials List to skip sub-objects of a block that is treated as one.
pub fn inside_one_object_block(blocks: &BlockLayer, r: ObjectRef) -> bool {
    blocks.root_of(r).is_some_and(|b| b.treat_as_one)
}

/// A material region's id from a details reference.
pub fn region_id(r: DetailRef) -> Option<Id> {
    match r {
        DetailRef::Region(i) => Some(i),
        _ => None,
    }
}

/// A plan point for tests and callers that need a typical region square.
pub fn square(x: f64, y: f64, side: f64) -> Vec<Point> {
    vec![
        Point::new(x, y),
        Point::new(x + side, y),
        Point::new(x + side, y + side),
        Point::new(x, y + side),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cad::{CadItem, CadObject};
    use crate::walls::Side;
    use crate::{Opening, OpeningKind, WallKind};

    fn project_with_wall() -> (Project, Id) {
        let mut p = Project::new("r");
        let wid = p.add_wall(0, Point::new(0.0, 0.0), Point::new(200.0, 0.0), 6.5, 96.0, WallKind::Exterior);
        (p, wid)
    }

    #[test]
    fn the_table_edits_like_chiefs() {
        let mut s = RegionStructure::new(
            1,
            vec![MaterialLayer::new("Tile", 0.5), MaterialLayer::new("Mortar", 0.75)],
        );
        assert!((s.total_thickness() - 1.25).abs() < 1e-9);
        let i = s.insert_above(1);
        assert_eq!((i, s.layers.len()), (1, 3));
        assert_eq!(s.layers[1].material, "Mortar", "a new layer starts as a copy of the row");
        let i = s.insert_below(2);
        assert_eq!(i, 3);
        assert_eq!(s.move_up(0), 0);
        assert_eq!(s.move_up(3), 2);
        assert_eq!(s.move_down(3), 3);
        assert_eq!(s.delete(0), 0);
        assert_eq!(s.layers.len(), 3);
        while s.layers.len() > 1 {
            s.delete(0);
        }
        assert_eq!(s.delete(0), 0);
        assert_eq!(s.layers.len(), 1, "the last layer stays");
        s.layers[0].thickness = 0.0;
        assert!(s.error().is_some());
        s.layers.clear();
        assert!(s.error().is_some());
    }

    #[test]
    fn bands_cut_into_or_sit_on_the_surface() {
        let layers = vec![MaterialLayer::new("Tile", 0.5), MaterialLayer::new("Mortar", 0.25)];
        let cut = layer_bands(&layers, true);
        assert_eq!((cut[0].lo, cut[0].hi), (-0.5, 0.0));
        assert_eq!((cut[1].lo, cut[1].hi), (-0.75, -0.5));
        let on = layer_bands(&layers, false);
        assert_eq!((on[0].lo, on[0].hi), (0.25, 0.75), "the exposed layer is outermost");
        assert_eq!((on[1].lo, on[1].hi), (0.0, 0.25));
    }

    #[test]
    fn a_wall_region_cuts_around_openings() {
        let (mut p, wid) = project_with_wall();
        let oid = p.alloc_id();
        let mut o = Opening::new(wid, 100.0, OpeningKind::Door, 36.0, 80.0, 0.0);
        o.id = oid;
        p.floors[0].openings.push(o);
        let rid = p.alloc_id();
        let r = MaterialRegion::wall(rid, wid, Side::Left, 0.0, 200.0, 0.0, 96.0);
        let rects = wall_region_rects(&p.floors[0], &r);
        let area: f64 = rects.iter().map(|q| (q[1] - q[0]) * (q[3] - q[2])).sum();
        assert!((area - (200.0 * 96.0 - 36.0 * 80.0)).abs() < 1e-6, "{area}");
        assert!(rects.len() >= 3);
        assert!((net_area(&p.floors[0], &r) - area).abs() < 1e-9);
        // No opening under the region: the plain rectangle.
        let r2 = MaterialRegion::wall(rid, wid, Side::Left, 0.0, 50.0, 0.0, 96.0);
        assert_eq!(wall_region_rects(&p.floors[0], &r2), vec![[0.0, 50.0, 0.0, 96.0]]);
    }

    #[test]
    fn a_structure_syncs_the_region_and_feeds_the_takeoff() {
        let (mut p, _) = project_with_wall();
        let rid = p.alloc_id();
        let mut d = DetailsLayer::default();
        d.regions.push(MaterialRegion::floor(rid, square(0.0, 0.0, 120.0)));
        p.floors[0].set_details(&d).unwrap();
        let s = RegionStructure::new(
            rid,
            vec![
                MaterialLayer::new("Ceramic Tile 12x12", 0.375),
                MaterialLayer {
                    role: LayerRole::AirGap,
                    thickness: 1.0,
                    ..MaterialLayer::default()
                },
                MaterialLayer::new("Concrete", 1.5),
            ],
        );
        let f = &mut p.floors[0];
        assert!(f.set_region_structure(s));
        assert!(f.has_region_structure(rid));
        let r = DetailsLayer::load(f).regions[0].clone();
        assert_eq!(r.material, "Ceramic Tile 12x12");
        assert!((r.thickness - 2.875).abs() < 1e-9);
        let q = f.region_quantities(&r);
        assert_eq!(q.len(), 2, "the air gap is not counted");
        assert!((q[0].area - 14400.0).abs() < 1e-6);
        assert!((q[1].volume - 14400.0 * 1.5).abs() < 1e-6);
        let rows = r15_takeoff(&p, 0);
        let flooring: Vec<_> = rows.iter().filter(|r| r.category == "Flooring").collect();
        assert_eq!(flooring.len(), 2);
        assert!((flooring[0].qty - 100.0).abs() < 1e-9, "10 ft x 10 ft");
        // Round trip.
        let back = Project::from_json(&p.to_json().unwrap()).unwrap();
        assert_eq!(back.floors[0].region_layers.len(), 1);
        assert_eq!(back.floors[0].region_layers[0].layers.len(), 3);
        // A region with no structure is one layer of its own.
        let mut f2 = back.floors[0].clone();
        f2.region_layers.clear();
        assert_eq!(f2.region_layers_of(&r).len(), 1);
        assert_eq!(f2.drop_region_orphans(), 0);
    }

    #[test]
    fn convert_polyline_makes_a_floor_region() {
        let mut p = Project::new("c");
        let cid = p.alloc_id();
        p.floors[0].cad.push(CadObject {
            id: cid,
            layer: "CAD, Default".into(),
            item: CadItem::Polyline {
                points: square(0.0, 0.0, 48.0),
                closed: true,
            },
        });
        let open = p.alloc_id();
        p.floors[0].cad.push(CadObject {
            id: open,
            layer: "CAD, Default".into(),
            item: CadItem::Polyline {
                points: square(0.0, 0.0, 48.0),
                closed: false,
            },
        });
        let new_id = p.alloc_id();
        let f = &mut p.floors[0];
        assert!(convert_polyline_to_region(f, open, new_id).is_err());
        assert_eq!(convert_polyline_to_region(f, cid, new_id), Ok(new_id));
        assert!(f.cad.iter().all(|c| c.id != cid));
        let r = &DetailsLayer::load(f).regions[0];
        assert!(r.is_floor() && (r.area() - 2304.0).abs() < 1e-9);
    }

    #[test]
    fn soffit_materials_follow_the_manual() {
        // 48 x 48 x 4 soffit with 3 inch brick: front only (the manual's example).
        assert!((soffit_surface(48.0, 48.0, 4.0, 3.0, true) - 2304.0).abs() < 1e-9);
        // 12 deep is deeper than 2 x 3: the sides count, less twice the thickness.
        let deep = soffit_surface(48.0, 48.0, 12.0, 3.0, true);
        assert!(deep > 2304.0);
        let free = soffit_surface(48.0, 48.0, 12.0, 3.0, false);
        assert!(free > deep);
        // A tiny soffit never goes negative.
        assert!(soffit_surface(5.0, 5.0, 10.0, 3.0, false) >= 25.0);
    }
}
