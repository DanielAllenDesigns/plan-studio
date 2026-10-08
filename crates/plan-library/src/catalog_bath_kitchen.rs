//! The Bath & Kitchen catalog: toilets, sinks, tubs, showers, laundry and
//! utility equipment, cooking and refrigeration appliances.
//!
//! Items that sit against a wall (toilets, tubs, showers, range hoods,
//! ovens, refrigerators, ...) are `WallMounted`: the origin is the
//! back-center and the symbol extends towards `+y`. Countertop sinks and
//! cooktops, and the free-standing tub and water heater, are drawn about
//! their center with the back on the `-y` side.

use crate::catalog::{entry, Catalog, Placement};
use crate::shapes::*;
use crate::symbol::Stroke;

const BK: &str = "Bath & Kitchen";

/// Returns the Bath & Kitchen catalog.
pub fn catalog() -> Catalog {
    let mut c = Catalog::new("Bath & Kitchen");
    toilets(&mut c);
    sinks(&mut c);
    tubs(&mut c);
    showers(&mut c);
    utility(&mut c);
    cooking(&mut c);
    refrigeration(&mut c);
    c
}

// ----------------------------------------------------------------- toilets

fn toilets(c: &mut Catalog) {
    let wall = Placement::WallMounted;
    let cat = [BK, "Toilets"];
    let bath = ["wc", "water closet", "commode", "bathroom"];

    c.items.push(entry(
        "core.bathkitchen.toilet_round",
        "Toilet, Round Front",
        wall,
        &cat,
        (20.0, 26.0, 30.0, 0.0),
        &[bath[0], bath[1], bath[2], bath[3], "round bowl"],
        toilet_sym(20.0, 7.0, 14.5, 26.0),
    ));
    c.items.push(entry(
        "core.bathkitchen.toilet_elongated",
        "Toilet, Elongated Comfort Height",
        wall,
        &cat,
        (20.0, 29.0, 30.0, 0.0),
        &[bath[0], bath[1], bath[2], bath[3], "elongated bowl", "ada"],
        toilet_sym(20.0, 7.0, 15.0, 29.0),
    ));
    c.items.push(entry(
        "core.bathkitchen.toilet_wall_hung",
        "Toilet, Wall-Hung",
        wall,
        &cat,
        (14.0, 22.0, 14.0, 15.0),
        &[
            bath[0],
            bath[1],
            bath[3],
            "wall mount",
            "carrier",
            "floating",
        ],
        toilet_sym(14.0, 4.0, 14.0, 22.0),
    ));
    let mut bidet = toilet_sym(15.0, 4.0, 14.0, 24.0);
    bidet.push(circle(-4.0, 2.0, 0.7)); // hot and cold handles
    bidet.push(circle(4.0, 2.0, 0.7));
    bidet.push(circle(0.0, 17.0, 0.9)); // drain
    c.items.push(entry(
        "core.bathkitchen.bidet",
        "Bidet",
        wall,
        &cat,
        (15.0, 24.0, 15.0, 0.0),
        &["bathroom", "wash", "hygiene"],
        bidet,
    ));
    let mut urinal = rounded_rect(-7.0, 0.0, 7.0, 14.0, 5.5);
    urinal.extend(rounded_rect(-4.5, 3.0, 4.5, 11.5, 3.5));
    urinal.push(circle(0.0, 9.0, 0.9));
    c.items.push(entry(
        "core.bathkitchen.urinal",
        "Urinal, Wall-Hung",
        wall,
        &cat,
        (14.0, 14.0, 24.0, 24.0),
        &["commercial", "restroom", "men's room"],
        urinal,
    ));
}

