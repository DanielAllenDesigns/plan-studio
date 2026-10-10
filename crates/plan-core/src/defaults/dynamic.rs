//! Dynamic defaults and Set as Default (manual pp. 103 to 104).
//!
//! An object whose settings are "Use Default" follows its defaults dialog: when
//! the default changes the object takes the new value. An object whose group
//! of settings was edited does not. Chief marks the state per *group* of
//! settings in the specification dialogs (a Use Default radio button, a
//! Default check box or a wrench in the field).
//!
//! Two stores carry the state, and [`DefaultsRef`] reads both:
//!
//! * doors and windows keep a [`crate::openings::UseDefault`] flag per group
//!   on the opening itself (the Opening Specification owns them);
//! * every other kind keeps an explicit state in [`FollowTable`], a slot of
//!   [`super::saved::SavedDefaults`]. An object with no entry for a group
//!   follows the default as long as its value is still the old default's
//!   value (the rule cabinets use), so an object made from a default follows
//!   without anyone having to record it; an entry makes the state explicit
//!   (a dialog's Use Default radio sets it, Set as Default clears it).
//!
//! [`Project::follow_wall_defaults`] is the follower of walls. Doors and
//! windows follow through [`Project::follow_type_defaults`], cabinets through
//! `plan-app`'s `apply_dynamic_defaults` and electrical devices through the
//! device heights; `plan-app`'s `plan_defaults::follow_changed_defaults` runs
//! them all after any default changed.

use super::{PlanDefaults, WallDefaults};
use crate::model::{Project, WallKind};
use crate::walls::WallClass;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The kinds of object that have a defaults dialog and can follow it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DefaultKind {
    ExteriorWall,
    InteriorWall,
    FoundationWall,
    Door,
    Window,
    Cabinet,
    Room,
    Stairs,
    Roof,
    Electrical,
}

impl DefaultKind {
    pub const ALL: [DefaultKind; 10] = [
        DefaultKind::ExteriorWall,
        DefaultKind::InteriorWall,
        DefaultKind::FoundationWall,
        DefaultKind::Door,
        DefaultKind::Window,
        DefaultKind::Cabinet,
        DefaultKind::Room,
        DefaultKind::Stairs,
        DefaultKind::Roof,
        DefaultKind::Electrical,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DefaultKind::ExteriorWall => "Exterior Wall",
            DefaultKind::InteriorWall => "Interior Wall",
            DefaultKind::FoundationWall => "Foundation Wall",
            DefaultKind::Door => "Door",
            DefaultKind::Window => "Window",
            DefaultKind::Cabinet => "Cabinet",
            DefaultKind::Room => "Room",
            DefaultKind::Stairs => "Stairs",
            DefaultKind::Roof => "Roof",
            DefaultKind::Electrical => "Electrical",
        }
    }

    /// The groups of settings that can follow the default, as `(id, label)`.
    pub fn groups(self) -> &'static [(&'static str, &'static str)] {
        match self {
            DefaultKind::ExteriorWall | DefaultKind::InteriorWall | DefaultKind::FoundationWall => {
                &[("type", "Wall Type and Thickness"), ("height", "Height")]
            }
            DefaultKind::Door | DefaultKind::Window => &[
                ("type", "Type"),
                ("casing", "Casing"),
                ("lintel", "Lintel"),
                ("sash", "Sash"),
                ("frame", "Frame"),
                ("jamb", "Jamb"),
                ("hardware", "Hardware"),
                ("treatments", "Treatments"),
                ("framing", "Framing"),
                ("rough", "Rough Opening"),
                ("materials", "Materials"),
            ],
            DefaultKind::Cabinet => &[
                ("countertop", "Countertop"),
                ("toe_kick", "Toe Kick"),
                ("backsplash", "Backsplash"),
            ],
            DefaultKind::Room => &[("heights", "Floor and Ceiling Heights")],
            DefaultKind::Stairs => &[("size", "Riser, Tread and Width")],
            DefaultKind::Roof => &[("pitch", "Pitch"), ("overhang", "Overhang")],
            DefaultKind::Electrical => &[("height", "Height")],
        }
    }

    /// Does the kind have a group named `group`?
    pub fn has_group(self, group: &str) -> bool {
        self.groups().iter().any(|(g, _)| *g == group)
    }
}

