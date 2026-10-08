## What this changes

<!-- One or two sentences. Link the issue or the parity ids (W-21, DW-8, CB-29) this covers. -->

## How it was tested

<!-- Unit tests, tool-level tests, a headless scenario in crates/plan-app/src/scenarios/, or a manual check. Name the tests. -->

## Checklist

- [ ] The change is small and does one thing.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass.
- [ ] New behavior has tests; a bug fix has a test that failed before the fix.
- [ ] Parity rows in `docs/parity-status.md` and `docs/parity/*.md` are updated with evidence (file, function, test), and `python3 scripts/parity-score.py --check-totals` passes.
- [ ] Anything uncertain about Chief's behavior is a new row in `DECISIONS.md` and the parity row says "verify in Chief".
- [ ] The matching manual chapter and `CHANGELOG.md` (`[Unreleased]`) are updated.
- [ ] No Chief Architect content (`.plan`, `.layout`, `.calib`, `.calibz`, textures, templates, thumbnails) is added, and screenshots show none.
- [ ] No new third-party crate in a library crate, and no GUI code outside `plan-app` and `plan-view3d`.
- [ ] One user action is one undo step.
- [ ] Wiring I could not do in a shared file is listed in `docs/integration-queue.md`.
