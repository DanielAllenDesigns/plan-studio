//! Fireplaces and chimneys in 3D (CB-87): the body with its firebox
//! recess, hearth, mantel, the chimney shaft up through the ceiling and the
//! roof to the height the 3-2-10 rule asks for, with its cap and flashing.
//!
//! The specification is `plan_core::fireplace::Fireplace`; every mesh of a
//! fireplace is tagged with the id of its placed symbol. The roof planes are
//! read from the project's roof records, so the chimney finds its height
//! without the roof builder.

use crate::builder::{MeshBuilder, MeshSet};
use crate::cover::read_floor_roof;
use crate::frame::to_scene;
use crate::mesh::{Material, Mesh};
use crate::triangulate::ear_clip;
use plan_core::deck::{ccw, offset_polygon};
use plan_core::fireplace::{
    body_poly, chimney_poly, chimney_top, firebox_poly, hearth_poly, mantel_poly, wall_cuts,
    CapKind, Fireplace, FireplaceKind, Frame,
};
use plan_core::geometry::point_in_polygon;
use plan_core::{Floor, PlacedSymbol, Point, Project, Wall};
use plan_roof::RoofPlane;

/// Inches per foot, for UV scaling.
const IN_PER_FT: f64 = 12.0;

/// Adds the prism over the simple polygon `poly` between scene heights `y0`
/// and `y1`: top and bottom faces by ear clipping, one quad per side.
pub(crate) fn add_prism(b: &mut MeshBuilder, poly: &[Point], y0: f64, y1: f64) {
    if poly.len() < 3 || y1 - y0 <= 1e-9 {
        return;
    }
    let poly = ccw(poly);
    let uv = |p: Point| [(p.x / IN_PER_FT) as f32, (p.y / IN_PER_FT) as f32];
    for t in ear_clip(&poly) {
        let tri = [poly[t[0]], poly[t[1]], poly[t[2]]];
        if plan_core::geometry::polygon_area(&tri).abs() < 1e-9 {
            continue;
        }
        let uvs = tri.map(uv);
        b.tri(tri.map(|p| to_scene(p, y1)), uvs, [0.0, 1.0, 0.0]);
        b.tri(tri.map(|p| to_scene(p, y0)), uvs, [0.0, -1.0, 0.0]);
    }
    let (v0, v1) = ((y0 / IN_PER_FT) as f32, (y1 / IN_PER_FT) as f32);
    let n = poly.len();
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let len = p.dist(q);
        if len <= 1e-9 {
            continue;
        }
        let d = (q - p).normalized();
        let out = [d.y as f32, 0.0, d.x as f32];
        let quad = [
            to_scene(p, y0),
            to_scene(q, y0),
            to_scene(q, y1),
            to_scene(p, y1),
        ];
        let u = (len / IN_PER_FT) as f32;
        b.quad(quad, [[0.0, v0], [u, v0], [u, v1], [0.0, v1]], out);
    }
}

/// The surface material a name asks for, else `default`.
pub(crate) fn material_named(name: &str, default: Material) -> Material {
    let n = name.trim().to_ascii_lowercase();
    if n.is_empty() {
        return default;
    }
    if let Some(m) = Material::from_layer_name(&n) {
        return m;
    }
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if has(&["paint", "trim", "white", "plaster"]) {
        Material::Trim
    } else if has(&[
        "wood", "oak", "pine", "cedar", "walnut", "cherry", "maple", "ipe", "teak",
    ]) {
        Material::DoorPanel
    } else if has(&["metal", "steel", "iron", "copper", "aluminum"]) {
        Material::Metal
    } else if has(&["tile", "marble", "granite", "slate"]) {
        Material::Stone
    } else {
        default
    }
}

/// The roof surfaces of the whole project, for heights.
pub struct Roofs {
    planes: Vec<(Vec<Point>, RoofPlane)>,
}

impl Roofs {
    pub fn of(project: &Project) -> Self {
        let mut planes = Vec::new();
        for floor in &project.floors {
            for e in read_floor_roof(floor).planes {
                planes.push((e.plane.plan_polygon(), e.plane));
            }
        }
        Self { planes }
    }

