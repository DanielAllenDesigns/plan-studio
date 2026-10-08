//! Slabs, slab holes, square pads, round piers and holes in the floor and
//! ceiling platforms (the Slab flyout and the Floor flyout's two platform
//! hole tools; `docs/chief-x18-subtools.md`).
//!
//! Lengths are inches. Elevations are relative to the finished-floor
//! elevation of the floor the object sits on (`Floor::elevation`), so a slab
//! with `top_elevation == 0` has its top at the floor's elevation and its
//! body below it.
//!
//! # Storage
//!
//! A floor's [`FoundationLayer`] lives in the typed slot `Floor.foundation`
//! (opaque JSON, like `roofs` and `electrical`), saved and undone with the
//! plan. Older files kept it as one JSON text record in `Floor.cad` on the
//! hidden layer [`DATA_LAYER`]: [`FoundationLayer::load`] still reads such a
//! record when the slot is empty, [`FoundationLayer::store`] moves the data
//! into the slot and removes the record, and [`migrate_legacy`] converts a
//! whole project once when it is loaded.

use crate::cad::{CadItem, CadObject};
use crate::geometry::{point_in_polygon, polygon_area, polygon_centroid, Point};
use crate::layers::LineStyle;
use crate::model::{Floor, Id, Project};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Layer slabs and slab holes are drawn on.
pub const SLAB_LAYER: &str = "Slabs";
/// Layer square pads and round piers are drawn on.
pub const PIER_LAYER: &str = "Piers/Pads";
/// Layer holes in the floor platform are drawn on (dashed outline).
pub const FLOOR_HOLES_LAYER: &str = "Floors, Holes";
/// Layer holes in the ceiling platform are drawn on (dashed outline).
pub const CEILING_HOLES_LAYER: &str = "Ceilings, Holes";
/// Hidden layer of the JSON record that stores the [`FoundationLayer`].
pub const DATA_LAYER: &str = "Foundation, Data";
/// Prefix of the record text, so other text on the layer is never mistaken for it.
const RECORD_TAG: &str = "FND1:";

/// Default slab thickness, inches.
pub const DEFAULT_SLAB_THICKNESS: f64 = 4.0;
/// Default footing width, inches.
pub const DEFAULT_FOOTING_WIDTH: f64 = 16.0;
/// Default footing depth (height below the slab or pad), inches.
pub const DEFAULT_FOOTING_DEPTH: f64 = 8.0;
/// Default square pad size, inches.
pub const DEFAULT_PAD_SIZE: f64 = 24.0;
/// Default pad thickness, inches.
pub const DEFAULT_PAD_THICKNESS: f64 = 12.0;
/// Default round pier diameter, inches.
pub const DEFAULT_PIER_DIAMETER: f64 = 12.0;
/// Default round pier height, inches.
pub const DEFAULT_PIER_HEIGHT: f64 = 36.0;
/// Name of the default slab material.
pub const DEFAULT_MATERIAL: &str = "Concrete";

const CU_IN_PER_CU_YD: f64 = 46_656.0;

/// A footing: the concrete below a slab edge, a hole edge or a pier.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Footing {
    /// Width across the footing, inches (for a pier footing the side of the
    /// square footing).
    pub width: f64,
    /// Height of the footing below the slab, pad or pier, inches.
    pub depth: f64,
}

impl Default for Footing {
    fn default() -> Self {
        Self {
            width: DEFAULT_FOOTING_WIDTH,
            depth: DEFAULT_FOOTING_DEPTH,
        }
    }
}

/// A slab (Slab, Slab with Footing).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Slab {
    pub id: Id,
    pub outline: Vec<Point>,
    /// Slab thickness, inches.
    pub thickness: f64,
    /// Elevation of the top of the slab relative to the floor, inches.
    pub top_elevation: f64,
    /// A footing under the outer edge (Slab with Footing).
    pub footing: Option<Footing>,
    pub material: String,
    /// Holes cut into this slab by its own specification.
    pub holes: Vec<Vec<Point>>,
    pub layer: String,
    /// Plan fill color.
    pub fill_color: [u8; 3],
    /// Plan fill pattern name (`None`, `Solid`, `Hatch`, `Cross Hatch`, `Grid`).
    pub fill_pattern: String,
    pub line_style: LineStyle,
}

impl Default for Slab {
    fn default() -> Self {
        Self {
            id: 0,
            outline: Vec::new(),
            thickness: DEFAULT_SLAB_THICKNESS,
            top_elevation: 0.0,
            footing: None,
            material: DEFAULT_MATERIAL.to_string(),
            holes: Vec::new(),
            layer: SLAB_LAYER.to_string(),
            fill_color: [190, 190, 185],
            fill_pattern: "Hatch".to_string(),
            line_style: LineStyle::Solid,
        }
    }
}

impl Slab {
    pub fn new(id: Id, outline: Vec<Point>) -> Self {
        Self {
            id,
            outline,
            ..Self::default()
        }
    }

