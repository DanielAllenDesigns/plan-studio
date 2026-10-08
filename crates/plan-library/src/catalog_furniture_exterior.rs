//! The Furniture & Exterior catalog: seating, case goods, bedroom furniture,
//! outdoor living features, a pool and vehicles.
//!
//! Beds, cribs, case goods and uprights that stand against a wall are
//! `WallMounted` (back-center origin, wall at `y = 0`). Everything else is
//! `FreeStanding` and drawn about its center, with the back on the `-y` side
//! and the front facing `+y`.

use crate::catalog::{entry, Catalog, CatalogItem, Placement};
use crate::shapes::*;
use crate::symbol::Stroke;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

const FURN: &str = "Furniture";
const EXT: &str = "Exterior";

/// Returns the Furniture & Exterior catalog.
pub fn catalog() -> Catalog {
    let mut c = Catalog::new("Furniture & Exterior");
    seating(&mut c);
    living(&mut c);
    bedroom(&mut c);
    kitchen_and_bar(&mut c);
    outdoor_living(&mut c);
    site(&mut c);
    c
}

/// A free-standing item. `dims` is `(width, depth, height)`.
fn free(
    id: &str,
    name: &str,
    cat: &[&str],
    dims: (f64, f64, f64),
    tags: &[&str],
    strokes: Vec<Stroke>,
) -> CatalogItem {
    let size = (dims.0, dims.1, dims.2, 0.0);
    entry(id, name, Placement::FreeStanding, cat, size, tags, strokes)
}

/// A wall-backed item. `dims` is `(width, depth, height)`.
fn walled(
    id: &str,
    name: &str,
    cat: &[&str],
    dims: (f64, f64, f64),
    tags: &[&str],
    strokes: Vec<Stroke>,
) -> CatalogItem {
    let size = (dims.0, dims.1, dims.2, 0.0);
    entry(id, name, Placement::WallMounted, cat, size, tags, strokes)
}

// ----------------------------------------------------------------- seating

fn seating(c: &mut Catalog) {
    let cat = [FURN, "Seating"];

    c.items.push(free(
        "core.furniture.sectional_l_100",
        "Sectional Sofa, L-Shape 100x100",
        &cat,
        (100.0, 100.0, 34.0),
        &["couch", "living room", "corner sofa", "chaise sectional"],
        vec![
            polyline(
                &[
                    (-50.0, -50.0),
                    (50.0, -50.0),
                    (50.0, -14.0),
                    (-14.0, -14.0),
                    (-14.0, 50.0),
                    (-50.0, 50.0),
                ],
                true,
            ),
            polyline(&[(-40.0, 50.0), (-40.0, -40.0), (50.0, -40.0)], false), // back cushions
            line((20.0, -40.0), (20.0, -14.0)),
            line((-40.0, -14.0), (-14.0, -14.0)),
            line((-40.0, 18.0), (-14.0, 18.0)),
        ],
    ));
    let mut chaise = rounded_rect_c(0.0, 0.0, 70.0, 32.0, 4.0);
    chaise.push(line((-31.0, -10.0), (31.0, -10.0))); // back
    chaise.push(line((-24.0, -10.0), (-24.0, 16.0))); // raised head end
    chaise.extend(rounded_rect(-33.0, -6.0, -26.0, 12.0, 2.0)); // bolster
    c.items.push(free(
        "core.furniture.chaise_70x32",
        "Chaise Lounge 70x32",
        &cat,
        (70.0, 32.0, 32.0),
        &["chaise", "lounge", "daybed", "living room"],
        chaise,
    ));
    let mut recliner = rounded_rect_c(0.0, 0.0, 36.0, 38.0, 5.0);
    recliner.push(line((-12.5, -8.0), (12.5, -8.0))); // back
    recliner.push(line((-12.5, -8.0), (-12.5, 19.0))); // arms
    recliner.push(line((12.5, -8.0), (12.5, 19.0)));
    recliner.push(rect(-10.0, 9.0, 10.0, 19.0)); // footrest
    c.items.push(free(
        "core.furniture.recliner_36x38",
        "Recliner 36x38",
        &cat,
        (36.0, 38.0, 40.0),
        &[
            "chair",
            "armchair",
            "lounge chair",
            "living room",
            "theater seating",
        ],
        recliner,
    ));
    let mut ottoman = rounded_rect_c(0.0, 0.0, 30.0, 30.0, 4.0);
    ottoman.extend(rounded_rect_c(0.0, 0.0, 26.0, 26.0, 3.0));
    for (x, y) in [(-6.0, -6.0), (6.0, -6.0), (-6.0, 6.0), (6.0, 6.0)] {
        ottoman.push(circle(x, y, 1.0)); // tufting buttons
    }
    c.items.push(free(
        "core.furniture.ottoman_30x30",
        "Ottoman 30x30",
        &cat,
        (30.0, 30.0, 17.0),
        &["pouf", "footstool", "footrest", "living room"],
        ottoman,
    ));
}

