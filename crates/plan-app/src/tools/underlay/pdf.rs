//! Pictures inside PDF files (scanned surveys and existing-house plans).
//!
//! This build has no PDF renderer, so a PDF can be an underlay only when its
//! pages are pictures: a scanner (or "print to PDF" of a photo) stores each
//! page as one image stream. [`page_images`] reads the file's object table
//! itself, finds the page tree, and returns the picture of every page:
//!
//! * the objects are found by scanning for `N G obj` (so a damaged or
//!   incrementally updated cross-reference table does not matter), and the
//!   objects packed in `/ObjStm` streams of PDF 1.5 files are unpacked;
//! * the catalog's `/Pages` tree gives the pages in order, with inherited
//!   `/Resources`; each page's `/XObject` images (looking into form XObjects
//!   too) are candidates and the largest is the page's picture;
//! * `DCTDecode` (JPEG) streams are kept as they are; `FlateDecode` streams
//!   (the PNG-style pictures of the same family, with or without a PNG or TIFF
//!   predictor) are inflated and written as a PNG; gray, RGB, CMYK, ICCBased,
//!   Indexed and 1/2/4/8/16-bit samples are understood;
//! * anything else (CCITT, JBIG2, JPX, LZW, a page of vector drawing and text)
//!   gives a message that says to export the page as a PNG instead.
//!
//! Files without a readable page tree give their pictures in file order, one
//! per page.

use super::inflate::{inflate_zlib, zlib_stored};
use plan_core::images::image_size;
use std::collections::HashMap;

/// One JPEG picture found in a PDF.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PdfPicture {
    pub width: u32,
    pub height: u32,
    /// The JPEG file bytes, exactly as stored in the PDF.
    pub jpeg: Vec<u8>,
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from >= hay.len() || needle.is_empty() {
        return None;
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

#[cfg(test)]
fn rfind(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).rposition(|w| w == needle)
}

/// The JPEG image streams of a PDF, in file order: the older whole-file scan,
/// kept as a cross-check of the object-table reader in the tests.
#[cfg(test)]
pub fn jpeg_pictures(pdf: &[u8]) -> Vec<PdfPicture> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(p) = find(pdf, b"stream", i) {
        // "endstream" contains "stream": skip it.
        if p >= 3 && &pdf[p - 3..p] == b"end" {
            i = p + 6;
            continue;
        }
        let head = &pdf[p.saturating_sub(4096)..p];
        let dict = rfind(head, b"obj").map_or(head, |k| &head[k + 3..]);
        let mut start = p + 6;
        if pdf.get(start) == Some(&b'\r') {
            start += 1;
        }
        if pdf.get(start) == Some(&b'\n') {
            start += 1;
        }
        let Some(end) = find(pdf, b"endstream", start) else {
            break;
        };
        let is_jpeg_image = find(dict, b"/DCTDecode", 0).is_some()
            && find(dict, b"/Image", 0).is_some()
            && pdf[start..end].starts_with(&[0xFF, 0xD8]);
        if is_jpeg_image {
            let mut data = &pdf[start..end];
            while let Some((&last, rest)) = data.split_last() {
                if last == b'\n' || last == b'\r' {
                    data = rest;
                } else {
                    break;
                }
            }
            if let Some((width, height, _)) = image_size(data) {
                out.push(PdfPicture {
                    width,
                    height,
                    jpeg: data.to_vec(),
                });
            }
        }
        i = end + 9;
    }
    out
}

// ----------------------------------------------------------------------
// Objects
// ----------------------------------------------------------------------

/// The message when a PDF holds no picture this build can read.
pub const NO_PICTURE: &str = "That PDF holds no scanned picture (this build cannot draw vector PDF pages): export the page as a PNG and import that";

type Dict = HashMap<String, Val>;

static NULL: Val = Val::Null;

/// A PDF value.
#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Null,
    Bool(bool),
    Int(i64),
    Real(f64),
    Name(String),
    Str(Vec<u8>),
    Arr(Vec<Val>),
    Dict(Dict),
    Ref(u32),
}

impl Val {
    fn num(&self) -> Option<f64> {
        match self {
            Val::Int(i) => Some(*i as f64),
            Val::Real(r) => Some(*r),
            _ => None,
        }
    }

    fn name(&self) -> Option<&str> {
        match self {
            Val::Name(n) => Some(n),
            _ => None,
        }
    }
}

fn is_ws(b: u8) -> bool {
    matches!(b, 0 | 9 | 10 | 12 | 13 | 32)
}

fn is_delim(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

struct Lex<'a> {
    d: &'a [u8],
    p: usize,
}

