use bevy_ecs::prelude::*;
use tile_core::chunk::ChunkData;
use tile_core::coords::{CHUNK_SIZE, CHUNK_AREA};
use tile_core::material::MAT_AIR;

/// Single-chunk horizontal fluid CA.
///
/// For each tile that contains liquid AND is passable (terrain == AIR):
///   1. Gather the set of passable cardinal neighbors.
///   2. Equalize liquid_amount among self + those neighbors.
///   3. Accumulate deltas, then apply to `liquid_amount_write`.
///
/// Mass-conserving by construction: total transferred out
/// equals total transferred in across all participants.
pub fn fluid_step_local(mut chunks: Query<&mut ChunkData>) {
    for mut chunk in chunks.iter_mut() {
        // Skip entirely if no liquid present in read buffer
        let has_liquid = chunk.liquid_amount_read.iter().any(|&a| a > 0);
        if !has_liquid {
            continue;
        }

        // ── Phase 1: compute deltas (read-only snapshot) ─────────────────
        // Copy read amounts to a local buffer so we can later mutably borrow chunk.
        let mut amounts = [0u8; CHUNK_AREA];
        amounts.copy_from_slice(&*chunk.liquid_amount_read);

        let mut deltas = [0i16; CHUNK_AREA];
        let terrain = &*chunk.terrain;

        for y in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let idx = y * CHUNK_SIZE + x;
                let amount = amounts[idx];
                if amount == 0 || terrain[idx] != MAT_AIR {
                    continue;
                }

                // Collect passable neighbor indices
                let mut nb = [0usize; 4];
                let mut nb_count = 0usize;

                if x > 0 {
                    let ni = idx - 1;
                    if terrain[ni] == MAT_AIR { nb[nb_count] = ni; nb_count += 1; }
                }
                if x + 1 < CHUNK_SIZE {
                    let ni = idx + 1;
                    if terrain[ni] == MAT_AIR { nb[nb_count] = ni; nb_count += 1; }
                }
                if y > 0 {
                    let ni = idx - CHUNK_SIZE;
                    if terrain[ni] == MAT_AIR { nb[nb_count] = ni; nb_count += 1; }
                }
                if y + 1 < CHUNK_SIZE {
                    let ni = idx + CHUNK_SIZE;
                    if terrain[ni] == MAT_AIR { nb[nb_count] = ni; nb_count += 1; }
                }

                if nb_count == 0 {
                    continue;
                }

                // Equalize: transfer from higher to lower.
                // Transfer per neighbor = (our - theirs) / (nb_count + 1), floored.
                let divisor = (nb_count as u16) + 1;
                for i in 0..nb_count {
                    let ni = nb[i];
                    let n_amount = amounts[ni];
                    if amount > n_amount {
                        let diff = (amount as u16) - (n_amount as u16);
                        let transfer = (diff / divisor) as i16;
                        if transfer > 0 {
                            deltas[idx] -= transfer;
                            deltas[ni] += transfer;
                        }
                    }
                }
            }
        }

        // ── Phase 2: apply deltas (write access) ────────────────────────
        chunk.liquid_amount_write.copy_from_slice(&amounts);
        for i in 0..CHUNK_AREA {
            if deltas[i] != 0 {
                let new_val = (chunk.liquid_amount_write[i] as i16) + deltas[i];
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
            ChunkCoord { cx: 0, cy: 0, cz: 0 },
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
    fn test_mass_conservation() {
        let mut app = bevy_app::App::new();

        // Place a blob of water in the center
        let chunk = make_water_chunk(&[
            (16, 16, 200),
            (17, 16, 100),
            (15, 16, 50),
        ]);
        let initial_mass = total_liquid(&chunk);
        let entity = app.world_mut().spawn(chunk).id();

        app.add_systems(bevy_app::Update, (fluid_step_local, swap_buffers_system).chain());

        // Run 20 simulation ticks
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

        app.add_systems(bevy_app::Update, (fluid_step_local, swap_buffers_system).chain());
        app.update();

        let chunk = app.world().get::<ChunkData>(entity).unwrap();
        let center = 16 * CHUNK_SIZE + 16;
        let right  = 16 * CHUNK_SIZE + 17;
        let left   = 16 * CHUNK_SIZE + 15;
        let up     = 15 * CHUNK_SIZE + 16;
        let down   = 17 * CHUNK_SIZE + 16;

        assert!(chunk.liquid_amount_read[center] < 100, "Center should lose some water");
        let neighbor_total = chunk.liquid_amount_read[right] as u32
            + chunk.liquid_amount_read[left] as u32
            + chunk.liquid_amount_read[up] as u32
            + chunk.liquid_amount_read[down] as u32;
        assert!(neighbor_total > 0, "Neighbors should have gained water");
    }

    #[test]
    fn test_determinism() {
        fn run_sim(ticks: usize) -> Vec<u8> {
            let mut app = bevy_app::App::new();
            let chunk = make_water_chunk(&[
                (16, 16, 200),
                (10, 10, 100),
                (20, 5, 150),
            ]);
            let entity = app.world_mut().spawn(chunk).id();
            app.add_systems(bevy_app::Update,
                (fluid_step_local, swap_buffers_system).chain()
            );
            for _ in 0..ticks {
                app.update();
            }
            let chunk = app.world().get::<ChunkData>(entity).unwrap();
            chunk.liquid_amount_read.to_vec()
        }

        let run_a = run_sim(10);
        let run_b = run_sim(10);
        assert_eq!(run_a, run_b, "Same initial state must produce identical results");
    }

    #[test]
    fn test_solid_blocks_flow() {
        let mut app = bevy_app::App::new();

        let mut chunk = make_water_chunk(&[(5, 5, 200)]);
        // Surround the water tile with solid terrain on all 4 sides
        chunk.terrain[5 * CHUNK_SIZE + 4] = tile_core::material::MaterialId(1); // left
        chunk.terrain[5 * CHUNK_SIZE + 6] = tile_core::material::MaterialId(1); // right
        chunk.terrain[4 * CHUNK_SIZE + 5] = tile_core::material::MaterialId(1); // up
        chunk.terrain[6 * CHUNK_SIZE + 5] = tile_core::material::MaterialId(1); // down

        let entity = app.world_mut().spawn(chunk).id();
        app.add_systems(bevy_app::Update, (fluid_step_local, swap_buffers_system).chain());

        for _ in 0..5 {
            app.update();
        }

        let chunk = app.world().get::<ChunkData>(entity).unwrap();
        assert_eq!(chunk.liquid_amount_read[5 * CHUNK_SIZE + 5], 200,
            "Enclosed water must not move");
    }
}
