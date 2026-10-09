//! Drawing Groups (LAY-36): a number on every object that sets the order the
//! plan draws in, lowest group first (so a higher group lies on top).
//!
//! Each kind of object starts in the group of its kind (Default Settings >
//! Drawing Groups, [`DrawingGroupTable`]); Edit > Drawing Group > Bring to
//! Front / Send to Back / Set Drawing Group give single objects a group of
//! their own, kept on the floor ([`GroupEntry`], only for objects that
//! differ). The plan view, the Layout window and the sheet PDF sort the CAD
//! objects (and text) by their group; a CAD object whose group is lower than
//! the Walls group draws behind the walls, one at or above it over them.
//! Objects in the same group keep the order they were drawn in.

use crate::cad::{CadItem, CadObject};
use crate::groups::ObjectRef;
use crate::model::{Floor, Project};
use serde::{Deserialize, Serialize};

/// Smallest and largest drawing group.
pub const GROUP_MIN: i32 = 0;
pub const GROUP_MAX: i32 = 999;

/// The kinds of the Drawing Groups table with their starting groups, back to
/// front. CAD starts at 21 (Chief's manual: CAD objects sit behind walls
/// until sent to front); Text keeps its own group, 80, in front of
/// everything.
pub const KINDS: [(&str, i32); 15] = [
    ("Rooms", 10),
    ("Foundation", 15),
    ("Material Regions", 20),
    ("Stairs", 30),
    ("Cabinets and Symbols", 40),
    ("Roofs", 45),
    ("Walls", 50),
    ("Doors and Windows", 55),
    ("Electrical", 60),
    ("Framing", 65),
    ("Dimensions", 70),
    ("CAD", 21),
    ("Text", 80),
    ("Schedules", 85),
    ("Cameras", 95),
];

/// The kind name an object draws under.
pub const WALLS: &str = "Walls";
pub const CAD: &str = "CAD";
pub const TEXT: &str = "Text";

/// One row of the table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KindGroup {
    pub kind: String,
    pub group: i32,
}

/// The group of each kind of object (Default Settings > Drawing Groups).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrawingGroupTable {
    pub kinds: Vec<KindGroup>,
}

impl Default for DrawingGroupTable {
    fn default() -> Self {
        Self {
            kinds: KINDS
                .iter()
                .map(|(k, g)| KindGroup {
                    kind: (*k).to_string(),
                    group: *g,
                })
                .collect(),
        }
    }
}

impl DrawingGroupTable {
    /// The group of `kind` (its starting group when the table lacks it).
    pub fn group_of(&self, kind: &str) -> i32 {
        self.kinds
            .iter()
            .find(|k| k.kind == kind)
            .map(|k| k.group)
            .or_else(|| KINDS.iter().find(|(k, _)| *k == kind).map(|(_, g)| *g))
            .unwrap_or(GROUP_MAX / 2)
    }

    /// Sets the group of `kind`, kept within the range. False for an
    /// unknown kind.
    pub fn set(&mut self, kind: &str, group: i32) -> bool {
        let group = group.clamp(GROUP_MIN, GROUP_MAX);
        if !KINDS.iter().any(|(k, _)| *k == kind) {
            return false;
        }
        match self.kinds.iter_mut().find(|k| k.kind == kind) {
            Some(k) => k.group = group,
            None => self.kinds.push(KindGroup {
                kind: kind.to_string(),
                group,
            }),
        }
        true
    }

    /// Every kind back at its starting group.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Nothing differs from the starting groups.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The kind an object draws under.
    pub fn kind_of(floor: &Floor, r: ObjectRef) -> &'static str {
        match r {
            ObjectRef::Wall(_) => WALLS,
            ObjectRef::Opening(_) => "Doors and Windows",
            ObjectRef::Dimension(_) => "Dimensions",
            ObjectRef::Cad(id) => {
                let is_text = floor
                    .cad
                    .iter()
                    .any(|c| c.id == id && matches!(c.item, CadItem::Text { .. }));
                if is_text {
                    TEXT
                } else {
                    CAD
                }
            }
            ObjectRef::Symbol(_) | ObjectRef::Cabinet(_) => "Cabinets and Symbols",
            ObjectRef::Stair(_) => "Stairs",
            ObjectRef::Device(_) => "Electrical",
            ObjectRef::RoofPlane(_) => "Roofs",
            ObjectRef::Foundation(_) => "Foundation",
            ObjectRef::Framing(_) => "Framing",
            ObjectRef::Detail(_) => "Material Regions",
            ObjectRef::Schedule(_) => "Schedules",
            ObjectRef::Block(_) => "Cabinets and Symbols",
            ObjectRef::Solid(_) => "Material Regions",
            ObjectRef::Camera(_) => "Cameras",
        }
    }
}

