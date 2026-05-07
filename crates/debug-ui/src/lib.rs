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

pub struct DebugUiPlugin;

impl Plugin for DebugUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin)
            .add_plugins(FrameTimeDiagnosticsPlugin)
            .add_plugins(EntityCountDiagnosticsPlugin)
            .init_resource::<ReactionHistory>()
            .init_resource::<InspectedTile>()
            .add_systems(Update, update_reaction_history)
            .add_systems(Update, tile_click_system)
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
