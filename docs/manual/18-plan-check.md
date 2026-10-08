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

- **Jurisdiction.** The list holds the **IRC 2021 residential** preset and **Custom**. Picking the preset puts every limit and every rule back to its starting value; the name changes to Custom by
  itself as soon as a limit is edited.
- **Limits** (a collapsed section): 32 numbers the rules compare against. Edit any of them to match your jurisdiction.
- **The rule list**, grouped (Rooms, Doors and windows, Stairs and guards, Bath and kitchen, Garage, Roof, Electrical, Framing, Plan geometry). Each rule has a tick box, its severity and a tooltip with what it
  checks; **All on** and **All off** act on a group. A rule that is switched off does not run.

The settings and the ignore list travel with the plan: they are two reserved entries of the plan's Project Information custom fields (`plancheck.settings` and `plancheck.ignored`, JSON in a string).
Nothing is stored while the settings are the preset's, so a plan that never opened the dialog is unchanged. Do not rename or edit those fields by hand. Door/Window Check applies the rules' on/off ticks and the ignore list
but uses the preset's limits.

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

52 rules. Each finding carries the code reference below as its rule name, so a report reads as a list of sections to look up. Severity is the usual one; some rules give a lower one for a milder case.

| Group | Rules (severity) |
|---|---|
| Rooms | R304.1 minimum room area (error), R304.2 minimum room dimension (error), R305.1 ceiling height (error), R311.6 hallway width (error), R311.1 means of egress (error: a room is reached through a door), R311.1 access through bathroom (warning), unnamed room (info) |
| Doors and windows | R310.2 egress (error: a bedroom egress window or exterior door), R303.3 ventilation (info), R303.1 natural light (info: glazing 8% of the floor) and ventilation area (warning: openable glazing 4%), R311.2 door width (warning), R311.2 egress door height (warning), R311.2 bedroom door swing (info), R311.3 landing at a door (warning), R312.1.1 door to a drop (error: an upper-floor exterior door must open onto a deck), R312.2 window fall protection (info) |
| Stairs and guards | R311.7.1 stair width, R311.7.2 headroom, R311.7.3 vertical rise (12'-7"), R311.7.5.1 riser height, R311.7.5.2 tread depth, R311.7.6 landings (all errors); R311.7.5 stair comfort 2R+T (info); R311.7.6 door swing over a stair (warning); R311.7.8 handrails (warning: four or more risers); R312.1.2 guard height (error); R312.1.1 guards on decks over 30" (error); R311.8 ramps (error) |
| Bath and kitchen | R307.1 water closet clearance, P2708.1 shower and tub size, R307.1 door swing into a fixture (warnings); NKBA kitchen aisle width (warning), NKBA kitchen counter depth (info) |
| Garage | R309.1 garage floor (info), R302.5.1 garage opening into a bedroom (error), garage door width (error), garage door rating (info) |
| Roof | R905.2.2 roof slope (warning), R905.1.1 underlayment (info), steep slope advisory (info) |
| Electrical | R314.3 smoke alarm in each bedroom and on every level (warnings), R315.2 carbon monoxide alarms (info), NEC 210.8(A) GFCI protection (warning), NEC 210.52(A) receptacle spacing (warning) |
| Framing | R602.7 header size, R502.3.1 joist span (warnings) |
| Plan geometry | Tiny wall (under 6"), dangling exterior wall, duplicate wall (warnings); opening fits and does not overlap (error) |

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
- A rule fires on the objects the model has: a missing smoke alarm is found because bedrooms exist and no alarm is placed, but a bedroom without a room name or type may escape the bedroom rules.
- The report has no sheet reference or date; the layout page is plain text. Format it in the layout like any text box.
