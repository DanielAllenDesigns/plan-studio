//! New CAD Arc (CAD-8, CAD-112; Round 16 brief 06): draws one arc from
//! numbers, the way a surveyor's curve table gives it.
//!
//! **Start**: absolute, or relative to / polar from the Current Point (as in
//! New CAD Line). **Direction**: the way the arc leaves its start (an angle,
//! bearing or azimuth), or the direction the previous line or arc ended in,
//! turned by an angle. **Radius**, with the curve to the **Left** or
//! **Right** of that direction. **Length**: the arc angle, the arc length or
//! the chord length. The maths is `tools::cad::arcs::input_arc`.
//!
//! **OK** draws and closes, **Next** draws and starts the next arc at the end
//! of this one, heading the way it ended (a compound curve is typed piece by
//! piece). Each arc is one undo step.

use super::input_line::{angle, field, length, StartKind};
use crate::editor::EditorContext;
use crate::tools::cad::arcs::{Curve, Extent};
use crate::tools::cad::survey::{self, ArcSpec, DirSpec, StartSpec};
use eframe::egui;
use std::cell::RefCell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirKind {
    Angle,
    /// Continue the previous line or arc, turned by an angle.
    Previous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtentKind {
    ArcAngle,
    ArcLength,
    ChordLength,
}

impl ExtentKind {
    const ALL: [ExtentKind; 3] = [
        ExtentKind::ArcAngle,
        ExtentKind::ArcLength,
        ExtentKind::ChordLength,
    ];

    fn label(self) -> &'static str {
        match self {
            ExtentKind::ArcAngle => "Arc Angle",
            ExtentKind::ArcLength => "Arc Length",
            ExtentKind::ChordLength => "Chord Length",
        }
    }
}

/// The boxes of the dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct Form {
    pub start: StartKind,
    pub start_a: String,
    pub start_b: String,
    pub dir: DirKind,
    pub dir_text: String,
    /// The direction is the chord's (else the tangent at the start).
    pub chord_dir: bool,
    pub radius: String,
    pub curve: Curve,
    pub extent: ExtentKind,
    pub extent_text: String,
    pub error: Option<String>,
}

impl Form {
    pub fn new(has_current: bool) -> Self {
        Self {
            start: if has_current {
                StartKind::Relative
            } else {
                StartKind::Absolute
            },
            start_a: "0".into(),
            start_b: "0".into(),
            dir: DirKind::Angle,
            dir_text: String::new(),
            chord_dir: false,
            radius: String::new(),
            curve: Curve::Left,
            extent: ExtentKind::ArcAngle,
            extent_text: String::new(),
            error: None,
        }
    }

    /// The form for the arc after one just drawn: it starts where that one
    /// ended and heads on the way it ended.
    pub fn next(&mut self) {
        self.start = StartKind::Relative;
        self.start_a = "0".into();
        self.start_b = "0".into();
        self.dir = DirKind::Previous;
        self.dir_text = "0".into();
        self.extent_text.clear();
        self.error = None;
    }

    pub fn spec(&self) -> Result<ArcSpec, String> {
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
        let direction = match self.dir {
            DirKind::Angle => DirSpec::Angle(angle(&self.dir_text, "Direction")?),
            DirKind::Previous => DirSpec::Previous {
                turn: plan_core::calc::eval_number(&self.dir_text)
                    .ok_or("Turn: enter an angle in degrees")?,
            },
        };
        let radius = length(&self.radius, "Radius")?;
        let extent = match self.extent {
            ExtentKind::ArcAngle => Extent::Angle(
                plan_core::calc::eval_number(&self.extent_text)
                    .ok_or("Arc Angle: enter degrees")?,
            ),
            ExtentKind::ArcLength => Extent::ArcLength(length(&self.extent_text, "Arc Length")?),
            ExtentKind::ChordLength => {
                Extent::ChordLength(length(&self.extent_text, "Chord Length")?)
            }
        };
        Ok(ArcSpec {
            start,
            direction,
            radius,
            curve: self.curve,
            extent,
            chord: self.chord_dir,
        })
    }
}

thread_local! {
    static OPEN: RefCell<Option<Form>> = const { RefCell::new(None) };
}

pub fn is_open() -> bool {
    OPEN.with(|o| o.borrow().is_some())
}

/// Opens New CAD Arc.
pub fn open() {
    OPEN.with(|o| *o.borrow_mut() = Some(Form::new(survey::current_point().is_some())));
}

#[cfg(test)]
pub fn close() {
    OPEN.with(|o| *o.borrow_mut() = None);
}

