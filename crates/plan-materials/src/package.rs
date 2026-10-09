//! PBR material packages: the zip a manufacturer-texture site such as
//! Lightbeans (lightbeans.com/en/textures) hands out per material.
//!
//! A package is a seamless set of image maps (albedo / base colour, normal,
//! roughness, metallic, height / displacement, ambient occlusion, opacity)
//! and, from Lightbeans, a `productmetadata.txt` with the real-world size of
//! the scanned sample. Plan Studio never downloads or bundles these files: the
//! user downloads the zip, [`MaterialPackage::read`] opens it with our own zip
//! and JPEG / PNG decoders, and [`MaterialPackage::import_to`] copies the map
//! files (not moves) into `~/.plan-studio/textures/<package>/` so plans keep
//! working after the download is deleted.
//!
//! File names and metadata keys are matched tolerantly (case-insensitive
//! substrings, `key: value` or `key=value` lines, mm / cm / m / in units), as
//! the real layout is not published; see DECISIONS.md.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use plan_library::image::{self, Rgba8Image};

use crate::material::{MaterialClass, MaterialDef};

/// A decoded map.
pub type DecodedImage = Rgba8Image;

/// Default largest side of a map kept decoded for the GPU, pixels.
pub const DEFAULT_MAX_SIDE: u32 = 2048;
/// Inches per tile when the metadata gives no size.
pub const DEFAULT_TILE_IN: f64 = 24.0;
/// The folder under `~/.plan-studio/` that holds imported maps.
pub const TEXTURES_DIR: &str = "textures";
/// The metadata file's name inside a Lightbeans zip.
pub const METADATA_FILE: &str = "productmetadata.txt";

/// The kinds of map a package carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MapKind {
    Albedo,
    Normal,
    Roughness,
    Metallic,
    Height,
    Ao,
    Opacity,
}

impl MapKind {
    /// Every kind in the Texture tab's order.
    pub const ALL: [MapKind; 7] = [
        MapKind::Albedo,
        MapKind::Normal,
        MapKind::Roughness,
        MapKind::Metallic,
        MapKind::Height,
        MapKind::Ao,
        MapKind::Opacity,
    ];

    /// The stable key stored in `MaterialDef::maps_off` and used for file names.
    pub fn key(self) -> &'static str {
        match self {
            MapKind::Albedo => "albedo",
            MapKind::Normal => "normal",
            MapKind::Roughness => "roughness",
            MapKind::Metallic => "metallic",
            MapKind::Height => "height",
            MapKind::Ao => "ao",
            MapKind::Opacity => "opacity",
        }
    }

    /// The name shown in the dialog.
    pub fn label(self) -> &'static str {
        match self {
            MapKind::Albedo => "Albedo (base colour)",
            MapKind::Normal => "Normal",
            MapKind::Roughness => "Roughness",
            MapKind::Metallic => "Metallic",
            MapKind::Height => "Height / displacement",
            MapKind::Ao => "Ambient occlusion",
            MapKind::Opacity => "Opacity",
        }
    }
}

