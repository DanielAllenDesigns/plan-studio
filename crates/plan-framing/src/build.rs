//! The Build Framing dialog's data: which groups of framing a build makes,
//! which it leaves alone (Retain Framing), which rebuild by themselves, and
//! the defaults of the Posts and Trusses tabs.
//!
//! Everything here is plain data and pure functions, so the rules are
//! testable without an editor: [`BuildOptions::keeps`] says whether an
//! existing member survives a rebuild, [`merge_rebuild`] applies it, and
//! [`fingerprint`] gives the stable hash the auto-rebuild compares.

use crate::manual::{FramingMaterial, LumberSize, MemberKind as ManualKind};
use crate::member::{Member, MemberKind};
use crate::truss::TrussType;
use plan_core::Id;
use serde::{Deserialize, Serialize};

/// The four groups of Chief's Build Framing dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Group {
    Floor,
    Ceiling,
    Wall,
    Roof,
}

impl Group {
    pub const ALL: [Group; 4] = [Group::Floor, Group::Ceiling, Group::Wall, Group::Roof];

    pub fn name(self) -> &'static str {
        match self {
            Group::Floor => "Floor",
            Group::Ceiling => "Ceiling",
            Group::Wall => "Wall",
            Group::Roof => "Roof",
        }
    }

    fn index(self) -> usize {
        match self {
            Group::Floor => 0,
            Group::Ceiling => 1,
            Group::Wall => 2,
            Group::Roof => 3,
        }
    }
}

/// The group an automatic member belongs to. Blocking is wall blocking when it
/// belongs to a wall and floor blocking otherwise.
pub fn group_of(m: &Member) -> Group {
    match m.kind {
        MemberKind::Stud
        | MemberKind::KingStud
        | MemberKind::TrimmerStud
        | MemberKind::CrippleStud
        | MemberKind::CornerStud
        | MemberKind::TeeStud
        | MemberKind::TopPlate
        | MemberKind::BottomPlate
        | MemberKind::Header
        | MemberKind::Sill => Group::Wall,
        MemberKind::Blocking => {
            if m.wall_id.is_some() {
                Group::Wall
            } else {
                Group::Floor
            }
        }
        MemberKind::RimJoist
        | MemberKind::Joist
        | MemberKind::TrimmerJoist
        | MemberKind::HeaderJoist
        | MemberKind::Ledger => Group::Floor,
        MemberKind::CeilingJoist => Group::Ceiling,
        MemberKind::Rafter
        | MemberKind::Ridge
        | MemberKind::Hip
        | MemberKind::Valley
        | MemberKind::Fascia
        | MemberKind::CollarTie
        | MemberKind::TrussTopChord
        | MemberKind::TrussBottomChord
        | MemberKind::TrussWeb => Group::Roof,
    }
}

/// The group of a manually placed or layout-built member kind.
pub fn group_of_manual(kind: ManualKind) -> Group {
    match kind {
        ManualKind::Rafter
        | ManualKind::RoofBeam
        | ManualKind::RoofBlocking
        | ManualKind::RoofPurlin
        | ManualKind::RoofTruss
        | ManualKind::GirderTruss
        | ManualKind::TrussBase => Group::Roof,
        ManualKind::Joist
        | ManualKind::JoistBlocking
        | ManualKind::FloorCeilingBeam
        | ManualKind::FloorCeilingTruss => Group::Floor,
        _ => Group::Wall,
    }
}

/// One flag per [`Group`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GroupFlags {
    pub floor: bool,
    pub ceiling: bool,
    pub wall: bool,
    pub roof: bool,
}

impl GroupFlags {
    pub const NONE: GroupFlags = GroupFlags {
        floor: false,
        ceiling: false,
        wall: false,
        roof: false,
    };

    pub fn get(&self, g: Group) -> bool {
        match g {
            Group::Floor => self.floor,
            Group::Ceiling => self.ceiling,
            Group::Wall => self.wall,
            Group::Roof => self.roof,
        }
    }

    pub fn set(&mut self, g: Group, on: bool) {
        match g {
            Group::Floor => self.floor = on,
            Group::Ceiling => self.ceiling = on,
            Group::Wall => self.wall = on,
            Group::Roof => self.roof = on,
        }
    }

    pub fn any(&self) -> bool {
        Group::ALL.iter().any(|g| self.get(*g))
    }
}

impl Default for GroupFlags {
    fn default() -> Self {
        Self::NONE
    }
}

/// The Trusses tab: the truss Build Framing lays over each Truss Base.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrussDefaults {
    pub kind: TrussType,
    /// Top chord pitch, rise per 12.
    pub pitch: f64,
    /// Heel height above the bearing plane, inches.
    pub heel_height: f64,
    /// Overhang of the top chord past the bearing, inches.
    pub overhang: f64,
    /// On-centre spacing, inches.
    pub spacing: f64,
}

