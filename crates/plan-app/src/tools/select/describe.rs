//! Words for the object under the pointer or in the selection (S-6, S-98,
//! DW-65): the hover tooltip, the status bar's selection description and the
//! Z / height of the thing. Pure reads of the model, so they are easy to test.

use crate::editor::{EditorContext, ObjectRef};
use plan_core::cad::CadItem;
use plan_core::OpeningKind;

/// The name of a wall as Chief's status bar says it.
fn wall_noun(w: &plan_core::Wall) -> String {
    if !w.class.is_standard() {
        return w.class.label().to_string();
    }
    if w.is_curved() {
        "Curved Wall".into()
    } else {
        "Straight Wall".into()
    }
}

/// "Exterior Wall" / "Interior Wall" with the kind of the wall.
fn wall_kind_name(w: &plan_core::Wall) -> String {
    if !w.class.is_standard() {
        return w.class.label().to_string();
    }
    match w.kind {
        plan_core::WallKind::Exterior => "Exterior Wall".into(),
        plan_core::WallKind::Interior => "Interior Wall".into(),
    }
}

/// Selections larger than this are counted without naming their types.
const SUMMARY_NAMES_MAX: usize = 64;

/// The noun for the status bar's "1 Straight Wall selected".
pub fn noun(cx: &EditorContext, o: ObjectRef) -> String {
    let f = cx.floor();
    match o {
        ObjectRef::Wall(id) => f.wall(id).map_or_else(|| "Wall".into(), |w| wall_noun(w)),
        ObjectRef::Opening(id) => f
            .openings
            .iter()
            .find(|x| x.id == id)
            .map_or_else(|| "Opening".into(), |x| x.type_name().to_string()),
        ObjectRef::Cabinet(id) => crate::editor::placed::cabinet_by_id(f, id)
            .map_or_else(|| "Cabinet".into(), |c| c.kind.name().to_string()),
        ObjectRef::Stair(id) => crate::editor::stairs_view::find(f, id).map_or_else(
            || "Stairs".into(),
            |s| {
                if s.is_landing() {
                    "Landing".into()
                } else {
                    "Staircase".into()
                }
            },
        ),
        ObjectRef::Cad(id) | ObjectRef::Text(id) => f
            .cad
            .iter()
            .find(|c| c.id == id)
            .map_or_else(|| "CAD Object".into(), |c| cad_noun(&c.item).to_string()),
        other => other.type_name().to_string(),
    }
}

fn cad_noun(item: &CadItem) -> &'static str {
    match item {
        CadItem::Line { .. } => "Line",
        CadItem::Arc { .. } => "Arc",
        CadItem::Circle { .. } => "Circle",
        CadItem::Polyline { closed: true, .. } => "Closed Polyline",
        CadItem::Polyline { .. } => "Polyline",
        CadItem::Text { .. } => "Text",
    }
}

/// One line that names the object and its main size, as the hover tooltip
/// shows it: "Exterior Wall, 24'-0\" long", "Hinged Door 3068".
pub fn describe(cx: &EditorContext, o: ObjectRef) -> String {
    let f = cx.floor();
    let len = |v: f64| cx.fmt_dim(v);
    match o {
        ObjectRef::Wall(id) => match f.wall(id) {
            Some(w) => format!(
                "{}, {} long, {} thick",
                wall_kind_name(w),
                len(w.length()),
                len(w.thickness)
            ),
            None => "Wall".into(),
        },
        ObjectRef::Opening(id) => match f.openings.iter().find(|x| x.id == id) {
            Some(x) => format!("{} {}", x.type_name(), x.auto_label()),
            None => "Opening".into(),
        },
        ObjectRef::Dimension(id) => match f.dimensions.iter().find(|d| d.id == id) {
            Some(d) => format!("Dimension, {}", len(d.start.dist(d.end))),
            None => "Dimension".into(),
        },
        ObjectRef::Cad(id) | ObjectRef::Text(id) => match f.cad.iter().find(|c| c.id == id) {
            Some(c) => match &c.item {
                CadItem::Line { a, b } => format!("Line, {} long", len(a.dist(*b))),
                CadItem::Arc { radius, .. } => format!("Arc, radius {}", len(*radius)),
                CadItem::Circle { radius, .. } => format!("Circle, radius {}", len(*radius)),
                CadItem::Polyline { points, closed } => format!(
                    "{}, {} points",
                    if *closed {
                        "Closed Polyline"
                    } else {
                        "Polyline"
                    },
                    points.len()
                ),
                CadItem::Text { text, .. } => {
                    let short: String = text.chars().take(24).collect();
                    format!("Text \"{short}\"")
                }
            },
            None => "CAD Object".into(),
        },
        ObjectRef::Cabinet(id) => match crate::editor::placed::cabinet_by_id(f, id) {
            Some(c) => format!("{}, {} x {}", c.kind.name(), len(c.width), len(c.depth)),
            None => "Cabinet".into(),
        },
        ObjectRef::Symbol(id) => match f.symbol(id) {
            Some(s) => {
                let name = if s.label.trim().is_empty() {
                    s.catalog_id.as_str()
                } else {
                    s.label.as_str()
                };
                format!("Symbol, {name}")
            }
            None => "Symbol".into(),
        },
        ObjectRef::Room(i) => match cx.rooms.get(i) {
            Some(r) if !r.label.trim().is_empty() => format!("Room, {}", r.label),
            _ => "Room".into(),
        },
        other => noun(cx, other),
    }
}

