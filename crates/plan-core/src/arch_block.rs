//! Architectural blocks (reference manual "Architectural Blocks", pp. 1057 to
//! 1062; parity rows CB-427..CB-438).
//!
//! An architectural block is a set of architectural objects that move, rotate,
//! copy and schedule as one. The sub-objects keep living in their own stores
//! (cabinets, symbols, details, devices, slabs, compound solids ...); the
//! block is a record in [`Floor::blocks`] that lists them by [`ObjectRef`]. A
//! block may hold other blocks (nesting) and is a member like any other:
//! `ObjectRef::Block(id)`.
//!
//! * **Eligible** members are cabinets (base, wall, full height, soffits,
//!   shelves, partitions, countertops), library symbols (fixtures, furniture,
//!   hardware, millwork, shapes, images, distribution records), electrical
//!   devices, slabs, 3D solids and molding polylines, compound solids and
//!   other blocks. CAD objects, text, dimensions, walls, openings, stairs,
//!   roofs, framing and cameras are not (CAD blocks hold those).
//! * **Ganged electrical blocks** hold two or more devices and nothing else.
//! * The block sits on the layer all members share, else on
//!   [`BLOCK_LAYER`].
//! * [`Floor::explode_block`] dissolves the record and leaves the members.
//!
//! plan-core cannot see the geometry of cabinets or devices (their crates
//! depend on it), so the plan box of a block is computed by the editor
//! (`tools::arch_block`); this module owns the structure and the rules.

use crate::details::{DetailRef, DetailsLayer};
use crate::foundation::{FoundationLayer, FoundationRef};
use crate::geometry::Point;
use crate::groups::ObjectRef;
use crate::model::{Floor, Id};
use serde::{Deserialize, Serialize};

/// The layer a block of objects from different layers sits on.
pub const BLOCK_LAYER: &str = "Architectural Blocks";
/// The layer block labels are drawn on.
pub const BLOCK_LABEL_LAYER: &str = "Architectural Blocks, Labels";
/// The name a new block starts with.
pub const DEFAULT_BLOCK_NAME: &str = "Architectural Block";

/// Where "to Top" and "to Bottom" are measured from (reference manual
/// "Elevation References", p. 21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ElevationRef {
    Absolute,
    #[default]
    FromFloor,
    FromFinishedFloor,
    FromTerrain,
    FromCeiling,
    FromRoof,
}

impl ElevationRef {
    pub const ALL: [ElevationRef; 6] = [
        ElevationRef::Absolute,
        ElevationRef::FromFloor,
        ElevationRef::FromFinishedFloor,
        ElevationRef::FromTerrain,
        ElevationRef::FromCeiling,
        ElevationRef::FromRoof,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ElevationRef::Absolute => "Absolute",
            ElevationRef::FromFloor => "From Floor",
            ElevationRef::FromFinishedFloor => "From Finished Floor",
            ElevationRef::FromTerrain => "From Terrain",
            ElevationRef::FromCeiling => "From Ceiling",
            ElevationRef::FromRoof => "From Roof",
        }
    }
}

/// What the block is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BlockKind {
    #[default]
    Standard,
    /// Two or more wall- or cabinet-mounted electrical objects (CB-428).
    GangedElectrical,
}

/// How the block's label is made (CB-430).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BlockLabelMode {
    /// The block's name.
    #[default]
    Automatic,
    Custom,
    Hidden,
}

/// The Label panel.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BlockLabel {
    pub mode: BlockLabelMode,
    pub text: String,
    /// Offset of the label from its default place (the label's Move handle).
    pub offset: Point,
    /// Show the labels of the sub-objects that have one.
    pub sub_object_labels: bool,
}

/// The Schedule panel of a block that is treated as one object.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BlockSchedule {
    /// Leave the block out of every schedule.
    pub exclude: bool,
    pub category: String,
    pub mark: String,
    pub manufacturer: String,
    pub model: String,
    pub note: String,
}

