//! Electrical Defaults of a plan (manual pp. 691-693; E-11, E-19..E-22).
//!
//! The Electrical Defaults dialog of Chief has four parts, all kept in
//! `Project::electrical_defaults` (as JSON, so plan-core needs no electrical
//! types):
//!
//! * Default Library Objects: the symbol each Electrical Tool places
//!   ([`ElectricalDefaults::object`]; here one of the built-in
//!   [`DeviceKind`]s of the tool's family).
//! * Default Heights, four groups and a Use Default Heights switch: Outlet
//!   (receptacles and the phone, data and TV jacks), Switch (switches,
//!   doorbells and thermostats), Above Base Cabinet (measured up from the
//!   counter top) and On Cabinet Side (measured up from the cabinet's bottom).
//!   DECISIONS 87 and 89 had one height per kind; the four groups replace them
//!   and a plan saved with the old per-kind heights still reads
//!   ([`ElectricalDefaults::load`] migrates them).
//! * Electrical Connection Defaults ([`ConnectionDefaults`]): the curvature
//!   ratio of a new spline, its line style, an arrow and a label.
//! * Rope Light Defaults ([`RopeSpec`]).
//!
//! A plan with every default stores nothing.

use crate::device::{Device, DeviceKind, COUNTER_OUTLET_HEIGHT, OUTLET_HEIGHT, SWITCH_HEIGHT};
use crate::layer::Arrow;
use crate::rope::RopeSpec;
use plan_core::{LineStyle, Project};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Key of the legacy kitchen counter receptacle height (an absolute height).
/// Plans saved before the four groups keep it; it migrates to
/// [`ElectricalDefaults::above_base_cabinet`].
pub const COUNTER_OUTLET_KEY: &str = "Counter Outlet";

/// The counter height the legacy absolute counter outlet height is measured
/// against when it migrates, inches (a standard 36" base cabinet).
pub const STANDARD_COUNTER_HEIGHT: f64 = 36.0;
/// Default Above Base Cabinet height, inches above the counter top
/// (44" above the floor over a 36" counter, the old counter outlet height).
pub const ABOVE_BASE_CABINET_HEIGHT: f64 = COUNTER_OUTLET_HEIGHT - STANDARD_COUNTER_HEIGHT;
/// Default On Cabinet Side height, inches up from the cabinet's bottom
/// (manual p. 693: 32 in, 800 mm).
pub const ON_CABINET_SIDE_HEIGHT: f64 = 32.0;
/// Default curvature ratio of an Electrical Connection (sagitta over chord).
pub const DEFAULT_CURVATURE: f64 = 0.2;

/// The height groups of the Electrical Defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeightGroup {
    /// Outlets and the phone, data and TV jacks.
    Outlet,
    /// Switches, doorbells and thermostats.
    Switch,
}

/// Where a wall or cabinet device is placed, which picks the height group.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeightContext {
    /// On a wall: the Outlet or Switch height.
    Wall,
    /// On the wall above a base cabinet: the Above Base Cabinet height up
    /// from `counter_top`.
    AboveCounter { counter_top: f64 },
    /// On the side of a cabinet or soffit that spans `bottom` to `top`: the
    /// On Cabinet Side height up from the bottom, kept on the box.
    CabinetSide { bottom: f64, top: f64 },
}

/// Electrical Connection Defaults: what a spline looks like when first drawn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectionDefaults {
    /// Sagitta over chord of the first arc (0 is a straight line).
    pub curvature_ratio: f64,
    pub line_style: LineStyle,
    pub arrow: Arrow,
    /// Label panel: text a new connection carries.
    pub label: String,
}

impl Default for ConnectionDefaults {
    fn default() -> Self {
        Self {
            curvature_ratio: DEFAULT_CURVATURE,
            line_style: LineStyle::Dashed,
            arrow: Arrow::None,
            label: String::new(),
        }
    }
}

/// One Electrical Tool in the Default Library Objects list: the name Chief
/// shows, the kind the tool places unless another is chosen and the kinds the
/// Library button offers for it.
#[derive(Debug, Clone, Copy)]
pub struct ToolSlot {
    pub key: &'static str,
    pub builtin: DeviceKind,
    pub choices: &'static [DeviceKind],
}