impl Default for TrussDefaults {
    fn default() -> Self {
        Self {
            kind: TrussType::Fink,
            pitch: 6.0,
            heel_height: 0.0,
            overhang: 12.0,
            spacing: 24.0,
        }
    }
}

/// The Posts tab: what the Post tools start with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PostDefaults {
    pub size: LumberSize,
    pub material: FramingMaterial,
    /// Post with Footing under every new post.
    pub footing: bool,
}

impl Default for PostDefaults {
    fn default() -> Self {
        Self {
            size: LumberSize::FOUR_BY_FOUR,
            material: FramingMaterial::Lumber,
            footing: false,
        }
    }
}

/// The options of Build Framing (the dialog's Floor, Ceiling, Roof, Wall,
/// Posts and Trusses tabs). A plan that never opened the dialog uses
/// [`BuildOptions::default`]: build walls, floors and roofs, no ceiling
/// framing, nothing retained, nothing automatic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildOptions {
    /// Groups a build makes. A group that is off keeps what it has.
    pub build: GroupFlags,
    /// Groups that rebuild by themselves when their inputs change.
    pub auto_rebuild: GroupFlags,
    /// Retain Framing per group: a build leaves the group's members alone,
    /// so edits made to them survive.
    pub retain: GroupFlags,
    /// Retain Wall Framing: walls whose framing a build leaves alone.
    pub retain_walls: Vec<Id>,
    /// Turn the framing layers on after a build.
    pub show_layers: bool,
    pub trusses: TrussDefaults,
    pub posts: PostDefaults,
    /// Fingerprints of the inputs of each floor's groups at its last build,
    /// by floor, in [`Group::ALL`] order (see [`fingerprint`]).
    pub built: Vec<[u64; 4]>,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            build: GroupFlags {
                floor: true,
                ceiling: false,
                wall: true,
                roof: true,
            },
            auto_rebuild: GroupFlags::NONE,
            retain: GroupFlags::NONE,
            retain_walls: Vec::new(),
            show_layers: true,
            trusses: TrussDefaults::default(),
            posts: PostDefaults::default(),
            built: Vec::new(),
        }
    }
}

impl BuildOptions {
    /// Whether a rebuild leaves the existing member `m` as it is: its group
    /// is not built, or is retained, or it frames a retained wall.
    pub fn keeps(&self, m: &Member) -> bool {
        let g = group_of(m);
        !self.build.get(g)
            || self.retain.get(g)
            || m.wall_id.is_some_and(|w| self.retain_walls.contains(&w))
    }

    /// Whether a rebuild leaves the group alone (not built, or retained).
    pub fn group_frozen(&self, g: Group) -> bool {
        !self.build.get(g) || self.retain.get(g)
    }

    /// Whether `wall` is built at all: the wall group is on and not retained,
    /// and this wall's framing is not retained.
    pub fn builds_wall(&self, wall: Id) -> bool {
        !self.group_frozen(Group::Wall) && !self.retain_walls.contains(&wall)
    }

    /// Marks the framing of wall `id` (not) retained.
    pub fn set_wall_retained(&mut self, id: Id, retained: bool) {
        self.retain_walls.retain(|w| *w != id);
        if retained {
            self.retain_walls.push(id);
        }
    }

    /// The fingerprints recorded for floor `fi`, if it was built.
    pub fn built_for(&self, fi: usize) -> Option<[u64; 4]> {
        self.built
            .get(fi)
            .copied()
            .filter(|b| b.iter().any(|x| *x != 0))
    }

    /// Records the fingerprints of floor `fi`.
    pub fn record_built(&mut self, fi: usize, prints: [u64; 4]) {
        if self.built.len() <= fi {
            self.built.resize(fi + 1, [0; 4]);
        }
        self.built[fi] = prints;
    }

    /// Records the fingerprint of one group of floor `fi`.
    pub fn record_group(&mut self, fi: usize, g: Group, print: u64) {
        if self.built.len() <= fi {
            self.built.resize(fi + 1, [0; 4]);
        }
        self.built[fi][g.index()] = print;
    }

    /// The groups of floor `fi` that are due for an automatic rebuild: the
    /// floor was built, the group rebuilds automatically, is not frozen, and
    /// its fingerprint in `now` differs from the one recorded at the build.
    pub fn due(&self, fi: usize, now: [u64; 4]) -> Vec<Group> {
        let Some(then) = self.built_for(fi) else {
            return Vec::new();
        };
        Group::ALL
            .into_iter()
            .filter(|g| {
                self.auto_rebuild.get(*g)
                    && !self.group_frozen(*g)
                    && then[g.index()] != now[g.index()]
            })
            .collect()
    }
}

