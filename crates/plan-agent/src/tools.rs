//! The tool catalog the model sees and the executor that applies tool calls
//! to a `&mut Project`.
//!
//! Units: tool inputs and outputs use FEET (decimal), room areas are square
//! feet; the plan stores inches and the executor converts. Floors are 1-based
//! in the tools. Every tool either returns a JSON value or an error string
//! (which becomes a `tool_result` with `is_error`); a failed tool never
//! changes the project.

use crate::prompt;
use crate::session::Parameter;
use plan_core::defaults::PlanDefaults;
use plan_core::floors::{FoundationKind, FoundationOptions, FoundationRooms};
use plan_core::geometry::{dist_to_segment, Point};
use plan_core::model::{Id, Opening, OpeningKind, Project, Wall, WallEnd, WallKind};
use plan_core::openings::{clamp_opening_center, exterior_sign, openings_conflict, OpeningStyle};
use plan_core::rooms::{apply_function_defaults, detect_rooms, function_defaults};
use plan_core::units::fmt_ft_in;
use plan_core::walls::{WallClass, MIN_WALL_THICKNESS};
use plan_core::Layer;
use serde_json::{json, Map, Value};
use std::sync::OnceLock;

/// Shortest wall a tool will draw, inches.
const MIN_WALL_LEN: f64 = 6.0;
/// Two wall ends closer than this are one corner, inches.
const JOIN_TOL: f64 = 0.5;
/// Clear distance between an opening jamb and a wall end or a neighbour, inches.
const OPENING_MARGIN: f64 = 2.0;
/// Coordinates beyond this many feet are rejected as mistakes.
const MAX_COORD_FT: f64 = 5000.0;
/// Most parameters one session may define.
const MAX_PARAMETERS: usize = 24;

// ----- the catalog -----

/// One tool as the API wants it.
#[derive(Clone, Debug)]
pub struct ToolDef {
    pub name: &'static str,
    pub description: String,
    pub input_schema: Value,
}

impl ToolDef {
    /// The JSON for the request's `tools` array: strict, with eager input streaming.
    pub fn to_api_json(&self) -> Value {
        json!({
            "name": self.name,
            "description": self.description,
            "strict": true,
            "input_schema": self.input_schema,
            "eager_input_streaming": true,
        })
    }
}

fn s_num(desc: &str) -> Value {
    json!({"type": "number", "description": desc})
}
fn s_opt_num(desc: &str) -> Value {
    json!({"type": ["number", "null"], "description": desc})
}
fn s_int(desc: &str) -> Value {
    json!({"type": "integer", "description": desc})
}
fn s_str(desc: &str) -> Value {
    json!({"type": "string", "description": desc})
}
fn s_opt_str(desc: &str) -> Value {
    json!({"type": ["string", "null"], "description": desc})
}
fn s_bool(desc: &str) -> Value {
    json!({"type": "boolean", "description": desc})
}
fn s_enum(values: &[&str], desc: &str) -> Value {
    json!({"type": "string", "enum": values, "description": desc})
}
fn s_point(desc: &str) -> Value {
    json!({
        "type": "array",
        "items": {"type": "number"},
        "description": format!("[x, y] in feet. {desc}"),
    })
}
fn s_floor() -> Value {
    s_int("Floor number, 1-based, as listed by get_plan_summary.")
}

/// A strict tool: every property is required, none may be added.
fn tool(name: &'static str, description: &str, props: Vec<(&str, Value)>) -> ToolDef {
    let required: Vec<&str> = props.iter().map(|(k, _)| *k).collect();
    let properties: Map<String, Value> =
        props.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    ToolDef {
        name,
        description: description.to_string(),
        input_schema: json!({
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        }),
    }
}

const WALL_KINDS: [&str; 5] = [
    "exterior",
    "interior",
    "railing",
    "room_divider",
    "foundation",
];

/// The tools, in a fixed order (the order is part of the cached prefix).
pub fn catalog() -> Vec<ToolDef> {
    let kind = || {
        s_enum(
            &WALL_KINDS,
            "Wall kind. exterior and interior take the plan's default wall types; room_divider is invisible and only closes a room.",
        )
    };
    vec![
        tool(
            "get_plan_summary",
            "The plan as text (floors, rooms with areas, footprint, defaults, parameters) plus every floor's walls. Use it to re-read the plan after edits.",
            vec![],
        ),
        tool(
            "list_walls",
            "Walls of a floor: id, start and end points [x, y] in feet, length_ft, thickness_in, kind and the ids of the openings in each wall.",
            vec![("floor", s_floor())],
        ),
        tool(
            "list_rooms",
            "Rooms detected on a floor (closed wall loops): index, name, type, area_sqft (to wall centerlines), interior_area_sqft and bbox [min_x, min_y, max_x, max_y] in feet.",
            vec![("floor", s_floor())],
        ),
        tool(
            "list_openings",
            "Doors and windows of a floor: id, wall_id, kind, center_offset_ft (from the wall start), width_ft, height_ft, sill_ft, style and label.",
            vec![("floor", s_floor())],
        ),
        tool(
            "add_wall",
            "Draw one straight wall from start to end. The exterior face of an exterior wall is on the LEFT of start-to-end (draw exterior outlines clockwise with y up). Ends that share coordinates are connected. Returns the new wall's id and length.",
            vec![
                ("floor", s_floor()),
                ("start", s_point("Start point.")),
                ("end", s_point("End point.")),
                ("kind", kind()),
                ("thickness_in", s_opt_num("Wall thickness in inches, or null for the plan default of this kind.")),
            ],
        ),
        tool(
            "add_wall_rectangle",
            "Draw four connected walls around a rectangle whose lower-left corner is (x, y), width along +x and depth along +y. Walls run counter-clockwise; for exterior walls the exterior face is outward. Returns the four wall ids (bottom, right, top, left).",
            vec![
                ("floor", s_floor()),
                ("x", s_num("Lower-left corner x, feet.")),
                ("y", s_num("Lower-left corner y, feet.")),
                ("width", s_num("Size along +x, feet.")),
                ("depth", s_num("Size along +y, feet.")),
                ("kind", kind()),
            ],
        ),
        tool(
            "move_wall_endpoint",
            "Move one end of a wall to a new point. Other wall ends at the old point follow so corners stay connected; openings are kept inside their walls.",
            vec![
                ("floor", s_floor()),
                ("id", s_int("Wall id.")),
                ("end", s_enum(&["start", "end"], "Which end to move.")),
                ("to", s_point("New position of that end.")),
            ],
        ),
        tool(
            "translate_wall",
            "Move a whole wall by (dx, dy) feet. Walls joined to its ends are stretched to stay connected.",
            vec![
                ("floor", s_floor()),
                ("id", s_int("Wall id.")),
                ("dx", s_num("Move along +x, feet.")),
                ("dy", s_num("Move along +y, feet.")),
            ],
        ),
        tool(
            "remove_wall",
            "Delete a wall and the doors and windows in it.",
            vec![("floor", s_floor()), ("id", s_int("Wall id."))],
        ),
        tool(
            "add_opening",
            "Put a door or window in a wall, centered at center_offset_ft from the wall's start (clamped to fit). Sizes default from the plan's door and window defaults. Doors on exterior walls swing inward. Returns the opening id and its label.",
            vec![
                ("floor", s_floor()),
                ("wall_id", s_int("Wall id.")),
                ("kind", s_enum(&["door", "window"], "Door or window.")),
                ("center_offset_ft", s_num("Distance from the wall start to the opening center, feet.")),
                ("width_ft", s_opt_num("Width in feet, or null for the default.")),
                ("height_ft", s_opt_num("Height in feet, or null for the default.")),
                ("style", s_opt_str("Style name such as Hinged Door, Double Door, Sliding Door, Garage Door, Doorway, Casement Window, Fixed Window, or null for the default.")),
            ],
        ),
        tool(
            "remove_opening",
            "Delete a door or window.",
            vec![("floor", s_floor()), ("id", s_int("Opening id."))],
        ),
        tool(
            "set_opening_width",
            "Change an opening's width, keeping it centered where it fits.",
            vec![
                ("floor", s_floor()),
                ("id", s_int("Opening id.")),
                ("width_ft", s_num("New width, feet.")),
            ],
        ),
        tool(
            "slide_opening",
            "Move an opening along its wall so its center is at center_offset_ft from the wall start.",
            vec![
                ("floor", s_floor()),
                ("id", s_int("Opening id.")),
                ("center_offset_ft", s_num("New center offset, feet.")),
            ],
        ),
        tool(
            "flip_swing",
            "Make a door swing to the other side of its wall.",
            vec![("floor", s_floor()), ("id", s_int("Opening id."))],
        ),
        tool(
            "flip_hinge",
            "Move a door's hinge to the other jamb, keeping the swing side.",
            vec![("floor", s_floor()), ("id", s_int("Opening id."))],
        ),
        tool(
            "set_room_name",
            "Name the room that contains a point and give it a room type from the plan's room type list.",
            vec![
                ("floor", s_floor()),
                ("point", s_point("A point inside the room.")),
                ("name", s_str("Room name, e.g. Primary Bedroom.")),
                ("room_type", s_str("A room type from the plan's list, e.g. Bedroom, Kitchen, Garage.")),
            ],
        ),
        tool(
            "build_new_floor",
            "Add a floor above the highest normal floor, optionally copying the exterior walls (with their doors and windows) of the floor below. Returns the new floor's number.",
            vec![("copy_exterior_walls", s_bool("Copy the exterior walls of the floor below."))],
        ),
        tool(
            "delete_floor",
            "Delete a floor with everything on it. The last floor cannot be deleted.",
            vec![("floor", s_floor())],
        ),
        tool(
            "build_foundation",
            "Build (or rebuild) the foundation under the exterior walls of the first floor. slab is a monolithic slab, stem_wall a crawl-space foundation wall, basement a full-height foundation, pier grade beams on piers. The foundation becomes floor 1 and the other floors move up one.",
            vec![("kind", s_enum(&["slab", "stem_wall", "basement", "pier"], "Foundation type."))],
        ),
        tool(
            "validate_plan",
            "Check every floor: rooms found, unclosed wall ends (a room-defining wall end that touches no other wall) and overlapping openings. Call it after editing and fix what it reports.",
            vec![],
        ),
        tool(
            "define_parameter",
            "Expose a number the user may want to tweak with a slider (living area, garage width, ...). Creates the parameter or replaces the one with the same name. It does not change the plan by itself; build the plan with the same value.",
            vec![
                ("name", s_str("snake_case id, e.g. garage_width_ft.")),
                ("label", s_str("Label for the slider, e.g. Garage width.")),
                ("value", s_num("Current value.")),
                ("min", s_num("Smallest value.")),
                ("max", s_num("Largest value.")),
                ("unit", s_str("ft, in, sqft, count or an empty string.")),
                ("description", s_str("One sentence on what the number controls.")),
            ],
        ),
        tool(
            "read_parameters",
            "The parameters defined so far, with their current values (the user may have moved a slider).",
            vec![],
        ),
        tool(
            "measure",
            "Distance between two points: feet and a feet-and-inches text.",
            vec![
                ("from", s_point("First point.")),
                ("to", s_point("Second point.")),
            ],
        ),
    ]
}

