//! Code minimums in the app: the IRC figures of the plan's Plan Check
//! settings, as the tools, dialogs and defaults use them.
//!
//! * [`code_minimums`] is the single accessor: the [`CodeMinimums`] of the
//!   plan's Plan Check settings (the jurisdiction preset and any limits or
//!   amendments edited there).
//! * [`frame`] runs once a frame: it publishes the minimums (and the
//!   facts a dialog cannot see itself: which windows are in sleeping rooms,
//!   which doors are exterior) for the dialogs' code notices, and keeps the
//!   live check current.
//! * [`apply_code_minimums`] raises a [`PlanDefaults`] to code-legal values;
//!   [`seed_new_plan`] does it for a new plan when the preference is on.
//! * [`fix_finding`] is the Plan Check "Fix": it writes the minimum into the
//!   stair, railing or footing a finding names (one undo step).
//! * The live check (Plan Check > check while drawing) re-counts the rule
//!   groups an edit touched and keeps the count for the status bar and the
//!   Plan Check button.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::hash::{Hash, Hasher};

use plan_check::{CodeMinimums, Finding, Severity, Target};
use plan_core::defaults::{WallLayer, WallTypeDef};
use plan_core::{FloorKind, Id, OpeningKind, PlanDefaults, Project, WallKind};
use plan_stairs::{RailStyle, RailingParams, StairParams, StairShape};

use super::stairs_view as sv;
use super::{rooms_edit, EditorContext};

/// Name of the wall type [`apply_code_minimums`] adds for the wall between a
/// garage and the house.
pub const GARAGE_WALL_TYPE: &str = "Garage Separation-6";

/// The minimums of the plan's Plan Check settings.
pub fn code_minimums(cx: &EditorContext) -> CodeMinimums {
    CodeMinimums::load(&cx.project)
}

// ===================================================================
// What dialogs read
// ===================================================================

#[derive(Default)]
struct Published {
    minimums: CodeMinimums,
    seeds: plan_core::CodeDefaults,
    /// Windows that sit in a sleeping room, by opening id (active floor).
    sleeping_windows: HashSet<Id>,
    /// Exterior doors of the grade floor that can be the required egress door.
    egress_doors: HashSet<Id>,
    /// Normal floors in the plan (storeys).
    stories: usize,
    /// The active floor is the grade floor (the smaller egress area applies).
    grade_floor: bool,
    /// Cache key the opening sets were computed for.
    key: Option<(u64, u64)>,
}

thread_local! {
    static PUBLISHED: RefCell<Published> = RefCell::new(Published::default());
    static SETTINGS_OPEN: std::cell::Cell<Option<std::time::Instant>> = const { std::cell::Cell::new(None) };
}

/// The minimums the open plan is held to (the 2021 IRC until the app has
/// published a plan's).
pub fn active() -> CodeMinimums {
    PUBLISHED.with(|p| p.borrow().minimums.clone())
}

/// The code-legal starting values of the open plan's defaults.
pub fn active_seeds() -> plan_core::CodeDefaults {
    PUBLISHED.with(|p| p.borrow().seeds.clone())
}

/// Storeys of the open plan (at least 1), for the footing width table.
pub fn active_stories() -> usize {
    PUBLISHED.with(|p| p.borrow().stories.max(1))
}

/// The footing width and thickness a Build Foundation dialog is held to for a
/// stem wall `stem` inches tall: the table width for the plan's storeys, and
/// a thickness that is at least the minimum and puts the bottom of the
/// footing at the frost depth below grade.
pub fn footing_limits(stem: f64) -> (f64, f64) {
    let m = active();
    let width = m.footing_width(active_stories());
    let thickness = m.footing_thickness_for_frost(-stem.max(0.0), m.footing_min_thickness);
    (width, thickness)
}

/// Is the active floor the grade floor?
pub fn active_grade_floor() -> bool {
    PUBLISHED.with(|p| p.borrow().grade_floor)
}

