//! Moving, rotating, resizing, mirroring and replicating the selection
//! (S-47, S-48, S-52, S-53, S-101..S-106, DW-23), Align/Distribute (S-54),
//! Make Parallel/Perpendicular (S-41) and the click-driven modes behind them
//! (cursor-attached Paste, Point to Point Move, Reflect About Object, Center
//! Object).
//!
//! The geometry is [`plan_core::transform::Xform`]; this module applies it to
//! every kind the editor stores:
//!
//! | kind | move | rotate | mirror | resize |
//! |---|---|---|---|---|
//! | walls (with openings), dimensions, CAD, text, symbols, cameras | yes | yes | yes | yes |
//! | cabinets, devices | yes | yes | yes | position and size |
//! | stairs and landings | yes | yes | yes | position |
//! | roof planes, framing, foundation, details, schedules, terrain | yes | no | no | no |
//!
//! A command that meets a kind it cannot turn leaves that object alone and
//! says so in the status bar.

use super::selection::{hit_test_cx, ObjectRef};
use super::{details_view, ops, placed, roof_view, site_view, stairs_view, Camera, EditorContext};
use crate::tools::{PointerEvent, ToolResult};
use eframe::egui::{self, Stroke};
use plan_cabinets::{FaceCell, FaceItem, FaceLayout};
use plan_core::cad::CadItem;
use plan_core::geometry::Point;
use plan_core::groups::ObjectRef as CoreRef;
use plan_core::transform::{
    align_offsets, bounds_of, distribute_offsets, parallel_end, union_box, AlignMode, Axis, Xform,
};
use plan_core::{Id, WallEnd};
use std::cell::RefCell;
use std::f64::consts::PI;

// ----- bounds -----

/// The points that bound `o` in plan (a box's corners, a path's vertices).
pub fn object_points(cx: &EditorContext, o: ObjectRef) -> Vec<Point> {
    let f = cx.floor();
    if let Some(core) = o.to_group_ref() {
        use plan_core::ObjectRef as G;
        if matches!(
            core,
            G::Wall(_) | G::Opening(_) | G::Dimension(_) | G::Cad(_) | G::Symbol(_) | G::Camera(_)
        ) {
            return plan_core::transform::object_bounds(&cx.project, cx.floor, core)
                .map(|(lo, hi)| vec![lo, hi])
                .unwrap_or_default();
        }
    }
    match o {
        ObjectRef::Cabinet(id) => placed::cabinet_by_id(f, id)
            .map(|c| c.corners().to_vec())
            .unwrap_or_default(),
        ObjectRef::Stair(id) => stairs_view::find(f, id)
            .map(|s| s.footprint())
            .unwrap_or_default(),
        ObjectRef::Device(id) => site_view::electrical_layer(cx.floor, f)
            .device(id)
            .map(|d| vec![d.position])
            .unwrap_or_default(),
        ObjectRef::RoofPlane(id) => roof_view::load(f)
            .pick_polys()
            .into_iter()
            .filter(|(i, _, _)| *i == id)
            .flat_map(|(_, _, polys)| polys.into_iter().flatten())
            .collect(),
        ObjectRef::Foundation(id) => {
            let layer = plan_core::foundation::FoundationLayer::load(f);
            layer
                .find(id)
                .and_then(|r| super::foundation_view::outline_points(&layer, r))
                .unwrap_or_default()
        }
        ObjectRef::Framing(id) => super::framing_view::find(f, id)
            .map(|r| r.extent())
            .unwrap_or_default(),
        ObjectRef::Detail(id) => plan_core::details::DetailsLayer::load(f)
            .find(id)
            .and_then(|r| details_view::vertices(cx, r))
            .unwrap_or_default(),
        ObjectRef::Solid(id) => super::solids_view::outline_points(f, id),
        ObjectRef::Block(id) => f
            .blocks
            .flat_members(id)
            .into_iter()
            .flat_map(|m| object_points(cx, ObjectRef::from_group_ref(m)))
            .collect(),
        ObjectRef::Schedule(id) => super::schedule_view::extents(cx)
            .into_iter()
            .filter(|(i, _, _)| *i == id)
            .flat_map(|(_, lo, hi)| [lo, hi])
            .collect(),
        ObjectRef::TerrainObject(h) => site_view::terrain_view(&cx.project)
            .map(|v| site_view::hit_points(&v.record.terrain, h))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// The box around one object, if it has a place in the plan.
pub fn object_bounds(cx: &EditorContext, o: ObjectRef) -> Option<(Point, Point)> {
    bounds_of(object_points(cx, o))
}

/// The box around every object of `items`.
pub fn items_bounds(cx: &EditorContext, items: &[ObjectRef]) -> Option<(Point, Point)> {
    items
        .iter()
        .filter_map(|o| object_bounds(cx, *o))
        .reduce(union_box)
}

/// The center of the box around the selection.
pub fn selection_center(cx: &EditorContext) -> Option<Point> {
    items_bounds(cx, &cx.selection.items)
        .map(|(lo, hi)| Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5))
}

// ----- applying a transform -----

/// What an application of a transform did.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Report {
    pub changed: usize,
    /// Type names of objects the transform cannot be applied to.
    pub skipped: Vec<&'static str>,
}

