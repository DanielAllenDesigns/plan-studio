//! Import Settings from Plan/Layout and the legacy imports (manual pp. 114 to
//! 118).
//!
//! A source is another plan: its project and, when it comes from a Chief
//! template or a Plan Studio defaults file, its [`PlanDefaults`]. The dialog
//! lists the importable items by category ([`items`]), the user picks some
//! and a way to treat name clashes ([`Clash`]), and [`Project::import_settings`]
//! brings them in. Settings only come from files in the same units; CAD,
//! Floor, Framing, Foundation and General Plan defaults are never imported.
//!
//! The legacy imports read an exchange file of one category: [`LayerFile`]
//! (`.layers`), [`DefaultSetsFile`] (`.cadefs`), [`WallFile`] (the wall
//! definitions `.dat`) and [`parse_note_types`] (`.json`). Plan Studio writes
//! and reads these as JSON; Chief's own binary files are read by name only
//! (see `plan-app/src/dialogs/import_settings.rs`).

use super::saved::{DefaultSet, SavedKind, SavedValue};
use super::{DimensionDefaultSet, PlanDefaults, WallTypeDef};
use crate::layer_sets::{LayerSetDef, SavedPlanView};
use crate::layers::LayerSet;
use crate::model::Project;
use crate::text_styles::{NoteType, NoteTypes};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The categories of the Import Default Settings dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ImportCategory {
    SavedDefaults,
    DefaultSettings,
    LayerSets,
    NoteTypes,
    SavedPlanViews,
    WallTypes,
}

impl ImportCategory {
    pub const ALL: [ImportCategory; 6] = [
        ImportCategory::SavedDefaults,
        ImportCategory::DefaultSettings,
        ImportCategory::LayerSets,
        ImportCategory::NoteTypes,
        ImportCategory::SavedPlanViews,
        ImportCategory::WallTypes,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ImportCategory::SavedDefaults => "Multiple Saved Defaults",
            ImportCategory::DefaultSettings => "Default Settings",
            ImportCategory::LayerSets => "Layer Sets",
            ImportCategory::NoteTypes => "Note Types",
            ImportCategory::SavedPlanViews => "Saved Plan Views",
            ImportCategory::WallTypes => "Wall Types",
        }
    }

    /// The categories a layout can offer (manual p. 115).
    pub const LAYOUT: [ImportCategory; 2] =
        [ImportCategory::SavedDefaults, ImportCategory::LayerSets];
}

/// How a name that exists already is treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Clash {
    /// The imported one replaces the one in the open file.
    Replace,
    /// The open file's is kept and the imported one gets a 2 (then 3...).
    #[default]
    Rename,
}

/// The group id of the Default Sets among the saved defaults.
pub const DEFAULT_SETS_GROUP: &str = "default_sets";

/// The Default Settings groups that can be imported, as `(id, label)`.
pub const SETTINGS_GROUPS: &[(&str, &str)] = &[
    ("walls", "Walls"),
    ("doors", "Doors"),
    ("windows", "Windows"),
    ("cabinets", "Cabinets"),
    ("rooms", "Rooms and Room Types"),
    ("roofs", "Roofs"),
    ("text", "Text Pages"),
    ("schedules", "Schedules"),
    ("electrical", "Electrical"),
    ("materials", "Materials"),
];

/// One importable thing: a category, a group inside it (the saved default
/// kind id, [`DEFAULT_SETS_GROUP`] or a settings group) and a name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ImportItem {
    pub category: ImportCategory,
    pub group: String,
    pub name: String,
}

impl ImportItem {
    fn new(category: ImportCategory, group: &str, name: &str) -> Self {
        Self {
            category,
            group: group.to_string(),
            name: name.to_string(),
        }
    }
}

/// A plan to import settings from.
#[derive(Debug, Clone)]
pub struct ImportSource {
    pub project: Project,
    /// The defaults that come with the file, when it has any.
    pub defaults: Option<PlanDefaults>,
    /// The source is a layout (only Default Sets and Layer Sets import).
    pub layout: bool,
    /// The source uses US units (settings only import from the same units).
    pub imperial: bool,
}

impl ImportSource {
    pub fn from_project(project: Project) -> Self {
        Self {
            project,
            defaults: None,
            layout: false,
            imperial: true,
        }
    }
}

