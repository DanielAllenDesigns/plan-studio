//! The Staircase Specification's read-outs and rules: Best Fit riser height,
//! tread depth modes, the Lock End action, the table of sections and
//! subsections, and merging two flights into one section.

use crate::layout::Layout;
use crate::{solve, RadiusRef, Stair, StairShape};
use plan_core::Point;

/// Chief's ideal riser height, inches: the Best Fit riser is the one closest
/// to this that still reaches the next level exactly.
pub const BEST_FIT_RISER: f64 = 6.75;
/// The specification table lists at most this many sections and subsections.
pub const SPEC_ROWS: usize = 10;
/// How far apart the ends of two flights may be and still merge, inches.
pub const MERGE_TOLERANCE: f64 = 6.0;

/// The riser count and height that suit a rise best.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BestFit {
    pub risers: u32,
    pub riser_height: f64,
}

/// The riser height closest to 6 3/4 inches that divides `rise` evenly.
/// Of the two counts either side of `rise / 6.75` the one whose riser height
/// lies nearer 6.75 wins; on a tie the higher count (shallower stair).
pub fn best_fit(rise: f64) -> BestFit {
    let rise = rise.max(0.0);
    let ideal = rise / BEST_FIT_RISER;
    let lo = (ideal.floor() as u32).max(1);
    let hi = (ideal.ceil() as u32).max(1);
    let dist = |n: u32| (rise / f64::from(n) - BEST_FIT_RISER).abs();
    let risers = if dist(lo) + 1e-9 < dist(hi) { lo } else { hi };
    BestFit {
        risers,
        riser_height: rise / f64::from(risers),
    }
}

/// How a staircase compares with its Best Fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitStatus {
    /// It uses the Best Fit count.
    Best,
    /// Fewer, taller risers than the Best Fit: a steeper rise angle.
    Steeper,
    /// More, shorter risers: a shallower rise angle.
    Shallower,
}

/// Compares a stair of `risers` over `rise` with the Best Fit.
pub fn fit_status(risers: u32, rise: f64) -> FitStatus {
    let best = best_fit(rise).risers;
    match risers.cmp(&best) {
        std::cmp::Ordering::Equal => FitStatus::Best,
        std::cmp::Ordering::Less => FitStatus::Steeper,
        std::cmp::Ordering::Greater => FitStatus::Shallower,
    }
}

/// The Staircase Information read-outs for one stair section.
#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    /// "Reaches the next level, steeper than the Best Fit" and so on.
    pub reach: String,
    /// "Best fit riser height of 6.75 requires 16 total risers to reach ...".
    pub best_fit: String,
    pub status: FitStatus,
    pub best: BestFit,
    /// Rise angle of the section, degrees.
    pub rise_angle: f64,
    /// Make Best Fit has something to do: Automatic Heights on and the
    /// current count differs from the Best Fit.
    pub can_make_best_fit: bool,
}

/// The read-outs for a section that climbs `rise` in `risers` risers with
/// treads `tread` deep. `automatic` is the Automatic Heights check box.
pub fn info(rise: f64, risers: u32, tread: f64, automatic: bool) -> Info {
    let best = best_fit(rise);
    let status = fit_status(risers, rise);
    let riser = if risers > 0 {
        rise / f64::from(risers)
    } else {
        0.0
    };
    let reach = if !automatic {
        "Start and end heights are set manually".to_string()
    } else {
        match status {
            FitStatus::Best => "Reaches the next level, at the Best Fit rise angle".into(),
            FitStatus::Steeper => "Reaches the next level, steeper than the Best Fit".into(),
            FitStatus::Shallower => "Reaches the next level, shallower than the Best Fit".into(),
        }
    };
    Info {
        reach,
        best_fit: format!(
            "Best fit riser height of {:.3} requires {} total risers to reach {:.2} to next level",
            best.riser_height, best.risers, rise
        ),
        status,
        best,
        rise_angle: rise_angle(riser, tread),
        can_make_best_fit: automatic && status != FitStatus::Best,
    }
}

