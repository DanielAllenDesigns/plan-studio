//! Offscreen targets and screen-space passes of the interactive renderer.
//!
//! The Standard view renders in up to four steps:
//!
//! 1. **Shadow map**: the opaque meshes drawn depth-only from the sun into a
//!    square depth texture fitted to the scene ([`crate::quality::shadow_map`]).
//! 2. **Scene**: the lit meshes into an offscreen colour + depth target.
//! 3. **Ambient occlusion**: a half-resolution pass reads the scene depth,
//!    rebuilds view-space positions and normals and samples a hemisphere
//!    kernel; a 4 x 4 box blur removes the rotation noise.
//! 4. **Composite**: one full-screen pass applies FXAA, the occlusion, depth
//!    and normal edge lines (Technical Illustration) and the watercolor wash
//!    onto the window.
//!
//! This module owns the GL objects and the GLSL for steps 1, 3 and 4 and the
//! sky gradient; `gpu` owns the scene shader and the draw order.

use std::collections::HashMap;

use eframe::glow::{self, HasContext as _};

/// Sky gradient behind the model.
pub(crate) const SKY_FRAGMENT: &str = r#"
#ifdef GL_ES
precision highp float;
#endif
uniform mat4 u_inv_vp;
uniform vec3 u_eye;
uniform int u_ortho;
uniform vec3 u_sky_top;
uniform vec3 u_sky_horizon;
uniform vec3 u_ground;
uniform int u_ground_mode;    // 0 fade, 1 flat color, 2 sky carries on
uniform vec3 u_ground_solid;
uniform int u_backdrop_on;
uniform sampler2D u_backdrop;
uniform vec2 u_backdrop_scale;
in vec2 v_uv;
out vec4 f_color;
void main() {
    if (u_backdrop_on == 1) {
        // A picture behind the model, scaled to cover the view.
        vec2 uv = (v_uv - 0.5) * u_backdrop_scale + 0.5;
        f_color = vec4(texture(u_backdrop, vec2(uv.x, 1.0 - uv.y)).rgb, 1.0);
        return;
    }
    vec3 col = u_sky_horizon;
    if (u_ortho == 0) {
        vec4 p = u_inv_vp * vec4(v_uv * 2.0 - 1.0, 1.0, 1.0);
        vec3 dir = normalize(p.xyz / p.w - u_eye);
        float up = dir.y;
        if (up >= 0.0) {
            col = mix(u_sky_horizon, u_sky_top, pow(clamp(up, 0.0, 1.0), 0.5));
        } else if (u_ground_mode == 1) {
            col = mix(u_sky_horizon, u_ground_solid, smoothstep(0.0, 0.02, -up));
        } else if (u_ground_mode == 2) {
            col = mix(u_sky_horizon, u_sky_top, pow(clamp(-up, 0.0, 1.0), 0.5));
        } else {
            col = mix(u_sky_horizon, u_ground, smoothstep(0.0, 0.35, -up));
        }
    }
    f_color = vec4(col, 1.0);
}
"#;

