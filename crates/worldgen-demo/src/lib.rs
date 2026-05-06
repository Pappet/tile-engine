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
use tile_core::liquid::LiquidId;
use tile_core::material::{MAT_AIR, MaterialId};
use worldgen_api::WorldgenConfig;

pub struct DemoWorldgenPlugin;

impl bevy_app::Plugin for DemoWorldgenPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(bevy_app::PreStartup, gen_demo_chunks);
    }
}

const GRANIT: MaterialId = MaterialId(1);

// --- Global Layout ---
const ZONE_STEP: i32 = 32;

// --- Zone 0: The Original Basins (Shrunk) ---
const Z0_LEFT_X: i32 = -15;
const Z0_RIGHT_X: i32 = 15;
const Z0_BOTTOM_Y: i32 = -10;
const Z0_TOP_Y: i32 = 10;
const Z0_INNER_L_X: i32 = -5;
const Z0_INNER_R_X: i32 = 5;

// --- Zone 1: The Staircase ---
const Z1_OFF_X: i32 = ZONE_STEP;
const Z1_OFF_Y: i32 = 0;
const Z1_WIDTH: i32 = 16;
const Z1_HEIGHT: i32 = 12;

// --- Zone 2: The Reaction Chamber ---
const Z2_OFF_X: i32 = 0;
const Z2_OFF_Y: i32 = ZONE_STEP;
const Z2_WIDTH: i32 = 16;
const Z2_HEIGHT: i32 = 16;

// --- Zone 3: The Density Pool ---
const Z3_OFF_X: i32 = -ZONE_STEP;
const Z3_OFF_Y: i32 = 0;
const Z3_WIDTH: i32 = 12;
const Z3_HEIGHT: i32 = 12;

// --- Zone 4: The U-Pipe ---
const Z4_OFF_X: i32 = 0;
const Z4_OFF_Y: i32 = -ZONE_STEP;

// --- Z-Levels ---
const FLOOR_Z: i32 = 0;
const WALL_TOP_Z: i32 = 8;

fn gen_demo_chunks(
    config: Res<WorldgenConfig>,
    mut commands: Commands,
    mut world: ResMut<tile_core::world::World>,
) {
    let bounds = config.bounds_radius.unwrap_or(3) as i32;

    let min_z = 0;
    let max_z = WALL_TOP_Z + 2;

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
    // 1. Zone 0: Original Basins
    if (Z0_LEFT_X..=Z0_RIGHT_X).contains(&wx) && (Z0_BOTTOM_Y..=Z0_TOP_Y).contains(&wy) {
        if cz == FLOOR_Z {
            return true;
        }
        if cz > 5 {
            return false;
        }
        // Outer walls
        if wx == Z0_LEFT_X || wx == Z0_RIGHT_X || wy == Z0_BOTTOM_Y || wy == Z0_TOP_Y {
            return true;
        }
        // Inner walls (spillover at z=4)
        if cz <= 3 && (wx == Z0_INNER_L_X || wx == Z0_INNER_R_X) {
            return true;
        }
    }

    // 2. Zone 1: Staircase
    let x1 = wx - Z1_OFF_X;
    let y1 = wy - Z1_OFF_Y;
    if (0..Z1_WIDTH).contains(&x1) && (-Z1_HEIGHT / 2..Z1_HEIGHT / 2).contains(&y1) {
        if cz == FLOOR_Z {
            return true;
        }
        // Outer walls
        if x1 == 0 || x1 == Z1_WIDTH - 1 || y1 == -Z1_HEIGHT / 2 || y1 == Z1_HEIGHT / 2 - 1 {
            if cz <= 6 {
                return true;
            }
        }
        // Steps: 4 steps of 4x10 each
        let step = x1 / 4; // 0, 1, 2, 3
        if cz <= step {
            return true;
        }
    }

    // 3. Zone 2: Reaction Chamber (Basin with two high inlets)
    let x2 = wx - Z2_OFF_X;
    let y2 = wy - Z2_OFF_Y;
    if (-Z2_WIDTH / 2..Z2_WIDTH / 2).contains(&x2) && (-Z2_HEIGHT / 2..Z2_HEIGHT / 2).contains(&y2)
    {
        if cz == FLOOR_Z {
            return true;
        }
        if cz > 6 {
            return false;
        }
        // Outer walls
        if x2 == -Z2_WIDTH / 2
            || x2 == Z2_WIDTH / 2 - 1
            || y2 == -Z2_HEIGHT / 2
            || y2 == Z2_HEIGHT / 2 - 1
        {
            return true;
        }
    }

    // 4. Zone 3: Density Pool (Deep)
    let x3 = wx - Z3_OFF_X;
    let y3 = wy - Z3_OFF_Y;
    if (-Z3_WIDTH / 2..Z3_WIDTH / 2).contains(&x3) && (-Z3_HEIGHT / 2..Z3_HEIGHT / 2).contains(&y3)
    {
        if cz == FLOOR_Z {
            return true;
        }
        if cz > 8 {
            return false;
        }
        // High walls
        if x3 == -Z3_WIDTH / 2
            || x3 == Z3_WIDTH / 2 - 1
            || y3 == -Z3_HEIGHT / 2
            || y3 == Z3_HEIGHT / 2 - 1
        {
            return true;
        }
    }

    // 5. Zone 4: U-Pipe Pressure Test
    let x4 = wx - Z4_OFF_X;
    let y4 = wy - Z4_OFF_Y;
    if (-9..9).contains(&x4) && (-3..3).contains(&y4) {
        if cz == FLOOR_Z {
            return true;
        }
        if cz > 7 {
            return false;
        }

        // Shafts (Inner 2x2)
        let in_shaft_l = (-7..-5).contains(&x4) && (-1..1).contains(&y4) && cz <= 6;
        let in_shaft_r = (5..7).contains(&x4) && (-1..1).contains(&y4) && cz <= 6;
        // Pipe (Inner 2x2 at cz=1)
        let in_pipe = (-7..7).contains(&x4) && (-1..1).contains(&y4) && cz == 1;

        if in_shaft_l || in_shaft_r || in_pipe {
            return false;
        }

        if cz <= 7 {
            return true;
        }
    }

    false
}

