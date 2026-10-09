//! Where a schedule's cells fall: the table cut into wrapped tables, swapped
//! or not, with column widths, row heights and the turn about its centre.
//! Everything here is plain geometry in plan inches (no painter), so it is
//! tested without a window.
//!
//! Local coordinates run right (`x`) and down (`y`) from the upper-left
//! corner of the schedule before it is turned; a point of the plan is
//! `position + (x, -y)`, then turned about the middle of the schedule.

use plan_core::geometry::Point;
use plan_core::schedules::{ColumnSpec, Schedule, TableAlign, TextAlign, WrapBy};
use plan_docs::schedule::wrap_counts;
use plan_docs::schedule_kinds::Preview;
use plan_docs::Schedule as Table;

/// Characters narrower than this many character heights are rare; a column is
/// sized as `chars * CHAR_W * height`.
pub const CHAR_W: f64 = 0.56;
/// Row height in character heights.
pub const ROW_H: f64 = 1.55;
/// Title row height in character heights.
pub const TITLE_H: f64 = 2.1;
/// Line spacing of a cell that wraps, in character heights.
pub const LINE_H: f64 = 1.2;
/// Height of a row with an object preview, in character heights.
pub const PREVIEW_ROW_H: f64 = 4.2;
/// Narrowest object preview column, in character heights.
pub const PREVIEW_COL_W: f64 = 6.0;

/// Character heights of the three text styles of a schedule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    /// Main Text Style.
    pub body: f64,
    /// Title Text Style.
    pub title: f64,
    /// Header Text Style.
    pub head: f64,
}

impl Metrics {
    pub fn uniform(h: f64) -> Self {
        let h = h.max(0.5);
        Self {
            body: h,
            title: h * 1.15,
            head: h,
        }
    }
}

/// The width of `s` set at character height `h`.
pub fn text_w(s: &str, h: f64, bold: bool) -> f64 {
    s.chars().count() as f64 * h * if bold { CHAR_W * 1.1 } else { CHAR_W }
}

/// `text` broken into lines no wider than `max` at character height `h`
/// (words stay whole unless one is wider than the column).
pub fn wrap_text(text: &str, max: f64, h: f64) -> Vec<String> {
    if max <= 0.0 || text_w(text, h, false) <= max {
        return vec![text.to_string()];
    }
    let per_line = ((max / (h * CHAR_W)).floor() as usize).max(1);
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let mut word = word.to_string();
        loop {
            let used = cur.chars().count();
            let w = word.chars().count();
            if used == 0 && w <= per_line {
                cur = word;
                break;
            }
            if used > 0 && used + 1 + w <= per_line {
                cur.push(' ');
                cur.push_str(&word);
                break;
            }
            if used > 0 {
                lines.push(std::mem::take(&mut cur));
                continue;
            }
            // A word wider than the column is cut.
            let head: String = word.chars().take(per_line).collect();
            let rest: String = word.chars().skip(per_line).collect();
            lines.push(head);
            word = rest;
            if word.is_empty() {
                break;
            }
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// One table of a (possibly wrapped) schedule.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    /// The cells, headings included.
    pub grid: Vec<Vec<String>>,
    /// Leading rows set as headings (an unswapped table with its column
    /// headings shown).
    pub head_rows: usize,
    /// Leading columns set as headings (a swapped table with its headings
    /// shown).
    pub head_cols: usize,
    /// Does this table carry the title row?
    pub title: bool,
    /// Upper-left corner, local.
    pub x: f64,
    pub y: f64,
    pub col_w: Vec<f64>,
    pub row_h: Vec<f64>,
    pub title_h: f64,
    pub w: f64,
    pub h: f64,
    pub swapped: bool,
    /// The first object (row of the data table) in this table, and how many.
    pub first: usize,
    pub count: usize,
}

impl Piece {
    /// Upper-left corner of cell `(r, c)`, local.
    pub fn cell_origin(&self, r: usize, c: usize) -> (f64, f64) {
        (
            self.x + self.col_w[..c.min(self.col_w.len())].iter().sum::<f64>(),
            self.y + self.title_h + self.row_h[..r.min(self.row_h.len())].iter().sum::<f64>(),
        )
    }

    /// What cell `(r, c)` shows: the attribute (column of the data table)
    /// and the object (row of the data table), either of which a heading
    /// does not have.
    pub fn cell_ref(&self, r: usize, c: usize) -> (Option<usize>, Option<usize>) {
        if self.swapped {
            let obj = (c >= self.head_cols).then(|| self.first + c - self.head_cols);
            (Some(r), obj)
        } else {
            let obj = (r >= self.head_rows).then(|| self.first + r - self.head_rows);
            (Some(c), obj)
        }
    }

    /// Local rectangle `(x0, y0, x1, y1)`.
    pub fn rect(&self) -> (f64, f64, f64, f64) {
        (self.x, self.y, self.x + self.w, self.y + self.h)
    }
}

/// A laid-out schedule.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// The data table: one row per object, as the plan builds it.
    pub table: Table,
    /// Character height of the table text.
    pub h: f64,
    /// Width of each column of the first table.
    pub col_w: Vec<f64>,
    pub width: f64,
    pub title_h: f64,
    /// Height of an ordinary row.
    pub row_h: f64,
    pub height: f64,
    pub pieces: Vec<Piece>,
    /// The field id of each column of the data table.
    pub fields: Vec<String>,
    /// The alignment each column asks for (`None` follows the schedule).
    pub aligns: Vec<Option<TextAlign>>,
    pub previews: Vec<Option<Preview>>,
    pub swapped: bool,
    /// Degrees counter-clockwise about the middle.
    pub angle: f64,
    pub metrics: Metrics,
}

