# Defaults that differ: Chief's Residential Template, Daniel's working template and Plan Studio today

Consolidated from part 7 ("Defaults that differ") and the decoded values in `crates/plan-app/assets/templates/chief-x18-daniel.json`. **Three columns of values, one column for Daniel's decision.** "Chief" is what the Tutorial Guide states or sets while following the Residential Template; the stock template's own starting values are mostly not printed in the guide, so a blank means unknown, not zero. "Daniel's template" is the `x17 Working Template 2025-08-20.plan` as decoded into the embedded JSON (`docs/daniel-template-inventory.md`). "Plan Studio today" is what a new plan gets: Daniel's embedded template, or the code default where the template is silent. No row was changed by this audit; the second-to-last column is a suggestion.

| Setting | Chief Residential Template (tutorial) | Daniel's working template | Plan Studio today | Suggested | Why / brief | Daniel decides |
|---|---|---|---|---|---|---|
| New file | Project + Plan (Residential Template) + Layout (Arch D 24x36); units chosen at creation (pp. 3, 45) | Plan templates (x17 Working Template, Residential, Interior, Commercial) and layout templates (Arch D, Arch C, Tabloid, ISO A1...) in his Templates folder | One .psplan from Daniel's embedded template; the layout lives inside the plan; units from the template | Offer both | Brief 26 adds the New Plan from Template chooser; keep one-file plans (DECISIONS 61). | [ ] Chief  [ ] Daniel's  [ ] Both |
| Roof and exterior dimensions when a room closes | Built automatically; Auto Rebuild Roofs / Auto Refresh on in the template (the guide turns it off for lesson 7) (pp. 6, 15, 129) | Not decoded from his template (the flags are not in the decoded data) | Not built automatically; Auto Rebuild defaults on once a roof exists (DECISIONS 97) | Offer both | Briefs 18 and 28 add a Preferences switch, default off. Check which Daniel's own Chief template does. | [ ] Chief  [ ] Daniel's  [ ] Both |
| Default exterior wall type | Siding-6 (the guide changes it to Stone-6 in lesson 1) | Stucco-6, 7 5/8 in (real stack 7.635 in, DECISIONS 7); also Siding-6, Brick-6, stone-6 | Stucco-6 (the template wins when seeding is on) | Keep Daniel's | Daniel works in stucco; no reason to change. | [ ] Chief  [ ] Daniel's  [ ] Both |
| Interior wall type | Interior-4 (2x4 plus drywall) | Interior-4 (4.5 in) and Interior-6 | Interior-4 and Interior-6 | Keep Daniel's |  | [ ] Chief  [ ] Daniel's  [ ] Both |
| Extra wall types the guide uses | Fire-6, Room Divider (0 in), Deck Railing/Fence, Interior Railing (3 layers), 8" Concrete Stem Wall | 108 wall types in the working template | 12 embedded: Stucco-6, Siding-6, Brick-6, Foundation-8, stone-6, Glass-1, Railing-4, Deck Railing-4, Deck Edge-2, Fence-Wood-2, Interior-4, Interior-6 | Offer both | Import all 108 through Import Settings (brief 26) and add Fire-6, Room Divider and Interior Railing as stock extras (brief 12). | [ ] Chief  [ ] Daniel's  [ ] Both |
| Pony wall | Upper Siding-6, lower Stone-6, Elevation of Lower Wall Top 20 in | Upper Stucco-6, lower Foundation-8, split at 36 in | Same as Daniel's | Keep Daniel's |  | [ ] Chief  [ ] Daniel's  [ ] Both |
| Ceiling height, Floor 1 | 97 1/8 in rough ceiling set in the lesson; second floor default 97 1/8 in | 109 1/8 in (walls 109.125 in) | 109 1/8 in | Keep Daniel's | Custom homes: 9 ft 1 1/8 in plus platform. | [ ] Chief  [ ] Daniel's  [ ] Both |
| Floor structure | 12 in (3/4 in OSB + 2x12) after the lesson's edit; I-joist before | One 10 1/4 in thickness per floor default | One thickness, no layer table | Follow Chief | Layered definitions (brief 13) keep 10 1/4 in as the default. | [ ] Chief  [ ] Daniel's  [ ] Both |
| Floor and ceiling finish | Layered Floor Finish (e.g. 7/8 in with underlayment); Ceiling Finish drywall plus paint; 5/8 in drywall in the garage | Floor finish 3/4 in, ceiling finish 5/8 in | Thickness and a material name | Follow Chief | Brief 13. | [ ] Chief  [ ] Daniel's  [ ] Both |
| Garage and porch structure | Garage 4 in concrete slab; Porch concrete with ceiling and roof over | 46 room types including Garage, Porch, Slab, Courtyard, Crawl Space | Room-type defaults hold function and finish name only | Follow Chief | Brief 15 (functions) and 17. | [ ] Chief  [ ] Daniel's  [ ] Both |
| Window default | Single Casement 30 x 54, lites 3 x 4 (lesson 5), rough opening +1 in | Single Casement 32 x 72, sill 24 in, lites 1 x 1, egress and tempered on | Same as Daniel's | Keep Daniel's | His windows are code egress sizes. | [ ] Chief  [ ] Daniel's  [ ] Both |
| Door defaults | Interior Door P04; exterior door with four lites over a panel | Door P04 both; interior 30 x 96, 1 3/8 in thick, casing 3 1/2 in; exterior 36 x 96, 1 3/4 in thick, casing 3 1/4 in | Same as Daniel's | Keep Daniel's |  | [ ] Chief  [ ] Daniel's  [ ] Both |
| Auto Exterior Dimension locate | Openings: Sides; walls at the Wall Dimension Layer; Reach 24 in in the Electrical set | Set '1/4" Scale': openings None, walls Main Layer, centers on, reach 0, offset 32 in | Same as Daniel's | Keep Daniel's | His 14 dimension sets are the standard. | [ ] Chief  [ ] Daniel's  [ ] Both |
| Stairs | Interior stairs 44 in wide, newels 4 x 44 in, riser about 7 5/8 in | Not in the decoded template | Code defaults: width 36 in, riser 7.5 in, tread 10 in, headroom 80 in | Offer both | Read his real stair defaults from a Chief plan; IRC limits stay as warnings. | [ ] Chief  [ ] Daniel's  [ ] Both |
| Deck | Plank gap 1/4 in; footings Height Above Terrain 6 in, Thickness 30 in | Not in the decoded template | Plank gap 0.25 in; footing height above grade 36 in, thickness 12 in, size 18 in | Follow Chief | Footing numbers differ widely; confirm with Daniel against the frost depth in the Plan Check settings. | [ ] Chief  [ ] Daniel's  [ ] Both |
| Roof | Rafters; pitch 12 in 12 for the cottage; overhang 6 in for dormers; depth 11 1/4 in; spacing 16 in | Hip, pitch 8 in 12, overhang 16 in; rafter spacing 24 in, depth 5 1/2 in, thickness 6 in | Same as Daniel's | Keep Daniel's |  | [ ] Chief  [ ] Daniel's  [ ] Both |
| Soffit | 24 x 12 x 12 at 84 in (lesson 15) | 24 x 12 x 12 at 84 in | Same | Keep Daniel's | Agrees. | [ ] Chief  [ ] Daniel's  [ ] Both |
| Outlet, switch and counter heights | Outlet 11 1/2 in, switch 48 in, counter outlet 43 in | Not decoded | Outlet 12 in, switch 48 in, counter outlet 44 in | Follow Chief | Brief 25 replaces these with four height groups. | [ ] Chief  [ ] Daniel's  [ ] Both |
| Rich text character height | 6 in at 1/4 in scale; 3 in at kitchen-and-bath scale | Default text style 6 in (9 pt printed), Schedule and label styles 4.5 in; 13 per-scale rich text default sets | 6 in; no per-view saved text defaults | Keep Daniel's | Needs Saved Defaults and Default Sets (brief 26). | [ ] Chief  [ ] Daniel's  [ ] Both |
| Layout sheet numbering | Label A0.#, A1.#, E1.# with duplicates intended | Layout templates carry their own pages; numbering pattern not decoded | A-{n}; duplicates refused (DECISIONS 62) | Follow Chief | Brief 01. | [ ] Chief  [ ] Daniel's  [ ] Both |
| Layout grid snap | 1/8 in Grid Snap Unit; nudge equals the unit | Not decoded | Fixed 1/16 in; nudge 1/16 in (Shift 1/4 in) | Follow Chief | Brief 01 (L-230). | [ ] Chief  [ ] Daniel's  [ ] Both |
| Layout drawing scale | 1 in = 1 in; plan default 1/4 in = 1 ft | Dimension and rich text default sets per scale (1", 1/2", 1/4", 1/8") | Implicit 1:1; Send to Layout picks its own scale | Follow Chief | Briefs 02 and 03. | [ ] Chief  [ ] Daniel's  [ ] Both |
| Send to Layout site scale | 1 ft = 100 ft offered | Not decoded | Not offered (list stops at 1 in = 20 ft) | Follow Chief | Brief 02. | [ ] Chief  [ ] Daniel's  [ ] Both |
| Reference grid and crosshairs | On in the template | Grid spacing 12 in, snap 1 in | Grid 12 in with 1 in snap; crosshairs a View toggle | Keep Daniel's |  | [ ] Chief  [ ] Daniel's  [ ] Both |