/// Everything `src` offers, by category. Saved defaults are listed per kind
/// (and the Default Sets as a group of their own).
pub fn items(src: &ImportSource) -> Vec<ImportItem> {
    let mut out = Vec::new();
    let proj = src.project.clone();
    let defs = src.defaults.clone().unwrap_or_default();
    for kind in SavedKind::ALL {
        if kind == SavedKind::ManualDimensions && src.defaults.is_none() {
            continue;
        }
        if kind != SavedKind::ManualDimensions && !proj.saved_defaults.lists.contains_key(kind.id())
        {
            continue;
        }
        for n in proj.saved_names(&defs, kind) {
            out.push(ImportItem::new(
                ImportCategory::SavedDefaults,
                kind.id(),
                &n,
            ));
        }
    }
    for s in &src.project.saved_defaults.sets {
        out.push(ImportItem::new(
            ImportCategory::SavedDefaults,
            DEFAULT_SETS_GROUP,
            &s.name,
        ));
    }
    for n in src.project.layer_sets.names() {
        out.push(ImportItem::new(ImportCategory::LayerSets, "", n));
    }
    if src.layout {
        return out;
    }
    if let Some(d) = &src.defaults {
        for (id, _) in SETTINGS_GROUPS {
            let present = match *id {
                "text" => d.pages.keys().any(|k| k.starts_with("text.")),
                "schedules" => d.pages.keys().any(|k| k.starts_with("schedules.")),
                "electrical" | "materials" => false,
                _ => true,
            };
            if present {
                out.push(ImportItem::new(
                    ImportCategory::DefaultSettings,
                    id,
                    label_of_group(id),
                ));
            }
        }
    }
    if src.project.electrical_defaults.is_some() {
        out.push(ImportItem::new(
            ImportCategory::DefaultSettings,
            "electrical",
            label_of_group("electrical"),
        ));
    }
    if !src.project.material_defaults.is_empty() {
        out.push(ImportItem::new(
            ImportCategory::DefaultSettings,
            "materials",
            label_of_group("materials"),
        ));
    }
    for t in &src.project.note_types.types {
        out.push(ImportItem::new(ImportCategory::NoteTypes, "", &t.name));
    }
    for v in &src.project.plan_views {
        out.push(ImportItem::new(ImportCategory::SavedPlanViews, "", &v.name));
    }
    for t in &src.project.wall_types {
        out.push(ImportItem::new(ImportCategory::WallTypes, "", &t.name));
    }
    out
}

fn label_of_group(id: &str) -> &'static str {
    SETTINGS_GROUPS
        .iter()
        .find(|(g, _)| *g == id)
        .map_or("", |(_, l)| *l)
}

/// What an import did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub added: usize,
    pub replaced: usize,
    pub renamed: usize,
    pub skipped: usize,
    /// Notes for the status line (a layout cannot import wall types, ...).
    pub notes: Vec<String>,
}

impl ImportReport {
    pub fn total(&self) -> usize {
        self.added + self.replaced + self.renamed
    }

    pub fn summary(&self) -> String {
        format!(
            "Imported {} settings ({} added, {} replaced, {} renamed)",
            self.total(),
            self.added,
            self.replaced,
            self.renamed
        )
    }
}

/// `name` made unique among `taken`: "name 2", "name 3", ... (the number
/// Chief appends).
pub fn numbered_name(name: &str, taken: &dyn Fn(&str) -> bool) -> String {
    let mut n = 2;
    loop {
        let c = format!("{name} {n}");
        if !taken(&c) {
            return c;
        }
        n += 1;
    }
}

impl Project {
    /// Imports the picked `items` of `src` (Import Settings from Plan/Layout).
    /// Each item is brought in once; a name that exists already follows
    /// `clash`. The active saved defaults and the shown layer set stay as they
    /// are.
    pub fn import_settings(
        &mut self,
        d: &mut PlanDefaults,
        src: &ImportSource,
        picks: &BTreeSet<ImportItem>,
        clash: Clash,
    ) -> ImportReport {
        let mut rep = ImportReport::default();
        let mut s_proj = src.project.clone();
        let mut s_defs = src.defaults.clone().unwrap_or_default();
        for item in picks {
            if src.layout && !ImportCategory::LAYOUT.contains(&item.category) {
                rep.skipped += 1;
                continue;
            }
            match item.category {
                ImportCategory::LayerSets => {
                    self.import_layer_set(src, &item.name, clash, &mut rep)
                }
                ImportCategory::SavedDefaults => {
                    if item.group == DEFAULT_SETS_GROUP {
                        self.import_default_set(&s_proj, &item.name, clash, &mut rep);
                    } else if let Some(kind) = SavedKind::from_id(&item.group) {
                        self.import_saved(
                            d,
                            &mut s_proj,
                            &mut s_defs,
                            kind,
                            &item.name,
                            clash,
                            &mut rep,
                        );
                    } else {
                        rep.skipped += 1;
                    }
                }
                ImportCategory::NoteTypes => {
                    if let Some(t) = src.project.note_types.get(&item.name) {
                        self.import_note_type(t, clash, &mut rep);
                    }
                }
                ImportCategory::SavedPlanViews => {
                    if let Some(v) = src.project.plan_view(&item.name) {
                        self.import_plan_view(v, clash, &mut rep);
                    }
                }
                ImportCategory::WallTypes => {
                    if let Some(t) = src.project.wall_types.iter().find(|t| t.name == item.name) {
                        self.import_wall_type(t, clash, &mut rep);
                    }
                }
                ImportCategory::DefaultSettings => {
                    self.import_setting_group(d, src, &item.group, &mut rep)
                }
            }
        }
        rep
    }

