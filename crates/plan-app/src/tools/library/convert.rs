//! Convert to Symbol and Replace From Library for the selection (CB-57,
//! CB-58).
//!
//! * **Convert to Symbol** turns the selected 3D solids (boxes, polyline
//!   solids, cylinders, cones, spheres, pyramids) into one User Catalog symbol:
//!   its 3D model is the solids' meshes, its plan drawing the projection of that
//!   model, its size the solids' bounding box. The solids are replaced in the
//!   plan by a placed copy of the new symbol, in one undo step.
//! * **Replace From Library** swaps every selected library symbol for the
//!   active Library Browser item, keeping position, angle, reflect, label,
//!   layer and elevation; the size is kept too unless "Keep size" is off, when
//!   the new item's own size is used. Cabinets and devices (one at a time) go
//!   through [`super::user::replace_other`]. One undo step for the whole
//!   selection.

use super::{make, user};
use crate::editor::{details_view, EditorContext, ObjectRef};
use plan_core::details::Solid3d;
use plan_core::geometry::Point;
use plan_core::{Id, PlacedSymbol};
use plan_library::{CatalogItem, ItemKind, Placement};
use std::cell::Cell;
use std::sync::Arc;

/// Command id of Library > Convert to Symbol.
pub const CONVERT_TO_SYMBOL: &str = "library.convert_to_symbol";
/// Command id of Library > Replace From Library.
pub const REPLACE_FROM_LIBRARY: &str = "library.replace_from_library";

thread_local! {
    static KEEP_SIZE: Cell<bool> = const { Cell::new(true) };
}

/// Replace From Library keeps the replaced symbol's size (default) instead of
/// taking the new item's.
pub fn keep_size() -> bool {
    KEEP_SIZE.with(Cell::get)
}

pub fn set_keep_size(on: bool) {
    KEEP_SIZE.with(|k| k.set(on));
}

/// What Convert to Symbol made.
#[derive(Debug, Clone)]
pub struct Converted {
    /// The new User Catalog item.
    pub item: Arc<CatalogItem>,
    /// The placed copy that took the solids' place.
    pub symbol: Id,
    /// How many solids were converted.
    pub solids: usize,
}

/// The selected 3D solids of the active floor.
pub fn selected_solids(cx: &EditorContext) -> Vec<Solid3d> {
    let layer = details_view::load(cx);
    cx.selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Detail(id) => layer.solids.iter().find(|s| s.id == *id).cloned(),
            _ => None,
        })
        .collect()
}

/// Is a 3D solid selected (so Convert to Symbol applies)?
pub fn can_convert(cx: &EditorContext) -> bool {
    cx.selection
        .items
        .iter()
        .any(|o| matches!(o, ObjectRef::Detail(_)))
        && !selected_solids(cx).is_empty()
}

