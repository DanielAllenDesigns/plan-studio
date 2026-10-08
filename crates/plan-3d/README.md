# plan-3d

Turns a `plan_core::Project` into renderable triangle meshes and exports glTF 2.0.
No GPU code: the output is plain vertex/index buffers any renderer can upload.

- `build_scene(&Project) -> Scene` builds walls (with door/window holes and reveals),
  door panels, window frames and glass, plus floor and ceiling slabs for detected rooms.
- Coordinates: X = plan x, Y = up (inches, offset by floor elevation), Z = -plan y. UVs are in feet.
- `gltf::export_gltf` returns the `.gltf` JSON and `.bin` bytes; `gltf::write_gltf_files` writes both to disk.
- `triangulate::ear_clip` is a small ear-clipping triangulator that handles concave polygons.
