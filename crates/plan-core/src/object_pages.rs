//! The panels most specification dialogs share (reference manual: Label Panel
//! p. 730, Schedule Panel p. 735, Manufacturer Panel p. 655), kept per object
//! beside the custom properties in `Project.props` (`PropTable::pages`).
//!
//! Every panel is optional on an object: no entry means the defaults, and an
//! entry equal to the defaults is dropped ([`ObjectPages::prune`]), so a plan
//! that never touches these panels stores nothing and old files load as they
//! were.
//!
//! * [`LabelPage`]: automatic or custom label (with macros), Suppress Label in
//!   All Views, offsets and angle, and the label layer (system, object or
//!   custom). [`LabelPage::text`] makes the text the plan draws.
//! * [`SchedulePage`]: Include in Schedule, Show Schedule Callout, the callout
//!   rotation and the custom schedule categories.
//! * [`ManufacturerPage`]: the contact block of a manufacturer-catalog item.
//! * [`crate::elevation_ref::ElevationRef`] of the object's height field.

use crate::elevation_ref::ElevationRef;
use crate::openings::{OpeningKind, OpeningStyle, SizeFormat};
use crate::text_styles::{expand_macros, MacroContext, TextMacros};
use crate::units::fmt_ft_in;
use serde::{Deserialize, Serialize};

/// Which layer an object's label sits on (Label Layer, manual p. 734).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LabelLayer {
    /// The system layer of the object type's labels ("Cabinets, Labels").
    #[default]
    System,
    /// The layer of the object itself.
    Object,
    /// Any layer saved in the plan: [`LabelPage::custom_layer`].
    Custom,
}

impl LabelLayer {
    pub const ALL: [LabelLayer; 3] = [LabelLayer::System, LabelLayer::Object, LabelLayer::Custom];

    pub fn name(self) -> &'static str {
        match self {
            LabelLayer::System => "Use System Layer",
            LabelLayer::Object => "Use Object Layer",
            LabelLayer::Custom => "Use Custom Layer",
        }
    }
}

/// The Label panel of one object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LabelPage {
    /// Suppress Label in All Views.
    pub suppress: bool,
    /// Display in Plan View.
    pub display_in_plan: bool,
    /// Specify Label (else the Automatic Label).
    pub specify: bool,
    /// The custom label; may use macros (see [`expand_label`]).
    pub text: String,
    /// Use Default Formatting for the size macros.
    pub default_formatting: bool,
    /// Size Format of the Automatic Label (doors and windows).
    pub size_format: SizeFormat,
    /// Include Schedule Number in the Automatic Label.
    pub include_schedule_number: bool,
    /// Include Type in the Automatic Label.
    pub include_type: bool,
    pub layer: LabelLayer,
    /// The layer of [`LabelLayer::Custom`].
    pub custom_layer: String,
    /// X Offset: along the object's front face, inches.
    pub offset_x: f64,
    /// Y Offset: away from the front face, inches.
    pub offset_y: f64,
    /// Angle, degrees.
    pub angle: f64,
    /// Absolute Angle (else relative to the object's front face).
    pub absolute_angle: bool,
}

impl Default for LabelPage {
    fn default() -> Self {
        Self {
            suppress: false,
            display_in_plan: true,
            specify: false,
            text: String::new(),
            default_formatting: true,
            size_format: SizeFormat::WidthHeight,
            include_schedule_number: false,
            include_type: false,
            layer: LabelLayer::System,
            custom_layer: String::new(),
            offset_x: 0.0,
            offset_y: 0.0,
            angle: 0.0,
            absolute_angle: false,
        }
    }
}

/// What the macros of a label can report about the object.
#[derive(Debug, Clone, Default)]
pub struct LabelFacts {
    /// The Automatic Label.
    pub automatic: String,
    /// The object's type name (`Double Hung`, `Base Cabinet`).
    pub type_name: String,
    pub name: String,
    pub width: Option<f64>,
    pub depth: Option<f64>,
    pub height: Option<f64>,
    pub length: Option<f64>,
    /// Elevation above the floor, inches.
    pub elevation: Option<f64>,
    pub schedule_number: String,
    pub code: String,
    pub comment: String,
    pub description: String,
    pub manufacturer: String,
    pub supplier: String,
}

impl LabelPage {
    /// Is the label drawn in plan at all?
    pub fn shown(&self) -> bool {
        !self.suppress && self.display_in_plan
    }

