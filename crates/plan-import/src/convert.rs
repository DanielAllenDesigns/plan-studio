//! Units and conversion of DXF drawings into plan-core objects.
//!
//! [`convert`] flattens the drawing (blocks expanded, dimensions kept as
//! objects or drawn), maps its layers, places it (units, scale, rotation,
//! insertion point) and returns a [`Converted`]: CAD objects with their own
//! look ([`CadAttrs`]), dimensions, CAD blocks, hatch requests and the plan
//! layers the objects need. [`apply`] adds that to a project.

mod apply;
mod dims;
mod flatten;
#[cfg(test)]
mod tests;

pub use apply::{add_objects, apply_converted, make_blocks, ApplyReport, BlockConflict};
pub use dims::{linear_dimension, text_override};

use crate::dxf::geom::{ellipse_points, Nurbs};
use crate::dxf::*;
use flatten::{flatten, FlattenOptions};
use plan_core::cad::{
    bezier_spline, lines_to_polylines, ArrowStyle, CadAttrs, CadItem, CadObject, FillAttr, PolyArc,
    TEXT_WIDTH_FACTOR,
};
use plan_core::dimension::{Dimension, DimensionKind};
use plan_core::layers::LineStyle;
use plan_core::text_box::{HAlign, TextBox};
use plan_core::text_styles::RichRun;
use plan_core::{Id, Point, Project};
use std::f64::consts::TAU;

/// Multiplier that converts drawing units to inches.
///
/// `override_units` (a user's choice in the import dialog) wins over the
/// units declared in the file. Unitless drawings are taken to be inches.
pub fn to_inches_factor(units: DxfUnits, override_units: Option<DxfUnits>) -> f64 {
    match override_units.unwrap_or(units) {
        DxfUnits::Unitless | DxfUnits::Inches => 1.0,
        DxfUnits::Feet => 12.0,
        DxfUnits::Millimeters => 1.0 / 25.4,
        DxfUnits::Centimeters => 1.0 / 2.54,
        DxfUnits::Meters => 1.0 / 0.0254,
        DxfUnits::Yards => 36.0,
        DxfUnits::Miles => 63_360.0,
        DxfUnits::Kilometers => 1000.0 / 0.0254,
        DxfUnits::Decimeters => 0.1 / 0.0254,
    }
}

/// The units a drawing is read in when the file declares none: millimetres
/// for a metric file (`$MEASUREMENT` 1), otherwise inches (Chief defaults to
/// inches or millimetres the same way).
pub fn default_units(d: &DxfDrawing) -> DxfUnits {
    match d.units {
        DxfUnits::Unitless if d.metric => DxfUnits::Millimeters,
        DxfUnits::Unitless => DxfUnits::Inches,
        u => u,
    }
}

/// Where a DXF layer goes in the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerTarget {
    /// The DXF name under the import's prefix (`prefix + name`).
    Keep,
    /// A plan layer by this exact name (an existing one or a new one).
    Rename(String),
    /// Not imported.
    Skip,
}

/// One row of the layer mapping: a DXF layer and where it lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerMapping {
    pub source: String,
    pub target: LayerTarget,
}

/// What to do with DIMENSION entities (Import Drawing Assistant, Drawing
/// Unit page).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DimensionMode {
    /// Linear and aligned dimensions become dimension objects; the rest
    /// are drawn as lines and text.
    #[default]
    Objects,
    /// Every dimension is drawn as lines and text in its own CAD block.
    Blocks,
}

/// How INSERTs become CAD blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockMode {
    /// Each INSERT with more than one object is a CAD block of that name.
    #[default]
    PerInsert,
    /// The whole drawing is one CAD block named after the file.
    WholeDrawing,
    /// Loose objects.
    None,
}

/// How a drawing is placed in the plan and what is read from it.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportOptions {
    /// Plan inches per drawing unit (see [`to_inches_factor`]).
    pub factor: f64,
    /// Extra scale on top of the units (1.0 keeps the drawing's size).
    pub scale: f64,
    /// Counter-clockwise rotation about the insertion point, degrees.
    pub rotation_deg: f64,
    /// The drawing point (drawing units) that goes to `insertion`.
    pub base: Point,
    /// The plan point (inches) the base point is placed at.
    pub insertion: Point,
    /// Prefix of [`LayerTarget::Keep`] layers.
    pub layer_prefix: String,
    /// Layers listed here are mapped; the others are kept.
    pub layers: Vec<LayerMapping>,
    /// Every layer goes to this one plan layer (Layer Mapping: "A single
    /// layer"); an entry of `layers` still wins.
    pub single_layer: Option<String>,
    /// New plan layers take the colour, weight and line style of the DXF
    /// layer they come from.
    pub layer_attrs: bool,
    /// Put colour, weight and line style on every object, also when the
    /// DXF has them by layer (what is left of a layer when its attributes
    /// are not imported).
    pub object_attrs: bool,
    pub dimensions: DimensionMode,
    pub blocks: BlockMode,
    /// Name of the whole-drawing block.
    pub drawing_name: String,
    pub include_paper_space: bool,
    pub import_hatch: bool,
    pub import_points: bool,
    /// Join lines that share end points into polylines.
    pub join_lines: bool,
    /// Join lines that close a rectangle into a box (closed polyline).
    pub boxes: bool,
    /// Text styles the plan has; a DXF style of the same name is kept.
    pub text_styles: Vec<String>,
}

