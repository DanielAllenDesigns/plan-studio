//! The face layout tree: Chief's "Front/Sides/Back" face items.

use serde::{Deserialize, Serialize};

use crate::item::ItemProps;

/// Chief's default separation (rail) height, inches.
const SEPARATION: f64 = 1.5;
/// Chief's default drawer front height, inches.
const DRAWER: f64 = 6.0;
/// Slack when comparing summed lengths.
const EPS: f64 = 1e-9;

/// One item in a face layout. A `height` of `0.0` means "auto": the item
/// receives an equal share of whatever height the fixed items leave over.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FaceItem {
    /// A horizontal rail between items (a face-frame rail when framed).
    Separation { height: f64 },
    /// A drawer front.
    Drawer { height: f64 },
    /// A door whose swing side is chosen automatically by its position.
    DoorAuto { height: f64 },
    /// A door hinged on the left.
    DoorLeft { height: f64 },
    /// A door hinged on the right.
    DoorRight { height: f64 },
    /// A pair of doors meeting in the middle.
    DoubleDoor { height: f64 },
    /// An open (front-less) bay.
    Opening { height: f64 },
    /// A solid slab front with no handle (fillers, finished ends).
    Panel { height: f64 },
    /// An appliance or fixture front (for example a dishwasher or a sink tip-out).
    Appliance { height: f64, name: String },
    /// Items placed side by side. Its own `height` follows the vertical rules;
    /// each cell's item fills that height.
    HorizontalLayout { height: f64, cells: Vec<FaceCell> },
    /// A door whose swing side is the left one (Auto Left Door): one door
    /// until the opening is wider than the Auto Door Threshold, then a pair.
    DoorAutoLeft { height: f64 },
    /// A drawer front with no drawer behind it (False Drawer).
    FalseDrawer { height: f64 },
    /// Two drawers side by side.
    DoubleDrawer { height: f64 },
    /// Two false drawer fronts side by side (False Double Drawer).
    FalseDoubleDrawer { height: f64 },
    /// A cutting board that slides out under the top.
    CuttingBoard { height: f64 },
    /// An open bay with roll-out shelves.
    Rollout { height: f64 },
    /// A solid flat surface (Blank Area).
    Blank { height: f64 },
    /// Items stacked top to bottom (Vertical Layout); each item carries its
    /// own height, auto items share what is left.
    VerticalLayout { height: f64, items: Vec<FaceItem> },
    /// An item with settings of its own (Door/Drawer/Side Panel Face Item
    /// Specification, Cabinet Shelf Specification, Lock from Auto Resize).
    Custom {
        item: Box<FaceItem>,
        props: Box<ItemProps>,
    },
}

/// The 18 item types of the Front/Sides/Back panel's Item Type list
/// (reference manual p. 676), in the order that list shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemKind {
    BlankArea,
    HorizontalLayout,
    VerticalLayout,
    FalseDrawer,
    FalseDoubleDrawer,
    Drawer,
    DoubleDrawer,
    CuttingBoard,
    AutoRightDoor,
    AutoLeftDoor,
    LeftDoor,
    RightDoor,
    DoubleDoor,
    DoorPanel,
    Opening,
    Rollout,
    Separation,
    Appliance,
}

impl ItemKind {
    pub const ALL: [ItemKind; 18] = [
        ItemKind::BlankArea,
        ItemKind::HorizontalLayout,
        ItemKind::VerticalLayout,
        ItemKind::FalseDrawer,
        ItemKind::FalseDoubleDrawer,
        ItemKind::Drawer,
        ItemKind::DoubleDrawer,
        ItemKind::CuttingBoard,
        ItemKind::AutoRightDoor,
        ItemKind::AutoLeftDoor,
        ItemKind::LeftDoor,
        ItemKind::RightDoor,
        ItemKind::DoubleDoor,
        ItemKind::DoorPanel,
        ItemKind::Opening,
        ItemKind::Rollout,
        ItemKind::Separation,
        ItemKind::Appliance,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ItemKind::BlankArea => "Blank Area",
            ItemKind::HorizontalLayout => "Layout - Horizontal",
            ItemKind::VerticalLayout => "Layout - Vertical",
            ItemKind::FalseDrawer => "False Drawer",
            ItemKind::FalseDoubleDrawer => "False Double Drawer",
            ItemKind::Drawer => "Drawer",
            ItemKind::DoubleDrawer => "Double Drawer",
            ItemKind::CuttingBoard => "Cutting Board",
            ItemKind::AutoRightDoor => "Door - Auto Right",
            ItemKind::AutoLeftDoor => "Door - Auto Left",
            ItemKind::LeftDoor => "Door - Left",
            ItemKind::RightDoor => "Door - Right",
            ItemKind::DoubleDoor => "Double Door",
            ItemKind::DoorPanel => "Door Panel",
            ItemKind::Opening => "Opening",
            ItemKind::Rollout => "Rollout",
            ItemKind::Separation => "Separation",
            ItemKind::Appliance => "Appliance",
        }
    }

    /// The kinds a user can add or retype to (layouts are made by
    /// splitting, so they are left out of the New Cabinet Face Item list).
    pub fn is_leaf(self) -> bool {
        !matches!(self, ItemKind::HorizontalLayout | ItemKind::VerticalLayout)
    }

    /// A new item of this kind `height` tall (0 = auto). Layouts start
    /// empty.
    pub fn make(self, height: f64) -> FaceItem {
        match self {
            ItemKind::BlankArea => FaceItem::Blank { height },
            ItemKind::HorizontalLayout => FaceItem::HorizontalLayout {
                height,
                cells: Vec::new(),
            },
            ItemKind::VerticalLayout => FaceItem::VerticalLayout {
                height,
                items: Vec::new(),
            },
            ItemKind::FalseDrawer => FaceItem::FalseDrawer { height },
            ItemKind::FalseDoubleDrawer => FaceItem::FalseDoubleDrawer { height },
            ItemKind::Drawer => FaceItem::Drawer { height },
            ItemKind::DoubleDrawer => FaceItem::DoubleDrawer { height },
            ItemKind::CuttingBoard => FaceItem::CuttingBoard { height },
            ItemKind::AutoRightDoor => FaceItem::DoorAuto { height },
            ItemKind::AutoLeftDoor => FaceItem::DoorAutoLeft { height },
            ItemKind::LeftDoor => FaceItem::DoorLeft { height },
            ItemKind::RightDoor => FaceItem::DoorRight { height },
            ItemKind::DoubleDoor => FaceItem::DoubleDoor { height },
            ItemKind::DoorPanel => FaceItem::Panel { height },
            ItemKind::Opening => FaceItem::Opening { height },
            ItemKind::Rollout => FaceItem::Rollout { height },
            ItemKind::Separation => FaceItem::Separation { height },
            ItemKind::Appliance => FaceItem::Appliance {
                height,
                name: "Appliance".to_string(),
            },
        }
    }
}

