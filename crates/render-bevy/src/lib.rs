use std::collections::HashSet;

use bevy::prelude::*;
use sim_fluid::LiquidRegistry;
use tile_core::chunk::ChunkData;
use tile_core::coords::{CHUNK_SIZE, ChunkCoord, WorldPos};
use tile_core::liquid::LIQ_NONE;
use tile_core::material::{MAT_AIR, MaterialRegistry};
use tile_core::world::World;

pub mod camera;

#[derive(Resource)]
pub struct ActiveZLayer(pub i32);

impl Default for ActiveZLayer {
    fn default() -> Self {
        Self(2)
    }
}

#[derive(Component)]
pub struct ChunkVisuals {
    pub tiles: Vec<Entity>,
}

/// The tile currently under the mouse cursor (updated every frame). `None` when cursor is
/// outside the window or over a position with no chunk.
#[derive(Resource, Default)]
pub struct HoveredTile(pub Option<WorldPos>);

#[derive(Component)]
struct HoverSprite;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveZLayer>();
        app.init_resource::<HoveredTile>();
        app.add_systems(Startup, (camera::setup_camera, spawn_hover_sprite));
        app.add_systems(Update, camera::camera_control_system);
        app.add_systems(Update, handle_z_layer_change);
        app.add_systems(
            Update,
            hover_tile_system.after(camera::camera_control_system),
        );
        app.add_systems(Update, render_chunks_system.after(handle_z_layer_change));
    }
}

fn spawn_hover_sprite(mut commands: Commands) {
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::srgba(1.0, 1.0, 1.0, 0.0), // hidden until cursor moves
                custom_size: Some(Vec2::new(16.0, 16.0)),
                ..default()
            },
            transform: Transform::from_xyz(0.0, 0.0, 2.0),
            ..default()
        },
        HoverSprite,
    ));
}

fn hover_tile_system(
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<camera::TileCamera>>,
    active_z: Res<ActiveZLayer>,
    world: Res<World>,
    mut hovered: ResMut<HoveredTile>,
    mut hover_sprites: Query<(&mut Transform, &mut Sprite), With<HoverSprite>>,
) {
    let Ok((mut transform, mut sprite)) = hover_sprites.get_single_mut() else {
        return;
    };
    let Ok(window) = windows.get_single() else {
        return;
    };
    let Ok((camera, camera_transform)) = cameras.get_single() else {
        return;
    };

    let Some(cursor_pos) = window.cursor_position() else {
        hovered.0 = None;
        sprite.color = Color::srgba(1.0, 1.0, 1.0, 0.0);
        return;
    };

    if let Some(wp) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
        // Tiles are center-anchored sprites: tile at (tx, ty) covers
        // [tx*16-8, tx*16+8] in world space, so tile = floor((w+8)/16).
        let tile_x = ((wp.x + 8.0) / 16.0).floor() as i32;
        let tile_y = ((wp.y + 8.0) / 16.0).floor() as i32;
        let world_pos = WorldPos {
            x: tile_x,
            y: tile_y,
            z: active_z.0,
        };
        let (coord, _) = world_pos.split();

        if world.chunks.contains_key(&coord) {
            hovered.0 = Some(world_pos);
            transform.translation.x = tile_x as f32 * 16.0;
            transform.translation.y = tile_y as f32 * 16.0;
            sprite.color = Color::srgba(1.0, 1.0, 1.0, 0.20);
        } else {
            hovered.0 = None;
            sprite.color = Color::srgba(1.0, 1.0, 1.0, 0.0);
        }
    } else {
        hovered.0 = None;
        sprite.color = Color::srgba(1.0, 1.0, 1.0, 0.0);
    }
}

// Depth peeking: dim factors and atmospheric tint per level below active Z.
const DEPTH_DIM_1: f32 = 0.55;
const DEPTH_DIM_2: f32 = 0.30;

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

