//! 3D meshes of Chief library objects, ready for Plan Studio's 3D view and
//! ray tracer.
//!
//! [`object_meshes`] decodes the geometry of one object: every
//! `CD AB 74 00` triangle mesh (in `SymbolData` and the `AssociatedData` tail),
//! or, when there is none, the `symDxf` polygon faces fan-triangulated.
//! [`placed_symbol_meshes`] then fits it to a [`PlacedSymbol`] and moves it into
//! scene space as [`plan_3d::Mesh`]es. [`MeshCache`] keeps decoded objects.
//!
//! # Frames
//!
//! Chief stores X right, Y towards the back, Z up, with the object's front
//! facing -Y. [`Triangles`] positions are already mapped to Plan Studio's scene
//! axes with `(x, y, z) -> (x, z, -y)` (a proper rotation: winding is kept), so
//! the object's front faces +Z. See [`plan_3d::import`] for how that frame
//! becomes a placed symbol.
//!
//! # Colors
//!
//! None found. The 80-byte triangle record is `a b c`, a partner id, two ids
//! and (on some records) three pairs of f64 that look like per-corner texture
//! coordinates; there is no RGB. Objects carry only *named* material groups
//! (`FRAME`, `LEGS`, `UPHOLSTERY`, ...) in the `AssociatedData` tail, with no
//! per-triangle assignment decoded and no color attached. [`Triangles::color`]
//! is therefore `None`, and such parts render with [`DEFAULT_MATERIAL`]
//! (mid-gray). A future decoder can fill `color` without any API change.

use crate::catalog::ChiefCatalog;
use crate::decode::{
    parse_face_stream, parse_triangle_meshes, split_associated, Mesh, ObjectBlobs,
};
use crate::error::Result;
use plan_3d::import::{fit_meshes_to_box, mesh_from_triangles, transform_mesh};
use plan_3d::Material;
use plan_core::PlacedSymbol;
use std::collections::VecDeque;
use std::sync::Arc;

/// Material used for parts without a color: a neutral mid-gray.
pub const DEFAULT_MATERIAL: Material = Material::Concrete;

/// One decoded part of an object: an indexed triangle soup in the natural
/// import frame (inches, X right, Y up, front towards +Z).
#[derive(Debug, Clone, PartialEq)]
pub struct Triangles {
    /// Vertex positions.
    pub positions: Vec<[f32; 3]>,
    /// Per-vertex normals when the source has them. Chief meshes do not, so
    /// this is `None` and flat normals are computed on import.
    pub normals: Option<Vec<[f32; 3]>>,
    /// Triangle vertex indices, three per triangle, counter-clockwise from
    /// outside.
    pub indices: Vec<u32>,
    /// sRGB color of the part, when known (currently never).
    pub color: Option<[u8; 3]>,
}

