//! Wall Details and the Truss Detail (manual pp. 924 and 943).
//!
//! Both are CAD details (a floor marked as a detail, see
//! `plan_core::details`), so the Text, Dimension and CAD tools annotate them,
//! the Project Browser lists them, and Send to Layout prints them. What the
//! program draws in them is remembered in the detail floor's `framing` slot
//! as a [`DetailMap`] (the ids of the CAD objects it made and what each
//! stands for), so a refresh replaces exactly those and leaves the user's
//! annotations alone.
//!
//! * A **Wall Detail** is made for every wall that has built framing, named
//!   from the wall's label, and redrawn from the wall's members every time
//!   framing is built or a member is edited ([`refresh_details`]). Each
//!   member is a closed polyline on `Detail, Framing`; the title, labels and
//!   dimensions are on `Detail, Notes` and `Detail, Lines`.
//! * The **Truss Detail** draws every truss configuration of the plan once
//!   with its web layout, the label and, when several trusses share it, the
//!   quantity in parentheses below it.

use super::{
    load, load_records, manual_members, settings, EditorContext, FramingSettings, ObjectRef, Record,
};
use plan_core::cad::{CadAttrs, CadItem, CadObject, FillAttr};
use plan_core::details::{
    CadDetailInfo, DetailSource, DETAIL_FRAMING_LAYER, DETAIL_LINES_LAYER, DETAIL_NOTES_LAYER,
};
use plan_core::geometry::Point;
use plan_core::{Floor, Id, Project, Wall};
use plan_framing::{
    truss_configs, wall_detail, wall_detail_dims, wall_detail_members, DetailDim, Member, Stroke,
    TrussConfig,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The name of the plan view (tab) that shows detail `name` (the CAD Detail
/// Management's convention).
fn tab_name(name: &str) -> String {
    format!("Detail: {name}")
}

/// Key of the one-key object that stores a detail's [`DetailMap`] in its
/// floor's `framing` slot.
const MAP_KEY: &str = "FramingDetailMap";

/// Height of the text of a Wall Detail, inches.
const TEXT: f64 = 3.0;

/// What a refresh made in a detail floor.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DetailMap {
    /// Every CAD object the program made (a refresh replaces them).
    pub generated: Vec<Id>,
    /// Wall Detail: the CAD polyline of each member and the member's index
    /// among the automatic members of the wall's floor.
    pub members: Vec<(Id, usize)>,
    /// Truss Detail: the CAD objects of each configuration.
    pub configs: Vec<ConfigLink>,
}

/// One configuration drawn in the Truss Detail.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigLink {
    pub label: String,
    /// The CAD objects of its diagram and label.
    pub cad: Vec<Id>,
    /// The manual or laid-out truss members that share it.
    pub trusses: Vec<Id>,
    /// Trusses that came from the roof framing (no manual member).
    pub auto_count: usize,
}

/// The map stored in `floor`'s framing slot (empty when there is none).
pub fn load_map(floor: &Floor) -> DetailMap {
    floor
        .framing
        .iter()
        .find_map(|v| v.as_object()?.get(MAP_KEY).cloned())
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

fn store_map(floor: &mut Floor, map: &DetailMap) {
    floor
        .framing
        .retain(|v| !v.as_object().is_some_and(|o| o.contains_key(MAP_KEY)));
    if let Ok(v) = serde_json::to_value(map) {
        let mut o = serde_json::Map::new();
        o.insert(MAP_KEY.to_string(), v);
        floor.framing.push(Value::Object(o));
    }
}

// ----- labels and lookups -----

/// The label of `wall`: the one the user typed, else the Automatic Label
/// `W<n>` (its place among the floor's walls, counting from 1).
pub fn wall_label(floor: &Floor, wall: &Wall) -> String {
    wall.extras
        .label_text
        .clone()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| {
            let n = floor
                .walls
                .iter()
                .position(|w| w.id == wall.id)
                .map_or(0, |i| i + 1);
            format!("W{n}")
        })
}

/// The building floor that holds wall `wall`.
pub fn floor_of_wall(project: &Project, wall: Id) -> Option<usize> {
    project
        .floors
        .iter()
        .position(|f| !f.is_cad_detail() && f.walls.iter().any(|w| w.id == wall))
}

