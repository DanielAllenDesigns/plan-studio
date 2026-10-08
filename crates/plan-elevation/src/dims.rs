//! Automatic elevation dimensions: vertical strings left of the building.
//!
//! * **Floor-to-floor** (outer string): one dimension from each finished
//!   floor to the next, and from the top floor to its top of plate.
//! * **Openings** (inner string): per floor, the floor level, every sill and
//!   head of the doors and windows seen in the view, and the top of plate.
//! * **Overall**: the lowest floor to the highest plate.
//!
//! The values come from the project's levels and openings, so they follow the
//! model exactly; each one is also kept in [`Drawing::dims`] with its value.

use crate::drawing::{Drawing, EdgeKind, Line2, LineWeight, TEXT_CHAR_W, TEXT_H};
use plan_core::units::fmt_ft_in_frac;
use plan_core::{FloorKind, Id, Point, Project};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// What a dimension measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DimKind {
    /// Finished floor to the next finished floor.
    FloorToFloor,
    /// The top floor's finished floor to its top of plate.
    FloorToPlate,
    /// One step of the opening string: floor, sill, head or plate to the next.
    Opening,
    /// Lowest floor to the highest top of plate.
    Overall,
}

/// One dimension of an elevation string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElevDim {
    pub kind: DimKind,
    /// Lower level, inches (drawing Y).
    pub from: f64,
    /// Upper level, inches.
    pub to: f64,
    /// X of the dimension line in the drawing.
    pub x: f64,
    /// The dimension text, feet-inches to 1/8".
    pub text: String,
}

impl ElevDim {
    /// The measured length, inches.
    pub fn value(&self) -> f64 {
        self.to - self.from
    }
}

/// Which strings to draw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DimOptions {
    pub floor_to_floor: bool,
    pub openings: bool,
    pub overall: bool,
}

impl Default for DimOptions {
    fn default() -> Self {
        Self {
            floor_to_floor: true,
            openings: true,
            overall: true,
        }
    }
}

/// Distance from the building to the first dimension line, inches.
const FIRST_LINE_GAP: f64 = 14.0;
/// Space between a string's text and the next string's line, inches.
const STRING_GAP: f64 = 10.0;
/// Half length of the tick at each dimension end, inches.
const TICK: f64 = 3.0;
/// Levels closer than this are one level, inches.
const LEVEL_TOL: f64 = 0.25;
/// Most levels one floor's opening string may have.
const MAX_LEVELS: usize = 10;

fn text_width(s: &str) -> f64 {
    TEXT_CHAR_W * s.chars().count() as f64
}

/// Top of the tallest drawn wall of a floor, relative to the first floor.
fn plate_of(project: &Project, floor: usize) -> Option<f64> {
    let f = &project.floors[floor];
    f.walls
        .iter()
        .filter(|w| !w.flags.invisible && !w.flags.railing)
        .map(|w| w.height)
        .reduce(f64::max)
        .map(|h| f.elevation + h)
}

fn push_dim(drawing: &mut Drawing, dim: ElevDim, tick_at: &[f64]) {
    drawing.lines.push(Line2 {
        a: Point::new(dim.x, dim.from),
        b: Point::new(dim.x, dim.to),
        weight: LineWeight::Light,
        kind: EdgeKind::Annotation,
    });
    for &y in tick_at {
        for (a, b) in [
            (
                Point::new(dim.x - TICK, y - TICK),
                Point::new(dim.x + TICK, y + TICK),
            ),
            (
                Point::new(dim.x - TICK * 2.0, y),
                Point::new(dim.x + TICK * 2.0, y),
            ),
        ] {
            drawing.lines.push(Line2 {
                a,
                b,
                weight: LineWeight::Light,
                kind: EdgeKind::Annotation,
            });
        }
    }
    let at = Point::new(
        dim.x - TICK - text_width(&dim.text),
        0.5 * (dim.from + dim.to) - 0.35 * TEXT_H,
    );
    drawing.texts.push((at, dim.text.clone()));
    drawing.dims.push(dim);
}

/// Draws one string through `levels` (ascending) at `x`; returns the width of
/// its widest text.
fn chain(drawing: &mut Drawing, kind: DimKind, levels: &[f64], x: f64) -> f64 {
    let mut widest = 0.0_f64;
    for w in levels.windows(2) {
        let (from, to) = (w[0], w[1]);
        let text = fmt_ft_in_frac(to - from, 8);
        widest = widest.max(text_width(&text));
        push_dim(
            drawing,
            ElevDim {
                kind,
                from,
                to,
                x,
                text,
            },
            &[from, to],
        );
    }
    widest
}

