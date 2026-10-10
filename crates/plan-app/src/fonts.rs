//! System fonts for plan text, layout text and printed PDFs.
//!
//! * **Discovery**: [`FontCatalog::scan`] walks the font folders of the
//!   machine (macOS `/System/Library/Fonts`, `/Library/Fonts`,
//!   `~/Library/Fonts`; Linux `/usr/share/fonts`, `~/.fonts`; Windows
//!   `C:\Windows\Fonts`) and reads each `.ttf`, `.otf`, `.ttc` and `.otc`
//!   with our own name-table parser (family, style, weight, italic) without
//!   loading the whole file.
//! * **Matching**: a text style names a font the way Chief does (`Avenir`,
//!   `Avenir Book`, `Arial`, `Chief Blueprint`) plus bold and italic flags.
//!   [`FontCatalog::find`] maps that to a real face: Chief's names get the
//!   usual stand-ins (Arial becomes Helvetica Neue on a Mac without it,
//!   `Avenir` bold is Avenir Heavy), the weight and italic pick the face,
//!   and a family that is not installed gives `None`: the caller then uses
//!   the bundled font (screen) or Helvetica (PDF), with a one-time note
//!   ([`take_notes`]). `Chief Blueprint` (a drafting font nobody has) is
//!   always the bundled font.
//! * **Screen**: [`font_id`] gives egui a [`FontId`] in the face's own
//!   family. The bytes are loaded lazily, the first time a style asks for
//!   the face, into egui's font definitions (a font set in one frame is
//!   usable from the next: until then the bundled font is drawn).
//! * **Paper**: [`SystemFontSource`] hands the same faces to the PDF writer,
//!   which embeds a subset of each (see `plan_docs::pdf`). Installed fonts
//!   are embedded only in PDFs the user makes on this machine; a font whose
//!   licence forbids embedding (`OS/2` `fsType`) or one with PostScript
//!   outlines is printed in Helvetica instead.
//!
//! Preferences > Fonts holds the one switch: use system fonts at all.

use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId};
use plan_core::text_styles::{normalize_font_name, split_font_name};
use plan_docs::pdf::truetype::{scan_faces, Outlines};
use plan_docs::pdf::{FontFace, FontSource, FontSpec};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

// ------------------------------------------------------------ catalog --

/// One installed face.
#[derive(Debug, Clone, PartialEq)]
pub struct SystemFace {
    /// Family: `Avenir`.
    pub family: String,
    /// Style within the family: `Book`, `Heavy Oblique`.
    pub style: String,
    pub full_name: String,
    /// `usWeightClass` (400 regular, 700 bold).
    pub weight: u16,
    pub italic: bool,
    pub outlines: Outlines,
    pub embeddable: bool,
    pub path: PathBuf,
    /// Face index inside a `.ttc` (0 for a single font).
    pub index: u32,
}

impl SystemFace {
    /// The egui family name this face is registered under.
    pub fn egui_name(&self) -> String {
        format!("{} {} #{}", self.family, self.style, self.index)
    }
}

/// The installed faces.
#[derive(Debug, Clone, Default)]
pub struct FontCatalog {
    faces: Vec<SystemFace>,
}

/// The font folders of this machine.
pub fn font_dirs() -> Vec<PathBuf> {
    let home = crate::paths::home_dir();
    let mut dirs: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        dirs.push("/System/Library/Fonts".into());
        dirs.push("/Library/Fonts".into());
        dirs.extend(home.iter().map(|h| h.join("Library/Fonts")));
    } else if cfg!(target_os = "windows") {
        dirs.push(r"C:\Windows\Fonts".into());
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(PathBuf::from(local).join(r"Microsoft\Windows\Fonts"));
        }
    } else {
        dirs.push("/usr/share/fonts".into());
        dirs.push("/usr/local/share/fonts".into());
        dirs.extend(home.iter().map(|h| h.join(".fonts")));
        dirs.extend(home.iter().map(|h| h.join(".local/share/fonts")));
    }
    dirs
}

/// Font files under `dir` (folders nested up to four deep), in name order.
fn font_files(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        let hidden = p
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'));
        if hidden {
            continue;
        }
        if p.is_dir() {
            if depth < 4 {
                font_files(&p, depth + 1, out);
            }
        } else if p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
            ["ttf", "otf", "ttc", "otc"].contains(&e.to_ascii_lowercase().as_str())
        }) {
            out.push(p);
        }
    }
}

