//! Electrical devices: kinds, defaults and placed instances.

use crate::symbol::Stroke;
use plan_core::{Id, Point, DEFAULT_CEILING_HEIGHT};
use serde::{Deserialize, Serialize};

/// Every electrical device the layer can hold.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DeviceKind {
    /// Duplex 110 V receptacle.
    Outlet110,
    /// 220 V receptacle (dryer, range, EV charger).
    Outlet220,
    /// Ground-fault circuit-interrupter receptacle.
    Gfci,
    /// Floor-mounted receptacle.
    OutletFloor,
    Switch,
    Switch3Way,
    SwitchDimmer,
    CeilingLight,
    RecessedCan,
    PendantLight,
    WallSconce,
    CeilingFan,
    SmokeDetector,
    CoDetector,
    Thermostat,
    Doorbell,
    /// Main or sub panel.
    Panel,
    /// Strip light; `length` is in inches along the device's local Y axis.
    RopeLight {
        length: f64,
    },
}

/// Receptacle height to the center of the plate, inches.
pub const OUTLET_HEIGHT: f64 = 12.0;
/// Counter-height receptacle (kitchen backsplash), inches.
pub const COUNTER_OUTLET_HEIGHT: f64 = 44.0;
/// Switch height to the center of the plate, inches.
pub const SWITCH_HEIGHT: f64 = 48.0;

