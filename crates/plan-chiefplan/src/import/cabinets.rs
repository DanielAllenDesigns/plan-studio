//! Cabinets (class 15) and island countertops (class 109 on a floor).
//!
//! Class 15 is Chief's box object: base, wall and tall cabinets, but also
//! shelves and soffits (a soffit is a 6" box at 108" with the strings
//! `soffit_material_data`). Every one carries the placement record of
//! [`blocks`](super::blocks): the middle of the back edge, the front direction,
//! depth, width, height and the elevation of the top. Checked on job C (50
//! cabinets on the first floor): the point lies one half wall thickness
//! (3.25" or 3.5") from the wall behind in 43 of 50, the front direction points
//! away from that wall in the same 43; the other seven are islands and
//! soffits.
//!
//! | Plan Studio kind | rule (bottom = top - height) | Confidence |
//! |---|---|---|
//! | `Soffit` | the object has `soffit` strings | High |
//! | `Shelf` | height <= 1.5" and bottom above the floor | Medium |
//! | `Wall` | bottom >= 1" | High |
//! | `FullHeight` | bottom on the floor and height over 40" | High |
//! | `Base` | the rest | High |
//!
//! A base cabinet carries two class 109 children (1,158 bytes each) where a
//! wall or tall cabinet carries one: 32 of 40 base cabinets have two, all 55
//! wall and tall cabinets have one, so two means "has a countertop". The
//! door and drawer fronts are class 114 children named after their style
//! (`Lincoln Door`, `Lincoln Flat Panel Drawer`, `Framed Panel`); the face
//! layout is only inferred from those names (a drawer and a door, doors only,
//! drawers only), Low.
//!
//! A class 109 object sitting directly on a floor is a free-form countertop
//! (an island): its outline is a closed chain of line records and, counted from
//! the end of the object, the thickness sits at `size - 2777` and the top
//! elevation at `size - 2769` (checked on four islands of three jobs).
//!
//! A library object built into a cabinet (class 123 below it, which would
//! otherwise be an orphan in the cabinet's own frame) sets its look: a sink or
//! lavatory gives the sink-base face, a dishwasher an open bay with the
//! appliance name `Dishwasher`.
//!
//! # Corner cabinets (stage 3)
//!
//! A corner cabinet is stored as its square bounding box like any other box,
//! with nothing that marks it: no line records of its own (0 of 555 cabinets
//! of 11 projects), no `corner`/`blind` string among the 262 distinct strings
//! of the class, and the bytes of equal-size cabinets differ only in GUID and
//! id. What marks it is where it stands. Of 495 boxes, three are square (36"
//! x 36", 30" to 36" high), and all three have their back-right corner within
//! 3.5" of two perpendicular walls (half a 6.5" wall plus the 0.25" the
//! healing leaves): the inside corner of a kitchen. The importer therefore
//! reads a base or wall box that is square (within 1.5"), 30" to 60" a side
//! and has a back corner on two perpendicular walls as `CornerBase` /
//! `CornerWall` (Low: geometry only, a 36" square cabinet that happens to
//! stand in a corner without being an L would be misread; flagged "verify in
//! Chief"). The legs are the box's sides, the arms 24" (12" for a wall
//! cabinet), the notch opposite the wall corner, a diagonal front. A box
//! whose corner is the back-right one is placed with its origin there and
//! turned a quarter turn, so Plan Studio's corner (the origin) is at the walls.
//!
//! Blind cabinets have no such mark (a blind base is a plain rectangle that
//! tucks behind its neighbour): they stay plain boxes. A cabinet's label is
//! not stored either (`=label` is one of the material list's macros, not a
//! value), so labels stay automatic.
//!
//! Not decoded: the cabinet's catalog name, per-cabinet materials, handle
//! style, blind cabinets, appliance cutouts, moldings.

use super::blocks::{own_block, BoxBlock};
use super::lines::{chain, find_edges, is_closed, polygon_of};
use super::tree::{f64_at, strings_in, ObjectTree};
use plan_core::geometry::{dist_to_segment, Point};
use plan_core::model::Wall;
use serde_json::{json, Value};

