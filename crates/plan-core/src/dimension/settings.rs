//! The Dimension Defaults panels beyond the number format (manual pp. 478 to
//! 496, in our own words): General, Setup Automatic, Setup Temporary,
//! Extensions, Layer and the Locate panel of each dimension tool.
//!
//! A saved dimension default set (`DimensionDefaultSet`) keeps one
//! [`DimSetup`] and one [`ToolLocates`] next to its older typed fields; every
//! field here is `#[serde(default)]`, so plans saved before this round load
//! with Chief's defaults.

use super::{AutoString, DimLabelOptions, LocateGroup, ObjectLocate, OpeningLocate, WallLocate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ----- General panel -----

/// How the dimension numbers are rounded when the model is more exact than
/// the number format (manual p. 477).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RoundMethod {
    /// The parts of a string always add up to the whole: each part is the
    /// difference between two rounded positions.
    #[default]
    Grid,
    /// Every part is rounded on its own (legacy).
    Distance,
}

impl RoundMethod {
    pub const ALL: [RoundMethod; 2] = [RoundMethod::Grid, RoundMethod::Distance];

    pub fn label(self) -> &'static str {
        match self {
            RoundMethod::Grid => "Grid Rounding",
            RoundMethod::Distance => "Distance Rounding",
        }
    }
}

/// Where the number sits against its dimension line (General panel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TextPos {
    /// On the line, which breaks behind it. With a second format the primary
    /// number sits above the line and the second beneath it.
    Centered,
    #[default]
    Above,
    Below,
}

impl TextPos {
    pub const ALL: [TextPos; 3] = [TextPos::Centered, TextPos::Above, TextPos::Below];

    pub fn label(self) -> &'static str {
        match self {
            TextPos::Centered => "Centered on Dimension Line",
            TextPos::Above => "Above Dimension Line",
            TextPos::Below => "Below Dimension Line",
        }
    }
}

/// Where the Auto Exterior first line offset is measured from (Setup
/// Automatic panel, "Offset from Wall").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OffsetFrom {
    /// The wall's center line.
    Center,
    /// The nearest edge of the wall's dimension layer (its main layer here).
    #[default]
    DimensionLayer,
    /// The wall's nearest surface.
    Surface,
}

impl OffsetFrom {
    pub const ALL: [OffsetFrom; 3] = [
        OffsetFrom::Center,
        OffsetFrom::DimensionLayer,
        OffsetFrom::Surface,
    ];

    pub fn label(self) -> &'static str {
        match self {
            OffsetFrom::Center => "Center",
            OffsetFrom::DimensionLayer => "Dimension Layer",
            OffsetFrom::Surface => "Surface",
        }
    }
}

/// Which wall layer the temporary dimensions locate (Setup Temporary panel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TempWalls {
    #[default]
    Surfaces,
    DimensionLayer,
}

impl TempWalls {
    pub const ALL: [TempWalls; 2] = [TempWalls::Surfaces, TempWalls::DimensionLayer];

    pub fn label(self) -> &'static str {
        match self {
            TempWalls::Surfaces => "Surfaces",
            TempWalls::DimensionLayer => "Wall Dimension Layer",
        }
    }
}

fn yes() -> bool {
    true
}

fn two() -> u32 {
    2
}

