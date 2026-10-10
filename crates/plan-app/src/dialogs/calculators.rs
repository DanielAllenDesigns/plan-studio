//! Tools > Calculators: the structural calculators in one window with a tab
//! for each: Header/Beam (IRC R602.7), Joist Span (R502.3), Rafter Span
//! (R802.4), Stair (R311.7.5) and Deck Beam/Joist (DCA 6).
//!
//! The arithmetic is `plan_check::tables`. Each tab takes inputs, shows the
//! result as text and a table, and has an "Apply to selected" button that
//! writes the result into the plan as one undo step:
//!
//! * Header/Beam: the header size (plies, depth, material, jack studs) of
//!   every selected door or window, each sized from its own width;
//! * Joist Span: the joist size and spacing of the floor framing defaults
//!   (Default Settings > Framing), which Build Framing then uses;
//! * Rafter Span: the rafter size and spacing of the roof framing defaults;
//! * Stair: the rise, riser count and tread depth of the selected stair;
//! * Deck: the joist and beam of the selected deck room's Deck Specification.
//!
//! `run_command` opens the window from a menu id and fills the inputs from
//! the selection (an opening's width, a stair's rise); `show_all` draws it
//! (the shell calls it from `docks::show_dialogs`).

use crate::editor::{framing_view, rooms_edit, stairs_view, EditorContext, ObjectRef};
use eframe::egui::{self, Align2, RichText, Vec2};
use plan_check::tables::{
    self as t, DeckInput, Grade, HeaderInput, HeaderKind, JoistInput, Material, RafterInput,
    Species, StairInput, Strength,
};
use plan_core::units::fmt_ft_in;
use plan_core::Id;
use plan_framing::Lumber;
use std::cell::RefCell;

/// Menu id: Tools > Calculators > Header/Beam...
pub const HEADER: &str = "calc.header";
/// Menu id: Tools > Calculators > Joist Span...
pub const JOIST: &str = "calc.joist";
/// Menu id: Tools > Calculators > Rafter Span...
pub const RAFTER: &str = "calc.rafter";
/// Menu id: Tools > Calculators > Stair...
pub const STAIR: &str = "calc.stair";
/// Menu id: Tools > Calculators > Deck Beam/Joist...
pub const DECK: &str = "calc.deck";

/// The tabs of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Header,
    Joist,
    Rafter,
    Stair,
    Deck,
}

impl Tab {
    pub const ALL: [Tab; 5] = [Tab::Header, Tab::Joist, Tab::Rafter, Tab::Stair, Tab::Deck];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Header => "Header/Beam",
            Tab::Joist => "Joist Span",
            Tab::Rafter => "Rafter Span",
            Tab::Stair => "Stair",
            Tab::Deck => "Deck Beam/Joist",
        }
    }

    fn from_id(id: &str) -> Option<Tab> {
        Some(match id {
            HEADER => Tab::Header,
            JOIST => Tab::Joist,
            RAFTER => Tab::Rafter,
            STAIR => Tab::Stair,
            DECK => Tab::Deck,
            _ => return None,
        })
    }
}

/// What the window holds between frames.
#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub open: bool,
    pub tab: Tab,
    /// Lumber of every tab.
    pub material: Material,
    pub header: HeaderInput,
    pub joist: JoistInput,
    /// The span the joists must make, inches.
    pub joist_span: f64,
    pub rafter: RafterInput,
    /// The run the rafters must make, inches of plan.
    pub rafter_run: f64,
    pub stair: StairInput,
    pub deck: DeckInput,
    /// The last result of an Apply button.
    pub message: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            open: false,
            tab: Tab::Header,
            material: Material::Lumber(Species::DouglasFirLarch, Grade::No2),
            header: HeaderInput::default(),
            joist: JoistInput::default(),
            joist_span: 144.0,
            rafter: RafterInput::default(),
            rafter_run: 144.0,
            stair: StairInput::default(),
            deck: DeckInput::default(),
            message: String::new(),
        }
    }
}

