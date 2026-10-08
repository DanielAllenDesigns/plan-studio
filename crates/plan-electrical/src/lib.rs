//! plan-electrical: Chief's electrical layer.
//!
//! Wall-mounted and ceiling devices with plan symbols ([`DeviceKind::symbol`]),
//! NEC-style Auto Place Outlets ([`auto_place_outlets`]), switch-to-light
//! connections drawn as arcs ([`connect`], [`ElectricalLayer::connection_arc`]),
//! circuits and a device schedule ([`circuits`], [`schedule`], [`legend`]) and
//! 3D stand-in meshes ([`meshes`]).
//!
//! Units are inches (Y up in plan space); angles are radians. A wall-mounted
//! device's `angle` is the direction it faces, out of the wall into the room.
//! Wall-mounted devices sit on the wall face: `place_on_wall` offsets half the
//! wall thickness from the centerline.

mod circuit;
mod device;
mod layer;
mod mesh3d;
mod place;
mod symbol;

pub use circuit::{assign_circuits, circuits, legend, schedule, Circuit, CircuitOptions};
pub use device::{Device, DeviceKind, COUNTER_OUTLET_HEIGHT, OUTLET_HEIGHT, SWITCH_HEIGHT};
pub use layer::{connect, connect_in, Connection, ElectricalLayer};
pub use mesh3d::meshes;
pub use place::{
    auto_place_outlets, auto_place_room_light, auto_place_switch, place_free, place_on_wall,
    AutoOutletOptions, RoomFunction, WallSide,
};
pub use symbol::Stroke;

#[cfg(test)]
mod tests;