impl FontCatalog {
    /// The faces of every font file under `dirs`. The first file to offer a
    /// family and style wins.
    pub fn scan(dirs: &[PathBuf]) -> Self {
        let mut files = Vec::new();
        for d in dirs {
            font_files(d, 0, &mut files);
        }
        let mut faces: Vec<SystemFace> = Vec::new();
        let mut seen: HashSet<(String, String)> = HashSet::new();
        for path in files {
            let Ok(mut f) = std::fs::File::open(&path) else {
                continue;
            };
            for (index, info) in scan_faces(&mut f) {
                let key = (
                    normalize_font_name(&info.family),
                    normalize_font_name(&info.style),
                );
                if info.family.is_empty() || !seen.insert(key) {
                    continue;
                }
                faces.push(SystemFace {
                    family: info.family,
                    style: info.style,
                    full_name: info.full_name,
                    weight: info.weight,
                    italic: info.italic,
                    outlines: info.outlines,
                    embeddable: info.embeddable,
                    path: path.clone(),
                    index,
                });
            }
        }
        faces.sort_by(|a, b| {
            normalize_font_name(&a.family)
                .cmp(&normalize_font_name(&b.family))
                .then(a.weight.cmp(&b.weight))
                .then(a.italic.cmp(&b.italic))
                .then(a.style.cmp(&b.style))
        });
        Self { faces }
    }

    /// The catalog of this machine's font folders.
    pub fn system() -> Self {
        Self::scan(&font_dirs())
    }

    #[cfg(test)]
    pub fn faces(&self) -> &[SystemFace] {
        &self.faces
    }

    /// Installed families, sorted, each once.
    pub fn families(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for f in &self.faces {
            if out.last().is_none_or(|l| !same(l, &f.family)) {
                out.push(f.family.clone());
            }
        }
        out
    }

    pub fn has_family(&self, name: &str) -> bool {
        self.faces.iter().any(|f| same(&f.family, name))
    }

    /// The faces of one family.
    pub fn faces_of(&self, family: &str) -> Vec<&SystemFace> {
        self.faces
            .iter()
            .filter(|f| same(&f.family, family))
            .collect()
    }

    /// The face a request maps to: the first installed family of
    /// [`family_candidates`], then the face of it that fits the style, bold
    /// and italic best. `None` when no candidate family is installed.
    pub fn find(&self, spec: &FontSpec) -> Option<&SystemFace> {
        for cand in family_candidates(&spec.family) {
            let faces = self.faces_of(&cand);
            if !faces.is_empty() {
                return pick(&faces, spec);
            }
        }
        None
    }

    /// Whether the family a request names is installed itself (not just a
    /// stand-in for it).
    pub fn has_exact(&self, spec: &FontSpec) -> bool {
        let (fam, _) = split_font_name(&spec.family);
        self.has_family(&spec.family) || self.has_family(fam)
    }
}

fn same(a: &str, b: &str) -> bool {
    normalize_font_name(a) == normalize_font_name(b)
}

/// The families to try for a font name, best first: the name itself, then
/// the stand-ins the Chief fonts and the common office fonts get. Empty for
/// `Chief Blueprint`, which is always the bundled font.
pub fn family_candidates(name: &str) -> Vec<String> {
    let name = name.trim();
    let (family, _) = split_font_name(name);
    let key = normalize_font_name(family);
    if key.starts_with("chiefblueprint") {
        return Vec::new();
    }
    let alts: &[&str] = match key.as_str() {
        "avenir" => &[
            "Avenir Next",
            "Helvetica Neue",
            "Helvetica",
            "Arial",
            "Liberation Sans",
            "DejaVu Sans",
        ],
        "arial" => &[
            "Helvetica Neue",
            "Helvetica",
            "Liberation Sans",
            "Arimo",
            "DejaVu Sans",
        ],
        "arialnarrow" => &["Liberation Sans Narrow", "Arial", "Helvetica Neue"],
        "helvetica" => &["Helvetica Neue", "Arial", "Liberation Sans"],
        "timesnewroman" => &["Times", "Liberation Serif", "Georgia"],
        "couriernew" => &["Courier", "Liberation Mono", "Menlo"],
        "verdana" => &["DejaVu Sans", "Arial"],
        "georgia" => &["Times New Roman", "Times"],
        "calibri" => &["Carlito", "Helvetica Neue", "Arial"],
        _ => &[],
    };
    let mut out = vec![name.to_string()];
    if family != name {
        out.push(family.to_string());
    }
    out.extend(alts.iter().map(|s| s.to_string()));
    out
}

/// Words that name an ordinary upright face.
fn is_regular_word(style: &str) -> bool {
    matches!(
        normalize_font_name(style).as_str(),
        "" | "book" | "regular" | "roman" | "normal"
    )
}

