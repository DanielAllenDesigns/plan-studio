//! The Exterior Room (R-106, manual pp. 449 to 451): selecting it, its
//! highlight band and Living Area labels in the plan, its two edge grips that
//! set the level's default ceiling and floor heights, and applying its
//! specification (exterior wall coverings and materials).
//!
//! Each structure on each floor has an Exterior Room. Click just outside an
//! exterior wall (or on its Living Area label) with the Select tool, or press
//! Tab (Select Next Object) until the status bar says Exterior Room.

use super::*;
use crate::editor::selection::ObjectRef;
use plan_core::living::{ExteriorEdge, ExteriorRoom, Structure};

/// A selected Exterior Room: a point inside its structure, on a floor.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ExtSel {
    pub floor: usize,
    pub point: Point,
}

/// A drag of one edge grip: the value it started from and where it is now.
#[derive(Clone, Copy, Debug)]
pub struct ExtDrag {
    pub edge: ExteriorEdge,
    pub grab: Point,
    pub start: f64,
    pub now: f64,
}

#[derive(Default)]
pub struct ExtState {
    pub selected: Option<ExtSel>,
    pub dialog_request: Option<ExtSel>,
    pub drag: Option<ExtDrag>,
}

/// How far outside the wall centerlines a click still means the Exterior
/// Room: the thickest exterior wall's half plus a generous pick distance.
fn reach(cx: &EditorContext) -> f64 {
    let half = cx
        .floor()
        .walls
        .iter()
        .filter(|w| w.kind == plan_core::WallKind::Exterior)
        .map(|w| w.thickness * 0.5)
        .fold(3.0, f64::max);
    half + (cx.pick_tol() * 2.0).max(8.0)
}

/// The structures of the active floor.
pub fn structures(cx: &EditorContext) -> Vec<Structure> {
    living::structures(&cx.rooms)
}

/// The index of the structure whose Exterior Room a click at `p` means: just
/// outside its exterior walls (not in a room), or on its Living Area label.
pub fn exterior_at(cx: &EditorContext, p: Point) -> Option<usize> {
    if cx.rooms.iter().any(|r| r.contains(p)) {
        return None;
    }
    let reach = reach(cx);
    structures(cx)
        .iter()
        .position(|s| living::near_outline(&s.outline, p, reach))
        .or_else(|| exterior_label_at(cx, p))
}

/// The structure whose Living Area label is under `p`.
pub fn exterior_label_at(cx: &EditorContext, p: Point) -> Option<usize> {
    if !cx.layers().is_visible("Room Labels") || !show_living_label(&cx.defaults) {
        return None;
    }
    living_labels(cx).into_iter().find_map(|a| {
        let text = a.text();
        let (hw, hh) = label_half_extent(cx, &text);
        ((p.x - a.label_at.x).abs() <= hw && (p.y - a.label_at.y).abs() <= hh)
            .then_some(a.structure)
    })
}

/// Selects the Exterior Room of structure `idx` of the active floor.
pub fn select_exterior(cx: &mut EditorContext, idx: usize) {
    let Some(s) = structures(cx).into_iter().nth(idx) else {
        return;
    };
    cx.selection.clear();
    let sel = ExtSel {
        floor: cx.floor,
        point: s.anchor,
    };
    with(|st| {
        st.selected = None;
        st.ext.selected = Some(sel);
    });
    cx.status = "Exterior Room".into();
}

/// The structure (index into the active floor's structures) whose Exterior
/// Room is selected.
pub fn selected_exterior(cx: &EditorContext) -> Option<usize> {
    let sel = with(|s| s.ext.selected)?;
    if sel.floor != cx.floor {
        return None;
    }
    structures(cx)
        .iter()
        .position(|s| s.rooms.iter().any(|&i| cx.rooms[i].contains(sel.point)))
}

/// What Select Next Object (Tab) did.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Next {
    /// Select this object.
    Hit(ObjectRef),
    /// The Exterior Room is now selected.
    Exterior,
    Nothing,
}