/// One group of settings of one kind: what the Use Default switches and
/// "Set as Default" address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefaultsRef {
    pub kind: DefaultKind,
    pub group: &'static str,
}

impl DefaultsRef {
    /// The reference for `group` of `kind`, if the kind has such a group.
    pub fn new(kind: DefaultKind, group: &str) -> Option<DefaultsRef> {
        kind.groups()
            .iter()
            .find(|(g, _)| *g == group)
            .map(|(g, _)| DefaultsRef { kind, group: g })
    }

    /// Every group of `kind`.
    pub fn all(kind: DefaultKind) -> Vec<DefaultsRef> {
        kind.groups()
            .iter()
            .map(|(g, _)| DefaultsRef { kind, group: g })
            .collect()
    }
}

/// The explicit Use Default state per object and group of the kinds that do
/// not carry it themselves.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FollowTable(BTreeMap<u64, BTreeMap<String, bool>>);

impl FollowTable {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The recorded state of `group` of object `id`: `Some(true)` is Use
    /// Default, `Some(false)` is the object's own value, `None` is not
    /// recorded.
    pub fn state(&self, id: u64, group: &str) -> Option<bool> {
        self.0.get(&id).and_then(|m| m.get(group)).copied()
    }

    /// Records the state of `group` of object `id`.
    pub fn set(&mut self, id: u64, group: &str, use_default: bool) {
        self.0
            .entry(id)
            .or_default()
            .insert(group.to_string(), use_default);
    }

    /// Forgets what is recorded for `group` of object `id`.
    pub fn clear(&mut self, id: u64, group: &str) {
        if let Some(m) = self.0.get_mut(&id) {
            m.remove(group);
            if m.is_empty() {
                self.0.remove(&id);
            }
        }
    }

    /// Forgets everything recorded for object `id`.
    pub fn clear_object(&mut self, id: u64) {
        self.0.remove(&id);
    }

    /// Keeps the entries of the objects `live` accepts.
    pub fn retain_objects(&mut self, live: &dyn Fn(u64) -> bool) {
        self.0.retain(|id, _| live(*id));
    }

    /// Does the object follow after a default changed from a value the object
    /// has (`still_old_value`) to a new one? An explicit state wins.
    pub fn follows(&self, id: u64, group: &str, still_old_value: bool) -> bool {
        self.state(id, group).unwrap_or(still_old_value)
    }

    /// The number of objects with a recorded state.
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

/// The defaults governing a wall of `class` and `kind`, the kind they are
/// the defaults of and the thickness a wall type that is not in the list
/// gets. Walls with no defaults dialog (railing, pony, glass, ...) have none.
fn wall_defaults_of<'a>(
    d: &'a PlanDefaults,
    class: &WallClass,
    kind: WallKind,
) -> Option<(DefaultKind, &'a WallDefaults, f64)> {
    match class {
        WallClass::Foundation => Some((
            DefaultKind::FoundationWall,
            &d.foundation_wall,
            super::FALLBACK_FOUNDATION_THICKNESS,
        )),
        WallClass::Standard => Some(match kind {
            WallKind::Exterior => (
                DefaultKind::ExteriorWall,
                &d.exterior_wall,
                crate::DEFAULT_EXTERIOR_THICKNESS,
            ),
            WallKind::Interior => (
                DefaultKind::InteriorWall,
                &d.interior_wall,
                crate::DEFAULT_INTERIOR_THICKNESS,
            ),
        }),
        _ => None,
    }
}

