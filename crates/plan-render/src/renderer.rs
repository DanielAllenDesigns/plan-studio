//! Scene preparation, multi-threaded sample accumulation and progressive output.

use crate::bvh::{Bvh, Tri};
use crate::camera::Camera;
use crate::denoise::bilateral;
use crate::image::Image;
use crate::integrator::Frame;
use crate::lighting::{Environment, PointLight};
use crate::png::write_png;
use crate::settings::RenderSettings;
use crate::vec3::V3;
use plan_3d::{build_scene, Scene};
use plan_core::Project;
use std::io;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

/// Progress callback: running image and samples done; return `false` to stop.
pub type ProgressFn<'a> = dyn FnMut(&Image, u32) -> bool + 'a;

/// Rows of pixels handed to a worker at a time.
const BAND_ROWS: usize = 4;

/// A path tracer bound to one scene (the BVH is built once and reused).
#[derive(Debug)]
pub struct Renderer {
    bvh: Bvh,
    diagonal: f32,
}

impl Renderer {
    /// Flatten every mesh triangle into a BVH. Degenerate triangles are dropped.
    pub fn new(scene: &Scene) -> Renderer {
        let mut tris = Vec::with_capacity(scene.triangle_count());
        for mesh in &scene.meshes {
            let material = mesh.material.index() as u32;
            let vertex = |i: u32| V3::from_array(mesh.vertices[i as usize].position);
            for t in mesh.indices.as_chunks::<3>().0 {
                tris.extend(Tri::new(vertex(t[0]), vertex(t[1]), vertex(t[2]), material));
            }
        }
        let diagonal = scene.bounds().map_or(0.0, |(lo, hi)| {
            (V3::from_array(hi) - V3::from_array(lo)).length()
        });
        Renderer {
            bvh: Bvh::build(tris),
            diagonal,
        }
    }

    /// Number of triangles in the acceleration structure.
    pub fn triangle_count(&self) -> usize {
        self.bvh.tris.len()
    }

    /// Render a complete image.
    pub fn render(
        &self,
        cam: &Camera,
        env: &Environment,
        lights: &[PointLight],
        settings: &RenderSettings,
    ) -> Image {
        self.run(cam, env, lights, settings, None)
    }

    /// Render in passes of growing size (1, 1, 2, 4, ... samples), calling
    /// `callback` with the running image and the samples done after each.
    /// Return `false` from the callback to stop early; the image so far is returned.
    ///
    /// Completed runs equal [`Renderer::render`] exactly.
    pub fn render_progressive(
        &self,
        cam: &Camera,
        env: &Environment,
        lights: &[PointLight],
        settings: &RenderSettings,
        callback: &mut ProgressFn<'_>,
    ) -> Image {
        self.run(cam, env, lights, settings, Some(callback))
    }

    fn run(
        &self,
        cam: &Camera,
        env: &Environment,
        lights: &[PointLight],
        settings: &RenderSettings,
        mut callback: Option<&mut ProgressFn<'_>>,
    ) -> Image {
        let frame = Frame::new(&self.bvh, self.diagonal, cam, env, lights, settings);
        let (w, h) = (
            settings.width.max(1) as usize,
            settings.height.max(1) as usize,
        );
        let threads = if settings.threads == 0 {
            std::thread::available_parallelism().map_or(1, |n| n.get())
        } else {
            settings.threads
        };
        let total = settings.samples.max(1);
        let mut acc = vec![[0.0_f64; 3]; w * h];
        let mut done = 0;
        while done < total {
            let pass = match callback {
                Some(_) => done.clamp(1, total - done),
                None => total,
            };
            accumulate(&frame, &mut acc, w, (done, pass), threads);
            done += pass;
            let image = finish(&acc, (w, h), done, settings);
            if let Some(cb) = callback.as_deref_mut() {
                if !cb(&image, done) || done == total {
                    return image;
                }
            } else if done == total {
                return image;
            }
        }
        finish(&acc, (w, h), total, settings)
    }
}

/// Add samples `first .. first + count` for every pixel, bands spread over threads.
fn accumulate(
    frame: &Frame<'_>,
    acc: &mut [[f64; 3]],
    width: usize,
    (first, count): (u32, u32),
    threads: usize,
) {
    let bands = Mutex::new(acc.chunks_mut(width * BAND_ROWS).enumerate());
    let workers = threads.max(1);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let next = bands.lock().unwrap_or_else(PoisonError::into_inner).next();
                let Some((band, pixels)) = next else { break };
                let start = band * BAND_ROWS * width;
                for (i, slot) in pixels.iter_mut().enumerate() {
                    let (x, y) = ((start + i) % width, (start + i) / width);
                    for s in first..first + count {
                        let c = frame.sample(x as u32, y as u32, s);
                        slot[0] += f64::from(c.x);
                        slot[1] += f64::from(c.y);
                        slot[2] += f64::from(c.z);
                    }
                }
            });
        }
    });
}

/// Average the accumulator, optionally denoise and tone map.
fn finish(
    acc: &[[f64; 3]],
    (w, h): (usize, usize),
    samples: u32,
    settings: &RenderSettings,
) -> Image {
    let inv = 1.0 / f64::from(samples.max(1));
    let mut hdr: Vec<[f32; 3]> = acc.iter().map(|a| a.map(|v| (v * inv) as f32)).collect();
    if settings.denoise {
        hdr = bilateral(&hdr, w, h);
    }
    Image::from_hdr(
        w as u32,
        h as u32,
        hdr,
        settings.tone_map,
        settings.exposure,
    )
}

/// Build the project's 3D scene, render it under the default clear-day sky and
/// save a PNG.
pub fn render_to_file(
    project: &Project,
    cam: &Camera,
    settings: &RenderSettings,
    path: impl AsRef<Path>,
) -> io::Result<()> {
    let renderer = Renderer::new(&build_scene(project));
    let image = renderer.render(cam, &Environment::default(), &[], settings);
    write_png(path, &image)
}