/// The face of `faces` (one family) that fits `spec`: the style it names, else
/// the closest weight (bold is 700) with the right slant.
fn pick<'a>(faces: &[&'a SystemFace], spec: &FontSpec) -> Option<&'a SystemFace> {
    let mut style = spec.style.trim();
    // "Book" with the bold flag on means bold.
    if spec.bold && is_regular_word(style) {
        style = "";
    }
    // Chief's pairing: Avenir bold is Avenir Heavy.
    let (fam, _) = split_font_name(&spec.family);
    if style.is_empty() && spec.bold && same(fam, "Avenir") {
        style = "Heavy";
    }
    if !style.is_empty() {
        let s = normalize_font_name(style);
        let mut wanted: Vec<String> = Vec::new();
        if spec.italic {
            wanted.push(format!("{s}oblique"));
            wanted.push(format!("{s}italic"));
        }
        wanted.push(s);
        for w in wanted {
            if let Some(f) = faces.iter().find(|f| normalize_font_name(&f.style) == w) {
                return Some(f);
            }
        }
    }
    let target: i32 = if spec.bold { 700 } else { 400 };
    faces.iter().copied().min_by_key(|f| {
        let slant = if f.italic == spec.italic { 0 } else { 1000 };
        let weight = (i32::from(f.weight) - target).abs();
        (
            slant + weight,
            u8::from(f.outlines == Outlines::Cff),
            u8::from(!is_regular_word(&f.style)),
            f.style.len(),
        )
    })
}

static CATALOG_OVERRIDE: RwLock<Option<Arc<FontCatalog>>> = RwLock::new(None);

/// Replaces the catalog [`catalog`] returns (tests use a folder of sample
/// fonts); `None` goes back to this machine's.
#[cfg(test)]
pub fn set_catalog(catalog: Option<Arc<FontCatalog>>) {
    *CATALOG_OVERRIDE.write().unwrap_or_else(|e| e.into_inner()) = catalog;
}

/// The catalog of this machine, scanned the first time it is asked for.
pub fn catalog() -> Arc<FontCatalog> {
    if let Some(c) = CATALOG_OVERRIDE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
    {
        return c;
    }
    static CATALOG: OnceLock<Arc<FontCatalog>> = OnceLock::new();
    CATALOG
        .get_or_init(|| Arc::new(FontCatalog::system()))
        .clone()
}

/// Scans the font folders on a background thread so the first text style
/// that asks for a face does not wait for it.
#[cfg_attr(test, allow(dead_code))]
pub fn prefetch() {
    std::thread::spawn(|| {
        let _ = catalog();
    });
}

// ----------------------------------------------------------- font bytes --

/// A font file's bytes, read once and kept.
pub fn load_bytes(path: &Path) -> Option<Arc<Vec<u8>>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<Vec<u8>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Mutex::default);
    if let Some(b) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(path) {
        return Some(b.clone());
    }
    let bytes = Arc::new(std::fs::read(path).ok()?);
    cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(path.to_path_buf(), bytes.clone());
    Some(bytes)
}

// ---------------------------------------------------------- preferences --

static USE_SYSTEM: AtomicBool = AtomicBool::new(true);

/// Are installed fonts used (plan, layout and PDF)? On unless switched off in
/// Preferences > Fonts.
pub fn use_system_fonts() -> bool {
    USE_SYSTEM.load(Ordering::Relaxed)
}

/// Switches installed fonts on or off for this run (not saved).
pub fn set_use_system_fonts_for_run(on: bool) {
    USE_SYSTEM.store(on, Ordering::Relaxed);
    install_pdf_source();
}

/// `~/.plan-studio/fonts.json`.
fn prefs_path() -> Option<PathBuf> {
    crate::paths::user_file("fonts.json")
}

/// The saved switch (on when there is no file or it is unreadable).
pub fn read_prefs_at(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("use_system_fonts")?.as_bool())
        .unwrap_or(true)
}

/// Saves the switch.
pub fn write_prefs_at(path: &Path, on: bool) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::json!({ "use_system_fonts": on }).to_string();
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// Sets the switch for this run and saves it.
pub fn set_use_system_fonts(on: bool) -> Result<(), String> {
    set_use_system_fonts_for_run(on);
    match prefs_path() {
        Some(p) => write_prefs_at(&p, on),
        None => Err(crate::paths::NO_HOME.to_string()),
    }
}

// ---------------------------------------------------------------- notes --

#[derive(Default)]
struct Notes {
    /// Not yet taken by [`take_notes`].
    pending: Vec<String>,
    /// Families already noted (each is reported once).
    noted: HashSet<String>,
    /// Every note of this run, for Preferences > Fonts.
    history: Vec<String>,
}

fn notes() -> &'static Mutex<Notes> {
    static NOTES: OnceLock<Mutex<Notes>> = OnceLock::new();
    NOTES.get_or_init(Mutex::default)
}

