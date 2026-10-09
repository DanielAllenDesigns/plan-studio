//! CAD details (Chief's detail windows): a drawing of loose CAD in detail
//! space that is not part of the building.
//!
//! A CAD detail is stored as a [`Floor`] that carries a [`CadDetailInfo`] in
//! `Floor.detail`. Using a floor means the CAD tools, layers, dimensions,
//! blocks, undo and the plan tabs work on a detail without any special case;
//! the marker keeps the detail out of the building's floor stack: details
//! are always appended after the last floor, hold no walls, and the Project
//! Browser and CAD Detail Management list them apart from the floors.
//!
//! Details made from a section or elevation camera (Auto Detail) remember
//! the camera; their names follow the camera's name and plan callout number
//! until the user renames them ([`Project::sync_detail_names`]).

use crate::cad::CadItem;
use crate::geometry::Point;
use crate::model::{Floor, Id, Project};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Layer of the cut lines (the section poche outline) of a detail.
pub const DETAIL_CUT_LAYER: &str = "Detail, Cut";
/// Layer of the outlines and silhouettes of a detail.
pub const DETAIL_LINES_LAYER: &str = "Detail, Lines";
/// Layer of the material hatch of a detail.
pub const DETAIL_HATCH_LAYER: &str = "Detail, Hatch";
/// Layer of insulation batts.
pub const DETAIL_INSULATION_LAYER: &str = "Detail, Insulation";
/// Layer of framing members.
pub const DETAIL_FRAMING_LAYER: &str = "Detail, Framing";
/// Layer of detail components (flashing, sheathing, siding, anchors).
pub const DETAIL_COMPONENTS_LAYER: &str = "Detail, Components";
/// Layer of notes and labels.
pub const DETAIL_NOTES_LAYER: &str = "Detail, Notes";

/// Name, colour and plotted weight (1/100 mm) of the detail layers.
pub const DETAIL_LAYERS: [(&str, [u8; 3], u32); 7] = [
    (DETAIL_CUT_LAYER, [0, 0, 0], 50),
    (DETAIL_LINES_LAYER, [40, 40, 40], 25),
    (DETAIL_HATCH_LAYER, [120, 120, 120], 9),
    (DETAIL_INSULATION_LAYER, [190, 150, 30], 13),
    (DETAIL_FRAMING_LAYER, [140, 95, 50], 35),
    (DETAIL_COMPONENTS_LAYER, [60, 90, 140], 25),
    (DETAIL_NOTES_LAYER, [0, 0, 0], 13),
];

/// Default print scale of a detail: paper inches per foot (1 1/2" = 1'-0").
pub const DEFAULT_DETAIL_SCALE: f64 = 1.5;

/// What a CAD detail was made from.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum DetailSource {
    /// Drawn by hand (New Detail).
    #[default]
    Blank,
    /// Auto Detail of a section or elevation camera.
    Camera { camera: Id },
    /// CAD Detail From View: the lines of the plan floor of that name.
    PlanView { floor: String },
}

/// The marker of a detail floor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CadDetailInfo {
    pub source: DetailSource,
    /// The user typed this name: a camera rename or a new callout number
    /// leaves it alone.
    pub name_locked: bool,
    /// Print scale, paper inches per foot of the detail.
    pub scale: f64,
}

impl Default for CadDetailInfo {
    fn default() -> Self {
        Self {
            source: DetailSource::Blank,
            name_locked: false,
            scale: DEFAULT_DETAIL_SCALE,
        }
    }
}

impl CadDetailInfo {
    /// A detail made from `source`.
    pub fn from_source(source: DetailSource) -> Self {
        Self {
            source,
            ..Self::default()
        }
    }
}

impl Floor {
    /// Is this floor a CAD detail?
    pub fn is_cad_detail(&self) -> bool {
        self.detail.is_some()
    }

    /// Bounds of the detail's CAD objects, `None` when it is empty.
    pub fn cad_extent(&self) -> Option<(Point, Point)> {
        let mut it = self.cad.iter().map(|o| o.bounds());
        let first = it.next()?;
        Some(it.fold(first, |(lo, hi), (a, b)| {
            (
                Point::new(lo.x.min(a.x), lo.y.min(a.y)),
                Point::new(hi.x.max(b.x), hi.y.max(b.y)),
            )
        }))
    }
}

/// The automatic name of a detail made from a camera: the camera's name,
/// led by the number of its plan callout (the sheet caption reads the same).
pub fn auto_detail_name(camera_name: &str, callout: Option<u32>) -> String {
    let name = camera_name.trim();
    let name = if name.is_empty() { "Section" } else { name };
    match callout {
        Some(n) => format!("{n} - {name}"),
        None => name.to_string(),
    }
}

