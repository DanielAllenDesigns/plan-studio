//! Browsing the material library: the list of the Material Painter palette
//! and the Library Browser dock when it is filtered to Materials.
//!
//! Both show the same tree (categories, then materials) with a search field.
//! A click makes a material the active one, a double-click or the context
//! menu's Edit opens its Material Specification, and the context menu saves a
//! core material to My Materials (the user library file) or removes the
//! user's copy.

use super::{
    active_material, library, open_new_spec, open_spec_for, remove_from_user_library,
    save_to_user_library, set_active, set_painter_mode, state, swatch, user_library, PainterMode,
};
use crate::editor::EditorContext;
use eframe::egui;
use plan_materials::{MaterialClass, MaterialDef, MaterialLibrary};

/// What the browsers remember between frames.
#[derive(Default)]
pub struct BrowserState {
    /// Only the user's own materials.
    pub mine_only: bool,
    /// The last thing a context menu did.
    pub status: String,
}

/// The materials `lib` has that match `search` (every one for an empty
/// search), the user's own only when `mine_only`.
pub fn matching<'a>(
    lib: &'a MaterialLibrary,
    search: &str,
    mine_only: bool,
    user: &MaterialLibrary,
) -> Vec<&'a MaterialDef> {
    lib.search(search)
        .into_iter()
        .filter(|m| !mine_only || user.materials.iter().any(|u| u.name == m.name))
        .collect()
}

/// Is `name` one of the user's materials?
pub fn is_mine(user: &MaterialLibrary, name: &str) -> bool {
    user.materials.iter().any(|u| u.name == name)
}

/// Saves a copy of the core material `name` to My Materials; true when
/// there was one to copy.
pub fn save_copy(lib: &MaterialLibrary, name: &str) -> Result<bool, String> {
    match lib.find(name) {
        Some(def) => save_to_user_library(def).map(|_| true),
        None => Ok(false),
    }
}

fn row(ui: &mut egui::Ui, m: &MaterialDef, active: Option<&str>, user: &MaterialLibrary) {
    let mine = is_mine(user, &m.name);
    let response = ui
        .horizontal(|ui| {
            swatch(ui, m.color);
            let mut text = m.name.clone();
            if m.class != MaterialClass::General {
                text.push_str(&format!("  ({})", m.class.name()));
            }
            if mine {
                text.push_str("  \u{2605}");
            }
            ui.selectable_label(active == Some(m.name.as_str()), text)
        })
        .inner;
    if response.clicked() {
        set_active(Some(m.name.clone()));
    }
    if response.double_clicked() {
        open_spec_for(m);
    }
    response.context_menu(|ui| {
        if ui.button("Edit Material...").clicked() {
            open_spec_for(m);
            ui.close_menu();
        }
        if ui.button("Paint with This").clicked() {
            set_active(Some(m.name.clone()));
            set_painter_mode(PainterMode::Paint);
            ui.close_menu();
        }
        if !mine && ui.button("Save to My Materials").clicked() {
            state(|s| {
                s.browser.status = match save_to_user_library(m) {
                    Ok(n) => format!("Saved {n} to My Materials"),
                    Err(e) => e,
                }
            });
            ui.close_menu();
        }
        if mine && ui.button("Delete from My Materials").clicked() {
            state(|s| {
                s.browser.status = match remove_from_user_library(&m.name) {
                    Ok(()) => format!("Removed {} from My Materials", m.name),
                    Err(e) => e,
                }
            });
            ui.close_menu();
        }
    });
}

