# Chief Architect X18 — specification dialogs

Captured 2026-10-07 from the *Default Settings* dialog (Edit ▸ Default
Settings…), which opens the same multi-tab dialog an object shows on
double-click. Every Chief dialog shares one frame: a **tab list down the left
edge**, the active **tab's panel in the middle**, a **live 3D/plan preview on
the right** with view-mode buttons above it, and **OK / Cancel / Help** along
the bottom. Disabled controls stay visible but grayed, so the full option set
is always discoverable. Plan Studio reproduces this frame (`dialogs.rs`) and
fills tabs in as features land.

## Default Settings tree (top level)

3D Solid · 3D View Defaults · Cabinets · CAD · Camera Tools · Corner Trim ·
Default Sets · Dimension · Distributed Objects · Doors · Dormer · Electrical ·
Floors and Rooms · Foundation · Framing · General · Image · Layout · Materials
· Plan · Railing and Deck · Roofs · Schedules · Slab · Stairs · Terrain ·
Text · Walls · Windows (a search field filters the tree).

## Wall Specification (Walls ▸ Exterior Wall Defaults)

Tabs: **General · Structure · Roof · Foundation · Wall Types · Wall Cap ·
Wall Covering · Rail Style · Newels/Balusters · Rails · Layer · Materials ·
Label · Components · Object Information · Schedule**

### General
- *General*: ☐ Foundation Wall · ☐ Railing · ☐ Terrain Retaining Wall ·
  ☐ Attic Wall · Thickness `7 5/8"` · Wall Length (per-object) · Wall Angle ·
  Lock: ◉ Start ○ Center ○ End
- *Options*: ☐ Invisible · ☐ No Room Definition · ☐ No Locate · ☐ Lock
  Center · ☐ No Room Moldings Exterior · ☐ No Room Moldings Interior ·
  ☐ Automatically Generated Wall · ☐ Ignored by Hide Exterior Walls
- *Curved Wall*: Radius to ○ Outer Surface ○ Main Layer Outside ○ Main Layer
  Inside ○ Inner Surface · Radius · Lock ○ Arc Center ○ Ends · Facet Angle
  `7.5°` · ☑ Automatic Facet Angle

### Structure
- *Default Wall Heights*: ☑ Default Wall Top Height · ☑ Default Wall Bottom
  Height
- *Platform Intersections*: Invisible Walls and Railings ☑ Generate Between
  Platforms · Ceiling Platform ◉ Automatic ○ Stop at Ceiling Above ○ Balloon
  Through Ceiling Above ○ Hang Floor Platform Above on Wall (☐ Subflooring to
  Wall Interior, ☐ Include Ledger) · Floor Platform ◉ Automatic ○ Stop at
  Floor Below ○ Balloon/Extend Through Floor Below
- *Wall Intersections*: ☐ Through Wall At End · ☐ Through Wall At Start
- *Rim Joist*: ◉ Automatic ○ Double ○ Single
- *Double Wall*: ○ Furred Wall ○ Split Framing ◉ Frame Through
- *Stud Layout*: ☑ Use Framing Reference · ☐ Reverse Stud Rollout Direction ·
  Stud Rollout Offset `0"`
- *Framing*: ☐ Retain Wall Framing · ☐ Bearing Wall · ☑ Stagger Multiple
  Framing Layers · ☐ Create Wall/Footing Below · ☑ Insert Floor Framing Below
  · ☑ View Wall Detail from Exterior

### Roof
- *Roof Options*: ◉ Hip Wall ○ Full Gable Wall ○ Dutch Gable Wall ○ High
  Shed/Gable Wall ○ Knee Wall ○ Extend Slope Downward · ☐ Roof Cuts Wall at
  Bottom · ☑ Include Frieze · ☐ Include Automatic End Truss Above
- *Pitch Options*: Pitch `8"` in 12 (with "use default" wrench) · ☐ Upper
  Pitch `6"` in 12 · Starts at Height `138 9/16"` · In from Baseline `15"`
- *Overhang*: Length `16"`
- ☐ *Auto Roof Return*: Length `36"` · Extend `0"` · Roof Type ◉ Gable ○ Hip
  ○ Full · Slope ◉ Sloping ○ Flat · ☑ Include Shadow Boards · ☐ Include
  Ridge Caps · ☐ Include Frieze · ☐ Include Gutter