/// How a door item builds at a given opening width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoorPlan {
    /// No door (the item is not a door).
    None,
    /// One door, hinged on the left or the right.
    Single { left: bool },
    /// A pair meeting in the middle.
    Pair,
}

/// Opening widths up to this are one door for the Auto door items, inches.
pub const AUTO_DOOR_THRESHOLD: f64 = 24.0;

/// A cell of a [`FaceItem::HorizontalLayout`]: an item and its optional width.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaceCell {
    pub item: FaceItem,
    /// Fixed width in inches; `None` shares the remaining width equally.
    pub width: Option<f64>,
}

impl FaceItem {
    /// The item's declared height (`0.0` = auto).
    pub fn height(&self) -> f64 {
        match self {
            FaceItem::Separation { height }
            | FaceItem::Drawer { height }
            | FaceItem::DoorAuto { height }
            | FaceItem::DoorLeft { height }
            | FaceItem::DoorRight { height }
            | FaceItem::DoubleDoor { height }
            | FaceItem::Opening { height }
            | FaceItem::Panel { height }
            | FaceItem::Appliance { height, .. }
            | FaceItem::HorizontalLayout { height, .. }
            | FaceItem::DoorAutoLeft { height }
            | FaceItem::FalseDrawer { height }
            | FaceItem::DoubleDrawer { height }
            | FaceItem::FalseDoubleDrawer { height }
            | FaceItem::CuttingBoard { height }
            | FaceItem::Rollout { height }
            | FaceItem::Blank { height }
            | FaceItem::VerticalLayout { height, .. } => *height,
            FaceItem::Custom { item, .. } => item.height(),
        }
    }

    /// A copy of this item with its height replaced.
    pub fn with_height(&self, h: f64) -> FaceItem {
        let mut item = self.clone();
        item.set_height(h);
        item
    }

    /// Replaces the declared height in place.
    pub fn set_height(&mut self, h: f64) {
        match self {
            FaceItem::Separation { height }
            | FaceItem::Drawer { height }
            | FaceItem::DoorAuto { height }
            | FaceItem::DoorLeft { height }
            | FaceItem::DoorRight { height }
            | FaceItem::DoubleDoor { height }
            | FaceItem::Opening { height }
            | FaceItem::Panel { height }
            | FaceItem::Appliance { height, .. }
            | FaceItem::HorizontalLayout { height, .. }
            | FaceItem::DoorAutoLeft { height }
            | FaceItem::FalseDrawer { height }
            | FaceItem::DoubleDrawer { height }
            | FaceItem::FalseDoubleDrawer { height }
            | FaceItem::CuttingBoard { height }
            | FaceItem::Rollout { height }
            | FaceItem::Blank { height }
            | FaceItem::VerticalLayout { height, .. } => *height = h,
            FaceItem::Custom { item, .. } => item.set_height(h),
        }
    }

    /// The item without its Custom wrapper.
    pub fn base(&self) -> &FaceItem {
        match self {
            FaceItem::Custom { item, .. } => item.base(),
            other => other,
        }
    }

    /// The item without its Custom wrapper, mutably.
    pub fn base_mut(&mut self) -> &mut FaceItem {
        match self {
            FaceItem::Custom { item, .. } => item.base_mut(),
            other => other,
        }
    }

    /// The item's settings, when it has any.
    pub fn props(&self) -> Option<&ItemProps> {
        match self {
            FaceItem::Custom { props, .. } => Some(props),
            _ => None,
        }
    }