/// A point in local coordinates turned into the plan and back.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub origin: Point,
    pub centre: Point,
    cos: f64,
    sin: f64,
}

impl Frame {
    pub fn to_plan(&self, x: f64, y: f64) -> Point {
        let p = Point::new(self.origin.x + x, self.origin.y - y);
        let (dx, dy) = (p.x - self.centre.x, p.y - self.centre.y);
        Point::new(
            self.centre.x + dx * self.cos - dy * self.sin,
            self.centre.y + dx * self.sin + dy * self.cos,
        )
    }

    pub fn to_local(&self, p: Point) -> (f64, f64) {
        let (dx, dy) = (p.x - self.centre.x, p.y - self.centre.y);
        let (ux, uy) = (
            self.centre.x + dx * self.cos + dy * self.sin,
            self.centre.y - dx * self.sin + dy * self.cos,
        );
        (ux - self.origin.x, self.origin.y - uy)
    }

    pub fn angle(&self) -> f64 {
        self.sin.atan2(self.cos)
    }
}

impl Layout {
    /// The frame of the schedule whose upper-left corner is `top_left`.
    pub fn frame(&self, top_left: Point) -> Frame {
        let a = self.angle.to_radians();
        Frame {
            origin: top_left,
            centre: Point::new(
                top_left.x + self.width / 2.0,
                top_left.y - self.height / 2.0,
            ),
            cos: a.cos(),
            sin: a.sin(),
        }
    }

    /// The rectangle `(min, max)` in plan inches (Y up) that holds the
    /// schedule with the upper-left corner at `top_left`, turned.
    pub fn bounds(&self, top_left: Point) -> (Point, Point) {
        let f = self.frame(top_left);
        let corners = [
            f.to_plan(0.0, 0.0),
            f.to_plan(self.width, 0.0),
            f.to_plan(self.width, self.height),
            f.to_plan(0.0, self.height),
        ];
        let mut lo = corners[0];
        let mut hi = corners[0];
        for c in &corners[1..] {
            lo = Point::new(lo.x.min(c.x), lo.y.min(c.y));
            hi = Point::new(hi.x.max(c.x), hi.y.max(c.y));
        }
        (lo, hi)
    }

    /// Is the plan point `p` on the schedule?
    pub fn contains(&self, top_left: Point, p: Point) -> bool {
        let (x, y) = self.frame(top_left).to_local(p);
        self.pieces.iter().any(|pc| {
            let (x0, y0, x1, y1) = pc.rect();
            x >= x0 && x <= x1 && y >= y0 && y <= y1
        })
    }
}

