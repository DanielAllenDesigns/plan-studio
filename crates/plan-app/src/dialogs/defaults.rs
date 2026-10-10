//! Default Settings (Edit > Default Settings...): a searchable tree whose
//! leaves open the same specification dialogs objects use. The Preferences >
//! Templates leaf opens the Templates page: the default plan and layout
//! template paths, the seeding switch and what was decoded from them.

use crate::editor::EditorContext;
use crate::templates::{self, SeedCache, TemplateSettings};
use eframe::egui::{self, Align, Align2, Key, Layout, Modifiers};
use plan_core::defaults::saved::SavedKind;
use plan_core::openings::types::DefaultKey;
use plan_core::{OpeningKind, OpeningStyle};
use std::cell::RefCell;
use std::path::PathBuf;

/// A leaf of the Default Settings tree.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DefaultsEntry {
    ExteriorWall,
    InteriorWall,
    FoundationWall,
    InteriorDoor,
    ExteriorDoor,
    Window,
    /// The Defaults dialog of one other door or window type (Double Door,
    /// Pocket Door, Casement Window, Bay Window...: manual pp. 103, 603).
    OpeningType(DefaultKey),
    /// Opened from the Saved Defaults dialog of Manual Dimensions (its Edit
    /// button), not from a leaf of its own any more.
    #[allow(dead_code)]
    Dimensions,
    RoomTypes,
    /// Floors and Rooms > Floor Defaults (R-56).
    FloorDefaults,
    /// Foundation > Foundation: the Foundation Defaults dialog (R-129).
    Foundation,
    TextStyles,
    /// Preferences > Templates (the Templates page window).
    Templates,
}

/// A leaf of the tree: a dialog `main` opens for the entry, or a page this
/// module draws itself.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Leaf {
    Entry(DefaultsEntry),
    /// Roofs > Roof Defaults (RF-14, RF-15, RF-28, RF-31).
    RoofDefaults,
    /// Cabinets: the Cabinet Defaults dialog on the tab at this index.
    CabinetDefaults(usize),
    /// Framing: the Framing Defaults dialog on the tab at this index.
    FramingDefaults(usize),
    /// Terrain > Terrain Defaults (`default_settings_terrain.rs`).
    TerrainDefaults,
    /// A generic page of `default_pages` (its id).
    Page(&'static str),
    /// Electrical (`default_pages/electrical.rs`): the plan's device heights.
    Electrical,
    /// Floors and Rooms > Floor/Ceiling Platform (`assembly_def.rs`): the
    /// plan-wide layered Floor and Ceiling Structure and Finish.
    Platforms,
    /// A command of the application (an existing dialog or tool window).
    Run(Action),
}

use crate::toolbar::Action;

/// The cabinet kinds: tab indexes of the Cabinet Defaults dialog.
const CABINET_LEAVES: &[(&str, Leaf)] = &[
    ("Base Cabinet", Leaf::CabinetDefaults(0)),
    ("Wall Cabinet", Leaf::CabinetDefaults(1)),
    ("Full Height Cabinet", Leaf::CabinetDefaults(2)),
    ("Soffit", Leaf::CabinetDefaults(3)),
    ("Shelf", Leaf::CabinetDefaults(4)),
    ("Partition", Leaf::CabinetDefaults(5)),
    ("Countertop", Leaf::CabinetDefaults(6)),
    ("Backsplash", Leaf::CabinetDefaults(7)),
    // The General Cabinet Defaults dialog (`cabinet_defaults.rs`); it opens
    // from here only.
    (
        "General Cabinet",
        Leaf::CabinetDefaults(GENERAL_CABINET_LEAF),
    ),
];

/// The index of the General Cabinet leaf among the cabinet leaves; it opens
/// its own dialog instead of a tab of the Cabinet Defaults.
const GENERAL_CABINET_LEAF: usize = 99;

