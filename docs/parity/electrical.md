# Parity: Electrical (Chief X18), round 14

Source: the CB-62..CB-68 rows of `cabinets-stairs-framing-terrain-library.md` (section F), refined for round 14.
Status values: Done, Partial, Open. "verify in Chief" marks behavior chosen as the most Chief-like option (see `DECISIONS.md` 87 to 90).

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| E-1 | 110V outlet (duplex symbol), 220V outlet (three blades, `220V`), GFCI outlet (`GFCI` tag), WP outlet (`WP` tag, weatherproof GFCI), Dedicated outlet (single blade, `DED`) | Done, verify in Chief (glyphs) | `plan-electrical/src/symbol.rs`; `tests::flags_voltage_and_symbols_of_the_outlet_kinds`; scenario `the_flag_symbols_are_drawn_in_the_plan` |
| E-2 | Voltage and flags are read from the type (110V / 220V, GFCI, WP, Dedicated) | Done | `DeviceKind::{voltage,is_gfci,is_weatherproof,is_dedicated,flags}`; dialog General tab rows Voltage and Flags |
| E-3 | Outlet placement: click a wall, snaps to the nearest wall within 12", faces into the room, height 12" (WP 18") | Done | `tools/electrical.rs::wall_placement`; s08 `outlets_and_switches_mount_on_the_wall_face_nearest_the_click`; s32 `the_new_flavors_activate_and_stay_on_the_wall` |
| E-4 | Switch placement 48", Auto Place Switches near the latch jamb | Done | `place::auto_place_switch`, `tools/electrical.rs::auto_place_floor_switches` (round 13) |
| E-5 | Electrical Connection: click a switch, then each light or outlet it controls; dashed arc, bendable | Done | `layer::connect_in`; s08 `a_light_snaps_to_the_room_center_and_a_switch_is_connected_to_it` |
| E-6 | A light with two switches makes 3-way switches (S3), with three the middle one 4-way (S4); the symbol follows; removing a connection turns them back | Done, verify in Chief (order of 4-ways) | `layer::normalize_switch_kinds`; `tests::a_light_with_two_switches_gets_3_way_switches_and_three_gets_a_4_way`; s32 `connecting_a_second_and_third_switch_to_a_light_makes_s3_and_s4_switches` |
| E-7 | Auto Place Outlets, NEC 210.52: wall spaces of 2' or more, nothing farther than 6' from an outlet (12' apart), 6" clear of door jambs | Done | `place::auto_place_outlets`; `tests::outlets_on_the_40x30_shell_follow_the_nec_rules` |
| E-8 | Kitchen counter outlets every 4' at 44", GFCI | Done (cabinets are not consulted, every kitchen wall gets them) | `place.rs`; `tests::kitchen_gets_gfci_counter_outlets_at_44` |
| E-9 | Bath, laundry, garage GFCI; a small bath still gets one outlet | Done | `tests::a_tiny_bath_still_gets_a_gfci` |
| E-10 | Exterior WP GFCI outlets, front and back | Done, verify in Chief | `place::auto_place_exterior_outlets`; `tests::exterior_receptacles_go_on_the_outside_front_and_back`; s32 `auto_place_outlets_adds_the_exterior_weatherproof_receptacles_in_one_step` |
| E-11 | Default device heights per kind (outlet 12", switch 48", counter outlet 44"), editable | Done | `defaults.rs`, `Project::electrical_defaults`; dialog General tab; `tests::default_heights_are_stored_in_the_project`; s32 `device_heights_come_from_the_plans_electrical_defaults`; `tools::electrical::tests` `the_dialog_stores_default_heights_in_the_same_undo_step` |
| E-12 | Electrical Specification tabs General / Label / Layer | Partial: General (type, voltage, flags, height, defaults, label, circuit), Label (show, text), Layer (disabled). Switches and Materials stay as extra tabs. Label text height is not stored (disabled "3\"") | `dialogs/electrical.rs` |
| E-13 | Electrical schedule columns: mark, type, height, circuit | Done: the Electrical Schedule has mark, type, count, label, mount height, circuit; `schedule_rows` adds the voltage and flags | `plan-electrical/src/circuit.rs::schedule_rows`; `tests::the_schedule_rows_carry_mark_type_height_circuit_and_flags`; `plan-docs/schedule_kinds.rs::electrical` |
| E-14 | Dedicated circuits (220V and dedicated outlets each get a circuit) | Done | `circuit::circuits`; same test |
| E-15 | Electrical layer display: devices and connections on `Electrical` | Partial: the connection arcs share the `Electrical` layer; Chief's separate Electrical Connection layer is open (needs `plan-core/layers.rs`) | `editor/site_view.rs::draw_devices` |
| E-16 | 3D device meshes face out of their wall; WP in-use cover | Done | `mesh3d.rs`; `tests::wall_devices_are_modeled_facing_out_of_their_wall` |

<!-- coverage-audit:start -->
## Coverage audit additions (2026-10-08)

Rows added by the Round 14 coverage audit (`docs/chief-feature-coverage.md`): Chief X18 features found in the menu, toolbar, sub-tool and dialog captures, or known from the product, that no row above covered. Status comes from a code search, not a Chief session; "verify in Chief" marks behavior known only from the product. Variants of one flyout or tab share one row.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| E-17 | Ceiling Fan: Low-voltage, safety and fan symbols placed on a wall or ceiling with a height and schedule row. Also covers: Smoke Detector; CO Detector; Thermostat; Doorbell; Data Jack; Phone Jack; TV Jack; Electrical Panel. (Not captured; verify in Chief.) | Done | toolbar.rs electrical() entries (E::CeilingFan ... E::Panel); plan-electrical DeviceKind |
<!-- coverage-audit:end -->
