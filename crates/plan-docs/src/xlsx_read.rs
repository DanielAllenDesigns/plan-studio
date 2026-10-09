//! A small `.xlsx` and CSV reader, the other half of [`crate::xlsx`].
//!
//! A workbook is a zip of XML parts. [`read_xlsx`] reads the zip's central
//! directory (stored and deflated entries; the inflate decoder is
//! `plan_calib::inflate`), the sheet list and its relationships, the shared
//! strings and each worksheet's cells, and gives back every sheet as a dense
//! table of text: a number cell as its shortest exact decimal, a Yes/No cell
//! as `TRUE` / `FALSE`, a shared or inline string as its text. Formulas are
//! read by their cached value. Styles, merged cells and everything else are
//! ignored, which is all a property exchange needs.
//!
//! The XML reader is a small pull tokenizer, enough for the parts Excel,
//! LibreOffice, Numbers and Google Sheets write (namespace prefixes, CDATA,
//! numeric character references, rich-text runs).

use plan_library::archive::crc32;
use std::collections::BTreeMap;
use std::io::{self, Write};

/// Largest part we inflate: a sheet of 200 000 rows is a few MB.
const MAX_PART: usize = 128 * 1024 * 1024;
/// Largest sheet we expand into a dense table, in cells.
const MAX_CELLS: usize = 4_000_000;

/// One worksheet as a table of text. Short rows are padded with "".
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReadSheet {
    pub name: String,
    pub hidden: bool,
    pub rows: Vec<Vec<String>>,
}

// ===================================================================
// Zip
// ===================================================================

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(at)?, *b.get(at + 1)?]))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *b.get(at)?,
        *b.get(at + 1)?,
        *b.get(at + 2)?,
        *b.get(at + 3)?,
    ]))
}

/// A writer that refuses more than `MAX_PART` bytes.
struct Capped(Vec<u8>);

impl Write for Capped {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.0.len() + buf.len() > MAX_PART {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "part too large"));
        }
        self.0.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Every file of the zip `bytes`, decompressed, by name.
