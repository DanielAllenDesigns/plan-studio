//! Record Walkthrough output: a Motion-JPEG `.avi` movie, a numbered PNG
//! sequence, or both, rendered with the path tracer and, for the Vector View,
//! Technical Illustration, Line Drawing and Watercolor techniques, run
//! through `plan_render::stylize` (`docs/parity/3d-views-cameras.md`, C-71).
//!
//! The frame loop is shared by the PNG sequence and the movie, so the same
//! poses are rendered once whatever is written. Stopping early still leaves a
//! valid movie (the index is written for the frames that exist).

use plan_3d::Scene;
use plan_core::camera_view::WalkRecord;
use plan_core::CameraObject;
use plan_render::{AviWriter, Image, Style};
use std::fs::File;
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};

/// Everything one recording needs besides where to put it.
pub struct RecordJob<'a> {
    pub scene: &'a Scene,
    pub cam: &'a CameraObject,
    /// Elevation of the camera's floor, inches.
    pub elevation: f64,
    pub lights: &'a [plan_render::PointLight],
    pub env: &'a plan_render::Environment,
    pub settings: &'a plan_render::RenderSettings,
    /// Frame rate, format and movie quality.
    pub walk: WalkRecord,
    /// A technique look to apply to every frame.
    pub style: Option<Style>,
}

/// What a recording wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recorded {
    /// Frames rendered.
    pub frames: usize,
    /// The movie, when one was asked for.
    pub video: Option<PathBuf>,
    /// False when the progress callback stopped it early.
    pub complete: bool,
}

/// The file name of a walkthrough's movie: the camera's name made safe, plus
/// `.avi`.
pub fn video_path(dir: &Path, camera_name: &str) -> PathBuf {
    let stem: String = camera_name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stem = stem.trim_matches('_');
    dir.join(format!(
        "{}.avi",
        if stem.is_empty() { "walkthrough" } else { stem }
    ))
}

/// The picture with `style` applied (the pixels only; alpha is kept).
pub fn styled(image: Image, style: Option<Style>) -> Image {
    match style {
        None => image,
        Some(s) => {
            let rgba = plan_render::stylize(&image.rgba, image.width, image.height, s);
            Image { rgba, ..image }
        }
    }
}