    /// Wraps the item with `props` (or unwraps it when they are all
    /// default). Layouts take no settings and come back unchanged.
    pub fn with_props(self, props: ItemProps) -> FaceItem {
        let inner = match self {
            FaceItem::Custom { item, .. } => *item,
            other => other,
        };
        if props.is_default()
            || matches!(
                inner,
                FaceItem::HorizontalLayout { .. } | FaceItem::VerticalLayout { .. }
            )
        {
            return inner;
        }
        FaceItem::Custom {
            item: Box::new(inner),
            props: Box::new(props),
        }
    }

    /// Mutable access to the settings, wrapping the item first when it has
    /// none yet. Call [`FaceItem::tidy`] afterwards to unwrap default ones.
    pub fn props_mut(&mut self) -> Option<&mut ItemProps> {
        if matches!(
            self,
            FaceItem::HorizontalLayout { .. } | FaceItem::VerticalLayout { .. }
        ) {
            return None;
        }
        if !matches!(self, FaceItem::Custom { .. }) {
            let inner = std::mem::replace(self, FaceItem::Blank { height: 0.0 });
            *self = FaceItem::Custom {
                item: Box::new(inner),
                props: Box::default(),
            };
        }
        match self {
            FaceItem::Custom { props, .. } => Some(props),
            _ => None,
        }
    }

    /// Unwraps a Custom item whose settings are all default.
    pub fn tidy(&mut self) {
        if let FaceItem::Custom { item, props } = self {
            if props.is_default() {
                let inner = std::mem::replace(&mut **item, FaceItem::Blank { height: 0.0 });
                *self = inner;
            }
        }
    }

    /// Which of the 18 item types this is (a Custom wrapper reports the
    /// item inside it).
    pub fn kind(&self) -> ItemKind {
        match self.base() {
            FaceItem::Separation { .. } => ItemKind::Separation,
            FaceItem::Drawer { .. } => ItemKind::Drawer,
            FaceItem::DoorAuto { .. } => ItemKind::AutoRightDoor,
            FaceItem::DoorAutoLeft { .. } => ItemKind::AutoLeftDoor,
            FaceItem::DoorLeft { .. } => ItemKind::LeftDoor,
            FaceItem::DoorRight { .. } => ItemKind::RightDoor,
            FaceItem::DoubleDoor { .. } => ItemKind::DoubleDoor,
            FaceItem::Opening { .. } => ItemKind::Opening,
            FaceItem::Panel { .. } => ItemKind::DoorPanel,
            FaceItem::Appliance { .. } => ItemKind::Appliance,
            FaceItem::HorizontalLayout { .. } => ItemKind::HorizontalLayout,
            FaceItem::FalseDrawer { .. } => ItemKind::FalseDrawer,
            FaceItem::DoubleDrawer { .. } => ItemKind::DoubleDrawer,
            FaceItem::FalseDoubleDrawer { .. } => ItemKind::FalseDoubleDrawer,
            FaceItem::CuttingBoard { .. } => ItemKind::CuttingBoard,
            FaceItem::Rollout { .. } => ItemKind::Rollout,
            FaceItem::Blank { .. } => ItemKind::BlankArea,
            FaceItem::VerticalLayout { .. } => ItemKind::VerticalLayout,
            // base() never returns a Custom wrapper.
            FaceItem::Custom { .. } => ItemKind::BlankArea,
        }
    }

    /// True for the items that are a door or a pair of doors.
    pub fn is_door(&self) -> bool {
        matches!(
            self.base(),
            FaceItem::DoorAuto { .. }
                | FaceItem::DoorAutoLeft { .. }
                | FaceItem::DoorLeft { .. }
                | FaceItem::DoorRight { .. }
                | FaceItem::DoubleDoor { .. }
        )
    }

    /// True for the items with a drawer front (real or false).
    pub fn is_drawer(&self) -> bool {
        matches!(
            self.base(),
            FaceItem::Drawer { .. }
                | FaceItem::FalseDrawer { .. }
                | FaceItem::DoubleDrawer { .. }
                | FaceItem::FalseDoubleDrawer { .. }
        )
    }

    /// True for the items whose front is a solid piece that closes the
    /// cabinet face (doors, drawers, panels, blank areas and cutting
    /// boards).
    pub fn is_front(&self) -> bool {
        self.is_door()
            || self.is_drawer()
            || matches!(
                self.base(),
                FaceItem::Panel { .. } | FaceItem::Blank { .. } | FaceItem::CuttingBoard { .. }
            )
    }

    /// True for the items that open onto shelves: Opening, Rollout and the
    /// doors.
    pub fn has_shelves(&self) -> bool {
        self.is_door()
            || matches!(
                self.base(),
                FaceItem::Opening { .. } | FaceItem::Rollout { .. }
            )
    }

    /// How a door item builds when its opening is `width` wide and Auto
    /// doors split above `threshold`.
    pub fn door_plan(&self, width: f64, threshold: f64) -> DoorPlan {
        match self.base() {
            FaceItem::DoorLeft { .. } => DoorPlan::Single { left: true },
            FaceItem::DoorRight { .. } => DoorPlan::Single { left: false },
            FaceItem::DoubleDoor { .. } => DoorPlan::Pair,
            FaceItem::DoorAuto { .. } if width > threshold + 1e-9 => DoorPlan::Pair,
            FaceItem::DoorAuto { .. } => DoorPlan::Single { left: false },
            FaceItem::DoorAutoLeft { .. } if width > threshold + 1e-9 => DoorPlan::Pair,
            FaceItem::DoorAutoLeft { .. } => DoorPlan::Single { left: true },
            _ => DoorPlan::None,
        }
    }