impl State {
    /// The inputs with the chosen material put in.
    fn header_input(&self) -> HeaderInput {
        HeaderInput {
            material: self.material,
            ..self.header
        }
    }

    fn joist_input(&self) -> JoistInput {
        JoistInput {
            material: self.material,
            ..self.joist
        }
    }

    fn rafter_input(&self) -> RafterInput {
        RafterInput {
            material: self.material,
            ..self.rafter
        }
    }

    fn deck_input(&self) -> DeckInput {
        DeckInput {
            material: self.material,
            ..self.deck
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// A copy of the window's state.
pub fn snapshot() -> State {
    state(|s| s.clone())
}

/// Changes the window's state (the inputs), for tests and presets.
#[cfg(test)]
pub fn edit(f: impl FnOnce(&mut State)) {
    state(f);
}

/// Is the window up?
pub fn is_open() -> bool {
    state(|s| s.open)
}

/// Closes the window.
#[cfg(test)]
pub fn close() {
    state(|s| s.open = false);
}

// ----- the selection -----

/// The selected doors and windows of the active floor.
fn selected_openings(cx: &EditorContext) -> Vec<Id> {
    cx.selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Opening(id) => Some(*id),
            _ => None,
        })
        .collect()
}

fn selected_stair(cx: &EditorContext) -> Option<Id> {
    cx.selection.items.iter().find_map(|o| match o {
        ObjectRef::Stair(id) => Some(*id),
        _ => None,
    })
}

/// Fills the inputs of `tab` from the selection.
fn prefill(cx: &EditorContext, st: &mut State) {
    match st.tab {
        Tab::Header => {
            if let Some(id) = selected_openings(cx).first() {
                if let Some(op) = cx.floor().openings.iter().find(|o| o.id == *id) {
                    st.header.span = op.width;
                }
            }
        }
        Tab::Stair => {
            if let Some(id) = selected_stair(cx) {
                if let Some(o) = stairs_view::find(cx.floor(), id) {
                    if !o.is_landing() {
                        st.stair.total_rise = o.stair.params.total_rise;
                        st.stair.tread = o.stair.params.tread_depth;
                    }
                }
            }
        }
        _ => {}
    }
}

/// Opens the window on the tab of menu command `id`; false when `id` is not
/// one of this module's.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    let Some(tab) = Tab::from_id(id) else {
        return false;
    };
    state(|st| {
        st.open = true;
        st.tab = tab;
        st.message.clear();
        // The stair limits follow the plan's Plan Check settings.
        st.stair.limits =
            t::StairLimits::from_options(&plan_check::CheckSettings::load(&cx.project).options);
        prefill(cx, st);
    });
    true
}

// ----- apply -----

/// Header/Beam: sizes the header of each selected opening from its own width
/// and writes it into the opening's Framing tab. One undo step.
pub fn apply_header(cx: &mut EditorContext, st: &State) -> Result<String, String> {
    let ids = selected_openings(cx);
    if ids.is_empty() {
        return Err("Select a door or window first".into());
    }
    let mut picks = Vec::new();
    for id in &ids {
        let Some(op) = cx.floor().openings.iter().find(|o| o.id == *id) else {
            continue;
        };
        let input = HeaderInput {
            span: op.width,
            ..st.header_input()
        };
        let Some(pick) = t::header_recommend(&input) else {
            return Err(format!(
                "No header in the table carries {}: size it as an engineered beam",
                fmt_ft_in(op.width)
            ));
        };
        picks.push((*id, pick));
    }
    cx.begin_change("Apply Header Size");
    for (id, pick) in &picks {
        if let Some(op) = cx.floor_mut().openings.iter_mut().find(|o| o.id == *id) {
            let f = &mut op.extras.spec.framing;
            f.include_header = true;
            f.header_plies = Some(pick.plies);
            f.header_depth = Some(pick.depth);
            f.header_material = match pick.kind {
                HeaderKind::Lumber => plan_core::openings::spec::HeaderMaterial::Lumber,
                HeaderKind::Lvl => plan_core::openings::spec::HeaderMaterial::Lvl,
            };
            f.trimmers = Some(pick.jacks);
        }
    }
    cx.mark_dirty();
    Ok(match picks.as_slice() {
        [(_, p)] => format!("Header {} with {} jack stud(s) each end", p.label, p.jacks),
        many => format!("Sized the headers of {} openings", many.len()),
    })
}

