use std::collections::{HashMap, VecDeque};

use bevy::diagnostic::{
    DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, egui};
use render_bevy::ActiveZLayer;
use render_bevy::camera::TileCamera;
use sim_fluid::LiquidRegistry;
use sim_reaction::resolver::PendingEffects;
use tile_core::activity::WakeRequests;
use tile_core::chunk::ChunkData;
use tile_core::coords::ChunkCoord;
use tile_core::coords::WorldPos;
use tile_core::liquid::{LIQ_NONE, LiquidId};
use tile_core::material::{MaterialId, MaterialRegistry};

const HISTORY_LEN: usize = 60;

/// Rolling reaction-rate history (one entry per tick, last 60).
#[derive(Resource, Default)]
struct ReactionHistory(VecDeque<usize>);

/// Last tile clicked by the user for inspection.
#[derive(Resource, Default)]
pub struct InspectedTile(pub Option<WorldPos>);

/// Toggleable debug overlays drawn over the world.
#[derive(Resource, Default)]
pub struct DebugOverlays {
    pub chunk_borders: bool,
    pub activity: bool,
    pub liquid_heatmap: bool,
}

pub struct DebugUiPlugin;

impl Plugin for DebugUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin)
            .add_plugins(FrameTimeDiagnosticsPlugin)
            .add_plugins(EntityCountDiagnosticsPlugin)
            .init_resource::<ReactionHistory>()
            .init_resource::<InspectedTile>()
            .init_resource::<DebugOverlays>()
            .add_systems(Update, update_reaction_history)
            .add_systems(Update, tile_click_system)
            .add_systems(Update, toggle_overlays_system)
            .add_systems(Update, draw_overlays_system)
            .add_systems(
                Update,
                debug_window
                    .after(update_reaction_history)
                    .after(tile_click_system),
            );
    }
}

fn update_reaction_history(
    pending: Option<Res<PendingEffects>>,
    mut history: ResMut<ReactionHistory>,
) {
    let count = pending.map(|p| p.reactions_last_tick).unwrap_or(0);
    if history.0.len() >= HISTORY_LEN {
        history.0.pop_front();
    }
    history.0.push_back(count);
}

fn tile_click_system(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<TileCamera>>,
    active_z: Res<ActiveZLayer>,
    mut inspected: ResMut<InspectedTile>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok(window) = windows.get_single() else {
        return;
    };
    let Ok((camera, camera_transform)) = cameras.get_single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        inspected.0 = None;
        return;
    };
    if let Some(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
        let tile_x = ((world_pos.x + 8.0) / 16.0).floor() as i32;
        let tile_y = ((world_pos.y + 8.0) / 16.0).floor() as i32;
        inspected.0 = Some(WorldPos {
            x: tile_x,
            y: tile_y,
            z: active_z.0,
        });
    } else {
        inspected.0 = None;
    }
}

fn toggle_overlays_system(keys: Res<ButtonInput<KeyCode>>, mut overlays: ResMut<DebugOverlays>) {
    if keys.just_pressed(KeyCode::F1) {
        overlays.chunk_borders = !overlays.chunk_borders;
    }
    if keys.just_pressed(KeyCode::F2) {
        overlays.activity = !overlays.activity;
    }
    if keys.just_pressed(KeyCode::F3) {
        overlays.liquid_heatmap = !overlays.liquid_heatmap;
    }
}

