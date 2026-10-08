# Fonts

Text styles name a font the way Chief does (`Avenir`, `Avenir Book`, `Arial`,
`Chief Blueprint`) plus Bold and Italic. Plan Studio draws and prints that
text in the installed font of that name. Code: `crates/plan-app/src/fonts.rs`
(discovery, matching, egui, the PDF font source), `crates/plan-docs/src/pdf/
truetype.rs` (our font reader and subsetter), `crates/plan-docs/src/pdf/mod.rs`
(embedding).

## What is found

`FontCatalog::scan` walks the font folders and reads every `.ttf`, `.otf`,
`.ttc` and `.otc` with our own `name`/`OS/2`/`head` parser, without loading
the whole file (some system fonts are 20 MB):

| System  | Folders |
|---------|---------|
| macOS   | `/System/Library/Fonts` (and `Supplemental`), `/Library/Fonts`, `~/Library/Fonts` |
| Linux   | `/usr/share/fonts`, `/usr/local/share/fonts`, `~/.fonts`, `~/.local/share/fonts` |
| Windows | `C:\Windows\Fonts`, `%LOCALAPPDATA%\Microsoft\Windows\Fonts` |

Folders are walked four levels deep; the first file to offer a family and style
wins. The scan runs on a background thread at start-up.

## How a style maps to a face

1. The family is tried in order: the name itself, then Chief's stand-ins.
   `Arial` becomes Helvetica Neue (then Helvetica, Liberation Sans) on a
   machine without it; `Avenir` becomes Avenir Next, Helvetica Neue, Arial;
   Times New Roman, Courier New, Calibri and the others have similar chains.
   `Chief Blueprint` is a drafting font nobody has: it is always the bundled
   font.
2. Within the family: a named style (`Avenir Book`, `Heavy`) wins. Otherwise
   the face closest to weight 400 (regular) or 700 (Bold), with the slant
   asked for (Italic is the Oblique face). Chief's pairing holds: Avenir with
   Bold is Avenir Heavy. Bold on a `Book` style means Bold.
3. A family with no installed candidate gives the bundled font on screen and
   Helvetica on paper, with a one-time note (Preferences > Fonts lists them).

On this Mac: Arial and Arial Bold (`/System/Library/Fonts/Supplemental`),
Avenir Book and Avenir Heavy (`/System/Library/Fonts/Avenir.ttc`, faces 0
and 4).

## On screen

`fonts::font_id` gives egui a `FontId` in the face's own family. The bytes
are read lazily the first time a style asks for the face and added to egui's
font definitions (egui applies a font set in one frame from the next, so the
very first frame shows the bundled font). Plan text objects, dimension
numbers, room labels and the layout window's box text use it.

## On paper

`PdfDoc::use_font` embeds a subset of the face as a TrueType font
(`/FontFile2`, WinAnsi encoding, `/Widths`): the glyphs the document uses
(composite glyphs pulled in whole), a new `cmap`, `head`, `hhea`, `maxp`,
`hmtx`, `loca`, `glyf`, `post`, and `cvt `/`fpgm`/`prep` when present. One
subset per font per document (about 8 to 15 KB per face). `fonts::
install_pdf_source` hands every new PDF the installed fonts.

Limits:

* A font with PostScript (`CFF`) outlines (most `.otf` files) is not
  embedded: the text prints in Helvetica and a note says so.
* **Licensing.** Fonts licensed on a machine are embedded only into the PDFs
  the user makes there, and only when the font's own `OS/2` `fsType` allows
  embedding. A font marked "restricted licence" (bit 1) or bitmap-only (bit 9)
  is never embedded: Helvetica is printed and a note says so. Nothing is
  copied into plans, templates or the repository; a PDF carries only the
  glyphs it shows.
* WinAnsi only: characters outside Latin-1 and the usual punctuation print
  as `?` (as with Helvetica).
* Layout page CAD text, leaders and title blocks are not text-style text and
  stay Helvetica.

## Editing

* Default Settings > Text Styles: the Font row is a picker of the installed
  families (searchable; Chief names that are not installed are marked) with
  a preview line set in the face the style maps to. Bold and Italic map to
  real faces.
* Default Settings > Text Styles > Replace Fonts (Chief TXT-12): replaces one
  family by another in every style of the list being edited. It applies
  with the dialog's OK, as one undo step ("Text Styles").
* Preferences > Fonts: the interface text size, and "Use system fonts for plan,
  layout and PDF text" (saved in `~/.plan-studio/fonts.json`; off means the
  bundled font on screen and Helvetica on paper).