/// Joist Span: the smallest joist that makes the span at the chosen spacing
/// becomes the floor framing default. One undo step.
pub fn apply_joist(cx: &mut EditorContext, st: &State) -> Result<String, String> {
    let input = st.joist_input();
    let Some(n) = t::joist_size_for(&input, st.joist_span) else {
        return Err(format!(
            "No joist in the table spans {} at {}\" on centre",
            fmt_ft_in(st.joist_span),
            input.spacing
        ));
    };
    let mut settings = framing_view::settings(&cx.project);
    settings.walls.joist_size = Lumber::two_by(t::actual_depth(n));
    settings.walls.joist_spacing = input.spacing;
    framing_view::set_settings(cx, settings);
    Ok(format!(
        "Floor joists 2x{n} at {}\" on centre (Framing Defaults)",
        input.spacing
    ))
}

/// Rafter Span: the roof framing default rafter. One undo step.
pub fn apply_rafter(cx: &mut EditorContext, st: &State) -> Result<String, String> {
    let input = st.rafter_input();
    let Some(n) = t::rafter_size_for(&input, st.rafter_run) else {
        return Err(format!(
            "No rafter in the table runs {} at {}\" on centre",
            fmt_ft_in(st.rafter_run),
            input.spacing
        ));
    };
    let mut settings = framing_view::settings(&cx.project);
    settings.roof.rafter = Lumber::two_by(t::actual_depth(n));
    settings.roof.spacing = input.spacing;
    framing_view::set_settings(cx, settings);
    Ok(format!(
        "Rafters 2x{n} at {}\" on centre (Framing Defaults)",
        input.spacing
    ))
}

/// Stair: the selected stair takes the rise, the number of risers and the
/// tread depth. One undo step.
pub fn apply_stair(cx: &mut EditorContext, st: &State) -> Result<String, String> {
    let Some(id) = selected_stair(cx) else {
        return Err("Select a stair first".into());
    };
    let Some(r) = t::stair_layout(&st.stair) else {
        return Err("Type a rise above zero".into());
    };
    if stairs_view::find(cx.floor(), id).is_none_or(|o| o.is_landing()) {
        return Err("Select a stair, not a landing".into());
    }
    cx.begin_change("Apply Stair Layout");
    let floor = cx.floor;
    stairs_view::update(&mut cx.project, floor, id, |o| {
        stairs_view::set_total_rise(o, st.stair.total_rise);
        stairs_view::set_risers(o, r.risers);
        o.stair.params.tread_depth = r.tread_depth;
    });
    cx.mark_dirty();
    Ok(format!(
        "Stair: {} risers of {:.3}\", {} treads of {:.2}\"",
        r.risers, r.riser_height, r.treads, r.tread_depth
    ))
}

/// Deck: the joist and beam of the selected deck room's specification. One
/// undo step.
pub fn apply_deck(cx: &mut EditorContext, st: &State) -> Result<String, String> {
    let Some(idx) = rooms_edit::selected_room(cx) else {
        return Err("Select a deck room first".into());
    };
    let Some(room) = cx.rooms.get(idx).cloned() else {
        return Err("Select a deck room first".into());
    };
    let Some(entry) = rooms_edit::name_entry(cx, &room) else {
        return Err("Select a deck room first".into());
    };
    if !plan_core::deck::is_deck(entry) {
        return Err("The selected room is not a deck".into());
    }
    let anchor = entry.anchor;
    let d = st.deck_input();
    let result = t::deck_design(&d);
    cx.begin_change("Apply Deck Framing");
    let floor = cx.floor;
    if let Some(n) = cx.project.floors[floor]
        .room_names
        .iter_mut()
        .find(|n| n.anchor == anchor)
    {
        let spec = n.deck.get_or_insert_with(Default::default);
        spec.framing.joist_size = format!("2x{}", d.joist_nominal);
        spec.framing.joist_spacing = d.joist_spacing;
        spec.framing.beam_size = format!("2x{}", d.beam_nominal);
        spec.framing.beam_plies = d.beam_plies;
        spec.framing.post_spacing = result.beam.inches.max(24.0);
    }
    cx.mark_dirty();
    Ok(format!(
        "Deck joists 2x{} at {}\", beam {}-2x{}, posts {} apart",
        d.joist_nominal,
        d.joist_spacing,
        d.beam_plies,
        d.beam_nominal,
        fmt_ft_in(result.beam.inches)
    ))
}