- ☑ *Lower Wall Type if Split by Butting Roof*: [Interior-6 ▾]
- ☐ *Treat As Part Of Bay/Box/Bow Window*: ☐ Use Existing Roof · ☐ Extend
  Existing Roof Over

### Foundation
- *Foundation*: ☐ Foundation Wall · ☐ Slab Footing · Wall Thickness `7 5/8"`
- ☐ *Footing*: Width `24"` · Height `12"` · ☑ Automatic Footing Bottom Height
  · ☐ Vertical Footing · Footing Offset `0"` · ☐ Center Footing on Main Layer
  · ☐ Align Footing on Outside · fill-style swatch · Fill Style… · Layer
  ☑ Default [Footings ▾] Define…
- *Slab*: ☑ Add Chamfer on Monolithic Slab · ☐ Add Chamfer on Regular Slab ·
  Chamfer Width `4"` · Chamfer Height `4"` · Monolithic Slab Pour Number `1`
- ☑ *Sill Plate*: Construction [Sill Plate ▾] Define…

### Wall Types
- *General*: Wall Type [Stucco-6 ▾] Define… Library… · layer-stack preview
  strip · Layer ☑ Default [Walls, Normal ▾] Define…
- ☐ *Pony Wall*: Lower Wall Type [stone-6 ▾] · preview · Layer · Elevation of
  Lower Wall Top `24"` · Height Off Floor `24"` · Align Pony Wall at ○ Outer
  Surface ◉ Main Layer Outside ○ Wall Center ○ Main Layer Inside ○ Inner
  Surface · Display in Plan View ○ Use Default (Upper Wall) ○ Upper Wall
  ○ Lower Wall ◉ Upper Wall and Lower Wall Outline ○ Upper Wall Outline and
  Lower Wall ○ Upper Wall and Lower Wall

### Wall Cap
- *Wall Cap Profile* table (Name · Width · Height · Repeat Distance · Horiz.
  Offset · Vertical Offset) with Add New… · Replace… · Default · Delete · Add
  to Library · ☐ Retain Aspect Ratio · ☐ Full Wall Width · ☐ Split Pony Wall
- *Selected Profile Options*: Vertical Position [Under Polyline ▾] ·
  Horizontal Position [Inside Wall ▾] · Profile Rotation `0.0°` · Reflect
  Horizontal / Reflect Vertical · ☐ Texture Up Direction Is Along Molding ·
  ☐ Count Components in Materials List · profile preview · ☑ Show Position
  Indicator

### Wall Covering
- *Materials*: Wall Covering [▾] · Add New… Replace… Delete · texture and
  pattern swatches
- *Position*: Top To Ceiling `0"` · Height `0"` · Floor To Bottom `0"` · Wall
  Side ☐ Interior ☐ Exterior
- *Options*: ☐ No Room Wall Coverings

### Layer
- Layer ☑ Default [Walls, Normal ▾] Define… · Drawing Group [Default: 29 –
  Wall ▾]

### Materials
- Component tree (Wall ▸ Exterior Wall Surface · Interior Wall Surface · Sill
  Plate) with Material column (`Default: Sand Finish – Eggshell`, `Default:
  Drywall`, `Default: Fir Framing`) · pattern + texture swatches · Select
  Material…

### Label
- *Display Options*: ☐ Suppress Label in All Views · ☑ Display in Plan View
- *Label Content*: ◉ Automatic Label ○ Specify Label (text box, Insert macro
  ▾) · ☑ Use Default Formatting
- *Appearance*: ☐ Display Border · Fill Style swatch Fill Style… · Text Style
  [Use Layer Text Style ▾] Define… · Alignment [Left ▾] · Margins Left
  `3/4"` Right `3/4"` Top `1/8"` Bottom `3/8"` · ☑ Auto Adjust Text Direction
- *Plan View Position and Orientation*: Angle `0.0°` ◉ Relative Angle
  ○ Absolute Angle · Position Offset X `0"` Y `0"`
- *Label Layer*: ◉ Use System Layer (Walls, Labels) ○ Use Object Layer
  (Walls, Normal) ○ Use Custom Layer [▾] Define…

### Components
- Toolbar (+ add, ✕ delete, ↺ reset, ⚙) · left tree of components (Wall ▸
  Wall – Main Part ▸ Sand Finish – Eggshell, Housewrap, OSB-Hrz, Drywall) ·
  right table of Formula/Value rows: ID, Sub Category, Supplier, Manufacturer,
  Code, Size, Description, Count, Extra, Price, % Markup, Labor, Equipment,
  Total Cost, Comment, Label, Accounting Code (e.g. `3400 – Exterior siding`)

