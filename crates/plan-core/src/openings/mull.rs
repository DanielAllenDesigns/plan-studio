//! Automatic mulling, Make Mulled Unit, window levels and stacked openings
//! (DW-51, DW-52, DW-158..DW-161; manual pp. 608 to 611).
//!
//! * **Automatic mulling**: windows and doors whose casings touch share one
//!   casing ([`auto_mulled`]). It is derived from the positions, never stored:
//!   the openings stay separate objects for dimensions and the Materials List.
//!   The gap between two of them is the Minimum Separation, and it is the width
//!   of the casing they share ([`shared_casing_width`]).
//! * **Make Mulled Unit** ([`Project::make_mulled_unit`]): openings of one wall
//!   within 24 in of each other, side to side *or* top to bottom, become a
//!   blocked unit whose settings are a [`MulledSpec`]. Complex units come from
//!   mulling several blocks together. [`Project::explode_mulled_unit`] breaks it
//!   up again.
//! * **Levels** ([`pick_order`]): stacked openings can sit on Window Levels;
//!   level 0 is drawn in the layer colour and picked first.
//! * **Caution** ([`Project::stacked_clusters`]): four or more openings in one
//!   place, and [`Project::delete_duplicate`] to thin them out.

use super::{vertically_apart, OpeningStyle, MULL_DOOR_STYLES};
use crate::model::{Id, Opening, OpeningKind, Project};
use serde::{Deserialize, Serialize};

/// The most two components of a mulled unit may be apart, inches.
pub const UNIT_REACH: f64 = 24.0;
/// How many openings in one place raise the Caution symbol.
pub const CAUTION_COUNT: usize = 4;

// ----- the Mulled Unit specification -----

/// How a mulled unit is labelled and counted (Label panel, manual p. 611).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MulledLabel {
    /// One label for the whole unit, which is one object in schedules.
    #[default]
    Single,
    /// The labels of the components, counted one by one.
    Components,
    /// No labels; the components are counted one by one.
    Suppress,
}

impl MulledLabel {
    pub const ALL: [MulledLabel; 3] = [
        MulledLabel::Single,
        MulledLabel::Components,
        MulledLabel::Suppress,
    ];

    pub fn name(self) -> &'static str {
        match self {
            MulledLabel::Single => "Show Single Label for Entire Unit",
            MulledLabel::Components => "Show Component Labels",
            MulledLabel::Suppress => "Suppress All Labels",
        }
    }
}

/// The Mulled Unit Specification and Defaults (manual pp. 609 to 611).
/// Stored on every component of the unit; the Mulled Unit Defaults hold the
/// values a new unit starts with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MulledSpec {
    /// Counted and drawn as a door: the Doors layer and label layer, the
    /// Doors category of the Materials List, "Mulled Door Units" in schedules.
    /// On from the moment a door is a component.
    pub treat_as_door: bool,
    /// One hole in the wall around the whole unit instead of one per
    /// component.
    pub single_hole: bool,
    /// Mullion depth from the interior face and from the exterior face, inches.
    pub mullion_inside: f64,
    pub mullion_outside: f64,
    pub label: MulledLabel,
    /// The layer of the unit; `None` for the Windows (or Doors) layer.
    pub layer: Option<String>,
    /// How the unit was built: components side by side, stacked, or several
    /// blocks mulled together (read only).
    pub arrangement: MulledArrangement,
    /// Counts the edits of the unit's specification. The components each hold
    /// a copy; the copy with the highest revision is the unit's and
    /// [`Project::sync_unit_specs`] gives it to the others.
    pub revision: u32,
}

impl Default for MulledSpec {
    fn default() -> Self {
        Self {
            treat_as_door: false,
            single_hole: false,
            mullion_inside: 1.5,
            mullion_outside: 1.5,
            label: MulledLabel::Single,
            layer: None,
            arrangement: MulledArrangement::Horizontal,
            revision: 0,
        }
    }
}

/// How the blocks of a unit are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MulledArrangement {
    /// Side by side.
    #[default]
    Horizontal,
    /// Stacked, top to bottom.
    Vertical,
    /// Blocks mulled together in the other direction.
    Complex,
}

impl MulledSpec {
    /// The schedule category of the unit (manual p. 611).
    pub fn schedule_category(&self) -> &'static str {
        if self.treat_as_door {
            "Mulled Door Units"
        } else {
            "Mulled Window Units"
        }
    }
}

// ----- automatic mulling -----

/// How far the casing of `o` reaches beyond its jamb, inches: the casing's
/// width and reveal, or nothing when both faces have no casing.
pub fn casing_reach_of(o: &Opening) -> f64 {
    let s = &o.extras.spec;
    if !s.casing_interior && !s.casing_exterior {
        return 0.0;
    }
    let c = o.casing.unwrap_or_default();
    let ext = s.casing_exterior_size.unwrap_or(c);
    let w = match (s.casing_interior, s.casing_exterior) {
        (true, true) => (c.width + c.reveal).max(ext.width + ext.reveal),
        (true, false) => c.width + c.reveal,
        _ => ext.width + ext.reveal,
    };
    w.max(0.0)
}

