//! All OpenGL code lives here (via `eframe::glow`). Nothing in this module can
//! be unit tested without a GL context; keep it thin and put logic elsewhere
//! (`quality` holds the settings and math, `pipeline` the offscreen targets
//! and screen-space passes).
//!
//! Every function taking a `glow::Context` must be called on the thread that
//! owns the GL context (inside an egui paint callback, or with eframe's
//! `CreationContext::gl`).
//!
//! A frame is: shadow map (cached while the scene and sun are unchanged),
//! scene (sky, opaque meshes, edge lines, glass) into an offscreen target,
//! half-resolution ambient occlusion, then one composite pass onto the
//! window. Without the offscreen path (a driver that cannot make the
//! targets) the scene draws straight onto the window with shadows only.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use eframe::egui_glow::ShaderVersion;
use eframe::glow::{self, HasContext as _};
use plan_3d::{Bounds, Material, Scene};
use plan_core::Id;
use plan_materials::textures::TextureStore;

use crate::camera::Camera;
use crate::edges::unique_edges;
use crate::math::{self, Mat4, Vec3};
use crate::pipeline::{self, Prog, Targets};
use crate::quality::{
    self, Look, ShadowMap, ViewLight, ViewSettings, MAX_AO_SAMPLES, MAX_POINT_LIGHTS,
};
use crate::texturing::{self, ImageTexture};
use crate::Lighting;

/// `GL_TEXTURE_MAX_ANISOTROPY_EXT` and its limit query.
const TEXTURE_MAX_ANISOTROPY: u32 = 0x84FE;
const MAX_TEXTURE_MAX_ANISOTROPY: u32 = 0x84FF;
/// Anisotropic filtering requested when the driver offers it.
const WANT_ANISOTROPY: f32 = 8.0;

const VERTEX_SHADER: &str = r#"
#ifdef GL_ES
precision highp float;
#endif
uniform mat4 u_mvp;
uniform vec2 u_ndc_offset;  // shifts the picture by a fraction of the window (thick lines)
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
    gl_Position.xy += u_ndc_offset * gl_Position.w;
}
"#;

const FRAGMENT_SHADER: &str = r#"
#ifdef GL_ES
precision highp float;
precision highp sampler2D;
#endif
uniform vec4 u_color;
uniform vec3 u_key_dir;   // unit vector pointing toward the key light (the sun)
uniform vec3 u_fill_dir;  // unit vector pointing toward the fill light
uniform float u_ambient;
uniform float u_key;
uniform vec3 u_eye;
uniform vec3 u_view_dir;  // unit view direction (used when orthographic)
uniform int u_ortho;
uniform int u_unlit;
uniform sampler2D u_tex;
uniform int u_tex_mode;   // 0 flat color, 1 planar material texture, 2 picture (vertex uv)
uniform int u_proj;       // planar projection: 0 by face, 1 plan XZ
uniform vec2 u_inv_scale; // 1 / inches per repeat
uniform int u_flip;       // mirror a picture horizontally
uniform int u_look;       // 0 standard, 1 physical, 2 clay, 3 glass house, 4 watercolor,
                          // 5 technical, 6 duotone, 7 flat
uniform float u_rough;    // GGX roughness of the material, 0..1
uniform float u_metal;    // 0 dielectric .. 1 metal
uniform float u_spec;     // specular and sky-reflection strength of the look
uniform float u_exposure;
uniform vec3 u_sky_top;   // display-encoded sky colors (reflections)
uniform vec3 u_sky_horizon;
uniform vec3 u_ground;
uniform sampler2D u_shadow;
uniform int u_shadow_on;
uniform mat4 u_shadow_mvp;
uniform vec3 u_shadow_params; // x normal offset (inches), y depth bias, z texel in uv
uniform int u_pcf;            // PCF radius in texels
uniform int u_light_n;
uniform vec3 u_light_pos[8];
uniform vec3 u_light_col[8];  // color * intensity / pi
in vec3 v_pos;
in vec3 v_normal;
in vec2 v_uv;
out vec4 f_color;
//PLANAR_UV//

const float PI = 3.14159265;

// Fraction of the sun that reaches `pos`: percentage-closer filtering of the
// shadow map around the sample, with the position pushed off the surface.
float shadow_term(vec3 pos, vec3 n) {
    vec4 sp = u_shadow_mvp * vec4(pos + n * u_shadow_params.x, 1.0);
    vec3 pr = sp.xyz / sp.w * 0.5 + 0.5;
    if (pr.x < 0.0 || pr.x > 1.0 || pr.y < 0.0 || pr.y > 1.0 || pr.z > 1.0) {
        return 1.0;
    }
    float lit = 0.0;
    float taps = 0.0;
    for (int j = -2; j <= 2; ++j) {
        if (abs(j) > u_pcf) {
            continue;
        }
        for (int i = -2; i <= 2; ++i) {
            if (abs(i) > u_pcf) {
                continue;
            }
            float d = texture(u_shadow, pr.xy + vec2(float(i), float(j)) * u_shadow_params.z).r;
            lit += (pr.z - u_shadow_params.y <= d) ? 1.0 : 0.0;
            taps += 1.0;
        }
    }
    return lit / taps;
}

float ggx_d(float nh, float a2) {
    float t = nh * nh * (a2 - 1.0) + 1.0;
    return a2 / (PI * t * t);
}

// Height-correlated Smith visibility, already divided by 4 n.l n.v.
float smith_v(float nl, float nv, float a2) {
    float gv = nl * sqrt(nv * nv * (1.0 - a2) + a2);
    float gl = nv * sqrt(nl * nl * (1.0 - a2) + a2);
    return 0.5 / (gv + gl + 1e-4);
}

vec3 sky_linear(vec3 dir) {
    vec3 c = dir.y >= 0.0 ? mix(u_sky_horizon, u_sky_top, pow(clamp(dir.y, 0.0, 1.0), 0.5))
                          : mix(u_sky_horizon, u_ground, smoothstep(0.0, 0.35, -dir.y));
    return pow(c, vec3(2.2));
}

// Soft-shoulder tone curve: linear to 0.6, then rolls off toward 1.
vec3 tone(vec3 c) {
    vec3 hi = 0.6 + 0.4 * (1.0 - exp(-(c - 0.6) / 0.4));
    return mix(c, hi, step(0.6, c));
}

