# Release checklist

How to cut a Plan Studio release, what the release workflow produces, the licensing check,
and the manual QA pass that is still owed. Work top to bottom; tick as you go.

Status when this was written (after Round 8, commit `6f6a7b9`): no version has ever been
tagged, the workspace is `0.1.0`, and the live manual QA pass in section 4 has **not** been done.
The automated gate (1,852 tests) passes; that is not the same as a person using the program.

## 1. The gate (must be green before you tag)

The release workflow builds and packages. **It does not run the tests**, so a tag on a broken
commit still publishes. Make sure the commit you will tag has passed CI on `main`
(`.github/workflows/ci.yml`: fmt, clippy and tests on Ubuntu, macOS and Windows), and run the
same three commands locally on the machine you are releasing from:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] CI is green on the exact commit to tag, on all three operating systems.
- [ ] The three commands above pass locally.
- [ ] `git status` is clean and you are on `main`, up to date with `origin/main`.
- [ ] The hotkey list is current: the `plan-config` test fails if `docs/chief-hotkeys-resolved.md` is stale
      (regenerate with `cargo run -p plan-config --example gen_hotkeys_md > docs/chief-hotkeys-resolved.md`).
- [ ] Section 4 (manual QA) is done and its findings are fixed or written down as known issues in the release notes.
- [ ] Section 3 (licensing) is confirmed.

## 2. Cut the release

### 2.1 Choose the version

Use `X.Y.Z` (for example `0.1.0`). A tag with a hyphen (`v0.2.0-rc1`) is published as a **pre-release**; any
other tag is a normal release. The tag must start with `v` for the workflow to run.

### 2.2 Bump the version

The version lives in one place that matters to cargo, and in two that do not follow it automatically.

1. `Cargo.toml` (repository root), `[workspace.package]`, `version = "0.1.0"`. All 22 crates take it with
   `version.workspace = true`, so this one edit versions every crate. The program's About dialog prints it
   (`env!("CARGO_PKG_VERSION")`).
2. **`scripts/macos-bundle.sh` hard-codes the version.** Change both `CFBundleVersion` and
   `CFBundleShortVersionString` (currently `0.1.0`) by hand. The release workflow runs this script to build
   the `.app`, so a forgotten edit ships a Mac app whose Info.plist still says the old version.
3. `Cargo.lock` records every workspace crate's version. After step 1 run `cargo check --workspace` and commit
   the updated `Cargo.lock`.
4. Documents that name the version: `docs/manual/00-index.md` ("version 0.1.0") and the example tag in
   `docs/manual/01-getting-started.md`.

- [ ] Version bumped in `Cargo.toml`, `scripts/macos-bundle.sh` (twice) and `Cargo.lock`.
- [ ] `CHANGELOG.md`: rename `[Unreleased]` to the new version and date, and open a fresh empty `[Unreleased]`.
- [ ] Commit ("Release vX.Y.Z") and push to `main`; wait for CI.

### 2.3 Tag and push

```bash
git tag -a vX.Y.Z -m "Plan Studio X.Y.Z"
git push origin vX.Y.Z
```

Pushing the tag starts `.github/workflows/release.yml` (workflow "Release"). Watch it under the repository's
Actions tab. If a build fails, fix it on `main`, delete the tag locally and on the remote
(`git push origin :refs/tags/vX.Y.Z`), and tag again.

### 2.4 What the workflow produces

The workflow has two jobs.

**Job `build`** runs four times in parallel (`fail-fast: false`, so one failing platform does not cancel the
others), each time `cargo build --release -p plan-app --target <triple>` on a fresh runner with the stable Rust
toolchain. The release profile is thin LTO with one codegen unit (`Cargo.toml`). Packages, where
`$TAG` is the tag you pushed (for example `v0.1.0`):

