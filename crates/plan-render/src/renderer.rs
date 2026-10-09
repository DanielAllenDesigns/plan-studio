//! Scene preparation, multi-threaded sample accumulation and progressive output.

use crate::bvh::{Bvh, Tri};
use crate::camera::Camera;
use crate::denoise::{bilateral, guided, Guides};
use crate::image::Image;
use crate::integrator::Frame;
use crate::lighting::{AreaLight, Environment, PointLight};
use crate::png::write_png;
use crate::settings::RenderSettings;
use crate::settings::Technique;
use crate::shading::{Custom, MATERIAL_COUNT};
use crate::vec3::V3;
use plan_3d::{build_scene, Scene};
use plan_core::Project;
use plan_materials::textures::TextureStore;
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

/// Progress callback: running image and samples done; return `false` to stop.
pub type ProgressFn<'a> = dyn FnMut(&Image, u32) -> bool + 'a;

/// Rows of pixels handed to a worker at a time.
const BAND_ROWS: usize = 4;

/// A path tracer bound to one scene (the BVH is built once and reused).
#[derive(Debug)]
pub struct Renderer {
    bvh: Bvh,
    diagonal: f32,
    textures: Arc<TextureStore>,
    /// The looks of meshes with their own `color` (and the surface the
    /// Material Painter gave them); a triangle's material index
    /// `MATERIAL_COUNT + i` means `custom[i]`.
    custom: Vec<Custom>,
}

impl Renderer {
    /// Flatten every mesh triangle into a BVH. Degenerate triangles are dropped.
    /// A mesh with a [`plan_3d::Mesh::color`] is shaded with that colour as its
    /// albedo in place of its material's, and with the roughness, metalness,
    /// transparency and glow the Material Painter gave it
    /// ([`plan_3d::Mesh::paint_surface`]).
    pub fn new(scene: &Scene) -> Renderer {
        let mut tris = Vec::with_capacity(scene.triangle_count());
        let mut custom: Vec<Custom> = Vec::new();
        for mesh in &scene.meshes {
            let material = match mesh.color {
                None => mesh.material.index() as u32,
                Some(rgb) => {
                    let key = Custom {
                        material: mesh.material,
                        color: rgb,
                        paint: mesh.paint_surface(),
                        maps: plan_materials::pbr::lookup(
                            mesh.object_id,
                            mesh.material,
                            mesh.color,
                        )
                        .map(|src| src.key()),
                    };
                    let at = custom.iter().position(|c| *c == key).unwrap_or_else(|| {
                        custom.push(key);
                        custom.len() - 1
                    });
                    (MATERIAL_COUNT + at) as u32
                }
            };
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
            textures: TextureStore::shared(),
            custom,
        }
    }

    /// Use `store` for texture lookups instead of the process-wide one
    /// (tests use a store without Chief's files).
    pub fn with_texture_store(mut self, store: Arc<TextureStore>) -> Renderer {
        self.textures = store;
        self
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
        self.run(cam, env, (lights, &[]), settings, None)
    }

    /// [`Renderer::render`] with rectangular area lights as well.
    pub fn render_with_areas(
        &self,
        cam: &Camera,
        env: &Environment,
        lights: (&[PointLight], &[AreaLight]),
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
        self.run(cam, env, (lights, &[]), settings, Some(callback))
    }

    /// [`Renderer::render_progressive`] with rectangular area lights as well.
    pub fn render_progressive_with_areas(
        &self,
        cam: &Camera,
        env: &Environment,
        lights: (&[PointLight], &[AreaLight]),
        settings: &RenderSettings,
        callback: &mut ProgressFn<'_>,
    ) -> Image {
        self.run(cam, env, lights, settings, Some(callback))
    }

