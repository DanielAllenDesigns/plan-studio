//! Specification dialogs of the Trim tools, Material Region, Wall Hatching,
//! Polygon Shaped Deck and the 3D Solid tools: Corner Board, Quoin, Molding,
//! Material Region, Wall Hatching, Deck and 3D Solid Specification. Every
//! dialog has General, Materials, Line Style and Layer pages; the Molding
//! dialog also edits a custom cross section point by point. Opened by a double-click on the object (or right after a
//! solid, wall region or hatch is drawn); the dialog edits a [`Draft`] clone
//! that the tool stores on OK as one undo step.

use super::{
    off, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_ACCENT,
    PV_FAINT, PV_INK, PV_WALL,
};
use crate::editor::details_view::{material_names, PATTERN_NAMES};
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::details::{
    bounds, wall_rect, CornerBoard, DeckPolygon, DetailRef, DetailStyle, DetailsLayer,
    MaterialRegion, MoldingLine, MoldingSide, Quoin, QuoinStyle, RegionKind, Solid3d, SolidKind,
    WallHatch,
};
use plan_core::geometry::Point;
use plan_core::moldings::builtin_profiles;
use plan_core::units::fmt_ft_in;
use plan_core::walls::Side;
use plan_core::Id;

mod management;
use super::molding;
#[cfg(test)]
pub use management::{components_open, management_open};
pub use management::{
    open_auto_detail, open_components, open_management, select_detail, show_windows,
};

const FULL_TABS: &[Tab] = &[
    on("General"),
    on("Materials"),
    on("Line Style"),
    on("Layer"),
];
const HATCH_TABS: &[Tab] = &[on("General"), on("Line Style"), on("Layer")];
/// Corner boards and quoins: General, Layer, Materials and Components.
const TRIM_TABS: &[Tab] = &[
    on("General"),
    on("Line Style"),
    on("Materials"),
    on("Components"),
    on("Layer"),
];
/// Molding Specification: General, Selected Line, the Moldings panel, Line
/// Style, Fill Style (not built), Materials, Label and Components.
const MOLDING_TABS: &[Tab] = &[
    on("General"),
    on("Moldings"),
    on("Selected Line"),
    on("Line Style"),
    off("Fill Style"),
    on("Materials"),
    on("Label"),
    on("Components"),
    on("Layer"),
];

/// Material names offered before the library's own.
pub const BASE_MATERIALS: [&str; 8] = [
    "Painted White Trim",
    "Oak Flooring",
    "Wood",
    "Concrete",
    "Stone",
    "Metal",
    "Glass",
    "Ceramic Tile 12x12",
];

/// The object being edited.
#[derive(Debug, Clone, PartialEq)]
pub enum Draft {
    CornerBoard(CornerBoard),
    Quoin(Quoin),
    Molding(MoldingLine),
    Region(MaterialRegion),
    Hatch(WallHatch),
    Deck(DeckPolygon),
    Solid(Solid3d),
}

impl Draft {
    /// The draft of the object `r` on `layer`.
    pub fn of(layer: &DetailsLayer, r: DetailRef) -> Option<Draft> {
        match r {
            DetailRef::CornerBoard(i) => layer.corner_board(i).cloned().map(Draft::CornerBoard),
            DetailRef::Quoin(i) => layer.quoin(i).cloned().map(Draft::Quoin),
            DetailRef::Molding(i) => layer.molding(i).cloned().map(Draft::Molding),
            DetailRef::Region(i) => layer.region(i).cloned().map(Draft::Region),
            DetailRef::Hatch(i) => layer.hatch(i).cloned().map(Draft::Hatch),
            DetailRef::Deck(i) => layer.deck(i).cloned().map(Draft::Deck),
            DetailRef::Solid(i) => layer.solid(i).cloned().map(Draft::Solid),
        }
    }

    /// Writes the draft back over the object with the same id; returns
    /// whether that object still exists.
    pub fn apply(&self, layer: &mut DetailsLayer) -> bool {
        fn put<T: Clone>(v: &mut [T], id: Id, get: impl Fn(&T) -> Id, new: &T) -> bool {
            v.iter_mut()
                .find(|x| get(x) == id)
                .map(|x| *x = new.clone())
                .is_some()
        }
        match self {
            Draft::CornerBoard(d) => put(&mut layer.corner_boards, d.id, |x| x.id, d),
            Draft::Quoin(d) => put(&mut layer.quoins, d.id, |x| x.id, d),
            Draft::Molding(d) => put(&mut layer.moldings, d.id, |x| x.id, d),
            Draft::Region(d) => put(&mut layer.regions, d.id, |x| x.id, d),
            Draft::Hatch(d) => put(&mut layer.hatches, d.id, |x| x.id, d),
            Draft::Deck(d) => put(&mut layer.decks, d.id, |x| x.id, d),
            Draft::Solid(d) => put(&mut layer.solids, d.id, |x| x.id, d),
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Draft::CornerBoard(_) => "Corner Board Specification",
            Draft::Quoin(_) => "Quoin Specification",
            Draft::Molding(_) => "Molding Specification",
            Draft::Region(_) => "Material Region Specification",
            Draft::Hatch(_) => "Wall Hatching Specification",
            Draft::Deck(_) => "Deck Specification",
            Draft::Solid(_) => "3D Solid Specification",
        }
    }