    fn import_layer_set(
        &mut self,
        src: &ImportSource,
        name: &str,
        clash: Clash,
        rep: &mut ImportReport,
    ) {
        let Some(def) = src.project.layer_sets.get(name).cloned() else {
            return;
        };
        // Layers the other plan has and this one lacks come along.
        for st in &def.states {
            if self.layers.get(&st.layer).is_none() {
                if let Some(l) = src.project.layers.get(&st.layer) {
                    self.layers.add(l.clone());
                }
            }
        }
        self.put_layer_set(def, clash, rep);
    }

    fn put_layer_set(&mut self, mut def: LayerSetDef, clash: Clash, rep: &mut ImportReport) {
        let name = def.name.clone();
        if self.layer_sets.get(&name).is_some() {
            match clash {
                Clash::Replace => {
                    if let Some(slot) = self.layer_sets.get_mut(&name) {
                        *slot = def;
                    }
                    rep.replaced += 1;
                }
                Clash::Rename => {
                    def.name = numbered_name(&name, &|n| self.layer_sets.get(n).is_some());
                    self.layer_sets.sets.push(def);
                    rep.renamed += 1;
                }
            }
        } else {
            self.layer_sets.sets.push(def);
            rep.added += 1;
        }
    }

    fn import_note_type(&mut self, t: &NoteType, clash: Clash, rep: &mut ImportReport) {
        if let Some(existing) = self.note_types.types.iter_mut().find(|e| e.name == t.name) {
            match clash {
                Clash::Replace => {
                    *existing = t.clone();
                    rep.replaced += 1;
                }
                Clash::Rename => {
                    let mut copy = t.clone();
                    copy.name = numbered_name(&t.name, &|n| self.note_types.get(n).is_some());
                    self.note_types.types.push(copy);
                    rep.renamed += 1;
                }
            }
        } else {
            self.note_types.types.push(t.clone());
            rep.added += 1;
        }
    }

    fn import_plan_view(&mut self, v: &SavedPlanView, clash: Clash, rep: &mut ImportReport) {
        let mut v = v.clone();
        // A view needs a layer set of its name here; when the source's set was
        // not imported the view keeps working on the active one.
        if self.layer_sets.get(&v.layer_set).is_none() {
            v.layer_set = self.layer_sets.active.clone();
        }
        if let Some(i) = self.plan_views.iter().position(|e| e.name == v.name) {
            match clash {
                Clash::Replace => {
                    self.plan_views[i] = v;
                    rep.replaced += 1;
                }
                Clash::Rename => {
                    v.name = numbered_name(&v.name.clone(), &|n| self.plan_view(n).is_some());
                    self.plan_views.push(v);
                    rep.renamed += 1;
                }
            }
        } else {
            self.plan_views.push(v);
            rep.added += 1;
        }
    }

