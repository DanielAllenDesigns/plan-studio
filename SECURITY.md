# Security policy

## Supported versions

Plan Studio is pre-alpha and no version has been released yet. Security fixes go into the `main` branch and, once
releases exist, into the latest release. Older releases are not patched.

## What counts as a security problem

Plan Studio is a desktop program that reads and writes files on your own machine; it runs no server and has no
accounts. The problems we care most about are:

- A crafted file that crashes the program, hangs it, exhausts memory or runs unexpected code when you open or import it:
  `.psplan` plans, DXF, OBJ, glTF, PNG and JPEG images, PDFs used as underlays, Chief `.plan`, `.layout`, `.calib` and
  `.calibz` files, templates, hotkey and toolbar files, and `.calibz` library zips.
- Writing outside the folder you chose (path traversal in a library zip or an export).
- The release packages or the build and release scripts doing something other than what they say.
- Leaking private files or paths from your machine into a plan, an export, a PDF or a crash copy.

Ordinary bugs (a wrong dimension, a crash on valid input that cannot be exploited) belong in the public issue tracker.

## Reporting a vulnerability

Please do not open a public issue for a security problem. Report it privately through GitHub:

1. Go to <https://github.com/DanielAllenDesigns/plan-studio/security/advisories/new> (the repository's Security tab,
   "Report a vulnerability").
2. Describe what happens, the version or commit, your operating system, and the steps to reproduce. Attach a small
   sample file if one triggers it. Do not attach Chief Architect files or other material you are not allowed to share;
   describe the file instead.

You will get an acknowledgement within a few days. We will confirm the problem, fix it on `main`, and credit you in the
changelog if you wish. Please give us a reasonable time to fix it before you disclose it publicly.

If private reporting is not available, open a public issue that says only that you have a security report and ask for a
private channel; do not put details in it.

## Release packages

Releases list a SHA-256 checksum for every file in `SHA256SUMS.txt`; check downloads with `sha256sum -c SHA256SUMS.txt`.
The macOS app is ad hoc signed but not notarized, and the Windows program is unsigned (see
[docs/release-checklist.md](docs/release-checklist.md)); your system will warn you the first time you open them.
