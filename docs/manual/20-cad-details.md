# Chapter 20: Pictures, Underlays and CAD Details

This chapter covers three things that sit beside the model: pictures and underlays (images you place for looks or
for tracing), CAD details (drawings in detail space that are not part of the building), and the commands that make
and manage them: Import Picture, Point to Point Resize, Rotate to Align, Auto Detail, CAD Detail From View, CAD
Detail Management and Detail Components.

## 20.1 Import Picture and the Image Specification

**File > Import > Picture...** opens the Picture File box (PNG or JPEG). The picture is placed flat in the middle of
the plan, its longest side 36", and its **Image Specification** opens (the Symbol Specification hosts it; you can also
open it by double-clicking any picture). One undo step ("Import Picture") takes the picture away again.

| Page | What it sets |
|---|---|
| **General** | The file (Browse...), the size with **Keep aspect ratio** (typing a width sets the depth, and the other way round), Match picture proportions, elevation, position, rotation, flip, and the **transparent colour**: tick the box, pick the colour or press "Use corner colour" (the top-left pixel), and set the tolerance |
| **Image** | Billboard: stand the picture up so it faces the camera |
| **Layer** | The layer the picture is drawn on |
| **Label** | A label for the picture |

Both PNG and JPEG pictures are drawn in the plan (baseline and progressive JPEG). A JPEG that cannot be decoded is
drawn as a frame with a note. Billboard pictures (Create Billboard Image) and the Backdrop images of the camera views are
described in chapters 6 and 10.

## 20.2 Underlays and tracing

An **underlay** is a picture placed under the plan for tracing: a scanned survey, an existing-house plan, a PDF page.
**File > Import > Underlay Picture** or **Tools > Underlays...** imports one; the Underlays window lists them and sets
opacity, rotation, lock and visibility.

### PDF files

Plan Studio has no PDF renderer, so a PDF can be an underlay only when its pages are pictures, which is what a scanner
produces. The file's object table is read directly (including PDF 1.5 object streams): the page tree gives the pages
in order and the largest image of each page is its picture. JPEG pages are kept as they are; Flate pictures (gray, RGB,
indexed, CMYK, 1 to 16 bits, with or without a PNG or TIFF predictor) are written as a PNG into
`~/.plan-studio/underlays/`. The Underlays window's **PDF page** box picks the page. A PDF drawn with vector lines and
text, or stored with CCITT, JBIG2, JPX or LZW, says so: **export the page as a PNG** and import that instead.

### Point to Point Resize

A scan has no scale. Click **Point to Point Resize...** in the Underlays window (or choose the Point to Point Resize
tool in the Image flyout for a picture), click two points on the drawing that are a known distance apart, and type that
distance (for example `24'-0"`). The picture is scaled about the first point so the two points are exactly that far
apart. For an underlay the distance is typed in the Underlays window; for a picture a small window asks for it. Each
resize is one undo step.

### Rotate to Align

Click **Rotate to Align...** (or the tool in the Image flyout) and click the two ends of a line that should be level,
such as a foundation edge or a property line drawn square. The picture is turned about the first point to the nearest
axis (level, or plumb when the line is nearer to vertical), so it is never turned more than 45 degrees. One undo step.

Locked underlays and pictures on locked layers refuse both.

## 20.3 CAD details

