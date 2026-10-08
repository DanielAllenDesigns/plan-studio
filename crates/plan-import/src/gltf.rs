//! glTF 2.0 reader (`.gltf` JSON with external or `data:` buffers, and binary
//! `.glb`) for 3D library symbols. No external crates.
//!
//! Supported: scenes and the node hierarchy (`matrix` or translation /
//! rotation / scale), meshes with triangle, triangle-strip and triangle-fan
//! primitives, indexed or not, 8/16/32-bit indices, float `POSITION`
//! accessors with byte strides, and `baseColorFactor` as the part color.
//! Not supported (ignored or an error): sparse accessors, Draco and
//! `KHR_mesh_quantization` compression, textures, skins, animations and
//! cameras. glTF is Y-up, +Z-front and in meters; [`default_options`] holds
//! those defaults.

use crate::model::{
    ImportedModel, ImportedPart, ModelError, ModelOptions, UpAxis, INCHES_PER_METER,
};
use serde_json::Value;

/// Meters to inches, Y up: what glTF means by its numbers.
pub fn default_options() -> ModelOptions {
    ModelOptions {
        unit_scale: INCHES_PER_METER,
        up_axis: UpAxis::Y,
    }
}

fn bad<T>(m: impl Into<String>) -> Result<T, ModelError> {
    Err(ModelError(m.into()))
}

// ----- base64 -----

/// Decodes standard base64 (padding optional, whitespace ignored).
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

// ----- matrices (column-major 4x4) -----

type Mat = [f64; 16];

const IDENTITY: Mat = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

fn mul(a: &Mat, b: &Mat) -> Mat {
    let mut r = [0.0; 16];
    for c in 0..4 {
        for row in 0..4 {
            r[c * 4 + row] = (0..4).map(|k| a[k * 4 + row] * b[c * 4 + k]).sum();
        }
    }
    r
}

fn transform_point(m: &Mat, p: [f32; 3]) -> [f32; 3] {
    let (x, y, z) = (p[0] as f64, p[1] as f64, p[2] as f64);
    [
        (m[0] * x + m[4] * y + m[8] * z + m[12]) as f32,
        (m[1] * x + m[5] * y + m[9] * z + m[13]) as f32,
        (m[2] * x + m[6] * y + m[10] * z + m[14]) as f32,
    ]
}

fn determinant3(m: &Mat) -> f64 {
    m[0] * (m[5] * m[10] - m[9] * m[6]) - m[4] * (m[1] * m[10] - m[9] * m[2])
        + m[8] * (m[1] * m[6] - m[5] * m[2])
}

fn numbers(v: Option<&Value>, n: usize) -> Option<Vec<f64>> {
    let a = v?.as_array()?;
    (a.len() == n)
        .then(|| a.iter().filter_map(Value::as_f64).collect::<Vec<_>>())
        .filter(|v| v.len() == n)
}

fn node_matrix(node: &Value) -> Mat {
    if let Some(m) = numbers(node.get("matrix"), 16) {
        let mut out = IDENTITY;
        out.copy_from_slice(&m);
        return out;
    }
    let t = numbers(node.get("translation"), 3).unwrap_or(vec![0.0; 3]);
    let q = numbers(node.get("rotation"), 4).unwrap_or(vec![0.0, 0.0, 0.0, 1.0]);
    let s = numbers(node.get("scale"), 3).unwrap_or(vec![1.0; 3]);
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    let (x, y, z, w) = if n > 1e-12 {
        (q[0] / n, q[1] / n, q[2] / n, q[3] / n)
    } else {
        (0.0, 0.0, 0.0, 1.0)
    };
    [
        (1.0 - 2.0 * (y * y + z * z)) * s[0],
        (2.0 * (x * y + z * w)) * s[0],
        (2.0 * (x * z - y * w)) * s[0],
        0.0,
        (2.0 * (x * y - z * w)) * s[1],
        (1.0 - 2.0 * (x * x + z * z)) * s[1],
        (2.0 * (y * z + x * w)) * s[1],
        0.0,
        (2.0 * (x * z + y * w)) * s[2],
        (2.0 * (y * z - x * w)) * s[2],
        (1.0 - 2.0 * (x * x + y * y)) * s[2],
        0.0,
        t[0],
        t[1],
        t[2],
        1.0,
    ]
}

// ----- container -----

