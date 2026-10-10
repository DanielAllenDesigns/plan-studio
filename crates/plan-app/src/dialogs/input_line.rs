//! New CAD Line (CAD-5, CAD-112, CAD-134; Round 16 brief 06): draws one line
//! from numbers.
//!
//! **Start**: absolute (X and Y on the plan's axes), relative to the Current
//! Point (X and Y steps) or polar from the Current Point (a distance and an
//! angle). **End**: absolute, relative to the start, polar from the start, or
//! relative to the previous line (a distance and the angle turned from the
//! previous line's direction). Lengths are read in the Number Style; every
//! angle box takes a degree value, a quadrant bearing (`N 61 25 10 E`) or an
//! azimuth (`Az 121.5`), whatever the Angle Style.
//!
//! **OK** draws the line and closes; **Next** draws it and starts the next
//! line at its end, so a traverse is typed course by course. Each line is one
//! undo step (`survey::enter_line`).

use crate::editor::EditorContext;
use crate::tools::cad::survey::{self, EndSpec, LineSpec, StartSpec};
use eframe::egui;
use std::cell::RefCell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartKind {
    Absolute,
    /// From the Current Point, by X and Y.
    Relative,
    /// From the Current Point, by distance and angle.
    Polar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndKind {
    Absolute,
    /// From the start, by X and Y.
    Relative,
    /// From the start, by distance and angle.
    Polar,
    /// From the start, by distance and a turn from the previous line.
    FromPrevious,
}

/// The boxes of the dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct Form {
    pub start: StartKind,
    pub end: EndKind,
    pub start_a: String,
    pub start_b: String,
    pub end_a: String,
    pub end_b: String,
    pub error: Option<String>,
}

impl Form {
    /// The form on opening: a line that starts at the Current Point when
    /// there is one, else at the origin.
    pub fn new(has_current: bool) -> Self {
        Self {
            start: if has_current {
                StartKind::Relative
            } else {
                StartKind::Absolute
            },
            end: EndKind::Polar,
            start_a: "0".into(),
            start_b: "0".into(),
            end_a: String::new(),
            end_b: String::new(),
            error: None,
        }
    }

    /// The form for the line after one just drawn: it starts at the end of
    /// that line.
    pub fn next(&mut self) {
        self.start = StartKind::Relative;
        self.start_a = "0".into();
        self.start_b = "0".into();
        if self.end == EndKind::Absolute {
            self.end = EndKind::Polar;
        }
        self.end_a.clear();
        self.end_b.clear();
        self.error = None;
    }

    /// What the boxes say, or why they do not read.
    pub fn spec(&self) -> Result<LineSpec, String> {
        let start = match self.start {
            StartKind::Absolute => StartSpec::Absolute {
                x: length(&self.start_a, "Start X")?,
                y: length(&self.start_b, "Start Y")?,
            },
            StartKind::Relative => StartSpec::Relative {
                dx: length(&self.start_a, "Start X")?,
                dy: length(&self.start_b, "Start Y")?,
            },
            StartKind::Polar => StartSpec::Polar {
                dist: length(&self.start_a, "Start distance")?,
                angle: angle(&self.start_b, "Start angle")?,
            },
        };
        let end = match self.end {
            EndKind::Absolute => EndSpec::Absolute {
                x: length(&self.end_a, "End X")?,
                y: length(&self.end_b, "End Y")?,
            },
            EndKind::Relative => EndSpec::Relative {
                dx: length(&self.end_a, "End X")?,
                dy: length(&self.end_b, "End Y")?,
            },
            EndKind::Polar => EndSpec::Polar {
                dist: length(&self.end_a, "Length")?,
                angle: angle(&self.end_b, "Angle")?,
            },
            EndKind::FromPrevious => EndSpec::FromPrevious {
                dist: length(&self.end_a, "Length")?,
                turn: plan_core::calc::eval_number(&self.end_b)
                    .ok_or("Turn: enter an angle in degrees")?,
            },
        };
        Ok(LineSpec { start, end })
    }
}

/// A length box, in the Number Style.
pub(crate) fn length(text: &str, what: &str) -> Result<f64, String> {
    survey::parse_length(text).ok_or_else(|| format!("{what}: enter a length"))
}

/// An angle box: degrees, a quadrant bearing or an azimuth.
pub(crate) fn angle(text: &str, what: &str) -> Result<f64, String> {
    survey::parse_angle_text(text).ok_or_else(|| format!("{what}: enter an angle or bearing"))
}

/// One labelled text box.
pub(crate) fn field(ui: &mut egui::Ui, label: &str, text: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::TextEdit::singleline(text).desired_width(120.0));
    });
}

thread_local! {
    static OPEN: RefCell<Option<Form>> = const { RefCell::new(None) };
}

pub fn is_open() -> bool {
    OPEN.with(|o| o.borrow().is_some())
}

/// Opens New CAD Line.
pub fn open() {
    OPEN.with(|o| *o.borrow_mut() = Some(Form::new(survey::current_point().is_some())));
}

#[cfg(test)]
pub fn close() {
    OPEN.with(|o| *o.borrow_mut() = None);
}

