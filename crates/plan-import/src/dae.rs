//! COLLADA (`.dae`) reader for 3D library symbols, the format SketchUp,
//! Blender and most 3D warehouses export.
//!
//! Reads, with the crate's own XML reader ([`crate::xml`]):
//!
//! * geometry: `<triangles>`, `<polylist>` and `<polygons>` (polygons are
//!   ear-clipped like OBJ faces), with their `POSITION` source reached
//!   through `<vertices>`;
//! * the scene: the visual scene's node tree with `<matrix>`, `<translate>`,
//!   `<rotate>` and `<scale>`, `<instance_geometry>` and `<instance_node>`;
//!   a file without a scene places every geometry once;
//! * materials: `<instance_material>` bindings, the effect's diffuse color
//!   and diffuse texture (resolved through the effect's sampler and surface
//!   to the `<image>` file name);
//! * `<asset>`: `<unit meter=...>` and `<up_axis>`, see [`declared`].
//!
//! An `X_UP` file is turned to Y up while reading. The remaining unit and up
//! axis conversion is `ModelOptions`, whose defaults for a `.dae` are the
//! file's own ([`declared`]).

use crate::model::{
    ImportedModel, ImportedPart, ModelError, ModelOptions, UpAxis, INCHES_PER_METER,
};
use crate::obj::triangulate_polygon;
use crate::xml::{self, Node};
use std::collections::HashMap;

/// What the `<asset>` block of a COLLADA file says.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Declared {
    /// Meters per file unit (`<unit meter=...>`, 1 when absent).
    pub meter: f64,
    /// The up axis, with `X_UP` reported as [`UpAxis::Y`] (the reader turns
    /// it).
    pub up_axis: UpAxis,
    /// True when the file said `X_UP`.
    pub x_up: bool,
}

/// The unit and up axis a COLLADA file declares. `None` when the text is not
/// readable XML.
pub fn declared(bytes: &[u8]) -> Option<Declared> {
    let root = xml::parse(&String::from_utf8_lossy(bytes)).ok()?;
    Some(declared_of(&root))
}

fn declared_of(root: &Node) -> Declared {
    let asset = root.child("asset");
    let meter = asset
        .and_then(|a| a.child("unit"))
        .and_then(|u| u.attr("meter"))
        .and_then(|m| m.trim().parse::<f64>().ok())
        .filter(|m| *m > 0.0 && m.is_finite())
        .unwrap_or(1.0);
    let up = asset
        .and_then(|a| a.child("up_axis"))
        .map(|u| u.text.trim().to_ascii_uppercase())
        .unwrap_or_default();
    Declared {
        meter,
        up_axis: if up == "Z_UP" { UpAxis::Z } else { UpAxis::Y },
        x_up: up == "X_UP",
    }
}

impl Declared {
    /// The import options that match the file: its unit in inches and its up
    /// axis.
    pub fn options(&self) -> ModelOptions {
        ModelOptions {
            unit_scale: self.meter * INCHES_PER_METER,
            up_axis: self.up_axis,
        }
    }
}

type Mat = [f64; 16];

const IDENTITY: Mat = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

/// `a * b` for row-major 4x4 matrices (apply `b` first, then `a`).
fn mul(a: &Mat, b: &Mat) -> Mat {
    let mut o = [0.0; 16];
    for r in 0..4 {
        for c in 0..4 {
            o[r * 4 + c] = (0..4).map(|k| a[r * 4 + k] * b[k * 4 + c]).sum();
        }
    }
    o
}

fn apply(m: &Mat, p: [f32; 3]) -> [f32; 3] {
    let (x, y, z) = (f64::from(p[0]), f64::from(p[1]), f64::from(p[2]));
    let t = |r: usize| (m[r * 4] * x + m[r * 4 + 1] * y + m[r * 4 + 2] * z + m[r * 4 + 3]) as f32;
    [t(0), t(1), t(2)]
}

fn det3(m: &Mat) -> f64 {
    m[0] * (m[5] * m[10] - m[6] * m[9]) - m[1] * (m[4] * m[10] - m[6] * m[8])
        + m[2] * (m[4] * m[9] - m[5] * m[8])
}

