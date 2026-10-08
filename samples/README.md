# Sample plans

Three ready-to-open `.psplan` projects for Plan Studio (File > Open, or
`cargo run -p plan-app` and pick one). They start from Daniel's Chief X18
defaults (`PlanDefaults::chief_x18_daniel`): 9'-1 1/8" ceilings, Stucco-6 or
Siding-6 exterior walls, Interior-4 partitions, 36" exterior and 30" interior
doors, and the 1/4" scale dimension settings.

The files are generated, not hand-edited. To rebuild them:

```text
cargo run -p plan-core --example make_samples
```

The example reads every file back with `Project::from_json`, checks that it
re-serializes identically, and asserts the room counts, room names, 6" grid,
outward-facing exterior walls, clear openings and symbol placement before it
prints the summary table. Edit `crates/plan-core/examples/make_samples.rs` to
change a sample. If the files already exist and the write is refused, delete
them first.

| File | Floors | Rooms | Interior area | Doors | Windows | Symbols |
|---|---|---|---|---|---|---|
| `ranch-3bed.psplan` | 1 | 11 | 1,880 sf (house 1,421 sf + garage 459 sf) | 14 | 11 | 12 |
| `two-story-colonial.psplan` | Foundation, 1st, 2nd | 1 + 6 + 9 | 1,200 sf + 1,186 sf above the foundation | 9 + 9 | 12 + 16 | 6 + 9 |
| `studio-adu.psplan` | 1 | 3 | 349 sf (403 sf measured to the outside of the walls) | 3 | 5 | 6 |

"Doors" counts every door-kind opening, so cased openings, pocket and bifold
doors and the garage door are included. Each file also carries automatic
exterior dimensions on every normal floor.

## ranch-3bed.psplan

One-story 3 bed / 2 bath ranch. The house is 48' x 32' (centerlines) with an
attached 22' x 22' two-car garage on the east end, so the overall footprint is
70' x 32'. The front is the bottom edge of the plan.

- Rooms: Living, Dining, Kitchen, Laundry, Hall, Bedroom 2, Bedroom 3, Bath,
  Master Bedroom, Master Bath, 2-Car Garage.
- Exterior walls are Stucco-6. The wall between the house and the garage is an
  interior wall.
- Bedroom, Master Bath and Laundry doors swing into the room they serve. The
  hall bath uses a pocket door. Living, Dining, Kitchen and Hall are joined by
  cased openings. The garage has a 16' garage door, a side entry door and a
  door into the Laundry.
- Every bedroom window is an egress window (36" x 48" or larger, 36" sill).
  The bath windows are tempered.
- Symbols: toilets, tub, shower, vanities, range, refrigerator, washer,
  dryer and a water heater, all placed flush to a wall.

## two-story-colonial.psplan

Center-hall colonial, 40' x 32', siding exterior, four bedrooms up. The floors
are Foundation (a 48" stem wall under the exterior walls from `build_foundation`),
1st Floor and 2nd Floor (the exterior shell comes from `build_new_floor`).

- 1st Floor: Living, Family Room, Dining Room, Kitchen, Center Hall, Powder Room.
- 2nd Floor: Master Bedroom, Master Bath, Master Closet, Bedroom 2, Bedroom 3,
  Bedroom 4, a Jack-and-Jill Bath between Bedrooms 2 and 3, Laundry and the
  Upstairs Hall. Every bedroom window is an egress window.
- The stairs are a placeholder symbol (`core.stairs.placeholder_straight`,
  42" x 132") on the west side of the hall on both floors. It marks the
  footprint only. Draw the real stair with the stair tool.

## studio-adu.psplan

A 19'-6" square accessory dwelling unit (403 sf measured to the outside of the
walls). The kitchenette runs along the west wall (refrigerator, range, double
sink under a window), the bath is in the northeast corner (shower, toilet,
vanity, hinged door swinging in) and a closet with a bifold door sits in the
northwest corner. The studio room itself is typed Great Room, since the
default room types have no Studio.

## Notes

- Symbol ids use the plan-library naming (`core.plumbing.toilet_elongated`,
  `core.appliances.range_30`, and so on). The plan only stores the id string
  and size, so the files open without the library.
- Doors store `swing_flipped` (which side of the wall the leaf swings to) and
  `hinge_at_end` (which jamb carries the hinge) separately, as the model
  documents. The 2D plan drawing currently derives the hinge side from
  `swing_flipped` alone, so a few doors may show the hinge on the opposite jamb
  from the one stored. The swing side is always right.