| Matrix name | Runner and target | File produced | Contents |
|---|---|---|---|
| `macos-arm64` | `macos-latest`, `aarch64-apple-darwin` | `plan-studio-$TAG-macos-arm64.zip` | `Plan Studio.app` made by `scripts/macos-bundle.sh` (binary `plan-studio` plus an Info.plist, identifier `com.danielallendesigns.plan-studio`), ad hoc signed with `codesign --force --deep -s -`, zipped with `ditto` |
| `macos-x86_64` | `macos-latest`, `x86_64-apple-darwin` (cross-compiled; the runner is Apple silicon) | `plan-studio-$TAG-macos-x86_64.zip` | The same `.app` layout for Intel Macs |
| `windows-x86_64` | `windows-latest`, `x86_64-pc-windows-msvc` | `plan-studio-$TAG-windows-x86_64.zip` | `plan-studio.exe` alone |
| `linux-x86_64` | `ubuntu-latest`, `x86_64-unknown-linux-gnu` (after installing `libgtk-3-dev`, `libxkbcommon-dev`, `libwayland-dev`, the xcb render, shape and xfixes dev packages and `libgl1-mesa-dev`) | `plan-studio-$TAG-linux-x86_64.tar.gz` | A folder `plan-studio-$TAG-linux-x86_64/` holding the `plan-studio` binary and `plan-studio.desktop` (install notes are in the `.desktop` file: copy the binary to `~/.local/bin` and the `.desktop` file to `~/.local/share/applications`) |

Each package is also kept as a workflow artifact named `plan-studio-<matrix name>` (`if-no-files-found: error`).

**Job `release`** runs after **all four** build jobs succeed (`needs: build`; if any build fails, nothing is published).
It downloads the four artifacts into `dist/` and, with `softprops/action-gh-release@v2`, creates a **GitHub Release for the tag**
with these files attached:

- the four packages above, and
- the three sample plans: `samples/ranch-3bed.psplan`, `samples/studio-adu.psplan`, `samples/two-story-colonial.psplan`.

The release notes are the ones GitHub **generates** from the commits and pull requests since the previous release
(`generate_release_notes: true`); the workflow does not read `CHANGELOG.md`. It is marked a pre-release when the tag contains a hyphen.
The job has `contents: write`; the build jobs have read-only access. `fail_on_unmatched_files: true` makes the job fail rather than publish
a release with a missing file.

What the workflow does **not** produce: no installer (`.dmg`, `.msi`, `.deb`, AppImage), no notarized macOS build,
no signed Windows binary, no checksums, and no ARM builds for Windows or Linux. Consequences to put in the release notes:

- The Mac app is ad hoc signed, not notarized. Gatekeeper will refuse a downloaded copy until the user right-clicks it and chooses Open
  (or clears the quarantine flag with `xattr -dr com.apple.quarantine "Plan Studio.app"`).
- Windows SmartScreen will warn about the unsigned `plan-studio.exe`.
- The Linux binary needs the GTK 3, xkbcommon, Wayland or X11 and OpenGL runtime libraries of the distribution.

### 2.5 After the workflow finishes

- [ ] The release page lists exactly the four packages and three `.psplan` samples.
- [ ] Edit the release body: paste the new `CHANGELOG.md` section above the generated notes, and add the Gatekeeper and SmartScreen notes.
- [ ] Download each package you can run. The About dialog shows the new version; File > Open Plan loads a sample; a wall, a door and a save and reopen work.
- [ ] Windows and Linux are built by CI but have **not** been tried by hand yet (ROADMAP). Do not announce them as supported until someone has run section 4 on each.
- [ ] Announce, and bump any download link.

## 3. Licensing note (check before every release)

- Plan Studio is **MIT** licensed (`LICENSE`, Copyright Daniel Sievers / Daniel Allen Designs). `DECISIONS.md` item 1 records that the license is not final.
- **Chief Architect content is never bundled.** The repository and the release packages contain no Chief Architect code, assets or catalog content.
  The Chief library catalogs (`.calib`, `.calibz`) are read **in place, at runtime, from the user's own Chief install**
  and are never copied, re-saved or shipped (`DECISIONS.md` item 3). `.gitignore` excludes `*.calib`, `*.calibz`, `*.calib_error`, `*.plan` and
  `*.layout` so they cannot be committed by accident; do not remove those lines. Chief templates (`.plan`, `.layout`) are likewise read from the user's install.
- Icons, the built-in 2D symbols (about 145 in `plan-library`) and the three sample plans are original work.
- **What does ship that came from Daniel's own Chief setup:** the compiled-in template defaults
  (`crates/plan-app/assets/templates/chief-x18-daniel.json`: wall types, text styles, dimension sets and heights decoded from his template), his hotkey and toolbar files embedded by
  `plan-config` (`docs/chief-config-raw/`), and the written analyses of Chief's UI (`docs/chief-x18-*.md`). These are configuration, facts and descriptions, not Chief code or
  library content, but they are derived from a licensed product, so before the first public release the owner must confirm that publishing them is acceptable.
