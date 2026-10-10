//! The Materials List by surface: which library material each surface of the
//! plan shows (its paint, its class default, or the built-in material of its
//! kind) and how much of it there is, for a room, the active floor or the
//! whole plan.
//!
//! Surfaces counted: both faces of every wall (net of its openings), the
//! door and window of every opening, the floor and the ceiling of every room,
//! and the true sloped area of every roof plane. A wall shared by two rooms
//! counts its whole length for each room. The areas are added up per material
//! by `plan_materials::summarize`, which prices them with the Materials List
//! tab of each material's specification.

use super::paint::{current_material, room_objects, RoomSurface, Target};
use super::{library_with_blends, object_id_of};
use crate::editor::selection::ObjectRef;
use crate::editor::EditorContext;
use plan_core::{OpeningKind, Room, WallKind};
use plan_materials::{summarize, MaterialQuantity, Region, SurfaceArea};

fn roof_area_sq_in(v: &serde_json::Value) -> f64 {
    let Some(poly) = v
        .get("polygon3d")
        .and_then(|p| serde_json::from_value::<Vec<[f64; 3]>>(p.clone()).ok())
    else {
        return 0.0;
    };
    if poly.len() < 3 {
        return 0.0;
    }
    // Newell vector: half its length is the true area.
    let mut s = [0.0; 3];
    for i in 0..poly.len() {
        let (c, d) = (poly[i], poly[(i + 1) % poly.len()]);
        s[0] += (c[1] - d[1]) * (c[2] + d[2]);
        s[1] += (c[2] - d[2]) * (c[0] + d[0]);
        s[2] += (c[0] - d[0]) * (c[1] + d[1]);
    }
    (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt() * 0.5
}

/// Every surface of `region` with the material it shows. Empty when a room
/// index is out of range.
pub fn surface_areas(cx: &mut EditorContext, region: Region) -> Vec<SurfaceArea> {
    let floors: Vec<usize> = match region {
        Region::Plan => (0..cx.project.floors.len()).collect(),
        Region::Floor | Region::Room(_) => vec![cx.floor],
    };
    let mut out = Vec::new();
    let mut cache = Vec::new();
    for fl in floors {
        let rooms: Vec<Room> = if fl == cx.floor {
            cx.rooms_now().to_vec()
        } else {
            plan_core::detect_rooms(&cx.project.floors[fl].walls, 0.5)
        };
        // The objects and rooms the region reaches on this floor.
        let (only_objects, room_ids): (Option<Vec<ObjectRef>>, Vec<usize>) = match region {
            Region::Room(i) => match rooms.get(i) {
                Some(r) => (Some(room_objects(&cx.project, fl, r)), vec![i]),
                None => return out,
            },
            _ => (None, (0..rooms.len()).collect()),
        };
        let included = |o: ObjectRef| only_objects.as_ref().is_none_or(|v| v.contains(&o));
        let floor = cx.project.floors[fl].clone();
        let mut add = |cx: &mut EditorContext, target: Target, area: f64, what: &str| {
            if area <= 0.0 {
                return;
            }
            if let Some(material) = current_material(cx, &target, &mut cache) {
                out.push(SurfaceArea {
                    material,
                    area_sq_in: area,
                    what: what.to_string(),
                });
            }
        };
        for w in &floor.walls {
            if !included(ObjectRef::Wall(w.id)) {
                continue;
            }
            let cut: f64 = floor
                .openings
                .iter()
                .filter(|o| o.wall_id == w.id)
                .map(|o| o.width * o.height)
                .sum();
            let face = (w.path_length() * w.height - cut).max(0.0);
            let part = |p: &str| Target::Part {
                floor: fl,
                obj: ObjectRef::Wall(w.id),
                part: p.to_string(),
            };
            if w.kind == WallKind::Interior {
                add(
                    cx,
                    part("Interior Wall Surface"),
                    face * 2.0,
                    "Interior wall",
                );
            } else {
                add(cx, part("Exterior Wall Surface"), face, "Exterior wall");
                add(cx, part("Interior Wall Surface"), face, "Interior wall");
            }
        }
        for o in &floor.openings {
            if !included(ObjectRef::Opening(o.id)) {
                continue;
            }
            let (part, what) = match o.kind {
                OpeningKind::Door => ("Door Panel", "Door"),
                OpeningKind::Window => ("Glass", "Window"),
            };
            add(
                cx,
                Target::Part {
                    floor: fl,
                    obj: ObjectRef::Opening(o.id),
                    part: part.to_string(),
                },
                o.width * o.height,
                what,
            );
        }
        for &i in &room_ids {
            let area = rooms[i].interior_area_sq_in;
            for (surface, what) in [
                (RoomSurface::Floor, "Floor"),
                (RoomSurface::Ceiling, "Ceiling"),
            ] {
                add(
                    cx,
                    Target::Room {
                        floor: fl,
                        room: i,
                        surface,
                    },
                    area,
                    what,
                );
            }
        }
        if only_objects.is_none() {
            for v in &floor.roofs {
                if v.get("kind").and_then(|k| k.as_str()) != Some("plane") {
                    continue;
                }
                let Some(id) = v.get("id").and_then(|i| i.as_u64()) else {
                    continue;
                };
                let obj = ObjectRef::RoofPlane(id);
                if object_id_of(obj).is_none() {
                    continue;
                }
                add(
                    cx,
                    Target::Part {
                        floor: fl,
                        obj,
                        part: "Roof Surface".to_string(),
                    },
                    roof_area_sq_in(v),
                    "Roof",
                );
            }
        }
    }
    out
}

/// The by-surface Materials List of `region`: one line per material, largest
/// area first, priced from the specifications.
pub fn lines(cx: &mut EditorContext, region: Region) -> Vec<MaterialQuantity> {
    let areas = surface_areas(cx, region);
    summarize(&library_with_blends(&cx.project), &areas)
}

/// The names of the active floor's rooms, for the region picker.
pub fn room_names(cx: &mut EditorContext) -> Vec<String> {
    let rooms = cx.rooms_now().to_vec();
    rooms
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let name = cx.room_name(r);
            if name.is_empty() {
                format!("Room {}", i + 1)
            } else {
                name
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::object_materials::WHOLE_OBJECT;
    use plan_core::Point;

    fn house() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let c = [
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            Point::new(120.0, 120.0),
            Point::new(0.0, 120.0),
        ];
        let mut walls = Vec::new();
        for i in 0..4 {
            walls.push(
                cx.project
                    .add_wall(0, c[i], c[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior),
            );
        }
        cx.project
            .add_opening(0, walls[0], 60.0, OpeningKind::Window)
            .expect("window");
        cx.refresh();
        cx
    }

    fn area_of(lines: &[MaterialQuantity], name: &str) -> f64 {
        lines
            .iter()
            .find(|l| l.name == name)
            .map_or(0.0, |l| l.area_sq_ft)
    }

    #[test]
    fn the_plan_list_counts_walls_net_of_openings_and_the_room_surfaces() {
        let mut cx = house();
        let win = cx.project.floors[0].openings[0].clone();
        let net = (4.0 * 120.0 * 96.0 - win.width * win.height) / 144.0;
        let l = lines(&mut cx, Region::Floor);
        // Built-in materials: exterior sand finish, interior drywall. A
        // room has none of its own until the plan's Room defaults name one.
        assert!((area_of(&l, "Sand Finish – Eggshell") - net).abs() < 1e-6);
        assert!((area_of(&l, "Drywall") - net).abs() < 1e-6);
        let interior = cx.rooms_now()[0].interior_area_sq_in / 144.0;
        assert_eq!(area_of(&l, "Oak Flooring"), 0.0);
        cx.project
            .set_class_material("Room", "Floor Finish", "Oak Flooring");
        cx.project
            .set_class_material("Room", "Ceiling Finish", "Color – White");
        let l = lines(&mut cx, Region::Floor);
        assert!((area_of(&l, "Oak Flooring") - interior).abs() < 1e-6);
        assert!((area_of(&l, "Color – White") - interior).abs() < 1e-6);
        assert!(l.iter().all(|q| q.known));
        // Painting one wall moves its exterior area to the new material.
        let wall = cx.project.floors[0].walls[2].id;
        cx.project
            .set_object_material(wall, WHOLE_OBJECT, "Brick – Red");
        let l = lines(&mut cx, Region::Floor);
        let one_face = 120.0 * 96.0 / 144.0;
        assert!((area_of(&l, "Brick – Red") - 2.0 * one_face).abs() < 1e-6);
        assert!((area_of(&l, "Drywall") - (net - one_face)).abs() < 1e-6);
    }

    #[test]
    fn a_room_list_is_narrower_than_the_floor_and_plan_covers_every_floor() {
        let mut cx = house();
        // A second floor with a lone wall of its own.
        cx.project
            .floors
            .push(plan_core::Floor::new("2nd Floor", 108.0));
        cx.project.add_wall(
            1,
            Point::new(0.0, 0.0),
            Point::new(60.0, 0.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        cx.project
            .set_class_material("Room", "Floor Finish", "Oak Flooring");
        let room = lines(&mut cx, Region::Room(0));
        assert!(area_of(&room, "Oak Flooring") > 0.0);
        let floor_total: f64 = lines(&mut cx, Region::Floor)
            .iter()
            .map(|l| l.area_sq_ft)
            .sum();
        let plan_total: f64 = lines(&mut cx, Region::Plan)
            .iter()
            .map(|l| l.area_sq_ft)
            .sum();
        let room_total: f64 = room.iter().map(|l| l.area_sq_ft).sum();
        assert!(room_total <= floor_total + 1e-9);
        assert!(plan_total > floor_total, "the other floor's wall counts");
        assert!(lines(&mut cx, Region::Room(9)).is_empty());
        assert_eq!(room_names(&mut cx).len(), 1);
    }

    #[test]
    fn a_priced_material_costs_by_its_unit() {
        let mut cx = house();
        let mut tile = plan_materials::MaterialDef::new("Slate Tile", &["Flooring"], [60, 60, 70]);
        tile.price = 54.0;
        tile.unit = plan_materials::PriceUnit::SqYd;
        super::super::set_user_library_for_test({
            let mut u = plan_materials::MaterialLibrary::default();
            u.add(tile);
            u
        });
        // The room itself is painted.
        let anchor = crate::editor::rooms_edit::room_anchor(&cx.rooms_now()[0]);
        cx.project.floors[0].room_names.push({
            let mut n = plan_core::RoomName::new(anchor, "Den", "");
            n.floor_finish = Some("Slate Tile".into());
            n
        });
        let l = lines(&mut cx, Region::Room(0));
        let line = l
            .iter()
            .find(|q| q.name == "Slate Tile")
            .expect("tiled floor");
        let sq_yd = line.area_sq_ft / 9.0;
        assert!((line.quantity - sq_yd).abs() < 1e-9);
        assert!((line.cost - sq_yd * 54.0).abs() < 1e-6);
        super::super::set_user_library_for_test(plan_materials::MaterialLibrary::default());
    }
}