impl Triangles {
    /// Number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

/// Maps a decoded part to a built-in [`Material`]: the nearest opaque
/// material color, or [`DEFAULT_MATERIAL`] without a color.
pub fn material_for(color: Option<[u8; 3]>) -> Material {
    let Some(c) = color else {
        return DEFAULT_MATERIAL;
    };
    let lin = |v: u8| {
        let s = v as f32 / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    let want = [lin(c[0]), lin(c[1]), lin(c[2])];
    let dist = |m: &Material| {
        let k = m.color();
        (0..3).map(|i| (k[i] - want[i]).powi(2)).sum::<f32>()
    };
    Material::ALL
        .iter()
        .filter(|m| {
            m.color()[3] >= 0.999
                && !matches!(
                    m,
                    Material::Ceiling | Material::WallExterior | Material::WallInterior
                )
        })
        .min_by(|a, b| dist(a).total_cmp(&dist(b)))
        .copied()
        .unwrap_or(DEFAULT_MATERIAL)
}

/// Chief (x, y, z) to the natural import frame.
fn to_scene(p: [f64; 3]) -> [f32; 3] {
    [p[0] as f32, p[2] as f32, -p[1] as f32]
}

fn from_chief_mesh(m: &Mesh) -> Option<Triangles> {
    if m.triangles.is_empty() {
        return None;
    }
    let mut t = Triangles {
        positions: m.vertices.iter().copied().map(to_scene).collect(),
        normals: None,
        indices: m.triangles.iter().flatten().copied().collect(),
        color: None,
    };
    trim_outliers(&mut t);
    if t.indices.is_empty() {
        return None;
    }
    orient_outward(&mut t);
    Some(t)
}

/// Drops triangles that have a vertex far outside the bulk of the mesh.
///
/// The `symDxf` face scanner occasionally accepts a garbage "face" whose
/// coordinates are finite but absurd (one real sofa had a vertex at x =
/// -57088 in), which would shrink the whole object when fitted. The bulk is the
/// 5th to 95th percentile range per axis; anything more than twice that span
/// (plus a foot) beyond it goes. Clean meshes are never touched.
fn trim_outliers(t: &mut Triangles) {
    let mut used: Vec<u32> = t.indices.clone();
    used.sort_unstable();
    used.dedup();
    if used.len() < 8 {
        return;
    }
    let mut ok = [(f32::MIN, f32::MAX); 3];
    for (k, range) in ok.iter_mut().enumerate() {
        let mut c: Vec<f32> = used.iter().map(|&i| t.positions[i as usize][k]).collect();
        c.sort_unstable_by(f32::total_cmp);
        let lo = c[c.len() / 20];
        let hi = c[c.len() - 1 - c.len() / 20];
        let slack = 2.0 * (hi - lo) + 12.0;
        *range = (lo - slack, hi + slack);
    }
    let inside = |i: u32| {
        let p = t.positions[i as usize];
        (0..3).all(|k| p[k] >= ok[k].0 && p[k] <= ok[k].1)
    };
    if used.iter().all(|&i| inside(i)) {
        return;
    }
    let kept: Vec<u32> = t
        .indices
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|tri| tri.iter().all(|&i| inside(i)))
        .flatten()
        .copied()
        .collect();
    t.indices = kept;
}

/// Flips the winding when the mesh is inside-out: the signed volume about the
/// vertex centroid is negative for faces that wind clockwise from outside.
fn orient_outward(t: &mut Triangles) {
    if t.positions.is_empty() {
        return;
    }
    let n = t.positions.len() as f64;
    let c = t.positions.iter().fold([0.0f64; 3], |mut a, p| {
        for k in 0..3 {
            a[k] += p[k] as f64 / n;
        }
        a
    });
    let mut vol = 0.0f64;
    for tri in t.indices.as_chunks::<3>().0 {
        let q = |i: u32| {
            let p = t.positions[i as usize];
            [p[0] as f64 - c[0], p[1] as f64 - c[1], p[2] as f64 - c[2]]
        };
        let (a, b, d) = (q(tri[0]), q(tri[1]), q(tri[2]));
        vol += a[0] * (b[1] * d[2] - b[2] * d[1]) - a[1] * (b[0] * d[2] - b[2] * d[0])
            + a[2] * (b[0] * d[1] - b[1] * d[0]);
    }
    if vol < 0.0 {
        for tri in t.indices.as_chunks_mut::<3>().0 {
            tri.swap(1, 2);
        }
    }
}

/// Width (X), depth (Z) and height (Y) of the combined parts, inches; zeros
/// when there is no geometry. Compare it with the library size before placing:
/// objects with undecoded record kinds give partial meshes, and fitting those
/// to the full size stretches them.
pub fn parts_extent(parts: &[Triangles]) -> [f32; 3] {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for t in parts {
        for &i in &t.indices {
            if let Some(p) = t.positions.get(i as usize) {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
        }
    }
    if lo[0] > hi[0] {
        return [0.0; 3];
    }
    [hi[0] - lo[0], hi[2] - lo[2], hi[1] - lo[1]]
}

/// Decodes the geometry in already-loaded blobs (see [`object_meshes`]).
pub fn triangles_from_blobs(blobs: &ObjectBlobs) -> Vec<Triangles> {
    let mut meshes: Vec<Mesh> = Vec::new();
    if let Some(sd) = blobs.symbol_data.as_deref() {
        meshes.extend(parse_triangle_meshes(sd));
    }
    if let Some(a) = blobs.associated.as_deref() {
        meshes.extend(parse_triangle_meshes(split_associated(a).1));
    }
    if meshes.is_empty() {
        if let Some(fs) = blobs.sym_dxf.as_deref().and_then(parse_face_stream) {
            meshes.push(fs.mesh);
        }
    }
    meshes.iter().filter_map(from_chief_mesh).collect()
}

/// All geometry of one library object as scene-frame triangle parts.
///
/// Reads the object's blobs (up to a few MB), decodes every triangle mesh and
/// falls back to the `symDxf` faces when there are none. The result is empty
/// for objects without decodable 3D geometry (for example plants and some
/// parametric objects).
pub fn object_meshes(cat: &ChiefCatalog, library_object_id: i64) -> Result<Vec<Triangles>> {
    Ok(triangles_from_blobs(&cat.object_blobs(library_object_id)?))
}

/// Fits decoded parts to a placed symbol and moves them into scene space.
///
/// The parts are scaled per axis to `placed.width` x `placed.height` x
/// `placed.depth` as one object, anchored at the symbol's back-center, then
/// rotated by `placed.angle`, mirrored when `placed.flip` is set and moved to
/// `(position.x, floor_elevation + placed.elevation, -position.y)`. Meshes are
/// tagged with `placed.id`. A symbol at angle 0 faces plan +Y, like its 2D
/// footprint.
pub fn place_triangles(
    parts: &[Triangles],
    placed: &PlacedSymbol,
    floor_elevation: f64,
) -> Vec<plan_3d::Mesh> {
    let meshes: Vec<plan_3d::Mesh> = parts
        .iter()
        .map(|t| {
            mesh_from_triangles(
                &t.positions,
                t.normals.as_deref(),
                &t.indices,
                material_for(t.color),
                Some(placed.id),
            )
        })
        .filter(|m| !m.indices.is_empty())
        .collect();
    let fitted = fit_meshes_to_box(
        &meshes,
        placed.width as f32,
        placed.depth as f32,
        placed.height as f32,
    );
    let origin = [
        placed.position.x as f32,
        (floor_elevation + placed.elevation) as f32,
        -placed.position.y as f32,
    ];
    // Natural frame faces +Z; a symbol at angle 0 faces scene -Z (plan +Y).
    let yaw = (placed.angle + 180.0).to_radians() as f32;
    fitted
        .iter()
        .map(|m| transform_mesh(m, origin, yaw, [1.0; 3], placed.flip))
        .collect()
}

/// Decodes `library_object_id` and places it (see [`place_triangles`]). Gives
/// no meshes when the object has no geometry or cannot be read; use
/// [`MeshCache`] to avoid decoding the same object repeatedly.
pub fn placed_symbol_meshes(
    cat: &ChiefCatalog,
    library_object_id: i64,
    placed: &PlacedSymbol,
    floor_elevation: f64,
) -> Vec<plan_3d::Mesh> {
    match object_meshes(cat, library_object_id) {
        Ok(parts) => place_triangles(&parts, placed, floor_elevation),
        Err(_) => Vec::new(),
    }
}

/// Default capacity of [`MeshCache`], in objects.
pub const DEFAULT_CACHE_OBJECTS: usize = 64;

type Key = (String, i64);

/// A small least-recently-used cache of decoded objects, keyed by
/// (catalog uuid, library object id). Objects with no geometry are cached too.
#[derive(Debug)]
pub struct MeshCache {
    capacity: usize,
    /// Least recently used first.
    entries: VecDeque<(Key, Arc<Vec<Triangles>>)>,
}

impl Default for MeshCache {
    fn default() -> Self {
        Self::new(DEFAULT_CACHE_OBJECTS)
    }
}

impl MeshCache {
    /// A cache holding up to `capacity` objects (at least one).
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: VecDeque::new(),
        }
    }

