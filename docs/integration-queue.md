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
5. Done in the camera round: vector elevations in the 3D panel (`View3dState::vector`, `dialogs::camera::{elevation_options_with_sun, render_elevation_with}` have callers), Wall Elevation / Auto Elevations / Auto Back-Clipped / Walkthrough / Add Lights as `CameraVariant`s (flyouts in `toolbar.rs`), camera-backed layout boxes (`BoxSource::Camera`, `LayoutRenderContext::camera_drawing`, `dialogs::camera::{layout_context, send_camera_to_layout}`), walkthrough play/record (`View3dCommand::{PlayWalkthrough, RecordWalkthrough}`), lights (`Project::{lights, add_light, ...}` in plan-core `camera.rs`, Adjust Lights, Sun Angle window feeding the ray tracer and elevation shadows).
   Still to wire (files outside the camera round): menus.rs 3D menu (`Create Auto Elevations>` and `Walkthroughs>` are inert; use `C::Tool(V::AutoElevation | V::AutoBackclipped | V::WallElevation | V::Walkthrough | V::AddLights)`, `View3dCommand::{PlayWalkthrough, RecordWalkthrough, AdjustLights}`); bind Adjust Lights to Ctrl+Opt+Cmd+L; call `draw_camera_symbols` from render.rs (item 1) so light and walkthrough symbols show outside the Camera tool; the row-1 Send to Layout button should call `dialogs::camera::send_camera_to_layout` once the app owns a Layout; move lights from hidden CAD records (`plan_core::camera::LIGHTS_LAYER`) to a typed Project slot when model.rs is open; 3D picking.

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
Applied in the roofs/foundation integration pass: `Floor.foundation` is the typed slot (`FoundationLayer::{load, store}`; old `"Foundation, Data"` records are read when the slot is empty, dropped on the next store, and `plan_core::foundation::migrate_legacy` runs from `roof_view::migrate_legacy` on load); `ObjectRef::Foundation(id)` is wired through `exists`, `layer_of`, `hit_test_cx` (a slab picked only by its interior ranks below the room), `extra_in_rect`, group move (`dispatch::translate_extra`), Delete (`dispatch::delete_extra`, one undo step) and the double-click / Enter specification (`shell/spec_dialogs.rs`, `FoundationDialog`); `render::draw_plan` calls `foundation_view::draw_foundation` under the walls and the tool no longer draws; `build_view_scene` adds `foundation_meshes` and `project_hash` covers the slot; `build_scene` cuts the platform holes out of the floor and ceiling slabs (`foundation::build_platform`).
Done in the selection pass: the foundation tool uses `cx.selection` (`foundation_view::{selected, select, clear_selection}` take the context; the thread-local is gone), and slabs, slab holes and platform holes have a corner handle each (`HandleKind::Reshape(i)`, `Op::FoundationVertex`, one undo step "Reshape Foundation Object").
Open:
- Only "Floor Material Region" and the wall-flyout "Slab Footing" (a wall flavor, not this tool) remain `todo` in the Slab/Floor flyouts.
- No edge handles (drag an edge, insert a corner); pads and piers move by their body only.