impl Lex<'_> {
    fn skip_ws(&mut self) {
        while let Some(&b) = self.d.get(self.p) {
            if is_ws(b) {
                self.p += 1;
            } else if b == b'%' {
                while self.d.get(self.p).is_some_and(|&c| c != 10 && c != 13) {
                    self.p += 1;
                }
            } else {
                break;
            }
        }
    }

    fn token(&mut self) -> &[u8] {
        let s = self.p;
        while self
            .d
            .get(self.p)
            .is_some_and(|&b| !is_ws(b) && !is_delim(b))
        {
            self.p += 1;
        }
        &self.d[s..self.p]
    }

    fn name(&mut self) -> String {
        // The '/' is consumed by the caller.
        let raw = self.token().to_vec();
        let mut out = Vec::with_capacity(raw.len());
        let mut i = 0;
        while i < raw.len() {
            if raw[i] == b'#' && i + 3 <= raw.len() {
                let h = std::str::from_utf8(&raw[i + 1..i + 3])
                    .ok()
                    .and_then(|s| u8::from_str_radix(s, 16).ok());
                if let Some(b) = h {
                    out.push(b);
                    i += 3;
                    continue;
                }
            }
            out.push(raw[i]);
            i += 1;
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    fn literal_string(&mut self) -> Vec<u8> {
        // '(' consumed.
        let mut out = Vec::new();
        let mut depth = 1;
        while let Some(&b) = self.d.get(self.p) {
            self.p += 1;
            match b {
                b'(' => {
                    depth += 1;
                    out.push(b);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    out.push(b);
                }
                b'\\' => {
                    let Some(&e) = self.d.get(self.p) else {
                        break;
                    };
                    self.p += 1;
                    match e {
                        b'n' => out.push(10),
                        b'r' => out.push(13),
                        b't' => out.push(9),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'0'..=b'7' => {
                            let mut v = u32::from(e - b'0');
                            for _ in 0..2 {
                                match self.d.get(self.p) {
                                    Some(&c @ b'0'..=b'7') => {
                                        v = v * 8 + u32::from(c - b'0');
                                        self.p += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push(v as u8);
                        }
                        b'\r' => {
                            if self.d.get(self.p) == Some(&b'\n') {
                                self.p += 1;
                            }
                        }
                        b'\n' => {}
                        other => out.push(other),
                    }
                }
                _ => out.push(b),
            }
        }
        out
    }

    fn hex_string(&mut self) -> Vec<u8> {
        // '<' consumed.
        let mut nibbles = Vec::new();
        while let Some(&b) = self.d.get(self.p) {
            self.p += 1;
            if b == b'>' {
                break;
            }
            if let Some(v) = (b as char).to_digit(16) {
                nibbles.push(v as u8);
            }
        }
        if nibbles.len() % 2 == 1 {
            nibbles.push(0);
        }
        nibbles.chunks(2).map(|c| c[0] << 4 | c[1]).collect()
    }

    fn number(&mut self) -> Option<Val> {
        let tok = self.token();
        let s = std::str::from_utf8(tok).ok()?;
        if let Ok(i) = s.parse::<i64>() {
            return Some(Val::Int(i));
        }
        s.parse::<f64>().ok().map(Val::Real)
    }

    /// The next value; `None` at a keyword (`endobj`, `stream`...) or the end.
    fn value(&mut self) -> Option<Val> {
        self.value_at(0)
    }

    fn value_at(&mut self, depth: usize) -> Option<Val> {
        if depth > 40 {
            return None;
        }
        self.skip_ws();
        let b = *self.d.get(self.p)?;
        match b {
            b'/' => {
                self.p += 1;
                Some(Val::Name(self.name()))
            }
            b'(' => {
                self.p += 1;
                Some(Val::Str(self.literal_string()))
            }
            b'<' if self.d.get(self.p + 1) == Some(&b'<') => {
                self.p += 2;
                let mut dict = Dict::new();
                loop {
                    self.skip_ws();
                    match self.d.get(self.p) {
                        None => break,
                        Some(b'>') => {
                            self.p += if self.d.get(self.p + 1) == Some(&b'>') {
                                2
                            } else {
                                1
                            };
                            break;
                        }
                        Some(b'/') => {
                            self.p += 1;
                            let key = self.name();
                            match self.value_at(depth + 1) {
                                Some(v) => {
                                    dict.insert(key, v);
                                }
                                None => break,
                            }
                        }
                        Some(_) => {
                            // Junk between entries: skip a token.
                            let before = self.p;
                            let _ = self.token();
                            if self.p == before {
                                self.p += 1;
                            }
                        }
                    }
                }
                Some(Val::Dict(dict))
            }
            b'<' => {
                self.p += 1;
                Some(Val::Str(self.hex_string()))
            }
            b'[' => {
                self.p += 1;
                let mut items = Vec::new();
                loop {
                    self.skip_ws();
                    match self.d.get(self.p) {
                        None => break,
                        Some(b']') => {
                            self.p += 1;
                            break;
                        }
                        Some(_) => match self.value_at(depth + 1) {
                            Some(v) => items.push(v),
                            None => break,
                        },
                    }
                }
                Some(Val::Arr(items))
            }
            b'0'..=b'9' | b'+' | b'-' | b'.' => {
                let v = self.number()?;
                if let Val::Int(n) = v {
                    if n >= 0 {
                        // `n g R` is a reference.
                        let save = self.p;
                        self.skip_ws();
                        let g = self.token().to_vec();
                        if !g.is_empty() && g.iter().all(u8::is_ascii_digit) {
                            self.skip_ws();
                            if self.d.get(self.p) == Some(&b'R')
                                && self
                                    .d
                                    .get(self.p + 1)
                                    .is_none_or(|&c| is_ws(c) || is_delim(c))
                            {
                                self.p += 1;
                                return u32::try_from(n).ok().map(Val::Ref);
                            }
                        }
                        self.p = save;
                    }
                }
                Some(v)
            }
            _ => {
                let save = self.p;
                let tok = self.token().to_vec();
                match tok.as_slice() {
                    b"true" => Some(Val::Bool(true)),
                    b"false" => Some(Val::Bool(false)),
                    b"null" => Some(Val::Null),
                    _ => {
                        self.p = save;
                        None
                    }
                }
            }
        }
    }
}

/// An indirect object: its value and, for a stream, where the (still
/// encoded) data lies in the file.
#[derive(Clone, Debug)]
struct Obj {
    val: Val,
    stream: Option<(usize, usize)>,
}

/// The objects of a PDF file.
pub struct Doc<'a> {
    data: &'a [u8],
    objs: HashMap<u32, Obj>,
    /// Object numbers in the order they were found.
    order: Vec<u32>,
}

/// Largest picture or stream the reader inflates, bytes.
const MAX_STREAM: usize = 400_000_000;

impl<'a> Doc<'a> {
    /// Reads the object table of `data`.
    pub fn parse(data: &'a [u8]) -> Doc<'a> {
        let mut doc = Doc {
            data,
            objs: HashMap::new(),
            order: Vec::new(),
        };
        doc.scan();
        doc.unpack_object_streams();
        doc
    }

    fn scan(&mut self) {
        let d = self.data;
        let mut i = 0;
        while let Some(p) = find(d, b"obj", i) {
            i = p + 3;
            // "N G obj": white space before, a delimiter or white space after.
            let after_ok = d.get(p + 3).is_none_or(|&c| is_ws(c) || is_delim(c));
            if p == 0 || !is_ws(d[p - 1]) || !after_ok {
                continue;
            }
            let mut j = p - 1;
            while j > 0 && is_ws(d[j]) {
                j -= 1;
            }
            let ge = j + 1;
            while j > 0 && d[j].is_ascii_digit() {
                j -= 1;
            }
            if !d[j].is_ascii_digit() {
                j += 1;
            }
            let gs = j;
            if gs >= ge || j == 0 {
                continue;
            }
            j -= 1;
            if !is_ws(d[j]) {
                continue;
            }
            while j > 0 && is_ws(d[j]) {
                j -= 1;
            }
            let ne = j + 1;
            while j > 0 && d[j].is_ascii_digit() {
                j -= 1;
            }
            if !d[j].is_ascii_digit() {
                j += 1;
            }
            let ns = j;
            if ns >= ne {
                continue;
            }
            let Some(num) = std::str::from_utf8(&d[ns..ne])
                .ok()
                .and_then(|s| s.parse::<u32>().ok())
            else {
                continue;
            };
            let mut lex = Lex { d, p: p + 3 };
            let Some(val) = lex.value() else {
                continue;
            };
            lex.skip_ws();
            let mut stream = None;
            if matches!(val, Val::Dict(_)) && d[lex.p.min(d.len())..].starts_with(b"stream") {
                let mut start = lex.p + 6;
                if d.get(start) == Some(&b'\r') {
                    start += 1;
                }
                if d.get(start) == Some(&b'\n') {
                    start += 1;
                }
                let end = self.stream_end(&val, start);
                if let Some(end) = end {
                    stream = Some((start, end));
                    i = end;
                }
            } else {
                i = lex.p.max(i);
            }
            if !self.objs.contains_key(&num) {
                self.order.push(num);
            }
            self.objs.insert(num, Obj { val, stream });
        }
    }

    /// Where the stream data that begins at `start` ends: by `/Length` when it
    /// is a plain number that lands on `endstream`, else by searching.
    fn stream_end(&self, dict: &Val, start: usize) -> Option<usize> {
        let d = self.data;
        if let Val::Dict(m) = dict {
            if let Some(Val::Int(len)) = m.get("Length") {
                let end = start.checked_add(usize::try_from(*len).ok()?)?;
                if end <= d.len() {
                    let mut k = end;
                    while d.get(k).is_some_and(|&c| is_ws(c)) && k < end + 4 {
                        k += 1;
                    }
                    if d[k.min(d.len())..].starts_with(b"endstream") {
                        return Some(end);
                    }
                }
            }
        }
        let e = find(d, b"endstream", start)?;
        let mut end = e;
        if end > start && d[end - 1] == b'\n' {
            end -= 1;
        }
        if end > start && d[end - 1] == b'\r' {
            end -= 1;
        }
        Some(end)
    }

    fn unpack_object_streams(&mut self) {
        let streams: Vec<u32> = self
            .objs
            .iter()
            .filter(|(_, o)| {
                o.stream.is_some()
                    && matches!(&o.val, Val::Dict(m) if m.get("Type").and_then(Val::name) == Some("ObjStm"))
            })
            .map(|(n, _)| *n)
            .collect();
        for sn in streams {
            let Some(obj) = self.objs.get(&sn).cloned() else {
                continue;
            };
            let Val::Dict(dict) = &obj.val else {
                continue;
            };
            let n = dict.get("N").and_then(Val::num).unwrap_or(0.0) as usize;
            let first = dict.get("First").and_then(Val::num).unwrap_or(0.0) as usize;
            let Ok((bytes, None)) = self.decode_chain(&obj) else {
                continue;
            };
            let mut head = Lex { d: &bytes, p: 0 };
            let mut entries = Vec::new();
            for _ in 0..n.min(1_000_000) {
                head.skip_ws();
                let (Some(Val::Int(on)), Some(Val::Int(off))) = (head.value(), head.value()) else {
                    break;
                };
                entries.push((on as u32, off as usize));
            }
            for (on, off) in entries {
                if self.objs.contains_key(&on) {
                    continue;
                }
                let mut lex = Lex {
                    d: &bytes,
                    p: first + off,
                };
                if let Some(val) = lex.value() {
                    self.order.push(on);
                    self.objs.insert(on, Obj { val, stream: None });
                }
            }
        }
    }

    /// Follows references to the value they name.
    fn resolve<'b>(&'b self, v: &'b Val) -> &'b Val {
        let mut v = v;
        for _ in 0..16 {
            match v {
                Val::Ref(n) => match self.objs.get(n) {
                    Some(o) => v = &o.val,
                    None => return &NULL,
                },
                _ => return v,
            }
        }
        &NULL
    }

    fn dict<'b>(&'b self, v: &'b Val) -> Option<&'b Dict> {
        match self.resolve(v) {
            Val::Dict(d) => Some(d),
            _ => None,
        }
    }

    fn int_of(&self, v: Option<&Val>) -> Option<i64> {
        match self.resolve(v?) {
            Val::Int(i) => Some(*i),
            Val::Real(r) => Some(*r as i64),
            _ => None,
        }
    }

    /// The filters of a stream dictionary with their decode parameters.
    fn filters(&self, dict: &Dict) -> Vec<(String, Dict)> {
        let names: Vec<String> = match dict
            .get("Filter")
            .or_else(|| dict.get("F"))
            .map(|v| self.resolve(v))
        {
            Some(Val::Name(n)) => vec![n.clone()],
            Some(Val::Arr(a)) => a
                .iter()
                .filter_map(|v| self.resolve(v).name().map(str::to_string))
                .collect(),
            _ => Vec::new(),
        };
        let parms: Vec<Dict> = match dict
            .get("DecodeParms")
            .or_else(|| dict.get("DP"))
            .map(|v| self.resolve(v))
        {
            Some(Val::Dict(d)) => vec![d.clone()],
            Some(Val::Arr(a)) => a
                .iter()
                .map(|v| self.dict(v).cloned().unwrap_or_default())
                .collect(),
            _ => Vec::new(),
        };
        names
            .into_iter()
            .enumerate()
            .map(|(i, n)| (n, parms.get(i).cloned().unwrap_or_default()))
            .collect()
    }

    fn raw_stream(&self, obj: &Obj) -> Option<&'a [u8]> {
        let (s, e) = obj.stream?;
        self.data.get(s..e)
    }

    /// Decodes the stream through its general-purpose filters (Flate, hex).
    /// Returns the data and, if an image codec (DCT, CCITT, JBIG2, JPX...) or
    /// an unsupported filter is next, its name.
    fn decode_chain(&self, obj: &Obj) -> Result<(Vec<u8>, Option<String>), String> {
        let Val::Dict(dict) = &obj.val else {
            return Err("not a stream".into());
        };
        let mut data = self.raw_stream(obj).ok_or("no stream data")?.to_vec();
        for (name, parms) in self.filters(dict) {
            match name.as_str() {
                "FlateDecode" | "Fl" => {
                    data = inflate_zlib(&data, MAX_STREAM).map_err(|e| e.to_string())?;
                    let predictor = self.int_of(parms.get("Predictor")).unwrap_or(1);
                    if predictor > 1 {
                        let colors = self.int_of(parms.get("Colors")).unwrap_or(1).max(1) as usize;
                        let bpc = self
                            .int_of(parms.get("BitsPerComponent"))
                            .unwrap_or(8)
                            .max(1) as usize;
                        let cols = self.int_of(parms.get("Columns")).unwrap_or(1).max(1) as usize;
                        data = undo_predictor(&data, predictor, colors, bpc, cols)?;
                    }
                }
                "ASCIIHexDecode" | "AHx" => {
                    // `hex_string` reads up to the closing '>' (or the end).
                    data = Lex { d: &data, p: 0 }.hex_string();
                }
                other => return Ok((data, Some(other.to_string()))),
            }
        }
        Ok((data, None))
    }
}

/// Undoes a PNG (10-15) or TIFF (2) predictor on `data`.
fn undo_predictor(
    data: &[u8],
    predictor: i64,
    colors: usize,
    bpc: usize,
    cols: usize,
) -> Result<Vec<u8>, String> {
    let bits_per_pixel = colors * bpc;
    let bpp = bits_per_pixel.div_ceil(8).max(1);
    let row = (cols * bits_per_pixel).div_ceil(8);
    if row == 0 {
        return Err("empty rows".into());
    }
    match predictor {
        2 => {
            if bpc != 8 {
                return Err("TIFF predictor on samples that are not 8 bit".into());
            }
            let mut out = data.to_vec();
            for r in out.chunks_mut(row) {
                for i in colors..r.len() {
                    r[i] = r[i].wrapping_add(r[i - colors]);
                }
            }
            Ok(out)
        }
        10..=15 => {
            let stride = row + 1;
            let rows = data.len() / stride;
            if rows == 0 {
                return Err("damaged compressed data".into());
            }
            let mut out = vec![0u8; rows * row];
            for y in 0..rows {
                let filter = data[y * stride];
                let src = &data[y * stride + 1..(y + 1) * stride];
                let (prev_rows, cur_rows) = out.split_at_mut(y * row);
                let prev: &[u8] = if y == 0 {
                    &[]
                } else {
                    &prev_rows[(y - 1) * row..]
                };
                let cur = &mut cur_rows[..row];
                for x in 0..row {
                    let a = if x >= bpp { cur[x - bpp] } else { 0 };
                    let b = prev.get(x).copied().unwrap_or(0);
                    let c = if x >= bpp {
                        prev.get(x - bpp).copied().unwrap_or(0)
                    } else {
                        0
                    };
                    cur[x] = src[x].wrapping_add(match filter {
                        0 => 0,
                        1 => a,
                        2 => b,
                        3 => ((u16::from(a) + u16::from(b)) / 2) as u8,
                        4 => paeth(a, b, c),
                        _ => return Err("damaged picture rows".into()),
                    });
                }
            }
            Ok(out)
        }
        _ => Ok(data.to_vec()),
    }
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (ia, ib, ic) = (i32::from(a), i32::from(b), i32::from(c));
    let p = ia + ib - ic;
    let (pa, pb, pc) = ((p - ia).abs(), (p - ib).abs(), (p - ic).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

// ----------------------------------------------------------------------
// Pages and their pictures
// ----------------------------------------------------------------------

/// The picture of a page as a file's worth of bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageData {
    Jpeg(Vec<u8>),
    Png(Vec<u8>),
}

impl PageData {
    /// The file extension of the data.
    pub fn extension(&self) -> &'static str {
        match self {
            PageData::Jpeg(_) => "jpg",
            PageData::Png(_) => "png",
        }
    }

    pub fn bytes(&self) -> &[u8] {
        match self {
            PageData::Jpeg(b) | PageData::Png(b) => b,
        }
    }
}

/// The picture of one page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageImage {
    /// The page's place in the PDF, 1-based (pages without a usable picture
    /// leave gaps).
    pub page: usize,
    pub width: u32,
    pub height: u32,
    pub data: PageData,
}

/// What [`page_images`] found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PdfPages {
    /// Pages whose picture could be read, in page order.
    pub pages: Vec<PageImage>,
    /// Pages in the file (0 when the page tree could not be read and the
    /// pictures were taken in file order).
    pub page_count: usize,
    /// Why other pages were left out.
    pub problems: Vec<String>,
}

/// An image XObject of a page.
struct Candidate {
    num: u32,
    pixels: u64,
}

impl Doc<'_> {
    fn catalog_pages(&self) -> Option<&Val> {
        // The last catalog wins (an incremental update replaces the first).
        let mut found = None;
        for n in &self.order {
            if let Some(Obj {
                val: Val::Dict(d), ..
            }) = self.objs.get(n)
            {
                if d.get("Type").and_then(Val::name) == Some("Catalog") {
                    found = d.get("Pages");
                }
            }
        }
        found
    }

    /// The `/Resources` of every page in order.
    fn page_resources(&self) -> Vec<Option<&Val>> {
        let mut out = Vec::new();
        if let Some(root) = self.catalog_pages() {
            self.walk_pages(root, None, &mut out, 0);
        }
        out
    }

    fn walk_pages<'b>(
        &'b self,
        node: &'b Val,
        inherited: Option<&'b Val>,
        out: &mut Vec<Option<&'b Val>>,
        depth: usize,
    ) {
        if depth > 32 || out.len() > 20_000 {
            return;
        }
        let Some(d) = self.dict(node) else {
            return;
        };
        let res = d.get("Resources").or(inherited);
        match d.get("Kids").map(|k| self.resolve(k)) {
            Some(Val::Arr(kids)) => {
                for k in kids {
                    self.walk_pages(k, res, out, depth + 1);
                }
            }
            _ => out.push(res),
        }
    }

    /// The image XObjects reachable from `resources` (forms are searched too).
    fn images_in(&self, resources: &Val, depth: usize, out: &mut Vec<Candidate>) {
        if depth > 3 {
            return;
        }
        let Some(res) = self.dict(resources) else {
            return;
        };
        let Some(xo) = res.get("XObject").and_then(|x| self.dict(x)) else {
            return;
        };
        let mut keys: Vec<&String> = xo.keys().collect();
        keys.sort();
        for k in keys {
            let Val::Ref(num) = xo[k] else {
                continue;
            };
            let Some(obj) = self.objs.get(&num) else {
                continue;
            };
            let Val::Dict(d) = &obj.val else {
                continue;
            };
            match d.get("Subtype").and_then(Val::name) {
                Some("Image") => {
                    let w = self.int_of(d.get("Width")).unwrap_or(0).max(0) as u64;
                    let h = self.int_of(d.get("Height")).unwrap_or(0).max(0) as u64;
                    if w > 0 && h > 0 && !out.iter().any(|c| c.num == num) {
                        out.push(Candidate { num, pixels: w * h });
                    }
                }
                Some("Form") => {
                    if let Some(r) = d.get("Resources") {
                        self.images_in(r, depth + 1, out);
                    }
                }
                _ => {}
            }
        }
    }

    /// Every image XObject in file order (for files with no page tree).
    fn all_images(&self) -> Vec<Candidate> {
        let mut out = Vec::new();
        for n in &self.order {
            let Some(o) = self.objs.get(n) else {
                continue;
            };
            if o.stream.is_none() {
                continue;
            }
            let Val::Dict(d) = &o.val else {
                continue;
            };
            if d.get("Subtype").and_then(Val::name) == Some("Image") {
                let w = self.int_of(d.get("Width")).unwrap_or(0).max(0) as u64;
                let h = self.int_of(d.get("Height")).unwrap_or(0).max(0) as u64;
                if w > 0 && h > 0 {
                    out.push(Candidate {
                        num: *n,
                        pixels: w * h,
                    });
                }
            }
        }
        out
    }

    fn color_space(&self, v: Option<&Val>) -> Result<ColorSpace, String> {
        let Some(v) = v else {
            return Ok(ColorSpace::Gray);
        };
        match self.resolve(v) {
            Val::Name(n) => match n.as_str() {
                "DeviceGray" | "G" | "CalGray" => Ok(ColorSpace::Gray),
                "DeviceRGB" | "RGB" | "CalRGB" => Ok(ColorSpace::Rgb),
                "DeviceCMYK" | "CMYK" => Ok(ColorSpace::Cmyk),
                other => Err(format!("the colour space {other}")),
            },
            Val::Arr(a) => {
                let kind = a.first().and_then(|x| self.resolve(x).name()).unwrap_or("");
                match kind {
                    "ICCBased" => {
                        let n = a
                            .get(1)
                            .and_then(|s| self.dict(s))
                            .and_then(|d| self.int_of(d.get("N")))
                            .unwrap_or(3);
                        match n {
                            1 => Ok(ColorSpace::Gray),
                            3 => Ok(ColorSpace::Rgb),
                            4 => Ok(ColorSpace::Cmyk),
                            _ => Err("an ICC colour space".into()),
                        }
                    }
                    "CalRGB" => Ok(ColorSpace::Rgb),
                    "CalGray" => Ok(ColorSpace::Gray),
                    "Indexed" | "I" => {
                        let base = self.color_space(a.get(1))?;
                        let comps = base.components();
                        if !matches!(base, ColorSpace::Gray | ColorSpace::Rgb | ColorSpace::Cmyk) {
                            return Err("an indexed colour space of this base".into());
                        }
                        let hival = self.int_of(a.get(2)).unwrap_or(255).clamp(0, 255) as usize;
                        let table: Vec<u8> = match a.get(3).map(|t| self.resolve(t)) {
                            Some(Val::Str(s)) => s.clone(),
                            _ => match a.get(3) {
                                Some(Val::Ref(n)) => self
                                    .objs
                                    .get(n)
                                    .and_then(|o| self.decode_chain(o).ok())
                                    .map(|(d, _)| d)
                                    .unwrap_or_default(),
                                _ => Vec::new(),
                            },
                        };
                        let mut palette = Vec::new();
                        for i in 0..=hival {
                            let e = table
                                .get(i * comps..(i + 1) * comps)
                                .unwrap_or(&[0u8; 4][..]);
                            palette.push(match base {
                                ColorSpace::Gray => [e[0], e[0], e[0]],
                                ColorSpace::Cmyk => cmyk_rgb(e),
                                _ => [e[0], e[1], e[2]],
                            });
                        }
                        Ok(ColorSpace::Indexed(palette))
                    }
                    other => Err(format!("the colour space {other}")),
                }
            }
            _ => Ok(ColorSpace::Gray),
        }
    }

    /// Reads image object `num` as a file.
    fn extract(&self, num: u32) -> Result<(u32, u32, PageData), String> {
        let obj = self.objs.get(&num).ok_or("a missing picture object")?;
        let Val::Dict(d) = &obj.val else {
            return Err("a picture that is not a stream".into());
        };
        let w = self.int_of(d.get("Width")).unwrap_or(0);
        let h = self.int_of(d.get("Height")).unwrap_or(0);
        if w <= 0 || h <= 0 || w > 40_000 || h > 40_000 || (w * h) as u64 > 250_000_000 {
            return Err("a picture of an unusable size".into());
        }
        let (w, h) = (w as usize, h as usize);
        let (data, codec) = self.decode_chain(obj)?;
        match codec.as_deref() {
            Some("DCTDecode" | "DCT") => {
                if !data.starts_with(&[0xFF, 0xD8]) {
                    return Err("a damaged JPEG picture".into());
                }
                let (jw, jh) = image_size(&data).map_or((w as u32, h as u32), |(a, b, _)| (a, b));
                return Ok((jw, jh, PageData::Jpeg(data)));
            }
            Some(other) => {
                return Err(format!(
                    "a picture stored with {other}, which this build cannot read"
                ))
            }
            None => {}
        }
        let mask = matches!(
            d.get("ImageMask").map(|v| self.resolve(v)),
            Some(Val::Bool(true))
        );
        let cs = if mask {
            ColorSpace::Gray
        } else {
            self.color_space(d.get("ColorSpace"))?
        };
        let bpc = if mask {
            1
        } else {
            self.int_of(d.get("BitsPerComponent")).unwrap_or(8) as usize
        };
        if !matches!(bpc, 1 | 2 | 4 | 8 | 16) {
            return Err("samples of an unusual size".into());
        }
        let invert = matches!(
            d.get("Decode").map(|v| self.resolve(v)),
            Some(Val::Arr(a)) if a.len() >= 2 && a[0].num() == Some(1.0) && a[1].num() == Some(0.0)
        );
        let comps = cs.components();
        let row = (w * comps * bpc).div_ceil(8);
        if data.len() < row * h {
            return Err("a picture with missing rows".into());
        }
        let mut rows = data;
        rows.truncate(row * h);
        if invert && !matches!(cs, ColorSpace::Indexed(_)) {
            for b in &mut rows {
                *b = !*b;
            }
        }
        let png = match cs {
            ColorSpace::Gray => png_file(w, h, 0, bpc as u8, None, &rows, row),
            ColorSpace::Rgb => {
                if bpc != 8 && bpc != 16 {
                    return Err("RGB samples that are not 8 or 16 bit".into());
                }
                png_file(w, h, 2, bpc as u8, None, &rows, row)
            }
            ColorSpace::Indexed(ref pal) => png_file(w, h, 3, bpc as u8, Some(pal), &rows, row),
            ColorSpace::Cmyk => {
                if bpc != 8 {
                    return Err("CMYK samples that are not 8 bit".into());
                }
                let mut rgb = Vec::with_capacity(w * h * 3);
                for px in rows.as_chunks::<4>().0 {
                    rgb.extend_from_slice(&cmyk_rgb(px));
                }
                png_file(w, h, 2, 8, None, &rgb, w * 3)
            }
        };
        Ok((w as u32, h as u32, PageData::Png(png)))
    }
}

