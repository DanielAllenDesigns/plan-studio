//! Enter Coordinates (manual pp. 196-197): Tab or Enter while a wall, line or
//! other drag-drawn object is being drawn, or while a Move handle or a wall
//! end is dragged, asks for the new location by number.
//!
//! The dialog shows the start location and takes the end either as X and Y
//! (Absolute, on the plan's axes, or Relative to Start, as though the start
//! were 0,0) or, with Polar, as a Distance and an Angle from the start. The
//! choices are remembered between uses. Every number box takes arithmetic
//! (`plan_core::calc`).
//!
//! The dialog answers as a length and an angle: [`deliver`] puts them in the
//! typed input of the editor exactly as if they had been typed, so the tool
//! that asked commits them with the Enter key it already understands.

use crate::editor::typed_input::{angle_deg, polar};
use crate::editor::EditorContext;
use crate::tools::KeyEvent;
use eframe::egui;
use plan_core::calc::eval_number;
use plan_core::geometry::Point;
use plan_core::units::{parse_length, LengthUnit};
use std::cell::{Cell, RefCell};

/// How the end location is entered; kept between uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mode {
    /// Relative to Start (else Absolute).
    pub relative: bool,
    /// Distance and Angle (else X and Y).
    pub polar: bool,
}

impl Default for Mode {
    fn default() -> Self {
        Self {
            relative: true,
            polar: false,
        }
    }
}

/// What the dialog answered.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    /// The end location, as the distance from the start (inches) and the
    /// angle (degrees counter-clockwise from east).
    Ok {
        length: f64,
        angle_deg: f64,
    },
    Cancel,
}

struct State {
    start: Point,
    mode: Mode,
    unit: LengthUnit,
    x: String,
    y: String,
    dist: String,
    angle: String,
    /// Text for lengths, from the plan's dimension format.
    fmt: Box<dyn Fn(f64) -> String>,
}

