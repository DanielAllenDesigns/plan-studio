# plan-stairs

Parametric stair engine for Plan Studio (a Chief Architect-style Stair Specification).

- `solve(&StairParams)` rounds the rise to whole risers and checks IRC R311.7: riser 4"-7.75", tread >= 10", width >= 36", headroom, and the 2R+T comfort range (warning only).
- `Stair` + `StairShape`: straight, L, U, winder (pie treads) and ramp, turning left or right.
- `plan_symbol(&Stair, cut_at)` returns `Stroke`s: outline, one line per riser, landings, direction arrow, UP label, and the zigzag break line for stairs cut by the floor above.
- `meshes(&Stair)` returns `plan_3d::Mesh` boxes (treads, risers, stringers, landings, ramp slab, optional handrail); `tagged_meshes` labels each with a `StairPart`.
- `footprint` (stairwell polygon) and `top_point` (where the stair arrives, with elevation).
- Lengths are inches; the local frame has the origin at the bottom riser's left corner.
- Plan-3d has no wood material, so treads/landings use `Floor` and the rest `WallInterior`.
