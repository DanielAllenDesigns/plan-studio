//! Floor and ceiling slabs triangulated from detected room polygons.

use crate::builder::MeshBuilder;
use crate::mesh::{Material, Mesh};
use crate::triangulate::ear_clip;
use plan_core::Point;

/// Slab thickness, inches.
pub const SLAB_THICKNESS: f64 = 1.0;
/// Finish thickness added above the floor elevation, inches.
pub const FLOOR_FINISH: f64 = 0.75;

const IN_PER_FT: f64 = 12.0;

type V3d = [f64; 3];

fn sub3(a: V3d, b: V3d) -> V3d {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross3(a: V3d, b: V3d) -> V3d {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit3(a: V3d) -> Option<V3d> {
    let len = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    (len > 1e-9).then(|| [a[0] / len, a[1] / len, a[2] / len])
}

fn f32v(a: V3d) -> [f32; 3] {
    a.map(|c| c as f32)
}

/// A slab of `thickness` inches below a planar polygon in scene space
/// (`x`, `y` up, `z`; inches), for roof planes and ceilings.
///
/// `polygon3d` is the upper surface, in either winding; the slab extends
/// against the plane normal that points up (or the Newell normal when the
/// plane is vertical). Returns an empty mesh for degenerate input.
pub fn slab_from_polygon(polygon3d: &[[f64; 3]], thickness: f64, material: Material) -> Mesh {
    let mut mesh = MeshBuilder::new(material);
    let n = polygon3d.len();
    // Newell normal: CCW orientation when viewed from its tip.
    let mut nn = [0.0; 3];
    for i in 0..n {
        let (a, b) = (polygon3d[i], polygon3d[(i + 1) % n]);
        nn[0] += (a[1] - b[1]) * (a[2] + b[2]);
        nn[1] += (a[2] - b[2]) * (a[0] + b[0]);
        nn[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let Some(nn) = (n >= 3).then(|| unit3(nn)).flatten() else {
        return mesh.finish(None);
    };
    let up = if nn[1] < -1e-9 { nn.map(|c| -c) } else { nn };
    let drop = thickness.max(0.0);
    let bottom = |p: V3d| {
        [
            p[0] - up[0] * drop,
            p[1] - up[1] * drop,
            p[2] - up[2] * drop,
        ]
    };

    // Triangulate in the plane projected along its dominant axis.
    let axis = (0..3)
        .max_by(|&a, &b| nn[a].abs().total_cmp(&nn[b].abs()))
        .unwrap_or(1);
    let (ia, ib) = match axis {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    };
    let flat: Vec<Point> = polygon3d.iter().map(|p| Point::new(p[ia], p[ib])).collect();
    let uv = |p: V3d| [(p[ia] / IN_PER_FT) as f32, (p[ib] / IN_PER_FT) as f32];
    for [a, b, c] in ear_clip(&flat) {
        let tri = [polygon3d[a], polygon3d[b], polygon3d[c]];
        let uvs = tri.map(uv);
        mesh.tri(tri.map(f32v), uvs, f32v(up));
        if drop > 0.0 {
            mesh.tri(tri.map(|p| f32v(bottom(p))), uvs, f32v(up.map(|c| -c)));
        }
    }
    if drop > 0.0 {
        for i in 0..n {
            let (a, b) = (polygon3d[i], polygon3d[(i + 1) % n]);
            let Some(out) = unit3(cross3(sub3(b, a), nn)) else {
                continue;
            };
            let len =
                ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
            let u = (len / IN_PER_FT) as f32;
            let quad = [a, b, bottom(b), bottom(a)].map(f32v);
            mesh.quad(
                quad,
                [[0.0, 0.0], [u, 0.0], [u, 1.0], [0.0, 1.0]],
                f32v(out),
            );
        }
    }
    mesh.finish(None)
}

/// Per-room vertical settings that reach the 3D platforms (R-23..R-30, R-33):
/// the room name entry whose anchor lies in the room supplies a floor height
/// offset, a ceiling height override, the Floor and Ceiling Structure, the
/// finish thickness and the Floor/Ceiling Under/Over This Room switches;
/// rooms without a named entry keep the floor's defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RoomLevels {
    /// Raise of the room's floor above the floor datum, inches.
    pub floor_offset: f64,
    /// Ceiling height measured from the room's own floor, inches.
    pub ceiling_height: f64,
    /// Floor finish above the platform, inches.
    pub floor_finish: f64,
    /// Thickness of the floor platform, inches.
    pub floor_thickness: f64,
    /// Thickness of the ceiling platform, inches.
    pub ceiling_thickness: f64,
    /// Build the floor platform under the room (R-30, Open Below).
    pub has_floor: bool,
    /// Build the ceiling platform over the room (R-30, Deck and Porch).
    pub has_ceiling: bool,
}

/// The levels of `room` on `floor`.
///
/// * The ceiling platform hangs above the finished ceiling: the slab runs
///   from the finished ceiling up through the ceiling finish layers (R-27) -
///   or up to the Rough Ceiling height when the room has one (R-25) - and
///   the platform itself.
/// * On a foundation floor built with a basement the ceiling is the
///   underside of the first floor's platform (the floor's ceiling structure
///   thickness), and the floor's `ceiling_height` reaches the first floor's
///   finished level (R-18).
/// * A room with the Monolithic Slab Foundation flag has a floor platform of
///   the slab's thickness (R-31).
pub(crate) fn room_levels(floor: &plan_core::Floor, room: &plan_core::Room) -> RoomLevels {
    let named = room.name_entry(&floor.room_names);
    let misc = named.and_then(|n| n.misc.as_ref());
    let layered = |layers: Option<&Vec<plan_core::extras::StructureLayer>>| {
        layers
            .filter(|l| !l.is_empty())
            .map(|l| plan_core::extras::structure_thickness(l))
    };
    let floor_offset = named.map_or(0.0, |n| n.floor_height_offset);
    let platform_above = if floor.kind == plan_core::FloorKind::Foundation {
        floor.settings.ceiling_structure_thickness.max(0.0)
    } else {
        0.0
    };
    let ceiling_height = named.and_then(|n| n.ceiling_height).unwrap_or_else(|| {
        if platform_above > 0.0 {
            (floor.ceiling_height - platform_above - floor_offset).max(1.0)
        } else {
            floor.ceiling_height
        }
    });
    let own_ceiling = layered(misc.map(|m| &m.ceiling_structure));
    let (gap, platform) = match own_ceiling {
        None if platform_above > 0.0 => (0.0, (platform_above - SLAB_THICKNESS).max(0.5)),
        own => {
            let finish = misc
                .map_or(floor.settings.ceiling_finish_thickness, |m| {
                    m.ceiling_finish_thickness
                })
                .max(0.0);
            let rough = named
                .and_then(|n| n.rough_ceiling)
                .map_or(finish, |r| (r - ceiling_height).max(finish));
            (rough, own.unwrap_or(SLAB_THICKNESS))
        }
    };
    let monolithic = named.and_then(|n| n.monolithic_slab);
    RoomLevels {
        floor_offset,
        ceiling_height,
        floor_finish: misc.map_or(floor.settings.floor_finish_thickness, |m| {
            m.floor_finish_thickness
        }),
        floor_thickness: monolithic
            .map(|s| s.thickness.max(0.5))
            .or_else(|| layered(misc.map(|m| &m.floor_structure)))
            .unwrap_or(SLAB_THICKNESS),
        ceiling_thickness: gap + platform,
        has_floor: named.is_none_or(|n| n.has_floor),
        has_ceiling: named.is_none_or(|n| n.has_ceiling),
    }
}

/// Top of the ceiling platform of `room` on `floor`, scene elevation: where
/// a flat roof over the room sits (the Flat Roof directive).
pub fn room_ceiling_top(floor: &plan_core::Floor, room: &plan_core::Room) -> f64 {
    let l = room_levels(floor, room);
    floor.elevation + l.floor_offset + l.ceiling_height + l.ceiling_thickness
}

/// Stem walls under a room (R-26, R-40): concrete walls along the room's
/// exterior walls from the underside of the room's floor platform up to the
/// floor datum, where the walls above begin. A garage floor dropped below
/// the house floor gets them from its drop; a room with a Stem Wall height
/// gets them that deep below the datum. They stop at garage doors.
/// `thickness` is the foundation wall type's (the wall's own when unknown).
/// One mesh per run, tagged with its wall.
fn stem_only(
    floor: &plan_core::Floor,
    room: &plan_core::Room,
    levels: &RoomLevels,
    thickness: Option<f64>,
) -> Vec<Mesh> {
    use crate::builder::MeshSet;
    use crate::frame::Frame;
    use plan_core::geometry::dist_to_segment;
    use plan_core::{OpeningStyle, WallKind};
    // A Stem Wall height, or the thickened edge of a Monolithic Slab (R-31).
    let explicit = room
        .name_entry(&floor.room_names)
        .and_then(|n| {
            let slab = n.monolithic_slab.map(|s| s.stem_height);
            match (n.stem_wall_height, slab) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (a, b) => a.or(b),
            }
        })
        .filter(|h| *h > 0.5);
    let dropped = levels.floor_offset < -0.5;
    let datum = floor.elevation;
    let platform = datum + levels.floor_offset - levels.floor_thickness;
    let y0 = match (dropped, explicit) {
        (true, Some(h)) => platform.min(datum - h),
        (true, None) => platform,
        (false, Some(h)) => datum - h,
        (false, None) => return Vec::new(),
    };
    if datum - y0 < 0.5 || room.polygon.len() < 3 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let n = room.polygon.len();
    for i in 0..n {
        let (p, q) = (room.polygon[i], room.polygon[(i + 1) % n]);
        let len = p.dist(q);
        if len < 1.0 {
            continue;
        }
        let dir = q.sub(p).normalized();
        let mid = Point::lerp(p, q, 0.5);
        let Some(wall) = floor.walls.iter().find(|w| {
            w.kind == WallKind::Exterior
                && !w.flags.invisible
                && w.length() > 1e-6
                && dist_to_segment(mid, w.start, w.end) <= w.thickness * 0.5 + 1.0
                && w.direction().cross(dir).abs() < 0.02
        }) else {
            continue;
        };
        // Garage doors in the run leave the stem open; the rest is split.
        let along = |pt: Point| pt.sub(p).dot(dir);
        let mut gaps: Vec<(f64, f64)> = floor
            .openings_on(wall.id)
            .filter(|o| o.style == OpeningStyle::Garage)
            .map(|o| {
                let c = along(wall.point_at(o.center_offset));
                (c - o.width * 0.5, c + o.width * 0.5)
            })
            .collect();
        gaps.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut runs = Vec::new();
        let mut cursor = 0.0;
        for (g0, g1) in gaps {
            if g0 > cursor {
                runs.push((cursor, g0.min(len)));
            }
            cursor = cursor.max(g1);
        }
        if cursor < len {
            runs.push((cursor, len));
        }
        let t = thickness.unwrap_or(wall.thickness).max(1.0);
        let mut synthetic = wall.clone();
        synthetic.start = p;
        synthetic.end = q;
        synthetic.thickness = t;
        synthetic.curve = None;
        synthetic.bottom_offset = 0.0;
        let frame = Frame::new(&synthetic, y0);
        for (s0, s1) in runs.into_iter().filter(|r| r.1 - r.0 > 1.0) {
            let mut set = MeshSet::default();
            frame.cuboid(
                set.material(Material::Concrete),
                (s0, s1),
                (-t * 0.5, t * 0.5),
                (0.0, datum - y0),
            );
            out.extend(set.finish(Some(wall.id)));
        }
    }
    out
}

/// Everything a room adds beside its platforms and stem walls: the stem
/// walls themselves (R-26, R-31), the footings under foundation walls
/// (R-61), the base, chair rail and crown moldings of its Moldings tab
/// (R-34) and the floor, ceiling and wall surfaces of its Materials tab
/// (R-36). One mesh per run or surface.
pub(crate) fn stem_walls(
    floor: &plan_core::Floor,
    room: &plan_core::Room,
    levels: &RoomLevels,
    thickness: Option<f64>,
) -> Vec<Mesh> {
    let mut out = stem_only(floor, room, levels, thickness);
    out.extend(footings(floor, room));
    out.extend(room_moldings(floor, room, levels));
    out.extend(room_surfaces(floor, room, levels));
    out
}

/// Footings under the foundation walls of a Walls with Footings foundation
/// (R-61): a concrete box under each wall run of the room, as wide and deep
/// as the Build Foundation dialog said.
fn footings(floor: &plan_core::Floor, room: &plan_core::Room) -> Vec<Mesh> {
    use crate::builder::MeshSet;
    use crate::frame::Frame;
    use plan_core::floors::FoundationKind;
    use plan_core::geometry::dist_to_segment;
    let Some(build) = floor.settings.foundation else {
        return Vec::new();
    };
    if floor.kind != plan_core::FloorKind::Foundation
        || !matches!(build.kind, FoundationKind::StemWall { .. })
        || build.footing_width <= 0.0
        || build.footing_depth <= 0.0
        || room.polygon.len() < 3
    {
        return Vec::new();
    }
    let mut out = Vec::new();
    let n = room.polygon.len();
    for i in 0..n {
        let (p, q) = (room.polygon[i], room.polygon[(i + 1) % n]);
        let len = p.dist(q);
        if len < 1.0 {
            continue;
        }
        let dir = q.sub(p).normalized();
        let mid = Point::lerp(p, q, 0.5);
        let Some(wall) = floor.walls.iter().find(|w| {
            w.is_foundation()
                && !w.is_curved()
                && w.length() > 1e-6
                && dist_to_segment(mid, w.start, w.end) <= w.thickness * 0.5 + 1.0
                && w.direction().cross(dir).abs() < 0.02
        }) else {
            continue;
        };
        let mut synthetic = wall.clone();
        synthetic.start = p;
        synthetic.end = q;
        synthetic.bottom_offset = 0.0;
        let base = floor.elevation + wall.bottom_offset;
        let frame = Frame::new(&synthetic, base - build.footing_depth);
        let half = build.footing_width * 0.5;
        let mut set = MeshSet::default();
        frame.cuboid(
            set.material(Material::Concrete),
            (0.0, len),
            (-half, half),
            (0.0, build.footing_depth),
        );
        out.extend(set.finish(Some(wall.id)));
    }
    out
}

fn scene_point(p: Point, y: f64) -> [f32; 3] {
    [p.x as f32, y as f32, (-p.y) as f32]
}

/// The molding runs of a room's Moldings tab, swept along the interior
/// surfaces with the miters of [`crate::details::molding_mesh`] (R-34).
/// Base sits on the finished floor, a chair rail 32" up, crown hangs from the
/// finished ceiling; each stops at doors and openings that reach into its
/// height.
fn room_moldings(
    floor: &plan_core::Floor,
    room: &plan_core::Room,
    levels: &RoomLevels,
) -> Vec<Mesh> {
    use plan_core::details::{MoldingLine, MoldingProfile};
    use plan_core::extras::MoldingKind;
    use plan_core::rooms::{molding_def, molding_defs, molding_span};
    let Some(named) = room.name_entry(&floor.room_names) else {
        return Vec::new();
    };
    let on_floor = levels.floor_offset + levels.floor_finish;
    let ceiling = levels.ceiling_height - levels.floor_finish;
    let mut out = Vec::new();
    for m in &named.moldings {
        if m.profile.trim().is_empty() || (m.kind == MoldingKind::Crown && !levels.has_ceiling) {
            continue;
        }
        let Some(def) = molding_def(&m.profile).or_else(|| molding_defs(m.kind).first().copied())
        else {
            continue;
        };
        let height = if m.height > 0.0 {
            m.height
        } else {
            def.height()
        };
        let (lo, hi) = molding_span(m.kind, height, ceiling);
        for run in room.molding_runs(floor, on_floor + lo, on_floor + hi) {
            let line = MoldingLine {
                polyline: run,
                profile: MoldingProfile::Custom(def.points()),
                height,
                width: def.projection(),
                elevation: floor.elevation + on_floor + lo,
                ..MoldingLine::default()
            };
            out.extend(crate::details::molding_mesh(&line, 0.0));
        }
    }
    out
}

/// A surface material named in the Materials tab: `None` when it is empty or
/// is what the surface is already built of.
fn surface_material(name: &str, plain: Material) -> Option<Material> {
    let n = name.trim();
    if n.is_empty() {
        return None;
    }
    let m = crate::details::material_of(n, plain);
    (m != plain).then_some(m)
}

/// Which surface of a room a plate is (R-36): the role a Room-mode paint
/// names, whatever the name is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomSurface {
    Floor,
    Ceiling,
    Walls,
}

/// The object id of a room's surface plate. It is not a plan object: the high
/// bit is set, and the rest hashes the floor, the room and the role, so the
/// app can find the plates a room named itself and paint them with the exact
/// library material ([`room_surface_names`]).
pub fn room_surface_id(floor: &plan_core::Floor, room: &plan_core::Room, role: RoomSurface) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    eat(&floor.elevation.to_bits().to_le_bytes());
    eat(floor.name.as_bytes());
    eat(room.label.as_bytes());
    eat(&((room.centroid.x * 8.0).round() as i64).to_le_bytes());
    eat(&((room.centroid.y * 8.0).round() as i64).to_le_bytes());
    eat(&[role as u8]);
    h | (1 << 63)
}

