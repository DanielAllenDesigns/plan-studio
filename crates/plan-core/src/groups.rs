//! Object groups and clipboard (`docs/parity/select-and-edit.md`, S-35..S-38,
//! copy/paste).

use crate::cad::{CadItem, CadObject};
use crate::camera::CameraObject;
use crate::dimension::Dimension;
use crate::geometry::Point;
use crate::model::{Id, Opening, Project, Wall};
use crate::symbols::PlacedSymbol;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A reference to one object in a plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObjectRef {
    Wall(Id),
    Opening(Id),
    Dimension(Id),
    Cad(Id),
    Symbol(Id),
    Camera(Id),
    // Kinds the editor stores as opaque records; a group may hold them but
    // the clipboard here does not copy them (the editor's clipboard does).
    Cabinet(Id),
    Stair(Id),
    Device(Id),
    RoofPlane(Id),
    Foundation(Id),
    Framing(Id),
    Detail(Id),
    Schedule(Id),
    /// An architectural block (`Floor::blocks`).
    Block(Id),
    /// A compound 3D solid made by a Boolean operation (`Floor::solid_layer`).
    Solid(Id),
}

/// A set of objects that select and move together (S-35).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectGroup {
    pub id: Id,
    pub members: Vec<ObjectRef>,
}

impl crate::model::Floor {
    /// The group containing `r`, if any (S-35).
    pub fn group_of(&self, r: ObjectRef) -> Option<&ObjectGroup> {
        self.groups.iter().find(|g| g.members.contains(&r))
    }

    /// Every member of the group holding `r` (itself included), or just `r`
    /// when it is in no group: what a click on `r` selects (S-35).
    pub fn group_members_of(&self, r: ObjectRef) -> Vec<ObjectRef> {
        self.group_of(r)
            .map_or_else(|| vec![r], |g| g.members.clone())
    }

    /// `refs` with the rest of every group they touch, without repeats and in
    /// first-seen order.
    pub fn expand_groups(&self, refs: &[ObjectRef]) -> Vec<ObjectRef> {
        let mut out: Vec<ObjectRef> = Vec::new();
        for r in refs {
            for m in self.group_members_of(*r) {
                if !out.contains(&m) {
                    out.push(m);
                }
            }
        }
        out
    }

    /// Drops the members for which `alive` is false and dissolves groups left
    /// with fewer than two members (after objects were deleted). Returns how
    /// many groups went.
    pub fn prune_groups(&mut self, alive: impl Fn(ObjectRef) -> bool) -> usize {
        let before = self.groups.len();
        for g in &mut self.groups {
            g.members.retain(|m| alive(*m));
        }
        self.groups.retain(|g| g.members.len() >= 2);
        before - self.groups.len()
    }
}

/// Copied objects, independent of any floor. Wall openings travel with their walls.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Clipboard {
    pub walls: Vec<Wall>,
    pub openings: Vec<Opening>,
    pub dimensions: Vec<Dimension>,
    pub cad: Vec<CadObject>,
    pub symbols: Vec<PlacedSymbol>,
    pub cameras: Vec<CameraObject>,
    /// Groups wholly inside the copied set, in terms of the copied (old) ids.
    pub groups: Vec<Vec<ObjectRef>>,
}

impl Clipboard {
    pub fn is_empty(&self) -> bool {
        self.walls.is_empty()
            && self.dimensions.is_empty()
            && self.cad.is_empty()
            && self.symbols.is_empty()
            && self.cameras.is_empty()
    }
}

fn translate_cad(item: &mut CadItem, d: Point) {
    match item {
        CadItem::Line { a, b } => {
            *a = *a + d;
            *b = *b + d;
        }
        CadItem::Arc { center, .. } | CadItem::Circle { center, .. } => *center = *center + d,
        CadItem::Polyline { points, .. } => points.iter_mut().for_each(|p| *p = *p + d),
        CadItem::Text { pos, .. } => *pos = *pos + d,
    }
}