/// A file that resolves a relative `uri` to its bytes (a `.bin` next to the
/// `.gltf`).
pub type Resolver<'a> = &'a dyn Fn(&str) -> Option<Vec<u8>>;

fn split_glb(bytes: &[u8]) -> Result<(Value, Option<Vec<u8>>), ModelError> {
    let u32_at = |at: usize| -> Option<u32> {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    if u32_at(4) != Some(2) {
        return bad("Only binary glTF version 2 is supported");
    }
    let total = (u32_at(8).unwrap_or(0) as usize).min(bytes.len());
    let mut at = 12;
    let mut json: Option<Value> = None;
    let mut bin: Option<Vec<u8>> = None;
    while at + 8 <= total {
        let len = u32_at(at).unwrap_or(0) as usize;
        let kind = u32_at(at + 4).unwrap_or(0);
        let Some(data) = bytes.get(at + 8..at + 8 + len) else {
            return bad("Truncated glb chunk");
        };
        match kind {
            0x4E4F_534A => {
                json = Some(
                    serde_json::from_slice(data)
                        .map_err(|e| ModelError(format!("Bad glTF JSON: {e}")))?,
                )
            }
            0x004E_4942 if bin.is_none() => bin = Some(data.to_vec()),
            _ => {}
        }
        at += 8 + len.div_ceil(4) * 4;
    }
    match json {
        Some(j) => Ok((j, bin)),
        None => bad("The glb file has no JSON chunk"),
    }
}

fn load_buffers(
    root: &Value,
    glb_bin: Option<Vec<u8>>,
    resolve: Option<Resolver>,
) -> Result<Vec<Vec<u8>>, ModelError> {
    let mut out = Vec::new();
    let mut glb = glb_bin;
    for (i, b) in root
        .get("buffers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let data = match b.get("uri").and_then(Value::as_str) {
            None => match glb.take() {
                Some(d) => d,
                None => return bad(format!("Buffer {i} has no data")),
            },
            Some(uri) if uri.starts_with("data:") => {
                let Some((_, payload)) = uri.split_once(";base64,") else {
                    return bad(format!("Buffer {i}: only base64 data URIs are supported"));
                };
                base64_decode(payload)
                    .ok_or_else(|| ModelError(format!("Buffer {i}: bad base64 data")))?
            }
            Some(uri) => {
                let name = uri.replace("%20", " ");
                match resolve.and_then(|r| r(&name)) {
                    Some(d) => d,
                    None => return bad(format!("The buffer file {name} was not found")),
                }
            }
        };
        out.push(data);
    }
    Ok(out)
}

// ----- accessors -----

struct View<'a> {
    data: &'a [u8],
    offset: usize,
    stride: usize,
    count: usize,
    comps: usize,
    ctype: u64,
}

fn comp_size(ctype: u64) -> Option<usize> {
    match ctype {
        5120 | 5121 => Some(1),
        5122 | 5123 => Some(2),
        5125 | 5126 => Some(4),
        _ => None,
    }
}

fn accessor<'a>(root: &Value, buffers: &'a [Vec<u8>], idx: usize) -> Result<View<'a>, ModelError> {
    let acc = root
        .get("accessors")
        .and_then(|a| a.get(idx))
        .ok_or_else(|| ModelError(format!("Missing accessor {idx}")))?;
    if acc.get("sparse").is_some() {
        return bad("Sparse accessors are not supported");
    }
    let count = acc.get("count").and_then(Value::as_u64).unwrap_or(0) as usize;
    let ctype = acc
        .get("componentType")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let comps = match acc.get("type").and_then(Value::as_str) {
        Some("SCALAR") => 1,
        Some("VEC2") => 2,
        Some("VEC3") => 3,
        Some("VEC4") => 4,
        _ => return bad("Unsupported accessor type"),
    };
    let csize = comp_size(ctype).ok_or_else(|| ModelError("Unsupported component type".into()))?;
    let Some(view_idx) = acc.get("bufferView").and_then(Value::as_u64) else {
        return bad("An accessor without a bufferView is not supported");
    };
    let view = root
        .get("bufferViews")
        .and_then(|a| a.get(view_idx as usize))
        .ok_or_else(|| ModelError("Missing bufferView".into()))?;
    let buf = view.get("buffer").and_then(Value::as_u64).unwrap_or(0) as usize;
    let data = buffers
        .get(buf)
        .ok_or_else(|| ModelError("Missing buffer".into()))?;
    let voff = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let aoff = acc.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let elem = csize * comps;
    let stride = view
        .get("byteStride")
        .and_then(Value::as_u64)
        .map_or(elem, |s| s as usize)
        .max(elem);
    let offset = voff + aoff;
    let need = if count == 0 {
        0
    } else {
        offset + stride * (count - 1) + elem
    };
    if need > data.len() {
        return bad("An accessor reads past the end of its buffer");
    }
    Ok(View {
        data,
        offset,
        stride,
        count,
        comps,
        ctype,
    })
}

