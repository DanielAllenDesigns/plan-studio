//! The framing buttons of the Edit toolbar and their commands (manual pp.
//! 914 to 927, 941 and 943): Build Framing for Selected Object(s) and for
//! Parent Object(s), Open Wall Detail, Find Wall, Move to Framing Ref, Join
//! and Lap Ends, Join and Mitre Ends, Add Break, Open Truss Detail and Find
//! Trusses, and the member edits of a Wall Detail.
//!
//! `EditorContext::extra_edit_actions` lists [`edit_actions`] for every tool
//! and `run_custom` tries [`run_command`].

use super::selected::{self, Outcome, Target};
use super::{details, find, selected as selected_ids, EditorContext, ObjectRef};
use crate::editor::actions::{EditAction, EditActionKind};
use plan_core::geometry::Point;
use plan_core::{Id, Wall};
use plan_framing::{FlatTo, Member};

/// Command ids.
pub mod cmd {
    pub const BUILD_SELECTED: &str = "framing.build_selected";
    pub const BUILD_PARENT: &str = "framing.build_parent";
    pub const OPEN_WALL_DETAIL: &str = "framing.open_wall_detail";
    pub const FIND_WALL: &str = "framing.find_wall";
    pub const MOVE_TO_REF: &str = "framing.move_to_ref";
    pub const JOIN_LAP: &str = "framing.join_lap";
    pub const JOIN_MITRE: &str = "framing.join_mitre";
    pub const ADD_BREAK: &str = "framing.add_break";
    pub const OPEN_TRUSS_DETAIL: &str = "framing.open_truss_detail";
    pub const FIND_TRUSSES: &str = "framing.find_trusses";
    pub const WALL_MEMBER_DELETE: &str = "framing.wall_member_delete";
    pub const FLAT_INSIDE: &str = "framing.flat_inside";
    pub const FLAT_OUTSIDE: &str = "framing.flat_outside";
    /// Opens the Truss Detail window (Build > Framing > Truss Detail).
    pub const TRUSS_WINDOW: &str = "framing.truss_window";
}

fn button(id: &'static str, label: &'static str, enabled: bool) -> EditAction {
    EditAction {
        kind: EditActionKind::Custom {
            id,
            label,
            icon: "",
        },
        label,
        icon: None,
        enabled,
    }
}

/// Whether a target's framing is retained, which disables the build buttons.
fn retained(cx: &EditorContext, t: &Target) -> bool {
    let st = super::settings(&cx.project);
    match t {
        Target::Wall(id) => st.build.retain_walls.contains(id),
        Target::RoofPlane(id) => st.build.retain_planes.contains(id),
        Target::Tray(id) => cx.floor().trays.get(*id).is_some_and(|t| t.retain_framing),
        Target::Room(i) => cx.rooms.get(*i).is_some_and(|r| {
            r.name_entry(&cx.floor().room_names)
                .is_some_and(|n| n.options.retain_framing)
        }),
        Target::Truss(id) => find(cx.floor(), *id)
            .and_then(|r| r.member().and_then(|m| m.truss.as_ref().map(|t| t.locked)))
            .unwrap_or(false),
    }
}

/// The manual linear members selected on the active floor.
fn selected_linear(cx: &EditorContext) -> Vec<Id> {
    selected_ids(cx)
        .into_iter()
        .filter(|id| {
            find(cx.floor(), *id)
                .and_then(|r| r.member().map(|m| m.is_linear() || m.kind.is_truss()))
                .unwrap_or(false)
        })
        .collect()
}