/// An architectural block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ArchBlock {
    pub id: Id,
    pub kind: BlockKind,
    /// The sub-objects, other blocks included.
    pub members: Vec<ObjectRef>,
    pub name: String,
    /// The layer the block sits on.
    pub layer: String,
    /// Display Bounding Box (CB-436).
    pub display_bounding_box: bool,
    /// Display Sub-Objects: off shows only the bounding box.
    pub display_sub_objects: bool,
    /// Display Sub-Objects Using Block Layer.
    pub sub_objects_use_block_layer: bool,
    /// Display Sub-Objects Using Block Draw Order.
    pub sub_objects_use_block_draw_order: bool,
    /// Treat as One Object in schedules and the Materials List.
    pub treat_as_one: bool,
    /// Size/Position: what to Top and to Bottom are measured from.
    pub elevation_ref: ElevationRef,
    /// Suppress Adjacent Room Moldings where the bounding box crosses them.
    pub suppress_room_moldings: bool,
    pub label: BlockLabel,
    pub schedule: BlockSchedule,
}

impl Default for ArchBlock {
    fn default() -> Self {
        Self {
            id: 0,
            kind: BlockKind::Standard,
            members: Vec::new(),
            name: DEFAULT_BLOCK_NAME.to_string(),
            layer: BLOCK_LAYER.to_string(),
            display_bounding_box: true,
            display_sub_objects: true,
            sub_objects_use_block_layer: false,
            sub_objects_use_block_draw_order: false,
            treat_as_one: true,
            elevation_ref: ElevationRef::FromFloor,
            suppress_room_moldings: false,
            label: BlockLabel::default(),
            schedule: BlockSchedule::default(),
        }
    }
}

impl ArchBlock {
    /// The text of the plan label, `None` when hidden.
    pub fn label_text(&self) -> Option<String> {
        match self.label.mode {
            BlockLabelMode::Hidden => None,
            BlockLabelMode::Automatic => Some(self.name.clone()),
            BlockLabelMode::Custom => Some(self.label.text.clone()),
        }
    }

    /// Turning off the sub-objects turns the bounding box on (p. 1060).
    pub fn normalize(&mut self) {
        if !self.display_sub_objects {
            self.display_bounding_box = true;
        }
        if self.kind == BlockKind::GangedElectrical {
            self.treat_as_one = true;
        }
    }
}

/// Every architectural block of a floor.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockLayer {
    pub blocks: Vec<ArchBlock>,
}

impl BlockLayer {
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn get(&self, id: Id) -> Option<&ArchBlock> {
        self.blocks.iter().find(|b| b.id == id)
    }

    pub fn get_mut(&mut self, id: Id) -> Option<&mut ArchBlock> {
        self.blocks.iter_mut().find(|b| b.id == id)
    }

    /// The block `r` is a direct member of.
    pub fn parent_of(&self, r: ObjectRef) -> Option<&ArchBlock> {
        self.blocks.iter().find(|b| b.members.contains(&r))
    }

    /// The outermost block holding `r`, directly or through nesting.
    pub fn root_of(&self, r: ObjectRef) -> Option<&ArchBlock> {
        let mut cur = self.parent_of(r)?;
        for _ in 0..self.blocks.len() + 1 {
            match self.parent_of(ObjectRef::Block(cur.id)) {
                Some(p) => cur = p,
                None => break,
            }
        }
        Some(cur)
    }

    /// Every non-block object inside block `id`, nesting resolved. Cycles
    /// (which the editor never makes) cannot loop.
    pub fn flat_members(&self, id: Id) -> Vec<ObjectRef> {
        let mut out = Vec::new();
        let mut seen = vec![id];
        self.collect(id, &mut out, &mut seen);
        out
    }

    fn collect(&self, id: Id, out: &mut Vec<ObjectRef>, seen: &mut Vec<Id>) {
        let Some(b) = self.get(id) else {
            return;
        };
        for m in &b.members {
            match *m {
                ObjectRef::Block(inner) => {
                    if !seen.contains(&inner) {
                        seen.push(inner);
                        self.collect(inner, out, seen);
                    }
                }
                other => {
                    if !out.contains(&other) {
                        out.push(other);
                    }
                }
            }
        }
    }

