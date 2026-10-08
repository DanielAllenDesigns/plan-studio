//! The Space Planning questionnaire and the room list it expands into.

use crate::arrange::arrange;
use crate::boxes::{snap_to_grid, RoomBox, GRID};
use plan_core::Point;
use serde::{Deserialize, Serialize};

/// Answers to the Space Planning Assistant's questionnaire.
///
/// Sizes are target areas in square feet. `baths` counts a half bath as 0.5,
/// so `2.5` means two full baths (one of them the master bath) plus a powder
/// room.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Questionnaire {
    pub bedrooms: u32,
    pub baths: f32,
    pub garage_bays: u32,
    pub stories: u32,
    pub living_sq_ft: f64,
    pub kitchen_sq_ft: f64,
    pub dining_sq_ft: f64,
    pub master_sq_ft: f64,
    pub bedroom_sq_ft: f64,
    pub office: bool,
    pub laundry: bool,
    pub mudroom: bool,
    pub pantry: bool,
    pub covered_porch: bool,
    pub deck: bool,
}

impl Default for Questionnaire {
    /// 3 bed / 2.5 bath / 2-car garage / 1 story, with a laundry room.
    fn default() -> Self {
        Self {
            bedrooms: 3,
            baths: 2.5,
            garage_bays: 2,
            stories: 1,
            living_sq_ft: 320.0,
            kitchen_sq_ft: 200.0,
            dining_sq_ft: 160.0,
            master_sq_ft: 224.0,
            bedroom_sq_ft: 144.0,
            office: false,
            laundry: true,
            mudroom: false,
            pantry: false,
            covered_porch: false,
            deck: false,
        }
    }
}

/// One room the questionnaire asks for, before it becomes a [`RoomBox`].
struct Spec {
    name: String,
    room_type: &'static str,
    /// Width x height in inches, already on the grid.
    size: (f64, f64),
    /// Requested area, sq ft (before rounding to the grid).
    target_sq_ft: f64,
    floor: usize,
}

/// Size for `sq_ft` at width:height ratio `aspect`, sides rounded to 6".
fn size_for_area(sq_ft: f64, aspect: f64) -> (f64, f64) {
    let h_in = (sq_ft.max(1.0) * 144.0 / aspect).sqrt();
    let w_in = h_in * aspect;
    (
        snap_to_grid(w_in).max(2.0 * GRID),
        snap_to_grid(h_in).max(2.0 * GRID),
    )
}

/// A fixed-size room given in feet.
fn fixed(name: &str, room_type: &'static str, w_ft: f64, h_ft: f64, floor: usize) -> Spec {
    Spec {
        name: name.to_string(),
        room_type,
        size: (w_ft * 12.0, h_ft * 12.0),
        target_sq_ft: w_ft * h_ft,
        floor,
    }
}

/// A room sized from a requested area.
fn sized(name: &str, room_type: &'static str, sq_ft: f64, aspect: f64, floor: usize) -> Spec {
    Spec {
        name: name.to_string(),
        room_type,
        size: size_for_area(sq_ft, aspect),
        target_sq_ft: sq_ft,
        floor,
    }
}

