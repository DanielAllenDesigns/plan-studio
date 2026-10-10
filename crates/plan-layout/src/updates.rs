//! Keeping layout views current (manual pp. 1401 to 1404).
//!
//! * A **dynamic** view (a plan view, or a camera, elevation or section sent
//!   with Live View, Always Update) draws the plan as it is; there is nothing
//!   to update.
//! * A **semi-dynamic** view (Live View, Update on Demand) keeps the picture
//!   it was last updated with until you update it, and is updated again when
//!   its page is printed.
//! * A **Plot Lines** view keeps its lines until you update it, and printing
//!   does not update it.
//! * A **static** view (a picture) is never updated, only replaced.

use crate::boxview::{CameraLink, UpdateKind};
use crate::model::{Layout, LayoutBox};
use crate::render::{make_art_with, LayoutRenderContext};
use plan_core::Id;

/// Which views Tools > Layout > Update Layout Views updates.
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateScope {
    /// Update All Views: every semi-dynamic and Plot Lines view.
    All,
    /// Update All Live Views: the semi-dynamic views.
    LiveViews,
    /// Update All Plot Line Views.
    PlotLines,
    /// The Update View edit tool: the views you selected.
    Selected(Vec<Id>),
    /// Printing: the semi-dynamic views of the pages printed (Plot Lines
    /// views are left alone).
    OnPrint,
}

/// What an update did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UpdateReport {
    /// Views whose picture was made again.
    pub updated: usize,
    /// Views in scope that cannot be updated: static pictures, or views whose
    /// plan view is gone.
    pub skipped: usize,
    /// Dynamic views in scope: they were current already.
    pub already_current: usize,
}

impl UpdateScope {
    fn wants(&self, b: &LayoutBox) -> bool {
        let kind = b.update_kind();
        match self {
            UpdateScope::All => matches!(kind, UpdateKind::SemiDynamic | UpdateKind::PlotLine),
            UpdateScope::LiveViews | UpdateScope::OnPrint => kind == UpdateKind::SemiDynamic,
            UpdateScope::PlotLines => kind == UpdateKind::PlotLine,
            UpdateScope::Selected(ids) => ids.contains(&b.id),
        }
    }
}

/// Makes the picture of `b` again from the plan. False for a view that keeps
/// no picture or whose source is gone.
pub fn update_box(b: &mut LayoutBox, cx: &LayoutRenderContext) -> bool {
    let scenes = crate::extent::SceneSource::for_context(cx);
    update_box_with(b, cx, &scenes)
}

fn update_box_with(
    b: &mut LayoutBox,
    cx: &LayoutRenderContext,
    scenes: &crate::extent::SceneSource,
) -> bool {
    if !b.has_camera_options() || b.view.camera == CameraLink::Always {
        return false;
    }
    let Some(mut art) = make_art_with(b, cx, scenes) else {
        return false;
    };
    art.rev = b.view.art.as_ref().map_or(1, |a| a.rev + 1);
    b.view.art = Some(art);
    true
}

/// Updates the views `scope` names on every page of `layout` (or only on the
/// pages in `pages`, by number, when given).
pub fn update_views(
    layout: &mut Layout,
    cx: &LayoutRenderContext,
    scope: &UpdateScope,
    pages: Option<&[u32]>,
) -> UpdateReport {
    let scenes = crate::extent::SceneSource::for_context(cx);
    let mut report = UpdateReport::default();
    for p in &mut layout.pages {
        if pages.is_some_and(|ps| !ps.contains(&p.number)) {
            continue;
        }
        for b in &mut p.boxes {
            if !scope.wants(b) {
                continue;
            }
            match b.update_kind() {
                UpdateKind::Dynamic => report.already_current += 1,
                UpdateKind::Static => report.skipped += 1,
                _ => {
                    if update_box_with(b, cx, &scenes) {
                        report.updated += 1;
                    } else {
                        report.skipped += 1;
                    }
                }
            }
        }
    }
    report
}

