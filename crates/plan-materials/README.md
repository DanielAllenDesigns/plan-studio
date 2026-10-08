# plan-materials

Chief-style material system as plain data (inches throughout).

- `MaterialDef` + `core_library()`: 45 Chief-like materials (Sand Finish, Drywall, Fir Framing, OSB-Hrz, siding, masonry, roofing, flooring, glass, metals, site) with `find`, `search`, `by_category`, JSON round trip.
- `pattern_strokes()` / `clip_strokes_to_polygon()`: scale-aware 2D hatches (brick, block, shingle, lap siding, board and batten, tile, herringbone, insulation, concrete, earth, grass) clipped to a rect or gable polygon, capped at 20,000 segments.
- `render_texture()`: deterministic, tileable RGBA8 procedural bitmaps (wood, brick, stucco, concrete, shingles, siding, tile, carpet, grass, stone, glass, metal).
- `default_assignments_for()`: Components/Materials tab defaults for Wall, Door, Window, Room, Cabinet, Roof.
- `RenderingTechnique` + `settings()`: Standard, Vector View, Technical Illustration, Watercolor, Line Drawing, Glass House, Physically Based, Clay, Duotone.
- `SunSettings::from_date_time_location()` (solar altitude/azimuth), `LightSource`, `default_room_light()`.
- `MaterialClass` (General, Plastic, Metal, Glass, Mirror, Emissive, Transparent) and `MaterialDef::surface()`: the roughness, metalness, transparency and emissive strength the GL view and the path tracer read, with the class holding them in range. `MaterialDef` also carries the Material Specification's pattern scale and angle, texture offset / angle / blend colour, manufacturer, supplier, price and unit (all `serde(default)`, so older libraries load).
- `MaterialDef::hatch_strokes()` / `pattern_strokes_turned()` / `Pattern::scaled()`: the plan and elevation hatch at the Pattern tab's scale and angle.
- `transform_rgba()` (texture offset, angle, blend colour baked into a bitmap), `list_texture_files()` / `filter_texture_files()` (Chief's texture folders or any folder).
- `blend_name()` / `parse_blend_name()` / `MaterialLibrary::resolve()`: Blend Colors materials named after their two parts and made on the fly.
- `PaintMode` / `PaintScope`: the Material Painter palette's modes and scope.
- `summarize()` / `to_csv()`: the by-surface Materials List (areas per material priced with the specification's unit).
