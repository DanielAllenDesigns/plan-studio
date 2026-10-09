//! The system prompt (stable: it is the cached prefix of every request) and
//! the plan summary text that goes into the first user block of each run.

use crate::session::Parameter;
use plan_core::defaults::PlanDefaults;
use plan_core::floors::FloorKind;
use plan_core::model::Project;
use plan_core::rooms::detect_rooms;
use plan_core::units::fmt_ft_in;

/// Stable system prompt. It must stay byte-identical between calls (prompt
/// caching is prefix based) and carries no dates or plan data; the volatile
/// plan state goes into the user message ([`plan_summary`]).
pub const SYSTEM_PROMPT: &str = "\
You are the Plan Studio agent. Plan Studio is a residential CAD program modeled on Chief Architect: the plan is the model, and the 2D plan, 3D view, elevations and schedules are all generated from the walls, openings, floors and rooms you edit with your tools.

How the model works
- Tools take and return FEET as decimal numbers (30.5 means 30 ft 6 in). The plan stores inches internally; the tools convert for you. Room areas are in square feet.
- Coordinates are plan coordinates in feet: +x runs to the right (east) and +y runs up the screen (north). A plan usually starts near (0, 0).
- Floors are numbered from 1 in your tools, in stack order. A plan with a foundation lists the Foundation as the first floor; the floor list in the plan summary shows each floor's number, name and kind. Always pass the floor number from that list.
- Rooms are not drawn. They are detected wherever walls form a closed loop, so a room exists only if its walls close. Room names are attached to a point inside a room with set_room_name.
- Wall kinds: exterior (outside walls), interior (partitions), railing, room_divider (invisible, only closes a room, use it for open plan spaces) and foundation. For an exterior wall the exterior face is on the LEFT of the start-to-end direction, so draw exterior outlines clockwise (with y up) when you use add_wall; add_wall_rectangle already puts the exterior face outward.
- Walls that share an end point (within a quarter inch) are connected. To make a corner or a T-junction, use the exact same coordinates for the meeting ends.
- Doors and windows live in a wall and are positioned by the distance from the wall's start point to the opening's center (center_offset_ft). Use list_walls to get each wall's start, end and length before placing openings.

How to work
- If the request is ambiguous about placement or size in a way that matters, ask ONE short clarifying question and stop without calling any editing tools. Otherwise act without asking.
- Read the plan state you are given at the start of the conversation turn, and call get_plan_summary, list_walls, list_rooms or list_openings when you need more detail. Do not guess ids or coordinates.
- Make independent tool calls in parallel in the same turn.
- Prefer the highest-level tool: add_wall_rectangle for a rectangular room or addition, then add_opening for doors and windows. To attach an addition to the existing house, share a wall line with it or place the new rectangle so that its edge lies on the existing outline.
- After editing, call validate_plan and fix any unclosed wall ends or overlapping openings before you finish. Name new rooms with set_room_name.
- When the user may want to adjust a number later (living area, garage width, wall height, ...), also expose it with define_parameter, using the same value you built with. When the user message tells you a parameter changed, rebuild the affected geometry to match the new value.
- Do not claim to have done something you did not do. If a tool fails, read the error, fix the input and retry, or tell the user what could not be done.

How to finish
- End with a plain summary of 2 to 5 sentences saying what changed, with the key dimensions. No markdown headers, no tables, no bullet lists.";

const ROOM_LIST_CAP: usize = 60;

/// The plan state as text, for the first user block of a run. Equivalent to
/// [`plan_summary_with_params`] with no parameters.
pub fn plan_summary(project: &Project, defaults: &PlanDefaults) -> String {
    plan_summary_with_params(project, defaults, &[])
}