pub struct ShowcaseEntities {
    pub sources: Vec<(WorldPos, LiquidId, u8, i16)>, // pos, kind, rate, temp
    pub drains: Vec<(WorldPos, u8)>,                 // pos, rate
}

pub fn demo_showcase_entities() -> ShowcaseEntities {
    let sources = vec![
        // Zone 0: Original Basins
        (WorldPos { x: -10, y: 0, z: 1 }, LiquidId(2), 5, 1300), // Magma
        (WorldPos { x: 0, y: 0, z: 1 }, LiquidId(1), 5, 20),     // Water
        (WorldPos { x: 10, y: 0, z: 1 }, LiquidId(4), 5, 20),    // Oil
        // Zone 1: Staircase (Source at the top step)
        (
            WorldPos {
                x: Z1_OFF_X + 13,
                y: 0,
                z: 5,
            },
            LiquidId(1),
            8,
            20,
        ),
        // Zone 2: Reaction Chamber (Water and Magma inlets)
        (
            WorldPos {
                x: -4,
                y: Z2_OFF_Y,
                z: 4,
            },
            LiquidId(1),
            5,
            20,
        ),
        (
            WorldPos {
                x: 4,
                y: Z2_OFF_Y,
                z: 4,
            },
            LiquidId(2),
            5,
            1300,
        ),
        // Zone 3: Density Pool (Acid and Oil)
        (
            WorldPos {
                x: Z3_OFF_X,
                y: 0,
                z: 6,
            },
            LiquidId(5),
            4,
            20,
        ), // Acid (Heavy)
        (
            WorldPos {
                x: Z3_OFF_X,
                y: 3,
                z: 6,
            },
            LiquidId(4),
            4,
            20,
        ), // Oil (Light)
        // Zone 4: U-Pipe
        (
            WorldPos {
                x: -6,
                y: Z4_OFF_Y,
                z: 6,
            },
            LiquidId(1),
            10,
            20,
        ),
    ];
    let drains = Vec::new();

    ShowcaseEntities { sources, drains }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zone0_floor_is_solid() {
        for x in Z0_LEFT_X..=Z0_RIGHT_X {
            for y in Z0_BOTTOM_Y..=Z0_TOP_Y {
                assert!(is_demo_solid(x, y, FLOOR_Z), "Floor missing at ({x},{y})");
            }
        }
    }

    #[test]
    fn test_zone0_interiors_are_air() {
        let test_z = FLOOR_Z + 1;
        // Left basin
        assert!(!is_demo_solid(-10, 0, test_z));
        // Middle basin
        assert!(!is_demo_solid(0, 0, test_z));
        // Right basin
        assert!(!is_demo_solid(10, 0, test_z));
    }

    #[test]
    fn test_staircase_steps() {
        // Step 0
        assert!(is_demo_solid(Z1_OFF_X + 2, 0, 0));
        assert!(!is_demo_solid(Z1_OFF_X + 2, 0, 1));
        // Step 3
        assert!(is_demo_solid(Z1_OFF_X + 14, 0, 3));
        assert!(!is_demo_solid(Z1_OFF_X + 14, 0, 4));
    }

    #[test]
    fn test_upipe_shafts() {
        // Left shaft interior (2x2)
        assert!(!is_demo_solid(Z4_OFF_X - 6, Z4_OFF_Y, 2));
        assert!(!is_demo_solid(Z4_OFF_X - 6, Z4_OFF_Y, 5));
        // Left shaft walls
        assert!(is_demo_solid(Z4_OFF_X - 8, Z4_OFF_Y, 3));

        // Connecting pipe interior at cz=1
        assert!(!is_demo_solid(Z4_OFF_X, Z4_OFF_Y, 1));
        // Wall above pipe at cz=2
        assert!(is_demo_solid(Z4_OFF_X, Z4_OFF_Y, 2));
    }

    #[test]
    fn test_showcase_entities_valid() {
        let entities = demo_showcase_entities();
        for (pos, _, _, _) in entities.sources {
            assert!(
                !is_demo_solid(pos.x, pos.y, pos.z),
                "Source at {pos:?} is inside solid!"
            );
        }
    }
}
