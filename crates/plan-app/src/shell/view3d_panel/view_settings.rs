//! What a camera's saved view settings do to the 3D panel
//! (`docs/parity/3d-views-cameras.md`, C-6, C-31, C-44, C-62, C-67, C-70):
//! the rendering technique by name, the floors a camera shows and the clip at
//! a Floor Camera's ceiling, the backdrop, Preview or Final View quality and
//! the sun and ambient light. Plain functions over the model so they can be
//! tested without a window; the panel calls them every frame.

use plan_3d::{Mesh, Scene};
use plan_core::camera_view::{
    BackdropKind, CameraView, CrossSectionSlider, Lighting, SliderSide, ViewQuality,
};
use plan_core::{CameraKind, CameraObject, Project};
use plan_materials::RenderingTechnique;
use plan_view3d::ViewSettings;

/// Slack above a Floor Camera's ceiling so the ceiling surface itself stays,
/// inches.
pub const CEILING_SLACK: f64 = 0.5;

/// The technique whose menu name is `name`.
pub fn technique_named(name: &str) -> Option<RenderingTechnique> {
    RenderingTechnique::ALL
        .into_iter()
        .find(|t| t.label().eq_ignore_ascii_case(name.trim()))
}

/// The technique a camera asks for: its own choice, else the one its kind
/// implies (a Glass House camera draws as Glass House).
pub fn technique_of(c: &CameraObject) -> Option<RenderingTechnique> {
    c.view
        .technique
        .as_deref()
        .and_then(technique_named)
        .or((c.kind == CameraKind::GlassHouse).then_some(RenderingTechnique::GlassHouse))
}

/// The floors and height limit a camera shows.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraScope {
    /// Only this floor and the ones below it.
    pub floor: Option<usize>,
    /// Nothing above this height (the Floor Camera's ceiling), inches.
    pub clip_above: Option<f64>,
    /// Nothing centred below this height (the lowest floor of a pick),
    /// inches.
    pub clip_below: Option<f64>,
}

/// How far under a picked floor's finished elevation the cut falls, so its
/// floor system stays and the ceiling and walls of the floor below go,
/// inches.
pub const FLOOR_SYSTEM_SLACK: f64 = 14.0;

/// The scope camera `c` has: Floors Displayed, and for a Floor Camera the
/// floor's own ceiling as the top.
pub fn scope_of(project: &Project, c: &CameraObject) -> CameraScope {
    let floor = project.floors.get(c.floor);
    match c.kind {
        CameraKind::FloorCamera => CameraScope {
            floor: Some(c.floor),
            clip_above: floor.map(|f| f.elevation + f.ceiling_height + CEILING_SLACK),
            clip_below: None,
        },
        _ => match c.view.floors.range(c.floor, project.floors.len()) {
            None => CameraScope::default(),
            Some((from, to)) => CameraScope {
                // Everything above the top floor is left out of the build.
                floor: (to + 1 < project.floors.len()).then_some(to),
                clip_above: None,
                clip_below: (from > 0)
                    .then(|| project.floors.get(from))
                    .flatten()
                    .map(|f| f.elevation - FLOOR_SYSTEM_SLACK),
            },
        },
    }
}

/// Drops the triangles that lie wholly above `limit` (scene Y, inches);
/// triangles that cross it stay.
pub fn clip_above(scene: &Scene, limit: f64) -> Scene {
    let limit = limit as f32;
    let mut out = Scene::default();
    for m in &scene.meshes {
        let mut indices = Vec::with_capacity(m.indices.len());
        for tri in m.indices.as_chunks::<3>().0 {
            let lowest = tri
                .iter()
                .map(|&i| m.vertices[i as usize].position[1])
                .fold(f32::INFINITY, f32::min);
            if lowest < limit {
                indices.extend_from_slice(tri);
            }
        }
        if !indices.is_empty() {
            out.meshes.push(Mesh {
                vertices: m.vertices.clone(),
                indices,
                material: m.material,
                object_id: m.object_id,
                color: m.color,
            });
        }
    }
    out
}