    /// The highest roof surface above `p`, if any roof covers it.
    pub fn height(&self, p: Point) -> Option<f64> {
        self.planes
            .iter()
            .filter(|(poly, _)| point_in_polygon(p, poly))
            .filter_map(|(_, plane)| plane.height_at(p))
            .fold(None, |best: Option<f64>, h| {
                Some(best.map_or(h, |b| b.max(h)))
            })
    }
}

/// Everything a fireplace builds: its meshes and where its chimney tops out
/// (scene elevation of the top of the shaft).
pub struct FireplaceBuild {
    pub meshes: Vec<Mesh>,
    pub top: f64,
}

/// Builds the 3D parts of `fp`/`sym`, standing on `floor`.
pub fn build_fireplace(
    floor: &Floor,
    sym: &PlacedSymbol,
    fp: &Fireplace,
    roofs: &Roofs,
) -> FireplaceBuild {
    let mut set = MeshSet::default();
    let frame = Frame::of(sym);
    let base = floor.elevation + fp.elevation;
    let body_top = floor.elevation + fp.body_top(floor.ceiling_height);
    let masonry = fp.kind == FireplaceKind::Masonry;
    let body_mat = material_named(
        &fp.materials.body,
        if masonry {
            Material::Brick
        } else {
            Material::WallInterior
        },
    );
    let chimney_mat = material_named(
        &fp.materials.chimney,
        if masonry {
            Material::Brick
        } else {
            Material::Siding
        },
    );
    let top = chimney_top(fp, sym, floor.elevation, floor.ceiling_height, &|p| {
        roofs.height(p)
    });

    if fp.no_firebox {
        // A solid block: the fireplace foundation, or a fireplace without
        // its firebox. Its chimney, if it has one, rises from the top.
        add_prism(set.material(body_mat), &body_poly(sym), base, body_top);
    } else if fp.kind.has_firebox() {
        // The body, with the firebox opening cut into its front.
        let (w, d) = (sym.width, sym.depth);
        let fw = fp.firebox.width.min(w - 2.0) * 0.5;
        // The firebox is moved off the center by its Offset.
        let c = fp.firebox.offset;
        let fd = fp.firebox.depth.min(d - 1.0);
        let hearth_top = base + fp.hearth.height.max(0.0);
        let open_bottom = hearth_top + fp.firebox.raise;
        let open_top = (open_bottom + fp.firebox.height).min(body_top - 6.0);
        let b = set.material(body_mat);
        // Jambs to the sides of the opening and the mass behind it.
        add_prism(b, &frame.rect(-w * 0.5, c - fw, 0.0, d), base, body_top);
        add_prism(b, &frame.rect(c + fw, w * 0.5, 0.0, d), base, body_top);
        add_prism(b, &frame.rect(c - fw, c + fw, 0.0, d - fd), base, body_top);
        // Under and over the opening.
        let front = frame.rect(c - fw, c + fw, d - fd, d);
        add_prism(b, &front, base, open_bottom.min(body_top));
        add_prism(b, &front, open_top, body_top);
        // The firebox lining.
        let lining = material_named(
            &fp.materials.firebox,
            if masonry {
                Material::Concrete
            } else {
                Material::Metal
            },
        );
        let l = set.material(lining);
        let t = 0.75;
        let open_h = (open_bottom, open_top);
        if open_h.1 - open_h.0 > 1.0 {
            add_prism(
                l,
                &frame.rect(c - fw, c + fw, d - fd, d - fd + t),
                open_h.0,
                open_h.1,
            );
            add_prism(
                l,
                &frame.rect(c - fw, c - fw + t, d - fd + t, d),
                open_h.0,
                open_h.1,
            );
            add_prism(
                l,
                &frame.rect(c + fw - t, c + fw, d - fd + t, d),
                open_h.0,
                open_h.1,
            );
            add_prism(
                l,
                &frame.rect(c - fw + t, c + fw - t, d - fd + t, d),
                (open_h.0 - 0.5).max(base),
                open_h.0,
            );
        }
        // The hearth.
        let hearth = hearth_poly(fp, sym);
        if !hearth.is_empty() {
            let top_y = hearth_top.max(base + 0.5);
            let bottom_y = (top_y - fp.hearth.thickness).max(base);
            let m = material_named(&fp.materials.hearth, Material::Stone);
            add_prism(set.material(m), &hearth, bottom_y, top_y);
        }
        // The mantel: a shelf on the front of the body, with legs if asked.
        let shelf = mantel_poly(fp, sym);
        let shelf_top = floor.elevation + fp.mantel.height;
        if !shelf.is_empty() && shelf_top < body_top + 0.1 {
            let m = material_named(&fp.materials.mantel, Material::DoorPanel);
            let mb = set.material(m);
            add_prism(mb, &shelf, shelf_top - fp.mantel.thickness, shelf_top);
            let leg = fp.mantel.leg_width;
            if leg > 0.5 {
                let half = fp.mantel_width() * 0.5;
                let c = fp.firebox.offset;
                let deep = (fp.mantel.depth * 0.5).max(1.0);
                for (x0, x1) in [(c - half, c - half + leg), (c + half - leg, c + half)] {
                    add_prism(
                        mb,
                        &frame.rect(x0, x1, d, d + deep),
                        base,
                        shelf_top - fp.mantel.thickness,
                    );
                }
            }
        }
    } else {
        // A chimney on its own: the shaft stands from the floor.
        let cap_h = cap_height(fp);
        let b = set.material(chimney_mat);
        add_prism(b, &chimney_poly(fp, sym), base, (top - cap_h).max(body_top));
    }

    if fp.chimney.enabled && (fp.kind.has_firebox() || fp.no_firebox) {
        let shaft = chimney_poly(fp, sym);
        let shaft_top = (top - cap_height(fp)).max(body_top);
        add_prism(set.material(chimney_mat), &shaft, body_top, shaft_top);
    }
    if fp.chimney.enabled {
        cap_meshes(&mut set, fp, sym, top);
        if fp.chimney.flashing {
            flashing(&mut set, fp, sym, roofs);
        }
    }
    FireplaceBuild {
        meshes: set.finish(Some(sym.id)),
        top,
    }
}