/// Chief's Default Settings tree: 29 groups. A leaf opens the same
/// specification dialog an object shows, an existing window, or a page of
/// `default_pages`.
const TREE: &[(&str, &[(&str, Leaf)])] = &[
    ("3D Solid", &[("3D Solid", Leaf::Page("solid3d"))]),
    (
        "3D View Defaults",
        &[(
            "3D View Defaults",
            Leaf::Run(Action::View3d(
                crate::shell::view3d_panel::View3dCommand::Defaults,
            )),
        )],
    ),
    ("Cabinets", CABINET_LEAVES),
    (
        "CAD",
        &[
            ("General CAD", Leaf::Page("cad.general")),
            ("Arcs", Leaf::Page("cad.arcs")),
            ("Boxes", Leaf::Page("cad.boxes")),
            ("Circles", Leaf::Page("cad.circles")),
            ("Lines", Leaf::Page("cad.lines")),
            ("Polylines", Leaf::Page("cad.polylines")),
            ("Splines", Leaf::Page("cad.splines")),
            (
                "Revision Clouds",
                Leaf::Run(Action::Custom(super::saved_defaults::command_id(
                    SavedKind::RevisionClouds,
                ))),
            ),
        ],
    ),
    (
        "Camera Tools",
        &[
            ("Full Camera", Leaf::Page("camera.full")),
            ("Floor Camera", Leaf::Page("camera.floor")),
            ("Doll House", Leaf::Page("camera.doll_house")),
            ("Glass House", Leaf::Page("camera.glass_house")),
            ("Overview", Leaf::Page("camera.overview")),
            ("Elevation", Leaf::Page("camera.elevation")),
            ("Section", Leaf::Page("camera.section")),
            ("Backclipped", Leaf::Page("camera.backclipped")),
            ("Wall Elevation", Leaf::Page("camera.wall_elevation")),
        ],
    ),
    ("Corner Trim", &[("Corner Trim", Leaf::Page("corner_trim"))]),
    (
        "Default Sets",
        &[(
            "Default Sets",
            Leaf::Run(Action::Custom(super::default_sets::DEFAULT_SETS)),
        )],
    ),
    (
        "Dimension",
        &[
            (
                "Dimensions",
                Leaf::Run(Action::Custom(super::saved_defaults::command_id(
                    SavedKind::ManualDimensions,
                ))),
            ),
            ("General", Leaf::Page("dimension.general")),
            ("Setup Automatic", Leaf::Page("dimension.setup_automatic")),
            ("Setup Temporary", Leaf::Page("dimension.setup_temporary")),
            ("Secondary Format", Leaf::Page("dimension.secondary")),
            (
                "Extensions: Centerlines",
                Leaf::Page("dimension.centerlines"),
            ),
            ("Layer", Leaf::Page("dimension.layer")),
            ("Locate Manual", Leaf::Page("dimension.locate_manual")),
            (
                "Locate End to End",
                Leaf::Page("dimension.locate_end_to_end"),
            ),
            (
                "Locate Centerline",
                Leaf::Page("dimension.locate_centerline"),
            ),
            ("Locate Interior", Leaf::Page("dimension.locate_interior")),
            (
                "Locate Auto Exterior",
                Leaf::Page("dimension.locate_auto_exterior"),
            ),
            ("Locate Auto Room", Leaf::Page("dimension.locate_auto_room")),
            (
                "Locate Auto Elevation",
                Leaf::Page("dimension.locate_auto_elevation"),
            ),
            (
                "Locate Elevations",
                Leaf::Page("dimension.locate_elevations"),
            ),
            (
                "Auto Story Pole Dimensions",
                Leaf::Page("dimension.auto_story_pole"),
            ),
            (
                "Story Pole: Locate Elevations",
                Leaf::Page("dimension.pole_elevations"),
            ),
        ],
    ),
    (
        "Distributed Objects",
        &[("Distributed Objects", Leaf::Page("distributed"))],
    ),
    (
        "Doors",
        &[
            ("Interior Door", Leaf::Entry(DefaultsEntry::InteriorDoor)),
            ("Exterior Door", Leaf::Entry(DefaultsEntry::ExteriorDoor)),
            ("Garage Door", Leaf::Page("garage_door")),
            ("Double Door", door(OpeningStyle::DoubleDoor, false)),
            ("Doorway", door(OpeningStyle::Doorway, false)),
            ("Interior Sliding Door", door(OpeningStyle::Sliding, false)),
            ("Exterior Sliding Door", door(OpeningStyle::Sliding, true)),
            ("Pocket Door", door(OpeningStyle::Pocket, false)),
            ("Bifold Door", door(OpeningStyle::Bifold, false)),
            ("Barn Door", door(OpeningStyle::Barn, false)),
            ("Fixed Door", door(OpeningStyle::Fixed, false)),
            ("Shower Door", door(OpeningStyle::Shower, false)),
        ],
    ),
    ("Dormer", &[("Dormer", Leaf::Page("dormer"))]),
    ("Electrical", &[("Electrical", Leaf::Electrical)]),
    (
        "Floors and Rooms",
        &[
            ("Floor Defaults", Leaf::Entry(DefaultsEntry::FloorDefaults)),
            ("Floor Levels", Leaf::Page("floor_levels")),
            ("Floor/Ceiling Platform", Leaf::Platforms),
            ("Room Types", Leaf::Entry(DefaultsEntry::RoomTypes)),
            (
                "Room Functions",
                Leaf::Run(Action::Custom(super::saved_defaults::command_id(
                    SavedKind::RoomFunctions,
                ))),
            ),
            ("Rooms", Leaf::Page("rooms")),
            ("Room Label", Leaf::Page("room_label")),
        ],
    ),
    (
        "Foundation",
        &[
            // The Foundation Defaults dialog (manual p. 738).
            ("Foundation", Leaf::Entry(DefaultsEntry::Foundation)),
            ("Footing and Wall", Leaf::Page("foundation")),
        ],
    ),
    (
        "Framing",
        &[
            ("Framing Defaults", Leaf::FramingDefaults(0)),
            ("Wall Framing", Leaf::FramingDefaults(0)),
            ("Headers", Leaf::FramingDefaults(1)),
            ("Floor Framing", Leaf::FramingDefaults(2)),
            ("Roof Framing", Leaf::FramingDefaults(3)),
            (
                "Framing Types",
                Leaf::Run(Action::Custom(super::saved_defaults::command_id(
                    SavedKind::FramingTypes,
                ))),
            ),
        ],
    ),
    (
        "General",
        &[
            ("General Plan Defaults", Leaf::Page("general")),
            ("Templates", Leaf::Entry(DefaultsEntry::Templates)),
        ],
    ),
    ("Image", &[("Image", Leaf::Page("image"))]),
    ("Layout", &[("Layout", Leaf::Page("layout"))]),
    (
        "Materials",
        &[
            (
                "Materials Defaults",
                Leaf::Run(Action::Custom("materials.defaults")),
            ),
            ("Materials List", Leaf::Page("materials_list")),
            (
                "Structural Member Reporting",
                Leaf::Run(Action::Custom(super::saved_defaults::command_id(
                    SavedKind::StructuralMemberReporting,
                ))),
            ),
            ("Molding Polylines", Leaf::Page("molding_polylines")),
        ],
    ),
    (
        "Plan",
        &[
            ("Plan Defaults", Leaf::Page("plan")),
            (
                "Active Layer Display Options",
                Leaf::Run(Action::OpenLayerDisplay),
            ),
            ("Plan Check", Leaf::Run(Action::Custom("check.settings"))),
            (
                "Watermark",
                Leaf::Run(Action::Custom(super::watermark::DEFAULTS)),
            ),
        ],
    ),
    (
        "Railing and Deck",
        &[("Railing and Deck", Leaf::Page("railing_deck"))],
    ),
    (
        "Roofs",
        &[
            ("Roof Defaults", Leaf::RoofDefaults),
            (
                "Tray Ceiling",
                Leaf::Run(Action::Custom(super::tray_ceiling::DEFAULTS)),
            ),
        ],
    ),
    (
        "Schedules",
        &[
            ("Door Schedule", Leaf::Page("schedules.door")),
            ("Window Schedule", Leaf::Page("schedules.window")),
            ("Room Schedule", Leaf::Page("schedules.room")),
            ("Wall Schedule", Leaf::Page("schedules.wall")),
            ("Cabinet Schedule", Leaf::Page("schedules.cabinet")),
            ("Electrical Schedule", Leaf::Page("schedules.electrical")),
            ("Framing Schedule", Leaf::Page("schedules.framing")),
            ("Fixture Schedule", Leaf::Page("schedules.fixture")),
            ("Furniture Schedule", Leaf::Page("schedules.furniture")),
            ("Plant Schedule", Leaf::Page("schedules.plant")),
            ("Stair Schedule", Leaf::Page("schedules.stair")),
            ("Room Finish Schedule", Leaf::Page("schedules.room_finish")),
            ("Note Schedule", Leaf::Page("schedules.note")),
            ("General Schedule", Leaf::Page("schedules.general")),
        ],
    ),
    ("Slab", &[("Slab", Leaf::Page("slab"))]),
    ("Stairs", &[("Stairs", Leaf::Page("stairs"))]),
    ("Terrain", &[("Terrain Defaults", Leaf::TerrainDefaults)]),
    (
        "Text",
        &[
            ("Text Styles", Leaf::Entry(DefaultsEntry::TextStyles)),
            (
                "Text",
                Leaf::Run(Action::Custom(super::saved_defaults::command_id(
                    SavedKind::Text,
                ))),
            ),
            (
                "Rich Text",
                Leaf::Run(Action::Custom(super::saved_defaults::command_id(
                    SavedKind::RichText,
                ))),
            ),
            (
                "Arrows",
                Leaf::Run(Action::Custom(super::saved_defaults::command_id(
                    SavedKind::Arrows,
                ))),
            ),
            ("Leader Lines", Leaf::Page("text.leader_lines")),
            (
                "Callouts",
                Leaf::Run(Action::Custom(super::saved_defaults::command_id(
                    SavedKind::Callouts,
                ))),
            ),
            (
                "Markers",
                Leaf::Run(Action::Custom(super::saved_defaults::command_id(
                    SavedKind::Markers,
                ))),
            ),
            (
                "Notes",
                Leaf::Run(Action::Custom(super::saved_defaults::command_id(
                    SavedKind::Notes,
                ))),
            ),
        ],
    ),
    (
        "Walls",
        &[
            ("Exterior Wall", Leaf::Entry(DefaultsEntry::ExteriorWall)),
            ("Interior Wall", Leaf::Entry(DefaultsEntry::InteriorWall)),
            (
                "Foundation Wall",
                Leaf::Entry(DefaultsEntry::FoundationWall),
            ),
            ("Railing Wall", Leaf::Page("walls.railing")),
            ("Fence", Leaf::Page("walls.fence")),
            ("Pony Wall", Leaf::Page("walls.pony")),
            ("Half Wall", Leaf::Page("walls.half")),
            ("Glass Wall", Leaf::Page("walls.glass")),
            ("Attic Wall", Leaf::Page("walls.attic")),
        ],
    ),
    (
        "Windows",
        &[
            ("Window", Leaf::Entry(DefaultsEntry::Window)),
            ("Casement Window", window(OpeningStyle::Casement)),
            ("Fixed Window", window(OpeningStyle::Fixed)),
            ("Sliding Window", window(OpeningStyle::SlidingWindow)),
            ("Awning Window", window(OpeningStyle::Awning)),
            ("Hopper Window", window(OpeningStyle::Hopper)),
            ("Bay Window", window(OpeningStyle::BayWindow)),
            ("Bow Window", window(OpeningStyle::BowWindow)),
            ("Box Window", window(OpeningStyle::BoxWindow)),
            ("Pass-Through", window(OpeningStyle::PassThrough)),
            ("Wall Niche", window(OpeningStyle::WallNiche)),
        ],
    ),
];