- Brand names (Chief Architect, Chief) are used only to describe compatibility. Keep the sentence "No Chief Architect code, assets or catalog content ships in this repository" true
  in `README.md` and `docs/manual/00-index.md`.
- [ ] No `.calib`, `.calibz`, `.plan` or `.layout` file is tracked: `git ls-files | grep -Ei '\.(calibz?|plan|layout)$'` prints nothing.
- [ ] The embedded Daniel files above are confirmed publishable by the owner.
- [ ] The release notes say Chief catalogs are read from the user's own install and are not included.

## 4. Manual QA pass (still pending)

The 96 scenario tests drive the tools through synthetic events and check the model; nobody has yet sat at the real window and worked
through a house. Do this on **each** operating system you will announce, starting with macOS. Use a fresh plan (File > New Plan) and keep the status bar in view:
each step names what should happen. Anything that looks wrong goes in `docs/qa-findings.md` with a repro.

### 4.1 Walls to room

- [ ] Draw a 40' x 30' exterior shell with Straight Exterior Wall (`Shift+Q`): click-click chain, closing back on the first point. A room forms and its label shows an interior area close to the walls' inside faces.
- [ ] Add two interior partitions (`Ctrl+Alt+Cmd+6`, Ctrl+Alt+6 off macOS): T-junctions join cleanly and more rooms appear.
- [ ] Select Objects (`Space`): click a wall, drag it (it moves perpendicular), drag an end handle, type a length in a temporary dimension, marquee several walls, Tab to cycle.
- [ ] Undo (`Cmd+Z`, `Ctrl+Z` off macOS) and redo (`Cmd+Y`) step through the above; each step has a name in Edit.
- [ ] Draw a curved wall (three clicks), a half wall and a railing.

### 4.2 Doors and windows

- [ ] Hinged Door (`D, H` or `3`): click near the start of a wall with the pointer inside the building, then near the end with the pointer outside. The two doors swing to opposite sides and hinge at opposite jambs (QA-01).
- [ ] Window (`Shift+W`): placed with its sill; an overlap is refused with a message.
- [ ] Slide an opening along its wall, click the swing handle, Reverse Swing on the Edit toolbar.
- [ ] Drag a door onto another wall: it re-hosts.

### 4.3 Dialogs

- [ ] Double-click a wall, a door, a window and a room: each Specification opens with the right tabs; change a value, OK, undo restores it; Cancel changes nothing.
- [ ] Room Specification: set a Ceiling Height and a Floor Height; check them in 3D (4.7).
- [ ] Wall Type Definitions, Default Settings (walls, doors, windows, dimensions, room types, text styles, Preferences > Templates), Customize Hotkeys (assign, conflict, Reassign, Reset).
- [ ] Layer Display Options: hide, lock and unhide a layer; switch a layer set and a saved view.
- [ ] Dimensions: Manual, Auto Exterior (`Shift+A`), an interior string; double-click one; type a new text. Text (`Y`), Rich Text, a Callout and a Note. A CAD box, polyline, arc, spline; Fillet, Offset, Trim; make and insert a CAD block; Hatch.
- [ ] Project Information (Tools menu): fill every tab, OK, reopen: values persist; Undo reverts.

### 4.4 Cabinets and the library

- [ ] Base Cabinet (`Shift+T`) and Wall Cabinet (`Cmd+T`) along a wall; Tab changes kind; a filler, a corner cabinet; Generate Countertop (`G`).
- [ ] Cabinet Specification: front face editor (drag a divider), door and drawer styles, labels.
- [ ] Library Browser (`Cmd+L`): search a fixture and place it on a wall (it rotates flush); Replace From Library.
- [ ] If Chief is installed: enable the Chief catalogs, search, see thumbnails, place a Chief object and see it in 3D.

### 4.5 Stairs

