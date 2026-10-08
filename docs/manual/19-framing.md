# Chapter 19: Framing

Build Framing turns the walls, floors, ceilings and roof of the plan into lumber: plates, studs, headers, joists, rafters and trusses, each a real piece
with a size and a length. The pieces are drawn in the plan, built into the 3D view, counted in the Framing Schedule and the Materials List, and kept with
the plan. This chapter covers the **Build Framing** dialog, the options that decide what a rebuild keeps, the framing layers, the Framing Overview and the
wall detail. The manual framing tools (General Framing, Post, Joist, Bearing Line and so on), the Framing Takeoff and the schedules are in 11.11; the
engine is the `plan-framing` crate.

Lengths are inches inside the program and show as feet and inches.

## 19.1 Build > Framing

| Command | What it does |
|---|---|
| **Build Framing...** | Opens the Build Framing dialog for the active floor. OK saves the options and builds, as one undo step ("Build Framing"). |
| **Build All Framing...** | The same dialog with "Build every floor" checked ("Build All Framing"). |
| **Framing Defaults...** | The Framing Defaults page (Walls, Headers, Floor, Roof) without building. |
| **Framing Overview** | Switches the plan to the Framing Overview plan view and back (19.5). |

The **Build Framing** and **Build All Framing** items of the framing flyout, and Shift+Cmd+S, build at once with the saved options and no dialog (planned: they
open the dialog like the menu items). **Delete Framing** removes the built framing of the active floor; manual members and layout lines stay.

## 19.2 The Build Framing dialog

Seven tabs, as in Chief. The first line of every tab is **Build every floor**.

**Floor** (check boxes **Build floor framing**, **Auto rebuild floor framing**, **Retain existing floor framing**)

- **Joist size** and **Joist spacing**, and **Joists run**: across the shorter side (the default), parallel to X or parallel to Y. A Joist Direction line in a room
  overrides it. A line under the size shows what that size carries at that spacing (19.4).
- **Rim joists** on or off, **Single** or **Double**; **Mid-span blocking**.
- **Joists bear on**: **Every wall bears** (each room is framed on its own) or **Exterior walls and Bearing Lines** (only the exterior walls enclose a platform and
  the Bearing Lines drawn with the Bearing Line tool split it, so joists run across interior partitions).
- **Stairwell header and trimmer plies**: a Floor Hole in the platform (a stairwell) gets trimmers beside it and headers across its ends.
- A room with no floor platform (**Open Below**, a deck) gets no floor joists.

**Ceiling**: **Build ceiling framing** is off by default. Ceiling joists rest on the top plates, with their own **Joist size**, **Joist spacing** and
**Joists run**. They are skipped for a room open to the floor above, for a room with no ceiling, and when the roof is framed with trusses or with its own ceiling joists.

**Roof**: rafter size and spacing, ridge, hip and valley and fascia sizes, tail cut (plumb, level or square; a plane's own Eave cut wins), birdsmouth seat,
collar ties, ceiling joists, and **Trusses instead of rafters**. Rafters get their tail cut and birdsmouth, and a ridge, hips and valleys where the roof has them.

**Wall**: stud size and spacing, top and bottom plates, king studs and trimmers per side, cripple spacing, corner studs, tee backing, wall blocking, and the
**header size by opening width** table (2x6 to 4', 2x8 to 5', 2x10 to 6', 2x12 beyond, as Chief's span table; a fixed depth overrides it). A 2x4 stud is
upgraded to 2x6 in walls 6" or thicker. **Retain Wall Framing** shows how many walls are retained; **Clear** releases them.

**Posts**: the size, material and footing the Post and Post with Footing tools start with. With **Footing under every new post** checked a Post becomes a Post
with Footing.

**Trusses**: the truss a Truss Base is filled with (Fink, Howe, King post, Scissor, Attic or Mono), its pitch, heel height, overhang and spacing; and for roof planes
**Trusses instead of rafters**, the roof truss spacing and "Trusses over a span of".

**Framing Defaults**: **Turn the framing layers on** after a build, and a summary of the sizes in use.

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

## 19.6 Wall detail

`framing_view::wall_detail_of` gives the framing elevation of one wall, and `paint_wall_detail` draws it with dimensions: the wall length and height, the on-centre spacing of the
common studs, and for each opening the rough opening width and height, the header height and, for a window, the sill height. (Planned: a Wall Detail window from the
wall's context menu; the dimensioned detail is not on screen yet. The 3D elevation of the Framing Overview already shows the studs.)

## 19.7 What is not done

- The Build Framing flyout item and the hotkey build without the dialog; auto rebuild is not called every frame; the Wall Specification's **Retain Wall Framing** check
  box is still dimmed (all in the integration queue).
- No Foundation framing group; no rollout or Reverse Rollout of studs; directed (Joist Direction) floors do not frame stairwell holes.
- No deck or beam calculators; the dialog layout, the checkbox wording and the plan dash patterns are marked verify in Chief (DECISIONS 112 to 115).
