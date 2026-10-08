//! 3D meshes for a cabinet: carcass, toe kick, countertop, fronts, handles,
//! moldings, corner and blind cabinets, fillers, appliance bays, custom
//! countertops and backsplashes.
//!
//! `plan-3d` has no wood or laminate materials, so the carcass and fronts use
//! [`Material::WallInterior`], the countertop uses [`Material::Floor`] as a
//! stand-in and handles use [`Material::WindowFrame`] as a metal stand-in. A
//! cabinet's [`PartMaterials`] override those per part.

use plan_3d::{Material, Mesh, Vertex};
use plan_core::geometry::Point;
use plan_core::Id;

use crate::cabinet::{
    BlindSide, Cabinet, CabinetKind, DoorProfile, FaceSide, HandleStyle, HingeStyle,
    MaterialChoice, Overlay, SideKind,
};
use crate::face::{FaceItem, FaceLayout};
use crate::top::{CornerTreatment, EdgeProfile};

/// Side, back, top and bottom panel thickness, inches.
pub(crate) const PANEL: f64 = 0.75;
/// Face-frame thickness, inches.
pub(crate) const FRAME: f64 = 0.75;
/// How far handles stand proud of the front, inches.
const HANDLE_PROJECTION: f64 = 1.0;

pub(crate) type V3 = [f64; 3];

/// The six faces of a box: outward normal and the four `(ix, iy, iz)` corner
/// selectors (0 = min, 1 = max), wound counter-clockwise seen from outside.
const FACES: [([f64; 3], [[u8; 3]; 4]); 6] = [
    (
        [1.0, 0.0, 0.0],
        [[1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1]],
    ),
    (
        [-1.0, 0.0, 0.0],
        [[0, 1, 0], [0, 0, 0], [0, 0, 1], [0, 1, 1]],
    ),
    (
        [0.0, 1.0, 0.0],
        [[1, 1, 0], [0, 1, 0], [0, 1, 1], [1, 1, 1]],
    ),
    (
        [0.0, -1.0, 0.0],
        [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]],
    ),
    (
        [0.0, 0.0, 1.0],
        [[0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]],
    ),
    (
        [0.0, 0.0, -1.0],
        [[0, 1, 0], [1, 1, 0], [1, 0, 0], [0, 0, 0]],
    ),
];

/// Maps a material choice onto a `plan-3d` material; `Default` keeps
/// `fallback`.
pub(crate) fn pick(choice: MaterialChoice, fallback: Material) -> Material {
    match choice {
        MaterialChoice::Default => fallback,
        MaterialChoice::Wood => Material::WallInterior,
        MaterialChoice::Painted => Material::Trim,
        MaterialChoice::Stone => Material::Stone,
        MaterialChoice::Concrete => Material::Concrete,
        MaterialChoice::Metal => Material::Metal,
        MaterialChoice::Glass => Material::Glass,
    }
}

/// Local-frame to scene-frame transform plus the output mesh list.
pub(crate) struct Builder {
    id: Id,
    x: f64,
    y: f64,
    cos: f64,
    sin: f64,
    elevation: f64,
    pub(crate) meshes: Vec<Mesh>,
}

impl Builder {
    pub(crate) fn new(cab: &Cabinet) -> Self {
        let (sin, cos) = cab.angle.sin_cos();
        Self {
            id: cab.id,
            x: cab.position.x,
            y: cab.position.y,
            cos,
            sin,
            elevation: cab.elevation,
            meshes: Vec::new(),
        }
    }

    /// Local `(x, y, z-up)` to scene `(X, Y-up, Z = -plan y)`.
    pub(crate) fn point(&self, p: V3) -> [f32; 3] {
        let px = self.x + p[0] * self.cos - p[1] * self.sin;
        let py = self.y + p[0] * self.sin + p[1] * self.cos;
        [px as f32, (self.elevation + p[2]) as f32, (-py) as f32]
    }

    pub(crate) fn direction(&self, n: V3) -> [f32; 3] {
        let nx = n[0] * self.cos - n[1] * self.sin;
        let ny = n[0] * self.sin + n[1] * self.cos;
        [nx as f32, n[2] as f32, (-ny) as f32]
    }

    pub(crate) fn push(&mut self, vertices: Vec<Vertex>, indices: Vec<u32>, material: Material) {
        if indices.is_empty() {
            return;
        }
        self.meshes.push(Mesh {
            vertices,
            indices,
            material,
            object_id: Some(self.id),
        });
    }

