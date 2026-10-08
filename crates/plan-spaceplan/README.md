# plan-spaceplan

Chief's Space Planning Assistant for Plan Studio (lengths in inches, 6" grid).

- `Questionnaire` + `generate_boxes`: answers become named, colored `RoomBox`es, auto-arranged by a
  greedy affinity packer (heuristic documented in `src/arrange.rs`).
- `bump`: drag-time snapping to neighbor edges and corners; never leaves an overlap.
- `validate`: overlaps, floating rooms, baths without hall/bedroom, garage not touching the house.
- `build_house`: boxes become walls (shared edge = one interior wall), doors, windows and room names.
- `plan_symbols`: filled rectangles with name and area labels. Deck/Porch boxes get no walls.