/// The material name `room` itself gives to `role` (not the floor's default),
/// if any.
fn own_name(floor: &plan_core::Floor, room: &plan_core::Room, role: RoomSurface) -> Option<String> {
    let n = room.name_entry(&floor.room_names)?;
    let name = match role {
        RoomSurface::Floor => n.floor_finish.as_deref(),
        RoomSurface::Ceiling => n.ceiling_finish.as_deref(),
        RoomSurface::Walls => n.misc.as_ref().map(|m| m.wall_covering.as_str()),
    }?;
    (!name.trim().is_empty()).then(|| name.to_string())
}

/// Every room surface plate of `project` whose material the room names itself
/// (what the Material Painter's Room mode writes): its object id and the
/// library name. The plates carry that id, so the app recolors them with the
/// exact material, texture and class instead of the keyword guess.
pub fn room_surface_names(project: &plan_core::Project) -> Vec<(u64, RoomSurface, String)> {
    let mut out = Vec::new();
    for floor in &project.floors {
        if floor.room_names.is_empty() {
            continue;
        }
        for room in plan_core::detect_rooms(&floor.walls, super::ROOM_TOLERANCE) {
            for role in [RoomSurface::Floor, RoomSurface::Ceiling, RoomSurface::Walls] {
                if let Some(name) = own_name(floor, &room, role) {
                    out.push((room_surface_id(floor, &room, role), role, name));
                }
            }
        }
    }
    out
}

