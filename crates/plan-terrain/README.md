# plan-terrain

Chief-style terrain engine for Plan Studio (parity CB-43 to CB-48). Units are inches.

- `Terrain` (serde + `Default`, a flat 100' x 80' lot) holds the perimeter, elevation points/lines/regions, modifiers, features, and road/driveway/sidewalk strips.
- `build_terrain`: grid and data sampling, inverse-distance elevation, modifiers, own Bowyer-Watson Delaunay (`delaunay::triangulate`), perimeter/hole clipping, Laplacian smoothing.
- `elevation_at` (bucket index + barycentric) and `contours` (marching triangles, chained polylines, every 5th major).
- `terrain_mesh` / `road_meshes` give plan-3d meshes (X right, Y up, Z = -plan y; UVs in feet); roads drape 0.5" above the surface.
- `auto_hole_for_building` cuts a hole 12" outside a footprint; `plan_symbols` returns perimeter, labeled contours, features and road edges as `Stroke`s.
- Deviations: terrain uses `Material::Floor` as grass, roads use `Roof`/`WallExterior` as pavement; only `Hole` features change the surface.
- `flatten_spline` converts spline control points to polylines for the spline tools.
- Gate: `cargo test -p plan-terrain`, `cargo clippy -p plan-terrain --all-targets -- -D warnings`.
