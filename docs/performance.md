# Performance

Plan Studio has to stay smooth on a real house. Daniel's plans run to 300 to
800 walls, 150 openings, 60 cabinets, 20 roof planes, 40 dimensions, 200 CAD
items, 10 schedules and a terrain with 50 elevation lines over three floors.
Chief redraws such a plan in well under 16 ms; so must we.

## The benchmark house

`samples/large-house.psplan` is that plan, generated without randomness by
`crates/plan-core/examples/make_samples.rs`:

| | count |
|---|---|
| floors | 3 |
| walls | 591 (about 197 a floor) |
| openings | 153 (doors and windows) |
| rooms | 78 a floor, every one named |
| cabinets | 60 |
| roof planes | 20 |
| dimensions | 40 |
| CAD items | 200 (14 carry a fill or hatch) |
| schedules | 10 (door and window tables list all floors) |
| material regions / wall hatches | 16 / 42 |
| terrain | built, 50 elevation lines, contour interval 24" |

Rebuild only this file with
`cargo run -p plan-core --example make_samples -- --large-only` (delete the old
file first if the write is refused). A test
(`the_large_sample_has_the_advertised_object_counts`) fails if the counts or the
typed slots (cabinets, roofs, terrain), which the generator writes as JSON,
drift from what their crates parse.

## Running the benchmark

```text
cargo test -p plan-app perf_bench -- --ignored --nocapture
cargo test -p plan-app --release perf_bench -- --ignored --nocapture   # the real numbers
```

`editor/perf_bench.rs` (a child of `editor/render.rs`) times, as medians after
a warm-up call: startup pieces, `Project::from_json`, `cx.refresh()` and the
room/outline functions alone, a scaling series of single floors of 220 to
1,624 walls, `draw_plan` into a headless egui painter at three zooms on the
ground floor and the roof floor (with a per-stage breakdown from the
`section!` markers in `draw_plan`, which cost nothing outside test builds), a
whole headless UI frame the way `PlanApp::update` runs it, a built-framing
floor, a Select Objects pointer move, the layout window, `project_hash`,
`build_view_scene`, and the construction set (layout build and `render_pdf`,
with the drawing time of each page).

The numbers below are the dev/test profile (`opt-level = 1` for everything)
on the development Mac. They are for comparing before and after, not for
quoting: a release build is several times faster. (The release numbers could
not be taken for this write-up: the build sandbox refused to run `rustc` from
the build scripts of a clean release build. Run the second command above on a
normal shell.)

## Before and after

Large sample, milliseconds, median.

| operation | before | after |
|---|---:|---:|
| `draw_plan`, ground floor, whole house in view (0.45 px/in) | 5.27 | 0.95 |
| `draw_plan`, ground floor, working zoom (1.5 px/in) | 5.48 | 1.19 |
| `draw_plan`, ground floor, close-up (5 px/in) | 5.90 | 1.64 |
| `draw_plan`, roof floor, whole house | 2.32 | 0.46 |
| `draw_plan`, roof floor, close-up | 2.94 | 1.15 |
| `draw_schedules`, 6 tables on the ground floor | 4.42 | 0.01 |
| whole idle UI frame (menus, panels, canvas) | not measured (about 4.4 more) | 1.34 |
| `cx.refresh()` after a CAD/dimension/schedule edit | 1.69 | 0.00 |
| `cx.refresh()` after a wall edit, 197 walls | 1.69 | 0.61 |
| `detect_rooms`, 197 walls | 0.75 | 0.28 |
| `wall_outlines`, 197 walls | 0.35 | 0.06 |
| `wall_layer_outlines`, 197 walls | 0.52 | 0.23 |
| `detect_rooms`, 840 walls on one floor | 11.05 | 1.25 |
| `wall_outlines`, 840 walls | 5.55 | 0.23 |
| `detect_rooms`, 1,624 walls | 45.80 | 2.38 |
| `wall_outlines`, 1,624 walls | 21.65 | 0.43 |
| framed floor (3,813 members): `refresh()` after a non-wall / a wall edit | 4.1 / 4.1 | 0.61 / 1.19 |
| framed floor: draw at 0.45 / 5 px/in | 2.07 / 2.61 | 1.03 / 1.50 |
| framed floor: the framing stage of a frame | 1.15 | 0.16 |
| layout window, one frame (per-frame plan signature) | 2.0 extra | 0.10 total |
| `project_hash` | 1.57 | 1.71 (not changed; `view3d_panel.rs`) |
| `build_view_scene`, all floors | 4.70 | 3.69 (not changed) |
| construction set: build the layout | 5.6 | 6.2 (not changed) |
| construction set: `render_pdf` (1.3 MB) | 597 | 625 (not changed) |
| Select Objects, one pointer move | 0.20 | 0.21 |

