//! Edit handles of the selected object (S-11..S-28): a small model plus
//! hit-testing and drawing. The select tool interprets the drags.

use super::ops::cad_center;
use super::selection::{cad_by_id, ObjectRef};
use super::{
    foundation_view, framing_view, placed, roof_view, site_view, stairs_view, Camera, EditorContext,
};
use crate::theme::Palette;
use crate::tools::camera::{self as camera_tool, CamHandle};
use eframe::egui::{self, CursorIcon, Rect, Shape, Stroke, Vec2};
use plan_core::cad::CadItem;
use plan_core::details::{DetailRef, DetailsLayer};
use plan_core::foundation::FoundationLayer;
use plan_core::geometry::Point;
use plan_core::{OpeningKind, OpeningStyle};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HandleKind {
    /// Free move of the whole object.
    Move,
    ResizeStart,
    ResizeEnd,
    /// Rotate about the object's center.
    Rotate,
    /// Move vertex `n` (polylines).
    Reshape(usize),
    /// Door swing (click flips it).
    Swing,
    /// Chief's signature wall move: perpendicular to the wall only. Also the
    /// line-offset handle of a dimension and the slide handle of an opening.
    PerpendicularMove,
    /// The apex of a curved wall: dragging it sets the bulge (W-67).
    Bulge,
    /// The label of a door or window: dragging it moves the label (DW-63).
    Label,
    /// The pitch arrow of a roof plane: dragging it up the slope steepens it
    /// (RF-38).
    Pitch,
    /// The middle of edge `n` of a roof plane: moves the edge square to
    /// itself (RF-38).
    EdgeMove(usize),
    /// An edit handle of a callout, marker or note (`plan_core::callout::handle`
    /// ids): Concentric Resize, Rotate, Extend, Add Text Line with Arrow,
    /// Add Callout Arrow and an arrow's Rotate handle.
    Annot(u8),
}

/// The roof plane handle a [`HandleKind`] of a `RoofPlane` target stands for;
/// `Move` has none (the plane moves as a whole). The Select tool turns these
/// into `roof_view::apply_handle_drag` calls.
pub fn roof_plane_handle(kind: HandleKind) -> Option<roof_view::PlaneHandle> {
    use roof_view::PlaneHandle;
    match kind {
        HandleKind::Reshape(i) => Some(PlaneHandle::Vertex(i)),
        HandleKind::EdgeMove(i) => Some(PlaneHandle::Edge(i)),
        HandleKind::Pitch => Some(PlaneHandle::Pitch),
        HandleKind::Rotate => Some(PlaneHandle::Rotate),
        _ => None,
    }
}

