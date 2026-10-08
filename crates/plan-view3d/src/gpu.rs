//! All OpenGL code lives here (via `eframe::glow`). Nothing in this module can
//! be unit tested without a GL context; keep it thin and put logic elsewhere.
//!
//! Every function taking a `glow::Context` must be called on the thread that
//! owns the GL context (inside an egui paint callback, or with eframe's
//! `CreationContext::gl`).

use eframe::egui_glow::ShaderVersion;
use eframe::glow::{self, HasContext as _};
use plan_3d::{Material, Scene};

use crate::edges::unique_edges;
use crate::math::{self, Mat4, Vec3};
use crate::Lighting;

const VERTEX_SHADER: &str = r#"
#ifdef GL_ES
precision highp float;
#endif
uniform mat4 u_mvp;
in vec3 a_pos;
in vec3 a_normal;
in vec2 a_uv;
out vec3 v_pos;
out vec3 v_normal;
out vec2 v_uv;
void main() {
    v_pos = a_pos;
    v_normal = a_normal;
    v_uv = a_uv;
    gl_Position = u_mvp * vec4(a_pos, 1.0);
}
"#;

const FRAGMENT_SHADER: &str = r#"
#ifdef GL_ES
precision highp float;
#endif
uniform vec4 u_color;
uniform vec3 u_key_dir;   // unit vector pointing toward the key light
uniform vec3 u_fill_dir;  // unit vector pointing toward the fill light
uniform float u_ambient;
uniform float u_key;
uniform vec3 u_eye;
uniform vec3 u_view_dir;  // unit view direction (used when orthographic)
uniform int u_ortho;
uniform int u_unlit;
in vec3 v_pos;
in vec3 v_normal;
in vec2 v_uv;
out vec4 f_color;
void main() {
    if (u_unlit == 1) {
        f_color = u_color;
        return;
    }
    vec3 to_eye = (u_ortho == 1) ? -u_view_dir : normalize(u_eye - v_pos);
    vec3 n = normalize(v_normal);
    // Two-sided lighting: always shade the side that faces the camera.
    if (dot(n, to_eye) < 0.0) {
        n = -n;
    }
    float lambert = max(dot(n, u_key_dir), 0.0) * u_key
                  + max(dot(n, u_fill_dir), 0.0) * u_key * 0.35;
    float lit = min(u_ambient + lambert, 1.0);
    // Material colors are linear; the egui framebuffer is not sRGB-encoded.
    vec3 rgb = pow(u_color.rgb * lit, vec3(1.0 / 2.2));
    f_color = vec4(rgb, u_color.a);
}
"#;

/// Everything the paint callback needs for one frame (plain data, no GL).
#[derive(Clone, Debug)]
pub(crate) struct FrameParams {
    pub view_proj: Mat4,
    pub eye: Vec3,
    pub view_dir: Vec3,
    pub ortho: bool,
    pub lighting: Lighting,
    pub show_edges: bool,
    pub hide_ceiling_roof: bool,
}

struct Uniforms {
    mvp: Option<glow::UniformLocation>,
    color: Option<glow::UniformLocation>,
    key_dir: Option<glow::UniformLocation>,
    fill_dir: Option<glow::UniformLocation>,
    ambient: Option<glow::UniformLocation>,
    key: Option<glow::UniformLocation>,
    eye: Option<glow::UniformLocation>,
    view_dir: Option<glow::UniformLocation>,
    ortho: Option<glow::UniformLocation>,
    unlit: Option<glow::UniformLocation>,
}

struct GpuProgram {
    program: glow::Program,
    uniforms: Uniforms,
}

struct GpuMesh {
    material: Material,
    color: [f32; 4],
    centroid: Vec3,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    ebo: glow::Buffer,
    index_count: i32,
    edge_vao: Option<glow::VertexArray>,
    edge_vbo: Option<glow::Buffer>,
    edge_vertex_count: i32,
}

impl GpuMesh {
    fn is_transparent(&self) -> bool {
        self.color[3] < 0.999
    }
}

/// GL resources for one uploaded scene plus the shader program.
#[derive(Default)]
pub(crate) struct GpuScene {
    program: Option<GpuProgram>,
    meshes: Vec<GpuMesh>,
    error: Option<String>,
    program_tried: bool,
}