/// Thickness of the cap slab the shaft stops under.
fn cap_height(fp: &Fireplace) -> f64 {
    match fp.chimney.cap {
        CapKind::Crown | CapKind::Stone => fp.chimney.cap_thickness.max(0.5),
        CapKind::None | CapKind::RainCap => 0.0,
    }
}

/// The cap, flue liner and rain cap at the top of the chimney.
fn cap_meshes(set: &mut MeshSet, fp: &Fireplace, sym: &PlacedSymbol, top: f64) {
    let shaft = chimney_poly(fp, sym);
    let c = &fp.chimney;
    let frame = Frame::of(sym);
    let d = if fp.kind.has_firebox() {
        c.depth.min(sym.depth)
    } else {
        sym.depth
    };
    let flue = frame.rect(
        -c.flue_width * 0.5,
        c.flue_width * 0.5,
        (d - c.flue_depth) * 0.5,
        (d + c.flue_depth) * 0.5,
    );
    let cap_mat = material_named(
        &fp.materials.cap,
        match c.cap {
            CapKind::Stone => Material::Stone,
            CapKind::RainCap => Material::Metal,
            _ => Material::Concrete,
        },
    );
    match c.cap {
        CapKind::None => {}
        CapKind::Crown | CapKind::Stone => {
            let t = c.cap_thickness.max(0.5);
            let out = offset_polygon(&shaft, -c.cap_overhang.max(0.0)).unwrap_or(shaft);
            add_prism(set.material(cap_mat), &out, top - t, top);
            add_prism(
                set.material(Material::Concrete),
                &flue,
                top,
                top + c.flue_rise.max(0.5),
            );
        }
        CapKind::RainCap => {
            let rise = c.flue_rise.max(4.0);
            let m = set.material(cap_mat);
            add_prism(m, &flue, top - 1.0, top + rise);
            let hood = offset_polygon(&flue, -c.cap_overhang.max(1.0)).unwrap_or(flue);
            add_prism(m, &hood, top + rise, top + rise + 1.0);
        }
    }
}

/// Apron flashing on each side of the shaft where it meets the roof.
fn flashing(set: &mut MeshSet, fp: &Fireplace, sym: &PlacedSymbol, roofs: &Roofs) {
    let shaft = chimney_poly(fp, sym);
    let center = shaft
        .iter()
        .fold(Point::ZERO, |acc, p| acc + *p * (1.0 / shaft.len() as f64));
    for i in 0..shaft.len() {
        let (a, b) = (shaft[i], shaft[(i + 1) % shaft.len()]);
        let mid = Point::lerp(a, b, 0.5);
        let Some(h) = roofs.height(mid + (mid - center).normalized() * 3.0) else {
            continue;
        };
        let out = (mid - center).normalized();
        let t = 0.6;
        let strip = vec![a, b, b + out * t, a + out * t];
        add_prism(set.material(Material::Metal), &strip, h - 0.5, h + 8.0);
    }
}

