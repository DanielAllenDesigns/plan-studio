//! Span and stair calculators: header and beam sizes (IRC R602.7), floor
//! joist spans (R502.3), rafter spans (R802.4), stair risers and treads
//! (R311.7.5) and deck joists, beams and footings (DCA 6).
//!
//! The tables of the IRC and of DCA 6 are not copied here. Each span is
//! computed from the lumber's reference design values (NDS Supplement, Table
//! 4A, visually graded dimension lumber 2" to 4" thick) with the limits the
//! code tables use: bending, shear at the face of the support and
//! deflection. Spans come out in whole inches, rounded down, like the printed
//! tables. The joist spans stay a little under the conservative table of
//! `rules_mep.rs`; the rafter and header spans are not checked against the
//! printed tables and may be longer than they are. They are a design aid, not
//! a structural design: the printed table of the adopted code decides
//! (decision 331).
//!
//! Southern Pine is not built in, because its design values depend on the
//! width of the member and on the edition of the NDS Supplement; pick
//! [`Material::Custom`] and type the values from the supplement.
//!
//! All lengths are inches, loads pounds per square foot, stresses psi.

use crate::CheckOptions;

// ----- lumber -----

/// Species and species groups with built-in values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Species {
    DouglasFirLarch,
    HemFir,
    SprucePineFir,
}

impl Species {
    /// Every species, in menu order.
    pub const ALL: [Species; 3] = [
        Species::DouglasFirLarch,
        Species::HemFir,
        Species::SprucePineFir,
    ];

    /// Menu text.
    pub fn name(self) -> &'static str {
        match self {
            Species::DouglasFirLarch => "Douglas fir-larch",
            Species::HemFir => "Hem-fir",
            Species::SprucePineFir => "Spruce-pine-fir",
        }
    }
}

/// Visual grade of dimension lumber.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Grade {
    No1,
    No2,
    No3,
}

impl Grade {
    /// Every grade, in menu order.
    pub const ALL: [Grade; 3] = [Grade::No1, Grade::No2, Grade::No3];

    /// Menu text.
    pub fn name(self) -> &'static str {
        match self {
            Grade::No1 => "No. 1",
            Grade::No2 => "No. 2",
            Grade::No3 => "No. 3",
        }
    }
}

/// Reference design values of a material.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Strength {
    /// Bending, psi.
    pub fb: f64,
    /// Modulus of elasticity, psi.
    pub e: f64,
    /// Shear parallel to the grain, psi.
    pub fv: f64,
    /// Compression perpendicular to the grain (bearing), psi.
    pub fc_perp: f64,
    /// Apply the size factor of dimension lumber to `fb` (false for the
    /// values typed for a custom material and for engineered lumber).
    pub size_factor: bool,
}

/// What a member is made of.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Material {
    /// Sawn dimension lumber of a species and grade.
    Lumber(Species, Grade),
    /// Values typed by the user (Southern Pine, other species).
    Custom(Strength),
}

impl Material {
    /// Short name for results and the menu.
    pub fn name(self) -> String {
        match self {
            Material::Lumber(s, g) => format!("{} {}", s.name(), g.name()),
            Material::Custom(c) => format!("Custom (Fb {:.0}, E {:.2}M)", c.fb, c.e / 1e6),
        }
    }

    /// The reference design values.
    pub fn strength(self) -> Strength {
        match self {
            Material::Custom(c) => c,
            Material::Lumber(s, g) => lumber_strength(s, g),
        }
    }
}

/// NDS Supplement Table 4A, 2" to 4" thick, 2" and wider. Spruce-pine-fir
/// lists No. 1 and No. 2 together.
fn lumber_strength(s: Species, g: Grade) -> Strength {
    let (fb, e, fv, fc_perp) = match (s, g) {
        (Species::DouglasFirLarch, Grade::No1) => (1000.0, 1.7e6, 180.0, 625.0),
        (Species::DouglasFirLarch, Grade::No2) => (900.0, 1.6e6, 180.0, 625.0),
        (Species::DouglasFirLarch, Grade::No3) => (525.0, 1.4e6, 180.0, 625.0),
        (Species::HemFir, Grade::No1) => (975.0, 1.5e6, 150.0, 405.0),
        (Species::HemFir, Grade::No2) => (850.0, 1.3e6, 150.0, 405.0),
        (Species::HemFir, Grade::No3) => (500.0, 1.2e6, 150.0, 405.0),
        (Species::SprucePineFir, Grade::No1 | Grade::No2) => (875.0, 1.4e6, 135.0, 425.0),
        (Species::SprucePineFir, Grade::No3) => (500.0, 1.2e6, 135.0, 425.0),
    };
    Strength {
        fb,
        e,
        fv,
        fc_perp,
        size_factor: true,
    }
}

/// Size factor `CF` for bending of a 2x member of nominal depth `nominal`
/// (NDS Table 4A adjustment factors; No. 3 lumber has smaller factors).
fn size_factor(nominal: u32, grade_no3: bool) -> f64 {
    match (nominal, grade_no3) {
        (..=4, false) => 1.5,
        (5..=6, false) => 1.3,
        (7..=8, false) => 1.2,
        (9..=10, false) => 1.1,
        (11..=12, false) => 1.0,
        (_, false) => 0.9,
        (..=4, true) => 1.1,
        (5..=6, true) => 1.1,
        (7..=8, true) => 1.05,
        (9..=10, true) => 1.0,
        (_, true) => 0.9,
    }
}

/// Actual depth of a nominal 2x board: 2x4 3 1/2", 2x6 5 1/2", then nominal
/// less 3/4".
pub fn actual_depth(nominal: u32) -> f64 {
    match nominal {
        0..=4 => 3.5,
        5..=6 => 5.5,
        n => f64::from(n) - 0.75,
    }
}

/// Actual thickness of a 2x ply.
pub const PLY: f64 = 1.5;

/// The nominal depths the span calculators offer.
pub const JOIST_SIZES: [u32; 4] = [6, 8, 10, 12];
/// The centre-to-centre spacings of joists and rafters, inches.
pub const SPACINGS: [f64; 4] = [12.0, 16.0, 19.2, 24.0];