/// A toilet from the wall out: tank (or in-wall carrier plate) `tank_w` by
/// `tank_d`, then a bowl `bowl_w` wide with a half-round front, `total_d`
/// from the wall to the front of the bowl.
fn toilet_sym(tank_w: f64, tank_d: f64, bowl_w: f64, total_d: f64) -> Vec<Stroke> {
    let hw = bowl_w / 2.0;
    let cy = total_d - hw;
    let mut s = rounded_rect(-tank_w / 2.0, 0.0, tank_w / 2.0, tank_d, 1.5);
    s.push(circle(0.0, tank_d / 2.0, 1.0)); // flush button
    s.push(line((-hw, tank_d), (-hw, cy)));
    s.push(line((hw, tank_d), (hw, cy)));
    s.push(arc(0.0, cy, hw, 0.0, 180.0));
    let ih = hw - 2.0;
    s.push(polyline(
        &[(-ih, cy), (-ih, tank_d + 2.0), (ih, tank_d + 2.0), (ih, cy)],
        false,
    ));
    s.push(arc(0.0, cy, ih, 0.0, 180.0));
    s
}

// ------------------------------------------------------------------- sinks

fn sinks(c: &mut Catalog) {
    let wall = Placement::WallMounted;
    let top = Placement::Countertop;
    let cat = [BK, "Sinks"];

    c.items.push(entry(
        "core.bathkitchen.sink_console_30",
        "Console Sink 30",
        wall,
        &cat,
        (30.0, 20.0, 34.0, 0.0),
        &["lavatory", "basin", "bathroom", "legs", "washstand"],
        console_sink(),
    ));
    let mut oval = vec![
        ellipse(0.0, 0.0, 9.5, 7.5, 24),
        ellipse(0.0, 0.4, 8.2, 5.9, 24),
        circle(0.0, 0.8, 0.9),  // drain
        circle(0.0, -6.7, 0.5), // faucet
    ];
    oval.push(arc(0.0, 0.4, 6.4, 200.0, 250.0));
    c.items.push(entry(
        "core.bathkitchen.sink_undermount_oval",
        "Undermount Oval Sink 19x15",
        top,
        &cat,
        (19.0, 15.0, 8.0, 26.0),
        &["lavatory", "basin", "bathroom", "vanity", "undermount"],
        oval,
    ));
    c.items.push(entry(
        "core.bathkitchen.sink_vessel_16",
        "Vessel Sink 16",
        top,
        &cat,
        (16.0, 16.0, 6.0, 34.0),
        &["lavatory", "basin", "bathroom", "vanity", "bowl"],
        vec![
            circle(0.0, 0.0, 8.0),
            circle(0.0, 0.0, 7.2),
            circle(0.0, 0.0, 0.9),
            arc(0.0, 0.0, 5.5, 200.0, 250.0),
        ],
    ));
    for (w, name) in [
        (60.0, "Double Vanity Sink 60"),
        (72.0, "Double Vanity Sink 72"),
    ] {
        let id = format!("core.bathkitchen.sink_double_vanity_{w:.0}");
        c.items.push(entry(
            &id,
            name,
            wall,
            &cat,
            (w, 22.0, 34.0, 0.0),
            &[
                "lavatory",
                "basin",
                "bathroom",
                "vanity cabinet",
                "his and hers",
                "double bowl",
            ],
            double_vanity(w),
        ));
    }
    c.items.push(entry(
        "core.bathkitchen.sink_farmhouse_33",
        "Farmhouse Apron Sink 33",
        top,
        &cat,
        (33.0, 22.0, 10.0, 26.0),
        &["kitchen", "apron front", "farm sink", "fireclay"],
        farmhouse_sink(),
    ));
    let mut bar = rounded_rect_c(0.0, 0.0, 15.0, 15.0, 2.0);
    bar.extend(rounded_rect_c(0.0, 0.5, 12.0, 11.0, 3.0));
    bar.push(circle(0.0, 0.5, 0.9));
    bar.push(circle(0.0, -6.5, 0.6));
    c.items.push(entry(
        "core.bathkitchen.sink_bar_15",
        "Bar Sink 15",
        top,
        &cat,
        (15.0, 15.0, 7.0, 29.0),
        &["prep sink", "wet bar", "kitchen", "basin"],
        bar,
    ));
    let mut laundry = vec![rect(-12.0, 0.0, 12.0, 22.0)];
    laundry.extend(rounded_rect(-10.0, 5.0, 10.0, 20.0, 2.0));
    laundry.push(circle(0.0, 2.5, 1.0));
    laundry.push(circle(-4.0, 2.5, 0.7));
    laundry.push(circle(4.0, 2.5, 0.7));
    laundry.push(circle(0.0, 12.5, 1.0));
    c.items.push(entry(
        "core.bathkitchen.sink_laundry_24",
        "Laundry Sink 24",
        wall,
        &cat,
        (24.0, 22.0, 34.0, 0.0),
        &["utility sink", "mud room", "laundry", "tub sink", "basin"],
        laundry,
    ));
}