impl Project {
    /// Dynamic wall defaults: after the wall defaults changed from `old` to
    /// `new`, every wall that follows takes the new wall type (and so its
    /// thickness) and the new height. A wall follows a group when its state
    /// is recorded as Use Default, or when nothing is recorded and its value
    /// is still the old default's. Returns how many walls changed.
    pub fn follow_wall_defaults(&mut self, old: &PlanDefaults, new: &PlanDefaults) -> usize {
        let mut changed = 0;
        let table = self.saved_defaults.follow.clone();
        for f in &mut self.floors {
            for w in &mut f.walls {
                let Some((_, was, was_fallback)) = wall_defaults_of(old, &w.class, w.kind) else {
                    continue;
                };
                let Some((_, now, now_fallback)) = wall_defaults_of(new, &w.class, w.kind) else {
                    continue;
                };
                let mut touched = false;
                if was.wall_type != now.wall_type {
                    let same = w.wall_type.as_deref() == Some(was.wall_type.as_str())
                        || (w.wall_type.is_none()
                            && (w.thickness - old.thickness_of(was, was_fallback)).abs() < 1e-6);
                    if table.follows(w.id, "type", same) {
                        w.wall_type = Some(now.wall_type.clone());
                        w.thickness = new.thickness_of(now, now_fallback);
                        touched = true;
                    }
                }
                if (was.height - now.height).abs() > 1e-9 {
                    let same = (w.height - was.height).abs() < 1e-6;
                    if table.follows(w.id, "height", same) {
                        w.height = now.height;
                        touched = true;
                    }
                }
                if touched {
                    changed += 1;
                }
            }
        }
        changed
    }

    /// Does `group` of object `id` of `kind` follow its default? Doors and
    /// windows answer from their own flags; the other kinds from the
    /// [`FollowTable`], where an unrecorded group follows (a new object uses
    /// the default).
    pub fn group_follows(&self, kind: DefaultKind, id: u64, group: &str) -> bool {
        if matches!(kind, DefaultKind::Door | DefaultKind::Window) {
            return self
                .floors
                .iter()
                .find_map(|f| f.openings.iter().find(|o| o.id == id))
                .is_some_and(|o| {
                    use crate::openings::DynGroup;
                    DynGroup::of_kind(o.kind)
                        .iter()
                        .find(|g| dyn_group_id(**g) == group)
                        .is_some_and(|g| o.extras.spec.dynamic.get(*g))
                });
        }
        self.saved_defaults.follow.state(id, group).unwrap_or(true)
    }

    /// Sets Use Default for `group` of object `id`; `false` when the object
    /// or group does not exist.
    pub fn set_group_follows(&mut self, kind: DefaultKind, id: u64, group: &str, on: bool) -> bool {
        if !kind.has_group(group) {
            return false;
        }
        if matches!(kind, DefaultKind::Door | DefaultKind::Window) {
            use crate::openings::DynGroup;
            for f in &mut self.floors {
                if let Some(o) = f.openings.iter_mut().find(|o| o.id == id) {
                    let Some(g) = DynGroup::of_kind(o.kind)
                        .iter()
                        .copied()
                        .find(|g| dyn_group_id(*g) == group)
                    else {
                        return false;
                    };
                    o.extras.spec.dynamic.set(g, on);
                    return true;
                }
            }
            return false;
        }
        self.saved_defaults.follow.set(id, group, on);
        true
    }

    /// Set as Default for a wall (the Edit toolbar button, manual p. 104):
    /// the wall's type, thickness and height become the defaults of its kind
    /// and the wall goes back to Use Default. Returns the kind changed, or
    /// `None` for a wall that has no defaults dialog (railing, pony, glass).
    pub fn set_wall_as_default(
        &mut self,
        d: &mut PlanDefaults,
        wall_id: u64,
    ) -> Option<DefaultKind> {
        let w = self
            .floors
            .iter()
            .find_map(|f| f.walls.iter().find(|w| w.id == wall_id))?
            .clone();
        let (kind, _, _) = wall_defaults_of(d, &w.class, w.kind)?;
        let slot = match kind {
            DefaultKind::FoundationWall => &mut d.foundation_wall,
            DefaultKind::InteriorWall => &mut d.interior_wall,
            _ => &mut d.exterior_wall,
        };
        if let Some(t) = &w.wall_type {
            slot.wall_type = t.clone();
        }
        slot.height = w.height;
        self.saved_defaults.follow.clear_object(wall_id);
        Some(kind)
    }