### Object Information
- Code · Comment · Description (`%automatic_description%`) · Manufacturer ·
  Supplier (each with an Insert macro ▾) · Custom Object Fields (Custom
  Fields / Field Value lists)

### Schedule
- ☑ Include in Schedule · ☑ Show Schedule Callout · Callout Location
  Rotation `0.0°` · ◉ Auto Schedule Category (Wall – Stucco-6) ○ Include in
  Schedule As: (checkbox tree of every schedule category: 3D Solids,
  Backsplashes, Cabinet, Cabinet Accessories, Ceiling Planes, Countertops,
  Door, Electrical, Fixture, Framing Deck/Floor-Ceiling/General/Roof/Truss/
  Wall, Furniture, Geometric Shapes, Hardware, Material Regions, Materials
  List Polylines, Millwork, Molding, Note, Piers/Pads, Plant, Polylines, Roof
  Planes, Roof Trim, Room, Slabs, Sprinkler, Terrain, Wall, Window …)

### Plan Studio mapping (Wall)
| Chief tab | Plan Studio phase |
|---|---|
| General (thickness, length, angle, lock, invisible/no-room-definition) | Phase 1 |
| Wall Types (layer stack, pony wall) | Phase 1 (layer stack) / Phase 2 (pony) |
| Structure (heights, platform intersections, framing) | Phase 2–3 |
| Roof | Phase 3 |
| Foundation | Phase 3 |
| Layer, Label | Phase 1 |
| Materials | Phase 2 |
| Wall Cap, Wall Covering, Rail Style, Newels/Balusters, Rails | Phase 2–3 |
| Components, Object Information, Schedule | Phase 4 |

## Door Specification (Doors ▸ Interior Door Defaults)

Tabs: **General · Options · Casing · Lintel · Sill/Threshold · Lites · Jamb ·
Arch · Hardware · Shutters · Opening Indicators · Rough Opening · Framing ·
Energy Values · Layer · Materials · Label · Components · Object Information ·
Schedule**

### General
- *General*: Door Style [Door P04 ▾] Library… · Door Type [Hinged ▾] (set by
  the tool) · Swing Angle `90°`
- *Size and Position*: Width `30"` · Height `96"` · Thickness `1 3/8"` ·
  Elevation Reference [From Floor ▾] · Floor to Top `96"` · Floor to Bottom
  `0"`
- *Panel*: Height `95 15/16"` ☑ Automatic · Bottom Offset `1/16"` ☑ From
  Floor Finish · Panel Offset `0"` · ☐ Use Clearance Gaps
- *Barn Doors*: Side Overhang · Top Overhang
- *Panel Frame Widths*: Width `4"` ☑ Uniform · Left/Right/Top `4"` …

### Options
- *Door Panels*: ○ Single Door Only ○ Double Door Only ◉ Calculate from Width
  ○ Custom (Left/Right counts) · Garage Door Vertical Panels `4` · ☐ All Glass
- *Plan Display*: ◉ Automatic ○ Show Top Edge ○ Hide Top Edge
- *Open/Close Display*: ☑ Show Open in 2D · ☐ Show Open in 3D
- *Opaque Glass*: ◉ Automatic ○ Opaque ○ Transparent
- *Door Swing*: ○ Both Doors Swing ○ Left Swing Only ○ Right Swing Only ·
  ☐ Swings from Center · ☑ Swings Both Directions
- *Safety*: ☐ Tempered Glass · ☐ Fire Door
- *Recessed into Wall*: ☑ Recessed To Layer [1 – Drywall ▾]
- *In Curved Wall*: ◉ Straight Door ○ Curved Door
- *Plinth Blocks*: ☐ Interior Plinth Block · ☐ Exterior Plinth Block

### Casing
- ☑ *Use Interior Casing*: Casing Profile Library… Clear · Width `3 1/2"` ·
  Depth `3/4"` · Reveal `1/4"` · profile preview "Default"
- ☑ *Use Exterior Casing* (grayed for interior door): Width `3 1/4"` · Depth
  `1"` · Reveal `1/4"`
- *Double Wall Options*: ◉ Through ○ Enlarged ○ Double ○ Not Through
- *Curved Wall Casing*: ○ Straight ◉ Radial ○ Parallel

