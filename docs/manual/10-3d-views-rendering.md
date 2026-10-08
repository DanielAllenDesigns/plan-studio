# Chapter 10: 3D Views, Cameras and Rendering

Every 3D view is generated from the plan. Edit a wall and the 3D model follows. This
chapter covers the 3D views, the camera objects, the rendering techniques, the Sun
Angle, the live view's shadows, ambient occlusion and quality settings, the ray tracer (sky, exposure, depth of field, Save Image), the textures of the 3D view and the ray tracer, the
vector elevations you can open as drawings (free-angle sections, plan callouts, automatic dimensions, material labels, DXF), Auto Elevations, Auto Interior Elevations
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
  - a **curved** wall is built as a run of straight facets (one per 7.5 degrees) of its class, its ends mitered against the walls joined to it, and **doors and windows
    are cut through it**: the jambs and head follow the arc and each unit stands square to the arc's tangent at its center (chapter 2.2); a gable end on an arc rises to the roof.
- Door leaves and window frames, sashes and glass for every opening style (chapter 3.2), with lite grids: hinged and double
  doors, sliding doors (two to four overlapping panels on two tracks), pocket, bifold (a pair of panels, or two pairs), garage, barn, shower and
  fixed doors, a doorway (jambs only); hung and fixed windows, casements (one sash, two from 4' wide), sliding windows, awnings and hoppers
  (hinged at the head or the sill, with a hinge bar), and bay, box and bow windows projecting from the exterior face. A Pass-Through is an empty hole and a Wall Niche is a recess in the room face
  of the wall (3 1/2" deep, not a through hole).
- Optional detail through `SceneOptions`: doors drawn open at an angle, interior and exterior casing, window
  sills and exterior thresholds. The editor's 3D view calls the builder with the defaults (doors closed, no
  casing), so today you see closed doors and no casing (planned: View options for open doors and casing).
- Floor and ceiling platforms for every detected room, at the room's own Floor Height offset and Ceiling Height when its Room Specification sets them (chapter 4.4), with any **Hole in Floor Platform** and **Hole in Ceiling Platform**
  cut out (chapter 16). The room's function shapes them: a Garage floor sits 24" lower on a concrete slab, a Deck or Porch has no ceiling platform,
  an Open Below, Attic or Courtyard room has no floor platform, and the ceiling of the room under an Open Below room is open to it. The floor and ceiling layers of a Floor/Ceiling
  Structure Define give the platform its thickness. A room nested inside another room leaves a hole in the surrounding room's platforms. A dropped Garage floor, and any room with a **Stem Wall** height, get concrete **stem walls** under the room's exterior walls, from the underside of the floor platform up to the floor level, interrupted at garage doors (chapter 4.4).
- **Walls follow the roof** (chapter 8.3): gable ends rise to the roof in a triangle, hip and shed roofs clip the walls under them, interior walls rise to a vaulted ceiling, and
  butting roofs are trimmed with flashing and attic walls fill the gap above a lower roof. Half, pony and foundation walls are cut by the roof the same way but are never raised to a gable (a curved standard wall is, facet by facet); with **Roof Cuts Wall at Bottom** a wall standing over a lower roof is cut along that roof (chapter 8.4a). Railing, glass, fencing and the other special classes keep flat tops.
- Slabs, slab holes, footings, square pads and round piers (chapter 16).
- Wall **bottom heights**: a wall that starts above the floor (chapter 2.9) is built from its bottom to its top, for standard walls and every straight wall class;
  the openings keep their sill heights measured from the floor, so only the part of an opening inside the wall is cut.

The editor appends:

- **Roofs** (chapter 8): each plane as a 6" slab with its holes cut (and trimmed where it butts a taller wall), skylights (curb, frame, glass), vaulted ceiling planes
  and dormers, and the eave detail: the eave cut (plumb, level or square), fascia, soffit or exposed rafter tails, rake boards, optional frieze, gutters, ridge and hip caps, and flashing (Trim, Roof and Metal materials), each mesh tagged with its plane
  so a click in 3D picks the plane. The sizes and switches come from Roof Defaults (chapter 8.4a) and a plane can override them.
- **Framing** (chapter 11.11): the members placed by the framing tools and all the members Build Framing makes (wall studs, plates and headers, floor joists, rafters with their
  tail cuts and birdsmouths, trusses), one box per piece, on the Framing layers.
- **Electrical devices** (chapter 9.4a): cover plates, trim rings, pendants, fans, sconces, detectors, the panel and rope lights, in the finish chosen for each.
- **Placed library symbols**: a box of the symbol's width, depth and height for a built-in symbol, and the decoded meshes
  for a Chief catalog object (a box when the geometry is partial, nothing when the catalog is unavailable; chapter 6.6).
- **Exterior details** (chapter 17): corner boards, quoins, moldings, floor and wall material regions, polygon decks and the 3D solids.
- **Cabinets** (QA-05, fixed in Round 8): every placed cabinet kind (base, wall, corner, fillers, custom countertops and backsplashes ...)
  is meshed at its stored position, rotation and floor elevation. Countertops are drawn in the Stone material and handles in Metal.
- **Stairs** (QA-06, fixed in Round 8): treads, risers, stringers, winder and landing slabs, ramps, and the walls, half-walls and railings
  (newels, balusters, rails) on each side. Treads, landings and ramps use the Framing material; risers, stringers, walls and railings the
  Trim material, so the Floor material stays the room slabs. The stairwell hole Auto Stairwell cuts in the floor above is cut out of that floor's
  platform (QA-04; chapter 7.4).
- **Terrain** (chapter 9): the surface after Build Terrain, the roads draped on it, and the landscape objects (walls and curbs, features, garden
  beds, grass, water, stepping stones, plants, sprinklers). The landscape uses its own Grass, Mulch, Foliage and Water materials: lawn and grass regions Grass, garden beds Mulch (or their named material), canopies Foliage, water Water (translucent), trunks Framing, edging and stones Stone.
- **Pictures, billboards and 3D solid features** (chapter 6.7): a picture or billboard is a quad that shows its **own bitmap** in the GL view (PNG or JPEG, with the picture's transparent color cut out; 10.8a), or a flat-colored quad when the file is missing or cannot be decoded, and always in the ray tracer; billboards turn to face the camera in the live view (glTF export and the ray tracer keep the stored angle); a 3D Solid Feature draws its library item in Concrete.

Coordinates: X is plan x, Y is up, Z is
negative plan y, all in inches. The scene is rebuilt automatically when the plan changes (it
watches a hash of the floors, walls, openings, placed symbols, room names, cabinets, stairs, roofs, foundation objects, details and the terrain), and **3D > Rebuild 3D** forces it.

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
| Material Painter, Material Eyedropper, Delete Surface (toggles) | | Work: the 3D view's paint, pick-material and remove-material modes (10.8). |
| Adjust Material Definition, Interactive Material Editor | | Work: they open the Material Builder and the Materials list (10.8). |
| Object Eyedropper | | (planned) |

### The 3D menu

Create Orthographic View (Front, Back, Left, Right Elevation; Plan Overhead; Cross
Section/Elevation; Back-Clipped Cross Section), Create Perspective View (Full Camera, Perspective
Full Overview `Shift+K`, Perspective Floor Overview, Doll House View, **Glass House View**, **Floor Camera**, Ray Trace...), **Create Auto
Elevations** (Auto Elevations, Auto Back-Clipped Elevations, Wall Elevation Camera), **Walkthroughs**
(Create Walkthrough Path, **Walkthrough Path from CAD Polyline**, Play Walkthrough, Record Walkthrough...), Rendering Techniques, Rebuild 3D,
**Save Camera**, **View Quality** (Preview, Final View), Export > glTF..., **Materials..., Material Painter, Adjust Materials... and Material Builder...** (10.8), **Lighting**
(**Lighting...**, Add Lights, Adjust Lights), **Delete Surface**, and **3D View Defaults... `Cmd+1`**. Chief's camera-movement submenus, isometric views and camera view options are not in the menu (`DECISIONS.md`
item 19); Create Orthographic View covers the view directions.

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

### Selecting in 3D

Click an object in a 3D view to select it. The selection is the plan's own, so the plan shows the same objects when you go back, and the 3D view tints them orange. `Shift`-click adds an object to the selection or removes it. Double-click selects the object and opens its specification. `Delete` or `Backspace` deletes the selection. A click on a surface with no object clears the selection. Not built: dragging objects in 3D, Alt-click to set the orbit center, and a hover highlight.

## 10.3 Camera objects

A Full Camera or a section camera is a plan object on the `Cameras` layer, drawn as a camera
symbol with a viewing cone. Select it with the Camera tool (or Select Objects).

- **Move** handle: drags the camera in plan; the 3D view updates live.
- **Aim** handle: rotates the view direction; `Shift` snaps to 15 degrees.
- **Clip** handle: sets the back clip distance.
- **Angle of view** handles (the two far corners of the view cone, round) and the **Tilt** handle (a diamond a quarter of the way along the cone: drag it forward to look up, back to look down, 85 degrees either way) belong to Full and Floor Cameras. With the Camera tool active, select the camera and drag them; each drag is one undo step and the 3D view follows live. (The Select tool lists Move, Aim and Clip only; see the integration queue.)
- Section cameras also have the **two line-end** handles.
- `Tab` cycles cameras, `Delete` removes one, double-click opens the Camera Specification.
- A camera marked **Locked camera** in its Camera Specification ignores every handle ("This camera is locked").
- The Project Browser lists the plan's cameras (Cameras node). Click one to select it, switch to its floor and pan the plan to it; **double-click, or right-click > Restore (open 3D view)**, opens its 3D view; right-click also offers Rename, Delete and Send to Layout. **Save Camera** at the top of the node keeps the 3D view on screen as a camera (below).
- Camera kinds: **Full Camera**, **Floor Camera** (a Full Camera whose 3D view shows only its own floor, clipped at that floor's ceiling), **Perspective Overview**, **Doll House View**, **Glass House** (the overview drawn as a Glass House), **Framing Overview** (the kind only: it opens like an overview, the framing view fills it), cross sections and elevations, and walkthroughs. Floor Camera and Glass House View are in the Camera and 3D View flyouts and the 3D > Create Perspective View menu.
- A section or elevation camera also draws a **callout** in the plan (10.7).

### Dialog: Camera Specification

| Tab | Fields |
|---|---|
| Camera | Name; Camera Type (read-only); Floor; Camera Position (or Cut Line and Center for a section); View Direction; Section Length (sections) or Height Above Floor, Angle of View and **Tilt** (Full and Floor Cameras; positive looks up, 85 degrees either way); **Clipping**: Back Clip Distance, Far Clip Distance (Limit view distance); **Floors Displayed**: all floors or this floor and below (a Floor Camera always shows its own floor clipped at its ceiling); **Display**: Show camera in plan, Locked camera. A walkthrough instead shows **Walking speed**, the **Path nodes** table (10.12) and its Record Walkthrough rate |
| Backdrop | **Default sky**, **Sky color**, or **Image**: a picture from Chief's Backdrops folder (`~/Documents/Chief Architect Premier X18 Data/Backdrops`, read when the view is drawn, never copied) or any JPEG/PNG path |
| Rendering | **Technique** (all nine, saved with the camera), **View quality** (Preview or Final View), **Cast shadows**, and **Override the plan's lighting for this camera** (Ambient light and Sun intensity; otherwise 3D > Lighting rules). Elevation and cross-section cameras (and wall elevations) add an **Elevation rendering** section (10.7). |
| Label | Label text (empty uses the camera's name), **Show the label beside the camera in the plan**, **Show the label over the 3D view** |

The angle of view is limited to 5 to 170 degrees. Everything on the tabs is saved with the camera (`CameraObject.view`), so a saved camera reopens with its technique, tilt, backdrop and quality. OK is one undo step.

**Backdrops.** The picture fills the view behind the model, scaled to cover it (the overflow is cropped), in the techniques that draw a sky (Standard, Physically Based, Clay, Watercolor, Duotone). A picture that is not found falls back to the default sky and the dialog says so. Pictures are decoded once and shrunk to 2048 pixels on the long side for the graphics card.

**Preview and Final View.** The 3D bar has **Preview** (fast: no shadows or occlusion, low quality) and **Final View** (shadows, occlusion and anti-aliasing at the default Medium quality; Shading > Quality raises it to High) buttons, and 3D > View Quality has the same two rows. A camera opens in the quality saved on it; the Shading menu still adjusts the individual switches.

### Saved cameras (Save Camera, Restore, Delete)

**3D > Save Camera** (or **Save Camera** in the Project Browser's Cameras node) keeps the view on screen as a camera, in one undo step, named "Saved Camera n":

- a **Full Camera** view becomes a Full Camera at the eye, with its direction, height, angle of view and tilt;
- a **Perspective Overview** or **Doll House** view keeps its eye and target (`CameraView.pose`) so Restore brings back the exact orbit, and is not drawn in the plan (**Show camera in plan** off); a Glass House overview is saved as a Glass House camera;
- the rendering technique, Preview or Final View and, for a Floor Overview, "this floor and below" are saved with it.

The orthographic views (elevations, Plan Overhead) cannot be saved ("Save Camera works from the perspective views"). **Restore**: double-click the camera in the Project Browser, or right-click > Restore (open 3D view). **Delete**: right-click > Delete (or select it and press `Delete`); both are one undo step.

### Dialog: 3D View Defaults (`Cmd+1`)

Opened from 3D > 3D View Defaults... (`Cmd+1`; the menu shows whatever key the live hotkey map gives it).
Camera eye height (12" to 600"), Angle of view, and the Rendering technique used when a view opens, plus the look of the plan **callouts** of section and elevation cameras: **Callout shape** (Circle, Square or Hexagon), **Callout size** (radius, 4" to 36") and **Callout shows the name** (10.7).

## 10.4 Rendering techniques

Row 1 Rendering Techniques flyout, 3D > Rendering Techniques. Nine techniques are listed:

| Technique | In the interactive 3D view |
|---|---|
| Standard | Shaded surfaces with sky light, sun shadows and soft ambient occlusion (10.4a), and **textures** while the 3D toolbar's Textures box is ticked (10.8a). |
| Vector View | Flat shading, white background, edge lines. On an elevation, section or wall elevation camera it shows the camera's **vector drawing** instead (10.7). |
| Technical Illustration | Flat banded color with bold **silhouette and crease lines** drawn from the depth and normals of the picture, so the edges follow the model at any zoom. On an elevation, section or wall elevation camera it shows the vector drawing with material-colored faces and heavier lines, always with shadows (10.7). |
| Watercolor | A **pigment wash**: soft color, darkened edges and paper grain over a warm paper background (an approximation of Chief's bitmap technique). |
| Line Drawing | Edge lines on white. |
| Glass House | Every surface transparent glass so structure behind shows. |
| Physically Based | Standard with glossier highlights and sky reflections (textures included); the real result comes from the Ray Trace window (10.6), which textures it too. |
| Clay | Uniform gray material on a gray background. |
| Duotone | Two-tone approximation on a warm background. |

Switching technique never changes the model, only how the view is shown. Only Standard and Physically Based show textures; the other techniques keep their flat looks.

### 10.4a Shadows, ambient occlusion and quality in the live view

The 3D toolbar has a **Shading** menu (it appears in the standard 3D views; the vector drawings have their own bar) with the settings of the OpenGL view:

| Setting | What it does |
|---|---|
| Shadows | The sun casts shadows through a **shadow map** fitted to the model, filtered with percentage-closer filtering (PCF) so the edges are soft. The sun is the Sun Angle sun (10.5). Looks that do not use shadows (Line Drawing and other flat looks) ignore it. |
| Ambient occlusion | Screen-space occlusion (SSAO): a half-resolution pass samples a hemisphere around each pixel from the depth buffer and a blur removes the noise, darkening creases and contact areas. |
| Quality | **Low**: a 1024 shadow map with hard edges, 8 occlusion samples, no anti-aliasing. **Medium** (the default): 2048 map with 3 x 3 PCF, 12 samples, FXAA. **High**: 4096 map with 5 x 5 PCF, 16 samples, FXAA. |
| Exposure | A slider from 0.5 to 2.0 applied before the tone curve. |

Also in the live view: a sky gradient and ground fade behind the model (a flat horizon color in orthographic views), **GGX specular highlights with Fresnel and a roughness and metalness for each scene material** (glass and water also reflect the sky), up to **8 point lights** (the nearest plan lights and lighting fixtures of the electrical plan, 10.13) evaluated in the shader, and **FXAA** anti-aliasing at Medium and High. The Technical Illustration edge lines and the Watercolor wash are part of the same final screen pass.

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
| Sky | **Clear sky** (the Preetham analytic sky, with a **Turbidity** slider from 2 for very clear to 10 for hazy; it brightens toward the horizon and the sun and changes color from noon to sunset) or **Gradient** (the two-color zenith-to-horizon blend). The clear sky needs the sun above the horizon, otherwise the gradient is used. |
| Exposure | A slider in stops (EV), -3 to +3, applied before tone mapping |
| Depth of field | **Aperture** (0 is a pinhole, everything sharp; a larger aperture blurs what is off the focus plane) and **Focus** distance (0 focuses on what is at the center of the image) |
| Noise | **Denoise (keeps edges and textures)**: a bilateral filter guided by the albedo, normal and depth of the first hit, so textures and geometry edges stay sharp while path-tracing noise is smoothed |

Press **Render**; a progress bar shows `done / total samples` and the preview refines as passes
finish (the button becomes **Cancel**). The result reads "Finished: 256 samples", or "Stopped after
80 of 256 samples" if cancelled. **Save Image...** writes the picture as a PNG; the size list beside it saves at **Same size**, **2x size** or **4x size** (the render is made again at that size, reduced until the image fits 8192 pixels on a side and 24 million pixels). If the model is empty the dialog says "There is nothing to render".

How the tracer works: a binned-SAH BVH over every triangle; Lambert plus a GGX coat per
material; thin-sheet glass with Fresnel (index 1.5); soft sun shadows and point lights; **next-event estimation** (area lights, such as lighting fixtures, are sampled directly at each hit, so a room lit by fixtures converges in far fewer samples than by chance alone); sky by
cosine sampling or the Preetham model; Russian roulette after 2 bounces; ACES, Reinhard or linear tone mapping; an
optional bilateral denoise. Output is deterministic: the same seed gives byte-identical
pixels for any thread count. On Apple silicon in a release build, a 480x360 image at 64 samples takes
about half a second on 20 threads; a 1920 x 1080 image at 256 samples is a minutes-scale job on
a laptop. The PNG is uncompressed (about 4 bytes per pixel). Real projects with tens of thousands of
triangles are 2 to 4 times slower than the small test scene.

Textured materials (brick, siding, shingles, wood ...) are painted with their bitmaps in the **Physically Based** technique (10.8a); **Clay** ignores colors and textures.
The Ray Trace window has no switch for this: textures are always on in a render, a perspective box, Print Model and a recorded walkthrough.

Not built: light sets, glass and material overrides per surface, bump or normal maps, and animation
(planned).

## 10.7 Elevations and sections

`plan-elevation` turns the 3D scene into hidden-line vector elevations (Front, Back, Left,
Right), cross sections and a plan overhead, as weighted line drawings (Heavy, Medium, Light,
Hidden, plus Hatch and Annotation lines). The Create Construction Set PDF draws them (chapter 11).
Elevation and section cameras open as these drawings in the 3D panel (below), Auto Elevations, Auto Interior Elevations and Wall Elevation cameras make them
(10.11), and Send to Layout puts them on a layout page (chapter 11.3). **Export DXF** writes a drawing as a DXF (below).

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

The same tab has a **Vector View** section for the annotations and line weights of the camera's drawing, and a **Plan callout** section (below):

| Vector View setting | What it does |
|---|---|
| **Level callouts** (under Labels) | "T.O. SUBFLOOR" and "T.O. PLATE" notes; they need the Labels option. |
| **Automatic dimensions (floor-to-floor, openings)** | Adds the vertical dimension strings left of the building (below). |
| **Material labels (siding, brick, roofing)** | Adds a text leader naming the cladding or roofing of each region (below). |
| **Line weights from the layers** | Draws each wall's and opening's lines at the pen weight of its layer in Layer Display Options (Heavy, Medium or Light class), instead of the engine's own weights. |
| **Dashed hidden lines** | Draws hidden edges dashed. |
| **Export drawing as DXF...** | Writes this camera's drawing as a DXF (below). |

The editor turns these into the engine's `Options` (`elevation_options`) and `render_elevation` draws the camera's 2D drawing with them. The vector view (below) and camera
boxes in a layout read them; the construction set's own elevation and section boxes do not.

### Free-angle sections and elevations

A **Cross Section/Elevation** camera or a **Wall Elevation** camera is cut along its own line at **any angle**, not only square to a side of the plan: the engine turns the scene about the
vertical axis into the camera's frame, runs the same hidden-line pipeline and clips the drawing to the length of the camera line (`plan-elevation` `FreeView`). Drawing space is view-local: X runs along
the camera line from its left end (looking along the view direction) to its right end. An exterior elevation camera (Auto Elevations) draws the whole building from its direction.

### Plan callouts, view numbers and sheet references

Every section, wall elevation and elevation camera draws a **callout** in the plan: a bubble behind the middle of its cut line (on the viewer's side) with a stem to the line. The bubble holds the **view number**
and, once the camera's view is on a layout page (a Camera box), a dividing line with that **sheet's number** under it (`A-3`). Cameras are numbered in order unless the camera has a number of its own:
in the Camera Specification's Plan callout section tick **Show the callout in the plan** and **View number** (1 to 99); with no number ticked the cameras are numbered automatically in plan order, taking
the lowest number nobody claims. The shape (Circle, Square or Hexagon), size and whether the callout shows the camera's name are set in 3D > 3D View Defaults (`Cmd+1`).

### Automatic elevation dimensions

With **Automatic dimensions** on, the drawing gets vertical dimension strings in a column left of the building, taken from the plan's levels and openings so they follow the model exactly:

- **Floor to floor** (the outer string): from each finished floor to the next, and from the top floor to its top of plate.
- **Openings** (the inner string): per floor, the floor level, the sill and head of every door and window seen in the view, and the top of plate.
- **Overall**: the lowest floor to the highest top of plate.

Values read feet-inches to 1/8".

### Material labels

With **Material labels** on, a text leader names the surface of each cladding or roofing region: SIDING, BRICK, STUCCO, STONE, ROOFING, CONCRETE, TRIM or METAL. The labels line up in a column beside the
drawing. Regions under about 3 sq ft are not labelled, and a second region of the same material is labelled when it is at least 40% as big as the largest and 10' or more away from it.

### Export DXF

The vector view's bar (**Export DXF...**) and the Camera Specification (**Export drawing as DXF...**) write the camera's drawing as a DXF named after the camera (`<camera name>.dxf`). It is an R12 ASCII DXF in inches of the building. The lines go on
layers by weight class (`<camera name>, Heavy`, `Medium`, `Light`, `Hidden`, plus `Hatch` and `Annotation`) and the text on `<camera name>, Text`, so the plotted weights survive in the CAD program; filled regions (poche and shadows) are not written. "Nothing to export: the view is empty" if there is nothing to draw.

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

The panel's bar has the technique box, **Refresh**, **Export DXF...**, **Send to Layout**, **Layout PDF...**, **Ray Trace...** and **Back to Plan**. Drag to pan, scroll to zoom at the pointer, double-click to fit
the drawing. The drawing is made on a worker thread (the bar shows a spinner and the view says "Drawing the view..."), remade when the plan, the camera, the technique or the Sun Angle changes, and
**Refresh** remakes it on demand. "Nothing to draw: the view is empty." appears for an empty plan.

Limits. Everything in "What the engine draws" below applies.

### What the engine draws

- **Regions** (on by default): faces, section cuts and shadows are traced as polygons, so a cut is filled with a gray poche
  and shadows with a lighter gray beneath the lines. Rings are simplified at about half a pixel.
- **Hatch**: patterns are coarsened for a **1/4" = 1'-0" sheet** and start at each region's lower-left corner; the scale is fixed, it does
  not follow the layout box's scale.
- **Shadows**: a shadow map at the raster size with a constant bias; faces turned from the sun count as shadowed. The ground plane is
  only drawn (and shadowed) in the plan overhead view. It is an approximation.
- **Depth weights** and **labels** as in the table. **Sections**: the cut line is Heavy and the cut faces are filled.
- **Limits**: accuracy is about one pixel of the depth buffer, so fine detail is dropped; glass occludes like a solid; sections assume
  closed meshes; there are no curves (a curved wall is its facets, and the openings cut in it follow the arc, chapter 2.2); the 3D scene it reads includes cabinets, stairs and terrain but not electrical devices.

## 10.8 Materials

`plan-materials` holds a Chief-style material system as data: 45 materials (Sand Finish,
Drywall, Fir Framing, OSB-Hrz, siding, masonry, roofing, flooring, glass, metals, site), scale-aware
2D hatches (brick, block, shingle, lap siding, tile, herringbone, insulation, concrete, earth,
grass), deterministic procedural textures, default assignments per object (Wall, Door, Window,
Room, Cabinet, Roof), the nine rendering techniques, and sun position from date, time and
location. The editor uses its technique list and sun calculation, the textures (10.8a) and the material tools below;
material tabs on objects are (planned).

### Material tools (3D menu and row 1 of the 3D buttons)

- **Materials...** lists the library (the 45 core materials plus your own) by category, with a search box; click one to make it
  the **active material**.
- **Material Painter** is a mode of the 3D view. With it on, a click on a surface applies the active material to the object that surface
  belongs to (a wall, a door, a window, a cabinet, a roof plane, ...) instead of selecting it; one undo step, "Paint Material". The
  **Material Eyedropper** makes the clicked object's painted material the active one, and **Delete Surface** removes it. The row-1
  buttons toggle the same modes; the pick uses the object id of the mesh under the pointer (chapter 10.2, Selecting in 3D).
- **Adjust Materials...** works on the selected object: a **Whole object** row and one row per part of the object's kind
  (a wall's Exterior and Interior Wall Surface, a door's Door Panel, Casing, Jamb and Hardware, a window's Frame, Sash and Glass, ...)
  with a drop-down of the library. A part without its own material takes the whole-object one; window glass stays clear unless
  the material is glass. Overrides are stored per object in the plan (`object_materials`).
- **Material Builder...** makes a material: name, category, color, roughness, metallic, transparency, a 2D hatch pattern and an optional
  texture image path. **Save to My Materials** adds it to `~/.plan-studio/materials.json`; it replaces a core material of the same name.
- The viewport shades with 23 fixed scene materials, so a painted object shows the scene material **closest** to the library one (by what it
  is, such as brick, shingle or floor, else by color), with that scene material's texture if it has one (10.8a); the library material's own exact color and texture image are not drawn (`DECISIONS.md` item 17).

## 10.8a Textures

Since Round 11 the 3D view and the ray tracer paint surfaces with bitmaps instead of flat colors. The scene still has 23 fixed scene materials (`plan_3d::Material`); **16 of them are textured** and the other seven (interior wall, ceiling, window glass and frame, door glass, trim and foliage) and the selection tint stay flat. The textured ones are
exterior wall and siding, floor, door panel, roof, stucco, brick, stone, concrete, metal, framing, grass, mulch, water, asphalt and gravel.

**Where a texture comes from** (`plan-materials/src/textures.rs`), for each textured material in this order:

1. **Chief's own texture file**, read at run time from your Chief install when it is there, by file name, in these folders (a missing folder is skipped):
   the folder named by the `PLAN_STUDIO_TEXTURES` environment variable (searched first, if you set it), `~/Documents/Chief Architect Premier X18 Data/Textures`, then
   `/Library/Application Support/Chief Architect Premier X18/Referenced Files`. The names tried are Chief's stock ones, for example `Brick(36).jpg`, `LapSidingCRCAAB.jpg`, `Stucco(48).jpg`, `OakHardwoodHoney.jpg` (then `Oak.jpg`),
   `Asphalt Roofing Grey 2016.jpg` (then `Shingle - Grey.JPG`), `StoneVeneer.jpg`, `Concrete(72).jpg`, `BrushedMetal.jpg`, `Fir(36).jpg`, `Grass5.jpg`, `Mulch(dark).jpg`, `Water3(48).jpg`, `Asphalt-01.jpg` and `Gravel.jpg`.
   A number in parentheses is the size one repeat covers in inches (`Brick(36).jpg` repeats every 36"). The files are Chief's licensed content: they are decoded in memory, never copied, written or committed, and a file larger than 1024 pixels on a side is shrunk on load.
2. A **procedural fallback** generated in code (lap siding, brick with mortar, stucco, shingles, wood grain, stone, concrete, metal, grass, carpet-like mulch, water ripples), tinted from the material's flat color. The repository ships no Chief image, and a machine without Chief looks textured anyway.
   If a Chief file is not found (a different install, a renamed file) or does not decode, the fallback is used for that material alone.

The decoded pictures are cached (least recently used out first, 512 MB), shared by the viewport and the ray tracer. Textures load on a background thread the first time a scene needs them, and at most two are sent to the graphics card per frame, so the view of a big house may fill in a moment after it opens instead of stalling.

**Mapping.** There are no UV coordinates on the meshes. Each fragment is mapped planarly (`planar_uv`, shared by the OpenGL shader, the ray tracer and the tests): a wall or roof face is mapped in **its own plane**, with the picture's top pointing up the wall or up the slope and one repeat covering the tile size in real inches, so siding courses run level on every wall and shingles run down every roof plane;
a flat face, and the floor, ceiling, grass, mulch, water, asphalt and gravel materials, are mapped on **plan X and Z**. (A fully triplanar mapping, blending three projections, is not built.) The graphics card filters with mipmaps and, where the driver offers it, anisotropic filtering; surfaces are lit as before, so a texture darkens in shade like a flat color.

**The Textures switch.** The floating bar of a 3D view has a **Textures** box (on by default; not saved with the plan): off shows the flat scene colors. It works in the **Standard** and **Physically Based** techniques; the other techniques never show textures.

**Pictures.** A picture or billboard (chapter 6.7) shows its own PNG or JPEG bitmap on its quad in the viewport: straight RGBA, the "make one color transparent" key cut out, mirrored if the picture is flipped, shrunk to at most 2048 pixels on a side, decoded once and cached by file, modification time and transparency settings. A file that is missing or cannot be read stays a flat-colored quad. The ray tracer still draws a picture as a flat-colored quad.

**The ray tracer** looks up the same bitmap for a textured material and maps it with the same function (bilinear, repeating). It scales the bitmap's brightness to the material's flat albedo (between 1/4 and 4 times, never above 0.95), so turning textures on changes the pattern and not the exposure. Only the Physically Based technique does this, and only for opaque materials.

**Image formats.** Textures, 3D pictures and layout picture boxes go through a decoder written for the program (`plan-library/src/image`, no external image crate): **PNG** of every color type and bit depth (palette, transparency chunks, interlaced), and **JPEG** baseline and **progressive**, grayscale, YCbCr, RGB and Adobe CMYK/YCCK, any chroma subsampling, restart markers; the EXIF orientation tag is ignored. A 2048 x 2048 JPEG decodes in well under a second even in a debug build. Underlay pictures (12.4a) and the pictures drawn in the plan (6.7) still use their own, older decoders: a JPEG in the plan view is still a framed placeholder.

**Limits.**

- No bump or normal maps, and no texture rotation, offset or mirror (Chief's Material Definition has them).
- A painted object (Material Painter, Adjust Materials; 10.8) takes the texture of the scene material it maps to, not the library material's own image; Material Builder's texture path is kept with the material but is not drawn in the view.
- Pictures are flat quads in the ray tracer, and the selection tint is never textured.
- The mapping is planar: a curved wall's facets and the sides of a stair tread each take the plane of their own face, so a texture can show a seam where faces meet at an angle.
- A texture follows the scene material a mesh carries, so a cabinet, stair or library object is textured only where its mesh uses one of the 16 materials (a countertop in Stone, a stair tread in Framing); glTF export stays untextured (flat material colors, UVs in feet).

## 10.9 Export: glTF

3D > Export > **glTF...** asks for a file name and writes `<name>.gltf` plus `<name>.bin`
(glTF 2.0). The model is the same scene the 3D view shows. UVs are in feet. See chapter 12.

## 10.10 Differences from Chief

- Textures (10.8a) come from 16 scene materials, Chief's own files when your install has them and generated ones otherwise, with no bump maps; there are still no exact colors: a painted object shows the closest of the viewport's 23 fixed scene materials (10.8, `DECISIONS.md` item 17). Pictures are flat in the ray tracer.
- Many Chief techniques are approximations of the real look.
- Cameras carry shadows, lock, tilt, backdrop, quality and a lighting override (10.3), but no Lighting set, fog or ground options, and Floors Displayed offers all floors or this floor and below (not a pick of floors). Elevation hatch is fixed at a 1/4" scale. The angle-of-view and tilt handles of a camera work in the Camera tool, not yet under Select Objects; the tilt handle is a Plan Studio design (the plan has no tilt gesture of its own; verify in Chief).
- Electrical devices and Build Framing's wall, floor and roof members are in the live 3D view, but not in the vector elevations and sections (except the wall framing of the Framing Overview, chapter 11.11). 3D objects cannot be dragged, Alt-click does not set the orbit center, and there is no hover highlight (see Selecting in 3D under 10.2).
- Walkthrough recording uses the path tracer at low quality by default (640 x 480, 8 samples per pixel, denoised, 12 frames a second; the dialog changes all three) and writes a PNG sequence, not a video file (10.12).
- Per-light color temperature and the other lighting tools of Chief are not built; lights are simple point lights (10.13). The live view's shadows are one sun shadow map with PCF; there are no shadows from point lights.

## 10.11 Auto Elevations and Wall Elevation cameras

These make the elevation cameras whose drawings 10.7 describes. They are on the Full Camera flyout and in 3D > Create Auto Elevations.

| Tool | Gesture | What it makes |
|---|---|---|
| Wall Elevation Camera | Click a wall. | An elevation camera named "Wall Elevation" (numbered if the name is taken) standing on the side of the wall you clicked and looking at that face. Its cut line sits 1/2" in front of the face and reaches 2" past the far face, and the drawing is back-clipped to the wall's thickness. "Click a wall to make its elevation" if nothing is under the pointer. |
| Auto Elevations | One click. | The four exterior elevation cameras (north, east, south and west) 24" outside the building, made or, if they exist, updated. The status bar says "Auto Elevations: 4 cameras". "Draw some walls first" with no walls. |
| Auto Back-Clipped Elevations | One click. | Four back-clipped sections outside the building that draw 60" behind the building face. |
| Auto Interior Elevations | Click inside a room. | Four wall elevation cameras for that room (the smallest room when rooms nest), named `<room> North Wall`, `<room> East Wall`, `<room> South Wall`, `<room> West Wall` after the wall each looks at (north is plan +Y; a room without a name is called "Room"). Each stands 1" inside the opposite wall's surface, looks at its wall and reaches 8" past the wall's far face, so the drawing shows the wall, its openings, cabinets and fixtures. Run it again and the same cameras are updated instead of doubled. "Click inside a closed room to make its interior elevations" if no room is there (a room side under 12" gets none). It is in 3D > Create Auto Elevations and on the Full Camera flyout. |

Each new camera is selected (and gets a plan callout, 10.7), the tool returns to Select Objects and the first camera's 3D view opens. Open the Camera Specification (double-click the camera symbol) to change its name, its Elevation rendering options and its
back clip. In a layout, send a camera with the Send to Layout dialog (chapter 11.3).

## 10.12 Walkthroughs

A **walkthrough** is a camera object that moves along a path you draw in the plan.

- **Create Walkthrough Path** (Walkthrough flyout, 3D > Walkthroughs): click the first point, click each further node, and double-click or press `Enter` to finish. Press and drag at a node to fix its look direction
  (otherwise the camera looks along the path). The camera object keeps the path, an eye height for each node (66" by default) and the optional look directions.
- **Create a path from a CAD line**: select a CAD polyline (or line) and choose 3D > Walkthroughs > **Walkthrough Path from CAD Polyline**; every vertex becomes a node (a closed polyline comes back to its start). One undo step.
- **Camera Specification of a walkthrough**: **Walking speed** (6" to 600" per second, 36" by default) and a **Path nodes** table (x, y, height, look direction, **tilt** and **hold** per node, with a remove button), and the length and the
  time in seconds. Each node is a **key frame**: the camera tilts between the node values and stands still for a node's *hold* seconds before it sets off (holds are added to the time).
- **Play Walkthrough** (Walkthrough flyout, 3D > Walkthroughs) opens the 3D view from the first node and plays the path at its speed. The bar shows **Play Walkthrough** (it becomes **Pause**), **Stop**,
  **|<** and **>|** (jump to the previous or next key frame), a time slider in seconds you can scrub with "frame n of m" beside it, the **Speed** in inches per second (editing it changes the camera, with merged undo) and **Record Walkthrough...**. "No walkthrough yet: draw one with Create Walkthrough Path" if there is none.
- **Record Walkthrough...** opens a dialog: **Frames per second** (1 to 60, 12 by default), **Picture size** (640 x 480, 960 x 720, 1280 x 720, 1920 x 1080), **Samples per pixel**, and the **Folder** (typed, or Choose Folder...). It shows how many frames the walk comes to. Record saves the settings on the camera (one undo step) and renders the walk as numbered PNG frames, `frame_0001.png`, `frame_0002.png` ..., with a `make_video.sh` script that runs `ffmpeg` to assemble `walkthrough.mp4` (ffmpeg is not needed to record). A
  Record Walkthrough window shows "n / total frames" with a Cancel button, and the status bar reports "Recorded n frames to <folder>". The recording runs on a background thread.

**Quality is low by default.** Recording uses the path tracer (`plan-render`), not the live viewport (the viewport has no offscreen target): 12 frames a second at 640 x 480, 8 samples per pixel, denoised, with the current Sun Angle sun and the plan's lights (10.13); the dialog raises any of them. Frames are spread evenly in time, so key-frame holds and tilts show; `make_video.sh` uses the chosen rate.
A walk of a minute at the defaults is about 720 frames. Expect soft, noisy stills; raise the quality by rendering stills with Ray Trace (10.6) from Full Camera views instead. The output is an image sequence, not a video file.

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

**3D > Lighting > Lighting...** is the plan's light rig, saved with the plan (`Project.lighting`) and applied in one undo step:

| Section | What it holds |
|---|---|
| Sun | Direction (degrees clockwise from north), Height above the horizon, Sun intensity (0 to 2) and Ambient light (0 to 1); "Set the sun from a date" turns a month, day, solar time and latitude into those angles |
| Interior lights | One switch for **all** interior lights (the plan's lights and the electrical fixtures): off leaves them out of the live view and every ray trace without deleting them; below it the plan's lights with an on/off box and a power each. **Adjust Lights...** opens the full dialog above |

The live view's key light follows the Sun direction and its strength the Sun intensity; a camera can override Ambient light and Sun intensity on its Rendering tab. The Sun Angle toggle (10.5) still overrides the direction while it is on.

Lights are used by the **ray tracer** (Ray Trace... and the recorded walkthroughs), alongside the sun, and by the **live OpenGL view**, which lights each pixel from the nearest 8 point lights (the plan's lights and the electrical fixtures, 10.4a) without shadows from them. Chief's Lighting submenu, color temperature and
light sets are not built. The Sun Angle (10.5) is the sun for the ray tracer, the viewport's key light and the shadows of vector elevations.