/// Records, once per family, that a style's font is not installed.
fn note_missing(family: &str) {
    let mut n = notes().lock().unwrap_or_else(|e| e.into_inner());
    if n.noted.insert(normalize_font_name(family)) {
        let text = format!("Font \"{family}\" is not installed; the bundled font is used.");
        n.pending.push(text.clone());
        n.history.push(text);
    }
}

/// Takes the PDF writer's fallback notes into `n` (each once).
fn absorb(n: &mut Notes) {
    for note in plan_docs::pdf::take_font_notes() {
        if !n.history.contains(&note) {
            n.history.push(note.clone());
            n.pending.push(note);
        }
    }
}

/// The font notes since the last call: families that are not installed (the
/// bundled font is drawn) and fonts the PDF writer could not embed
/// (Helvetica is printed). Each is reported once.
pub fn take_notes() -> Vec<String> {
    let mut n = notes().lock().unwrap_or_else(|e| e.into_inner());
    absorb(&mut n);
    std::mem::take(&mut n.pending)
}

/// Puts the new font notes (if any) on the status line, once each: the shell
/// calls this every frame. Returns whether the status changed.
pub fn post_notes(status: &mut String) -> bool {
    status_from_notes(status, take_notes())
}

fn status_from_notes(status: &mut String, notes: Vec<String>) -> bool {
    if notes.is_empty() {
        return false;
    }
    *status = notes.join(" ");
    true
}

/// Every font note of this run (including those already taken).
pub fn notes_so_far() -> Vec<String> {
    let mut n = notes().lock().unwrap_or_else(|e| e.into_inner());
    absorb(&mut n);
    n.history.clone()
}

// ----------------------------------------------------------------- egui --

/// The spec for a font name (`Avenir`, `Avenir Book`) with bold and italic.
pub fn spec_named(font: &str, bold: bool, italic: bool) -> FontSpec {
    let (family, style) = split_font_name(font);
    FontSpec {
        family: family.to_string(),
        style: style.to_string(),
        bold,
        italic,
    }
}

/// The egui font for `spec` at `size`: the installed face when it is
/// available to this frame, else the bundled proportional font. A face seen
/// for the first time is loaded now and drawn from the next frame on.
pub fn font_id(ctx: &egui::Context, spec: &FontSpec, size: f32) -> FontId {
    font_id_with(ctx, &catalog(), spec, size)
}

/// [`font_id`] over a given catalog.
pub fn font_id_with(
    ctx: &egui::Context,
    catalog: &FontCatalog,
    spec: &FontSpec,
    size: f32,
) -> FontId {
    let bundled = FontId::proportional(size);
    if !use_system_fonts() || ctx.cumulative_pass_nr() == 0 {
        return bundled;
    }
    let Some(face) = catalog.find(spec) else {
        // A family that no candidate covers (and not the always-bundled
        // Chief Blueprint) is worth a note.
        if !family_candidates(&spec.family).is_empty() {
            note_missing(&spec.family);
        }
        return bundled;
    };
    let name = face.egui_name();
    if applied_families(ctx).contains(&name) {
        return FontId::new(size, FontFamily::Name(name.into()));
    }
    load_into(ctx, face);
    bundled
}

/// The font families this frame's egui knows, looked up once per pass.
fn applied_families(ctx: &egui::Context) -> Arc<HashSet<String>> {
    let id = egui::Id::new("plan_studio_font_families");
    let pass = ctx.cumulative_pass_nr();
    if let Some((p, set)) = ctx.data(|d| d.get_temp::<(u64, Arc<HashSet<String>>)>(id)) {
        if p == pass {
            return set;
        }
    }
    let set: HashSet<String> = ctx.fonts(|f| {
        f.families()
            .into_iter()
            .filter_map(|fam| match fam {
                FontFamily::Name(n) => Some(n.to_string()),
                _ => None,
            })
            .collect()
    });
    let set = Arc::new(set);
    ctx.data_mut(|d| d.insert_temp(id, (pass, set.clone())));
    set
}

/// Adds `face` to egui's font definitions (set for the next frame). Glyphs
/// the face lacks fall back to the bundled fonts.
fn load_into(ctx: &egui::Context, face: &SystemFace) -> bool {
    let id = egui::Id::new("plan_studio_font_defs");
    let mut defs: FontDefinitions = ctx
        .data(|d| d.get_temp::<FontDefinitions>(id))
        .unwrap_or_default();
    let name = face.egui_name();
    if defs.font_data.contains_key(&name) {
        // Requested already; the definitions take effect next frame.
        return true;
    }
    let Some(bytes) = load_bytes(&face.path) else {
        note_missing(&face.family);
        return false;
    };
    defs.font_data.insert(
        name.clone(),
        Arc::new(FontData {
            font: Cow::Owned(bytes.as_ref().clone()),
            index: face.index,
            tweak: Default::default(),
        }),
    );
    let mut chain = vec![name.clone()];
    chain.extend(
        defs.families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap_or_default(),
    );
    defs.families.insert(FontFamily::Name(name.into()), chain);
    ctx.data_mut(|d| d.insert_temp(id, defs.clone()));
    ctx.set_fonts(defs);
    ctx.request_repaint();
    true
}