### Lintel
- ☐ Use Interior Lintel / ☐ Use Exterior Lintel: Lintel Profile Library…
  Clear · Width `3 1/4"` · Extend `0"` · ☐ Wrap · preview

### Sill/Threshold
- ☐ *Use Threshold*: Interior Depth `0"` · Exterior Depth `1/4"`
- ☐ *Use Interior Sill* / ☐ *Use Exterior Sill*: Sill Profile Library… Clear ·
  Extend `0"` · Inset `0"` · ☐ Wrap · ☑ Apron · preview

### Lites
- *Lites*: Type [Normal ▾] · Lites Across `1` · Lites Vertical `1` · Muntin
  Width `7/8"`
- *Round Top Arch*: Ray Count `0` · ☐ Concentric

### Jamb
- ☑ *Has Jamb*: Positioning ○ Door Size Includes Jamb ◉ Door Size Excludes
  Jamb · Sides Width `3/4"` · Top Width `3/4"` · ☑ Fit Jamb to Wall · Depth
  `6"` · Inset `0"`

### Arch
- *Arch*: Type [No Arch ▾] · Height `0"` · Radius `5/8"`
- *Options*: ☐ Reflect Vertically · ◉ Full Arch ○ Left Arch ○ Right Arch

### Hardware
- *Handles*: Interior Handle [Lever ▾] Library… Edit… · Exterior Handle
  [Lever ▾] · In from Door Edge `3"` · Up from Bottom `36"`
- *Locks*: Interior Lock [None ▾] · Exterior Lock [None ▾] · Up from Bottom
  `42"`
- *Hinges*: Hinges [Standard ▾] Library… · In from Top/Bottom `7"` · Number
  of Hinges `3`
- *Sliding Tracks*: Sliding Track [None ▾] · Height Above Door · Hangers
  [None ▾] · In from Sides `3"`

### Opening Indicators
- Default Visibility ☑ Show

### Rough Opening
- Total Width `32"` · Total Height `98 1/2"` · Header Bottom Height
  `98 1/2"` · ◉ Additional Space (Additional Width `2"`, Additional Height
  `2 1/2"`) ○ Clearance Gap (Side `1/4"`, Top `1/2"`, Bottom `0"`)
- *Add for Concrete Cutout*: Each Side `4"` · Plan Display ☑ Show In Floor
  Below

### Framing
- *Header*: ☑ Include Header · Construction [Wall Header – Lumber ▾] Define…
  · ☑ Determine Framing Type from Wall Layer · Framing Method [Standard ▾] ·
  Count `2` · Thickness `1 1/2"` · Depth `11 1/4"` ☑ Calculate from Width ·
  ☑ Evenly Spaced · Spacing
- *Header Placement*: Depth ◉ Flush Against Exterior Edge ○ Flush Against
  Interior Edge · Vertical ◉ Top of Opening ○ Top of Wall
- *Adjacent Openings*: ☐ Combine Headers · Max Combine Distance `3"`
- *Supports*: Trimmer Construction [Match Stud Default ▾] · Trimmer Count `1`
  · King Stud Construction · King Stud Count `1`
- *Sills*: Sill Construction [Match Stud Default ▾] …

### Energy Values
- Assembly: Door Type [Solid (under 50 % glazing) ▾] · U-Factor `0.3` · SHGC
  `0.3`

### Label (door-specific additions)
- Automatic Label size format ○ Height/Width ◉ Width/Height ○ Width Only ·
  Additional Text ☑ Include Schedule Number ☑ Include Type · margins Left
  `1"` Right `1"` Top `3/16"` Bottom `1/2"` · Label Layer (Doors, Labels)

### Plan Studio mapping (Door)
General size/position + swing, Options (swing, show open in 2D), Casing
widths, Jamb, Label → Phase 1. Lites, Arch, Hardware, Sill, Lintel → Phase 2
(3D). Rough Opening, Framing, Energy Values → Phase 3. Rest → Phase 4.

## Window Specification (Window Defaults)

Tabs: **General · Options · Casing · Lintel · Sill/Threshold · Sash · Frame ·
Lites · Shape · Arch · Treatments · Shutters · Opening Indicators · Rough
Opening · Framing · Energy Values · Layer · Materials · Label · Components ·
Object Information · Schedule**