/// The leaf of a door type's Defaults dialog.
const fn door(style: OpeningStyle, exterior: bool) -> Leaf {
    Leaf::Entry(DefaultsEntry::OpeningType(DefaultKey {
        kind: OpeningKind::Door,
        style,
        exterior,
    }))
}

/// The leaf of a window type's Defaults dialog.
const fn window(style: OpeningStyle) -> Leaf {
    Leaf::Entry(DefaultsEntry::OpeningType(DefaultKey {
        kind: OpeningKind::Window,
        style,
        exterior: false,
    }))
}

/// The groups and leaves of the tree, for the tests and the parity checks.
#[cfg(test)]
pub(crate) fn all_leaves() -> Vec<(&'static str, &'static str, Leaf)> {
    TREE.iter()
        .flat_map(|(g, ls)| ls.iter().map(move |(n, l)| (*g, *n, *l)))
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DefaultsOutcome {
    Open,
    Close,
    /// Edit the chosen leaf (double-click or the Edit button).
    Edit(DefaultsEntry),
    /// Run an application command (a leaf that opens an existing window).
    Run(Action),
}

#[derive(Default)]
pub struct DefaultsDialog {
    search: String,
    selected: Option<Leaf>,
    /// Reset to Template was clicked once; the next click does it.
    confirm_reset: bool,
}

/// What a leaf opens, in a line.
fn describe(leaf: Leaf) -> &'static str {
    match leaf {
        Leaf::Entry(DefaultsEntry::ExteriorWall | DefaultsEntry::InteriorWall) => {
            "The Wall Specification every new wall of this kind starts from."
        }
        Leaf::Entry(DefaultsEntry::FoundationWall) => {
            "The Wall Specification new foundation walls start from."
        }
        Leaf::Entry(DefaultsEntry::InteriorDoor | DefaultsEntry::ExteriorDoor) => {
            "The Door Specification new doors start from."
        }
        Leaf::Entry(DefaultsEntry::Window) => "The Window Specification new windows start from.",
        Leaf::Entry(DefaultsEntry::OpeningType(k)) if k.kind == OpeningKind::Door => {
            "The Door Specification doors of this type start from, and the doors using the default follow."
        }
        Leaf::Entry(DefaultsEntry::OpeningType(_)) => {
            "The Window Specification windows of this type start from, and the windows using the default follow."
        }
        Leaf::Entry(DefaultsEntry::Foundation) => {
            "The Foundation Defaults: footings, stem walls, slabs and piers of new foundations."
        }
        Leaf::Entry(DefaultsEntry::Dimensions) => {
            "The saved dimension default sets: format, automatic dimensions, extensions and arrows."
        }
        Leaf::Entry(DefaultsEntry::RoomTypes) => "The room types and what each one means.",
        Leaf::Entry(DefaultsEntry::FloorDefaults) => {
            "Ceiling height, platforms, finishes and materials of floors built from now on."
        }
        Leaf::Entry(DefaultsEntry::TextStyles) => "The text styles new plans start with.",
        Leaf::Entry(DefaultsEntry::Templates) => {
            "The Chief plan and layout templates new plans start from."
        }
        Leaf::RoofDefaults => "Eave cut, fascia, soffit, rafter tails, attic walls and baseline.",
        Leaf::CabinetDefaults(_) => "The Cabinet Defaults: sizes, styles and hardware.",
        Leaf::FramingDefaults(_) => "The Framing Defaults: members, spacing and headers.",
        Leaf::TerrainDefaults => "The Terrain Specification of this plan.",
        Leaf::Page(_) => "A page of default values for new objects of this kind.",
        Leaf::Electrical => "The height each kind of device is placed at; kept with this plan.",
        Leaf::Platforms => {
            "The layers of the floor and ceiling platforms and their finishes; kept with this plan."
        }
        Leaf::Run(_) => "Opens the matching window.",
    }
}

