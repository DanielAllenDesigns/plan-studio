//! plan-3d: turns a [`plan_core::Project`] into triangle meshes and glTF 2.0.
//!
//! No GPU code lives here: a [`Scene`] is plain vertex/index buffers that any
//! renderer can upload. Scene space is right-handed with **X = plan x**,
//! **Y = up** (inches, offset by the floor elevation) and **Z = -plan y**, so
//! the plan's Y-up reads as looking down -Z. UVs are in feet.

mod builder;
mod casing;
pub mod details;
mod doors;
pub mod foundation;
mod frame;
pub mod gltf;
pub mod images;
pub mod import;
mod leaf;
mod mesh;
mod opening;
mod railing;
mod roof;
mod slab;
pub mod triangulate;
mod wall;
pub mod wall_kinds;
mod windows;

pub use mesh::{Bounds, Material, Mesh, Scene, Vertex};
pub use railing::post_count as railing_post_count;
pub use roof::{
    ceiling_plane_meshes, dormer_meshes, roof_meshes, roof_plane_meshes, skylight_meshes,
    CEILING_FRAMING_MATERIAL,
};
pub use slab::slab_from_polygon;

use plan_core::foundation::PlatformKind;
use plan_core::geometry::point_in_polygon;
use plan_core::{detect_rooms, Floor, Project, Room, Wall, WallKind, WallTypeDef};
use wall::{InteriorSign, WallLook};

/// Snap tolerance handed to room detection, inches.
const ROOM_TOLERANCE: f64 = 0.5;

/// Half-wall height, 36".
const HALF_WALL_HEIGHT: f64 = 36.0;

/// How openings are displayed (resolved from [`SceneOptions`]).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct OpeningDisplay {
    /// Hinged/shower leaf and bifold fold angle; `0` is closed.
    pub open_angle_deg: f64,
}

/// Detail switches for [`build_scene_with`]. Build with
/// `SceneOptions { show_casing: true, ..Default::default() }`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneOptions {
    /// Draw doors open (hinged swing, sliding/pocket/barn slid, bifold folded).
    pub doors_open: bool,
    /// Add interior and exterior casing, window sills and exterior thresholds.
    pub show_casing: bool,
    /// Cut lite grids into doors and garage doors (windows always have lites).
    pub show_lites: bool,
    /// Angle used when `doors_open` is set, degrees.
    pub open_angle_deg: f64,
}

impl Default for SceneOptions {
    fn default() -> Self {
        Self {
            doors_open: false,
            show_casing: false,
            show_lites: true,
            open_angle_deg: 90.0,
        }
    }
}

impl SceneOptions {
    /// The resolved opening display (closed when `doors_open` is off).
    pub fn display(&self) -> OpeningDisplay {
        OpeningDisplay {
            open_angle_deg: if self.doors_open {
                self.open_angle_deg
            } else {
                0.0
            },
        }
    }
}

/// Build every mesh for the project: per floor, walls, openings, floors and ceilings.
pub fn build_scene(project: &Project) -> Scene {
    build_scene_with(project, &SceneOptions::default())
}

/// [`build_scene`] with explicit detail options.
pub fn build_scene_with(project: &Project, opts: &SceneOptions) -> Scene {
    build_scene_with_types(project, opts, &[])
}

/// [`build_scene_with`] that also resolves wall types in `defaults` (for
/// example `PlanDefaults::wall_types`) when the project does not define them.
pub fn build_scene_with_types(
    project: &Project,
    opts: &SceneOptions,
    defaults: &[WallTypeDef],
) -> Scene {
    let mut scene = Scene::default();
    let types = TypeLookup {
        project: &project.wall_types,
        defaults,
    };
    for floor in &project.floors {
        add_floor(floor, opts, &types, &mut scene);
    }
    scene
}

/// Wall-type registry: project types first, then defaults.
struct TypeLookup<'a> {
    project: &'a [WallTypeDef],
    defaults: &'a [WallTypeDef],
}

impl TypeLookup<'_> {
    /// Thickness and surface material of the named type.
    fn info(&self, name: &str) -> Option<wall_kinds::TypeInfo> {
        let ty = self
            .project
            .iter()
            .chain(self.defaults)
            .find(|t| t.name == name)?;
        Some(wall_kinds::TypeInfo {
            thickness: ty.thickness(),
            exterior: self.exterior_material(name),
        })
    }

    /// Surface material of the named type's exterior layer, if it maps.
    fn exterior_material(&self, name: &str) -> Option<Material> {
        let ty = self
            .project
            .iter()
            .chain(self.defaults)
            .find(|t| t.name == name)?;
        let layer = ty.layers.first()?;
        Material::from_layer_name(&layer.name)
            .or_else(|| Material::from_layer_name(&layer.material))
    }
}