/// The detail floor that is wall `wall`'s Wall Detail.
pub fn wall_detail_floor(project: &Project, wall: Id) -> Option<usize> {
    project.floors.iter().position(|f| {
        f.detail
            .as_ref()
            .is_some_and(|d| d.source == DetailSource::WallDetail { wall })
    })
}

/// The wall a Wall Detail floor shows.
pub fn detail_wall(floor: &Floor) -> Option<Id> {
    match floor.detail.as_ref()?.source {
        DetailSource::WallDetail { wall } => Some(wall),
        _ => None,
    }
}

/// The Truss Detail floor.
pub fn truss_detail_floor(project: &Project) -> Option<usize> {
    project
        .floors
        .iter()
        .position(|f| f.detail.as_ref().is_some_and(|d| d.source == DetailSource::TrussDetail))
}

/// Every Wall Detail as `(detail floor, building floor, wall id, label)`, in
/// floor order: what the Project Browser lists.
pub fn wall_details(project: &Project) -> Vec<(usize, usize, Id, String)> {
    let mut out = Vec::new();
    for (di, f) in project.floors.iter().enumerate() {
        let Some(wall) = detail_wall(f) else { continue };
        let Some(fi) = floor_of_wall(project, wall) else {
            continue;
        };
        let label = project.floors[fi]
            .walls
            .iter()
            .find(|w| w.id == wall)
            .map_or_else(|| f.name.clone(), |w| wall_label(&project.floors[fi], w));
        out.push((di, fi, wall, label));
    }
    out.sort_by_key(|(_, fi, _, _)| *fi);
    out
}

// ----- generating the drawing -----

/// A generated CAD object waiting for an id.
struct Piece {
    layer: &'static str,
    item: CadItem,
    /// The member (index among the floor's automatic members) it draws.
    member: Option<usize>,
    fill: Option<FillAttr>,
}

fn line(layer: &'static str, a: Point, b: Point) -> Piece {
    Piece {
        layer,
        item: CadItem::Line { a, b },
        member: None,
        fill: None,
    }
}

fn text(layer: &'static str, pos: Point, text: String, height: f64) -> Piece {
    Piece {
        layer,
        item: CadItem::Text {
            pos,
            text,
            height,
            angle: 0.0,
        },
        member: None,
        fill: None,
    }
}

/// The dimension `d` as a line with extension ticks and its text, offset from
/// the members as the detail says.
fn dimension_pieces(d: &DetailDim, flip: &dyn Fn(Point) -> Point) -> Vec<Piece> {
    let (a, b) = (flip(d.a), flip(d.b));
    let horizontal = (a.y - b.y).abs() < 1e-6;
    let (a2, b2, tick) = if horizontal {
        let dy = if d.offset >= 0.0 { d.offset } else { d.offset };
        (
            Point::new(a.x, a.y + dy),
            Point::new(b.x, b.y + dy),
            Point::new(0.0, d.offset.signum() * 2.0),
        )
    } else {
        // Along the wall's height the offset is to the side; mirrored views
        // put it on the other side.
        let flipped = flip(Point::new(1.0, 0.0)).x < flip(Point::new(0.0, 0.0)).x;
        let dx = if flipped { -d.offset } else { d.offset };
        (
            Point::new(a.x + dx, a.y),
            Point::new(b.x + dx, b.y),
            Point::new(dx.signum() * 2.0, 0.0),
        )
    };
    let mid = Point::lerp(a2, b2, 0.5);
    vec![
        line(DETAIL_LINES_LAYER, a2, b2),
        line(DETAIL_LINES_LAYER, a, a2 + tick),
        line(DETAIL_LINES_LAYER, b, b2 + tick),
        text(
            DETAIL_NOTES_LAYER,
            if horizontal {
                mid + Point::new(-8.0, 1.0)
            } else {
                mid + Point::new(1.0, 0.0)
            },
            d.text.clone(),
            TEXT * 0.8,
        ),
    ]
}

