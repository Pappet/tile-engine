use bevy_ecs::prelude::*;
use std::collections::HashMap;
use tile_core::chunk::ChunkData;
use tile_core::coords::{CHUNK_AREA, ChunkCoord};
use tile_core::material::{MAT_AIR, MaterialId};

/// Read-only frozen copy of all chunks' liquid state, built once per tick in PreTick.
///
/// Solves the Bevy borrow conflict: `fluid_step_local` mutably iterates ChunkData
/// so it cannot simultaneously read other chunks' components (Bibel §8.2).
#[derive(Resource, Default)]
pub struct LiquidSnapshot {
    amounts: HashMap<ChunkCoord, Box<[u8; CHUNK_AREA]>>,
    terrain: HashMap<ChunkCoord, Box<[MaterialId; CHUNK_AREA]>>,
}

impl LiquidSnapshot {
    pub fn get_amount(&self, coord: ChunkCoord, local_idx: usize) -> u8 {
        self.amounts.get(&coord).map_or(0, |a| a[local_idx])
    }

    pub fn is_passable(&self, coord: ChunkCoord, local_idx: usize) -> bool {
        self.terrain
            .get(&coord)
            .map_or(false, |t| t[local_idx] == MAT_AIR)
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
}

/// PreTick system: freeze liquid_amount_read and terrain from every chunk.
pub fn snapshot_liquid(mut snapshot: ResMut<LiquidSnapshot>, chunks: Query<&ChunkData>) {
    snapshot.amounts.clear();
    snapshot.terrain.clear();
    for chunk in chunks.iter() {
        let mut amounts = Box::new([0u8; CHUNK_AREA]);
        amounts.copy_from_slice(&*chunk.liquid_amount_read);
        snapshot.amounts.insert(chunk.coord, amounts);

        let mut terrain = Box::new([MAT_AIR; CHUNK_AREA]);
        terrain.copy_from_slice(&*chunk.terrain);
        snapshot.terrain.insert(chunk.coord, terrain);
    }
}