/// Full-screen triangle from the vertex index; draw 3 vertices with an empty VAO.
pub(crate) const FULLSCREEN_VERTEX: &str = r#"
#ifdef GL_ES
precision highp float;
#endif
out vec2 v_uv;
void main() {
    vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
    v_uv = p;
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
"#;

/// Depth-only vertex shader of the shadow pass.
pub(crate) const SHADOW_VERTEX: &str = r#"
#ifdef GL_ES
precision highp float;
#endif
uniform mat4 u_mvp;
in vec3 a_pos;
void main() {
    gl_Position = u_mvp * vec4(a_pos, 1.0);
}
"#;

/// Depth-only fragment shader of the shadow pass.
pub(crate) const SHADOW_FRAGMENT: &str = r#"
#ifdef GL_ES
precision highp float;
#endif
out vec4 f_color;
void main() {
    f_color = vec4(1.0);
}
"#;

/// Helpers shared by the passes that rebuild positions from the depth buffer.
const DEPTH_HELPERS: &str = r#"
uniform sampler2D u_depth;
uniform mat4 u_inv_proj;
uniform int u_ortho;
vec3 view_pos(vec2 uv) {
    float d = texture(u_depth, uv).r;
    vec4 v = u_inv_proj * vec4(uv * 2.0 - 1.0, d * 2.0 - 1.0, 1.0);
    return v.xyz / v.w;
}
// View-space normal facing the camera, from the smaller depth step per axis.
vec3 view_normal(vec2 uv, vec2 px, vec3 p) {
    vec3 r = view_pos(uv + vec2(px.x, 0.0));
    vec3 l = view_pos(uv - vec2(px.x, 0.0));
    vec3 u = view_pos(uv + vec2(0.0, px.y));
    vec3 d = view_pos(uv - vec2(0.0, px.y));
    vec3 dx = abs(r.z - p.z) < abs(l.z - p.z) ? r - p : p - l;
    vec3 dy = abs(u.z - p.z) < abs(d.z - p.z) ? u - p : p - d;
    vec3 n = normalize(cross(dx, dy));
    vec3 to_cam = u_ortho == 1 ? vec3(0.0, 0.0, 1.0) : -p;
    return dot(n, to_cam) < 0.0 ? -n : n;
}
"#;

/// Half-resolution ambient occlusion.
const SSAO_BODY: &str = r#"
uniform mat4 u_proj;
uniform vec3 u_kernel[16];
uniform int u_kernel_n;
uniform float u_radius;
uniform float u_bias;
uniform vec2 u_texel;
in vec2 v_uv;
out vec4 f_color;
// 4 x 4 ordered-dither value in [0, 1): matched to the 4 x 4 blur.
float bayer4(vec2 frag) {
    int x = int(mod(frag.x, 4.0));
    int y = int(mod(frag.y, 4.0));
    int bx = x ^ y;
    int idx = ((bx & 1) << 3) | ((y & 1) << 2) | (bx & 2) | ((y & 2) >> 1);
    return float(idx) / 16.0;
}
void main() {
    if (texture(u_depth, v_uv).r >= 1.0) {
        f_color = vec4(1.0);
        return;
    }
    vec3 p = view_pos(v_uv);
    vec3 n = view_normal(v_uv, u_texel, p);
    float ang = 6.2831853 * bayer4(gl_FragCoord.xy);
    vec3 rvec = vec3(cos(ang), sin(ang), 0.0);
    vec3 t = normalize(rvec - n * dot(rvec, n));
    vec3 b = cross(n, t);
    mat3 tbn = mat3(t, b, n);
    float occ = 0.0;
    for (int i = 0; i < 16; ++i) {
        if (i >= u_kernel_n) {
            break;
        }
        vec3 sp = p + tbn * u_kernel[i] * u_radius;
        vec4 clip = u_proj * vec4(sp, 1.0);
        vec2 suv = clip.xy / clip.w * 0.5 + 0.5;
        if (suv.x < 0.0 || suv.x > 1.0 || suv.y < 0.0 || suv.y > 1.0) {
            continue;
        }
        float sz = view_pos(suv).z;
        float range = smoothstep(0.0, 1.0, u_radius / max(abs(p.z - sz), 1e-3));
        occ += (sz >= sp.z + u_bias + 0.003 * abs(p.z) ? 1.0 : 0.0) * range;
    }
    float ao = 1.0 - occ / float(max(u_kernel_n, 1));
    f_color = vec4(vec3(ao), 1.0);
}
"#;

/// Depth-aware 4 x 4 box blur of the raw occlusion.
const BLUR_BODY: &str = r#"
uniform sampler2D u_ao;
uniform vec2 u_texel;
in vec2 v_uv;
out vec4 f_color;
void main() {
    float zc = view_pos(v_uv).z;
    float sum = 0.0;
    float wsum = 0.0;
    for (int j = -2; j < 2; ++j) {
        for (int i = -2; i < 2; ++i) {
            vec2 uv = v_uv + vec2(float(i), float(j)) * u_texel;
            float z = view_pos(uv).z;
            float w = 1.0 / (1.0 + 60.0 * abs(z - zc) / max(abs(zc), 1.0));
            sum += texture(u_ao, uv).r * w;
            wsum += w;
        }
    }
    f_color = vec4(vec3(sum / wsum), 1.0);
}
"#;

/// FXAA, occlusion, edge lines and watercolor wash in one full-screen pass.
const COMPOSITE_BODY: &str = r#"
uniform sampler2D u_color;
uniform sampler2D u_ao;
uniform vec2 u_texel;
uniform float u_ao_strength;
uniform int u_fxaa;
uniform float u_edge_strength;
uniform float u_edge_px;
uniform vec3 u_edge_color;
uniform float u_wash;
in vec2 v_uv;
out vec4 f_color;

float luma(vec3 c) {
    return dot(c, vec3(0.299, 0.587, 0.114));
}

// Lottes' FXAA 1.0 edge blur.
vec3 fxaa(vec2 uv) {
    vec3 nw = texture(u_color, uv + vec2(-1.0, -1.0) * u_texel).rgb;
    vec3 ne = texture(u_color, uv + vec2(1.0, -1.0) * u_texel).rgb;
    vec3 sw = texture(u_color, uv + vec2(-1.0, 1.0) * u_texel).rgb;
    vec3 se = texture(u_color, uv + vec2(1.0, 1.0) * u_texel).rgb;
    vec3 m = texture(u_color, uv).rgb;
    float lnw = luma(nw);
    float lne = luma(ne);
    float lsw = luma(sw);
    float lse = luma(se);
    float lm = luma(m);
    float lmin = min(lm, min(min(lnw, lne), min(lsw, lse)));
    float lmax = max(lm, max(max(lnw, lne), max(lsw, lse)));
    vec2 dir = vec2(-((lnw + lne) - (lsw + lse)), (lnw + lsw) - (lne + lse));
    float dir_reduce = max((lnw + lne + lsw + lse) * (0.25 * 0.125), 1.0 / 128.0);
    float rcp = 1.0 / (min(abs(dir.x), abs(dir.y)) + dir_reduce);
    dir = clamp(dir * rcp, vec2(-8.0), vec2(8.0)) * u_texel;
    vec3 a = 0.5 * (texture(u_color, uv + dir * (1.0 / 3.0 - 0.5)).rgb
                  + texture(u_color, uv + dir * (2.0 / 3.0 - 0.5)).rgb);
    vec3 b = a * 0.5 + 0.25 * (texture(u_color, uv + dir * -0.5).rgb
                             + texture(u_color, uv + dir * 0.5).rgb);
    float lb = luma(b);
    return (lb < lmin || lb > lmax) ? a : b;
}

float hash(vec2 p) {
    return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453);
}
float vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), f.x),
               mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), f.x), f.y);
}