/// The tools whose symbol can be chosen. The Light tool places a ceiling,
/// recessed or pendant fixture; the Wall Light tool an interior or exterior
/// one (E-31); the outlet and switch tools one of their family.
pub const TOOL_SLOTS: &[ToolSlot] = &[
    ToolSlot {
        key: "110V Outlet",
        builtin: DeviceKind::Outlet110,
        choices: &[DeviceKind::Outlet110, DeviceKind::Outlet110Quad],
    },
    ToolSlot {
        key: "GFCI Outlet",
        builtin: DeviceKind::Gfci,
        choices: &[DeviceKind::Gfci],
    },
    ToolSlot {
        key: "220V Outlet",
        builtin: DeviceKind::Outlet220,
        choices: &[DeviceKind::Outlet220, DeviceKind::OutletDedicated],
    },
    ToolSlot {
        key: "Switch",
        builtin: DeviceKind::Switch,
        choices: &[DeviceKind::Switch, DeviceKind::SwitchDimmer],
    },
    ToolSlot {
        key: "Light",
        builtin: DeviceKind::CeilingLight,
        choices: &[
            DeviceKind::CeilingLight,
            DeviceKind::RecessedCan,
            DeviceKind::PendantLight,
        ],
    },
    ToolSlot {
        key: "Wall Light",
        builtin: DeviceKind::WallSconce,
        choices: &[DeviceKind::WallSconce],
    },
    ToolSlot {
        key: "Wall Light (Exterior)",
        builtin: DeviceKind::WallLightExterior,
        choices: &[DeviceKind::WallLightExterior],
    },
    ToolSlot {
        key: "Ceiling Fan",
        builtin: DeviceKind::CeilingFan,
        choices: &[DeviceKind::CeilingFan],
    },
    ToolSlot {
        key: "Smoke Detector",
        builtin: DeviceKind::SmokeDetector,
        choices: &[DeviceKind::SmokeDetector],
    },
    ToolSlot {
        key: "CO Detector",
        builtin: DeviceKind::CoDetector,
        choices: &[DeviceKind::CoDetector],
    },
    ToolSlot {
        key: "Thermostat",
        builtin: DeviceKind::Thermostat,
        choices: &[DeviceKind::Thermostat],
    },
    ToolSlot {
        key: "Doorbell",
        builtin: DeviceKind::Doorbell,
        choices: &[DeviceKind::Doorbell],
    },
];

/// The slot a device of `kind` belongs to: the tool that places it, for Set
/// as Default. Weatherproof and automatic kinds (WP outlets, 3-way switches)
/// belong to no slot: the tools choose them.
pub fn slot_of(kind: DeviceKind) -> Option<&'static ToolSlot> {
    TOOL_SLOTS.iter().find(|s| {
        s.choices
            .iter()
            .any(|c| std::mem::discriminant(c) == std::mem::discriminant(&kind))
    })
}

/// The defaults stored in the project; see the module docs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ElectricalDefaults {
    /// Place new devices at the heights below; off uses the height saved with
    /// each symbol ([`DeviceKind::default_height`]).
    pub use_default_heights: bool,
    /// Outlet group, inches to the center of the device.
    pub outlet_height: f64,
    /// Switch group, inches to the center of the device.
    pub switch_height: f64,
    /// Above Base Cabinet, inches above the counter top.
    pub above_base_cabinet: f64,
    /// On Cabinet Side, inches up from the cabinet's bottom.
    pub on_cabinet_side: f64,
    /// Default Library Objects: tool name to the kind the tool places.
    pub objects: BTreeMap<String, DeviceKind>,
    pub connection: ConnectionDefaults,
    pub rope: RopeSpec,
    /// Per-kind heights of the older record (DECISIONS 87, 89), read once.
    #[serde(skip_serializing)]
    heights: BTreeMap<String, f64>,
}

impl Default for ElectricalDefaults {
    fn default() -> Self {
        Self {
            use_default_heights: true,
            outlet_height: OUTLET_HEIGHT,
            switch_height: SWITCH_HEIGHT,
            above_base_cabinet: ABOVE_BASE_CABINET_HEIGHT,
            on_cabinet_side: ON_CABINET_SIDE_HEIGHT,
            objects: BTreeMap::new(),
            connection: ConnectionDefaults::default(),
            rope: RopeSpec::default(),
            heights: BTreeMap::new(),
        }
    }
}

impl ElectricalDefaults {
    /// The defaults stored in `project` (the built-in ones when none are). A
    /// record with the older per-kind heights is migrated: the 110V outlet
    /// height becomes the Outlet group, the Switch height the Switch group
    /// and the counter outlet height the Above Base Cabinet height over a
    /// standard counter.
    pub fn load(project: &Project) -> Self {
        let mut d: ElectricalDefaults = project
            .electrical_defaults
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        d.migrate();
        d
    }

    fn migrate(&mut self) {
        let legacy = std::mem::take(&mut self.heights);
        if legacy.is_empty() {
            return;
        }
        let pick = |names: &[DeviceKind]| names.iter().find_map(|k| legacy.get(k.name()).copied());
        if let Some(h) = pick(&[
            DeviceKind::Outlet110,
            DeviceKind::Gfci,
            DeviceKind::Outlet220,
            DeviceKind::OutletDedicated,
            DeviceKind::Outlet110Quad,
        ]) {
            self.outlet_height = h;
        }
        if let Some(h) = pick(&[DeviceKind::Switch, DeviceKind::SwitchDimmer]) {
            self.switch_height = h;
        }
        if let Some(h) = legacy.get(COUNTER_OUTLET_KEY) {
            self.above_base_cabinet = (h - STANDARD_COUNTER_HEIGHT).max(0.0);
        }
    }

