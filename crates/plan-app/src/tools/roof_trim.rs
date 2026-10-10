//! Roof trim as molding polylines (manual pp. 831 to 834, 861 to 879; RF-15,
//! RF-26, RF-52).
//!
//! Build Roof generates the pieces of the eaves, rakes and ridges as molding
//! polylines on the layer [`LAYER`]: rafter tails, ridge caps, gutters,
//! frieze, shadow boards, subfascia and lookouts (`plan_3d::roof_trim_lines`
//! works them out from the roof planes). They are ordinary `MoldingLine`s of
//! the floor's details layer with Automatically Generated on, so they select
//! and edit like any molding polyline (editing one clears the flag and a
//! later [`regenerate`] leaves it alone) and the Materials List counts them
//! under Exterior Trim.
//!
//! What is generated is set by the [`RoofTrimOptions`] kept with the roof as
//! `{"kind": "roof_trim", ...}` in `Floor.roofs` (`roof_view` passes the
//! record through). Nothing is made until a part is switched on.
//!
//! * [`regenerate`] replaces the automatic trim of a floor; `roof_view::rebuild`
//!   calls it after storing the planes (docs/integration-queue.md).
//! * [`build_trim`] is the command: one undo step, "Build Roof Trim".
//! * [`apply_options`] stores the options from the Roof Trim dialog.

// The entry points are called from the roof, menu and edit-toolbar owners'
// files once the hooks in docs/integration-queue.md are in; until then
// only the scenario tests use them.
#![allow(dead_code)]

use crate::editor::{details_view, EditorContext};
use plan_3d::{roof_trim_lines, RoofCover, RoofTrimOptions, TrimKind, TrimLine};
use plan_core::details::{DetailsLayer, MoldingLine, MoldingProfile, MoldingSource};
use plan_core::geometry::Point;
use plan_core::layers::Layer;
use plan_core::moldings::{find_profile, MoldingEntry, MoldingTable, MoldingType, ProfileDef};
use plan_core::{Floor, Project};
use serde_json::{json, Value};

/// The layer the generated trim is on.
pub const LAYER: &str = "Roofs, Trim";
/// `kind` of the options record in `Floor.roofs`.
const KIND: &str = "roof_trim";
/// Start of the label of every generated line.
pub const LABEL: &str = "Roof Trim: ";

fn is_record(v: &Value) -> bool {
    v.get("kind").and_then(Value::as_str) == Some(KIND)
}