    /// Elevation of the underside of the slab, relative to the floor.
    pub fn bottom_elevation(&self) -> f64 {
        self.top_elevation - self.thickness
    }

    /// Area inside the outline, square inches.
    pub fn gross_area(&self) -> f64 {
        outline_area(&self.outline)
    }

    /// Length of the outline, inches.
    pub fn perimeter(&self) -> f64 {
        outline_perimeter(&self.outline)
    }

    /// Area after taking out its own holes and the layer's slab holes that
    /// fall inside it (`extra`).
    pub fn net_area(&self, extra: &[&SlabHole]) -> f64 {
        let own: f64 = self.holes.iter().map(|h| outline_area(h)).sum();
        let more: f64 = extra.iter().map(|h| outline_area(&h.outline)).sum();
        (self.gross_area() - own - more).max(0.0)
    }

    /// Concrete in the slab body, cubic inches: net area times thickness.
    pub fn slab_volume(&self, extra: &[&SlabHole]) -> f64 {
        self.net_area(extra) * self.thickness
    }

    /// Length of footing: the outer edge when the slab has a footing, plus
    /// the edge of every `extra` hole that has one.
    pub fn footing_length(&self, extra: &[&SlabHole]) -> f64 {
        let outer = if self.footing.is_some() {
            self.perimeter()
        } else {
            0.0
        };
        let holes: f64 = extra
            .iter()
            .filter(|h| h.with_footing)
            .map(|h| outline_perimeter(&h.outline))
            .sum();
        outer + holes
    }

    /// Concrete in the footings, cubic inches (footing length times width
    /// times depth). Hole footings use this slab's footing size, or the
    /// default size when the slab has none.
    pub fn footing_volume(&self, extra: &[&SlabHole]) -> f64 {
        let f = self.footing.unwrap_or_default();
        self.footing_length(extra) * f.width * f.depth
    }

    /// Slab body plus footings, cubic yards.
    pub fn concrete_cu_yd(&self, extra: &[&SlabHole]) -> f64 {
        cu_in_to_cu_yd(self.slab_volume(extra) + self.footing_volume(extra))
    }

    pub fn translate(&mut self, d: Point) {
        translate_outline(&mut self.outline, d);
        for h in &mut self.holes {
            translate_outline(h, d);
        }
    }
}

/// A hole in the room's slab floor (Slab Hole, Slab Hole with Footing).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SlabHole {
    pub id: Id,
    pub outline: Vec<Point>,
    pub with_footing: bool,
    pub layer: String,
    pub line_style: LineStyle,
}

impl Default for SlabHole {
    fn default() -> Self {
        Self {
            id: 0,
            outline: Vec::new(),
            with_footing: false,
            layer: SLAB_LAYER.to_string(),
            line_style: LineStyle::Dashed,
        }
    }
}

impl SlabHole {
    pub fn new(id: Id, outline: Vec<Point>, with_footing: bool) -> Self {
        Self {
            id,
            outline,
            with_footing,
            ..Self::default()
        }
    }

    pub fn area(&self) -> f64 {
        outline_area(&self.outline)
    }

    pub fn translate(&mut self, d: Point) {
        translate_outline(&mut self.outline, d);
    }
}

/// An axis-aligned concrete box: a pad, or the footing under a pad or pier.
/// `bottom` and `top` are elevations relative to the floor, inches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxSolid {
    pub min: Point,
    pub max: Point,
    pub bottom: f64,
    pub top: f64,
}

impl BoxSolid {
    pub fn volume(&self) -> f64 {
        (self.max.x - self.min.x) * (self.max.y - self.min.y) * (self.top - self.bottom)
    }

    /// The four plan corners, counter-clockwise from the minimum corner.
    pub fn corners(&self) -> [Point; 4] {
        [
            self.min,
            Point::new(self.max.x, self.min.y),
            self.max,
            Point::new(self.min.x, self.max.y),
        ]
    }
}

/// A square pad (Square Pad).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pad {
    pub id: Id,
    pub center: Point,
    /// Side of the square, inches.
    pub size: f64,
    pub thickness: f64,
    /// Elevation of the top of the pad relative to the floor, inches.
    pub elevation: f64,
    pub material: String,
    pub layer: String,
}

impl Default for Pad {
    fn default() -> Self {
        Self {
            id: 0,
            center: Point::ZERO,
            size: DEFAULT_PAD_SIZE,
            thickness: DEFAULT_PAD_THICKNESS,
            elevation: 0.0,
            material: DEFAULT_MATERIAL.to_string(),
            layer: PIER_LAYER.to_string(),
        }
    }
}

impl Pad {
    pub fn new(id: Id, center: Point) -> Self {
        Self {
            id,
            center,
            ..Self::default()
        }
    }

    /// The pad as a box.
    pub fn solid(&self) -> BoxSolid {
        let h = self.size * 0.5;
        BoxSolid {
            min: Point::new(self.center.x - h, self.center.y - h),
            max: Point::new(self.center.x + h, self.center.y + h),
            bottom: self.elevation - self.thickness,
            top: self.elevation,
        }
    }

