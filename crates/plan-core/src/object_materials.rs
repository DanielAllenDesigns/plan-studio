//! Per-object material overrides (Material Painter, Adjust Materials): the
//! materials an object has been given in place of its defaults, by the name
//! of a `plan_materials` library material. The names are resolved by the app
//! (plan-core does not depend on the material library), so an override whose
//! material was removed from the library simply stops applying.
//!
//! An object is a plan object id (the id the 3D meshes carry as their
//! `object_id`). A part named `""` stands for the whole object; other parts
//! use the component names of `plan_materials::default_assignments_for`
//! ("Exterior Wall Surface", "Door Panel", ...).

use crate::model::{Id, Project};
use serde::{Deserialize, Serialize};

/// The part name that stands for the whole object.
pub const WHOLE_OBJECT: &str = "";

/// One part of an object and the material it was given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartMaterial {
    pub part: String,
    pub material: String,
}

/// The overrides of one object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectMaterial {
    pub object: Id,
    pub parts: Vec<PartMaterial>,
}

impl Project {
    /// The material `part` of `object` was given; a part without its own
    /// override falls back to the object's whole-object material.
    pub fn object_material(&self, object: Id, part: &str) -> Option<&str> {
        let o = self.object_materials.iter().find(|o| o.object == object)?;
        o.parts
            .iter()
            .find(|p| p.part == part)
            .or_else(|| o.parts.iter().find(|p| p.part == WHOLE_OBJECT))
            .map(|p| p.material.as_str())
    }

    /// All overrides of `object`.
    pub fn object_materials_of(&self, object: Id) -> &[PartMaterial] {
        self.object_materials
            .iter()
            .find(|o| o.object == object)
            .map_or(&[], |o| &o.parts)
    }

    /// Gives `part` of `object` the material called `material`. Returns true
    /// when something changed.
    pub fn set_object_material(&mut self, object: Id, part: &str, material: &str) -> bool {
        let entry = match self
            .object_materials
            .iter()
            .position(|o| o.object == object)
        {
            Some(i) => &mut self.object_materials[i],
            None => {
                self.object_materials.push(ObjectMaterial {
                    object,
                    parts: Vec::new(),
                });
                self.object_materials.last_mut().expect("just pushed")
            }
        };
        match entry.parts.iter_mut().find(|p| p.part == part) {
            Some(p) if p.material == material => false,
            Some(p) => {
                p.material = material.to_string();
                true
            }
            None => {
                entry.parts.push(PartMaterial {
                    part: part.to_string(),
                    material: material.to_string(),
                });
                true
            }
        }
    }

    /// Removes the override of one part (or all of them with `None`); true
    /// when something was removed. An object left with no override is dropped.
    pub fn clear_object_material(&mut self, object: Id, part: Option<&str>) -> bool {
        let Some(i) = self
            .object_materials
            .iter()
            .position(|o| o.object == object)
        else {
            return false;
        };
        let before = self.object_materials[i].parts.len();
        match part {
            Some(p) => self.object_materials[i].parts.retain(|x| x.part != p),
            None => self.object_materials[i].parts.clear(),
        }
        let changed = self.object_materials[i].parts.len() != before;
        if self.object_materials[i].parts.is_empty() {
            self.object_materials.remove(i);
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overrides_set_fall_back_clear_and_round_trip() {
        let mut p = Project::new("t");
        assert_eq!(p.object_material(7, WHOLE_OBJECT), None);
        assert!(p.set_object_material(7, WHOLE_OBJECT, "Brick – Red"));
        assert!(!p.set_object_material(7, WHOLE_OBJECT, "Brick – Red"));
        assert!(p.set_object_material(7, "Casing", "Painted White Trim"));
        assert_eq!(p.object_material(7, "Casing"), Some("Painted White Trim"));
        // A part without its own override falls back to the whole object.
        assert_eq!(p.object_material(7, "Jamb"), Some("Brick – Red"));
        let back = Project::from_json(&p.to_json().unwrap()).unwrap();
        assert_eq!(back.object_materials, p.object_materials);
        assert!(p.clear_object_material(7, Some("Casing")));
        assert_eq!(p.object_material(7, "Casing"), Some("Brick – Red"));
        assert!(p.clear_object_material(7, None));
        assert!(p.object_materials.is_empty());
        assert!(!p.clear_object_material(7, None));
    }
}
