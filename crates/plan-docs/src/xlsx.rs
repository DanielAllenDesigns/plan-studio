//! A minimal Excel workbook (`.xlsx`) writer for schedules and the Materials
//! List: one worksheet, a bold frozen header row, text cells as inline
//! strings, numbers and dollar amounts as numbers. A workbook is a zip of a
//! few XML parts; the zip is written stored (no compression) by
//! `plan_library::archive::write_zip`.

use crate::schedule::Schedule;

/// Longest sheet name Excel accepts.
const MAX_SHEET_NAME: usize = 31;

/// Style ids in `xl/styles.xml`: 0 plain, 1 bold header, 2 dollars.
const STYLE_HEADER: u8 = 1;
const STYLE_MONEY: u8 = 2;

/// What a cell holds.
#[derive(Debug, Clone, PartialEq)]
enum Cell {
    Text(String),
    Number(f64),
    Money(f64),
}

/// Escapes `s` for XML text and drops characters XML 1.0 cannot carry.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out
}

/// A worksheet name Excel accepts: no `[]:*?/\`, at most 31 characters, not
/// empty, and not starting or ending with an apostrophe.
pub fn sheet_name(title: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| if "[]:*?/\\".contains(c) { ' ' } else { c })
        .collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let s = s.trim_matches('\'').trim();
    let s: String = s.chars().take(MAX_SHEET_NAME).collect();
    let s = s.trim().to_string();
    if s.is_empty() {
        "Sheet1".to_string()
    } else {
        s
    }
}

/// Cell reference such as `A1`, `Z3`, `AA10` for zero-based `col` and `row`.
fn cell_ref(col: usize, row: usize) -> String {
    let mut letters = Vec::new();
    let mut n = col + 1;
    while n > 0 {
        let rem = (n - 1) % 26;
        letters.push((b'A' + rem as u8) as char);
        n = (n - 1) / 26;
    }
    letters.reverse();
    format!("{}{}", letters.into_iter().collect::<String>(), row + 1)
}

/// Reads `text` as a number when the whole of it is one (no leading zeros,
/// so `007` and `3068`-style codes with a zero stay text where that matters),
/// or as dollars (`$1,234.50`).
fn classify(text: &str) -> Cell {
    let t = text.trim();
    if let Some(rest) = t.strip_prefix('$') {
        let plain: String = rest.chars().filter(|c| *c != ',').collect();
        if is_decimal(&plain) {
            if let Ok(v) = plain.parse::<f64>() {
                return Cell::Money(v);
            }
        }
    }
    if is_decimal(t) {
        if let Ok(v) = t.parse::<f64>() {
            return Cell::Number(v);
        }
    }
    Cell::Text(text.to_string())
}

/// `-?(0|[1-9][0-9]*)(\.[0-9]+)?`: plain decimals, nothing Excel would reshape.
fn is_decimal(s: &str) -> bool {
    let s = s.strip_prefix('-').unwrap_or(s);
    let (int, frac) = match s.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (s, None),
    };
    let int_ok = match int.len() {
        0 => false,
        1 => int.bytes().all(|b| b.is_ascii_digit()),
        _ => !int.starts_with('0') && int.bytes().all(|b| b.is_ascii_digit()),
    };
    int_ok && frac.is_none_or(|f| !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()))
}

fn push_cell(xml: &mut String, col: usize, row: usize, cell: &Cell, style: u8) {
    let r = cell_ref(col, row);
    match cell {
        Cell::Text(t) if t.is_empty() => {
            if style != 0 {
                xml.push_str(&format!("<c r=\"{r}\" s=\"{style}\"/>"));
            }
        }
        Cell::Text(t) => {
            let s = if style == 0 {
                String::new()
            } else {
                format!(" s=\"{style}\"")
            };
            xml.push_str(&format!(
                "<c r=\"{r}\"{s} t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
                xml_escape(t)
            ));
        }
        Cell::Number(v) => xml.push_str(&format!("<c r=\"{r}\"><v>{v}</v></c>")),
        Cell::Money(v) => xml.push_str(&format!("<c r=\"{r}\" s=\"{STYLE_MONEY}\"><v>{v}</v></c>")),
    }
}

