//! One entry point for every 3D symbol file type, and the guesses the Import
//! 3D Symbol dialog starts from (units and up axis).
//!
//! | extension | reader | up axis | units |
//! |---|---|---|---|
//! | `obj` | [`crate::obj`] | Y | guessed from the size |
//! | `gltf`, `glb` | [`crate::gltf`] | Y | meters |
//! | `stl` | [`crate::stl`] | Z | guessed from the size |
//! | `3ds` | [`crate::tds`] | Z | guessed from the size |
//! | `dae` | [`crate::dae`] | the file's `<up_axis>` | the file's `<unit>` |
//! | `skp` | not readable (see [`SKP_MESSAGE`]) | | |

use crate::dae;
use crate::gltf;
use crate::model::{ImportedModel, ModelError, ModelOptions, UpAxis, UNITS};
use crate::obj;
use crate::stl;
use crate::tds;

/// What Chief-style import says for a SketchUp file: its format is
/// proprietary and only SketchUp's own SDK reads it.
pub const SKP_MESSAGE: &str =
    "Export from SketchUp as COLLADA (.dae) or OBJ (File > Export > 3D Model), then import that file";

/// Extensions the 3D symbol import offers, SketchUp last (it only explains).
pub const EXTENSIONS: [&str; 7] = ["obj", "gltf", "glb", "stl", "3ds", "dae", "skp"];

/// Extensions that can really be read (no SketchUp).
pub const READABLE: [&str; 6] = ["obj", "gltf", "glb", "stl", "3ds", "dae"];

/// True for a file extension the 3D symbol import knows (any case).
pub fn is_3d_extension(ext: &str) -> bool {
    EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
}

/// Parses the 3D file `bytes` of type `ext` (`obj`, `gltf`, `glb`, `stl`,
/// `3ds` or `dae`, any case) with `opts`. `mtl` is an OBJ's material library
/// text and `resolve` loads external glTF buffers.
pub fn parse_3d(
    ext: &str,
    bytes: &[u8],
    mtl: Option<&str>,
    resolve: Option<gltf::Resolver>,
    opts: &ModelOptions,
) -> Result<ImportedModel, ModelError> {
    match ext.to_ascii_lowercase().as_str() {
        "obj" => obj::parse_obj(&String::from_utf8_lossy(bytes), mtl, opts),
        "gltf" | "glb" => gltf::parse_gltf(bytes, resolve, opts),
        "stl" => stl::parse_stl(bytes, opts),
        "3ds" => tds::parse_3ds(bytes, opts),
        "dae" => dae::parse_dae(bytes, opts),
        "skp" => Err(ModelError(SKP_MESSAGE.into())),
        other => Err(ModelError(format!(
            "Cannot import .{other} files; use STL, 3DS, COLLADA (.dae), OBJ, glTF (.gltf) or binary glTF (.glb)"
        ))),
    }
}

/// Index into [`UNITS`] of the unit that makes a model whose longest side is
/// `longest` (in file units) a plausible library symbol: the one whose result
/// in inches is nearest 36 in on a log scale, with a small preference for
/// inches and meters (what CAD and glTF files use).
pub fn guess_unit_index(longest: f64) -> usize {
    if !(longest.is_finite() && longest > 0.0) {
        return 0;
    }
    let mut best = (f64::MAX, 0usize);
    for (i, (name, k)) in UNITS.iter().enumerate() {
        // Feet are rarely a file unit for a single symbol: leave them to a
        // deliberate choice.
        if *name == "Feet" {
            continue;
        }
        let bias = match *name {
            "Inches" => 0.35,
            "Meters" => 0.15,
            _ => 0.0,
        };
        let score = ((longest * k) / 36.0).ln().abs() - bias;
        if score < best.0 {
            best = (score, i);
        }
    }
    best.1
}

/// The options an import starts from, and where they came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    /// Inches per unit and up axis.
    pub options: ModelOptions,
    /// Name of the unit (one of [`UNITS`], or `"Declared by the file"`).
    pub unit_name: String,
    /// True when the file itself declared the unit (COLLADA, glTF).
    pub from_file: bool,
}

