//! Number Style and Angle Style (PR-30, PR-31, DS-47; Round 16 brief 06).
//!
//! One dialog, reached from Preferences (Unit Conversions page) and from the
//! General Plan Defaults page of Default Settings, sets how the plan shows
//! and reads numbers:
//!
//! * **Length**: feet and inches (with fractions to a chosen denominator),
//!   decimal feet, inches, or metric, with the decimals, the unit marks and
//!   trailing zeroes;
//! * **Angle**: degrees, degrees/minutes/seconds, quadrant bearings
//!   (`N 61 25 10 E`) or azimuths, with the decimals of a degree or second;
//! * **Pitch**: rise over 12 or degrees;
//! * **Custom units**: a name and a multiplier on the base unit (inches,
//!   square inches or cubic inches) for lengths, areas and volumes.
//!
//! The style is kept in the plan (`Project::number_style`, one undo step) and
//! is in force through `tools::cad::survey`. Nothing is changed until OK.

use crate::editor::EditorContext;
use crate::tools::cad::survey;
use eframe::egui;
use plan_core::bearing::{AngleStyle, CustomUnit, NumberStyle, PitchStyle, Quantity};
use plan_core::units::LengthUnit;
use std::cell::RefCell;

struct State {
    style: NumberStyle,
    new_name: String,
    new_quantity: Quantity,
    new_multiplier: String,
    error: Option<String>,
}

thread_local! {
    static OPEN: RefCell<Option<State>> = const { RefCell::new(None) };
}

pub fn is_open() -> bool {
    OPEN.with(|o| o.borrow().is_some())
}

/// Opens the dialog with the style now in force.
pub fn open() {
    open_with(survey::number_style());
}

fn open_with(style: NumberStyle) {
    OPEN.with(|o| {
        *o.borrow_mut() = Some(State {
            style,
            new_name: String::new(),
            new_quantity: Quantity::Length,
            new_multiplier: "1".into(),
            error: None,
        });
    });
}

/// Closes the dialog without an answer.
#[cfg(test)]
pub fn close() {
    OPEN.with(|o| *o.borrow_mut() = None);
}

pub const UNITS: [LengthUnit; 6] = [
    LengthUnit::FeetInches,
    LengthUnit::Inches,
    LengthUnit::DecimalFeet,
    LengthUnit::Millimeters,
    LengthUnit::Centimeters,
    LengthUnit::Meters,
];

pub fn unit_label(u: LengthUnit) -> &'static str {
    match u {
        LengthUnit::FeetInches => "Feet and Inches",
        LengthUnit::Inches => "Inches",
        LengthUnit::DecimalFeet => "Decimal Feet",
        LengthUnit::Millimeters => "Millimeters",
        LengthUnit::Centimeters => "Centimeters",
        LengthUnit::Meters => "Meters",
    }
}

pub fn quantity_label(q: Quantity) -> &'static str {
    match q {
        Quantity::Length => "Length (in)",
        Quantity::Area => "Area (sq in)",
        Quantity::Volume => "Volume (cu in)",
    }
}

pub fn pitch_label(p: PitchStyle) -> &'static str {
    match p {
        PitchStyle::RiseOver12 => "Rise in 12",
        PitchStyle::Degrees => "Degrees",
    }
}

/// A multiplier typed as a number, a fraction (`1/36`) or arithmetic.
fn read_multiplier(text: &str) -> Option<f64> {
    if let Some((n, d)) = text.split_once('/') {
        let (n, d) = (n.trim().parse::<f64>().ok()?, d.trim().parse::<f64>().ok()?);
        return (d != 0.0).then(|| n / d);
    }
    plan_core::calc::eval_number(text)
}

/// Adds a custom unit typed in the dialog: a name that is not used yet and a
/// multiplier above zero (`value in unit = base value * multiplier`).
pub fn add_custom_unit(
    style: &mut NumberStyle,
    name: &str,
    quantity: Quantity,
    multiplier: &str,
) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Give the unit a name".into());
    }
    if style
        .custom_units
        .iter()
        .any(|u| u.name.eq_ignore_ascii_case(name) && u.quantity == quantity)
    {
        return Err(format!("There is already a unit called \"{name}\""));
    }
    let m = read_multiplier(multiplier)
        .filter(|m| *m > 0.0 && m.is_finite())
        .ok_or("The multiplier needs a number above zero")?;
    style.custom_units.push(CustomUnit {
        name: name.to_string(),
        quantity,
        multiplier: m,
    });
    Ok(())
}

/// A few sample values in the style, for the preview under the choices.
pub fn preview(style: &NumberStyle) -> Vec<(&'static str, String)> {
    let mut rows = vec![
        ("Length", style.format_length(12.0 * 25.0 + 6.5)),
        ("Angle", style.format_angle(28.5)),
        ("Bearing", style.format_angle(156.75)),
        ("Pitch", style.format_pitch(26.565_051_177_078)),
    ];
    for u in &style.custom_units {
        rows.push(("Custom", u.format(1000.0, 3)));
    }
    rows
}

fn combo<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut T,
    all: &[T],
    label: impl Fn(T) -> &'static str,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(label(*value))
        .show_ui(ui, |ui| {
            for v in all {
                ui.selectable_value(value, *v, label(*v));
            }
        });
}

