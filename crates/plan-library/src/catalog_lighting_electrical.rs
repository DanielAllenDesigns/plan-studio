//! The Lighting & Electrical catalog: ceiling, wall and exterior fixtures,
//! fans, outlets, low-voltage jacks and the service panel.
//!
//! `Ceiling` items are drawn about their center and carry elevation `0`
//! (they attach at the ceiling plane, whatever the ceiling height is).
//! Wall-mounted items use the back-center origin with the wall at `y = 0`.

use crate::catalog::{entry, Catalog, Placement};
use crate::shapes::*;
use crate::symbol::Stroke;

const LIGHTING: &str = "Lighting";
const ELECTRICAL: &str = "Electrical";

/// Returns the Lighting & Electrical catalog.
pub fn catalog() -> Catalog {
    let mut c = Catalog::new("Lighting & Electrical");
    ceiling_fixtures(&mut c);
    wall_fixtures(&mut c);
    exterior_fixtures(&mut c);
    fans_and_heat(&mut c);
    wiring(&mut c);
    c
}

// ------------------------------------------------------- ceiling fixtures

fn ceiling_fixtures(c: &mut Catalog) {
    let ceil = Placement::Ceiling;
    let cat = [LIGHTING, "Ceiling"];

    let mut chandelier = vec![circle(0.0, 0.0, 14.0), circle(0.0, 0.0, 5.0)];
    for k in 0..6 {
        let a = 60.0 * f64::from(k);
        chandelier.push(line(polar(0.0, 0.0, 5.0, a), polar(0.0, 0.0, 9.5, a)));
        let (x, y) = polar(0.0, 0.0, 11.5, a);
        chandelier.push(circle(x, y, 2.0)); // bulbs
    }
    c.items.push(entry(
        "core.lighting.chandelier_28",
        "Chandelier 28",
        ceil,
        &cat,
        (28.0, 28.0, 30.0, 0.0),
        &[
            "light",
            "fixture",
            "dining",
            "foyer",
            "six light",
            "6 light",
        ],
        chandelier,
    ));

    let mut pendant = vec![rect(-18.0, -1.0, 18.0, 1.0)];
    for x in [-12.0, 0.0, 12.0] {
        pendant.push(circle(x, 0.0, 4.0));
        pendant.push(circle(x, 0.0, 1.0));
    }
    c.items.push(entry(
        "core.lighting.pendant_3light",
        "Pendant, 3-Light",
        ceil,
        &cat,
        (36.0, 8.0, 24.0, 0.0),
        &[
            "light",
            "fixture",
            "kitchen island",
            "linear pendant",
            "three light",
        ],
        pendant,
    ));

    let mut track = vec![rect(-24.0, -3.0, 24.0, -1.0)];
    for x in [-16.0, 0.0, 16.0] {
        track.push(circle(x, 1.0, 2.0)); // heads
    }
    c.items.push(entry(
        "core.lighting.track_light_48",
        "Track Light 48",
        ceil,
        &cat,
        (48.0, 6.0, 6.0, 0.0),
        &["light", "fixture", "rail", "spot", "3 head", "three head"],
        track,
    ));

    c.items.push(entry(
        "core.lighting.recessed_4in",
        "Recessed Light 4in",
        ceil,
        &cat,
        (4.0, 4.0, 6.0, 0.0),
        &[
            "can",
            "downlight",
            "pot light",
            "can light",
            "4 inch",
            "recessed",
        ],
        vec![circle(0.0, 0.0, 2.0), circle(0.0, 0.0, 1.4)],
    ));
    c.items.push(entry(
        "core.lighting.recessed_6in",
        "Recessed Light 6in",
        ceil,
        &cat,
        (6.0, 6.0, 7.0, 0.0),
        &[
            "can",
            "downlight",
            "pot light",
            "can light",
            "6 inch",
            "recessed",
        ],
        vec![circle(0.0, 0.0, 3.0), circle(0.0, 0.0, 2.1)],
    ));

    let mut drum = vec![circle(0.0, 0.0, 7.0), circle(0.0, 0.0, 5.6)];
    for k in 0..4 {
        let (x, y) = polar(0.0, 0.0, 6.3, 45.0 + 90.0 * f64::from(k));
        drum.push(circle(x, y, 0.4)); // screws
    }
    c.items.push(entry(
        "core.lighting.surface_drum_14",
        "Surface Mount Drum Light 14",
        ceil,
        &cat,
        (14.0, 14.0, 5.0, 0.0),
        &["light", "fixture", "flush mount", "ceiling light", "drum"],
        drum,
    ));
}

// ---------------------------------------------------------- wall fixtures