/// Expand the questionnaire into the list of rooms (in a stable order).
fn room_specs(q: &Questionnaire) -> Vec<Spec> {
    let upper = usize::from(q.stories > 1);
    let mut s = vec![
        fixed("Entry", "Entry", 8.0, 8.0, 0),
        sized("Living Room", "Living", q.living_sq_ft, 1.25, 0),
        sized("Kitchen", "Kitchen", q.kitchen_sq_ft, 4.0 / 3.0, 0),
        sized("Dining", "Dining", q.dining_sq_ft, 1.2, 0),
    ];
    if q.garage_bays > 0 {
        // 12' for one bay, 22' for two (the Chief default), +10' per extra bay.
        let w = 10.0 * f64::from(q.garage_bays) + 2.0;
        s.push(fixed("Garage", "Garage", w, 22.0, 0));
    }
    if q.bedrooms > 0 {
        s.push(sized(
            "Master Bedroom",
            "Master Bedroom",
            q.master_sq_ft,
            1.2,
            upper,
        ));
        for n in 2..=q.bedrooms {
            s.push(sized(
                &format!("Bedroom {n}"),
                "Bedroom",
                q.bedroom_sq_ft,
                1.2,
                upper,
            ));
        }
        // Hall length grows with the number of doors it has to serve.
        s.push(fixed(
            "Hall",
            "Hall",
            4.0,
            8.0 + 4.0 * f64::from(q.bedrooms),
            upper,
        ));
    }
    let baths = q.baths.max(0.0);
    let full = baths.floor() as u32;
    let half = baths - baths.floor() >= 0.5;
    if q.bedrooms > 0 && full > 0 {
        s.push(fixed("Master Bath", "Master Bath", 8.0, 10.0, upper));
    }
    let plain_baths = if q.bedrooms > 0 {
        full.saturating_sub(1)
    } else {
        full
    };
    for n in 1..=plain_baths {
        s.push(fixed(&format!("Bath {n}"), "Bath", 5.0, 8.0, upper));
    }
    if half {
        s.push(fixed("Powder Room", "Powder Room", 5.0, 6.0, 0));
    }
    if q.laundry {
        s.push(fixed("Laundry", "Laundry", 6.0, 8.0, 0));
    }
    if q.office {
        s.push(fixed("Office", "Office", 10.0, 12.0, 0));
    }
    if q.mudroom {
        s.push(fixed("Mud Room", "Mud Room", 6.0, 8.0, 0));
    }
    if q.pantry {
        s.push(fixed("Pantry", "Pantry", 4.0, 6.0, 0));
    }
    if q.covered_porch {
        s.push(fixed("Porch", "Porch", 14.0, 8.0, 0));
    }
    if q.deck {
        s.push(fixed("Deck", "Deck", 16.0, 12.0, 0));
    }
    s
}

impl Questionnaire {
    /// Total area (sq ft) of every room the questionnaire asks for: the sizes
    /// the user typed plus Chief's stock sizes for baths, garage, entry, hall,
    /// laundry, office, mud room, pantry, porch and deck.
    pub fn expected_area_sq_ft(&self) -> f64 {
        room_specs(self).iter().map(|s| s.target_sq_ft).sum()
    }
}