pub fn read_zip_entries(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, String> {
    const EOCD: u32 = 0x0605_4b50;
    const CENTRAL: u32 = 0x0201_4b50;
    const LOCAL: u32 = 0x0403_4b50;
    if bytes.len() < 22 {
        return Err("This is not an Excel workbook (too small).".into());
    }
    let end = (0..=bytes.len() - 22)
        .rev()
        .take(65_557)
        .find(|&i| u32_at(bytes, i) == Some(EOCD))
        .ok_or("This is not an Excel workbook (not a zip file).")?;
    let count = u16_at(bytes, end + 10).unwrap_or(0) as usize;
    let mut at = u32_at(bytes, end + 16).unwrap_or(0) as usize;
    let mut out = BTreeMap::new();
    for _ in 0..count {
        if u32_at(bytes, at) != Some(CENTRAL) {
            return Err("The workbook's directory is damaged.".into());
        }
        let flags = u16_at(bytes, at + 8).unwrap_or(0);
        let method = u16_at(bytes, at + 10).unwrap_or(0xFFFF);
        let crc = u32_at(bytes, at + 16).unwrap_or(0);
        let csize = u32_at(bytes, at + 20).unwrap_or(0) as usize;
        let usize_ = u32_at(bytes, at + 24).unwrap_or(0) as usize;
        let nlen = u16_at(bytes, at + 28).unwrap_or(0) as usize;
        let xlen = u16_at(bytes, at + 30).unwrap_or(0) as usize;
        let clen = u16_at(bytes, at + 32).unwrap_or(0) as usize;
        let local = u32_at(bytes, at + 42).unwrap_or(0) as usize;
        let name = bytes
            .get(at + 46..at + 46 + nlen)
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .ok_or("The workbook's directory is damaged.")?;
        at += 46 + nlen + xlen + clen;
        if name.ends_with('/') {
            continue;
        }
        // Only the parts a workbook is read from are expanded.
        let wanted = name.starts_with("xl/") && name.ends_with(".xml")
            || name.ends_with(".rels")
            || name == "[Content_Types].xml";
        if !wanted {
            continue;
        }
        if flags & 1 != 0 {
            return Err("This workbook is password protected; remove the password first.".into());
        }
        if usize_ > MAX_PART {
            return Err(format!("\"{name}\" is too large to read."));
        }
        if u32_at(bytes, local) != Some(LOCAL) {
            return Err(format!("\"{name}\" is damaged in the workbook."));
        }
        let lnlen = u16_at(bytes, local + 26).unwrap_or(0) as usize;
        let lxlen = u16_at(bytes, local + 28).unwrap_or(0) as usize;
        let start = local + 30 + lnlen + lxlen;
        let raw = bytes
            .get(start..start.saturating_add(csize))
            .ok_or_else(|| format!("\"{name}\" is cut off in the workbook."))?;
        let data = match method {
            0 => raw.to_vec(),
            8 => {
                let mut w = Capped(Vec::with_capacity(usize_.min(1 << 24)));
                plan_calib::inflate::inflate(raw, &mut w)
                    .map_err(|e| format!("\"{name}\" could not be decompressed: {e}"))?;
                w.0
            }
            m => return Err(format!("\"{name}\" uses compression method {m}.")),
        };
        if crc32(&data) != crc {
            return Err(format!("\"{name}\" is damaged (checksum mismatch)."));
        }
        out.insert(name, data);
    }
    Ok(out)
}

// ===================================================================
// XML
// ===================================================================

#[derive(Debug, Clone, PartialEq)]
enum Ev {
    Start {
        name: String,
        attrs: Vec<(String, String)>,
        empty: bool,
    },
    End(String),
    Text(String),
}

/// The part of a qualified name after the last `:`.
fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(j) = rest.find(';').filter(|j| *j <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..j];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            e if e.starts_with("#x") || e.starts_with("#X") => u32::from_str_radix(&e[2..], 16)
                .ok()
                .and_then(char::from_u32),
            e if e.starts_with('#') => e[1..].parse::<u32>().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => out.push(c),
            None => out.push_str(&rest[..=j]),
        }
        rest = &rest[j + 1..];
    }
    out.push_str(rest);
    out
}

/// Excel writes `_x000D_` for characters XML cannot carry.
fn unescape_x(s: &str) -> String {
    if !s.contains("_x") {
        return s.to_string();
    }
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'_'
            && i + 7 <= b.len()
            && b[i + 1] == b'x'
            && b[i + 6] == b'_'
            && b[i + 2..i + 6].iter().all(u8::is_ascii_hexdigit)
        {
            let code = u32::from_str_radix(&s[i + 2..i + 6], 16).unwrap_or(0);
            if let Some(c) = char::from_u32(code) {
                out.push(c);
                i += 7;
                continue;
            }
        }
        // Copy one whole char.
        let ch = s[i..].chars().next().unwrap_or('_');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn parse_attrs(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        let ns = i;
        while i < b.len() && b[i] != b'=' && !b[i].is_ascii_whitespace() && b[i] != b'/' {
            i += 1;
        }
        let name = &s[ns..i];
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || b[i] != b'=' {
            continue;
        }
        i += 1;
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || (b[i] != b'"' && b[i] != b'\'') {
            continue;
        }
        let q = b[i];
        i += 1;
        let vs = i;
        while i < b.len() && b[i] != q {
            i += 1;
        }
        out.push((local(name).to_string(), unescape(&s[vs..i.min(b.len())])));
        i += 1;
    }
    out
}