/// The matrix of a node's transform elements, in document order.
fn node_matrix(n: &Node) -> Mat {
    let mut m = IDENTITY;
    for c in &n.children {
        let v = c.floats();
        let t = match c.name.as_str() {
            "matrix" if v.len() >= 16 => {
                let mut a = [0.0; 16];
                a.copy_from_slice(&v[..16]);
                a
            }
            "translate" if v.len() >= 3 => {
                let mut a = IDENTITY;
                a[3] = v[0];
                a[7] = v[1];
                a[11] = v[2];
                a
            }
            "scale" if v.len() >= 3 => {
                let mut a = IDENTITY;
                a[0] = v[0];
                a[5] = v[1];
                a[10] = v[2];
                a
            }
            "rotate" if v.len() >= 4 => {
                let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                if len < 1e-12 {
                    continue;
                }
                let (x, y, z) = (v[0] / len, v[1] / len, v[2] / len);
                let (s, co) = v[3].to_radians().sin_cos();
                let k = 1.0 - co;
                [
                    co + x * x * k,
                    x * y * k - z * s,
                    x * z * k + y * s,
                    0.0,
                    y * x * k + z * s,
                    co + y * y * k,
                    y * z * k - x * s,
                    0.0,
                    z * x * k - y * s,
                    z * y * k + x * s,
                    co + z * z * k,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                ]
            }
            _ => continue,
        };
        m = mul(&m, &t);
    }
    m
}

/// `#id` -> `id`.
fn target(url: &str) -> &str {
    url.trim().trim_start_matches('#')
}

struct Doc<'a> {
    root: &'a Node,
    by_id: HashMap<&'a str, &'a Node>,
}

impl<'a> Doc<'a> {
    fn new(root: &'a Node) -> Self {
        let mut by_id = HashMap::new();
        fn walk<'a>(n: &'a Node, map: &mut HashMap<&'a str, &'a Node>) {
            if let Some(id) = n.attr("id") {
                map.entry(id).or_insert(n);
            }
            for c in &n.children {
                walk(c, map);
            }
        }
        walk(root, &mut by_id);
        Doc { root, by_id }
    }

    fn get(&self, url: &str) -> Option<&'a Node> {
        self.by_id.get(target(url)).copied()
    }
}

/// A surface look: diffuse color and texture file.
#[derive(Debug, Clone, Default)]
struct Look {
    color: Option<[u8; 3]>,
    texture: Option<String>,
    name: String,
}

