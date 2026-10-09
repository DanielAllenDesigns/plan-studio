//! Adding a [`Converted`] drawing to a project: layers, objects with their
//! look, dimensions and CAD blocks.
//!
//! The editor runs this in one undo step, in two phases so it can draw the
//! hatch lines between them: [`add_objects`] (layers, objects, dimensions)
//! then [`make_blocks`] (groups the objects of each INSERT, hatch lines
//! included).

use super::Converted;
use plan_core::cad::CadAttrs;
use plan_core::layers::Layer;
use plan_core::{Id, Project};
use std::collections::BTreeMap;

/// What to do with an imported block whose name the floor already has
/// (Duplicate CAD Blocks page).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockConflict {
    /// The new block gets a unique name (`name_Copy_1`).
    #[default]
    AutoName,
    /// The blocks already on the floor with that name are deleted first.
    Replace,
    /// The block on the floor is kept and placed at each INSERT instead of
    /// the imported one (at the insertion point, without the INSERT's scale
    /// or rotation: Plan Studio's blocks are copies, not shared definitions).
    UseExisting,
}

/// What [`add_objects`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyReport {
    /// Id of each of [`Converted::objects`] (0: not added).
    pub ids: Vec<Id>,
    pub dimension_ids: Vec<Id>,
    pub new_layers: usize,
    /// Imported blocks renamed because the floor had the name: (from, to).
    pub renamed_blocks: Vec<(String, String)>,
    /// Blocks of the floor deleted by [`BlockConflict::Replace`].
    pub replaced_blocks: usize,
    /// Per imported block: its final name, and `true` if it is skipped in
    /// favour of the floor's own ([`BlockConflict::UseExisting`]).
    pub block_names: Vec<(String, bool)>,
    /// Blocks placed from the floor's own definition.
    pub reused_blocks: usize,
    pub blocks_made: usize,
}

/// Layers, objects and dimensions of `conv` onto `floor`. `conflicts` says
/// what to do per block name (by upper-cased name); `default_conflict` for
/// the rest.
pub fn add_objects(
    project: &mut Project,
    floor: usize,
    conv: &Converted,
    default_conflict: BlockConflict,
    conflicts: &BTreeMap<String, BlockConflict>,
) -> ApplyReport {
    let mut rep = ApplyReport::default();
    for spec in &conv.layers {
        if project.layers.get(&spec.name).is_some() {
            continue;
        }
        let mut layer = Layer::new(spec.name.clone(), spec.color, spec.weight);
        layer.display = spec.visible;
        layer.line_style = spec.line_style;
        if project.layers.add(layer) {
            rep.new_layers += 1;
        }
    }

    // Block names against the floor's.
    let mut existing: Vec<(String, Id)> = project
        .floors
        .get(floor)
        .map(|f| {
            f.cad_blocks()
                .into_iter()
                .map(|b| (b.name, b.group))
                .collect()
        })
        .unwrap_or_default();
    let mut skip = vec![false; conv.blocks.len()];
    let mut names: Vec<String> = conv.blocks.iter().map(|b| b.name.clone()).collect();
    let mut taken: Vec<String> = existing.iter().map(|(n, _)| n.to_uppercase()).collect();
    let mut renamed: BTreeMap<String, String> = BTreeMap::new();
    let mut replaced_names: Vec<String> = Vec::new();
    for (i, b) in conv.blocks.iter().enumerate() {
        let key = b.name.to_uppercase();
        let clash = existing.iter().any(|(n, _)| n.to_uppercase() == key);
        if !clash {
            continue;
        }
        let policy = conflicts.get(&key).copied().unwrap_or(default_conflict);
        match policy {
            BlockConflict::AutoName => {
                let to = renamed
                    .entry(key.clone())
                    .or_insert_with(|| {
                        let mut n = 1;
                        loop {
                            let cand = format!("{}_Copy_{n}", b.name);
                            if !taken.contains(&cand.to_uppercase()) {
                                taken.push(cand.to_uppercase());
                                break cand;
                            }
                            n += 1;
                        }
                    })
                    .clone();
                names[i] = to;
            }
            BlockConflict::Replace => {
                if !replaced_names.contains(&key) {
                    replaced_names.push(key.clone());
                    let groups: Vec<Id> = existing
                        .iter()
                        .filter(|(n, _)| n.to_uppercase() == key)
                        .map(|(_, g)| *g)
                        .collect();
                    for g in groups {
                        project.delete_cad_block(floor, g);
                        rep.replaced_blocks += 1;
                    }
                    existing.retain(|(n, _)| n.to_uppercase() != key);
                }
            }
            BlockConflict::UseExisting => skip[i] = true,
        }
    }
    for (from, to) in renamed {
        let orig = conv
            .blocks
            .iter()
            .find(|b| b.name.to_uppercase() == from)
            .map_or(from, |b| b.name.clone());
        rep.renamed_blocks.push((orig, to));
    }
    rep.block_names = names.iter().cloned().zip(skip.iter().copied()).collect();

    // Place the floor's own block where an INSERT asked for a reused one.
    for (i, b) in conv.blocks.iter().enumerate() {
        if !skip[i] {
            continue;
        }
        let key = b.name.to_uppercase();
        if let Some((_, g)) = existing.iter().find(|(n, _)| n.to_uppercase() == key) {
            if project.insert_cad_block(floor, *g, b.insertion).is_some() {
                rep.reused_blocks += 1;
            }
        }
    }

    for o in &conv.objects {
        if o.block
            .is_some_and(|b| skip.get(b).copied().unwrap_or(false))
        {
            rep.ids.push(0);
            continue;
        }
        let id = project.add_cad(floor, o.layer.clone(), o.item.clone());
        if !o.attrs.is_default() {
            let mut a: CadAttrs = o.attrs.clone();
            a.target = id;
            project.set_cad_attrs(floor, a);
        }
        rep.ids.push(id);
    }
    for d in &conv.dimensions {
        let id = project.add_dimension(floor, d.dim.clone());
        rep.dimension_ids.push(id);
    }
    rep
}

