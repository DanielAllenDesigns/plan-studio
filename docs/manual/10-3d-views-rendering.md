# Chapter 10: 3D Views, Cameras and Rendering

Every 3D view is generated from the plan. Edit a wall and the 3D model follows. This
chapter covers the 3D views, the camera objects, the rendering techniques, the Sun
Angle, the ray tracer, the vector elevations you can open as drawings, Auto Elevations
and Wall Elevation cameras, walkthroughs, lights and glTF export.

## 10.1 What is in the 3D model

`plan_3d::build_scene` turns the plan into triangle meshes, and the editor adds a few more families on top:

- Walls, with openings cut through them and reveals at the jambs. The exterior surface takes its
  material from the wall type's outer layer (stucco, siding, brick, stone, concrete ...). Walls are built
  by class (chapter 2):
  - a **foundation** wall is concrete and reaches its Foundation Height below the floor;
  - a **pony** wall is two walls stacked at the split height, each with its own type, thickness and look;
  - a **glass** wall is a 1/2" glass pane in a 2" frame, and a **glass pony** wall a solid lower part under glass;
  - a **half-wall** is cut down to its height; a **room divider** has no mesh;
  - **railing** and **deck railing** walls are posts (at most 8' apart and one at each end), top and bottom rails and
    3/4" balusters; a **deck edge** is a rim board; **fencing** is posts every 8' at most with pickets, boards or
    rails by Fence Style;
  - a **curved** wall is built as a run of straight facets (one per 7.5 degrees) of its class, and **doors and windows
    are not cut through it**.
- Door leaves and window frames, sashes and glass for every opening style the model knows (hinged, sliding,
  pocket, bifold, garage, barn, shower, fixed, casement, bay, box and bow), with lite grids.
- Optional detail through `SceneOptions`: doors drawn open at an angle, interior and exterior casing, window
  sills and exterior thresholds. The editor's 3D view calls the builder with the defaults (doors closed, no
  casing), and the editor only places hinged doors and plain windows, so today you see
  closed hinged doors and plain windows (planned: View options for open doors and casing).
- Floor and ceiling platforms for every detected room, with any **Hole in Floor Platform** and **Hole in Ceiling Platform**
  cut out (chapter 16).
- Slabs, slab holes, footings, square pads and round piers (chapter 16).
- Wall **bottom heights**: a wall that starts above the floor (chapter 2.9) is built from its bottom to its top, for standard walls and every straight wall class;
  the openings keep their sill heights measured from the floor, so only the part of an opening inside the wall is cut.

The editor appends:

- **Roofs** (chapter 8): each plane as a slab with its holes cut, skylights (curb, frame, glass), vaulted ceiling planes
  and dormers.
- **Manual framing** (chapter 11.11): the members placed by the framing tools and those Build Framing makes from the layout
  lines (directed joists, bearing beams, laid-out trusses). The members Build Framing makes from the walls, floors and roof
  are drawn in plan only, not in 3D.
- **Placed library symbols**: a box of the symbol's width, depth and height for a built-in symbol, and the decoded meshes
  for a Chief catalog object (a box when the geometry is partial, nothing when the catalog is unavailable; chapter 6.6).
- **Exterior details** (chapter 17): corner boards, quoins, moldings, floor and wall material regions, polygon decks and the 3D solids.

Stairs, cabinets, electrical devices and terrain have 3D builders in their own crates but the 3D view does not draw
them at the Round 7 commit (planned). The scenario tests record this as QA-05 (cabinets) and QA-06 (stairs) in `docs/qa-findings.md`;
the scene's change hash does not see them either, so the view would not even rebuild. Both are being fixed in Round 8. Coordinates: X is plan x, Y is up, Z is
negative plan y, all in inches. The scene is rebuilt automatically when the plan changes (it
watches a hash of the floors, walls, openings, placed symbols, roofs, foundation objects and details), and **3D > Rebuild 3D** forces it.

## 10.2 Tools and commands

### Row 1 camera buttons