    /// Reset to Defaults for walls (W-121, manual pp. 118-119): the walls in
    /// `ids` on `floor` (every wall of the floor when `ids` is empty) take
    /// the wall type, thickness and height of their kind's defaults again
    /// and go back to Use Default. Walls without a defaults dialog (railing,
    /// pony, glass) are left alone. Returns how many walls changed.
    pub fn reset_walls_to_defaults(
        &mut self,
        d: &PlanDefaults,
        floor: usize,
        ids: &[u64],
    ) -> usize {
        let Some(f) = self.floors.get(floor) else {
            return 0;
        };
        let targets: Vec<u64> = f
            .walls
            .iter()
            .filter(|w| ids.is_empty() || ids.contains(&w.id))
            .map(|w| w.id)
            .collect();
        let mut changed = 0;
        for id in targets {
            let Some(w) = self.floors[floor].wall(id).cloned() else {
                continue;
            };
            let Some((_, wd, fallback)) = wall_defaults_of(d, &w.class, w.kind) else {
                continue;
            };
            if let Some(def) = d.wall_type(&wd.wall_type).cloned() {
                if self.wall_type_def(&wd.wall_type).is_none() {
                    self.register_wall_type(def);
                }
            }
            let typed = d.wall_type(&wd.wall_type).is_some();
            let thickness = d.thickness_of(wd, fallback);
            let Some(m) = self.floors[floor].wall_mut(id) else {
                continue;
            };
            let before = (m.wall_type.clone(), m.thickness, m.height);
            if typed {
                m.wall_type = Some(wd.wall_type.clone());
            }
            m.thickness = thickness;
            m.height = wd.height;
            if before != (m.wall_type.clone(), m.thickness, m.height) {
                changed += 1;
            }
            self.saved_defaults.follow.clear_object(id);
        }
        changed
    }
}

