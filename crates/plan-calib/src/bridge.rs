//! Bridge from a [`ChiefCatalog`] to Plan Studio's own catalog types.
//!
//! Size and plan symbol come from [`crate::decode`]:
//!
//! * the size is the `w d h 1 1 1` record of the object's `Data` blob, or, when
//!   it has none, the bounds of its decoded geometry;
//! * the plan symbol is the top-down hidden-line view of that geometry
//!   (`symDxf` faces and `CD AB 74 00` meshes), used only when its footprint is
//!   within 10% of the size;
//! * otherwise the item gets a placeholder symbol (its footprint rectangle, an
//!   X, and the first letter of its name drawn with a tiny stroke font).
//!
//! When no blob gives a size, `AssociatedData` JSON width/depth/height keys are
//! tried (none of the sampled X18 catalogs have them) and then a 24 x 24 x 24 in
//! cube is used and the item is tagged `size-unknown`.

use crate::catalog::{ChiefCatalog, ObjectSummary};
use crate::decode::{self, DecodedObject};
use crate::error::Result;
use plan_core::geometry::Point;
use plan_library::{Catalog, CatalogItem, Placement, Stroke, Symbol2d};
use serde_json::Value as Json;
use std::collections::HashMap;

/// Edge length (inches) used when no size can be found.
pub const DEFAULT_SIZE: f64 = 24.0;
/// Tag added to items whose size is a guess.
pub const SIZE_UNKNOWN_TAG: &str = "size-unknown";

/// How much of a conversion came from decoded Chief data.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BridgeStats {
    /// Items whose plan symbol was decoded from geometry (not a placeholder).
    pub decoded_symbols: usize,
    /// Items whose size was decoded from blobs (`Data` record or geometry
    /// bounds), as opposed to the JSON keys or the 24 in default.
    pub decoded_sizes: usize,
    /// Items converted.
    pub total: usize,
}

/// The converted catalog plus the thumbnails `CatalogItem` has no field for.
#[derive(Debug, Clone, Default)]
pub struct BridgeResult {
    /// Plan Studio catalog; item ids are `chief.<catalog-uuid>.<object id>`.
    pub catalog: Catalog,
    /// PNG thumbnails keyed by item id.
    pub thumbnails: HashMap<String, Vec<u8>>,
    /// Decode coverage of this conversion.
    pub stats: BridgeStats,
}

/// Converts up to `limit` objects of `cat` (all when `None`).
///
/// Thumbnails and the geometry blobs (up to a few MB per object) are read for
/// every converted object, so for large catalogs pass a limit or convert in
/// pages.
pub fn to_plan_library(cat: &ChiefCatalog, limit: Option<usize>) -> Result<BridgeResult> {
    let mut out = BridgeResult {
        catalog: Catalog::new(cat.name()),
        ..Default::default()
    };
    let take = limit.unwrap_or(usize::MAX);
    let mut objects = cat.objects()?;
    for obj in objects.by_ref().take(take) {
        let json = cat.associated_json(obj.library_object_id)?;
        let decoded = decode::decode_object(&cat.object_blobs(obj.library_object_id)?);
        let item = item_from(cat, &obj, json.as_ref(), &decoded, &mut out.stats);
        if obj.has_thumbnail {
            if let Some(png) = cat.thumbnail(obj.library_object_id)? {
                out.thumbnails.insert(item.id.clone(), png);
            }
        }
        out.catalog.items.push(item);
    }
    if let Some(e) = objects.take_error() {
        return Err(e);
    }
    Ok(out)
}

