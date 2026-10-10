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