    fn import_wall_type(&mut self, t: &WallTypeDef, clash: Clash, rep: &mut ImportReport) {
        if let Some(i) = self.wall_types.iter().position(|e| e.name == t.name) {
            match clash {
                Clash::Replace => {
                    self.wall_types[i] = t.clone();
                    rep.replaced += 1;
                }
                Clash::Rename => {
                    let mut c = t.clone();
                    c.name =
                        numbered_name(&t.name, &|n| self.wall_types.iter().any(|w| w.name == n));
                    self.wall_types.push(c);
                    rep.renamed += 1;
                }
            }
        } else {
            self.wall_types.push(t.clone());
            rep.added += 1;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn import_saved(
        &mut self,
        d: &mut PlanDefaults,
        s_proj: &mut Project,
        s_defs: &mut PlanDefaults,
        kind: SavedKind,
        name: &str,
        clash: Clash,
        rep: &mut ImportReport,
    ) {
        if kind == SavedKind::ManualDimensions {
            let Some(set) = s_defs.dimension_set(name).cloned() else {
                return;
            };
            self.put_dimension_set(d, set, clash, rep);
            return;
        }
        let Some(value) = s_proj.saved_value(s_defs, kind, name) else {
            return;
        };
        let exists = self.saved_names(d, kind).iter().any(|n| n == name);
        match (exists, clash) {
            (false, _) => {
                self.saved_store(d, kind, name, value, false);
                rep.added += 1;
            }
            (true, Clash::Replace) => {
                self.saved_store(d, kind, name, value, true);
                rep.replaced += 1;
            }
            (true, Clash::Rename) => {
                self.saved_store(d, kind, name, value, false);
                rep.renamed += 1;
            }
        }
    }

    fn put_dimension_set(
        &mut self,
        d: &mut PlanDefaults,
        set: DimensionDefaultSet,
        clash: Clash,
        rep: &mut ImportReport,
    ) {
        let name = set.name.clone();
        if let Some(i) = d.dimension_sets.iter().position(|s| s.name == name) {
            match clash {
                Clash::Replace => {
                    d.dimension_sets[i] = set;
                    if d.active_dimension_set == name {
                        let n = name.clone();
                        d.set_active_dimension_set(&n);
                    }
                    rep.replaced += 1;
                }
                Clash::Rename => {
                    let new_name = numbered_name(&name, &|n| d.dimension_set(n).is_some());
                    d.dimension_sets.push(set.cloned_as(new_name));
                    rep.renamed += 1;
                }
            }
        } else {
            d.dimension_sets.push(set);
            rep.added += 1;
        }
    }

    fn import_default_set(
        &mut self,
        s_proj: &Project,
        name: &str,
        clash: Clash,
        rep: &mut ImportReport,
    ) {
        let Some(mut set) = s_proj.saved_defaults.set(name).cloned() else {
            return;
        };
        if let Some(i) = self.saved_defaults.sets.iter().position(|s| s.name == name) {
            match clash {
                Clash::Replace => {
                    self.saved_defaults.sets[i] = set;
                    rep.replaced += 1;
                }
                Clash::Rename => {
                    set.name = numbered_name(name, &|n| self.saved_defaults.set(n).is_some());
                    self.saved_defaults.sets.push(set);
                    rep.renamed += 1;
                }
            }
        } else {
            self.saved_defaults.sets.push(set);
            rep.added += 1;
        }
    }

    fn import_setting_group(
        &mut self,
        d: &mut PlanDefaults,
        src: &ImportSource,
        group: &str,
        rep: &mut ImportReport,
    ) {
        let Some(s) = &src.defaults else {
            // Only the groups a plan file carries itself.
            match group {
                "electrical" => {
                    if let Some(v) = &src.project.electrical_defaults {
                        self.electrical_defaults = Some(v.clone());
                        rep.replaced += 1;
                    }
                }
                "materials" => {
                    self.material_defaults = src.project.material_defaults.clone();
                    rep.replaced += 1;
                }
                _ => rep.skipped += 1,
            }
            return;
        };
        match group {
            "walls" => {
                d.exterior_wall = s.exterior_wall.clone();
                d.interior_wall = s.interior_wall.clone();
                d.foundation_wall = s.foundation_wall.clone();
                d.wall_variants = s.wall_variants.clone();
            }
            "doors" => {
                d.interior_door = s.interior_door.clone();
                d.exterior_door = s.exterior_door.clone();
                d.opening_variants = s.opening_variants.clone();
            }
            "windows" => d.window = s.window.clone(),
            "cabinets" => d.cabinets = s.cabinets.clone(),
            "rooms" => d.rooms.room_types = s.rooms.room_types.clone(),
            "roofs" => d.roof_detail = s.roof_detail.clone(),
            "text" => copy_pages(d, s, "text."),
            "schedules" => copy_pages(d, s, "schedules."),
            "electrical" => {
                if let Some(v) = &src.project.electrical_defaults {
                    self.electrical_defaults = Some(v.clone());
                }
            }
            "materials" => self.material_defaults = src.project.material_defaults.clone(),
            _ => {
                rep.skipped += 1;
                return;
            }
        }
        rep.replaced += 1;
    }
}

fn copy_pages(d: &mut PlanDefaults, s: &PlanDefaults, prefix: &str) {
    d.pages.retain(|k, _| !k.starts_with(prefix));
    for (k, v) in s.pages.iter().filter(|(k, _)| k.starts_with(prefix)) {
        d.pages.insert(k.clone(), v.clone());
    }
}

// ===================================================================
// Legacy exchange files
// ===================================================================

/// A `.layers` file: layer sets with the layers they need.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LayerFile {
    pub sets: Vec<LayerSetDef>,
    #[serde(default)]
    pub layers: LayerSet,
}

impl LayerFile {
    /// The layer sets `names` of `project` (all when `names` is empty).
    pub fn export(project: &Project, names: &[String]) -> LayerFile {
        LayerFile {
            sets: project
                .layer_sets
                .sets
                .iter()
                .filter(|s| names.is_empty() || names.contains(&s.name))
                .cloned()
                .collect(),
            layers: project.layers.clone(),
        }
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(s: &str) -> serde_json::Result<LayerFile> {
        serde_json::from_str(s)
    }
}

impl Project {
    /// Import Layer Sets (legacy `.layers`): the sets in `names` come in, a
    /// name that exists is replaced or gets a 2 (Existing Layer Sets). Returns
    /// the report.
    pub fn import_layer_file(
        &mut self,
        file: &LayerFile,
        names: &[String],
        clash: Clash,
    ) -> ImportReport {
        let mut rep = ImportReport::default();
        for def in file.sets.iter().filter(|s| names.contains(&s.name)) {
            for st in &def.states {
                if self.layers.get(&st.layer).is_none() {
                    if let Some(l) = file.layers.get(&st.layer) {
                        self.layers.add(l.clone());
                    }
                }
            }
            self.put_layer_set(def.clone(), clash, &mut rep);
        }
        rep
    }
}

/// A `.cadefs` file: Default Sets with the saved defaults they use and the
/// layer sets associated with them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DefaultSetsFile {
    pub default_sets: Vec<DefaultSet>,
    /// Saved defaults by kind id then name.
    pub saved: Vec<(String, String, SavedValue)>,
    pub dimension_sets: Vec<DimensionDefaultSet>,
    /// Layer sets associated with a Default Set (others are not exported).
    pub layer_sets: Vec<LayerSetDef>,
}

impl DefaultSetsFile {
    /// Everything a `.cadefs` carries from `project`: all Default Sets, all
    /// saved defaults of the annotation kinds and the layer sets the Default
    /// Sets use.
    pub fn export(project: &Project, d: &PlanDefaults) -> DefaultSetsFile {
        let mut p = project.clone();
        let mut dd = d.clone();
        let mut saved = Vec::new();
        for kind in SavedKind::ANNOTATION {
            if kind == SavedKind::ManualDimensions {
                continue;
            }
            for n in p.saved_names(&mut dd, kind) {
                if let Some(v) = p.saved_value(&mut dd, kind, &n) {
                    saved.push((kind.id().to_string(), n, v));
                }
            }
        }
        let used: BTreeSet<&str> = project
            .saved_defaults
            .sets
            .iter()
            .map(|s| s.layer_set.as_str())
            .filter(|s| !s.is_empty())
            .collect();
        DefaultSetsFile {
            default_sets: project.saved_defaults.sets.clone(),
            saved,
            dimension_sets: d.dimension_sets.clone(),
            layer_sets: project
                .layer_sets
                .sets
                .iter()
                .filter(|s| used.contains(s.name.as_str()))
                .cloned()
                .collect(),
        }
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(s: &str) -> serde_json::Result<DefaultSetsFile> {
        serde_json::from_str(s)
    }
}

impl Project {
    /// Import Default Sets (legacy `.cadefs`). With `overwrite` the file's
    /// saved defaults of Text, Rich Text, Callouts, Markers, Arrows and
    /// Dimensions replace the ones in the plan; without, they are kept along
    /// with the imported ones. Saved Dimension Defaults are never overwritten
    /// while the plan has dimensions. Duplicate names follow `clash`. A layer
    /// set associated with a Default Set only comes in when the plan does not
    /// have it.
    pub fn import_default_sets_file(
        &mut self,
        d: &mut PlanDefaults,
        file: &DefaultSetsFile,
        overwrite: bool,
        clash: Clash,
    ) -> ImportReport {
        let mut rep = ImportReport::default();
        let has_dimensions = self.floors.iter().any(|f| !f.dimensions.is_empty());
        if overwrite {
            for kind in [
                SavedKind::Text,
                SavedKind::RichText,
                SavedKind::Callouts,
                SavedKind::Markers,
                SavedKind::Arrows,
            ] {
                let incoming: Vec<_> = file
                    .saved
                    .iter()
                    .filter(|(k, _, _)| k == kind.id())
                    .collect();
                if incoming.is_empty() {
                    continue;
                }
                // Keep the active one and what the sets use: a saved default
                // in use cannot go.
                let names = self.saved_names(d, kind);
                for n in names {
                    if incoming.iter().all(|(_, in_name, _)| *in_name != n) {
                        let _ = self.saved_delete(d, kind, &n);
                    }
                }
            }
            if !has_dimensions && !file.dimension_sets.is_empty() {
                let keep_active = d.active_dimension_set.clone();
                d.dimension_sets = file.dimension_sets.clone();
                if !d.set_active_dimension_set(&keep_active) {
                    if let Some(first) = d.dimension_sets.first().map(|s| s.name.clone()) {
                        d.set_active_dimension_set(&first);
                    }
                }
                rep.replaced += file.dimension_sets.len();
            }
        }
        for (kid, name, value) in &file.saved {
            let Some(kind) = SavedKind::from_id(kid) else {
                continue;
            };
            let exists = self.saved_names(d, kind).iter().any(|n| n == name);
            let replace = matches!(clash, Clash::Replace);
            self.saved_store(d, kind, name, value.clone(), replace);
            match (exists, replace) {
                (false, _) => rep.added += 1,
                (true, true) => rep.replaced += 1,
                (true, false) => rep.renamed += 1,
            }
        }
        if !overwrite || has_dimensions {
            for set in &file.dimension_sets {
                self.put_dimension_set(d, set.clone(), clash, &mut rep);
            }
        }
        for def in &file.layer_sets {
            if self.layer_sets.get(&def.name).is_none() {
                self.layer_sets.sets.push(def.clone());
                rep.added += 1;
            }
        }
        for set in &file.default_sets {
            let mut set = set.clone();
            if let Some(i) = self
                .saved_defaults
                .sets
                .iter()
                .position(|s| s.name == set.name)
            {
                match clash {
                    Clash::Replace => {
                        self.saved_defaults.sets[i] = set;
                        rep.replaced += 1;
                    }
                    Clash::Rename => {
                        let old = set.name.clone();
                        set.name = numbered_name(&old, &|n| self.saved_defaults.set(n).is_some());
                        // The renamed set uses the names the saved defaults
                        // were stored under here; a rename of those is the
                        // same "name 2" rule, so members that were renamed on
                        // import follow.
                        self.saved_defaults.sets.push(set);
                        rep.renamed += 1;
                    }
                }
            } else {
                self.saved_defaults.sets.push(set);
                rep.added += 1;
            }
        }
        rep
    }

