use bevy_ecs::prelude::*;
use tile_core::coords::WorldPos;

/// Defines the execution order of world generation stages.
/// Runs in Bevy's `Startup` or `PreStartup` schedules.
#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum WorldgenSet {
    /// Global or regional maps like height, temperature, rainfall
    Maps,
    /// Declarative placement of rivers, caves, ore veins, ruins
    Structures,
    /// Final realization into ChunkData and Entity spawning
    Chunks,
}

#[derive(Resource)]
pub struct WorldgenConfig {
    pub seed: u64,
    /// Optionally define limits. None = infinite.
    pub bounds_radius: Option<u32>,
}

// ── Stub Resources for Standard-Maps ────────────────────────────────────────

#[derive(Resource, Default)]
pub struct Heightmap {
    pub width: i32,
    pub height: i32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub data: Vec<i32>,
}

impl Heightmap {
    pub fn get_z(&self, x: i32, y: i32) -> i32 {
        let lx = x - self.offset_x;
        let ly = y - self.offset_y;
        if lx < 0 || lx >= self.width || ly < 0 || ly >= self.height {
            return 0; // Default out-of-bounds
        }
        self.data[(ly * self.width + lx) as usize]
    }
}

#[derive(Resource, Default)]
pub struct ClimateMap {
    // Stub
}

#[derive(Resource, Default)]
pub struct RiverGraph {
    // Stub
}

#[derive(Resource, Default)]
pub struct StratigraphyMap {
    // Stub
}

// ── Service API ─────────────────────────────────────────────────────────────

pub trait SurfaceQuery {
    /// Returns the surface Z-level at a given global (X, Y).
    fn get_surface_z(&self, x: i32, y: i32) -> i32;
    
    /// Helper to check if a specific 3D tile is underground.
    fn is_underground(&self, pos: WorldPos) -> bool {
        pos.z < self.get_surface_z(pos.x, pos.y)
    }
}

impl SurfaceQuery for Heightmap {
    fn get_surface_z(&self, x: i32, y: i32) -> i32 {
        self.get_z(x, y)
    }
}

// ── Deterministic Hashing ───────────────────────────────────────────────────

/// SplitMix64-style hash to derive deterministic sub-seeds or random numbers.
pub fn subseed(master: u64, modifier: u64) -> u64 {
    let mut x = master.wrapping_add(modifier).wrapping_add(0x9E3779B97F4A7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subseed_determinism() {
        let master = 42;
        let s1 = subseed(master, 100);
        let s2 = subseed(master, 100);
        let s3 = subseed(master, 101);
        
        assert_eq!(s1, s2, "Same inputs must yield same output");
        assert_ne!(s1, s3, "Different inputs should yield different outputs");
    }
}