fn as_bytes<T: Copy>(data: &[T], to_bytes: impl Fn(T) -> [u8; 4]) -> Vec<u8> {
    data.iter().flat_map(|v| to_bytes(*v)).collect()
}

fn compile(
    gl: &glow::Context,
    version: &str,
    kind: u32,
    src: &str,
) -> Result<glow::Shader, String> {
    unsafe {
        let shader = gl.create_shader(kind)?;
        gl.shader_source(shader, &format!("{version}{src}"));
        gl.compile_shader(shader);
        if gl.get_shader_compile_status(shader) {
            Ok(shader)
        } else {
            let log = gl.get_shader_info_log(shader);
            gl.delete_shader(shader);
            Err(format!("shader compile failed: {log}"))
        }
    }
}

fn build_program(gl: &glow::Context) -> Result<GpuProgram, String> {
    let version = ShaderVersion::get(gl);
    if !version.is_new_shader_interface() {
        return Err(format!(
            "OpenGL shader version {version:?} is too old for plan-view3d (needs GLSL 1.40 or ES 3.00)"
        ));
    }
    let decl = version.version_declaration();
    unsafe {
        let vs = compile(gl, decl, glow::VERTEX_SHADER, VERTEX_SHADER)?;
        let fs = match compile(gl, decl, glow::FRAGMENT_SHADER, FRAGMENT_SHADER) {
            Ok(fs) => fs,
            Err(e) => {
                gl.delete_shader(vs);
                return Err(e);
            }
        };
        let program = gl.create_program()?;
        gl.attach_shader(program, vs);
        gl.attach_shader(program, fs);
        gl.bind_attrib_location(program, 0, "a_pos");
        gl.bind_attrib_location(program, 1, "a_normal");
        gl.bind_attrib_location(program, 2, "a_uv");
        gl.link_program(program);
        let linked = gl.get_program_link_status(program);
        gl.detach_shader(program, vs);
        gl.detach_shader(program, fs);
        gl.delete_shader(vs);
        gl.delete_shader(fs);
        if !linked {
            let log = gl.get_program_info_log(program);
            gl.delete_program(program);
            return Err(format!("program link failed: {log}"));
        }
        let loc = |name: &str| gl.get_uniform_location(program, name);
        let uniforms = Uniforms {
            mvp: loc("u_mvp"),
            color: loc("u_color"),
            key_dir: loc("u_key_dir"),
            fill_dir: loc("u_fill_dir"),
            ambient: loc("u_ambient"),
            key: loc("u_key"),
            eye: loc("u_eye"),
            view_dir: loc("u_view_dir"),
            ortho: loc("u_ortho"),
            unlit: loc("u_unlit"),
        };
        Ok(GpuProgram { program, uniforms })
    }
}

/// Create a VAO holding a new vertex buffer filled with `bytes`, with one
/// float attribute per `(location, components)` entry interleaved at `stride`
/// bytes, and optionally a new element buffer filled with `index_bytes`.
///
/// The VAO is bound before any buffer is touched so egui's own VAO state is
/// never modified. Returns `(vao, vbo, ebo)` with all bindings reset.
unsafe fn make_vao(
    gl: &glow::Context,
    bytes: &[u8],
    attribs: &[(u32, i32)],
    stride: i32,
    index_bytes: Option<&[u8]>,
) -> Result<(glow::VertexArray, glow::Buffer, Option<glow::Buffer>), String> {
    unsafe {
        let vao = gl.create_vertex_array()?;
        gl.bind_vertex_array(Some(vao));
        let vbo = gl.create_buffer()?;
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STATIC_DRAW);
        let mut offset = 0;
        for &(location, components) in attribs {
            gl.enable_vertex_attrib_array(location);
            gl.vertex_attrib_pointer_f32(location, components, glow::FLOAT, false, stride, offset);
            offset += components * 4;
        }
        let ebo = match index_bytes {
            Some(idx) => {
                let ebo = gl.create_buffer()?;
                // The element buffer binding is captured by the bound VAO.
                gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
                gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, idx, glow::STATIC_DRAW);
                Some(ebo)
            }
            None => None,
        };
        gl.bind_vertex_array(None);
        gl.bind_buffer(glow::ARRAY_BUFFER, None);
        Ok((vao, vbo, ebo))
    }
}