/// The clear width between the jambs of `a` and `b` along their wall,
/// negative when they overlap sideways.
pub fn gap_between(a: &Opening, b: &Opening) -> f64 {
    if a.center_offset <= b.center_offset {
        b.start_offset() - a.end_offset()
    } else {
        a.start_offset() - b.end_offset()
    }
}

/// Whether the casings of `a` and `b` touch: the same wall, a gap no wider than
/// the two casings together, and some height in common.
pub fn casings_touch(a: &Opening, b: &Opening) -> bool {
    a.id != b.id && a.wall_id == b.wall_id && !vertically_apart(a, b) && {
        let gap = gap_between(a, b);
        gap > -1e-6 && gap <= casing_reach_of(a) + casing_reach_of(b) + 1e-6
    }
}

/// Whether a piece of a unit can be mulled automatically: not a niche, a
/// projecting unit or a door that slides, folds or rolls.
fn auto_mullable(o: &Opening) -> bool {
    match o.kind {
        OpeningKind::Window => !matches!(o.style, OpeningStyle::WallNiche) && !o.style.projects(),
        OpeningKind::Door => MULL_DOOR_STYLES.contains(&o.style),
    }
}

/// Whether `a` and `b` mull automatically (manual p. 608): their casings touch
/// and their sills are equal; a window mulls to a door when its bottom is at
/// the door's, the floor.
pub fn auto_mulled(a: &Opening, b: &Opening) -> bool {
    if !auto_mullable(a) || !auto_mullable(b) || !casings_touch(a, b) {
        return false;
    }
    match (a.kind, b.kind) {
        (OpeningKind::Window, OpeningKind::Window) => (a.sill_height - b.sill_height).abs() < 0.01,
        (OpeningKind::Door, OpeningKind::Window) => b.sill_height <= a.sill_height + 0.01,
        (OpeningKind::Window, OpeningKind::Door) => a.sill_height <= b.sill_height + 0.01,
        (OpeningKind::Door, OpeningKind::Door) => false,
    }
}

/// The width of the casing `a` and `b` share, inches: the gap between them,
/// which the Minimum Separation keeps from closing. `None` when they do not
/// mull automatically.
pub fn shared_casing_width(a: &Opening, b: &Opening) -> Option<f64> {
    auto_mulled(a, b).then(|| gap_between(a, b).max(0.0))
}

/// The automatically mulled group `o` is part of among `siblings` (openings of
/// one wall, `o` included or not), ordered along the wall.
pub fn auto_group<'a>(o: &'a Opening, siblings: &'a [&'a Opening]) -> Vec<&'a Opening> {
    let mut group: Vec<&Opening> = vec![o];
    let mut grew = true;
    while grew {
        grew = false;
        for s in siblings {
            if group.iter().any(|g| g.id == s.id) {
                continue;
            }
            if group.iter().any(|g| auto_mulled(g, s)) {
                group.push(s);
                grew = true;
            }
        }
    }
    group.sort_by(|a, b| a.center_offset.total_cmp(&b.center_offset));
    group
}

// ----- levels and stacks -----

/// The openings of `wall_id` that cover the wall offset `s`, in the order a
/// click picks them: Window Level 0 first, then the other levels, the lowest
/// sill first within a level.
pub fn pick_order(openings: &[Opening], wall_id: Id, s: f64) -> Vec<Id> {
    let mut v: Vec<&Opening> = openings
        .iter()
        .filter(|o| o.wall_id == wall_id && o.start_offset() <= s && s <= o.end_offset())
        .collect();
    v.sort_by(|a, b| {
        a.extras
            .spec
            .level
            .cmp(&b.extras.spec.level)
            .then(a.sill_height.total_cmp(&b.sill_height))
            .then(a.id.cmp(&b.id))
    });
    v.into_iter().map(|o| o.id).collect()
}

/// A place where [`CAUTION_COUNT`] or more openings share the same space.
#[derive(Debug, Clone, PartialEq)]
pub struct StackCluster {
    pub wall_id: Id,
    /// Where along the wall the Caution symbol stands: the middle of the
    /// stretch the cluster covers.
    pub at: f64,
    pub ids: Vec<Id>,
}

/// Whether two openings of one wall overlap in both width and height.
fn overlaps_in_space(a: &Opening, b: &Opening) -> bool {
    a.wall_id == b.wall_id
        && a.start_offset() < b.end_offset() - 1e-6
        && a.end_offset() > b.start_offset() + 1e-6
        && !vertically_apart(a, b)
        && a.sill_height < b.sill_height + b.height - 1e-6
        && a.sill_height + a.height > b.sill_height + 1e-6
}

