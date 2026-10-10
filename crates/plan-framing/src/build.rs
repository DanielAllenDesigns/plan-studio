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

/// Set in the `wall_id` of the members framing a tray ceiling (the low bits are
/// the tray's id): they belong to the Ceiling group, not the Wall group, though
/// they are plates and studs.
pub const TRAY_MEMBER_FLAG: Id = 1 << 62;

/// The group an automatic member belongs to. Blocking is wall blocking when it
/// belongs to a wall and floor blocking otherwise.
pub fn group_of(m: &Member) -> Group {
    if m.wall_id.is_some_and(|w| w & TRAY_MEMBER_FLAG != 0) {
        return Group::Ceiling;
    }
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

/// How joists that meet over a bearing wall, beam or line are joined
/// (Bear Joists on Beams and on Bearing Walls, manual p. 891).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Splice {
    /// The joists lap side by side over the support: 8" of lap, centred.
    #[default]
    Lap,
    /// The joists butt end to end, centred over the support.
    Butt,
}

impl Splice {
    pub const ALL: [Splice; 2] = [Splice::Lap, Splice::Butt];

    pub fn name(self) -> &'static str {
        match self {
            Splice::Lap => "Lap",
            Splice::Butt => "Butt",
        }
    }
}

/// Length of the lap of two joists over a support, inches (centred).
pub const LAP_LENGTH: f64 = 8.0;

/// The Blocking style of a platform or roof (manual pp. 891, 898).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BlockingStyle {
    /// Blocking pieces align with each other.
    #[default]
    InLine,
    /// Blocking alternates on either side of the line.
    Stagger,
    /// Cross bridging: in line in plan, crossed pieces in 3D and the list.
    Cross,
}

impl BlockingStyle {
    pub const ALL: [BlockingStyle; 3] = [
        BlockingStyle::InLine,
        BlockingStyle::Stagger,
        BlockingStyle::Cross,
    ];

    pub fn name(self) -> &'static str {
        match self {
            BlockingStyle::InLine => "In Line",
            BlockingStyle::Stagger => "Stagger",
            BlockingStyle::Cross => "Cross/Bridging",
        }
    }
}

/// How the ends of doubled boards (rim joists, top plates) meet at a corner
/// or intersection (manual pp. 891, 894).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Connection {
    /// Ends alternate in a herringbone pattern.
    #[default]
    Stagger,
    /// Every board ends flush with the one beside it.
    Flush,
}

impl Connection {
    pub const ALL: [Connection; 2] = [Connection::Stagger, Connection::Flush];

    pub fn name(self) -> &'static str {
        match self {
            Connection::Stagger => "Stagger",
            Connection::Flush => "Flush",
        }
    }
}

/// How the studs at a wall corner or intersection are made (manual p. 893).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WallConnection {
    /// Three studs.
    #[default]
    Standard,
    /// Two studs.
    Reduced,
    /// Two studs and horizontal ladder blocking between them.
    Laddered,
    /// Three studs in a U (corners only).
    UShaped,
}

impl WallConnection {
    pub const ALL: [WallConnection; 4] = [
        WallConnection::Standard,
        WallConnection::Reduced,
        WallConnection::Laddered,
        WallConnection::UShaped,
    ];

    pub fn name(self) -> &'static str {
        match self {
            WallConnection::Standard => "Standard",
            WallConnection::Reduced => "Reduced Stud",
            WallConnection::Laddered => "Laddered",
            WallConnection::UShaped => "U Shaped",
        }
    }
}

fn yes() -> bool {
    true
}

