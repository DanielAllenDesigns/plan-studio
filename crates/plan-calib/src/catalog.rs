//! Chief Architect catalog access: [`ChiefCatalog`] opens one `.calib` or
//! `.calibz` and exposes names, categories, keywords and thumbnails.
//!
//! Table layout (see `docs/chief-library-format.md`): objects live in
//! `LibraryObjects`; their display name is `Tags4LibraryObjects.Alias`; the
//! category tree is `Tags` (via `Parent`); keywords are
//! `Keywords4LibraryObjects` joined to `Keywords`. Everything is read lazily:
//! a small index of the link tables is built the first time it is needed, and
//! blobs are fetched only on request.

use crate::decode::ObjectBlobs;
use crate::error::{Error, Result};
use crate::sqlite::{Db, RowIter, Value};
use crate::zip::CalibZ;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Largest `AssociatedData` JSON header we are willing to parse.
const MAX_JSON_BYTES: usize = 64 << 20;

/// One entry of the `Tags` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    /// `TagUniqueId`.
    pub uuid: String,
    /// Display name.
    pub name: String,
    /// UUID of the parent tag, `None` for roots.
    pub parent: Option<String>,
    /// `GroupType` (2 = category tree).
    pub group_type: i64,
    /// `Color` as stored (ARGB-style `u32`).
    pub color: u32,
}

/// A node of the category tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryNode {
    /// `TagUniqueId` (empty for a synthetic top node).
    pub uuid: String,
    /// Category name.
    pub name: String,
    /// Number of objects linked directly to this tag.
    pub direct_count: usize,
    /// Child categories, sorted by name.
    pub children: Vec<CategoryNode>,
}

impl CategoryNode {
    /// Direct child by exact name.
    pub fn child(&self, name: &str) -> Option<&CategoryNode> {
        self.children.iter().find(|c| c.name == name)
    }

    /// Objects linked to this node or any descendant (an object linked to
    /// several tags of the subtree is counted once per link).
    pub fn total_count(&self) -> usize {
        self.direct_count + self.children.iter().map(Self::total_count).sum::<usize>()
    }
}

/// Lightweight description of one library object.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectSummary {
    /// `LibraryObjectId` (rowid; also the `AssociatedData` key).
    pub library_object_id: i64,
    /// `UniqueId` UUID.
    pub unique_id: String,
    /// `LibraryObjects.Type` (meaning not decoded).
    pub type_code: i64,
    /// `Tags4LibraryObjects.Alias`, else `Legacy_Name`, else `"Object <id>"`.
    pub name: String,
    /// Category names from the catalog root down (longest chain among the
    /// object's tags).
    pub category_path: Vec<String>,
    /// Keywords, de-duplicated case-insensitively, in first-seen order.
    pub keywords: Vec<String>,
    /// Whether a thumbnail blob is present.
    pub has_thumbnail: bool,
    /// `LibraryObjects.Metric` flag.
    pub metric: bool,
}

/// Link between an object and one tag.
struct Link {
    tag: String,
    alias: Option<String>,
}

/// In-memory join of the small link tables, built once per catalog.
struct Index {
    tags: Vec<Tag>,
    tag_pos: HashMap<String, usize>,
    links: HashMap<String, Vec<Link>>,
    keywords: HashMap<i64, Vec<String>>,
}

impl Index {
    /// Names from the root down to `tag`, guarding against cycles.
    fn chain(&self, tag: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut cur = Some(tag);
        while let Some(id) = cur {
            if !seen.insert(id) {
                break;
            }
            let Some(&i) = self.tag_pos.get(id) else {
                break;
            };
            out.push(self.tags[i].name.clone());
            cur = self.tags[i].parent.as_deref();
        }
        out.reverse();
        out
    }
}

/// Column positions in `LibraryObjects`, resolved by name.
#[derive(Clone, Copy)]
struct ObjectColumns {
    id: usize,
    kind: Option<usize>,
    metric: Option<usize>,
    unique_id: Option<usize>,
    legacy_name: Option<usize>,
    thumbnail: Option<usize>,
    /// Number of leading columns that must be decoded.
    needed: usize,
}

/// An open Chief library catalog.
pub struct ChiefCatalog {
    db: Db,
    source_path: PathBuf,
    id: String,
    name: String,
    calibz: Option<CalibZ>,
    index: OnceLock<Index>,
    cols: Option<ObjectColumns>,
}

