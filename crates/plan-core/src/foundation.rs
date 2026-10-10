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
use crate::object_pages::{LabelPage, SchedulePage};
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

fn is_default_label(l: &LabelPage) -> bool {
    *l == LabelPage::default()
}

fn is_default_schedule(s: &SchedulePage) -> bool {
    *s == SchedulePage::default()
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
    /// Footing Offset: how far the footing reaches out past the edges of the
    /// slab; 0 puts the outside of the footing under the slab's edge.
    pub footing_offset: f64,
    /// What Top and Bottom of the Slab Specification are measured from
    /// (Elevation Reference); the slab itself is stored floor-relative.
    pub elevation_base: crate::elevation_ref::ElevationBase,
    pub material: String,
    /// Holes cut into this slab by its own specification.
    pub holes: Vec<Vec<Point>>,
    pub layer: String,
    /// Plan fill color.
    pub fill_color: [u8; 3],
    /// Plan fill pattern name (`None`, `Solid`, `Hatch`, `Cross Hatch`, `Grid`).
    pub fill_pattern: String,
    pub line_style: LineStyle,
    /// The Label panel (Chief's Label tab); defaults are not stored.
    #[serde(skip_serializing_if = "is_default_label")]
    pub label: LabelPage,
    /// The Schedule panel (Chief's Schedule tab); defaults are not stored.
    #[serde(skip_serializing_if = "is_default_schedule")]
    pub schedule: SchedulePage,
}

