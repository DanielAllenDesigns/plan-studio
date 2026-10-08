# plan-framing

Chief-style "Build Framing" for Plan Studio. Lengths are inches; the 3D frame is X right, Y up, Z = -plan y.

- `frame_wall`: bottom/top plates, studs at 16" o.c. plus an end stud, and for each opening king studs, trimmers, a plied header, cripples above and (windows) a sill with cripples below.
- `frame_floor`: joists clipped to the room polygon (`AlongX`, `AlongY` or `Auto` = span the shorter side), rim joists, optional mid-span blocking.
- `Member`: kind, lumber, length, box transform and a 12-triangle `mesh()` (uses `Material::WallExterior` as a wood stand-in).
- `wall_detail`: the 2D framing elevation (rectangles plus labels) for Chief's Wall Detail view.
- `takeoff`: counts by cut label, board feet from nominal sizes, linear feet per size.
- `FramingDefaults`: spacing, plates, header plies, king/trimmer counts, joist size; 2x4 studs upgrade to 2x6 in walls 6" or thicker.
- Not covered yet: corner and T-intersection framing, combined headers for adjacent openings, wall-top slopes.

## Manually placed framing

- `manual` (`FramingMember`, `LumberSize`, `FramingMaterial`): General Framing, Post, Post with Footing, Blocking, Joist, Joist Blocking, Floor/Ceiling Beam, Floor/Ceiling Truss, Bearing Line, Rafter, Roof Beam, Roof Blocking, Roof Purlin, Roof Truss, Girder Truss and Truss Base as plan-described data with per-kind defaults (joists 2x10 @ 16", rafters 2x8 @ 24", posts 4x4, beams 4x10, trusses 2x4). `to_boxes()` returns `OrientedBox`es (centre, size, axes, yaw/pitch/roll) the app can mesh; `post_with_footing` also returns a `Footing` centred under the post.
- `truss` (`Truss::generate`, `TrussSpec`): Fink, Howe, king post, scissor, attic and mono trusses as 2D chords and webs in the truss plane (centreline nodes, bottom chord split at web nodes; a 24' 6:12 Fink has 9 members), a `TrussEnvelope` outline, plies for girders, and thin 3D boxes.
- `layout` (`JoistDirectionLine`, `RoofTrussDirection`, `BearingLine`, `ReferenceMarker`, `TrussBase`): `frame_floor_directed` (joists run perpendicular to the direction line; a bearing line splits them and adds a beam), `layout_trusses` over a truss base, `frame_wall_with_marker` (first stud on the marker).
- `manual_takeoff`, `combined_takeoff` and `MaterialList::to_csv`: board feet (nominal for sawn, actual for engineered), counts by size and length.
- Not covered: connectors and notches, rim joists in `frame_floor_directed`, truss-to-truss girder placement, truss webs at engineered (non-panel) points, beam sizing.