    fn run(
        &self,
        cam: &Camera,
        env: &Environment,
        lights: (&[PointLight], &[AreaLight]),
        settings: &RenderSettings,
        mut callback: Option<&mut ProgressFn<'_>>,
    ) -> Image {
        let frame = Frame::new(
            &self.bvh,
            self.diagonal,
            cam,
            env,
            lights,
            settings,
            (&self.textures, &self.custom),
        );
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
        if settings.preview_blocks {
            if let Some(cb) = callback.as_deref_mut() {
                let preview = block_preview(&frame, (w, h), threads, settings);
                if !cb(&preview, 0) {
                    return preview;
                }
            }
        }
        // Guide buffers for the denoiser: one centre ray per pixel, once.
        let guides = (settings.denoise && settings.technique != Technique::Ambient)
            .then(|| guide_buffers(&frame, (w, h), threads));
        while done < total {
            let pass = match callback {
                Some(_) => done.clamp(1, total - done),
                None => total,
            };
            accumulate(&frame, &mut acc, w, (done, pass), threads);
            done += pass;
            // The guided filter is slow on big images: intermediate previews of
            // those stay raw and only the finished image is denoised.
            let denoise = done == total || w * h <= 400_000;
            let image = finish(
                &acc,
                (w, h),
                done,
                settings,
                guides.as_ref().filter(|_| denoise),
            );
            if let Some(cb) = callback.as_deref_mut() {
                if !cb(&image, done) || done == total {
                    return image;
                }
            } else if done == total {
                return image;
            }
        }
        finish(&acc, (w, h), total, settings, guides.as_ref())
    }
}

/// Albedo, normal and depth of the first hit per pixel.
struct GuideBuffers {
    albedo: Vec<[f32; 3]>,
    normal: Vec<[f32; 3]>,
    depth: Vec<f32>,
}

/// Run `f(x, y)` for every cell of a `width`-wide grid, rows spread over threads.
fn par_grid<T: Send + Clone>(
    cells: &mut [T],
    width: usize,
    threads: usize,
    f: &(dyn Fn(usize, usize) -> T + Sync),
) {
    let rows = Mutex::new(cells.chunks_mut(width.max(1)).enumerate());
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| loop {
                let next = rows.lock().unwrap_or_else(PoisonError::into_inner).next();
                let Some((y, row)) = next else { break };
                for (x, cell) in row.iter_mut().enumerate() {
                    *cell = f(x, y);
                }
            });
        }
    });
}

fn guide_buffers(frame: &Frame<'_>, (w, h): (usize, usize), threads: usize) -> GuideBuffers {
    let mut cells = vec![(V3::ZERO, V3::ZERO, 0.0_f32); w * h];
    par_grid(&mut cells, w, threads, &|x, y| {
        let g = frame.guide(x as u32, y as u32);
        (g.albedo, g.normal, g.depth)
    });
    GuideBuffers {
        albedo: cells.iter().map(|c| [c.0.x, c.0.y, c.0.z]).collect(),
        normal: cells.iter().map(|c| [c.1.x, c.1.y, c.1.z]).collect(),
        depth: cells.iter().map(|c| c.2).collect(),
    }
}

/// Size in pixels of the blocks of the quick first preview.
const PREVIEW_BLOCK: usize = 4;

/// One sample per `PREVIEW_BLOCK` x `PREVIEW_BLOCK` block, replicated: a
/// coarse picture available long before the first full pass.
fn block_preview(
    frame: &Frame<'_>,
    (w, h): (usize, usize),
    threads: usize,
    settings: &RenderSettings,
) -> Image {
    let (bw, bh) = (w.div_ceil(PREVIEW_BLOCK), h.div_ceil(PREVIEW_BLOCK));
    let mut cells = vec![V3::ZERO; bw * bh];
    par_grid(&mut cells, bw, threads, &|bx, by| {
        let x = (bx * PREVIEW_BLOCK + PREVIEW_BLOCK / 2).min(w - 1);
        let y = (by * PREVIEW_BLOCK + PREVIEW_BLOCK / 2).min(h - 1);
        frame.sample(x as u32, y as u32, u32::MAX)
    });
    let mut hdr = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let c = cells[(y / PREVIEW_BLOCK) * bw + x / PREVIEW_BLOCK];
            hdr.push([c.x, c.y, c.z]);
        }
    }
    Image::from_hdr(
        w as u32,
        h as u32,
        hdr,
        settings.tone_map,
        settings.exposure,
    )
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
    guides: Option<&GuideBuffers>,
) -> Image {
    let inv = 1.0 / f64::from(samples.max(1));
    let mut hdr: Vec<[f32; 3]> = acc.iter().map(|a| a.map(|v| (v * inv) as f32)).collect();
    if settings.denoise && guides.is_some() {
        hdr = match guides {
            Some(g) => guided(
                &hdr,
                &Guides {
                    albedo: &g.albedo,
                    normal: &g.normal,
                    depth: &g.depth,
                },
                w,
                h,
            ),
            None => bilateral(&hdr, w, h),
        };
    } else if settings.denoise && settings.technique == Technique::Ambient {
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