/// Called when the chrome is applied: reads the saved switch, starts the
/// font scan in the background and hands the PDF writer its font source.
pub fn install(ctx: &egui::Context) {
    // Tests keep to the defaults: no reading of the user's files, no scan,
    // and PDFs that do not depend on the fonts of the machine.
    #[cfg(not(test))]
    {
        static ONCE: OnceLock<()> = OnceLock::new();
        ONCE.get_or_init(|| {
            if let Some(p) = prefs_path() {
                USE_SYSTEM.store(read_prefs_at(&p), Ordering::Relaxed);
            }
            install_pdf_source();
            prefetch();
        });
    }
    // The bundled fonts stay as egui has them until a style asks for a face.
    let _ = ctx;
}

// ------------------------------------------------------------------ PDF --

/// The installed fonts as the PDF writer's font source.
#[derive(Debug, Default)]
pub struct SystemFontSource {
    /// A catalog of its own (tests); `None`: this machine's.
    catalog: Option<Arc<FontCatalog>>,
}

impl SystemFontSource {
    #[cfg(test)]
    pub fn with_catalog(catalog: Arc<FontCatalog>) -> Self {
        Self {
            catalog: Some(catalog),
        }
    }
}

impl FontSource for SystemFontSource {
    fn face(&self, spec: &FontSpec) -> Option<FontFace> {
        if !use_system_fonts() {
            return None;
        }
        let catalog = self.catalog.clone().unwrap_or_else(catalog);
        let face = catalog.find(spec)?;
        Some(FontFace {
            data: load_bytes(&face.path)?,
            index: face.index,
        })
    }
}

/// Gives every new PDF the installed fonts (none while they are switched off).
pub fn install_pdf_source() {
    let src: Option<Arc<dyn FontSource>> =
        use_system_fonts().then(|| Arc::new(SystemFontSource::default()) as Arc<dyn FontSource>);
    plan_docs::pdf::set_default_font_source(src);
}

// -------------------------------------------------------------- picker --

/// A sample line to judge a font by: letters, then a dimension.
pub const SAMPLE: &str = "Great Room 12'-6\" x 14'-0\"  AaBbCc 0123";

/// A font picker: a drop-down of the installed families (and the Chief
/// names, marked when absent), then the face the choice, `font_style`,
/// `bold` and `italic` map to, and a preview line set in it. Returns true
/// when `font` changed.
pub fn font_picker(
    ui: &mut egui::Ui,
    salt: &str,
    font: &mut String,
    font_style: &str,
    bold: bool,
    italic: bool,
) -> bool {
    let cat = catalog();
    let installed = cat.families();
    let mut changed = false;
    let filter_id = ui.make_persistent_id((salt, "font_filter"));
    let shown = if font.is_empty() {
        "(none)".to_string()
    } else {
        font.clone()
    };
    egui::ComboBox::from_id_salt((salt, "font"))
        .selected_text(shown)
        .width(220.0)
        .show_ui(ui, |ui| {
            let mut filter: String = ui.data(|d| d.get_temp(filter_id)).unwrap_or_default();
            ui.add(
                egui::TextEdit::singleline(&mut filter)
                    .hint_text("Search fonts")
                    .desired_width(200.0),
            );
            ui.data_mut(|d| d.insert_temp(filter_id, filter.clone()));
            let needle = normalize_font_name(&filter);
            let chief = ["Avenir", "Arial", "Arial Narrow", "Chief Blueprint"];
            let mut names: Vec<(String, bool)> = chief
                .iter()
                .filter(|c| !installed.iter().any(|i| same(i, c)))
                .map(|c| ((*c).to_string(), false))
                .collect();
            if !font.is_empty()
                && !installed.iter().any(|i| same(i, font))
                && !names.iter().any(|(n, _)| same(n, font))
            {
                names.push((font.clone(), false));
            }
            names.extend(installed.iter().map(|n| (n.clone(), true)));
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    for (name, present) in names {
                        if !needle.is_empty() && !normalize_font_name(&name).contains(&needle) {
                            continue;
                        }
                        let label = if present {
                            egui::RichText::new(&name)
                        } else {
                            egui::RichText::new(format!("{name} (not installed)")).weak()
                        };
                        if ui.selectable_label(same(font, &name), label).clicked() && *font != name
                        {
                            *font = name;
                            changed = true;
                        }
                    }
                });
        });
    let mut spec = spec_named(font, bold, italic);
    if !font_style.trim().is_empty() {
        spec.style = font_style.trim().to_string();
    }
    face_preview(ui, &spec);
    changed
}