/// An object whose drawing group is its own.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GroupEntry {
    pub object: ObjectRef,
    pub group: i32,
}

impl Floor {
    /// The drawing group set on `r` itself, if any.
    pub fn drawing_group_override(&self, r: ObjectRef) -> Option<i32> {
        self.drawing_groups
            .iter()
            .find(|e| e.object == r)
            .map(|e| e.group)
    }

    /// The drawing group `r` draws in: its own, else its kind's.
    pub fn drawing_group(&self, table: &DrawingGroupTable, r: ObjectRef) -> i32 {
        self.drawing_group_override(r)
            .unwrap_or_else(|| table.group_of(DrawingGroupTable::kind_of(self, r)))
    }

    /// Gives `r` a group of its own, or `None` to put it back in its kind's.
    pub fn set_drawing_group(&mut self, r: ObjectRef, group: Option<i32>) {
        self.drawing_groups.retain(|e| e.object != r);
        if let Some(g) = group {
            self.drawing_groups.push(GroupEntry {
                object: r,
                group: g.clamp(GROUP_MIN, GROUP_MAX),
            });
        }
    }

    /// Forgets the entries of objects that are gone.
    pub fn prune_drawing_groups(&mut self) -> usize {
        let before = self.drawing_groups.len();
        let keep: Vec<GroupEntry> = self
            .drawing_groups
            .iter()
            .copied()
            .filter(|e| self.object_exists(e.object))
            .collect();
        self.drawing_groups = keep;
        before - self.drawing_groups.len()
    }

    /// Does the floor hold `r`? Only the kinds the core model stores can be
    /// checked; the opaque kinds are kept (the editor prunes those).
    fn object_exists(&self, r: ObjectRef) -> bool {
        match r {
            ObjectRef::Wall(id) => self.walls.iter().any(|w| w.id == id),
            ObjectRef::Opening(id) => self.openings.iter().any(|o| o.id == id),
            ObjectRef::Dimension(id) => self.dimensions.iter().any(|d| d.id == id),
            ObjectRef::Cad(id) => self.cad.iter().any(|c| c.id == id),
            ObjectRef::Symbol(id) => self.symbols.iter().any(|s| s.id == id),
            _ => true,
        }
    }

    /// The group of every CAD object, in the order of `self.cad` (one pass:
    /// the objects' kind is read off the object itself).
    fn cad_groups(&self, table: &DrawingGroupTable) -> Vec<i32> {
        let (cad, text) = (table.group_of(CAD), table.group_of(TEXT));
        let own: std::collections::HashMap<crate::model::Id, i32> = self
            .drawing_groups
            .iter()
            .filter_map(|e| match e.object {
                ObjectRef::Cad(id) => Some((id, e.group)),
                _ => None,
            })
            .collect();
        self.cad
            .iter()
            .map(|c| {
                own.get(&c.id).copied().unwrap_or(match c.item {
                    CadItem::Text { .. } => text,
                    _ => cad,
                })
            })
            .collect()
    }

    /// The CAD objects in drawing order: by group, then the order they were
    /// made in.
    pub fn cad_draw_order_with(&self, table: &DrawingGroupTable) -> Vec<&CadObject> {
        let mut v: Vec<(i32, &CadObject)> = self
            .cad_groups(table)
            .into_iter()
            .zip(self.cad.iter())
            .collect();
        v.sort_by_key(|(g, _)| *g);
        v.into_iter().map(|(_, c)| c).collect()
    }

    /// [`Floor::cad_draw_order_with`] with the starting groups, for the
    /// places that have no project table at hand (the sheet PDF).
    pub fn cad_draw_order(&self) -> Vec<&CadObject> {
        self.cad_draw_order_with(&DrawingGroupTable::default())
    }

    /// The CAD objects that draw behind the walls and those that draw over
    /// them, each in drawing order.
    pub fn cad_by_walls(&self, table: &DrawingGroupTable) -> (Vec<&CadObject>, Vec<&CadObject>) {
        let walls = table.group_of(WALLS);
        let mut v: Vec<(i32, &CadObject)> = self
            .cad_groups(table)
            .into_iter()
            .zip(self.cad.iter())
            .collect();
        v.sort_by_key(|(g, _)| *g);
        let (behind, front): (Vec<_>, Vec<_>) = v.into_iter().partition(|(g, _)| *g < walls);
        (
            behind.into_iter().map(|(_, c)| c).collect(),
            front.into_iter().map(|(_, c)| c).collect(),
        )
    }