/// The angle of the stair's slope, degrees.
pub fn rise_angle(riser: f64, tread: f64) -> f64 {
    if tread <= 0.0 {
        return 90.0;
    }
    (riser / tread).atan().to_degrees()
}

/// How the tread depth is determined (Advanced Options).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TreadMode {
    /// The program defines the depth and number of treads.
    #[default]
    Automatic,
    /// The depth is fixed; a change of length changes the number of treads.
    LockDepth,
    /// The number of treads is fixed; a change of length changes the depth.
    LockCount,
    /// Several sections with their own settings stay as they are.
    NoChange,
}

impl TreadMode {
    pub const ALL: [TreadMode; 4] = [
        TreadMode::Automatic,
        TreadMode::LockDepth,
        TreadMode::LockCount,
        TreadMode::NoChange,
    ];

    pub fn name(self) -> &'static str {
        match self {
            TreadMode::Automatic => "Automatic Treads",
            TreadMode::LockDepth => "Lock Tread Depth",
            TreadMode::LockCount => "Lock Number of Treads",
            TreadMode::NoChange => "No Change",
        }
    }

    /// The mode two lock flags stand for. Both on is "No Change": each
    /// value stays where it is.
    pub fn from_locks(depth: bool, count: bool) -> Self {
        match (depth, count) {
            (false, false) => TreadMode::Automatic,
            (true, false) => TreadMode::LockDepth,
            (false, true) => TreadMode::LockCount,
            (true, true) => TreadMode::NoChange,
        }
    }

    /// `(lock depth, lock count)` the mode sets; `None` leaves both as they
    /// are (No Change).
    pub fn locks(self) -> Option<(bool, bool)> {
        match self {
            TreadMode::Automatic => Some((false, false)),
            TreadMode::LockDepth => Some((true, false)),
            TreadMode::LockCount => Some((false, true)),
            TreadMode::NoChange => None,
        }
    }
}

/// Which end of the selected section stays put when its length changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockEnd {
    /// The top stays; the section and everything below it move.
    Top,
    /// The bottom stays; the section's top and everything above it move.
    Bottom,
}

impl LockEnd {
    /// Selecting a staircase near its bottom end locks the top, near its top
    /// end locks the bottom. `along` is the click's position from 0 (bottom)
    /// to 1 (top).
    pub fn from_click(along: f64) -> Self {
        if along < 0.5 {
            LockEnd::Top
        } else {
            LockEnd::Bottom
        }
    }

    /// How far the section's bottom end moves along its direction of travel
    /// when its length grows by `delta` (negative to shrink): the bottom
    /// moves back with Lock Top, not at all with Lock Bottom.
    pub fn bottom_shift(self, delta: f64) -> f64 {
        match self {
            LockEnd::Top => -delta,
            LockEnd::Bottom => 0.0,
        }
    }

    /// Likewise for the top end.
    pub fn top_shift(self, delta: f64) -> f64 {
        match self {
            LockEnd::Top => 0.0,
            LockEnd::Bottom => delta,
        }
    }
}

/// One line of the specification table.
#[derive(Debug, Clone, PartialEq)]
pub struct SpecRow {
    /// "1" for a section, "1-2" for the second subsection of section 1.
    pub number: String,
    /// Run along the walkline: treads times tread depth.
    pub length: f64,
    pub width: f64,
    pub tread_depth: f64,
    pub treads: u32,
    pub bottom_height: f64,
    pub top_height: f64,
    pub riser_height: f64,
    /// Radius of a curved section, measured as the Radius Reference says.
    pub radius: Option<f64>,
    pub winders: bool,
}

/// The sections of `stair` bottom to top as `(risers, width)`: a straight,
/// curved or winder stair is one; an L or U stair is its flights.
fn flights(stair: &Stair) -> Vec<(u32, f64)> {
    match stair.params.shape {
        StairShape::LShaped { .. } | StairShape::UShaped { .. } => {
            let layout = Layout::build(stair);
            layout.flights.iter().map(|f| (f.risers, f.width)).collect()
        }
        _ => vec![(solve(&stair.params).risers, stair.params.width)],
    }
}

