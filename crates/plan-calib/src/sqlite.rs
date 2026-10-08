//! A minimal, read-only SQLite 3 reader.
//!
//! Chief Architect `.calib` files are ordinary SQLite databases that can be
//! several gigabytes. This module reads them with positioned reads, one page
//! at a time, and never loads a whole file into memory.
//!
//! Supported: rowid tables stored in table b-trees (interior `0x05` and leaf
//! `0x0D` pages), all record serial types, overflow chains, UTF-8 text.
//! Not supported (reported as [`Error::Unsupported`]): WAL mode, UTF-16 text,
//! `WITHOUT ROWID` tables, and databases with a hot rollback journal. Indexes
//! are ignored; every lookup is by rowid or by full table scan.

use crate::error::{Error, Result};
use crate::source::Source;
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Maximum number of interior b-tree pages kept in memory (bounds the cache
/// at `INTERIOR_CACHE_CAP * page_size`, 16 MiB for 4 KiB pages).
const INTERIOR_CACHE_CAP: usize = 4096;
/// Deepest b-tree we accept before declaring the file corrupt.
const MAX_DEPTH: usize = 40;
/// Sanity cap on the number of columns in one record.
const MAX_COLUMNS: usize = 4096;

/// One SQLite value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// SQL `NULL`.
    Null,
    /// Any of the integer storage classes, widened to `i64`.
    Int(i64),
    /// An IEEE-754 double.
    Real(f64),
    /// UTF-8 text (invalid sequences are replaced).
    Text(String),
    /// A blob with its bytes read. When the read used a prefix limit the vector
    /// is truncated to that limit.
    Blob(Vec<u8>),
    /// A blob whose bytes were deliberately not read; holds its full length.
    BlobLen(usize),
}

impl Value {
    /// The integer, if this is [`Value::Int`].
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(v) => Some(*v),
            _ => None,
        }
    }

    /// The number as `f64` for [`Value::Int`] and [`Value::Real`].
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(v) => Some(*v as f64),
            Value::Real(v) => Some(*v),
            _ => None,
        }
    }

    /// The text, if this is [`Value::Text`].
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }

    /// The blob bytes, if this is [`Value::Blob`].
    pub fn as_blob(&self) -> Option<&[u8]> {
        match self {
            Value::Blob(b) => Some(b),
            _ => None,
        }
    }

    /// True for SQL `NULL`.
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Full blob length for [`Value::Blob`] (as read) and [`Value::BlobLen`].
    pub fn blob_len(&self) -> Option<usize> {
        match self {
            Value::Blob(b) => Some(b.len()),
            Value::BlobLen(n) => Some(*n),
            _ => None,
        }
    }

    /// Owned text if this is [`Value::Text`] and non-empty.
    pub fn non_empty_text(&self) -> Option<String> {
        match self {
            Value::Text(s) if !s.is_empty() => Some(s.clone()),
            _ => None,
        }
    }
}

/// How blob bytes are handled while decoding a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlobMode {
    /// Report [`Value::BlobLen`], read nothing.
    Skip,
    /// Read every byte.
    Full,
    /// Read at most this many leading bytes.
    Prefix(usize),
}

/// Schema information for one table, parsed from `sqlite_master`.
#[derive(Debug, Clone)]
struct TableInfo {
    name: String,
    root: u32,
    columns: Vec<String>,
    /// Index of an `INTEGER PRIMARY KEY` column (stored as NULL in records and
    /// replaced by the rowid on read).
    rowid_alias: Option<usize>,
    without_rowid: bool,
}

/// A read-only handle on a SQLite database file.
pub struct Db {
    src: Source,
    path: PathBuf,
    page_size: usize,
    usable: usize,
    page_count: u32,
    tables: HashMap<String, TableInfo>,
    table_order: Vec<String>,
    cache: Mutex<HashMap<u32, Arc<Vec<u8>>>>,
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Db")
            .field("path", &self.path)
            .field("page_size", &self.page_size)
            .field("page_count", &self.page_count)
            .field("tables", &self.table_order)
            .finish()
    }
}

