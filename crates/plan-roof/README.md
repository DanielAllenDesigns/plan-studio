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
