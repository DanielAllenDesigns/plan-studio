//! Thermal Envelope Data and Export to REScheck (L-83; manual pp. 1304 to
//! 1306).
//!
//! The thermal envelope is the boundary around the conditioned space: floor
//! platforms, ceiling platforms, walls, doors and windows. [`envelope`] reads
//! it from the plan:
//!
//! * A **conditioned room** is a room whose own setting says so, else whose
//!   room type does (interior types and Open Below are conditioned;
//!   exterior and hybrid types are not).
//! * A **wall** is on the envelope when conditioned space lies on one side of
//!   it and not on the other. It faces the unconditioned side. Its gross area
//!   is the centerline length times the wall height, openings included (the
//!   doors and windows are listed on their own). Doors and windows in it take
//!   its direction.
//! * **Direction** is the compass quarter the face looks toward, from the
//!   plan's North Pointer (the terrain's north angle; up on the screen when
//!   there is none): within 45 degrees of north is North, and likewise South,
//!   East and West ([`Orientation::of_azimuth`]).
//! * **Floors** and **ceilings** are the interior areas of the conditioned
//!   rooms that have no conditioned room under them or over them. A
//!   first-floor room with a monolithic slab is a **slab on grade** instead
//!   (its perimeter is the length of the envelope walls on that floor).
//!   Their R-values come from the layered definitions (`plan_core::assemblies`)
//!   when the plan has them.
//! * Wall **R-values** are read from the wall type's layers: insulation
//!   layers (by their name and material) count as cavity insulation when they
//!   are the main layer and as continuous insulation otherwise, at 3.5 R per
//!   inch (5 for foam). A type with none has the options' default (DECISIONS
//!   TM5).
//!
//! [`thermal_csv`] writes the envelope one component per row, by floor level.
//! [`to_rxl`] writes the REScheck file: like walls on a floor are grouped by
//! direction, and with Group Similar Walls / Group Similar Doors/Windows
//! like ones on every floor are added together. The XML element names are
//! our own reading of the manual (DECISIONS TM6).

use plan_core::defaults::{RoomTypeDef, WallTypeDef};
use plan_core::geometry::{point_in_polygon, Point};
use plan_core::model::{Floor, OpeningKind, Wall};
use plan_core::rooms::{detect_rooms, function_defaults, Room};
use plan_core::walls::WallClass;
use plan_core::Project;
use std::collections::BTreeMap;

/// North, East, South or West.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Orientation {
    North,
    East,
    South,
    West,
}

impl Orientation {
    pub const ALL: [Orientation; 4] = [
        Orientation::North,
        Orientation::East,
        Orientation::South,
        Orientation::West,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Orientation::North => "North",
            Orientation::East => "East",
            Orientation::South => "South",
            Orientation::West => "West",
        }
    }

    /// The quarter a compass azimuth (degrees clockwise from true north)
    /// falls in: 45 degrees or less from north is North, and so on.
    pub fn of_azimuth(az: f64) -> Orientation {
        let a = az.rem_euclid(360.0);
        if !(45.0 + 1e-9..315.0 - 1e-9).contains(&a) {
            Orientation::North
        } else if (a - 180.0).abs() <= 45.0 + 1e-9 {
            Orientation::South
        } else if a < 180.0 {
            Orientation::East
        } else {
            Orientation::West
        }
    }

    /// The quarter a direction on the plan looks toward, given the north
    /// angle of the plan (degrees clockwise from the top of the plan to
    /// north).
    pub fn of_plan_vector(v: Point, north_angle_deg: f64) -> Orientation {
        let plan = v.x.atan2(v.y).to_degrees();
        Orientation::of_azimuth(plan - north_angle_deg)
    }
}

/// What a component of the envelope is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    FloorPlatform,
    Slab,
    CeilingPlatform,
    Wall,
    Door,
    Window,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::FloorPlatform => "Floor",
            Kind::Slab => "Slab on Grade",
            Kind::CeilingPlatform => "Ceiling",
            Kind::Wall => "Wall",
            Kind::Door => "Door",
            Kind::Window => "Window",
        }
    }
}

/// One piece of the envelope, or a group of like pieces added together.
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    /// The floor level (`None` for a group across floors).
    pub floor: Option<usize>,
    pub floor_name: String,
    pub kind: Kind,
    pub assembly: String,
    pub orientation: Option<Orientation>,
    /// Gross area, square feet (the slab's is zero).
    pub area_sq_ft: f64,
    pub r_cavity: f64,
    pub r_continuous: f64,
    /// Doors and windows.
    pub u_factor: f64,
    pub shgc: f64,
    /// Slab perimeter, feet.
    pub perimeter_ft: f64,
    /// The wall label; a group keeps it when every wall has the same one.
    pub label: String,
    /// How many pieces were added together.
    pub count: usize,
}

/// What the export reads besides the plan.
#[derive(Debug, Clone, Copy, Default)]
pub struct Inputs<'a> {
    /// The plan's room types (Default Settings > Rooms): an unnamed or
    /// unset room takes its type's conditioned setting.
    pub room_types: &'a [RoomTypeDef],
    /// Wall types of the program's defaults, for walls whose type the plan
    /// does not carry itself.
    pub wall_types: &'a [WallTypeDef],
}

/// The choices of the Export to REScheck dialog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// Walls with the same properties and direction are added together,
    /// whatever floor they are on (otherwise per floor).
    pub group_walls: bool,
    /// Doors and windows with the same properties and direction are added
    /// together (otherwise each is listed).
    pub group_openings: bool,
    /// Cavity R-value of a wall type that has no insulation layer.
    pub default_wall_cavity_r: f64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            group_walls: true,
            group_openings: true,
            default_wall_cavity_r: 0.0,
        }
    }
}