/// The Apply button of the open tab; the status line text.
pub fn apply_current(cx: &mut EditorContext) -> String {
    let st = snapshot();
    let r = match st.tab {
        Tab::Header => apply_header(cx, &st),
        Tab::Joist => apply_joist(cx, &st),
        Tab::Rafter => apply_rafter(cx, &st),
        Tab::Stair => apply_stair(cx, &st),
        Tab::Deck => apply_deck(cx, &st),
    };
    let text = match r {
        Ok(s) => s,
        Err(e) => e,
    };
    state(|s| s.message = text.clone());
    text
}

/// Whether the Apply button of `tab` has something to apply to.
fn can_apply(cx: &EditorContext, tab: Tab) -> bool {
    match tab {
        Tab::Header => !selected_openings(cx).is_empty(),
        Tab::Joist | Tab::Rafter => true,
        Tab::Stair => selected_stair(cx).is_some(),
        Tab::Deck => rooms_edit::selected_room(cx).is_some(),
    }
}

fn apply_label(tab: Tab) -> &'static str {
    match tab {
        Tab::Header => "Apply to Selected Openings",
        Tab::Joist => "Apply to Floor Framing Defaults",
        Tab::Rafter => "Apply to Roof Framing Defaults",
        Tab::Stair => "Apply to Selected Stair",
        Tab::Deck => "Apply to Selected Deck",
    }
}

fn apply_hint(tab: Tab) -> &'static str {
    match tab {
        Tab::Header => "Select a door or window; each gets the header its own width needs",
        Tab::Joist => "Sets the joist size and spacing Build Framing uses for floors",
        Tab::Rafter => "Sets the rafter size and spacing Build Framing uses for roofs",
        Tab::Stair => "Select a stair; it takes the rise, the risers and the tread depth",
        Tab::Deck => "Select a deck room; its Deck Specification takes the joist and beam",
    }
}

// ----- drawing -----

/// A length in inches with its feet-and-inches reading.
fn length(ui: &mut egui::Ui, label: &str, v: &mut f64, speed: f64) {
    ui.label(label);
    ui.horizontal(|ui| {
        ui.add(
            egui::DragValue::new(v)
                .speed(speed)
                .range(0.0..=4000.0)
                .suffix("\""),
        );
        ui.weak(fmt_ft_in(*v));
    });
    ui.end_row();
}

fn choose(ui: &mut egui::Ui, id: &str, label: &str, v: &mut f64, options: &[f64], unit: &str) {
    ui.label(label);
    egui::ComboBox::from_id_salt(id)
        .selected_text(format!("{v}{unit}"))
        .show_ui(ui, |ui| {
            for o in options {
                ui.selectable_value(v, *o, format!("{o}{unit}"));
            }
        });
    ui.end_row();
}

