//! Deck planking in 3D (CB-86): the decking boards of a deck room, the
//! picture frame border, and a skirt where the framing has not been built.
//!
//! A room is a deck when it has a Deck Specification
//! (`plan_core::deck::DeckSpec`). Its platform is not drawn as a slab then:
//! the boards sit on the joists, which Build Framing > Deck makes as framing
//! members (`plan_framing::deck`).

use crate::builder::MeshSet;
use crate::fireplace::{add_prism, material_named};
use crate::mesh::{Material, Mesh};
use crate::slab::room_levels;
use plan_core::deck::{joist_depth, ledger_edges, plank_layout};
use plan_core::{detect_rooms, Floor, Point, Project, Room};

/// Does `room` draw its own boards instead of the platform slab?
pub(crate) fn draws_boards(floor: &Floor, room: &Room) -> bool {
    room.name_entry(&floor.room_names)
        .and_then(|n| n.deck.as_ref())
        .is_some_and(|d| d.draws_boards())
}

/// The meshes of one deck: boards, border and (without framing) the skirt.
fn deck_room_meshes(floor: &Floor, room: &Room) -> Vec<Mesh> {
    let Some(name) = room.name_entry(&floor.room_names) else {
        return Vec::new();
    };
    let Some(spec) = name.deck.as_ref() else {
        return Vec::new();
    };
    let outline: &[Point] = if room.inner_polygon.len() >= 3 {
        &room.inner_polygon
    } else {
        &room.polygon
    };
    let levels = room_levels(floor, room);
    let top = floor.elevation + levels.floor_offset + levels.floor_finish;
    let thick = spec.board_thickness();
    let mut set = MeshSet::default();
    if spec.planking.enabled {
        let layout = plank_layout(outline, &spec.planking);
        let field = material_named(&spec.planking.material, Material::Floor);
        let border = if spec.planking.border_material.trim().is_empty() {
            field
        } else {
            material_named(&spec.planking.border_material, field)
        };
        for plank in &layout.planks {
            let m = if plank.border { border } else { field };
            add_prism(set.material(m), &plank.corners, top - thick, top);
        }
    }
    if spec.framing.enabled && !spec.framing.built {
        // The rim all round (not along the house): a board on edge under the
        // decking, so the deck reads as a deck before it is framed.
        let ledger = ledger_edges(outline, &floor.walls);
        let depth = joist_depth(&spec.framing);
        let n = outline.len();
        let area = plan_core::geometry::polygon_area(outline);
        for i in 0..n {
            if ledger.contains(&i) {
                continue;
            }
            let (a, b) = (outline[i], outline[(i + 1) % n]);
            if a.dist(b) < 1.0 {
                continue;
            }
            // Inward is to the left of a counter-clockwise edge.
            let dir = (b - a).normalized();
            let inward = if area >= 0.0 { dir.perp() } else { -dir.perp() };
            let strip = vec![a, b, b + inward * 1.5, a + inward * 1.5];
            add_prism(
                set.material(Material::Framing),
                &strip,
                top - thick - depth,
                top - thick,
            );
        }
    }
    set.finish(None)
}