fn tokenize(xml: &str) -> Vec<Ev> {
    let mut out = Vec::new();
    let mut rest = xml.strip_prefix('\u{feff}').unwrap_or(xml);
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("<![CDATA[") {
            let end = r.find("]]>").unwrap_or(r.len());
            out.push(Ev::Text(r[..end].to_string()));
            rest = r.get(end + 3..).unwrap_or("");
        } else if let Some(r) = rest.strip_prefix("<!--") {
            rest = r.find("-->").map_or("", |e| &r[e + 3..]);
        } else if let Some(r) = rest.strip_prefix("<?") {
            rest = r.find("?>").map_or("", |e| &r[e + 2..]);
        } else if let Some(r) = rest.strip_prefix("<!") {
            rest = r.find('>').map_or("", |e| &r[e + 1..]);
        } else if let Some(r) = rest.strip_prefix("</") {
            let end = r.find('>').unwrap_or(r.len());
            out.push(Ev::End(local(r[..end].trim()).to_string()));
            rest = r.get(end + 1..).unwrap_or("");
        } else if let Some(r) = rest.strip_prefix('<') {
            let end = r.find('>').unwrap_or(r.len());
            let body = &r[..end];
            let empty = body.ends_with('/');
            let body = body.trim_end_matches('/');
            let name_end = body
                .find(|c: char| c.is_ascii_whitespace())
                .unwrap_or(body.len());
            out.push(Ev::Start {
                name: local(&body[..name_end]).to_string(),
                attrs: parse_attrs(&body[name_end..]),
                empty,
            });
            rest = r.get(end + 1..).unwrap_or("");
        } else {
            let end = rest.find('<').unwrap_or(rest.len());
            out.push(Ev::Text(unescape(&rest[..end])));
            rest = &rest[end..];
        }
    }
    out
}

fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

// ===================================================================
// Workbook parts
// ===================================================================

/// The strings of `xl/sharedStrings.xml`, rich-text runs joined and phonetic
/// guides dropped.
fn shared_strings(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Option<String> = None;
    let mut in_t = false;
    let mut skip = 0usize;
    for ev in tokenize(xml) {
        match ev {
            Ev::Start { name, empty, .. } => match name.as_str() {
                "si" => {
                    cur = Some(String::new());
                    if empty {
                        out.push(cur.take().unwrap_or_default());
                    }
                }
                "rPh" if !empty => skip += 1,
                "t" if skip == 0 && !empty => in_t = true,
                _ => {}
            },
            Ev::End(name) => match name.as_str() {
                "si" => out.push(cur.take().unwrap_or_default()),
                "rPh" => skip = skip.saturating_sub(1),
                "t" => in_t = false,
                _ => {}
            },
            Ev::Text(t) => {
                if let (true, Some(c)) = (in_t, cur.as_mut()) {
                    c.push_str(&unescape_x(&t));
                }
            }
        }
    }
    out
}

/// `(name, rel id, hidden)` per sheet of `xl/workbook.xml`.
fn sheet_list(xml: &str) -> Vec<(String, String, bool)> {
    tokenize(xml)
        .into_iter()
        .filter_map(|ev| match ev {
            Ev::Start { name, attrs, .. } if name == "sheet" => Some((
                attr(&attrs, "name")?.to_string(),
                attr(&attrs, "id")?.to_string(),
                matches!(attr(&attrs, "state"), Some("hidden" | "veryHidden")),
            )),
            _ => None,
        })
        .collect()
}

/// Relationship id to target path, relative to the folder of the workbook.
fn relationships(xml: &str) -> BTreeMap<String, String> {
    tokenize(xml)
        .into_iter()
        .filter_map(|ev| match ev {
            Ev::Start { name, attrs, .. } if name == "Relationship" => Some((
                attr(&attrs, "Id")?.to_string(),
                attr(&attrs, "Target")?.to_string(),
            )),
            _ => None,
        })
        .collect()
}