/// sRGB byte of a COLLADA color component (taken as already display-coded,
/// like the OBJ `Kd`).
fn byte(c: f64) -> u8 {
    (c.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn file_name(path: &str) -> String {
    let p = path.trim().trim_start_matches("file://");
    p.rsplit(['/', '\\']).next().unwrap_or(p).to_string()
}

/// The image file a texture reference of `effect` ends at.
fn texture_file(doc: &Doc, effect: &Node, texture: &str) -> Option<String> {
    let find_param = |sid: &str| -> Option<&Node> {
        let mut all = Vec::new();
        effect.find_all("newparam", &mut all);
        all.into_iter().find(|n| n.attr("sid") == Some(sid))
    };
    // texture -> sampler2D/source -> surface/init_from -> image
    let mut image_id = texture.to_string();
    if let Some(sampler) = find_param(texture) {
        if let Some(src) = sampler.find("source") {
            let sid = src.text.trim();
            if let Some(surface) = find_param(sid) {
                if let Some(init) = surface.find("init_from") {
                    image_id = init.text.trim().to_string();
                }
            }
        }
    }
    let img = doc.get(&image_id)?;
    let init = img.child("init_from")?;
    let text = init
        .child("ref")
        .map(|r| r.text.as_str())
        .unwrap_or(init.text.as_str());
    let f = file_name(text);
    (!f.is_empty()).then_some(f)
}

fn look_of_material(doc: &Doc, material_id: &str) -> Look {
    let mut look = Look::default();
    let Some(mat) = doc.get(material_id) else {
        return look;
    };
    look.name = mat
        .attr("name")
        .or_else(|| mat.attr("id"))
        .unwrap_or("")
        .to_string();
    let Some(effect) = mat
        .child("instance_effect")
        .and_then(|i| i.attr("url"))
        .and_then(|u| doc.get(u))
    else {
        return look;
    };
    // The diffuse (or constant/emission as a fallback) of the shader.
    for tag in ["diffuse", "emission"] {
        let Some(d) = effect.find(tag) else { continue };
        if let Some(col) = d.child("color") {
            let v = col.floats();
            if v.len() >= 3 && look.color.is_none() {
                look.color = Some([byte(v[0]), byte(v[1]), byte(v[2])]);
            }
        }
        if let Some(t) = d.child("texture").and_then(|t| t.attr("texture")) {
            if look.texture.is_none() {
                look.texture = texture_file(doc, effect, t);
            }
        }
        if look.color.is_some() || look.texture.is_some() {
            break;
        }
    }
    look
}

/// One geometry's mesh: raw triangles per material symbol.
struct GeomPart {
    /// The `material` symbol of the primitive.
    symbol: Option<String>,
    positions: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

fn source_floats(mesh: &Node, id: &str) -> Option<(Vec<f64>, usize)> {
    let src = mesh
        .children_named("source")
        .find(|s| s.attr("id") == Some(id))?;
    let arr = src.child("float_array")?;
    let stride = src
        .find("accessor")
        .and_then(|a| a.attr("stride"))
        .and_then(|s| s.parse().ok())
        .unwrap_or(3)
        .max(1);
    Some((arr.floats(), stride))
}

fn geometry_parts(geom: &Node) -> Vec<GeomPart> {
    let Some(mesh) = geom.child("mesh") else {
        return Vec::new();
    };
    // <vertices id=...> -> POSITION source.
    let vertex_pos: HashMap<&str, &str> = mesh
        .children_named("vertices")
        .filter_map(|v| {
            let id = v.attr("id")?;
            let pos = v
                .children_named("input")
                .find(|i| i.attr("semantic") == Some("POSITION"))?
                .attr("source")?;
            Some((id, target(pos)))
        })
        .collect();
    let mut out = Vec::new();
    for prim in &mesh.children {
        if !matches!(prim.name.as_str(), "triangles" | "polylist" | "polygons") {
            continue;
        }
        let inputs: Vec<&Node> = prim.children_named("input").collect();
        let stride = inputs
            .iter()
            .filter_map(|i| i.attr("offset").and_then(|o| o.parse::<usize>().ok()))
            .max()
            .map_or(1, |m| m + 1);
        let Some(vin) = inputs.iter().find(|i| i.attr("semantic") == Some("VERTEX")) else {
            continue;
        };
        let voff: usize = vin.attr("offset").and_then(|o| o.parse().ok()).unwrap_or(0);
        let vsrc = target(vin.attr("source").unwrap_or(""));
        let pos_id = vertex_pos.get(vsrc).copied().unwrap_or(vsrc);
        let Some((floats, pstride)) = source_floats(mesh, pos_id) else {
            continue;
        };
        let verts: Vec<[f32; 3]> = floats
            .chunks(pstride)
            .filter(|c| c.len() >= 3)
            .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32])
            .collect();

        // Polygons as lists of vertex indices.
        let mut polys: Vec<Vec<usize>> = Vec::new();
        let corner = |p: &[usize], k: usize| p.get(k * stride + voff).copied();
        match prim.name.as_str() {
            "triangles" => {
                let p: Vec<usize> = prim.children_named("p").flat_map(|p| p.ints()).collect();
                for t in 0..p.len() / stride / 3 {
                    polys.push((0..3).filter_map(|k| corner(&p, t * 3 + k)).collect());
                }
            }
            "polylist" => {
                let vc = prim.child("vcount").map(Node::ints).unwrap_or_default();
                let p = prim.child("p").map(Node::ints).unwrap_or_default();
                let mut at = 0usize;
                for n in vc {
                    polys.push((0..n).filter_map(|k| corner(&p, at + k)).collect());
                    at += n;
                }
            }
            _ => {
                for pn in prim.children_named("p") {
                    let p = pn.ints();
                    polys.push(
                        (0..p.len() / stride)
                            .filter_map(|k| corner(&p, k))
                            .collect(),
                    );
                }
            }
        }
        let mut part = GeomPart {
            symbol: prim.attr("material").map(str::to_string),
            positions: Vec::new(),
            indices: Vec::new(),
        };
        let mut remap: HashMap<usize, u32> = HashMap::new();
        for poly in polys {
            if poly.len() < 3 || poly.iter().any(|&i| i >= verts.len()) {
                continue;
            }
            let pts: Vec<[f32; 3]> = poly.iter().map(|&i| verts[i]).collect();
            for tri in triangulate_polygon(&pts) {
                for k in tri {
                    let g = poly[k];
                    let l = *remap.entry(g).or_insert_with(|| {
                        part.positions.push(verts[g]);
                        (part.positions.len() - 1) as u32
                    });
                    part.indices.push(l);
                }
            }
        }
        if !part.indices.is_empty() {
            out.push(part);
        }
    }
    out
}

struct Builder<'a> {
    doc: &'a Doc<'a>,
    geoms: HashMap<&'a str, Vec<GeomPart>>,
    parts: Vec<ImportedPart>,
    depth: usize,
}