/// The envelope of a plan.
#[derive(Debug, Clone, PartialEq)]
pub struct Envelope {
    /// Square feet.
    pub conditioned_floor_area: f64,
    /// Degrees clockwise from the top of the plan to north.
    pub north_angle: f64,
    /// The side that faces down the screen.
    pub front: Orientation,
    /// By floor level, then kind, in plan order.
    pub components: Vec<Component>,
}

impl Envelope {
    /// The components of `kind` (and `orientation` when given).
    pub fn of(&self, kind: Kind, orientation: Option<Orientation>) -> Vec<&Component> {
        self.components
            .iter()
            .filter(|c| c.kind == kind && orientation.is_none_or(|o| c.orientation == Some(o)))
            .collect()
    }

    /// Total gross area of `kind` (and `orientation` when given), square feet.
    pub fn area(&self, kind: Kind, orientation: Option<Orientation>) -> f64 {
        self.of(kind, orientation)
            .iter()
            .map(|c| c.area_sq_ft)
            .sum()
    }
}

/// The plan's north angle: the terrain's, else zero (up on the screen).
pub fn north_angle_of(project: &Project) -> f64 {
    project
        .terrain
        .as_ref()
        .and_then(|v| {
            v.get("terrain")
                .and_then(|t| t.get("north_angle"))
                .or_else(|| v.get("north_angle"))
        })
        .and_then(|a| a.as_f64())
        .unwrap_or(0.0)
}

fn sq_ft(sq_in: f64) -> f64 {
    sq_in / 144.0
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn is_conditioned_room(room_name: Option<&plan_core::model::RoomName>, inputs: &Inputs) -> bool {
    let Some(e) = room_name else {
        return true;
    };
    if let Some(c) = e.conditioned {
        return c;
    }
    match inputs.room_types.iter().find(|t| t.name == e.room_type) {
        Some(t) => t.conditioned,
        None => function_defaults(&e.room_type, &e.room_type).conditioned,
    }
}

struct FloorRooms {
    rooms: Vec<Room>,
    conditioned: Vec<bool>,
}

impl FloorRooms {
    fn of(f: &Floor, inputs: &Inputs) -> FloorRooms {
        let rooms = detect_rooms(&f.walls, 0.5);
        let conditioned = rooms
            .iter()
            .map(|r| is_conditioned_room(r.name_entry(&f.room_names), inputs))
            .collect();
        FloorRooms { rooms, conditioned }
    }

    fn conditioned_at(&self, p: Point) -> bool {
        self.rooms.iter().zip(&self.conditioned).any(|(r, c)| {
            *c && (point_in_polygon(p, &r.inner_polygon) || point_in_polygon(p, &r.polygon))
        })
    }
}

fn layer_is_insulation(name: &str, material: &str) -> Option<f64> {
    let t = format!("{name} {material}").to_lowercase();
    const FOAM: [&str; 6] = ["foam", "xps", "eps", "polyiso", "rigid", "spray"];
    const BATT: [&str; 6] = [
        "insul",
        "batt",
        "fiberglass",
        "cellulose",
        "rockwool",
        "mineral wool",
    ];
    if FOAM.iter().any(|k| t.contains(k)) {
        Some(5.0)
    } else if BATT.iter().any(|k| t.contains(k)) {
        Some(3.5)
    } else {
        None
    }
}

/// Cavity and continuous R-values of a wall type (see the module docs).
pub fn wall_r_values(def: Option<&WallTypeDef>, default_cavity: f64) -> (f64, f64) {
    let Some(def) = def else {
        return (default_cavity, 0.0);
    };
    let (mut cavity, mut continuous) = (0.0, 0.0);
    for l in &def.layers {
        if let Some(per_inch) = layer_is_insulation(&l.name, &l.material) {
            if l.is_main {
                cavity += per_inch * l.thickness;
            } else {
                continuous += per_inch * l.thickness;
            }
        }
    }
    if cavity == 0.0 {
        cavity = default_cavity;
    }
    (round2(cavity), round2(continuous))
}

fn platform_r(
    kinds: &[plan_core::assemblies::AssemblyKind],
    f: &Floor,
    misc: Option<&plan_core::extras::RoomMisc>,
) -> (f64, f64) {
    let (mut cav, mut cont) = (0.0, 0.0);
    for k in kinds {
        let r = plan_core::assemblies::resolve(*k, &f.settings, misc);
        for l in &r.assembly.layers {
            cav += l.r_cavity.max(0.0);
            cont += l.r_continuous.max(0.0);
        }
    }
    (round2(cav), round2(cont))
}

fn platform_name(
    kind: plan_core::assemblies::AssemblyKind,
    f: &Floor,
    misc: Option<&plan_core::extras::RoomMisc>,
) -> String {
    let r = plan_core::assemblies::resolve(kind, &f.settings, misc);
    r.assembly
        .framing_layer()
        .map(|l| l.material.clone())
        .or_else(|| r.assembly.layers.first().map(|l| l.material.clone()))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| kind.label().to_string())
}

fn skip_wall(w: &Wall, def: Option<&WallTypeDef>) -> bool {
    if !matches!(
        w.class,
        WallClass::Standard
            | WallClass::Foundation
            | WallClass::Pony { .. }
            | WallClass::HalfWall { .. }
    ) {
        return true;
    }
    let n = w
        .wall_type
        .as_deref()
        .or(def.map(|d| d.name.as_str()))
        .unwrap_or("")
        .to_lowercase();
    ["railing", "fence", "deck"].iter().any(|k| n.contains(k))
}