    /// The plan outline, counter-clockwise.
    pub fn outline(&self) -> Vec<Point> {
        self.solid().corners().to_vec()
    }

    pub fn volume(&self) -> f64 {
        self.solid().volume()
    }

    pub fn concrete_cu_yd(&self) -> f64 {
        cu_in_to_cu_yd(self.volume())
    }
}

/// A round pier (Round Pier).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pier {
    pub id: Id,
    pub center: Point,
    pub diameter: f64,
    pub height: f64,
    /// Elevation of the top of the pier relative to the floor, inches.
    pub elevation: f64,
    /// A square footing under the pier (`width` is its side).
    pub footing: Option<Footing>,
    pub material: String,
    pub layer: String,
}

impl Default for Pier {
    fn default() -> Self {
        Self {
            id: 0,
            center: Point::ZERO,
            diameter: DEFAULT_PIER_DIAMETER,
            height: DEFAULT_PIER_HEIGHT,
            elevation: 0.0,
            footing: None,
            material: DEFAULT_MATERIAL.to_string(),
            layer: PIER_LAYER.to_string(),
        }
    }
}

impl Pier {
    pub fn new(id: Id, center: Point) -> Self {
        Self {
            id,
            center,
            ..Self::default()
        }
    }

    /// Elevation of the underside of the shaft.
    pub fn bottom_elevation(&self) -> f64 {
        self.elevation - self.height
    }

    /// Volume of the cylindrical shaft, cubic inches.
    pub fn shaft_volume(&self) -> f64 {
        let r = self.diameter * 0.5;
        PI * r * r * self.height
    }

    /// The footing under the pier as a box, if it has one.
    pub fn footing_solid(&self) -> Option<BoxSolid> {
        let f = self.footing?;
        let h = f.width * 0.5;
        let top = self.bottom_elevation();
        Some(BoxSolid {
            min: Point::new(self.center.x - h, self.center.y - h),
            max: Point::new(self.center.x + h, self.center.y + h),
            bottom: top - f.depth,
            top,
        })
    }

    /// Shaft plus footing, cubic inches.
    pub fn volume(&self) -> f64 {
        self.shaft_volume() + self.footing_solid().map_or(0.0, |b| b.volume())
    }

    pub fn concrete_cu_yd(&self) -> f64 {
        cu_in_to_cu_yd(self.volume())
    }
}

/// Which platform a [`PlatformHole`] is cut through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum PlatformKind {
    #[default]
    Floor,
    Ceiling,
}

impl PlatformKind {
    pub fn name(self) -> &'static str {
        match self {
            PlatformKind::Floor => "Floor",
            PlatformKind::Ceiling => "Ceiling",
        }
    }

    /// The layer the dashed outline is drawn on.
    pub fn layer(self) -> &'static str {
        match self {
            PlatformKind::Floor => FLOOR_HOLES_LAYER,
            PlatformKind::Ceiling => CEILING_HOLES_LAYER,
        }
    }
}

/// A hole in the floor or ceiling platform (Hole in Floor/Ceiling Platform).
/// In 3D the platform slab mesh gets the hole; in plan it is a dashed outline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlatformHole {
    pub id: Id,
    pub outline: Vec<Point>,
    pub kind: PlatformKind,
    /// The object that owns the hole (a stair's Auto Stairwell), if any: the
    /// hole is removed with it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<Id>,
}

impl Default for PlatformHole {
    fn default() -> Self {
        Self {
            id: 0,
            outline: Vec::new(),
            kind: PlatformKind::Floor,
            owner: None,
        }
    }
}

impl PlatformHole {
    pub fn new(id: Id, outline: Vec<Point>, kind: PlatformKind) -> Self {
        Self {
            id,
            outline,
            kind,
            owner: None,
        }
    }

    /// A stairwell: a hole in the floor platform of the floor above, cut
    /// where the stair `stair` passes through (Auto Stairwell, CB-29).
    pub fn stairwell(id: Id, outline: Vec<Point>, stair: Id) -> Self {
        Self {
            id,
            outline,
            kind: PlatformKind::Floor,
            owner: Some(stair),
        }
    }

    pub fn layer(&self) -> &'static str {
        self.kind.layer()
    }

    pub fn area(&self) -> f64 {
        outline_area(&self.outline)
    }

    pub fn translate(&mut self, d: Point) {
        translate_outline(&mut self.outline, d);
    }
}

/// Addresses one object of a [`FoundationLayer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FoundationRef {
    Slab(Id),
    SlabHole(Id),
    Pad(Id),
    Pier(Id),
    PlatformHole(Id),
}

impl FoundationRef {
    pub fn id(self) -> Id {
        match self {
            FoundationRef::Slab(i)
            | FoundationRef::SlabHole(i)
            | FoundationRef::Pad(i)
            | FoundationRef::Pier(i)
            | FoundationRef::PlatformHole(i) => i,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            FoundationRef::Slab(_) => "Slab",
            FoundationRef::SlabHole(_) => "Slab Hole",
            FoundationRef::Pad(_) => "Square Pad",
            FoundationRef::Pier(_) => "Round Pier",
            FoundationRef::PlatformHole(_) => "Platform Hole",
        }
    }
}

