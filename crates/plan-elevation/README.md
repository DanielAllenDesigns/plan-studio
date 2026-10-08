# plan-elevation

Turns a `plan_3d::Scene` into Chief-style 2D vector drawings: elevations (Front/Back/Left/Right), cross sections and the plan overhead (Top) view, as weighted line segments with hidden lines removed.

- API: `elevation`, `section(SectionCut)`, `plan_overhead`, `elevation_from_project`; each returns a `Drawing` (`Line2` list, bounds) with `to_cad(layer_prefix)` (layers `"{prefix}, Heavy|Medium|Light|Hidden"`), `merge_collinear()` and `svg()`.
- Drawing space is inches of the building, X right, Y up, unshifted (Front: x = scene x; Back mirrors it; Left/Right use -plan y; Top is the plan frame).
- Algorithm: (1) project triangles orthographically; for a section clip at the plane, collect plane/mesh intersections as Heavy cut lines and fill the cut faces so they occlude. (2) Weld vertices, split T-junctions, pick candidate edges: silhouettes and free borders (Heavy), creases over `crease_angle_deg` (Medium), material/object seams (Light).
- (3) Rasterize a depth buffer (`raster_px` along the longer axis; back faces skipped when `cull_backfaces`). (4) Sample each edge at ~1 px, keep visible runs (bias 1.5 px + 0.02"), bridge short hidden gaps, drop runs under 2 px, optionally emit hidden runs dashed. (5) `merge_collinear` joins touching segments and trims lines hidden under heavier ones.
- Limits: accuracy is about one pixel (extent / `raster_px`) so finer detail is dropped; glass occludes like a solid; section filling assumes closed meshes (what plan-3d emits); welding is 0.01" and T-junction repair 0.02"; no hatching, curves or text.
- Tests use `raster_px = 256` for speed: `cargo test -p plan-elevation`.