/// Thin plates of the surface materials a room names for its floor, ceiling
/// and walls (R-36), lying on the platform tops, the ceiling underside and the
/// interior wall faces. A surface without a name of its own takes the floor's
/// default (Floor Defaults).
fn room_surfaces(
    floor: &plan_core::Floor,
    room: &plan_core::Room,
    levels: &RoomLevels,
) -> Vec<Mesh> {
    use crate::builder::MeshBuilder;
    use plan_core::foundation::PlatformKind;
    const PLATE: f64 = 0.06;
    let named = room.name_entry(&floor.room_names);
    let pick = |own: Option<&str>, fallback: &str| -> String {
        own.filter(|s| !s.trim().is_empty())
            .unwrap_or(fallback)
            .to_string()
    };
    let floor_name = pick(
        named.and_then(|n| n.floor_finish.as_deref()),
        &floor.settings.floor_material,
    );
    let ceiling_name = pick(
        named.and_then(|n| n.ceiling_finish.as_deref()),
        &floor.settings.ceiling_material,
    );
    let wall_name = pick(
        named
            .and_then(|n| n.misc.as_ref())
            .map(|m| m.wall_covering.as_str()),
        &floor.settings.wall_material,
    );
    let mut out = Vec::new();
    let y_top = floor.elevation + levels.floor_offset + levels.floor_finish;
    let y_ceiling = floor.elevation + levels.floor_offset + levels.ceiling_height;
    // A surface the room names itself always gets its plate, tagged with its
    // role's id: a library name the keyword table does not know ("Walnut")
    // still shows once the app paints the plate with the exact material.
    let role_material = |role: RoomSurface, name: &str, plain: Material| -> Option<Material> {
        surface_material(name, plain).or_else(|| own_name(floor, room, role).map(|_| plain))
    };
    let tag = |role: RoomSurface, meshes: Vec<Mesh>| -> Vec<Mesh> {
        if own_name(floor, room, role).is_none() {
            return meshes;
        }
        let id = room_surface_id(floor, room, role);
        meshes
            .into_iter()
            .map(|mut m| {
                m.object_id = Some(id);
                m
            })
            .collect()
    };
    if levels.has_floor {
        if let Some(mat) = role_material(RoomSurface::Floor, &floor_name, Material::Floor) {
            let mut cut = crate::foundation::platform_holes(floor, PlatformKind::Floor);
            cut.extend(room.holes.iter().cloned());
            out.extend(tag(
                RoomSurface::Floor,
                crate::foundation::build_platform(
                    mat,
                    std::slice::from_ref(room),
                    &cut,
                    y_top,
                    y_top + PLATE,
                    None,
                )
                .into_iter()
                .collect(),
            ));
        }
    }
    if levels.has_ceiling {
        if let Some(mat) = role_material(RoomSurface::Ceiling, &ceiling_name, Material::Ceiling) {
            let mut cut = crate::foundation::platform_holes(floor, PlatformKind::Ceiling);
            cut.extend(room.holes.iter().cloned());
            out.extend(tag(
                RoomSurface::Ceiling,
                crate::foundation::build_platform(
                    mat,
                    std::slice::from_ref(room),
                    &cut,
                    y_ceiling - PLATE,
                    y_ceiling,
                    None,
                )
                .into_iter()
                .collect(),
            ));
        }
    }
    if let Some(mat) = role_material(RoomSurface::Walls, &wall_name, Material::WallInterior) {
        let poly = if room.inner_polygon.len() >= 3 {
            &room.inner_polygon
        } else {
            &room.polygon
        };
        let mut mesh = MeshBuilder::new(mat);
        let n = poly.len();
        for i in 0..n {
            let (p, q) = (poly[i], poly[(i + 1) % n]);
            let len = p.dist(q);
            if len < 1.0 {
                continue;
            }
            let dir = q.sub(p).normalized();
            let into = dir.perp();
            let lift = into * PLATE;
            let normal = [into.x as f32, 0.0, (-into.y) as f32];
            let mut panel = |a: f64, b: f64, y0: f64, y1: f64| {
                if b - a < 0.25 || y1 - y0 < 0.25 {
                    return;
                }
                let (pa, pb) = (p + dir * a + lift, p + dir * b + lift);
                mesh.quad(
                    [
                        scene_point(pa, y0),
                        scene_point(pb, y0),
                        scene_point(pb, y1),
                        scene_point(pa, y1),
                    ],
                    [
                        [(a / IN_PER_FT) as f32, (y0 / IN_PER_FT) as f32],
                        [(b / IN_PER_FT) as f32, (y0 / IN_PER_FT) as f32],
                        [(b / IN_PER_FT) as f32, (y1 / IN_PER_FT) as f32],
                        [(a / IN_PER_FT) as f32, (y1 / IN_PER_FT) as f32],
                    ],
                    normal,
                );
            };
            let mut cursor = 0.0;
            for o in plan_core::rooms::edge_openings(floor, p, q) {
                let (from, to) = (o.from.max(0.0), o.to.min(len));
                if to <= from {
                    continue;
                }
                panel(cursor, from, y_top, y_ceiling);
                panel(from, to, y_top, floor.elevation + o.sill);
                panel(from, to, floor.elevation + o.head, y_ceiling);
                cursor = cursor.max(to);
            }
            panel(cursor, len, y_top, y_ceiling);
        }
        if !mesh.is_empty() {
            out.extend(tag(RoomSurface::Walls, vec![mesh.finish(None)]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_room(floor: &mut plan_core::Floor) -> plan_core::Room {
        use plan_core::{Wall, WallKind};
        let c = [
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            Point::new(120.0, 96.0),
            Point::new(0.0, 96.0),
        ];
        for i in 0..4 {
            floor.walls.push(Wall {
                id: i as u64 + 1,
                ..Wall::new(c[i], c[(i + 1) % 4], 6.0, 108.0, WallKind::Exterior)
            });
        }
        plan_core::detect_rooms(&floor.walls, 0.5).remove(0)
    }

    #[test]
    fn surface_materials_skip_what_the_surface_already_is() {
        assert_eq!(surface_material("", Material::Floor), None);
        assert_eq!(surface_material("  ", Material::Floor), None);
        assert_eq!(surface_material("Oak Hardwood", Material::Floor), None);
        assert_eq!(
            surface_material("Ceramic Tile", Material::Floor),
            Some(Material::Stone)
        );
        assert_eq!(
            surface_material("Painted Drywall", Material::WallInterior),
            None
        );
        assert_eq!(
            surface_material("Brick", Material::WallInterior),
            Some(Material::Brick)
        );
        for n in plan_core::rooms::FLOOR_SURFACES {
            let _ = surface_material(n, Material::Floor);
        }
    }

    #[test]
    fn levels_add_the_finish_the_rough_ceiling_and_the_slab_flag() {
        let mut floor = plan_core::Floor::new("1st", 0.0);
        floor.ceiling_height = 108.0;
        let room = box_room(&mut floor);
        // An unnamed room: 5/8" ceiling finish and a 1" platform above the
        // finished ceiling.
        let l = room_levels(&floor, &room);
        assert_eq!(l.ceiling_height, 108.0);
        assert!((l.ceiling_thickness - (0.625 + SLAB_THICKNESS)).abs() < 1e-9);
        assert_eq!(l.floor_thickness, SLAB_THICKNESS);
        // A rough ceiling above the finish sets the gap.
        let mut n = plan_core::RoomName::new(Point::new(60.0, 48.0), "Den", "Den");
        n.rough_ceiling = Some(120.0);
        floor.room_names = vec![n.clone()];
        let l = room_levels(&floor, &room);
        assert!((l.ceiling_thickness - (12.0 + SLAB_THICKNESS)).abs() < 1e-9);
        // One below the finish layers is no lower than them.
        floor.room_names[0].rough_ceiling = Some(100.0);
        let l = room_levels(&floor, &room);
        assert!((l.ceiling_thickness - (0.625 + SLAB_THICKNESS)).abs() < 1e-9);
        // The slab flag sets the floor platform and its edge depth.
        floor.room_names[0].rough_ceiling = None;
        floor.room_names[0].monolithic_slab = Some(plan_core::rooms::RoomSlab {
            thickness: 6.0,
            stem_height: 14.0,
        });
        let l = room_levels(&floor, &room);
        assert_eq!(l.floor_thickness, 6.0);
        let stem = stem_only(&floor, &room, &l, None);
        assert_eq!(stem.len(), 4, "a run of stem wall under each exterior wall");
        let (lo, _) = stem[0].bounds().unwrap();
        assert!((f64::from(lo[1]) + 14.0).abs() < 1e-4);
    }

    #[test]
    fn a_foundation_floor_takes_its_ceiling_from_the_platform_above() {
        let mut floor = plan_core::Floor::new("Foundation", -108.0);
        floor.kind = plan_core::FloorKind::Foundation;
        floor.ceiling_height = 108.0;
        floor.settings.ceiling_structure_thickness = 10.25;
        let room = box_room(&mut floor);
        let l = room_levels(&floor, &room);
        assert!((l.ceiling_height - (108.0 - 10.25)).abs() < 1e-9);
        assert!((l.ceiling_thickness - (10.25 - SLAB_THICKNESS)).abs() < 1e-9);
        // The ceiling top is the underside of the first floor's slab (1" below
        // its finished level at 0).
        assert!((room_ceiling_top(&floor, &room) + SLAB_THICKNESS).abs() < 1e-9);
        // A basement slab's offset is taken off the clear height.
        let mut n = plan_core::RoomName::new(Point::new(60.0, 48.0), "Basement", "Basement");
        n.floor_height_offset = 4.0;
        floor.room_names = vec![n];
        let l = room_levels(&floor, &room);
        assert!((l.ceiling_height - (108.0 - 10.25 - 4.0)).abs() < 1e-9);
        assert!((room_ceiling_top(&floor, &room) + SLAB_THICKNESS).abs() < 1e-9);
    }

    #[test]
    fn flat_square_slab_is_a_closed_box() {
        let sq = [
            [0.0, 100.0, 0.0],
            [120.0, 100.0, 0.0],
            [120.0, 100.0, -120.0],
            [0.0, 100.0, -120.0],
        ];
        for poly in [sq.to_vec(), sq.iter().rev().copied().collect::<Vec<_>>()] {
            let m = slab_from_polygon(&poly, 6.0, Material::Roof);
            assert_eq!(m.triangle_count(), 12);
            let (lo, hi) = m.bounds().unwrap();
            assert!((hi[1] - 100.0).abs() < 1e-4 && (lo[1] - 94.0).abs() < 1e-4);
            // Normals agree with winding and the box faces outward.
            let center = [60.0, 97.0, -60.0];
            for t in m.indices.chunks(3) {
                let p = |i: u32| m.vertices[i as usize].position.map(f64::from);
                let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                let g = cross3(sub3(b, a), sub3(c, a));
                let out = sub3(a, center);
                assert!(g[0] * out[0] + g[1] * out[1] + g[2] * out[2] > 0.0);
            }
        }
    }

    #[test]
    fn sloped_slab_hangs_below_the_plane_and_degenerate_is_empty() {
        let tilted = [
            [0.0, 0.0, 0.0],
            [120.0, 0.0, 0.0],
            [120.0, 60.0, -120.0],
            [0.0, 60.0, -120.0],
        ];
        let m = slab_from_polygon(&tilted, 4.0, Material::Roof);
        assert_eq!(m.triangle_count(), 12);
        let (lo, _) = m.bounds().unwrap();
        assert!(lo[1] < 0.0);
        assert_eq!(
            slab_from_polygon(&tilted[..2], 4.0, Material::Roof).triangle_count(),
            0
        );
    }

    #[test]
    fn a_surface_the_room_names_gets_a_tagged_plate_whatever_the_name() {
        let mut floor = plan_core::Floor::new("1st", 0.0);
        let room = box_room(&mut floor);
        let levels = room_levels(&floor, &room);
        // Nothing named: no plate.
        let plain = room_surfaces(&floor, &room, &levels);
        assert!(plain.iter().all(|m| m.object_id.is_none()));
        // "Walnut" is no keyword of the table, but the room names it itself.
        let mut n = plan_core::RoomName::new(Point::new(60.0, 48.0), "Den", "Den");
        n.floor_finish = Some("Walnut".into());
        floor.room_names = vec![n];
        let tagged: Vec<_> = room_surfaces(&floor, &room, &levels)
            .into_iter()
            .filter(|m| m.object_id.is_some())
            .collect();
        assert_eq!(tagged.len(), 1);
        let id = room_surface_id(&floor, &room, RoomSurface::Floor);
        assert_eq!(tagged[0].object_id, Some(id));
        assert_ne!(
            id,
            room_surface_id(&floor, &room, RoomSurface::Ceiling),
            "each role has its own id"
        );
        assert!(id >> 63 == 1, "never a plan object id");
        let mut p = plan_core::Project::new("t");
        p.floors[0] = floor;
        let names = room_surface_names(&p);
        assert_eq!(names.len(), 1);
        assert_eq!(
            (names[0].0, names[0].1, names[0].2.as_str()),
            (id, RoomSurface::Floor, "Walnut")
        );
    }
}