/// Everything the Slab tools and the platform hole tools put on one floor.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FoundationLayer {
    pub slabs: Vec<Slab>,
    pub holes: Vec<SlabHole>,
    pub pads: Vec<Pad>,
    pub piers: Vec<Pier>,
    pub platform_holes: Vec<PlatformHole>,
}

impl FoundationLayer {
    pub fn is_empty(&self) -> bool {
        self.slabs.is_empty()
            && self.holes.is_empty()
            && self.pads.is_empty()
            && self.piers.is_empty()
            && self.platform_holes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.slabs.len()
            + self.holes.len()
            + self.pads.len()
            + self.piers.len()
            + self.platform_holes.len()
    }

    // ----- storage -----

    /// The layer stored on `floor` (empty when it has none or the data does
    /// not parse). Reads the legacy CAD record when the slot is empty.
    pub fn load(floor: &Floor) -> Self {
        match &floor.foundation {
            Some(v) => serde_json::from_value(v.clone()).unwrap_or_default(),
            None => floor
                .cad
                .iter()
                .find_map(record_json)
                .and_then(|json| serde_json::from_str(json).ok())
                .unwrap_or_default(),
        }
    }

    /// Stores the layer on `floor`, replacing what was there (and dropping a
    /// legacy record). An empty layer clears the slot.
    pub fn store(&self, floor: &mut Floor) {
        floor.cad.retain(|c| record_json(c).is_none());
        // Plain data always serializes; keep the old slot on the impossible error.
        floor.foundation = if self.is_empty() {
            None
        } else {
            match serde_json::to_value(self) {
                Ok(v) => Some(v),
                Err(_) => floor.foundation.take(),
            }
        };
    }

    // ----- lookup -----

    /// The object with this id, if any.
    pub fn find(&self, id: Id) -> Option<FoundationRef> {
        if self.slabs.iter().any(|s| s.id == id) {
            Some(FoundationRef::Slab(id))
        } else if self.holes.iter().any(|s| s.id == id) {
            Some(FoundationRef::SlabHole(id))
        } else if self.pads.iter().any(|s| s.id == id) {
            Some(FoundationRef::Pad(id))
        } else if self.piers.iter().any(|s| s.id == id) {
            Some(FoundationRef::Pier(id))
        } else if self.platform_holes.iter().any(|s| s.id == id) {
            Some(FoundationRef::PlatformHole(id))
        } else {
            None
        }
    }

    pub fn slab(&self, id: Id) -> Option<&Slab> {
        self.slabs.iter().find(|s| s.id == id)
    }

    pub fn slab_mut(&mut self, id: Id) -> Option<&mut Slab> {
        self.slabs.iter_mut().find(|s| s.id == id)
    }

    pub fn hole(&self, id: Id) -> Option<&SlabHole> {
        self.holes.iter().find(|s| s.id == id)
    }

    pub fn hole_mut(&mut self, id: Id) -> Option<&mut SlabHole> {
        self.holes.iter_mut().find(|s| s.id == id)
    }

    pub fn pad(&self, id: Id) -> Option<&Pad> {
        self.pads.iter().find(|s| s.id == id)
    }

    pub fn pad_mut(&mut self, id: Id) -> Option<&mut Pad> {
        self.pads.iter_mut().find(|s| s.id == id)
    }

    pub fn pier(&self, id: Id) -> Option<&Pier> {
        self.piers.iter().find(|s| s.id == id)
    }

    pub fn pier_mut(&mut self, id: Id) -> Option<&mut Pier> {
        self.piers.iter_mut().find(|s| s.id == id)
    }

    pub fn platform_hole(&self, id: Id) -> Option<&PlatformHole> {
        self.platform_holes.iter().find(|s| s.id == id)
    }

    pub fn platform_hole_mut(&mut self, id: Id) -> Option<&mut PlatformHole> {
        self.platform_holes.iter_mut().find(|s| s.id == id)
    }

    /// The layer the object is drawn on.
    pub fn layer_of(&self, r: FoundationRef) -> Option<String> {
        match r {
            FoundationRef::Slab(i) => self.slab(i).map(|s| s.layer.clone()),
            FoundationRef::SlabHole(i) => self.hole(i).map(|s| s.layer.clone()),
            FoundationRef::Pad(i) => self.pad(i).map(|s| s.layer.clone()),
            FoundationRef::Pier(i) => self.pier(i).map(|s| s.layer.clone()),
            FoundationRef::PlatformHole(i) => self.platform_hole(i).map(|s| s.layer().to_string()),
        }
    }

    // ----- editing -----