// ----- the beam engine -----

/// A rectangular section: width `b` (all plies) by depth `d`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Section {
    b: f64,
    d: f64,
}

impl Section {
    fn modulus(self) -> f64 {
        self.b * self.d * self.d / 6.0
    }

    fn inertia(self) -> f64 {
        self.b * self.d.powi(3) / 12.0
    }
}

/// Adjusted design values of a member (every factor but load duration).
#[derive(Debug, Clone, Copy)]
struct Adjusted {
    fb: f64,
    e: f64,
    fv: f64,
}

/// Load on a simple span, pounds per inch of span, and the deflection limits
/// as the divisor of the span (`360` is L/360).
#[derive(Debug, Clone, Copy)]
struct Loads {
    /// Total load divided by its load duration factor: the governing
    /// strength case.
    strength: f64,
    /// Live part for the live-load deflection limit.
    live: f64,
    /// Dead plus live for the total-load deflection limit.
    total: f64,
    live_limit: f64,
    total_limit: f64,
}

/// What decided a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Governs {
    Bending,
    Shear,
    LiveDeflection,
    TotalDeflection,
}

impl Governs {
    /// Text for a result line.
    pub fn name(self) -> &'static str {
        match self {
            Governs::Bending => "bending",
            Governs::Shear => "shear",
            Governs::LiveDeflection => "live load deflection",
            Governs::TotalDeflection => "total load deflection",
        }
    }
}

/// A span and what limits it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    /// Longest span, whole inches rounded down.
    pub inches: f64,
    pub governs: Governs,
}

impl Span {
    /// `12'-9"`.
    pub fn text(self) -> String {
        plan_core::units::fmt_ft_in(self.inches)
    }
}

/// Longest simple span of `sec` under `loads`: the least of bending, shear
/// (taken at the face of the support, distance `d` from the centre of the
/// bearing) and the two deflection limits. A span the loads rule out
/// altogether is 0.
fn max_span(sec: Section, m: Adjusted, loads: Loads) -> Span {
    let w = loads.strength.max(1e-9);
    let bending = (8.0 * m.fb * sec.modulus() / w).sqrt();
    let v_allow = 2.0 / 3.0 * m.fv * sec.b * sec.d;
    let shear = 2.0 * sec.d + 2.0 * v_allow / w;
    let ei = 384.0 * m.e * sec.inertia();
    let live = (ei / (5.0 * loads.live_limit * loads.live.max(1e-9))).cbrt();
    let total = (ei / (5.0 * loads.total_limit * loads.total.max(1e-9))).cbrt();
    let mut best = (bending, Governs::Bending);
    for c in [
        (shear, Governs::Shear),
        (live, Governs::LiveDeflection),
        (total, Governs::TotalDeflection),
    ] {
        if c.0 < best.0 {
            best = c;
        }
    }
    Span {
        inches: (best.0 + 1e-6).floor().max(0.0),
        governs: best.1,
    }
}

/// Strength of a member at the nominal depth, with the load duration applied
/// later, the repetitive member factor `cr` and the wet service and incising
/// factors when `wet` (pressure-treated deck lumber).
fn adjust(mat: Material, nominal: u32, cr: f64, wet: bool) -> Adjusted {
    let s = mat.strength();
    let no3 = matches!(mat, Material::Lumber(_, Grade::No3));
    let cf = if s.size_factor {
        size_factor(nominal, no3)
    } else {
        1.0
    };
    let mut fb = s.fb * cf;
    let mut e = s.e;
    let mut fv = s.fv;
    if wet {
        // NDS 4.3: wet service (Fb 0.85 unless Fb*CF <= 1150; E 0.9; Fv 0.97)
        // and incising of treated lumber (Fb, Fv 0.8; E 0.95).
        let cm_fb = if fb <= 1150.0 { 1.0 } else { 0.85 };
        fb *= cm_fb * 0.8;
        e *= 0.9 * 0.95;
        fv *= 0.97 * 0.8;
    }
    Adjusted {
        fb: fb * cr,
        e,
        fv,
    }
}

/// Pounds per inch of span for `psf` over a tributary width of `trib` inches.
fn per_inch(psf: f64, trib: f64) -> f64 {
    psf * trib / 144.0
}

// ----- floor joists (R502.3) -----

/// What the joists carry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JoistInput {
    pub material: Material,
    /// Nominal depth: 6, 8, 10 or 12.
    pub nominal: u32,
    /// Centre to centre, inches.
    pub spacing: f64,
    /// 30 psf in sleeping rooms and attics with storage, 40 elsewhere.
    pub live: f64,
    /// 10 psf, or 20 psf with a heavy floor (tile, stone).
    pub dead: f64,
}

impl Default for JoistInput {
    fn default() -> Self {
        Self {
            material: Material::Lumber(Species::DouglasFirLarch, Grade::No2),
            nominal: 10,
            spacing: 16.0,
            live: 40.0,
            dead: 10.0,
        }
    }
}

/// Longest joist span (R502.3.1): bending, shear, live load L/360 and total
/// load L/240; repetitive member factor 1.15; load duration 1.0.
pub fn joist_span(i: &JoistInput) -> Span {
    let sec = Section {
        b: PLY,
        d: actual_depth(i.nominal),
    };
    let m = adjust(i.material, i.nominal, 1.15, false);
    let loads = Loads {
        strength: per_inch(i.live + i.dead, i.spacing),
        live: per_inch(i.live, i.spacing),
        total: per_inch(i.live + i.dead, i.spacing),
        live_limit: 360.0,
        total_limit: 240.0,
    };
    max_span(sec, m, loads)
}

/// One row of the joist table: a size with its span at every spacing.
#[derive(Debug, Clone, PartialEq)]
pub struct SpanRow {
    pub label: String,
    pub spans: Vec<Span>,
}

/// The joist span table for the material and loads of `i`: a row for each of
/// [`JOIST_SIZES`], a column for each of [`SPACINGS`].
pub fn joist_table(i: &JoistInput) -> Vec<SpanRow> {
    JOIST_SIZES
        .iter()
        .map(|&n| SpanRow {
            label: format!("2x{n}"),
            spans: SPACINGS
                .iter()
                .map(|&s| {
                    joist_span(&JoistInput {
                        nominal: n,
                        spacing: s,
                        ..*i
                    })
                })
                .collect(),
        })
        .collect()
}