impl Report {
    fn skip(&mut self, o: ObjectRef) {
        let n = o.type_name();
        if !self.skipped.contains(&n) {
            self.skipped.push(n);
        }
    }

    /// `"Rotated 3 objects"` plus a note on what was left out.
    pub fn status(&self, verb: &str) -> String {
        let mut s = format!(
            "{verb} {} object{}",
            self.changed,
            if self.changed == 1 { "" } else { "s" }
        );
        if !self.skipped.is_empty() {
            s.push_str(&format!(" ({} cannot be turned)", self.skipped.join(", ")));
        }
        s
    }
}

/// Moves `items` by `d` without snapping: walls take the walls joined to
/// them along (as a free move does), openings slide along their wall, and
/// every other kind shifts. No undo step is opened.
pub fn translate_objects(cx: &mut EditorContext, items: &[ObjectRef], d: Point) {
    if d.length() < 1e-9 {
        return;
    }
    let fl = cx.floor;
    let before = cx.floor().walls.clone();
    let walls: Vec<Id> = items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Wall(id) => Some(*id),
            _ => None,
        })
        .collect();
    ops::translate_walls_with_followers(&mut cx.project, fl, &walls, d);
    cx.translate_extra(items, d);
    for o in items {
        match *o {
            ObjectRef::Opening(id) => {
                let Some(op) = cx.floor().openings.iter().find(|x| x.id == id).cloned() else {
                    continue;
                };
                if walls.contains(&op.wall_id) {
                    continue;
                }
                if let Some(dir) = cx.floor().wall(op.wall_id).map(|w| w.direction()) {
                    ops::place_opening_at(
                        &mut cx.project,
                        fl,
                        id,
                        op.wall_id,
                        op.center_offset + d.dot(dir),
                    );
                }
            }
            ObjectRef::Dimension(id) => {
                if let Some(dim) = cx.project.floors[fl]
                    .dimensions
                    .iter_mut()
                    .find(|x| x.id == id)
                {
                    dim.translate(d);
                }
            }
            ObjectRef::Cad(id) | ObjectRef::Text(id) => {
                if let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) {
                    ops::translate_cad(&mut c.item, d);
                }
            }
            _ => {}
        }
    }
    details_view::follow_walls(&mut cx.project, fl, &before);
    placed::sync_distributions(cx);
    cx.mark_dirty();
}

/// Mirrors the hinges and the left-to-right order of a cabinet face.
fn mirror_item(item: &FaceItem) -> FaceItem {
    match item {
        FaceItem::DoorLeft { height } => FaceItem::DoorRight { height: *height },
        FaceItem::DoorRight { height } | FaceItem::DoorAuto { height } => {
            FaceItem::DoorLeft { height: *height }
        }
        FaceItem::HorizontalLayout { height, cells } => FaceItem::HorizontalLayout {
            height: *height,
            cells: cells
                .iter()
                .rev()
                .map(|c| FaceCell {
                    item: mirror_item(&c.item),
                    width: c.width,
                })
                .collect(),
        },
        other => other.clone(),
    }
}

fn mirror_face(layout: &FaceLayout) -> FaceLayout {
    FaceLayout {
        items: layout.items.iter().map(mirror_item).collect(),
        frame_width: layout.frame_width,
    }
}