    /// Number of cached objects.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is cached.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Drops everything.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// The cached parts of an object, marking them most recently used.
    pub fn get(&mut self, catalog_id: &str, library_object_id: i64) -> Option<Arc<Vec<Triangles>>> {
        let pos = self
            .entries
            .iter()
            .position(|(k, _)| k.1 == library_object_id && k.0 == catalog_id)?;
        let entry = self.entries.remove(pos)?;
        let parts = entry.1.clone();
        self.entries.push_back(entry);
        Some(parts)
    }

    /// Stores `parts` as the most recently used entry, evicting the oldest
    /// when full.
    pub fn insert(
        &mut self,
        catalog_id: &str,
        library_object_id: i64,
        parts: Vec<Triangles>,
    ) -> Arc<Vec<Triangles>> {
        let parts = Arc::new(parts);
        self.entries
            .retain(|(k, _)| !(k.1 == library_object_id && k.0 == catalog_id));
        while self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries
            .push_back(((catalog_id.to_owned(), library_object_id), parts.clone()));
        parts
    }

    /// Cached or freshly decoded parts of an object.
    pub fn get_or_decode(
        &mut self,
        cat: &ChiefCatalog,
        library_object_id: i64,
    ) -> Result<Arc<Vec<Triangles>>> {
        if let Some(hit) = self.get(cat.id(), library_object_id) {
            return Ok(hit);
        }
        let parts = object_meshes(cat, library_object_id)?;
        Ok(self.insert(cat.id(), library_object_id, parts))
    }

