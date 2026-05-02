pub mod fluid_ca;

use bevy_ecs::prelude::Resource;
use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tile_core::material::MaterialId;
pub use tile_core::liquid::{LiquidId, GasId, LIQ_NONE};

/// Bevy plugin that registers the single-chunk fluid simulation systems.
pub struct FluidPlugin;

impl bevy_app::Plugin for FluidPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        use bevy_ecs::schedule::IntoSystemConfigs;
        app.add_systems(bevy_app::Update, (
            fluid_ca::fluid_step_local,
            fluid_ca::swap_buffers_system,
        ).chain());
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct LiquidFlags: u32 {
        const POTABLE       = 1 << 0;
        const SACRED        = 1 << 1;
        const CONDUCTIVE    = 1 << 2;
        const MAGNETIC      = 1 << 3;
        const STAINS        = 1 << 4;
        const EVAPORATES    = 1 << 5;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidProperties {
    pub name: String,
    pub density: f32,
    pub viscosity: u8,
    pub freeze_point: i16,
    pub boil_point: i16,
    pub freezes_to: MaterialId,
    pub boils_to: GasId,
    pub damages_living: u8,
    pub corrosion: u8,
    pub ignites_flammable: bool,
    pub color: [u8; 4],
    pub emits_light: u8,
    pub flags: LiquidFlags,
}

#[derive(Resource, Default, Serialize, Deserialize)]
pub struct LiquidRegistry {
    liquids: HashMap<LiquidId, LiquidProperties>,
}

impl LiquidRegistry {
    pub fn add(&mut self, id: LiquidId, props: LiquidProperties) {
        self.liquids.insert(id, props);
    }

    pub fn get(&self, id: LiquidId) -> Option<&LiquidProperties> {
        self.liquids.get(&id)
    }
}

pub fn builtin_liquids() -> Vec<(LiquidId, LiquidProperties)> {
    vec![
        (LiquidId(1), LiquidProperties {
            name: "Water".to_string(),
            density: 1.0,
            viscosity: 10,
            freeze_point: 0,
            boil_point: 100,
            freezes_to: MaterialId(4), // Eis is usually MaterialId(4) in our setup
            boils_to: GasId(1), // Steam stub
            damages_living: 0,
            corrosion: 0,
            ignites_flammable: false,
            color: [0, 0, 255, 200],
            emits_light: 0,
            flags: LiquidFlags::POTABLE | LiquidFlags::EVAPORATES,
        }),
        (LiquidId(2), LiquidProperties {
            name: "Magma".to_string(),
            density: 2.5,
            viscosity: 200,
            freeze_point: 1200,
            boil_point: 3000,
            freezes_to: MaterialId(5), // Basalt
            boils_to: GasId(0), // None
            damages_living: 255,
            corrosion: 50,
            ignites_flammable: true,
            color: [255, 50, 0, 255],
            emits_light: 15,
            flags: LiquidFlags::empty(),
        }),
        (LiquidId(3), LiquidProperties {
            name: "Blood".to_string(),
            density: 1.06,
            viscosity: 80,
            freeze_point: -2,
            boil_point: 100,
            freezes_to: MaterialId(0), // None
            boils_to: GasId(1), // Steam
            damages_living: 0,
            corrosion: 0,
            ignites_flammable: false,
            color: [150, 0, 0, 255],
            emits_light: 0,
            flags: LiquidFlags::SACRED | LiquidFlags::STAINS,
        }),
        (LiquidId(4), LiquidProperties {
            name: "Oil".to_string(),
            density: 0.85,
            viscosity: 40,
            freeze_point: -40,
            boil_point: 300,
            freezes_to: MaterialId(0),
            boils_to: GasId(0),
            damages_living: 10,
            corrosion: 0,
            ignites_flammable: true, // actually it IS flammable, so doesn't instantly ignite others unless burning, but for struct properties it fits
            color: [50, 50, 50, 255],
            emits_light: 0,
            flags: LiquidFlags::STAINS,
        }),
        (LiquidId(5), LiquidProperties {
            name: "Acid".to_string(),
            density: 1.20,
            viscosity: 15,
            freeze_point: -50,
            boil_point: 120,
            freezes_to: MaterialId(0),
            boils_to: GasId(0),
            damages_living: 100,
            corrosion: 200,
            ignites_flammable: false,
            color: [0, 255, 0, 200],
            emits_light: 5,
            flags: LiquidFlags::CONDUCTIVE,
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_liq_none_is_zero() {
        assert_eq!(LIQ_NONE.0, 0);
    }

    #[test]
    fn test_add_get_roundtrip() {
        let mut reg = LiquidRegistry::default();
        let props = LiquidProperties {
            name: "TestFluid".to_string(),
            density: 1.0,
            viscosity: 10,
            freeze_point: 0,
            boil_point: 100,
            freezes_to: MaterialId(0),
            boils_to: GasId(0),
            damages_living: 0,
            corrosion: 0,
            ignites_flammable: false,
            color: [0, 0, 0, 0],
            emits_light: 0,
            flags: LiquidFlags::empty(),
        };

        reg.add(LiquidId(42), props.clone());
        let fetched = reg.get(LiquidId(42)).expect("Liquid missing");
        assert_eq!(fetched.name, "TestFluid");
    }

    #[test]
    fn test_serde_roundtrip() {
        let mut reg = LiquidRegistry::default();
        for (id, props) in builtin_liquids() {
            reg.add(id, props);
        }

        let serialized = bincode::serialize(&reg).unwrap();
        let deserialized: LiquidRegistry = bincode::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.liquids.len(), 5);
        assert_eq!(deserialized.get(LiquidId(2)).unwrap().name, "Magma");
    }
}
