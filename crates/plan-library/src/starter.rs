//! The built-in starter catalog.
//!
//! Every symbol is built from [`crate::shapes`] primitives. Wall-backed items
//! are authored with their back-center at the origin (`y = 0` is the wall,
//! the item extends towards `+y`); everything else is authored about its
//! center. Sizes are inches.

use crate::catalog::{Catalog, CatalogItem, Placement};
use crate::shapes::*;
use crate::symbol::{Stroke, Symbol2d};
use plan_core::geometry::Point;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

const ARCH: &str = "Architectural";

/// Returns the built-in Core Catalog: plumbing fixtures, appliances,
/// cabinets, furniture, electrical symbols and exterior items, each with a
/// 2D plan symbol.
pub fn core_catalog() -> Catalog {
    let mut c = Catalog::new("Core Catalog");
    plumbing(&mut c);
    appliances(&mut c);
    cabinets(&mut c);
    furniture(&mut c);
    electrical(&mut c);
    exterior(&mut c);
    c
}

/// Shifts a back-center symbol so its origin is its center.
fn centered(sym: Symbol2d, depth: f64) -> Symbol2d {
    sym.transformed(Point::new(0.0, -depth / 2.0), 0.0, 1.0)
}

// ---------------------------------------------------------------- plumbing

fn plumbing(c: &mut Catalog) {
    let wall = Placement::WallMounted;
    let cat = |sub: &'static str| [ARCH, "Plumbing", sub];

    c.items.push(
        CatalogItem::new(
            "core.plumbing.toilet_elongated",
            "Toilet, Elongated",
            wall,
            toilet(),
        )
        .with_category(&cat("Toilets"))
        .with_size(20.0, 28.0, 30.0)
        .with_tags(&["wc", "water closet", "commode", "bathroom"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.plumbing.pedestal_sink",
            "Pedestal Sink",
            wall,
            pedestal_sink(),
        )
        .with_category(&cat("Sinks"))
        .with_size(20.0, 18.0, 34.0)
        .with_tags(&["lavatory", "basin", "bathroom"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.plumbing.vanity_sink_30",
            "Vanity Sink 30",
            wall,
            vanity_sink(),
        )
        .with_category(&cat("Sinks"))
        .with_size(30.0, 21.0, 34.0)
        .with_tags(&["lavatory", "basin", "bathroom", "vanity cabinet"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.plumbing.kitchen_sink_double",
            "Kitchen Sink, Double Bowl",
            Placement::Countertop,
            kitchen_sink(),
        )
        .with_category(&cat("Sinks"))
        .with_size(33.0, 22.0, 8.0)
        .with_elevation(28.0)
        .with_tags(&["kitchen", "double bowl", "basin"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.plumbing.bathtub_60x30",
            "Bathtub 60x30",
            wall,
            bathtub(),
        )
        .with_category(&cat("Bathtubs"))
        .with_size(60.0, 30.0, 20.0)
        .with_tags(&["tub", "soaking", "bathroom"]),
    );
    c.items.push(
        CatalogItem::new("core.plumbing.shower_36x36", "Shower 36x36", wall, shower())
            .with_category(&cat("Showers"))
            .with_size(36.0, 36.0, 80.0)
            .with_tags(&["stall", "bathroom"]),
    );
}

fn toilet() -> Symbol2d {
    let mut s = rounded_rect(-10.0, 0.0, 10.0, 8.0, 1.5); // tank
    s.push(circle(0.0, 4.0, 1.0)); // flush button
    s.push(line((-7.5, 8.0), (-7.5, 20.5))); // bowl outline
    s.push(line((7.5, 8.0), (7.5, 20.5)));
    s.push(arc(0.0, 20.5, 7.5, 0.0, 180.0));
    s.push(polyline(
        &[(-5.5, 20.5), (-5.5, 10.0), (5.5, 10.0), (5.5, 20.5)],
        false,
    )); // inner rim
    s.push(arc(0.0, 20.5, 5.5, 0.0, 180.0));
    Symbol2d::new(s)
}

fn pedestal_sink() -> Symbol2d {
    let s = vec![
        line((-10.0, 0.0), (10.0, 0.0)),
        line((-10.0, 0.0), (-10.0, 8.0)),
        line((10.0, 0.0), (10.0, 8.0)),
        arc(0.0, 8.0, 10.0, 0.0, 180.0),
        // basin
        line((-7.5, 3.0), (-7.5, 8.0)),
        line((7.5, 3.0), (7.5, 8.0)),
        line((-7.5, 3.0), (7.5, 3.0)),
        arc(0.0, 8.0, 7.5, 0.0, 180.0),
        circle(0.0, 7.0, 0.8), // drain
        circle(0.0, 1.5, 0.7), // faucet
    ];
    Symbol2d::new(s)
}

fn vanity_sink() -> Symbol2d {
    let mut s = vec![rect(-15.0, 0.0, 15.0, 21.0)];
    s.extend(rounded_rect(-9.5, 5.0, 9.5, 18.0, 4.0));
    s.push(circle(0.0, 11.5, 0.9)); // drain
    s.push(circle(0.0, 2.5, 1.1)); // faucet
    Symbol2d::new(s)
}

fn kitchen_sink() -> Symbol2d {
    let mut s = rounded_rect(-16.5, -11.0, 16.5, 11.0, 2.0);
    for sx in [-1.0, 1.0] {
        let (x0, x1) = if sx < 0.0 { (-15.0, -1.0) } else { (1.0, 15.0) };
        s.extend(rounded_rect(x0, -6.0, x1, 8.5, 3.0));
        s.push(circle(sx * 8.0, 1.25, 1.2)); // drain
    }
    s.push(circle(0.0, -8.5, 1.2)); // faucet
    Symbol2d::new(s)
}

fn bathtub() -> Symbol2d {
    let mut s = rounded_rect(-30.0, 0.0, 30.0, 30.0, 2.0);
    s.extend(rounded_rect(-27.0, 3.0, 27.0, 27.0, 8.0));
    s.push(circle(-23.0, 15.0, 1.2)); // drain
    s.push(circle(-23.0, 24.0, 0.7)); // overflow
    Symbol2d::new(s)
}

fn shower() -> Symbol2d {
    Symbol2d::new(vec![
        rect(-18.0, 0.0, 18.0, 36.0),
        rect(-16.0, 2.0, 16.0, 34.0), // curb
        line((-16.0, 2.0), (16.0, 34.0)),
        circle(0.0, 18.0, 2.0), // drain
    ])
}

// -------------------------------------------------------------- appliances

fn appliances(c: &mut Catalog) {
    let wall = Placement::WallMounted;
    let cat = |sub: &'static str| [ARCH, "Appliances", sub];

    c.items.push(
        CatalogItem::new("core.appliances.range_30", "Range 30", wall, range())
            .with_category(&cat("Cooking"))
            .with_size(30.0, 26.0, 36.0)
            .with_tags(&["stove", "oven", "cooktop", "kitchen"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.appliances.refrigerator_36x30",
            "Refrigerator 36x30",
            wall,
            refrigerator(),
        )
        .with_category(&cat("Refrigeration"))
        .with_size(36.0, 30.0, 70.0)
        .with_tags(&["fridge", "freezer", "kitchen"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.appliances.dishwasher_24",
            "Dishwasher 24",
            wall,
            dishwasher(),
        )
        .with_category(&cat("Dishwashers"))
        .with_size(24.0, 24.0, 34.0)
        .with_tags(&["kitchen"]),
    );
    c.items.push(
        CatalogItem::new("core.appliances.washer_27", "Washer 27", wall, washer())
            .with_category(&cat("Laundry"))
            .with_size(27.0, 28.0, 38.0)
            .with_tags(&["washing machine", "laundry"]),
    );
    c.items.push(
        CatalogItem::new("core.appliances.dryer_27", "Dryer 27", wall, dryer())
            .with_category(&cat("Laundry"))
            .with_size(27.0, 28.0, 38.0)
            .with_tags(&["clothes dryer", "laundry"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.appliances.microwave",
            "Microwave",
            Placement::Countertop,
            microwave(),
        )
        .with_category(&cat("Cooking"))
        .with_size(24.0, 16.0, 14.0)
        .with_elevation(36.0)
        .with_tags(&["kitchen", "oven"]),
    );
}

fn range() -> Symbol2d {
    let mut s = vec![rect(-15.0, 0.0, 15.0, 26.0)];
    for x in [-7.5, 7.5] {
        for y in [7.0, 17.0] {
            s.push(circle(x, y, 4.2));
            s.push(circle(x, y, 2.0));
        }
    }
    for x in [-9.0, -3.0, 3.0, 9.0] {
        s.push(circle(x, 24.0, 0.7)); // knobs
    }
    Symbol2d::new(s)
}

fn refrigerator() -> Symbol2d {
    let mut s = vec![
        rect(-18.0, 0.0, 18.0, 30.0),
        line((-18.0, 26.0), (18.0, 26.0)), // door panel edge
        line((0.0, 26.0), (0.0, 30.0)),    // door seam
    ];
    // Door swings, hinged at the outer corners and opened 90 degrees.
    s.push(arc(-18.0, 30.0, 18.0, 0.0, 90.0));
    s.push(line((-18.0, 30.0), (-18.0, 48.0)));
    s.push(arc(18.0, 30.0, 18.0, 90.0, 180.0));
    s.push(line((18.0, 30.0), (18.0, 48.0)));
    Symbol2d::new(s)
}

fn dishwasher() -> Symbol2d {
    Symbol2d::new(vec![
        rect(-12.0, 0.0, 12.0, 24.0),
        line((-12.0, 21.5), (12.0, 21.5)), // door panel edge
        line((-8.0, 23.0), (8.0, 23.0)),   // handle
        circle(0.0, 11.0, 5.0),            // spray arm
        line((-5.0, 11.0), (5.0, 11.0)),
        line((0.0, 6.0), (0.0, 16.0)),
    ])
}

fn washer() -> Symbol2d {
    let mut s = vec![
        rect(-13.5, 0.0, 13.5, 28.0),
        line((-13.5, 5.0), (13.5, 5.0)), // control panel edge
        circle(0.0, 17.0, 9.0),          // door
        circle(0.0, 17.0, 6.5),          // drum glass
    ];
    for x in [-8.0, 8.0] {
        s.push(circle(x, 2.5, 0.9)); // knobs
    }
    Symbol2d::new(s)
}

fn dryer() -> Symbol2d {
    Symbol2d::new(vec![
        rect(-13.5, 0.0, 13.5, 28.0),
        line((-13.5, 5.0), (13.5, 5.0)),
        circle(0.0, 17.0, 9.0),
        circle(0.0, 17.0, 4.5),
        circle(0.0, 2.5, 0.9),     // single dial
        rect(8.0, 0.5, 12.0, 2.5), // timer window
        // vent louvres along the door
        line((-4.0, 21.5), (4.0, 21.5)),
        line((-5.0, 23.5), (5.0, 23.5)),
    ])
}

fn microwave() -> Symbol2d {
    Symbol2d::new(vec![
        rect(-12.0, -8.0, 12.0, 8.0),
        rect(-10.5, -6.5, 5.0, 6.5),   // door window
        line((7.0, -8.0), (7.0, 8.0)), // control panel edge
        circle(-2.75, 0.0, 5.0),       // turntable
        circle(9.5, -4.0, 0.8),
        circle(9.5, 0.0, 0.8),
        circle(9.5, 4.0, 0.8),
    ])
}

// ---------------------------------------------------------------- cabinets

fn cabinets(c: &mut Catalog) {
    let wall = Placement::WallMounted;
    let cat = |sub: &'static str| [ARCH, "Cabinets", sub];

    for w in [24.0_f64, 30.0, 36.0] {
        let n = w as u32;
        c.items.push(
            CatalogItem::new(
                format!("core.cabinets.base_{n}"),
                format!("Base Cabinet {n}"),
                wall,
                cabinet(w, 24.0),
            )
            .with_category(&cat("Base Cabinets"))
            .with_size(w, 24.0, 34.5)
            .with_tags(&["kitchen", "lower cabinet"]),
        );
    }
    for w in [24.0_f64, 30.0, 36.0] {
        let n = w as u32;
        c.items.push(
            CatalogItem::new(
                format!("core.cabinets.wall_{n}"),
                format!("Wall Cabinet {n}"),
                wall,
                cabinet(w, 12.0),
            )
            .with_category(&cat("Wall Cabinets"))
            .with_size(w, 12.0, 30.0)
            .with_elevation(54.0)
            .with_tags(&["kitchen", "upper cabinet"]),
        );
    }
    c.items.push(
        CatalogItem::new(
            "core.cabinets.island_36x72",
            "Island 36x72",
            Placement::FreeStanding,
            island(),
        )
        .with_category(&cat("Islands"))
        .with_size(36.0, 72.0, 36.0)
        .with_tags(&["kitchen", "base cabinet"]),
    );
}

/// A cabinet plan rectangle with door-swing lines. Doors up to 24" wide are
/// single (hinged left); wider cabinets get a pair hinged at the outsides.
fn cabinet(width: f64, depth: f64) -> Symbol2d {
    let hw = width / 2.0;
    let mut s = vec![rect(-hw, 0.0, hw, depth)];
    let mid = depth / 2.0;
    if width <= 24.0 {
        s.push(door_swing((-hw, mid), (hw, 0.0), (hw, depth)));
    } else {
        s.push(door_swing((-hw, mid), (0.0, 0.0), (0.0, depth)));
        s.push(door_swing((hw, mid), (0.0, 0.0), (0.0, depth)));
        s.push(line((0.0, 0.0), (0.0, depth)));
    }
    Symbol2d::new(s)
}

fn island() -> Symbol2d {
    Symbol2d::new(vec![
        rect(-18.0, -36.0, 18.0, 36.0),
        rect(-16.0, -34.0, 16.0, 34.0), // countertop edge
        line((-16.0, -12.0), (16.0, -12.0)),
        line((-16.0, 12.0), (16.0, 12.0)),
    ])
}

// --------------------------------------------------------------- furniture

fn furniture(c: &mut Catalog) {
    let wall = Placement::WallMounted;
    let free = Placement::FreeStanding;
    let cat = |sub: &'static str| [ARCH, "Furniture", sub];

    c.items.push(
        CatalogItem::new(
            "core.furniture.bed_queen",
            "Bed Queen 60x80",
            wall,
            bed(60.0, 26.0, 14.5),
        )
        .with_category(&cat("Beds"))
        .with_size(60.0, 80.0, 25.0)
        .with_tags(&["bedroom", "mattress"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.furniture.bed_king",
            "Bed King 76x80",
            wall,
            bed(76.0, 34.0, 19.0),
        )
        .with_category(&cat("Beds"))
        .with_size(76.0, 80.0, 25.0)
        .with_tags(&["bedroom", "mattress"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.furniture.nightstand",
            "Nightstand 24x18",
            wall,
            nightstand(),
        )
        .with_category(&cat("Storage"))
        .with_size(24.0, 18.0, 26.0)
        .with_tags(&["bedroom", "bedside table"]),
    );
    c.items.push(
        CatalogItem::new("core.furniture.dresser", "Dresser 60x18", wall, dresser())
            .with_category(&cat("Storage"))
            .with_size(60.0, 18.0, 34.0)
            .with_tags(&["bedroom", "chest of drawers"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.furniture.sofa_84x36",
            "Sofa 84x36",
            wall,
            seating(84.0, 36.0, 7.0, 9.0, 3),
        )
        .with_category(&cat("Seating"))
        .with_size(84.0, 36.0, 34.0)
        .with_tags(&["couch", "living room"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.furniture.loveseat_60x36",
            "Loveseat 60x36",
            wall,
            seating(60.0, 36.0, 7.0, 9.0, 2),
        )
        .with_category(&cat("Seating"))
        .with_size(60.0, 36.0, 34.0)
        .with_tags(&["couch", "living room"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.furniture.armchair_36x36",
            "Armchair 36x36",
            free,
            centered(seating(36.0, 36.0, 7.0, 9.0, 1), 36.0),
        )
        .with_category(&cat("Seating"))
        .with_size(36.0, 36.0, 34.0)
        .with_tags(&["chair", "living room"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.furniture.coffee_table_48x24",
            "Coffee Table 48x24",
            free,
            coffee_table(),
        )
        .with_category(&cat("Tables"))
        .with_size(48.0, 24.0, 18.0)
        .with_tags(&["living room"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.furniture.dining_table_72x36",
            "Dining Table 72x36 with 6 Chairs",
            free,
            dining_table(),
        )
        .with_category(&cat("Tables"))
        .with_size(72.0, 36.0, 30.0)
        .with_tags(&["dining room", "chairs", "kitchen table"]),
    );
    c.items.push(
        CatalogItem::new("core.furniture.desk_60x30", "Desk 60x30", wall, desk())
            .with_category(&cat("Desks"))
            .with_size(60.0, 30.0, 30.0)
            .with_tags(&["office", "workstation"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.furniture.office_chair",
            "Office Chair",
            free,
            office_chair(),
        )
        .with_category(&cat("Seating"))
        .with_size(26.0, 26.0, 40.0)
        .with_tags(&["office", "task chair", "swivel"]),
    );
}

/// Bed with its headboard on the wall (`y = 0`) and two pillows.
fn bed(width: f64, pillow_w: f64, pillow_cx: f64) -> Symbol2d {
    let hw = width / 2.0;
    let mut s = rounded_rect(-hw, 0.0, hw, 80.0, 2.0);
    s.push(line((-hw, 4.0), (hw, 4.0))); // headboard
    for sx in [-1.0, 1.0] {
        let cx = sx * pillow_cx;
        s.extend(rounded_rect(
            cx - pillow_w / 2.0,
            7.0,
            cx + pillow_w / 2.0,
            21.0,
            4.0,
        ));
    }
    s.push(line((-hw, 38.0), (hw, 38.0))); // turned-down sheet
    s.push(line((-hw, 40.0), (hw, 40.0)));
    Symbol2d::new(s)
}

fn nightstand() -> Symbol2d {
    let mut s = rounded_rect(-12.0, 0.0, 12.0, 18.0, 1.0);
    s.extend(rounded_rect(-10.5, 1.5, 10.5, 16.5, 0.5));
    s.push(circle(0.0, 9.0, 3.5)); // lamp
    Symbol2d::new(s)
}

fn dresser() -> Symbol2d {
    let mut s = rounded_rect(-30.0, 0.0, 30.0, 18.0, 1.0);
    s.push(rect(-28.5, 1.5, 28.5, 16.5));
    s.push(line((-10.0, 1.5), (-10.0, 16.5)));
    s.push(line((10.0, 1.5), (10.0, 16.5)));
    Symbol2d::new(s)
}

/// Sofa-style seating with its back on the wall: arms, back, and `cushions`
/// seat cushions separated by dividing lines.
fn seating(width: f64, depth: f64, arm: f64, back: f64, cushions: u32) -> Symbol2d {
    let hw = width / 2.0;
    let mut s = rounded_rect(-hw, 0.0, hw, depth, 3.0);
    let (sx0, sx1) = (-hw + arm, hw - arm);
    s.push(line((sx0, back), (sx0, depth)));
    s.push(line((sx1, back), (sx1, depth)));
    s.push(line((sx0, back), (sx1, back)));
    for k in 1..cushions {
        let x = sx0 + (sx1 - sx0) * f64::from(k) / f64::from(cushions);
        s.push(line((x, back), (x, depth)));
    }
    Symbol2d::new(s)
}

fn coffee_table() -> Symbol2d {
    let mut s = rounded_rect_c(0.0, 0.0, 48.0, 24.0, 1.5);
    s.extend(rounded_rect_c(0.0, 0.0, 44.0, 20.0, 1.0));
    Symbol2d::new(s)
}

/// Dining chair about its center, back on the -Y side, facing +Y.
fn dining_chair() -> Symbol2d {
    let mut s = rounded_rect_c(0.0, 0.0, 17.0, 17.0, 3.0);
    s.extend(rounded_rect(-8.5, -11.5, 8.5, -8.5, 1.2));
    Symbol2d::new(s)
}

fn dining_table() -> Symbol2d {
    let mut sym = Symbol2d::new(rounded_rect(-36.0, -18.0, 36.0, 18.0, 1.0));
    let chair = dining_chair();
    let placements = [
        (Point::new(-18.0, -23.5), 0.0),
        (Point::new(18.0, -23.5), 0.0),
        (Point::new(-18.0, 23.5), PI),
        (Point::new(18.0, 23.5), PI),
        (Point::new(-41.5, 0.0), -FRAC_PI_2),
        (Point::new(41.5, 0.0), FRAC_PI_2),
    ];
    for (pos, angle) in placements {
        sym.merge(chair.transformed(pos, angle, 1.0));
    }
    sym
}

fn desk() -> Symbol2d {
    let mut s = rounded_rect(-30.0, 0.0, 30.0, 30.0, 1.0);
    s.push(rect(14.0, 0.0, 30.0, 30.0)); // drawer pedestal
    s.push(rect(-26.0, 10.0, 10.0, 26.0)); // blotter
    Symbol2d::new(s)
}

fn office_chair() -> Symbol2d {
    let mut s = rounded_rect_c(0.0, 0.0, 18.0, 18.0, 4.0); // seat
    s.extend(rounded_rect(-8.0, -12.5, 8.0, -9.5, 1.2)); // back
    for sx in [-1.0, 1.0] {
        s.extend(rounded_rect(
            sx * 10.5 - 0.75,
            -6.0,
            sx * 10.5 + 0.75,
            6.0,
            0.7,
        )); // arms
    }
    for k in 0..5 {
        // five-star base
        let a = FRAC_PI_2 + TAU * f64::from(k) / 5.0;
        let tip = (13.0 * a.cos() * 0.92, 13.0 * a.sin() * 0.92);
        s.push(line((0.0, 0.0), tip));
        s.push(circle(12.0 * a.cos(), 12.0 * a.sin(), 1.0)); // caster
    }
    Symbol2d::new(s)
}

// -------------------------------------------------------------- electrical

fn electrical(c: &mut Catalog) {
    let wall = Placement::WallMounted;
    let ceil = Placement::Ceiling;
    let cat = |sub: &'static str| [ARCH, "Electrical", sub];

    c.items.push(
        CatalogItem::new(
            "core.electrical.outlet_duplex",
            "Duplex Outlet",
            wall,
            duplex(),
        )
        .with_category(&cat("Outlets"))
        .with_size(6.0, 6.0, 4.0)
        .with_elevation(16.0)
        .with_tags(&["receptacle", "120v", "plug"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.electrical.outlet_220v",
            "220V Outlet",
            wall,
            outlet_220(),
        )
        .with_category(&cat("Outlets"))
        .with_size(6.0, 6.0, 4.0)
        .with_elevation(18.0)
        .with_tags(&["receptacle", "240v", "dryer", "range", "plug"]),
    );
    c.items.push(
        CatalogItem::new("core.electrical.outlet_gfci", "GFCI Outlet", wall, gfci())
            .with_category(&cat("Outlets"))
            .with_size(6.0, 6.0, 4.0)
            .with_elevation(42.0)
            .with_tags(&["receptacle", "ground fault", "wet location", "plug"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.electrical.switch_single",
            "Single-Pole Switch",
            wall,
            switch(false),
        )
        .with_category(&cat("Switches"))
        .with_size(6.0, 6.0, 4.0)
        .with_elevation(48.0)
        .with_tags(&["light switch", "s"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.electrical.switch_3way",
            "3-Way Switch",
            wall,
            switch(true),
        )
        .with_category(&cat("Switches"))
        .with_size(6.0, 6.0, 4.0)
        .with_elevation(48.0)
        .with_tags(&["light switch", "three way", "s3"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.electrical.ceiling_light",
            "Ceiling Light",
            ceil,
            ceiling_light(),
        )
        .with_category(&cat("Lighting"))
        .with_size(12.0, 12.0, 4.0)
        .with_elevation(92.0)
        .with_tags(&["fixture", "flush mount"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.electrical.recessed_can",
            "Recessed Can",
            ceil,
            Symbol2d::new(vec![circle(0.0, 0.0, 3.0), circle(0.0, 0.0, 1.8)]),
        )
        .with_category(&cat("Lighting"))
        .with_size(6.0, 6.0, 6.0)
        .with_elevation(90.0)
        .with_tags(&["downlight", "can light", "pot light"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.electrical.ceiling_fan",
            "Ceiling Fan",
            ceil,
            ceiling_fan(),
        )
        .with_category(&cat("Lighting"))
        .with_size(52.0, 52.0, 12.0)
        .with_elevation(84.0)
        .with_tags(&["fan", "blades"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.electrical.smoke_detector",
            "Smoke Detector",
            ceil,
            smoke_detector(),
        )
        .with_category(&cat("Safety"))
        .with_size(8.0, 8.0, 2.0)
        .with_elevation(94.0)
        .with_tags(&["alarm", "fire", "life safety"]),
    );
}

/// The enclosing circle of a wall device: radius 3 touching the wall.
fn device_circle() -> Stroke {
    circle(0.0, 3.0, 3.0)
}

fn duplex() -> Symbol2d {
    Symbol2d::new(vec![
        device_circle(),
        line((-1.0, 0.7), (-1.0, 5.3)),
        line((1.0, 0.7), (1.0, 5.3)),
    ])
}

fn outlet_220() -> Symbol2d {
    let mut s = vec![device_circle()];
    for deg in [90.0_f64, 210.0, 330.0] {
        let r = deg.to_radians();
        s.push(line((0.0, 3.0), (2.4 * r.cos(), 3.0 + 2.4 * r.sin())));
    }
    Symbol2d::new(s)
}

fn gfci() -> Symbol2d {
    Symbol2d::new(vec![
        device_circle(),
        line((-1.4, 0.8), (-1.4, 1.8)),
        line((1.4, 0.8), (1.4, 1.8)),
        rect(-1.5, 2.2, 1.5, 4.2), // test/reset box
        line((-1.4, 4.6), (-1.4, 5.2)),
        line((1.4, 4.6), (1.4, 5.2)),
    ])
}

fn switch(three_way: bool) -> Symbol2d {
    let mut s = vec![device_circle()];
    if three_way {
        s.extend(s_glyph(-1.1, 3.0, 0.7));
        s.extend(three_glyph(1.1, 3.0, 0.7));
    } else {
        s.extend(s_glyph(0.0, 3.0, 1.0));
    }
    Symbol2d::new(s)
}

fn ceiling_light() -> Symbol2d {
    Symbol2d::new(vec![
        circle(0.0, 0.0, 6.0),
        line((-6.0, 0.0), (6.0, 0.0)),
        line((0.0, -6.0), (0.0, 6.0)),
    ])
}

fn ceiling_fan() -> Symbol2d {
    let mut s = vec![circle(0.0, 0.0, 4.0)];
    for k in 0..4 {
        let a = FRAC_PI_2 * f64::from(k);
        // A tapered blade along +X, rotated into place.
        let blade = Symbol2d::new(vec![polyline(
            &[(4.0, -2.0), (26.0, -4.5), (26.0, 4.5), (4.0, 2.0)],
            true,
        )]);
        s.extend(blade.transformed(Point::ZERO, a, 1.0).strokes);
    }
    Symbol2d::new(s)
}

fn smoke_detector() -> Symbol2d {
    let mut s = vec![circle(0.0, 0.0, 4.0), circle(0.0, 0.0, 3.0)];
    s.extend(s_glyph(0.0, 0.0, 0.7));
    Symbol2d::new(s)
}

// ---------------------------------------------------------------- exterior

fn exterior(c: &mut Catalog) {
    let cat = |sub: &'static str| [ARCH, "Exterior", sub];

    c.items.push(
        CatalogItem::new(
            "core.exterior.tree_deciduous",
            "Tree, Deciduous",
            Placement::FreeStanding,
            tree(),
        )
        .with_category(&cat("Plants"))
        .with_size(48.0, 48.0, 300.0)
        .with_tags(&["landscape", "canopy", "shade tree"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.exterior.shrub",
            "Shrub",
            Placement::FreeStanding,
            shrub(),
        )
        .with_category(&cat("Plants"))
        .with_size(36.0, 36.0, 36.0)
        .with_tags(&["landscape", "bush", "hedge"]),
    );
    c.items.push(
        CatalogItem::new(
            "core.exterior.car",
            "Car 72x180",
            Placement::FreeStanding,
            car(),
        )
        .with_category(&cat("Vehicles"))
        .with_size(72.0, 180.0, 60.0)
        .with_tags(&["automobile", "vehicle", "driveway", "garage"]),
    );
}

/// A ring of `lobes` circles of radius `lobe_r` at `ring_r` from the center.
fn lobes(lobes: u32, ring_r: f64, lobe_r: f64) -> Vec<Stroke> {
    (0..lobes)
        .map(|k| {
            let a = TAU * f64::from(k) / f64::from(lobes);
            circle(ring_r * a.cos(), ring_r * a.sin(), lobe_r)
        })
        .collect()
}

fn tree() -> Symbol2d {
    let mut s = lobes(8, 16.0, 8.0);
    s.push(circle(0.0, 0.0, 14.0));
    s.push(circle(0.0, 0.0, 1.5)); // trunk
    Symbol2d::new(s)
}

fn shrub() -> Symbol2d {
    let mut s = lobes(8, 9.0, 9.0);
    s.push(circle(0.0, 0.0, 6.0));
    Symbol2d::new(s)
}

/// Top view of a car facing +Y.
fn car() -> Symbol2d {
    let mut s = rounded_rect(-36.0, -90.0, 36.0, 90.0, 14.0);
    // Cabin, windshield and rear window.
    s.push(polyline(
        &[
            (-25.0, -52.0),
            (25.0, -52.0),
            (31.0, -30.0),
            (31.0, 10.0),
            (27.0, 30.0),
            (-27.0, 30.0),
            (-31.0, 10.0),
            (-31.0, -30.0),
        ],
        true,
    ));
    s.push(line((-29.0, 10.0), (29.0, 10.0)));
    s.push(line((-28.0, -40.0), (28.0, -40.0)));
    // Wheels.
    for (x0, x1) in [(-36.0, -32.0), (32.0, 36.0)] {
        for yc in [58.0, -58.0] {
            s.push(rect(x0, yc - 12.0, x1, yc + 12.0));
        }
    }
    // Headlights and taillights.
    for sx in [-1.0, 1.0] {
        s.push(line((sx * 30.0, 86.0), (sx * 20.0, 88.5)));
        s.push(line((sx * 30.0, -86.0), (sx * 20.0, -88.5)));
    }
    Symbol2d::new(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Library;
    use std::collections::HashSet;

    fn lib() -> Library {
        Library::with_core()
    }

    #[test]
    fn core_catalog_has_enough_unique_items_with_symbols() {
        let cat = core_catalog();
        assert!(cat.items.len() >= 40, "only {} items", cat.items.len());
        let ids: HashSet<&str> = cat.items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids.len(), cat.items.len(), "duplicate ids");
        for item in &cat.items {
            assert!(!item.symbol.is_empty(), "{} has no symbol", item.id);
            assert!(item.id.starts_with("core."), "{}", item.id);
            assert!(item.category.first().map(String::as_str) == Some(ARCH));
            assert!(item.width > 0.0 && item.depth > 0.0 && item.height > 0.0);
            assert!(item.elevation >= 0.0);
            let b = item.symbol.bounds().expect("bounds");
            assert!(
                b.width() > 0.0 && b.height() > 0.0,
                "{} is degenerate",
                item.id
            );
        }
    }

    #[test]
    fn symbols_match_declared_footprint() {
        // Symbols may overhang their footprint only when they draw door
        // swings or tucked-in chairs.
        let overhang = [
            "core.appliances.refrigerator_36x30",
            "core.furniture.dining_table_72x36",
            "core.furniture.office_chair",
        ];
        for item in core_catalog().items {
            if overhang.contains(&item.id.as_str()) {
                continue;
            }
            let b = item.symbol.bounds().unwrap();
            assert!(
                (b.width() - item.width).abs() <= 1.0,
                "{}: symbol width {} vs {}",
                item.id,
                b.width(),
                item.width
            );
            assert!(
                (b.height() - item.depth).abs() <= 1.0,
                "{}: symbol depth {} vs {}",
                item.id,
                b.height(),
                item.depth
            );
            if item.placement == Placement::WallMounted {
                assert!(b.min.y.abs() < 1e-6, "{} back edge not on wall", item.id);
                assert!(b.center().x.abs() < 1e-6, "{} not centered on x", item.id);
            } else {
                assert!(
                    b.center().x.abs() < 0.5 && b.center().y.abs() < 0.5,
                    "{}",
                    item.id
                );
            }
        }
    }

    #[test]
    fn json_round_trip_preserves_items() {
        let cat = core_catalog();
        let json = cat.to_json().unwrap();
        let back = Catalog::from_json(&json).unwrap();
        assert_eq!(back.items.len(), cat.items.len());
        assert_eq!(back, cat);
    }

    #[test]
    fn search_sink_returns_the_three_sinks() {
        let lib = lib();
        let hits = lib.search("sink");
        assert_eq!(
            hits.len(),
            3,
            "{:?}",
            hits.iter().map(|i| &i.name).collect::<Vec<_>>()
        );
        assert!(hits.iter().all(|i| i.name.to_lowercase().contains("sink")));
        let names: Vec<&str> = hits.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"Kitchen Sink, Double Bowl"));
        assert!(names.contains(&"Pedestal Sink"));
        assert!(names.contains(&"Vanity Sink 30"));
        // Equal name matches fall back to alphabetical order.
        assert_eq!(names[0], "Kitchen Sink, Double Bowl");
        assert_eq!(names[1], "Pedestal Sink");
    }

    #[test]
    fn search_matches_tags_and_categories() {
        let lib = lib();
        assert_eq!(
            lib.search("fridge")[0].id,
            "core.appliances.refrigerator_36x30"
        );
        assert!(lib.search("electrical").len() >= 9);
        assert_eq!(lib.get("core.exterior.car").unwrap().name, "Car 72x180");
    }

    #[test]
    fn tree_has_architectural_root_with_matching_counts() {
        let lib = lib();
        let total = lib.len();
        let tree = lib.tree();
        assert_eq!(tree.count, total);
        let arch = tree.child("Architectural").expect("Architectural root");
        assert_eq!(arch.count, total);
        let sum: usize = arch.children.iter().map(|c| c.count).sum();
        assert_eq!(sum, total);
        for name in [
            "Plumbing",
            "Appliances",
            "Cabinets",
            "Furniture",
            "Electrical",
            "Exterior",
        ] {
            assert!(arch.child(name).is_some(), "missing {name}");
        }
    }

    #[test]
    fn dining_table_includes_six_chairs() {
        let item = core_catalog()
            .items
            .into_iter()
            .find(|i| i.id == "core.furniture.dining_table_72x36")
            .unwrap();
        let b = item.symbol.bounds().unwrap();
        assert!(b.width() > 72.0 && b.height() > 36.0);
        // table (4 lines + 4 arcs) plus six chairs of 16 strokes each
        assert_eq!(item.symbol.strokes.len(), 8 + 6 * 16);
    }

    #[test]
    fn placed_symbol_rotates_into_plan_space() {
        let toilet = core_catalog()
            .items
            .into_iter()
            .find(|i| i.id == "core.plumbing.toilet_elongated")
            .unwrap();
        // Toilet on a wall running along +Y (rotated 90 degrees): the bowl
        // extends along -X from the wall.
        let placed = toilet
            .symbol
            .transformed(Point::new(100.0, 50.0), FRAC_PI_2, 1.0);
        let b = placed.bounds().unwrap();
        assert!((b.max.x - 100.0).abs() < 1e-6);
        assert!((b.width() - 28.0).abs() < 0.01);
        assert!((b.height() - 20.0).abs() < 0.01);
    }
}