| Button | Hotkey | What it does |
|---|---|---|
| **3D View** flyout: Perspective Full Overview | `Shift+K` | Opens an orbitable 3D view framing the whole plan. No camera object is created. |
| Perspective Floor Overview | | The same, but floors above the active floor are left out. |
| Doll House View | | An overview with ceilings and roof hidden so you can see into the rooms. |
| Orthographic Full Overview | | A plan overhead (top-down orthographic) view. |
| **Full Camera** flyout: Full Camera | `Shift+J` | Press at the eye position in the plan, drag to set the viewing direction, release. A camera object is created and the 3D view opens from it (eye 66" high, 60 degree angle of view). |
| Cross Section/Elevation Camera | | Press and drag a cut line in the plan; release creates a section camera. The view looks to the **left** of the drag direction, so dragging a line west to east looks north. |
| Back-Clipped Cross Section | | The same with a rear clip plane that limits what shows behind the cut. |
| Wall Elevation Camera | | Click a wall: an elevation camera of that wall face (10.11). |
| Auto Elevations, Auto Back-Clipped Elevations | | One click makes the four exterior elevation cameras (10.11). |
| **Mouse-Orbit Camera** | | Opens (or switches to) the orbit view. |
| **Cross Section Slider** | | Toggles a slider that moves a section plane through the model. |
| **Rendering Techniques** flyout | | Choose a technique (10.4). |
| **Create Walkthrough Path** flyout: Create Walkthrough Path, Play Walkthrough, Record Walkthrough | | Draw a path in the plan, then play it in the 3D view or record it as frames (10.12). |
| **Add Lights** flyout: Add Lights, Adjust Lights | `Ctrl+Alt+Cmd+L` (Adjust Lights; `Ctrl+Alt+L` off macOS) | Click to place lights; Adjust Lights opens the lights dialog (10.13). |
| **Sun Angle** (toggle) | | Opens the Sun Angle window (10.5). |
| Material Painter, Material Eyedropper, Object Eyedropper, Delete Surface, Adjust Material Definition, Interactive Material Editor | | (planned; engine in `plan-materials`) |

### The 3D menu

Create Orthographic View (Front, Back, Left, Right Elevation; Plan Overhead; Cross
Section/Elevation; Back-Clipped Cross Section), Create Perspective View (Full Camera, Perspective
Full Overview `Shift+K`, Perspective Floor Overview, Doll House View, Ray Trace...), **Create Auto
Elevations** (Auto Elevations, Auto Back-Clipped Elevations, Wall Elevation Camera), **Walkthroughs**
(Create Walkthrough Path, Play Walkthrough, Record Walkthrough...), Rendering Techniques, Rebuild 3D,
Export > glTF..., and **3D View Defaults... `Cmd+1`**. The rest of Chief's 3D menu (camera movement,
isometric views, materials, the Lighting submenu, camera view options) is listed and dimmed (planned);
the lights live on the row 1 Add Lights flyout.

### Navigating a 3D view

| Gesture | Perspective and orbit views | Full Camera |
|---|---|---|
| Left-drag | Orbits (yaw and pitch) | Turns the view |
| Right- or middle-drag, `Shift`+drag | Pans | Pans |
| Scroll wheel | Dolly in and out | Dolly |
| `W` `A` `S` `D` or arrow keys | | Walk forward, back, left, right |
| `Page Up` / `Page Down` | | Raise or lower the eye |
| `Tab` | Cycles through camera objects | Cycles |
| `Esc` | Returns to the plan | Returns to the plan |

Elevation and overhead views pan and zoom only. Opaque surfaces are drawn first, then translucent
ones (window glass) back to front; an optional dark edge overlay outlines the model. A 3D view
needs OpenGL 3.1 (GLSL 1.40); if setup fails the viewport stays blank and the reason is available
in the error text.

## 10.3 Camera objects

A Full Camera or a section camera is a plan object on the `Cameras` layer, drawn as a camera
symbol with a viewing cone. Select it with the Camera tool (or Select Objects).

