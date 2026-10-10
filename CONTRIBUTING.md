# Contributing to Plan Studio

Thank you for helping. Plan Studio is an open-source Rust re-creation of Chief Architect X18's workflow for residential
design. Read the [Code of Conduct](CODE_OF_CONDUCT.md) first, then this page. For the big picture see the
[README](README.md), chapter 14 of the [manual](docs/manual/14-architecture-for-contributors.md) and
[docs/architecture-tools.md](docs/architecture-tools.md).

## Ground rules

1. **Chief content stays out of the repository.** Never commit or paste Chief Architect `.plan`, `.layout`, `.calib` or
   `.calibz` files, textures, library thumbnails, templates or anything else copied from a Chief install (they are in
   `.gitignore`). Plan Studio reads those from the user's own install at run time and never copies them. Tests use
   synthetic fixtures built at test time. Screenshots must not show Chief library content.
2. **No marketing comparisons.** Name Chief Architect only to describe compatibility (a file format, an import source,
   a behavior we match). `python3 scripts/brand-sweep.py` lists where the product names appear; reword anything that reads as a
   comparison, or add a reasoned line to `scripts/brand-sweep-allow.txt`.
3. **The plan is the model.** Data lives in `plan-core` (typed `#[serde(default)]` slots on `Floor` and `Project`); every
   view is derived from it. If you store the same fact twice, one copy should be computed.
4. **Chief's words and Chief's behavior.** Tool names, dialog tab names, status-bar hints and tooltips use Chief's
   vocabulary. When you cannot confirm Chief's exact behavior, pick the most Chief-like option, record it in
   `DECISIONS.md` and mark the parity row "verify in Chief".

## Toolchain

