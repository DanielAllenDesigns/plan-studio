//! plan-view3d: a reusable egui widget that renders a [`plan_3d::Scene`] with
//! OpenGL through eframe's glow backend.
//!
//! It provides Chief-style camera views (see [`standard_views`]): an orbiting
//! perspective overview, a "doll house" view with ceilings and roof hidden, a
//! first-person walkthrough, and orthographic elevations for drawing work.
//!
//! The camera and matrix math ([`camera`], [`math`]) and edge extraction
//! ([`edges`]) are plain Rust and unit tested; every OpenGL call is isolated in
//! the private `gpu` module.

pub mod camera;
pub mod edges;
mod gpu;
pub mod math;
mod viewport;

pub use camera::{standard_views, Camera, CameraMode};
pub use viewport::{Lighting, Viewport3d};