    /// Removes the object; returns whether it existed.
    pub fn remove(&mut self, r: FoundationRef) -> bool {
        fn drop_id<T>(v: &mut Vec<T>, id: Id, get: impl Fn(&T) -> Id) -> bool {
            let n = v.len();
            v.retain(|x| get(x) != id);
            v.len() != n
        }
        match r {
            FoundationRef::Slab(i) => drop_id(&mut self.slabs, i, |s| s.id),
            FoundationRef::SlabHole(i) => drop_id(&mut self.holes, i, |s| s.id),
            FoundationRef::Pad(i) => drop_id(&mut self.pads, i, |s| s.id),
            FoundationRef::Pier(i) => drop_id(&mut self.piers, i, |s| s.id),
            FoundationRef::PlatformHole(i) => drop_id(&mut self.platform_holes, i, |s| s.id),
        }
    }

    /// Moves the object by `d`; returns whether it existed.
    pub fn translate(&mut self, r: FoundationRef, d: Point) -> bool {
        match r {
            FoundationRef::Slab(i) => self.slab_mut(i).map(|s| s.translate(d)).is_some(),
            FoundationRef::SlabHole(i) => self.hole_mut(i).map(|s| s.translate(d)).is_some(),
            FoundationRef::Pad(i) => self.pad_mut(i).map(|s| s.center = s.center + d).is_some(),
            FoundationRef::Pier(i) => self.pier_mut(i).map(|s| s.center = s.center + d).is_some(),
            FoundationRef::PlatformHole(i) => {
                self.platform_hole_mut(i).map(|s| s.translate(d)).is_some()
            }
        }
    }

    // ----- derived geometry -----

    /// The slab holes that fall inside `slab`.
    pub fn holes_in<'a>(&'a self, slab: &Slab) -> Vec<&'a SlabHole> {
        self.holes
            .iter()
            .filter(|h| hole_inside(&h.outline, &slab.outline))
            .collect()
    }

    /// Every hole outline cut out of `slab`: its own and the layer's slab
    /// holes inside it.
    pub fn all_hole_outlines(&self, slab: &Slab) -> Vec<Vec<Point>> {
        slab.holes
            .iter()
            .cloned()
            .chain(self.holes_in(slab).into_iter().map(|h| h.outline.clone()))
            .collect()
    }

    /// The platform hole owned by `owner` (a stair's stairwell), if any.
    pub fn owned_hole(&self, owner: Id) -> Option<&PlatformHole> {
        self.platform_holes.iter().find(|h| h.owner == Some(owner))
    }

    /// Removes every platform hole owned by `owner`; returns how many went.
    pub fn remove_owned(&mut self, owner: Id) -> usize {
        let n = self.platform_holes.len();
        self.platform_holes.retain(|h| h.owner != Some(owner));
        n - self.platform_holes.len()
    }

    /// Outlines of the platform holes of one kind.
    pub fn platform_hole_outlines(&self, kind: PlatformKind) -> Vec<Vec<Point>> {
        self.platform_holes
            .iter()
            .filter(|h| h.kind == kind)
            .map(|h| h.outline.clone())
            .collect()
    }

    /// Concrete of one slab, with the layer's holes taken out, cubic yards.
    pub fn slab_concrete_cu_yd(&self, slab: &Slab) -> f64 {
        slab.concrete_cu_yd(&self.holes_in(slab))
    }

    /// All concrete on the floor (slabs, footings, pads, piers), cubic yards.
    pub fn total_concrete_cu_yd(&self) -> f64 {
        self.slabs
            .iter()
            .map(|s| self.slab_concrete_cu_yd(s))
            .sum::<f64>()
            + self.pads.iter().map(Pad::concrete_cu_yd).sum::<f64>()
            + self.piers.iter().map(Pier::concrete_cu_yd).sum::<f64>()
    }
}

/// Project-load step (no undo entry): moves every legacy `"Foundation, Data"`
/// record into `Floor.foundation` and drops the hidden layer. Floors whose
/// slot is already filled keep it (their stale record is removed). Returns
/// whether anything changed.
pub fn migrate_legacy(project: &mut Project) -> bool {
    let mut changed = false;
    for floor in &mut project.floors {
        let Some(i) = floor.cad.iter().position(|c| record_json(c).is_some()) else {
            continue;
        };
        if floor.foundation.is_none() {
            let layer = FoundationLayer::load(floor);
            layer.store(floor);
        } else {
            floor.cad.remove(i);
        }
        changed = true;
    }
    if !project
        .floors
        .iter()
        .any(|f| f.cad.iter().any(|c| c.layer == DATA_LAYER))
    {
        project.layers.layers.retain(|l| l.name != DATA_LAYER);
    }
    changed
}

impl Floor {
    /// The floor's foundation data as a typed object (`None` when unset).
    pub fn foundation_as<T: DeserializeOwned>(&self) -> Result<Option<T>, serde_json::Error> {
        self.foundation
            .as_ref()
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()
    }

    /// Replace the floor's foundation data.
    pub fn set_foundation<T: Serialize>(&mut self, value: &T) -> Result<(), serde_json::Error> {
        self.foundation = Some(serde_json::to_value(value)?);
        Ok(())
    }
}