    /// [`placed_symbol_meshes`] through the cache.
    pub fn placed_symbol_meshes(
        &mut self,
        cat: &ChiefCatalog,
        library_object_id: i64,
        placed: &PlacedSymbol,
        floor_elevation: f64,
    ) -> Vec<plan_3d::Mesh> {
        match self.get_or_decode(cat, library_object_id) {
            Ok(parts) => place_triangles(&parts, placed, floor_elevation),
            Err(_) => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::testdata::{box_mesh, face_stream, tri_mesh};
    use plan_core::geometry::Point;

    fn blobs_with_box(w: f64, d: f64, h: f64) -> ObjectBlobs {
        let (v, t) = box_mesh(w, d, h);
        let mut assoc = vec![0u8; 12];
        assoc.extend_from_slice(&2u32.to_le_bytes());
        assoc.extend_from_slice(b"{}");
        assoc.extend(tri_mesh(&v, &t));
        ObjectBlobs {
            associated: Some(assoc),
            ..Default::default()
        }
    }

    fn bounds(ms: &[plan_3d::Mesh]) -> ([f32; 3], [f32; 3]) {
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for m in ms {
            let (l, h) = m.bounds().unwrap();
            for k in 0..3 {
                lo[k] = lo[k].min(l[k]);
                hi[k] = hi[k].max(h[k]);
            }
        }
        (lo, hi)
    }

    fn flat(t: &Triangles) -> plan_3d::Mesh {
        mesh_from_triangles(&t.positions, None, &t.indices, DEFAULT_MATERIAL, None)
    }

    #[test]
    fn decodes_triangle_records_into_the_scene_frame() {
        let parts = triangles_from_blobs(&blobs_with_box(4.0, 3.0, 2.0));
        assert_eq!(parts.len(), 1);
        let p = &parts[0];
        assert_eq!(p.triangle_count(), 12);
        assert!(p.color.is_none() && p.normals.is_none());
        assert_eq!(parts_extent(&parts), [4.0, 3.0, 2.0]);
        assert_eq!(parts_extent(&[]), [0.0; 3]);
        let mesh = flat(p);
        let (lo, hi) = mesh.bounds().unwrap();
        // x 0..4, up 0..2, Z = -chief y: -3..0.
        assert_eq!((lo, hi), ([0.0, 0.0, -3.0], [4.0, 2.0, 0.0]));
        // Faces point away from the centroid after orientation.
        let c = [2.0, 1.0, -1.5];
        for tri in mesh.indices.as_chunks::<3>().0 {
            let v = mesh.vertices[tri[0] as usize];
            let d: f32 = (0..3).map(|a| (v.position[a] - c[a]) * v.normal[a]).sum();
            assert!(d > 0.0, "{d}");
        }
    }

    #[test]
    fn garbage_far_faces_are_trimmed() {
        let quad = |z: f64, x: f64| {
            [
                [x, 0.0, z],
                [x + 10.0, 0.0, z],
                [x + 10.0, 5.0, z],
                [x, 5.0, z],
            ]
        };
        let mut quads: Vec<_> = (0..20).map(|i| quad(i as f64, 0.0)).collect();
        let clean = triangles_from_blobs(&ObjectBlobs {
            sym_dxf: Some(face_stream(&quads)),
            ..Default::default()
        });
        quads.push(quad(3.0, -57088.0));
        let dirty = triangles_from_blobs(&ObjectBlobs {
            sym_dxf: Some(face_stream(&quads)),
            ..Default::default()
        });
        assert_eq!(clean[0].triangle_count(), 40);
        assert_eq!(dirty[0].triangle_count(), 40);
        assert_eq!(parts_extent(&dirty), parts_extent(&clean));
    }

    #[test]
    fn inside_out_meshes_are_flipped() {
        let (v, t) = box_mesh(2.0, 2.0, 2.0);
        let inverted: Vec<[u32; 3]> = t.iter().map(|a| [a[0], a[2], a[1]]).collect();
        let decode = |tris: &[[u32; 3]]| {
            let blobs = ObjectBlobs {
                symbol_data: Some(tri_mesh(&v, tris)),
                ..Default::default()
            };
            let m = flat(&triangles_from_blobs(&blobs)[0]);
            m.vertices.iter().map(|v| v.normal[1]).sum::<f32>()
        };
        // Both orientations end up with the same winding.
        assert!((decode(&inverted) - decode(&t)).abs() < 1e-4);
    }

    #[test]
    fn falls_back_to_face_stream() {
        let quad = |z: f64| [[0.0, 0.0, z], [10.0, 0.0, z], [10.0, 5.0, z], [0.0, 5.0, z]];
        let faces = face_stream(&[quad(0.0), quad(8.0)]);
        let blobs = ObjectBlobs {
            sym_dxf: Some(faces.clone()),
            ..Default::default()
        };
        let parts = triangles_from_blobs(&blobs);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].triangle_count(), 4);
        // Triangle records win over the face stream.
        let mut both = blobs_with_box(1.0, 1.0, 1.0);
        both.sym_dxf = Some(faces);
        assert_eq!(triangles_from_blobs(&both)[0].triangle_count(), 12);
        assert!(triangles_from_blobs(&ObjectBlobs::default()).is_empty());
    }