/// How the path tracer draws a technique: its own Clay or Physically Based
/// shading and, for the techniques it has no shader for, the look the
/// post-process (`plan_render::stylize`) puts on the picture.
pub fn render_look(t: RenderingTechnique) -> (plan_render::Technique, Option<plan_render::Style>) {
    use plan_render::{Style, Technique};
    match t {
        RenderingTechnique::Clay => (Technique::Clay, None),
        RenderingTechnique::VectorView => (Technique::Clay, Some(Style::VectorView)),
        RenderingTechnique::LineDrawing => (Technique::Clay, Some(Style::LineDrawing)),
        RenderingTechnique::TechnicalIllustration => (
            Technique::PhysicallyBased,
            Some(Style::TechnicalIllustration),
        ),
        RenderingTechnique::Watercolor => (Technique::PhysicallyBased, Some(Style::Watercolor)),
        RenderingTechnique::Standard
        | RenderingTechnique::PhysicallyBased
        | RenderingTechnique::GlassHouse
        | RenderingTechnique::Duotone => (Technique::PhysicallyBased, None),
    }
}

/// Drops the triangles centred below `limit` (scene Y, inches): the floors
/// under the lowest one a camera picks.
pub fn clip_below(scene: &Scene, limit: f64) -> Scene {
    let limit = limit as f32;
    let mut out = Scene::default();
    for m in &scene.meshes {
        let mut indices = Vec::with_capacity(m.indices.len());
        for tri in m.indices.as_chunks::<3>().0 {
            let centre = tri
                .iter()
                .map(|&i| m.vertices[i as usize].position[1])
                .sum::<f32>()
                / 3.0;
            if centre >= limit {
                indices.extend_from_slice(tri);
            }
        }
        if !indices.is_empty() {
            out.meshes.push(Mesh {
                vertices: m.vertices.clone(),
                indices,
                material: m.material,
                object_id: m.object_id,
                color: m.color,
            });
        }
    }
    out
}

/// What lies below the horizon for a camera's Backdrop tab, display-encoded.
pub fn ground_of(view: Option<&CameraView>) -> plan_view3d::Ground {
    use plan_core::camera_view::GroundKind;
    let Some(v) = view else {
        return plan_view3d::Ground::Fade;
    };
    match v.backdrop.ground {
        GroundKind::Default => plan_view3d::Ground::Fade,
        GroundKind::Color => {
            plan_view3d::Ground::Solid(v.backdrop.ground_color.map(|c| f32::from(c) / 255.0))
        }
        GroundKind::None => plan_view3d::Ground::Sky,
    }
}

/// The distance haze of a camera's Backdrop tab.
pub fn fog_of(view: Option<&CameraView>) -> plan_view3d::Fog {
    let Some(v) = view else {
        return plan_view3d::Fog::default();
    };
    plan_view3d::Fog {
        density: v.backdrop.fog.density_per_inch(),
        color: v
            .backdrop
            .fog
            .color
            .map(|c| c.map(|b| f32::from(b) / 255.0)),
    }
}

/// The sky colour of a Sky color backdrop, display-encoded.
pub fn sky_color(view: &CameraView) -> Option<[f32; 4]> {
    (view.backdrop.kind == BackdropKind::Color).then(|| {
        let [r, g, b] = view.backdrop.color;
        [
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            1.0,
        ]
    })
}

/// The unit direction toward the sun in viewport (scene) space.
pub fn key_dir(l: &Lighting) -> [f32; 3] {
    l.to_sun().map(|v| v as f32)
}

/// The viewport's light rig for the plan's [`Lighting`] and, if a camera is
/// showing, its overrides. `flat` techniques draw unlit.
pub fn rig(
    plan: &Lighting,
    view: Option<&CameraView>,
    sun_override: Option<[f32; 3]>,
    flat: bool,
) -> plan_view3d::Lighting {
    if flat {
        return plan_view3d::Lighting {
            ambient: 1.0,
            key: 0.0,
            ..plan_view3d::Lighting::default()
        };
    }
    let base = plan_view3d::Lighting::default();
    let (ambient, sun) = match view {
        // Use Sunlight off (C-150): the sun light source is off in the view.
        Some(v) if !v.options.use_sunlight => (v.ambient_in(plan), 0.0),
        Some(v) => (v.ambient_in(plan), v.sun_in(plan)),
        None => (
            plan.ambient.clamp(0.0, 1.0),
            plan.sun_intensity.clamp(0.0, 2.0),
        ),
    };
    // The default rig (ambient 0.45, key 0.65) is the plan's default lighting.
    plan_view3d::Lighting {
        key_dir: sun_override.unwrap_or_else(|| key_dir(plan)),
        ambient: ambient as f32,
        key: base.key * sun as f32,
    }
}