impl Project {
    /// Floor indices of the CAD details, in project order.
    pub fn cad_detail_floors(&self) -> Vec<usize> {
        (0..self.floors.len())
            .filter(|&i| self.floors[i].is_cad_detail())
            .collect()
    }

    /// `base`, or `base 2`, `base 3`... so that no other floor has the name.
    /// `except` is a floor that may keep its own name.
    pub fn unique_floor_name(&self, base: &str, except: Option<usize>) -> String {
        let base = base.trim();
        let base = if base.is_empty() { "CAD Detail" } else { base };
        let taken = |n: &str| {
            self.floors
                .iter()
                .enumerate()
                .any(|(i, f)| Some(i) != except && f.name == n)
        };
        if !taken(base) {
            return base.to_string();
        }
        (2..)
            .map(|n| format!("{base} {n}"))
            .find(|n| !taken(n))
            .unwrap_or_else(|| base.to_string())
    }

    /// Appends an empty CAD detail called `name` (made unique) after the last
    /// floor and returns its floor index. The detail layers are added to the
    /// plan's layer set when they are missing.
    pub fn add_cad_detail(&mut self, name: &str, info: CadDetailInfo) -> usize {
        let name = self.unique_floor_name(name, None);
        let last = self.floors.last();
        let mut floor = Floor::new(name, last.map_or(0.0, |f| f.elevation));
        if let Some(l) = last {
            floor.ceiling_height = l.ceiling_height;
        }
        floor.detail = Some(info);
        self.floors.push(floor);
        ensure_detail_layers(&mut self.layers);
        self.floors.len() - 1
    }

    /// Renames a detail (the name becomes the user's: it stays through
    /// callout renumbering). False for a floor that is not a detail or an
    /// empty name.
    pub fn rename_cad_detail(&mut self, idx: usize, name: &str) -> bool {
        if name.trim().is_empty() || !self.floors.get(idx).is_some_and(Floor::is_cad_detail) {
            return false;
        }
        let name = self.unique_floor_name(name, Some(idx));
        let f = &mut self.floors[idx];
        f.name = name;
        if let Some(d) = &mut f.detail {
            d.name_locked = true;
        }
        true
    }

    /// A copy of the detail with new ids everywhere (CAD objects, their
    /// styles, blocks, dimensions, symbols), appended after the last floor
    /// and named "<name> copy". Returns its index.
    pub fn duplicate_cad_detail(&mut self, idx: usize) -> Option<usize> {
        let mut f = self.floors.get(idx).filter(|f| f.is_cad_detail())?.clone();
        f.name = self.unique_floor_name(&format!("{} copy", f.name), None);
        if let Some(d) = &mut f.detail {
            d.name_locked = true;
        }
        let mut ids: HashMap<Id, Id> = HashMap::new();
        for o in &mut f.cad {
            let new = self.alloc_id();
            ids.insert(o.id, new);
            o.id = new;
        }
        for d in &mut f.dimensions {
            let new = self.alloc_id();
            ids.insert(d.id, new);
            d.id = new;
        }
        for s in &mut f.symbols {
            let new = self.alloc_id();
            ids.insert(s.id, new);
            s.id = new;
        }
        for s in &mut f.symbols {
            s.owner = s.owner.and_then(|o| ids.get(&o).copied());
        }
        for a in &mut f.cad_attrs {
            a.target = ids.get(&a.target).copied().unwrap_or(a.target);
            if let Some(fill) = &mut a.fill {
                fill.lines = fill
                    .lines
                    .iter()
                    .filter_map(|l| ids.get(l).copied())
                    .collect();
            }
        }
        let mut groups: HashMap<Id, Id> = HashMap::new();
        for g in &mut f.groups {
            let new = self.alloc_id();
            groups.insert(g.id, new);
            g.id = new;
            for m in &mut g.members {
                use crate::groups::ObjectRef as R;
                *m = match *m {
                    R::Cad(i) => R::Cad(ids.get(&i).copied().unwrap_or(i)),
                    R::Dimension(i) => R::Dimension(ids.get(&i).copied().unwrap_or(i)),
                    R::Symbol(i) => R::Symbol(ids.get(&i).copied().unwrap_or(i)),
                    other => other,
                };
            }
        }
        for b in &mut f.cad_blocks {
            b.group = groups.get(&b.group).copied().unwrap_or(b.group);
        }
        self.floors.push(f);
        Some(self.floors.len() - 1)
    }

