//! Autodesk 3DS reader for 3D library symbols.
//!
//! A 3DS file is a tree of chunks (`u16` id, `u32` length including the
//! 6-byte header, little endian). This reader takes, from the editor chunk
//! (`0x3D3D`):
//!
//! * every named object's triangle mesh (`0x4100`): vertices (`0x4110`),
//!   faces (`0x4120`) and the per-material face lists (`0x4130`), one
//!   [`ImportedPart`] per object and material;
//! * every material (`0xAFFF`): name, diffuse color and the diffuse
//!   texture's file name.
//!
//! Keyframes, lights, cameras, texture coordinates and the local mesh matrix
//! are ignored (3DS stores mesh vertices in world space). 3DS is Z up, so the
//! default import options for it use [`UpAxis::Z`]; there are no units, so
//! the caller picks or guesses them ([`crate::detect`]).

use crate::model::{ImportedModel, ImportedPart, ModelError, ModelOptions};
use std::collections::HashMap;

const MAIN: u16 = 0x4D4D;
const EDITOR: u16 = 0x3D3D;
const OBJECT: u16 = 0x4000;
const TRIMESH: u16 = 0x4100;
const VERTICES: u16 = 0x4110;
const FACES: u16 = 0x4120;
const FACE_MATERIAL: u16 = 0x4130;
const MATERIAL: u16 = 0xAFFF;
const MAT_NAME: u16 = 0xA000;
const MAT_DIFFUSE: u16 = 0xA020;
const MAT_TEXMAP: u16 = 0xA200;
const MAT_TEXFILE: u16 = 0xA300;
const COLOR_FLOAT: u16 = 0x0010;
const COLOR_BYTE: u16 = 0x0011;
const COLOR_BYTE_GAMMA: u16 = 0x0012;
const COLOR_FLOAT_GAMMA: u16 = 0x0013;

/// One chunk: id and payload (without the header).
struct Chunk<'a> {
    id: u16,
    body: &'a [u8],
}

/// The chunks laid end to end in `data`; a chunk that overruns `data` ends
/// the list (damaged files still give what came before).
fn chunks(data: &[u8]) -> Vec<Chunk<'_>> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at + 6 <= data.len() {
        let id = u16::from_le_bytes([data[at], data[at + 1]]);
        let len =
            u32::from_le_bytes([data[at + 2], data[at + 3], data[at + 4], data[at + 5]]) as usize;
        if len < 6 || at + len > data.len() {
            break;
        }
        out.push(Chunk {
            id,
            body: &data[at + 6..at + len],
        });
        at += len;
    }
    out
}

/// A NUL-terminated string at the start of `b` and the bytes after it.
fn cstring(b: &[u8]) -> (String, &[u8]) {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    let s = b[..end].iter().map(|&c| c as char).collect();
    (s, b.get(end + 1..).unwrap_or(&[]))
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(at)?, *b.get(at + 1)?]))
}

fn f32_at(b: &[u8], at: usize) -> Option<f32> {
    Some(f32::from_le_bytes([
        *b.get(at)?,
        *b.get(at + 1)?,
        *b.get(at + 2)?,
        *b.get(at + 3)?,
    ]))
}

/// A material of the file.
#[derive(Debug, Clone, Default)]
struct Material {
    color: Option<[u8; 3]>,
    texture: Option<String>,
}

fn color_of(chunk_body: &[u8]) -> Option<[u8; 3]> {
    for c in chunks(chunk_body) {
        match c.id {
            COLOR_BYTE | COLOR_BYTE_GAMMA => {
                return Some([*c.body.first()?, *c.body.get(1)?, *c.body.get(2)?]);
            }
            COLOR_FLOAT | COLOR_FLOAT_GAMMA => {
                let q = |i: usize| (f32_at(c.body, i * 4).unwrap_or(0.0).clamp(0.0, 1.0) * 255.0).round() as u8;
                return Some([q(0), q(1), q(2)]);
            }
            _ => {}
        }
    }
    None
}

fn read_material(body: &[u8]) -> (String, Material) {
    let mut name = String::new();
    let mut m = Material::default();
    for c in chunks(body) {
        match c.id {
            MAT_NAME => name = cstring(c.body).0,
            MAT_DIFFUSE => m.color = color_of(c.body),
            MAT_TEXMAP => {
                for t in chunks(c.body) {
                    if t.id == MAT_TEXFILE {
                        m.texture = Some(cstring(t.body).0);
                    }
                }
            }
            _ => {}
        }
    }
    (name, m)
}

/// One object's mesh before it is split by material.
struct Mesh {
    name: String,
    verts: Vec<[f32; 3]>,
    faces: Vec<[u16; 3]>,
    /// `(material name, face indices)`.
    face_materials: Vec<(String, Vec<u16>)>,
}