/// What the layout needs besides the table.
pub struct Input<'a> {
    pub table: Table,
    pub def: &'a Schedule,
    /// The columns of the table (`schedule_kinds::effective_columns`).
    pub cols: &'a [ColumnSpec],
    pub previews: Vec<Option<Preview>>,
    pub metrics: Metrics,
}

fn is_preview(field: &str) -> bool {
    plan_core::schedules::is_preview_field(field)
}

/// How many trailing rows are totals (no object behind them, not blank).
fn totals_tail(table: &Table, previews: &[Option<Preview>]) -> usize {
    let mut n = 0;
    for (i, row) in table.rows.iter().enumerate().rev() {
        let blank = row.iter().all(String::is_empty);
        if previews.get(i).is_some_and(Option::is_none) && !blank {
            n += 1;
        } else {
            break;
        }
    }
    n.min(1)
}

/// Sizes the schedule: column widths and row heights from the text and the
/// columns' own widths, the title and heading rows, wrapping and swapping.
pub fn build(input: Input) -> Layout {
    let Input {
        table,
        def,
        cols,
        previews,
        metrics,
    } = input;
    let h = metrics.body.max(0.5);
    let [ml, mr, mt, mb] = def.margins;
    let base_row = (ROW_H * h).max(h + mt + mb);
    let head_row = (ROW_H * metrics.head).max(metrics.head + mt + mb);
    let title_h = if def.show_title {
        (TITLE_H * metrics.title).max(metrics.title + mt + mb)
    } else {
        0.0
    };
    let fields: Vec<String> = cols.iter().map(|c| c.field.clone()).collect();
    let tail = totals_tail(&table, &previews);
    let swapped = def.swap && !table.columns.is_empty();

    // Column widths of the data table (an unswapped table shares them over
    // its wrapped tables).
    let preview_col = |i: usize| fields.get(i).is_some_and(|f| is_preview(f));
    let data_w: Vec<f64> = (0..table.columns.len())
        .map(|i| {
            let own = cols.get(i).map_or(0.0, |c| c.width);
            if own > 0.0 {
                return own;
            }
            let mut w = table
                .rows
                .iter()
                .map(|r| text_w(r.get(i).map_or("", String::as_str), h, false))
                .fold(0.0_f64, f64::max);
            if def.show_headings {
                w = w.max(text_w(&table.columns[i], metrics.head, true));
            }
            if preview_col(i) {
                w = w.max(PREVIEW_COL_W * h);
            }
            w + ml + mr
        })
        .collect();
    let lines_of =
        |text: &str, width: f64, ch: f64| wrap_text(text, (width - ml - mr).max(ch), ch).len();
    let has_preview = |r: usize| {
        previews.get(r).is_some_and(Option::is_some) && (0..table.columns.len()).any(preview_col)
    };
    // Height of data row `r` of the table.
    let data_row_h: Vec<f64> = table
        .rows
        .iter()
        .enumerate()
        .map(|(r, row)| {
            let lines = row
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    if preview_col(i) {
                        1
                    } else {
                        lines_of(t, data_w[i], h)
                    }
                })
                .max()
                .unwrap_or(1);
            let mut hh = if lines > 1 {
                base_row.max(lines as f64 * LINE_H * h + mt + mb)
            } else {
                base_row
            };
            if has_preview(r) {
                hh = hh.max(PREVIEW_ROW_H * h);
            }
            hh
        })
        .collect();

    let show_title = def.show_title;
    let wrap = def.wrap;
    let mut pieces: Vec<Piece> = Vec::new();

    if !swapped {
        let head_rows = usize::from(def.show_headings);
        let overhead_first = title_h + if def.show_headings { head_row } else { 0.0 };
        let overhead_rest = (if wrap.title_each && show_title {
            title_h
        } else {
            0.0
        }) + (if wrap.headings_each && def.show_headings {
            head_row
        } else {
            0.0
        });
        let counts: Vec<usize> = if wrap.enabled {
            wrap_counts(&data_row_h, overhead_first, overhead_rest, wrap.by)
        } else {
            vec![table.rows.len()]
        };
        let parts = if wrap.enabled {
            table.wrapped(&counts, tail)
        } else {
            vec![table.clone()]
        };
        let mut first = 0usize;
        let mut x = 0.0;
        for (k, part) in parts.iter().enumerate() {
            let heads = head_rows > 0 && (k == 0 || wrap.headings_each);
            let title = show_title && (k == 0 || wrap.title_each);
            let mut grid: Vec<Vec<String>> = Vec::new();
            let mut row_h: Vec<f64> = Vec::new();
            if heads {
                grid.push(part.columns.clone());
                row_h.push(head_row);
            }
            for (j, row) in part.rows.iter().enumerate() {
                grid.push(row.clone());
                row_h.push(data_row_h[(first + j).min(data_row_h.len().saturating_sub(1))]);
            }
            let w: f64 = data_w.iter().sum();
            let t_h = if title { title_h } else { 0.0 };
            let body_h: f64 = row_h.iter().sum();
            let count = part.rows.len();
            pieces.push(Piece {
                grid,
                head_rows: usize::from(heads),
                head_cols: 0,
                title,
                x,
                y: 0.0,
                col_w: data_w.clone(),
                row_h,
                title_h: t_h,
                w,
                h: t_h + body_h,
                swapped: false,
                first,
                count,
            });
            first += count;
            x += w + wrap.offset;
        }
        // The title is never narrower than its text.
        if let Some(p0) = pieces.first_mut() {
            if p0.title {
                let need = text_w(&table.title, metrics.title, true) + ml + mr;
                if need > p0.w {
                    p0.w = need;
                }
            }
        }
        align_pieces(&mut pieces, false, wrap.align, wrap.justify, base_row);
    } else {
        // Swapped: attributes down, objects across. The object columns are as
        // wide as their widest cell.
        let n_obj = table.rows.len();
        let attr_w = |t: &Table| -> f64 {
            let heading = if def.show_headings {
                t.columns
                    .iter()
                    .map(|c| text_w(c, metrics.head, true))
                    .fold(0.0, f64::max)
            } else {
                0.0
            };
            heading + ml + mr
        };
        let head_w = if def.show_headings {
            attr_w(&table).max(PREVIEW_COL_W * h * 0.5)
        } else {
            0.0
        };
        let obj_w: Vec<f64> = (0..n_obj)
            .map(|r| {
                let mut w = table.rows[r]
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        if preview_col(i) {
                            PREVIEW_COL_W * h
                        } else {
                            text_w(t, h, false)
                        }
                    })
                    .fold(0.0_f64, f64::max);
                w = w.max(text_w(
                    &table.rows[r].first().cloned().unwrap_or_default(),
                    h,
                    false,
                ));
                w + ml + mr
            })
            .collect();
        let attr_h: Vec<f64> = (0..table.columns.len())
            .map(|i| {
                let mut hh = if i == 0 {
                    head_row.max(base_row)
                } else {
                    base_row
                };
                if preview_col(i) {
                    hh = hh.max(PREVIEW_ROW_H * h);
                }
                hh
            })
            .collect();
        let overhead_first = head_w;
        let overhead_rest = if wrap.headings_each { head_w } else { 0.0 };
        let counts: Vec<usize> = if wrap.enabled {
            wrap_counts(&obj_w, overhead_first, overhead_rest, wrap.by)
        } else {
            vec![n_obj]
        };
        let parts = if wrap.enabled {
            table.wrapped(&counts, tail)
        } else {
            vec![table.clone()]
        };
        let mut first = 0usize;
        let mut y = 0.0;
        for (k, part) in parts.iter().enumerate() {
            let heads = def.show_headings && (k == 0 || wrap.headings_each);
            let title = show_title && (k == 0 || wrap.title_each);
            let t = part.transposed();
            // grid row 0 holds the first attribute (the objects' numbers).
            let mut grid: Vec<Vec<String>> = Vec::new();
            let mut head_line = t.columns.clone();
            if !heads {
                head_line.remove(0);
            }
            grid.push(head_line);
            for row in &t.rows {
                let mut r = row.clone();
                if !heads {
                    r.remove(0);
                }
                grid.push(r);
            }
            let count = part.rows.len();
            let mut col_w: Vec<f64> = Vec::new();
            if heads {
                col_w.push(head_w);
            }
            col_w.extend_from_slice(&obj_w[first..first + count]);
            let w: f64 = col_w.iter().sum();
            let t_h = if title { title_h } else { 0.0 };
            let body_h: f64 = attr_h.iter().sum();
            pieces.push(Piece {
                grid,
                head_rows: 0,
                head_cols: usize::from(heads),
                title,
                x: 0.0,
                y,
                col_w,
                row_h: attr_h.clone(),
                title_h: t_h,
                w,
                h: t_h + body_h,
                swapped: true,
                first,
                count,
            });
            first += count;
            y += t_h + body_h + wrap.offset;
        }
        if let Some(p0) = pieces.first_mut() {
            if p0.title {
                let need = text_w(&table.title, metrics.title, true) + ml + mr;
                if need > p0.w {
                    p0.w = need;
                }
            }
        }
        align_pieces(&mut pieces, true, wrap.align, wrap.justify, base_row);
    }

    let width = pieces.iter().map(|p| p.x + p.w).fold(0.0_f64, f64::max);
    let height = pieces.iter().map(|p| p.y + p.h).fold(0.0_f64, f64::max);
    let first = pieces.first();
    Layout {
        col_w: first.map(|p| p.col_w.clone()).unwrap_or_default(),
        title_h,
        row_h: base_row,
        h,
        width,
        height,
        fields,
        aligns: cols.iter().map(|c| c.align).collect(),
        previews,
        swapped,
        angle: def.angle,
        metrics,
        table,
        pieces,
    }
}