/// Reads the thermal envelope of `project` (every piece, ungrouped).
pub fn envelope(project: &Project, inputs: &Inputs, opts: &Options) -> Envelope {
    let north = north_angle_of(project);
    let per_floor: Vec<FloorRooms> = project
        .floors
        .iter()
        .map(|f| FloorRooms::of(f, inputs))
        .collect();
    let mut components = Vec::new();
    let mut conditioned_area = 0.0;
    for (fi, f) in project.floors.iter().enumerate() {
        let fr = &per_floor[fi];
        let mk = |kind: Kind, assembly: String| Component {
            floor: Some(fi),
            floor_name: f.name.clone(),
            kind,
            assembly,
            orientation: None,
            area_sq_ft: 0.0,
            r_cavity: 0.0,
            r_continuous: 0.0,
            u_factor: 0.0,
            shgc: 0.0,
            perimeter_ft: 0.0,
            label: String::new(),
            count: 1,
        };
        // Floor and ceiling platforms, room by room.
        let mut floors_c = Vec::new();
        let mut ceilings_c = Vec::new();
        let mut slab_area = 0.0;
        for (ri, r) in fr.rooms.iter().enumerate() {
            if !fr.conditioned[ri] {
                continue;
            }
            let area = r.interior_area_sq_ft();
            conditioned_area += area;
            let entry = r.name_entry(&f.room_names);
            let misc = entry.and_then(|e| e.misc.as_ref());
            let c = r.centroid;
            let under = fi > 0 && per_floor[fi - 1].conditioned_at(c);
            let over = per_floor.get(fi + 1).is_some_and(|up| up.conditioned_at(c));
            if !under {
                if fi == 0 && entry.is_some_and(|e| e.monolithic_slab.is_some()) {
                    slab_area += area;
                } else {
                    let (cav, cont) = platform_r(
                        &[plan_core::assemblies::AssemblyKind::FloorStructure],
                        f,
                        misc,
                    );
                    floors_c.push(Component {
                        area_sq_ft: area,
                        r_cavity: cav,
                        r_continuous: cont,
                        ..mk(
                            Kind::FloorPlatform,
                            platform_name(
                                plan_core::assemblies::AssemblyKind::FloorStructure,
                                f,
                                misc,
                            ),
                        )
                    });
                }
            }
            if !over {
                let (cav, cont) = platform_r(
                    &[
                        plan_core::assemblies::AssemblyKind::CeilingStructure,
                        plan_core::assemblies::AssemblyKind::CeilingFinish,
                    ],
                    f,
                    misc,
                );
                ceilings_c.push(Component {
                    area_sq_ft: area,
                    r_cavity: cav,
                    r_continuous: cont,
                    ..mk(
                        Kind::CeilingPlatform,
                        platform_name(
                            plan_core::assemblies::AssemblyKind::CeilingStructure,
                            f,
                            misc,
                        ),
                    )
                });
            }
        }
        // Walls, doors and windows.
        let mut walls_c = Vec::new();
        let mut doors_c = Vec::new();
        let mut windows_c = Vec::new();
        let mut slab_perimeter = 0.0;
        for w in &f.walls {
            let def = w.wall_type.as_deref().and_then(|n| {
                project
                    .wall_types
                    .iter()
                    .chain(inputs.wall_types)
                    .find(|t| t.name == n)
            });
            if skip_wall(w, def) {
                continue;
            }
            let mid = Point::new((w.start.x + w.end.x) * 0.5, (w.start.y + w.end.y) * 0.5);
            let n = w.normal();
            let off = w.thickness * 0.5 + 4.0;
            let left = fr.conditioned_at(Point::new(mid.x + n.x * off, mid.y + n.y * off));
            let right = fr.conditioned_at(Point::new(mid.x - n.x * off, mid.y - n.y * off));
            if left == right {
                continue;
            }
            // The wall faces the side that is not conditioned.
            let out = if left {
                Point::new(-n.x, -n.y)
            } else {
                Point::new(n.x, n.y)
            };
            let dir = Orientation::of_plan_vector(out, north);
            let length = w.end.sub(w.start).length();
            let (cav, cont) = wall_r_values(def, opts.default_wall_cavity_r);
            let assembly = w.wall_type.clone().unwrap_or_else(|| {
                match w.kind {
                    plan_core::model::WallKind::Exterior => "Exterior Wall",
                    plan_core::model::WallKind::Interior => "Interior Wall",
                }
                .to_string()
            });
            slab_perimeter += length / 12.0;
            walls_c.push(Component {
                orientation: Some(dir),
                area_sq_ft: length * w.height / 144.0,
                r_cavity: cav,
                r_continuous: cont,
                label: w.extras.label_text.clone().unwrap_or_default(),
                ..mk(Kind::Wall, assembly)
            });
            for o in f.openings.iter().filter(|o| o.wall_id == w.id) {
                let door = o.kind == OpeningKind::Door;
                let e = &o.extras.spec.energy;
                let assembly = if e.construction.trim().is_empty() {
                    o.type_name().to_string()
                } else {
                    e.construction.clone()
                };
                let c = Component {
                    orientation: Some(dir),
                    area_sq_ft: sq_ft(o.width * o.height),
                    u_factor: e.u_factor,
                    shgc: e.shgc,
                    ..mk(if door { Kind::Door } else { Kind::Window }, assembly)
                };
                if door {
                    doors_c.push(c);
                } else {
                    windows_c.push(c);
                }
            }
        }
        components.extend(floors_c);
        if slab_area > 0.0 {
            let (_, cont) = platform_r(
                &[plan_core::assemblies::AssemblyKind::FloorStructure],
                f,
                None,
            );
            components.push(Component {
                perimeter_ft: round2(slab_perimeter),
                r_continuous: cont,
                area_sq_ft: slab_area,
                ..mk(Kind::Slab, "Slab on Grade".to_string())
            });
        }
        components.extend(ceilings_c);
        components.extend(walls_c);
        components.extend(doors_c);
        components.extend(windows_c);
    }
    let front = Orientation::of_plan_vector(Point::new(0.0, -1.0), north);
    Envelope {
        conditioned_floor_area: round2(conditioned_area),
        north_angle: north,
        front,
        components,
    }
}

