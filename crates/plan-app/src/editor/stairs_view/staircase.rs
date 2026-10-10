//! The staircase as a whole (Round 16, brief 21): the sections and landings
//! joined to a stair, their numbers and specification table, Make Best Fit,
//! the tread depth mode, Lock Top / Lock Bottom, merging two flights, and
//! the landing rules (adjacent landings, automatic thickness).

use super::*;
use plan_stairs::{LockEnd, MergeError, SpecRow, TreadMode};

/// Everything joined to `id` through landings, bottom to top: the sections
/// (stairs and ramps) and the landings between them. Empty if `id` does not
/// exist. Two landings are joined when their edges lie within 1 inch.
pub fn staircase(floor: &Floor, id: Id) -> Vec<StairObj> {
    let all = load(floor);
    let Some(start) = all.iter().position(|o| o.id() == id) else {
        return Vec::new();
    };
    let n = all.len();
    let linked = |a: &StairObj, b: &StairObj| -> bool {
        match (a.is_landing(), b.is_landing()) {
            (false, true) => joint(a, b).is_some(),
            (true, false) => joint(b, a).is_some(),
            (true, true) => plan_stairs::are_adjacent(&a.footprint(), &b.footprint()),
            (false, false) => false,
        }
    };
    let mut seen = vec![false; n];
    seen[start] = true;
    let mut queue = vec![start];
    while let Some(i) = queue.pop() {
        for j in 0..n {
            if !seen[j] && linked(&all[i], &all[j]) {
                seen[j] = true;
                queue.push(j);
            }
        }
    }
    let mut out: Vec<StairObj> = all
        .into_iter()
        .zip(seen)
        .filter_map(|(o, s)| s.then_some(o))
        .collect();
    // Bottom to top; a landing sorts just below the stair that starts on it,
    // so after the one that arrives.
    out.sort_by(|a, b| {
        let key = |o: &StairObj| {
            if o.is_landing() {
                o.landing_height() - 0.001
            } else {
                o.bottom_height()
            }
        };
        key(a).total_cmp(&key(b))
    });
    out
}

/// The sections (not the landings) of the staircase of `id`, bottom to top.
pub fn sections(floor: &Floor, id: Id) -> Vec<StairObj> {
    staircase(floor, id)
        .into_iter()
        .filter(|o| !o.is_landing())
        .collect()
}

/// The specification table of the staircase of `id` (at most ten rows).
pub fn spec_table(floor: &Floor, id: Id) -> Vec<SpecRow> {
    let secs = sections(floor, id);
    let stairs: Vec<&Stair> = secs.iter().map(|o| &o.stair).collect();
    plan_stairs::spec_rows(&stairs)
}

/// The numbers shown on the selected staircase: `(position, "1")` for a
/// section, `(position, "1-2")` for a subsection, spread along each stair.
pub fn section_labels(floor: &Floor, id: Id) -> Vec<(Point, String)> {
    let mut out = Vec::new();
    let mut done = 0;
    for o in sections(floor, id) {
        let rows = plan_stairs::spec_rows(&[&o.stair]);
        let (from, to) = (o.bottom_center(), top_point(&o.stair).0);
        let mut count = 0;
        for (i, r) in rows.iter().enumerate() {
            let (sec, sub) = match r.number.split_once('-') {
                Some((s, k)) => (s.parse::<usize>().unwrap_or(1), Some(k)),
                None => (r.number.parse::<usize>().unwrap_or(1), None),
            };
            count = count.max(sec);
            let number = match sub {
                Some(k) => format!("{}-{k}", done + sec),
                None => (done + sec).to_string(),
            };
            let t = (i as f64 + 0.5) / rows.len() as f64;
            out.push((from + (to - from) * t, number));
        }
        done += count;
    }
    out
}

/// The Staircase Information of a section with the counts of its staircase.
#[derive(Debug, Clone, PartialEq)]
pub struct StaircaseInfo {
    pub info: plan_stairs::Info,
    pub sections: usize,
    pub landings: usize,
    pub risers: u32,
}