/// The id of a door or window group in the reference tables.
pub fn dyn_group_id(g: crate::openings::DynGroup) -> &'static str {
    use crate::openings::DynGroup;
    match g {
        DynGroup::Type => "type",
        DynGroup::Casing => "casing",
        DynGroup::Lintel => "lintel",
        DynGroup::Sash => "sash",
        DynGroup::Frame => "frame",
        DynGroup::Jamb => "jamb",
        DynGroup::Hardware => "hardware",
        DynGroup::Treatments => "treatments",
        DynGroup::Framing => "framing",
        DynGroup::Rough => "rough",
        DynGroup::Materials => "materials",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    fn plan() -> (Project, PlanDefaults) {
        let d = PlanDefaults::chief_x18_daniel();
        let mut p = Project::from_defaults("T", &d);
        for (a, b, k) in [
            (
                Point::new(0.0, 0.0),
                Point::new(120.0, 0.0),
                WallKind::Exterior,
            ),
            (
                Point::new(120.0, 0.0),
                Point::new(120.0, 120.0),
                WallKind::Exterior,
            ),
            (
                Point::new(0.0, 60.0),
                Point::new(120.0, 60.0),
                WallKind::Interior,
            ),
        ] {
            let wd = d.walls_for(k);
            let id = p.add_wall(0, a, b, d.thickness_of(wd, 6.0), wd.height, k);
            p.floors[0].wall_mut(id).unwrap().wall_type = Some(wd.wall_type.clone());
        }
        (p, d)
    }

    #[test]
    fn the_group_tables_cover_every_kind() {
        for k in DefaultKind::ALL {
            assert!(!k.groups().is_empty(), "{k:?}");
            for r in DefaultsRef::all(k) {
                assert!(k.has_group(r.group));
            }
        }
        assert!(DefaultsRef::new(DefaultKind::Door, "casing").is_some());
        assert!(DefaultsRef::new(DefaultKind::Door, "nope").is_none());
    }

    #[test]
    fn walls_that_use_the_default_follow_a_changed_default_and_edited_ones_do_not() {
        let (mut p, d) = plan();
        let ids: Vec<u64> = p.floors[0].walls.iter().map(|w| w.id).collect();
        // The second exterior wall was edited to its own height.
        p.floors[0].walls[1].height = 144.0;
        let mut now = d.clone();
        now.exterior_wall.height = 120.0;
        let n = p.follow_wall_defaults(&d, &now);
        assert_eq!(n, 1);
        assert_eq!(p.floors[0].wall(ids[0]).unwrap().height, 120.0, "follows");
        assert_eq!(
            p.floors[0].wall(ids[1]).unwrap().height,
            144.0,
            "kept its own"
        );
        assert_eq!(
            p.floors[0].wall(ids[2]).unwrap().height,
            d.interior_wall.height,
            "an interior wall is another default"
        );
        // An explicit Use Default makes even the edited wall follow.
        assert!(p.set_group_follows(DefaultKind::ExteriorWall, ids[1], "height", true));
        let mut later = now.clone();
        later.exterior_wall.height = 100.0;
        p.follow_wall_defaults(&now, &later);
        assert_eq!(p.floors[0].wall(ids[1]).unwrap().height, 100.0);
        // An explicit own value stops a wall that still matches.
        assert!(p.set_group_follows(DefaultKind::ExteriorWall, ids[0], "height", false));
        let mut last = later.clone();
        last.exterior_wall.height = 96.0;
        p.follow_wall_defaults(&later, &last);
        // It followed the second change (100) while it used the default; the
        // third (96) found it on its own value.
        assert_eq!(p.floors[0].wall(ids[0]).unwrap().height, 100.0);
        assert!(!p.group_follows(DefaultKind::ExteriorWall, ids[0], "height"));
        assert!(p.group_follows(DefaultKind::ExteriorWall, ids[2], "height"));
    }

    #[test]
    fn a_changed_wall_type_brings_its_thickness() {
        let (mut p, d) = plan();
        let mut now = d.clone();
        now.exterior_wall.wall_type = "Brick-6".into();
        let n = p.follow_wall_defaults(&d, &now);
        assert_eq!(n, 2, "both exterior walls");
        let w = &p.floors[0].walls[0];
        assert_eq!(w.wall_type.as_deref(), Some("Brick-6"));
        assert_eq!(w.thickness, now.thickness_of(&now.exterior_wall, 0.0));
    }

    #[test]
    fn set_as_default_copies_the_wall_and_returns_it_to_use_default() {
        let (mut p, mut d) = plan();
        let id = p.floors[0].walls[0].id;
        p.floors[0].walls[0].height = 130.0;
        p.floors[0].walls[0].wall_type = Some("Siding-6".into());
        p.saved_defaults.follow.set(id, "height", false);
        let kind = p.set_wall_as_default(&mut d, id).unwrap();
        assert_eq!(kind, DefaultKind::ExteriorWall);
        assert_eq!(d.exterior_wall.height, 130.0);
        assert_eq!(d.exterior_wall.wall_type, "Siding-6");
        assert!(p.saved_defaults.follow.is_empty());
        // A railing wall has no defaults dialog.
        p.floors[0].walls[2].class = WallClass::Railing;
        let rid = p.floors[0].walls[2].id;
        assert!(p.set_wall_as_default(&mut d, rid).is_none());
        assert!(p.set_wall_as_default(&mut d, 9999).is_none());
    }

    #[test]
    fn the_follow_table_resolves_explicit_state_before_the_value_rule() {
        let mut t = FollowTable::default();
        assert!(t.follows(1, "height", true));
        assert!(!t.follows(1, "height", false));
        t.set(1, "height", false);
        assert!(!t.follows(1, "height", true));
        t.set(1, "height", true);
        assert!(t.follows(1, "height", false));
        t.clear(1, "height");
        assert!(t.is_empty());
        t.set(2, "type", true);
        t.set(3, "type", true);
        t.retain_objects(&|id| id == 3);
        assert_eq!(t.len(), 1);
    }
}