// Depth silhouettes (the nearer side of a jump) and normal creases, 0..1.
float edge_amount(vec2 uv) {
    vec2 px = u_texel * u_edge_px;
    vec3 c = view_pos(uv);
    vec3 r = view_pos(uv + vec2(px.x, 0.0));
    vec3 l = view_pos(uv - vec2(px.x, 0.0));
    vec3 u = view_pos(uv + vec2(0.0, px.y));
    vec3 d = view_pos(uv - vec2(0.0, px.y));
    if (texture(u_depth, uv).r >= 1.0) {
        // Sky: only the far side of a silhouette, which the model's own pixels cover.
        return 0.0;
    }
    vec3 dx = abs(r.z - c.z) < abs(l.z - c.z) ? r - c : c - l;
    vec3 dy = abs(u.z - c.z) < abs(d.z - c.z) ? u - c : c - d;
    vec3 n = normalize(cross(dx, dy));
    vec3 to_cam = u_ortho == 1 ? vec3(0.0, 0.0, 1.0) : -c;
    if (dot(n, to_cam) < 0.0) {
        n = -n;
    }
    // Neighbours lying behind this pixel's tangent plane: it is the near edge.
    float behind = max(max(-dot(r - c, n), -dot(l - c, n)), max(-dot(u - c, n), -dot(d - c, n)));
    float sil = smoothstep(1.0, 2.2, behind / (0.012 * abs(c.z) + 1.0));
    // Crease: the four quadrant normals around the pixel disagree.
    vec3 n_tr = normalize(cross(r - c, u - c));
    vec3 n_tl = normalize(cross(u - c, l - c));
    vec3 n_bl = normalize(cross(l - c, d - c));
    vec3 n_br = normalize(cross(d - c, r - c));
    float agree = min(min(dot(n_tr, n_bl), dot(n_tl, n_br)), min(dot(n_tr, n_tl), dot(n_bl, n_br)));
    float crease = smoothstep(0.12, 0.45, 1.0 - agree) * step(behind, 0.012 * abs(c.z) + 1.0);
    return clamp(max(sil, crease * 0.8), 0.0, 1.0);
}