/// Column letters to a zero-based index (`A` = 0, `AA` = 26).
fn col_index(letters: &str) -> Option<usize> {
    if letters.is_empty() {
        return None;
    }
    let mut n = 0usize;
    for c in letters.bytes() {
        if !c.is_ascii_alphabetic() {
            return None;
        }
        n = n * 26 + (c.to_ascii_uppercase() - b'A') as usize + 1;
    }
    Some(n - 1)
}

/// `B7` to (column 1, row 6).
fn parse_ref(r: &str) -> Option<(usize, usize)> {
    let split = r.find(|c: char| c.is_ascii_digit())?;
    let col = col_index(&r[..split])?;
    let row = r[split..].parse::<usize>().ok()?.checked_sub(1)?;
    Some((col, row))
}

/// A number cell's text: the shortest decimal that reads back the same.
fn number_text(raw: &str) -> String {
    match raw.trim().parse::<f64>() {
        Ok(v) if v.is_finite() => format!("{v}"),
        _ => raw.trim().to_string(),
    }
}

fn sheet_cells(xml: &str, strings: &[String]) -> Result<Vec<Vec<String>>, String> {
    let mut cells: BTreeMap<(usize, usize), String> = BTreeMap::new();
    let mut at: Option<(usize, usize)> = None;
    let mut ty = String::new();
    let mut value = String::new();
    let mut inline = String::new();
    let mut in_v = false;
    let mut in_t = false;
    let mut in_is = false;
    let mut skip = 0usize;
    let mut next_col = 0usize;
    let mut row = 0usize;
    for ev in tokenize(xml) {
        match ev {
            Ev::Start { name, attrs, empty } => match name.as_str() {
                "row" => {
                    row = attr(&attrs, "r")
                        .and_then(|r| r.parse::<usize>().ok())
                        .map_or(row, |r| r.saturating_sub(1));
                    next_col = 0;
                }
                "c" => {
                    let pos = attr(&attrs, "r")
                        .and_then(parse_ref)
                        .unwrap_or((next_col, row));
                    at = Some(pos);
                    next_col = pos.0 + 1;
                    ty = attr(&attrs, "t").unwrap_or("n").to_string();
                    value.clear();
                    inline.clear();
                    if empty {
                        at = None;
                    }
                }
                "v" if !empty => in_v = true,
                "is" if !empty => in_is = true,
                "rPh" if !empty => skip += 1,
                "t" if in_is && skip == 0 && !empty => in_t = true,
                _ => {}
            },
            Ev::End(name) => match name.as_str() {
                "v" => in_v = false,
                "is" => in_is = false,
                "rPh" => skip = skip.saturating_sub(1),
                "t" => in_t = false,
                "c" => {
                    if let Some(pos) = at.take() {
                        let text = match ty.as_str() {
                            "s" => value
                                .trim()
                                .parse::<usize>()
                                .ok()
                                .and_then(|i| strings.get(i).cloned())
                                .unwrap_or_default(),
                            "inlineStr" => unescape_x(&inline),
                            "str" => unescape_x(&value),
                            "b" => if value.trim() == "1" { "TRUE" } else { "FALSE" }.to_string(),
                            "e" => String::new(),
                            _ if value.trim().is_empty() => String::new(),
                            _ => number_text(&value),
                        };
                        if !text.is_empty() {
                            cells.insert((pos.1, pos.0), text);
                        }
                    }
                }
                _ => {}
            },
            Ev::Text(t) => {
                if in_v {
                    value.push_str(&t);
                } else if in_t {
                    inline.push_str(&t);
                }
            }
        }
    }
    let rows = cells.keys().map(|(r, _)| r + 1).max().unwrap_or(0);
    let cols = cells.keys().map(|(_, c)| c + 1).max().unwrap_or(0);
    if rows.saturating_mul(cols) > MAX_CELLS {
        return Err("This sheet is too large to import.".into());
    }
    let mut out = vec![vec![String::new(); cols]; rows];
    for ((r, c), v) in cells {
        out[r][c] = v;
    }
    Ok(out)
}

