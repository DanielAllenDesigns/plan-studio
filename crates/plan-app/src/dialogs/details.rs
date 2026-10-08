//! Specification dialogs of the Trim tools, Material Region, Wall Hatching,
//! Polygon Shaped Deck and the 3D Solid tools: Corner Board, Quoin, Molding,
//! Material Region, Wall Hatching, Deck and 3D Solid Specification. Every
//! dialog has General, Materials, Line Style (shown, not stored yet) and
//! Layer pages. Opened by a double-click on the object (or right after a
//! solid, wall region or hatch is drawn); the dialog edits a [`Draft`] clone
//! that the tool stores on OK as one undo step.

use super::{
    dis_combo, off, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab,
    PV_ACCENT, PV_FAINT, PV_INK, PV_WALL,
};
use crate::editor::details_view::{material_names, PATTERN_NAMES};
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::details::{
    bounds, wall_rect, CornerBoard, DeckPolygon, DetailRef, DetailsLayer, MaterialRegion,
    MoldingLine, MoldingProfile, Quoin, RegionKind, Solid3d, SolidKind, WallHatch,
};
use plan_core::geometry::Point;
use plan_core::units::fmt_ft_in;
use plan_core::walls::Side;
use plan_core::Id;

const FULL_TABS: &[Tab] = &[
    on("General"),
    on("Materials"),
    off("Line Style"),
    on("Layer"),
];
const HATCH_TABS: &[Tab] = &[on("General"), off("Line Style"), on("Layer")];

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
            _ => FULL_TABS,
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
        Some(Self {
            frame: SpecDialog::new(draft.title(), ("details_spec", draft.title())),
            form: Form {
                draft,
                layers,
                materials,
                fields: Fields::default(),
            },
        })
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
                fields.length_row(ui, "Bottom height", "cb_base", &mut b.base);
                fields.length_row(ui, "Height", "cb_height", &mut b.height);
                ui.add_space(6.0);
                ui.weak(format!("Lumber {:.2} cu ft", b.volume() / 1728.0));
            }
            Draft::Quoin(q) => {
                fields.length_row(ui, "Long block", "q_width", &mut q.width);
                fields.length_row(ui, "Block height", "q_height", &mut q.height);
                fields.length_row(ui, "Depth", "q_depth", &mut q.depth);
                fields.length_row(ui, "Bottom height", "q_base", &mut q.base);
                fields.length_row(ui, "Stack height", "q_total", &mut q.total_height);
                row(ui, "Courses", |ui| {
                    ui.checkbox(&mut q.alternating, "Alternate long and short blocks")
                });
                ui.add_space(6.0);
                ui.weak(format!("{} courses", q.courses()));
            }
            Draft::Molding(m) => {
                row(ui, "Profile", |ui| {
                    let current = m.profile.name();
                    egui::ComboBox::from_id_salt("molding_profile")
                        .selected_text(current)
                        .show_ui(ui, |ui| {
                            for p in [
                                MoldingProfile::Crown,
                                MoldingProfile::Base,
                                MoldingProfile::Chair,
                                MoldingProfile::Casing,
                            ] {
                                let name = p.name();
                                if ui.selectable_label(current == name, name).clicked() {
                                    let (h, w) = p.default_size();
                                    m.height = h;
                                    m.width = w;
                                    m.profile = p;
                                }
                            }
                            if current == "Custom" {
                                let _ = ui.selectable_label(true, "Custom");
                            }
                        });
                });
                fields.length_row(ui, "Height", "m_height", &mut m.height);
                fields.length_row(ui, "Projection", "m_width", &mut m.width);
                fields.length_row(ui, "Bottom height", "m_elev", &mut m.elevation);
                ui.add_space(6.0);
                ui.weak(format!(
                    "Length {}, projects to the left of the drawing direction",
                    fmt_ft_in(m.length())
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

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        self.draft.tabs()
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
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
                m.height > 0.0 && m.width > 0.0,
                "The molding needs a height and a projection",
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
            "Materials" => self.materials_page(ui),
            "Line Style" => {
                section(ui, "Line Style");
                row(ui, "Line style", |ui| {
                    dis_combo(ui, "details_line_style", "Solid");
                });
                ui.weak("Not stored yet");
            }
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
                let s = m.section();
                outline_preview(p, area, &s, PV_WALL, ink);
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
        }
    }

    fn names() -> Vec<String> {
        vec!["Moldings".into(), "Decks".into()]
    }

    #[test]
    fn each_kind_has_general_materials_line_style_and_layer_pages() {
        let l = layer();
        let tabs = |r| DetailsDialog::new(&l, r, names()).unwrap().tab_names();
        for r in [
            DetailRef::CornerBoard(1),
            DetailRef::Quoin(2),
            DetailRef::Molding(3),
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
        assert_eq!(
            tabs(DetailRef::Hatch(6)),
            ["General", "Line Style", "Layer"]
        );
        // Line Style is shown but disabled.
        let d = DetailsDialog::new(&l, DetailRef::Deck(7), names()).unwrap();
        let line = d
            .form
            .draft
            .tabs()
            .iter()
            .find(|t| t.name == "Line Style")
            .unwrap();
        assert!(!line.enabled);
        assert!(DetailsDialog::new(&l, DetailRef::Deck(99), names()).is_none());
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