/// The scale a camera's Ambient Occlusion amount puts on the look's own
/// strength: the slider's middle (0.5) keeps it, 0 turns it off, 1 doubles it.
pub fn ao_scale(view: Option<&CameraView>) -> f32 {
    view.map_or(1.0, |v| (v.options.ambient_occlusion * 2.0).clamp(0.0, 2.0) as f32)
}

/// The lens a camera's Depth of Field settings give the ray tracer:
/// `(aperture, focus distance)` in inches, or `None` when it is off. The
/// aperture opening is 24 inches over the F-Stop, so f/5.6 blurs about as much
/// as the Ray Trace window's own default lens (4 in).
pub fn dof_lens(view: &CameraView) -> Option<(f32, f32)> {
    let o = &view.options;
    o.dof_on
        .then(|| ((24.0 / o.f_stop.max(1.0)) as f32, o.focus_distance.max(1.0) as f32))
}

/// The near clip a camera's Clip Surfaces Within asks for: always for an
/// eye-level view, and for an overview only once changed from the default
/// (an overview's own near plane grows with its distance).
pub fn near_of(view: Option<&CameraView>, eye_level: bool) -> Option<f32> {
    use plan_core::camera_view::spec::DEFAULT_CLIP_WITHIN;
    let v = view?;
    let within = v.options.clip_surfaces_within;
    (eye_level || (within - DEFAULT_CLIP_WITHIN).abs() > 1e-9).then_some(within as f32)
}

/// The view settings for Preview or Final View, keeping the user's exposure;
/// a camera with shadows off keeps them off in Final View too.
pub fn settings_for(quality: ViewQuality, exposure: f32, shadows: bool) -> ViewSettings {
    let mut s = match quality {
        ViewQuality::Preview => ViewSettings::preview(),
        ViewQuality::Final => ViewSettings::final_view(),
    };
    s.exposure = exposure;
    s.shadows &= shadows;
    s
}

// ----- Cross Section Slider, Hide Camera-Facing Walls, Clip Within -----

/// The planes of a camera's Cross Section Slider as a small copyable value
/// for [`super::ViewScope`]: the position of each plane in
/// [`SliderSide::ALL`] order, `None` when its box is unchecked.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SliderClip {
    planes: [Option<f64>; 6],
}

impl SliderClip {
    pub fn of(s: &CrossSectionSlider) -> Self {
        let mut planes = [None; 6];
        for p in s.planes.iter().filter(|p| p.on) {
            if let Some(i) = SliderSide::ALL.iter().position(|x| *x == p.side) {
                planes[i] = Some(p.position);
            }
        }
        Self { planes }
    }

    pub fn is_active(&self) -> bool {
        self.planes.iter().any(Option::is_some)
    }

    fn as_slider(&self) -> CrossSectionSlider {
        let mut s = CrossSectionSlider::default();
        for (side, pos) in SliderSide::ALL.iter().zip(self.planes) {
            if let Some(pos) = pos {
                s.set_on(*side, true);
                s.set_position(*side, pos);
            }
        }
        s
    }

    pub fn hash_into(&self, h: &mut impl std::hash::Hasher) {
        use std::hash::Hash;
        for p in self.planes {
            p.map(f64::to_bits).hash(h);
        }
    }

    /// Drops the triangles (by centroid) that the planes cut away. A plane
    /// is measured from the edge of the model it cuts first, so the bounds
    /// come from `scene` itself.
    pub fn apply(&self, scene: &Scene) -> Scene {
        let Some((lo, hi)) = scene.bounds() else {
            return scene.clone();
        };
        // Scene (x, up, -plan y) to the slider's (plan x, plan y, up).
        let to_plan = |p: [f32; 3]| [f64::from(p[0]), -f64::from(p[2]), f64::from(p[1])];
        let (lo, hi) = (
            [
                f64::from(lo[0]),
                -f64::from(hi[2]),
                f64::from(lo[1]),
            ],
            [
                f64::from(hi[0]),
                -f64::from(lo[2]),
                f64::from(hi[1]),
            ],
        );
        let slider = self.as_slider();
        let mut out = Scene::default();
        for m in &scene.meshes {
            let mut indices = Vec::with_capacity(m.indices.len());
            for tri in m.indices.as_chunks::<3>().0 {
                let mut c = [0.0_f32; 3];
                for &i in tri {
                    let p = m.vertices[i as usize].position;
                    for k in 0..3 {
                        c[k] += p[k] / 3.0;
                    }
                }
                if !slider.removes(to_plan(c), lo, hi) {
                    indices.extend_from_slice(tri);
                }
            }
            if !indices.is_empty() {
                out.meshes.push(Mesh {
                    vertices: m.vertices.clone(),
                    indices,
                    material: m.material,
                    object_id: m.object_id,
                    color: m.color,
                });
            }
        }
        out
    }
}

