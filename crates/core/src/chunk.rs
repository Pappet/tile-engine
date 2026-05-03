use crate::coords::{CHUNK_AREA, ChunkCoord};
use crate::liquid::{LIQ_NONE, LiquidId};
use crate::material::MaterialId;
use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

// ── Default-value helpers for #[serde(skip, default = "...")] ───────────────

fn default_zero_i16_box() -> Box<[i16; CHUNK_AREA]> {
    vec![0i16; CHUNK_AREA]
        .into_boxed_slice()
        .try_into()
        .unwrap()
}

fn default_zero_u8_box() -> Box<[u8; CHUNK_AREA]> {
    vec![0u8; CHUNK_AREA].into_boxed_slice().try_into().unwrap()
}

fn default_liquid_kind_box() -> Box<[LiquidId; CHUNK_AREA]> {
    vec![LIQ_NONE; CHUNK_AREA]
        .into_boxed_slice()
        .try_into()
        .unwrap()
}

// ── Custom serde for Box<[T; CHUNK_AREA]> via Vec round-trip ────────────────

mod box_array_material {
    use super::*;
    use serde::{Deserializer, Serializer};
    pub fn serialize<S: Serializer>(
        data: &[MaterialId; CHUNK_AREA],
        s: S,
    ) -> Result<S::Ok, S::Error> {
        let slice: &[MaterialId] = data;
        serde::Serialize::serialize(slice, s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Box<[MaterialId; CHUNK_AREA]>, D::Error> {
        let v: Vec<MaterialId> = Vec::deserialize(d)?;
        v.into_boxed_slice()
            .try_into()
            .map_err(|_| serde::de::Error::custom("bad chunk len"))
    }
}

mod box_array_i16 {
    use super::*;
    use serde::{Deserializer, Serializer};
    pub fn serialize<S: Serializer>(data: &[i16; CHUNK_AREA], s: S) -> Result<S::Ok, S::Error> {
        let slice: &[i16] = data;
        serde::Serialize::serialize(slice, s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Box<[i16; CHUNK_AREA]>, D::Error> {
        let v: Vec<i16> = Vec::deserialize(d)?;
        v.into_boxed_slice()
            .try_into()
            .map_err(|_| serde::de::Error::custom("bad chunk len"))
    }
}

mod box_array_u8 {
    use super::*;
    use serde::{Deserializer, Serializer};
    pub fn serialize<S: Serializer>(data: &[u8; CHUNK_AREA], s: S) -> Result<S::Ok, S::Error> {
        let slice: &[u8] = data;
        serde::Serialize::serialize(slice, s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Box<[u8; CHUNK_AREA]>, D::Error> {
        let v: Vec<u8> = serde::Deserialize::deserialize(d)?;
        v.into_boxed_slice()
            .try_into()
            .map_err(|_| serde::de::Error::custom("bad chunk len"))
    }
}

mod box_array_liquid {
    use super::*;
    use serde::{Deserializer, Serializer};
    pub fn serialize<S: Serializer>(
        data: &[LiquidId; CHUNK_AREA],
        s: S,
    ) -> Result<S::Ok, S::Error> {
        let slice: &[LiquidId] = data;
        serde::Serialize::serialize(slice, s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Box<[LiquidId; CHUNK_AREA]>, D::Error> {
        let v: Vec<LiquidId> = Vec::deserialize(d)?;
        v.into_boxed_slice()
            .try_into()
            .map_err(|_| serde::de::Error::custom("bad chunk len"))
    }
}

// ── ChunkData ───────────────────────────────────────────────────────────────

/// The data for a single chunk of tiles (32×32 = 1024 tiles).
/// Uses SoA layout with double-buffered fields for simulation.
#[derive(Component, Serialize, Deserialize)]
pub struct ChunkData {
    pub coord: ChunkCoord,

    // ── Authoritative terrain + temperature ─────────────────────────────
    #[serde(with = "box_array_material")]
    pub terrain: Box<[MaterialId; CHUNK_AREA]>,

    #[serde(with = "box_array_i16")]
    pub temp: Box<[i16; CHUNK_AREA]>,

    // ── Fluid fields (read = authoritative, write = derived buffer) ─────
    #[serde(with = "box_array_liquid")]
    pub liquid_kind: Box<[LiquidId; CHUNK_AREA]>,
    #[serde(skip, default = "default_liquid_kind_box")]
    pub liquid_kind_write: Box<[LiquidId; CHUNK_AREA]>,

    #[serde(with = "box_array_u8")]
    pub liquid_amount_read: Box<[u8; CHUNK_AREA]>,
    #[serde(skip, default = "default_zero_u8_box")]
    pub liquid_amount_write: Box<[u8; CHUNK_AREA]>,

    #[serde(with = "box_array_i16")]
    pub liquid_temp_read: Box<[i16; CHUNK_AREA]>,
    #[serde(skip, default = "default_zero_i16_box")]
    pub liquid_temp_write: Box<[i16; CHUNK_AREA]>,

    #[serde(with = "box_array_u8")]
    pub pressure_read: Box<[u8; CHUNK_AREA]>,
    #[serde(skip, default = "default_zero_u8_box")]
    pub pressure_write: Box<[u8; CHUNK_AREA]>,

    // ── Bookkeeping (never serialized) ──────────────────────────────────
    /// Tracks if this chunk needs to be saved.
    #[serde(skip)]
    pub dirty: bool,

    /// Bitmask of active simulation sub-systems in this chunk.
    #[serde(skip)]
    pub active: crate::activity::SystemMask,

    /// The tick this chunk was last marked active.
    #[serde(skip)]
    pub last_active_tick: u64,
}

impl ChunkData {
    /// Create a new chunk filled entirely with `material` and zeroed fluid data.
    pub fn new_filled(coord: ChunkCoord, material: MaterialId) -> Self {
        Self {
            coord,
            terrain: vec![material; CHUNK_AREA]
                .into_boxed_slice()
                .try_into()
                .unwrap(),
            temp: default_zero_i16_box(),
            liquid_kind: default_liquid_kind_box(),
            liquid_kind_write: default_liquid_kind_box(),
            liquid_amount_read: default_zero_u8_box(),
            liquid_amount_write: default_zero_u8_box(),
            liquid_temp_read: default_zero_i16_box(),
            liquid_temp_write: default_zero_i16_box(),
            pressure_read: default_zero_u8_box(),
            pressure_write: default_zero_u8_box(),
            dirty: false,
            active: crate::activity::SystemMask::empty(),
            last_active_tick: 0,
        }
    }

    /// Swap read ↔ write buffers for all double-buffered fluid fields.
    /// Called once per tick in PostTick.
    pub fn swap_buffers(&mut self) {
        std::mem::swap(&mut self.liquid_kind, &mut self.liquid_kind_write);
        std::mem::swap(&mut self.liquid_amount_read, &mut self.liquid_amount_write);
        std::mem::swap(&mut self.liquid_temp_read, &mut self.liquid_temp_write);
        std::mem::swap(&mut self.pressure_read, &mut self.pressure_write);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::MAT_AIR;

    #[test]
    fn chunk_data_size_sanity_check() {
        use std::mem::size_of;
        // Heap allocations per chunk:
        //   terrain:             1024 × 2  = 2048
        //   temp:                1024 × 2  = 2048
        //   liquid_kind:         1024 × 2  = 2048
        //   liquid_amount (×2):  1024 × 1  = 2048
        //   liquid_temp   (×2):  1024 × 2  = 4096
        //   pressure      (×2):  1024 × 1  = 2048
        //   Total                          ≈ 14 KB  (< 20 KB ✔)
        assert!(size_of::<ChunkData>() <= 128);
        assert_eq!(size_of::<[MaterialId; CHUNK_AREA]>(), 2048);
    }

    #[test]
    fn chunk_data_allocation() {
        let coord = ChunkCoord {
            cx: 0,
            cy: 0,
            cz: 0,
        };
        let chunk = ChunkData::new_filled(coord, MAT_AIR);

        assert_eq!(chunk.coord, coord);
        assert_eq!(chunk.terrain[0], MAT_AIR);
        assert_eq!(chunk.terrain[CHUNK_AREA - 1], MAT_AIR);
        assert!(!chunk.dirty);
    }

    #[test]
    fn test_chunk_data_serde_roundtrip() {
        let coord = ChunkCoord {
            cx: 1,
            cy: -1,
            cz: 2,
        };
        let mut chunk = ChunkData::new_filled(coord, MAT_AIR);

        // Set read buffer values
        chunk.liquid_amount_read[0] = 50;
        chunk.liquid_temp_read[5] = 300;
        chunk.pressure_read[10] = 42;

        // Set write buffer values (should be dropped on deser)
        chunk.liquid_amount_write[0] = 99;

        let serialized = bincode::serialize(&chunk).unwrap();
        let deserialized: ChunkData = bincode::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.coord, coord);
        // Read buffers are preserved
        assert_eq!(deserialized.liquid_amount_read[0], 50);
        assert_eq!(deserialized.liquid_temp_read[5], 300);
        assert_eq!(deserialized.pressure_read[10], 42);
        // Write buffers are zeroed (skipped)
        assert_eq!(deserialized.liquid_amount_write[0], 0);
    }

    #[test]
    fn test_swap_buffers() {
        let coord = ChunkCoord {
            cx: 0,
            cy: 0,
            cz: 0,
        };
        let mut chunk = ChunkData::new_filled(coord, MAT_AIR);

        chunk.liquid_amount_write[0] = 100;
        chunk.swap_buffers();
        assert_eq!(chunk.liquid_amount_read[0], 100);
        assert_eq!(chunk.liquid_amount_write[0], 0);
    }
}