/// The trim options of `floor` (nothing switched on before they are set).
pub fn options(floor: &Floor) -> RoofTrimOptions {
    floor
        .roofs
        .iter()
        .find(|v| is_record(v))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

/// Stores `opts` as the trim options of floor `fi`.
pub fn set_options(project: &mut Project, fi: usize, opts: &RoofTrimOptions) {
    let floor = &mut project.floors[fi];
    floor.roofs.retain(|v| !is_record(v));
    if let Ok(Value::Object(mut m)) = serde_json::to_value(opts) {
        m.insert("kind".into(), json!(KIND));
        floor.roofs.push(Value::Object(m));
    }
}

/// Is `m` a molding line this module generated (edited or not)?
pub fn is_roof_trim(m: &MoldingLine) -> bool {
    m.layer == LAYER && m.label.starts_with(LABEL)
}

/// The trim of floor `fi` for `opts`, from the roof as it is stored.
pub fn trim_lines(project: &Project, fi: usize, opts: &RoofTrimOptions) -> Vec<TrimLine> {
    if !opts.any() {
        return Vec::new();
    }
    let cover = RoofCover::from_project(project);
    cover
        .floor(fi)
        .map(|f| roof_trim_lines(&f.eaves, &f.detail, opts))
        .unwrap_or_default()
}

/// The molding polyline for `line`: its library profile (or a plain board)
/// at the part's size, a 3D line along the roof edge. The label names the
/// part so the Materials List and the specification show what it is.
pub fn molding_of(line: &TrimLine, id: plan_core::Id, own: &[ProfileDef]) -> MoldingLine {
    let part = line.kind.label();
    let mut points = line.points.clone();
    let mut bottoms = line.bottoms.clone();
    if line.closed {
        points.push(points[0]);
        bottoms.push(bottoms[0]);
    }
    let named = (!line.profile.is_empty())
        .then(|| find_profile(&line.profile, own))
        .flatten();
    let (mut def, name) = match (&line.section, named) {
        (Some(section), _) => {
            // A section placed in space (a ridge cap on its plane): the
            // profile is moved to its back-bottom corner and the line's
            // heights follow it.
            let lo = section.iter().fold(f64::MAX, |m, p| m.min(p.y));
            for b in &mut bottoms {
                *b += lo;
            }
            let def = ProfileDef::from_polyline(part, MoldingType::Eave, section)
                .unwrap_or_else(|_| plan_core::moldings::square_profile());
            (def, part.to_string())
        }
        (None, Some(def)) => {
            let name = format!("{part} - {}", def.name);
            (def, name)
        }
        (None, None) => {
            let rect = [
                Point::new(0.0, 0.0),
                Point::new(line.width, 0.0),
                Point::new(line.width, line.height),
                Point::new(0.0, line.height),
            ];
            let def = ProfileDef::from_polyline(part, MoldingType::Eave, &rect)
                .unwrap_or_else(|_| plan_core::moldings::square_profile());
            (def, part.to_string())
        }
    };
    def.name = name;
    def.kind = MoldingType::Eave;
    let mut row = MoldingEntry::new(def);
    if line.section.is_none() {
        row.width = line.width.max(0.01);
        row.height = line.height.max(0.01);
    }
    row.kind = MoldingType::Eave;
    let elevation = bottoms.iter().copied().fold(f64::MAX, f64::min);
    let level = bottoms.iter().all(|b| (b - bottoms[0]).abs() < 1e-6);
    MoldingLine {
        id,
        polyline: points,
        profile: MoldingProfile::Custom(row.profile.section().to_vec()),
        height: row.height,
        width: row.width,
        elevation: if level { bottoms[0] } else { elevation },
        heights: if level { Vec::new() } else { bottoms },
        side: line.side,
        layer: LAYER.to_string(),
        automatic: true,
        source: MoldingSource::Manual,
        table: MoldingTable {
            rows: vec![row],
            ..MoldingTable::default()
        },
        label: format!("{LABEL}{part}"),
        ..MoldingLine::default()
    }
}

fn ensure_layer(project: &mut Project) {
    if project.layers.get(LAYER).is_none() {
        project.layers.add(Layer::new(LAYER, [120, 80, 50], 18));
    }
}

/// Replaces the automatic roof trim of floor `fi` with what the stored
/// options and roof make now. A line that was edited by hand (its Automatic
/// flag cleared) stays. Returns the number of lines generated.
pub fn regenerate(project: &mut Project, fi: usize) -> usize {
    let opts = options(&project.floors[fi]);
    let mut layer = DetailsLayer::load(&project.floors[fi]);
    let before = layer.moldings.len();
    layer.moldings.retain(|m| !(is_roof_trim(m) && m.automatic));
    let had_any = before != layer.moldings.len();
    let lines = trim_lines(project, fi, &opts);
    if lines.is_empty() && !had_any {
        return 0;
    }
    ensure_layer(project);
    let own = layer.profiles.clone();
    let kept: Vec<MoldingLine> = layer
        .moldings
        .iter()
        .filter(|m| is_roof_trim(m))
        .cloned()
        .collect();
    let mut made = 0;
    for l in &lines {
        let id = project.alloc_id();
        let m = molding_of(l, id, &own);
        // A line edited by hand stands in for the one Build Roof would make
        // in the same place.
        if kept.iter().any(|k| replaces(k, &m)) {
            continue;
        }
        layer.moldings.push(m);
        made += 1;
    }
    details_view::save(project, fi, &layer);
    made
}

/// Slack when deciding that an edited line is the one a new line replaces,
/// inches.
const REPLACE_SLACK: f64 = 12.0;

/// Does the hand-edited line `old` stand where the generated line `new` would
/// go: the same part, and the middle of `new` within [`REPLACE_SLACK`] of the
/// plan bounds of `old`?
fn replaces(old: &MoldingLine, new: &MoldingLine) -> bool {
    if old.label != new.label || new.polyline.is_empty() || old.polyline.is_empty() {
        return false;
    }
    let (mut lo, mut hi) = (old.polyline[0], old.polyline[0]);
    for p in &old.polyline {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let mid = Point::lerp(new.polyline[0], new.polyline[new.polyline.len() / 2], 0.5);
    mid.x >= lo.x - REPLACE_SLACK
        && mid.x <= hi.x + REPLACE_SLACK
        && mid.y >= lo.y - REPLACE_SLACK
        && mid.y <= hi.y + REPLACE_SLACK
}

/// Build Roof Trim: generates the trim for the active floor from its
/// options. One undo step; false (and no step) when nothing is switched on.
pub fn build_trim(cx: &mut EditorContext) -> bool {
    let fl = cx.floor;
    let opts = options(cx.floor());
    if !opts.any() {
        cx.status = "Switch a roof trim part on in the Roof Trim dialog first".into();
        return false;
    }
    cx.begin_change("Build Roof Trim");
    let n = regenerate(&mut cx.project, fl);
    cx.mark_dirty();
    cx.refresh();
    cx.status = format!("{n} roof trim line(s) generated");
    n > 0
}

/// Roof Trim dialog OK: stores `opts` and regenerates the trim, as one undo
/// step.
pub fn apply_options(cx: &mut EditorContext, opts: &RoofTrimOptions) -> usize {
    let fl = cx.floor;
    cx.begin_change("Roof Trim");
    set_options(&mut cx.project, fl, opts);
    let n = regenerate(&mut cx.project, fl);
    cx.mark_dirty();
    cx.refresh();
    n
}

/// The generated trim of floor `fi` per kind, for the dialog's summary and
/// the tests: `(kind, lines, total length in inches)`.
pub fn summary(floor: &Floor) -> Vec<(TrimKind, usize, f64)> {
    let layer = DetailsLayer::load(floor);
    TrimKind::ALL
        .iter()
        .filter_map(|k| {
            let label = format!("{LABEL}{}", k.label());
            let lines: Vec<&MoldingLine> = layer
                .moldings
                .iter()
                .filter(|m| is_roof_trim(m) && m.label == label)
                .collect();
            (!lines.is_empty()).then(|| (*k, lines.len(), lines.iter().map(|m| m.length()).sum()))
        })
        .collect()
}