impl Db {
    /// Opens `path`, validates the 100-byte header, and loads the table list
    /// from `sqlite_master`.
    pub fn open(path: impl AsRef<Path>) -> Result<Db> {
        let path = path.as_ref();
        let mut journal = path.as_os_str().to_owned();
        journal.push("-journal");
        if std::fs::metadata(&journal)
            .map(|m| m.len() > 0)
            .unwrap_or(false)
        {
            return Err(Error::unsupported(format!(
                "{} has a hot rollback journal; refusing to read an inconsistent database",
                path.display()
            )));
        }

        let src = Source::open(path)?;
        let mut h = [0u8; 100];
        src.read_exact_at(&mut h, 0)
            .map_err(|_| Error::corrupt("file is shorter than the 100-byte SQLite header"))?;
        if &h[..16] != b"SQLite format 3\0" {
            return Err(Error::corrupt("missing 'SQLite format 3' magic"));
        }
        let page_size = match u16::from_be_bytes([h[16], h[17]]) {
            1 => 65536,
            n if n >= 512 && n.is_power_of_two() => n as usize,
            n => return Err(Error::corrupt(format!("invalid page size {n}"))),
        };
        if h[18] == 2 || h[19] == 2 {
            return Err(Error::unsupported(
                "database is in WAL mode (header bytes 18/19 = 2); only rollback-journal files are supported",
            ));
        }
        let usable = page_size - h[20] as usize;
        if usable < 480 {
            return Err(Error::corrupt(
                "reserved space leaves fewer than 480 usable bytes",
            ));
        }
        let encoding = u32::from_be_bytes([h[56], h[57], h[58], h[59]]);
        if encoding > 1 {
            return Err(Error::unsupported(format!(
                "text encoding {encoding} (only UTF-8 is supported)"
            )));
        }
        let page_count = (src.len() / page_size as u64) as u32;
        if page_count == 0 {
            return Err(Error::corrupt("file contains no complete page"));
        }

        let mut db = Db {
            src,
            path: path.to_owned(),
            page_size,
            usable,
            page_count,
            tables: HashMap::new(),
            table_order: Vec::new(),
            cache: Mutex::new(HashMap::new()),
        };

        // sqlite_master(type, name, tbl_name, rootpage, sql) always lives at page 1.
        let master = TableInfo {
            name: "sqlite_master".into(),
            root: 1,
            columns: vec![
                "type".into(),
                "name".into(),
                "tbl_name".into(),
                "rootpage".into(),
                "sql".into(),
            ],
            rowid_alias: None,
            without_rowid: false,
        };
        let rows: Vec<Vec<Value>> = RowIter::new(&db, &master).collect::<Result<_>>()?;
        for row in rows {
            let kind = row.first().and_then(Value::as_str).unwrap_or("");
            if kind != "table" {
                continue;
            }
            let name = row.get(1).and_then(Value::as_str).unwrap_or("").to_owned();
            let root = row.get(3).and_then(Value::as_i64).unwrap_or(0);
            let sql = row.get(4).and_then(Value::as_str).unwrap_or("");
            if root <= 0 || name.is_empty() || name.starts_with("sqlite_") {
                continue;
            }
            let (columns, rowid_alias, without_rowid) = parse_create_table(sql);
            let key = name.to_ascii_lowercase();
            db.table_order.push(name.clone());
            db.tables.insert(
                key,
                TableInfo {
                    name,
                    root: root as u32,
                    columns,
                    rowid_alias,
                    without_rowid,
                },
            );
        }
        Ok(db)
    }

    /// The path this database was opened from.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Database page size in bytes.
    pub fn page_size(&self) -> usize {
        self.page_size
    }

    /// Size of the file in bytes.
    pub fn file_len(&self) -> u64 {
        self.src.len()
    }

    /// Names of all ordinary tables, in schema order.
    pub fn table_names(&self) -> &[String] {
        &self.table_order
    }

    /// True when a table of that name exists (case-insensitive).
    pub fn has_table(&self, name: &str) -> bool {
        self.tables.contains_key(&name.to_ascii_lowercase())
    }

    /// Column names of a table, parsed from its `CREATE TABLE` text.
    pub fn columns(&self, table: &str) -> Option<&[String]> {
        self.tables
            .get(&table.to_ascii_lowercase())
            .map(|t| t.columns.as_slice())
    }

    /// Index of `column` in `table` (case-insensitive).
    pub fn column_index(&self, table: &str, column: &str) -> Option<usize> {
        self.columns(table)?
            .iter()
            .position(|c| c.eq_ignore_ascii_case(column))
    }

    fn table_info(&self, name: &str) -> Result<&TableInfo> {
        let t = self
            .tables
            .get(&name.to_ascii_lowercase())
            .ok_or_else(|| Error::not_found(format!("table '{name}'")))?;
        if t.without_rowid {
            return Err(Error::unsupported(format!(
                "table '{}' is WITHOUT ROWID",
                t.name
            )));
        }
        Ok(t)
    }