fn item_from(
    cat: &ChiefCatalog,
    obj: &ObjectSummary,
    json: Option<&Json>,
    decoded: &DecodedObject,
    stats: &mut BridgeStats,
) -> CatalogItem {
    stats.total += 1;
    let round = |v: f64| (v * 1000.0).round() / 1000.0;
    let decoded_size = decoded.size.filter(|s| s.width > 0.0 && s.depth > 0.0);
    let json_dims = json.and_then(find_dims);
    let (w, d, h, elevation, size_known) = match (decoded_size, json_dims) {
        (Some(s), _) => {
            stats.decoded_sizes += 1;
            (
                round(s.width),
                round(s.depth),
                round(s.height),
                s.elevation,
                true,
            )
        }
        (None, Some((w, d, h))) => (w, d, h, 0.0, true),
        (None, None) => (DEFAULT_SIZE, DEFAULT_SIZE, DEFAULT_SIZE, 0.0, false),
    };

    let mut tags: Vec<String> = obj.keywords.clone();
    tags.push("chief".into());
    if !size_known {
        tags.push(SIZE_UNKNOWN_TAG.into());
    }
    let category = if obj.category_path.is_empty() {
        vec![cat.name().to_owned()]
    } else {
        obj.category_path.clone()
    };
    let symbol = match &decoded.symbol {
        Some(sym) if decoded.symbol_fits_size != Some(false) => {
            stats.decoded_symbols += 1;
            Symbol2d::new(sym.strokes.clone())
        }
        _ => placeholder_symbol(&obj.name, w, d),
    };
    let mut item = CatalogItem::new(
        format!("chief.{}.{}", cat.id(), obj.library_object_id),
        obj.name.clone(),
        Placement::FreeStanding,
        symbol,
    );
    item.category = category;
    item.width = w;
    item.depth = d;
    item.height = h;
    item.elevation = elevation;
    item.tags = tags;
    item
}

/// Looks for width/depth/height anywhere in `json`.
///
/// Keys are matched case-insensitively (`Width`, `Depth`, `Height`). A `Size`
/// entry may be an object with those keys or an array `[width, depth, height]`.
/// Width and depth are required; a missing height becomes [`DEFAULT_SIZE`].
/// Values are taken as inches (the unit is unconfirmed; see the README).
pub fn find_dims(json: &Json) -> Option<(f64, f64, f64)> {
    #[derive(Default)]
    struct Found {
        w: Option<f64>,
        d: Option<f64>,
        h: Option<f64>,
    }
    fn num(v: &Json) -> Option<f64> {
        let n = match v {
            Json::Number(n) => n.as_f64()?,
            Json::String(s) => s.trim().parse().ok()?,
            _ => return None,
        };
        (n.is_finite() && n > 0.0).then_some(n)
    }
    fn walk(v: &Json, f: &mut Found, depth: usize) {
        if depth > 8 {
            return;
        }
        match v {
            Json::Object(m) => {
                for (k, val) in m {
                    match k.to_ascii_lowercase().as_str() {
                        "width" => f.w = f.w.or_else(|| num(val)),
                        "depth" => f.d = f.d.or_else(|| num(val)),
                        "height" => f.h = f.h.or_else(|| num(val)),
                        "size" => {
                            if let Json::Array(a) = val {
                                if a.len() >= 2 {
                                    f.w = f.w.or_else(|| num(&a[0]));
                                    f.d = f.d.or_else(|| num(&a[1]));
                                    if let Some(h) = a.get(2) {
                                        f.h = f.h.or_else(|| num(h));
                                    }
                                }
                            } else {
                                walk(val, f, depth + 1);
                            }
                        }
                        _ => walk(val, f, depth + 1),
                    }
                }
            }
            Json::Array(a) => a.iter().for_each(|x| walk(x, f, depth + 1)),
            _ => {}
        }
    }
    let mut f = Found::default();
    walk(json, &mut f, 0);
    Some((f.w?, f.d?, f.h.unwrap_or(DEFAULT_SIZE)))
}