/// The smallest of [`JOIST_SIZES`] that spans `span` inches, if any.
pub fn joist_size_for(i: &JoistInput, span: f64) -> Option<u32> {
    JOIST_SIZES
        .iter()
        .copied()
        .find(|&n| joist_span(&JoistInput { nominal: n, ..*i }).inches >= span - 1e-6)
}

// ----- rafters (R802.4) -----

/// What the rafters carry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RafterInput {
    pub material: Material,
    pub nominal: u32,
    pub spacing: f64,
    /// Ground snow load, psf; 0 for a roof live load of 20 psf.
    pub ground_snow: f64,
    /// Dead load on the slope, psf (10, or 15 with heavy roofing).
    pub dead: f64,
    /// Roof pitch, rise in 12.
    pub pitch: f64,
    /// A finished ceiling is attached to the rafters (L/240 live load
    /// deflection instead of L/180).
    pub ceiling_attached: bool,
}

impl Default for RafterInput {
    fn default() -> Self {
        Self {
            material: Material::Lumber(Species::DouglasFirLarch, Grade::No2),
            nominal: 8,
            spacing: 16.0,
            ground_snow: 0.0,
            dead: 10.0,
            pitch: 6.0,
            ceiling_attached: false,
        }
    }
}

impl RafterInput {
    /// The live (snow) load on the horizontal projection: `0.7 Pg`, at least
    /// the 20 psf roof live load.
    pub fn roof_load(&self) -> f64 {
        (0.7 * self.ground_snow).max(20.0)
    }

    /// The dead load on the horizontal projection.
    pub fn dead_horizontal(&self) -> f64 {
        self.dead * (1.0 + (self.pitch / 12.0).powi(2)).sqrt()
    }
}

/// Load duration factor of the roof load: 1.15 snow, 1.25 roof live load.
fn roof_duration(ground_snow: f64) -> f64 {
    if ground_snow * 0.7 > 20.0 {
        1.15
    } else {
        1.25
    }
}

/// Longest rafter span, measured on the horizontal projection (R802.4.1):
/// bending and shear with the roof's load duration factor, live load L/180
/// (L/240 with a ceiling attached) and total load L/180.
pub fn rafter_span(i: &RafterInput) -> Span {
    let sec = Section {
        b: PLY,
        d: actual_depth(i.nominal),
    };
    let m = adjust(i.material, i.nominal, 1.15, false);
    let live = i.roof_load();
    let dead = i.dead_horizontal();
    let cd = roof_duration(i.ground_snow);
    let loads = Loads {
        strength: per_inch(live + dead, i.spacing) / cd,
        live: per_inch(live, i.spacing),
        total: per_inch(live + dead, i.spacing),
        live_limit: if i.ceiling_attached { 240.0 } else { 180.0 },
        total_limit: 180.0,
    };
    max_span(sec, m, loads)
}

/// The rafter span table: a row for each size, a column for each spacing.
pub fn rafter_table(i: &RafterInput) -> Vec<SpanRow> {
    JOIST_SIZES
        .iter()
        .map(|&n| SpanRow {
            label: format!("2x{n}"),
            spans: SPACINGS
                .iter()
                .map(|&s| {
                    rafter_span(&RafterInput {
                        nominal: n,
                        spacing: s,
                        ..*i
                    })
                })
                .collect(),
        })
        .collect()
}

/// The smallest rafter that spans `run` inches of plan.
pub fn rafter_size_for(i: &RafterInput, run: f64) -> Option<u32> {
    JOIST_SIZES
        .iter()
        .copied()
        .find(|&n| rafter_span(&RafterInput { nominal: n, ..*i }).inches >= run - 1e-6)
}

// ----- headers and beams (R602.7) -----

/// The roof (and one floor) a header carries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeaderInput {
    /// Opening width, inches.
    pub span: f64,
    pub material: Material,
    /// Ground snow load, psf (20, 30, 50, 70 in the IRC tables).
    pub ground_snow: f64,
    /// Building width, feet (20, 28, 36 in the IRC tables).
    pub building_width_ft: f64,
    /// A floor with a centre bearing wall above the header (0 or 1).
    pub floors_above: u32,
    /// Depth of the wall, inches: 3.5 or 5.5 (bearing of the jack studs).
    pub wall_depth: f64,
}

impl Default for HeaderInput {
    fn default() -> Self {
        Self {
            span: 48.0,
            material: Material::Lumber(Species::DouglasFirLarch, Grade::No2),
            ground_snow: 30.0,
            building_width_ft: 28.0,
            floors_above: 0,
            wall_depth: 5.5,
        }
    }
}

/// Dead load of roof and ceiling, psf (R602.7 tables assume 15 psf).
const ROOF_DEAD: f64 = 15.0;
/// Roof overhang added to the tributary width, inches.
const OVERHANG: f64 = 24.0;
/// Floor loads over a header, psf, and the wall above it.
const FLOOR_LIVE: f64 = 30.0;
const FLOOR_DEAD: f64 = 10.0;
const WALL_DEAD_PLF: f64 = 80.0;

/// Pounds per inch carried by a header of `i`: strength (each case divided by
/// its duration factor), live and total.
fn header_loads(i: &HeaderInput) -> Loads {
    let half = i.building_width_ft * 12.0 / 2.0;
    let roof_trib = half + OVERHANG;
    let roof_live = (0.7 * i.ground_snow).max(20.0);
    let roof_dead_w = per_inch(ROOF_DEAD, roof_trib);
    let roof_live_w = per_inch(roof_live, roof_trib);
    let (floor_live_w, floor_dead_w, wall_w) = if i.floors_above > 0 {
        (
            per_inch(FLOOR_LIVE, half),
            per_inch(FLOOR_DEAD, half),
            WALL_DEAD_PLF / 12.0,
        )
    } else {
        (0.0, 0.0, 0.0)
    };
    let dead = roof_dead_w + floor_dead_w + wall_w;
    let cd_roof = roof_duration(i.ground_snow);
    // The cases of NDS 2.3.2: dead plus floor live (1.0), dead plus roof load
    // (snow 1.15, roof live 1.25), and all of them together (shortest duration
    // governs).
    let cases = [
        (dead + floor_live_w, 1.0),
        (dead + roof_live_w, cd_roof),
        (dead + roof_live_w + floor_live_w, cd_roof),
    ];
    let strength = cases
        .iter()
        .map(|(w, cd)| w / cd)
        .fold(0.0_f64, f64::max);
    Loads {
        strength,
        live: roof_live_w + floor_live_w,
        total: dead + roof_live_w + floor_live_w,
        live_limit: 360.0,
        total_limit: 240.0,
    }
}