/// The table of a staircase whose flights are `stairs` bottom to top.
/// Section numbers run 1, 2, ...; a section made of merged subsections is
/// listed once per subsection as 1-1, 1-2, ... Landings are not sections.
/// At most [`SPEC_ROWS`] rows.
pub fn spec_rows(stairs: &[&Stair]) -> Vec<SpecRow> {
    let mut rows = Vec::new();
    let mut section = 0;
    for st in stairs {
        if matches!(st.params.shape, StairShape::Landing { .. }) {
            continue;
        }
        let p = &st.params;
        let sol = solve(p);
        let curved = matches!(p.shape, StairShape::Curved { .. });
        let radius = curved.then(|| p.curve_radius(p.radius_ref)).flatten();
        let winders = matches!(p.shape, StairShape::Winder { .. });
        let mut base = st.base;
        for (risers, width) in flights(st) {
            section += 1;
            let h = if sol.risers > 0 {
                sol.riser_height
            } else {
                0.0
            };
            let subs: Vec<u32> =
                if p.subsections.len() >= 2 && p.subsections.iter().sum::<u32>() + 1 == risers {
                    p.subsections.clone()
                } else {
                    vec![risers.saturating_sub(1)]
                };
            let split = subs.len() >= 2;
            let mut sub_base = base;
            for (k, &treads) in subs.iter().enumerate() {
                // The first subsection also holds the bottom riser.
                let steps = treads + u32::from(k == 0 || !split);
                let top = sub_base + f64::from(steps) * h;
                rows.push(SpecRow {
                    number: if split {
                        format!("{section}-{}", k + 1)
                    } else {
                        section.to_string()
                    },
                    length: f64::from(treads) * p.tread_depth,
                    width,
                    tread_depth: p.tread_depth,
                    treads,
                    bottom_height: sub_base,
                    top_height: top,
                    riser_height: h,
                    radius,
                    winders,
                });
                sub_base = top;
            }
            base += f64::from(risers) * h;
        }
    }
    rows.truncate(SPEC_ROWS);
    rows
}

/// Why two sections cannot merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeError {
    /// A ramp cannot merge with a stair (connect them with a landing) or
    /// with another ramp here.
    Ramp,
    /// Only straight flights merge in this build; a curved section keeps its
    /// own subsection.
    Shape,
    /// One runs up, the other down.
    Direction,
    /// The sections are not parallel (or are drawn in opposite directions).
    NotParallel,
    /// The top of the lower section does not meet the bottom of the upper.
    Ends,
    /// The upper section does not start where the lower one ends in height
    /// (Automatic Heights).
    Heights,
    /// The widths differ: one section has one width.
    Width,
}

impl MergeError {
    pub fn message(self) -> &'static str {
        match self {
            MergeError::Ramp => "Stairs and ramps cannot merge; connect them with a landing",
            MergeError::Shape => "Only straight sections merge",
            MergeError::Direction => "Stairs drawn up and down cannot merge",
            MergeError::NotParallel => "Sections must be parallel and drawn in the same direction",
            MergeError::Ends => "The top of the lower section must meet the bottom of the upper",
            MergeError::Heights => "Sections must use Automatic Heights to merge",
            MergeError::Width => "Merged sections must have the same width",
        }
    }
}