/// The worksheet XML: header row, frozen, then the data rows.
fn sheet_xml(columns: &[String], rows: &[Vec<String>]) -> String {
    let width = |col: usize| -> f64 {
        let longest = std::iter::once(columns.get(col).map_or(0, |c| c.chars().count()))
            .chain(
                rows.iter()
                    .map(|r| r.get(col).map_or(0, |c| c.chars().count())),
            )
            .max()
            .unwrap_or(0);
        (longest as f64 + 2.0).clamp(6.0, 60.0)
    };
    let ncols = columns
        .len()
        .max(rows.iter().map(Vec::len).max().unwrap_or(0));
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\n",
    );
    xml.push_str(
        "<sheetViews><sheetView workbookViewId=\"0\"><pane ySplit=\"1\" topLeftCell=\"A2\" \
         activePane=\"bottomLeft\" state=\"frozen\"/></sheetView></sheetViews>\n",
    );
    if ncols > 0 {
        xml.push_str("<cols>");
        for c in 0..ncols {
            xml.push_str(&format!(
                "<col min=\"{0}\" max=\"{0}\" width=\"{1}\" customWidth=\"1\"/>",
                c + 1,
                width(c)
            ));
        }
        xml.push_str("</cols>\n");
    }
    xml.push_str("<sheetData>\n");
    xml.push_str("<row r=\"1\">");
    for (c, h) in columns.iter().enumerate() {
        push_cell(&mut xml, c, 0, &Cell::Text(h.clone()), STYLE_HEADER);
    }
    xml.push_str("</row>\n");
    for (r, row) in rows.iter().enumerate() {
        xml.push_str(&format!("<row r=\"{}\">", r + 2));
        for (c, text) in row.iter().enumerate() {
            push_cell(&mut xml, c, r + 1, &classify(text), 0);
        }
        xml.push_str("</row>\n");
    }
    xml.push_str("</sheetData>\n</worksheet>\n");
    xml
}

const STYLES_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\
<numFmts count=\"1\"><numFmt numFmtId=\"164\" formatCode=\"&quot;$&quot;#,##0.00\"/></numFmts>\
<fonts count=\"2\"><font><sz val=\"11\"/><name val=\"Calibri\"/></font>\
<font><b/><sz val=\"11\"/><name val=\"Calibri\"/></font></fonts>\
<fills count=\"2\"><fill><patternFill patternType=\"none\"/></fill>\
<fill><patternFill patternType=\"gray125\"/></fill></fills>\
<borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders>\
<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>\
<cellXfs count=\"3\">\
<xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/>\
<xf numFmtId=\"0\" fontId=\"1\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyFont=\"1\"/>\
<xf numFmtId=\"164\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyNumberFormat=\"1\"/>\
</cellXfs>\
<cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles>\
</styleSheet>\n";

const CONTENT_TYPES: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>\
<Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>\
<Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>\
<Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>\
</Types>\n";

const ROOT_RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/>\
</Relationships>\n";

const WORKBOOK_RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/>\
<Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>\
</Relationships>\n";

fn workbook_xml(sheet: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" \
         xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\
         <sheets><sheet name=\"{}\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>\n",
        xml_escape(sheet)
    )
}

/// The bytes of an `.xlsx` workbook holding one sheet named after `title`
/// with `columns` as its header row and `rows` below it.
pub fn to_xlsx(title: &str, columns: &[String], rows: &[Vec<String>]) -> Vec<u8> {
    let parts: Vec<(String, Vec<u8>)> = vec![
        (
            "[Content_Types].xml".into(),
            CONTENT_TYPES.as_bytes().to_vec(),
        ),
        ("_rels/.rels".into(), ROOT_RELS.as_bytes().to_vec()),
        (
            "xl/workbook.xml".into(),
            workbook_xml(&sheet_name(title)).into_bytes(),
        ),
        (
            "xl/_rels/workbook.xml.rels".into(),
            WORKBOOK_RELS.as_bytes().to_vec(),
        ),
        ("xl/styles.xml".into(), STYLES_XML.as_bytes().to_vec()),
        (
            "xl/worksheets/sheet1.xml".into(),
            sheet_xml(columns, rows).into_bytes(),
        ),
    ];
    plan_library::archive::write_zip(&parts)
}

impl Schedule {
    /// The schedule as an Excel workbook (one sheet named after its title):
    /// the same cells as [`Schedule::to_csv`], with numbers and dollar
    /// amounts stored as numbers.
    pub fn to_xlsx(&self) -> Vec<u8> {
        to_xlsx(&self.title, &self.columns, &self.rows)
    }
}