/// Is this window in a bedroom (or another sleeping room) of the active floor?
pub fn window_in_sleeping_room(opening: Id) -> bool {
    PUBLISHED.with(|p| p.borrow().sleeping_windows.contains(&opening))
}

/// Is this door an exterior door that could be the required egress door?
pub fn door_is_required_egress(opening: Id) -> bool {
    PUBLISHED.with(|p| p.borrow().egress_doors.contains(&opening))
}

/// Publishes `cx`'s minimums and, when the plan changed, the room facts.
/// [`frame`] calls it; scenario tests call it after setting a plan up.
pub fn publish(cx: &EditorContext) {
    let minimums = code_minimums(cx);
    let key = cx.cache_key();
    PUBLISHED.with(|p| {
        let mut p = p.borrow_mut();
        p.minimums = minimums;
        p.seeds = cx.defaults.code.clone();
        if p.key != Some(key) {
            p.key = Some(key);
            p.stories = stories(&cx.project);
            p.grade_floor = cx.floor().elevation <= 0.0;
            let (sleep, egress) = opening_facts(cx);
            p.sleeping_windows = sleep;
            p.egress_doors = egress;
        }
    });
}

/// Storeys: the normal floors of the plan.
pub fn stories(project: &Project) -> usize {
    project
        .floors
        .iter()
        .filter(|f| f.kind == FloorKind::Normal)
        .count()
        .max(1)
}

/// Which windows of the active floor are in sleeping rooms, and which
/// exterior doors are candidates for the required egress door.
fn room_type_of(cx: &EditorContext, r: &plan_core::Room) -> String {
    rooms_edit::name_entry(cx, r)
        .map(|n| {
            if n.room_type.trim().is_empty() {
                n.name.clone()
            } else {
                n.room_type.clone()
            }
        })
        .unwrap_or_else(|| r.label.clone())
}

/// Does a window `offset` inches along `wall` look into a bedroom or another
/// sleeping room (on either side of the wall)?
pub fn sleeping_room_at(cx: &EditorContext, wall: &plan_core::Wall, offset: f64) -> bool {
    let c = wall.point_at(offset.clamp(0.0, wall.length()));
    let n = wall.normal().scale(3.0);
    [c.add(n), c.sub(n)].iter().any(|p| {
        cx.rooms
            .iter()
            .find(|r| r.contains(*p))
            .is_some_and(|r| CodeMinimums::is_sleeping(&room_type_of(cx, r)))
    })
}

fn opening_facts(cx: &EditorContext) -> (HashSet<Id>, HashSet<Id>) {
    let floor = cx.floor();
    let mut sleeping = HashSet::new();
    let mut egress = HashSet::new();
    let grade = floor.elevation <= 0.0 && floor.kind == FloorKind::Normal;
    for op in &floor.openings {
        let Some(w) = floor.wall(op.wall_id) else {
            continue;
        };
        match op.kind {
            OpeningKind::Window => {
                if sleeping_room_at(cx, w, op.center_offset) {
                    sleeping.insert(op.id);
                }
            }
            OpeningKind::Door => {
                let garage_door = matches!(op.style, plan_core::OpeningStyle::Garage);
                if grade && w.kind == WallKind::Exterior && !garage_door {
                    egress.insert(op.id);
                }
            }
        }
    }
    (sleeping, egress)
}

/// Marks the Plan Check Settings dialog as open for a moment (the status bar
/// shows the code edition while it is).
pub fn note_settings_open() {
    SETTINGS_OPEN.with(|s| s.set(Some(std::time::Instant::now())));
}

/// The Plan Check Settings dialog was closed.
pub fn note_settings_closed() {
    SETTINGS_OPEN.with(|s| s.set(None));
}

fn settings_open() -> bool {
    SETTINGS_OPEN.with(|s| {
        s.get()
            .is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(600))
    })
}

// ===================================================================
// Defaults seeded from code
// ===================================================================