- **Move** handle: drags the camera in plan; the 3D view updates live.
- **Aim** handle: rotates the view direction; `Shift` snaps to 15 degrees.
- **Clip** handle: sets the back clip distance.
- Section cameras also have the **two line-end** handles.
- `Tab` cycles cameras, `Delete` removes one, double-click opens the Camera Specification.
- The Project Browser lists the plan's cameras (Plan > Cameras). Click one to select it, switch to its floor and pan the plan to it.

### Dialog: Camera Specification

| Tab | Fields |
|---|---|
| General | Name; Camera Type (read-only: Full Camera, Perspective Overview, Doll House View, Cross Section, Wall Elevation, Elevation, Walkthrough, Orthographic); Floor; Camera Position (or Cut Line and Center for a section); View Direction; Section Length (sections) or Height Above Floor and Angle of View (perspective). A walkthrough instead shows **Walking speed** and a **Path nodes** table (10.12) |
| Options | **Clipping**: Back Clip Distance, Far Clip Distance (Limit view distance). **Display**: Show camera in plan, Locked camera (both disabled) |
| Rendering | Technique (kept per camera for the session), Cast shadows (disabled). Elevation and cross-section cameras (and wall elevations) add an **Elevation rendering** section (10.7). |

The angle of view is limited to 5 to 170 degrees.

### Dialog: 3D View Defaults (`Cmd+1`)

Opened from 3D > 3D View Defaults... (`Cmd+1`; the menu shows whatever key the live hotkey map gives it).
Camera eye height (12" to 600"), Angle of view, and the Rendering technique used when a view opens.

## 10.4 Rendering techniques

Row 1 Rendering Techniques flyout, 3D > Rendering Techniques. Nine techniques are listed:

| Technique | In the interactive 3D view |
|---|---|
| Standard | Shaded surfaces with an edge overlay. |
| Vector View | Flat shading, white background, edge lines. On an elevation, section or wall elevation camera it shows the camera's **vector drawing** instead (10.7). |
| Technical Illustration | Flat shading with material-color fill and edge lines. On an elevation, section or wall elevation camera it shows the vector drawing with material-colored faces and heavier lines, always with shadows (10.7). |
| Watercolor | Flat shading on a warm paper background (an approximation of the bitmap technique). |
| Line Drawing | Edge lines on white. |
| Glass House | Every surface transparent glass so structure behind shows. |
| Physically Based | Looks like Standard interactively; the real result comes from the Ray Trace window (10.6). |
| Clay | Uniform gray material on a gray background. |
| Duotone | Two-tone approximation on a warm background. |

Switching technique never changes the model, only how the view is shown.

## 10.5 Sun Angle

The Sun Angle toggle opens the **Sun Angle** window. Two radio buttons choose how the sun is set:

- **Date and time**: **Date** (month and day), **Time** (0 to 24 hours) and **Latitude** (-66 to 66 degrees).
- **Angles**: type the **Azimuth** (0 to 360 degrees) and **Altitude** (-10 to 90 degrees) directly.

The window shows the resulting azimuth and altitude. The sun is the viewport's key light while it is above the horizon, the sun of the ray tracer, and, while the
toggle is on, the sun that casts the **shadows of the vector elevations** (10.7). **Use for elevation shadows** turns shadows on in every elevation and section camera and stores
this sun on them (one undo step, "Sun Angle Shadows"). Closing the window turns the toggle off.

## 10.6 Ray Trace

3D > Create Perspective View > **Ray Trace...** opens a progressive path tracer
(`plan-render`) rendering the current 3D view. It runs on background threads and can be
cancelled.

| Setting | Choices |
|---|---|
| Image size | 1280 x 960, 1920 x 1080 |
| Samples per pixel | 64, 256, 1024 |
| Technique | Physically Based, Clay |
| Sun date, time, latitude | As in the Sun Angle window |