/// How a header option is built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderKind {
    /// Plies of 2x lumber, 1 1/2" each.
    Lumber,
    /// Plies of laminated veneer lumber, 1 3/4" each.
    Lvl,
}

/// Laminated veneer lumber, 2.0E: Fb 2600, E 2.0M, Fv 285, Fc perp 750 psi.
const LVL: Strength = Strength {
    fb: 2600.0,
    e: 2.0e6,
    fv: 285.0,
    fc_perp: 750.0,
    size_factor: false,
};

/// One header or beam section with what it carries.
#[derive(Debug, Clone, PartialEq)]
pub struct HeaderOption {
    /// `"2-2x8"` or `"2-1 3/4 x 9 1/2 LVL"`.
    pub label: String,
    pub kind: HeaderKind,
    pub plies: u32,
    /// Depth of the header, inches.
    pub depth: f64,
    /// Total thickness, inches.
    pub width: f64,
    /// Longest opening this section carries under the given loads.
    pub span: Span,
    /// The opening asked for is within `span`.
    pub passes: bool,
    /// Jack studs under each end of the header.
    pub jacks: u32,
}

/// Every section the calculator offers, smallest capacity first.
fn header_candidates() -> Vec<(HeaderKind, u32, f64, f64)> {
    let mut out = Vec::new();
    for plies in [2u32, 3] {
        for n in [4u32, 6, 8, 10, 12] {
            out.push((HeaderKind::Lumber, plies, actual_depth(n), f64::from(n)));
        }
    }
    for plies in [1u32, 2] {
        for d in [7.25, 9.25, 9.5, 11.25, 11.875, 14.0] {
            out.push((HeaderKind::Lvl, plies, d, 0.0));
        }
    }
    out
}

/// `9 1/4`, `11 7/8`, `14`: an LVL depth as inches and a fraction.
fn lvl_depth(d: f64) -> String {
    let whole = d.floor();
    let eighths = ((d - whole) * 8.0).round() as u32;
    let frac = match eighths {
        0 => "",
        2 => " 1/4",
        4 => " 1/2",
        7 => " 7/8",
        _ => "",
    };
    format!("{whole:.0}{frac}")
}

/// Jack studs needed at each end for bearing of the header on their ends
/// (each stud bears 1 1/2" by the header's width, at most the wall's depth).
fn jack_studs(reaction: f64, fc_perp: f64, bearing_width: f64) -> u32 {
    let per_stud = fc_perp * PLY * bearing_width;
    ((reaction / per_stud) - 1e-9).ceil().max(1.0) as u32
}

/// All header options for `i`, each with its span under the loads and whether
/// it carries the opening, ordered from the lightest to the heaviest.
pub fn header_options(i: &HeaderInput) -> Vec<HeaderOption> {
    let loads = header_loads(i);
    let s = i.material.strength();
    let mut out: Vec<HeaderOption> = header_candidates()
        .into_iter()
        .map(|(kind, plies, depth, nominal)| {
            let (strength, adj, thickness, label) = match kind {
                HeaderKind::Lumber => {
                    let n = nominal as u32;
                    (
                        s,
                        adjust(i.material, n, 1.0, false),
                        f64::from(plies) * PLY,
                        format!("{plies}-2x{n}"),
                    )
                }
                HeaderKind::Lvl => {
                    // Volume factor (12/d)^(1/9) for sections deeper than 12".
                    let cv = if depth > 12.0 {
                        (12.0 / depth).powf(1.0 / 9.0)
                    } else {
                        1.0
                    };
                    (
                        LVL,
                        Adjusted {
                            fb: LVL.fb * cv,
                            e: LVL.e,
                            fv: LVL.fv,
                        },
                        f64::from(plies) * 1.75,
                        format!("{plies}-1 3/4 x {} LVL", lvl_depth(depth)),
                    )
                }
            };
            let sec = Section {
                b: thickness,
                d: depth,
            };
            let span = max_span(sec, adj, loads);
            let reaction = loads.total * i.span / 2.0;
            let jacks = jack_studs(reaction, strength.fc_perp, thickness.min(i.wall_depth));
            HeaderOption {
                label,
                kind,
                plies,
                depth,
                width: thickness,
                passes: span.inches >= i.span - 1e-6,
                span,
                jacks,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        let key = |o: &HeaderOption| o.width * o.depth * o.depth;
        key(a).total_cmp(&key(b))
    });
    out
}

/// The lightest section that carries the opening: dimension lumber first, an
/// LVL when lumber cannot.
pub fn header_recommend(i: &HeaderInput) -> Option<HeaderOption> {
    let all = header_options(i);
    all.iter()
        .find(|o| o.passes && o.kind == HeaderKind::Lumber)
        .or_else(|| all.iter().find(|o| o.passes))
        .cloned()
}

// ----- stairs (R311.7) -----

/// The limits a stair is held to (inches).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StairLimits {
    /// Tallest riser (R311.7.5.1).
    pub riser_max: f64,
    /// Shallowest tread (R311.7.5.2).
    pub tread_min: f64,
    /// Greatest rise of one flight (R311.7.3).
    pub flight_rise_max: f64,
    /// Greatest difference between risers of one flight (R311.7.5.1).
    pub riser_variation: f64,
    /// Headroom (R311.7.2).
    pub headroom: f64,
    /// Stair width (R311.7.1).
    pub width_min: f64,
}

