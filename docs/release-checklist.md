# Release checklist

How to cut a Plan Studio release, what the release workflow produces, the licensing check,
and the manual QA pass that is still owed. Work top to bottom; tick as you go.

Status when this was written (Rounds 10, 11 and 12 in the working tree on top of the Round 9 commit `0ceb404`, not committed yet): no version has ever been
tagged, the workspace is `0.1.0`, and the live manual QA pass in section 4 has **not** been done.
The automated gate (about 3,120 tests, counted from the source; run `cargo test --workspace` for the exact number) is expected to pass; that is not the same as a person using the program.
Sections 4.7 (textures), 4.11 (file management) and 4.12 (Round 12: Plan Check, Chief plan import, toolbar sets, the Help viewer, curved walls, sections, rendering quality and the rest) are new and have never been run by hand.

## 1. The gate (must be green before you tag)

The release workflow first checks the tag and the changelog, then runs `cargo test --workspace --locked` on Ubuntu, macOS and Windows before it builds or packages anything (a failing test stops the release), but CI is the full gate. Make sure the commit you will tag has passed CI on `main`
(`.github/workflows/ci.yml`: fmt, clippy and tests on Ubuntu, macOS and Windows), and run the
same three commands locally on the machine you are releasing from:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] CI is green on the exact commit to tag, on all three operating systems.
- [ ] The three commands above pass locally, and so does `scripts/test-packaging.sh` (the packaging scripts against a stand-in binary: ICO, Windows zip, Linux tarball and AppImage staging, changelog extraction; seconds, no compile).
- [ ] `git status` is clean and you are on `main`, up to date with `origin/main`.
- [ ] The hotkey list is current: the `plan-config` test fails if `docs/chief-hotkeys-resolved.md` is stale
      (regenerate with `cargo run -p plan-config --example gen_hotkeys_md > docs/chief-hotkeys-resolved.md`).
- [ ] Section 4 (manual QA, including 4.12) is done and its findings are fixed or written down as known issues in the release notes.
- [ ] The manual builds into the program: `cargo build -p plan-app` embeds `docs/manual/*.md` (`crates/plan-app/build.rs`); open Help > Launch Help and see all 18 chapters in the tree.
- [ ] Section 3 (licensing) is confirmed.
- [ ] `python3 scripts/parity-score.py --check-totals` passes (the Totals table in `docs/parity-status.md` must match its per-id rows), and `python3 scripts/brand-sweep.py --strict` has been run with every listed `literal` and `doc` hit reviewed: anything that reads as a marketing comparison rather than naming a file format or an import source is reworded, or added to `scripts/brand-sweep-allow.txt` with a reason.

## 2. Cut the release

### 2.1 Choose the version

Use `X.Y.Z` (for example `0.1.0`). A tag with a hyphen (`v0.2.0-rc1`) is published as a **pre-release**; any
other tag is a normal release. The tag must start with `v` for the workflow to run.

### 2.2 Bump the version

The version lives in one place that matters to cargo, and in two that do not follow it automatically.

1. `Cargo.toml` (repository root), `[workspace.package]`, `version = "0.1.0"`. All 22 crates take it with
   `version.workspace = true`, so this one edit versions every crate. The program's About dialog prints it
   (`env!("CARGO_PKG_VERSION")`).
2. `scripts/macos-bundle.sh` reads the version from the workspace `Cargo.toml` (the `version = "..."` line) and writes it
   into `CFBundleVersion` and `CFBundleShortVersionString`, so there is nothing to edit there. The Windows zip name
   (`scripts/windows-package.sh`), the Linux file names and the release workflow's tag check all read the same line through
   `scripts/version.sh`, so one edit versions every package. The release workflow runs it
   to build the `.app`, and `scripts/macos-dmg.sh` packs the signed app. If the app icon changed, run
   `cargo run -p plan-app --example gen_app_icon` and commit the files it writes in `crates/plan-app/assets/icons/app/`.
3. `Cargo.lock` records every workspace crate's version. After step 1 run `cargo check --workspace` and commit
   the updated `Cargo.lock`.
4. Documents that name the version: `docs/manual/00-index.md` ("version 0.1.0") and the example tag in
   `docs/manual/01-getting-started.md`.

- [ ] Version bumped in `Cargo.toml` and `Cargo.lock`.
- [ ] `CHANGELOG.md`: rename `[Unreleased]` to the new version and date (`## [0.1.0] - 2026-MM-DD`), and open a fresh empty `[Unreleased]`. The release workflow publishes that section as the release notes and **fails the `verify` job if it is missing** (for a final release; a pre-release tag such as `v0.2.0-rc1` falls back to `[Unreleased]`). Check it locally: `sh scripts/release-notes.sh 0.1.0`.
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

The workflow has four jobs, in this order. Nothing is published unless every earlier job succeeded.

