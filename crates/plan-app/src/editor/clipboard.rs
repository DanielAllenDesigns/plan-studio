//! The editor clipboard (S-81..S-84, S-93): what Cut and Copy take, and how
//! Paste, Paste Hold Position and Duplicate put it back.
//!
//! A [`Clipboard`] is a snapshot of the copied objects; it belongs to no
//! floor, so it pastes onto any floor. Kinds copied: walls with their
//! openings, dimensions, CAD and text (with style extras and whole CAD
//! blocks), cameras, cabinets, library symbols, stairs and landings,
//! electrical devices, manual framing, slabs, holes, pads, piers and platform
//! holes, moldings, deck polygons, floor material regions, 3D solids,
//! schedules and terrain elements. Not copied: roof planes, ceilings and
//! dormers (they are rebuilt from the walls), the terrain perimeter, corner
//! boards, quoins, wall material regions and wall hatching (they belong to a
//! wall's geometry), and rooms.
//!
//! Every pasted object gets a fresh id; groups wholly inside the copied set
//! come back as groups.
//!
//! The clipboard also lives in a file, `~/.plan-studio/clipboard.json` (S-85):
//! a Plan Studio plan holding only the copied objects. A Copy writes it, and a
//! running copy of the program that sees a newer file takes it as its
//! clipboard ([`poll_file`]), so a Copy in one plan pastes into another plan
//! or another window. The layers the objects sit on travel with them by name;
//! a layer the destination lacks is made when the objects land.