/// The Edit toolbar buttons the framing objects add to the selection.
pub fn edit_actions(cx: &EditorContext) -> Vec<EditAction> {
    let mut v = Vec::new();
    // In a Wall Detail the CAD objects stand for members.
    if details::in_wall_detail(cx) {
        if !details::selected_wall_members(cx).is_empty() {
            v.push(button(cmd::BUILD_PARENT, "Build Framing for Parent Object(s)", true));
            v.push(button(cmd::FIND_WALL, "Find Wall", true));
            v.push(button(cmd::FLAT_INSIDE, "Flat to Inside", true));
            v.push(button(cmd::FLAT_OUTSIDE, "Flat to Outside", true));
            v.push(button(cmd::WALL_MEMBER_DELETE, "Delete Framing Member(s)", true));
        }
        return v;
    }
    if details::in_truss_detail(cx) {
        if cx
            .selection
            .items
            .iter()
            .any(|o| matches!(o, ObjectRef::Cad(_)))
        {
            v.push(button(cmd::FIND_TRUSSES, "Find Trusses", true));
        }
        return v;
    }
    let targets = selected::selected_targets(cx);
    if !targets.is_empty() {
        let any_free = targets.iter().any(|t| !retained(cx, t));
        v.push(button(
            cmd::BUILD_SELECTED,
            "Build Framing for Selected Object(s)",
            any_free,
        ));
    }
    let walls: Vec<Id> = cx
        .selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Wall(id) => Some(*id),
            _ => None,
        })
        .collect();
    if let [wall] = walls[..] {
        let has = super::load(cx.floor())
            .iter()
            .any(|m| m.wall_id == Some(wall));
        v.push(button(cmd::OPEN_WALL_DETAIL, "Open Wall Detail", has));
    }
    let linear = selected_linear(cx);
    if !selected_ids(cx).is_empty() {
        if !selected::parent_targets(cx).is_empty() {
            v.push(button(cmd::BUILD_PARENT, "Build Framing for Parent Object(s)", true));
        }
        let marker = super::reference_marker(&cx.project, cx.floor).is_some();
        if !linear.is_empty() {
            v.push(button(cmd::MOVE_TO_REF, "Move to Framing Ref", marker));
        }
    }
    let selected_trusses = linear
        .iter()
        .filter(|id| {
            find(cx.floor(), **id)
                .and_then(|r| r.member().map(|m| m.kind.is_truss()))
                .unwrap_or(false)
        })
        .count();
    if selected_trusses > 0 {
        v.push(button(cmd::OPEN_TRUSS_DETAIL, "Open Truss Detail", true));
    }
    let joinable: Vec<Id> = linear
        .iter()
        .copied()
        .filter(|id| {
            find(cx.floor(), *id)
                .and_then(|r| r.member().map(|m| m.is_linear()))
                .unwrap_or(false)
        })
        .collect();
    if joinable.len() == 2 && joinable.len() == selected_ids(cx).len() {
        v.push(button(cmd::JOIN_LAP, "Join and Lap Ends", true));
        v.push(button(cmd::JOIN_MITRE, "Join and Mitre Ends", true));
    }
    if joinable.len() == 1 && selected_ids(cx).len() == 1 {
        v.push(button(cmd::ADD_BREAK, "Add Break", true));
    }
    v
}

/// Rotates `m` onto its wide face and aligns it with the inside or outside of
/// the wall's framing layer (the Rotate option, manual p. 929): the depth and
/// thickness swap places and the member moves across the wall until it is
/// flush with that face of the layer.
pub fn flat_member(m: &mut Member, wall: &Wall, outside: bool) {
    let n = wall.normal();
    let side = if wall.exterior_side == plan_core::walls::Side::Left {
        1.0
    } else {
        -1.0
    };
    // Unit vector across the wall, pointing out, in the 3D frame.
    let out = [n.x * side, 0.0, -n.y * side];
    let extent = |m: &Member| {
        let t = &m.transform;
        let az = t.axis_z();
        let d = |a: [f64; 3]| (a[0] * out[0] + a[2] * out[2]).abs();
        d(t.axis_y) * m.lumber.depth + d(az) * m.lumber.thickness
    };
    let layer_depth = extent(m);
    // Lay it on its wide face: the depth runs where the thickness ran.
    let z = m.transform.axis_z();
    m.transform.axis_y = z;
    let flat = extent(m);
    let centre = |m: &Member| {
        (m.transform.origin[0] - wall.start.x) * out[0]
            + (m.transform.origin[2] + wall.start.y) * out[2]
    };
    let want = (if outside { 1.0 } else { -1.0 }) * (layer_depth / 2.0 - flat / 2.0);
    let shift = want - centre(m);
    m.transform.origin[0] += out[0] * shift;
    m.transform.origin[2] += out[2] * shift;
    m.label = format!("{} ({})", m.label, if outside { "flat to outside" } else { "flat to inside" });
}