/// The envelope with like pieces added together: like walls on one floor by
/// direction always, and across floors with `group_walls`; doors and windows
/// with `group_openings`. Floor and ceiling platforms stay as they are.
pub fn grouped(env: &Envelope, opts: &Options) -> Vec<Component> {
    type Key = (
        Option<usize>,
        Kind,
        String,
        Option<Orientation>,
        i64,
        i64,
        i64,
        i64,
    );
    let key = |c: &Component| -> Key {
        let by_floor = match c.kind {
            Kind::Wall => (!opts.group_walls).then_some(c.floor).flatten(),
            Kind::Door | Kind::Window => (!opts.group_openings).then_some(c.floor).flatten(),
            _ => c.floor,
        };
        (
            by_floor,
            c.kind,
            c.assembly.clone(),
            c.orientation,
            (c.r_cavity * 100.0).round() as i64,
            (c.r_continuous * 100.0).round() as i64,
            (c.u_factor * 1000.0).round() as i64,
            (c.shgc * 1000.0).round() as i64,
        )
    };
    let mut out: Vec<Component> = Vec::new();
    let mut index: BTreeMap<Key, usize> = BTreeMap::new();
    for c in &env.components {
        let mergeable = matches!(c.kind, Kind::Wall | Kind::Door | Kind::Window);
        // Each door or window is its own item when not grouped.
        if !mergeable || (!opts.group_openings && matches!(c.kind, Kind::Door | Kind::Window)) {
            out.push(c.clone());
            continue;
        }
        match index.get(&key(c)) {
            Some(&i) => {
                let g = &mut out[i];
                g.area_sq_ft += c.area_sq_ft;
                g.count += c.count;
                if g.label != c.label {
                    g.label.clear();
                }
                if g.floor != c.floor {
                    g.floor = None;
                    g.floor_name = "All floors".to_string();
                }
            }
            None => {
                index.insert(key(c), out.len());
                out.push(c.clone());
            }
        }
    }
    out
}

