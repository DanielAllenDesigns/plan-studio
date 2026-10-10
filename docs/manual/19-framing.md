# Chapter 19: Framing

Build Framing turns the walls, floors, ceilings and roof of the plan into lumber: plates, studs, headers, joists, rafters and trusses, each a real piece
with a size and a length. The pieces are drawn in the plan, built into the 3D view, counted in the Framing Schedule and the Materials List, and kept with
the plan. This chapter covers the **Build Framing** command, the options that decide what a rebuild keeps, the framing layers, the Framing Overview and the
wall detail. The manual framing tools (General Framing, Post, Joist, Bearing Line and so on), the Framing Takeoff and the schedules are in 11.11; the
engine is the `plan-framing` crate.

Lengths are inches inside the program and show as feet and inches.

## 19.1 Build > Framing

| Command | What it does |
|---|---|
| **Build Framing...** | Opens the Build Framing command for the active floor. OK saves the choices and builds, as one undo step ("Build Framing"). |
| **Build All Framing...** | The same with the walls and roof of every floor checked ("Build All Framing"). |
| **Framing Defaults...** | The Automatic Framing Defaults panels (19.2) without building. |
| **Truss Detail...** | The Truss Detail window (19.11). |
| **Framing Overview** | Switches the plan to the Framing Overview plan view and back (19.5). |

The **Build Framing** and **Build All Framing** items of the framing flyout, and Shift+Cmd+S, build at once with the saved options and no dialog (planned: they
open the dialog like the menu items). **Delete Framing** removes the built framing of the active floor; manual members and layout lines stay.

## 19.2 The Build Framing command and the Automatic Framing Defaults

Build Framing is a command, as in Chief. Its window has one page:

- **Automatic Framing Defaults...** switches the window to the defaults panels below; OK or Cancel returns to the command (Cancel puts the defaults back).
- **Automatically Rebuild Framing**: **Floor**, **Ceiling**, **Wall** and **Roof**. A checked part is rebuilt by itself when the walls, openings, rooms, roof planes or
  layout lines it is made from change (19.3).
- **Build Framing Once**: **Floor** and **Ceiling** each with a floor choice (**Current Floor**, **All Floors** or one floor by name), **Wall** and **Roof**. OK builds
  the checked parts once. A part that is not checked keeps the framing it has. "Build the walls and roof of every floor" is what Build All Framing checks.
- **Retain existing framing**: one box per part keeps a whole part as it is (Chief retains per object: see 19.3). **Retain Wall Framing** and the roof planes
  that retain their framing are counted, with a **Clear** button. **Turn the framing layers on** after a build.

The **Automatic Framing Defaults** panels (Default Settings > Framing opens the same ones):