impl View<'_> {
    fn read(&self, i: usize, c: usize) -> f64 {
        let size = comp_size(self.ctype).unwrap_or(1);
        let at = self.offset + i * self.stride + c * size;
        let b = &self.data[at..at + size];
        match self.ctype {
            5120 => b[0] as i8 as f64,
            5121 => b[0] as f64,
            5122 => i16::from_le_bytes([b[0], b[1]]) as f64,
            5123 => u16::from_le_bytes([b[0], b[1]]) as f64,
            5125 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
            _ => f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
        }
    }
}

fn read_positions(
    root: &Value,
    buffers: &[Vec<u8>],
    idx: usize,
) -> Result<Vec<[f32; 3]>, ModelError> {
    let v = accessor(root, buffers, idx)?;
    if v.comps != 3 || v.ctype != 5126 {
        return bad("POSITION must be float VEC3 (quantized meshes are not supported)");
    }
    Ok((0..v.count)
        .map(|i| {
            [
                v.read(i, 0) as f32,
                v.read(i, 1) as f32,
                v.read(i, 2) as f32,
            ]
        })
        .collect())
}

fn read_indices(root: &Value, buffers: &[Vec<u8>], idx: usize) -> Result<Vec<u32>, ModelError> {
    let v = accessor(root, buffers, idx)?;
    if v.comps != 1 || v.ctype == 5126 {
        return bad("Indices must be an unsigned integer SCALAR");
    }
    Ok((0..v.count).map(|i| v.read(i, 0) as u32).collect())
}

fn srgb_byte(linear: f64) -> u8 {
    let l = linear.clamp(0.0, 1.0);
    let s = if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round() as u8
}

fn material_color(root: &Value, prim: &Value) -> Option<[u8; 3]> {
    let m = root
        .get("materials")?
        .get(prim.get("material")?.as_u64()? as usize)?;
    let f = numbers(m.get("pbrMetallicRoughness")?.get("baseColorFactor"), 4)?;
    Some([srgb_byte(f[0]), srgb_byte(f[1]), srgb_byte(f[2])])
}

/// Triangle index list from a primitive's `mode`.
fn triangles(mode: u64, idx: &[u32]) -> Vec<u32> {
    match mode {
        4 => idx.as_chunks::<3>().0.iter().flatten().copied().collect(),
        5 => (0..idx.len().saturating_sub(2))
            .flat_map(|i| {
                if i % 2 == 0 {
                    [idx[i], idx[i + 1], idx[i + 2]]
                } else {
                    [idx[i + 1], idx[i], idx[i + 2]]
                }
            })
            .collect(),
        6 => (1..idx.len().saturating_sub(1))
            .flat_map(|i| [idx[0], idx[i], idx[i + 1]])
            .collect(),
        _ => Vec::new(),
    }
}

struct Walker<'a> {
    root: &'a Value,
    buffers: &'a [Vec<u8>],
    parts: Vec<ImportedPart>,
    depth: usize,
}