/// The wall as drawn: half and pony walls are cut down, and the exterior look
/// comes from its wall type (a pony wall uses its lower type).
fn wall_as_drawn(wall: &Wall, types: &TypeLookup) -> (Wall, WallLook) {
    let mut drawn = wall.clone();
    let mut type_name = wall.wall_type.as_deref();
    if let Some(pony) = &wall.flags.pony {
        drawn.height = pony.lower_height.clamp(0.0, wall.height);
        type_name = Some(pony.lower_type.as_str());
    } else if wall.flags.half_wall {
        drawn.height = wall.height.min(HALF_WALL_HEIGHT);
    }
    let exterior = type_name
        .and_then(|n| types.exterior_material(n))
        .or_else(|| {
            wall.wall_type
                .as_deref()
                .and_then(|n| types.exterior_material(n))
        })
        .unwrap_or(Material::WallExterior);
    (drawn, WallLook { exterior })
}

fn add_floor(floor: &Floor, opts: &SceneOptions, types: &TypeLookup, scene: &mut Scene) {
    let rooms = detect_rooms(&floor.walls, ROOM_TOLERANCE);
    for wall in &floor.walls {
        add_wall(floor, wall, &rooms, opts, types, scene);
    }
    // Rooms with the same floor offset and ceiling height share a platform
    // mesh; a named room's overrides (R-23, R-24) split it from the rest.
    let mut groups: Vec<(slab::RoomLevels, Vec<Room>)> = Vec::new();
    for room in &rooms {
        let levels = slab::room_levels(floor, room);
        match groups.iter_mut().find(|(l, _)| *l == levels) {
            Some((_, rs)) => rs.push(room.clone()),
            None => groups.push((levels, vec![room.clone()])),
        }
    }
    let floor_holes = foundation::platform_holes(floor, PlatformKind::Floor);
    let ceiling_holes = foundation::platform_holes(floor, PlatformKind::Ceiling);
    for (levels, group) in &groups {
        let finished_floor = floor.elevation + levels.floor_offset + slab::FLOOR_FINISH;
        let ceiling = floor.elevation + levels.floor_offset + levels.ceiling_height;
        // Holes in the platform (stairwells, light wells) are cut out.
        let slabs = [
            (
                Material::Floor,
                &floor_holes,
                finished_floor - slab::SLAB_THICKNESS,
                finished_floor,
            ),
            (
                Material::Ceiling,
                &ceiling_holes,
                ceiling,
                ceiling + slab::SLAB_THICKNESS,
            ),
        ];
        for (material, holes, y0, y1) in slabs {
            scene.meshes.extend(foundation::build_platform(
                material, group, holes, y0, y1, None,
            ));
        }
    }
}

fn add_wall(
    floor: &Floor,
    wall: &Wall,
    rooms: &[Room],
    opts: &SceneOptions,
    types: &TypeLookup,
    scene: &mut Scene,
) {
    if wall.flags.invisible {
        return;
    }
    if wall_kinds::handles(wall) {
        let interior = interior_sign(wall, rooms);
        let info = |n: &str| types.info(n);
        wall_kinds::add_wall(floor, wall, interior, opts, &info, &mut scene.meshes);
        return;
    }
    if wall.flags.railing {
        scene
            .meshes
            .extend(railing::build_railing(wall, floor.elevation));
        return;
    }
    let (drawn, look) = wall_as_drawn(wall, types);
    let hosted: Vec<_> = floor
        .openings_on(wall.id)
        .filter_map(|o| wall::hole_for(&drawn, o).map(|h| (o, h)))
        .collect();
    let holes: Vec<_> = hosted.iter().map(|(_, h)| *h).collect();
    let interior = interior_sign(wall, rooms);
    scene.meshes.extend(wall::build_wall(
        &drawn,
        floor.elevation,
        &holes,
        interior,
        look,
    ));
    for (opening, hole) in &hosted {
        scene.meshes.extend(opening::build_opening(
            &drawn,
            opening,
            hole,
            floor.elevation,
            interior,
            opts,
        ));
    }
}

/// The side of the wall facing a detected room (left `1.0`, right `-1.0`).
///
/// Only matters for exterior walls; falls back to the right side when neither
/// side touches a room, which leaves the left face as the exterior.
fn interior_sign(wall: &Wall, rooms: &[Room]) -> InteriorSign {
    if wall.kind == WallKind::Interior {
        return 1.0;
    }
    let mid = wall.point_at(wall.length() * 0.5);
    let reach = wall.thickness * 0.5 + 1.0;
    let side_in_room = |sign: f64| {
        let probe = mid + wall.normal() * (sign * reach);
        rooms.iter().any(|r| point_in_polygon(probe, &r.polygon))
    };
    if side_in_room(1.0) {
        1.0
    } else {
        -1.0
    }
}