/// Show Color off (C-146): every mesh drawn in the grey of its own colour.
pub fn gray_scene(scene: &mut Scene) {
    for m in &mut scene.meshes {
        let lin = m.color_linear().unwrap_or_else(|| {
            let c = m.material.color();
            [c[0], c[1], c[2]]
        });
        let y = 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2];
        let g = (y.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0).round() as u8;
        m.color = Some([g, g, g]);
    }
}

/// `scene` without the meshes of the objects in `hidden`.
pub fn without_objects(scene: &Scene, hidden: &std::collections::HashSet<plan_core::Id>) -> Scene {
    Scene {
        meshes: scene
            .meshes
            .iter()
            .filter(|m| m.object_id.is_none_or(|id| !hidden.contains(&id)))
            .cloned()
            .collect(),
    }
}

/// The plan point Hide Camera-Facing Exterior Walls looks from: an eye-level
/// camera's position, an overview's saved eye. `None` when the option is off.
pub fn hide_facing_from(c: &CameraObject) -> Option<plan_core::geometry::Point> {
    use plan_core::geometry::Point;
    if !c.view.options.hide_facing_walls {
        return None;
    }
    if c.kind.is_eye_level() {
        Some(c.position)
    } else if c.kind.is_overview() {
        c.overview_pose().map(|p| Point::new(p.eye[0], -p.eye[2]))
    } else {
        None
    }
}