// ------------------------------------------------------------ living room

fn living(c: &mut Catalog) {
    let cat = [FURN, "Living Room"];

    c.items.push(walled(
        "core.furniture.bookcase_36x12",
        "Bookcase 36x12",
        &cat,
        (36.0, 12.0, 72.0),
        &["bookshelf", "shelving", "shelves", "library", "storage"],
        vec![
            rect(-18.0, 0.0, 18.0, 12.0),
            rect(-16.5, 1.5, 16.5, 10.5),
            line((0.0, 1.5), (0.0, 10.5)),
        ],
    ));
    let mut media = rounded_rect(-36.0, 0.0, 36.0, 18.0, 1.0);
    media.push(line((-34.0, 2.0), (34.0, 2.0)));
    media.push(line((-12.0, 2.0), (-12.0, 16.0)));
    media.push(line((12.0, 2.0), (12.0, 16.0)));
    c.items.push(walled(
        "core.furniture.media_console_72x18",
        "Media Console 72x18",
        &cat,
        (72.0, 18.0, 24.0),
        &[
            "tv stand",
            "credenza",
            "entertainment center",
            "sideboard",
            "living room",
        ],
        media,
    ));
    c.items.push(entry(
        "core.furniture.tv_65_wall",
        "TV 65in, Wall-Mounted",
        Placement::WallMounted,
        &[FURN, "Electronics"],
        (57.0, 2.0, 33.0, 48.0),
        &["television", "flat screen", "display", "media", "65 inch"],
        vec![rect(-28.5, 0.0, 28.5, 2.0), line((-26.0, 1.0), (26.0, 1.0))],
    ));

    let mut upright = rounded_rect(-29.0, 0.0, 29.0, 24.0, 1.0);
    upright.push(rect(-27.0, 15.0, 27.0, 22.0)); // keyboard
    for k in 1..14 {
        let x = -27.0 + 4.0 * f64::from(k);
        upright.push(line((x, 15.0), (x, 22.0)));
    }
    c.items.push(walled(
        "core.furniture.piano_upright_58x24",
        "Piano, Upright 58x24",
        &cat,
        (58.0, 24.0, 48.0),
        &["music", "keyboard", "spinet", "music room"],
        upright,
    ));
    let mut grand = vec![fitted(
        &[
            (-30.0, 40.0),
            (30.0, 40.0),
            (30.0, 12.0),
            (26.0, 2.0),
            (20.0, -8.0),
            (12.0, -18.0),
            (4.0, -28.0),
            (-6.0, -36.0),
            (-16.0, -40.0),
            (-26.0, -38.0),
            (-30.0, -32.0),
        ],
        0.0,
        0.0,
        60.0,
        80.0,
        true,
    )];
    grand.push(rect(-27.0, 28.0, 27.0, 38.0)); // keyboard
    for k in 1..9 {
        let x = -27.0 + 6.0 * f64::from(k);
        grand.push(line((x, 28.0), (x, 38.0)));
    }
    c.items.push(free(
        "core.furniture.piano_grand_60x80",
        "Piano, Grand 60x80",
        &cat,
        (60.0, 80.0, 40.0),
        &["music", "baby grand", "keyboard", "music room"],
        grand,
    ));
}

// ----------------------------------------------------------------- bedroom