    /// Ids of `id` and of every block nested inside it.
    pub fn nested_ids(&self, id: Id) -> Vec<Id> {
        let mut ids = vec![id];
        let mut i = 0;
        while i < ids.len() {
            if let Some(b) = self.get(ids[i]) {
                for m in &b.members {
                    if let ObjectRef::Block(inner) = *m {
                        if !ids.contains(&inner) {
                            ids.push(inner);
                        }
                    }
                }
            }
            i += 1;
        }
        ids
    }

    /// How deep block `id` nests (0 for a block in no other block).
    pub fn depth_of(&self, id: Id) -> usize {
        let mut d = 0;
        let mut cur = ObjectRef::Block(id);
        while let Some(p) = self.parent_of(cur) {
            d += 1;
            cur = ObjectRef::Block(p.id);
            if d > self.blocks.len() {
                break;
            }
        }
        d
    }
}

/// Why a set of objects cannot be blocked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockError {
    /// Fewer than two objects.
    TooFew,
    /// The named kind cannot be in an architectural block.
    NotArchitectural(&'static str),
    /// An object is already inside another block (select that block instead).
    AlreadyBlocked(ObjectRef),
    /// Electrical objects can only be blocked with each other.
    ElectricalMixed,
    /// A ganged block needs two or more electrical devices.
    NeedDevices,
    /// An object does not exist.
    Missing(ObjectRef),
}

impl std::fmt::Display for BlockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockError::TooFew => write!(f, "Select two or more architectural objects to block"),
            BlockError::NotArchitectural(k) => write!(
                f,
                "{k} cannot be in an architectural block (CAD objects, text and dimensions go in CAD blocks)"
            ),
            BlockError::AlreadyBlocked(_) => {
                write!(f, "An object is already in an architectural block")
            }
            BlockError::ElectricalMixed => write!(
                f,
                "Electrical objects can only be blocked with other electrical objects"
            ),
            BlockError::NeedDevices => write!(
                f,
                "A ganged electrical block needs two or more electrical objects"
            ),
            BlockError::Missing(_) => write!(f, "An object no longer exists"),
        }
    }
}

impl std::error::Error for BlockError {}

/// The kind name used in messages.
fn kind_name(r: ObjectRef) -> &'static str {
    match r {
        ObjectRef::Wall(_) => "Walls",
        ObjectRef::Opening(_) => "Doors and windows",
        ObjectRef::Dimension(_) => "Dimensions",
        ObjectRef::Cad(_) => "CAD objects and text",
        ObjectRef::Camera(_) => "Cameras",
        ObjectRef::Stair(_) => "Stairs",
        ObjectRef::RoofPlane(_) => "Roof planes",
        ObjectRef::Framing(_) => "Framing",
        ObjectRef::Schedule(_) => "Schedules",
        ObjectRef::Detail(_) => "That detail object",
        ObjectRef::Foundation(_) => "That foundation object",
        _ => "That object",
    }
}

impl Floor {
    /// Does cabinet `id` exist (cabinets are stored as JSON records)?
    fn has_cabinet(&self, id: Id) -> bool {
        self.cabinets
            .iter()
            .any(|c| c.get("id").and_then(|v| v.as_u64()) == Some(id))
    }

    /// Does `r` exist on this floor? Devices live in the electrical slot as
    /// opaque JSON; they are looked up by id in its `devices` array.
    pub fn block_member_exists(&self, r: ObjectRef) -> bool {
        match r {
            ObjectRef::Cabinet(id) => self.has_cabinet(id),
            ObjectRef::Symbol(id) => self.symbol(id).is_some(),
            ObjectRef::Detail(id) => DetailsLayer::load(self).find(id).is_some(),
            ObjectRef::Foundation(id) => FoundationLayer::load(self).find(id).is_some(),
            ObjectRef::Device(id) => self
                .electrical
                .as_ref()
                .and_then(|e| e.get("devices"))
                .and_then(|d| d.as_array())
                .is_some_and(|a| a.iter().any(|d| d.get("id").and_then(|v| v.as_u64()) == Some(id))),
            ObjectRef::Block(id) => self.blocks.get(id).is_some(),
            ObjectRef::Solid(id) => self.solid_layer.compound(id).is_some(),
            ObjectRef::Wall(id) => self.wall(id).is_some(),
            _ => true,
        }
    }

