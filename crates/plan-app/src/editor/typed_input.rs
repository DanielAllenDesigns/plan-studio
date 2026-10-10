//! Typed length and angle while a tool is drawing or dragging (W-15, W-16).
//!
//! A tool that has a start point (the wall tool after its first click, the
//! Select tool while a wall end is dragged) arms the input. From then on the
//! shell hands it typed characters and the digit hotkeys step aside: digits
//! fill the length (feet-inches syntax, [`parse_ft_in`]), Tab switches to the
//! angle field and back, Backspace edits, Enter commits and Esc drops what was
//! typed. Tools read the values with [`TypedInput::length`] and
//! [`TypedInput::angle`]; temporary-dimension drags use
//! `tempdim::typed_value`. A length also takes arithmetic (`10' + 6"`,
//! `92 5/8 - 3"`; see `plan_core::calc`), and the Enter Coordinates dialog
//! hands its answer over as a length and angle ([`TypedInput::set_polar`]).

use eframe::egui::{Key, Modifiers};
use plan_core::units::parse_ft_in;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TypedField {
    #[default]
    Length,
    Angle,
}

/// What a key did to the input.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TypedKey {
    /// Not for the input; the tool handles the key as usual.
    Ignored,
    /// The text or the active field changed.
    Edited,
    /// Enter with something typed: the tool applies the values.
    Commit,
    /// Esc with something typed: the text was dropped.
    Cancelled,
}

#[derive(Clone, Debug, Default)]
pub struct TypedInput {
    armed: bool,
    field: TypedField,
    length: String,
    angle: String,
}

impl TypedInput {
    /// A tool has a start point: typed characters are wanted.
    pub fn arm(&mut self) {
        self.armed = true;
    }

    /// Like [`arm`](Self::arm), but the digits go to the angle first: a drag
    /// that turns something (Rotate) asks for degrees, not a length (S-28).
    pub fn arm_angle(&mut self) {
        self.armed = true;
        self.field = TypedField::Angle;
    }

    /// The drawing or drag ended: forget everything.
    pub fn disarm(&mut self) {
        *self = Self::default();
    }

    pub fn is_armed(&self) -> bool {
        self.armed
    }

    /// Drops the typed text but stays armed (the next wall of a chain).
    pub fn clear(&mut self) {
        self.field = TypedField::Length;
        self.length.clear();
        self.angle.clear();
    }

    pub fn has_text(&self) -> bool {
        !self.length.is_empty() || !self.angle.is_empty()
    }

    pub fn field(&self) -> TypedField {
        self.field
    }

    pub fn length_text(&self) -> &str {
        &self.length
    }

    pub fn angle_text(&self) -> &str {
        &self.angle
    }

    /// The typed length in inches (feet-inches syntax; `12-6` is 12'6").
    pub fn length(&self) -> Option<f64> {
        let t = self.length.trim();
        let t = if !t.contains('\'') && !t.starts_with('-') {
            match t.split_once('-') {
                Some((ft, rest)) if !ft.is_empty() && !rest.is_empty() => format!("{ft}'{rest}"),
                _ => t.to_string(),
            }
        } else {
            t.to_string()
        };
        let v = if plan_core::calc::has_operator(&t) {
            plan_core::units::parse_length(&t, plan_core::units::LengthUnit::Inches)
        } else {
            parse_ft_in(&t)
        };
        v.filter(|v| *v > 0.0)
    }

    /// Fills both fields with a length (inches) and an angle (degrees) and
    /// puts the length field in front, as if they had been typed: Enter then
    /// commits them. The Enter Coordinates dialog answers this way.
    pub fn set_polar(&mut self, length: f64, angle_deg: f64) {
        self.armed = true;
        self.field = TypedField::Length;
        self.length = format!("{length:.6}\"");
        self.angle = format!("{angle_deg:.6}");
    }

    /// The typed angle in degrees, counter-clockwise from east.
    pub fn angle(&self) -> Option<f64> {
        plan_core::calc::eval_number(self.angle.trim().trim_end_matches('\u{b0}'))
    }

    /// Is `key` a typed character the armed input takes instead of a hotkey?
    pub fn swallows(&self, key: Key, m: &Modifiers) -> bool {
        use Key::*;
        self.armed
            && !m.command
            && !m.ctrl
            && !m.alt
            && matches!(
                key,
                Num0 | Num1
                    | Num2
                    | Num3
                    | Num4
                    | Num5
                    | Num6
                    | Num7
                    | Num8
                    | Num9
                    | Minus
                    | Period
                    | Slash
                    | Quote
            )
    }

    fn active(&mut self) -> &mut String {
        match self.field {
            TypedField::Length => &mut self.length,
            TypedField::Angle => &mut self.angle,
        }
    }

