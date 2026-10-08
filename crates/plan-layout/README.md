# plan-layout

Chief's Layout, headless: multi-page sheets of layout boxes showing plans, elevations, sections,
schedules, CAD details, images and text at an architectural scale (paper inches, origin bottom-left).

- `Layout` / `LayoutPage` / `LayoutBox` / `BoxSource`: the serde data model. New fields all have serde defaults, so older layout JSON loads.
  - `Layout.page_background` (default on): Chief layout background (249, 248, 244) filled first on every page; `edge_line_weight` (default 18, 1/100 mm) is the page border ("Layout Edge").
  - `LayoutPage.template_page`: Chief's "Page Template" page. It is not printed; its boxes and CAD (text may use macros) repeat on every other page. Title block and border are drawn on every page regardless.
  - `LayoutBox.hatch_materials` (default on): material hatches in elevation and section boxes.
  - `BoxSource::ImageData { width, height, rgba }`: embedded RGBA pixels (flattened on white), fitted and centred in the box. `Image { path }` stays a placeholder frame.
  - `Layout::sheet_sizes_available()`: every `SheetSize::ALL`, current first. `content_pages()` / `template_pages()`.
- `TitleBlockTemplate::presentation_18x24()` and `from_daniel_18x24()`: 2.5" right strip with PROJECT, CLIENT, ADDRESS, SHEET TITLE, SHEET NO., DATE, SCALE, DRAWN BY; Daniel's adds a REVISIONS table of 5 rows (latest rows of `MacroContext.revisions`, right strip only).
- Macros: `%project.name%`, `%project.number%`, `%client%`, `%address%`, `%designer%`, `%date%`, `%date.long%` (`October 7, 2026`), `%revision%`, `%sheet.number%`, `%sheet.title%`, `%scale%`, `%page.count%`.
- `send_to_layout`: sizes a box to its source at a scale and shelf-packs it into the first free area.
- `send_to_layout_auto(.., scale: Option<Scale>, ..)`: with `None`, the largest `Scale::ALL` scale no bigger than `AUTO_SCALE_CEILING` (1/4") that fits the drawing area (`fit_largest_scale`; pass `Scale::ThreeInch` as the ceiling for the pure largest fit). A 40' x 30' plan is 1/4" on Arch D, 1/8" on Letter.
- `default_construction_set`: cover with sheet index, plans, elevations, section, schedules, framing placeholder; Daniel title block, background and Chief-style box labels (`FIRST FLOOR PLAN` for multi-floor projects, `1ST FLOOR PLAN` for a single floor) with `SCALE: ...` under each scaled box.
- `render_pdf` / `render_box_lines`: PDF via `plan-docs`.
  - One PDF clip rectangle per clipped box (`render_box_lines` soft-clips the same geometry for inspection).
  - Plan views use each layer's RGB colour, `line_weight` (1/100 mm to points, times the box's `line_weight_scale`) and line style (dashed, dotted, dash-dot as PDF dashes). Elevation weights are 0.7/0.35/0.18 pt, cut lines 1.0 pt, hidden lines dashed.
  - Bold title-block labels, schedule headings, room names and box captions; vertical dimension text and CAD text with an angle are rotated.
- `wall_face_hatch` / `HatchStroke` / `pattern_for`: the elevation hatch helper (scene to strokes).

## Material hatch limits

The hatch is built from the 3D scene, not from the elevation's outline loops (those split at every window, door and trim line and cannot be turned into reliable face polygons):

- Only triangles that face the camera and lie in a plane parallel to the view (depth spread under 0.05") are hatched, so vertical wall faces get a pattern; sloped roofs, gables and chimneys cut at an angle do not.
- Mesh material picks the pattern: WallExterior/Siding lap siding, Brick brick, Stone block, Stucco/Concrete stipple. Others (interior walls, trim, glass, roofs) are not hatched.
- Occlusion is not tested: a wall face hidden behind a nearer porch or wing still shows its hatch.
- Section boxes hatch the faces beyond the cut only; cut faces get no poche.
- Courses start at each face's lower-left corner, so bond lines do not line up between different walls; patterns coarsen below 1/32" on paper; at most 40,000 hatch strokes per view.

## Other limits

- Raster images embed as flattened RGB (no alpha); PNG/JPEG files are never decoded.
- The REVISIONS table only draws in the right-strip title block.
- Clip rectangles cut anything in the box, including rotated text; the soft clip used by `render_box_lines` drops text that does not fit whole.