void main() {
    vec2 uv = v_uv;
    vec3 col = u_fxaa == 1 ? fxaa(uv) : texture(u_color, uv).rgb;
    if (u_ao_strength > 0.0) {
        float ao = mix(1.0, texture(u_ao, uv).r, u_ao_strength);
        col *= ao;
    }
    if (u_wash > 0.0) {
        // Soft pigment wash: blur, lift, desaturate, step the tones.
        vec3 blur = 0.25 * (texture(u_color, uv + vec2(1.5, 0.0) * u_texel).rgb
                          + texture(u_color, uv - vec2(1.5, 0.0) * u_texel).rgb
                          + texture(u_color, uv + vec2(0.0, 1.5) * u_texel).rgb
                          + texture(u_color, uv - vec2(0.0, 1.5) * u_texel).rgb);
        col = mix(col, blur, 0.5 * u_wash);
        float l = luma(col);
        col = mix(vec3(l), col, 0.88);
        col = mix(col, vec3(1.0), 0.10 * u_wash);
        col = mix(col, floor(col * 9.0 + 0.5) / 9.0, 0.3 * u_wash);
    }
    if (u_edge_strength > 0.0) {
        vec2 wob = vec2(0.0);
        if (u_wash > 0.0) {
            // Hand-drawn wobble: shift where the edge is looked up.
            wob = (vec2(vnoise(gl_FragCoord.xy * 0.07), vnoise(gl_FragCoord.xy * 0.07 + 31.7)) - 0.5)
                  * u_texel * 3.0;
        }
        float e = edge_amount(uv + wob) * u_edge_strength;
        vec3 line = u_edge_color;
        col = mix(col, u_wash > 0.0 ? col * 0.55 + line * 0.3 : line, e);
    }
    if (u_wash > 0.0) {
        // Paper tooth: fine grain plus slow blotches.
        float grain = vnoise(gl_FragCoord.xy * 0.9) * 0.6 + vnoise(gl_FragCoord.xy * 0.18) * 0.4;
        col *= 1.0 - 0.09 * u_wash * grain;
        col += 0.03 * u_wash * (vnoise(gl_FragCoord.xy * 0.02) - 0.5);
    }
    f_color = vec4(clamp(col, 0.0, 1.0), 1.0);
}
"#;

/// The SSAO fragment shader source (depth helpers included).
pub(crate) fn ssao_fragment() -> String {
    format!("#ifdef GL_ES\nprecision highp float;\nprecision highp sampler2D;\n#endif\n{DEPTH_HELPERS}{SSAO_BODY}")
}

/// The AO blur fragment shader source.
pub(crate) fn blur_fragment() -> String {
    format!("#ifdef GL_ES\nprecision highp float;\nprecision highp sampler2D;\n#endif\n{DEPTH_HELPERS}{BLUR_BODY}")
}

/// The composite fragment shader source.
pub(crate) fn composite_fragment() -> String {
    format!("#ifdef GL_ES\nprecision highp float;\nprecision highp sampler2D;\n#endif\n{DEPTH_HELPERS}{COMPOSITE_BODY}")
}