fn read_mesh(name: &str, body: &[u8]) -> Option<Mesh> {
    let mut mesh = Mesh {
        name: name.to_string(),
        verts: Vec::new(),
        faces: Vec::new(),
        face_materials: Vec::new(),
    };
    for c in chunks(body) {
        match c.id {
            VERTICES => {
                let n = u16_at(c.body, 0)? as usize;
                for i in 0..n {
                    let o = 2 + i * 12;
                    mesh.verts
                        .push([f32_at(c.body, o)?, f32_at(c.body, o + 4)?, f32_at(c.body, o + 8)?]);
                }
            }
            FACES => {
                let n = u16_at(c.body, 0)? as usize;
                for i in 0..n {
                    let o = 2 + i * 8;
                    mesh.faces
                        .push([u16_at(c.body, o)?, u16_at(c.body, o + 2)?, u16_at(c.body, o + 4)?]);
                }
                // Sub-chunks follow the face list.
                let rest = c.body.get(2 + n * 8..).unwrap_or(&[]);
                for s in chunks(rest) {
                    if s.id == FACE_MATERIAL {
                        let (mat, tail) = cstring(s.body);
                        let count = u16_at(tail, 0).unwrap_or(0) as usize;
                        let faces = (0..count).filter_map(|i| u16_at(tail, 2 + i * 2)).collect();
                        mesh.face_materials.push((mat, faces));
                    }
                }
            }
            _ => {}
        }
    }
    (!mesh.verts.is_empty() && !mesh.faces.is_empty()).then_some(mesh)
}

/// True when `bytes` start like a 3DS file.
pub fn is_3ds(bytes: &[u8]) -> bool {
    u16_at(bytes, 0) == Some(MAIN)
}