    fn tabs(&self) -> &'static [Tab] {
        match self {
            Draft::Hatch(_) => HATCH_TABS,
            Draft::CornerBoard(_) | Draft::Quoin(_) => TRIM_TABS,
            Draft::Molding(_) => MOLDING_TABS,
            _ => FULL_TABS,
        }
    }

    /// The Components panel list of the draft, when it has one.
    fn components_mut(&mut self) -> Option<&mut Vec<String>> {
        match self {
            Draft::CornerBoard(d) => Some(&mut d.components),
            Draft::Quoin(d) => Some(&mut d.components),
            Draft::Molding(d) => Some(&mut d.components),
            _ => None,
        }
    }

    fn layer_mut(&mut self) -> &mut String {
        match self {
            Draft::CornerBoard(d) => &mut d.layer,
            Draft::Quoin(d) => &mut d.layer,
            Draft::Molding(d) => &mut d.layer,
            Draft::Region(d) => &mut d.layer,
            Draft::Hatch(d) => &mut d.layer,
            Draft::Deck(d) => &mut d.layer,
            Draft::Solid(d) => &mut d.layer,
        }
    }

    fn style_mut(&mut self) -> &mut DetailStyle {
        match self {
            Draft::CornerBoard(d) => &mut d.style,
            Draft::Quoin(d) => &mut d.style,
            Draft::Molding(d) => &mut d.style,
            Draft::Region(d) => &mut d.style,
            Draft::Hatch(d) => &mut d.style,
            Draft::Deck(d) => &mut d.style,
            Draft::Solid(d) => &mut d.style,
        }
    }

    fn material_mut(&mut self) -> Option<&mut String> {
        match self {
            Draft::CornerBoard(d) => Some(&mut d.material),
            Draft::Quoin(d) => Some(&mut d.material),
            Draft::Molding(d) => Some(&mut d.material),
            Draft::Region(d) => Some(&mut d.material),
            Draft::Hatch(_) => None,
            Draft::Deck(d) => Some(&mut d.material),
            Draft::Solid(d) => Some(&mut d.material),
        }
    }
}

pub struct DetailsDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    draft: Draft,
    layers: Vec<String>,
    materials: Vec<String>,
    fields: Fields,
    /// The Moldings panel of a molding.
    panel: molding::MoldingPanel,
    /// The edge the Selected Line panel and Molding on Selected Edge refer to.
    edge: usize,
    /// Edit was pressed on a profile of the Moldings panel.
    edit_request: Option<String>,
    /// The move fields of the Selected Line panel: along the edge, across it,
    /// up (Select Edit Plane).
    plane: [f64; 3],
}

impl DetailsDialog {
    /// The dialog for object `r`, or `None` when it no longer exists.
    /// `layers` are the plan's layer names for the Layer page.
    pub fn new(layer: &DetailsLayer, r: DetailRef, layers: Vec<String>) -> Option<Self> {
        let draft = Draft::of(layer, r)?;
        let mut materials: Vec<String> = BASE_MATERIALS.iter().map(|s| (*s).to_string()).collect();
        for n in material_names() {
            if !materials.contains(&n) {
                materials.push(n);
            }
        }
        let mut catalog = builtin_profiles();
        for p in &layer.profiles {
            match catalog
                .iter_mut()
                .find(|q| q.name.eq_ignore_ascii_case(&p.name))
            {
                Some(q) => *q = p.clone(),
                None => catalog.push(p.clone()),
            }
        }
        let mut draft = draft;
        if let Draft::Molding(m) = &mut draft {
            m.ensure_table();
        }
        Some(Self {
            frame: SpecDialog::new(draft.title(), ("details_spec", draft.title())),
            form: Form {
                draft,
                layers,
                materials,
                fields: Fields::default(),
                panel: molding::MoldingPanel::new(molding::PanelOptions::default(), catalog),
                edge: crate::tools::molding::selected_edge_of(match r {
                    DetailRef::Molding(i) => i,
                    _ => 0,
                }),
                edit_request: None,
                plane: [0.0; 3],
            },
        })
    }

    /// The profile the Edit button of the Moldings panel asked to open, if any.
    /// The shell host (`shell/spec_dialogs.rs`, not owned here) polls it; see
    /// docs/integration-queue.md.
    #[allow(dead_code)]
    pub fn take_edit_request(&mut self) -> Option<String> {
        self.form.edit_request.take()
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn draft(&self) -> &Draft {
        &self.form.draft
    }

    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut Draft {
        &mut self.form.draft
    }

    #[cfg(test)]
    pub fn tab_names(&self) -> Vec<&'static str> {
        self.form.draft.tabs().iter().map(|t| t.name).collect()
    }
}

fn number_row(ui: &mut Ui, label: &str, value: &mut f64, range: std::ops::RangeInclusive<f64>) {
    row(ui, label, |ui| {
        ui.add(egui::DragValue::new(value).speed(0.05).range(range))
    });
}