pub fn staircase_info(floor: &Floor, id: Id) -> Option<StaircaseInfo> {
    let o = find(floor, id)?;
    let all = staircase(floor, id);
    let sections: Vec<&StairObj> = all.iter().filter(|s| !s.is_landing()).collect();
    let risers: u32 = sections.iter().map(|s| s.solution().risers).sum();
    let sol = o.solution();
    let rise = o.stair.params.total_rise;
    Some(StaircaseInfo {
        info: plan_stairs::info(rise, sol.risers, o.stair.params.tread_depth, true),
        sections: sections.len(),
        landings: all.len() - sections.len(),
        risers,
    })
}

// ----- lock end from the click -----

thread_local! {
    static LAST_CLICK: std::cell::Cell<Option<(Id, LockEnd)>> =
        const { std::cell::Cell::new(None) };
}

/// Which end a click at `p` locks on the section `o`: a click nearer its
/// bottom locks the top, nearer its top locks the bottom (`LockEnd::from_click`).
pub fn click_lock_end(o: &StairObj, p: Point) -> LockEnd {
    let (from, to) = (o.bottom_center(), top_point(&o.stair).0);
    let axis = to - from;
    let len2 = axis.x * axis.x + axis.y * axis.y;
    let along = if len2 < 1e-9 {
        0.0
    } else {
        ((p - from).x * axis.x + (p - from).y * axis.y) / len2
    };
    LockEnd::from_click(along.clamp(0.0, 1.0))
}

/// Picks the stair under `p` like `pick` and remembers which end the click
/// locks, for the dialog that opens next.
pub fn pick_noting_end(floor: &Floor, p: Point, tol: f64) -> Option<Id> {
    let id = pick(floor, p, tol)?;
    if let Some(o) = find(floor, id).filter(|o| !o.is_landing()) {
        LAST_CLICK.with(|c| c.set(Some((id, click_lock_end(&o, p)))));
    }
    Some(id)
}

/// The end remembered for `id` by the last click on it, if any.
pub fn noted_lock_end(id: Id) -> Option<LockEnd> {
    LAST_CLICK
        .with(|c| c.get())
        .filter(|(i, _)| *i == id)
        .map(|(_, e)| e)
}

// ----- landings: between sections, Add Break -----

/// The farthest apart two sections may be for a click between them to
/// place a landing.
pub const MAX_LANDING_GAP: f64 = 144.0;

/// The outline of a landing that fills the gap between the top of one
/// section and the bottom of another, for a click at `p` between them, or
/// `None` if `p` is not between two unjoined sections. The nearest pair
/// whose gap midpoint lies within the gap of `p` wins.
pub fn landing_between(floor: &Floor, p: Point) -> Option<Vec<Point>> {
    let secs: Vec<StairObj> = load(floor)
        .into_iter()
        .filter(|o| !o.is_landing())
        .collect();
    let mut best: Option<(f64, Vec<Point>)> = None;
    for a in &secs {
        for b in &secs {
            if a.id() == b.id() {
                continue;
            }
            let (top, bot) = (top_point(&a.stair).0, b.bottom_center());
            let gap = top.dist(bot);
            // Sections turning by less than 90 degrees need a short edge
            // of at least 6 inches (`short_edge`); a narrower gap is no landing.
            let turn = a.along().cross(b.along()).atan2(a.along().dot(b.along()));
            if gap > MAX_LANDING_GAP || plan_stairs::short_edge(gap, turn) > gap + 1e-9 || gap < 0.5
            {
                continue;
            }
            let mid = (top + bot) * 0.5;
            let d = mid.dist(p);
            if d > (gap * 0.5).max(24.0) || best.as_ref().is_some_and(|(bd, _)| *bd <= d) {
                continue;
            }
            let (ra, rb) = (
                a.right() * (a.stair.params.width * 0.5),
                b.right() * (b.stair.params.width * 0.5),
            );
            let (a_l, a_r) = (top - ra, top + ra);
            let (b_l, b_r) = (bot - rb, bot + rb);
            let straight = [a_l, a_r, b_r, b_l];
            let crossed = [a_l, a_r, b_l, b_r];
            let area = |q: &[Point; 4]| plan_core::geometry::polygon_area(q).abs();
            let quad = if area(&straight) >= area(&crossed) {
                straight
            } else {
                crossed
            };
            if area(&quad) < 1.0 {
                continue;
            }
            best = Some((d, quad.to_vec()));
        }
    }
    best.map(|(_, q)| q)
}