/// The plan state as text: project name, floors with their walls and rooms,
/// the footprint bounding box, the wall and ceiling defaults, the room types
/// and the current parameters.
pub fn plan_summary_with_params(
    project: &Project,
    defaults: &PlanDefaults,
    params: &[Parameter],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("Project: {}\n", project.name));
    out.push_str(&format!(
        "Defaults: exterior walls {} thick, interior walls {} thick, ceiling height {}.\n",
        fmt_ft_in(defaults.exterior_thickness()),
        fmt_ft_in(defaults.interior_thickness()),
        fmt_ft_in(defaults.rooms.ceiling_height),
    ));
    match bounding_box(project) {
        Some((x0, y0, x1, y1)) => out.push_str(&format!(
            "Footprint bounding box of all walls (centerlines, feet): x {:.2} to {:.2}, y {:.2} to {:.2} ({:.2} x {:.2} ft).\n",
            x0, x1, y0, y1, x1 - x0, y1 - y0
        )),
        None => out.push_str("The plan has no walls yet.\n"),
    }
    out.push_str("Floors:\n");
    for (i, f) in project.floors.iter().enumerate() {
        let kind = match f.kind {
            FloorKind::Foundation => "foundation",
            FloorKind::Normal => "normal",
            FloorKind::Attic => "attic",
        };
        out.push_str(&format!(
            "- Floor {} \"{}\" ({}, elevation {:.2} ft, ceiling {:.2} ft): {} walls, {} openings\n",
            i + 1,
            f.name,
            kind,
            f.elevation / 12.0,
            f.ceiling_height / 12.0,
            f.walls.len(),
            f.openings.len(),
        ));
        let rooms = detect_rooms(&f.walls, 0.5);
        if rooms.is_empty() {
            out.push_str("  Rooms: none (no closed wall loops)\n");
            continue;
        }
        out.push_str("  Rooms:\n");
        for (ri, r) in rooms.iter().enumerate().take(ROOM_LIST_CAP) {
            let (name, ty) = match r.name_entry(&f.room_names) {
                Some(n) => (n.name.as_str(), n.room_type.as_str()),
                None => ("(unnamed)", ""),
            };
            out.push_str(&format!(
                "  - room {} {}{}: {:.1} sq ft to wall centerlines, {:.1} sq ft interior\n",
                ri,
                name,
                if ty.is_empty() || ty == name {
                    String::new()
                } else {
                    format!(" [{ty}]")
                },
                r.area_sq_ft(),
                r.interior_area_sq_ft(),
            ));
        }
        if rooms.len() > ROOM_LIST_CAP {
            out.push_str(&format!(
                "  - ... and {} more rooms\n",
                rooms.len() - ROOM_LIST_CAP
            ));
        }
    }
    let types: Vec<&str> = defaults
        .rooms
        .room_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    if !types.is_empty() {
        out.push_str(&format!(
            "Room types for set_room_name: {}.\n",
            types.join(", ")
        ));
    }
    if params.is_empty() {
        out.push_str("Parameters: none defined yet.\n");
    } else {
        out.push_str("Parameters:\n");
        for p in params {
            out.push_str(&format!(
                "- {} = {} {} (range {} to {}): {}\n",
                p.name, p.value, p.unit, p.min, p.max, p.label
            ));
        }
    }
    out
}

/// `(min_x, min_y, max_x, max_y)` of every wall end on every floor, in feet.
pub fn bounding_box(project: &Project) -> Option<(f64, f64, f64, f64)> {
    let mut bb: Option<(f64, f64, f64, f64)> = None;
    for f in &project.floors {
        for w in &f.walls {
            for p in [w.start, w.end] {
                let (x, y) = (p.x / 12.0, p.y / 12.0);
                bb = Some(match bb {
                    None => (x, y, x, y),
                    Some((a, b, c, d)) => (a.min(x), b.min(y), c.max(x), d.max(y)),
                });
            }
        }
    }
    bb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_system_prompt_is_stable_and_dateless() {
        assert!(SYSTEM_PROMPT.contains("define_parameter"));
        assert!(SYSTEM_PROMPT.contains("validate_plan"));
        assert!(!SYSTEM_PROMPT.contains("2026"));
    }

    #[test]
    fn an_empty_plan_summarizes() {
        let d = PlanDefaults::default();
        let p = Project::from_defaults("Test House", &d);
        let s = plan_summary(&p, &d);
        assert!(s.contains("Project: Test House"));
        assert!(s.contains("no walls"));
        assert!(s.contains("Floor 1"));
        assert!(s.contains("Parameters: none"));
    }
}