fn console_sink() -> Vec<Stroke> {
    let mut s = rounded_rect(-15.0, 0.0, 15.0, 20.0, 2.0);
    s.extend(rounded_rect(-11.0, 4.0, 11.0, 17.0, 6.0));
    s.push(circle(0.0, 2.0, 1.1)); // faucet
    s.push(circle(0.0, 10.5, 0.8)); // drain
    s.push(circle(-13.5, 18.5, 1.0)); // legs
    s.push(circle(13.5, 18.5, 1.0));
    s
}

fn double_vanity(w: f64) -> Vec<Stroke> {
    let mut s = vec![rect(-w / 2.0, 0.0, w / 2.0, 22.0)];
    s.push(line((-w / 2.0 + 1.0, 1.0), (w / 2.0 - 1.0, 1.0))); // backsplash
    s.push(line((0.0, 3.0), (0.0, 22.0))); // center divider
    for sx in [-1.0, 1.0] {
        let cx = sx * w / 4.0;
        s.push(ellipse(cx, 12.5, 10.0, 7.5, 24));
        s.push(circle(cx, 3.2, 0.9)); // faucet
        s.push(circle(cx, 12.5, 0.9)); // drain
    }
    s
}

fn farmhouse_sink() -> Vec<Stroke> {
    let mut s = rounded_rect_c(0.0, 0.0, 33.0, 22.0, 2.0);
    s.extend(rounded_rect(-14.5, -8.0, 14.5, 9.0, 2.5));
    s.push(line((-14.5, 10.2), (14.5, 10.2))); // apron lip
    s.push(circle(0.0, 0.5, 1.2)); // drain
    s.push(circle(0.0, -9.6, 0.8)); // faucet
    s
}

// -------------------------------------------------------------------- tubs

fn tubs(c: &mut Catalog) {
    let wall = Placement::WallMounted;
    let cat = [BK, "Tubs"];

    c.items.push(entry(
        "core.bathkitchen.tub_alcove_60x30_left",
        "Alcove Tub 60x30, Left Drain",
        wall,
        &cat,
        (60.0, 30.0, 20.0, 0.0),
        &["bathtub", "tub", "soaking", "bathroom", "left hand"],
        alcove_tub(-1.0),
    ));
    c.items.push(entry(
        "core.bathkitchen.tub_alcove_60x30_right",
        "Alcove Tub 60x30, Right Drain",
        wall,
        &cat,
        (60.0, 30.0, 20.0, 0.0),
        &["bathtub", "tub", "soaking", "bathroom", "right hand"],
        alcove_tub(1.0),
    ));
    c.items.push(entry(
        "core.bathkitchen.tub_freestanding_66x32",
        "Freestanding Tub 66x32",
        Placement::FreeStanding,
        &cat,
        (66.0, 32.0, 24.0, 0.0),
        &["bathtub", "tub", "oval", "soaking", "clawfoot", "bathroom"],
        vec![
            ellipse(0.0, 0.0, 33.0, 16.0, 40),
            ellipse(0.0, 0.0, 30.0, 13.0, 40),
            circle(-22.0, 0.0, 1.2), // drain
            circle(-18.0, 0.0, 0.7), // overflow
            circle(31.5, 0.0, 0.9),  // faucet
        ],
    ));
    c.items.push(entry(
        "core.bathkitchen.tub_corner_60x60",
        "Corner Tub 60x60",
        wall,
        &cat,
        (60.0, 60.0, 20.0, 0.0),
        &["bathtub", "tub", "soaking", "bathroom", "triangular"],
        corner_tub(),
    ));
    let mut whirl = rounded_rect(-36.0, 0.0, 36.0, 36.0, 2.0);
    whirl.extend(rounded_rect(-32.0, 4.0, 32.0, 32.0, 10.0));
    for x in [-18.0, 0.0, 18.0] {
        whirl.push(circle(x, 7.5, 1.0)); // jets
        whirl.push(circle(x, 28.5, 1.0));
    }
    whirl.push(circle(-26.0, 18.0, 1.3)); // drain
    c.items.push(entry(
        "core.bathkitchen.tub_whirlpool_72x36",
        "Whirlpool Tub 72x36",
        wall,
        &cat,
        (72.0, 36.0, 22.0, 0.0),
        &["bathtub", "tub", "jetted", "spa", "jacuzzi", "bathroom"],
        whirl,
    ));
}