impl Default for StairLimits {
    fn default() -> Self {
        Self {
            riser_max: 7.75,
            tread_min: 10.0,
            flight_rise_max: 151.0,
            riser_variation: 0.375,
            headroom: 80.0,
            width_min: 36.0,
        }
    }
}

impl StairLimits {
    /// The limits of a Plan Check run.
    pub fn from_options(o: &CheckOptions) -> Self {
        Self {
            riser_max: o.riser_max,
            tread_min: o.tread_min,
            headroom: o.headroom,
            width_min: o.stair_min_width,
            ..Self::default()
        }
    }
}

/// What the stair calculator is asked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StairInput {
    /// Floor to floor, inches.
    pub total_rise: f64,
    /// The riser height aimed at; the count of risers rounds to it.
    pub target_riser: f64,
    /// Tread depth wanted (nosing excluded), inches.
    pub tread: f64,
    pub limits: StairLimits,
}

impl Default for StairInput {
    fn default() -> Self {
        Self {
            total_rise: 108.0,
            target_riser: 7.0,
            tread: 10.5,
            limits: StairLimits::default(),
        }
    }
}

/// One line of the stair check.
#[derive(Debug, Clone, PartialEq)]
pub struct StairNote {
    pub ok: bool,
    pub text: String,
}

/// The stair the calculator lays out.
#[derive(Debug, Clone, PartialEq)]
pub struct StairResult {
    pub risers: u32,
    /// Treads in the flight: one fewer than the risers.
    pub treads: u32,
    pub riser_height: f64,
    pub tread_depth: f64,
    /// Horizontal length of the flight.
    pub total_run: f64,
    /// Length of the stringer along the slope.
    pub stringer: f64,
    pub angle_deg: f64,
    /// `2R + T`, comfortable between 24" and 25".
    pub two_r_plus_t: f64,
    pub notes: Vec<StairNote>,
}

impl StairResult {
    /// Every note is ok.
    pub fn passes(&self) -> bool {
        self.notes.iter().all(|n| n.ok)
    }
}

/// Lays out a stair: the fewest risers at or under the target height whose
/// risers do not pass the maximum, equal risers, `risers - 1` treads.
/// Returns `None` for a rise that is not positive.
pub fn stair_layout(i: &StairInput) -> Option<StairResult> {
    if i.total_rise <= 0.0 || i.total_rise.is_nan() {
        return None;
    }
    let l = &i.limits;
    let target = i.target_riser.clamp(4.0, l.riser_max);
    let mut risers = (i.total_rise / target).round().max(1.0) as u32;
    // Never over the maximum riser.
    while i.total_rise / f64::from(risers) > l.riser_max + 1e-9 {
        risers += 1;
    }
    let riser_height = i.total_rise / f64::from(risers);
    let treads = risers.saturating_sub(1);
    let tread_depth = i.tread;
    let total_run = f64::from(treads) * tread_depth;
    let stringer = (i.total_rise.powi(2) + total_run.powi(2)).sqrt();
    let angle_deg = if total_run > 0.0 {
        (i.total_rise / total_run).atan().to_degrees()
    } else {
        90.0
    };
    let two_r_plus_t = 2.0 * riser_height + tread_depth;
    let mut notes = Vec::new();
    let mut note = |ok: bool, text: String| notes.push(StairNote { ok, text });
    note(
        riser_height <= l.riser_max + 1e-9,
        format!(
            "Riser {:.3}\" (R311.7.5.1: at most {:.2}\")",
            riser_height, l.riser_max
        ),
    );
    note(
        tread_depth >= l.tread_min - 1e-9,
        format!(
            "Tread {:.2}\" (R311.7.5.2: at least {:.0}\")",
            tread_depth, l.tread_min
        ),
    );
    note(
        i.total_rise <= l.flight_rise_max + 1e-9,
        format!(
            "Rise {} (R311.7.3: a flight rises at most {}; add an intermediate landing)",
            plan_core::units::fmt_ft_in(i.total_rise),
            plan_core::units::fmt_ft_in(l.flight_rise_max)
        ),
    );
    note(
        (24.0..=25.0).contains(&(two_r_plus_t + 1e-9)) || (23.99..=25.01).contains(&two_r_plus_t),
        format!("2R + T = {two_r_plus_t:.2}\" (comfortable between 24\" and 25\")"),
    );
    if tread_depth < 11.0 {
        note(
            true,
            "Treads under 11\" need a nosing of 3/4\" to 1 1/4\" (R311.7.5.3)".into(),
        );
    }
    note(
        true,
        format!(
            "Headroom {} and width {:.0}\" are checked on the stair, not here",
            plan_core::units::fmt_ft_in(l.headroom),
            l.width_min
        ),
    );
    Some(StairResult {
        risers,
        treads,
        riser_height,
        tread_depth,
        total_run,
        stringer,
        angle_deg,
        two_r_plus_t,
        notes,
    })
}

// ----- decks (DCA 6) -----

/// A deck: the joists and the beam under them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeckInput {
    pub material: Material,
    /// Joist depth, nominal.
    pub joist_nominal: u32,
    /// Joist spacing, inches.
    pub joist_spacing: f64,
    /// Beam depth, nominal (2-ply or 3-ply built up).
    pub beam_nominal: u32,
    pub beam_plies: u32,
    /// Joist span between the ledger or a beam and the beam, inches.
    pub joist_span: f64,
    /// Joists overhang the beam by this much, inches.
    pub cantilever: f64,
    /// Allowable soil bearing, psf.
    pub soil_bearing: f64,
}

impl Default for DeckInput {
    fn default() -> Self {
        Self {
            material: Material::Lumber(Species::DouglasFirLarch, Grade::No2),
            joist_nominal: 8,
            joist_spacing: 16.0,
            beam_nominal: 10,
            beam_plies: 2,
            joist_span: 120.0,
            cantilever: 0.0,
            soil_bearing: 1500.0,
        }
    }
}

/// Deck live and dead loads, psf (DCA 6: 40 live, 10 dead).
pub const DECK_LIVE: f64 = 40.0;
pub const DECK_DEAD: f64 = 10.0;