/// Draws the arc the form describes; the form is left alone when it fails.
pub fn apply(cx: &mut EditorContext, form: &mut Form) -> bool {
    let done = form
        .spec()
        .and_then(|spec| survey::enter_arc(cx, spec).map_err(|e| e.to_string()));
    match done {
        Ok(arc) => {
            let st = survey::number_style();
            cx.status = format!(
                "Arc drawn: chord {}, {:.2} degrees",
                st.format_length(arc.start.dist(arc.end)),
                arc.sweep
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

/// Draws the window while it is open.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut f) = OPEN.with(|o| o.borrow_mut().take()) else {
        return;
    };
    let (mut ok, mut next, mut cancel) = (false, false, false);
    egui::Window::new("New CAD Arc")
        .id(egui::Id::new("input_arc"))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.strong("Start");
            ui.horizontal(|ui| {
                ui.radio_value(&mut f.start, StartKind::Absolute, "Absolute");
                ui.radio_value(&mut f.start, StartKind::Relative, "Relative to Current Point");
                ui.radio_value(&mut f.start, StartKind::Polar, "Polar");
            });
            if f.start == StartKind::Polar {
                field(ui, "Distance", &mut f.start_a);
                field(ui, "Angle", &mut f.start_b);
            } else {
                field(ui, "X", &mut f.start_a);
                field(ui, "Y", &mut f.start_b);
            }
            ui.separator();
            ui.strong("Direction");
            ui.horizontal(|ui| {
                ui.radio_value(&mut f.dir, DirKind::Angle, "Angle or Bearing");
                ui.radio_value(&mut f.dir, DirKind::Previous, "From Previous Line or Arc");
            });
            if f.dir == DirKind::Angle {
                ui.horizontal(|ui| {
                    ui.radio_value(&mut f.chord_dir, false, "Start Direction");
                    ui.radio_value(&mut f.chord_dir, true, "Chord Direction");
                });
            }
            field(
                ui,
                if f.dir == DirKind::Angle {
                    "Direction"
                } else {
                    "Turn Left (degrees)"
                },
                &mut f.dir_text,
            );
            ui.separator();
            field(ui, "Radius", &mut f.radius);
            ui.horizontal(|ui| {
                ui.label("Curve");
                ui.radio_value(&mut f.curve, Curve::Left, "Left");
                ui.radio_value(&mut f.curve, Curve::Right, "Right");
            });
            ui.horizontal(|ui| {
                for k in ExtentKind::ALL {
                    ui.radio_value(&mut f.extent, k, k.label());
                }
            });
            field(ui, f.extent.label(), &mut f.extent_text);
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

    #[test]
    fn the_form_reads_radius_curve_and_each_extent() {
        survey::set_number_style(NumberStyle::survey());
        let mut f = Form::new(false);
        f.dir_text = "N 90 E".into();
        f.radius = "100".into();
        f.extent_text = "90".into();
        let spec = f.spec().unwrap();
        let a = survey::resolve_arc(spec, None, None).unwrap();
        // Heading east, curving left: a quarter circle to (100, 100) ft.
        assert!(a.end.dist(Point::new(1200.0, 1200.0)) < 1e-6, "{:?}", a.end);
        f.curve = Curve::Right;
        let a = survey::resolve_arc(f.spec().unwrap(), None, None).unwrap();
        assert!(a.end.dist(Point::new(1200.0, -1200.0)) < 1e-6);
        // Read as the chord direction (east) the same quarter circle ends due east.
        f.chord_dir = true;
        let a = survey::resolve_arc(f.spec().unwrap(), None, None).unwrap();
        assert!(a.end.y.abs() < 1e-6 && a.end.x > 0.0, "{:?}", a.end);
        f.chord_dir = false;
        f.extent = ExtentKind::ChordLength;
        f.extent_text = "100".into();
        let a = survey::resolve_arc(f.spec().unwrap(), None, None).unwrap();
        assert!((a.sweep - 60.0).abs() < 1e-9);
        f.extent = ExtentKind::ArcLength;
        f.extent_text = "50".into();
        assert!(f.spec().is_ok());
        f.radius = "x".into();
        assert!(f.spec().unwrap_err().starts_with("Radius"));
        survey::set_number_style(NumberStyle::default());
    }

    #[test]
    fn next_continues_the_way_the_arc_ended() {
        let mut f = Form::new(false);
        f.next();
        assert_eq!(f.dir, DirKind::Previous);
        assert_eq!(f.start, StartKind::Relative);
        f.radius = "10'".into();
        f.extent_text = "30".into();
        // Needs a previous line or arc.
        assert!(survey::resolve_arc(f.spec().unwrap(), Some(Point::ZERO), None).is_err());
        let prev = Some((Point::new(-1.0, 0.0), Point::ZERO));
        let a = survey::resolve_arc(f.spec().unwrap(), Some(Point::ZERO), prev).unwrap();
        // Leaves heading east, so it starts at the origin and curves left.
        assert!(a.start.dist(Point::ZERO) < 1e-9);
        assert!(a.end.y > 0.0 && a.end.x > 0.0);
    }
}