/// Footprint rectangle, an X, and the first letter of `name`, centred on the
/// origin.
pub fn placeholder_symbol(name: &str, width: f64, depth: f64) -> Symbol2d {
    let (hw, hd) = (width / 2.0, depth / 2.0);
    let p = Point::new;
    let mut strokes = vec![
        Stroke::Polyline {
            points: vec![p(-hw, -hd), p(hw, -hd), p(hw, hd), p(-hw, hd)],
            closed: true,
        },
        Stroke::Polyline {
            points: vec![p(-hw, -hd), p(hw, hd)],
            closed: false,
        },
        Stroke::Polyline {
            points: vec![p(-hw, hd), p(hw, -hd)],
            closed: false,
        },
    ];

    let ch = name
        .chars()
        .find(|c| c.is_ascii_alphanumeric())
        .map_or('?', |c| c.to_ascii_uppercase());
    // The glyph grid is 2 wide by 4 tall; scale it to ~40% of the short side.
    let scale = (width.min(depth) * 0.4 / 4.0).max(0.1);
    for line in glyph(ch) {
        strokes.push(Stroke::Polyline {
            points: line
                .iter()
                .map(|&(x, y)| p((f64::from(x) - 1.0) * scale, (f64::from(y) - 2.0) * scale))
                .collect(),
            closed: false,
        });
    }
    Symbol2d::new(strokes)
}