/// Applies `x` to the objects of `items` on the active floor. A pure shift
/// goes through [`translate_objects`]. No undo step is opened.
pub fn apply_xform(cx: &mut EditorContext, items: &[ObjectRef], x: &Xform) -> Report {
    let mut rep = Report::default();
    if x.is_translation() {
        translate_objects(cx, items, x.shift());
        rep.changed = items.len();
        return rep;
    }
    let fl = cx.floor;
    let before = cx.floor().walls.clone();
    let k = x.scale_factor();
    let mirror = x.is_mirror();
    let walls: Vec<Id> = items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Wall(id) => Some(*id),
            _ => None,
        })
        .collect();
    // The kinds plan-core knows.
    let core: Vec<CoreRef> = items
        .iter()
        .filter(|o| {
            matches!(
                o,
                ObjectRef::Wall(_)
                    | ObjectRef::Dimension(_)
                    | ObjectRef::Cad(_)
                    | ObjectRef::Text(_)
                    | ObjectRef::Symbol(_)
                    | ObjectRef::Camera(_)
            )
        })
        .filter_map(|o| o.to_group_ref())
        .collect();
    rep.changed += cx.project.transform_objects(fl, &core, x);
    for o in items {
        match *o {
            ObjectRef::Cabinet(id) => {
                if let Some(mut c) = placed::cabinet_by_id(cx.floor(), id) {
                    let theta = c.angle;
                    if mirror {
                        // The old back-right corner becomes the back-left one.
                        let right = c.position + Point::new(theta.cos(), theta.sin()) * c.width;
                        c.position = x.apply(right);
                        c.angle = x.map_angle(theta) + PI;
                        c.face = mirror_face(&c.face);
                    } else {
                        c.position = x.apply(c.position);
                        c.angle = x.map_angle(theta);
                    }
                    c.width *= k;
                    c.depth *= k;
                    placed::replace_cabinet(&mut cx.project, fl, &c);
                    rep.changed += 1;
                }
            }
            ObjectRef::Stair(id) => {
                let done = stairs_view::update(&mut cx.project, fl, id, |s| {
                    let st = &mut s.stair;
                    let right = Point::new(st.direction.sin(), -st.direction.cos());
                    if mirror {
                        st.origin = x.apply(st.origin + right * st.params.width);
                        std::mem::swap(&mut st.params.left_side, &mut st.params.right_side);
                        st.params.turn = match st.params.turn {
                            plan_stairs::Turn::Left => plan_stairs::Turn::Right,
                            plan_stairs::Turn::Right => plan_stairs::Turn::Left,
                        };
                    } else {
                        st.origin = x.apply(st.origin);
                    }
                    st.direction = x.map_angle(st.direction);
                    for p in st.params.outline.iter_mut() {
                        *p = x.apply(*p);
                    }
                });
                if done {
                    rep.changed += 1;
                }
            }
            ObjectRef::Device(id) => {
                let mut layer = site_view::load_electrical(cx.floor());
                if let Some(d) = layer.device_mut(id) {
                    d.position = x.apply(d.position);
                    d.angle = x.map_angle(d.angle);
                    // A device turned off its wall no longer hangs on it.
                    if d.wall_id.is_some_and(|w| !walls.contains(&w)) {
                        d.wall_id = None;
                    }
                    site_view::save_electrical(&mut cx.project, fl, &layer);
                    rep.changed += 1;
                }
            }
            // Handled by `transform_objects` above, or follow their wall.
            ObjectRef::Wall(_)
            | ObjectRef::Opening(_)
            | ObjectRef::Dimension(_)
            | ObjectRef::Cad(_)
            | ObjectRef::Text(_)
            | ObjectRef::Symbol(_)
            | ObjectRef::Camera(_) => {}
            // A compound 3D solid turns, mirrors and scales as one mesh.
            ObjectRef::Solid(id) => {
                if let Some(c) = cx.project.floors[fl].solid_layer.compound_mut(id) {
                    c.xform(x);
                    rep.changed += 1;
                }
            }
            // A 3D solid primitive carries its position, turn and size.
            ObjectRef::Detail(id) => {
                let mut layer = details_view::load(cx);
                if let Some(s) = layer.solids.iter_mut().find(|s| s.id == id) {
                    plan_core::solids::xform_solid(s, x);
                    details_view::save(&mut cx.project, fl, &layer);
                    rep.changed += 1;
                } else {
                    rep.skip(*o);
                }
            }
            other => rep.skip(other),
        }
    }
    if !walls.is_empty() {
        details_view::follow_walls(&mut cx.project, fl, &before);
    }
    placed::sync_distributions(cx);
    cx.mark_dirty();
    rep
}

// ----- Transform/Replicate -----

/// Where Reflect puts its mirror line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReflectAxis {
    /// The vertical line `x = value` (mirrors left to right).
    Vertical(f64),
    /// The horizontal line `y = value` (mirrors top to bottom).
    Horizontal(f64),
    /// The line through two points.
    Line(Point, Point),
}

impl ReflectAxis {
    pub fn xform(self) -> Xform {
        match self {
            ReflectAxis::Vertical(x) => Xform::reflect(Point::new(x, 0.0), Point::new(x, 1.0)),
            ReflectAxis::Horizontal(y) => Xform::reflect(Point::new(0.0, y), Point::new(1.0, y)),
            ReflectAxis::Line(a, b) => Xform::reflect(a, b),
        }
    }
}

/// The Transform/Replicate Object dialog's values (S-103).
#[derive(Debug, Clone, PartialEq)]
pub struct TransformParams {
    /// Copies to make; `0` changes the selected objects themselves.
    pub copies: u32,
    pub move_x: f64,
    pub move_y: f64,
    /// Counter-clockwise degrees.
    pub rotate_deg: f64,
    /// Rotate about this point; `None` is the center of the selection.
    pub rotate_about: Option<Point>,
    /// Resize factor (1 leaves the size); about the center of the selection.
    pub resize: f64,
    pub reflect: Option<ReflectAxis>,
}

