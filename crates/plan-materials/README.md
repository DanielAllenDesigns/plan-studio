# plan-materials

Chief-style material system as plain data (inches throughout).

- `MaterialDef` + `core_library()`: 45 Chief-like materials (Sand Finish, Drywall, Fir Framing, OSB-Hrz, siding, masonry, roofing, flooring, glass, metals, site) with `find`, `search`, `by_category`, JSON round trip.
- `pattern_strokes()` / `clip_strokes_to_polygon()`: scale-aware 2D hatches (brick, block, shingle, lap siding, board and batten, tile, herringbone, insulation, concrete, earth, grass) clipped to a rect or gable polygon, capped at 20,000 segments.
- `render_texture()`: deterministic, tileable RGBA8 procedural bitmaps (wood, brick, stucco, concrete, shingles, siding, tile, carpet, grass, stone, glass, metal).
- `default_assignments_for()`: Components/Materials tab defaults for Wall, Door, Window, Room, Cabinet, Roof.
- `RenderingTechnique` + `settings()`: Standard, Vector View, Technical Illustration, Watercolor, Line Drawing, Glass House, Physically Based, Clay, Duotone.
- `SunSettings::from_date_time_location()` (solar altitude/azimuth), `LightSource`, `default_room_light()`.
