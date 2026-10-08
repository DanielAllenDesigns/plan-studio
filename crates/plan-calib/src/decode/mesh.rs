//! Geometry containers and the two binary mesh layouts found in Chief blobs.
//!
//! * `LibrarySymbolData.symDxf` is **not** DXF. It is a serialized list of 3D
//!   polygon faces (see [`parse_face_stream`]).
//! * `AssociatedData` and `SymbolData4LibraryObjects` carry `CD AB 74 00`
//!   records: a vertex array followed by 80-byte triangle records (see
//!   [`parse_triangle_meshes`]).
//!
//! Both are in inches, X to the right, Y towards the *back* of the object
//! (the front faces -Y), Z up.

/// An indexed triangle soup.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    /// Vertex positions (x, y, z) in inches.
    pub vertices: Vec<[f64; 3]>,
    /// Triangles as vertex indices.
    pub triangles: Vec<[u32; 3]>,
}

/// Axis-aligned 3D bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds3 {
    /// Smallest x, y, z.
    pub min: [f64; 3],
    /// Largest x, y, z.
    pub max: [f64; 3],
}

impl Bounds3 {
    /// Extent along x, y and z.
    pub fn extent(&self) -> [f64; 3] {
        [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ]
    }

    fn grow(&mut self, p: [f64; 3]) {
        for (i, c) in p.iter().enumerate() {
            self.min[i] = self.min[i].min(*c);
            self.max[i] = self.max[i].max(*c);
        }
    }
}

impl Mesh {
    /// Bounds of the vertices referenced by at least one triangle.
    pub fn bounds(&self) -> Option<Bounds3> {
        let mut b: Option<Bounds3> = None;
        for t in &self.triangles {
            for &i in t {
                let p = *self.vertices.get(i as usize)?;
                match &mut b {
                    Some(b) => b.grow(p),
                    None => b = Some(Bounds3 { min: p, max: p }),
                }
            }
        }
        b
    }
}

/// Union of the bounds of several meshes.
pub fn union_bounds(meshes: &[Mesh]) -> Option<Bounds3> {
    let mut out: Option<Bounds3> = None;
    for m in meshes {
        if let Some(b) = m.bounds() {
            match &mut out {
                Some(o) => {
                    o.grow(b.min);
                    o.grow(b.max);
                }
                None => out = Some(b),
            }
        }
    }
    out
}

/// The result of decoding a `symDxf` blob.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FaceStream {
    /// All polygon faces, fan-triangulated.
    pub mesh: Mesh,
    /// Number of polygon faces found.
    pub faces: usize,
    /// Face count the header declares, when the header has one.
    pub declared_faces: Option<u32>,
    /// Layer / material names found between faces (for example
    /// `Adjust Light` or `A-FIXT-MAIN-0`).
    pub layers: Vec<String>,
}

fn u16_at(b: &[u8], p: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(p..p + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], p: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(p..p + 4)?.try_into().ok()?))
}

fn f64_at(b: &[u8], p: usize) -> Option<f64> {
    Some(f64::from_le_bytes(b.get(p..p + 8)?.try_into().ok()?))
}

/// Largest coordinate magnitude accepted (inches). Real objects are far smaller.
const MAX_COORD: f64 = 1.0e5;

/// Decodes a `LibrarySymbolData.symDxf` blob.
///
/// Layout (little endian), verified on 541 Core Interiors and Exteriors blobs:
///
/// ```text
/// 0   u16  stream version (0x092b, 0x05bf, 0x0601, ...)
/// 2   u32  0xFFFFFFFF            (newer streams also carry a face count at 6)
/// ... faces, each:
///       u16 n      vertex count (3..)
///       u16 flags  low byte: edge-visibility bits, 0x1000: first face of a group
///       u32 aux    0 or 1
///       u16 tag    material / sub-object index
///       n x (f64 x, f64 y, f64 z)
///       trailer    0xFFFFFFFF-filled link words, optional layer name string
/// ```
///
/// The faces are located by scanning for a plausible `n/flags/aux` header
/// followed by `n` plausible coordinates rather than by following the trailer
/// (its length varies with the layer-name string). On 129 of 174 blobs the
/// header face count matches the faces found exactly and the rest is within a
/// few faces.
pub fn parse_face_stream(b: &[u8]) -> Option<FaceStream> {
    if b.len() < 16 {
        return None;
    }
    let declared = if b[2..6] == [0xFF; 4] {
        u32_at(b, 6)
    } else {
        None
    };
    let mut out = FaceStream {
        declared_faces: declared.filter(|&n| n < 0xFFFF_0000),
        ..Default::default()
    };
    let n = b.len();
    let mut p = 6;
    let mut gap_start = 6;
    while p + 10 <= n {
        if let Some(end) = try_face(b, p, &mut out.mesh) {
            collect_layers(&b[gap_start..p], &mut out.layers);
            out.faces += 1;
            p = end;
            gap_start = p;
        } else {
            p += 1;
        }
    }
    collect_layers(&b[gap_start.min(n)..], &mut out.layers);
    if out.faces == 0 {
        return None;
    }
    Some(out)
}