/// Cabinet / box class id.
pub const CABINET: u8 = 15;
/// Countertop class id (a child of a cabinet, or a free-form top on a floor).
pub const COUNTERTOP: u8 = 109;
/// The class of the door and drawer style entries inside a cabinet.
const STYLE_ENTRY: u8 = 114;
/// Furniture-group class (a cabinet may sit inside one).
pub const GROUP: u8 = 16;

/// Offsets of a free-form countertop's thickness and top, counted back from the
/// end of the object.
const TOP_THICKNESS_FROM_END: usize = 2777;
const TOP_ELEVATION_FROM_END: usize = 2769;

/// What kind of box a class 15 object is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxKind {
    Base,
    Wall,
    FullHeight,
    Soffit,
    Shelf,
}

impl BoxKind {
    /// The variant name `plan_cabinets::CabinetKind` serializes to.
    pub fn json_name(self) -> &'static str {
        match self {
            BoxKind::Base => "Base",
            BoxKind::Wall => "Wall",
            BoxKind::FullHeight => "FullHeight",
            BoxKind::Soffit => "Soffit",
            BoxKind::Shelf => "Shelf",
        }
    }

    /// The label of this kind in the report.
    pub fn label(self) -> &'static str {
        match self {
            BoxKind::Base => "base",
            BoxKind::Wall => "wall",
            BoxKind::FullHeight => "tall",
            BoxKind::Soffit => "soffit",
            BoxKind::Shelf => "shelf",
        }
    }
}

/// One decoded cabinet.
#[derive(Debug, Clone, PartialEq)]
pub struct ChiefCabinet {
    pub node: usize,
    pub block: BoxBlock,
    pub kind: BoxKind,
    /// A base cabinet with its own countertop (two class 109 children).
    pub has_top: bool,
    /// Name of the first door style entry (`Lincoln Door`), if any.
    pub door_style: Option<String>,
    /// Name of the first drawer style entry.
    pub drawer_style: Option<String>,
    /// Names of the library objects built into the cabinet (a sink, a
    /// dishwasher): class 123 descendants.
    pub fixtures: Vec<String>,
}