fn bedroom(c: &mut Catalog) {
    let cat = [FURN, "Bedroom"];

    let mut crib = rounded_rect(-15.0, 0.0, 15.0, 54.0, 1.5);
    crib.push(rect(-13.5, 1.5, 13.5, 52.5)); // rails
    crib.push(rect(-12.0, 3.0, 12.0, 51.0)); // mattress
    c.items.push(walled(
        "core.furniture.crib_30x54",
        "Crib 30x54",
        &cat,
        (30.0, 54.0, 36.0),
        &["nursery", "baby", "infant", "cot"],
        crib,
    ));
    c.items.push(walled(
        "core.furniture.bed_twin_39x75",
        "Bed, Twin 39x75",
        &cat,
        (39.0, 75.0, 25.0),
        &["bedroom", "single bed", "kids", "guest"],
        bed(39.0, &[0.0], 28.0),
    ));
    c.items.push(walled(
        "core.furniture.bed_full_54x75",
        "Bed, Full 54x75",
        &cat,
        (54.0, 75.0, 25.0),
        &["bedroom", "double bed", "kids", "guest"],
        bed(54.0, &[-13.5, 13.5], 24.0),
    ));
    let mut bunk = bed(39.0, &[0.0], 28.0);
    bunk.push(line((14.0, 22.0), (14.0, 70.0))); // ladder rail
    for k in 0..6 {
        let y = 26.0 + 8.0 * f64::from(k);
        bunk.push(line((14.0, y), (19.5, y))); // rungs
    }
    c.items.push(walled(
        "core.furniture.bed_bunk_39x75",
        "Bunk Bed 39x75",
        &cat,
        (39.0, 75.0, 66.0),
        &["bedroom", "twin over twin", "kids", "ladder"],
        bunk,
    ));
}

/// A bed with its head on the wall: frame, pillows centered at `pillows`
/// (each `pillow_w` wide) and a turned-down sheet.
fn bed(width: f64, pillows: &[f64], pillow_w: f64) -> Vec<Stroke> {
    let hw = width / 2.0;
    let mut s = rounded_rect(-hw, 0.0, hw, 75.0, 2.0);
    for &cx in pillows {
        s.extend(rounded_rect(
            cx - pillow_w / 2.0,
            7.0,
            cx + pillow_w / 2.0,
            21.0,
            4.0,
        ));
    }
    s.push(line((-hw, 38.0), (hw, 38.0)));
    s.push(line((-hw, 40.0), (hw, 40.0)));
    s
}

// ------------------------------------------------------- kitchen and bath

fn kitchen_and_bar(c: &mut Catalog) {
    let mut bench = rounded_rect_c(0.0, 0.0, 36.0, 14.0, 1.0);
    for y in [-3.5, 0.0, 3.5] {
        bench.push(line((-17.0, y), (17.0, y))); // slats
    }
    c.items.push(free(
        "core.furniture.bench_bathroom_36x14",
        "Bathroom Bench 36x14",
        &[FURN, "Bath & Kitchen"],
        (36.0, 14.0, 18.0),
        &["shower bench", "teak bench", "spa", "seat"],
        bench,
    ));

    // Island body with a seating overhang on the right and three stools
    // tucked in; the stools set the island's 48 inch width.
    let mut island = rounded_rect(-24.0, -42.0, 14.0, 42.0, 1.0);
    island.push(line((10.0, -40.0), (10.0, 40.0))); // overhang edge
    for y in [-24.0, 0.0, 24.0] {
        island.push(circle(17.0, y, 7.0));
        island.push(circle(17.0, y, 4.5));
    }
    c.items.push(free(
        "core.furniture.island_seating_48x84",
        "Kitchen Island with Seating 48x84",
        &[FURN, "Bath & Kitchen"],
        (48.0, 84.0, 36.0),
        &["kitchen", "counter", "breakfast bar", "stools", "peninsula"],
        island,
    ));

    let stool = |cx: f64| {
        vec![
            circle(cx, 0.0, 8.0),
            circle(cx, 0.0, 5.5),
            circle(cx, 0.0, 1.0),
        ]
    };
    c.items.push(free(
        "core.furniture.bar_stool_16",
        "Bar Stool 16",
        &[FURN, "Seating"],
        (16.0, 16.0, 30.0),
        &["stool", "counter stool", "barstool", "kitchen"],
        stool(0.0),
    ));
    let mut three = stool(-18.0);
    three.extend(stool(0.0));
    three.extend(stool(18.0));
    c.items.push(free(
        "core.furniture.bar_stools_set3",
        "Bar Stools, Set of 3",
        &[FURN, "Seating"],
        (52.0, 16.0, 30.0),
        &[
            "stool",
            "counter stool",
            "barstool",
            "kitchen",
            "island seating",
        ],
        three,
    ));
}

// ---------------------------------------------------------- outdoor living