/// Compile one shader stage, prefixing the version declaration.
pub(crate) fn compile(
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

/// A linked program with its uniform locations looked up by name once.
pub(crate) struct Prog {
    pub program: glow::Program,
    locs: HashMap<&'static str, glow::UniformLocation>,
}

impl Prog {
    /// Compile, link and look up `uniforms` by name.
    pub fn build(
        gl: &glow::Context,
        version: &str,
        (vertex, fragment): (&str, &str),
        attribs: &[(u32, &str)],
        uniforms: &[&'static str],
    ) -> Result<Prog, String> {
        unsafe {
            let vs = compile(gl, version, glow::VERTEX_SHADER, vertex)?;
            let fs = match compile(gl, version, glow::FRAGMENT_SHADER, fragment) {
                Ok(fs) => fs,
                Err(e) => {
                    gl.delete_shader(vs);
                    return Err(e);
                }
            };
            let program = gl.create_program()?;
            gl.attach_shader(program, vs);
            gl.attach_shader(program, fs);
            for (loc, name) in attribs {
                gl.bind_attrib_location(program, *loc, name);
            }
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
            // An array uniform is looked up by its plain name (element 0).
            let mut locs = HashMap::new();
            for name in uniforms {
                if let Some(l) = gl.get_uniform_location(program, name) {
                    locs.insert(*name, l);
                }
            }
            Ok(Prog { program, locs })
        }
    }

    /// Location of uniform `name`, if the program has it (and kept it).
    pub fn loc(&self, name: &str) -> Option<&glow::UniformLocation> {
        self.locs.get(name)
    }

    /// Set an `int` / `sampler` uniform (the program must be in use).
    pub fn i1(&self, gl: &glow::Context, name: &str, v: i32) {
        unsafe { gl.uniform_1_i32(self.loc(name), v) }
    }

    /// Set a `float` uniform.
    pub fn f1(&self, gl: &glow::Context, name: &str, v: f32) {
        unsafe { gl.uniform_1_f32(self.loc(name), v) }
    }

    /// Set a `vec2` uniform.
    pub fn f2(&self, gl: &glow::Context, name: &str, v: [f32; 2]) {
        unsafe { gl.uniform_2_f32(self.loc(name), v[0], v[1]) }
    }

    /// Set a `vec3` uniform or a `vec3[]` from a flat slice.
    pub fn f3(&self, gl: &glow::Context, name: &str, v: &[f32]) {
        unsafe { gl.uniform_3_f32_slice(self.loc(name), v) }
    }

    /// Set a `vec4` uniform.
    pub fn f4(&self, gl: &glow::Context, name: &str, v: [f32; 4]) {
        unsafe { gl.uniform_4_f32_slice(self.loc(name), &v) }
    }

    /// Set a column-major `mat4` uniform.
    pub fn m4(&self, gl: &glow::Context, name: &str, v: &[f32; 16]) {
        unsafe { gl.uniform_matrix_4_f32_slice(self.loc(name), false, v) }
    }

    /// Delete the program.
    pub fn delete(&self, gl: &glow::Context) {
        unsafe { gl.delete_program(self.program) }
    }
}

/// A 2D texture of the given format with clamped edges.
unsafe fn make_texture(
    gl: &glow::Context,
    (w, h): (i32, i32),
    (internal, format, ty): (u32, u32, u32),
    filter: u32,
) -> Result<glow::Texture, String> {
    unsafe {
        let tex = gl.create_texture()?;
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(tex));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            internal as i32,
            w,
            h,
            0,
            format,
            ty,
            glow::PixelUnpackData::Slice(None),
        );
        for (p, v) in [
            (glow::TEXTURE_MIN_FILTER, filter),
            (glow::TEXTURE_MAG_FILTER, filter),
            (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
            (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
        ] {
            gl.tex_parameter_i32(glow::TEXTURE_2D, p, v as i32);
        }
        gl.bind_texture(glow::TEXTURE_2D, None);
        Ok(tex)
    }
}

const COLOR_RGBA8: (u32, u32, u32) = (glow::RGBA8, glow::RGBA, glow::UNSIGNED_BYTE);
const DEPTH24: (u32, u32, u32) = (
    glow::DEPTH_COMPONENT24,
    glow::DEPTH_COMPONENT,
    glow::UNSIGNED_INT,
);

/// A framebuffer with colour and depth textures.
pub(crate) struct SceneTarget {
    pub fbo: glow::Framebuffer,
    pub color: glow::Texture,
    pub depth: glow::Texture,
    pub size: (i32, i32),
}

/// Half-resolution occlusion targets (raw and blurred).
pub(crate) struct AoTarget {
    pub raw_fbo: glow::Framebuffer,
    pub raw: glow::Texture,
    pub blur_fbo: glow::Framebuffer,
    pub blur: glow::Texture,
    pub size: (i32, i32),
}

/// The shadow map's depth-only framebuffer.
pub(crate) struct ShadowTarget {
    pub fbo: glow::Framebuffer,
    pub depth: glow::Texture,
    pub size: i32,
}

/// All offscreen targets, created on demand and resized with the viewport.
#[derive(Default)]
pub(crate) struct Targets {
    pub scene: Option<SceneTarget>,
    pub ao: Option<AoTarget>,
    pub shadow: Option<ShadowTarget>,
    /// Bound while drawing the vertex-less full-screen triangle.
    pub empty_vao: Option<glow::VertexArray>,
}

unsafe fn attach(
    gl: &glow::Context,
    fbo: glow::Framebuffer,
    color: Option<glow::Texture>,
    depth: Option<glow::Texture>,
) -> Result<(), String> {
    unsafe {
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
        if let Some(c) = color {
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(c),
                0,
            );
        } else {
            gl.draw_buffers(&[glow::NONE]);
            gl.read_buffer(glow::NONE);
        }
        if let Some(d) = depth {
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::DEPTH_ATTACHMENT,
                glow::TEXTURE_2D,
                Some(d),
                0,
            );
        }
        let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        if status == glow::FRAMEBUFFER_COMPLETE {
            Ok(())
        } else {
            Err(format!("framebuffer incomplete (status {status:#x})"))
        }
    }
}

