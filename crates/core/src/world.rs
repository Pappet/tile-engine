use std::collections::HashMap;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use serde::{Deserialize, Serialize};

use crate::chunk::ChunkData;
use crate::coords::{ChunkCoord, WorldPos};
use crate::material::MaterialId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tile {
    pub material: MaterialId,
}

#[derive(Resource, Default, Serialize, Deserialize)]
pub struct World {
    pub chunks: HashMap<ChunkCoord, Entity>,
    pub current_tick: u64,
}

#[derive(SystemParam)]
pub struct WorldAccess<'w, 's> {
    pub world: Res<'w, World>,
    pub chunks: Query<'w, 's, &'static ChunkData>,
}

#[derive(SystemParam)]
pub struct WorldAccessMut<'w, 's> {
    pub world: ResMut<'w, World>,
    pub chunks: Query<'w, 's, &'static mut ChunkData>,
    pub commands: Commands<'w, 's>,
}

impl<'w, 's> WorldAccess<'w, 's> {
    pub fn get_chunk(&self, coord: ChunkCoord) -> Option<&ChunkData> {
        let entity = self.world.chunks.get(&coord)?;
        self.chunks.get(*entity).ok()
    }

    pub fn get_tile(&self, pos: WorldPos) -> Option<Tile> {
        let (cc, lp) = pos.split();
        let chunk = self.get_chunk(cc)?;
        Some(Tile {
            material: chunk.terrain[lp.index()],
        })
    }
}

impl<'w, 's> WorldAccessMut<'w, 's> {
    pub fn get_chunk(&self, coord: ChunkCoord) -> Option<&ChunkData> {
        let entity = self.world.chunks.get(&coord)?;
        self.chunks.get(*entity).ok()
    }

    pub fn get_chunk_mut(&mut self, coord: ChunkCoord) -> Option<Mut<'_, ChunkData>> {
        let entity = self.world.chunks.get(&coord)?;
        self.chunks.get_mut(*entity).ok()
    }

    /// Returns the entity of the chunk. If it didn't exist, it is queued for spawning.
    ///
    /// # Caveat (P1.5 Deferred Spawn)
    ///
    /// Due to Bevy's deferred `Commands` execution, if a chunk is newly spawned by this
    /// function, its `ChunkData` component will **not** be accessible via the `chunks` query
    /// until the next system/stage execution!
    ///
    /// **Pattern to use:** Pre-Pass. Systems that generate new chunks (e.g., Worldgen)
    /// should run in an earlier Schedule/Stage (like `PreStartup` or `PreUpdate`) so that
    /// the chunks are fully spawned and queryable by Simulation/Logic systems.
    pub fn ensure_chunk(&mut self, coord: ChunkCoord) -> Entity {
        if let Some(&entity) = self.world.chunks.get(&coord) {
            return entity;
        }
        let chunk_data = ChunkData::new_filled(coord, crate::material::MAT_AIR);
        let entity = self.commands.spawn(chunk_data).id();
        self.world.chunks.insert(coord, entity);
        entity
    }

    /// Sets a tile's material.
    ///
    /// # Caveat
    /// If the chunk does not exist yet, it will be queued for spawning, but the tile
    /// will **NOT** be modified in the same frame due to the deferred spawn caveat.
    /// You must pre-create chunks in a prior stage.
    pub fn set_tile(&mut self, pos: WorldPos, tile: Tile) {
        let (cc, lp) = pos.split();
        let entity = self.ensure_chunk(cc);
        if let Ok(mut chunk) = self.chunks.get_mut(entity) {
            chunk.terrain[lp.index()] = tile.material;
            chunk.dirty = true;
        }
    }
}