use super::framing_view::{self, Record};
use super::selection::ObjectRef;
use super::{
    details_view, foundation_view, ops, placed, schedule_view, site_view, stairs_view,
    EditorContext,
};
use plan_cabinets::Cabinet;
use plan_core::cad::{CadAttrs, CadBlockInfo};
use plan_core::details::{
    DeckPolygon, DetailsLayer, MaterialRegion, MoldingLine, RegionKind, Solid3d,
};
use plan_core::foundation::{FoundationLayer, Pad, Pier, PlatformHole, Slab, SlabHole};
use plan_core::geometry::Point;
use plan_core::groups::ObjectRef as CoreRef;
use plan_core::schedules::{Schedule, ScheduleLayer};
use plan_core::{CadObject, CameraObject, Dimension, Id, Opening, PlacedSymbol, Wall};
use plan_electrical::Device;
use plan_terrain::{
    ElevationLine, ElevationPoint, ElevationRegion, Feature, Landscape, Modifier, RoadStrip,
    Terrain, TerrainBreak, TerrainWall,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

/// A copied element of the terrain (the perimeter is the whole terrain and
/// is not copied).
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum TerrainElem {
    Point(ElevationPoint),
    Line(ElevationLine),
    Region(ElevationRegion),
    Modifier(Modifier),
    Feature(Feature),
    Road(RoadStrip),
    Break(TerrainBreak),
    Wall(TerrainWall),
    Landscape(Landscape),
}

impl TerrainElem {
    /// The element at `hit` of `t`.
    fn take(t: &Terrain, hit: site_view::TerrainHit) -> Option<TerrainElem> {
        use site_view::TerrainHit as H;
        Some(match hit {
            H::Perimeter => return None,
            H::Point(i) => TerrainElem::Point(t.elevation_points.get(i)?.clone()),
            H::Line(i) => TerrainElem::Line(t.elevation_lines.get(i)?.clone()),
            H::Region(i) => TerrainElem::Region(t.elevation_regions.get(i)?.clone()),
            H::Modifier(i) => TerrainElem::Modifier(t.modifiers.get(i)?.clone()),
            H::Feature(i) => TerrainElem::Feature(t.features.get(i)?.clone()),
            H::Road(i) => TerrainElem::Road(t.roads.get(i)?.clone()),
            H::Break(i) => TerrainElem::Break(t.breaks.get(i)?.clone()),
            H::Wall(i) => TerrainElem::Wall(t.walls.get(i)?.clone()),
            H::Landscape(i) => TerrainElem::Landscape(t.landscape.get(i)?.clone()),
        })
    }

    /// Appends a copy to `t`; returns where it landed.
    fn push(&self, t: &mut Terrain) -> site_view::TerrainHit {
        use site_view::TerrainHit as H;
        match self {
            TerrainElem::Point(e) => {
                t.elevation_points.push(e.clone());
                H::Point(t.elevation_points.len() - 1)
            }
            TerrainElem::Line(e) => {
                t.elevation_lines.push(e.clone());
                H::Line(t.elevation_lines.len() - 1)
            }
            TerrainElem::Region(e) => {
                t.elevation_regions.push(e.clone());
                H::Region(t.elevation_regions.len() - 1)
            }
            TerrainElem::Modifier(e) => {
                t.modifiers.push(e.clone());
                H::Modifier(t.modifiers.len() - 1)
            }
            TerrainElem::Feature(e) => {
                t.features.push(e.clone());
                H::Feature(t.features.len() - 1)
            }
            TerrainElem::Road(e) => {
                t.roads.push(e.clone());
                H::Road(t.roads.len() - 1)
            }
            TerrainElem::Break(e) => {
                t.breaks.push(e.clone());
                H::Break(t.breaks.len() - 1)
            }
            TerrainElem::Wall(e) => {
                t.walls.push(e.clone());
                H::Wall(t.walls.len() - 1)
            }
            TerrainElem::Landscape(e) => {
                t.landscape.push(e.clone());
                H::Landscape(t.landscape.len() - 1)
            }
        }
    }

    /// The plan points that outline the element.
    fn points(&self) -> Vec<Point> {
        let mut t = Terrain {
            perimeter: Vec::new(),
            ..Terrain::default()
        };
        let hit = self.push(&mut t);
        site_view::hit_points(&t, hit)
    }
}

/// One copied object of a kind that is stored as its own record.
#[derive(Clone, Debug)]
pub enum ClipItem {
    Cabinet(Box<Cabinet>),
    Symbol(Box<PlacedSymbol>),
    Stair(Box<stairs_view::StairObj>),
    Device(Box<Device>),
    Framing(Box<Record>),
    Slab(Box<Slab>),
    SlabHole(Box<SlabHole>),
    Pad(Box<Pad>),
    Pier(Box<Pier>),
    PlatformHole(Box<PlatformHole>),
    Molding(Box<MoldingLine>),
    Region(Box<MaterialRegion>),
    Deck(Box<DeckPolygon>),
    Solid(Box<Solid3d>),
    Schedule(Box<Schedule>),
    Terrain(Box<TerrainElem>),
}

impl ClipItem {
    /// The points that bound the object in plan.
    pub fn points(&self) -> Vec<Point> {
        match self {
            ClipItem::Cabinet(c) => c.corners().to_vec(),
            ClipItem::Symbol(s) => s.footprint().to_vec(),
            ClipItem::Stair(s) => s.footprint(),
            ClipItem::Device(d) => vec![d.position],
            ClipItem::Framing(r) => r.extent(),
            ClipItem::Slab(s) => s.outline.clone(),
            ClipItem::SlabHole(h) => h.outline.clone(),
            ClipItem::Pad(p) => p.outline(),
            ClipItem::Pier(p) => {
                let r = p.diameter * 0.5;
                vec![
                    Point::new(p.center.x - r, p.center.y - r),
                    Point::new(p.center.x + r, p.center.y + r),
                ]
            }
            ClipItem::PlatformHole(h) => h.outline.clone(),
            ClipItem::Molding(m) => m.polyline.clone(),
            ClipItem::Region(r) => r.outline.clone(),
            ClipItem::Deck(d) => d.outline.clone(),
            ClipItem::Solid(s) => vec![s.position],
            ClipItem::Schedule(s) => vec![s.position],
            ClipItem::Terrain(e) => e.points(),
        }
    }

    /// Chief's name for the type of the object.
    pub fn type_name(&self) -> &'static str {
        match self {
            ClipItem::Terrain(_) => "Terrain Element",
            other => other.old_ref().type_name(),
        }
    }

    fn old_ref(&self) -> ObjectRef {
        match self {
            ClipItem::Cabinet(c) => ObjectRef::Cabinet(c.id),
            ClipItem::Symbol(s) => ObjectRef::Symbol(s.id),
            ClipItem::Stair(s) => ObjectRef::Stair(s.id()),
            ClipItem::Device(d) => ObjectRef::Device(d.id),
            ClipItem::Framing(r) => ObjectRef::Framing(r.id()),
            ClipItem::Slab(s) => ObjectRef::Foundation(s.id),
            ClipItem::SlabHole(h) => ObjectRef::Foundation(h.id),
            ClipItem::Pad(p) => ObjectRef::Foundation(p.id),
            ClipItem::Pier(p) => ObjectRef::Foundation(p.id),
            ClipItem::PlatformHole(h) => ObjectRef::Foundation(h.id),
            ClipItem::Molding(m) => ObjectRef::Detail(m.id),
            ClipItem::Region(r) => ObjectRef::Detail(r.id),
            ClipItem::Deck(d) => ObjectRef::Detail(d.id),
            ClipItem::Solid(s) => ObjectRef::Detail(s.id),
            ClipItem::Schedule(s) => ObjectRef::Schedule(s.id),
            // Terrain elements have no ids; the reference is never looked up.
            ClipItem::Terrain(_) => ObjectRef::Terrain,
        }
    }
}

