# 3D Gaussian Splatting for Plan Studio: investigation

Date: 2026-10-09. Status: research, nothing built. Decision owner: Daniel.

## 1. What Gaussian splatting is, and what it is not

3D Gaussian Splatting (3DGS) represents a scene as millions of tiny translucent
ellipsoids ("splats"), each with a position, rotation, scale, opacity and a
view-dependent colour (spherical harmonics). Rendering sorts them back to front
and alpha-blends them as screen-space quads. It is fast and looks photographic.

The photographic look does not come from the primitive. It comes from the
pipeline that produces splats: dozens of real photos or a video are fitted by
optimisation so the splats reproduce the photos, lighting included. The
realism is captured, not computed. Three consequences matter for a CAD tool:

- Lighting is baked. A splat scene is the light of the moment it was shot. It
  cannot be relit by our sun, and sun studies through it are meaningless.
  Relightable splats (DeferredGS, RTR-GS, Rng) exist as research, not tools.
- Geometry is soft. Splats are not surfaces: nothing to snap to, dimension,
  section or edit. Published geometric error against LiDAR is several
  centimetres. They are context, never model.
- It needs photos. Plan Studio generates geometry with no photos behind it, so
  "render the design as splats" produces our existing textures in a fuzzier
  form. The impressive "AI renderings" online are either captures of real
  places or generative models (Marble, TripoSplat) that invent detail.

So "3DGS as a rendering style" splits into four different features. They are
ranked below by value to a residential designer and by fit with our stack.

## 2. The four features, ranked

### A. Site context: import a captured splat of the real lot and house (build this)

The client walks the real property with a phone (Scaniverse, Polycam, KIRI
Engine, Luma, Postshot on a desktop) or a drone, exports a .ply / .spz, and we
show the new addition, pool house or second-storey sitting in the photographed
existing house and yard, in the interactive 3D view, Final View, walkthroughs
and panoramas. For remodels and additions this is the strongest presentation
tool there is, and Chief Architect does not have it.

Who has it: SketchUp (official Gaussian Splatting extension), D5 Render 3.0
(PLY import, marked experimental), Blender 5.3 (import + render, release
2026-11-10, no export), Notch 2026.2, Nuke 17, CesiumJS and Cesium for Unreal.
Twinmotion and Chief Architect: nothing found.

Fit with our stack: good. Splats are a new primitive drawn in one extra pass
of the existing glow pipeline, depth-tested against the mesh depth buffer. See
section 3.

### B. Export the design as a splat for web viewers (build after A)

Mesh2Splat (Electronic Arts, open source) converts a textured glTF mesh to
splats in milliseconds by sampling the surface through the rasterizer: one
Gaussian per fragment, colour from the diffuse texture, optional
metal-roughness and normal maps. The same idea runs on the CPU in Rust: sample
each triangle by area, one Gaussian per sample, scale from local sample
density, colour from our texture lookup. Output .ply / .spz plays in SuperSplat,
PlayCanvas, Scaniverse, Zillow-style web tours, and a phone browser with no
plugin. That is a client-sharing channel, not a rendering style. Modest value,
small build (the sampler is a few hundred lines in plan-3d or a new crate).

### C. "Photoreal study" via a generative service (park)

What people see online as AI splat renderings of houses: render the design
from 2 to 8 views, send them to a hosted generator (World Labs Marble 1.1
accepts 2 to 8 photos or a video and returns .spz; TripoSplat is open source,
single image to splat), load the returned splat as a view. Output is a baked,
non-editable asset that may hallucinate details the plan does not have; good
for a mood board, dangerous for a client who thinks it is the design. Needs a
GPU service or local CUDA. Fits the Plan Agent as a tool call later. Park until
A has shipped and Daniel has judged the quality on his own projects.

### D. A "Splat" art look for our own geometry (cheap, low priority)

Pointillist, soft-edged rendering of the model: sample our meshes to splats
(the B sampler) and draw them with the A renderer instead of the mesh. It is a
legitimate stylised technique next to Watercolor and Technical Illustration,
and costs almost nothing once A and B exist. It is not photorealism.

