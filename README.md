# Plan Studio

An open-source residential design CAD application written in Rust, in the
spirit of Chief Architect: you draw walls on a floor plan, and rooms, 3D,
elevations and schedules are generated from that one model.

**Status: pre-alpha.** The data model and 2D plan editor are the first
milestone. See [ROADMAP.md](ROADMAP.md) for where this is going and
[docs/chief-x18-ui-notes.md](docs/chief-x18-ui-notes.md) for the UI study the
design is based on.

![Plan Studio with Chief-style toolbars, menus and the Low Glare theme](docs/screenshot-chief-toolbars.jpg)

Earlier milestone: [phase 0 screenshot](docs/screenshot-phase0.jpg).

## Why

Chief Architect is the reference tool for residential designers, but it is
closed, Windows/macOS only, and expensive. Plan Studio aims at the core
residential workflow, open source, with a modern Rust codebase that others can
extend.

## Architecture

```
crates/
  plan-core   pure data model + geometry (no GUI, fully unit-tested)
  plan-app    desktop editor built on egui/eframe
```

Inside `plan-app`, every Chief tool is one module behind a shared `Tool`
trait (`tools/`), and the services they share (selection, snapping, handles,
temporary dimensions, undo, rendering) live in `editor/`. `main.rs` only owns
the window, panels and dialogs. See
[docs/architecture-tools.md](docs/architecture-tools.md) for the layout, the
trait and the rules for adding a tool.

- **The plan is the model.** `plan-core` holds walls, openings, floors and
  (soon) cabinets, roofs and stairs. Every view is derived from it.
- Lengths are stored in inches as `f64`; `plan_core::units` formats and parses
  architectural feet-and-inches (`12'-6 1/2"`).
- Room detection builds a planar graph from wall centerlines, splits at
  T-junctions and crossings, and traces the bounded faces.

## Building

Install Rust via [rustup](https://rustup.rs), then:

```bash
cargo test -p plan-core      # unit tests for the model
cargo run -p plan-app        # launch the editor
```

## Controls (plan-app)

| Action | Input |
|---|---|
| Tools | `1` Select, `2` Wall, `3` Door, `4` Window |
| Draw walls | click start, click end, keep clicking to chain, or press-drag-release for one wall; `Esc` to stop |
| Select / move | click, `Tab` to cycle, drag a wall to move it perpendicular, drag handles to stretch, drag empty space to marquee |
| Undo / redo | `Cmd+Z`, `Shift+Cmd+Z` or `Cmd+Y` |
| Angle snap | automatic at 15°; hold `Alt` for free angle |
| Zoom / pan | scroll wheel or pinch / middle- or right-drag |
| Delete | select, press `Delete` |

## Contributing

Issues and pull requests are welcome. Keep `plan-core` free of GUI code and
add a unit test with every geometry change. Run `cargo fmt` and
`cargo clippy` before opening a PR.

## License

MIT. See [LICENSE](LICENSE).
