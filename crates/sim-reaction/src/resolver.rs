use bevy_ecs::prelude::*;
use tile_core::chunk::ChunkData;
use tile_core::coords::ChunkCoord;
use tile_core::liquid::{LIQ_NONE, LiquidCollisionEvent, LiquidId};
use tile_core::material::MaterialRegistry;
use tile_core::rng::mix_hash;
use tile_core::world::World;

use crate::{Condition, Effect, ReactionId, ReactionRegistry, TriggerKind};
use sim_fluid::LiquidRegistry;

// ── PendingEffects ────────────────────────────────────────────────────────────

struct PendingEffect {
    coord: ChunkCoord,
    idx: usize,
    reaction_id: ReactionId,
    effects: Vec<Effect>,
}

/// Deferred effect buffer — filled in Phase 1, applied in Phase 2.
#[derive(Resource, Default)]
pub struct PendingEffects {
    buffer: Vec<PendingEffect>,
    /// How many periodic reactions fired last tick (set before drain, readable next frame).
    pub reactions_last_tick: usize,
}

impl PendingEffects {
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

// ── Condition evaluator ───────────────────────────────────────────────────────

pub(crate) fn evaluate(
    cond: &Condition,
    chunk: &ChunkData,
    idx: usize,
    material_reg: &MaterialRegistry,
    liquid_reg: &LiquidRegistry,
    incoming: Option<LiquidId>,
) -> bool {
    match cond {
        Condition::TileMaterialIs(mat) => chunk.terrain[idx] == *mat,
        Condition::TileLiquidIs(liq) => chunk.liquid_kind[idx] == *liq,
        Condition::TileLiquidAmountAtLeast(min) => chunk.liquid_amount_read[idx] >= *min,
        Condition::TempBetween(lo, hi) => {
            let t = chunk.temp[idx];
            t >= *lo && t <= *hi
        }
        Condition::LiquidTempBetween(lo, hi) => {
            let t = chunk.liquid_temp_read[idx];
            t >= *lo && t <= *hi
        }
        Condition::PressureAtLeast(min) => chunk.pressure_read[idx] >= *min,
        Condition::MaterialHasFlag(flag) => {
            if let Some(mat) = material_reg.get(chunk.terrain[idx]) {
                mat.flags.contains(*flag)
            } else {
                false
            }
        }
        Condition::LiquidHasFlag(flag) => {
            if let Some(props) = liquid_reg.get(chunk.liquid_kind[idx]) {
                props.flags.contains(*flag)
            } else {
                false
            }
        }
        Condition::MaterialContainsElement { elem, min_fraction } => {
            if let Some(mat) = material_reg.get(chunk.terrain[idx]) {
                mat.composition
                    .iter()
                    .any(|(e, f)| e == elem && *f >= *min_fraction)
            } else {
                false
            }
        }
        Condition::IncomingLiquidIs(liq) => incoming.is_some_and(|i| i == *liq),
        // Boolean
        Condition::Not(inner) => !evaluate(inner, chunk, idx, material_reg, liquid_reg, incoming),
        Condition::AnyOf(conds) => conds
            .iter()
            .any(|c| evaluate(c, chunk, idx, material_reg, liquid_reg, incoming)),
        Condition::AllOf(conds) => conds
            .iter()
            .all(|c| evaluate(c, chunk, idx, material_reg, liquid_reg, incoming)),
        // Not yet implemented
        Condition::NeighborMaterialIs { .. }
        | Condition::NeighborLiquidIs { .. }
        | Condition::AnyNeighborIs(_)
        | Condition::AgeAtLeast { .. }
        | Condition::IsTimeOfDay(_)
        | Condition::DepthBelow(_)
        | Condition::HasTag(_) => false,
    }
}

// ── Effect applier ────────────────────────────────────────────────────────────

pub(crate) fn apply_effects(chunk: &mut ChunkData, idx: usize, effects: &[Effect]) {
    for effect in effects {
        match effect {
            Effect::SetTileMaterial(mat) => {
                chunk.terrain[idx] = *mat;
                chunk.dirty = true;
            }
            Effect::SetLiquid { kind, amount, temp } => {
                chunk.liquid_kind[idx] = *kind;
                chunk.liquid_amount_read[idx] = *amount;
                chunk.liquid_temp_read[idx] = *temp;
                if *amount == 0 {
                    chunk.liquid_kind[idx] = LIQ_NONE;
                }
                chunk.dirty = true;
            }
            Effect::AddLiquidAmount(delta) => {
                let cur = chunk.liquid_amount_read[idx] as i16;
                let new = (cur + delta).clamp(0, 255) as u8;
                chunk.liquid_amount_read[idx] = new;
                if new == 0 {
                    chunk.liquid_kind[idx] = LIQ_NONE;
                }
                chunk.dirty = true;
            }
            Effect::AddTemperature(delta) => {
                chunk.temp[idx] = chunk.temp[idx].saturating_add(*delta);
                chunk.dirty = true;
            }
            Effect::SetTemperature(val) => {
                chunk.temp[idx] = *val;
                chunk.dirty = true;
            }
            // Out-of-scope for P6.2 — ignored
            Effect::SetStain { .. }
            | Effect::EmitGas { .. }
            | Effect::SpawnItem { .. }
            | Effect::SpawnEntity { .. }
            | Effect::EmitEvent(_)
            | Effect::PropagateToNeighbor { .. }
            | Effect::AreaEffect { .. } => {}
        }
    }
}

// ── periodic_reaction_system ──────────────────────────────────────────────────

pub fn periodic_reaction_system(
    world_res: Res<World>,
    registry: Res<ReactionRegistry>,
    mut pending: ResMut<PendingEffects>,
    mut chunks: Query<&mut ChunkData>,
    material_reg: Res<MaterialRegistry>,
    liquid_reg: Res<LiquidRegistry>,
) {
    let tick = world_res.current_tick;
    let periodic_ids = registry.by_trigger(&TriggerKind::Periodic);
    if periodic_ids.is_empty() {
        return;
    }

    // Pre-filter to only reactions that fire this tick, avoiding O(Chunks * TotalReactions).
    let mut active_reactions = Vec::with_capacity(periodic_ids.len());
    for rid in periodic_ids {
        if let Some(reaction) = registry.get(*rid)
            && let crate::Trigger::Periodic { every_ticks } = reaction.trigger
            && (every_ticks == 0 || tick.is_multiple_of(every_ticks as u64))
        {
            active_reactions.push((*rid, reaction));
        }
    }
    if active_reactions.is_empty() {
        return;
    }

    // Phase 1: collect pending effects (immutable chunk reads).
    for chunk in chunks.iter() {
        let coord = chunk.coord;
        for (rid, reaction) in &active_reactions {
            for idx in 0..tile_core::coords::CHUNK_AREA {
                if let Some(min_t) = reaction.min_temperature
                    && chunk.temp[idx] < min_t
                {
                    continue;
                }
                if let Some(max_t) = reaction.max_temperature
                    && chunk.temp[idx] > max_t
                {
                    continue;
                }

                // Probability roll (cheap, do before expensive evaluate).
                if reaction.probability < u16::MAX {
                    let h = mix_hash(coord, idx, tick, rid.0);
                    if (h & 0xFFFF) >= reaction.probability as u64 {
                        continue;
                    }
                }

                // Evaluate conditions (no incoming liquid for Periodic).
                if !reaction
                    .conditions
                    .iter()
                    .all(|c| evaluate(c, chunk, idx, &material_reg, &liquid_reg, None))
                {
                    continue;
                }

                pending.buffer.push(PendingEffect {
                    coord,
                    idx,
                    reaction_id: *rid,
                    effects: reaction.effects.clone(),
                });
            }
        }
    }

    // Phase 2: apply effects — deterministic order (coord, idx, reaction_id).
    pending
        .buffer
        .sort_unstable_by_key(|p| (p.coord.cx, p.coord.cy, p.coord.cz, p.idx, p.reaction_id.0));

    pending.reactions_last_tick = pending.buffer.len();
    let effects: Vec<_> = pending.buffer.drain(..).collect();
    for pe in effects {
        let Some(&entity) = world_res.chunks.get(&pe.coord) else {
            continue;
        };
        if let Ok(mut chunk) = chunks.get_mut(entity) {
            apply_effects(&mut chunk, pe.idx, &pe.effects);
        }
    }
}

// ── liquid_collision_reaction_system ─────────────────────────────────────────

/// Reads `LiquidCollisionEvent`s emitted by the fluid system and fires matching
/// `LiquidCollision`-trigger reactions (Bibel §9.4, §10.4).
///
/// Runs after `liquid_collision_detect` and before `init_fluid_write_buffers`
/// so effects are visible to the flow step this tick.
pub fn liquid_collision_reaction_system(
    mut events: EventReader<LiquidCollisionEvent>,
    registry: Res<ReactionRegistry>,
    world_res: Res<World>,
    mut chunks: Query<&mut ChunkData>,
    material_reg: Res<MaterialRegistry>,
    liquid_reg: Res<LiquidRegistry>,
) {
    let collision_ids = registry.by_trigger(&TriggerKind::LiquidCollision);
    if collision_ids.is_empty() || events.is_empty() {
        events.clear();
        return;
    }

    let tick = world_res.current_tick;

    for event in events.read() {
        let Some(&entity) = world_res.chunks.get(&event.coord) else {
            continue;
        };
        let Ok(mut chunk) = chunks.get_mut(entity) else {
            continue;
        };

        for rid in collision_ids {
            let reaction = match registry.get(*rid) {
                Some(r) => r,
                None => continue,
            };

            // Probability roll (cheap, do before expensive evaluate).
            if reaction.probability < u16::MAX {
                let h = mix_hash(event.coord, event.idx, tick, rid.0);
                if (h & 0xFFFF) >= reaction.probability as u64 {
                    continue;
                }
            }

            if !reaction.conditions.iter().all(|c| {
                evaluate(
                    c,
                    &chunk,
                    event.idx,
                    &material_reg,
                    &liquid_reg,
                    Some(event.incoming),
                )
            }) {
                continue;
            }

            let effects = reaction.effects.clone();
            apply_effects(&mut chunk, event.idx, &effects);
        }
    }
}

// ── ReactionPlugin ────────────────────────────────────────────────────────────

pub struct ReactionPlugin;

impl bevy_app::Plugin for ReactionPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<ReactionRegistry>();
        app.init_resource::<PendingEffects>();
        app.add_systems(bevy_app::Update, periodic_reaction_system);
        // liquid_collision_reaction_system is NOT added here because it needs
        // explicit ordering relative to FluidPlugin systems (after
        // liquid_collision_detect, before swap_buffers_system). Wire it in
        // app/main.rs with .after()/.before() constraints.
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Condition, Effect, Reaction, ReactionRegistry, Trigger};
    use bevy_app::App;
    use tile_core::chunk::ChunkData;
    use tile_core::coords::{CHUNK_SIZE, ChunkCoord};
    use tile_core::liquid::LiquidId;
    use tile_core::material::{MAT_AIR, MaterialId, MaterialRegistry, builtin_materials};
    use tile_core::world::World;

    fn make_app(with_world: bool) -> App {
        let mut app = App::new();
        app.init_resource::<ReactionRegistry>();
        app.init_resource::<PendingEffects>();
        app.init_resource::<LiquidRegistry>();
        let mut mat_reg = MaterialRegistry::default();
        for (id, mat) in builtin_materials() {
            mat_reg.add(id, mat);
        }
        app.insert_resource(mat_reg);
        if with_world {
            app.init_resource::<World>();
        } else {
            // current_tick = 0 by default
            app.init_resource::<World>();
        }
        app.add_systems(bevy_app::Update, periodic_reaction_system);
        app
    }

    fn make_air_chunk(coord: ChunkCoord) -> ChunkData {
        ChunkData::new_filled(coord, MAT_AIR)
    }

    const COORD: ChunkCoord = ChunkCoord {
        cx: 0,
        cy: 0,
        cz: 0,
    };

    // ── Acceptance Test 1: iron ore → magma (Bibel 10.13 §1) ─────────────────

    #[test]
    fn test_iron_ore_smelt() {
        const IRON_ORE: MaterialId = MaterialId(5);
        const MAGMA: LiquidId = LiquidId(2);

        let mut app = make_app(false);

        // Register iron-smelt reaction.
        let reaction = Reaction {
            name: "iron_smelt".to_string(),
            trigger: Trigger::Periodic { every_ticks: 1 },
            conditions: vec![
                Condition::TileMaterialIs(IRON_ORE),
                Condition::TempBetween(1200, i16::MAX),
            ],
            effects: vec![
                Effect::SetLiquid {
                    kind: MAGMA,
                    amount: 200,
                    temp: 1300,
                },
                Effect::SetTileMaterial(MAT_AIR),
            ],
            primary_material: Some(IRON_ORE),
            primary_liquid: None,
            min_temperature: Some(1200),
            max_temperature: None,
            probability: u16::MAX,
            cooldown_ticks: 0,
        };
        app.world_mut()
            .resource_mut::<ReactionRegistry>()
            .add(reaction);

        // Spawn chunk with iron ore at tile 42, temp 1300.
        let mut chunk = make_air_chunk(COORD);
        chunk.terrain[42] = IRON_ORE;
        chunk.temp[42] = 1300;
        let entity = app.world_mut().spawn(chunk).id();
        app.world_mut()
            .resource_mut::<World>()
            .chunks
            .insert(COORD, entity);

        app.update();

        let chunk = app.world().get::<ChunkData>(entity).unwrap();
        assert_eq!(
            chunk.terrain[42], MAT_AIR,
            "iron ore must become air after smelt"
        );
        assert_eq!(chunk.liquid_kind[42], MAGMA, "tile must contain magma");
        assert_eq!(chunk.liquid_amount_read[42], 200);
    }

    // ── Acceptance Test 2: water → ice cycle (Bibel 10.13 §2) ────────────────

    #[test]
    fn test_water_freeze() {
        const WATER: LiquidId = LiquidId(1);
        const ICE: MaterialId = MaterialId(10);

        let mut app = make_app(false);

        let reaction = Reaction {
            name: "water_freeze".to_string(),
            trigger: Trigger::Periodic { every_ticks: 1 },
            conditions: vec![
                Condition::TileLiquidIs(WATER),
                Condition::TileLiquidAmountAtLeast(10),
                Condition::TempBetween(i16::MIN, -1),
            ],
            effects: vec![
                Effect::SetTileMaterial(ICE),
                Effect::SetLiquid {
                    kind: LiquidId(0),
                    amount: 0,
                    temp: 0,
                },
            ],
            primary_material: None,
            primary_liquid: Some(WATER),
            min_temperature: None,
            max_temperature: Some(-1),
            probability: u16::MAX,
            cooldown_ticks: 0,
        };
        app.world_mut()
            .resource_mut::<ReactionRegistry>()
            .add(reaction);

        let mut chunk = make_air_chunk(COORD);
        let idx = 5 * CHUNK_SIZE + 5;
        chunk.liquid_kind[idx] = WATER;
        chunk.liquid_amount_read[idx] = 100;
        chunk.temp[idx] = -10; // below freezing
        let entity = app.world_mut().spawn(chunk).id();
        app.world_mut()
            .resource_mut::<World>()
            .chunks
            .insert(COORD, entity);

        app.update();

        let chunk = app.world().get::<ChunkData>(entity).unwrap();
        assert_eq!(chunk.terrain[idx], ICE, "water must freeze to ice");
        assert_eq!(chunk.liquid_amount_read[idx], 0, "liquid must be consumed");
    }

    // ── Determinism test (Bibel 10.9) ─────────────────────────────────────────

    #[test]
    fn test_determinism_same_tick_same_result() {
        let prob = 32768u16; // 50%

        let mut results_a = Vec::new();
        let mut results_b = Vec::new();

        for tick in 0u64..10 {
            let rid = ReactionId(0);
            let mut fired_a = Vec::new();
            let mut fired_b = Vec::new();

            // Roll twice with same inputs — must agree.
            for idx in 0..tile_core::coords::CHUNK_AREA {
                let h1 = mix_hash(COORD, idx, tick, rid.0);
                let h2 = mix_hash(COORD, idx, tick, rid.0);
                fired_a.push((h1 & 0xFFFF) < prob as u64);
                fired_b.push((h2 & 0xFFFF) < prob as u64);
            }
            results_a.push(fired_a.clone());
            results_b.push(fired_b);
        }

        for i in 0..10 {
            assert_eq!(
                results_a[i], results_b[i],
                "tick {i}: same inputs must produce same result"
            );
        }
    }

    // ── Periodic phase-offset: every_ticks=4 only fires on multiples ──────────

    #[test]
    fn test_periodic_phase_offset() {
        const IRON_ORE: MaterialId = MaterialId(5);
        const IDX: usize = 0;

        let mut app = make_app(false);

        let reaction = Reaction {
            name: "slow_smelt".to_string(),
            trigger: Trigger::Periodic { every_ticks: 4 },
            conditions: vec![Condition::TileMaterialIs(IRON_ORE)],
            effects: vec![Effect::SetTemperature(999)],
            primary_material: Some(IRON_ORE),
            primary_liquid: None,
            min_temperature: None,
            max_temperature: None,
            probability: u16::MAX,
            cooldown_ticks: 0,
        };
        app.world_mut()
            .resource_mut::<ReactionRegistry>()
            .add(reaction);

        let mut chunk = make_air_chunk(COORD);
        chunk.terrain[IDX] = IRON_ORE;
        let entity = app.world_mut().spawn(chunk).id();
        app.world_mut()
            .resource_mut::<World>()
            .chunks
            .insert(COORD, entity);

        // tick=0 → fires (0 % 4 == 0)
        app.update();
        let chunk = app.world().get::<ChunkData>(entity).unwrap();
        assert_eq!(chunk.temp[IDX], 999, "must fire at tick 0");

        // Reset temp, advance to tick=1 manually.
        app.world_mut().get_mut::<ChunkData>(entity).unwrap().temp[IDX] = 0;
        app.world_mut().resource_mut::<World>().current_tick = 1;

        app.update();
        let chunk = app.world().get::<ChunkData>(entity).unwrap();
        assert_eq!(chunk.temp[IDX], 0, "must NOT fire at tick 1");

        // tick=4 → fires again
        app.world_mut().get_mut::<ChunkData>(entity).unwrap().temp[IDX] = 0;
        app.world_mut().resource_mut::<World>().current_tick = 4;

        app.update();
        let chunk = app.world().get::<ChunkData>(entity).unwrap();
        assert_eq!(chunk.temp[IDX], 999, "must fire at tick 4");
    }

    // ── Acceptance Test: Magma + Water → Basalt (Bibel 9.13 §1) ─────────────

    fn make_collision_app() -> App {
        use bevy_ecs::schedule::IntoSystemConfigs;
        use sim_fluid::fluid_ca::{
            init_fluid_write_buffers, liquid_collision_detect, swap_buffers_system,
        };
        use sim_fluid::snapshot::{LiquidSnapshot, snapshot_liquid};
        use tile_core::activity::WakeRequests;
        use tile_core::liquid::LiquidCollisionEvent;

        let mut app = App::new();
        app.add_event::<LiquidCollisionEvent>();
        app.init_resource::<ReactionRegistry>();
        app.init_resource::<PendingEffects>();
        app.init_resource::<LiquidRegistry>();
        app.init_resource::<LiquidSnapshot>();
        app.init_resource::<WakeRequests>();
        app.init_resource::<World>();
        let mut mat_reg = MaterialRegistry::default();
        for (id, mat) in builtin_materials() {
            mat_reg.add(id, mat);
        }
        app.insert_resource(mat_reg);
        app.add_systems(
            bevy_app::Update,
            (
                snapshot_liquid,
                liquid_collision_detect,
                liquid_collision_reaction_system,
                init_fluid_write_buffers,
                swap_buffers_system,
            )
                .chain(),
        );
        app
    }

    #[test]
    fn test_magma_water_collision_makes_basalt() {
        const MAGMA: LiquidId = LiquidId(2);
        const WATER: LiquidId = LiquidId(1);
        const BASALT: MaterialId = MaterialId(7);
        const MAGMA_IDX: usize = 5 * CHUNK_SIZE + 5;
        const WATER_IDX: usize = 5 * CHUNK_SIZE + 6; // adjacent right

        let mut app = make_collision_app();

        // Reaction: Magma tile + incoming Water → remove liquid + set Basalt.
        let reaction = Reaction {
            name: "magma_water_solidify".to_string(),
            trigger: crate::Trigger::LiquidCollision,
            conditions: vec![
                Condition::TileLiquidIs(MAGMA),
                Condition::IncomingLiquidIs(WATER),
            ],
            effects: vec![
                Effect::SetLiquid {
                    kind: LiquidId(0),
                    amount: 0,
                    temp: 0,
                },
                Effect::SetTileMaterial(BASALT),
            ],
            primary_material: None,
            primary_liquid: Some(MAGMA),
            min_temperature: None,
            max_temperature: None,
            probability: u16::MAX,
            cooldown_ticks: 0,
        };
        app.world_mut()
            .resource_mut::<ReactionRegistry>()
            .add(reaction);

        let mut chunk = make_air_chunk(COORD);
        chunk.liquid_kind[MAGMA_IDX] = MAGMA;
        chunk.liquid_amount_read[MAGMA_IDX] = 100;
        chunk.liquid_kind[WATER_IDX] = WATER;
        chunk.liquid_amount_read[WATER_IDX] = 100;
        let entity = app.world_mut().spawn(chunk).id();
        app.world_mut()
            .resource_mut::<World>()
            .chunks
            .insert(COORD, entity);

        app.update();

        let chunk = app.world().get::<ChunkData>(entity).unwrap();
        assert_eq!(
            chunk.terrain[MAGMA_IDX], BASALT,
            "Magma tile must become Basalt on collision with Water"
        );
        assert_eq!(
            chunk.liquid_amount_read[MAGMA_IDX], 0,
            "Magma liquid must be removed"
        );
        assert_eq!(
            chunk.liquid_kind[MAGMA_IDX],
            tile_core::liquid::LIQ_NONE,
            "liquid_kind must be cleared"
        );
    }

    #[test]
    fn test_collision_only_fires_for_matching_incoming() {
        const MAGMA: LiquidId = LiquidId(2);
        const OIL: LiquidId = LiquidId(4);
        const BASALT: MaterialId = MaterialId(7);
        const MAGMA_IDX: usize = 5 * CHUNK_SIZE + 5;
        const OIL_IDX: usize = 5 * CHUNK_SIZE + 6;

        let mut app = make_collision_app();

        // Reaction only fires when incoming is WATER — oil should NOT trigger it.
        let reaction = Reaction {
            name: "magma_water_solidify".to_string(),
            trigger: crate::Trigger::LiquidCollision,
            conditions: vec![
                Condition::TileLiquidIs(MAGMA),
                Condition::IncomingLiquidIs(LiquidId(1)), // Water only
            ],
            effects: vec![Effect::SetTileMaterial(BASALT)],
            primary_material: None,
            primary_liquid: Some(MAGMA),
            min_temperature: None,
            max_temperature: None,
            probability: u16::MAX,
            cooldown_ticks: 0,
        };
        app.world_mut()
            .resource_mut::<ReactionRegistry>()
            .add(reaction);

        let mut chunk = make_air_chunk(COORD);
        chunk.liquid_kind[MAGMA_IDX] = MAGMA;
        chunk.liquid_amount_read[MAGMA_IDX] = 100;
        chunk.liquid_kind[OIL_IDX] = OIL; // Oil, not water
        chunk.liquid_amount_read[OIL_IDX] = 100;
        let entity = app.world_mut().spawn(chunk).id();
        app.world_mut()
            .resource_mut::<World>()
            .chunks
            .insert(COORD, entity);

        app.update();

        let chunk = app.world().get::<ChunkData>(entity).unwrap();
        assert_ne!(
            chunk.terrain[MAGMA_IDX], BASALT,
            "Magma must NOT become Basalt when adjacent to Oil (IncomingLiquidIs(Water) fails)"
        );
    }
}
