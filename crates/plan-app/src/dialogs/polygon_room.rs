//! New Polygon Shaped Room / New Polygon Shaped Deck (W-123, W-126, manual
//! pp. 375, 377): number of sides, the size as a side length, a radius to
//! the corners or a radius to the sides, the orientation, and (decks) Include
//! Railing. OK arms the Wall tool; one click then places the closed ring of
//! walls around the click (`WallTool::place_polygon`, one undo step). The
//! settings are remembered, one set for rooms and one for decks.

use eframe::egui;
use plan_core::geometry::Point;
use std::cell::{Cell, RefCell};

/// Room or deck.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PolyKind {
    Room,
    Deck,
}

/// What the size field means.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SizeBy {
    SideLength,
    RadiusToCorner,
    RadiusToSide,
}

/// The wall of a room: exterior or interior.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RoomWalls {
    Exterior,
    Interior,
}

/// Everything the dialog collects.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PolygonSpec {
    pub kind: PolyKind,
    pub sides: u32,
    pub size_by: SizeBy,
    /// Inches; read according to `size_by`.
    pub size: f64,
    /// Turns the polygon from its flat-bottom position, degrees.
    pub orientation: f64,
    pub walls: RoomWalls,
    /// Decks: railing walls (true) or deck edges (false).
    pub include_railing: bool,
}

pub const MIN_SIDES: u32 = 3;
pub const MAX_SIDES: u32 = 64;

impl PolygonSpec {
    pub fn new(kind: PolyKind) -> Self {
        Self {
            kind,
            sides: 6,
            size_by: SizeBy::SideLength,
            size: 120.0,
            orientation: 0.0,
            walls: RoomWalls::Exterior,
            include_railing: true,
        }
    }

    fn half_angle(&self) -> f64 {
        std::f64::consts::PI / self.sides.clamp(MIN_SIDES, MAX_SIDES) as f64
    }

    /// Radius from the center to the corners.
    pub fn radius_to_corner(&self) -> f64 {
        match self.size_by {
            SizeBy::RadiusToCorner => self.size,
            SizeBy::RadiusToSide => self.size / self.half_angle().cos(),
            SizeBy::SideLength => self.size / (2.0 * self.half_angle().sin()),
        }
    }

    /// Length of one side.
    pub fn side_length(&self) -> f64 {
        2.0 * self.radius_to_corner() * self.half_angle().sin()
    }

    /// Radius from the center to the middle of the sides.
    pub fn radius_to_side(&self) -> f64 {
        self.radius_to_corner() * self.half_angle().cos()
    }

    /// The corners around `center`, counter-clockwise; orientation 0 puts a
    /// side flat at the bottom (a square is square to the axes).
    pub fn corners(&self, center: Point) -> Vec<Point> {
        let n = self.sides.clamp(MIN_SIDES, MAX_SIDES);
        let r = self.radius_to_corner();
        let first = (self.orientation - 90.0).to_radians() + self.half_angle();
        (0..n)
            .map(|i| {
                let a = first + std::f64::consts::TAU * i as f64 / n as f64;
                Point::new(center.x + r * a.cos(), center.y + r * a.sin())
            })
            .collect()
    }
}

thread_local! {
    static WINDOW: RefCell<Option<PolygonSpec>> = const { RefCell::new(None) };
    static ARMED: Cell<Option<PolygonSpec>> = const { Cell::new(None) };
    static LAST_ROOM: Cell<Option<PolygonSpec>> = const { Cell::new(None) };
    static LAST_DECK: Cell<Option<PolygonSpec>> = const { Cell::new(None) };
}

fn last(kind: PolyKind) -> &'static std::thread::LocalKey<Cell<Option<PolygonSpec>>> {
    match kind {
        PolyKind::Room => &LAST_ROOM,
        PolyKind::Deck => &LAST_DECK,
    }
}

/// Opens the dialog with the remembered settings.
pub fn open(kind: PolyKind) {
    let spec = last(kind)
        .with(|c| c.get())
        .unwrap_or_else(|| PolygonSpec::new(kind));
    ARMED.with(|a| a.set(None));
    WINDOW.with(|w| *w.borrow_mut() = Some(spec));
}

/// OK: remembers the settings and arms the click that places the polygon.
pub fn accept(spec: PolygonSpec) {
    last(spec.kind).with(|c| c.set(Some(spec)));
    ARMED.with(|a| a.set(Some(spec)));
}

/// The settings waiting for a click, if the dialog was confirmed.
pub fn armed() -> Option<PolygonSpec> {
    ARMED.with(|a| a.get())
}

/// Forgets the armed settings (tool change); a dialog still open stays.
pub fn disarm() {
    ARMED.with(|a| a.set(None));
}

/// True while the dialog is on screen.
pub fn is_open() -> bool {
    WINDOW.with(|w| w.borrow().is_some())
}