/// The `tools` array of every request, built once so it is byte-identical
/// between calls (prompt caching is prefix based).
pub fn catalog_json() -> &'static [Value] {
    static CATALOG: OnceLock<Vec<Value>> = OnceLock::new();
    CATALOG.get_or_init(|| catalog().iter().map(ToolDef::to_api_json).collect())
}

// ----- argument access -----

struct Args<'a>(&'a Map<String, Value>);

impl Args<'_> {
    fn get(&self, key: &str) -> Result<&Value, String> {
        self.0
            .get(key)
            .ok_or_else(|| format!("Missing required input `{key}`."))
    }

    fn num(&self, key: &str) -> Result<f64, String> {
        let v = self
            .get(key)?
            .as_f64()
            .ok_or_else(|| format!("`{key}` must be a number."))?;
        if v.is_finite() {
            Ok(v)
        } else {
            Err(format!("`{key}` must be a finite number."))
        }
    }

    fn opt_num(&self, key: &str) -> Result<Option<f64>, String> {
        match self.0.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(_) => self.num(key).map(Some),
        }
    }

    fn int(&self, key: &str) -> Result<i64, String> {
        let v = self.get(key)?;
        if let Some(i) = v.as_i64() {
            return Ok(i);
        }
        match v.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 1e15 => Ok(f as i64),
            _ => Err(format!("`{key}` must be an integer.")),
        }
    }

    fn id(&self, key: &str) -> Result<Id, String> {
        let i = self.int(key)?;
        if i < 0 {
            return Err(format!("`{key}` must be a non-negative id."));
        }
        Ok(i as Id)
    }

    fn str(&self, key: &str) -> Result<&str, String> {
        self.get(key)?
            .as_str()
            .ok_or_else(|| format!("`{key}` must be a string."))
    }

    fn opt_str(&self, key: &str) -> Result<Option<&str>, String> {
        match self.0.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(_) => self.str(key).map(Some),
        }
    }

    fn bool(&self, key: &str) -> Result<bool, String> {
        self.get(key)?
            .as_bool()
            .ok_or_else(|| format!("`{key}` must be true or false."))
    }

    /// A feet coordinate, checked for sanity.
    fn coord_ft(&self, key: &str) -> Result<f64, String> {
        let v = self.num(key)?;
        check_coord(key, v)?;
        Ok(v)
    }

    /// `[x, y]` in feet, as a point in inches.
    fn point(&self, key: &str) -> Result<Point, String> {
        let arr = self
            .get(key)?
            .as_array()
            .filter(|a| a.len() == 2)
            .ok_or_else(|| format!("`{key}` must be an array of two numbers [x, y] in feet."))?;
        let x = arr[0]
            .as_f64()
            .ok_or_else(|| format!("`{key}` must contain numbers."))?;
        let y = arr[1]
            .as_f64()
            .ok_or_else(|| format!("`{key}` must contain numbers."))?;
        check_coord(key, x)?;
        check_coord(key, y)?;
        Ok(Point::new(x * 12.0, y * 12.0))
    }

    fn floor(&self, project: &Project) -> Result<usize, String> {
        let n = self.int("floor")?;
        if n < 1 || n as usize > project.floors.len() {
            return Err(format!(
                "Floor {n} does not exist. The plan has {} floor(s): {}.",
                project.floors.len(),
                floor_names(project)
            ));
        }
        Ok(n as usize - 1)
    }
}

fn check_coord(key: &str, v: f64) -> Result<(), String> {
    if !v.is_finite() || v.abs() > MAX_COORD_FT {
        return Err(format!(
            "`{key}` has a coordinate of {v} ft, which is out of range (limit {MAX_COORD_FT} ft)."
        ));
    }
    Ok(())
}

fn floor_names(project: &Project) -> String {
    project
        .floors
        .iter()
        .enumerate()
        .map(|(i, f)| format!("{} = \"{}\"", i + 1, f.name))
        .collect::<Vec<_>>()
        .join(", ")
}

fn r3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

/// Inches to feet, rounded to three decimals.
fn ft(inches: f64) -> f64 {
    r3(inches / 12.0)
}

fn pt_json(p: Point) -> Value {
    json!([ft(p.x), ft(p.y)])
}

fn end_pos(w: &Wall, e: WallEnd) -> Point {
    match e {
        WallEnd::Start => w.start,
        WallEnd::End => w.end,
    }
}

fn end_name(e: WallEnd) -> &'static str {
    match e {
        WallEnd::Start => "start",
        WallEnd::End => "end",
    }
}

fn wall_kind_name(w: &Wall) -> &'static str {
    if w.is_foundation() {
        return "foundation";
    }
    match &w.class {
        WallClass::Standard => match w.kind {
            WallKind::Exterior => "exterior",
            WallKind::Interior => "interior",
        },
        WallClass::Foundation => "foundation",
        WallClass::Pony { .. } => "pony_wall",
        WallClass::Glass => "glass_wall",
        WallClass::GlassPony { .. } => "glass_pony_wall",
        WallClass::HalfWall { .. } => "half_wall",
        WallClass::RoomDivider => "room_divider",
        WallClass::Railing | WallClass::DeckRailing => "railing",
        WallClass::DeckEdge => "deck_edge",
        WallClass::Fencing { .. } => "fence",
    }
}

