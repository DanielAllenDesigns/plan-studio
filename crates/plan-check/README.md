# plan-check

Chief's Tools > Checks menu as a rule engine. `plan_check` runs twelve IRC-based rules over one floor
(room size, bedroom egress, ventilation, door widths and swings, hallways, stairs, garage, wall geometry,
opening geometry, room access, natural light) and returns `Finding`s with severity, location, rule text and fix.
`door_window_check` runs the opening rules alone; `plan_footprint` traces the outer boundary of the rooms;
`report_markdown` groups findings by severity with counts. Limits live in `CheckOptions` (2021 IRC defaults).
Room types come from the caller (or the floor's room names); rooms are measured to wall centerlines.
Each rule is its own function in `src/rules.rs` with a doc comment naming its IRC section; add a rule by
writing one and calling it from `plan_check`. Test with `cargo test -p plan-check`.

Round 14 adds `rules_irc.rs` (egress door, handrail height and continuity, guard openings, garage separation,
alarms outside sleeping areas, footings), net clear egress sizes in `rules.rs`, the shower entrance in
`rules_fixtures.rs`, rafter spans and receptacle wall spaces in `rules_mep.rs`, and the Georgia 2020 preset and
rule-group switches in `settings.rs` (`PlanCheckSettings` is `CheckSettings`).