/// The first string of a style / library entry object (class 114 or similar):
/// the entry name. Strings with a copyright sign are not printable ASCII and
/// are skipped by the reader; a pure-ASCII copyright line is skipped here.
pub(crate) fn entry_name(bytes: &[u8], tree: &ObjectTree, node: usize) -> Option<String> {
    let n = tree.node(node);
    strings_in(bytes, n.marker + 0x18, n.end.min(n.marker + 400), 200)
        .into_iter()
        .map(|(_, s)| s)
        .find(|s| {
            let l = s.to_ascii_lowercase();
            !l.starts_with("copyright") && !s.trim().is_empty() && s.len() > 1
        })
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// Decodes the class 15 object at tree index `node`.
pub fn decode_cabinet(bytes: &[u8], tree: &ObjectTree, node: usize) -> Option<ChiefCabinet> {
    let n = tree.node(node);
    let block = own_block(bytes, tree, node)?;
    let elevation = block.elevation();
    let soffit = contains(&bytes[n.marker..n.end], b"soffit_");
    let kind = if soffit {
        BoxKind::Soffit
    } else if block.height <= 1.5 && elevation > 0.0 {
        BoxKind::Shelf
    } else if elevation >= 1.0 {
        BoxKind::Wall
    } else if block.height > 40.0 {
        BoxKind::FullHeight
    } else {
        BoxKind::Base
    };
    let tops = tree
        .children(node)
        .iter()
        .filter(|&&c| tree.node(c).class == COUNTERTOP)
        .count();
    let mut door_style = None;
    let mut drawer_style = None;
    for &c in tree.children(node) {
        if tree.node(c).class != STYLE_ENTRY {
            continue;
        }
        let Some(name) = entry_name(bytes, tree, c) else {
            continue;
        };
        if name.to_ascii_lowercase().contains("drawer") {
            drawer_style.get_or_insert(name);
        } else {
            door_style.get_or_insert(name);
        }
    }
    let mut fixtures = Vec::new();
    collect_fixtures(bytes, tree, node, &mut fixtures, 0);
    Some(ChiefCabinet {
        node,
        block,
        kind,
        has_top: kind == BoxKind::Base && tops >= 2,
        door_style,
        drawer_style,
        fixtures,
    })
}

/// Names of the class 123 objects below `node` (a sink or a dishwasher sits in
/// the cabinet or in a class 62 part of it), looking three levels down.
fn collect_fixtures(
    bytes: &[u8],
    tree: &ObjectTree,
    node: usize,
    out: &mut Vec<String>,
    depth: usize,
) {
    if depth > 3 {
        return;
    }
    for &c in tree.children(node) {
        match tree.node(c).class {
            super::symbols::SYMBOL => {
                let entry = tree
                    .children(c)
                    .iter()
                    .copied()
                    .find(|&e| tree.node(e).class == STYLE_ENTRY);
                if let Some(name) = entry.and_then(|e| entry_name(bytes, tree, e)) {
                    out.push(name);
                }
            }
            STYLE_ENTRY | 120 | 93 | COUNTERTOP => {}
            _ => collect_fixtures(bytes, tree, c, out, depth + 1),
        }
    }
}

fn has_fixture(c: &ChiefCabinet, words: &[&str]) -> bool {
    c.fixtures.iter().any(|f| {
        let l = f.to_ascii_lowercase();
        words.iter().any(|w| l.contains(w))
    })
}

fn sep() -> Value {
    json!({"Separation": {"height": 1.5}})
}

/// The face layout inferred from the style entries, as the JSON of
/// `plan_cabinets::FaceLayout`.
fn face_json(c: &ChiefCabinet) -> Value {
    let items: Vec<Value> = match c.kind {
        BoxKind::Soffit | BoxKind::Shelf => Vec::new(),
        BoxKind::Base if has_fixture(c, &["dishwasher"]) => {
            vec![json!({"Opening": {"height": 0.0}})]
        }
        BoxKind::Base if has_fixture(c, &["sink", "lavatory", "basin"]) => vec![
            sep(),
            json!({"Appliance": {"height": 6.0, "name": "Sink"}}),
            sep(),
            json!({"DoubleDoor": {"height": 0.0}}),
            sep(),
        ],
        BoxKind::Base => match (&c.door_style, &c.drawer_style) {
            (None, Some(_)) => {
                let mut v = vec![sep()];
                for _ in 0..3 {
                    v.push(json!({"Drawer": {"height": 0.0}}));
                    v.push(sep());
                }
                v
            }
            (Some(_), None) => {
                let door = if c.block.width >= 30.0 {
                    json!({"DoubleDoor": {"height": 0.0}})
                } else {
                    json!({"DoorAuto": {"height": 0.0}})
                };
                vec![sep(), door, sep()]
            }
            _ => vec![
                sep(),
                json!({"Drawer": {"height": 6.0}}),
                sep(),
                json!({"DoorAuto": {"height": 0.0}}),
                sep(),
            ],
        },
        BoxKind::Wall => {
            let mut v = vec![sep(), json!({"DoorAuto": {"height": 0.0}}), sep()];
            if c.block.height >= 42.0 {
                v.push(json!({"DoorAuto": {"height": 0.0}}));
                v.push(sep());
            }
            v
        }
        BoxKind::FullHeight => vec![
            sep(),
            json!({"DoorAuto": {"height": 0.0}}),
            sep(),
            json!({"DoorAuto": {"height": (c.block.height * 0.45).min(36.0)}}),
            sep(),
        ],
    };
    json!({"items": items, "frame_width": 1.5})
}

/// The `Floor.cabinets` entry of a cabinet, in the serialized shape of
/// `plan_cabinets::Cabinet` (`id` is assigned by the caller). Position is the
/// back-left corner, `angle` radians counter-clockwise, as Plan Studio keeps
/// cabinets.
pub fn cabinet_json(c: &ChiefCabinet, id: u64) -> Value {
    let b = &c.block;
    let (px, py) = b.back_left();
    let countertop = c.has_top.then(|| {
        json!({"thickness": 1.5, "overhang_front": 1.0, "overhang_sides": 0.0, "overhang_back": 0.0})
    });
    let toe = matches!(c.kind, BoxKind::Base | BoxKind::FullHeight)
        .then(|| json!({"height": 4.0, "depth": 3.0}));
    let mut door = serde_json::Map::new();
    if let Some(n) = &c.door_style {
        door.insert("name".into(), json!(n));
    }
    let mut drawer = serde_json::Map::new();
    if let Some(n) = &c.drawer_style {
        drawer.insert("name".into(), json!(n));
    }
    let appliance =
        (c.kind == BoxKind::Base && has_fixture(c, &["dishwasher"])).then_some("Dishwasher");
    json!({
        "id": id,
        "kind": c.kind.json_name(),
        "position": {"x": px, "y": py},
        "angle": b.angle(),
        "width": b.width,
        "depth": b.depth,
        "height": b.height,
        "elevation": b.elevation(),
        "countertop": countertop,
        "backsplash": null,
        "toe_kick": toe,
        "face": face_json(c),
        "door_style": Value::Object(door),
        "drawer_style": Value::Object(drawer),
        "overlay": {"Full": {"reveal": 0.0625}},
        "framed": true,
        "label": "",
        "appliance": appliance,
    })
}

/// Which back corner of a square box stands in the corner of two walls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CornerAt {
    BackLeft,
    BackRight,
}