/// The pieces of the drawing of `wall` from its members `members` (the
/// automatic members of the wall's floor), seen from the exterior or the
/// interior.
fn wall_pieces(
    floor: &Floor,
    wall: &Wall,
    members: &[Member],
    st: &FramingSettings,
) -> Vec<Piece> {
    let len = wall.length();
    // Seen from outside, the wall's start is on the viewer's left when the
    // exterior is on the right of start-to-end.
    let exterior_on_left = wall.exterior_side == plan_core::walls::Side::Left;
    let flip_x = exterior_on_left == st.build.detail.details_from_exterior;
    let flip = move |p: Point| {
        if flip_x {
            Point::new(len - p.x, p.y)
        } else {
            p
        }
    };
    let fill = st.build.detail.wall_detail_fill.as_ref().and_then(|f| {
        f.solid_rgba([140, 95, 50], [255, 255, 255]).map(|c| FillAttr {
            color: [c[0], c[1], c[2]],
            opacity: c[3],
            ..FillAttr::default()
        })
    });
    let list = wall_detail_members(wall, members);
    let mut rects = list.iter();
    let mut out = Vec::new();
    for s in wall_detail(wall, members) {
        match s {
            Stroke::Line(a, b) => out.push(line(DETAIL_LINES_LAYER, flip(a), flip(b))),
            Stroke::Rect { min, max } => {
                let (a, b) = (flip(min), flip(max));
                let (lo, hi) = (
                    Point::new(a.x.min(b.x), a.y.min(b.y)),
                    Point::new(a.x.max(b.x), a.y.max(b.y)),
                );
                out.push(Piece {
                    layer: DETAIL_FRAMING_LAYER,
                    item: CadItem::Polyline {
                        points: vec![
                            lo,
                            Point::new(hi.x, lo.y),
                            hi,
                            Point::new(lo.x, hi.y),
                        ],
                        closed: true,
                    },
                    member: rects.next().map(|m| m.index),
                    fill: fill.clone(),
                });
            }
            Stroke::Text {
                pos, text: t, height, ..
            } => out.push(text(DETAIL_NOTES_LAYER, flip(pos), t, height.max(TEXT * 0.6))),
        }
    }
    for d in wall_detail_dims(wall, members) {
        out.extend(dimension_pieces(&d, &flip));
    }
    let side = if st.build.detail.details_from_exterior {
        "exterior"
    } else {
        "interior"
    };
    out.push(text(
        DETAIL_NOTES_LAYER,
        Point::new(0.0, -24.0),
        format!(
            "{}: {} framing layer, seen from the {side}",
            wall_label(floor, wall),
            wall.wall_type.as_deref().unwrap_or("Wall"),
        ),
        TEXT,
    ));
    out
}

/// Replaces what a refresh made earlier in detail floor `idx` with `pieces`
/// and stores the map. The user's own objects stay.
fn replace_generated(
    project: &mut Project,
    idx: usize,
    pieces: Vec<Piece>,
    mut map: DetailMap,
    configs: Vec<(usize, Vec<usize>)>,
) -> DetailMap {
    let old = load_map(&project.floors[idx]);
    {
        let f = &mut project.floors[idx];
        f.cad.retain(|o| !old.generated.contains(&o.id));
        f.cad_attrs.retain(|a| !old.generated.contains(&a.target));
    }
    map.generated.clear();
    map.members.clear();
    let mut made: Vec<Id> = Vec::with_capacity(pieces.len());
    for p in pieces {
        let id = project.alloc_id();
        made.push(id);
        map.generated.push(id);
        if let Some(m) = p.member {
            map.members.push((id, m));
        }
        if let Some(fill) = p.fill {
            let mut a = CadAttrs::new(id);
            a.fill = Some(fill);
            project.floors[idx].cad_attrs.push(a);
        }
        project.floors[idx].cad.push(CadObject {
            id,
            layer: p.layer.to_string(),
            item: p.item,
        });
    }
    // Truss Detail: which of the made objects belong to which configuration.
    for (ci, range) in configs {
        if let Some(link) = map.configs.get_mut(ci) {
            link.cad = range.into_iter().filter_map(|i| made.get(i).copied()).collect();
        }
    }
    store_map(&mut project.floors[idx], &map);
    map
}

// ----- Wall Details -----