fn material_row(ui: &mut egui::Ui, m: &mut Material) {
    let mut species = match m {
        Material::Lumber(s, _) => Some(*s),
        Material::Custom(_) => None,
    };
    let mut grade = match m {
        Material::Lumber(_, g) => *g,
        Material::Custom(_) => Grade::No2,
    };
    ui.label("Species");
    egui::ComboBox::from_id_salt("calc_species")
        .selected_text(species.map_or("Custom values", Species::name))
        .show_ui(ui, |ui| {
            for s in Species::ALL {
                ui.selectable_value(&mut species, Some(s), s.name());
            }
            ui.selectable_value(&mut species, None, "Custom values");
        });
    ui.end_row();
    if species.is_some() {
        ui.label("Grade");
        egui::ComboBox::from_id_salt("calc_grade")
            .selected_text(grade.name())
            .show_ui(ui, |ui| {
                for g in Grade::ALL {
                    ui.selectable_value(&mut grade, g, g.name());
                }
            });
        ui.end_row();
    }
    *m = match (species, *m) {
        (Some(s), _) => Material::Lumber(s, grade),
        (None, Material::Custom(c)) => Material::Custom(c),
        (None, _) => Material::Custom(Strength {
            fb: 1000.0,
            e: 1.6e6,
            fv: 175.0,
            fc_perp: 565.0,
            size_factor: false,
        }),
    };
    if let Material::Custom(c) = m {
        ui.label("Fb (psi)");
        ui.add(
            egui::DragValue::new(&mut c.fb)
                .speed(10.0)
                .range(100.0..=4000.0),
        );
        ui.end_row();
        ui.label("E (psi)");
        ui.add(
            egui::DragValue::new(&mut c.e)
                .speed(10_000.0)
                .range(0.5e6..=3.0e6),
        );
        ui.end_row();
        ui.label("Fv (psi)");
        ui.add(
            egui::DragValue::new(&mut c.fv)
                .speed(1.0)
                .range(50.0..=500.0),
        );
        ui.end_row();
        ui.label("Fc perp (psi)");
        ui.add(
            egui::DragValue::new(&mut c.fc_perp)
                .speed(5.0)
                .range(100.0..=1500.0),
        );
        ui.end_row();
        ui.label("");
        ui.weak("Typed values; Southern Pine from the NDS Supplement");
        ui.end_row();
    }
}

fn span_table(ui: &mut egui::Ui, id: &str, rows: &[t::SpanRow], mark: Option<&str>) {
    egui::Grid::new(id).striped(true).show(ui, |ui| {
        ui.label("");
        for s in t::SPACINGS {
            ui.label(RichText::new(format!("{s}\" o.c.")).strong());
        }
        ui.end_row();
        for r in rows {
            let hit = mark == Some(r.label.as_str());
            ui.label(if hit {
                RichText::new(&r.label).strong()
            } else {
                RichText::new(&r.label)
            });
            for s in &r.spans {
                ui.label(s.text());
            }
            ui.end_row();
        }
    });
}

fn header_tab(ui: &mut egui::Ui, st: &mut State) {
    egui::Grid::new("calc_header_in")
        .num_columns(2)
        .show(ui, |ui| {
            material_row(ui, &mut st.material);
            length(ui, "Opening width", &mut st.header.span, 1.0);
            choose(
                ui,
                "calc_snow",
                "Ground snow load",
                &mut st.header.ground_snow,
                &[20.0, 30.0, 50.0, 70.0],
                " psf",
            );
            choose(
                ui,
                "calc_bw",
                "Building width",
                &mut st.header.building_width_ft,
                &[20.0, 28.0, 36.0],
                " ft",
            );
            let mut floors = f64::from(st.header.floors_above);
            choose(
                ui,
                "calc_fl",
                "Floors bearing above",
                &mut floors,
                &[0.0, 1.0],
                "",
            );
            st.header.floors_above = floors as u32;
            choose(
                ui,
                "calc_wd",
                "Wall depth",
                &mut st.header.wall_depth,
                &[3.5, 5.5],
                "\"",
            );
        });
    ui.separator();
    let input = st.header_input();
    match t::header_recommend(&input) {
        Some(r) => {
            ui.label(
                RichText::new(format!(
                    "Header {}: carries {} ({}); {} jack stud(s) each end",
                    r.label,
                    r.span.text(),
                    r.span.governs.name(),
                    r.jacks
                ))
                .strong(),
            );
        }
        None => {
            ui.colored_label(
                egui::Color32::from_rgb(200, 60, 40),
                format!(
                    "No header in the table carries {}: use an engineered beam",
                    fmt_ft_in(input.span)
                ),
            );
        }
    }
    ui.add_space(4.0);
    egui::ScrollArea::vertical()
        .max_height(220.0)
        .show(ui, |ui| {
            egui::Grid::new("calc_header_out")
                .striped(true)
                .show(ui, |ui| {
                    for h in ["Section", "Longest span", "Governs", "Jacks", ""] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for o in t::header_options(&input) {
                        ui.label(&o.label);
                        ui.label(o.span.text());
                        ui.label(o.span.governs.name());
                        ui.label(o.jacks.to_string());
                        ui.label(if o.passes { "carries" } else { "" });
                        ui.end_row();
                    }
                });
        });
    ui.weak("Computed from the NDS design values with a roof of 15 psf dead load, a 2' overhang and, with a floor above, 30 psf live and 10 psf dead. Check the printed table of the adopted code.");
}