/// The framing detail options of the Automatic Framing Defaults dialog that
/// `FramingDefaults` does not hold: how joists meet over supports, rim joists,
/// the connection styles of walls, blocking, mitred wall ends, headers and
/// the plan display of framing. They belong in the Floor Level, Wall and
/// Openings panels; they are kept here, with the build options, until the
/// framing defaults owner folds them in (docs/integration-queue.md).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DetailOptions {
    /// Bear Joists on Beams and Bearing Walls, floor framing.
    pub splice: Splice,
    /// The same for ceiling framing.
    pub ceiling_splice: Splice,
    /// Blocking style of floor framing (mid-span blocking and Joist Blocking).
    pub blocking_style: BlockingStyle,
    /// The same for ceiling framing.
    pub ceiling_blocking_style: BlockingStyle,
    /// Rim Joist Width: the horizontal thickness of one board.
    pub rim_width: f64,
    /// How doubled rim joists connect at the corners of a platform.
    pub rim_connection: Connection,
    /// Maximum Rim Joist Length; `0.0` runs each rim the full platform edge.
    pub max_rim_length: f64,
    /// Wall corners.
    pub corner_style: WallConnection,
    /// Wall intersections.
    pub tee_style: WallConnection,
    /// How the top plates connect at intersections.
    pub top_plate_connection: Connection,
    /// Stagger Blocking in walls: alternate on either side of the centre line.
    pub stagger_blocking: bool,
    /// Mitre Plate Ends of angled walls.
    pub mitre_plate_ends: bool,
    /// Rotate End Studs to the angle of the mitre.
    pub rotate_end_studs: bool,
    /// Horizontal Frame Through at an angled corner (butt the vertical walls
    /// against the horizontal ones).
    pub frame_through_horizontal: bool,
    /// Header Maximum Depth: a rough opening whose top is closer than this to
    /// the top plate gets a solid header filling the gap and no cripples.
    /// `0.0` switches the rule off.
    pub header_max_depth: f64,
    /// List Cut Header Lengths in Mixed Reporting.
    pub list_cut_header_lengths: bool,
    /// Bearing walls get a header at least two plies thick and double top
    /// plates.
    pub bearing_wall_headers: bool,
    /// Studs, kings, trimmers and posts draw as cross boxes in plan.
    pub show_cross: bool,
    /// Build Wall Framing Details from Exterior: a Wall Detail shows the wall
    /// as seen from outside.
    pub details_from_exterior: bool,
    /// The default fill of wall framing members in Wall Details.
    pub wall_detail_fill: Option<plan_core::fill_styles::FillStyle>,
}

impl Default for DetailOptions {
    fn default() -> Self {
        Self {
            splice: Splice::Lap,
            ceiling_splice: Splice::Lap,
            blocking_style: BlockingStyle::InLine,
            ceiling_blocking_style: BlockingStyle::InLine,
            rim_width: 1.5,
            rim_connection: Connection::Stagger,
            max_rim_length: 0.0,
            corner_style: WallConnection::Standard,
            tee_style: WallConnection::Standard,
            top_plate_connection: Connection::Stagger,
            stagger_blocking: false,
            mitre_plate_ends: true,
            rotate_end_studs: false,
            frame_through_horizontal: true,
            header_max_depth: 0.0,
            list_cut_header_lengths: false,
            bearing_wall_headers: true,
            show_cross: yes(),
            details_from_exterior: yes(),
            wall_detail_fill: None,
        }
    }
}

/// The floor choice of Build Framing Once (manual p. 915): which floors a
/// Floor or Ceiling build covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FloorPick {
    /// The floor the menu item names: the active floor for Build Framing,
    /// every floor for Build All Framing.
    #[default]
    Same,
    /// All Floors.
    All,
    /// One floor, by index.
    Floor(usize),
}

