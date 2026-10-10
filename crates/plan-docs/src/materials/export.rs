//! Exporting and printing a Materials List (manual pp. 1385-1387): Tab
//! Delimited (TXT), Comma Delimited (CSV), Spreadsheet (XML for Excel) and
//! Web Page (HTML), plus Excel's own `.xlsx`, BuilderTREND's CSV and a table
//! PDF for File > Print.

use super::list::{self, cell_units, ListLine, Row, UnitsMode};
use crate::pdf::PdfDoc;
use crate::schedule::{push_csv_row, Schedule};
use plan_core::materials_data::{ListSpec, MlColumn};

/// The file types of the Export Materials List dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportFormat {
    /// Tab Delimited (TXT).
    Txt,
    /// Comma Delimited (CSV).
    #[default]
    Csv,
    /// Spreadsheet (XML) that Excel opens.
    Xml,
    /// Web Page (HTML).
    Html,
    /// An Excel workbook (.xlsx).
    Xlsx,
}

impl ExportFormat {
    pub const ALL: [ExportFormat; 5] = [
        ExportFormat::Txt,
        ExportFormat::Csv,
        ExportFormat::Xml,
        ExportFormat::Html,
        ExportFormat::Xlsx,
    ];

    pub fn title(self) -> &'static str {
        match self {
            ExportFormat::Txt => "Tab Delimited (TXT)",
            ExportFormat::Csv => "Comma Delimited (CSV)",
            ExportFormat::Xml => "Spreadsheet (XML)",
            ExportFormat::Html => "Web Page (HTML)",
            ExportFormat::Xlsx => "Excel Workbook (XLSX)",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Txt => "txt",
            ExportFormat::Csv => "csv",
            ExportFormat::Xml => "xml",
            ExportFormat::Html => "html",
            ExportFormat::Xlsx => "xlsx",
        }
    }
}

/// The Third Party Formats of the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThirdParty {
    /// The formatting of the Chief Materials List.
    #[default]
    NoFormatting,
    /// BuilderTREND (CSV only).
    BuilderTrend,
}

/// The Export Materials List dialog's choices.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExportOptions {
    pub format: ExportFormat,
    /// Include Column Headers (HTML always has them).
    pub headers: bool,
    /// Include Hidden Columns: every column, shown or not.
    pub hidden_columns: bool,
    /// Export with Colors (XML and HTML).
    pub colors: bool,
    pub units: UnitsMode,
    pub third_party: ThirdParty,
    /// Open in Default Spreadsheet Editor when the file is written.
    pub open_in_editor: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: ExportFormat::Csv,
            headers: true,
            hidden_columns: false,
            colors: false,
            units: UnitsMode::WithAmounts,
            third_party: ThirdParty::NoFormatting,
            open_in_editor: false,
        }
    }
}

/// The table an export writes: a header and the rows (cells as text).
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub header: Vec<String>,
    pub rows: Vec<TableRow>,
}

/// A row of an export table.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
    pub cells: Vec<String>,
    pub kind: RowKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Group,
    Line,
    Subtotal,
    Total,
}

/// The columns an export or a printout has: the visible ones in order, or all
/// the Materials List columns when hidden ones are asked for.
pub fn export_columns(spec: &ListSpec, hidden: bool) -> Vec<MlColumn> {
    spec.columns
        .iter()
        .filter(|c| c.col.in_materials_list() && (hidden || c.visible))
        .map(|c| c.col)
        .collect()
}