fn joist_tab(ui: &mut egui::Ui, st: &mut State) {
    egui::Grid::new("calc_joist_in")
        .num_columns(2)
        .show(ui, |ui| {
            material_row(ui, &mut st.material);
            choose(
                ui,
                "calc_jl",
                "Live load",
                &mut st.joist.live,
                &[30.0, 40.0],
                " psf",
            );
            choose(
                ui,
                "calc_jd",
                "Dead load",
                &mut st.joist.dead,
                &[10.0, 20.0],
                " psf",
            );
            choose(
                ui,
                "calc_js",
                "Spacing",
                &mut st.joist.spacing,
                &t::SPACINGS,
                "\"",
            );
            length(ui, "Span to make", &mut st.joist_span, 1.0);
        });
    ui.separator();
    let input = st.joist_input();
    match t::joist_size_for(&input, st.joist_span) {
        Some(n) => ui.label(
            RichText::new(format!(
                "2x{n} at {}\" on centre spans {} (needs {})",
                input.spacing,
                t::joist_span(&JoistInput {
                    nominal: n,
                    ..input
                })
                .text(),
                fmt_ft_in(st.joist_span)
            ))
            .strong(),
        ),
        None => ui.colored_label(
            egui::Color32::from_rgb(200, 60, 40),
            "No joist of the table makes this span: add a bearing line or use engineered joists",
        ),
    };
    ui.add_space(4.0);
    let mark = t::joist_size_for(&input, st.joist_span).map(|n| format!("2x{n}"));
    span_table(
        ui,
        "calc_joist_out",
        &t::joist_table(&input),
        mark.as_deref(),
    );
    ui.weak("Longest span of each size, L/360 live load, L/240 total load. Check the printed table of the adopted code.");
}

fn rafter_tab(ui: &mut egui::Ui, st: &mut State) {
    egui::Grid::new("calc_rafter_in")
        .num_columns(2)
        .show(ui, |ui| {
            material_row(ui, &mut st.material);
            choose(
                ui,
                "calc_rs",
                "Ground snow load",
                &mut st.rafter.ground_snow,
                &[0.0, 20.0, 30.0, 50.0, 70.0],
                " psf",
            );
            choose(
                ui,
                "calc_rd",
                "Dead load",
                &mut st.rafter.dead,
                &[10.0, 15.0, 20.0],
                " psf",
            );
            choose(
                ui,
                "calc_rsp",
                "Spacing",
                &mut st.rafter.spacing,
                &t::SPACINGS,
                "\"",
            );
            ui.label("Pitch");
            ui.add(
                egui::DragValue::new(&mut st.rafter.pitch)
                    .speed(0.1)
                    .range(0.0..=24.0)
                    .suffix(":12"),
            );
            ui.end_row();
            ui.label("Finished ceiling attached");
            ui.checkbox(&mut st.rafter.ceiling_attached, "");
            ui.end_row();
            length(ui, "Run to make (plan)", &mut st.rafter_run, 1.0);
        });
    ui.separator();
    let input = st.rafter_input();
    match t::rafter_size_for(&input, st.rafter_run) {
        Some(n) => ui.label(
            RichText::new(format!(
                "2x{n} at {}\" on centre runs {} of plan (needs {})",
                input.spacing,
                t::rafter_span(&RafterInput {
                    nominal: n,
                    ..input
                })
                .text(),
                fmt_ft_in(st.rafter_run)
            ))
            .strong(),
        ),
        None => ui.colored_label(
            egui::Color32::from_rgb(200, 60, 40),
            "No rafter of the table makes this run: add a purlin brace or a ridge beam",
        ),
    };
    ui.add_space(4.0);
    let mark = t::rafter_size_for(&input, st.rafter_run).map(|n| format!("2x{n}"));
    span_table(
        ui,
        "calc_rafter_out",
        &t::rafter_table(&input),
        mark.as_deref(),
    );
    ui.weak("Span of the horizontal projection; snow load 0.7 x ground snow, at least 20 psf. Check the printed table of the adopted code.");
}

