# Parity spec: 3D Views and Cameras (Chief Architect X18)

> Status (2026-10-08): 71 ids: 31 Works, 25 Partial, 15 Missing, 0 Differs-by-design. Per-id evidence and gaps are in [../parity-status.md](../parity-status.md); the spec text below is the original and its code snapshots are out of date.

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
Chief for whether a camera symbol is drawn).

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

Plan Studio has camera objects (Full Camera, the cross-section cameras) placed from the plan with a Camera Specification dialog, and an orbitable 3D panel (`shell/view3d_panel.rs`) over `plan-view3d`: Perspective Full and Floor Overview, Doll House, Full Camera, four orthographic elevations, plan overhead, back-clipped sections and a section slider. Navigation is mouse-orbit, pan and dolly; 3D View Defaults (`Cmd+1`) sets new-camera defaults. Nine rendering techniques, the Sun Angle window and a CPU ray tracer with PNG output (`plan-render`) are in; glTF export too. The scene shows walls with openings, doors, windows, floors, ceilings and roof planes; stairs, cabinets, library 3D models, electrical devices, terrain and framing are not drawn in 3D yet. Missing in the viewport: textures and moving objects with the mouse in 3D. Cross sections, Wall Elevation and Auto Elevation cameras draw at the exact angle of their camera line (`plan_elevation::FreeView`), with plan callouts, Auto Interior Elevations, automatic elevation dimensions, material labels, per-layer line weights and DXF export in the Vector View. Elevations can be exported as DXF (File > Export > Elevation DXF). Clicking an object in the 3D view selects it in the plan's own selection (C-43: Shift adds, a double-click opens its specification, Delete deletes it) and tints it in the view. (Refreshed 2026-10-08. The behavior statements in this document are Chief's and unchanged; the gap table below is the original audit and is partly out of date.)

## 12. Gap table

| Chief behavior | Plan Studio today | Severity | Suggested implementation |
|---|---|---|---|
| C-4, C-5 Full Camera by click (position) + drag (direction) in plan, with cone | `FullCamera` mode exists in widget but no plan tool or camera object | Critical | Add `Camera` object type in plan-core (position, height, target, fov, floor) and a plan tool; open 3D tab from it |
| C-10..C-14 Full/Floor/Framing overview, Doll House | Orbit and DollHouse modes only; no per-floor display filter | High | Add `FloorsDisplayed` filter to scene build; add Floor Overview and Framing Overview variants |
| C-15 Orthographic overviews and 4 isometrics | Only 4 elevations + plan overhead | Med | Add orthographic Full/Floor Overview and SW/SE/NE/NW iso presets (yaw 45/135/225/315, pitch 35.26) |
| C-17..C-19 Cross Section / Back-Clipped cameras, cut line + direction arrow | Absent; plan-elevation is an empty crate | Critical | Section camera object (two points, depth, back-clip); mesh clipping plane in `plan-3d`; poche from cut faces |
| C-20, C-21 Wall Elevation camera, Create Auto Elevations | Absent | Critical | Generate 4 exterior + per-room interior elevation cameras from model extents/walls; feed plan-elevation |
| C-22 Hidden-line vector output from elevations | `edges.rs` extracts feature edges only on CPU, no hidden-line removal | High | Implement hidden-line via depth-tested edges or analytic HLR in plan-elevation; vector export to Layout |
| C-24..C-29 Camera symbol, handles (move/aim/FOV/clip) | None | Critical | Camera drawing in `plan-app` with hit-testing handles; undoable edits |
| C-30 Camera Specification dialog | None | High | Dialog on shared frame: name, position, height, target, FOV, floor |
| C-31, C-32 Camera View Options per view; defaults | Viewport fields (`show_edges`, `lighting`) only | High | `ViewOptions` struct saved per view/camera; separate defaults struct |
| C-33 Locked camera | None | Low | Boolean on camera; block handle edits |
| C-34 Mouse-Orbit vs Move Camera modes | Mode implied by `CameraMode`; no explicit tool choice | Med | Add `NavMode` toolbar state (Orbit, Pan, Walk, Look) following the 3D menu |
| C-35, C-36 Orbit/pan with clamped pitch, pan with H tool | Present | Done | Keep; add pitch-limit parity test |
| C-37 Zoom to rectangle, Fill Window Selected | `fit_to_bounds` only | Med | Add rubber-band zoom and Fill Window Selected using selected mesh bounds |
| C-38 Move Camera with Keyboard: arrows, PgUp/PgDn | WASD/arrows walk in Full Camera only | Med | Map arrows per Chief in all perspective views, add PgUp/PgDn vertical |
| C-39 3D Center / orbit centre pick (Snap to object) | Orbit centre fixed to bounds centre | High | Raycast pick on click with Alt; set `target`; draw centre marker |
| C-40, C-41 Tilt Camera, View Direction snaps | Pitch via drag only | Low | Add tilt tool and compass snap actions |
| C-42 Undo Zoom in 3D | 2D plan has `zoom_history`; 3D none | Low | Camera history stack |
| C-43 Surface picking and selection in 3D | `Mesh.object_id` exists, no picking | High | Ray-triangle pick over meshes returning `object_id`; selection sync with plan |
| C-45 Technique list (Standard, Vector, Technical Illustration, Watercolor, Line Drawing, Glass House, Physically Based, Clay, Duotone) | Standard only (+edge overlay) | High | Enum `RenderTechnique`; implement Standard, Vector/Line, Glass House, Clay first (shader switches); Watercolor/Duotone as post filters; PBR via offline path tracer later |
| C-47 Vector View with line weights, shadows, fill | Edge overlay only | High | Vector pass producing 2D polylines per layer weight; share with plan-docs for PDF |
| C-51 Physically Based ray-traced render | None | Low | Out of scope until Phase 5; consider `bvh` + CPU path tracer |
| C-54 Rebuild 3D | Scene rebuilt wholesale on change; no command | Low | Menu command that calls `build_scene` and drops caches |
| C-55 Delete Surface | None | Med | Per-object surface-hide set in model; skip faces in `plan-3d` |
| C-56..C-60 Material Painter, eyedroppers, Adjust Material Definition, Interactive Editor | Fixed 8 materials | Critical | Introduce `MaterialDef` table (colour, texture, shine, alpha), per-surface overrides on objects, painter tools picking via C-43 |
| C-61 Material Builder, textures | None; UVs in feet already provided | High | Texture loading (image crate) + material library JSON |
| C-62..C-67 Sun Angle, Add Lights, Light Sets, shadows | Fixed `Lighting` (key_dir, ambient) | High | Sun model (lat/long/date/time to direction); light objects; shadow map pass |
| C-68 3D View Defaults dialog | None | Med | Defaults dialog on shared frame, persisted in project |
| C-70 Sky, ground, backdrop | Single background colour | Low | Gradient sky + ground plane; terrain mesh later |
| C-71 Walkthrough path and preview | None | Low | Path object + playback of camera along spline |
| C-12 Framing Overview | plan-framing empty | Med | Depends on framing crate (see cabinets-stairs-framing-terrain-library.md CB-36) |
| 3D scene contents: roof, stairs, cabinets, terrain, library models | Only walls, openings, slabs | Critical | Mesh generators per crate feeding `Scene`; the 3D view is only as faithful as these |
| 3D view reachable from editor, tabs per view, Window menu | `plan-app` has no 3D dependency | Critical | Wire `Viewport3d` into `plan-app` as a view tab; move to wgpu later if needed |
