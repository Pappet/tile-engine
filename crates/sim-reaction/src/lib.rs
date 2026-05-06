pub mod resolver;

use std::collections::HashMap;

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use sim_fluid::LiquidFlags;
use tile_core::liquid::{GasId, LiquidId};
use tile_core::material::{ElementId, MaterialFlags, MaterialId};

// ── Primitive ID types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReactionId(pub u32);

/// Placeholder IDs for future sub-systems (P6.4+).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventKind(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TagId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityTypeId(pub u32);

// ── Direction ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    North,
    South,
    East,
    West,
    Up,
    Down,
}

// ── TimeOfDay ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeOfDay {
    Day,
    Night,
}

// ── Trigger (Bibel §10.4) ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TriggerKind {
    Periodic,
    LiquidCollision,
    LiquidTouchesMaterial,
    TemperatureCrossing,
    External,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Trigger {
    Periodic { every_ticks: u32 },
    LiquidCollision,
    LiquidTouchesMaterial,
    TemperatureCrossing { threshold: i16 },
    External(EventId),
}

impl Trigger {
    pub fn kind(&self) -> TriggerKind {
        match self {
            Trigger::Periodic { .. } => TriggerKind::Periodic,
            Trigger::LiquidCollision => TriggerKind::LiquidCollision,
            Trigger::LiquidTouchesMaterial => TriggerKind::LiquidTouchesMaterial,
            Trigger::TemperatureCrossing { .. } => TriggerKind::TemperatureCrossing,
            Trigger::External(_) => TriggerKind::External,
        }
    }
}

// ── Condition (Bibel §10.5) ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Condition {
    // Atomic
    TileMaterialIs(MaterialId),
    TileLiquidIs(LiquidId),
    TileLiquidAmountAtLeast(u8),
    TempBetween(i16, i16),
    LiquidTempBetween(i16, i16),
    PressureAtLeast(u8),
    NeighborMaterialIs { dir: Direction, mat: MaterialId },
    NeighborLiquidIs { dir: Direction, liq: LiquidId },
    AnyNeighborIs(MaterialId),
    MaterialHasFlag(MaterialFlags),
    LiquidHasFlag(LiquidFlags),
    MaterialContainsElement { elem: ElementId, min_fraction: f32 },
    AgeAtLeast { ticks: u32 },
    IsTimeOfDay(TimeOfDay),
    DepthBelow(i32),
    HasTag(TagId),
    IncomingLiquidIs(LiquidId),
    // Boolean
    Not(Box<Condition>),
    AnyOf(Vec<Condition>),
    AllOf(Vec<Condition>),
}

// ── Effect (Bibel §10.6) ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Effect {
    SetTileMaterial(MaterialId),
    SetLiquid {
        kind: LiquidId,
        amount: u8,
        temp: i16,
    },
    AddLiquidAmount(i16),
    AddTemperature(i16),
    SetTemperature(i16),
    SetStain {
        kind: LiquidId,
        amount: u8,
    },
    EmitGas {
        kind: GasId,
        amount: u8,
    },
    SpawnItem {
        item: ItemId,
        count: u8,
    },
    SpawnEntity {
        entity_type: EntityTypeId,
    },
    EmitEvent(EventKind),
    PropagateToNeighbor {
        dir: Direction,
        effect: Box<Effect>,
    },
    AreaEffect {
        radius: u8,
        effect: Box<Effect>,
    },
}

// ── Reaction (Bibel §10.3) ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reaction {
    pub name: String,
    pub trigger: Trigger,
    pub conditions: Vec<Condition>,
    pub effects: Vec<Effect>,

    pub primary_material: Option<MaterialId>,
    pub primary_liquid: Option<LiquidId>,
    pub min_temperature: Option<i16>,
    pub max_temperature: Option<i16>,

    /// 0–65535; 65535 = always fires (u16::MAX).
    pub probability: u16,
    pub cooldown_ticks: u16,
}

// ── ReactionRegistry (Bibel §10.2) ────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct ReactionRegistry {
    reactions: Vec<Reaction>,
    by_trigger: HashMap<TriggerKind, Vec<ReactionId>>,
    by_material: HashMap<MaterialId, Vec<ReactionId>>,
    by_liquid: HashMap<LiquidId, Vec<ReactionId>>,
}

impl ReactionRegistry {
    pub fn add(&mut self, reaction: Reaction) -> ReactionId {
        let id = ReactionId(self.reactions.len() as u32);

        self.by_trigger
            .entry(reaction.trigger.kind())
            .or_default()
            .push(id);

        if let Some(mat) = reaction.primary_material {
            self.by_material.entry(mat).or_default().push(id);
        }

        if let Some(liq) = reaction.primary_liquid {
            self.by_liquid.entry(liq).or_default().push(id);
        }

        self.reactions.push(reaction);
        id
    }

    pub fn get(&self, id: ReactionId) -> Option<&Reaction> {
        self.reactions.get(id.0 as usize)
    }