fn record_json(c: &CadObject) -> Option<&str> {
    if c.layer != DATA_LAYER {
        return None;
    }
    match &c.item {
        CadItem::Text { text, .. } => text.strip_prefix(RECORD_TAG),
        _ => None,
    }
}

// ----- geometry helpers -----

/// Unsigned area of an outline, square inches.
pub fn outline_area(pts: &[Point]) -> f64 {
    polygon_area(pts).abs()
}

/// Length of a closed outline, inches.
pub fn outline_perimeter(pts: &[Point]) -> f64 {
    if pts.len() < 2 {
        return 0.0;
    }
    (0..pts.len())
        .map(|i| pts[i].dist(pts[(i + 1) % pts.len()]))
        .sum()
}

/// Moves every vertex by `d`.
pub fn translate_outline(pts: &mut [Point], d: Point) {
    for p in pts {
        *p = *p + d;
    }
}

/// The four corners (counter-clockwise) of the axis-aligned rectangle with
/// opposite corners `a` and `b`.
pub fn rect_outline(a: Point, b: Point) -> Vec<Point> {
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
    vec![
        Point::new(x0, y0),
        Point::new(x1, y0),
        Point::new(x1, y1),
        Point::new(x0, y1),
    ]
}

/// Is `hole` inside `outer`? Its centroid is inside and its bounds do not
/// leave the bounds of `outer` (a hole may touch the outline).
pub fn hole_inside(hole: &[Point], outer: &[Point]) -> bool {
    const TOL: f64 = 0.01;
    if hole.len() < 3 || outer.len() < 3 {
        return false;
    }
    let (hl, hh) = bounds(hole);
    let (ol, oh) = bounds(outer);
    hl.x >= ol.x - TOL
        && hl.y >= ol.y - TOL
        && hh.x <= oh.x + TOL
        && hh.y <= oh.y + TOL
        && point_in_polygon(polygon_centroid(hole), outer)
}