The drawn output is the same: the shape counts of the headless frames are
identical before and after (ground floor, whole house: 11,377 shapes, 10,552
line segments, 573 paths, 172 texts; roof floor 6,012; working zoom 23,520;
close-up 60,270). The only difference is that framing members outside the
canvas are no longer sent to the painter.

Where the rest of a frame goes at the working zoom, ground floor: placed
cabinets 0.47 (their JSON is parsed every frame, `placed.rs`), the terrain
0.22 (`site_view`), walls 0.15, material regions 0.13, room labels 0.10.

## What changed

* **Schedules** (`editor/schedule_view.rs`): the tables, their sizes, the
  parsed `ScheduleLayer` and the door/window/cabinet callouts were rebuilt
  from the whole plan on every frame, for every table, even off screen (4.4
  ms of a 5.3 ms frame). They are cached until the editor context signals a
  change. Picking (every pointer move) and box selection use the same cache.
* **Rooms and wall outlines** (`plan-core`): `detect_rooms`, `wall_outlines`
  and `wall_layer_outlines` were quadratic (every wall against every wall,
  every graph node against every node, every room edge against every wall).
  They now use a uniform grid over the wall boxes (`geometry::BoxGrid`) and a
  hash of the graph nodes. Results are identical: the grid only narrows the
  candidates, which come back in the original order, and the full outputs
  were compared field by field (as `Debug` text) with the previous
  implementation on the samples and 40 randomised plans of 190 to 2,700 walls
  with gaps, jitter, diagonals, Ts and curves. Permanent tests compare the
  grid path with the plain scan.
* **`EditorContext::refresh`** skips the room detection and the outlines when
  the walls and wall types are unchanged (`plan_core::walls_equal`, an
  exhaustive field comparison), and skips parsing the framing JSON when the
  stored values are unchanged. An edit of a CAD line, a dimension or a
  schedule no longer redoes either.
* **Framing** (`editor/framing_view.rs`): the plan outline (convex hull) of
  every built member was computed every frame; hulls and the manual records
  are cached, and members outside the canvas are skipped.
* **Walls** (`editor/render.rs`): the mitered outline of each wall was found
  by scanning the outline list (quadratic in the walls); an id index is kept
  with the cache key.
* **Layout window** (`shell/layout_window.rs`): each frame serialized the
  floors, layers, cameras, text styles and terrain to JSON and hashed the
  text to decide whether its box drawings were stale. It now hashes the
  context's cache key.
* **Pattern strokes** (`details_view`): already cached by outline, material
  and zoom; nothing to change. `floor.cad_attr_map()` costs microseconds and
  is left alone.

## The cache contract

Draw caches are keyed by `EditorContext::cache_key()`: `(uid, rev)`. `rev`
moves on every `begin_change`, `begin_change_merged`, `record_undo_step`,
`mark_dirty`, `reset_view_state`, undo, redo, and on every `refresh()` that
recomputes. The rule for code that edits `cx.project` is the one the rooms
already imposed: call `begin_change` before the edit (which an undo step needs
anyway) or `mark_dirty` after it. An edit that does neither already left the
rooms, outlines and framing stale; now it also leaves the schedules, framing
hulls and layout drawings stale until something else signals. Functions that
change the project without an undo step of their own call `mark_dirty`
(`schedule_view::translate_ids` does).