    /// Add an axis-aligned (in the local frame) box as one mesh. Degenerate
    /// boxes are skipped.
    pub(crate) fn add_box(&mut self, min: V3, max: V3, material: Material) {
        if (0..3).any(|k| max[k] - min[k] <= 1e-9) {
            return;
        }
        let mut vertices = Vec::with_capacity(24);
        let mut indices = Vec::with_capacity(36);
        for (normal, corners) in FACES {
            let axis = (0..3).find(|&k| normal[k] != 0.0).unwrap_or(0);
            let (ua, va) = match axis {
                0 => (1, 2),
                1 => (0, 2),
                _ => (0, 1),
            };
            let base = vertices.len() as u32;
            for sel in corners {
                let p: V3 = std::array::from_fn(|k| if sel[k] == 1 { max[k] } else { min[k] });
                vertices.push(Vertex {
                    position: self.point(p),
                    normal: self.direction(normal),
                    uv: [(p[ua] / 12.0) as f32, (p[va] / 12.0) as f32],
                });
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        self.push(vertices, indices, material);
    }
}

/// A front line of a cabinet: a start point, the direction along it, the
/// outward normal (the direction dir rotated 90 degrees counter-clockwise)
/// and its length. The ordinary cabinet front is [`Frame::front`].
#[derive(Clone, Copy)]
pub(crate) struct Frame {
    pub(crate) origin: Point,
    pub(crate) dir: Point,
    pub(crate) nrm: Point,
    pub(crate) len: f64,
}

impl Frame {
    /// The front of a rectangular cabinet.
    pub(crate) fn front(w: f64, d: f64) -> Frame {
        Frame {
            origin: Point::new(0.0, d),
            dir: Point::new(1.0, 0.0),
            nrm: Point::new(0.0, 1.0),
            len: w,
        }
    }

    /// A front running from `from` to `to`, facing the left of that run
    /// rotated -90 degrees... i.e. `nrm` is `dir` turned counter-clockwise.
    pub(crate) fn along(from: Point, to: Point) -> Frame {
        let dir = to.sub(from).normalized();
        Frame {
            origin: from,
            dir,
            nrm: dir.perp(),
            len: from.dist(to),
        }
    }

    fn is_front(&self) -> bool {
        self.dir == Point::new(1.0, 0.0) && self.nrm == Point::new(0.0, 1.0) && self.origin.x == 0.0
    }

    pub(crate) fn pt(&self, x: f64, y: f64) -> Point {
        self.origin.add(self.dir.scale(x)).add(self.nrm.scale(y))
    }
}

/// A box in frame coordinates: `x0..x1` along the front, `z0..z1` up and
/// `y0..y1` out from the front line (negative is behind it).
pub(crate) fn frame_box(
    b: &mut Builder,
    f: Frame,
    (x0, x1): (f64, f64),
    (z0, z1): (f64, f64),
    (y0, y1): (f64, f64),
    material: Material,
) {
    if f.is_front() {
        b.add_box(
            [x0, f.origin.y + y0, z0],
            [x1, f.origin.y + y1, z1],
            material,
        );
    } else {
        let ring = [f.pt(x0, y0), f.pt(x1, y0), f.pt(x1, y1), f.pt(x0, y1)];
        b.add_prism(&ring, &[], z0, z1, material);
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Hinge {
    Left,
    Right,
}

/// Everything the front builders share about the cabinet being meshed.
pub(crate) struct FrontCtx<'a> {
    pub(crate) cab: &'a Cabinet,
    /// The face items to build (the front's, or a side's).
    pub(crate) layout: &'a FaceLayout,
    pub(crate) frame: Frame,
    /// Y of the carcass front relative to the front line (`cf - depth`).
    pub(crate) carcass_y: f64,
    /// Lowest and highest z of the face.
    pub(crate) z0: f64,
    pub(crate) z1: f64,
}

/// Builds the face items (rails, drawers, doors, panels) along `ctx.frame`.
/// `x_off` is where the face starts along the frame and `face_w` its width.
pub(crate) fn build_fronts(b: &mut Builder, ctx: &FrontCtx, x_off: f64, face_w: f64) {
    let cab = ctx.cab;
    let wi = pick(cab.materials.carcass, Material::WallInterior);
    let Ok(items) = ctx.layout.resolve(ctx.z1 - ctx.z0, face_w) else {
        return;
    };
    for r in items {
        let (x, y, iw, ih) = r.rect;
        let (rx0, rx1, rz0, rz1) = (x_off + x, x_off + x + iw, ctx.z0 + y, ctx.z0 + y + ih);
        let rect = (rx0, rx1, rz0, rz1);
        match &r.item {
            FaceItem::Separation { .. } => {
                if cab.framed {
                    frame_box(
                        b,
                        ctx.frame,
                        (rx0, rx1),
                        (rz0, rz1),
                        (ctx.carcass_y - FRAME, ctx.carcass_y),
                        wi,
                    );
                }
            }
            FaceItem::Opening { .. } => {}
            FaceItem::Drawer { .. } => drawer(b, ctx, rect),
            FaceItem::Appliance { .. } | FaceItem::Panel { .. } => {
                let style = &cab.door_style;
                front_panel(
                    b,
                    ctx,
                    rect,
                    style.thickness,
                    DoorProfile::Slab,
                    false,
                    pick(cab.materials.door, Material::WallInterior),
                );
            }
            FaceItem::DoorLeft { .. } => door(b, ctx, rect, Hinge::Left),
            FaceItem::DoorRight { .. } => door(b, ctx, rect, Hinge::Right),
            FaceItem::DoorAuto { .. } => {
                // Hinge toward the nearer end of the face; centred doors hinge right.
                let hinge = if (rx0 + rx1) / 2.0 < ctx.frame.len / 2.0 - 1e-9 {
                    Hinge::Left
                } else {
                    Hinge::Right
                };
                door(b, ctx, rect, hinge);
            }
            FaceItem::DoubleDoor { .. } => {
                let mid = (rx0 + rx1) / 2.0;
                door(b, ctx, (rx0, mid, rz0, rz1), Hinge::Left);
                door(b, ctx, (mid, rx1, rz0, rz1), Hinge::Right);
            }
            FaceItem::HorizontalLayout { .. } => {}
        }
    }
}

/// Build the cabinet's 3D meshes.
///
/// * Carcass: side, back and bottom panels (plus a top panel unless it is a
///   base cabinet, whose countertop acts as the top), 3/4" thick.
/// * Base/full-height toe kick: a 3/4" board whose front face is `depth` back
///   from the cabinet front, spanning the toe kick height.
/// * Countertop slab with its overhangs (with holes for sink and cooktop
///   cutouts and their fixtures), and an optional backsplash.
/// * Framed cabinets: stiles at both ends and a rail for every separation.
/// * Each drawer, door and appliance front as a panel (door/drawer thickness)
///   adjusted by the [`Overlay`] and built as a slab, a shaker frame or a
///   raised panel, with a small box handle (and two hinge blocks for exposed
///   hinges). Openings and separations in frameless cabinets get no geometry.
/// * Moldings: front and both returns per crown or light rail.
/// * Fillers are just their front panel (plus top and toe kick); a blind
///   cabinet closes its hidden end with a solid panel; an appliance bay has
///   two side panels around a placeholder appliance box.
/// * Corner cabinets are built from their L or diagonal footprint (see
///   `corner`), custom countertops and backsplashes as extruded outlines.
///
/// Shelves are a single board and partitions a single 3/4" vertical panel at
/// the local origin end; a soffit is one solid box. If the face layout cannot
/// be resolved (fixed items exceed the face) only the carcass is produced.
pub fn meshes(cabinet: &Cabinet) -> Vec<Mesh> {
    let mut b = Builder::new(cabinet);
    let (w, d, h) = (cabinet.width, cabinet.depth, cabinet.height);

    match cabinet.kind {
        CabinetKind::Shelf => {
            b.add_box(
                [0.0, 0.0, (h - PANEL).max(0.0)],
                [w, d, h],
                Material::WallInterior,
            );
            return b.meshes;
        }
        CabinetKind::Partition => {
            b.add_box(
                [0.0, 0.0, 0.0],
                [PANEL.min(w), d, h],
                Material::WallInterior,
            );
            return b.meshes;
        }
        CabinetKind::Soffit if cabinet.custom.is_some() => {
            if let Some(c) = &cabinet.custom {
                b.add_prism(
                    &c.outline,
                    &[],
                    0.0,
                    h,
                    pick(cabinet.materials.carcass, Material::WallInterior),
                );
            }
            return b.meshes;
        }
        CabinetKind::Soffit => {
            b.add_box(
                [0.0, 0.0, 0.0],
                [w, d, h],
                pick(cabinet.materials.carcass, Material::WallInterior),
            );
            return b.meshes;
        }
        CabinetKind::CustomCountertop => {
            b.custom_countertop(cabinet);
            return b.meshes;
        }
        CabinetKind::CustomBacksplash => {
            b.custom_backsplash(cabinet);
            return b.meshes;
        }
        CabinetKind::CounterHole => return b.meshes,
        CabinetKind::CornerBase | CabinetKind::CornerWall => {
            b.corner(cabinet);
            b.moldings(cabinet);
            return b.meshes;
        }
        _ => {}
    }
    b.rectangular(cabinet);
    b.moldings(cabinet);
    b.meshes
}

impl Builder {
    /// Base, wall, full-height, filler, blind and appliance-bay cabinets.
    fn rectangular(&mut self, cabinet: &Cabinet) {
        let (w, d, h) = (cabinet.width, cabinet.depth, cabinet.height);
        let toe = cabinet.toe_kick.filter(|t| t.height > 0.0);
        let z0 = toe.map_or(0.0, |t| t.height);
        let top = cabinet.countertop;
        let z1 = h - top.map_or(0.0, |c| c.thickness);
        let door_t = cabinet.door_style.thickness;
        let front_t = door_t.max(cabinet.drawer_style.thickness);
        // Front of the carcass: inset fronts sit flush with it, others stand proud
        // of it so the overall depth stays `depth`.
        let cf = match cabinet.overlay {
            Overlay::Inset { .. } => d,
            _ => d - front_t,
        };
        let wi = pick(cabinet.materials.carcass, Material::WallInterior);
        let bay = cabinet.appliance.is_some();
        let filler = cabinet.kind.is_filler();

        // Carcass.
        if bay {
            self.add_box([0.0, 0.0, z0], [PANEL, d, z1], wi);
            self.add_box([w - PANEL, 0.0, z0], [w, d, z1], wi);
        } else if !filler {
            let plain = |side| cabinet.side_kind(side) == SideKind::Plain;
            if plain(FaceSide::Left) {
                self.add_box([0.0, 0.0, z0], [PANEL, cf, z1], wi);
            }
            if plain(FaceSide::Right) {
                self.add_box([w - PANEL, 0.0, z0], [w, cf, z1], wi);
            }
            if plain(FaceSide::Back) {
                self.add_box([PANEL, 0.0, z0], [w - PANEL, PANEL, z1], wi);
            }
            self.add_box([PANEL, PANEL, z0], [w - PANEL, cf, z0 + PANEL], wi);
            if !cabinet.kind.is_base_like() {
                self.add_box([PANEL, PANEL, z1 - PANEL], [w - PANEL, cf, z1], wi);
            }
        }

        // Toe kick board, front face `depth` back from the cabinet front.
        if let Some(tk) = toe {
            let front = d - tk.depth;
            self.add_box(
                [0.0, (front - FRAME).max(0.0), 0.0],
                [w, front, tk.height],
                pick(cabinet.materials.toe_kick, Material::WallInterior),
            );
        }

        // Countertop and backsplash. A cabinet that gave its slab to a
        // generated top keeps its backsplash, standing on that top.
        if top.is_some() {
            self.countertop(cabinet, z1, h);
        }
        if let Some(bs) = cabinet.backsplash.filter(|s| s.height > 0.0) {
            let (side, back) = top.map_or((0.0, 0.0), |t| (t.overhang_sides, t.overhang_back));
            if top.is_some() || bs.lift > 0.0 {
                self.add_box(
                    [-side, -back, h + bs.lift],
                    [w + side, -back + bs.thickness, h + bs.lift + bs.height],
                    pick(cabinet.materials.backsplash, Material::Floor),
                );
            }
        }

        if bay {
            // Placeholder appliance filling the bay.
            self.add_box(
                [PANEL + 0.1, 1.0, z0 + 0.5],
                [w - PANEL - 0.1, d - 0.5, (z1 - 0.25).max(z0 + 1.0)],
                Material::Metal,
            );
            return;
        }

        // Face frame and fronts.
        let fw = if cabinet.framed {
            cabinet.face.frame_width
        } else {
            0.0
        };
        let (x_lo, x_hi) = match cabinet.blind {
            Some(bl) => match bl.side {
                BlindSide::Left => (bl.blind_width.min(w), w),
                BlindSide::Right => (0.0, (w - bl.blind_width).max(0.0)),
            },
            None => (0.0, w),
        };
        let face_w = x_hi - x_lo - 2.0 * fw;
        if face_w <= 0.0 || z1 <= z0 {
            return;
        }
        if let Some(bl) = cabinet.blind {
            // The hidden end: a solid front panel.
            let (bx0, bx1) = match bl.side {
                BlindSide::Left => (0.0, x_lo),
                BlindSide::Right => (x_hi, w),
            };
            let ctx = FrontCtx {
                cab: cabinet,
                layout: &cabinet.face,
                frame: Frame::front(w, d),
                carcass_y: cf - d,
                z0,
                z1,
            };
            front_panel(
                self,
                &ctx,
                (bx0, bx1, z0, z1),
                door_t,
                DoorProfile::Slab,
                false,
                pick(cabinet.materials.door, Material::WallInterior),
            );
        }
        if cabinet.framed {
            self.add_box([x_lo, cf - FRAME, z0], [x_lo + fw, cf, z1], wi);
            self.add_box([x_hi - fw, cf - FRAME, z0], [x_hi, cf, z1], wi);
        }
        let ctx = FrontCtx {
            cab: cabinet,
            layout: &cabinet.face,
            frame: Frame::front(w, d),
            carcass_y: cf - d,
            z0,
            z1,
        };
        build_fronts(self, &ctx, x_lo + fw, face_w);
        self.side_faces(cabinet, z0, z1);
    }

    /// The Left, Right and Back faces that are not the plain carcass panel:
    /// a finished slab, an open side or doors and drawers (`SideFace`). The
    /// panel of the carcass is left out where one is built.
    fn side_faces(&mut self, cabinet: &Cabinet, z0: f64, z1: f64) {
        let (w, d) = (cabinet.width, cabinet.depth);
        for side in [FaceSide::Left, FaceSide::Right, FaceSide::Back] {
            let Some(sf) = cabinet.side_face(side) else {
                continue;
            };
            let frame = match side {
                FaceSide::Left => Frame {
                    origin: Point::new(0.0, 0.0),
                    dir: Point::new(0.0, 1.0),
                    nrm: Point::new(-1.0, 0.0),
                    len: d,
                },
                FaceSide::Right => Frame {
                    origin: Point::new(w, d),
                    dir: Point::new(0.0, -1.0),
                    nrm: Point::new(1.0, 0.0),
                    len: d,
                },
                _ => Frame {
                    origin: Point::new(w, 0.0),
                    dir: Point::new(-1.0, 0.0),
                    nrm: Point::new(0.0, -1.0),
                    len: w,
                },
            };
            let t = cabinet.door_style.thickness;
            match sf.kind {
                SideKind::Plain | SideKind::Open => {}
                SideKind::Finished => frame_box(
                    self,
                    frame,
                    (0.0, frame.len),
                    (z0, z1),
                    (-t, 0.0),
                    pick(cabinet.materials.door, Material::WallInterior),
                ),
                SideKind::CustomFace => {
                    let ctx = FrontCtx {
                        cab: cabinet,
                        layout: &sf.layout,
                        frame,
                        carcass_y: -t,
                        z0,
                        z1,
                    };
                    build_fronts(self, &ctx, 0.0, frame.len);
                }
            }
        }
    }

    /// The countertop slab of a cabinet's own top, with holes and fixtures.
    pub(crate) fn countertop(&mut self, cabinet: &Cabinet, z1: f64, h: f64) {
        let material = pick(cabinet.materials.countertop, Material::Floor);
        if cabinet.cutouts.is_empty() {
            if let Some(t) = cabinet.countertop {
                let shaped = t.corner != CornerTreatment::None || t.edge != EdgeProfile::Square;
                if cabinet.kind.is_corner() || shaped {
                    if let Some(ring) = cabinet.top_local() {
                        self.add_slab(&ring, &[], (z1, h), t.edge, t.edge_size, material);
                    }
                } else {
                    self.add_box(
                        [-t.overhang_sides, -t.overhang_back, z1],
                        [
                            cabinet.width + t.overhang_sides,
                            cabinet.depth + t.overhang_front,
                            h,
                        ],
                        material,
                    );
                }
            }
        } else if let Some(ring) = cabinet.top_local() {
            let (edge, size) = cabinet
                .countertop
                .map_or((EdgeProfile::Square, 0.0), |t| (t.edge, t.edge_size));
            self.add_slab(&ring, &cabinet.holes_local(), (z1, h), edge, size, material);
            self.fixtures(cabinet, h);
        }
        self.waterfall_sides(cabinet, h, material);
    }

    /// The Waterfall edge: a slab as thick as the top runs from the top
    /// surface to the floor beside each end of the cabinet.
    fn waterfall_sides(&mut self, cabinet: &Cabinet, h: f64, material: Material) {
        let Some(t) = cabinet
            .countertop
            .filter(|t| t.edge == EdgeProfile::Waterfall)
        else {
            return;
        };
        if cabinet.kind.is_corner() || t.thickness <= 0.0 {
            return;
        }
        let (y0, y1) = (-t.overhang_back, cabinet.depth + t.overhang_front);
        let left = -t.overhang_sides;
        let right = cabinet.width + t.overhang_sides;
        self.add_box([left - t.thickness, y0, 0.0], [left, y1, h], material);
        self.add_box([right, y0, 0.0], [right + t.thickness, y1, h], material);
    }
}

/// Panel rect `(x0, x1, z0, z1)` adjusted for the overlay and clamped to the face.
fn overlay_rect(
    cab: &Cabinet,
    r: (f64, f64, f64, f64),
    z: (f64, f64),
    len: f64,
) -> (f64, f64, f64, f64) {
    let grow = match cab.overlay {
        Overlay::Full { reveal } => -reveal / 2.0,
        Overlay::Traditional { overlap } => overlap,
        Overlay::Inset { clearance } => -clearance,
    };
    (
        (r.0 - grow).max(0.0).min(len),
        (r.1 + grow).max(0.0).min(len),
        (r.2 - grow).max(z.0),
        (r.3 + grow).min(z.1),
    )
}

/// A front panel flush with the front line, built as a slab, a shaker frame
/// or a raised panel. Glass fronts are shaker frames with a glass centre.
fn front_panel(
    b: &mut Builder,
    ctx: &FrontCtx,
    r: (f64, f64, f64, f64),
    thickness: f64,
    profile: DoorProfile,
    glass: bool,
    material: Material,
) {
    let (x0, x1, z0, z1) = overlay_rect(ctx.cab, r, (ctx.z0, ctx.z1), ctx.frame.len);
    let f = ctx.frame;
    let profile = if glass && profile == DoorProfile::Slab {
        DoorProfile::Shaker
    } else {
        profile
    };
    let (w, h) = (x1 - x0, z1 - z0);
    let rail = ctx.cab.door_style.frame_width;
    if profile == DoorProfile::Slab || w <= 2.0 * rail + 0.5 || h <= 2.0 * rail + 0.5 {
        frame_box(b, f, (x0, x1), (z0, z1), (-thickness, 0.0), material);
        return;
    }
    // Frame: two stiles, two rails; then the centre panel.
    let y = (-thickness, 0.0);
    frame_box(b, f, (x0, x0 + rail), (z0, z1), y, material);
    frame_box(b, f, (x1 - rail, x1), (z0, z1), y, material);
    frame_box(b, f, (x0 + rail, x1 - rail), (z0, z0 + rail), y, material);
    frame_box(b, f, (x0 + rail, x1 - rail), (z1 - rail, z1), y, material);
    let (centre, cy) = match profile {
        DoorProfile::Raised => (material, (-thickness * 0.75, -0.05)),
        _ => (material, (-thickness * 0.65, -thickness * 0.3)),
    };
    let centre = if glass { Material::Glass } else { centre };
    frame_box(
        b,
        f,
        (x0 + rail, x1 - rail),
        (z0 + rail, z1 - rail),
        cy,
        centre,
    );
}

/// How far an open drawer stands out of the cabinet, inches (at most).
const DRAWER_PULL_OUT: f64 = 14.0;
/// Thickness of a drawer box's sides, back and bottom, inches.
const DRAWER_BOX: f64 = 0.5;
/// Tallest gap between shelves in the interior shown by an open door, in.
const SHELF_SPACING: f64 = 13.0;
/// Height above the cabinet bottom where a tall door's handle sits, inches.
const TALL_HANDLE_Z: f64 = 38.0;

/// The frame of a door swung 90 degrees open about the hinge at `hx` (along
/// `f`), toward the outside of the front.
fn open_frame(f: Frame, hx: f64, hinge: Hinge) -> Frame {
    let pivot = f.pt(hx, 0.0);
    // Left hinge: the leaf runs along +dir, so turn counter-clockwise to put
    // its free edge on the outward normal; right hinge: clockwise.
    let rot = |v: Point| match hinge {
        Hinge::Left => Point::new(-v.y, v.x),
        Hinge::Right => Point::new(v.y, -v.x),
    };
    Frame {
        origin: pivot.add(rot(f.origin.sub(pivot))),
        dir: rot(f.dir),
        nrm: rot(f.nrm),
        len: f.len,
    }
}

/// A copy of `ctx` with another front line.
fn with_frame<'a>(ctx: &FrontCtx<'a>, frame: Frame) -> FrontCtx<'a> {
    FrontCtx {
        cab: ctx.cab,
        layout: ctx.layout,
        frame,
        carcass_y: ctx.carcass_y,
        z0: ctx.z0,
        z1: ctx.z1,
    }
}

/// A door panel with its handle on the free edge and hinges on the other.
/// With Opening Indicators in 3D the leaf stands open at 90 degrees and the
/// shelves inside show.
fn door(b: &mut Builder, ctx: &FrontCtx, r: (f64, f64, f64, f64), hinge: Hinge) {
    let cab = ctx.cab;
    let style = &cab.door_style;
    let (x0, x1, z0, z1) = overlay_rect(cab, r, (ctx.z0, ctx.z1), ctx.frame.len);
    let open = cab.indicators_3d && ctx.frame.is_front();
    let leaf = if open {
        let hx = match hinge {
            Hinge::Left => x0,
            Hinge::Right => x1,
        };
        shelves(b, ctx, r);
        with_frame(ctx, open_frame(ctx.frame, hx, hinge))
    } else {
        with_frame(ctx, ctx.frame)
    };
    front_panel(
        b,
        &leaf,
        r,
        style.thickness,
        style.profile,
        style.glass,
        pick(cab.materials.door, Material::WallInterior),
    );
    door_hardware(b, &leaf, (x0, x1, z0, z1), hinge);
    if style.hinge == HingeStyle::Exposed {
        let hx = match hinge {
            Hinge::Left => (x0, x0 + 0.5),
            Hinge::Right => (x1 - 0.5, x1),
        };
        for hz in [z0 + style.hinge_from_edge, z1 - style.hinge_from_edge] {
            frame_box(
                b,
                leaf.frame,
                hx,
                (hz - 1.0, hz + 1.0),
                (0.0, 0.1),
                Material::Metal,
            );
        }
    }
}

/// Where a door's handle sits (Chief's door hardware): `handle_from_edge`
/// off the free edge; vertically `handle_from_top` off the top edge of a
/// base door or the bottom edge of a wall door (a pull is measured to its
/// near end, a knob to its centre); tall doors that cross 38" above the
/// floor carry it there; `handle_centered` centres it.
fn door_hardware(b: &mut Builder, ctx: &FrontCtx, r: (f64, f64, f64, f64), hinge: Hinge) {
    let cab = ctx.cab;
    let style = &cab.door_style;
    if style.handle == HandleStyle::None {
        return;
    }
    let (x0, x1, z0, z1) = r;
    let len = style.handle_length.max(1.0);
    // A pull is long, a knob a point; the offset runs to the pull's end.
    let half = match style.handle {
        HandleStyle::Pull | HandleStyle::Edge => len / 2.0,
        _ => 0.0,
    };
    let cx = match (style.handle, hinge) {
        (HandleStyle::Edge, Hinge::Left) => x1 - 0.5,
        (HandleStyle::Edge, Hinge::Right) => x0 + 0.5,
        (_, Hinge::Left) => x1 - style.handle_from_edge,
        (_, Hinge::Right) => x0 + style.handle_from_edge,
    };
    let tall = matches!(
        cab.kind,
        CabinetKind::FullHeight | CabinetKind::FullHeightFiller
    );
    let cz = if style.handle_centered {
        (z0 + z1) / 2.0
    } else if tall && z0 <= TALL_HANDLE_Z - 3.0 && z1 >= TALL_HANDLE_Z + 3.0 {
        TALL_HANDLE_Z
    } else if cab.kind.is_wall_like() || (tall && z0 > TALL_HANDLE_Z) {
        z0 + style.handle_from_top + half
    } else {
        z1 - style.handle_from_top - half
    };
    let cz = cz.clamp(z0.min(z1), z1.max(z0));
    handle(b, ctx.frame, style.handle, cx, cz, false, len);
}

/// A drawer front with its handle; with Opening Indicators in 3D the drawer
/// stands out of the cabinet with its box behind the front.
fn drawer(b: &mut Builder, ctx: &FrontCtx, rect: (f64, f64, f64, f64)) {
    let cab = ctx.cab;
    let style = &cab.drawer_style;
    let (rx0, rx1, rz0, rz1) = rect;
    let open = cab.indicators_3d && ctx.frame.is_front();
    let pull = if open {
        (cab.depth * 0.5).min(DRAWER_PULL_OUT)
    } else {
        0.0
    };
    let mut f = ctx.frame;
    f.origin = f.origin.add(f.nrm.scale(pull));
    let out = with_frame(ctx, f);
    front_panel(
        b,
        &out,
        rect,
        style.thickness,
        style.profile,
        false,
        pick(cab.materials.drawer, Material::WallInterior),
    );
    if open {
        drawer_box(b, &out, rect);
    }
    let (cx, cz) = ((rx0 + rx1) / 2.0, (rz0 + rz1) / 2.0);
    let len = style.handle_length.max(1.0);
    let cz = match style.handle {
        HandleStyle::Cup => rz1 - 1.25,
        HandleStyle::Edge => rz1 - 0.5,
        _ if style.handle_centered => cz,
        _ => (rz1 - style.handle_from_top).max(rz0),
    };
    handle(b, f, style.handle, cx, cz, true, len);
}

/// The box of an open drawer: bottom, two sides and a back behind the front.
fn drawer_box(b: &mut Builder, ctx: &FrontCtx, r: (f64, f64, f64, f64)) {
    let cab = ctx.cab;
    let (x0, x1, z0, z1) = overlay_rect(cab, r, (ctx.z0, ctx.z1), ctx.frame.len);
    let t = cab.drawer_style.thickness;
    let (bx0, bx1) = (x0 + 1.0, x1 - 1.0);
    let (bz0, bz1) = (z0 + 0.75, (z1 - 0.75).max(z0 + 1.5));
    let length = (cab.depth - t - 3.0).max(6.0);
    let (y0, y1) = (-t - length, -t);
    if bx1 - bx0 < 2.0 {
        return;
    }
    let wood = pick(cab.materials.drawer, Material::WallInterior);
    let f = ctx.frame;
    frame_box(b, f, (bx0, bx1), (bz0, bz0 + DRAWER_BOX), (y0, y1), wood);
    frame_box(b, f, (bx0, bx0 + DRAWER_BOX), (bz0, bz1), (y0, y1), wood);
    frame_box(b, f, (bx1 - DRAWER_BOX, bx1), (bz0, bz1), (y0, y1), wood);
    frame_box(b, f, (bx0, bx1), (bz0, bz1), (y0, y0 + DRAWER_BOX), wood);
}

/// Adjustable shelves inside the opening behind a door that stands open.
fn shelves(b: &mut Builder, ctx: &FrontCtx, r: (f64, f64, f64, f64)) {
    let cab = ctx.cab;
    let (x0, x1, z0, z1) = r;
    let cf = ctx.carcass_y + cab.depth;
    let (xa, xb) = (x0.max(PANEL), x1.min(cab.width - PANEL));
    let n = ((z1 - z0) / SHELF_SPACING).floor() as usize;
    if xb - xa < 3.0 || cf - PANEL < 3.0 {
        return;
    }
    let wood = pick(cab.materials.carcass, Material::WallInterior);
    for k in 1..=n.min(6) {
        let z = z0 + (z1 - z0) * k as f64 / (n + 1) as f64;
        b.add_box(
            [xa, PANEL, z - PANEL / 2.0],
            [xb, cf - 0.5, z + PANEL / 2.0],
            wood,
        );
    }
}

/// A handle centred at `(cx, cz)` on the front plane. `horizontal` pulls
/// (drawers) run along the front, the rest stand upright; `len` is a pull's
/// length.
fn handle(
    b: &mut Builder,
    f: Frame,
    style: HandleStyle,
    cx: f64,
    cz: f64,
    horizontal: bool,
    len: f64,
) {
    let part = |b: &mut Builder, x: (f64, f64), z: (f64, f64), y: (f64, f64)| {
        frame_box(b, f, x, z, y, Material::WindowFrame);
    };
    match style {
        HandleStyle::None => {}
        HandleStyle::Knob => part(
            b,
            (cx - 0.5, cx + 0.5),
            (cz - 0.5, cz + 0.5),
            (0.0, HANDLE_PROJECTION),
        ),
        HandleStyle::Pull => {
            // A bar on two posts.
            let (a, bw) = (len / 2.0, 0.25);
            let post = 0.4;
            let (bar_x, bar_z) = if horizontal {
                ((cx - a, cx + a), (cz - bw, cz + bw))
            } else {
                ((cx - bw, cx + bw), (cz - a, cz + a))
            };
            part(
                b,
                bar_x,
                bar_z,
                (HANDLE_PROJECTION - 2.0 * bw, HANDLE_PROJECTION),
            );
            let ends = [-(a - post), a - post];
            for e in ends {
                let (px, pz) = if horizontal {
                    ((cx + e - post, cx + e + post), (cz - bw, cz + bw))
                } else {
                    ((cx - bw, cx + bw), (cz + e - post, cz + e + post))
                };
                part(b, px, pz, (0.0, HANDLE_PROJECTION - 2.0 * bw));
            }
        }
        HandleStyle::Cup => part(b, (cx - 1.5, cx + 1.5), (cz - 0.75, cz + 0.75), (0.0, 0.5)),
        HandleStyle::Edge => {
            let a = len / 2.0;
            if horizontal {
                part(b, (cx - a, cx + a), (cz - 0.4, cz + 0.4), (0.0, 0.6));
            } else {
                part(b, (cx - 0.4, cx + 0.4), (cz - a, cz + a), (0.0, 0.6));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::geometry::Point;

    fn max_y(m: &Mesh) -> f32 {
        m.bounds().unwrap().1[1]
    }

    #[test]
    fn base_has_countertop_at_36_and_recessed_toe_kick() {
        let ms = meshes(&Cabinet::base(24.0));
        let top = ms
            .iter()
            .find(|m| m.material == Material::Floor)
            .expect("countertop");
        assert!((max_y(top) - 36.0).abs() < 1e-4);
        let (lo, hi) = top.bounds().unwrap();
        assert!((lo[1] - 34.5).abs() < 1e-4);
        // Front overhang of 1": plan y = 25, scene z = -25.
        assert!((lo[2] + 25.0).abs() < 1e-4);
        assert!((hi[2] - 0.0).abs() < 1e-4);

        // Toe kick board: bottom at 0, top at 4, front face 3" back (z = -21).
        let kick = ms
            .iter()
            .find(|m| {
                let (lo, hi) = m.bounds().unwrap();
                lo[1].abs() < 1e-4 && (hi[1] - 4.0).abs() < 1e-4
            })
            .expect("toe kick board");
        assert!((kick.bounds().unwrap().0[2] + 21.0).abs() < 1e-4);
    }

    #[test]
    fn all_triangles_face_outward() {
        let mut c = Cabinet::sink_base(36.0);
        c.angle = 0.7;
        c.position = Point::new(5.0, 9.0);
        c.backsplash = Some(crate::Backsplash::new(4.0, 0.5));
        for m in meshes(&c) {
            for t in m.indices.chunks(3) {
                let v: Vec<[f32; 3]> = t.iter().map(|&i| m.vertices[i as usize].position).collect();
                let n = m.vertices[t[0] as usize].normal;
                let e1 = [v[1][0] - v[0][0], v[1][1] - v[0][1], v[1][2] - v[0][2]];
                let e2 = [v[2][0] - v[0][0], v[2][1] - v[0][1], v[2][2] - v[0][2]];
                let cr = [
                    e1[1] * e2[2] - e1[2] * e2[1],
                    e1[2] * e2[0] - e1[0] * e2[2],
                    e1[0] * e2[1] - e1[1] * e2[0],
                ];
                let dot = cr[0] * n[0] + cr[1] * n[1] + cr[2] * n[2];
                assert!(dot > 0.0, "inward triangle in {:?}", m.material);
            }
        }
    }

    #[test]
    fn wall_cabinet_sits_at_54_with_fronts_and_no_countertop() {
        let ms = meshes(&Cabinet::wall(30.0));
        assert!(ms.iter().all(|m| m.material != Material::Floor));
        let (lo, hi) = ms.iter().flat_map(|m| m.vertices.iter()).fold(
            ([f32::MAX; 3], [f32::MIN; 3]),
            |(mut lo, mut hi), v| {
                for k in 0..3 {
                    lo[k] = lo[k].min(v.position[k]);
                    hi[k] = hi[k].max(v.position[k]);
                }
                (lo, hi)
            },
        );
        assert!((lo[1] - 54.0).abs() < 1e-4 && (hi[1] - 84.0).abs() < 1e-4);
        // A door handle (WindowFrame) exists.
        assert!(ms.iter().any(|m| m.material == Material::WindowFrame));
    }

    #[test]
    fn opening_gets_no_front_and_bad_layout_keeps_carcass() {
        let mut c = Cabinet::base(24.0);
        let with_door = meshes(&c).len();
        c.face.items = vec![FaceItem::Opening { height: 0.0 }];
        assert!(meshes(&c).len() < with_door);
        c.face.items = vec![FaceItem::Drawer { height: 99.0 }];
        let carcass_only = meshes(&c);
        assert!(!carcass_only.is_empty());
        assert!(carcass_only
            .iter()
            .all(|m| m.material != Material::WindowFrame));
    }

    /// Bounds of every mesh with the given material.
    fn bounds_of(ms: &[Mesh], material: Material) -> Vec<([f32; 3], [f32; 3])> {
        ms.iter()
            .filter(|m| m.material == material)
            .filter_map(Mesh::bounds)
            .collect()
    }

    #[test]
    fn a_soffit_polygon_is_an_extruded_outline() {
        let l = [
            Point::new(0.0, 0.0),
            Point::new(60.0, 0.0),
            Point::new(60.0, 12.0),
            Point::new(12.0, 12.0),
            Point::new(12.0, 40.0),
            Point::new(0.0, 40.0),
        ];
        let c = Cabinet::soffit_polygon(&l, 14.0, 84.0).unwrap();
        assert_eq!(c.kind, CabinetKind::Soffit);
        assert_eq!(
            (c.width, c.depth, c.height, c.elevation),
            (60.0, 40.0, 14.0, 84.0)
        );
        let ms = meshes(&c);
        assert_eq!(ms.len(), 1);
        let (lo, hi) = ms[0].bounds().unwrap();
        assert!((lo[1] - 84.0).abs() < 1e-4 && (hi[1] - 98.0).abs() < 1e-4);
        // The L is 60*12 + 12*28 = 1056 sq in of footprint.
        assert!((crate::ring_area(&c.footprint_local()) - 1056.0).abs() < 1e-9);
        assert!(Cabinet::soffit_polygon(&l[..2], 14.0, 84.0).is_none());
        assert_eq!(auto_label_code(&c), "SO");
    }

    fn auto_label_code(c: &Cabinet) -> String {
        crate::type_code(c)
    }

    #[test]
    fn side_faces_replace_the_carcass_panel() {
        use crate::cabinet::{FaceSide, SideKind};
        let mut c = Cabinet::base(24.0);
        let plain = meshes(&c);
        // Left end: a finished slab in the door material. The plain panel
        // (x = 0..0.75) is gone and the slab stands in the left side plane.
        c.set_side_face(
            FaceSide::Left,
            SideKind::Finished,
            crate::FaceLayout::single_door(),
        );
        let finished = meshes(&c);
        assert_eq!(
            finished.len(),
            plain.len(),
            "one panel swapped for one slab"
        );
        // Open right end: its panel is not built.
        c.set_side_face(
            FaceSide::Right,
            SideKind::Open,
            crate::FaceLayout::single_door(),
        );
        assert_eq!(meshes(&c).len(), plain.len() - 1);
        // Doors on the back add front items (and a handle) behind the cabinet.
        c.set_side_face(
            FaceSide::Back,
            SideKind::CustomFace,
            crate::FaceLayout::single_door(),
        );
        let ms = meshes(&c);
        let handles = bounds_of(&ms, Material::WindowFrame);
        assert!(
            handles.iter().any(|(lo, _)| lo[2] > -1.0),
            "a back handle sits at plan y = 0: {handles:?}"
        );
        // Everything still faces outward.
        for m in &ms {
            assert!(!m.indices.is_empty());
        }
    }

    #[test]
    fn side_faces_survive_the_json_and_plain_is_stored_as_nothing() {
        use crate::cabinet::{FaceSide, SideKind};
        let mut c = Cabinet::wall(30.0);
        assert_eq!(c.side_kind(FaceSide::Left), SideKind::Plain);
        c.set_side_face(
            FaceSide::Left,
            SideKind::CustomFace,
            crate::FaceLayout::drawer_bank(2),
        );
        assert_eq!(c.sides.len(), 1);
        let back: Cabinet = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back, c);
        assert_eq!(back.side_kind(FaceSide::Left), SideKind::CustomFace);
        // An older cabinet without the field loads.
        let mut v = serde_json::to_value(&c).unwrap();
        v.as_object_mut().unwrap().remove("sides");
        let old: Cabinet = serde_json::from_value(v).unwrap();
        assert!(old.sides.is_empty());
        c.set_side_face(
            FaceSide::Left,
            SideKind::Plain,
            crate::FaceLayout::single_door(),
        );
        assert!(c.sides.is_empty());
        // The front is never a side face.
        c.set_side_face(
            FaceSide::Front,
            SideKind::Open,
            crate::FaceLayout::single_door(),
        );
        assert!(c.sides.is_empty());
        assert_eq!(c.side_width(FaceSide::Left), c.depth);
    }

    #[test]
    fn shelf_and_partition_are_single_panels() {
        assert_eq!(meshes(&Cabinet::new(CabinetKind::Shelf, 36.0)).len(), 1);
        assert_eq!(meshes(&Cabinet::new(CabinetKind::Partition, 0.75)).len(), 1);
    }

    use crate::cabinet::{BlindSide, DoorProfile, HingeStyle, Molding};
    use crate::top::{CutoutKind, EdgeProfile};

    /// Signed volume of the meshes passing `keep` (divergence theorem); positive
    /// when every triangle faces outward.
    fn volume(ms: &[Mesh], keep: impl Fn(&Mesh) -> bool) -> f64 {
        let mut sum = 0.0;
        for m in ms.iter().filter(|m| keep(m)) {
            for t in m.indices.chunks(3) {
                let p: Vec<[f64; 3]> = t
                    .iter()
                    .map(|&i| m.vertices[i as usize].position.map(f64::from))
                    .collect();
                let c = [
                    p[1][1] * p[2][2] - p[1][2] * p[2][1],
                    p[1][2] * p[2][0] - p[1][0] * p[2][2],
                    p[1][0] * p[2][1] - p[1][1] * p[2][0],
                ];
                sum += (p[0][0] * c[0] + p[0][1] * c[1] + p[0][2] * c[2]) / 6.0;
            }
        }
        sum
    }

    fn assert_outward(ms: &[Mesh], what: &str) {
        for m in ms {
            for t in m.indices.chunks(3) {
                let v: Vec<[f32; 3]> = t.iter().map(|&i| m.vertices[i as usize].position).collect();
                let n = m.vertices[t[0] as usize].normal;
                let e1 = [v[1][0] - v[0][0], v[1][1] - v[0][1], v[1][2] - v[0][2]];
                let e2 = [v[2][0] - v[0][0], v[2][1] - v[0][1], v[2][2] - v[0][2]];
                let cr = [
                    e1[1] * e2[2] - e1[2] * e2[1],
                    e1[2] * e2[0] - e1[0] * e2[2],
                    e1[0] * e2[1] - e1[1] * e2[0],
                ];
                assert!(
                    cr[0] * n[0] + cr[1] * n[1] + cr[2] * n[2] > 0.0,
                    "inward triangle in {what} ({:?})",
                    m.material
                );
            }
        }
    }

    fn counter(w: f64, d: f64) -> Cabinet {
        let ring = [
            Point::new(10.0, 20.0),
            Point::new(10.0 + w, 20.0),
            Point::new(10.0 + w, 20.0 + d),
            Point::new(10.0, 20.0 + d),
        ];
        Cabinet::custom_countertop(&ring, 1.5, 36.0).unwrap()
    }

    #[test]
    fn filler_meshes_are_panel_plus_top_and_kick() {
        assert_eq!(
            meshes(&Cabinet::filler(CabinetKind::BaseFiller, 3.0)).len(),
            3
        );
        assert_eq!(
            meshes(&Cabinet::filler(CabinetKind::WallFiller, 3.0)).len(),
            1
        );
        assert_eq!(
            meshes(&Cabinet::filler(CabinetKind::FullHeightFiller, 3.0)).len(),
            2
        );
        // The filler panel has no handle.
        assert!(meshes(&Cabinet::filler(CabinetKind::WallFiller, 3.0))
            .iter()
            .all(|m| m.material != Material::WindowFrame));
        let mut f = Cabinet::filler(CabinetKind::BaseFiller, 3.0);
        f.angle = 0.4;
        assert_outward(&meshes(&f), "filler");
    }

    #[test]
    fn door_profiles_add_frame_pieces() {
        let slab = Cabinet::wall(30.0);
        let n = meshes(&slab).len();
        let mut shaker = slab.clone();
        shaker.door_style.apply_builtin("Shaker Door");
        // Four frame pieces and the centre panel replace the single slab.
        assert_eq!(meshes(&shaker).len(), n + 4);
        let mut raised = slab.clone();
        raised.door_style.profile = DoorProfile::Raised;
        assert_eq!(meshes(&raised).len(), n + 4);
        // The panel of a raised door stands further out than a shaker's.
        let front = |c: &Cabinet| {
            meshes(c)
                .iter()
                .filter(|m| m.material == Material::WallInterior)
                .map(|m| m.bounds().unwrap().1[2])
                .fold(f32::MIN, f32::max)
        };
        assert!(front(&raised) >= front(&shaker) - 1e-4);
        // Glass doors get a glass centre, even from a slab profile.
        let mut glass = slab.clone();
        glass.door_style.glass = true;
        let ms = meshes(&glass);
        assert_eq!(ms.len(), n + 4);
        assert!(ms.iter().any(|m| m.material == Material::Glass));
        for c in [&shaker, &raised, &glass] {
            assert_outward(&meshes(c), "door");
        }
        // A drawer profile applies to drawer fronts too.
        let mut drawers = Cabinet::base(24.0);
        let base_n = meshes(&drawers).len();
        drawers.drawer_style.apply_builtin("Shaker Drawer");
        assert_eq!(meshes(&drawers).len(), base_n + 4);
    }

    #[test]
    fn hinges_handles_and_overlays() {
        let mut c = Cabinet::wall(30.0);
        let n = meshes(&c).len();
        c.door_style.hinge = HingeStyle::Exposed;
        assert_eq!(meshes(&c).len(), n + 2);
        c.door_style.hinge = HingeStyle::Hidden;
        c.door_style.handle = HandleStyle::None;
        assert_eq!(meshes(&c).len(), n - 1);
        // A centred handle moves to mid height of the door.
        c.door_style.handle = HandleStyle::Pull;
        c.door_style.handle_centered = true;
        let h = meshes(&c)
            .into_iter()
            .find(|m| m.material == Material::WindowFrame)
            .unwrap();
        let (lo, hi) = h.bounds().unwrap();
        let mid = (lo[1] + hi[1]) / 2.0;
        assert!((mid - 69.0).abs() < 1.0, "{mid}");
    }

    #[test]
    fn moldings_add_a_front_and_two_returns() {
        let mut c = Cabinet::wall(30.0);
        let n = meshes(&c).len();
        c.moldings = vec![Molding::crown(), Molding::light_rail()];
        let ms = meshes(&c);
        assert_eq!(ms.len(), n + 6);
        let trim = ms.iter().filter(|m| m.material == Material::Trim).count();
        assert_eq!(trim, 6);
        // Crown sits on top (y 84..87.5), light rail under the bottom (52..54).
        let (lo, hi) = ms
            .iter()
            .filter(|m| m.material == Material::Trim)
            .map(|m| m.bounds().unwrap())
            .fold(([f32::MAX; 3], [f32::MIN; 3]), |(mut a, mut b), (l, h)| {
                for k in 0..3 {
                    a[k] = a[k].min(l[k]);
                    b[k] = b[k].max(h[k]);
                }
                (a, b)
            });
        assert!((lo[1] - 52.0).abs() < 1e-4 && (hi[1] - 87.5).abs() < 1e-4);
        assert_outward(&ms, "moldings");
    }

    #[test]
    fn blind_cabinets_close_their_hidden_end() {
        let plain = Cabinet::base(48.0);
        let blind = Cabinet::blind_base(48.0, 15.0, BlindSide::Left);
        assert_eq!(meshes(&blind).len(), meshes(&plain).len() + 1);
        // The doors move over: the left-most front starts at the blind width.
        let min_x = |c: &Cabinet| {
            meshes(c)
                .iter()
                .filter(|m| m.material == Material::WindowFrame)
                .map(|m| m.bounds().unwrap().0[0])
                .fold(f32::MAX, f32::min)
        };
        assert!(min_x(&blind) > min_x(&plain) + 10.0);
        assert_outward(&meshes(&blind), "blind");
    }

    #[test]
    fn appliance_bays_have_sides_and_a_placeholder() {
        let dw = meshes(&Cabinet::dishwasher_opening());
        // Two sides, toe kick, countertop, appliance.
        assert_eq!(dw.len(), 5);
        assert!(dw.iter().any(|m| m.material == Material::Metal));
        assert!(dw.iter().all(|m| m.material != Material::WindowFrame));
        let range = meshes(&Cabinet::range_opening(30.0));
        assert_eq!(range.len(), 3);
        assert_outward(&dw, "dishwasher");
    }

    #[test]
    fn corner_meshes_cover_the_footprint() {
        let diag = Cabinet::corner_base(36.0);
        let pie = Cabinet::corner_base(36.0).with_pie_cut(false);
        let susan = Cabinet::corner_base(36.0).with_pie_cut(true);
        for (c, name) in [(&diag, "diagonal"), (&pie, "pie"), (&susan, "susan")] {
            let ms = meshes(c);
            assert!(!ms.is_empty());
            assert_outward(&ms, name);
            // A top at 36" and handles; everything stays within the 36" legs
            // plus the 1.5" of overhang and handles at the fronts.
            let (lo, hi) = ms.iter().filter_map(|m| m.bounds()).fold(
                ([f32::MAX; 3], [f32::MIN; 3]),
                |(mut a, mut b), (l, h)| {
                    for k in 0..3 {
                        a[k] = a[k].min(l[k]);
                        b[k] = b[k].max(h[k]);
                    }
                    (a, b)
                },
            );
            assert!(lo[0] >= -1e-4 && lo[2] >= -36.0 - 1.5, "{lo:?}");
            assert!(hi[0] <= 36.0 + 1.5 && hi[2] <= 1e-4, "{hi:?}");
            assert!((hi[1] - 36.0).abs() < 1e-4, "{hi:?}");
        }
        // The lazy susan adds two shelves and a pole.
        assert_eq!(meshes(&susan).len(), meshes(&pie).len() + 3);
        // The countertop slab has the footprint's L area times its thickness.
        let top_volume = volume(&meshes(&pie), |m| m.material == Material::Floor);
        let expected = crate::geom::area(&pie.top_local().unwrap()) * 1.5;
        assert!(
            (top_volume - expected).abs() < 1e-2,
            "{top_volume} vs {expected}"
        );
        // Wall corners hang at 54".
        let wall = meshes(&Cabinet::corner_wall(24.0));
        assert!(wall.iter().all(|m| m.bounds().unwrap().0[1] >= 54.0 - 1e-4));
        assert_outward(&wall, "wall corner");
    }

    #[test]
    fn custom_countertop_mesh_volume_matches_the_model() {
        let mut c = counter(60.0, 25.0);
        let slab = |c: &Cabinet| volume(&meshes(c), |m| m.material == Material::Floor);
        assert!((slab(&c) - 60.0 * 25.0 * 1.5).abs() < 1e-3);
        // Top face at 36", bottom at 34.5".
        let ms = meshes(&c);
        let (lo, hi) = ms[0].bounds().unwrap();
        assert!((lo[1] - 34.5).abs() < 1e-4 && (hi[1] - 36.0).abs() < 1e-4);
        assert_outward(&ms, "custom top");
        // A sink hole removes volume and adds a basin.
        c.cutouts.push(crate::top::Cutout::rect(
            CutoutKind::Sink,
            "Sink",
            Point::new(30.0, 12.5),
            30.0,
            18.0,
        ));
        c.cutouts.push(crate::top::Cutout::rect(
            CutoutKind::Cooktop,
            "Cooktop",
            Point::new(50.0, 12.5),
            8.0,
            8.0,
        ));
        let ms = meshes(&c);
        assert!((slab(&c) - c.countertop_volume()).abs() < 1e-3);
        assert!(ms.iter().any(|m| m.material == Material::Metal));
        assert!(ms.iter().any(|m| m.material == Material::Glass));
        assert_outward(&ms, "top with holes");
        // Bevel and bullnose shave the edge but keep most of the volume.
        let square = slab(&counter(60.0, 25.0));
        for edge in [EdgeProfile::Beveled, EdgeProfile::Bullnose] {
            let mut e = counter(60.0, 25.0);
            let cu = e.custom.as_mut().unwrap();
            cu.edge = edge;
            cu.edge_size = 0.75;
            let v = slab(&e);
            assert!(v < square && v > square * 0.97, "{edge:?} {v} vs {square}");
            assert_outward(&meshes(&e), "edge");
        }
        // Rotation about the position does not change the volume.
        let mut r = counter(60.0, 25.0);
        r.angle = 0.9;
        assert!((slab(&r) - square).abs() < 1e-3);
    }

    #[test]
    fn concave_custom_tops_and_backsplashes() {
        let l = [
            Point::new(0.0, 0.0),
            Point::new(60.0, 0.0),
            Point::new(60.0, 25.0),
            Point::new(25.0, 25.0),
            Point::new(25.0, 60.0),
            Point::new(0.0, 60.0),
        ];
        let c = Cabinet::custom_countertop(&l, 1.5, 36.0).unwrap();
        let v = volume(&meshes(&c), |_| true);
        assert!((v - (60.0 * 25.0 + 25.0 * 35.0) * 1.5).abs() < 1e-3, "{v}");
        let bs = Cabinet::custom_backsplash(
            &[Point::ZERO, Point::new(48.0, 0.0), Point::new(48.0, 30.0)],
            4.0,
            0.5,
            36.0,
        )
        .unwrap();
        let ms = meshes(&bs);
        assert_eq!(ms.len(), 1);
        assert_outward(&ms, "backsplash");
        let (lo, hi) = ms[0].bounds().unwrap();
        assert!((lo[1] - 36.0).abs() < 1e-4 && (hi[1] - 40.0).abs() < 1e-4);
        // Strip area of the 78" long, 0.5" thick path (mitered corner) times 4".
        assert!((volume(&ms, |_| true) - 78.0 * 0.5 * 4.0).abs() < 0.3);
    }

    #[test]
    fn part_materials_override_the_stand_ins() {
        use crate::cabinet::MaterialChoice;
        let mut c = Cabinet::base(24.0);
        c.materials.countertop = MaterialChoice::Stone;
        c.materials.door = MaterialChoice::Painted;
        let ms = meshes(&c);
        assert!(ms.iter().any(|m| m.material == Material::Stone));
        assert!(ms.iter().all(|m| m.material != Material::Floor));
        assert!(ms.iter().any(|m| m.material == Material::Trim));
    }

    #[test]
    fn soffit_is_one_solid_box() {
        let ms = meshes(&Cabinet::new(CabinetKind::Soffit, 48.0));
        assert_eq!(ms.len(), 1);
        let (lo, hi) = ms[0].bounds().unwrap();
        assert!((lo[1] - 84.0).abs() < 1e-4 && (hi[1] - 96.0).abs() < 1e-4);
    }
}
