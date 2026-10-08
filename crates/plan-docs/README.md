# plan-docs

Construction-document outputs generated from a `plan-core` plan. No GUI, no
third-party dependencies.

- `schedule`: door, window, room and wall schedules (`Schedule` -> CSV / Markdown).
- `materials`: framing, drywall, sheathing, siding, flooring and door/window take-off, CSV export.
- `pdf`: minimal PDF 1.4 writer (`PdfDoc`) and `plan_sheet`, a scaled floor-plan sheet
  (ArchD / ArchC / Letter / Tabloid at 1/2", 1/4", 3/16" or 1/8" = 1'-0") with a title block.
- If the plan does not fit at the requested scale, `plan_sheet` steps down to the next smaller scale
  and reports it in `PlanSheetResult { scale_used, fitted }`.
- Floors are addressed by index into `Project::floors`; all lengths are inches.