fn outdoor_living(c: &mut Catalog) {
    let cat = [EXT, "Outdoor Living"];

    // Table 72x36 with six chairs tucked around it. The chairs overhang the
    // declared table size.
    let mut dining = rounded_rect_c(0.0, 0.0, 72.0, 36.0, 2.0);
    dining.push(circle(0.0, 0.0, 2.0)); // umbrella hole
    let chair = [rect(-8.5, -8.5, 8.5, 8.5), rect(-8.5, -11.5, 8.5, -8.5)];
    let spots = [
        ((-18.0, -24.5), 0.0),
        ((18.0, -24.5), 0.0),
        ((-18.0, 24.5), PI),
        ((18.0, 24.5), PI),
        ((-42.5, 0.0), -FRAC_PI_2),
        ((42.5, 0.0), FRAC_PI_2),
    ];
    for ((x, y), angle) in spots {
        for s in &chair {
            dining.push(s.transformed(plan_core::geometry::Point::new(x, y), angle, 1.0));
        }
    }
    c.items.push(free(
        "core.exterior.outdoor_dining_72x36",
        "Outdoor Dining Set 72x36, 6 Chairs",
        &cat,
        (72.0, 36.0, 30.0),
        &[
            "patio",
            "table",
            "chairs",
            "deck",
            "patio dining",
            "dining table",
        ],
        dining,
    ));

    let mut lounge = rounded_rect_c(0.0, 0.0, 72.0, 28.0, 3.0);
    lounge.extend(rounded_rect(-33.0, -11.0, -14.0, 11.0, 3.0)); // reclined back
    lounge.push(line((-14.0, -12.0), (-14.0, 12.0)));
    lounge.push(line((26.0, -12.0), (26.0, 12.0))); // foot section
    c.items.push(free(
        "core.exterior.patio_lounge_72x28",
        "Patio Lounge Chair 72x28",
        &cat,
        (72.0, 28.0, 14.0),
        &[
            "chaise",
            "pool lounger",
            "sun lounger",
            "deck",
            "outdoor seating",
        ],
        lounge,
    ));

    c.items.push(free(
        "core.exterior.grill_54x24",
        "Grill 54x24",
        &cat,
        (54.0, 24.0, 44.0),
        &["bbq", "barbecue", "outdoor kitchen", "gas grill", "patio"],
        vec![
            rect(-27.0, -12.0, 27.0, 12.0),
            line((-17.0, -12.0), (-17.0, 12.0)), // side shelf edge
            line((-14.0, 8.0), (24.0, 8.0)),     // lid handle
            line((-17.0, -8.0), (27.0, -8.0)),   // lid hinge
            circle(-22.0, 0.0, 3.5),             // side burner
            circle(-10.0, -10.0, 1.0),           // control knobs
            circle(0.0, -10.0, 1.0),
            circle(10.0, -10.0, 1.0),
            circle(20.0, -10.0, 1.0),
        ],
    ));

    let mut pit = vec![
        circle(0.0, 0.0, 18.0),
        circle(0.0, 0.0, 13.0),
        circle(0.0, 0.0, 5.0),
    ];
    for k in 0..12 {
        let (x, y) = polar(0.0, 0.0, 15.5, 30.0 * f64::from(k));
        pit.push(circle(x, y, 2.0)); // stones
    }
    c.items.push(free(
        "core.exterior.fire_pit_36",
        "Fire Pit 36",
        &cat,
        (36.0, 36.0, 18.0),
        &["fireplace", "outdoor fire", "patio", "bonfire", "round"],
        pit,
    ));

    let mut tub = rounded_rect_c(0.0, 0.0, 84.0, 84.0, 6.0);
    tub.extend(rounded_rect_c(0.0, 0.0, 76.0, 76.0, 5.0));
    tub.push(circle(0.0, 0.0, 22.0)); // footwell
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        tub.push(circle(sx * 30.0, sy * 30.0, 1.2)); // jets
    }
    c.items.push(free(
        "core.exterior.hot_tub_84x84",
        "Hot Tub 84x84",
        &cat,
        (84.0, 84.0, 36.0),
        &["spa", "jacuzzi", "whirlpool", "patio", "outdoor"],
        tub,
    ));

    let pool = |w: f64, h: f64| kidney(w, h);
    let mut p = vec![pool(180.0, 360.0), pool(168.0, 348.0)];
    p.push(line((-72.0, 0.0), (72.0, 0.0))); // shallow/deep break
    c.items.push(free(
        "core.exterior.pool_kidney_15x30",
        "Pool, Kidney 15x30",
        &cat,
        (180.0, 360.0, 60.0),
        &[
            "swimming pool",
            "kidney pool",
            "inground",
            "backyard",
            "water",
        ],
        p,
    ));
}

