# Parity spec: 3D Views and Cameras (Chief Architect X18)

> Status (2026-10-08, re-audited against HEAD 4c11b69): 71 ids: 40 Works, 26 Partial, 5 Missing, 0 Differs-by-design. Per-id evidence and gaps are in [../parity-status.md](../parity-status.md); the spec text below is the original and its code snapshots are out of date.

Scope: every camera tool, camera objects in plan and their edit handles, camera
options, 3D navigation, rendering techniques, material tools, lighting and 3D
defaults. Geometry generation for walls/floors (what is in the 3D model) is in
`walls.md`, `rooms-floors.md` and `roofs.md`; this file is about *viewing* it.

Sources: Chief X18 Reference Manual (3D chapters) from memory, the captured
toolbar/menu/hotkey docs (`chief-x18-toolbars.md`, `chief-x18-subtools.md`,
`chief-x18-menus.md`). Lines marked "(verify in Chief)" need checking in the
application. Scene units: inches.

Code read: `crates/plan-3d/src/*` (scene builder, glTF), `crates/plan-view3d/src/*`
(camera.rs, viewport.rs, edges.rs, gpu.rs). At the time of reading `plan-app`
does not depend on either crate (`plan-app/Cargo.toml` lists only `plan-core`),
so no 3D view is reachable from the editor yet.

## 1. Camera tool inventory (toolbar and 3D menu)

C-1. Toolbar row 1 holds these 3D controls, left to right: 3D view flyout (house
glyph), **Full Camera** flyout, **Mouse-Orbit Camera** flyout, **Cross Section
Slider** flyout, **Create Walkthrough Path** flyout, rendering technique flyout
(shows "Standard"), **Add Lights** flyout, **Sun Angle** toggle, Material Painter
and its eyedroppers, Delete Surface, Adjust Material Definition, Interactive
Material Editor. Each flyout shows the most recently used variant.

C-2. 3D menu structure: Create Orthographic View > ; Create Perspective View > ;
Create Auto Elevations > ; Move Camera with Mouse > ; Move Camera with Keyboard
> ; Move Camera > ; Orbit Camera > ; Tilt Camera > ; View Direction > ;
Isometric Views > ; Walkthroughs > ; Materials > ; Material Painter > ; Adjust
Materials > ; Adjust 3D Cladding > ; Material Builder... ; Lighting > ; Camera
View Options > ; Rendering Techniques > ; Toggle Patterns ; Delete Surface ;
Rebuild 3D ; 3D View Defaults... (Cmd+1).

C-3. Opening a camera view creates a new tab/window; every camera type below can
live as a saved view with its own display options, layer set and rendering
technique. Closing the tab keeps the camera object in the plan.

## 2. Full Camera

C-4. Tool: Full Camera, hotkey Shift+J. In floor plan, **click** sets the camera
position (the eye); **drag** from that point sets the viewing direction, with
the view cone drawn live. Releasing opens the 3D view in a new window.

C-5. The camera is created at the current floor with eye height from Camera
Defaults (default 66" above the floor of the floor it is placed on; verify in
Chief) and a target height equal to the eye height (level view). Field of view
default 60 degrees (verify in Chief).

C-6. The camera sits on the floor where it was drawn: eye height is relative to
that floor's finished floor, and the view automatically displays that floor and
all floors below plus the roof, per the camera's Display options.

C-7. Field of view is horizontal in Chief's dialog ("Angle of View"); typical
range 5 to 170 degrees, hard clamp at both ends. (verify in Chief)

C-8. Changing the camera height in the dialog or by dragging the eye handle in a
Cross Section/side view moves the eye vertically while keeping the target
height; the view tilts if the target height is set differently.

C-9. Full Camera views keep **vertical lines vertical** (no roll). Tilt is a
pitch only, adjusted with Tilt Camera or by dragging with the Move Camera tool.

## 3. Overview perspective cameras

C-10. **Perspective Full Overview** (Shift+K): one click in plan creates a
camera orbiting the whole model, looking at the model centre from above at the
default overview angle; it shows every floor and the roof.

C-11. **Perspective Floor Overview**: like Full Overview but shows only the
active floor and floors below; floors above and the roof are hidden. Used to
look at a floor from outside without the roof.

C-12. **Perspective Framing Overview**: shows framing members (and optionally
the structure) with surfaces hidden; framing must be built first.

C-13. **Doll House View**: a floor overview with ceilings and roof of the
current floor and above removed so interior rooms are visible; walls stay full
height (verify in Chief for how upper floors are handled).

C-14. Overview cameras are not placed in plan as a click-drag; they are
generated and appear as camera objects at the overview position (verify in
Chief for whether a camera symbol is drawn). Round 16 brief 32 (DECISIONS 166,
CS11): every saved overview has a symbol in the plan that can be selected,
edited and copied; it stands at the eye, looks toward the target, and places
the view (`CameraObject::overview_pose`, `ViewPose::symbol`).

C-15. **Orthographic Full / Floor / Framing Overview**: the same three overviews
with parallel projection. **Isometric Views** (SW, SE, NE, NW) are orthographic
overviews at the conventional 30/45 degree view angle.

C-16. Orthographic cameras have no field of view; zoom changes the visible
height, and the Camera View Options field "Scale" (if present) sets a drawing
scale for printing (verify in Chief).

## 4. Section and elevation cameras

C-17. **Cross Section/Elevation Camera**: click-drag in plan to define the cut
line. The line's two ends become the section's left and right extents; the
viewing direction is perpendicular to the line and points to one side, shown by
a direction arrow. The side is chosen by the drag direction (or by cursor side
after release; verify in Chief). All objects in front of the line are removed
and the model beyond is drawn.

C-18. Cross Section vs Elevation: the same camera produces an exterior
**elevation** when the line lies outside the model and a **cross section** when
it cuts through. Cut walls, floors and roofs show section fill (poche) in the
cut when "Cross Section" is in effect; outside the model the view is a plain
elevation.

C-19. **Back-Clipped Cross Section**: as above but a rear clip plane limits how
far behind the cut line geometry is drawn, so distant objects do not clutter the
section. The clip distance is a handle (C-33) and a dialog field (Back Clip).

C-20. **Wall Elevation Camera**: a camera for one wall surface, producing an
elevation of that wall face looking straight at it. Created by clicking a wall
side in plan or from the wall's context menu; the camera is placed with the
correct offset from the wall automatically. (verify in Chief for the exact
creation gesture)

C-21. **Create Auto Elevations** (3D > Create Auto Elevations): generates four
exterior elevation cameras (front, back, left, right) positioned on the model's
extents, each named after its side and saved as a view. Re-running updates the
existing four rather than duplicating.

C-22. All section/elevation cameras are orthographic, update live with plan
edits, and produce a **hidden-line** vector drawing in Vector View or a shaded
view in Standard. They are the basis for elevation sheets in Layout.

C-23. Chief's **Cross Section Slider** (toolbar flyout) moves a section plane
interactively through the model in a 3D view for exploration without creating a
permanent camera. (verify in Chief)

## 5. Camera objects in the plan

C-24. Every camera appears in plan as a camera symbol on layer "Cameras": a
small camera glyph at the eye, a view cone (two edge lines) toward the target,
and for sections a line with end arrows and clip lines. Default colour blue
(`#2F6CB3` family, matching the Camera toolbar group).

C-25. Selecting a camera shows edit handles. Full Camera handles: move (centre
of the glyph), rotate/aim (target end of the cone, drags the viewing direction
and angle), field-of-view handles (cone edges; drag changes angle of view),
and a distance handle on perspective overview cameras (verify in Chief).

C-26. Dragging the glyph moves the camera in X/Y; the 3D view updates live while
the mouse is down.

C-27. Dragging the target (look-at) end of the cone rotates the view about the
eye and changes where it points; Shift constrains to 15 degree increments.
(verify in Chief for the modifier)

C-28. Cross Section/Elevation cameras: handles at the line ends (stretch the
cut width), the line centre (move the whole camera), the direction arrow (flip
or rotate), and the **clip handle** (back clip distance for Back-Clipped).

C-29. A camera can be deleted with Delete; the view tab closes. Copy/Paste of
a camera creates another camera with the same options.

C-30. **Camera Specification dialog** (double-click the camera or its view >
Open Object): Name, Camera type, Camera Position X and Y, Height above floor,
Target position X, Y and height, Angle of View, Floor, Display: show hidden
objects, etc. Tabs include General and Display Options; the preview shows the
view. (field list: verify in Chief)

C-31. **Camera View Options** (3D > Camera View Options > Edit, or toolbar
Display Options in a 3D view): per-view settings: Rendering Technique, Floors
Displayed, Lighting set, Backdrop, Sky, Fog/Haze, Ground plane, Layer set,
Textures, Edge/line options, Cutaway/Clip, Pen Weights. These are saved with the
view. (verify tab list in Chief)

C-32. Camera View Options changes apply to the current view only; the *defaults*
for new cameras live in Default Settings > Camera Tools and 3D View Defaults.

C-33. Perspective cameras can be locked ("Locked Camera") so accidental drags in
plan do not move them. Locking prevents handle edits and orbiting. (verify in
Chief)

## 6. Navigation inside a 3D view

C-34. Two interaction modes:
- **Mouse-Orbit Camera**: dragging orbits the camera around a *center of
  rotation*; the camera stays at constant distance. This is the resting mode of
  overview cameras.
- **Move Camera** modes (Full Camera, Mouse-Walk/Pan/Look): dragging moves or
  turns the camera as if walking; the camera orbits the *eye*, not the model.
(verify in Chief for exact tool names under Move Camera with Mouse)

C-35. Orbit: left-drag orbits left/right (yaw) and up/down (pitch). Pitch is
limited so the view cannot flip past straight up/down. The orbit centre
defaults to the centre of the model's bounding box (see C-39).

C-36. Pan: middle-drag or Shift+left-drag (Pan Window tool, hotkey H, has the
same effect). Panning moves the camera and the orbit centre together.

C-37. Zoom/dolly: mouse wheel or Zoom tool moves the eye along its view
direction (perspective) or changes visible height (orthographic). Zoom to a
rectangle with the Zoom tool by dragging a box. Fill Window (Ctrl+F) fits the
view to the whole model; Fill Window Selected Objects fits the selection.

C-38. **Move Camera with Keyboard**: arrow keys walk the camera forward/back
and turn it left/right; with a modifier they strafe and tilt; Page Up/Down raise
and lower the eye. Step sizes come from Preferences > Behaviors. (verify exact
keys in Chief)

C-39. **3D Center**: the orbit/rotation centre can be moved. Chief's Mouse-Orbit
tool lets the user click an object or surface in the 3D view to make it the
rotation centre ("Snap to object"); with nothing selected the centre is the
model centre. A cross-hair marker shows the centre while orbiting. (verify in
Chief for the gesture; the capability is documented)

C-40. **Tilt Camera**: changes pitch only, preserving position; typed values
in degrees are available from the Camera Specification dialog.

C-41. **View Direction** submenu: snap the view to a compass direction (front,
back, left, right, plus corners) keeping the target. (verify in Chief)

C-42. Undo Zoom (button) steps back through zoom/pan changes in the view.

C-43. Selecting objects in a 3D view: click selects the object under the cursor
(surface-based picking), Shift adds, and a double-click opens its dialog, same
as in plan. Moving objects with the mouse in 3D works on the floor plane.

C-44. Hidden/transparent behavior: ceilings and roof are drawn according to the
view's Floors Displayed and the camera's position (a Full Camera inside a room
sees the ceiling; Floor Overview hides it).

## 7. Rendering techniques

