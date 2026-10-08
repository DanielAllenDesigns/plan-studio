# plan-3d

Turns a `plan_core::Project` into renderable triangle meshes and exports glTF 2.0.
No GPU code: the output is plain vertex/index buffers any renderer can upload.

- `build_scene(&Project) -> Scene` builds walls (with door/window holes and reveals),
  door panels, window frames and glass, plus floor and ceiling slabs for detected rooms.
- Coordinates: X = plan x, Y = up (inches, offset by floor elevation), Z = -plan y. UVs are in feet.
- `gltf::export_gltf` returns the `.gltf` JSON and `.bin` bytes; `gltf::write_gltf_files` writes both to disk.
- `triangulate::ear_clip` is a small ear-clipping triangulator that handles concave polygons.
- Roof meshes (from `plan-roof`): `roof_plane_meshes` (plane slab with holes, plus skylight curb `Roof`, frame `Trim`, glass `Glass`), `roof_meshes`, `ceiling_plane_meshes` (underside `WallInterior`, structure `CEILING_FRAMING_MATERIAL`, currently `Material::Floor` because `Material` has no framing variant), `dormer_meshes` (`Stucco` walls, `Roof` planes, `WindowFrame`/`WindowGlass`).
- `triangulate::ear_clip_with_holes` triangulates a polygon with holes by bridging.