    /// The label text: the Automatic Label (with the schedule number in
    /// front when asked), or the custom label with its macros expanded. `None`
    /// when the label is suppressed or hidden in plan, or when it is empty.
    pub fn text(
        &self,
        facts: &LabelFacts,
        ctx: &MacroContext,
        user: &TextMacros,
    ) -> Option<String> {
        if !self.shown() {
            return None;
        }
        let text = if self.specify {
            expand_label(&self.text, facts, self.default_formatting, ctx, user)
        } else if self.include_schedule_number && !facts.schedule_number.is_empty() {
            format!("{} {}", facts.schedule_number, facts.automatic)
        } else {
            facts.automatic.clone()
        };
        let text = text.trim().to_string();
        (!text.is_empty()).then_some(text)
    }

    /// The names of the layers a label of an object on `object_layer` goes
    /// to: the system label layer `system`, the object's own layer or the
    /// custom one.
    pub fn layer_name(&self, system: &str, object_layer: &str) -> String {
        match self.layer {
            LabelLayer::System => system.to_string(),
            LabelLayer::Object => object_layer.to_string(),
            LabelLayer::Custom if !self.custom_layer.trim().is_empty() => {
                self.custom_layer.trim().to_string()
            }
            LabelLayer::Custom => system.to_string(),
        }
    }
}

/// A length for a label macro: feet-inches with units (Default Formatting) or
/// decimal inches without.
pub fn fmt_label_length(inches: f64, default_formatting: bool) -> String {
    if default_formatting {
        fmt_ft_in(inches)
    } else {
        let s = format!("{inches:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Expands the object macros of a custom label (`%width%`, `%depth%`,
/// `%height%`, `%length%`, `%elevation%`, `%type%`, `%name%`, `%code%`,
/// `%comment%`, `%description%`, `%manufacturer%`, `%supplier%`,
/// `%schedule_number%`, `%automatic_label%`), then the global and user macros
/// (`%plan.name%`, `%floor%`, ...). Unknown names stay as typed.
pub fn expand_label(
    text: &str,
    facts: &LabelFacts,
    default_formatting: bool,
    ctx: &MacroContext,
    user: &TextMacros,
) -> String {
    let len = |v: Option<f64>| {
        v.map(|v| fmt_label_length(v, default_formatting))
            .unwrap_or_default()
    };
    let pairs: [(&str, String); 14] = [
        ("automatic_label", facts.automatic.clone()),
        ("type", facts.type_name.clone()),
        ("name", facts.name.clone()),
        ("width", len(facts.width)),
        ("depth", len(facts.depth)),
        ("height", len(facts.height)),
        ("length", len(facts.length)),
        ("elevation", len(facts.elevation)),
        ("schedule_number", facts.schedule_number.clone()),
        ("code", facts.code.clone()),
        ("comment", facts.comment.clone()),
        ("description", facts.description.clone()),
        ("manufacturer", facts.manufacturer.clone()),
        ("supplier", facts.supplier.clone()),
    ];
    let mut out = text.to_string();
    for (name, value) in pairs {
        out = out.replace(&format!("%{name}%"), &value);
    }
    expand_macros(&out, ctx, user)
}

/// The object macros the Insert Macro menu of a label offers, with help.
pub const OBJECT_MACROS: &[(&str, &str)] = &[
    ("automatic_label", "The Automatic Label"),
    ("type", "The type of the object"),
    ("name", "The object's name"),
    ("width", "Width"),
    ("depth", "Depth"),
    ("height", "Height"),
    ("length", "Length"),
    ("elevation", "Elevation above the floor"),
    ("schedule_number", "The Schedule Number"),
    ("code", "Code from Object Information"),
    ("comment", "Comment from Object Information"),
    ("description", "Description from Object Information"),
    ("manufacturer", "Manufacturer from Object Information"),
    ("supplier", "Supplier from Object Information"),
];

/// The short type code a window's Automatic Label adds when Include Type is
/// checked (`3040 DH`); empty for a style with none.
pub fn type_abbreviation(style: OpeningStyle) -> &'static str {
    match style {
        OpeningStyle::Window => "DH",
        OpeningStyle::Casement => "CS",
        OpeningStyle::SlidingWindow => "SL",
        OpeningStyle::Awning => "AW",
        OpeningStyle::Hopper => "HP",
        OpeningStyle::Fixed => "FX",
        OpeningStyle::BayWindow => "BAY",
        OpeningStyle::BowWindow => "BOW",
        OpeningStyle::BoxWindow => "BOX",
        OpeningStyle::PassThrough => "PT",
        OpeningStyle::WallNiche => "NI",
        _ => "",
    }
}

/// The Automatic Label of a door or window of `width` x `height` inches: the
/// size in `format` (`3040`, `4030`, or the width alone), then for a window
/// with Include Type the type code (`3040 DH`); a door's Include Type changes
/// its schedule columns, not its label. A schedule number goes in front when
/// asked.
pub fn opening_automatic_label(
    kind: OpeningKind,
    style: OpeningStyle,
    width: f64,
    height: f64,
    format: SizeFormat,
    include_type: bool,
    schedule_number: Option<&str>,
) -> String {
    let size = crate::openings::size_text(
        width,
        height,
        format,
        crate::openings::SizeStyle::Shorthand,
    );
    let mut text = size;
    if include_type && kind == OpeningKind::Window {
        let code = type_abbreviation(style);
        if !code.is_empty() {
            text = format!("{text} {code}");
        }
    }
    match schedule_number.map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => format!("{n} {text}"),
        None => text,
    }
}

/// The Schedule panel of one object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchedulePage {
    /// Include in Schedule.
    pub include: bool,
    /// Show Schedule Callout.
    pub show_callout: bool,
    /// Callout Location Rotation, degrees.
    pub callout_rotation: f64,
    /// Include in Schedule As: the custom categories (empty is Auto Schedule
    /// Category).
    pub categories: Vec<String>,
}