fn wall_json(project: &Project, fi: usize, w: &Wall) -> Value {
    let openings: Vec<Id> = project.floors[fi].openings_on(w.id).map(|o| o.id).collect();
    json!({
        "id": w.id,
        "start": pt_json(w.start),
        "end": pt_json(w.end),
        "length_ft": ft(w.length()),
        "thickness_in": r3(w.thickness),
        "kind": wall_kind_name(w),
        "openings": openings,
    })
}

fn opening_json(o: &Opening) -> Value {
    json!({
        "id": o.id,
        "wall_id": o.wall_id,
        "kind": match o.kind { OpeningKind::Door => "door", OpeningKind::Window => "window" },
        "center_offset_ft": ft(o.center_offset),
        "width_ft": ft(o.width),
        "height_ft": ft(o.height),
        "sill_ft": ft(o.sill_height),
        "style": o.type_name(),
        "label": o.label(),
    })
}

// ----- the executor -----

/// Runs tool `name` with `input` against `project`. Errors are messages for
/// the model; a failed call leaves the project untouched.
pub fn execute(
    name: &str,
    input: &Value,
    project: &mut Project,
    defaults: &PlanDefaults,
    params: &mut Vec<Parameter>,
) -> Result<Value, String> {
    let Some(map) = input.as_object() else {
        return Err("The tool input must be a JSON object.".to_string());
    };
    let a = Args(map);
    match name {
        "get_plan_summary" => Ok(get_plan_summary(project, defaults, params)),
        "list_walls" => {
            let fi = a.floor(project)?;
            let walls: Vec<Value> = project.floors[fi]
                .walls
                .iter()
                .map(|w| wall_json(project, fi, w))
                .collect();
            Ok(json!(walls))
        }
        "list_rooms" => {
            let fi = a.floor(project)?;
            Ok(json!(rooms_json(project, fi)))
        }
        "list_openings" => {
            let fi = a.floor(project)?;
            let out: Vec<Value> = project.floors[fi]
                .openings
                .iter()
                .map(opening_json)
                .collect();
            Ok(json!(out))
        }
        "add_wall" => add_wall(&a, project, defaults),
        "add_wall_rectangle" => add_wall_rectangle(&a, project, defaults),
        "move_wall_endpoint" => move_wall_endpoint(&a, project),
        "translate_wall" => translate_wall(&a, project),
        "remove_wall" => {
            let fi = a.floor(project)?;
            let id = a.id("id")?;
            require_wall(project, fi, id)?;
            let openings = project.floors[fi].openings_on(id).count();
            project.remove_wall(fi, id);
            Ok(json!({"removed_wall": id, "removed_openings": openings}))
        }
        "add_opening" => add_opening(&a, project, defaults),
        "remove_opening" => {
            let fi = a.floor(project)?;
            let id = a.id("id")?;
            require_opening(project, fi, id)?;
            project.remove_opening(fi, id);
            Ok(json!({"removed_opening": id}))
        }
        "set_opening_width" => {
            let fi = a.floor(project)?;
            let id = a.id("id")?;
            require_opening(project, fi, id)?;
            let w = a.num("width_ft")?;
            if !(0.5..=60.0).contains(&w) {
                return Err("`width_ft` must be between 0.5 and 60 ft.".to_string());
            }
            if !project.set_opening_width(fi, id, w * 12.0) {
                return Err(format!(
                    "Opening {id} cannot be {w} ft wide: it does not fit between its neighbours and the wall ends."
                ));
            }
            Ok(opening_json(find_opening(project, fi, id)?))
        }
        "slide_opening" => {
            let fi = a.floor(project)?;
            let id = a.id("id")?;
            require_opening(project, fi, id)?;
            let c = a.coord_ft("center_offset_ft")?;
            if !project.slide_opening(fi, id, c * 12.0) {
                return Err(format!(
                    "Opening {id} cannot slide there: the wall is too short or another opening is in the way."
                ));
            }
            Ok(opening_json(find_opening(project, fi, id)?))
        }
        "flip_swing" => {
            let fi = a.floor(project)?;
            let id = a.id("id")?;
            require_door(project, fi, id)?;
            project.flip_swing(fi, id);
            Ok(opening_json(find_opening(project, fi, id)?))
        }
        "flip_hinge" => {
            let fi = a.floor(project)?;
            let id = a.id("id")?;
            require_door(project, fi, id)?;
            project.flip_hinge(fi, id);
            Ok(opening_json(find_opening(project, fi, id)?))
        }
        "set_room_name" => set_room_name(&a, project, defaults),
        "build_new_floor" => {
            let copy = a.bool("copy_exterior_walls")?;
            let idx = project.build_new_floor(copy);
            Ok(json!({"floor_number": idx + 1, "name": project.floors[idx].name}))
        }
        "delete_floor" => {
            let fi = a.floor(project)?;
            let name = project.floors[fi].name.clone();
            if !project.delete_floor(fi) {
                return Err("The last remaining floor cannot be deleted.".to_string());
            }
            Ok(json!({"deleted_floor": fi + 1, "name": name, "floors": floor_names(project)}))
        }
        "build_foundation" => build_foundation(&a, project),
        "validate_plan" => Ok(validate_plan(project)),
        "define_parameter" => define_parameter(&a, params),
        "read_parameters" => Ok(json!(params.iter().map(param_json).collect::<Vec<_>>())),
        "measure" => {
            let from = a.point("from")?;
            let to = a.point("to")?;
            let d = from.dist(to);
            Ok(json!({"distance_ft": ft(d), "text": fmt_ft_in(d)}))
        }
        other => {
            let names: Vec<&str> = catalog().iter().map(|t| t.name).collect();
            Err(format!(
                "Unknown tool `{other}`. Available tools: {}.",
                names.join(", ")
            ))
        }
    }
}

fn param_json(p: &Parameter) -> Value {
    json!({
        "name": p.name, "label": p.label, "value": p.value, "min": p.min,
        "max": p.max, "unit": p.unit, "description": p.description,
    })
}

fn find_opening(project: &Project, fi: usize, id: Id) -> Result<&Opening, String> {
    project.floors[fi]
        .openings
        .iter()
        .find(|o| o.id == id)
        .ok_or_else(|| format!("No opening with id {id} on floor {}.", fi + 1))
}

fn require_opening(project: &Project, fi: usize, id: Id) -> Result<(), String> {
    find_opening(project, fi, id).map(|_| ())
}

fn require_door(project: &Project, fi: usize, id: Id) -> Result<(), String> {
    if find_opening(project, fi, id)?.kind != OpeningKind::Door {
        return Err(format!("Opening {id} is a window; only doors swing."));
    }
    Ok(())
}

fn require_wall(project: &Project, fi: usize, id: Id) -> Result<(), String> {
    if project.floors[fi].wall(id).is_none() {
        return Err(format!(
            "No wall with id {id} on floor {}. Call list_walls to see the ids.",
            fi + 1
        ));
    }
    Ok(())
}

fn rooms_json(project: &Project, fi: usize) -> Vec<Value> {
    let f = &project.floors[fi];
    detect_rooms(&f.walls, JOIN_TOL)
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let (name, ty) = match r.name_entry(&f.room_names) {
                Some(n) => (n.name.clone(), n.room_type.clone()),
                None => (String::new(), String::new()),
            };
            let bbox = r.polygon.iter().fold(None, |bb: Option<[f64; 4]>, p| {
                Some(match bb {
                    None => [p.x, p.y, p.x, p.y],
                    Some([a, b, c, d]) => [a.min(p.x), b.min(p.y), c.max(p.x), d.max(p.y)],
                })
            });
            json!({
                "index": i,
                "name": name,
                "type": ty,
                "area_sqft": r3(r.area_sq_ft()),
                "interior_area_sqft": r3(r.interior_area_sq_ft()),
                "bbox": bbox.map(|b| vec![ft(b[0]), ft(b[1]), ft(b[2]), ft(b[3])]),
            })
        })
        .collect()
}

