# plan-roof

Chief-style automatic roofs ("Build Roof") for Plan Studio, as plain 3D roof
planes. No GPU or rendering code; a viewer, exporter or take-off consumes the
polygons.

```rust
use plan_roof::{build_roof, footprint_from_walls, EdgeKind, EdgeRoof};

let footprint = footprint_from_walls(&walls, 0.5).unwrap();      // CCW, inches
let mut edges = vec![EdgeRoof::default(); footprint.len()];      // Hip, 8:12, 16"
edges[1].kind = EdgeKind::Gable;                                 // gable end
let roof = build_roof(&footprint, &edges, 108.0);                // top of wall
for plane in &roof.planes {
    println!("edge {} area {:.0} sq in", plane.source_edge, plane.area());
}
```

All lengths are inches. Output coordinates are `X = plan x`, `Y = elevation`,
`Z = -plan y`, polygons counter-clockwise seen from above (normals point up).

## API

* `EdgeRoof { pitch_in_12, kind, overhang }` with `EdgeKind::{Hip, Gable, Shed}`;
  `Default` is Hip, 8:12, 16" (the Chief wall-dialog Roof tab defaults).
* `build_roof(footprint, edges, baseline_elevation) -> Roof`.
* `Roof { planes, fascia_height, baseline_elevation, approximate }` and
  `Roof::bounds()`.
* `RoofPlane { polygon3d, pitch_in_12, baseline, source_edge }` with `area()`
  (true sloped area), `projected_area()`, `normal()` and `ridge_height()`. The
  first polygon edge (`[0] -> [1]`) is always the eave on the baseline.
* `footprint_from_walls(walls, tol) -> Option<Vec<Point>>`: the outer boundary
  of the wall centerlines.

## Roof features (holes, ceilings, dormers, gable lines, returns, edge specs)

All of these are input/output types in this crate; none needs a model field.
`plan-3d` meshes them (`roof_plane_meshes`, `skylight_meshes`,
`ceiling_plane_meshes`, `dormer_meshes`, `roof_meshes`).

| Chief feature | API |
|---|---|
| Roof Hole / Skylight (RF-42/43) | `RoofHole { outline, kind: Skylight \| Hole, skylight: Option<SkylightSpec { curb_height, glass_thickness, frame_width }> }`; `roof_plane_with_holes(&plane, &holes) -> RoofPolygonWithHoles { outer, holes, skylights, normal, .. }` |
| Ceiling Plane / Build Ceiling Planes (RF-45/46) | `CeilingPlane { outline, baseline, pitch_in_12, height_at_baseline, thickness }` (`height_at`, `polygon3d`, `area`); `ceiling_planes_for_vaulted_room(room_poly, &roof_planes, thickness) -> Vec<CeilingPlane>` |
| Auto Dormer / Explode Dormer (RF-48..51) | `auto_dormer(&main_plane, DormerSpec) -> Option<Dormer>`; `explode_dormer(&Dormer) -> ExplodedDormer` |
| Gable/Roof Line (RF-44) | `apply_gable_line(&roof, edge_index) -> Option<Roof>` |
| Auto Roof Return (RF-27) | `roof_return(&plane, edge, ReturnSpec { kind: Full \| Half \| Boxed, length }) -> Option<RoofReturn>`, `roof_return_at(.., at_start, ..)` |
| Wall Roof tab (RF-18..26) | `EdgeRoofSpec { pitch, overhang, gable, full_gable_wall, high_shed_gable, extend_slope_downward }`; `build_roof_with_specs(footprint, &specs, baseline)` |

Also `RoofPlane::height_at(plan_point)` and `RoofPlane::plan_polygon()`.

Mapping from the model (the model has no such fields yet, so wire these in the
app):