/// Makes or redraws the Wall Detail of `wall` (a wall of floor `fi` with
/// built framing), returning the detail floor's index. No undo step of its
/// own: callers run inside the step that changed the framing.
pub fn refresh_wall_detail(project: &mut Project, fi: usize, wall: Id) -> Option<usize> {
    let st = settings(project);
    let floor = &project.floors[fi];
    let w = floor.walls.iter().find(|w| w.id == wall)?.clone();
    let members = load(floor);
    if !members.iter().any(|m| m.wall_id == Some(wall)) {
        return None;
    }
    let label = wall_label(floor, &w);
    let pieces = wall_pieces(floor, &w, &members, &st);
    let idx = match wall_detail_floor(project, wall) {
        Some(i) => i,
        None => project.add_cad_detail(
            &label,
            CadDetailInfo::from_source(DetailSource::WallDetail { wall }),
        ),
    };
    // The name follows the wall's label until the user renames the detail.
    if project.floors[idx]
        .detail
        .as_ref()
        .is_some_and(|d| !d.name_locked)
        && project.floors[idx].name != label
    {
        let unique = project.unique_floor_name(&label, Some(idx));
        if let Some(old) = Some(project.floors[idx].name.clone()) {
            let (old_tab, new_tab) = (
                tab_name(&old),
                tab_name(&unique),
            );
            if let Some(v) = project.plan_views.iter_mut().find(|v| v.name == old_tab) {
                v.name = new_tab.clone();
            }
            if project.active_plan_view == old_tab {
                project.active_plan_view = new_tab;
            }
        }
        project.floors[idx].name = unique;
    }
    replace_generated(project, idx, pieces, DetailMap::default(), Vec::new());
    Some(idx)
}

/// Redraws every Wall Detail and the Truss Detail from the framing as it is:
/// a detail is made for each wall that has members, redrawn when it has, and
/// dropped (with its drawing) when its wall is gone or has no members left
/// and the user drew nothing of their own in it. No undo step of its own.
pub fn refresh_details(cx: &mut EditorContext) {
    refresh_details_in(&mut cx.project);
    // Floors may have been added or removed after the active one.
    if cx.floor >= cx.project.floors.len() {
        cx.floor = cx.project.floors.len().saturating_sub(1);
    }
}

/// [`refresh_details`] on the project alone.
pub fn refresh_details_in(project: &mut Project) {
    for fi in 0..project.floors.len() {
        if project.floors[fi].is_cad_detail() {
            continue;
        }
        let with_members: Vec<Id> = {
            let f = &project.floors[fi];
            let members = load(f);
            f.walls
                .iter()
                .filter(|w| members.iter().any(|m| m.wall_id == Some(w.id)))
                .map(|w| w.id)
                .collect()
        };
        for wall in with_members {
            refresh_wall_detail(project, fi, wall);
        }
    }
    // Drop the Wall Details that no longer have a wall with members.
    let stale: Vec<usize> = (0..project.floors.len())
        .filter(|&i| {
            let f = &project.floors[i];
            let Some(wall) = detail_wall(f) else {
                return false;
            };
            let has = floor_of_wall(project, wall).is_some_and(|fi| {
                load(&project.floors[fi])
                    .iter()
                    .any(|m| m.wall_id == Some(wall))
            });
            !has && user_objects(f) == 0
        })
        .collect();
    for i in stale.into_iter().rev() {
        project.delete_cad_detail(i);
    }
    refresh_truss_detail_in(project);
}

/// How many CAD objects of a detail floor the user drew (not the program).
fn user_objects(f: &Floor) -> usize {
    let map = load_map(f);
    f.cad.iter().filter(|o| !map.generated.contains(&o.id)).count()
}

/// Opens the Wall Detail of `wall` in its own tab. The detail exists once the
/// wall's framing is built; without framing there is nothing to open.
pub fn open_wall_detail(cx: &mut EditorContext, wall: Id) -> bool {
    let Some(fi) = floor_of_wall(&cx.project, wall) else {
        return false;
    };
    let idx = match wall_detail_floor(&cx.project, wall) {
        Some(i) => i,
        None => {
            // A wall whose framing was built before Wall Details existed: make it now.
            if !load(&cx.project.floors[fi])
                .iter()
                .any(|m| m.wall_id == Some(wall))
            {
                cx.status = "Build the wall's framing first: a Wall Detail shows its members".into();
                return false;
            }
            cx.begin_change("Open Wall Detail");
            let made = refresh_wall_detail(&mut cx.project, fi, wall);
            cx.mark_dirty();
            match made {
                Some(i) => i,
                None => {
                    cx.cancel_change();
                    return false;
                }
            }
        }
    };
    crate::tools::details::open_detail(cx, idx)
}