/// Resolves a relationship target against the workbook folder `xl/`.
fn resolve(target: &str) -> String {
    match target.strip_prefix('/') {
        Some(abs) => abs.to_string(),
        None => {
            let mut parts: Vec<&str> = vec!["xl"];
            for seg in target.split('/') {
                match seg {
                    ".." => {
                        parts.pop();
                    }
                    "." | "" => {}
                    s => parts.push(s),
                }
            }
            parts.join("/")
        }
    }
}

/// Reads every worksheet of the `.xlsx` workbook `bytes`.
pub fn read_xlsx(bytes: &[u8]) -> Result<Vec<ReadSheet>, String> {
    let parts = read_zip_entries(bytes)?;
    let text = |name: &str| -> Option<String> {
        parts
            .get(name)
            .map(|b| String::from_utf8_lossy(b).into_owned())
    };
    let wb =
        text("xl/workbook.xml").ok_or("This is not an Excel workbook (no xl/workbook.xml).")?;
    let rels = relationships(&text("xl/_rels/workbook.xml.rels").unwrap_or_default());
    let strings = text("xl/sharedStrings.xml")
        .map(|x| shared_strings(&x))
        .unwrap_or_default();
    let mut out = Vec::new();
    for (name, rid, hidden) in sheet_list(&wb) {
        let Some(target) = rels.get(&rid) else {
            continue;
        };
        let path = resolve(target);
        let Some(xml) = text(&path) else {
            // A chart sheet or a missing part: nothing to read.
            continue;
        };
        out.push(ReadSheet {
            name,
            hidden,
            rows: sheet_cells(&xml, &strings)?,
        });
    }
    Ok(out)
}

// ===================================================================
// CSV
// ===================================================================

