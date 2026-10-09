//! 360 degree panoramas: the equirectangular camera and the self-contained
//! HTML viewer that shows one (parity C-77).
//!
//! A panorama is an ordinary path-traced render whose rays leave the eye in
//! every direction ([`crate::Projection::Equirectangular`]): the picture is
//! twice as wide as it is high, its centre looks along the camera's heading,
//! the left and right edges meet behind the camera, the top row is straight
//! up and the bottom row straight down. [`write_panorama`] saves the PNG and
//! a single `.html` file beside it that embeds a JPEG of the picture and a
//! small WebGL script (no libraries, no server) to look around it.

use crate::camera::{equirect_dir, Camera};
use crate::image::Image;
use crate::jpeg::encode_jpeg;
use crate::png::write_png;
use crate::settings::{Projection, RenderSettings, MAX_PIXELS, MAX_SIDE};
use crate::vec3::V3;
use plan_core::Point;
use std::io;
use std::path::{Path, PathBuf};

/// The widths Export 360 Panorama offers, pixels.
pub const PANORAMA_WIDTHS: [u32; 4] = [1024, 2048, 4096, 8192];

/// The camera of a panorama taken at plan `position`, `eye_height` inches
/// above the scene origin, facing `heading_deg` (counter-clockwise from plan
/// +X, like every plan camera). The picture's centre looks along the heading.
pub fn panorama_camera(position: Point, heading_deg: f64, eye_height: f64) -> Camera {
    Camera::from_plan(position, heading_deg, eye_height, 90.0)
}

/// `base` made into a panorama `width` pixels wide (rounded to an even number
/// and kept inside the size limits) and half as high, at `samples` samples
/// per pixel.
pub fn panorama_settings(base: &RenderSettings, width: u32, samples: u32) -> RenderSettings {
    let mut w = u64::from(width.clamp(64, MAX_SIDE)) & !1;
    if w * (w / 2) > MAX_PIXELS {
        // The largest 2:1 picture inside the pixel limit.
        w = ((2.0 * MAX_PIXELS as f64).sqrt() as u64) & !1;
    }
    RenderSettings {
        width: w as u32,
        height: (w / 2) as u32,
        samples: samples.max(1),
        projection: Projection::Equirectangular,
        ..base.clone()
    }
}

/// The unit view direction of panorama position `(u, v)` (0..1 across and
/// down the picture) for `cam`, scene space.
pub fn panorama_direction(cam: &Camera, u: f32, v: f32) -> [f32; 3] {
    let (forward, right, up) = basis(cam);
    let d = equirect_dir(forward, right, up, u, v);
    [d.x, d.y, d.z]
}

/// Where in the picture (`u`, `v`, both 0..1) direction `dir` lands: the
/// inverse of [`panorama_direction`].
pub fn panorama_position(cam: &Camera, dir: [f32; 3]) -> (f32, f32) {
    let (forward, right, up) = basis(cam);
    let d = V3::from_array(dir).normalized();
    let lat = d.dot(up).clamp(-1.0, 1.0).asin();
    let lon = d.dot(right).atan2(d.dot(forward));
    (
        lon / std::f32::consts::TAU + 0.5,
        0.5 - lat / std::f32::consts::PI,
    )
}

/// Forward, right and up of a camera, the way the ray generator builds them.
fn basis(cam: &Camera) -> (V3, V3, V3) {
    let to_target = V3::from_array(cam.target) - V3::from_array(cam.eye);
    let forward = if to_target.length_sq() > 0.0 {
        to_target.normalized()
    } else {
        V3::new(0.0, 0.0, -1.0)
    };
    let mut hint = V3::from_array(cam.up);
    if hint.cross(forward).length_sq() < 1e-10 {
        hint = if forward.y.abs() > 0.9 {
            V3::new(0.0, 0.0, -1.0)
        } else {
            V3::new(0.0, 1.0, 0.0)
        };
    }
    let right = forward.cross(hint).normalized();
    let up = right.cross(forward);
    (forward, right, up)
}

