//! Recording a [`Walkthrough`] to numbered PNG frames with `plan-render`,
//! plus a helper script that assembles them into an mp4 with `ffmpeg`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use plan_3d::Scene;
use plan_render::{encode_png, Environment, RenderSettings, Renderer};

use crate::walkthrough::Walkthrough;

/// Name of the shell script written by [`write_ffmpeg_script`].
pub const FFMPEG_SCRIPT_NAME: &str = "make_video.sh";

/// File name of frame `number` (1-based): `frame_0001.png`.
pub fn frame_file_name(number: usize) -> String {
    format!("frame_{number:04}.png")
}

/// Render every frame of `wt` into `out_dir` as `frame_0001.png`, ...
///
/// `progress(done, total)` is called after each frame is written; return
/// `false` to stop early. Returns the number of frames written. The output
/// directory is created if needed.
pub fn render_frames(
    wt: &Walkthrough,
    scene: &Scene,
    settings: &RenderSettings,
    out_dir: &Path,
    progress: &mut dyn FnMut(usize, usize) -> bool,
) -> io::Result<usize> {
    fs::create_dir_all(out_dir)?;
    let renderer = Renderer::new(scene);
    let env = Environment::default();
    let total = wt.frame_count();
    for i in 0..total {
        let camera = wt.camera_at(wt.frame_time(i)).to_render_camera();
        let image = renderer.render(&camera, &env, &[], settings);
        fs::write(out_dir.join(frame_file_name(i + 1)), encode_png(&image))?;
        if !progress(i + 1, total) {
            return Ok(i + 1);
        }
    }
    Ok(total)
}

/// Write `make_video.sh` into `out_dir`: an `ffmpeg` command that turns the
/// frames into `walkthrough.mp4` at `fps` (H.264, yuv420p). Run it from
/// anywhere; it changes into its own directory first. ffmpeg itself is not
/// needed to call this. Returns the script's path.
pub fn write_ffmpeg_script(out_dir: &Path, fps: f64) -> io::Result<PathBuf> {
    fs::create_dir_all(out_dir)?;
    let fps = if fps.is_finite() && fps > 0.0 {
        fps
    } else {
        24.0
    };
    let script = format!(
        "#!/bin/sh\n\
         # Assemble the walkthrough frames into an mp4 (requires ffmpeg).\n\
         set -e\n\
         cd \"$(dirname \"$0\")\"\n\
         ffmpeg -y -framerate {fps} -i frame_%04d.png -c:v libx264 -pix_fmt yuv420p walkthrough.mp4\n"
    );
    let path = out_dir.join(FFMPEG_SCRIPT_NAME);
    fs::write(&path, script)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walkthrough::{KeyFrame, Walkthrough};
    use plan_3d::{Material, Mesh, Vertex};

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        std::env::temp_dir().join(format!("plan_view3d_{tag}_{}_{nanos}", std::process::id()))
    }

    fn wall_scene() -> Scene {
        let v = |x: f32, y: f32| Vertex {
            position: [x, y, -200.0],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 0.0],
        };
        Scene {
            meshes: vec![Mesh {
                vertices: vec![
                    v(-300.0, 0.0),
                    v(300.0, 0.0),
                    v(300.0, 120.0),
                    v(-300.0, 120.0),
                ],
                indices: vec![0, 1, 2, 0, 2, 3],
                material: Material::WallInterior,
                object_id: None,
                color: None,
            }],
        }
    }

    fn two_frame_walkthrough() -> Walkthrough {
        let kf = |t: f64, x: f64| KeyFrame {
            t_s: t,
            position: [x, 66.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            fov: 60.0,
        };
        // 2 fps for 1 s -> 2 frames.
        Walkthrough::from_keyframes(vec![kf(0.0, -50.0), kf(1.0, 50.0)], 2.0)
    }

    #[test]
    fn two_frame_render_writes_two_pngs() {
        let wt = two_frame_walkthrough();
        assert_eq!(wt.frame_count(), 2);
        let dir = temp_dir("frames");
        let settings = RenderSettings {
            width: 16,
            height: 12,
            samples: 1,
            threads: 1,
            ..Default::default()
        };
        let mut calls = Vec::new();
        let n = render_frames(&wt, &wall_scene(), &settings, &dir, &mut |d, t| {
            calls.push((d, t));
            true
        })
        .unwrap();
        assert_eq!(n, 2);
        assert_eq!(calls, vec![(1, 2), (2, 2)]);
        for name in ["frame_0001.png", "frame_0002.png"] {
            let bytes = fs::read(dir.join(name)).unwrap();
            assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
            // IHDR width/height.
            assert_eq!(&bytes[16..24], &[0, 0, 0, 16, 0, 0, 0, 12]);
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn progress_can_cancel() {
        let wt = two_frame_walkthrough();
        let dir = temp_dir("cancel");
        let settings = RenderSettings {
            width: 8,
            height: 6,
            samples: 1,
            threads: 1,
            ..Default::default()
        };
        let n = render_frames(&wt, &wall_scene(), &settings, &dir, &mut |_, _| false).unwrap();
        assert_eq!(n, 1);
        assert!(dir.join("frame_0001.png").exists());
        assert!(!dir.join("frame_0002.png").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn ffmpeg_script_names_frames_and_fps() {
        let dir = temp_dir("script");
        let path = write_ffmpeg_script(&dir, 30.0).unwrap();
        assert_eq!(path.file_name().unwrap(), FFMPEG_SCRIPT_NAME);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("#!/bin/sh"));
        assert!(text.contains("-framerate 30 "));
        assert!(text.contains("frame_%04d.png"));
        assert!(text.contains("walkthrough.mp4"));
        fs::remove_dir_all(&dir).unwrap();
    }
}