/// The table of a list: its columns, units written as asked, the lines in the
/// report order. `with_groups` adds the group headings and subtotals (a
/// printout); exports carry the lines and the total only.
pub fn table(
    lines: &[ListLine],
    spec: &ListSpec,
    opts: &ExportOptions,
    with_groups: bool,
) -> Table {
    let cols = export_columns(spec, opts.hidden_columns);
    let mut header: Vec<String> = Vec::new();
    for c in &cols {
        header.push(c.title().to_string());
        if *c == MlColumn::Count && opts.units == UnitsMode::NewColumn {
            header.push("Unit".into());
        }
    }
    let width = header.len();
    let blank = |label: String, at: Option<(MlColumn, String)>| {
        let mut cells = vec![String::new(); width];
        if let Some(first) = cells.first_mut() {
            *first = label;
        }
        if let Some((col, text)) = at {
            let mut pos = 0;
            for c in &cols {
                if *c == col {
                    cells[pos] = text.clone();
                }
                pos += 1;
                if *c == MlColumn::Count && opts.units == UnitsMode::NewColumn {
                    pos += 1;
                }
            }
        }
        cells
    };
    let money = |v: Option<f64>| super::fmt_money(v);
    let mut rows = Vec::new();
    for r in list::arrange(lines, spec, true) {
        match r {
            Row::Group(g) if with_groups => rows.push(TableRow {
                cells: blank(g, None),
                kind: RowKind::Group,
            }),
            Row::Group(_) => {}
            Row::Line(i) => {
                let l = &lines[i];
                let mut cells = Vec::new();
                for c in &cols {
                    cells.push(cell_units(l, *c, opts.units));
                    if *c == MlColumn::Count && opts.units == UnitsMode::NewColumn {
                        cells.push(l.line.unit.clone());
                    }
                }
                rows.push(TableRow {
                    cells,
                    kind: RowKind::Line,
                });
            }
            Row::Subtotal(g, t) if with_groups => rows.push(TableRow {
                cells: blank(
                    format!("Subtotal {g}"),
                    Some((MlColumn::TotalCost, money(t))),
                ),
                kind: RowKind::Subtotal,
            }),
            Row::Subtotal(..) => {}
            Row::GrandTotal(t) => rows.push(TableRow {
                cells: blank("Total".into(), Some((MlColumn::TotalCost, money(t)))),
                kind: RowKind::Total,
            }),
        }
    }
    Table { header, rows }
}