#[allow(clippy::too_many_arguments)]
fn render_chunks_system(
    mut commands: Commands,
    active_z: Res<ActiveZLayer>,
    material_reg: Res<MaterialRegistry>,
    liquid_reg: Option<Res<LiquidRegistry>>,
    world: Res<World>,
    new_chunks: Query<(Entity, &ChunkData), Without<ChunkVisuals>>,
    all_chunk_data: Query<&ChunkData>,
    all_with_visuals: Query<(&ChunkData, &ChunkVisuals)>,
    changed_any: Query<&ChunkData, Changed<ChunkData>>,
    mut sprites: Query<&mut Sprite>,
) {
    // 1. Spawn visuals for newly visible chunks.
    for (entity, chunk) in new_chunks.iter() {
        if chunk.coord.cz != active_z.0 {
            continue;
        }

        let offset_x = (chunk.coord.cx * CHUNK_SIZE as i32) as f32 * 16.0;
        let offset_y = (chunk.coord.cy * CHUNK_SIZE as i32) as f32 * 16.0;
        let mut tiles = Vec::with_capacity(CHUNK_SIZE * CHUNK_SIZE);
        let neighbors = ChunkNeighborhood::new(chunk, &world, &all_chunk_data);

        for ly in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let idx = ly * CHUNK_SIZE + lx;
                let color = tile_color(idx, &neighbors, &material_reg, &liquid_reg);
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

    // 2. Collect which active-z entities need a visual refresh.
    //    - A chunk at active_z changed directly.
    //    - A chunk at active_z - 1 or - 2 changed (depth-peek content changed).
    let mut needs_update: HashSet<Entity> = HashSet::new();

    for chunk in changed_any.iter() {
        let depth = active_z.0 - chunk.coord.cz;
        let target_coord = if depth == 0 {
            chunk.coord
        } else if depth == 1 || depth == 2 {
            ChunkCoord {
                cx: chunk.coord.cx,
                cy: chunk.coord.cy,
                cz: active_z.0,
            }
        } else {
            continue;
        };

        // Flag chunk + 4 cardinal neighbors so cross-chunk light glow redraws correctly.
        for (dcx, dcy) in [(0i32, 0i32), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            let neighbor = ChunkCoord {
                cx: target_coord.cx + dcx,
                cy: target_coord.cy + dcy,
                cz: target_coord.cz,
            };
            if let Some(&entity) = world.chunks.get(&neighbor) {
                needs_update.insert(entity);
            }
        }
    }

    // 3. Redraw all flagged chunks.
    for entity in needs_update {
        let Ok((chunk, visuals)) = all_with_visuals.get(entity) else {
            continue;
        };
        let neighbors = ChunkNeighborhood::new(chunk, &world, &all_chunk_data);
        for (idx, &tile_entity) in visuals.tiles.iter().enumerate() {
            if let Ok(mut sprite) = sprites.get_mut(tile_entity) {
                sprite.color = tile_color(idx, &neighbors, &material_reg, &liquid_reg);
            }
        }
    }
}

struct ChunkNeighborhood<'a> {
    center: &'a ChunkData,
    north: Option<&'a ChunkData>,
    south: Option<&'a ChunkData>,
    east: Option<&'a ChunkData>,
    west: Option<&'a ChunkData>,
    below1: Option<&'a ChunkData>,
    below2: Option<&'a ChunkData>,
}

impl<'a> ChunkNeighborhood<'a> {
    fn new(center: &'a ChunkData, world: &World, all_chunks: &'a Query<&ChunkData>) -> Self {
        let fetch = |dx: i32, dy: i32, dz: i32| {
            let coord = ChunkCoord {
                cx: center.coord.cx + dx,
                cy: center.coord.cy + dy,
                cz: center.coord.cz + dz,
            };
            world
                .chunks
                .get(&coord)
                .and_then(|&e| all_chunks.get(e).ok())
        };

        Self {
            center,
            north: fetch(0, 1, 0),
            south: fetch(0, -1, 0),
            east: fetch(1, 0, 0),
            west: fetch(-1, 0, 0),
            below1: fetch(0, 0, -1),
            below2: fetch(0, 0, -2),
        }
    }

    fn get_chunk(&self, coord: ChunkCoord) -> Option<&'a ChunkData> {
        if coord == self.center.coord {
            return Some(self.center);
        }
        let dx = coord.cx - self.center.coord.cx;
        let dy = coord.cy - self.center.coord.cy;
        let dz = coord.cz - self.center.coord.cz;