1. **`verify`** (Ubuntu, seconds): the tag, minus its `v` and any `-rc` suffix, must equal the workspace version in `Cargo.toml`
   (`scripts/version.sh`); `scripts/release-notes.sh` must find a `CHANGELOG.md` section for it; and `scripts/test-packaging.sh` must pass.
2. **`test`** (Ubuntu, macOS, Windows): `cargo test --workspace --locked`. `--locked` means a `Cargo.lock` that is out of step with the
   `Cargo.toml` files fails the release, so commit the updated lock file (section 2.2). The suite runs once per operating system here, not once per package.
3. **`build`** runs four times in parallel (`fail-fast: false`, so one failing platform does not cancel the
   others), each time `cargo build --release --locked -p plan-app --target <triple>` on a fresh runner with the stable Rust
   toolchain. The release profile is thin LTO with one codegen unit (`Cargo.toml`). Packages, where
   `$TAG` is the tag you pushed (for example `v0.1.0`) and `<version>` is the same without the `v`:

| Matrix name | Runner and target | File produced | Contents |
|---|---|---|---|
| `macos-arm64` | `macos-latest`, `aarch64-apple-darwin` | `plan-studio-$TAG-macos-arm64.zip` and `plan-studio-$TAG-macos-arm64.dmg` | `Plan Studio.app` made by `scripts/macos-bundle.sh` (binary `plan-studio`, `AppIcon.icns`, and an Info.plist with identifier `com.danielallendesigns.plan-studio`, declaring the `.psplan` document type so Finder opens plans in it), ad hoc signed with `codesign --force --deep -s -`, zipped with `ditto`; the `.dmg` (made by `scripts/macos-dmg.sh` with `hdiutil`) holds the same signed app and an Applications shortcut |
| `macos-x86_64` | `macos-latest`, `x86_64-apple-darwin` (cross-compiled; the runner is Apple silicon) | `plan-studio-$TAG-macos-x86_64.zip` and `plan-studio-$TAG-macos-x86_64.dmg` | The same `.app` layout and disk image for Intel Macs |
| `windows-x86_64` | `windows-latest`, `x86_64-pc-windows-msvc` | `Plan Studio-<version>-windows-x64.zip` (published as `Plan.Studio-<version>-windows-x64.zip`, see the `release` job) | One folder with `plan-studio.exe`, `plan-studio.ico` (packed from the PNG sizes in `crates/plan-app/assets/icons/app/` by `scripts/make-ico.py`, standard library only), `LICENSE` and a `README.txt`; made by `scripts/windows-package.sh` under Git Bash |
| `linux-x86_64` | `ubuntu-latest`, `x86_64-unknown-linux-gnu` (after installing `libgtk-3-dev`, `libxkbcommon-dev`, `libwayland-dev`, the xcb render, shape and xfixes dev packages and `libgl1-mesa-dev`) | `plan-studio-$TAG-linux-x86_64.tar.gz`, and `plan-studio-$TAG-linux-x86_64.AppImage` when the AppImage step works | The tarball holds a folder with `plan-studio`, `plan-studio.desktop`, `plan-studio.png`, `plan-studio-psplan.xml` (the `.psplan` MIME type), `install.sh` (per-user install into `~/.local`, `--uninstall` to remove) and `LICENSE`; made by `scripts/linux-appimage.sh`. The AppImage is built by the same script when `appimagetool` is available: the workflow downloads `appimagetool` 1.9.0 from its GitHub release in a **best-effort** step (`continue-on-error`), and if the download or the build fails the release carries the tarball only and the build log shows a warning |

Each package is also kept as a workflow artifact named `plan-studio-<matrix name>` (`if-no-files-found: error`).

4. **`release`** runs after **all four** build jobs succeed (`needs: build`; if any build fails, nothing is published).
   It downloads the four artifacts into `dist/`, replaces the spaces in file names with dots (the name GitHub would give the
   Windows zip anyway, so the names match the checksums), writes `dist/SHA256SUMS.txt`, and with `softprops/action-gh-release@v2` creates a **GitHub Release for the tag**
   named "Plan Studio <tag>" with these files attached:

- the packages above (the two `.dmg` images, the macOS and Windows zips, the Linux tarball and, if built, the AppImage),
- `SHA256SUMS.txt`, and
- the three sample plans: `samples/ranch-3bed.psplan`, `samples/studio-adu.psplan`, `samples/two-story-colonial.psplan`.

The release notes are the **`CHANGELOG.md` section for the version** (`scripts/release-notes.sh`, which drops link-reference lines), followed by the install notes in
`scripts/release-footer.md` (Gatekeeper, SmartScreen, Linux libraries; edit that file to change them). GitHub's generated notes are switched off. It is marked a pre-release when the tag contains a hyphen.
The `release` job has `contents: write`; the others have read-only access. `fail_on_unmatched_files: true` makes the job fail rather than publish
a release with a missing file.

