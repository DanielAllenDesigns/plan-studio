//! Electrical defaults of a plan: the height each kind of device is placed at.
//!
//! Chief places receptacles at 12", switches at 48" and kitchen counter
//! receptacles at 44" and lets the plan change them. The overrides live in
//! `Project::electrical_defaults` (as JSON, so plan-core needs no electrical
//! types); a kind without an entry uses [`DeviceKind::default_height`]. The
//! Electrical Service Specification edits them and Auto Place Outlets reads
//! the outlet and counter heights.

use crate::device::{Device, DeviceKind, COUNTER_OUTLET_HEIGHT};
use plan_core::Project;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Key of the kitchen counter receptacle height (a height, not a kind).
pub const COUNTER_OUTLET_KEY: &str = "Counter Outlet";

/// Per-kind default heights (inches above the finished floor), keyed by
/// [`DeviceKind::name`] or [`COUNTER_OUTLET_KEY`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ElectricalDefaults {
    #[serde(default)]
    pub heights: BTreeMap<String, f64>,
}

impl ElectricalDefaults {
    /// The defaults stored in `project` (the built-in ones when none are).
    pub fn load(project: &Project) -> Self {
        project
            .electrical_defaults
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default()
    }

    /// Stores these defaults in `project`; no overrides clears the record.
    pub fn store(&self, project: &mut Project) {
        project.electrical_defaults = if self.heights.is_empty() {
            None
        } else {
            serde_json::to_value(self).ok()
        };
    }

    /// The height `kind` is placed at.
    pub fn height(&self, kind: DeviceKind) -> f64 {
        self.heights
            .get(kind.name())
            .copied()
            .unwrap_or_else(|| kind.default_height())
    }

    /// The height of a kitchen counter receptacle.
    pub fn counter_height(&self) -> f64 {
        self.heights
            .get(COUNTER_OUTLET_KEY)
            .copied()
            .unwrap_or(COUNTER_OUTLET_HEIGHT)
    }

    /// Makes `height` the default of `kind`; the built-in height removes the override.
    pub fn set_height(&mut self, kind: DeviceKind, height: f64) {
        Self::set(
            &mut self.heights,
            kind.name(),
            height,
            kind.default_height(),
        );
    }

    /// Makes `height` the kitchen counter receptacle default.
    pub fn set_counter_height(&mut self, height: f64) {
        Self::set(
            &mut self.heights,
            COUNTER_OUTLET_KEY,
            height,
            COUNTER_OUTLET_HEIGHT,
        );
    }

    fn set(map: &mut BTreeMap<String, f64>, key: &str, height: f64, builtin: f64) {
        if (height - builtin).abs() < 1e-9 {
            map.remove(key);
        } else {
            map.insert(key.to_string(), height);
        }
    }

    /// Puts `d` at the default height of its kind.
    pub fn apply(&self, d: &mut Device) {
        d.height = self.height(d.kind);
    }

    /// Is `kind` at its built-in height?
    pub fn is_builtin(&self, kind: DeviceKind) -> bool {
        !self.heights.contains_key(kind.name())
    }
}
