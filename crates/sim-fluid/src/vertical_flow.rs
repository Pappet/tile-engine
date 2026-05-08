use bevy_ecs::prelude::*;
use tile_core::activity::WakeRequests;
use tile_core::chunk::ChunkData;
use tile_core::coords::{CHUNK_AREA, CHUNK_SIZE, ChunkCoord};
use tile_core::liquid::{LIQ_NONE, LiquidId};
use tile_core::material::MAT_AIR;

use crate::LiquidRegistry;
use crate::snapshot::LiquidSnapshot;

/// Bibel §9.5 — Vertical flow: gravity fall + density-based layering.
///
/// "Down" = lower cz. Each tile checks the chunk directly below (cz-1, same x,y).
///
/// Cases:
///   1. Below is air (no liquid): liquid falls — full transfer.
///   2. Below has lighter liquid: density swap — heavier sinks, lighter rises.
///   3. Below has same or heavier liquid: stable, no action.
///
/// Symmetric pattern: both chunks read from snapshot, write only own buffer.
pub fn liquid_vertical_flow(
    mut chunks: Query<&mut ChunkData>,
    snapshot: Res<LiquidSnapshot>,
    liquid_reg: Option<Res<LiquidRegistry>>,
    mut wake: ResMut<WakeRequests>,
) {
    for mut chunk in chunks.iter_mut() {
        let coord = chunk.coord;
        // Check if chunk above has liquid that could fall into us
        let above = ChunkCoord {
            cz: coord.cz + 1,
            ..coord
        };
        let below = ChunkCoord {
            cz: coord.cz - 1,
            ..coord
        };
        let above_has_liquid = snapshot.chunk_has_liquid(above);

        if chunk.liquid_count == 0 && !above_has_liquid {
            continue;
        }

        let mut amounts = [0u8; CHUNK_AREA];
        amounts.copy_from_slice(&*chunk.liquid_amount_read);
        let mut amount_deltas = [0i16; CHUNK_AREA];
        // Track per-tile kind changes from vertical flow. None = unchanged.
        let mut kind_change: [Option<LiquidId>; CHUNK_AREA] = [None; CHUNK_AREA];

        for y in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let idx = y * CHUNK_SIZE + x;

                if chunk.terrain[idx] != MAT_AIR {
                    continue;
                }

                let self_kind = chunk.liquid_kind[idx];
                let self_amt = amounts[idx];

                // ── Check tile directly below (same x,y, chunk cz-1) ─────────
                let nb_idx = idx; // same local position in the chunk below

                if snapshot.is_passable(below, nb_idx) {
                    let nb_kind = snapshot.get_kind(below, nb_idx);
                    let nb_amt = snapshot.get_amount(below, nb_idx);

                    if self_kind != LIQ_NONE && self_amt > 0 {
                        if nb_kind == LIQ_NONE || nb_amt == 0 {
                            // Case 1: air below → gravity fall
                            amount_deltas[idx] -= self_amt as i16;
                            kind_change[idx] = Some(LIQ_NONE);
                            wake.pending.push(below);
                        } else if nb_kind != self_kind {
                            // Case 2: different liquid → density swap if self is denser
                            let self_density = density(&liquid_reg, self_kind);
                            let nb_density = density(&liquid_reg, nb_kind);
                            if self_density > nb_density {
                                // Self (upper) is denser → swap: we become nb's liquid
                                amount_deltas[idx] = nb_amt as i16 - self_amt as i16;
                                kind_change[idx] = Some(nb_kind);
                                wake.pending.push(below);
                            }
                        }
                    }
                }

                // ── Receive liquid from above (chunk cz+1, same idx) ─────────
                if snapshot.is_passable(above, nb_idx) {
                    let above_kind = snapshot.get_kind(above, nb_idx);
                    let above_amt = snapshot.get_amount(above, nb_idx);

                    if above_kind != LIQ_NONE && above_amt > 0 {
                        if self_kind == LIQ_NONE || self_amt == 0 {
                            // Case 1 (receiving side): accept falling liquid
                            amount_deltas[idx] += above_amt as i16;
                            kind_change[idx] = Some(above_kind);
                        } else if above_kind != self_kind {
                            // Case 2 (receiving side): density swap — we rise if we're lighter
                            let above_density = density(&liquid_reg, above_kind);
                            let self_density = density(&liquid_reg, self_kind);
                            if above_density > self_density {
                                // Above is denser, we (lighter) should rise → we become above's liquid
                                amount_deltas[idx] = above_amt as i16 - self_amt as i16;
                                kind_change[idx] = Some(above_kind);
                            }
                        }
                    }
                }
            }
        }

        // ── Apply deltas on top of pre-initialized write buffers ──────────
        // `init_fluid_write_buffers` (and possibly horizontal CA) populated
        // liquid_amount_write/liquid_kind_write. We add vertical deltas and
        // only override kinds where vertical flow actually changed them.
        for (i, &d) in amount_deltas.iter().enumerate() {
            if d != 0 {
                let old_v = chunk.liquid_amount_write[i];
                let v = (old_v as i16) + d;
                let new_v = v.clamp(0, 255) as u8;
                if old_v == 0 && new_v > 0 {
                    chunk.liquid_count_write = chunk.liquid_count_write.saturating_add(1);
                } else if old_v > 0 && new_v == 0 {
                    chunk.liquid_count_write = chunk.liquid_count_write.saturating_sub(1);
                }
                chunk.liquid_amount_write[i] = new_v;
            }
        }
        for (i, change) in kind_change.iter().enumerate() {
            if let Some(k) = change {
                chunk.liquid_kind_write[i] = *k;
            }
        }
    }
}

