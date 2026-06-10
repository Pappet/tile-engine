use crate::coords::ChunkCoord;

/// Deterministic hash for simulation randomness (Bibel §10.9).
///
/// Mixes coord + tile index + tick + reaction ID using finalizer from
/// splitmix64. No global state — parallel-safe and reproducible.
#[inline]
pub fn mix_hash(coord: ChunkCoord, idx: usize, tick: u64, rid: u32) -> u64 {
    let mut h: u64 = 0xcafef00dd15ea5e5;
    h ^= (coord.cx as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15);
    h ^= (coord.cy as i64 as u64).wrapping_mul(0x6c62272e07bb0142);
    h ^= (coord.cz as i64 as u64).wrapping_mul(0x517cc1b727220a95);
    h ^= (idx as u64).wrapping_mul(0xbf58476d1ce4e5b9);
    h ^= tick.wrapping_mul(0x94d049bb133111eb);
    h ^= rid as u64;
    // splitmix64 finalizer
    h ^= h >> 30;
    h = h.wrapping_mul(0xbf58476d1ce4e5b9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94d049bb133111eb);
    h ^= h >> 31;
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mix_hash_deterministic() {
        let coord = ChunkCoord {
            cx: 1,
            cy: 2,
            cz: 0,
        };
        let a = mix_hash(coord, 5, 100, 0);
        let b = mix_hash(coord, 5, 100, 0);
        assert_eq!(a, b);
    }

    #[test]
    fn test_mix_hash_differs_on_tick() {
        let coord = ChunkCoord {
            cx: 0,
            cy: 0,
            cz: 0,
        };
        let a = mix_hash(coord, 0, 1, 0);
        let b = mix_hash(coord, 0, 2, 0);
        assert_ne!(a, b);
    }

    #[test]
    fn test_mix_hash_differs_on_idx() {
        let coord = ChunkCoord {
            cx: 0,
            cy: 0,
            cz: 0,
        };
        let a = mix_hash(coord, 0, 42, 1);
        let b = mix_hash(coord, 1, 42, 1);
        assert_ne!(a, b);
    }
}