fn raise(v: &mut f64, to: f64) -> bool {
    if *v < to - 1e-9 {
        *v = to;
        true
    } else {
        false
    }
}

fn lower(v: &mut f64, to: f64) -> bool {
    if *v > to + 1e-9 {
        *v = to;
        true
    } else {
        false
    }
}

/// Raises `d` to code-legal values: stairs, railings, the bedroom window, the
/// exterior door, the footing and the garage wall type. Values that are
/// already legal are left alone (a 96" exterior door stays), so it is safe
/// to run on Daniel's template. Returns what changed, one line each; empty
/// when `d` was already legal.
pub fn apply_code_minimums(d: &mut PlanDefaults, m: &CodeMinimums) -> Vec<String> {
    let mut out = Vec::new();
    let mut note = |changed: bool, what: &str| {
        if changed {
            out.push(what.to_string());
        }
    };
    let c = &mut d.code;
    note(lower(&mut c.stair_riser, m.stair_riser_max), "stair riser");
    note(raise(&mut c.stair_tread, m.stair_tread_min), "stair tread");
    note(raise(&mut c.stair_width, m.stair_width_min), "stair width");
    note(
        raise(&mut c.stair_headroom, m.stair_headroom_min),
        "stair headroom",
    );
    note(raise(&mut c.guard_height, m.guard_height), "guard height");
    let railing_wall = raise(&mut d.wall_variants.railing_height, m.guard_height);
    note(railing_wall, "railing wall height");
    let rail = c.handrail_height.clamp(m.handrail_min, m.handrail_max);
    note((c.handrail_height - rail).abs() > 1e-9, "handrail height");
    c.handrail_height = rail;
    note(
        lower(&mut c.baluster_opening, m.guard_sphere),
        "baluster opening",
    );
    note(
        legal_bedroom_window(&mut c.bedroom_window, m),
        "bedroom window",
    );
    let stories = 2;
    note(
        raise(&mut c.footing_width, m.footing_width(stories)),
        "footing width",
    );
    note(
        raise(&mut c.footing_thickness, m.footing_min_thickness),
        "footing thickness",
    );

    // The exterior entry door: a 36" leaf, 80" high.
    let ext = &mut d.exterior_door;
    let w = raise(&mut ext.width, m.egress_door_width());
    let h = raise(&mut ext.height, m.egress_door_height);
    note(w || h, "exterior door");

    // The wall between the garage and the house: 5/8" Type X on both faces,
    // so the garage side is covered whichever way the wall is drawn.
    let t = m.garage_gypsum_habitable_above.max(m.garage_gypsum_min);
    let present = d
        .wall_types
        .iter()
        .any(|w| w.name == d.code.garage_wall_type && !w.name.is_empty());
    if !present {
        let name = GARAGE_WALL_TYPE.to_string();
        if !d.wall_types.iter().any(|w| w.name == name) {
            d.wall_types.push(garage_wall_type(&name, t));
        }
        d.code.garage_wall_type = name;
        out.push("garage wall type".to_string());
    }
    out
}

fn garage_wall_type(name: &str, gypsum: f64) -> WallTypeDef {
    let l = WallLayer::new;
    WallTypeDef {
        name: name.into(),
        kind: WallKind::Interior,
        layers: vec![
            l("Drywall", gypsum, false, "Drywall Type X"),
            l("Framing", 5.5, true, "Fir Framing"),
            l("Drywall", gypsum, false, "Drywall Type X"),
        ],
    }
}

/// Resizes an egress window default until its net clear opening meets the
/// minimums; true when it changed.
fn legal_bedroom_window(w: &mut plan_core::defaults::WindowDefaults, m: &CodeMinimums) -> bool {
    let before = (w.width, w.height, w.sill_height, w.egress);
    let net = if w.window_type.to_lowercase().contains("slid") {
        0.5
    } else {
        1.0
    };
    raise(&mut w.width, m.egress_min_width / net);
    raise(&mut w.height, m.egress_min_height);
    let area = (m.egress_min_area * 144.0 / (w.width * net)).ceil();
    raise(&mut w.height, area);
    lower(&mut w.sill_height, m.egress_max_sill);
    w.egress = true;
    before != (w.width, w.height, w.sill_height, w.egress)
}