## 3. Technical fit with plan-view3d and plan-render

Current pipeline (plan-view3d/gpu.rs, 2.2k lines): eframe/glow, OpenGL 3.3 core
on Linux/Windows, 4.1 on macOS, GLES 3 fallback. Shadow map, scene to
offscreen target, half-res AO, composite. No compute shaders, so GPU radix
sorts (the wgpu / WebGPU viewers) are out unless we move eframe to its wgpu
backend, which would mean rewriting gpu.rs and pipeline.rs. Not worth it for
this feature.

Proposed splat pass on glow (the WebGL2 viewer recipe, proven at 1 to 3 M
splats at 60 fps):

1. Loader (GUI-free crate `plan-splat`): PLY binary (the de facto format,
   62 floats per splat in the reference layout) and SPZ (Niantic, gzip of
   quantised attributes, spec public). Decode to a compact struct: position
   f32x3, scale f32x3 (log), rotation quaternion, opacity, colour DC + optional
   SH bands 1 to 3. Keep SH degree selectable (0 is fine for context).
2. Bounds, crop box, transform (translate, rotate about Z, uniform scale) and
   a metric check: phone LiDAR captures are already in metres; photogrammetry
   ones are not, so an Align tool is needed anyway.
3. Sort on a worker thread: depth key = dot(view dir, position), 16-bit
   counting sort, O(n), 1 M splats in a few ms. Re-sort when the camera moves
   more than a threshold; the GPU draws the last order meanwhile (the visible
   artefact is a brief popping on fast orbits, accepted by every web viewer).
4. Draw: instanced quad per splat (`glDrawArraysInstanced`, 4 verts), sorted
   index buffer uploaded per re-sort, attributes in a float texture or a
   buffer. Vertex shader projects the 3D covariance (J W Σ Wᵀ Jᵀ), takes the
   2D eigenvectors, sizes the quad at 3σ; fragment shader evaluates
   exp(-½ dᵀ Σ⁻¹ d), discards past the cutoff, premultiplied alpha, blend
   ONE / ONE_MINUS_SRC_ALPHA, depth test ON against the mesh depth, depth
   write OFF. Draw after opaque meshes, before glass, into the same offscreen
   target so FXAA, tone curve and the composite apply unchanged.
5. Mesh shadows on the splat: sample the existing sun shadow map in the splat
   fragment shader and darken. Cheap and visually important (the addition
   should shade the lawn). Splat shadows onto the mesh: skip; splats have no
   reliable surface. A later option is writing splat depth to a second buffer
   and using it in the AO pass.
6. Looks: draw splats in Standard and Physical. In Clay, Glass House,
   Watercolor, Technical, Duotone, Flat either hide them or draw them as flat
   grey (Clay) so stylised elevations stay clean.
7. plan-render (path tracer): ray tracing splats is research (3DGUT). For
   Final View and recordings, rasterize splats to RGBA + depth with the same
   camera and composite with the path-traced mesh image by depth. The existing
   `style::stylize` post-process then applies to the merged frame. Panoramas
   work the same way per cube face.
8. Walkthrough recording and Export Picture work for free once the viewport
   pass exists.

Data model (plan-core): a typed slot `Splat { path, transform, crop_box,
visible, opacity, sh_degree, name }`. Store the file by reference next to the
.plan (1 M splats is 60 to 250 MB as PLY, roughly a tenth as SPZ; never embed).
Layer "Site Splat" in the layer sets, default displayed in camera views, hidden
in plan view (optionally a footprint rectangle in plan).

UI: File ▸ Import ▸ Gaussian Splat (.ply, .spz, and .glb carrying the Khronos
KHR_gaussian_splatting extension once that reader exists), an Align Splat tool
(two points on the splat to two points in the plan, like Chief's picture
scaling), a Splat Specification dialog (transform, crop, opacity, SH quality),
and a 3D View Defaults row for "Draw site splats". Hover and pick: treat the
splat as one object (click the bounding box), not per-splat.