impl Default for TransformParams {
    fn default() -> Self {
        Self {
            copies: 0,
            move_x: 0.0,
            move_y: 0.0,
            rotate_deg: 0.0,
            rotate_about: None,
            resize: 1.0,
            reflect: None,
        }
    }
}

impl TransformParams {
    /// The one step: resize, then mirror, then rotate, then move (copy k
    /// applies it k times, so a move makes a linear array and a rotation a
    /// radial one, S-104).
    pub fn xform(&self, center: Point) -> Xform {
        let mut x = Xform::IDENTITY;
        if (self.resize - 1.0).abs() > 1e-9 && self.resize > 0.0 {
            x = x.then(Xform::scale(center, self.resize));
        }
        if let Some(axis) = self.reflect {
            x = x.then(axis.xform());
        }
        if self.rotate_deg.abs() > 1e-9 {
            let about = self.rotate_about.unwrap_or(center);
            x = x.then(Xform::rotate(about, self.rotate_deg.to_radians()));
        }
        if self.move_x.abs() > 1e-9 || self.move_y.abs() > 1e-9 {
            x = x.then(Xform::translate(Point::new(self.move_x, self.move_y)));
        }
        x
    }
}

/// Transform/Replicate Object (S-47, S-103, S-104): one undo step. With
/// `copies = 0` the selected objects are changed; with N copies the
/// originals stay and copy k is the transform applied k times. The copies
/// become the selection. Locked layers refuse. Returns the status text.
pub fn transform_replicate(cx: &mut EditorContext, p: &TransformParams) -> Result<String, String> {
    if cx.selection.is_empty() {
        return Err("Select objects to transform".into());
    }
    let items = cx.selection.items.clone();
    if p.copies == 0 && items.iter().any(|o| !cx.check_unlocked(*o)) {
        return Err(cx.status.clone());
    }
    let center = selection_center(cx).ok_or("The selection has no place in the plan")?;
    let x = p.xform(center);
    if x.is_identity() {
        return Err("Nothing to do: every value leaves the objects as they are".into());
    }
    if p.copies == 0 {
        cx.begin_change("Transform/Replicate");
        let rep = apply_xform(cx, &items, &x);
        cx.selection.retain_existing(&cx.project, cx.floor);
        cx.mark_dirty();
        return Ok(rep.status("Transformed"));
    }
    let mut clip = super::Clipboard::capture(cx);
    if clip.is_empty() {
        return Err("Those objects cannot be copied".into());
    }
    let pure_shift = x.is_translation();
    if !pure_shift {
        // Kinds that cannot be turned are not copied by a rotation or mirror.
        clip.drop_unturnable();
    }
    cx.begin_change("Transform/Replicate");
    let mut all: Vec<ObjectRef> = Vec::new();
    let mut skipped = Report::default();
    for k in 1..=p.copies {
        let step = x.pow(k);
        if pure_shift {
            all.extend(clip.paste(cx, step.shift(), false));
        } else {
            let fresh = clip.paste(cx, Point::ZERO, false);
            let r = apply_xform(cx, &fresh, &step);
            skipped.skipped.extend(r.skipped);
            all.extend(fresh);
        }
    }
    if all.is_empty() {
        cx.cancel_change();
        return Err("Nothing was copied".into());
    }
    cx.selection.items = all;
    cx.mark_dirty();
    let mut msg = format!(
        "Made {} cop{} of {} object{}",
        p.copies,
        if p.copies == 1 { "y" } else { "ies" },
        clip.len(),
        if clip.len() == 1 { "" } else { "s" }
    );
    if !clip.skipped.is_empty() {
        msg.push_str(&format!(" (not copied: {})", clip.skipped.join(", ")));
    }
    Ok(msg)
}

/// Rotates the selection by `angle` radians about `about` (or its center),
/// as one undo step named `label`.
pub fn rotate_selection(cx: &mut EditorContext, angle: f64, about: Option<Point>) -> Report {
    let items = cx.selection.items.clone();
    let Some(center) = about.or_else(|| selection_center(cx)) else {
        return Report::default();
    };
    if items.iter().any(|o| !cx.check_unlocked(*o)) {
        return Report::default();
    }
    cx.begin_change("Rotate");
    let rep = apply_xform(cx, &items, &Xform::rotate(center, angle));
    cx.mark_dirty();
    rep
}

/// Mirrors the selection about the line `a`..`b`: in place, or as copies
/// that leave the originals (S-48, S-105).
pub fn reflect_selection(cx: &mut EditorContext, a: Point, b: Point, copy: bool) -> Report {
    let items = cx.selection.items.clone();
    if a.dist(b) < 1e-6 || items.is_empty() {
        return Report::default();
    }
    let x = Xform::reflect(a, b);
    if copy {
        let mut clip = super::Clipboard::capture(cx);
        clip.drop_unturnable();
        if clip.is_empty() {
            return Report::default();
        }
        cx.begin_change("Reflect Copy");
        let fresh = clip.paste(cx, Point::ZERO, false);
        let rep = apply_xform(cx, &fresh, &x);
        cx.selection.items = fresh;
        cx.mark_dirty();
        return rep;
    }
    if items.iter().any(|o| !cx.check_unlocked(*o)) {
        return Report::default();
    }
    cx.begin_change("Reflect");
    let rep = apply_xform(cx, &items, &x);
    cx.mark_dirty();
    rep
}