impl<'a> Builder<'a> {
    fn instance_geometry(&mut self, inst: &'a Node, m: &Mat) {
        let Some(url) = inst.attr("url") else { return };
        // symbol -> material id
        let mut bind: HashMap<&str, &str> = HashMap::new();
        let mut ims = Vec::new();
        inst.find_all("instance_material", &mut ims);
        for im in ims {
            if let (Some(s), Some(t)) = (im.attr("symbol"), im.attr("target")) {
                bind.insert(s, target(t));
            }
        }
        self.place(url, &bind, m);
    }

    /// Adds the parts of the geometry at `url`, moved by `m`; `bind` maps a
    /// primitive's material symbol to a material id.
    fn place(&mut self, url: &str, bind: &HashMap<&str, &str>, m: &Mat) {
        let Some(geom) = self.doc.get(url) else {
            return;
        };
        let gid = geom.attr("id").unwrap_or("");
        if !self.geoms.contains_key(gid) {
            self.geoms.insert(gid, geometry_parts(geom));
        }
        let name = geom
            .attr("name")
            .or_else(|| geom.attr("id"))
            .unwrap_or("")
            .to_string();
        let flip = det3(m) < 0.0;
        for gp in &self.geoms[gid] {
            let look = gp
                .symbol
                .as_deref()
                .map(|s| look_of_material(self.doc, bind.get(s).copied().unwrap_or(s)))
                .unwrap_or_default();
            let mut indices = gp.indices.clone();
            if flip {
                // A mirroring transform reverses the winding.
                for t in indices.as_chunks_mut::<3>().0 {
                    t.swap(1, 2);
                }
            }
            self.parts.push(ImportedPart {
                name: name.clone(),
                color: look.color,
                material: (!look.name.is_empty()).then_some(look.name),
                texture: look.texture,
                positions: gp.positions.iter().map(|p| apply(m, *p)).collect(),
                indices,
            });
        }
    }

    fn node(&mut self, n: &'a Node, parent: &Mat) {
        if self.depth > 64 {
            return;
        }
        self.depth += 1;
        let m = mul(parent, &node_matrix(n));
        for c in &n.children {
            match c.name.as_str() {
                "instance_geometry" => self.instance_geometry(c, &m),
                "instance_node" => {
                    if let Some(t) = c.attr("url").and_then(|u| self.doc.get(u)) {
                        self.node(t, &m);
                    }
                }
                "node" => self.node(c, &m),
                _ => {}
            }
        }
        self.depth -= 1;
    }
}

