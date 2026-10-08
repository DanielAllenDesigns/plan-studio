# plan-render

Dependency-free CPU path tracer for Chief's "Physically Based" / Ray Trace stills (C-45, C-51, C-52, C-62..C-64).

- `Renderer::new(&Scene)` builds a binned-SAH BVH over every `plan-3d` triangle (material index per triangle).
- `render` / `render_progressive` run row bands on `std::thread`s; each pixel sample has its own PCG seed
  `(seed, pixel, sample)`, so output is byte-identical for any thread count or pass schedule.
- Shading: Lambert + GGX coat per material, thin-sheet glass (IOR 1.5 Fresnel, tint from colour alpha),
  next-event estimation for the sun disc (soft shadows) and point lights, sky by cosine sampling,
  Russian roulette after 2 bounces, firefly clamp.
- Techniques: `PhysicallyBased`, `Clay` (grey diffuse, clear glass), `Ambient` (occlusion only).
- Output: `Image { rgba, hdr }`, ACES / Reinhard / Linear tone map, optional bilateral denoise,
  `encode_png` / `write_png` (own PNG writer: stored zlib blocks, Adler-32, CRC-32) and `render_to_file(project, cam, settings, path)`.
- Scene frame is X right, Y up (inches), Z toward the viewer; `Camera::from_plan` maps plan `y` to `-Z`.

Expected speeds (Apple silicon, release, 4-bounce room scene of ~10 triangles): about 1.5 M paths/s per core,
roughly 20 M paths/s on 20 threads. A 480x360 image at 64 spp takes ~0.5 s on 20 threads (~7 s on one).
Real projects with tens of thousands of triangles are typically 2-4x slower; a 1920x1080 render at 256 spp is a
minutes-scale job on a laptop. The PNG is uncompressed (about 4 bytes per pixel).
