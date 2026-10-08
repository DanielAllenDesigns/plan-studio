//! A small TrueType / OpenType reader and subsetter for the PDF writer.
//!
//! [`Font::parse`] reads the tables the writer and the font picker need (names,
//! weight, `cmap`, advances, vertical metrics) from a `.ttf`, `.otf` or one
//! face of a `.ttc`; every read is bounds-checked, so a damaged file gives
//! `None` rather than a panic. [`Font::subset`] cuts a glyph-outline (`glyf`)
//! font down to the glyphs a document uses and rewrites it as a new, valid
//! TrueType font (`cmap`, `head`, `hhea`, `maxp`, `hmtx`, `loca`, `glyf`,
//! `post`, plus `cvt `, `fpgm` and `prep` when the font has them), which
//! [`super::PdfDoc`] embeds as `/FontFile2`. Fonts with PostScript (`CFF`)
//! outlines are read (names and metrics) but not subset; the PDF writer
//! falls back to Helvetica for them.
//!
//! Fonts licensed on the machine are embedded only when their `OS/2`
//! `fsType` allows it and only into the PDFs the user makes; a font marked
//! "restricted licence" is never embedded.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

// ------------------------------------------------------------ requests --

/// A font the document asks for: a family, an optional face style name
/// (`Heavy`), and the bold and italic flags.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct FontSpec {
    pub family: String,
    /// The face style by name (`Book`, `Heavy`); empty: bold and italic choose.
    pub style: String,
    pub bold: bool,
    pub italic: bool,
}

impl FontSpec {
    pub fn new(family: impl Into<String>, bold: bool, italic: bool) -> Self {
        Self {
            family: family.into(),
            style: String::new(),
            bold,
            italic,
        }
    }

    /// The font a text style asks for (`None` when it names no font).
    pub fn of_style(style: &plan_core::TextStyle) -> Option<Self> {
        let family = style.font_family();
        (!family.trim().is_empty()).then(|| Self {
            family: family.to_string(),
            style: style.font_face_style().to_string(),
            bold: style.bold,
            italic: style.italic,
        })
    }
}

/// The bytes of one font face: a whole file and the face index inside it
/// (non-zero only for `.ttc` collections).
#[derive(Debug, Clone)]
pub struct FontFace {
    pub data: Arc<Vec<u8>>,
    pub index: u32,
}

/// Where the PDF writer finds the font files a [`FontSpec`] names. The
/// application implements it over the installed fonts.
pub trait FontSource: Send + Sync + fmt::Debug {
    /// The face that best matches `spec`, or `None` when the family is not
    /// installed (the writer then uses Helvetica).
    fn face(&self, spec: &FontSpec) -> Option<FontFace>;
}

// -------------------------------------------------------------- reading --

/// What kind of outlines a font has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outlines {
    /// Quadratic `glyf` outlines (subsettable here).
    TrueType,
    /// PostScript `CFF` outlines (read, not subset).
    Cff,
}

/// What the name tables and `OS/2` say about one face.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceInfo {
    /// Typographic family (name 16, else name 1): `Avenir`, `Arial`.
    pub family: String,
    /// Style within the family (name 17, else name 2): `Book`, `Bold Italic`.
    pub style: String,
    pub full_name: String,
    pub postscript_name: String,
    /// `usWeightClass`, 100 to 900.
    pub weight: u16,
    pub italic: bool,
    pub outlines: Outlines,
    /// `OS/2` `fsType` allows the font to be embedded in a document.
    pub embeddable: bool,
}

type Tag = [u8; 4];

