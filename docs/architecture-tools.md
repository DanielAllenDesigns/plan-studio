# Tool architecture for parallel development

Every Chief Architect tool becomes one Rust module in `crates/plan-app/src/tools/`
behind a shared trait, so independent builders can implement tools in parallel
without touching the same file. `main.rs` only owns the window, panels and the
tool registry; all behavior lives in `editor/` (shared services) and `tools/`.

## Layout

```
crates/plan-app/src/
  main.rs            window, panels, menu/toolbar wiring; PlanApp owns the
                     ToolSet, EditorContext, dialogs and docks
  menus.rs           the Chief menu bar (File ... Help), built from the flyout tables
  toolbar.rs         toolbars, flyouts, BINDINGS, Action, ViewFlag, BarState
  theme.rs, icons.rs, paths.rs, plan_defaults.rs
  editor/            shared services (EditorContext and what hangs off it)
    mod.rs           EditorContext: project, floor, selection, snap, defaults,
                     view_flags, sheet, cached rooms/outlines/framing, requests
    selection.rs     Selection { items: Vec<ObjectRef> }, hit_test_cx, extra_in_rect
    snap.rs          Chief snap engine: object snaps, angle snaps, grid
    handles.rs       edit handles (handles_for(cx, scale)): move, stretch, rotate, swing
    tempdim.rs       temporary dimensions (display + type-to-move)
    history.rs       undo/redo wrapper around plan_core snapshots
    actions.rs       the Edit toolbar (EditActionKind, incl. Custom commands)
    dispatch.rs      routes Delete / Copy / Paste in Place / Reverse Swing and the
                     module-specific Custom commands to the module owning each kind
    render.rs        plan drawing: grid, reference floor, rooms, walls (joins),
                     openings, dimensions, CAD, sheet outline, highlights
    restyle.rs       the Color and Line Weights passes over painted shapes
    sheet.rs         the active layout's sheet size and scale
    ops.rs, connect.rs   wall creation and editing, connection repair
    stairs_view.rs, roof_view.rs, placed.rs, site_view.rs, rooms_edit.rs,
    framing_view.rs  storage, drawing and commands for one object family
    camera.rs        the 2D view camera (not the 3D camera objects)
  tools/             one module per Chief tool behind the Tool trait
    mod.rs           Tool trait, ToolId, ToolSet, registry(); one line per tool
    select.rs wall.rs opening.rs pan.rs dimension.rs text.rs cad.rs cabinet.rs
    library.rs stairs.rs roof.rs electrical.rs terrain.rs camera.rs foundation.rs
    framing.rs       the 19 manual framing flyout entries (members, direction lines, markers, truss base)
  dialogs/           Chief-style specification dialogs and Default Settings lists
    (wall, opening, room, floor, dimension, text, cad, cabinet, symbol, stairs,
     roof, electrical, terrain, camera, defaults, default_lists, hotkeys,
     layer_display, build_tools, exchange)
  shell/             the application shell around the canvas
    docks.rs         Active Layer Display Options and the Project Browser
    library_browser.rs   the Library Browser dock
    hotkeys.rs       runtime hotkey map (BINDINGS + Daniel's Chief keys + user edits)
    spec_dialogs.rs  hosts the specification dialog of every object kind
    view3d_panel.rs  the 3D view, camera requests and rendering commands
```

## The trait

```rust
pub trait Tool {
    fn id(&self) -> ToolId;
    fn name(&self) -> &'static str;               // Chief's name, e.g. "Straight Exterior Wall"
    fn hint(&self) -> String;                      // status-bar hint
    fn cursor(&self) -> egui::CursorIcon;
    /// A variant was picked (wall flavor, door or window, any `*Variant(..)`
    /// payload). Called before `activate` and when switching between variants.
    fn set_variant(&mut self, id: ToolId);
    /// Once per frame, before the canvas is drawn, so a tool that owns
    /// dialogs or palettes can apply what they asked for right away.
    fn frame(&mut self, cx: &mut EditorContext, ctx: &egui::Context);
    fn activate(&mut self, cx: &mut EditorContext);
    fn deactivate(&mut self, cx: &mut EditorContext);
    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult;
    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult;
    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult;
    fn double_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult;
    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult;   // Esc, Tab, Enter, Delete, arrows, typed text
    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera);
    fn edit_toolbar(&self, cx: &EditorContext) -> Vec<EditAction>;         // Chief's context edit toolbar
}
```

Every method except `id` and `name` has a default, so a tool implements only
what it needs.

- `PointerEvent { world, snapped, snap, screen, modifiers, button, down, drag_delta }`
  (`PointerEvent::at(cx, point)` builds one for tests).
- `KeyEvent { key: Option<Key>, text: Option<String>, modifiers }`: `text` carries typed
  digits and quotes for temporary-dimension entry.
