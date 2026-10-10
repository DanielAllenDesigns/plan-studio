//! Per-device options that sit beside the device record (E-32, E-33).
//!
//! A [`Device`](crate::Device) keeps the fields every placed device has; the
//! rarely changed ones live in [`DeviceOptions`], stored by device id in
//! [`ElectricalLayer::options`](crate::ElectricalLayer::options) and written
//! only for a device that differs from the defaults. That keeps old plans and
//! every `Device { .. }` literal valid.
//!
//! * Mounting: wall, floor, ceiling or the side of a cabinet.
//! * Recess (Options panel): Distance from Wall (negative sets the device into
//!   the wall), Cuts Floor / Ceiling / Wall, Cut Depth and Insert Depth.
//! * Size: Width and Height with Retain Aspect Ratio, and what the height is
//!   measured to (Center, Bottom or Top).
//! * Automatically Change Switch Type When Wiring (manual p. 697).

use crate::device::DeviceKind;
use plan_core::Id;
use serde::{Deserialize, Serialize};

/// Where a device is mounted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Mount {
    /// On a wall face (the default for wall devices).
    #[default]
    Wall,
    /// On the floor (floor outlets).
    Floor,
    /// On the ceiling (ceiling lights; a 110V outlet in a garage or slab room).
    Ceiling,
    /// On the side of a cabinet or soffit.
    CabinetSide,
}

impl Mount {
    /// Every choice, in the order the Options panel lists them.
    pub const ALL: [Mount; 4] = [
        Mount::Wall,
        Mount::Floor,
        Mount::Ceiling,
        Mount::CabinetSide,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Mount::Wall => "Wall",
            Mount::Floor => "Floor",
            Mount::Ceiling => "Ceiling",
            Mount::CabinetSide => "Cabinet Side",
        }
    }
}

/// What a height is measured to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HeightTo {
    #[default]
    Center,
    Bottom,
    Top,
}

impl HeightTo {
    pub const ALL: [HeightTo; 3] = [HeightTo::Center, HeightTo::Bottom, HeightTo::Top];

    pub fn name(self) -> &'static str {
        match self {
            HeightTo::Center => "Center",
            HeightTo::Bottom => "Bottom",
            HeightTo::Top => "Top",
        }
    }
}

/// Recessing a wall device into its wall (the Options panel).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Recess {
    /// Distance of the device from the wall face, inches. Negative sets the
    /// device into the wall; zero sits on the face.
    pub distance_from_wall: f64,
    pub cuts_floor: bool,
    pub cuts_ceiling: bool,
    pub cuts_wall: bool,
    /// How deep the cut goes into the host, inches.
    pub cut_depth: f64,
    /// How far the device inserts into the cut, inches.
    pub insert_depth: f64,
}

impl Default for Recess {
    fn default() -> Self {
        Self {
            distance_from_wall: 0.0,
            cuts_floor: false,
            cuts_ceiling: false,
            cuts_wall: false,
            cut_depth: 0.0,
            insert_depth: 0.0,
        }
    }
}

impl Recess {
    /// Is the device recessed (set into its host)?
    pub fn is_recessed(&self) -> bool {
        self.distance_from_wall < -1e-9 || self.cuts_wall || self.cuts_floor || self.cuts_ceiling
    }
}

/// The options of one device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceOptions {
    pub mount: Mount,
    pub recess: Recess,
    /// Width of the plate or fixture, inches; `None` is the kind's default.
    pub width: Option<f64>,
    /// Height (size) of the plate or fixture, inches; `None` is the default.
    pub size_height: Option<f64>,
    /// Keep the default width-to-height ratio when either is edited.
    pub retain_aspect: bool,
    /// What the device's mounting height is measured to in the dialog.
    pub height_to: HeightTo,
    /// A switch becomes 3-way or 4-way as it is wired (on by default).
    pub auto_switch_type: bool,
    /// The cabinet or soffit the device is mounted on, if any.
    pub host: Option<Id>,
    /// The ganged electrical block the device belongs to: the id of the
    /// block's first device, shared by every member (E-25).
    pub gang: Option<Id>,
}

impl Default for DeviceOptions {
    fn default() -> Self {
        Self {
            mount: Mount::Wall,
            recess: Recess::default(),
            width: None,
            size_height: None,
            retain_aspect: true,
            height_to: HeightTo::Center,
            auto_switch_type: true,
            host: None,
            gang: None,
        }
    }
}

impl DeviceOptions {
    /// Do all the options have their defaults (nothing to store)?
    pub fn is_default(&self) -> bool {
        *self == DeviceOptions::default()
    }

    /// The plate or fixture size of a `kind` with these options.
    pub fn size(&self, kind: DeviceKind) -> (f64, f64) {
        let (w, h) = kind.default_size();
        (self.width.unwrap_or(w), self.size_height.unwrap_or(h))
    }

    /// Sets the width; with Retain Aspect Ratio the height follows.
    pub fn set_width(&mut self, kind: DeviceKind, width: f64) {
        let (w0, h0) = kind.default_size();
        self.width = Some(width);
        if self.retain_aspect && w0 > 1e-9 {
            self.size_height = Some(width * h0 / w0);
        }
    }

    /// Sets the height (size); with Retain Aspect Ratio the width follows.
    pub fn set_size_height(&mut self, kind: DeviceKind, height: f64) {
        let (w0, h0) = kind.default_size();
        self.size_height = Some(height);
        if self.retain_aspect && h0 > 1e-9 {
            self.width = Some(height * w0 / h0);
        }
    }

    /// The mounting height measured `to`, given the height to the center
    /// and the device's size.
    pub fn height_measured(&self, kind: DeviceKind, center: f64, to: HeightTo) -> f64 {
        let half = self.size(kind).1 * 0.5;
        match to {
            HeightTo::Center => center,
            HeightTo::Bottom => center - half,
            HeightTo::Top => center + half,
        }
    }

    /// The height to the center from a height measured `to`.
    pub fn height_to_center(&self, kind: DeviceKind, value: f64, to: HeightTo) -> f64 {
        let half = self.size(kind).1 * 0.5;
        match to {
            HeightTo::Center => value,
            HeightTo::Bottom => value + half,
            HeightTo::Top => value - half,
        }
    }
}