thread_local! {
    static MODE: Cell<Mode> = const { Cell::new(Mode { relative: true, polar: false }) };
    static OPEN: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// The Absolute / Relative and Polar choices last used.
pub fn mode() -> Mode {
    MODE.with(Cell::get)
}

pub fn set_mode(m: Mode) {
    MODE.with(|c| c.set(m));
}

/// Closes the dialog without an answer.
#[cfg(test)]
pub fn close() {
    OPEN.with(|o| *o.borrow_mut() = None);
}

pub fn is_open() -> bool {
    OPEN.with(|o| o.borrow().is_some())
}

/// The distance and angle of `end` seen from `start`.
pub fn to_polar(start: Point, end: Point) -> (f64, f64) {
    (start.dist(end), angle_deg(start, end))
}

/// The end location the four boxes stand for under `mode`, or `None` while a
/// box in use does not read. Polar is always measured from the start.
pub fn resolve(start: Point, mode: Mode, boxes: [&str; 4], unit: LengthUnit) -> Option<Point> {
    let [x, y, dist, angle] = boxes;
    if mode.polar {
        let d = parse_length(dist, unit)?;
        let a = eval_number(angle)?;
        return Some(polar(start, d, a));
    }
    let p = Point::new(parse_length(x, unit)?, parse_length(y, unit)?);
    Some(if mode.relative { start + p } else { p })
}

impl State {
    /// The four boxes' text for the end location `end` under `mode`.
    fn texts(&self, mode: Mode, end: Point) -> [String; 4] {
        let f = &self.fmt;
        let rel = end - self.start;
        let (d, a) = to_polar(self.start, end);
        let shown = if mode.relative { rel } else { end };
        [
            f(shown.x),
            f(shown.y),
            f(d),
            format!("{a:.2}")
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string(),
        ]
    }

    fn set_texts(&mut self, t: [String; 4]) {
        [self.x, self.y, self.dist, self.angle] = t;
    }

    fn resolved(&self) -> Option<Point> {
        resolve(
            self.start,
            self.mode,
            [&self.x, &self.y, &self.dist, &self.angle],
            self.unit,
        )
    }

    /// Changes the entry style, keeping the end location the boxes stand for.
    fn switch(&mut self, mode: Mode) {
        if let Some(end) = self.resolved() {
            let t = self.texts(mode, end);
            self.set_texts(t);
        }
        self.mode = mode;
    }
}

/// Opens the dialog for a drawing or drag that began at `start`, with the
/// pointer now at `current`.
pub fn open(cx: &EditorContext, start: Point, current: Point) {
    let fmt_cx = cx.dim_format();
    let unit = fmt_cx.length.map_or(LengthUnit::FeetInches, |l| l.unit);
    let mut st = State {
        start,
        mode: mode(),
        unit,
        x: String::new(),
        y: String::new(),
        dist: String::new(),
        angle: String::new(),
        fmt: Box::new(move |v| fmt_cx.fmt_len(v)),
    };
    let t = st.texts(st.mode, current);
    st.set_texts(t);
    OPEN.with(|o| *o.borrow_mut() = Some(st));
}

/// Should this key open the dialog? Tab or Enter, plain, while the active
/// tool has a start location and nothing is typed yet. Opens it when so.
pub fn try_open(cx: &EditorContext, origin: Option<Point>, k: &KeyEvent) -> bool {
    let plain = !(k.modifiers.command || k.modifiers.ctrl || k.modifiers.alt || k.modifiers.shift);
    let wanted = k.is(egui::Key::Tab) || k.is(egui::Key::Enter);
    if !wanted || !plain || is_open() || cx.typed_input.has_text() || !cx.typed_input.is_armed() {
        return false;
    }
    let Some(start) = origin else {
        return false;
    };
    let current = cx.cursor_world.unwrap_or(start);
    open(cx, start, current);
    true
}

/// Hands the answer to the editor's typed input; the caller then sends the
/// tool an Enter key.
pub fn deliver(cx: &mut EditorContext, length: f64, angle_deg: f64) {
    cx.typed_input.set_polar(length, angle_deg);
}

/// Draws the window while it is open. `Some` once it closes.
pub fn show(ctx: &egui::Context) -> Option<Outcome> {
    let mut taken = OPEN.with(|o| o.borrow_mut().take())?;
    let mut ok = false;
    let mut cancel = false;
    let mut want = taken.mode;
    egui::Window::new("Enter Coordinates")
        .id(egui::Id::new("enter_coordinates"))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            let f = &taken.fmt;
            ui.label(format!(
                "Start Location: X {}   Y {}",
                f(taken.start.x),
                f(taken.start.y)
            ));
            ui.separator();
            ui.horizontal(|ui| {
                ui.radio_value(&mut want.relative, false, "Absolute");
                ui.radio_value(&mut want.relative, true, "Relative to Start");
            });
            ui.checkbox(&mut want.polar, "Polar");
            ui.separator();
            let valid = taken.resolved().is_some();
            let field = |ui: &mut egui::Ui, label: &str, text: &mut String| {
                ui.horizontal(|ui| {
                    ui.label(label);
                    ui.add(egui::TextEdit::singleline(text).desired_width(120.0));
                });
            };
            if want.polar {
                field(ui, "Distance", &mut taken.dist);
                field(ui, "Angle", &mut taken.angle);
            } else {
                field(ui, "X Position", &mut taken.x);
                field(ui, "Y Position", &mut taken.y);
            }
            if !valid {
                ui.colored_label(egui::Color32::LIGHT_RED, "Not a number or length.");
            }
            ui.horizontal(|ui| {
                ok = ui.add_enabled(valid, egui::Button::new("OK")).clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
    if want != taken.mode {
        taken.switch(want);
    }
    let (enter, esc) = ctx.input(|i| {
        (
            i.key_pressed(egui::Key::Enter),
            i.key_pressed(egui::Key::Escape),
        )
    });
    let ok = ok || (enter && taken.resolved().is_some());
    if ok {
        let end = taken.resolved()?;
        set_mode(taken.mode);
        let (length, angle_deg) = to_polar(taken.start, end);
        return Some(Outcome::Ok { length, angle_deg });
    }
    if cancel || esc {
        return Some(Outcome::Cancel);
    }
    OPEN.with(|o| *o.borrow_mut() = Some(taken));
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const IN: LengthUnit = LengthUnit::FeetInches;

    #[test]
    fn relative_xy_is_taken_from_the_start() {
        let m = Mode {
            relative: true,
            polar: false,
        };
        let p = resolve(Point::new(100.0, 50.0), m, ["10'", "-6\"", "", ""], IN).unwrap();
        assert!(p.dist(Point::new(220.0, 44.0)) < 1e-9, "{p:?}");
    }

    #[test]
    fn absolute_xy_ignores_the_start() {
        let m = Mode {
            relative: false,
            polar: false,
        };
        let p = resolve(Point::new(100.0, 50.0), m, ["20'", "5'", "", ""], IN).unwrap();
        assert_eq!(p, Point::new(240.0, 60.0));
    }

    #[test]
    fn polar_is_a_distance_and_angle_from_the_start() {
        let m = Mode {
            relative: false,
            polar: true,
        };
        let p = resolve(Point::new(10.0, 10.0), m, ["", "", "12'", "90"], IN).unwrap();
        assert!(p.dist(Point::new(10.0, 154.0)) < 1e-9, "{p:?}");
        let (d, a) = to_polar(Point::new(10.0, 10.0), p);
        assert!((d - 144.0).abs() < 1e-9 && (a - 90.0).abs() < 1e-9);
    }

    #[test]
    fn boxes_take_arithmetic() {
        let m = Mode {
            relative: true,
            polar: true,
        };
        let p = resolve(Point::ZERO, m, ["", "", "10' + 6\"", "45 + 45"], IN).unwrap();
        assert!(p.dist(Point::new(0.0, 126.0)) < 1e-9, "{p:?}");
        assert!(resolve(Point::ZERO, m, ["", "", "10' +", "90"], IN).is_none());
    }

    #[test]
    fn switching_the_style_keeps_the_location() {
        let mut st = State {
            start: Point::new(100.0, 100.0),
            mode: Mode {
                relative: true,
                polar: false,
            },
            unit: IN,
            x: "5'".into(),
            y: "0".into(),
            dist: String::new(),
            angle: String::new(),
            fmt: Box::new(|v| format!("{v}\"")),
        };
        let before = st.resolved().unwrap();
        st.switch(Mode {
            relative: false,
            polar: false,
        });
        assert_eq!(st.resolved().unwrap(), before);
        st.switch(Mode {
            relative: true,
            polar: true,
        });
        assert!(st.resolved().unwrap().dist(before) < 1e-6);
        assert_eq!(st.angle, "0");
    }

    #[test]
    fn the_style_is_remembered() {
        let m = Mode {
            relative: false,
            polar: true,
        };
        set_mode(m);
        assert_eq!(mode(), m);
        set_mode(Mode::default());
    }
}
