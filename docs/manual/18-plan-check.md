# Chapter 18: Plan Check

**Tools > Checks > Plan Check** walks the active floor against a list of residential code and plan-hygiene rules, based on the 2021 International
Residential Code (IRC) with a few National Electrical Code (NEC) and NKBA kitchen-and-bath guidelines, and shows what it finds one item at a time. It
is a drafting aid, not a plan review: it measures what the model contains, and a clean report does not make a plan code compliant. The rules live in
the `plan-check` crate (`rules.rs`, `rules_code.rs`, `rules_fixtures.rs`, `rules_mep.rs`); the settings, the report and the window are described here.

Lengths are inches inside the engine and are shown as feet-and-inches. Rooms are measured to wall centerlines, so clear (finished) dimensions
are slightly smaller than the ones checked.

## 18.1 The Check window

**Tools > Checks > Plan Check** runs every rule that is switched on; **Door/Window Check** runs only the two opening rules (door widths and opening
geometry). **Plan Footprint** is a different command: it traces the outer boundary of the rooms and adds it to the plan as a closed CAD polyline with an area note.

The window shows one finding at a time:

- A line **Finding 3 of 11** (or "No findings") and a status line such as `Plan Check: 2 errors, 3 warnings, 6 info (4 ignored)`.
- The finding: its **severity** in color (Error, Warning or Info), the **rule with its code reference** in italics (`IRC R311.7.5.1 riser height`), what is wrong,
  a **Fix** suggestion, and where it is (`Opening 12 at 14'-6", 3'-0"`). With no findings the window says "The plan passes these checks."
- **Previous** and **Next** step through the findings, most severe first.
- **Zoom to** centers the plan on the finding (it pans; the zoom level stays) and selects its object (a wall, opening, room, stair, fixture, cabinet, roof plane or deck). With **Zoom to each finding**
  ticked (the default) Previous, Next and Ignore do this by themselves.
- **Ignore** sets the current finding aside: it leaves the list and the status line counts it as ignored. The ignore list is kept with the plan (one undo step,
  "Ignore Plan Check Finding"). **Restore Ignored (n)** appears while there are any and puts them all back.
- **Check Again** re-runs the rules after you fix something, staying near the same place in the list.
- **Settings...** (Plan Check only) opens the Plan Check Settings (18.2).

An ignored finding is identified by its floor, rule, object and place (whole inches), so it stays ignored while you leave that object alone and comes back if the object moves.

## 18.2 Plan Check Settings

**Settings...** opens a window with three parts. OK stores the settings in the plan (one undo step, "Plan Check Settings") and checks again; Cancel drops the edits.