impl Project {
    /// Group `members` on `floor` (S-35). Needs at least two distinct members;
    /// members already in a group are moved out of it (a group left with fewer
    /// than two members is dissolved). Returns the new group's id.
    pub fn make_group(&mut self, floor: usize, members: &[ObjectRef]) -> Option<Id> {
        let mut unique: Vec<ObjectRef> = Vec::new();
        for m in members {
            if !unique.contains(m) {
                unique.push(*m);
            }
        }
        if unique.len() < 2 {
            return None;
        }
        let id = self.alloc_id();
        let groups = &mut self.floors[floor].groups;
        for g in groups.iter_mut() {
            g.members.retain(|m| !unique.contains(m));
        }
        groups.retain(|g| g.members.len() >= 2);
        groups.push(ObjectGroup {
            id,
            members: unique,
        });
        Some(id)
    }

    /// Ungroup (S-35): remove the group and return its members.
    pub fn explode_group(&mut self, floor: usize, group_id: Id) -> Option<Vec<ObjectRef>> {
        let groups = &mut self.floors[floor].groups;
        let at = groups.iter().position(|g| g.id == group_id)?;
        Some(groups.remove(at).members)
    }

    /// Copy objects of `floor` (S-38). Walls carry all of their openings;
    /// an opening reference alone is ignored (it needs its wall). Unknown
    /// references are skipped. Groups wholly inside the copied set are kept.
    pub fn copy_objects(&self, floor: usize, refs: &[ObjectRef]) -> Clipboard {
        let f = &self.floors[floor];
        let mut clip = Clipboard::default();
        let mut copied: HashSet<ObjectRef> = HashSet::new();
        for r in refs {
            match *r {
                ObjectRef::Wall(id) => {
                    if let Some(w) = f.wall(id) {
                        if copied.insert(*r) {
                            clip.walls.push(w.clone());
                            for o in f.openings_on(id) {
                                clip.openings.push(o.clone());
                                copied.insert(ObjectRef::Opening(o.id));
                            }
                        }
                    }
                }
                ObjectRef::Opening(_) => {}
                ObjectRef::Dimension(id) => {
                    if let Some(d) = f.dimensions.iter().find(|d| d.id == id) {
                        if copied.insert(*r) {
                            clip.dimensions.push(d.clone());
                        }
                    }
                }
                ObjectRef::Cad(id) => {
                    if let Some(c) = f.cad.iter().find(|c| c.id == id) {
                        if copied.insert(*r) {
                            clip.cad.push(c.clone());
                        }
                    }
                }
                ObjectRef::Symbol(id) => {
                    if let Some(s) = f.symbol(id) {
                        if copied.insert(*r) {
                            clip.symbols.push(s.clone());
                        }
                    }
                }
                ObjectRef::Camera(id) => {
                    if let Some(c) = self.camera(id) {
                        if copied.insert(*r) {
                            clip.cameras.push(c.clone());
                        }
                    }
                }
                _ => {}
            }
        }
        for g in &f.groups {
            if g.members.iter().all(|m| copied.contains(m)) {
                clip.groups.push(g.members.clone());
            }
        }
        clip
    }