/// Looks at the file and suggests units and up axis: the file's own values
/// when it declares them, else the format's convention and a size-based unit
/// guess. Never fails: an unreadable file gets inches and the format's axis.
pub fn suggest(ext: &str, bytes: &[u8]) -> Suggestion {
    let ext = ext.to_ascii_lowercase();
    match ext.as_str() {
        "gltf" | "glb" => {
            return Suggestion {
                options: gltf::default_options(),
                unit_name: "Meters".into(),
                from_file: true,
            }
        }
        "dae" => {
            if let Some(d) = dae::declared(bytes) {
                let o = d.options();
                let name = UNITS
                    .iter()
                    .find(|(_, k)| (*k - o.unit_scale).abs() < 1e-6 * k)
                    .map_or("Declared by the file", |(n, _)| *n);
                return Suggestion {
                    options: o,
                    unit_name: name.into(),
                    from_file: true,
                };
            }
        }
        _ => {}
    }
    let up_axis = if matches!(ext.as_str(), "stl" | "3ds") {
        UpAxis::Z
    } else {
        UpAxis::Y
    };
    let raw = ModelOptions {
        unit_scale: 1.0,
        up_axis: UpAxis::Y,
    };
    let longest = parse_3d(&ext, bytes, None, None, &raw)
        .ok()
        .and_then(|m| m.extent())
        .map(|e| f64::from(e[0].max(e[1]).max(e[2])))
        .unwrap_or(0.0);
    let i = guess_unit_index(longest);
    Suggestion {
        options: ModelOptions {
            unit_scale: UNITS[i].1,
            up_axis,
        },
        unit_name: UNITS[i].0.into(),
        from_file: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(name: &str) -> usize {
        UNITS.iter().position(|(n, _)| *n == name).unwrap()
    }

    #[test]
    fn unit_guess_follows_plausible_sizes() {
        // 2 m sofa in each unit.
        assert_eq!(guess_unit_index(2000.0), unit("Millimeters"));
        assert_eq!(guess_unit_index(200.0), unit("Centimeters"));
        assert_eq!(guess_unit_index(2.0), unit("Meters"));
        // A 30 in cabinet and a 60 in table are inches.
        assert_eq!(guess_unit_index(30.0), unit("Inches"));
        assert_eq!(guess_unit_index(60.0), unit("Inches"));
        assert_eq!(guess_unit_index(0.0), 0);
        assert_eq!(guess_unit_index(f64::NAN), 0);
    }

    #[test]
    fn dispatch_reads_every_format_and_explains_sketchup() {
        let stl = stl::tests::binary(&stl::tests::box_tris(1.0, 2.0, 3.0), "");
        let o = ModelOptions::default();
        assert_eq!(
            parse_3d("STL", &stl, None, None, &o)
                .unwrap()
                .triangle_count(),
            12
        );
        let tds = tds::tests::box_3ds(1.0, 2.0, 3.0);
        assert_eq!(
            parse_3d("3ds", &tds, None, None, &o)
                .unwrap()
                .triangle_count(),
            12
        );
        let dae = dae::tests::box_dae(1.0, 2.0, 3.0, 1.0, "Y_UP", 0.0);
        assert_eq!(
            parse_3d("dae", dae.as_bytes(), None, None, &o)
                .unwrap()
                .triangle_count(),
            12
        );
        let obj = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
        assert_eq!(
            parse_3d("obj", obj.as_bytes(), None, None, &o)
                .unwrap()
                .triangle_count(),
            1
        );
        let skp = parse_3d("skp", b"anything", None, None, &o).unwrap_err();
        assert!(skp
            .0
            .starts_with("Export from SketchUp as COLLADA (.dae) or OBJ"));
        assert!(parse_3d("fbx", b"x", None, None, &o)
            .unwrap_err()
            .0
            .contains(".fbx"));
        assert!(is_3d_extension("SKP") && is_3d_extension("Stl") && !is_3d_extension("fbx"));
    }

    #[test]
    fn suggestions_use_the_file_or_the_format_convention() {
        // A 2000 mm tall STL: Z up, millimeters.
        let stl = stl::tests::binary(&stl::tests::box_tris(600.0, 600.0, 2000.0), "");
        let s = suggest("stl", &stl);
        assert_eq!(s.options.up_axis, UpAxis::Z);
        assert_eq!(s.unit_name, "Millimeters");
        assert!(!s.from_file);
        // COLLADA declares meters per unit 0.01 = centimeters, Z up.
        let dae = dae::tests::box_dae(10.0, 10.0, 10.0, 0.01, "Z_UP", 0.0);
        let s = suggest("dae", dae.as_bytes());
        assert_eq!(
            (s.unit_name.as_str(), s.options.up_axis, s.from_file),
            ("Centimeters", UpAxis::Z, true)
        );
        // glTF is meters, Y up.
        let g = suggest("glb", b"");
        assert_eq!(
            (g.unit_name.as_str(), g.options.up_axis),
            ("Meters", UpAxis::Y)
        );
        // An unreadable OBJ falls back to inches.
        let bad = suggest("obj", b"nonsense");
        assert_eq!(bad.unit_name, "Inches");
    }
}