/// A cell safe for a tab-delimited file.
fn tab_safe(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

/// Tab-delimited text.
pub fn to_txt(t: &Table, headers: bool) -> String {
    let mut out = String::new();
    if headers {
        out.push_str(
            &t.header
                .iter()
                .map(|c| tab_safe(c))
                .collect::<Vec<_>>()
                .join("\t"),
        );
        out.push('\n');
    }
    for r in &t.rows {
        out.push_str(
            &r.cells
                .iter()
                .map(|c| tab_safe(c))
                .collect::<Vec<_>>()
                .join("\t"),
        );
        out.push('\n');
    }
    out
}

/// Comma-delimited text.
pub fn to_csv_text(t: &Table, headers: bool) -> String {
    let mut out = String::new();
    if headers {
        push_csv_row(&mut out, &t.header);
    }
    for r in &t.rows {
        push_csv_row(&mut out, &r.cells);
    }
    out
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn colors_of(spec: &ListSpec, with_colors: bool) -> ([u8; 3], [u8; 3], [u8; 3], [u8; 3]) {
    // (header fill, row fill, alternate row fill, text)
    if with_colors && spec.appearance.custom_colors {
        let a = &spec.appearance;
        (a.grid, a.background, a.background, a.text)
    } else if with_colors {
        (
            [0xE6, 0xE1, 0xD6],
            [0xFF, 0xFF, 0xFF],
            [0xF6, 0xF3, 0xEC],
            [0, 0, 0],
        )
    } else {
        (
            [0xF2, 0xF2, 0xF2],
            [0xFF, 0xFF, 0xFF],
            [0xF2, 0xF2, 0xF2],
            [0, 0, 0],
        )
    }
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

fn is_number(s: &str) -> bool {
    !s.is_empty() && s.replace(['$', ','], "").parse::<f64>().is_ok()
}

/// Spreadsheet XML (Excel 2003), numbers and currency as numbers.
pub fn to_xml(t: &Table, spec: &ListSpec, opts: &ExportOptions) -> String {
    let (head, _, _, text) = colors_of(spec, opts.colors);
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\"?>\n<?mso-application progid=\"Excel.Sheet\"?>\n");
    out.push_str("<Workbook xmlns=\"urn:schemas-microsoft-com:office:spreadsheet\" xmlns:ss=\"urn:schemas-microsoft-com:office:spreadsheet\">\n");
    out.push_str(&format!(
        "<Styles>\n<Style ss:ID=\"head\"><Font ss:Bold=\"1\" ss:Color=\"{}\"/><Interior ss:Color=\"{}\" ss:Pattern=\"Solid\"/></Style>\n<Style ss:ID=\"money\"><NumberFormat ss:Format=\"Currency\"/></Style>\n</Styles>\n",
        hex(text),
        hex(head)
    ));
    out.push_str("<Worksheet ss:Name=\"Materials List\">\n<Table>\n");
    if opts.headers {
        out.push_str("<Row>");
        for h in &t.header {
            out.push_str(&format!(
                "<Cell ss:StyleID=\"head\"><Data ss:Type=\"String\">{}</Data></Cell>",
                xml_escape(h)
            ));
        }
        out.push_str("</Row>\n");
    }
    for r in &t.rows {
        out.push_str("<Row>");
        for c in &r.cells {
            if is_number(c) {
                let money = c.starts_with('$');
                let v = c.replace(['$', ','], "");
                out.push_str(&format!(
                    "<Cell{}><Data ss:Type=\"Number\">{}</Data></Cell>",
                    if money { " ss:StyleID=\"money\"" } else { "" },
                    v
                ));
            } else {
                out.push_str(&format!(
                    "<Cell><Data ss:Type=\"String\">{}</Data></Cell>",
                    xml_escape(c)
                ));
            }
        }
        out.push_str("</Row>\n");
    }
    out.push_str("</Table>\n</Worksheet>\n</Workbook>\n");
    out
}

/// A web page with the list as a table. Column headers are always included.
pub fn to_html(t: &Table, spec: &ListSpec, opts: &ExportOptions, title: &str) -> String {
    let (head, odd, even, text) = colors_of(spec, opts.colors);
    let a = &spec.appearance;
    let border = if opts.colors && a.custom_colors {
        hex(a.grid)
    } else {
        "#C0C0C0".into()
    };
    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\">");
    out.push_str(&format!("<title>{}</title>\n<style>\n", xml_escape(title)));
    out.push_str(&format!(
        "body{{font-family:{},sans-serif;font-size:{}pt;color:{}}}\ntable{{border-collapse:collapse}}\nth,td{{padding:2px 8px;{}{}}}\nth{{background:{};text-align:left}}\ntr.odd td{{background:{}}}\ntr.even td{{background:{}}}\ntr.group td{{font-weight:bold}}\ntr.sub td,tr.total td{{font-weight:bold;border-top:2px solid {}}}\ntd.num{{text-align:right}}\n</style></head><body>\n",
        a.font,
        a.font_size,
        hex(text),
        if a.horizontal_lines { format!("border-top:1px {} {};border-bottom:1px {} {};", if a.solid_lines { "solid" } else { "dashed" }, border, if a.solid_lines { "solid" } else { "dashed" }, border) } else { String::new() },
        if a.vertical_lines { format!("border-left:1px {} {};border-right:1px {} {};", if a.solid_lines { "solid" } else { "dashed" }, border, if a.solid_lines { "solid" } else { "dashed" }, border) } else { String::new() },
        hex(head),
        hex(odd),
        hex(even),
        border
    ));
    out.push_str(&format!("<h1>{}</h1>\n<table>\n<tr>", xml_escape(title)));
    for h in &t.header {
        out.push_str(&format!("<th>{}</th>", xml_escape(h)));
    }
    out.push_str("</tr>\n");
    let mut n = 0;
    for r in &t.rows {
        let class = match r.kind {
            RowKind::Group => "group",
            RowKind::Subtotal => "sub",
            RowKind::Total => "total",
            RowKind::Line => {
                n += 1;
                if n % 2 == 1 {
                    "odd"
                } else {
                    "even"
                }
            }
        };
        out.push_str(&format!("<tr class=\"{class}\">"));
        for c in &r.cells {
            let num = if is_number(c) { " class=\"num\"" } else { "" };
            out.push_str(&format!("<td{num}>{}</td>", xml_escape(c)));
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</table>\n</body></html>\n");
    out
}

/// BuilderTREND's CSV: cost code, title, description, quantity, unit cost,
/// unit and markup. (The column set follows BuilderTREND's budget import;
/// verify it against a current import template.)
pub fn to_builder_trend(lines: &[ListLine], spec: &ListSpec) -> String {
    let mut out = String::new();
    push_csv_row(
        &mut out,
        &[
            "Cost Code".to_string(),
            "Title".into(),
            "Description".into(),
            "Quantity".into(),
            "Unit Cost".into(),
            "Unit".into(),
            "Markup %".into(),
        ],
    );
    for r in list::arrange(lines, spec, true) {
        if let Row::Line(i) = r {
            let l = &lines[i];
            let desc = [l.line.size.as_str(), l.comment.as_str()]
                .iter()
                .filter(|s| !s.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join("; ");
            push_csv_row(
                &mut out,
                &[
                    l.accounting_code.clone(),
                    l.line.item.clone(),
                    desc,
                    format!("{}", l.line.quantity + l.extra),
                    l.line
                        .unit_price
                        .map_or(String::new(), |p| format!("{p:.2}")),
                    l.line.unit.clone(),
                    if l.markup == 0.0 {
                        String::new()
                    } else {
                        format!("{}", l.markup)
                    },
                ],
            );
        }
    }
    out
}

/// The bytes of an export file (the text formats as UTF-8, the workbook as
/// its zip).
pub fn export(lines: &[ListLine], spec: &ListSpec, opts: &ExportOptions, title: &str) -> Vec<u8> {
    if opts.third_party == ThirdParty::BuilderTrend {
        return to_builder_trend(lines, spec).into_bytes();
    }
    let t = table(lines, spec, opts, matches!(opts.format, ExportFormat::Html));
    match opts.format {
        ExportFormat::Txt => to_txt(&t, opts.headers).into_bytes(),
        ExportFormat::Csv => to_csv_text(&t, opts.headers).into_bytes(),
        ExportFormat::Xml => to_xml(&t, spec, opts).into_bytes(),
        ExportFormat::Html => to_html(&t, spec, opts, title).into_bytes(),
        ExportFormat::Xlsx => {
            let columns = if opts.headers {
                t.header.clone()
            } else {
                Vec::new()
            };
            let rows: Vec<Vec<String>> = t.rows.iter().map(|r| r.cells.clone()).collect();
            crate::xlsx::to_xlsx("Materials List", &columns, &rows)
        }
    }
}

/// The list as a schedule-style table of its visible columns (a layout box, a
/// page of a construction set).
pub fn to_schedule(lines: &[ListLine], spec: &ListSpec, title: &str) -> Schedule {
    let t = table(lines, spec, &ExportOptions::default(), true);
    Schedule {
        title: title.to_string(),
        columns: t.header,
        rows: t.rows.into_iter().map(|r| r.cells).collect(),
    }
}

// -------------------------------------------------------------------- print --

/// File > Print: the list as a PDF table on letter pages (landscape when the
/// columns are wide), with the visible columns at their widths scaled to the
/// page, group headings, subtotals, the total and page numbers.
pub fn to_pdf(lines: &[ListLine], spec: &ListSpec, title: &str, subtitle: &str) -> Vec<u8> {
    let opts = ExportOptions::default();
    let t = table(lines, spec, &opts, true);
    let cols = export_columns(spec, false);
    // Column widths from the spec, with the Unit column none (units stay in
    // the Count cell).
    let widths: Vec<f64> = cols
        .iter()
        .map(|c| {
            spec.columns
                .iter()
                .find(|x| x.col == *c)
                .map_or(80.0, |x| f64::from(x.width))
        })
        .collect();
    let total_w: f64 = widths.iter().sum::<f64>().max(1.0);
    let landscape = total_w > 520.0;
    let (pw, ph) = if landscape {
        (792.0, 612.0)
    } else {
        (612.0, 792.0)
    };
    let margin = 36.0;
    let scale = (pw - 2.0 * margin) / total_w;
    let widths: Vec<f64> = widths.iter().map(|w| w * scale).collect();
    let size = f64::from(spec.appearance.font_size).clamp(6.0, 14.0) * 0.9;
    let row_h = size * 1.7;
    let top = ph - margin - 30.0;
    let bottom = margin + 14.0;
    let per_page = (((top - bottom) / row_h).floor() as usize)
        .saturating_sub(1)
        .max(1);

    let mut doc = PdfDoc::new(pw, ph);
    let pages = t.rows.len().div_ceil(per_page).max(1);
    let numeric: Vec<bool> = cols.iter().map(|c| c.numeric()).collect();
    for page in 0..pages {
        if page > 0 {
            doc.new_page();
        }
        doc.set_gray(0.0);
        doc.set_font_bold(true);
        doc.text(margin, ph - margin - 8.0, 14.0, title);
        doc.set_font_bold(false);
        doc.text(margin, ph - margin - 22.0, 8.0, subtitle);
        let mut y = top;
        // Header row.
        doc.fill_rect(
            margin,
            y - row_h + 3.0,
            pw - 2.0 * margin,
            row_h,
            crate::pdf::PdfColor::Gray(0.9),
        );
        doc.set_gray(0.0);
        doc.set_font_bold(true);
        draw_row(
            &mut doc,
            margin,
            y - row_h + 7.0,
            size,
            &t.header,
            &widths,
            &numeric,
        );
        doc.set_font_bold(false);
        y -= row_h;
        for r in t.rows.iter().skip(page * per_page).take(per_page) {
            let bold = r.kind != RowKind::Line;
            doc.set_font_bold(bold);
            draw_row(
                &mut doc,
                margin,
                y - row_h + 7.0,
                size,
                &r.cells,
                &widths,
                &numeric,
            );
            if spec.appearance.horizontal_lines {
                doc.set_gray(0.75);
                doc.line(margin, y - row_h + 3.0, pw - margin, y - row_h + 3.0, 0.3);
                doc.set_gray(0.0);
            }
            y -= row_h;
        }
        doc.set_font_bold(false);
        doc.set_gray(0.4);
        doc.text_right(
            pw - margin,
            margin - 4.0,
            7.0,
            &format!("Page {} of {pages}", page + 1),
        );
        doc.set_gray(0.0);
    }
    doc.finish()
}

fn draw_row(
    doc: &mut PdfDoc,
    x0: f64,
    y: f64,
    size: f64,
    cells: &[String],
    widths: &[f64],
    numeric: &[bool],
) {
    let mut x = x0;
    for (i, w) in widths.iter().enumerate() {
        let text = cells.get(i).map_or("", String::as_str);
        let fitted = fit(text, w - 6.0, size);
        if numeric.get(i).copied().unwrap_or(false) {
            doc.text_right(x + w - 3.0, y, size, &fitted);
        } else {
            doc.text(x + 3.0, y, size, &fitted);
        }
        x += w;
    }
}

/// `text` shortened with an ellipsis to fit `width` points.
fn fit(text: &str, width: f64, size: f64) -> String {
    if PdfDoc::text_width(text, size) <= width {
        return text.to_string();
    }
    let mut s: String = text.to_string();
    while !s.is_empty() && PdfDoc::text_width(&format!("{s}..."), size) > width {
        s.pop();
    }
    format!("{s}...")
}