    /// Import Wall Definitions (legacy `.dat`): same-named types are replaced
    /// when `replace_same`, else the plan's are kept. Returns how many came
    /// in.
    pub fn import_wall_file(&mut self, file: &WallFile, replace_same: bool) -> ImportReport {
        let mut rep = ImportReport::default();
        for t in &file.wall_types {
            match self.wall_types.iter().position(|e| e.name == t.name) {
                Some(i) if replace_same => {
                    self.wall_types[i] = t.clone();
                    rep.replaced += 1;
                }
                Some(_) => rep.skipped += 1,
                None => {
                    self.wall_types.push(t.clone());
                    rep.added += 1;
                }
            }
        }
        rep
    }

    /// Import Note Types (legacy `.json`): types the plan lacks are added,
    /// same-named ones follow `clash`.
    pub fn import_note_types(&mut self, types: &[NoteType], clash: Clash) -> ImportReport {
        let mut rep = ImportReport::default();
        for t in types {
            self.import_note_type(t, clash, &mut rep);
        }
        rep
    }
}

/// A wall definitions file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WallFile {
    pub wall_types: Vec<WallTypeDef>,
}

impl WallFile {
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(s: &str) -> serde_json::Result<WallFile> {
        serde_json::from_str(s)
    }
}

/// Reads an exported Note Types `.json`: a list of types or an object that
/// holds the list (`types`, `note_types` or `NoteTypes`); each type has a
/// name, a prefix and optionally a text style.
pub fn parse_note_types(text: &str) -> Result<Vec<NoteType>, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let list = match &v {
        serde_json::Value::Array(a) => a.clone(),
        serde_json::Value::Object(o) => ["types", "note_types", "NoteTypes", "noteTypes"]
            .iter()
            .find_map(|k| o.get(*k).and_then(|x| x.as_array()).cloned())
            .ok_or("No list of note types in the file")?,
        _ => return Err("Not a note types file".into()),
    };
    let field = |o: &serde_json::Map<String, serde_json::Value>, keys: &[&str]| -> String {
        keys.iter()
            .find_map(|k| o.get(*k).and_then(|x| x.as_str()))
            .unwrap_or("")
            .to_string()
    };
    let mut out = Vec::new();
    for item in list {
        let Some(o) = item.as_object() else {
            continue;
        };
        let name = field(o, &["name", "Name"]);
        let prefix = field(o, &["prefix", "Prefix"]);
        if name.trim().is_empty() || prefix.is_empty() {
            continue;
        }
        out.push(NoteType {
            name: name.trim().to_string(),
            prefix,
            style: field(o, &["style", "Style", "text_style"]),
        });
    }
    if out.is_empty() {
        Err("The file holds no note types".into())
    } else {
        Ok(out)
    }
}