/// The camera wedge handle a [`HandleKind`] of a `Camera` target stands for:
/// `Reshape(2)` and `Reshape(3)` are the far corners of the view cone (angle
/// of view), `Reshape(4)` the tilt diamond (C-25). The Select tool turns these
/// into `camera::apply_wedge` calls.
pub fn camera_wedge_handle(kind: HandleKind) -> Option<camera_tool::WedgeHandle> {
    use camera_tool::WedgeHandle;
    match kind {
        HandleKind::Reshape(2) => Some(WedgeHandle::FovLeft),
        HandleKind::Reshape(3) => Some(WedgeHandle::FovRight),
        HandleKind::Reshape(4) => Some(WedgeHandle::Tilt),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Handle {
    pub kind: HandleKind,
    pub pos: Point,
    pub cursor: CursorIcon,
    pub target: ObjectRef,
}

/// Handles for a single selected object; none for an empty or multiple
/// selection. `scale` is pixels per inch (rotate handles sit a fixed number
/// of pixels off the object). Stairs, cabinets, symbols, devices, roof planes
/// and cameras delegate to the module that owns them.
pub fn handles_for(cx: &EditorContext, scale: f64) -> Vec<Handle> {
    // A callout, marker or note is a group of CAD objects: the whole group
    // selected has the annotation's handles.
    let annot = crate::tools::text::annot_handles(cx, scale);
    if !annot.is_empty() {
        return annot;
    }
    let floor = cx.floor();
    let Some(target) = cx.selection.single() else {
        return Vec::new();
    };
    let h = |kind, pos, cursor| Handle {
        kind,
        pos,
        cursor,
        target,
    };
    match target {
        ObjectRef::Wall(id) => {
            let Some(w) = floor.wall(id) else {
                return Vec::new();
            };
            let resize = resize_cursor(w.end.sub(w.start));
            let mut out = vec![
                h(HandleKind::ResizeStart, w.start, resize),
                h(HandleKind::ResizeEnd, w.end, resize),
                h(
                    HandleKind::PerpendicularMove,
                    Point::lerp(w.start, w.end, 0.5),
                    CursorIcon::Move,
                ),
            ];
            // A curved wall has a bulge handle at the apex of its arc.
            if let Some(c) = w.curve.filter(|c| !c.is_straight()) {
                out.push(h(
                    HandleKind::Bulge,
                    Point::lerp(w.start, w.end, 0.5) + w.normal() * c.bulge,
                    CursorIcon::Grab,
                ));
            }
            out
        }
        ObjectRef::Opening(id) => {
            let Some(o) = floor.openings.iter().find(|o| o.id == id) else {
                return Vec::new();
            };
            let Some(w) = floor.wall(o.wall_id) else {
                return Vec::new();
            };
            let mut out = vec![h(
                HandleKind::PerpendicularMove,
                w.point_along(o.center_offset),
                resize_cursor(w.end.sub(w.start)),
            )];
            // Resize handles at the jambs (DW-26): the opposite jamb stays.
            // A mulled window only has them at the ends of its unit.
            let (unit_lo, unit_hi) = cx
                .project
                .unit_span(cx.floor, id)
                .unwrap_or((o.start_offset(), o.end_offset()));
            let resize = resize_cursor(w.end.sub(w.start));
            if o.start_offset() <= unit_lo + 1e-9 {
                out.push(h(
                    HandleKind::ResizeStart,
                    w.point_along(o.start_offset()),
                    resize,
                ));
            }
            if o.end_offset() >= unit_hi - 1e-9 {
                out.push(h(
                    HandleKind::ResizeEnd,
                    w.point_along(o.end_offset()),
                    resize,
                ));
            }
            // The swing handle sits at the free end of the leaf: click flips
            // the side, Shift-click moves the hinge (DW-33). Flavors without a
            // leaf end keep it beside the wall on their swing side.
            let leaf_style = matches!(
                o.style,
                OpeningStyle::Hinged
                    | OpeningStyle::Shower
                    | OpeningStyle::DoubleDoor
                    | OpeningStyle::Casement
            );
            if o.kind == OpeningKind::Door || o.style == OpeningStyle::Casement {
                let n = w.normal_along(o.center_offset);
                let side = if o.swing_flipped { n * -1.0 } else { n };
                let pos = if leaf_style {
                    let hinge = w.point_along(if o.hinge_at_end {
                        o.end_offset()
                    } else {
                        o.start_offset()
                    });
                    let reach = if o.style == OpeningStyle::DoubleDoor {
                        o.width * 0.5
                    } else {
                        o.width
                    };
                    hinge + side * reach
                } else {
                    w.point_along(o.center_offset)
                        + side * (w.thickness * 0.5 + 6.0 / scale.max(1e-6))
                };
                out.push(h(HandleKind::Swing, pos, CursorIcon::PointingHand));
            }
            // The label handle sits on the label while it is shown (DW-63).
            if let Some(l) = super::opening_view::opening_labels(cx)
                .into_iter()
                .find(|l| l.opening == id)
            {
                out.push(h(HandleKind::Label, l.at, CursorIcon::Grab));
            }
            out
        }
        ObjectRef::Dimension(id) => {
            let Some(d) = floor.dimensions.iter().find(|d| d.id == id) else {
                return Vec::new();
            };
            let (a, b) = d.line_points();
            let at = d
                .curve_geom(0.0, 0.0)
                .map_or_else(|| Point::lerp(a, b, 0.5), |g| g.label_at);
            vec![h(HandleKind::PerpendicularMove, at, CursorIcon::Move)]
        }
        ObjectRef::Cad(id) | ObjectRef::Text(id) => {
            let Some(c) = cad_by_id(floor, id) else {
                return Vec::new();
            };
            // A text has its box handles (S-25, TXT-3).
            if matches!(c.item, CadItem::Text { .. }) {
                return text_handles(cx, c, scale, &h);
            }
            let (_, hi) = c.bounds();
            let center = cad_center(&c.item);
            let mut out = vec![
                h(HandleKind::Move, center, CursorIcon::Move),
                h(
                    HandleKind::Rotate,
                    Point::new(center.x, hi.y + 24.0 / scale.max(1e-6)),
                    CursorIcon::Grab,
                ),
            ];
            match &c.item {
                CadItem::Line { a, b } => {
                    out.push(h(HandleKind::ResizeStart, *a, CursorIcon::Crosshair));
                    out.push(h(HandleKind::ResizeEnd, *b, CursorIcon::Crosshair));
                }
                CadItem::Circle { center, radius } => out.push(h(
                    HandleKind::ResizeEnd,
                    Point::new(center.x + radius, center.y),
                    CursorIcon::ResizeHorizontal,
                )),
                CadItem::Polyline { points, .. } => {
                    for (i, p) in points.iter().enumerate() {
                        out.push(h(HandleKind::Reshape(i), *p, CursorIcon::Crosshair));
                    }
                }
                _ => {}
            }
            out
        }
        ObjectRef::Stair(id) => stairs_view::find(floor, id)
            .map(|o| stairs_view::editor_handles(&o, scale))
            .unwrap_or_default(),
        ObjectRef::Cabinet(id) => {
            placed::placed_handles(floor, placed::PlacedRef::Cabinet(id), scale)
        }
        ObjectRef::Symbol(id) => {
            placed::placed_handles(floor, placed::PlacedRef::Symbol(id), scale)
        }
        ObjectRef::Device(id) => site_view::electrical_layer(cx.floor, floor)
            .device(id)
            .map(|d| vec![h(HandleKind::Move, d.position, CursorIcon::Move)])
            .unwrap_or_default(),
        ObjectRef::RoofPlane(id) => {
            let Some(r) = roof_view::load(floor).plane(id).cloned() else {
                return Vec::new();
            };
            let mut out = vec![h(HandleKind::Move, r.centroid(), CursorIcon::Move)];
            // Corners, edge middles, the pitch arrow and the rotate knob
            // (RF-38); a dormer's roof has only the move handle.
            for (handle, at) in r.handles() {
                out.push(match handle {
                    roof_view::PlaneHandle::Vertex(i) => {
                        h(HandleKind::Reshape(i), at, CursorIcon::Crosshair)
                    }
                    roof_view::PlaneHandle::Edge(i) => {
                        h(HandleKind::EdgeMove(i), at, CursorIcon::Grab)
                    }
                    roof_view::PlaneHandle::Pitch => h(HandleKind::Pitch, at, CursorIcon::Grab),
                    roof_view::PlaneHandle::Rotate => h(HandleKind::Rotate, at, CursorIcon::Grab),
                });
            }
            out
        }
        ObjectRef::Camera(id) => cx
            .project
            .camera(id)
            .map(|c| {
                camera_tool::handles_of(c)
                    .into_iter()
                    .map(|(k, pos)| {
                        let (kind, cursor) = match k {
                            CamHandle::Move => (HandleKind::Move, CursorIcon::Move),
                            CamHandle::Aim => (HandleKind::Rotate, CursorIcon::Grab),
                            CamHandle::Clip => (HandleKind::ResizeEnd, CursorIcon::Crosshair),
                            CamHandle::EndA => (HandleKind::ResizeStart, CursorIcon::Crosshair),
                            CamHandle::EndB => (HandleKind::Reshape(1), CursorIcon::Crosshair),
                        };
                        h(kind, pos, cursor)
                    })
                    .chain(camera_tool::wedge_handles_of(c).into_iter().map(|(w, pos)| {
                        use camera_tool::WedgeHandle;
                        match w {
                            WedgeHandle::FovLeft => {
                                h(HandleKind::Reshape(2), pos, CursorIcon::Crosshair)
                            }
                            WedgeHandle::FovRight => {
                                h(HandleKind::Reshape(3), pos, CursorIcon::Crosshair)
                            }
                            WedgeHandle::Tilt => h(HandleKind::Reshape(4), pos, CursorIcon::Grab),
                        }
                    }))
                    .collect()
            })
            .unwrap_or_default(),
        // Slabs, slab holes and platform holes: one handle per corner. All
        // foundation objects move by dragging their body (a group move).
        ObjectRef::Foundation(id) => {
            let layer = FoundationLayer::load(floor);
            layer
                .find(id)
                .and_then(|r| foundation_view::outline_points(&layer, r))
                .map(|pts| {
                    pts.into_iter()
                        .enumerate()
                        .map(|(i, p)| h(HandleKind::Reshape(i), p, CursorIcon::Crosshair))
                        .collect()
                })
                .unwrap_or_default()
        }
        // Line members and layout lines: an end handle each; Truss Bases: a
        // corner handle each. Posts and markers move by their body.
        ObjectRef::Framing(id) => match framing_view::find(floor, id) {
            Some(framing_view::Record::TrussBase { base, .. }) => base
                .points
                .iter()
                .enumerate()
                .map(|(i, p)| h(HandleKind::Reshape(i), *p, CursorIcon::Crosshair))
                .collect(),
            Some(r) => r
                .line_ends()
                .map(|(a, b)| {
                    let resize = resize_cursor(b.sub(a));
                    vec![
                        h(HandleKind::ResizeStart, a, resize),
                        h(HandleKind::ResizeEnd, b, resize),
                    ]
                })
                .unwrap_or_default(),
            None => Vec::new(),
        },
        // Region, deck and outline-solid corners (a corner handle each) and
        // the points of a molding: its two ends for a line, a corner handle
        // each for a polyline. Everything else moves by its body.
        ObjectRef::Detail(id) => {
            let layer = DetailsLayer::load(floor);
            let Some(r) = layer.find(id) else {
                return Vec::new();
            };
            let Some(pts) = layer.vertices(r) else {
                return Vec::new();
            };
            if matches!(r, DetailRef::Molding(_)) && pts.len() == 2 {
                let resize = resize_cursor(pts[1].sub(pts[0]));
                return vec![
                    h(HandleKind::ResizeStart, pts[0], resize),
                    h(HandleKind::ResizeEnd, pts[1], resize),
                ];
            }
            pts.into_iter()
                .enumerate()
                .map(|(i, p)| h(HandleKind::Reshape(i), p, CursorIcon::Crosshair))
                .collect()
        }
        // A vertex handle per point of a terrain element; the elements that
        // were stored flattened from a spline (hundreds of points) move by
        // their body only.
        ObjectRef::TerrainObject(hit) => {
            let Some(view) = site_view::terrain_view(&cx.project) else {
                return Vec::new();
            };
            let pts = site_view::hit_points(&view.record.terrain, hit);
            if pts.len() > MAX_TERRAIN_HANDLES {
                return Vec::new();
            }
            pts.into_iter()
                .enumerate()
                .map(|(i, p)| h(HandleKind::Reshape(i), p, CursorIcon::Crosshair))
                .collect()
        }
        // The terrain itself selected: the corners of its perimeter, which
        // reshape the perimeter like `TerrainObject(Perimeter)`.
        ObjectRef::Terrain => {
            let Some(view) = site_view::terrain_view(&cx.project) else {
                return Vec::new();
            };
            let hit = site_view::TerrainHit::Perimeter;
            let pts = site_view::hit_points(&view.record.terrain, hit);
            if pts.len() > MAX_TERRAIN_HANDLES {
                return Vec::new();
            }
            pts.into_iter()
                .enumerate()
                .map(|(i, p)| Handle {
                    kind: HandleKind::Reshape(i),
                    pos: p,
                    cursor: CursorIcon::Crosshair,
                    target: ObjectRef::TerrainObject(hit),
                })
                .collect()
        }
        ObjectRef::Room(_)
        | ObjectRef::Schedule(_)
        | ObjectRef::Block(_)
        | ObjectRef::Solid(_) => Vec::new(),
    }
}

/// The handles of a text object (S-25, TXT-3, TXT-13): Move at the middle of
/// its box, Rotate above it, and the box handles. `ResizeEnd` on the right
/// edge sets the wrap width (the text reflows), `Reshape(0)` on the top edge
/// the minimum box height and `Reshape(1)` on the upper right corner both
/// (see `tools::text::drag_box_handle`). The box is laid out for the size the
/// text is drawn at.
fn text_handles(
    cx: &EditorContext,
    c: &plan_core::CadObject,
    scale: f64,
    h: &dyn Fn(HandleKind, Point, CursorIcon) -> Handle,
) -> Vec<Handle> {
    use plan_core::text_box::{layout, BoxLayout};
    let attrs = cx
        .floor()
        .cad_attrs(c.id)
        .unwrap_or_else(|| plan_core::cad::CadAttrs::new(c.id));
    let drawn = super::render::printed_text_object(cx, c, Some(&attrs));
    let item = drawn.as_ref().map_or(&c.item, |o| &o.item);
    let CadItem::Text {
        pos,
        text,
        height,
        angle,
    } = item
    else {
        return Vec::new();
    };
    let lay = layout(text, &attrs.runs, *height, &attrs.text_box);
    let to = |x: f64, y: f64| BoxLayout::to_plan(*pos, *angle, Point::new(x, y));
    let up = 24.0 / scale.max(1e-6);
    vec![
        h(
            HandleKind::Move,
            to(lay.width * 0.5, lay.height * 0.5),
            CursorIcon::Move,
        ),
        h(
            HandleKind::Rotate,
            to(lay.width * 0.5, lay.height + up),
            CursorIcon::Grab,
        ),
        h(
            HandleKind::ResizeEnd,
            lay.width_handle(*pos, *angle),
            CursorIcon::ResizeHorizontal,
        ),
        h(
            HandleKind::Reshape(0),
            lay.height_handle(*pos, *angle),
            CursorIcon::ResizeVertical,
        ),
        h(
            HandleKind::Reshape(1),
            lay.corner_handle(*pos, *angle),
            CursorIcon::ResizeNeSw,
        ),
    ]
}

/// Terrain elements with more points than this get no vertex handles.
pub const MAX_TERRAIN_HANDLES: usize = 40;

fn resize_cursor(v: Point) -> CursorIcon {
    let (ax, ay) = (v.x.abs(), v.y.abs());
    if ay < ax * 0.3827 {
        CursorIcon::ResizeHorizontal
    } else if ax < ay * 0.3827 {
        CursorIcon::ResizeVertical
    } else if v.x * v.y > 0.0 {
        CursorIcon::ResizeNeSw
    } else {
        CursorIcon::ResizeNwSe
    }
}

/// The handle under `p` (within `tol` inches); the nearest wins, end handles
/// before the middle one on ties.
pub fn hit_handle(handles: &[Handle], p: Point, tol: f64) -> Option<Handle> {
    handles
        .iter()
        .filter(|h| h.pos.dist(p) <= tol)
        .min_by(|a, b| {
            let da = a.pos.dist(p) + f64::from(a.kind == HandleKind::PerpendicularMove) * 1e-6;
            let db = b.pos.dist(p) + f64::from(b.kind == HandleKind::PerpendicularMove) * 1e-6;
            da.total_cmp(&db)
        })
        .copied()
}

/// Draws the handles with a fixed screen size.
pub fn draw(handles: &[Handle], painter: &egui::Painter, cam: &Camera, pal: &Palette) {
    let stroke = Stroke::new(1.5_f32, pal.selection);
    for h in handles {
        let c = cam.world_to_screen(h.pos);
        match h.kind {
            HandleKind::ResizeStart | HandleKind::ResizeEnd | HandleKind::Reshape(_) => {
                let r = Rect::from_center_size(c, Vec2::splat(9.0));
                painter.add(Shape::rect_filled(r, 0.0, pal.background));
                painter.rect_stroke(r, 0.0, stroke, egui::StrokeKind::Inside);
            }
            HandleKind::Rotate => {
                painter.circle_filled(c, 5.0, pal.background);
                painter.circle_stroke(c, 5.0, stroke);
            }
            HandleKind::Swing => {
                painter.circle_filled(c, 4.5, pal.selection);
            }
            HandleKind::Bulge => {
                painter.circle_filled(c, 5.5, pal.selection);
                painter.circle_stroke(c, 5.5, Stroke::new(1.0_f32, pal.background));
            }
            HandleKind::Label => {
                let r = Rect::from_center_size(c, Vec2::splat(8.0));
                painter.add(Shape::rect_stroke(
                    r,
                    2.0,
                    Stroke::new(1.5_f32, pal.selection),
                    egui::StrokeKind::Inside,
                ));
                painter.circle_filled(c, 1.8, pal.selection);
            }
            HandleKind::Pitch => {
                // An arrow up the slope.
                let r = 6.0;
                painter.add(Shape::convex_polygon(
                    vec![
                        c + Vec2::new(0.0, -r),
                        c + Vec2::new(r, r * 0.8),
                        c + Vec2::new(-r, r * 0.8),
                    ],
                    pal.background,
                    stroke,
                ));
            }
            HandleKind::EdgeMove(_) => {
                let r = Rect::from_center_size(c, Vec2::new(10.0, 5.0));
                painter.add(Shape::rect_filled(r, 1.0, pal.background));
                painter.rect_stroke(r, 1.0, stroke, egui::StrokeKind::Inside);
            }
            HandleKind::Annot(id) => {
                use plan_core::callout::handle as ah;
                match id {
                    ah::RESIZE => {
                        painter.circle_filled(c, 4.0, pal.background);
                        painter.circle_stroke(c, 4.0, stroke);
                    }
                    ah::EXTEND => {
                        let r = Rect::from_center_size(c, Vec2::splat(9.0));
                        painter.add(Shape::rect_filled(r, 0.0, pal.background));
                        painter.rect_stroke(r, 0.0, stroke, egui::StrokeKind::Inside);
                    }
                    ah::ROTATE => {
                        let r = 7.0;
                        painter.add(Shape::convex_polygon(
                            vec![
                                c + Vec2::new(0.0, -r),
                                c + Vec2::new(r, r * 0.8),
                                c + Vec2::new(-r, r * 0.8),
                            ],
                            pal.background,
                            stroke,
                        ));
                    }
                    ah::ADD_LINE | ah::ADD_ARROW => {
                        let r = 6.0;
                        painter.add(Shape::convex_polygon(
                            vec![
                                c + Vec2::new(0.0, -r),
                                c + Vec2::new(r, 0.0),
                                c + Vec2::new(0.0, r),
                                c + Vec2::new(-r, 0.0),
                            ],
                            pal.background,
                            stroke,
                        ));
                    }
                    _ => {
                        let r = 4.5;
                        painter.add(Shape::convex_polygon(
                            vec![
                                c + Vec2::new(0.0, -r),
                                c + Vec2::new(r, r * 0.8),
                                c + Vec2::new(-r, r * 0.8),
                            ],
                            pal.background,
                            stroke,
                        ));
                    }
                }
            }
            HandleKind::Move | HandleKind::PerpendicularMove => {
                let r = 6.0;
                painter.add(Shape::convex_polygon(
                    vec![
                        c + Vec2::new(0.0, -r),
                        c + Vec2::new(r, 0.0),
                        c + Vec2::new(0.0, r),
                        c + Vec2::new(-r, 0.0),
                    ],
                    pal.background,
                    stroke,
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::WallKind;

    #[test]
    fn details_get_corner_and_end_handles() {
        use crate::editor::details_view as dv;
        use plan_core::details::MoldingProfile;
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let sq = |s: f64| {
            vec![
                Point::new(0.0, 0.0),
                Point::new(s, 0.0),
                Point::new(s, s),
                Point::new(0.0, s),
            ]
        };
        let deck = dv::add_deck(&mut cx, sq(96.0));
        let line = dv::add_molding(
            &mut cx,
            vec![Point::new(0.0, 200.0), Point::new(100.0, 200.0)],
            MoldingProfile::Crown,
        );
        let poly = dv::add_molding(
            &mut cx,
            vec![
                Point::new(0.0, 300.0),
                Point::new(100.0, 300.0),
                Point::new(100.0, 400.0),
            ],
            MoldingProfile::Crown,
        );
        let sphere = dv::add_solid(
            &mut cx,
            plan_core::details::SolidKind::Sphere { r: 6.0 },
            Point::new(500.0, 0.0),
        );
        cx.selection.set(ObjectRef::Detail(deck));
        let hs = handles_for(&cx, 2.0);
        assert_eq!(hs.len(), 4);
        assert!(hs.iter().all(|h| matches!(h.kind, HandleKind::Reshape(_))));
        assert_eq!(hs[2].pos, Point::new(96.0, 96.0));
        cx.selection.set(ObjectRef::Detail(line));
        let ends: Vec<HandleKind> = handles_for(&cx, 2.0).iter().map(|h| h.kind).collect();
        assert_eq!(ends, vec![HandleKind::ResizeStart, HandleKind::ResizeEnd]);
        cx.selection.set(ObjectRef::Detail(poly));
        assert_eq!(handles_for(&cx, 2.0).len(), 3);
        // A sphere has no corners; it moves by its body.
        cx.selection.set(ObjectRef::Detail(sphere));
        assert!(handles_for(&cx, 2.0).is_empty());
    }

    #[test]
    fn wall_handles_and_hit_priority() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let w = cx.project.add_wall(
            0,
            Point::ZERO,
            Point::new(100.0, 0.0),
            6.0,
            100.0,
            WallKind::Interior,
        );
        assert!(handles_for(&cx, 2.0).is_empty());
        cx.selection.set(ObjectRef::Wall(w));
        let hs = handles_for(&cx, 2.0);
        assert_eq!(hs.len(), 3);
        let hit = hit_handle(&hs, Point::new(98.0, 1.0), 5.0).unwrap();
        assert_eq!(hit.kind, HandleKind::ResizeEnd);
        let mid = hit_handle(&hs, Point::new(51.0, 1.0), 5.0).unwrap();
        assert_eq!(mid.kind, HandleKind::PerpendicularMove);
        assert!(hit_handle(&hs, Point::new(30.0, 30.0), 5.0).is_none());
    }

    #[test]
    fn a_curved_wall_has_a_bulge_handle_at_its_apex() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let w = cx.project.add_wall(
            0,
            Point::ZERO,
            Point::new(100.0, 0.0),
            6.0,
            100.0,
            WallKind::Interior,
        );
        cx.selection.set(ObjectRef::Wall(w));
        assert!(!handles_for(&cx, 2.0)
            .iter()
            .any(|h| h.kind == HandleKind::Bulge));
        cx.project
            .set_wall_curve(0, w, Some(plan_core::WallCurve { bulge: 20.0 }));
        let hs = handles_for(&cx, 2.0);
        let bulge = hs.iter().find(|h| h.kind == HandleKind::Bulge).unwrap();
        assert_eq!(bulge.pos, Point::new(50.0, 20.0));
        // Right of the chord for a negative bulge.
        cx.project
            .set_wall_curve(0, w, Some(plan_core::WallCurve { bulge: -20.0 }));
        let hs = handles_for(&cx, 2.0);
        let bulge = hs.iter().find(|h| h.kind == HandleKind::Bulge).unwrap();
        assert_eq!(bulge.pos, Point::new(50.0, -20.0));
        // Near the apex the bulge handle wins over the end handles.
        let hit = hit_handle(&hs, Point::new(51.0, -19.0), 5.0).unwrap();
        assert_eq!(hit.kind, HandleKind::Bulge);
    }
}