/// The decking of every deck room of the project.
pub fn deck_meshes(project: &Project) -> Vec<Mesh> {
    let mut out = Vec::new();
    for floor in &project.floors {
        if floor.room_names.iter().all(|n| n.deck.is_none()) {
            continue;
        }
        let rooms = detect_rooms(&floor.walls, 0.5);
        for room in &rooms {
            out.extend(deck_room_meshes(floor, room));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::deck::{DeckPlanking, DeckSpec};
    use plan_core::{RoomName, Wall, WallClass, WallKind};

    /// A 12 x 8 ft deck: a house wall along the bottom (y = 0) and deck
    /// edge walls on the other three sides.
    fn deck_project(spec: Option<DeckSpec>) -> Project {
        let mut p = Project::new("deck");
        let f = &mut p.floors[0];
        let pts = [(0.0, 0.0), (144.0, 0.0), (144.0, 96.0), (0.0, 96.0)];
        for i in 0..4 {
            let (a, b) = (pts[i], pts[(i + 1) % 4]);
            let mut w = if i == 0 {
                Wall::new(
                    Point::new(a.0, a.1),
                    Point::new(b.0, b.1),
                    6.5,
                    109.0,
                    WallKind::Exterior,
                )
            } else {
                let mut w = Wall::new(
                    Point::new(a.0, a.1),
                    Point::new(b.0, b.1),
                    1.5,
                    36.0,
                    WallKind::Exterior,
                );
                w.class = WallClass::DeckEdge;
                w.is_deck_edge = true;
                w
            };
            w.id = i as u64 + 1;
            f.walls.push(w);
        }
        let mut name = RoomName::new(Point::new(72.0, 48.0), "Deck", "Deck");
        name.has_ceiling = false;
        name.deck = spec;
        f.room_names.push(name);
        p
    }

    #[test]
    fn a_deck_room_gets_boards_on_top_of_the_joists() {
        let p = deck_project(Some(DeckSpec::default()));
        let meshes = deck_meshes(&p);
        assert!(!meshes.is_empty());
        assert!(meshes.iter().any(|m| m.material == Material::Floor));
        // Boards end at the finished floor of the room.
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        let levels = room_levels(&p.floors[0], &rooms[0]);
        let expected = levels.floor_offset + levels.floor_finish;
        let (_, hi) = meshes
            .iter()
            .filter(|m| m.material == Material::Floor)
            .fold(([f32::MAX; 3], [f32::MIN; 3]), |(mut lo, mut hi), m| {
                let (a, b) = m.bounds().unwrap();
                for k in 0..3 {
                    lo[k] = lo[k].min(a[k]);
                    hi[k] = hi[k].max(b[k]);
                }
                (lo, hi)
            });
        assert!(
            (f64::from(hi[1]) - expected).abs() < 1e-4,
            "{hi:?} vs {expected}"
        );
    }

    #[test]
    fn a_room_without_a_deck_specification_builds_no_boards() {
        let p = deck_project(None);
        assert!(deck_meshes(&p).is_empty());
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        assert!(!draws_boards(&p.floors[0], &rooms[0]));
    }

    #[test]
    fn a_deck_replaces_the_platform_slab() {
        let p = deck_project(Some(DeckSpec::default()));
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        assert!(draws_boards(&p.floors[0], &rooms[0]));
        let mut off = DeckSpec::default();
        off.planking.enabled = false;
        let p = deck_project(Some(off));
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        assert!(!draws_boards(&p.floors[0], &rooms[0]));
    }

    #[test]
    fn the_skirt_stands_until_the_framing_is_built_and_skips_the_house_side() {
        let mut spec = DeckSpec::default();
        spec.planking.enabled = false;
        let p = deck_project(Some(spec.clone()));
        let skirt = deck_meshes(&p);
        let framing: Vec<&Mesh> = skirt
            .iter()
            .filter(|m| m.material == Material::Framing)
            .collect();
        assert_eq!(framing.len(), 1);
        // Three sides: the front and the two ends (the house side is the ledger).
        let tris = framing[0].triangle_count();
        assert_eq!(tris, 3 * 12);
        spec.framing.built = true;
        let p = deck_project(Some(spec));
        assert!(deck_meshes(&p).is_empty());
    }

    #[test]
    fn the_border_boards_can_be_another_material() {
        let spec = DeckSpec {
            planking: DeckPlanking {
                border: true,
                border_material: "Stone".into(),
                ..DeckPlanking::default()
            },
            ..DeckSpec::default()
        };
        let p = deck_project(Some(spec));
        let meshes = deck_meshes(&p);
        assert!(meshes.iter().any(|m| m.material == Material::Stone));
        assert!(meshes.iter().any(|m| m.material == Material::Floor));
    }
}