Pull requests and `main` are checked too (`ci.yml`): the packaging scripts self-test runs on Ubuntu and Windows for every pull request, and a
`release-smoke` job (push to `main` and manual runs only, because the thin-LTO release build is slow) does `cargo build --release --locked -p plan-app` and then
the real packaging step on Ubuntu, macOS and Windows, so a release-profile or packaging break shows up on `main` and not on the day of the tag.
Caches are keyed per job (`ci-test`, `ci-release-smoke`, `release-test`, `release-<triple>`) on top of the action's own OS, toolchain and `Cargo.lock` keys, and
only `main` writes the CI caches.

What the workflow does **not** produce: no `.msi`, `.deb` or `.rpm`, no notarized macOS build or disk image,
no signed Windows binary, and no ARM builds for Windows or Linux. The Windows `.exe` has no embedded icon (embedding needs a resource compiler step such as the `winres` crate, which is not in the workspace graph; the `.ico` ships next to the exe for shortcuts). Consequences, all repeated in the release footer:

- The Mac app is ad hoc signed, not notarized. Gatekeeper will refuse a downloaded copy until the user right-clicks it and chooses Open
  (or clears the quarantine flag with `xattr -dr com.apple.quarantine "Plan Studio.app"`).
- Windows SmartScreen will warn about the unsigned `plan-studio.exe`.
- The Linux binary and the AppImage need the GTK 3, xkbcommon, Wayland or X11 and OpenGL runtime libraries of the distribution.

### 2.5 After the workflow finishes

- [ ] The release page lists the macOS zips and `.dmg` images (both architectures), the Windows zip, the Linux tarball (and the AppImage, or the build log says why not), `SHA256SUMS.txt` and the three `.psplan` samples; the body is the changelog section plus the install notes. `sha256sum -c SHA256SUMS.txt` passes in a folder holding the downloads.
- [ ] Open a `.dmg` on a Mac: it shows `Plan Studio.app` and an Applications shortcut; drag the app to Applications and launch it from there (right-click > Open the first time). The app has the Plan Studio icon in the Dock, in Finder and on `.psplan` files; Help > Launch Help opens the manual inside the program with a chapter tree on the left, and Help > About Plan Studio shows the new version, the MIT license and the Chief notice. The release page has both `plan-studio-$TAG-macos-arm64.dmg` and `plan-studio-$TAG-macos-x86_64.dmg`; open the one that matches your Mac.
- [ ] Read the release body: it already holds the `CHANGELOG.md` section and the Gatekeeper and SmartScreen notes; fix typos there and in `CHANGELOG.md`.
- [ ] Download each package you can run. The About dialog shows the new version; File > Open Plan loads a sample; a wall, a door and a save and reopen work.
- [ ] Windows and Linux are built by CI but have **not** been tried by hand yet (ROADMAP). Do not announce them as supported until someone has run section 4 on each.
- [ ] Announce, and bump any download link.

### 2.6 Dry run for v0.1.0 (do this before the real tag)

Nothing here creates a tag or a release on `origin`.

1. **Scripts, locally (a minute)**: `scripts/test-packaging.sh` prints five `ok:` lines. On a Mac also run the real bundle:
   `cargo build --release -p plan-app`, `scripts/macos-bundle.sh release /tmp/pkg`, `scripts/macos-dmg.sh /tmp/pkg /tmp/pkg test.dmg`, open the image, delete `/tmp/pkg`.
2. **Changelog**: `sh scripts/release-notes.sh 0.1.0` prints the section you expect (after renaming `[Unreleased]`) and the exit code is 0.
3. **Version**: `sh scripts/version.sh` prints `0.1.0`.
4. **Lock file**: `cargo check --workspace --locked` passes (the release uses `--locked`).
5. **Whole pipeline on a throwaway tag** (optional, on a fork or after deleting afterwards): push a pre-release tag such as `v0.1.0-rc1` after setting
   the workspace version to `0.1.0` (the tag check ignores the `-rc1` suffix; a pre-release tag falls back to `[Unreleased]` if the changelog has no section for it).
   The run produces a pre-release; check that each package opens (section 2.5), then delete the release and the tag
   (`git push origin :refs/tags/v0.1.0-rc1`). The first run is also the first test of the `appimagetool` download, the Git Bash packaging of the Windows zip
   and the `--locked` builds on the runners; none of those have run yet, so expect to fix something on the first try.
6. **Then** the real `v0.1.0`: section 2.3.

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

The scenario tests (24 files, about 210 tests) drive the tools through synthetic events and check the model; nobody has yet sat at the real window and worked
through a house. Do this on **each** operating system you will announce, starting with macOS. Use a fresh plan (File > New Plan) and keep the status bar in view:
each step names what should happen. Anything that looks wrong goes in `docs/qa-findings.md` with a repro.

### 4.1 Walls to room