/// What editing a leaf does: the entries go to `main`, the pages open here.
pub(crate) fn edit_outcome(leaf: Leaf) -> DefaultsOutcome {
    match leaf {
        Leaf::Entry(e) => DefaultsOutcome::Edit(e),
        Leaf::RoofDefaults => {
            open_roof_defaults();
            DefaultsOutcome::Open
        }
        Leaf::CabinetDefaults(GENERAL_CABINET_LEAF) => {
            super::cabinet_defaults::request_open();
            DefaultsOutcome::Open
        }
        Leaf::CabinetDefaults(tab) => {
            OPEN_CABINETS.with(|c| c.set(Some(tab)));
            DefaultsOutcome::Open
        }
        Leaf::FramingDefaults(tab) => {
            OPEN_FRAMING.with(|c| c.set(Some(tab)));
            DefaultsOutcome::Open
        }
        Leaf::TerrainDefaults => {
            super::default_settings_terrain::request_open();
            DefaultsOutcome::Open
        }
        Leaf::Page(id) => {
            super::default_pages::request_open(id);
            DefaultsOutcome::Open
        }
        Leaf::Electrical => {
            super::default_pages::electrical::request_open();
            DefaultsOutcome::Open
        }
        Leaf::Platforms => {
            super::assembly_def::request_page();
            DefaultsOutcome::Open
        }
        Leaf::Run(action) => DefaultsOutcome::Run(action),
    }
}

impl DefaultsDialog {
    pub fn new() -> Self {
        Self::default()
    }