fn get_plan_summary(project: &Project, defaults: &PlanDefaults, params: &[Parameter]) -> Value {
    let floors: Vec<Value> = project
        .floors
        .iter()
        .enumerate()
        .map(|(fi, f)| {
            json!({
                "floor": fi + 1,
                "name": f.name,
                "walls": f.walls.iter().map(|w| wall_json(project, fi, w)).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "summary": prompt::plan_summary_with_params(project, defaults, params),
        "floors": floors,
    })
}

// ----- walls -----

/// What a wall `kind` of the tools draws, from the plan defaults.
struct WallSpec {
    kind: WallKind,
    class: WallClass,
    wall_type: String,
    height: f64,
    thickness: f64,
}

fn wall_spec(kind: &str, d: &PlanDefaults) -> Result<WallSpec, String> {
    let v = &d.wall_variants;
    let thick = |name: &str, fallback: f64| d.wall_type(name).map_or(fallback, |t| t.thickness());
    Ok(match kind {
        "exterior" => WallSpec {
            kind: WallKind::Exterior,
            class: WallClass::Standard,
            wall_type: d.exterior_wall.wall_type.clone(),
            height: d.exterior_wall.height,
            thickness: d.exterior_thickness(),
        },
        "interior" => WallSpec {
            kind: WallKind::Interior,
            class: WallClass::Standard,
            wall_type: d.interior_wall.wall_type.clone(),
            height: d.interior_wall.height,
            thickness: d.interior_thickness(),
        },
        "foundation" => WallSpec {
            kind: WallKind::Exterior,
            class: WallClass::Foundation,
            wall_type: d.foundation_wall.wall_type.clone(),
            height: d.foundation_wall.height,
            thickness: d.foundation_thickness(),
        },
        "railing" => WallSpec {
            kind: WallKind::Interior,
            class: WallClass::Railing,
            wall_type: v.railing_type.clone(),
            height: v.railing_height,
            thickness: thick(&v.railing_type, d.interior_thickness()),
        },
        "room_divider" => WallSpec {
            kind: WallKind::Interior,
            class: WallClass::RoomDivider,
            wall_type: String::new(),
            height: d.interior_wall.height,
            thickness: MIN_WALL_THICKNESS,
        },
        other => {
            return Err(format!(
                "Unknown wall kind `{other}`. Use one of: {}.",
                WALL_KINDS.join(", ")
            ))
        }
    })
}

/// The layer a wall class is drawn on, with a plan color and weight (as the wall tool does).
fn class_layer(class: &WallClass) -> Option<Layer> {
    let name = class.default_layer()?;
    Some(match class {
        WallClass::RoomDivider => Layer::new(name, [150, 150, 150], 13),
        WallClass::Fencing { .. } => Layer::new(name, [90, 130, 60], 18),
        _ => Layer::new(name, [150, 100, 50], 25),
    })
}

/// One wall to create with [`new_wall`].
struct NewWall<'a> {
    fi: usize,
    start: Point,
    end: Point,
    kind: &'a str,
    thickness_in: Option<f64>,
    /// The wall is part of a counter-clockwise loop: its inside is on the
    /// left, so an exterior wall's exterior face goes to the right.
    ccw_loop: bool,
}

/// Creates a wall the way the wall tool does: defaults for the kind, the
/// default wall type, the layer of its class. `Err` before anything changes
/// when the input is unusable.
fn new_wall(project: &mut Project, d: &PlanDefaults, nw: &NewWall) -> Result<Id, String> {
    let NewWall {
        fi,
        start,
        end,
        kind,
        thickness_in,
        ccw_loop,
    } = *nw;
    let spec = wall_spec(kind, d)?;
    if start.dist(end) < MIN_WALL_LEN {
        return Err(format!(
            "A wall must be at least {} ft long (this one is {} ft).",
            MIN_WALL_LEN / 12.0,
            ft(start.dist(end))
        ));
    }
    if let Some(t) = thickness_in {
        if !(MIN_WALL_THICKNESS..=48.0).contains(&t) {
            return Err("`thickness_in` must be between 0.125 and 48 inches.".to_string());
        }
    }
    let thickness = thickness_in.unwrap_or(spec.thickness);
    let id = project.add_wall(fi, start, end, thickness, spec.height, spec.kind);
    // The default wall type of the kind (W-51), unless the thickness was overridden.
    let typed = thickness_in.is_none() && d.wall_type(&spec.wall_type).is_some();
    if typed && project.wall_type_def(&spec.wall_type).is_none() {
        if let Some(def) = d.wall_type(&spec.wall_type).cloned() {
            project.register_wall_type(def);
        }
    }
    if let Some(layer) = class_layer(&spec.class) {
        project.layers.add(layer);
    }
    let tool_layer = project.layers.tool_layer(match spec.kind {
        WallKind::Exterior => "walls_exterior",
        WallKind::Interior => "walls_interior",
    });
    if let Some(w) = project.floors[fi].wall_mut(id) {
        if typed {
            w.wall_type = Some(spec.wall_type.clone());
        }
        w.foundation_height = d.foundation_wall.height;
        match spec.class.default_layer() {
            Some(layer) => w.layer = layer.to_string(),
            None if !tool_layer.is_empty() => w.layer = tool_layer,
            None => {}
        }
        if !spec.class.is_standard() {
            w.set_class(spec.class.clone());
        }
        if ccw_loop && spec.kind == WallKind::Exterior {
            w.exterior_side = w.exterior_side.opposite();
        }
    }
    Ok(id)
}

fn add_wall(a: &Args, project: &mut Project, d: &PlanDefaults) -> Result<Value, String> {
    let fi = a.floor(project)?;
    let start = a.point("start")?;
    let end = a.point("end")?;
    let kind = a.str("kind")?;
    let thickness = a.opt_num("thickness_in")?;
    let nw = NewWall {
        fi,
        start,
        end,
        kind,
        thickness_in: thickness,
        ccw_loop: false,
    };
    let id = new_wall(project, d, &nw)?;
    let w = project.floors[fi]
        .wall(id)
        .ok_or("internal error: wall missing")?;
    Ok(json!({"id": id, "length_ft": ft(w.length())}))
}

fn add_wall_rectangle(a: &Args, project: &mut Project, d: &PlanDefaults) -> Result<Value, String> {
    let fi = a.floor(project)?;
    let x = a.coord_ft("x")?;
    let y = a.coord_ft("y")?;
    let w = a.num("width")?;
    let h = a.num("depth")?;
    let kind = a.str("kind")?;
    if w < MIN_WALL_LEN / 12.0 || h < MIN_WALL_LEN / 12.0 {
        return Err(format!(
            "Width and depth must each be at least {} ft.",
            MIN_WALL_LEN / 12.0
        ));
    }
    check_coord("width", w)?;
    check_coord("depth", h)?;
    wall_spec(kind, d)?; // validate the kind before drawing anything
    let c = [
        Point::new(x * 12.0, y * 12.0),
        Point::new((x + w) * 12.0, y * 12.0),
        Point::new((x + w) * 12.0, (y + h) * 12.0),
        Point::new(x * 12.0, (y + h) * 12.0),
    ];
    let mut ids = Vec::new();
    for i in 0..4 {
        let nw = NewWall {
            fi,
            start: c[i],
            end: c[(i + 1) % 4],
            kind,
            thickness_in: None,
            ccw_loop: true,
        };
        ids.push(new_wall(project, d, &nw)?);
    }
    let walls: Vec<Value> = ids
        .iter()
        .filter_map(|id| project.floors[fi].wall(*id))
        .map(|w| wall_json(project, fi, w))
        .collect();
    Ok(json!({
        "ids": ids,
        "walls": walls,
        "note": "Corners share end points, so the four walls are connected.",
    }))
}

/// Sets one end of a wall without touching neighbours. Openings keep their
/// distance from the end that stays put; the exterior side follows a
/// reversal of the wall's direction.
fn set_end(project: &mut Project, fi: usize, id: Id, end: WallEnd, to: Point) -> Vec<Id> {
    let f = &mut project.floors[fi];
    let Some(w) = f.wall_mut(id) else {
        return Vec::new();
    };
    let old_len = w.length();
    let old_dir = w.direction();
    match end {
        WallEnd::Start => w.start = to,
        WallEnd::End => w.end = to,
    }
    let new_len = w.length();
    if new_len > 1e-9 && old_len > 1e-9 && w.direction().dot(old_dir) < 0.0 {
        w.exterior_side = w.exterior_side.opposite();
    }
    if end == WallEnd::Start {
        for o in f.openings.iter_mut().filter(|o| o.wall_id == id) {
            o.center_offset += new_len - old_len;
        }
    }
    revalidate_openings(project, fi, id)
}

/// Clamps the openings of `wall_id` into the wall and removes the ones that
/// no longer fit. Returns the removed ids.
fn revalidate_openings(project: &mut Project, fi: usize, wall_id: Id) -> Vec<Id> {
    let f = &mut project.floors[fi];
    let Some(len) = f.wall(wall_id).map(Wall::path_length) else {
        return Vec::new();
    };
    let mut removed = Vec::new();
    f.openings.retain(|o| {
        let fits = o.wall_id != wall_id || len >= o.width + 2.0 * OPENING_MARGIN;
        if !fits {
            removed.push(o.id);
        }
        fits
    });
    for o in f.openings.iter_mut().filter(|o| o.wall_id == wall_id) {
        let half = o.width * 0.5;
        o.center_offset = o
            .center_offset
            .clamp(half + OPENING_MARGIN, len - half - OPENING_MARGIN);
    }
    removed
}

/// Wall ends within [`JOIN_TOL`] of `p`, other than wall `exclude`.
fn ends_at(project: &Project, fi: usize, p: Point, exclude: Id) -> Vec<(Id, WallEnd)> {
    let mut out = Vec::new();
    for w in &project.floors[fi].walls {
        if w.id == exclude {
            continue;
        }
        for e in [WallEnd::Start, WallEnd::End] {
            if end_pos(w, e).dist(p) <= JOIN_TOL {
                out.push((w.id, e));
            }
        }
    }
    out
}

/// A wall that would end up shorter than the minimum length.
fn check_not_collapsed(
    project: &Project,
    fi: usize,
    id: Id,
    end: WallEnd,
    to: Point,
) -> Result<(), String> {
    let w = project.floors[fi]
        .wall(id)
        .ok_or("internal error: wall missing")?;
    let other = end_pos(
        w,
        match end {
            WallEnd::Start => WallEnd::End,
            WallEnd::End => WallEnd::Start,
        },
    );
    if other.dist(to) < MIN_WALL_LEN {
        return Err(format!(
            "Wall {id} would be shorter than {} ft after that move.",
            MIN_WALL_LEN / 12.0
        ));
    }
    Ok(())
}

fn move_wall_endpoint(a: &Args, project: &mut Project) -> Result<Value, String> {
    let fi = a.floor(project)?;
    let id = a.id("id")?;
    require_wall(project, fi, id)?;
    let end = match a.str("end")? {
        "start" => WallEnd::Start,
        "end" => WallEnd::End,
        other => {
            return Err(format!(
                "`end` must be \"start\" or \"end\", not \"{other}\"."
            ))
        }
    };
    let to = a.point("to")?;
    let old = end_pos(project.floors[fi].wall(id).ok_or("internal error")?, end);
    let joined = ends_at(project, fi, old, id);
    check_not_collapsed(project, fi, id, end, to)?;
    for (oid, oe) in &joined {
        check_not_collapsed(project, fi, *oid, *oe, to)?;
    }
    let mut removed = set_end(project, fi, id, end, to);
    for (oid, oe) in &joined {
        removed.extend(set_end(project, fi, *oid, *oe, to));
    }
    Ok(json!({
        "wall": wall_json(project, fi, project.floors[fi].wall(id).ok_or("internal error")?),
        "joined_walls_moved": joined.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
        "openings_removed": removed,
    }))
}

fn translate_wall(a: &Args, project: &mut Project) -> Result<Value, String> {
    let fi = a.floor(project)?;
    let id = a.id("id")?;
    require_wall(project, fi, id)?;
    let dx = a.num("dx")?;
    let dy = a.num("dy")?;
    check_coord("dx", dx)?;
    check_coord("dy", dy)?;
    let delta = Point::new(dx * 12.0, dy * 12.0);
    let w = project.floors[fi].wall(id).ok_or("internal error")?.clone();
    let mut joined = ends_at(project, fi, w.start, id);
    for j in ends_at(project, fi, w.end, id) {
        if !joined.contains(&j) {
            joined.push(j);
        }
    }
    for (oid, oe) in &joined {
        let pos = end_pos(project.floors[fi].wall(*oid).ok_or("internal error")?, *oe);
        check_not_collapsed(project, fi, *oid, *oe, pos.add(delta))?;
    }
    // Every joined end follows the end it touched, not a blanket delta.
    let targets: Vec<(Id, WallEnd, Point)> = joined
        .iter()
        .filter_map(|(oid, oe)| {
            let pos = end_pos(project.floors[fi].wall(*oid)?, *oe);
            let follows = pos.dist(w.start) <= JOIN_TOL || pos.dist(w.end) <= JOIN_TOL;
            follows.then(|| (*oid, *oe, pos.add(delta)))
        })
        .collect();
    project.translate_wall(fi, id, delta);
    let mut removed = Vec::new();
    for (oid, oe, to) in targets {
        removed.extend(set_end(project, fi, oid, oe, to));
    }
    Ok(json!({
        "wall": wall_json(project, fi, project.floors[fi].wall(id).ok_or("internal error")?),
        "joined_walls_stretched": joined.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
        "openings_removed": removed,
    }))
}

// ----- openings -----

fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn parse_style(kind: OpeningKind, s: &str) -> Result<OpeningStyle, String> {
    let want = normalize(s);
    let list = OpeningStyle::for_kind_list(kind);
    list.iter()
        .copied()
        .find(|st| normalize(st.name(kind)) == want || normalize(&format!("{st:?}")) == want)
        .ok_or_else(|| {
            let names: Vec<&str> = list.iter().map(|st| st.name(kind)).collect();
            format!(
                "Unknown {} style `{s}`. Use one of: {}.",
                match kind {
                    OpeningKind::Door => "door",
                    OpeningKind::Window => "window",
                },
                names.join(", ")
            )
        })
}

fn add_opening(a: &Args, project: &mut Project, d: &PlanDefaults) -> Result<Value, String> {
    let fi = a.floor(project)?;
    let wall_id = a.id("wall_id")?;
    require_wall(project, fi, wall_id)?;
    let kind = match a.str("kind")? {
        "door" => OpeningKind::Door,
        "window" => OpeningKind::Window,
        other => {
            return Err(format!(
                "`kind` must be \"door\" or \"window\", not \"{other}\"."
            ))
        }
    };
    let center = a.coord_ft("center_offset_ft")? * 12.0;
    let width_ft = a.opt_num("width_ft")?;
    let height_ft = a.opt_num("height_ft")?;
    let style = a
        .opt_str("style")?
        .map(|s| parse_style(kind, s))
        .transpose()?;
    for (k, v) in [("width_ft", width_ft), ("height_ft", height_ft)] {
        if let Some(v) = v {
            if !(0.5..=60.0).contains(&v) {
                return Err(format!("`{k}` must be between 0.5 and 60 ft."));
            }
        }
    }

    let f = &project.floors[fi];
    let wall = f.wall(wall_id).ok_or("internal error: wall missing")?;
    let len = wall.path_length();
    let mut o = match kind {
        OpeningKind::Door => {
            let od = if wall.kind == WallKind::Exterior {
                &d.exterior_door
            } else {
                &d.interior_door
            };
            let mut o = Opening::default_door(0, wall_id, center);
            o.width = od.width;
            o.height = od.height;
            o
        }
        OpeningKind::Window => {
            let wd = &d.window;
            let mut o = Opening::default_window(0, wall_id, center);
            o.width = wd.width;
            o.height = wd.height;
            o.sill_height = wd.sill_height;
            o.lites = (wd.lites_across, wd.lites_vertical);
            o.egress = wd.egress;
            o.tempered = wd.tempered;
            o
        }
    };
    if let Some(st) = style {
        o = d.opening_variants.apply(&o, st);
    }
    if let Some(w) = width_ft {
        o.width = w * 12.0;
    }
    if let Some(h) = height_ft {
        o.height = h * 12.0;
    }
    let Some(c) = clamp_opening_center(len, o.width, center) else {
        return Err(format!(
            "Wall {wall_id} is {} ft long, too short for a {} ft wide opening (it needs the width plus {} in of wall).",
            ft(len),
            ft(o.width),
            2.0 * OPENING_MARGIN
        ));
    };
    o.center_offset = c;
    if let Some(other) = f
        .openings_on(wall_id)
        .find(|other| openings_conflict(&o, other, OPENING_MARGIN))
    {
        return Err(format!(
            "That spot overlaps opening {} ({} at {} ft). Choose another center_offset_ft or slide the other one first.",
            other.id,
            other.label(),
            ft(other.center_offset)
        ));
    }
    if kind == OpeningKind::Door {
        // Exterior doors swing into the house; the hinge goes to the nearer wall end.
        if wall.kind == WallKind::Exterior {
            let rooms = detect_rooms(&f.walls, JOIN_TOL);
            o.swing_flipped = exterior_sign(wall, &rooms) > 0.0;
        }
        o.hinge_at_end = o.center_offset > len * 0.5;
    }
    o.id = project.alloc_id();
    let out = json!({
        "id": o.id,
        "label": o.label(),
        "wall_id": wall_id,
        "center_offset_ft": ft(o.center_offset),
        "width_ft": ft(o.width),
        "height_ft": ft(o.height),
        "style": o.type_name(),
    });
    project.floors[fi].openings.push(o);
    Ok(out)
}

// ----- rooms, floors, foundation -----

fn set_room_name(a: &Args, project: &mut Project, d: &PlanDefaults) -> Result<Value, String> {
    let fi = a.floor(project)?;
    let p = a.point("point")?;
    let name = a.str("name")?.trim().to_string();
    let ty = a.str("room_type")?.trim().to_string();
    if name.is_empty() {
        return Err("`name` must not be empty.".to_string());
    }
    let Some(def) = d
        .rooms
        .room_types
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(&ty))
    else {
        let types: Vec<&str> = d.rooms.room_types.iter().map(|t| t.name.as_str()).collect();
        return Err(format!(
            "Unknown room type `{ty}`. Use one of: {}.",
            types.join(", ")
        ));
    };
    let rooms = detect_rooms(&project.floors[fi].walls, JOIN_TOL);
    if !rooms.iter().any(|r| r.contains(p)) {
        return Err(format!(
            "No closed room contains ({}, {}) on floor {}. Call list_rooms for the rooms, or validate_plan to find gaps in the walls.",
            ft(p.x),
            ft(p.y),
            fi + 1
        ));
    }
    let def = def.clone();
    project.set_room_name(fi, p, name.clone(), def.name.clone(), &rooms);
    // A new room type brings the platform defaults of its function (a garage floor sits lower).
    if let Some(entry) = project.floors[fi]
        .room_names
        .iter_mut()
        .find(|n| n.anchor == p)
    {
        let fd = function_defaults(&def.function, &def.name);
        apply_function_defaults(entry, &fd, 0.0);
    }
    Ok(json!({"named": name, "room_type": def.name}))
}