/// Lines wrapped tables up against each other: `Top`, `Centered` or
/// `Bottom` (`Left`, `Centered`, `Right` when swapped), and Justify Wrapped
/// Tables, which stretches the rows (or columns) so all are the same size.
fn align_pieces(
    pieces: &mut [Piece],
    swapped: bool,
    align: TableAlign,
    justify: bool,
    base_row: f64,
) {
    if pieces.len() < 2 {
        return;
    }
    if !swapped {
        let tallest = pieces.iter().map(|p| p.h).fold(0.0_f64, f64::max);
        for p in pieces.iter_mut() {
            if justify {
                // Stretch the body rows so every table is as tall as the
                // tallest.
                let body: f64 = p.row_h.iter().sum();
                let want = body + (tallest - p.h);
                if body > 0.0 && want > body {
                    let k = want / body;
                    p.row_h.iter_mut().for_each(|r| *r *= k);
                    p.h = tallest;
                }
            }
            p.y = match align {
                TableAlign::Start => 0.0,
                TableAlign::Centered => (tallest - p.h) / 2.0,
                TableAlign::End => tallest - p.h,
            };
        }
    } else {
        let widest = pieces.iter().map(|p| p.w).fold(0.0_f64, f64::max);
        for p in pieces.iter_mut() {
            if justify {
                // Stretch the object columns so every table is as wide as the
                // widest.
                let body: f64 = p.col_w.iter().skip(p.head_cols).sum();
                let want = body + (widest - p.w);
                if body > 0.0 && want > body {
                    let k = want / body;
                    p.col_w.iter_mut().skip(p.head_cols).for_each(|c| *c *= k);
                    p.w = widest;
                }
            }
            p.x = match align {
                TableAlign::Start => 0.0,
                TableAlign::Centered => (widest - p.w) / 2.0,
                TableAlign::End => widest - p.w,
            };
        }
    }
    let _ = base_row;
}

/// `WrapBy` of a wrapped length typed in plan inches, the way the Wrap
/// handle sets it.
pub fn max_size(len: f64) -> WrapBy {
    WrapBy::MaxSize(len.max(1.0))
}