fn be16(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes(
        d.get(o..o.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn be32(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        d.get(o..o.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn bei16(d: &[u8], o: usize) -> Option<i16> {
    be16(d, o).map(|v| v as i16)
}

/// Faces in a font file: 1 for a plain font, n for a `.ttc`, 0 when the
/// bytes are not a font.
pub fn face_count(data: &[u8]) -> u32 {
    match data.get(0..4) {
        Some(b"ttcf") => be32(data, 8).unwrap_or(0).min(256),
        Some(t) if t == [0, 1, 0, 0] || t == *b"OTTO" || t == *b"true" => 1,
        _ => 0,
    }
}

/// A parsed face. Cheap to clone (the bytes are shared).
#[derive(Clone)]
pub struct Font {
    data: Arc<Vec<u8>>,
    /// `(tag, offset, length)` of each table, offsets into `data`.
    tables: Vec<(Tag, usize, usize)>,
    units_per_em: u16,
    num_glyphs: u16,
    num_hmetrics: u16,
    loca_long: bool,
    outlines: Outlines,
    /// `(offset, format)` of the chosen Unicode `cmap` subtable.
    cmap: Option<(usize, u16)>,
}

impl fmt::Debug for Font {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Font")
            .field("bytes", &self.data.len())
            .field("glyphs", &self.num_glyphs)
            .field("units_per_em", &self.units_per_em)
            .field("outlines", &self.outlines)
            .finish()
    }
}

impl Font {
    /// Parses face `index` of `data` (0 for a plain font).
    pub fn parse(data: Arc<Vec<u8>>, index: u32) -> Option<Font> {
        let d: &[u8] = &data;
        let base = match d.get(0..4)? {
            b"ttcf" => {
                if index >= be32(d, 8)? {
                    return None;
                }
                be32(d, 12 + 4 * index as usize)? as usize
            }
            t if index == 0 && (*t == [0, 1, 0, 0] || *t == *b"OTTO" || *t == *b"true") => 0,
            _ => return None,
        };
        let n_tables = be16(d, base + 4)? as usize;
        let mut tables = Vec::with_capacity(n_tables);
        for i in 0..n_tables {
            let rec = base + 12 + 16 * i;
            let tag: Tag = d.get(rec..rec + 4)?.try_into().ok()?;
            let off = be32(d, rec + 8)? as usize;
            let len = be32(d, rec + 12)? as usize;
            if off.checked_add(len)? <= d.len() {
                tables.push((tag, off, len));
            }
        }
        let mut font = Font {
            data,
            tables,
            units_per_em: 0,
            num_glyphs: 0,
            num_hmetrics: 0,
            loca_long: false,
            outlines: Outlines::TrueType,
            cmap: None,
        };
        let head = font.table(b"head")?;
        let (units_per_em, loca_format) = (be16(head, 18)?, bei16(head, 50)?);
        let num_glyphs = be16(font.table(b"maxp")?, 4)?;
        let num_hmetrics = be16(font.table(b"hhea")?, 34)?;
        if units_per_em < 16 {
            return None;
        }
        font.units_per_em = units_per_em;
        font.loca_long = loca_format != 0;
        font.num_glyphs = num_glyphs;
        font.num_hmetrics = num_hmetrics;
        font.outlines = if font.table(b"glyf").is_some() && font.table(b"loca").is_some() {
            Outlines::TrueType
        } else if font.table(b"CFF ").is_some() || font.table(b"CFF2").is_some() {
            Outlines::Cff
        } else {
            return None;
        };
        font.table(b"hmtx")?;
        font.cmap = font.choose_cmap();
        font.cmap?;
        Some(font)
    }

    fn table(&self, tag: &[u8; 4]) -> Option<&[u8]> {
        let (_, off, len) = self.tables.iter().find(|(t, _, _)| t == tag)?;
        self.data.get(*off..off + len)
    }

    pub fn units_per_em(&self) -> u16 {
        self.units_per_em
    }

    pub fn num_glyphs(&self) -> u16 {
        self.num_glyphs
    }

    pub fn outlines(&self) -> Outlines {
        self.outlines
    }

    fn choose_cmap(&self) -> Option<(usize, u16)> {
        let t = self.table(b"cmap")?;
        let base = self.tables.iter().find(|(tag, _, _)| tag == b"cmap")?.1;
        let n = be16(t, 2)? as usize;
        let mut best: Option<(u8, usize, u16)> = None;
        for i in 0..n {
            let (plat, enc) = (be16(t, 4 + 8 * i)?, be16(t, 6 + 8 * i)?);
            let off = be32(t, 8 + 8 * i)? as usize;
            let fmt = be16(t, off)?;
            if fmt != 4 && fmt != 12 {
                continue;
            }
            let rank = match (plat, enc, fmt) {
                (3, 10, 12) => 5,
                (0, _, 12) => 4,
                (3, 1, 4) => 3,
                (0, _, 4) => 2,
                (3, 0, _) => 0, // symbol: not Unicode
                _ => 1,
            };
            if rank > 0 && best.is_none_or(|(r, _, _)| rank > r) {
                best = Some((rank, base + off, fmt));
            }
        }
        best.map(|(_, off, fmt)| (off, fmt))
    }

    /// The glyph for `c`, or `None` when the font has none.
    pub fn glyph_index(&self, c: char) -> Option<u16> {
        let (off, fmt) = self.cmap?;
        let d: &[u8] = &self.data;
        let cp = c as u32;
        let gid = match fmt {
            4 => {
                if cp > 0xFFFF {
                    return None;
                }
                let cp = cp as u16;
                let seg_x2 = be16(d, off + 6)? as usize;
                let segs = seg_x2 / 2;
                let end = off + 14;
                let start = end + seg_x2 + 2;
                let delta = start + seg_x2;
                let range = delta + seg_x2;
                // First segment whose end code is >= cp.
                let (mut lo, mut hi) = (0usize, segs);
                while lo < hi {
                    let mid = (lo + hi) / 2;
                    if be16(d, end + 2 * mid)? < cp {
                        lo = mid + 1;
                    } else {
                        hi = mid;
                    }
                }
                if lo >= segs || be16(d, start + 2 * lo)? > cp {
                    return None;
                }
                let (id_delta, id_range) = (be16(d, delta + 2 * lo)?, be16(d, range + 2 * lo)?);
                if id_range == 0 {
                    cp.wrapping_add(id_delta)
                } else {
                    let at = range
                        + 2 * lo
                        + id_range as usize
                        + 2 * (cp - be16(d, start + 2 * lo)?) as usize;
                    let g = be16(d, at)?;
                    if g == 0 {
                        0
                    } else {
                        g.wrapping_add(id_delta)
                    }
                }
            }
            12 => {
                let n = be32(d, off + 12)? as usize;
                let (mut lo, mut hi) = (0usize, n);
                while lo < hi {
                    let mid = (lo + hi) / 2;
                    if be32(d, off + 16 + 12 * mid + 4)? < cp {
                        lo = mid + 1;
                    } else {
                        hi = mid;
                    }
                }
                if lo >= n {
                    return None;
                }
                let g = off + 16 + 12 * lo;
                let (s, first) = (be32(d, g)?, be32(d, g + 8)?);
                if cp < s {
                    return None;
                }
                u16::try_from(first.checked_add(cp - s)?).ok()?
            }
            _ => return None,
        };
        (gid != 0 && gid < self.num_glyphs).then_some(gid)
    }

    /// Advance width of glyph `gid`, font units.
    pub fn advance(&self, gid: u16) -> u16 {
        let Some(h) = self.table(b"hmtx") else {
            return 0;
        };
        if self.num_hmetrics == 0 {
            return 0;
        }
        let i = gid.min(self.num_hmetrics - 1) as usize;
        be16(h, 4 * i).unwrap_or(0)
    }

    /// Advance of `c` in 1/1000 em; `None` when the font lacks the character.
    pub fn char_width_1000(&self, c: char) -> Option<f64> {
        let g = self.glyph_index(c)?;
        Some(f64::from(self.advance(g)) * 1000.0 / f64::from(self.units_per_em))
    }

    /// Width of `text` at `size_pt`, points (a missing character counts as
    /// half an em).
    pub fn text_width(&self, text: &str, size_pt: f64) -> f64 {
        let units: f64 = text
            .chars()
            .map(|c| self.char_width_1000(c).unwrap_or(500.0))
            .sum();
        units * size_pt / 1000.0
    }

    /// `(ascent, descent)` in 1/1000 em (`hhea`; descent is negative).
    pub fn ascent_descent_1000(&self) -> (f64, f64) {
        let k = 1000.0 / f64::from(self.units_per_em);
        let h = self.table(b"hhea");
        let a = h.and_then(|h| bei16(h, 4)).unwrap_or(0);
        let dsc = h.and_then(|h| bei16(h, 6)).unwrap_or(0);
        (f64::from(a) * k, f64::from(dsc) * k)
    }

    /// Font bounding box in 1/1000 em.
    pub fn bbox_1000(&self) -> [f64; 4] {
        let k = 1000.0 / f64::from(self.units_per_em);
        let h = self.table(b"head");
        let g = |o| f64::from(h.and_then(|h| bei16(h, o)).unwrap_or(0)) * k;
        [g(36), g(38), g(40), g(42)]
    }

    /// Cap height in 1/1000 em (`OS/2` v2+, else the height of `H`, else the ascent).
    pub fn cap_height_1000(&self) -> f64 {
        let k = 1000.0 / f64::from(self.units_per_em);
        if let Some(os2) = self.table(b"OS/2") {
            if be16(os2, 0).unwrap_or(0) >= 2 {
                if let Some(v) = bei16(os2, 88).filter(|v| *v > 0) {
                    return f64::from(v) * k;
                }
            }
        }
        if let Some((_, y1)) = self.glyph_y_range('H') {
            return f64::from(y1) * k;
        }
        self.ascent_descent_1000().0
    }

    /// `post` italic angle, degrees.
    pub fn italic_angle(&self) -> f64 {
        self.table(b"post")
            .and_then(|p| be32(p, 4))
            .map_or(0.0, |v| f64::from(v as i32) / 65536.0)
    }

    fn glyph_y_range(&self, c: char) -> Option<(i16, i16)> {
        let g = self.glyph_data(self.glyph_index(c)?)?;
        Some((bei16(g, 4)?, bei16(g, 8)?))
    }

    /// Names, weight and flags of this face.
    pub fn info(&self) -> FaceInfo {
        info_from_tables(
            self.table(b"name"),
            self.table(b"OS/2"),
            self.table(b"head"),
            self.outlines,
        )
    }

    fn loca(&self, gid: u16) -> Option<(usize, usize)> {
        let loca = self.table(b"loca")?;
        let g = gid as usize;
        let (a, b) = if self.loca_long {
            (be32(loca, 4 * g)? as usize, be32(loca, 4 * g + 4)? as usize)
        } else {
            (
                be16(loca, 2 * g)? as usize * 2,
                be16(loca, 2 * g + 2)? as usize * 2,
            )
        };
        (b >= a).then_some((a, b))
    }

    /// The outline bytes of glyph `gid` (empty for a blank glyph).
    fn glyph_data(&self, gid: u16) -> Option<&[u8]> {
        if gid >= self.num_glyphs {
            return None;
        }
        let glyf = self.table(b"glyf")?;
        let (a, b) = self.loca(gid)?;
        glyf.get(a..b)
    }

    // ---------------------------------------------------------- subset --

    /// A TrueType font holding only the glyphs for `chars` (and `.notdef`),
    /// with a Unicode `cmap` for exactly those characters. `None` for a font
    /// without `glyf` outlines or one that cannot be read. Characters the
    /// font lacks are left out of the result's map.
    pub fn subset(&self, chars: &[char]) -> Option<Subset> {
        if self.outlines != Outlines::TrueType {
            return None;
        }
        let mut wanted: BTreeMap<char, u16> = BTreeMap::new();
        for &c in chars {
            if let Some(g) = self.glyph_index(c) {
                wanted.insert(c, g);
            }
        }
        // Close over composite glyphs' components.
        let mut keep: Vec<u16> = vec![0];
        keep.extend(wanted.values().copied());
        let mut i = 0;
        while i < keep.len() {
            let g = keep[i];
            i += 1;
            for comp in self.components(self.glyph_data(g)?)? {
                keep.push(comp);
            }
        }
        keep.sort_unstable();
        keep.dedup();
        let new_gid: BTreeMap<u16, u16> = keep
            .iter()
            .enumerate()
            .map(|(n, &old)| (old, n as u16))
            .collect();

        // glyf + loca (long offsets).
        let mut glyf: Vec<u8> = Vec::new();
        let mut loca: Vec<u8> = Vec::new();
        for &old in &keep {
            loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
            let mut g = self.glyph_data(old)?.to_vec();
            if !g.is_empty() {
                self.remap_components(&mut g, &new_gid)?;
                glyf.extend_from_slice(&g);
                while !glyf.len().is_multiple_of(4) {
                    glyf.push(0);
                }
            }
        }
        loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());

        // hmtx: every glyph gets a full (advance, lsb) record.
        let hmtx = self.table(b"hmtx")?;
        let mut new_hmtx: Vec<u8> = Vec::new();
        for &old in &keep {
            let adv = self.advance(old);
            let lsb = if (old as usize) < self.num_hmetrics as usize {
                bei16(hmtx, 4 * old as usize + 2)?
            } else {
                bei16(
                    hmtx,
                    4 * self.num_hmetrics as usize
                        + 2 * (old as usize - self.num_hmetrics as usize),
                )
                .unwrap_or(0)
            };
            new_hmtx.extend_from_slice(&adv.to_be_bytes());
            new_hmtx.extend_from_slice(&lsb.to_be_bytes());
        }

        let n_new = keep.len() as u16;
        let mut head = self.table(b"head")?.to_vec();
        if head.len() < 54 {
            return None;
        }
        head[8..12].copy_from_slice(&[0; 4]); // checkSumAdjustment, set below
        head[50..52].copy_from_slice(&1i16.to_be_bytes()); // long loca
        let mut hhea = self.table(b"hhea")?.to_vec();
        if hhea.len() < 36 {
            return None;
        }
        hhea[34..36].copy_from_slice(&n_new.to_be_bytes());
        let mut maxp = self.table(b"maxp")?.to_vec();
        if maxp.len() < 6 {
            return None;
        }
        maxp[4..6].copy_from_slice(&n_new.to_be_bytes());

        let char_gid: BTreeMap<char, u16> = wanted
            .iter()
            .filter(|(c, _)| (**c as u32) <= 0xFFFF)
            .map(|(c, old)| (*c, new_gid[old]))
            .collect();

        let mut post = vec![0u8; 32];
        post[0..4].copy_from_slice(&0x0003_0000u32.to_be_bytes());
        if let Some(p) = self.table(b"post").filter(|p| p.len() >= 16) {
            post[4..16].copy_from_slice(&p[4..16]); // italic angle, underline, fixed pitch
        }

        let mut out: Vec<(Tag, Vec<u8>)> = vec![
            (*b"cmap", build_cmap(&char_gid)),
            (*b"glyf", glyf),
            (*b"head", head),
            (*b"hhea", hhea),
            (*b"hmtx", new_hmtx),
            (*b"loca", loca),
            (*b"maxp", maxp),
            (*b"post", post),
        ];
        for tag in [b"cvt ", b"fpgm", b"prep"] {
            if let Some(t) = self.table(tag) {
                out.push((*tag, t.to_vec()));
            }
        }
        out.sort_by_key(|(t, _)| *t);
        Some(Subset {
            data: write_sfnt(out),
            glyph_of: char_gid,
            glyphs: n_new,
        })
    }

    /// Component glyph ids of a composite glyph (empty for a simple one).
    fn components(&self, g: &[u8]) -> Option<Vec<u16>> {
        let mut out = Vec::new();
        if g.is_empty() || bei16(g, 0)? >= 0 {
            return Some(out);
        }
        let mut p = 10;
        loop {
            let flags = be16(g, p)?;
            out.push(be16(g, p + 2)?);
            p += 4 + comp_args_len(flags);
            if flags & 0x20 == 0 {
                return Some(out);
            }
        }
    }

    fn remap_components(&self, g: &mut [u8], map: &BTreeMap<u16, u16>) -> Option<()> {
        if g.len() < 10 || bei16(g, 0)? >= 0 {
            return Some(());
        }
        let mut p = 10;
        loop {
            let flags = be16(g, p)?;
            let old = be16(g, p + 2)?;
            g.get_mut(p + 2..p + 4)?
                .copy_from_slice(&map.get(&old)?.to_be_bytes());
            p += 4 + comp_args_len(flags);
            if flags & 0x20 == 0 {
                return Some(());
            }
        }
    }
}

/// The face info from the small tables that hold it (any may be missing).
fn info_from_tables(
    name_table: Option<&[u8]>,
    os2: Option<&[u8]>,
    head: Option<&[u8]>,
    outlines: Outlines,
) -> FaceInfo {
    let name = |ids: &[u16]| -> String {
        name_table
            .and_then(|t| ids.iter().find_map(|id| name_string_in(t, *id)))
            .unwrap_or_default()
    };
    let mac_style = head.and_then(|h| be16(h, 44)).unwrap_or(0);
    let weight = os2
        .and_then(|o| be16(o, 4))
        .filter(|w| (1..=1000).contains(w))
        .unwrap_or(if mac_style & 1 != 0 { 700 } else { 400 });
    let italic =
        os2.and_then(|o| be16(o, 62)).is_some_and(|s| s & 1 != 0) || mac_style & 2 != 0 || {
            // Some faces carry the slant in the style name only.
            let st = name(&[17, 2]).to_lowercase();
            st.contains("italic") || st.contains("oblique")
        };
    let fs_type = os2.and_then(|o| be16(o, 8)).unwrap_or(0);
    let mut family = name(&[16, 1]);
    let mut style = name(&[17, 2]);
    let full_name = name(&[4]);
    if family.is_empty() {
        family = full_name.clone();
    }
    if style.is_empty() {
        style = "Regular".into();
    }
    FaceInfo {
        family,
        style,
        full_name,
        postscript_name: name(&[6]),
        weight,
        italic,
        outlines,
        // Bit 1: restricted licence, no embedding. Bit 9: bitmaps only.
        embeddable: fs_type & 0x0002 == 0 && fs_type & 0x0200 == 0,
    }
}

/// A string of the `name` table `t`: Windows Unicode, else Unicode platform,
/// else Macintosh Roman (read as Latin-1), English preferred.
fn name_string_in(t: &[u8], id: u16) -> Option<String> {
    let n = be16(t, 2)? as usize;
    let store = be16(t, 4)? as usize;
    let mut best: Option<(u8, String)> = None;
    for i in 0..n {
        let r = 6 + 12 * i;
        if be16(t, r + 6)? != id {
            continue;
        }
        let (plat, enc, lang) = (be16(t, r)?, be16(t, r + 2)?, be16(t, r + 4)?);
        let (len, off) = (be16(t, r + 8)? as usize, be16(t, r + 10)? as usize);
        let Some(raw) = t.get(store + off..store + off + len) else {
            continue;
        };
        let (rank, text) = match (plat, enc) {
            (3, 1) | (3, 10) | (0, _) => {
                let units: Vec<u16> = raw
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u16::from_be_bytes(*c))
                    .collect();
                let rank = if plat == 3 && lang == 0x409 { 4 } else { 3 };
                (rank, String::from_utf16_lossy(&units))
            }
            (1, 0) => (
                if lang == 0 { 2 } else { 1 },
                raw.iter().map(|&b| b as char).collect(),
            ),
            _ => continue,
        };
        if !text.is_empty() && best.as_ref().is_none_or(|(r, _)| rank > *r) {
            best = Some((rank, text));
        }
    }
    best.map(|(_, s)| s)
}

/// Names and weights of every face in a font file, read without loading the
/// whole file (system fonts run to many megabytes): the table directory and
/// the small `name`, `OS/2` and `head` tables only. Returns `(face index,
/// info)`; empty for a file that is not a font.
pub fn scan_faces<R: std::io::Read + std::io::Seek>(r: &mut R) -> Vec<(u32, FaceInfo)> {
    use std::io::SeekFrom;
    fn read_at<R: std::io::Read + std::io::Seek>(
        r: &mut R,
        at: u64,
        len: usize,
    ) -> Option<Vec<u8>> {
        r.seek(SeekFrom::Start(at)).ok()?;
        let mut buf = vec![0u8; len];
        r.read_exact(&mut buf).ok()?;
        Some(buf)
    }
    let mut out = Vec::new();
    let Some(head) = read_at(r, 0, 12) else {
        return out;
    };
    let bases: Vec<u64> = match &head[0..4] {
        b"ttcf" => {
            let n = be32(&head, 8).unwrap_or(0).min(256) as usize;
            let Some(dir) = read_at(r, 12, 4 * n) else {
                return out;
            };
            (0..n)
                .filter_map(|i| be32(&dir, 4 * i))
                .map(u64::from)
                .collect()
        }
        t if t == [0, 1, 0, 0] || t == *b"OTTO" || t == *b"true" => vec![0],
        _ => return out,
    };
    for (index, base) in bases.into_iter().enumerate() {
        let Some(h) = read_at(r, base, 12) else {
            continue;
        };
        let n = be16(&h, 4).unwrap_or(0).min(128) as usize;
        let Some(dir) = read_at(r, base + 12, 16 * n) else {
            continue;
        };
        let entry = |tag: &[u8; 4]| -> Option<(u64, usize)> {
            (0..n).find_map(|i| {
                (dir.get(16 * i..16 * i + 4)? == tag)
                    .then(|| {
                        Some((
                            u64::from(be32(&dir, 16 * i + 8)?),
                            be32(&dir, 16 * i + 12)? as usize,
                        ))
                    })
                    .flatten()
            })
        };
        let outlines = if entry(b"glyf").is_some() && entry(b"loca").is_some() {
            Outlines::TrueType
        } else if entry(b"CFF ").is_some() || entry(b"CFF2").is_some() {
            Outlines::Cff
        } else {
            continue;
        };
        let mut small = |tag: &[u8; 4], cap: usize| -> Option<Vec<u8>> {
            let (off, len) = entry(tag)?;
            read_at(r, off, len.min(cap))
        };
        let name = small(b"name", 1 << 20);
        let os2 = small(b"OS/2", 96);
        let head = small(b"head", 54);
        out.push((
            index as u32,
            info_from_tables(name.as_deref(), os2.as_deref(), head.as_deref(), outlines),
        ));
    }
    out
}

/// Bytes after a component's flags and glyph index: its arguments and any
/// scale / 2x2 transform.
fn comp_args_len(flags: u16) -> usize {
    let mut n = if flags & 0x0001 != 0 { 4 } else { 2 };
    if flags & 0x0008 != 0 {
        n += 2;
    } else if flags & 0x0040 != 0 {
        n += 4;
    } else if flags & 0x0080 != 0 {
        n += 8;
    }
    n
}

/// A subset font and what became of the characters asked for.
#[derive(Debug, Clone)]
pub struct Subset {
    /// The new font file (`/FontFile2`).
    pub data: Vec<u8>,
    /// Character to glyph id in the subset (BMP characters the font has).
    pub glyph_of: BTreeMap<char, u16>,
    /// Glyphs in the subset, including `.notdef`.
    pub glyphs: u16,
}

/// A format 4 `cmap` (Windows Unicode BMP) for `map`.
fn build_cmap(map: &BTreeMap<char, u16>) -> Vec<u8> {
    // Segments of consecutive codes whose glyph ids step by the same delta.
    struct Seg {
        start: u16,
        end: u16,
        delta: u16,
    }
    let mut segs: Vec<Seg> = Vec::new();
    for (&c, &g) in map {
        let code = c as u32 as u16;
        let delta = g.wrapping_sub(code);
        match segs.last_mut() {
            Some(s) if s.end.checked_add(1) == Some(code) && s.delta == delta => s.end = code,
            _ => segs.push(Seg {
                start: code,
                end: code,
                delta,
            }),
        }
    }
    segs.push(Seg {
        start: 0xFFFF,
        end: 0xFFFF,
        delta: 1,
    });
    let n = segs.len();
    let len = 16 + 8 * n;
    let mut sub: Vec<u8> = Vec::with_capacity(len);
    let entry_selector = (usize::BITS - 1 - n.leading_zeros()) as u16;
    let search_range = 2 * (1u16 << entry_selector);
    sub.extend_from_slice(&4u16.to_be_bytes());
    sub.extend_from_slice(&(len as u16).to_be_bytes());
    sub.extend_from_slice(&0u16.to_be_bytes());
    sub.extend_from_slice(&((2 * n) as u16).to_be_bytes());
    sub.extend_from_slice(&search_range.to_be_bytes());
    sub.extend_from_slice(&entry_selector.to_be_bytes());
    sub.extend_from_slice(&(2 * n as u16 - search_range).to_be_bytes());
    for s in &segs {
        sub.extend_from_slice(&s.end.to_be_bytes());
    }
    sub.extend_from_slice(&0u16.to_be_bytes());
    for s in &segs {
        sub.extend_from_slice(&s.start.to_be_bytes());
    }
    for s in &segs {
        sub.extend_from_slice(&s.delta.to_be_bytes());
    }
    for _ in &segs {
        sub.extend_from_slice(&0u16.to_be_bytes());
    }
    let mut t: Vec<u8> = Vec::new();
    t.extend_from_slice(&0u16.to_be_bytes()); // version
    t.extend_from_slice(&1u16.to_be_bytes()); // one encoding record
    t.extend_from_slice(&3u16.to_be_bytes());
    t.extend_from_slice(&1u16.to_be_bytes());
    t.extend_from_slice(&12u32.to_be_bytes());
    t.extend_from_slice(&sub);
    t
}

fn checksum(d: &[u8]) -> u32 {
    d.chunks(4).fold(0u32, |sum, c| {
        let mut w = [0u8; 4];
        w[..c.len()].copy_from_slice(c);
        sum.wrapping_add(u32::from_be_bytes(w))
    })
}

/// Assembles an sfnt file from tables sorted by tag, fixing `head`'s
/// `checkSumAdjustment`.
fn write_sfnt(tables: Vec<(Tag, Vec<u8>)>) -> Vec<u8> {
    let n = tables.len();
    let entry_selector = (usize::BITS - 1 - n.leading_zeros()) as u16;
    let search_range = 16 * (1u16 << entry_selector);
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    out.extend_from_slice(&(n as u16).to_be_bytes());
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&entry_selector.to_be_bytes());
    out.extend_from_slice(&(16 * n as u16 - search_range).to_be_bytes());
    let mut offset = 12 + 16 * n;
    let mut head_at = None;
    for (tag, data) in &tables {
        out.extend_from_slice(tag);
        out.extend_from_slice(&checksum(data).to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        if tag == b"head" {
            head_at = Some(offset);
        }
        offset += data.len().div_ceil(4) * 4;
    }
    for (_, data) in &tables {
        out.extend_from_slice(data);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
    }
    if let Some(h) = head_at {
        let adj = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
        out[h + 8..h + 12].copy_from_slice(&adj.to_be_bytes());
    }
    out
}

// ---------------------------------------------------- sample font --

fn u16s(v: &[u16]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_be_bytes()).collect()
}

fn name_table(entries: &[(u16, &str)]) -> Vec<u8> {
    let mut strings: Vec<u8> = Vec::new();
    let mut recs: Vec<u8> = Vec::new();
    for (id, s) in entries {
        let utf16: Vec<u8> = s.encode_utf16().flat_map(u16::to_be_bytes).collect();
        recs.extend(u16s(&[
            3,
            1,
            0x409,
            *id,
            utf16.len() as u16,
            strings.len() as u16,
        ]));
        strings.extend(utf16);
    }
    let mut t = u16s(&[0, entries.len() as u16, 6 + 12 * entries.len() as u16]);
    t.extend(recs);
    t.extend(strings);
    t
}

/// A 4-glyph TrueType font: `.notdef`, `A` (a 100 unit square), `B` (a
/// composite of `A` moved 50 right) and `C` (a square, unused by tests
/// that subset to A and B). 1000 units per em, advances 500/600/700/800. Sample
/// data for tests across the workspace.
#[doc(hidden)]
pub fn synthetic_font(family: &str, style: &str, weight: u16, fs_type: u16) -> Vec<u8> {
    let square = |size: i16| -> Vec<u8> {
        let mut g = Vec::new();
        g.extend(1i16.to_be_bytes());
        for v in [0, 0, size, size] {
            g.extend(v.to_be_bytes());
        }
        g.extend(3u16.to_be_bytes()); // endPtsOfContours
        g.extend(0u16.to_be_bytes()); // instructionLength
        g.extend([1u8; 4]); // flags: on curve, 16-bit deltas
        for dx in [0, size, 0, -size] {
            g.extend(dx.to_be_bytes());
        }
        for dy in [0, 0, size, 0] {
            g.extend(dy.to_be_bytes());
        }
        while !g.len().is_multiple_of(4) {
            g.push(0);
        }
        g
    };
    let a = square(100);
    let mut b = Vec::new();
    b.extend((-1i16).to_be_bytes());
    for v in [0i16, 0, 150, 100] {
        b.extend(v.to_be_bytes());
    }
    b.extend(0x0003u16.to_be_bytes()); // words, xy values
    b.extend(1u16.to_be_bytes()); // component: glyph 1
    b.extend(50i16.to_be_bytes());
    b.extend(0i16.to_be_bytes());
    let c = square(80);
    let glyphs: [&[u8]; 4] = [&[], &a, &b, &c];
    let mut glyf = Vec::new();
    let mut loca = Vec::new();
    for g in glyphs {
        loca.extend((glyf.len() as u32 / 2).to_be_bytes().iter().skip(2));
        glyf.extend_from_slice(g);
        while !glyf.len().is_multiple_of(4) {
            glyf.push(0);
        }
    }
    loca.extend((glyf.len() as u32 / 2).to_be_bytes().iter().skip(2));
    let mut head = vec![0u8; 54];
    head[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
    head[12..16].copy_from_slice(&0x5F0F_3CF5u32.to_be_bytes());
    head[18..20].copy_from_slice(&1000u16.to_be_bytes());
    head[36..44].copy_from_slice(&u16s(&[0, 0, 150, 100]));
    head[44..46].copy_from_slice(&((weight >= 700) as u16).to_be_bytes());
    head[50..52].copy_from_slice(&0i16.to_be_bytes()); // short loca
    let mut hhea = vec![0u8; 36];
    hhea[4..6].copy_from_slice(&800u16.to_be_bytes());
    hhea[6..8].copy_from_slice(&(-200i16).to_be_bytes());
    hhea[34..36].copy_from_slice(&4u16.to_be_bytes());
    let mut maxp = vec![0u8; 32];
    maxp[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
    maxp[4..6].copy_from_slice(&4u16.to_be_bytes());
    let hmtx = u16s(&[500, 0, 600, 0, 700, 0, 800, 0]);
    let mut cmap = u16s(&[0, 1, 3, 1]);
    cmap.extend(12u32.to_be_bytes());
    // Format 4: A..C map to glyphs 1..3, plus the 0xFFFF terminator.
    let sub = u16s(&[
        4,
        32,
        0,
        4,
        4,
        1,
        0,
        0x43,
        0xFFFF,
        0,
        0x41,
        0xFFFF,
        1u16.wrapping_sub(0x41),
        1,
        0,
        0,
    ]);
    cmap.extend(sub);
    let mut os2 = vec![0u8; 78];
    os2[4..6].copy_from_slice(&weight.to_be_bytes());
    os2[8..10].copy_from_slice(&fs_type.to_be_bytes());
    if style.contains("Italic") || style.contains("Oblique") {
        os2[62..64].copy_from_slice(&1u16.to_be_bytes());
    }
    let post = vec![
        0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    let full = format!("{family} {style}");
    let mut tables = vec![
        (*b"OS/2", os2),
        (*b"cmap", cmap),
        (*b"glyf", glyf),
        (*b"head", head),
        (*b"hhea", hhea),
        (*b"hmtx", hmtx),
        (*b"loca", loca),
        (*b"maxp", maxp),
        (
            *b"name",
            name_table(&[
                (1, family),
                (2, style),
                (4, &full),
                (6, &full.replace(' ', "-")),
            ]),
        ),
        (*b"post", post),
    ];
    tables.sort_by_key(|(t, _)| *t);
    write_sfnt(tables)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn parse(bytes: Vec<u8>) -> Font {
        Font::parse(Arc::new(bytes), 0).expect("parses")
    }

    #[test]
    fn reads_names_weight_and_metrics_of_a_synthetic_font() {
        let f = parse(synthetic_font("Test Sans", "Bold", 700, 0));
        let i = f.info();
        assert_eq!(i.family, "Test Sans");
        assert_eq!(i.style, "Bold");
        assert_eq!(i.full_name, "Test Sans Bold");
        assert_eq!(i.postscript_name, "Test-Sans-Bold");
        assert_eq!((i.weight, i.italic, i.embeddable), (700, false, true));
        assert_eq!(i.outlines, Outlines::TrueType);
        assert_eq!(f.units_per_em(), 1000);
        assert_eq!(f.glyph_index('A'), Some(1));
        assert_eq!(f.glyph_index('C'), Some(3));
        assert_eq!(f.glyph_index('D'), None);
        assert_eq!(f.char_width_1000('B'), Some(700.0));
        assert!((f.text_width("AB", 10.0) - 13.0).abs() < 1e-9);
        assert_eq!(f.ascent_descent_1000(), (800.0, -200.0));
        assert_eq!(face_count(&synthetic_font("T", "R", 400, 0)), 1);
    }

    #[test]
    fn restricted_fonts_are_not_embeddable() {
        let f = parse(synthetic_font("Locked", "Regular", 400, 0x0002));
        assert!(!f.info().embeddable);
        let f = parse(synthetic_font("Editable", "Regular", 400, 0x0008));
        assert!(f.info().embeddable);
    }

    #[test]
    fn damaged_or_foreign_bytes_do_not_parse() {
        assert!(Font::parse(Arc::new(vec![]), 0).is_none());
        assert!(Font::parse(Arc::new(b"not a font at all".to_vec()), 0).is_none());
        let good = synthetic_font("T", "R", 400, 0);
        for cut in [4, 12, 40, good.len() / 2] {
            // A truncated file never panics.
            let _ = Font::parse(Arc::new(good[..cut].to_vec()), 0);
        }
        assert_eq!(face_count(b"junk"), 0);
        assert!(Font::parse(Arc::new(good), 1).is_none());
    }

    #[test]
    fn subset_keeps_used_glyphs_and_is_a_valid_font_that_parses_back() {
        let f = parse(synthetic_font("Test Sans", "Regular", 400, 0));
        let s = f.subset(&['B', 'Z']).expect("subsets");
        // .notdef, A (pulled in by composite B) and B; C is gone; Z is absent.
        assert_eq!(s.glyphs, 3);
        assert_eq!(s.glyph_of.len(), 1);
        let back = parse(s.data.clone());
        assert_eq!(back.num_glyphs(), 3);
        let gb = back.glyph_index('B').unwrap();
        assert_eq!(gb, s.glyph_of[&'B']);
        assert_eq!(back.glyph_index('C'), None);
        assert_eq!(
            back.glyph_index('A'),
            None,
            "only asked-for characters are mapped"
        );
        assert_eq!(back.advance(gb), 700);
        // The composite's component now points at the renumbered A (glyph 1).
        let comps = back.components(back.glyph_data(gb).unwrap()).unwrap();
        assert_eq!(comps, vec![1]);
        // Checksums: the whole file sums to the magic number.
        assert_eq!(checksum(&s.data), 0xB1B0_AFBA);
        assert!(s.data.len() < synthetic_font("Test Sans", "Regular", 400, 0).len() + 64);
    }

    #[test]
    fn subset_cmap_covers_runs_and_gaps() {
        let f = parse(synthetic_font("T", "R", 400, 0));
        let s = f.subset(&['A', 'B', 'C']).unwrap();
        let back = parse(s.data);
        for (c, g) in [('A', 1), ('B', 2), ('C', 3)] {
            assert_eq!(back.glyph_index(c), Some(g));
        }
        assert_eq!(back.glyph_index('D'), None);
        assert_eq!(back.glyph_index('@'), None);
    }

    #[test]
    fn subset_of_nothing_is_just_notdef() {
        let f = parse(synthetic_font("T", "R", 400, 0));
        let s = f.subset(&[]).unwrap();
        assert_eq!(s.glyphs, 1);
        assert!(s.glyph_of.is_empty());
        let back = parse(s.data);
        assert_eq!(back.glyph_index('A'), None);
    }
}