/// Objects copied with Copy or Cut; pasted back with new ids.
#[derive(Clone, Debug, Default)]
pub struct Clipboard {
    pub walls: Vec<Wall>,
    pub openings: Vec<Opening>,
    pub dimensions: Vec<Dimension>,
    pub cad: Vec<CadObject>,
    /// Style extras of the copied CAD objects (targets are the old ids).
    pub cad_attrs: Vec<CadAttrs>,
    /// CAD blocks whose every object was copied, with the old member ids.
    pub cad_blocks: Vec<(CadBlockInfo, Vec<Id>)>,
    pub cameras: Vec<CameraObject>,
    /// Cabinets, symbols, stairs, devices, framing, foundation objects,
    /// details and schedules.
    pub items: Vec<ClipItem>,
    /// Groups wholly inside the copied set, by the old references.
    pub groups: Vec<Vec<CoreRef>>,
    /// Types of selected objects the clipboard cannot take (roof planes,
    /// terrain elements, wall trim, rooms).
    pub skipped: Vec<&'static str>,
    /// The layers the copied objects sit on, so a paste into another plan can
    /// make the ones it lacks (S-85).
    pub layers: Vec<plan_core::Layer>,
}

impl Clipboard {
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        self.walls.len()
            + self.dimensions.len()
            + self.cad.len()
            + self.cameras.len()
            + self.items.len()
            // An opening counts on its own only when its wall did not come.
            + self
                .openings
                .iter()
                .filter(|o| !self.walls.iter().any(|w| w.id == o.wall_id))
                .count()
    }

    /// Drops the objects a rotation or mirror cannot turn (roof, framing,
    /// foundation, details, schedules, terrain), noting their types in
    /// [`skipped`](Self::skipped).
    pub fn drop_unturnable(&mut self) {
        let mut names: Vec<&'static str> = Vec::new();
        self.items.retain(|i| {
            let keep = matches!(
                i,
                ClipItem::Cabinet(_)
                    | ClipItem::Symbol(_)
                    | ClipItem::Stair(_)
                    | ClipItem::Device(_)
            );
            if !keep {
                let n = i.type_name();
                if !names.contains(&n) {
                    names.push(n);
                }
            }
            keep
        });
        for n in names {
            if !self.skipped.contains(&n) {
                self.skipped.push(n);
            }
        }
    }

    /// Takes the selection of `cx` (the active floor).
    pub fn capture(cx: &EditorContext) -> Clipboard {
        let f = cx.floor();
        let mut clip = Clipboard::default();
        for o in &cx.selection.items {
            match *o {
                ObjectRef::Wall(id) => {
                    if let Some(w) = f.wall(id) {
                        if !clip.walls.iter().any(|x| x.id == id) {
                            clip.walls.push(w.clone());
                            for op in f.openings_on(id) {
                                if !clip.openings.iter().any(|x| x.id == op.id) {
                                    clip.openings.push(op.clone());
                                }
                            }
                        }
                    }
                }
                ObjectRef::Opening(id) => {
                    if let Some(op) = f.openings.iter().find(|x| x.id == id) {
                        if !clip.openings.iter().any(|x| x.id == id) {
                            clip.openings.push(op.clone());
                        }
                    }
                }
                ObjectRef::Dimension(id) => {
                    clip.dimensions
                        .extend(f.dimensions.iter().find(|d| d.id == id).cloned());
                }
                ObjectRef::Cad(id) | ObjectRef::Text(id) => {
                    if !clip.cad.iter().any(|c| c.id == id) {
                        clip.cad.extend(f.cad.iter().find(|c| c.id == id).cloned());
                    }
                }
                ObjectRef::Camera(id) => {
                    clip.cameras.extend(cx.project.camera(id).cloned());
                }
                other => match capture_item(cx, other) {
                    Some(item) => clip.items.push(item),
                    None => {
                        let name = other.type_name();
                        if !clip.skipped.contains(&name) {
                            clip.skipped.push(name);
                        }
                    }
                },
            }
        }
        // A distribution's generated copies come back with their record.
        let owners: Vec<Id> = clip
            .items
            .iter()
            .filter_map(|i| match i {
                ClipItem::Symbol(s) if s.distribution.is_some() => Some(s.id),
                _ => None,
            })
            .collect();
        clip.items.retain(|i| match i {
            ClipItem::Symbol(s) => s.owner.is_none_or(|o| !owners.contains(&o)),
            _ => true,
        });
        // CAD style extras and whole CAD blocks travel with their objects.
        for c in &clip.cad {
            clip.cad_attrs.extend(f.cad_attrs(c.id));
        }
        for b in f.cad_blocks() {
            let members = f.group_members_cad(b.group);
            if members.len() >= 2 && members.iter().all(|m| clip.cad.iter().any(|c| c.id == *m)) {
                clip.cad_blocks.push((b, members));
            }
        }
        // Groups wholly inside the copied set (CAD blocks are groups too, but
        // they come back through `cad_blocks`).
        let copied: Vec<CoreRef> = cx
            .selection
            .items
            .iter()
            .filter_map(|o| o.to_group_ref())
            .collect();
        for g in &f.groups {
            let is_block = f.cad_block(g.id).is_some();
            if !is_block && g.members.len() >= 2 && g.members.iter().all(|m| copied.contains(m)) {
                clip.groups.push(g.members.clone());
            }
        }
        // The layers of everything copied (an opening rides on its wall's).
        for o in &cx.selection.items {
            if let Some(name) = super::selection::layer_of(f, *o) {
                if clip.layers.iter().all(|l| l.name != name) {
                    if let Some(l) = cx.project.layers.get(&name) {
                        clip.layers.push(l.clone());
                    }
                }
            }
        }
        clip
    }

    /// Every point of the copied objects (their center is the reference
    /// point a pasted group hangs on).
    pub fn points(&self) -> Vec<Point> {
        let mut pts = Vec::new();
        for w in &self.walls {
            pts.extend(w.footprint());
        }
        for d in &self.dimensions {
            let (p, q) = d.line_points();
            pts.extend([d.start, d.end, p, q]);
        }
        for c in &self.cad {
            let (lo, hi) = c.bounds();
            pts.extend([lo, hi]);
        }
        pts.extend(self.cameras.iter().map(|c| c.position));
        for i in &self.items {
            pts.extend(i.points());
        }
        pts
    }

    /// The center of the box around everything copied.
    pub fn center(&self) -> Option<Point> {
        plan_core::transform::bounds_of(self.points())
            .map(|(lo, hi)| Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5))
    }

    /// Line segments that sketch the copied objects (the ghost that follows
    /// the pointer): walls as their footprints, everything else as the box
    /// around it.
    pub fn ghost_segments(&self) -> Vec<(Point, Point)> {
        let mut out = Vec::new();
        let ring = |pts: &[Point], out: &mut Vec<(Point, Point)>| {
            for i in 0..pts.len() {
                out.push((pts[i], pts[(i + 1) % pts.len()]));
            }
        };
        for w in &self.walls {
            ring(&w.footprint(), &mut out);
        }
        for d in &self.dimensions {
            out.push(d.line_points());
        }
        for c in &self.cad {
            let (lo, hi) = c.bounds();
            ring(
                &[lo, Point::new(hi.x, lo.y), hi, Point::new(lo.x, hi.y)],
                &mut out,
            );
        }
        for i in &self.items {
            if let Some((lo, hi)) = plan_core::transform::bounds_of(i.points()) {
                ring(
                    &[lo, Point::new(hi.x, lo.y), hi, Point::new(lo.x, hi.y)],
                    &mut out,
                );
            }
        }
        out
    }

    /// Pastes onto the active floor of `cx`, every object shifted by
    /// `offset`. The caller opens the undo step. Returns the new objects (the
    /// pasted CAD blocks and groups are made too); with `as_group` they also
    /// become one group.
    pub fn paste(&self, cx: &mut EditorContext, offset: Point, as_group: bool) -> Vec<ObjectRef> {
        let fl = cx.floor;
        let mut sel: Vec<ObjectRef> = Vec::new();
        let mut map: HashMap<ObjectRef, ObjectRef> = HashMap::new();
        let mut wall_map: HashMap<Id, Id> = HashMap::new();
        for w in &self.walls {
            let id = cx.project.alloc_id();
            let mut copy = w.clone();
            copy.id = id;
            copy.start = copy.start + offset;
            copy.end = copy.end + offset;
            cx.project.floors[fl].walls.push(copy);
            wall_map.insert(w.id, id);
            map.insert(ObjectRef::Wall(w.id), ObjectRef::Wall(id));
            sel.push(ObjectRef::Wall(id));
        }
        for o in &self.openings {
            let Some(host) = wall_map.get(&o.wall_id) else {
                continue;
            };
            let id = cx.project.alloc_id();
            let mut copy = o.clone();
            copy.id = id;
            copy.wall_id = *host;
            cx.project.floors[fl].openings.push(copy);
            if let Some(extras) = cx.extras.openings.get(&o.id).cloned() {
                cx.extras.openings.insert(id, extras);
            }
            map.insert(ObjectRef::Opening(o.id), ObjectRef::Opening(id));
        }
        // The copies are free until the objects they were tied to are known
        // (DIM-38, below).
        let mut pasted_dims: Vec<(Id, [Option<plan_core::dim_assoc::DimAnchor>; 2])> = Vec::new();
        for d in &self.dimensions {
            let mut copy = d.clone();
            copy.anchors = [None, None];
            copy.translate(offset);
            let id = cx.project.add_dimension(fl, copy);
            pasted_dims.push((id, d.anchors));
            map.insert(ObjectRef::Dimension(d.id), ObjectRef::Dimension(id));
            sel.push(ObjectRef::Dimension(id));
        }
        let mut cad_map: HashMap<Id, Id> = HashMap::new();
        for c in &self.cad {
            let mut item = c.item.clone();
            ops::translate_cad(&mut item, offset);
            let id = cx.project.add_cad(fl, c.layer.clone(), item);
            cad_map.insert(c.id, id);
            map.insert(ObjectRef::Cad(c.id), ObjectRef::Cad(id));
            sel.push(ObjectRef::Cad(id));
        }
        for a in &self.cad_attrs {
            if let Some(&id) = cad_map.get(&a.target) {
                let mut copy = a.clone();
                copy.target = id;
                cx.project.set_cad_attrs(fl, copy);
            }
        }
        for (info, members) in &self.cad_blocks {
            let refs: Vec<CoreRef> = members
                .iter()
                .filter_map(|m| cad_map.get(m))
                .map(|id| CoreRef::Cad(*id))
                .collect();
            if let Some(group) = cx.project.make_group(fl, &refs) {
                let mut copy = info.clone();
                copy.group = group;
                cx.project.floors[fl].cad_blocks.push(copy);
            }
        }
        for c in &self.cameras {
            let mut copy = c.clone();
            copy.position = copy.position + offset;
            for p in copy.path.iter_mut() {
                *p = *p + offset;
            }
            copy.floor = fl;
            let id = cx.project.add_camera(copy);
            map.insert(ObjectRef::Camera(c.id), ObjectRef::Camera(id));
            sel.push(ObjectRef::Camera(id));
        }
        let mut moved: Vec<ObjectRef> = Vec::new();
        let mut has_distribution = false;
        for item in &self.items {
            if let Some(new) = insert_item(cx, item, &wall_map, offset) {
                if matches!(item, ClipItem::Symbol(s) if s.distribution.is_some()) {
                    has_distribution = true;
                }
                map.insert(item.old_ref(), new);
                sel.push(new);
                moved.push(new);
            }
        }
        // DIM-38: a dimension pasted with every object it was tied to stays
        // tied to the copies (it follows them); if any of those was not
        // copied it pastes free.
        // Strings stay strings among the copies; curved dimensions follow the
        // copies of their walls.
        let pairs: Vec<(Id, Id)> = self
            .dimensions
            .iter()
            .filter_map(|d| match map.get(&ObjectRef::Dimension(d.id)) {
                Some(ObjectRef::Dimension(n)) => Some((d.id, *n)),
                _ => None,
            })
            .collect();
        cx.project.floors[fl].repair_pasted_dimensions(&pairs, &|w| wall_map.get(&w).copied());
        for (id, old) in pasted_dims {
            let anchors = retie_anchors(&old, &map, offset);
            if let Some(d) = cx.project.floors[fl]
                .dimensions
                .iter_mut()
                .find(|d| d.id == id)
            {
                d.anchors = anchors;
            }
        }
        // Records inserted at their old place move to the drop point.
        if !moved.is_empty() && offset.length() > 1e-9 {
            cx.translate_extra(&moved, offset);
        }
        // Groups wholly inside the copied set come back as groups.
        for g in &self.groups {
            let members: Vec<CoreRef> = g
                .iter()
                .filter_map(|m| map.get(&ObjectRef::from_group_ref(*m)))
                .filter_map(|o| o.to_group_ref())
                .collect();
            if members.len() >= 2 {
                cx.project.make_group(fl, &members);
            }
        }
        if as_group {
            let members: Vec<CoreRef> = sel.iter().filter_map(|o| o.to_group_ref()).collect();
            cx.project.make_group(fl, &members);
        }
        // Pasted distribution records rebuild their copies.
        if has_distribution {
            placed::sync_distributions(cx);
        }
        self.ensure_layers(cx);
        cx.mark_dirty();
        sel
    }

    /// Makes the layers of the copied objects that the plan lacks (S-85).
    /// Layers it has keep their own settings. Returns the names made.
    pub fn ensure_layers(&self, cx: &mut EditorContext) -> Vec<String> {
        let mut made = Vec::new();
        for l in &self.layers {
            if cx.project.layers.get(&l.name).is_none() {
                let mut copy = l.clone();
                copy.display = true;
                copy.locked = false;
                cx.project.layers.layers.push(copy);
                made.push(l.name.clone());
            }
        }
        made
    }
}

