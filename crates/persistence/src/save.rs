use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufReader, BufWriter};
use tile_core::chunk::ChunkData;
use tile_core::world::World;

use crate::header::{SaveError, SaveHeader, read_header, write_header};
use crate::layout::SaveLayout;

// ── Events ────────────────────────────────────────────────────────────────────

#[derive(Event, Debug, Clone)]
pub struct SaveRequest {
    pub slot: String,
}

#[derive(Event, Debug, Clone)]
pub struct LoadRequest {
    pub slot: String,
}

// ── On-disk World state (Entity not serializable) ─────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorldSave {
    pub current_tick: u64,
}

// ── Pure save/load functions ─────────────────────────────────────────────────

/// Write header + world state + all dirty chunks to `layout`.
///
/// Atomic: writes to `<root>.tmp/` first, then renames to `<root>`.
pub fn save_world(
    layout: &SaveLayout,
    header: &SaveHeader,
    world: &World,
    chunks: impl Iterator<Item = ChunkData>,
) -> Result<(), SaveError> {
    // Write to a tmp directory alongside the real one.
    let tmp_root = {
        let mut p = layout.root().to_path_buf();
        let name = p
            .file_name()
            .map(|n| format!("{}.tmp", n.to_string_lossy()))
            .unwrap_or_else(|| "save.tmp".to_string());
        p.set_file_name(name);
        p
    };
    let tmp_layout = SaveLayout::new(&tmp_root);
    tmp_layout.create_dirs()?;

    // header.bin
    {
        let f = fs::File::create(tmp_layout.header_path())?;
        write_header(&mut BufWriter::new(f), header)?;
    }

    // world.bin — current_tick only (chunk-map rebuilt from chunk files + worldgen on load)
    {
        let world_save = WorldSave {
            current_tick: world.current_tick,
        };
        let f = fs::File::create(tmp_layout.world_path())?;
        bincode::serialize_into(BufWriter::new(f), &world_save)?;
    }

    // chunks/<cx>_<cy>_<cz>.bin — dirty chunks only
    for chunk in chunks {
        if !chunk.dirty {
            continue;
        }
        let c = chunk.coord;
        let path = tmp_layout.chunk_path(c.cx, c.cy, c.cz);
        let f = fs::File::create(path)?;
        bincode::serialize_into(BufWriter::new(f), &chunk)?;
    }

    // Atomic rename: remove old (if exists), rename tmp → real.
    if layout.root().exists() {
        fs::remove_dir_all(layout.root())?;
    }
    fs::rename(&tmp_root, layout.root())?;

    Ok(())
}

/// Load a save from `layout`.
///
/// Restores `world.current_tick` and returns all saved dirty chunks.
/// The caller is responsible for applying them to existing ECS entities.
pub fn load_world(layout: &SaveLayout) -> Result<(WorldSave, Vec<ChunkData>), SaveError> {
    // Validate header.
    {
        let f = fs::File::open(layout.header_path())?;
        read_header(&mut BufReader::new(f))?;
    }

    // World state.
    let world_save: WorldSave = {
        let f = fs::File::open(layout.world_path())?;
        bincode::deserialize_from(BufReader::new(f))?
    };

    // Dirty chunks.
    let chunks_dir = layout.chunks_dir();
    let mut chunks = Vec::new();
    if chunks_dir.is_dir() {
        for entry in fs::read_dir(&chunks_dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("bin") {
                continue;
            }
            let f = fs::File::open(&path)?;
            let mut chunk: ChunkData = bincode::deserialize_from(BufReader::new(f))?;
            chunk.dirty = true; // restored from save — still player-modified
            chunks.push(chunk);
        }
    }

    Ok((world_save, chunks))
}

// ── Bevy Plugin ───────────────────────────────────────────────────────────────

pub struct PersistencePlugin;

impl bevy_app::Plugin for PersistencePlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_event::<SaveRequest>();
        app.add_event::<LoadRequest>();
        app.add_systems(
            bevy_app::Update,
            (handle_save_requests, handle_load_requests).chain(),
        );
    }
}