// ----- Align and Distribute -----

/// The selection's objects with a place in the plan and their boxes.
fn boxed(cx: &EditorContext) -> Vec<(ObjectRef, (Point, Point))> {
    cx.selection
        .items
        .iter()
        .filter(|o| !matches!(o, ObjectRef::Opening(_)))
        .filter_map(|o| object_bounds(cx, *o).map(|b| (*o, b)))
        .collect()
}

/// Align Left/Right/Center/Top/Bottom/Middle (S-54): one undo step.
pub fn align_selection(cx: &mut EditorContext, mode: AlignMode) -> Result<usize, String> {
    let items = boxed(cx);
    if items.len() < 2 {
        return Err("Select two or more objects to align".into());
    }
    let offsets = align_offsets(&items.iter().map(|(_, b)| *b).collect::<Vec<_>>(), mode);
    shift_each(cx, &items, &offsets, mode.label())
}

/// Distribute Horizontally/Vertically with equal gaps (or a fixed `gap`).
pub fn distribute_selection(
    cx: &mut EditorContext,
    axis: Axis,
    gap: Option<f64>,
) -> Result<usize, String> {
    let items = boxed(cx);
    if items.len() < 3 && !(items.len() == 2 && gap.is_some()) {
        return Err("Select three or more objects to distribute".into());
    }
    let offsets = distribute_offsets(
        &items.iter().map(|(_, b)| *b).collect::<Vec<_>>(),
        axis,
        gap,
    );
    let label = match axis {
        Axis::Horizontal => "Distribute Horizontally",
        Axis::Vertical => "Distribute Vertically",
    };
    shift_each(cx, &items, &offsets, label)
}

fn shift_each(
    cx: &mut EditorContext,
    items: &[(ObjectRef, (Point, Point))],
    offsets: &[Point],
    label: &str,
) -> Result<usize, String> {
    if items.iter().any(|(o, _)| !cx.check_unlocked(*o)) {
        return Err(cx.status.clone());
    }
    let moving: Vec<usize> = (0..items.len())
        .filter(|&i| offsets[i].length() > 1e-9)
        .collect();
    if moving.is_empty() {
        return Err("The objects are already in place".into());
    }
    cx.begin_change(label);
    for &i in &moving {
        translate_objects(cx, &[items[i].0], offsets[i]);
    }
    cx.mark_dirty();
    Ok(moving.len())
}

// ----- Make Parallel / Perpendicular, Center, front and back -----

