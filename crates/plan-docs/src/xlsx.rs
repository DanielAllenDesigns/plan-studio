//! A minimal Excel workbook (`.xlsx`) writer for schedules and the Materials
//! List: one worksheet, a bold frozen header row, text cells as inline
//! strings, numbers and dollar amounts as numbers. A workbook is a zip of a
//! few XML parts; the zip is written stored (no compression) by
//! `plan_library::archive::write_zip`.

use crate::schedule::Schedule;

/// Longest sheet name Excel accepts.
const MAX_SHEET_NAME: usize = 31;

/// Style ids in `xl/styles.xml`: 0 plain, 1 bold header, 2 dollars, 3 editable
/// (unlocked), 4 computed (shaded, locked).
const STYLE_HEADER: u8 = 1;
const STYLE_MONEY: u8 = 2;
/// An editable cell: light yellow, unlocked under sheet protection.
const STYLE_EDIT: u8 = 3;
/// A computed cell: gray, locked under sheet protection.
const STYLE_LOCKED: u8 = 4;

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
<fills count=\"4\"><fill><patternFill patternType=\"none\"/></fill>\
<fill><patternFill patternType=\"gray125\"/></fill>\
<fill><patternFill patternType=\"solid\"><fgColor rgb=\"FFFFF8DC\"/><bgColor indexed=\"64\"/></patternFill></fill>\
<fill><patternFill patternType=\"solid\"><fgColor rgb=\"FFE4E4E4\"/><bgColor indexed=\"64\"/></patternFill></fill></fills>\
<borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders>\
<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>\
<cellXfs count=\"5\">\
<xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/>\
<xf numFmtId=\"0\" fontId=\"1\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyFont=\"1\"/>\
<xf numFmtId=\"164\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyNumberFormat=\"1\"/>\
<xf numFmtId=\"0\" fontId=\"0\" fillId=\"2\" borderId=\"0\" xfId=\"0\" applyFill=\"1\" applyProtection=\"1\"><protection locked=\"0\"/></xf>\
<xf numFmtId=\"0\" fontId=\"0\" fillId=\"3\" borderId=\"0\" xfId=\"0\" applyFill=\"1\"/>\
</cellXfs>\
<cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles>\
</styleSheet>\n";

fn content_types(sheets: usize) -> String {
    let mut x = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>\
<Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>",
    );
    for i in 1..=sheets {
        x.push_str(&format!(
            "<Override PartName=\"/xl/worksheets/sheet{i}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"
        ));
    }
    x.push_str(
        "<Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>\
</Types>\n",
    );
    x
}

const ROOT_RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/>\
</Relationships>\n";

fn workbook_rels(sheets: usize) -> String {
    let mut x = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
    );
    for i in 1..=sheets {
        x.push_str(&format!(
            "<Relationship Id=\"rId{i}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet{i}.xml\"/>"
        ));
    }
    x.push_str(&format!(
        "<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>\
</Relationships>\n",
        sheets + 1
    ));
    x
}

fn workbook_xml(names: &[String]) -> String {
    workbook_xml_states(names, &[])
}

/// [`workbook_xml`] with the tabs flagged in `hidden` hidden.
fn workbook_xml_states(names: &[String], hidden: &[bool]) -> String {
    let mut x = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" \
         xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets>",
    );
    for (i, n) in names.iter().enumerate() {
        let state = if hidden.get(i).copied().unwrap_or(false) {
            " state=\"hidden\""
        } else {
            ""
        };
        x.push_str(&format!(
            "<sheet name=\"{}\" sheetId=\"{}\"{state} r:id=\"rId{}\"/>",
            xml_escape(n),
            i + 1,
            i + 1
        ));
    }
    x.push_str("</sheets></workbook>\n");
    x
}

/// One worksheet of a multi-sheet workbook: a title (the tab name), the
/// header row and the data rows.
pub struct XlsxSheet<'a> {
    pub title: &'a str,
    pub columns: &'a [String],
    pub rows: &'a [Vec<String>],
}

