//! Handcrafted demo world for showcasing fluid mechanics.
//!
//! Top-Down scene where X and Y form the horizontal plane, and Z represents height.
//! The player switches Z-layers using Q/E.
//!
//! Layout in the X/Y plane (top-down):
//! ```text
//!   y=15  ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
//!         ┃         ┃         ┃         ┃         ┃
//!         ┃  Basin  ┃  Basin  ┃  Basin  ┃         ┃
//!         ┃  Left   ┃  Middle ┃  Right  ┃         ┃
//!         ┃         ┃         ┃         ┃         ┃
//!   y=-15 ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
//!         x=-25     x=-8      x=8       x=25
//! ```
//! - Floor is completely solid Granit at z=0.
//! - Outer walls go from z=1 up to z=5.
//! - Inner walls divide the area into 3 basins and go from z=1 up to z=3.
//!   (This allows fluids to spill over into adjacent basins when they reach z=4).
//!
//! Sources spawn at z=1 (just above the floor) inside each basin.

use bevy_ecs::prelude::*;
use tile_core::chunk::ChunkData;
use tile_core::coords::{CHUNK_SIZE, ChunkCoord, WorldPos};
use tile_core::material::{MAT_AIR, MaterialId};
use worldgen_api::WorldgenConfig;

pub struct DemoWorldgenPlugin;

impl bevy_app::Plugin for DemoWorldgenPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(bevy_app::PreStartup, gen_demo_chunks);
    }
}

const GRANIT: MaterialId = MaterialId(1);

// --- Top-Down Z-Layer Layout ---
/// Bottom of all basins.
const FLOOR_Z: i32 = 0;
/// Height of the inner dividing walls. Fluids spill over at Z=4.
const INNER_WALL_TOP_Z: i32 = 3;
/// Maximum height of the outer containment walls.
const WALL_TOP_Z: i32 = 5;

// --- X/Y Horizontal Layout ---
const OUTER_LEFT_X: i32 = -25;
const INNER_LEFT_X: i32 = -8;
const INNER_RIGHT_X: i32 = 8;
const OUTER_RIGHT_X: i32 = 25;

const OUTER_BOTTOM_Y: i32 = -15;
const OUTER_TOP_Y: i32 = 15;

fn gen_demo_chunks(
    config: Res<WorldgenConfig>,
    mut commands: Commands,
    mut world: ResMut<tile_core::world::World>,
) {
    let bounds = config.bounds_radius.unwrap_or(2) as i32;

    // We iterate over the necessary Z layers to build our pools.
    // Assuming chunk layers map 1:1 to z-coordinates.
    let min_z = 0;
    let max_z = WALL_TOP_Z + 2; // Generate a little bit of air above the walls

    for cz in min_z..=max_z {
        for cy in -bounds..bounds {
            for cx in -bounds..bounds {
                let coord = ChunkCoord { cx, cy, cz };
                let mut chunk = ChunkData::new_filled(coord, MAT_AIR);

                for ly in 0..CHUNK_SIZE {
                    for lx in 0..CHUNK_SIZE {
                        let wx = cx * CHUNK_SIZE as i32 + lx as i32;
                        let wy = cy * CHUNK_SIZE as i32 + ly as i32;
                        let idx = ly * CHUNK_SIZE + lx;

                        if is_demo_solid(wx, wy, cz) {
                            chunk.terrain[idx] = GRANIT;
                        }
                    }
                }

                let entity = commands.spawn(chunk).id();
                world.chunks.insert(coord, entity);
            }
        }
    }
}

/// Determines if a specific world coordinate should be solid Granit.
fn is_demo_solid(wx: i32, wy: i32, cz: i32) -> bool {
    // Completely outside our demo structure? -> Air
    if !(OUTER_LEFT_X..=OUTER_RIGHT_X).contains(&wx)
        || !(OUTER_BOTTOM_Y..=OUTER_TOP_Y).contains(&wy)
    {
        return false;
    }

    // 1. Solid Floor
    if cz == FLOOR_Z {
        return true;
    }

    // Above the outer walls? -> Air
    if cz > WALL_TOP_Z {
        return false;
    }

    // 2. Outer Walls
    if wx == OUTER_LEFT_X || wx == OUTER_RIGHT_X || wy == OUTER_BOTTOM_Y || wy == OUTER_TOP_Y {
        return true;
    }

    // 3. Inner Walls (lower than outer walls)
    if cz <= INNER_WALL_TOP_Z && (wx == INNER_LEFT_X || wx == INNER_RIGHT_X) {
        return true;
    }

    false
}

/// Suggested source positions: one per basin, sitting just above the solid floor.
pub fn demo_source_positions() -> [WorldPos; 3] {
    [
        WorldPos {
            x: -16,
            y: 0,
            z: FLOOR_Z + 1,
        }, // Left basin (e.g., Lava)
        WorldPos {
            x: -16,
            y: 5,
            z: FLOOR_Z + 1,
        }, // Middle basin (e.g., Water)
        WorldPos {
            x: 16,
            y: 0,
            z: FLOOR_Z + 1,
        }, // Right basin (e.g., Oil)
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_floor_is_solid() {
        for x in OUTER_LEFT_X..=OUTER_RIGHT_X {
            for y in OUTER_BOTTOM_Y..=OUTER_TOP_Y {
                assert!(is_demo_solid(x, y, FLOOR_Z), "Floor missing at ({x},{y})");
            }
        }
    }

    #[test]
    fn test_basin_interiors_are_air() {
        // Test a point inside each basin at a height where it should hold liquid
        let test_z = FLOOR_Z + 1;

        // Left basin
        assert!(!is_demo_solid(-16, 0, test_z), "Left basin is not air");
        // Middle basin
        assert!(!is_demo_solid(0, 0, test_z), "Middle basin is not air");
        // Right basin
        assert!(!is_demo_solid(16, 0, test_z), "Right basin is not air");
    }

    #[test]
    fn test_inner_wall_overflow() {
        // Inner walls should exist at z=3
        assert!(is_demo_solid(INNER_LEFT_X, 0, INNER_WALL_TOP_Z));

        // Inner walls should NOT exist at z=4 (allowing fluids to overflow and mix)
        assert!(!is_demo_solid(INNER_LEFT_X, 0, INNER_WALL_TOP_Z + 1));
    }

    #[test]
    fn test_outer_walls_contain_overflow() {
        // Outer walls must exist above the inner wall height to contain the simulation
        assert!(is_demo_solid(OUTER_LEFT_X, 0, INNER_WALL_TOP_Z + 1));
        assert!(is_demo_solid(OUTER_RIGHT_X, 0, WALL_TOP_Z));
    }

    #[test]
    fn test_source_positions_inside_basins() {
        let positions = demo_source_positions();
        for pos in positions {
            assert!(
                !is_demo_solid(pos.x, pos.y, pos.z),
                "Source spawned inside a wall at {pos:?}"
            );
        }
    }
}
