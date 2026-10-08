# plan-app (Plan Studio)

A desktop 2D floor-plan editor built with egui/eframe on top of `plan-core`.
Draw walls, drop in doors and windows, and see rooms and areas update live.

Run it with `cargo run -p plan-app` (binary name: `plan-studio`).

Controls:
- Tools: `1` Select, `2` Wall, `3` Door, `4` Window. Toggle Exterior/Interior in the toolbar.
- Wall tool: click to place points (continuous chain). Snaps to endpoints, the snap grid and 15 degree angles; hold Alt to skip angle snap. Esc or right-click ends the chain.
- Door/Window: click on a wall. Select: click a wall; Delete/Backspace removes it.
- View: scroll or pinch to zoom at the cursor; middle- or right-drag to pan.
- File menu: New, Open, Save, Save As (`.psplan` JSON files).