    /// The drawer fronts the item has side by side (0 when it is none).
    pub fn drawer_fronts(&self) -> usize {
        match self.base() {
            FaceItem::Drawer { .. } | FaceItem::FalseDrawer { .. } => 1,
            FaceItem::DoubleDrawer { .. } | FaceItem::FalseDoubleDrawer { .. } => 2,
            _ => 0,
        }
    }

    /// True for the drawers that really open (not the false fronts).
    pub fn opens_as_drawer(&self) -> bool {
        matches!(
            self.base(),
            FaceItem::Drawer { .. } | FaceItem::DoubleDrawer { .. }
        )
    }
}

/// Smallest height or width a dragged divider leaves an item, inches.
pub const MIN_ITEM: f64 = 1.5;

/// A draggable boundary in a face layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Divider {
    /// Between top-level items `above` and `above + 1`.
    Horizontal { above: usize },
    /// Between cells `left` and `left + 1` of the horizontal layout at top-level index `layout`.
    Vertical { layout: usize, left: usize },
}

/// Where a [`Divider`] sits in face coordinates measured from the top-left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DividerHandle {
    pub divider: Divider,
    /// Distance from the top (horizontal dividers) or the left (vertical ones).
    pub pos: f64,
    /// The extent along the divider: `(left, right)` or `(top, bottom)`.
    pub span: (f64, f64),
}

/// A leaf face item placed in face coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedFace {
    /// `(x, y, w, h)`: `x` from the face's left edge, `y` up from the face's
    /// bottom edge. The item's `height` equals `h`.
    pub rect: (f64, f64, f64, f64),
    /// The item itself, without its Custom wrapper.
    pub item: FaceItem,
    /// The item's own settings (style, hardware, shelves, percent open...),
    /// when it has any.
    pub props: Option<ItemProps>,
}

/// The vertical layout of one cabinet side. Items run top to bottom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaceLayout {
    pub items: Vec<FaceItem>,
    /// Stile width for framed cabinets (Chief's default is 1.5").
    pub frame_width: f64,
}

impl Default for FaceLayout {
    fn default() -> Self {
        Self::empty()
    }
}