impl crate::model::Floor {
    /// The boxes the wall `wall_id` has to cut for the mulled units whose
    /// settings ask for a Single Wall Hole: one box around each such unit, in
    /// wall offsets and heights. The wall builder adds them to the holes of the
    /// components, so the stretch between the components opens too.
    pub fn single_hole_boxes(&self, wall_id: Id) -> Vec<UnitHole> {
        let mut seen: Vec<Id> = Vec::new();
        let mut out = Vec::new();
        for o in self.openings_on(wall_id) {
            let (Some(g), Some(spec)) = (o.mull_group, o.extras.spec.mulled.as_ref()) else {
                continue;
            };
            if !spec.single_hole || seen.contains(&g) {
                continue;
            }
            seen.push(g);
            let hull = self
                .openings_on(wall_id)
                .filter(|m| m.mull_group == Some(g))
                .map(|m| UnitHole {
                    s0: m.start_offset(),
                    s1: m.end_offset(),
                    h0: m.sill_height,
                    h1: m.sill_height + m.height,
                })
                .reduce(|a, b| UnitHole {
                    s0: a.s0.min(b.s0),
                    s1: a.s1.max(b.s1),
                    h0: a.h0.min(b.h0),
                    h1: a.h1.max(b.h1),
                });
            out.extend(hull);
        }
        out
    }
}

/// The span a unit covers, as `(s0, s1, h0, h1)` in wall offset and height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnitHole {
    pub s0: f64,
    pub s1: f64,
    pub h0: f64,
    pub h1: f64,
}

impl Project {
    /// The openings that mull automatically with `id` (itself excluded),
    /// ordered along the wall.
    pub fn auto_mull_members(&self, floor: usize, id: Id) -> Vec<Id> {
        let f = &self.floors[floor];
        let Some(me) = f.openings.iter().find(|o| o.id == id) else {
            return Vec::new();
        };
        let sibs: Vec<&Opening> = f.openings_on(me.wall_id).collect();
        auto_group(me, &sibs)
            .into_iter()
            .filter(|o| o.id != id)
            .map(|o| o.id)
            .collect()
    }

    /// Everything the casing of `id` is shared with: its blocked unit and the
    /// openings that mull to it automatically, with the span they cover.
    pub fn casing_group(&self, floor: usize, id: Id) -> Vec<Id> {
        let f = &self.floors[floor];
        let mut ids = self.mull_members(floor, id);
        let mut i = 0;
        while i < ids.len() {
            for m in self.auto_mull_members(floor, ids[i]) {
                if !ids.contains(&m) {
                    ids.push(m);
                    for u in self.mull_members(floor, m) {
                        if !ids.contains(&u) {
                            ids.push(u);
                        }
                    }
                }
            }
            i += 1;
        }
        ids.retain(|i| f.openings.iter().any(|o| o.id == *i));
        ids
    }

