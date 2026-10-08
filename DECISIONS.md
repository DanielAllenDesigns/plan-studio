# Open decisions for Daniel

Items parked here need Daniel's call. Work continues elsewhere until he
answers; each item notes the assumption currently built into the code.

| # | Decision | Assumption in code until decided | Raised |
|---|---|---|---|
| 1 | License stays MIT, or switch to Apache-2.0 / GPL for stronger copyleft? | MIT | 2026-10-07 |
| 2 | App name: "Plan Studio" is a working title. Keep it, or pick a brand name? | Plan Studio | 2026-10-07 |

| 3 | Chief library content (.calib/.calibz): Plan Studio will read Core/Bonus/Manufacturer catalogs from the local Chief install at runtime (licensed to Daniel), never bundling them in the repo. OK, or should imported items be re-saved into a user library folder outside the repo? | Read at runtime from Chief's folders; our own starter symbols ship in the repo | 2026-10-08 |

## Queued work (not decisions, just the running order)

1. Hotkeys: bind Chief's full hotkey table (all chords) plus Daniel's custom
   hotkeys from his Chief data folder; add Tools ▸ Customize Hotkeys dialog.
2. Round 2 editing engine from docs/parity/select-and-edit.md and walls.md.
3. 3D view integration (plan-view3d) with Chief's camera tools.