/// Polylines of a 3 x 5 point stroke font (x in 0..=2, y in 0..=4, y up).
fn glyph(c: char) -> &'static [&'static [(u8, u8)]] {
    match c {
        'A' => &[&[(0, 0), (0, 3), (1, 4), (2, 3), (2, 0)], &[(0, 2), (2, 2)]],
        'B' => &[
            &[(0, 0), (0, 4), (1, 4), (2, 3), (1, 2), (0, 2)],
            &[(1, 2), (2, 1), (1, 0), (0, 0)],
        ],
        'C' => &[&[(2, 4), (0, 4), (0, 0), (2, 0)]],
        'D' => &[&[(0, 0), (0, 4), (1, 4), (2, 3), (2, 1), (1, 0), (0, 0)]],
        'E' => &[&[(2, 4), (0, 4), (0, 0), (2, 0)], &[(0, 2), (1, 2)]],
        'F' => &[&[(2, 4), (0, 4), (0, 0)], &[(0, 2), (1, 2)]],
        'G' => &[&[(2, 4), (0, 4), (0, 0), (2, 0), (2, 2), (1, 2)]],
        'H' => &[&[(0, 0), (0, 4)], &[(2, 0), (2, 4)], &[(0, 2), (2, 2)]],
        'I' => &[&[(0, 4), (2, 4)], &[(1, 4), (1, 0)], &[(0, 0), (2, 0)]],
        'J' => &[&[(0, 1), (1, 0), (2, 0), (2, 4)]],
        'K' => &[&[(0, 0), (0, 4)], &[(2, 4), (0, 2), (2, 0)]],
        'L' => &[&[(0, 4), (0, 0), (2, 0)]],
        'M' => &[&[(0, 0), (0, 4), (1, 2), (2, 4), (2, 0)]],
        'N' => &[&[(0, 0), (0, 4), (2, 0), (2, 4)]],
        'O' => &[&[(0, 0), (0, 4), (2, 4), (2, 0), (0, 0)]],
        'P' => &[&[(0, 0), (0, 4), (2, 4), (2, 2), (0, 2)]],
        'Q' => &[&[(0, 0), (0, 4), (2, 4), (2, 0), (0, 0)], &[(1, 1), (2, 0)]],
        'R' => &[&[(0, 0), (0, 4), (2, 4), (2, 2), (0, 2)], &[(1, 2), (2, 0)]],
        'S' => &[&[(2, 4), (0, 4), (0, 2), (2, 2), (2, 0), (0, 0)]],
        'T' => &[&[(0, 4), (2, 4)], &[(1, 4), (1, 0)]],
        'U' => &[&[(0, 4), (0, 0), (2, 0), (2, 4)]],
        'V' => &[&[(0, 4), (1, 0), (2, 4)]],
        'W' => &[&[(0, 4), (0, 0), (1, 2), (2, 0), (2, 4)]],
        'X' => &[&[(0, 0), (2, 4)], &[(0, 4), (2, 0)]],
        'Y' => &[&[(0, 4), (1, 2), (2, 4)], &[(1, 2), (1, 0)]],
        'Z' => &[&[(0, 4), (2, 4), (0, 0), (2, 0)]],
        '0' => &[&[(0, 0), (0, 4), (2, 4), (2, 0), (0, 0)], &[(0, 0), (2, 4)]],
        '1' => &[&[(0, 3), (1, 4), (1, 0)], &[(0, 0), (2, 0)]],
        '2' => &[&[(0, 4), (2, 4), (2, 2), (0, 2), (0, 0), (2, 0)]],
        '3' => &[&[(0, 4), (2, 4), (2, 0), (0, 0)], &[(0, 2), (2, 2)]],
        '4' => &[&[(0, 4), (0, 2), (2, 2)], &[(2, 4), (2, 0)]],
        '5' => &[&[(2, 4), (0, 4), (0, 2), (2, 2), (2, 0), (0, 0)]],
        '6' => &[&[(2, 4), (0, 4), (0, 0), (2, 0), (2, 2), (0, 2)]],
        '7' => &[&[(0, 4), (2, 4), (1, 0)]],
        '8' => &[&[(0, 0), (0, 4), (2, 4), (2, 0), (0, 0)], &[(0, 2), (2, 2)]],
        '9' => &[&[(2, 0), (2, 4), (0, 4), (0, 2), (2, 2)]],
        _ => &[&[(0, 3), (0, 4), (2, 4), (2, 2), (1, 2), (1, 1)]],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{self, Fixture};
    use serde_json::json;

    #[test]
    fn finds_sizes_in_varied_shapes() {
        assert_eq!(
            find_dims(&json!({"Width": 36, "Depth": 24.5, "Height": 84})),
            Some((36.0, 24.5, 84.0))
        );
        assert_eq!(
            find_dims(&json!({"Macros": [{"params": {"width": "30", "DEPTH": 18}}]})),
            Some((30.0, 18.0, DEFAULT_SIZE))
        );
        assert_eq!(
            find_dims(&json!({"Size": [10, 20, 30]})),
            Some((10.0, 20.0, 30.0))
        );
        assert_eq!(
            find_dims(&json!({"Size": {"Width": 5, "Depth": 6, "Height": 7}})),
            Some((5.0, 6.0, 7.0))
        );
        assert_eq!(find_dims(&json!({"Version": 4, "Macros": []})), None);
        assert_eq!(find_dims(&json!({"Width": 5})), None, "depth is required");
        assert_eq!(find_dims(&json!({"Width": 0, "Depth": -3})), None);
    }

    #[test]
    fn placeholder_has_rect_x_and_letter() {
        let s = placeholder_symbol("door e29", 36.0, 24.0);
        // rectangle + 2 diagonals + 'D' (one polyline)
        assert_eq!(s.strokes.len(), 4);
        let b = s.bounds().unwrap();
        assert!((b.width() - 36.0).abs() < 1e-9 && (b.height() - 24.0).abs() < 1e-9);
        // 'H' has three strokes; digits and fallbacks work too.
        assert_eq!(placeholder_symbol("Hinge", 10.0, 10.0).strokes.len(), 6);
        assert!(!placeholder_symbol("***", 10.0, 10.0).is_empty());
        assert_eq!(placeholder_symbol("7-Up", 10.0, 10.0).strokes.len(), 4);
    }

    #[test]
    fn converts_the_fixture() {
        let Some(fx) = Fixture::chief("bridge-fixture") else {
            return;
        };
        let cat = ChiefCatalog::open(&fx.path).unwrap();
        let r = to_plan_library(&cat, None).unwrap();
        assert_eq!(r.catalog.name, "bridge-fixture");
        assert_eq!(r.catalog.items.len(), 3);

        let a = &r.catalog.items[0];
        assert_eq!(a.id, format!("chief.{}.1", testutil::ROOT_UUID));
        assert_eq!(a.name, "Door One");
        assert_eq!(a.category, ["Fixtures", "Doors", "Entry Doors"]);
        assert_eq!(a.placement, Placement::FreeStanding);
        // Width found in AssociatedData but depth is not, so size is a guess.
        assert_eq!((a.width, a.depth, a.height), (24.0, 24.0, 24.0));
        assert!(a.tags.contains(&SIZE_UNKNOWN_TAG.to_string()));
        assert!(a.tags.contains(&"Oak".to_string()));
        assert!(!a.symbol.is_empty());

        assert_eq!(r.thumbnails.len(), 2);
        assert_eq!(r.thumbnails[&a.id], testutil::small_png());

        let limited = to_plan_library(&cat, Some(1)).unwrap();
        assert_eq!(limited.catalog.items.len(), 1);

        // Round-trips through plan-library's own JSON format.
        let json = r.catalog.to_json().unwrap();
        assert_eq!(Catalog::from_json(&json).unwrap(), r.catalog);
    }

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    #[test]
    fn uses_decoded_sizes_and_symbols() {
        use crate::decode::testdata::{box_mesh, tri_mesh};
        let size_record = |w: f32, d: f32, h: f32| {
            let mut b = vec![0u8; 16];
            b.extend_from_slice(&[0xFF; 4]);
            b.push(1);
            for v in [w, d, h, 1.0, 1.0, 1.0] {
                b.extend_from_slice(&v.to_le_bytes());
            }
            b
        };
        let (v, t) = box_mesh(36.0, 24.0, 34.0);
        let mut assoc = vec![0u8; 12];
        assoc.extend_from_slice(&2u32.to_le_bytes());
        assoc.extend_from_slice(b"{}");
        assoc.extend(tri_mesh(&v, &t));
        let sql = format!(
            "CREATE TABLE LibraryObjects (LibraryObjectId INTEGER PRIMARY KEY, UniqueId TEXT);\
             CREATE TABLE Data4LibraryObjects (LibraryObjectId INTEGER PRIMARY KEY, Data BLOB);\
             CREATE TABLE AssociatedData (AssociatedDataId INTEGER PRIMARY KEY, AssociatedDataBlob BLOB);\
             INSERT INTO LibraryObjects VALUES (1,'u1'),(2,'u2'),(3,'u3'),(4,'u4');\
             INSERT INTO Data4LibraryObjects VALUES (1, X'{}'),(2, X'{}'),(3, X'{}');\
             INSERT INTO AssociatedData VALUES (1, X'{}'),(4, X'{}');",
            hex(&size_record(36.0, 24.0, 34.0)),
            hex(&size_record(50.0, 20.0, 30.0)),
            hex(b"nothing useful"),
            hex(&assoc),
            hex(&assoc),
        );
        let Some(fx) = Fixture::build("bridge-decoded", &sql) else {
            return;
        };
        let cat = ChiefCatalog::open(&fx.path).unwrap();
        let r = to_plan_library(&cat, None).unwrap();
        assert_eq!(
            r.stats,
            BridgeStats {
                decoded_symbols: 2,
                decoded_sizes: 3,
                total: 4
            }
        );
        let items = &r.catalog.items;
        // 1: Data size and a matching mesh: decoded symbol.
        assert_eq!(
            (items[0].width, items[0].depth, items[0].height),
            (36.0, 24.0, 34.0)
        );
        assert!(!items[0].tags.contains(&SIZE_UNKNOWN_TAG.to_string()));
        let b = items[0].symbol.bounds().unwrap();
        assert!((b.width() - 36.0).abs() < 0.2 && (b.height() - 24.0).abs() < 0.2);
        assert_ne!(items[0].symbol, placeholder_symbol("x", 36.0, 24.0));
        // 2: Data size but no geometry: placeholder at the decoded size.
        assert_eq!((items[1].width, items[1].depth), (50.0, 20.0));
        assert_eq!(
            items[1].symbol,
            placeholder_symbol(&items[1].name, 50.0, 20.0)
        );
        // 3: nothing decodable: default size, tagged.
        assert_eq!(items[2].width, DEFAULT_SIZE);
        assert!(items[2].tags.contains(&SIZE_UNKNOWN_TAG.to_string()));
        // 4: geometry only: size from the mesh bounds.
        assert_eq!(
            (items[3].width, items[3].depth, items[3].height),
            (36.0, 24.0, 34.0)
        );
        assert!(!items[3].tags.contains(&SIZE_UNKNOWN_TAG.to_string()));
    }
}