Tests: `a_door_added_changes_the_schedule_that_is_drawn` (table text, layout
and labels after an add and after undo), `the_cached_layout_equals_a_fresh_build_after_every_kind_of_edit`,
`a_region_hatch_follows_the_region_when_it_moves` (every pattern stroke moves
with the region), `rooms_are_kept_for_non_wall_edits_and_rebuilt_for_wall_edits`,
`framing_is_parsed_again_only_when_its_values_change`, and in `plan-core`
`outlines_with_the_grid_equal_outlines_from_a_scan`,
`the_position_index_gives_the_rooms_a_scan_gives`,
`box_grid_returns_every_overlapping_box_in_order`, `walls_equal_sees_every_difference`.

## Build profiles

`[profile.release]` is `opt-level = 3`, thin LTO, one codegen unit.
`panic = "abort"` is deliberately not set: the 3D view builds its scene and
drawings on worker threads and survives a panic there with `catch_unwind`
(`shell/view3d_panel.rs`); with `abort` the same panic would close the editor
and lose unsaved work. The dev profile stays at `opt-level = 1` for the
workspace and dependencies; raising the dependencies to 3 would speed up
debug runs but rebuilds every dependency for everyone's first build, so it
was left as is.

`scripts/macos-bundle.sh release` only copies
`$CARGO_TARGET_DIR/release/plan-studio` into the `.app` and writes
`Info.plist`; it does not depend on the profile settings, so it is unaffected.
(A release build of the whole tree was not run for this change.)

## Startup

With the template cache warm, everything before the first frame is cheap:
embedded defaults 0.08 ms, `seeded_defaults` over a cached Chief template
0.7 ms, `PlanApp::new` 4.6 ms, the first headless UI frame 7.7 ms (fonts,
layout), opening the 1.4 MB sample 2.4 ms.

The one slow path was the first launch on a machine that has no `templates`
key in `~/.plan-studio/settings.json`: the scan in
`plan_config::detect_chief_templates` (`scan_for` walking the home folder for the
Chief plan and layout templates) ran for about 6.5 seconds in this environment, on
the main thread inside `main()`, before the window opened, and again for every
caller of `templates::load_settings` for as long as the settings file could not be
written.

That is fixed. `templates::load_settings` and so `plan_defaults::load` only read
the saved key (or this session's result); they never scan. `main()` starts the scan
with `templates::begin_detection`, which runs `detect_and_seed` on a thread (the
scan, the settings write and the decode of the template that seeds the defaults),
and `PlanApp::update` polls `templates::poll_detection` once a frame. The window opens
at once on the shipped defaults with the status line "Looking for your Chief templates in
the background..."; when the scan ends the defaults are seeded (a plan nobody has touched
starts over from them, a worked plan is kept) unless the user saved defaults of their own,
and the status line says what was found. The result is kept for the session even when the
settings file cannot be written, so a read-only home is scanned once per session and
not once per call (`templates::Startup`; tests drive it with an injected scan: the
scan runs on another thread, runs once, and a failed write does not repeat it).

## What is still slow, and why

* **Elevations in the construction set**: 590 ms of the 625 ms PDF is the
  three elevation/section drawings (front/back 187, left/right 367, section
  56; 20,000 lines). That is `plan-elevation` hidden-line removal, not the
  PDF writer. An export, not a redraw, but a layout page with an elevation box
  recomputes on every change of the plan.
* **`project_hash`** (1.7 ms) formats every wall and opening with `{:?}` and
  hashes the text, on every frame the 3D view is open (`view3d_panel.rs`). A
  hash over the numeric fields, or the context's `cache_key`, would make it
  free.
* **`build_view_scene`** 3.7 ms for three floors; fine for an edit, slow if
  the 3D view rebuilt it per frame (it is rebuilt on a hash change).
* **Placed cabinets** parse their JSON every frame (0.47 ms for 30 on the
  floor); `placed.rs` could use the same cache.
* **Terrain** (`site_view::terrain_view`) compares the stored JSON value with
  the cached one every frame (0.1 to 0.7 ms at close zoom with 50 lines).
* **Tessellation**: a close-up of the ground floor is 58,000 line segments
  before clipping. egui tessellates what it is given; the plan could cull
  segments outside the canvas before painting (the pattern strokes and dashed
  lines are the bulk). Tessellation of the whole frame measures 0.8 to 1.2 ms
  here.