C-45. 3D > Rendering Techniques (toolbar flyout, currently "Standard"):
Standard, Vector View, Technical Illustration, Watercolor, Line Drawing, Glass
House, Physically Based, Clay, Duotone (and, in some builds, Sketch/Colored
Pencil/Painterly families; verify list in Chief). The choice is per view and
saved with it.

C-46. **Standard**: OpenGL-style shaded view with materials, textures and
edges, using the view's lighting set. Fast and interactive.

C-47. **Vector View**: hidden-line vector rendering with line weights by
object/layer, optional shadows and fill; used for elevations and sections for
documentation, exported to Layout as vector data (not a bitmap).

C-48. **Technical Illustration**: vector-style with material-colour shading and
thick silhouette lines (a coloured hidden-line drawing).

C-49. **Watercolor**, **Line Drawing**, **Duotone**: stylized, bitmap-based
filters generated from the Standard view (colour washes; plain pen lines; two
tone). Each has its own parameter dialog (stroke weight, colours).

C-50. **Glass House**: all surfaces drawn as transparent so structure behind is
visible; edges are emphasised.

C-51. **Physically Based**: ray-traced (path-traced) photoreal rendering with
quality presets, uses sun, sky and light sources, and progressive refinement
(the image keeps improving until stopped). Requires light sets (see 9).
Output can be saved as an image.

C-52. **Clay**: untextured uniform-material render for form studies.

C-53. Switching technique never changes the model; it only changes the view
settings. Techniques that are bitmap-based re-render when the camera moves.

## 8. Rebuild, Delete Surface and materials

C-54. **Rebuild 3D** (3D > Rebuild 3D): regenerates the 3D model of the plan from
scratch, discarding cached 3D (restores deleted surfaces, re-applies auto-rebuild
settings). F12 rebuilds walls/floors/ceilings (R-33).

C-55. **Delete Surface** (toolbar toggle; 3D menu): click a surface in a 3D view
to remove it from the model for that object, for example a wall face or the
ceiling over one room, to open a view into a space. It is an editing operation
recorded in Undo; Rebuild 3D restores the surface. (verify in Chief for the
restore semantics)

C-56. **Material Painter** (toolbar button): click a surface to apply the
active material from the Material Painter palette/Library. Modes (verify exact
list in Chief):
- Paint Surface: only the clicked surface.
- Paint Object: the whole object's primary surface set.
- Paint Same: every surface using the same material within the object.
Click with Alt/Option picks the surface material (acts as eyedropper).

C-57. **Material Eyedropper** (toggle): click a surface to make its material the
active paint material; **Object Eyedropper**: click an object to pick the whole
object's material set (all its surfaces) for painting onto similar objects.

C-58. **Adjust Material Definition** (toggle): click a surface to open its
Material Definition dialog: name, texture image, colour, shininess, transparency,
bump, reflectivity, texture size/offset/rotation/mirror, "Use Texture Size As
Default". Changes edit the Library material in place or copy it ("Edit a
Copy"). (verify option names in Chief)

C-59. **Interactive Material Editor**: live editing of material properties while
the 3D view updates; keeps the 3D view in front.

C-60. Materials set by the painter override the object's default (walls
materials come from the Wall Types layers in Wall Specification). Material
Painter changes save to the plan, not to the Library.

C-61. **Material Builder** (3D menu): creates and edits material definitions
into the User Library.

## 9. Lighting

C-62. Default lighting in Standard view is automatic: a sun/key light plus
ambient fill, with face shading from normals; no shadows unless the view enables
Shadows.

C-63. **Sun Angle** (toggle; CAD > Sun Angle too): sets sun position by
Latitude, Longitude (or city), Date, Time, Daylight Saving and North direction.
Sun position affects shadows and Physically Based lighting. Toggle Sunlight,
Move Sun and Move Moon adjust it interactively in 3D. (hotkeys in
`chief-x18-subtools.md`)

C-64. **Add Lights**: places light fixtures (library lights/Light tool) in plan
with intensity, colour temperature and cone. The Add Lights flyout offers to
auto-place lights in a room and **Adjust Lights** (Ctrl+Opt+Cmd+L) opens the
light set editor.

C-65. **Light Sets**: a Camera View Options > Lighting setting chooses which
lights are on for the view (None, Auto, Day, Night, Custom sets). Sets are named
and saved in the plan. (verify names in Chief)

C-66. Lights are objects on layer "Lights, Electrical" and are selectable in
plan and 3D; they have intensity, colour, distance and cone angle.

C-67. Shadows: sun shadows and light shadows in Standard view are optional
("Enable Shadows" in Camera View Options > Display).

## 10. 3D defaults and display

C-68. Edit > Default Settings > 3D View Defaults (Cmd+1) holds default
rendering technique, default view options for each camera type, default
lighting and the sun. Camera Tools defaults hold height, field of view and
overview angle. (verify in Chief for exact grouping)

C-69. Display of edges: Standard shows object outlines at a weight set in
Camera View Options (feature edges, silhouette). Line weights in Vector View
follow Layer Display Options pen weights.

C-70. Backdrop and sky: view option for sky colour/image and ground colour;
terrain, when built, is rendered as ground.

C-71. Walkthrough: Create Walkthrough Path draws a path in plan (spline of
camera positions), Walkthrough Preview plays it; an exported movie is Phase 5.
(verify in Chief for editing keyframes)

## 11. Plan Studio today

Plan Studio has camera objects (Full Camera, the cross-section cameras) placed from the plan with a Camera Specification dialog, and an orbitable 3D panel (`shell/view3d_panel.rs`) over `plan-view3d`: Perspective Full and Floor Overview, Doll House, Full Camera, four orthographic elevations, plan overhead, back-clipped sections and a section slider. Navigation is mouse-orbit, pan and dolly; 3D View Defaults (`Cmd+1`) sets new-camera defaults. Nine rendering techniques, the Sun Angle window and a CPU ray tracer with PNG output (`plan-render`) are in; glTF export too. The scene shows walls with openings (casing, jambs, sills, transoms, doors that can stand open), floors, ceilings, roof planes, stairs, cabinets, library 3D models, placed symbols (a box where there is no model), electrical devices, terrain, framing and details. Textures are drawn (`shell/view3d_panel/textures.rs`); with the Select tool a press on a selected cabinet, symbol, device, detail or stair drags it across the floor (`shell/view3d_panel/drag.rs`). Cross sections, Wall Elevation and Auto Elevation cameras draw at the exact angle of their camera line (`plan_elevation::FreeView`), with plan callouts, Auto Interior Elevations, automatic elevation dimensions, material labels, per-layer line weights and DXF export in the Vector View. Elevations can be exported as DXF (File > Export > Elevation DXF). Clicking an object in the 3D view selects it in the plan's own selection (C-43: Shift adds, a double-click opens its specification, Delete deletes it) and tints it in the view. (Refreshed 2026-10-08. The behavior statements in this document are Chief's and unchanged; the gap table below is the original audit and is partly out of date.)

## 12. Gap table