    /// `enabled` is false while a specification dialog is open on top.
    pub fn show(&mut self, ctx: &egui::Context, enabled: bool) -> DefaultsOutcome {
        let mut outcome = DefaultsOutcome::Open;
        let mut open = true;
        let query = self.search.trim().to_lowercase();
        egui::Window::new("Default Settings")
            .id(egui::Id::new("default_settings_dialog"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([520.0, 520.0])
            .min_size([360.0, 320.0])
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Search");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search)
                                .hint_text("Filter the tree")
                                .desired_width(f32::INFINITY),
                        );
                    });
                    ui.separator();
                    let tree_height = (ui.available_height() - 76.0).max(80.0);
                    egui::ScrollArea::vertical()
                        .id_salt("defaults_tree")
                        .max_height(tree_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let mut any = false;
                            for (group, leaves) in TREE {
                                let group_hit =
                                    query.is_empty() || group.to_lowercase().contains(&query);
                                let shown: Vec<_> = leaves
                                    .iter()
                                    .filter(|(name, _)| {
                                        group_hit || name.to_lowercase().contains(&query)
                                    })
                                    .collect();
                                if shown.is_empty() {
                                    continue;
                                }
                                any = true;
                                let mut header = egui::CollapsingHeader::new(*group)
                                    .id_salt(("defaults_group", group))
                                    .default_open(false);
                                if !query.is_empty() {
                                    header = header.open(Some(true));
                                }
                                header.show(ui, |ui| {
                                    for (name, entry) in shown {
                                        let resp = ui.selectable_label(
                                            self.selected == Some(*entry),
                                            *name,
                                        );
                                        if resp.clicked() {
                                            self.selected = Some(*entry);
                                        }
                                        if resp.double_clicked() {
                                            self.selected = Some(*entry);
                                            outcome = edit_outcome(*entry);
                                        }
                                    }
                                });
                            }
                            if !any {
                                ui.weak("No settings match the search");
                            }
                        });
                    ui.separator();
                    match self.selected {
                        Some(leaf) => ui.weak(describe(leaf)),
                        None => ui.weak("Select a setting, then Edit."),
                    };
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            outcome = DefaultsOutcome::Close;
                        }
                        let edit = ui.add_enabled(
                            self.selected.is_some(),
                            egui::Button::new("Edit\u{2026}"),
                        );
                        if edit.clicked() {
                            if let Some(e) = self.selected {
                                outcome = edit_outcome(e);
                            }
                        }
                        let label = if self.confirm_reset {
                            "Click again to reset all"
                        } else {
                            "Reset to Template"
                        };
                        if ui
                            .button(label)
                            .on_hover_text(
                                "Put every default back to the template (the plan's own data is not touched)",
                            )
                            .clicked()
                        {
                            if self.confirm_reset {
                                RESET.with(|c| c.set(true));
                            }
                            self.confirm_reset = !self.confirm_reset;
                        }
                    });
                });
            });
        if !open {
            outcome = DefaultsOutcome::Close;
        }
        if enabled && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            outcome = DefaultsOutcome::Close;
        }
        outcome
    }
}

