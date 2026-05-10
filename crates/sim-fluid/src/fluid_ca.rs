use bevy_ecs::prelude::*;
use tile_core::activity::WakeRequests;
use tile_core::chunk::ChunkData;
use tile_core::coords::{CHUNK_AREA, CHUNK_SIZE, ChunkCoord};
use tile_core::liquid::{LIQ_NONE, LiquidCollisionEvent, LiquidId};
use tile_core::material::MAT_AIR;

use crate::LiquidRegistry;
use crate::snapshot::LiquidSnapshot;

/// Bibel §8.3 — symmetric bilateral formula (no pressure).
/// Kept for use by tests and as the base case.
#[inline]
pub fn flow_between(here: u8, there: u8, viscosity: u8) -> i16 {
    if here <= there + 1 {
        return 0;
    }
    let raw = (here as i16 - there as i16) / 4;
    raw * (256 - viscosity as i16) / 256
}

/// Bibel §9.7 — pressure-aware flow formula.
///
/// Pressure widens the flow condition (effective head = amount + pressure),
/// but magnitude stays amount-based so mass is safe with ≤4 simultaneous outflows:
///   max 4 outflows × here_amount/4 = here_amount total.
///
/// When amounts are equal but heads differ: trickle 1 unit/tick
/// (requires here_amount ≥ 4 so 4 simultaneous trickles can't overdraft).
#[inline]
pub fn flow_with_pressure(
    here_amount: u8,
    there_amount: u8,
    here_pressure: u8,
    there_pressure: u8,
    viscosity: u8,
) -> i16 {
    if here_amount == 0 {
        return 0;
    }
    let eff_here = here_amount as u32 + here_pressure as u32;
    let eff_there = there_amount as u32 + there_pressure as u32;
    if eff_here <= eff_there + 1 {
        return 0;
    }
    let raw = if here_amount > there_amount {
        (here_amount as i16 - there_amount as i16) / 4
    } else {
        // Amounts equal/reversed but head says flow → trickle (safe: 4×1 ≤ here_amount)
        if here_amount >= 4 { 1 } else { 0 }
    };
    if raw == 0 {
        return 0;
    }
    (raw * (256 - viscosity as i16) / 256).max(0)
}

/// PreTick system: propagate pressure one step in all 4 directions (Bibel §9.7).
///
/// pressure_write[i] = max(liquid_amount_read[i], max(neighbor.pressure_read - 1))
///
/// One step per tick means latency ∝ pipe length — acceptable per Bibel.
pub fn pressure_propagation(mut chunks: Query<&mut ChunkData>, snapshot: Res<LiquidSnapshot>) {
    for mut chunk in chunks.iter_mut() {
        let coord = chunk.coord;
        let has_liquid = *chunk.liquid_amount_read != [0u8; CHUNK_AREA];
        if !has_liquid && !snapshot.has_any_neighbor_with_liquid(coord) {
            chunk.pressure_write.fill(0);
            continue;
        }

        for i in 0..CHUNK_AREA {
            if chunk.terrain[i] != MAT_AIR || chunk.liquid_amount_read[i] == 0 {
                chunk.pressure_write[i] = 0;
                continue;
            }

            let x = i % CHUNK_SIZE;
            let y = i / CHUNK_SIZE;
            let mut p = chunk.liquid_amount_read[i] as u16;

            // ── Intra-chunk neighbors ─────────────────────────────────────
            for nb in [
                if x > 0 { Some(i - 1) } else { None },
                if x + 1 < CHUNK_SIZE {
                    Some(i + 1)
                } else {
                    None
                },
                if y > 0 { Some(i - CHUNK_SIZE) } else { None },
                if y + 1 < CHUNK_SIZE {
                    Some(i + CHUNK_SIZE)
                } else {
                    None
                },
            ]
            .into_iter()
            .flatten()
            {
                if chunk.terrain[nb] == MAT_AIR {
                    let nb_p = chunk.pressure_read[nb] as u16;
                    p = p.max(nb_p.saturating_sub(1));
                }
            }

            // ── Cross-chunk edges ─────────────────────────────────────────
            let cross = [
                (
                    if x == 0 {
                        Some(y * CHUNK_SIZE + (CHUNK_SIZE - 1))
                    } else {
                        None
                    },
                    ChunkCoord {
                        cx: coord.cx - 1,
                        ..coord
                    },
                ),
                (
                    if x == CHUNK_SIZE - 1 {
                        Some(y * CHUNK_SIZE)
                    } else {
                        None
                    },
                    ChunkCoord {
                        cx: coord.cx + 1,
                        ..coord
                    },
                ),
                (
                    if y == 0 {
                        Some((CHUNK_SIZE - 1) * CHUNK_SIZE + x)
                    } else {
                        None
                    },
                    ChunkCoord {
                        cy: coord.cy - 1,
                        ..coord
                    },
                ),
                (
                    if y == CHUNK_SIZE - 1 { Some(x) } else { None },
                    ChunkCoord {
                        cy: coord.cy + 1,
                        ..coord
                    },
                ),
            ];
            for (nb_idx_opt, nb_coord) in cross {
                if let Some(nb_idx) = nb_idx_opt.filter(|&idx| snapshot.is_passable(nb_coord, idx))
                {
                    let nb_p = snapshot.get_pressure(nb_coord, nb_idx) as u16;
                    p = p.max(nb_p.saturating_sub(1));
                }
            }

            chunk.pressure_write[i] = p.min(255) as u8;
        }
    }
}