/// Make Parallel/Perpendicular to the direction `along` (S-41): selected
/// walls keep their start and length and swing their far end; CAD lines turn
/// about their start. One undo step.
pub fn make_parallel(cx: &mut EditorContext, along: Point, perpendicular: bool) -> usize {
    let items = cx.selection.items.clone();
    if items.iter().any(|o| !cx.check_unlocked(*o)) {
        return 0;
    }
    let fl = cx.floor;
    let label = if perpendicular {
        "Make Perpendicular"
    } else {
        "Make Parallel"
    };
    cx.begin_change(label);
    let mut n = 0;
    for o in &items {
        match *o {
            ObjectRef::Wall(id) => {
                let Some(w) = cx.floor().wall(id).cloned() else {
                    continue;
                };
                let to = parallel_end(w.start, w.end, along, perpendicular);
                if to.dist(w.end) > 1e-6
                    && ops::move_wall_end_joined(&mut cx.project, fl, id, WallEnd::End, to)
                {
                    n += 1;
                }
            }
            ObjectRef::Cad(id) | ObjectRef::Text(id) => {
                if let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) {
                    if let CadItem::Line { a, b } = &mut c.item {
                        let to = parallel_end(*a, *b, along, perpendicular);
                        if to.dist(*b) > 1e-6 {
                            *b = to;
                            n += 1;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if n == 0 {
        cx.cancel_change();
    } else {
        cx.mark_dirty();
    }
    n
}

/// Center Object for a lone door or window (S-53): centers it on its wall.
pub fn center_opening_in_wall(cx: &mut EditorContext) -> Result<(), String> {
    let Some(ObjectRef::Opening(id)) = cx.selection.single() else {
        return Err("Select one door or window".into());
    };
    if !cx.check_unlocked(ObjectRef::Opening(id)) {
        return Err(cx.status.clone());
    }
    let Some(op) = cx.floor().openings.iter().find(|o| o.id == id).cloned() else {
        return Err("Select one door or window".into());
    };
    let Some(len) = cx.floor().wall(op.wall_id).map(|w| w.length()) else {
        return Err("The opening has no wall".into());
    };
    cx.begin_change("Center Object");
    let fl = cx.floor;
    if ops::place_opening_at(&mut cx.project, fl, id, op.wall_id, len * 0.5) {
        cx.mark_dirty();
        Ok(())
    } else {
        cx.cancel_change();
        Err("There is no room to center it".into())
    }
}

/// Center the selection halfway between two walls (along the first wall's
/// normal), keeping its place along them. One undo step.
pub fn center_between_walls(cx: &mut EditorContext, a: Id, b: Id) -> Result<(), String> {
    let (Some(wa), Some(wb)) = (cx.floor().wall(a).cloned(), cx.floor().wall(b).cloned()) else {
        return Err("Click two walls".into());
    };
    let Some(center) = selection_center(cx) else {
        return Err("The selection has no place in the plan".into());
    };
    let n = wa.normal();
    let (oa, ob) = (wa.start.dot(n), wb.start.dot(n));
    let shift = n * ((oa + ob) * 0.5 - center.dot(n));
    move_selection(cx, shift, "Center Object")
}

/// Center the selection on the middle of the room polygon `room`.
pub fn center_in_room(cx: &mut EditorContext, room: usize) -> Result<(), String> {
    let Some(poly) = cx.rooms.get(room).map(|r| r.polygon.clone()) else {
        return Err("Click inside a room".into());
    };
    let (Some((lo, hi)), Some(center)) = (bounds_of(poly), selection_center(cx)) else {
        return Err("The selection has no place in the plan".into());
    };
    let mid = Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5);
    move_selection(cx, mid - center, "Center Object")
}

/// Moves the selection by `d` as one undo step; refuses on locked layers.
pub fn move_selection(cx: &mut EditorContext, d: Point, label: &str) -> Result<(), String> {
    let items = cx.selection.items.clone();
    if items.is_empty() {
        return Err("Select objects first".into());
    }
    if items.iter().any(|o| !cx.check_unlocked(*o)) {
        return Err(cx.status.clone());
    }
    if d.length() < 1e-6 {
        return Err("The objects are already there".into());
    }
    cx.begin_change(label);
    translate_objects(cx, &items, d);
    Ok(())
}

/// Move to Front / Move to Back: CAD objects are drawn in list order, so the
/// last one is on top. Returns how many were reordered.
pub fn reorder_cad(cx: &mut EditorContext, to_front: bool) -> usize {
    let ids: Vec<Id> = cx
        .selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Cad(i) | ObjectRef::Text(i) => Some(*i),
            _ => None,
        })
        .collect();
    if ids.is_empty() || ids.iter().any(|i| !cx.check_unlocked(ObjectRef::Cad(*i))) {
        return 0;
    }
    cx.begin_change(if to_front {
        "Move to Front"
    } else {
        "Move to Back"
    });
    let fl = cx.floor;
    let list = &mut cx.project.floors[fl].cad;
    let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(list)
        .into_iter()
        .partition(|c| ids.contains(&c.id));
    let n = mine.len();
    *list = if to_front {
        rest.into_iter().chain(mine).collect()
    } else {
        mine.into_iter().chain(rest).collect()
    };
    cx.mark_dirty();
    n
}

// ----- the multi-object Rotate handle -----

/// Pixels above the selection box the group Rotate handle sits.
const ROTATE_HANDLE_PX: f64 = 26.0;

/// The pivot and the handle of the group Rotate (S-101, S-102): shown when
/// two or more turnable objects are selected.
pub fn group_rotate_handle(cx: &EditorContext) -> Option<(Point, Point)> {
    if cx.selection.len() < 2 {
        return None;
    }
    // Only objects that turn count (this also keeps the per-frame cost low:
    // the other kinds need their records parsed to find their box).
    let turnable: Vec<ObjectRef> = cx
        .selection
        .items
        .iter()
        .copied()
        .filter(|o| {
            matches!(
                o,
                ObjectRef::Wall(_)
                    | ObjectRef::Dimension(_)
                    | ObjectRef::Cad(_)
                    | ObjectRef::Text(_)
                    | ObjectRef::Symbol(_)
                    | ObjectRef::Camera(_)
                    | ObjectRef::Cabinet(_)
                    | ObjectRef::Stair(_)
                    | ObjectRef::Device(_)
            )
        })
        .collect();
    if turnable.len() < 2 {
        return None;
    }
    let (lo, hi) = items_bounds(cx, &turnable)?;
    let center = Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5);
    let handle = Point::new(center.x, hi.y + ROTATE_HANDLE_PX / cx.px_per_in.max(1e-6));
    Some((center, handle))
}

// ----- click-driven modes -----

/// A mode in which the next clicks mean something other than selecting.
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    /// Paste: the clipboard hangs on the pointer until a click drops it.
    Paste { as_group: bool, at: Option<Point> },
    /// Point to Point Move: click the point to move from, then the point to
    /// move to.
    PointToPoint {
        from: Option<Point>,
        to: Option<Point>,
    },
    /// Reflect About Object: click a wall or CAD line to mirror about.
    Reflect { copy: bool },
    /// Center Object: click a room, or two walls.
    Center { first_wall: Option<Id> },
    /// Make Parallel/Perpendicular: click the wall or CAD line to match.
    Parallel { perpendicular: bool },
}

