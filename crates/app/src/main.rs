use bevy::prelude::*;
use sim_fluid::{FluidPlugin, LiquidRegistry, LiquidSource, builtin_liquids};
use tile_core::chunk::ChunkData;
use tile_core::liquid::LiquidId;
use tile_core::material::MAT_AIR;

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Tile Engine".to_string(),
            ..default()
        }),
        ..default()
    }));

    #[cfg(feature = "profile")]
    {
        puffin::set_scopes_on(true);
        let server_addr = format!("0.0.0.0:{}", puffin_http::DEFAULT_PORT);
        let server = puffin_http::Server::new(&server_addr).unwrap();
        eprintln!("Puffin profiler running on {}", server_addr);
        app.insert_non_send_resource(server);
        app.add_systems(Update, profile_system);
    }

    app.add_plugins(tile_core::activity::CorePlugin);
    app.add_plugins(FluidPlugin);
    app.add_plugins(render_bevy::RenderPlugin);
    app.add_plugins(worldgen_demo::DemoWorldgenPlugin);

    app.init_resource::<tile_core::world::World>();

    // Configure Worldgen
    app.insert_resource(worldgen_api::WorldgenConfig {
        seed: 123456789,
        bounds_radius: Some(2), // 2x2 chunk bounds radius -> 4x4 chunks (16 chunks)
    });

    // Initialize MaterialRegistry with builtin materials
    let mut material_reg = tile_core::material::MaterialRegistry::default();
    for (id, mat) in tile_core::material::builtin_materials() {
        material_reg.add(id, mat);
    }
    app.insert_resource(material_reg);

    // Initialize LiquidRegistry with builtin liquids
    let mut liquid_reg = LiquidRegistry::default();
    for (id, props) in builtin_liquids() {
        liquid_reg.add(id, props);
    }
    app.insert_resource(liquid_reg);

    app.add_systems(Startup, spawn_liquid_sources);
    app.add_systems(PostStartup, clear_source_terrain);
    app.add_systems(Update, dummy_system);

    app.run();
}

fn spawn_liquid_sources(mut commands: Commands) {
    // Demo basins: left=Magma, middle=Water, right=Oil.
    let [magma_pos, water_pos, oil_pos] = worldgen_demo::demo_source_positions();

    commands.spawn(LiquidSource {
        pos: magma_pos,
        kind: LiquidId(2), // Magma — visc=200, glows
        rate: 5,
        temperature: 1300,
        max_pressure: 200,
    });
    commands.spawn(LiquidSource {
        pos: water_pos,
        kind: LiquidId(1), // Water — visc=10
        rate: 5,
        temperature: 20,
        max_pressure: 255,
    });
    commands.spawn(LiquidSource {
        pos: oil_pos,
        kind: LiquidId(4), // Oil — visc=40
        rate: 5,
        temperature: 20,
        max_pressure: 255,
    });
}

/// Force-clear terrain at every LiquidSource position so sources are never blocked by worldgen.
fn clear_source_terrain(
    sources: Query<&LiquidSource>,
    mut chunks: Query<&mut ChunkData>,
) {
    for source in sources.iter() {
        let (coord, lp) = source.pos.split();
        let idx = lp.index();
        if let Some(mut chunk) = chunks.iter_mut().find(|c| c.coord == coord) {
            chunk.terrain[idx] = MAT_AIR;
        }
    }
}

#[cfg(feature = "profile")]
fn profile_system() {
    puffin::GlobalProfiler::lock().new_frame();
}

fn dummy_system() {
    #[cfg(feature = "profile")]
    puffin::profile_function!();
    // Do nothing
}

#[cfg(test)]
mod tests {
    use super::*;
    use tile_core::chunk::ChunkData;
    use tile_core::coords::WorldPos;
    use tile_core::material::MaterialId;
    use tile_core::world::{Tile, World, WorldAccess, WorldAccessMut};

    fn ensure_chunks_system(mut access: WorldAccessMut) {
        for i in -50..50 {
            let pos = WorldPos { x: i, y: i, z: 0 };
            let (cc, _) = pos.split();
            access.ensure_chunk(cc);
        }
    }

    fn setup_system(mut access: WorldAccessMut) {
        // Write 100 tiles over multiple chunks including negative coords
        for i in -50..50 {
            let pos = WorldPos { x: i, y: i, z: 0 };
            access.set_tile(
                pos,
                Tile {
                    material: MaterialId(42),
                },
            );
        }
    }

    fn verify_system(access: WorldAccess) {
        for i in -50..50 {
            let pos = WorldPos { x: i, y: i, z: 0 };
            let tile = access.get_tile(pos).expect("Tile should exist");
            assert_eq!(tile.material, MaterialId(42));
        }

        // Verify that the chunks are marked as dirty
        let pos = WorldPos {
            x: -50,
            y: -50,
            z: 0,
        };
        let (cc, _) = pos.split();
        let chunk = access.get_chunk(cc).expect("Chunk should exist");
        assert!(chunk.dirty);
    }

    #[test]
    fn test_world_integration() {
        let mut app = App::new();
        app.init_resource::<World>();

        // Pre-create chunks because of deferred spawn caveat
        app.add_systems(PreStartup, ensure_chunks_system);
        // Setup runs in Startup (after PreStartup commands applied)
        app.add_systems(Startup, setup_system);
        // Verify runs in Update
        app.add_systems(Update, verify_system);

        // This will run PreStartup, apply commands, Startup, apply commands, then Update
        app.update();
    }

    fn caveat_system(mut access: WorldAccessMut) {
        let pos = WorldPos {
            x: 1000,
            y: 1000,
            z: 1000,
        };
        // Trying to set a tile in a non-existent chunk
        access.set_tile(
            pos,
            Tile {
                material: MaterialId(99),
            },
        );
        // Reading it immediately in the same system fails because the ChunkData is not spawned yet!
        assert!(access.get_chunk(pos.split().0).is_none());
    }

    #[test]
    fn test_deferred_spawn_caveat() {
        let mut app = App::new();
        app.init_resource::<World>();
        app.add_systems(Update, caveat_system);
        app.update();
        // After the update, commands have been applied, so the chunk NOW exists
        // but the tile modification from set_tile was lost due to the caveat.
        let pos = WorldPos {
            x: 1000,
            y: 1000,
            z: 1000,
        };
        let world = app.world().resource::<World>();
        let entity = world.chunks.get(&pos.split().0).unwrap();
        let chunk_data = app.world().get::<ChunkData>(*entity).unwrap();
        // Still MAT_AIR (0), not 99!
        assert_eq!(
            chunk_data.terrain[pos.split().1.index()],
            tile_core::material::MAT_AIR
        );
    }
}
