//! Electrical devices: kinds, defaults and placed instances.

use crate::symbol::Stroke;
use plan_core::{Id, Point, DEFAULT_CEILING_HEIGHT};
use serde::{Deserialize, Serialize};

/// Every electrical device the layer can hold.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DeviceKind {
    /// Duplex 110 V receptacle.
    Outlet110,
    /// Quad (two duplex) 110 V receptacle.
    Outlet110Quad,
    /// 220 V receptacle (dryer, range, EV charger).
    Outlet220,
    /// Ground-fault circuit-interrupter receptacle.
    Gfci,
    /// Floor-mounted receptacle.
    OutletFloor,
    /// Weatherproof (WP) GFCI receptacle for exterior walls.
    OutletWp,
    /// Single receptacle on its own dedicated circuit (refrigerator, disposal, freezer).
    OutletDedicated,
    Switch,
    /// Weatherproof switch for exterior walls and exterior rooms.
    SwitchWp,
    Switch3Way,
    /// Four-way switch (the middle of a three-way run).
    Switch4Way,
    SwitchDimmer,
    CeilingLight,
    RecessedCan,
    PendantLight,
    WallSconce,
    /// Outdoor wall light: the Light tool's choice on an exterior wall or in
    /// an exterior room (E-31).
    WallLightExterior,
    /// Free-standing path light for the ground outside a room.
    PathLight,
    CeilingFan,
    SmokeDetector,
    CoDetector,
    Thermostat,
    Doorbell,
    /// Data (network) jack.
    DataJack,
    /// Telephone jack.
    PhoneJack,
    /// Coax / TV jack.
    TvJack,
    /// Main or sub panel.
    Panel,
    /// Strip light; `length` is in inches along the device's local Y axis.
    RopeLight {
        length: f64,
    },
}

/// Plate and fixture finishes offered by the Materials tab of the Electrical
/// Service Specification (the first is the default).
pub const FINISHES: [&str; 6] = [
    "White",
    "Ivory",
    "Light Almond",
    "Brown",
    "Black",
    "Stainless Steel",
];

/// Receptacle height to the center of the plate, inches.
pub const OUTLET_HEIGHT: f64 = 12.0;
/// Counter-height receptacle (kitchen backsplash), inches to the plate center.
pub const COUNTER_OUTLET_HEIGHT: f64 = 44.0;
/// Weatherproof exterior receptacle height above the finished floor, inches.
pub const WP_OUTLET_HEIGHT: f64 = 18.0;
/// Switch height to the center of the plate, inches.
pub const SWITCH_HEIGHT: f64 = 48.0;
/// Height of a free-standing path light, inches.
pub const PATH_LIGHT_HEIGHT: f64 = 18.0;