    /// Iterates all rows of `name` in rowid order. Blobs are read in full and
    /// every column is decoded unless narrowed with [`RowIter::with_blobs`]
    /// and [`RowIter::with_max_columns`].
    pub fn table(&self, name: &str) -> Result<RowIter<'_>> {
        let info = self.table_info(name)?;
        Ok(RowIter::new(self, info))
    }

    /// Reads a single column of the row with the given rowid, or `None` when
    /// the row does not exist. For `Text` and `Blob` values at most `limit`
    /// leading bytes are read, which makes peeking at a header of a multi-MB
    /// blob cheap. A rowid-alias column returns the rowid itself.
    pub fn read_column(
        &self,
        table: &str,
        rowid: i64,
        column: usize,
        limit: Option<usize>,
    ) -> Result<Option<Value>> {
        let info = self.table_info(table)?;
        let Some((page, off)) = self.find_leaf_cell(info.root, rowid)? else {
            return Ok(None);
        };
        if info.rowid_alias == Some(column) {
            return Ok(Some(Value::Int(rowid)));
        }
        let (_, mut payload) = self.open_payload(&page, off)?;
        let types = read_serial_types(&mut payload)?;
        if column >= types.len() {
            return Ok(Some(Value::Null));
        }
        let offset: usize = types.header_len
            + types[..column]
                .iter()
                .map(|&t| serial_size(t))
                .sum::<usize>();
        let mode = limit.map_or(BlobMode::Full, BlobMode::Prefix);
        let text_cap = limit.unwrap_or(usize::MAX);
        let v = read_value(&mut payload, types[column], offset, mode, text_cap)?;
        Ok(Some(v))
    }

    // ---------------------------------------------------------------- pages

    /// Reads page `pgno` (1-based). Interior b-tree pages are cached.
    fn page(&self, pgno: u32) -> Result<Arc<Vec<u8>>> {
        if pgno == 0 || pgno > self.page_count {
            return Err(Error::corrupt(format!(
                "page {pgno} out of range (file has {} pages)",
                self.page_count
            )));
        }
        if let Some(p) = self
            .cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&pgno)
        {
            return Ok(Arc::clone(p));
        }
        let mut buf = vec![0u8; self.page_size];
        self.src
            .read_exact_at(&mut buf, (pgno as u64 - 1) * self.page_size as u64)?;
        let page = Arc::new(buf);
        let hdr = if pgno == 1 { 100 } else { 0 };
        if page[hdr] == 0x05 {
            let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            if cache.len() < INTERIOR_CACHE_CAP {
                cache.insert(pgno, Arc::clone(&page));
            }
        }
        Ok(page)
    }

    fn frame(&self, pgno: u32) -> Result<Frame> {
        let page = self.page(pgno)?;
        let hdr = if pgno == 1 { 100 } else { 0 };
        let kind = page[hdr];
        let (head, right) = match kind {
            0x0D => (8, 0),
            0x05 => (
                12,
                u32::from_be_bytes([page[hdr + 8], page[hdr + 9], page[hdr + 10], page[hdr + 11]]),
            ),
            other => {
                return Err(Error::corrupt(format!(
                    "page {pgno}: unexpected b-tree page type 0x{other:02x} in a table"
                )))
            }
        };
        let ncells = u16::from_be_bytes([page[hdr + 3], page[hdr + 4]]) as usize;
        if hdr + head + 2 * ncells > page.len() {
            return Err(Error::corrupt(format!(
                "page {pgno}: cell pointer array overruns page"
            )));
        }
        Ok(Frame {
            page,
            hdr,
            head,
            kind,
            ncells,
            idx: 0,
            right,
        })
    }

    /// Descends the table b-tree rooted at `root` to the leaf cell for `rowid`.
    fn find_leaf_cell(&self, root: u32, rowid: i64) -> Result<Option<(Arc<Vec<u8>>, usize)>> {
        let mut pgno = root;
        for _ in 0..MAX_DEPTH {
            let f = self.frame(pgno)?;
            if f.kind == 0x0D {
                let (mut lo, mut hi) = (0usize, f.ncells);
                while lo < hi {
                    let mid = (lo + hi) / 2;
                    let off = f.cell_offset(mid)?;
                    let (_, n1) = varint(&f.page, off)?;
                    let (key, _) = varint(&f.page, off + n1)?;
                    match (key as i64).cmp(&rowid) {
                        std::cmp::Ordering::Equal => return Ok(Some((f.page, off))),
                        std::cmp::Ordering::Less => lo = mid + 1,
                        std::cmp::Ordering::Greater => hi = mid,
                    }
                }
                return Ok(None);
            }
            // Interior: first cell whose key >= rowid owns the subtree.
            let (mut lo, mut hi) = (0usize, f.ncells);
            while lo < hi {
                let mid = (lo + hi) / 2;
                let off = f.cell_offset(mid)?;
                let (key, _) = varint(&f.page, off + 4)?;
                if (key as i64) >= rowid {
                    hi = mid;
                } else {
                    lo = mid + 1;
                }
            }
            pgno = if lo < f.ncells {
                f.child_at(lo)?
            } else {
                f.right
            };
        }
        Err(Error::corrupt("b-tree deeper than 40 levels"))
    }

    /// Parses a table-leaf cell at `off`, returning its rowid and a payload
    /// reader that follows the overflow chain on demand.
    fn open_payload<'a>(&'a self, page: &'a [u8], off: usize) -> Result<(i64, Payload<'a>)> {
        let (total, n1) = varint(page, off)?;
        let (rowid, n2) = varint(page, off + n1)?;
        let total = usize::try_from(total).map_err(|_| Error::corrupt("payload size overflow"))?;
        let start = off + n1 + n2;
        let u = self.usable;
        let max_local = u - 35;
        let local = if total <= max_local {
            total
        } else {
            let min_local = (u - 12) * 32 / 255 - 23;
            let k = min_local + (total - min_local) % (u - 4);
            if k <= max_local {
                k
            } else {
                min_local
            }
        };
        if start + local > page.len() {
            return Err(Error::corrupt("cell payload overruns page"));
        }
        let first_overflow = if total > local {
            if start + local + 4 > page.len() {
                return Err(Error::corrupt("missing overflow pointer"));
            }
            u32::from_be_bytes([
                page[start + local],
                page[start + local + 1],
                page[start + local + 2],
                page[start + local + 3],
            ])
        } else {
            0
        };
        Ok((
            rowid as i64,
            Payload {
                db: self,
                local: &page[start..start + local],
                total,
                first_overflow,
                chain: Vec::new(),
            },
        ))
    }
}