/// The 3D meshes of every fireplace and chimney of the project.
pub fn fireplace_meshes(project: &Project) -> Vec<Mesh> {
    if !project.floors.iter().any(|f| {
        f.symbols
            .iter()
            .any(plan_core::fireplace::is_fireplace_symbol)
    }) {
        return Vec::new();
    }
    let roofs = Roofs::of(project);
    let mut out = Vec::new();
    for floor in &project.floors {
        for (sym, fp) in floor.fireplace_symbols() {
            out.extend(build_fireplace(floor, sym, &fp, &roofs).meshes);
        }
    }
    out
}

/// The chimney tops of the project: `(symbol id, floor index, scene
/// elevation of the top)`.
pub fn chimney_tops(project: &Project) -> Vec<(plan_core::Id, usize, f64)> {
    let roofs = Roofs::of(project);
    let mut out = Vec::new();
    for (i, floor) in project.floors.iter().enumerate() {
        for (sym, fp) in floor.fireplace_symbols() {
            if fp.chimney.enabled {
                let top = chimney_top(&fp, sym, floor.elevation, floor.ceiling_height, &|p| {
                    roofs.height(p)
                });
                out.push((sym.id, i, top));
            }
        }
    }
    out
}

/// The holes in the wall `wall` of `floor` where a fireplace replaces it
/// (`s0`, `s1`, bottom and top above the floor).
pub(crate) fn wall_holes(floor: &Floor, wall: &Wall) -> Vec<crate::wall::Hole> {
    let mut out = Vec::new();
    for (sym, fp) in floor.fireplace_symbols() {
        if !fp.in_wall {
            continue;
        }
        let top = fp.body_top(floor.ceiling_height);
        for (s0, s1) in wall_cuts(&fp, sym, wall) {
            let h0 = wall.bottom_offset.max(fp.elevation);
            let h1 = (wall.bottom_offset + wall.height).min(top);
            if h1 - h0 > 1.0 {
                out.push(crate::wall::Hole {
                    s0,
                    s1,
                    h0,
                    h1,
                    niche_depth: None,
                });
            }
        }
    }
    out
}

/// The holes the chimney chases cut through the platforms of floor `fi`:
/// `(floor platform holes, ceiling platform holes)`. The shaft of a
/// fireplace on this floor goes through its ceiling; the chase of one on a
/// floor below goes through both platforms.
pub(crate) fn chase_holes(project: &Project, fi: usize) -> (Vec<Vec<Point>>, Vec<Vec<Point>>) {
    let mut floor_holes = Vec::new();
    let mut ceiling_holes = Vec::new();
    if let Some(floor) = project.floors.get(fi) {
        for (sym, fp) in floor.fireplace_symbols() {
            if fp.chimney.enabled {
                ceiling_holes.push(chimney_poly(&fp, sym));
            }
        }
    }
    for (_, poly) in plan_core::fireplace::chases_on_floor(project, fi) {
        floor_holes.push(poly.clone());
        ceiling_holes.push(poly);
    }
    (floor_holes, ceiling_holes)
}

/// Plan outline of the body of every fireplace of `floor` (tests and the
/// picking code use it).
pub fn body_outlines(floor: &Floor) -> Vec<(plan_core::Id, Vec<Point>)> {
    floor
        .fireplace_symbols()
        .into_iter()
        .map(|(s, _)| (s.id, body_poly(s)))
        .collect()
}