thread_local! {
    static MODE: RefCell<Option<Mode>> = const { RefCell::new(None) };
}

/// The mode in force, if any.
pub fn mode() -> Option<Mode> {
    MODE.with(|m| m.borrow().clone())
}

pub fn mode_active() -> bool {
    MODE.with(|m| m.borrow().is_some()) || crate::tools::cad_ops::active()
}

/// Starts `mode`, replacing any other, with its hint in the status bar.
pub fn begin_mode(cx: &mut EditorContext, mode: Mode) {
    cx.status = match &mode {
        Mode::Paste { .. } => "Paste: click to place the objects, Esc cancels".into(),
        Mode::PointToPoint { .. } => "Point to Point Move: click the point to move from".into(),
        Mode::Reflect { .. } => "Reflect: click a wall or line to mirror about".into(),
        Mode::Center { .. } => "Center Object: click a room, or click two walls".into(),
        Mode::Parallel { perpendicular } => format!(
            "Make {}: click the wall or line to match",
            if *perpendicular {
                "Perpendicular"
            } else {
                "Parallel"
            }
        ),
    };
    MODE.with(|m| *m.borrow_mut() = Some(mode));
}

/// Ends the mode without doing anything. Returns whether there was one.
pub fn cancel_mode(cx: &mut EditorContext) -> bool {
    let had = MODE.with(|m| m.borrow_mut().take()).is_some() | crate::tools::cad_ops::cancel(cx);
    if had {
        cx.status.clear();
    }
    had
}

/// The pointer moved: a hanging paste follows it.
pub fn mode_pointer_move(p: &PointerEvent) {
    crate::tools::cad_ops::pointer_move(p);
    MODE.with(|m| {
        if let Some(Mode::Paste { at, .. }) = m.borrow_mut().as_mut() {
            *at = Some(p.snapped);
        }
    });
}