    /// Feeds one key or typed text. Does nothing unless armed.
    pub fn handle(&mut self, key: Option<Key>, text: Option<&str>) -> TypedKey {
        if !self.armed {
            return TypedKey::Ignored;
        }
        if let Some(t) = text {
            let field = self.field;
            let ok: String = t
                .chars()
                .filter(|c| match field {
                    TypedField::Length => c.is_ascii_digit() || " '\"-/.+*".contains(*c),
                    TypedField::Angle => c.is_ascii_digit() || ".-+*/ ".contains(*c),
                })
                .collect();
            if ok.is_empty() {
                return TypedKey::Ignored;
            }
            self.active().push_str(&ok);
            return TypedKey::Edited;
        }
        match key {
            Some(Key::Backspace) if !self.active().is_empty() => {
                self.active().pop();
                TypedKey::Edited
            }
            Some(Key::Tab) => {
                self.field = match self.field {
                    TypedField::Length => TypedField::Angle,
                    TypedField::Angle => TypedField::Length,
                };
                TypedKey::Edited
            }
            Some(Key::Enter) if self.has_text() => TypedKey::Commit,
            Some(Key::Escape) if self.has_text() => {
                self.clear();
                TypedKey::Cancelled
            }
            _ => TypedKey::Ignored,
        }
    }
}

/// `start` plus `len` along `deg` degrees counter-clockwise from east, with
/// the rounding noise of the trigonometry removed (90 degrees is exactly up).
pub fn polar(start: plan_core::geometry::Point, len: f64, deg: f64) -> plan_core::geometry::Point {
    let r = deg.to_radians();
    let clean = |v: f64| (v * 1e12).round() / 1e12;
    plan_core::geometry::Point::new(
        start.x + clean(r.cos()) * len,
        start.y + clean(r.sin()) * len,
    )
}

/// The angle of `to` seen from `from`, degrees in `[0, 360)`.
pub fn angle_deg(from: plan_core::geometry::Point, to: plan_core::geometry::Point) -> f64 {
    let a = to.sub(from).angle().to_degrees().rem_euclid(360.0);
    if a >= 360.0 - 1e-9 {
        0.0
    } else {
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::geometry::Point;

    fn typed(t: &mut TypedInput, s: &str) {
        assert_eq!(t.handle(None, Some(s)), TypedKey::Edited);
    }

    #[test]
    fn unarmed_input_takes_nothing() {
        let mut t = TypedInput::default();
        assert_eq!(t.handle(None, Some("1")), TypedKey::Ignored);
        assert!(!t.swallows(Key::Num1, &Modifiers::NONE));
        t.arm();
        assert!(t.swallows(Key::Num1, &Modifiers::NONE));
        assert!(!t.swallows(Key::Num1, &Modifiers::COMMAND));
        assert!(!t.swallows(Key::A, &Modifiers::NONE));
    }

    #[test]
    fn length_and_angle_fields_parse_feet_inches() {
        let mut t = TypedInput::default();
        t.arm();
        typed(&mut t, "12'");
        typed(&mut t, "6");
        assert_eq!(t.length(), Some(150.0));
        assert_eq!(t.handle(Some(Key::Tab), None), TypedKey::Edited);
        assert_eq!(t.field(), TypedField::Angle);
        // Letters never enter a field; the angle takes digits, dot and minus.
        assert_eq!(t.handle(None, Some("x'")), TypedKey::Ignored);
        typed(&mut t, "90");
        assert_eq!(t.angle(), Some(90.0));
        assert_eq!(t.handle(Some(Key::Backspace), None), TypedKey::Edited);
        assert_eq!(t.angle(), Some(9.0));
        assert_eq!(t.handle(Some(Key::Enter), None), TypedKey::Commit);
        assert_eq!(t.handle(Some(Key::Escape), None), TypedKey::Cancelled);
        assert!(!t.has_text() && t.is_armed());
        assert_eq!(t.handle(Some(Key::Enter), None), TypedKey::Ignored);
    }

    #[test]
    fn dash_separates_feet_and_inches() {
        let mut t = TypedInput::default();
        t.arm();
        typed(&mut t, "12-6");
        assert_eq!(t.length(), Some(150.0));
        t.clear();
        typed(&mut t, "30");
        assert_eq!(t.length(), Some(30.0));
        t.clear();
        assert_eq!(t.length(), None);
    }

    #[test]
    fn polar_is_exact_on_the_axes() {
        let o = Point::new(10.0, 20.0);
        assert_eq!(polar(o, 144.0, 90.0), Point::new(10.0, 164.0));
        assert_eq!(polar(o, 144.0, 180.0), Point::new(-134.0, 20.0));
        assert_eq!(polar(o, 144.0, 270.0), Point::new(10.0, -124.0));
        assert_eq!(polar(o, 144.0, 0.0), Point::new(154.0, 20.0));
        assert_eq!(angle_deg(o, Point::new(10.0, 164.0)), 90.0);
        assert_eq!(angle_deg(o, Point::new(154.0, 20.0)), 0.0);
        assert_eq!(angle_deg(o, Point::new(10.0, -124.0)), 270.0);
    }

    #[test]
    fn a_rotate_drag_arms_the_angle_field_first() {
        let mut t = TypedInput::default();
        t.arm_angle();
        assert!(t.is_armed());
        assert_eq!(t.field(), TypedField::Angle);
        typed(&mut t, "45");
        assert_eq!(t.angle(), Some(45.0));
        assert_eq!(t.length(), None);
        // Tab hops to the length, Enter commits.
        assert_eq!(t.handle(Some(Key::Tab), None), TypedKey::Edited);
        assert_eq!(t.field(), TypedField::Length);
        assert_eq!(t.handle(Some(Key::Enter), None), TypedKey::Commit);
    }

    #[test]
    fn disarm_forgets_everything() {
        let mut t = TypedInput::default();
        t.arm();
        typed(&mut t, "5");
        t.disarm();
        assert!(!t.is_armed() && !t.has_text());
    }
}