    /// May `r` be a sub-object of an architectural block?
    pub fn block_eligible(&self, r: ObjectRef) -> Result<(), BlockError> {
        let ok = match r {
            ObjectRef::Cabinet(_)
            | ObjectRef::Symbol(_)
            | ObjectRef::Device(_)
            | ObjectRef::Block(_)
            | ObjectRef::Solid(_) => true,
            ObjectRef::Foundation(id) => matches!(
                FoundationLayer::load(self).find(id),
                Some(FoundationRef::Slab(_))
            ),
            ObjectRef::Detail(id) => matches!(
                DetailsLayer::load(self).find(id),
                Some(DetailRef::Solid(_) | DetailRef::Molding(_))
            ),
            _ => false,
        };
        if !ok {
            return Err(BlockError::NotArchitectural(kind_name(r)));
        }
        if !self.block_member_exists(r) {
            return Err(BlockError::Missing(r));
        }
        // A distribution copy belongs to its record, not to a block.
        if let ObjectRef::Symbol(id) = r {
            if self.symbol(id).is_some_and(|s| s.owner.is_some()) {
                return Err(BlockError::NotArchitectural("A distributed copy"));
            }
        }
        Ok(())
    }

    /// The layer a block of `members` sits on: the layer they share, else
    /// [`BLOCK_LAYER`]. `layer_of` gives the layer of one member.
    pub fn block_layer_for(
        &self,
        members: &[ObjectRef],
        layer_of: &dyn Fn(ObjectRef) -> Option<String>,
    ) -> String {
        let mut layer: Option<String> = None;
        for m in members {
            let l = match m {
                ObjectRef::Block(id) => self.blocks.get(*id).map(|b| b.layer.clone()),
                other => layer_of(*other),
            };
            let Some(l) = l else { continue };
            match &layer {
                None => layer = Some(l),
                Some(cur) if *cur == l => {}
                Some(_) => return BLOCK_LAYER.to_string(),
            }
        }
        layer.unwrap_or_else(|| BLOCK_LAYER.to_string())
    }

    /// Makes an architectural block of `members` with id `id` (Make
    /// Architectural Block, or the Make Ganged Electrical Block edit tool
    /// when `kind` is [`BlockKind::GangedElectrical`]).
    pub fn make_block(
        &mut self,
        id: Id,
        members: &[ObjectRef],
        kind: BlockKind,
        layer_of: &dyn Fn(ObjectRef) -> Option<String>,
    ) -> Result<Id, BlockError> {
        let mut unique: Vec<ObjectRef> = Vec::new();
        for m in members {
            if !unique.contains(m) {
                unique.push(*m);
            }
        }
        if unique.len() < 2 {
            return Err(if kind == BlockKind::GangedElectrical {
                BlockError::NeedDevices
            } else {
                BlockError::TooFew
            });
        }
        let devices = unique
            .iter()
            .filter(|m| matches!(m, ObjectRef::Device(_)))
            .count();
        if kind == BlockKind::GangedElectrical && devices != unique.len() {
            return Err(BlockError::NeedDevices);
        }
        if kind == BlockKind::Standard && devices > 0 && devices != unique.len() {
            return Err(BlockError::ElectricalMixed);
        }
        for m in &unique {
            self.block_eligible(*m)?;
            if self.blocks.parent_of(*m).is_some() {
                return Err(BlockError::AlreadyBlocked(*m));
            }
        }
        let layer = self.block_layer_for(&unique, layer_of);
        let mut b = ArchBlock {
            id,
            kind,
            members: unique.clone(),
            layer,
            ..ArchBlock::default()
        };
        if kind == BlockKind::GangedElectrical {
            b.name = "Ganged Electrical Block".to_string();
        }
        b.normalize();
        // The members leave any object group: the block is the group now.
        for g in &mut self.groups {
            g.members.retain(|m| !unique.contains(m));
        }
        self.groups.retain(|g| g.members.len() >= 2);
        self.blocks.blocks.push(b);
        Ok(id)
    }

