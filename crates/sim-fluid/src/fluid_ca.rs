use bevy_ecs::prelude::*;
use tile_core::chunk::ChunkData;
use tile_core::coords::{CHUNK_AREA, CHUNK_SIZE};
use tile_core::material::MAT_AIR;

/// Bibel §8.3 — Symmetrische Bilanzgleichung.
///
/// Both sides of a chunk boundary call this with the same arguments
/// and only modify their own write buffer. Mass conservation is automatic.
///
/// `viscosity`: 0 = max flow, 255 = no flow.
/// For P4.3 we default to 0 (wired to LiquidRegistry in P4.8).
#[inline]
pub fn flow_between(here: u8, there: u8, viscosity: u8) -> i16 {
    if here <= there + 1 {
        return 0;
    }
    let raw = (here as i16 - there as i16) / 4;
    raw * (256 - viscosity as i16) / 256
}

/// Single-chunk horizontal fluid CA using bilateral `flow_between`.
///
/// Each tile only processes its RIGHT and DOWN neighbor to avoid
/// double-counting. The left/up neighbor pair is handled when
/// *that* neighbor processes its right/down.
///
/// This matches Bibel §8.3 / §8.5 principle 2 and prepares for
/// cross-chunk symmetry in P4.4.
pub fn fluid_step_local(mut chunks: Query<&mut ChunkData>) {
    for mut chunk in chunks.iter_mut() {
        // Skip entirely if no liquid present in read buffer
        let has_liquid = chunk.liquid_amount_read.iter().any(|&a| a > 0);
        if !has_liquid {
            continue;
        }

        // ── Phase 1: snapshot + compute deltas ──────────────────────────
        let mut amounts = [0u8; CHUNK_AREA];
        amounts.copy_from_slice(&*chunk.liquid_amount_read);

        let mut deltas = [0i16; CHUNK_AREA];
        let terrain = &*chunk.terrain;

        // Default viscosity = 0 (max flow). Will be per-liquid in P4.8.
        let viscosity: u8 = 0;

        for y in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let idx = y * CHUNK_SIZE + x;

                // Skip solid tiles
                if terrain[idx] != MAT_AIR {
                    continue;
                }

                let here = amounts[idx];

                // ── Right neighbor ───────────────────────────────────────
                if x + 1 < CHUNK_SIZE {
                    let ni = idx + 1;
                    if terrain[ni] == MAT_AIR {
                        let there = amounts[ni];
                        let flow = flow_between(here, there, viscosity);
                        if flow > 0 {
                            deltas[idx] -= flow;
                            deltas[ni] += flow;
                        } else {
                            // Check reverse direction (there → here)
                            let rev = flow_between(there, here, viscosity);
                            if rev > 0 {
                                deltas[ni] -= rev;
                                deltas[idx] += rev;
                            }
                        }
                    }
                }

                // ── Down neighbor ────────────────────────────────────────
                if y + 1 < CHUNK_SIZE {
                    let ni = idx + CHUNK_SIZE;
                    if terrain[ni] == MAT_AIR {
                        let there = amounts[ni];
                        let flow = flow_between(here, there, viscosity);
                        if flow > 0 {
                            deltas[idx] -= flow;
                            deltas[ni] += flow;
                        } else {
                            let rev = flow_between(there, here, viscosity);
                            if rev > 0 {
                                deltas[ni] -= rev;
                                deltas[idx] += rev;
                            }
                        }
                    }
                }
            }
        }

        // ── Phase 2: apply deltas ───────────────────────────────────────
        chunk.liquid_amount_write.copy_from_slice(&amounts);
        for (i, &delta) in deltas.iter().enumerate() {
            if delta != 0 {
                let new_val = (chunk.liquid_amount_write[i] as i16) + delta;
                chunk.liquid_amount_write[i] = new_val.clamp(0, 255) as u8;
            }
        }
    }
}

