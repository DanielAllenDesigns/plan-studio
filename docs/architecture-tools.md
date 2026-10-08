# Tool architecture for parallel development

Every Chief Architect tool becomes one Rust module in `crates/plan-app/src/tools/`
behind a shared trait, so independent builders can implement tools in parallel
without touching the same file. `main.rs` only owns the window, panels and the
tool registry; all behavior lives in `editor/` (shared services) and `tools/`.

## Layout

```
crates/plan-app/src/
  main.rs            window, panels, menu/toolbar wiring (thin)
  editor/
    mod.rs           EditorContext, EditorState, Command dispatch
    selection.rs     Selection { items: Vec<ObjectRef> }, hit testing helpers
    snap.rs          Chief snap engine: object snaps, angle snaps, grid, bumping
    handles.rs       Edit handles (Move, Resize/Extend, Rotate, Reshape, Swing…)
    tempdim.rs       Temporary dimensions (display + type-to-move)
    history.rs       undo/redo wrapper around plan_core::History
    render.rs        plan drawing: walls (joins::wall_outlines), openings, rooms,
                     dimensions, text, CAD, cabinets, stairs, symbols, layers
  tools/
    mod.rs           `Tool` trait, `ToolId`, `registry()`; one line per tool
    select.rs        Select Objects (S-ids from docs/parity/select-and-edit.md)
    wall.rs          Straight/curved walls, railings, room dividers (W-ids)
    opening.rs       Doors and windows (D-ids)
    dimension.rs     Manual, end-to-end, interior, auto exterior… (DM-ids)
    text.rs          Text, rich text, leader line, callout, marker, note
    cad.rs           Line, arc, circle, box, polyline, spline, points
    cabinet.rs       Cabinet tools (plan-cabinets)
    stairs.rs        Stair tools (plan-stairs)
    roof.rs          Roof planes, build roof (plan-roof)
    electrical.rs    Outlets, switches, lights, connect electrical
    library.rs       Symbol placement from plan-library
    camera.rs        Camera/3D view tools (plan-view3d)
    terrain.rs       Terrain tools (later)
```

## The trait

```rust
pub trait Tool {
    fn id(&self) -> ToolId;
    fn name(&self) -> &'static str;               // Chief's name, e.g. "Straight Exterior Wall"
    fn hint(&self) -> String;                      // status-bar hint
    fn cursor(&self) -> egui::CursorIcon;
    fn activate(&mut self, cx: &mut EditorContext);
    fn deactivate(&mut self, cx: &mut EditorContext);
    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult;
    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult;
    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult;
    fn double_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult;
    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult;   // Esc, Tab, Enter, Delete, arrows
    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera);
    fn edit_toolbar(&self, cx: &EditorContext) -> Vec<EditAction>;         // Chief's context edit toolbar
}
```

- `PointerEvent { world: Point, snapped: Point, screen: Pos2, modifiers, button, drag_delta }`.
- `ToolResult { consumed: bool, repaint: bool, switch_to: Option<ToolId>, commit: Option<String> /* undo label */ }`.
- `EditorContext` gives mutable access to `project`, `floor`, `selection`, `history`
  (call `cx.begin_change("Move wall")` before mutating; the context snapshots for
  undo), `snap`, `defaults: PlanDefaults`, `layers`, `view_flags`, `status`, and
  `rooms` (cached detect_rooms, invalidated by `cx.mark_dirty()`).
- Objects are addressed by `ObjectRef { Wall(Id), Opening(Id), Dimension(Id), Cad(Id), Cabinet(Id), Stair(Id), RoofPlane(Id), Symbol(Id), Camera(Id), Text(Id) }`.

## Rules for tool builders

1. Implement Chief's behavior as specified in `docs/parity/*.md`; cite the ids in
   doc comments (`// W-12`).
2. Only touch your own `tools/<name>.rs`, your crate(s), and one line in
   `tools/mod.rs` to register the tool. If `editor/` needs a new capability,
   add it in a new file `editor/<name>.rs` and one `pub mod` line.
3. Every tool has unit tests for its state machine (drive it with synthetic
   `PointerEvent`s on a `Project` and assert the model), no GUI needed.
4. Status-bar hints and tooltips use Chief's words.
5. Hotkeys come from `toolbar::BINDINGS`; tools never read raw keys for activation.
