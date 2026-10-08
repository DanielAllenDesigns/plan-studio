//! plan-3d: turns a [`plan_core::Project`] into triangle meshes and glTF 2.0.
//!
//! No GPU code lives here: a [`Scene`] is plain vertex/index buffers that any
//! renderer can upload. Scene space is right-handed with **X = plan x**,
//! **Y = up** (inches, offset by the floor elevation) and **Z = -plan y**, so
//! the plan's Y-up reads as looking down -Z. UVs are in feet.

mod builder;
mod casing;
mod clip;
mod cover;
pub mod details;
mod doors;
mod eave;
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

pub use clip::{Piece, TopProfile};
pub use cover::{
    read_floor_roof, AtticPanel, BottomCut, Butt, FloorCover, FloorRoofInput, RoofCover,
    RoofDetail, RoofTypes, SplitKind, Surface,
};
pub use eave::{
    eave_detail_meshes, eave_elements, rafter_count, EaveElement, EaveKind, EaveOverrides,
    EavePlane,
};
pub use mesh::{Bounds, Material, Mesh, Scene, Vertex};
pub use railing::post_count as railing_post_count;
pub use roof::{
    ceiling_plane_meshes, ceiling_plane_meshes_joined, dormer_eave_meshes, dormer_meshes,
    gable_face_meshes, roof_meshes, roof_plane_meshes, skylight_meshes, CEILING_FRAMING_MATERIAL,
};
pub use slab::{room_ceiling_top, slab_from_polygon};

use plan_core::foundation::PlatformKind;
use plan_core::geometry::point_in_polygon;
use plan_core::{detect_rooms, Floor, Project, Room, Wall, WallKind, WallTypeDef};
use wall::{InteriorSign, Shape, Split, WallLook};

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
    /// Walls follow the roof: gable ends rise to the ridge and walls under a
    /// roof plane are cut to it (RF-16). The roof is read from the project's
    /// roof records; pass your own with [`build_scene_covered`].
    pub roof_cuts_walls: bool,
}

