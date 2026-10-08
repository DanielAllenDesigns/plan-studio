//! The Plants catalog: trees, shrubs, hedges, ground cover, grasses and
//! landscape features, each drawn as a plan-view planting symbol.
//!
//! Every plant is `FreeStanding` and drawn about its center; `width` and
//! `depth` are the mature canopy (or footprint) diameter in inches.

use crate::catalog::{entry, Catalog, CatalogItem, Placement};
use crate::shapes::*;
use crate::symbol::Stroke;

const PLANTS: &str = "Plants";

/// Returns the Plants catalog.
pub fn catalog() -> Catalog {
    let mut c = Catalog::new("Plants");
    trees(&mut c);
    shrubs(&mut c);
    landscape(&mut c);
    c
}

/// A plant at ground level. `dims` is `(width, depth, height)`.
fn plant(
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

// ------------------------------------------------------------------- trees

fn trees(c: &mut Catalog) {
    let deciduous = [PLANTS, "Trees", "Deciduous"];
    let evergreen = [PLANTS, "Trees", "Evergreen"];
    let ornamental = [PLANTS, "Trees", "Ornamental"];

    c.items.push(plant(
        "core.plants.crape_myrtle_10ft",
        "Crape Myrtle 10ft",
        &deciduous,
        (120.0, 120.0, 168.0),
        &["crape myrtle", "crepe myrtle", "deciduous", "small tree"],
        canopy_tree(120.0, 8),
    ));
    c.items.push(plant(
        "core.plants.oak_20ft",
        "Oak Tree 20ft",
        &deciduous,
        (240.0, 240.0, 420.0),
        &["oak", "deciduous", "shade tree"],
        canopy_tree(240.0, 12),
    ));
    c.items.push(plant(
        "core.plants.maple_30ft",
        "Maple Tree 30ft",
        &deciduous,
        (360.0, 360.0, 540.0),
        &["maple", "oak", "deciduous", "shade tree", "large tree"],
        canopy_tree(360.0, 16),
    ));
    c.items.push(plant(
        "core.plants.pine_20ft",
        "Pine Tree 20ft",
        &evergreen,
        (240.0, 240.0, 600.0),
        &["pine", "evergreen", "conifer", "spruce"],
        conifer(240.0),
    ));
    c.items.push(plant(
        "core.plants.cypress_12ft",
        "Cypress Tree 12ft",
        &evergreen,
        (144.0, 144.0, 360.0),
        &["cypress", "evergreen", "conifer", "cedar", "arborvitae"],
        conifer(144.0),
    ));
    c.items.push(plant(
        "core.plants.palm_12ft",
        "Palm Tree 12ft",
        &[PLANTS, "Trees", "Palm"],
        (144.0, 144.0, 240.0),
        &["palm", "tropical", "fronds"],
        palm(144.0),
    ));
    c.items.push(plant(
        "core.plants.japanese_maple_8ft",
        "Japanese Maple 8ft",
        &ornamental,
        (96.0, 96.0, 108.0),
        &["japanese maple", "maple", "ornamental tree", "acer"],
        ornamental_tree(96.0, 8, 0),
    ));
    c.items.push(plant(
        "core.plants.magnolia_20ft",
        "Magnolia 20ft",
        &ornamental,
        (240.0, 240.0, 360.0),
        &[
            "magnolia",
            "ornamental tree",
            "flowering",
            "southern magnolia",
        ],
        ornamental_tree(240.0, 12, 6),
    ));
}

/// Deciduous canopy: a scalloped outline, a ring of six leaf clusters and a
/// center dot for the trunk.
fn canopy_tree(canopy: f64, lobes: u32) -> Vec<Stroke> {
    let r = canopy / 2.0;
    let mut s = vec![scalloped_circle(0.0, 0.0, r, lobes, r * 0.12)];
    for k in 0..6 {
        let (x, y) = polar(0.0, 0.0, r * 0.5, 60.0 * f64::from(k) + 30.0);
        s.push(circle(x, y, r * 0.34));
    }
    s.push(circle(0.0, 0.0, canopy / 40.0));
    s
}

/// Evergreen: a spiky star outline with a smaller star inside and a dot.
fn conifer(canopy: f64) -> Vec<Stroke> {
    let r = canopy / 2.0;
    vec![
        star(0.0, 0.0, r, r * 0.5, 12),
        star(0.0, 0.0, r * 0.5, r * 0.22, 12),
        circle(0.0, 0.0, canopy / 48.0),
    ]
}

/// Palm: twelve diamond-shaped fronds radiating from the trunk.
fn palm(canopy: f64) -> Vec<Stroke> {
    let r = canopy / 2.0;
    let mut s = Vec::new();
    for k in 0..12 {
        let a = 30.0 * f64::from(k);
        s.push(polyline(
            &[
                polar(0.0, 0.0, r * 0.12, a),
                polar(0.0, 0.0, r * 0.5, a - 11.0),
                polar(0.0, 0.0, r, a),
                polar(0.0, 0.0, r * 0.5, a + 11.0),
            ],
            true,
        ));
    }
    s.push(circle(0.0, 0.0, canopy / 30.0));
    s
}

/// Ornamental tree: a double scalloped canopy, optionally with `flowers`
/// blossoms around the middle.
fn ornamental_tree(canopy: f64, lobes: u32, flowers: u32) -> Vec<Stroke> {
    let r = canopy / 2.0;
    let mut s = vec![
        scalloped_circle(0.0, 0.0, r, lobes, r * 0.18),
        scalloped_circle(0.0, 0.0, r * 0.6, lobes, r * 0.1),
    ];
    for k in 0..flowers {
        let (x, y) = polar(
            0.0,
            0.0,
            r * 0.78,
            360.0 * f64::from(k) / f64::from(flowers),
        );
        s.push(circle(x, y, r * 0.07));
    }
    s.push(circle(0.0, 0.0, canopy / 40.0));
    s
}

// ------------------------------------------------------------------ shrubs

fn shrubs(c: &mut Catalog) {
    let round = [PLANTS, "Shrubs", "Round"];
    let hedge = [PLANTS, "Shrubs", "Hedges"];

    c.items.push(plant(
        "core.plants.boxwood_2ft",
        "Boxwood Shrub 2ft",
        &round,
        (24.0, 24.0, 24.0),
        &["boxwood", "shrub", "evergreen shrub", "round shrub"],
        round_shrub(24.0),
    ));
    c.items.push(plant(
        "core.plants.hydrangea_3ft",
        "Hydrangea Shrub 3ft",
        &round,
        (36.0, 36.0, 36.0),
        &["hydrangea", "shrub", "flowering shrub", "round shrub"],
        round_shrub(36.0),
    ));
    c.items.push(plant(
        "core.plants.holly_4ft",
        "Holly Shrub 4ft",
        &round,
        (48.0, 48.0, 60.0),
        &["holly", "shrub", "evergreen shrub", "round shrub"],
        round_shrub(48.0),
    ));
    c.items.push(plant(
        "core.plants.hedge_4ft",
        "Hedge 4ft",
        &hedge,
        (48.0, 24.0, 48.0),
        &["hedge", "boxwood", "privet", "clipped hedge"],
        hedge_row(48.0),
    ));
    c.items.push(plant(
        "core.plants.hedge_8ft",
        "Hedge 8ft",
        &hedge,
        (96.0, 24.0, 48.0),
        &["hedge", "boxwood", "privet", "holly", "clipped hedge"],
        hedge_row(96.0),
    ));
}

/// Round shrub: scalloped outline around four leaf clusters.
fn round_shrub(d: f64) -> Vec<Stroke> {
    let r = d / 2.0;
    let mut s = vec![scalloped_circle(0.0, 0.0, r, 8, r * 0.14)];
    for k in 0..4 {
        let (x, y) = polar(0.0, 0.0, r * 0.45, 45.0 + 90.0 * f64::from(k));
        s.push(circle(x, y, r * 0.32));
    }
    s.push(circle(0.0, 0.0, r * 0.07));
    s
}

/// Hedge: a rectangle with scalloped long edges and a center line.
fn hedge_row(length: f64) -> Vec<Stroke> {
    let bumps = (length / 12.0).round() as u32;
    vec![
        wavy_rect(0.0, 0.0, length, 24.0, bumps, 3.0),
        line((-length / 2.0 + 5.0, 0.0), (length / 2.0 - 5.0, 0.0)),
    ]
}

// --------------------------------------------------------------- landscape

fn landscape(c: &mut Catalog) {
    let beds = [PLANTS, "Beds & Ground Cover"];
    let grasses = [PLANTS, "Grasses & Perennials"];
    let features = [PLANTS, "Landscape Features"];

    c.items.push(plant(
        "core.plants.ground_cover_patch",
        "Ground Cover Patch",
        &beds,
        (48.0, 36.0, 6.0),
        &["ground cover", "groundcover", "mulch", "pachysandra", "ivy"],
        ground_cover(),
    ));
    c.items.push(plant(
        "core.plants.flower_bed_border",
        "Flower Bed Border",
        &beds,
        (96.0, 36.0, 18.0),
        &["flower bed", "border", "annuals", "garden bed"],
        flower_bed(),
    ));
    c.items.push(plant(
        "core.plants.perennial_clump",
        "Perennial Clump",
        &grasses,
        (24.0, 24.0, 24.0),
        &["perennial", "hosta", "daylily", "flowers"],
        perennial_clump(),
    ));
    c.items.push(plant(
        "core.plants.ornamental_grass_30in",
        "Ornamental Grass 30in",
        &grasses,
        (30.0, 30.0, 36.0),
        &["grass", "ornamental grass", "fountain grass", "muhly"],
        grass_tuft(30.0, 16),
    ));
    c.items.push(plant(
        "core.plants.pampas_grass_48in",
        "Pampas Grass 48in",
        &grasses,
        (48.0, 48.0, 72.0),
        &["grass", "pampas grass", "ornamental grass", "plume"],
        grass_tuft(48.0, 24),
    ));
    c.items.push(plant(
        "core.plants.boulder",
        "Boulder",
        &features,
        (36.0, 30.0, 24.0),
        &["rock", "stone", "landscape boulder"],
        boulder(),
    ));
    c.items.push(plant(
        "core.plants.planter_box_36x16",
        "Planter Box 36x16",
        &features,
        (36.0, 16.0, 18.0),
        &["planter", "raised bed", "window box", "container"],
        planter_box(),
    ));
    c.items.push(plant(
        "core.plants.planter_pot_18",
        "Planter Pot 18",
        &features,
        (18.0, 18.0, 18.0),
        &["planter", "pot", "container", "urn"],
        vec![
            circle(0.0, 0.0, 9.0),
            circle(0.0, 0.0, 7.5),
            circle(0.0, 0.0, 1.5),
        ],
    ));
}

fn ground_cover() -> Vec<Stroke> {
    let mut s = vec![blob(0.0, 0.0, 24.0, 18.0, 5.0, 0.4)];
    for k in 0..8 {
        let (x, y) = polar(0.0, 0.0, 1.0, 45.0 * f64::from(k) + 10.0);
        s.push(circle(13.0 * x, 9.0 * y, 1.2));
    }
    s.push(circle(0.0, 0.0, 1.2));
    s
}

/// An elliptical bed with an inner edge and flowers around the border.
fn flower_bed() -> Vec<Stroke> {
    let mut s = vec![
        ellipse(0.0, 0.0, 48.0, 18.0, 40),
        ellipse(0.0, 0.0, 44.0, 14.0, 40),
    ];
    for k in 0..14 {
        let a = 360.0 * f64::from(k) / 14.0;
        let (x, y) = polar(0.0, 0.0, 1.0, a);
        s.push(circle(46.0 * x, 16.0 * y, 1.6));
    }
    s
}

fn perennial_clump() -> Vec<Stroke> {
    let mut s = vec![scalloped_circle(0.0, 0.0, 12.0, 8, 3.0)];
    for k in 0..5 {
        let (x, y) = polar(0.0, 0.0, 6.5, 72.0 * f64::from(k) + 18.0);
        s.push(circle(x, y, 1.8));
    }
    s.push(circle(0.0, 0.0, 1.2));
    s
}

/// A grass tuft: `blades` blades radiating from a small crown.
fn grass_tuft(d: f64, blades: u32) -> Vec<Stroke> {
    let r = d / 2.0;
    let mut s: Vec<Stroke> = (0..blades)
        .map(|k| {
            let a = 360.0 * f64::from(k) / f64::from(blades);
            line(polar(0.0, 0.0, r * 0.18, a), polar(0.0, 0.0, r, a))
        })
        .collect();
    s.push(circle(0.0, 0.0, r * 0.18));
    s
}

fn boulder() -> Vec<Stroke> {
    vec![
        polyline(
            &[
                (-18.0, -3.0),
                (-12.0, -11.0),
                (-2.0, -15.0),
                (9.0, -13.0),
                (17.0, -5.0),
                (18.0, 4.0),
                (11.0, 13.0),
                (0.0, 15.0),
                (-11.0, 12.0),
                (-17.0, 6.0),
            ],
            true,
        ),
        polyline(&[(-9.0, 7.0), (-2.0, 2.0), (6.0, 4.0), (10.0, -2.0)], false),
        line((-2.0, 2.0), (-4.0, -7.0)),
    ]
}

fn planter_box() -> Vec<Stroke> {
    vec![
        rect(-18.0, -8.0, 18.0, 8.0),
        rect(-16.5, -6.5, 16.5, 6.5),
        line((-16.5, -6.5), (16.5, 6.5)),
        line((-16.5, 6.5), (16.5, -6.5)),
    ]
}
