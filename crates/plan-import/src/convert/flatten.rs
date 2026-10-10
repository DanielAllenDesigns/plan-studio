//! Block expansion: INSERT/MINSERT recursively replaced by the transformed
//! contents of the block, with the layer / colour / weight / line type
//! inheritance of BYBLOCK and layer `0`, attribute handling and dimension
//! blocks.

use super::dims::{dimension_entities, linear_dimension, xform_dimension};
use crate::dxf::geom::{sample_polyline, Xf};
use crate::dxf::*;
use plan_core::Point;

/// Guard against self-referencing blocks.
pub const MAX_INSERT_DEPTH: usize = 16;

/// What to do with dimensions and which parts of the file to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlattenOptions {
    /// Linear and aligned dimensions stay dimension entities.
    pub dims_as_objects: bool,
    /// Track every top-level INSERT (and every exploded dimension when
    /// `dims_as_objects` is off) as an instance.
    pub instances: bool,
    pub include_paper: bool,
}

/// One leaf entity of the expanded drawing.
#[derive(Debug, Clone, PartialEq)]
pub struct Flat {
    pub entity: DxfEntity,
    /// The top-level INSERT (or exploded dimension) it came from.
    pub inst: Option<usize>,
}

/// A top-level block instance.
#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    pub name: String,
    /// Where it sits in the drawing.
    pub at: Point,
}

#[derive(Debug, Default)]
pub struct Flattened {
    pub items: Vec<Flat>,
    pub instances: Vec<Instance>,
    /// Block names an INSERT asked for and the file does not define.
    pub missing_blocks: Vec<String>,
    /// INSERTs of external references (not imported).
    pub xref_inserts: Vec<String>,
}

/// Expand the model space (and paper space when asked) of `d`.
pub fn flatten(d: &DxfDrawing, opts: &FlattenOptions) -> Flattened {
    let mut w = Walker {
        d,
        opts,
        out: Flattened::default(),
    };
    w.run(&d.entities, None);
    if opts.include_paper && !d.paper_entities.is_empty() {
        w.run(&d.paper_entities, Some("Paper Space"));
    }
    w.out
}

struct Walker<'a> {
    d: &'a DxfDrawing,
    opts: &'a FlattenOptions,
    out: Flattened,
}

