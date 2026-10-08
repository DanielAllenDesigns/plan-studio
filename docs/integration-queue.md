# Integration queue (shared-file wiring the tool builders could not do)

Collected from builder reports; applied by the integration pass that owns
main.rs, editor/{selection,handles,ops,render,actions}.rs and tools/select.rs.

## Stairs (editor/stairs_view.rs, tools/stairs.rs, dialogs/stairs.rs)
1. main.rs `open_spec(ObjectRef::Stair(id))` → `StairDialog::new(stairs_view::find(floor, id)?)`; on OK `stairs_view::apply_edit(&mut cx, dialog.draft())`.
2. selection.rs: `ObjectRef::exists` → `Stair(i) => stairs_view::exists(floor, i)`; `layer_of` → "Stairs".
3. select.rs / handles.rs / ops.rs: use `stairs_view::{pick, in_rect, handles, hit_handle, drag_handle, drag_label, delete_selected}`.
4. Edit toolbar: expose `stairs_view::edit_commands(cx)` / `run_command(cx, cmd)` (Auto Stairwell, Flare/Curve, Add/Remove Breakline, Make Railing) — `EditActionKind` needs an `Custom(&'static str)` or stair variants.
5. render.rs `draw_walls` must skip `flags.invisible` walls (stairwell dividers currently draw).
6. tools/mod.rs test `stubs_say_they_are_not_implemented` must point at a remaining stub.

## Rooms / floors / build tools (editor/rooms_edit.rs, dialogs/{room,floor,build_tools}.rs)
- Done by the builder itself: main.rs `show_all` hook + action arm, menus Tools ▸ Checks/Space Planning/Schedules, Build ▸ Floor flyouts, render hooks.
- Review: room selection state lives in a thread-local in rooms_edit.rs (no `ObjectRef::Room`); consider adding `ObjectRef::Room(usize)` + `EditorRequest::OpenRoom` in the integration pass.
- Foundation undo leaves the active floor index clamped instead of decremented (needs EditorContext awareness of floor shifts).
- Room extras (conditioned, finishes, stem wall, fill style, label options) are session-only until RoomName grows fields (plan-core follow-up).

## Template seeding (plan-chiefplan → PlanDefaults)
- Add `layer_sets: Vec<LayerSet>` (named, per-set display/lock/color) to PlanDefaults/Project, seeded from Daniel's template inventory at first run when `~/Documents/Chief Architect Premier X18 Data/Templates/<default>.plan` exists (plan-chiefplan::scan_daniel_templates); Active Layer Display Options gets a layer-set selector like Chief's view selector.
- Wall type names from the template (102 approximate) registered with Chief-like stacks where names match known patterns (Siding-6, Stucco-6, Brick-6, ICF-Stucco, Frame-5 1/2, Foundation …).
- Text styles (12) and dimension sets (14) as named presets in Default Settings ▸ Text / Dimension.

## Roofs (editor/roof_view.rs, tools/roof.rs, dialogs/roof.rs)
1. `ToolId` needs variant payloads (or a shared `set_variant(name)` convention) so flyout entries select Roof Plane / Build Roof / Gable Line / Hole / Skylight / Delete; same problem for Stairs (`note_pick` hack) — unify in tools/mod.rs.
2. main.rs: call the active tool once per frame (`tool.frame(cx, ctx)`) so tool-owned dialogs apply immediately; call `roof_view::auto_rebuild(&mut cx)` after `cx.refresh()` each frame.
3. selection.rs: `ObjectRef::exists`/`layer_of`/delete for `RoofPlane` via `roof_view::exists`; Edit toolbar needs Join Roof Planes / Rebuild actions (open `EditActionKind`).
4. `Q` hotkey → Roof Plane (BINDINGS + toolbar test).
5. Roofs are stored per floor (shown on the top floor). Roof holes are cut in 3D now (see "Roof features" below).

## Cabinets / library placement (editor/placed.rs, tools/{cabinet,library}.rs, dialogs/{cabinet,symbol}.rs)
1. selection.rs `ObjectRef::exists`: `Cabinet(i) => placed::exists(floor, PlacedRef::Cabinet(i))`, `Symbol(i) => floor.symbol(i).is_some()`; `layer_of` → "Cabinets, Base/Wall/…" / symbol.layer.
2. EditorContext::apply_edit_action + ops::delete_objects: route Delete → `placed::delete_placed`, Copy → `placed::copy_placed`, Paste in Place → `placed::paste_placed`, ReverseSwing → `placed::reverse_door_swing`; add `EditActionKind::ReplaceFromLibrary` (placeholder currently reuses OpenObject).
3. main.rs open_spec: `ObjectRef::Cabinet` → `CabinetDialog::new(placed::cabinet_by_id(..))`, OK → `placed::apply_cabinet`; `ObjectRef::Symbol` → `SymbolDialog::new(symbol, layer_names)`, OK → `placed::apply_symbol`.
4. select.rs: pick via `placed::hit_placed(cx, p, tol)`; handles via `placed::placed_handles` + `handles::draw`.
5. Library Browser already calls `library::set_active_item`; `library::library_quick_pick(ui, cx)` available.
6. Cabinet variant selection: `cabinet::request_kind(kind)` before `set_active` (unify with the ToolId variant fix).