void main() {
    if (u_unlit == 1) {
        f_color = u_color;
        return;
    }
    vec4 albedo = u_color;
    if (u_tex_mode == 1) {
        // Textures are sRGB8: sampling returns linear light, like u_color.
        vec4 t = texture(u_tex, planar_uv(v_pos, v_normal, u_proj, u_inv_scale));
        albedo = vec4(t.rgb, u_color.a * t.a);
    } else if (u_tex_mode == 2) {
        vec2 uv = vec2(u_flip == 1 ? 1.0 - v_uv.x : v_uv.x, 1.0 - v_uv.y);
        vec4 t = texture(u_tex, uv);
        if (t.a < 0.02) {
            discard;
        }
        albedo = vec4(t.rgb, u_color.a * t.a);
    }
    vec3 to_eye = (u_ortho == 1) ? -u_view_dir : normalize(u_eye - v_pos);
    vec3 n = normalize(v_normal);
    // Two-sided lighting: always shade the side that faces the camera.
    if (dot(n, to_eye) < 0.0) {
        n = -n;
    }
    float ndl = max(dot(n, u_key_dir), 0.0);
    if (u_look == 3) {
        // Glass house: cool translucent panes, denser where seen edge-on.
        float fr = pow(1.0 - abs(dot(n, to_eye)), 2.0);
        vec3 tint = mix(vec3(0.62, 0.78, 0.92), vec3(0.88, 0.95, 1.0), fr);
        tint *= 0.86 + 0.14 * ndl;
        f_color = vec4(tint, clamp(0.10 + 0.55 * fr, 0.08, 0.7));
        return;
    }
    float sh = 1.0;
    if (u_shadow_on == 1 && ndl > 0.0) {
        sh = shadow_term(v_pos, n);
    }
    // Sky light from above, a little bounce light from below.
    float hemi = mix(0.85, 1.1, n.y * 0.5 + 0.5);
    float key_light = u_key * ndl * sh;
    if (u_look == 5) {
        // Technical illustration: two flat bands instead of a gradient.
        key_light = u_key * smoothstep(0.02, 0.12, ndl * sh);
    }
    float fill = u_key * 0.35 * max(dot(n, u_fill_dir), 0.0);
    vec3 light = vec3(u_ambient * hemi + key_light + fill);
    for (int i = 0; i < 8; ++i) {
        if (i >= u_light_n) {
            break;
        }
        vec3 d = u_light_pos[i] - v_pos;
        float d2 = dot(d, d);
        float nl = max(dot(n, d * inversesqrt(max(d2, 1.0))), 0.0);
        light += u_light_col[i] * nl / (d2 + 144.0);
    }
    vec3 base = albedo.rgb;
    vec3 rgb = base * (1.0 - u_metal) * light;
    float shine = 0.0;
    if (u_spec > 0.0) {
        vec3 f0 = mix(vec3(0.04), base, u_metal);
        float nv = max(dot(n, to_eye), 1e-3);
        float a = max(u_rough * u_rough, 0.03);
        float a2 = a * a;
        vec3 h = normalize(u_key_dir + to_eye);
        float vh = max(dot(to_eye, h), 0.0);
        vec3 f = f0 + (1.0 - f0) * pow(1.0 - vh, 5.0);
        float sun_spec = ndl > 0.0 ? ggx_d(max(dot(n, h), 0.0), a2) * smith_v(ndl, nv, a2) * ndl : 0.0;
        vec3 spec = f * min(sun_spec * PI * u_key * sh, 3.0);
        // Sky reflection fades in with grazing angles and glossiness.
        float gloss = (1.0 - u_rough) * (1.0 - u_rough);
        vec3 fr = f0 + (max(vec3(1.0 - u_rough), f0) - f0) * pow(1.0 - nv, 5.0);
        spec += sky_linear(reflect(-to_eye, n)) * fr * gloss * clamp(u_ambient * 1.6, 0.0, 1.0);
        spec *= u_spec;
        rgb += spec;
        shine = max(spec.r, max(spec.g, spec.b));
    }
    rgb *= u_exposure;
    if (u_look == 6) {
        // Duotone: light faces keep their color, shade falls to a deep blue.
        float l = dot(rgb, vec3(0.2126, 0.7152, 0.0722));
        rgb = mix(vec3(0.03, 0.07, 0.18), base * 1.15, smoothstep(0.08, 0.7, l));
    }
    rgb = tone(rgb);
    float alpha = albedo.a;
    if (alpha < 0.999) {
        // Reflections on glass and water show through the clear pane.
        alpha = clamp(alpha + shine * 0.8, 0.0, 1.0);
    }
    f_color = vec4(pow(rgb, vec3(1.0 / 2.2)), alpha);
}
"#;

/// The fragment shader with the shared surface-to-texture mapping inserted.
fn fragment_source() -> String {
    FRAGMENT_SHADER.replace("//PLANAR_UV//", &texturing::glsl_planar_uv())
}

/// Uniforms of the scene program.
const MAIN_UNIFORMS: &[&str] = &[
    "u_mvp",
    "u_ndc_offset",
    "u_color",
    "u_key_dir",
    "u_fill_dir",
    "u_ambient",
    "u_key",
    "u_eye",
    "u_view_dir",
    "u_ortho",
    "u_unlit",
    "u_tex",
    "u_tex_mode",
    "u_proj",
    "u_inv_scale",
    "u_flip",
    "u_look",
    "u_rough",
    "u_metal",
    "u_spec",
    "u_exposure",
    "u_sky_top",
    "u_sky_horizon",
    "u_ground",
    "u_shadow",
    "u_shadow_on",
    "u_shadow_mvp",
    "u_shadow_params",
    "u_pcf",
    "u_light_n",
    "u_light_pos",
    "u_light_col",
];

/// Everything the paint callback needs for one frame (plain data, no GL).
#[derive(Clone, Debug)]
pub(crate) struct FrameParams {
    pub proj: Mat4,
    pub view_proj: Mat4,
    pub eye: Vec3,
    pub view_dir: Vec3,
    pub ortho: bool,
    pub lighting: Lighting,
    pub show_edges: bool,
    pub hide_ceiling_roof: bool,
    /// Paint material textures and pictures (Standard technique).
    pub textures: bool,
    pub look: Look,
    pub settings: ViewSettings,
    /// Background (horizon) colour, display-encoded.
    pub background: [f32; 4],
    /// Point lights to evaluate (already the nearest few).
    pub lights: Vec<ViewLight>,
    pub bounds: Option<Bounds>,
    /// `(x, y, w, h)` of the callback rectangle in framebuffer pixels.
    pub viewport: (i32, i32, i32, i32),
    /// Framebuffer the window is drawn to (`None` is the default one).
    pub target_fbo: Option<glow::Framebuffer>,
    pub pixels_per_point: f32,
}

impl FrameParams {
    /// A frame looking through `camera` into a window of the given `aspect`
    /// ratio and `viewport` pixels.
    pub fn for_camera(camera: &Camera, aspect: f32, viewport: (i32, i32, i32, i32)) -> FrameParams {
        FrameParams {
            proj: camera.projection_matrix(aspect),
            view_proj: camera.view_projection(aspect),
            eye: camera.eye(),
            view_dir: camera.forward(),
            ortho: camera.mode.is_orthographic(),
            lighting: Lighting::default(),
            show_edges: true,
            hide_ceiling_roof: camera.mode.hides_ceiling_and_roof(),
            textures: true,
            look: Look::Standard,
            settings: ViewSettings::default(),
            background: [0.85, 0.89, 0.94, 1.0],
            lights: Vec::new(),
            bounds: None,
            viewport,
            target_fbo: None,
            pixels_per_point: 1.0,
        }
    }
}