/// Worksheet names Excel accepts, made unique (`Door Schedule`, `Door
/// Schedule 2`, ...), compared without regard to case.
pub(crate) fn unique_sheet_names(titles: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in titles {
        let base = sheet_name(t);
        let mut name = base.clone();
        let mut n = 2;
        while out.iter().any(|o| o.eq_ignore_ascii_case(&name)) {
            let suffix = format!(" {n}");
            let keep = MAX_SHEET_NAME.saturating_sub(suffix.chars().count());
            name = format!("{}{suffix}", base.chars().take(keep).collect::<String>());
            n += 1;
        }
        out.push(name);
    }
    out
}

/// The bytes of an `.xlsx` workbook with one worksheet per entry of `sheets`
/// (at least one: an empty list gives an empty "Sheet1"). Tab names are
/// shortened to Excel's rules and made unique.
pub fn to_xlsx_sheets(sheets: &[XlsxSheet]) -> Vec<u8> {
    let empty = XlsxSheet {
        title: "Sheet1",
        columns: &[],
        rows: &[],
    };
    let sheets: Vec<&XlsxSheet> = if sheets.is_empty() {
        vec![&empty]
    } else {
        sheets.iter().collect()
    };
    let names = unique_sheet_names(&sheets.iter().map(|s| s.title).collect::<Vec<_>>());
    let mut parts: Vec<(String, Vec<u8>)> = vec![
        (
            "[Content_Types].xml".into(),
            content_types(sheets.len()).into_bytes(),
        ),
        ("_rels/.rels".into(), ROOT_RELS.as_bytes().to_vec()),
        ("xl/workbook.xml".into(), workbook_xml(&names).into_bytes()),
        (
            "xl/_rels/workbook.xml.rels".into(),
            workbook_rels(sheets.len()).into_bytes(),
        ),
        ("xl/styles.xml".into(), STYLES_XML.as_bytes().to_vec()),
    ];
    for (i, s) in sheets.iter().enumerate() {
        parts.push((
            format!("xl/worksheets/sheet{}.xml", i + 1),
            sheet_xml(s.columns, s.rows).into_bytes(),
        ));
    }
    plan_library::archive::write_zip(&parts)
}

/// The bytes of an `.xlsx` workbook holding one sheet named after `title`
/// with `columns` as its header row and `rows` below it.
pub fn to_xlsx(title: &str, columns: &[String], rows: &[Vec<String>]) -> Vec<u8> {
    to_xlsx_sheets(&[XlsxSheet {
        title,
        columns,
        rows,
    }])
}

/// Several schedules as one workbook, a worksheet each.
pub fn schedules_to_xlsx(schedules: &[Schedule]) -> Vec<u8> {
    let sheets: Vec<XlsxSheet> = schedules
        .iter()
        .map(|s| XlsxSheet {
            title: &s.title,
            columns: &s.columns,
            rows: &s.rows,
        })
        .collect();
    to_xlsx_sheets(&sheets)
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

// ===================================================================
// Workbooks made for editing
// ===================================================================

/// What the cells of an editable-workbook column hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnData {
    /// Always text, so `007` or `1.50` come back exactly as they went out.
    #[default]
    Text,
    /// Numbers (a cell that is not a plain number stays text).
    Number,
}

/// One column of a [`EditSheet`].
#[derive(Debug, Clone, Default)]
pub struct EditColumn {
    pub header: String,
    /// Hidden in Excel (the object id column); the data is still there.
    pub hidden: bool,
    /// Unlocked and tinted: the cells the user is meant to change. The rest
    /// are shaded and locked by the sheet protection.
    pub editable: bool,
    pub data: ColumnData,
    /// A drop-down list of choices on the data cells (list properties and
    /// Yes / No). Left out when a choice has a comma or quote, or the list
    /// is longer than Excel's 255-character literal.
    pub choices: Vec<String>,
}