thread_local! {
    /// Reset to Template was confirmed; [`show_templates_page`] does it.
    static RESET: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Reset to Template: the plan defaults go back to `base` (the embedded
/// template, seeded from the Chief plan template when that is set up). The
/// plan's own data (terrain, electrical heights, framing settings) is not
/// touched, and the per-session edits of the default dialogs are forgotten.
pub fn reset_to_template(cx: &mut EditorContext, base: plan_core::PlanDefaults) {
    cx.defaults = base;
    forget_dialog_edits(cx);
    cx.status = "Reset the default settings to the template".into();
}

/// Command id: Edit > Reset to Defaults.
pub const RESET_TO_DEFAULTS: &str = "defaults.reset";

// ===================================================================
// Preferences > Templates
// ===================================================================

struct TemplatesPage {
    settings: TemplateSettings,
    plan_text: String,
    layout_text: String,
    cache: SeedCache,
    status: String,
}

thread_local! {
    static PAGE: RefCell<Option<TemplatesPage>> = const { RefCell::new(None) };
}

fn path_text(p: &Option<PathBuf>) -> String {
    p.as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn non_empty(s: &str) -> Option<PathBuf> {
    let t = s.trim();
    (!t.is_empty()).then(|| PathBuf::from(t))
}

/// Opens the Templates page window (it is drawn by [`show_templates_page`]).
pub fn open_templates_page() {
    let settings = templates::load_settings();
    let page = TemplatesPage {
        plan_text: path_text(&settings.plan),
        layout_text: path_text(&settings.layout),
        settings,
        cache: SeedCache::load(),
        status: String::new(),
    };
    PAGE.with(|p| *p.borrow_mut() = Some(page));
}

/// Re-reads the settings into the Templates page when it is open (after the
/// Import window changed them).
pub fn reload_templates_page() {
    let is_open = PAGE.with(|p| p.borrow().is_some());
    if is_open {
        open_templates_page();
    }
}

/// Draws the Templates page and the Roof Defaults page if they are open;
/// call once a frame.
pub fn show_templates_page(ctx: &egui::Context, cx: &mut EditorContext) {
    // Objects that use a default follow it when a dialog or Set as Default
    // changed it since the last frame.
    crate::plan_defaults::track(cx);
    super::saved_defaults::show(ctx, cx);
    super::default_sets::show_all(ctx, cx);
    super::template_chooser::show_all(ctx, cx);
    super::import_settings::show_all(ctx, cx);
    show_roof_defaults(ctx, cx);
    show_cabinet_defaults(ctx, cx);
    super::cabinet_defaults::show(ctx, cx);
    show_framing_defaults(ctx, cx);
    super::default_settings_terrain::show(ctx, cx);
    super::default_pages::show(ctx, cx);
    if RESET.with(|c| c.replace(false)) {
        reset_to_template(cx, crate::plan_defaults::template_base().0);
    }
    let Some(mut page) = PAGE.with(|p| p.borrow_mut().take()) else {
        return;
    };
    if page.show(ctx, cx) {
        PAGE.with(|p| {
            let mut slot = p.borrow_mut();
            if slot.is_none() {
                *slot = Some(page);
            }
        });
    }
}

/// Forgets the per-session edits of the default dialogs so they show the
/// new defaults again (what Reset to Chief X18 Template does).
pub fn forget_dialog_edits(cx: &mut EditorContext) {
    use crate::dialogs::{OpeningTarget, WallTarget};
    for key in [
        WallTarget::DefaultExterior.key(),
        WallTarget::DefaultInterior.key(),
        WallTarget::DefaultFoundation.key(),
    ] {
        cx.extras.walls.remove(&key);
    }
    for target in [
        OpeningTarget::DefaultDoor,
        OpeningTarget::DefaultExteriorDoor,
        OpeningTarget::DefaultWindow,
    ]
    .into_iter()
    .chain(
        DefaultKey::doors()
            .into_iter()
            .chain(DefaultKey::windows())
            .map(OpeningTarget::DefaultType),
    ) {
        cx.extras.openings.remove(&target.key());
    }
}

/// Saves `settings`, decodes the templates (`force` re-reads even an
/// unchanged file) and, unless the user saved their own defaults, makes the
/// result the defaults new plans start from. Returns the cache and a status
/// line.
pub fn apply_template_settings(
    cx: &mut EditorContext,
    settings: &TemplateSettings,
    force: bool,
) -> (SeedCache, String) {
    let mut parts: Vec<String> = Vec::new();
    if let Err(e) = templates::save_settings(settings) {
        parts.push(format!("Could not save the settings: {e}"));
    }
    let out = templates::refresh(settings, force);
    parts.extend(out.notes.iter().cloned());
    let own = crate::plan_defaults::user_path().is_some_and(|p| p.exists());
    if own {
        parts.push(
            "Your saved template (defaults.json) takes priority; use File > Templates > Reset to Chief X18 Template to use the Chief template".into(),
        );
    } else {
        cx.defaults =
            templates::defaults_from(settings, &out.cache, crate::plan_defaults::embedded());
        forget_dialog_edits(cx);
        match (&out.cache.plan, settings.seed_from_chief) {
            (Some(p), true) => parts.insert(
                0,
                format!(
                    "Defaults seeded from {} ({} wall types)",
                    p.file_name,
                    p.wall_types.len()
                ),
            ),
            (_, false) => parts.insert(0, "Using the shipped Chief X18 defaults".into()),
            (None, true) => {}
        }
    }
    (out.cache, parts.join("; "))
}

impl TemplatesPage {
    fn current_settings(&self) -> TemplateSettings {
        TemplateSettings {
            plan: non_empty(&self.plan_text),
            layout: non_empty(&self.layout_text),
            seed_from_chief: self.settings.seed_from_chief,
        }
    }

    fn apply(&mut self, cx: &mut EditorContext, force: bool) {
        self.settings = self.current_settings();
        let (cache, status) = apply_template_settings(cx, &self.settings, force);
        self.cache = cache;
        cx.status = status.clone();
        self.status = status;
    }

    /// Returns `false` when the window was closed.
    fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) -> bool {
        let mut open = true;
        let mut close = false;
        let mut apply = false;
        let mut force = false;
        egui::Window::new("Preferences: Templates")
            .id(egui::Id::new("preferences_templates"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(560.0)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.label("New plans and layouts start from these Chief templates.");
                ui.add_space(4.0);
                for (label, text, exts) in [
                    ("Plan template", &mut self.plan_text, &["plan", "tpl"][..]),
                    ("Layout template", &mut self.layout_text, &["layout"][..]),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(label);
                        let edit = ui.add(
                            egui::TextEdit::singleline(text)
                                .desired_width(ui.available_width() - 90.0),
                        );
                        if edit.lost_focus() && edit.changed() {
                            apply = true;
                        }
                        if ui.button("Browse\u{2026}").clicked() {
                            if let Some(p) = rfd::FileDialog::new()
                                .add_filter("Chief template", exts)
                                .pick_file()
                            {
                                *text = p.to_string_lossy().into_owned();
                                apply = true;
                            }
                        }
                    });
                }
                if ui
                    .checkbox(
                        &mut self.settings.seed_from_chief,
                        "Seed defaults from Chief template",
                    )
                    .changed()
                {
                    apply = true;
                }
                ui.horizontal(|ui| {
                    if ui.button("Re-read template now").clicked() {
                        apply = true;
                        force = true;
                    }
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
                ui.separator();
                ui.strong("Decoded from the templates");
                match &self.cache.plan {
                    Some(p) => {
                        ui.label(&p.file_name);
                        for line in templates::plan_summary_lines(p) {
                            ui.label(line);
                        }
                        ui.weak(format!("Last read {}", templates::format_time(p.read_at)));
                    }
                    None => {
                        ui.weak("No plan template has been read.");
                    }
                }
                ui.add_space(4.0);
                match &self.cache.layout {
                    Some(l) => {
                        ui.label(&l.file_name);
                        for line in templates::layout_summary_lines(l) {
                            ui.label(line);
                        }
                        ui.weak(format!("Last read {}", templates::format_time(l.read_at)));
                    }
                    None => {
                        ui.weak("No layout template has been read.");
                    }
                }
                if !self.status.is_empty() {
                    ui.separator();
                    ui.label(&self.status);
                }
            });
        if apply {
            self.apply(cx, force);
        }
        open && !close
    }
}

// ===================================================================
// Roofs > Roof Defaults
// ===================================================================

/// The Roof Defaults page: a draft of [`plan_core::defaults::RoofDetailDefaults`]
/// (eave cut, fascia, soffit, rafter tails, attic walls, baseline rule) that
/// Build Roof copies into the roof settings of its floor, and the 3D view
/// draws from.
pub struct RoofDefaultsPage {
    pub draft: plan_core::defaults::RoofDetailDefaults,
    /// Also give the roofs of this plan the new detail.
    pub apply_to_plan: bool,
    fields: super::Fields,
    types: Vec<String>,
}

impl RoofDefaultsPage {
    pub fn new(cx: &EditorContext) -> Self {
        Self {
            draft: cx.defaults.roof_detail.clone(),
            apply_to_plan: true,
            fields: super::Fields::default(),
            types: cx
                .defaults
                .wall_types
                .iter()
                .map(|t| t.name.clone())
                .collect(),
        }
    }

    /// Why OK is off, if it is.
    pub fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            Some("Enter a valid length".into())
        } else {
            super::roof::detail_error(&self.draft)
        }
    }

    /// Makes the draft the defaults and, when asked, the detail of the
    /// plan's roofs (one undo step). Returns the floors changed.
    pub fn apply(&self, cx: &mut EditorContext) -> usize {
        cx.defaults.roof_detail = self.draft.clone();
        let mut changed = 0;
        if self.apply_to_plan {
            cx.begin_change("Roof Defaults");
            changed = crate::editor::roof_view::apply_detail(&mut cx.project, &self.draft);
        }
        cx.mark_dirty();
        changed
    }
}

