//! Layout templates: a layout saved as a starting point (File > Save As
//! Template for layouts).
//!
//! A template is the layout's sheet, title block, margins, layers and pages
//! with their boxes and annotations, minus what only makes sense in the plan
//! it came from: camera, perspective and placed-schedule boxes name ids of
//! that plan. It is stored as JSON, one file per template, under
//! `~/.plan-studio/templates` (the application reads and writes the files;
//! this module is the pure part).

use crate::model::{BoxSource, Layout};
use plan_docs::SheetSize;
use serde::{Deserialize, Serialize};

/// The file format version written to template files.
pub const TEMPLATE_VERSION: u32 = 1;

/// Extension of a layout template file, after the template's file name.
pub const TEMPLATE_EXTENSION: &str = ".layout.json";

/// A template file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutTemplate {
    pub version: u32,
    /// The name the template is listed under.
    pub name: String,
    /// New layouts of this template's sheet size start from it (one template
    /// per sheet size can be the default).
    #[serde(default)]
    pub default_for_sheet: bool,
    pub layout: Layout,
}

/// Is a box of this source bound to ids of the plan it was made in?
fn is_plan_bound(source: &BoxSource) -> bool {
    matches!(
        source,
        BoxSource::Camera { .. } | BoxSource::Perspective { .. } | BoxSource::PlacedSchedule { .. }
    )
}

impl Layout {
    /// This layout as a template: the same pages, boxes and annotations
    /// without the boxes that point at cameras and placed schedules of this
    /// plan (their ids mean nothing in another one).
    pub fn to_template(&self) -> Layout {
        let mut out = self.clone();
        for page in &mut out.pages {
            page.boxes.retain(|b| !is_plan_bound(&b.source));
        }
        out
    }
}

impl LayoutTemplate {
    /// A template of `layout` called `name`.
    pub fn new(name: &str, layout: &Layout, default_for_sheet: bool) -> Self {
        Self {
            version: TEMPLATE_VERSION,
            name: name.trim().to_string(),
            default_for_sheet,
            layout: layout.to_template(),
        }
    }

    /// The file's JSON text.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// Reads a template file's text. Refuses a newer format than this build
    /// knows.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let t: LayoutTemplate =
            serde_json::from_str(text).map_err(|e| format!("not a layout template: {e}"))?;
        if t.version > TEMPLATE_VERSION {
            return Err(format!(
                "the template was saved by a newer version (format {})",
                t.version
            ));
        }
        Ok(t)
    }

    /// The sheet size the template is for.
    pub fn sheet(&self) -> SheetSize {
        self.layout.sheet
    }

    /// The layout a new project starts with from this template: its pages
    /// and settings under `name`.
    pub fn instantiate(&self, name: &str) -> Layout {
        let mut l = self.layout.clone();
        l.name = name.to_string();
        l
    }
}

/// The file name (without the extension) a template called `name` is saved
/// under: letters, digits, spaces, dashes and underscores stay, anything else
/// becomes `_`. Empty names give `Template`.
pub fn template_file_stem(name: &str) -> String {
    let s: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '(' | ')') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim_matches(|c: char| c == '.' || c == ' ').to_string();
    if s.is_empty() {
        "Template".to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::Point;
    use plan_docs::Scale;

    fn layout() -> Layout {
        let mut l = Layout::new("Smith Layout", SheetSize::ArchC);
        let p = l.add_page(1, "Plan");
        let rect = (Point::new(1.0, 1.0), Point::new(6.0, 5.0));
        for (id, source) in [
            (
                1,
                BoxSource::PlanView {
                    floor: 0,
                    layer_set: "Floor Plan".into(),
                },
            ),
            (2, BoxSource::Camera { camera_id: 7 }),
            (3, BoxSource::Perspective { camera_id: 7 }),
            (4, BoxSource::PlacedSchedule { floor: 0, id: 3 }),
            (5, BoxSource::SheetIndex),
            (6, BoxSource::text("NOTES", 10.0)),
        ] {
            p.boxes.push(crate::model::LayoutBox::new(
                id,
                rect,
                source,
                Scale::QuarterInch,
            ));
        }
        p.add_text(Point::new(2.0, 2.0), "GENERAL NOTES", 0.125);
        l.add_page(2, "Page Template").template_page = true;
        l
    }

    #[test]
    fn a_template_drops_the_boxes_bound_to_this_plan() {
        let t = LayoutTemplate::new("  Presentation  ", &layout(), true);
        assert_eq!(t.name, "Presentation");
        let kept: Vec<u64> = t.layout.pages[0].boxes.iter().map(|b| b.id).collect();
        assert_eq!(kept, [1, 5, 6]);
        // Annotations, the template page and the sheet stay.
        assert_eq!(t.layout.pages[0].cad.len(), 1);
        assert!(t.layout.pages[1].template_page);
        assert_eq!(t.sheet(), SheetSize::ArchC);
    }

    #[test]
    fn a_template_round_trips_through_its_json() {
        let t = LayoutTemplate::new("Presentation", &layout(), true);
        let json = t.to_json().unwrap();
        let back = LayoutTemplate::from_json(&json).unwrap();
        assert_eq!(back, t);
        assert!(back.default_for_sheet);
        // The flag is optional in the file.
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v.as_object_mut().unwrap().remove("default_for_sheet");
        let old = LayoutTemplate::from_json(&v.to_string()).unwrap();
        assert!(!old.default_for_sheet);
        // A newer format and plain nonsense are refused with a reason.
        v["version"] = serde_json::json!(TEMPLATE_VERSION + 1);
        assert!(LayoutTemplate::from_json(&v.to_string())
            .unwrap_err()
            .contains("newer"));
        assert!(LayoutTemplate::from_json("{}")
            .unwrap_err()
            .contains("not a layout template"));
    }

    #[test]
    fn instantiating_keeps_the_pages_under_the_new_name() {
        let t = LayoutTemplate::new("Presentation", &layout(), false);
        let l = t.instantiate("Jones Layout");
        assert_eq!(l.name, "Jones Layout");
        assert_eq!(l.pages.len(), 2);
        assert_eq!(l.sheet, SheetSize::ArchC);
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(template_file_stem("Arch C / Plans"), "Arch C _ Plans");
        assert_eq!(template_file_stem("  ../../etc "), "______etc");
        assert_eq!(template_file_stem("   "), "Template");
        assert_eq!(template_file_stem("Plan (18x24)"), "Plan (18x24)");
        assert_eq!(template_file_stem("a:b*c?"), "a_b_c_");
    }
}