    fn placed(angle: f64, flip: bool) -> PlacedSymbol {
        let mut s = PlacedSymbol::new("x", Point::new(100.0, 50.0), 40.0, 20.0, 30.0);
        s.angle = angle;
        s.flip = flip;
        s.elevation = 5.0;
        s.id = 9;
        s
    }

    #[test]
    fn placement_matches_the_footprint() {
        let parts = triangles_from_blobs(&blobs_with_box(4.0, 3.0, 2.0));
        for angle in [0.0, 90.0, 37.0, 180.0] {
            for flip in [false, true] {
                let s = placed(angle, flip);
                let ms = place_triangles(&parts, &s, 96.0);
                assert!(ms.iter().all(|m| m.object_id == Some(9)));
                let (lo, hi) = bounds(&ms);
                // Vertical: floor + elevation .. + height.
                assert!((lo[1] - 101.0).abs() < 1e-3 && (hi[1] - 131.0).abs() < 1e-3);
                // Plan footprint corners -> scene (x, -y).
                let (mut flo, mut fhi) = ([f32::MAX; 2], [f32::MIN; 2]);
                for p in s.footprint() {
                    for (k, c) in [p.x as f32, -p.y as f32].into_iter().enumerate() {
                        flo[k] = flo[k].min(c);
                        fhi[k] = fhi[k].max(c);
                    }
                }
                assert!((lo[0] - flo[0]).abs() < 1e-2 && (hi[0] - fhi[0]).abs() < 1e-2);
                assert!((lo[2] - flo[1]).abs() < 1e-2 && (hi[2] - fhi[1]).abs() < 1e-2);
            }
        }
    }