impl std::fmt::Debug for ChiefCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChiefCatalog")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("path", &self.source_path)
            .finish()
    }
}

impl ChiefCatalog {
    /// Opens a `.calib` (SQLite) or `.calibz` (zip containing a `.calib` and
    /// textures). A `.calibz` is extracted once to the system temp directory
    /// (`plan-studio/`) and reused on later opens.
    pub fn open(path: impl AsRef<Path>) -> Result<ChiefCatalog> {
        let path = path.as_ref();
        let is_zip = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("calibz"));
        let (db_path, calibz) = if is_zip {
            let cz = CalibZ::open(path)?;
            (cz.extract_calib_to_temp()?, Some(cz))
        } else {
            (path.to_owned(), None)
        };
        let db = Db::open(&db_path)?;
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "catalog".into());
        let id = read_global_id(&db).unwrap_or_else(|| name.clone());
        let cols = resolve_object_columns(&db);
        Ok(ChiefCatalog {
            db,
            source_path: path.to_owned(),
            id,
            name,
            calibz,
            index: OnceLock::new(),
            cols,
        })
    }

    /// Reads only `GlobalData.DatabaseUniqueId` of a plain `.calib`, without
    /// building anything else. `None` when the file has no such table.
    pub fn peek_id(path: impl AsRef<Path>) -> Result<Option<String>> {
        Ok(read_global_id(&Db::open(path)?))
    }

    /// Replaces the display name (used to apply the registry name).
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// `GlobalData.DatabaseUniqueId` (falls back to the file stem when absent).
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Display name: the registry name when set, else the file stem.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The path this catalog was opened from (the `.calibz` if applicable).
    pub fn path(&self) -> &Path {
        &self.source_path
    }

    /// The underlying SQLite handle, for callers that need raw tables.
    pub fn db(&self) -> &Db {
        &self.db
    }

    /// The `.calibz` wrapper when this catalog came from one.
    pub fn calibz(&self) -> Option<&CalibZ> {
        self.calibz.as_ref()
    }

    /// Reads a texture by basename from the `.calibz`, if this catalog is one
    /// and holds that file.
    pub fn texture(&self, name: &str) -> Result<Option<Vec<u8>>> {
        match &self.calibz {
            Some(cz) if cz.has_texture(name) => cz.texture(name).map(Some),
            _ => Ok(None),
        }
    }

    /// All rows of the `Tags` table (empty when the catalog has none).
    pub fn tags(&self) -> Result<Vec<Tag>> {
        if !self.db.has_table("Tags") {
            return Ok(Vec::new());
        }
        let d = &self.db;
        let col = |n: &str| d.column_index("Tags", n);
        let (uuid, name, group, parent, color) = (
            col("TagUniqueId").ok_or_else(|| Error::not_found("Tags.TagUniqueId"))?,
            col("Name").ok_or_else(|| Error::not_found("Tags.Name"))?,
            col("GroupType"),
            col("Parent"),
            col("Color"),
        );
        let mut out = Vec::new();
        for row in d.table("Tags")? {
            let row = row?;
            out.push(Tag {
                uuid: row[uuid].as_str().unwrap_or_default().to_owned(),
                name: row[name].as_str().unwrap_or_default().to_owned(),
                parent: parent.and_then(|i| row[i].non_empty_text()),
                group_type: group.and_then(|i| row[i].as_i64()).unwrap_or(0),
                color: color.and_then(|i| row[i].as_i64()).unwrap_or(0) as u32,
            });
        }
        Ok(out)
    }

    /// The category tree.
    ///
    /// Roots are tags with a NULL parent and `GroupType` 2. A Chief catalog has
    /// exactly one (its UUID equals [`id`](Self::id)); that root is returned
    /// directly. With zero or several roots a synthetic node named after the
    /// catalog is returned with the roots as children.
    pub fn category_tree(&self) -> Result<CategoryNode> {
        let idx = self.index()?;
        let mut kids: HashMap<&str, Vec<usize>> = HashMap::new();
        let mut roots = Vec::new();
        for (i, t) in idx.tags.iter().enumerate() {
            match &t.parent {
                Some(p) => kids.entry(p.as_str()).or_default().push(i),
                None if t.group_type == 2 => roots.push(i),
                None => {}
            }
        }
        let mut direct: HashMap<&str, usize> = HashMap::new();
        for links in idx.links.values() {
            for l in links {
                *direct.entry(l.tag.as_str()).or_default() += 1;
            }
        }
        fn build(
            i: usize,
            idx: &Index,
            kids: &HashMap<&str, Vec<usize>>,
            direct: &HashMap<&str, usize>,
            depth: usize,
        ) -> CategoryNode {
            let t = &idx.tags[i];
            let mut children: Vec<CategoryNode> = if depth < 64 {
                kids.get(t.uuid.as_str())
                    .map(|v| {
                        v.iter()
                            .map(|&c| build(c, idx, kids, direct, depth + 1))
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            children.sort_by_key(|c| (c.name.to_lowercase(), c.name.clone()));
            CategoryNode {
                uuid: t.uuid.clone(),
                name: t.name.clone(),
                direct_count: direct.get(t.uuid.as_str()).copied().unwrap_or(0),
                children,
            }
        }
        let mut nodes: Vec<CategoryNode> = roots
            .iter()
            .map(|&r| build(r, idx, &kids, &direct, 0))
            .collect();
        if nodes.len() == 1 {
            return Ok(nodes.remove(0));
        }
        nodes.sort_by_key(|c| (c.name.to_lowercase(), c.name.clone()));
        Ok(CategoryNode {
            uuid: String::new(),
            name: self.name.clone(),
            direct_count: 0,
            children: nodes,
        })
    }

    /// Number of rows in `LibraryObjects`.
    pub fn count(&self) -> Result<usize> {
        if !self.db.has_table("LibraryObjects") {
            return Ok(0);
        }
        let mut n = 0;
        for r in self
            .db
            .table("LibraryObjects")?
            .with_blobs(false)
            .with_max_columns(1)
        {
            r?;
            n += 1;
        }
        Ok(n)
    }

    /// Iterates the catalog's objects lazily in rowid order.
    ///
    /// The first call builds an index of the link tables (a few thousand small
    /// rows). Iteration itself reads only the leading columns of each object
    /// and never touches blobs. An I/O error mid-iteration ends the iterator;
    /// it is then available from [`Objects::error`].
    pub fn objects(&self) -> Result<Objects<'_>> {
        let index = self.index()?;
        let rows = match self.cols {
            Some(c) if self.db.has_table("LibraryObjects") => Some(
                self.db
                    .table("LibraryObjects")?
                    .with_blobs(false)
                    .with_max_columns(c.needed),
            ),
            _ => None,
        };
        Ok(Objects {
            index,
            rows,
            cols: self.cols,
            error: None,
        })
    }

    /// PNG bytes of an object's thumbnail, `None` when it has none.
    pub fn thumbnail(&self, library_object_id: i64) -> Result<Option<Vec<u8>>> {
        let Some(c) = self.cols.and_then(|c| c.thumbnail) else {
            return Ok(None);
        };
        match self
            .db
            .read_column("LibraryObjects", library_object_id, c, None)?
        {
            Some(Value::Blob(b)) if !b.is_empty() => Ok(Some(b)),
            _ => Ok(None),
        }
    }

    /// The JSON header of an object's `AssociatedData` blob, when present.
    ///
    /// Per the format notes the blob starts with 12 flag bytes, a little-endian
    /// u32 JSON length, then the JSON text at offset 16. Only that prefix is
    /// read. `None` when the row is missing, empty, or not in that layout.
    pub fn associated_json(&self, library_object_id: i64) -> Result<Option<serde_json::Value>> {
        let table = "AssociatedData";
        if !self.db.has_table(table) {
            return Ok(None);
        }
        let Some(col) = self.db.column_index(table, "AssociatedDataBlob") else {
            return Ok(None);
        };
        let Some(Value::Blob(head)) =
            self.db
                .read_column(table, library_object_id, col, Some(17))?
        else {
            return Ok(None);
        };
        if head.len() < 17 || (head[16] != b'{' && head[16] != b'[') {
            return Ok(None);
        }
        let len = u32::from_le_bytes([head[12], head[13], head[14], head[15]]) as usize;
        if len == 0 || len > MAX_JSON_BYTES {
            return Ok(None);
        }
        let Some(Value::Blob(full)) =
            self.db
                .read_column(table, library_object_id, col, Some(16 + len))?
        else {
            return Ok(None);
        };
        if full.len() < 16 + len {
            return Ok(None);
        }
        Ok(serde_json::from_slice(&full[16..16 + len]).ok())
    }

    /// Reads one blob column of one row as owned bytes; `None` when the table,
    /// column or row is missing or the value is NULL or empty.
    fn blob(&self, table: &str, column: &str, rowid: i64) -> Result<Option<Vec<u8>>> {
        if !self.db.has_table(table) {
            return Ok(None);
        }
        let Some(col) = self.db.column_index(table, column) else {
            return Ok(None);
        };
        match self.db.read_column(table, rowid, col, None)? {
            Some(Value::Blob(b)) if !b.is_empty() => Ok(Some(b)),
            _ => Ok(None),
        }
    }

    /// The raw binary blobs of an object: `Data`, the `AssociatedData` blob,
    /// `SymbolData`, and `symDxf` / `symBlock` of its `LibrarySymbolData` row.
    ///
    /// Blobs can be several megabytes (the largest `AssociatedData` row of
    /// Core Architectural is 3.3 MB), so call this per object, not per catalog.
    /// Missing tables, rows and NULL values give `None` fields, not errors.
    pub fn object_blobs(&self, library_object_id: i64) -> Result<ObjectBlobs> {
        let mut out = ObjectBlobs {
            data: self.blob("Data4LibraryObjects", "Data", library_object_id)?,
            associated: self.blob("AssociatedData", "AssociatedDataBlob", library_object_id)?,
            symbol_data: self.blob("SymbolData4LibraryObjects", "SymbolData", library_object_id)?,
            ..Default::default()
        };
        let objects = "LibraryObjects";
        if let Some(col) = self.db.column_index(objects, "LibSymDataId") {
            if let Some(Value::Int(sym_id)) =
                self.db
                    .read_column(objects, library_object_id, col, Some(0))?
            {
                out.sym_dxf = self.blob("LibrarySymbolData", "symDxf", sym_id)?;
                out.sym_block = self.blob("LibrarySymbolData", "symBlock", sym_id)?;
            }
        }
        Ok(out)
    }

    fn index(&self) -> Result<&Index> {
        if let Some(i) = self.index.get() {
            return Ok(i);
        }
        let built = self.build_index()?;
        Ok(self.index.get_or_init(|| built))
    }

    fn build_index(&self) -> Result<Index> {
        let tags = self.tags()?;
        let tag_pos = tags
            .iter()
            .enumerate()
            .map(|(i, t)| (t.uuid.clone(), i))
            .collect();

        let mut links: HashMap<String, Vec<Link>> = HashMap::new();
        let t4 = "Tags4LibraryObjects";
        if self.db.has_table(t4) {
            let c = |n: &str| self.db.column_index(t4, n);
            if let (Some(o), Some(t)) = (c("LibraryObjectUniqueId"), c("TagUniqueId")) {
                let a = c("Alias");
                let need = [o, t, a.unwrap_or(0)].into_iter().max().unwrap_or(0) + 1;
                for row in self.db.table(t4)?.with_blobs(false).with_max_columns(need) {
                    let row = row?;
                    let (Some(obj), Some(tag)) = (row[o].as_str(), row[t].as_str()) else {
                        continue;
                    };
                    links.entry(obj.to_owned()).or_default().push(Link {
                        tag: tag.to_owned(),
                        alias: a.and_then(|i| row[i].non_empty_text()),
                    });
                }
            }
        }

        let mut keywords: HashMap<i64, Vec<String>> = HashMap::new();
        let (k, k4) = ("Keywords", "Keywords4LibraryObjects");
        if self.db.has_table(k) && self.db.has_table(k4) {
            let (kid, kw) = (
                self.db.column_index(k, "KeywordId"),
                self.db.column_index(k, "Keyword"),
            );
            let (lo, lk) = (
                self.db.column_index(k4, "LibraryObjectId"),
                self.db.column_index(k4, "KeywordId"),
            );
            if let (Some(kid), Some(kw), Some(lo), Some(lk)) = (kid, kw, lo, lk) {
                let mut words: HashMap<i64, String> = HashMap::new();
                for row in self.db.table(k)?.with_blobs(false) {
                    let row = row?;
                    if let (Some(i), Some(w)) = (row[kid].as_i64(), row[kw].as_str()) {
                        words.insert(i, w.to_owned());
                    }
                }
                let mut seen: HashSet<(i64, String)> = HashSet::new();
                for row in self.db.table(k4)?.with_blobs(false) {
                    let row = row?;
                    let (Some(obj), Some(kwid)) = (row[lo].as_i64(), row[lk].as_i64()) else {
                        continue;
                    };
                    if let Some(w) = words.get(&kwid) {
                        if seen.insert((obj, w.to_lowercase())) {
                            keywords.entry(obj).or_default().push(w.clone());
                        }
                    }
                }
            }
        }
        Ok(Index {
            tags,
            tag_pos,
            links,
            keywords,
        })
    }
}

/// Lazy iterator over a catalog's objects. See [`ChiefCatalog::objects`].
pub struct Objects<'a> {
    index: &'a Index,
    rows: Option<RowIter<'a>>,
    cols: Option<ObjectColumns>,
    error: Option<Error>,
}