Press **Render**; a progress bar shows `done / total samples` and the preview refines as passes
finish (the button becomes **Cancel**). The result reads "Finished: 256 samples", or "Stopped after
80 of 256 samples" if cancelled. **Save PNG...** writes `render.png`. If the model is empty the dialog says "There is nothing to render".

How the tracer works: a binned-SAH BVH over every triangle; Lambert plus a GGX coat per
material; thin-sheet glass with Fresnel (index 1.5); soft sun shadows and point lights; sky by
cosine sampling; Russian roulette after 2 bounces; ACES, Reinhard or linear tone mapping; an
optional bilateral denoise. Output is deterministic: the same seed gives byte-identical
pixels for any thread count. On Apple silicon in a release build, a 480x360 image at 64 samples takes
about half a second on 20 threads; a 1920 x 1080 image at 256 samples is a minutes-scale job on
a laptop. The PNG is uncompressed (about 4 bytes per pixel). Real projects with tens of thousands of
triangles are 2 to 4 times slower than the small test scene.

Not built: depth of field, light sets, glass and material overrides per surface, and animation
(planned).

## 10.7 Elevations and sections

`plan-elevation` turns the 3D scene into hidden-line vector elevations (Front, Back, Left,
Right), cross sections and a plan overhead, as weighted line drawings (Heavy, Medium, Light,
Hidden, plus Hatch and Annotation lines). The Create Construction Set PDF draws them (chapter 11).
Elevation and section cameras open as these drawings in the 3D panel (below), Auto Elevations and Wall Elevation cameras make them
(10.11), and Send to Layout puts them on a layout page (chapter 11.3). There is no command that sends one to CAD.

### Elevation rendering (camera Rendering tab)

The Camera Specification of a **Cross Section/Elevation camera** (and of a wall elevation or auto elevation camera) has an
**Elevation rendering** section on its Rendering tab. The choices are stored on the camera, so they round-trip with the plan:

