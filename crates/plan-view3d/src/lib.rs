//! plan-view3d: a reusable egui widget that renders a [`plan_3d::Scene`] with
//! OpenGL through eframe's glow backend.
//!
//! It provides Chief-style camera views (see [`standard_views`]): an orbiting
//! perspective overview, a "doll house" view with ceilings and roof hidden, a
//! first-person walkthrough, and orthographic elevations for drawing work.
//!
//! Textures: textured materials (brick, siding, shingles, wood...) are mapped
//! in the fragment shader from world position and face orientation
//! ([`planar_uv`]), so meshes carry no extra UVs; pictures draw their bitmap on
//! their quad ([`ImageTexture`]). Textures upload on demand when a material
//! first appears, are re-uploaded after a lost GL context, and turn off with
//! [`Viewport3d::textures_enabled`].
//!
//! The camera and matrix math ([`camera`], [`math`]) and edge extraction
//! ([`edges`]) are plain Rust and unit tested; every OpenGL call is isolated in
//! the private `gpu` module.

pub mod backdrop;
pub mod camera;
pub mod edges;
pub mod export;
mod gpu;
pub mod math;
mod pipeline;
pub mod quality;
mod texturing;
mod viewport;
pub mod walkthrough;

pub use backdrop::{BackdropImage, Fog, Ground};
pub use camera::{standard_views, Camera, CameraMode};
pub use quality::{
    nearest_lights, shadow_map, ssao_kernel, tone_map, Look, LookParams, Quality, ShadowMap,
    ViewLight, ViewSettings, MAX_AO_SAMPLES, MAX_POINT_LIGHTS,
};
pub use texturing::{
    glsl_planar_uv, needed_materials, next_uploads, pending_uploads, planar_uv, projection,
    ImageTexture, Projection, SurfaceTexture, MAX_PICTURE_SIDE, MAX_UPLOADS_PER_FRAME,
};
pub use viewport::{Lighting, Viewport3d};
pub use walkthrough::{preview_poses, KeyFrame, Walkthrough, WalkthroughPath, WalkthroughSource};
