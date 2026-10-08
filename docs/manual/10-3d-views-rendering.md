# Chapter 10: 3D Views, Cameras and Rendering

Every 3D view is generated from the plan. Edit a wall and the 3D model follows. This
chapter covers the 3D views, the camera objects, the rendering techniques, the Sun
Angle, the ray tracer and glTF export.

## 10.1 What is in the 3D model

`plan_3d::build_scene` turns the plan into triangle meshes:

- Walls, with openings cut through them and reveals at the jambs. The exterior surface takes its
  material from the wall type's outer layer (stucco, siding, brick, stone, concrete ...). Half walls and
  pony walls are cut down to their height, and a wall flagged Railing is drawn as posts, rails and balusters.
- Door leaves and window frames, sashes and glass for every opening style the model knows (hinged, sliding,
  pocket, bifold, garage, barn, shower, fixed, casement, bay, box and bow), with lite grids.
- Optional detail through `SceneOptions`: doors drawn open at an angle, interior and exterior casing, window
  sills and exterior thresholds. The editor's 3D view calls the builder with the defaults (doors closed, no
  casing), and nothing in the editor sets the non-hinged styles or the wall flags yet, so today you see
  closed hinged doors, plain windows and full-height walls (planned: View options for open doors and casing).
- Floor and ceiling slabs for every detected room.

The editor adds **roof planes** (chapter 8). That is the whole model in the 3D view today. Stairs, cabinets,
library symbols, electrical devices, terrain and framing have 3D builders in their own crates
but the 3D view does not draw them yet (planned). Coordinates: X is plan x, Y is up, Z is
negative plan y, all in inches. The scene is rebuilt automatically when the plan changes (it
watches a hash of the floors, walls and openings), and **3D > Rebuild 3D** forces it.

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
| **Mouse-Orbit Camera** | | Opens (or switches to) the orbit view. |
| **Cross Section Slider** | | Toggles a slider that moves a section plane through the model. |
| **Rendering Techniques** flyout | | Choose a technique (10.4). |
| Create Walkthrough Path, Add Lights | | (planned) |
| **Sun Angle** (toggle) | | Opens the Sun Angle window (10.5). |
| Material Painter, Material Eyedropper, Object Eyedropper, Delete Surface, Adjust Material Definition, Interactive Material Editor | | (planned; engine in `plan-materials`) |

### The 3D menu

Create Orthographic View (Front, Back, Left, Right Elevation; Plan Overhead; Cross
Section/Elevation; Back-Clipped Cross Section), Create Perspective View (Full Camera, Perspective
Full Overview `Shift+K`, Perspective Floor Overview, Doll House View, Ray Trace...), Rendering
Techniques, Rebuild 3D, Export > glTF..., and **3D View Defaults... `Cmd+1`**. The rest of Chief's
3D menu (camera movement, isometric views, walkthroughs, materials, lighting, camera view options,
Create Auto Elevations) is listed and dimmed (planned).

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
- The Project Browser lists the plan's cameras.

### Dialog: Camera Specification

| Tab | Fields |
|---|---|
| General | Name; Camera Type (read-only: Full Camera, Perspective Overview, Doll House View, Cross Section, Wall Elevation, Orthographic); Floor; Camera Position (or Cut Line and Center for a section); View Direction; Section Length (sections) or Height Above Floor and Angle of View (perspective) |
| Options | **Clipping**: Back Clip Distance, Far Clip Distance (Limit view distance). **Display**: Show camera in plan, Locked camera (both disabled) |
| Rendering | Technique (kept per camera for the session), Cast shadows (disabled) |

The angle of view is limited to 5 to 170 degrees.

### Dialog: 3D View Defaults (`Cmd+1`)

Camera eye height (12" to 600"), Angle of view, and the Rendering technique used when a view opens.

## 10.4 Rendering techniques

Row 1 Rendering Techniques flyout, 3D > Rendering Techniques. Nine techniques are listed:

| Technique | In the interactive 3D view |
|---|---|
| Standard | Shaded surfaces with an edge overlay. |
| Vector View | Flat shading, white background, edge lines. |
| Technical Illustration | Flat shading with material-color fill and edge lines. |
| Watercolor | Flat shading on a warm paper background (an approximation of the bitmap technique). |
| Line Drawing | Edge lines on white. |
| Glass House | Every surface transparent glass so structure behind shows. |
| Physically Based | Looks like Standard interactively; the real result comes from the Ray Trace window (10.6). |
| Clay | Uniform gray material on a gray background. |
| Duotone | Two-tone approximation on a warm background. |

Switching technique never changes the model, only how the view is shown.

## 10.5 Sun Angle

The Sun Angle toggle opens a small window: **Date** (month and day), **Time** (0 to 24 hours),
**Latitude** (-66 to 66 degrees). It shows the sun's azimuth and altitude and uses the sun as the
viewport's key light while it is above the horizon. The same sun is used by the ray tracer.

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
Hidden). The drawings are used by the Create Construction Set PDF (chapter 11). There is no
editor command yet that opens one as an elevation window or sends it to CAD, and Create Auto
Elevations is (planned).

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
- Cameras do not yet carry shadows, lock, or per-camera lighting.
- No framing, terrain, stairs, cabinets or fixtures in 3D.