- `ToolResult { consumed, repaint, switch_to: Option<ToolId>, commit: Option<String> /* undo label */ }`.
- `EditorContext` gives mutable access to `project`, `floor`, `selection`, the
  undo history (call `cx.begin_change("Move wall")` before mutating; the context
  snapshots for undo), `snap`, `defaults: PlanDefaults`, `layers()` (the active plan
  view's layers), `view_flags`, `status`, `requests` and the cached `rooms`,
  `outlines`, `layer_outlines` and `framing` (recomputed by `cx.refresh()` after
  `cx.mark_dirty()`).

### ToolId and variants

`ToolId` names a tool, and for tools with flyouts a payload that names the exact
sub-tool, so one tool object serves every entry of its flyout:

| Id | Payload |
|---|---|
| `Wall { kind }` | exterior or interior flavor (one object, so a chain survives a switch) |
| `Door`, `Window` | one opening tool object |
| `ElectricalVariant(ElecVariant)`, `TerrainVariant(TerrainVariant)` | outlet, switch, light ...; perimeter, build ... |
| `StairsVariant(StairKind)`, `RoofVariant(RoofMode)`, `CabinetVariant(CabinetKind)` | the flyout entry |
| `DimensionVariant(DimMode)`, `TextVariant(TextMode)`, `CadVariant(CadMode)` | the flyout entry |
| `CameraVariant(CameraVariant)` | Full Camera, overviews, Doll House, sections |
| `FramingVariant(FramingVariant)` | General Framing, Post, Joist, Joist Direction, Bearing Line, Rafter, Roof Truss, Truss Base ... (`framing_view` stores the objects in `Floor.framing`) |
| `FireplaceVariant(FireplaceMode)` | Fireplace, Fireplace in Wall, Prefab Fireplace, Chimney (a placed symbol plus a `Floor::fireplaces` record; `editor::fireplace_view` places, specifies and draws them, `dialogs::fireplace` is the Fireplace Specification) |

`ToolSet::set_active` finds the tool whose `id().same_tool(id)` and calls
`set_variant(id)` with the full payload; `ToolId::base()` gives the plain id.
`ToolSet::active_id()` returns the id the tool was picked with, so the toolbar
can mark the flyout entry. A plain id (no payload) keeps the tool's current
variant.

### ObjectRef

Objects are addressed by `ObjectRef { Wall(Id), Opening(Id), Dimension(Id), Cad(Id),
Text(Id), Cabinet(Id), Symbol(Id), Stair(Id), RoofPlane(Id), Camera(Id), Device(Id),
Room(usize), Terrain, Foundation(Id), Framing(Id) }`. `Room(i)` indexes the detected rooms of the active floor
(`cx.rooms`); `Terrain` is the project's single terrain; `Device` is an electrical
device (`site_view`); `Foundation` is a slab, slab hole, pad, pier or platform hole
(`foundation_view`); `Framing` is a manual framing record (`framing_view`); the
foundation and framing tools keep no selection of their own, they use
`cx.selection`. Text objects are CAD items, so a picked text is `Cad(id)`.
`ObjectRef::exists_in` and `selection::layer_of` tell whether and on which layer
an object lives.

### Editing flow

Pick and edit go through shared services so every kind behaves alike:
`selection::hit_test_cx` and `extra_in_rect` find objects, `handles::handles_for`
gives their handles, `editor/dispatch.rs` routes Delete, Copy, Paste in Place,
Reverse Swing and the `Custom` Edit toolbar commands to the module that owns the
kind, and `EditorRequest::OpenSpec(ObjectRef)` asks the shell to open the object's
specification dialog (`shell/spec_dialogs.rs` hosts them all). Menu and toolbar
commands that are not tools are `toolbar::Action` values handled in
`PlanApp::apply`.

## Dynamic defaults, Set as Default and saved defaults (round 16, brief 26)

An object follows its defaults dialog while its settings are "Use Default"
(manual p. 103). What a tool owner does:

* **A new spec field group** (say a stair's riser, tread and width): add it to
  `DefaultKind::groups()` in `plan-core/src/defaults/dynamic.rs`, give the
  object its Use Default state, and make the dialog's Use Default radio or
  Default check box call `Project::set_group_follows(kind, id, group, on)`.
  An object whose state is unrecorded follows while its value still equals the
  old default's.
* **A follower**: a function that, given the plan defaults before and after,
  gives the objects that follow the new value (`Project::follow_wall_defaults`,
  `Project::follow_type_defaults` for doors and windows, the cabinets'
  `apply_dynamic_defaults`). Call it from
  `plan_app::plan_defaults::follow_changed_defaults`; it runs after any change
  of the plan defaults because `plan_defaults::track` (called once a frame from
  `dialogs::defaults::show_templates_page`) notices the change, so a defaults
  dialog needs no hook of its own. It is one undo step, "Default Settings".
* **Set as Default** is the Edit toolbar button (`plan_defaults::SET_AS_DEFAULT`
  with `can_set_as_default`); a kind that has its own button keeps it.
* **A new tool with several saved defaults** (`SavedKind` in
  `plan-core/src/defaults/saved.rs`): keep the values the tool reads in
  `Project::annot_defaults` or as page values under `SavedKind::prefix()`;
  `Project::saved_activate` swaps them, the Saved Defaults dialog and Default
  Sets need nothing more. A tool that records which saved default made an object
  calls `SavedDefaults::note_use` so Delete is blocked while it is used.
* **Saved plan views** carry `PlanViewSpec` (`plan-core/src/defaults/views.rs`):
  the Selected Defaults, Show Color and the rotation come back through
  `plan_views::view_shown`, which `EditorContext::show_plan_view` calls.

## Rules for tool builders

1. Implement Chief's behavior as specified in `docs/parity/*.md`; cite the ids in
   doc comments (`// W-12`).
2. Only touch your own `tools/<name>.rs`, your crate(s), and one line in
   `tools/mod.rs` to register the tool (and a `ToolId` variant if it has a flyout). If `editor/` needs a new capability,
   add it in a new file `editor/<name>.rs` and one `pub mod` line.
3. Every tool has unit tests for its state machine (drive it with synthetic
   `PointerEvent`s on a `Project` and assert the model), no GUI needed.
4. Status-bar hints and tooltips use Chief's words.
5. Hotkeys come from the runtime map in `shell/hotkeys.rs` (the `toolbar::BINDINGS` base table,
   then Daniel's Chief keys, then the user's edits); tools never read raw keys for activation.