/// The line a click at `at` names as a mirror or direction reference: a
/// wall's centerline, a CAD line, or the nearest side of a polyline.
fn reference_line(cx: &EditorContext, at: Point) -> Option<(Point, Point)> {
    let tol = cx.pick_tol();
    for hit in hit_test_cx(cx, at, tol) {
        match hit {
            ObjectRef::Wall(id) => {
                if let Some(w) = cx.floor().wall(id) {
                    return Some((w.start, w.end));
                }
            }
            ObjectRef::Cad(id) | ObjectRef::Text(id) => {
                if let Some(c) = cx.floor().cad.iter().find(|c| c.id == id) {
                    match &c.item {
                        CadItem::Line { a, b } => return Some((*a, *b)),
                        CadItem::Polyline { points, closed } => {
                            let n = points.len();
                            let sides = if *closed { n } else { n.saturating_sub(1) };
                            return (0..sides).map(|i| (points[i], points[(i + 1) % n])).min_by(
                                |s, t| {
                                    plan_core::geometry::dist_to_segment(at, s.0, s.1).total_cmp(
                                        &plan_core::geometry::dist_to_segment(at, t.0, t.1),
                                    )
                                },
                            );
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// A click while a mode is active. `None`: no mode, the tool handles it.
pub fn mode_pointer_down(cx: &mut EditorContext, p: &PointerEvent) -> Option<ToolResult> {
    if let Some(res) = crate::tools::cad_ops::pointer_down(cx, p) {
        return Some(res);
    }
    let mode = mode()?;
    let end = |cx: &mut EditorContext| {
        MODE.with(|m| *m.borrow_mut() = None);
        let _ = cx;
    };
    match mode {
        Mode::Paste { as_group, .. } => {
            let Some(clip) = cx.clipboard.clone().filter(|c| !c.is_empty()) else {
                end(cx);
                return Some(ToolResult::consumed());
            };
            let center = clip.center().unwrap_or(p.snapped);
            end(cx);
            let n = cx.paste_attached(p.snapped - center, as_group);
            Some(if n > 0 {
                ToolResult::committed("Paste")
            } else {
                ToolResult::consumed()
            })
        }
        Mode::PointToPoint { from: None, .. } => {
            MODE.with(|m| {
                *m.borrow_mut() = Some(Mode::PointToPoint {
                    from: Some(p.snapped),
                    to: None,
                })
            });
            cx.status = "Point to Point Move: click the point to move to".into();
            Some(ToolResult::consumed())
        }
        Mode::PointToPoint {
            from: Some(from), ..
        } => {
            end(cx);
            match move_selection(cx, p.snapped - from, "Point to Point Move") {
                Ok(()) => {
                    cx.status = "Moved the objects".into();
                    Some(ToolResult::committed("Point to Point Move"))
                }
                Err(e) => {
                    cx.status = e;
                    Some(ToolResult::consumed())
                }
            }
        }
        Mode::Reflect { copy } => {
            let Some((a, b)) = reference_line(cx, p.world) else {
                cx.status = "Click a wall or a line".into();
                return Some(ToolResult::consumed());
            };
            end(cx);
            let rep = reflect_selection(cx, a, b, copy);
            cx.status = rep.status(if copy {
                "Mirrored copies of"
            } else {
                "Mirrored"
            });
            Some(if rep.changed > 0 {
                ToolResult::committed("Reflect")
            } else {
                ToolResult::consumed()
            })
        }
        Mode::Parallel { perpendicular } => {
            let Some((a, b)) = reference_line(cx, p.world) else {
                cx.status = "Click a wall or a line".into();
                return Some(ToolResult::consumed());
            };
            end(cx);
            let n = make_parallel(cx, b - a, perpendicular);
            cx.status = format!(
                "Made {n} object{} {}",
                if n == 1 { "" } else { "s" },
                if perpendicular {
                    "perpendicular"
                } else {
                    "parallel"
                }
            );
            Some(if n > 0 {
                ToolResult::committed("Make Parallel")
            } else {
                ToolResult::consumed()
            })
        }
        Mode::Center { first_wall } => {
            let tol = cx.pick_tol();
            let hits = hit_test_cx(cx, p.world, tol);
            let wall = hits.iter().find_map(|h| match h {
                ObjectRef::Wall(id) => Some(*id),
                _ => None,
            });
            let result = match (wall, first_wall) {
                (Some(w), None) => {
                    MODE.with(|m| {
                        *m.borrow_mut() = Some(Mode::Center {
                            first_wall: Some(w),
                        })
                    });
                    cx.status = "Center Object: click the second wall".into();
                    return Some(ToolResult::consumed());
                }
                (Some(w), Some(first)) if w != first => center_between_walls(cx, first, w),
                (Some(_), Some(_)) => {
                    cx.status = "Click a different wall".into();
                    return Some(ToolResult::consumed());
                }
                (None, None) => match super::rooms_edit::room_index_at(cx, p.world) {
                    Some(room) => center_in_room(cx, room),
                    None => {
                        cx.status = "Click a room, or click two walls".into();
                        return Some(ToolResult::consumed());
                    }
                },
                (None, Some(_)) => {
                    cx.status = "Click the second wall".into();
                    return Some(ToolResult::consumed());
                }
            };
            end(cx);
            Some(match result {
                Ok(()) => {
                    cx.status = "Centered the objects".into();
                    ToolResult::committed("Center Object")
                }
                Err(e) => {
                    cx.status = e;
                    ToolResult::consumed()
                }
            })
        }
    }
}

/// Esc (or a right click) ends the mode. Returns whether one was active.
pub fn mode_escape(cx: &mut EditorContext) -> bool {
    cancel_mode(cx)
}

/// Draws what the mode shows: the pasted objects' outline at the pointer, or
/// the rubber band of a Point to Point Move.
pub fn draw_mode_overlay(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    crate::tools::cad_ops::draw_overlay(cx, painter, cam);
    let Some(mode) = mode() else {
        return;
    };
    let pal = &cx.palette;
    let stroke = Stroke::new(1.5_f32, pal.ghost_stroke);
    match mode {
        Mode::Paste { at: Some(at), .. } => {
            let Some(clip) = cx.clipboard.as_ref() else {
                return;
            };
            let Some(center) = clip.center() else {
                return;
            };
            let d = at - center;
            for (a, b) in clip.ghost_segments() {
                painter.line_segment(
                    [cam.world_to_screen(a + d), cam.world_to_screen(b + d)],
                    stroke,
                );
            }
        }
        Mode::PointToPoint {
            from: Some(from), ..
        } => {
            if let Some(to) = cx.cursor_world {
                painter.line_segment([cam.world_to_screen(from), cam.world_to_screen(to)], stroke);
            }
        }
        _ => {}
    }
}

/// Draws the group Rotate handle for the selection.
pub fn draw_group_rotate_handle(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let Some((center, handle)) = group_rotate_handle(cx) else {
        return;
    };
    let pal = &cx.palette;
    let (c, h) = (cam.world_to_screen(center), cam.world_to_screen(handle));
    painter.line_segment([c, h], Stroke::new(1.0_f32, pal.selection));
    painter.circle_filled(h, 5.0, pal.selection);
    painter.circle_stroke(h, 5.0, Stroke::new(1.0_f32, pal.background));
}