/// The record behind a selection entry of an extra kind.
fn capture_item(cx: &EditorContext, o: ObjectRef) -> Option<ClipItem> {
    let f = cx.floor();
    Some(match o {
        ObjectRef::Cabinet(id) => ClipItem::Cabinet(Box::new(placed::cabinet_by_id(f, id)?)),
        // A fireplace carries its specification with the copy.
        ObjectRef::Symbol(id) => ClipItem::Symbol(Box::new(f.symbol_for_copy(f.symbol(id)?))),
        ObjectRef::Stair(id) => ClipItem::Stair(Box::new(stairs_view::find(f, id)?)),
        ObjectRef::Device(id) => {
            let layer = super::site_view::electrical_layer(cx.floor, f);
            ClipItem::Device(Box::new(layer.device(id)?.clone()))
        }
        ObjectRef::Framing(id) => {
            let rec = framing_view::find(f, id)?;
            // A built member becomes a placed one, so a rebuild keeps it.
            ClipItem::Framing(Box::new(match rec {
                Record::Built(m) => Record::Manual(m),
                other => other,
            }))
        }
        ObjectRef::Foundation(id) => {
            let layer = FoundationLayer::load(f);
            if let Some(s) = layer.slabs.iter().find(|x| x.id == id) {
                ClipItem::Slab(Box::new(s.clone()))
            } else if let Some(h) = layer.holes.iter().find(|x| x.id == id) {
                ClipItem::SlabHole(Box::new(h.clone()))
            } else if let Some(p) = layer.pads.iter().find(|x| x.id == id) {
                ClipItem::Pad(Box::new(p.clone()))
            } else if let Some(p) = layer.piers.iter().find(|x| x.id == id) {
                ClipItem::Pier(Box::new(p.clone()))
            } else {
                let h = layer.platform_holes.iter().find(|x| x.id == id)?;
                // A stairwell belongs to its stair; the copy stands alone.
                let mut h = h.clone();
                h.owner = None;
                ClipItem::PlatformHole(Box::new(h))
            }
        }
        ObjectRef::Detail(id) => {
            let layer = DetailsLayer::load(f);
            if let Some(m) = layer.molding(id) {
                ClipItem::Molding(Box::new(m.clone()))
            } else if let Some(d) = layer.deck(id) {
                ClipItem::Deck(Box::new(d.clone()))
            } else if let Some(s) = layer.solid(id) {
                ClipItem::Solid(Box::new(s.clone()))
            } else {
                // Only floor regions: a wall region, hatch, corner board or
                // quoin belongs to its wall.
                let r = layer
                    .region(id)
                    .filter(|r| matches!(r.kind, RegionKind::Floor))?;
                ClipItem::Region(Box::new(r.clone()))
            }
        }
        ObjectRef::Schedule(id) => {
            let layer = ScheduleLayer::load(f);
            ClipItem::Schedule(Box::new(layer.find(id)?.clone()))
        }
        ObjectRef::TerrainObject(hit) => {
            let rec = site_view::load_terrain(&cx.project)?;
            ClipItem::Terrain(Box::new(TerrainElem::take(&rec.terrain, hit)?))
        }
        _ => return None,
    })
}

