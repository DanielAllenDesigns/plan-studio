//! glTF 2.0 export: one buffer, one primitive per mesh, one material per [`Material`].

use crate::builder::V3;
use crate::mesh::{Material, Mesh, Scene};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const FLOAT: u32 = 5126;
const UNSIGNED_INT: u32 = 5125;
const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;

/// Default `.bin` file name used by [`export_gltf`].
const DEFAULT_BIN_URI: &str = "scene.bin";

/// Accumulates the binary buffer plus the glTF bufferViews and accessors.
#[derive(Default)]
struct Packer {
    bin: Vec<u8>,
    buffer_views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Packer {
    /// Append `bytes` as a bufferView and return its index.
    fn view(&mut self, bytes: &[u8], target: u32) -> usize {
        self.buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": self.bin.len(),
            "byteLength": bytes.len(),
            "target": target,
        }));
        self.bin.extend_from_slice(bytes);
        self.buffer_views.len() - 1
    }

    fn accessor(&mut self, mut accessor: Value, view: usize) -> usize {
        accessor["bufferView"] = json!(view);
        self.accessors.push(accessor);
        self.accessors.len() - 1
    }

    fn floats(
        &mut self,
        data: &[f32],
        count: usize,
        kind: &str,
        bounds: Option<(V3, V3)>,
    ) -> usize {
        let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();
        let view = self.view(&bytes, ARRAY_BUFFER);
        let mut acc = json!({ "componentType": FLOAT, "count": count, "type": kind });
        if let Some((min, max)) = bounds {
            acc["min"] = json!(min);
            acc["max"] = json!(max);
        }
        self.accessor(acc, view)
    }

    fn indices(&mut self, data: &[u32]) -> usize {
        let bytes: Vec<u8> = data.iter().flat_map(|i| i.to_le_bytes()).collect();
        let view = self.view(&bytes, ELEMENT_ARRAY_BUFFER);
        let acc = json!({ "componentType": UNSIGNED_INT, "count": data.len(), "type": "SCALAR" });
        self.accessor(acc, view)
    }

    /// Pack one mesh and return its glTF mesh object.
    fn mesh(&mut self, mesh: &Mesh) -> Value {
        let n = mesh.vertices.len();
        let flat = |f: fn(&crate::Vertex) -> Vec<f32>| -> Vec<f32> {
            mesh.vertices.iter().flat_map(f).collect()
        };
        let bounds = mesh.bounds().expect("packed meshes are non-empty");
        let position = self.floats(&flat(|v| v.position.to_vec()), n, "VEC3", Some(bounds));
        let normal = self.floats(&flat(|v| v.normal.to_vec()), n, "VEC3", None);
        let uv = self.floats(&flat(|v| v.uv.to_vec()), n, "VEC2", None);
        let indices = self.indices(&mesh.indices);
        json!({
            "primitives": [{
                "attributes": { "POSITION": position, "NORMAL": normal, "TEXCOORD_0": uv },
                "indices": indices,
                "material": mesh.material.index(),
            }]
        })
    }
}

fn material_json(material: &Material) -> Value {
    let mut m = json!({
        "name": material.name(),
        "pbrMetallicRoughness": {
            "baseColorFactor": material.color(),
            "metallicFactor": 0.0,
            "roughnessFactor": 0.9,
        },
    });
    if material.color()[3] < 1.0 {
        m["alphaMode"] = json!("BLEND");
        m["doubleSided"] = json!(true);
    }
    m
}

fn node_name(mesh: &Mesh, index: usize) -> String {
    match mesh.object_id {
        Some(id) => format!("{}_{id}", mesh.material.name()),
        None => format!("{}_{index}", mesh.material.name()),
    }
}

/// Build the `.gltf` JSON and `.bin` bytes, pointing the buffer at `bin_uri`.
fn export_with_uri(scene: &Scene, bin_uri: &str) -> (String, Vec<u8>) {
    let mut packer = Packer::default();
    let mut meshes = Vec::new();
    let mut nodes = Vec::new();
    for (i, mesh) in scene
        .meshes
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.indices.is_empty())
    {
        nodes.push(json!({ "name": node_name(mesh, i), "mesh": meshes.len() }));
        meshes.push(packer.mesh(mesh));
    }
    let mut root = json!({
        "asset": { "version": "2.0", "generator": "plan-3d" },
        "scene": 0,
        "scenes": [{ "nodes": (0..nodes.len()).collect::<Vec<_>>() }],
        "nodes": nodes,
        "meshes": meshes,
        "materials": Material::ALL.iter().map(material_json).collect::<Vec<_>>(),
    });
    if !packer.bin.is_empty() {
        root["buffers"] = json!([{ "uri": bin_uri, "byteLength": packer.bin.len() }]);
        root["bufferViews"] = json!(packer.buffer_views);
        root["accessors"] = json!(packer.accessors);
    }
    let text = serde_json::to_string_pretty(&root).expect("glTF JSON is serializable");
    (text, packer.bin)
}

/// Export `scene` as glTF 2.0: the `.gltf` JSON text and the `.bin` buffer.
///
/// The JSON refers to the buffer as `scene.bin`; use [`write_gltf_files`] to
/// get a URI matching the file it writes.
pub fn export_gltf(scene: &Scene) -> (String, Vec<u8>) {
    export_with_uri(scene, DEFAULT_BIN_URI)
}

/// `path` with `suffix` appended to the file name (keeps dots in the stem).
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut os = path.as_os_str().to_owned();
    os.push(suffix);
    PathBuf::from(os)
}

/// Write `<path>.gltf` and `<path>.bin`; the JSON's buffer URI is the `.bin` file name.
pub fn write_gltf_files(scene: &Scene, path_without_ext: impl AsRef<Path>) -> std::io::Result<()> {
    let base = path_without_ext.as_ref();
    let stem = base.file_name().and_then(|n| n.to_str()).unwrap_or("scene");
    let (json, bin) = export_with_uri(scene, &format!("{stem}.bin"));
    std::fs::write(with_suffix(base, ".gltf"), json)?;
    std::fs::write(with_suffix(base, ".bin"), bin)
}