fn build_foundation(a: &Args, project: &mut Project) -> Result<Value, String> {
    let kind = a.str("kind")?;
    let mut opts = match kind {
        "slab" => FoundationOptions::new(FoundationKind::MonolithicSlab),
        "stem_wall" => FoundationOptions::new(FoundationKind::StemWall { height: 36.0 }),
        "basement" => {
            let mut o = FoundationOptions::new(FoundationKind::StemWall { height: 108.0 });
            o.rooms = FoundationRooms::Basement;
            o
        }
        "pier" => FoundationOptions::new(FoundationKind::Pier),
        other => {
            return Err(format!(
                "Unknown foundation kind `{other}`. Use one of: slab, stem_wall, basement, pier."
            ))
        }
    };
    if kind == "stem_wall" {
        opts.rooms = FoundationRooms::Auto;
    }
    let first_normal = project
        .floors
        .iter()
        .position(|f| f.kind != plan_core::floors::FloorKind::Foundation)
        .unwrap_or(0);
    let has_exterior = project.floors[first_normal]
        .walls
        .iter()
        .any(|w| w.kind == WallKind::Exterior && !w.flags.foundation && !w.flags.invisible);
    if !has_exterior {
        return Err(
            "The first floor has no exterior walls to build a foundation under. Draw the exterior walls first."
                .to_string(),
        );
    }
    let idx = project.build_foundation_with(&opts);
    Ok(json!({
        "foundation_floor": idx + 1,
        "kind": kind,
        "floors": floor_names(project),
        "note": "Floor numbers of the other floors moved up by one if the foundation is new.",
    }))
}

