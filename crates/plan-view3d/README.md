# plan-view3d

An egui widget that draws a `plan_3d::Scene` with OpenGL through eframe's glow backend.
Views follow Chief Architect: Perspective Full Overview (orbit), Doll House (ceiling and roof hidden), Full Camera (first-person at 66" eye height), orthographic Front/Back/Left/Right elevations, and Plan Overhead. `standard_views()` lists them by name.
Embed it by keeping a `Viewport3d` in your app, calling `set_scene(gl, &scene)` once a GL context exists (`CreationContext::gl`, or `queue_scene` to upload on the next paint), and `viewport.ui(ui, size)` each frame.
Left-drag orbits, right/middle-drag pans, scroll zooms; in Full Camera left-drag turns and W/A/S/D or the arrows walk; elevations pan and zoom.
Run eframe with `NativeOptions { depth_buffer: 24, .. }`, otherwise there is no depth test, and call `destroy(gl)` from `App::on_exit`.
Draw order is opaque first, then translucent meshes (window glass) back to front; edges are an optional dark line overlay (`show_edges`).
Lighting (`quality`, `pipeline`, `gpu`): the Standard shader shades with a sun, fill, sky-tinted ambient, up to 8 nearest point lights (`set_point_lights`), GGX highlights, Fresnel sky reflections and metalness from `plan_materials::scene_surface`, then a soft-shoulder tone curve.
Each frame is a shadow map from the sun (fitted to the scene, 1024/2048/4096 by quality, PCF 1/3x3/5x5, re-rendered only when the scene or sun changes), the scene into an offscreen target, half-resolution SSAO with a 4x4 blur, and one composite pass (FXAA, occlusion, depth/normal edge lines, watercolor wash and paper grain).
`Viewport3d::settings` (`ViewSettings`: shadows, ambient occlusion, `Quality`, exposure) and `Viewport3d::look` (`Look`: Standard, Physical, Clay, GlassHouse, Watercolor, Technical, Duotone, Flat) drive it; looks without sun shadows or occlusion skip those passes. A pass whose shader or target fails turns itself off and says why in `render_notes()`.
Needs GLSL 1.40 or ES 3.00; if setup fails, `gl_error()` explains why and the viewport stays blank.
`camera`, `math`, `edges` and `quality` (shadow-map fit, AO kernel, tone curve, settings) are plain Rust and unit tested; all OpenGL lives in the private `gpu` and `pipeline` modules. `cargo test -p plan-view3d headless -- --ignored --nocapture` runs them against a real offscreen macOS GL context (`PLAN_GL_DUMP=<dir>` writes PPMs).

```rust
use eframe::egui;
use plan_view3d::{CameraMode, Viewport3d};

struct App { viewport: Viewport3d }

impl App {
    fn new(cc: &eframe::CreationContext<'_>, scene: &plan_3d::Scene) -> Self {
        let mut viewport = Viewport3d::new();
        if let Some(gl) = cc.gl.as_ref() { viewport.set_scene(gl, scene); }
        viewport.set_mode(CameraMode::DollHouse);
        Self { viewport }
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| { let size = ui.available_size(); self.viewport.ui(ui, size); });
    }
    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) { if let Some(gl) = gl { self.viewport.destroy(gl); } }
}
```