/// Draws the open dialog (called every frame from `build_tools::show_all`).
pub fn show(ctx: &egui::Context) {
    let Some(mut spec) = WINDOW.with(|w| w.borrow_mut().take()) else {
        return;
    };
    let title = match spec.kind {
        PolyKind::Room => "New Polygon Shaped Room",
        PolyKind::Deck => "New Polygon Shaped Deck",
    };
    let (mut open, mut ok, mut cancel) = (true, false, false);
    egui::Window::new(title)
        .id(egui::Id::new("polygon_room"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            egui::Grid::new("polygon_room_grid")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Number of sides");
                    ui.add(egui::DragValue::new(&mut spec.sides).range(MIN_SIDES..=MAX_SIDES));
                    ui.end_row();
                    ui.label("Size is");
                    ui.vertical(|ui| {
                        ui.radio_value(&mut spec.size_by, SizeBy::SideLength, "Side length");
                        ui.radio_value(
                            &mut spec.size_by,
                            SizeBy::RadiusToCorner,
                            "Radius to corner",
                        );
                        ui.radio_value(&mut spec.size_by, SizeBy::RadiusToSide, "Radius to side");
                    });
                    ui.end_row();
                    ui.label("Size");
                    ui.add(
                        egui::DragValue::new(&mut spec.size)
                            .speed(1.0)
                            .range(6.0..=100_000.0)
                            .suffix(" in"),
                    );
                    ui.end_row();
                    ui.label("Orientation");
                    ui.add(
                        egui::DragValue::new(&mut spec.orientation)
                            .speed(0.5)
                            .suffix("\u{b0}"),
                    );
                    ui.end_row();
                    match spec.kind {
                        PolyKind::Room => {
                            ui.label("Walls");
                            ui.horizontal(|ui| {
                                ui.radio_value(&mut spec.walls, RoomWalls::Exterior, "Exterior");
                                ui.radio_value(&mut spec.walls, RoomWalls::Interior, "Interior");
                            });
                            ui.end_row();
                        }
                        PolyKind::Deck => {
                            ui.label("Include Railing");
                            ui.checkbox(&mut spec.include_railing, "");
                            ui.end_row();
                        }
                    }
                });
            ui.label(format!(
                "Side {:.1} in, radius to corner {:.1} in, radius to side {:.1} in",
                spec.side_length(),
                spec.radius_to_corner(),
                spec.radius_to_side()
            ));
            ui.horizontal(|ui| {
                ok = ui.button("OK").clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
    if ok {
        accept(spec);
    } else if open && !cancel {
        WINDOW.with(|w| *w.borrow_mut() = Some(spec));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_size_choices_describe_the_same_polygon() {
        let mut s = PolygonSpec::new(PolyKind::Room);
        s.sides = 8;
        s.size = 100.0;
        let (rc, rs, side) = (s.radius_to_corner(), s.radius_to_side(), s.side_length());
        assert!((side - 100.0).abs() < 1e-9);
        let mut t = s;
        t.size_by = SizeBy::RadiusToCorner;
        t.size = rc;
        assert!((t.side_length() - 100.0).abs() < 1e-9);
        t.size_by = SizeBy::RadiusToSide;
        t.size = rs;
        assert!((t.side_length() - 100.0).abs() < 1e-9);
        assert!(rs < rc);
    }

    #[test]
    fn a_four_sided_polygon_is_a_square_to_the_axes_and_orientation_turns_it() {
        let mut s = PolygonSpec::new(PolyKind::Room);
        s.sides = 4;
        s.size = 120.0;
        let c = s.corners(Point::new(0.0, 0.0));
        assert_eq!(c.len(), 4);
        for i in 0..4 {
            let (a, b) = (c[i], c[(i + 1) % 4]);
            assert!((a.dist(b) - 120.0).abs() < 1e-6);
            assert!(
                (a.x - b.x).abs() < 1e-6 || (a.y - b.y).abs() < 1e-6,
                "{a:?} {b:?}"
            );
        }
        s.orientation = 45.0;
        let d = s.corners(Point::new(0.0, 0.0));
        assert!(d.iter().any(|p| p.x.abs() < 1e-6 || p.y.abs() < 1e-6));
    }

    #[test]
    fn settings_are_remembered_per_kind_and_ok_arms_the_click() {
        let mut s = PolygonSpec::new(PolyKind::Deck);
        s.sides = 5;
        accept(s);
        assert_eq!(armed().map(|a| a.sides), Some(5));
        open(PolyKind::Deck);
        assert!(armed().is_none() && is_open());
        assert_eq!(WINDOW.with(|w| w.borrow().map(|a| a.sides)), Some(5));
        open(PolyKind::Room);
        assert_eq!(WINDOW.with(|w| w.borrow().map(|a| a.sides)), Some(6));
        disarm();
    }
}