fn handle_save_requests(
    mut events: EventReader<SaveRequest>,
    world: Res<World>,
    chunks: Query<&ChunkData>,
) {
    for req in events.read() {
        let layout = SaveLayout::new(&req.slot);
        let header = SaveHeader::new(0, (0, 0)); // seed/size not available here; P5.3 will wire WorldgenConfig
        if let Err(e) = save_world(&layout, &header, &world, chunks.iter().cloned()) {
            eprintln!("[persistence] save failed: {e}");
        } else {
            eprintln!("[persistence] saved to {}", req.slot);
        }
    }
}

fn handle_load_requests(
    mut events: EventReader<LoadRequest>,
    mut world: ResMut<World>,
    mut chunks: Query<&mut ChunkData>,
) {
    for req in events.read() {
        let layout = SaveLayout::new(&req.slot);
        match load_world(&layout) {
            Err(e) => eprintln!("[persistence] load failed: {e}"),
            Ok((world_save, loaded_chunks)) => {
                world.current_tick = world_save.current_tick;
                for loaded in loaded_chunks {
                    let coord = loaded.coord;
                    if let Some(&entity) = world.chunks.get(&coord)
                        && let Ok(mut chunk) = chunks.get_mut(entity)
                    {
                        *chunk = loaded;
                    }
                }
                eprintln!("[persistence] loaded from {}", req.slot);
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tile_core::coords::ChunkCoord;
    use tile_core::material::{MAT_AIR, MaterialId};

    fn tmp_slot(id: &str) -> String {
        std::env::temp_dir()
            .join(format!("tile_engine_p52_{}_{id}", std::process::id()))
            .to_string_lossy()
            .into_owned()
    }

    fn make_dirty_chunk(coord: ChunkCoord, mat: MaterialId) -> ChunkData {
        let mut c = ChunkData::new_filled(coord, MAT_AIR);
        c.terrain[0] = mat;
        c.dirty = true;
        c
    }

    fn make_clean_chunk(coord: ChunkCoord) -> ChunkData {
        ChunkData::new_filled(coord, MAT_AIR)
    }

    fn make_world_with_chunks(chunks: &[(ChunkCoord, Entity)]) -> World {
        let mut w = World {
            current_tick: 42,
            ..Default::default()
        };
        for (coord, entity) in chunks {
            w.chunks.insert(*coord, *entity);
        }
        w
    }

    #[test]
    fn test_save_load_roundtrip() {
        let slot = tmp_slot("roundtrip");
        let layout = SaveLayout::new(&slot);

        let coord_a = ChunkCoord {
            cx: 1,
            cy: 2,
            cz: 0,
        };
        let coord_b = ChunkCoord {
            cx: 3,
            cy: 4,
            cz: 0,
        };

        let dirty = make_dirty_chunk(coord_a, MaterialId(1));
        let clean = make_clean_chunk(coord_b);

        let world = make_world_with_chunks(&[
            (coord_a, Entity::from_raw(10)),
            (coord_b, Entity::from_raw(11)),
        ]);

        let header = SaveHeader::new(999, (64, 64));
        save_world(&layout, &header, &world, [dirty.clone(), clean].into_iter())
            .expect("save failed");

        let (ws, loaded_chunks) = load_world(&layout).expect("load failed");

        assert_eq!(ws.current_tick, 42);
        // Only dirty chunk should be on disk.
        assert_eq!(loaded_chunks.len(), 1);
        assert_eq!(loaded_chunks[0].coord, coord_a);
        assert_eq!(loaded_chunks[0].terrain[0], MaterialId(1));

        std::fs::remove_dir_all(&slot).ok();
    }

    #[test]
    fn test_clean_chunks_not_saved() {
        let slot = tmp_slot("clean");
        let layout = SaveLayout::new(&slot);

        let coord = ChunkCoord {
            cx: 0,
            cy: 0,
            cz: 0,
        };
        let clean = make_clean_chunk(coord);
        let world = make_world_with_chunks(&[(coord, Entity::from_raw(1))]);

        let header = SaveHeader::new(0, (0, 0));
        save_world(&layout, &header, &world, [clean].into_iter()).expect("save failed");

        let (_, chunks) = load_world(&layout).expect("load failed");
        assert!(chunks.is_empty(), "no dirty chunks → nothing saved");

        std::fs::remove_dir_all(&slot).ok();
    }

    #[test]
    fn test_bitidentical_roundtrip() {
        let slot_a = tmp_slot("bit_a");
        let slot_b = tmp_slot("bit_b");

        let coord = ChunkCoord {
            cx: 0,
            cy: 0,
            cz: 0,
        };
        let dirty = make_dirty_chunk(coord, MaterialId(3));
        let mut world = make_world_with_chunks(&[(coord, Entity::from_raw(1))]);
        world.current_tick = 77;

        let header = SaveHeader::new(123, (32, 32));

        // First save.
        save_world(
            &SaveLayout::new(&slot_a),
            &header,
            &world,
            [dirty.clone()].into_iter(),
        )
        .expect("save_a failed");

        // Load, then save again.
        let (ws, loaded) = load_world(&SaveLayout::new(&slot_a)).expect("load failed");
        world.current_tick = ws.current_tick;
        let reloaded_chunk = loaded.into_iter().next().unwrap();

        save_world(
            &SaveLayout::new(&slot_b),
            &header,
            &world,
            [reloaded_chunk].into_iter(),
        )
        .expect("save_b failed");

        // Compare chunk files — must be bitidentical.
        let file_a = SaveLayout::new(&slot_a).chunk_path(coord.cx, coord.cy, coord.cz);
        let file_b = SaveLayout::new(&slot_b).chunk_path(coord.cx, coord.cy, coord.cz);
        let bytes_a = std::fs::read(&file_a).unwrap();
        let bytes_b = std::fs::read(&file_b).unwrap();
        assert_eq!(
            bytes_a, bytes_b,
            "save→load→save must produce bitidentical chunk files"
        );

        std::fs::remove_dir_all(&slot_a).ok();
        std::fs::remove_dir_all(&slot_b).ok();
    }

    #[test]
    fn test_atomic_write_no_corruption_on_clean_target() {
        let slot = tmp_slot("atomic");
        let layout = SaveLayout::new(&slot);

        let coord = ChunkCoord {
            cx: 0,
            cy: 0,
            cz: 0,
        };
        let dirty = make_dirty_chunk(coord, MaterialId(2));
        let world = make_world_with_chunks(&[(coord, Entity::from_raw(1))]);
        let header = SaveHeader::new(0, (0, 0));

        // First save creates the directory.
        save_world(&layout, &header, &world, [dirty].into_iter()).expect("first save failed");
        assert!(layout.root().is_dir(), "save dir must exist after save");
        assert!(
            !layout
                .root()
                .with_file_name(format!(
                    "{}.tmp",
                    layout.root().file_name().unwrap().to_string_lossy()
                ))
                .exists(),
            "tmp dir must be cleaned up"
        );

        std::fs::remove_dir_all(&slot).ok();
    }

    #[test]
    fn test_bad_magic_rejected_on_load() {
        use crate::header::SAVE_MAGIC;
        use std::io::Write;

        let slot = tmp_slot("badmagic");
        let layout = SaveLayout::new(&slot);
        layout.create_dirs().unwrap();

        // Write a header with wrong magic bytes directly.
        let mut f = std::fs::File::create(layout.header_path()).unwrap();
        let bad: [u8; 4] = *b"XXXX";
        let version: u32 = 1;
        // Serialize manually (magic, version, then enough bytes).
        let _ = SAVE_MAGIC; // silence unused
        f.write_all(&bad).unwrap();
        f.write_all(&version.to_le_bytes()).unwrap();
        // Write placeholder bytes so bincode has enough data.
        f.write_all(&[0u8; 64]).unwrap();
        drop(f);

        let result = load_world(&layout);
        assert!(result.is_err(), "should reject bad magic");
        let err = result.err().unwrap();
        // Either BadMagic or Decode error (bincode sees corrupt header).
        assert!(
            matches!(
                err,
                crate::header::SaveError::BadMagic { .. } | crate::header::SaveError::Decode(_)
            ),
            "unexpected error variant"
        );

        std::fs::remove_dir_all(&slot).ok();
    }
}