/// The optional passes' programs, built once (a failure disables the pass).
#[derive(Default)]
struct Passes {
    tried: bool,
    shadow: Option<Prog>,
    sky: Option<Prog>,
    ssao: Option<Prog>,
    blur: Option<Prog>,
    composite: Option<Prog>,
}

struct GpuMesh {
    material: Material,
    object_id: Option<Id>,
    color: [f32; 4],
    /// GGX roughness and metalness from the scene material table.
    rough: f32,
    metal: f32,
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

/// A material texture resident on the GPU.
struct MaterialTex {
    tex: glow::Texture,
    inv_scale: [f32; 2],
    proj: i32,
}

/// A picture texture resident on the GPU.
struct PictureTex {
    tex: glow::Texture,
    has_alpha: bool,
}

/// How one mesh is textured in this frame.
#[derive(Clone, Copy)]
struct TexBind {
    tex: glow::Texture,
    /// Shader `u_tex_mode`: 1 planar material texture, 2 picture.
    mode: i32,
    proj: i32,
    inv_scale: [f32; 2],
    flip: bool,
    blended: bool,
}

/// GL resources for one uploaded scene plus the shader program.
pub(crate) struct GpuScene {
    program: Option<Prog>,
    meshes: Vec<GpuMesh>,
    error: Option<String>,
    program_tried: bool,
    store: Arc<TextureStore>,
    /// Textured materials the uploaded scene uses.
    needed: Vec<Material>,
    material_tex: HashMap<Material, MaterialTex>,
    /// Materials whose texture could not be produced (drawn flat).
    unavailable: HashSet<Material>,
    pictures: HashMap<u64, PictureTex>,
    /// Picture binding by mesh object id: (texture key, mirrored).
    by_object: HashMap<Id, (u64, bool)>,
    anisotropy: Option<Option<f32>>,
    /// A 1 x 1 white texture bound while drawing flat colors, so the sampler
    /// never points at an empty unit.
    blank: Option<glow::Texture>,
    /// Material textures still waiting for an upload after the last frame.
    pending_uploads: usize,
    /// Background texture loads in flight (the viewport spawns them).
    pub prefetching: Arc<AtomicUsize>,
    passes: Passes,
    targets: Targets,
    /// What the shadow map in `targets` was rendered from.
    shadow_key: Option<u64>,
    /// Bumped whenever the resident meshes change.
    mesh_version: u64,
    /// Why optional passes (shadows, occlusion, composite) are off.
    notes: Vec<String>,
}

impl Default for GpuScene {
    fn default() -> Self {
        GpuScene::with_store(TextureStore::shared())
    }
}

fn as_bytes<T: Copy>(data: &[T], to_bytes: impl Fn(T) -> [u8; 4]) -> Vec<u8> {
    data.iter().flat_map(|v| to_bytes(*v)).collect()
}

/// The GLSL version declaration for this context, or why it is too old.
fn shader_version(gl: &glow::Context) -> Result<&'static str, String> {
    let version = ShaderVersion::get(gl);
    if !version.is_new_shader_interface() {
        return Err(format!(
            "OpenGL shader version {version:?} is too old for plan-view3d (needs GLSL 1.40 or ES 3.00)"
        ));
    }
    Ok(version.version_declaration())
}

fn build_program(gl: &glow::Context) -> Result<Prog, String> {
    let decl = shader_version(gl)?;
    Prog::build(
        gl,
        decl,
        (VERTEX_SHADER, &fragment_source()),
        &[(0, "a_pos"), (1, "a_normal"), (2, "a_uv")],
        MAIN_UNIFORMS,
    )
}