thread_local! {
    static ROOF_OPEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ROOF_PAGE: RefCell<Option<RoofDefaultsPage>> = const { RefCell::new(None) };
}

/// Opens the Roof Defaults page (drawn by [`show_templates_page`]'s caller
/// once a frame).
pub fn open_roof_defaults() {
    ROOF_OPEN.with(|c| c.set(true));
}

/// Is the Roof Defaults page open?
#[cfg_attr(not(test), allow(dead_code))]
pub fn roof_defaults_open() -> bool {
    ROOF_OPEN.with(|c| c.get()) || ROOF_PAGE.with(|p| p.borrow().is_some())
}

fn show_roof_defaults(ctx: &egui::Context, cx: &mut EditorContext) {
    if ROOF_OPEN.with(|c| c.replace(false)) {
        ROOF_PAGE.with(|p| *p.borrow_mut() = Some(RoofDefaultsPage::new(cx)));
    }
    let Some(mut page) = ROOF_PAGE.with(|p| p.borrow_mut().take()) else {
        return;
    };
    let mut open = true;
    let mut outcome = super::Outcome::Open;
    egui::Window::new("Roof Defaults")
        .id(egui::Id::new("roof_defaults_page"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([460.0, 560.0])
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            let height = (ui.available_height() - 80.0).max(120.0);
            egui::ScrollArea::vertical()
                .id_salt("roof_defaults_scroll")
                .max_height(height)
                .show(ui, |ui| {
                    super::roof::detail_form(ui, &mut page.fields, &mut page.draft, &page.types);
                });
            ui.separator();
            ui.checkbox(
                &mut page.apply_to_plan,
                "Also use for the roofs in this plan",
            );
            let error = page.error();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_enabled(error.is_none(), egui::Button::new("   OK   "))
                    .clicked()
                {
                    outcome = super::Outcome::Ok;
                }
                if ui.button("Cancel").clicked() {
                    outcome = super::Outcome::Cancel;
                }
                if let Some(e) = &error {
                    ui.colored_label(egui::Color32::from_rgb(0xE0, 0x4B, 0x4B), e);
                }
            });
        });
    if !open || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        outcome = super::Outcome::Cancel;
    }
    match outcome {
        super::Outcome::Open => ROOF_PAGE.with(|p| *p.borrow_mut() = Some(page)),
        super::Outcome::Cancel => {}
        super::Outcome::Ok => {
            let n = page.apply(cx);
            cx.status = if n > 0 {
                format!("Saved the roof defaults; {n} roof(s) in this plan use them")
            } else {
                "Saved the roof defaults".into()
            };
        }
    }
}

// ===================================================================
// Cabinets > Cabinet Defaults, Framing > Framing Defaults
// ===================================================================

/// Command id: opens the Cabinet Defaults window (Edit menu).
pub const CABINETS: &str = "defaults.cabinets";
/// Command id: opens the Framing Defaults window (Build > Framing).
pub const FRAMING: &str = "defaults.framing";
/// Command id: switches to the Framing Overview, or back (Build > Framing).
pub const FRAMING_OVERVIEW: &str = "defaults.framing_overview";

thread_local! {
    /// The tab the Cabinet Defaults open on, once asked for.
    static OPEN_CABINETS: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    /// The tab the Framing Defaults open on, once asked for.
    static OPEN_FRAMING: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static CABINET_PAGE: RefCell<Option<super::cabinet::CabinetDefaultsDialog>> =
        const { RefCell::new(None) };
    static FRAMING_PAGE: RefCell<Option<super::framing::FramingDefaultsDialog>> =
        const { RefCell::new(None) };
}