    /// Explode Architectural Block: the record goes, the members stay (a
    /// nested block stays a block). Returns the members, `None` when there
    /// is no such block.
    pub fn explode_block(&mut self, id: Id) -> Option<Vec<ObjectRef>> {
        let at = self.blocks.blocks.iter().position(|b| b.id == id)?;
        let b = self.blocks.blocks.remove(at);
        // A block that held this one now holds its members instead.
        for outer in &mut self.blocks.blocks {
            if let Some(i) = outer.members.iter().position(|m| *m == ObjectRef::Block(id)) {
                outer.members.splice(i..=i, b.members.iter().copied());
            }
        }
        self.drawing_groups.retain(|e| e.object != ObjectRef::Block(id));
        Some(b.members)
    }

    /// Drops the members `alive` rejects and the blocks left empty (after
    /// objects were deleted). A ganged block left with one device stays a
    /// block. Returns how many blocks went.
    pub fn prune_blocks(&mut self, alive: impl Fn(ObjectRef) -> bool) -> usize {
        let before = self.blocks.blocks.len();
        loop {
            let ids: Vec<Id> = self.blocks.blocks.iter().map(|b| b.id).collect();
            for b in &mut self.blocks.blocks {
                b.members.retain(|m| match m {
                    ObjectRef::Block(i) => ids.contains(i),
                    other => alive(*other),
                });
            }
            let n = self.blocks.blocks.len();
            self.blocks.blocks.retain(|b| !b.members.is_empty());
            if self.blocks.blocks.len() == n {
                break;
            }
        }
        before - self.blocks.blocks.len()
    }

    /// Removes block `id`, with every block nested in it, and returns all
    /// the non-block objects that were inside (the caller deletes them).
    pub fn remove_block_with_members(&mut self, id: Id) -> Vec<ObjectRef> {
        let flat = self.blocks.flat_members(id);
        let ids = self.blocks.nested_ids(id);
        self.blocks.blocks.retain(|b| !ids.contains(&b.id));
        for outer in &mut self.blocks.blocks {
            outer.members.retain(|m| *m != ObjectRef::Block(id));
        }
        flat
    }