### General
- *General*: Window Type [Single Casement ▾] · Swing Angle `90°`
- *Size and Position*: Width `32"` · Height `72"` · Elevation Reference [From
  Floor ▾] · Floor to Top `96"` · Floor to Bottom `24"`
- *Component Options* (mulled units): Component Size · hinge grid Left /
  Center / Right × Fixed / Left Hinge / Right Hinge
- *Options*: Louver Size `1"` · Minimum Separation `2"`

### Options
- *Options*: ☐ Interior Corner Block · ☐ Exterior Corner Block · ☑ Egress ·
  ☑ Tempered Glass
- *Display*: ☐ Show Open in 2D · ☐ Show Open in 3D
- *Recessed into Wall*: ☑ Recessed To Layer [3 – OSB-Hrz ▾]

### Casing / Lintel / Sill/Threshold
Same controls as the door dialog (interior and exterior casing profile,
width, depth, reveal; lintel; sill with apron).

### Sash
- ☑ *Has Sash*: Side Width `1 1/2"` · Middle Width `1 1/2"` · Top Width
  `1 1/2"` · Bottom Width `2 1/2"` · Depth `1 1/2"` · Inset `1 1/2"` · Curved
  Options ◉ Straight ○ Curved

### Frame
- ☑ *Has Frame*: Positioning ◉ Window Size Includes Frame ○ Window Size
  Excludes Frame · Sides Width `3/4"` · Top Width `3/4"` · Bottom Width
  `3/4"` · ☑ Fit Frame to Wall · Depth `6"` · Inset `0"`
- *Options*: Corner Join ◉ Post ○ Mitered

### Lites
- Type [Normal ▾] · Lites Across `1` · Lites Vertical `1` · Muntin Width
  `7/8"` · ☑ Lites in Fixed · ☑ Lites in Movable · ☐ Muntin in Corner · ☑ Auto
  Adjust Lites for Component Size · Round Top Arch: Ray Count, ☐ Concentric

### Shape
- "Window width is: 32"" · Revert All · *Sides* Height Left `72"` Right `72"`
  · *Top Inside Corners* ☐ Left ☐ Right (Height, Offset `10 11/16"`) ·
  *Bottom Corners* ☐ Left ☐ Right (Height `0"`)

### Treatments
- *Curtains*: Style [None ▾] Library… · Height Off Floor `18"` · Height Above
  Casing `2"`
- *Blinds*: Style [None ▾] Library…
- *Exterior Millwork Above Casing*: Style [None ▾] · Height `12"` · Width
  `38"`
- *Exterior Millwork Below Casing*: Style [None ▾] · Height `12"` · Extend `0"`

### Shutters
- Type [None ▾] Library… · *Size* ☑ Match Opening Width (Width `16"`) ·
  ☑ Match Opening Height (Height `72"`) · *Position* ◉ On Casing ○ Outside
  Casing ○ Custom (Offset from Side `1"`, Offset from Bottom `0"`) · *Options*
  Louver Size `1"` · ☐ Reverse Direction · ☑ Match Arch · ☐ Show Closed ·
  *Sides* ◉ Automatic ○ Left Side Only ○ Both Sides ○ Right Side Only

### Label
Same as door: size format Width/Height, ☑ Include Schedule Number, ☑ Include
Type; label layer "Windows, Labels"; default position offset Y `-6"`.

### Plan Studio mapping (Window)
General, Options (egress, tempered), Frame, Label → Phase 1. Sash, Lites,
Shape, Treatments, Shutters → Phase 2 (3D). Framing, Energy → Phase 3.

## Room Types and Room Specification (Floors and Rooms ▸ Room Types)

*Room Types* is a list dialog: Available Room Types table (Room Types · In
Use ✓) with Edit… · Copy… · Rename… · Delete · Select All · Clear All ·
Help · Cancel · OK. Built-in types seen: Bedroom #5, Bonus Room, Closet,
Courtyard, Crawl Space, Deck, Dinette, Dining, Dining Room, Dressing Room,
Entry, Family Room, Flat Roof, … (the list continues alphabetically through
Garage, Kitchen, Living, Master Bath, Porch, Utility and so on).