/// Splits the edge `edge` of a landing at the fraction `t` of its length
/// with a new corner, so a stair railing can meet the landing railing there
/// (CB-141). The two halves keep the railing of the edge. Returns whether
/// the landing changed.
pub fn add_break_to(o: &mut StairObj, edge: usize, t: f64) -> bool {
    if !o.is_landing() {
        return false;
    }
    let mut pts = o.footprint();
    let n = pts.len();
    if n < 3 || edge >= n || !(0.0..=1.0).contains(&t) {
        return false;
    }
    let (a, b) = (pts[edge], pts[(edge + 1) % n]);
    pts.insert(edge + 1, a + (b - a) * t.clamp(0.05, 0.95));
    let mut rails = o.stair.params.edge_rails.clone();
    rails.resize(n, plan_stairs::EdgeRail::Automatic);
    let rail = rails[edge];
    rails.insert(edge + 1, rail);
    o.stair.params.outline = pts;
    o.stair.params.edge_rails = rails;
    true
}

/// The index of the longest edge of the landing outline.
pub fn longest_edge(o: &StairObj) -> usize {
    let pts = o.footprint();
    let n = pts.len();
    (0..n)
        .max_by(|&i, &j| {
            let (li, lj) = (pts[i].dist(pts[(i + 1) % n]), pts[j].dist(pts[(j + 1) % n]));
            li.total_cmp(&lj)
        })
        .unwrap_or(0)
}

/// Add Break on the landing `id` (one undo step): the edge gets a corner at
/// fraction `t`.
pub fn add_landing_break(cx: &mut EditorContext, id: Id, edge: usize, t: f64) -> bool {
    let Some(mut o) = find(cx.floor(), id) else {
        return false;
    };
    if !add_break_to(&mut o, edge, t) {
        return false;
    }
    cx.begin_change("Add Break");
    let fl = cx.floor;
    update(&mut cx.project, fl, id, |x| *x = o);
    cx.mark_dirty();
    true
}

// ----- tread mode -----

/// The tread depth mode of a stair, from its two lock flags.
pub fn tread_mode(o: &StairObj) -> TreadMode {
    TreadMode::from_locks(o.x.lock_tread, o.x.lock_count)
}

/// Sets the tread depth mode of the stair `id` (one undo step). No Change
/// leaves the flags as they are.
pub fn set_tread_mode(cx: &mut EditorContext, id: Id, mode: TreadMode) -> bool {
    let Some((depth, count)) = mode.locks() else {
        return false;
    };
    if find(cx.floor(), id).is_none() {
        return false;
    }
    cx.begin_change("Tread Depth Mode");
    let fl = cx.floor;
    update(&mut cx.project, fl, id, |o| {
        o.x.lock_tread = depth;
        o.x.lock_count = count;
    });
    cx.mark_dirty();
    true
}

// ----- moving and resizing -----

fn translate(o: &mut StairObj, by: Point) {
    o.stair.origin = o.stair.origin + by;
    for p in &mut o.stair.params.outline {
        *p = *p + by;
    }
}

/// Length of a section along its walkline: treads times tread depth.
pub fn section_length(o: &StairObj) -> f64 {
    if o.is_landing() {
        return o.landing_depth().unwrap_or(0.0);
    }
    f64::from(o.solution().treads) * o.stair.params.tread_depth
}

/// Sets the length of a stepped section by the rule of its tread mode:
/// Automatic and Lock Number of Treads change the tread depth, Lock Tread
/// Depth changes the number of treads (the nearest whole tread; the riser
/// height follows from the rise), No Change refuses. Returns the length that
/// was reached.
pub fn set_length(o: &mut StairObj, len: f64) -> f64 {
    if o.is_landing() || o.is_ramp() {
        return section_length(o);
    }
    let mode = tread_mode(o);
    let len = len.max(1.0);
    match mode {
        TreadMode::NoChange => {}
        TreadMode::LockDepth => {
            let depth = o.stair.params.tread_depth.max(1.0);
            let treads = ((len / depth).round() as u32).max(1);
            let rise = o.stair.params.total_rise.max(1.0);
            o.stair.params.riser_height_target = rise / f64::from(treads + 1);
        }
        TreadMode::Automatic | TreadMode::LockCount => {
            let treads = o.solution().treads.max(1);
            o.stair.params.tread_depth = len / f64::from(treads);
        }
    }
    section_length(o)
}