    /// Paste a clipboard onto `floor` shifted by `offset` (S-37). Every object
    /// gets a fresh id; pasted openings follow their pasted walls and keep
    /// their offsets along them; cameras land on `floor`. Returns the new
    /// objects' references (walls, openings, dimensions, CAD, symbols, cameras).
    pub fn paste(&mut self, floor: usize, clip: &Clipboard, offset: Point) -> Vec<ObjectRef> {
        let mut out = Vec::new();
        let mut map: HashMap<ObjectRef, ObjectRef> = HashMap::new();
        let mut wall_ids: HashMap<Id, Id> = HashMap::new();

        for w in &clip.walls {
            let id = self.alloc_id();
            let mut w = w.clone();
            wall_ids.insert(w.id, id);
            map.insert(ObjectRef::Wall(w.id), ObjectRef::Wall(id));
            w.id = id;
            w.start = w.start + offset;
            w.end = w.end + offset;
            self.floors[floor].walls.push(w);
            out.push(ObjectRef::Wall(id));
        }
        for o in &clip.openings {
            let Some(&wall_id) = wall_ids.get(&o.wall_id) else {
                continue;
            };
            let id = self.alloc_id();
            let mut o = o.clone();
            map.insert(ObjectRef::Opening(o.id), ObjectRef::Opening(id));
            o.id = id;
            o.wall_id = wall_id;
            self.floors[floor].openings.push(o);
            out.push(ObjectRef::Opening(id));
        }
        // The copies are free until the objects they were tied to are known
        // (DIM-38, below).
        let mut pasted_dims: Vec<(Id, [Option<crate::dim_assoc::DimAnchor>; 2])> = Vec::new();
        for d in &clip.dimensions {
            let id = self.alloc_id();
            let mut d = d.clone();
            map.insert(ObjectRef::Dimension(d.id), ObjectRef::Dimension(id));
            pasted_dims.push((id, d.anchors));
            d.id = id;
            d.anchors = [None, None];
            d.start = d.start + offset;
            d.end = d.end + offset;
            self.floors[floor].dimensions.push(d);
            out.push(ObjectRef::Dimension(id));
        }
        for c in &clip.cad {
            let id = self.alloc_id();
            let mut c = c.clone();
            map.insert(ObjectRef::Cad(c.id), ObjectRef::Cad(id));
            c.id = id;
            translate_cad(&mut c.item, offset);
            self.floors[floor].cad.push(c);
            out.push(ObjectRef::Cad(id));
        }
        for s in &clip.symbols {
            let id = self.alloc_id();
            let mut s = s.clone();
            map.insert(ObjectRef::Symbol(s.id), ObjectRef::Symbol(id));
            s.id = id;
            s.position = s.position + offset;
            self.floors[floor].symbols.push(s);
            out.push(ObjectRef::Symbol(id));
        }
        for c in &clip.cameras {
            let id = self.alloc_id();
            let mut c = c.clone();
            map.insert(ObjectRef::Camera(c.id), ObjectRef::Camera(id));
            c.id = id;
            c.position = c.position + offset;
            c.floor = floor;
            self.cameras.push(c);
            out.push(ObjectRef::Camera(id));
        }
        // DIM-38: a dimension pasted with every object it was tied to stays
        // tied to the copies (it follows them); if any of those was not
        // copied it pastes free.
        for (id, old) in pasted_dims {
            let anchors = retie_anchors(&old, &map, offset);
            if let Some(d) = self.floors[floor]
                .dimensions
                .iter_mut()
                .find(|d| d.id == id)
            {
                d.anchors = anchors;
            }
        }
        for g in &clip.groups {
            let members: Vec<ObjectRef> = g.iter().filter_map(|m| map.get(m).copied()).collect();
            if members.len() >= 2 {
                let id = self.alloc_id();
                self.floors[floor].groups.push(ObjectGroup { id, members });
            }
        }
        out
    }
}