## Electrical / terrain (editor/site_view.rs, tools/{electrical,terrain}.rs, dialogs/{electrical,terrain}.rs)
1. `ObjectRef` needs `Device(Id)` (and `Terrain`) variants so Select tool can pick/move devices via `site_view::{device_at, slide_on_wall, flip_side}`; Edit toolbar Flip Side / Rotate.
2. main.rs: host `ElectricalDialog::for_device` / `TerrainDialog::new` through open_spec (currently drawn from the tool overlay; OK applies on next event).
3. render.rs: move `draw_devices` after the openings loop so wall devices draw over wall fill.
4. shell/hotkeys.rs test `sequences_use_the_buffer_and_expire` expects NotImplemented for "Auto Place Outlets" — update to the SetTool variant.
5. Terrain menu wiring: Terrain Specification…, Clear Terrain, `site_view::auto_building_hole` (Make Terrain Hole Around Building).
6. `ToolId` gained `ElectricalVariant`/`TerrainVariant` payloads — adopt the same pattern for Stairs/Roof/Cabinet/Dimension/Text/CAD.

## 3D view / cameras (shell/view3d_panel.rs, tools/camera.rs, dialogs/camera.rs)
1. render.rs draw_plan: call `tools::camera::draw_camera_symbols(cx, painter, cam, None)` so camera symbols show in Select.
2. main.rs open_spec `ObjectRef::Camera(id)` → `view3d_panel::Outbox::global().post(ViewRequest::OpenCameraSpec(id))`; `ObjectRef::exists` for cameras.
3. main.rs send_key: skip `delete_selection` while `view3d.active`; choosing a plan tool from the toolbar should return to the plan view; `on_exit` → `Viewport3d::destroy(gl)`.
4. Bind Shift+J (Full Camera) and Shift+K (Perspective Full Overview).
5. Deferred: Walkthrough tool (plan-view3d has paths now), Add Lights objects, Wall Elevation + Auto Elevations cameras, 3D picking.

## Dimensions / text / CAD (tools/{dimension,text,cad}.rs, dialogs/{dimension,text,cad}.rs) — builder finished
1. Variant selection: add `Action::SetToolVariant(ToolId, &'static str)`; call `tools::{dimension,text,cad}::request_variant(name)` before `set_tool`; then bind K (Circle), Shift+P (Rectangular Polyline), Shift+A (Auto Exterior Dimensions).
2. main.rs open_spec: for `ObjectRef::Cad(id)` try `dialogs::text::open_for` then `dialogs::cad::open_for`; `ObjectRef::Dimension(id)` → `dialogs::dimension::open_for`; each dialog has `show(ctx) -> Outcome` and `apply(cx) -> bool`.
3. Text objects are `ObjectRef::Cad`; `ObjectRef::Text` unused (exists() false).
4. Typed input relies on `cx.temp.editing` placeholder (EditField usize::MAX) — formalize as `EditorContext::capture_typing(bool)`.