/// A 60x30 alcove tub; `sign` is -1 for a left drain and +1 for a right one.
fn alcove_tub(sign: f64) -> Vec<Stroke> {
    let mut s = rounded_rect(-30.0, 0.0, 30.0, 30.0, 2.0);
    s.extend(rounded_rect(-27.0, 3.0, 27.0, 27.0, 8.0));
    s.push(circle(sign * 23.0, 15.0, 1.2)); // drain
    s.push(circle(sign * 23.0, 24.0, 0.7)); // overflow
    s.push(circle(sign * 23.0, 1.5, 0.8)); // faucet
    s
}

/// A tub in a corner: walls along the back and the left, a quarter-round
/// front.
fn corner_tub() -> Vec<Stroke> {
    let a0 = (3.0f64 / 57.0).asin().to_degrees();
    let edge = 57.0 * a0.to_radians().cos();
    vec![
        line((-30.0, 0.0), (30.0, 0.0)),
        arc(-30.0, 0.0, 60.0, 0.0, 90.0),
        line((-30.0, 60.0), (-30.0, 0.0)),
        line((-27.0, 3.0), (-30.0 + edge, 3.0)),
        arc(-30.0, 0.0, 57.0, a0, 90.0 - a0),
        line((-27.0, edge), (-27.0, 3.0)),
        circle(-12.0, 12.0, 1.2), // drain
        circle(-12.0, 20.0, 0.7), // overflow
    ]
}

// ----------------------------------------------------------------- showers