/// Sets the length of section `id`, with `lock` saying which end stays: the
/// sections and landings on the moving side move with it (one undo step).
/// Returns how far the section grew.
pub fn resize_section(
    cx: &mut EditorContext,
    id: Id,
    len: f64,
    lock: LockEnd,
) -> Result<f64, String> {
    let fl = cx.floor;
    let before = find(cx.floor(), id).ok_or("Select a stair first")?;
    if before.is_landing() || before.is_ramp() {
        return Err("Lock Top and Lock Bottom apply to stair sections".into());
    }
    let mut probe = before.clone();
    let delta = set_length(&mut probe, len) - section_length(&before);
    if delta.abs() < 1e-9 && probe == before {
        return Err(if tread_mode(&before) == TreadMode::NoChange {
            "Tread depth mode is No Change"
        } else {
            "The length is unchanged"
        }
        .into());
    }
    let all = staircase(cx.floor(), id);
    cx.begin_change("Stair Section Length");
    update(&mut cx.project, fl, id, |o| *o = probe.clone());
    shift_for_resize(&mut cx.project, fl, &before, &all, delta, lock);
    cx.mark_dirty();
    Ok(delta)
}

/// After section `before` changed its length by `delta`: moves the section
/// (Lock Top moves its bottom end back) and the sections and landings on the
/// moving side of the locked end.
pub(super) fn shift_for_resize(
    project: &mut Project,
    fl: usize,
    before: &StairObj,
    all: &[StairObj],
    delta: f64,
    lock: LockEnd,
) {
    if delta.abs() < 1e-9 {
        return;
    }
    let (bottom, top) = (before.bottom_height(), before.top_height());
    let along = before.along();
    let below_shift = along * lock.bottom_shift(delta);
    let above_shift = along * lock.top_shift(delta);
    if below_shift.length() > 1e-9 {
        update(project, fl, before.id(), |o| translate(o, below_shift));
    }
    for o in all.iter().filter(|o| o.id() != before.id()) {
        let h = if o.is_landing() {
            o.landing_height()
        } else {
            o.bottom_height()
        };
        let by = if h <= bottom + 1e-6 && (o.is_landing() || o.top_height() <= bottom + 1e-6) {
            below_shift
        } else if h >= top - 1e-6 {
            above_shift
        } else {
            continue;
        };
        if by.length() > 1e-9 {
            update(project, fl, o.id(), |s| translate(s, by));
        }
    }
}

// ----- Make Best Fit -----

/// Make Best Fit: the staircase of `id` takes the riser height nearest
/// 6 3/4 inches that reaches its top exactly. Every section gets that riser
/// height; the others keep their riser counts and the selected section adds
/// or removes the difference (its end moves as `lock` says; heights above
/// follow, landings take the new tops).
pub fn make_best_fit(cx: &mut EditorContext, id: Id, lock: LockEnd) -> Result<String, String> {
    let fl = cx.floor;
    let o = find(cx.floor(), id).ok_or("Select a stair first")?;
    if o.is_landing() || o.is_ramp() {
        return Err("Make Best Fit applies to stair sections".into());
    }
    let all = staircase(cx.floor(), id);
    let secs: Vec<&StairObj> = all.iter().filter(|s| !s.is_landing()).collect();
    let total: f64 = secs.iter().map(|s| s.stair.params.total_rise).sum();
    let best = plan_stairs::best_fit(total);
    let others: u32 = secs
        .iter()
        .filter(|s| s.id() != id)
        .map(|s| s.solution().risers)
        .sum();
    let mine = o.solution().risers;
    if best.risers == others + mine {
        return Err("Already the Best Fit".into());
    }
    if best.risers < others + 2 {
        return Err("The other sections already use more risers than the Best Fit".into());
    }
    let want = best.risers - others;
    let h = best.riser_height;
    let len_before = section_length(&o);
    let mut base = secs.first().map_or(0.0, |s| s.bottom_height());
    cx.begin_change("Make Best Fit");
    let mut new_len = len_before;
    for s in &secs {
        let risers = if s.id() == id {
            want
        } else {
            s.solution().risers
        };
        let b = base;
        base += f64::from(risers) * h;
        let is_me = s.id() == id;
        update(&mut cx.project, fl, s.id(), |t| {
            t.stair.base = b;
            t.stair.params.total_rise = f64::from(risers) * h;
            t.stair.params.riser_height_target = h;
            if is_me {
                // A stair that keeps its tread depth grows by the new treads.
                new_len = f64::from(risers - 1) * t.stair.params.tread_depth;
            }
        });
    }
    // Landings take the tops of the sections that arrive on them.
    for l in all.iter().filter(|s| s.is_landing()) {
        connect(&mut cx.project, fl, l.id());
    }
    // The selected section's new length moves the sections beyond its locked
    // end.
    shift_for_resize(&mut cx.project, fl, &o, &all, new_len - len_before, lock);
    cx.mark_dirty();
    Ok(format!(
        "Make Best Fit: {} risers of {:.3} in",
        best.risers, best.riser_height
    ))
}