/// Every Dimension Defaults setting that is not the number format, the
/// extension and arrow sizes or the Locate panels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DimSetup {
    // --- General ---
    /// `None` follows the older "text above line" flag.
    pub text_position: Option<TextPos>,
    /// The second format, tolerance, rounding method, rounded-value
    /// indicators and fixed label angle (`position` is filled in from
    /// `text_position` when the format is built).
    pub label: DimLabelOptions,
    /// The last fixed label angle typed (degrees), kept while the label
    /// angle is automatic.
    pub label_angle_value: f64,
    /// A second leader line segment, with its length.
    pub leader_second_segment: bool,
    pub leader_second_length: f64,
    pub leader_arrow: bool,
    /// "Match Dimension", "Arrow", "Tick", "Slash" or "Dot".
    pub leader_arrow_style: String,
    pub leader_arrow_match_size: bool,
    pub leader_arrow_size: f64,
    /// 3D display: extension lines run to the marked object.
    pub extend_extensions_3d: bool,
    pub label_faces_camera: bool,
    /// Layouts: the distance from the marked object a dragged dimension line
    /// snaps to.
    pub snap_line_separation: f64,
    /// Layouts: how far a manual dimension reaches for objects to locate
    /// (24 inches in a plan, 1 inch on a layout page).
    pub layout_reach: f64,
    // --- Setup Automatic: exterior ---
    pub offset_from: OffsetFrom,
    /// How far Auto Exterior reaches for walls set back from the exterior
    /// walls; further ones get their own lines.
    pub exterior_reach: f64,
    /// Smallest room (square feet) Auto Exterior Dimensions generate for.
    pub exterior_min_area: f64,
    pub exterior_height_3d: f64,
    pub exterior_vertical_labels: bool,
    /// The overall string.
    pub exterior_overall: bool,
    /// The inner strings (up to three rows per wall in all).
    pub exterior_inner: bool,
    pub exterior_auto_refresh: bool,
    // --- Setup Automatic: room ---
    pub room_min_area: f64,
    pub room_height_3d: f64,
    pub room_vertical_labels: bool,
    pub room_overall: bool,
    /// A second string further from the walls (objects as the Locate Auto
    /// Room panel says).
    pub room_outer: bool,
    pub room_auto_refresh: bool,
    pub room_allow_duplicates: bool,
    /// Room dimensions run inside the room (otherwise outside it).
    pub room_inside: bool,
    // --- Setup Automatic: elevation ---
    pub elevation_overall: bool,
    pub elevation_outer: bool,
    pub elevation_auto_refresh: bool,
    pub elevation_left: bool,
    pub elevation_right: bool,
    pub elevation_top: bool,
    pub elevation_bottom: bool,
    // --- Setup Temporary ---
    pub temp_row_limit: u32,
    pub temp_reach: f64,
    pub temp_walls: TempWalls,
    pub temp_exterior_primary: bool,
    pub temp_exterior_secondary: bool,
    pub temp_interior_primary: bool,
    pub temp_interior_secondary: bool,
    pub temp_interior_centers: bool,
    pub temp_inside_cad: bool,
    pub temp_inside_terrain: bool,
    // --- Extensions: centerline ---
    pub auto_mark_centerlines: bool,
    pub centerline_same_angle: bool,
    pub centerline_offset: f64,
    // --- Layer ---
    /// The layers each kind of dimension is drawn on; empty is Chief's
    /// `Dimensions, Manual` and so on.
    pub layer_manual: String,
    pub layer_automatic: String,
    // --- Story pole ---
    pub pole: PoleSetup,
}

impl Default for DimSetup {
    fn default() -> Self {
        Self {
            text_position: None,
            label: DimLabelOptions::default(),
            label_angle_value: 0.0,
            leader_second_segment: false,
            leader_second_length: 12.0,
            leader_arrow: false,
            leader_arrow_style: "Match Dimension".into(),
            leader_arrow_match_size: true,
            leader_arrow_size: 6.0,
            extend_extensions_3d: true,
            label_faces_camera: false,
            snap_line_separation: 1.0,
            layout_reach: 1.0,
            offset_from: OffsetFrom::DimensionLayer,
            exterior_reach: 48.0,
            exterior_min_area: 0.0,
            exterior_height_3d: 36.0,
            exterior_vertical_labels: false,
            exterior_overall: true,
            exterior_inner: true,
            exterior_auto_refresh: false,
            room_min_area: 0.0,
            room_height_3d: 36.0,
            room_vertical_labels: false,
            room_overall: true,
            room_outer: false,
            room_auto_refresh: false,
            room_allow_duplicates: false,
            room_inside: true,
            elevation_overall: true,
            elevation_outer: false,
            elevation_auto_refresh: false,
            elevation_left: true,
            elevation_right: false,
            elevation_top: false,
            elevation_bottom: false,
            temp_row_limit: two(),
            temp_reach: 48.0,
            temp_walls: TempWalls::Surfaces,
            temp_exterior_primary: yes(),
            temp_exterior_secondary: false,
            temp_interior_primary: yes(),
            temp_interior_secondary: yes(),
            temp_interior_centers: false,
            temp_inside_cad: yes(),
            temp_inside_terrain: yes(),
            auto_mark_centerlines: yes(),
            centerline_same_angle: yes(),
            centerline_offset: 3.0,
            layer_manual: String::new(),
            layer_automatic: String::new(),
            pole: PoleSetup::default(),
        }
    }
}