impl ImportOptions {
    /// Units only: the drawing keeps its origin, size and layer names.
    pub fn new(factor: f64, layer_prefix: &str) -> Self {
        Self {
            factor,
            scale: 1.0,
            rotation_deg: 0.0,
            base: Point::ZERO,
            insertion: Point::ZERO,
            layer_prefix: layer_prefix.to_string(),
            layers: Vec::new(),
            single_layer: None,
            layer_attrs: false,
            object_attrs: false,
            dimensions: DimensionMode::Objects,
            blocks: BlockMode::PerInsert,
            drawing_name: "Imported Drawing".into(),
            include_paper_space: false,
            import_hatch: true,
            import_points: true,
            join_lines: false,
            boxes: false,
            text_styles: Vec::new(),
        }
    }

    /// The plan layer a DXF layer maps to; `None` for a skipped layer.
    pub fn target_layer(&self, dxf_layer: &str) -> Option<String> {
        match self.layers.iter().find(|m| m.source == dxf_layer) {
            Some(LayerMapping {
                target: LayerTarget::Skip,
                ..
            }) => None,
            Some(LayerMapping {
                target: LayerTarget::Rename(name),
                ..
            }) if !name.trim().is_empty() => Some(name.clone()),
            _ => match &self.single_layer {
                Some(s) if !s.trim().is_empty() => Some(s.clone()),
                _ => Some(format!("{}{}", self.layer_prefix, dxf_layer)),
            },
        }
    }

    fn k(&self) -> f64 {
        self.factor * self.scale
    }

    /// A drawing point in the plan.
    pub fn place(&self, p: Point) -> Point {
        let k = self.k();
        let (s, c) = self.rotation_deg.to_radians().sin_cos();
        let d = p.sub(self.base).scale(k);
        self.insertion
            .add(Point::new(d.x * c - d.y * s, d.x * s + d.y * c))
    }
}

/// A hatch the editor draws with its hatch lines (the DXF HATCH asked for
/// this pattern on the object's outline).
#[derive(Debug, Clone, PartialEq)]
pub struct HatchSpec {
    /// A name of the CAD Hatch tool's patterns ("Diagonal Lines", "Brick"...).
    pub style: &'static str,
    /// Plan inches between the lines.
    pub spacing: f64,
    pub angle_deg: f64,
    pub color: [u8; 3],
}

/// One CAD object to add.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedObject {
    /// The plan layer.
    pub layer: String,
    /// The DXF layer it came from.
    pub source_layer: String,
    pub item: CadItem,
    /// Its own look (the `target` is filled in when it is added).
    pub attrs: CadAttrs,
    /// Index into [`Converted::blocks`].
    pub block: Option<usize>,
    /// A pattern fill to draw on this closed outline.
    pub hatch: Option<HatchSpec>,
}

/// One dimension to add.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedDimension {
    pub layer: String,
    pub dim: Dimension,
}

/// A CAD block to make of the objects that name it.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedBlock {
    pub name: String,
    /// The block's insertion point in the plan.
    pub insertion: Point,
}

/// A plan layer the import needs.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanLayerSpec {
    pub name: String,
    pub color: [u8; 3],
    /// Hundredths of a millimetre.
    pub weight: u32,
    pub line_style: LineStyle,
    pub visible: bool,
    /// The DXF layer it comes from (empty for a single target layer).
    pub from: String,
}

/// The result of [`convert`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Converted {
    pub objects: Vec<ImportedObject>,
    pub dimensions: Vec<ImportedDimension>,
    pub blocks: Vec<ImportedBlock>,
    pub layers: Vec<PlanLayerSpec>,
    /// Everything is one CAD block of this name (BlockMode::WholeDrawing).
    pub drawing_block: Option<String>,
    /// Things worth telling the user.
    pub notes: Vec<String>,
}

/// Counts for the preview line of the assistant.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Summary {
    pub objects: usize,
    pub lines: usize,
    pub polylines: usize,
    pub arcs_circles: usize,
    pub texts: usize,
    pub hatches: usize,
    pub dimensions: usize,
    pub blocks: usize,
    pub layers: usize,
}