// ----- merging -----

/// Merges the section `upper` onto the top of `lower` (one undo step): one
/// section of two subsections. The upper object is deleted.
pub fn merge_sections(cx: &mut EditorContext, lower: Id, upper: Id) -> Result<String, String> {
    let fl = cx.floor;
    let a = find(cx.floor(), lower).ok_or("Select a stair first")?;
    let b = find(cx.floor(), upper).ok_or("Select a stair first")?;
    if a.is_landing() || b.is_landing() {
        return Err("Landings do not merge".into());
    }
    let merged = plan_stairs::merge(&a.stair, &b.stair).map_err(MergeError::message)?;
    cx.begin_change("Merge Sections");
    update(&mut cx.project, fl, lower, |o| {
        o.stair = merged;
        o.x.break_line = b.x.break_line;
        o.x.stairwell_walls
            .extend(b.x.stairwell_walls.iter().copied());
    });
    remove(&mut cx.project, fl, upper);
    cx.selection.set(ObjectRef::Stair(lower));
    cx.mark_dirty();
    Ok("Merge Sections: 2 subsections".into())
}

/// The section `id` can merge with: the one whose bottom meets its top
/// (`Some((id, upper))`) or whose top meets its bottom.
pub fn merge_partner(floor: &Floor, id: Id) -> Option<(Id, Id)> {
    let me = find(floor, id)?;
    if me.is_landing() {
        return None;
    }
    load(floor)
        .into_iter()
        .filter(|o| o.id() != id && !o.is_landing())
        .find_map(|o| {
            if plan_stairs::merge(&me.stair, &o.stair).is_ok() {
                Some((id, o.id()))
            } else if plan_stairs::merge(&o.stair, &me.stair).is_ok() {
                Some((o.id(), id))
            } else {
                None
            }
        })
}

// ----- landing rules -----