/// What the deck calculator finds.
#[derive(Debug, Clone, PartialEq)]
pub struct DeckResult {
    /// Longest joist span for the joist size and spacing.
    pub joist: Span,
    /// The joists carry the span asked for.
    pub joist_ok: bool,
    /// Longest cantilever DCA 6 allows: a quarter of the joist span.
    pub cantilever_max: f64,
    pub cantilever_ok: bool,
    /// Longest beam span between posts.
    pub beam: Span,
    /// Load on one post at that spacing, pounds.
    pub post_load: f64,
    /// Side of a square footing under one post, inches.
    pub footing_side: f64,
}

/// Joist span on the deck's loads (wet service, incised treated lumber).
pub fn deck_joist_span(i: &DeckInput) -> Span {
    let sec = Section {
        b: PLY,
        d: actual_depth(i.joist_nominal),
    };
    let m = adjust(i.material, i.joist_nominal, 1.15, true);
    let loads = Loads {
        strength: per_inch(DECK_LIVE + DECK_DEAD, i.joist_spacing),
        live: per_inch(DECK_LIVE, i.joist_spacing),
        total: per_inch(DECK_LIVE + DECK_DEAD, i.joist_spacing),
        live_limit: 360.0,
        total_limit: 240.0,
    };
    max_span(sec, m, loads)
}

/// Longest beam span between posts for a deck whose joists span
/// `joist_span` and overhang `cantilever` (tributary width: half the span
/// plus the overhang).
pub fn deck_beam_span(i: &DeckInput) -> Span {
    let sec = Section {
        b: f64::from(i.beam_plies) * PLY,
        d: actual_depth(i.beam_nominal),
    };
    let m = adjust(i.material, i.beam_nominal, 1.0, true);
    let trib = i.joist_span / 2.0 + i.cantilever;
    let w = per_inch(DECK_LIVE + DECK_DEAD, trib);
    let loads = Loads {
        strength: w,
        live: per_inch(DECK_LIVE, trib),
        total: w,
        live_limit: 360.0,
        total_limit: 240.0,
    };
    max_span(sec, m, loads)
}

