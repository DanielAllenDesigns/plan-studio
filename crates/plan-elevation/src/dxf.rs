//! DXF output of a [`Drawing`]: its lines on layers by weight class.

use crate::drawing::{Drawing, EdgeKind};
use plan_core::{CadItem, Project};

impl Drawing {
    /// The layer a line goes on in [`Drawing::to_dxf`]: hatch and annotation
    /// strokes have their own layers; every other line sits on its weight
    /// layer (`Heavy`, `Medium`, `Light`, or `Hidden` for dashed lines).
    pub fn dxf_layer(prefix: &str, line: &crate::drawing::Line2) -> String {
        match line.kind {
            EdgeKind::Hatch => format!("{prefix}, Hatch"),
            EdgeKind::Annotation => format!("{prefix}, Annotation"),
            EdgeKind::Hidden => format!("{prefix}, Hidden"),
            _ => format!("{prefix}, {}", line.weight.name()),
        }
    }

    /// The drawing as an R12 ASCII DXF in inches of the building: lines on
    /// layers `"{layer_prefix}, Heavy|Medium|Light|Hidden|Hatch|Annotation"`
    /// (see [`Drawing::dxf_layer`]) and the annotation text on
    /// `"{layer_prefix}, Text"`. `None` for a drawing without lines.
    pub fn to_dxf(&self, layer_prefix: &str) -> Option<String> {
        if self.lines.is_empty() {
            return None;
        }
        let mut sheet = Project::new(layer_prefix);
        for l in &self.lines {
            sheet.add_cad(
                0,
                Self::dxf_layer(layer_prefix, l),
                CadItem::Line { a: l.a, b: l.b },
            );
        }
        for (at, text) in &self.texts {
            sheet.add_cad(
                0,
                format!("{layer_prefix}, Text"),
                CadItem::Text {
                    pos: *at,
                    text: text.clone(),
                    height: crate::drawing::TEXT_H,
                    angle: 0.0,
                },
            );
        }
        Some(plan_core::write_dxf(&sheet, 0, &[]))
    }

    /// The distinct DXF layers [`Drawing::to_dxf`] uses, in first-use order.
    pub fn dxf_layers(&self, layer_prefix: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for l in &self.lines {
            let name = Self::dxf_layer(layer_prefix, l);
            if !out.contains(&name) {
                out.push(name);
            }
        }
        if !self.texts.is_empty() {
            out.push(format!("{layer_prefix}, Text"));
        }
        out
    }
}