        match (dx, dy, dz) {
            (0, 1, 0) => self.north,
            (0, -1, 0) => self.south,
            (1, 0, 0) => self.east,
            (-1, 0, 0) => self.west,
            (0, 0, -1) => self.below1,
            (0, 0, -2) => self.below2,
            _ => None,
        }
    }
}

/// Flat RGBA for a tile ignoring the active-layer liquid compositing.
/// Used for depth-peek lookups on sub-layers. Returns `None` for air + no liquid.
fn flat_tile_rgba(
    idx: usize,
    chunk: &ChunkData,
    material_reg: &MaterialRegistry,
    liquid_reg: &Option<Res<LiquidRegistry>>,
) -> Option<[u8; 4]> {
    let liq = chunk.liquid_kind[idx];
    if liq != LIQ_NONE
        && chunk.liquid_amount_read[idx] > 0
        && let Some(props) = liquid_reg.as_ref().and_then(|r| r.get(liq))
    {
        let [r, g, b, a] = props.color;
        if props.emits_light > 0 {
            let boost = 1.0 + props.emits_light as f32 / 255.0;
            return Some([
                (r as f32 * boost).min(255.0) as u8,
                (g as f32 * boost).min(255.0) as u8,
                (b as f32 * boost).min(255.0) as u8,
                a,
            ]);
        }
        return Some([r, g, b, a]);
    }

    let mat = chunk.terrain[idx];
    if mat == MAT_AIR {
        return None;
    }
    if let Some(m) = material_reg.get(mat) {
        return Some(m.display_color);
    }
    Some([255, 0, 255, 255])
}

/// Dim and blue-tint a colour by depth level. Each level: ×dim factor, −10 on R/G, +5 on B.
fn apply_depth_tint([r, g, b, a]: [u8; 4], dim: f32, levels: u32) -> [u8; 4] {
    let atm = levels as f32;
    let r = (r as f32 * dim - 10.0 * atm).clamp(0.0, 255.0) as u8;
    let g = (g as f32 * dim - 10.0 * atm).clamp(0.0, 255.0) as u8;
    let b = (b as f32 * dim + 5.0 * atm).clamp(0.0, 255.0) as u8;
    [r, g, b, a]
}

/// Background RGBA: solid terrain colour, or depth-peeked Z-1/Z-2 content for AIR tiles.
/// Does not include the current tile's liquid.
fn background_rgba(
    idx: usize,
    neighbors: &ChunkNeighborhood,
    material_reg: &MaterialRegistry,
    liquid_reg: &Option<Res<LiquidRegistry>>,
) -> [u8; 4] {
    let mat = neighbors.center.terrain[idx];
    if mat != MAT_AIR {
        if let Some(m) = material_reg.get(mat) {
            return m.display_color;
        }
        return [255, 0, 255, 255];
    }

    for depth_level in 1u32..=2 {
        let below = ChunkCoord {
            cx: neighbors.center.coord.cx,
            cy: neighbors.center.coord.cy,
            cz: neighbors.center.coord.cz - depth_level as i32,
        };
        if let Some(below_chunk) = neighbors.get_chunk(below)
            && let Some(rgba) = flat_tile_rgba(idx, below_chunk, material_reg, liquid_reg)
        {
            let dim = if depth_level == 1 {
                DEPTH_DIM_1
            } else {
                DEPTH_DIM_2
            };
            return apply_depth_tint(rgba, dim, depth_level);
        }
    }

    [0, 0, 0, 0]
}