/// Bounding box `(min, max)` of the points.
pub fn bounds(pts: &[Point]) -> (Point, Point) {
    let Some(first) = pts.first() else {
        return (Point::ZERO, Point::ZERO);
    };
    pts.iter().fold((*first, *first), |(lo, hi), p| {
        (
            Point::new(lo.x.min(p.x), lo.y.min(p.y)),
            Point::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    })
}

/// Longest miter, as a multiple of the offset distance, before a corner is
/// cut square.
const MITER_LIMIT: f64 = 4.0;

/// A closed ring moved `d` inches to the left of its direction of travel:
/// into the material for a counter-clockwise outer ring or a clockwise hole
/// ring (negative `d` moves right). Corners are mitered, and cut square past
/// a miter of four times `d`.
pub fn offset_ring(ring: &[Point], d: f64) -> Vec<Point> {
    let n = ring.len();
    (0..n)
        .map(|i| {
            let prev = ring[(i + n - 1) % n];
            let cur = ring[i];
            let next = ring[(i + 1) % n];
            let n0 = (cur - prev).normalized().perp();
            let n1 = (next - cur).normalized().perp();
            let denom = 1.0 + n0.dot(n1);
            let shift = if denom < 2.0 / (MITER_LIMIT * MITER_LIMIT) {
                n0.scale(d)
            } else {
                (n0 + n1).scale(d / denom)
            };
            cur + shift
        })
        .collect()
}

/// Cubic inches to cubic yards.
pub fn cu_in_to_cu_yd(cu_in: f64) -> f64 {
    cu_in / CU_IN_PER_CU_YD
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, y: f64, s: f64) -> Vec<Point> {
        rect_outline(Point::new(x, y), Point::new(x + s, y + s))
    }

    fn sample() -> FoundationLayer {
        let mut slab = Slab::new(1, square(0.0, 0.0, 240.0));
        slab.footing = Some(Footing::default());
        slab.holes.push(square(10.0, 10.0, 20.0));
        FoundationLayer {
            slabs: vec![slab],
            holes: vec![SlabHole::new(2, square(100.0, 100.0, 40.0), true)],
            pads: vec![Pad::new(3, Point::new(300.0, 0.0))],
            piers: vec![{
                let mut p = Pier::new(4, Point::new(320.0, 40.0));
                p.footing = Some(Footing::default());
                p
            }],
            platform_holes: vec![
                PlatformHole::new(5, square(50.0, 50.0, 36.0), PlatformKind::Floor),
                PlatformHole::new(6, square(60.0, 60.0, 20.0), PlatformKind::Ceiling),
            ],
        }
    }

    #[test]
    fn defaults_match_chief() {
        let s = Slab::default();
        assert_eq!(s.thickness, 4.0);
        assert_eq!(s.top_elevation, 0.0);
        assert_eq!(s.layer, "Slabs");
        assert!(s.footing.is_none());
        let f = Footing::default();
        assert_eq!((f.width, f.depth), (16.0, 8.0));
        let p = Pad::default();
        assert_eq!((p.size, p.thickness), (24.0, 12.0));
        let r = Pier::default();
        assert_eq!((r.diameter, r.height), (12.0, 36.0));
        assert!(r.footing.is_none());
    }

    #[test]
    fn layer_round_trips_through_json_and_the_floor_slot() {
        let layer = sample();
        let back: FoundationLayer =
            serde_json::from_str(&serde_json::to_string(&layer).unwrap()).unwrap();
        assert_eq!(back, layer);

        let mut floor = Floor::new("1st", 0.0);
        assert!(FoundationLayer::load(&floor).is_empty());
        layer.store(&mut floor);
        assert!(
            floor.cad.is_empty(),
            "the typed slot replaces the CAD record"
        );
        assert!(floor.foundation.is_some());
        assert_eq!(FoundationLayer::load(&floor), layer);
        assert_eq!(
            floor.foundation_as::<FoundationLayer>().unwrap().unwrap(),
            layer
        );
        // Storing again replaces the data.
        let mut changed = layer.clone();
        changed.slabs[0].thickness = 6.0;
        changed.store(&mut floor);
        assert_eq!(FoundationLayer::load(&floor).slabs[0].thickness, 6.0);
        // An empty layer clears the slot.
        FoundationLayer::default().store(&mut floor);
        assert!(floor.foundation.is_none());
        // And survives a whole-floor serde round trip.
        layer.store(&mut floor);
        let floor2: Floor = serde_json::from_str(&serde_json::to_string(&floor).unwrap()).unwrap();
        assert_eq!(FoundationLayer::load(&floor2), layer);
        // Files written before the slot existed load with it empty.
        let mut v = serde_json::to_value(Floor::new("old", 0.0)).unwrap();
        v.as_object_mut().unwrap().remove("foundation");
        let old: Floor = serde_json::from_value(v).unwrap();
        assert!(old.foundation.is_none());
    }

    /// A floor holding `layer` the old way: one text record on the hidden layer.
    fn legacy_record(layer: &FoundationLayer) -> CadObject {
        CadObject {
            id: 0,
            layer: DATA_LAYER.to_string(),
            item: CadItem::Text {
                pos: Point::ZERO,
                text: format!("{RECORD_TAG}{}", serde_json::to_string(layer).unwrap()),
                height: 1.0,
                angle: 0.0,
            },
        }
    }

    #[test]
    fn legacy_cad_record_loads_and_migrates_into_the_slot() {
        let layer = sample();
        let mut project = Project::new("old");
        project.floors[0].cad.push(legacy_record(&layer));
        let mut data = crate::layers::Layer::new(DATA_LAYER, [150, 150, 150], 13);
        data.display = false;
        project.layers.layers.push(data);
        // Readable before the migration.
        assert_eq!(FoundationLayer::load(&project.floors[0]), layer);

        assert!(migrate_legacy(&mut project));
        let floor = &project.floors[0];
        assert!(floor.cad.is_empty());
        assert!(floor.foundation.is_some());
        assert_eq!(FoundationLayer::load(floor), layer);
        assert!(project.layers.get(DATA_LAYER).is_none());
        // Migrating again is a no-op.
        assert!(!migrate_legacy(&mut project));

        // A filled slot wins over a stale record.
        let mut other = layer.clone();
        other.pads.clear();
        project.floors[0].cad.push(legacy_record(&other));
        assert!(migrate_legacy(&mut project));
        assert_eq!(FoundationLayer::load(&project.floors[0]), layer);
        assert!(project.floors[0].cad.is_empty());
    }

    #[test]
    fn storing_drops_a_legacy_record() {
        let layer = sample();
        let mut floor = Floor::new("1st", 0.0);
        floor.cad.push(legacy_record(&layer));
        let mut changed = layer.clone();
        changed.slabs[0].thickness = 5.0;
        changed.store(&mut floor);
        assert!(floor.cad.is_empty());
        assert_eq!(FoundationLayer::load(&floor).slabs[0].thickness, 5.0);
    }

    #[test]
    fn old_and_partial_records_load_with_defaults() {
        let json = r#"{"slabs":[{"id":9,"outline":[{"x":0.0,"y":0.0},{"x":10.0,"y":0.0},{"x":10.0,"y":10.0}]}]}"#;
        let l: FoundationLayer = serde_json::from_str(json).unwrap();
        assert_eq!(l.slabs[0].thickness, DEFAULT_SLAB_THICKNESS);
        assert_eq!(l.slabs[0].layer, SLAB_LAYER);
        assert!(l.pads.is_empty() && l.platform_holes.is_empty());
    }

    #[test]
    fn slab_with_hole_volume_is_net_area_times_thickness() {
        let mut s = Slab::new(1, square(0.0, 0.0, 240.0));
        s.holes.push(square(10.0, 10.0, 20.0));
        let hole = SlabHole::new(2, square(100.0, 100.0, 40.0), false);
        let expect = (240.0 * 240.0 - 20.0 * 20.0 - 40.0 * 40.0) * 4.0;
        assert!((s.slab_volume(&[&hole]) - expect).abs() < 1e-6);
        assert!((s.net_area(&[&hole]) - (57_600.0 - 400.0 - 1_600.0)).abs() < 1e-6);
        // Orientation does not matter.
        let mut cw = s.clone();
        cw.outline.reverse();
        assert!((cw.slab_volume(&[&hole]) - expect).abs() < 1e-6);
    }

    #[test]
    fn footing_and_cubic_yards() {
        let l = sample();
        let s = &l.slabs[0];
        let holes = l.holes_in(s);
        assert_eq!(holes.len(), 1, "the slab hole at (100,100) is inside");
        // Outer 4 x 240" plus the footed hole's 4 x 40".
        assert!((s.footing_length(&holes) - (960.0 + 160.0)).abs() < 1e-9);
        assert!((s.footing_volume(&holes) - 1120.0 * 16.0 * 8.0).abs() < 1e-6);
        let body = (57_600.0 - 400.0 - 1_600.0) * 4.0;
        let total = body + 1120.0 * 128.0;
        assert!((l.slab_concrete_cu_yd(s) - total / 46_656.0).abs() < 1e-9);

        let pad = &l.pads[0];
        assert!((pad.volume() - 24.0 * 24.0 * 12.0).abs() < 1e-9);
        let pier = &l.piers[0];
        let shaft = PI * 36.0 * 36.0;
        assert!((pier.shaft_volume() - shaft).abs() < 1e-6);
        let fb = pier.footing_solid().unwrap();
        assert_eq!((fb.bottom, fb.top), (-44.0, -36.0));
        assert!((pier.volume() - (shaft + 16.0 * 16.0 * 8.0)).abs() < 1e-6);
        assert!(l.total_concrete_cu_yd() > l.slab_concrete_cu_yd(s));
    }

    #[test]
    fn offset_ring_moves_into_the_material() {
        let outer = square(0.0, 0.0, 100.0);
        let inner = offset_ring(&outer, 10.0);
        assert!((outline_area(&inner) - 80.0 * 80.0).abs() < 1e-9);
        let mut hole = square(40.0, 40.0, 20.0);
        hole.reverse();
        let grown = offset_ring(&hole, 5.0);
        assert!((outline_area(&grown) - 30.0 * 30.0).abs() < 1e-9);
    }

    #[test]
    fn a_hole_only_cuts_the_slab_it_is_inside() {
        let l = sample();
        let far = Slab::new(7, square(1000.0, 1000.0, 100.0));
        assert!(l.holes_in(&far).is_empty());
        assert_eq!(l.all_hole_outlines(&l.slabs[0]).len(), 2);
        assert_eq!(l.platform_hole_outlines(PlatformKind::Floor).len(), 1);
        assert_eq!(l.platform_hole_outlines(PlatformKind::Ceiling).len(), 1);
    }

    #[test]
    fn a_stairwell_hole_is_owned_by_its_stair() {
        let mut l = FoundationLayer::default();
        l.platform_holes
            .push(PlatformHole::stairwell(9, square(0.0, 0.0, 36.0), 4));
        l.platform_holes.push(PlatformHole::new(
            10,
            square(80.0, 0.0, 12.0),
            PlatformKind::Floor,
        ));
        assert_eq!(l.owned_hole(4).map(|h| h.id), Some(9));
        assert!((l.owned_hole(4).unwrap().area() - 36.0 * 36.0).abs() < 1e-9);
        // The owner survives a round trip; a plain hole stays ownerless.
        let json = serde_json::to_string(&l).unwrap();
        let back: FoundationLayer = serde_json::from_str(&json).unwrap();
        assert_eq!(back, l);
        assert_eq!(back.platform_holes[1].owner, None);
        assert_eq!(l.remove_owned(4), 1);
        assert_eq!(l.platform_holes.len(), 1);
        assert_eq!(l.remove_owned(4), 0);
    }

    #[test]
    fn find_translate_and_remove() {
        let mut l = sample();
        assert_eq!(l.len(), 6);
        assert_eq!(l.find(3), Some(FoundationRef::Pad(3)));
        assert_eq!(l.find(99), None);
        let d = Point::new(10.0, -5.0);
        for id in 1..=6 {
            let r = l.find(id).unwrap();
            assert!(l.translate(r, d));
        }
        assert_eq!(l.pad(3).unwrap().center, Point::new(310.0, -5.0));
        assert_eq!(l.slab(1).unwrap().outline[0], Point::new(10.0, -5.0));
        assert_eq!(l.slab(1).unwrap().holes[0][0], Point::new(20.0, 5.0));
        assert_eq!(
            l.layer_of(FoundationRef::PlatformHole(6)).as_deref(),
            Some("Ceilings, Holes")
        );
        assert_eq!(
            l.layer_of(FoundationRef::PlatformHole(5)).as_deref(),
            Some("Floors, Holes")
        );
        for id in 1..=6 {
            assert!(l.remove(l.find(id).unwrap()));
        }
        assert!(l.is_empty());
        assert!(!l.translate(FoundationRef::Pad(3), d));
    }
}