/// The search field and the tree of `lib`.
pub fn list(ui: &mut egui::Ui, lib: &MaterialLibrary, salt: &str) {
    let user = user_library();
    let mut search = state(|s| s.search.clone());
    let mut mine_only = state(|s| s.browser.mine_only);
    ui.horizontal(|ui| {
        ui.label("Search");
        ui.add(egui::TextEdit::singleline(&mut search).desired_width(150.0));
        ui.checkbox(&mut mine_only, "My Materials");
    });
    state(|s| {
        s.search = search.clone();
        s.browser.mine_only = mine_only;
    });
    let active = active_material();
    let shown = matching(lib, &search, mine_only, &user);
    egui::ScrollArea::vertical()
        .id_salt(("materials_list_scroll", salt))
        .show(ui, |ui| {
            if shown.is_empty() {
                ui.weak("No material matches");
            } else if search.trim().is_empty() {
                let mut by: std::collections::BTreeMap<String, Vec<&MaterialDef>> =
                    std::collections::BTreeMap::new();
                for m in shown {
                    by.entry(
                        m.category
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "Uncategorized".to_string()),
                    )
                    .or_default()
                    .push(m);
                }
                for (cat, mats) in by {
                    egui::CollapsingHeader::new(format!("{cat} ({})", mats.len()))
                        .id_salt(("mat_cat", salt, cat.clone()))
                        .default_open(mine_only)
                        .show(ui, |ui| {
                            for m in mats {
                                row(ui, m, active.as_deref(), &user);
                            }
                        });
                }
            } else {
                for m in shown {
                    row(ui, m, active.as_deref(), &user);
                }
            }
        });
    let status = state(|s| s.browser.status.clone());
    if !status.is_empty() {
        ui.weak(status);
    }
}

/// The Library dock's Materials filter: the list, with New..., the active
/// material and a Paint button.
pub fn panel(ui: &mut egui::Ui, _cx: &mut EditorContext) {
    let lib = library();
    ui.horizontal(|ui| {
        if ui.button("New Material...").clicked() {
            open_new_spec();
        }
        let active = active_material();
        if ui
            .add_enabled(active.is_some(), egui::Button::new("Paint with Active"))
            .clicked()
        {
            set_painter_mode(PainterMode::Paint);
        }
    });
    match active_material().and_then(|n| lib.resolve(&n)) {
        Some(d) => {
            ui.horizontal(|ui| {
                ui.label("Active:");
                swatch(ui, d.color);
                ui.strong(&d.name);
            });
        }
        None => {
            ui.weak("No active material");
        }
    }
    list(ui, &lib, "dock");
}

/// Draws the Library dock's Objects / Materials switch; true when Materials
/// is chosen (the dock then shows [`panel`] instead of the object library).
pub fn filter_switch(ui: &mut egui::Ui) -> bool {
    let mut materials = state(|s| s.browse_materials);
    ui.horizontal(|ui| {
        ui.selectable_value(&mut materials, false, "Objects");
        ui.selectable_value(&mut materials, true, "Materials");
    });
    state(|s| s.browse_materials = materials);
    materials
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_materials::core_library;

    #[test]
    fn search_and_my_materials_filter_the_list() {
        let core = core_library();
        let mut user = MaterialLibrary::default();
        let mut mine = MaterialDef::new("Barn Red", &["Paint", "Custom"], [160, 75, 55]);
        mine.class = MaterialClass::Plastic;
        user.add(mine);
        let all = core.merged_with(&user);
        assert_eq!(matching(&all, "", false, &user).len(), all.materials.len());
        let found = matching(&all, "barn", false, &user);
        assert_eq!(found.len(), 1);
        let only_mine = matching(&all, "", true, &user);
        assert_eq!(only_mine.len(), 1);
        assert_eq!(only_mine[0].name, "Barn Red");
        assert!(is_mine(&user, "Barn Red") && !is_mine(&user, "Drywall"));
        // A search by category finds the whole category.
        assert!(matching(&all, "flooring", false, &user).len() >= 2);
        assert!(matching(&all, "zzzz", false, &user).is_empty());
    }

    #[test]
    fn the_panel_and_the_switch_draw_headlessly() {
        let ctx = egui::Context::default();
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        let mut on = false;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                on = filter_switch(ui);
                if on {
                    panel(ui, &mut cx);
                }
            });
        });
        assert!(!on, "the dock starts on Objects");
        state(|s| s.browse_materials = true);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                on = filter_switch(ui);
                if on {
                    panel(ui, &mut cx);
                }
            });
        });
        assert!(on);
        state(|s| s.browse_materials = false);
    }
}