/// Build the shadow, sky, occlusion and composite programs. A pass whose
/// shader fails is left out (its effect turns off) and the reason is added
/// to `errors`.
fn build_passes(gl: &glow::Context, errors: &mut Vec<String>) -> Passes {
    let mut passes = Passes {
        tried: true,
        ..Passes::default()
    };
    let decl = match shader_version(gl) {
        Ok(d) => d,
        Err(e) => {
            errors.push(e);
            return passes;
        }
    };
    let mut note = |name: &str, r: Result<Prog, String>| match r {
        Ok(p) => Some(p),
        Err(e) => {
            errors.push(format!("{name}: {e}"));
            None
        }
    };
    passes.shadow = note(
        "shadow pass",
        Prog::build(
            gl,
            decl,
            (pipeline::SHADOW_VERTEX, pipeline::SHADOW_FRAGMENT),
            &[(0, "a_pos")],
            &["u_mvp"],
        ),
    );
    passes.sky = note(
        "sky",
        Prog::build(
            gl,
            decl,
            (pipeline::FULLSCREEN_VERTEX, pipeline::SKY_FRAGMENT),
            &[],
            &[
                "u_inv_vp",
                "u_eye",
                "u_ortho",
                "u_sky_top",
                "u_sky_horizon",
                "u_ground",
            ],
        ),
    );
    passes.ssao = note(
        "ambient occlusion",
        Prog::build(
            gl,
            decl,
            (pipeline::FULLSCREEN_VERTEX, &pipeline::ssao_fragment()),
            &[],
            &[
                "u_depth",
                "u_inv_proj",
                "u_proj",
                "u_ortho",
                "u_kernel",
                "u_kernel_n",
                "u_radius",
                "u_bias",
                "u_texel",
            ],
        ),
    );
    passes.blur = note(
        "occlusion blur",
        Prog::build(
            gl,
            decl,
            (pipeline::FULLSCREEN_VERTEX, &pipeline::blur_fragment()),
            &[],
            &["u_depth", "u_inv_proj", "u_ortho", "u_ao", "u_texel"],
        ),
    );
    passes.composite = note(
        "composite",
        Prog::build(
            gl,
            decl,
            (pipeline::FULLSCREEN_VERTEX, &pipeline::composite_fragment()),
            &[],
            &[
                "u_color",
                "u_depth",
                "u_ao",
                "u_inv_proj",
                "u_ortho",
                "u_texel",
                "u_ao_strength",
                "u_fxaa",
                "u_edge_strength",
                "u_edge_px",
                "u_edge_color",
                "u_wash",
            ],
        ),
    );
    passes
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
    /// A scene that looks textures up in `store`.
    pub fn with_store(store: Arc<TextureStore>) -> GpuScene {
        GpuScene {
            program: None,
            meshes: Vec::new(),
            error: None,
            program_tried: false,
            store,
            needed: Vec::new(),
            material_tex: HashMap::new(),
            unavailable: HashSet::new(),
            pictures: HashMap::new(),
            by_object: HashMap::new(),
            anisotropy: None,
            blank: None,
            pending_uploads: 0,
            prefetching: Arc::new(AtomicUsize::new(0)),
            passes: Passes::default(),
            targets: Targets::default(),
            shadow_key: None,
            mesh_version: 0,
            notes: Vec::new(),
        }
    }

    /// Message describing why GL setup failed, if it did.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Why optional passes are off (shader or target failures), if any.
    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    /// Number of meshes currently resident on the GPU.
    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }

    /// Material textures resident on the GPU.
    pub fn material_texture_count(&self) -> usize {
        self.material_tex.len()
    }

    /// Picture textures resident on the GPU.
    pub fn picture_texture_count(&self) -> usize {
        self.pictures.len()
    }

    /// Material textures the last frame could not upload yet.
    pub fn pending_uploads(&self) -> usize {
        self.pending_uploads
    }

    /// Compile the shader program once (a failed attempt is not retried).
    pub fn ensure_program(&mut self, gl: &glow::Context) {
        if self.program.is_some() || self.program_tried {
            return;
        }
        self.program_tried = true;
        match build_program(gl) {
            Ok(p) => {
                self.program = Some(p);
                self.blank = unsafe { create_texture(gl, 1, 1, &[255; 4], true, None) }.ok();
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// True when the GL context was replaced behind our back: the program we
    /// compiled no longer names anything.
    pub fn context_was_replaced(&self, gl: &glow::Context) -> bool {
        self.program
            .as_ref()
            .is_some_and(|p| !unsafe { gl.is_program(p.program) })
    }

    /// Forget every GL object without deleting it (the context that owned
    /// them is gone). The caller re-uploads the scene and pictures; material
    /// textures come back on demand.
    pub fn forget_context(&mut self) {
        self.program = None;
        self.program_tried = false;
        self.error = None;
        self.meshes.clear();
        self.material_tex.clear();
        self.unavailable.clear();
        self.pictures.clear();
        self.by_object.clear();
        self.anisotropy = None;
        self.blank = None;
        self.pending_uploads = 0;
        self.passes = Passes::default();
        self.targets.forget();
        self.shadow_key = None;
        self.notes.clear();
        self.mesh_version += 1;
    }

    /// Replace the resident meshes with `scene`.
    pub fn upload(&mut self, gl: &glow::Context, scene: &Scene) {
        self.ensure_program(gl);
        self.free_meshes(gl);
        self.needed = texturing::needed_materials(scene);
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

    /// Make `images` the resident picture textures: new keys are uploaded,
    /// keys no longer listed are freed, unchanged ones cost nothing.
    pub fn set_pictures(&mut self, gl: &glow::Context, images: &[ImageTexture]) {
        let aniso = self.max_anisotropy(gl);
        let wanted: HashSet<u64> = images
            .iter()
            .filter(|i| i.is_valid())
            .map(|i| i.key)
            .collect();
        let stale: Vec<u64> = self
            .pictures
            .keys()
            .copied()
            .filter(|k| !wanted.contains(k))
            .collect();
        for k in stale {
            if let Some(p) = self.pictures.remove(&k) {
                unsafe { gl.delete_texture(p.tex) };
            }
        }
        self.by_object.clear();
        for img in images.iter().filter(|i| i.is_valid()) {
            if let std::collections::hash_map::Entry::Vacant(slot) = self.pictures.entry(img.key) {
                match unsafe { create_texture(gl, img.width, img.height, &img.rgba, false, aniso) }
                {
                    Ok(tex) => {
                        slot.insert(PictureTex {
                            tex,
                            has_alpha: img.has_alpha(),
                        });
                    }
                    Err(e) => {
                        self.error = Some(e);
                        continue;
                    }
                }
            }
            self.by_object.insert(img.object_id, (img.key, img.flip));
        }
    }

    fn max_anisotropy(&mut self, gl: &glow::Context) -> Option<f32> {
        *self.anisotropy.get_or_insert_with(|| {
            gl.supported_extensions()
                .contains("GL_EXT_texture_filter_anisotropic")
                .then(|| unsafe { gl.get_parameter_f32(MAX_TEXTURE_MAX_ANISOTROPY) })
                .map(|max| max.min(WANT_ANISOTROPY))
                .filter(|v| *v > 1.0)
        })
    }

    /// Upload the textures the scene needs and has not got yet, a couple per
    /// frame. A texture still being decoded in the background waits.
    fn upload_material_textures(&mut self, gl: &glow::Context) {
        let resident: HashSet<Material> = self
            .material_tex
            .keys()
            .chain(self.unavailable.iter())
            .copied()
            .collect();
        let aniso = self.max_anisotropy(gl);
        let loading = self.prefetching.load(Ordering::Relaxed) > 0;
        for m in texturing::next_uploads(&self.needed, &resident) {
            let image = if loading {
                self.store.peek_material(m)
            } else {
                self.store.material(m)
            };
            let Some(image) = image else {
                if !loading {
                    self.unavailable.insert(m);
                }
                continue;
            };
            let up = unsafe {
                create_texture(
                    gl,
                    image.image.width,
                    image.image.height,
                    &image.image.rgba,
                    true,
                    aniso,
                )
            };
            match up {
                Ok(tex) => {
                    self.material_tex.insert(
                        m,
                        MaterialTex {
                            tex,
                            inv_scale: [
                                1.0 / image.scale_in[0].max(1e-3),
                                1.0 / image.scale_in[1].max(1e-3),
                            ],
                            proj: texturing::proj_code(m),
                        },
                    );
                }
                Err(e) => {
                    self.error = Some(e);
                    self.unavailable.insert(m);
                }
            }
        }
        let resident: HashSet<Material> = self
            .material_tex
            .keys()
            .chain(self.unavailable.iter())
            .copied()
            .collect();
        self.pending_uploads = texturing::pending_uploads(&self.needed, &resident);
    }

    /// The texture a mesh is drawn with this frame, if any.
    fn bind_for(&self, m: &GpuMesh, textures: bool) -> Option<TexBind> {
        if !textures || m.material == Material::Selection {
            return None;
        }
        if let Some(&(key, flip)) = m.object_id.and_then(|id| self.by_object.get(&id)) {
            if let Some(p) = self.pictures.get(&key) {
                return Some(TexBind {
                    tex: p.tex,
                    mode: 2,
                    proj: 0,
                    inv_scale: [1.0, 1.0],
                    flip,
                    blended: p.has_alpha,
                });
            }
        }
        let t = self.material_tex.get(&m.material)?;
        Some(TexBind {
            tex: t.tex,
            mode: 1,
            proj: t.proj,
            inv_scale: t.inv_scale,
            flip: false,
            blended: false,
        })
    }

    /// Draw the resident meshes into the framebuffer region of `frame`.
    ///
    /// `scissor` is `(x, y, w, h)` in framebuffer pixels (origin bottom-left);
    /// nothing outside it is touched. The depth buffer is cleared inside it
    /// before drawing.
    pub fn paint(
        &mut self,
        gl: &glow::Context,
        frame: &FrameParams,
        scissor: (i32, i32, i32, i32),
    ) {
        if self.program.is_none() {
            return;
        }
        if frame.textures {
            self.upload_material_textures(gl);
        }
        self.ensure_passes(gl);
        let look = frame.look.params();
        let key_dir = math::normalize(frame.lighting.key_dir);
        // Fill light: weak, from the opposite horizontal side, a bit lower.
        let fill_dir = math::normalize([-key_dir[0], key_dir[1] * 0.4, -key_dir[2]]);
        let draws: Vec<Draw> = self
            .meshes
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                !(frame.hide_ceiling_roof
                    && matches!(m.material, Material::Ceiling | Material::Roof))
            })
            .map(|(index, m)| {
                let tex = self.bind_for(m, frame.textures);
                Draw {
                    index,
                    tex,
                    blended: m.is_transparent() || tex.is_some_and(|t| t.blended),
                }
            })
            .collect();
        let (vx, vy, vw, vh) = frame.viewport;

        // Step 1: the sun's shadow map.
        let sun_on = frame.lighting.key > 0.0;
        let shadow = if frame.settings.shadows && look.shadows && sun_on {
            self.shadow_pass(gl, frame, &draws, key_dir)
        } else {
            None
        };
        gl_check(gl, "shadow pass");

        // Which screen-space effects run this frame.
        let passes = &self.passes;
        let ao_ready = passes.ssao.is_some() && passes.blur.is_some();
        let ao_on = frame.settings.ambient_occlusion && look.ao_strength > 0.0 && ao_ready;
        let fxaa_on = frame.settings.quality.fxaa();
        let wants_post = passes.composite.is_some()
            && vw >= 16
            && vh >= 16
            && (ao_on || fxaa_on || look.edge_lines > 0.0 || look.wash > 0.0);
        let scene_target = if wants_post {
            match self.targets.ensure_scene(gl, (vw, vh)) {
                Ok(s) => Some((s.fbo, s.color, s.depth)),
                Err(e) => {
                    self.notes.push(format!("offscreen target: {e}"));
                    None
                }
            }
        } else {
            None
        };

        // Step 2: the scene.
        unsafe {
            match scene_target {
                Some((fbo, _, _)) => {
                    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
                    gl.viewport(0, 0, vw, vh);
                    gl.disable(glow::SCISSOR_TEST);
                    let bg = frame.background;
                    gl.clear_color(bg[0], bg[1], bg[2], 1.0);
                    gl.depth_mask(true);
                    gl.clear_depth_f32(1.0);
                    gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
                }
                None => {
                    gl.bind_framebuffer(glow::FRAMEBUFFER, frame.target_fbo);
                    gl.viewport(vx, vy, vw, vh);
                    gl.scissor(scissor.0, scissor.1, scissor.2, scissor.3);
                    gl.enable(glow::SCISSOR_TEST);
                    gl.depth_mask(true);
                    gl.clear_depth_f32(1.0);
                    gl.clear(glow::DEPTH_BUFFER_BIT);
                }
            }
        }
        gl_check(gl, "scene target setup");
        if look.sky {
            self.draw_sky(gl, frame);
        }
        gl_check(gl, "sky");
        self.draw_scene(
            gl,
            frame,
            &look,
            &draws,
            (key_dir, fill_dir),
            shadow.as_ref(),
        );
        gl_check(gl, "scene");

        // Steps 3 and 4: occlusion and the composite onto the window.
        if let Some((_, color, depth)) = scene_target {
            let ao = if ao_on {
                self.ao_passes(gl, frame, depth)
            } else {
                None
            };
            gl_check(gl, "occlusion");
            self.composite(gl, frame, &look, (color, depth, ao), scissor, fxaa_on);
            gl_check(gl, "composite");
        }

        unsafe {
            // Leave state the way egui_glow expects to find it.
            gl.bind_framebuffer(glow::FRAMEBUFFER, frame.target_fbo);
            gl.viewport(vx, vy, vw, vh);
            gl.scissor(scissor.0, scissor.1, scissor.2, scissor.3);
            gl.enable(glow::SCISSOR_TEST);
            for unit in [glow::TEXTURE2, glow::TEXTURE1, glow::TEXTURE0] {
                gl.active_texture(unit);
                gl.bind_texture(glow::TEXTURE_2D, None);
            }
            gl.disable(glow::POLYGON_OFFSET_FILL);
            gl.disable(glow::DEPTH_TEST);
            gl.depth_mask(true);
            gl.disable(glow::BLEND);
            gl.bind_vertex_array(None);
            gl.use_program(None);
        }
    }

    /// Build the optional passes' programs once.
    fn ensure_passes(&mut self, gl: &glow::Context) {
        if !self.passes.tried {
            let mut errors = Vec::new();
            self.passes = build_passes(gl, &mut errors);
            self.notes.extend(errors);
        }
    }

    /// Render the shadow map unless it is still valid; returns what the scene
    /// shader needs to sample it.
    fn shadow_pass(
        &mut self,
        gl: &glow::Context,
        frame: &FrameParams,
        draws: &[Draw],
        key_dir: Vec3,
    ) -> Option<ShadowUse> {
        let (min, max) = frame.bounds?;
        let prog = self.passes.shadow.as_ref()?;
        let size = frame.settings.quality.shadow_size();
        let map = quality::shadow_map(key_dir, min, max, size);
        let (fbo, depth) = match self.targets.ensure_shadow(gl, size) {
            Ok(t) => (t.fbo, t.depth),
            Err(e) => {
                self.notes.push(format!("shadow map: {e}"));
                return None;
            }
        };
        let key = shadow_key(self.mesh_version, key_dir, frame.hide_ceiling_roof, size);
        if self.shadow_key != Some(key) {
            unsafe {
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
                gl.viewport(0, 0, size, size);
                gl.disable(glow::SCISSOR_TEST);
                gl.disable(glow::BLEND);
                gl.depth_mask(true);
                gl.clear_depth_f32(1.0);
                gl.clear(glow::DEPTH_BUFFER_BIT);
                gl.enable(glow::DEPTH_TEST);
                gl.depth_func(glow::LESS);
                gl.disable(glow::CULL_FACE);
                // Slope-scaled bias keeps lit faces from shadowing themselves.
                gl.enable(glow::POLYGON_OFFSET_FILL);
                gl.polygon_offset(2.0, 4.0);
                gl.use_program(Some(prog.program));
                prog.m4(gl, "u_mvp", &map.view_proj);
                for d in draws.iter().filter(|d| !d.blended) {
                    let m = &self.meshes[d.index];
                    gl.bind_vertex_array(Some(m.vao));
                    gl.draw_elements(glow::TRIANGLES, m.index_count, glow::UNSIGNED_INT, 0);
                }
                gl.disable(glow::POLYGON_OFFSET_FILL);
                gl.bind_vertex_array(None);
            }
            self.shadow_key = Some(key);
        }
        Some(ShadowUse { map, depth, size })
    }

    /// Paint the sky gradient and ground fade over the whole target.
    fn draw_sky(&mut self, gl: &glow::Context, frame: &FrameParams) {
        let (Some(prog), Some(inv_vp)) = (self.passes.sky.as_ref(), math::invert(&frame.view_proj))
        else {
            return;
        };
        let Some(vao) = self.targets.fullscreen_vao(gl) else {
            return;
        };
        let (top, horizon, ground) = quality::sky_colors(frame.background);
        unsafe {
            gl.disable(glow::DEPTH_TEST);
            gl.depth_mask(false);
            gl.disable(glow::BLEND);
            gl.use_program(Some(prog.program));
            prog.m4(gl, "u_inv_vp", &inv_vp);
            prog.f3(gl, "u_eye", &frame.eye);
            prog.i1(gl, "u_ortho", i32::from(frame.ortho));
            prog.f3(gl, "u_sky_top", &top);
            prog.f3(gl, "u_sky_horizon", &horizon);
            prog.f3(gl, "u_ground", &ground);
            gl.bind_vertex_array(Some(vao));
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            gl.bind_vertex_array(None);
            gl.depth_mask(true);
        }
    }

    /// The opaque meshes, edge lines and glass with the scene shader.
    fn draw_scene(
        &self,
        gl: &glow::Context,
        frame: &FrameParams,
        look: &quality::LookParams,
        draws: &[Draw],
        (key_dir, fill_dir): (Vec3, Vec3),
        shadow: Option<&ShadowUse>,
    ) {
        let Some(prog) = &self.program else { return };
        unsafe {
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LEQUAL);
            gl.disable(glow::CULL_FACE);
            // Push filled faces back a hair so edge lines win the depth test.
            gl.enable(glow::POLYGON_OFFSET_FILL);
            gl.polygon_offset(1.0, 1.0);

            gl.use_program(Some(prog.program));
            prog.m4(gl, "u_mvp", &frame.view_proj);
            prog.f2(gl, "u_ndc_offset", [0.0, 0.0]);
            prog.f3(gl, "u_key_dir", &key_dir);
            prog.f3(gl, "u_fill_dir", &fill_dir);
            prog.f1(gl, "u_ambient", frame.lighting.ambient);
            prog.f1(gl, "u_key", frame.lighting.key);
            prog.f3(gl, "u_eye", &frame.eye);
            prog.f3(gl, "u_view_dir", &frame.view_dir);
            prog.i1(gl, "u_ortho", i32::from(frame.ortho));
            prog.i1(gl, "u_unlit", 0);
            prog.i1(gl, "u_tex", 0);
            prog.i1(gl, "u_tex_mode", 0);
            prog.i1(gl, "u_look", look.code);
            prog.f1(gl, "u_spec", look.specular);
            prog.f1(gl, "u_exposure", frame.settings.exposure);
            let (top, horizon, ground) = quality::sky_colors(frame.background);
            prog.f3(gl, "u_sky_top", &top);
            prog.f3(gl, "u_sky_horizon", &horizon);
            prog.f3(gl, "u_ground", &ground);
            // Point lights.
            let mut pos = [0.0_f32; 3 * MAX_POINT_LIGHTS];
            let mut col = [0.0_f32; 3 * MAX_POINT_LIGHTS];
            let n = frame.lights.len().min(MAX_POINT_LIGHTS);
            for (i, l) in frame.lights.iter().take(n).enumerate() {
                pos[i * 3..i * 3 + 3].copy_from_slice(&l.position);
                col[i * 3..i * 3 + 3].copy_from_slice(&l.color);
            }
            prog.i1(gl, "u_light_n", n as i32);
            prog.f3(gl, "u_light_pos", &pos);
            prog.f3(gl, "u_light_col", &col);
            // Shadows.
            match shadow {
                Some(s) => {
                    gl.active_texture(glow::TEXTURE1);
                    gl.bind_texture(glow::TEXTURE_2D, Some(s.depth));
                    gl.active_texture(glow::TEXTURE0);
                    prog.i1(gl, "u_shadow", 1);
                    prog.i1(gl, "u_shadow_on", 1);
                    prog.m4(gl, "u_shadow_mvp", &s.map.view_proj);
                    prog.f3(
                        gl,
                        "u_shadow_params",
                        &[
                            s.map.texel_world * 1.5,
                            (s.map.texel_world * 0.8 + 0.05) / s.map.depth_range.max(1.0),
                            1.0 / s.size as f32,
                        ],
                    );
                    prog.i1(gl, "u_pcf", frame.settings.quality.pcf_radius());
                }
                None => {
                    // Keep unit 1 pointing at a real texture.
                    gl.active_texture(glow::TEXTURE1);
                    gl.bind_texture(glow::TEXTURE_2D, self.blank);
                    gl.active_texture(glow::TEXTURE0);
                    prog.i1(gl, "u_shadow", 1);
                    prog.i1(gl, "u_shadow_on", 0);
                }
            }
            gl.active_texture(glow::TEXTURE0);

            // Opaque pass.
            gl.disable(glow::BLEND);
            for d in draws.iter().filter(|d| !d.blended) {
                self.draw_fill(gl, prog, look, &self.meshes[d.index], d.tex);
            }

            // Edge overlay (opaque and transparent meshes alike); a glass
            // house draws its lines over the panes instead.
            let glass_house = look.code == 3;
            if frame.show_edges && !glass_house {
                let c = if look.edge_lines > 0.0 {
                    look.edge_color
                } else {
                    [0.16, 0.16, 0.17]
                };
                gl.disable(glow::POLYGON_OFFSET_FILL);
                self.draw_edges(gl, prog, draws, [c[0], c[1], c[2], 1.0], &[[0.0, 0.0]]);
                gl.enable(glow::POLYGON_OFFSET_FILL);
            }

            // Transparent pass, back to front by centroid distance from the eye.
            let mut glass: Vec<&Draw> = draws.iter().filter(|d| d.blended).collect();
            if !glass.is_empty() {
                let dist = |m: &GpuMesh| math::length(math::sub(m.centroid, frame.eye));
                glass.sort_by(|a, b| {
                    dist(&self.meshes[b.index]).total_cmp(&dist(&self.meshes[a.index]))
                });
                gl.enable(glow::BLEND);
                gl.blend_equation_separate(glow::FUNC_ADD, glow::FUNC_ADD);
                gl.blend_func_separate(
                    glow::SRC_ALPHA,
                    glow::ONE_MINUS_SRC_ALPHA,
                    glow::ONE,
                    glow::ONE_MINUS_SRC_ALPHA,
                );
                gl.depth_mask(false);
                for d in glass {
                    self.draw_fill(gl, prog, look, &self.meshes[d.index], d.tex);
                }
                gl.depth_mask(true);
            }
            if frame.show_edges && glass_house {
                // Everything is see-through: draw all the lines, hidden ones
                // too, about two pixels wide (four copies half a pixel apart).
                let (w, h) = (
                    frame.viewport.2.max(1) as f32,
                    frame.viewport.3.max(1) as f32,
                );
                let (dx, dy) = (0.5 / w, 0.5 / h);
                gl.disable(glow::POLYGON_OFFSET_FILL);
                gl.disable(glow::DEPTH_TEST);
                gl.enable(glow::BLEND);
                let c = look.edge_color;
                self.draw_edges(
                    gl,
                    prog,
                    draws,
                    [c[0], c[1], c[2], 0.85],
                    &[[-dx, -dy], [dx, -dy], [-dx, dy], [dx, dy]],
                );
                gl.enable(glow::DEPTH_TEST);
            }
            gl.bind_vertex_array(None);
            gl.disable(glow::BLEND);
            gl.disable(glow::POLYGON_OFFSET_FILL);
        }
    }

    /// Draw the feature-edge lines of `draws` in `color`, once per offset.
    unsafe fn draw_edges(
        &self,
        gl: &glow::Context,
        prog: &Prog,
        draws: &[Draw],
        color: [f32; 4],
        offsets: &[[f32; 2]],
    ) {
        unsafe {
            prog.i1(gl, "u_unlit", 1);
            prog.f4(gl, "u_color", color);
            for off in offsets {
                prog.f2(gl, "u_ndc_offset", *off);
                for d in draws {
                    let m = &self.meshes[d.index];
                    if let Some(vao) = m.edge_vao {
                        gl.bind_vertex_array(Some(vao));
                        gl.draw_arrays(glow::LINES, 0, m.edge_vertex_count);
                    }
                }
            }
            prog.f2(gl, "u_ndc_offset", [0.0, 0.0]);
            prog.i1(gl, "u_unlit", 0);
        }
    }

    /// Half-resolution ambient occlusion from the scene depth; returns the
    /// blurred occlusion texture.
    fn ao_passes(
        &mut self,
        gl: &glow::Context,
        frame: &FrameParams,
        depth: glow::Texture,
    ) -> Option<glow::Texture> {
        let (Some(ssao), Some(blur)) = (self.passes.ssao.as_ref(), self.passes.blur.as_ref())
        else {
            return None;
        };
        let inv_proj = math::invert(&frame.proj)?;
        let size = (frame.viewport.2, frame.viewport.3);
        let (raw_fbo, raw, blur_fbo, blur_tex, half) = match self.targets.ensure_ao(gl, size) {
            Ok(a) => (a.raw_fbo, a.raw, a.blur_fbo, a.blur, a.size),
            Err(e) => {
                self.notes.push(format!("occlusion target: {e}"));
                return None;
            }
        };
        let vao = self.targets.fullscreen_vao(gl)?;
        let n = frame.settings.quality.ao_samples().min(MAX_AO_SAMPLES);
        let mut kernel = [0.0_f32; 3 * MAX_AO_SAMPLES];
        for (i, k) in quality::ssao_kernel(n, 11).iter().enumerate() {
            kernel[i * 3..i * 3 + 3].copy_from_slice(k);
        }
        let radius = frame.bounds.map_or(24.0, |(lo, hi)| {
            (math::length(math::sub(hi, lo)) * 0.03).clamp(14.0, 40.0)
        });
        unsafe {
            gl.disable(glow::SCISSOR_TEST);
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::BLEND);
            gl.depth_mask(false);
            gl.bind_vertex_array(Some(vao));
            gl.viewport(0, 0, half.0, half.1);

            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(raw_fbo));
            gl.use_program(Some(ssao.program));
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, Some(depth));
            ssao.i1(gl, "u_depth", 1);
            ssao.m4(gl, "u_inv_proj", &inv_proj);
            ssao.m4(gl, "u_proj", &frame.proj);
            ssao.i1(gl, "u_ortho", i32::from(frame.ortho));
            ssao.f3(gl, "u_kernel", &kernel);
            ssao.i1(gl, "u_kernel_n", n as i32);
            ssao.f1(gl, "u_radius", radius);
            ssao.f1(gl, "u_bias", radius * 0.04);
            ssao.f2(gl, "u_texel", [1.0 / size.0 as f32, 1.0 / size.1 as f32]);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);

            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(blur_fbo));
            gl.use_program(Some(blur.program));
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(raw));
            blur.i1(gl, "u_ao", 0);
            blur.i1(gl, "u_depth", 1);
            blur.m4(gl, "u_inv_proj", &inv_proj);
            blur.i1(gl, "u_ortho", i32::from(frame.ortho));
            blur.f2(gl, "u_texel", [1.0 / half.0 as f32, 1.0 / half.1 as f32]);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            gl.bind_vertex_array(None);
            gl.depth_mask(true);
        }
        Some(blur_tex)
    }

    /// FXAA, occlusion, edge lines and the watercolor wash onto the window.
    fn composite(
        &mut self,
        gl: &glow::Context,
        frame: &FrameParams,
        look: &quality::LookParams,
        (color, depth, ao): (glow::Texture, glow::Texture, Option<glow::Texture>),
        scissor: (i32, i32, i32, i32),
        fxaa: bool,
    ) {
        let (Some(prog), Some(inv_proj)) =
            (self.passes.composite.as_ref(), math::invert(&frame.proj))
        else {
            return;
        };
        let Some(vao) = self.targets.fullscreen_vao(gl) else {
            return;
        };
        let (vx, vy, vw, vh) = frame.viewport;
        unsafe {
            gl.bind_framebuffer(glow::FRAMEBUFFER, frame.target_fbo);
            gl.viewport(vx, vy, vw, vh);
            gl.scissor(scissor.0, scissor.1, scissor.2, scissor.3);
            gl.enable(glow::SCISSOR_TEST);
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::BLEND);
            gl.depth_mask(false);
            gl.use_program(Some(prog.program));
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(color));
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, Some(depth));
            gl.active_texture(glow::TEXTURE2);
            gl.bind_texture(glow::TEXTURE_2D, ao.or(Some(color)));
            gl.active_texture(glow::TEXTURE0);
            prog.i1(gl, "u_color", 0);
            prog.i1(gl, "u_depth", 1);
            prog.i1(gl, "u_ao", 2);
            prog.m4(gl, "u_inv_proj", &inv_proj);
            prog.i1(gl, "u_ortho", i32::from(frame.ortho));
            prog.f2(gl, "u_texel", [1.0 / vw as f32, 1.0 / vh as f32]);
            prog.f1(
                gl,
                "u_ao_strength",
                if ao.is_some() { look.ao_strength } else { 0.0 },
            );
            prog.i1(gl, "u_fxaa", i32::from(fxaa));
            prog.f1(gl, "u_edge_strength", look.edge_lines);
            prog.f1(
                gl,
                "u_edge_px",
                (look.edge_width * frame.pixels_per_point).clamp(1.0, 4.0),
            );
            prog.f3(gl, "u_edge_color", &look.edge_color);
            prog.f1(gl, "u_wash", look.wash);
            gl.bind_vertex_array(Some(vao));
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            gl.bind_vertex_array(None);
            gl.depth_mask(true);
        }
    }

    unsafe fn draw_fill(
        &self,
        gl: &glow::Context,
        prog: &Prog,
        look: &quality::LookParams,
        m: &GpuMesh,
        tex: Option<TexBind>,
    ) {
        unsafe {
            prog.f4(gl, "u_color", m.color);
            // Looks without gloss (clay, flat, technical) also hide the
            // material table's roughness and metal.
            prog.f1(gl, "u_rough", m.rough);
            prog.f1(
                gl,
                "u_metal",
                if look.specular > 0.0 { m.metal } else { 0.0 },
            );
            match tex {
                Some(t) => {
                    gl.bind_texture(glow::TEXTURE_2D, Some(t.tex));
                    prog.i1(gl, "u_tex_mode", t.mode);
                    prog.i1(gl, "u_proj", t.proj);
                    prog.f2(gl, "u_inv_scale", t.inv_scale);
                    prog.i1(gl, "u_flip", i32::from(t.flip));
                }
                None => {
                    gl.bind_texture(glow::TEXTURE_2D, self.blank);
                    prog.i1(gl, "u_tex_mode", 0);
                }
            }
            gl.bind_vertex_array(Some(m.vao));
            gl.draw_elements(glow::TRIANGLES, m.index_count, glow::UNSIGNED_INT, 0);
        }
    }

    fn free_meshes(&mut self, gl: &glow::Context) {
        self.mesh_version += 1;
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
        for (_, t) in self.material_tex.drain() {
            unsafe { gl.delete_texture(t.tex) };
        }
        for (_, p) in self.pictures.drain() {
            unsafe { gl.delete_texture(p.tex) };
        }
        self.by_object.clear();
        self.unavailable.clear();
        if let Some(b) = self.blank.take() {
            unsafe { gl.delete_texture(b) };
        }
        if let Some(p) = self.program.take() {
            p.delete(gl);
        }
        let passes = std::mem::take(&mut self.passes);
        for p in [
            passes.shadow,
            passes.sky,
            passes.ssao,
            passes.blur,
            passes.composite,
        ]
        .into_iter()
        .flatten()
        {
            p.delete(gl);
        }
        self.targets.destroy(gl);
        self.shadow_key = None;
        self.program_tried = false;
    }
}

