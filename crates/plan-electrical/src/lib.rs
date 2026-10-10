//! plan-electrical: Chief's electrical layer.
//!
//! Wall-mounted and ceiling devices with plan symbols ([`DeviceKind::symbol`]),
//! NEC-style Auto Place Outlets ([`auto_place_outlets`]), switch-to-light
//! connections drawn as arcs ([`connect`], [`ElectricalLayer::connection_arc`]),
//! circuits and a device schedule ([`circuits`], [`schedule`], [`schedule_rows`],
//! [`legend`]), 3-way / 4-way switch promotion ([`normalize_switch_kinds`]),
//! exterior weatherproof outlets ([`auto_place_exterior_outlets`]), the plan's
//! Electrical Defaults ([`ElectricalDefaults`]: library objects, four height
//! groups, connection and rope light defaults), splines with vertices that
//! attach to and detach from devices ([`ElectricalLayer::detach_end`]),
//! rope light paths ([`RopeLightPath`]), per-device options such as recess
//! and size ([`DeviceOptions`]) and 3D stand-in meshes ([`meshes`]).
//!
//! Units are inches (Y up in plan space); angles are radians. A wall-mounted
//! device's `angle` is the direction it faces, out of the wall into the room.
//! Wall-mounted devices sit on the wall face: `place_on_wall` offsets half the
//! wall thickness from the centerline.

mod circuit;
mod defaults;
mod device;
mod layer;
mod mesh3d;
mod options;
mod place;
mod rope;
mod symbol;

pub use circuit::{
    assign_circuits, circuits, legend, schedule, schedule_rows, Circuit, CircuitOptions,
    ScheduleRow,
};
pub use defaults::{
    slot_of, ConnectionDefaults, ElectricalDefaults, HeightContext, HeightGroup, ToolSlot,
    ABOVE_BASE_CABINET_HEIGHT, COUNTER_OUTLET_KEY, DEFAULT_CURVATURE, ON_CABINET_SIDE_HEIGHT,
    STANDARD_COUNTER_HEIGHT, TOOL_SLOTS,
};
pub use device::{
    Device, DeviceKind, COUNTER_OUTLET_HEIGHT, FINISHES, OUTLET_HEIGHT, PATH_LIGHT_HEIGHT,
    SWITCH_HEIGHT, WP_OUTLET_HEIGHT,
};
pub use layer::{
    connect, connect_drawn, connect_in, connect_with, disconnect, normalize_switch_kinds, Arrow,
    ConnEnd, Connection, ElectricalLayer,
};
pub use mesh3d::{electrical_meshes, finish_material, meshes};
pub use options::{DeviceOptions, HeightTo, Mount, Recess};
pub use place::{
    auto_place_exterior_outlets, auto_place_outlets, auto_place_outlets_by_rules,
    auto_place_room_light, auto_place_switch, face_is_exterior, kind_for_setting, place_free,
    place_on_wall, AutoOutletOptions, RoomFunction, WallSide,
};
pub use rope::{light_positions, RopeLightPath, RopeReference, RopeSpec};
pub use symbol::Stroke;

#[cfg(test)]
mod tests;
