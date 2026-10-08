# plan-cabinets

Parametric cabinet engine mirroring Chief Architect's cabinet model.

- `Cabinet`: a box (base, wall, full height, soffit, shelf, partition) with countertop, backsplash, toe kick and Chief defaults (`Cabinet::base/wall/full_height`).
- `FaceLayout`: Chief's Front/Sides/Back tree (separations, drawers, doors, openings, appliances, horizontal layouts); `resolve()` turns auto heights/widths into rects.
- `plan_symbol()`: 2D plan strokes (outline, countertop, front line, wall-cabinet cross, label).
- `meshes()`: `plan_3d::Mesh` carcass, toe kick, countertop, backsplash, fronts and handles.
- `auto_label()` (B24, B36-SB, W3030, FH2484) and `run_along_wall()` for placing a row against a wall.
- Units are inches; local frame has the back at y = 0 and the front toward +Y; 3D is X right, Y up, Z = -plan y.
- Stand-ins: plan-3d has no wood/laminate materials, so the countertop uses `Material::Floor` and fronts use `Material::WallInterior`.