    /// The block a click on `r` selects: the outermost block holding it.
    pub fn block_for_pick(&self, r: ObjectRef) -> Option<Id> {
        self.blocks.root_of(r).map(|b| b.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Project;
    use crate::symbols::PlacedSymbol;

    fn sym(p: &mut Project, x: f64) -> Id {
        let mut s = PlacedSymbol::new("chair", Point::new(x, 0.0), 20.0, 20.0, 30.0);
        s.layer = "Furniture".into();
        p.add_symbol(0, s)
    }

    fn layer(_: ObjectRef) -> Option<String> {
        Some("Furniture".into())
    }

    #[test]
    fn make_and_explode_a_block() {
        let mut p = Project::new("b");
        let (a, b) = (sym(&mut p, 0.0), sym(&mut p, 40.0));
        let id = p.alloc_id();
        let f = &mut p.floors[0];
        let r = f.make_block(id, &[ObjectRef::Symbol(a), ObjectRef::Symbol(b)], BlockKind::Standard, &layer);
        assert_eq!(r, Ok(id));
        assert_eq!(f.blocks.get(id).unwrap().layer, "Furniture");
        assert_eq!(f.blocks.flat_members(id).len(), 2);
        assert_eq!(f.block_for_pick(ObjectRef::Symbol(a)), Some(id));
        // An object in a block cannot join a second one.
        let c = {
            let mut s = PlacedSymbol::new("t", Point::ZERO, 1.0, 1.0, 1.0);
            s.layer = "Furniture".into();
            p.add_symbol(0, s)
        };
        let id2 = p.alloc_id();
        let f = &mut p.floors[0];
        let again = f.make_block(id2, &[ObjectRef::Symbol(a), ObjectRef::Symbol(c)], BlockKind::Standard, &layer);
        assert_eq!(again, Err(BlockError::AlreadyBlocked(ObjectRef::Symbol(a))));
        let members = f.explode_block(id).unwrap();
        assert_eq!(members.len(), 2);
        assert!(f.blocks.is_empty());
        assert!(f.explode_block(id).is_none());
    }

    #[test]
    fn nesting_flattens_and_explode_hands_members_to_the_outer_block() {
        let mut p = Project::new("n");
        let ids: Vec<Id> = (0..4).map(|i| sym(&mut p, i as f64 * 30.0)).collect();
        let (inner, outer) = (p.alloc_id(), p.alloc_id());
        let f = &mut p.floors[0];
        f.make_block(inner, &[ObjectRef::Symbol(ids[0]), ObjectRef::Symbol(ids[1])], BlockKind::Standard, &layer)
            .unwrap();
        f.make_block(
            outer,
            &[ObjectRef::Block(inner), ObjectRef::Symbol(ids[2]), ObjectRef::Symbol(ids[3])],
            BlockKind::Standard,
            &layer,
        )
        .unwrap();
        assert_eq!(f.blocks.flat_members(outer).len(), 4);
        assert_eq!(f.blocks.depth_of(inner), 1);
        assert_eq!(f.block_for_pick(ObjectRef::Symbol(ids[0])), Some(outer));
        f.explode_block(inner).unwrap();
        assert_eq!(f.blocks.get(outer).unwrap().members.len(), 4);
        let gone = f.remove_block_with_members(outer);
        assert_eq!(gone.len(), 4);
        assert!(f.blocks.is_empty());
    }

    #[test]
    fn cad_objects_and_mixed_electrical_are_refused() {
        let mut p = Project::new("e");
        let a = sym(&mut p, 0.0);
        let id = p.alloc_id();
        let f = &mut p.floors[0];
        let e = f.make_block(id, &[ObjectRef::Symbol(a), ObjectRef::Cad(77)], BlockKind::Standard, &layer);
        assert!(matches!(e, Err(BlockError::NotArchitectural(_))));
        let e = f.make_block(id, &[ObjectRef::Symbol(a)], BlockKind::Standard, &layer);
        assert_eq!(e, Err(BlockError::TooFew));
        let e = f.make_block(id, &[ObjectRef::Symbol(a), ObjectRef::Device(5)], BlockKind::GangedElectrical, &layer);
        assert_eq!(e, Err(BlockError::NeedDevices));
    }

    #[test]
    fn mixed_layers_land_on_the_blocks_layer_and_prune_drops_dead_members() {
        let mut p = Project::new("l");
        let a = sym(&mut p, 0.0);
        let b = sym(&mut p, 50.0);
        let id = p.alloc_id();
        let f = &mut p.floors[0];
        let mixed = |r: ObjectRef| match r {
            ObjectRef::Symbol(i) if i == a => Some("Furniture".to_string()),
            _ => Some("Fixtures".to_string()),
        };
        f.make_block(id, &[ObjectRef::Symbol(a), ObjectRef::Symbol(b)], BlockKind::Standard, &mixed)
            .unwrap();
        assert_eq!(f.blocks.get(id).unwrap().layer, BLOCK_LAYER);
        assert_eq!(f.prune_blocks(|r| r != ObjectRef::Symbol(a)), 0);
        assert_eq!(f.blocks.get(id).unwrap().members, vec![ObjectRef::Symbol(b)]);
        assert_eq!(f.prune_blocks(|_| false), 1);
    }

    #[test]
    fn labels_and_flags() {
        let mut b = ArchBlock::default();
        assert_eq!(b.label_text().as_deref(), Some(DEFAULT_BLOCK_NAME));
        b.label.mode = BlockLabelMode::Custom;
        b.label.text = "Island".into();
        assert_eq!(b.label_text().as_deref(), Some("Island"));
        b.label.mode = BlockLabelMode::Hidden;
        assert_eq!(b.label_text(), None);
        b.display_bounding_box = false;
        b.display_sub_objects = false;
        b.normalize();
        assert!(b.display_bounding_box);
    }
}
