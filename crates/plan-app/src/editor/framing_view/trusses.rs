//! Trusses in the plan: where a roof truss gets its shape from, Force Truss
//! Rebuild, Lock Truss Envelope and Webbing, and the TR-X / FTR-X labels.
//!
//! A roof truss's envelope follows the roof above it: when it is drawn,
//! moved or rebuilt it takes the pitch of the roof plane over its middle. A
//! locked truss keeps what it has. A floor or ceiling truss is as deep as
//! the platform it was drawn in and does not change when it moves.

use super::{
    all_manual_in_plan, load_records, roof_of, store_records, EditorContext, LAYER_TRUSSES,
};
use plan_core::geometry::{point_in_polygon, Point};
use plan_core::{Id, Project};
use plan_framing::{FramingMember, ManualMemberKind};
use plan_roof::Roof;

/// Layer of the labels of roof trusses.
pub const LAYER_ROOF_TRUSS_LABELS: &str = "Framing, Roof Truss Labels";
/// Layer of the labels of floor and ceiling trusses.
pub const LAYER_FLOOR_TRUSS_LABELS: &str = "Framing, Floor/Ceiling Truss Labels";

/// The roof plane's pitch over `at`, if the roof has a plane there.
pub fn pitch_over(roof: &Roof, at: Point) -> Option<f64> {
    roof.planes
        .iter()
        .find(|p| {
            let poly: Vec<Point> = p.polygon3d.iter().map(|v| Point::new(v[0], -v[2])).collect();
            point_in_polygon(at, &poly)
        })
        .map(|p| p.pitch_in_12)
}

/// Makes truss `m` fit where it stands (Force Truss Rebuild, and what a roof
/// truss does when it is moved): a roof truss takes the pitch of the roof
/// plane over its middle, a floor or ceiling truss the depth of its platform;
/// the span is the member's length. A locked truss is left as it is.
pub fn conform(m: &mut FramingMember, roof: Option<&Roof>) {
    if !m.kind.is_truss() {
        return;
    }
    let mid = Point::lerp(m.start, m.end, 0.5);
    let len = m.plan_length();
    let Some(spec) = m.truss.as_mut() else {
        return;
    };
    if spec.locked {
        return;
    }
    spec.span = len;
    if m.kind != ManualMemberKind::FloorCeilingTruss {
        if let Some(p) = roof.and_then(|r| pitch_over(r, mid)) {
            spec.pitch = p;
        }
    }
}

/// A moved roof truss conforms to the roof where it lands; a floor or ceiling
/// truss, and a locked one, keep what they have.
pub fn conform_moved(project: &mut Project, fi: usize, ids: &[Id]) {
    let roof = roof_of(&project.floors[fi]);
    let mut records = load_records(&project.floors[fi]);
    let mut changed = false;
    for r in &mut records {
        if !ids.contains(&r.id()) {
            continue;
        }
        if let super::Record::Manual(m) | super::Record::Built(m) = r {
            if m.kind.is_truss() && m.kind != ManualMemberKind::FloorCeilingTruss {
                let before = m.truss.clone();
                conform(m, roof.as_ref());
                changed |= m.truss != before;
            }
        }
    }
    if changed {
        store_records(&mut project.floors[fi], &records);
    }
}

/// The automatic label of every truss member in the plan, by id (`TR-1`, the
/// order each distinct configuration first appeared; `FTR-1` for floor and
/// ceiling trusses). Trusses with the same configuration share a label.
pub fn truss_labels(project: &Project) -> Vec<(Id, String)> {
    let members = all_manual_in_plan(project);
    plan_framing::truss_labels(&members, &[])
}

/// What a truss shows in plan: the label the user typed, else the
/// automatic one.
pub fn label_of(project: &Project, id: Id) -> Option<String> {
    let members = all_manual_in_plan(project);
    let m = members.iter().find(|m| m.id == id)?;
    if !m.custom_label.trim().is_empty() {
        return Some(m.custom_label.clone());
    }
    truss_labels(project)
        .into_iter()
        .find(|(i, _)| *i == id)
        .map(|(_, l)| l)
}

/// The layer a truss's label is drawn on.
pub fn label_layer(kind: ManualMemberKind) -> &'static str {
    if kind == ManualMemberKind::FloorCeilingTruss {
        LAYER_FLOOR_TRUSS_LABELS
    } else {
        LAYER_ROOF_TRUSS_LABELS
    }
}

/// Whether the framing tools' layer of trusses is shown on `cx`.
pub fn trusses_visible(cx: &EditorContext) -> bool {
    cx.layers().is_visible(LAYER_TRUSSES)
}
