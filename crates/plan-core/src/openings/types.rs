//! Door and window defaults per type, the Window Type list and dynamic
//! ("Use Default") settings (DW-124, DW-154, DW-164; manual pp. 103, 571,
//! 603, 619).
//!
//! Chief keeps one Defaults dialog per door type (Interior and Exterior for
//! Hinged and Sliding, then Doorway, Pocket, Bifold, Garage...) and one for
//! windows. A [`DefaultKey`] names one of them and a [`TypeDefault`] holds the
//! opening it places, looks and all, so the Defaults dialog is the
//! Specification dialog. Several settings are *dynamic*: a placed opening
//! that has not been edited in that group ([`UseDefault`]) follows the default
//! when it changes ([`Project::follow_type_defaults`]), and *Set as Default*
//! ([`OpeningVariantDefaults::set_as_default`]) copies a placed opening into
//! its type's default.

use super::{OpeningStyle, OpeningVariantDefaults};
use crate::model::{Id, Opening, OpeningKind, Project, WallKind};
use serde::{Deserialize, Serialize};

// ----- the Window Type list -----

/// The Window Type list of the Window Specification (General panel, manual
/// p. 619). Each type is drawn by one of the [`OpeningStyle`]s; the type adds
/// how many components the unit has and how it opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum WindowType {
    Fixed,
    SingleHung,
    #[default]
    DoubleHung,
    SingleCasement,
    DoubleCasement,
    TripleCasement,
    LeftSliding,
    RightSliding,
    TripleSliding,
    SingleAwning,
    DoubleAwning,
    TripleAwning,
    SingleHopper,
    DoubleHopper,
    TripleHopper,
    Louvered,
    GlassLouver,
    PassThrough,
}

/// How a window type is opened in 3D views (General panel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenMode {
    /// A Percent Open: hung and sliding windows.
    Percent,
    /// A Swing Angle: casement, awning and hopper windows.
    Angle,
    /// Neither: fixed, louvered and pass-through.
    Never,
}

impl WindowType {
    /// Every type, in the order of the list.
    pub const ALL: [WindowType; 18] = [
        WindowType::Fixed,
        WindowType::SingleHung,
        WindowType::DoubleHung,
        WindowType::SingleCasement,
        WindowType::DoubleCasement,
        WindowType::TripleCasement,
        WindowType::LeftSliding,
        WindowType::RightSliding,
        WindowType::TripleSliding,
        WindowType::SingleAwning,
        WindowType::DoubleAwning,
        WindowType::TripleAwning,
        WindowType::SingleHopper,
        WindowType::DoubleHopper,
        WindowType::TripleHopper,
        WindowType::Louvered,
        WindowType::GlassLouver,
        WindowType::PassThrough,
    ];