impl Walker<'_> {
    fn node(&mut self, idx: usize, parent: &Mat) -> Result<(), ModelError> {
        if self.depth > 64 {
            return bad("The glTF node tree is too deep (or loops)");
        }
        let Some(node) = self.root.get("nodes").and_then(|n| n.get(idx)) else {
            return bad(format!("Missing node {idx}"));
        };
        let m = mul(parent, &node_matrix(node));
        if let Some(mesh) = node.get("mesh").and_then(Value::as_u64) {
            self.mesh(mesh as usize, &m, node.get("name").and_then(Value::as_str))?;
        }
        self.depth += 1;
        for c in node
            .get("children")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(c) = c.as_u64() {
                self.node(c as usize, &m)?;
            }
        }
        self.depth -= 1;
        Ok(())
    }

    fn mesh(&mut self, idx: usize, m: &Mat, node_name: Option<&str>) -> Result<(), ModelError> {
        let Some(mesh) = self.root.get("meshes").and_then(|n| n.get(idx)) else {
            return bad(format!("Missing mesh {idx}"));
        };
        let name = mesh
            .get("name")
            .and_then(Value::as_str)
            .or(node_name)
            .unwrap_or("");
        let flip = determinant3(m) < 0.0;
        for prim in mesh
            .get("primitives")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let mode = prim.get("mode").and_then(Value::as_u64).unwrap_or(4);
            if !(4..=6).contains(&mode) {
                continue;
            }
            let Some(pos_idx) = prim
                .get("attributes")
                .and_then(|a| a.get("POSITION"))
                .and_then(Value::as_u64)
            else {
                continue;
            };
            let positions = read_positions(self.root, self.buffers, pos_idx as usize)?;
            let seq: Vec<u32>;
            let raw = match prim.get("indices").and_then(Value::as_u64) {
                Some(i) => {
                    seq = read_indices(self.root, self.buffers, i as usize)?;
                    &seq
                }
                None => {
                    seq = (0..positions.len() as u32).collect();
                    &seq
                }
            };
            let mut indices = triangles(mode, raw);
            if flip {
                for t in indices.as_chunks_mut::<3>().0 {
                    t.swap(1, 2);
                }
            }
            self.parts.push(ImportedPart {
                name: name.to_string(),
                color: material_color(self.root, prim),
                positions: positions.iter().map(|p| transform_point(m, *p)).collect(),
                indices,
            });
        }
        Ok(())
    }
}