fn stair_tab(ui: &mut egui::Ui, st: &mut State) {
    egui::Grid::new("calc_stair_in")
        .num_columns(2)
        .show(ui, |ui| {
            length(
                ui,
                "Total rise (floor to floor)",
                &mut st.stair.total_rise,
                0.25,
            );
            ui.label("Riser aimed at");
            ui.add(
                egui::DragValue::new(&mut st.stair.target_riser)
                    .speed(0.05)
                    .range(4.0..=8.0)
                    .suffix("\""),
            );
            ui.end_row();
            ui.label("Tread depth");
            ui.add(
                egui::DragValue::new(&mut st.stair.tread)
                    .speed(0.05)
                    .range(6.0..=16.0)
                    .suffix("\""),
            );
            ui.end_row();
        });
    ui.separator();
    match t::stair_layout(&st.stair) {
        Some(r) => {
            ui.label(
                RichText::new(format!(
                    "{} risers of {:.3}\", {} treads of {:.2}\"",
                    r.risers, r.riser_height, r.treads, r.tread_depth
                ))
                .strong(),
            );
            ui.label(format!(
                "Run {}, stringer {}, angle {:.1} degrees, 2R + T = {:.2}\"",
                fmt_ft_in(r.total_run),
                fmt_ft_in(r.stringer),
                r.angle_deg,
                r.two_r_plus_t
            ));
            ui.add_space(4.0);
            for n in &r.notes {
                let c = if n.ok {
                    egui::Color32::from_rgb(60, 140, 70)
                } else {
                    egui::Color32::from_rgb(200, 60, 40)
                };
                ui.colored_label(c, format!("{} {}", if n.ok { "ok" } else { "no" }, n.text));
            }
        }
        None => {
            ui.label("Type a rise above zero.");
        }
    }
}

fn deck_tab(ui: &mut egui::Ui, st: &mut State) {
    egui::Grid::new("calc_deck_in")
        .num_columns(2)
        .show(ui, |ui| {
            material_row(ui, &mut st.material);
            let mut jn = f64::from(st.deck.joist_nominal);
            choose(
                ui,
                "calc_dj",
                "Joist size (2x)",
                &mut jn,
                &[6.0, 8.0, 10.0, 12.0],
                "",
            );
            st.deck.joist_nominal = jn as u32;
            choose(
                ui,
                "calc_djs",
                "Joist spacing",
                &mut st.deck.joist_spacing,
                &t::SPACINGS,
                "\"",
            );
            let mut bn = f64::from(st.deck.beam_nominal);
            choose(
                ui,
                "calc_db",
                "Beam size (2x)",
                &mut bn,
                &[6.0, 8.0, 10.0, 12.0],
                "",
            );
            st.deck.beam_nominal = bn as u32;
            let mut plies = f64::from(st.deck.beam_plies);
            choose(ui, "calc_dp", "Beam plies", &mut plies, &[2.0, 3.0], "");
            st.deck.beam_plies = plies as u32;
            length(
                ui,
                "Joist span (ledger to beam)",
                &mut st.deck.joist_span,
                1.0,
            );
            length(
                ui,
                "Joist overhang past the beam",
                &mut st.deck.cantilever,
                1.0,
            );
            choose(
                ui,
                "calc_ds",
                "Soil bearing",
                &mut st.deck.soil_bearing,
                &[1000.0, 1500.0, 2000.0, 3000.0],
                " psf",
            );
        });
    ui.separator();
    let d = st.deck_input();
    let r = t::deck_design(&d);
    let ok = |b: bool| {
        if b {
            egui::Color32::from_rgb(60, 140, 70)
        } else {
            egui::Color32::from_rgb(200, 60, 40)
        }
    };
    ui.colored_label(
        ok(r.joist_ok),
        format!(
            "Joists 2x{} at {}\" span {} ({}); the deck needs {}",
            d.joist_nominal,
            d.joist_spacing,
            r.joist.text(),
            r.joist.governs.name(),
            fmt_ft_in(d.joist_span)
        ),
    );
    ui.colored_label(
        ok(r.cantilever_ok),
        format!(
            "Overhang {} of at most {} (a quarter of the joist span)",
            fmt_ft_in(d.cantilever),
            fmt_ft_in(r.cantilever_max)
        ),
    );
    ui.label(
        RichText::new(format!(
            "Beam {}-2x{}: posts up to {} apart ({})",
            d.beam_plies,
            d.beam_nominal,
            r.beam.text(),
            r.beam.governs.name()
        ))
        .strong(),
    );
    ui.label(format!(
        "Load on a post {:.0} lb; footing {:.0}\" square at {} psf",
        r.post_load, r.footing_side, d.soil_bearing
    ));
    ui.weak("Pressure-treated, incised lumber in wet service; 40 psf live, 10 psf dead. Check DCA 6 and the adopted code.");
}