/// Standard base64 with padding.
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const VIEWER: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>@@TITLE@@</title>
<style>
html,body{margin:0;height:100%;background:#15171a;overflow:hidden}
canvas{display:block;width:100%;height:100%;touch-action:none;cursor:grab}
canvas:active{cursor:grabbing}
#hint{position:fixed;left:12px;bottom:10px;color:#e6e6e6;font:13px system-ui,sans-serif;opacity:.8;pointer-events:none}
</style></head><body>
<canvas id="v"></canvas>
<div id="hint">@@TITLE@@ &middot; drag to look around, scroll or pinch to zoom</div>
<script>
var SRC="@@IMAGE@@";
var yaw=@@YAW@@*Math.PI/180,pitch=0,fov=75*Math.PI/180,dirty=true;
var cv=document.getElementById('v');
var img=new Image();
img.onload=function(){start();};
img.src=SRC;
function clamp(x,a,b){return Math.max(a,Math.min(b,x));}
function controls(){
  var pts={},last=0;
  cv.addEventListener('pointerdown',function(e){cv.setPointerCapture(e.pointerId);pts[e.pointerId]=[e.clientX,e.clientY];last=0;});
  function up(e){delete pts[e.pointerId];last=0;}
  cv.addEventListener('pointerup',up);cv.addEventListener('pointercancel',up);
  cv.addEventListener('pointermove',function(e){
    var p=pts[e.pointerId];if(!p)return;
    var ids=Object.keys(pts);
    if(ids.length===2){
      pts[e.pointerId]=[e.clientX,e.clientY];
      var a=pts[ids[0]],b=pts[ids[1]],d=Math.hypot(a[0]-b[0],a[1]-b[1]);
      if(last>0){fov=clamp(fov*last/d,0.4,1.9);dirty=true;}
      last=d;return;
    }
    var k=fov/cv.clientHeight;
    yaw-=(e.clientX-p[0])*k;pitch=clamp(pitch+(e.clientY-p[1])*k,-1.55,1.55);
    pts[e.pointerId]=[e.clientX,e.clientY];dirty=true;
  });
  cv.addEventListener('wheel',function(e){e.preventDefault();fov=clamp(fov*Math.exp(e.deltaY*0.001),0.4,1.9);dirty=true;},{passive:false});
  window.addEventListener('resize',function(){dirty=true;});
}
function size(){
  var r=window.devicePixelRatio||1,w=Math.round(cv.clientWidth*r),h=Math.round(cv.clientHeight*r);
  if(cv.width!==w||cv.height!==h){cv.width=w;cv.height=h;}
}
function start(){
  controls();
  var gl=cv.getContext('webgl')||cv.getContext('experimental-webgl');
  if(!gl||img.width>gl.getParameter(gl.MAX_TEXTURE_SIZE)){flat();return;}
  function sh(t,s){var o=gl.createShader(t);gl.shaderSource(o,s);gl.compileShader(o);return o;}
  var pr=gl.createProgram();
  gl.attachShader(pr,sh(gl.VERTEX_SHADER,'attribute vec2 p;varying vec2 q;void main(){q=p;gl_Position=vec4(p,0.,1.);}'));
  gl.attachShader(pr,sh(gl.FRAGMENT_SHADER,
   'precision highp float;varying vec2 q;uniform sampler2D t;uniform vec4 s;'+
   'void main(){'+
   'vec3 d=normalize(vec3(q.x*s.x*s.z,q.y*s.z,-1.0));'+
   'float cp=cos(s.y),sp=sin(s.y);d=vec3(d.x,d.y*cp-d.z*sp,d.y*sp+d.z*cp);'+
   'float cy=cos(s.w),sy=sin(s.w);d=vec3(d.x*cy-d.z*sy,d.y,d.x*sy+d.z*cy);'+
   'float lon=atan(d.x,-d.z),lat=asin(clamp(d.y,-1.0,1.0));'+
   'gl_FragColor=texture2D(t,vec2(lon/6.2831853+0.5,0.5-lat/3.1415927));}'));
  gl.linkProgram(pr);
  gl.useProgram(pr);
  var buf=gl.createBuffer();gl.bindBuffer(gl.ARRAY_BUFFER,buf);
  gl.bufferData(gl.ARRAY_BUFFER,new Float32Array([-1,-1,3,-1,-1,3]),gl.STATIC_DRAW);
  var loc=gl.getAttribLocation(pr,'p');gl.enableVertexAttribArray(loc);gl.vertexAttribPointer(loc,2,gl.FLOAT,false,0,0);
  var tex=gl.createTexture();gl.bindTexture(gl.TEXTURE_2D,tex);
  gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA,gl.RGBA,gl.UNSIGNED_BYTE,img);
  gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_S,gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_T,gl.CLAMP_TO_EDGE);
  var us=gl.getUniformLocation(pr,'s');
  (function frame(){
    if(dirty){dirty=false;size();gl.viewport(0,0,cv.width,cv.height);
      gl.uniform4f(us,cv.width/cv.height,pitch,Math.tan(fov/2),yaw);
      gl.drawArrays(gl.TRIANGLES,0,3);}
    requestAnimationFrame(frame);
  })();
}
function flat(){
  var c=cv.getContext('2d');
  (function frame(){
    if(dirty){dirty=false;size();
      var h=cv.height,w=Math.round(img.width*h/img.height),x=((-yaw/(2*Math.PI)+0.5)*w)%w-w/2;
      for(var o=-w;o<=cv.width+w;o+=w)c.drawImage(img,x+o-w/2+cv.width/2,0,w,h);}
    requestAnimationFrame(frame);
  })();
}
</script></body></html>
"#;