| Setting | What it does |
|---|---|
| **Hatch materials** | Draws material hatch lines on the visible faces: brick, siding (6" lap), stucco, stone, roof shingle, concrete and glass; other materials stay plain. Off by default. |
| **Shadows** | Casts shadows from the sun. Set **Sun azimuth (from north)** and **Sun height** by hand (default 135 degrees and 45 degrees), or fill in **Date (month, day)**, **Solar time (hours)** and **Latitude** and press **Set sun from date and time**. The sun must be 0 to 90 degrees high. |
| **Section back-clip depth** | For a cross section: limit the drawing to that depth behind the cut (120" when first ticked), set in **Depth behind the cut**. |
| **Line weight by distance** | Lines more than 12" behind the nearest drawn line step down one weight class. |
| **Labels (title, levels, roof pitch)** | Adds the title (the camera's name), "T.O. SUBFLOOR" and "T.O. PLATE" level callouts, "GRADE" with a grade line, and roof pitch triangles such as `8:12`. On by default. |

The editor turns these into the engine's `Options` (`elevation_options`) and `render_elevation` draws the camera's 2D drawing with them. The vector view (below) and camera
boxes in a layout read them; the construction set's own elevation and section boxes do not.

### Opening a vector elevation (Vector View and Technical Illustration)

Creating an elevation, section or wall elevation camera opens its 3D view (`Tab` in a 3D view steps through the plan's cameras). Choose the
**Vector View** or **Technical Illustration** technique from the technique box in the panel's bar. The 3D view is replaced by the camera's drawing: weighted lines, filled
faces, gray poche on cut surfaces, shadows, material hatch and labels, according to the camera's Elevation rendering settings.

| | Vector View | Technical Illustration |
|---|---|---|
| Faces | White | Filled with the surface's material color |
| Cut surfaces | Gray poche | Darker poche |
| Lines | Heavy, medium and light weights; cut lines heaviest | The same, heavier |
| Shadows | When the camera's Shadows option is on, or the Sun Angle toggle is on | Always: from the Sun Angle when it is on, else from the camera's own sun |

The panel's bar has the technique box, **Refresh**, **Send to Layout**, **Layout PDF...**, **Ray Trace...** and **Back to Plan**. Drag to pan, scroll to zoom at the pointer, double-click to fit
the drawing. The drawing is made on a worker thread (the bar shows a spinner and the view says "Drawing the view..."), remade when the plan, the camera, the technique or the Sun Angle changes, and
**Refresh** remakes it on demand. "Nothing to draw: the view is empty." appears for an empty plan.

Limits. The drawing is made from the nearest axis: a cross section or wall elevation drawn at an angle is cut square to the nearest of the four sides, through the center of its line, so its drawing
does not follow an angled cut. Everything in "What the engine draws" below applies.

### What the engine draws

- **Regions** (on by default): faces, section cuts and shadows are traced as polygons, so a cut is filled with a gray poche
  and shadows with a lighter gray beneath the lines. Rings are simplified at about half a pixel.
- **Hatch**: patterns are coarsened for a **1/4" = 1'-0" sheet** and start at each region's lower-left corner; the scale is fixed, it does
  not follow the layout box's scale.
- **Shadows**: a shadow map at the raster size with a constant bias; faces turned from the sun count as shadowed. The ground plane is
  only drawn (and shadowed) in the plan overhead view. It is an approximation.
- **Depth weights** and **labels** as in the table. **Sections**: the cut line is Heavy and the cut faces are filled.
- **Limits**: accuracy is about one pixel of the depth buffer, so fine detail is dropped; glass occludes like a solid; sections assume
  closed meshes; there are no curves (a curved wall is its facets); the 3D scene it reads has no stairs, cabinets, electrical or terrain.

## 10.8 Materials

`plan-materials` holds a Chief-style material system as data: 45 materials (Sand Finish,
Drywall, Fir Framing, OSB-Hrz, siding, masonry, roofing, flooring, glass, metals, site), scale-aware
2D hatches (brick, block, shingle, lap siding, tile, herringbone, insulation, concrete, earth,
grass), deterministic procedural textures, default assignments per object (Wall, Door, Window,
Room, Cabinet, Roof), the nine rendering techniques, and sun position from date, time and
location. The editor uses its technique list and sun calculation; the rest (Material Painter,
material tabs on objects, textures in the viewport) is (planned).

## 10.9 Export: glTF

3D > Export > **glTF...** asks for a file name and writes `<name>.gltf` plus `<name>.bin`
(glTF 2.0). The model is the same scene the 3D view shows. UVs are in feet. See chapter 12.

## 10.10 Differences from Chief

- No textures or material assignment in the view; surfaces use flat material colors.
- Many Chief techniques are approximations of the real look.
- Cameras do not yet carry shadows (perspective), lock, or per-camera lighting. Elevation hatch is fixed at a 1/4" scale, and a vector elevation is cut square to the nearest axis.
- No terrain or electrical devices in 3D, no stairs or cabinets at the Round 7 commit (QA-05, QA-06; Round 8), and Build Framing's wall, floor and roof members are not drawn in 3D.
- Curved walls have no door or window cuts in 3D.
- Walkthrough recording uses the path tracer at low quality (640 x 480, 8 samples per pixel, denoised, 12 frames a second) and writes a PNG sequence, not a video file (10.12).
- The Lighting submenu, per-light color temperature and the other lighting tools of Chief are not built; lights are simple point lights (10.13).

## 10.11 Auto Elevations and Wall Elevation cameras

These make the elevation cameras whose drawings 10.7 describes. They are on the Full Camera flyout and in 3D > Create Auto Elevations.

| Tool | Gesture | What it makes |
|---|---|---|
| Wall Elevation Camera | Click a wall. | An elevation camera named "Wall Elevation" (numbered if the name is taken) standing on the side of the wall you clicked and looking at that face. Its cut line sits 1/2" in front of the face and reaches 2" past the far face, and the drawing is back-clipped to the wall's thickness. "Click a wall to make its elevation" if nothing is under the pointer. |
| Auto Elevations | One click. | The four exterior elevation cameras (north, east, south and west) 24" outside the building, made or, if they exist, updated. The status bar says "Auto Elevations: 4 cameras". "Draw some walls first" with no walls. |
| Auto Back-Clipped Elevations | One click. | Four back-clipped sections outside the building that draw 60" behind the building face. |

Each new camera is selected, the tool returns to Select Objects and the first camera's 3D view opens. Open the Camera Specification (double-click the camera symbol) to change its name, its Elevation rendering options and its
back clip. In a layout, send a camera with the Send to Layout dialog (chapter 11.3).

## 10.12 Walkthroughs

A **walkthrough** is a camera object that moves along a path you draw in the plan.

- **Create Walkthrough Path** (Walkthrough flyout, 3D > Walkthroughs): click the first point, click each further node, and double-click or press `Enter` to finish. Press and drag at a node to fix its look direction
  (otherwise the camera looks along the path). The camera object keeps the path, an eye height for each node (66" by default) and the optional look directions.
- **Camera Specification of a walkthrough**: **Walking speed** (6" to 600" per second, 36" by default) and a **Path nodes** table (x, y, height and look direction per node, with a remove button), and the length and the
  time in seconds.
- **Play Walkthrough** (Walkthrough flyout, 3D > Walkthroughs) opens the 3D view from the first node and plays the path at its speed. The bar shows **Play Walkthrough** (it becomes **Pause**), **Stop**,
  a time slider in seconds you can scrub, the **Speed** in inches per second (editing it changes the camera, with merged undo) and **Record Walkthrough...**. "No walkthrough yet: draw one with Create Walkthrough Path" if there is none.
- **Record Walkthrough...** asks for a folder and renders the walk as numbered PNG frames, `frame_0001.png`, `frame_0002.png` ..., with a `make_video.sh` script that runs `ffmpeg` to assemble `walkthrough.mp4` (ffmpeg is not needed to record). A
  Record Walkthrough window shows "n / total frames" with a Cancel button, and the status bar reports "Recorded n frames to <folder>". The recording runs on a background thread.

**Quality is low on purpose.** Recording uses the path tracer (`plan-render`), not the live viewport (the viewport has no offscreen target): 12 frames a second at 640 x 480, 8 samples per pixel, denoised, with the current Sun Angle sun and the plan's lights (10.13).
A walk of a minute is about 720 frames. Expect soft, noisy stills; raise the quality by rendering stills with Ray Trace (10.6) from Full Camera views instead. The output is an image sequence, not a video file.

## 10.13 Lights and the Sun Angle

**Add Lights** (row 1 Add Lights flyout) places **point lights** in the plan: click to place one at the default height; click an existing light to open Adjust Lights; `Delete` removes the selected light. Lights are drawn in the plan
(a small symbol of about 9" radius) and saved with the plan in the typed `Project.lights` slot (chapter 12.2).

**Adjust Lights** (the flyout entry, or `Ctrl+Alt+Cmd+L`; `Ctrl+Alt+L` off macOS) opens a dialog with:

| Section | What it holds |
|---|---|
| Lights in the plan | A row per light: on/off, a name, height above the floor, power (intensity), color, a shadows check box and a remove button. A double-clicked light is highlighted. |
| Electrical fixtures | **Lighting fixtures of the electrical plan emit light**: ceiling lights, recessed cans, pendants and sconces from chapter 9 also light the render (on by default). |
| New lights | The defaults of the Add Lights tool: **Fixed height** (84" when you tick it) or, by default, 12" below the ceiling; **Power** (1.0 lights a room from a 7' ceiling) and a color (warm white by default). |

OK stores the changes as one undo step.

Lights are used by the **ray tracer** (Ray Trace... and the recorded walkthroughs), alongside the sun; the live OpenGL view does not draw them. Chief's Lighting submenu, color temperature and
light sets are not built. The Sun Angle (10.5) is the sun for the ray tracer, the viewport's key light and the shadows of vector elevations.