impl Default for SchedulePage {
    fn default() -> Self {
        Self {
            include: true,
            show_callout: true,
            callout_rotation: 0.0,
            categories: Vec::new(),
        }
    }
}

/// The Manufacturer panel: contact information of a catalog item.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ManufacturerPage {
    pub name: String,
    pub contact: String,
    pub phone: String,
    pub email: String,
    pub website: String,
    pub address: String,
    pub catalog: String,
}

/// Everything the shared panels hold for one object.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ObjectPages {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<LabelPage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<SchedulePage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<ManufacturerPage>,
    /// What the height field of the object is measured from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elevation: Option<ElevationRef>,
}

impl ObjectPages {
    /// Nothing but defaults.
    pub fn is_empty(&self) -> bool {
        self.label.is_none()
            && self.schedule.is_none()
            && self.manufacturer.is_none()
            && self.elevation.is_none()
    }

    /// Drops every panel that holds only its defaults.
    pub fn prune(&mut self) {
        if self.label.as_ref() == Some(&LabelPage::default()) {
            self.label = None;
        }
        if self.schedule.as_ref() == Some(&SchedulePage::default()) {
            self.schedule = None;
        }
        if self.manufacturer.as_ref() == Some(&ManufacturerPage::default()) {
            self.manufacturer = None;
        }
        if self.elevation.is_some_and(|e| e.is_legacy()) {
            self.elevation = None;
        }
    }

    /// The label panel as shown: the stored one, else the defaults.
    pub fn label_or_default(&self) -> LabelPage {
        self.label.clone().unwrap_or_default()
    }

    pub fn schedule_or_default(&self) -> SchedulePage {
        self.schedule.clone().unwrap_or_default()
    }

    pub fn manufacturer_or_default(&self) -> ManufacturerPage {
        self.manufacturer.clone().unwrap_or_default()
    }