fn dedup_levels(mut v: Vec<f64>) -> Vec<f64> {
    v.sort_by(f64::total_cmp);
    v.dedup_by(|a, b| (*a - *b).abs() < LEVEL_TOL);
    v
}

/// Adds the dimension strings to `drawing`, left of `left_x` (the building's
/// left edge). `seen` are the ids of the objects that show in the view; with
/// `known` false every opening counts. Returns the x of the left end of the
/// strings (so level callouts can stay clear of them), or `None` when nothing
/// was drawn.
pub(crate) fn add_dimensions(
    drawing: &mut Drawing,
    project: &Project,
    left_x: f64,
    seen: &HashSet<Id>,
    known: bool,
    opts: &DimOptions,
) -> Option<f64> {
    let normal: Vec<usize> = (0..project.floors.len())
        .filter(|&i| project.floors[i].kind == FloorKind::Normal)
        .filter(|&i| plate_of(project, i).is_some())
        .collect();
    if normal.is_empty() {
        return None;
    }
    let mut order = normal.clone();
    order.sort_by(|&a, &b| {
        project.floors[a]
            .elevation
            .total_cmp(&project.floors[b].elevation)
    });
    let first = project.floors[order[0]].elevation;
    let last = *order.last()?;
    let top = plate_of(project, last)?;

    let mut x = left_x - FIRST_LINE_GAP;
    let mut extent = left_x;
    let before = drawing.dims.len();

    if opts.openings {
        let mut widest = 0.0_f64;
        let mut any = false;
        for &i in &order {
            let f = &project.floors[i];
            let plate = plate_of(project, i)?;
            let mut levels = vec![f.elevation, plate];
            let mut count = 0;
            for o in &f.openings {
                if known && !seen.contains(&o.id) {
                    continue;
                }
                count += 1;
                for y in [
                    f.elevation + o.sill_height,
                    f.elevation + o.sill_height + o.height,
                ] {
                    if y > f.elevation + LEVEL_TOL && y < plate - LEVEL_TOL {
                        levels.push(y);
                    }
                }
            }
            if count == 0 {
                continue;
            }
            let mut levels = dedup_levels(levels);
            levels.truncate(MAX_LEVELS);
            if levels.len() > 1 {
                widest = widest.max(chain(drawing, DimKind::Opening, &levels, x));
                any = true;
            }
        }
        if any {
            extent = x - TICK - widest;
            x = extent - STRING_GAP;
        }
    }

    if opts.floor_to_floor {
        let levels: Vec<f64> = order.iter().map(|&i| project.floors[i].elevation).collect();
        let mut widest = 0.0_f64;
        let mut drawn = false;
        for w in levels.windows(2) {
            let text = fmt_ft_in_frac(w[1] - w[0], 8);
            widest = widest.max(text_width(&text));
            push_dim(
                drawing,
                ElevDim {
                    kind: DimKind::FloorToFloor,
                    from: w[0],
                    to: w[1],
                    x,
                    text,
                },
                &[w[0], w[1]],
            );
            drawn = true;
        }
        if let Some(&e) = levels.last() {
            if top - e > LEVEL_TOL {
                let text = fmt_ft_in_frac(top - e, 8);
                widest = widest.max(text_width(&text));
                push_dim(
                    drawing,
                    ElevDim {
                        kind: DimKind::FloorToPlate,
                        from: e,
                        to: top,
                        x,
                        text,
                    },
                    &[e, top],
                );
                drawn = true;
            }
        }
        if drawn {
            extent = x - TICK - widest;
            x = extent - STRING_GAP;
        }
    }

    if opts.overall && top - first > LEVEL_TOL && order.len() > 1 {
        let text = fmt_ft_in_frac(top - first, 8);
        let widest = text_width(&text);
        push_dim(
            drawing,
            ElevDim {
                kind: DimKind::Overall,
                from: first,
                to: top,
                x,
                text,
            },
            &[first, top],
        );
        extent = x - TICK - widest;
    }

    (drawing.dims.len() > before).then_some(extent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_merge_within_the_tolerance() {
        let v = dedup_levels(vec![80.0, 0.0, 80.1, 109.0]);
        assert_eq!(v, vec![0.0, 80.0, 109.0]);
    }

    #[test]
    fn a_dimension_reports_its_value() {
        let d = ElevDim {
            kind: DimKind::FloorToFloor,
            from: 10.0,
            to: 129.375,
            x: 0.0,
            text: String::new(),
        };
        assert!((d.value() - 119.375).abs() < 1e-9);
    }
}