    #[test]
    fn front_faces_plan_plus_y_at_angle_zero() {
        // Chief box (front = min y) with a fin sticking out of the front.
        let (mut v, mut t) = box_mesh(4.0, 3.0, 2.0);
        let base = v.len() as u32;
        v.extend([[0.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, -1.0, 2.0]]);
        t.push([base, base + 1, base + 2]);
        let blobs = ObjectBlobs {
            symbol_data: Some(tri_mesh(&v, &t)),
            ..Default::default()
        };
        let parts = triangles_from_blobs(&blobs);
        let s = PlacedSymbol::new("x", Point::new(0.0, 0.0), 4.0, 4.0, 2.0);
        let ms = place_triangles(&parts, &s, 0.0);
        let (lo, hi) = bounds(&ms);
        // Depth 0..4 along plan +Y = scene -Z; the fin is the front-most point.
        assert!(
            (lo[2] + 4.0).abs() < 1e-3 && hi[2].abs() < 1e-3,
            "{lo:?} {hi:?}"
        );
        let fin_z = ms[0]
            .vertices
            .iter()
            .map(|v| v.position[2])
            .fold(f32::MAX, f32::min);
        assert!((fin_z + 4.0).abs() < 1e-3);
    }

    #[test]
    fn colors_map_to_materials() {
        assert_eq!(material_for(None), DEFAULT_MATERIAL);
        assert_eq!(material_for(Some([200, 70, 50])), Material::Brick);
        assert_ne!(material_for(Some([250, 250, 250])), Material::Glass);
    }

    fn tri_part(n: usize) -> Vec<Triangles> {
        vec![Triangles {
            positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: None,
            indices: [0, 1, 2].repeat(n),
            color: None,
        }]
    }

    #[test]
    fn cache_evicts_least_recently_used() {
        let mut c = MeshCache::new(2);
        assert!(c.is_empty());
        c.insert("a", 1, tri_part(1));
        c.insert("a", 2, tri_part(2));
        assert!(c.get("a", 1).is_some()); // 1 is now newest
        c.insert("b", 1, tri_part(3)); // evicts ("a", 2)
        assert_eq!(c.len(), 2);
        assert!(c.get("a", 2).is_none());
        assert_eq!(c.get("a", 1).unwrap()[0].triangle_count(), 1);
        assert_eq!(c.get("b", 1).unwrap()[0].triangle_count(), 3);
        // Re-inserting replaces rather than duplicating.
        c.insert("b", 1, tri_part(4));
        assert_eq!(c.len(), 2);
        assert_eq!(c.get("b", 1).unwrap()[0].triangle_count(), 4);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(MeshCache::new(0).capacity, 1);
    }
}
