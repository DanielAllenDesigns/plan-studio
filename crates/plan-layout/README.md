# plan-layout

Chief's Layout, headless: multi-page sheets of layout boxes showing plans, elevations, sections,
schedules, CAD details, images and text at an architectural scale (paper inches, origin bottom-left).

- `Layout` / `LayoutPage` / `LayoutBox` / `BoxSource`: the serde data model.
- `TitleBlockTemplate::presentation_18x24()`: 2.5" right strip; fields use macros such as `%sheet.number%`.
- `send_to_layout`: sizes a box to its source at a scale and shelf-packs it into the first free area.
- `default_construction_set`: cover with sheet index, plans, elevations, section, schedules, framing placeholder.
- `render_pdf` / `render_box_lines`: PDF via `plan-docs`, boxes clipped in software, pens in paper points.
- Limits: only `plan_docs::Scale`'s four scales; images print a placeholder; text cannot rotate.