#[derive(Clone, Debug, PartialEq)]
enum ColorSpace {
    Gray,
    Rgb,
    Cmyk,
    Indexed(Vec<[u8; 3]>),
}

impl ColorSpace {
    fn components(&self) -> usize {
        match self {
            ColorSpace::Gray | ColorSpace::Indexed(_) => 1,
            ColorSpace::Rgb => 3,
            ColorSpace::Cmyk => 4,
        }
    }
}

fn cmyk_rgb(p: &[u8]) -> [u8; 3] {
    let k = u32::from(*p.get(3).unwrap_or(&0));
    let f = |c: u8| (u32::from(255 - c) * (255 - k) / 255) as u8;
    [f(p[0]), f(p[1]), f(p[2])]
}

// ----- a minimal PNG writer -----

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    c ^ 0xFFFF_FFFF
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut body = kind.to_vec();
    body.extend_from_slice(data);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
}

/// A PNG of `rows` (`row` bytes per row, no filter bytes) as stored blocks.
fn png_file(
    w: usize,
    h: usize,
    color_type: u8,
    depth: u8,
    palette: Option<&[[u8; 3]]>,
    rows: &[u8],
    row: usize,
) -> Vec<u8> {
    let mut raw = Vec::with_capacity((row + 1) * h);
    for y in 0..h {
        raw.push(0);
        raw.extend_from_slice(&rows[y * row..(y + 1) * row]);
    }
    let mut out = vec![0x89, b'P', b'N', b'G', 13, 10, 26, 10];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[depth, color_type, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    if let Some(p) = palette {
        let plte: Vec<u8> = p.iter().flatten().copied().collect();
        chunk(&mut out, b"PLTE", &plte);
    }
    chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

/// The pictures of the pages of the PDF `pdf`, or a message that says why
/// none can be used.
pub fn page_images(pdf: &[u8]) -> Result<PdfPages, String> {
    if !pdf.starts_with(b"%PDF") && find(&pdf[..pdf.len().min(1024)], b"%PDF", 0).is_none() {
        return Err("That file is not a PDF".into());
    }
    let doc = Doc::parse(pdf);
    let mut result = PdfPages::default();
    let resources = doc.page_resources();
    if resources.is_empty() {
        // No page tree: the images in file order, one per page.
        let mut n = 0;
        for c in doc.all_images() {
            n += 1;
            match doc.extract(c.num) {
                Ok((width, height, data)) => result.pages.push(PageImage {
                    page: n,
                    width,
                    height,
                    data,
                }),
                Err(e) => result.problems.push(format!("picture {n}: {e}")),
            }
        }
    } else {
        result.page_count = resources.len();
        for (i, res) in resources.iter().enumerate() {
            let mut found = Vec::new();
            if let Some(r) = res {
                doc.images_in(r, 0, &mut found);
            }
            // The largest picture is the page (the rest are stamps and logos).
            found.sort_by_key(|c| std::cmp::Reverse(c.pixels));
            let Some(best) = found.first() else {
                result.problems.push(format!(
                    "page {}: no picture (vector drawing or text)",
                    i + 1
                ));
                continue;
            };
            match doc.extract(best.num) {
                Ok((width, height, data)) => result.pages.push(PageImage {
                    page: i + 1,
                    width,
                    height,
                    data,
                }),
                Err(e) => result.problems.push(format!("page {}: {e}", i + 1)),
            }
        }
    }
    if result.pages.is_empty() {
        let why = result
            .problems
            .iter()
            .find(|p| !p.contains("no picture"))
            .cloned();
        return Err(match why {
            Some(w) => format!("That PDF has {w}: export the page as a PNG and import that"),
            None => NO_PICTURE.to_string(),
        });
    }
    Ok(result)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    const JPG_A: &[u8] = include_bytes!("testdata/rgb444.jpg");
    const JPG_B: &[u8] = include_bytes!("testdata/gray.jpg");

    pub fn sample_pdf() -> Vec<u8> {
        let mut pdf = b"%PDF-1.4\n".to_vec();
        for (n, jpg) in [(1, JPG_A), (2, JPG_B)] {
            pdf.extend_from_slice(
                format!(
                    "{n} 0 obj\n<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\r\n",
                    jpg.len()
                )
                .as_bytes(),
            );
            pdf.extend_from_slice(jpg);
            pdf.extend_from_slice(b"\nendstream\nendobj\n");
        }
        // A Flate image and a content stream are not pictures we can use.
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Subtype /Image /Filter /FlateDecode /Length 4 >>\nstream\nabcd\nendstream\nendobj\n",
        );
        pdf.extend_from_slice(b"4 0 obj\n<< /Length 5 >>\nstream\nq Q\nendstream\nendobj\n%%EOF\n");
        pdf
    }

    /// A PDF writer for the tests: pages of one picture each, with a page
    /// tree, in the given object layout.
    pub struct Builder {
        out: Vec<u8>,
        objs: Vec<(u32, usize)>,
    }

    impl Builder {
        pub fn new() -> Builder {
            Builder {
                out: b"%PDF-1.5\n%\xE2\xE3\xCF\xD3\n".to_vec(),
                objs: Vec::new(),
            }
        }

        pub fn object(&mut self, num: u32, body: &str) {
            self.objs.push((num, self.out.len()));
            self.out
                .extend_from_slice(format!("{num} 0 obj\n{body}\nendobj\n").as_bytes());
        }

        pub fn stream(&mut self, num: u32, dict: &str, data: &[u8]) {
            self.objs.push((num, self.out.len()));
            self.out.extend_from_slice(
                format!("{num} 0 obj\n<< {dict} /Length {} >>\nstream\n", data.len()).as_bytes(),
            );
            self.out.extend_from_slice(data);
            self.out.extend_from_slice(b"\nendstream\nendobj\n");
        }

        pub fn finish(mut self, root: u32) -> Vec<u8> {
            let xref = self.out.len();
            let max = self.objs.iter().map(|o| o.0).max().unwrap_or(0);
            self.out.extend_from_slice(
                format!("xref\n0 {}\n0000000000 65535 f \n", max + 1).as_bytes(),
            );
            for n in 1..=max {
                let off = self.objs.iter().find(|o| o.0 == n).map_or(0, |o| o.1);
                self.out
                    .extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
            }
            self.out.extend_from_slice(
                format!(
                    "trailer\n<< /Size {} /Root {root} 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                    max + 1
                )
                .as_bytes(),
            );
            self.out
        }
    }

    /// Two pages with pictures: page 1 an 8-bit RGB Flate picture with PNG
    /// predictor 15 (a raw gradient), page 2 a JPEG; objects 1-4 plus the
    /// page tree.
    pub fn paged_pdf() -> (Vec<u8>, Vec<u8>) {
        let (w, h) = (6usize, 4usize);
        let mut rgb = Vec::new();
        for y in 0..h {
            for x in 0..w {
                rgb.extend_from_slice(&[(x * 40) as u8, (y * 60) as u8, 200]);
            }
        }
        // PNG-filtered rows (filter 1 = Sub on every row), then zlib.
        let mut filtered = Vec::new();
        for y in 0..h {
            filtered.push(1);
            let row = &rgb[y * w * 3..(y + 1) * w * 3];
            for i in 0..row.len() {
                let left = if i >= 3 { row[i - 3] } else { 0 };
                filtered.push(row[i].wrapping_sub(left));
            }
        }
        let z = zlib_stored(&filtered);
        let mut b = Builder::new();
        b.object(1, "<< /Type /Catalog /Pages 2 0 R >>");
        b.object(2, "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 /Resources << /XObject << /Im0 5 0 R >> >> >>");
        b.object(3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 400] >>");
        b.object(
            4,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 400] /Resources << /XObject << /Im1 6 0 R >> >> >>",
        );
        b.stream(
            5,
            &format!(
                "/Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /DecodeParms << /Predictor 15 /Colors 3 /Columns {w} /BitsPerComponent 8 >>"
            ),
            &z,
        );
        b.stream(
            6,
            "/Type /XObject /Subtype /Image /Width 40 /Height 24 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode",
            JPG_A,
        );
        (b.finish(1), rgb)
    }

    #[test]
    fn scanned_pages_are_the_jpeg_streams_in_file_order() {
        let found = jpeg_pictures(&sample_pdf());
        assert_eq!(found.len(), 2);
        assert_eq!((found[0].width, found[0].height), (40, 24));
        assert_eq!((found[1].width, found[1].height), (21, 17));
        assert_eq!(found[0].jpeg, JPG_A);
        assert_eq!(found[1].jpeg, JPG_B);
    }

    #[test]
    fn vector_pdfs_and_junk_hold_no_pictures() {
        assert!(jpeg_pictures(
            b"%PDF-1.4\n1 0 obj\n<< /Length 3 >>\nstream\nabc\nendstream\nendobj\n"
        )
        .is_empty());
        assert!(jpeg_pictures(b"").is_empty());
        assert!(jpeg_pictures(b"stream").is_empty());
    }

    #[test]
    fn values_parse_names_strings_arrays_dicts_and_references() {
        let mut lex = Lex {
            d: b"<< /A 12 /B [1 2.5 (x\\051y) <4142>] /C /N#61me /D 7 0 R /E << /F true >> >>",
            p: 0,
        };
        let Some(Val::Dict(d)) = lex.value() else {
            panic!("a dictionary");
        };
        assert_eq!(d["A"], Val::Int(12));
        assert_eq!(
            d["B"],
            Val::Arr(vec![
                Val::Int(1),
                Val::Real(2.5),
                Val::Str(b"x)y".to_vec()),
                Val::Str(b"AB".to_vec())
            ])
        );
        assert_eq!(d["C"], Val::Name("Name".into()));
        assert_eq!(d["D"], Val::Ref(7));
        assert!(matches!(&d["E"], Val::Dict(e) if e["F"] == Val::Bool(true)));
    }

    #[test]
    fn the_page_tree_gives_each_pages_picture_in_order() {
        let (pdf, rgb) = paged_pdf();
        let pages = page_images(&pdf).unwrap();
        assert_eq!(pages.page_count, 2);
        assert_eq!(pages.pages.len(), 2);
        // Page 1 (the Flate picture, resources inherited from the tree) is a
        // PNG that decodes to the pixels that went in.
        let p1 = &pages.pages[0];
        assert_eq!((p1.page, p1.width, p1.height), (1, 6, 4));
        let PageData::Png(png) = &p1.data else {
            panic!("a PNG");
        };
        let img = plan_library::image::decode(png).unwrap();
        assert_eq!((img.width, img.height), (6, 4));
        for (i, px) in img.rgba.as_chunks::<4>().0.iter().enumerate() {
            assert_eq!(&px[..3], &rgb[i * 3..i * 3 + 3], "pixel {i}");
        }
        // Page 2 keeps its JPEG as it was stored.
        let p2 = &pages.pages[1];
        assert_eq!(p2.page, 2);
        assert_eq!(p2.data, PageData::Jpeg(JPG_A.to_vec()));
        assert_eq!((p2.width, p2.height), (40, 24));
    }

    #[test]
    fn a_pdf_without_a_page_tree_gives_its_pictures_in_file_order() {
        let pages = page_images(&sample_pdf()).unwrap();
        assert_eq!(pages.page_count, 0);
        assert_eq!(pages.pages.len(), 2);
        assert_eq!(pages.pages[0].data, PageData::Jpeg(JPG_A.to_vec()));
        assert_eq!(pages.pages[1].data, PageData::Jpeg(JPG_B.to_vec()));
    }

    #[test]
    fn objects_packed_in_an_object_stream_are_found() {
        // Catalog, Pages and Page live inside an /ObjStm (PDF 1.5); the
        // picture is a gray 1-bit Flate image without a predictor.
        let bodies = [
            "<< /Type /Catalog /Pages 11 0 R >>",
            "<< /Type /Pages /Kids [12 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 11 0 R /Resources << /XObject << /X 20 0 R >> >> >>",
        ];
        let nums = [10u32, 11, 12];
        let mut head = String::new();
        let mut body = String::new();
        for (n, b) in nums.iter().zip(bodies) {
            head.push_str(&format!("{n} {} ", body.len()));
            body.push_str(b);
            body.push(' ');
        }
        let first = head.len();
        let packed = zlib_stored(format!("{head}{body}").as_bytes());
        // 16 x 2 pixels, 1 bit each: rows of 2 bytes.
        let rows = [0xF0u8, 0x0F, 0xAA, 0x55];
        let mut b = Builder::new();
        b.stream(
            9,
            &format!("/Type /ObjStm /N 3 /First {first} /Filter /FlateDecode"),
            &packed,
        );
        b.stream(
            20,
            "/Type /XObject /Subtype /Image /Width 16 /Height 2 /ColorSpace /DeviceGray /BitsPerComponent 1 /Filter /FlateDecode",
            &zlib_stored(&rows),
        );
        let pdf = b.finish(10);
        let pages = page_images(&pdf).unwrap();
        assert_eq!(pages.page_count, 1);
        let PageData::Png(png) = &pages.pages[0].data else {
            panic!("a PNG");
        };
        let img = plan_library::image::decode(png).unwrap();
        assert_eq!((img.width, img.height), (16, 2));
        // 0xF0: the first four pixels are white (1), the next four black.
        assert_eq!(img.rgba[0], 255);
        assert_eq!(img.rgba[4 * 4], 0);
        // 0xAA / 0x55: alternating pixels on the second row.
        assert_eq!(img.rgba[16 * 4], 255);
        assert_eq!(img.rgba[(16 + 1) * 4], 0);
        assert_eq!(img.rgba[(16 + 9) * 4], 255);
    }

    #[test]
    fn indexed_and_cmyk_pictures_become_rgb() {
        let mut b = Builder::new();
        b.object(1, "<< /Type /Catalog /Pages 2 0 R >>");
        b.object(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
        b.object(
            3,
            "<< /Type /Page /Parent 2 0 R /Resources << /XObject << /A 4 0 R >> >> >>",
        );
        // 4 x 1 indexed picture, 8 bit: palette red, green, blue, white.
        b.stream(
            4,
            "/Subtype /Image /Width 4 /Height 1 /BitsPerComponent 8 /ColorSpace [/Indexed /DeviceRGB 3 <FF0000 00FF00 0000FF FFFFFF>] /Filter /FlateDecode",
            &zlib_stored(&[0, 1, 2, 3]),
        );
        let pdf = b.finish(1);
        let pages = page_images(&pdf).unwrap();
        let PageData::Png(png) = &pages.pages[0].data else {
            panic!("a PNG");
        };
        let img = plan_library::image::decode(png).unwrap();
        assert_eq!(&img.rgba[0..3], &[255, 0, 0]);
        assert_eq!(&img.rgba[4..7], &[0, 255, 0]);
        assert_eq!(&img.rgba[8..11], &[0, 0, 255]);
        assert_eq!(&img.rgba[12..15], &[255, 255, 255]);
        assert_eq!(cmyk_rgb(&[0, 0, 0, 0]), [255, 255, 255]);
        assert_eq!(cmyk_rgb(&[0, 0, 0, 255]), [0, 0, 0]);
    }

    #[test]
    fn vector_pages_and_unreadable_codecs_say_to_export_a_png() {
        let mut b = Builder::new();
        b.object(1, "<< /Type /Catalog /Pages 2 0 R >>");
        b.object(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
        b.object(
            3,
            "<< /Type /Page /Parent 2 0 R /Resources << >> /Contents 4 0 R >>",
        );
        b.stream(4, "", b"0 0 m 100 100 l S");
        let err = page_images(&b.finish(1)).unwrap_err();
        assert!(err.contains("export the page as a PNG"), "{err}");

        let mut b = Builder::new();
        b.object(1, "<< /Type /Catalog /Pages 2 0 R >>");
        b.object(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
        b.object(
            3,
            "<< /Type /Page /Parent 2 0 R /Resources << /XObject << /A 4 0 R >> >> >>",
        );
        b.stream(
            4,
            "/Subtype /Image /Width 8 /Height 8 /BitsPerComponent 1 /ColorSpace /DeviceGray /Filter /CCITTFaxDecode",
            b"xx",
        );
        let err = page_images(&b.finish(1)).unwrap_err();
        assert!(err.contains("CCITTFaxDecode"), "{err}");
        assert!(err.contains("export the page as a PNG"), "{err}");

        assert!(page_images(b"hello").unwrap_err().contains("not a PDF"));
    }

    #[test]
    fn the_png_predictors_undo() {
        // Up filter on two rows of three bytes.
        let data = [2, 1, 2, 3, 2, 1, 1, 1];
        assert_eq!(
            undo_predictor(&data, 12, 1, 8, 3).unwrap(),
            vec![1, 2, 3, 2, 3, 4]
        );
        // Average and Paeth stay in range on the first row.
        assert_eq!(undo_predictor(&[3, 4, 4], 15, 1, 8, 2).unwrap(), vec![4, 6]);
        assert_eq!(
            undo_predictor(&[4, 5, 5], 15, 1, 8, 2).unwrap(),
            vec![5, 10]
        );
        // TIFF predictor adds the pixel to the left.
        assert_eq!(
            undo_predictor(&[1, 1, 1, 1], 2, 1, 8, 4).unwrap(),
            vec![1, 2, 3, 4]
        );
        assert!(undo_predictor(&[9, 1, 1], 15, 1, 8, 2).is_err());
    }
}
