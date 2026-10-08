# plan-framing

Chief-style "Build Framing" for Plan Studio. Lengths are inches; the 3D frame is X right, Y up, Z = -plan y.

- `frame_wall`: bottom/top plates, studs at 16" o.c. plus an end stud, and for each opening king studs, trimmers, a plied header, cripples above and (windows) a sill with cripples below.
- `frame_floor`: joists clipped to the room polygon (`AlongX`, `AlongY` or `Auto` = span the shorter side), rim joists, optional mid-span blocking.
- `Member`: kind, lumber, length, box transform and a 12-triangle `mesh()` (uses `Material::WallExterior` as a wood stand-in).
- `wall_detail`: the 2D framing elevation (rectangles plus labels) for Chief's Wall Detail view.
- `takeoff`: counts by cut label, board feet from nominal sizes, linear feet per size.
- `FramingDefaults`: spacing, plates, header plies, king/trimmer counts, joist size; 2x4 studs upgrade to 2x6 in walls 6" or thicker.
- Not covered yet: corner and T-intersection framing, combined headers for adjacent openings, wall-top slopes.