    /// Deletes a detail and keeps the cameras and saved plan views that point
    /// at later floors pointing at them. False for a floor that is not a
    /// detail.
    pub fn delete_cad_detail(&mut self, idx: usize) -> bool {
        if !self.floors.get(idx).is_some_and(Floor::is_cad_detail) {
            return false;
        }
        self.floors.remove(idx);
        self.cameras.retain(|c| c.floor != idx);
        for c in self.cameras.iter_mut().filter(|c| c.floor > idx) {
            c.floor -= 1;
        }
        for v in &mut self.plan_views {
            v.floor = match v.floor {
                Some(f) if f == idx => None,
                Some(f) if f > idx => Some(f - 1),
                other => other,
            };
        }
        true
    }

    /// The name a camera-made detail should have now, or `None` for a detail
    /// that follows no camera (blank, from a plan view, or whose camera is
    /// gone) or whose name the user typed.
    pub fn expected_detail_name(&self, idx: usize) -> Option<String> {
        let d = self.floors.get(idx)?.detail.as_ref()?;
        if d.name_locked {
            return None;
        }
        let DetailSource::Camera { camera } = d.source else {
            return None;
        };
        let c = self.camera(camera)?;
        Some(auto_detail_name(&c.name, self.callout_number(camera)))
    }

    /// Does a camera-made detail carry a name that is out of date?
    pub fn detail_names_stale(&self) -> bool {
        self.cad_detail_floors().into_iter().any(|i| {
            self.expected_detail_name(i).is_some_and(|n| {
                self.floors[i].name != n && self.unique_floor_name(&n, Some(i)) == n
            })
        })
    }

    /// Gives every camera-made detail whose name the user never typed the
    /// name of its camera and callout. Returns how many were renamed.
    pub fn sync_detail_names(&mut self) -> usize {
        let mut n = 0;
        for i in self.cad_detail_floors() {
            let Some(want) = self.expected_detail_name(i) else {
                continue;
            };
            if self.floors[i].name == want {
                continue;
            }
            let unique = self.unique_floor_name(&want, Some(i));
            if unique == want {
                self.floors[i].name = unique;
                n += 1;
            }
        }
        n
    }
}

/// Adds the detail layers that `layers` lacks. True when any was added.
pub fn ensure_detail_layers(layers: &mut crate::layers::LayerSet) -> bool {
    let mut added = false;
    for (name, color, weight) in DETAIL_LAYERS {
        added |= layers.add(crate::layers::Layer::new(name, color, weight));
    }
    added
}