/// An open b-tree page being traversed.
struct Frame {
    page: Arc<Vec<u8>>,
    /// Offset of the b-tree page header (100 on page 1, else 0).
    hdr: usize,
    /// Length of the page header (8 leaf, 12 interior).
    head: usize,
    kind: u8,
    ncells: usize,
    idx: usize,
    right: u32,
}

impl Frame {
    fn cell_offset(&self, i: usize) -> Result<usize> {
        let p = self.hdr + self.head + 2 * i;
        let off = u16::from_be_bytes([self.page[p], self.page[p + 1]]) as usize;
        if off >= self.page.len() {
            return Err(Error::corrupt("cell offset beyond page"));
        }
        Ok(off)
    }

    /// Left-child page number of interior cell `i`.
    fn child_at(&self, i: usize) -> Result<u32> {
        let off = self.cell_offset(i)?;
        if off + 4 > self.page.len() {
            return Err(Error::corrupt("interior cell overruns page"));
        }
        Ok(u32::from_be_bytes([
            self.page[off],
            self.page[off + 1],
            self.page[off + 2],
            self.page[off + 3],
        ]))
    }
}

/// Lazy row iterator over one table.
///
/// Yields `Result<Vec<Value>>` with one entry per column (in `CREATE TABLE`
/// order, truncated by [`with_max_columns`](Self::with_max_columns)). An
/// `INTEGER PRIMARY KEY` column carries the rowid. After an error the
/// iterator ends.
pub struct RowIter<'a> {
    db: &'a Db,
    stack: Vec<Frame>,
    pending_root: Option<u32>,
    ncols: usize,
    rowid_alias: Option<usize>,
    blobs: bool,
    max_columns: usize,
    failed: bool,
}

impl<'a> RowIter<'a> {
    fn new(db: &'a Db, info: &TableInfo) -> Self {
        RowIter {
            db,
            stack: Vec::new(),
            // The root page is loaded on the first `next()` so that a read
            // error surfaces as an iterator item.
            pending_root: Some(info.root),
            ncols: info.columns.len(),
            rowid_alias: info.rowid_alias,
            blobs: true,
            max_columns: usize::MAX,
            failed: false,
        }
    }