impl Converted {
    pub fn summary(&self) -> Summary {
        let mut s = Summary {
            objects: self.objects.len(),
            hatches: self.objects.iter().filter(|o| o.hatch.is_some()).count(),
            dimensions: self.dimensions.len(),
            blocks: self.blocks.len(),
            layers: self.layers.len(),
            ..Summary::default()
        };
        for o in &self.objects {
            match o.item {
                CadItem::Line { .. } => s.lines += 1,
                CadItem::Polyline { .. } => s.polylines += 1,
                CadItem::Circle { .. } | CadItem::Arc { .. } => s.arcs_circles += 1,
                CadItem::Text { .. } => s.texts += 1,
            }
        }
        s
    }

    /// Bounds of the objects (plan inches).
    pub fn bounds(&self) -> Option<(Point, Point)> {
        let mut it = self
            .objects
            .iter()
            .map(|o| o.item.bounds())
            .chain(self.dimensions.iter().map(|d| (d.dim.start, d.dim.end)));
        let first = it.next()?;
        Some(it.fold(first, |(lo, hi), (a, b)| {
            (
                Point::new(lo.x.min(a.x).min(b.x), lo.y.min(a.y).min(b.y)),
                Point::new(hi.x.max(a.x).max(b.x), hi.y.max(a.y).max(b.y)),
            )
        }))
    }
}

/// Bounds of what the drawing draws, in drawing units (the lower-left corner
/// is the "Move drawing to the origin" base point). Every layer counts.
pub fn drawing_bounds(drawing: &DxfDrawing, include_paper_space: bool) -> Option<(Point, Point)> {
    let mut o = ImportOptions::new(1.0, "");
    o.include_paper_space = include_paper_space;
    o.blocks = BlockMode::None;
    o.import_points = false;
    convert(drawing, &o).bounds()
}

/// How many drawing objects each DXF layer holds once the blocks are
/// expanded (by layer name, sorted).
pub fn layer_counts(drawing: &DxfDrawing, include_paper_space: bool) -> Vec<(String, usize)> {
    let fl = flatten(
        drawing,
        &FlattenOptions {
            dims_as_objects: true,
            instances: false,
            include_paper: include_paper_space,
        },
    );
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for f in &fl.items {
        *counts.entry(f.entity.props.layer.clone()).or_default() += 1;
    }
    counts.into_iter().collect()
}