* Wall > Roof tab: Hip Wall = `EdgeRoofSpec::default()`; Full Gable Wall =
  `full_gable_wall` (same as `gable`: the gable wall always reaches the ridge);
  High Shed/Gable Wall = `high_shed_gable` (edge kind Shed, overhang forced to 0);
  Extend Slope Downward = `extend_slope_downward: Some(inches of drop)`; Pitch
  and Overhang = `pitch`, `overhang`. Priority: high shed, then gable, then hip.
  Dutch gable, knee wall and upper pitch are not modelled.
* Dormer defaults: width, wall height, roof type/pitch and window map to
  `DormerSpec`. `position_along_eave` is the distance from `baseline.0` to the
  dormer centre, `setback_from_eave` the plan distance from the eave line to the
  front wall. For gable and hip, `height_to_ridge > wall_height` wins over
  `pitch` (the pitch is then derived and reported on the roof planes); a shed
  dormer uses `pitch` and halves it when it is not flatter than the main roof.
  The Dormer has `front_wall`, `side_walls` (cheek walls, triangles), `roof_planes`
  (gable 2, shed 1, hip 3; each polygon starts with its eave edge),
  `hole_in_main_roof` (a `RoofHole` of kind Hole whose area equals the sum of
  the dormer roof planes' projected areas), and `window_opening`. `DormerWall`
  carries `start`/`end`/`base_elevation`/`height` so the app can create a wall
  object from it; the cheek walls are triangles under the main roof, i.e. they
  are cut by the main roof like Chief's (RF-16).
* Roof Hole tool: one `RoofHole` per click polygon; pass the holes together with
  a plane to `roof_plane_with_holes`.
* Ceiling plane tool: `CeilingPlane { baseline, pitch_in_12, height_at_baseline
  (scene Y), outline, thickness }`; the plane rises toward the left of
  `baseline.0 -> baseline.1`.
* Auto Roof Return: Chief's Gable/Hip/Full types are approximated by `Full`
  (quad continuing the main plane around the corner), `Half` (its triangle) and
  `Boxed` (level boxed return); `length` runs along the adjacent rake or hip edge
  and projects the same distance past the corner. Slope, extend, shadow boards,
  ridge caps, frieze and gutter options are not modelled.

### Behaviour and limits of the features

* **Holes** must lie completely inside one plane (not touching its boundary) and
  must not overlap an earlier hole. Others are not clipped; their indices are
  returned in `skipped_holes`. A feature spanning a ridge or hip (chimney) needs
  one hole per plane. Skylight curb and glass are measured along the plane
  normal; the frame ring is inset in the plane. The mesh triangulates the polygon
  with holes by bridging (`plan_3d::triangulate::ear_clip_with_holes`).
* **Ceiling planes** are one per (roof plane, room overlap piece) and follow each
  roof plane (same pitch and eave baseline), lowered by `thickness` measured
  along the roof normal. A room spanning a hip/valley gets one piece per plane
  with no mitre between them. Flat rooms are not handled here (they keep the
  floor ceiling height). Only plan overlap is computed; walls are not cut.
* **Dormers**: no overhang, soffit or fascia on the dormer roof; the front wall is
  parallel to the eave; cheek walls are plain triangles. `auto_dormer` returns
  `None` when the footprint does not lie inside the main plane (too big, too
  close to the eave, or past the ridge), when the hip ridge would collapse, or
  for a flat or degenerate plane. A dormer across two planes is not supported.
* **Gable line** rebuilds the roof from the planes' eave baselines, so it needs
  the planes to form one closed ring or one chain with a single straight gap
  (checked by the area tiling test); otherwise it returns `None`. From a footprint
  and edges, set `EdgeKind::Gable` and call `build_roof` instead. The new gable
  wall stands on the old eave line (the edge's overhang becomes the rake).
* **Edge specs** use the exact weighted straight skeleton already described above:
  there is no "pitches within 2:12" approximation. The same exceptions apply
  (rare parallel-jog cases fall back to `Roof::approximate`). `extend_slope_downward`
  adds a strip along the plane's fall line below its eave (planar; the hips shared
  with neighbours are not continued below the old eave), so the planes then no
  longer tile the eave polygon.

## Algorithm

1. **Overhang.** Each footprint edge is moved outward by its own overhang and
   neighbouring offset lines are intersected. That polygon is the eave line at
   `baseline_elevation`.
2. **Weighted straight skeleton.** The eave polygon is a wavefront that moves
   inward as height `t` grows. Edge `i` moves with horizontal speed
   `12 / pitch_i` (a Hip edge) or `0` (Gable and Shed edges, which stay vertical
   and let their neighbours' vertices slide along them). A vertex moves with the
   velocity that keeps it on the offset lines of both its edges. An event loop
   processes the earliest of:
   * *edge event*: an edge shrinks to zero length, its two vertices merge;
   * *split event*: a reflex vertex hits a wavefront edge, splitting the loop;
   * *collapse*: a loop whose vertices have become collinear (zero area) is
     closed with the final ridge segments.
3. **Faces.** Every vertex trajectory is a skeleton arc. Each arc is recorded on
   the faces of both edges it separates, with nodes merged by position, so
   neighbouring planes share their hip/ridge/valley vertices exactly and the
   roof is watertight. Edge `i`'s outline is traced from its eave segment through
   those arcs and lifted to `Y = baseline + t`. Gable and Shed edges emit no
   plane.
4. **Check.** The projected plane areas must tile the eave polygon within 0.1%;
   otherwise the exact path is rejected.

`build_roof` is O(n^2) per event and fine for building outlines.

## Which path runs

The full skeleton is implemented and used whenever it succeeds. This covers
rectangles, L/T/U/H/plus/stepped and general simple polygons (rectilinear or
not), per-edge pitches, per-edge overhangs, and gable/shed edges. The test suite
also sweeps the L, T and U footprints over all single-edge pitch changes.

When it cannot succeed, `Roof::approximate` is set and the roof degrades in two
steps (each plane's `pitch_in_12` always reports the pitch actually used):

1. the same footprint with one uniform pitch (length-weighted mean of the hip
   edges), which still gives a correct hip/valley layout;
2. if that also fails, a hip roof on the footprint's bounding rectangle.

## Limits

* The footprint must be a simple polygon (no self-intersections, no holes).
  Several detached buildings need one `build_roof` call each.
* **Known unsupported case:** two parallel, same-facing wall planes of
  different pitch (or hip next to gable) on either side of a short jog that
  collapses while the faster plane has not met its ridge. The faster plane
  overtakes its neighbour's line and the wavefront needs an "infinite speed"
  vertex, which is not implemented. This shows up only with mixed pitches or a
  Hip/Gable mix on long sides, and falls back to `approximate`. In the
  exhaustive sweeps used during development (L/T/U/notch/hexagon footprints,
  pitches 4/8/12 per edge) about 1% of assignments hit it, and about 3.6% of
  Hip/Gable/two-pitch mixes with up to two gables. Uniform-pitch hip roofs never
  did on the rectilinear test shapes; only very spiky random star polygons with
  overhang (thin slots that the overhang inverts) were rejected, about 1%.
* Overhangs large enough to invert an edge (very short edges, narrow slots) are
  rejected and approximated.
* `EdgeKind::Shed` is geometrically a vertical wall like `Gable` that emits no
  plane; a true single-slope lean-to is produced by making the opposite edges
  Hip and the high wall Shed. Chief's Dutch gable, high shed/gable, knee wall
  and per-wall "upper pitch" options are not modelled.
* `fascia_height` is a constant nominal 6" (`DEFAULT_FASCIA_HEIGHT`); fascia,
  frieze and gutter geometry are not generated.
* `footprint_from_walls` ignores free-standing walls and interior partitions
  and removes collinear vertices (end-to-end walls, T-junctions). A footprint
  that touches itself at a single corner yields a non-simple outline.