impl Form {
    fn general(&mut self, ui: &mut Ui) {
        section(ui, "General");
        let fields = &mut self.fields;
        match &mut self.draft {
            Draft::CornerBoard(b) => {
                fields.length_row(ui, "Width", "cb_width", &mut b.width);
                fields.length_row(ui, "Thickness", "cb_thickness", &mut b.thickness);
                row(ui, "Bottom", |ui| {
                    ui.checkbox(&mut b.set_bottom, "Set Bottom")
                        .on_hover_text("Off: the bottom of the floor platform")
                });
                if b.set_bottom {
                    fields.length_row(ui, "Bottom height", "cb_base", &mut b.base);
                }
                row(ui, "Top", |ui| {
                    ui.checkbox(&mut b.set_top, "Set Top")
                        .on_hover_text("Off: the top plate of the walls")
                });
                if b.set_top {
                    let mut top = b.top();
                    if fields.length_row(ui, "Top height", "cb_top", &mut top) {
                        b.height = (top - b.base).max(0.0);
                    }
                } else {
                    ui.weak(format!(
                        "Runs from {} to the top plate ({})",
                        fmt_ft_in(b.base),
                        fmt_ft_in(b.top())
                    ));
                }
                row(ui, "Sheathing", |ui| {
                    ui.checkbox(&mut b.recessed, "Recessed To Sheathing Layer")
                });
                ui.add_space(6.0);
                ui.weak(format!(
                    "{} corner, lumber {:.2} cu ft",
                    if b.inside { "Inside" } else { "Outside" },
                    b.volume() / 1728.0
                ));
            }
            Draft::Quoin(q) => {
                fields.length_row(ui, "Width", "q_width", &mut q.width);
                fields.length_row(ui, "Thickness", "q_depth", &mut q.depth);
                fields.length_row(ui, "Quoin Height", "q_height", &mut q.height);
                fields.length_row(ui, "Quoin Gap", "q_gap", &mut q.gap);
                row(ui, "Style", |ui| {
                    let mut st = q.quoin_style();
                    egui::ComboBox::from_id_salt("quoin_style")
                        .selected_text(st.name())
                        .show_ui(ui, |ui| {
                            for k in QuoinStyle::ALL {
                                ui.selectable_value(&mut st, k, k.name());
                            }
                        });
                    q.set_quoin_style(st);
                });
                row(ui, "Start", |ui| {
                    ui.checkbox(&mut q.swap_start, "Swap Start Block")
                });
                row(ui, "Bottom", |ui| {
                    ui.checkbox(&mut q.set_bottom, "Set Bottom")
                });
                if q.set_bottom {
                    fields.length_row(ui, "Bottom height", "q_base", &mut q.base);
                }
                row(ui, "Top", |ui| ui.checkbox(&mut q.set_top, "Set Top"));
                if q.set_top {
                    let mut top = q.top();
                    if fields.length_row(ui, "Top height", "q_top", &mut top) {
                        q.total_height = (top - q.base).max(0.0);
                    }
                } else {
                    ui.weak(format!(
                        "Stacks up to the top plate ({})",
                        fmt_ft_in(q.top())
                    ));
                }
                row(ui, "Sheathing", |ui| {
                    ui.checkbox(&mut q.recessed, "Recessed To Sheathing Layer")
                });
                ui.add_space(6.0);
                ui.weak(format!(
                    "{} corner, {} courses, {} blocks",
                    if q.inside { "Inside" } else { "Outside" },
                    q.courses(),
                    q.block_count()
                ));
            }
            Draft::Molding(m) => {
                let floor_note = "The bottom edge of the molding above the floor";
                let mut h = m.elevation;
                if fields.length_row(ui, "Height from Z=0", "m_elev", &mut h) {
                    m.elevation = h;
                    m.heights.clear();
                    m.automatic = false;
                }
                ui.weak(floor_note);
                row(ui, "Twisted Joints", |ui| {
                    ui.vertical(|ui| {
                        ui.checkbox(
                            &mut m.auto_orient,
                            "Auto Calc Orientation at Twisted Joints",
                        );
                        ui.checkbox(&mut m.mitre_twisted, "Mitre Molding at Twisted Joints");
                        ui.checkbox(
                            &mut m.mitre_if_next_off,
                            "Mitre Molding If Next Edge Turned Off",
                        );
                    })
                    .inner
                });
                let n = m.edge_count();
                if n > 0 {
                    self.edge = self.edge.min(n - 1);
                    let mut on = m.edge_on(self.edge);
                    row(ui, "Selected Edge", |ui| {
                        if ui.small_button("\u{25C0}").clicked() && self.edge > 0 {
                            self.edge -= 1;
                        }
                        ui.label(format!("{} of {}", self.edge + 1, n));
                        if ui.small_button("\u{25B6}").clicked() && self.edge + 1 < n {
                            self.edge += 1;
                        }
                    });
                    let on_before = on;
                    row(ui, "Molding on Selected Edge", |ui| {
                        ui.checkbox(&mut on, "On")
                    });
                    if on != on_before {
                        m.set_edge_on(self.edge, on);
                    }
                }
                row(ui, "Generated", |ui| {
                    ui.add_enabled(
                        m.automatic,
                        egui::Checkbox::new(&mut m.automatic, "Automatically Generated"),
                    )
                });
                ui.add_space(6.0);
                ui.weak(format!(
                    "Length {} ({} 3D), {} edge{}, projects to the {}",
                    fmt_ft_in(m.length()),
                    fmt_ft_in(m.edge_lengths_on()),
                    n,
                    if n == 1 { "" } else { "s" },
                    match m.side {
                        MoldingSide::Left => "left",
                        MoldingSide::Right => "right",
                    }
                ));
            }
            Draft::Region(r) => {
                match r.kind {
                    RegionKind::Floor => {
                        row(ui, "Region on", |ui| ui.label("Floor"));
                        row(ui, "Finish layers", |ui| {
                            ui.checkbox(&mut r.cut_finish_layers, "Cut finish layers")
                        });
                    }
                    RegionKind::Wall(_) => {
                        row(ui, "Region on", |ui| {
                            ui.radio_value(&mut r.side, Side::Left, "Left face");
                            ui.radio_value(&mut r.side, Side::Right, "Right face")
                        });
                        let (mut u0, mut u1, mut v0, mut v1) =
                            r.uv_bounds().unwrap_or((0.0, 0.0, 0.0, 0.0));
                        let a = fields.length_row(ui, "From (along wall)", "r_u0", &mut u0);
                        let b = fields.length_row(ui, "To (along wall)", "r_u1", &mut u1);
                        let c = fields.length_row(ui, "Bottom", "r_v0", &mut v0);
                        let d = fields.length_row(ui, "Top", "r_v1", &mut v1);
                        if a || b || c || d {
                            r.outline = wall_rect(u0, u1, v0, v1);
                        }
                    }
                }
                fields.length_row(ui, "Thickness", "r_thickness", &mut r.thickness);
                ui.add_space(6.0);
                ui.weak(format!("Area {:.1} sq ft", r.area() / 144.0));
            }
            Draft::Hatch(h) => {
                row(ui, "Pattern", |ui| {
                    egui::ComboBox::from_id_salt("hatch_pattern")
                        .selected_text(h.pattern.clone())
                        .show_ui(ui, |ui| {
                            for p in PATTERN_NAMES {
                                ui.selectable_value(&mut h.pattern, p.to_string(), p);
                            }
                        });
                });
                number_row(ui, "Scale", &mut h.scale, 0.1..=10.0);
                fields.degrees_row(ui, "Angle", "deg_hatch", &mut h.angle);
                ui.add_space(6.0);
                ui.weak("Drawn inside the wall between its layers");
            }
            Draft::Deck(d) => {
                fields.length_row(ui, "Top height", "d_elev", &mut d.elevation);
                fields.length_row(ui, "Board thickness", "d_board", &mut d.board_thickness);
                row(ui, "Railing", |ui| {
                    ui.checkbox(&mut d.railing, "Railing around the edge")
                });
                ui.add_space(6.0);
                ui.weak(format!(
                    "Area {:.1} sq ft, perimeter {}, decking {:.1} cu ft",
                    d.area() / 144.0,
                    fmt_ft_in(d.perimeter()),
                    d.volume() / 1728.0
                ));
            }
            Draft::Solid(s) => {
                match &mut s.kind {
                    SolidKind::Box { w, d, h } => {
                        fields.length_row(ui, "Width", "s_w", w);
                        fields.length_row(ui, "Depth", "s_d", d);
                        fields.length_row(ui, "Height", "s_h", h);
                    }
                    SolidKind::Cylinder { r, h } | SolidKind::Cone { r, h } => {
                        fields.length_row(ui, "Radius", "s_r", r);
                        fields.length_row(ui, "Height", "s_h", h);
                    }
                    SolidKind::Sphere { r } => {
                        fields.length_row(ui, "Radius", "s_r", r);
                    }
                    SolidKind::PolylineSolid { h, .. } | SolidKind::Pyramid { h, .. } => {
                        fields.length_row(ui, "Height", "s_h", h);
                    }
                    SolidKind::Face { .. } => {}
                }
                fields.length_row(ui, "Bottom height", "s_elev", &mut s.elevation);
                fields.degrees_row(ui, "Rotation", "deg_rot", &mut s.rotation);
                ui.add_space(6.0);
                ui.weak(format!(
                    "{}, volume {:.2} cu ft",
                    s.kind.name(),
                    s.volume() / 1728.0
                ));
            }
        }
    }