/// With `PLAN_GL_DEBUG` set, prints any GL error raised since the last check
/// together with the stage that just ran.
fn gl_check(gl: &glow::Context, stage: &str) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ON.get_or_init(|| std::env::var_os("PLAN_GL_DEBUG").is_some()) {
        let e = unsafe { gl.get_error() };
        if e != 0 {
            eprintln!("GL error {e:#x} after {stage}");
        }
    }
}

/// One mesh to draw this frame.
struct Draw {
    index: usize,
    tex: Option<TexBind>,
    blended: bool,
}

/// The shadow map the scene shader samples.
struct ShadowUse {
    map: ShadowMap,
    depth: glow::Texture,
    size: i32,
}

/// What the cached shadow map depends on.
fn shadow_key(mesh_version: u64, sun: Vec3, hide_roof: bool, size: i32) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    mesh_version.hash(&mut h);
    for c in sun {
        c.to_bits().hash(&mut h);
    }
    hide_roof.hash(&mut h);
    size.hash(&mut h);
    h.finish()
}

/// Create an sRGB RGBA8 texture with mipmaps. Material textures repeat,
/// pictures clamp. Anisotropic filtering is used when `aniso` is given.
unsafe fn create_texture(
    gl: &glow::Context,
    width: u32,
    height: u32,
    rgba: &[u8],
    repeat: bool,
    aniso: Option<f32>,
) -> Result<glow::Texture, String> {
    unsafe {
        let tex = gl.create_texture()?;
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(tex));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::SRGB8_ALPHA8 as i32,
            width as i32,
            height as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(rgba)),
        );
        let wrap = if repeat {
            glow::REPEAT
        } else {
            glow::CLAMP_TO_EDGE
        } as i32;
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, wrap);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, wrap);
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR_MIPMAP_LINEAR as i32,
        );
        if let Some(a) = aniso {
            gl.tex_parameter_f32(glow::TEXTURE_2D, TEXTURE_MAX_ANISOTROPY, a);
        }
        gl.generate_mipmap(glow::TEXTURE_2D);
        gl.bind_texture(glow::TEXTURE_2D, None);
        Ok(tex)
    }
}