/// Names of the blocks no INSERT of the file uses (the assistant lists them;
/// Plan Studio keeps no block library to place them in).
pub fn unused_blocks(drawing: &DxfDrawing) -> Vec<String> {
    let mut used: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    fn walk(d: &DxfDrawing, ents: &[DxfEntity], used: &mut std::collections::BTreeSet<String>) {
        for e in ents {
            match &e.kind {
                DxfKind::Insert(i) => {
                    let key = i.block.to_uppercase();
                    if used.insert(key.clone()) {
                        if let Some(b) = d.blocks.get(&key) {
                            walk(d, &b.entities, used);
                        }
                    }
                }
                DxfKind::Dimension(dim) if !dim.block.is_empty() => {
                    let key = dim.block.to_uppercase();
                    if used.insert(key.clone()) {
                        if let Some(b) = d.blocks.get(&key) {
                            walk(d, &b.entities, used);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    walk(drawing, &drawing.entities, &mut used);
    walk(drawing, &drawing.paper_entities, &mut used);
    drawing
        .blocks
        .iter()
        .filter(|(k, b)| !b.anonymous && b.xref.is_empty() && !used.contains(*k))
        .map(|(_, b)| b.name.clone())
        .collect()
}

/// Convert a drawing's entities (INSERTs exploded) to CAD objects scaled by
/// `factor` (see [`to_inches_factor`]).
///
/// Each object's layer is `layer_prefix` followed by the DXF layer name, e.g.
/// prefix `"Import: "` gives `"Import: A-WALL"`. Bulged polyline segments are
/// sampled into straight pieces (8 per 90 degrees). Object ids are
/// sequential placeholders starting at 1; use [`apply_cad`] to add the result
/// to a [`Project`] with real ids.
pub fn to_cad_objects(drawing: &DxfDrawing, factor: f64, layer_prefix: &str) -> Vec<CadObject> {
    to_cad_objects_with(drawing, &ImportOptions::new(factor, layer_prefix))
}

/// [`to_cad_objects`] with the full set of import options: the layer mapping
/// (skipped layers are dropped, renamed ones land on the plan layer given),
/// an extra scale, a rotation and the insertion point. Dimensions and the
/// extra look of each object are in [`convert`]'s result.
pub fn to_cad_objects_with(drawing: &DxfDrawing, opts: &ImportOptions) -> Vec<CadObject> {
    convert(drawing, opts)
        .objects
        .into_iter()
        .enumerate()
        .map(|(i, o)| CadObject {
            id: i as Id + 1,
            layer: o.layer,
            item: o.item,
        })
        .collect()
}

/// Add converted objects to `floor` of `project` with fresh ids; returns them.
pub fn apply_cad(project: &mut Project, floor: usize, objects: &[CadObject]) -> Vec<Id> {
    objects
        .iter()
        .map(|o| project.add_cad(floor, o.layer.clone(), o.item.clone()))
        .collect()
}

// ===================================================================
// Conversion
// ===================================================================

/// The look an object carries of its own.
#[derive(Debug, Clone, Copy, Default)]
struct Look {
    color: Option<[u8; 3]>,
    weight: Option<u32>,
    dash: Option<LineStyle>,
}

struct Conv<'a> {
    d: &'a DxfDrawing,
    o: &'a ImportOptions,
    out: Converted,
    inst_block: Vec<Option<usize>>,
    insts: Vec<flatten::Instance>,
}

/// Run the conversion.
pub fn convert(drawing: &DxfDrawing, opts: &ImportOptions) -> Converted {
    let fl = flatten(
        drawing,
        &FlattenOptions {
            dims_as_objects: opts.dimensions == DimensionMode::Objects,
            instances: opts.blocks == BlockMode::PerInsert,
            include_paper: opts.include_paper_space,
        },
    );
    let mut c = Conv {
        d: drawing,
        o: opts,
        out: Converted::default(),
        inst_block: vec![None; fl.instances.len()],
        insts: fl.instances.clone(),
    };
    for f in &fl.items {
        c.entity(&f.entity, f.inst);
    }
    if opts.blocks == BlockMode::WholeDrawing && !c.out.objects.is_empty() {
        c.out.drawing_block = Some(opts.drawing_name.clone());
    }
    c.finish_layers();
    let mut out = c.out;
    if opts.join_lines || opts.boxes {
        join_lines(&mut out, opts);
    }
    if !fl.missing_blocks.is_empty() {
        out.notes.push(format!(
            "Blocks the file uses but does not define: {}",
            fl.missing_blocks.join(", ")
        ));
    }
    let mut xrefs: Vec<&String> = drawing.xrefs.iter().collect();
    for x in &fl.xref_inserts {
        if !xrefs.contains(&x) {
            xrefs.push(x);
        }
    }
    if !xrefs.is_empty() {
        out.notes.push(format!(
            "External references are not imported (the files are missing from this import): {}",
            xrefs.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        ));
    }
    if !opts.include_paper_space && !drawing.paper_entities.is_empty() {
        out.notes.push(format!(
            "{} paper space objects were left out",
            drawing.paper_entities.len()
        ));
    }
    out
}

impl Conv<'_> {
    fn k(&self) -> f64 {
        self.o.k()
    }

    fn place(&self, p: Point) -> Point {
        self.o.place(p)
    }

    fn turn(&self) -> f64 {
        self.o.rotation_deg.to_radians()
    }

    fn look(&self, p: &DxfProps) -> Look {
        let layer = self.d.layer(&p.layer);
        let layer_rgb = layer.map_or([0, 0, 0], DxfLayer::rgb);
        let color = p.color.rgb().or(self.o.object_attrs.then_some(layer_rgb));
        let weight = if p.weight >= 0 {
            Some(p.weight as u32)
        } else if self.o.object_attrs {
            layer.filter(|l| l.weight >= 0).map(|l| l.weight as u32)
        } else {
            None
        };
        let explicit_lt = !p.linetype.is_empty() && !p.linetype.eq_ignore_ascii_case("BYLAYER");
        let dash = if explicit_lt {
            Some(self.linestyle(&p.linetype))
        } else if self.o.object_attrs {
            let s = self.linestyle(layer.map_or("CONTINUOUS", |l| l.linetype.as_str()));
            (s != LineStyle::Solid).then_some(s)
        } else {
            None
        };
        Look { color, weight, dash }
    }

    fn linestyle(&self, name: &str) -> LineStyle {
        linetype_style(self.d.linetype(name), name)
    }

    /// The layer's RGB (black when unknown).
    fn layer_rgb(&self, name: &str) -> [u8; 3] {
        self.d.layer(name).map_or([0, 0, 0], DxfLayer::rgb)
    }

    fn block_for(&mut self, inst: Option<usize>) -> Option<usize> {
        let i = inst?;
        if let Some(b) = self.inst_block[i] {
            return Some(b);
        }
        let ins = self.insts.get(i)?;
        let info = ImportedBlock {
            name: ins.name.clone(),
            insertion: self.place(ins.at),
        };
        self.out.blocks.push(info);
        let idx = self.out.blocks.len() - 1;
        self.inst_block[i] = Some(idx);
        Some(idx)
    }

    /// Adds an object; returns its index.
    fn push(
        &mut self,
        props: &DxfProps,
        layer: &str,
        item: CadItem,
        look: Look,
        inst: Option<usize>,
        tune: impl FnOnce(&mut CadAttrs),
    ) -> usize {
        let mut attrs = CadAttrs::new(0);
        attrs.color = look.color;
        attrs.weight = look.weight;
        attrs.dash = look.dash;
        tune(&mut attrs);
        let block = self.block_for(inst);
        self.out.objects.push(ImportedObject {
            layer: layer.to_string(),
            source_layer: props.layer.clone(),
            item,
            attrs,
            block,
            hatch: None,
        });
        self.out.objects.len() - 1
    }

    fn entity(&mut self, e: &DxfEntity, inst: Option<usize>) {
        let p = &e.props;
        let Some(layer) = self.o.target_layer(&p.layer) else {
            return;
        };
        let look = self.look(p);
        let k = self.k();
        let turn = self.turn();
        match &e.kind {
            DxfKind::Line { a, b } => {
                let item = CadItem::Line {
                    a: self.place(*a),
                    b: self.place(*b),
                };
                self.push(p, &layer, item, look, inst, |_| {});
            }
            DxfKind::Polyline {
                points,
                closed,
                bulges,
            } => {
                let (pts, arcs) = sample_with_arcs(points, bulges, *closed);
                let pts: Vec<Point> = pts.into_iter().map(|q| self.place(q)).collect();
                if pts.len() < 2 {
                    return;
                }
                let item = CadItem::Polyline {
                    points: pts,
                    closed: *closed,
                };
                self.push(p, &layer, item, look, inst, |a| a.arc_edges = arcs);
            }
            DxfKind::Circle { center, radius } => {
                let item = CadItem::Circle {
                    center: self.place(*center),
                    radius: radius * k,
                };
                self.push(p, &layer, item, look, inst, |_| {});
            }
            DxfKind::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => {
                let item = CadItem::Arc {
                    center: self.place(*center),
                    radius: radius * k,
                    start_angle: start_deg.to_radians() + turn,
                    end_angle: end_deg.to_radians() + turn,
                };
                self.push(p, &layer, item, look, inst, |_| {});
            }
            DxfKind::Ellipse { center, u, v, t0, t1 } => {
                let pts = ellipse_points(*center, *u, *v, *t0, *t1);
                let full = (t1 - t0).abs() >= TAU - 1e-6;
                self.polyline_item(p, &layer, pts, full, look, inst);
            }
            DxfKind::Spline {
                degree,
                closed,
                knots,
                weights,
                control,
                fit,
            } => {
                let pts = if control.len() > *degree {
                    Nurbs {
                        degree: *degree,
                        knots,
                        weights,
                        control,
                    }
                    .sample(8)
                } else if fit.len() >= 2 {
                    bezier_spline(fit, *closed, 8, 0.5)
                } else {
                    control.clone()
                };
                let mut pts = pts;
                let mut is_closed = *closed;
                if pts.len() > 2 && pts[0].dist(pts[pts.len() - 1]) < 1e-9 * (1.0 + pts[0].length()) {
                    pts.pop();
                    is_closed = true;
                }
                self.polyline_item(p, &layer, pts, is_closed, look, inst);
            }
            DxfKind::Hatch(h) => self.hatch(p, &layer, h, look, inst),
            DxfKind::Face { points, filled } => {
                if points.len() < 3 {
                    return;
                }
                let fill = filled.then(|| look.color.unwrap_or_else(|| self.layer_rgb(&p.layer)));
                let pts: Vec<Point> = points.iter().map(|q| self.place(*q)).collect();
                let item = CadItem::Polyline {
                    points: pts,
                    closed: true,
                };
                self.push(p, &layer, item, look, inst, |a| {
                    a.fill = fill.map(|color| FillAttr {
                        color,
                        ..FillAttr::default()
                    });
                });
            }
            DxfKind::Marker { pos } => {
                if self.o.import_points {
                    let item = CadItem::Circle {
                        center: self.place(*pos),
                        radius: POINT_RADIUS,
                    };
                    self.push(p, &layer, item, look, inst, |_| {});
                }
            }
            DxfKind::Text(t) => self.text(p, &layer, t, look, inst),
            DxfKind::Dimension(d) => self.dimension(p, &layer, d),
            DxfKind::Leader { points, arrow } => {
                let pts: Vec<Point> = points.iter().map(|q| self.place(*q)).collect();
                let item = CadItem::Polyline {
                    points: pts,
                    closed: false,
                };
                let arrow = *arrow;
                self.push(p, &layer, item, look, inst, |a| {
                    if arrow {
                        a.arrow_start = ArrowStyle::Filled;
                    }
                });
            }
            DxfKind::MLeader(m) => {
                for line in &m.lines {
                    let pts: Vec<Point> = line.iter().map(|q| self.place(*q)).collect();
                    let item = CadItem::Polyline {
                        points: pts,
                        closed: false,
                    };
                    let arrow = m.arrow;
                    self.push(p, &layer, item, look, inst, |a| {
                        if arrow {
                            a.arrow_start = ArrowStyle::Filled;
                        }
                    });
                }
                if !m.text.trim().is_empty() {
                    let at = m
                        .text_pos
                        .or_else(|| m.lines.last().and_then(|l| l.last()).copied())
                        .unwrap_or_default();
                    let t = DxfText {
                        pos: at,
                        h: HJust::Left,
                        v: VJust::Top,
                        text: m.text.clone(),
                        runs: Vec::new(),
                        height: if m.height > 0.0 { m.height } else { 0.125 / k.max(1e-9) },
                        angle_deg: m.angle_deg,
                        width_factor: 1.0,
                        oblique_deg: 0.0,
                        style: String::new(),
                        wrap_width: 0.0,
                        mtext: true,
                        tag: String::new(),
                        attdef: false,
                        invisible: false,
                    };
                    self.text(p, &layer, &t, look, inst);
                }
            }
            // Expanded before conversion.
            DxfKind::Insert(_) => {}
        }
    }

    fn polyline_item(
        &mut self,
        p: &DxfProps,
        layer: &str,
        pts: Vec<Point>,
        closed: bool,
        look: Look,
        inst: Option<usize>,
    ) {
        if pts.len() < 2 {
            return;
        }
        let pts: Vec<Point> = pts.into_iter().map(|q| self.place(q)).collect();
        let item = CadItem::Polyline { points: pts, closed };
        self.push(p, layer, item, look, inst, |_| {});
    }

    fn hatch(&mut self, p: &DxfProps, layer: &str, h: &DxfHatch, look: Look, inst: Option<usize>) {
        if !self.o.import_hatch {
            return;
        }
        let color = look.color.unwrap_or_else(|| self.layer_rgb(&p.layer));
        let k = self.k();
        for l in &h.loops {
            let pts: Vec<Point> = l.points.iter().map(|q| self.place(*q)).collect();
            if pts.len() < 3 {
                continue;
            }
            let item = CadItem::Polyline {
                points: pts,
                closed: true,
            };
            let external = l.external;
            let solid = h.solid;
            let at = self.push(p, layer, item, look, inst, |a| {
                if external && solid {
                    a.fill = Some(FillAttr {
                        color,
                        ..FillAttr::default()
                    });
                }
            });
            if external && !solid {
                let (style, _) = hatch_style(&h.pattern, h.angle_deg);
                let spacing = h.spacing.map_or(4.0, |s| s * k).max(0.5);
                self.out.objects[at].hatch = Some(HatchSpec {
                    style,
                    spacing,
                    angle_deg: h.angle_deg + self.o.rotation_deg,
                    color,
                });
            }
        }
    }

    fn text(&mut self, p: &DxfProps, layer: &str, t: &DxfText, look: Look, inst: Option<usize>) {
        if t.text.trim().is_empty() {
            return;
        }
        let k = self.k();
        let mut height = t.height;
        if height <= 0.0 {
            height = self
                .d
                .text_style(&t.style)
                .map(|s| s.height)
                .filter(|h| *h > 0.0)
                .unwrap_or(0.125 / k.max(1e-9));
        }
        let h_in = height * k;
        let runs: Vec<RichRun> = t
            .runs
            .iter()
            .map(|r| RichRun {
                text: r.text.clone(),
                bold: r.bold,
                italic: r.italic,
                underline: r.underline,
                scale: r.scale,
                color: r.color,
                font: r.font.clone(),
                ..RichRun::default()
            })
            .collect();
        let wrap = if t.mtext { t.wrap_width * k } else { 0.0 };
        let halign = match t.h {
            HJust::Left => HAlign::Left,
            HJust::Center => HAlign::Center,
            HJust::Right => HAlign::Right,
        };
        let tb = TextBox {
            width: wrap,
            halign: if t.mtext { halign } else { HAlign::Left },
            ..TextBox::default()
        };
        let lay = plan_core::text_box::layout(&t.text, &runs, h_in, &tb);
        let lines = t.text.split('\n').count().max(1);
        let (w, hh) = if t.mtext {
            (lay.width, lay.height)
        } else {
            (
                (t.text.chars().count() as f64 * h_in * TEXT_WIDTH_FACTOR * t.width_factor).max(lay.width * t.width_factor),
                h_in * lines as f64,
            )
        };
        let dx = match t.h {
            HJust::Left => 0.0,
            HJust::Center => -w / 2.0,
            HJust::Right => -w,
        };
        let dy = match t.v {
            VJust::Baseline | VJust::Bottom => 0.0,
            VJust::Middle => -hh / 2.0,
            VJust::Top => -hh,
        };
        let ang = (t.angle_deg + self.o.rotation_deg).to_radians();
        let (s, c) = ang.sin_cos();
        let pos = self.place(t.pos).add(Point::new(dx * c - dy * s, dx * s + dy * c));
        let style = (!t.style.is_empty())
            .then(|| {
                self.o
                    .text_styles
                    .iter()
                    .find(|n| n.eq_ignore_ascii_case(&t.style))
                    .cloned()
            })
            .flatten();
        let item = CadItem::Text {
            pos,
            text: t.text.clone(),
            height: h_in,
            angle: ang,
        };
        let boxed = t.mtext && (wrap > 0.0 || halign != HAlign::Left);
        self.push(p, layer, item, look, inst, |a| {
            a.runs = runs;
            if boxed {
                a.text_box = tb;
            }
            a.text_style = style;
        });
    }

    fn dimension(&mut self, p: &DxfProps, layer: &str, d: &DxfDimension) {
        let Some((start, end, offset)) = linear_dimension(d) else {
            return;
        };
        let k = self.k();
        let turn = self.turn();
        // The dimension line's offset and the points turn and scale with
        // the drawing.
        let (s, e) = (self.place(start), self.place(end));
        let mut dim = Dimension::new(0, DimensionKind::Manual, s, e, offset * k);
        let _ = (turn, p);
        dim.text_override = text_override(d);
        self.out.dimensions.push(ImportedDimension {
            layer: layer.to_string(),
            dim,
        });
    }

    fn finish_layers(&mut self) {
        // Plan layers in first-use order.
        let mut seen: Vec<(String, String)> = Vec::new();
        for o in &self.out.objects {
            if !seen.iter().any(|(n, _)| *n == o.layer) {
                seen.push((o.layer.clone(), o.source_layer.clone()));
            }
        }
        let single = self.o.single_layer.as_ref().filter(|s| !s.trim().is_empty());
        for (name, from) in seen {
            let src = (!from.is_empty()).then(|| self.d.layer(&from)).flatten();
            let spec = match src {
                Some(l) if self.o.layer_attrs && single != Some(&name) => PlanLayerSpec {
                    name,
                    color: l.rgb(),
                    weight: l.weight_hundredths(),
                    line_style: self.linestyle(&l.linetype),
                    visible: l.visible,
                    from,
                },
                _ => PlanLayerSpec {
                    name,
                    color: [60, 60, 60],
                    weight: 18,
                    line_style: LineStyle::Solid,
                    visible: src.is_none_or(|l| l.visible) || single.is_some(),
                    from,
                },
            };
            self.out.layers.push(spec);
        }
    }
}

/// Radius of the circle a DXF POINT is drawn as, inches.
pub const POINT_RADIUS: f64 = 0.5;

/// Vertices of a polyline with bulged segments replaced by sampled arcs, and
/// the arc edges that record them (see [`CadAttrs::arc_edges`]).
fn sample_with_arcs(points: &[Point], bulges: &[f64], closed: bool) -> (Vec<Point>, Vec<PolyArc>) {
    let n = points.len();
    if n == 0 {
        return (Vec::new(), Vec::new());
    }
    let segments = if closed { n } else { n - 1 };
    let mut out = vec![points[0]];
    let mut arcs = Vec::new();
    for i in 0..segments {
        let (a, b) = (points[i], points[(i + 1) % n]);
        let bulge = bulges.get(i).copied().unwrap_or(0.0);
        let from = out.len() - 1;
        let mid = crate::dxf::geom::bulge_points(a, b, bulge);
        let had = !mid.is_empty();
        out.extend(mid);
        if i + 1 < n {
            out.push(b);
        }
        if had {
            let to = if i + 1 < n { out.len() - 1 } else { out.len() };
            arcs.push(PolyArc { from, to, bulge });
        }
    }
    (out, arcs)
}

/// The line style a DXF line type stands for.
pub fn linetype_style(def: Option<&DxfLinetype>, name: &str) -> LineStyle {
    let n = name.to_ascii_uppercase();
    if n.is_empty() || n == "CONTINUOUS" || n == "BYLAYER" || n == "BYBLOCK" {
        return LineStyle::Solid;
    }
    if n.contains("DASHDOT") || n.contains("CENTER") || n.contains("PHANTOM") || n.contains("DIVIDE") {
        return LineStyle::DashDot;
    }
    if n.contains("DOT") && !n.contains("DASH") {
        return LineStyle::Dotted;
    }
    if n.contains("DASH") || n.contains("HIDDEN") || n.contains("BORDER") {
        return LineStyle::Dashed;
    }
    match def {
        Some(d) if !d.pattern.is_empty() => {
            let dots = d.pattern.iter().filter(|v| **v == 0.0).count();
            let dashes = d.pattern.iter().filter(|v| **v > 0.0).count();
            if dashes == 0 && dots == 0 {
                LineStyle::Solid
            } else if dots > 0 && dashes > 0 {
                LineStyle::DashDot
            } else if dots > 0 {
                LineStyle::Dotted
            } else {
                LineStyle::Dashed
            }
        }
        _ => LineStyle::Solid,
    }
}

/// The CAD Hatch tool pattern for an AutoCAD hatch pattern name, and
/// whether it is a solid fill. Unknown names go by the pattern angle.
pub fn hatch_style(name: &str, angle_deg: f64) -> (&'static str, bool) {
    let n = name.trim().to_ascii_uppercase();
    match n.as_str() {
        "" | "SOLID" => ("Solid", true),
        "ANSI31" | "ANSI32" | "ANSI33" | "ANSI34" | "ANSI35" | "ANSI36" | "ANSI39" | "STEEL" | "ANGLE" => {
            ("Diagonal Lines", false)
        }
        "ANSI37" | "ANSI38" | "NET" | "NET3" | "CROSS" | "PLUS" | "GRATE" | "HOUND" | "DASH" | "ESCHER" | "STARS" | "ZIGZAG" | "SWAMP" => {
            ("Cross Hatch", false)
        }
        "LINE" => ("Horizontal Lines", false),
        "BRICK" | "BRSTONE" | "AR-BRSTD" | "AR-B816" | "AR-B816C" | "AR-B88" | "AR-RROOF" | "AR-RSHKE" => {
            ("Brick", false)
        }
        "AR-HBONE" | "AR-PARQ1" | "HONEY" | "SQUARE" | "BOX" | "TRIANG" | "ARROWS" | "ANSI3A" => {
            ("Block", false)
        }
        "AR-CONC" | "CONCRETE" | "AR-SAND" | "GRAVEL" => ("Concrete", false),
        "EARTH" | "DOTS" | "GRASS" | "CORK" | "MUDST" | "FLEX" => ("Earth", false),
        "INSUL" | "ANSI37A" => ("Insulation", false),
        _ => {
            let a = angle_deg.rem_euclid(180.0);
            if !(5.0..=175.0).contains(&a) {
                ("Horizontal Lines", false)
            } else if (a - 90.0).abs() < 5.0 {
                ("Vertical Lines", false)
            } else {
                ("Diagonal Lines", false)
            }
        }
    }
}

// ----- joining lines -----

fn same_look(a: &CadAttrs, b: &CadAttrs) -> bool {
    a.color == b.color && a.weight == b.weight && a.dash == b.dash
}

fn is_rectangle(pts: &[Point]) -> bool {
    if pts.len() != 4 {
        return false;
    }
    let side = |i: usize| pts[(i + 1) % 4].sub(pts[i]);
    (0..4).all(|i| {
        let (a, b) = (side(i), side((i + 1) % 4));
        let denom = a.length() * b.length();
        denom > 1e-12 && (a.dot(b) / denom).abs() < 1e-3
    })
}

/// Lines that share end points become polylines (and rectangles boxes),
/// per layer, look and block. Polylines are not touched.
fn join_lines(c: &mut Converted, opts: &ImportOptions) {
    let tol = 0.05;
    let objs = std::mem::take(&mut c.objects);
    let mut done = vec![false; objs.len()];
    let mut out: Vec<ImportedObject> = Vec::with_capacity(objs.len());
    for i in 0..objs.len() {
        if done[i] {
            continue;
        }
        if !matches!(objs[i].item, CadItem::Line { .. }) {
            out.push(objs[i].clone());
            continue;
        }
        // Every later line with the same layer, look and block.
        let group: Vec<usize> = (i..objs.len())
            .filter(|j| {
                !done[*j]
                    && matches!(objs[*j].item, CadItem::Line { .. })
                    && objs[*j].layer == objs[i].layer
                    && objs[*j].block == objs[i].block
                    && same_look(&objs[*j].attrs, &objs[i].attrs)
            })
            .collect();
        let lines: Vec<(Point, Point)> = group
            .iter()
            .filter_map(|j| match objs[*j].item {
                CadItem::Line { a, b } => Some((a, b)),
                _ => None,
            })
            .collect();
        let chains = lines_to_polylines(&lines, tol);
        for j in &group {
            done[*j] = true;
        }
        for (pts, closed) in chains {
            let keep = if opts.join_lines {
                true
            } else {
                closed && is_rectangle(&pts)
            };
            if keep && pts.len() > 2 {
                let mut o = objs[i].clone();
                o.item = CadItem::Polyline { points: pts, closed };
                out.push(o);
            } else if keep {
                // A lone segment stays a line.
                let mut o = objs[i].clone();
                o.item = CadItem::Line { a: pts[0], b: pts[1] };
                out.push(o);
            } else {
                // Put the segments back as they were.
                let n = pts.len();
                let segs = if closed { n } else { n - 1 };
                for s in 0..segs {
                    let mut o = objs[i].clone();
                    o.item = CadItem::Line {
                        a: pts[s],
                        b: pts[(s + 1) % n],
                    };
                    out.push(o);
                }
            }
        }
    }
    c.objects = out;
}