/// Additive light contribution from emissive liquid sources within 3 tiles (4-directional).
/// Returns `[r, g, b]` glow to add to the tile colour. Only called for AIR-terrain tiles.
fn compute_light_glow(
    idx: usize,
    neighbors: &ChunkNeighborhood,
    liquid_reg: &Option<Res<LiquidRegistry>>,
) -> [u8; 3] {
    let lx = (idx % CHUNK_SIZE) as i32;
    let ly = (idx / CHUNK_SIZE) as i32;
    let wx = neighbors.center.coord.cx * CHUNK_SIZE as i32 + lx;
    let wy = neighbors.center.coord.cy * CHUNK_SIZE as i32 + ly;
    let wz = neighbors.center.coord.cz;

    let mut gr = 0.0f32;
    let mut gg = 0.0f32;
    let mut gb = 0.0f32;

    for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
        for dist in 1i32..=3 {
            let nx = wx + dx * dist;
            let ny = wy + dy * dist;
            let ncx = nx.div_euclid(CHUNK_SIZE as i32);
            let ncy = ny.div_euclid(CHUNK_SIZE as i32);
            let nlx = nx.rem_euclid(CHUNK_SIZE as i32) as usize;
            let nly = ny.rem_euclid(CHUNK_SIZE as i32) as usize;
            let nidx = nly * CHUNK_SIZE + nlx;
            let ncoord = ChunkCoord {
                cx: ncx,
                cy: ncy,
                cz: wz,
            };

            if let Some(nc) = neighbors.get_chunk(ncoord)
                && nc.liquid_kind[nidx] != LIQ_NONE
                && nc.liquid_amount_read[nidx] > 0
                && let Some(props) = liquid_reg
                    .as_ref()
                    .and_then(|r| r.get(nc.liquid_kind[nidx]))
                && props.emits_light > 0
            {
                let intensity = props.emits_light as f32 / 255.0 * 0.6f32.powi(dist);
                let [lr, lg, lb, _] = props.color;
                gr += lr as f32 * intensity;
                gg += lg as f32 * intensity;
                gb += lb as f32 * intensity;
            }
        }
    }

    [
        gr.min(255.0) as u8,
        gg.min(255.0) as u8,
        gb.min(255.0) as u8,
    ]
}

/// Final tile colour: liquid composited over background with amount-driven alpha, pressure glow,
/// and additive light contribution from nearby emissive sources.
fn tile_color(
    idx: usize,
    neighbors: &ChunkNeighborhood,
    material_reg: &MaterialRegistry,
    liquid_reg: &Option<Res<LiquidRegistry>>,
) -> Color {
    let liq = neighbors.center.liquid_kind[idx];
    let amount = neighbors.center.liquid_amount_read[idx];

    if liq != LIQ_NONE
        && amount > 0
        && let Some(props) = liquid_reg.as_ref().and_then(|r| r.get(liq))
    {
        let [lr, lg, lb, _] = props.color;
        let pressure_boost = 1.0 + neighbors.center.pressure_read[idx] as f32 / 255.0 * 0.4;

        if props.emits_light > 0 {
            // Emissive source: fully opaque, IS the light — no glow added to self.
            let boost = (1.0 + props.emits_light as f32 / 255.0) * pressure_boost;
            return Color::srgba_u8(
                (lr as f32 * boost).min(255.0) as u8,
                (lg as f32 * boost).min(255.0) as u8,
                (lb as f32 * boost).min(255.0) as u8,
                255,
            );
        }

        // Non-emissive liquid: amount-alpha composite over background, then receive glow.
        let alpha = (amount as f32 / 255.0 * 0.75 + 0.25).min(1.0);
        let lr = (lr as f32 * pressure_boost).min(255.0);
        let lg = (lg as f32 * pressure_boost).min(255.0);
        let lb = (lb as f32 * pressure_boost).min(255.0);
        let [bg_r, bg_g, bg_b, _] = background_rgba(idx, neighbors, material_reg, liquid_reg);
        let inv = 1.0 - alpha;
        let [gr, gg, gb] = compute_light_glow(idx, neighbors, liquid_reg);
        return Color::srgba_u8(
            (lr * alpha + bg_r as f32 * inv + gr as f32).min(255.0) as u8,
            (lg * alpha + bg_g as f32 * inv + gg as f32).min(255.0) as u8,
            (lb * alpha + bg_b as f32 * inv + gb as f32).min(255.0) as u8,
            255,
        );
    }

    // No liquid: background (terrain or depth peek). Add glow for air tiles.
    let [mut r, mut g, mut b, a] = background_rgba(idx, neighbors, material_reg, liquid_reg);
    if neighbors.center.terrain[idx] == MAT_AIR {
        let [gr, gg, gb] = compute_light_glow(idx, neighbors, liquid_reg);
        r = (r as u16 + gr as u16).min(255) as u8;
        g = (g as u16 + gg as u16).min(255) as u8;
        b = (b as u16 + gb as u16).min(255) as u8;
    }
    Color::srgba_u8(r, g, b, a)
}