/// Draws the window when it is open.
pub fn show_all(ctx: &egui::Context, cx: &mut EditorContext) {
    if !is_open() {
        return;
    }
    let mut st = snapshot();
    let mut open = true;
    let mut apply = false;
    let mut use_selection = false;
    egui::Window::new("Structural Calculators")
        .id(egui::Id::new("structural_calculators"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(560.0, 520.0))
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for tab in Tab::ALL {
                    if ui.selectable_label(st.tab == tab, tab.title()).clicked() {
                        st.tab = tab;
                        st.message.clear();
                    }
                }
            });
            ui.separator();
            match st.tab {
                Tab::Header => header_tab(ui, &mut st),
                Tab::Joist => joist_tab(ui, &mut st),
                Tab::Rafter => rafter_tab(ui, &mut st),
                Tab::Stair => stair_tab(ui, &mut st),
                Tab::Deck => deck_tab(ui, &mut st),
            }
            ui.separator();
            ui.horizontal(|ui| {
                let enabled = can_apply(cx, st.tab);
                apply = ui
                    .add_enabled(enabled, egui::Button::new(apply_label(st.tab)))
                    .on_hover_text(apply_hint(st.tab))
                    .clicked();
                if matches!(st.tab, Tab::Header | Tab::Stair) {
                    use_selection = ui
                        .button("Use Selection")
                        .on_hover_text("Take the width of the selected opening or the rise of the selected stair")
                        .clicked();
                }
                if ui.button("Close").clicked() {
                    st.open = false;
                }
            });
            if !st.message.is_empty() {
                ui.weak(&st.message);
            }
            ui.weak("A design aid, not a structural design. The printed tables of the adopted code decide.");
        });
    if use_selection {
        prefill(cx, &mut st);
    }
    st.open &= open;
    state(|s| *s = st);
    if apply {
        cx.status = apply_current(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_ids_map_to_tabs() {
        for (id, tab) in [
            (HEADER, Tab::Header),
            (JOIST, Tab::Joist),
            (RAFTER, Tab::Rafter),
            (STAIR, Tab::Stair),
            (DECK, Tab::Deck),
        ] {
            assert_eq!(Tab::from_id(id), Some(tab));
        }
        assert_eq!(Tab::from_id("calc.nothing"), None);
        assert_eq!(Tab::ALL.len(), 5);
    }

    #[test]
    fn the_default_state_is_closed_on_the_header_tab() {
        let s = State::default();
        assert!(!s.open);
        assert_eq!(s.tab, Tab::Header);
        assert_eq!(s.header_input().material, s.material);
        assert_eq!(s.joist_input().spacing, 16.0);
    }
}
