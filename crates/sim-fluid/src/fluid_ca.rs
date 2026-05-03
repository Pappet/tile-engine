use bevy_ecs::prelude::*;
use tile_core::activity::WakeRequests;
use tile_core::chunk::ChunkData;
use tile_core::coords::{CHUNK_AREA, CHUNK_SIZE, ChunkCoord};
use tile_core::material::MAT_AIR;

use crate::snapshot::LiquidSnapshot;

/// Bibel §8.3 — symmetric bilateral formula.
/// Both sides of a chunk boundary call this with identical args and write only
/// their own buffer. Mass conservation is automatic.
/// viscosity: 0 = max flow, 255 = no flow.
#[inline]
pub fn flow_between(here: u8, there: u8, viscosity: u8) -> i16 {
    if here <= there + 1 {
        return 0;
    }
    let raw = (here as i16 - there as i16) / 4;
    raw * (256 - viscosity as i16) / 256
}

/// Horizontal fluid CA — intra-chunk + cross-chunk borders.
///
/// Intra-chunk: right+down only (avoids double-counting tile pairs).
/// Cross-chunk: all 4 edges; each chunk writes only its own buffer.
/// Both sides read from `LiquidSnapshot` (Bibel §8.3/§8.5).
pub fn fluid_step_local(
    mut chunks: Query<&mut ChunkData>,
    snapshot: Res<LiquidSnapshot>,
    mut wake: ResMut<WakeRequests>,
) {
    for mut chunk in chunks.iter_mut() {
        let coord = chunk.coord;
        let has_liquid = chunk.liquid_amount_read.iter().any(|&a| a > 0);
        if !has_liquid && !snapshot.has_any_neighbor(coord) {
            continue;
        }

        let mut amounts = [0u8; CHUNK_AREA];
        amounts.copy_from_slice(&*chunk.liquid_amount_read);
        let mut deltas = [0i16; CHUNK_AREA];
        let viscosity: u8 = 0; // per-liquid in P4.8

        // ── Intra-chunk: right + down pairs only ─────────────────────────
        for y in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let idx = y * CHUNK_SIZE + x;
                if chunk.terrain[idx] != MAT_AIR {
                    continue;
                }
                let here = amounts[idx];

                if x + 1 < CHUNK_SIZE {
                    let ni = idx + 1;
                    if chunk.terrain[ni] == MAT_AIR {
                        apply_intra(here, amounts[ni], viscosity, idx, ni, &mut deltas);
                    }
                }
                if y + 1 < CHUNK_SIZE {
                    let ni = idx + CHUNK_SIZE;
                    if chunk.terrain[ni] == MAT_AIR {
                        apply_intra(here, amounts[ni], viscosity, idx, ni, &mut deltas);
                    }
                }
            }
        }

        // ── Cross-chunk: all 4 edges ──────────────────────────────────────
        let right = ChunkCoord {
            cx: coord.cx + 1,
            ..coord
        };
        let left = ChunkCoord {
            cx: coord.cx - 1,
            ..coord
        };
        let down = ChunkCoord {
            cy: coord.cy + 1,
            ..coord
        };
        let up = ChunkCoord {
            cy: coord.cy - 1,
            ..coord
        };

        for y in 0..CHUNK_SIZE {
            let r_our = y * CHUNK_SIZE + (CHUNK_SIZE - 1);
            let r_nb = y * CHUNK_SIZE;
            apply_cross(
                r_our,
                right,
                r_nb,
                &amounts,
                &chunk.terrain,
                &snapshot,
                viscosity,
                &mut deltas,
                &mut wake,
            );

            let l_our = y * CHUNK_SIZE;
            let l_nb = y * CHUNK_SIZE + (CHUNK_SIZE - 1);
            apply_cross(
                l_our,
                left,
                l_nb,
                &amounts,
                &chunk.terrain,
                &snapshot,
                viscosity,
                &mut deltas,
                &mut wake,
            );
        }
        for x in 0..CHUNK_SIZE {
            let d_our = (CHUNK_SIZE - 1) * CHUNK_SIZE + x;
            let d_nb = x;
            apply_cross(
                d_our,
                down,
                d_nb,
                &amounts,
                &chunk.terrain,
                &snapshot,
                viscosity,
                &mut deltas,
                &mut wake,
            );

            let u_our = x;
            let u_nb = (CHUNK_SIZE - 1) * CHUNK_SIZE + x;
            apply_cross(
                u_our,
                up,
                u_nb,
                &amounts,
                &chunk.terrain,
                &snapshot,
                viscosity,
                &mut deltas,
                &mut wake,
            );
        }

        // ── Apply deltas ──────────────────────────────────────────────────
        chunk.liquid_amount_write.copy_from_slice(&amounts);
        for (i, &d) in deltas.iter().enumerate() {
            if d != 0 {
                let v = (chunk.liquid_amount_write[i] as i16) + d;
                chunk.liquid_amount_write[i] = v.clamp(0, 255) as u8;
            }
        }
    }
}

