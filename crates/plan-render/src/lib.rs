//! plan-render: a dependency-free CPU path tracer for Chief's "Physically Based"
//! and "Clay" rendering techniques (parity items C-45, C-51, C-52, C-62..C-64).
//!
//! A [`plan_3d::Scene`] is flattened into a BVH by [`Renderer::new`]; a render is
//! then a pure function of the camera, [`Environment`], [`PointLight`]s and
//! [`RenderSettings`] (including the seed), so the same inputs always give the
//! same bytes, whatever the thread count. Output is an [`Image`] (tone-mapped
//! RGBA8 plus the linear HDR buffer) that [`write_png`] saves without any
//! compression library.
//!
//! Scene frame: X right, Y up (inches), Z toward the viewer (plan `y` is `-Z`).

mod albedo;
mod avi;
mod bvh;
mod camera;
mod denoise;
mod image;
mod integrator;
mod jpeg;
mod lighting;
mod panorama;
mod pbrmap;
mod png;
mod renderer;
mod rng;
mod settings;
mod shading;
mod sky;
mod style;
mod tonemap;
mod vec3;

pub use avi::{read_avi, AviInfo, AviWriter};
pub use camera::Camera;
pub use image::Image;
pub use jpeg::encode_jpeg;
pub use lighting::{AreaLight, Environment, PointLight, Sun};
pub use panorama::{
    base64, panorama_camera, panorama_direction, panorama_position, panorama_settings, viewer_html,
    write_panorama, PanoramaFiles, PANORAMA_WIDTHS,
};
pub use png::{encode_png, write_png};
pub use renderer::{render_to_file, ProgressFn, Renderer};
pub use settings::{Projection, RenderSettings, Technique, MAX_PIXELS, MAX_SIDE};
pub use sky::SkyModel;
pub use style::{edge_strength, stylize, Style};
pub use tonemap::ToneMap;