/// Says which installed face `spec` maps to (or why the bundled font is
/// used) and shows [`SAMPLE`] set in it.
pub fn face_preview(ui: &mut egui::Ui, spec: &FontSpec) {
    let cat = catalog();
    let font = &spec.family;
    match cat.find(spec) {
        Some(face) if use_system_fonts() => {
            let stand_in = !cat.has_exact(spec);
            ui.weak(if stand_in {
                format!("{font} is not installed; {} is used.", face.full_name)
            } else {
                format!("Face: {}", face.full_name)
            });
            let id = font_id(ui.ctx(), spec, 16.0);
            ui.label(egui::RichText::new(SAMPLE).font(id));
        }
        _ if !use_system_fonts() => {
            ui.weak("System fonts are off (Preferences > Fonts): the bundled font is used.");
        }
        _ if family_candidates(font).is_empty() => {
            ui.weak("Chief Blueprint is a drafting font; the bundled font is used.");
        }
        _ => {
            ui.weak(format!(
                "{font} is not installed; the bundled font is used."
            ));
            ui.label(egui::RichText::new(SAMPLE));
        }
    }
}

// ---------------------------------------------------------------- tests --

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::TextStyle;
    use plan_docs::pdf::truetype::synthetic_font;
    use plan_docs::PdfDoc;

    /// A throw-away folder under the target dir's temp area.
    fn scratch(name: &str) -> PathBuf {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "plan-studio-fonts-{name}-{}-{n}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A `.ttc` of the given fonts (table offsets shifted to the file).
    fn make_ttc(fonts: &[Vec<u8>]) -> Vec<u8> {
        let head = 12 + 4 * fonts.len();
        let mut out = Vec::new();
        out.extend_from_slice(b"ttcf");
        out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        out.extend_from_slice(&(fonts.len() as u32).to_be_bytes());
        let mut bases = Vec::new();
        let mut at = head;
        for f in fonts {
            bases.push(at);
            out.extend_from_slice(&(at as u32).to_be_bytes());
            at += f.len();
        }
        for (f, base) in fonts.iter().zip(bases) {
            let mut f = f.clone();
            let n = u16::from_be_bytes([f[4], f[5]]) as usize;
            for i in 0..n {
                let o = 12 + 16 * i + 8;
                let off = u32::from_be_bytes([f[o], f[o + 1], f[o + 2], f[o + 3]]) + base as u32;
                f[o..o + 4].copy_from_slice(&off.to_be_bytes());
            }
            out.extend_from_slice(&f);
        }
        out
    }

    /// Avenir (in a `.ttc`: Book, Heavy, Book Oblique), Arial-less Helvetica
    /// Neue (Regular, Bold) and a restricted font, in a temp folder.
    fn sample_catalog() -> (FontCatalog, PathBuf) {
        let dir = scratch("catalog");
        let ttc = make_ttc(&[
            synthetic_font("Avenir", "Book", 400, 0),
            synthetic_font("Avenir", "Heavy", 900, 0),
            synthetic_font("Avenir", "Book Oblique", 400, 0),
        ]);
        std::fs::write(dir.join("Avenir.ttc"), ttc).unwrap();
        std::fs::create_dir_all(dir.join("Supplemental")).unwrap();
        for (file, style, weight) in [("HN.ttf", "Regular", 400), ("HN-Bold.ttf", "Bold", 700)] {
            std::fs::write(
                dir.join("Supplemental").join(file),
                synthetic_font("Helvetica Neue", style, weight, 0),
            )
            .unwrap();
        }
        std::fs::write(
            dir.join("Locked.ttf"),
            synthetic_font("Locked", "Regular", 400, 2),
        )
        .unwrap();
        std::fs::write(dir.join("readme.txt"), "not a font").unwrap();
        std::fs::write(dir.join("broken.ttf"), b"junk").unwrap();
        (FontCatalog::scan(std::slice::from_ref(&dir)), dir)
    }

    #[test]
    fn scanning_reads_family_style_weight_from_ttf_and_ttc_files() {
        let (cat, dir) = sample_catalog();
        assert_eq!(cat.families(), vec!["Avenir", "Helvetica Neue", "Locked"]);
        let av = cat.faces_of("avenir");
        assert_eq!(av.len(), 3);
        let styles: Vec<(&str, u16, u32)> = av
            .iter()
            .map(|f| (f.style.as_str(), f.weight, f.index))
            .collect();
        assert!(styles.contains(&("Book", 400, 0)));
        assert!(styles.contains(&("Heavy", 900, 1)));
        assert!(styles.contains(&("Book Oblique", 400, 2)));
        assert!(!cat.faces_of("Locked")[0].embeddable);
        assert!(cat.has_family("Helvetica Neue") && !cat.has_family("Arial"));
        // Files nested in a folder are found; junk is skipped.
        assert_eq!(cat.faces_of("Helvetica Neue").len(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn chief_names_map_to_real_faces() {
        let (cat, dir) = sample_catalog();
        let find = |font: &str, bold: bool, italic: bool| {
            cat.find(&spec_named(font, bold, italic))
                .map(|f| (f.family.clone(), f.style.clone()))
        };
        let pair = |a: &str, b: &str| Some((a.to_string(), b.to_string()));
        // Avenir Book is the Book face; plain Avenir the same.
        assert_eq!(find("Avenir Book", false, false), pair("Avenir", "Book"));
        assert_eq!(find("Avenir", false, false), pair("Avenir", "Book"));
        // The bold flag on Avenir gives Chief's Heavy, with or without "Book".
        assert_eq!(find("Avenir", true, false), pair("Avenir", "Heavy"));
        assert_eq!(find("Avenir Book", true, false), pair("Avenir", "Heavy"));
        assert_eq!(find("Avenir Heavy", false, false), pair("Avenir", "Heavy"));
        // Italic asks for the oblique face.
        assert_eq!(find("Avenir", false, true), pair("Avenir", "Book Oblique"));
        // Arial is not installed here: Helvetica Neue stands in, bold by weight.
        assert_eq!(
            find("Arial", false, false),
            pair("Helvetica Neue", "Regular")
        );
        assert_eq!(find("Arial", true, false), pair("Helvetica Neue", "Bold"));
        // Chief Blueprint is always the bundled font; unknown families too.
        assert_eq!(find("Chief Blueprint", false, false), None);
        assert!(family_candidates("Chief Blueprint").is_empty());
        assert_eq!(find("Papyrus", false, false), None);
        // Candidate chains.
        assert_eq!(family_candidates("Arial")[0], "Arial");
        assert!(family_candidates("Arial").contains(&"Helvetica Neue".to_string()));
        assert_eq!(family_candidates("Papyrus"), vec!["Papyrus"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_text_style_maps_through_its_font_and_flags() {
        let (cat, dir) = sample_catalog();
        let st = TextStyle::plan_sized("Room Label Style", 6.0, true)
            .with_font("Avenir")
            .with_italic(false);
        let f = cat.find(&FontSpec::of_style(&st).unwrap()).unwrap();
        assert_eq!(f.style, "Heavy");
        let st = TextStyle::plan_sized("x", 6.0, false)
            .with_font("Avenir")
            .with_font_style("Heavy");
        assert_eq!(
            cat.find(&FontSpec::of_style(&st).unwrap()).unwrap().style,
            "Heavy"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn egui_gets_the_installed_face_from_the_next_frame() {
        let (cat, dir) = sample_catalog();
        let ctx = egui::Context::default();
        let spec = spec_named("Avenir", true, false);
        let mut seen: Vec<FontId> = Vec::new();
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                let id = font_id_with(ctx, &cat, &spec, 14.0);
                // Drawing with whatever came back never fails.
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.label(egui::RichText::new("AB 12'-6\"").font(id.clone()));
                });
                seen.push(id);
            });
        }
        // The first frame loads the face and draws the bundled font ...
        assert_eq!(seen[0].family, FontFamily::Proportional);
        // ... then the face's own family is used.
        assert_eq!(
            seen.last().unwrap().family,
            FontFamily::Name("Avenir Heavy #1".into())
        );
        assert_eq!(seen.last().unwrap().size, 14.0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn font_notes_reach_the_status_line_once_each() {
        let mut status = "Ready".to_string();
        assert!(!status_from_notes(&mut status, Vec::new()));
        assert_eq!(status, "Ready");
        let notes = vec![
            "Font \"A\" is not installed; the bundled font is used.".to_string(),
            "Font \"B\" does not allow embedding.".to_string(),
        ];
        assert!(status_from_notes(&mut status, notes));
        assert!(
            status.contains("\"A\"") && status.contains("\"B\""),
            "{status}"
        );
        // The real queue is drained by taking: a second post has nothing.
        let mut other = "Ready".to_string();
        let _ = post_notes(&mut other);
        assert!(take_notes()
            .iter()
            .all(|n| !n.contains("Font \"A\" is not")));
    }

    #[test]
    fn a_missing_family_falls_back_to_the_bundled_font_with_one_note() {
        let (cat, dir) = sample_catalog();
        let ctx = egui::Context::default();
        let spec = spec_named("Vanished Display", false, false);
        let mut ids = Vec::new();
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                ids.push(font_id_with(ctx, &cat, &spec, 12.0));
            });
        }
        assert!(ids.iter().all(|i| i.family == FontFamily::Proportional));
        let notes = take_notes();
        let mine: Vec<_> = notes
            .iter()
            .filter(|n| n.contains("Vanished Display"))
            .collect();
        assert_eq!(mine.len(), 1, "{notes:?}");
        assert!(mine[0].contains("is not installed"));
        // Reported once: asking again adds nothing.
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            font_id_with(ctx, &cat, &spec, 12.0);
        });
        assert!(take_notes().iter().all(|n| !n.contains("Vanished Display")));
        // Chief Blueprint is bundled by design: no note.
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            font_id_with(
                ctx,
                &cat,
                &spec_named("Chief Blueprint", false, false),
                12.0,
            );
        });
        assert!(take_notes().iter().all(|n| !n.contains("Chief Blueprint")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_pdf_source_embeds_the_matched_face_and_refuses_restricted_fonts() {
        let (cat, dir) = sample_catalog();
        let src = Arc::new(SystemFontSource::with_catalog(Arc::new(cat)));
        let mut d = PdfDoc::new(100.0, 50.0);
        d.set_font_source(Some(src));
        // Avenir bold: the Heavy face from face index 1 of the .ttc.
        assert!(d.use_font(Some(&spec_named("Avenir", true, false))));
        d.text(5.0, 5.0, 10.0, "AB");
        // Arial: Helvetica Neue stands in.
        assert!(d.use_font(Some(&spec_named("Arial", false, false))));
        d.text(5.0, 20.0, 10.0, "A");
        // Licence forbids embedding: Helvetica with a note.
        assert!(!d.use_font(Some(&spec_named("Locked", false, false))));
        assert!(
            d.font_notes()[0].contains("does not allow embedding"),
            "{:?}",
            d.font_notes()
        );
        assert_eq!(d.embedded_font_count(), 2);
        let pdf = d.finish();
        let t = String::from_utf8_lossy(&pdf);
        assert_eq!(t.matches("/FontFile2").count(), 2);
        assert!(t.contains("Avenir-Heavy"));
        assert!(t.contains("Helvetica-Neue-Regular"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn prefs_round_trip_and_default_on() {
        let dir = scratch("prefs");
        let p = dir.join("fonts.json");
        assert!(read_prefs_at(&p), "no file: on");
        write_prefs_at(&p, false).unwrap();
        assert!(!read_prefs_at(&p));
        write_prefs_at(&p, true).unwrap();
        assert!(read_prefs_at(&p));
        std::fs::write(&p, "garbage").unwrap();
        assert!(read_prefs_at(&p), "unreadable: on");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_plan_draws_text_in_its_styles_font() {
        use crate::editor::{Camera, EditorContext};
        use plan_core::{CadItem, Point};
        let (cat, dir) = sample_catalog();
        set_catalog(Some(Arc::new(cat)));
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        for st in &mut cx.project.text_styles.styles {
            st.font = "Avenir".into();
            st.bold = st.name == "Room Label Style";
        }
        cx.project.add_cad(
            0,
            "Text",
            CadItem::Text {
                pos: Point::new(10.0, 10.0),
                text: "Kitchen".into(),
                height: 6.0,
                angle: 0.0,
            },
        );
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let (_, painter) =
                        ui.allocate_painter(egui::vec2(400.0, 300.0), egui::Sense::hover());
                    let mut cam = Camera::default_view();
                    cam.rect = painter.clip_rect();
                    cam.px_per_in = 2.0;
                    crate::editor::render::draw_plan(&cx, &painter, &cam);
                });
            });
        }
        let families: Vec<FontFamily> = ctx.fonts(|f| f.families());
        assert!(
            families.contains(&FontFamily::Name("Avenir Book #0".into())),
            "{families:?}"
        );
        set_catalog(None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_picker_draws_headlessly() {
        let ctx = egui::Context::default();
        let mut font = "Avenir".to_string();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    font_picker(ui, "t", &mut font, "", true, false);
                });
            });
        }
        assert_eq!(font, "Avenir");
    }

    /// Needs a Mac (or any machine with Arial and Avenir installed).
    #[test]
    #[ignore = "depends on the fonts installed on this machine"]
    fn this_machines_fonts() {
        let cat = FontCatalog::system();
        assert!(!cat.families().is_empty());
        for (font, bold) in [
            ("Arial", false),
            ("Arial", true),
            ("Avenir", false),
            ("Avenir", true),
        ] {
            let f = cat.find(&spec_named(font, bold, false));
            eprintln!(
                "{font} bold={bold}: {:?}",
                f.map(|f| (&f.full_name, &f.path, f.index))
            );
            assert!(f.is_some(), "{font}");
        }
        eprintln!(
            "{} families, {} faces",
            cat.families().len(),
            cat.faces().len()
        );
    }
}