/// The deck: joists, cantilever, beam, post load and footing.
pub fn deck_design(i: &DeckInput) -> DeckResult {
    let joist = deck_joist_span(i);
    let beam = deck_beam_span(i);
    let trib = i.joist_span / 2.0 + i.cantilever;
    let post_load = (DECK_LIVE + DECK_DEAD) * trib * beam.inches / 144.0;
    let area_sq_in = post_load / i.soil_bearing.max(1.0) * 144.0;
    let cantilever_max = (i.joist_span / 4.0).floor();
    DeckResult {
        joist_ok: joist.inches >= i.joist_span - 1e-6,
        cantilever_ok: i.cantilever <= cantilever_max + 1e-6,
        cantilever_max,
        joist,
        beam,
        post_load,
        footing_side: area_sq_in.sqrt().ceil(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DF2: Material = Material::Lumber(Species::DouglasFirLarch, Grade::No2);

    fn joist(n: u32, spacing: f64) -> f64 {
        joist_span(&JoistInput {
            nominal: n,
            spacing,
            ..JoistInput::default()
        })
        .inches
    }

    #[test]
    fn joist_spans_stay_near_the_conservative_plan_check_table() {
        // `rules_mep.rs` limits (40 psf live, 16" on centre, DF-L No. 2):
        // 2x6 117", 2x8 154", 2x10 195", 2x12 236". The computed spans are
        // never more than 2" longer and at most 10% shorter.
        for (n, limit) in [(6u32, 117.0), (8, 154.0), (10, 195.0), (12, 236.0)] {
            let got = joist(n, 16.0);
            assert!(
                got <= limit + 2.0 && got >= limit * 0.9,
                "2x{n}: computed {got}, plan check limit {limit}"
            );
        }
    }

    #[test]
    fn joist_spans_shrink_with_spacing_and_grow_with_depth() {
        for n in JOIST_SIZES {
            let row: Vec<f64> = SPACINGS.iter().map(|&s| joist(n, s)).collect();
            assert!(row.windows(2).all(|w| w[0] >= w[1]), "2x{n}: {row:?}");
        }
        for s in SPACINGS {
            let col: Vec<f64> = JOIST_SIZES.iter().map(|&n| joist(n, s)).collect();
            assert!(col.windows(2).all(|w| w[0] < w[1]), "{s}: {col:?}");
        }
    }

    #[test]
    fn table_edges_are_whole_inches_and_the_size_lookup_is_exact() {
        let i = JoistInput::default();
        let t = joist_table(&i);
        assert_eq!(t.len(), 4);
        assert_eq!(t[0].spans.len(), 4);
        assert_eq!(t[0].label, "2x6");
        for r in &t {
            for s in &r.spans {
                assert_eq!(s.inches, s.inches.floor());
            }
        }
        let span = joist(10, 16.0);
        // The span itself is carried by a 2x10, one inch more needs a 2x12.
        assert_eq!(joist_size_for(&i, span), Some(10));
        assert_eq!(joist_size_for(&i, span + 1.0), Some(12));
        // Past the longest 2x12 nothing in the table is enough.
        assert_eq!(joist_size_for(&i, joist(12, 16.0) + 1.0), None);
        // The very shortest span is a 2x6.
        assert_eq!(joist_size_for(&i, 1.0), Some(6));
    }

    #[test]
    fn lighter_floor_load_and_better_grade_span_farther() {
        let base = JoistInput::default();
        let sleeping = JoistInput { live: 30.0, ..base };
        assert!(joist_span(&sleeping).inches > joist_span(&base).inches);
        let tile = JoistInput { dead: 20.0, ..base };
        assert!(joist_span(&tile).inches < joist_span(&base).inches);
        let no1 = JoistInput {
            material: Material::Lumber(Species::DouglasFirLarch, Grade::No1),
            ..base
        };
        assert!(joist_span(&no1).inches >= joist_span(&base).inches);
        let hem = JoistInput {
            material: Material::Lumber(Species::HemFir, Grade::No2),
            ..base
        };
        assert!(joist_span(&hem).inches < joist_span(&base).inches);
        let spf = JoistInput {
            material: Material::Lumber(Species::SprucePineFir, Grade::No2),
            ..base
        };
        assert!(joist_span(&spf).inches < joist_span(&base).inches);
    }

    #[test]
    fn a_custom_material_uses_its_own_values() {
        let c = Material::Custom(Strength {
            fb: 1200.0,
            e: 1.6e6,
            fv: 175.0,
            fc_perp: 565.0,
            size_factor: false,
        });
        let a = joist_span(&JoistInput {
            material: c,
            ..JoistInput::default()
        });
        let b = joist_span(&JoistInput::default());
        assert_ne!(a.inches, b.inches);
        assert!(c.name().starts_with("Custom"));
    }

    #[test]
    fn deflection_governs_shallow_floor_joists_and_bending_the_deep_ones() {
        let shallow = joist_span(&JoistInput {
            nominal: 6,
            spacing: 12.0,
            ..JoistInput::default()
        });
        assert_eq!(shallow.governs, Governs::LiveDeflection);
        let deep = joist_span(&JoistInput {
            nominal: 12,
            spacing: 12.0,
            ..JoistInput::default()
        });
        assert_eq!(deep.governs, Governs::Bending);
        let weak = joist_span(&JoistInput {
            nominal: 8,
            material: Material::Lumber(Species::DouglasFirLarch, Grade::No3),
            ..JoistInput::default()
        });
        assert_eq!(weak.governs, Governs::Bending);
    }

    #[test]
    fn rafter_spans() {
        let r = |n: u32, snow: f64| {
            rafter_span(&RafterInput {
                nominal: n,
                ground_snow: snow,
                ..RafterInput::default()
            })
            .inches
        };
        // More snow, shorter span; deeper rafters, longer span.
        assert!(r(8, 0.0) > r(8, 50.0));
        assert!(r(10, 30.0) > r(8, 30.0));
        // Ground snow under 28.6 psf is under the 20 psf roof live load.
        assert_eq!(r(8, 20.0), r(8, 0.0));
        assert!(r(8, 30.0) < r(8, 0.0));
        // A 2x8 at 16" under a 20 psf roof runs between 12' and 19'.
        assert!((150.0..230.0).contains(&r(8, 0.0)), "{}", r(8, 0.0));
        // A finished ceiling tightens the live load deflection limit.
        let with = rafter_span(&RafterInput {
            ceiling_attached: true,
            nominal: 12,
            spacing: 12.0,
            ..RafterInput::default()
        });
        let without = rafter_span(&RafterInput {
            nominal: 12,
            spacing: 12.0,
            ..RafterInput::default()
        });
        assert!(with.inches <= without.inches);
        // A steeper roof carries more dead load per foot of run.
        let steep = RafterInput {
            pitch: 12.0,
            ..RafterInput::default()
        };
        assert!(steep.dead_horizontal() > RafterInput::default().dead_horizontal());
    }

    #[test]
    fn rafter_table_and_lookup() {
        let i = RafterInput::default();
        let t = rafter_table(&i);
        assert_eq!(t.len(), 4);
        assert_eq!(t[3].label, "2x12");
        let run = rafter_span(&RafterInput { nominal: 8, ..i }).inches;
        assert_eq!(rafter_size_for(&i, run), Some(8));
        assert_eq!(rafter_size_for(&i, run + 1.0), Some(10));
        assert_eq!(rafter_size_for(&i, 10_000.0), None);
    }

    #[test]
    fn headers_grow_with_the_opening() {
        let rec = |span: f64| header_recommend(&HeaderInput { span, ..HeaderInput::default() });
        let narrow = rec(36.0).unwrap();
        let wide = rec(72.0).unwrap();
        assert!(narrow.depth <= wide.depth);
        assert!(narrow.passes && wide.passes);
        // A 36" opening under 30 psf snow on a 28' house: a 2-2x8 is enough
        // (the IRC table gives 2-2x8 up to about 4').
        assert!(narrow.label.starts_with("2-2x") || narrow.label.starts_with("3-2x"));
        assert_eq!(narrow.kind, HeaderKind::Lumber);
    }

    #[test]
    fn the_recommendation_is_the_lightest_section_that_passes() {
        let i = HeaderInput {
            span: 60.0,
            ..HeaderInput::default()
        };
        let all = header_options(&i);
        let rec = header_recommend(&i).unwrap();
        let lighter_lumber: Vec<&HeaderOption> = all
            .iter()
            .filter(|o| o.kind == HeaderKind::Lumber)
            .take_while(|o| o.label != rec.label)
            .collect();
        assert!(lighter_lumber.iter().all(|o| !o.passes));
        // One inch past its span the same section no longer passes.
        let edge = HeaderInput {
            span: rec.span.inches,
            ..i
        };
        assert!(header_options(&edge)
            .iter()
            .find(|o| o.label == rec.label)
            .unwrap()
            .passes);
        let over = HeaderInput {
            span: rec.span.inches + 1.0,
            ..i
        };
        assert!(!header_options(&over)
            .iter()
            .find(|o| o.label == rec.label)
            .unwrap()
            .passes);
    }

    #[test]
    fn snow_width_and_a_floor_above_shorten_a_header() {
        let section = |i: HeaderInput| {
            header_options(&i)
                .into_iter()
                .find(|o| o.label == "2-2x10")
                .unwrap()
                .span
                .inches
        };
        let base = HeaderInput::default();
        assert!(section(HeaderInput { ground_snow: 70.0, ..base }) < section(base));
        assert!(
            section(HeaderInput { building_width_ft: 36.0, ..base })
                < section(HeaderInput { building_width_ft: 20.0, ..base })
        );
        assert!(section(HeaderInput { floors_above: 1, ..base }) < section(base));
    }

    #[test]
    fn an_opening_wider_than_any_lumber_header_gets_an_lvl_or_nothing() {
        let wide = header_recommend(&HeaderInput {
            span: 168.0,
            ground_snow: 30.0,
            ..HeaderInput::default()
        });
        match wide {
            Some(o) => assert_eq!(o.kind, HeaderKind::Lvl, "{}", o.label),
            None => panic!("a 14' opening has an LVL header"),
        }
        assert!(header_recommend(&HeaderInput {
            span: 600.0,
            ..HeaderInput::default()
        })
        .is_none());
    }

    #[test]
    fn jack_studs_follow_the_reaction() {
        let narrow = header_recommend(&HeaderInput {
            span: 24.0,
            ..HeaderInput::default()
        })
        .unwrap();
        let wide = header_recommend(&HeaderInput {
            span: 96.0,
            floors_above: 1,
            ..HeaderInput::default()
        })
        .unwrap();
        assert_eq!(narrow.jacks, 1);
        assert!(wide.jacks > narrow.jacks, "{} jacks", wide.jacks);
        assert_eq!(jack_studs(0.0, 625.0, 5.5), 1);
        // Exactly one stud's capacity needs one; a pound more needs two.
        let cap = 625.0 * 1.5 * 5.5;
        assert_eq!(jack_studs(cap, 625.0, 5.5), 1);
        assert_eq!(jack_studs(cap + 1.0, 625.0, 5.5), 2);
    }

    #[test]
    fn stair_layout_follows_the_code() {
        let s = stair_layout(&StairInput::default()).unwrap();
        // 108" at 7" is 15 risers of 7.2".
        assert_eq!(s.risers, 15);
        assert_eq!(s.treads, 14);
        assert!((s.riser_height - 7.2).abs() < 1e-9);
        assert!((s.total_run - 147.0).abs() < 1e-9);
        assert!(s.passes(), "{:?}", s.notes);
        assert!((s.two_r_plus_t - 24.9).abs() < 1e-9);
        // Over the maximum riser: add a riser.
        let steep = stair_layout(&StairInput {
            total_rise: 93.0,
            target_riser: 7.75,
            ..StairInput::default()
        })
        .unwrap();
        assert_eq!(steep.risers, 12);
        assert!(steep.riser_height <= 7.75);
        // The largest rise before another riser is needed: 8 x 7.75 = 62".
        let edge = stair_layout(&StairInput {
            total_rise: 62.0,
            target_riser: 7.75,
            ..StairInput::default()
        })
        .unwrap();
        assert_eq!(edge.risers, 8);
        let over = stair_layout(&StairInput {
            total_rise: 62.1,
            target_riser: 7.75,
            ..StairInput::default()
        })
        .unwrap();
        assert_eq!(over.risers, 9);
    }

    #[test]
    fn stair_notes_flag_shallow_treads_and_long_flights() {
        let shallow = stair_layout(&StairInput {
            tread: 9.0,
            ..StairInput::default()
        })
        .unwrap();
        assert!(!shallow.passes());
        assert!(shallow.notes.iter().any(|n| !n.ok && n.text.contains("Tread")));
        let tall = stair_layout(&StairInput {
            total_rise: 160.0,
            ..StairInput::default()
        })
        .unwrap();
        assert!(tall.notes.iter().any(|n| !n.ok && n.text.contains("R311.7.3")));
        let at = stair_layout(&StairInput {
            total_rise: 151.0,
            ..StairInput::default()
        })
        .unwrap();
        assert!(at.notes.iter().all(|n| !n.text.contains("R311.7.3") || n.ok));
        assert!(stair_layout(&StairInput {
            total_rise: 0.0,
            ..StairInput::default()
        })
        .is_none());
        assert!(stair_layout(&StairInput {
            total_rise: f64::NAN,
            ..StairInput::default()
        })
        .is_none());
        // One riser has no treads.
        let one = stair_layout(&StairInput {
            total_rise: 7.0,
            ..StairInput::default()
        })
        .unwrap();
        assert_eq!((one.risers, one.treads), (1, 0));
    }

    #[test]
    fn deck_joists_are_shorter_than_floor_joists_and_the_beam_grows_with_plies() {
        let d = DeckInput::default();
        let floor = joist(8, 16.0);
        assert!(deck_joist_span(&d).inches < floor);
        let two = deck_beam_span(&d).inches;
        let three = deck_beam_span(&DeckInput {
            beam_plies: 3,
            ..d
        })
        .inches;
        assert!(three > two);
        let deeper = deck_beam_span(&DeckInput {
            beam_nominal: 12,
            ..d
        })
        .inches;
        assert!(deeper > two);
        // A wider deck loads the beam more.
        let wide = deck_beam_span(&DeckInput {
            joist_span: 180.0,
            ..d
        })
        .inches;
        assert!(wide < two);
    }

    #[test]
    fn deck_design_reports_the_cantilever_post_and_footing() {
        let d = DeckInput {
            joist_span: 96.0,
            cantilever: 24.0,
            ..DeckInput::default()
        };
        let r = deck_design(&d);
        assert_eq!(r.cantilever_max, 24.0);
        assert!(r.cantilever_ok);
        let too_far = deck_design(&DeckInput {
            cantilever: 25.0,
            ..d
        });
        assert!(!too_far.cantilever_ok);
        assert!(r.post_load > 0.0);
        // Footing area carries the post load at the soil bearing.
        let soft = deck_design(&DeckInput {
            soil_bearing: 1000.0,
            ..d
        });
        assert!(soft.footing_side >= r.footing_side);
        assert!(r.footing_side * r.footing_side / 144.0 * d.soil_bearing >= r.post_load - 1e-6);
        // A joist span the joists cannot make is flagged.
        let long = deck_design(&DeckInput {
            joist_span: 240.0,
            ..d
        });
        assert!(!long.joist_ok);
    }

    #[test]
    fn stair_limits_follow_the_check_settings() {
        let o = CheckOptions {
            riser_max: 7.0,
            ..CheckOptions::default()
        };
        let s = stair_layout(&StairInput {
            limits: StairLimits::from_options(&o),
            total_rise: 100.0,
            target_riser: 7.75,
            ..StairInput::default()
        })
        .unwrap();
        assert!(s.riser_height <= 7.0, "{}", s.riser_height);
    }

    #[test]
    fn span_text_is_feet_and_inches() {
        let s = Span {
            inches: 153.0,
            governs: Governs::Bending,
        };
        assert_eq!(s.text(), "12'-9\"");
        assert_eq!(Governs::Shear.name(), "shear");
        assert_eq!(Material::Lumber(Species::HemFir, Grade::No1).name(), "Hem-fir No. 1");
        let _ = DF2;
    }
}