/// Whether `corner` touches two perpendicular walls (within half the wall's
/// thickness plus 3").
fn in_wall_corner(corner: (f64, f64), walls: &[Wall]) -> bool {
    let p = Point::new(corner.0, corner.1);
    let near: Vec<&Wall> = walls
        .iter()
        .filter(|w| w.start.dist(w.end) > 1.0)
        .filter(|w| dist_to_segment(p, w.start, w.end) <= w.thickness / 2.0 + 3.0)
        .collect();
    near.iter().enumerate().any(|(i, a)| {
        near[i + 1..].iter().any(|b| {
            let (da, db) = (a.end.sub(a.start), b.end.sub(b.start));
            let cross = (da.x * db.y - da.y * db.x) / (da.x.hypot(da.y) * db.x.hypot(db.y));
            cross.abs() > 0.5
        })
    })
}

/// The back corner of a square base or wall box that stands in the corner of
/// two walls: a corner cabinet (see the module notes; Low).
pub fn corner_at(c: &ChiefCabinet, walls: &[Wall]) -> Option<CornerAt> {
    let b = &c.block;
    if !matches!(c.kind, BoxKind::Base | BoxKind::Wall)
        || (b.depth - b.width).abs() > 1.5
        || !(30.0..=60.0).contains(&b.width)
    {
        return None;
    }
    let corners = b.corners();
    match (
        in_wall_corner(corners[0], walls),
        in_wall_corner(corners[1], walls),
    ) {
        (true, false) => Some(CornerAt::BackLeft),
        (false, true) => Some(CornerAt::BackRight),
        _ => None,
    }
}

/// Turns the `Floor.cabinets` entry of a box into a corner cabinet standing
/// at `at`: kind `CornerBase`/`CornerWall`, both legs the box's side, the
/// origin at the wall corner.
pub fn make_corner(v: &mut Value, c: &ChiefCabinet, at: CornerAt) {
    let b = &c.block;
    let leg = (b.width + b.depth) / 2.0;
    let wall = c.kind == BoxKind::Wall;
    let (px, py, angle) = match at {
        CornerAt::BackLeft => {
            let (x, y) = b.back_left();
            (x, y, b.angle())
        }
        CornerAt::BackRight => {
            let (x, y) = b.corners()[1];
            (x, y, b.angle() + std::f64::consts::FRAC_PI_2)
        }
    };
    v["kind"] = json!(if wall { "CornerWall" } else { "CornerBase" });
    v["position"] = json!({"x": px, "y": py});
    v["angle"] = json!(angle);
    v["width"] = json!(leg);
    v["depth"] = json!(leg);
    v["corner"] = json!({
        "style": "Diagonal",
        "lazy_susan": false,
        "arm_depth": if wall { 12.0_f64 } else { 24.0_f64 }.min(leg),
    });
}