fn upload_mesh(gl: &glow::Context, mesh: &plan_3d::Mesh) -> Result<GpuMesh, String> {
    let surface = plan_materials::scene_surface(mesh.material);
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
            object_id: mesh.object_id,
            color: mesh.material.color(),
            rough: surface.roughness,
            metal: surface.metallic,
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

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "macos")]
    mod headless;

    /// The shader sources, headless: they mention every uniform the Rust side
    /// looks up. `PLAN_SHADER_DUMP=<dir>` also writes them for a GLSL compiler.
    #[test]
    fn shader_sources_name_the_uniforms_the_program_reads() {
        let frag = fragment_source();
        for name in MAIN_UNIFORMS {
            if *name == "u_mvp" || *name == "u_ndc_offset" {
                assert!(VERTEX_SHADER.contains(name), "vertex shader lacks {name}");
            } else {
                assert!(frag.contains(name), "fragment shader lacks {name}");
            }
        }
        assert!(frag.contains("vec2 planar_uv("), "{frag}");
        assert!(!frag.contains("//PLANAR_UV//"));
        // The shader's tone curve is the one `quality::tone_map` mirrors.
        assert!(frag.contains("0.6 + 0.4 * (1.0 - exp(-(c - 0.6) / 0.4))"));
        if let Some(dir) = std::env::var_os("PLAN_SHADER_DUMP") {
            let dir = std::path::PathBuf::from(dir);
            let write = |name: &str, text: &str| std::fs::write(dir.join(name), text).unwrap();
            write("view.vert", VERTEX_SHADER);
            write("view.frag", &frag);
            write("fullscreen.vert", pipeline::FULLSCREEN_VERTEX);
            write("shadow.vert", pipeline::SHADOW_VERTEX);
            write("shadow.frag", pipeline::SHADOW_FRAGMENT);
            write("sky.frag", pipeline::SKY_FRAGMENT);
            write("ssao.frag", &pipeline::ssao_fragment());
            write("blur.frag", &pipeline::blur_fragment());
            write("composite.frag", &pipeline::composite_fragment());
        }
    }

    #[test]
    fn the_shadow_cache_key_tracks_scene_sun_and_size() {
        let k = |v, sun: Vec3, hide, size| shadow_key(v, sun, hide, size);
        let base = k(1, [0.0, 1.0, 0.0], false, 2048);
        assert_eq!(base, k(1, [0.0, 1.0, 0.0], false, 2048));
        assert_ne!(base, k(2, [0.0, 1.0, 0.0], false, 2048));
        assert_ne!(base, k(1, [0.1, 1.0, 0.0], false, 2048));
        assert_ne!(base, k(1, [0.0, 1.0, 0.0], true, 2048));
        assert_ne!(base, k(1, [0.0, 1.0, 0.0], false, 4096));
    }
}