impl DimSetup {
    /// The label options with the text position resolved (`above` is the
    /// older "text above line" flag).
    pub fn label_options(&self, above: bool) -> DimLabelOptions {
        DimLabelOptions {
            position: self.position(above),
            ..self.label
        }
    }

    /// The text position in force: the explicit choice, else `above`.
    pub fn position(&self, above: bool) -> TextPos {
        self.text_position.unwrap_or(if above {
            TextPos::Above
        } else {
            TextPos::Centered
        })
    }
}

/// The Auto Story Pole Dimension Defaults dialog (manual pp. 493 to 496).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PoleSetup {
    pub left: bool,
    /// How far across the building the left pole reaches for marks, percent.
    pub left_reach: u32,
    pub right: bool,
    pub right_reach: u32,
    pub line_separation: f64,
    pub first_line_offset: f64,
    /// The inner string (floor to plate and so on).
    pub inner: bool,
    /// The outer string between the elevation markers.
    pub between_markers: bool,
    pub primary_ridges_only: bool,
    pub primary_heights_only: bool,
    /// Marks the pole locates: the names of the Locate Elevations panel.
    pub marks: Vec<PoleMark>,
}

impl Default for PoleSetup {
    fn default() -> Self {
        Self {
            left: true,
            left_reach: 100,
            right: false,
            right_reach: 100,
            line_separation: 12.0,
            first_line_offset: 24.0,
            inner: true,
            between_markers: true,
            primary_ridges_only: true,
            primary_heights_only: true,
            marks: PoleMark::default_marks(),
        }
    }
}

/// One entry of the Locate Elevations panel: a kind of elevation mark the
/// pole locates, its displayed name and whether it is on the outer string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PoleMark {
    pub kind: MarkKind,
    /// The name shown beside the mark; empty is the kind's own.
    pub name: String,
    pub outer: bool,
}

impl PoleMark {
    pub fn new(kind: MarkKind, outer: bool) -> Self {
        Self {
            kind,
            name: String::new(),
            outer,
        }
    }

    pub fn display(&self) -> &str {
        if self.name.is_empty() {
            self.kind.label()
        } else {
            &self.name
        }
    }

    /// Chief's default list: the marks story poles start with.
    pub fn default_marks() -> Vec<PoleMark> {
        vec![
            PoleMark::new(MarkKind::TopOfSubfloor, true),
            PoleMark::new(MarkKind::TopOfPlate, true),
            PoleMark::new(MarkKind::Ceiling, false),
            PoleMark::new(MarkKind::Eave, false),
            PoleMark::new(MarkKind::Ridge, true),
        ]
    }
}

/// A kind of elevation a story pole or an elevation dimension can locate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MarkKind {
    /// The first floor's finished floor, the datum.
    Grade,
    TopOfSubfloor,
    TopOfPlate,
    Ceiling,
    /// The low edge of a roof plane.
    Eave,
    Ridge,
    /// Door and window sill and head.
    OpeningSill,
    OpeningHead,
}

impl MarkKind {
    pub const ALL: [MarkKind; 8] = [
        MarkKind::Grade,
        MarkKind::TopOfSubfloor,
        MarkKind::TopOfPlate,
        MarkKind::Ceiling,
        MarkKind::Eave,
        MarkKind::Ridge,
        MarkKind::OpeningSill,
        MarkKind::OpeningHead,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MarkKind::Grade => "Grade",
            MarkKind::TopOfSubfloor => "Top of Subfloor",
            MarkKind::TopOfPlate => "Top of Plate",
            MarkKind::Ceiling => "Ceiling",
            MarkKind::Eave => "Eave",
            MarkKind::Ridge => "Ridge",
            MarkKind::OpeningSill => "Sill",
            MarkKind::OpeningHead => "Head",
        }
    }
}

// ----- Locate panels -----

/// The dimension tool a Locate panel belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LocateTool {
    /// Manual, Baseline and Running dimensions.
    Manual,
    /// End to End and Point to Point dimensions.
    EndToEnd,
    Centerline,
    Interior,
    AutoExterior,
    AutoRoom,
    AutoElevation,
    /// The marks vertical manual dimensions locate in elevations and
    /// sections.
    Elevations,
}