impl GpuScene {
    /// Message describing why GL setup failed, if it did.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Number of meshes currently resident on the GPU.
    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }

    /// Compile the shader program once (a failed attempt is not retried).
    pub fn ensure_program(&mut self, gl: &glow::Context) {
        if self.program.is_some() || self.program_tried {
            return;
        }
        self.program_tried = true;
        match build_program(gl) {
            Ok(p) => self.program = Some(p),
            Err(e) => self.error = Some(e),
        }
    }

    /// Replace the resident meshes with `scene`.
    pub fn upload(&mut self, gl: &glow::Context, scene: &Scene) {
        self.ensure_program(gl);
        self.free_meshes(gl);
        for mesh in &scene.meshes {
            if mesh.indices.is_empty() || mesh.vertices.is_empty() {
                continue;
            }
            let n = mesh.vertices.len() as u32;
            if mesh.indices.iter().any(|&i| i >= n) {
                continue; // malformed mesh: never hand GL out-of-range indices
            }
            match upload_mesh(gl, mesh) {
                Ok(m) => self.meshes.push(m),
                Err(e) => self.error = Some(e),
            }
        }
    }

    /// Draw the resident meshes into the current framebuffer region.
    ///
    /// `scissor` is `(x, y, w, h)` in framebuffer pixels (origin bottom-left);
    /// the depth buffer is cleared inside it before drawing.
    pub fn paint(&self, gl: &glow::Context, frame: &FrameParams, scissor: (i32, i32, i32, i32)) {
        let Some(prog) = &self.program else { return };
        let u = &prog.uniforms;
        let visible = |m: &&GpuMesh| {
            !(frame.hide_ceiling_roof && matches!(m.material, Material::Ceiling | Material::Roof))
        };
        let key_dir = math::normalize(frame.lighting.key_dir);
        // Fill light: weak, from the opposite horizontal side, a bit lower.
        let fill_dir = math::normalize([-key_dir[0], key_dir[1] * 0.4, -key_dir[2]]);

        unsafe {
            gl.scissor(scissor.0, scissor.1, scissor.2, scissor.3);
            gl.enable(glow::SCISSOR_TEST);
            gl.depth_mask(true);
            gl.clear_depth_f32(1.0);
            gl.clear(glow::DEPTH_BUFFER_BIT);

            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LEQUAL);
            gl.disable(glow::CULL_FACE);
            // Push filled faces back a hair so edge lines win the depth test.
            gl.enable(glow::POLYGON_OFFSET_FILL);
            gl.polygon_offset(1.0, 1.0);

            gl.use_program(Some(prog.program));
            gl.uniform_matrix_4_f32_slice(u.mvp.as_ref(), false, &frame.view_proj);
            gl.uniform_3_f32_slice(u.key_dir.as_ref(), &key_dir);
            gl.uniform_3_f32_slice(u.fill_dir.as_ref(), &fill_dir);
            gl.uniform_1_f32(u.ambient.as_ref(), frame.lighting.ambient);
            gl.uniform_1_f32(u.key.as_ref(), frame.lighting.key);
            gl.uniform_3_f32_slice(u.eye.as_ref(), &frame.eye);
            gl.uniform_3_f32_slice(u.view_dir.as_ref(), &frame.view_dir);
            gl.uniform_1_i32(u.ortho.as_ref(), frame.ortho as i32);
            gl.uniform_1_i32(u.unlit.as_ref(), 0);

            // Opaque pass.
            gl.disable(glow::BLEND);
            for m in self
                .meshes
                .iter()
                .filter(visible)
                .filter(|m| !m.is_transparent())
            {
                self.draw_fill(gl, u, m);
            }

            // Edge overlay (opaque and transparent meshes alike).
            if frame.show_edges {
                gl.disable(glow::POLYGON_OFFSET_FILL);
                gl.uniform_1_i32(u.unlit.as_ref(), 1);
                gl.uniform_4_f32(u.color.as_ref(), 0.16, 0.16, 0.17, 1.0);
                for m in self.meshes.iter().filter(visible) {
                    if let Some(vao) = m.edge_vao {
                        gl.bind_vertex_array(Some(vao));
                        gl.draw_arrays(glow::LINES, 0, m.edge_vertex_count);
                    }
                }
                gl.uniform_1_i32(u.unlit.as_ref(), 0);
                gl.enable(glow::POLYGON_OFFSET_FILL);
            }

            // Transparent pass, back to front by centroid distance from the eye.
            let mut glass: Vec<&GpuMesh> = self
                .meshes
                .iter()
                .filter(visible)
                .filter(|m| m.is_transparent())
                .collect();
            if !glass.is_empty() {
                let dist = |m: &GpuMesh| math::length(math::sub(m.centroid, frame.eye));
                glass.sort_by(|a, b| dist(b).total_cmp(&dist(a)));
                gl.enable(glow::BLEND);
                gl.blend_equation_separate(glow::FUNC_ADD, glow::FUNC_ADD);
                gl.blend_func_separate(
                    glow::SRC_ALPHA,
                    glow::ONE_MINUS_SRC_ALPHA,
                    glow::ONE,
                    glow::ONE_MINUS_SRC_ALPHA,
                );
                gl.depth_mask(false);
                for m in glass {
                    self.draw_fill(gl, u, m);
                }
                gl.depth_mask(true);
            }

            // Leave state the way egui_glow expects to find it.
            gl.disable(glow::POLYGON_OFFSET_FILL);
            gl.disable(glow::DEPTH_TEST);
            gl.bind_vertex_array(None);
            gl.use_program(None);
        }
    }

    unsafe fn draw_fill(&self, gl: &glow::Context, u: &Uniforms, m: &GpuMesh) {
        unsafe {
            gl.uniform_4_f32_slice(u.color.as_ref(), &m.color);
            gl.bind_vertex_array(Some(m.vao));
            gl.draw_elements(glow::TRIANGLES, m.index_count, glow::UNSIGNED_INT, 0);
        }
    }

    fn free_meshes(&mut self, gl: &glow::Context) {
        for m in self.meshes.drain(..) {
            unsafe {
                gl.delete_vertex_array(m.vao);
                gl.delete_buffer(m.vbo);
                gl.delete_buffer(m.ebo);
                if let Some(v) = m.edge_vao {
                    gl.delete_vertex_array(v);
                }
                if let Some(b) = m.edge_vbo {
                    gl.delete_buffer(b);
                }
            }
        }
    }

    /// Free every GL object. The scene can be re-uploaded afterwards.
    pub fn destroy(&mut self, gl: &glow::Context) {
        self.free_meshes(gl);
        if let Some(p) = self.program.take() {
            unsafe { gl.delete_program(p.program) };
        }
        self.program_tried = false;
    }
}