/// What a new plan starts from: `d` raised to the 2021 IRC when the
/// "Seed defaults from code minimums" preference is on (it is by default).
/// Returns what changed.
pub fn seed_new_plan(d: &mut PlanDefaults) -> Vec<String> {
    if !crate::dialogs::preferences::pages::current()
        .architectural
        .seed_code_defaults
    {
        return Vec::new();
    }
    apply_code_minimums(d, &CodeMinimums::default())
}

/// Raises `cx.defaults` to the minimums of the plan's settings (no undo step
/// of its own: the caller has begun one) and returns the status line.
pub fn raise_defaults(cx: &mut EditorContext) -> String {
    let m = code_minimums(cx);
    let changed = apply_code_minimums(&mut cx.defaults, &m);
    cx.mark_dirty();
    if changed.is_empty() {
        format!("Defaults already meet {}", m.label())
    } else {
        format!(
            "Raised the defaults to {}: {}",
            m.label(),
            changed.join(", ")
        )
    }
}

/// `d` as a new plan starts from it: seeded from the code minimums when the
/// preference is on.
pub fn seeded(mut d: PlanDefaults) -> PlanDefaults {
    seed_new_plan(&mut d);
    d
}

/// Default Settings > Plan Check > Apply code minimums to defaults: raises
/// the defaults to the minimums of the plan's settings and returns what
/// moved for the status bar. The plan's history gets one step ("Apply Code
/// Minimums to Defaults") so the command shows in Undo; the defaults are the
/// application's, so Undo restores the plan, not them (DECISIONS).
pub fn apply_to_defaults(cx: &mut EditorContext) -> String {
    cx.begin_change("Apply Code Minimums to Defaults");
    raise_defaults(cx)
}

// ===================================================================
// Tools start from the minimums
// ===================================================================

/// A new stair's parameters made code-legal: the riser and headroom of the
/// plan's minimums and the code defaults, a guard or handrail at the legal
/// height. `build` in `stairs_view` calls it before it lays the stair out.
pub fn legalize_stair_params(p: &mut StairParams) {
    if p.spiral
        || matches!(
            p.shape,
            StairShape::Landing { .. } | StairShape::Ramp { .. }
        )
    {
        return;
    }
    let (m, seeds) = PUBLISHED.with(|x| {
        let x = x.borrow();
        (x.minimums.clone(), x.seeds.clone())
    });
    p.riser_height_target = seeds.stair_riser.min(m.stair_riser_max);
    raise(&mut p.tread_depth, m.stair_tread_min.max(seeds.stair_tread));
    raise(&mut p.width, m.stair_width_min.max(seeds.stair_width));
    raise(
        &mut p.headroom_min,
        m.stair_headroom_min.max(seeds.stair_headroom),
    );
    let both = [&mut p.railing]
        .into_iter()
        .chain(p.left_railing.as_mut())
        .chain(p.right_railing.as_mut());
    for r in both {
        legalize_rail(r, &m, seeds.guard_height, seeds.baluster_opening);
    }
}

/// A stair railing at a legal height with legal baluster openings.
fn legalize_rail(r: &mut RailingParams, m: &CodeMinimums, guard: f64, opening: f64) {
    raise(
        &mut r.height,
        m.stair_guard_height.max(guard.min(m.handrail_max)),
    );
    lower(&mut r.height, m.handrail_max.max(m.guard_height));
    if let RailStyle::Balusters { spacing, .. } = &mut r.style {
        lower(spacing, m.guard_sphere.min(opening));
    }
}