/// Converts the selected 3D solids to a User Catalog symbol named `name`
/// (`Solid Symbol n` when `None`) and puts a placed copy where they were.
pub fn convert_to_symbol(cx: &mut EditorContext, name: Option<&str>) -> Result<Converted, String> {
    let solids = selected_solids(cx);
    if solids.is_empty() {
        return Err(
            "Select 3D solids (a box, a polyline solid, a cylinder ...) to convert to a symbol"
                .into(),
        );
    }
    let (mut lo, mut hi) = (
        Point::new(f64::MAX, f64::MAX),
        Point::new(f64::MIN, f64::MIN),
    );
    let (mut bottom, mut top) = (f64::MAX, f64::MIN);
    for s in &solids {
        for p in s.footprint() {
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        bottom = bottom.min(s.elevation);
        top = top.max(s.elevation + s.height());
    }
    let centre = Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5);
    let (w, d, h) = (hi.x - lo.x, hi.y - lo.y, top - bottom);
    if w < 0.1 || d < 0.1 || h < 0.1 {
        return Err("The selected solids are flat: give them a height first".into());
    }
    // The solids' meshes around the origin, on the floor.
    let meshes: Vec<plan_3d::Mesh> = solids
        .iter()
        .filter_map(|s| {
            let mut c = s.clone();
            c.position = c.position - centre;
            c.elevation -= bottom;
            plan_3d::details::solid_mesh(&c, 0.0)
        })
        .collect();
    // Scene frame (front towards -z) -> symbol-local frame (front +z).
    let model = make::model_from_meshes(&meshes)
        .rotated_y(180.0)
        .normalized();
    if model.is_empty() {
        return Err("The selected solids have no surface to save".into());
    }
    let id = user::new_id(ItemKind::Model);
    let name = match name.map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => plan_library::manage::valid_name(n)?,
        None => format!("Solid Symbol {}", id.rsplit('.').next().unwrap_or("1")),
    };
    let folder = user::default_folder(ItemKind::Model);
    let mut item =
        make::item_from_model(&model, &id, &name, &folder, Some(Placement::FreeStanding))?;
    item.tags = vec!["3d".into(), "converted".into(), "solid".into()];
    let saved = user::add(item, Some(&model))?;

    // The plan: the solids go, a placed copy of the symbol takes their place.
    let fl = cx.floor;
    cx.begin_change("Convert to Symbol");
    let mut layer = details_view::load(cx);
    let ids: Vec<Id> = solids.iter().map(|s| s.id).collect();
    layer.solids.retain(|s| !ids.contains(&s.id));
    details_view::save(&mut cx.project, fl, &layer);
    // Free-standing symbols are placed by their back-center.
    let mut sym = PlacedSymbol::new(
        saved.id.clone(),
        Point::new(centre.x, centre.y - d * 0.5),
        w,
        d,
        h,
    );
    sym.elevation = bottom;
    sym.layer = user::layer_of(&saved);
    user::ensure_layer(cx, &sym.layer.clone());
    let symbol = cx.project.add_symbol(fl, sym);
    cx.selection.set(ObjectRef::Symbol(symbol));
    cx.mark_dirty();
    cx.status = format!(
        "Converted {} solid(s) to the symbol \"{}\" in the User Catalog",
        solids.len(),
        saved.name
    );
    Ok(Converted {
        item: saved,
        symbol,
        solids: solids.len(),
    })
}

/// Replace From Library for the selection. Returns the number of objects
/// replaced (0 with a message in the status bar when nothing could be).
pub fn replace_selected(cx: &mut EditorContext) -> usize {
    // A cabinet or an electrical device is swapped on its own.
    if let Some(sel @ (ObjectRef::Cabinet(_) | ObjectRef::Device(_))) = cx.selection.single() {
        return usize::from(user::replace_other(cx, sel));
    }
    let ids: Vec<Id> = cx
        .selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Symbol(id) => Some(*id),
            _ => None,
        })
        .collect();
    if ids.is_empty() {
        cx.status = "Select library objects to replace".into();
        return 0;
    }
    let Some(active) = super::active_item() else {
        cx.status = "Pick an item in the Library Browser, then choose Replace From Library".into();
        return 0;
    };
    let Some(item) = super::find_item(&active) else {
        cx.status = format!("Unknown library item: {active}");
        return 0;
    };
    if item.payload.is_some() {
        cx.status = "Pick a plain library symbol to replace symbols".into();
        return 0;
    }
    let keep = keep_size();
    let mut updated: Vec<PlacedSymbol> = Vec::new();
    for id in ids {
        let Some(old) = cx.floor().symbol(id) else {
            continue;
        };
        if old.catalog_id == item.id || old.image.is_some() || old.distribution.is_some() {
            continue;
        }
        let mut new = old.clone();
        new.catalog_id = item.id.clone();
        if !keep {
            new.width = item.width;
            new.depth = item.depth;
            new.height = item.height;
        }
        updated.push(new);
    }
    if updated.is_empty() {
        cx.status = "The selection already is that library item".into();
        return 0;
    }
    cx.begin_change("Replace From Library");
    let fl = cx.floor;
    for new in &updated {
        if let Some(s) = cx.project.floors[fl]
            .symbols
            .iter_mut()
            .find(|s| s.id == new.id)
        {
            *s = new.clone();
        }
    }
    cx.mark_dirty();
    user::touch_recent(&item.id);
    cx.status = format!("Replaced {} object(s) with {}", updated.len(), item.name);
    updated.len()
}

/// Runs a command id of this module; false when it is not ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        CONVERT_TO_SYMBOL => {
            if let Err(e) = convert_to_symbol(cx, None) {
                cx.status = e;
            }
            true
        }
        REPLACE_FROM_LIBRARY => {
            replace_selected(cx);
            true
        }
        _ => false,
    }
}