/// The members a Wall Detail's selected CAD objects draw, as indexes among the
/// automatic members of the wall's floor. Empty outside a Wall Detail.
pub fn selected_wall_members(cx: &EditorContext) -> Vec<usize> {
    let map = load_map(cx.floor());
    cx.selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Cad(id) => map.members.iter().find(|(c, _)| c == id).map(|(_, m)| *m),
            _ => None,
        })
        .collect()
}

// ----- the Truss Detail -----

/// Gap between diagrams in the Truss Detail, inches.
const GAP: f64 = 48.0;

/// Draws one configuration's diagram with its label at `origin` (the left end
/// of the bearing line); returns the pieces and the height used.
fn config_pieces(c: &TrussConfig, origin: Point) -> (Vec<Piece>, f64) {
    let mut out = Vec::new();
    let min_x = c
        .members
        .iter()
        .flat_map(|m| [m.a.x, m.b.x])
        .fold(0.0, f64::min);
    let at = |p: Point| Point::new(origin.x + p.x - min_x, origin.y + p.y);
    for m in &c.members {
        let poly = m.polygon();
        out.push(Piece {
            layer: DETAIL_FRAMING_LAYER,
            item: CadItem::Polyline {
                points: poly.iter().map(|p| at(*p)).collect(),
                closed: true,
            },
            member: None,
            fill: None,
        });
    }
    // The bearing line.
    out.push(line(
        DETAIL_LINES_LAYER,
        at(Point::new(0.0, 0.0)),
        at(Point::new(c.span, 0.0)),
    ));
    let label = if c.count > 1 {
        format!("{} ({})", c.label, c.count)
    } else {
        c.label.clone()
    };
    out.push(text(
        DETAIL_NOTES_LAYER,
        origin + Point::new(0.0, -14.0),
        label,
        TEXT * 1.5,
    ));
    (out, c.height().max(12.0) + 24.0)
}

/// The manual and laid-out truss members of every building floor, in floor
/// order (the order trusses were made).
pub fn all_truss_members(project: &Project) -> Vec<plan_framing::FramingMember> {
    project
        .floors
        .iter()
        .filter(|f| !f.is_cad_detail())
        .flat_map(manual_members)
        .filter(|m| m.kind.is_truss())
        .collect()
}

/// The roof framing's own trusses, floor by floor.
fn all_auto_trusses(project: &Project) -> Vec<Member> {
    project
        .floors
        .iter()
        .filter(|f| !f.is_cad_detail())
        .flat_map(load)
        .collect()
}

/// Every truss configuration of the plan with its label and count.
pub fn configs(project: &Project) -> Vec<TrussConfig> {
    truss_configs(&all_truss_members(project), &all_auto_trusses(project))
}

/// Makes, redraws or drops the Truss Detail. Dropped when no trusses are left
/// and the user drew nothing in it.
pub fn refresh_truss_detail_in(project: &mut Project) {
    let configs = configs(project);
    let existing = truss_detail_floor(project);
    if configs.is_empty() {
        if let Some(i) = existing {
            if user_objects(&project.floors[i]) == 0 {
                project.delete_cad_detail(i);
            }
        }
        return;
    }
    let idx = existing.unwrap_or_else(|| {
        project.add_cad_detail(
            "Truss Detail",
            CadDetailInfo::from_source(DetailSource::TrussDetail),
        )
    });
    let mut pieces: Vec<Piece> = Vec::new();
    let mut map = DetailMap::default();
    let mut ranges: Vec<(usize, Vec<usize>)> = Vec::new();
    let mut y = 0.0;
    for (ci, c) in configs.iter().enumerate() {
        let before = pieces.len();
        let (p, used) = config_pieces(c, Point::new(0.0, y));
        pieces.extend(p);
        ranges.push((ci, (before..pieces.len()).collect()));
        map.configs.push(ConfigLink {
            label: c.label.clone(),
            cad: Vec::new(),
            trusses: c.ids.clone(),
            auto_count: if c.spec.is_none() { c.count } else { 0 },
        });
        y += used + GAP;
    }
    replace_generated(project, idx, pieces, map, ranges);
}