/// Auto Place Outlets spacing from the minimums: no point along a wall is
/// farther from a receptacle than the code allows, counter receptacles are
/// no farther apart than the counter spacing, and wall spaces narrower than
/// the minimum get none.
pub fn outlet_options(m: &CodeMinimums, o: &mut plan_electrical::AutoOutletOptions) {
    lower(&mut o.max_spacing, m.receptacle_max_spacing);
    lower(&mut o.kitchen_counter_spacing, m.counter_receptacle_spacing);
    raise(&mut o.min_wall_segment, m.receptacle_wall_space_min);
}

// ===================================================================
// Plan Check "Fix"
// ===================================================================

/// Whether [`fix_finding`] knows how to correct `f`.
pub fn can_fix(f: &Finding) -> bool {
    matches!(
        (f.rule, f.object),
        (
            "IRC R311.7.1 stair width"
                | "IRC R311.7.2 headroom"
                | "IRC R311.7.5.1 riser height"
                | "IRC R311.7.5.2 tread depth"
                | "IRC R311.7.8.1 handrail height"
                | "IRC R312.1.2 guard height"
                | "IRC R312.1.3 opening limitation",
            Some(Target::Stair(_)),
        ) | (
            "IRC R403.1.1 footing size" | "IRC R403.1.4 footing depth",
            _
        )
    )
}

/// Writes the minimum a finding is about into the stair or footing it names.
/// One undo step ("Fix Stair to Code" / "Fix Footing to Code"). Returns the
/// status text, or `None` when the finding is not one it fixes or nothing
/// had to change.
pub fn fix_finding(cx: &mut EditorContext, f: &Finding) -> Option<String> {
    let m = code_minimums(cx);
    match (f.rule, f.object) {
        (_, Some(Target::Stair(id))) if can_fix(f) => {
            let before = sv::find(cx.floor(), id)?;
            let mut after = before.clone();
            if !fix_stair_params(&mut after.stair.params, &m) {
                return None;
            }
            let fl = cx.floor;
            cx.begin_change("Fix Stair to Code");
            sv::update(&mut cx.project, fl, id, |o| *o = after);
            cx.mark_dirty();
            Some("Set the stair to the code minimums".to_string())
        }
        ("IRC R403.1.1 footing size" | "IRC R403.1.4 footing depth", _) => fix_footing(cx, &m),
        _ => None,
    }
}

/// Raises the stair to the minimums for width, headroom, tread and riser, and
/// puts every rail at a legal height with legal baluster openings. True when
/// anything changed.
pub fn fix_stair_params(p: &mut StairParams, m: &CodeMinimums) -> bool {
    let before = p.clone();
    if !matches!(
        p.shape,
        StairShape::Landing { .. } | StairShape::Ramp { .. }
    ) && !p.spiral
    {
        lower(&mut p.riser_height_target, m.stair_riser_max);
        raise(&mut p.tread_depth, m.stair_tread_min);
        raise(&mut p.headroom_min, m.stair_headroom_min);
    }
    raise(&mut p.width, m.stair_width_min);
    let guard = m.guard_height;
    let rails = [&mut p.railing]
        .into_iter()
        .chain(p.left_railing.as_mut())
        .chain(p.right_railing.as_mut());
    for r in rails {
        raise(&mut r.height, m.stair_guard_height);
        lower(&mut r.height, m.handrail_max.max(guard));
        if let RailStyle::Balusters { spacing, .. } = &mut r.style {
            lower(spacing, m.guard_sphere);
        }
    }
    *p != before
}

