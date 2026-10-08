# plan-electrical

Chief's electrical layer: devices, plan symbols, Auto Place Outlets, circuits, 3D stand-ins.

- `DeviceKind`/`Device`/`ElectricalLayer` (serde): 18 kinds, default heights, `symbol()` strokes. Inches, angles in radians.
- `place_on_wall` puts a device on the wall face (half thickness off the centerline), facing into the room; `place_free` for ceilings.
- `auto_place_outlets`: nothing over 6' from an outlet, clear of door jambs, 44" GFCI kitchen counters, GFCI in wet rooms and garages.
- `auto_place_room_light`, `auto_place_switch` (48", 6" past the latch jamb), `connect`/`connect_in` (curved arcs).
- `circuits`/`assign_circuits`, `schedule`, `legend`; `meshes` gives white `plan_3d::Mesh` plates and discs.
- `RoomFunction` lives here (plan-core has none); rooms are matched to it by `Room::label`.