/// The result of a rebuild: the members of `old` that stay plus the members
/// of `new` that are not shadowed by something kept. `new` should have been
/// made for the whole plan; whatever [`BuildOptions::keeps`] protects is
/// dropped from it, so retained walls and frozen groups are not doubled.
pub fn merge_rebuild(old: Vec<Member>, new: Vec<Member>, opts: &BuildOptions) -> Vec<Member> {
    let mut out: Vec<Member> = old.into_iter().filter(|m| opts.keeps(m)).collect();
    out.extend(new.into_iter().filter(|m| !opts.keeps(m)));
    out
}

/// Like [`merge_rebuild`], but only the groups in `only` are replaced (an
/// automatic rebuild of the groups whose inputs changed); every other group
/// of `old` stays as it is.
pub fn merge_groups(
    old: Vec<Member>,
    new: Vec<Member>,
    opts: &BuildOptions,
    only: &[Group],
) -> Vec<Member> {
    let replaced = |m: &Member| only.contains(&group_of(m)) && !opts.keeps(m);
    let mut out: Vec<Member> = old.into_iter().filter(|m| !replaced(m)).collect();
    out.extend(new.into_iter().filter(replaced));
    out
}

/// A stable 64-bit hash (FNV-1a) of `text`. The inputs of a group are
/// formatted with `{:?}` and hashed, so the value does not change between
/// runs and can be stored in the plan. Never 0.
pub fn fingerprint(text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{frame_wall, FramingDefaults, Lumber, Transform3};
    use plan_core::{Point, Wall, WallKind};

    fn wall(id: Id) -> Wall {
        Wall {
            id,
            start: Point::new(0.0, (id as f64) * 200.0),
            end: Point::new(144.0, (id as f64) * 200.0),
            thickness: 6.5,
            height: 109.125,
            kind: WallKind::Exterior,
            layer: "Walls, Normal".into(),
            ..Default::default()
        }
    }

    fn walls_members(ids: &[Id]) -> Vec<Member> {
        ids.iter()
            .flat_map(|&i| frame_wall(&wall(i), &[], 0.0, &FramingDefaults::default()))
            .collect()
    }

    fn joist() -> Member {
        Member::new(
            MemberKind::Joist,
            Lumber::two_by(9.25),
            100.0,
            Transform3 {
                origin: [0.0; 3],
                axis_x: [1.0, 0.0, 0.0],
                axis_y: [0.0, 1.0, 0.0],
            },
            None,
        )
    }

    #[test]
    fn members_belong_to_the_group_of_their_kind() {
        let w = walls_members(&[1]);
        assert!(w.iter().all(|m| group_of(m) == Group::Wall));
        assert_eq!(group_of(&joist()), Group::Floor);
        let mut c = joist();
        c.kind = MemberKind::CeilingJoist;
        assert_eq!(group_of(&c), Group::Ceiling);
        let mut r = joist();
        r.kind = MemberKind::Rafter;
        assert_eq!(group_of(&r), Group::Roof);
        // Blocking follows its wall.
        let mut b = joist();
        b.kind = MemberKind::Blocking;
        assert_eq!(group_of(&b), Group::Floor);
        b.wall_id = Some(3);
        assert_eq!(group_of(&b), Group::Wall);
        assert_eq!(group_of_manual(ManualKind::RoofTruss), Group::Roof);
        assert_eq!(group_of_manual(ManualKind::Joist), Group::Floor);
    }

    #[test]
    fn the_default_options_build_walls_floors_and_roofs_only() {
        let o = BuildOptions::default();
        assert!(o.build.wall && o.build.floor && o.build.roof);
        assert!(!o.build.ceiling);
        assert!(!o.auto_rebuild.any());
        assert!(!o.retain.any());
        assert!(o.show_layers);
        // An unbuilt group keeps what it has.
        assert!(o.keeps(&{
            let mut c = joist();
            c.kind = MemberKind::CeilingJoist;
            c
        }));
        assert!(!o.keeps(&joist()));
    }

    #[test]
    fn a_retained_wall_and_a_retained_group_survive_a_rebuild() {
        let old = walls_members(&[1, 2]);
        let new = walls_members(&[1, 2]);
        let n_one = walls_members(&[1]).len();
        let mut o = BuildOptions::default();
        o.set_wall_retained(1, true);
        assert!(!o.builds_wall(1) && o.builds_wall(2));
        let merged = merge_rebuild(old.clone(), new.clone(), &o);
        // Wall 1 is the old one, wall 2 the new one: the same total, no doubling.
        assert_eq!(merged.len(), old.len());
        assert_eq!(
            merged.iter().filter(|m| m.wall_id == Some(1)).count(),
            n_one
        );
        // Un-retaining puts the wall back in play.
        o.set_wall_retained(1, false);
        assert!(o.builds_wall(1));
        // A retained group keeps its old members and takes none of the new.
        o.retain.wall = true;
        let mut edited = old.clone();
        edited[0].length += 1.0;
        let merged = merge_rebuild(edited.clone(), new, &o);
        assert_eq!(merged, edited);
        assert!(o.group_frozen(Group::Wall) && !o.group_frozen(Group::Floor));
    }

    #[test]
    fn a_group_that_is_off_is_left_as_it_is_and_the_others_rebuild() {
        let mut old = walls_members(&[1]);
        old.push(joist());
        let mut new_floor = joist();
        new_floor.length = 50.0;
        let new = vec![new_floor.clone()];
        let mut o = BuildOptions::default();
        o.build.floor = false;
        let merged = merge_rebuild(old.clone(), new.clone(), &o);
        // The floor group is off: its old joist stays and the new one is not
        // added. The walls rebuild (this `new` has none), so they go.
        assert_eq!(merged, vec![joist()]);
        o.build.floor = true;
        let merged = merge_rebuild(old.clone(), new, &o);
        assert_eq!(
            merged
                .iter()
                .filter(|m| m.kind == MemberKind::Joist)
                .count(),
            1
        );
        assert!(merged.iter().any(|m| m == &new_floor));
        // The walls (not rebuilt here, `new` has none) went too: nothing keeps them.
        assert!(merged.iter().all(|m| m.kind == MemberKind::Joist));
    }

    #[test]
    fn merging_only_the_changed_groups_leaves_the_others_alone() {
        let mut old = walls_members(&[1]);
        let walls = old.len();
        old.push(joist());
        let mut j = joist();
        j.length = 80.0;
        let merged = merge_groups(
            old.clone(),
            vec![j.clone()],
            &BuildOptions::default(),
            &[Group::Floor],
        );
        assert_eq!(merged.len(), walls + 1);
        assert!(merged.iter().any(|m| m.length == 80.0));
        // A retained wall is not replaced even when its group is due.
        let mut o = BuildOptions::default();
        o.set_wall_retained(1, true);
        let merged = merge_groups(old.clone(), walls_members(&[1]), &o, &[Group::Wall]);
        assert_eq!(merged, old);
    }

    #[test]
    fn auto_rebuild_is_due_only_for_built_floors_and_changed_automatic_groups() {
        let mut o = BuildOptions::default();
        o.auto_rebuild.wall = true;
        o.auto_rebuild.roof = true;
        o.retain.roof = true;
        // Never built: nothing is due.
        assert!(o.due(0, [5, 5, 5, 5]).is_empty());
        o.record_built(0, [1, 2, 3, 4]);
        // Unchanged: nothing is due.
        assert!(o.due(0, [1, 2, 3, 4]).is_empty());
        // The walls and floor changed; only the automatic, unfrozen group is due.
        assert_eq!(o.due(0, [9, 2, 8, 4]), vec![Group::Wall]);
        // The roof changed but is retained.
        assert!(o.due(0, [1, 2, 3, 99]).is_empty());
        // Another floor was never built.
        assert!(o.due(1, [9, 9, 9, 9]).is_empty());
        o.record_group(0, Group::Wall, 8);
        assert!(o.due(0, [1, 2, 8, 4]).is_empty());
    }

    #[test]
    fn the_fingerprint_is_stable_and_never_zero() {
        assert_eq!(fingerprint("a wall"), fingerprint("a wall"));
        assert_ne!(fingerprint("a wall"), fingerprint("a walk"));
        assert_ne!(fingerprint(""), 0);
        // FNV-1a of "a" is a known constant.
        assert_eq!(fingerprint("a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn the_options_round_trip_and_old_files_read_as_defaults() {
        let mut o = BuildOptions::default();
        o.set_wall_retained(7, true);
        o.auto_rebuild.floor = true;
        o.trusses.spacing = 16.0;
        o.record_built(1, [1, 2, 3, 4]);
        let json = serde_json::to_string(&o).unwrap();
        assert_eq!(serde_json::from_str::<BuildOptions>(&json).unwrap(), o);
        assert_eq!(
            serde_json::from_str::<BuildOptions>("{}").unwrap(),
            BuildOptions::default()
        );
        assert_eq!(
            serde_json::from_str::<BuildOptions>("{\"retain_walls\":[3]}")
                .unwrap()
                .retain_walls,
            vec![3]
        );
    }
}