    /// When `false`, blob bytes are not read; blob cells come back as
    /// [`Value::BlobLen`]. Default `true`.
    pub fn with_blobs(mut self, blobs: bool) -> Self {
        self.blobs = blobs;
        self
    }

    /// Decode only the first `n` columns. Columns after a large blob are only
    /// reachable cheaply when blobs are skipped (their sizes sit in the record
    /// header), so combine this with `with_blobs(false)` for wide tables.
    pub fn with_max_columns(mut self, n: usize) -> Self {
        self.max_columns = n;
        self
    }

    fn advance(&mut self) -> Result<Option<Vec<Value>>> {
        if let Some(root) = self.pending_root.take() {
            let f = self.db.frame(root)?;
            self.stack.push(f);
        }
        loop {
            let Some(top) = self.stack.last_mut() else {
                return Ok(None);
            };
            if top.kind == 0x0D {
                if top.idx >= top.ncells {
                    self.stack.pop();
                    continue;
                }
                let off = top.cell_offset(top.idx)?;
                top.idx += 1;
                let page = Arc::clone(&top.page);
                return self.decode_cell(&page, off).map(Some);
            }
            // Interior page: visit each left child, then the right-most pointer.
            if top.idx > top.ncells {
                self.stack.pop();
                continue;
            }
            let child = if top.idx < top.ncells {
                top.child_at(top.idx)?
            } else {
                top.right
            };
            top.idx += 1;
            if self.stack.len() >= MAX_DEPTH {
                return Err(Error::corrupt("b-tree deeper than 40 levels"));
            }
            let f = self.db.frame(child)?;
            self.stack.push(f);
        }
    }

    fn decode_cell(&self, page: &[u8], off: usize) -> Result<Vec<Value>> {
        let (rowid, mut payload) = self.db.open_payload(page, off)?;
        let types = read_serial_types(&mut payload)?;
        let n = self.ncols.min(self.max_columns);
        let mode = if self.blobs {
            BlobMode::Full
        } else {
            BlobMode::Skip
        };
        let mut out = Vec::with_capacity(n);
        let mut offset = types.header_len;
        for i in 0..n {
            if self.rowid_alias == Some(i) {
                out.push(Value::Int(rowid));
                if let Some(&t) = types.get(i) {
                    offset += serial_size(t);
                }
                continue;
            }
            match types.get(i) {
                None => out.push(Value::Null),
                Some(&t) => {
                    out.push(read_value(&mut payload, t, offset, mode, usize::MAX)?);
                    offset += serial_size(t);
                }
            }
        }
        Ok(out)
    }
}

impl Iterator for RowIter<'_> {
    type Item = Result<Vec<Value>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        match self.advance() {
            Ok(Some(row)) => Some(Ok(row)),
            Ok(None) => None,
            Err(e) => {
                self.failed = true;
                Some(Err(e))
            }
        }
    }
}

// ------------------------------------------------------------------ payload

/// Reader over a cell payload: the local bytes plus the overflow page chain.
struct Payload<'a> {
    db: &'a Db,
    local: &'a [u8],
    total: usize,
    first_overflow: u32,
    /// Overflow page numbers discovered so far.
    chain: Vec<u32>,
}

impl<'a> Payload<'a> {
    /// Makes sure `chain[k]` is known by following next-pointers.
    fn ensure_chain(&mut self, k: usize) -> Result<()> {
        while self.chain.len() <= k {
            let next = match self.chain.last() {
                None => self.first_overflow,
                Some(&prev) => {
                    let mut b = [0u8; 4];
                    self.db
                        .src
                        .read_exact_at(&mut b, (prev as u64 - 1) * self.db.page_size as u64)?;
                    u32::from_be_bytes(b)
                }
            };
            if next == 0 || next > self.db.page_count {
                return Err(Error::corrupt(
                    "overflow chain ends early or points out of range",
                ));
            }
            self.chain.push(next);
        }
        Ok(())
    }