impl Walker<'_> {
    fn run(&mut self, ents: &[DxfEntity], one_block: Option<&str>) {
        let paper_inst = one_block.filter(|_| self.opts.instances).map(|name| {
            self.out.instances.push(Instance {
                name: name.to_string(),
                at: Point::ZERO,
            });
            self.out.instances.len() - 1
        });
        for e in ents {
            let (inst, name_at) = match &e.kind {
                DxfKind::Insert(i) if self.opts.instances => (true, Some((i.block.clone(), i.pos))),
                DxfKind::Dimension(_) if self.opts.instances && !self.opts.dims_as_objects => {
                    (true, Some(("Dimension".to_string(), Point::ZERO)))
                }
                _ => (false, None),
            };
            let leaves = self.expand(e, 0);
            let tag = if let Some(p) = paper_inst {
                Some(p)
            } else if inst && leaves.len() > 1 {
                let (name, at) = name_at.unwrap_or_default();
                self.out.instances.push(Instance { name, at });
                Some(self.out.instances.len() - 1)
            } else {
                None
            };
            for l in leaves {
                self.out.items.push(Flat {
                    entity: l,
                    inst: tag,
                });
            }
        }
    }

    fn layer_color(&self, layer: &str) -> DxfColor {
        let d = self.d;
        d.layer(layer).map_or(DxfColor::Aci(7), |l| l.color)
    }

    /// `e` replaced by its leaves, in the frame `e` is written in.
    fn expand(&mut self, e: &DxfEntity, depth: usize) -> Vec<DxfEntity> {
        let d = self.d;
        match &e.kind {
            DxfKind::Insert(ins) => self.insert(e, ins, depth),
            DxfKind::Dimension(dim) => {
                if self.opts.dims_as_objects && linear_dimension(dim).is_some() {
                    return vec![e.clone()];
                }
                if !dim.block.is_empty()
                    && d.block(&dim.block).is_some()
                    && depth < MAX_INSERT_DEPTH
                {
                    // The anonymous block is drawn at the origin of the
                    // dimension's own frame.
                    let ins = DxfInsert {
                        block: dim.block.clone(),
                        pos: Point::ZERO,
                        scale: (1.0, 1.0),
                        rotation_deg: 0.0,
                        attribs: Vec::new(),
                        columns: 1,
                        rows: 1,
                        col_spacing: 0.0,
                        row_spacing: 0.0,
                    };
                    let stand_in = DxfEntity {
                        kind: DxfKind::Insert(Box::new(ins.clone())),
                        props: e.props.clone(),
                    };
                    return self.insert(&stand_in, &ins, depth);
                }
                let style = d.dim_style(&dim.style);
                dimension_entities(dim, &style, &e.props)
            }
            _ => vec![e.clone()],
        }
    }

    fn insert(&mut self, e: &DxfEntity, ins: &DxfInsert, depth: usize) -> Vec<DxfEntity> {
        if depth >= MAX_INSERT_DEPTH {
            return Vec::new();
        }
        let d = self.d;
        let Some(def) = d.block(&ins.block) else {
            if !self.out.missing_blocks.contains(&ins.block) {
                self.out.missing_blocks.push(ins.block.clone());
            }
            return Vec::new();
        };
        if !def.xref.is_empty() {
            if !self.out.xref_inserts.contains(&def.xref) {
                self.out.xref_inserts.push(def.xref.clone());
            }
            return Vec::new();
        }
        let children: Vec<DxfEntity> = def
            .entities
            .iter()
            .flat_map(|c| self.expand(c, depth + 1))
            .collect();
        let has_attribs = !ins.attribs.is_empty();
        let mut out = Vec::new();
        let (cols, rows) = (ins.columns.max(1), ins.rows.max(1));
        let spin = Xf::new(Point::ZERO, Point::ZERO, (1.0, 1.0), ins.rotation_deg);
        for row in 0..rows {
            for col in 0..cols {
                let step = Point::new(
                    f64::from(col) * ins.col_spacing,
                    f64::from(row) * ins.row_spacing,
                );
                let pos = ins.pos.add(spin.rotate(step));
                let xf = Xf::new(def.base, pos, ins.scale, ins.rotation_deg);
                for c in &children {
                    if let DxfKind::Text(t) = &c.kind {
                        // The attribute values replace the definitions.
                        if t.attdef && (has_attribs || t.invisible) {
                            continue;
                        }
                    }
                    out.push(self.inherit(xform(&xf, c, def.base), &e.props));
                }
            }
        }
        for a in &ins.attribs {
            if let DxfKind::Text(t) = &a.kind {
                if t.invisible {
                    continue;
                }
            }
            out.push(a.clone());
        }
        out
    }

    /// The BYBLOCK / layer `0` rules of an entity inside an insert.
    fn inherit(&self, mut leaf: DxfEntity, ins: &DxfProps) -> DxfEntity {
        let p = &mut leaf.props;
        if p.layer == "0" {
            p.layer = ins.layer.clone();
        }
        if p.color == DxfColor::ByBlock {
            p.color = match ins.color {
                DxfColor::ByLayer => self.layer_color(&ins.layer),
                other => other,
            };
        }
        if p.weight == WEIGHT_BY_BLOCK {
            p.weight = match ins.weight {
                WEIGHT_BY_LAYER => self
                    .d
                    .layer(&ins.layer)
                    .map_or(WEIGHT_DEFAULT, |l| l.weight),
                other => other,
            };
        }
        if p.linetype.eq_ignore_ascii_case("BYBLOCK") {
            p.linetype = if ins.linetype.is_empty() || ins.linetype.eq_ignore_ascii_case("BYLAYER")
            {
                self.d
                    .layer(&ins.layer)
                    .map_or(String::new(), |l| l.linetype.clone())
            } else {
                ins.linetype.clone()
            };
        }
        leaf
    }
}