/// Stores a copy of `item` on the active floor under a fresh id, at its
/// original place, and returns the new reference.
/// The anchors of a pasted dimension: each tied end moved to the copy of its
/// object, or both ends free when an object it was tied to was not copied.
fn retie_anchors(
    old: &[Option<plan_core::dim_assoc::DimAnchor>; 2],
    map: &HashMap<ObjectRef, ObjectRef>,
    offset: Point,
) -> [Option<plan_core::dim_assoc::DimAnchor>; 2] {
    use plan_core::dim_assoc::AnchorTarget;
    let mut out = [None, None];
    for (k, a) in old.iter().enumerate() {
        let Some(a) = a else { continue };
        let was = match a.target {
            AnchorTarget::Wall => ObjectRef::Wall(a.wall),
            AnchorTarget::Opening => ObjectRef::Opening(a.wall),
            AnchorTarget::Cabinet => ObjectRef::Cabinet(a.wall),
            AnchorTarget::Symbol => ObjectRef::Symbol(a.wall),
            AnchorTarget::Cad => ObjectRef::Cad(a.wall),
            AnchorTarget::Stair => ObjectRef::Stair(a.wall),
            AnchorTarget::Device => ObjectRef::Device(a.wall),
        };
        let now = match map.get(&was) {
            Some(
                ObjectRef::Wall(id)
                | ObjectRef::Opening(id)
                | ObjectRef::Cabinet(id)
                | ObjectRef::Symbol(id)
                | ObjectRef::Cad(id)
                | ObjectRef::Stair(id)
                | ObjectRef::Device(id),
            ) => *id,
            _ => return [None, None],
        };
        let mut copy = *a;
        copy.wall = now;
        copy.last = copy.last + offset;
        out[k] = Some(copy);
    }
    out
}

