use bevy::prelude::*;
use sim_fluid::LiquidRegistry;
use tile_core::chunk::ChunkData;
use tile_core::coords::CHUNK_SIZE;
use tile_core::liquid::LIQ_NONE;
use tile_core::material::{MAT_AIR, MaterialRegistry};

pub mod camera;
pub mod debug;

#[derive(Resource, Default)]
pub struct ActiveZLayer(pub i32);

#[derive(Component)]
pub struct ChunkVisuals {
    pub tiles: Vec<Entity>,
}

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(debug::DebugUiPlugin);
        app.init_resource::<ActiveZLayer>();
        app.add_systems(Startup, camera::setup_camera);
        app.add_systems(Update, camera::camera_control_system);
        app.add_systems(Update, handle_z_layer_change);
        app.add_systems(Update, render_chunks_system.after(handle_z_layer_change));
    }
}

fn handle_z_layer_change(
    mut commands: Commands,
    active_z: Res<ActiveZLayer>,
    mut visuals: Query<(Entity, &mut ChunkVisuals)>,
) {
    if active_z.is_changed() {
        for (entity, visuals) in visuals.iter_mut() {
            for &tile_entity in &visuals.tiles {
                commands.entity(tile_entity).despawn();
            }
            commands.entity(entity).remove::<ChunkVisuals>();
        }
    }
}

fn render_chunks_system(
    mut commands: Commands,
    active_z: Res<ActiveZLayer>,
    material_reg: Res<MaterialRegistry>,
    liquid_reg: Option<Res<LiquidRegistry>>,
    mut chunks: Query<(Entity, &ChunkData), Without<ChunkVisuals>>,
    changed_chunks: Query<(&ChunkData, &ChunkVisuals), Changed<ChunkData>>,
    mut sprites: Query<&mut Sprite>,
) {
    // 1. Spawn visuals for newly visible chunks
    for (entity, chunk) in chunks.iter_mut() {
        if chunk.coord.cz != active_z.0 {
            continue;
        }

        let mut tiles = Vec::with_capacity(chunk.terrain.len());

        // 16.0 pixel per tile.
        let offset_x = (chunk.coord.cx * CHUNK_SIZE as i32) as f32 * 16.0;
        let offset_y = (chunk.coord.cy * CHUNK_SIZE as i32) as f32 * 16.0;

        for ly in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let idx = ly * CHUNK_SIZE + lx;
                let color = tile_color(idx, chunk, &material_reg, &liquid_reg);

                let x = offset_x + (lx as f32) * 16.0;
                let y = offset_y + (ly as f32) * 16.0;

                let tile_entity = commands
                    .spawn(SpriteBundle {
                        sprite: Sprite {
                            color,
                            custom_size: Some(Vec2::new(16.0, 16.0)),
                            ..default()
                        },
                        transform: Transform::from_xyz(x, y, 0.0),
                        ..default()
                    })
                    .id();

                tiles.push(tile_entity);
            }
        }

        commands.entity(entity).insert(ChunkVisuals { tiles });
    }

    // 2. Update existing visuals if ChunkData changed
    for (chunk, visuals) in changed_chunks.iter() {
        if chunk.coord.cz != active_z.0 {
            continue;
        }
        for (idx, &tile_entity) in visuals.tiles.iter().enumerate() {
            if let Ok(mut sprite) = sprites.get_mut(tile_entity) {
                sprite.color = tile_color(idx, chunk, &material_reg, &liquid_reg);
            }
        }
    }
}

/// Determine the display color for a tile.
///
/// Priority: liquid (if present) > terrain. Liquid with emits_light > 0
/// gets its RGB brightened proportional to the emits_light value (Bibel §9.8).
fn tile_color(
    idx: usize,
    chunk: &ChunkData,
    material_reg: &MaterialRegistry,
    liquid_reg: &Option<Res<LiquidRegistry>>,
) -> Color {
    let liquid_kind = chunk.liquid_kind[idx];
    if liquid_kind != LIQ_NONE
        && chunk.liquid_amount_read[idx] > 0
        && let Some(props) = liquid_reg.as_ref().and_then(|r| r.get(liquid_kind))
    {
        let [r, g, b, a] = props.color;
        if props.emits_light > 0 {
            let boost = 1.0 + (props.emits_light as f32 / 255.0);
            return Color::srgba(
                (r as f32 / 255.0 * boost).min(1.0),
                (g as f32 / 255.0 * boost).min(1.0),
                (b as f32 / 255.0 * boost).min(1.0),
                a as f32 / 255.0,
            );
        }
        return Color::srgba_u8(r, g, b, a);
    }

    let mat_id = chunk.terrain[idx];
    if mat_id == MAT_AIR {
        Color::srgba_u8(0, 0, 0, 0)
    } else if let Some(mat) = material_reg.get(mat_id) {
        let [r, g, b, a] = mat.display_color;
        Color::srgba_u8(r, g, b, a)
    } else {
        Color::srgba_u8(255, 0, 255, 255)
    }
}
