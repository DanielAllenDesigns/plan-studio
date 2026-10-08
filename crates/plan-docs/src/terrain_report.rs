//! The terrain's cut/fill report: soil moved by every graded pad (the building
//! pad and the terrain features set to grade the ground), as a table for the
//! plan sheets and layout boxes and as the soil lines of the Materials List.
//!
//! The terrain is read from `Project::terrain` (the `terrain` member of the
//! stored record) and the volumes come from [`plan_terrain::cut_fill_report`]:
//! cubic yards cut and filled under each pad and in its sloped sides.

use crate::schedule::Schedule;
use plan_core::units::fmt_ft_in;
use plan_core::Project;
use plan_terrain::{cut_fill_report, CutFillReport, Terrain};

/// Columns of the cut/fill table.
pub const CUT_FILL_COLUMNS: [&str; 6] = [
    "Pad",
    "Top",
    "Area sq ft",
    "Cut cu yd",
    "Fill cu yd",
    "Net cu yd",
];

/// Volumes below this many cubic yards are not listed.
const MIN_YARDS: f64 = 0.005;

/// The terrain the project stores, if there is one.
pub fn terrain_of(project: &Project) -> Option<Terrain> {
    let value = project.terrain.as_ref()?.get("terrain")?.clone();
    serde_json::from_value(value).ok()
}

/// Cut and fill of every graded pad of the project's terrain (empty without a
/// terrain or a pad).
pub fn cut_fill_of(project: &Project) -> CutFillReport {
    terrain_of(project)
        .map(|t| cut_fill_report(&t))
        .unwrap_or_default()
}

fn yards(v: f64) -> String {
    format!("{v:.1}")
}

/// The cut/fill report as a table: one row per pad (name, top elevation, area,
/// cubic yards cut, filled and net) and a total row. `None` when the terrain
/// has no graded pad.
pub fn cut_fill_schedule(project: &Project) -> Option<Schedule> {
    let report = cut_fill_of(project);
    if report.is_empty() {
        return None;
    }
    let mut rows: Vec<Vec<String>> = report
        .items
        .iter()
        .map(|i| {
            vec![
                i.name.clone(),
                fmt_ft_in(i.top),
                format!("{:.0}", i.area_sq_ft),
                yards(i.cut_cy()),
                yards(i.fill_cy()),
                yards(i.cut_cy() - i.fill_cy()),
            ]
        })
        .collect();
    rows.push(vec![
        "Total".into(),
        String::new(),
        format!(
            "{:.0}",
            report.items.iter().map(|i| i.area_sq_ft).sum::<f64>()
        ),
        yards(report.cut_cy()),
        yards(report.fill_cy()),
        yards(report.net_cy()),
    ]);
    Some(Schedule {
        title: "Cut and Fill".into(),
        columns: CUT_FILL_COLUMNS.map(String::from).to_vec(),
        rows,
    })
}

/// Total `(cut, fill)` soil in cubic yards for the Materials List's
/// Landscaping lines; each is 0 when there is none.
pub fn soil_yards(project: &Project) -> (f64, f64) {
    let report = cut_fill_of(project);
    let keep = |v: f64| if v >= MIN_YARDS { v } else { 0.0 };
    (keep(report.cut_cy()), keep(report.fill_cy()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::Point;
    use plan_terrain::{BuildingPad, Feature, FeatureKind};

    fn square(x: f64, y: f64, s: f64) -> Vec<Point> {
        vec![
            Point::new(x, y),
            Point::new(x + s, y),
            Point::new(x + s, y + s),
            Point::new(x, y + s),
        ]
    }

    fn project_with(t: &Terrain) -> Project {
        let mut p = Project::new("Site");
        p.terrain = Some(serde_json::json!({
            "terrain": serde_json::to_value(t).unwrap(),
            "contour_interval": 12.0,
            "built": true,
        }));
        p
    }

    #[test]
    fn no_terrain_and_no_pad_give_no_table() {
        assert!(cut_fill_schedule(&Project::new("Empty")).is_none());
        assert_eq!(soil_yards(&Project::new("Empty")), (0.0, 0.0));
        let p = project_with(&Terrain::default());
        assert!(cut_fill_schedule(&p).is_none());
    }

    #[test]
    fn a_graded_pad_has_a_row_and_the_total() {
        let mut t = Terrain::default();
        t.features.push(Feature {
            kind: FeatureKind::Rectangular,
            polygon: square(480.0, 360.0, 240.0),
            height: 12.0,
            pad: true,
            ..Feature::default()
        });
        let p = project_with(&t);
        let table = cut_fill_schedule(&p).unwrap();
        assert_eq!(table.columns, CUT_FILL_COLUMNS);
        assert_eq!(table.rows.len(), 2);
        let row = &table.rows[0];
        assert!(row[0].starts_with("Rectangular Feature 1"));
        assert_eq!(row[1], "1'-0\"");
        assert_eq!(row[2], "400");
        // 20' x 20' x 1' of fill under the pad plus the sides: about 17.9 cu yd (14.8 under the pad).
        let fill: f64 = row[4].parse().unwrap();
        assert!((17.0..19.0).contains(&fill), "{fill}");
        assert_eq!(row[3], "0.0");
        let net: f64 = row[5].parse().unwrap();
        assert!((net + fill).abs() < 0.11, "net {net} vs fill {fill}");
        assert_eq!(table.rows[1][0], "Total");
        assert_eq!(table.rows[1][4], row[4]);
        // CSV and Markdown come with the schedule.
        assert!(table.to_csv().starts_with("Pad,Top,Area sq ft,Cut cu yd"));
        assert!(table.to_markdown().contains("### Cut and Fill"));
    }

    #[test]
    fn the_materials_list_gets_cut_and_fill_lines_under_landscaping() {
        let t = Terrain {
            building_pad: Some(BuildingPad {
                footprint: square(400.0, 300.0, 360.0),
                first_floor: Some(-48.0),
                ..BuildingPad::default()
            }),
            ..Terrain::default()
        };
        let p = project_with(&t);
        let (cut, fill) = soil_yards(&p);
        assert!(cut > 20.0 && fill == 0.0, "{cut} cut, {fill} fill");
        let lines = crate::materials_report(
            &p,
            crate::MaterialsScope::AllFloors,
            None,
            &crate::MasterList::without_waste(),
        );
        let soil: Vec<_> = lines
            .iter()
            .filter(|l| l.category == "Landscaping" && l.unit == "cu yd")
            .collect();
        assert_eq!(soil.len(), 1, "only the cut: nothing to fill");
        assert!(soil[0].item.contains("cut"));
        assert!((soil[0].quantity - cut).abs() < 0.011);
        assert!(soil[0].id.starts_with("LS-"));
    }
}