/// Draws the line the form describes; the form is left alone when it fails.
pub fn apply(cx: &mut EditorContext, form: &mut Form) -> bool {
    let done = form
        .spec()
        .and_then(|spec| survey::enter_line(cx, spec).map_err(|e| e.to_string()));
    match done {
        Ok((a, b)) => {
            cx.status = format!(
                "Line drawn: {} at {}",
                survey::number_style().format_length(a.dist(b)),
                survey::azimuth_text(a, b, &survey::number_style())
            );
            form.error = None;
            true
        }
        Err(e) => {
            form.error = Some(e);
            false
        }
    }
}

fn start_fields(ui: &mut egui::Ui, f: &mut Form) {
    ui.strong("Start");
    ui.horizontal(|ui| {
        ui.radio_value(&mut f.start, StartKind::Absolute, "Absolute");
        ui.radio_value(
            &mut f.start,
            StartKind::Relative,
            "Relative to Current Point",
        );
        ui.radio_value(&mut f.start, StartKind::Polar, "Polar");
    });
    match f.start {
        StartKind::Absolute | StartKind::Relative => {
            field(ui, "X", &mut f.start_a);
            field(ui, "Y", &mut f.start_b);
        }
        StartKind::Polar => {
            field(ui, "Distance", &mut f.start_a);
            field(ui, "Angle", &mut f.start_b);
        }
    }
}

fn end_fields(ui: &mut egui::Ui, f: &mut Form) {
    ui.strong("End");
    ui.horizontal(|ui| {
        ui.radio_value(&mut f.end, EndKind::Absolute, "Absolute");
        ui.radio_value(&mut f.end, EndKind::Relative, "Relative to Start");
        ui.radio_value(&mut f.end, EndKind::Polar, "Polar");
        ui.radio_value(
            &mut f.end,
            EndKind::FromPrevious,
            "Relative to Previous Line",
        );
    });
    match f.end {
        EndKind::Absolute | EndKind::Relative => {
            field(ui, "X", &mut f.end_a);
            field(ui, "Y", &mut f.end_b);
        }
        EndKind::Polar => {
            field(ui, "Length", &mut f.end_a);
            field(ui, "Angle", &mut f.end_b);
        }
        EndKind::FromPrevious => {
            field(ui, "Length", &mut f.end_a);
            field(ui, "Turn Left (degrees)", &mut f.end_b);
        }
    }
}

/// Draws the window while it is open.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut f) = OPEN.with(|o| o.borrow_mut().take()) else {
        return;
    };
    let (mut ok, mut next, mut cancel) = (false, false, false);
    egui::Window::new("New CAD Line")
        .id(egui::Id::new("input_line"))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            start_fields(ui, &mut f);
            ui.separator();
            end_fields(ui, &mut f);
            if let Some(e) = &f.error {
                ui.colored_label(egui::Color32::LIGHT_RED, e);
            }
            ui.horizontal(|ui| {
                ok = ui.button("   OK   ").clicked();
                next = ui.button("Next").clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
    let (enter, esc) = ctx.input(|i| {
        (
            i.key_pressed(egui::Key::Enter),
            i.key_pressed(egui::Key::Escape),
        )
    });
    if ok || enter {
        if !apply(cx, &mut f) {
            OPEN.with(|o| *o.borrow_mut() = Some(f));
        }
    } else if next {
        if apply(cx, &mut f) {
            f.next();
        }
        OPEN.with(|o| *o.borrow_mut() = Some(f));
    } else if !(cancel || esc) {
        OPEN.with(|o| *o.borrow_mut() = Some(f));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::bearing::NumberStyle;
    use plan_core::geometry::Point;

    fn survey_form() -> Form {
        survey::set_number_style(NumberStyle::survey());
        Form::new(false)
    }

    #[test]
    fn a_polar_end_reads_a_bearing_and_a_length() {
        let mut f = survey_form();
        f.end_a = "100".into();
        f.end_b = "N 90 E".into();
        let spec = f.spec().unwrap();
        let (a, b) = survey::resolve_line(spec, None, None).unwrap();
        assert_eq!(a, Point::ZERO);
        // 100 decimal feet due east.
        assert!(b.dist(Point::new(1200.0, 0.0)) < 1e-6, "{b:?}");
        survey::set_number_style(NumberStyle::default());
    }

    #[test]
    fn boxes_that_do_not_read_say_which() {
        let mut f = survey_form();
        f.end_a = "abc".into();
        f.end_b = "N 45 E".into();
        assert!(f.spec().unwrap_err().starts_with("Length"));
        f.end_a = "10".into();
        f.end_b = "Q 45 E".into();
        assert!(f.spec().unwrap_err().starts_with("Angle"));
        survey::set_number_style(NumberStyle::default());
    }

    #[test]
    fn next_restarts_at_the_end_of_the_line() {
        let mut f = Form::new(false);
        f.end = EndKind::Absolute;
        f.next();
        assert_eq!(f.start, StartKind::Relative);
        assert_eq!(f.end, EndKind::Polar);
        assert!(f.end_a.is_empty());
        assert!(Form::new(true).start == StartKind::Relative);
        assert!(Form::new(false).start == StartKind::Absolute);
    }

    #[test]
    fn relative_to_previous_takes_a_turn_with_arithmetic() {
        let mut f = Form::new(false);
        f.end = EndKind::FromPrevious;
        f.end_a = "5'".into();
        f.end_b = "45 + 45".into();
        let EndSpec::FromPrevious { turn, .. } = f.spec().unwrap().end else {
            panic!()
        };
        assert!((turn - 90.0).abs() < 1e-9);
    }
}