| Chief behavior | Plan Studio today | Severity | Suggested implementation |
|---|---|---|---|
| C-4, C-5 Full Camera by click (position) + drag (direction) in plan, with cone | `FullCamera` mode exists in widget but no plan tool or camera object | Critical | Add `Camera` object type in plan-core (position, height, target, fov, floor) and a plan tool; open 3D tab from it |
| C-10..C-14 Full/Floor/Framing overview, Doll House | Done in earlier rounds; Floors Displayed picks a range of floors (round 15; C15-7) | High | Add `FloorsDisplayed` filter to scene build; add Floor Overview and Framing Overview variants |
| C-15 Orthographic overviews and 4 isometrics | Done in round 16 (brief 32, CS6): 3D > Create Orthographic View opens the Full, Floor and Framing Overviews and the SW, SE, NE and NW Isometric Views as parallel orbits (`Camera::parallel`, `View3dCommand::Parallel`; view3d_panel test `orthographic_overviews_and_isometric_views_are_parallel_orbits`); was: only 4 elevations + plan overhead | Med | Add orthographic Full/Floor Overview and SW/SE/NE/NW iso presets (yaw 45/135/225/315, pitch 35.26) |
| C-17..C-19 Cross Section / Back-Clipped cameras, cut line + direction arrow | Absent; plan-elevation is an empty crate | Critical | Section camera object (two points, depth, back-clip); mesh clipping plane in `plan-3d`; poche from cut faces |
| C-20, C-21 Wall Elevation camera, Create Auto Elevations | Absent | Critical | Generate 4 exterior + per-room interior elevation cameras from model extents/walls; feed plan-elevation |
| C-22 Hidden-line vector output from elevations | `edges.rs` extracts feature edges only on CPU, no hidden-line removal | High | Implement hidden-line via depth-tested edges or analytic HLR in plan-elevation; vector export to Layout |
| C-24..C-29 Camera symbol, handles (move/aim/FOV/clip) | Done in the Camera tool (round 14): symbol, move, aim, clip, angle-of-view handles at the cone corners and a tilt diamond, one undo step each; a locked camera ignores them. The Select tool lists Move, Aim and Clip only | Med | Add the wedge handles to `editor/handles.rs` and `tools/select.rs` (integration-queue item 1) |
| C-30 Camera Specification dialog | Done (round 14): tabs Camera (position, direction, angle of view, height, tilt, clipping, floors displayed, lock), Backdrop, Rendering (technique, Preview or Final View, shadows, lighting override) and Label, saved on the camera | Low | Verify the tab contents against Chief (DECISIONS 160) |
| C-31, C-32 Camera View Options per view; defaults | Per camera: `CameraObject.view` (technique, quality, backdrop with ground and fog, shadows, ambient and sun override, floors picked per floor, tilt, lock, label, light set, orthographic view). Defaults: eye height, angle, technique, callout style. Round 15 (`dialogs/camera.rs`, `scenarios/s50_cameras_r15.rs`; DECISIONS C15-3, C15-4, C15-7) | Low | A layer set per view; saving the defaults with the plan |
| C-33 Locked camera | Done (round 14): `CameraView.locked`, enforced by `apply_handle_with` and `apply_wedge` | - | - |
| C-34 Mouse-Orbit vs Move Camera modes | Mode implied by `CameraMode`; no explicit tool choice | Med | Add `NavMode` toolbar state (Orbit, Pan, Walk, Look) following the 3D menu |
| C-35, C-36 Orbit/pan with clamped pitch, pan with H tool | Present | Done | Keep; add pitch-limit parity test |
| C-37 Zoom to rectangle, Fill Window Selected | `fit_to_bounds` only | Med | Add rubber-band zoom and Fill Window Selected using selected mesh bounds |
| C-38 Move Camera with Keyboard: arrows, PgUp/PgDn | Done (Round 14) for the menu: WASD/arrows/PgUp/PgDn in Full Camera and 3D > Move Camera with Keyboard / Move Camera steps (`shell/view3d_panel/nudge.rs`, 24" per step); the step sizes are not Chief's Preferences > Behaviors values (verify in Chief) | Med | Read the step sizes from Preferences |
| C-39 3D Center / orbit centre pick (Snap to object) | Done: Alt-click sets the orbit centre (`Camera::set_orbit_center`); no cross-hair marker yet | High | Draw a centre marker |
| C-40, C-41 Tilt Camera, View Direction snaps | Done (Round 14) as menu steps: 3D > Tilt Camera (5 degrees) and View Direction (eight compass snaps), `nudge.rs`; no tilt drag tool | Low | A tilt tool with typed degrees |
| C-42 Undo Zoom in 3D | Done (round 15): 3D > Undo Zoom (`View3dCommand::UndoZoom`, `view3d_panel/zoom_history.rs`; tests `a_gesture_is_one_step_back_to_where_it_began`, s50 `undo_zoom_steps_back_through_zooms_and_pans_in_the_3d_view`); includes orbits, 50 steps (C15-6) | Low | The View > Undo Zoom item and hotkey still run the plan's zoom (integration queue) |
| C-43 Surface picking and selection in 3D | Done: ray-triangle pick (`shell/view3d_panel/pick.rs`), selection sync with the plan, 3D drag and hover tint | High | - |
| C-45 Technique list (Standard, Vector, Technical Illustration, Watercolor, Line Drawing, Glass House, Physically Based, Clay, Duotone) | All nine in the live view (shader looks); pictures from the path tracer (walkthrough movies, panoramas, the path-traced Final View, `Snapshot3d`) get Vector View, Technical Illustration, Line Drawing and Watercolor from `plan_render::stylize` (round 15, `plan-render/src/style.rs`; C15-5) | Low | Duotone and Glass House in the path tracer |
| C-47 Vector View with line weights, shadows, fill | Edge overlay only | High | Vector pass producing 2D polylines per layer weight; share with plan-docs for PDF |
| C-51 Physically Based ray-traced render | None | Low | Out of scope until Phase 5; consider `bvh` + CPU path tracer |
| C-54 Rebuild 3D | Done: 3D > Rebuild 3D and 3D > Refresh (round 15, `View3dCommand::{Rebuild, Refresh}`; s50 `refresh_and_rebuild_run_from_the_3d_menu_commands`) | - | - |
| C-55 Delete Surface | None | Med | Per-object surface-hide set in model; skip faces in `plan-3d` |
| C-56..C-60 Material Painter, eyedroppers, Adjust Material Definition, Interactive Editor | Fixed 8 materials | Critical | Introduce `MaterialDef` table (colour, texture, shine, alpha), per-surface overrides on objects, painter tools picking via C-43 |
| C-61 Material Builder, textures | None; UVs in feet already provided | High | Texture loading (image crate) + material library JSON |
| C-62..C-67 Sun Angle, Add Lights, Light Sets, shadows | 3D > Lighting (round 14): a persisted sun (direction, height, strength, from a date), ambient, one switch for the interior lights and the plan's lights with their power; Adjust Lights; per-camera shadows and lighting override. Round 15: **light sets** (`Lighting.sets`, `active_set`, `CameraView.light_set`; Adjust Lights, 3D > Lighting, the Rendering tab; `dialogs::camera::shining_lights`; C15-3) | Low | Light shadows, color temperature, intensities in a set |
| C-68 3D View Defaults dialog | None | Med | Defaults dialog on shared frame, persisted in project |
| C-70 Sky, ground, backdrop | Sky gradient with ground fade; the Backdrop tab adds a sky colour or a picture from Chief's Backdrops folder (read at run time) behind the model in the techniques that draw a sky, a Ground option (fade, flat colour, none) and distance Fog (round 15; `plan-view3d` `Ground`, `Fog`, shader `apply_fog`; C15-4) | Low | A backdrop for the techniques with no sky |
| C-71 Walkthrough path and preview | Done: path from the Walkthrough tool or a CAD polyline, key frames (height, look, tilt, hold), Play with a scrub bar and key-frame jumps, Record Walkthrough as a Motion-JPEG AVI (`plan_render::AviWriter`, baseline JPEG encoder), a numbered PNG sequence or both, at a chosen frame rate and size, in the camera's technique (round 15, `view3d_panel/record.rs`; C15-2) | - | - |
| C-12 Framing Overview | The camera kind exists (`CameraKind::FramingOverview`) and opens as an overview; the framing view fills the scene | Med | The framing builder's own scope and toolbar entry |
| 3D scene contents: roof, stairs, cabinets, terrain, library models | Done: walls, openings, slabs, roofs, stairs, cabinets, symbols (an unknown catalog item is a labelled block), library models, electrical, terrain, framing, details | Critical | - |
| 3D view reachable from editor, tabs per view, Window menu | `plan-app` has no 3D dependency | Critical | Wire `Viewport3d` into `plan-app` as a view tab; move to wgpu later if needed |

<!-- coverage-audit:start -->
## Coverage audit additions (2026-10-08)

Rows added by the Round 14 coverage audit (`docs/chief-feature-coverage.md`): Chief X18 features found in the menu, toolbar, sub-tool and dialog captures, or known from the product, that no row above covered. Status comes from a code search, not a Chief session; "verify in Chief" marks behavior known only from the product. Variants of one flyout or tab share one row.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| C-72 | 3D Solid: Primitive 3D solids and faces with a material, edited by dialog. Also covers: Face; Cone; Cylinder; Pyramid; Sphere. | Works | tools/details.rs DetailsVariant::Solid3d, Face, Cone, Cylinder, Pyramid, Sphere; toolbar.rs solid_3d() |
| C-73 | Adjust 3D Cladding ▸: Adjust the position, scale and offset of siding/brick on a wall surface in 3D. | Missing | no 3D cladding adjust tool (siding start point, course offset) |
| C-74 | Toggle Patterns: Show plan/elevation material patterns instead of textures in the 3D view. | Missing | shown dimmed in Chief's menu; no toggle between material patterns and textures in 3D (grep finds nothing) |
| C-75 | 3D Solid flyout: 3D solid flyout. | Works | toolbar.rs solid_3d() |
| C-76 | Photon mapping / global illumination bake: Older Chief photon-map light bake; modern Chief uses ray tracing only. (Not captured; verify in Chief.) | Differs-by-design | plan-render is a path tracer (plan-render integrator.rs); it needs no photon pass |
| C-77 | 360 panorama / Virtual Tour export: Render a 360-degree panorama and a clickable virtual tour for sharing with clients. (Not captured; verify in Chief.) | Partial | 3D > Export > 360 Panorama: `plan_render::{Projection::Equirectangular, panorama_settings, write_panorama, viewer_html}`, `dialogs/panorama.rs`; tests `position_is_the_inverse_of_direction`, `a_panorama_renders_and_saves_png_and_html_side_by_side`, s50 `export_360_panorama_writes_a_png_and_a_viewer_page_from_a_full_camera` (C15-1). A single panorama and its web page; no clickable multi-room tour (verify in Chief) |
| C-78 | 3D Viewer / Chief 3D sharing (cloud): Upload a 3D model for clients to view in a browser. (Not captured; verify in Chief.) | Missing | Chief's online 3D Viewer service is out of scope; glTF export is the replacement (menus.rs glTF…) |
| C-79 | Photo sharing / Share to social, Chief cloud: Send images and models to Chief cloud services. (Not captured; verify in Chief.) | Missing | cloud sharing is out of scope; Export Picture (Missing) is the local equivalent |
<!-- coverage-audit:end -->

<!-- materials-r14:start -->
## Materials, round 14 (C-55..C-61)

Status and evidence of the Material Painter, eyedroppers, Material Specification and Materials List rows after round 14. "Verify in Chief" marks behavior chosen without a Chief session (`DECISIONS.md` 126-132).

| ID | Status | Evidence |
|---|---|---|
| C-55 Delete Surface | Partial | `tools/materials/paint.rs` `erase` clears the paint of the extent the palette's mode and scope reach as one undo step (`use_default_material_clears_the_paint_in_the_extent`); no per-face hide set and no Rebuild 3D restore (the surface is a paint override, not a deleted face) |
| C-56 Material Painter | Works | modes Component, Object, Room, Floor, Plan and Blend Colors, the Scope dropdown (All Surfaces, Same Material, Same Object Type) and the Use Default Material button (`tools/materials/paint.rs` `paint_click`, `targets_for`; tests `component_object_room_floor_and_plan_reach_more_and_more`, `room_mode_paints_the_room_and_the_scope_narrows_it`, `blend_colors_mixes_the_active_material_into_the_current_one`; scenario s37 `clicks_in_the_3d_view_paint_by_mode_scope_and_use_default_material`); the palette is `palette_window`; Chief's exact mode and scope names: verify in Chief |
| C-57 Material Eyedropper, Object Eyedropper | Works | `paint.rs` `sample` (the clicked component's material, class default or built-in, becomes active; the object's whole set of paint is remembered and painted onto objects of the same kind); tests `the_eyedroppers_and_adjust_pick_the_clicked_material`, s37 `the_eyedroppers_and_adjust_definition_work_on_clicks_in_the_3d_view`; the toolbar toggle ids are `OBJECT_EYEDROPPER` and `ADJUST_DEFINITION` (wiring in `docs/integration-queue.md`) |
| C-58 Adjust Material Definition | Works | `PainterMode::Adjust` opens the Material Specification of the clicked surface's material; OK saves a copy to My Materials under the same name (it takes the core material's place), Revert to Library deletes the copy (`tools/materials.rs` `finish_spec`; test `ok_saves_the_specification_to_my_materials_and_revert_goes_back_to_the_core_one`); "Use Texture Size As Default" and mirror are not built |
| C-59 Interactive Material Editor | Works | with Update 3D view while editing on, the open specification is the material of its name in the 3D view (`set_preview`, `library()`; tests `the_specification_previews_live_and_cancel_puts_the_library_back`, s37 `the_material_specification_drives_the_3d_view_and_both_renderers`); keeping the 3D view in front is the window system's |
| C-60 Painter overrides defaults | Works | resolution is the object's own paint, then its class default (`Project::material_defaults`, Default Settings > Materials: `tools/materials/defaults.rs`), then the built-in material (`override_def_with_class`; tests `class_defaults_paint_unpainted_objects_and_the_wall_paint_wins`, s37 `materials_defaults_reach_the_3d_scene_and_undo_as_one_step`); painter changes save to the plan |
| C-61 Material Builder | Works | Material Specification dialog with the tabs Pattern, Texture, Properties and Materials List (`tools/materials/spec.rs`), saved to `~/.plan-studio/materials.json` |
| C-61a Material Specification: Pattern | Works | CAD pattern, scale and angle drive the plan and elevation hatch (`MaterialDef::hatch_strokes`, `tools::materials::hatch_strokes`; test `pattern_scale_and_angle_change_the_hatch`, s37 `the_pattern_tab_scale_and_angle_change_the_plan_hatch`); the plan regions of `editor/details_view.rs` still use the core library until the queue item is applied |
| C-61b Material Specification: Texture | Partial | picture from Chief's texture folders (read at run time, `plan_materials::list_texture_files`) or any file, tile size, offset, angle and blend colour baked into the bitmap (`transform_rgba`); angles that are not multiples of 90 degrees may show a seam; bump maps are not read |
| C-61c Material Specification: Properties | Works | (manual pp. 1123-1126: Chief has ten classes, General, Matte, Mirror, Plastic, Polished, Predefined Metal, Shiny Metal, Translucent, Transparent and Water; Glass is a Transparent material and emissive is a setting; ours are the older names below, migrated in Round 17) class General, Plastic, Metal, Glass, Mirror, Emissive, Transparent with roughness, metalness, transparency and emissive that the GL view (`u_rough`, `u_metal`, `u_emissive`, colour alpha) and the ray tracer (`Surface::painted`) honour (`plan_materials/src/tests.rs a_class_sets_typical_values...`, `plan-render/tests/paint_surface.rs`, `plan-view3d gpu.rs paint_surface_tests`); the class numbers are ours (verify in Chief) |
| C-61d Material Specification: Materials List | Works | manufacturer, supplier, price, unit and accounting code (`MaterialDef::{manufacturer, supplier, price, unit}`) |
| C-61e Library material browsing | Works | the Library dock's Objects / Materials switch (`shell/docks.rs`, `tools/materials/browser.rs`): search, categories, My Materials filter, edit, Save to My Materials, Delete from My Materials (test `search_and_my_materials_filter_the_list`, s37 `the_library_dock_filters_to_materials_and_saves_to_my_materials`) |
| C-61f Materials List by surface and region | Works | Tools > Materials List > By Surface: Room, Active floor or Whole plan, with manufacturer, supplier, price, unit, quantity and cost per material, CSV export (`tools/materials/surfaces.rs`, `plan_materials::summarize`; tests in `surfaces.rs`, s37 `the_materials_list_by_surface_adds_areas_by_region_with_prices`). Manual p. 1105: Chief does not calculate wall materials applied with the Material Painter (cosmetic, apart from foundation wall footings) and counts objects by their center point; ours counts both faces of painted walls, to become a named option (Round 17) |
<!-- materials-r14:end -->