#[inline]
fn apply_intra(
    here: u8,
    there: u8,
    visc: u8,
    hi: usize,
    ti: usize,
    deltas: &mut [i16; CHUNK_AREA],
) {
    let f = flow_between(here, there, visc);
    if f > 0 {
        deltas[hi] -= f;
        deltas[ti] += f;
    } else {
        let r = flow_between(there, here, visc);
        if r > 0 {
            deltas[ti] -= r;
            deltas[hi] += r;
        }
    }
}

/// Modifies only OUR tile's delta. Neighbor handles its side symmetrically.
#[allow(clippy::too_many_arguments)]
#[inline]
fn apply_cross(
    our_idx: usize,
    nb_coord: ChunkCoord,
    nb_idx: usize,
    amounts: &[u8; CHUNK_AREA],
    terrain: &[tile_core::material::MaterialId; CHUNK_AREA],
    snapshot: &LiquidSnapshot,
    visc: u8,
    deltas: &mut [i16; CHUNK_AREA],
    wake: &mut WakeRequests,
) {
    if terrain[our_idx] != MAT_AIR {
        return;
    }
    if !snapshot.is_passable(nb_coord, nb_idx) {
        return;
    }
    let self_val = amounts[our_idx];
    let nb_val = snapshot.get_amount(nb_coord, nb_idx);

    let out = flow_between(self_val, nb_val, visc);
    if out > 0 {
        deltas[our_idx] -= out;
        wake.pending.push(nb_coord);
        return;
    }
    let inflow = flow_between(nb_val, self_val, visc);
    if inflow > 0 {
        deltas[our_idx] += inflow;
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

    fn total_liquid(chunk: &ChunkData) -> u32 {
        chunk.liquid_amount_read.iter().map(|&a| a as u32).sum()
    }

    fn make_chunk(coord: ChunkCoord, water_tiles: &[(usize, usize, u8)]) -> ChunkData {
        let mut chunk = ChunkData::new_filled(coord, MAT_AIR);
        for &(x, y, amount) in water_tiles {
            let idx = y * CHUNK_SIZE + x;
            chunk.liquid_kind[idx] = tile_core::liquid::LiquidId(1);
            chunk.liquid_amount_read[idx] = amount;
        }
        chunk
    }

    fn make_app_with_snapshot() -> bevy_app::App {
        let mut app = bevy_app::App::new();
        app.init_resource::<LiquidSnapshot>();
        app.init_resource::<WakeRequests>();
        app.add_systems(
            bevy_app::Update,
            (
                crate::snapshot::snapshot_liquid,
                fluid_step_local,
                swap_buffers_system,
            )
                .chain(),
        );
        app
    }

    #[test]
    fn test_flow_between_basic() {
        assert!(flow_between(100, 0, 0) > 0);
        assert_eq!(flow_between(50, 50, 0), 0);
        assert_eq!(flow_between(51, 50, 0), 0);
        let full = flow_between(100, 0, 0);
        let visc = flow_between(100, 0, 200);
        assert!(visc < full);
        assert_eq!(flow_between(100, 0, 255), 0);
    }

    #[test]
    fn test_mass_conservation_single_chunk() {
        let mut app = make_app_with_snapshot();
        let chunk = make_chunk(
            ChunkCoord {
                cx: 0,
                cy: 0,
                cz: 0,
            },
            &[(16, 16, 200), (17, 16, 100), (15, 16, 50)],
        );
        let initial = total_liquid(&chunk);
        let entity = app.world_mut().spawn(chunk).id();
        for _ in 0..20 {
            app.update();
        }
        let chunk = app.world().get::<ChunkData>(entity).unwrap();
        assert_eq!(total_liquid(chunk), initial, "mass conserved");
    }

    #[test]
    fn test_water_spreads_single_chunk() {
        let mut app = make_app_with_snapshot();
        let chunk = make_chunk(
            ChunkCoord {
                cx: 0,
                cy: 0,
                cz: 0,
            },
            &[(16, 16, 100)],
        );
        let entity = app.world_mut().spawn(chunk).id();
        app.update();
        let chunk = app.world().get::<ChunkData>(entity).unwrap();
        let center = 16 * CHUNK_SIZE + 16;
        let right = 16 * CHUNK_SIZE + 17;
        let down = 17 * CHUNK_SIZE + 16;
        assert!(
            chunk.liquid_amount_read[center] < 100,
            "center should lose water"
        );
        assert!(
            chunk.liquid_amount_read[right] > 0 || chunk.liquid_amount_read[down] > 0,
            "at least one neighbor gains water"
        );
    }

    #[test]
    fn test_solid_blocks_flow() {
        let mut app = make_app_with_snapshot();
        let mut chunk = make_chunk(
            ChunkCoord {
                cx: 0,
                cy: 0,
                cz: 0,
            },
            &[(5, 5, 200)],
        );
        chunk.terrain[5 * CHUNK_SIZE + 4] = tile_core::material::MaterialId(1);
        chunk.terrain[5 * CHUNK_SIZE + 6] = tile_core::material::MaterialId(1);
        chunk.terrain[4 * CHUNK_SIZE + 5] = tile_core::material::MaterialId(1);
        chunk.terrain[6 * CHUNK_SIZE + 5] = tile_core::material::MaterialId(1);
        let entity = app.world_mut().spawn(chunk).id();
        for _ in 0..5 {
            app.update();
        }
        let chunk = app.world().get::<ChunkData>(entity).unwrap();
        assert_eq!(
            chunk.liquid_amount_read[5 * CHUNK_SIZE + 5],
            200,
            "enclosed water must not move"
        );
    }

    #[test]
    fn test_cross_chunk_flow_and_mass_conservation() {
        let mut app = make_app_with_snapshot();

        // Chunk A (cx=0): water at right-edge tile (31, 16)
        let mut chunk_a = make_chunk(
            ChunkCoord {
                cx: 0,
                cy: 0,
                cz: 0,
            },
            &[],
        );
        let edge_idx = 16 * CHUNK_SIZE + (CHUNK_SIZE - 1);
        chunk_a.liquid_kind[edge_idx] = tile_core::liquid::LiquidId(1);
        chunk_a.liquid_amount_read[edge_idx] = 200;

        // Chunk B (cx=1): all air, no liquid
        let chunk_b = make_chunk(
            ChunkCoord {
                cx: 1,
                cy: 0,
                cz: 0,
            },
            &[],
        );

        let initial_total = total_liquid(&chunk_a) + total_liquid(&chunk_b);
        let ea = app.world_mut().spawn(chunk_a).id();
        let eb = app.world_mut().spawn(chunk_b).id();

        for _ in 0..30 {
            app.update();
        }

        let a = app.world().get::<ChunkData>(ea).unwrap();
        let b = app.world().get::<ChunkData>(eb).unwrap();

        let final_total = total_liquid(a) + total_liquid(b);
        assert_eq!(initial_total, final_total, "mass conserved across chunks");

        // Water must have moved into chunk B
        let b_left_edge = 16 * CHUNK_SIZE;
        assert!(
            b.liquid_amount_read[b_left_edge] > 0,
            "water must cross chunk boundary"
        );
    }

    #[test]
    fn test_determinism() {
        fn run_sim(ticks: usize) -> (Vec<u8>, Vec<u8>) {
            let mut app = make_app_with_snapshot();
            let chunk = make_chunk(
                ChunkCoord {
                    cx: 0,
                    cy: 0,
                    cz: 0,
                },
                &[(16, 16, 200), (10, 10, 100), (20, 5, 150)],
            );
            let entity = app.world_mut().spawn(chunk).id();
            for _ in 0..ticks {
                app.update();
            }
            let chunk = app.world().get::<ChunkData>(entity).unwrap();
            (
                chunk.liquid_amount_read.to_vec(),
                chunk.liquid_amount_write.to_vec(),
            )
        }
        let (a, _) = run_sim(10);
        let (b, _) = run_sim(10);
        assert_eq!(a, b, "deterministic");
    }
}