/// Splits a file stem into lower-case alphanumeric words.
fn tokens(stem: &str) -> Vec<String> {
    stem.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// The kind a single word of a file name stands for.
fn kind_of_word(w: &str) -> Option<MapKind> {
    if w.contains("opacity") || w.contains("alpha") || w == "mask" || w.contains("transparency") {
        return Some(MapKind::Opacity);
    }
    if w.contains("norm") || matches!(w, "nrm" | "nor" | "nrml") {
        return Some(MapKind::Normal);
    }
    if w.contains("rough") {
        return Some(MapKind::Roughness);
    }
    if w.contains("metal") {
        return Some(MapKind::Metallic);
    }
    if w.contains("height") || w.contains("disp") || w.contains("bump") {
        return Some(MapKind::Height);
    }
    if w == "ao" || w.contains("ambient") || w.contains("occlusion") {
        return Some(MapKind::Ao);
    }
    if w.contains("albedo")
        || w.contains("basecolor")
        || w.contains("color")
        || w.contains("colour")
        || w.contains("diffuse")
        || matches!(w, "col" | "diff" | "base")
    {
        return Some(MapKind::Albedo);
    }
    None
}

/// Words at the end of a name that say nothing about the kind.
fn is_filler(w: &str) -> bool {
    matches!(
        w,
        "gl" | "dx" | "opengl" | "directx" | "srgb" | "linear" | "raw" | "tiled" | "seamless"
    ) || (w.ends_with('k') && w.len() <= 3 && w[..w.len() - 1].chars().all(|c| c.is_ascii_digit()))
        || w.chars().all(|c| c.is_ascii_digit())
}

/// Which map a picture file name stands for, and whether a normal map is in
/// the DirectX convention. `None` for anything that is not a map (previews,
/// thumbnails, glossiness and the like). The product's name is usually part
/// of the file name, so the words nearest the end win ("Metal_Roof_Normal_GL"
/// is a normal map, not a metallic one).
pub fn classify(file_name: &str) -> Option<(MapKind, bool)> {
    let lower = file_name.to_lowercase();
    let (stem, ext) = lower.rsplit_once('.')?;
    if !matches!(ext, "png" | "jpg" | "jpeg") {
        return None;
    }
    let words = tokens(stem);
    let word = |w: &str| words.iter().any(|t| t == w);
    if word("preview")
        || word("thumb")
        || word("thumbnail")
        || word("swatch")
        || stem.contains("gloss")
    {
        return None;
    }
    let kind = words
        .iter()
        .rev()
        .filter(|w| !is_filler(w))
        .find_map(|w| kind_of_word(w))
        // CamelCase or run-together names ("BrickBaseColor"): the whole stem.
        .or_else(|| kind_of_word(&stem.replace(['_', '-', ' ', '.'], "")))?;
    let dx = kind == MapKind::Normal
        && (words.iter().any(|w| w == "dx" || w.contains("directx")) || stem.contains("normaldx"));
    Some((kind, dx))
}

// ---- metadata -----------------------------------------------------------------

/// `key: value` / `key=value` lines of a metadata file, keys lower-cased.
/// Quotes, trailing commas and braces (a JSON file) are tolerated.
pub fn parse_metadata(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        let Some(at) = line.find([':', '=']) else {
            continue;
        };
        let clean = |s: &str| {
            s.trim()
                .trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | '{' | '}' | '#' | ' '))
                .to_string()
        };
        let key = clean(&line[..at]).to_lowercase();
        let value = clean(&line[at + 1..]);
        if !key.is_empty() && !value.is_empty() {
            out.push((key, value));
        }
    }
    out
}

/// Inches per unit, for a unit word.
fn unit_in(word: &str) -> Option<f64> {
    match word.trim_matches(|c: char| !c.is_alphanumeric() && c != '"' && c != '\'') {
        "mm" | "millimeter" | "millimeters" | "millimetre" | "millimetres" => Some(1.0 / 25.4),
        "cm" | "centimeter" | "centimeters" | "centimetre" | "centimetres" => Some(1.0 / 2.54),
        "m" | "meter" | "meters" | "metre" | "metres" => Some(1.0 / 0.0254),
        "in" | "inch" | "inches" | "\"" => Some(1.0),
        "ft" | "foot" | "feet" | "'" => Some(12.0),
        _ => None,
    }
}

/// The numbers in `text` and the last unit word that follows or ends it.
fn numbers_and_unit(text: &str) -> (Vec<f64>, Option<f64>) {
    let mut numbers = Vec::new();
    let mut unit = None;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(char::is_ascii_digit)) {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_digit()
                    || chars[i] == '.'
                    || (chars[i] == ',' && chars.get(i + 1).is_some_and(char::is_ascii_digit)))
            {
                i += 1;
            }
            let raw: String = chars[start..i].iter().collect();
            if let Ok(v) = raw.replace(',', ".").parse::<f64>() {
                numbers.push(v);
            }
        } else if c.is_alphabetic() || c == '"' || c == '\'' {
            let start = i;
            while i < chars.len()
                && (chars[i].is_alphabetic() || chars[i] == '"' || chars[i] == '\'')
            {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect::<String>().to_lowercase();
            if let Some(u) = unit_in(&word) {
                unit = Some(u);
            }
        } else {
            i += 1;
        }
    }
    (numbers, unit)
}