/// Moves the items so that the lower-left corner of their bounds is the
/// origin (detail space starts at 0,0).
pub fn normalize_to_origin(items: &mut [(String, CadItem)]) {
    let mut lo: Option<Point> = None;
    for (_, it) in items.iter() {
        let (a, _) = it.bounds();
        lo = Some(match lo {
            Some(l) => Point::new(l.x.min(a.x), l.y.min(a.y)),
            None => a,
        });
    }
    let Some(lo) = lo else {
        return;
    };
    let d = Point::new(-lo.x, -lo.y);
    for (_, it) in items.iter_mut() {
        *it = crate::cad::translated(it, d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{CameraKind, CameraObject};

    fn cam_mut(p: &mut Project, id: Id) -> &mut CameraObject {
        p.cameras.iter_mut().find(|c| c.id == id).unwrap()
    }

    fn detail_project() -> (Project, usize) {
        let mut p = Project::new("t");
        let i = p.add_cad_detail("Wall Detail", CadDetailInfo::default());
        (p, i)
    }

    #[test]
    fn a_detail_is_appended_after_the_floors_and_adds_its_layers() {
        let (p, i) = detail_project();
        assert_eq!(i, 1);
        assert!(p.floors[i].is_cad_detail());
        assert!(!p.floors[0].is_cad_detail());
        assert_eq!(p.cad_detail_floors(), vec![1]);
        for (name, _, _) in DETAIL_LAYERS {
            assert!(p.layers.get(name).is_some(), "{name}");
        }
    }

    #[test]
    fn build_new_floor_lands_under_the_details_not_after_them() {
        let (mut p, d) = detail_project();
        assert_eq!(d, 1);
        let at = p.build_new_floor_with(&crate::floors::NewFloorOptions::default());
        assert_eq!(at, Some(1), "the new floor goes above floor 0");
        assert!(!p.floors[1].is_cad_detail());
        assert!(p.floors[2].is_cad_detail(), "the detail stays last");
    }

    #[test]
    fn names_stay_unique_and_a_rename_locks_the_name() {
        let (mut p, a) = detail_project();
        let b = p.add_cad_detail("Wall Detail", CadDetailInfo::default());
        assert_eq!(p.floors[b].name, "Wall Detail 2");
        assert!(p.rename_cad_detail(b, "Sill"));
        assert_eq!(p.floors[b].name, "Sill");
        assert!(p.floors[b].detail.as_ref().unwrap().name_locked);
        // Renaming to its own name keeps it.
        assert!(p.rename_cad_detail(a, "Wall Detail"));
        assert_eq!(p.floors[a].name, "Wall Detail");
        assert!(!p.rename_cad_detail(0, "x"));
        assert!(!p.rename_cad_detail(a, "  "));
    }

    #[test]
    fn duplicating_gives_new_ids_to_objects_styles_and_blocks() {
        let (mut p, i) = detail_project();
        let a = p.add_cad(
            i,
            DETAIL_LINES_LAYER,
            CadItem::Line {
                a: Point::new(0.0, 0.0),
                b: Point::new(10.0, 0.0),
            },
        );
        let b = p.add_cad(
            i,
            DETAIL_LINES_LAYER,
            CadItem::Line {
                a: Point::new(0.0, 5.0),
                b: Point::new(10.0, 5.0),
            },
        );
        p.edit_cad_attrs(i, a, |at| at.color = Some([1, 2, 3]));
        let g = p.make_cad_block(i, &[a, b], Some("Pair")).unwrap();
        let j = p.duplicate_cad_detail(i).unwrap();
        assert_eq!(p.floors[j].name, "Wall Detail copy");
        let ids: Vec<Id> = p.floors[j].cad.iter().map(|o| o.id).collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.iter().all(|x| *x != a && *x != b));
        assert_eq!(p.floors[j].cad_attrs.len(), 1);
        assert!(ids.contains(&p.floors[j].cad_attrs[0].target));
        let block = &p.floors[j].cad_blocks[0];
        assert_ne!(block.group, g);
        assert_eq!(p.floors[j].group_members_cad(block.group).len(), 2);
        // The original is untouched.
        assert_eq!(p.floors[i].cad.len(), 2);
        assert_eq!(p.floors[i].cad_blocks[0].group, g);
    }

    #[test]
    fn deleting_a_detail_moves_the_floor_indices_of_cameras_and_views() {
        let (mut p, i) = detail_project();
        let j = p.add_cad_detail("Second", CadDetailInfo::default());
        let mut cam = CameraObject::new(
            CameraKind::CrossSection { back_clip: None },
            Point::ZERO,
            90.0,
            "A",
            j,
        );
        cam.floor = j;
        p.add_camera(cam);
        p.plan_views[0].floor = Some(j);
        assert!(p.delete_cad_detail(i));
        assert_eq!(p.floors.len(), 2);
        assert_eq!(p.floors[1].name, "Second");
        assert_eq!(p.cameras[0].floor, 1);
        assert_eq!(p.plan_views[0].floor, Some(1));
        assert!(!p.delete_cad_detail(0), "a real floor is not a detail");
    }

    #[test]
    fn camera_made_details_follow_the_camera_name_until_renamed() {
        let mut p = Project::new("t");
        let cam = CameraObject::new(
            CameraKind::CrossSection { back_clip: None },
            Point::ZERO,
            90.0,
            "Section A",
            0,
        );
        let cid = p.add_camera(cam);
        let n = p.callout_number(cid).unwrap();
        let name = auto_detail_name("Section A", Some(n));
        let i = p.add_cad_detail(
            &name,
            CadDetailInfo::from_source(DetailSource::Camera { camera: cid }),
        );
        assert_eq!(p.floors[i].name, format!("{n} - Section A"));
        assert!(!p.detail_names_stale());
        cam_mut(&mut p, cid).name = "Wall Section".into();
        assert!(p.detail_names_stale());
        assert_eq!(p.sync_detail_names(), 1);
        assert_eq!(p.floors[i].name, format!("{n} - Wall Section"));
        cam_mut(&mut p, cid).callout.number = Some(7);
        assert_eq!(p.sync_detail_names(), 1);
        assert_eq!(p.floors[i].name, "7 - Wall Section");
        // A typed name is left alone.
        assert!(p.rename_cad_detail(i, "Typical Wall"));
        cam_mut(&mut p, cid).name = "Other".into();
        assert!(!p.detail_names_stale());
        assert_eq!(p.sync_detail_names(), 0);
        assert_eq!(p.floors[i].name, "Typical Wall");
    }

    #[test]
    fn items_normalize_to_the_origin() {
        let mut items = vec![
            (
                "a".to_string(),
                CadItem::Line {
                    a: Point::new(10.0, 20.0),
                    b: Point::new(30.0, 20.0),
                },
            ),
            (
                "a".to_string(),
                CadItem::Line {
                    a: Point::new(15.0, 25.0),
                    b: Point::new(15.0, 40.0),
                },
            ),
        ];
        normalize_to_origin(&mut items);
        let CadItem::Line { a, .. } = &items[0].1 else {
            panic!()
        };
        assert_eq!((a.x, a.y), (0.0, 0.0));
    }
}