/// Applies the Auto Adjust rules to landing `id`: the thickness is one riser
/// plus the floor finish (6 3/4 inches free-standing, with no section
/// attached), and a landing that sits within an inch of an earlier one rises
/// one riser above it and loses the railing along the shared edges. Returns
/// true when anything changed.
pub fn adjust_landing(project: &mut Project, fl: usize, id: Id) -> bool {
    let objs = load(&project.floors[fl]);
    let Some(me) = objs
        .iter()
        .find(|o| o.id() == id && o.is_landing())
        .cloned()
    else {
        return false;
    };
    let attached: Vec<&StairObj> = objs
        .iter()
        .filter(|s| !s.is_landing() && joint(s, &me).is_some())
        .collect();
    let riser = attached
        .iter()
        .map(|s| s.solution().riser_height)
        .fold(0.0, f64::max);
    let free = attached.is_empty();
    let finish = 0.0;
    let mut thickness = me.stair.params.slab_thickness;
    if me.stair.params.landing_auto_thickness {
        thickness =
            plan_stairs::auto_thickness(if riser > 0.0 { riser } else { 7.5 }, finish, free);
    }
    // Earlier landings (smaller id) next to this one.
    let poly = me.footprint();
    let earlier: Vec<&StairObj> = objs
        .iter()
        .filter(|o| {
            o.is_landing() && o.id() < id && plan_stairs::are_adjacent(&o.footprint(), &poly)
        })
        .collect();
    let mut height = me.landing_height();
    let mut rails = me.stair.params.edge_rails.clone();
    if me.stair.params.landing_auto_height {
        if let Some(first) = earlier.iter().map(|l| l.landing_height()).reduce(f64::max) {
            let step = if riser > 0.0 { riser } else { 7.5 };
            height = plan_stairs::adjacent_height(first, step);
        }
    }
    if !earlier.is_empty() {
        let n = poly.len();
        rails.resize(n, plan_stairs::EdgeRail::Automatic);
        for l in &earlier {
            let theirs = l.footprint();
            let mut their_rails = l.stair.params.edge_rails.clone();
            their_rails.resize(theirs.len(), plan_stairs::EdgeRail::Automatic);
            for (i, j) in
                plan_stairs::adjacent_edges(&poly, &theirs, plan_stairs::ADJACENT_TOLERANCE)
            {
                rails[i] = plan_stairs::EdgeRail::No;
                their_rails[j] = plan_stairs::EdgeRail::No;
            }
            // The earlier landing loses the railing along the shared edge too.
            if their_rails != l.stair.params.edge_rails {
                update(project, fl, l.id(), |t| {
                    t.stair.params.edge_rails = their_rails
                });
            }
        }
    }
    let changed = (thickness - me.stair.params.slab_thickness).abs() > 1e-9
        || (height - me.landing_height()).abs() > 1e-9
        || rails != me.stair.params.edge_rails;
    if changed {
        update(project, fl, id, |l| {
            l.stair.params.slab_thickness = thickness;
            l.stair.params.total_rise = height;
            l.stair.params.edge_rails = rails;
        });
    }
    changed
}

/// Copies the Display on Floor Above rule of `id` to every section and
/// landing of its staircase, without opening an undo step. Returns how many
/// objects changed.
pub(super) fn copy_floor_above(project: &mut Project, fl: usize, id: Id) -> usize {
    let Some(me) = find(&project.floors[fl], id) else {
        return 0;
    };
    let rule = me.stair.params.plan.floor_above;
    let ids: Vec<Id> = staircase(&project.floors[fl], id)
        .into_iter()
        .filter(|o| o.id() != id && o.stair.params.plan.floor_above != rule)
        .map(|o| o.id())
        .collect();
    for i in &ids {
        update(project, fl, *i, |o| o.stair.params.plan.floor_above = rule);
    }
    ids.len()
}

/// Apply to All Connected Sections: copies the Display on Floor Above rule of
/// `id` to every section and landing of its staircase (one undo step).
/// Returns how many objects were changed.
pub fn apply_floor_above_to_connected(cx: &mut EditorContext, id: Id) -> usize {
    let fl = cx.floor;
    let probe = {
        let mut p = cx.project.clone();
        copy_floor_above(&mut p, fl, id)
    };
    if probe == 0 {
        return 0;
    }
    cx.begin_change("Display on Floor Above");
    let n = copy_floor_above(&mut cx.project, fl, id);
    cx.mark_dirty();
    n
}

/// Make Best Fit on a dialog draft, which sees only its own section: the
/// riser count nearest the Best Fit for its own rise. False when it already
/// is the Best Fit.
pub fn best_fit_draft(o: &mut StairObj) -> bool {
    if o.is_landing() || o.is_ramp() {
        return false;
    }
    let best = plan_stairs::best_fit(o.stair.params.total_rise);
    if best.risers == o.solution().risers || best.risers < 2 {
        return false;
    }
    super::set_risers(o, best.risers);
    true
}

// ----- moving sections together or apart (CB-125, CB-126) -----

/// The objects a drag of `id` moves: the whole staircase (its sections and
/// landings) unless the Stair Sections Move Independently preference is on;
/// Shift held for the drag flips that choice, so Shift on a section moves
/// it apart from its neighbours (and with the preference on, Shift moves
/// the whole staircase).
pub fn move_group(floor: &Floor, id: Id, independent: bool, shift: bool) -> Vec<Id> {
    if independent != shift {
        return vec![id];
    }
    let mut ids: Vec<Id> = staircase(floor, id).iter().map(StairObj::id).collect();
    if ids.is_empty() {
        ids.push(id);
    }
    ids
}

