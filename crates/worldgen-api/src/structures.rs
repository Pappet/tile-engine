use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tile_core::coords::{ChunkCoord, WorldPos};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaveStructure {
    pub center: WorldPos,
    pub radius: f32,
    pub length: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OreVein {
    pub start: WorldPos,
    pub end: WorldPos,
    pub thickness: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StructureType {
    Cave(CaveStructure),
    Ore(OreVein),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldStructure {
    pub id: u64,
    pub data: StructureType,
}

/// Stub for a spatial index.
/// Later this could be an rstar R-Tree. For now, it's just empty or simple buckets.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpatialIndex {
    // Simple stub: mapping chunk coord to list of structure IDs
    pub buckets: HashMap<ChunkCoord, Vec<u64>>,
}

#[derive(Resource, Default, Serialize, Deserialize)]
pub struct WorldStructures {
    pub structures: HashMap<u64, WorldStructure>,
    pub index: SpatialIndex,
}

impl WorldStructures {
    pub fn structures_intersecting_chunk(&self, _coord: ChunkCoord) -> Vec<&WorldStructure> {
        // Stub implementation
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_structures_serde() {
        let mut ws = WorldStructures::default();
        let cave = CaveStructure {
            center: WorldPos {
                x: 100,
                y: 100,
                z: 10,
            },
            radius: 5.0,
            length: 50.0,
        };
        ws.structures.insert(
            1,
            WorldStructure {
                id: 1,
                data: StructureType::Cave(cave),
            },
        );
        ws.index.buckets.insert(
            ChunkCoord {
                cx: 0,
                cy: 0,
                cz: 0,
            },
            vec![1],
        );

        let serialized = bincode::serialize(&ws).unwrap();
        let deserialized: WorldStructures = bincode::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.structures.len(), 1);
        assert_eq!(deserialized.index.buckets.len(), 1);
    }
}