A **CAD detail** is a drawing made of CAD lines, text and dimensions in detail space (inches of the detail, printed at
the detail's scale). Each is stored as a floor that carries a detail mark (`Floor.detail`), so the CAD tools, layers,
blocks and undo work in it as they do anywhere; the mark keeps it out of the building: details come after the last
floor, hold no walls, are listed under **CAD Details** in the Project Browser (not under Floors), and open in a tab of
their own.

The details use their own layers, added to the plan the first time one is needed:

| Layer | Holds |
|---|---|
| `Detail, Cut` | The section cut lines |
| `Detail, Lines` | Outlines, silhouettes and the lines between wall layers |
| `Detail, Hatch` | Material hatch |
| `Detail, Insulation` | Insulation batts and rigid insulation |
| `Detail, Framing` | Framing members |
| `Detail, Components` | Flashing, sheathing, siding and anchors from the component catalogue |
| `Detail, Notes` | Titles, the scale note, layer names |

## 20.4 Auto Detail

**CAD > Auto Detail** (and the Auto Detail toolbar button) turns a cross section or an elevation into a detail. With a
section or elevation selected in the plan it is used at once; otherwise the Auto Detail window lists the plan's
sections and elevations and sets the scale (3/4", 1", 1 1/2" or 3" to the foot) and what to draw. The detail is one
undo step and opens in its own tab.

What it draws:

- The camera's drawing with its material hatch made for the detail's scale, as CAD lines on the layers above.
- Every wall the cut passes through, as its assembly: the lines between the wall type's layers (Siding, Sheathing,
  Framing, Drywall for Siding-6), each layer's material pattern as hatch, and the layer names as notes beside the first
  cut wall.
- The framing layer: the framing members Build Framing made for that wall which the cut plane passes through (bottom
  plate, double top plate, and studs when the cut goes through one), each a box with the X that marks cut lumber. A wall
  with no built framing gets an assumed bottom plate and double top plate.
- A batt of **insulation** in the stud cavity between the plates, for exterior walls and for any wall type with an
  insulation layer. A cut that passes through a stud draws the stud and no batt; nudge the section line to see the cavity.
- A title (the callout number and the view's name), the scale, and the text the camera's labels option adds.

The detail is named after its camera: "1 - Wall Section" with the callout number of the section in the plan. When
the section is renamed or its callout is renumbered, the detail's name follows, until you rename the detail yourself.

## 20.5 CAD Detail From View

**CAD > CAD Detail From View** with a section or elevation selected is Auto Detail. Without one it copies the lines of
the active plan floor (wall outlines and visible CAD objects) and its dimensions into a new detail named "<floor>
Detail", at 1/4" to the foot. Run on a detail it says so.

## 20.6 CAD Detail Management

**CAD > CAD Detail Management...** lists the plan's details with their source, object count and scale. For the selected
detail:

| Button | Does |
|---|---|
| **Rename** | Renames it (the name is then yours: callout changes leave it alone); its tab follows |
| **Open** | Shows it in a tab named "Detail: <name>" and switches to it |
| **Duplicate** | Makes "<name> copy" with new ids for every object, style and block |
| **Delete** | Removes the detail and its tab; the editor leaves it if it was showing it |
| **Send to Layout** | Puts a detail box on the last page of the layout at the detail's scale (the layout caption is the detail's name) |

**New Detail** starts a blank one. The Project Browser's CAD Details list opens a detail with a click and offers the same
commands on a right-click. Each command is one undo step.

## 20.7 Detail Components

**CAD > Detail Components...** opens a chooser of built-in pieces, grouped as Framing (2x4 to 2x12 sections, 2x6 wall
plates), Insulation (3 1/2", 5 1/2" and 7 1/4" batts, 2" rigid), Flashing (Z flashing, drip edge, sloped sill flashing),
Sheathing (7/16" OSB, 5/8" plywood), Siding (lap, board and batten, shiplap) and Anchors (1/2" x 10" and 5/8" x 12"
bolts). Real lumber sizes are used (a 2x6 is 1 1/2" x 5 1/2").

**Place by Clicking** arms the component and switches to the Detail Component tool: each click places it, centred on the
click, as a CAD block named after the component, on the detail layers (hatch on `Detail, Hatch`, sheet metal heavier);
Esc stops. **Insert Below the Drawing** puts it under what the floor already holds. A locked detail layer refuses the
placement. Each placement is one undo step.

## 20.8 What is not here

- No PDF renderer: vector PDFs must be exported as PNG first (20.2).
- Auto Detail draws a section's cut walls by their wall type layers; roofs, floors and ceilings that are cut come from
  the camera's drawing as lines and hatch only, with no assembly of their own. A wall cut at a steep angle to its run
  shows its apparent thickness.
- Details are not linked back: editing the model does not update a detail that was already made (make it again).
- Chief's detail windows are tabs here; a detail is not an object placed in the plan.