- [ ] Draw a 40' x 30' exterior shell with Straight Exterior Wall (`Shift+Q`): click-click chain, closing back on the first point. A room forms and its label shows an interior area close to the walls' inside faces.
- [ ] Add two interior partitions (`Ctrl+Alt+Cmd+6`, Ctrl+Alt+6 off macOS): T-junctions join cleanly and more rooms appear.
- [ ] Select Objects (`Space`): click a wall, drag it (it moves perpendicular), drag an end handle, type a length in a temporary dimension, marquee several walls, Tab to cycle.
- [ ] Undo (`Cmd+Z`, `Ctrl+Z` off macOS) and redo (`Cmd+Y`) step through the above; each step has a name in Edit.
- [ ] Draw a curved wall (three clicks), a half wall and a railing.
- [ ] Typed input: after the first click of a wall type `12'6`, `Tab`, `90`, `Enter`: a wall exactly 12'-6" long at 90 degrees. Hold `Shift` and `Alt` while drawing and check the angle and the snap. Edit > Snap Settings: turn Endpoint off and on, change the angle increment.
- [ ] Wall edit commands: Break Wall, Remove Break, Reverse Layers, Change Line/Arc (drag the bulge handle), Make Arc Tangent, Convert to Polyline, Fix Wall Connections.
- [ ] Edit menu: Copy, then Paste (the copy hangs on the pointer; click to drop, `Esc` cancels), Paste Hold Position, `Cmd+D`, `Cmd+A`, Group and Ungroup, Select Same Type, Delete Objects (`Shift+Space`), Transform/Replicate with copies, Reflect About Object, Align and Distribute, Lock and Unlock, Send to Layer, Action History. Right-click an object and empty space.

### 4.2 Doors and windows

- [ ] Hinged Door (`D, H` or `3`): click near the start of a wall with the pointer inside the building, then near the end with the pointer outside. The two doors swing to opposite sides and hinge at opposite jambs (QA-01).
- [ ] Window (`Shift+W`): placed with its sill; an overlap is refused with a message.
- [ ] Slide an opening along its wall, click the swing handle, Reverse Swing on the Edit toolbar.
- [ ] Drag a door onto another wall: it re-hosts.
- [ ] Every Door and Window flyout entry (Doorway, Sliding, Pocket, Bifold, Barn, Fixed, Garage, Shower, Double; Bay, Bow, Box, Pass-Through, Wall Niche, Casement, Sliding, Awning, Hopper): the symbol, the label (`3068` or the `D01` mark), and the 3D unit. Drag a jamb handle and type a width; Mull two windows and Unmull; Center on Wall Segment; Flip Hinge; Reverse Side on a bay.
- [ ] Round 11 tabs: on a window set Sash, a Diamond and a Prairie lite style, an exterior Lintel and Sill, and an Arch (Round Top, then Gothic); on a door turn Hardware on and add shutters on an exterior wall; check them in 3D and in an elevation. Mull a door with a window beside it (a sidelite): one frame post and one casing round the pair. Door Panels > Calculate from Width: a hinged door widened past 40" becomes a double door. Turn on Snap to standard widths in Default Settings > Doors, drag a jamb and see the width land on a standard one (hold `Alt` to skip it). Drag a label by its handle and use Reset Label Position; hide the Doors, Labels layer.

### 4.3 Dialogs