    pub fn name(self) -> &'static str {
        match self {
            WindowType::Fixed => "Fixed",
            WindowType::SingleHung => "Single Hung",
            WindowType::DoubleHung => "Double Hung",
            WindowType::SingleCasement => "Single Casement",
            WindowType::DoubleCasement => "Double Casement",
            WindowType::TripleCasement => "Triple Casement",
            WindowType::LeftSliding => "Left Sliding",
            WindowType::RightSliding => "Right Sliding",
            WindowType::TripleSliding => "Triple Sliding",
            WindowType::SingleAwning => "Single Awning",
            WindowType::DoubleAwning => "Double Awning",
            WindowType::TripleAwning => "Triple Awning",
            WindowType::SingleHopper => "Single Hopper",
            WindowType::DoubleHopper => "Double Hopper",
            WindowType::TripleHopper => "Triple Hopper",
            WindowType::Louvered => "Louvered",
            WindowType::GlassLouver => "Glass Louver",
            WindowType::PassThrough => "Pass-Through",
        }
    }

    /// The type called `name` (the Window Type list's text, or an old plan's
    /// type name such as `Double Hung` or `Fixed Glass`).
    pub fn from_name(name: &str) -> Option<WindowType> {
        let n = name.trim();
        if let Some(t) = Self::ALL.iter().find(|t| t.name().eq_ignore_ascii_case(n)) {
            return Some(*t);
        }
        // The names the Window Defaults used before the full list.
        match n.to_ascii_lowercase().as_str() {
            "fixed glass" | "picture" => Some(WindowType::Fixed),
            "slider" | "sliding" => Some(WindowType::LeftSliding),
            "awning" => Some(WindowType::SingleAwning),
            "hopper" => Some(WindowType::SingleHopper),
            "casement" => Some(WindowType::SingleCasement),
            "window" | "hung" => Some(WindowType::DoubleHung),
            _ => None,
        }
    }

    /// The style that draws the type.
    pub fn style(self) -> OpeningStyle {
        match self {
            WindowType::Fixed | WindowType::Louvered | WindowType::GlassLouver => {
                OpeningStyle::Fixed
            }
            WindowType::SingleHung | WindowType::DoubleHung => OpeningStyle::Window,
            WindowType::SingleCasement
            | WindowType::DoubleCasement
            | WindowType::TripleCasement => OpeningStyle::Casement,
            WindowType::LeftSliding | WindowType::RightSliding | WindowType::TripleSliding => {
                OpeningStyle::SlidingWindow
            }
            WindowType::SingleAwning | WindowType::DoubleAwning | WindowType::TripleAwning => {
                OpeningStyle::Awning
            }
            WindowType::SingleHopper | WindowType::DoubleHopper | WindowType::TripleHopper => {
                OpeningStyle::Hopper
            }
            WindowType::PassThrough => OpeningStyle::PassThrough,
        }
    }

    /// A type for a window drawn by `style` (the one a new window of that
    /// style starts with). Projecting units and niches have none.
    pub fn for_style(style: OpeningStyle) -> Option<WindowType> {
        Some(match style {
            OpeningStyle::Window => WindowType::DoubleHung,
            OpeningStyle::Fixed => WindowType::Fixed,
            OpeningStyle::Casement => WindowType::SingleCasement,
            OpeningStyle::SlidingWindow | OpeningStyle::Sliding => WindowType::LeftSliding,
            OpeningStyle::Awning => WindowType::SingleAwning,
            OpeningStyle::Hopper => WindowType::SingleHopper,
            OpeningStyle::PassThrough => WindowType::PassThrough,
            _ => return None,
        })
    }

    /// How many movable components (sashes) the unit has.
    pub fn components(self) -> usize {
        match self {
            WindowType::Fixed
            | WindowType::SingleHung
            | WindowType::SingleCasement
            | WindowType::SingleAwning
            | WindowType::SingleHopper
            | WindowType::Louvered
            | WindowType::GlassLouver
            | WindowType::PassThrough => 1,
            WindowType::DoubleHung
            | WindowType::DoubleCasement
            | WindowType::LeftSliding
            | WindowType::RightSliding
            | WindowType::DoubleAwning
            | WindowType::DoubleHopper => 2,
            WindowType::TripleCasement
            | WindowType::TripleSliding
            | WindowType::TripleAwning
            | WindowType::TripleHopper => 3,
        }
    }

    /// Percent Open or Swing Angle, or neither (General panel).
    pub fn open_mode(self) -> OpenMode {
        match self {
            WindowType::SingleHung
            | WindowType::DoubleHung
            | WindowType::LeftSliding
            | WindowType::RightSliding
            | WindowType::TripleSliding => OpenMode::Percent,
            WindowType::SingleCasement
            | WindowType::DoubleCasement
            | WindowType::TripleCasement
            | WindowType::SingleAwning
            | WindowType::DoubleAwning
            | WindowType::TripleAwning
            | WindowType::SingleHopper
            | WindowType::DoubleHopper
            | WindowType::TripleHopper => OpenMode::Angle,
            WindowType::Fixed
            | WindowType::Louvered
            | WindowType::GlassLouver
            | WindowType::PassThrough => OpenMode::Never,
        }
    }

    /// The label of the Component Size field, `None` for the types with no
    /// such field (Component Options, manual p. 620).
    pub fn component_size_label(self) -> Option<&'static str> {
        match self {
            WindowType::SingleHung
            | WindowType::DoubleHung
            | WindowType::DoubleAwning
            | WindowType::DoubleHopper
            | WindowType::TripleHopper => Some("Bottom Component Size"),
            WindowType::TripleAwning => Some("Top/Bottom Component Size"),
            WindowType::TripleCasement | WindowType::TripleSliding => Some("Side Component Size"),
            WindowType::LeftSliding => Some("Right Component Size"),
            WindowType::RightSliding | WindowType::DoubleCasement => Some("Left Component Size"),
            _ => None,
        }
    }

    /// Whether each component's opening direction can be chosen.
    pub fn has_component_opens(self) -> bool {
        matches!(
            self,
            WindowType::SingleCasement
                | WindowType::DoubleCasement
                | WindowType::TripleCasement
                | WindowType::DoubleAwning
                | WindowType::TripleAwning
                | WindowType::DoubleHopper
                | WindowType::TripleHopper
        )
    }

    /// Whether the Louver Size field applies.
    pub fn is_louvered(self) -> bool {
        matches!(self, WindowType::Louvered | WindowType::GlassLouver)
    }
}

// ----- default slots -----

/// One defaults dialog: a door or window type (and, for Hinged and Sliding
/// doors, whether it is the Interior or the Exterior one).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultKey {
    pub kind: OpeningKind,
    pub style: OpeningStyle,
    /// Exterior doors only (Hinged and Sliding keep a second set).
    pub exterior: bool,
}