    /// Returns `len` payload bytes starting at `off`.
    fn read(&mut self, off: usize, len: usize) -> Result<Cow<'a, [u8]>> {
        let end = off
            .checked_add(len)
            .filter(|&e| e <= self.total)
            .ok_or_else(|| Error::corrupt("record field extends past its payload"))?;
        if end <= self.local.len() {
            return Ok(Cow::Borrowed(&self.local[off..end]));
        }
        let mut out = Vec::with_capacity(len);
        let mut pos = off;
        if pos < self.local.len() {
            out.extend_from_slice(&self.local[pos..]);
            pos = self.local.len();
        }
        let cap = self.db.usable - 4;
        while pos < end {
            let rel = pos - self.local.len();
            let (k, inner) = (rel / cap, rel % cap);
            self.ensure_chain(k)?;
            let take = (cap - inner).min(end - pos);
            let at = (self.chain[k] as u64 - 1) * self.db.page_size as u64 + 4 + inner as u64;
            let start = out.len();
            out.resize(start + take, 0);
            self.db.src.read_exact_at(&mut out[start..], at)?;
            pos += take;
        }
        Ok(Cow::Owned(out))
    }
}

/// Serial types of one record plus the byte length of its header.
struct SerialTypes {
    types: Vec<u64>,
    header_len: usize,
}

impl std::ops::Deref for SerialTypes {
    type Target = [u64];
    fn deref(&self) -> &[u64] {
        &self.types
    }
}

fn read_serial_types(p: &mut Payload<'_>) -> Result<SerialTypes> {
    if p.total == 0 {
        return Ok(SerialTypes {
            types: Vec::new(),
            header_len: 0,
        });
    }
    let first = p.read(0, p.total.min(9))?;
    let (hlen, _) = varint(&first, 0)?;
    let hlen = hlen as usize;
    if hlen == 0 || hlen > p.total {
        return Err(Error::corrupt("record header length out of range"));
    }
    let hdr = p.read(0, hlen)?;
    let mut types = Vec::new();
    let mut pos = varint(&hdr, 0)?.1;
    while pos < hlen {
        let (t, n) = varint(&hdr, pos)?;
        types.push(t);
        pos += n;
        if types.len() > MAX_COLUMNS {
            return Err(Error::corrupt("record has an absurd number of columns"));
        }
    }
    Ok(SerialTypes {
        types,
        header_len: hlen,
    })
}

/// Number of content bytes for a serial type.
fn serial_size(t: u64) -> usize {
    match t {
        0 | 8 | 9 | 10 | 11 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        5 => 6,
        6 | 7 => 8,
        n if n % 2 == 0 => ((n - 12) / 2) as usize,
        n => ((n - 13) / 2) as usize,
    }
}

fn read_value(
    p: &mut Payload<'_>,
    t: u64,
    off: usize,
    blobs: BlobMode,
    text_cap: usize,
) -> Result<Value> {
    let size = serial_size(t);
    Ok(match t {
        0 => Value::Null,
        8 => Value::Int(0),
        9 => Value::Int(1),
        1..=6 => {
            let b = p.read(off, size)?;
            let mut v: i64 = if b[0] & 0x80 != 0 { -1 } else { 0 };
            for &x in b.iter() {
                v = (v << 8) | x as i64;
            }
            Value::Int(v)
        }
        7 => {
            let b = p.read(off, 8)?;
            Value::Real(f64::from_be_bytes(b[..8].try_into().expect("8 bytes")))
        }
        10 | 11 => return Err(Error::corrupt("reserved serial type in record")),
        n if n % 2 == 0 => match blobs {
            BlobMode::Skip => Value::BlobLen(size),
            BlobMode::Full => Value::Blob(p.read(off, size)?.into_owned()),
            BlobMode::Prefix(max) => Value::Blob(p.read(off, size.min(max))?.into_owned()),
        },
        _ => {
            let b = p.read(off, size.min(text_cap))?;
            Value::Text(String::from_utf8_lossy(&b).into_owned())
        }
    })
}

/// Decodes a SQLite varint at `pos`, returning `(value, bytes_used)`.
pub(crate) fn varint(buf: &[u8], pos: usize) -> Result<(u64, usize)> {
    let mut v: u64 = 0;
    for i in 0..9 {
        let b = *buf
            .get(pos + i)
            .ok_or_else(|| Error::corrupt("varint runs past end of buffer"))?;
        if i == 8 {
            return Ok(((v << 8) | b as u64, 9));
        }
        v = (v << 7) | (b & 0x7f) as u64;
        if b & 0x80 == 0 {
            return Ok((v, i + 1));
        }
    }
    unreachable!("loop returns by i == 8")
}

// ------------------------------------------------------------ schema parser