/// Renders the walkthrough and writes what `job.walk.format` asks for into
/// `out_dir`. `progress(done, total)` returns `false` to stop.
pub fn record(
    job: &RecordJob<'_>,
    out_dir: &Path,
    progress: &mut dyn FnMut(usize, usize) -> bool,
) -> io::Result<Recorded> {
    std::fs::create_dir_all(out_dir)?;
    let walk = job.walk.clamped();
    let (settings, cam) = (job.settings, job.cam);
    let renderer = plan_render::Renderer::new(job.scene);
    let total = super::walk_frame_count(cam, walk.fps);
    let aspect = settings.width as f32 / settings.height.max(1) as f32;
    let fov = super::vertical_fov(cam.fov_deg as f32, aspect);
    let duration = cam.walk_duration_s();
    let video = walk.format.video().then(|| video_path(out_dir, &cam.name));
    let mut avi = match &video {
        Some(path) => Some(AviWriter::new(
            BufWriter::new(File::create(path)?),
            settings.width,
            settings.height,
            walk.fps,
        )?),
        None => None,
    };
    let mut done = 0;
    let mut complete = true;
    let mut failure = None;
    for i in 0..total {
        // Frames are spread evenly in time from the first node to the last,
        // so the holds and tilts of the key frames show up as they play.
        let t = if total > 1 {
            duration * i as f64 / (total - 1) as f64
        } else {
            0.0
        };
        let pose = cam.walk_pose_at_time(t);
        let mut camera = plan_render::Camera::from_plan(
            pose.position,
            pose.direction_deg,
            job.elevation + pose.eye_height,
            f64::from(fov),
        );
        // The ray tracer's camera looks 100" ahead: raise or lower that point.
        camera.target[1] += (pose.tilt_deg.to_radians().tan() * 100.0) as f32;
        let image = styled(
            renderer.render(&camera, job.env, job.lights, settings),
            job.style,
        );
        let wrote = (|| -> io::Result<()> {
            if walk.format.frames() {
                std::fs::write(
                    out_dir.join(plan_view3d::export::frame_file_name(i + 1)),
                    plan_render::encode_png(&image),
                )?;
            }
            if let Some(a) = avi.as_mut() {
                a.add_image(&image, walk.quality)?;
            }
            Ok(())
        })();
        if let Err(e) = wrote {
            failure = Some(e);
            break;
        }
        done = i + 1;
        if !progress(done, total) {
            complete = false;
            break;
        }
    }
    // Close the movie even after a stop or a failed frame, so what exists plays.
    if let Some(a) = avi {
        a.finish().and_then(|mut w| io::Write::flush(&mut w))?;
    }
    if let Some(e) = failure {
        return Err(e);
    }
    if complete && walk.format.frames() {
        plan_view3d::export::write_ffmpeg_script(out_dir, walk.fps)?;
    }
    Ok(Recorded {
        frames: done,
        video,
        complete,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::camera_view::RecordFormat;
    use plan_core::geometry::Point;
    use plan_core::CameraKind;

    fn walker() -> CameraObject {
        let mut c = CameraObject::new(CameraKind::Walkthrough, Point::ZERO, 0.0, "Front Walk", 0);
        c.path = vec![Point::new(60.0, 40.0), Point::new(180.0, 40.0)];
        c.position = c.path[0];
        c.walk_speed = 120.0; // one second
        c
    }

    fn tiny() -> plan_render::RenderSettings {
        plan_render::RenderSettings {
            width: 32,
            height: 24,
            samples: 1,
            threads: 2,
            ..plan_render::RenderSettings::default()
        }
    }

    fn run(
        tag: &str,
        format: RecordFormat,
        style: Option<Style>,
        stop_after: usize,
    ) -> (Recorded, PathBuf) {
        let scene = Scene::default();
        let cam = walker();
        let env = plan_render::Environment::default();
        let settings = tiny();
        let dir = std::env::temp_dir().join(format!("plan_record_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let job = RecordJob {
            scene: &scene,
            cam: &cam,
            elevation: 0.0,
            lights: &[],
            env: &env,
            settings: &settings,
            walk: WalkRecord {
                fps: 4.0,
                format,
                quality: 80,
                ..WalkRecord::default()
            },
            style,
        };
        let rec = record(&job, &dir, &mut |d, _| d < stop_after).unwrap();
        (rec, dir)
    }

    #[test]
    fn a_video_is_one_avi_of_every_frame_and_no_pngs() {
        let (rec, dir) = run("video", RecordFormat::Video, None, usize::MAX);
        assert_eq!((rec.frames, rec.complete), (4, true));
        let path = rec.video.expect("a movie");
        assert_eq!(path.file_name().unwrap(), "Front_Walk.avi");
        let bytes = std::fs::read(&path).unwrap();
        let info = plan_render::read_avi(&bytes).unwrap();
        assert_eq!((info.width, info.height, info.frames.len()), (32, 24, 4));
        assert_eq!(info.fps, 4.0);
        for i in 0..4 {
            let f = plan_library::image::jpeg::decode(info.frame(&bytes, i).unwrap()).unwrap();
            assert_eq!((f.width, f.height), (32, 24));
        }
        assert!(!dir.join("frame_0001.png").exists());
        assert!(!dir.join("make_video.sh").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn both_writes_the_movie_the_frames_and_the_script() {
        let (rec, dir) = run("both", RecordFormat::Both, None, usize::MAX);
        assert!(rec.video.as_ref().is_some_and(|p| p.exists()));
        assert!(dir.join("frame_0004.png").exists());
        assert!(dir.join("make_video.sh").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stopping_early_leaves_a_playable_movie() {
        let (rec, dir) = run("stop", RecordFormat::Video, None, 2);
        assert_eq!((rec.frames, rec.complete), (2, false));
        let bytes = std::fs::read(rec.video.unwrap()).unwrap();
        assert_eq!(plan_render::read_avi(&bytes).unwrap().frames.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_technique_look_is_applied_to_the_frames() {
        // An empty scene is just sky; Line Drawing turns it white.
        let (_, plain) = run("plain", RecordFormat::Frames, None, usize::MAX);
        let (_, line) = run(
            "line",
            RecordFormat::Frames,
            Some(Style::LineDrawing),
            usize::MAX,
        );
        let read = |d: &Path| std::fs::read(d.join("frame_0001.png")).unwrap();
        assert_ne!(read(&plain), read(&line), "the look changes the picture");
        let _ = std::fs::remove_dir_all(&plain);
        let _ = std::fs::remove_dir_all(&line);
    }

    #[test]
    fn movie_names_are_made_safe() {
        let d = Path::new("/tmp/x");
        assert_eq!(
            video_path(d, "Kitchen / Hall").file_name().unwrap(),
            "Kitchen___Hall.avi"
        );
        assert_eq!(video_path(d, "  ").file_name().unwrap(), "walkthrough.avi");
        assert_eq!(
            video_path(d, "../..").file_name().unwrap(),
            "walkthrough.avi"
        );
    }
}
