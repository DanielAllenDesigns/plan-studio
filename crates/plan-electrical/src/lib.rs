//! plan-electrical: Chief's electrical layer.
//!
//! Wall-mounted and ceiling devices with plan symbols ([`DeviceKind::symbol`]),
//! NEC-style Auto Place Outlets ([`auto_place_outlets`]), switch-to-light
//! connections drawn as arcs ([`connect`], [`ElectricalLayer::connection_arc`]),
//! circuits and a device schedule ([`circuits`], [`schedule`], [`schedule_rows`],
//! [`legend`]), 3-way / 4-way switch promotion ([`normalize_switch_kinds`]),
//! exterior weatherproof outlets ([`auto_place_exterior_outlets`]), the plan's
//! default device heights ([`ElectricalDefaults`]) and 3D stand-in meshes
//! ([`meshes`]).
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
mod place;
mod symbol;

pub use circuit::{
    assign_circuits, circuits, legend, schedule, schedule_rows, Circuit, CircuitOptions,
    ScheduleRow,
};
pub use defaults::{ElectricalDefaults, COUNTER_OUTLET_KEY};
pub use device::{
    Device, DeviceKind, COUNTER_OUTLET_HEIGHT, FINISHES, OUTLET_HEIGHT, SWITCH_HEIGHT,
    WP_OUTLET_HEIGHT,
};
pub use layer::{
    connect, connect_in, disconnect, normalize_switch_kinds, Connection, ElectricalLayer,
};
pub use mesh3d::{electrical_meshes, finish_material, meshes};
pub use place::{
    auto_place_exterior_outlets, auto_place_outlets, auto_place_room_light, auto_place_switch,
    place_free, place_on_wall, AutoOutletOptions, RoomFunction, WallSide,
};
pub use symbol::Stroke;

#[cfg(test)]
mod tests;