impl DefaultKey {
    /// The key of a `kind` and `style`, with `exterior` kept only where the
    /// type has an Interior and an Exterior set.
    pub fn new(kind: OpeningKind, style: OpeningStyle, exterior: bool) -> DefaultKey {
        let both = kind == OpeningKind::Door
            && matches!(style, OpeningStyle::Hinged | OpeningStyle::Sliding);
        DefaultKey {
            kind,
            style,
            exterior: both && exterior,
        }
    }

    /// The main window default: the one the plain Window tool places, whose
    /// type a window set to *Use Default* follows.
    pub fn main_window() -> DefaultKey {
        DefaultKey::new(OpeningKind::Window, OpeningStyle::Window, false)
    }

    /// The defaults slot an opening is governed by. A window whose type is
    /// set to Use Default is governed by the main window default; one placed
    /// in a wall of `wall_kind` takes the Exterior door set in an exterior
    /// wall unless its Specification says otherwise.
    pub fn of(o: &Opening, wall_kind: WallKind) -> DefaultKey {
        if o.kind == OpeningKind::Window && o.extras.spec.dynamic.window_type {
            return Self::main_window();
        }
        let exterior = o
            .extras
            .spec
            .exterior_door
            .unwrap_or(wall_kind == WallKind::Exterior);
        DefaultKey::new(o.kind, o.style, exterior)
    }

    /// The name on the Default Settings list.
    pub fn name(self) -> String {
        let base = self.style.name(self.kind);
        let both = self.kind == OpeningKind::Door
            && matches!(self.style, OpeningStyle::Hinged | OpeningStyle::Sliding);
        if both {
            format!(
                "{} {base}",
                if self.exterior {
                    "Exterior"
                } else {
                    "Interior"
                }
            )
        } else {
            base.to_string()
        }
    }

    /// Every defaults dialog doors have, in the order of the list.
    pub fn doors() -> Vec<DefaultKey> {
        let mut v = Vec::new();
        for style in OpeningStyle::DOORS {
            v.push(DefaultKey::new(OpeningKind::Door, style, false));
            if matches!(style, OpeningStyle::Hinged | OpeningStyle::Sliding) {
                v.push(DefaultKey::new(OpeningKind::Door, style, true));
            }
        }
        v
    }

    /// Every defaults dialog windows have.
    pub fn windows() -> Vec<DefaultKey> {
        OpeningStyle::WINDOWS
            .iter()
            .map(|s| DefaultKey::new(OpeningKind::Window, *s, false))
            .collect()
    }
}

/// The revision of the type defaults the placed openings were last brought up
/// to. It never makes two defaults differ.
#[derive(Debug, Clone, Copy, Default)]
pub struct Followed(pub u32);

impl PartialEq for Followed {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// The opening a type places, with every setting the Specification has.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeDefault {
    pub key: DefaultKey,
    pub template: Opening,
}

// ----- dynamic settings -----

/// A group of settings that can follow its default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DynGroup {
    /// The Window Type (or the Door Style).
    Type,
    Casing,
    Lintel,
    Sash,
    Frame,
    Jamb,
    Hardware,
    Treatments,
    Framing,
    Rough,
    Materials,
}

impl DynGroup {
    pub fn name(self) -> &'static str {
        match self {
            DynGroup::Type => "Type",
            DynGroup::Casing => "Casing",
            DynGroup::Lintel => "Lintel",
            DynGroup::Sash => "Sash",
            DynGroup::Frame => "Frame",
            DynGroup::Jamb => "Jamb",
            DynGroup::Hardware => "Hardware",
            DynGroup::Treatments => "Treatments",
            DynGroup::Framing => "Framing",
            DynGroup::Rough => "Rough Opening",
            DynGroup::Materials => "Materials",
        }
    }

    /// The groups an opening of `kind` has dynamic (manual pp. 572, 603).
    pub fn of_kind(kind: OpeningKind) -> &'static [DynGroup] {
        match kind {
            OpeningKind::Door => &[
                DynGroup::Type,
                DynGroup::Casing,
                DynGroup::Lintel,
                DynGroup::Jamb,
                DynGroup::Hardware,
                DynGroup::Framing,
                DynGroup::Rough,
                DynGroup::Materials,
            ],
            OpeningKind::Window => &[
                DynGroup::Type,
                DynGroup::Casing,
                DynGroup::Lintel,
                DynGroup::Sash,
                DynGroup::Frame,
                DynGroup::Treatments,
                DynGroup::Framing,
                DynGroup::Rough,
                DynGroup::Materials,
            ],
        }
    }
}