/// The Z range of an object as a status-bar note ("Z 0\" to 9'-0\""), when it
/// has a height.
pub fn height_note(cx: &EditorContext, o: ObjectRef) -> Option<String> {
    let f = cx.floor();
    let base = f.elevation;
    let range = |lo: f64, hi: f64| {
        Some(format!(
            "Z {} to {}",
            cx.fmt_dim(base + lo),
            cx.fmt_dim(base + hi)
        ))
    };
    match o {
        ObjectRef::Wall(id) => {
            let w = f.wall(id)?;
            range(w.bottom_offset, w.bottom_offset + w.height)
        }
        ObjectRef::Opening(id) => {
            let x = f.openings.iter().find(|x| x.id == id)?;
            let (lo, hi) = match x.kind {
                OpeningKind::Door => (x.sill_height, x.sill_height + x.height),
                OpeningKind::Window => (x.sill_height, x.sill_height + x.height),
            };
            range(lo, hi)
        }
        ObjectRef::Cabinet(id) => {
            let c = crate::editor::placed::cabinet_by_id(f, id)?;
            range(c.elevation, c.elevation + c.height)
        }
        ObjectRef::Symbol(id) => {
            let s = f.symbol(id)?;
            range(s.elevation, s.elevation + s.height)
        }
        _ => None,
    }
}

/// The status bar's selection text: "1 Straight Wall selected, Z 0\" to
/// 9'-0\"", or "3 objects selected". `None` with nothing selected.
pub fn selection_summary(cx: &EditorContext) -> Option<String> {
    let items = &cx.selection.items;
    let first = *items.first()?;
    if items.len() == 1 {
        let mut s = format!("1 {} selected", noun(cx, first));
        if let Some(z) = height_note(cx, first) {
            s.push_str(", ");
            s.push_str(&z);
        }
        return Some(s);
    }
    // A big selection is only counted: naming every object each frame would
    // cost more than the line is worth.
    if items.len() > SUMMARY_NAMES_MAX {
        return Some(format!("{} objects selected", items.len()));
    }
    let n0 = noun(cx, first);
    if items.iter().all(|o| noun(cx, *o) == n0) {
        let plural = if n0.ends_with('s') {
            n0
        } else {
            format!("{n0}s")
        };
        Some(format!("{} {plural} selected", items.len()))
    } else {
        Some(format!("{} objects selected", items.len()))
    }
}

/// What the hover tooltip says about the object under the pointer, with its
/// Z range.
pub fn hover_text(cx: &EditorContext) -> Option<String> {
    let o = cx.hover?;
    // Rooms are only "under the pointer" when nothing else is.
    let mut s = describe(cx, o);
    if let Some(z) = height_note(cx, o) {
        s.push_str(", ");
        s.push_str(&z);
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::geometry::Point;
    use plan_core::WallKind;

    fn cx_with_wall() -> (EditorContext, plan_core::Id) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let id = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(288.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        (cx, id)
    }

    #[test]
    fn a_hovered_wall_is_named_with_its_length() {
        let (mut cx, id) = cx_with_wall();
        cx.hover = Some(ObjectRef::Wall(id));
        let t = hover_text(&cx).unwrap();
        assert!(t.starts_with("Exterior Wall, "), "{t}");
        assert!(t.contains("24'"), "{t}");
        assert!(t.contains("Z "), "{t}");
    }

    #[test]
    fn a_hovered_door_reads_style_and_size() {
        let (mut cx, wall) = cx_with_wall();
        let door = cx
            .project
            .add_opening(0, wall, 100.0, OpeningKind::Door)
            .unwrap();
        cx.hover = Some(ObjectRef::Opening(door));
        let t = hover_text(&cx).unwrap();
        assert!(t.starts_with("Hinged Door 3068"), "{t}");
    }

    #[test]
    fn the_selection_is_counted_and_typed() {
        let (mut cx, id) = cx_with_wall();
        assert_eq!(selection_summary(&cx), None);
        cx.selection.set(ObjectRef::Wall(id));
        let s = selection_summary(&cx).unwrap();
        assert!(s.starts_with("1 Straight Wall selected"), "{s}");
        let id2 = cx.project.add_wall(
            0,
            Point::new(0.0, 100.0),
            Point::new(288.0, 100.0),
            4.0,
            96.0,
            WallKind::Interior,
        );
        cx.selection.add(ObjectRef::Wall(id2));
        assert_eq!(
            selection_summary(&cx).as_deref(),
            Some("2 Straight Walls selected")
        );
        let c = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::ZERO,
                b: Point::new(10.0, 0.0),
            },
        );
        cx.selection.add(ObjectRef::Cad(c));
        assert_eq!(
            selection_summary(&cx).as_deref(),
            Some("3 objects selected")
        );
    }
}
