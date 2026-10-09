//! The AutoCAD Color Index (ACI): 256 colours computed from the standard
//! table's pattern, and the nearest-index search the export side uses.

/// RGB of AutoCAD colour `index` (1..=255). Index 7 is white in the table
/// but prints black on paper, so it is returned as black here (the plan is
/// drawn on a white sheet; `export` maps black and white both to 7).
/// Index 0 (BYBLOCK) and 256 (BYLAYER) are not colours: they give `None`.
pub fn aci_rgb(index: i32) -> Option<[u8; 3]> {
    match index {
        1 => Some([255, 0, 0]),
        2 => Some([255, 255, 0]),
        3 => Some([0, 255, 0]),
        4 => Some([0, 255, 255]),
        5 => Some([0, 0, 255]),
        6 => Some([255, 0, 255]),
        7 => Some([0, 0, 0]),
        8 => Some([65, 65, 65]),
        9 => Some([128, 128, 128]),
        10..=249 => {
            let n = (index - 10) as usize;
            let hue = (n / 10) as f64 * 15.0;
            let step = n % 10;
            // Value falls 100, 80, 60, 50, 30 percent in pairs; every second
            // entry is the half-saturated twin.
            let value = [1.0, 0.8, 0.6, 0.5, 0.3][step / 2];
            let sat = if step.is_multiple_of(2) { 1.0 } else { 0.5 };
            Some(hsv(hue, sat, value))
        }
        250 => Some([51, 51, 51]),
        251 => Some([91, 91, 91]),
        252 => Some([132, 132, 132]),
        253 => Some([173, 173, 173]),
        254 => Some([214, 214, 214]),
        255 => Some([255, 255, 255]),
        _ => None,
    }
}

fn hsv(h: f64, s: f64, v: f64) -> [u8; 3] {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    // The table truncates (127.5 is 127).
    let q = |f: f64| ((f + m) * 255.0 + 1e-6).floor() as u8;
    [q(r), q(g), q(b)]
}

/// The closest ACI index (1..=255) to an RGB colour; black and white both
/// give 7.
pub fn nearest_aci(rgb: [u8; 3]) -> i32 {
    if rgb == [0, 0, 0] || rgb == [255, 255, 255] {
        return 7;
    }
    let dist = |c: [u8; 3]| -> i32 {
        (0..3)
            .map(|i| {
                let d = i32::from(rgb[i]) - i32::from(c[i]);
                d * d
            })
            .sum()
    };
    (1..=255)
        .filter(|i| *i != 7)
        .filter_map(|i| aci_rgb(i).map(|c| (i, dist(c))))
        .min_by_key(|(_, d)| *d)
        .map_or(7, |(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_entries_match_the_table() {
        assert_eq!(aci_rgb(1), Some([255, 0, 0]));
        assert_eq!(aci_rgb(10), Some([255, 0, 0]));
        assert_eq!(aci_rgb(11), Some([255, 127, 127]));
        assert_eq!(aci_rgb(12), Some([204, 0, 0]));
        assert_eq!(aci_rgb(13), Some([204, 102, 102]));
        assert_eq!(aci_rgb(14), Some([153, 0, 0]));
        assert_eq!(aci_rgb(16), Some([127, 0, 0]));
        assert_eq!(aci_rgb(18), Some([76, 0, 0]));
        assert_eq!(aci_rgb(30), Some([255, 127, 0]));
        assert_eq!(aci_rgb(40), Some([255, 191, 0]));
        assert_eq!(aci_rgb(50), Some([255, 255, 0]));
        assert_eq!(aci_rgb(70), Some([127, 255, 0]));
        assert_eq!(aci_rgb(90), Some([0, 255, 0]));
        assert_eq!(aci_rgb(110), Some([0, 255, 127]));
        assert_eq!(aci_rgb(130), Some([0, 255, 255]));
        assert_eq!(aci_rgb(150), Some([0, 127, 255]));
        assert_eq!(aci_rgb(170), Some([0, 0, 255]));
        assert_eq!(aci_rgb(250), Some([51, 51, 51]));
        assert_eq!(aci_rgb(255), Some([255, 255, 255]));
        assert_eq!(aci_rgb(0), None);
        assert_eq!(aci_rgb(256), None);
    }

    #[test]
    fn nearest_finds_the_exact_entry() {
        assert_eq!(nearest_aci([255, 0, 0]), 1);
        assert_eq!(nearest_aci([0, 0, 0]), 7);
        assert_eq!(nearest_aci([255, 255, 255]), 7);
        assert_eq!(nearest_aci([0, 0, 254]), 5);
        for i in [20, 95, 140, 201, 240, 252] {
            assert_eq!(aci_rgb(nearest_aci(aci_rgb(i).unwrap())), aci_rgb(i));
        }
    }
}