/// Sets how a cross section, elevation or camera box is linked (Camera View
/// Options). Live View, Always Update drops the kept picture; the others make
/// one if the box has none. False when nothing changed or the box has no
/// camera options.
pub fn set_camera_link(b: &mut LayoutBox, cx: &LayoutRenderContext, link: CameraLink) -> bool {
    if !b.has_camera_options() || b.view.camera == link {
        return false;
    }
    b.view.camera = link;
    if link == CameraLink::Always {
        b.view.art = None;
    } else if b.view.art.is_none() {
        update_box(b, cx);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::BoxSource;
    use plan_core::{Point, WallKind};
    use plan_docs::{Scale, SheetSize};
    use plan_elevation::ViewDir;

    fn project(len: f64) -> plan_core::Project {
        let mut p = plan_core::Project::new("T");
        let c = [
            Point::new(0.0, 0.0),
            Point::new(len, 0.0),
            Point::new(len, 120.0),
            Point::new(0.0, 120.0),
        ];
        for i in 0..4 {
            p.add_wall(0, c[i], c[(i + 1) % 4], 6.5, 109.125, WallKind::Exterior);
        }
        p
    }

    fn layout_with(link: CameraLink) -> Layout {
        let mut layout = Layout::new("L", SheetSize::ArchD);
        layout.add_page(1, "Elevations");
        let mut b = LayoutBox::new(
            1,
            (Point::new(1.0, 1.0), Point::new(10.0, 6.0)),
            BoxSource::Elevation {
                dir: ViewDir::Front,
            },
            Scale::QuarterInch,
        );
        b.view.camera = link;
        layout.page_mut(1).unwrap().boxes.push(b);
        layout
    }

    fn lines_of(layout: &Layout) -> usize {
        layout.pages[0].boxes[0]
            .view
            .art
            .as_ref()
            .map_or(0, |a| a.lines.len())
    }

    #[test]
    fn semi_dynamic_and_plot_line_views_keep_their_picture_until_updated() {
        let short = project(120.0);
        let long = project(480.0);
        let mut layout = layout_with(CameraLink::Always);
        assert_eq!(layout.pages[0].boxes[0].update_kind(), UpdateKind::Dynamic);
        {
            let cx = LayoutRenderContext::new(&short);
            let b = &mut layout.pages[0].boxes[0];
            // Dynamic: nothing is kept.
            assert!(!update_box(b, &cx));
            assert!(set_camera_link(b, &cx, CameraLink::PlotLines));
            assert_eq!(b.update_kind(), UpdateKind::PlotLine);
        }
        let n = lines_of(&layout);
        assert!(n > 0, "the lines of the front elevation are kept");
        let width = |l: &Layout| l.pages[0].boxes[0].view.art.as_ref().unwrap().bounds.1.x;
        let w0 = width(&layout);
        // The plan changes: printing does not touch a Plot Lines view ...
        let cx = LayoutRenderContext::new(&long);
        let r = update_views(&mut layout, &cx, &UpdateScope::OnPrint, None);
        assert_eq!(r, UpdateReport::default());
        assert_eq!(width(&layout), w0);
        // ... Update All Live Views does not either ...
        let r = update_views(&mut layout, &cx, &UpdateScope::LiveViews, None);
        assert_eq!(r.updated, 0);
        // ... only Update All Plot Line Views (or All, or the selection) does.
        let r = update_views(&mut layout, &cx, &UpdateScope::PlotLines, None);
        assert_eq!(r.updated, 1);
        assert!(width(&layout) > w0 + 100.0);
        assert!(layout.pages[0].boxes[0].view.art.as_ref().unwrap().rev >= 2);
    }

    #[test]
    fn update_on_demand_views_update_when_printed() {
        let short = project(120.0);
        let long = project(480.0);
        let mut layout = layout_with(CameraLink::Always);
        {
            let cx = LayoutRenderContext::new(&short);
            let b = &mut layout.pages[0].boxes[0];
            assert!(set_camera_link(b, &cx, CameraLink::OnDemand));
            assert_eq!(b.update_kind(), UpdateKind::SemiDynamic);
        }
        let cx = LayoutRenderContext::new(&long);
        let r = update_views(&mut layout, &cx, &UpdateScope::OnPrint, Some(&[1]));
        assert_eq!(r.updated, 1);
        // Another page's views are left alone.
        let r = update_views(&mut layout, &cx, &UpdateScope::All, Some(&[7]));
        assert_eq!(r, UpdateReport::default());
        // Going back to Always Update drops the picture.
        let b = &mut layout.pages[0].boxes[0];
        assert!(set_camera_link(b, &cx, CameraLink::Always));
        assert!(b.view.art.is_none());
        assert!(!set_camera_link(b, &cx, CameraLink::Always));
    }

    #[test]
    fn static_pictures_are_skipped_and_dynamic_views_are_current() {
        let p = project(120.0);
        let cx = LayoutRenderContext::new(&p);
        let mut layout = layout_with(CameraLink::Always);
        layout.pages[0].boxes.push(LayoutBox::new(
            2,
            (Point::new(1.0, 7.0), Point::new(3.0, 9.0)),
            BoxSource::ImageData {
                width: 1,
                height: 1,
                rgba: vec![0, 0, 0, 255],
            },
            Scale::QuarterInch,
        ));
        let r = update_views(&mut layout, &cx, &UpdateScope::Selected(vec![1, 2]), None);
        assert_eq!((r.updated, r.skipped, r.already_current), (0, 1, 1));
    }
}