impl LocateTool {
    pub const ALL: [LocateTool; 8] = [
        LocateTool::Manual,
        LocateTool::EndToEnd,
        LocateTool::Centerline,
        LocateTool::Interior,
        LocateTool::AutoExterior,
        LocateTool::AutoRoom,
        LocateTool::AutoElevation,
        LocateTool::Elevations,
    ];

    /// The panel's name in the dialog.
    pub fn label(self) -> &'static str {
        match self {
            LocateTool::Manual => "Locate Manual",
            LocateTool::EndToEnd => "Locate End to End",
            LocateTool::Centerline => "Locate Centerline",
            LocateTool::Interior => "Locate Interior",
            LocateTool::AutoExterior => "Locate Auto Exterior",
            LocateTool::AutoRoom => "Locate Auto Room",
            LocateTool::AutoElevation => "Locate Auto Elevation",
            LocateTool::Elevations => "Locate Elevations",
        }
    }

    fn key(self) -> &'static str {
        match self {
            LocateTool::Manual => "manual",
            LocateTool::EndToEnd => "end_to_end",
            LocateTool::Centerline => "centerline",
            LocateTool::Interior => "interior",
            LocateTool::AutoExterior => "auto_exterior",
            LocateTool::AutoRoom => "auto_room",
            LocateTool::AutoElevation => "auto_elevation",
            LocateTool::Elevations => "elevations",
        }
    }
}

/// The marks of the Locate panels, by category: `(category, [(key, label,
/// default)])`. A mark's key is `<category key>.<key>`.
pub const LOCATE_MARKS: &[(&str, &str, &[(&str, &str, bool)])] = &[
    (
        "cabinets",
        "Cabinets",
        &[
            ("sides", "Sides", true),
            ("corners", "Corners", false),
            ("centers", "Centers", false),
            ("moldings", "Moldings", false),
            ("countertop", "Countertop", false),
            ("backsplash", "Backsplash", false),
            ("toe_kick", "Toe Kick", false),
            ("openings", "Openings", false),
            ("doors", "Doors/Drawers/Panels", false),
        ],
    ),
    (
        "fixtures",
        "Fixtures/Appliances",
        &[("sides", "Sides/Corners", true), ("centers", "Centers", false)],
    ),
    (
        "furniture",
        "Furniture",
        &[("sides", "Sides/Corners", true), ("centers", "Centers", false)],
    ),
    (
        "openings",
        "Openings",
        &[
            ("casing", "Casing", false),
            ("centers", "Centers", false),
            ("rough", "Rough Opening", false),
            ("sides", "Sides", true),
        ],
    ),
    (
        "cad",
        "CAD Objects",
        &[
            ("lines", "Line/Sides", true),
            ("ends", "Ends/Corners", true),
            ("callouts", "Callouts/Markers", false),
            ("clip_lines", "Clip Lines", false),
            ("text", "Text", false),
            ("construction", "Construction Lines", false),
        ],
    ),
    (
        "solids",
        "3D Solids",
        &[
            ("sides", "Sides", true),
            ("corners", "Corners", false),
            ("centers", "Centers", false),
        ],
    ),
    (
        "framing",
        "Framing",
        &[
            ("single", "Single Side", false),
            ("both", "Both Sides", true),
            ("centers", "Centers", false),
        ],
    ),
    (
        "electrical",
        "Electrical",
        &[
            ("lights", "Lights", true),
            ("outlets", "Outlets", true),
            ("switches", "Switches", true),
            ("other", "Other", false),
        ],
    ),
    (
        "other",
        "Other Objects",
        &[
            ("blocks", "Architectural Blocks", false),
            ("plants", "Plants and Images", false),
            ("newel_centers", "Newel Centers", false),
            ("newel_sides", "Newel Sides", false),
        ],
    ),
];

/// The default of mark `key` (`<category>.<mark>`) in [`LOCATE_MARKS`].
pub fn mark_default(key: &str) -> bool {
    let Some((cat, mark)) = key.split_once('.') else {
        return false;
    };
    LOCATE_MARKS
        .iter()
        .find(|(c, _, _)| *c == cat)
        .and_then(|(_, _, ms)| ms.iter().find(|(m, _, _)| *m == mark))
        .is_some_and(|(_, _, d)| *d)
}