/// The unit a metadata key announces ("width_mm", "Width (cm)", "size in m").
fn key_unit(key: &str) -> Option<f64> {
    key.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .rev()
        .find_map(unit_in)
}

/// The real-world size in inches `(width, height)` the metadata states.
pub fn size_from_metadata(meta: &[(String, String)]) -> Option<(f64, f64)> {
    let skip = [
        "disp",
        "bump",
        "relief",
        "depth",
        "thick",
        "map",
        "normal",
        "resolution",
        "pixel",
        "px",
    ];
    let mut width: Option<f64> = None;
    let mut height: Option<f64> = None;
    let mut both: Option<(f64, f64)> = None;
    for (key, value) in meta {
        if skip.iter().any(|s| key.contains(s)) {
            continue;
        }
        let is_w = key.contains("width") || key.contains("breadth");
        let is_h = key.contains("height") || key.contains("length");
        let is_s = key.contains("size") || key.contains("dimension");
        if !(is_w || is_h || is_s) {
            continue;
        }
        let (nums, unit) = numbers_and_unit(value);
        // No unit anywhere: scan metadata is millimetres.
        let per = unit.or_else(|| key_unit(key)).unwrap_or(1.0 / 25.4);
        let Some(&first) = nums.first() else { continue };
        if first <= 0.0 {
            continue;
        }
        if is_w && !is_s {
            width.get_or_insert(first * per);
        } else if is_h && !is_s {
            height.get_or_insert(first * per);
        } else if both.is_none() {
            let second = nums.get(1).copied().filter(|v| *v > 0.0).unwrap_or(first);
            both = Some((first * per, second * per));
        }
    }
    match (width, height, both) {
        (Some(w), Some(h), _) => Some((w, h)),
        (Some(w), None, _) => Some((w, w)),
        (None, Some(h), _) => Some((h, h)),
        (None, None, b) => b,
    }
}

fn meta_value<'a>(meta: &'a [(String, String)], keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|k| {
        meta.iter()
            .find(|(mk, v)| mk.as_str() == *k && !v.is_empty())
            .map(|(_, v)| v.as_str())
    })
}

// ---- the package ----------------------------------------------------------------

/// The decoded maps of a package, each at most the read size's longest side.
#[derive(Debug, Clone, Default)]
pub struct Maps {
    pub albedo: Option<DecodedImage>,
    pub normal: Option<DecodedImage>,
    pub roughness: Option<DecodedImage>,
    pub metallic: Option<DecodedImage>,
    pub height: Option<DecodedImage>,
    pub ao: Option<DecodedImage>,
    pub opacity: Option<DecodedImage>,
}

impl Maps {
    /// The map of `kind`.
    pub fn get(&self, kind: MapKind) -> Option<&DecodedImage> {
        match kind {
            MapKind::Albedo => self.albedo.as_ref(),
            MapKind::Normal => self.normal.as_ref(),
            MapKind::Roughness => self.roughness.as_ref(),
            MapKind::Metallic => self.metallic.as_ref(),
            MapKind::Height => self.height.as_ref(),
            MapKind::Ao => self.ao.as_ref(),
            MapKind::Opacity => self.opacity.as_ref(),
        }
    }

    fn slot(&mut self, kind: MapKind) -> &mut Option<DecodedImage> {
        match kind {
            MapKind::Albedo => &mut self.albedo,
            MapKind::Normal => &mut self.normal,
            MapKind::Roughness => &mut self.roughness,
            MapKind::Metallic => &mut self.metallic,
            MapKind::Height => &mut self.height,
            MapKind::Ao => &mut self.ao,
            MapKind::Opacity => &mut self.opacity,
        }
    }
}

/// One map file of the package as it came out of the zip.
#[derive(Debug, Clone)]
pub struct PackageFile {
    pub kind: MapKind,
    /// The file's name inside the zip.
    pub file_name: String,
    /// The original, undecoded bytes (what [`MaterialPackage::import_to`] copies).
    pub bytes: Vec<u8>,
    /// Pixel size of the full-resolution file.
    pub full_size: (u32, u32),
    /// A normal map in the DirectX convention.
    pub dx_normal: bool,
}