/// Merges `upper` onto the top of `lower` (Extend handle dragged onto the
/// other section): one straight section made of two subsections. It keeps
/// `lower`'s id, position and style. The run of the two plus the gap between
/// them is spread over every tread (Ignore Subsection Boundaries), the
/// junction tread being made from the gap; the risers all share one height.
pub fn merge(lower: &Stair, upper: &Stair) -> Result<Stair, MergeError> {
    let (a, b) = (&lower.params, &upper.params);
    if matches!(a.shape, StairShape::Ramp { .. }) || matches!(b.shape, StairShape::Ramp { .. }) {
        return Err(MergeError::Ramp);
    }
    let curved = matches!(a.shape, StairShape::Curved { .. });
    if curved != matches!(b.shape, StairShape::Curved { .. })
        || (!curved && (a.shape != StairShape::Straight || b.shape != StairShape::Straight))
    {
        return Err(MergeError::Shape);
    }
    if a.down != b.down {
        return Err(MergeError::Direction);
    }
    if (a.width - b.width).abs() > 0.01 {
        return Err(MergeError::Width);
    }
    let (sa, sb) = (solve(a), solve(b));
    let gap = if curved {
        // Two curved sections merge when they turn the same way about one
        // centre with one radius, the upper starting where the lower ends.
        if a.turn != b.turn
            || (a.curve_radius(RadiusRef::InnerArc) != b.curve_radius(RadiusRef::InnerArc))
        {
            return Err(MergeError::NotParallel);
        }
        curved_gap(lower, upper).ok_or(MergeError::Ends)?
    } else {
        let turn = (lower.direction - upper.direction).rem_euclid(std::f64::consts::TAU);
        if turn.min(std::f64::consts::TAU - turn) > 0.01 {
            return Err(MergeError::NotParallel);
        }
        let along = Point::new(lower.direction.cos(), lower.direction.sin());
        let right = Point::new(along.y, -along.x);
        let top_a = lower.origin + along * (f64::from(sa.treads) * a.tread_depth);
        let gap_vec = upper.origin - top_a;
        let lateral = gap_vec.x * right.x + gap_vec.y * right.y;
        if lateral.abs() > MERGE_TOLERANCE {
            return Err(MergeError::Ends);
        }
        gap_vec.x * along.x + gap_vec.y * along.y
    };
    if gap < -MERGE_TOLERANCE || gap > 3.0 * a.tread_depth.max(b.tread_depth) {
        return Err(MergeError::Ends);
    }
    if (upper.base - (lower.base + a.total_rise)).abs() > 0.01 {
        return Err(MergeError::Heights);
    }
    let gap = gap.max(0.0);
    let risers = sa.risers + sb.risers;
    let treads = risers - 1;
    let run = f64::from(sa.treads) * a.tread_depth + gap + f64::from(sb.treads) * b.tread_depth;
    let mut out = lower.clone();
    out.params.total_rise = a.total_rise + b.total_rise;
    out.params.riser_height_target = out.params.total_rise / f64::from(risers);
    out.params.tread_depth = run / f64::from(treads);
    out.params.top_landing = b.top_landing;
    out.params.subsections = vec![sa.treads, sb.treads + 1];
    Ok(out)
}

/// The gap along the walking line between the top of curved section `lower`
/// and the bottom of curved section `upper`, or `None` when they do not lie
/// on one circle (centres or radii more than [`MERGE_TOLERANCE`] apart).
fn curved_gap(lower: &Stair, upper: &Stair) -> Option<f64> {
    let (la, lb) = (Layout::build(lower), Layout::build(upper));
    let (ca, cb) = (la.curve?, lb.curve?);
    let (c_a, c_b) = (la.frame.uv(ca.center), lb.frame.uv(cb.center));
    if c_a.dist(c_b) > MERGE_TOLERANCE {
        return None;
    }
    let mid = |c: &crate::layout::Curve| c.width / 2.0;
    let end = la.frame.uv(ca.at_lat(ca.sweep(), mid(&ca)));
    let start = lb.frame.uv(cb.at_lat(0.0, mid(&cb)));
    let ang = |p: Point| (p.y - c_a.y).atan2(p.x - c_a.x);
    let mut d = ang(start) - ang(end);
    if !ca.left {
        d = -d;
    }
    let tau = std::f64::consts::TAU;
    d = (d + std::f64::consts::PI).rem_euclid(tau) - std::f64::consts::PI;
    Some(d * ca.walk())
}