/// Select Next Object at `at`: through the objects under it (`hits`, topmost
/// first) and then the Exterior Room, when `at` is just outside an exterior
/// wall (p. 449). Shift goes the other way round.
pub fn exterior_cycle(
    cx: &mut EditorContext,
    at: Point,
    hits: &[ObjectRef],
    backwards: bool,
) -> Next {
    let ext = exterior_at(cx, at);
    let n = hits.len() + usize::from(ext.is_some());
    if n == 0 {
        return Next::Nothing;
    }
    let now = if selected_exterior(cx).is_some() && ext.is_some() {
        Some(hits.len())
    } else {
        cx.selection
            .single()
            .and_then(|s| hits.iter().position(|h| *h == s))
    };
    let next = match now {
        Some(i) if backwards => (i + n - 1) % n,
        Some(i) => (i + 1) % n,
        None if backwards => n - 1,
        None => 0,
    };
    match (hits.get(next), ext) {
        (Some(h), _) => Next::Hit(*h),
        (None, Some(e)) => {
            select_exterior(cx, e);
            Next::Exterior
        }
        _ => Next::Nothing,
    }
}

// ----- the edge grips -----

/// Half the size of a grip, screen points.
const GRIP: f32 = 5.0;

/// The plan positions of the two grips of the selected Exterior Room: the top
/// edge (default ceiling height) and the bottom edge (default floor height),
/// outside the right end of the structure.
pub fn grips(cx: &EditorContext, s: &Structure) -> [(ExteriorEdge, Point); 2] {
    let first = s.outline.first().copied().unwrap_or(s.anchor);
    let (mut lo, mut hi) = (first, first);
    for p in &s.outline {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let off = 24.0 + cx.pick_tol();
    [
        (ExteriorEdge::Top, Point::new(hi.x + off, hi.y)),
        (ExteriorEdge::Bottom, Point::new(hi.x + off, lo.y)),
    ]
}

/// Can the bottom edge move? Floor 1 stays at zero (manual p. 444); the
/// layered floor platforms set their own thickness.
fn bottom_editable(cx: &EditorContext) -> bool {
    let first = cx
        .project
        .floors
        .iter()
        .position(|f| f.kind != FloorKind::Foundation);
    Some(cx.floor) != first
        && cx.floor().kind == FloorKind::Normal
        && cx
            .floor()
            .settings
            .platform
            .slot(plan_core::assemblies::AssemblyKind::FloorStructure)
            .is_legacy()
}

/// The value a grip sets: the default ceiling height, or the thickness of
/// the floor platform that fixes the default floor height.
fn edge_value(cx: &EditorContext, edge: ExteriorEdge) -> f64 {
    match edge {
        ExteriorEdge::Top => cx.floor().ceiling_height,
        ExteriorEdge::Bottom => cx.floor().settings.floor_structure_thickness,
    }
}

/// Is an edge grip under `world`? Starts a drag.
pub fn exterior_drag_down(cx: &mut EditorContext, world: Point) -> bool {
    let Some(i) = selected_exterior(cx) else {
        return false;
    };
    let s = structures(cx).swap_remove(i);
    let tol = cx.pick_tol() * 1.5;
    for (edge, at) in grips(cx, &s) {
        if edge == ExteriorEdge::Bottom && !bottom_editable(cx) {
            continue;
        }
        if at.dist(world) <= tol {
            let v = edge_value(cx, edge);
            with(|st| {
                st.ext.drag = Some(ExtDrag {
                    edge,
                    grab: world,
                    start: v,
                    now: v,
                });
            });
            return true;
        }
    }
    false
}

pub fn exterior_dragging() -> bool {
    with(|s| s.ext.drag.is_some())
}

/// Drags the grip: one inch of plan is one inch of height; the readout is
/// the temporary dimension from the default rough floor to the rough ceiling.
pub fn exterior_drag_move(cx: &mut EditorContext, world: Point) -> bool {
    let moved = with(|st| {
        let d = st.ext.drag.as_mut()?;
        let raw = d.start + (world.y - d.grab.y);
        d.now = (raw / cx.snap_unit().max(0.125)).round() * cx.snap_unit().max(0.125);
        d.now = d.now.max(match d.edge {
            ExteriorEdge::Top => 12.0,
            ExteriorEdge::Bottom => 0.0,
        });
        Some((d.edge, d.now))
    });
    match moved {
        Some((ExteriorEdge::Top, v)) => {
            cx.status = format!("Exterior Room: default ceiling height {}", cx.fmt_dim(v));
            true
        }
        Some((ExteriorEdge::Bottom, v)) => {
            cx.status = format!("Exterior Room: floor platform {}", cx.fmt_dim(v));
            true
        }
        None => false,
    }
}

/// Lets go of the grip: the new default is one undo step. `None` when no grip
/// was held, `Some(false)` when it came back to where it started.
pub fn exterior_drag_up(cx: &mut EditorContext) -> Option<bool> {
    let d = with(|st| st.ext.drag.take())?;
    if (d.now - d.start).abs() < 1e-9 {
        return Some(false);
    }
    Some(set_default_heights(cx, d.edge, d.now))
}

/// Sets the default ceiling height of the active level (top edge) or the
/// thickness of its floor platform (bottom edge, which moves the level's
/// default floor height), as one undo step. Floors above follow.
pub fn set_default_heights(cx: &mut EditorContext, edge: ExteriorEdge, value: f64) -> bool {
    if (edge_value(cx, edge) - value).abs() < 1e-9 {
        return false;
    }
    let mut settings = cx.floor().settings.clone();
    let mut ceiling = cx.floor().ceiling_height;
    match edge {
        ExteriorEdge::Top => ceiling = value.max(12.0),
        ExteriorEdge::Bottom => {
            if !bottom_editable(cx) {
                cx.status = "The default floor height of this level cannot be changed".into();
                return false;
            }
            settings.floor_structure_thickness = value.max(0.0);
        }
    }
    if !apply_floor_defaults(cx, ceiling, settings, false) {
        return false;
    }
    cx.status = format!(
        "Exterior Room: ceiling {}, floor at {}",
        cx.fmt_dim(cx.floor().ceiling_height),
        cx.fmt_dim(cx.floor().elevation)
    );
    true
}

// ----- drawing -----

/// Draws the Living Area labels and the selected Exterior Room's band and
/// grips (called after the room fills).
pub(super) fn draw(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let pal = &cx.palette;
    if cx.layers().is_visible("Room Labels") && show_living_label(&cx.defaults) {
        for a in living_labels(cx) {
            painter.text(
                cam.world_to_screen(a.label_at),
                Align2::CENTER_CENTER,
                a.text(),
                FontId::proportional(LABEL_FONT_PX as f32),
                pal.room_label,
            );
        }
    }
    let Some(i) = selected_exterior(cx) else {
        return;
    };
    let s = structures(cx).swap_remove(i);
    let band = screen_poly(cam, &s.outline);
    let col = pal.selection;
    painter.add(Shape::closed_line(
        band.clone(),
        Stroke::new(10.0_f32, col.gamma_multiply(0.35)),
    ));
    painter.add(Shape::closed_line(band, Stroke::new(1.5_f32, col)));
    let drag = with(|st| st.ext.drag);
    for (edge, at) in grips(cx, &s) {
        if edge == ExteriorEdge::Bottom && !bottom_editable(cx) {
            continue;
        }
        let c = cam.world_to_screen(at);
        painter.rect_filled(
            egui::Rect::from_center_size(c, egui::vec2(GRIP * 2.0, GRIP * 2.0)),
            0.0,
            col,
        );
        let v = drag
            .filter(|d| d.edge == edge)
            .map_or_else(|| edge_value(cx, edge), |d| d.now);
        let text = match edge {
            ExteriorEdge::Top => format!("Ceiling {}", cx.fmt_dim(v)),
            ExteriorEdge::Bottom => format!("Floor platform {}", cx.fmt_dim(v)),
        };
        painter.text(
            c + egui::vec2(10.0, 0.0),
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(11.0),
            col,
        );
    }
}

// ----- the Exterior Room Specification -----

/// Asks the shell to open the Exterior Room Specification of structure `idx`.
pub fn request_exterior_dialog(cx: &EditorContext, idx: usize) {
    if let Some(s) = structures(cx).into_iter().nth(idx) {
        let sel = ExtSel {
            floor: cx.floor,
            point: s.anchor,
        };
        with(|st| st.ext.dialog_request = Some(sel));
    }
}

/// The pending request as a structure index of the active floor.
pub fn take_exterior_dialog_request(cx: &EditorContext) -> Option<usize> {
    let sel = with(|s| s.ext.dialog_request.take())?;
    if sel.floor != cx.floor {
        return None;
    }
    structures(cx)
        .iter()
        .position(|s| s.rooms.iter().any(|&i| cx.rooms[i].contains(sel.point)))
}

/// What the Exterior Room Specification starts from.
#[derive(Clone, Debug)]
pub struct ExteriorInit {
    pub floor: usize,
    pub floor_name: String,
    /// The structure's anchor point.
    pub anchor: Point,
    pub spec: ExteriorRoom,
    pub rooms: usize,
    pub exterior_walls: usize,
    pub living_sq_ft: f64,
    pub footprint_sq_ft: f64,
    pub ceiling_height: f64,
    pub floor_elevation: f64,
    pub floor_thickness: f64,
    pub bottom_editable: bool,
    /// Materials for the covering and the surface lists.
    pub wall_materials: Vec<&'static str>,
}

/// Everything the dialog starts from for structure `idx`.
pub fn exterior_dialog_init(cx: &EditorContext, idx: usize) -> Option<ExteriorInit> {
    let s = structures(cx).into_iter().nth(idx)?;
    let f = cx.floor();
    let mut spec = living::exterior_room_of(f, &cx.rooms, &s)
        .cloned()
        .unwrap_or_default();
    spec.anchor = s.anchor;
    let living_sq_ft = living_labels(cx)
        .into_iter()
        .find(|a| a.structure == idx)
        .map_or(0.0, |a| a.sq_ft());
    Some(ExteriorInit {
        floor: cx.floor,
        floor_name: f.name.clone(),
        anchor: s.anchor,
        rooms: s.rooms.len(),
        exterior_walls: living::structure_exterior_walls(f, &s).len(),
        living_sq_ft,
        footprint_sq_ft: polygon_area(&s.outline) / 144.0,
        ceiling_height: f.ceiling_height,
        floor_elevation: f.elevation,
        floor_thickness: f.settings.floor_structure_thickness,
        bottom_editable: bottom_editable(cx),
        wall_materials: plan_core::rooms::WALL_SURFACES.to_vec(),
        spec,
    })
}

/// Writes an accepted Exterior Room Specification as one undo step: the
/// covering and surface material go onto every exterior wall of the
/// structure, the specification is kept with the floor, and the level's
/// default ceiling height and floor platform change when edited.
pub fn apply_exterior_spec(
    cx: &mut EditorContext,
    anchor: Point,
    spec: &ExteriorRoom,
    ceiling_height: f64,
    floor_thickness: f64,
) -> bool {
    let Some(i) = structures(cx)
        .iter()
        .position(|s| s.rooms.iter().any(|&r| cx.rooms[r].contains(anchor)))
    else {
        return false;
    };
    let s = structures(cx).swap_remove(i);
    let ids = living::structure_exterior_walls(cx.floor(), &s);
    let fl = cx.floor;
    let mut stored = spec.clone();
    stored.anchor = s.anchor;
    // OK on a specification that changes nothing is not an undo step.
    let same_spec = match living::exterior_room_of(cx.floor(), &cx.rooms, &s) {
        Some(e) => {
            e.covering == stored.covering
                && e.surface_material == stored.surface_material
                && e.surface_rgb == stored.surface_rgb
        }
        None => stored.is_blank(),
    };
    let same_heights = (ceiling_height.max(12.0) - cx.floor().ceiling_height).abs() < 1e-9
        && (!bottom_editable(cx)
            || (floor_thickness.max(0.0) - cx.floor().settings.floor_structure_thickness).abs()
                < 1e-9);
    if same_spec && same_heights {
        cx.status = "Exterior Room: nothing changed".into();
        return true;
    }
    cx.begin_change("Exterior Room Specification");
    {
        let rooms = cx.rooms.clone();
        let floor = &mut cx.project.floors[fl];
        floor
            .exterior_rooms
            .retain(|e| !s.rooms.iter().any(|&r| rooms[r].contains(e.anchor)));
        if !stored.is_blank() {
            floor.exterior_rooms.push(stored.clone());
        }
        living::apply_exterior_room(floor, &ids, &stored);
    }
    let mut settings = cx.floor().settings.clone();
    if bottom_editable(cx) {
        settings.floor_structure_thickness = floor_thickness.max(0.0);
    }
    let ceiling = ceiling_height.max(12.0);
    if (ceiling - cx.floor().ceiling_height).abs() > 1e-9
        || (settings.floor_structure_thickness - cx.floor().settings.floor_structure_thickness)
            .abs()
            > 1e-9
    {
        let old_settings = cx.floor().settings.clone();
        cx.project.apply_floor_settings(fl, ceiling, settings);
        cx.project.rooms_follow_floor_finish(fl, &old_settings);
    }
    cx.project.sync_platform_mirrors();
    cx.mark_dirty();
    cx.refresh();
    cx.status = format!("Exterior Room: {} exterior walls updated", ids.len());
    true
}