/// Parses a 3DS file. Fails when it holds no mesh.
pub fn parse_3ds(bytes: &[u8], opts: &ModelOptions) -> Result<ImportedModel, ModelError> {
    if !is_3ds(bytes) {
        return Err(ModelError("Not a 3DS file".into()));
    }
    let main = chunks(bytes);
    let Some(main) = main.iter().find(|c| c.id == MAIN) else {
        return Err(ModelError("Not a 3DS file".into()));
    };
    let mut materials: HashMap<String, Material> = HashMap::new();
    let mut meshes: Vec<Mesh> = Vec::new();
    for ed in chunks(main.body).iter().filter(|c| c.id == EDITOR) {
        for c in chunks(ed.body) {
            match c.id {
                MATERIAL => {
                    let (n, m) = read_material(c.body);
                    materials.insert(n, m);
                }
                OBJECT => {
                    let (name, rest) = cstring(c.body);
                    for t in chunks(rest).iter().filter(|t| t.id == TRIMESH) {
                        if let Some(m) = read_mesh(&name, t.body) {
                            meshes.push(m);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let mut parts = Vec::new();
    for mesh in &meshes {
        let n = mesh.verts.len();
        let mut taken = vec![false; mesh.faces.len()];
        let mut groups: Vec<(Option<&str>, Vec<usize>)> = Vec::new();
        for (mat, faces) in &mesh.face_materials {
            let list: Vec<usize> = faces
                .iter()
                .map(|&f| f as usize)
                .filter(|&f| f < mesh.faces.len() && !taken[f])
                .collect();
            for &f in &list {
                taken[f] = true;
            }
            if !list.is_empty() {
                groups.push((Some(mat.as_str()), list));
            }
        }
        let rest: Vec<usize> = (0..mesh.faces.len()).filter(|&f| !taken[f]).collect();
        if !rest.is_empty() {
            groups.push((None, rest));
        }
        for (mat, list) in groups {
            let m = mat.and_then(|n| materials.get(n));
            let mut indices = Vec::new();
            for f in list {
                let t = mesh.faces[f];
                if t.iter().all(|&i| (i as usize) < n) {
                    indices.extend(t.iter().map(|&i| u32::from(i)));
                }
            }
            parts.push(ImportedPart {
                name: mesh.name.clone(),
                color: m.and_then(|m| m.color),
                material: mat.map(str::to_string),
                texture: m.and_then(|m| m.texture.clone()),
                positions: mesh.verts.clone(),
                indices,
            });
        }
    }
    if parts.is_empty() {
        return Err(ModelError("The 3DS file has no meshes".into()));
    }
    let model = ImportedModel { parts }.cleaned().converted(opts);
    if model.is_empty() {
        return Err(ModelError("The 3DS file has no usable faces".into()));
    }
    Ok(model)
}

/// Builders of 3DS bytes for tests.
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::UpAxis;

    pub(crate) fn chunk(id: u16, body: &[u8]) -> Vec<u8> {
        let mut v = id.to_le_bytes().to_vec();
        v.extend_from_slice(&((body.len() + 6) as u32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    fn cstr(s: &str) -> Vec<u8> {
        let mut v = s.as_bytes().to_vec();
        v.push(0);
        v
    }

    /// A 3DS of one box object `w x d x h` with two materials: the top face
    /// pair "Lid" (red, with a texture) and the rest unassigned.
    pub(crate) fn box_3ds(w: f32, d: f32, h: f32) -> Vec<u8> {
        let tris = crate::stl::tests::box_tris(w, d, h);
        let mut verts: Vec<[f32; 3]> = Vec::new();
        let mut faces: Vec<[u16; 3]> = Vec::new();
        for t in &tris {
            let mut f = [0u16; 3];
            for (k, p) in t.iter().enumerate() {
                let i = verts.iter().position(|q| q == p).unwrap_or_else(|| {
                    verts.push(*p);
                    verts.len() - 1
                });
                f[k] = i as u16;
            }
            faces.push(f);
        }
        let mut vbody = (verts.len() as u16).to_le_bytes().to_vec();
        for p in &verts {
            for c in p {
                vbody.extend_from_slice(&c.to_le_bytes());
            }
        }
        // Faces 2 and 3 are the top (see `box_tris` order).
        let mut mbody = cstr("Lid");
        mbody.extend_from_slice(&2u16.to_le_bytes());
        mbody.extend_from_slice(&2u16.to_le_bytes());
        mbody.extend_from_slice(&3u16.to_le_bytes());
        let mut fbody = (faces.len() as u16).to_le_bytes().to_vec();
        for f in &faces {
            for i in f {
                fbody.extend_from_slice(&i.to_le_bytes());
            }
            fbody.extend_from_slice(&0u16.to_le_bytes());
        }
        fbody.extend(chunk(FACE_MATERIAL, &mbody));
        let mut tri = chunk(VERTICES, &vbody);
        tri.extend(chunk(FACES, &fbody));
        let mut obj = cstr("Crate");
        obj.extend(chunk(TRIMESH, &tri));

        let mut lid = chunk(MAT_NAME, &cstr("Lid"));
        lid.extend(chunk(MAT_DIFFUSE, &chunk(COLOR_BYTE, &[200, 30, 20])));
        lid.extend(chunk(
            MAT_TEXMAP,
            &chunk(MAT_TEXFILE, &cstr("lid_oak.jpg")),
        ));
        let mut editor = chunk(MATERIAL, &lid);
        editor.extend(chunk(OBJECT, &obj));
        chunk(MAIN, &chunk(EDITOR, &editor))
    }

    #[test]
    fn reads_the_mesh_materials_and_texture() {
        let m = parse_3ds(&box_3ds(10.0, 20.0, 30.0), &ModelOptions::default()).unwrap();
        assert_eq!(m.triangle_count(), 12);
        assert_eq!(m.extent().unwrap(), [10.0, 20.0, 30.0]);
        let lid = m.parts.iter().find(|p| p.material.as_deref() == Some("Lid")).unwrap();
        assert_eq!(lid.triangle_count(), 2);
        assert_eq!(lid.color, Some([200, 30, 20]));
        assert_eq!(lid.texture.as_deref(), Some("lid_oak.jpg"));
        assert_eq!(lid.name, "Crate");
        let rest = m.parts.iter().find(|p| p.material.is_none()).unwrap();
        assert_eq!(rest.triangle_count(), 10);
        assert_eq!(rest.color, None);
    }

    #[test]
    fn z_up_maps_depth_and_height() {
        let opts = ModelOptions {
            unit_scale: 1.0,
            up_axis: UpAxis::Z,
        };
        let m = parse_3ds(&box_3ds(10.0, 20.0, 30.0), &opts).unwrap();
        assert_eq!(m.extent().unwrap(), [10.0, 30.0, 20.0]);
    }

    #[test]
    fn float_colors_and_damaged_files() {
        let mut fl = Vec::new();
        for c in [1.0f32, 0.5, 0.0] {
            fl.extend_from_slice(&c.to_le_bytes());
        }
        assert_eq!(
            color_of(&chunk(COLOR_FLOAT, &fl)),
            Some([255, 128, 0]),
            "float rgb"
        );
        assert!(parse_3ds(b"nope", &ModelOptions::default()).is_err());
        // A main chunk with no editor chunk has no mesh.
        assert!(parse_3ds(&chunk(MAIN, &[]), &ModelOptions::default()).is_err());
        // Cut short: the mesh chunks no longer fit, so there is nothing.
        let bytes = box_3ds(1.0, 1.0, 1.0);
        assert!(parse_3ds(&bytes[..bytes.len() / 2], &ModelOptions::default()).is_err());
    }
}