    /// Line Style page: the detail's own color, weight and dash in the plan.
    fn line_style_page(&mut self, ui: &mut Ui) {
        use plan_core::layers::LineStyle;
        section(ui, "Line Style");
        let st = self.draft.style_mut();
        let mut own = st.color.is_some();
        if ui.checkbox(&mut own, "Own color").changed() {
            st.color = own.then_some([60, 60, 60]);
        }
        if let Some(c) = &mut st.color {
            row(ui, "Color", |ui| ui.color_edit_button_srgb(c));
        }
        let mut own = st.weight.is_some();
        if ui.checkbox(&mut own, "Own line weight").changed() {
            st.weight = own.then_some(25);
        }
        if let Some(w) = &mut st.weight {
            row(ui, "Weight", |ui| {
                ui.add(egui::DragValue::new(w).range(5..=200).suffix(" /100 mm"))
            });
        }
        row(ui, "Line style", |ui| {
            let names = [
                (None, "As the layer"),
                (Some(LineStyle::Solid), "Solid"),
                (Some(LineStyle::Dashed), "Dashed"),
                (Some(LineStyle::Dotted), "Dotted"),
                (Some(LineStyle::DashDot), "Dash-dot"),
            ];
            let current = names
                .iter()
                .find(|(v, _)| *v == st.dash)
                .map_or("As the layer", |(_, n)| *n);
            egui::ComboBox::from_id_salt("details_line_style")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for (v, n) in names {
                        ui.selectable_value(&mut st.dash, v, n);
                    }
                });
        });
        if st.is_default() {
            ui.weak("Drawn with the color, weight and dash of its layer.");
        }
    }

    /// The Moldings panel of a molding (profiles, offsets, stacking) with
    /// Extrude Inside Polyline and Reverse Direction.
    fn moldings_page(&mut self, ui: &mut Ui) {
        let Draft::Molding(m) = &mut self.draft else {
            return;
        };
        section(ui, "Moldings");
        row(ui, "Polyline", |ui| {
            ui.checkbox(&mut m.extrude_inside, "Extrude Inside Polyline")
                .on_hover_text(
                    "A closed polyline puts the profile inside whichever way it was drawn",
                )
        });
        row(ui, "Direction", |ui| {
            if ui.button("Reverse Direction").clicked() {
                m.reverse_direction();
            }
        });
        let ev = self.panel.show(ui, &mut m.table);
        if ev.changed {
            m.automatic = false;
            // The single-profile fields follow the first row.
            if let Some(r) = m.table.rows.first() {
                m.width = r.width;
                m.height = r.height;
            }
        }
        if ev.edit.is_some() {
            self.edit_request = ev.edit;
        }
    }

    /// Selected Line panel: the 3D length and the two angles of an edge, its
    /// end heights, and Select Edit Plane moves.
    fn selected_line_page(&mut self, ui: &mut Ui) {
        let Draft::Molding(m) = &mut self.draft else {
            return;
        };
        section(ui, "Selected Line");
        let n = m.edge_count();
        if n == 0 {
            ui.weak("The molding has no edge");
            return;
        }
        self.edge = self.edge.min(n - 1);
        row(ui, "Edge", |ui| {
            if ui.small_button("\u{25C0}").clicked() && self.edge > 0 {
                self.edge -= 1;
            }
            ui.label(format!("{} of {}", self.edge + 1, n));
            if ui.small_button("\u{25B6}").clicked() && self.edge + 1 < n {
                self.edge += 1;
            }
        });
        let i = self.edge;
        let fields = &mut self.fields;
        let (mut xy, mut from) = m.edge_angles(i);
        let mut len = m.edge_length_3d(i);
        let a = fields.length_row(ui, "3D Length", "sl_len", &mut len);
        let b = fields.degrees_row(ui, "Angle in XY Plane", "deg_sl_xy", &mut xy);
        let c = fields.degrees_row(ui, "Angle from XY Plane", "deg_sl_from", &mut from);
        if (a || b || c) && len > 0.0 {
            m.set_edge_3d(i, len, xy, from.clamp(-89.0, 89.0));
        }
        let mut z0 = m.vertex_bottom(i);
        let mut z1 = m.vertex_bottom(i + 1);
        if fields.length_row(ui, "Start height", "sl_z0", &mut z0) {
            m.set_vertex_bottom(i, z0);
        }
        if fields.length_row(ui, "End height", "sl_z1", &mut z1) {
            m.set_vertex_bottom(i + 1, z1);
        }
        section(ui, "Select Edit Plane");
        let plane = &mut self.plane;
        fields.length_row(ui, "Along the edge", "sl_along", &mut plane[0]);
        fields.length_row(ui, "Across (perpendicular)", "sl_across", &mut plane[1]);
        fields.length_row(ui, "Up", "sl_up", &mut plane[2]);
        ui.horizontal(|ui| {
            if ui.button("Move Edge").clicked() {
                m.move_edge(i, plane[0], plane[1], plane[2]);
                *plane = [0.0; 3];
            }
            ui.weak("Moves both ends of the edge; the edges next to it follow.");
        });
    }

    /// Label panel: a custom label for the molding.
    fn label_page(&mut self, ui: &mut Ui) {
        let Draft::Molding(m) = &mut self.draft else {
            return;
        };
        section(ui, "Label");
        row(ui, "Show label", |ui| {
            ui.checkbox(&mut m.show_label, "Display")
        });
        row(ui, "Label text", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut m.label)
                    .hint_text("the profile name")
                    .desired_width(200.0),
            )
        });
    }

    /// Components panel: the names of the components that make the object.
    fn components_page(&mut self, ui: &mut Ui) {
        section(ui, "Components");
        let Some(list) = self.draft.components_mut() else {
            return;
        };
        let mut remove = None;
        for (i, c) in list.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(c).desired_width(200.0));
                if ui.small_button("\u{2212}").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            list.remove(i);
        }
        if ui.button("Add Component").clicked() {
            list.push(String::new());
        }
        if list.is_empty() {
            ui.weak("No components. They are listed in the Materials List.");
        }
    }

    fn materials_page(&mut self, ui: &mut Ui) {
        section(ui, "Materials");
        let names = self.materials.clone();
        let Some(material) = self.draft.material_mut() else {
            return;
        };
        row(ui, "Material", |ui| {
            egui::ComboBox::from_id_salt("details_material")
                .selected_text(material.clone())
                .width(220.0)
                .show_ui(ui, |ui| {
                    if !names.contains(material) {
                        let own = material.clone();
                        ui.selectable_value(material, own.clone(), own);
                    }
                    for n in &names {
                        ui.selectable_value(material, n.clone(), n);
                    }
                });
        });
    }

    fn layer_page(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        let layers = self.layers.clone();
        let layer = self.draft.layer_mut();
        row(ui, "Layer", |ui| {
            egui::ComboBox::from_id_salt("details_layer")
                .selected_text(layer.clone())
                .show_ui(ui, |ui| {
                    for name in &layers {
                        ui.selectable_value(layer, name.clone(), name);
                    }
                });
        });
    }
}