/// Footing width and thickness (and the thickness for the frost depth) of the
/// plan's foundation floor.
fn fix_footing(cx: &mut EditorContext, m: &CodeMinimums) -> Option<String> {
    let stories = stories(&cx.project);
    let idx = cx
        .project
        .floors
        .iter()
        .position(|f| f.kind == FloorKind::Foundation && f.settings.foundation.is_some())?;
    let (elevation, mut b) = {
        let f = &cx.project.floors[idx];
        (f.elevation, f.settings.foundation?)
    };
    let before = b;
    let (w, t) = m.footing_for(stories, b.footing_width, b.footing_depth);
    b.footing_width = w;
    b.footing_depth = m.footing_thickness_for_frost(elevation, t);
    if b == before {
        return None;
    }
    cx.begin_change("Fix Footing to Code");
    cx.project.floors[idx].settings.foundation = Some(b);
    cx.mark_dirty();
    Some(format!(
        "Footing set to {} wide and {} thick",
        cx.fmt_dim(b.footing_width),
        cx.fmt_dim(b.footing_depth)
    ))
}

// ===================================================================
// Notices for the dialogs' fields
// ===================================================================

/// One thing a stair is short of, for tests and the Fix preview.
#[derive(Debug, Clone, PartialEq)]
pub struct Issue {
    /// The code section.
    pub rule: &'static str,
    /// The field (`riser`, `tread`, `width`, `headroom`).
    pub field: &'static str,
    /// The limit it is held to.
    pub limit: f64,
}

/// What a stair's fields are past, in dialog order.
pub fn stair_issues(p: &StairParams, m: &CodeMinimums) -> Vec<Issue> {
    use crate::dialogs::code_notice::{check_limit, LimitKind};
    let mut out = Vec::new();
    let mut add = |bad: bool, rule, field, limit| {
        if bad {
            out.push(Issue { rule, field, limit });
        }
    };
    if matches!(p.shape, StairShape::Landing { .. }) {
        add(
            check_limit(p.width, m.stair_width_min, LimitKind::Min),
            "R311.7.1 width",
            "width",
            m.stair_width_min,
        );
        return out;
    }
    add(
        check_limit(p.width, m.stair_width_min, LimitKind::Min),
        "R311.7.1 width",
        "width",
        m.stair_width_min,
    );
    if !matches!(p.shape, StairShape::Ramp { .. }) {
        add(
            check_limit(p.tread_depth, m.stair_tread_min, LimitKind::Min),
            "R311.7.5.2 tread",
            "tread",
            m.stair_tread_min,
        );
        add(
            check_limit(p.riser_height_target, m.stair_riser_max, LimitKind::Max),
            "R311.7.5.1 riser",
            "riser",
            m.stair_riser_max,
        );
        add(
            check_limit(p.headroom_min, m.stair_headroom_min, LimitKind::Min),
            "R311.7.2 headroom",
            "headroom",
            m.stair_headroom_min,
        );
    }
    out
}

// ===================================================================
// Live check
// ===================================================================

/// The groups the live check keeps current.
pub const LIVE_GROUPS: [&str; 4] = [
    "Stairs and guards",
    "Doors and windows",
    "Rooms",
    "Foundation",
];

/// What the last live check found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LiveSummary {
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
    /// Findings per live group, in [`LIVE_GROUPS`] order.
    pub groups: [usize; 4],
    /// Groups re-counted by the last run.
    pub rerun: [bool; 4],
    /// Microseconds the last run took (the check and the bookkeeping).
    pub micros: u128,
    /// How many runs there were.
    pub runs: u32,
}

impl LiveSummary {
    /// Findings of the live groups.
    pub fn total(&self) -> usize {
        self.errors + self.warnings + self.info
    }
}

#[derive(Default)]
struct LiveState {
    key: Option<(u64, u64)>,
    floor: usize,
    settings: u64,
    prints: [u64; 4],
    per_group: [(usize, usize, usize); 4],
    summary: LiveSummary,
    on: bool,
}

thread_local! {
    static LIVE: RefCell<LiveState> = RefCell::new(LiveState::default());
}

fn h64<T: Hash>(t: &T) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

fn f64s(h: &mut impl Hasher, vs: &[f64]) {
    for v in vs {
        h.write_u64(v.to_bits());
    }
}