## Manual framing (done; editor/framing_view.rs, tools/framing.rs)
`ObjectRef::Framing(id)` addresses a manual `framing_view::Record` (members, Joist / Roof Truss Direction, Bearing Line, Reference Marker, Truss Base). Wired through `exists`, `layer_of` (the record's framing layer), `hit_test_cx`, `extra_in_rect`, group move (`framing_view::translate_in`), Delete (`delete_extra`, "Delete Framing"), the double-click / Enter Framing Member Specification (`spec_dialogs`, members only; layout lines and markers have no dialog) and Edit-toolbar Open Object. The framing tool's thread-local selection is gone: it reads and writes `cx.selection` (`framing_view::{selected, select}` take the context), so Select and the framing tools share it. Handles: an end handle at each end of a line member / direction line / bearing line (`Op::FramingEnd`, "Stretch Framing"; the dragged end snaps from the fixed one and keeps at least `MIN_MEMBER`; a rebuilt member that is stretched becomes a manual one) and a corner handle on a Truss Base (`Op::FramingVertex`, "Reshape Truss Base"). Posts and markers move by their body.

## Roof features (done; editor/roof_view.rs, tools/roof.rs, dialogs/roof.rs)
Live: Roof Hole / Skylight (rectangle drag or click, `HoleRecord`s on the plane, dashed in plan, cut in 3D with curb, frame and glass), Ceiling Plane (record kind `ceiling`, layer "Ceiling Planes", Delete Ceiling Planes), Auto Dormer + Dormer Specification (record kind `dormer`: main plane + `DormerSpec`, geometry regenerated by `plan_roof::auto_dormer`), Explode Dormer (tool mode and Edit toolbar command), Gable/Roof Line on an eave (automatic planes: Build Roof edge override + rebuild; manual planes: `apply_gable_line`), Roof Return (Roof flyout entry "Roof Return"; fixed 24" length, Shift half, Alt boxed, no dialog), Roof Plane Specification Holes tab and "Build Roof Edge" tab (`RoofSettings::edge_specs`, fed to `EdgeRoofSpec` through `build_roof_with_specs`). `ObjectRef::RoofPlane(id)` addresses any of plane, ceiling plane or dormer.
Done in the selection pass:
- **Ceiling Plane Specification** (`dialogs/roof.rs::CeilingDialog`, hosted in `spec_dialogs`): General (height at baseline, pitch, thickness), Line Style, Layer; opened by double-click / Enter / Open Object; `CeilingRecord` gained `line_style` and `auto`.
- **Join Roof Planes** (RF-41): `plan_roof::join_planes(a, edge, b)` (extends or trims `a`'s edge to the line where the two planes meet; `None` for parallel planes, edges whose neighbours run parallel to that line, or a result that folds over) and Join mode in `tools/roof.rs` (click an edge of the first plane, or start from the Edit toolbar's "Join Roof Planes" with the plane selected, then click the second plane; `roof_view::join_planes_record`, one undo step; the joined plane becomes manual).
- **Auto Floating Dormer** (RF-49): `DormerRecord::floating`; a floating dormer cuts no hole in the roof plane under it (3D meshes and Explode Dormer), editing it keeps the flag. Roof flyout entry is live (`RoofMode::FloatingDormer`).
- **Build Ceiling Planes** (RF-46): the Build Roof dialog checkbox "Build ceiling planes for vaulted rooms" (`RoofSettings::build_ceiling_planes`); Build Roof makes `ceiling_planes_for_vaulted_room` records (auto, layer "Ceiling Planes") for every room whose `RoomName::has_ceiling` is off ("Ceiling Over This Room" in the Room dialog), replacing the ones it made before; manual ceiling planes stay. Auto Rebuild only reruns on wall changes, so changing a room's ceiling flag needs Build Roof again.
- **Extend Slope Downward** (RF-24): an edge whose wall has that directive continues `EXTEND_SLOPE_DROP` (24") below its eave (`EdgeRoofSpec::extend_slope_downward`).
- **Auto Roof Return** (RF-27): a gable-end wall (Full Gable) with `auto_roof_return` makes a full 24" return (`AUTO_RETURN_LENGTH`) on the planes at both of its corners; the returns are automatic planes without a source edge and are replaced by every rebuild.
Open:
- Explode Dormer keeps the roof planes and the footprint hole but not the dormer walls: `Wall` has no bottom-offset / start-height field (only `foundation_height`, which reaches below the floor), so a wall cannot start at the roof surface. Needs a `Wall` field in plan-core `model.rs` (e.g. `base_offset`) honored by the wall mesh, the plan view and room detection; then `explode_dormer_record` can add `ExplodedDormer::walls` as real walls.
- Extend Slope Downward uses a fixed drop; Chief reaches down to the wall below. Roof Return has no length dialog; Auto Roof Return only applies at gable ends. Dutch gable, knee wall and the upper-pitch directives are not read by Build Roof.
- main.rs: nothing needed. Ceiling Plane and Delete Ceiling Planes show their Chief hotkeys in the flyout; whether the runtime hotkey map binds them is up to `shell/hotkeys.rs`. Join Roof Planes is on the Edit toolbar only, as in Chief.

## Details: trim, material regions, hatching, decks, 3D solids (editor/details_view.rs, tools/details.rs, dialogs/details.rs, plan-core/details.rs, plan-3d/details.rs)
Live: Corner Boards, Auto Place Corner Boards, Quoins, Auto Place Quoins, Molding Line / Polyline, Floor Material Region, Wall Material Region, Wall Hatching, Polygon Shaped Deck, Slab Footing (wall flyout; makes a foundation `Slab` with footing), and the 3D Solid flyout (3D Solid, Face, Cone, Cylinder, Pyramid, Sphere). Data is the typed slot `Floor.details` (`DetailsLayer::{load, store}`); plan drawing is wired in `render::draw_plan` (`details_view::{draw_under, draw_over}`); the tool owns its dialogs and selection.
1. 3D scene (view3d_panel.rs, not edited by the details builder): where the scene is built, add `scene.meshes.extend(crate::editor::details_view::detail_meshes(&project));` (same place as `foundation_meshes`), and include `floor.details` in `project_hash` (hash `floor.details.as_ref().map(|v| v.to_string())` or its serde length) so edits rebuild the cached meshes.
2. Selection (selection.rs / handles.rs / dispatch.rs, not edited): add `ObjectRef::Detail(Id)`; `exists` -> `details_view::exists(cx, DetailRef::..)` via `DetailsLayer::find(id)`; `layer_of` -> `DetailsLayer::layer_of`; `hit_test_cx` -> `details_view::pick(cx, p, tol)` (floor regions and decks rank below rooms like slabs; corner trim and moldings above); `extra_in_rect` -> `details_view::in_rect(cx, lo, hi, crossing)`; group move -> `details_view::translate_ids`; Delete -> `details_view::delete_ids`; `EditorRequest::OpenSpec(ObjectRef::Detail(id))` -> host `DetailsDialog::new(&details_view::load(cx), layer.find(id)?, layer_names)` in `shell/spec_dialogs.rs` (OK applies `dialog.draft().apply(&mut layer)` inside `details_view::edit`). Until then the selection lives in `details_view::{selected, select, clear_selection}` (thread-local) and only the details tool picks, moves (Ctrl/Cmd-drag), deletes and opens these objects. The highlight is drawn by `draw_over` from that thread-local.
3. Wall Material Region uses a plan-length range (press on the wall, drag along it) plus bottom/top heights in the dialog instead of a mini elevation popup; a face is picked by the side of the wall clicked. Wall regions and hatches belong to their wall and do not move with Ctrl-drag (they follow the wall); deleting a wall leaves its regions and hatches orphaned (3D and plan skip them): `dispatch::delete_extra` should drop `regions`/`hatches` whose `wall_id` is gone.
4. Hotkeys: none are bound for these entries (Chief lists none for them in `docs/chief-x18-subtools.md`).
5. Open: the Line Style page of every dialog is shown disabled (objects have no stored line style yet); moldings do not miter at polyline corners (each segment is swept separately); the "3D Solid Feature" entry stays a stub; a Custom molding profile has no profile editor (points come from data only); quoin/corner board follow their wall only through `Auto Place`, they do not move if the wall moves; Pyramid is an extra `SolidKind` beyond the spec's list (the flyout has the entry).
6. `toolbar.rs` test `ALLOWED_NOT_IMPLEMENTED` still lists "Polygon Shaped Deck", "Slab Footing", "Wall Hatching", "Wall Material Region" (harmless; remove them when convenient).