- [ ] Double-click a wall, a door, a window and a room: each Specification opens with the right tabs; change a value, OK, undo restores it; Cancel changes nothing.
- [ ] Room Specification: set a Ceiling Height and a Floor Height; check them in 3D (4.7).
- [ ] Wall Type Definitions, Default Settings (walls, doors, windows, dimensions, room types, text styles, Preferences > Templates), Customize Hotkeys (assign, conflict, Reassign, Reset).
- [ ] Layer Display Options: hide, lock and unhide a layer; switch a layer set and a saved view.
- [ ] Dimensions: Manual, Auto Exterior (`Shift+A`: three strings per side, also on a house turned 30 degrees), Auto Interior with an openings string; move a wall and see the dimension follow; double-click one (Located Objects, Show Extension Line, Text Style); type a new text; a Printed Size text style at two sheet scales. Text (`Y`), Rich Text, a Callout and a Note. A CAD box, polyline, arc, spline; Fillet, Offset, Trim; make and insert a CAD block; Hatch.
- [ ] Round 11 dimensions: on a kitchen run of base cabinets with a sink, the dimension tool's Auto NKBA button gives a cabinet-faces string, a sink-center string and an overall string; Auto Exterior on a house with a curved bay; Reverse, Convert to Manual, Align and Distribute on a few selected dimensions (each one undo step); in Snap Settings turn Extension on and start a wall in line with another; Points/Markers snaps to a CAD point; export a DXF of a plan with a printed-size text style and a dimension with a hidden extension line.
- [ ] Project Information (Tools menu): fill every tab, OK, reopen: values persist; Undo reverts.
- [ ] Preferences (`Cmd+,`): change Text size, the selection color and a snap default; restart and confirm they stay. View > Status Bar and Toolbars hide and show.
- [ ] Rooms and floors: Floor Defaults (`Shift+Cmd+Y`); Build New Floor with each derive option and a foundation; Insert New Floor Below; the Reference Display dialog; name a room Garage (floor 24" lower), Deck and Open Below; drag a room label; type a label template with `<name> <area>`.
- [ ] Round 11 rooms: turn Roof Over This Room off for a courtyard and Build Roof (the roof leaves a hole or its edge moves to the partition); Flat Roof Over This Room on another room (a level plane at its ceiling); a Garage shows a concrete stem wall in 3D, interrupted at its garage door; with Reference Display on, a wall drawn on the 2nd floor snaps to the ends and crossings of the 1st floor's walls, and turning a layer's Ref box off in Layer Display Options stops it.

### 4.4 Cabinets and the library

- [ ] Base Cabinet (`Shift+T`) and Wall Cabinet (`Cmd+T`) along a wall; Tab changes kind; a filler, a corner cabinet; Generate Countertop (`G`).
- [ ] Cabinet Specification: front, side and back face editors (drag a divider), door and drawer styles, labels. Drag a cabinet's depth and corner handles; drag one into a gap that is nearly its width; touch two base cabinets and see one countertop (Preferences > Architectural).
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

- [ ] Build Roof (`Ctrl+Alt+Shift+Cmd+N`): hip, gable and shed edges; 3D shows a closed roof. On the Roof tab of an exterior wall set Full Gable, Dutch Gable, a Knee Wall and an Upper Pitch and rebuild.
- [ ] Roofs and walls in 3D: a gable end rises to a triangle, a hip roof clips the walls, an interior wall under a vault rises to it, a lower roof butting a taller wall is trimmed with flashing and gets an attic wall above; fascia, soffit and rake boards show.
- [ ] Roof Defaults (Edit > Default Settings > Roofs): set the Eave Cut to Square, turn Exposed Rafter Tails and Gutters on and OK with "Also use for the roofs in this plan": the roof in 3D shows the cut, the tails (no soffit under them) and the gutters; then set one plane's Gutters to Off on its Options tab. Auto Attic Walls off removes the attic wall over a lower roof; Roof Cuts Wall at Bottom cuts a second-floor wall along the first-floor roof. A half wall under a gable stays low.
- [ ] Roof Plane, Edit (reshape), Gable/Roof Line, Roof Hole, Skylight, Auto Dormer, Explode Dormer, Join Roof Planes, Ceiling Plane; each undoes.
- [ ] The status bar names each roof mode as you switch (QA-07).

### 4.7 3D

- [ ] Perspective Full Overview (`Shift+K`), Doll House, Full Camera (`Shift+J`: W A S D, arrows), an elevation and a cross section; `Esc` returns to the plan.
- [ ] Cabinets, stairs, slabs, roofs, trim and a Chief object appear; changing a wall rebuilds the view.
- [ ] Terrain: perimeter, a few elevation points, a Hill, Build Terrain, a road, a driveway, a terrain wall and curb, a garden bed, grass, a water feature, stepping stones, a plant run and a sprinkler run. Contours draw in the plan; the terrain and landscape show in 3D (green Grass, Mulch and Foliage).
- [ ] Images: Create Image with a PNG and a JPEG (in the plan the JPEG draws as a frame with a note; in 3D both show their own picture), a billboard, Create Image Library, a distribution path and region (move the record and the copies follow), 3D Solid Feature.
- [ ] Click an object in 3D (select, Shift-click, double-click, `Delete`). Materials..., Material Painter, Adjust Materials and Material Builder: a painted object shows the texture of the closest scene material.
- [ ] **Textures, with Chief's folders present.** On a machine with Chief Architect Premier X18 installed (and its Data folder in `~/Documents`): open a plan with siding or brick walls, a shingle roof, a wood floor and a lawn in the Standard technique with the 3D bar's Textures box ticked. The walls, roof, floor and grass show Chief's pictures at a believable size (`Brick(36).jpg` repeats about every 3 feet), siding courses run level on every wall and shingles run down each roof plane. Untick Textures: flat colors return. Switch to Physically Based and Ray Trace a small image: the same textures, with about the same brightness as the flat render. Turn on Create Image with a JPEG picture: it shows in 3D.
- [ ] **Textures, with Chief's folders absent.** Best on a machine with no Chief at all. On a machine that has it, rename `~/Documents/Chief Architect Premier X18 Data/Textures` and (needs administrator rights) `/Library/Application Support/Chief Architect Premier X18/Referenced Files` aside, and relaunch: the view still shows textured walls, roof, floor and grass, now generated ones, with no error or blank materials. Set `PLAN_STUDIO_TEXTURES` to a folder holding one of the named files and check that it wins. Look for stalls: a big plan should open its 3D view and fill textures in over a moment, not freeze. The other techniques (Clay, Vector View, Line Drawing) never show textures. Put the folders back afterwards.
- [ ] Rendering techniques and Sun Angle; Ray Trace a small image to PNG; glTF export opens in another viewer.
- [ ] Vector View elevation and its Send to Layout button; Auto Elevations; a Walkthrough path (play, then record a short sequence).
- [ ] The room with a custom ceiling height and floor offset (4.3) shows the right levels.

### 4.8 Layout and schedules

- [ ] Place a Door Schedule, Window Schedule and Room Schedule in the plan; D01 and W01 callouts appear; the Room Schedule area equals the plan label (interior area); drag, select with Select Objects, marquee, Delete and undo a schedule.
- [ ] File > New Layout: page tabs, `S, L` Send to Layout (plan view at "Largest that fits"), an elevation camera box, move and resize boxes by handles, Layout Box Specification, Page Setup, add and delete pages.
- [ ] One undo stack: make a plan edit, then a layout edit; Undo steps back through both in order, from either view.
- [ ] The title block shows the Project Information values on every page.
- [ ] Add a text box, a picture (PNG), a Materials List box, a perspective box (Send to Layout with a Full Camera open; Update Layout Views), a line, a rectangle and a page text; rotate a box with the knob; switch Page Setup to Portrait.
- [ ] Round 11 layout: draw a Circle, an Arc (three clicks), a Leader and a Revision Cloud on a page; move and resize each by its handles and undo; Layers... hides the Revision Clouds layer and the cloud disappears from the page and from the PDF; a Text Box in each fit (Wrap, Shrink to fit, As typed); Add Picture with a PNG and a JPEG; Add Sheet Index; a perspective box at a higher Resolution and Update Views (a progress bar shows and the window stays usable); right-click a camera in the Project Browser > Send to Layout; Create Construction Set adds ten sheets to the layout (one-floor plan with materials) and offers the PDF; Print Model on a perspective camera to a PDF.
- [ ] Schedules: group a Door Schedule by Type with a totals line; click a row of a schedule window and see the object selected; place a Stair, Room Finish and Note Schedule.

### 4.9 Print and export

- [ ] File > Print > Print Layout and Export Layout PDF: open the PDF; check sheet size, scale, title block, line weights, hatch and poche fills, and the bookmarks.
- [ ] File > Print > Print...: a PDF on a smaller paper size with Tiling, then Grayscale, then a page range; Open in viewer; on macOS and Linux a real printer; Print Preview; Print Image; with the system printer chosen, the Printer list shows the printers `lpstat -p` finds.
- [ ] Tools > Schedules > Create Construction Set: the sheets appear in the layout (cover with the sheet index, site plan, plans, elevations, section, details, schedules, the Materials List sheet and the framing placeholder: ten for a one-floor plan with materials), and the PDF it offers has the eight-sheet set (cover, plan, two elevation sheets, section, schedules, Materials List, framing placeholder).
- [ ] Tools > Materials List: each category, All floors, the Master List tab (a waste factor, stock lengths, a unit price), Export CSV and Export PDF, Send to Layout.
- [ ] File > Import > Underlay Picture: a PNG, a JPEG and a scanned PDF; calibrate with two points and a distance; opacity, rotation, lock; a DXF import with a layer mapping and Convert to walls.
- [ ] Export DXF and open it in another CAD program; Import a DXF and run CAD to Walls.
- [ ] Framing Takeoff exports a CSV that opens in a spreadsheet.

### 4.10 Files, platform and housekeeping

- [ ] Save, quit, reopen: everything above is still there; open the three samples; open a plan saved before Round 8 (CAD styles and blocks, text macros and note types survive).
- [ ] Windows and Linux only: `Ctrl+Z` is Undo (Down One Floor has no key there; give it one in Customize Hotkeys and confirm it holds after a restart); the settings folder is created; file dialogs open; HiDPI text is readable.
- [ ] macOS only: the downloaded `.app` opens after right-click > Open, and the Dock shows the name "Plan Studio".
- [ ] No panics in the terminal while doing any of the above; if one occurs, copy the message into the finding.

### 4.11 File management (Round 11; checked against `crates/plan-app/src/files.rs` as of 2026-10-08)

This covers safe saves, archives, autosave, crash recovery, the unsaved-changes prompts and opening from outside (manual chapter 12.2a). `files.rs` is in the working tree and not committed, so check this section against the commit you tag. Times in file names are UTC.

- [ ] **Safe save and archive.** Save a plan twice with an edit between: `Archives/<plan name>/<plan name>-<yyyymmdd-hhmmss>.psplan` appears beside the plan holding the first version, and the plan file holds the second. Save ten more times: only the newest 20 archives stay (set the number in File > Manage Auto Archives, 1 to 500, and see it change). Two saves inside one second keep two copies. Save As over another plan archives that plan's old file.
- [ ] **A failed save changes nothing.** Make the plan's folder read-only (or save to a full volume), edit and Save: the status bar says "Save failed: ...; the file on disk is unchanged", the title keeps its dot, and the file on disk is intact. Make only the `Archives` folder unwritable: the save succeeds and the status bar adds "could not archive the old version".
- [ ] **Title and status bar.** The title reads `Plan Studio — <file>` and gets a dot after an edit; undo back to the saved state and the dot goes; the status bar shows "Saved just now", then "Saved 2 min ago", with ", edited since" after an edit.
- [ ] **Autosave.** Edit without saving and wait for the interval (5 minutes by default; set 1 in Manage Auto Archives to be quick): `Archives/<plan name>/autosave.psplan` appears and the real file is untouched. Undo back to the saved state or Save: the autosave is deleted. An unsaved new plan autosaves to `~/.plan-studio/recovery/untitled-autosave.psplan`. Turning autosave off stops it.
- [ ] **Recover from an autosave.** Edit, wait for the autosave, then kill the program (`kill -9`), reopen the plan from Open Recent: "Recover Unsaved Work" says the autosave is newer than the saved file. Recover (`Enter`) loads it as unsaved (dot in the title) and the next Save removes the autosave. Repeat and choose Discard: the autosave goes and the plan opens as saved. `Enter` never chooses Discard.
- [ ] **Crash copy.** Make unsaved edits and quit with `Cmd+Q` (macOS), or force a panic: `~/.plan-studio/recovery/recovery-<time>.psplan` is written (the newest 10 stay). The next launch offers it ("Plan Studio did not close normally ..."); Recover opens it as an untitled plan, Save asks for a name. Quit an unedited plan, and a new plan nobody touched: nothing is written and nothing is offered.
- [ ] **Unsaved-changes prompts.** With edits, New Plan, Open Plan, Open Recent, Close Plan, File > Quit, the window's close button and a dropped `.psplan` each stop with Unsaved Changes. `Enter` saves and carries on (a cancelled Save As stays put), `Cmd+D` (`Ctrl+D` off macOS) discards, `Esc` cancels. With no edits, none of them asks. No hotkey reaches the plan while a prompt is open.
- [ ] **Revert, copy, backup.** Revert to Saved on an unsaved plan says there is nothing to revert to; on an edited one it asks (`Enter` reverts); undo history is reset. Save a Copy writes `<project> copy.psplan` and the open plan stays the original. Backup Entire Plan into a folder writes `<plan>-backup-<time>.zip`: unzip it and find the plan, `assets/01-...` copies of each underlay and picture, and `README.txt` (move a picture first to see "not found" in the status bar and README).
- [ ] **Manage Auto Archives.** The window lists the plan's newest ten archives; Open on one asks first if there are unsaved edits; Show Archives Folder opens the folder; the settings survive a restart (`files` in `~/.plan-studio/settings.json`). Open Recent lists the last ten, drops a moved plan, and Clear Menu empties it.
- [ ] **Finder double-click (macOS, from the downloaded `.app` that was right-click > Opened).** With Plan Studio **not** running, double-click a `.psplan` in Finder: the app launches and that plan opens (not a blank plan, and no recovery prompt hiding it). With the app running, double-click another `.psplan`: it opens in the running app (after the unsaved-changes prompt if the open plan has edits), `open -a "Plan Studio" file.psplan` does the same, and dropping a `.psplan` on the Dock icon too. The `.psplan` files show the app's document icon and "Plan Studio Plan" as their kind, and Get Info > Open with offers Plan Studio.
- [ ] **Opening from outside, Windows and Linux.** `plan-studio house.psplan` from a terminal opens the plan at launch (also a `file://` path with `%20` in it). On Linux copy `scripts/linux/plan-studio-psplan.xml` from the repository (the release workflow does not put it in the tarball yet, though `plan-studio.desktop` points at it) to `~/.local/share/mime/packages`, run `update-mime-database ~/.local/share/mime`, install the `.desktop` file, and double-click a `.psplan` in the file manager. On Windows, associate `.psplan` with `plan-studio.exe` by hand (there is no installer) and double-click one. Dropping a `.psplan` on the window opens it on every platform; dropping another file says "Drop a .psplan file to open it".

### 4.12 Round 12 (checked against the code as of 2026-10-08; none of it has been run by hand)

- [ ] **Curved walls.** Draw a Curved Exterior Wall between two straight walls: the ends miter and the layer lines (siding, drywall) are exact arcs. Place a door and a window in the arc and look in 3D and in an elevation: the jambs follow the arc and the unit stands square to it. Change its Wall Class to Glass and to Pony; add a roof over it (a gable end follows the roof). Watch the Radius / Arc / Chord readout while setting the arc, and type a radius in the Arc section of the Wall Specification. Known: a straight wall drawn to meet the arc ends square, and the door symbol in plan sits on the chord.
- [ ] **Sections and elevations.** Cross Section/Elevation at an angle (not square to the plan): the vector view is cut along the line. The plan shows a callout bubble with a number; send the view to a layout page and the callout gains `A-n` under a line. In the Camera Specification turn on Automatic dimensions, Material labels and Line weights from the layers, then Export drawing as DXF and open the DXF in another program (layers by weight). 3D View Defaults: change the Callout shape and size. Auto Interior Elevations: click inside a room and get four wall elevations; click again and see no duplicates.
- [ ] **Plan Check.** Tools > Checks > Plan Check on a house with a deliberately small bedroom, a steep stair and no smoke alarm: Previous / Next / Zoom to select the object; Ignore removes a finding and Restore Ignored (n) brings it back; Settings: change a limit (the jurisdiction becomes Custom), switch a rule off, choose the IRC 2021 preset again; Save Report (Markdown), Report PDF, Add to Layout (needs a layout). Close and reopen the plan: settings and the ignore list are still there.
- [ ] **Chief plan import** (needs a project `.plan` from your own Chief install; never commit it). File > Import > Chief Plan on a 2-3 floor project: floors, walls, doors, windows, named rooms, dimensions and text appear; the status bar gives the counts and the notes; compare the wall layout against the Chief plan or its PDF; the project saves with Save As. Check that save first if you have unsaved work: the import replaces the open plan without asking.
- [ ] **Toolbar sets.** Open a 3D view, a vector elevation and the layout: each shows its own toolbar set. Tools > Toolbars and Hotkeys > Customize Toolbars: hide a button, add a row with a name, Move Up and Down, Lock Toolbars, Import Chief Toolbar File (any `.toolbar` from your Chief install), Export and Load Exported File; quit and relaunch: the changes persist (`~/.plan-studio/toolbars.json`). Reset This View.
- [ ] **Customize Hotkeys.** Filter by All / Assigned / Unassigned / In conflict; record a key that another command uses and see the conflict warning and Reassign; Import Chief Hotkeys (a `UserHotkeys.xml`); Export as JSON and CSV; Print List opens a PDF.
- [ ] **Help viewer and About.** Help > Launch Help: search a word, follow a link between chapters, Back and Forward, Contents; View Tutorial Guide opens chapter 1; Keyboard Shortcuts opens chapter 13.
- [ ] **Rendering.** In a Standard 3D view: the Shading menu (Shadows, Ambient occlusion, Quality Low / Medium / High, Exposure); orbit and watch for flicker or stalls on a large plan; Technical Illustration edges and Watercolor wash; Sun Angle moves the shadows. Ray Trace: Clear sky and a turbidity change, Exposure, Aperture and Focus, Denoise, then Save Image at Same size, 2x and 4x (the 4x file must open).
- [ ] **Cabinets.** Cabinet tool: `Shift+Tab` walks Vanity, Pantry, Tall Oven, Refrigerator; click a temporary dimension of a selected cabinet and type a width; click a base cabinet into a gap 1-2" off its width and see it fit; Waterfall edge and a full-height backsplash; labels with macros (`<N> <WxD>`) and the `Cabinets, Labels` layer; Opening Indicators in 3D; Edit > Default Settings > Cabinets > Cabinet Defaults.
- [ ] **User Catalog.** Library Browser: New Folder, Add Selection to Library (a cabinet and a symbol), favorites, filters, the 3D preview; Import 3D Model with an OBJ and a glTF (check units and up axis); Export Library, then Import Library into a fresh settings folder (`HOME` pointed at a temp folder) and see the items return.
- [ ] **Terrain.** A terrain wall that cuts the surface (grade step) and a kidney feature with Grade the terrain: cut/fill cubic yards in the Terrain Specification; drag a spline control point; a crowned road with curbs; North Pointer then Sun Angle shadows turn; Scale Bar; Build Terrain reports its stages; turn off the auto-rebuild and see "Terrain out of date".
- [ ] **Layers and plan views.** Layer Display Options: select several layers and hide them in one click; Modify All Layer Sets; Layer Set Management (New, Copy, Rename, Delete, Make Active, Import From Plan File); Active Layers by Tool; Tools > Plan Views: Add Template Plan Views (20), Plan View Specification, Save and Reset Plan View, and tabs above the plan (switch, reorder, middle-click closes). Project Browser shows Floors, Plan Views, Cameras, Schedules, CAD Details and Layout.
- [ ] **Electrical and framing.** Electrical: Auto Place Switches in a two-door room (3-way pair), drag the bend handle of a connection, devices in 3D with their finishes. Framing: Edit > Default Settings > Framing (change the header table and rebuild), corner and tee backing in a wall plan, a stairwell framed with trimmers and headers, rafters with birdsmouths in 3D, Framing Overview and back, the Framing Takeoff by member type.
- [ ] **Print.** With a 3D view open File > Print > Print Image (size dialog, then a PNG); Print Model; Layout menu Layer Display Options and Add Sheet Index.