- **Jurisdiction.** The list holds the **IRC 2021 residential** preset, the **Georgia 2020** preset (the 2018 IRC with the 2020 Georgia amendments, frost depth 12") and **Custom**. Picking a preset puts every limit and every rule back to that preset's
  starting values; the name changes to Custom by itself as soon as a limit is edited, and back to the preset's name if the limits are brought back to it.
- **Limits** (a collapsed section): 32 numbers the rules compare against. Edit any of them to match your jurisdiction.
- **The rule list**, grouped (Rooms, Doors and windows, Stairs and guards, Bath and kitchen, Garage, Roof, Electrical, Framing, Foundation, Plan geometry). Each rule has a tick box, its severity and a tooltip with what it
  checks; **All on** and **All off** act on a whole group. A rule that is switched off does not run.

The settings and the ignore list travel with the plan: they are two reserved entries of the plan's Project Information custom fields (`plancheck.settings` and `plancheck.ignored`, JSON in a string).
Nothing is stored while the settings are the preset's, so a plan that never opened the dialog is unchanged. Do not rename or edit those fields by hand. Door/Window Check applies the rules' on/off ticks and the ignore list
but uses the preset's limits.

Seven more limits are held by the jurisdiction preset and are not in the dialog's list yet: the frost depth (12"), how far the first floor sits above grade (6", an assumption because the plan has no grade of its own), the thinnest footing (6"),
the highest handrail (38"), the sphere a guard must stop (4"), the clear space in front of a shower or tub (24") and the thinnest gypsum board on the garage side of the house wall (1/2").

The 32 limits, with the 2021 IRC preset's values:

| Limit | Preset | Limit | Preset |
|---|---|---|---|
| Habitable room area | 70 sq ft | Stair width | 36" |
| Habitable room dimension | 84" | Riser height (max) | 7 3/4" |
| Ceiling height | 84" | Tread depth | 10" |
| Bath ceiling height | 80" | Stair headroom | 80" |
| Hallway width | 36" | Stair landing depth | 36" |
| Egress opening area | 5.7 sq ft | Guard required above | 30" |
| Egress opening width | 20" | Stair guard height | 34" |
| Egress opening height | 24" | Garage door to house | 32" |
| Egress sill height (max) | 44" | Openable glazing share | 0.04 |
| Entry door clear width | 32" | Toilet side clearance | 15" |
| Exterior door height | 80" | Toilet front clearance | 21" |
| Bath door width | 24" | Shower and tub side | 30" |
| Landing outside a door | 36" | Shower area | 900 sq in |
| Kitchen walkway | 36" | Kitchen work aisle | 42" |
| Counter depth | 24" | Roof pitch minimum | 2:12 |
| Roof pitch, single underlayment | 4:12 | Roof pitch, steep advisory | 12:12 |

## 18.3 The rules

64 rules. Each finding carries the code reference below as its rule name, so a report reads as a list of sections to look up. Severity is the usual one; some rules give a lower one for a milder case.

| Group | Rules (severity) |
|---|---|
| Rooms | R304.1 minimum room area (error), R304.2 minimum room dimension (error), R305.1 ceiling height (error), R311.6 hallway width (error), R311.1 means of egress (error: a room is reached through a door), R311.1 access through bathroom (warning), unnamed room (info) |
| Doors and windows | R310.2 egress (error: a bedroom needs an exterior door or an operable window with 5.7 sq ft net clear, 5.0 on the grade floor, 20" wide, 24" high, sill 44" at most; a fixed window does not count and a sliding window counts half its width), R311.2 egress door (error: the grade floor needs a side-hinged exterior door 3'-0" x 6'-8" or larger that is not a garage door), R303.3 ventilation (info), R303.1 natural light (info: glazing 8% of the floor) and ventilation area (warning: openable glazing 4%), R311.2 door width (warning), R311.2 egress door height (warning), R311.2 bedroom door swing (info), R311.3 landing at a door (warning), R312.1.1 door to a drop (error: an upper-floor exterior door must open onto a deck), R312.2 window fall protection (info) |
| Stairs and guards | R311.7.1 stair width, R311.7.2 headroom, R311.7.3 vertical rise (12'-7"), R311.7.5.1 riser height, R311.7.5.2 tread depth, R311.7.6 landings (all errors); R311.7.5 stair comfort 2R+T (info); R311.7.6 door swing over a stair (warning); R311.7.8 handrails (warning: four or more risers), R311.7.8.1 handrail height (34" to 38"), R311.7.8.2 handrail continuity (a railed flight must not run into an unrailed landing); R312.1.2 guard height (error: 34" on a stair, 36" on a landing), R312.1.3 opening limitation (error: balusters or cables that pass a 4" sphere); R312.1.1 guards on decks over 30" (error); R311.8 ramps (error) |
| Bath and kitchen | R307.1 water closet clearance, R307.1 shower entrance clearance (24" in front of a shower or tub), P2708.1 shower and tub size, R307.1 door swing into a fixture (warnings); NKBA kitchen aisle width (warning), NKBA kitchen counter depth (info) |
| Garage | R309.1 garage floor (info), R302.5.1 garage opening into a bedroom (error), garage door width (error), garage door rating (info), R302.6 garage separation (the garage side of a wall between garage and house needs 1/2" gypsum board: error when thinner, warning when another material, info when the wall has no wall type) |
| Roof | R905.2.2 roof slope (warning), R905.1.1 underlayment (info), steep slope advisory (info) |
| Electrical | R314.3 smoke alarm in each bedroom, outside each sleeping area and on every level (warnings), R315.2 carbon monoxide alarms and R315.3 CO alarm outside each sleeping area (info), NEC 210.8(A) GFCI protection (warning), NEC 210.52(A) receptacle count and E3901.2 receptacle spacing (warnings: no point along a wall space of 2' or more is farther than 6' from a receptacle; a door breaks a wall space) |
| Framing | R602.7 header size, R502.3.1 joist span, R802.4.1 rafter span (warnings; the span tables are conservative simplifications, not a structural design) |
| Foundation | R403.1.4 footing depth (warning: the underside must be at least the frost depth below grade), R403.1.1 footing size (warning: width by number of storeys, 12", 15" or 18", and 6" thick) |
| Plan geometry | Tiny wall (under 6"), dangling exterior wall, duplicate wall (warnings); opening fits and does not overlap (error) |

The footing rules read the foundation floor made by Build Foundation (walls with footings, a thickened slab edge) and the slabs, pads and piers on a floor. Because the plan has no grade of its own they assume the first floor's finished floor is
6" above grade; a footing finding says so. They give a place to zoom to but select nothing.

Some rules need the right objects to be in the plan: the electrical rules read the placed devices (chapter 9), the stair rules read the stairs of the floor (chapter 7), the roof rules the roof planes (chapter 8), the
bath and kitchen rules the fixtures, cabinets and room types, and the room rules the room names and types. Room types come from the room names, so name your rooms for the room rules to know which are bedrooms, baths and kitchens.

## 18.4 Reports

The buttons under the finding write the list as it stands (ignored findings are left out):

- **Save Report...** writes **Markdown** (`Plan Check.md`): a heading, a count line and one section per severity (Errors, Warnings, Info), each finding as the rule in bold, the message, its place and a *Fix* line.
- **Report PDF...** writes a PDF table titled "Plan Check - <floor name>" with the columns **No.**, **Severity**, **Code reference**, **Where**, **Finding** and **Fix** (`Plan Check Report.pdf`).
- **Add to Layout** adds a page called **Plan Check** to the plan's layout with the findings as a text box (one wrapped paragraph per finding), as one undo step. It needs a layout: "Make a layout first (File > New Layout),
  then add the Plan Check page". Chapter 11.3 describes pages and boxes.

The same three outputs are written for Door/Window Check.

## 18.5 Known limits

- The rules measure the model, not a code official's reading of it: tolerance, local amendments and exceptions are yours to apply (that is what the limits and the rule ticks are for).
- Only the active floor is checked each time. Run Plan Check on each floor.
- Not checked, because the model has no data for it: beam clearance (6'-8") in R305.1, 5/8" Type X on the ceiling under a habitable room above a garage, deck baluster spacing (a deck only has a Railing tick box) and the N1102 fenestration U-factor.
- A rule fires on the objects the model has: a missing smoke alarm is found because bedrooms exist and no alarm is placed, but a bedroom without a room name or type may escape the bedroom rules.
- The report has no sheet reference or date; the layout page is plain text. Format it in the layout like any text box.

## 18.6 Code minimums

The limits Plan Check checks are also the minimums the rest of the program holds your plan to. They come from the jurisdiction in **Tools > Checks > Plan Check Settings...** (IRC 2021 residential, Georgia 2020, or Custom once you edit a limit), so changing the preset changes them everywhere at once. While the settings are open the status bar names the code edition, for example `Code: IRC 2021` or `Code: IRC 2018 (Georgia)`.

**Starting values.** A new plan and File > New start from code-legal defaults: stairs at 7 1/2" risers (7 3/4" at most), 10" treads, 36" width and 6'-8" headroom; rails 36" high (a handrail 34" to 38") with openings that stop a 4" sphere; a 3-0 x 5-0 casement as the bedroom window default (net clear 15 sq ft, sill 36"); an exterior door of at least 36" x 80"; a footing as wide as the table asks for (15" for two storeys) and at least 6" thick; and a garage-separation wall type with 5/8" Type X gypsum on both faces. Values that are already legal are left alone: Daniel's 96" exterior door stays 96". The preference **Preferences > Architectural > Seed defaults from code minimums** (on by default) turns this off. **Plan Check Settings > Apply code minimums to defaults** (also Tools > Checks > Apply Code Minimums to Defaults) raises the current defaults to the minimums of the open plan; the status bar lists what moved. It is one step in the plan's Undo list, but Undo does not put the defaults back, because the defaults belong to the program rather than the plan.

**Notices under the fields.** A dialog field that is past a minimum gets an amber line under it with the code section and the limit, and a **Set to code** button that writes the limit into the field. The notice only warns: OK is never blocked, and Set to code is part of the dialog's draft, so OK is still one undo step. The notices are in:

- Staircase Specification, General: Width (R311.7.1), Tread Depth (R311.7.5.2), Riser Height (R311.7.5.1), Headroom (R311.7.2); Newels/Balusters: Clear Spacing (R312.1.3, a 4" sphere); Rails: Guard Height (R311.7.8.1, 34" to 38" for a stair rail). A landing's width has the same notice.
- Wall Specification, Rail Style (a railing wall): Railing Height (R312.1.2, 36").
- Window Specification, for a window in a bedroom (a room whose type has bed, master or nursery in it): the net clear opening (R310.2.1: 5.7 sq ft, 5.0 on the grade floor; 20" wide; 24" high; a sliding window counts half its width) and the sill (R310.2.2, 44" at most). A fixed window is flagged because it cannot be the escape opening.
- Door Specification, for an exterior door of the grade floor (not a garage door): the egress door width (a 36" leaf for 32" clear) and height (80", R311.2).
- Room Specification: the ceiling height of a habitable room, hall, bath or laundry (R305.1) on the Structure tab, and a note on General when a habitable room is smaller than 70 sq ft (R304.1).
- Build Foundation, Walls with Footings: Footing Width (Table R403.1(1)) and Footing Depth, the footing's thickness (R403.1.1 and the frost depth, R403.1.4), against the stem wall height; the footing rows of the slab and pad pages have the 6" thickness notice.
- Build Framing: a note when joist or rafter spacing is wider than the span tables cover (24") or wider than the 16" they are written for, with the table span of the size.

**Tools.** A new stair starts at the plan's minimums (a stricter riser limit in the settings gives a stricter stair). Auto Place Outlets spaces receptacles by the plan's limits (a receptacle within 6' along a wall, counter receptacles 48" apart, none for a wall space under 24").

**Fix.** In the Plan Check window, **Fix** writes the minimum into the object a stair finding names (width, headroom, tread, riser, rail height, baluster opening) or into the footings of the foundation floor (width and thickness), as one undo step (*Fix Stair to Code*, *Fix Footing to Code*). The button is greyed for findings that have no automatic fix.

**Check while drawing.** With **Tools > Checks > Check While Drawing** on (it is on by default, and a preference on the Architectural page), Plan Check re-counts after each edit the rule groups the edit touched: Stairs and guards, Doors and windows, Rooms and Foundation. The status bar shows `Live check: 2 errors, 1 warning` while there are any, and a Plan Check button on a toolbar shows the count as a red badge. The full list is still **Tools > Checks > Plan Check**.

Local amendments: the settings dialog edits the limits Plan Check has fields for. A limit with no field there (receptacle spacing, for example) can be amended in the plan file under `plancheck.minimum_overrides`, a name to number map (`CodeMinimums::numeric_keys` lists the names).