/// Turn the questionnaire into room boxes.
///
/// One box per room, with a Chief-like default shape (bedrooms about 1.2:1,
/// kitchen 4:3, baths 5'x8', master bath 8'x10', 22'x22' two-car garage),
/// room type, color and floor (bedrooms, baths and the hall go to floor 1 when
/// `stories > 1`). Ids start at 1.
///
/// The boxes come back already arranged; see [`crate::arrange`] for the
/// heuristic. Every floor is shifted so its bounding box starts at (0, 0).
pub fn generate_boxes(q: &Questionnaire) -> Vec<RoomBox> {
    let mut boxes: Vec<RoomBox> = room_specs(q)
        .into_iter()
        .zip(1u64..)
        .map(|(s, id)| RoomBox::new(id, s.name, s.room_type, Point::ZERO, s.size, s.floor))
        .collect();
    arrange(&mut boxes);
    boxes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boxes::{overlaps, validate};

    /// ASCII sketch of one floor, 2 ft per character (debugging aid).
    fn sketch(boxes: &[RoomBox], floor: usize) -> String {
        let fl: Vec<&RoomBox> = boxes.iter().filter(|b| b.floor == floor).collect();
        let w = fl.iter().map(|b| b.rect.1.x).fold(0.0, f64::max);
        let h = fl.iter().map(|b| b.rect.1.y).fold(0.0, f64::max);
        let (cols, rows) = ((w / 24.0).ceil() as usize, (h / 24.0).ceil() as usize);
        let mut out = String::new();
        for r in (0..rows).rev() {
            for c in 0..cols {
                let p = Point::new(c as f64 * 24.0 + 12.0, r as f64 * 24.0 + 12.0);
                let ch = fl
                    .iter()
                    .find(|b| {
                        p.x > b.rect.0.x && p.x < b.rect.1.x && p.y > b.rect.0.y && p.y < b.rect.1.y
                    })
                    .map_or('.', |b| b.name.chars().next().unwrap_or('?'));
                out.push(ch);
            }
            out.push('\n');
        }
        out
    }

    fn assert_clean(boxes: &[RoomBox]) {
        for (i, a) in boxes.iter().enumerate() {
            for b in &boxes[i + 1..] {
                assert!(
                    a.floor != b.floor || !overlaps(&a.rect, &b.rect),
                    "{} overlaps {}",
                    a.name,
                    b.name
                );
            }
        }
        let issues = validate(boxes);
        assert!(issues.is_empty(), "issues: {issues:?}");
    }

    #[test]
    fn default_questionnaire_yields_valid_arrangement() {
        let q = Questionnaire::default();
        let boxes = generate_boxes(&q);
        println!("{}", sketch(&boxes, 0));
        assert!(boxes.len() >= 12, "{} boxes", boxes.len());
        assert_clean(&boxes);
        let total: f64 = boxes.iter().map(RoomBox::area_sq_ft).sum();
        let expected = q.expected_area_sq_ft();
        assert!(
            (total - expected).abs() / expected < 0.2,
            "{total} vs {expected}"
        );
        let asked = q.living_sq_ft
            + q.kitchen_sq_ft
            + q.dining_sq_ft
            + q.master_sq_ft
            + 2.0 * q.bedroom_sq_ft;
        assert!(total > asked);
        for b in &boxes {
            assert_eq!(b.rect.0.x % GRID, 0.0);
            assert_eq!(b.rect.0.y % GRID, 0.0);
            assert_eq!(b.width() % GRID, 0.0);
        }
        let names: Vec<&str> = boxes.iter().map(|b| b.name.as_str()).collect();
        for n in [
            "Master Bedroom",
            "Bedroom 2",
            "Bedroom 3",
            "Master Bath",
            "Bath 1",
            "Living Room",
            "Kitchen",
            "Dining",
            "Garage",
            "Entry",
            "Hall",
        ] {
            assert!(names.contains(&n), "missing {n}");
        }
        let garage = boxes.iter().find(|b| b.name == "Garage").unwrap();
        assert_eq!((garage.width(), garage.height()), (264.0, 264.0));
    }

    #[test]
    fn two_story_with_all_options_is_valid() {
        let q = Questionnaire {
            bedrooms: 4,
            baths: 3.5,
            stories: 2,
            office: true,
            mudroom: true,
            pantry: true,
            covered_porch: true,
            deck: true,
            ..Questionnaire::default()
        };
        let boxes = generate_boxes(&q);
        println!("{}\n--\n{}", sketch(&boxes, 0), sketch(&boxes, 1));
        assert_clean(&boxes);
        let up: Vec<&str> = boxes
            .iter()
            .filter(|b| b.floor == 1)
            .map(|b| b.room_type.as_str())
            .collect();
        assert!(up.iter().filter(|t| **t == "Bedroom").count() == 3);
        assert!(up.contains(&"Master Bedroom") && up.contains(&"Hall"));
        assert!(boxes
            .iter()
            .filter(|b| b.floor == 0)
            .all(|b| !b.room_type.contains("Bedroom")));
        let total: f64 = boxes.iter().map(RoomBox::area_sq_ft).sum();
        assert!((total - q.expected_area_sq_ft()).abs() / total < 0.2);
    }

    #[test]
    fn small_and_edge_questionnaires_do_not_panic() {
        for (bed, baths, bays) in [(0, 0.0, 0), (1, 1.0, 1), (2, 0.5, 3), (5, 4.0, 0)] {
            let q = Questionnaire {
                bedrooms: bed,
                baths,
                garage_bays: bays,
                ..Questionnaire::default()
            };
            let boxes = generate_boxes(&q);
            assert_clean(&boxes);
        }
    }

    #[test]
    fn questionnaire_serde_round_trip() {
        let q = Questionnaire {
            deck: true,
            baths: 3.5,
            ..Questionnaire::default()
        };
        let back: Questionnaire =
            serde_json::from_str(&serde_json::to_string(&q).unwrap()).unwrap();
        assert_eq!(q, back);
    }
}