- [ ] Draw Stairs (`Shift+Y`): drag a straight run on a two-floor plan (Build New Floor first). The status bar reads the risers and treads; the UP arrow and break line draw.
- [ ] L-Shaped, U-Shaped, Curve to Left and Curved Stairs; Tab flips the turn before drawing; Click Stairs; Draw Ramp.
- [ ] Landing: drag a rectangle; click a polygon; draw a stair that arrives on it and another that starts on it. The landing takes the first stair's height.
- [ ] Select a stair: the Move, Rotate, Run and Width handles work; Esc cancels a drag.
- [ ] Staircase Specification: every tab; set Left Side and Right Side to Railing, Half Wall and Wall; open risers; each stringer style; lock the tread depth and change Top Height; Fit stair to floor-to-floor. The code warnings show in red and do not block OK.
- [ ] Auto Stairwell on a stair that reaches the floor above: the hole shows in 3D and a Stairwell room forms; delete the stair and both go.
- [ ] Look at the stair in 3D from several sides, including the underside.
- [ ] Plan Check lists a stair finding for a deliberately steep stair.

### 4.6 Roofs

- [ ] Build Roof (`Ctrl+Alt+Shift+Cmd+N`): hip, gable and shed edges; 3D shows a closed roof.
- [ ] Roof Plane, Edit (reshape), Gable/Roof Line, Roof Hole, Skylight, Auto Dormer, Explode Dormer, Join Roof Planes, Ceiling Plane; each undoes.
- [ ] The status bar names each roof mode as you switch (QA-07).

### 4.7 3D

- [ ] Perspective Full Overview (`Shift+K`), Doll House, Full Camera (`Shift+J`: W A S D, arrows), an elevation and a cross section; `Esc` returns to the plan.
- [ ] Cabinets, stairs, slabs, roofs, trim and a Chief object appear; changing a wall rebuilds the view.
- [ ] Terrain: perimeter, a few elevation points, a Hill, Build Terrain, a road, a driveway, a terrain wall and curb, a garden bed, grass, a water feature, stepping stones, a plant run and a sprinkler run. Contours draw in the plan; the terrain and landscape show in 3D (brown stand-in materials are expected).
- [ ] Images: Create Image with a PNG and a JPEG (the JPEG draws as a frame with a note), a billboard, Create Image Library, a distribution path and region (move the record and the copies follow), 3D Solid Feature.
- [ ] Rendering techniques and Sun Angle; Ray Trace a small image to PNG; glTF export opens in another viewer.
- [ ] Vector View elevation and its Send to Layout button; Auto Elevations; a Walkthrough path (play, then record a short sequence).
- [ ] The room with a custom ceiling height and floor offset (4.3) shows the right levels.

### 4.8 Layout and schedules

- [ ] Place a Door Schedule, Window Schedule and Room Schedule in the plan; D01 and W01 callouts appear; the Room Schedule area equals the plan label (interior area); drag, select with Select Objects, marquee, Delete and undo a schedule.
- [ ] File > New Layout: page tabs, `S, L` Send to Layout (plan view at "Largest that fits"), an elevation camera box, move and resize boxes by handles, Layout Box Specification, Page Setup, add and delete pages.
- [ ] One undo stack: make a plan edit, then a layout edit; Undo steps back through both in order, from either view.
- [ ] The title block shows the Project Information values on every page.

### 4.9 Print and export

- [ ] File > Print > Print Layout and Export Layout PDF: open the PDF; check sheet size, scale, title block, line weights, hatch and poche fills.
- [ ] Tools > Schedules > Create Construction Set: the cover, plans, elevations, section and schedules are all present.
- [ ] Export DXF and open it in another CAD program; Import a DXF and run CAD to Walls.
- [ ] Materials List and Framing Takeoff export CSV that opens in a spreadsheet.

### 4.10 Files, platform and housekeeping

- [ ] Save, quit, reopen: everything above is still there; open the three samples; open a plan saved before Round 8 (CAD styles and blocks, text macros and note types survive).
- [ ] Windows and Linux only: `Ctrl+Z` is Undo (Down One Floor has no key there; give it one in Customize Hotkeys and confirm it holds after a restart); the settings folder is created; file dialogs open; HiDPI text is readable.
- [ ] macOS only: the downloaded `.app` opens after right-click > Open, and the Dock shows the name "Plan Studio".
- [ ] No panics in the terminal while doing any of the above; if one occurs, copy the message into the finding.