/// Runs the menu commands of this module; false for any other id.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        CABINETS => OPEN_CABINETS.with(|c| c.set(Some(0))),
        FRAMING => OPEN_FRAMING.with(|c| c.set(Some(0))),
        RESET_TO_DEFAULTS => RESET.with(|c| c.set(true)),
        FRAMING_OVERVIEW => {
            if crate::editor::framing_view::in_overview(&cx.project) {
                crate::editor::framing_view::leave_overview(cx);
            } else {
                crate::editor::framing_view::activate_overview(cx);
            }
        }
        crate::plan_defaults::SET_AS_DEFAULT => {
            crate::plan_defaults::set_as_default(cx);
        }
        // Saved Defaults, Default Sets and Active Defaults, the tray ceiling
        // defaults, the template windows and the settings imports.
        _ => {
            return super::saved_defaults::run_command(cx, id)
                || super::default_sets::run_command(cx, id)
                || super::tray_ceiling::run_command(cx, id)
                || super::template_chooser::run_command(cx, id)
                || super::import_settings::run_command(cx, id)
        }
    }
    true
}

fn show_cabinet_defaults(ctx: &egui::Context, cx: &mut EditorContext) {
    if let Some(tab) = OPEN_CABINETS.with(|c| c.replace(None)) {
        let mut dlg = super::cabinet::CabinetDefaultsDialog::new(&cx.defaults.cabinets);
        dlg.start_on(tab);
        CABINET_PAGE.with(|p| *p.borrow_mut() = Some(dlg));
    }
    let Some(mut dlg) = CABINET_PAGE.with(|p| p.borrow_mut().take()) else {
        return;
    };
    match dlg.show(ctx) {
        super::Outcome::Open => CABINET_PAGE.with(|p| *p.borrow_mut() = Some(dlg)),
        super::Outcome::Cancel => {}
        super::Outcome::Ok => {
            // Dynamic defaults: cabinets that still have an old default value
            // follow the new one (manual p. 644).
            let old = std::mem::replace(&mut cx.defaults.cabinets, dlg.draft().clone());
            crate::tools::cabinet::apply_dynamic_defaults(cx, &old);
            cx.mark_dirty();
            cx.status = "Saved the cabinet defaults".into();
        }
    }
}

fn show_framing_defaults(ctx: &egui::Context, cx: &mut EditorContext) {
    if let Some(tab) = OPEN_FRAMING.with(|c| c.replace(None)) {
        let settings = crate::editor::framing_view::settings(&cx.project);
        let mut dlg = super::framing::FramingDefaultsDialog::new(&settings);
        dlg.start_on(tab);
        FRAMING_PAGE.with(|p| *p.borrow_mut() = Some(dlg));
    }
    let Some(mut dlg) = FRAMING_PAGE.with(|p| p.borrow_mut().take()) else {
        return;
    };
    match dlg.show(ctx) {
        super::Outcome::Open => FRAMING_PAGE.with(|p| *p.borrow_mut() = Some(dlg)),
        super::Outcome::Cancel => {}
        super::Outcome::Ok => {
            crate::editor::framing_view::set_settings(cx, dlg.draft().clone());
            cx.status = "Saved the framing defaults".into();
        }
    }
}

/// Is the Cabinet Defaults window open or asked for?
#[cfg(test)]
pub(crate) fn cabinet_defaults_open() -> bool {
    OPEN_CABINETS.with(|c| c.get().is_some())
        || CABINET_PAGE.with(|p| p.borrow().is_some())
        || super::cabinet_defaults::is_open()
}

/// Is the Framing Defaults window open or asked for?
#[cfg(test)]
pub(crate) fn framing_defaults_open() -> bool {
    OPEN_FRAMING.with(|c| c.get().is_some()) || FRAMING_PAGE.with(|p| p.borrow().is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::defaults::EaveCut;

    #[test]
    fn the_tree_lists_roof_defaults_and_it_opens_its_own_page() {
        let leaves: Vec<_> = TREE
            .iter()
            .flat_map(|(_, l)| l.iter())
            .map(|(n, l)| (*n, *l))
            .collect();
        assert!(leaves.contains(&("Roof Defaults", Leaf::RoofDefaults)));
        // Every other leaf still goes to the entry's dialog.
        assert_eq!(
            edit_outcome(Leaf::Entry(DefaultsEntry::Templates)),
            DefaultsOutcome::Edit(DefaultsEntry::Templates)
        );
        ROOF_OPEN.with(|c| c.set(false));
        assert_eq!(edit_outcome(Leaf::RoofDefaults), DefaultsOutcome::Open);
        assert!(roof_defaults_open());
        ROOF_OPEN.with(|c| c.set(false));
    }

    #[test]
    fn ok_on_the_roof_defaults_page_feeds_the_defaults_and_the_plan() {
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        let mut page = RoofDefaultsPage::new(&cx);
        assert!(
            page.draft.baseline_at_plate,
            "Daniel's template seats the roof on the plate"
        );
        page.draft.eave_cut = EaveCut::Square;
        page.draft.rafter_tails = true;
        assert!(page.error().is_none());
        // A plan with a stored roof settings entry takes the detail too.
        cx.project.floors[0].roofs = vec![serde_json::json!({"kind": "settings"})];
        assert_eq!(page.apply(&mut cx), 1);
        assert_eq!(cx.defaults.roof_detail.eave_cut, EaveCut::Square);
        let stored = crate::editor::roof_view::load(&cx.project.floors[0])
            .settings
            .unwrap();
        assert!(stored.detail.rafter_tails);
        // A bad size blocks OK.
        page.draft.thickness = 0.0;
        assert!(page.error().is_some());
    }
}