    /// The span along the wall that the shared casing of `id` goes around,
    /// when it is shared with anything.
    pub fn casing_span(&self, floor: usize, id: Id) -> Option<(f64, f64)> {
        let ids = self.casing_group(floor, id);
        if ids.len() < 2 {
            return None;
        }
        let f = &self.floors[floor];
        ids.iter()
            .filter_map(|i| f.openings.iter().find(|o| o.id == *i))
            .map(|o| (o.start_offset(), o.end_offset()))
            .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)))
    }

    /// The hole a mulled unit cuts when its settings ask for a Single Wall
    /// Hole: the box around every component.
    pub fn unit_hole(&self, floor: usize, id: Id) -> Option<UnitHole> {
        let f = &self.floors[floor];
        let members = self.mull_members(floor, id);
        let boxes = members
            .iter()
            .filter_map(|i| f.openings.iter().find(|o| o.id == *i))
            .map(|o| UnitHole {
                s0: o.start_offset(),
                s1: o.end_offset(),
                h0: o.sill_height,
                h1: o.sill_height + o.height,
            });
        boxes.reduce(|a, b| UnitHole {
            s0: a.s0.min(b.s0),
            s1: a.s1.max(b.s1),
            h0: a.h0.min(b.h0),
            h1: a.h1.max(b.h1),
        })
    }

    /// [`crate::model::Floor::single_hole_boxes`] of floor `floor`.
    pub fn single_hole_boxes(&self, floor: usize, wall_id: Id) -> Vec<UnitHole> {
        self.floors[floor].single_hole_boxes(wall_id)
    }

    /// The spec of the unit `id` belongs to, `None` for an opening on its own.
    pub fn mulled_spec(&self, floor: usize, id: Id) -> Option<&MulledSpec> {
        self.floors[floor]
            .openings
            .iter()
            .find(|o| o.id == id)
            .and_then(|o| o.extras.spec.mulled.as_ref())
    }

    /// Gives every component of each unit the specification with the highest
    /// revision among them (the one edited last). Returns how many components
    /// changed.
    pub fn sync_unit_specs(&mut self) -> usize {
        let mut changed = 0;
        for f in &mut self.floors {
            let mut groups: Vec<Id> = f.openings.iter().filter_map(|o| o.mull_group).collect();
            groups.sort_unstable();
            groups.dedup();
            for g in groups {
                let best = f
                    .openings
                    .iter()
                    .filter(|o| o.mull_group == Some(g))
                    .filter_map(|o| o.extras.spec.mulled.as_ref().map(|m| (m.revision, o.id, m)))
                    .min_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)))
                    .map(|(_, _, m)| m.clone());
                let Some(best) = best else {
                    continue;
                };
                for o in f.openings.iter_mut().filter(|o| o.mull_group == Some(g)) {
                    if o.extras.spec.mulled.as_ref() != Some(&best) {
                        o.extras.spec.mulled = Some(best.clone());
                        changed += 1;
                    }
                }
            }
        }
        changed
    }

    /// Sets the spec of the whole unit `id` belongs to. Returns how many
    /// components took it.
    pub fn set_mulled_spec(&mut self, floor: usize, id: Id, spec: &MulledSpec) -> usize {
        let members = self.mull_members(floor, id);
        let mut n = 0;
        for o in &mut self.floors[floor].openings {
            if members.contains(&o.id) && o.mull_group.is_some() {
                o.extras.spec.mulled = Some(spec.clone());
                n += 1;
            }
        }
        n
    }

    /// The components of the unit `id` belongs to, lowest level first and then
    /// left to right (the order Select Next Object walks them).
    pub fn unit_components(&self, floor: usize, id: Id) -> Vec<Id> {
        let f = &self.floors[floor];
        let mut v: Vec<&Opening> = self
            .mull_members(floor, id)
            .iter()
            .filter_map(|i| f.openings.iter().find(|o| o.id == *i))
            .collect();
        v.sort_by(|a, b| {
            a.sill_height
                .total_cmp(&b.sill_height)
                .then(a.center_offset.total_cmp(&b.center_offset))
        });
        v.into_iter().map(|o| o.id).collect()
    }

    /// Select Next Object (manual pp. 258, 610, 612): the next opening in the
    /// stack under `current`'s middle, in pick order, wrapping round. `None`
    /// when `current` is alone there.
    pub fn select_next_opening(&self, floor: usize, current: Id) -> Option<Id> {
        let f = &self.floors[floor];
        let cur = f.openings.iter().find(|o| o.id == current)?;
        let order = pick_order(&f.openings, cur.wall_id, cur.center_offset);
        if order.len() < 2 {
            return None;
        }
        let i = order.iter().position(|x| *x == current)?;
        Some(order[(i + 1) % order.len()])
    }

    /// Make Mulled Unit (manual pp. 609 to 610): blocks `ids` (and the units
    /// they already belong to) into one unit. The components must stand in the
    /// same wall, no more than [`UNIT_REACH`] apart side to side or top to
    /// bottom, and run either side by side or stacked; a mixture is built by
    /// mulling blocks together. The unit takes `defaults`, treated as a door
    /// when a door is a component. Nothing moves. Returns the unit's group id.
    pub fn make_mulled_unit(
        &mut self,
        floor: usize,
        ids: &[Id],
        defaults: &MulledSpec,
    ) -> Result<Id, String> {
        let f = &self.floors[floor];
        let mut all: Vec<Id> = Vec::new();
        for id in ids {
            if !f.openings.iter().any(|o| o.id == *id) {
                return Err("That opening is gone".into());
            }
            for m in self.mull_members(floor, *id) {
                if !all.contains(&m) {
                    all.push(m);
                }
            }
        }
        if all.len() < 2 {
            return Err("Select at least two doors or windows to block into a unit".into());
        }
        let parts: Vec<&Opening> = all
            .iter()
            .filter_map(|i| f.openings.iter().find(|o| o.id == *i))
            .collect();
        let wall_id = parts[0].wall_id;
        if parts.iter().any(|o| o.wall_id != wall_id) {
            return Err("The components must be in the same wall".into());
        }
        // The existing units are blocks; an opening on its own is a block.
        let mut blocks: Vec<Vec<&Opening>> = Vec::new();
        for o in &parts {
            match o.mull_group {
                Some(g) => match blocks
                    .iter_mut()
                    .find(|b| b.first().is_some_and(|x| x.mull_group == Some(g)))
                {
                    Some(b) => b.push(o),
                    None => blocks.push(vec![o]),
                },
                None => blocks.push(vec![o]),
            }
        }
        let boxes: Vec<UnitHole> = blocks
            .iter()
            .map(|b| {
                b.iter()
                    .map(|o| UnitHole {
                        s0: o.start_offset(),
                        s1: o.end_offset(),
                        h0: o.sill_height,
                        h1: o.sill_height + o.height,
                    })
                    .reduce(|a, c| UnitHole {
                        s0: a.s0.min(c.s0),
                        s1: a.s1.max(c.s1),
                        h0: a.h0.min(c.h0),
                        h1: a.h1.max(c.h1),
                    })
                    .expect("a block has a member")
            })
            .collect();
        if parts.iter().any(|o| {
            matches!(
                o.style,
                OpeningStyle::BayWindow | OpeningStyle::BowWindow | OpeningStyle::BoxWindow
            )
        }) {
            return Err("A bay, box or bow window cannot be part of a mulled unit".into());
        }
        if parts.iter().any(|o| {
            o.kind == OpeningKind::Window && o.is_shaped() && o.extras.spec.shape.sides_slant()
        }) {
            return Err("The edges that face one another must be straight and parallel".into());
        }
        let arrangement = arrangement_of(&boxes)?;
        // Nothing else may stand between the components.
        let (first, last) = boxes.iter().fold((f64::MAX, f64::MIN), |(lo, hi), b| {
            (lo.min(b.s0), hi.max(b.s1))
        });
        let (bottom, top) = boxes.iter().fold((f64::MAX, f64::MIN), |(lo, hi), b| {
            (lo.min(b.h0), hi.max(b.h1))
        });
        let outside_all = |x: &Opening| {
            boxes.iter().all(|b| {
                x.start_offset() >= b.s1 - 1e-6
                    || x.end_offset() <= b.s0 + 1e-6
                    || x.sill_height >= b.h1 - 1e-6
                    || x.sill_height + x.height <= b.h0 + 1e-6
            })
        };
        if f.openings_on(wall_id).any(|x| {
            !all.contains(&x.id)
                && x.start_offset() < last - 1e-6
                && x.end_offset() > first + 1e-6
                && x.sill_height < top - 1e-6
                && x.sill_height + x.height > bottom + 1e-6
                && outside_all(x)
        }) {
            return Err("Another opening is in the way".into());
        }
        let treat_as_door = parts.iter().any(|o| o.kind == OpeningKind::Door);
        let group = parts
            .iter()
            .map(|o| o.id)
            .min()
            .expect("at least two parts");
        let mut spec = defaults.clone();
        spec.treat_as_door = spec.treat_as_door || treat_as_door;
        spec.arrangement = if blocks.iter().any(|b| b.len() > 1) && blocks.len() > 1 {
            // Blocks mulled together: in the direction they run, a unit made
            // of units is a complex one when the blocks differ in direction.
            let inner_vertical = blocks.iter().any(|b| {
                b.len() > 1
                    && b.iter().any(|o| {
                        o.extras
                            .spec
                            .mulled
                            .as_ref()
                            .is_some_and(|m| m.arrangement != arrangement)
                    })
            });
            if inner_vertical {
                MulledArrangement::Complex
            } else {
                arrangement
            }
        } else {
            arrangement
        };
        for o in &mut self.floors[floor].openings {
            if all.contains(&o.id) {
                o.mull_group = Some(group);
                o.extras.spec.mulled = Some(spec.clone());
            }
        }
        Ok(group)
    }

    /// Explode Mulled Unit (manual p. 611): the unit `id` belongs to becomes
    /// separate windows and doors again. Returns how many components were
    /// released.
    pub fn explode_mulled_unit(&mut self, floor: usize, id: Id) -> usize {
        let members = self.mull_members(floor, id);
        let mut n = 0;
        for o in &mut self.floors[floor].openings {
            if members.contains(&o.id) && o.mull_group.take().is_some() {
                o.extras.spec.mulled = None;
                n += 1;
            }
        }
        n
    }

    /// The places of `floor` where [`CAUTION_COUNT`] or more openings share
    /// the same space (the Caution symbol, manual p. 609).
    pub fn stacked_clusters(&self, floor: usize) -> Vec<StackCluster> {
        let f = &self.floors[floor];
        let mut seen: Vec<Id> = Vec::new();
        let mut out = Vec::new();
        for o in &f.openings {
            if seen.contains(&o.id) {
                continue;
            }
            let mut group: Vec<&Opening> = vec![o];
            let mut i = 0;
            while i < group.len() {
                let cur = group[i];
                for n in &f.openings {
                    if !group.iter().any(|g| g.id == n.id) && overlaps_in_space(cur, n) {
                        group.push(n);
                    }
                }
                i += 1;
            }
            seen.extend(group.iter().map(|g| g.id));
            if group.len() >= CAUTION_COUNT {
                let lo = group
                    .iter()
                    .map(|g| g.start_offset())
                    .fold(f64::MAX, f64::min);
                let hi = group
                    .iter()
                    .map(|g| g.end_offset())
                    .fold(f64::MIN, f64::max);
                let mut ids: Vec<Id> = group.iter().map(|g| g.id).collect();
                ids.sort_unstable();
                out.push(StackCluster {
                    wall_id: o.wall_id,
                    at: (lo + hi) * 0.5,
                    ids,
                });
            }
        }
        out
    }

    /// Delete Duplicate (manual p. 609): removes the newest opening of the
    /// Caution cluster at `id`, leaving the rest. Returns the id removed, or
    /// `None` when `id` is in no cluster.
    pub fn delete_duplicate(&mut self, floor: usize, id: Id) -> Option<Id> {
        let cluster = self
            .stacked_clusters(floor)
            .into_iter()
            .find(|c| c.ids.contains(&id))?;
        let victim = *cluster.ids.iter().max()?;
        self.floors[floor].openings.retain(|o| o.id != victim);
        Some(victim)
    }
}