## Material packages (Lightbeans), round 15 (Beyond Chief)

Chief has no PBR texture sets; these rows are ours. Lightbeans (lightbeans.com/en/textures) offers manufacturer materials as seamless PBR sets, one zip each. Plan Studio never downloads or bundles them: you download, the app imports (`DECISIONS.md` LB1 to LB4). "Verify with a real zip" marks the file-name and metadata matching, which was written without a sample.

| ID | Status | Evidence |
|---|---|---|
| BC-PKG1 Material package reader | Works | `plan_materials::package` `MaterialPackage::read` / `read_path` (own zip reader and JPEG / PNG decoders; maps downsampled to `ReadOptions::max_side`, default 2048, raw files kept); `classify`, `parse_metadata`, `size_from_metadata`; tests in `package/tests.rs` (`a_synthetic_zip_reads_into_a_package`, `map_names_are_matched_case_insensitively_by_substring`, `metadata_lines_parse_with_colons_equals_and_units`); verify with a real zip |
| BC-PKG2 Import to the user library | Works | `MaterialPackage::import_to` copies the maps to `~/.plan-studio/textures/<package>/`, `MaterialDef::from_package` sets texture, tile size from the real-world size, class General, manufacturer, map paths, source and date (tests `import_copies_files_and_records_the_source_and_date`, `from_package_sets_texture_scale_class_maker_and_map_paths`; app: `tools/materials/package.rs` `import_zip`, test `importing_a_zip_copies_the_maps_and_saves_a_material`) |
| BC-PKG3 File > Import > Material Package... | Works | one or many zips (`menus.rs`, command `materials.import_package`, `package::import_command`; tests `the_import_command_uses_the_file_box_and_survives_cancel`, `many_zips_import_together_and_the_last_becomes_active`); a `.zip` dropped on the window imports too (`files.rs`, `package::handle_dropped`; s53 `a_zip_dropped_on_the_window_is_imported`) |
| BC-PKG4 Lightbeans folder in the Library Browser | Works | Materials view: Import Material Package, "Browse Lightbeans textures..." link (opens `https://lightbeans.com/en/textures` in the system browser), the imported packages with thumbnails, the "New downloads" group (`package::lightbeans_folder`; tests `the_lightbeans_folder_draws_headlessly_with_packages_and_new_downloads`, `the_browse_row_opens_the_lightbeans_page`) |
| BC-PKG5 Watch Downloads folder | Works | Preferences > Render, off by default; polled every 3 s on the UI thread, no threads (`package::poll_downloads`, `scan_downloads`; tests `scanning_downloads_finds_only_new_package_zips_once`, `the_watcher_lists_new_downloads_until_imported_or_dismissed`) |
| BC-PKG6 Package maps in the 3D view | Works | normal (tangent space, planar frame), roughness, metallic, ambient occlusion and opacity cut-out in the fragment shader (`plan-view3d gpu.rs`, `u_pbr`, `u_nrm_tex`, `u_orm_tex`; GLSL `tangent_frame` mirrors `plan_materials::pbr::tangent_frame`, tests `tangent_frames_are_orthonormal_right_handed_and_follow_planar_uv`, `the_tangent_frame_glsl_mirrors_the_rust_frame`); scalar class values stay where a map is absent |
| BC-PKG7 Package maps in the ray tracer | Works | roughness, metallic, occlusion, normal, opacity and the albedo of a painted package material (`plan-render pbrmap.rs`; tests `maps_replace_roughness_metalness_albedo_and_bend_the_normal`, `an_opacity_map_cuts_the_surface_out`); occlusion darkens the diffuse albedo; shown for painted meshes only |
| BC-PKG8 Texture tab map switches | Works | per-map checkboxes for the package's maps on the Texture tab (`spec.rs package_maps`; `MaterialDef::maps_off`, test s53 `the_pbr_maps_preference_and_the_texture_tab_switches_turn_maps_off`) |
| BC-PKG9 PBR preferences | Works | Preferences > Render: PBR maps on / off, max texture size (1024 to 8192), Watch Downloads folder (`RenderPrefs::{pbr_maps, max_texture_side, watch_downloads}`) |
| BC-PKG10 Paint and undo | Works | a package material paints like any other (one undo step "Paint Material"; s53 `an_imported_package_paints_a_wall_with_its_maps_and_undoes`) |
| BC-PKG11 Open | Open | EXR / TIFF maps (the decoders read PNG and JPEG only; skipped with a warning); displacement is a bump only (no geometry); metallic-roughness packed maps (ORM) are not split; no transmission / sheen / clearcoat maps |

<!-- cameras-r15:start -->
## 3D views and cameras, round 15

Status and evidence of the rows this round touched. "Verify in Chief" marks behavior chosen without a Chief session (`DECISIONS.md` C15-1 to C15-10).

| ID | Status | Evidence |
|---|---|---|
| C-77 360 panorama | Partial | see the audit table above; the viewer page needs a browser with WebGL (a flat canvas is the fallback) |
| C-71 Walkthrough video | Works | `plan-render/src/{avi,jpeg}.rs` (tests `frames_round_trip_through_the_avi_and_our_jpeg_decoder`, `a_picture_round_trips_through_our_decoder`), `view3d_panel/record.rs` (`a_video_is_one_avi_of_every_frame_and_no_pngs`, `stopping_early_leaves_a_playable_movie`), s50 `a_walkthrough_records_a_motion_jpeg_movie_that_decodes`; AVI 1.0, so a movie stays under about 3.5 GB |
| C-65 Light sets | Works | `plan-core camera_view.rs` (`light_sets_are_named_unique_and_follow_a_rename`), `dialogs::camera::AdjustLightsDialog` light sets, s50 `light_sets_choose_which_lights_shine_for_the_plan_and_for_one_camera`, `a_deleted_light_leaves_every_light_set`; sets store on/off only (verify in Chief) |
| C-70 Ground and fog | Works | headless GL test `fog_tints_the_model_and_the_ground_option_changes_the_horizon` (ignored, needs a GL context), `view_settings::{ground_of, fog_of}`, s50 `ground_and_fog_from_the_backdrop_tab_reach_the_viewport` |
| C-45/47/49 Techniques in pictures | Partial | `plan-render/src/style.rs` tests (`line_drawing_is_white_with_dark_lines_on_the_boundary`, `vector_view_uses_flat_grey_tiers_and_black_lines`, `technical_illustration_keeps_the_material_hue_in_flat_tiers`, `watercolor_softens_darkens_the_edge_and_is_deterministic`); the numbers are ours (verify in Chief) |
| C-42 Undo Zoom (3D) | Works | `view3d_panel/zoom_history.rs`, s50 `undo_zoom_steps_back_through_zooms_and_pans_in_the_3d_view` |
| C-6/C-44 Floors Displayed per floor | Works | `view_settings::scope_of`/`clip_below`, s50 `floors_displayed_can_pick_a_range_of_floors` |
| Save Camera of an orthographic view | Works | `view3d_panel/extras.rs`, s50 `an_orthographic_view_is_saved_as_a_camera_and_restored`, panel test `save_camera_keeps_an_orthographic_view_as_an_orthographic_camera` |
| C-53 Final View in the panel | Works | `view3d_panel/final_view.rs` (`the_render_waits_for_a_still_camera_then_starts_once`, `stop_ends_the_render_early_and_keeps_the_picture_so_far`), s50 `the_path_traced_final_view_waits_for_a_still_camera_refines_and_stops` |
| C-54 Rebuild 3D, Refresh | Works | s50 `refresh_and_rebuild_run_from_the_3d_menu_commands` |
| C-73 Adjust 3D Cladding | Missing | not built (a siding start point and course offset need per-wall surface parameters in `plan-3d`) |
<!-- cameras-r15:end -->

## Manual audit additions (part 5)