/// Fingerprints of what each live group reads, in [`LIVE_GROUPS`] order:
/// stairs and the walls and doors beside them, openings and walls, rooms
/// (walls and names), the foundation floor.
fn fingerprints(cx: &EditorContext) -> [u64; 4] {
    let f = cx.floor();
    let mut walls = std::collections::hash_map::DefaultHasher::new();
    for w in &f.walls {
        walls.write_u64(w.id);
        f64s(
            &mut walls,
            &[
                w.start.x,
                w.start.y,
                w.end.x,
                w.end.y,
                w.thickness,
                w.height,
            ],
        );
        walls.write_u8(w.kind as u8);
        w.wall_type.hash(&mut walls);
    }
    let walls = walls.finish();
    let mut ops = std::collections::hash_map::DefaultHasher::new();
    for o in &f.openings {
        ops.write_u64(o.id);
        ops.write_u64(o.wall_id);
        f64s(
            &mut ops,
            &[o.center_offset, o.width, o.height, o.sill_height],
        );
        ops.write_u8(o.kind as u8);
        ops.write_u8(o.style as u8);
        ops.write_u8(o.egress as u8);
        ops.write_u8(o.swing_flipped as u8);
    }
    let ops = ops.finish();
    // Rooms read the doors (means of egress, access through a bath), not the
    // windows.
    let mut doors = std::collections::hash_map::DefaultHasher::new();
    for o in f.openings.iter().filter(|o| o.kind == OpeningKind::Door) {
        doors.write_u64(o.id);
        doors.write_u64(o.wall_id);
        f64s(&mut doors, &[o.center_offset, o.width]);
    }
    let doors = doors.finish();
    let mut names = std::collections::hash_map::DefaultHasher::new();
    for n in &f.room_names {
        n.name.hash(&mut names);
        n.room_type.hash(&mut names);
        f64s(&mut names, &[n.anchor.x, n.anchor.y]);
    }
    f64s(&mut names, &[f.ceiling_height, f.elevation]);
    let names = names.finish();
    let stairs = h64(&f.stairs.iter().map(|v| v.to_string()).collect::<Vec<_>>());
    let found = h64(&(
        format!("{:?}", f.settings.foundation),
        f.foundation.as_ref().map(|v| v.to_string()),
        project_floor_shape(&cx.project),
    ));
    [
        h64(&(stairs, walls, ops)),
        h64(&(ops, walls, names)),
        h64(&(walls, names, doors)),
        found,
    ]
}

/// The floors' kinds and elevations (the foundation rules read them all).
fn project_floor_shape(p: &Project) -> Vec<(u8, u64)> {
    p.floors
        .iter()
        .map(|f| (f.kind as u8, f.elevation.to_bits()))
        .collect()
}

fn group_index(rule: &str) -> Option<usize> {
    let g = plan_check::rule_catalog()
        .iter()
        .find(|r| r.id == rule)
        .map(|r| r.group)?;
    LIVE_GROUPS.iter().position(|x| *x == g)
}

/// The live check's last result.
pub fn live_summary() -> LiveSummary {
    LIVE.with(|l| l.borrow().summary.clone())
}

/// Switches the live check on or off (the "check while drawing" setting);
/// off forgets the counts.
pub fn set_live(on: bool) {
    LIVE.with(|l| {
        let mut l = l.borrow_mut();
        if l.on != on {
            *l = LiveState {
                on,
                ..LiveState::default()
            };
        }
    });
}