/// How boxes (blocks) are laid out: side by side or stacked, each within
/// [`UNIT_REACH`] of the next. A mixture is refused.
fn arrangement_of(boxes: &[UnitHole]) -> Result<MulledArrangement, String> {
    let overlap = |a0: f64, a1: f64, b0: f64, b1: f64| a1.min(b1) - a0.max(b0);
    let n = boxes.len();
    // Side by side: no two share width, every pair of neighbours shares some
    // height and is within reach.
    let mut by_s: Vec<&UnitHole> = boxes.iter().collect();
    by_s.sort_by(|a, b| a.s0.total_cmp(&b.s0));
    let side_by_side = by_s.windows(2).all(|w| {
        let gap = w[1].s0 - w[0].s1;
        gap > -1e-6
            && gap <= UNIT_REACH + 1e-6
            && overlap(w[0].h0, w[0].h1, w[1].h0, w[1].h1) > 1e-6
    });
    let mut by_h: Vec<&UnitHole> = boxes.iter().collect();
    by_h.sort_by(|a, b| a.h0.total_cmp(&b.h0));
    let stacked = by_h.windows(2).all(|w| {
        let gap = w[1].h0 - w[0].h1;
        gap > -1e-6
            && gap <= UNIT_REACH + 1e-6
            && overlap(w[0].s0, w[0].s1, w[1].s0, w[1].s1) > 1e-6
    });
    if n >= 2 && side_by_side {
        Ok(MulledArrangement::Horizontal)
    } else if n >= 2 && stacked {
        Ok(MulledArrangement::Vertical)
    } else {
        Err(format!(
            "Components must be within {} of one another, side to side or top to bottom, but not both; block one direction first and then mull the blocks together",
            crate::units::fmt_ft_in(UNIT_REACH)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::WallKind;

    fn plan() -> (Project, Id) {
        let mut p = Project::new("t");
        let w = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(400.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        (p, w)
    }

    fn put(p: &mut Project, w: Id, kind: OpeningKind, c: f64, width: f64, sill: f64, h: f64) -> Id {
        let id = p.alloc_id();
        let mut o = Opening::new(w, c, kind, width, h, sill);
        o.id = id;
        p.floors[0].openings.push(o);
        id
    }

    fn get(p: &Project, id: Id) -> &Opening {
        p.floors[0].openings.iter().find(|o| o.id == id).unwrap()
    }

    #[test]
    fn casings_that_touch_mull_and_share_the_gap() {
        let (mut p, w) = plan();
        // Two 36 in windows 2 in apart: 3.75 in casings overlap.
        let a = put(&mut p, w, OpeningKind::Window, 100.0, 36.0, 30.0, 48.0);
        let b = put(&mut p, w, OpeningKind::Window, 138.0, 36.0, 30.0, 48.0);
        assert!(casings_touch(get(&p, a), get(&p, b)));
        assert!(auto_mulled(get(&p, a), get(&p, b)));
        assert_eq!(shared_casing_width(get(&p, a), get(&p, b)), Some(2.0));
        assert_eq!(p.auto_mull_members(0, a), vec![b]);
        assert_eq!(p.casing_span(0, a), Some((82.0, 156.0)));
        // 8 in apart: casings (3.75 + 3.75 = 7.5) do not touch.
        let c = put(&mut p, w, OpeningKind::Window, 212.0, 36.0, 30.0, 48.0);
        assert!(!casings_touch(get(&p, b), get(&p, c)));
        assert!(p.auto_mull_members(0, c).is_empty());
        // A chain: a third window within reach joins the group.
        let d = put(&mut p, w, OpeningKind::Window, 176.0, 36.0, 30.0, 48.0);
        let mut group = p.auto_mull_members(0, a);
        group.sort_unstable();
        assert_eq!(group, vec![b, c, d]);
        // Each window is still its own object.
        assert!(get(&p, a).mull_group.is_none());
    }

    #[test]
    fn sills_must_match_and_a_window_mulls_to_a_door_at_the_floor() {
        let (mut p, w) = plan();
        let a = put(&mut p, w, OpeningKind::Window, 100.0, 36.0, 30.0, 48.0);
        let b = put(&mut p, w, OpeningKind::Window, 138.0, 36.0, 36.0, 48.0);
        assert!(casings_touch(get(&p, a), get(&p, b)));
        assert!(!auto_mulled(get(&p, a), get(&p, b)), "sills differ");
        // A door: the window mulls to it only with its bottom at the floor.
        let d = put(&mut p, w, OpeningKind::Door, 220.0, 36.0, 0.0, 80.0);
        let hi = put(&mut p, w, OpeningKind::Window, 250.0, 24.0, 30.0, 40.0);
        let low = put(&mut p, w, OpeningKind::Window, 190.0, 24.0, 0.0, 40.0);
        // The window beside the door at sill 30 is vertically inside the door's
        // height but its bottom is not at the floor.
        assert!(!auto_mulled(get(&p, d), get(&p, hi)));
        assert!(auto_mulled(get(&p, d), get(&p, low)));
        // Two doors never mull.
        let d2 = put(&mut p, w, OpeningKind::Door, 300.0, 36.0, 0.0, 80.0);
        assert!(!auto_mulled(get(&p, d), get(&p, d2)));
    }

    #[test]
    fn make_mulled_unit_follows_the_24_inch_rule() {
        let (mut p, w) = plan();
        let a = put(&mut p, w, OpeningKind::Window, 100.0, 36.0, 30.0, 48.0);
        let b = put(&mut p, w, OpeningKind::Window, 160.0, 36.0, 30.0, 48.0); // 24 in gap
        let far = put(&mut p, w, OpeningKind::Window, 260.0, 36.0, 30.0, 48.0); // 64 in gap
        let spec = MulledSpec::default();
        assert!(p.make_mulled_unit(0, &[a, far], &spec).is_err());
        let g = p.make_mulled_unit(0, &[a, b], &spec).unwrap();
        assert_eq!(g, a.min(b));
        assert!(get(&p, a).extras.spec.mulled.is_some());
        assert_eq!(get(&p, a).mull_group, Some(g));
        // The components did not move.
        assert_eq!(get(&p, a).center_offset, 100.0);
        assert_eq!(get(&p, b).center_offset, 160.0);
        // The hole of the unit is the box around both.
        let hole = p.unit_hole(0, a).unwrap();
        assert_eq!(
            hole,
            UnitHole {
                s0: 82.0,
                s1: 178.0,
                h0: 30.0,
                h1: 78.0
            }
        );
        // Explode releases them.
        assert_eq!(p.explode_mulled_unit(0, a), 2);
        assert!(get(&p, a).mull_group.is_none() && get(&p, a).extras.spec.mulled.is_none());
    }

    #[test]
    fn a_single_wall_hole_unit_cuts_one_box_around_its_components() {
        let (mut p, w) = plan();
        let a = put(&mut p, w, OpeningKind::Window, 100.0, 36.0, 30.0, 48.0);
        let b = put(&mut p, w, OpeningKind::Window, 150.0, 36.0, 30.0, 48.0);
        let mut spec = MulledSpec::default();
        p.make_mulled_unit(0, &[a, b], &spec).unwrap();
        // Separate holes by default.
        assert!(p.single_hole_boxes(0, w).is_empty());
        spec.single_hole = true;
        spec.revision = 1;
        p.set_mulled_spec(0, a, &spec);
        let boxes = p.single_hole_boxes(0, w);
        assert_eq!(
            boxes,
            vec![UnitHole {
                s0: 82.0,
                s1: 168.0,
                h0: 30.0,
                h1: 78.0
            }]
        );
    }

    #[test]
    fn the_unit_takes_the_specification_edited_last() {
        let (mut p, w) = plan();
        let a = put(&mut p, w, OpeningKind::Window, 100.0, 36.0, 30.0, 48.0);
        let b = put(&mut p, w, OpeningKind::Window, 140.0, 36.0, 30.0, 48.0);
        p.make_mulled_unit(0, &[a, b], &MulledSpec::default())
            .unwrap();
        // A dialog edit of the second component, a higher revision.
        {
            let o = p.floors[0].openings.iter_mut().find(|o| o.id == b).unwrap();
            let m = o.extras.spec.mulled.as_mut().unwrap();
            m.single_hole = true;
            m.revision = 1;
        }
        assert_eq!(p.sync_unit_specs(), 1);
        for id in [a, b] {
            let m = get(&p, id).extras.spec.mulled.as_ref().unwrap();
            assert!(m.single_hole && m.revision == 1);
        }
        // Nothing left to do.
        assert_eq!(p.sync_unit_specs(), 0);
    }

    #[test]
    fn a_door_makes_the_unit_a_door_and_stacks_need_a_column() {
        let (mut p, w) = plan();
        let door = put(&mut p, w, OpeningKind::Door, 100.0, 36.0, 0.0, 80.0);
        let top = put(&mut p, w, OpeningKind::Window, 100.0, 36.0, 90.0, 20.0); // 10 in over the door
        let side = put(&mut p, w, OpeningKind::Window, 132.0, 20.0, 0.0, 60.0);
        // Door and transom: stacked.
        p.make_mulled_unit(0, &[door, top], &MulledSpec::default())
            .unwrap();
        let spec = p.mulled_spec(0, door).unwrap();
        assert!(spec.treat_as_door);
        assert_eq!(spec.arrangement, MulledArrangement::Vertical);
        assert_eq!(spec.schedule_category(), "Mulled Door Units");
        // A side window over the same selection mixes the directions.
        let raw = p.make_mulled_unit(0, &[door, side], &MulledSpec::default());
        // The side window is beside the (door + transom) block: blocks run
        // side by side, so this is a complex unit built from a stacked block.
        let g = raw.unwrap();
        assert_eq!(p.mull_members(0, door).len(), 3);
        let spec = p.mulled_spec(0, side).unwrap();
        assert_eq!(spec.arrangement, MulledArrangement::Complex);
        assert_eq!(get(&p, side).mull_group, Some(g));
    }

    #[test]
    fn a_mixture_of_directions_in_one_go_is_refused() {
        let (mut p, w) = plan();
        let a = put(&mut p, w, OpeningKind::Window, 100.0, 36.0, 0.0, 40.0);
        let b = put(&mut p, w, OpeningKind::Window, 140.0, 36.0, 0.0, 40.0);
        let c = put(&mut p, w, OpeningKind::Window, 100.0, 36.0, 50.0, 30.0);
        // a beside b and c over a: side by side AND stacked.
        assert!(p
            .make_mulled_unit(0, &[a, b, c], &MulledSpec::default())
            .is_err());
        // Block the column first, then mull the blocks.
        p.make_mulled_unit(0, &[a, c], &MulledSpec::default())
            .unwrap();
        assert!(p
            .make_mulled_unit(0, &[a, b], &MulledSpec::default())
            .is_ok());
    }

    #[test]
    fn level_zero_is_picked_first_and_select_next_walks_the_stack() {
        let (mut p, w) = plan();
        let a = put(&mut p, w, OpeningKind::Window, 100.0, 36.0, 30.0, 20.0);
        let b = put(&mut p, w, OpeningKind::Window, 100.0, 36.0, 60.0, 20.0);
        let c = put(&mut p, w, OpeningKind::Window, 100.0, 36.0, 0.0, 20.0);
        for (id, lvl) in [(a, 1), (b, 0), (c, 2)] {
            p.floors[0]
                .openings
                .iter_mut()
                .find(|o| o.id == id)
                .unwrap()
                .extras
                .spec
                .level = lvl;
        }
        let order = pick_order(&p.floors[0].openings, w, 100.0);
        assert_eq!(order, vec![b, a, c], "level 0, 1, 2");
        assert_eq!(p.select_next_opening(0, b), Some(a));
        assert_eq!(p.select_next_opening(0, a), Some(c));
        assert_eq!(p.select_next_opening(0, c), Some(b));
        // Off the stack: nothing to cycle to.
        let lone = put(&mut p, w, OpeningKind::Window, 300.0, 36.0, 30.0, 20.0);
        assert_eq!(p.select_next_opening(0, lone), None);
    }

    #[test]
    fn four_openings_in_one_place_raise_the_caution_and_delete_duplicate_thins_them() {
        let (mut p, w) = plan();
        let mut ids = Vec::new();
        for _ in 0..4 {
            ids.push(put(&mut p, w, OpeningKind::Window, 100.0, 36.0, 30.0, 48.0));
        }
        let lone = put(&mut p, w, OpeningKind::Window, 300.0, 36.0, 30.0, 48.0);
        let clusters = p.stacked_clusters(0);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].ids.len(), 4);
        assert_eq!(clusters[0].at, 100.0);
        assert!(!clusters[0].ids.contains(&lone));
        // Delete Duplicate takes the newest until no Caution remains.
        assert_eq!(p.delete_duplicate(0, ids[0]), Some(ids[3]));
        assert!(p.stacked_clusters(0).is_empty());
        assert_eq!(p.delete_duplicate(0, ids[0]), None);
        // Three in one place are not a problem.
        assert_eq!(p.floors[0].openings.len(), 4);
    }
}