fn draw_overlays_system(
    overlays: Res<DebugOverlays>,
    active_z: Res<ActiveZLayer>,
    chunks: Query<&ChunkData>,
    wake_requests: Option<Res<WakeRequests>>,
    mut gizmos: Gizmos,
) {
    if !overlays.chunk_borders && !overlays.activity && !overlays.liquid_heatmap {
        return;
    }

    let active_chunks: Vec<_> = chunks.iter().filter(|c| c.coord.cz == active_z.0).collect();

    // Chunk borders — white outline per chunk.
    if overlays.chunk_borders {
        for chunk in &active_chunks {
            draw_chunk_rect(&mut gizmos, chunk.coord, Color::srgba(1.0, 1.0, 1.0, 0.35));
        }
    }

    // Activity overlay — blue outline for chunks with pending wake requests.
    if overlays.activity
        && let Some(ref wake) = wake_requests
    {
        for &coord in &wake.pending {
            if coord.cz == active_z.0 {
                draw_chunk_rect(&mut gizmos, coord, Color::srgba(0.2, 0.5, 1.0, 0.7));
            }
        }
    }

    // Liquid heatmap — per-tile cyan outline, alpha proportional to amount.
    if overlays.liquid_heatmap {
        for chunk in &active_chunks {
            let base_x = chunk.coord.cx * tile_core::coords::CHUNK_SIZE as i32;
            let base_y = chunk.coord.cy * tile_core::coords::CHUNK_SIZE as i32;
            for (idx, &amount) in chunk.liquid_amount_read.iter().enumerate() {
                if amount == 0 {
                    continue;
                }
                let lx = (idx % tile_core::coords::CHUNK_SIZE) as i32;
                let ly = (idx / tile_core::coords::CHUNK_SIZE) as i32;
                let tx = (base_x + lx) as f32 * 16.0;
                let ty = (base_y + ly) as f32 * 16.0;
                let alpha = amount as f32 / 255.0 * 0.8;
                draw_tile_rect(&mut gizmos, tx, ty, Color::srgba(0.0, 0.85, 1.0, alpha));
            }
        }
    }
}

/// Draw a chunk-sized rectangle outline (512×512 world units).
fn draw_chunk_rect(gizmos: &mut Gizmos, coord: ChunkCoord, color: Color) {
    let x0 = coord.cx as f32 * 512.0 - 8.0;
    let x1 = x0 + 512.0;
    let y0 = coord.cy as f32 * 512.0 - 8.0;
    let y1 = y0 + 512.0;
    gizmos.line_2d(Vec2::new(x0, y0), Vec2::new(x1, y0), color);
    gizmos.line_2d(Vec2::new(x1, y0), Vec2::new(x1, y1), color);
    gizmos.line_2d(Vec2::new(x1, y1), Vec2::new(x0, y1), color);
    gizmos.line_2d(Vec2::new(x0, y1), Vec2::new(x0, y0), color);
}

/// Draw a tile-sized rectangle outline (16×16 world units, center-anchored).
fn draw_tile_rect(gizmos: &mut Gizmos, cx: f32, cy: f32, color: Color) {
    let (x0, x1) = (cx - 8.0, cx + 8.0);
    let (y0, y1) = (cy - 8.0, cy + 8.0);
    gizmos.line_2d(Vec2::new(x0, y0), Vec2::new(x1, y0), color);
    gizmos.line_2d(Vec2::new(x1, y0), Vec2::new(x1, y1), color);
    gizmos.line_2d(Vec2::new(x1, y1), Vec2::new(x0, y1), color);
    gizmos.line_2d(Vec2::new(x0, y1), Vec2::new(x0, y0), color);
}

