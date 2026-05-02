use bevy_ecs::prelude::*;
use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MaterialId(pub u16);

pub const MAT_AIR: MaterialId = MaterialId(0);

/// Stub element table. Extend as needed; never reuse an ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ElementId(pub u8);

#[allow(dead_code)]
impl ElementId {
    pub const H:  Self = Self(0);
    pub const C:  Self = Self(1);
    pub const O:  Self = Self(2);
    pub const FE: Self = Self(3);
    pub const SI: Self = Self(4);
    pub const S:  Self = Self(5);
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct MaterialFlags: u32 {
        const FLAMMABLE  = 1 << 0;
        const CONDUCTIVE = 1 << 1;
        const MAGNETIC   = 1 << 2;
        const PRECIOUS   = 1 << 3;
        const ORGANIC    = 1 << 4;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    pub name: String,
    /// Elemental composition: (element, mass fraction). Fractions should sum to ~1.0.
    pub composition: SmallVec<[(ElementId, f32); 4]>,
    pub density: f32,
    pub hardness: f32,
    pub melting_point: f32,
    pub erodibility: f32,
    pub flags: MaterialFlags,
    pub is_solid: bool,
    pub is_diggable: bool,
    pub display_color: [u8; 4],
}

#[derive(Debug, Default, Resource, Serialize, Deserialize)]
pub struct MaterialRegistry {
    materials: Vec<Option<Material>>,
}

impl MaterialRegistry {
    pub fn add(&mut self, id: MaterialId, material: Material) {
        let idx = id.0 as usize;
        if self.materials.len() <= idx {
            self.materials.resize_with(idx + 1, || None);
        }
        self.materials[idx] = Some(material);
    }

    pub fn get(&self, id: MaterialId) -> Option<&Material> {
        self.materials.get(id.0 as usize)?.as_ref()
    }
}

// ── Test materials ────────────────────────────────────────────────────────────

pub fn builtin_materials() -> [(MaterialId, Material); 6] {
    [
        (
            MAT_AIR,
            Material {
                name: "Air".into(),
                composition: SmallVec::new(),
                density: 0.0,
                hardness: 0.0,
                melting_point: 0.0,
                erodibility: 0.0,
                flags: MaterialFlags::empty(),
                is_solid: false,
                is_diggable: false,
                display_color: [0, 0, 0, 0],
            },
        ),
        (
            MaterialId(1),
            Material {
                name: "Granit".into(),
                composition: SmallVec::from_slice(&[
                    (ElementId::SI, 0.32),
                    (ElementId::O, 0.46),
                    (ElementId::FE, 0.03),
                ]),
                density: 2.7,
                hardness: 7.0,
                melting_point: 1215.0,
                erodibility: 0.05,
                flags: MaterialFlags::empty(),
                is_solid: true,
                is_diggable: true,
                display_color: [150, 130, 120, 255],
            },
        ),
        (
            MaterialId(2),
            Material {
                name: "Sandstein".into(),
                composition: SmallVec::from_slice(&[
                    (ElementId::SI, 0.40),
                    (ElementId::O, 0.53),
                ]),
                density: 2.3,
                hardness: 4.0,
                melting_point: 1650.0,
                erodibility: 0.35,
                flags: MaterialFlags::empty(),
                is_solid: true,
                is_diggable: true,
                display_color: [210, 180, 120, 255],
            },
        ),
        (
            MaterialId(3),
            Material {
                name: "Sand".into(),
                composition: SmallVec::from_slice(&[
                    (ElementId::SI, 0.40),
                    (ElementId::O, 0.53),
                ]),
                density: 1.6,
                hardness: 2.0,
                melting_point: 1650.0,
                erodibility: 0.80,
                flags: MaterialFlags::empty(),
                is_solid: false,
                is_diggable: true,
                display_color: [230, 210, 150, 255],
            },
        ),
        (
            MaterialId(4),
            Material {
                name: "Eis".into(),
                composition: SmallVec::from_slice(&[
                    (ElementId::H, 0.11),
                    (ElementId::O, 0.89),
                ]),
                density: 0.92,
                hardness: 1.5,
                melting_point: 0.0,
                erodibility: 0.40,
                flags: MaterialFlags::empty(),
                is_solid: true,
                is_diggable: true,
                display_color: [180, 220, 240, 200],
            },
        ),
        (
            MaterialId(5),
            Material {
                name: "Basalt".into(),
                composition: SmallVec::from_slice(&[
                    (ElementId::SI, 0.25),
                    (ElementId::O, 0.45),
                    (ElementId::FE, 0.15),
                ]),
                density: 3.0,
                hardness: 6.0,
                melting_point: 1100.0,
                erodibility: 0.10,
                flags: MaterialFlags::empty(),
                is_solid: true,
                is_diggable: true,
                display_color: [50, 50, 55, 255],
            },
        ),
    ]
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn filled_registry() -> MaterialRegistry {
        let mut reg = MaterialRegistry::default();
        for (id, mat) in builtin_materials() {
            reg.add(id, mat);
        }
        reg
    }

    #[test]
    fn air_constant_is_zero() {
        assert_eq!(MAT_AIR.0, 0);
    }

    #[test]
    fn air_is_not_solid() {
        let reg = filled_registry();
        let air = reg.get(MAT_AIR).unwrap();
        assert!(!air.is_solid);
        assert!(!air.is_diggable);
    }

    #[test]
    fn add_get_roundtrip() {
        let reg = filled_registry();
        for (id, mat) in builtin_materials() {
            let found = reg.get(id).expect("material should exist");
            assert_eq!(found.name, mat.name);
        }
    }

    #[test]
    fn missing_id_returns_none() {
        let reg = filled_registry();
        assert!(reg.get(MaterialId(99)).is_none());
    }

    #[test]
    fn serde_roundtrip() {
        let reg = filled_registry();
        let json = serde_json::to_string(&reg).unwrap();
        let reg2: MaterialRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(reg2.get(MaterialId(1)).unwrap().name, "Granit");
    }
}