    /// Stores these defaults in `project`; all-default defaults clear the
    /// record.
    pub fn store(&self, project: &mut Project) {
        project.electrical_defaults = if *self == ElectricalDefaults::default() {
            None
        } else {
            serde_json::to_value(self).ok()
        };
    }

    /// Stored value of the group's height.
    pub fn group_height(&self, g: HeightGroup) -> f64 {
        match g {
            HeightGroup::Outlet => self.outlet_height,
            HeightGroup::Switch => self.switch_height,
        }
    }

    fn set_group_height(&mut self, g: HeightGroup, h: f64) {
        match g {
            HeightGroup::Outlet => self.outlet_height = h,
            HeightGroup::Switch => self.switch_height = h,
        }
    }

    /// The height `kind` is placed at on a wall. Kinds outside the Outlet and
    /// Switch groups keep their own default height, and so does everything
    /// when Use Default Heights is off.
    pub fn height(&self, kind: DeviceKind) -> f64 {
        self.height_for(kind, HeightContext::Wall)
    }

    /// The height `kind` is placed at in `ctx` (inches above the floor to the
    /// center of the device).
    pub fn height_for(&self, kind: DeviceKind, ctx: HeightContext) -> f64 {
        let Some(group) = kind.height_group() else {
            return kind.default_height();
        };
        if !self.use_default_heights {
            return kind.default_height();
        }
        match ctx {
            HeightContext::Wall => self.group_height(group),
            HeightContext::AboveCounter { counter_top } => counter_top + self.above_base_cabinet,
            HeightContext::CabinetSide { bottom, top } => {
                let (lo, hi) = (bottom.min(top), bottom.max(top));
                (bottom + self.on_cabinet_side).clamp(lo, hi)
            }
        }
    }

    /// The height of a kitchen counter receptacle over a standard counter.
    pub fn counter_height(&self) -> f64 {
        STANDARD_COUNTER_HEIGHT + self.above_base_cabinet
    }

    /// Makes `height` the Above Base Cabinet default for a standard counter.
    pub fn set_counter_height(&mut self, height: f64) {
        self.above_base_cabinet = (height - STANDARD_COUNTER_HEIGHT).max(0.0);
    }

    /// Makes `height` the default of the group `kind` belongs to; a kind
    /// without a group has no default to set.
    pub fn set_height(&mut self, kind: DeviceKind, height: f64) {
        if let Some(g) = kind.height_group() {
            self.set_group_height(g, height);
        }
    }

    /// Puts `d` at the default height of its kind on a wall.
    pub fn apply(&self, d: &mut Device) {
        d.height = self.height(d.kind);
    }

    /// Is the height of `kind` the built-in one?
    pub fn is_builtin(&self, kind: DeviceKind) -> bool {
        match kind.height_group() {
            Some(g) => {
                let builtin = ElectricalDefaults::default();
                (self.group_height(g) - builtin.group_height(g)).abs() < 1e-9
            }
            None => true,
        }
    }

    /// The kind the tool named `tool` places: the Default Library Object
    /// chosen for it, else `fallback`.
    pub fn object(&self, tool: &str, fallback: DeviceKind) -> DeviceKind {
        self.objects.get(tool).copied().unwrap_or(fallback)
    }

    /// Chooses the Default Library Object of `tool`; the tool's own kind
    /// removes the choice.
    pub fn set_object(&mut self, tool: &str, builtin: DeviceKind, kind: DeviceKind) {
        if std::mem::discriminant(&kind) == std::mem::discriminant(&builtin) {
            self.objects.remove(tool);
        } else {
            self.objects.insert(tool.to_string(), kind);
        }
    }

    /// Set as Default for a device: its kind becomes the Default Library
    /// Object of its tool and, on a wall, its height the default of its
    /// height group. Returns whether anything changed.
    pub fn set_from_device(&mut self, d: &Device, ctx: HeightContext) -> bool {
        let before = self.clone();
        if let Some(slot) = slot_of(d.kind) {
            self.set_object(slot.key, slot.builtin, d.kind);
        }
        if let Some(g) = d.kind.height_group() {
            match ctx {
                HeightContext::Wall => self.set_group_height(g, d.height),
                HeightContext::AboveCounter { counter_top } => {
                    self.above_base_cabinet = (d.height - counter_top).max(0.0)
                }
                HeightContext::CabinetSide { bottom, .. } => {
                    self.on_cabinet_side = (d.height - bottom).max(0.0)
                }
            }
        }
        *self != before
    }
}