/// The anchors of a pasted dimension: each tied end moved to the copy of its
/// object, or both ends free when an object it was tied to was not copied
/// (the same rule as the editor's clipboard).
fn retie_anchors(
    old: &[Option<crate::dim_assoc::DimAnchor>; 2],
    map: &HashMap<ObjectRef, ObjectRef>,
    offset: Point,
) -> [Option<crate::dim_assoc::DimAnchor>; 2] {
    use crate::dim_assoc::AnchorTarget;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::CameraKind;
    use crate::model::{OpeningKind, WallKind};

    fn plan() -> (Project, Id, Id) {
        let mut p = Project::new("g");
        let a = p.add_wall(
            0,
            Point::ZERO,
            Point::new(200.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        let b = p.add_wall(
            0,
            Point::new(200.0, 0.0),
            Point::new(200.0, 100.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        p.add_opening(0, a, 100.0, OpeningKind::Door).unwrap();
        (p, a, b)
    }

    #[test]
    fn group_make_find_explode() {
        let (mut p, a, b) = plan();
        let g = p
            .make_group(0, &[ObjectRef::Wall(a), ObjectRef::Wall(b)])
            .unwrap();
        assert_eq!(p.floors[0].group_of(ObjectRef::Wall(a)).unwrap().id, g);
        assert!(p.floors[0].group_of(ObjectRef::Cad(1)).is_none());
        assert!(p
            .make_group(0, &[ObjectRef::Wall(a), ObjectRef::Wall(a)])
            .is_none());
        // Regrouping a member steals it; the leftover single-member group dissolves.
        let c = p.add_cad(
            0,
            "CAD, Default",
            CadItem::Circle {
                center: Point::ZERO,
                radius: 5.0,
            },
        );
        let g2 = p
            .make_group(0, &[ObjectRef::Wall(a), ObjectRef::Cad(c)])
            .unwrap();
        assert_eq!(p.floors[0].groups.len(), 1);
        assert_eq!(p.floors[0].group_of(ObjectRef::Wall(a)).unwrap().id, g2);
        assert!(p.floors[0].group_of(ObjectRef::Wall(b)).is_none());
        let members = p.explode_group(0, g2).unwrap();
        assert_eq!(members.len(), 2);
        assert!(p.floors[0].groups.is_empty());
        assert!(p.explode_group(0, g2).is_none());
    }

    #[test]
    fn copy_paste_assigns_new_ids_and_carries_openings() {
        let (mut p, a, b) = plan();
        let cam = p.add_camera(CameraObject::new(
            CameraKind::FullCamera,
            Point::new(5.0, 5.0),
            0.0,
            "c",
            0,
        ));
        let sym = p.add_symbol(
            0,
            PlacedSymbol::new("x", Point::new(20.0, 20.0), 10.0, 10.0, 10.0),
        );
        let dim = p.add_dimension(
            0,
            Dimension::new(
                0,
                crate::dimension::DimensionKind::Manual,
                Point::ZERO,
                Point::new(10.0, 0.0),
                6.0,
            ),
        );
        let cad = p.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::ZERO,
                b: Point::new(3.0, 4.0),
            },
        );
        p.make_group(0, &[ObjectRef::Wall(a), ObjectRef::Wall(b)])
            .unwrap();
        let refs = [
            ObjectRef::Wall(a),
            ObjectRef::Wall(b),
            ObjectRef::Camera(cam),
            ObjectRef::Symbol(sym),
            ObjectRef::Dimension(dim),
            ObjectRef::Cad(cad),
            ObjectRef::Wall(9999),
        ];
        let clip = p.copy_objects(0, &refs);
        assert_eq!(clip.walls.len(), 2);
        assert_eq!(clip.openings.len(), 1);
        assert_eq!(clip.groups.len(), 1);
        assert!(!clip.is_empty());

        let before_ids: Vec<Id> = p.floors[0].walls.iter().map(|w| w.id).collect();
        let new = p.paste(0, &clip, Point::new(0.0, 500.0));
        // 2 walls + 1 opening + camera + symbol + dim + cad.
        assert_eq!(new.len(), 7);
        let f = &p.floors[0];
        assert_eq!(f.walls.len(), 4);
        let pasted: Vec<&Wall> = f
            .walls
            .iter()
            .filter(|w| !before_ids.contains(&w.id))
            .collect();
        assert_eq!(pasted.len(), 2);
        assert!(pasted.iter().any(|w| w.start == Point::new(0.0, 500.0)));
        // The pasted opening belongs to a pasted wall, same offset.
        assert_eq!(f.openings.len(), 2);
        let po = f
            .openings
            .iter()
            .find(|o| pasted.iter().any(|w| w.id == o.wall_id))
            .unwrap();
        assert!((po.center_offset - 100.0).abs() < 1e-9);
        assert_ne!(po.wall_id, a);
        // All ids unique across the plan.
        let mut ids: Vec<Id> = f.walls.iter().map(|w| w.id).collect();
        ids.extend(f.openings.iter().map(|o| o.id));
        ids.extend(f.symbols.iter().map(|s| s.id));
        ids.extend(f.dimensions.iter().map(|d| d.id));
        ids.extend(f.cad.iter().map(|c| c.id));
        ids.extend(p.cameras.iter().map(|c| c.id));
        let set: HashSet<Id> = ids.iter().copied().collect();
        assert_eq!(set.len(), ids.len());
        // Translations applied; the pasted group has new members.
        assert_eq!(f.symbols[1].position, Point::new(20.0, 520.0));
        assert_eq!(f.dimensions[1].start, Point::new(0.0, 500.0));
        match &f.cad[1].item {
            CadItem::Line { b, .. } => assert_eq!(*b, Point::new(3.0, 504.0)),
            _ => panic!("expected a line"),
        }
        assert_eq!(p.cameras.len(), 2);
        assert_eq!(f.groups.len(), 2);
        assert!(f.groups[1]
            .members
            .iter()
            .all(|m| !f.groups[0].members.contains(m)));
        // Paste onto another floor places cameras there.
        p.build_new_floor(false);
        p.paste(1, &clip, Point::ZERO);
        assert_eq!(p.cameras_on(1).count(), 1);
        assert_eq!(p.floors[1].walls.len(), 2);
    }

    #[test]
    fn opening_alone_is_not_copied() {
        let (p, _, _) = plan();
        let oid = p.floors[0].openings[0].id;
        let clip = p.copy_objects(0, &[ObjectRef::Opening(oid)]);
        assert!(clip.is_empty() && clip.openings.is_empty());
    }

    #[test]
    fn groups_expand_a_click_and_dissolve_when_members_go() {
        let (mut p, a, b) = plan();
        let c = p.add_cad(
            0,
            "CAD, Default",
            CadItem::Circle {
                center: Point::ZERO,
                radius: 5.0,
            },
        );
        p.make_group(
            0,
            &[
                ObjectRef::Wall(a),
                ObjectRef::Wall(b),
                ObjectRef::Cabinet(77),
            ],
        )
        .unwrap();
        let f = &p.floors[0];
        // A click on any member names the whole group; a loose object stays alone.
        assert_eq!(f.group_members_of(ObjectRef::Wall(b)).len(), 3);
        assert_eq!(
            f.group_members_of(ObjectRef::Cad(c)),
            vec![ObjectRef::Cad(c)]
        );
        let both = f.expand_groups(&[ObjectRef::Wall(a), ObjectRef::Cad(c), ObjectRef::Wall(b)]);
        assert_eq!(both.len(), 4);
        // The cabinet vanishes: two members remain. A second loss dissolves it.
        let f = &mut p.floors[0];
        assert_eq!(f.prune_groups(|r| r != ObjectRef::Cabinet(77)), 0);
        assert_eq!(f.groups[0].members.len(), 2);
        assert_eq!(f.prune_groups(|r| r != ObjectRef::Wall(b)), 1);
        assert!(f.groups.is_empty());
    }

    /// A dimension tied to a wall follows the copy of that wall when both
    /// are pasted, and pastes free when only the dimension is (DIM-38).
    #[test]
    fn a_pasted_dimension_is_tied_to_the_copies_of_its_walls() {
        use crate::dim_assoc::{AnchorTarget, DimAnchor, DimAttach};
        let (mut p, a, b) = plan();
        let mut d = Dimension::new(
            0,
            crate::dimension::DimensionKind::Manual,
            Point::ZERO,
            Point::new(10.0, 0.0),
            6.0,
        );
        let anchor = |wall, at| DimAnchor {
            wall,
            target: AnchorTarget::Wall,
            at,
            side: 0.0,
            last: Point::ZERO,
            axis: Default::default(),
            extra: 0.0,
        };
        d.anchors = [
            Some(anchor(a, DimAttach::Start)),
            Some(anchor(b, DimAttach::End)),
        ];
        let dim = p.add_dimension(0, d);
        let both = p.copy_objects(
            0,
            &[
                ObjectRef::Wall(a),
                ObjectRef::Wall(b),
                ObjectRef::Dimension(dim),
            ],
        );
        let new = p.paste(0, &both, Point::new(0.0, 500.0));
        let pasted_walls: Vec<Id> = new
            .iter()
            .filter_map(|r| match r {
                ObjectRef::Wall(id) => Some(*id),
                _ => None,
            })
            .collect();
        let pasted = new
            .iter()
            .find_map(|r| match r {
                ObjectRef::Dimension(id) => p.floors[0].dimensions.iter().find(|d| d.id == *id),
                _ => None,
            })
            .unwrap();
        let tied: Vec<Id> = pasted.anchors.iter().flatten().map(|x| x.wall).collect();
        assert_eq!(tied.len(), 2);
        assert!(tied.iter().all(|w| pasted_walls.contains(w) && *w != a && *w != b));
        // Alone, the copy has nothing to follow.
        let alone = p.copy_objects(0, &[ObjectRef::Dimension(dim)]);
        let new = p.paste(0, &alone, Point::new(0.0, 900.0));
        let free = new
            .iter()
            .find_map(|r| match r {
                ObjectRef::Dimension(id) => p.floors[0].dimensions.iter().find(|d| d.id == *id),
                _ => None,
            })
            .unwrap();
        assert_eq!(free.anchors, [None, None]);
    }
}