/// One Locate panel: what a dimension tool locates (manual pp. 484 to 489).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolLocate {
    /// Walls by surface or main layer (`MainLayer` is the wall's dimension
    /// layer), openings by sides or centers, cabinets and fixtures.
    pub group: LocateGroup,
    /// Walls are not located at all (the panel's "None").
    pub walls_none: bool,
    /// Exterior walls: the exterior (primary) and interior (secondary) side.
    pub exterior_primary: bool,
    pub exterior_secondary: bool,
    /// Interior walls: the primary and secondary side and the center.
    pub interior_primary: bool,
    pub interior_secondary: bool,
    pub interior_centers: bool,
    pub wall_steps: bool,
    pub brick_ledge_lines: bool,
    /// Report the thickness of walls a line crosses.
    pub display_wall_widths: bool,
    /// Per-category marks, `<category>.<mark>` to on or off; a missing key
    /// is the default of [`LOCATE_MARKS`].
    pub marks: BTreeMap<String, bool>,
    /// The Outer and Inner string lists of Auto Room and Auto Elevation.
    pub outer: Vec<MarkKind>,
    pub inner: Vec<MarkKind>,
}

impl Default for ToolLocate {
    fn default() -> Self {
        Self {
            group: LocateGroup {
                walls: WallLocate::MainLayer,
                openings: OpeningLocate::Sides,
                cabinets: ObjectLocate::Sides,
                fixtures: ObjectLocate::Sides,
            },
            walls_none: false,
            exterior_primary: true,
            exterior_secondary: false,
            interior_primary: true,
            interior_secondary: false,
            interior_centers: false,
            wall_steps: false,
            brick_ledge_lines: false,
            display_wall_widths: true,
            marks: BTreeMap::new(),
            outer: Vec::new(),
            inner: Vec::new(),
        }
    }
}

impl ToolLocate {
    /// Is `<category>.<mark>` located? (`key` like `"cabinets.centers"`.)
    pub fn mark(&self, key: &str) -> bool {
        self.marks
            .get(key)
            .copied()
            .unwrap_or_else(|| mark_default(key))
    }

    pub fn set_mark(&mut self, key: &str, on: bool) {
        if mark_default(key) == on {
            self.marks.remove(key);
        } else {
            self.marks.insert(key.to_string(), on);
        }
    }

    /// Are the centers of cabinets located (the Centerline panel's way)?
    pub fn cabinet_centers(&self) -> bool {
        self.mark("cabinets.centers")
    }

    /// Do openings locate their rough opening instead of their nominal
    /// sides?
    pub fn rough_openings(&self) -> bool {
        self.mark("openings.rough") && !self.mark("openings.sides")
    }

    /// Do openings locate the outer edges of their casing?
    pub fn casing_edges(&self) -> bool {
        self.mark("openings.casing") && !self.mark("openings.sides")
    }
}

/// The Locate panels the set has edited. A tool without an entry follows the
/// Manual panel, the way Chief's panels start as copies of it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolLocates {
    /// The Manual panel's extras (its group lives in the set's older typed
    /// fields: `locate_walls`, `locate_openings`, `locate_cabinets`,
    /// `locate_fixtures`).
    pub manual: Option<ToolLocate>,
    panels: BTreeMap<String, ToolLocate>,
}

impl ToolLocates {
    /// The panel the set has stored for `tool`, if it edited it.
    pub fn stored(&self, tool: LocateTool) -> Option<&ToolLocate> {
        if tool == LocateTool::Manual {
            return self.manual.as_ref();
        }
        self.panels.get(tool.key())
    }

    pub fn store(&mut self, tool: LocateTool, panel: ToolLocate) {
        if tool == LocateTool::Manual {
            self.manual = Some(panel);
        } else {
            self.panels.insert(tool.key().to_string(), panel);
        }
    }

    /// Forgets the panel's edits (it follows Manual again).
    pub fn reset(&mut self, tool: LocateTool) {
        if tool == LocateTool::Manual {
            self.manual = None;
        } else {
            self.panels.remove(tool.key());
        }
    }
}