*Room Type Defaults* (Edit… on a type) tabs: **General · Structure · Moldings
· Layer · Fill Style · Materials · Label · Components**. The preview is a
live cross-section showing CEILING `121 1/8"` and FLOOR `0"` with the floor
and ceiling platform layer stack dimensioned (5 1/2", 5/8", 116 3/8", 4 1/8",
16 3/4").

### General
- Room Name `Flat Roof` · Function [Utility ▾] (Standard, Living, Utility,
  Deck, Garage, Porch, Open Below …)
- *Living Area*: ○ Include in Total Living Area Calculation ◉ Exclude from
  Total Living Area Calculation ○ Use Default (Included)
- *Conditioned Room*: ○ Conditioned ◉ Unconditioned ○ Use Default
  (Conditioned)

### Structure (per-room version)
Absolute/relative Floor and Ceiling heights, Rough Ceiling, Stem Wall, Floor
and Ceiling Finish thickness, ☑ Floor Under This Room / ☑ Ceiling Over This
Room / ☑ Roof Over This Room, Floor and Ceiling Structure Define… buttons,
Monolithic Slab Foundation, Flat Ceiling / Sloped options. (Tab names seen;
field detail to be verified in a per-room Room Specification capture.)

### Label
- Display in All Views: ☑ Interior Dimensions · ☑ Interior Area · ☑ Standard
  Area · ☑ Display in Plan View
- Appearance: Text Style [Use Layer Text Style ▾] Define…

### Components
Same materials-list tree as walls, scoped to the room's floor and ceiling
finish layers (Shasta White, Foam Underlayment, Drywall, Color – Bone, base
molding), with the note "Values are calculated based on the room displayed in
the preview (interior area = 100 sq ft)".

### Plan Studio mapping (Room)
Room name + room type list + living-area flag + area label options → Phase 1
(room labels already exist). Structure heights/finishes → Phase 2. Moldings,
Fill Style, Materials → Phase 2–3.

## Cabinet Specification (Cabinets ▸ Base Cabinet Defaults)

Tabs: **General · Box Construction · Front/Sides/Back · Door/Drawer ·
Accessories · Opening Indicators · Moldings · Layer · Fill Style · Materials ·
Label · Components · Object Information · Schedule**. Preview: 3D cabinet
with view buttons (plan, front, side, perspective, etc.).

### General
- *Cabinet Style*: Type [Standard ▾] · ☐ Treat As Filler
- *Size/Position*: Width `24"` · Height `36"` ("Including Countertop") ·
  Depth `24"` · Elevation Reference [From Finished Floor ▾] · Finished Floor
  to Top `36"` · Finished Floor to Bottom `0"`
- ☑ *Countertop*: Thickness `1 1/2"` · Overhang `1"` ☐ Uniform · Front `1"`
  Back `1"` Left `1"` Right `1"` · Corner Treatment ◉ None ○ Clipped
  ○ Rounded · Corner Clip/Radius `3/8"` · ☑ Include Countertop in Schedule ·
  ☐ Display Molding Edges in Plan Views
- ☐ *Backsplash*: Height `0"` · Thickness `1/2"` · Options ☐ Side ☐ Always
  Present · ☑ Include Backsplash in Schedule
- ☑ *Toe Kick*: Height `4"` · Depth `3"` (continues below the fold)

### Box Construction
- *Box Construction*: ◉ Framed (Separation `1 1/2"` · Left Stile Extend `0"`
  · Right Stile Extend `0"`) ○ Frameless
- *Top/Bottom/Sides*: Top ◉ Auto ○ Has Top ○ No Top · Bottom ◉ Auto ○ Has
  Bottom ○ No Bottom · Side Thickness `3/4"` · Back Thickness `3/4"`
- *Door/Drawer Overlay*: ○ Traditional Overlay (Overlap `3/8"`) ◉ Full
  Overlay (Reveal `1/16"`) ○ Inset (Clearance `1/16"`)
- *Cabinet Corner Treatment*: ◉ None ○ Clipped ○ Rounded · Corner
  Clip/Radius · ☑ Automatic Placement ☐ Back Left ☐ Back Right ☐ Front Left
  ☐ Front Right

### Front/Sides/Back
- *Cabinet Side*: Side [Front ▾] · Side Type [Custom Face ▾]
- *Face Items*: tree "Vertical Layout Parent ▸ 1 Separation – Horizontal ·
  2 Layout – Horizontal ▸ 2.1 Drawer (Default: Lincoln Flat Panel Drawer) ·
  3 Separation – Horizontal · 4 Door – Auto Right (Default: Lincoln Door) ·
  5 Separation – Horizontal" with Add New… · Delete · Move Up · Move Down ·
  Split Vertical · Split Horizontal · Equalize
- *Selected Item Properties*: Item Type [Layout – Vertical ▾] · Item Height ·
  Item Width · Item Reveal · ☐ Lock from Auto-Resize · Shelves Specify… ·
  Appliance/Door/Drawer Specify… Edit… Clear ☐ Reverse Appliance · Percent
  Open …

### Door/Drawer
- *Door Panel*: Main Style [Lincoln Door ▾] Library… · Thickness `3/4"` ·
  ☐ Glass Doors · ☐ Stile Between Double Doors
- *Door Handle*: Main Style [Knob ▾] Library… Edit… · Vertical Position
  ○ Centered ◉ Distance From Top `1 3/8"` · Horizontal Position ○ Centered
  ◉ Distance From Edge `1 3/8"`
- *Door Hinges*: Main Style [Hidden ▾] · Up/Down From Edge `3"`
- *Drawer Panel*: Main Style [Lincoln Flat Panel Drawer ▾] · Thickness `3/4"`
- *Drawer Handle*: Main Style [Knob ▾] · Horizontal Position ◉ One Handle
  Centered …

### Accessories
- *Front Pilasters*: Front Pilaster [None ▾] Library… · Left/Right ◉ Auto
  ○ On ○ Off · Width `2"` · ☐ Extend to Bottom
- *Feet*: Foot Style [None ▾] · Width Offset `0"` · Depth Offset `0"` ·
  ☐ Always Present · ☐ Stretch To Fit · ☐ Retain Toe Kick
- *Side Panels*: Main Panel Style [Slab Panels ▾] · Thickness `3/4"`
- *Side Panel Options*: ☑ Full Size Panel · ☐ Full Overlay · ☐ Extend to
  Bottom
- *Top Appliance / Fixture*: Front Offset `1 1/2"`

### Moldings
- *Profiles* table (Name · Width · Height · Repeat Distance · Horiz. Offset ·
  Vertical Offset) with Add New… · Make Copy · Edit… · Replace… · Delete ·
  Make Stack · Explode Stack · Move Up · Move Down · Add to Library ·
  ☐ Retain Aspect Ratio · ☐ Auto Offset
- *Selected Profile Options*: Type · Position on Object [Top ▾] · Molding
  Position: Vertical Position [Under Polyline ▾] · Profile Rotation `0.0°` ·
  Reflect Horizontal / Vertical · ☐ Offset Molding For Face Items · ☐ Texture
  Up Direction Is Along Molding · ☐ Count Components in Materials List ·
  ☑ Show Position Indicator

### Plan Studio mapping (Cabinet)
General size/position + countertop + toe kick → Phase 2 (cabinets are a 3D
feature). Front/Sides/Back face layout tree is the heart of Chief's cabinet
tool and is a Phase 2 milestone of its own. Box Construction, Door/Drawer
styles, Accessories, Moldings → Phase 2–3.

## Dimension Defaults (Dimension ▸ Dimensions)

Dimensions use **Saved Dimension Defaults**: a list dialog (Available
Dimension Defaults: 1" Scale · 1/2" Scale · 1/4" Scale (active) · 1/8" Scale
· Electrical · Foundation · Framing · HVAC · Kitchen and Bath · Legacy NKBA ·
NKBA · Plot Plan · Roof · … with Edit… Copy… Rename… Delete · Select All ·
Clear All) and a "Currently Active Dimension Defaults" combo. Each set opens
*Dimension Defaults – <name>*.

Tabs: **General · Setup Automatic · Setup Temporary · Locate Manual · Locate
End to End · Locate Centerline · Locate Interior · Locate Auto Exterior ·
Locate Auto Room · Locate Auto Elevation · Locate Elevations · Primary Format
· Secondary Format · Extensions · Layer · Arrow · Text Style**

### General
- *General*: Baseline Line Separation `18"` ☑ Use Auto Line Separation ·
  Reach `24"`
- *Rounded Value Indicators*: ☐ Show + or – After Number · ☐ Show ~ Before
  Number
- *Rounding Method*: ◉ Grid Rounding ○ Distance Rounding
- *Dimension Text Position And Orientation*: ○ Centered On Dimension Line
  ◉ Above Dimension Line ○ Below Dimension Line · Angle `0.0°` ☑ Automatic
- *Leader Line*: Leader Style [Square Corner ▾] · ☐ Include Second Segment ·
  Second Segment Length `5"` · ☑ Include Arrow · Style [● line ▾] · Size
  `1 1/4"` ☐ Match Dimension
- *3D Display*: ☑ Extend Extensions To Mark · ☐ Label Faces Camera

### Setup Automatic
- *General*: Line Separation `18"` · 1st Line Offset `32"` · Offset From Wall
  ◉ Center ○ Dimension Layer ○ Surface
- *Exterior*: Reach `240"` · Minimum Area `10.0` sq ft · 3D Height Above
  Floor `10"` ☐ Vertical Labels · ☑ Overall Dimension · ☑ Inner/Outer
  Dimensions · ☐ Auto Refresh
- *Room*: Minimum Area `10.0` sq ft · 3D Height Above Floor `10"` · ☑ Overall
  Dimension · ☐ Outer Dimension · ☐ Auto Refresh · ☐ Allow Duplicates · Line
  Position ○ Outside Room ◉ Inside Room
- *Elevation*: ☑ Overall Dimension · ☐ Outer Dimension (Top/Bottom) · ☐ Auto
  Refresh · ☑ Dimension on Left · ☐ Dimension on Right · ☐ Dimension Across
  Top · ☐ Dimension Across Bottom

### Setup Temporary
- *Options*: Dimension Row Limit `1` · Reach `24"`
- *Walls*: ○ Surfaces ◉ Wall Dimension Layer
- *Wall Options*: Exterior ☑ Primary Side ☑ Secondary Side · Interior
  ☑ Primary Side ☑ Secondary Side ☐ Centers
- *Locate Objects Inside*: ☐ CAD Objects · ☐ Terrain Objects
- *Locate Objects* tree: Openings (Casing, ☑ Centers, Rough Opening, Sides) ·
  Cabinets (☑ Sides, ☑ Corners, Centers, Moldings, Countertop, Backsplash,
  Toe Kick, Openings, Doors/Drawers/Panels) · CAD Objects (☑ Lines/Sides,
  ☑ Ends/Corners, ☑ Callouts/Markers, ☑ Clip Lines, Text, Construction
  Lines) · Fixtures/Appliances · …

### Locate Manual (and the other Locate tabs)
- *Walls*: ○ Surfaces ◉ Wall Dimension Layer ○ None
- *Wall Options*: Exterior ☑ Primary Side ☐ Secondary Side · Interior
  ☑ Primary Side ☐ Secondary Side ☐ Centers · ☐ Wall Steps · ☐ Brick Ledge
  Lines · ☑ Display Wall Widths
- *Locate Objects*: the same object tree (Openings ▸ Centers checked by
  default). *Locate Auto Exterior* shows the same controls with only Openings
  ▸ Centers available.

### Primary Format
- *Format*: Units [' – " ▾] · ☑ Unit Indicators · ☐ Leading Zeroes
  · ☑ Trailing Zeroes · ☐ Thousands Separator (◉ Use Comma ○ Use Space) ·
  ☐ Display as Inches (Less than or equal to `18"`) · Fraction Style
  [Diagonal ▾] · Fraction Text Size `60%`
- *Accuracy*: ○ Decimal Places `4` ◉ Smallest Fraction 1/ `8` · ☑ Show
  Denominator · ☑ Reduce Fractions (◉ Use Greatest Common Divisor ○ Use
  Closest Fraction)
- *Angular Format*: Angle Style [Degrees ▾] · Decimal Places `2`

### Extensions
- *Extension Length*: Length Away From Marked Object `3"` · ◉ Fixed Gap From
  Marked Object `3"` ○ Length Towards Marked Object `3"`
- ☐ *Fixed Proximity*: Distance To Marked Object `12"`
- *Centerline*: ☐ Auto Mark Centerline · ☐ Same Angle As Dimension · Offset
  From Extension `3"`

### Arrow
- ☑ Include Arrow · Style [tick ▾] Select… · CAD Block Fill ◉ Use Block Fill
  Color ○ Use Arrow Fill Color · Arrow Fill Color swatch · ○ Match Line Color
  ◉ Custom Fill Color · Size `2 1/4"`

### Plan Studio mapping (Dimension)
Primary Format (units, fraction accuracy) and Extensions/Arrow drive how
every dimension string is drawn and are needed in Phase 1 with the first
dimension tool. Setup Automatic (reach, offsets, what to locate) → Phase 1
auto exterior dimensions. Temporary dimensions already exist and will take
the Setup Temporary options.