/// Groups the objects of each imported INSERT into a CAD block (and the whole
/// drawing into one when `conv.drawing_block` is set). Hatch lines drawn for
/// an object (its fill's `lines`) join its block. A block of one object stays
/// a loose object. Returns how many blocks were made.
pub fn make_blocks(
    project: &mut Project,
    floor: usize,
    conv: &Converted,
    rep: &mut ApplyReport,
) -> usize {
    let members_of = |project: &Project, ids: &[Id]| -> Vec<Id> {
        let mut out = Vec::new();
        for id in ids {
            out.push(*id);
            if let Some(f) = project.floors.get(floor) {
                if let Some(fill) = f.cad_attrs(*id).and_then(|a| a.fill) {
                    out.extend(
                        fill.lines
                            .iter()
                            .copied()
                            .filter(|l| f.cad.iter().any(|c| c.id == *l)),
                    );
                }
            }
        }
        out
    };
    let mut made = 0;
    if let Some(name) = &conv.drawing_block {
        let ids: Vec<Id> = rep.ids.iter().copied().filter(|i| *i != 0).collect();
        let members = members_of(project, &ids);
        if project
            .make_cad_block(floor, &members, Some(name))
            .is_some()
        {
            made += 1;
        }
    } else {
        for (bi, b) in conv.blocks.iter().enumerate() {
            let (name, skipped) = rep
                .block_names
                .get(bi)
                .cloned()
                .unwrap_or_else(|| (b.name.clone(), false));
            if skipped {
                continue;
            }
            let ids: Vec<Id> = conv
                .objects
                .iter()
                .zip(&rep.ids)
                .filter(|(o, id)| o.block == Some(bi) && **id != 0)
                .map(|(_, id)| *id)
                .collect();
            let members = members_of(project, &ids);
            if let Some(group) = project.make_cad_block(floor, &members, Some(&name)) {
                project.edit_cad_block(floor, group, |info| info.insertion = Some(b.insertion));
                made += 1;
            }
        }
    }
    rep.blocks_made = made;
    made
}

/// [`add_objects`] and [`make_blocks`] for a caller with no hatch lines to
/// draw in between.
pub fn apply_converted(
    project: &mut Project,
    floor: usize,
    conv: &Converted,
    default_conflict: BlockConflict,
    conflicts: &BTreeMap<String, BlockConflict>,
) -> ApplyReport {
    let mut rep = add_objects(project, floor, conv, default_conflict, conflicts);
    make_blocks(project, floor, conv, &mut rep);
    rep
}