Rows added by the Chief X18 Reference Manual audit, part 5 (pages 1099 to 1306; `docs/chief-manual-coverage/part5-materials-3d-rendering-pictures-import-export.md`). Each is a manual feature that had no parity row. Status comes from a code search on 2026-10-08 (Round 15 builders still editing); lines marked verify in Chief are not confirmed against the program.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| C-80 | Keep Pattern/Texture in Sync and Global Symbol Mapping (manual p. 1100): Keep Pattern/Texture in Sync: one check box ties scale, offset and angle of pattern and texture together. | Missing | the Pattern and Texture tabs hold separate scale and angle (`pattern_scale`, `texture_scale_in`) |
| C-81 | Stretch to Fit textures and Retain Aspect Ratio on the Texture tab (manual pp. 1100-1121): Stretch to Fit textures (artwork that does not tile and is stretched across the surface, for picture frames); Texture Scale X and Y, Stretch to Fit, Retain Aspect Ratio, Reset Original Aspect Ratio. | Partial | Texture tab has only a tile size; no stretch flag / Tile size width and height (C-61b); no Stretch to Fit, no aspect lock, no reset |
| C-82 | Material maps beyond the PBR set (anisotropy, rotation, emissive, translucent, transmission, transparent) (manual pp. 1100-1126): Fourteen material map kinds: ambient occlusion, anisotropy, bump, emissive, metal, normal, opacity, rotation, roughness, texture, translucent, transmission color, transmission roughness, transparent; Material Class Mix for General materials: Metal Map, Translucent Map with Translucency, Transmission Color Map, Transmission Roughness (+map), Transparent Map, Index of Refraction. | Partial | normal, roughness, metallic, height (bump), ambient occlusion and opacity maps from a package (BC-PKG1, BC-PKG6, BC-PKG7, BC-PKG8) plus a scalar bump; anisotropy, rotation, emissive, translucent, transmission and transparent maps are not read (BC-PKG11) / see the maps row |
| C-83 | Per-map Invert and Remove on the Texture and Properties tabs (manual p. 1101): Invert check box next to each map (all but texture, normal, ambient occlusion and transmission color). | Missing | only `MaterialDef.normal_flip_y` for a DirectX normal map; no per-map invert |
| C-84 | Emissive materials as area lights in ray-traced views (manual p. 1101): Emissive materials act as Area Lights in ray-traced views, and a fixture's bulb material can take its color and intensity from the fixture's own lights. | Partial | `plan-render` `AreaLight` panels are sampled directly and can be set in the Ray Trace window, and an Emissive class material glows (C-61c); the plan's Emissive materials do not become area lights, and there is no "Apply Emissive and Color from Light(s)" switch |
| C-85 | Material structure type and Materials List calculation method (Area, Count, Linear, Volume, None) (manual pp. 1101-1128): Every material has a Materials List calculation method (piece, area, volume, none); Structure type of a material (Concrete, Masonry, Other, Framing) decides brick ledges, rebar, steel mesh and pour counts; Calculation methods Area, Count, Linear, Volume, None; Count uses width, height and depth; Linear is strip length; None hides the material; Representing overlap: set the pattern to the exposed size, the texture to the rendered size and the Materials List panel to the full piece size, without Update from Pattern; Materials List panel: structure type, calculation method, Width, Height, Depth, Update from Pattern. | Missing | the Materials List tab holds price and unit only; the take-off adds surface areas (C-61f) / no structure type on `MaterialDef` / Materials List by Surface (C-61f) adds areas only |
| C-86 | Material Defaults categories and multi-select (Room Moldings, Cabinet Door/Drawer, Countertop, Hardware) (manual p. 1102): Named default categories shared across dialogs (Room Moldings for base, crown, chair rail and casing; Cabinet, Cabinet Door/Drawer, Countertop and Hardware for the cabinet defaults); Material Defaults dialog: scrollable category list, Shift/Ctrl multi-select, Select Material button or either preview box opens Select Material. | Partial | defaults.rs has no moldings, countertop, hardware or door/drawer categories; the Cabinet class holds the cabinet parts / `tools/materials/defaults.rs` shows class and part rows with a picker per row; no multi-select, no preview boxes (same row as the categories gap) |
| C-87 | Select Material dialog (Library Materials, Plan Materials and Material Defaults panels) (manual pp. 1103-1108): Select Material dialog opens when the painter starts; Use Default Material check box (library panel) or "Use Default" row (plan panel) paints the default; Select Material dialog with a Library Materials panel, a Plan Materials panel and (only for a library symbol's component) a Material Defaults panel; Material Defaults panel: tie a library symbol component to a Material Defaults category. | Partial | C-56: the palette window and its Use Default Material button; there is no modal Select Material dialog / a drop-down list per component; no modal dialog (see the new row above) / symbols paint by name (Symbol Specification Materials tab, DECISIONS 132) |
| C-88 | Copy Selected Material button in the painter (manual p. 1103): Copy Selected Material button: copies the chosen material, opens Define Material and loads the copy into the painter. | Missing | the Material Specification saves to My Materials under the same name (C-58); no copy-and-paint button |
| C-89 | Scoping modes and the Create Copy of Material prompt for material editing (manual pp. 1104-1111): The same five scoping modes appear on Adjust Material Definition and the Interactive Material Editor; Adjust Material Definition: click a surface, choose a scope mode, answer the Create Copy of Material prompt (Create a Copy with a name, or Edit the Source Material), edit in Define Material; Five editing scoping modes (Component, Object, Room, Floor, Plan; Plan is the default and edits the source). | Partial | Adjust opens the clicked material's specification without a scope; no mode buttons / C-58 opens the Material Specification of the clicked surface's material; no scope buttons, no copy prompt / see the scoping-modes row |
| C-90 | Missing-texture warning on the Materials panel (manual p. 1105): Caution symbol on the panel name and "Texture Missing" in the preview when a material's texture file is missing. | Missing | `tools/materials` shows no missing-file warning |
| C-91 | Plan Materials dialog (In Use, Purge, Merge, Replace, Add to Library) (manual pp. 1108-1115): Plan Materials panel: list of materials already in the plan, with "Use Default" at the top; Plan Materials dialog New and Copy buttons; Add to Library; Plan Materials dialog (3D > Materials > Plan Materials): search box, sortable list of the materials in the plan, In Use column; Buttons Edit, New, Copy, Purge (remove unused), Delete (unused only), Merge (many into the first), Add to Library, Replace (swap a material, defaults included, with a library one). | Missing | no Plan Materials list (new row below) / see the Plan Materials row / no dialog; the Materials window lists library and plan materials without usage counts (`dialogs/materials.rs`) |
| C-92 | Material preview pane (shapes, techniques, backdrops, orbit) (manual pp. 1108-1120): Rendering technique, object shape and backdrop chosen in Select Material, Plan Materials and Define Material are shared between those dialogs (shape and backdrop also with the Library Browser); Preview with Physically Based, Standard or Vector View, zoom and orbit, Color on/off, Restore Original View, Cube, Sphere, Teapot, Plane; choices persist after closing; Dialog preview pane: orbit, zoom, technique (Physically Based, Standard, Vector), Rotate Spherical Backdrop, Mouse Orbit, Color, Restore Original View, shapes, backdrops. | Missing | no preview technique, shape or backdrop controls anywhere / no material preview pane beyond the sphere swatch / see the preview pane row |
| C-93 | Plan-specific material definitions that travel with the plan (manual p. 1108): Material definitions are plan-specific: editing a material changes only the current plan, not the library or other plans. | Differs-by-design | materials live in the user library by name (`Project::object_materials` stores names); a plan carries no copy of the definition, so a plan opened elsewhere loses custom materials |
| C-94 | Interactive Material Editor axis handles (scale, rotation, offset) (manual p. 1109): Interactive Material Editor: an axis polyline at the click with handles for scale (round), Y scale and X scale (square), rotation (triangle) and offset (move); Tab on a handle opens an exact-value box. | Missing | C-59 gives live preview while the specification is open, not on-surface handles |
| C-95 | Create materials from a pasted image or a screen capture (Stretch to Fit) (manual p. 1112): Paste Image: copy an image, then Edit > Paste to make a material from it; Screen Capture to a material (Stretch to Fit by default). | Missing | no image-clipboard paste into the library / no screen capture tool (see the Screen Capture rows) |
| C-96 | Convert Textures to Materials and Create Plan Materials Library (manual p. 1113): Convert Textures to Materials: a whole folder of texture images becomes a library of materials with the same folder structure; Create Plan Materials Library: a library folder made from all materials used in the plan. | Missing | Lightbeans zips import one package at a time (BC-PKG3); no folder conversion / no such command |
| C-97 | Pattern tab: line color, line weight, shading contrast, offsets and library patterns (manual pp. 1118-1119): Pattern panel colors: Material Color, pattern Line Color, Line Weight, Shading Contrast (used when the Vector View option Apply Shading Contrast is on); Pattern Type drop-down with Library choice, custom patterns, Pattern from Texture and Add Pattern to Library. | Partial | Material Color is on the Properties tab; Line Color, Line Weight and Shading Contrast are missing / a CAD-pattern drop-down (`PATTERNS`) only; no library or custom patterns |
| C-98 | Texture tab sources: TIFF, BMP, GIF and zip textures (manual p. 1121): Texture panel: material name; Texture Source path with Browse, Remove, .jpg/.bmp/.png/.gif/.tif and textures inside .zip files. | Partial | picture file with Browse, a Generated button and a search of Chief's texture folders (C-61b); PNG and JPEG read, no TIFF, BMP or GIF, no zip |
| C-99 | Set Material Color Using Texture (manual p. 1122): Material Color on the Texture panel: Blend with Texture, Color button, Set Material Color Using Texture. | Partial | blend color and amount (C-61b); no "use the texture's predominant color" button |
| C-100 | Material classes Matte, Polished, Predefined Metal, Shiny Metal, Translucent, Water and their settings (manual pp. 1123-1204): Properties panel: a Material Class list of ten classes (General, Matte, Mirror, Plastic, Polished, Predefined Metal, Shiny Metal, Translucent, Transparent, Water); Class settings: Apply Emissive and Color from Light(s), Diffuse, Emissive presets and value, Metal type, Metallic, Opacity Map, Reflection (+color), Roughness (+map), Thin, Transparency; Water class: wave style (Basic, Detailed, Ripply, Flowing Sheet), Wave Chop, Wind Speed, Wind Direction, Wave Scale; Transparency three ways: General transparency, Transparent class with index of refraction, Translucent. | Partial | seven classes: General, Plastic, Metal, Glass, Mirror, Emissive, Transparent (`MaterialClass`); Matte, Polished, Predefined Metal, Shiny Metal, Translucent and Water are missing / sliders for Roughness, Metalness, Transparency, Emissive and Bump (C-61c); no Diffuse, Reflection color, Thin, Metal type or emissive presets / no Water class |
| C-101 | Clear Coat and Brushed settings (ray-traced) (manual p. 1127): Clear Coat (ray-traced): roughness, roughness map, normal map, use base normal or bump; Brushed (ray-traced): anisotropy, anisotropy map, rotation, rotation map. | Missing | no clear coat / no brushed finish |
| C-102 | Pattern from Texture dialog (manual p. 1129): Pattern from Texture: Source map and file, Simple threshold, Advanced Low/High thresholds, Auto Compute Thresholds, Use Threshold Factor (1-10), Filter Size, Original and Output previews. | Missing | no edge-trace of a texture into a CAD pattern |
| C-103 | Material Builder dialog with parametric builders (Masonry and Stone, Tile, Wood) (manual pp. 1131-1138): 3D > Material Builder opens a dialog built on Substance: pick a builder, set inputs, set outputs, Add to Library; Builder choices: Masonry and Stone, Tile, Wood, or an external Substance (.sbsar) file; Material Outputs: maps included, name, Material Scale (default 20 inches), Open Material When Added to Library; Reset to Defaults; Masonry and Stone builder inputs: output size, random seed and Randomize, shape, pattern layout (1-3 colors, checker or random), stone fill including custom image, roughness, invert, scale, cleavage, roundness, wany edge, row offset; Masonry and Stone margins and stone groups: margin fill, width, hue, saturation, lightness; up to three stone groups each with color, hue, saturation, lightness; maps Texture, Roughness, Normal. | Missing | "Material Builder" in our 3D menu opens the Material Specification dialog (C-61); no parametric builders / see the builder row; the .sbsar import is the vendor format / same |
| C-104 | 3D Cladding definition: profiles on structural layers (create, edit, library) (manual p. 1140): Create from Adjust 3D Cladding on a layer with a regular material, from a Material Layers Definition layer whose role is changed to 3D Cladding (Edit button), or from a Wall Type Definition layer's Edit Layer > role 3D Cladding > Edit; Edit by the Adjust tool, by the layer definitions, or Open Object on a User Catalog cladding. | Missing | W-138 lists role 3D Cladding in the wall layer dialog but nothing builds it / same |
| C-105 | Adjust 3D Cladding, Interactive 3D Cladding Editor and scoping modes (manual pp. 1141-1142): Interactive 3D Cladding Editor: an axis polyline with a Move handle (offsets) and a Rotate handle (Row Angle); Tab opens exact entry; Five cladding scoping modes (Component, Object, Room, Floor, Plan). | Missing | no spec; the cladding itself is missing / same |
| C-106 | Random 3D molding variations for cladding (manual p. 1142): Generate Random 3D Moldings: number of variations, HSL color range, vary texture offset horizontally or vertically. | Missing | no spec |
| C-107 | 3D Cladding Specification dialog (General and Materials panels) (manual pp. 1143-1145): General panel, Profiles table: name, Width, Height, Repeat Distance (piece length that drives the Materials List count), Horizontal Offset, Vertical Offset, Retain Aspect Ratio; buttons Add New, Make Copy, Edit (object information and schedule), Replace, Default, Delete, Add to Library; Selected Profile Options: preview with position indicator, Vertical Position, Auto Lap, Profile Rotation, Reflect Horizontal and Vertical, Count Components in Materials List; 3D Cladding Options: Library Name, Row Overlap/Gap, Angle Rows, Offset Rows Up, Offset Even Rows In, Randomly Offset Texture U and V, Randomly Distribute 3D Moldings; Materials panel: one material per profile component, including generated random moldings; Add 3D Cladding to Library. | Missing | no spec (the dialog does not exist) / same |
| C-108 | Camera defaults dialogs: the same panels as the specification dialogs (manual p. 1148): Each defaults dialog has the same settings as its specification dialog, and several can be edited together with Shift/Ctrl. | Missing | the pages hold a few invented fields (eye height, angle, technique, depth) not the specification dialog's panels |
| C-109 | Reflections, Animate Water and Light Bloom camera options (manual pp. 1148-1205): Full Camera Defaults > Reflections is also what walkthroughs use; Reflections, Animate Water, Light Bloom; Water Animation of Water materials in Physically Based; Toggle Water Animation. | Missing | no Reflections setting / no per-camera switches / no Water class |
| C-110 | 3D View Defaults options (camera bumps, auto turn, display of active cameras and openings, surface edge lines) (manual pp. 1149-1150): 3D View Defaults dialog: Camera Bumps Off Walls (walk up stairs), Turn Automatically near walls, Legacy Compatible Texture Mapping, Auto Adjust Electrical Default Glass Properties, Always Display Active Cameras, Display Openings Independent of Walls and Roofs; Surface Edge Lines for Vector Views: Use Layer Settings, Use Object Settings. | Missing | C-68 covers the dialog name; ours (`defaults_window` in `view3d_panel.rs`) has eye height, angle of view, technique and the callout shape, size and name, none of Chief's six options; the defaults_pages brief adds a 3D View Defaults group (stored fields where the model has none) / line weights come from layer pens only for opted-in cameras (C-47, C-69) |
| C-111 | Sunlight Defaults and Adjust Sunlight dialog (Generic Sun, Sun Follows Camera, key frames) (manual pp. 1147-1222): Generic Sun Defaults dialog (initial sun intensity, color, angle for new cameras); Adjust Sunlight dialog: Use Generic Sun (intensity, color, Sun Follows Camera, tilt, direction, Reset to Defaults) or Use Sun Angle (list, New, Edit, Delete); per key frame Interpolate Sun; Reset to Defaults. | Partial | `Project.lighting` is plan-wide; no defaults dialog / 3D > Lighting dialog: azimuth, height, strength, date; per-camera override (C-63); no color, no Sun Follows Camera, no Sun Angle list, no key-frame sun |
| C-112 | A layer set per 3D view (manual pp. 1147-1157): Layer Set Defaults: which layer set each 3D view tool starts with; Layer Display Options per view, with a layer set per view type (Camera View, Section View, 3D Framing set); "CAD, Clip Lines" layer. | Missing | no layer set per 3D view (layer_sets.rs has plan views only) / the layer set is per plan view only; layers apply to the 3D model through the scene filter (`layer_sets.rs`) |
| C-113 | Rendering Technique Options and Defaults dialogs (one panel per technique) (manual pp. 1147-1239): Rendering Technique Defaults dialog: initial options of each technique; Backdrop Intensity day and night in the Physically Based and Clay options; Rendering Technique Options dialog (3D > Rendering Techniques > Technique Options, double-click the parent button, Define in the camera dialog); updates the view live; saved with a saved view; a Reference version in Change Floor/Reference; Physically Based panel: backdrop; Exposure (Automatic or Manual); Tone Mapping Operator; Global Illumination (Opaque Bounces, Transmissive/Specular Bounces, DLSS, Maximum Samples, Daytime and Nighttime Background Intensity, Use Only Backdrop for Lighting); Color Adjustment (Hue, Saturation, Brightness); Hand Drawn Lines on Top. | Partial | no technique options at all / no technique options / no such dialog |
| C-114 | Auto Elevation tools one side at a time (Front, Back, Left, Right) and repeat numbering (manual p. 1151): Auto Elevation tools: Front, Back, Left, Right and All Elevations as separate tools. | Done | C-21: Auto Elevations makes the four at once; Auto Back-Clipped and Auto Interior as well; the single-side tools are not offered ; Round 16 brief 32: Front, Back, Left and Right Elevation under 3D > Create Auto Elevations (`tools/camera.rs` `AutoSide`, `add_auto_elevations_for`, `View3dCommand::AutoSide`; s91 `auto_elevations_can_be_made_one_side_at_a_time`). Running one again updates its camera (CS16); verify in Chief. |
| C-115 | Alternate rendering technique for cameras (right-drag creation) (manual p. 1152): Right-click and drag creates the camera with the Alternate rendering technique (all but overviews). | Missing | no Alternate technique in the camera defaults |
| C-116 | Walkthrough Preview side window (frame range, quality, Create Camera View, Create Sun Angle) (manual pp. 1152-1276): A camera view from one frame of a Walkthrough Path preview; Walkthrough Path Previews: Active Walkthrough list with Define; preview pane (drag to set angle and tilt); timeline slider with current frame, Start and End Frame handles, key frame diamonds and pause bars; Preview controls: Use Recording Quality or Use Standard Quality, Create Camera View, Create Sun Angle, previous/next key frame and frame, Play from Beginning or Current Frame, Record; Preview Options: Preview Pane, Key Frame Settings, Horizontal or Vertical Layout. | Partial | no Create Camera View in the preview / C-71: Play with a scrub bar and key-frame jumps; no Start/End range, no drag-to-aim / key-frame jumps and Play only |
| C-117 | Selecting and editing model objects inside an elevation or section view (manual p. 1156): Objects that the line of sight does not cut and that are inside the back clip (a window in elevation) keep their 3D definition and can be selected, moved and stretched in the view. | Missing | elevation views are drawings (`plan_elevation::Drawing`), not editable 3D |
| C-118 | Technique letter in the camera symbol (manual p. 1157): Plan symbols: a camera symbol shows position, direction, field of view, line of sight, focal point and a letter for its technique; a section symbol shows location, clip plane, line of sight and back clip. | Partial | C-24, C-28 (symbols and clip line); the technique letter (S, V, Ph, C, G, T, W, H, D) is not drawn |
| C-119 | Plan Display panel of the camera specification (all floors, symbol size, focal point, field of view indicators) (manual p. 1158): Full Overview, Cross Section and Back Clipped symbols show on all floors; Full Camera, Floor Overview and Wall Elevation only on their floor; a per-camera Display on All Floors. | Partial | `CameraView.show_in_plan` only; no all-floors flag ; Round 16 brief 32: Plan Display tab (`dialogs/camera/panels.rs` `plan_display_tab`): Display on All Floors (`Project::cameras_on`), Display as Callout with Placement, Callout Label, Text Below Line, Callout Size, Arrow, Cross Section Line Style and Weight (`callouts_for`, `draw_one`), Camera Symbol Size and Show Field of View Indicators; Show Camera Focal Point and FOV Indicator Length are stored only (CS14). s91 `plan_display_places_labels_and_shows_the_camera_on_every_floor`. |
| C-120 | Move a camera to another floor (manual p. 1158): Move a camera to another floor with Up One Floor/Down One Floor while its view is active. | Missing | no floor change for an open camera |
| C-121 | Incremental Move Distance and Incremental Rotate Angle per camera (manual p. 1160): Incremental Move Distance (pan and forward/back dolly) and Incremental Rotate Angle (orbit, tilt, side dolly) are set per camera; Shift while stepping holds the redraw. | Done | DECISIONS 41: fixed 24 in, 15 and 5 degree steps; Chief keeps the increments in the Camera Specification (disagrees with DECISIONS 41) ; Round 16 brief 32: per-camera `CameraView::{move_step, rotate_step}` (24 in, 15 deg), edited in the Camera tab Navigation group (`dialogs/camera.rs` `navigation`), used by the 3D menu steps (`view3d_panel/nudge.rs` `Steps`, `apply_with`; scenario s91 `a_camera_keeps_its_own_incremental_steps`). Pan, dolly and mouse-orbit drags still use their own speeds; verify in Chief. ; Round 16 brief 32: Navigation group on the Positioning tab, steps per camera (CS3, `nudge.rs`; s91 test). |
| C-122 | Mouse-Orbit throw and auto-spin (manual p. 1160): Ctrl/Cmd+Alt+S spins a camera view or overview; Esc stops. | Missing | no auto-spin |
| C-123 | Wheel zoom speed rules in 3D (distance-based, Shift faster, Ctrl slower) (manual p. 1161): Wheel and trackpad pan and zoom with any tool; zoom speed follows the distance to the object under the pointer, Shift speeds up and Ctrl/Cmd slows; pan speed follows the line of sight. | Partial | wheel dolly and pan work (C-36, C-37); modifier speeds are not verified |
| C-124 | Clip Surfaces Within distance (manual p. 1161): Zooming moves the camera and does not change the clipping distance, so close objects can drop out. | Missing | no Clip Surfaces Within value |
| C-125 | Mouse-Dolly, Mouse-Tilt, 3D Center on Point and 3D Focus on Object camera tools (manual pp. 1162-1163): Mouse-Dolly Camera: drag up/down to move forward/back and left/right to turn; right drag tilts; Mouse-Tilt Camera: left drag tilts, right drag orbits; 3D Focus on Object and the Focus on Selected edit button. | Missing | C-34 (only orbit and pan modes) / same / no focus-on-object |
| C-126 | View Direction: Top, Bottom and Restore Original View (manual p. 1165): View Direction tools: Front, Back, Top, Bottom, Left Side, Right Side, Restore Original View. | Partial | C-41: eight compass snaps; Top, Bottom and Restore Original View are missing |
| C-127 | Placing and drawing objects in 3D views (manual pp. 1165-1166): Place windows, doors, cabinets, electrical objects, corner trim and most library objects by clicking in a 3D view, against walls, on floor platforms or inside the terrain perimeter; Draw custom countertops, roof planes, terrain features and roads in camera views and overviews; Build Framing and Build Roof dialogs from 3D. | Missing | placing is plan-only; the 3D view selects and drags (C-43) / same |
| C-128 | Edit handles and temporary dimensions on the handle surface in 3D views (manual p. 1166): Select with Select Objects; edit handles on a handle surface (a wall has two resize handles, a cabinet top ten); all moves in the surface plane. | Partial | C-43 selects and drags across the floor; no handle surface or resize handles in 3D |
| C-129 | Draw text, CAD and dimensions on a section or elevation view (saved with the view) (manual p. 1166): Text, CAD and Dimension tools in cross section/elevation views; annotations laid over the view and saved with it. | Done | elevations carry generated dimensions and labels (C-47) but user CAD, text and dimensions on a section are not saved with the view (CAD Detail from View is the workaround) ; Round 16 brief 32: annotations are stored with the view (`camera_view/annot.rs`, `CameraView::annotations`), drawn over section and elevation drawings (`dialogs/camera.rs` `annotation_layer`), kept by save/reopen and layout send (s91 `a_section_clipped_stepped_and_annotated_is_saved_sent_and_undone`). Tools that draw them (Text, Note, Leader, Dimension, CAD in a section view) are not hosted yet: integration queue. R17-01: tools write `ViewAnnotation` through `ToolSet::hosted` (`tools/view_annot.rs`), one undo step each, Select moves handles; vector view routes pointer events (`view3d_panel` `ViewPointer`); verify in Chief (drawing-vertex snap for Interior / Point to Point). |
| C-130 | Text, Leader Line and Note annotations in camera views (manual pp. 1167-1179): Rich Text, Text and Note tools plus dimensions in camera views and overviews; Leader Line, Rich Text, Text and most Dimension tools (not Angular, not Auto Elevation Dimensions) in camera views and overviews; notes show too; items live in that view only and need the view saved; a Stationary Walkthrough keeps them; Annotations sit on the layers of the view's Selected Defaults; they show in front unless wholly hidden; not in Glass House; line styles without text; not in Plot Lines layouts. | Partial | DIM-62 (dimension lines in camera views) / DIM-62 for dimensions; the rest is new / same R17-01: Text, Rich Text, Note, Leader Line, Dimension and CAD line / polyline / box work in section and elevation views; overviews and camera (perspective) views still do not host them. |
| C-131 | Cross Section Lines layer and Point Markers (manual p. 1167): Cross Section Lines: CAD lines Chief draws where the section plane cuts objects, on the locked "Cross Section Lines" layer, replaced on redraw; dimensions to them lock to automatic Point Markers. | Partial | the cut is drawn as poché regions in the elevation drawing (C-18), not as editable lines ; Round 16 brief 32: Cross Section Lines from the cut regions (`plan_elevation::cross_section_lines`), drawn in the view while the locked "Cross Section Lines" layer is on and not sent to layout; a dimension that meets one gets a Point Marker and follows the line (`attach_cut_markers`, `AnnotKind::relocate`; s91 `cut_walls_get_cross_section_lines_and_a_dimension_to_one_stays_on_it`). The dimension tools do not call `attach_cut_markers` yet (integration queue). |
| C-132 | Project Browser camera menu: Open View, Find in Plan, Edit View (manual p. 1167): Open a saved camera from the Project Browser: Open View, Find in Plan, Edit View. | Partial | Project Browser Cameras list: Restore, Rename, Delete, Send to Layout; Find in Plan and Edit View missing |
| C-133 | Set as Default for camera views (manual pp. 1168-1173): Camera symbol edit tools (Open View, Send Camera's View to Layout, Set as Default, Create Walkthrough); Scene Clipping defaults for the section tools are restricted; Set as Default (in a view) updates Wall Elevation, Back Clipped or Cross Section defaults by which boxes are checked. | Partial | Open Object and Send to Layout exist (L-5); Set as Default is missing / no Set as Default for cameras |
| C-134 | Context menu of the 3D view (manual p. 1169): Right-click in empty 3D space: in Vector Views File/Edit/Tools/Window commands, otherwise view tools and toggles. | Missing | no context menu in the 3D view |
| C-135 | Reset Saved Camera (All and Position) and the save-on-close prompt (manual p. 1170): Reset Saved Camera (All) and (Position): undo changes to a saved view; a prompt asks to restore or keep when the view closes. | Missing | no reset command or prompt |
| C-136 | Clip Sides, Clip Elevation and Clip Lines (manual pp. 1171-1172): Clip Lines on the "CAD, Clip Lines" layer with handles for side and elevation clipping; Side clipping: Clip Sides with a Clip Width, handle or Clip Lines to change it; default is the full width; 3D > Camera View Options > Clip Sides toggles; Top and bottom clipping: Clip Elevation with Bottom and Top Clip Elevation, or Clip Lines. | Partial | no clip lines / the cut line's length always limits the width (`SectionCut.half_width`); there is no full-width default or toggle / no vertical clip ; Round 16 brief 32: Clip Sides, Clip Elevation Bottom/Top in the Scene Clipping group (`dialogs/camera.rs` `scene_clipping`, `apply_clip`; `camera_view/clip.rs` `ClipVolume`; `plan-elevation` `clip_y`; s91 test). Clip Lines handles on the plan and the Clip Lines layer are not drawn yet. ; Round 16 brief 32: Clip Sides, Clip Elevation (`dialogs/camera.rs` `scene_clipping`, `apply_clip`; `plan-elevation` `clip_y`; s91 test); Clip Lines are drawn in the plan for a selected clipped section and in the view while the "CAD, Clip Lines" layer is on (`draw_clip_lines`, `view_layers`). Dragging the Clip Lines in the view is not built; the dialog edits the values. |
| C-137 | Stepped cutting planes (Add Break on a section line) (manual p. 1171): Stepped cutting planes: Add Break on the section line, drag the handle beside the break perpendicular for a step and along it to move the step. | Done | the section line is straight (`SectionLine { a, b }`) ; Round 16 brief 32: stepped planes (`StepPlane`, Add Break, per-piece offsets, Make Parallel/Perpendicular in the dialog), drawn piece by piece (`render_stepped`, s91 test). Dragging the break handles on the plan is not built. ; Round 16 brief 32: stepped planes (`StepPlane`, Add Break `add_section_break`, per-piece offsets, Make Parallel/Perpendicular in the dialog), drawn piece by piece (`render_stepped`, CS4); diamond Break handles and Step handles in the plan (`CamHandle::Break/Step`, s91 `a_stepped_plane_has_handles_in_the_plan_that_move_and_step_it`). The Add Break edit button calls `add_section_break` (integration queue). |
| C-138 | Clip to Room options for elevations (manual p. 1173): Clip to Room (default for Wall Elevation; Ignore Railings and Invisible Walls; Ignore Walls Above). | Partial | Wall Elevations and Auto Interior Elevations back-clip to the room (C-20, C-21); no check box and no ignore options ; Round 16 brief 32: Clip to Room with Ignore Railings and Ignore Walls Above (`Project::room_clip`, dialog check boxes); default on for new Wall Elevations (`SectionClip::for_kind`). |
| C-139 | Selected Defaults panel of camera specifications (manual p. 1173): Annotations use the view's Active Defaults (Selected Defaults panel). | Done | no Selected Defaults in the camera dialog; Tools > Active View > Active Defaults is the plan's ; Round 16 brief 32: Selected Defaults tab = the Active Defaults panel for the view (`dialogs/camera/panels.rs` `selected_defaults_tab`, `CameraView::selected`; s91/panels tests). Annotation tools read `ViewDefaults::annotation_layer` (integration queue). |
| C-140 | Depth Cue for cross section and elevation views (manual p. 1175): Depth Cue: Use Depth Cue, Keep Start/End in Sync, Start and End distances, Fog Opacity, Fog Color; does not affect text, dimensions or CAD. | Done | the Backdrop tab has Fog for rendered views (C15-4) but no section/elevation depth cue ; Round 16 brief 32: Depth Cue group in the Camera tab (Use Depth Cue, Keep Start/End in Sync, Start, End, Fog Opacity, Fog Color), `DepthCue` in `camera_view/spec.rs`; fog shown as line weight (CS7, `plan-elevation` `hlr::cue_weight`; s91 `depth_cue_fades_the_far_lines_and_below_grade_restyles_the_low_ones`). Fog Color is stored only. |
| C-141 | Cross Section Slider: several planes in camera views, saved with the camera (manual p. 1176): Cross Section Slider dialog: several cutting planes, each with a check box, a position slider and a typed position measured from the first edge cut; keep working while open; saved with the camera; not carried into CAD Detail or plot-line sheets. | Done | C-23: one plane along the elevation direction, a slider in the Vector View; no multi-plane dialog, no perspective-camera use, not saved with the camera ; Round 16 brief 32: Cross Section Slider dialog with six cutting planes, saved with the camera (CS9; `dialogs/camera/slider.rs`, `CrossSectionSlider`, `view_settings::SliderClip`; view_settings test `slider_planes_cut_the_scene_from_the_edge_of_the_model`). |
| C-142 | Draw on Bounding Box and Draw on Surface for dimensions and text in 3D views (manual p. 1178): Draw Mode buttons: Draw on Bounding Box, Draw on Surface, Set Offset (Offset from Draw Surface, global, also in Preferences > CAD); a circle with cross hairs marks the target; Dimension Selected Edge edit tool in 3D. | Missing | no drawing surfaces in 3D / DIM-62 |
| C-143 | Object labels in camera views (manual p. 1179): Object labels display in camera views when their layers are on. | Missing | no labels in the live 3D view |
| C-144 | Move and Rotate (Local and Global) edit tools for 3D annotations (manual p. 1179): Move Object (Local/Global), Move Object's Label (Local/Global), Rotate Object (Local/Global), Rotate Label (Local/Global) edit tools on a plane or axis. | Missing | no 3D annotation |
| C-145 | Picture export: WebP and HDR for ray-traced views (manual p. 1185): Export a 3D view as .bmp, .jpg, .png, .tif or .webp; Physically Based and Clay as .hdr; File > Export > Picture. | Partial | L-49, DECISIONS 403 (PNG, JPEG, BMP, TIFF); WebP and HDR are missing |
| C-146 | Show Color and Show Watermark camera options (manual p. 1187): General: Name (unique), Saved, Show Color, Show Watermark. | Done | Name works; Saved is implicit; Show Color and Show Watermark are missing ; Round 16 brief 32: Show Color (grey scene, `view_settings::gray_scene`) and Show Watermark (`extras::paint_watermark`) on the Camera tab (CS15). |
| C-147 | Ambient Occlusion amount per camera (manual p. 1188): Ambient Occlusion amount (slider). | Done | SSAO fixed by view quality (`quality.rs`); no per-camera amount ; Round 16 brief 32: Ambient Occlusion slider scales the look's occlusion (`Viewport3d::ao_scale`, `view_settings::ao_scale`; CS15). |
| C-148 | Super Resolution and Sharpening (Upscaling) camera options (manual pp. 1188-1202): Upscaling: Sharpening and Super Resolution factors; Super Resolution: render below native resolution and upscale (not in sections). | Partial | none in the dialog (the Final View sharpens internally) / see the Upscaling row ; Round 16 brief 32: Sharpening and Super Resolution are stored and edited in the Upscaling group; no renderer reads them yet. |
| C-149 | Depth of Field in the camera specification (manual p. 1188): Depth of Field: enable, F-Stop, Focus Distance. | Done | aperture and focus in the Ray Trace window (C-51); not a camera setting ; Round 16 brief 32: Depth of Field group (Enabled, F-Stop, Focus Distance) sets the Ray Trace lens when the camera is shown (`view_settings::dof_lens`; CS15). |
| C-150 | Use Sunlight and Maximum Lights in the camera Lighting group (manual p. 1189): Lighting: Use Sunlight, Adjust Sunlight; Automatic with a Maximum number of lights, or a Light Set with Adjust Lights. | Done | C-65 light sets and the lighting override on the Rendering tab; Use Sunlight and Maximum Lights are missing ; Round 16 brief 32: Use Sunlight (`view_settings::rig`), Automatic with a Maximum Number or a Light Set (CS15). Adjust Sunlight is the Sun Angle window. |
| C-151 | Hide Camera-Facing Exterior Walls (manual pp. 1189-1205): Options: Poché, Field of View, Clip Surfaces Within, Show Lower Floors, Hide Camera-Facing Exterior Walls, Extend Terrain to Horizon; Hide Camera-Facing Exterior Walls (also hides attic walls, cabinets and symbols on those walls, and their doors and windows); also a Walkthrough Path option. | Done | Field of view (C-7) and floors displayed (C15-7) work; the rest are missing / no such view option ; Round 16 brief 32: Hide Camera-Facing Exterior Walls, Clip Surfaces Within (`Camera::near`) and Field of View on the Options group (CS13, CS15; `camera_view/facing.rs`, view_settings test `walls_facing_an_outside_camera_leave_the_scene`). Cabinets on the hidden walls stay. |
| C-152 | Poché switch on cross section cameras (manual p. 1189): Poché check box on the camera. | Done | sections always draw poché ; Round 16 brief 32: Poche check box in Scene Clipping drops the cut fill (`render_elevation_with`; s91 test). |
| C-153 | Below Grade panel (line overrides under the terrain or a height) (manual p. 1190): Below Grade panel: Override Color, Style, Weight; applies below the terrain perimeter or an absolute height; lists the object types. | Done | no below-grade line overrides ; Round 16 brief 32: Below Grade tab (Override Color, Style, Weight; Terrain Perimeter or Absolute Height; affected types listed), applied by `plan_elevation::override_below` (CS10; s91 test). |
| C-154 | Generated Sky backdrop (sun, moon, stars) (manual p. 1193): Use Generated Sky with Starlight Intensity, Star Density, Moon Luminance, Moon Intensity, Moon Tilt, Moon Direction, Moon and Sun Angular Radius, Reset. | Missing | the sky is a gradient (`quality::sky_colors`) with no sun disc, moon or stars |
| C-155 | Spherical panoramic backdrops and Rotate Spherical Backdrop (manual pp. 1193-1204): Spherical Panoramic Backdrop: Horizontal and Vertical Tile, Horizontal Span, Horizontal Offset, Vertical Max and Min, Eye Level; Backdrop contributes to global illumination in Physically Based and Clay; Generated Sky. | Missing | a backdrop is a flat picture / the path tracer has its own sky (Preetham); a backdrop picture does not light the scene |
| C-156 | Layer and Drawing Group panel for cameras and sections (manual p. 1194): Layer panel: the layer of the camera symbol. | Partial | cameras sit on one layer ; Round 16 brief 32: Layer tab (layer of the camera symbol, used to show or hide it; Drawing Group number stored only; CS14). |
| C-157 | Framing Back Clip for section views (manual p. 1196): Scene Clipping: Poché; Framing Back Clip with Back Clip Framing After; Back Clip with Back Clip After; Clip Sides with Clip Width; Clip Elevation with Bottom and Top; Clip to Room with Ignore Railings and Invisible Walls and Ignore Walls Above. | Partial | Back Clip After and Section Length only; the other six groups are missing (rows above) ; Round 16 brief 32: Poche, Framing Back Clip with After, Clip Sides, Clip Elevation and Clip to Room are in the dialog and stored; Framing Back Clip is stored but not yet applied to framing in the drawing. ; Round 16 brief 32: Scene Clipping group complete in the dialog. Framing Back Clip is stored and shown, and applies to framing when the framing view is drawn from a section (integration queue). |
| C-158 | Callout placement, section line style and weight (Plan Display of sections) (manual pp. 1198-1200): Plan Display: Display on All Floors; Display as Callout needed for the symbol to print or export; Placement (Center, Left Side, Right Side, Both Sides, Custom with offset); Callout Label; Text Below Line (Automatic = layout page label); Callout Size; Arrow; Cross Section Line Style and Weight (by layer); Arrow panel: an arrow on the clip plane line when callout placement is Left or Right. | Done | callout show and number (C-24); placement, line style and weight are missing / the Arrow panel is not offered on cameras ; Round 16 brief 32: Plan Display for sections (CS14; s91 test). Section line style and weight draw in the plan; a printed callout still needs Display as Callout (`callout.show`). |
| C-159 | Denoise View toggle and DLSS-style real-time denoise (manual p. 1201): Denoise View command swaps the live ray trace for a denoised offline copy and back. | Missing | the denoiser runs on save (`plan-render/src/denoise.rs`); no view toggle |
| C-160 | Tone mapping operator and exposure (Hable, ACES) (manual p. 1202): Tone mapping operators in Physically Based and Clay: Hable or ACES. | Partial | `plan_render::ToneMap` is ACES, Reinhard or Linear; no Hable and no per-view choice in the UI |
| C-161 | Sun Angle objects (date, time, place, shadows) (manual pp. 1203-1220): Sun shadow polylines can show in plan view for a date, time and place; Sun Angle objects: location (latitude, longitude, time zone) in General Plan Defaults; a date and time per Sun Angle; daylight saving; USNO formulas; Sun Angles show date and time in plan and make Sun Shadow polylines (hatched, shaped by terrain) on the "Sun Angles & Shadows" layer; Make Shadow and Delete Shadow edit tools; Sun Angle Specification: Earth Data (date with calendar, time, daylight saving, time zone), Plan View Display (symbol length, Show Date, Make/Delete Shadow, Always Update), Location, Solar Angles (altitude, direction). | Missing | no shadow polylines in plan (see Sun Angle objects) / `SunDate { month, day, hours, latitude }` for one plan sun; no longitude, time zone or daylight saving / same |
| C-162 | Opaque Window/Door Glass option (manual p. 1204): Opaque Window/Door Glass option per technique (color from the material or custom), overridable per door. | Missing | windows and doors keep their glass; no technique option and no per-door override |
| C-163 | Hand Drawn Lines on Top overlay (manual pp. 1205-1238): Hand Drawn Lines on Top over most techniques; Toggle Hand Drawn Lines on Top; Hand Drawn Lines panel: line color, Thickness (scaled pixels), Extend Amount, Squiggle Amplitude and Frequency. | Missing | the Line Drawing technique is a separate look; no overlay / no hand-drawn technique |
| C-164 | Spot lights and light position indicators (manual p. 1206): Added Lights (Add Lights tool): Point or Spot; layers "Light Sources" and its labels; need a room; 2D symbols that do not exist in 3D, optional red cross hairs and a blue spot arrow. | Partial | C-64, C-66: point lights only, a hidden-layer record, no position cross hairs |
| C-165 | Toggle Sunlight and overcast lighting (Use Only Backdrop for Lighting) (manual p. 1207): Sunlight alone lights exterior daytime views and shines through windows with shadows on; 3D > Lighting > Toggle Sunlight; Use Only Backdrop for Lighting for overcast scenes. | Partial | the sun is on by default and can be overridden per camera (C-63); no Toggle Sunlight command, no overcast option |
| C-166 | Move Sun and Move Moon tools with the direction indicator (manual p. 1207): Move Sun and Move Moon tools (drag, or Ctrl/Cmd+click to place) with the Sun/Moon Direction Indicator in the corner. | Missing | no tool and no moon |
| C-167 | Interior, daytime and nighttime ambient light levels (manual p. 1207): Three ambient lights: Interior, Daytime and Nighttime; Ambient Occlusion amount per camera. | Partial | one Ambient level (`Lighting.ambient`, camera override); not three |
| C-168 | Light brightness per light set (Adjust Brightness) (manual p. 1209): Light Intensity (constant per light) and Brightness (percent per light set); Adjust Brightness edit tool. | Missing | the light's Power is the only level; no brightness per set |
| C-169 | Adjust Lights dialog columns (fixtures, In Use, Brightness, Show Position) (manual pp. 1213-1214): Light Source Properties table with customisable columns (On, Use Area Lights, Count, Room, Floor, Type, Intensity, Color, In Use, Show Position, Brightness), expandable fixtures with their punctual and area lights; Adjust Light, Adjust Material and Adjust Brightness buttons. | Partial | a list with on, color and shadows per light; the fixture tree and the columns are missing / same |
| C-170 | Create/Edit Light Set from Selection (manual p. 1214): Create/Edit Light Set from Selection edit tool (new set with only the selected lights on, or add them to a set). | Missing | sets are made inside Adjust Lights |
| C-171 | Adjust Area Light Material dialog (manual p. 1214): Adjust Area Light Material dialog: create a copy, edit the source or pick another material; Apply Emissive and Color from Light(s); Emissive preset list; Material Color; texture; Define Material. | Missing | see the emissive row |
| C-172 | Light Specification dialog (Location, Light Data, Layer, Label) (manual p. 1216): Light Specification dialog for an Added Light: Location (Light Display on all floors or one floor, Absolute Elevation, Display Size), Light Data, Layer, Label. | Missing | Added Lights edit in Adjust Lights; no specification dialog |
| C-173 | North Pointer drives the sun and bearings (manual p. 1217): North Pointer tool defines true north for sunlight, conditioned area totals and bearings; several pointers stay in step; default is up. | Partial | Terrain > North Pointer places one (site symbols); the sun, bearings and the REScheck orientation do not read it |
| C-174 | Next and Previous Rendering Technique commands (manual p. 1228): Next/Previous Rendering Technique tools. | Missing | not on the toolbar or menu |
| C-175 | Backdrops in the library: import, folder import, HDR and the Backdrop Specification dialog (manual pp. 1241-1243): Pick a backdrop by clicking in the view with a library backdrop selected, or with Select Backdrop; backdrops are view-specific; .hdr files for high dynamic range; Add backdrops: File > Import > Backdrop, User Catalog New > Backdrop, paste, screen capture, Create Backdrop Library from a folder; Backdrop Specification dialog: name, location (copied into the My Backdrops folder), spherical options (tile, span, offset, vertical max and min, eye level), copyright, preview; .hdr imports as spherical. | Partial | Backdrop tab picks by name from Chief's Backdrops folder; click-to-apply from the Library and .hdr are missing / the backdrop list is read from Chief's folder; nothing imports / same |
| C-176 | Orbital and Stationary Walkthroughs (sun study) (manual pp. 1265-1280): Create Orbital Walkthrough Path edit tool: a circle around a camera's or overview's focal point, radius equal to the line of sight, every frame facing the focal point; Create Stationary Walkthrough: a time-lapse sun study from one perspective view with the date and time overlaid; Sun Angle Options: Time Lapse or Seasonal interpolation, Start and End Sun Angles with New, Edit, Delete, Elapsed Clock Time, Sun Time Overlay; Time Overlay Settings: Color, Print Size, Font, Include Date, Layout, Angle, Transparency, Margins. | Missing | no orbital path / no sun-study video / see Stationary Walkthrough |
| C-177 | Frame-by-frame walkthrough recording (Record, Pause, Save Frame, Stop) (manual p. 1265): Record Walkthrough frame by frame: Record, Pause Recording, Save Frame and Stop Recording while moving the camera; each redraw is a frame. | Missing | recording follows a path only |
| C-178 | Walkthrough path labels, Reverse Direction and Key Frame tools (manual pp. 1267-1268): Paths show in plan only, on the "Walkthrough Paths" layer, in front, print and export; an automatic label (floor and number) and an optional custom label on "Polylines, Labels"; Reverse Direction edit button for a path. | Partial | paths draw in plan (C-71); the label rules are missing / CAD Reverse Direction exists (`tools/cad/edit.rs`); not offered for walkthrough paths |
| C-179 | Key Frame fields: time, speed after, sunlight, pause, floor (manual pp. 1268-1275): Key Frames change camera direction, tilt, height, speed, sunlight and the floor; Add Key Frame edit tool with Sticky Mode; the frame takes values from its neighbours; Delete Key Frame (at least two stay); Key Frames list in the preview, with Floor, Time, Speed After, angles, height, sunlight source (Sun From Key Frame, Interpolating), Adjust Sunlight, Pause. | Partial | path nodes are key frames with height, look direction, tilt and hold (C-71, DECISIONS 165); time, speed after, sun and floor are missing / see the Key Frame row |
| C-180 | Walkthrough path across floors (stairs and ramps) (manual p. 1268): Paths follow stairs and ramps and continue on the next floor with a dashed extension. | Missing | paths are on one floor; node floor is not stored |
| C-181 | Walkthrough Path Specification panels (Key Frames list, Resolution, Duration, Camera, Backdrop, Line Style, Label) (manual pp. 1269-1271): Walkthrough Path Specification dialog (also the Defaults dialog): General (Key Frames list with Floor, Time, Speed After, Camera Angle, Tilt, Height, Sunlight, Pause), Camera, Backdrop, Polyline, Selected Line/Arc, Line Style, Label; Key Frame Symbol Size; Camera Angles absolute or relative to the path; Camera Heights absolute or relative to floor/terrain. | Partial | the Camera dialog's Walkthrough section: speed, node table (x, y, height, look, tilt, hold), fps (`walkthrough_page`) / path heights are above the floor; angle is "along the path" or fixed |