/// Opens the Truss Detail in its own tab and selects the diagram of the
/// configuration truss `truss` (a manual member) has, so the view shows it.
pub fn open_truss_detail(cx: &mut EditorContext, truss: Option<Id>) -> bool {
    if truss_detail_floor(&cx.project).is_none() {
        cx.begin_change("Truss Detail");
        refresh_truss_detail_in(&mut cx.project);
        cx.mark_dirty();
        if truss_detail_floor(&cx.project).is_none() {
            cx.cancel_change();
            cx.status = "There are no trusses: draw a truss or build the roof with trusses".into();
            return false;
        }
    }
    let Some(idx) = truss_detail_floor(&cx.project) else {
        return false;
    };
    let cad: Vec<Id> = truss
        .and_then(|id| {
            load_map(&cx.project.floors[idx])
                .configs
                .into_iter()
                .find(|c| c.trusses.contains(&id))
        })
        .map(|c| c.cad)
        .unwrap_or_default();
    let opened = crate::tools::details::open_detail(cx, idx);
    if opened && !cad.is_empty() {
        cx.selection.items = cad.into_iter().map(ObjectRef::Cad).collect();
    }
    opened
}

/// Find Trusses: leaves the Truss Detail for the plan of the floor the
/// trusses of the configuration drawn by the selected CAD objects stand on,
/// with those trusses selected. Returns how many were found.
pub fn find_trusses(cx: &mut EditorContext) -> usize {
    let map = load_map(cx.floor());
    let picked: Vec<Id> = cx
        .selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Cad(id) => Some(*id),
            _ => None,
        })
        .collect();
    let ids: Vec<Id> = map
        .configs
        .iter()
        .filter(|c| c.cad.iter().any(|x| picked.contains(x)))
        .flat_map(|c| c.trusses.iter().copied())
        .collect();
    if ids.is_empty() {
        cx.status = "Select a truss drawing to find its trusses".into();
        return 0;
    }
    let Some(fi) = cx.project.floors.iter().position(|f| {
        !f.is_cad_detail() && load_records(f).iter().any(|r| ids.contains(&r.id()))
    }) else {
        return 0;
    };
    let on_floor: Vec<Id> = load_records(&cx.project.floors[fi])
        .iter()
        .filter(|r| matches!(r, Record::Manual(_) | Record::Built(_)))
        .map(Record::id)
        .filter(|id| ids.contains(id))
        .collect();
    cx.floor = fi;
    cx.reset_view_state();
    super::select(cx, on_floor.clone());
    cx.status = format!("Found {} truss(es)", on_floor.len());
    on_floor.len()
}

/// Find Trusses for the configuration labelled `label` (from the Truss Detail
/// window or schedule): goes to the floor of the first manual truss that has
/// it and selects every truss of the configuration on that floor. Returns how
/// many were selected.
pub fn find_config(cx: &mut EditorContext, label: &str) -> usize {
    let Some(c) = configs(&cx.project).into_iter().find(|c| c.label == label) else {
        return 0;
    };
    let Some(fi) = cx.project.floors.iter().position(|f| {
        !f.is_cad_detail() && load_records(f).iter().any(|r| c.ids.contains(&r.id()))
    }) else {
        cx.status = format!("{label}: the trusses came from the roof framing; see the Framing Overview");
        return 0;
    };
    let on_floor: Vec<Id> = load_records(&cx.project.floors[fi])
        .iter()
        .map(Record::id)
        .filter(|id| c.ids.contains(id))
        .collect();
    if cx.floor != fi {
        cx.floor = fi;
        cx.reset_view_state();
    }
    super::select(cx, on_floor.clone());
    cx.status = format!("Found {} truss(es) of {label}", on_floor.len());
    on_floor.len()
}

/// Whether the active floor is a Wall Detail.
pub fn in_wall_detail(cx: &EditorContext) -> bool {
    detail_wall(cx.floor()).is_some()
}

/// Whether the active floor is the Truss Detail.
pub fn in_truss_detail(cx: &EditorContext) -> bool {
    cx.floor().detail.as_ref().is_some_and(|d| d.source == DetailSource::TrussDetail)
}
