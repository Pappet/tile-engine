use bevy_ecs::prelude::*;
use std::collections::{HashMap, HashSet};
use tile_core::chunk::ChunkData;
use tile_core::coords::{CHUNK_AREA, ChunkCoord};
use tile_core::liquid::{LIQ_NONE, LiquidId};
use tile_core::material::{MAT_AIR, MaterialId};

/// Read-only frozen copy of all chunks' liquid state, built once per tick in PreTick.
///
/// Solves the Bevy borrow conflict: `fluid_step_local` mutably iterates ChunkData
/// so it cannot simultaneously read other chunks' components (Bibel §8.2).
#[derive(Resource, Default)]
pub struct LiquidSnapshot {
    amounts: HashMap<ChunkCoord, Box<[u8; CHUNK_AREA]>>,
    kinds: HashMap<ChunkCoord, Box<[LiquidId; CHUNK_AREA]>>,
    terrain: HashMap<ChunkCoord, Box<[MaterialId; CHUNK_AREA]>>,
    pressures: HashMap<ChunkCoord, Box<[u8; CHUNK_AREA]>>,
    has_liquid: HashSet<ChunkCoord>,
}

impl LiquidSnapshot {
    pub fn get_amount(&self, coord: ChunkCoord, local_idx: usize) -> u8 {
        self.amounts.get(&coord).map_or(0, |a| a[local_idx])
    }

    pub fn get_kind(&self, coord: ChunkCoord, local_idx: usize) -> LiquidId {
        self.kinds.get(&coord).map_or(LIQ_NONE, |k| k[local_idx])
    }

    pub fn get_pressure(&self, coord: ChunkCoord, local_idx: usize) -> u8 {
        self.pressures.get(&coord).map_or(0, |p| p[local_idx])
    }

    pub fn is_passable(&self, coord: ChunkCoord, local_idx: usize) -> bool {
        self.terrain
            .get(&coord)
            .is_some_and(|t| t[local_idx] == MAT_AIR)
    }

    /// True if chunk exists in snapshot and has any non-zero liquid amount.
    pub fn chunk_has_liquid(&self, coord: ChunkCoord) -> bool {
        self.has_liquid.contains(&coord)
    }

    /// True if any of the 4 horizontal neighbor chunks exist in the snapshot.
    pub fn has_any_neighbor(&self, coord: ChunkCoord) -> bool {
        [
            ChunkCoord {
                cx: coord.cx + 1,
                ..coord
            },
            ChunkCoord {
                cx: coord.cx - 1,
                ..coord
            },
            ChunkCoord {
                cy: coord.cy + 1,
                ..coord
            },
            ChunkCoord {
                cy: coord.cy - 1,
                ..coord
            },
        ]
        .iter()
        .any(|nb| self.amounts.contains_key(nb))
    }

    /// True if any of the 4 horizontal neighbor chunks have non-zero liquid.
    pub fn has_any_neighbor_with_liquid(&self, coord: ChunkCoord) -> bool {
        [
            ChunkCoord {
                cx: coord.cx + 1,
                ..coord
            },
            ChunkCoord {
                cx: coord.cx - 1,
                ..coord
            },
            ChunkCoord {
                cy: coord.cy + 1,
                ..coord
            },
            ChunkCoord {
                cy: coord.cy - 1,
                ..coord
            },
        ]
        .iter()
        .any(|nb| self.chunk_has_liquid(*nb))
    }

    /// True if chunk above (cz+1) or below (cz-1) exists in the snapshot.
    pub fn has_vertical_neighbor(&self, coord: ChunkCoord) -> bool {
        let above = ChunkCoord {
            cz: coord.cz + 1,
            ..coord
        };
        let below = ChunkCoord {
            cz: coord.cz - 1,
            ..coord
        };
        self.amounts.contains_key(&above) || self.amounts.contains_key(&below)
    }
}

/// PreTick system: freeze liquid_amount_read, liquid_kind, and terrain from every chunk.
///
/// Uses entry-based insertion to reuse existing heap allocations across ticks,
/// avoiding ~4 Box alloc/dealloc per chunk per tick.
pub fn snapshot_liquid(mut snapshot: ResMut<LiquidSnapshot>, chunks: Query<&ChunkData>) {
    let mut seen = std::collections::HashSet::with_capacity(chunks.iter().len());

    for chunk in chunks.iter() {
        seen.insert(chunk.coord);

        snapshot
            .amounts
            .entry(chunk.coord)
            .or_insert_with(|| Box::new([0u8; CHUNK_AREA]))
            .copy_from_slice(&*chunk.liquid_amount_read);

        snapshot
            .kinds
            .entry(chunk.coord)
            .or_insert_with(|| Box::new([LIQ_NONE; CHUNK_AREA]))
            .copy_from_slice(&*chunk.liquid_kind);

        snapshot
            .terrain
            .entry(chunk.coord)
            .or_insert_with(|| Box::new([MAT_AIR; CHUNK_AREA]))
            .copy_from_slice(&*chunk.terrain);

        snapshot
            .pressures
            .entry(chunk.coord)
            .or_insert_with(|| Box::new([0u8; CHUNK_AREA]))
            .copy_from_slice(&*chunk.pressure_read);

        if chunk.liquid_amount_read.iter().any(|&a| a > 0) {
            snapshot.has_liquid.insert(chunk.coord);
        } else {
            snapshot.has_liquid.remove(&chunk.coord);
        }
    }

    // Remove entries for chunks that no longer exist.
    snapshot.amounts.retain(|k, _| seen.contains(k));
    snapshot.kinds.retain(|k, _| seen.contains(k));
    snapshot.terrain.retain(|k, _| seen.contains(k));
    snapshot.pressures.retain(|k, _| seen.contains(k));
    snapshot.has_liquid.retain(|k| seen.contains(k));
}