/// Extracts column names from a `CREATE TABLE` statement.
///
/// Returns `(columns, rowid_alias_index, without_rowid)`. The parser splits
/// the parenthesised body at top-level commas (aware of quotes and nested
/// parentheses), takes the first token of each piece as the column name, and
/// skips table constraints (`PRIMARY KEY(..)`, `FOREIGN KEY`, `CHECK`, ...).
pub(crate) fn parse_create_table(sql: &str) -> (Vec<String>, Option<usize>, bool) {
    let chars: Vec<char> = sql.chars().collect();
    let Some(open) = chars.iter().position(|&c| c == '(') else {
        return (Vec::new(), None, false);
    };
    let mut pieces: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut close = chars.len();
    for (i, &c) in chars.iter().enumerate().skip(open) {
        if let Some(q) = quote {
            cur.push(c);
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '\'' | '"' | '`' => {
                quote = Some(c);
                cur.push(c);
            }
            '[' => {
                quote = Some(']');
                cur.push(c);
            }
            '(' => {
                depth += 1;
                if depth > 1 {
                    cur.push(c);
                }
            }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    close = i;
                    break;
                }
                cur.push(c);
            }
            ',' if depth == 1 => pieces.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    pieces.push(cur);

    let tail: String = chars[(close + 1).min(chars.len())..].iter().collect();
    let without_rowid = tail.to_ascii_uppercase().contains("WITHOUT ROWID");

    let mut columns = Vec::new();
    let mut alias = None;
    for piece in pieces {
        let piece = piece.trim();
        if piece.is_empty() {
            continue;
        }
        let (name, rest, quoted) = first_token(piece);
        if !quoted
            && matches!(
                name.to_ascii_uppercase().as_str(),
                "CONSTRAINT" | "PRIMARY" | "FOREIGN" | "UNIQUE" | "CHECK"
            )
        {
            continue;
        }
        let rest_up = rest.to_ascii_uppercase();
        let mut words = rest_up.split_whitespace();
        if words.next() == Some("INTEGER") && rest_up.contains("PRIMARY KEY") {
            alias = Some(columns.len());
        }
        columns.push(name);
    }
    (columns, alias, without_rowid)
}