impl Default for Slab {
    fn default() -> Self {
        Self {
            id: 0,
            outline: Vec::new(),
            thickness: DEFAULT_SLAB_THICKNESS,
            top_elevation: 0.0,
            footing: None,
            footing_offset: 0.0,
            elevation_base: crate::elevation_ref::ElevationBase::FromFloor,
            material: DEFAULT_MATERIAL.to_string(),
            holes: Vec::new(),
            layer: SLAB_LAYER.to_string(),
            fill_color: [190, 190, 185],
            fill_pattern: "Hatch".to_string(),
            line_style: LineStyle::Solid,
            label: LabelPage::default(),
            schedule: SchedulePage::default(),
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
    /// Material of the hole's edge (Materials tab).
    pub material: String,
    /// The Label panel (Chief's Label tab); defaults are not stored.
    #[serde(skip_serializing_if = "is_default_label")]
    pub label: LabelPage,
    /// The Schedule panel (Chief's Schedule tab); defaults are not stored.
    #[serde(skip_serializing_if = "is_default_schedule")]
    pub schedule: SchedulePage,
}

impl Default for SlabHole {
    fn default() -> Self {
        Self {
            id: 0,
            outline: Vec::new(),
            with_footing: false,
            layer: SLAB_LAYER.to_string(),
            line_style: LineStyle::Dashed,
            material: DEFAULT_MATERIAL.to_string(),
            label: LabelPage::default(),
            schedule: SchedulePage::default(),
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
    /// The Label panel (Chief's Label tab); defaults are not stored.
    #[serde(skip_serializing_if = "is_default_label")]
    pub label: LabelPage,
    /// The Schedule panel (Chief's Schedule tab); defaults are not stored.
    #[serde(skip_serializing_if = "is_default_schedule")]
    pub schedule: SchedulePage,
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
            label: LabelPage::default(),
            schedule: SchedulePage::default(),
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
    /// The Label panel (Chief's Label tab); defaults are not stored.
    #[serde(skip_serializing_if = "is_default_label")]
    pub label: LabelPage,
    /// The Schedule panel (Chief's Schedule tab); defaults are not stored.
    #[serde(skip_serializing_if = "is_default_schedule")]
    pub schedule: SchedulePage,
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
            label: LabelPage::default(),
            schedule: SchedulePage::default(),
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
    pub material: String,
    /// Layer picked on the Layer tab; empty means the layer of `kind`.
    // TODO parity: nothing draws or hides by this layer yet.
    pub layer: String,
    /// The Label panel (Chief's Label tab); defaults are not stored.
    #[serde(skip_serializing_if = "is_default_label")]
    pub label: LabelPage,
    /// The Schedule panel (Chief's Schedule tab); defaults are not stored.
    #[serde(skip_serializing_if = "is_default_schedule")]
    pub schedule: SchedulePage,
}

impl Default for PlatformHole {
    fn default() -> Self {
        Self {
            id: 0,
            outline: Vec::new(),
            kind: PlatformKind::Floor,
            owner: None,
            material: DEFAULT_MATERIAL.to_string(),
            layer: String::new(),
            label: LabelPage::default(),
            schedule: SchedulePage::default(),
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
            ..Self::default()
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
            ..Self::default()
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
            Some(v) => crate::foreign::read_layer(v),
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
        // Records this build cannot read stay in the slot (QA-28).
        floor.foundation =
            crate::foreign::layer_slot(self, self.is_empty(), floor.foundation.as_ref());
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

// ===================================================================
// Foundation Defaults and Options (manual pp. 738-741)
// ===================================================================

/// Piers under a Grade Beams on Piers foundation are Round Piers or Square
/// Pads (Foundation Defaults, Piers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PierShape {
    #[default]
    Round,
    Square,
}

impl PierShape {
    pub const ALL: [PierShape; 2] = [PierShape::Round, PierShape::Square];

    pub fn name(self) -> &'static str {
        match self {
            PierShape::Round => "Round",
            PierShape::Square => "Square",
        }
    }
}

/// Rebar of one foundation component (Options panel): bars per course, the
/// spacing of the courses, the bar size in eighths of an inch (4 is 1/2 in)
/// and the lap where sticks meet, in bar diameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rebar {
    pub bars: u32,
    pub spacing: f64,
    pub size: u32,
    pub overlap: f64,
}

impl Default for Rebar {
    fn default() -> Self {
        Self {
            bars: 2,
            spacing: 24.0,
            size: 4,
            overlap: 40.0,
        }
    }
}

impl Rebar {
    /// Diameter of the bar, inches.
    pub fn diameter(&self) -> f64 {
        f64::from(self.size.max(1)) / 8.0
    }

    /// Length of rebar for a run `run` inches long, counting the lap at each
    /// 20-foot stick, inches.
    pub fn run_length(&self, run: f64) -> f64 {
        const STICK: f64 = 240.0;
        let sticks = (run / STICK).ceil().max(1.0);
        run + (sticks - 1.0) * self.overlap * self.diameter()
    }
}

/// The Rebar group of the Options panel: footing, wall horizontal courses,
/// wall vertical courses, pier and slab, and mesh for the slab instead of
/// rebar.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RebarSet {
    pub footing: Rebar,
    pub wall_horizontal: Rebar,
    pub wall_vertical: Rebar,
    pub pier: Rebar,
    pub slab: Rebar,
    pub use_mesh: bool,
}

impl Default for RebarSet {
    fn default() -> Self {
        Self {
            footing: Rebar {
                bars: 2,
                spacing: 0.0,
                ..Rebar::default()
            },
            wall_horizontal: Rebar {
                bars: 1,
                spacing: 32.0,
                ..Rebar::default()
            },
            wall_vertical: Rebar {
                bars: 1,
                spacing: 32.0,
                ..Rebar::default()
            },
            pier: Rebar {
                bars: 4,
                spacing: 0.0,
                ..Rebar::default()
            },
            slab: Rebar {
                bars: 1,
                spacing: 18.0,
                ..Rebar::default()
            },
            use_mesh: true,
        }
    }
}

/// Default Garage Floor to Stem Wall Top, inches: the garage slab sits this
/// far below the top of its stem walls (the platform plus 12 in puts it
/// there, manual p. 748).
pub const GARAGE_FLOOR_TO_STEM_TOP: f64 = 12.0;
/// Default Lower Garage Floor of a Monolithic Slab, inches (3 1/2 in).
pub const LOWER_GARAGE_FLOOR: f64 = 3.5;
/// Default Minimum Garage Height, inches: the least height of the stem walls
/// of a garage foundation (manual p. 749).
pub const MIN_GARAGE_HEIGHT: f64 = 24.0;
/// Default Minimum Height of foundation stem walls and grade beams, inches.
pub const MIN_STEM_HEIGHT: f64 = 12.0;
/// The terrain sits this far below the top of stem walls and grade beams,
/// inches (manual p. 749).
pub const TERRAIN_BELOW_STEM_TOP: f64 = 6.0;
/// The terrain sits this far below the top of a monolithic slab, inches.
pub const TERRAIN_BELOW_SLAB_TOP: f64 = 8.0;
/// Walls whose heights differ by less than this are not a step, inches
/// (1/16 in, manual p. 746).
pub const STEP_TOLERANCE: f64 = 1.0 / 16.0;

/// Everything in the Foundation Defaults and Build Foundation dialogs that is
/// not already a field of [`crate::floors::FoundationOptions`] (manual pp.
/// 739-741): Auto Rebuild, Hang 1st Floor Platform Inside Foundation Walls,
/// S markers, Slab at Top of Stem Wall, the minimum height, the piers, the
/// Garage Options and the Options panel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FoundationSettings {
    /// Rebuild the foundation whenever Floor 1 changes in a way that affects
    /// it; floor 0 is then locked against hand editing.
    pub auto_rebuild: bool,
    /// Stem walls build up to the top of Floor 1's platform instead of
    /// stopping under it (Walls with Footings only).
    pub hang_platform: bool,
    /// Show an "S" in the plan wherever the wall height steps.
    pub s_markers: bool,
    /// Walls with Footings: the slab floor is flush with the stem wall tops
    /// and every room of Floor 1 takes its floor from the foundation.
    pub slab_at_stem_top: bool,
    /// Minimum height of stem walls and grade beams, inches.
    pub min_height: f64,
    /// A Round Pier's diameter or a Square Pad's side, inches.
    pub pier_width: f64,
    pub pier_shape: PierShape,
    /// Garage Options. `garage_floor` is the Build Foundation dialog's
    /// Garage Floor box: garage and slab rooms get their own slab and curbs.
    pub garage_floor: bool,
    pub garage_floor_to_stem_top: f64,
    pub lower_garage_floor: f64,
    pub min_garage_height: f64,
    /// Stepped stem walls get vertical footings.
    pub vertical_step_footings: bool,
    /// Chamfer width and height of monolithic slab footings, inches.
    pub chamfer_width: f64,
    pub chamfer_height: f64,
    pub rebar: RebarSet,
    pub foam_seal: bool,
    pub termite_flashing: bool,
}

impl Default for FoundationSettings {
    fn default() -> Self {
        Self {
            auto_rebuild: false,
            hang_platform: false,
            s_markers: true,
            slab_at_stem_top: false,
            min_height: MIN_STEM_HEIGHT,
            pier_width: DEFAULT_PIER_DIAMETER,
            pier_shape: PierShape::Round,
            garage_floor: true,
            garage_floor_to_stem_top: GARAGE_FLOOR_TO_STEM_TOP,
            lower_garage_floor: LOWER_GARAGE_FLOOR,
            min_garage_height: MIN_GARAGE_HEIGHT,
            vertical_step_footings: false,
            chamfer_width: 4.0,
            chamfer_height: 4.0,
            rebar: RebarSet::default(),
            foam_seal: false,
            termite_flashing: false,
        }
    }
}

impl FoundationSettings {
    /// How far a garage or slab room whose floor height is 0 is lowered when
    /// a Walls with Footings or Grade Beams on Piers foundation is built: the
    /// floor platform plus the Garage Floor to Stem Wall Top, inches (the
    /// slab then ends up that far under the stem wall top, which is the
    /// platform's underside).
    pub fn garage_drop(&self, platform: f64) -> f64 {
        platform + self.garage_floor_to_stem_top
    }
}

/// Where the wall heights of a foundation step: a point shared by two
/// foundation walls whose tops differ (Show "S" Markers on Step Foundation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepMarker {
    /// The shared end point.
    pub at: Point,
    /// The lower and the higher wall top at the step, inches above the floor.
    pub low: f64,
    pub high: f64,
    /// The taller wall (the marker sits along it).
    pub tall_wall: Id,
}

/// The steps in the foundation walls of `floor`, one per point where two
/// foundation walls meet at different top heights.
pub fn step_markers(floor: &Floor) -> Vec<StepMarker> {
    let walls: Vec<&crate::model::Wall> = floor
        .walls
        .iter()
        .filter(|w| w.flags.foundation && !w.flags.invisible && w.length() > 1e-6)
        .collect();
    let top = |w: &crate::model::Wall| w.bottom_offset + w.height;
    let mut out: Vec<StepMarker> = Vec::new();
    for (i, a) in walls.iter().enumerate() {
        for b in &walls[i + 1..] {
            for pa in [a.start, a.end] {
                for pb in [b.start, b.end] {
                    if pa.dist(pb) > 0.5 || (top(a) - top(b)).abs() < STEP_TOLERANCE {
                        continue;
                    }
                    if out.iter().any(|m| m.at.dist(pa) < 0.5) {
                        continue;
                    }
                    let (tall, low, high) = if top(a) > top(b) {
                        (a, top(b), top(a))
                    } else {
                        (b, top(a), top(b))
                    };
                    out.push(StepMarker {
                        at: pa,
                        low,
                        high,
                        tall_wall: tall.id,
                    });
                }
            }
        }
    }
    out
}

/// Where the "S" of a step is drawn: a little way along the taller wall from
/// the step and to its left, so it does not sit on the wall lines.
pub fn step_marker_label_at(floor: &Floor, m: &StepMarker, offset: f64) -> Point {
    let Some(w) = floor.walls.iter().find(|w| w.id == m.tall_wall) else {
        return m.at;
    };
    let toward = if m.at.dist(w.start) <= m.at.dist(w.end) {
        w.direction()
    } else {
        w.direction() * -1.0
    };
    m.at + toward * offset + toward.perp() * (w.thickness * 0.5 + offset)
}

/// The points of a pier run along a wall from `start` to `end`: both ends and
/// as many between as keep every span at most `max_separation` long.
pub fn pier_positions(start: Point, end: Point, max_separation: f64) -> Vec<Point> {
    let len = start.dist(end);
    let n = (len / max_separation.max(12.0)).ceil().max(1.0) as usize;
    let mut out: Vec<Point> = (0..=n)
        .map(|i| Point::lerp(start, end, i as f64 / n as f64))
        .collect();
    out.dedup_by(|a, b| a.dist(*b) < 1.0);
    out
}

/// The width of the cutout a door leaves in a garage stem wall or curb (the
/// Rough Opening panel's Add for Concrete Cutout; the rough opening itself
/// when the door asks for none).
pub fn garage_cut_width(op: &crate::model::Opening) -> f64 {
    op.concrete_cutout_width()
        .unwrap_or_else(|| op.rough_width())
}

/// Where the automatic terrain sits under a plan with a foundation: 6 in
/// below the tops of the stem walls or grade beams, 8 in below the top of a
/// monolithic slab (manual p. 749). Absolute elevation, inches; `None` when
/// the plan has no foundation floor.
pub fn terrain_elevation(project: &Project) -> Option<f64> {
    let f0 = project
        .floors
        .first()
        .filter(|f| f.kind == crate::floors::FloorKind::Foundation)?;
    let first = project
        .floors
        .iter()
        .find(|f| f.kind == crate::floors::FloorKind::Normal)?;
    let platform = first.settings.floor_structure_thickness;
    let opts = f0.settings.foundation_options;
    let kind = opts
        .map(|o| o.kind)
        .or(f0.settings.foundation.map(|b| b.kind))?;
    Some(match kind {
        crate::floors::FoundationKind::MonolithicSlab => first.elevation - TERRAIN_BELOW_SLAB_TOP,
        _ => {
            let gap = opts.map_or(0.0, |o| o.top_gap(platform));
            first.elevation - gap - TERRAIN_BELOW_STEM_TOP
        }
    })
}

/// One line of the foundation take-off for the Materials List (Options
/// panel): rebar, mesh, foam seal and termite flashing. The program does not
/// draw them in any view.
#[derive(Debug, Clone, PartialEq)]
pub struct TakeoffRow {
    pub item: String,
    pub quantity: f64,
    pub unit: &'static str,
}

/// Rebar and the rest of the Options panel for the foundation floor of
/// `project`, as feet of bar (and square feet of mesh), in the order footing,
/// wall horizontal, wall vertical, pier, slab, then foam seal and termite
/// flashing in linear feet. Empty when the foundation was not built with
/// options.
pub fn foundation_takeoff(project: &Project) -> Vec<TakeoffRow> {
    let Some(floor) = project
        .floors
        .iter()
        .find(|f| f.kind == crate::floors::FloorKind::Foundation)
    else {
        return Vec::new();
    };
    let Some(opts) = floor.settings.foundation_options else {
        return Vec::new();
    };
    let s = opts.settings;
    let walls: Vec<&crate::model::Wall> = floor
        .walls
        .iter()
        .filter(|w| w.flags.foundation && !w.flags.invisible)
        .collect();
    let length: f64 = walls.iter().map(|w| w.length()).sum();
    let mut rows = Vec::new();
    let mut add = |item: &str, inches: f64, unit: &'static str, per: f64| {
        if inches > 0.0 {
            rows.push(TakeoffRow {
                item: item.to_string(),
                quantity: inches / per,
                unit,
            });
        }
    };
    let footed = floor
        .settings
        .foundation
        .is_some_and(|b| b.footing_width > 0.0);
    if footed {
        let r = s.rebar.footing;
        let each: f64 = walls.iter().map(|w| r.run_length(w.length())).sum();
        add("Footing rebar", each * f64::from(r.bars), "ft", 12.0);
    }
    let rh = s.rebar.wall_horizontal;
    let rv = s.rebar.wall_vertical;
    let mut horizontal = 0.0;
    let mut vertical = 0.0;
    for w in &walls {
        let courses = (w.height / rh.spacing.max(1.0)).floor().max(1.0);
        horizontal += rh.run_length(w.length()) * f64::from(rh.bars) * courses;
        let uprights = (w.length() / rv.spacing.max(1.0)).ceil() + 1.0;
        vertical += rv.run_length(w.height) * f64::from(rv.bars) * uprights;
    }
    add("Wall horizontal rebar", horizontal, "ft", 12.0);
    add("Wall vertical rebar", vertical, "ft", 12.0);
    let layer = FoundationLayer::load(floor);
    let piers = layer.piers.len() + layer.pads.len();
    if piers > 0 {
        let r = s.rebar.pier;
        let height = layer
            .piers
            .iter()
            .map(|p| p.height)
            .chain(layer.pads.iter().map(|p| p.thickness))
            .fold(0.0, f64::max);
        add(
            "Pier rebar",
            r.run_length(height) * f64::from(r.bars) * piers as f64,
            "ft",
            12.0,
        );
    }
    let slab_area: f64 = layer.slabs.iter().map(|sl| sl.net_area(&[])).sum();
    if slab_area > 0.0 {
        if s.rebar.use_mesh {
            add("Slab mesh", slab_area, "sq ft", 144.0);
        } else {
            let r = s.rebar.slab;
            // A grid of bars at the spacing, both ways.
            let grid = 2.0 * slab_area / r.spacing.max(1.0);
            add("Slab rebar", grid * f64::from(r.bars), "ft", 12.0);
        }
    }
    if s.foam_seal {
        add("Foam seal", length, "ft", 12.0);
    }
    if s.termite_flashing {
        add("Termite flashing", length, "ft", 12.0);
    }
    rows
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