impl PackageFile {
    /// The file's extension, lower-case, `png` when it has none.
    pub fn extension(&self) -> String {
        Path::new(&self.file_name)
            .extension()
            .and_then(|e| e.to_str())
            .map_or_else(|| "png".to_string(), str::to_lowercase)
    }
}

/// How a package is read.
#[derive(Debug, Clone, Copy)]
pub struct ReadOptions {
    /// Maps bigger than this are box-filtered down for the GPU.
    pub max_side: u32,
}

impl Default for ReadOptions {
    fn default() -> Self {
        Self {
            max_side: DEFAULT_MAX_SIDE,
        }
    }
}

/// A material package read from a zip.
#[derive(Debug, Clone)]
pub struct MaterialPackage {
    /// The product's name (metadata, else the zip's name).
    pub name: String,
    pub manufacturer: String,
    /// Decoded maps, downsampled to [`ReadOptions::max_side`].
    pub maps: Maps,
    /// The raw files the maps were decoded from.
    pub files: Vec<PackageFile>,
    /// Real-world size of the scan, inches `(width, height)`.
    pub real_size_in: Option<(f64, f64)>,
    /// The zip this was read from, when it came from a file.
    pub source_zip: Option<PathBuf>,
    /// The folder the zip lives in.
    pub source_dir: PathBuf,
    /// The metadata file's `key: value` pairs.
    pub metadata: Vec<(String, String)>,
    /// Things worth telling the user (maps skipped, size unknown).
    pub warnings: Vec<String>,
}

/// Why a zip is not a material package.
pub type PackageError = String;

fn bad_entry(name: &str) -> bool {
    name.starts_with("__MACOSX/")
        || name.contains("/__MACOSX/")
        || name.rsplit('/').next().is_some_and(|b| b.starts_with("._"))
}

/// Does the zip at `path` hold a `productmetadata.txt`? (Reads only its
/// central directory.)
pub fn is_lightbeans_zip(path: &Path) -> bool {
    plan_calib::ZipArchive::open(path).is_ok_and(|z| z.find_basename(METADATA_FILE).is_some())
}

/// Cheap header read of an image's pixel size.
fn image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.starts_with(&image::png::SIGNATURE) && bytes.len() >= 24 {
        let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
        let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
        return Some((w, h));
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2;
        while i + 9 < bytes.len() {
            if bytes[i] != 0xFF {
                i += 1;
                continue;
            }
            let marker = bytes[i + 1];
            if marker == 0xFF || marker == 0x00 || (0xD0..=0xD9).contains(&marker) {
                i += if marker == 0xFF { 1 } else { 2 };
                continue;
            }
            let len = usize::from(u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]));
            if matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
                let h = u32::from(u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]));
                let w = u32::from(u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]));
                return Some((w, h));
            }
            i += 2 + len;
        }
    }
    None
}

impl MaterialPackage {
    /// Reads the zip at `path` (default options).
    pub fn read_path(path: &Path) -> Result<MaterialPackage, PackageError> {
        Self::read_path_with(path, &ReadOptions::default())
    }

    /// Reads the zip at `path`.
    pub fn read_path_with(
        path: &Path,
        opts: &ReadOptions,
    ) -> Result<MaterialPackage, PackageError> {
        let zip =
            plan_calib::ZipArchive::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let fallback = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut pkg = Self::from_archive(&zip, &fallback, opts)?;
        pkg.source_zip = Some(path.to_path_buf());
        pkg.source_dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        Ok(pkg)
    }

    /// Reads a zip held in memory (default options). The bytes are written to
    /// a temporary file because the zip reader seeks in files.
    pub fn read(zip_bytes: &[u8]) -> Result<MaterialPackage, PackageError> {
        Self::read_with(zip_bytes, "", &ReadOptions::default())
    }