/// RFC 4180 CSV: quoted fields with `""`, commas or semicolons-free, CR LF or
/// LF rows. A leading byte order mark is dropped. Blank lines are skipped.
pub fn read_csv(text: &str) -> Vec<Vec<String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut any = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' if field.is_empty() => {
                quoted = true;
                any = true;
            }
            ',' => {
                row.push(std::mem::take(&mut field));
                any = true;
            }
            '\r' => {}
            '\n' => {
                if any || !field.is_empty() || !row.is_empty() {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                any = false;
            }
            c => {
                field.push(c);
                any = true;
            }
        }
    }
    if any || !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xlsx::{to_xlsx_edit, ColumnData, EditColumn, EditSheet};

    #[test]
    fn csv_handles_quotes_crlf_and_bom() {
        let rows = read_csv("\u{feff}a,b\r\n\"3'-0\"\"\",\"x,y\"\r\n\r\nlast,");
        assert_eq!(rows[0], ["a", "b"]);
        assert_eq!(rows[1], ["3'-0\"", "x,y"]);
        assert_eq!(rows[2], ["last", ""]);
        assert_eq!(rows.len(), 3);
        assert!(read_csv("").is_empty());
        assert_eq!(read_csv("a,\"multi\nline\"\n")[0][1], "multi\nline");
    }

    #[test]
    fn xml_entities_and_excel_escapes_are_decoded() {
        assert_eq!(
            unescape("R&amp;D &lt;x&gt; &#65;&#x42; &bogus;"),
            "R&D <x> AB &bogus;"
        );
        assert_eq!(unescape_x("a_x000D_b_x0041_ _x00"), "a\rbA _x00");
    }

    #[test]
    fn references_and_numbers() {
        assert_eq!(parse_ref("A1"), Some((0, 0)));
        assert_eq!(parse_ref("AA10"), Some((26, 9)));
        assert_eq!(parse_ref("1A"), None);
        assert_eq!(number_text("12"), "12");
        assert_eq!(number_text("3.1400000000000001"), "3.14");
        assert_eq!(number_text("1E3"), "1000");
        assert_eq!(resolve("worksheets/sheet1.xml"), "xl/worksheets/sheet1.xml");
        assert_eq!(
            resolve("/xl/worksheets/sheet2.xml"),
            "xl/worksheets/sheet2.xml"
        );
        assert_eq!(resolve("../xl/styles.xml"), "xl/styles.xml");
    }

    /// A workbook the way Excel writes it: shared strings, rich text runs,
    /// namespace prefixes, sparse cells, booleans, a formula with a cached
    /// value, a hidden sheet. Stored (not deflated) here; the deflate path is
    /// covered below.
    fn excel_like() -> Vec<u8> {
        let parts: Vec<(String, Vec<u8>)> = vec![
            ("[Content_Types].xml".into(), b"<Types/>".to_vec()),
            (
                "xl/workbook.xml".into(),
                br#"<?xml version="1.0"?><x:workbook xmlns:x="m" xmlns:r="r"><x:sheets>
<x:sheet name="Doors &amp; Windows" sheetId="1" r:id="rId1"/>
<x:sheet name="_meta" sheetId="2" state="hidden" r:id="rId2"/></x:sheets></x:workbook>"#
                    .to_vec(),
            ),
            (
                "xl/_rels/workbook.xml.rels".into(),
                br#"<Relationships>
<Relationship Id="rId1" Type="t" Target="worksheets/sheet1.xml"/>
<Relationship Id="rId2" Type="t" Target="/xl/worksheets/sheet2.xml"/></Relationships>"#
                    .to_vec(),
            ),
            (
                "xl/sharedStrings.xml".into(),
                "<sst><si><t>Mark</t></si>\
                 <si><r><t>Rich </t></r><r><t xml:space=\"preserve\">text</t></r><rPh><t>ignored</t></rPh></si>\
                 <si><t>caf\u{e9} &amp; co</t></si></sst>"
                    .as_bytes()
                    .to_vec(),
            ),
            (
                "xl/worksheets/sheet1.xml".into(),
                br#"<worksheet><sheetData>
<row r="1"><c r="A1" t="s"><v>0</v></c><c r="C1" t="s"><v>1</v></c></row>
<row r="3"><c r="A3" t="s"><v>2</v></c><c r="B3"><v>3.5</v></c><c r="C3" t="b"><v>1</v></c>
<c r="D3"><f>B3*2</f><v>7</v></c><c r="E3" t="inlineStr"><is><t>inline</t></is></c><c r="F3" t="e"><v>#N/A</v></c></row>
</sheetData></worksheet>"#
                    .to_vec(),
            ),
            (
                "xl/worksheets/sheet2.xml".into(),
                br#"<worksheet><sheetData><row r="1"><c r="A1" t="str"><v>formula text</v></c></row></sheetData></worksheet>"#
                    .to_vec(),
            ),
        ];
        plan_library::archive::write_zip(&parts)
    }

    #[test]
    fn an_excel_style_workbook_reads_into_dense_text_tables() {
        let sheets = read_xlsx(&excel_like()).unwrap();
        assert_eq!(sheets.len(), 2);
        assert_eq!(sheets[0].name, "Doors & Windows");
        assert!(!sheets[0].hidden);
        assert_eq!(sheets[0].rows.len(), 3);
        assert_eq!(sheets[0].rows[0], ["Mark", "", "Rich text", "", ""]);
        assert_eq!(sheets[0].rows[1], [""; 5], "an absent row is blank");
        assert_eq!(
            sheets[0].rows[2],
            ["caf\u{e9} & co", "3.5", "TRUE", "7", "inline"]
        );
        assert!(sheets[1].hidden);
        assert_eq!(sheets[1].rows[0][0], "formula text");
    }

    #[test]
    fn a_written_workbook_reads_back_cell_for_cell() {
        let cols = vec![
            EditColumn {
                header: "PlanStudio ID".into(),
                hidden: true,
                ..EditColumn::default()
            },
            EditColumn {
                header: "Mark".into(),
                editable: true,
                ..EditColumn::default()
            },
            EditColumn {
                header: "Qty".into(),
                editable: true,
                data: ColumnData::Number,
                ..EditColumn::default()
            },
            EditColumn {
                header: "Size".into(),
                ..EditColumn::default()
            },
        ];
        let rows = vec![
            vec![
                "door:1".into(),
                "007".into(),
                "2".into(),
                "3'-0\" x 6'-8\"".into(),
            ],
            vec![
                "door:2".into(),
                "R&D <odd> \"q\"".into(),
                "".into(),
                "12.5".into(),
            ],
            vec!["door:3".into(), "1.50".into(), "x".into(), "".into()],
        ];
        let book = to_xlsx_edit(&[
            EditSheet {
                title: "Doors".into(),
                protect: true,
                columns: cols,
                rows: rows.clone(),
                ..EditSheet::default()
            },
            EditSheet {
                title: "_meta".into(),
                hidden: true,
                columns: vec![EditColumn::default(); 2],
                rows: vec![vec!["k".into(), "v".into()]],
                ..EditSheet::default()
            },
        ]);
        let back = read_xlsx(&book).unwrap();
        assert_eq!(back[0].name, "Doors");
        assert_eq!(back[0].rows[0], ["PlanStudio ID", "Mark", "Qty", "Size"]);
        assert_eq!(&back[0].rows[1..], &rows[..], "text survives exactly");
        assert!(back[1].hidden && back[1].name == "_meta");
    }

    #[test]
    fn deflated_parts_are_inflated_and_checked() {
        // A hand-made deflate stream: one stored block holding "hi".
        let raw = [0x01u8, 0x02, 0x00, 0xFD, 0xFF, b'h', b'i'];
        let crc = crc32(b"hi");
        let name = b"xl/a.xml";
        let mut zip = Vec::new();
        // local header
        zip.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        zip.extend_from_slice(&[20, 0, 0, 0, 8, 0, 0, 0, 0, 0]);
        zip.extend_from_slice(&crc.to_le_bytes());
        zip.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        zip.extend_from_slice(&2u32.to_le_bytes());
        zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(name);
        zip.extend_from_slice(&raw);
        let cd_at = zip.len();
        zip.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        zip.extend_from_slice(&[20, 0, 20, 0, 0, 0, 8, 0, 0, 0, 0, 0]);
        zip.extend_from_slice(&crc.to_le_bytes());
        zip.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        zip.extend_from_slice(&2u32.to_le_bytes());
        zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
        zip.extend_from_slice(&[0; 12]);
        zip.extend_from_slice(&0u32.to_le_bytes());
        zip.extend_from_slice(name);
        let cd_len = zip.len() - cd_at;
        zip.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        zip.extend_from_slice(&[0, 0, 0, 0, 1, 0, 1, 0]);
        zip.extend_from_slice(&(cd_len as u32).to_le_bytes());
        zip.extend_from_slice(&(cd_at as u32).to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        let parts = read_zip_entries(&zip).unwrap();
        assert_eq!(parts["xl/a.xml"], b"hi");
        // A flipped payload byte is caught by the checksum.
        let mut bad = zip.clone();
        let at = 30 + name.len() + 5;
        bad[at] ^= 0xFF;
        assert!(read_zip_entries(&bad).unwrap_err().contains("checksum"));
    }

    #[test]
    fn junk_is_refused_with_a_message() {
        assert!(read_xlsx(b"hello").unwrap_err().contains("not an Excel"));
        assert!(read_xlsx(&[0u8; 100]).unwrap_err().contains("not an Excel"));
        let no_wb = plan_library::archive::write_zip(&[("x.txt".into(), b"a".to_vec())]);
        assert!(read_xlsx(&no_wb).unwrap_err().contains("workbook.xml"));
    }
}