/// A free-form countertop on a floor.
#[derive(Debug, Clone, PartialEq)]
pub struct ChiefCountertop {
    pub node: usize,
    /// Counter-clockwise outline, plan inches.
    pub outline: Vec<(f64, f64)>,
    pub thickness: f64,
    /// Elevation of the top surface.
    pub top: f64,
}

/// Decodes the class 109 object at `node` as a free-form countertop; `None`
/// for the small 109 children of cabinets and for objects without a closed
/// outline.
pub fn decode_countertop(bytes: &[u8], tree: &ObjectTree, node: usize) -> Option<ChiefCountertop> {
    let n = tree.node(node);
    if n.len() < 3000 {
        return None;
    }
    let edges = find_edges(bytes, n.marker, n.marker + 0x200, n.end);
    if edges.is_empty() {
        return None;
    }
    let c = chain(&edges, 0);
    if !is_closed(&c) {
        return None;
    }
    let mut outline = polygon_of(&c);
    if outline.len() < 3 {
        return None;
    }
    if super::lines::signed_area(&outline) < 0.0 {
        outline.reverse();
    }
    let thickness =
        f64_at(bytes, n.end - TOP_THICKNESS_FROM_END).filter(|v| (0.25..=12.0).contains(v))?;
    let top =
        f64_at(bytes, n.end - TOP_ELEVATION_FROM_END).filter(|v| (10.0..=60.0).contains(v))?;
    Some(ChiefCountertop {
        node,
        outline,
        thickness,
        top,
    })
}

