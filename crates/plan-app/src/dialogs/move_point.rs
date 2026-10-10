//! Move Point (CAD-112; Round 16 brief 06): double-click a temporary point to
//! move it by number.
//!
//! The dialog shows where the point is. **Absolute** gives its new X and Y,
//! **Relative** moves it by X and Y steps and **Polar** by a distance and an
//! angle (a degree value, bearing or azimuth). The moved point becomes the
//! Current Point. One undo step (`survey::move_point`).

use super::input_line::{angle, field, length};
use crate::editor::EditorContext;
use crate::tools::cad::survey::{self, PointSpec};
use eframe::egui;
use plan_core::geometry::Point;
use std::cell::RefCell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Absolute,
    Relative,
    Polar,
}

struct State {
    from: Point,
    kind: Kind,
    a: String,
    b: String,
    error: Option<String>,
}

impl State {
    fn spec(&self) -> Result<PointSpec, String> {
        Ok(match self.kind {
            Kind::Absolute => PointSpec::Absolute {
                x: length(&self.a, "X")?,
                y: length(&self.b, "Y")?,
            },
            Kind::Relative => PointSpec::Relative {
                dx: length(&self.a, "X")?,
                dy: length(&self.b, "Y")?,
            },
            Kind::Polar => PointSpec::Polar {
                dist: length(&self.a, "Distance")?,
                angle: angle(&self.b, "Angle")?,
            },
        })
    }

    /// Re-fills the boxes so they stand for the same spot in another style.
    fn switch(&mut self, to: Kind) {
        let st = survey::number_style();
        match to {
            Kind::Absolute => {
                self.a = st.format_length(self.from.x);
                self.b = st.format_length(self.from.y);
            }
            Kind::Relative => {
                self.a = "0".into();
                self.b = "0".into();
            }
            Kind::Polar => {
                self.a = "0".into();
                self.b = String::new();
            }
        }
        self.kind = to;
    }
}

thread_local! {
    static OPEN: RefCell<Option<State>> = const { RefCell::new(None) };
}

pub fn is_open() -> bool {
    OPEN.with(|o| o.borrow().is_some())
}

/// Opens Move Point for the temporary point at `from`.
pub fn open(from: Point) {
    let mut st = State {
        from,
        kind: Kind::Relative,
        a: String::new(),
        b: String::new(),
        error: None,
    };
    st.switch(Kind::Absolute);
    OPEN.with(|o| *o.borrow_mut() = Some(st));
}

/// Double-click at `at` (plan inches): opens Move Point when a temporary
/// point is there. Returns whether it did.
pub fn open_at(cx: &EditorContext, at: Point, tol: f64) -> bool {
    match survey::temporary_point_at(cx, at, tol) {
        Some(p) => {
            open(p);
            true
        }
        None => false,
    }
}

#[cfg(test)]
pub fn close() {
    OPEN.with(|o| *o.borrow_mut() = None);
}

/// Draws the window while it is open.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut st) = OPEN.with(|o| o.borrow_mut().take()) else {
        return;
    };
    let (mut ok, mut cancel) = (false, false);
    let mut want = st.kind;
    egui::Window::new("Move Point")
        .id(egui::Id::new("move_point"))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            let f = survey::number_style();
            ui.label(format!(
                "Point at X {}   Y {}",
                f.format_length(st.from.x),
                f.format_length(st.from.y)
            ));
            ui.horizontal(|ui| {
                ui.radio_value(&mut want, Kind::Absolute, "Absolute");
                ui.radio_value(&mut want, Kind::Relative, "Relative");
                ui.radio_value(&mut want, Kind::Polar, "Polar");
            });
            if st.kind == Kind::Polar {
                field(ui, "Distance", &mut st.a);
                field(ui, "Angle", &mut st.b);
            } else {
                field(ui, "X", &mut st.a);
                field(ui, "Y", &mut st.b);
            }
            if let Some(e) = &st.error {
                ui.colored_label(egui::Color32::LIGHT_RED, e);
            }
            ui.horizontal(|ui| {
                ok = ui.button("   OK   ").clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
    if want != st.kind {
        st.switch(want);
    }
    let (enter, esc) = ctx.input(|i| {
        (
            i.key_pressed(egui::Key::Enter),
            i.key_pressed(egui::Key::Escape),
        )
    });
    if ok || enter {
        match st
            .spec()
            .and_then(|spec| survey::move_point(cx, st.from, spec))
        {
            Ok(to) => {
                let f = survey::number_style();
                cx.status = format!(
                    "Point moved to X {} Y {}",
                    f.format_length(to.x),
                    f.format_length(to.y)
                );
            }
            Err(e) => {
                st.error = Some(e);
                OPEN.with(|o| *o.borrow_mut() = Some(st));
            }
        }
    } else if !(cancel || esc) {
        OPEN.with(|o| *o.borrow_mut() = Some(st));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_boxes_read_in_each_style() {
        let mut st = State {
            from: Point::new(100.0, 50.0),
            kind: Kind::Relative,
            a: "12".into(),
            b: "-6".into(),
            error: None,
        };
        assert_eq!(
            st.spec().unwrap(),
            PointSpec::Relative { dx: 12.0, dy: -6.0 }
        );
        st.switch(Kind::Polar);
        assert!(st.spec().is_err(), "an angle is needed");
        st.b = "90".into();
        st.a = "10".into();
        let PointSpec::Polar { dist, angle } = st.spec().unwrap() else {
            panic!()
        };
        assert!((dist - 10.0).abs() < 1e-9 && (angle - 90.0).abs() < 1e-9);
        st.switch(Kind::Absolute);
        let p = survey::resolve_move(st.from, st.spec().unwrap());
        assert!(p.dist(st.from) < 0.01, "{p:?}");
    }

    #[test]
    fn opening_marks_the_dialog_open() {
        close();
        open(Point::ZERO);
        assert!(is_open());
        close();
        assert!(!is_open());
    }
}