/// `e` moved into the frame of an INSERT: `p' = pos + R S (p - base)`.
pub fn xform(xf: &Xf, e: &DxfEntity, _base: Point) -> DxfEntity {
    let uniform = xf.uniform();
    let kind = match &e.kind {
        DxfKind::Line { a, b } => DxfKind::Line {
            a: xf.point(*a),
            b: xf.point(*b),
        },
        DxfKind::Polyline {
            points,
            closed,
            bulges,
        } => {
            if uniform {
                DxfKind::Polyline {
                    points: points.iter().map(|p| xf.point(*p)).collect(),
                    closed: *closed,
                    bulges: bulges
                        .iter()
                        .map(|b| if xf.mirrored() { -b } else { *b })
                        .collect(),
                }
            } else {
                // A stretched arc is not an arc: use its samples.
                let pts = sample_polyline(points, bulges, *closed);
                let pts: Vec<Point> = pts.iter().map(|p| xf.point(*p)).collect();
                let n = pts.len();
                DxfKind::Polyline {
                    points: pts,
                    closed: *closed,
                    bulges: vec![0.0; n],
                }
            }
        }
        DxfKind::Circle { center, radius } => {
            if uniform {
                DxfKind::Circle {
                    center: xf.point(*center),
                    radius: xf.radius(*radius),
                }
            } else {
                DxfKind::Ellipse {
                    center: xf.point(*center),
                    u: xf.vector(Point::new(*radius, 0.0)),
                    v: xf.vector(Point::new(0.0, *radius)),
                    t0: 0.0,
                    t1: std::f64::consts::TAU,
                }
            }
        }
        DxfKind::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => {
            if uniform {
                let (s, e2) = (xf.angle(*start_deg), xf.angle(*end_deg));
                // A mirror reverses the sweep direction, so swap the ends.
                let (start_deg, end_deg) = if xf.mirrored() { (e2, s) } else { (s, e2) };
                DxfKind::Arc {
                    center: xf.point(*center),
                    radius: xf.radius(*radius),
                    start_deg,
                    end_deg,
                }
            } else {
                let (t0, t1) =
                    crate::dxf::geom::ellipse_range(start_deg.to_radians(), end_deg.to_radians());
                DxfKind::Ellipse {
                    center: xf.point(*center),
                    u: xf.vector(Point::new(*radius, 0.0)),
                    v: xf.vector(Point::new(0.0, *radius)),
                    t0,
                    t1,
                }
            }
        }
        DxfKind::Ellipse {
            center,
            u,
            v,
            t0,
            t1,
        } => DxfKind::Ellipse {
            center: xf.point(*center),
            u: xf.vector(*u),
            v: xf.vector(*v),
            t0: *t0,
            t1: *t1,
        },
        DxfKind::Spline {
            degree,
            closed,
            knots,
            weights,
            control,
            fit,
        } => DxfKind::Spline {
            degree: *degree,
            closed: *closed,
            knots: knots.clone(),
            weights: weights.clone(),
            control: control.iter().map(|p| xf.point(*p)).collect(),
            fit: fit.iter().map(|p| xf.point(*p)).collect(),
        },
        DxfKind::Hatch(h) => DxfKind::Hatch(Box::new(DxfHatch {
            loops: h
                .loops
                .iter()
                .map(|l| HatchLoop {
                    points: l.points.iter().map(|p| xf.point(*p)).collect(),
                    external: l.external,
                })
                .collect(),
            pattern: h.pattern.clone(),
            solid: h.solid,
            angle_deg: h.angle_deg + xf.rot_deg,
            scale: h.scale * xf.sx.abs(),
            spacing: h.spacing.map(|s| s * xf.sx.abs()),
        })),
        DxfKind::Face { points, filled } => DxfKind::Face {
            points: points.iter().map(|p| xf.point(*p)).collect(),
            filled: *filled,
        },
        DxfKind::Marker { pos } => DxfKind::Marker {
            pos: xf.point(*pos),
        },
        DxfKind::Text(t) => {
            let mut t = (**t).clone();
            t.pos = xf.point(t.pos);
            t.height *= xf.height_scale();
            t.wrap_width *= xf.sx.abs();
            t.angle_deg = xf.angle(t.angle_deg);
            if xf.mirrored() && xf.sx < 0.0 {
                // Mirrored text reads backwards in the block; keep it
                // readable by turning it a half turn.
                t.angle_deg += 180.0;
            }
            t.width_factor *= (xf.sx / xf.sy).abs();
            DxfKind::Text(Box::new(t))
        }
        DxfKind::Dimension(d) => DxfKind::Dimension(Box::new(xform_dimension(xf, d))),
        DxfKind::Leader { points, arrow } => DxfKind::Leader {
            points: points.iter().map(|p| xf.point(*p)).collect(),
            arrow: *arrow,
        },
        DxfKind::MLeader(m) => DxfKind::MLeader(Box::new(DxfMLeader {
            lines: m
                .lines
                .iter()
                .map(|l| l.iter().map(|p| xf.point(*p)).collect())
                .collect(),
            text: m.text.clone(),
            text_pos: m.text_pos.map(|p| xf.point(p)),
            height: m.height * xf.height_scale(),
            angle_deg: xf.angle(m.angle_deg),
            arrow: m.arrow,
        })),
        // Inserts are expanded before they get here.
        DxfKind::Insert(_) => return e.clone(),
    };
    DxfEntity {
        kind,
        props: e.props.clone(),
    }
}