/// Runs a framing Edit toolbar command. False when `id` is not one.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        cmd::BUILD_SELECTED | cmd::BUILD_PARENT => {
            let parent = id == cmd::BUILD_PARENT;
            let out = if parent {
                selected::build_parents(cx, None)
            } else {
                selected::build_selected(cx, None)
            };
            if let Outcome::AskGroup { room } = out {
                crate::dialogs::framing::ask_framing_group(room, parent);
            }
        }
        cmd::OPEN_WALL_DETAIL => {
            let wall = cx.selection.items.iter().find_map(|o| match o {
                ObjectRef::Wall(id) => Some(*id),
                _ => None,
            });
            if let Some(w) = wall {
                details::open_wall_detail(cx, w);
            }
        }
        cmd::FIND_WALL => {
            if let Some(w) = details::detail_wall(cx.floor()) {
                crate::editor::schedule_view::find_wall(cx, w);
            }
        }
        cmd::MOVE_TO_REF => {
            let ids = selected_linear(cx);
            let n = selected::move_to_reference(cx, &ids);
            if n > 0 {
                cx.status = format!("Moved {n} framing member(s) to the Framing Reference");
            }
        }
        cmd::JOIN_LAP | cmd::JOIN_MITRE => {
            let ids = selected_ids(cx);
            if let [a, b] = ids[..] {
                if !selected::join_ends(cx, a, b, id == cmd::JOIN_MITRE) {
                    cx.status = "Those members cannot be joined: they must not be parallel".into();
                }
            }
        }
        cmd::ADD_BREAK => {
            if let Some(&only) = selected_ids(cx).first() {
                if let Some(r) = find(cx.floor(), only).and_then(|r| r.member().cloned()) {
                    let mid = Point::lerp(r.start, r.end, 0.5);
                    selected::add_break(cx, only, mid);
                }
            }
        }
        cmd::OPEN_TRUSS_DETAIL => {
            let truss = selected_ids(cx).into_iter().find(|id| {
                find(cx.floor(), *id)
                    .and_then(|r| r.member().map(|m| m.kind.is_truss()))
                    .unwrap_or(false)
            });
            details::open_truss_detail(cx, truss);
        }
        cmd::FIND_TRUSSES => {
            details::find_trusses(cx);
        }
        cmd::TRUSS_WINDOW => crate::dialogs::truss_detail::open(),
        cmd::WALL_MEMBER_DELETE => {
            let pick = details::selected_wall_members(cx);
            let n = selected::edit_wall_members(cx, "Delete Framing Member", &pick, |_| false);
            cx.selection.clear();
            cx.status = format!("Deleted {n} framing member(s)");
        }
        cmd::FLAT_INSIDE | cmd::FLAT_OUTSIDE => {
            let outside = id == cmd::FLAT_OUTSIDE;
            let pick = details::selected_wall_members(cx);
            let Some(wall) = details::detail_wall(cx.floor()) else {
                return true;
            };
            let Some(w) = details::floor_of_wall(&cx.project, wall)
                .and_then(|fi| cx.project.floors[fi].walls.iter().find(|w| w.id == wall).cloned())
            else {
                return true;
            };
            let n = selected::edit_wall_members(
                cx,
                if outside { "Flat to Outside" } else { "Flat to Inside" },
                &pick,
                |m| {
                    flat_member(m, &w, outside);
                    true
                },
            );
            cx.status = format!("Turned {n} member(s) {}", FlatTo::ALL[if outside { 2 } else { 1 }].name());
        }
        _ => return false,
    }
    true
}