impl crate::defaults::DimensionDefaults {
    /// The Locate panel of `tool` as the tools read it. Manual is the set's
    /// own typed fields plus its stored extras; a tool without a panel of its
    /// own follows Manual, except the ones whose nature differs: Centerline
    /// locates centers, Interior and Auto Room the interior surfaces, and
    /// Elevations the elevation group.
    pub fn tool_locate(&self, tool: LocateTool) -> ToolLocate {
        let mut manual = self.locates.manual.clone().unwrap_or_default();
        manual.group = self.main_locate();
        if tool == LocateTool::Manual {
            return manual;
        }
        if let Some(p) = self.locates.stored(tool) {
            return p.clone();
        }
        let mut t = manual;
        match tool {
            LocateTool::Manual | LocateTool::EndToEnd | LocateTool::AutoExterior => {}
            LocateTool::Centerline => {
                t.group.walls = WallLocate::Centers;
                t.group.openings = OpeningLocate::Centers;
                t.marks.insert("cabinets.centers".into(), true);
                t.marks.insert("fixtures.centers".into(), true);
                // A centerline dimension locates centers, not sides.
                t.marks.insert("cabinets.sides".into(), false);
                t.marks.insert("fixtures.sides".into(), false);
            }
            LocateTool::Interior | LocateTool::AutoRoom => {
                t.display_wall_widths = false;
                t.interior_secondary = false;
                if self.interior_locates_interior_surfaces {
                    t.group.walls = WallLocate::Surfaces;
                }
            }
            LocateTool::AutoElevation | LocateTool::Elevations => {
                t.group = self.elevation_group();
            }
        }
        t
    }

    /// Stores the Locate panel of `tool`. Manual and the older typed fields
    /// stay in step.
    pub fn set_tool_locate(&mut self, tool: LocateTool, panel: ToolLocate) {
        if tool == LocateTool::Manual {
            self.locate_walls = panel.group.walls;
            self.set_opening_locate(panel.group.openings);
            self.locate_cabinets = panel.group.cabinets;
            self.locate_fixtures = panel.group.fixtures;
        } else if tool == LocateTool::AutoElevation {
            self.elevation_locate = Some(panel.group);
        }
        self.locates.store(tool, panel);
    }
}

/// The kind of view a dimension is drawn in: Dimension Defaults keep what to
/// locate and how far to reach for each (manual pp. 478 to 489).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DimView {
    Plan,
    Elevation,
    Section,
    Layout,
}

impl DimView {
    pub const ALL: [DimView; 4] = [
        DimView::Plan,
        DimView::Elevation,
        DimView::Section,
        DimView::Layout,
    ];
}

impl crate::defaults::DimensionDefaults {
    /// The Locate panel a manual dimension in `view` reads: Manual in a plan
    /// and on a layout page, Elevations in an elevation or a section.
    pub fn locate_for_view(&self, view: DimView) -> ToolLocate {
        match view {
            DimView::Plan | DimView::Layout => self.tool_locate(LocateTool::Manual),
            DimView::Elevation | DimView::Section => self.tool_locate(LocateTool::Elevations),
        }
    }

    /// How far a manual dimension reaches for objects in `view`, plan
    /// inches (the layout figure is paper inches).
    pub fn reach_for_view(&self, view: DimView) -> f64 {
        match view {
            DimView::Layout => self.setup.layout_reach.max(0.0),
            _ if self.reach > 0.0 => self.reach,
            _ => 24.0,
        }
    }
}