    /// The lowest and highest group among the objects of the floor (the core
    /// kinds), not counting `except`.
    pub fn group_span(
        &self,
        table: &DrawingGroupTable,
        except: &[ObjectRef],
    ) -> Option<(i32, i32)> {
        let mut all: Vec<ObjectRef> = Vec::new();
        all.extend(self.walls.iter().map(|w| ObjectRef::Wall(w.id)));
        all.extend(self.openings.iter().map(|o| ObjectRef::Opening(o.id)));
        all.extend(self.dimensions.iter().map(|d| ObjectRef::Dimension(d.id)));
        all.extend(self.cad.iter().map(|c| ObjectRef::Cad(c.id)));
        all.extend(self.symbols.iter().map(|s| ObjectRef::Symbol(s.id)));
        let groups: Vec<i32> = all
            .into_iter()
            .filter(|r| !except.contains(r))
            .map(|r| self.drawing_group(table, r))
            .collect();
        Some((*groups.iter().min()?, *groups.iter().max()?))
    }
}

impl Project {
    /// Sets the group of each of `refs` on `floor` (`None`: back to the
    /// kind's). Returns how many changed.
    pub fn set_drawing_groups(
        &mut self,
        floor: usize,
        refs: &[ObjectRef],
        group: Option<i32>,
    ) -> usize {
        let table = self.drawing_group_defaults.clone();
        let Some(f) = self.floors.get_mut(floor) else {
            return 0;
        };
        let mut n = 0;
        for r in refs {
            let before = f.drawing_group(&table, *r);
            let had = f.drawing_group_override(*r);
            f.set_drawing_group(*r, group);
            if f.drawing_group(&table, *r) != before || had.is_some() != group.is_some() {
                n += 1;
            }
        }
        n
    }

    /// Bring to Front: each of `refs` gets a group above every other object
    /// of the floor (one above the highest). Returns how many changed.
    pub fn drawing_group_to_front(&mut self, floor: usize, refs: &[ObjectRef]) -> usize {
        let table = self.drawing_group_defaults.clone();
        let Some(span) = self
            .floors
            .get(floor)
            .and_then(|f| f.group_span(&table, refs))
        else {
            return self.set_drawing_groups(floor, refs, Some(GROUP_MAX));
        };
        let target = (span.1 + 1).min(GROUP_MAX);
        let mut n = 0;
        for r in refs {
            let f = &mut self.floors[floor];
            if f.drawing_group(&table, *r) != target || f.drawing_group_override(*r).is_none() {
                f.set_drawing_group(*r, Some(target));
                n += 1;
            }
        }
        n
    }

