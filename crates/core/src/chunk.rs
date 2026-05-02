use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::coords::{ChunkCoord, CHUNK_AREA};
use crate::material::MaterialId;

#[derive(Component, Serialize, Deserialize)]
pub struct ChunkData {
    pub coord: ChunkCoord,
    #[serde(with = "chunk_array_serde")]
    pub terrain: Box<[MaterialId; CHUNK_AREA]>,
    pub dirty: bool,
    #[serde(skip)]
    pub active: crate::activity::SystemMask,
    #[serde(skip)]
    pub last_active_tick: u64,
}

mod chunk_array_serde {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(
        data: &Box<[MaterialId; CHUNK_AREA]>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let slice: &[MaterialId] = &**data;
        slice.serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Box<[MaterialId; CHUNK_AREA]>, D::Error> {
        let vec: Vec<MaterialId> = Vec::deserialize(deserializer)?;
        let boxed_slice = vec.into_boxed_slice();
        boxed_slice
            .try_into()
            .map_err(|_| serde::de::Error::custom("invalid chunk size"))
    }
}

impl ChunkData {
    pub fn new_filled(coord: ChunkCoord, material: MaterialId) -> Self {
        // Allocate zeroed/uninitialized memory on the heap and fill it,
        // avoiding a stack overflow that `Box::new([material; CHUNK_AREA])` might cause.
        let terrain = vec![material; CHUNK_AREA].into_boxed_slice();
        let terrain = match terrain.try_into() {
            Ok(b) => b,
            Err(_) => unreachable!(),
        };

        Self {
            coord,
            terrain,
            dirty: false,
            active: crate::activity::SystemMask::empty(),
            last_active_tick: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::MAT_AIR;

    #[test]
    fn chunk_data_size_sanity_check() {
        use std::mem::size_of;
        // ChunkData only holds a ChunkCoord (12 bytes), a Box (8 bytes), and a bool (1 byte) + padding.
        // The heap allocation holds 1024 * 2 bytes = 2048 bytes (2KB).
        assert!(size_of::<ChunkData>() <= 32); // size of the struct itself
        assert_eq!(size_of::<[MaterialId; CHUNK_AREA]>(), 2048);
    }

    #[test]
    fn chunk_data_allocation() {
        let coord = ChunkCoord { cx: 0, cy: 0, cz: 0 };
        let chunk = ChunkData::new_filled(coord, MAT_AIR);
        
        assert_eq!(chunk.coord, coord);
        assert_eq!(chunk.terrain[0], MAT_AIR);
        assert_eq!(chunk.terrain[CHUNK_AREA - 1], MAT_AIR);
        assert!(!chunk.dirty);
    }
}