impl Form {
    fn panel_selected(&self) -> usize {
        self.panel.selected()
    }
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        self.draft.tabs()
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() || self.panel.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        let positive = |ok: bool, msg: &str| (!ok).then(|| msg.to_string());
        let err = match &self.draft {
            Draft::CornerBoard(b) => positive(
                b.width > 0.0 && b.thickness > 0.0 && b.height > 0.0,
                "The board needs a width, a thickness and a height",
            ),
            Draft::Quoin(q) => positive(
                q.width > 0.0 && q.height > 0.0 && q.depth > 0.0 && q.total_height > 0.0,
                "The quoins need a size, a depth and a stack height",
            ),
            Draft::Molding(m) => positive(
                m.height > 0.0
                    && m.width > 0.0
                    && m.table
                        .rows
                        .iter()
                        .all(|r| r.width > 0.0 && r.height > 0.0 && r.profile.is_valid()),
                "Every profile of the molding needs a height and a projection",
            ),
            Draft::Region(r) => {
                if r.thickness <= 0.0 {
                    Some("The thickness must be greater than zero".into())
                } else if r.material.trim().is_empty() {
                    Some("Pick a material".into())
                } else if let Some((u0, u1, v0, v1)) = r.uv_bounds() {
                    positive(
                        u1 > u0 && v1 > v0,
                        "The region must have a length and a height",
                    )
                } else {
                    None
                }
            }
            Draft::Hatch(h) => positive(h.scale > 0.0, "The scale must be greater than zero"),
            Draft::Deck(d) => positive(d.board_thickness > 0.0, "The boards need a thickness"),
            Draft::Solid(s) => {
                let ok = match &s.kind {
                    SolidKind::Box { w, d, h } => *w > 0.0 && *d > 0.0 && *h > 0.0,
                    SolidKind::Cylinder { r, h } | SolidKind::Cone { r, h } => *r > 0.0 && *h > 0.0,
                    SolidKind::Sphere { r } => *r > 0.0,
                    SolidKind::PolylineSolid { h, .. } | SolidKind::Pyramid { h, .. } => *h > 0.0,
                    SolidKind::Face { .. } => true,
                };
                positive(ok, "The solid needs positive dimensions")
            }
        };
        if err.is_some() {
            return err;
        }
        let mut probe = self.draft.clone();
        if probe.layer_mut().trim().is_empty() {
            return Some("Pick a layer".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        let name = self.draft.tabs()[tab].name;
        match name {
            "General" => self.general(ui),
            "Moldings" => self.moldings_page(ui),
            "Selected Line" => self.selected_line_page(ui),
            "Label" => self.label_page(ui),
            "Components" => self.components_page(ui),
            "Materials" => self.materials_page(ui),
            "Line Style" => self.line_style_page(ui),
            "Layer" => self.layer_page(ui),
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        pv_text(
            p,
            Pos2::new(rect.center().x, rect.min.y + 6.0),
            Align2::CENTER_CENTER,
            self.draft.title().trim_end_matches(" Specification"),
            11.0,
        );
        let area = Rect::from_min_max(
            Pos2::new(rect.min.x + 10.0, rect.min.y + 26.0),
            Pos2::new(rect.max.x - 10.0, rect.max.y - 10.0),
        );
        let ink = Stroke::new(1.5_f32, PV_INK);
        match &self.draft {
            Draft::CornerBoard(b) => {
                // Enlarged: the board is a few inches on a building.
                let k = 12.0 / b.width.max(0.1);
                let apex = Point::ZERO;
                let l = b
                    .axes
                    .l_polygon(apex, b.width * k, b.width * k, b.thickness * k);
                outline_preview(p, area, &l, PV_WALL, ink);
            }
            Draft::Quoin(q) => {
                let (la, lb) = q.course_lengths(0);
                let l = q.axes.l_polygon(Point::ZERO, la, lb, q.depth * 2.0);
                outline_preview(p, area, &l, PV_WALL, ink);
            }
            Draft::Molding(m) => {
                if m.table.is_empty() {
                    let s = m.section();
                    outline_preview(p, area, &s, PV_WALL, ink);
                } else {
                    molding::preview(p, area, &m.table, self.panel_selected());
                }
            }
            Draft::Region(r) => {
                if r.outline.len() >= 3 {
                    outline_preview(p, area, &r.outline, PV_ACCENT.gamma_multiply(0.4), ink);
                }
            }
            Draft::Hatch(_) => {
                let r = Rect::from_center_size(area.center(), egui::vec2(120.0, 40.0));
                p.rect_filled(r, 0.0, PV_WALL);
                p.rect_stroke(r, 0.0, ink, egui::StrokeKind::Inside);
                let mut x = r.min.x - 40.0;
                while x < r.max.x {
                    let a = Pos2::new(x.max(r.min.x), r.max.y - (x.max(r.min.x) - x));
                    let b = Pos2::new(
                        (x + 40.0).min(r.max.x),
                        r.max.y - ((x + 40.0).min(r.max.x) - x),
                    );
                    p.line_segment([a, b], Stroke::new(1.0_f32, PV_FAINT));
                    x += 10.0;
                }
            }
            Draft::Deck(d) => outline_preview(p, area, &d.outline, PV_WALL, ink),
            Draft::Solid(s) => outline_preview(p, area, &s.local_footprint(), PV_WALL, ink),
        }
    }
}

/// Draws `outline` scaled to fit `area`.
fn outline_preview(p: &Painter, area: Rect, outline: &[Point], fill: Color32, stroke: Stroke) {
    if outline.len() < 3 {
        return;
    }
    let (lo, hi) = bounds(outline);
    let (w, h) = ((hi.x - lo.x).max(1e-3), (hi.y - lo.y).max(1e-3));
    let k = (f64::from(area.width()) / w).min(f64::from(area.height()) / h) as f32;
    let origin = area.center() - egui::vec2(w as f32 * k * 0.5, -(h as f32) * k * 0.5);
    let to_screen = |q: &Point| {
        Pos2::new(
            origin.x + (q.x - lo.x) as f32 * k,
            origin.y - (q.y - lo.y) as f32 * k,
        )
    };
    let pts: Vec<Pos2> = outline.iter().map(to_screen).collect();
    let mut closed = pts.clone();
    closed.push(closed[0]);
    // Concave outlines (the corner "L") are filled as the triangles of a
    // fan only when convex; otherwise just outlined over a pale backdrop.
    p.add(egui::Shape::convex_polygon(
        pts,
        fill.gamma_multiply(0.5),
        Stroke::NONE,
    ));
    p.add(egui::Shape::line(closed, stroke));
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::details::MoldingProfile;
    use plan_core::moldings::{builtin_profiles, MoldingType};

    fn layer() -> DetailsLayer {
        let sq = vec![
            Point::new(0.0, 0.0),
            Point::new(96.0, 0.0),
            Point::new(96.0, 96.0),
            Point::new(0.0, 96.0),
        ];
        DetailsLayer {
            corner_boards: vec![CornerBoard {
                id: 1,
                ..CornerBoard::default()
            }],
            quoins: vec![Quoin {
                id: 2,
                ..Quoin::default()
            }],
            moldings: vec![MoldingLine::new(
                3,
                vec![Point::ZERO, Point::new(96.0, 0.0)],
                MoldingProfile::Crown,
                109.0,
            )],
            regions: vec![
                MaterialRegion::floor(4, sq.clone()),
                MaterialRegion::wall(5, 77, Side::Left, 12.0, 60.0, 0.0, 48.0),
            ],
            hatches: vec![WallHatch {
                id: 6,
                wall_id: 77,
                ..WallHatch::default()
            }],
            decks: vec![DeckPolygon::new(7, sq)],
            solids: vec![Solid3d::new(8, SolidKind::Sphere { r: 12.0 }, Point::ZERO)],
            ..DetailsLayer::default()
        }
    }

    fn names() -> Vec<String> {
        vec!["Moldings".into(), "Decks".into()]
    }

    #[test]
    fn each_kind_has_its_pages() {
        let l = layer();
        let tabs = |r| DetailsDialog::new(&l, r, names()).unwrap().tab_names();
        for r in [
            DetailRef::Region(4),
            DetailRef::Deck(7),
            DetailRef::Solid(8),
        ] {
            assert_eq!(
                tabs(r),
                ["General", "Materials", "Line Style", "Layer"],
                "{r:?}"
            );
        }
        // Corner boards and quoins: General, Layer, Materials, Components.
        for r in [DetailRef::CornerBoard(1), DetailRef::Quoin(2)] {
            assert_eq!(
                tabs(r),
                ["General", "Line Style", "Materials", "Components", "Layer"],
                "{r:?}"
            );
        }
        // The Molding Specification has the panels of the manual.
        assert_eq!(
            tabs(DetailRef::Molding(3)),
            [
                "General",
                "Moldings",
                "Selected Line",
                "Line Style",
                "Fill Style",
                "Materials",
                "Label",
                "Components",
                "Layer"
            ]
        );
        assert_eq!(
            tabs(DetailRef::Hatch(6)),
            ["General", "Line Style", "Layer"]
        );
        // Line Style is live.
        let d = DetailsDialog::new(&l, DetailRef::Deck(7), names()).unwrap();
        let line = d
            .form
            .draft
            .tabs()
            .iter()
            .find(|t| t.name == "Line Style")
            .unwrap();
        assert!(line.enabled);
        assert!(DetailsDialog::new(&l, DetailRef::Deck(99), names()).is_none());
    }

    /// Every text a page draws.
    fn page_texts(d: &mut DetailsDialog, page: &str) -> Vec<String> {
        fn texts(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|x| texts(x, out)),
                _ => {}
            }
        }
        let tab = d.form.tabs().iter().position(|t| t.name == page).unwrap();
        let ctx = egui::Context::default();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| d.form.page(ui, tab));
        });
        let mut all = Vec::new();
        for c in &out.shapes {
            texts(&c.shape, &mut all);
        }
        all
    }

    #[test]
    fn the_line_style_page_is_stored_with_the_detail() {
        use plan_core::layers::LineStyle;
        let mut l = layer();
        for r in [
            DetailRef::CornerBoard(1),
            DetailRef::Quoin(2),
            DetailRef::Molding(3),
            DetailRef::Region(4),
            DetailRef::Hatch(6),
            DetailRef::Deck(7),
            DetailRef::Solid(8),
        ] {
            let mut d = DetailsDialog::new(&l, r, names()).unwrap();
            let drawn = page_texts(&mut d, "Line Style");
            assert!(drawn.iter().any(|t| t == "Own color"), "{r:?} {drawn:?}");
            let st = d.form.draft.style_mut();
            assert!(st.is_default());
            st.color = Some([200, 30, 30]);
            st.weight = Some(50);
            st.dash = Some(LineStyle::Dashed);
            assert!(d.draft().apply(&mut l), "{r:?}");
            let back = DetailsDialog::new(&l, r, names()).unwrap();
            let mut probe = back.draft().clone();
            assert_eq!(probe.style_mut().color, Some([200, 30, 30]), "{r:?}");
            assert_eq!(probe.style_mut().weight, Some(50));
            assert_eq!(probe.style_mut().dash, Some(LineStyle::Dashed));
        }
        // It survives the JSON, and a detail from an older file has none.
        let json = serde_json::to_string(&l).unwrap();
        let back: DetailsLayer = serde_json::from_str(&json).unwrap();
        assert_eq!(back, l);
        let mut v: serde_json::Value = serde_json::to_value(l.deck(7).unwrap()).unwrap();
        v.as_object_mut().unwrap().remove("style");
        let old: plan_core::details::DeckPolygon = serde_json::from_value(v).unwrap();
        assert!(old.style.is_default());
    }

    #[test]
    fn the_moldings_panel_edits_the_table_of_the_draft() {
        let mut l = layer();
        let mut d = DetailsDialog::new(&l, DetailRef::Molding(3), names()).unwrap();
        // The old single profile became the first row of the table.
        let Draft::Molding(m) = d.draft() else {
            panic!("a molding")
        };
        assert_eq!(m.table.len(), 1);
        assert_eq!(m.table.rows[0].profile.name, "Crown");
        let drawn = page_texts(&mut d, "Moldings");
        for want in [
            "Extrude Inside Polyline",
            "Reverse Direction",
            "Make Stack",
            "Selected Profile Options",
            "Retain Aspect Ratio",
        ] {
            assert!(drawn.iter().any(|t| t == want), "{want} in {drawn:?}");
        }
        // The buttons: add a library profile, offset and stack it.
        let Form { draft, panel, .. } = &mut d.form;
        let Draft::Molding(m) = draft else { panic!() };
        let base = builtin_profiles()
            .into_iter()
            .find(|p| p.kind == MoldingType::Base)
            .unwrap();
        assert!(panel.apply(&mut m.table, molding::PanelAction::AddNew(base)));
        m.table.rows[1].h_offset = -0.25;
        panel.mark(0, true);
        assert!(panel.apply(&mut m.table, molding::PanelAction::MakeStack));
        assert!(d.form.error().is_none());
        assert!(d.draft().apply(&mut l));
        let stored = l.molding(3).unwrap();
        assert_eq!(stored.table.len(), 2);
        assert_eq!(stored.table.rows[0].stack, stored.table.rows[1].stack);
        // Two parts, the recessed one 1/4 behind the back line.
        let parts = stored.placed_parts();
        assert_eq!(parts.len(), 2);
        assert!(parts[1].section.iter().any(|p| p.x < -0.2));
        assert!(parts[1].dz > 0.0, "the second sits on the first");
        // A profile of no size is an error.
        let Draft::Molding(m) = d.draft_mut() else {
            panic!()
        };
        m.table.rows[0].width = 0.0;
        assert!(d.form.error().is_some());
    }

    #[test]
    fn the_selected_line_page_sets_lengths_angles_and_heights() {
        let mut l = layer();
        l.moldings[0].polyline = vec![Point::ZERO, Point::new(96.0, 0.0), Point::new(96.0, 48.0)];
        let mut d = DetailsDialog::new(&l, DetailRef::Molding(3), names()).unwrap();
        let drawn = page_texts(&mut d, "Selected Line");
        for want in [
            "3D Length",
            "Angle in XY Plane",
            "Angle from XY Plane",
            "Select Edit Plane",
        ] {
            assert!(drawn.iter().any(|t| t == want), "{want} in {drawn:?}");
        }
        let drawn = page_texts(&mut d, "General");
        for want in [
            "Height from Z=0",
            "Auto Calc Orientation at Twisted Joints",
            "Mitre Molding at Twisted Joints",
            "Mitre Molding If Next Edge Turned Off",
            "Molding on Selected Edge",
        ] {
            assert!(drawn.iter().any(|t| t == want), "{want} in {drawn:?}");
        }
        // Edge 0 rises 24 inches over its 96: 3D length and angles follow.
        let Draft::Molding(m) = d.draft_mut() else {
            panic!()
        };
        assert!(m.set_edge_3d(0, 100.0, 0.0, 14.0));
        assert!(m.is_sloped());
        assert!((m.edge_length_3d(0) - 100.0).abs() < 1e-9);
        let (xy, from) = m.edge_angles(0);
        assert!(xy.abs() < 1e-9 && (from - 14.0).abs() < 1e-9);
        // The edge can come off and go back on.
        assert!(m.set_edge_on(1, false));
        assert!(!m.edge_on(1));
        assert!(d.draft().apply(&mut l));
        assert!(l.molding(3).unwrap().is_sloped());
        assert!(!l.molding(3).unwrap().edge_on(1));
    }

    #[test]
    fn the_corner_board_and_quoin_pages_have_the_field_sets_of_the_manual() {
        let l = layer();
        let mut d = DetailsDialog::new(&l, DetailRef::CornerBoard(1), names()).unwrap();
        let drawn = page_texts(&mut d, "General");
        for want in [
            "Width",
            "Thickness",
            "Set Bottom",
            "Set Top",
            "Recessed To Sheathing Layer",
        ] {
            assert!(drawn.iter().any(|t| t == want), "{want} in {drawn:?}");
        }
        let mut d = DetailsDialog::new(&l, DetailRef::Quoin(2), names()).unwrap();
        let drawn = page_texts(&mut d, "General");
        for want in [
            "Width",
            "Thickness",
            "Quoin Height",
            "Quoin Gap",
            "Style",
            "Swap Start Block",
            "Set Bottom",
            "Set Top",
        ] {
            assert!(drawn.iter().any(|t| t == want), "{want} in {drawn:?}");
        }
        // Components can be listed.
        let Draft::Quoin(q) = d.draft_mut() else {
            panic!()
        };
        q.components.push("Stone block".into());
        let drawn = page_texts(&mut d, "Components");
        assert!(drawn.iter().any(|t| t == "Add Component"), "{drawn:?}");
        let _ = MoldingProfile::Crown;
    }

    #[test]
    fn applying_a_draft_replaces_only_that_object() {
        let mut l = layer();
        let mut d = DetailsDialog::new(&l, DetailRef::Deck(7), names()).unwrap();
        if let Draft::Deck(deck) = d.draft_mut() {
            deck.elevation = 30.0;
            deck.railing = true;
            deck.material = "Wood".into();
        }
        assert!(d.draft().apply(&mut l));
        let deck = l.deck(7).unwrap();
        assert_eq!(deck.elevation, 30.0);
        assert!(deck.railing);
        assert_eq!(l.quoin(2).unwrap().width, 16.0);
        // A deleted object no longer applies.
        l.remove(DetailRef::Deck(7));
        assert!(!d.draft().apply(&mut l));
    }

    #[test]
    fn the_materials_combo_lists_the_library() {
        let d = DetailsDialog::new(&layer(), DetailRef::Quoin(2), names()).unwrap();
        assert!(d.form.materials.len() > BASE_MATERIALS.len());
        assert!(d.form.materials.iter().any(|m| m == "Brick \u{2013} Red"));
    }

    #[test]
    fn the_form_rejects_nonsense() {
        let l = layer();
        let mut d = DetailsDialog::new(&l, DetailRef::CornerBoard(1), names()).unwrap();
        assert!(d.form.error().is_none());
        if let Draft::CornerBoard(b) = d.draft_mut() {
            b.width = 0.0;
        }
        assert!(d.form.error().is_some());
        let mut d = DetailsDialog::new(&l, DetailRef::Region(5), names()).unwrap();
        assert!(d.form.error().is_none());
        if let Draft::Region(r) = d.draft_mut() {
            r.outline = wall_rect(30.0, 30.0, 0.0, 48.0);
        }
        assert!(d.form.error().unwrap().contains("length"));
        let mut d = DetailsDialog::new(&l, DetailRef::Solid(8), names()).unwrap();
        if let Draft::Solid(s) = d.draft_mut() {
            s.kind = SolidKind::Sphere { r: 0.0 };
        }
        assert!(d.form.error().is_some());
        let mut d = DetailsDialog::new(&l, DetailRef::Hatch(6), names()).unwrap();
        if let Draft::Hatch(h) = d.draft_mut() {
            h.scale = 0.0;
        }
        assert!(d.form.error().is_some());
        let mut d = DetailsDialog::new(&l, DetailRef::Deck(7), names()).unwrap();
        if let Draft::Deck(deck) = d.draft_mut() {
            deck.layer = " ".into();
        }
        assert!(d.form.error().unwrap().contains("layer"));
    }
}