#[allow(clippy::too_many_arguments)]
fn debug_window(
    mut contexts: EguiContexts,
    diagnostics: Res<DiagnosticsStore>,
    active_z: Res<ActiveZLayer>,
    world: Res<tile_core::world::World>,
    pending_effects: Option<Res<PendingEffects>>,
    wake_requests: Option<Res<WakeRequests>>,
    reaction_history: Res<ReactionHistory>,
    chunks: Query<&ChunkData>,
    liquid_reg: Option<Res<LiquidRegistry>>,
    inspected: Res<InspectedTile>,
    material_reg: Option<Res<MaterialRegistry>>,
    overlays: Res<DebugOverlays>,
) {
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);

    let frame_ms = if fps > 0.0 { 1000.0 / fps } else { 0.0 };

    let entity_count = diagnostics
        .get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT)
        .and_then(|d| d.value())
        .unwrap_or(0.0) as u64;

    egui::Window::new("Debug")
        .resizable(true)
        .default_width(220.0)
        .show(contexts.ctx_mut(), |ui| {
            ui.collapsing("Performance", |ui| {
                ui.label(format!("FPS:         {fps:.1}"));
                ui.label(format!("Frame:       {frame_ms:.2} ms"));
            });

            ui.collapsing("World", |ui| {
                let dirty = chunks.iter().filter(|c| c.dirty).count();
                ui.label(format!("Tick:        {}", world.current_tick));
                ui.label(format!("Z Layer:     {}", active_z.0));
                ui.label(format!("Chunks:      {}", world.chunks.len()));
                ui.label(format!("Dirty:       {dirty}"));
                ui.label(format!("Entities:    {entity_count}"));
            });

            ui.collapsing("Fluid", |ui| {
                let mut by_kind: HashMap<LiquidId, u64> = HashMap::new();
                let mut active_chunks = 0usize;
                for chunk in chunks.iter() {
                    let mut chunk_has_liq = false;
                    for (i, &amt) in chunk.liquid_amount_read.iter().enumerate() {
                        if amt > 0 {
                            chunk_has_liq = true;
                            let kind = chunk.liquid_kind[i];
                            *by_kind.entry(kind).or_insert(0) += amt as u64;
                        }
                    }
                    if chunk_has_liq {
                        active_chunks += 1;
                    }
                }
                let pending_wake = wake_requests.as_ref().map(|w| w.pending.len()).unwrap_or(0);
                ui.label(format!("Active chunks: {active_chunks}"));
                ui.label(format!("Wake queue:    {pending_wake}"));
                if by_kind.is_empty() {
                    ui.label("No liquid");
                } else {
                    let mut kinds: Vec<_> = by_kind.into_iter().collect();
                    kinds.sort_by_key(|(k, _)| k.0);
                    for (kind, total) in kinds {
                        if kind == LIQ_NONE {
                            continue;
                        }
                        let name = liquid_reg
                            .as_ref()
                            .and_then(|r| r.get(kind))
                            .map(|p| p.name.as_str())
                            .unwrap_or("?");
                        ui.label(format!("  [{:>3}] {:<8} {total}", kind.0, name));
                    }
                }
            });

            ui.collapsing("Reactions", |ui| {
                let pending_react = pending_effects.map(|p| p.len()).unwrap_or(0);
                let avg = if reaction_history.0.is_empty() {
                    0.0
                } else {
                    reaction_history.0.iter().sum::<usize>() as f64
                        / reaction_history.0.len() as f64
                };
                ui.label(format!("Pending:     {pending_react}"));
                ui.label(format!("Rate (60t):  {avg:.1}/tick"));
            });

            ui.collapsing("Overlays", |ui| {
                ui.label("F1 Chunk Borders  F2 Activity  F3 Heatmap");
                ui.label(format!(
                    "Borders:{} Activity:{} Heatmap:{}",
                    if overlays.chunk_borders { "ON " } else { "off" },
                    if overlays.activity { "ON " } else { "off" },
                    if overlays.liquid_heatmap {
                        "ON "
                    } else {
                        "off"
                    },
                ));
            });

            ui.collapsing("Inspector", |ui| {
                if let Some(pos) = inspected.0 {
                    ui.label(format!("Pos: ({}, {}, {})", pos.x, pos.y, pos.z));
                    let (coord, lp) = pos.split();
                    let idx = lp.index();
                    if let Some(&entity) = world.chunks.get(&coord) {
                        if let Ok(chunk) = chunks.get(entity) {
                            let mat_id: MaterialId = chunk.terrain[idx];
                            let mat_name = material_reg
                                .as_ref()
                                .and_then(|r| r.get(mat_id))
                                .map(|m| m.name.as_str())
                                .unwrap_or("?");
                            ui.label(format!("Terrain:    {:>3} \"{}\"", mat_id.0, mat_name));

                            let liq_id = chunk.liquid_kind[idx];
                            if liq_id == LIQ_NONE || chunk.liquid_amount_read[idx] == 0 {
                                ui.label("Liquid:     none");
                            } else {
                                let liq_name = liquid_reg
                                    .as_ref()
                                    .and_then(|r| r.get(liq_id))
                                    .map(|p| p.name.as_str())
                                    .unwrap_or("?");
                                ui.label(format!("Liquid:     {:>3} \"{}\"", liq_id.0, liq_name));
                                ui.label(format!("Amount:     {}", chunk.liquid_amount_read[idx]));
                                ui.label(format!("Liq temp:   {}", chunk.liquid_temp_read[idx]));
                            }
                            ui.label(format!("Pressure:   {}", chunk.pressure_read[idx]));
                            ui.label(format!("Tile temp:  {}", chunk.temp[idx]));
                            ui.label(format!("Dirty:      {}", chunk.dirty));
                        } else {
                            ui.label("(chunk entity not found)");
                        }
                    } else {
                        ui.label("(no chunk at position)");
                    }
                } else {
                    ui.label("Click a tile to inspect.");
                }
            });
        });
}