// ----- validation -----

fn validate_plan(project: &Project) -> Value {
    let mut floors = Vec::new();
    let mut all_ok = true;
    for (fi, f) in project.floors.iter().enumerate() {
        let rooms = detect_rooms(&f.walls, JOIN_TOL);
        let mut unclosed = Vec::new();
        for w in f.walls.iter().filter(|w| w.defines_rooms()) {
            for e in [WallEnd::Start, WallEnd::End] {
                let p = end_pos(w, e);
                let closed = f.walls.iter().filter(|o| o.id != w.id).any(|o| {
                    [o.start, o.end].iter().any(|q| q.dist(p) <= 1.0)
                        || dist_to_segment(p, o.start, o.end) <= 1.0
                });
                if !closed {
                    unclosed
                        .push(json!({"wall_id": w.id, "end": end_name(e), "point": pt_json(p)}));
                }
            }
        }
        let mut overlapping = Vec::new();
        let mut outside = Vec::new();
        for (i, o) in f.openings.iter().enumerate() {
            if let Some(w) = f.wall(o.wall_id) {
                let len = w.path_length();
                if o.start_offset() < -1e-6 || o.end_offset() > len + 1e-6 {
                    outside.push(json!({"opening_id": o.id, "wall_id": o.wall_id}));
                }
            } else {
                outside.push(json!({"opening_id": o.id, "wall_id": o.wall_id}));
            }
            for other in f
                .openings
                .iter()
                .skip(i + 1)
                .filter(|x| x.wall_id == o.wall_id)
            {
                if openings_conflict(o, other, 0.0) {
                    overlapping.push(json!({
                        "wall_id": o.wall_id, "opening_a": o.id, "opening_b": other.id
                    }));
                }
            }
        }
        let unnamed = rooms
            .iter()
            .filter(|r| r.name_entry(&f.room_names).is_none())
            .count();
        if !unclosed.is_empty() || !overlapping.is_empty() || !outside.is_empty() {
            all_ok = false;
        }
        floors.push(json!({
            "floor": fi + 1,
            "name": f.name,
            "rooms": rooms.len(),
            "unnamed_rooms": unnamed,
            "unclosed_wall_ends": unclosed,
            "overlapping_openings": overlapping,
            "openings_outside_wall": outside,
        }));
    }
    json!({"ok": all_ok, "floors": floors})
}

// ----- parameters -----

fn define_parameter(a: &Args, params: &mut Vec<Parameter>) -> Result<Value, String> {
    let name = a.str("name")?.trim().to_string();
    let valid_name = !name.is_empty()
        && name.len() <= 48
        && name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !valid_name {
        return Err(
            "`name` must be snake_case: lowercase letters, digits and underscores, starting with a letter."
                .to_string(),
        );
    }
    let label = a.str("label")?.trim().to_string();
    let (value, min, max) = (a.num("value")?, a.num("min")?, a.num("max")?);
    if label.is_empty() {
        return Err("`label` must not be empty.".to_string());
    }
    if min >= max {
        return Err("`min` must be less than `max`.".to_string());
    }
    if value < min || value > max {
        return Err(format!(
            "`value` {value} is outside the range {min} to {max}."
        ));
    }
    let unit = a.str("unit")?.trim().to_string();
    if unit.len() > 12 {
        return Err("`unit` is too long; use ft, in, sqft, count or an empty string.".to_string());
    }
    let p = Parameter {
        name: name.clone(),
        label,
        value,
        min,
        max,
        unit,
        description: a.str("description")?.trim().to_string(),
    };
    match params.iter_mut().find(|q| q.name == name) {
        Some(slot) => *slot = p,
        None => {
            if params.len() >= MAX_PARAMETERS {
                return Err(format!(
                    "At most {MAX_PARAMETERS} parameters can be defined."
                ));
            }
            params.push(p);
        }
    }
    Ok(json!({"name": name, "value": value, "parameters": params.len()}))
}