fn insert_item(
    cx: &mut EditorContext,
    item: &ClipItem,
    wall_map: &HashMap<Id, Id>,
    offset: Point,
) -> Option<ObjectRef> {
    let fl = cx.floor;
    match item {
        ClipItem::Cabinet(c) => {
            placed::add_cabinet(&mut cx.project, fl, (**c).clone()).map(ObjectRef::Cabinet)
        }
        ClipItem::Symbol(s) => {
            // A pasted copy of a distributed object stands alone; a pasted
            // distribution record makes its own copies.
            let mut s = (**s).clone();
            s.owner = None;
            let record = s.distribution.is_some();
            let id = cx.project.add_symbol(fl, s);
            if record {
                cx.project.rebuild_distribution(fl, id);
            }
            Some(ObjectRef::Symbol(id))
        }
        ClipItem::Stair(s) => {
            let mut s = (**s).clone();
            // The stairwell the original cut is not the copy's.
            s.x.stairwell_walls.clear();
            s.x.stairwell_hole = None;
            s.x.stairwell_guard = false;
            s.x.guard_walls.clear();
            Some(ObjectRef::Stair(stairs_view::add(&mut cx.project, fl, s)))
        }
        ClipItem::Device(d) => {
            let mut d = (**d).clone();
            d.id = 0;
            d.switched_by.clear();
            d.circuit = None;
            d.wall_id = d
                .wall_id
                .and_then(|w| wall_map.get(&w).copied())
                .or_else(|| {
                    // Pasted in place it keeps its wall.
                    (offset.length() < 1e-9).then_some(d.wall_id).flatten()
                });
            let mut layer = super::site_view::load_electrical(&cx.project.floors[fl]);
            let id = layer.add(d);
            super::site_view::save_electrical(&mut cx.project, fl, &layer);
            Some(ObjectRef::Device(id))
        }
        ClipItem::Framing(r) => {
            let id = cx.project.alloc_id();
            let rec = with_record_id((**r).clone(), id);
            let mut records = framing_view::load_records(&cx.project.floors[fl]);
            records.push(rec);
            framing_view::store_records(&mut cx.project.floors[fl], &records);
            framing_view::ensure_manual_layers(&mut cx.project);
            Some(ObjectRef::Framing(id))
        }
        ClipItem::Slab(_)
        | ClipItem::SlabHole(_)
        | ClipItem::Pad(_)
        | ClipItem::Pier(_)
        | ClipItem::PlatformHole(_) => {
            let id = cx.project.alloc_id();
            let mut layer = FoundationLayer::load(&cx.project.floors[fl]);
            match item {
                ClipItem::Slab(s) => layer.slabs.push(Slab {
                    id,
                    ..(**s).clone()
                }),
                ClipItem::SlabHole(h) => layer.holes.push(SlabHole {
                    id,
                    ..(**h).clone()
                }),
                ClipItem::Pad(p) => layer.pads.push(Pad {
                    id,
                    ..(**p).clone()
                }),
                ClipItem::Pier(p) => layer.piers.push(Pier {
                    id,
                    ..(**p).clone()
                }),
                ClipItem::PlatformHole(h) => layer.platform_holes.push(PlatformHole {
                    id,
                    ..(**h).clone()
                }),
                _ => unreachable!(),
            }
            foundation_view::save(&mut cx.project, fl, &layer);
            Some(ObjectRef::Foundation(id))
        }
        ClipItem::Molding(_) | ClipItem::Region(_) | ClipItem::Deck(_) | ClipItem::Solid(_) => {
            let id = cx.project.alloc_id();
            let mut layer = DetailsLayer::load(&cx.project.floors[fl]);
            match item {
                ClipItem::Molding(m) => layer.moldings.push(MoldingLine {
                    id,
                    ..(**m).clone()
                }),
                ClipItem::Region(r) => layer.regions.push(MaterialRegion {
                    id,
                    ..(**r).clone()
                }),
                ClipItem::Deck(d) => layer.decks.push(DeckPolygon {
                    id,
                    ..(**d).clone()
                }),
                ClipItem::Solid(s) => layer.solids.push(Solid3d {
                    id,
                    ..(**s).clone()
                }),
                _ => unreachable!(),
            }
            details_view::save(&mut cx.project, fl, &layer);
            Some(ObjectRef::Detail(id))
        }
        ClipItem::Schedule(s) => {
            let id = cx.project.alloc_id();
            let mut layer = ScheduleLayer::load(&cx.project.floors[fl]);
            layer.add(Schedule {
                id,
                ..(**s).clone()
            });
            schedule_view::save(&mut cx.project, fl, &layer);
            Some(ObjectRef::Schedule(id))
        }
        ClipItem::Terrain(e) => {
            // The terrain belongs to the plan, not to a floor.
            let mut rec = site_view::load_terrain(&cx.project).unwrap_or_default();
            let hit = e.push(&mut rec.terrain);
            site_view::save_terrain(&mut cx.project, &rec);
            Some(ObjectRef::TerrainObject(hit))
        }
    }
}