fn showers(c: &mut Catalog) {
    let wall = Placement::WallMounted;
    let cat = [BK, "Showers"];

    c.items.push(entry(
        "core.bathkitchen.shower_36x36",
        "Shower Stall 36x36",
        wall,
        &cat,
        (36.0, 36.0, 80.0, 0.0),
        &["shower", "stall", "bathroom"],
        vec![
            rect(-18.0, 0.0, 18.0, 36.0),
            rect(-16.5, 1.5, 16.5, 34.5), // curb
            circle(0.0, 18.0, 2.0),       // drain
            circle(0.0, 3.0, 1.2),        // shower head
        ],
    ));
    c.items.push(entry(
        "core.bathkitchen.shower_48x36_glass",
        "Shower 48x36, Glass Enclosure",
        wall,
        &cat,
        (48.0, 36.0, 80.0, 0.0),
        &["shower", "stall", "bathroom", "glass door", "frameless"],
        vec![
            rect(-24.0, 0.0, 24.0, 36.0),
            rect(-22.5, 1.5, 22.5, 34.5),      // curb
            line((-22.5, 34.5), (22.5, 34.5)), // glass panel
            line((8.0, 34.5), (8.0, 36.0)),    // door edge
            circle(0.0, 16.0, 2.0),            // drain
            circle(-14.0, 3.0, 1.2),           // shower head
        ],
    ));
    c.items.push(entry(
        "core.bathkitchen.shower_60x36_curbless",
        "Shower 60x36, Curbless",
        wall,
        &cat,
        (60.0, 36.0, 80.0, 0.0),
        &[
            "shower",
            "bathroom",
            "barrier free",
            "zero threshold",
            "linear drain",
            "ada",
        ],
        vec![
            rect(-30.0, 0.0, 30.0, 36.0),
            rect(-27.0, 31.0, 27.0, 33.0),      // linear drain
            line((27.0, 1.0), (27.0, 30.0)),    // fixed glass panel
            circle(0.0, 3.0, 1.2),              // shower head
            line((-24.0, 20.0), (-24.0, 28.0)), // slope arrow
            polyline(&[(-26.0, 25.0), (-24.0, 28.0), (-22.0, 25.0)], false),
        ],
    ));
    c.items.push(entry(
        "core.bathkitchen.shower_neo_angle_38",
        "Shower, Neo-Angle 38",
        wall,
        &cat,
        (38.0, 38.0, 80.0, 0.0),
        &["shower", "bathroom", "corner shower", "neo angle", "angled"],
        vec![
            polyline(
                &[
                    (-19.0, 0.0),
                    (19.0, 0.0),
                    (19.0, 10.0),
                    (-9.0, 38.0),
                    (-19.0, 38.0),
                ],
                true,
            ),
            polyline(
                &[
                    (-17.5, 1.5),
                    (17.5, 1.5),
                    (17.5, 9.4),
                    (-9.6, 36.5),
                    (-17.5, 36.5),
                ],
                true,
            ),
            circle(-3.0, 15.0, 2.0), // drain
        ],
    ));
}

// ----------------------------------------------------------------- utility

fn utility(c: &mut Catalog) {
    let cat = [BK, "Laundry & Utility"];

    let mut heater = vec![
        circle(0.0, 0.0, 12.0),
        circle(0.0, 0.0, 10.2),
        circle(0.0, 0.0, 3.0),
    ];
    let (vx, vy) = polar(0.0, 0.0, 7.5, 45.0);
    heater.push(circle(vx, vy, 1.1)); // relief valve
    heater.push(circle(-5.0, 6.0, 0.9)); // supply and return stubs
    heater.push(circle(5.0, 6.0, 0.9));
    c.items.push(entry(
        "core.bathkitchen.water_heater_24",
        "Water Heater 24",
        Placement::FreeStanding,
        &cat,
        (24.0, 24.0, 60.0, 0.0),
        &["tank", "hot water", "boiler", "utility"],
        heater,
    ));

    let mut wd = rounded_rect(-13.5, 0.0, 13.5, 30.0, 1.5);
    wd.push(line((-13.5, 6.0), (13.5, 6.0))); // control panel
    wd.push(circle(-6.0, 3.0, 1.2));
    wd.push(circle(0.0, 3.0, 1.2));
    wd.push(circle(9.5, 3.0, 1.5)); // dryer vent
    wd.push(circle(0.0, 18.5, 8.0)); // door
    wd.push(circle(0.0, 18.5, 5.5)); // drum
    wd.push(circle(0.0, 18.5, 1.0));
    c.items.push(entry(
        "core.bathkitchen.washer_dryer_stacked",
        "Washer/Dryer, Stacked",
        Placement::WallMounted,
        &cat,
        (27.0, 30.0, 76.0, 0.0),
        &["laundry", "washer", "dryer", "stackable", "stack unit"],
        wd,
    ));
}

// ----------------------------------------------------------------- cooking

