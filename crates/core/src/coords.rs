use serde::{Deserialize, Serialize};

pub const CHUNK_SIZE: usize = 32;
pub const CHUNK_AREA: usize = CHUNK_SIZE * CHUNK_SIZE;
pub const CHUNK_SHIFT: i32 = 5;
pub const CHUNK_MASK: i32 = 31;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorldPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkCoord {
    pub cx: i32,
    pub cy: i32,
    pub cz: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocalPos {
    pub lx: u8,
    pub ly: u8,
}

impl WorldPos {
    pub fn split(self) -> (ChunkCoord, LocalPos) {
        let cc = ChunkCoord {
            cx: self.x >> CHUNK_SHIFT,
            cy: self.y >> CHUNK_SHIFT,
            cz: self.z,
        };
        let lp = LocalPos {
            lx: (self.x & CHUNK_MASK) as u8,
            ly: (self.y & CHUNK_MASK) as u8,
        };
        (cc, lp)
    }
}

impl ChunkCoord {
    pub fn world_pos(self, lp: LocalPos) -> WorldPos {
        WorldPos {
            x: (self.cx << CHUNK_SHIFT) | (lp.lx as i32),
            y: (self.cy << CHUNK_SHIFT) | (lp.ly as i32),
            z: self.cz,
        }
    }
}

impl LocalPos {
    pub fn index(self) -> usize {
        (self.ly as usize) * CHUNK_SIZE + (self.lx as usize)
    }

    pub fn from_index(index: usize) -> Self {
        debug_assert!(index < CHUNK_AREA);
        LocalPos {
            lx: (index % CHUNK_SIZE) as u8,
            ly: (index / CHUNK_SIZE) as u8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_worldpos_roundtrip(x in any::<i32>(), y in any::<i32>(), z in any::<i32>()) {
            let pos = WorldPos { x, y, z };
            let (cc, lp) = pos.split();
            let roundtrip = cc.world_pos(lp);
            assert_eq!(pos, roundtrip);
            assert!(lp.lx < CHUNK_SIZE as u8);
            assert!(lp.ly < CHUNK_SIZE as u8);
        }

        #[test]
        fn test_localpos_index_roundtrip(lx in 0..CHUNK_SIZE as u8, ly in 0..CHUNK_SIZE as u8) {
            let lp = LocalPos { lx, ly };
            let index = lp.index();
            assert!(index < CHUNK_AREA);
            let roundtrip = LocalPos::from_index(index);
            assert_eq!(lp, roundtrip);
        }
    }

    #[test]
    fn test_split_edge_cases() {
        // Exactly zero
        let pos = WorldPos { x: 0, y: 0, z: 0 };
        let (cc, lp) = pos.split();
        assert_eq!(cc.cx, 0);
        assert_eq!(cc.cy, 0);
        assert_eq!(lp.lx, 0);
        assert_eq!(lp.ly, 0);

        // Just below zero
        let pos = WorldPos { x: -1, y: -1, z: 0 };
        let (cc, lp) = pos.split();
        assert_eq!(cc.cx, -1);
        assert_eq!(cc.cy, -1);
        assert_eq!(lp.lx, 31);
        assert_eq!(lp.ly, 31);

        // Just below chunk size
        let pos = WorldPos { x: 31, y: 31, z: 0 };
        let (cc, lp) = pos.split();
        assert_eq!(cc.cx, 0);
        assert_eq!(cc.cy, 0);
        assert_eq!(lp.lx, 31);
        assert_eq!(lp.ly, 31);

        // Exactly chunk size
        let pos = WorldPos { x: 32, y: 32, z: 0 };
        let (cc, lp) = pos.split();
        assert_eq!(cc.cx, 1);
        assert_eq!(cc.cy, 1);
        assert_eq!(lp.lx, 0);
        assert_eq!(lp.ly, 0);

        // Exactly negative chunk size
        let pos = WorldPos { x: -32, y: -32, z: 0 };
        let (cc, lp) = pos.split();
        assert_eq!(cc.cx, -1);
        assert_eq!(cc.cy, -1);
        assert_eq!(lp.lx, 0);
        assert_eq!(lp.ly, 0);

        // Just below negative chunk size
        let pos = WorldPos { x: -33, y: -33, z: 0 };
        let (cc, lp) = pos.split();
        assert_eq!(cc.cx, -2);
        assert_eq!(cc.cy, -2);
        assert_eq!(lp.lx, 31);
        assert_eq!(lp.ly, 31);
    }

    #[test]
    fn test_split_extreme_values() {
        // Max values
        let pos = WorldPos { x: i32::MAX, y: i32::MAX, z: i32::MAX };
        let (cc, lp) = pos.split();
        assert_eq!(cc.cx, i32::MAX >> CHUNK_SHIFT);
        assert_eq!(cc.cy, i32::MAX >> CHUNK_SHIFT);
        assert_eq!(cc.cz, i32::MAX);
        assert_eq!(lp.lx, (i32::MAX & CHUNK_MASK) as u8);
        assert_eq!(lp.ly, (i32::MAX & CHUNK_MASK) as u8);

        // Min values
        let pos = WorldPos { x: i32::MIN, y: i32::MIN, z: i32::MIN };
        let (cc, lp) = pos.split();
        assert_eq!(cc.cx, i32::MIN >> CHUNK_SHIFT);
        assert_eq!(cc.cy, i32::MIN >> CHUNK_SHIFT);
        assert_eq!(cc.cz, i32::MIN);
        assert_eq!(lp.lx, (i32::MIN & CHUNK_MASK) as u8);
        assert_eq!(lp.ly, (i32::MIN & CHUNK_MASK) as u8);
    }
}