    /// [`MaterialPackage::read`] with a fallback name and options.
    pub fn read_with(
        zip_bytes: &[u8],
        fallback_name: &str,
        opts: &ReadOptions,
    ) -> Result<MaterialPackage, PackageError> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let tmp = std::env::temp_dir().join(format!(
            "plan-studio-package-{}-{}.zip",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&tmp, zip_bytes).map_err(|e| e.to_string())?;
        let zip = plan_calib::ZipArchive::open(&tmp).map_err(|e| e.to_string());
        let out = zip.and_then(|z| Self::from_archive(&z, fallback_name, opts));
        let _ = std::fs::remove_file(&tmp);
        out
    }

    fn from_archive(
        zip: &plan_calib::ZipArchive,
        fallback_name: &str,
        opts: &ReadOptions,
    ) -> Result<MaterialPackage, PackageError> {
        let entries: Vec<_> = zip
            .entries()
            .iter()
            .filter(|e| !e.is_dir() && !bad_entry(&e.name))
            .collect();
        // Metadata.
        let mut metadata = Vec::new();
        let meta_entry = entries
            .iter()
            .find(|e| e.basename().eq_ignore_ascii_case(METADATA_FILE))
            .or_else(|| {
                entries.iter().find(|e| {
                    let b = e.basename().to_lowercase();
                    b.contains("metadata") && (b.ends_with(".txt") || b.ends_with(".json"))
                })
            });
        if let Some(e) = meta_entry {
            if let Ok(bytes) = zip.read(e) {
                metadata = parse_metadata(&String::from_utf8_lossy(&bytes));
            }
        }
        // Pick one file per kind: the largest, a GL normal over a DX one.
        let mut best: BTreeMap<MapKind, (&plan_calib::ZipEntry, bool)> = BTreeMap::new();
        for e in &entries {
            let Some((kind, dx)) = classify(e.basename()) else {
                continue;
            };
            let better = match best.get(&kind) {
                None => true,
                Some((cur, cur_dx)) => {
                    if kind == MapKind::Normal && *cur_dx != dx {
                        !dx
                    } else {
                        e.uncompressed_size > cur.uncompressed_size
                    }
                }
            };
            if better {
                best.insert(kind, (*e, dx));
            }
        }
        if !best.contains_key(&MapKind::Albedo) {
            return Err(
                "no albedo / base colour image found in the zip (is this a texture package?)"
                    .to_string(),
            );
        }
        let mut pkg = MaterialPackage {
            name: String::new(),
            manufacturer: String::new(),
            maps: Maps::default(),
            files: Vec::new(),
            real_size_in: size_from_metadata(&metadata),
            source_zip: None,
            source_dir: PathBuf::new(),
            metadata,
            warnings: Vec::new(),
        };
        for (kind, (entry, dx)) in best {
            let bytes = match zip.read(entry) {
                Ok(b) => b,
                Err(e) => {
                    pkg.warnings
                        .push(format!("{}: could not be read ({e})", entry.basename()));
                    continue;
                }
            };
            let full_size = image_size(&bytes).unwrap_or((0, 0));
            match image::decode(&bytes) {
                Ok(img) => {
                    let full = (img.width, img.height);
                    *pkg.maps.slot(kind) = Some(img.downscaled(opts.max_side));
                    pkg.files.push(PackageFile {
                        kind,
                        file_name: entry.basename().to_string(),
                        bytes,
                        full_size: full,
                        dx_normal: dx && kind == MapKind::Normal,
                    });
                }
                Err(e) => {
                    pkg.warnings.push(format!(
                        "{} ({} map, {}x{}): {e}",
                        entry.basename(),
                        kind.key(),
                        full_size.0,
                        full_size.1
                    ));
                }
            }
        }
        if pkg.maps.albedo.is_none() {
            return Err(format!(
                "the albedo image could not be decoded: {}",
                pkg.warnings.join("; ")
            ));
        }
        pkg.manufacturer = meta_value(
            &pkg.metadata,
            &["manufacturer", "brand", "producer", "vendor", "company"],
        )
        .unwrap_or("")
        .to_string();
        let top_dir = entries
            .iter()
            .find_map(|e| e.name.split_once('/').map(|(d, _)| d.to_string()));
        pkg.name = meta_value(
            &pkg.metadata,
            &[
                "name",
                "product name",
                "productname",
                "product_name",
                "product",
                "material name",
                "material",
                "title",
            ],
        )
        .map(str::to_string)
        .or_else(|| Some(fallback_name.to_string()).filter(|n| !n.trim().is_empty()))
        .or(top_dir)
        .unwrap_or_else(|| "Material Package".to_string());
        if pkg.real_size_in.is_none() {
            pkg.warnings.push(format!(
                "no real-world size in the metadata: using {DEFAULT_TILE_IN}\" tiles"
            ));
        }
        Ok(pkg)
    }

