use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use tile_core::chunk::ChunkData;
use tile_core::coords::WorldPos;
use tile_core::liquid::{LIQ_NONE, LiquidId};
use tile_core::material::MAT_AIR;

use crate::LiquidFlags;

/// Bibel §9.8 — spawns liquid into a tile each tick (rate units, capped at 255).
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct LiquidSource {
    pub pos: WorldPos,
    pub kind: LiquidId,
    /// Units added per tick.
    pub rate: u8,
    pub temperature: i16,
    /// Back-pressure limit: source stalls if tile pressure ≥ this value.
    pub max_pressure: u8,
}

/// Which liquid kinds a drain accepts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LiquidFilter {
    All,
    ByFlags(LiquidFlags),
}

/// Bibel §9.8 — removes liquid from a tile each tick (rate units, floor 0).
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct LiquidDrain {
    pub pos: WorldPos,
    /// Units removed per tick.
    pub rate: u8,
    pub accepts: LiquidFilter,
}

/// PreTick system — runs before `snapshot_liquid`.
///
/// Sources inject liquid; drains remove it. Both operate directly on
/// `liquid_amount_read` so the snapshot sees the result this tick.
pub fn run_sources_drains(
    sources: Query<&LiquidSource>,
    drains: Query<&LiquidDrain>,
    mut chunks: Query<&mut ChunkData>,
) {
    for source in sources.iter() {
        let (coord, lp) = source.pos.split();
        let idx = lp.index();
        if let Some(mut chunk) = chunks.iter_mut().find(|c| c.coord == coord) {
            if chunk.terrain[idx] != MAT_AIR {
                continue;
            }
            let current = chunk.liquid_amount_read[idx];
            if chunk.pressure_read[idx] >= source.max_pressure && source.max_pressure > 0 {
                continue;
            }
            let new_val = (current as u16 + source.rate as u16).min(255) as u8;
            chunk.liquid_amount_read[idx] = new_val;
            if chunk.liquid_kind[idx] == LIQ_NONE {
                chunk.liquid_kind[idx] = source.kind;
            }
        }
    }

    for drain in drains.iter() {
        let (coord, lp) = drain.pos.split();
        let idx = lp.index();
        if let Some(mut chunk) = chunks.iter_mut().find(|c| c.coord == coord) {
            let kind = chunk.liquid_kind[idx];
            if kind == LIQ_NONE {
                continue;
            }
            let accepted = match &drain.accepts {
                LiquidFilter::All => true,
                LiquidFilter::ByFlags(_) => true, // full filter in P4.8 when registry is wired
            };
            if !accepted {
                continue;
            }
            let current = chunk.liquid_amount_read[idx];
            let new_val = current.saturating_sub(drain.rate);
            chunk.liquid_amount_read[idx] = new_val;
            if new_val == 0 {
                chunk.liquid_kind[idx] = LIQ_NONE;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tile_core::chunk::ChunkData;
    use tile_core::coords::ChunkCoord;

    fn make_app() -> bevy_app::App {
        use bevy_ecs::schedule::IntoSystemConfigs;
        let mut app = bevy_app::App::new();
        app.init_resource::<crate::snapshot::LiquidSnapshot>();
        app.init_resource::<tile_core::activity::WakeRequests>();
        app.add_systems(
            bevy_app::Update,
            (
                run_sources_drains,
                crate::snapshot::snapshot_liquid,
                crate::fluid_ca::fluid_step_local,
                crate::fluid_ca::swap_buffers_system,
            )
                .chain(),
        );
        app
    }

    fn make_air_chunk(coord: ChunkCoord) -> ChunkData {
        ChunkData::new_filled(coord, MAT_AIR)
    }

    fn total_liquid(chunk: &ChunkData) -> u32 {
        chunk.liquid_amount_read.iter().map(|&a| a as u32).sum()
    }

    #[test]
    fn test_source_adds_liquid() {
        let mut app = make_app();
        let coord = ChunkCoord {
            cx: 0,
            cy: 0,
            cz: 0,
        };
        let entity = app.world_mut().spawn(make_air_chunk(coord)).id();
        app.world_mut().spawn(LiquidSource {
            pos: WorldPos { x: 5, y: 5, z: 0 },
            kind: LiquidId(1),
            rate: 2,
            temperature: 20,
            max_pressure: 255,
        });

        for _ in 0..100 {
            app.update();
        }

        let chunk = app.world().get::<ChunkData>(entity).unwrap();
        let total = total_liquid(chunk);
        // rate=2 over 100 ticks = 200 units minimum (may spread but mass conserved)
        assert!(total >= 200, "expected ≥200 units, got {total}");
    }

    #[test]
    fn test_drain_removes_liquid() {
        let mut app = make_app();
        let coord = ChunkCoord {
            cx: 0,
            cy: 0,
            cz: 0,
        };
        let mut chunk = make_air_chunk(coord);
        let idx = 5 * tile_core::coords::CHUNK_SIZE + 5;
        chunk.liquid_kind[idx] = LiquidId(1);
        chunk.liquid_amount_read[idx] = 100;
        let entity = app.world_mut().spawn(chunk).id();

        app.world_mut().spawn(LiquidDrain {
            pos: WorldPos { x: 5, y: 5, z: 0 },
            rate: 5,
            accepts: LiquidFilter::All,
        });

        for _ in 0..10 {
            app.update();
        }

        let chunk = app.world().get::<ChunkData>(entity).unwrap();
        let total = total_liquid(chunk);
        assert!(total < 100, "drain must reduce liquid; got {total}");
    }

    #[test]
    fn test_source_drain_serde_roundtrip() {
        let src = LiquidSource {
            pos: WorldPos { x: 10, y: 20, z: 3 },
            kind: LiquidId(2),
            rate: 5,
            temperature: -10,
            max_pressure: 200,
        };
        let drain = LiquidDrain {
            pos: WorldPos { x: 11, y: 20, z: 3 },
            rate: 3,
            accepts: LiquidFilter::All,
        };
        let src_bytes = bincode::serialize(&src).unwrap();
        let src2: LiquidSource = bincode::deserialize(&src_bytes).unwrap();
        assert_eq!(src2.rate, 5);

        let drain_bytes = bincode::serialize(&drain).unwrap();
        let drain2: LiquidDrain = bincode::deserialize(&drain_bytes).unwrap();
        assert_eq!(drain2.rate, 3);
    }
}