/// Parses a `.gltf` (JSON) or `.glb` file. `resolve` loads external buffer
/// files by their relative `uri`. The result is converted with `opts`
/// (see [`default_options`]).
pub fn parse_gltf(
    bytes: &[u8],
    resolve: Option<Resolver>,
    opts: &ModelOptions,
) -> Result<ImportedModel, ModelError> {
    let (root, glb_bin) = if bytes.starts_with(b"glTF") {
        split_glb(bytes)?
    } else {
        let v: Value = serde_json::from_slice(bytes)
            .map_err(|e| ModelError(format!("Not a glTF file: {e}")))?;
        (v, None)
    };
    let version = root
        .get("asset")
        .and_then(|a| a.get("version"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if !version.starts_with('2') {
        return bad("Only glTF 2.0 files are supported");
    }
    if root
        .get("extensionsRequired")
        .and_then(Value::as_array)
        .is_some_and(|a| {
            a.iter()
                .any(|e| e.as_str() == Some("KHR_draco_mesh_compression"))
        })
    {
        return bad("Draco-compressed glTF files are not supported");
    }
    let buffers = load_buffers(&root, glb_bin, resolve)?;
    let mut w = Walker {
        root: &root,
        buffers: &buffers,
        parts: Vec::new(),
        depth: 0,
    };
    let scene_nodes: Option<Vec<usize>> = root
        .get("scenes")
        .and_then(Value::as_array)
        .and_then(|s| {
            let i = root.get("scene").and_then(Value::as_u64).unwrap_or(0) as usize;
            s.get(i)
        })
        .and_then(|s| s.get("nodes"))
        .and_then(Value::as_array)
        .map(|n| {
            n.iter()
                .filter_map(|v| v.as_u64().map(|u| u as usize))
                .collect()
        });
    match scene_nodes {
        Some(nodes) => {
            for n in nodes {
                w.node(n, &IDENTITY)?;
            }
        }
        None => {
            // No scene: every mesh as it is.
            let count = root
                .get("meshes")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            for i in 0..count {
                w.mesh(i, &IDENTITY, None)?;
            }
        }
    }
    let model = ImportedModel { parts: w.parts }.cleaned().converted(opts);
    if model.is_empty() {
        return bad("The glTF file has no triangle meshes");
    }
    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn b64(data: &[u8]) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for c in data.chunks(3) {
            let n = (c[0] as u32) << 16
                | (*c.get(1).unwrap_or(&0) as u32) << 8
                | *c.get(2).unwrap_or(&0) as u32;
            s.push(T[(n >> 18) as usize & 63] as char);
            s.push(T[(n >> 12) as usize & 63] as char);
            s.push(if c.len() > 1 {
                T[(n >> 6) as usize & 63] as char
            } else {
                '='
            });
            s.push(if c.len() > 2 {
                T[n as usize & 63] as char
            } else {
                '='
            });
        }
        s
    }

    /// A unit quad in the XY plane as two triangles plus 16-bit indices.
    fn quad_buffer() -> Vec<u8> {
        let mut b = Vec::new();
        for p in [
            [0.0f32, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ] {
            for c in p {
                b.extend_from_slice(&c.to_le_bytes());
            }
        }
        for i in [0u16, 1, 2, 0, 2, 3] {
            b.extend_from_slice(&i.to_le_bytes());
        }
        b
    }

    fn quad_json(uri: Option<String>, node: Value, mode: u64) -> Value {
        let buf = quad_buffer();
        let mut buffer = json!({ "byteLength": buf.len() });
        if let Some(u) = uri {
            buffer["uri"] = json!(u);
        }
        json!({
            "asset": { "version": "2.0" },
            "scene": 0,
            "scenes": [{ "nodes": [0] }],
            "nodes": [node],
            "meshes": [{ "name": "quad", "primitives": [{
                "attributes": { "POSITION": 0 }, "indices": 1, "mode": mode, "material": 0 }] }],
            "materials": [{ "pbrMetallicRoughness": { "baseColorFactor": [1.0, 0.0, 0.0, 1.0] } }],
            "buffers": [buffer],
            "bufferViews": [
                { "buffer": 0, "byteOffset": 0, "byteLength": 48 },
                { "buffer": 0, "byteOffset": 48, "byteLength": 12 }],
            "accessors": [
                { "bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3" },
                { "bufferView": 1, "componentType": 5123, "count": 6, "type": "SCALAR" }],
        })
    }

    fn data_uri() -> String {
        format!(
            "data:application/octet-stream;base64,{}",
            b64(&quad_buffer())
        )
    }

    fn glb(json: &Value, bin: &[u8]) -> Vec<u8> {
        let mut j = serde_json::to_vec(json).unwrap();
        while !j.len().is_multiple_of(4) {
            j.push(b' ');
        }
        let mut b = bin.to_vec();
        while !b.len().is_multiple_of(4) {
            b.push(0);
        }
        let total = 12 + 8 + j.len() + 8 + b.len();
        let mut out = b"glTF".to_vec();
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(j.len() as u32).to_le_bytes());
        out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
        out.extend_from_slice(&j);
        out.extend_from_slice(&(b.len() as u32).to_le_bytes());
        out.extend_from_slice(&0x004E_4942u32.to_le_bytes());
        out.extend_from_slice(&b);
        out
    }

    const INCHES: ModelOptions = ModelOptions {
        unit_scale: 1.0,
        up_axis: UpAxis::Y,
    };

    #[test]
    fn base64_decodes() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64_decode("aGVsbG8").unwrap(), b"hello");
        assert_eq!(base64_decode("aGV sbG8=\n").unwrap(), b"hello");
        assert!(base64_decode("a$b").is_none());
        let data: Vec<u8> = (0..=255).collect();
        assert_eq!(base64_decode(&b64(&data)).unwrap(), data);
    }

    #[test]
    fn a_data_uri_gltf_gives_two_red_triangles() {
        let j = quad_json(Some(data_uri()), json!({ "mesh": 0 }), 4);
        let m = parse_gltf(&serde_json::to_vec(&j).unwrap(), None, &INCHES).unwrap();
        assert_eq!(m.triangle_count(), 2);
        assert_eq!(m.parts[0].name, "quad");
        assert_eq!(m.parts[0].color, Some([255, 0, 0]));
        assert_eq!(m.extent().unwrap(), [1.0, 1.0, 0.0]);
        // Meters become inches with the defaults.
        let inch = parse_gltf(&serde_json::to_vec(&j).unwrap(), None, &default_options()).unwrap();
        assert!((inch.extent().unwrap()[0] - 39.370_08).abs() < 1e-3);
    }

    #[test]
    fn a_glb_and_an_external_buffer_read_the_same() {
        let j = quad_json(None, json!({ "mesh": 0 }), 4);
        let g = glb(&j, &quad_buffer());
        let from_glb = parse_gltf(&g, None, &INCHES).unwrap();
        assert_eq!(from_glb.triangle_count(), 2);

        let ext = quad_json(Some("quad.bin".into()), json!({ "mesh": 0 }), 4);
        let bin = quad_buffer();
        let resolver = |name: &str| (name == "quad.bin").then(|| bin.clone());
        let from_ext =
            parse_gltf(&serde_json::to_vec(&ext).unwrap(), Some(&resolver), &INCHES).unwrap();
        assert_eq!(from_ext, from_glb);
        // A missing buffer file is an error, not a panic.
        let none = |_: &str| None;
        assert!(parse_gltf(&serde_json::to_vec(&ext).unwrap(), Some(&none), &INCHES).is_err());
    }

    #[test]
    fn node_transforms_apply_and_mirrored_nodes_keep_outward_faces() {
        let moved = quad_json(
            Some(data_uri()),
            json!({ "mesh": 0, "translation": [10.0, 0.0, 0.0], "scale": [2.0, 2.0, 2.0] }),
            4,
        );
        let m = parse_gltf(&serde_json::to_vec(&moved).unwrap(), None, &INCHES).unwrap();
        let (lo, hi) = m.bounds().unwrap();
        assert_eq!((lo[0], hi[0], hi[1]), (10.0, 12.0, 2.0));

        let mirrored = quad_json(
            Some(data_uri()),
            json!({ "mesh": 0, "scale": [-1.0, 1.0, 1.0] }),
            4,
        );
        let flipped = parse_gltf(&serde_json::to_vec(&mirrored).unwrap(), None, &INCHES).unwrap();
        let plain = quad_json(Some(data_uri()), json!({ "mesh": 0 }), 4);
        let plain = parse_gltf(&serde_json::to_vec(&plain).unwrap(), None, &INCHES).unwrap();
        // The unit quad faces +Z; mirrored in x, the winding is swapped so
        // it still faces +Z.
        let nz = |m: &ImportedModel| {
            let p = &m.parts[0];
            let t = &p.indices[..3];
            let (a, b, c) = (
                p.positions[t[0] as usize],
                p.positions[t[1] as usize],
                p.positions[t[2] as usize],
            );
            (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
        };
        assert!(nz(&plain) > 0.0 && nz(&flipped) > 0.0);
    }

    #[test]
    fn strips_fans_and_unsupported_modes() {
        // Indices 0,1,2,0,2,3 as a strip are 4 triangles; as a fan, 4 too;
        // lines (mode 1) give nothing and so an error.
        let strip = quad_json(Some(data_uri()), json!({ "mesh": 0 }), 5);
        let m = parse_gltf(&serde_json::to_vec(&strip).unwrap(), None, &INCHES).unwrap();
        assert!(m.triangle_count() >= 2 && m.triangle_count() <= 4);
        let fan = quad_json(Some(data_uri()), json!({ "mesh": 0 }), 6);
        assert!(parse_gltf(&serde_json::to_vec(&fan).unwrap(), None, &INCHES).is_ok());
        let lines = quad_json(Some(data_uri()), json!({ "mesh": 0 }), 1);
        assert!(parse_gltf(&serde_json::to_vec(&lines).unwrap(), None, &INCHES).is_err());
    }

    #[test]
    fn bad_input_is_an_error() {
        assert!(parse_gltf(b"", None, &INCHES).is_err());
        assert!(parse_gltf(b"{}", None, &INCHES).is_err());
        assert!(parse_gltf(b"glTF\x02\0\0\0\x0c\0\0\0", None, &INCHES).is_err());
        let mut v1 = quad_json(Some(data_uri()), json!({ "mesh": 0 }), 4);
        v1["asset"]["version"] = json!("1.0");
        assert!(parse_gltf(&serde_json::to_vec(&v1).unwrap(), None, &INCHES).is_err());
        // A node loop does not hang.
        let mut looped = quad_json(Some(data_uri()), json!({ "mesh": 0, "children": [0] }), 4);
        looped["nodes"][0]["children"] = json!([0]);
        assert!(parse_gltf(&serde_json::to_vec(&looped).unwrap(), None, &INCHES).is_err());
        // An accessor past the buffer end.
        let mut short = quad_json(Some(data_uri()), json!({ "mesh": 0 }), 4);
        short["accessors"][0]["count"] = json!(1000);
        assert!(parse_gltf(&serde_json::to_vec(&short).unwrap(), None, &INCHES).is_err());
    }
}
