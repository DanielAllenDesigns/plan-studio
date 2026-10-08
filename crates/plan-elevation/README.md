# plan-elevation

Turns a `plan_3d::Scene` into Chief-style 2D vector drawings: elevations (Front/Back/Left/Right), cross sections and the plan overhead (Top) view, as weighted line segments with hidden lines removed.

- API: `elevation`, `section(SectionCut)`, `plan_overhead`, `elevation_from_project`; each returns a `Drawing` (`Line2` list, bounds) with `to_cad(layer_prefix)` (layers `"{prefix}, Heavy|Medium|Light|Hidden"`), `merge_collinear()` and `svg()`.
- Drawing space is inches of the building, X right, Y up, unshifted (Front: x = scene x; Back mirrors it; Left/Right use -plan y; Top is the plan frame).
- Algorithm: (1) project triangles orthographically; for a section clip at the plane, collect plane/mesh intersections as Heavy cut lines and fill the cut faces so they occlude. (2) Weld vertices, split T-junctions, pick candidate edges: silhouettes and free borders (Heavy), creases over `crease_angle_deg` (Medium), material/object seams (Light).
- (3) Rasterize a depth buffer (`raster_px` along the longer axis; back faces skipped when `cull_backfaces`). (4) Sample each edge at ~1 px, keep visible runs (bias 1.5 px + 0.02"), bridge short hidden gaps, drop runs under 2 px, optionally emit hidden runs dashed. (5) `merge_collinear` joins touching segments and trims lines hidden under heavier ones.
- Limits: accuracy is about one pixel (extent / `raster_px`) so finer detail is dropped; glass occludes like a solid; section filling assumes closed meshes (what plan-3d emits); welding is 0.01" and T-junction repair 0.02"; no curves.
- Tests use `raster_px = 256` for speed: `cargo test -p plan-elevation`.

## Chief-style detail

- **Regions** (`Options.regions`, default on): the rasterizer also writes a per-pixel id (material + source object + Face/Cut). Boundaries are traced on the pixel lattice, smoothed like marching squares (edge midpoints; exact corners between straight runs), simplified with Douglas-Peucker at 0.5 px (rings of 8 vertices or fewer are left alone), and holes are joined by a zero-width slit so each `Drawing.regions` entry is a single ring. Kinds: `Face`, `Cut` (section poche; cut loops are filled per source object, named after the material with the most cut length) and `Shadow`. `Drawing::cut_regions()` / `regions_of(kind)`.
- **Hatch** (`Options.hatch`, default off): Light `EdgeKind::Hatch` lines clipped to each Face polygon. Brick, Siding (6" lap), Stucco (every third concrete dash), Stone (irregular coursed stone), Roof (shingle), Concrete, Glass/WindowGlass (45 degrees at 6"); other materials are unhatched. Patterns are coarsened for a 1/4" = 1'-0" sheet; courses start at the region's lower-left corner.
- **Sections**: `Options.section_depth` clips geometry to that many inches behind the cut plane (no cut lines are made at the back plane).
- **Shadows** (`Options.shadows: Some(SunDir { azimuth_deg, altitude_deg })`, bearing clockwise from plan north): shadow map at the raster size, constant bias, faces turned from the sun count as shadowed. The ground plane is only visible (and shadowed) in the Top view. Approximation.
- **Depth weights** (`Options.depth_weights`): lines more than 12" behind the nearest drawn line step down one weight class. Approximation of Chief's line weight by distance.
- **Labels**: `elevation_with_labels` / `annotate` add `Drawing.texts` (left-baseline anchors): title, "T.O. SUBFLOOR" / "T.O. PLATE" callouts, "GRADE", a grade line and roof pitch triangles ("8:12") as `EdgeKind::Annotation` lines.
- `svg()` paints face regions white, poche gray (`#8c8c8c`), shadows translucent, then lines and text.
