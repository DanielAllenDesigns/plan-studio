# plan-render

Dependency-free CPU path tracer for Chief's "Physically Based" / Ray Trace stills (C-45, C-51, C-52, C-62..C-64).

- `Renderer::new(&Scene)` builds a binned-SAH BVH over every `plan-3d` triangle (material index per triangle).
- `render` / `render_progressive` run row bands on `std::thread`s; each pixel sample has its own PCG seed
  `(seed, pixel, sample)`, so output is byte-identical for any thread count or pass schedule.
- Shading: Lambert + GGX coat per material, thin-sheet glass (IOR 1.5 Fresnel, tint from colour alpha),
  next-event estimation for the sun disc (soft shadows) and point lights, sky by cosine sampling,
  Russian roulette after 2 bounces, firefly clamp.
- Lights: sun disc, spherical point lights and rectangular `AreaLight` panels, all sampled directly (next-event
  estimation; `RenderSettings::next_event = false` leaves area lights to chance for comparison). Metals have no
  diffuse lobe and reflect in their own colour (`plan_materials::scene_surface`).
- Sky: the two-colour gradient or `SkyModel::Preetham { turbidity }`, an analytic clear sky that brightens toward
  the sun and horizon and changes colour with sun height.
- Camera: thin lens with `aperture` and `focus_dist` (0 focuses on the image-centre surface).
- Denoise (`RenderSettings::denoise`): a joint bilateral filter guided by the first hit's albedo, normal and depth,
  applied to radiance divided by albedo so texture detail and geometry edges survive.
- Progress: `render_progressive` can start with a blocky one-sample-per-4x4 preview (`preview_blocks`);
  `RenderSettings::scaled(2)` / `scaled(4)` give the larger size for Save Image (capped at 8192 px a side, 24 MP).
  `RenderSettings` and `SkyModel` serialize with serde.
- Techniques: `PhysicallyBased`, `Clay` (grey diffuse, clear glass), `Ambient` (occlusion only).
- Output: `Image { rgba, hdr }`, ACES / Reinhard / Linear tone map, optional bilateral denoise,
  `encode_png` / `write_png` (own PNG writer: stored zlib blocks, Adler-32, CRC-32) and `render_to_file(project, cam, settings, path)`.
- Scene frame is X right, Y up (inches), Z toward the viewer; `Camera::from_plan` maps plan `y` to `-Z`.
- Panoramas (round 15): `RenderSettings::projection = Projection::Equirectangular` traces a full 360 x 180 degree
  picture (`panorama_settings` makes the 2:1 size, `panorama_camera` places the eye); `write_panorama` saves the PNG and a
  self-contained `.html` viewer (a JPEG data URI plus a small WebGL script). `panorama_direction` / `panorama_position` map
  picture positions to view directions and back.
- Video (round 15): `encode_jpeg` is a baseline JPEG encoder (YCbCr 4:4:4, standard Huffman tables) and `AviWriter` streams
  frames into a Motion-JPEG AVI (`read_avi` parses it back); both are dependency free.
- Looks (round 15): `stylize(rgba, w, h, Style)` turns a picture into Vector View, Technical Illustration, Line Drawing or
  Watercolor with a Sobel edge detector and flat shading tiers.

Expected speeds (Apple silicon, release, 4-bounce room scene of ~10 triangles): about 1.5 M paths/s per core,
roughly 20 M paths/s on 20 threads. A 480x360 image at 64 spp takes ~0.5 s on 20 threads (~7 s on one).
Real projects with tens of thousands of triangles are typically 2-4x slower; a 1920x1080 render at 256 spp is a
minutes-scale job on a laptop. The PNG is uncompressed (about 4 bytes per pixel).