/// A kidney outline `w` wide and `h` long with its dent on the right.
fn kidney(w: f64, h: f64) -> Stroke {
    let pts: Vec<(f64, f64)> = (0..48)
        .map(|k| {
            let a = TAU * f64::from(k) / 48.0;
            let d = if a > PI { a - TAU } else { a };
            let r = 1.0 - 0.55 * (-(d / 0.7).powi(2)).exp();
            (r * a.cos(), r * a.sin())
        })
        .collect();
    // Long axis along Y: swap the axes before fitting.
    let swapped: Vec<(f64, f64)> = pts.iter().map(|&(x, y)| (y, x)).collect();
    fitted(&swapped, 0.0, 0.0, w, h, true)
}

// -------------------------------------------------------------------- site

fn site(c: &mut Catalog) {
    let cat = [EXT, "Site"];
    let vehicles = [EXT, "Vehicles"];

    c.items.push(free(
        "core.exterior.mailbox",
        "Mailbox",
        &cat,
        (8.0, 20.0, 54.0),
        &["post box", "curbside", "letterbox", "street"],
        vec![
            rect(-4.0, -10.0, 4.0, 10.0),
            line((-4.0, 7.0), (4.0, 7.0)), // door
            circle(0.0, -3.0, 2.0),        // post
        ],
    ));

    c.items.push(free(
        "core.exterior.bike",
        "Bicycle",
        &cat,
        (68.0, 22.0, 40.0),
        &["bike", "cycle", "garage", "storage"],
        vec![
            ellipse(-21.0, 0.0, 13.0, 1.2, 24), // wheels
            ellipse(21.0, 0.0, 13.0, 1.2, 24),
            line((-21.0, 0.0), (-4.0, 0.0)), // frame
            line((-4.0, 0.0), (18.0, 0.0)),
            line((18.0, -11.0), (18.0, 11.0)), // handlebar
            ellipse(-7.0, 0.0, 4.0, 1.6, 12),  // saddle
        ],
    ));

    c.items.push(free(
        "core.exterior.suv_78x192",
        "SUV 78x192",
        &vehicles,
        (78.0, 192.0, 72.0),
        &[
            "car",
            "vehicle",
            "driveway",
            "garage",
            "parking",
            "crossover",
        ],
        vehicle(78.0, 192.0, false),
    ));
    c.items.push(free(
        "core.exterior.pickup_80x230",
        "Pickup Truck 80x230",
        &vehicles,
        (80.0, 230.0, 76.0),
        &["truck", "vehicle", "driveway", "garage", "parking", "f150"],
        vehicle(80.0, 230.0, true),
    ));
}

/// A vehicle seen from above, front towards `+y`: body, cabin glass, roof,
/// doors, wheels and, for a pickup, an open bed.
fn vehicle(w: f64, len: f64, pickup: bool) -> Vec<Stroke> {
    let (hw, hl) = (w / 2.0, len / 2.0);
    let mut s = rounded_rect_c(0.0, 0.0, w, len, 12.0);
    let inner = hw - 4.0;
    if pickup {
        let cab_back = hl - 130.0; // cab spans 130 in from the nose
        s.push(rect(-inner, cab_back + 70.0, inner, hl - 4.0)); // hood
        s.push(rect(-inner, cab_back, inner, cab_back + 70.0)); // cab
        s.push(polyline(
            &[
                (-inner + 4.0, cab_back + 50.0),
                (inner - 4.0, cab_back + 50.0),
                (inner - 8.0, cab_back + 68.0),
                (-inner + 8.0, cab_back + 68.0),
            ],
            true,
        ));
        s.push(rect(-inner, -hl + 4.0, inner, cab_back - 2.0)); // bed
        s.push(line((-inner, -hl + 8.0), (inner, -hl + 8.0))); // tailgate
    } else {
        s.push(rect(-inner, -hl + 14.0, inner, hl - 56.0)); // roof
        s.push(polyline(
            &[
                (-inner + 2.0, hl - 56.0),
                (inner - 2.0, hl - 56.0),
                (inner - 6.0, hl - 80.0),
                (-inner + 6.0, hl - 80.0),
            ],
            true,
        )); // windshield
        s.push(polyline(
            &[
                (-inner + 4.0, -hl + 14.0),
                (inner - 4.0, -hl + 14.0),
                (inner - 8.0, -hl + 28.0),
                (-inner + 8.0, -hl + 28.0),
            ],
            true,
        )); // rear glass
    }
    for sx in [-1.0, 1.0] {
        let (x0, x1) = (sx * hw, sx * (hw - 4.0));
        for cy in [hl - 40.0, -hl + 40.0] {
            s.push(rect(x0.min(x1), cy - 13.0, x0.max(x1), cy + 13.0)); // wheels
        }
    }
    s
}