Memory and performance budget: 1 M splats ≈ 64 MB on the GPU as floats,
≈ 24 MB with f16 and quantised rotation; cap at 3 M with a warning; LOD by
skipping every nth splat beyond a budget until a proper hierarchy exists.

## 4. Formats and standards

- PLY: universal today. Every capture app exports it.
- SPZ (Niantic Spatial): compressed, about 10x smaller, exported by Scaniverse,
  Marble, SuperSplat. Public spec, small decoder.
- SOG (PlayCanvas): web-oriented, less common in capture apps. Skip for now.
- glTF KHR_gaussian_splatting: release candidate February 2026, ratification
  targeted Q2 2026, no confirmation of ratification found as of 2026-10-09;
  compression sub-extensions (SPZ, L-GSC) still moving. Cesium already ships
  it. Add the reader when our glTF importer is next touched, not before.

## 5. Effort estimate

Phase A (viewport + import + align + Final View composite): one builder round,
two builders (plan-splat crate with loaders, sort and tests; plan-view3d pass
and plan-app UI), roughly 2 to 3k lines with tests and a scenario (sXX
splat import, align, visibility per look, picture export).
Phase B (export the design as .ply/.spz): half a builder.
Phase D (Splat look): a quarter of a builder once A and B exist.
Phase C (generative): a Plan Agent tool plus a service account; estimate when
A is judged.

Test data: Daniel captures his own house with Scaniverse (free, iPhone, metric
scale from LiDAR, exports PLY and SPZ). Do not pull sample splats from the web
into the repo; licences are unclear.

## 6. Decisions for Daniel

| # | Question | Assumption if unanswered |
|---|----------|--------------------------|
| GS1 | Build A first, as scoped above? | Yes, next free builder round |
| GS2 | Stay on glow with a CPU sort, or move eframe to wgpu for GPU sorting? | Stay on glow |
| GS3 | Splat file by reference next to the .plan, or embedded? | By reference |
| GS4 | Park C (generative photoreal) until A ships? | Park |
| GS5 | Draw splats in stylised looks as flat grey, or hide them? | Flat grey in Clay, hidden elsewhere |

## 7. Sources consulted

- Mesh2Splat (EA SEED): https://github.com/electronicarts/mesh2splat
- SketchUp Gaussian Splats extension: https://help.sketchup.com/cs/gaussian-splats-extension
- D5 Render 3.0 splat import: https://radiancefields.com/d5-render-3.0-expands-gaussian-splatting-capabilities
- Blender 5.3 native splats: https://radiancefields.com/blender-5.3-will-bring-native-3d-gaussian-splat-import-and-rendering
- Khronos KHR_gaussian_splatting press release: https://www.khronos.org/news/press/gltf-gaussian-splatting-press-release
- Extension lineage incl. SPZ sub-extension: https://docs.3dtiled.iconem.com/docs/references/3dtiles-gltf/3dgs-extension-lineage
- wgpu_3dgs_viewer (Rust, wgpu): https://docs.rs/wgpu-3dgs-viewer
- Brush (Rust training engine): https://github.com/ArthurBrussee/brush
- WebGL2 viewer technique (Portality): https://radiancefields.com/portality-webgl2-gaussian-splat-viewer
- WebSplatter (quad sizing, sorting): https://arxiv.org/pdf/2602.03207
- Relighting research: DeferredGS https://arxiv.org/abs/2404.09412 , RTR-GS https://arxiv.org/pdf/2507.07733 , Rng https://arxiv.org/html/2409.19702v3
- CAD-anchored splats: CADSplat https://arxiv.org/pdf/2609.18473 ; mesh-prior architecture https://arxiv.org/abs/2407.15435v2
- World Labs Marble: https://radiancefields.com/world-labs-formally-launches-marble-a-generative-world-model
- TripoSplat (ComfyUI docs): https://docs.comfy.org/tutorials/3d/triposplat.md
- State of the field 2026: https://www.thefuture3d.com/blog/state-of-gaussian-splatting-2026/
- Zillow SkyTour splats: https://lidarnews.com/zillow-3d-tours-with-gaussian-splatting/