    /// Does the package carry a map of `kind`?
    pub fn has(&self, kind: MapKind) -> bool {
        self.files.iter().any(|f| f.kind == kind)
    }

    /// The file of `kind`.
    pub fn file(&self, kind: MapKind) -> Option<&PackageFile> {
        self.files.iter().find(|f| f.kind == kind)
    }

    /// Copies the package's map files into `<root>/<package name>/` as
    /// `albedo.jpg`, `normal.png` ... plus a `package.json` that records the
    /// source and the date. An existing folder of the same name is reused
    /// (its files are overwritten).
    pub fn import_to(&self, root: &Path, today: &str) -> Result<ImportedPackage, String> {
        let folder = safe_folder_name(&self.name);
        let dir = root.join(folder);
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let mut files = Vec::new();
        for f in &self.files {
            let path = dir.join(format!("{}.{}", f.kind.key(), f.extension()));
            std::fs::write(&path, &f.bytes).map_err(|e| format!("{}: {e}", path.display()))?;
            files.push((f.kind, path));
        }
        let record = serde_json::json!({
            "name": self.name,
            "manufacturer": self.manufacturer,
            "source": self.source_zip.as_ref().map(|p| p.display().to_string()),
            "imported": today,
            "real_size_in": self.real_size_in,
            "maps": files.iter().map(|(k, p)| (k.key(), p.file_name().map(|n| n.to_string_lossy().into_owned()))).collect::<BTreeMap<_, _>>(),
        });
        let text = serde_json::to_string_pretty(&record).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("package.json"), text)
            .map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(ImportedPackage { dir, files })
    }
}

/// A package copied into the user's textures folder.
#[derive(Debug, Clone)]
pub struct ImportedPackage {
    pub dir: PathBuf,
    pub files: Vec<(MapKind, PathBuf)>,
}

impl ImportedPackage {
    /// The copied file of `kind`.
    pub fn path(&self, kind: MapKind) -> Option<&Path> {
        self.files
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, p)| p.as_path())
    }
}

/// A folder name made of the characters every file system accepts.
pub fn safe_folder_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '(' | ')') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim().trim_start_matches('.').to_string();
    if s.is_empty() {
        "package".to_string()
    } else {
        s
    }
}