/// The JSON a Note Types export holds.
pub fn note_types_json(types: &NoteTypes) -> String {
    serde_json::to_string_pretty(&types.types).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defaults::saved::SavedKind;

    fn source() -> ImportSource {
        let mut d = PlanDefaults::chief_x18_daniel();
        let mut p = Project::from_defaults("Src", &d);
        p.layer_sets.copy_set("Default Set", "Plot Plan Layer Set");
        p.layer_sets.copy_set("Default Set", "Electrical Layer Set");
        p.saved_copy(&mut d, SavedKind::RichText, "Default", "Plot Rich")
            .unwrap();
        p.saved_copy(&mut d, SavedKind::Arrows, "Default", "Big Arrow")
            .unwrap();
        p.default_set_save_new(&mut d, "Plot").unwrap();
        p.note_types.add("Roof Note", "R");
        p.plan_views
            .push(SavedPlanView::new("Plot Plan View", "Plot Plan Layer Set"));
        d.dimension_sets
            .push(d.dimension_sets[0].cloned_as("Custom Scale"));
        ImportSource {
            project: p,
            defaults: Some(d),
            layout: false,
            imperial: true,
        }
    }

    fn all(src: &ImportSource) -> BTreeSet<ImportItem> {
        items(src).into_iter().collect()
    }

    #[test]
    fn the_source_lists_each_category() {
        let src = source();
        let it = items(&src);
        for c in [
            ImportCategory::SavedDefaults,
            ImportCategory::DefaultSettings,
            ImportCategory::LayerSets,
            ImportCategory::NoteTypes,
            ImportCategory::SavedPlanViews,
            ImportCategory::WallTypes,
        ] {
            assert!(it.iter().any(|i| i.category == c), "{c:?}");
        }
        assert!(it
            .iter()
            .any(|i| i.group == DEFAULT_SETS_GROUP && i.name == "Plot"));
        assert!(it
            .iter()
            .any(|i| i.group == "rich_text" && i.name == "Plot Rich"));
        // A layout offers only default sets and layer sets.
        let mut lay = src.clone();
        lay.layout = true;
        assert!(items(&lay)
            .iter()
            .all(|i| ImportCategory::LAYOUT.contains(&i.category)));
    }

    #[test]
    fn importing_everything_into_a_fresh_plan_adds_each_item_once() {
        let src = source();
        let mut d = PlanDefaults::chief_x18_daniel();
        let mut p = Project::from_defaults("Dst", &d);
        let before_sets = p.layer_sets.sets.len();
        let picks = all(&src);
        let rep = p.import_settings(&mut d, &src, &picks, Clash::Rename);
        assert!(rep.total() > 5, "{rep:?}");
        // The source's "Default Set" clashes with ours and arrives as "Default Set 2".
        assert_eq!(p.layer_sets.sets.len(), before_sets + 3);
        assert!(p.layer_sets.get("Default Set 2").is_some());
        assert!(p
            .saved_names(&mut d, SavedKind::RichText)
            .contains(&"Plot Rich".to_string()));
        assert!(p
            .saved_names(&mut d, SavedKind::Arrows)
            .contains(&"Big Arrow".to_string()));
        assert!(p.saved_defaults.set("Plot").is_some());
        assert!(p.note_types.get("Roof Note").is_some());
        assert!(p.plan_view("Plot Plan View").is_some());
        assert!(d.dimension_set("Custom Scale").is_some());
        // Importing the same picks again renames, it never loses or doubles
        // silently: names that clash get a 2.
        let again = p.import_settings(&mut d, &src, &picks, Clash::Rename);
        assert!(again.renamed >= 5, "{again:?}");
        assert!(p.layer_sets.get("Plot Plan Layer Set 2").is_some());
        assert!(p.saved_defaults.set("Plot 2").is_some());
        assert!(p.plan_view("Plot Plan View 2").is_some());
        assert!(d.dimension_set("Custom Scale 2").is_some());
        // And Replace keeps the count.
        let n = p.layer_sets.sets.len();
        let rep = p.import_settings(&mut d, &src, &picks, Clash::Replace);
        assert!(rep.replaced >= 5, "{rep:?}");
        assert_eq!(p.layer_sets.sets.len(), n);
    }

    #[test]
    fn only_the_picked_items_come_in() {
        let src = source();
        let mut d = PlanDefaults::chief_x18_daniel();
        let mut p = Project::from_defaults("Dst", &d);
        let picks: BTreeSet<ImportItem> = items(&src)
            .into_iter()
            .filter(|i| i.category == ImportCategory::NoteTypes && i.name == "Roof Note")
            .collect();
        let rep = p.import_settings(&mut d, &src, &picks, Clash::Rename);
        assert_eq!(rep.total(), 1);
        assert!(p.layer_sets.get("Plot Plan Layer Set").is_none());
    }

    #[test]
    fn a_layout_source_skips_what_a_layout_does_not_have() {
        let mut src = source();
        src.layout = true;
        let mut d = PlanDefaults::chief_x18_daniel();
        let mut p = Project::from_defaults("Dst", &d);
        let mut picks = all(&src);
        picks.insert(ImportItem {
            category: ImportCategory::WallTypes,
            group: String::new(),
            name: "Siding-6".into(),
        });
        let rep = p.import_settings(&mut d, &src, &picks, Clash::Rename);
        assert_eq!(rep.skipped, 1);
        assert!(p.note_types.get("Roof Note").is_none());
    }

    #[test]
    fn a_plan_view_without_its_layer_set_falls_back_to_the_active_one() {
        let src = source();
        let mut d = PlanDefaults::chief_x18_daniel();
        let mut p = Project::from_defaults("Dst", &d);
        let picks: BTreeSet<ImportItem> = items(&src)
            .into_iter()
            .filter(|i| i.category == ImportCategory::SavedPlanViews && i.name == "Plot Plan View")
            .collect();
        p.import_settings(&mut d, &src, &picks, Clash::Rename);
        assert_eq!(
            p.plan_view("Plot Plan View").unwrap().layer_set,
            p.layer_sets.active
        );
    }

    #[test]
    fn layer_sets_file_round_trips_and_imports_with_replace_or_copy() {
        let src = source();
        let file = LayerFile::export(&src.project, &[]);
        let back = LayerFile::from_json(&file.to_json().unwrap()).unwrap();
        assert_eq!(back, file);
        let mut d = PlanDefaults::chief_x18_daniel();
        let mut p = Project::from_defaults("Dst", &d);
        let names = vec!["Plot Plan Layer Set".to_string()];
        let _ = &mut d;
        let r = p.import_layer_file(&back, &names, Clash::Rename);
        assert_eq!(r.added, 1);
        let r = p.import_layer_file(&back, &names, Clash::Rename);
        assert_eq!(r.renamed, 1);
        assert!(p.layer_sets.get("Plot Plan Layer Set 2").is_some());
        let r = p.import_layer_file(&back, &names, Clash::Replace);
        assert_eq!(r.replaced, 1);
        // "Plot Plan Layer Set 2" exists, so the next copy is the 3.
        p.import_layer_file(&back, &names, Clash::Rename);
        assert!(p.layer_sets.get("Plot Plan Layer Set 3").is_some());
    }

    #[test]
    fn default_sets_file_imports_overwrite_or_keep_and_never_clobbers_dimensions_in_use() {
        let src = source();
        let sdefs = src.defaults.clone().unwrap();
        let file = DefaultSetsFile::export(&src.project, &sdefs);
        assert!(file.default_sets.iter().any(|s| s.name == "Plot"));
        assert!(!file.layer_sets.is_empty());
        let back = DefaultSetsFile::from_json(&file.to_json().unwrap()).unwrap();
        assert_eq!(back, file);

        let mut d = PlanDefaults::chief_x18_daniel();
        let mut p = Project::from_defaults("Dst", &d);
        let r = p.import_default_sets_file(&mut d, &back, false, Clash::Rename);
        assert!(r.added >= 3, "{r:?}");
        assert!(p
            .saved_names(&mut d, SavedKind::RichText)
            .contains(&"Plot Rich".to_string()));
        assert!(p.saved_defaults.set("Plot").is_some());
        // Not overwriting keeps the originals and renames the duplicates.
        let r = p.import_default_sets_file(&mut d, &back, false, Clash::Rename);
        assert!(r.renamed >= 3, "{r:?}");
        assert!(p
            .saved_names(&mut d, SavedKind::RichText)
            .contains(&"Plot Rich 2".to_string()));
        // Overwrite replaces the lists: what the file lacks goes (unless held).
        p.saved_copy(&mut d, SavedKind::Arrows, "Default", "Extra")
            .unwrap();
        let r = p.import_default_sets_file(&mut d, &back, true, Clash::Replace);
        assert!(r.replaced >= 1);
        assert!(!p
            .saved_names(&mut d, SavedKind::Arrows)
            .contains(&"Extra".to_string()));
        // With a dimension drawn, the saved dimension defaults are not overwritten.
        let n = d.dimension_sets.len();
        let mut f2 = back.clone();
        f2.dimension_sets.truncate(2);
        let id = p.alloc_id();
        p.floors[0].dimensions.push(crate::dimension::Dimension {
            id,
            kind: crate::dimension::DimensionKind::Manual,
            start: crate::geometry::Point::new(0.0, 0.0),
            end: crate::geometry::Point::new(10.0, 0.0),
            offset: 0.0,
            text_override: None,
            anchors: [None, None],
            hide_ext: [false, false],
            auto_group: Default::default(),
            text_style: None,
            look: Default::default(),
        });
        p.import_default_sets_file(&mut d, &f2, true, Clash::Replace);
        assert!(d.dimension_sets.len() >= n, "dimension sets kept");
    }

    #[test]
    fn wall_definition_files_replace_or_keep_same_names() {
        let src = source();
        let file = WallFile {
            wall_types: src.project.wall_types.iter().take(3).cloned().collect(),
        };
        let back = WallFile::from_json(&file.to_json().unwrap()).unwrap();
        let mut d = PlanDefaults::chief_x18_daniel();
        let mut p = Project::from_defaults("Dst", &d);
        let _ = &mut d;
        let n = p.wall_types.len();
        let mut changed = back.clone();
        changed.wall_types[0].layers[0].thickness += 1.0;
        let r = p.import_wall_file(&changed, false);
        assert_eq!(r.skipped, 3);
        assert_eq!(p.wall_types.len(), n);
        let r = p.import_wall_file(&changed, true);
        assert_eq!(r.replaced, 3);
        assert_eq!(
            p.wall_types[0].layers[0].thickness,
            changed.wall_types[0].layers[0].thickness
        );
    }

    #[test]
    fn note_types_parse_from_the_shapes_a_json_export_may_have() {
        let a = r#"[{"name":"Roof Note","prefix":"R"},{"Name":"Plumbing Note","Prefix":"P","Style":"1/4\" Text Style"}]"#;
        let t = parse_note_types(a).unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t[1].style, "1/4\" Text Style");
        let b = r#"{"NoteTypes":[{"name":"X","prefix":"X"}]}"#;
        assert_eq!(parse_note_types(b).unwrap().len(), 1);
        assert!(parse_note_types("{}").is_err());
        assert!(parse_note_types("not json").is_err());
        assert!(parse_note_types("[]").is_err());
        let mut d = PlanDefaults::chief_x18_daniel();
        let mut p = Project::from_defaults("P", &d);
        let _ = &mut d;
        let r = p.import_note_types(&t, Clash::Rename);
        assert_eq!(r.added, 2);
        let r = p.import_note_types(&t, Clash::Rename);
        assert_eq!(r.renamed, 2);
        assert!(p.note_types.get("Roof Note 2").is_some());
        let round = parse_note_types(&note_types_json(&p.note_types)).unwrap();
        assert_eq!(round.len(), p.note_types.types.len());
    }
}