impl DeviceKind {
    /// Short display name used in schedules.
    pub fn name(&self) -> &'static str {
        match self {
            DeviceKind::Outlet110 => "110V Outlet",
            DeviceKind::Outlet110Quad => "Quad Outlet",
            DeviceKind::Outlet220 => "220V Outlet",
            DeviceKind::Gfci => "GFCI Outlet",
            DeviceKind::OutletFloor => "Floor Outlet",
            DeviceKind::OutletWp => "WP Outlet",
            DeviceKind::OutletDedicated => "Dedicated Outlet",
            DeviceKind::Switch => "Switch",
            DeviceKind::SwitchWp => "WP Switch",
            DeviceKind::Switch3Way => "3-Way Switch",
            DeviceKind::Switch4Way => "4-Way Switch",
            DeviceKind::SwitchDimmer => "Dimmer Switch",
            DeviceKind::CeilingLight => "Ceiling Light",
            DeviceKind::RecessedCan => "Recessed Light",
            DeviceKind::PendantLight => "Pendant Light",
            DeviceKind::WallSconce => "Wall Sconce",
            DeviceKind::WallLightExterior => "Exterior Wall Light",
            DeviceKind::PathLight => "Path Light",
            DeviceKind::CeilingFan => "Ceiling Fan",
            DeviceKind::SmokeDetector => "Smoke Detector",
            DeviceKind::CoDetector => "CO Detector",
            DeviceKind::Thermostat => "Thermostat",
            DeviceKind::Doorbell => "Doorbell",
            DeviceKind::DataJack => "Data Jack",
            DeviceKind::PhoneJack => "Phone Jack",
            DeviceKind::TvJack => "TV Jack",
            DeviceKind::Panel => "Electrical Panel",
            DeviceKind::RopeLight { .. } => "Rope Light",
        }
    }

    /// One-line legend text.
    pub fn description(&self) -> &'static str {
        match self {
            DeviceKind::Outlet110 => "Duplex receptacle, 110 V",
            DeviceKind::Outlet110Quad => "Quad receptacle, 110 V",
            DeviceKind::Outlet220 => "Receptacle, 220 V dedicated",
            DeviceKind::Gfci => "Duplex receptacle, GFCI protected",
            DeviceKind::OutletFloor => "Floor receptacle",
            DeviceKind::OutletWp => "Weatherproof receptacle, GFCI protected",
            DeviceKind::OutletDedicated => "Single receptacle, dedicated circuit",
            DeviceKind::Switch => "Single-pole switch",
            DeviceKind::SwitchWp => "Weatherproof switch",
            DeviceKind::Switch3Way => "Three-way switch",
            DeviceKind::Switch4Way => "Four-way switch",
            DeviceKind::SwitchDimmer => "Dimmer switch",
            DeviceKind::CeilingLight => "Ceiling light fixture",
            DeviceKind::RecessedCan => "Recessed can light",
            DeviceKind::PendantLight => "Pendant light",
            DeviceKind::WallSconce => "Wall sconce",
            DeviceKind::WallLightExterior => "Exterior wall light",
            DeviceKind::PathLight => "Path light",
            DeviceKind::CeilingFan => "Ceiling fan",
            DeviceKind::SmokeDetector => "Smoke detector",
            DeviceKind::CoDetector => "Carbon monoxide detector",
            DeviceKind::Thermostat => "Thermostat",
            DeviceKind::Doorbell => "Doorbell button",
            DeviceKind::DataJack => "Data jack",
            DeviceKind::PhoneJack => "Telephone jack",
            DeviceKind::TvJack => "TV / coax jack",
            DeviceKind::Panel => "Electrical panel",
            DeviceKind::RopeLight { .. } => "Rope light",
        }
    }

    /// One of each kind, in schedule/legend order (`RopeLight` has length 0).
    pub fn all() -> [DeviceKind; 28] {
        [
            DeviceKind::Outlet110,
            DeviceKind::Outlet110Quad,
            DeviceKind::Outlet220,
            DeviceKind::Gfci,
            DeviceKind::OutletFloor,
            DeviceKind::OutletWp,
            DeviceKind::OutletDedicated,
            DeviceKind::Switch,
            DeviceKind::SwitchWp,
            DeviceKind::Switch3Way,
            DeviceKind::Switch4Way,
            DeviceKind::SwitchDimmer,
            DeviceKind::CeilingLight,
            DeviceKind::RecessedCan,
            DeviceKind::PendantLight,
            DeviceKind::WallSconce,
            DeviceKind::WallLightExterior,
            DeviceKind::PathLight,
            DeviceKind::CeilingFan,
            DeviceKind::SmokeDetector,
            DeviceKind::CoDetector,
            DeviceKind::Thermostat,
            DeviceKind::Doorbell,
            DeviceKind::DataJack,
            DeviceKind::PhoneJack,
            DeviceKind::TvJack,
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
            DeviceKind::Outlet110
            | DeviceKind::Outlet110Quad
            | DeviceKind::Outlet220
            | DeviceKind::Gfci
            | DeviceKind::OutletDedicated
            | DeviceKind::DataJack
            | DeviceKind::PhoneJack => OUTLET_HEIGHT,
            DeviceKind::OutletWp => WP_OUTLET_HEIGHT,
            DeviceKind::OutletFloor => 0.0,
            DeviceKind::Switch
            | DeviceKind::Switch3Way
            | DeviceKind::Switch4Way
            | DeviceKind::SwitchDimmer
            | DeviceKind::SwitchWp => SWITCH_HEIGHT,
            DeviceKind::Doorbell | DeviceKind::TvJack => SWITCH_HEIGHT,
            DeviceKind::Thermostat => 52.0,
            DeviceKind::WallSconce | DeviceKind::WallLightExterior => 66.0,
            DeviceKind::PathLight => PATH_LIGHT_HEIGHT,
            DeviceKind::Panel | DeviceKind::CoDetector => 60.0,
            DeviceKind::CeilingLight
            | DeviceKind::RecessedCan
            | DeviceKind::CeilingFan
            | DeviceKind::SmokeDetector => DEFAULT_CEILING_HEIGHT,
            DeviceKind::PendantLight | DeviceKind::RopeLight { .. } => 84.0,
        }
    }

    /// The Electrical Defaults height group of this kind: receptacles and the
    /// phone, data and TV jacks follow the Outlet height; switches, doorbells
    /// and thermostats the Switch height (manual p. 692). Everything else
    /// (lights, detectors, the panel) keeps its own default height.
    pub fn height_group(&self) -> Option<crate::defaults::HeightGroup> {
        use crate::defaults::HeightGroup as G;
        match self {
            DeviceKind::OutletFloor => None,
            k if k.is_outlet() => Some(G::Outlet),
            k if k.is_low_voltage() => Some(G::Outlet),
            k if k.is_switch() => Some(G::Switch),
            DeviceKind::Doorbell | DeviceKind::Thermostat => Some(G::Switch),
            _ => None,
        }
    }

    /// The size of the plate or fixture in the plan and in 3D, inches:
    /// width along the wall (or the fixture's X) and height (or Y).
    pub fn default_size(&self) -> (f64, f64) {
        match self {
            DeviceKind::OutletWp | DeviceKind::SwitchWp => (5.0, 5.5),
            DeviceKind::Thermostat => (3.5, 3.5),
            DeviceKind::WallSconce | DeviceKind::WallLightExterior => (5.0, 8.0),
            DeviceKind::CeilingLight => (12.0, 12.0),
            DeviceKind::RecessedCan => (8.0, 8.0),
            DeviceKind::PendantLight => (14.0, 14.0),
            DeviceKind::CeilingFan => (29.0, 29.0),
            DeviceKind::SmokeDetector | DeviceKind::CoDetector => (9.0, 9.0),
            DeviceKind::Panel => (14.0, 20.0),
            DeviceKind::OutletFloor => (4.0, 4.0),
            DeviceKind::PathLight => (8.0, PATH_LIGHT_HEIGHT),
            DeviceKind::RopeLight { length } => (0.75, *length),
            _ => (2.75, 4.5),
        }
    }

    /// Mounted on a wall face (placed with [`place_on_wall`](crate::place_on_wall)).
    pub fn is_wall_mounted(&self) -> bool {
        matches!(
            self,
            DeviceKind::Outlet110
                | DeviceKind::Outlet110Quad
                | DeviceKind::Outlet220
                | DeviceKind::Gfci
                | DeviceKind::OutletWp
                | DeviceKind::OutletDedicated
                | DeviceKind::Switch
                | DeviceKind::SwitchWp
                | DeviceKind::Switch3Way
                | DeviceKind::Switch4Way
                | DeviceKind::SwitchDimmer
                | DeviceKind::WallSconce
                | DeviceKind::WallLightExterior
                | DeviceKind::CoDetector
                | DeviceKind::Thermostat
                | DeviceKind::Doorbell
                | DeviceKind::DataJack
                | DeviceKind::PhoneJack
                | DeviceKind::TvJack
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

    /// Telephone, data and TV jacks.
    pub fn is_low_voltage(&self) -> bool {
        matches!(
            self,
            DeviceKind::DataJack | DeviceKind::PhoneJack | DeviceKind::TvJack
        )
    }

    /// The kinds the Electrical Service Specification can change this device
    /// into: the same family (receptacles, switches, ceiling fixtures,
    /// detectors, low-voltage jacks), so the mounting stays valid.
    pub fn family(&self) -> Vec<DeviceKind> {
        let group = |k: &DeviceKind| -> u8 {
            match k {
                _ if k.is_outlet() => 0,
                _ if k.is_switch() => 1,
                DeviceKind::CeilingLight
                | DeviceKind::RecessedCan
                | DeviceKind::PendantLight
                | DeviceKind::CeilingFan => 2,
                DeviceKind::SmokeDetector | DeviceKind::CoDetector => 3,
                _ if k.is_low_voltage() => 4,
                DeviceKind::WallSconce | DeviceKind::WallLightExterior => 5,
                DeviceKind::Thermostat => 6,
                DeviceKind::Doorbell => 7,
                DeviceKind::Panel => 8,
                DeviceKind::PathLight => 10,
                _ => 9,
            }
        };
        let g = group(self);
        let mounted = |k: &DeviceKind| (k.is_wall_mounted(), k.is_ceiling());
        DeviceKind::all()
            .into_iter()
            .filter(|k| {
                (group(k) == g && mounted(k) == mounted(self))
                    || (std::mem::discriminant(k) == std::mem::discriminant(self))
            })
            // Keep the rope light's own length; the others are zero-sized.
            .map(|k| {
                if matches!(k, DeviceKind::RopeLight { .. }) {
                    *self
                } else {
                    k
                }
            })
            .collect()
    }

    /// Any switch kind.
    pub fn is_switch(&self) -> bool {
        matches!(
            self,
            DeviceKind::Switch
                | DeviceKind::SwitchWp
                | DeviceKind::Switch3Way
                | DeviceKind::Switch4Way
                | DeviceKind::SwitchDimmer
        )
    }

    /// A receptacle of any voltage (wall or floor).
    pub fn is_outlet(&self) -> bool {
        matches!(
            self,
            DeviceKind::Outlet110
                | DeviceKind::Outlet110Quad
                | DeviceKind::Outlet220
                | DeviceKind::Gfci
                | DeviceKind::OutletWp
                | DeviceKind::OutletDedicated
                | DeviceKind::OutletFloor
        )
    }

    /// Supply voltage of a receptacle: 220 for the 220 V outlet, 110 for the
    /// other receptacles, `None` for everything else.
    pub fn voltage(&self) -> Option<u32> {
        match self {
            DeviceKind::Outlet220 => Some(220),
            k if k.is_outlet() => Some(110),
            _ => None,
        }
    }

    /// Ground-fault protected: the GFCI and weatherproof receptacles.
    pub fn is_gfci(&self) -> bool {
        matches!(self, DeviceKind::Gfci | DeviceKind::OutletWp)
    }

    /// Weatherproof (exterior) receptacle.
    pub fn is_weatherproof(&self) -> bool {
        matches!(
            self,
            DeviceKind::OutletWp
                | DeviceKind::SwitchWp
                | DeviceKind::WallLightExterior
                | DeviceKind::PathLight
        )
    }

    /// The weatherproof counterpart the Electrical Tools place outdoors (on
    /// an exterior wall or in an exterior room): a 110V or GFCI receptacle
    /// becomes the WP outlet, a switch the WP switch and the interior wall
    /// light the exterior one. Other kinds (the 220V outlet, ceiling lights)
    /// are the same outdoors.
    pub fn outdoor(&self) -> DeviceKind {
        match self {
            DeviceKind::Outlet110 | DeviceKind::Outlet110Quad | DeviceKind::Gfci => {
                DeviceKind::OutletWp
            }
            DeviceKind::Switch => DeviceKind::SwitchWp,
            DeviceKind::WallSconce => DeviceKind::WallLightExterior,
            k => *k,
        }
    }

    /// The indoor kind of a weatherproof one (the inverse of [`outdoor`](Self::outdoor);
    /// a WP outlet is a GFCI indoors).
    pub fn indoor(&self) -> DeviceKind {
        match self {
            DeviceKind::OutletWp => DeviceKind::Gfci,
            DeviceKind::SwitchWp => DeviceKind::Switch,
            DeviceKind::WallLightExterior => DeviceKind::WallSconce,
            k => *k,
        }
    }

    /// Always on a circuit of its own: the 220 V and dedicated receptacles.
    pub fn is_dedicated(&self) -> bool {
        matches!(self, DeviceKind::Outlet220 | DeviceKind::OutletDedicated)
    }

    /// The flags a schedule or legend shows after the type: `110V`/`220V`,
    /// `GFCI`, `WP`, `Dedicated`.
    pub fn flags(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        match self.voltage() {
            Some(110) => out.push("110V"),
            Some(_) => out.push("220V"),
            None => {}
        }
        if self.is_gfci() {
            out.push("GFCI");
        }
        if self.is_weatherproof() {
            out.push("WP");
        }
        if self.is_dedicated() {
            out.push("Dedicated");
        }
        out
    }

    /// A lighting load (fixtures, sconces, rope light).
    pub fn is_light(&self) -> bool {
        matches!(
            self,
            DeviceKind::CeilingLight
                | DeviceKind::RecessedCan
                | DeviceKind::PendantLight
                | DeviceKind::WallSconce
                | DeviceKind::WallLightExterior
                | DeviceKind::PathLight
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
    /// Plate or fixture finish ("White", "Ivory", "Stainless Steel", ...);
    /// empty means the default white.
    #[serde(default)]
    pub finish: String,
    /// Hide the label in the plan (the label is still kept).
    #[serde(default)]
    pub hide_label: bool,
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

    /// [`symbol_world`](Self::symbol_world) for a device with the size of
    /// `opts` (Width in the dialog): the symbol grows with the width.
    pub fn symbol_world_with(&self, opts: &crate::options::DeviceOptions) -> Vec<Stroke> {
        let (dw, _) = self.kind.default_size();
        let k = match (opts.width, self.kind) {
            (_, DeviceKind::RopeLight { .. }) => 1.0,
            (Some(w), _) if dw > 1e-9 && w > 1e-9 => w / dw,
            _ => 1.0,
        };
        if (k - 1.0).abs() < 1e-9 {
            return self.symbol_world();
        }
        self.kind
            .symbol()
            .iter()
            .map(|s| s.scaled(k).transformed(self.position, self.angle))
            .collect()
    }
}