/// `YYYY-MM-DD` of a Unix time (UTC).
pub fn iso_date(unix_secs: u64) -> String {
    // Civil-from-days (Howard Hinnant).
    let z = (unix_secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// Today's date, `YYYY-MM-DD` (UTC).
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    iso_date(secs)
}

impl MaterialDef {
    /// A library material for an imported package: the albedo as its texture
    /// (tile size from the real-world size), class General, the maker as
    /// manufacturer, the extra maps as file paths, and where it came from.
    pub fn from_package(
        pkg: &MaterialPackage,
        imported: &ImportedPackage,
        today: &str,
    ) -> MaterialDef {
        let color = pkg.maps.albedo.as_ref().map_or([200, 200, 200], |i| {
            let a = i.average_color();
            [a[0], a[1], a[2]]
        });
        let mut def = MaterialDef::new(&pkg.name, &["Lightbeans"], color);
        def.set_class(MaterialClass::General);
        let (w, h) = pkg
            .real_size_in
            .unwrap_or((DEFAULT_TILE_IN, DEFAULT_TILE_IN));
        def.texture_scale_in = (w, h);
        def.manufacturer = pkg.manufacturer.clone();
        let path = |k: MapKind| imported.path(k).map(|p| p.display().to_string());
        def.texture_path = path(MapKind::Albedo);
        def.normal_map = path(MapKind::Normal);
        def.roughness_map = path(MapKind::Roughness);
        def.metallic_map = path(MapKind::Metallic);
        def.height_map = path(MapKind::Height);
        def.ao_map = path(MapKind::Ao);
        def.opacity_map = path(MapKind::Opacity);
        def.normal_flip_y = pkg.file(MapKind::Normal).is_some_and(|f| f.dx_normal);
        def.package_source = pkg.source_zip.as_ref().map(|p| p.display().to_string());
        def.package_imported = Some(today.to_string());
        // The bump slider scales the normals; 0.5 is as authored.
        def.bump = if def.normal_map.is_some() || def.height_map.is_some() {
            0.5
        } else {
            0.0
        };
        def
    }

    /// The image file of `kind`, if the material has one.
    pub fn map_path(&self, kind: MapKind) -> Option<&str> {
        match kind {
            MapKind::Albedo => self.texture_path.as_deref(),
            MapKind::Normal => self.normal_map.as_deref(),
            MapKind::Roughness => self.roughness_map.as_deref(),
            MapKind::Metallic => self.metallic_map.as_deref(),
            MapKind::Height => self.height_map.as_deref(),
            MapKind::Ao => self.ao_map.as_deref(),
            MapKind::Opacity => self.opacity_map.as_deref(),
        }
    }

    /// Is the map of `kind` present and not switched off?
    pub fn map_enabled(&self, kind: MapKind) -> bool {
        self.map_path(kind).is_some() && !self.maps_off.iter().any(|k| k == kind.key())
    }

    /// Switches the map of `kind` on or off (the Texture tab's checkboxes).
    pub fn set_map_enabled(&mut self, kind: MapKind, on: bool) {
        self.maps_off.retain(|k| k != kind.key());
        if !on {
            self.maps_off.push(kind.key().to_string());
        }
    }

    /// Does this material come from a package with maps beyond the albedo?
    pub fn has_pbr_maps(&self) -> bool {
        MapKind::ALL
            .into_iter()
            .filter(|k| *k != MapKind::Albedo)
            .any(|k| self.map_path(k).is_some())
    }
}

/// A store-only (no compression) zip of `entries` `(name, bytes)`: the
/// synthetic packages of the tests are built with it. Not a general writer.
pub fn store_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    fn le16(out: &mut Vec<u8>, v: u16) {
        out.extend_from_slice(&v.to_le_bytes());
    }
    fn le32(out: &mut Vec<u8>, v: u32) {
        out.extend_from_slice(&v.to_le_bytes());
    }
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let offset = out.len() as u32;
        let crc = plan_calib::zip::crc32(data);
        // Local file header.
        le32(&mut out, 0x0403_4b50);
        le16(&mut out, 20);
        le16(&mut out, 0);
        le16(&mut out, 0);
        le16(&mut out, 0);
        le16(&mut out, 0x21);
        le32(&mut out, crc);
        le32(&mut out, data.len() as u32);
        le32(&mut out, data.len() as u32);
        le16(&mut out, name.len() as u16);
        le16(&mut out, 0);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        // Central directory entry.
        le32(&mut central, 0x0201_4b50);
        le16(&mut central, 20);
        le16(&mut central, 20);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0x21);
        le32(&mut central, crc);
        le32(&mut central, data.len() as u32);
        le32(&mut central, data.len() as u32);
        le16(&mut central, name.len() as u16);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le32(&mut central, 0);
        le32(&mut central, offset);
        central.extend_from_slice(name.as_bytes());
    }
    let cd_offset = out.len() as u32;
    let cd_size = central.len() as u32;
    out.extend_from_slice(&central);
    le32(&mut out, 0x0605_4b50);
    le16(&mut out, 0);
    le16(&mut out, 0);
    le16(&mut out, entries.len() as u16);
    le16(&mut out, entries.len() as u16);
    le32(&mut out, cd_size);
    le32(&mut out, cd_offset);
    le16(&mut out, 0);
    out
}

#[cfg(test)]
mod tests;