/// Which groups of an opening follow the defaults. All off for an opening
/// from an old plan or one built by hand; the tools turn them on when they
/// place from the defaults ([`UseDefault::all`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UseDefault {
    pub window_type: bool,
    pub casing: bool,
    pub lintel: bool,
    pub sash: bool,
    pub frame: bool,
    pub jamb: bool,
    pub hardware: bool,
    pub treatments: bool,
    pub framing: bool,
    pub rough: bool,
    pub materials: bool,
}

impl UseDefault {
    /// Every dynamic group of `kind` on: what a newly placed opening has.
    pub fn all(kind: OpeningKind) -> UseDefault {
        let mut u = UseDefault::default();
        for g in DynGroup::of_kind(kind) {
            u.set(*g, true);
        }
        u
    }

    pub fn get(&self, g: DynGroup) -> bool {
        match g {
            DynGroup::Type => self.window_type,
            DynGroup::Casing => self.casing,
            DynGroup::Lintel => self.lintel,
            DynGroup::Sash => self.sash,
            DynGroup::Frame => self.frame,
            DynGroup::Jamb => self.jamb,
            DynGroup::Hardware => self.hardware,
            DynGroup::Treatments => self.treatments,
            DynGroup::Framing => self.framing,
            DynGroup::Rough => self.rough,
            DynGroup::Materials => self.materials,
        }
    }

    pub fn set(&mut self, g: DynGroup, on: bool) {
        let slot = match g {
            DynGroup::Type => &mut self.window_type,
            DynGroup::Casing => &mut self.casing,
            DynGroup::Lintel => &mut self.lintel,
            DynGroup::Sash => &mut self.sash,
            DynGroup::Frame => &mut self.frame,
            DynGroup::Jamb => &mut self.jamb,
            DynGroup::Hardware => &mut self.hardware,
            DynGroup::Treatments => &mut self.treatments,
            DynGroup::Framing => &mut self.framing,
            DynGroup::Rough => &mut self.rough,
            DynGroup::Materials => &mut self.materials,
        };
        *slot = on;
    }

    /// Whether any group follows its default.
    pub fn any(&self) -> bool {
        *self != UseDefault::default()
    }
}

/// Whether `a` and `b` hold the same values in group `g`.
pub fn group_eq(a: &Opening, b: &Opening, g: DynGroup) -> bool {
    let (sa, sb) = (&a.extras.spec, &b.extras.spec);
    match g {
        DynGroup::Type => {
            a.style == b.style
                && sa.window_type == sb.window_type
                && a.extras.style_name == b.extras.style_name
        }
        DynGroup::Casing => {
            a.casing == b.casing
                && sa.casing_interior == sb.casing_interior
                && sa.casing_exterior == sb.casing_exterior
                && sa.casing_in_plan == sb.casing_in_plan
                && sa.casing_exterior_size == sb.casing_exterior_size
                && sa.casing_profile == sb.casing_profile
                && sa.curved_casing == sb.curved_casing
        }
        DynGroup::Lintel => sa.lintel == sb.lintel && sa.sill == sb.sill,
        DynGroup::Sash => {
            sa.has_sash == sb.has_sash
                && sa.sash_top == sb.sash_top
                && sa.sash_bottom == sb.sash_bottom
                && sa.mullion_width == sb.mullion_width
                && a.extras.sash_width == b.extras.sash_width
        }
        DynGroup::Frame => {
            a.extras.frame_width == b.extras.frame_width
                && sa.size_includes_frame == sb.size_includes_frame
        }
        DynGroup::Jamb => {
            a.extras.jamb_width == b.extras.jamb_width
                && sa.jamb_in_plan == sb.jamb_in_plan
                && sa.size_includes_frame == sb.size_includes_frame
        }
        DynGroup::Hardware => sa.hardware == sb.hardware,
        DynGroup::Treatments => sa.treatments == sb.treatments,
        DynGroup::Framing => sa.framing == sb.framing,
        DynGroup::Rough => sa.rough == sb.rough,
        DynGroup::Materials => sa.materials == sb.materials,
    }
}