**Floor Levels**: joist size, spacing and direction, rim joists (**Single** or **Double**, **Rim Joist Width**, **Rim Joist Connection** Stagger or Flush, **Max Rim
Joist Length**), mid-span blocking, **Joists bear on** (every wall, or exterior walls and bearing lines), ceiling joists, the stairwell header and trimmer plies, and
- **Joists over bearing walls and beams**: **Lap** (8" of lap, side by side, centred on the support) or **Butt** (end to end), for floors and for ceilings;
- **Blocking**: **In Line**, **Stagger** or **Cross/Bridging** for floors and ceilings;
- **Use Framing Reference** for each floor (19.8). A line under each size shows what it carries at that spacing (19.4).

**Wall**: stud size and spacing, plates, kings and trimmers, cripple spacing, corner studs, tee backing, blocking with **Stagger Blocking**, and
- **Wall connections**: corners and intersections **Standard** (three studs), **Reduced Stud** (two), **Laddered** (two and ladder blocking) or, for corners, **U Shaped**;
  **Top plate connection** Stagger or Flush;
- **Mitre ends of angled walls**: **Mitre Plate Ends**, **Rotate End Studs**, **Horizontal Frame Through**;
- **Wall Detail views**: **Build Wall Framing Details from Exterior**; studs and posts as cross boxes in plan; bearing walls get double plates and headers.

**Openings**: the **header size by opening width** table (2x6 to 4', 2x8 to 5', 2x10 to 6', 2x12 beyond; a fixed depth overrides it), the **Maximum Depth** rule (an
opening whose top is that close to the top plate gets one solid header and no cripples) and **List Cut Header Lengths**.

**Roof**: rafters, ridge, hip and valley and fascia sizes, tail cut, birdsmouth seat, collar ties, ceiling joists, **Trusses instead of rafters**, and
**Use Framing Reference** and **Trim Framing To Soffits**, **Roof Lookouts** (spacing, offset from the subfascia, gable overhang), **Hip Girder Truss** (count, distance
from the wall's main layer) and **Roof Overframing** with its Overframe Layer.

**Trusses**: the truss a Truss Base is filled with (type, pitch, heel height, overhang, spacing) and the roof truss spacing. **Posts**: the size, material and footing
the Post tools start with.

## 19.3 Build, Auto rebuild and Retain

Each group (Floor, Ceiling, Wall, Roof) has three check boxes.

- **Build**: a group that is off keeps what it has. A first build with ceiling framing off makes no ceiling joists; turning it off later leaves the ones already there.
- **Retain existing framing**: a build leaves the group's members exactly as they are, so edits survive. New members for the group are not made.
- **Auto rebuild**: when the walls, openings, rooms, roof planes or layout lines a group is made from change, the group is rebuilt by itself. Only floors that
  have been built with auto rebuild on are touched, and only the groups whose inputs changed. A retained group is not rebuilt. Auto rebuild makes no undo step of
  its own; undoing the edit undoes the rebuild. (The shell calls it once a frame in a later round; today it runs from the editor API and the tests.)

**Retain Wall Framing** works per wall: a retained wall keeps its studs, plates and headers through every build, so a stud moved or a header resized by hand
survives a rebuild of the rest. A built member you stretch with its end handle becomes a manual member and also survives. Members placed with the framing
tools are manual and are never replaced.

## 19.4 Span warnings

After a build the status line names any joists or rafters that carry more than their size allows, for example `Check spans: 16 floor joists 2x10 span up to 15' 9" (2x10 at 16"
o.c. carries 15' 7 1/4")`, and the Floor, Ceiling and Roof tabs show what each size carries. The numbers come from a beam calculation (bending and
live-load deflection, Douglas Fir-Larch No. 2, 40 psf live and 10 dead for floors, 10 and 5 for ceilings, 20 and 10 for roofs). It is a planning aid that sits a little under
the building-code span tables and ignores shear and bearing; it is not a code check and not Chief's structural calculator.

## 19.5 Layers, plan styles and the Framing Overview

- **Framing layers are off in a new plan**, like Chief's. The five manual layers (`Framing, Floor Joists`, `Framing, Rafters`, `Framing, Posts`, `Framing, Beams`,
  `Framing, Trusses`) are created hidden and turn on when you place a member on them. A build turns on the layers it used unless **Turn the framing layers on**
  is unchecked.
- **Plan line styles** of built framing follow the member kind: walls (plates, studs, headers, sills) solid and lightly filled, wall blocking lighter, floor joists and
  rims solid, ceiling joists dashed, rafters, ridge, hips, valleys, fascia and truss chords long-dashed, truss webs dotted. The dash patterns are Plan Studio's choice
  (verify in Chief).
- **Framing Overview** (Build > Framing) is a saved plan view and layer set that shows the framing alone; it builds the floor's framing first when there is none. In an
  elevation or section made in the overview, the wall skins are replaced by the wall framing.
- The **Framing Overview 3D camera** (a Perspective Overview of the framing) takes its scene from `framing_view::overview_scene`: every framing member of every
  floor and nothing else, shown even with the framing layers off. (Planned: the 3D panel's Framing Overview command calls it.)

## 19.6 Wall Details

Every wall with built framing gets a **Wall Detail**, made when the framing is built and named from the wall's label (the label typed in the Wall Specification, else
`W1`, `W2`... in wall order). A Wall Detail is a CAD detail: it opens in a tab of its own, lists in the **Project Browser** under **Wall Details** (with the floor's name when
there are several floors), takes the Text, Dimension and CAD tools, and **Send to Layout** prints it. Select a wall and click **Open Wall Detail** on the Edit toolbar.

The drawing shows each member as a box, with its label, the wall's length and height, the stud spacing, and for each opening the rough opening, header height and sill
height, and a title that names the wall and the side it is seen from (the exterior unless **Build Wall Framing Details from Exterior** is off). What you add stays when the
detail is redrawn; the members, labels and dimensions the program drew are redrawn after every build and every member edit.

Select a member's box and the Edit toolbar offers **Build Framing for Parent Object(s)** (rebuilds the wall), **Find Wall**, **Flat to Inside**, **Flat to Outside** and
**Delete Framing Member(s)**. These edit the wall's real members. Rebuilding the wall replaces them again unless **Retain Wall Framing** is on in the Wall Specification.

## 19.7 Build Framing for Selected and Parent Objects

With a wall, a room, a roof plane, a tray ceiling or a truss selected, **Build Framing for Selected Object(s)** rebuilds just that object's framing, as one undo step:

- a **wall**: its studs, plates, headers and the posts under beams that cross it;
- a **room** (its platform): the floor and ceiling joists; if other rooms of the room's Framing Group are in the same platform Chief asks whether to give the room a
  **new Framing Group**: Yes, and the platforms are separate; No, and the platform is built as it is (19.8);
- a **roof plane**: the roof framing that stands over it;
- a **tray ceiling**: its side walls and joists;
- a **truss**: its envelope and webbing are made again for where it stands (the same as **Force Truss Rebuild**).

The button is dimmed for an object that retains its framing (**Retain Wall Framing**, **Retain Floor/Ceiling Framing** on the room, **Retain Framing** on the tray ceiling or
roof plane). **Build Framing for Parent Object(s)** does the same from a member: the wall of a Wall Detail's member, the platform of a joist, the roof plane of a rafter, a truss.

## 19.8 Framing Groups, bearing walls and beams, the Framing Reference, joins

- **Framing Group** (Room Specification): when only the exterior walls bear, rooms of different groups are separate platforms, so floors can be built at different times.
- **Bearing Wall** (Wall Specification) and **Bearing Beam** (Framing Specification): in that mode joists run across them and lap or butt over them (19.2). A beam standing
  1" or more above the joists holds them by its sides. Without a Joist Direction line, joists run across the longest bearing wall or beam. A **Bearing Line** drawn with
  the tool splits the joists with a gap and gets a beam of its own. Posts are placed under a Floor/Ceiling Beam where it crosses a wall.
- **Framing Reference Marker**: each floor uses its own first marker, else the first floor's. With **Use Framing Reference** on (per floor; **Roof** for rafters), walls,
  joists, ceiling joists and rafters start at the marker: a stud or joist is centred on it plus whole spacings. **Move to Framing Ref** moves the selected parallel
  manual members so the first lies on that grid. Deck joists take the marker when the deck builder passes it (integration queue).
- **Joist Direction** and **Roof Truss Direction** lines have a Specification (double-click): Construction, Depth, Width and Spacing for joists; Truss Spacing, chord and web
  depths, Maximum Horizontal Span and Require Kingpost for trusses.
- **Join and Lap Ends** / **Join and Mitre Ends** (two selected members), **Add Break** (one member, at its middle) and the **Rotate** choice (Flat to Inside or Outside)
  of a General Framing member change manual members. **End Profile** gives each end of a joist, beam, rafter or General Framing member a shape and size.

## 19.9 Display

Studs, kings, trimmers, cripples and posts draw as **cross boxes** in plan (a box with an X; **Show Cross** on a post turns it off). The Framing Specification has **Fill
Style** (plan and Wall Detail) and **Label** panels: an automatic label or a specified one with **Insert Macro** (`%nominal_size%`, `%size%`, `%width%`, `%depth%`,
`%length%`, `%type%`...), drawn on the `Framing, Labels` layer. A selected line member shows an **S** and an **E** at its ends. In a section made into a detail, cut lumber is
a box with an X and cut blocking a box with one diagonal.

## 19.10 Truss specification

A truss opens the **Roof Truss**, **Girder Truss** or **Floor/Ceiling Truss Specification**. Roof trusses: Truss Type (including Double Fink and Double Howe), pitch, heel
height and overhang; **Member Sizing** (top chord, bottom chord, webbing; plies for a girder); **Maximum Horizontal Span** (a smaller span gives a finer web);
**Horizontal Blocking** (vertical spacing, rollout offset with Automatic); **Roof Directives** (Require Kingpost, End Truss, Energy Heel, Drop Hip Truss, Reduced Gable,
Attic Truss, Sloping Flat Truss); **Options** (Automatically Generated Truss, Force Truss Rebuild, Lock Truss Envelope and Webbing, Use Special Snapping, Calculate
Chords/Webbing in Materials List, Show Multi-Ply Lines). Floor and ceiling trusses have member depths, thickness, maximum span, blocking and Vertical Supports. A moved
roof truss takes the pitch of the roof plane where it lands unless it is locked. Trusses are for illustration; have an engineer approve every design.

## 19.11 Truss labels, the Truss Detail and the schedule

Trusses are labelled `TR-1`, `TR-2`... (roof and girder trusses) and `FTR-1`... (floor and ceiling), numbered by the order each distinct configuration first appears; trusses with
the same configuration share a label. Labels show in plan on the `Framing, Roof Truss Labels` and `Framing, Floor/Ceiling Truss Labels` layers. The **Truss Detail** (a CAD
detail, made with the first truss) draws each configuration once with its web layout, the label and, when several share it, the quantity in parentheses (`TR-1 (3)`).
**Open Truss Detail** (a selected truss) opens it on that diagram; **Find Trusses** (a selected diagram) goes to the plan and selects the trusses. The **Truss Detail
window** (Build > Framing) lists the configurations with those buttons and **Force Truss Rebuild**. `plan_framing::truss_schedule` gives the truss rows (label,
quantity, type, span, pitch, plies, members) of the framing schedule.

## 19.12 What is not done

- The Build Framing flyout item and the hotkey build without the dialog; auto rebuild is not called every frame (integration queue).
- No Foundation framing group; no rollout or Reverse Rollout of studs; directed (Joist Direction) floors do not frame stairwell holes or rim joists.
- Jack, hip, girder and subgirder trusses and the Truss Base specification are Round 17. The Truss Detail is not linked back to the trusses (deleting a drawing deletes
  nothing). Trim Framing To Soffits and End Profile do not change the 3D shape yet. Posts under beams do not cut the top plates.
- The dialog layout, the checkbox wording, the plan dash patterns and the Double Fink and Double Howe webs are marked verify in Chief (DECISIONS 112 to 115, FL1 to FL16).

## 19.13 Framing Member Defaults, Framing Types and Structural Member Reporting

Build > Framing now lists Automatic Framing Defaults, Manual Framing Defaults, Framing Member Defaults, Framing Types and Structural Member Reporting. They are stored with the plan (the catalogue sits in the first floor's framing slot), and each OK is one undo step.

- **Framing Member Defaults** name a type, a material and a Role (Floor Joist, Header, Plate, Rafter and so on) and never a size. The list shows an In Use column and offers Edit (several at once), Copy, Rename, Delete (refused while in use), Merge, Purge and Apply to selected members. Applying a default to hand-drawn members changes their type, Role and material and keeps their size and place.
- **Framing Types** are Wood (lumber, I-joist, glulam, engineered lumber, LVL, PSL, VSL), Steel (I, box, C, U channel; Steel C names a supporting U channel type), Concrete and Other. A type's shape is stamped on every automatic member, so editing Lumber to I-Joist redraws the floor joists at once. Display Nominal Sizes and Include Name in Labels decide how the framing schedule and Materials List describe it.
- **Automatic Framing Defaults** has panels for Foundation, the floor levels, Deck, Deck Support, Wall, Openings (header sizes by opening width, List Cut Header Lengths), Fireplaces, Roof and Trusses. **Manual Framing Defaults** shape new general framing, beams (With Joists or Under Joists, Align Exterior), posts and posts with footings.
- **Structural Member Reporting** counts framing as a Buy List (boards from the Board Sizes table, in priority order, with the saw kerf), a Cut List (one line per cut), Linear Length, or Mixed (studs counted, plates and headers in feet; one Mixed per plan). Saved defaults can be edited, copied or converted, renamed, deleted, imported and exported. The dialog's totals come from the same members as the framing takeoff, so a Cut List's feet equal the takeoff's.

Not done: the Materials List still counts framing with its own Buy List / Cut List / Linear Feet switch rather than the active reporting default; the Default Settings tree still opens the saved-defaults list for Framing Types and Structural Member Reporting; the Build Framing dialog's own Automatic Framing Defaults button and the tool double-click do not open the new dialogs yet (docs/integration-queue.md).