*Suggested*: **Follow Chief** = change Plan Studio to Chief's behavior or number; **Keep Daniel's** = Daniel's template is his standard, leave it; **Offer both** = add a template or preference choice.

## What the template carries, by count

The biggest difference is not a single value but how much of Daniel's setup the embedded template holds. Counts from `docs/daniel-template-inventory.md` and the embedded JSON:

| Item | Chief Residential Template | Daniel's x17 Working Template | Embedded in Plan Studio |
|---|---|---|---|
| Layer sets | 22 | 34 | 1 (Default Set) |
| Layers | 217 | 356 | 18 (Floor Plan set) |
| Text styles | 9 | 12 | 6 |
| Dimension default sets | 11 | 14 | 13 (1" Scale ... Roof) |
| Rich text default sets | not itemised | 13 | 0 (no saved text defaults) |
| Wall types | 41 | 108 | 12 |
| Saved plan views | 12 | 20 | 0 |
| Macros | 172 | 161 | built-in only |

This is why briefs 26 (saved defaults, default sets, saved plan views, Import Settings) and 12 (wall type definitions) rank so high: opening Daniel's real working template should bring his layer sets, 14 dimension sets, 13 text sets, 108 wall types and 20 saved views with it, and switching between them should work the way it does in Chief.

## Decisions that need Daniel before launch

**Decided 2026-10-09 (Daniel): use my template.** Every row below resolves to the "Daniel's template" column; Chief's Residential Template values stay as reference. Recorded as DECISIONS DT1.

1. Rows marked *Offer both*: new-plan behavior (automatic roof and exterior dimensions), stair numbers, the template chooser.
2. Rows marked *Follow Chief*: confirm each (layered structure, layout numbering, grid snap, outlet heights, deck footings).
3. Whether imported templates (108 wall types, 34 layer sets) should replace the embedded 12-type template or merge into it.