    /// Send to Back: each of `refs` gets a group below every other object of
    /// the floor (one below the lowest, never under 0).
    pub fn drawing_group_to_back(&mut self, floor: usize, refs: &[ObjectRef]) -> usize {
        let table = self.drawing_group_defaults.clone();
        let Some(span) = self
            .floors
            .get(floor)
            .and_then(|f| f.group_span(&table, refs))
        else {
            return self.set_drawing_groups(floor, refs, Some(GROUP_MIN));
        };
        let target = (span.0 - 1).max(GROUP_MIN);
        let mut n = 0;
        for r in refs {
            let f = &mut self.floors[floor];
            if f.drawing_group(&table, *r) != target || f.drawing_group_override(*r).is_none() {
                f.set_drawing_group(*r, Some(target));
                n += 1;
            }
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::WallKind;

    fn plan() -> (Project, Vec<u64>) {
        let mut p = Project::new("g");
        let w = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        let a = p.add_cad(
            0,
            "CAD, Default",
            CadItem::Circle {
                center: Point::ZERO,
                radius: 5.0,
            },
        );
        let b = p.add_cad(
            0,
            "CAD, Default",
            CadItem::Circle {
                center: Point::ZERO,
                radius: 6.0,
            },
        );
        let t = p.add_cad(
            0,
            "CAD, Default",
            CadItem::Text {
                pos: Point::ZERO,
                text: "x".into(),
                height: 4.0,
                angle: 0.0,
            },
        );
        (p, vec![w, a, b, t])
    }

    #[test]
    fn kinds_start_in_their_groups_and_cad_order_is_unchanged() {
        let (p, ids) = plan();
        let t = &p.drawing_group_defaults;
        let f = &p.floors[0];
        assert_eq!(f.drawing_group(t, ObjectRef::Wall(ids[0])), 50);
        assert_eq!(f.drawing_group(t, ObjectRef::Cad(ids[1])), 21);
        assert_eq!(
            f.drawing_group(t, ObjectRef::Cad(ids[3])),
            80,
            "text has its own group"
        );
        let order: Vec<u64> = f.cad_draw_order().iter().map(|c| c.id).collect();
        assert_eq!(order, vec![ids[1], ids[2], ids[3]]);
        let (behind, front) = f.cad_by_walls(t);
        assert_eq!(behind.len(), 2, "CAD at 21 sits behind the walls at 50");
        assert_eq!(front.len(), 1, "text at 80 sits in front");
    }

    #[test]
    fn front_and_back_move_objects_across_the_others() {
        let (mut p, ids) = plan();
        let r = ObjectRef::Cad(ids[1]);
        assert_eq!(p.drawing_group_to_front(0, &[r]), 1);
        let t = p.drawing_group_defaults.clone();
        assert_eq!(p.floors[0].drawing_group(&t, r), 81);
        let order: Vec<u64> = p.floors[0].cad_draw_order().iter().map(|c| c.id).collect();
        assert_eq!(
            order,
            vec![ids[2], ids[3], ids[1]],
            "the first circle is now last"
        );
        // Already in front: nothing changes.
        assert_eq!(p.drawing_group_to_front(0, &[r]), 0);
        assert_eq!(p.drawing_group_to_back(0, &[r]), 1);
        assert_eq!(
            p.floors[0].drawing_group(&t, r),
            20,
            "below the other circle's 21"
        );
        let (behind, front) = p.floors[0].cad_by_walls(&t);
        assert_eq!(
            behind.iter().map(|c| c.id).collect::<Vec<_>>(),
            vec![ids[1], ids[2]]
        );
        assert_eq!(front.len(), 1);
    }

    #[test]
    fn set_group_clamps_and_none_clears() {
        let (mut p, ids) = plan();
        let r = ObjectRef::Cad(ids[2]);
        assert_eq!(p.set_drawing_groups(0, &[r], Some(5000)), 1);
        let t = p.drawing_group_defaults.clone();
        assert_eq!(p.floors[0].drawing_group(&t, r), GROUP_MAX);
        assert_eq!(p.set_drawing_groups(0, &[r], None), 1);
        assert_eq!(p.floors[0].drawing_group_override(r), None);
        assert!(p.floors[0].drawing_groups.is_empty());
    }

    #[test]
    fn the_kind_table_moves_a_whole_kind() {
        let (mut p, ids) = plan();
        assert!(p.drawing_group_defaults.set("CAD", 60));
        assert!(!p.drawing_group_defaults.set("Nonsense", 20));
        let t = p.drawing_group_defaults.clone();
        let (behind, front) = p.floors[0].cad_by_walls(&t);
        assert!(behind.is_empty(), "the circles now sit over the walls");
        assert_eq!(front.len(), 3);
        assert_eq!(front[0].id, ids[1]);
        p.drawing_group_defaults.reset();
        assert!(p.drawing_group_defaults.is_default());
    }

    #[test]
    fn groups_survive_a_save_and_old_files_load() {
        let (mut p, ids) = plan();
        p.set_drawing_groups(0, &[ObjectRef::Cad(ids[1])], Some(12));
        p.drawing_group_defaults.set("Walls", 60);
        let json = serde_json::to_string(&p).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        let t = back.drawing_group_defaults.clone();
        assert_eq!(back.floors[0].drawing_group(&t, ObjectRef::Cad(ids[1])), 12);
        assert_eq!(t.group_of("Walls"), 60);
        // A project saved before the field existed has the starting table.
        let mut v: serde_json::Value = serde_json::to_value(Project::new("old")).unwrap();
        v.as_object_mut().unwrap().remove("drawing_group_defaults");
        v["floors"][0]
            .as_object_mut()
            .unwrap()
            .remove("drawing_groups");
        let old: Project = serde_json::from_value(v).unwrap();
        assert!(old.drawing_group_defaults.is_default());
        assert!(old.floors[0].drawing_groups.is_empty());
    }

    #[test]
    fn pruning_forgets_deleted_objects() {
        let (mut p, ids) = plan();
        p.set_drawing_groups(
            0,
            &[ObjectRef::Cad(ids[1]), ObjectRef::Cad(ids[2])],
            Some(3),
        );
        p.remove_cad(0, ids[1]);
        assert_eq!(p.floors[0].prune_drawing_groups(), 1);
        assert_eq!(p.floors[0].drawing_groups.len(), 1);
    }
}
