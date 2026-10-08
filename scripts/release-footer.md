
---

### Installing

- **macOS**: open the `.dmg` and drag Plan Studio onto Applications. The app is ad hoc signed, not notarized, so Gatekeeper refuses a downloaded copy until you right-click it and choose Open (or run `xattr -dr com.apple.quarantine "/Applications/Plan Studio.app"`). Use `macos-arm64` on Apple silicon and `macos-x86_64` on Intel.
- **Windows**: unzip `Plan.Studio-*-windows-x64.zip` and run `plan-studio.exe`. The program is unsigned, so SmartScreen warns once (More info, Run anyway).
- **Linux**: unpack the `.tar.gz` and run `./install.sh` (per-user, no root), or run the `.AppImage` if the release has one (`chmod +x` first). Both need the system GTK 3, xkbcommon, X11 or Wayland and OpenGL libraries.
- `SHA256SUMS.txt` lists a checksum for every file. The `.psplan` files are sample plans.