// ===================================================================
// CSV
// ===================================================================

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn num(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Thermal Envelope Data: a comma-delimited file with a row for each
/// component of the envelope, by floor level, then a total of each kind and
/// direction.
pub fn thermal_csv(env: &Envelope) -> String {
    let mut out = String::new();
    out.push_str(
        "Floor Level,Component,Assembly,Direction,Area (sq ft),Cavity R,Continuous R,U-Factor,SHGC,Slab Perimeter (ft),Label\n",
    );
    for c in &env.components {
        let row = [
            csv_field(&c.floor_name),
            csv_field(c.kind.label()),
            csv_field(&c.assembly),
            c.orientation
                .map(|o| o.label().to_string())
                .unwrap_or_default(),
            num(c.area_sq_ft),
            if matches!(c.kind, Kind::Door | Kind::Window) {
                String::new()
            } else {
                num(c.r_cavity)
            },
            if matches!(c.kind, Kind::Door | Kind::Window) {
                String::new()
            } else {
                num(c.r_continuous)
            },
            if matches!(c.kind, Kind::Door | Kind::Window) {
                num(c.u_factor)
            } else {
                String::new()
            },
            if matches!(c.kind, Kind::Door | Kind::Window) {
                num(c.shgc)
            } else {
                String::new()
            },
            if c.kind == Kind::Slab {
                num(c.perimeter_ft)
            } else {
                String::new()
            },
            csv_field(&c.label),
        ];
        out.push_str(&row.join(","));
        out.push('\n');
    }
    // A total for each kind and direction.
    for kind in [
        Kind::FloorPlatform,
        Kind::Slab,
        Kind::CeilingPlatform,
        Kind::Wall,
        Kind::Door,
        Kind::Window,
    ] {
        let dirs: Vec<Option<Orientation>> =
            if matches!(kind, Kind::Wall | Kind::Door | Kind::Window) {
                Orientation::ALL.iter().map(|o| Some(*o)).collect()
            } else {
                vec![None]
            };
        for d in dirs {
            let items = env.of(kind, d);
            if items.is_empty() {
                continue;
            }
            let area: f64 = items.iter().map(|c| c.area_sq_ft).sum();
            let row = [
                "Total".to_string(),
                csv_field(kind.label()),
                String::new(),
                d.map(|o| o.label().to_string()).unwrap_or_default(),
                num(area),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                if kind == Kind::Slab {
                    num(items.iter().map(|c| c.perimeter_ft).sum())
                } else {
                    String::new()
                },
                String::new(),
            ];
            out.push_str(&row.join(","));
            out.push('\n');
        }
    }
    out
}

// ===================================================================
// REScheck (.rxl)
// ===================================================================

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

fn tag(out: &mut String, indent: usize, name: &str, value: &str) {
    out.push_str(&"  ".repeat(indent));
    out.push_str(&format!("<{name}>{}</{name}>\n", esc(value)));
}

fn open(out: &mut String, indent: usize, name: &str) {
    out.push_str(&"  ".repeat(indent));
    out.push_str(&format!("<{name}>\n"));
}

fn close(out: &mut String, indent: usize, name: &str) {
    out.push_str(&"  ".repeat(indent));
    out.push_str(&format!("</{name}>\n"));
}

/// The REScheck file of a plan: the project data from Project Information,
/// the conditioned floor area and the envelope. Location and the permit
/// fields are not exported, and neither are skylights.
pub fn to_rxl(project: &Project, inputs: &Inputs, opts: &Options) -> String {
    let env = envelope(project, inputs, opts);
    let list = grouped(&env, opts);
    let info = &project.info;
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    open(&mut out, 0, "REScheck");
    open(&mut out, 1, "Project");
    tag(&mut out, 2, "ProjectType", "New Construction");
    tag(&mut out, 2, "BuildingType", "1-and-2 Family, Detached");
    tag(&mut out, 2, "FrontFaces", env.front.label());
    tag(
        &mut out,
        2,
        "ConditionedFloorArea",
        &num(env.conditioned_floor_area),
    );
    open(&mut out, 2, "OwnerAgent");
    tag(&mut out, 3, "Name", &info.client_name);
    tag(&mut out, 3, "Address", &info.client_address_line());
    tag(&mut out, 3, "Phone", &info.client_phone);
    tag(&mut out, 3, "Email", &info.client_email);
    close(&mut out, 2, "OwnerAgent");
    open(&mut out, 2, "DesignerContractor");
    tag(&mut out, 3, "Name", &info.designer);
    tag(&mut out, 3, "Company", &info.company);
    close(&mut out, 2, "DesignerContractor");
    close(&mut out, 1, "Project");
    open(&mut out, 1, "Envelope");
    let section = |out: &mut String, name: &str, item: &str, kinds: &[Kind]| {
        let items: Vec<&Component> = list.iter().filter(|c| kinds.contains(&c.kind)).collect();
        open(out, 2, name);
        for c in items {
            open(out, 3, item);
            if !c.label.is_empty() {
                tag(out, 4, "Label", &c.label);
            }
            tag(out, 4, "Assembly", &c.assembly);
            if let Some(o) = c.orientation {
                tag(out, 4, "Orientation", o.label());
            }
            match c.kind {
                Kind::Slab => {
                    tag(out, 4, "Perimeter", &num(c.perimeter_ft));
                    tag(out, 4, "ContinuousR", &num(c.r_continuous));
                }
                Kind::Door | Kind::Window => {
                    tag(out, 4, "GrossArea", &num(c.area_sq_ft));
                    tag(out, 4, "UFactor", &num(c.u_factor));
                    tag(out, 4, "SHGC", &num(c.shgc));
                }
                Kind::Wall => {
                    tag(out, 4, "GrossArea", &num(c.area_sq_ft));
                    tag(out, 4, "CavityR", &num(c.r_cavity));
                    tag(out, 4, "ContinuousR", &num(c.r_continuous));
                    tag(out, 4, "OnCenterSpacing", "16");
                }
                _ => {
                    tag(out, 4, "GrossArea", &num(c.area_sq_ft));
                    tag(out, 4, "CavityR", &num(c.r_cavity));
                    tag(out, 4, "ContinuousR", &num(c.r_continuous));
                }
            }
            close(out, 3, item);
        }
        close(out, 2, name);
    };
    section(&mut out, "Floors", "Floor", &[Kind::FloorPlatform]);
    section(&mut out, "Slabs", "Slab", &[Kind::Slab]);
    section(&mut out, "Ceilings", "Ceiling", &[Kind::CeilingPlatform]);
    section(&mut out, "Walls", "Wall", &[Kind::Wall]);
    section(&mut out, "Doors", "Door", &[Kind::Door]);
    section(&mut out, "Windows", "Window", &[Kind::Window]);
    close(&mut out, 1, "Envelope");
    close(&mut out, 0, "REScheck");
    out
}

/// Checks that `xml` is well formed: one root, tags nested and closed, no
/// stray `<` or `&`. Used by the tests and before a file is written.
pub fn well_formed(xml: &str) -> Result<(), String> {
    let body = xml
        .strip_prefix("<?xml")
        .and_then(|s| s.split_once("?>"))
        .map(|(_, rest)| rest)
        .unwrap_or(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut roots = 0;
    let mut rest = body;
    while let Some(i) = rest.find('<') {
        let text = &rest[..i];
        if stack.is_empty() && !text.trim().is_empty() {
            return Err("text outside the root".into());
        }
        check_text(text)?;
        let Some(j) = rest[i..].find('>') else {
            return Err("a tag is not closed".into());
        };
        let t = &rest[i + 1..i + j];
        if let Some(name) = t.strip_prefix('/') {
            match stack.pop() {
                Some(open) if open == name => {}
                other => return Err(format!("</{name}> does not close {other:?}")),
            }
        } else if t.ends_with('/') {
            if stack.is_empty() {
                roots += 1;
            }
        } else {
            if stack.is_empty() {
                roots += 1;
            }
            let name = t.split_whitespace().next().unwrap_or("");
            if name.is_empty() {
                return Err("an empty tag".into());
            }
            stack.push(name.to_string());
        }
        rest = &rest[i + j + 1..];
    }
    check_text(rest)?;
    if !stack.is_empty() {
        return Err(format!("{:?} is never closed", stack.last()));
    }
    if roots != 1 {
        return Err(format!("{roots} root elements"));
    }
    Ok(())
}

fn check_text(t: &str) -> Result<(), String> {
    let mut rest = t;
    while let Some(i) = rest.find('&') {
        let tail = &rest[i..];
        let ok = ["&amp;", "&lt;", "&gt;", "&quot;", "&apos;"]
            .iter()
            .any(|e| tail.starts_with(e));
        if !ok {
            return Err("a stray &".into());
        }
        rest = &tail[1..];
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::model::WallKind;

    const SQ: [(f64, f64); 4] = [(0.0, 0.0), (240.0, 0.0), (240.0, 180.0), (0.0, 180.0)];

    /// A 20' x 15' two-story box: two exterior walls get windows, the
    /// front door is on the south wall.
    fn two_story() -> Project {
        let mut p = Project::new("Two Story");
        p.floors
            .push(plan_core::model::Floor::new("2nd Floor", 109.0));
        for fi in 0..2 {
            for i in 0..4 {
                let a = SQ[i];
                let b = SQ[(i + 1) % 4];
                p.add_wall(
                    fi,
                    Point::new(a.0, a.1),
                    Point::new(b.0, b.1),
                    6.0,
                    96.0,
                    WallKind::Exterior,
                );
            }
        }
        let south = p.floors[0].walls[0].id;
        let east = p.floors[0].walls[1].id;
        let north0 = p.floors[0].walls[2].id;
        p.add_opening(0, south, 120.0, OpeningKind::Door).unwrap();
        p.add_opening(0, east, 90.0, OpeningKind::Window).unwrap();
        p.add_opening(0, north0, 60.0, OpeningKind::Window).unwrap();
        p.add_opening(0, north0, 180.0, OpeningKind::Window)
            .unwrap();
        let north1 = p.floors[1].walls[2].id;
        p.add_opening(1, north1, 120.0, OpeningKind::Window)
            .unwrap();
        p.info.client_name = "Jane & Joe <Smith>".into();
        p.info.designer = "Dan".into();
        p.info.company = "DAD".into();
        p
    }

    #[test]
    fn orientation_buckets_are_quarters_around_the_compass() {
        use Orientation::*;
        for (az, want) in [
            (0.0, North),
            (44.0, North),
            (45.0, North),
            (46.0, East),
            (90.0, East),
            (134.0, East),
            (136.0, South),
            (180.0, South),
            (224.0, South),
            (226.0, West),
            (270.0, West),
            (314.0, West),
            (316.0, North),
            (359.0, North),
            (-10.0, North),
            (400.0, North),
        ] {
            assert_eq!(Orientation::of_azimuth(az), want, "{az}");
        }
        // Up on the screen is north with no North Pointer; a pointer turned
        // 90 degrees makes the right side of the plan north.
        assert_eq!(
            Orientation::of_plan_vector(Point::new(0.0, 1.0), 0.0),
            North
        );
        assert_eq!(
            Orientation::of_plan_vector(Point::new(0.0, -1.0), 0.0),
            South
        );
        assert_eq!(Orientation::of_plan_vector(Point::new(1.0, 0.0), 0.0), East);
        assert_eq!(
            Orientation::of_plan_vector(Point::new(1.0, 0.0), 90.0),
            North
        );
        assert_eq!(
            Orientation::of_plan_vector(Point::new(0.0, 1.0), 90.0),
            West
        );
    }

    #[test]
    fn the_envelope_of_a_box_has_four_walls_a_floor_and_a_ceiling() {
        let p = two_story();
        let env = envelope(&p, &Inputs::default(), &Options::default());
        // Both floors are conditioned: 2 x 20' x 15' interior.
        assert!(env.conditioned_floor_area > 500.0 && env.conditioned_floor_area < 620.0);
        assert_eq!(env.of(Kind::Wall, None).len(), 8);
        // Each wall is 20' or 15' long and 8' (96") tall.
        let south = env.area(Kind::Wall, Some(Orientation::South));
        assert!((south - 2.0 * 20.0 * 8.0).abs() < 1e-6, "{south}");
        let east = env.area(Kind::Wall, Some(Orientation::East));
        assert!((east - 2.0 * 15.0 * 8.0).abs() < 1e-6, "{east}");
        // The ground floor is a floor platform, the top floor a ceiling; the
        // ceiling of floor 1 and the floor of floor 2 are inside.
        assert_eq!(env.of(Kind::FloorPlatform, None).len(), 1);
        assert_eq!(env.of(Kind::FloorPlatform, None)[0].floor, Some(0));
        assert_eq!(env.of(Kind::CeilingPlatform, None).len(), 1);
        assert_eq!(env.of(Kind::CeilingPlatform, None)[0].floor, Some(1));
        // Doors and windows take the direction of their wall.
        assert_eq!(env.of(Kind::Door, Some(Orientation::South)).len(), 1);
        assert_eq!(env.of(Kind::Window, Some(Orientation::North)).len(), 3);
        assert_eq!(env.of(Kind::Window, Some(Orientation::East)).len(), 1);
        assert_eq!(env.front, Orientation::South);
        let windows = env.area(Kind::Window, None);
        let want = (4.0 * 36.0 * 60.0) / 144.0;
        assert!((windows - want).abs() < 1e-6, "{windows} vs {want}");
    }

    #[test]
    fn a_north_angle_turns_the_directions() {
        let mut p = two_story();
        p.terrain = Some(serde_json::json!({ "terrain": { "north_angle": 90.0 } }));
        let env = envelope(&p, &Inputs::default(), &Options::default());
        // 90 degrees: the right side of the plan is north.
        assert!((env.area(Kind::Wall, Some(Orientation::North)) - 2.0 * 15.0 * 8.0).abs() < 1e-6);
        assert!((env.area(Kind::Wall, Some(Orientation::West)) - 2.0 * 20.0 * 8.0).abs() < 1e-6);
        // North is to the right, so the bottom of the screen faces east.
        assert_eq!(env.front, Orientation::East);
        assert_eq!(north_angle_of(&p), 90.0);
    }

    #[test]
    fn unconditioned_rooms_and_their_walls_follow_the_room_setting() {
        let mut p = two_story();
        // Floor 1 is a garage: unconditioned.
        p.floors[0].room_names.push(plan_core::model::RoomName {
            anchor: Point::new(120.0, 90.0),
            name: "Garage".into(),
            room_type: "Garage".into(),
            conditioned: Some(false),
            ..Default::default()
        });
        let env = envelope(&p, &Inputs::default(), &Options::default());
        // Floor 1 has no envelope walls any more; the second floor's floor
        // platform is now over unconditioned space.
        assert!(env.of(Kind::Wall, None).iter().all(|c| c.floor == Some(1)));
        assert_eq!(env.of(Kind::Wall, None).len(), 4);
        assert_eq!(env.of(Kind::FloorPlatform, None)[0].floor, Some(1));
        assert!(env.of(Kind::Door, None).is_empty());
    }

    #[test]
    fn wall_r_values_come_from_the_wall_type_layers() {
        use plan_core::defaults::{WallLayer, WallTypeDef};
        let def = WallTypeDef {
            props: Default::default(),
            name: "Insulated-6".into(),
            kind: WallKind::Exterior,
            layers: vec![
                WallLayer::new("Foam Sheathing", 1.0, false, "XPS Foam"),
                WallLayer::new("Insulation", 5.5, true, "Fiberglass Batt"),
                WallLayer::new("Drywall", 0.5, false, "Drywall"),
            ],
        };
        assert_eq!(wall_r_values(Some(&def), 0.0), (19.25, 5.0));
        assert_eq!(wall_r_values(None, 13.0), (13.0, 0.0));
        let plain = WallTypeDef {
            props: Default::default(),
            layers: vec![WallLayer::new("Framing", 5.5, true, "Fir Framing")],
            ..def
        };
        assert_eq!(wall_r_values(Some(&plain), 13.0), (13.0, 0.0));
    }

    #[test]
    fn grouping_adds_like_walls_and_openings() {
        let p = two_story();
        let env = envelope(&p, &Inputs::default(), &Options::default());
        let all = Options::default();
        let g = grouped(&env, &all);
        // Four directions of wall across the floors; windows grouped by
        // direction; one door.
        assert_eq!(g.iter().filter(|c| c.kind == Kind::Wall).count(), 4);
        let north_windows: Vec<_> = g
            .iter()
            .filter(|c| c.kind == Kind::Window && c.orientation == Some(Orientation::North))
            .collect();
        assert_eq!(north_windows.len(), 1);
        assert_eq!(north_windows[0].count, 3);
        // Walls per floor when Group Similar Walls is off.
        let per_floor = Options {
            group_walls: false,
            ..Options::default()
        };
        assert_eq!(
            grouped(&env, &per_floor)
                .iter()
                .filter(|c| c.kind == Kind::Wall)
                .count(),
            8
        );
        // Each window on its own when Group Similar Doors/Windows is off.
        let single = Options {
            group_openings: false,
            ..Options::default()
        };
        assert_eq!(
            grouped(&env, &single)
                .iter()
                .filter(|c| c.kind == Kind::Window)
                .count(),
            4
        );
        // Grouping never changes the totals.
        let total = |v: &[Component], k: Kind| {
            v.iter()
                .filter(|c| c.kind == k)
                .map(|c| c.area_sq_ft)
                .sum::<f64>()
        };
        assert!((total(&g, Kind::Wall) - env.area(Kind::Wall, None)).abs() < 1e-6);
        assert!((total(&g, Kind::Window) - env.area(Kind::Window, None)).abs() < 1e-6);
    }

    #[test]
    fn wall_labels_survive_grouping_only_when_identical() {
        let mut p = two_story();
        for fi in 0..2 {
            p.floors[fi].walls[0].extras.label_text = Some("Front".into());
        }
        p.floors[0].walls[1].extras.label_text = Some("Side A".into());
        p.floors[1].walls[1].extras.label_text = Some("Side B".into());
        let env = envelope(&p, &Inputs::default(), &Options::default());
        let g = grouped(&env, &Options::default());
        let label_of = |o: Orientation| {
            g.iter()
                .find(|c| c.kind == Kind::Wall && c.orientation == Some(o))
                .unwrap()
                .label
                .clone()
        };
        assert_eq!(label_of(Orientation::South), "Front");
        assert_eq!(label_of(Orientation::East), "");
    }

    #[test]
    fn the_csv_has_a_row_per_component_and_totals_that_match() {
        let p = two_story();
        let env = envelope(&p, &Inputs::default(), &Options::default());
        let csv = thermal_csv(&env);
        let lines: Vec<&str> = csv.lines().collect();
        assert!(lines[0].starts_with("Floor Level,Component,Assembly,Direction"));
        let body = &lines[1..];
        assert_eq!(
            body.iter()
                .filter(|l| l.starts_with("1st Floor,Wall,"))
                .count(),
            4
        );
        assert_eq!(
            body.iter()
                .filter(|l| l.starts_with("2nd Floor,Window,"))
                .count(),
            1
        );
        let total_row = body
            .iter()
            .find(|l| l.starts_with("Total,Wall,") && l.contains(",South,"))
            .unwrap();
        let area: f64 = total_row.split(',').nth(4).unwrap().parse().unwrap();
        assert!((area - env.area(Kind::Wall, Some(Orientation::South))).abs() < 0.01);
        let windows = body
            .iter()
            .filter(|l| l.starts_with("Total,Window,"))
            .map(|l| l.split(',').nth(4).unwrap().parse::<f64>().unwrap())
            .sum::<f64>();
        assert!((windows - env.area(Kind::Window, None)).abs() < 0.02);
        // Commas and quotes in names are quoted.
        assert_eq!(csv_field("Wall, \"A\""), "\"Wall, \"\"A\"\"\"");
    }

    #[test]
    fn the_rxl_is_well_formed_and_carries_the_project_and_envelope() {
        let p = two_story();
        let xml = to_rxl(&p, &Inputs::default(), &Options::default());
        well_formed(&xml).unwrap();
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<REScheck>\n"));
        assert!(xml.contains("<ProjectType>New Construction</ProjectType>"));
        assert!(xml.contains("<BuildingType>1-and-2 Family, Detached</BuildingType>"));
        assert!(xml.contains("<FrontFaces>South</FrontFaces>"));
        // The owner's name is escaped.
        assert!(xml.contains("<Name>Jane &amp; Joe &lt;Smith&gt;</Name>"));
        assert!(xml.contains("<Company>DAD</Company>"));
        assert_eq!(xml.matches("<Wall>").count(), 4);
        // Three north windows are one item, the east window another.
        assert_eq!(xml.matches("<Window>").count(), 2);
        assert_eq!(xml.matches("<Door>").count(), 1);
        // Location and permit data are not exported.
        assert!(!xml.contains("Permit") && !xml.contains("Location"));
        // Ungrouped, every window and wall stands alone.
        let alone = to_rxl(
            &p,
            &Inputs::default(),
            &Options {
                group_walls: false,
                group_openings: false,
                ..Options::default()
            },
        );
        well_formed(&alone).unwrap();
        assert_eq!(alone.matches("<Wall>").count(), 8);
        assert_eq!(alone.matches("<Window>").count(), 4);
    }

    #[test]
    fn the_rxl_matches_a_golden_file_for_a_one_wall_house() {
        // One conditioned box, no doors or windows, default insulation.
        let mut p = Project::new("Tiny");
        for i in 0..4 {
            let (a, b) = (SQ[i], SQ[(i + 1) % 4]);
            p.add_wall(
                0,
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                6.0,
                96.0,
                WallKind::Exterior,
            );
        }
        let xml = to_rxl(&p, &Inputs::default(), &Options::default());
        let env = envelope(&p, &Inputs::default(), &Options::default());
        let area = num(env.conditioned_floor_area);
        let golden = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<REScheck>
  <Project>
    <ProjectType>New Construction</ProjectType>
    <BuildingType>1-and-2 Family, Detached</BuildingType>
    <FrontFaces>South</FrontFaces>
    <ConditionedFloorArea>{area}</ConditionedFloorArea>
    <OwnerAgent>
      <Name></Name>
      <Address></Address>
      <Phone></Phone>
      <Email></Email>
    </OwnerAgent>
    <DesignerContractor>
      <Name></Name>
      <Company></Company>
    </DesignerContractor>
  </Project>
  <Envelope>
    <Floors>
      <Floor>
        <Assembly>Floor Platform</Assembly>
        <GrossArea>{area}</GrossArea>
        <CavityR>0</CavityR>
        <ContinuousR>0</ContinuousR>
      </Floor>
    </Floors>
    <Slabs>
    </Slabs>
    <Ceilings>
      <Ceiling>
        <Assembly>Ceiling Structure</Assembly>
        <GrossArea>{area}</GrossArea>
        <CavityR>0</CavityR>
        <ContinuousR>0</ContinuousR>
      </Ceiling>
    </Ceilings>
    <Walls>
      <Wall>
        <Assembly>Exterior Wall</Assembly>
        <Orientation>North</Orientation>
        <GrossArea>{n}</GrossArea>
        <CavityR>0</CavityR>
        <ContinuousR>0</ContinuousR>
        <OnCenterSpacing>16</OnCenterSpacing>
      </Wall>
      <Wall>
        <Assembly>Exterior Wall</Assembly>
        <Orientation>East</Orientation>
        <GrossArea>{e}</GrossArea>
        <CavityR>0</CavityR>
        <ContinuousR>0</ContinuousR>
        <OnCenterSpacing>16</OnCenterSpacing>
      </Wall>
      <Wall>
        <Assembly>Exterior Wall</Assembly>
        <Orientation>South</Orientation>
        <GrossArea>{n}</GrossArea>
        <CavityR>0</CavityR>
        <ContinuousR>0</ContinuousR>
        <OnCenterSpacing>16</OnCenterSpacing>
      </Wall>
      <Wall>
        <Assembly>Exterior Wall</Assembly>
        <Orientation>West</Orientation>
        <GrossArea>{e}</GrossArea>
        <CavityR>0</CavityR>
        <ContinuousR>0</ContinuousR>
        <OnCenterSpacing>16</OnCenterSpacing>
      </Wall>
    </Walls>
    <Doors>
    </Doors>
    <Windows>
    </Windows>
  </Envelope>
</REScheck>
",
            n = num(20.0 * 8.0),
            e = num(15.0 * 8.0)
        );
        // The walls come in drawing order: south, east, north, west.
        let sorted = {
            let mut lines: Vec<&str> = xml.lines().collect();
            lines.sort_unstable();
            lines.join("\n")
        };
        let mut gold_lines: Vec<&str> = golden.lines().collect();
        gold_lines.sort_unstable();
        assert_eq!(sorted, gold_lines.join("\n"), "{xml}");
        well_formed(&xml).unwrap();
    }

    #[test]
    fn well_formed_catches_broken_xml() {
        assert!(well_formed("<a><b></a></b>").is_err());
        assert!(well_formed("<a>").is_err());
        assert!(well_formed("<a>x & y</a>").is_err());
        assert!(well_formed("<a/><b/>").is_err());
        assert!(well_formed("<a>x &amp; y<b/></a>").is_ok());
    }
}