    fn foundation_wall(id: Id, a: (f64, f64), b: (f64, f64), top: f64) -> crate::model::Wall {
        let mut w = crate::model::Wall::new(
            Point::new(a.0, a.1),
            Point::new(b.0, b.1),
            8.0,
            top,
            crate::model::WallKind::Exterior,
        );
        w.id = id;
        w.flags.foundation = true;
        w
    }

    #[test]
    fn a_step_is_where_foundation_walls_of_different_tops_meet() {
        let mut f = Floor::new("Foundation", -48.0);
        f.walls = vec![
            foundation_wall(1, (0.0, 0.0), (100.0, 0.0), 36.0),
            foundation_wall(2, (100.0, 0.0), (200.0, 0.0), 60.0),
            // A third wall at the same point and the same top as the first.
            foundation_wall(3, (100.0, 0.0), (100.0, 80.0), 36.0),
            // A wall that differs by less than 1/16 in is no step.
            foundation_wall(4, (200.0, 0.0), (200.0, 80.0), 60.03),
        ];
        let marks = step_markers(&f);
        assert_eq!(marks.len(), 1, "{marks:?}");
        let m = marks[0];
        assert_eq!(m.at, Point::new(100.0, 0.0));
        assert_eq!((m.low, m.high, m.tall_wall), (36.0, 60.0, 2));
        // The S sits beside the taller wall, off the wall lines.
        let at = step_marker_label_at(&f, &m, 6.0);
        assert!(at.x > 100.0 + 5.0 && at.y.abs() > 4.0, "{at:?}");
        // A bottom that steps counts as the top it makes.
        f.walls[0].bottom_offset = 24.0;
        f.walls[0].height = 12.0;
        assert_eq!(step_markers(&f).len(), 1);
        f.walls[0].height = 24.0;
        assert_eq!(step_markers(&f).len(), 1, "48 against 60 still steps");
    }