impl Targets {
    /// The scene target for a window of `size`, (re)created when the size changed.
    pub fn ensure_scene(
        &mut self,
        gl: &glow::Context,
        size: (i32, i32),
    ) -> Result<&SceneTarget, String> {
        if self.scene.as_ref().is_some_and(|s| s.size != size) {
            self.free_scene(gl);
        }
        if self.scene.is_none() {
            unsafe {
                let color = make_texture(gl, size, COLOR_RGBA8, glow::LINEAR)?;
                let depth = make_texture(gl, size, DEPTH24, glow::NEAREST)?;
                let fbo = gl.create_framebuffer()?;
                let made = attach(gl, fbo, Some(color), Some(depth));
                if let Err(e) = made {
                    gl.delete_framebuffer(fbo);
                    gl.delete_texture(color);
                    gl.delete_texture(depth);
                    return Err(e);
                }
                self.scene = Some(SceneTarget {
                    fbo,
                    color,
                    depth,
                    size,
                });
            }
        }
        self.scene.as_ref().ok_or_else(|| "no scene target".into())
    }

    /// The half-resolution occlusion targets for a window of `size`.
    pub fn ensure_ao(&mut self, gl: &glow::Context, size: (i32, i32)) -> Result<&AoTarget, String> {
        let half = ((size.0 / 2).max(1), (size.1 / 2).max(1));
        if self.ao.as_ref().is_some_and(|a| a.size != half) {
            self.free_ao(gl);
        }
        if self.ao.is_none() {
            unsafe {
                let raw = make_texture(gl, half, COLOR_RGBA8, glow::LINEAR)?;
                let blur = make_texture(gl, half, COLOR_RGBA8, glow::LINEAR)?;
                let raw_fbo = gl.create_framebuffer()?;
                let blur_fbo = gl.create_framebuffer()?;
                let made = attach(gl, raw_fbo, Some(raw), None)
                    .and_then(|()| attach(gl, blur_fbo, Some(blur), None));
                if let Err(e) = made {
                    gl.delete_framebuffer(raw_fbo);
                    gl.delete_framebuffer(blur_fbo);
                    gl.delete_texture(raw);
                    gl.delete_texture(blur);
                    return Err(e);
                }
                self.ao = Some(AoTarget {
                    raw_fbo,
                    raw,
                    blur_fbo,
                    blur,
                    size: half,
                });
            }
        }
        self.ao.as_ref().ok_or_else(|| "no occlusion target".into())
    }