    pub fn by_trigger(&self, kind: &TriggerKind) -> &[ReactionId] {
        self.by_trigger
            .get(kind)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn by_material(&self, mat: MaterialId) -> &[ReactionId] {
        self.by_material
            .get(&mat)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn by_liquid(&self, liq: LiquidId) -> &[ReactionId] {
        self.by_liquid
            .get(&liq)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn len(&self) -> usize {
        self.reactions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.reactions.is_empty()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn water_ice_reaction() -> Reaction {
        Reaction {
            name: "water_freeze".to_string(),
            trigger: Trigger::Periodic { every_ticks: 4 },
            conditions: vec![
                Condition::TileLiquidIs(LiquidId(1)),
                Condition::TempBetween(i16::MIN, 0),
            ],
            effects: vec![
                Effect::SetTileMaterial(MaterialId(10)), // Ice
                Effect::SetLiquid {
                    kind: LiquidId(0),
                    amount: 0,
                    temp: 0,
                },
            ],
            primary_material: None,
            primary_liquid: Some(LiquidId(1)),
            min_temperature: None,
            max_temperature: Some(0),
            probability: u16::MAX,
            cooldown_ticks: 0,
        }
    }

    fn iron_smelt_reaction() -> Reaction {
        Reaction {
            name: "iron_smelt".to_string(),
            trigger: Trigger::Periodic { every_ticks: 8 },
            conditions: vec![
                Condition::TileMaterialIs(MaterialId(5)), // Iron ore
                Condition::TempBetween(1200, i16::MAX),
            ],
            effects: vec![
                Effect::SetLiquid {
                    kind: LiquidId(2),
                    amount: 200,
                    temp: 1300,
                },
                Effect::SetTileMaterial(MaterialId(1)), // Air
            ],
            primary_material: Some(MaterialId(5)),
            primary_liquid: None,
            min_temperature: Some(1200),
            max_temperature: None,
            probability: u16::MAX,
            cooldown_ticks: 0,
        }
    }

    fn magma_water_collision() -> Reaction {
        Reaction {
            name: "magma_water_collision".to_string(),
            trigger: Trigger::LiquidCollision,
            conditions: vec![
                Condition::TileLiquidIs(LiquidId(2)),     // Magma
                Condition::IncomingLiquidIs(LiquidId(1)), // Water
            ],
            effects: vec![
                Effect::SetLiquid {
                    kind: LiquidId(0),
                    amount: 0,
                    temp: 0,
                },
                Effect::SetTileMaterial(MaterialId(7)), // Basalt
            ],
            primary_material: None,
            primary_liquid: Some(LiquidId(2)),
            min_temperature: None,
            max_temperature: None,
            probability: u16::MAX,
            cooldown_ticks: 0,
        }
    }

    #[test]
    fn test_registry_add_and_lookup() {
        let mut reg = ReactionRegistry::default();
        let id0 = reg.add(water_ice_reaction());
        let id1 = reg.add(iron_smelt_reaction());
        let id2 = reg.add(magma_water_collision());

        assert_eq!(reg.len(), 3);
        assert_eq!(id0, ReactionId(0));
        assert_eq!(id1, ReactionId(1));
        assert_eq!(id2, ReactionId(2));

        assert_eq!(reg.get(id0).unwrap().name, "water_freeze");
        assert_eq!(reg.get(id1).unwrap().name, "iron_smelt");
        assert_eq!(reg.get(id2).unwrap().name, "magma_water_collision");
    }

    #[test]
    fn test_by_trigger_index() {
        let mut reg = ReactionRegistry::default();
        reg.add(water_ice_reaction());
        reg.add(iron_smelt_reaction());
        reg.add(magma_water_collision());

        let periodic = reg.by_trigger(&TriggerKind::Periodic);
        assert_eq!(periodic.len(), 2, "two periodic reactions");
        assert!(periodic.contains(&ReactionId(0)));
        assert!(periodic.contains(&ReactionId(1)));

        let collision = reg.by_trigger(&TriggerKind::LiquidCollision);
        assert_eq!(collision.len(), 1);
        assert!(collision.contains(&ReactionId(2)));

        let external = reg.by_trigger(&TriggerKind::External);
        assert!(external.is_empty());
    }

    #[test]
    fn test_by_material_index() {
        let mut reg = ReactionRegistry::default();
        reg.add(water_ice_reaction()); // no primary_material
        reg.add(iron_smelt_reaction()); // primary_material = MaterialId(5)
        reg.add(magma_water_collision()); // no primary_material

        let iron_reactions = reg.by_material(MaterialId(5));
        assert_eq!(iron_reactions.len(), 1);
        assert_eq!(iron_reactions[0], ReactionId(1));

        let unknown = reg.by_material(MaterialId(99));
        assert!(unknown.is_empty());
    }

    #[test]
    fn test_by_liquid_index() {
        let mut reg = ReactionRegistry::default();
        reg.add(water_ice_reaction()); // primary_liquid = LiquidId(1) — Water
        reg.add(iron_smelt_reaction()); // no primary_liquid
        reg.add(magma_water_collision()); // primary_liquid = LiquidId(2) — Magma

        let water_reactions = reg.by_liquid(LiquidId(1));
        assert_eq!(water_reactions.len(), 1);
        assert_eq!(water_reactions[0], ReactionId(0));

        let magma_reactions = reg.by_liquid(LiquidId(2));
        assert_eq!(magma_reactions.len(), 1);
        assert_eq!(magma_reactions[0], ReactionId(2));
    }

    #[test]
    fn test_reaction_serde_roundtrip() {
        let r = magma_water_collision();
        let bytes = bincode::serialize(&r).unwrap();
        let r2: Reaction = bincode::deserialize(&bytes).unwrap();
        assert_eq!(r2.name, r.name);
        assert!(matches!(r2.trigger, Trigger::LiquidCollision));
        assert_eq!(r2.probability, u16::MAX);
    }

    #[test]
    fn test_condition_boolean_variants_serde() {
        let cond = Condition::AnyOf(vec![
            Condition::TileMaterialIs(MaterialId(1)),
            Condition::Not(Box::new(Condition::TileLiquidIs(LiquidId(3)))),
        ]);
        let bytes = bincode::serialize(&cond).unwrap();
        let cond2: Condition = bincode::deserialize(&bytes).unwrap();
        let Condition::AnyOf(inner) = cond2 else {
            panic!("expected AnyOf");
        };
        assert_eq!(inner.len(), 2);
    }
}