fn cooking(c: &mut Catalog) {
    let wall = Placement::WallMounted;
    let top = Placement::Countertop;
    let cat = [BK, "Kitchen", "Cooking"];

    for (w, label) in [(30.0, "30"), (36.0, "36")] {
        let id = format!("core.bathkitchen.range_hood_{label}");
        c.items.push(entry(
            &id,
            &format!("Range Hood {label}"),
            wall,
            &cat,
            (w, 20.0, 18.0, 66.0),
            &["vent hood", "exhaust", "kitchen", "canopy"],
            range_hood(w),
        ));
    }
    c.items.push(entry(
        "core.bathkitchen.cooktop_30_4burner",
        "Cooktop 30, 4 Burner",
        top,
        &cat,
        (30.0, 21.0, 4.0, 32.0),
        &[
            "stove top",
            "stovetop",
            "gas",
            "electric",
            "kitchen",
            "burners",
        ],
        cooktop(30.0, false),
    ));
    c.items.push(entry(
        "core.bathkitchen.cooktop_36_5burner",
        "Cooktop 36, 5 Burner",
        top,
        &cat,
        (36.0, 21.0, 4.0, 32.0),
        &[
            "stove top",
            "stovetop",
            "gas",
            "electric",
            "kitchen",
            "burners",
        ],
        cooktop(36.0, true),
    ));
    c.items.push(entry(
        "core.bathkitchen.wall_oven_30",
        "Wall Oven 30",
        wall,
        &cat,
        (30.0, 24.0, 28.0, 30.0),
        &["oven", "built-in oven", "kitchen", "single oven"],
        wall_oven(false),
    ));
    c.items.push(entry(
        "core.bathkitchen.double_oven_30",
        "Double Wall Oven 30",
        wall,
        &cat,
        (30.0, 24.0, 51.0, 24.0),
        &[
            "oven",
            "built-in oven",
            "kitchen",
            "double oven",
            "twin oven",
        ],
        wall_oven(true),
    ));
}

fn range_hood(w: f64) -> Vec<Stroke> {
    let hw = w / 2.0;
    vec![
        rect(-hw, 0.0, hw, 20.0),
        rect(-hw + 2.0, 2.0, hw - 2.0, 18.0),
        circle(0.0, 7.0, 3.5),        // duct
        circle(-hw / 2.0, 14.0, 1.5), // lights
        circle(hw / 2.0, 14.0, 1.5),
        line((-hw + 2.0, 11.0), (hw - 2.0, 11.0)), // filter edge
    ]
}

fn cooktop(w: f64, five: bool) -> Vec<Stroke> {
    let mut s = rounded_rect_c(0.0, 0.0, w, 21.0, 1.5);
    let mut burner = |x: f64, y: f64, r: f64| {
        s.push(circle(x, y, r));
        s.push(circle(x, y, r * 0.5));
    };
    if five {
        for (x, y) in [(-12.0, -5.0), (12.0, -5.0), (-12.0, 5.0), (12.0, 5.0)] {
            burner(x, y, 4.2);
        }
        burner(0.0, 0.0, 5.2);
    } else {
        for (x, y) in [(-7.5, -5.0), (7.5, -5.0), (-7.5, 5.0), (7.5, 5.0)] {
            burner(x, y, 4.2);
        }
    }
    s
}

fn wall_oven(double: bool) -> Vec<Stroke> {
    let mut s = vec![
        rect(-15.0, 0.0, 15.0, 24.0),
        rect(-13.5, 1.5, 13.5, 22.5),
        rect(-9.0, 6.0, 9.0, 18.0),      // door glass
        line((-9.0, 23.2), (9.0, 23.2)), // handle
    ];
    if double {
        s.push(line((-9.0, 21.0), (9.0, 21.0)));
        s.push(line((-13.5, 12.0), (-9.0, 12.0)));
        s.push(line((9.0, 12.0), (13.5, 12.0)));
    }
    s
}

// ------------------------------------------------------------ refrigeration