    pub fn elevation_or_default(&self) -> ElevationRef {
        self.elevation.unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elevation_ref::{ElevationBase, ElevationEdge};

    fn facts() -> LabelFacts {
        LabelFacts {
            automatic: "3040 DH".into(),
            type_name: "Double Hung".into(),
            width: Some(36.0),
            height: Some(48.0),
            schedule_number: "W3".into(),
            code: "AND-3040".into(),
            manufacturer: "Andersen".into(),
            ..LabelFacts::default()
        }
    }

    fn ctx() -> MacroContext {
        MacroContext {
            plan_name: "Hill House".into(),
            floor_name: "1st Floor".into(),
            floor_number: 1,
            floor_count: 2,
            ..MacroContext::default()
        }
    }

    #[test]
    fn label_macros_expand_with_and_without_default_formatting() {
        let user = TextMacros::default();
        let page = LabelPage {
            specify: true,
            text: "%type% %width% x %height%\n%code% (%manufacturer%) %plan.name%".into(),
            ..LabelPage::default()
        };
        assert_eq!(
            page.text(&facts(), &ctx(), &user).unwrap(),
            "Double Hung 3'-0\" x 4'-0\"\nAND-3040 (Andersen) Hill House"
        );
        let plain = LabelPage {
            default_formatting: false,
            ..page.clone()
        };
        assert_eq!(
            plain.text(&facts(), &ctx(), &user).unwrap(),
            "Double Hung 36 x 48\nAND-3040 (Andersen) Hill House"
        );
        // Unknown names and stray percent signs stay as typed.
        let odd = LabelPage {
            specify: true,
            text: "100% %nope% %width%".into(),
            ..LabelPage::default()
        };
        assert_eq!(
            odd.text(&facts(), &ctx(), &user).unwrap(),
            "100% %nope% 3'-0\""
        );
        // A user macro nests.
        let mut user = TextMacros::default();
        assert!(user.add("tag", "[%schedule_number%]"));
        let tagged = LabelPage {
            specify: true,
            text: "%tag%".into(),
            ..LabelPage::default()
        };
        assert_eq!(tagged.text(&facts(), &ctx(), &user).unwrap(), "[W3]");
    }

    #[test]
    fn the_automatic_label_and_the_suppress_switches() {
        let user = TextMacros::default();
        let mut page = LabelPage::default();
        assert_eq!(page.text(&facts(), &ctx(), &user).unwrap(), "3040 DH");
        page.include_schedule_number = true;
        assert_eq!(page.text(&facts(), &ctx(), &user).unwrap(), "W3 3040 DH");
        page.suppress = true;
        assert_eq!(page.text(&facts(), &ctx(), &user), None);
        page.suppress = false;
        page.display_in_plan = false;
        assert_eq!(page.text(&facts(), &ctx(), &user), None);
        // An empty custom label draws nothing.
        let empty = LabelPage {
            specify: true,
            ..LabelPage::default()
        };
        assert_eq!(empty.text(&facts(), &ctx(), &user), None);
    }

    #[test]
    fn the_label_layer_is_system_object_or_custom() {
        let mut page = LabelPage::default();
        assert_eq!(page.layer_name("Walls, Labels", "Walls"), "Walls, Labels");
        page.layer = LabelLayer::Object;
        assert_eq!(page.layer_name("Walls, Labels", "Walls"), "Walls");
        page.layer = LabelLayer::Custom;
        // A custom layer with no name falls back to the system layer.
        assert_eq!(page.layer_name("Walls, Labels", "Walls"), "Walls, Labels");
        page.custom_layer = "Notes".into();
        assert_eq!(page.layer_name("Walls, Labels", "Walls"), "Notes");
    }

    #[test]
    fn window_automatic_labels_follow_the_size_format_and_type() {
        use OpeningKind::{Door, Window};
        let w = |fmt, ty, n| {
            opening_automatic_label(Window, OpeningStyle::Window, 36.0, 48.0, fmt, ty, n)
        };
        assert_eq!(w(SizeFormat::WidthHeight, false, None), "3040");
        assert_eq!(w(SizeFormat::WidthHeight, true, None), "3040 DH");
        assert_eq!(w(SizeFormat::HeightWidth, true, None), "4030 DH");
        assert_eq!(w(SizeFormat::WidthOnly, false, None), "30");
        assert_eq!(w(SizeFormat::WidthHeight, true, Some("W3")), "W3 3040 DH");
        assert_eq!(w(SizeFormat::WidthHeight, true, Some(" ")), "3040 DH");
        // A door's Include Type leaves its label alone.
        assert_eq!(
            opening_automatic_label(
                Door,
                OpeningStyle::Hinged,
                36.0,
                80.0,
                SizeFormat::WidthHeight,
                true,
                None
            ),
            "3068"
        );
        assert_eq!(
            opening_automatic_label(
                Window,
                OpeningStyle::Casement,
                30.0,
                48.0,
                SizeFormat::WidthHeight,
                true,
                None
            ),
            "2640 CS"
        );
    }

    #[test]
    fn defaults_are_pruned_and_the_pages_round_trip() {
        let mut p = ObjectPages {
            label: Some(LabelPage::default()),
            schedule: Some(SchedulePage::default()),
            manufacturer: Some(ManufacturerPage::default()),
            elevation: Some(ElevationRef::default()),
        };
        assert!(!p.is_empty());
        p.prune();
        assert!(p.is_empty());
        p.schedule = Some(SchedulePage {
            include: false,
            categories: vec!["Allowance".into()],
            ..SchedulePage::default()
        });
        p.elevation = Some(ElevationRef::new(
            ElevationBase::FromCeiling,
            ElevationEdge::ToTop,
        ));
        p.label = Some(LabelPage {
            specify: true,
            text: "%width%".into(),
            layer: LabelLayer::Custom,
            custom_layer: "Notes".into(),
            offset_x: 4.0,
            ..LabelPage::default()
        });
        p.manufacturer = Some(ManufacturerPage {
            name: "Kohler".into(),
            ..ManufacturerPage::default()
        });
        p.prune();
        let json = serde_json::to_string(&p).unwrap();
        let back: ObjectPages = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
        // An empty record stores nothing.
        assert_eq!(
            serde_json::to_string(&ObjectPages::default()).unwrap(),
            "{}"
        );
        let old: ObjectPages = serde_json::from_str("{}").unwrap();
        assert!(old.is_empty());
        assert!(old.schedule_or_default().include);
        assert!(old.label_or_default().shown());
    }
}