/// One Auto Exterior string a setup may switch off: `Overall` follows the
/// Overall Dimension check box and the inner ones the Inner Dimension box.
pub fn exterior_strings_for(base: &[AutoString], setup: &DimSetup) -> Vec<AutoString> {
    base.iter()
        .copied()
        .filter(|s| match s {
            AutoString::Overall => setup.exterior_overall,
            _ => setup.exterior_inner,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defaults::PlanDefaults;

    #[test]
    fn marks_default_from_the_table_and_store_only_changes() {
        let mut t = ToolLocate::default();
        assert!(t.mark("cabinets.sides"));
        assert!(!t.mark("cabinets.centers"));
        assert!(!t.mark("nothing.at_all"));
        t.set_mark("cabinets.centers", true);
        assert!(t.mark("cabinets.centers"));
        assert_eq!(t.marks.len(), 1);
        t.set_mark("cabinets.centers", false);
        assert!(t.marks.is_empty(), "back at the default: nothing stored");
        t.set_mark("openings.rough", true);
        assert!(!t.rough_openings(), "sides still on");
        t.set_mark("openings.sides", false);
        assert!(t.rough_openings());
    }

    #[test]
    fn a_tool_without_a_panel_follows_manual_but_keeps_its_nature() {
        let mut d = PlanDefaults::default().dimensions;
        d.locate_walls = WallLocate::Surfaces;
        let manual = d.tool_locate(LocateTool::Manual);
        assert_eq!(manual.group.walls, WallLocate::Surfaces);
        assert_eq!(
            d.tool_locate(LocateTool::EndToEnd).group.walls,
            WallLocate::Surfaces
        );
        let c = d.tool_locate(LocateTool::Centerline);
        assert_eq!(c.group.walls, WallLocate::Centers);
        assert_eq!(c.group.openings, OpeningLocate::Centers);
        assert!(c.cabinet_centers());
        assert!(!c.mark("cabinets.sides") && !c.mark("fixtures.sides"));
        let i = d.tool_locate(LocateTool::Interior);
        assert!(!i.display_wall_widths);
        // Editing a panel stores it; Manual writes the typed fields.
        let mut p = d.tool_locate(LocateTool::AutoRoom);
        p.group.openings = OpeningLocate::Sides;
        d.set_tool_locate(LocateTool::AutoRoom, p);
        assert_eq!(
            d.tool_locate(LocateTool::AutoRoom).group.openings,
            OpeningLocate::Sides
        );
        assert_eq!(
            d.opening_locate(),
            OpeningLocate::Centers,
            "Manual keeps its own"
        );
        let mut m = d.tool_locate(LocateTool::Manual);
        m.group.walls = WallLocate::Centers;
        m.group.openings = OpeningLocate::None;
        d.set_tool_locate(LocateTool::Manual, m);
        assert_eq!(d.locate_walls, WallLocate::Centers);
        assert_eq!(d.opening_locate(), OpeningLocate::None);
    }

    #[test]
    fn each_view_reads_its_own_locate_panel_and_reach() {
        let mut d = PlanDefaults::default().dimensions;
        d.locate_walls = WallLocate::Surfaces;
        assert_eq!(
            d.locate_for_view(DimView::Plan).group.walls,
            WallLocate::Surfaces
        );
        assert_eq!(
            d.locate_for_view(DimView::Layout).group.walls,
            WallLocate::Surfaces
        );
        // Elevations and sections follow the elevation group until edited.
        let mut e = d.tool_locate(LocateTool::Elevations);
        e.group.walls = WallLocate::Centers;
        d.set_tool_locate(LocateTool::Elevations, e);
        assert_eq!(
            d.locate_for_view(DimView::Section).group.walls,
            WallLocate::Centers
        );
        assert_eq!(
            d.locate_for_view(DimView::Elevation).group.walls,
            WallLocate::Centers
        );
        assert_eq!(d.reach_for_view(DimView::Plan), 24.0);
        assert_eq!(d.reach_for_view(DimView::Layout), 1.0);
        d.reach = 36.0;
        d.setup.layout_reach = 2.0;
        assert_eq!(d.reach_for_view(DimView::Plan), 36.0);
        assert_eq!(d.reach_for_view(DimView::Layout), 2.0);
        assert_eq!(DimView::ALL.len(), 4);
    }

    #[test]
    fn setup_defaults_are_chiefs_and_old_files_load() {
        let s = DimSetup::default();
        assert_eq!(s.label.rounding, RoundMethod::Grid);
        assert_eq!(s.temp_row_limit, 2);
        assert_eq!(s.position(true), TextPos::Above);
        assert_eq!(s.position(false), TextPos::Centered);
        let loaded: DimSetup = serde_json::from_str("{}").unwrap();
        assert_eq!(loaded, s);
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<DimSetup>(&json).unwrap(), s);
        let strings = exterior_strings_for(
            &[
                AutoString::Openings,
                AutoString::WallToWall,
                AutoString::Overall,
            ],
            &DimSetup {
                exterior_overall: false,
                ..DimSetup::default()
            },
        );
        assert_eq!(strings, vec![AutoString::Openings, AutoString::WallToWall]);
    }
}