/// The `Floor.cabinets` entry of a free-form countertop (`CustomCountertop`
/// with a closed outline relative to its bounding box, as
/// `Cabinet::custom_countertop` builds it).
pub fn countertop_json(t: &ChiefCountertop, id: u64) -> Value {
    let (mut lo, mut hi) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
    for &(x, y) in &t.outline {
        lo = (lo.0.min(x), lo.1.min(y));
        hi = (hi.0.max(x), hi.1.max(y));
    }
    let outline: Vec<Value> = t
        .outline
        .iter()
        .map(|&(x, y)| json!({"x": x - lo.0, "y": y - lo.1}))
        .collect();
    json!({
        "id": id,
        "kind": "CustomCountertop",
        "position": {"x": lo.0, "y": lo.1},
        "angle": 0.0,
        "width": hi.0 - lo.0,
        "depth": hi.1 - lo.1,
        "height": t.thickness,
        "elevation": t.top - t.thickness,
        "countertop": null,
        "backsplash": null,
        "toe_kick": null,
        "face": {"items": [], "frame_width": 1.5},
        "door_style": {},
        "drawer_style": {},
        "overlay": {"Full": {"reveal": 0.0625}},
        "framed": false,
        "label": "",
        "custom": {
            "outline": outline,
            "thickness": t.thickness,
            "edge": "Square",
            "edge_size": 0.75,
            "closed": true,
            "corner": "None",
            "corner_size": 1.0,
        },
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::import::blocks::tests::put_block;
    use crate::import::lines::tests::put_rect;
    use crate::import::tree::testutil::*;

    /// A cabinet object with the block at `at` and `tops` countertop children.
    pub fn cabinet_obj(
        at: usize,
        v: [f64; 8],
        tops: usize,
        styles: &[&str],
        soffit: bool,
    ) -> Vec<u8> {
        let kids: Vec<Vec<u8>> = (0..tops)
            .map(|_| sized(COUNTERTOP, 0, 0x60, |_| {}))
            .chain(styles.iter().map(|s| {
                sized(STYLE_ENTRY, 0, 0x100, |b| {
                    let c = cstr(s);
                    b[0x38..0x38 + c.len()].copy_from_slice(&c);
                })
            }))
            .collect();
        let extra: usize = kids.iter().map(|k| k.len() - 1).sum();
        let total = 0x500 + extra;
        sized(CABINET, 0, total, |b| {
            put_block(b, at, v);
            if soffit {
                let s = b"=soffit_material_data.quantity";
                b[0x400..0x400 + s.len()].copy_from_slice(s);
            }
            let mut p = 0x500;
            for k in &kids {
                b[p..p + k.len() - 1].copy_from_slice(&k[1..]);
                p += k.len() - 1;
            }
        })
    }

    #[test]
    fn classifies_boxes_by_elevation_height_and_strings() {
        let cases: [([f64; 8], usize, bool, BoxKind); 6] = [
            (
                [100.0, 50.0, 1.0, 0.0, 24.0, 36.0, 36.0, 36.0],
                2,
                false,
                BoxKind::Base,
            ),
            (
                [100.0, 50.0, 1.0, 0.0, 12.0, 36.0, 42.0, 96.0],
                1,
                false,
                BoxKind::Wall,
            ),
            (
                [100.0, 50.0, 1.0, 0.0, 24.0, 48.0, 96.0, 96.0],
                1,
                false,
                BoxKind::FullHeight,
            ),
            (
                [100.0, 50.0, 1.0, 0.0, 12.0, 106.0, 0.75, 70.0],
                0,
                false,
                BoxKind::Shelf,
            ),
            (
                [100.0, 50.0, 0.0, -1.0, 8.0, 243.0, 6.0, 120.25],
                0,
                true,
                BoxKind::Soffit,
            ),
            (
                [100.0, 50.0, 1.0, 0.0, 24.0, 30.0, 36.0, 36.0],
                1,
                false,
                BoxKind::Base,
            ),
        ];
        for (v, tops, soffit, kind) in cases {
            let obj = cabinet_obj(0x300, v, tops, &[], soffit);
            let tree = ObjectTree::build(&obj);
            let i = tree.of_kind(CABINET, 0).next().unwrap();
            let c = decode_cabinet(&obj, &tree, i).unwrap();
            assert_eq!(c.kind, kind, "{v:?}");
            assert_eq!(c.has_top, kind == BoxKind::Base && tops >= 2);
        }
    }

    #[test]
    fn style_names_drive_the_face_layout_and_json_shape() {
        let obj = cabinet_obj(
            0x300,
            [806.583, 623.708, 1.0, 0.0, 24.0, 41.8125, 36.0, 36.0],
            2,
            &["Lincoln Door", "Lincoln Flat Panel Drawer"],
            false,
        );
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(CABINET, 0).next().unwrap();
        let c = decode_cabinet(&obj, &tree, i).unwrap();
        assert_eq!(c.door_style.as_deref(), Some("Lincoln Door"));
        assert_eq!(c.drawer_style.as_deref(), Some("Lincoln Flat Panel Drawer"));
        let j = cabinet_json(&c, 77);
        assert_eq!(j["kind"], "Base");
        assert_eq!(j["id"], 77);
        assert_eq!(j["face"]["items"].as_array().unwrap().len(), 5);
        assert_eq!(j["countertop"]["thickness"], 1.5);
        assert_eq!(j["toe_kick"]["height"], 4.0);
        assert_eq!(j["door_style"]["name"], "Lincoln Door");
        // Front toward +x: angle -90 degrees, back-left corner above the middle.
        let a = j["angle"].as_f64().unwrap();
        assert!((a + std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        let py = j["position"]["y"].as_f64().unwrap();
        assert!((py - (623.708 + 41.8125 / 2.0)).abs() < 1e-9);

        let doors_only = cabinet_obj(
            0x300,
            [0.0, 0.0, 0.0, 1.0, 24.0, 36.0, 36.0, 36.0],
            1,
            &["Framed Panel"],
            false,
        );
        let tree = ObjectTree::build(&doors_only);
        let i = tree.of_kind(CABINET, 0).next().unwrap();
        let c = decode_cabinet(&doors_only, &tree, i).unwrap();
        let j = cabinet_json(&c, 1);
        assert_eq!(
            j["face"]["items"][1],
            json!({"DoubleDoor": {"height": 0.0}})
        );
        assert!(j["countertop"].is_null());
        let wall = cabinet_obj(
            0x300,
            [10.0, 20.0, 0.0, 1.0, 12.0, 30.0, 42.0, 96.0],
            1,
            &[],
            false,
        );
        let tree = ObjectTree::build(&wall);
        let i = tree.of_kind(CABINET, 0).next().unwrap();
        let w = decode_cabinet(&wall, &tree, i).unwrap();
        let j = cabinet_json(&w, 2);
        assert_eq!(j["elevation"], 54.0);
        assert!(j["toe_kick"].is_null());
        assert_eq!(j["face"]["items"].as_array().unwrap().len(), 5);
    }

    #[test]
    fn built_in_fixtures_shape_the_face() {
        use crate::import::symbols::tests::symbol_obj;
        let with_fixture = |name: &str| {
            let inner = symbol_obj([0.0, 0.0, 0.0, -1.0, 20.0, 20.0, 12.0, 12.0], name, &[]);
            let total = 0x500 + inner.len() - 1;
            let obj = sized(CABINET, 0, total, |b| {
                put_block(b, 0x300, [100.0, 24.0, 0.0, 1.0, 24.0, 36.0, 36.0, 36.0]);
                b[0x500..0x500 + inner.len() - 1].copy_from_slice(&inner[1..]);
            });
            let tree = ObjectTree::build(&obj);
            let i = tree.of_kind(CABINET, 0).next().unwrap();
            decode_cabinet(&obj, &tree, i).unwrap()
        };
        let sink = with_fixture("Offset Undermount Sink");
        assert_eq!(sink.fixtures, vec!["Offset Undermount Sink"]);
        let j = cabinet_json(&sink, 1);
        assert_eq!(
            j["face"]["items"][1],
            json!({"Appliance": {"height": 6.0, "name": "Sink"}})
        );
        assert!(j["appliance"].is_null());
        let dw = with_fixture("Dishwasher (panel)");
        let j = cabinet_json(&dw, 2);
        assert_eq!(j["appliance"], "Dishwasher");
        assert_eq!(j["face"]["items"], json!([{"Opening": {"height": 0.0}}]));
        // Other fixtures leave the face alone.
        let other = with_fixture("Modern Table");
        assert_eq!(
            cabinet_json(&other, 3)["face"]["items"]
                .as_array()
                .unwrap()
                .len(),
            5
        );
    }

    #[test]
    fn free_form_countertop_outline_thickness_and_top() {
        let total = 4117;
        let obj = sized(COUNTERTOP, 0, total, |b| {
            put_rect(b, 671, 908.375, 705.0, 68.0, 120.0);
            put_f64(b, total - TOP_THICKNESS_FROM_END, 1.5);
            put_f64(b, total - TOP_ELEVATION_FROM_END, 36.0);
        });
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(COUNTERTOP, 0).next().unwrap();
        let t = decode_countertop(&obj, &tree, i).unwrap();
        assert_eq!(t.outline.len(), 4);
        assert_eq!((t.thickness, t.top), (1.5, 36.0));
        let j = countertop_json(&t, 5);
        assert_eq!(j["kind"], "CustomCountertop");
        assert_eq!(j["width"], 68.0);
        assert_eq!(j["depth"], 120.0);
        assert_eq!(j["elevation"], 34.5);
        assert_eq!(j["custom"]["outline"].as_array().unwrap().len(), 4);
        // A small class 109 (a cabinet's child) is not a countertop.
        let small = sized(COUNTERTOP, 0, 1158, |_| {});
        let tree = ObjectTree::build(&small);
        let i = tree.of_kind(COUNTERTOP, 0).next().unwrap();
        assert!(decode_countertop(&small, &tree, i).is_none());
    }

    fn corner_walls() -> Vec<Wall> {
        use plan_core::model::WallKind;
        // The inside faces of a room corner at (0, 0): wall centrelines 3.25"
        // outside of it, 6.5" thick.
        vec![
            Wall::new(
                Point::new(-3.25, -3.25),
                Point::new(400.0, -3.25),
                6.5,
                96.0,
                WallKind::Interior,
            ),
            Wall::new(
                Point::new(-3.25, -3.25),
                Point::new(-3.25, 300.0),
                6.5,
                96.0,
                WallKind::Interior,
            ),
        ]
    }

    fn decoded(v: [f64; 8]) -> ChiefCabinet {
        let obj = cabinet_obj(0x300, v, 2, &["Lincoln Door"], false);
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(CABINET, 0).next().unwrap();
        decode_cabinet(&obj, &tree, i).unwrap()
    }

    #[test]
    fn a_square_box_in_a_wall_corner_is_a_corner_cabinet() {
        let walls = corner_walls();
        // Back against the south wall, facing +y: its back-left corner is the
        // room corner.
        let a = decoded([18.0, 0.0, 0.0, 1.0, 36.0, 36.0, 36.0, 36.0]);
        assert_eq!(corner_at(&a, &walls), Some(CornerAt::BackLeft));
        let mut j = cabinet_json(&a, 5);
        make_corner(&mut j, &a, CornerAt::BackLeft);
        assert_eq!(j["kind"], "CornerBase");
        assert_eq!(
            (
                j["position"]["x"].as_f64().unwrap(),
                j["position"]["y"].as_f64().unwrap()
            ),
            (0.0, 0.0)
        );
        assert_eq!(
            (j["width"].as_f64().unwrap(), j["depth"].as_f64().unwrap()),
            (36.0, 36.0)
        );
        assert_eq!(j["corner"]["arm_depth"], 24.0);
        assert_eq!(j["corner"]["style"], "Diagonal");
        // Back against the west wall, facing +x: the room corner is its
        // back-right one, so the origin goes there and the cabinet turns 90
        // degrees.
        let b = decoded([0.0, 18.0, 1.0, 0.0, 36.0, 36.0, 36.0, 36.0]);
        assert_eq!(corner_at(&b, &walls), Some(CornerAt::BackRight));
        let mut j = cabinet_json(&b, 6);
        let before = j["angle"].as_f64().unwrap();
        make_corner(&mut j, &b, CornerAt::BackRight);
        assert!((j["position"]["x"].as_f64().unwrap()).abs() < 1e-9);
        assert!((j["position"]["y"].as_f64().unwrap()).abs() < 1e-9);
        assert!((j["angle"].as_f64().unwrap() - before - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        // A wall box becomes a corner wall cabinet with 12" arms.
        let w = decoded([18.0, 0.0, 0.0, 1.0, 36.0, 36.0, 30.0, 84.0]);
        assert_eq!(w.kind, BoxKind::Wall);
        let mut j = cabinet_json(&w, 7);
        make_corner(&mut j, &w, corner_at(&w, &walls).unwrap());
        assert_eq!(j["kind"], "CornerWall");
        assert_eq!(j["corner"]["arm_depth"], 12.0);
    }

    #[test]
    fn other_boxes_stay_plain() {
        let walls = corner_walls();
        // Not square.
        let a = decoded([18.0, 0.0, 0.0, 1.0, 24.0, 36.0, 36.0, 36.0]);
        assert_eq!(corner_at(&a, &walls), None);
        // Square but in the middle of a wall (neither back corner at two walls).
        let b = decoded([100.0, 0.0, 0.0, 1.0, 36.0, 36.0, 36.0, 36.0]);
        assert_eq!(corner_at(&b, &walls), None);
        // Square and in the corner, but too small to be a corner cabinet.
        let c = decoded([9.0, 0.0, 0.0, 1.0, 18.0, 18.0, 36.0, 36.0]);
        assert_eq!(corner_at(&c, &walls), None);
        // A tall cabinet is never read as a corner.
        let d = decoded([18.0, 0.0, 0.0, 1.0, 36.0, 36.0, 84.0, 84.0]);
        assert_eq!(d.kind, BoxKind::FullHeight);
        assert_eq!(corner_at(&d, &walls), None);
        // No walls, no corner.
        assert_eq!(corner_at(&a, &[]), None);
    }
}