## Layer sets / plan views / text styles (plan-core done)
- Dock "Active Layer Display Options": layer-set selector (Project.layer_sets, active) and the view selector combo in row 1 drives `Project.plan_views` (`activate_plan_view`); render uses `cx.project.view_layers()` (effective layer set) instead of `project.layers` directly.
- First run: if Daniel's Chief template exists, `plan_chiefplan::scan` + `apply_seed` into the loaded PlanDefaults (cache result to ~/.plan-studio/defaults.json); add File ▸ Templates ▸ "Import Chief Template…" (rfd, .plan) that runs the same seeding.
- Default Settings tree: Text ▸ Text Styles (TextStyles editor), Dimension ▸ Saved Dimension Defaults (list dialog like Chief's: Edit/Copy/Rename/Delete + active combo).

## Renderer bugs found by the samples builder
- 2D door drawing (plan-app editor/render.rs and plan-layout render.rs) ties the hinge side to `swing_flipped` and ignores `hinge_at_end`; draw hinge from `hinge_at_end`, swing side from `swing_flipped` (plan-3d and plan-check already do).

## Integration pass (done)
Unified `ObjectRef` (Room, Device, Terrain added), `selection::{hit_test_cx, extra_in_rect}`, `handles::handles_for(cx, scale)`, `editor/dispatch.rs` (delete/copy/paste/custom Edit actions), `ToolId::*Variant` payloads, `Tool::frame`, `shell/spec_dialogs.rs` (all dialogs hosted), Terrain menu commands, plan-view/layer-set wiring (`EditorContext::layers()` = active view), 2D hinge fix, File > Templates > Import Chief Template.
Open: first-run auto-scan of Daniel's template; Default Settings text/dimension style editors; `EditorContext::capture_typing`; wall.rs snap tests (other builder).

## Chief catalogs in the app (plan-calib done: 403 catalogs, sizes 89%, symbols via projection, meshes decoded)
- Library Browser "Chief catalogs" node → `plan_calib::ChiefLibrary::discover()`: tree by Core/Bonus/Manufacturer/User → catalog → category tree; search across catalogs; thumbnails (PNG bytes → egui texture, cached); click → bridge the object (`to_plan_library` with decoded size/symbol) into a transient catalog and set it as the active Library item.
- Placed Chief symbols in 3D: convert decoded triangle meshes (decode::mesh) to `plan_3d::Mesh` at the placed transform (new `plan-3d` helper `mesh_from_triangles`), cached per catalog object id.
- Settings: Library ▸ "Chief Architect catalogs: Enabled/Disabled" + path override; licensing note in the dialog.

## Slabs / foundation tools (done)
Applied in the roofs/foundation integration pass: `Floor.foundation` is the typed slot (`FoundationLayer::{load, store}`; old `"Foundation, Data"` records are read when the slot is empty, dropped on the next store, and `plan_core::foundation::migrate_legacy` runs from `roof_view::migrate_legacy` on load); `ObjectRef::Foundation(Id)` is wired through `exists`, `layer_of`, `hit_test_cx` (a slab picked only by its interior ranks below the room), `extra_in_rect`, group move (`dispatch::translate_extra`), Delete (`dispatch::delete_extra`, one undo step) and the double-click / Enter specification (`shell/spec_dialogs.rs`, `FoundationDialog`); `render::draw_plan` calls `foundation_view::draw_foundation` under the walls and the tool no longer draws; `build_view_scene` adds `foundation_meshes` and `project_hash` covers the slot; `build_scene` cuts the platform holes out of the floor and ceiling slabs (`foundation::build_platform`).
Open:
- Only "Floor Material Region" and the wall-flyout "Slab Footing" (a wall flavor, not this tool) remain `todo` in the Slab/Floor flyouts.
- The foundation tool keeps its own selection in a thread-local (`foundation_view::{selected, select}`) while it is active; the Select tool uses `cx.selection`. Merge them if the tool ever needs the Edit toolbar.
- `handles.rs` returns no handles for `ObjectRef::Foundation` (objects move by dragging the body); vertex handles for slab outlines are not built.

## Roof features (done in the roofs/foundation pass; editor/roof_view.rs, tools/roof.rs, dialogs/roof.rs)
Live: Roof Hole / Skylight (rectangle drag or click, `HoleRecord`s on the plane, dashed in plan, cut in 3D with curb, frame and glass), Ceiling Plane (record kind `ceiling`, layer "Ceiling Planes", Delete Ceiling Planes), Auto Dormer + Dormer Specification (record kind `dormer`: main plane + `DormerSpec`, geometry regenerated by `plan_roof::auto_dormer`), Explode Dormer (tool mode and Edit toolbar command), Gable/Roof Line on an eave (automatic planes: Build Roof edge override + rebuild; manual planes: `apply_gable_line`), Roof Return (new Roof flyout entries "Explode Dormer" and "Roof Return", no hotkeys), Roof Plane Specification Holes tab and "Build Roof Edge" tab (`RoofSettings::edge_specs`, fed to `EdgeRoofSpec` through `build_roof_with_specs`). `ObjectRef::RoofPlane(id)` addresses any of plane, ceiling plane or dormer.
Open:
- Explode Dormer keeps the roof planes and the footprint hole but not the dormer walls (a wall object cannot start at the roof surface); ceiling planes have no specification dialog; Roof Return has fixed length 24" (Shift half, Alt boxed) and no dialog; Auto Floating Dormer and Join Roof Planes stay stubs; Build Ceiling Planes (RF-46) is not wired to the Build Roof dialog.
- The wall roof directive's `auto_roof_return` flag and `ExtendSlopeDownward` are not read by Build Roof.
- main.rs: nothing needed. Ceiling Plane and Delete Ceiling Planes show their Chief hotkeys in the flyout; whether the runtime hotkey map binds them is up to `shell/hotkeys.rs`.