/// Parses a COLLADA file. Fails when it holds no triangles. `opts` converts
/// the result (see [`declared`] for the file's own values).
pub fn parse_dae(bytes: &[u8], opts: &ModelOptions) -> Result<ImportedModel, ModelError> {
    let text = String::from_utf8_lossy(bytes);
    let root = xml::parse(&text)?;
    if root.name != "COLLADA" {
        return Err(ModelError("Not a COLLADA file".into()));
    }
    let doc = Doc::new(&root);
    let decl = declared_of(&root);
    let mut b = Builder {
        doc: &doc,
        geoms: HashMap::new(),
        parts: Vec::new(),
        depth: 0,
    };
    // Rotates an X_UP file to Y up: (x, y, z) -> (-y, x, z).
    let base: Mat = if decl.x_up {
        [
            0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]
    } else {
        IDENTITY
    };
    let scene_node = doc
        .root
        .child("scene")
        .and_then(|s| s.child("instance_visual_scene"))
        .and_then(|i| i.attr("url"))
        .and_then(|u| doc.get(u));
    match scene_node {
        Some(vs) => b.node(vs, &base),
        None => {
            // No scene: every geometry once, untransformed.
            let mut geoms = Vec::new();
            doc.root.find_all("geometry", &mut geoms);
            for g in geoms {
                b.place(g.attr("id").unwrap_or(""), &HashMap::new(), &base);
            }
        }
    }
    if b.parts.is_empty() {
        return Err(ModelError("The COLLADA file has no triangles".into()));
    }
    let model = ImportedModel { parts: b.parts }.cleaned().converted(opts);
    if model.is_empty() {
        return Err(ModelError(
            "The COLLADA file has no usable triangles".into(),
        ));
    }
    Ok(model)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A COLLADA text of a box `w x d x h` (Z up, in `meter` meters per
    /// unit) built from one `<polylist>` of quads with a red material that
    /// has a texture, instanced twice: once at the origin and once moved by
    /// `shift` along x.
    pub(crate) fn box_dae(w: f64, d: f64, h: f64, meter: f64, up: &str, shift: f64) -> String {
        let v = [
            [0.0, 0.0, 0.0],
            [w, 0.0, 0.0],
            [w, d, 0.0],
            [0.0, d, 0.0],
            [0.0, 0.0, h],
            [w, 0.0, h],
            [w, d, h],
            [0.0, d, h],
        ];
        let pos: Vec<String> = v.iter().flatten().map(|c| c.to_string()).collect();
        let quads = "3 2 1 0  4 5 6 7  0 1 5 4  1 2 6 5  2 3 7 6  3 0 4 7";
        let second = if shift != 0.0 {
            format!(
                "<node id=\"n2\"><translate>{shift} 0 0</translate><instance_geometry url=\"#g\"/></node>"
            )
        } else {
            String::new()
        };
        format!(
            r##"<?xml version="1.0" encoding="utf-8"?>
<COLLADA xmlns="http://www.collada.org/2005/11/COLLADASchema" version="1.4.1">
 <asset><unit meter="{meter}" name="unit"/><up_axis>{up}</up_axis></asset>
 <library_images><image id="img1"><init_from>textures/lid_oak.jpg</init_from></image></library_images>
 <library_effects><effect id="eff1"><profile_COMMON>
   <newparam sid="surf"><surface type="2D"><init_from>img1</init_from></surface></newparam>
   <newparam sid="samp"><sampler2D><source>surf</source></sampler2D></newparam>
   <technique sid="common"><phong><diffuse><color>0.8 0.1 0.1 1</color></diffuse></phong></technique>
 </profile_COMMON></effect>
 <effect id="eff2"><profile_COMMON><technique sid="common"><lambert><diffuse><texture texture="samp" texcoord="uv"/></diffuse></lambert></technique>
   <newparam sid="surf"><surface type="2D"><init_from>img1</init_from></surface></newparam>
   <newparam sid="samp"><sampler2D><source>surf</source></sampler2D></newparam></profile_COMMON></effect></library_effects>
 <library_materials><material id="mat1" name="Red"><instance_effect url="#eff1"/></material>
   <material id="mat2" name="Oak"><instance_effect url="#eff2"/></material></library_materials>
 <library_geometries><geometry id="g" name="Crate"><mesh>
  <source id="g-pos"><float_array id="g-pos-a" count="24">{pos}</float_array>
   <technique_common><accessor source="#g-pos-a" count="8" stride="3"/></technique_common></source>
  <vertices id="g-v"><input semantic="POSITION" source="#g-pos"/></vertices>
  <polylist material="m1" count="6"><input semantic="VERTEX" source="#g-v" offset="0"/>
   <vcount>4 4 4 4 4 4</vcount><p>{quads}</p></polylist>
 </mesh></geometry></library_geometries>
 <library_visual_scenes><visual_scene id="vs"><node id="n1"><instance_geometry url="#g"><bind_material><technique_common>
   <instance_material symbol="m1" target="#mat1"/></technique_common></bind_material></instance_geometry></node>{second}</visual_scene></library_visual_scenes>
 <scene><instance_visual_scene url="#vs"/></scene>
</COLLADA>"##,
            pos = pos.join(" ")
        )
    }

    #[test]
    fn reads_quads_material_and_declared_units() {
        let text = box_dae(10.0, 20.0, 30.0, 0.0254, "Z_UP", 0.0);
        let d = declared(text.as_bytes()).unwrap();
        assert_eq!(d.up_axis, UpAxis::Z);
        assert!((d.options().unit_scale - 1.0).abs() < 1e-6);
        let m = parse_dae(text.as_bytes(), &d.options()).unwrap();
        // 6 quads -> 12 triangles; Z up: height is y.
        assert_eq!(m.triangle_count(), 12);
        let e = m.extent().unwrap();
        assert!(
            (e[0] - 10.0).abs() < 1e-3 && (e[1] - 30.0).abs() < 1e-3 && (e[2] - 20.0).abs() < 1e-3,
            "{e:?}"
        );
        let p = &m.parts[0];
        assert_eq!(p.name, "Crate");
        assert_eq!(p.material.as_deref(), Some("Red"));
        assert_eq!(p.color, Some([204, 26, 26]));
    }

    #[test]
    fn node_transforms_place_each_instance() {
        let text = box_dae(10.0, 10.0, 10.0, 0.0254, "Y_UP", 25.0);
        let m = parse_dae(
            text.as_bytes(),
            &declared(text.as_bytes()).unwrap().options(),
        )
        .unwrap();
        assert_eq!(m.parts.len(), 2);
        assert_eq!(m.triangle_count(), 24);
        assert!((m.extent().unwrap()[0] - 35.0).abs() < 1e-3);
    }

    #[test]
    fn texture_resolves_through_sampler_and_surface() {
        let text = box_dae(1.0, 1.0, 1.0, 1.0, "Y_UP", 0.0).replace(
            "symbol=\"m1\" target=\"#mat1\"",
            "symbol=\"m1\" target=\"#mat2\"",
        );
        let m = parse_dae(text.as_bytes(), &ModelOptions::default()).unwrap();
        assert_eq!(m.parts[0].texture.as_deref(), Some("lid_oak.jpg"));
        assert_eq!(m.parts[0].material.as_deref(), Some("Oak"));
    }

    #[test]
    fn meter_units_and_x_up() {
        // meter = 1: a 1 x 2 x 3 box in meters.
        let text = box_dae(1.0, 2.0, 3.0, 1.0, "X_UP", 0.0);
        let d = declared(text.as_bytes()).unwrap();
        assert!(d.x_up);
        let m = parse_dae(text.as_bytes(), &d.options()).unwrap();
        let e = m.extent().unwrap();
        // X up: the 1 m x extent is the height; y (2 m) and z (3 m) stay.
        assert!((e[1] - 39.370_08).abs() < 0.01, "{e:?}");
        assert!((e[0] - 78.740_16).abs() < 0.01, "{e:?}");
    }

    #[test]
    fn triangles_and_junk() {
        let tri = r##"<COLLADA><library_geometries><geometry id="g"><mesh>
 <source id="p"><float_array id="pa" count="9">0 0 0 4 0 0 0 0 4</float_array></source>
 <vertices id="v"><input semantic="POSITION" source="#p"/></vertices>
 <triangles count="1"><input semantic="VERTEX" source="#v" offset="0"/><p>0 1 2</p></triangles>
 </mesh></geometry></library_geometries></COLLADA>"##;
        // No scene: the geometry is placed once.
        let m = parse_dae(tri.as_bytes(), &ModelOptions::default()).unwrap();
        assert_eq!(m.triangle_count(), 1);
        assert!(parse_dae(b"<html/>", &ModelOptions::default()).is_err());
        assert!(parse_dae(b"binary\x00\x01", &ModelOptions::default()).is_err());
        assert!(parse_dae(b"<COLLADA/>", &ModelOptions::default()).is_err());
    }
}