impl FloorPick {
    /// Whether floor `fi` is covered, given the active floor and whether the
    /// menu item builds every floor.
    pub fn covers(self, fi: usize, active: usize, all_floors: bool) -> bool {
        match self {
            FloorPick::Same => all_floors || fi == active,
            FloorPick::All => true,
            FloorPick::Floor(f) => fi == f,
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
    /// Retain Framing of roof planes (by id): a build leaves the roof framing
    /// that stands over them alone.
    pub retain_planes: Vec<Id>,
    /// Turn the framing layers on after a build.
    pub show_layers: bool,
    pub trusses: TrussDefaults,
    pub posts: PostDefaults,
    /// Build Framing Once: the floors a Floor build covers.
    pub floor_pick: FloorPick,
    /// Build Framing Once: the floors a Ceiling build covers.
    pub ceiling_pick: FloorPick,
    /// Use Framing Reference per floor (joist and deck joist layout), by floor
    /// index; a floor past the end of the list uses it.
    pub floor_reference: Vec<bool>,
    /// Use Framing Reference for the layout of rafters, plan wide.
    pub roof_reference: bool,
    /// The framing detail options.
    pub detail: DetailOptions,
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
            retain_planes: Vec::new(),
            show_layers: true,
            trusses: TrussDefaults::default(),
            posts: PostDefaults::default(),
            floor_pick: FloorPick::Same,
            ceiling_pick: FloorPick::Same,
            floor_reference: Vec::new(),
            roof_reference: true,
            detail: DetailOptions::default(),
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

    /// Whether the joists of floor `fi` start from a Framing Reference Marker.
    pub fn uses_reference(&self, fi: usize) -> bool {
        self.floor_reference.get(fi).copied().unwrap_or(true)
    }

    /// Sets Use Framing Reference for floor `fi`.
    pub fn set_reference(&mut self, fi: usize, on: bool) {
        if self.floor_reference.len() <= fi {
            self.floor_reference.resize(fi + 1, true);
        }
        self.floor_reference[fi] = on;
    }

    /// The groups a Build Framing Once of floor `fi` makes: the saved Build
    /// flags, with Floor and Ceiling limited to the floors picked. A group
    /// that is off keeps its members.
    pub fn once_for(&self, fi: usize, active: usize, all_floors: bool) -> BuildOptions {
        let mut o = self.clone();
        o.build.floor &= self.floor_pick.covers(fi, active, all_floors);
        o.build.ceiling &= self.ceiling_pick.covers(fi, active, all_floors);
        // Walls and roofs follow the menu item.
        let this_floor = all_floors || fi == active;
        o.build.wall &= this_floor;
        o.build.roof &= this_floor;
        o
    }

    /// Marks the framing of roof plane `id` (not) retained.
    pub fn set_plane_retained(&mut self, id: Id, retained: bool) {
        self.retain_planes.retain(|w| *w != id);
        if retained {
            self.retain_planes.push(id);
        }
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

    // ----- Round 16: Build Framing Once, references, detail options -----

    #[test]
    fn a_floor_pick_covers_the_named_floors() {
        // "Same" follows the menu item: the active floor, or all of them.
        assert!(FloorPick::Same.covers(1, 1, false));
        assert!(!FloorPick::Same.covers(0, 1, false));
        assert!(FloorPick::Same.covers(0, 1, true));
        assert!(FloorPick::All.covers(0, 1, false));
        assert!(FloorPick::Floor(2).covers(2, 0, false));
        assert!(!FloorPick::Floor(2).covers(1, 0, true));
    }

    #[test]
    fn build_once_limits_floor_and_ceiling_to_the_picked_floors() {
        let mut o = BuildOptions::default();
        o.build.ceiling = true;
        o.floor_pick = FloorPick::Floor(1);
        o.ceiling_pick = FloorPick::All;
        // Active floor 0, Build Framing: floor 1's platform and every ceiling, but walls
        // and roof only of floor 0.
        let f0 = o.once_for(0, 0, false);
        assert!(!f0.build.floor && f0.build.ceiling && f0.build.wall && f0.build.roof);
        let f1 = o.once_for(1, 0, false);
        assert!(f1.build.floor && f1.build.ceiling && !f1.build.wall && !f1.build.roof);
        // A group that is off stays off for every floor.
        o.build.floor = false;
        assert!(!o.once_for(1, 0, false).build.floor);
        // The walls of floor 1 are untouched by a build that is not "all floors":
        // its old members are kept.
        let wall = Member::new(
            MemberKind::Stud,
            crate::TWO_BY_FOUR,
            90.0,
            Transform3 {
                origin: [0.0; 3],
                axis_x: [0.0, 1.0, 0.0],
                axis_y: [0.0, 0.0, 1.0],
            },
            Some(4),
        );
        assert!(f1.keeps(&wall));
        assert!(!f0.keeps(&wall));
    }

    #[test]
    fn framing_reference_is_per_floor_and_on_unless_switched_off() {
        let mut o = BuildOptions::default();
        assert!(o.uses_reference(0) && o.uses_reference(5));
        o.set_reference(2, false);
        assert!(o.uses_reference(0) && o.uses_reference(1));
        assert!(!o.uses_reference(2));
        assert!(o.roof_reference);
        let json = serde_json::to_string(&o).unwrap();
        assert_eq!(serde_json::from_str::<BuildOptions>(&json).unwrap(), o);
    }

    #[test]
    fn detail_options_default_to_chiefs_choices_and_round_trip() {
        let d = DetailOptions::default();
        assert_eq!(d.splice, Splice::Lap);
        assert_eq!(d.corner_style, WallConnection::Standard);
        assert_eq!(d.blocking_style, BlockingStyle::InLine);
        assert_eq!(d.rim_connection, Connection::Stagger);
        assert!(d.mitre_plate_ends && d.show_cross && !d.stagger_blocking);
        assert_eq!(Splice::ALL.len() + Connection::ALL.len(), 4);
        assert_eq!(BlockingStyle::Cross.name(), "Cross/Bridging");
        assert_eq!(WallConnection::UShaped.name(), "U Shaped");
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(serde_json::from_str::<DetailOptions>(&json).unwrap(), d);
        assert_eq!(
            serde_json::from_str::<DetailOptions>("{}").unwrap(),
            DetailOptions::default()
        );
    }
}
