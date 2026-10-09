//! What a saved plan view keeps beyond its layer set and camera (Plan View
//! Specification, manual pp. 177 to 181): the Saved flag, Remember
//! Zoom/Rotation and the rotation itself, Show Color, Link to Layout, the
//! Save Options and the Selected Defaults (the Default Set, the saved default
//! of each annotation kind and the current CAD layer).

use crate::layer_sets::{SavedPlanView, TEMPLATE_PLAN_VIEWS};
use crate::model::Project;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The dozen saved plan views a Residential template carries (named in the
/// tutorial guide and in Daniel's working template; the Residential template
/// lists twelve views and its decode reads names only, DECISIONS DS13).
pub const STARTER_VIEWS: [&str; 12] = [
    "Working Plan View",
    "Presentation Plan View",
    "Floor Plan View Dimensioned",
    "Foundation Plan View",
    "Framing, Floor Plan View",
    "Framing, Ceiling Plan View",
    "Framing, Roof Plan View",
    "Electrical Plan View",
    "HVAC Plan View",
    "Kitchen and Bath Plan View",
    "Plot Plan View",
    "Roof Plan View",
];

/// What happens to a change of the view's attributes made outside the
/// specification dialog (Save Options).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SaveOption {
    /// Ask when the view is closed.
    #[default]
    Prompt,
    /// Every change is saved at once.
    Always,
    /// Changes are dropped when the view is closed.
    Never,
}

impl SaveOption {
    pub const ALL: [SaveOption; 3] = [SaveOption::Prompt, SaveOption::Always, SaveOption::Never];

    pub fn label(self) -> &'static str {
        match self {
            SaveOption::Prompt => "Prompt to Save",
            SaveOption::Always => "Always Save",
            SaveOption::Never => "Never Save",
        }
    }
}

fn is_yes(b: &bool) -> bool {
    *b
}

/// The Plan View Specification values a [`crate::SavedPlanView`] keeps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlanViewSpec {
    /// A saved view (all of them in a plan file; an unsaved view is only a
    /// window and is not stored).
    #[serde(skip_serializing_if = "is_yes")]
    pub saved: bool,
    /// Keep the extents and the rotation of the view for the next time it is
    /// opened.
    #[serde(skip_serializing_if = "is_yes")]
    pub remember_zoom_rotation: bool,
    /// The view's rotation in degrees counterclockwise, relative to north up,
    /// as `Rotate Plan View` typed it (-180 to 180).
    #[serde(skip_serializing_if = "is_zero")]
    pub rotation_deg: f64,
    /// Show Color; off draws the view in black and white.
    #[serde(skip_serializing_if = "is_yes")]
    pub show_color: bool,
    /// A layout view sent from this plan view stays linked to it.
    #[serde(skip_serializing_if = "is_yes")]
    pub link_to_layout: bool,
    pub save_option: SaveOption,
    /// Name of the Default Set the view uses; empty when the individual
    /// picks in [`selected`](Self::selected) are used.
    pub default_set: String,
    /// The saved default of each kind the view uses, by
    /// [`crate::defaults::saved::SavedKind::id`].
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub selected: BTreeMap<String, String>,
    /// The Current CAD Layer of the view; empty keeps it.
    pub cad_layer: String,
    /// The view has changes that were not saved (Prompt to Save asks about
    /// them). Session state, never stored.
    #[serde(skip)]
    pub dirty: bool,
}

fn is_zero(d: &f64) -> bool {
    *d == 0.0
}

impl Default for PlanViewSpec {
    fn default() -> Self {
        Self {
            saved: true,
            remember_zoom_rotation: true,
            rotation_deg: 0.0,
            show_color: true,
            link_to_layout: true,
            save_option: SaveOption::Prompt,
            default_set: String::new(),
            selected: BTreeMap::new(),
            cad_layer: String::new(),
            dirty: false,
        }
    }
}

/// A rotation shown the way the dialog shows it: -180 to 180 degrees (270
/// reads -90).
pub fn normalize_deg(deg: f64) -> f64 {
    let mut d = deg % 360.0;
    if d > 180.0 {
        d -= 360.0;
    } else if d <= -180.0 {
        d += 360.0;
    }
    if d == 0.0 {
        0.0
    } else {
        d
    }
}

impl Project {
    /// Adds the [`STARTER_VIEWS`] the plan lacks, each on the layer set of its
    /// name (the working template's: a copy of the active set when the plan
    /// has no such set). Returns how many were added.
    pub fn seed_starter_plan_views(&mut self) -> usize {
        let mut added = 0;
        for view in STARTER_VIEWS {
            if self.plan_view(view).is_some() {
                continue;
            }
            let set = TEMPLATE_PLAN_VIEWS
                .iter()
                .find(|(v, _)| *v == view)
                .map_or(self.layer_sets.active.as_str(), |(_, s)| *s)
                .to_string();
            if self.layer_sets.get(&set).is_none() {
                let active = self.layer_sets.active.clone();
                if !self.layer_sets.copy_set(&active, &set) {
                    continue;
                }
            }
            self.plan_views.push(SavedPlanView::new(view, set));
            added += 1;
        }
        added
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_reads_between_minus_180_and_180() {
        assert_eq!(normalize_deg(270.0), -90.0);
        assert_eq!(normalize_deg(90.0), 90.0);
        assert_eq!(normalize_deg(-270.0), 90.0);
        assert_eq!(normalize_deg(180.0), 180.0);
        assert_eq!(normalize_deg(-180.0), 180.0);
        assert_eq!(normalize_deg(360.0), 0.0);
        assert_eq!(normalize_deg(450.0), 90.0);
        assert_eq!(normalize_deg(-0.0), 0.0);
    }

    #[test]
    fn the_starter_views_are_twelve_distinct_working_views() {
        let mut p = Project::new("T");
        assert_eq!(STARTER_VIEWS.len(), 12);
        assert_eq!(p.seed_starter_plan_views(), 12);
        assert_eq!(p.seed_starter_plan_views(), 0);
        for v in STARTER_VIEWS {
            let view = p.plan_view(v).unwrap();
            assert!(p.layer_sets.get(&view.layer_set).is_some(), "{v}");
        }
        assert_eq!(
            p.plan_view("Plot Plan View").unwrap().layer_set,
            "Plot Plan Layer Set"
        );
    }

    #[test]
    fn the_default_spec_stores_nothing() {
        let s = PlanViewSpec::default();
        let v = serde_json::to_value(&s).unwrap();
        let o = v.as_object().unwrap();
        assert!(o.contains_key("save_option"));
        assert!(!o.contains_key("rotation_deg"));
        assert!(!o.contains_key("saved"));
        let back: PlanViewSpec = serde_json::from_value(v).unwrap();
        assert_eq!(back, s);
        let rotated = PlanViewSpec {
            rotation_deg: 33.5,
            show_color: false,
            ..PlanViewSpec::default()
        };
        let back: PlanViewSpec =
            serde_json::from_str(&serde_json::to_string(&rotated).unwrap()).unwrap();
        assert_eq!(back, rotated);
    }
}