- A recent stable Rust toolchain from [rustup](https://rustup.rs), with the `rustfmt` and `clippy` components. The
  workspace is edition 2021 and uses egui/eframe 0.31.
- A working OpenGL driver (3.1 or better). On Linux install the GUI development packages:
  `libgtk-3-dev libxkbcommon-dev libwayland-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libgl1-mesa-dev`.
- Python 3 for the scripts in `scripts/` (standard library only).
- macOS is the primary platform; Windows and Linux build and are checked by CI.

```bash
cargo run -p plan-app            # open the editor (binary: plan-studio)
cargo test -p plan-core          # one crate
cargo test --workspace           # everything (takes a while)
```

## Workspace layout

Twenty-two crates under `crates/`; the table in the [README](README.md#architecture) says what each is. In short:

- `plan-core` is the model, geometry, units and snapshot undo. It has no GUI code.
- Engine crates (`plan-roof`, `plan-stairs`, `plan-cabinets`, `plan-electrical`, `plan-terrain`, `plan-framing`,
  `plan-render`, `plan-check`, ...) are plain Rust libraries with their own tests.
- Format and reader crates (`plan-import`, `plan-calib`, `plan-chiefplan`, `plan-config`, `plan-library`) parse files.
- `plan-docs`, `plan-layout` and `plan-elevation` produce schedules, layouts and drawings.
- `plan-view3d` (the OpenGL viewport) and `plan-app` (the editor) are the only crates that depend on egui/eframe.

Inside `crates/plan-app/src/`: `tools/` (one module per Chief tool behind the `Tool` trait), `editor/` (shared services:
selection, snapping, handles, temporary dimensions, undo, rendering), `dialogs/` (specification dialogs and Default
Settings), `shell/` (docks, hotkeys, the dialog host, the 3D panel), `scenarios/` (headless end-to-end tests), plus
`main.rs`, `menus.rs` and `toolbar.rs`.

### Dependency policy for engines

The libraries are deliberately light. Library crates depend only on other workspace crates plus `serde` and `serde_json`
(already in `[workspace.dependencies]`); file formats, image decoders, inflate, the path tracer and the geometry
kernels are written in-house on `std`. Do not add a third-party crate to a library crate. Only `plan-app` and
`plan-view3d` may depend on `eframe`, `egui_extras` or `rfd`. If a feature seems to need a new crate anywhere, open an
issue first (or park it in `DECISIONS.md`) and explain what the standard library cannot do. Generated or reader code
that needs a tool such as `sqlite3` or `zip` in tests must skip itself when the tool is missing.

## Tests

Every feature needs tests, and every geometry change in `plan-core` needs a unit test. Pick the lowest layer that proves
the behavior:

1. **Unit tests** in the crate (`#[cfg(test)] mod tests`). Engines are tested without any window.
2. **Tool and dialog logic tests** in `plan-app`: drive a tool with synthetic `PointerEvent`s on a fresh
   `EditorContext::new(plan_defaults::embedded())` and assert on the `Project`; test dialog form logic (for example
   `WallForm::set_length`) rather than egui drawing.
3. **Headless scenario tests** in `crates/plan-app/src/scenarios/` (`sNN_*.rs`, registered in `scenarios/mod.rs`). They use
   the `Sim` harness to drive the real tools, hotkeys, menus and dialogs through pointer and key events on a `PlanApp`
   with no window, then check the model, the undo stack, the 3D scene and the documents. Add one to cover a whole
   workflow. A scenario that exposes a bug is written to `docs/qa-findings.md` and marked `#[ignore = "QA-nn"]` until fixed.
4. **Generated-document checks**: `docs/chief-hotkeys-resolved.md` is regenerated with
   `cargo run -p plan-config --example gen_hotkeys_md > docs/chief-hotkeys-resolved.md` and a test fails when it is stale.
5. **Tests that need a real Chief install** (`plan-calib`, `plan-chiefplan`, the texture timings) are `#[ignore]` and
   are run by hand with `--ignored`; everything else uses synthetic fixtures.

Keep runs targeted while you work (`cargo test -p plan-roof`, `cargo test -p plan-app walls`) and run the whole workspace
before opening the pull request.

## Formatting, lints and the CI gate

CI (`.github/workflows/ci.yml`) runs these on Linux, macOS and Windows for every pull request. Run them locally:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 scripts/parity-score.py --check-totals   # the Totals table in docs/parity-status.md matches its rows
bash scripts/test-packaging.sh                   # packaging scripts against a stand-in binary (seconds)
```

`cargo fmt --all` fixes formatting. The packaging and parity checks matter if you touch `scripts/`, `.github/` or
`docs/parity-status.md`. On pushes to `main` CI also does a real release build and packaging step on all three systems.

## Several people editing at once

The code base is often edited by several contributors (or tools) in the same tree at the same time, so:

- Edit the files your change needs and no more. Shared files (`main.rs`, `menus.rs`, `toolbar.rs`, `tools/mod.rs`,
  `editor/*`, `shell/spec_dialogs.rs`) get small, targeted edits: re-read the current file, change the few lines you
  need, never reformat or rewrite a shared file whole.
- Format only files you own. If the build breaks in a file you did not touch, wait and retry rather than fixing it
  under someone else's hands.
- If a feature needs wiring in a file you should not change, leave it and append what is needed to
  [docs/integration-queue.md](docs/integration-queue.md) (the file, the function, the one-line change).
- One user action is exactly one undo step. Call `cx.begin_change("Label")` before mutating the model and
  `cx.mark_dirty()` after.

## Parity rows and decisions

The target is measured, not guessed.

- **Parity specs** (`docs/parity/*.md`) describe Chief's behavior with stable ids such as `W-21`. Cite the ids in doc
  comments next to the code (`// W-21`).
- **Parity status** (`docs/parity-status.md`) has one row per id under "Per-id status": status (Works, Partial, Missing,
  Differs-by-design), what the code does, and the evidence (file, function, test name). When you change behavior, update the row
  and its evidence in the same pull request. Works means a test, a scenario or the tool itself exercises it. If the
  per-area Totals table no longer matches the rows, `python3 scripts/parity-score.py --check-totals` says so; fix the table.
  `python3 scripts/parity-score.py` prints the weighted score and the build order of the Missing ids.
- **`DECISIONS.md`** is an append-only table: number, the decision or question, the assumption built into the code, and
  the date. Add a row whenever you had to choose where the spec or Chief's behavior was unclear, never renumber, and when a
  row is settled strike it through and say how. Mark the matching parity row "verify in Chief".
- **Docs**: update the matching manual chapter (`docs/manual/`, which is also built into the program's Help) and add a
  line to `CHANGELOG.md` under `[Unreleased]`.

## How to add a tool

Chapter 14.4 of the manual walks through it in full; the short version:

1. Read the tool's section in `docs/parity/*.md` and its flyout in `docs/chief-x18-subtools.md`.
2. Create `crates/plan-app/src/tools/<name>.rs` with a struct that implements the `Tool` trait (`id`, `name`, `hint`,
   and the pointer, key and overlay methods you need; every other method has a default). Use Chief's name for `name()`.
3. Register it: a `pub mod` line and one line in `registry()` in `tools/mod.rs`; for a flyout family add a
   `ToolId::...Variant(..)` and map it in `ToolId::base()`. The test `registry_covers_every_tool_id_once` checks it.
4. Put it on the toolbar in `toolbar.rs` (an `item(..., Action::SetTool(ToolId::...))` entry). Menus and the Customize
   Hotkeys command list are generated from the same tables, and Chief's hotkey for the command name binds automatically.
5. Change the model through the context: `cx.begin_change`, `cx.snap_at` for snapping, `cx.check_unlocked` for locked
   layers, `cx.fmt_dim` for lengths.
6. Write the engine first when there is an algorithm (a new crate or module with its own tests), then the tool that calls it.
7. Test with synthetic events and add a scenario; update the parity rows and the manual.

## How to add a dialog

Specification dialogs live in `crates/plan-app/src/dialogs/<name>.rs` and use Chief's tab names.

1. Implement `SpecPages` (in `dialogs.rs`): `tabs()` (built with `on("Name")` for live tabs and `off("Name")` for tabs
   that exist in Chief but are not built yet), `page(ui, tab)`, `preview(painter, rect)` and `error()`.
2. Wrap it in `SpecDialog::new("Title", key)` and add one arm to `SpecDialogs::open` in `shell/spec_dialogs.rs` so a
   double-click and `Enter` on the object's `ObjectRef` open it.
3. Controls the model cannot store yet are drawn with `dis_check`, `dis_radio` or `session_check`, never hidden.
4. OK applies the change as one undo step. Test the form logic directly and add a scenario that opens the dialog
   (`Sim::open_spec`), edits it, presses OK and checks the model and undo.

## Pull request checklist

The pull request template asks for the same things:

- [ ] The change is as small as it can be and does one thing.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass.
- [ ] New behavior has tests (unit, tool-level or a headless scenario); a bug fix has a test that failed before.
- [ ] Parity rows are updated with evidence, and `python3 scripts/parity-score.py --check-totals` passes.
- [ ] Anything uncertain about Chief's behavior is a `DECISIONS.md` row and marked "verify in Chief".
- [ ] The manual chapter and `CHANGELOG.md` are updated.
- [ ] No Chief Architect content, no new third-party crate in a library crate, no GUI code in a non-GUI crate.
- [ ] Each user action is one undo step.

## Reporting problems

Use the issue templates for bugs and feature requests. For a security problem, follow [SECURITY.md](SECURITY.md) instead
of opening a public issue.