/// One worksheet of a workbook made to be edited and imported back.
#[derive(Debug, Clone, Default)]
pub struct EditSheet {
    pub title: String,
    /// A hidden tab (`_meta`).
    pub hidden: bool,
    /// Protect the sheet (no password) so only the unlocked cells take edits.
    pub protect: bool,
    pub columns: Vec<EditColumn>,
    pub rows: Vec<Vec<String>>,
}

/// The `<dataValidation>` literal list for `choices`, when Excel can carry it.
fn list_formula(choices: &[String]) -> Option<String> {
    if choices.is_empty()
        || choices.iter().any(|c| c.contains([',', '"']))
        || choices.iter().map(|c| c.chars().count() + 1).sum::<usize>() > 255
    {
        return None;
    }
    Some(format!("\"{}\"", xml_escape(&choices.join(","))))
}

fn push_edit_cell(xml: &mut String, col: usize, row: usize, text: &str, c: &EditColumn) {
    let style = if c.editable { STYLE_EDIT } else { STYLE_LOCKED };
    let r = cell_ref(col, row);
    if text.is_empty() {
        xml.push_str(&format!("<c r=\"{r}\" s=\"{style}\"/>"));
        return;
    }
    if c.data == ColumnData::Number || !c.editable {
        if let Cell::Number(v) = classify(text) {
            xml.push_str(&format!("<c r=\"{r}\" s=\"{style}\"><v>{v}</v></c>"));
            return;
        }
    }
    xml.push_str(&format!(
        "<c r=\"{r}\" s=\"{style}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
        xml_escape(text)
    ));
}

fn edit_sheet_xml(sheet: &EditSheet) -> String {
    let ncols = sheet
        .columns
        .len()
        .max(sheet.rows.iter().map(Vec::len).max().unwrap_or(0));
    let width = |col: usize| -> f64 {
        let longest = std::iter::once(
            sheet
                .columns
                .get(col)
                .map_or(0, |c| c.header.chars().count()),
        )
        .chain(
            sheet
                .rows
                .iter()
                .map(|r| r.get(col).map_or(0, |c| c.chars().count())),
        )
        .max()
        .unwrap_or(0);
        (longest as f64 + 2.0).clamp(8.0, 60.0)
    };
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
            let hidden = sheet.columns.get(c).is_some_and(|c| c.hidden);
            xml.push_str(&format!(
                "<col min=\"{0}\" max=\"{0}\" width=\"{1}\" customWidth=\"1\"{2}/>",
                c + 1,
                width(c),
                if hidden { " hidden=\"1\"" } else { "" }
            ));
        }
        xml.push_str("</cols>\n");
    }
    xml.push_str("<sheetData>\n<row r=\"1\">");
    for (c, col) in sheet.columns.iter().enumerate() {
        push_cell(
            &mut xml,
            c,
            0,
            &Cell::Text(col.header.clone()),
            STYLE_HEADER,
        );
    }
    xml.push_str("</row>\n");
    for (r, row) in sheet.rows.iter().enumerate() {
        xml.push_str(&format!("<row r=\"{}\">", r + 2));
        for (c, col) in sheet.columns.iter().enumerate() {
            push_edit_cell(
                &mut xml,
                c,
                r + 1,
                row.get(c).map_or("", String::as_str),
                col,
            );
        }
        xml.push_str("</row>\n");
    }
    xml.push_str("</sheetData>\n");
    if sheet.protect {
        // No password: Review > Unprotect Sheet opens it up again.
        xml.push_str(
            "<sheetProtection sheet=\"1\" objects=\"1\" scenarios=\"1\" formatColumns=\"0\" \
             formatRows=\"0\" sort=\"0\" autoFilter=\"0\"/>\n",
        );
    }
    // Drop-down lists over the data rows.
    let last = sheet.rows.len() + 1;
    let validations: Vec<String> = sheet
        .columns
        .iter()
        .enumerate()
        .filter(|(_, c)| c.editable)
        .filter_map(|(i, c)| {
            let f = list_formula(&c.choices)?;
            Some(format!(
                "<dataValidation type=\"list\" allowBlank=\"1\" showErrorMessage=\"1\" \
                 errorTitle=\"Not a choice\" error=\"Pick a value from the list.\" \
                 sqref=\"{a}:{b}\"><formula1>{f}</formula1></dataValidation>",
                a = cell_ref(i, 1),
                b = cell_ref(i, last.max(2) - 1),
            ))
        })
        .collect();
    if !validations.is_empty() && !sheet.rows.is_empty() {
        xml.push_str(&format!(
            "<dataValidations count=\"{}\">{}</dataValidations>\n",
            validations.len(),
            validations.concat()
        ));
    }
    xml.push_str("</worksheet>\n");
    xml
}

