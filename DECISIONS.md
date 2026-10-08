# Open decisions for Daniel

Items parked here need Daniel's call. Work continues elsewhere until he
answers; each item notes the assumption currently built into the code.

| # | Decision | Assumption in code until decided | Raised |
|---|---|---|---|
| 1 | License stays MIT, or switch to Apache-2.0 / GPL for stronger copyleft? | MIT | 2026-10-07 |
| 2 | App name: "Plan Studio" is a working title. Keep it, or pick a brand name? | Plan Studio | 2026-10-07 |

| 3 | Chief library content (.calib/.calibz): Plan Studio will read Core/Bonus/Manufacturer catalogs from the local Chief install at runtime (licensed to Daniel), never bundling them in the repo. OK, or should imported items be re-saved into a user library folder outside the repo? | Read at runtime from Chief's folders; our own starter symbols ship in the repo | 2026-10-08 |
| 4 | Hotkeys off macOS: Daniel's Control+Z (Down One Floor) and Command+Z (Undo) fold onto one Ctrl+Z on Windows/Linux. Code lets the Command chord win (Undo = Ctrl+Z, Chief's Windows default) and reports Down One Floor as unmapped there. Pick a different Windows key for Down One Floor? | Undo wins; Down One Floor unmapped off macOS | 2026-10-08 |
| 5 | Straight Railing's Chief hotkey is ⌘Q, which is Quit on macOS. Shown on the entry, not bound. Rebind (e.g. ⌥⌘Q) or leave? | Not bound | 2026-10-08 |
| 6 | Click Stairs heading: built as "the pointer's last movement direction before the click" (defaults to up the screen). Verify in Chief. | As built | 2026-10-08 |
| 7 | Stucco-6 from the real template is 7.635" (0.01" housewrap layer) vs the 7 5/8" captured from the dialog; the template wins when seeding is on. Keep the real stack? | Template wins | 2026-10-08 |
| 8 | 3D selection tint and billboards go through `plan-view3d`'s whole-scene upload (`Viewport3d::queue_scene`), re-queued only when the selection or a billboard's geometry changes, because plan-view3d was outside this round. A real overlay pass in the viewport (`set_overlay`, uploaded every frame, drawn after the cached meshes) would be cheaper and could draw an outline. Add it? | Re-queue | 2026-10-08 |
| 9 | `Material::Selection` (an orange translucent tint) was added next to the landscape materials; it is only ever in the live viewport, never in an export or ray trace. Keep it as a Material or give the viewport its own highlight colour? | Material | 2026-10-08 |
| 10 | A click in the 3D view on a surface that is no object (floor slab, ground, a hidden-layer mesh) clears the selection, like clicking empty plan; the first triangle the ray meets wins even if an object stands behind it. Alternative: skip object-less surfaces and pick what is behind. | Nearest wins | 2026-10-08 |

## Queued work (not decisions, just the running order)

1. ~~Hotkeys: bind Chief's full hotkey table (all chords) plus Daniel's custom
   hotkeys from his Chief data folder; add Tools ▸ Customize Hotkeys dialog.~~
   **Done** (2026-10-08): the base table, Daniel's Chief hotkeys (embedded) and
   the Customize Hotkeys dialog with `~/.plan-studio/hotkeys.json`; chords fold
   to Ctrl where there is no Command key.
2. Round 2 editing engine from docs/parity/select-and-edit.md and walls.md.
3. 3D view integration (plan-view3d) with Chief's camera tools.