fn upload_mesh(gl: &glow::Context, mesh: &plan_3d::Mesh) -> Result<GpuMesh, String> {
    let mut flat: Vec<f32> = Vec::with_capacity(mesh.vertices.len() * 8);
    let mut sum = [0.0f64; 3];
    for v in &mesh.vertices {
        flat.extend_from_slice(&v.position);
        flat.extend_from_slice(&v.normal);
        flat.extend_from_slice(&v.uv);
        for (s, p) in sum.iter_mut().zip(v.position) {
            *s += f64::from(p);
        }
    }
    let n = mesh.vertices.len() as f64;
    let centroid = [
        (sum[0] / n) as f32,
        (sum[1] / n) as f32,
        (sum[2] / n) as f32,
    ];
    let vertex_bytes = as_bytes(&flat, f32::to_ne_bytes);
    let index_bytes = as_bytes(&mesh.indices, u32::to_ne_bytes);

    let edge_points: Vec<f32> = unique_edges(&mesh.vertices, &mesh.indices)
        .into_iter()
        .flat_map(|(a, b)| [a[0], a[1], a[2], b[0], b[1], b[2]])
        .collect();

    unsafe {
        let (vao, vbo, ebo) = make_vao(
            gl,
            &vertex_bytes,
            &[(0, 3), (1, 3), (2, 2)],
            32,
            Some(&index_bytes),
        )?;
        let ebo = ebo.ok_or("element buffer missing")?;

        let (edge_vao, edge_vbo) = if edge_points.is_empty() {
            (None, None)
        } else {
            let bytes = as_bytes(&edge_points, f32::to_ne_bytes);
            let (vao, vbo, _) = make_vao(gl, &bytes, &[(0, 3)], 12, None)?;
            (Some(vao), Some(vbo))
        };

        Ok(GpuMesh {
            material: mesh.material,
            color: mesh.material.color(),
            centroid,
            vao,
            vbo,
            ebo,
            index_count: mesh.indices.len() as i32,
            edge_vao,
            edge_vbo,
            edge_vertex_count: (edge_points.len() / 3) as i32,
        })
    }
}