fn wall_fixtures(c: &mut Catalog) {
    let wall = Placement::WallMounted;
    let cat = [LIGHTING, "Wall"];

    c.items.push(entry(
        "core.lighting.wall_sconce",
        "Wall Sconce",
        wall,
        &cat,
        (8.0, 4.0, 10.0, 66.0),
        &["light", "fixture", "half round", "hallway", "bedside"],
        vec![
            line((-4.0, 0.0), (4.0, 0.0)),
            arc(0.0, 0.0, 4.0, 0.0, 180.0),
            arc(0.0, 0.0, 2.4, 0.0, 180.0),
        ],
    ));

    let mut strip = vec![rect(-12.0, 0.0, 12.0, 2.5)];
    for x in [-10.0, 10.0] {
        strip.push(line((x, 0.0), (x, 2.5)));
    }
    strip.push(line((-10.0, 1.25), (10.0, 1.25)));
    c.items.push(entry(
        "core.lighting.under_cabinet_strip_24",
        "Under-Cabinet Light Strip 24",
        wall,
        &cat,
        (24.0, 2.5, 1.5, 52.0),
        &[
            "light",
            "task light",
            "led strip",
            "puck",
            "kitchen",
            "linear",
        ],
        strip,
    ));

    let mut vanity = rounded_rect(-12.0, 0.0, 12.0, 5.0, 1.0);
    for x in [-7.5, 0.0, 7.5] {
        vanity.push(circle(x, 2.5, 1.6));
    }
    c.items.push(entry(
        "core.lighting.vanity_bar_3light",
        "Vanity Bar, 3-Light",
        wall,
        &cat,
        (24.0, 5.0, 6.0, 78.0),
        &[
            "light",
            "fixture",
            "bath bar",
            "bathroom",
            "three light",
            "mirror light",
        ],
        vanity,
    ));

    c.items.push(entry(
        "core.lighting.step_light",
        "Step Light",
        wall,
        &cat,
        (5.0, 2.0, 3.0, 14.0),
        &["light", "stair light", "recessed", "low voltage", "path"],
        vec![
            rect(-2.5, 0.0, 2.5, 2.0),
            line((-1.5, 0.7), (1.5, 0.7)),
            line((-1.5, 1.3), (1.5, 1.3)),
        ],
    ));
}

// ------------------------------------------------------ exterior fixtures

fn exterior_fixtures(c: &mut Catalog) {
    let cat = [LIGHTING, "Exterior"];

    c.items.push(entry(
        "core.lighting.wall_lantern_exterior",
        "Exterior Wall Lantern",
        Placement::WallMounted,
        &cat,
        (8.0, 9.0, 16.0, 66.0),
        &[
            "light",
            "fixture",
            "outdoor",
            "porch light",
            "garage",
            "coach light",
        ],
        vec![
            rect(-4.0, 0.0, 4.0, 9.0),
            rect(-2.5, 1.5, 2.5, 7.5),
            circle(0.0, 4.5, 1.2),
            line((-4.0, 1.0), (4.0, 1.0)),
        ],
    ));
    c.items.push(entry(
        "core.lighting.post_light",
        "Post Light",
        Placement::FreeStanding,
        &cat,
        (10.0, 10.0, 84.0, 0.0),
        &[
            "light",
            "fixture",
            "lamp post",
            "outdoor",
            "driveway",
            "landscape",
        ],
        vec![
            rect(-5.0, -5.0, 5.0, 5.0),
            circle(0.0, 0.0, 3.6),
            circle(0.0, 0.0, 1.2),
        ],
    ));
    c.items.push(entry(
        "core.lighting.flood_light",
        "Flood Light",
        Placement::WallMounted,
        &cat,
        (12.0, 8.0, 8.0, 96.0),
        &[
            "light",
            "fixture",
            "security light",
            "outdoor",
            "motion",
            "eave",
        ],
        vec![
            rect(-6.0, 0.0, 6.0, 2.0),
            circle(-3.8, 5.8, 2.2),
            circle(3.8, 5.8, 2.2),
            circle(0.0, 5.8, 1.2), // motion sensor
        ],
    ));
}

// --------------------------------------------------------- fans and heat