fn refrigeration(c: &mut Catalog) {
    let wall = Placement::WallMounted;
    let cat = [BK, "Kitchen", "Refrigeration"];
    let apps = [BK, "Kitchen", "Appliances"];

    c.items.push(entry(
        "core.bathkitchen.fridge_counter_depth_36",
        "Refrigerator, Counter-Depth 36",
        wall,
        &cat,
        (36.0, 25.0, 70.0, 0.0),
        &[
            "fridge",
            "freezer",
            "kitchen",
            "counter depth",
            "side by side",
        ],
        fridge(36.0, 25.0, false),
    ));
    c.items.push(entry(
        "core.bathkitchen.fridge_french_door_36",
        "Refrigerator, French Door 36",
        wall,
        &cat,
        (36.0, 32.0, 70.0, 0.0),
        &[
            "fridge",
            "freezer",
            "kitchen",
            "french door",
            "bottom freezer",
        ],
        fridge(36.0, 32.0, true),
    ));
    let mut wine = rounded_rect(-12.0, 0.0, 12.0, 24.0, 1.0);
    wine.push(line((-10.5, 22.0), (10.5, 22.0))); // glass door
    for y in [6.0, 10.0, 14.0, 18.0] {
        wine.push(line((-9.0, y), (9.0, y))); // racks
    }
    c.items.push(entry(
        "core.bathkitchen.wine_fridge_24",
        "Wine Fridge 24",
        wall,
        &cat,
        (24.0, 24.0, 34.0, 0.0),
        &[
            "wine cooler",
            "beverage center",
            "under counter",
            "refrigerator",
        ],
        wine,
    ));
    let mut compactor = rounded_rect(-7.5, 0.0, 7.5, 24.0, 1.0);
    compactor.push(rect(-6.0, 2.0, 6.0, 22.0));
    compactor.push(circle(0.0, 12.0, 4.0));
    compactor.push(line((-4.0, 12.0), (4.0, 12.0)));
    compactor.push(line((0.0, 8.0), (0.0, 16.0)));
    c.items.push(entry(
        "core.bathkitchen.trash_compactor_15",
        "Trash Compactor 15",
        wall,
        &apps,
        (15.0, 24.0, 34.0, 0.0),
        &["trash", "garbage", "waste", "kitchen", "under counter"],
        compactor,
    ));
    let mut ice = rounded_rect(-7.5, 0.0, 7.5, 24.0, 1.0);
    ice.push(line((-6.0, 22.0), (6.0, 22.0))); // door
    for k in 0..3 {
        // snowflake
        let a = 60.0 * f64::from(k);
        ice.push(line(
            polar(0.0, 11.0, 4.5, a),
            polar(0.0, 11.0, 4.5, a + 180.0),
        ));
    }
    c.items.push(entry(
        "core.bathkitchen.ice_maker_15",
        "Ice Maker 15",
        wall,
        &apps,
        (15.0, 24.0, 34.0, 0.0),
        &["icemaker", "ice", "under counter", "wet bar", "kitchen"],
        ice,
    ));
}

/// A refrigerator from the wall out: body, front door panel, door seam and
/// handles. A French-door model has a split seam and a dispenser.
fn fridge(w: f64, d: f64, french: bool) -> Vec<Stroke> {
    let hw = w / 2.0;
    let door = d - 3.5;
    let mut s = vec![
        rect(-hw, 0.0, hw, d),
        line((-hw, door), (hw, door)),
        line((0.0, door), (0.0, d)),
        line((-3.0, door + 1.0), (-3.0, d - 1.0)), // handles
        line((3.0, door + 1.0), (3.0, d - 1.0)),
    ];
    if french {
        s.push(line((-hw + 3.0, door), (-hw + 3.0, d)));
        s.push(line((hw - 3.0, door), (hw - 3.0, d)));
        s.push(rect(-hw + 5.0, 6.0, hw - 5.0, door - 3.0)); // cabinet top
    } else {
        s.push(rect(-hw + 3.0, 3.0, hw - 3.0, door - 3.0));
    }
    s
}