fn try_face(b: &[u8], p: usize, mesh: &mut Mesh) -> Option<usize> {
    let nv = u16_at(b, p)? as usize;
    if !(3..=64).contains(&nv) {
        return None;
    }
    let flags = u16_at(b, p + 2)?;
    let aux = u32_at(b, p + 4)?;
    if flags & 0xE000 != 0 || aux > 0xFF {
        return None;
    }
    let start = p + 10;
    let end = start + nv * 24;
    if end > b.len() {
        return None;
    }
    let mut pts = Vec::with_capacity(nv);
    for k in 0..nv {
        let q = start + k * 24;
        let (x, y, z) = (f64_at(b, q)?, f64_at(b, q + 8)?, f64_at(b, q + 16)?);
        if ![x, y, z]
            .iter()
            .all(|v| v.is_finite() && v.abs() < MAX_COORD)
        {
            return None;
        }
        pts.push([x, y, z]);
    }
    let base = mesh.vertices.len() as u32;
    mesh.vertices.extend_from_slice(&pts);
    for k in 1..nv as u32 - 1 {
        mesh.triangles.push([base, base + k, base + k + 1]);
    }
    Some(end)
}

/// Appends printable-ASCII words of the gap bytes (layer names) to `out`.
fn collect_layers(gap: &[u8], out: &mut Vec<String>) {
    let mut run = Vec::new();
    for &c in gap.iter().chain(std::iter::once(&0u8)) {
        if (0x20..0x7F).contains(&c) {
            run.push(c);
            continue;
        }
        if run.len() >= 4 && run.iter().filter(|c| c.is_ascii_alphabetic()).count() >= 3 {
            let s = String::from_utf8_lossy(&run).trim().to_owned();
            if s.len() >= 4 && !out.contains(&s) {
                out.push(s);
            }
        }
        run.clear();
    }
}

/// Decodes every `CD AB 74 00` triangle-mesh record in `b` (an `AssociatedData`
/// tail or a `SymbolData` blob).
///
/// Layout, verified on 588 records in Core Architectural, Interiors, MEP and
/// Exteriors:
///
/// ```text
/// 0   CD AB 74 00
/// 4   u16 flags   (0x0f7f, 0x0ecd, 0x0ee2, 0x0e05, 0x0f58 seen)
/// 6   u32 version (1..5)
/// 10  u32 N       vertex count
/// 14  N x 48 bytes: f64 x, y, z, then three f64 that are 0 in every sample
/// ..  u32 M       triangle record count
/// ..  M x 80 bytes: u32 a, b, c (vertex indices), u32 partner triangle,
///                   u32 e, u32 f (ids), then 14 zero words
/// ```
///
/// Records that do not validate (counts beyond the blob, indices past `N`,
/// non-finite coordinates) are skipped.
pub fn parse_triangle_meshes(b: &[u8]) -> Vec<Mesh> {
    let mut out = Vec::new();
    let mut p = 0;
    while p + 18 <= b.len() {
        if b[p] == 0xCD && b[p + 1] == 0xAB && b[p + 2] == 0x74 && b[p + 3] == 0x00 {
            if let Some((mesh, end)) = triangle_mesh_at(b, p) {
                out.push(mesh);
                p = end;
                continue;
            }
        }
        p += 1;
    }
    out
}

fn triangle_mesh_at(b: &[u8], p: usize) -> Option<(Mesh, usize)> {
    let n = u32_at(b, p + 10)? as usize;
    if !(3..=4_000_000).contains(&n) {
        return None;
    }
    let vstart = p + 14;
    let mstart = vstart.checked_add(n.checked_mul(48)?)?;
    let m = u32_at(b, mstart)? as usize;
    if m == 0 || m > 8_000_000 {
        return None;
    }
    let tstart = mstart + 4;
    let end = tstart.checked_add(m.checked_mul(80)?)?;
    if end > b.len() {
        return None;
    }
    let mut mesh = Mesh {
        vertices: Vec::with_capacity(n),
        triangles: Vec::with_capacity(m),
    };
    for i in 0..n {
        let q = vstart + i * 48;
        let v = [f64_at(b, q)?, f64_at(b, q + 8)?, f64_at(b, q + 16)?];
        if !v.iter().all(|c| c.is_finite() && c.abs() < MAX_COORD) {
            return None;
        }
        mesh.vertices.push(v);
    }
    for i in 0..m {
        let q = tstart + i * 80;
        let t = [u32_at(b, q)?, u32_at(b, q + 4)?, u32_at(b, q + 8)?];
        if t.iter().any(|&v| v as usize >= n) {
            return None;
        }
        if t[0] != t[1] && t[1] != t[2] && t[0] != t[2] {
            mesh.triangles.push(t);
        }
    }
    Some((mesh, end))
}