/// What a camera adds to the scope beyond floors: its slider planes and the
/// plan position to hide camera-facing walls from.
pub fn extra_scope_of(c: &CameraObject) -> (SliderClip, Option<plan_core::geometry::Point>) {
    (
        SliderClip::of(&c.view.slider),
        hide_facing_from(c),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]
    use super::*;
    use plan_core::camera_view::FloorsDisplayed;
    use plan_core::geometry::Point;
    use plan_core::WallKind;

    fn two_storey() -> Project {
        let mut p = Project::new("Two");
        p.build_new_floor(false);
        p
    }

    #[test]
    fn a_technique_is_found_by_its_menu_name() {
        assert_eq!(
            technique_named("glass house"),
            Some(RenderingTechnique::GlassHouse)
        );
        assert_eq!(
            technique_named("  Standard "),
            Some(RenderingTechnique::Standard)
        );
        assert_eq!(technique_named("nope"), None);
        for t in RenderingTechnique::ALL {
            assert_eq!(technique_named(t.label()), Some(t), "{}", t.label());
        }
    }

    #[test]
    fn a_glass_house_camera_draws_as_glass_house_unless_told_otherwise() {
        let mut c = CameraObject::new(CameraKind::GlassHouse, Point::ZERO, 0.0, "G", 0);
        assert_eq!(technique_of(&c), Some(RenderingTechnique::GlassHouse));
        c.view.technique = Some("Clay".into());
        assert_eq!(technique_of(&c), Some(RenderingTechnique::Clay));
        let full = CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "F", 0);
        assert_eq!(technique_of(&full), None);
    }

    #[test]
    fn the_scope_follows_floors_displayed_and_the_floor_camera() {
        let p = two_storey();
        let mut c = CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "F", 0);
        assert_eq!(scope_of(&p, &c), CameraScope::default());
        c.view.floors = FloorsDisplayed::ThisAndBelow;
        assert_eq!(
            scope_of(&p, &c),
            CameraScope {
                floor: Some(0),
                clip_above: None,
                clip_below: None,
            }
        );
        let mut f = CameraObject::new(CameraKind::FloorCamera, Point::ZERO, 0.0, "Fl", 1);
        let s = scope_of(&p, &f);
        assert_eq!(s.floor, Some(1));
        let top = p.floors[1].elevation + p.floors[1].ceiling_height + CEILING_SLACK;
        assert_eq!(s.clip_above, Some(top));
        f.floor = 0;
        assert!(scope_of(&p, &f).clip_above.unwrap() < top);
    }

    #[test]
    fn clipping_drops_what_lies_above_the_ceiling() {
        let mut p = Project::new("House");
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 192.0),
            Point::new(0.0, 192.0),
        ];
        for i in 0..4 {
            p.add_wall(0, c[i], c[(i + 1) % 4], 6.5, 109.125, WallKind::Exterior);
        }
        let scene = plan_3d::build_scene(&p);
        let top = |s: &Scene| s.bounds().unwrap().1[1];
        assert!(top(&scene) > 100.0);
        let clipped = clip_above(&scene, 60.0);
        assert!(clipped.triangle_count() < scene.triangle_count());
        // Triangles crossing the limit stay, so the wall still reaches it.
        assert!(top(&clipped) >= 60.0);
        assert_eq!(
            clip_above(&scene, 1.0e6).triangle_count(),
            scene.triangle_count()
        );
        assert_eq!(clip_above(&scene, -1.0).triangle_count(), 0);
    }

    #[test]
    fn the_light_rig_follows_the_plan_and_the_camera() {
        let plan = Lighting::default();
        let r = rig(&plan, None, None, false);
        assert!((r.ambient - 0.45).abs() < 1e-6 && (r.key - 0.65).abs() < 1e-6);
        let mut v = CameraView::default();
        v.sun_intensity = Some(0.0);
        v.ambient = Some(0.9);
        let r = rig(&plan, Some(&v), None, false);
        assert!((r.ambient - 0.9).abs() < 1e-6 && r.key == 0.0);
        // The Sun Angle window's sun wins over the plan's.
        let r = rig(&plan, None, Some([0.0, 1.0, 0.0]), false);
        assert_eq!(r.key_dir, [0.0, 1.0, 0.0]);
        // Flat techniques are unlit.
        let r = rig(&plan, None, None, true);
        assert_eq!((r.ambient, r.key), (1.0, 0.0));
        // The sun sits where the Lighting dialog put it.
        let mut moved = plan.clone();
        moved.sun_azimuth_deg = 90.0;
        moved.sun_altitude_deg = 10.0;
        assert!(rig(&moved, None, None, false).key_dir[0] > 0.9);
    }

    #[test]
    fn the_quality_presets_keep_exposure_and_the_cameras_shadow_choice() {
        let s = settings_for(ViewQuality::Final, 1.4, true);
        assert!(s.shadows && s.ambient_occlusion);
        assert_eq!(s.exposure, 1.4);
        assert!(!settings_for(ViewQuality::Final, 1.0, false).shadows);
        let p = settings_for(ViewQuality::Preview, 0.8, true);
        assert!(!p.shadows && !p.ambient_occlusion);
        assert_eq!(p.exposure, 0.8);
    }

    #[test]
    fn a_color_backdrop_gives_a_sky_colour_and_the_default_does_not() {
        let mut v = CameraView::default();
        assert_eq!(sky_color(&v), None);
        v.backdrop.kind = BackdropKind::Color;
        v.backdrop.color = [255, 0, 51];
        let c = sky_color(&v).unwrap();
        assert_eq!((c[0], c[1], c[3]), (1.0, 0.0, 1.0));
        assert!((c[2] - 0.2).abs() < 1e-6);
    }

    #[test]
    fn a_picked_range_of_floors_builds_up_to_the_top_floor_and_cuts_below_the_lowest() {
        let mut p = two_storey();
        p.build_new_floor(false);
        assert_eq!(p.floors.len(), 3);
        let mut c = CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "F", 0);
        c.view.floors = FloorsDisplayed::Picked { from: 1, to: 1 };
        let s = scope_of(&p, &c);
        assert_eq!(s.floor, Some(1), "the third floor is left out");
        assert_eq!(
            s.clip_below,
            Some(p.floors[1].elevation - FLOOR_SYSTEM_SLACK)
        );
        // Picking up to the top floor needs no cut above; from the ground, none below.
        c.view.floors = FloorsDisplayed::Picked { from: 0, to: 2 };
        assert_eq!(scope_of(&p, &c), CameraScope::default());
        // A pick given the wrong way round still works.
        c.view.floors = FloorsDisplayed::Picked { from: 2, to: 1 };
        let s = scope_of(&p, &c);
        assert_eq!((s.floor, s.clip_below.is_some()), (None, true));
    }

    #[test]
    fn clipping_below_drops_what_is_centred_under_the_limit() {
        let mut p = Project::new("House");
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 192.0),
            Point::new(0.0, 192.0),
        ];
        for i in 0..4 {
            p.add_wall(0, c[i], c[(i + 1) % 4], 6.5, 109.125, WallKind::Exterior);
        }
        let scene = plan_3d::build_scene(&p);
        let n = scene.triangle_count();
        assert_eq!(clip_below(&scene, -1.0e6).triangle_count(), n);
        assert_eq!(clip_below(&scene, 1.0e6).triangle_count(), 0);
        let half = clip_below(&scene, 50.0);
        assert!(half.triangle_count() < n && half.triangle_count() > 0);
        // What is left is centred at or above the limit: the bounds rise.
        assert!(half.bounds().unwrap().1[1] >= 100.0);
    }

    #[test]
    fn ground_and_fog_come_from_the_backdrop_tab() {
        use plan_core::camera_view::{Fog, GroundKind};
        assert_eq!(ground_of(None), plan_view3d::Ground::Fade);
        assert_eq!(fog_of(None), plan_view3d::Fog::default());
        let mut v = CameraView::default();
        assert_eq!(ground_of(Some(&v)), plan_view3d::Ground::Fade);
        v.backdrop.ground = GroundKind::Color;
        v.backdrop.ground_color = [255, 0, 51];
        let plan_view3d::Ground::Solid(c) = ground_of(Some(&v)) else {
            panic!("a flat ground");
        };
        assert_eq!((c[0], c[1]), (1.0, 0.0));
        v.backdrop.ground = GroundKind::None;
        assert_eq!(ground_of(Some(&v)), plan_view3d::Ground::Sky);
        assert!(!fog_of(Some(&v)).is_on());
        v.backdrop.fog = Fog {
            on: true,
            distance_ft: 100.0,
            color: Some([255, 255, 255]),
        };
        let f = fog_of(Some(&v));
        assert!(f.is_on() && (f.density - 1.0 / 1200.0).abs() < 1e-9);
        assert_eq!(f.color, Some([1.0, 1.0, 1.0]));
    }

    #[test]
    fn techniques_map_to_a_tracer_shading_and_a_look() {
        use plan_render::{Style, Technique};
        assert_eq!(
            render_look(RenderingTechnique::Clay),
            (Technique::Clay, None)
        );
        assert_eq!(
            render_look(RenderingTechnique::VectorView),
            (Technique::Clay, Some(Style::VectorView))
        );
        assert_eq!(
            render_look(RenderingTechnique::Watercolor).1,
            Some(Style::Watercolor)
        );
        assert_eq!(render_look(RenderingTechnique::Standard).1, None);
        // Every look the post-process has is reached by exactly one technique name.
        for st in Style::ALL {
            let n = RenderingTechnique::ALL
                .into_iter()
                .filter(|t| render_look(*t).1 == Some(st))
                .count();
            assert_eq!(n, 1, "{st:?}");
        }
        for t in RenderingTechnique::ALL {
            assert_eq!(
                Style::from_technique_name(t.label()),
                render_look(t).1,
                "{}",
                t.label()
            );
        }
    }

    fn box_house() -> (Project, Vec<plan_core::Id>) {
        let mut p = Project::new("box");
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 180.0),
            Point::new(0.0, 180.0),
        ];
        let mut ids = Vec::new();
        for i in 0..4 {
            let id = p.add_wall(0, pts[i], pts[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior);
            if let Some(w) = p.floors[0].walls.iter_mut().find(|w| w.id == id) {
                w.exterior_side = plan_core::walls::Side::Right;
            }
            ids.push(id);
        }
        (p, ids)
    }

    #[test]
    fn slider_planes_cut_the_scene_from_the_edge_of_the_model() {
        let (p, _) = box_house();
        let all = super::super::build_view_scene(&p, &super::super::ViewScope::default());
        let mut s = CrossSectionSlider::default();
        assert!(!SliderClip::of(&s).is_active());
        s.set_on(SliderSide::Left, true);
        s.set_position(SliderSide::Left, 120.0);
        let half = SliderClip::of(&s).apply(&all);
        assert!(half.triangle_count() > 0 && half.triangle_count() < all.triangle_count());
        // A second plane from above cuts more.
        s.set_on(SliderSide::Top, true);
        s.set_position(SliderSide::Top, 200.0);
        let less = SliderClip::of(&s).apply(&all);
        assert!(less.triangle_count() < half.triangle_count());
        // The scope applies the same planes when a view is built.
        let scope = super::super::ViewScope {
            slider: SliderClip::of(&s),
            ..super::super::ViewScope::default()
        };
        assert_eq!(
            super::super::build_view_scene(&p, &scope).triangle_count(),
            less.triangle_count()
        );
    }

    #[test]
    fn walls_facing_an_outside_camera_leave_the_scene() {
        let (p, ids) = box_house();
        let has = |scene: &Scene, id| scene.meshes.iter().any(|m| m.object_id == Some(id));
        let all = super::super::build_view_scene(&p, &super::super::ViewScope::default());
        assert!(ids.iter().all(|id| has(&all, *id)));
        let scope = super::super::ViewScope {
            hide_from: Some(Point::new(120.0, -400.0)),
            ..super::super::ViewScope::default()
        };
        let seen = super::super::build_view_scene(&p, &scope);
        assert!(!has(&seen, ids[0]), "the wall toward the camera is gone");
        assert!(has(&seen, ids[2]), "the far wall stays");
        // A camera inside the house hides nothing.
        let inside = super::super::ViewScope {
            hide_from: Some(Point::new(120.0, 90.0)),
            ..super::super::ViewScope::default()
        };
        assert!(has(&super::super::build_view_scene(&p, &inside), ids[0]));
        // The option reads the camera: eye-level position, off by default.
        let mut c = CameraObject::new(CameraKind::FullCamera, Point::new(5.0, 6.0), 0.0, "c", 0);
        assert_eq!(hide_facing_from(&c), None);
        c.view.options.hide_facing_walls = true;
        assert_eq!(hide_facing_from(&c), Some(Point::new(5.0, 6.0)));
    }

    #[test]
    fn show_color_off_draws_every_mesh_in_grey() {
        let (p, _) = box_house();
        let mut scene = super::super::build_view_scene(&p, &super::super::ViewScope::default());
        gray_scene(&mut scene);
        assert!(scene
            .meshes
            .iter()
            .all(|m| m.color.is_some_and(|c| c[0] == c[1] && c[1] == c[2])));
    }

    #[test]
    fn camera_options_reach_the_light_the_occlusion_and_the_near_plane() {
        let plan = Lighting::default();
        let mut v = CameraView::default();
        let sun_on = rig(&plan, Some(&v), None, false);
        assert!(sun_on.key > 0.0);
        v.options.use_sunlight = false;
        assert_eq!(rig(&plan, Some(&v), None, false).key, 0.0);
        // The slider's middle keeps the look's occlusion.
        assert_eq!(ao_scale(Some(&CameraView::default())), 1.0);
        v.options.ambient_occlusion = 0.0;
        assert_eq!(ao_scale(Some(&v)), 0.0);
        assert_eq!(ao_scale(None), 1.0);
        // An eye-level camera always sets its near plane; an overview only
        // once it differs from the default.
        v.options.clip_surfaces_within = 2.0;
        assert_eq!(near_of(Some(&v), true), Some(2.0));
        assert_eq!(near_of(Some(&v), false), None);
        v.options.clip_surfaces_within = 12.0;
        assert_eq!(near_of(Some(&v), false), Some(12.0));
        assert_eq!(near_of(None, true), None);
    }

    #[test]
    fn depth_of_field_gives_the_ray_tracer_a_lens() {
        let mut v = CameraView::default();
        assert_eq!(dof_lens(&v), None);
        v.options.dof_on = true;
        v.options.f_stop = 8.0;
        v.options.focus_distance = 144.0;
        assert_eq!(dof_lens(&v), Some((3.0, 144.0)));
        v.options.f_stop = 2.0;
        assert!(dof_lens(&v).unwrap().0 > 3.0, "a lower F-Stop blurs more");
    }
}