/// Swap read/write buffers on all chunks. Called in PostTick.
pub fn swap_buffers_system(mut chunks: Query<&mut ChunkData>) {
    for mut chunk in chunks.iter_mut() {
        chunk.swap_buffers();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::IntoSystemConfigs;
    use tile_core::coords::ChunkCoord;

    /// Helper: sum all liquid_amount in the read buffer.
    fn total_liquid(chunk: &ChunkData) -> u32 {
        chunk.liquid_amount_read.iter().map(|&a| a as u32).sum()
    }

    /// Helper: create a flat air chunk with some water placed.
    fn make_water_chunk(water_tiles: &[(usize, usize, u8)]) -> ChunkData {
        let mut chunk = ChunkData::new_filled(
            ChunkCoord {
                cx: 0,
                cy: 0,
                cz: 0,
            },
            MAT_AIR,
        );
        for &(x, y, amount) in water_tiles {
            let idx = y * CHUNK_SIZE + x;
            chunk.liquid_kind[idx] = tile_core::liquid::LiquidId(1); // Water
            chunk.liquid_amount_read[idx] = amount;
        }
        chunk
    }

    #[test]
    fn test_flow_between_basic() {
        // Large difference → flow
        assert!(flow_between(100, 0, 0) > 0);
        // Equal → no flow
        assert_eq!(flow_between(50, 50, 0), 0);
        // Difference of 1 → no flow (threshold: here <= there+1)
        assert_eq!(flow_between(51, 50, 0), 0);
        // Difference of 2 → no flow (raw = 2/4 = 0)
        assert_eq!(flow_between(52, 50, 0), 0);
        // High viscosity → reduced flow
        let full = flow_between(100, 0, 0);
        let visc = flow_between(100, 0, 200);
        assert!(visc < full);
        // Max viscosity → no flow
        assert_eq!(flow_between(100, 0, 255), 0);
    }

    #[test]
    fn test_mass_conservation() {
        let mut app = bevy_app::App::new();

        let chunk = make_water_chunk(&[(16, 16, 200), (17, 16, 100), (15, 16, 50)]);
        let initial_mass = total_liquid(&chunk);
        let entity = app.world_mut().spawn(chunk).id();

        app.add_systems(
            bevy_app::Update,
            (fluid_step_local, swap_buffers_system).chain(),
        );

        for _ in 0..20 {
            app.update();
        }

        let chunk = app.world().get::<ChunkData>(entity).unwrap();
        let final_mass = total_liquid(chunk);
        assert_eq!(initial_mass, final_mass, "Mass must be conserved exactly");
    }

    #[test]
    fn test_water_spreads() {
        let mut app = bevy_app::App::new();

        let chunk = make_water_chunk(&[(16, 16, 100)]);
        let entity = app.world_mut().spawn(chunk).id();

        app.add_systems(
            bevy_app::Update,
            (fluid_step_local, swap_buffers_system).chain(),
        );
        app.update();

        let chunk = app.world().get::<ChunkData>(entity).unwrap();
        let center = 16 * CHUNK_SIZE + 16;
        let right = 16 * CHUNK_SIZE + 17;
        let down = 17 * CHUNK_SIZE + 16;

        assert!(
            chunk.liquid_amount_read[center] < 100,
            "Center should lose some water"
        );
        // At least right and down neighbors should gain water
        assert!(
            chunk.liquid_amount_read[right] > 0 || chunk.liquid_amount_read[down] > 0,
            "At least one neighbor should have gained water"
        );
    }

    #[test]
    fn test_determinism() {
        fn run_sim(ticks: usize) -> Vec<u8> {
            let mut app = bevy_app::App::new();
            let chunk = make_water_chunk(&[(16, 16, 200), (10, 10, 100), (20, 5, 150)]);
            let entity = app.world_mut().spawn(chunk).id();
            app.add_systems(
                bevy_app::Update,
                (fluid_step_local, swap_buffers_system).chain(),
            );
            for _ in 0..ticks {
                app.update();
            }
            let chunk = app.world().get::<ChunkData>(entity).unwrap();
            chunk.liquid_amount_read.to_vec()
        }

        let run_a = run_sim(10);
        let run_b = run_sim(10);
        assert_eq!(
            run_a, run_b,
            "Same initial state must produce identical results"
        );
    }

    #[test]
    fn test_solid_blocks_flow() {
        let mut app = bevy_app::App::new();

        let mut chunk = make_water_chunk(&[(5, 5, 200)]);
        chunk.terrain[5 * CHUNK_SIZE + 4] = tile_core::material::MaterialId(1);
        chunk.terrain[5 * CHUNK_SIZE + 6] = tile_core::material::MaterialId(1);
        chunk.terrain[4 * CHUNK_SIZE + 5] = tile_core::material::MaterialId(1);
        chunk.terrain[6 * CHUNK_SIZE + 5] = tile_core::material::MaterialId(1);

        let entity = app.world_mut().spawn(chunk).id();
        app.add_systems(
            bevy_app::Update,
            (fluid_step_local, swap_buffers_system).chain(),
        );

        for _ in 0..5 {
            app.update();
        }

        let chunk = app.world().get::<ChunkData>(entity).unwrap();
        assert_eq!(
            chunk.liquid_amount_read[5 * CHUNK_SIZE + 5],
            200,
            "Enclosed water must not move"
        );
    }
}