/// The bytes of a workbook with one [`EditSheet`] per entry: hidden columns,
/// unlocked editable cells, shaded locked ones, optional sheet protection
/// and drop-down lists. Tab names follow Excel's rules and are made unique.
pub fn to_xlsx_edit(sheets: &[EditSheet]) -> Vec<u8> {
    let empty = EditSheet {
        title: "Sheet1".into(),
        ..EditSheet::default()
    };
    let sheets: Vec<&EditSheet> = if sheets.is_empty() {
        vec![&empty]
    } else {
        sheets.iter().collect()
    };
    let names = unique_sheet_names(&sheets.iter().map(|s| s.title.as_str()).collect::<Vec<_>>());
    let hidden: Vec<bool> = sheets.iter().map(|s| s.hidden).collect();
    let mut parts: Vec<(String, Vec<u8>)> = vec![
        (
            "[Content_Types].xml".into(),
            content_types(sheets.len()).into_bytes(),
        ),
        ("_rels/.rels".into(), ROOT_RELS.as_bytes().to_vec()),
        (
            "xl/workbook.xml".into(),
            workbook_xml_states(&names, &hidden).into_bytes(),
        ),
        (
            "xl/_rels/workbook.xml.rels".into(),
            workbook_rels(sheets.len()).into_bytes(),
        ),
        ("xl/styles.xml".into(), STYLES_XML.as_bytes().to_vec()),
    ];
    for (i, s) in sheets.iter().enumerate() {
        parts.push((
            format!("xl/worksheets/sheet{}.xml", i + 1),
            edit_sheet_xml(s).into_bytes(),
        ));
    }
    plan_library::archive::write_zip(&parts)
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

    #[test]
    fn several_schedules_make_one_workbook_with_unique_tab_names() {
        let a = table();
        let mut b = table();
        b.title = "door schedule".into();
        let mut c = table();
        c.title = "Windows".into();
        let zip = schedules_to_xlsx(&[a, b, c]);
        let wb = part(&zip, "xl/workbook.xml");
        assert!(wb.contains("name=\"Door Schedule\""), "{wb}");
        assert!(wb.contains("name=\"door schedule 2\""), "{wb}");
        assert!(wb.contains("name=\"Windows\""), "{wb}");
        for i in 1..=3 {
            let sheet = part(&zip, &format!("xl/worksheets/sheet{i}.xml"));
            assert!(sheet.contains("Mark"), "sheet {i}");
        }
        let ct = part(&zip, "[Content_Types].xml");
        assert!(ct.contains("sheet3.xml") && !ct.contains("sheet4.xml"));
        let rels = part(&zip, "xl/_rels/workbook.xml.rels");
        assert!(
            rels.contains("rId4") && rels.contains("styles.xml"),
            "{rels}"
        );
        // No schedules still gives a readable workbook.
        let empty = schedules_to_xlsx(&[]);
        assert!(part(&empty, "xl/workbook.xml").contains("Sheet1"));
    }

    #[test]
    fn an_edit_workbook_hides_locks_tints_and_validates() {
        let col = |h: &str, editable: bool| EditColumn {
            header: h.into(),
            editable,
            ..EditColumn::default()
        };
        let mut id = col("PlanStudio ID", false);
        id.hidden = true;
        let mut pick = col("Rating", true);
        pick.choices = vec!["A".into(), "B & C".into()];
        let mut commas = col("Tag", true);
        commas.choices = vec!["x,y".into(), "z".into()];
        let mut qty = col("Qty", true);
        qty.data = ColumnData::Number;
        let sheet = EditSheet {
            title: "Doors".into(),
            protect: true,
            columns: vec![id, pick, commas, qty, col("Size", false)],
            rows: vec![
                vec![
                    "door:1".into(),
                    "A".into(),
                    "".into(),
                    "2".into(),
                    "36".into(),
                ],
                vec![
                    "door:2".into(),
                    "".into(),
                    "x".into(),
                    "007".into(),
                    "".into(),
                ],
            ],
            ..EditSheet::default()
        };
        let meta = EditSheet {
            title: "doors".into(),
            hidden: true,
            ..EditSheet::default()
        };
        let zip = to_xlsx_edit(&[sheet, meta]);
        let x = part(&zip, "xl/worksheets/sheet1.xml");
        // The id column is hidden; the others are not.
        assert!(
            x.contains("<col min=\"1\" max=\"1\" width=\"15\" customWidth=\"1\" hidden=\"1\"/>"),
            "{x}"
        );
        assert!(!x.contains("<col min=\"2\" max=\"2\" width=\"8\" customWidth=\"1\" hidden"));
        // Editable cells are style 3 (unlocked), the rest style 4; empty
        // cells keep their style so the whole column is tinted.
        assert!(x.contains("<c r=\"B2\" s=\"3\" t=\"inlineStr\">"), "{x}");
        assert!(x.contains("<c r=\"C2\" s=\"3\"/>"), "{x}");
        assert!(x.contains("<c r=\"A2\" s=\"4\" t=\"inlineStr\">"), "{x}");
        // Numbers: a Number column stores numbers; a locked plain number too;
        // leading zeros stay text.
        assert!(x.contains("<c r=\"D2\" s=\"3\"><v>2</v></c>"), "{x}");
        assert!(x.contains("<c r=\"E2\" s=\"4\"><v>36</v></c>"), "{x}");
        assert!(x.contains("<c r=\"D3\" s=\"3\" t=\"inlineStr\">"), "{x}");
        // Protection follows the data, then the one usable drop-down.
        let prot = x.find("<sheetProtection").unwrap();
        assert!(x.find("</sheetData>").unwrap() < prot);
        let dv = x.find("<dataValidations count=\"1\">").unwrap();
        assert!(prot < dv);
        assert!(
            x.contains("sqref=\"B2:B3\"><formula1>\"A,B &amp; C\"</formula1>"),
            "{x}"
        );
        assert!(
            !x.contains("sqref=\"C2"),
            "a choice with a comma gets no drop-down"
        );
        // The hidden tab, and a name that clashes with another is made unique.
        let wb = part(&zip, "xl/workbook.xml");
        assert!(
            wb.contains("name=\"doors 2\"") && wb.contains("state=\"hidden\""),
            "{wb}"
        );
        assert!(!wb.contains("name=\"Doors\" sheetId=\"1\" state"));
        let styles = part(&zip, "xl/styles.xml");
        assert!(styles.contains("cellXfs count=\"5\"") && styles.contains("locked=\"0\""));
        // The old writers are untouched by the new styles.
        assert!(part(&table().to_xlsx(), "xl/styles.xml").contains("cellXfs count=\"5\""));
        // No sheets: still a workbook.
        assert!(part(&to_xlsx_edit(&[]), "xl/workbook.xml").contains("Sheet1"));
    }
}