/// Plan outline of the firebox opening of every fireplace of `floor`.
pub fn firebox_outlines(floor: &Floor) -> Vec<(plan_core::Id, Vec<Point>)> {
    floor
        .fireplace_symbols()
        .into_iter()
        .filter(|(_, fp)| fp.kind.has_firebox())
        .map(|(s, fp)| (s.id, firebox_poly(&fp, s)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::fireplace::ChimneyTop;

    fn volume(m: &Mesh) -> f64 {
        let p = |i: u32| m.vertices[i as usize].position.map(f64::from);
        m.indices
            .chunks(3)
            .map(|t| {
                let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                let cr = [
                    b[1] * c[2] - b[2] * c[1],
                    b[2] * c[0] - b[0] * c[2],
                    b[0] * c[1] - b[1] * c[0],
                ];
                (a[0] * cr[0] + a[1] * cr[1] + a[2] * cr[2]) / 6.0
            })
            .sum::<f64>()
            .abs()
    }

    fn project(kind: FireplaceKind) -> (Project, plan_core::Id) {
        let mut p = Project::new("fp");
        let id = p.add_fireplace(0, kind, Point::new(100.0, 50.0), 0.0);
        (p, id)
    }

    fn bounds(meshes: &[Mesh]) -> ([f32; 3], [f32; 3]) {
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for m in meshes {
            let (a, b) = m.bounds().unwrap();
            for k in 0..3 {
                lo[k] = lo[k].min(a[k]);
                hi[k] = hi[k].max(b[k]);
            }
        }
        (lo, hi)
    }

    #[test]
    fn a_masonry_fireplace_has_a_body_a_hearth_a_mantel_and_a_chimney() {
        let (p, id) = project(FireplaceKind::Masonry);
        let meshes = fireplace_meshes(&p);
        assert!(meshes.iter().all(|m| m.object_id == Some(id)));
        let mats: Vec<Material> = meshes.iter().map(|m| m.material).collect();
        for m in [
            Material::Brick,
            Material::Stone,
            Material::DoorPanel,
            Material::Concrete,
        ] {
            assert!(mats.contains(&m), "{m:?} in {mats:?}");
        }
        // With no roof the chimney ends 3 ft over the ceiling (109 1/8 + 36)
        // plus the cap and flue.
        let (lo, hi) = bounds(&meshes);
        assert!(lo[1].abs() < 1e-3);
        assert!(
            (f64::from(hi[1]) - (109.125 + 36.0 + 8.0)).abs() < 1e-3,
            "{hi:?}"
        );
        // The hearth reaches 16" in front of the body (plan y 50 + 24 + 16).
        let z_front = -(50.0 + 24.0 + 16.0);
        assert!((f64::from(lo[2]) - z_front).abs() < 1e-3, "{lo:?}");
    }

    #[test]
    fn the_firebox_is_a_recess_in_the_front_of_the_body() {
        let (p, id) = project(FireplaceKind::Masonry);
        let floor = &p.floors[0];
        let sym = floor.symbol(id).unwrap();
        let fp = floor.fireplace_of(sym);
        let roofs = Roofs::of(&p);
        let built = build_fireplace(floor, sym, &fp, &roofs);
        // Brick volume: the body less the 36 x 18 x 30 recess, plus the
        // shaft above the body (nothing: the body reaches the ceiling and
        // the shaft stands on it).
        let brick: f64 = built
            .meshes
            .iter()
            .filter(|m| m.material == Material::Brick)
            .map(volume)
            .sum();
        let body = 72.0 * 24.0 * 109.125;
        let recess = 36.0 * 18.0 * 30.0;
        let shaft = 48.0 * 24.0 * (109.125 + 36.0 - 4.0 - 109.125);
        assert!(
            (brick - (body - recess + shaft)).abs() < 8.0,
            "{brick} vs {}",
            body - recess + shaft
        );
    }

    #[test]
    fn the_chimney_follows_the_roof() {
        let (mut p, id) = project(FireplaceKind::Masonry);
        // A flat roof plane at 140" over the fireplace.
        p.floors[0].roofs.push(serde_json::json!({
            "kind": "plane",
            "id": 99,
            "polygon3d": [[0.0,140.0,0.0],[300.0,140.0,0.0],[300.0,140.0,-200.0],[0.0,140.0,-200.0]],
            "baseline": [{"x":0.0,"y":0.0},{"x":300.0,"y":0.0}],
            "pitch": 0.0,
        }));
        let tops = chimney_tops(&p);
        assert_eq!(tops.len(), 1);
        assert_eq!(tops[0].0, id);
        assert!((tops[0].2 - 176.0).abs() < 1e-6, "{tops:?}");
        let (lo, hi) = bounds(&fireplace_meshes(&p));
        let _ = lo;
        assert!((f64::from(hi[1]) - (176.0 + 8.0)).abs() < 1e-3);
        // Flashing: metal around the shaft at the roof.
        let m = fireplace_meshes(&p);
        assert!(m.iter().any(|x| x.material == Material::Metal));
    }

    #[test]
    fn a_chimney_on_its_own_is_one_shaft_from_the_floor() {
        let (p, _) = project(FireplaceKind::ChimneyOnly);
        let meshes = fireplace_meshes(&p);
        let (lo, hi) = bounds(&meshes);
        assert!(lo[1].abs() < 1e-3);
        assert!(hi[1] > 140.0);
        // No hearth stone, no mantel wood.
        assert!(!meshes.iter().any(|m| m.material == Material::DoorPanel));
        assert!(!meshes.iter().any(|m| m.material == Material::Stone));
    }

    #[test]
    fn a_fixed_height_ends_the_chimney_there_and_a_rain_cap_is_metal() {
        let (mut p, id) = project(FireplaceKind::Prefab);
        let mut fp = p.floors[0].fireplace_of(p.floors[0].symbol(id).unwrap());
        fp.chimney.top = ChimneyTop::Height(200.0);
        p.floors[0].set_fireplace(fp);
        let meshes = fireplace_meshes(&p);
        let (_, hi) = bounds(&meshes);
        // 200 plus the flue pipe above the top.
        assert!(
            f64::from(hi[1]) > 200.0 && f64::from(hi[1]) < 215.0,
            "{hi:?}"
        );
        assert!(meshes.iter().any(|m| m.material == Material::Metal));
    }

    #[test]
    fn an_in_wall_fireplace_cuts_a_hole_in_the_wall() {
        let mut p = Project::new("w");
        let id = p.add_fireplace(0, FireplaceKind::Masonry, Point::new(100.0, -10.0), 0.0);
        let mut fp = p.floors[0].fireplace_of(p.floors[0].symbol(id).unwrap());
        fp.in_wall = true;
        p.floors[0].set_fireplace(fp);
        let wall = Wall::new(
            Point::new(0.0, 0.0),
            Point::new(300.0, 0.0),
            6.5,
            109.125,
            plan_core::WallKind::Exterior,
        );
        let holes = wall_holes(&p.floors[0], &wall);
        assert_eq!(holes.len(), 1);
        assert!((holes[0].s0 - 64.0).abs() < 1e-6 && (holes[0].s1 - 136.0).abs() < 1e-6);
        assert_eq!((holes[0].h0, holes[0].h1), (0.0, 109.125));
        // A fireplace beside the wall leaves it whole.
        let q = Project::new("q");
        assert!(wall_holes(&q.floors[0], &wall).is_empty());
    }

    #[test]
    fn chases_cut_the_platforms_of_the_floors_above() {
        let mut p = Project::new("c");
        p.floors.push(Floor::new("2nd Floor", 120.0));
        p.add_fireplace(0, FireplaceKind::Masonry, Point::new(0.0, 0.0), 0.0);
        let (f0, c0) = chase_holes(&p, 0);
        assert!(f0.is_empty());
        assert_eq!(c0.len(), 1);
        let (f1, c1) = chase_holes(&p, 1);
        assert_eq!((f1.len(), c1.len()), (1, 1));
    }

    #[test]
    fn material_names_map_to_surfaces() {
        assert_eq!(material_named("", Material::Brick), Material::Brick);
        assert_eq!(
            material_named("Stone Veneer", Material::Brick),
            Material::Stone
        );
        assert_eq!(
            material_named("Painted Wood", Material::Brick),
            Material::Trim
        );
        assert_eq!(
            material_named("Walnut", Material::Brick),
            Material::DoorPanel
        );
        assert_eq!(material_named("Steel", Material::Brick), Material::Metal);
        assert_eq!(material_named("???", Material::Floor), Material::Floor);
    }

    #[test]
    fn default_records_build_for_symbols_without_one() {
        let (mut p, id) = project(FireplaceKind::Masonry);
        p.floors[0].fireplaces.clear();
        let meshes = fireplace_meshes(&p);
        assert!(!meshes.is_empty());
        assert!(meshes.iter().all(|m| m.object_id == Some(id)));
    }
}