    /// The shadow map target of `size` texels per side.
    pub fn ensure_shadow(
        &mut self,
        gl: &glow::Context,
        size: i32,
    ) -> Result<&ShadowTarget, String> {
        if self.shadow.as_ref().is_some_and(|s| s.size != size) {
            self.free_shadow(gl);
        }
        if self.shadow.is_none() {
            unsafe {
                let depth = make_texture(gl, (size, size), DEPTH24, glow::NEAREST)?;
                let fbo = gl.create_framebuffer()?;
                if let Err(e) = attach(gl, fbo, None, Some(depth)) {
                    gl.delete_framebuffer(fbo);
                    gl.delete_texture(depth);
                    return Err(e);
                }
                self.shadow = Some(ShadowTarget { fbo, depth, size });
            }
        }
        self.shadow
            .as_ref()
            .ok_or_else(|| "no shadow target".into())
    }

    /// The VAO to bind for full-screen triangles.
    pub fn fullscreen_vao(&mut self, gl: &glow::Context) -> Option<glow::VertexArray> {
        if self.empty_vao.is_none() {
            self.empty_vao = unsafe { gl.create_vertex_array() }.ok();
        }
        self.empty_vao
    }

    fn free_scene(&mut self, gl: &glow::Context) {
        if let Some(s) = self.scene.take() {
            unsafe {
                gl.delete_framebuffer(s.fbo);
                gl.delete_texture(s.color);
                gl.delete_texture(s.depth);
            }
        }
    }

    fn free_ao(&mut self, gl: &glow::Context) {
        if let Some(a) = self.ao.take() {
            unsafe {
                gl.delete_framebuffer(a.raw_fbo);
                gl.delete_framebuffer(a.blur_fbo);
                gl.delete_texture(a.raw);
                gl.delete_texture(a.blur);
            }
        }
    }

    fn free_shadow(&mut self, gl: &glow::Context) {
        if let Some(s) = self.shadow.take() {
            unsafe {
                gl.delete_framebuffer(s.fbo);
                gl.delete_texture(s.depth);
            }
        }
    }

    /// Free every GL object.
    pub fn destroy(&mut self, gl: &glow::Context) {
        self.free_scene(gl);
        self.free_ao(gl);
        self.free_shadow(gl);
        if let Some(v) = self.empty_vao.take() {
            unsafe { gl.delete_vertex_array(v) };
        }
    }

    /// Forget every GL object without deleting it (the context is gone).
    pub fn forget(&mut self) {
        *self = Targets::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composite_and_ssao_sources_are_complete() {
        let frag = composite_fragment();
        for name in [
            "u_color",
            "u_depth",
            "u_ao",
            "u_inv_proj",
            "u_fxaa",
            "u_edge_strength",
            "u_wash",
            "u_ao_strength",
        ] {
            assert!(frag.contains(name), "composite lacks {name}");
        }
        let ssao = ssao_fragment();
        for name in ["u_kernel", "u_proj", "u_radius", "u_bias", "bayer4"] {
            assert!(ssao.contains(name), "ssao lacks {name}");
        }
        assert!(blur_fragment().contains("u_ao"));
        assert!(SKY_FRAGMENT.contains("u_sky_top"));
        assert!(FULLSCREEN_VERTEX.contains("gl_VertexID"));
    }
}