fn fans_and_heat(c: &mut Catalog) {
    let ceil = Placement::Ceiling;
    let cat = [LIGHTING, "Fans & Heat"];

    let mut fan = vec![
        circle(0.0, 0.0, 26.0),
        circle(0.0, 0.0, 6.0),
        circle(0.0, 0.0, 4.0),
    ];
    for k in 0..4 {
        let a = 45.0 + 90.0 * f64::from(k);
        let (dx, dy) = polar(0.0, 0.0, 1.0, a + 90.0);
        let at = |r: f64, half: f64| {
            let (bx, by) = polar(0.0, 0.0, r, a);
            (bx + dx * half, by + dy * half)
        };
        fan.push(polyline(
            &[at(7.0, -3.0), at(24.0, -3.5), at(24.0, 3.5), at(7.0, 3.0)],
            true,
        ));
    }
    c.items.push(entry(
        "core.lighting.ceiling_fan_light_52",
        "Ceiling Fan with Light 52",
        ceil,
        &cat,
        (52.0, 52.0, 14.0, 0.0),
        &["fan", "blades", "light kit", "light", "fixture"],
        fan,
    ));

    let mut exhaust = vec![
        rect(-7.0, -7.0, 7.0, 7.0),
        circle(0.0, 0.0, 5.5),
        circle(0.0, 0.0, 1.2),
    ];
    for k in 0..4 {
        let a = 90.0 * f64::from(k);
        exhaust.push(arc(0.0, 0.0, 3.5, a, a + 70.0)); // fan blades
    }
    c.items.push(entry(
        "core.lighting.exhaust_fan_14",
        "Exhaust Fan 14",
        ceil,
        &cat,
        (14.0, 14.0, 8.0, 0.0),
        &["bath fan", "vent fan", "ventilation", "bathroom", "fan"],
        exhaust,
    ));

    let mut heat = vec![rect(-6.0, -6.0, 6.0, 6.0), circle(0.0, 0.0, 4.5)];
    for dx in [-2.0, 0.0, 2.0] {
        heat.push(line((dx, -3.0), (dx, 3.0))); // heating element
    }
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        heat.push(line((sx * 4.6, sy * 4.6), (sx * 5.6, sy * 5.6)));
    }
    c.items.push(entry(
        "core.lighting.heat_lamp",
        "Heat Lamp",
        ceil,
        &cat,
        (12.0, 12.0, 7.0, 0.0),
        &["heater", "bath heater", "infrared", "bathroom", "light"],
        heat,
    ));
}

// ----------------------------------------------------------------- wiring

fn wiring(c: &mut Catalog) {
    let wall = Placement::WallMounted;
    let outlets = [ELECTRICAL, "Outlets"];
    let low = [ELECTRICAL, "Low Voltage"];

    c.items.push(entry(
        "core.electrical.dryer_outlet_240v",
        "Dryer Outlet 240V",
        wall,
        &outlets,
        (6.0, 6.0, 4.0, 18.0),
        &["receptacle", "240v", "220v", "30 amp", "laundry", "plug"],
        vec![
            circle(0.0, 3.0, 3.0),
            line((-1.6, 1.6), (-1.6, 4.4)),
            line((1.6, 1.6), (1.6, 4.4)),
            line((0.0, 1.2), (0.0, 2.4)),
        ],
    ));
    c.items.push(entry(
        "core.electrical.usb_outlet",
        "USB Outlet",
        wall,
        &outlets,
        (6.0, 6.0, 4.0, 16.0),
        &["receptacle", "120v", "usb charger", "plug", "duplex"],
        vec![
            circle(0.0, 3.0, 3.0),
            rect(-1.9, 2.2, -0.4, 3.8),
            rect(0.4, 2.2, 1.9, 3.8),
            line((-2.0, 1.1), (2.0, 1.1)),
        ],
    ));
    c.items.push(entry(
        "core.electrical.floor_outlet",
        "Floor Outlet",
        Placement::FreeStanding,
        &outlets,
        (6.0, 6.0, 2.0, 0.0),
        &["receptacle", "floor box", "120v", "plug", "island"],
        vec![
            circle(0.0, 0.0, 3.0),
            rect(-1.8, -1.8, 1.8, 1.8),
            circle(0.0, 0.0, 0.8),
        ],
    ));
    c.items.push(entry(
        "core.electrical.data_jack",
        "Data Jack",
        wall,
        &low,
        (6.0, 6.0, 4.0, 16.0),
        &[
            "ethernet",
            "network",
            "cat6",
            "rj45",
            "low voltage",
            "telecom",
        ],
        vec![
            polyline(&[(-3.0, 0.0), (3.0, 0.0), (0.0, 6.0)], true),
            circle(0.0, 2.0, 0.9),
        ],
    ));
    c.items.push(entry(
        "core.electrical.tv_outlet",
        "TV Outlet",
        wall,
        &low,
        (6.0, 6.0, 4.0, 48.0),
        &[
            "coax",
            "cable",
            "hdmi",
            "television",
            "low voltage",
            "media",
        ],
        vec![
            rect(-3.0, 0.0, 3.0, 6.0),
            line((-2.2, 4.5), (-0.4, 4.5)), // T
            line((-1.3, 4.5), (-1.3, 1.5)),
            polyline(&[(0.4, 4.5), (1.3, 1.5), (2.2, 4.5)], false), // V
        ],
    ));
    c.items.push(entry(
        "core.electrical.panel_200a",
        "Electrical Panel 200A",
        wall,
        &[ELECTRICAL, "Panels"],
        (16.0, 5.0, 36.0, 48.0),
        &[
            "200a",
            "200 amp",
            "breaker box",
            "service panel",
            "load center",
            "main panel",
        ],
        panel(),
    ));
}

/// A service panel: enclosure, a label plate and two breaker columns.
fn panel() -> Vec<Stroke> {
    vec![
        rect(-8.0, 0.0, 8.0, 5.0),
        rect(-5.5, 1.2, 5.5, 3.8),      // label plate
        line((-7.0, 1.0), (-7.0, 4.0)), // breaker columns
        line((7.0, 1.0), (7.0, 4.0)),
        line((-5.5, 2.5), (5.5, 2.5)),
    ]
}