impl DeviceKind {
    /// Short display name used in schedules.
    pub fn name(&self) -> &'static str {
        match self {
            DeviceKind::Outlet110 => "110V Outlet",
            DeviceKind::Outlet220 => "220V Outlet",
            DeviceKind::Gfci => "GFCI Outlet",
            DeviceKind::OutletFloor => "Floor Outlet",
            DeviceKind::Switch => "Switch",
            DeviceKind::Switch3Way => "3-Way Switch",
            DeviceKind::SwitchDimmer => "Dimmer Switch",
            DeviceKind::CeilingLight => "Ceiling Light",
            DeviceKind::RecessedCan => "Recessed Light",
            DeviceKind::PendantLight => "Pendant Light",
            DeviceKind::WallSconce => "Wall Sconce",
            DeviceKind::CeilingFan => "Ceiling Fan",
            DeviceKind::SmokeDetector => "Smoke Detector",
            DeviceKind::CoDetector => "CO Detector",
            DeviceKind::Thermostat => "Thermostat",
            DeviceKind::Doorbell => "Doorbell",
            DeviceKind::Panel => "Electrical Panel",
            DeviceKind::RopeLight { .. } => "Rope Light",
        }
    }

    /// One-line legend text.
    pub fn description(&self) -> &'static str {
        match self {
            DeviceKind::Outlet110 => "Duplex receptacle, 110 V",
            DeviceKind::Outlet220 => "Receptacle, 220 V dedicated",
            DeviceKind::Gfci => "Duplex receptacle, GFCI protected",
            DeviceKind::OutletFloor => "Floor receptacle",
            DeviceKind::Switch => "Single-pole switch",
            DeviceKind::Switch3Way => "Three-way switch",
            DeviceKind::SwitchDimmer => "Dimmer switch",
            DeviceKind::CeilingLight => "Ceiling light fixture",
            DeviceKind::RecessedCan => "Recessed can light",
            DeviceKind::PendantLight => "Pendant light",
            DeviceKind::WallSconce => "Wall sconce",
            DeviceKind::CeilingFan => "Ceiling fan",
            DeviceKind::SmokeDetector => "Smoke detector",
            DeviceKind::CoDetector => "Carbon monoxide detector",
            DeviceKind::Thermostat => "Thermostat",
            DeviceKind::Doorbell => "Doorbell button",
            DeviceKind::Panel => "Electrical panel",
            DeviceKind::RopeLight { .. } => "Rope light",
        }
    }

    /// One of each kind, in schedule/legend order (`RopeLight` has length 0).
    pub fn all() -> [DeviceKind; 18] {
        [
            DeviceKind::Outlet110,
            DeviceKind::Outlet220,
            DeviceKind::Gfci,
            DeviceKind::OutletFloor,
            DeviceKind::Switch,
            DeviceKind::Switch3Way,
            DeviceKind::SwitchDimmer,
            DeviceKind::CeilingLight,
            DeviceKind::RecessedCan,
            DeviceKind::PendantLight,
            DeviceKind::WallSconce,
            DeviceKind::CeilingFan,
            DeviceKind::SmokeDetector,
            DeviceKind::CoDetector,
            DeviceKind::Thermostat,
            DeviceKind::Doorbell,
            DeviceKind::Panel,
            DeviceKind::RopeLight { length: 0.0 },
        ]
    }

    /// Default height above the finished floor, inches.
    ///
    /// Wall devices give the height of the plate center. Ceiling devices give
    /// the height of the fixture face (flush ones sit at the default ceiling).
    pub fn default_height(&self) -> f64 {
        match self {
            DeviceKind::Outlet110 | DeviceKind::Outlet220 | DeviceKind::Gfci => OUTLET_HEIGHT,
            DeviceKind::OutletFloor => 0.0,
            DeviceKind::Switch | DeviceKind::Switch3Way | DeviceKind::SwitchDimmer => SWITCH_HEIGHT,
            DeviceKind::Doorbell => SWITCH_HEIGHT,
            DeviceKind::Thermostat => 52.0,
            DeviceKind::WallSconce => 66.0,
            DeviceKind::Panel | DeviceKind::CoDetector => 60.0,
            DeviceKind::CeilingLight
            | DeviceKind::RecessedCan
            | DeviceKind::CeilingFan
            | DeviceKind::SmokeDetector => DEFAULT_CEILING_HEIGHT,
            DeviceKind::PendantLight | DeviceKind::RopeLight { .. } => 84.0,
        }
    }

    /// Mounted on a wall face (placed with [`place_on_wall`](crate::place_on_wall)).
    pub fn is_wall_mounted(&self) -> bool {
        matches!(
            self,
            DeviceKind::Outlet110
                | DeviceKind::Outlet220
                | DeviceKind::Gfci
                | DeviceKind::Switch
                | DeviceKind::Switch3Way
                | DeviceKind::SwitchDimmer
                | DeviceKind::WallSconce
                | DeviceKind::CoDetector
                | DeviceKind::Thermostat
                | DeviceKind::Doorbell
                | DeviceKind::Panel
        )
    }

    /// Mounted on the ceiling.
    pub fn is_ceiling(&self) -> bool {
        matches!(
            self,
            DeviceKind::CeilingLight
                | DeviceKind::RecessedCan
                | DeviceKind::PendantLight
                | DeviceKind::CeilingFan
                | DeviceKind::SmokeDetector
        )
    }

    /// Any switch kind.
    pub fn is_switch(&self) -> bool {
        matches!(
            self,
            DeviceKind::Switch | DeviceKind::Switch3Way | DeviceKind::SwitchDimmer
        )
    }

    /// A receptacle of any voltage (wall or floor).
    pub fn is_outlet(&self) -> bool {
        matches!(
            self,
            DeviceKind::Outlet110
                | DeviceKind::Outlet220
                | DeviceKind::Gfci
                | DeviceKind::OutletFloor
        )
    }

    /// A lighting load (fixtures, sconces, rope light).
    pub fn is_light(&self) -> bool {
        matches!(
            self,
            DeviceKind::CeilingLight
                | DeviceKind::RecessedCan
                | DeviceKind::PendantLight
                | DeviceKind::WallSconce
                | DeviceKind::RopeLight { .. }
        )
    }
}

/// A placed electrical device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    /// Unique within a layer; `0` means "not yet added" (see [`ElectricalLayer::add`](crate::ElectricalLayer::add)).
    pub id: Id,
    pub kind: DeviceKind,
    /// Plan position, inches. Wall devices sit on the wall face.
    pub position: Point,
    /// Facing direction in radians (for wall devices: out of the wall, into the room).
    pub angle: f64,
    /// Height above the finished floor, inches.
    pub height: f64,
    /// Host wall, if wall-mounted.
    pub wall_id: Option<Id>,
    /// Circuit number once assigned.
    pub circuit: Option<u32>,
    pub label: String,
    /// Switches that control this device.
    pub switched_by: Vec<Id>,
}

impl Device {
    /// The plan symbol rotated to `angle` and moved to `position`.
    pub fn symbol_world(&self) -> Vec<Stroke> {
        self.kind
            .symbol()
            .iter()
            .map(|s| s.transformed(self.position, self.angle))
            .collect()
    }
}