/// Draws the window while it is open; OK stores the style in the plan.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut st) = OPEN.with(|o| o.borrow_mut().take()) else {
        return;
    };
    let mut ok = false;
    let mut cancel = false;
    egui::Window::new("Number Style")
        .id(egui::Id::new("number_style"))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.strong("Length");
            egui::Grid::new("number_style_length")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Format");
                    combo(ui, "ns_unit", &mut st.style.length.unit, &UNITS, unit_label);
                    ui.end_row();
                    if matches!(
                        st.style.length.unit,
                        LengthUnit::FeetInches | LengthUnit::Inches
                    ) {
                        ui.label("Fractions to");
                        egui::ComboBox::from_id_salt("ns_frac")
                            .selected_text(format!("1/{}", st.style.length.fraction_denominator))
                            .show_ui(ui, |ui| {
                                for d in [2, 4, 8, 16, 32, 64] {
                                    ui.selectable_value(
                                        &mut st.style.length.fraction_denominator,
                                        d,
                                        format!("1/{d}"),
                                    );
                                }
                            });
                        ui.end_row();
                    } else {
                        ui.label("Decimals");
                        ui.add(egui::DragValue::new(&mut st.style.length.decimals).range(0..=6));
                        ui.end_row();
                    }
                });
            ui.checkbox(&mut st.style.length.unit_indicators, "Show unit marks");
            ui.checkbox(&mut st.style.length.trailing_zeroes, "Keep trailing zeroes");
            ui.separator();
            ui.strong("Angle Style");
            egui::Grid::new("number_style_angle")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Style");
                    combo(
                        ui,
                        "ns_angle",
                        &mut st.style.angle.style,
                        &AngleStyle::ALL,
                        AngleStyle::name,
                    );
                    ui.end_row();
                    ui.label(if st.style.angle.style == AngleStyle::Degrees {
                        "Decimals of a degree"
                    } else {
                        "Decimals of a second"
                    });
                    ui.add(egui::DragValue::new(&mut st.style.angle.decimals).range(0..=6));
                    ui.end_row();
                    ui.label("Roof pitch");
                    combo(
                        ui,
                        "ns_pitch",
                        &mut st.style.pitch,
                        &[PitchStyle::RiseOver12, PitchStyle::Degrees],
                        pitch_label,
                    );
                    ui.end_row();
                });
            ui.separator();
            ui.strong("Custom Units");
            let mut remove = None;
            for (i, u) in st.style.custom_units.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{}  ({}, x {})",
                        u.name,
                        quantity_label(u.quantity),
                        u.multiplier
                    ));
                    if ui.small_button("Remove").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                st.style.custom_units.remove(i);
            }
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut st.new_name)
                        .hint_text("Name")
                        .desired_width(70.0),
                );
                combo(
                    ui,
                    "ns_qty",
                    &mut st.new_quantity,
                    &[Quantity::Length, Quantity::Area, Quantity::Volume],
                    quantity_label,
                );
                ui.label("x");
                ui.add(egui::TextEdit::singleline(&mut st.new_multiplier).desired_width(60.0));
                if ui.button("Add").clicked() {
                    let (name, q, m) = (
                        st.new_name.clone(),
                        st.new_quantity,
                        st.new_multiplier.clone(),
                    );
                    match add_custom_unit(&mut st.style, &name, q, &m) {
                        Ok(()) => {
                            st.new_name.clear();
                            st.error = None;
                        }
                        Err(e) => st.error = Some(e),
                    }
                }
            });
            if let Some(e) = &st.error {
                ui.colored_label(egui::Color32::LIGHT_RED, e);
            }
            ui.separator();
            egui::Grid::new("number_style_preview")
                .num_columns(2)
                .show(ui, |ui| {
                    for (k, v) in preview(&st.style) {
                        ui.weak(k);
                        ui.monospace(v);
                        ui.end_row();
                    }
                });
            ui.horizontal(|ui| {
                ok = ui.button("   OK   ").clicked();
                cancel = ui.button("Cancel").clicked();
                if ui.button("Reset").clicked() {
                    st.style = NumberStyle::default();
                }
            });
        });
    let (enter, esc) = ctx.input(|i| {
        (
            i.key_pressed(egui::Key::Enter),
            i.key_pressed(egui::Key::Escape),
        )
    });
    if ok || enter {
        survey::store_number_style(cx, st.style);
        cx.status = "Number Style set".into();
    } else if cancel || esc {
        // dropped
    } else {
        OPEN.with(|o| *o.borrow_mut() = Some(st));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_units_need_a_name_and_a_multiplier() {
        let mut s = NumberStyle::default();
        add_custom_unit(&mut s, "yd", Quantity::Length, "1/36").unwrap();
        assert_eq!(s.custom_units.len(), 1);
        assert!((s.custom_units[0].from_base(72.0) - 2.0).abs() < 1e-9);
        assert!(add_custom_unit(&mut s, " ", Quantity::Length, "2").is_err());
        assert!(add_custom_unit(&mut s, "YD", Quantity::Length, "2").is_err());
        assert!(add_custom_unit(&mut s, "yd", Quantity::Area, "2").is_ok());
        assert!(add_custom_unit(&mut s, "z", Quantity::Length, "0").is_err());
        assert!(add_custom_unit(&mut s, "z", Quantity::Length, "x").is_err());
        assert_eq!(s.custom_units.len(), 2);
    }

    #[test]
    fn the_preview_follows_the_style() {
        let survey_style = NumberStyle::survey();
        let rows = preview(&survey_style);
        assert_eq!(rows[0].1, "25.54'");
        let plain = preview(&NumberStyle::default());
        assert!(plain[0].1.starts_with("25'"), "{}", plain[0].1);
        assert_ne!(rows[2].1, plain[2].1);
    }

    #[test]
    fn opening_seeds_from_the_style_in_force() {
        close();
        survey::set_number_style(NumberStyle::survey());
        assert!(!is_open());
        open();
        assert!(is_open());
        let seeded = OPEN.with(|o| o.borrow().as_ref().map(|s| s.style.clone()));
        assert_eq!(seeded, Some(NumberStyle::survey()));
        close();
        survey::set_number_style(NumberStyle::default());
    }
}