/// The self-contained viewer page for a panorama: `jpeg` is embedded as a
/// data URI so the file works alone (also from `file://`, where a separate
/// picture could not be used as a WebGL texture). `title` is shown in the tab
/// and as a caption; `yaw_deg` turns the first view to the right.
pub fn viewer_html(title: &str, jpeg: &[u8], yaw_deg: f32) -> String {
    VIEWER
        .replace("@@TITLE@@", &escape_html(title))
        .replace(
            "@@IMAGE@@",
            &format!("data:image/jpeg;base64,{}", base64(jpeg)),
        )
        .replace(
            "@@YAW@@",
            &format!("{:.3}", if yaw_deg.is_finite() { yaw_deg } else { 0.0 }),
        )
}

/// Where [`write_panorama`] put its files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanoramaFiles {
    pub png: PathBuf,
    pub html: PathBuf,
}

/// Saves `image` as the PNG `png_path` and writes the viewer page next to it
/// (same name, `.html`). `title` is the caption of the page.
pub fn write_panorama(image: &Image, png_path: &Path, title: &str) -> io::Result<PanoramaFiles> {
    let png = png_path.with_extension("png");
    let html = png_path.with_extension("html");
    write_png(&png, image)?;
    let jpeg = encode_jpeg(image.width, image.height, &image.rgba, 90);
    std::fs::write(&html, viewer_html(title, &jpeg, 0.0))?;
    Ok(PanoramaFiles { png, html })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam() -> Camera {
        // Facing plan +X (scene +X), eye 60 in up.
        panorama_camera(Point::new(100.0, 40.0), 0.0, 60.0)
    }

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4)
    }

    #[test]
    fn the_centre_looks_along_the_heading_and_the_poles_up_and_down() {
        let c = cam();
        assert!(close(panorama_direction(&c, 0.5, 0.5), [1.0, 0.0, 0.0]));
        assert!(close(panorama_direction(&c, 0.5, 0.0), [0.0, 1.0, 0.0]));
        assert!(close(panorama_direction(&c, 0.5, 1.0), [0.0, -1.0, 0.0]));
        // The two edges meet straight behind the camera.
        assert!(close(panorama_direction(&c, 0.0, 0.5), [-1.0, 0.0, 0.0]));
        assert!(close(panorama_direction(&c, 1.0, 0.5), [-1.0, 0.0, 0.0]));
    }

    #[test]
    fn moving_right_in_the_picture_turns_the_view_to_the_right() {
        // Facing +X with +Y up, the right-hand side is scene +Z (plan south).
        let c = cam();
        assert!(close(panorama_direction(&c, 0.75, 0.5), [0.0, 0.0, 1.0]));
        assert!(close(panorama_direction(&c, 0.25, 0.5), [0.0, 0.0, -1.0]));
        // A heading of 90 degrees (plan north, scene -Z) turns the centre.
        let north = panorama_camera(Point::ZERO, 90.0, 0.0);
        assert!(close(
            panorama_direction(&north, 0.5, 0.5),
            [0.0, 0.0, -1.0]
        ));
        assert!(close(
            panorama_direction(&north, 0.75, 0.5),
            [1.0, 0.0, 0.0]
        ));
    }

    #[test]
    fn position_is_the_inverse_of_direction() {
        let c = cam();
        for (u, v) in [
            (0.5, 0.5),
            (0.1, 0.3),
            (0.9, 0.7),
            (0.33, 0.8),
            (0.62, 0.12),
        ] {
            let (pu, pv) = panorama_position(&c, panorama_direction(&c, u, v));
            assert!(
                (pu - u).abs() < 1e-4 && (pv - v).abs() < 1e-4,
                "{u},{v} -> {pu},{pv}"
            );
        }
    }

    #[test]
    fn the_settings_are_two_to_one_even_and_inside_the_limits() {
        let base = RenderSettings::default();
        let s = panorama_settings(&base, 2048, 16);
        assert_eq!((s.width, s.height, s.samples), (2048, 1024, 16));
        assert_eq!(s.projection, Projection::Equirectangular);
        let odd = panorama_settings(&base, 1001, 0);
        assert_eq!((odd.width, odd.height, odd.samples), (1000, 500, 1));
        let huge = panorama_settings(&base, 100_000, 4);
        assert!(
            huge.width <= MAX_SIDE && huge.width > 6000,
            "the largest that fits"
        );
        assert!(u64::from(huge.width) * u64::from(huge.height) <= MAX_PIXELS);
        assert_eq!(huge.width, huge.height * 2);
        assert!(panorama_settings(&base, 1, 4).width >= 64);
    }

    #[test]
    fn base64_matches_the_standard_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn the_viewer_is_one_self_contained_file() {
        let jpeg = encode_jpeg(8, 4, &[200; 8 * 4 * 4], 80);
        let html = viewer_html("Great <Room> & more", &jpeg, 30.0);
        assert!(html.starts_with("<!doctype html>"));
        assert!(
            html.contains("data:image/jpeg;base64,/9j/"),
            "a JPEG data URI"
        );
        assert!(html.contains("Great &lt;Room&gt; &amp; more"));
        assert!(html.contains("var yaw=30.000*"));
        assert!(html.contains("getContext('webgl')") && html.contains("texture2D"));
        // No external references: nothing is fetched from anywhere.
        assert!(!html.contains("src=\"http") && !html.contains("href=\"http"));
        assert!(!html.contains("@@"), "every placeholder is filled");
    }

    #[test]
    fn a_panorama_renders_and_saves_png_and_html_side_by_side() {
        use crate::Renderer;
        use plan_3d::{Material, Mesh, Scene, Vertex};
        // A sealed box 200 in wide around the eye: walls, floor and ceiling.
        let mut mesh = Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            material: Material::WallInterior,
            object_id: None,
            color: None,
        };
        let quad = |m: &mut Mesh, c: [[f32; 3]; 4], n: [f32; 3]| {
            let base = m.vertices.len() as u32;
            for p in c {
                m.vertices.push(Vertex {
                    position: p,
                    normal: n,
                    uv: [0.0, 0.0],
                });
            }
            m.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        };
        let h = 100.0_f32;
        quad(
            &mut mesh,
            [[-h, -h, -h], [h, -h, -h], [h, -h, h], [-h, -h, h]],
            [0.0, 1.0, 0.0],
        );
        quad(
            &mut mesh,
            [[-h, h, h], [h, h, h], [h, h, -h], [-h, h, -h]],
            [0.0, -1.0, 0.0],
        );
        quad(
            &mut mesh,
            [[-h, -h, -h], [-h, h, -h], [h, h, -h], [h, -h, -h]],
            [0.0, 0.0, 1.0],
        );
        quad(
            &mut mesh,
            [[h, -h, h], [h, h, h], [-h, h, h], [-h, -h, h]],
            [0.0, 0.0, -1.0],
        );
        quad(
            &mut mesh,
            [[-h, -h, h], [-h, h, h], [-h, h, -h], [-h, -h, -h]],
            [1.0, 0.0, 0.0],
        );
        quad(
            &mut mesh,
            [[h, -h, -h], [h, h, -h], [h, h, h], [h, -h, h]],
            [-1.0, 0.0, 0.0],
        );
        let scene = Scene { meshes: vec![mesh] };
        let renderer = Renderer::new(&scene);
        let settings = panorama_settings(
            &RenderSettings {
                threads: 2,
                ..RenderSettings::default()
            },
            128,
            2,
        );
        let image = renderer.render(
            &panorama_camera(Point::ZERO, 0.0, 0.0),
            &crate::Environment::default(),
            &[],
            &settings,
        );
        assert_eq!((image.width, image.height), (128, 64));
        // Closed on every side: no pixel is sky.
        assert!(image
            .rgba
            .chunks(4)
            .all(|p| p[0] < 250 || p[1] < 250 || p[2] < 250));
        let dir = std::env::temp_dir().join(format!("plan_pano_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let files = write_panorama(&image, &dir.join("room.png"), "Room").unwrap();
        assert_eq!(files.png.extension().unwrap(), "png");
        assert_eq!(files.html.extension().unwrap(), "html");
        assert_eq!(files.png.file_stem(), files.html.file_stem());
        let png = std::fs::read(&files.png).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        let html = std::fs::read_to_string(&files.html).unwrap();
        assert!(html.contains("data:image/jpeg;base64,"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