/// `rec` with the object id `id`.
fn with_record_id(mut rec: Record, id: Id) -> Record {
    match &mut rec {
        Record::Manual(m) | Record::Built(m) => m.id = id,
        Record::JoistDirection { id: i, .. }
        | Record::TrussDirection { id: i, .. }
        | Record::BearingLine { id: i, .. }
        | Record::Marker { id: i, .. }
        | Record::TrussBase { id: i, .. } => *i = id,
    }
    rec
}

thread_local! {
    /// Set when the clipboard was just filled; the shell then leaves a note
    /// on the system clipboard (see [`take_system_clipboard_note`]).
    static NOTE_PENDING: Cell<bool> = const { Cell::new(false) };
}

/// The text left on the system clipboard after a Copy or Cut.
pub const SYSTEM_NOTE: &str = "Plan Studio objects";

/// True once after the editor clipboard was filled. egui sends a Paste
/// event for Cmd+V only while the system clipboard holds some text, so the
/// shell writes [`SYSTEM_NOTE`] there whenever this says yes.
pub fn take_system_clipboard_note() -> bool {
    NOTE_PENDING.with(|n| n.replace(false))
}

impl EditorContext {
    /// Makes `clip` the clipboard.
    pub(super) fn store_clipboard(&mut self, clip: Clipboard) {
        write_file(self, &clip);
        self.clipboard = Some(clip);
        NOTE_PENDING.with(|n| n.set(true));
    }
}