impl Default for SceneOptions {
    fn default() -> Self {
        Self {
            doors_open: false,
            show_casing: false,
            show_lites: true,
            open_angle_deg: 90.0,
            roof_cuts_walls: true,
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
    let cover = if opts.roof_cuts_walls {
        RoofCover::from_project(project)
    } else {
        RoofCover::none()
    };
    build_scene_covered(project, opts, defaults, &cover)
}

/// [`build_scene_with_types`] with the roof given: each wall's top follows
/// the roof and ceiling planes of `cover` above its floor, and the attic
/// walls `cover` works out are added.
pub fn build_scene_covered(
    project: &Project,
    opts: &SceneOptions,
    defaults: &[WallTypeDef],
    cover: &RoofCover,
) -> Scene {
    let mut scene = Scene::default();
    for (i, floor) in project.floors.iter().enumerate() {
        let types = TypeLookup {
            project: &project.wall_types,
            defaults,
            cover: cover.floor(i),
            roof: Some(cover),
            floor: i,
        };
        let open_above = open_below_holes(project.floors.get(i + 1));
        add_floor(floor, &open_above, opts, &types, &mut scene);
    }
    let any = TypeLookup {
        project: &project.wall_types,
        defaults,
        cover: None,
        roof: None,
        floor: 0,
    };
    scene
        .meshes
        .extend(cover.attic_meshes_typed(&|n| any.exterior_material(n)));
    scene
}

/// Does the outline `hole` cover the whole of `room` (same extent and area
/// within a hair)? Such a room is open to the floor above, not cut by it.
fn covers(hole: &[plan_core::Point], room: &[plan_core::Point]) -> bool {
    let (hl, hh) = plan_core::foundation::bounds(hole);
    let (rl, rh) = plan_core::foundation::bounds(room);
    let near = |a: f64, b: f64| (a - b).abs() <= 1.0;
    let (ha, ra) = (
        plan_core::geometry::polygon_area(hole).abs(),
        plan_core::geometry::polygon_area(room).abs(),
    );
    near(hl.x, rl.x) && near(hl.y, rl.y) && near(hh.x, rh.x) && near(hh.y, rh.y) && ha >= ra * 0.98
}

/// The rooms of `above` that have no floor platform (Open Below, R-40): the
/// ceiling of the floor under them is open to them, so each outline is a hole
/// in that ceiling.
fn open_below_holes(above: Option<&Floor>) -> Vec<Vec<plan_core::Point>> {
    let Some(above) = above else {
        return Vec::new();
    };
    if above.room_names.iter().all(|n| n.has_floor) {
        return Vec::new();
    }
    detect_rooms(&above.walls, ROOM_TOLERANCE)
        .into_iter()
        .filter(|r| !slab::room_levels(above, r).has_floor)
        .map(|r| r.polygon)
        .collect()
}

/// Wall-type registry: project types first, then defaults.
struct TypeLookup<'a> {
    project: &'a [WallTypeDef],
    defaults: &'a [WallTypeDef],
    /// The roof and ceiling planes above the floor being built.
    cover: Option<&'a FloorCover>,
    /// The roof of every floor (walls stand on the roofs below them and
    /// butt the roofs beside them).
    roof: Option<&'a RoofCover>,
    /// Index of the floor being built.
    floor: usize,
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

    /// The material split of a wall along its roof line: below a butting
    /// roof the lower wall type, above the plate the attic wall type (RF-28,
    /// RF-31). `base` is the wall's scene bottom, `top` its top profile and
    /// `drop` how far a cut bottom reaches below it.
    fn split_for(
        &self,
        wall: &Wall,
        base: f64,
        top: Option<&clip::TopProfile>,
        drop: f64,
    ) -> Option<Split> {
        let roof = self.roof?;
        let (profile, kind) = roof.wall_split(wall, base, top)?;
        let names = roof.types();
        let (below, above) = match kind {
            SplitKind::Butt => (self.named_material(&names.lower), None),
            SplitKind::Plate => (None, self.named_material(&names.attic)),
        };
        (below.is_some() || above.is_some()).then(|| Split {
            profile: profile.raised(drop),
            below,
            above,
        })
    }

    /// Thickness of the foundation wall type (the first type named
    /// "Foundation...") that the stem walls use.
    fn foundation_thickness(&self) -> Option<f64> {
        self.project
            .iter()
            .chain(self.defaults)
            .find(|t| t.name.starts_with("Foundation"))
            .map(|t| t.thickness())
    }

    fn named_material(&self, name: &str) -> Option<Material> {
        (!name.is_empty()).then(|| self.exterior_material(name))?
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

fn add_floor(
    floor: &Floor,
    open_above: &[Vec<plan_core::Point>],
    opts: &SceneOptions,
    types: &TypeLookup,
    scene: &mut Scene,
) {
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
    // A dropped garage (or a room with a Stem Wall) gets stem walls up to the
    // floor datum under its exterior walls.
    let stem = types.foundation_thickness();
    for room in &rooms {
        let levels = slab::room_levels(floor, room);
        scene
            .meshes
            .extend(slab::stem_walls(floor, room, &levels, stem));
    }
    let floor_holes = foundation::platform_holes(floor, PlatformKind::Floor);
    let mut ceiling_holes = foundation::platform_holes(floor, PlatformKind::Ceiling);
    ceiling_holes.extend(open_above.iter().cloned());
    for (levels, group) in &groups {
        let finished_floor = floor.elevation + levels.floor_offset + levels.floor_finish;
        let ceiling = floor.elevation + levels.floor_offset + levels.ceiling_height;
        // A room open to the floor above (Open Below) has no ceiling at all.
        let under_open: Vec<&Room> = group
            .iter()
            .filter(|r| !open_above.iter().any(|h| covers(h, &r.polygon)))
            .collect();
        let all: Vec<&Room> = group.iter().collect();
        let slabs = [
            (
                levels.has_floor,
                Material::Floor,
                all,
                &floor_holes,
                finished_floor - levels.floor_thickness,
                finished_floor,
            ),
            (
                levels.has_ceiling,
                Material::Ceiling,
                under_open,
                &ceiling_holes,
                ceiling,
                ceiling + levels.ceiling_thickness,
            ),
        ];
        for (wanted, material, rooms, holes, y0, y1) in slabs {
            if !wanted {
                continue;
            }
            // Holes in the platform (stairwells, light wells) are cut out,
            // and the ground under every nested room (R-11) from the room
            // around it; rooms with a nested room are built on their own.
            let (holed, plain): (Vec<&Room>, Vec<&Room>) =
                rooms.into_iter().partition(|r| !r.holes.is_empty());
            let plain: Vec<Room> = plain.into_iter().cloned().collect();
            scene.meshes.extend(foundation::build_platform(
                material, &plain, holes, y0, y1, None,
            ));
            for room in holed {
                let mut cut = holes.clone();
                cut.extend(room.holes.iter().cloned());
                scene.meshes.extend(foundation::build_platform(
                    material,
                    std::slice::from_ref(room),
                    &cut,
                    y0,
                    y1,
                    None,
                ));
            }
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
        wall_kinds::add_wall(
            floor,
            wall,
            interior,
            opts,
            &info,
            types.cover,
            &mut scene.meshes,
        );
        return;
    }
    if wall.flags.railing {
        scene.meshes.extend(railing::build_railing(
            wall,
            floor.elevation + wall.bottom_offset,
        ));
        return;
    }
    let (mut drawn, look) = wall_as_drawn(wall, types);
    let base = floor.elevation + wall.bottom_offset;
    // The roof shapes the top of an ordinary wall (RF-16, RF-20, RF-23); a
    // half or pony wall is cut by it but never raised to it.
    let top = types.cover.and_then(|c| {
        if drawn.height == wall.height {
            c.wall_top(wall, base, wall.height)
        } else {
            c.wall_top_clipped(wall, base, drawn.height)
        }
    });
    if let Some(t) = &top {
        // A gable rises above the plate; openings in it need the taller wall.
        drawn.height = drawn.height.max(t.max_height());
    }
    let hosted: Vec<_> = floor
        .openings_on(wall.id)
        .filter_map(|o| wall::hole_for(&drawn, o).map(|h| (o, h)))
        .collect();
    let holes: Vec<_> = hosted.iter().map(|(_, h)| *h).collect();
    let interior = interior_sign(wall, rooms);
    // A wall standing on a roof below it is cut along the roof (Roof Cuts
    // Wall at Bottom): it reaches down to the roof or is cut up to it.
    let cut = types
        .roof
        .and_then(|r| r.wall_bottom(types.floor, wall, base, drawn.height));
    let drop = cut.as_ref().map_or(0.0, |c| c.drop);
    let mut body = drawn.clone();
    body.bottom_offset -= drop;
    body.height += drop;
    let raised_top = top.as_ref().map(|t| t.raised(drop));
    let split = types.split_for(wall, base, top.as_ref(), drop);
    let shape = Shape {
        top: raised_top.as_ref(),
        bottom: cut.as_ref().map(|c| &c.profile),
        split: split.as_ref(),
    };
    let cuts = curved_neighbour_cuts(floor, wall);
    scene.meshes.extend(wall::build_wall_shaped_cut(
        &body,
        floor.elevation,
        &holes,
        interior,
        look,
        &shape,
        &cuts,
    ));
    for (opening, hole) in &hosted {
        // The wall's other openings, so a mulled unit shares one frame post
        // and one casing (DW-51, DW-52).
        scene.meshes.extend(opening::build_opening_in_wall(
            &drawn,
            opening,
            hole,
            floor.elevation,
            interior,
            opts,
            &hosted,
        ));
    }
}

/// The end cuts of a straight `wall` at the ends where a curved wall joins
/// it, taken from the joined outline (`plan_core::joins::wall_outlines`) so
/// the two meet in a mitre. Square everywhere else, so the corners of straight
/// walls are built as before.
fn curved_neighbour_cuts(floor: &Floor, wall: &Wall) -> wall::EndCuts {
    const JOIN_TOL: f64 = 0.5;
    if wall.is_curved() || !floor.walls.iter().any(Wall::is_curved) {
        return wall::EndCuts::NONE;
    }
    let curved_at = |p: plan_core::Point| {
        floor.walls.iter().any(|o| {
            o.id != wall.id
                && o.is_curved()
                && (o.start.dist(p) <= JOIN_TOL || o.end.dist(p) <= JOIN_TOL)
        })
    };
    let (at_start, at_end) = (curved_at(wall.start), curved_at(wall.end));
    if !at_start && !at_end {
        return wall::EndCuts::NONE;
    }
    let outlines = plan_core::joins::wall_outlines(&floor.walls, JOIN_TOL);
    let Some(outline) = outlines.iter().find(|o| o.wall_id == wall.id) else {
        return wall::EndCuts::NONE;
    };
    let joined = wall::EndCuts::from_outline(&outline.polygon);
    wall::EndCuts {
        start: joined.start.filter(|_| at_start),
        end: joined.end.filter(|_| at_end),
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