impl Objects<'_> {
    /// The error that stopped iteration early, if any.
    pub fn error(&self) -> Option<&Error> {
        self.error.as_ref()
    }

    /// Takes the error that stopped iteration early, if any.
    pub fn take_error(&mut self) -> Option<Error> {
        self.error.take()
    }
}

impl Iterator for Objects<'_> {
    type Item = ObjectSummary;

    fn next(&mut self) -> Option<ObjectSummary> {
        let rows = self.rows.as_mut()?;
        let cols = self.cols?;
        let row = match rows.next()? {
            Ok(r) => r,
            Err(e) => {
                self.error = Some(e);
                self.rows = None;
                return None;
            }
        };
        let id = row[cols.id].as_i64().unwrap_or(0);
        let get_text =
            |c: Option<usize>| c.and_then(|i| row.get(i)).and_then(Value::non_empty_text);
        let unique_id = get_text(cols.unique_id).unwrap_or_default();
        let links = self.index.links.get(&unique_id);

        let alias = links.and_then(|ls| ls.iter().find_map(|l| l.alias.clone()));
        let name = alias
            .or_else(|| get_text(cols.legacy_name))
            .unwrap_or_else(|| format!("Object {id}"));
        let category_path = links
            .map(|ls| {
                let mut best: Vec<String> = Vec::new();
                for l in ls {
                    let c = self.index.chain(&l.tag);
                    if c.len() > best.len() {
                        best = c;
                    }
                }
                best
            })
            .unwrap_or_default();
        Some(ObjectSummary {
            library_object_id: id,
            unique_id,
            type_code: cols
                .kind
                .and_then(|i| row.get(i))
                .and_then(Value::as_i64)
                .unwrap_or(0),
            name,
            category_path,
            keywords: self.index.keywords.get(&id).cloned().unwrap_or_default(),
            has_thumbnail: cols
                .thumbnail
                .and_then(|i| row.get(i))
                .and_then(Value::blob_len)
                .is_some_and(|n| n > 0),
            metric: cols
                .metric
                .and_then(|i| row.get(i))
                .and_then(Value::as_i64)
                .unwrap_or(0)
                != 0,
        })
    }
}