// ----- the clipboard file (S-85) -----

/// The file name under `~/.plan-studio`.
pub const FILE_NAME: &str = "clipboard.json";
const FORMAT: &str = "plan-studio-clipboard";

thread_local! {
    /// Where the clipboard file is (tests point it into a scratch folder; the
    /// outer `Option` says whether it was set).
    static PATH_OVERRIDE: RefCell<Option<Option<PathBuf>>> = const { RefCell::new(None) };
    /// The modified time of the file as this process last wrote or read it.
    static SEEN: RefCell<Option<SystemTime>> = const { RefCell::new(None) };
    static LAST_POLL: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Points the clipboard file somewhere else (`Some(path)`), turns it off
/// (`None`) or back to the default (`reset_file_path`).
pub fn set_file_path(path: Option<PathBuf>) {
    PATH_OVERRIDE.with(|p| *p.borrow_mut() = Some(path));
    SEEN.with(|s| *s.borrow_mut() = None);
}

pub fn reset_file_path() {
    PATH_OVERRIDE.with(|p| *p.borrow_mut() = None);
    SEEN.with(|s| *s.borrow_mut() = None);
}

fn file_path() -> Option<PathBuf> {
    if let Some(o) = PATH_OVERRIDE.with(|p| p.borrow().clone()) {
        return o;
    }
    // Tests never touch the real home folder.
    if cfg!(test) {
        return None;
    }
    crate::paths::user_file(FILE_NAME)
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// A plan that holds exactly the copied objects, as JSON text.
fn scratch_json(cx: &EditorContext, clip: &Clipboard) -> Option<String> {
    let mut scratch = plan_core::Project::from_defaults("Clipboard", &cx.defaults);
    scratch.layers = cx.project.layers.clone();
    scratch.wall_types = cx.project.wall_types.clone();
    let mut sc = EditorContext::with_project(scratch, cx.defaults.clone());
    clip.paste(&mut sc, Point::ZERO, false);
    let project = sc.project.to_json().ok()?;
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    Some(format!(
        "{{\"format\":\"{FORMAT}\",\"version\":1,\"stamp\":{stamp},\"project\":{project}}}"
    ))
}

/// Writes `clip` to the clipboard file (best effort: a failure only means the
/// clipboard stays inside this window).
pub fn write_file(cx: &EditorContext, clip: &Clipboard) -> bool {
    let Some(path) = file_path() else {
        return false;
    };
    let Some(text) = scratch_json(cx, clip) else {
        return false;
    };
    if let Some(dir) = path.parent() {
        if std::fs::create_dir_all(dir).is_err() {
            return false;
        }
    }
    let tmp = path.with_extension("json.tmp");
    let ok = std::fs::write(&tmp, text).is_ok() && std::fs::rename(&tmp, &path).is_ok();
    if ok {
        SEEN.with(|s| *s.borrow_mut() = modified(&path));
    }
    ok
}

/// Reads a clipboard from the file at `path`.
fn read_file(cx: &EditorContext, path: &Path) -> Option<Clipboard> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    if value.get("format").and_then(serde_json::Value::as_str) != Some(FORMAT) {
        return None;
    }
    let project = plan_core::Project::from_json(&value.get("project")?.to_string()).ok()?;
    let layers = project.layers.layers.clone();
    let mut sc = EditorContext::with_project(project, cx.defaults.clone());
    // Everything in the file was copied, whatever its layer shows.
    for l in &mut sc.project.layers.layers {
        l.display = true;
        l.locked = false;
    }
    super::selection::select_all(&mut sc);
    let mut clip = Clipboard::capture(&sc);
    clip.layers = layers;
    (!clip.is_empty()).then_some(clip)
}

/// Takes the clipboard file as the clipboard when it was written by someone
/// else since this process last looked (a Copy in another window or plan).
/// True when the clipboard changed.
pub fn poll_file_now(cx: &mut EditorContext) -> bool {
    let Some(path) = file_path() else {
        return false;
    };
    let Some(mtime) = modified(&path) else {
        return false;
    };
    if SEEN.with(|s| *s.borrow() == Some(mtime)) {
        return false;
    }
    SEEN.with(|s| *s.borrow_mut() = Some(mtime));
    match read_file(cx, &path) {
        Some(clip) => {
            cx.clipboard = Some(clip);
            NOTE_PENDING.with(|n| n.set(true));
            true
        }
        None => false,
    }
}

/// [`poll_file_now`] at most twice a second; the Select tool calls it once a
/// frame.
pub fn poll_file(cx: &mut EditorContext) {
    let now = Instant::now();
    let due = LAST_POLL.with(|l| {
        let due = l
            .get()
            .is_none_or(|t| now.duration_since(t).as_millis() >= 500);
        if due {
            l.set(Some(now));
        }
        due
    });
    if due {
        poll_file_now(cx);
    }
}
