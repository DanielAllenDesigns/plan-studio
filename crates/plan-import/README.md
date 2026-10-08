# plan-import
Brings outside drawings into Plan Studio, like Chief's Import Drawing and CAD to Walls.
- `parse_dxf`: tolerant ASCII DXF reader (CRLF/LF, padded or bare codes) giving a `DxfDrawing` with layers, units, extents, blocks and entities.
- Supports LINE, LWPOLYLINE, POLYLINE/VERTEX, CIRCLE, ARC, TEXT, MTEXT, INSERT; anything else is counted in `skipped`.
- `DxfDrawing::explode_inserts` expands block references (scale, rotation, base point, nesting).
- `to_inches_factor` + `to_cad_objects` produce plan-core `CadObject`s in inches; bulged segments become sampled arcs.
- `cad_to_walls` pairs parallel lines into wall proposals, measures thickness and closes corners and T-junctions.
- `apply_walls` / `apply_cad` add the results to a `Project` with fresh ids.
- DWG and binary DXF are not supported; convert to ASCII DXF first.
- Round-trips `plan_core::write_dxf` output. Test with `cargo test -p plan-import`.
- 3D symbols: `parse_3d(ext, ...)` reads OBJ, glTF/GLB, STL (binary and ASCII), 3DS and COLLADA (.dae, own XML reader) into an `ImportedModel` in inches, Y up, with part colors, material names and texture file names. `suggest` gives the file's own unit and axis (COLLADA, glTF) or the format's axis (STL/3DS are Z up) and a size-based unit guess; `shape` turns (Symbol faces direction) and resizes (3D bounding box). SketchUp `.skp` is refused with how to export instead.