/// Copies group `g` of `src` onto `dst`.
pub fn copy_group(dst: &mut Opening, src: &Opening, g: DynGroup) {
    let sp = &src.extras.spec;
    match g {
        DynGroup::Type => {
            dst.style = src.style;
            dst.extras.spec.window_type = sp.window_type;
            dst.extras.style_name = src.extras.style_name.clone();
        }
        DynGroup::Casing => {
            dst.casing = src.casing;
            let d = &mut dst.extras.spec;
            d.casing_interior = sp.casing_interior;
            d.casing_exterior = sp.casing_exterior;
            d.casing_in_plan = sp.casing_in_plan;
            d.casing_exterior_size = sp.casing_exterior_size;
            d.casing_profile = sp.casing_profile;
            d.curved_casing = sp.curved_casing;
        }
        DynGroup::Lintel => {
            dst.extras.spec.lintel = sp.lintel;
            dst.extras.spec.sill = sp.sill;
        }
        DynGroup::Sash => {
            let d = &mut dst.extras.spec;
            d.has_sash = sp.has_sash;
            d.sash_top = sp.sash_top;
            d.sash_bottom = sp.sash_bottom;
            d.mullion_width = sp.mullion_width;
            dst.extras.sash_width = src.extras.sash_width;
        }
        DynGroup::Frame => {
            dst.extras.frame_width = src.extras.frame_width;
            dst.extras.spec.size_includes_frame = sp.size_includes_frame;
        }
        DynGroup::Jamb => {
            dst.extras.jamb_width = src.extras.jamb_width;
            dst.extras.spec.jamb_in_plan = sp.jamb_in_plan;
            dst.extras.spec.size_includes_frame = sp.size_includes_frame;
        }
        DynGroup::Hardware => dst.extras.spec.hardware = sp.hardware,
        DynGroup::Treatments => dst.extras.spec.treatments = sp.treatments,
        DynGroup::Framing => dst.extras.spec.framing = sp.framing.clone(),
        DynGroup::Rough => dst.extras.spec.rough = sp.rough,
        DynGroup::Materials => dst.extras.spec.materials = sp.materials.clone(),
    }
}

/// An opening edited in a dialog: every dynamic group whose values `after`
/// changed relative to `before` stops following its default, unless the
/// edit also turned it on. Returns how many groups were released.
pub fn release_edited_groups(before: &Opening, after: &mut Opening) -> usize {
    let mut n = 0;
    for g in DynGroup::of_kind(after.kind) {
        let was = before.extras.spec.dynamic.get(*g);
        let still = after.extras.spec.dynamic.get(*g);
        if was && still && !group_eq(before, after, *g) {
            after.extras.spec.dynamic.set(*g, false);
            n += 1;
        }
    }
    n
}

// ----- the defaults of every type -----

impl OpeningVariantDefaults {
    /// Whether the type defaults changed since the placed openings last
    /// followed them.
    pub fn needs_follow(&self) -> bool {
        self.revision != self.followed.0
    }

    /// Records that the placed openings follow the defaults as they are now.
    pub fn mark_followed(&mut self) {
        self.followed = Followed(self.revision);
    }

    /// The default stored for `key`, when its dialog has been set.
    pub fn type_default(&self, key: DefaultKey) -> Option<&TypeDefault> {
        self.types.iter().find(|t| t.key == key)
    }

    /// Stores `template` as the default of `key` (the Defaults dialog's OK),
    /// replacing any earlier one, and bumps [`Self::revision`] so placed
    /// openings that use the default pick the change up.
    pub fn set_type_default(&mut self, key: DefaultKey, template: Opening) {
        let mut template = template;
        // A default is a template: nothing about where it stands.
        template.id = 0;
        template.mull_group = None;
        template.extras.spec.mulled = None;
        template.extras.spec.dynamic = UseDefault::default();
        template.extras.spec.level = 0;
        match self.types.iter_mut().find(|t| t.key == key) {
            Some(t) => t.template = template,
            None => self.types.push(TypeDefault { key, template }),
        }
        self.revision = self.revision.wrapping_add(1);
    }

    /// *Set as Default* (Edit toolbar, manual p. 104): `o`, placed in a wall
    /// of `wall_kind`, becomes the default of its type. Returns the key.
    pub fn set_as_default(&mut self, o: &Opening, wall_kind: WallKind) -> DefaultKey {
        let key = DefaultKey::of(o, wall_kind);
        self.set_type_default(key, o.clone());
        key
    }

    /// `template` made into a new opening of `style`: the default of its type
    /// where the Defaults dialog has set one, the older per-style size
    /// otherwise. The new opening follows the default in every dynamic group.
    pub fn place(&self, template: &Opening, style: OpeningStyle, exterior: bool) -> Opening {
        let key = DefaultKey::new(template.kind, style, exterior);
        let mut o = match self.type_default(key) {
            Some(t) => {
                // The geometry fields that belong to the place, not the type.
                let mut o = t.template.clone();
                o.id = template.id;
                o.wall_id = template.wall_id;
                o.center_offset = template.center_offset;
                o.swing_flipped = template.swing_flipped;
                o.hinge_at_end = template.hinge_at_end;
                o.style = style;
                // The main window default's own type shows through.
                if key == DefaultKey::main_window() {
                    o.style = t.template.style;
                }
                // The plain Hinged Door and Window take their size from the
                // plan defaults (the Door and Window Defaults dialogs, the
                // code minimums), which the type default only mirrors.
                if (key.kind == OpeningKind::Door && key.style == OpeningStyle::Hinged)
                    || key == DefaultKey::main_window()
                {
                    o.width = template.width;
                    o.height = template.height;
                    o.sill_height = template.sill_height;
                }
                o
            }
            None => self.apply(template, style),
        };
        o.extras.spec.dynamic = UseDefault::all(o.kind);
        // Only the plain Window tool follows the main window type.
        if o.kind == OpeningKind::Window && style != OpeningStyle::Window {
            o.extras.spec.dynamic.window_type = false;
        }
        // A window whose type was never chosen takes the one its style draws.
        if o.kind == OpeningKind::Window && o.extras.spec.window_type.style() != o.style {
            if let Some(t) = WindowType::for_style(o.style) {
                o.extras.spec.window_type = t;
            }
        }
        o
    }