/// The Materials List as an Excel workbook, with the [`crate::MATERIAL_COLUMNS`]
/// header and a total row when anything is priced.
pub fn materials_to_xlsx(lines: &[crate::MaterialLine]) -> Vec<u8> {
    crate::materials::to_schedule(lines, "Materials List").to_xlsx()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(zip: &[u8], name: &str) -> String {
        let entries = plan_library::archive::read_zip(zip).expect("a zip");
        let (_, bytes) = entries
            .into_iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("no {name}"));
        String::from_utf8(bytes).unwrap()
    }

    fn table() -> Schedule {
        Schedule {
            title: "Door Schedule".into(),
            columns: vec!["Mark".into(), "Size".into(), "Qty".into(), "Price".into()],
            rows: vec![
                vec![
                    "D01".into(),
                    "3'-0\" x 6'-8\"".into(),
                    "2".into(),
                    "$1,234.50".into(),
                ],
                vec!["D02".into(), "R&D <odd>".into(), "007".into(), "".into()],
            ],
        }
    }

    #[test]
    fn a_workbook_is_a_zip_of_the_spreadsheetml_parts() {
        let zip = table().to_xlsx();
        let names: Vec<String> = plan_library::archive::read_zip(&zip)
            .unwrap()
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert_eq!(names[0], "[Content_Types].xml");
        for want in [
            "_rels/.rels",
            "xl/workbook.xml",
            "xl/_rels/workbook.xml.rels",
            "xl/styles.xml",
            "xl/worksheets/sheet1.xml",
        ] {
            assert!(names.iter().any(|n| n == want), "{want} in {names:?}");
        }
        let wb = part(&zip, "xl/workbook.xml");
        assert!(wb.contains("name=\"Door Schedule\""), "{wb}");
        assert!(part(&zip, "[Content_Types].xml").contains("sheet1.xml"));
    }

    #[test]
    fn cells_are_typed_escaped_and_addressed() {
        let sheet = part(&table().to_xlsx(), "xl/worksheets/sheet1.xml");
        // Header row: bold inline strings.
        assert!(
            sheet.contains(
                "<c r=\"A1\" s=\"1\" t=\"inlineStr\"><is><t xml:space=\"preserve\">Mark</t>"
            ),
            "{sheet}"
        );
        // Text with quotes stays text; XML characters are escaped.
        assert!(sheet.contains("3'-0&quot; x 6'-8&quot;"), "{sheet}");
        assert!(sheet.contains("R&amp;D &lt;odd&gt;"), "{sheet}");
        // Plain numbers are numbers, leading zeros stay text, dollars are
        // numbers in the money style, empty cells are left out.
        assert!(sheet.contains("<c r=\"C2\"><v>2</v></c>"), "{sheet}");
        assert!(sheet.contains(">007</t>"), "{sheet}");
        assert!(
            sheet.contains("<c r=\"D2\" s=\"2\"><v>1234.5</v></c>"),
            "{sheet}"
        );
        assert!(!sheet.contains("r=\"D3\""), "{sheet}");
        // The header row is frozen and the columns are sized.
        assert!(sheet.contains("state=\"frozen\""));
        assert!(sheet.contains("<col min=\"2\" max=\"2\""));
    }

    #[test]
    fn references_and_names_follow_excel_rules() {
        assert_eq!(cell_ref(0, 0), "A1");
        assert_eq!(cell_ref(25, 2), "Z3");
        assert_eq!(cell_ref(26, 9), "AA10");
        assert_eq!(cell_ref(701, 0), "ZZ1");
        assert_eq!(cell_ref(702, 0), "AAA1");
        assert_eq!(sheet_name("Door/Window: [All]"), "Door Window All");
        assert_eq!(sheet_name("   "), "Sheet1");
        assert_eq!(sheet_name(&"x".repeat(50)).chars().count(), 31);
        assert_eq!(sheet_name("'quoted'"), "quoted");
        assert!(matches!(classify("12.50"), Cell::Number(v) if (v - 12.5).abs() < 1e-12));
        assert!(matches!(classify("-3"), Cell::Number(_)));
        assert!(matches!(classify("1e5"), Cell::Text(_)));
        assert!(matches!(classify("0.5"), Cell::Number(_)));
        assert!(matches!(classify("01"), Cell::Text(_)));
        assert!(matches!(classify("1."), Cell::Text(_)));
        assert!(matches!(classify("$5"), Cell::Money(_)));
        assert!(matches!(classify("$x"), Cell::Text(_)));
    }

    #[test]
    fn an_empty_table_is_still_a_valid_workbook() {
        let s = Schedule::default();
        let zip = s.to_xlsx();
        let sheet = part(&zip, "xl/worksheets/sheet1.xml");
        assert!(sheet.contains("<sheetData>"));
        assert!(!sheet.contains("<cols>"));
        assert!(part(&zip, "xl/workbook.xml").contains("name=\"Sheet1\""));
    }

    #[test]
    fn the_materials_list_exports_with_its_header_and_total() {
        use crate::materials::MaterialLine;
        let line = |item: &str, qty: f64, price: Option<f64>| MaterialLine {
            category: "Framing".into(),
            id: "F1".into(),
            item: item.into(),
            size: "2x4".into(),
            quantity: qty,
            unit: "ea".into(),
            net: qty,
            waste_pct: 0.0,
            unit_price: price.map(|p| p / qty),
            price,
            key: String::new(),
        };
        let zip = materials_to_xlsx(&[line("Stud", 10.0, Some(25.0)), line("Plate", 4.0, None)]);
        let sheet = part(&zip, "xl/worksheets/sheet1.xml");
        assert!(sheet.contains(">Description</t>"), "{sheet}");
        assert!(sheet.contains(">Stud</t>"), "{sheet}");
        assert!(sheet.contains("<v>25</v>"), "{sheet}");
        assert!(sheet.contains(">Total</t>"), "{sheet}");
        assert!(part(&zip, "xl/workbook.xml").contains("Materials List"));
    }
}