    #[test]
    fn piers_are_no_further_apart_than_the_maximum_separation() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(100.0, 0.0);
        assert_eq!(pier_positions(a, b, 100.0).len(), 2);
        assert_eq!(pier_positions(a, b, 99.0).len(), 3, "two spans of 50");
        let many = pier_positions(a, b, 12.0);
        assert!(many.windows(2).all(|w| w[0].dist(w[1]) <= 12.0 + 1e-9));
        // Less than a foot is a foot.
        assert_eq!(pier_positions(a, b, 1.0).len(), 10);
    }

    #[test]
    fn the_garage_cut_is_the_rough_opening_plus_each_side() {
        let mut o =
            crate::model::Opening::new(1, 60.0, crate::model::OpeningKind::Door, 108.0, 84.0, 0.0);
        assert_eq!(garage_cut_width(&o), 108.0);
        o.extras.spec.rough.add_width = 4.0;
        assert_eq!(garage_cut_width(&o), 112.0);
        o.extras.spec.rough.concrete_each_side = 3.0;
        assert_eq!(garage_cut_width(&o), 118.0);
    }

    #[test]
    fn rebar_laps_each_20_foot_stick() {
        let r = Rebar::default();
        assert_eq!(r.diameter(), 0.5);
        assert_eq!(r.run_length(100.0), 100.0);
        // 25 ft is two sticks: one lap of 40 bar diameters (20 in).
        assert_eq!(r.run_length(300.0), 320.0);
    }
}