// ----- the stairwell stops the top of a stair (CB-104) -----

/// A dragged stair whose top would pass a wall of the floor above is held
/// short of it when the stair opens to a stairwell there: the top stops at
/// the wall's near face. Walls that belong to the stair's own stairwell are
/// ignored. Only straight runs stop; `dragged` comes back as it is for any
/// other case.
pub fn stairwell_stop(
    project: &Project,
    fl: usize,
    orig: &StairObj,
    dragged: StairObj,
) -> StairObj {
    if orig.is_landing() || orig.is_curved() || orig.is_ramp() {
        return dragged;
    }
    let Some(above) = project.floors.get(fl + 1) else {
        return dragged;
    };
    if !open_to_floor_above(project, fl, orig) {
        return dragged;
    }
    let (a, dir) = (orig.bottom_center(), orig.along());
    let want = (top_point(&dragged.stair).0 - a).dot(dir);
    let have = (top_point(&orig.stair).0 - a).dot(dir);
    if want <= have + 1e-6 {
        return dragged;
    }
    let mut limit = want;
    for w in &above.walls {
        if orig.x.stairwell_walls.contains(&w.id) || orig.x.guard_walls.contains(&w.id) {
            continue;
        }
        // Where the centre line a -> a + dir * want meets the wall.
        let e = w.end - w.start;
        let denom = dir.cross(e);
        if denom.abs() < 1e-9 {
            continue;
        }
        let q = w.start - a;
        let t = q.cross(e) / denom;
        let u = q.cross(dir) / denom;
        if (0.0..=1.0).contains(&u) && t > have - 1e-6 {
            limit = limit.min((t - w.thickness / 2.0).max(have));
        }
    }
    if limit >= want - 1e-9 {
        return dragged;
    }
    let mut out = dragged;
    set_run(&mut out, limit.max(MIN_SIZE));
    out
}

// ----- Convert Polyline to Landing -----

pub const CONVERT_POLYLINE: &str = "stairs.convert_polyline";

fn selected_polyline(cx: &EditorContext) -> Option<(Id, Vec<Point>)> {
    let ObjectRef::Cad(id) = cx.selection.single()? else {
        return None;
    };
    cx.floor().cad.iter().find_map(|c| match &c.item {
        plan_core::CadItem::Polyline { points, .. } if c.id == id && points.len() >= 3 => {
            Some((id, points.clone()))
        }
        _ => None,
    })
}

/// The Edit button for a selected closed-enough polyline.
pub fn edit_buttons(cx: &EditorContext) -> Vec<crate::editor::EditAction> {
    if selected_polyline(cx).is_none() {
        return Vec::new();
    }
    vec![crate::editor::EditAction::new(
        crate::editor::EditActionKind::Custom {
            id: CONVERT_POLYLINE,
            label: "Convert Polyline to Landing",
            icon: "",
        },
    )]
}

/// Runs this module's Edit command; false when `id` is not one of them.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    if id != CONVERT_POLYLINE {
        return false;
    }
    match convert_polyline_to_landing(cx) {
        Ok(msg) => cx.status = msg,
        Err(e) => cx.status = e,
    }
    true
}

/// Turns the selected CAD polyline into a polygon landing of the current
/// floor (one undo step); the polyline goes. The landing takes the
/// polyline's corners, the default landing height and railing.
pub fn convert_polyline_to_landing(cx: &mut EditorContext) -> Result<String, String> {
    let (cad_id, pts) =
        selected_polyline(cx).ok_or("Select a polyline of three or more corners")?;
    if plan_core::geometry::polygon_area(&pts).abs() < 1.0 {
        return Err("The polyline encloses no area".into());
    }
    let fl = cx.floor;
    let obj = build_polygon_landing(&cx.project, fl, &pts);
    cx.begin_change("Convert Polyline to Landing");
    cx.project.floors[fl].cad.retain(|c| c.id != cad_id);
    let id = add(&mut cx.project, fl, obj);
    connect(&mut cx.project, fl, id);
    adjust_landing(&mut cx.project, fl, id);
    cx.selection.set(ObjectRef::Stair(id));
    cx.mark_dirty();
    Ok("Convert Polyline to Landing".into())
}