fn read_global_id(db: &Db) -> Option<String> {
    if !db.has_table("GlobalData") {
        return None;
    }
    let col = db.column_index("GlobalData", "DatabaseUniqueId")?;
    db.table("GlobalData")
        .ok()?
        .with_blobs(false)
        .find_map(|r| {
            r.ok()
                .and_then(|row| row.get(col).and_then(Value::non_empty_text))
        })
}

fn resolve_object_columns(db: &Db) -> Option<ObjectColumns> {
    let t = "LibraryObjects";
    let id = db.column_index(t, "LibraryObjectId")?;
    let c = |n: &str| db.column_index(t, n);
    let mut cols = ObjectColumns {
        id,
        kind: c("Type"),
        metric: c("Metric"),
        unique_id: c("UniqueId"),
        legacy_name: c("Legacy_Name"),
        thumbnail: c("Thumbnail"),
        needed: 0,
    };
    cols.needed = [
        Some(cols.id),
        cols.kind,
        cols.metric,
        cols.unique_id,
        cols.legacy_name,
        cols.thumbnail,
    ]
    .into_iter()
    .flatten()
    .max()
    .unwrap_or(0)
        + 1;
    Some(cols)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{self, Fixture};

    #[test]
    fn fixture_catalog_end_to_end() {
        let Some(fx) = Fixture::chief("catalog-e2e") else {
            return;
        };
        let cat = ChiefCatalog::open(&fx.path).unwrap();
        assert_eq!(cat.id(), testutil::ROOT_UUID);
        assert_eq!(cat.name(), "catalog-e2e");
        assert_eq!(cat.count().unwrap(), 3);

        let tags = cat.tags().unwrap();
        assert_eq!(tags.len(), 3);
        let root = tags.iter().find(|t| t.parent.is_none()).unwrap();
        assert_eq!((root.name.as_str(), root.group_type), ("Fixtures", 2));
        assert_eq!(root.color, 0xffff_ffff);
        let entry = tags.iter().find(|t| t.name == "Entry Doors").unwrap();
        assert_eq!(entry.parent.as_deref(), Some(testutil::DOORS_UUID));

        let tree = cat.category_tree().unwrap();
        assert_eq!(tree.name, "Fixtures");
        assert_eq!(tree.children.len(), 1);
        let doors = tree.child("Doors").unwrap();
        assert_eq!(doors.direct_count, 1);
        assert_eq!(doors.child("Entry Doors").unwrap().direct_count, 1);
        assert_eq!(tree.total_count(), 3);

        let objs: Vec<ObjectSummary> = cat.objects().unwrap().collect();
        assert_eq!(objs.len(), 3);

        assert_eq!(objs[0].library_object_id, 1);
        assert_eq!(objs[0].name, "Door One");
        assert_eq!(objs[0].category_path, ["Fixtures", "Doors", "Entry Doors"]);
        assert_eq!(
            objs[0].keywords,
            ["Oak", "Front"],
            "case-insensitive dedupe"
        );
        assert!(objs[0].has_thumbnail);
        assert!(!objs[0].metric);
        assert_eq!(objs[0].type_code, 7);

        assert_eq!(objs[1].name, "Legacy Thing", "falls back to Legacy_Name");
        assert_eq!(objs[1].category_path, ["Fixtures", "Doors"]);
        assert_eq!(objs[1].keywords, ["Front"]);
        assert!(!objs[1].has_thumbnail);
        assert!(objs[1].metric);

        assert_eq!(objs[2].name, "Object 3", "falls back to the id");
        assert_eq!(objs[2].category_path, ["Fixtures"]);
        assert!(objs[2].has_thumbnail);

        let t1 = cat.thumbnail(1).unwrap().unwrap();
        assert_eq!(&t1[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(t1, testutil::small_png());
        assert_eq!(cat.thumbnail(2).unwrap(), None);
        assert_eq!(
            cat.thumbnail(3).unwrap().unwrap(),
            testutil::overflow_thumbnail(),
            "overflow chain"
        );
        assert_eq!(cat.thumbnail(99).unwrap(), None);
    }

    #[test]
    fn associated_json_roundtrip() {
        let Some(fx) = Fixture::chief("catalog-json") else {
            return;
        };
        let cat = ChiefCatalog::open(&fx.path).unwrap();
        let j = cat.associated_json(1).unwrap().unwrap();
        assert_eq!(j["Version"], 4);
        assert_eq!(j["Width"], 36.5);
        assert!(cat.associated_json(2).unwrap().is_none(), "NULL blob");
        assert!(cat.associated_json(3).unwrap().is_none(), "not JSON");
        assert!(cat.associated_json(404).unwrap().is_none());
    }

    #[test]
    fn peek_id_and_with_name() {
        let Some(fx) = Fixture::chief("catalog-peek") else {
            return;
        };
        assert_eq!(
            ChiefCatalog::peek_id(&fx.path).unwrap().as_deref(),
            Some(testutil::ROOT_UUID)
        );
        let cat = ChiefCatalog::open(&fx.path).unwrap().with_name("Pretty");
        assert_eq!(cat.name(), "Pretty");
        assert!(cat.texture("x.jpg").unwrap().is_none());
    }

    #[test]
    fn missing_tables_are_tolerated() {
        let Some(fx) = Fixture::build(
            "catalog-bare",
            "CREATE TABLE LibraryObjects (LibraryObjectId INTEGER PRIMARY KEY, UniqueId TEXT);\
             INSERT INTO LibraryObjects VALUES (5, 'u-5');",
        ) else {
            return;
        };
        let cat = ChiefCatalog::open(&fx.path).unwrap();
        assert_eq!(cat.id(), "catalog-bare");
        let o: Vec<_> = cat.objects().unwrap().collect();
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].name, "Object 5");
        assert!(o[0].category_path.is_empty());
        assert_eq!(cat.tags().unwrap().len(), 0);
        assert_eq!(cat.category_tree().unwrap().children.len(), 0);
        assert_eq!(cat.thumbnail(5).unwrap(), None);
    }
}