// ----- summaries for the transcript -----

/// A short one-line description of a tool call for the UI.
pub fn input_summary(name: &str, input: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(m) = input.as_object() {
        for (k, v) in m {
            let text = match v {
                Value::Null => continue,
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            parts.push(format!("{k}={text}"));
        }
    }
    let mut s = if parts.is_empty() {
        name.to_string()
    } else {
        format!("{name} {}", parts.join(" "))
    };
    if s.chars().count() > 140 {
        s = s.chars().take(137).collect::<String>() + "...";
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Project, PlanDefaults, Vec<Parameter>) {
        let d = PlanDefaults::default();
        (Project::from_defaults("t", &d), d, Vec::new())
    }

    fn run(
        p: &mut Project,
        d: &PlanDefaults,
        params: &mut Vec<Parameter>,
        name: &str,
        input: Value,
    ) -> Result<Value, String> {
        execute(name, &input, p, d, params)
    }

    #[test]
    fn every_tool_is_strict_with_every_property_required() {
        let tools = catalog();
        assert_eq!(tools.len(), 23);
        for t in &tools {
            let j = t.to_api_json();
            assert_eq!(j["strict"], true, "{}", t.name);
            let schema = &j["input_schema"];
            assert_eq!(schema["additionalProperties"], false, "{}", t.name);
            let props: Vec<&String> = schema["properties"].as_object().unwrap().keys().collect();
            let mut req: Vec<String> = schema["required"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect();
            req.sort();
            let mut want: Vec<String> = props.iter().map(|s| s.to_string()).collect();
            want.sort();
            assert_eq!(req, want, "{}", t.name);
        }
        let names: std::collections::BTreeSet<_> = tools.iter().map(|t| t.name).collect();
        assert_eq!(names.len(), tools.len(), "tool names are unique");
    }

    #[test]
    fn a_rectangle_with_a_door_and_a_window_makes_one_closed_room() {
        let (mut p, d, mut params) = setup();
        let r = run(
            &mut p,
            &d,
            &mut params,
            "add_wall_rectangle",
            json!({"floor": 1, "x": 0, "y": 0, "width": 30, "depth": 40, "kind": "exterior"}),
        )
        .unwrap();
        let ids = r["ids"].as_array().unwrap().clone();
        assert_eq!(ids.len(), 4);
        // bottom wall: door; right wall: window
        let door = run(
            &mut p,
            &d,
            &mut params,
            "add_opening",
            json!({"floor": 1, "wall_id": ids[0], "kind": "door", "center_offset_ft": 15,
                   "width_ft": null, "height_ft": null, "style": null}),
        )
        .unwrap();
        assert!(door["label"].as_str().unwrap().len() >= 4);
        run(
            &mut p,
            &d,
            &mut params,
            "add_opening",
            json!({"floor": 1, "wall_id": ids[1], "kind": "window", "center_offset_ft": 20,
                   "width_ft": 4, "height_ft": null, "style": "Casement Window"}),
        )
        .unwrap();
        let rooms = run(&mut p, &d, &mut params, "list_rooms", json!({"floor": 1})).unwrap();
        let rooms = rooms.as_array().unwrap();
        assert_eq!(rooms.len(), 1);
        assert!((rooms[0]["area_sqft"].as_f64().unwrap() - 1200.0).abs() < 1.0);
        run(
            &mut p,
            &d,
            &mut params,
            "set_room_name",
            json!({"floor": 1, "point": [15, 20], "name": "Great Room", "room_type": "Living"}),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let rooms = run(&mut p, &d, &mut params, "list_rooms", json!({"floor": 1})).unwrap();
        assert_eq!(rooms[0]["name"], "Great Room");
        let v = run(&mut p, &d, &mut params, "validate_plan", json!({})).unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["floors"][0]["rooms"], 1);
        assert_eq!(
            v["floors"][0]["unclosed_wall_ends"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        let openings = run(
            &mut p,
            &d,
            &mut params,
            "list_openings",
            json!({"floor": 1}),
        )
        .unwrap();
        assert_eq!(openings.as_array().unwrap().len(), 2);
        // The exterior face points outward on every wall.
        let center = Point::new(15.0 * 12.0, 20.0 * 12.0);
        for w in &p.floors[0].walls {
            let mid = w.point_at(w.length() * 0.5);
            assert!(
                w.exterior_normal().dot(mid.sub(center)) > 0.0,
                "wall {} exterior faces inward",
                w.id
            );
        }
        // Exterior doors swing inward: the door sits in the bottom wall (y = 0).
        let o = p.floors[0]
            .openings
            .iter()
            .find(|o| o.kind == OpeningKind::Door)
            .unwrap();
        let w = p.floors[0].wall(o.wall_id).unwrap();
        let swing_side = if o.swing_flipped { -1.0 } else { 1.0 } * w.normal().y;
        assert!(swing_side > 0.0, "the door swings toward the room");
    }

    #[test]
    fn an_open_wall_chain_is_reported_unclosed() {
        let (mut p, d, mut params) = setup();
        for (s, e) in [([0, 0], [20, 0]), ([20, 0], [20, 20])] {
            run(
                &mut p,
                &d,
                &mut params,
                "add_wall",
                json!({"floor": 1, "start": s, "end": e, "kind": "interior", "thickness_in": null}),
            )
            .unwrap();
        }
        let v = run(&mut p, &d, &mut params, "validate_plan", json!({})).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(
            v["floors"][0]["unclosed_wall_ends"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn conflicting_openings_and_bad_floors_are_errors_that_change_nothing() {
        let (mut p, d, mut params) = setup();
        let r = run(
            &mut p,
            &d,
            &mut params,
            "add_wall_rectangle",
            json!({"floor": 1, "x": 0, "y": 0, "width": 20, "depth": 20, "kind": "interior"}),
        )
        .unwrap();
        let wall = r["ids"][0].clone();
        let add = |p: &mut Project, params: &mut Vec<Parameter>, at: f64| {
            run(
                p,
                &d,
                params,
                "add_opening",
                json!({"floor": 1, "wall_id": wall, "kind": "door", "center_offset_ft": at,
                       "width_ft": null, "height_ft": null, "style": null}),
            )
        };
        add(&mut p, &mut params, 10.0).unwrap();
        let snapshot = serde_json::to_string(&p).unwrap();
        let err = add(&mut p, &mut params, 10.5).unwrap_err();
        assert!(err.contains("overlaps"), "{err}");
        assert_eq!(
            serde_json::to_string(&p).unwrap(),
            snapshot,
            "a failed tool changes nothing"
        );

        let err = run(&mut p, &d, &mut params, "list_walls", json!({"floor": 9})).unwrap_err();
        assert!(err.contains("Floor 9 does not exist"), "{err}");
        let err = run(&mut p, &d, &mut params, "list_walls", json!({"floor": 0})).unwrap_err();
        assert!(err.contains("does not exist"), "{err}");
        let err = run(
            &mut p,
            &d,
            &mut params,
            "remove_wall",
            json!({"floor": 1, "id": 9999}),
        )
        .unwrap_err();
        assert!(err.contains("No wall"), "{err}");
        let err = run(&mut p, &d, &mut params, "bogus", json!({})).unwrap_err();
        assert!(err.contains("Unknown tool"), "{err}");
        let err = run(
            &mut p,
            &d,
            &mut params,
            "add_wall",
            json!({"floor": 1, "start": [0, 0], "end": [0.1, 0], "kind": "interior", "thickness_in": null}),
        )
        .unwrap_err();
        assert!(err.contains("at least"), "{err}");
        assert_eq!(serde_json::to_string(&p).unwrap(), snapshot);
    }

    #[test]
    fn invalid_room_type_lists_the_valid_ones() {
        let (mut p, d, mut params) = setup();
        run(
            &mut p,
            &d,
            &mut params,
            "add_wall_rectangle",
            json!({"floor": 1, "x": 0, "y": 0, "width": 12, "depth": 12, "kind": "interior"}),
        )
        .unwrap();
        let err = run(
            &mut p,
            &d,
            &mut params,
            "set_room_name",
            json!({"floor": 1, "point": [6, 6], "name": "X", "room_type": "Throne Room"}),
        )
        .unwrap_err();
        assert!(err.contains("Use one of"), "{err}");
        let err = run(
            &mut p,
            &d,
            &mut params,
            "set_room_name",
            json!({"floor": 1, "point": [60, 60], "name": "X", "room_type": "Bedroom"}),
        )
        .unwrap_err();
        assert!(err.contains("No closed room"), "{err}");
    }

    #[test]
    fn moving_an_endpoint_drags_the_joined_walls() {
        let (mut p, d, mut params) = setup();
        let r = run(
            &mut p,
            &d,
            &mut params,
            "add_wall_rectangle",
            json!({"floor": 1, "x": 0, "y": 0, "width": 20, "depth": 10, "kind": "exterior"}),
        )
        .unwrap();
        let bottom = r["ids"][0].clone();
        // Stretch the bottom wall's end (the lower-right corner) to x = 25.
        run(
            &mut p,
            &d,
            &mut params,
            "move_wall_endpoint",
            json!({"floor": 1, "id": bottom, "end": "end", "to": [25, 0]}),
        )
        .unwrap();
        let rooms = run(&mut p, &d, &mut params, "list_rooms", json!({"floor": 1})).unwrap();
        assert_eq!(rooms.as_array().unwrap().len(), 1, "the room stays closed");
        let v = run(&mut p, &d, &mut params, "validate_plan", json!({})).unwrap();
        assert_eq!(v["ok"], true);
        // Slide the whole top wall up by 5 ft: the side walls stretch with it.
        let top = r["ids"][2].clone();
        run(
            &mut p,
            &d,
            &mut params,
            "translate_wall",
            json!({"floor": 1, "id": top, "dx": 0, "dy": 5}),
        )
        .unwrap();
        let rooms = run(&mut p, &d, &mut params, "list_rooms", json!({"floor": 1})).unwrap();
        assert_eq!(rooms.as_array().unwrap().len(), 1);
        let walls = run(&mut p, &d, &mut params, "list_walls", json!({"floor": 1})).unwrap();
        assert_eq!(walls.as_array().unwrap().len(), 4);
        let left = walls
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["id"] == r["ids"][3])
            .unwrap();
        assert_eq!(left["length_ft"], 15.0);
    }

    #[test]
    fn opening_edits_work_and_remove_wall_takes_its_openings() {
        let (mut p, d, mut params) = setup();
        let r = run(
            &mut p,
            &d,
            &mut params,
            "add_wall",
            json!({"floor": 1, "start": [0, 0], "end": [20, 0], "kind": "exterior", "thickness_in": null}),
        )
        .unwrap();
        let wid = r["id"].clone();
        let o = run(
            &mut p,
            &d,
            &mut params,
            "add_opening",
            json!({"floor": 1, "wall_id": wid, "kind": "door", "center_offset_ft": 5,
                   "width_ft": 3, "height_ft": null, "style": "hinged"}),
        )
        .unwrap();
        let oid = o["id"].clone();
        run(
            &mut p,
            &d,
            &mut params,
            "slide_opening",
            json!({"floor": 1, "id": oid, "center_offset_ft": 12}),
        )
        .unwrap();
        run(
            &mut p,
            &d,
            &mut params,
            "set_opening_width",
            json!({"floor": 1, "id": oid, "width_ft": 6}),
        )
        .unwrap();
        run(
            &mut p,
            &d,
            &mut params,
            "flip_swing",
            json!({"floor": 1, "id": oid}),
        )
        .unwrap();
        run(
            &mut p,
            &d,
            &mut params,
            "flip_hinge",
            json!({"floor": 1, "id": oid}),
        )
        .unwrap();
        let list = run(
            &mut p,
            &d,
            &mut params,
            "list_openings",
            json!({"floor": 1}),
        )
        .unwrap();
        assert_eq!(list[0]["center_offset_ft"], 12.0);
        assert_eq!(list[0]["width_ft"], 6.0);
        let err = run(
            &mut p,
            &d,
            &mut params,
            "set_opening_width",
            json!({"floor": 1, "id": oid, "width_ft": 30}),
        )
        .unwrap_err();
        assert!(err.contains("cannot be"), "{err}");
        let r = run(
            &mut p,
            &d,
            &mut params,
            "remove_wall",
            json!({"floor": 1, "id": wid}),
        )
        .unwrap();
        assert_eq!(r["removed_openings"], 1);
        assert!(p.floors[0].openings.is_empty());
    }

    #[test]
    fn floors_and_foundations() {
        let (mut p, d, mut params) = setup();
        let err = run(
            &mut p,
            &d,
            &mut params,
            "build_foundation",
            json!({"kind": "slab"}),
        )
        .unwrap_err();
        assert!(err.contains("no exterior walls"), "{err}");
        run(
            &mut p,
            &d,
            &mut params,
            "add_wall_rectangle",
            json!({"floor": 1, "x": 0, "y": 0, "width": 24, "depth": 30, "kind": "exterior"}),
        )
        .unwrap();
        let r = run(
            &mut p,
            &d,
            &mut params,
            "build_new_floor",
            json!({"copy_exterior_walls": true}),
        )
        .unwrap();
        assert_eq!(r["floor_number"], 2);
        assert_eq!(p.floors[1].walls.len(), 4);
        let r = run(
            &mut p,
            &d,
            &mut params,
            "build_foundation",
            json!({"kind": "stem_wall"}),
        )
        .unwrap();
        assert_eq!(r["foundation_floor"], 1);
        assert_eq!(p.floors.len(), 3);
        let r = run(&mut p, &d, &mut params, "delete_floor", json!({"floor": 3})).unwrap();
        assert_eq!(r["deleted_floor"], 3);
        let err = run(
            &mut p,
            &d,
            &mut params,
            "build_foundation",
            json!({"kind": "moat"}),
        )
        .unwrap_err();
        assert!(err.contains("slab, stem_wall, basement, pier"), "{err}");
        run(&mut p, &d, &mut params, "delete_floor", json!({"floor": 2})).unwrap();
        let err = run(&mut p, &d, &mut params, "delete_floor", json!({"floor": 1})).unwrap_err();
        assert!(err.contains("last remaining"), "{err}");
    }

    #[test]
    fn parameters_upsert_and_validate() {
        let (mut p, d, mut params) = setup();
        let def = |v: f64| {
            json!({"name": "garage_width_ft", "label": "Garage width", "value": v, "min": 12,
                   "max": 40, "unit": "ft", "description": "Width of the garage."})
        };
        run(&mut p, &d, &mut params, "define_parameter", def(24.0)).unwrap();
        run(&mut p, &d, &mut params, "define_parameter", def(26.0)).unwrap();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].value, 26.0);
        assert!(run(&mut p, &d, &mut params, "define_parameter", def(50.0)).is_err());
        let bad = json!({"name": "Garage Width", "label": "x", "value": 1, "min": 0, "max": 2, "unit": "", "description": ""});
        assert!(run(&mut p, &d, &mut params, "define_parameter", bad).is_err());
        let r = run(&mut p, &d, &mut params, "read_parameters", json!({})).unwrap();
        assert_eq!(r[0]["value"], 26.0);
    }

    #[test]
    fn measure_and_summary() {
        let (mut p, d, mut params) = setup();
        let r = run(
            &mut p,
            &d,
            &mut params,
            "measure",
            json!({"from": [0, 0], "to": [3, 4]}),
        )
        .unwrap();
        assert_eq!(r["distance_ft"], 5.0);
        assert_eq!(r["text"], "5'-0\"");
        let r = run(&mut p, &d, &mut params, "get_plan_summary", json!({})).unwrap();
        assert!(r["summary"].as_str().unwrap().contains("Floor 1"));
        assert_eq!(r["floors"][0]["floor"], 1);
    }

    #[test]
    fn input_summaries_are_short() {
        let s = input_summary(
            "add_wall_rectangle",
            &json!({"floor": 1, "x": 0, "kind": "exterior", "t": null}),
        );
        assert!(s.starts_with("add_wall_rectangle "));
        assert!(!s.contains("t="));
        let long = input_summary("x", &json!({"a": "y".repeat(500)}));
        assert!(long.chars().count() <= 140);
    }
}