    /// The type the main window default places (what *Use Default* in the
    /// Window Type list means).
    pub fn main_window_type(&self) -> WindowType {
        match self.type_default(DefaultKey::main_window()) {
            Some(t) => t.template.extras.spec.window_type,
            None => WindowType::default(),
        }
    }
}

impl Project {
    /// Dynamic defaults (manual p. 103): every opening that follows the
    /// default in a group takes the values `defaults` hold now. Returns how
    /// many openings changed.
    pub fn follow_type_defaults(&mut self, defaults: &OpeningVariantDefaults) -> usize {
        let mut changed = 0;
        for fi in 0..self.floors.len() {
            let kinds: Vec<(Id, WallKind)> = self.floors[fi]
                .openings
                .iter()
                .map(|o| {
                    let wk = self.floors[fi]
                        .wall(o.wall_id)
                        .map_or(WallKind::Interior, |w| w.kind);
                    (o.id, wk)
                })
                .collect();
            for (id, wk) in kinds {
                let Some(pos) = self.floors[fi].openings.iter().position(|o| o.id == id) else {
                    continue;
                };
                let me = self.floors[fi].openings[pos].clone();
                if !me.extras.spec.dynamic.any() {
                    continue;
                }
                let mut next = me.clone();
                // The type first: it can change which default governs.
                if me.kind == OpeningKind::Window && me.extras.spec.dynamic.window_type {
                    if let Some(main) = defaults.type_default(DefaultKey::main_window()) {
                        copy_group(&mut next, &main.template, DynGroup::Type);
                    }
                }
                let key = DefaultKey::of(&next, wk);
                if let Some(t) = defaults.type_default(key) {
                    for g in DynGroup::of_kind(me.kind) {
                        if *g == DynGroup::Type {
                            continue;
                        }
                        if me.extras.spec.dynamic.get(*g) {
                            copy_group(&mut next, &t.template, *g);
                        }
                    }
                    // A door that follows its style takes the default's.
                    if me.kind == OpeningKind::Door && me.extras.spec.dynamic.window_type {
                        copy_group(&mut next, &t.template, DynGroup::Type);
                    }
                }
                if next != me {
                    self.floors[fi].openings[pos] = next;
                    changed += 1;
                }
            }
        }
        // A mulled unit edited in a dialog goes to all its components; an
        // opening recessed to a wall layer takes the layer's depth.
        changed + self.sync_unit_specs() + self.sync_recess_depths()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::Project;
    use crate::openings::{Casing, Lintel};

    fn plan() -> (Project, Id) {
        let mut p = Project::new("t");
        let w = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(400.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        (p, w)
    }

    fn template(kind: OpeningKind) -> Opening {
        match kind {
            OpeningKind::Door => Opening::default_door(0, 0, 0.0),
            OpeningKind::Window => Opening::default_window(0, 0, 0.0),
        }
    }

    #[test]
    fn every_window_type_has_a_style_components_and_a_way_to_open() {
        assert_eq!(WindowType::ALL.len(), 18);
        for t in WindowType::ALL {
            // A type's style draws it, and the name finds it again.
            assert_eq!(WindowType::from_name(t.name()), Some(t));
            let n = t.components();
            assert!((1..=3).contains(&n), "{t:?}");
        }
        assert_eq!(WindowType::DoubleHung.style(), OpeningStyle::Window);
        assert_eq!(WindowType::TripleCasement.components(), 3);
        // Percent Open for hung and sliding, Swing Angle for the hinged ones,
        // neither for fixed and louvered (manual p. 619).
        assert_eq!(WindowType::SingleHung.open_mode(), OpenMode::Percent);
        assert_eq!(WindowType::TripleSliding.open_mode(), OpenMode::Percent);
        assert_eq!(WindowType::DoubleCasement.open_mode(), OpenMode::Angle);
        assert_eq!(WindowType::DoubleHopper.open_mode(), OpenMode::Angle);
        assert_eq!(WindowType::Fixed.open_mode(), OpenMode::Never);
        assert_eq!(WindowType::Louvered.open_mode(), OpenMode::Never);
        // The Component Size labels of p. 620.
        assert_eq!(
            WindowType::DoubleHung.component_size_label(),
            Some("Bottom Component Size")
        );
        assert_eq!(
            WindowType::TripleAwning.component_size_label(),
            Some("Top/Bottom Component Size")
        );
        assert_eq!(
            WindowType::TripleSliding.component_size_label(),
            Some("Side Component Size")
        );
        assert_eq!(
            WindowType::LeftSliding.component_size_label(),
            Some("Right Component Size")
        );
        assert_eq!(
            WindowType::RightSliding.component_size_label(),
            Some("Left Component Size")
        );
        assert_eq!(WindowType::Fixed.component_size_label(), None);
        // Old plans' type names still read.
        assert_eq!(
            WindowType::from_name("Fixed Glass"),
            Some(WindowType::Fixed)
        );
        assert_eq!(
            WindowType::from_name("Slider"),
            Some(WindowType::LeftSliding)
        );
    }

    #[test]
    fn hinged_and_sliding_doors_have_interior_and_exterior_defaults() {
        let doors = DefaultKey::doors();
        let hinged: Vec<_> = doors
            .iter()
            .filter(|k| k.style == OpeningStyle::Hinged)
            .collect();
        assert_eq!(hinged.len(), 2);
        assert_eq!(hinged[0].name(), "Interior Hinged Door");
        assert_eq!(hinged[1].name(), "Exterior Hinged Door");
        // Doorway, Pocket, Bifold, Garage have one each.
        for s in [
            OpeningStyle::Doorway,
            OpeningStyle::Pocket,
            OpeningStyle::Bifold,
            OpeningStyle::Garage,
        ] {
            assert_eq!(doors.iter().filter(|k| k.style == s).count(), 1);
        }
        // The exterior flag means nothing where there is one set.
        assert_eq!(
            DefaultKey::new(OpeningKind::Door, OpeningStyle::Pocket, true),
            DefaultKey::new(OpeningKind::Door, OpeningStyle::Pocket, false)
        );
        assert_eq!(DefaultKey::windows().len(), OpeningStyle::WINDOWS.len());
    }

    #[test]
    fn a_door_is_exterior_by_its_wall_unless_it_says_otherwise() {
        let mut d = template(OpeningKind::Door);
        assert!(DefaultKey::of(&d, WallKind::Exterior).exterior);
        assert!(!DefaultKey::of(&d, WallKind::Interior).exterior);
        d.extras.spec.exterior_door = Some(false);
        assert!(!DefaultKey::of(&d, WallKind::Exterior).exterior);
        d.extras.spec.exterior_door = Some(true);
        assert!(DefaultKey::of(&d, WallKind::Interior).exterior);
        // A pocket door is just a pocket door.
        d.style = OpeningStyle::Pocket;
        assert!(!DefaultKey::of(&d, WallKind::Exterior).exterior);
    }

    #[test]
    fn a_placed_opening_uses_the_default_in_every_dynamic_group_until_edited() {
        let (mut p, w) = plan();
        let mut v = OpeningVariantDefaults::default();
        let t = template(OpeningKind::Window);
        let a = v.place(&t, OpeningStyle::Window, false);
        assert_eq!(a.extras.spec.dynamic, UseDefault::all(OpeningKind::Window));
        let a_id = p.alloc_id();
        let mut a = a;
        a.id = a_id;
        a.wall_id = w;
        a.center_offset = 100.0;
        p.floors[0].openings.push(a);
        // Nothing set yet: following changes nothing.
        assert_eq!(p.follow_type_defaults(&v), 0);
        // The main window default gets a lintel and a wide casing.
        let mut def = template(OpeningKind::Window);
        def.extras.spec.lintel = Lintel {
            exterior: true,
            interior: false,
            height: 6.0,
            depth: 1.0,
            extend: 2.0,
            style: crate::openings::LintelStyle::Flat,
        };
        def.casing = Some(Casing {
            width: 5.0,
            depth: 1.0,
            reveal: 0.5,
        });
        v.set_type_default(DefaultKey::main_window(), def);
        let before = v.revision;
        assert_eq!(p.follow_type_defaults(&v), 1);
        let o = &p.floors[0].openings[0];
        assert!(o.extras.spec.lintel.exterior);
        assert_eq!(o.casing.map(|c| c.width), Some(5.0));
        // It still uses the default afterwards.
        assert!(o.extras.spec.dynamic.casing && o.extras.spec.dynamic.lintel);
        // Edited casing: the casing group stops following, the lintel keeps on.
        let mut edited = o.clone();
        edited.casing = Some(Casing {
            width: 2.0,
            depth: 0.5,
            reveal: 0.0,
        });
        let mut after = edited.clone();
        assert_eq!(release_edited_groups(o, &mut after), 1);
        assert!(!after.extras.spec.dynamic.casing && after.extras.spec.dynamic.lintel);
        p.floors[0].openings[0] = after;
        // A second change of the default reaches the lintel but not the casing.
        let mut def2 = template(OpeningKind::Window);
        def2.extras.spec.lintel = Lintel {
            exterior: false,
            interior: true,
            height: 4.0,
            depth: 1.0,
            extend: 1.0,
            style: crate::openings::LintelStyle::Flat,
        };
        def2.casing = Some(Casing {
            width: 6.0,
            depth: 1.0,
            reveal: 0.5,
        });
        v.set_type_default(DefaultKey::main_window(), def2);
        assert!(v.revision > before);
        assert_eq!(p.follow_type_defaults(&v), 1);
        let o = &p.floors[0].openings[0];
        assert!(o.extras.spec.lintel.interior && !o.extras.spec.lintel.exterior);
        assert_eq!(
            o.casing.map(|c| c.width),
            Some(2.0),
            "the edited casing stays"
        );
    }

    #[test]
    fn use_default_window_type_follows_the_main_window_default() {
        let (mut p, w) = plan();
        let mut v = OpeningVariantDefaults::default();
        let id = p.alloc_id();
        let mut o = v.place(&template(OpeningKind::Window), OpeningStyle::Window, false);
        o.id = id;
        o.wall_id = w;
        o.center_offset = 100.0;
        p.floors[0].openings.push(o);
        // The main window default becomes a Triple Casement.
        let mut def = template(OpeningKind::Window);
        def.style = OpeningStyle::Casement;
        def.extras.spec.window_type = WindowType::TripleCasement;
        v.set_type_default(DefaultKey::main_window(), def);
        assert_eq!(p.follow_type_defaults(&v), 1);
        let o = &p.floors[0].openings[0];
        assert_eq!(o.style, OpeningStyle::Casement);
        assert_eq!(o.extras.spec.window_type, WindowType::TripleCasement);
        assert_eq!(v.main_window_type(), WindowType::TripleCasement);
        // A window that chose its type is not moved.
        let id2 = p.alloc_id();
        let mut o2 = v.place(&template(OpeningKind::Window), OpeningStyle::Fixed, false);
        o2.id = id2;
        o2.wall_id = w;
        o2.center_offset = 250.0;
        assert!(
            !o2.extras.spec.dynamic.window_type,
            "a Fixed window tool does not follow the type"
        );
        p.floors[0].openings.push(o2);
        let mut def = template(OpeningKind::Window);
        def.style = OpeningStyle::Awning;
        v.set_type_default(DefaultKey::main_window(), def);
        p.follow_type_defaults(&v);
        let o2 = p.floors[0].openings.iter().find(|o| o.id == id2).unwrap();
        assert_eq!(o2.style, OpeningStyle::Fixed);
    }

    #[test]
    fn set_as_default_stores_the_opening_under_its_key() {
        let mut v = OpeningVariantDefaults::default();
        let mut o = template(OpeningKind::Door);
        o.width = 42.0;
        o.id = 77;
        o.mull_group = Some(3);
        let key = v.set_as_default(&o, WallKind::Exterior);
        assert_eq!(key.name(), "Exterior Hinged Door");
        let t = v.type_default(key).unwrap();
        assert_eq!(t.template.width, 42.0);
        // A template stands nowhere.
        assert_eq!((t.template.id, t.template.mull_group), (0, None));
        assert!(v
            .type_default(DefaultKey::new(
                OpeningKind::Door,
                OpeningStyle::Hinged,
                false
            ))
            .is_none());
        // The plain Hinged Door takes its size from the plan defaults.
        let placed = v.place(&template(OpeningKind::Door), OpeningStyle::Hinged, true);
        assert_eq!(placed.width, 36.0);
        // A pocket door default, though, is its own size.
        let mut pocket = template(OpeningKind::Door);
        pocket.style = OpeningStyle::Pocket;
        pocket.width = 33.0;
        v.set_as_default(&pocket, WallKind::Interior);
        let placed = v.place(&template(OpeningKind::Door), OpeningStyle::Pocket, false);
        assert_eq!(placed.width, 33.0);
    }

    #[test]
    fn the_type_defaults_survive_json_and_an_old_file_has_none() {
        let mut v = OpeningVariantDefaults::default();
        v.set_as_default(&template(OpeningKind::Window), WallKind::Exterior);
        let back: OpeningVariantDefaults =
            serde_json::from_str(&serde_json::to_string(&v).unwrap()).unwrap();
        assert_eq!(back, v);
        let old: OpeningVariantDefaults = serde_json::from_str("{}").unwrap();
        assert!(old.types.is_empty());
    }
}