impl FaceLayout {
    /// A layout with no items.
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            frame_width: SEPARATION,
        }
    }

    /// Chief's default base face: separation, 6" drawer, separation, auto
    /// door, separation. `height` is the face height; the drawer shrinks when
    /// the face is too short for the fixed items to fit.
    pub fn base_default(height: f64) -> Self {
        let drawer = DRAWER.min(((height - 3.0 * SEPARATION) / 3.0).max(0.0));
        Self {
            items: vec![
                FaceItem::Separation { height: SEPARATION },
                FaceItem::Drawer { height: drawer },
                FaceItem::Separation { height: SEPARATION },
                FaceItem::DoorAuto { height: 0.0 },
                FaceItem::Separation { height: SEPARATION },
            ],
            frame_width: SEPARATION,
        }
    }

    /// A wall cabinet face: one auto door, or two stacked doors for tall
    /// (42" and up) cabinets.
    pub fn wall_default(height: f64) -> Self {
        let mut items = vec![
            FaceItem::Separation { height: SEPARATION },
            FaceItem::DoorAuto { height: 0.0 },
            FaceItem::Separation { height: SEPARATION },
        ];
        if height >= 42.0 {
            items.push(FaceItem::DoorAuto { height: 0.0 });
            items.push(FaceItem::Separation { height: SEPARATION });
        }
        Self {
            items,
            frame_width: SEPARATION,
        }
    }

    /// A pantry/oven-tower face: an auto-height upper door over a lower door of
    /// at most 36" (45% of the face on short faces).
    pub fn full_height_default(height: f64) -> Self {
        let lower = 36.0_f64.min(height * 0.45).max(0.0);
        Self {
            items: vec![
                FaceItem::Separation { height: SEPARATION },
                FaceItem::DoorAuto { height: 0.0 },
                FaceItem::Separation { height: SEPARATION },
                FaceItem::DoorAuto { height: lower },
                FaceItem::Separation { height: SEPARATION },
            ],
            frame_width: SEPARATION,
        }
    }

    /// `n` equal auto-height drawers separated by rails (empty for `n == 0`).
    pub fn drawer_bank(n: usize) -> Self {
        let mut items = Vec::new();
        if n > 0 {
            items.push(FaceItem::Separation { height: SEPARATION });
            for _ in 0..n {
                items.push(FaceItem::Drawer { height: 0.0 });
                items.push(FaceItem::Separation { height: SEPARATION });
            }
        }
        Self {
            items,
            frame_width: SEPARATION,
        }
    }

    /// A sink base: a 6" tip-out front named "Sink" over a double door.
    pub fn sink_base() -> Self {
        Self {
            items: vec![
                FaceItem::Separation { height: SEPARATION },
                FaceItem::Appliance {
                    height: DRAWER,
                    name: "Sink".to_string(),
                },
                FaceItem::Separation { height: SEPARATION },
                FaceItem::DoubleDoor { height: 0.0 },
                FaceItem::Separation { height: SEPARATION },
            ],
            frame_width: SEPARATION,
        }
    }

    /// A single solid panel: the face of a filler.
    pub fn filler_panel() -> Self {
        Self {
            items: vec![FaceItem::Panel { height: 0.0 }],
            frame_width: SEPARATION,
        }
    }

    /// One auto door: the default of a custom side or back face.
    pub fn single_door() -> Self {
        Self {
            items: vec![FaceItem::DoorAuto { height: 0.0 }],
            frame_width: SEPARATION,
        }
    }

    /// A single open bay: an appliance opening.
    pub fn opening() -> Self {
        Self {
            items: vec![FaceItem::Opening { height: 0.0 }],
            frame_width: SEPARATION,
        }
    }

    /// The dividers a user can drag, with where each sits (see [`Divider`]).
    ///
    /// Horizontal dividers lie between neighbouring top-level items; each is
    /// listed when a movable (non-separation) item exists on both sides of it.
    /// Vertical dividers lie between cells of a horizontal layout. Fails like
    /// [`FaceLayout::resolve`].
    pub fn dividers(
        &self,
        total_height: f64,
        total_width: f64,
    ) -> Result<Vec<DividerHandle>, String> {
        let heights = distribute(
            self.items.iter().map(FaceItem::height).collect(),
            total_height,
            "height",
        )?;
        let mut out = Vec::new();
        let mut top = 0.0;
        for (i, (item, h)) in self.items.iter().zip(&heights).enumerate() {
            if i + 1 < self.items.len()
                && self.movable_above(i).is_some()
                && self.movable_below(i + 1).is_some()
            {
                out.push(DividerHandle {
                    divider: Divider::Horizontal { above: i },
                    pos: top + h,
                    span: (0.0, total_width),
                });
            }
            if let FaceItem::HorizontalLayout { cells, .. } = item {
                let widths = distribute(
                    cells.iter().map(|c| c.width.unwrap_or(0.0)).collect(),
                    total_width,
                    "width",
                )?;
                let mut x = 0.0;
                for (j, w) in widths
                    .iter()
                    .enumerate()
                    .take(cells.len().saturating_sub(1))
                {
                    x += w;
                    out.push(DividerHandle {
                        divider: Divider::Vertical { layout: i, left: j },
                        pos: x,
                        span: (top, top + h),
                    });
                }
            }
            top += h;
        }
        Ok(out)
    }

    fn movable_above(&self, from: usize) -> Option<usize> {
        (0..=from)
            .rev()
            .find(|&i| !matches!(self.items[i], FaceItem::Separation { .. }))
    }

    fn movable_below(&self, from: usize) -> Option<usize> {
        (from..self.items.len()).find(|&i| !matches!(self.items[i], FaceItem::Separation { .. }))
    }

    /// Drags `divider` by `delta` inches (down for horizontal dividers,
    /// right for vertical ones). The two items either side take the change
    /// and become fixed-size; everything else, separations included, keeps
    /// its size. Nothing changes on error.
    ///
    /// # Errors
    /// When the divider does not exist, the layout does not resolve, or an
    /// item would shrink below [`MIN_ITEM`].
    pub fn drag_divider(
        &mut self,
        total_height: f64,
        total_width: f64,
        divider: Divider,
        delta: f64,
    ) -> Result<(), String> {
        match divider {
            Divider::Horizontal { above } => {
                let heights = distribute(
                    self.items.iter().map(FaceItem::height).collect(),
                    total_height,
                    "height",
                )?;
                let a = self
                    .movable_above(above)
                    .ok_or("nothing above that divider can move")?;
                let b = self
                    .movable_below(above + 1)
                    .ok_or("nothing below that divider can move")?;
                let (ha, hb) = (heights[a] + delta, heights[b] - delta);
                if ha < MIN_ITEM - EPS || hb < MIN_ITEM - EPS {
                    return Err(format!("items cannot be smaller than {MIN_ITEM}\""));
                }
                self.items[a] = self.items[a].with_height(ha);
                self.items[b] = self.items[b].with_height(hb);
            }
            Divider::Vertical { layout, left } => {
                let Some(FaceItem::HorizontalLayout { cells, .. }) = self.items.get_mut(layout)
                else {
                    return Err("that item is not a horizontal layout".to_string());
                };
                if left + 1 >= cells.len() {
                    return Err("no cell to the right of that divider".to_string());
                }
                let widths = distribute(
                    cells.iter().map(|c| c.width.unwrap_or(0.0)).collect(),
                    total_width,
                    "width",
                )?;
                let (wa, wb) = (widths[left] + delta, widths[left + 1] - delta);
                if wa < MIN_ITEM - EPS || wb < MIN_ITEM - EPS {
                    return Err(format!("items cannot be narrower than {MIN_ITEM}\""));
                }
                cells[left].width = Some(wa);
                cells[left + 1].width = Some(wb);
            }
        }
        Ok(())
    }

    /// True when the lowest face item closes the bottom of the box: a
    /// separation or a blank area (or nothing at all). Any other item makes
    /// the cabinet an appliance garage, which has no bottom unless it is
    /// switched on.
    pub fn bottom_is_closed(&self) -> bool {
        fn closed(item: &FaceItem) -> bool {
            match item.base() {
                FaceItem::Separation { .. } | FaceItem::Blank { .. } => true,
                FaceItem::HorizontalLayout { cells, .. } => cells.iter().all(|c| closed(&c.item)),
                FaceItem::VerticalLayout { items, .. } => items.last().is_none_or(closed),
                _ => false,
            }
        }
        self.items.last().is_none_or(closed)
    }

    fn is_locked(item: &FaceItem) -> bool {
        item.props().is_some_and(|p| p.locked)
    }

    /// Follows a change of the face height from `old` to `new` inches:
    /// auto items share the difference; a layout of fixed items gives it to
    /// the lowest item that is not a separation and not locked (going up
    /// when that item would fall below [`MIN_ITEM`]). Nothing changes on
    /// error.
    ///
    /// # Errors
    /// When no item can take the change.
    pub fn fit_height(&mut self, old: f64, new: f64) -> Result<(), String> {
        let delta = new - old;
        if delta.abs() < EPS || self.items.is_empty() {
            return Ok(());
        }
        let fixed: f64 = self.items.iter().map(FaceItem::height).sum();
        let has_auto = self.items.iter().any(|i| i.height() <= 0.0);
        if has_auto {
            // The auto items absorb it as long as the fixed ones still fit.
            return if fixed <= new + EPS {
                Ok(())
            } else {
                let mut me = self.clone();
                me.shrink_fixed(fixed - new)?;
                *self = me;
                Ok(())
            };
        }
        let mut me = self.clone();
        let mut heights: Vec<f64> = me.items.iter().map(FaceItem::height).collect();
        let mut left = delta;
        for i in (0..me.items.len()).rev() {
            if matches!(me.items[i].base(), FaceItem::Separation { .. })
                || Self::is_locked(&me.items[i])
            {
                continue;
            }
            let next = (heights[i] + left).max(MIN_ITEM);
            left -= next - heights[i];
            heights[i] = next;
            if left.abs() < EPS {
                break;
            }
        }
        if left.abs() > EPS {
            return Err("No face item can take the change in height".to_string());
        }
        for (item, h) in me.items.iter_mut().zip(heights) {
            if (item.height() - h).abs() > EPS {
                item.set_height(h);
            }
        }
        *self = me;
        Ok(())
    }

    /// Takes `amount` inches from fixed items, lowest first, never leaving
    /// one smaller than [`MIN_ITEM`].
    fn shrink_fixed(&mut self, amount: f64) -> Result<(), String> {
        let mut left = amount;
        for i in (0..self.items.len()).rev() {
            let h = self.items[i].height();
            if h <= 0.0
                || matches!(self.items[i].base(), FaceItem::Separation { .. })
                || Self::is_locked(&self.items[i])
            {
                continue;
            }
            let take = (h - MIN_ITEM).max(0.0).min(left);
            self.items[i].set_height(h - take);
            left -= take;
            if left < EPS {
                return Ok(());
            }
        }
        Err("No face item can take the change in height".to_string())
    }

    /// Sets the declared height of top-level item `index` to `h` while the
    /// face stays `total` tall: the lowest item that is not a separation
    /// takes the difference. When the lowest item itself changes, the item
    /// above it takes an increase and a decrease leaves a separation and a
    /// blank area below it (reference manual p. 662). Nothing changes on
    /// error.
    ///
    /// # Errors
    /// When the layout does not resolve or an item would fall below
    /// [`MIN_ITEM`].
    pub fn set_item_height(&mut self, total: f64, index: usize, h: f64) -> Result<(), String> {
        if index >= self.items.len() {
            return Err("no such face item".to_string());
        }
        if h < MIN_ITEM - EPS {
            return Err(format!("items cannot be smaller than {MIN_ITEM}\""));
        }
        let heights = distribute(
            self.items.iter().map(FaceItem::height).collect(),
            total,
            "height",
        )?;
        let diff = h - heights[index];
        if diff.abs() < EPS {
            return Ok(());
        }
        let loose =
            |i: usize, items: &[FaceItem]| !matches!(items[i].base(), FaceItem::Separation { .. });
        let lowest = (0..self.items.len()).rev().find(|&i| loose(i, &self.items));
        let mut me = self.clone();
        for (item, hh) in me.items.iter_mut().zip(&heights) {
            item.set_height(*hh);
        }
        if Some(index) != lowest {
            let k = lowest.ok_or("nothing can take the change")?;
            let take = heights[k] - diff;
            if take < MIN_ITEM - EPS {
                return Err(format!("items cannot be smaller than {MIN_ITEM}\""));
            }
            me.items[index].set_height(h);
            me.items[k].set_height(take);
        } else if diff > 0.0 {
            let above = (0..index).rev().find(|&i| loose(i, &me.items));
            let k = above.ok_or("no item above can give up the height")?;
            let take = heights[k] - diff;
            if take < MIN_ITEM - EPS {
                return Err(format!("items cannot be smaller than {MIN_ITEM}\""));
            }
            me.items[index].set_height(h);
            me.items[k].set_height(take);
        } else {
            let freed = -diff;
            me.items[index].set_height(h);
            if freed >= SEPARATION + MIN_ITEM {
                me.items.push(FaceItem::Separation { height: SEPARATION });
                me.items.push(FaceItem::Blank {
                    height: freed - SEPARATION,
                });
            } else {
                me.items.push(FaceItem::Blank { height: freed });
            }
        }
        *self = me;
        Ok(())
    }

    /// True when any item (at any depth) is an [`FaceItem::Appliance`] named `name`.
    pub fn has_appliance(&self, name: &str) -> bool {
        fn walk(item: &FaceItem, name: &str) -> bool {
            match item {
                FaceItem::Appliance { name: n, .. } => n == name,
                FaceItem::HorizontalLayout { cells, .. } => {
                    cells.iter().any(|c| walk(&c.item, name))
                }
                FaceItem::VerticalLayout { items, .. } => items.iter().any(|i| walk(i, name)),
                FaceItem::Custom { item, .. } => walk(item, name),
                _ => false,
            }
        }
        self.items.iter().any(|i| walk(i, name))
    }

    /// Place every leaf item in face coordinates (see [`ResolvedFace::rect`]).
    ///
    /// Items with height `0` split the height left over by the fixed items
    /// equally; `HorizontalLayout` cells with no width do the same with the
    /// width. Horizontal layouts are flattened to their cells. Fixed items are
    /// stacked from the top, so a layout without auto items that falls short
    /// of `total_height` leaves a gap at the bottom.
    ///
    /// # Errors
    /// Returns a message when fixed heights (or widths) exceed what is
    /// available, or when a size is negative or not finite.
    pub fn resolve(
        &self,
        total_height: f64,
        total_width: f64,
    ) -> Result<Vec<ResolvedFace>, String> {
        let heights = distribute(
            self.items.iter().map(FaceItem::height).collect(),
            total_height,
            "height",
        )?;
        let mut out = Vec::new();
        let mut top = total_height;
        for (item, h) in self.items.iter().zip(heights) {
            let y = top - h;
            place(item, 0.0, y, total_width, h, &mut out)?;
            top = y;
        }
        Ok(out)
    }
}