#[cfg(test)]
pub(crate) mod testdata {
    /// Builds a synthetic `symDxf`-style stream with the given quads.
    pub fn face_stream(quads: &[[[f64; 3]; 4]]) -> Vec<u8> {
        let mut b = vec![0x2B, 0x09, 0xFF, 0xFF, 0xFF, 0xFF];
        b.extend_from_slice(&(quads.len() as u32).to_le_bytes());
        for q in quads {
            b.extend_from_slice(&4u16.to_le_bytes());
            b.extend_from_slice(&0x100Fu16.to_le_bytes());
            b.extend_from_slice(&0u32.to_le_bytes());
            b.extend_from_slice(&1u16.to_le_bytes());
            for v in q {
                for c in v {
                    b.extend_from_slice(&c.to_le_bytes());
                }
            }
            b.extend_from_slice(&[0xFF; 12]);
            b.extend_from_slice(&13u32.to_le_bytes());
            b.extend_from_slice(b"Adjust Light");
        }
        b
    }

    /// Builds a synthetic `CD AB 74 00` record.
    pub fn tri_mesh(verts: &[[f64; 3]], tris: &[[u32; 3]]) -> Vec<u8> {
        let mut b = vec![0xCD, 0xAB, 0x74, 0x00, 0x7F, 0x0F, 5, 0, 0, 0];
        b.extend_from_slice(&(verts.len() as u32).to_le_bytes());
        for v in verts {
            for c in v {
                b.extend_from_slice(&c.to_le_bytes());
            }
            b.extend_from_slice(&[0u8; 24]);
        }
        b.extend_from_slice(&(tris.len() as u32).to_le_bytes());
        for t in tris {
            for i in t {
                b.extend_from_slice(&i.to_le_bytes());
            }
            b.extend_from_slice(&[0u8; 68]);
        }
        b
    }

    /// A box `w` x `d` x `h` with its back-left-bottom corner at the origin
    /// (12 triangles, shared vertices).
    pub fn box_mesh(w: f64, d: f64, h: f64) -> (Vec<[f64; 3]>, Vec<[u32; 3]>) {
        let v = vec![
            [0.0, 0.0, 0.0],
            [w, 0.0, 0.0],
            [w, d, 0.0],
            [0.0, d, 0.0],
            [0.0, 0.0, h],
            [w, 0.0, h],
            [w, d, h],
            [0.0, d, h],
        ];
        let t = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        (v, t)
    }
}

#[cfg(test)]
mod tests {
    use super::parse_face_stream;
    use super::parse_triangle_meshes;
    use super::testdata::*;

    #[test]
    fn face_stream_roundtrip() {
        let quad = [
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 5.0, 0.0],
            [0.0, 5.0, 0.0],
        ];
        let other = [
            [0.0, 0.0, 3.0],
            [10.0, 0.0, 3.0],
            [10.0, 5.0, 3.0],
            [0.0, 5.0, 3.0],
        ];
        let fs = parse_face_stream(&face_stream(&[quad, other])).unwrap();
        assert_eq!(fs.faces, 2);
        assert_eq!(fs.declared_faces, Some(2));
        assert_eq!(fs.mesh.triangles.len(), 4);
        assert_eq!(fs.layers, ["Adjust Light"]);
        let b = fs.mesh.bounds().unwrap();
        assert_eq!(b.extent(), [10.0, 5.0, 3.0]);
    }

    #[test]
    fn face_stream_rejects_garbage() {
        assert!(parse_face_stream(&[0u8; 8]).is_none());
        assert!(parse_face_stream(&[0xFFu8; 64]).is_none());
        assert!(parse_face_stream(b"not a face stream at all, just text").is_none());
    }

    #[test]
    fn triangle_mesh_roundtrip_and_validation() {
        let (v, t) = box_mesh(4.0, 3.0, 2.0);
        let mut blob = vec![9u8; 7];
        blob.extend(tri_mesh(&v, &t));
        blob.extend_from_slice(&[1, 2, 3]);
        let meshes = parse_triangle_meshes(&blob);
        assert_eq!(meshes.len(), 1);
        assert_eq!(meshes[0].triangles.len(), 12);
        assert_eq!(meshes[0].bounds().unwrap().extent(), [4.0, 3.0, 2.0]);

        // Index past the vertex array: rejected.
        let bad = tri_mesh(&v, &[[0, 1, 99]]);
        assert!(parse_triangle_meshes(&bad).is_empty());
        // Truncated record: rejected.
        assert!(parse_triangle_meshes(&tri_mesh(&v, &t)[..200]).is_empty());
    }
}