/// Splits off the first (possibly quoted) token: `(token, rest, was_quoted)`.
fn first_token(s: &str) -> (String, &str, bool) {
    let mut chars = s.char_indices();
    let Some((_, c0)) = chars.next() else {
        return (String::new(), "", false);
    };
    let closer = match c0 {
        '"' => Some('"'),
        '\'' => Some('\''),
        '`' => Some('`'),
        '[' => Some(']'),
        _ => None,
    };
    if let Some(close) = closer {
        for (i, c) in chars {
            if c == close {
                return (s[c0.len_utf8()..i].to_owned(), &s[i + c.len_utf8()..], true);
            }
        }
        return (s[c0.len_utf8()..].to_owned(), "", true);
    }
    let end = s
        .char_indices()
        .find(|&(_, c)| c.is_whitespace() || c == '(')
        .map_or(s.len(), |(i, _)| i);
    (s[..end].to_owned(), &s[end..], false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{self, Fixture};

    #[test]
    fn varints() {
        assert_eq!(varint(&[0x05], 0).unwrap(), (5, 1));
        assert_eq!(varint(&[0x81, 0x00], 0).unwrap(), (128, 2));
        assert_eq!(varint(&[0xff; 9], 0).unwrap(), (u64::MAX, 9));
        assert!(varint(&[0x80], 0).is_err());
    }

    #[test]
    fn create_table_parsing() {
        let (cols, alias, wr) = parse_create_table(
            "CREATE TABLE Tags (\nTagUniqueId TEXT NOT NULL PRIMARY KEY,\nName TEXT NOT NULL,\n\
             Pinned INTEGER NOT NULL DEFAULT 0 CHECK (Pinned IN (0, 1)),\n\
             FOREIGN KEY(Parent) REFERENCES Tags(TagUniqueId) ON DELETE RESTRICT,\n\
             CHECK(LENGTH(TagUniqueId) = 36))",
        );
        assert_eq!(cols, ["TagUniqueId", "Name", "Pinned"]);
        assert_eq!(alias, None);
        assert!(!wr);

        let (cols, alias, _) = parse_create_table(
            "CREATE TABLE \"my t\" (\"a b\" INTEGER PRIMARY KEY, [c] TEXT DEFAULT 'x,y', `d` BLOB)",
        );
        assert_eq!(cols, ["a b", "c", "d"]);
        assert_eq!(alias, Some(0));

        let (_, _, wr) =
            parse_create_table("CREATE TABLE t (a INTEGER PRIMARY KEY, b) WITHOUT ROWID");
        assert!(wr);
    }

    #[test]
    fn rejects_garbage_and_wal() {
        let dir = testutil::scratch_dir("sqlite-garbage");
        let junk = dir.join("junk.calib");
        std::fs::write(&junk, vec![7u8; 5000]).unwrap();
        assert!(matches!(Db::open(&junk), Err(Error::Corrupt(_))));

        let Some(fx) = Fixture::build("sqlite-wal", "PRAGMA journal_mode=WAL; CREATE TABLE a(x);")
        else {
            return;
        };
        let err = Db::open(&fx.path).unwrap_err();
        assert!(
            matches!(err, Error::Unsupported(ref m) if m.contains("WAL")),
            "{err}"
        );
    }

    #[test]
    fn header_and_schema() {
        let Some(fx) = Fixture::chief("sqlite-schema") else {
            return;
        };
        let db = Db::open(&fx.path).unwrap();
        assert_eq!(db.page_size(), 1024);
        assert!(db.has_table("libraryobjects"));
        assert!(db.has_table("Tags4LibraryObjects"));
        let cols = db.columns("LibraryObjects").unwrap();
        assert_eq!(cols[0], "LibraryObjectId");
        assert_eq!(db.column_index("LibraryObjects", "thumbnail"), Some(19));
        assert!(db.table("NoSuchTable").is_err());
    }

    #[test]
    fn scans_multi_level_tree() {
        let Some(fx) = Fixture::chief("sqlite-bigtable") else {
            return;
        };
        let db = Db::open(&fx.path).unwrap();
        let rows: Vec<Vec<Value>> = db.table("Big").unwrap().collect::<Result<_>>().unwrap();
        assert_eq!(rows.len(), testutil::BIG_ROWS);
        let mut sum = 0i64;
        for (i, r) in rows.iter().enumerate() {
            let id = r[0].as_i64().unwrap();
            assert_eq!(id, i as i64 + 1, "rowid order / alias");
            sum += id;
            assert_eq!(r[1].as_str().unwrap(), testutil::big_text(id));
            assert_eq!(r[2].as_f64().unwrap(), id as f64 / 4.0);
            assert_eq!(r[3].blob_len().unwrap(), testutil::big_blob_len(id));
        }
        assert_eq!(
            sum,
            (testutil::BIG_ROWS as i64) * (testutil::BIG_ROWS as i64 + 1) / 2
        );

        // blobs skipped: lengths only
        let first = db
            .table("Big")
            .unwrap()
            .with_blobs(false)
            .next()
            .unwrap()
            .unwrap();
        assert!(matches!(first[3], Value::BlobLen(n) if n == testutil::big_blob_len(1)));

        // column narrowing
        let narrow = db
            .table("Big")
            .unwrap()
            .with_max_columns(2)
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(narrow.len(), 2);
    }

    #[test]
    fn rowid_lookup_and_overflow() {
        let Some(fx) = Fixture::chief("sqlite-lookup") else {
            return;
        };
        let db = Db::open(&fx.path).unwrap();
        for id in [1i64, 2, 777, 1500, testutil::BIG_ROWS as i64] {
            let t = db.read_column("Big", id, 1, None).unwrap().unwrap();
            assert_eq!(t.as_str().unwrap(), testutil::big_text(id));
            let b = db.read_column("Big", id, 3, None).unwrap().unwrap();
            assert_eq!(b.as_blob().unwrap(), testutil::big_blob(id).as_slice());
        }
        assert_eq!(db.read_column("Big", 0, 1, None).unwrap(), None);
        assert_eq!(db.read_column("Big", 99_999, 1, None).unwrap(), None);
        // prefix of an overflowing blob
        let p = db.read_column("Big", 1500, 3, Some(10)).unwrap().unwrap();
        assert_eq!(p.as_blob().unwrap(), &testutil::big_blob(1500)[..10]);
        // alias column returns the rowid
        assert_eq!(
            db.read_column("Big", 42, 0, None).unwrap(),
            Some(Value::Int(42))
        );

        // the object-3 thumbnail spans several overflow pages
        let thumb = db
            .read_column("LibraryObjects", 3, 19, None)
            .unwrap()
            .unwrap();
        assert_eq!(
            thumb.as_blob().unwrap(),
            testutil::overflow_thumbnail().as_slice()
        );
    }
}
