//! Handcrafted demo world for showcasing fluid mechanics.
//!
//! Single-layer scene at z=2 with three basins separated by walls. Z=1 is a
//! full Granit slab acting as the basin floor (prevents vertical drain).
//! All other Z-levels are air.
//!
//! Layout at z=2 (top-down, y up):
//! ```text
//!   y=10  ┃                                       ┃
//!         ┃         ┃         ┃         ┃         ┃
//!   y=-15 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//! ```
//! - Outer walls at x=-30, x=29
//! - Inner walls at x=-11, x=11 (split into 3 basins)
//! - Floor row at y=-15
//!
//! Sources spawned by the app sit above each basin so liquid pools inside.

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

/// Bottom of basins (floor row).
const FLOOR_Y: i32 = -15;
/// Top of side walls.
const WALL_TOP_Y: i32 = 10;
/// Top of inner dividing walls (lower so liquid can spill over when full).
const INNER_WALL_TOP_Y: i32 = 5;

/// Wall x-coordinates.
const OUTER_LEFT_X: i32 = -30;
const INNER_LEFT_X: i32 = -11;
const INNER_RIGHT_X: i32 = 11;
const OUTER_RIGHT_X: i32 = 29;

/// Demo layer (visible).
const DEMO_Z: i32 = 2;
/// Solid floor below demo layer (prevents vertical drain).
const FLOOR_Z: i32 = 1;

fn gen_demo_chunks(
    config: Res<WorldgenConfig>,
    mut commands: Commands,
    mut world: ResMut<tile_core::world::World>,
) {
    let bounds = config.bounds_radius.unwrap_or(2) as i32;

    for cz in -2..=2 {
        for cy in -bounds..bounds {
            for cx in -bounds..bounds {
                let coord = ChunkCoord { cx, cy, cz };
                let mut chunk = ChunkData::new_filled(coord, MAT_AIR);

                for ly in 0..CHUNK_SIZE {
                    for lx in 0..CHUNK_SIZE {
                        let wx = cx * CHUNK_SIZE as i32 + lx as i32;
                        let wy = cy * CHUNK_SIZE as i32 + ly as i32;
                        let idx = ly * CHUNK_SIZE + lx;

                        if cz == FLOOR_Z {
                            chunk.terrain[idx] = GRANIT;
                        } else if cz == DEMO_Z && is_demo_wall(wx, wy) {
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

/// True if (wx, wy) at z=DEMO_Z is wall material.
fn is_demo_wall(wx: i32, wy: i32) -> bool {
    // Floor row spanning all three basins.
    if wy == FLOOR_Y && (OUTER_LEFT_X..=OUTER_RIGHT_X).contains(&wx) {
        return true;
    }
    // Outer walls (taller).
    if (wx == OUTER_LEFT_X || wx == OUTER_RIGHT_X) && (FLOOR_Y..=WALL_TOP_Y).contains(&wy) {
        return true;
    }
    // Inner dividing walls (shorter — liquid spills over when basin full).
    if (wx == INNER_LEFT_X || wx == INNER_RIGHT_X)
        && (FLOOR_Y..=INNER_WALL_TOP_Y).contains(&wy)
    {
        return true;
    }
    false
}

/// Suggested source positions: one per basin, just above the floor.
pub fn demo_source_positions() -> [WorldPos; 3] {
    [
        WorldPos {
            x: -20,
            y: FLOOR_Y + 8,
            z: DEMO_Z,
        }, // left basin (Magma)
        WorldPos {
            x: 0,
            y: FLOOR_Y + 8,
            z: DEMO_Z,
        }, // middle basin (Water)
        WorldPos {
            x: 20,
            y: FLOOR_Y + 8,
            z: DEMO_Z,
        }, // right basin (Oil)
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_floor_row_is_wall() {
        for x in OUTER_LEFT_X..=OUTER_RIGHT_X {
            assert!(is_demo_wall(x, FLOOR_Y), "floor missing at x={x}");
        }
    }

    #[test]
    fn test_basin_interiors_are_air() {
        // Left basin interior
        for x in (OUTER_LEFT_X + 1)..INNER_LEFT_X {
            for y in (FLOOR_Y + 1)..WALL_TOP_Y {
                assert!(!is_demo_wall(x, y), "left basin not air at ({x},{y})");
            }
        }
        // Middle basin interior
        for x in (INNER_LEFT_X + 1)..INNER_RIGHT_X {
            for y in (FLOOR_Y + 1)..WALL_TOP_Y {
                assert!(!is_demo_wall(x, y), "middle basin not air at ({x},{y})");
            }
        }
    }

    #[test]
    fn test_walls_present() {
        assert!(is_demo_wall(OUTER_LEFT_X, 0));
        assert!(is_demo_wall(OUTER_RIGHT_X, 0));
        assert!(is_demo_wall(INNER_LEFT_X, 0));
        assert!(is_demo_wall(INNER_RIGHT_X, 0));
    }

    #[test]
    fn test_source_positions_inside_basins() {
        let positions = demo_source_positions();
        for pos in positions {
            assert!(!is_demo_wall(pos.x, pos.y), "source on wall at {pos:?}");
            assert_eq!(pos.z, DEMO_Z);
        }
    }
}