/// Density for a LiquidId, defaulting to 1.0 if registry missing or id unknown.
fn density(reg: &Option<Res<LiquidRegistry>>, id: LiquidId) -> f32 {
    reg.as_ref()
        .and_then(|r| r.get(id))
        .map_or(1.0, |p| p.density)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin_liquids;
    use crate::snapshot::snapshot_liquid;
    use bevy_app::prelude::*;
    use bevy_ecs::schedule::IntoSystemConfigs;
    use tile_core::coords::ChunkCoord;

    fn make_chunk(coord: ChunkCoord) -> ChunkData {
        ChunkData::new_filled(coord, MAT_AIR)
    }

    fn make_app() -> App {
        let mut app = App::new();
        app.init_resource::<LiquidSnapshot>();
        app.init_resource::<WakeRequests>();
        let mut reg = LiquidRegistry::default();
        for (id, props) in builtin_liquids() {
            reg.add(id, props);
        }
        app.insert_resource(reg);
        app.add_systems(
            Update,
            (
                snapshot_liquid,
                crate::fluid_ca::init_fluid_write_buffers,
                liquid_vertical_flow,
                crate::fluid_ca::swap_buffers_system,
            )
                .chain(),
        );
        app
    }

    fn total_liquid(chunk: &ChunkData) -> u32 {
        chunk.liquid_amount_read.iter().map(|&a| a as u32).sum()
    }

    #[test]
    fn test_water_falls_into_air() {
        let mut app = make_app();

        // Chunk A (cz=1): water at tile (16,16) — above
        let mut chunk_a = make_chunk(ChunkCoord {
            cx: 0,
            cy: 0,
            cz: 1,
        });
        let idx = 16 * CHUNK_SIZE + 16;
        chunk_a.liquid_kind[idx] = LiquidId(1); // Water
        chunk_a.liquid_amount_read[idx] = 100;

        // Chunk B (cz=0): all air — below
        let chunk_b = make_chunk(ChunkCoord {
            cx: 0,
            cy: 0,
            cz: 0,
        });

        let ea = app.world_mut().spawn(chunk_a).id();
        let eb = app.world_mut().spawn(chunk_b).id();

        let initial = total_liquid(app.world().get::<ChunkData>(ea).unwrap())
            + total_liquid(app.world().get::<ChunkData>(eb).unwrap());

        app.update(); // 1 tick

        let a = app.world().get::<ChunkData>(ea).unwrap();
        let b = app.world().get::<ChunkData>(eb).unwrap();

        assert_eq!(
            total_liquid(a) + total_liquid(b),
            initial,
            "mass conserved during fall"
        );
        assert_eq!(a.liquid_amount_read[idx], 0, "water left upper chunk");
        assert_eq!(
            b.liquid_amount_read[idx], 100,
            "water arrived in lower chunk"
        );
        assert_eq!(b.liquid_kind[idx], LiquidId(1), "liquid kind preserved");
    }

    #[test]
    fn test_oil_water_density_swap() {
        let mut app = make_app();

        let idx = 16 * CHUNK_SIZE + 16;

        // Chunk A (cz=1, upper): Water (density=1.0) — unstable on top of Oil
        let mut chunk_a = make_chunk(ChunkCoord {
            cx: 0,
            cy: 0,
            cz: 1,
        });
        chunk_a.liquid_kind[idx] = LiquidId(1); // Water
        chunk_a.liquid_amount_read[idx] = 80;

        // Chunk B (cz=0, lower): Oil (density=0.85) — lighter, should rise
        let mut chunk_b = make_chunk(ChunkCoord {
            cx: 0,
            cy: 0,
            cz: 0,
        });
        chunk_b.liquid_kind[idx] = LiquidId(4); // Oil
        chunk_b.liquid_amount_read[idx] = 60;

        let ea = app.world_mut().spawn(chunk_a).id();
        let eb = app.world_mut().spawn(chunk_b).id();

        let initial = total_liquid(app.world().get::<ChunkData>(ea).unwrap())
            + total_liquid(app.world().get::<ChunkData>(eb).unwrap());

        app.update(); // 1 tick — density swap should occur

        let a = app.world().get::<ChunkData>(ea).unwrap();
        let b = app.world().get::<ChunkData>(eb).unwrap();

        assert_eq!(total_liquid(a) + total_liquid(b), initial, "mass conserved");
        // After swap: Oil (lighter) should be on top (cz=1), Water (denser) below (cz=0)
        assert_eq!(a.liquid_kind[idx], LiquidId(4), "oil rose to upper chunk");
        assert_eq!(b.liquid_kind[idx], LiquidId(1), "water sank to lower chunk");
        assert_eq!(a.liquid_amount_read[idx], 60, "oil amount in upper");
        assert_eq!(b.liquid_amount_read[idx], 80, "water amount in lower");
    }

    #[test]
    fn test_stable_layering_no_swap() {
        let mut app = make_app();

        let idx = 10 * CHUNK_SIZE + 10;

        // Already stable: Oil (light) on top, Water (heavy) below
        let mut chunk_a = make_chunk(ChunkCoord {
            cx: 0,
            cy: 0,
            cz: 1,
        });
        chunk_a.liquid_kind[idx] = LiquidId(4); // Oil — lighter, already on top
        chunk_a.liquid_amount_read[idx] = 50;

        let mut chunk_b = make_chunk(ChunkCoord {
            cx: 0,
            cy: 0,
            cz: 0,
        });
        chunk_b.liquid_kind[idx] = LiquidId(1); // Water — heavier, already below
        chunk_b.liquid_amount_read[idx] = 50;

        let ea = app.world_mut().spawn(chunk_a).id();
        let eb = app.world_mut().spawn(chunk_b).id();

        for _ in 0..5 {
            app.update();
        }

        let a = app.world().get::<ChunkData>(ea).unwrap();
        let b = app.world().get::<ChunkData>(eb).unwrap();

        // Should not swap (already stable)
        assert_eq!(a.liquid_kind[idx], LiquidId(4), "oil stays on top");
        assert_eq!(b.liquid_kind[idx], LiquidId(1), "water stays below");
        assert_eq!(a.liquid_amount_read[idx], 50);
        assert_eq!(b.liquid_amount_read[idx], 50);
    }
}