/// Brings the live check up to date for the plan as it is now. Cheap when
/// nothing changed (one comparison); otherwise it fingerprints the inputs
/// of the four live groups and re-counts only the groups whose inputs
/// changed (all of them after a floor change or a settings change). The
/// check runs with the plan's Plan Check settings, so switched-off and
/// ignored findings stay out of the count.
pub fn update_live(cx: &mut EditorContext) -> LiveSummary {
    let on = crate::dialogs::preferences::pages::current()
        .architectural
        .check_while_drawing;
    set_live(on);
    if !on {
        return LiveSummary::default();
    }
    let key = cx.cache_key();
    if LIVE.with(|l| l.borrow().key == Some(key)) {
        return live_summary();
    }
    let t0 = std::time::Instant::now();
    let prints = fingerprints(cx);
    // The Plan Check settings, the ignore list and the amendments, as stored.
    let settings = h64(&cx
        .project
        .info
        .custom
        .iter()
        .filter(|(k, _)| k.starts_with("plancheck."))
        .collect::<Vec<_>>());
    let (floor, old_prints, old_settings, first) = LIVE.with(|l| {
        let l = l.borrow();
        (l.floor, l.prints, l.settings, l.key.is_none())
    });
    let all = first || floor != cx.floor || old_settings != settings;
    let dirty: [bool; 4] = std::array::from_fn(|i| all || prints[i] != old_prints[i]);
    let mut per_group = LIVE.with(|l| l.borrow().per_group);
    if dirty.iter().any(|d| *d) {
        let run = crate::dialogs::plan_check::run_check_full(
            cx,
            crate::dialogs::plan_check::CheckKind::Plan,
        );
        let mut fresh = [(0usize, 0usize, 0usize); 4];
        for f in &run.findings {
            if let Some(g) = group_index(f.rule) {
                match f.severity {
                    Severity::Error => fresh[g].0 += 1,
                    Severity::Warning => fresh[g].1 += 1,
                    Severity::Info => fresh[g].2 += 1,
                }
            }
        }
        for i in 0..4 {
            if dirty[i] {
                per_group[i] = fresh[i];
            }
        }
    }
    let summary = LiveSummary {
        errors: per_group.iter().map(|g| g.0).sum(),
        warnings: per_group.iter().map(|g| g.1).sum(),
        info: per_group.iter().map(|g| g.2).sum(),
        groups: std::array::from_fn(|i| per_group[i].0 + per_group[i].1 + per_group[i].2),
        rerun: dirty,
        micros: t0.elapsed().as_micros(),
        runs: LIVE.with(|l| l.borrow().summary.runs) + 1,
    };
    LIVE.with(|l| {
        let mut l = l.borrow_mut();
        l.key = Some(cx.cache_key());
        l.floor = cx.floor;
        l.settings = settings;
        l.prints = prints;
        l.per_group = per_group;
        l.summary = summary.clone();
    });
    summary
}

/// "2 errors, 1 warning" for the live count; `None` when there is nothing
/// to report or the live check is off.
pub fn live_text() -> Option<String> {
    let s = live_summary();
    if !LIVE.with(|l| l.borrow().on) {
        return None;
    }
    let e = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let mut parts = Vec::new();
    if s.errors > 0 {
        parts.push(e(s.errors, "error", "errors"));
    }
    if s.warnings > 0 {
        parts.push(e(s.warnings, "warning", "warnings"));
    }
    (!parts.is_empty()).then(|| parts.join(", "))
}

/// The number on the Plan Check toolbar button: errors and warnings the
/// live check counts (0 draws no badge).
pub fn badge_count() -> usize {
    let s = live_summary();
    s.errors + s.warnings
}

/// The status-bar text: the code edition while the Plan Check Settings are
/// open, and the live count of findings.
pub fn status_text(cx: &EditorContext) -> Option<String> {
    let mut parts = Vec::new();
    if settings_open() {
        parts.push(format!("Code: {}", code_minimums(cx).label()));
    }
    if let Some(t) = live_text() {
        parts.push(format!("Live check: {t}"));
    }
    (!parts.is_empty()).then(|| parts.join("  "))
}

/// Once a frame, after `refresh`: publishes the minimums for the dialogs and
/// updates the live check.
pub fn frame(cx: &mut EditorContext) {
    publish(cx);
    update_live(cx);
}

/// Overrides stored with the plan, for the Plan Check Settings dialog.
pub fn overrides(cx: &EditorContext) -> BTreeMap<String, f64> {
    CodeMinimums::load_overrides(&cx.project)
}