/// Split `total` among `sizes` (`<= 0` = auto) and return each final size.
fn distribute(sizes: Vec<f64>, total: f64, what: &str) -> Result<Vec<f64>, String> {
    if sizes.iter().any(|s| !s.is_finite() || *s < 0.0) {
        return Err(format!("{what} must be finite and not negative"));
    }
    let fixed: f64 = sizes.iter().sum();
    if fixed > total + EPS {
        return Err(format!(
            "fixed item {what}s total {fixed:.3}\" but only {total:.3}\" is available"
        ));
    }
    let autos = sizes.iter().filter(|s| **s <= 0.0).count();
    let share = if autos > 0 {
        (total - fixed) / autos as f64
    } else {
        0.0
    };
    Ok(sizes
        .into_iter()
        .map(|s| if s <= 0.0 { share } else { s })
        .collect())
}

/// Emit `item` into the rect `(x, y, w, h)`, recursing through horizontal
/// and vertical layouts.
fn place(
    item: &FaceItem,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    out: &mut Vec<ResolvedFace>,
) -> Result<(), String> {
    match item {
        FaceItem::HorizontalLayout { cells, .. } => {
            let widths = distribute(
                cells.iter().map(|c| c.width.unwrap_or(0.0)).collect(),
                w,
                "width",
            )?;
            let mut cx = x;
            for (cell, cw) in cells.iter().zip(widths) {
                place(&cell.item, cx, y, cw, h, out)?;
                cx += cw;
            }
        }
        FaceItem::VerticalLayout { items, .. } => {
            let heights = distribute(items.iter().map(FaceItem::height).collect(), h, "height")?;
            let mut top = y + h;
            for (sub, sh) in items.iter().zip(heights) {
                top -= sh;
                place(sub, x, top, w, sh, out)?;
            }
        }
        FaceItem::Custom { item, props } => {
            let before = out.len();
            place(item, x, y, w, h, out)?;
            for r in &mut out[before..] {
                if r.props.is_none() {
                    r.props = Some((**props).clone());
                }
            }
        }
        leaf => out.push(ResolvedFace {
            rect: (x, y, w, h),
            item: leaf.with_height(h),
            props: None,
        }),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_default_fills_box_height_exactly() {
        let r = FaceLayout::base_default(34.5).resolve(34.5, 21.0).unwrap();
        assert_eq!(r.len(), 5);
        let sum: f64 = r.iter().map(|f| f.rect.3).sum();
        assert!((sum - 34.5).abs() < 1e-9, "sum {sum}");
        // Door gets the rest: 34.5 - 1.5 - 6 - 1.5 - 1.5 = 24.
        assert!((r[3].rect.3 - 24.0).abs() < 1e-9);
        // Top item sits at the top of the face, bottom item at y = 0.
        assert!((r[0].rect.1 + r[0].rect.3 - 34.5).abs() < 1e-9);
        assert!(r[4].rect.1.abs() < 1e-9);
    }

    #[test]
    fn resolve_errors_when_fixed_items_exceed_height() {
        let layout = FaceLayout {
            items: vec![
                FaceItem::Drawer { height: 20.0 },
                FaceItem::Drawer { height: 20.0 },
            ],
            frame_width: 1.5,
        };
        assert!(layout.resolve(34.5, 24.0).is_err());
    }

    #[test]
    fn horizontal_layout_splits_widths() {
        let layout = FaceLayout {
            items: vec![FaceItem::HorizontalLayout {
                height: 0.0,
                cells: vec![
                    FaceCell {
                        item: FaceItem::Drawer { height: 0.0 },
                        width: Some(10.0),
                    },
                    FaceCell {
                        item: FaceItem::DoorAuto { height: 0.0 },
                        width: None,
                    },
                    FaceCell {
                        item: FaceItem::DoorAuto { height: 0.0 },
                        width: None,
                    },
                ],
            }],
            frame_width: 1.5,
        };
        let r = layout.resolve(30.0, 40.0).unwrap();
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].rect, (0.0, 0.0, 10.0, 30.0));
        assert_eq!(r[1].rect, (10.0, 0.0, 15.0, 30.0));
        assert_eq!(r[2].rect, (25.0, 0.0, 15.0, 30.0));
        let too_wide = FaceLayout {
            items: vec![FaceItem::HorizontalLayout {
                height: 0.0,
                cells: vec![FaceCell {
                    item: FaceItem::Drawer { height: 0.0 },
                    width: Some(50.0),
                }],
            }],
            frame_width: 1.5,
        };
        assert!(too_wide.resolve(30.0, 40.0).is_err());
    }

    #[test]
    fn presets_resolve_and_sink_is_detected() {
        for (l, h) in [
            (FaceLayout::wall_default(30.0), 30.0),
            (FaceLayout::wall_default(42.0), 42.0),
            (FaceLayout::full_height_default(80.0), 80.0),
            (FaceLayout::drawer_bank(3), 30.0),
            (FaceLayout::sink_base(), 30.0),
        ] {
            let r = l.resolve(h, 24.0).unwrap();
            assert!(!r.is_empty());
        }
        assert!(FaceLayout::sink_base().has_appliance("Sink"));
        assert!(!FaceLayout::base_default(34.5).has_appliance("Sink"));
        assert!(FaceLayout::drawer_bank(0).items.is_empty());
    }

    #[test]
    fn dragging_a_divider_moves_only_its_two_neighbours() {
        let mut l = FaceLayout::base_default(34.5);
        let before = l.resolve(34.5, 24.0).unwrap();
        // Drawer 6", door 24": move the divider under the drawer down 3".
        l.drag_divider(34.5, 24.0, Divider::Horizontal { above: 1 }, 3.0)
            .unwrap();
        let after = l.resolve(34.5, 24.0).unwrap();
        assert!((after[1].rect.3 - 9.0).abs() < 1e-9);
        assert!((after[3].rect.3 - 21.0).abs() < 1e-9);
        // Separations and the total stay put.
        assert_eq!(after[0].rect.3, before[0].rect.3);
        let sum: f64 = after.iter().map(|f| f.rect.3).sum();
        assert!((sum - 34.5).abs() < 1e-9);
        // Dragging back restores the layout's resolved rects.
        l.drag_divider(34.5, 24.0, Divider::Horizontal { above: 1 }, -3.0)
            .unwrap();
        let back = l.resolve(34.5, 24.0).unwrap();
        for (a, b) in before.iter().zip(&back) {
            assert!((a.rect.1 - b.rect.1).abs() < 1e-9 && (a.rect.3 - b.rect.3).abs() < 1e-9);
        }
        // Too far leaves everything untouched.
        let snapshot = l.clone();
        assert!(l
            .drag_divider(34.5, 24.0, Divider::Horizontal { above: 1 }, 30.0)
            .is_err());
        assert_eq!(l, snapshot);
    }

    #[test]
    fn dragging_a_vertical_divider_resizes_cells() {
        let mut l = FaceLayout {
            items: vec![FaceItem::HorizontalLayout {
                height: 0.0,
                cells: vec![
                    FaceCell {
                        item: FaceItem::DoorLeft { height: 0.0 },
                        width: None,
                    },
                    FaceCell {
                        item: FaceItem::DoorRight { height: 0.0 },
                        width: None,
                    },
                ],
            }],
            frame_width: 1.5,
        };
        let d = l.dividers(30.0, 40.0).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].divider, Divider::Vertical { layout: 0, left: 0 });
        assert!((d[0].pos - 20.0).abs() < 1e-9);
        l.drag_divider(30.0, 40.0, d[0].divider, 5.0).unwrap();
        let r = l.resolve(30.0, 40.0).unwrap();
        assert!((r[0].rect.2 - 25.0).abs() < 1e-9 && (r[1].rect.2 - 15.0).abs() < 1e-9);
        assert!(l.drag_divider(30.0, 40.0, d[0].divider, -30.0).is_err());
        assert!(l
            .drag_divider(30.0, 40.0, Divider::Vertical { layout: 3, left: 0 }, 1.0)
            .is_err());
    }

    #[test]
    fn dividers_skip_separations_only_runs() {
        let l = FaceLayout::base_default(34.5);
        let d = l.dividers(34.5, 24.0).unwrap();
        // Of the four boundaries (sep|drawer, drawer|sep, sep|door, door|sep)
        // the first has nothing movable above and the last nothing below.
        assert_eq!(d.len(), 2);
        assert!((d[0].pos - 7.5).abs() < 1e-9);
        assert!((d[1].pos - 9.0).abs() < 1e-9);
        assert!(FaceLayout::filler_panel()
            .dividers(30.0, 3.0)
            .unwrap()
            .is_empty());
        assert_eq!(
            FaceLayout::opening().items,
            vec![FaceItem::Opening { height: 0.0 }]
        );
    }
}