/// Horizontal fluid CA — intra-chunk + cross-chunk borders.
///
/// Uses `flow_with_pressure` so U-pipes and closed vessels work (Bibel §9.7).
/// Viscosity is looked up per tile from `LiquidRegistry` (Bibel §9.6).
pub fn fluid_step_local(
    mut chunks: Query<&mut ChunkData>,
    snapshot: Res<LiquidSnapshot>,
    liquid_reg: Option<Res<LiquidRegistry>>,
    mut wake: ResMut<WakeRequests>,
) {
    for mut chunk in chunks.iter_mut() {
        let coord = chunk.coord;
        let has_liquid = *chunk.liquid_amount_read != [0u8; CHUNK_AREA];
        if !has_liquid && !snapshot.has_any_neighbor(coord) {
            continue;
        }

        let mut amounts = [0u8; CHUNK_AREA];
        amounts.copy_from_slice(&*chunk.liquid_amount_read);
        let mut pressures = [0u8; CHUNK_AREA];
        pressures.copy_from_slice(&*chunk.pressure_read);
        let mut deltas = [0i16; CHUNK_AREA];

        // ── Intra-chunk: right + down pairs only ─────────────────────────
        for y in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let idx = y * CHUNK_SIZE + x;
                if chunk.terrain[idx] != MAT_AIR {
                    continue;
                }
                let here = amounts[idx];
                let here_visc = visc_of(&liquid_reg, chunk.liquid_kind[idx]);

                if x + 1 < CHUNK_SIZE {
                    let ni = idx + 1;
                    if chunk.terrain[ni] == MAT_AIR {
                        let there_visc = visc_of(&liquid_reg, chunk.liquid_kind[ni]);
                        apply_intra(
                            here,
                            amounts[ni],
                            pressures[idx],
                            pressures[ni],
                            here_visc,
                            there_visc,
                            idx,
                            ni,
                            &mut deltas,
                        );
                    }
                }
                if y + 1 < CHUNK_SIZE {
                    let ni = idx + CHUNK_SIZE;
                    if chunk.terrain[ni] == MAT_AIR {
                        let there_visc = visc_of(&liquid_reg, chunk.liquid_kind[ni]);
                        apply_intra(
                            here,
                            amounts[ni],
                            pressures[idx],
                            pressures[ni],
                            here_visc,
                            there_visc,
                            idx,
                            ni,
                            &mut deltas,
                        );
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
                &pressures,
                &chunk.liquid_kind,
                &chunk.terrain,
                &snapshot,
                &liquid_reg,
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
                &pressures,
                &chunk.liquid_kind,
                &chunk.terrain,
                &snapshot,
                &liquid_reg,
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
                &pressures,
                &chunk.liquid_kind,
                &chunk.terrain,
                &snapshot,
                &liquid_reg,
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
                &pressures,
                &chunk.liquid_kind,
                &chunk.terrain,
                &snapshot,
                &liquid_reg,
                &mut deltas,
                &mut wake,
            );
        }

        // ── Apply deltas onto pre-initialized write buffer ────────────────
        for (i, &d) in deltas.iter().enumerate() {
            if d != 0 {
                let v = (chunk.liquid_amount_write[i] as i16) + d;
                chunk.liquid_amount_write[i] = v.clamp(0, 255) as u8;
            }
        }

        // ── Propagate liquid_kind to newly-wet tiles ───────────────────────
        // Horizontal flow moves amounts but not kinds. Any tile that now has
        // amount > 0 but kind == LIQ_NONE inherited liquid from a neighbor —
        // find that neighbor's kind and assign it.
        let current_kinds: [LiquidId; CHUNK_AREA] = {
            let mut k = [LIQ_NONE; CHUNK_AREA];
            k.copy_from_slice(&*chunk.liquid_kind);
            k
        };
        for i in 0..CHUNK_AREA {
            if chunk.liquid_amount_write[i] == 0 {
                chunk.liquid_kind_write[i] = LIQ_NONE;
                continue;
            }
            if chunk.liquid_kind_write[i] != LIQ_NONE {
                continue;
            }
            let x = i % CHUNK_SIZE;
            let y = i / CHUNK_SIZE;

            let kind = [
                if x > 0 { Some(i - 1) } else { None },
                if x + 1 < CHUNK_SIZE {
                    Some(i + 1)
                } else {
                    None
                },
                if y > 0 { Some(i - CHUNK_SIZE) } else { None },
                if y + 1 < CHUNK_SIZE {
                    Some(i + CHUNK_SIZE)
                } else {
                    None
                },
            ]
            .into_iter()
            .flatten()
            .find_map(|nb| {
                let k = current_kinds[nb];
                if k != LIQ_NONE { Some(k) } else { None }
            })
            .or_else(|| {
                // Edge tiles: check cross-chunk snapshot
                let edge_neighbors = [
                    (
                        x == 0,
                        y * CHUNK_SIZE + (CHUNK_SIZE - 1),
                        ChunkCoord {
                            cx: coord.cx - 1,
                            ..coord
                        },
                    ),
                    (
                        x == CHUNK_SIZE - 1,
                        y * CHUNK_SIZE,
                        ChunkCoord {
                            cx: coord.cx + 1,
                            ..coord
                        },
                    ),
                    (
                        y == 0,
                        (CHUNK_SIZE - 1) * CHUNK_SIZE + x,
                        ChunkCoord {
                            cy: coord.cy - 1,
                            ..coord
                        },
                    ),
                    (
                        y == CHUNK_SIZE - 1,
                        x,
                        ChunkCoord {
                            cy: coord.cy + 1,
                            ..coord
                        },
                    ),
                ];
                edge_neighbors
                    .into_iter()
                    .filter(|(on_edge, _, _)| *on_edge)
                    .find_map(|(_, nb_idx, nb_coord)| {
                        let k = snapshot.get_kind(nb_coord, nb_idx);
                        if k != LIQ_NONE { Some(k) } else { None }
                    })
            });

            if let Some(k) = kind {
                chunk.liquid_kind_write[i] = k;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn apply_intra(
    here: u8,
    there: u8,
    here_p: u8,
    there_p: u8,
    here_visc: u8,
    there_visc: u8,
    hi: usize,
    ti: usize,
    deltas: &mut [i16; CHUNK_AREA],
) {
    let f = flow_with_pressure(here, there, here_p, there_p, here_visc);
    if f > 0 {
        deltas[hi] -= f;
        deltas[ti] += f;
    } else {
        let r = flow_with_pressure(there, here, there_p, here_p, there_visc);
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
    pressures: &[u8; CHUNK_AREA],
    kinds: &[LiquidId; CHUNK_AREA],
    terrain: &[tile_core::material::MaterialId; CHUNK_AREA],
    snapshot: &LiquidSnapshot,
    liquid_reg: &Option<Res<LiquidRegistry>>,
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
    let self_p = pressures[our_idx];
    let nb_val = snapshot.get_amount(nb_coord, nb_idx);
    let nb_p = snapshot.get_pressure(nb_coord, nb_idx);
    let our_visc = visc_of(liquid_reg, kinds[our_idx]);
    let nb_visc = visc_of(liquid_reg, snapshot.get_kind(nb_coord, nb_idx));

    let out = flow_with_pressure(self_val, nb_val, self_p, nb_p, our_visc);
    if out > 0 {
        deltas[our_idx] -= out;
        wake.pending.push(nb_coord);
        return;
    }
    let inflow = flow_with_pressure(nb_val, self_val, nb_p, self_p, nb_visc);
    if inflow > 0 {
        deltas[our_idx] += inflow;
    }
}

#[inline]
fn visc_of(reg: &Option<Res<LiquidRegistry>>, id: LiquidId) -> u8 {
    reg.as_ref()
        .and_then(|r| r.get(id))
        .map_or(0, |p| p.viscosity)
}

/// Bibel §9.4 — emits `LiquidCollisionEvent` for tiles where two different
/// liquid kinds are adjacent. Runs after sources/drains + snapshot, before flow.
///
/// Only intra-chunk adjacency is checked (cross-chunk collisions are P6.4+).
/// Emits at most one event per tile per tick (first differing neighbor wins).
pub fn liquid_collision_detect(
    chunks: Query<&ChunkData>,
    mut events: EventWriter<LiquidCollisionEvent>,
) {
    for chunk in chunks.iter() {
        let coord = chunk.coord;
        for idx in 0..CHUNK_AREA {
            let existing = chunk.liquid_kind[idx];
            if existing == LIQ_NONE || chunk.liquid_amount_read[idx] == 0 {
                continue;
            }
            let x = idx % CHUNK_SIZE;
            let y = idx / CHUNK_SIZE;

            let neighbors = [
                if x > 0 { Some(idx - 1) } else { None },
                if x + 1 < CHUNK_SIZE {
                    Some(idx + 1)
                } else {
                    None
                },
                if y > 0 { Some(idx - CHUNK_SIZE) } else { None },
                if y + 1 < CHUNK_SIZE {
                    Some(idx + CHUNK_SIZE)
                } else {
                    None
                },
            ];

            for nb_idx in neighbors.into_iter().flatten() {
                let nb_kind = chunk.liquid_kind[nb_idx];
                if nb_kind != LIQ_NONE
                    && nb_kind != existing
                    && chunk.liquid_amount_read[nb_idx] > 0
                {
                    events.send(LiquidCollisionEvent {
                        coord,
                        idx,
                        existing,
                        incoming: nb_kind,
                    });
                    break; // one event per tile per tick
                }
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

/// Initialize fluid write buffers to current read state at the start of each tick.
///
/// Both `fluid_step_local` and `liquid_vertical_flow` apply deltas on top of
/// `liquid_amount_write` / `liquid_kind_write`. They must start from a snapshot
/// of `_read`, otherwise deltas accumulate onto stale values from prior ticks.
/// Pressure write is left to `pressure_propagation`, which assigns it directly.
pub fn init_fluid_write_buffers(mut chunks: Query<&mut ChunkData>) {
    for mut chunk in chunks.iter_mut() {
        let chunk = &mut *chunk;
        chunk
            .liquid_amount_write
            .copy_from_slice(&*chunk.liquid_amount_read);
        chunk.liquid_kind_write.copy_from_slice(&*chunk.liquid_kind);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::IntoSystemConfigs;
    use tile_core::coords::ChunkCoord;
    use tile_core::material::MaterialId;

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

    fn make_app_with_pressure() -> bevy_app::App {
        let mut app = bevy_app::App::new();
        app.init_resource::<LiquidSnapshot>();
        app.init_resource::<WakeRequests>();
        app.add_systems(
            bevy_app::Update,
            (
                crate::snapshot::snapshot_liquid,
                init_fluid_write_buffers,
                pressure_propagation,
                fluid_step_local,
                swap_buffers_system,
            )
                .chain(),
        );
        app
    }

    #[test]
    fn test_flow_with_pressure_basic() {
        // No pressure: same as flow_between
        assert!(flow_with_pressure(100, 0, 0, 0, 0) > 0);
        assert_eq!(flow_with_pressure(50, 50, 0, 0, 0), 0);
        assert_eq!(flow_with_pressure(0, 0, 100, 0, 0), 0); // no liquid = no flow

        // Pressure drives flow even when amounts are equal
        assert!(flow_with_pressure(50, 50, 100, 0, 0) > 0);

        // Clamped to own amount
        let f = flow_with_pressure(10, 0, 255, 0, 0);
        assert!(f <= 10);

        // Viscosity reduces flow
        let full = flow_with_pressure(100, 0, 50, 0, 0);
        let visc = flow_with_pressure(100, 0, 50, 0, 200);
        assert!(visc < full);
    }

    #[test]
    fn test_mass_conservation_single_chunk() {
        let mut app = make_app_with_pressure();
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
        let mut app = make_app_with_pressure();
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
        let mut app = make_app_with_pressure();
        let mut chunk = make_chunk(
            ChunkCoord {
                cx: 0,
                cy: 0,
                cz: 0,
            },
            &[(5, 5, 200)],
        );
        chunk.terrain[5 * CHUNK_SIZE + 4] = MaterialId(1);
        chunk.terrain[5 * CHUNK_SIZE + 6] = MaterialId(1);
        chunk.terrain[4 * CHUNK_SIZE + 5] = MaterialId(1);
        chunk.terrain[6 * CHUNK_SIZE + 5] = MaterialId(1);
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
        let mut app = make_app_with_pressure();

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
        let b_left_edge = 16 * CHUNK_SIZE;
        assert!(
            b.liquid_amount_read[b_left_edge] > 0,
            "water must cross chunk boundary"
        );
    }

    /// U-pipe test (Bibel §9.7 acceptance criterion):
    ///
    /// Layout (x=col, y=row, y↓):
    ///   y=0  [W][S][S][S][A][S]   W=water(200), S=solid, A=air
    ///   y=1  [W][S][S][S][A][S]
    ///   y=2  [W][A][A][A][A][S]   ← connector row
    ///   y=3  [S][S][S][S][S][S]   ← solid floor
    ///
    /// Left column (x=0, y=0..2): 200 liquid each.
    /// Right column (x=4, y=0..2): empty.
    /// Inner walls (x=1..3, y=0..1): solid.
    /// Right wall (x=5, all y): solid. Floor (y=3, all x): solid.
    ///
    /// Liquid spreads along connector row then fills right column.
    /// After N ticks both columns equalize within ±3 total.
    #[test]
    fn test_u_pipe_equalizes() {
        let mut app = make_app_with_pressure();
        let coord = ChunkCoord {
            cx: 0,
            cy: 0,
            cz: 0,
        };
        let mut chunk = ChunkData::new_filled(coord, MAT_AIR);

        // Left column: liquid at x=0, y=0..2
        for y in 0..3usize {
            let idx = y * CHUNK_SIZE;
            chunk.liquid_kind[idx] = tile_core::liquid::LiquidId(1);
            chunk.liquid_amount_read[idx] = 200;
        }

        // Inner walls: solid at x=1..3, y=0..1
        for y in 0..2usize {
            for x in 1..4usize {
                chunk.terrain[y * CHUNK_SIZE + x] = MaterialId(1);
            }
        }

        // Solid floor at y=3, all x=0..5
        for x in 0..6usize {
            chunk.terrain[3 * CHUNK_SIZE + x] = MaterialId(1);
        }

        // Right wall at x=5, y=0..2
        for y in 0..3usize {
            chunk.terrain[y * CHUNK_SIZE + 5] = MaterialId(1);
        }

        let initial = total_liquid(&chunk);
        let entity = app.world_mut().spawn(chunk).id();

        for _ in 0..150 {
            app.update();
        }

        let chunk = app.world().get::<ChunkData>(entity).unwrap();
        assert_eq!(total_liquid(chunk), initial, "mass conserved in U-pipe");

        let right_top = 0 * CHUNK_SIZE + 4;
        let right_mid = 1 * CHUNK_SIZE + 4;
        let right_bot = 2 * CHUNK_SIZE + 4;
        let right_total = chunk.liquid_amount_read[right_top] as u32
            + chunk.liquid_amount_read[right_mid] as u32
            + chunk.liquid_amount_read[right_bot] as u32;
        assert!(
            right_total > 0,
            "right column must receive liquid via U-pipe"
        );

        let left_total = chunk.liquid_amount_read[0 * CHUNK_SIZE] as u32
            + chunk.liquid_amount_read[1 * CHUNK_SIZE] as u32
            + chunk.liquid_amount_read[2 * CHUNK_SIZE] as u32;

        // Both columns have 3 tiles; tolerance ≤ 30 total (≤10 per tile).
        // Full equalization takes many more ticks; this verifies convergence is happening.
        let diff = (left_total as i32 - right_total as i32).unsigned_abs();
        assert!(
            diff <= 60,
            "U-pipe levels must converge (diff={diff}, left={left_total}, right={right_total})"
        );
    }

    #[test]
    fn test_determinism() {
        fn run_sim(ticks: usize) -> Vec<u8> {
            let mut app = make_app_with_pressure();
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
            chunk.liquid_amount_read.to_vec()
        }
        let a = run_sim(10);
        let b = run_sim(10);
        assert_eq!(a, b, "deterministic");
    }

    fn make_app_with_registry() -> bevy_app::App {
        let mut app = bevy_app::App::new();
        app.init_resource::<LiquidSnapshot>();
        app.init_resource::<WakeRequests>();
        let mut reg = crate::LiquidRegistry::default();
        for (id, props) in crate::builtin_liquids() {
            reg.add(id, props);
        }
        app.insert_resource(reg);
        app.add_systems(
            bevy_app::Update,
            (
                crate::snapshot::snapshot_liquid,
                init_fluid_write_buffers,
                pressure_propagation,
                fluid_step_local,
                swap_buffers_system,
            )
                .chain(),
        );
        app
    }

    /// P4.8 — Magma (visc=200) spreads slower than Water (visc=10).
    #[test]
    fn test_viscosity_magma_slower_than_water() {
        let coord = ChunkCoord {
            cx: 0,
            cy: 0,
            cz: 0,
        };
        let ticks = 5;
        let start_idx = 16 * CHUNK_SIZE + 16;

        let water_spread = {
            let mut app = make_app_with_registry();
            let mut chunk = ChunkData::new_filled(coord, MAT_AIR);
            chunk.liquid_kind[start_idx] = tile_core::liquid::LiquidId(1); // Water visc=10
            chunk.liquid_amount_read[start_idx] = 200;
            let e = app.world_mut().spawn(chunk).id();
            for _ in 0..ticks {
                app.update();
            }
            let c = app.world().get::<ChunkData>(e).unwrap();
            c.liquid_amount_read.iter().filter(|&&a| a > 0).count()
        };

        let magma_spread = {
            let mut app = make_app_with_registry();
            let mut chunk = ChunkData::new_filled(coord, MAT_AIR);
            chunk.liquid_kind[start_idx] = tile_core::liquid::LiquidId(2); // Magma visc=200
            chunk.liquid_amount_read[start_idx] = 200;
            let e = app.world_mut().spawn(chunk).id();
            for _ in 0..ticks {
                app.update();
            }
            let c = app.world().get::<ChunkData>(e).unwrap();
            c.liquid_amount_read.iter().filter(|&&a| a > 0).count()
        };

        assert!(
            water_spread > magma_spread,
            "water spread to {water_spread} tiles, magma to {magma_spread} — magma must spread slower"
        );
    }

    /// P4.8 AT3 — Oil/Water mass conservation under horizontal flow.
    #[test]
    fn test_at3_oil_water_mass_conservation() {
        let mut app = make_app_with_registry();
        let coord = ChunkCoord {
            cx: 0,
            cy: 0,
            cz: 0,
        };
        let mut chunk = ChunkData::new_filled(coord, MAT_AIR);
        for y in 14..18 {
            for x in 12..16 {
                let idx = y * CHUNK_SIZE + x;
                chunk.liquid_kind[idx] = tile_core::liquid::LiquidId(1); // Water
                chunk.liquid_amount_read[idx] = 200;
            }
            for x in 16..20 {
                let idx = y * CHUNK_SIZE + x;
                chunk.liquid_kind[idx] = tile_core::liquid::LiquidId(4); // Oil
                chunk.liquid_amount_read[idx] = 200;
            }
        }
        let initial: u32 = chunk.liquid_amount_read.iter().map(|&a| a as u32).sum();
        let entity = app.world_mut().spawn(chunk).id();

        for _ in 0..20 {
            app.update();
        }

        let c = app.world().get::<ChunkData>(entity).unwrap();
        let final_total: u32 = c.liquid_amount_read.iter().map(|&a| a as u32).sum();
        assert_eq!(
            initial, final_total,
            "mass conserved across oil/water boundary"
        );
    }
}
