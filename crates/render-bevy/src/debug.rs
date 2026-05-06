use bevy::diagnostic::{
    DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
    SystemInformationDiagnosticsPlugin,
};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, egui};

use crate::ActiveZLayer;
use sim_reaction::resolver::PendingEffects;
use tile_core::activity::{SystemMask, WakeRequests};
use tile_core::chunk::ChunkData;

pub struct DebugUiPlugin;

impl Plugin for DebugUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin)
            .add_plugins(FrameTimeDiagnosticsPlugin)
            .add_plugins(EntityCountDiagnosticsPlugin)
            .add_systems(Update, debug_window);
    }
}

fn debug_window(
    mut contexts: EguiContexts,
    diagnostics: Res<DiagnosticsStore>,
    active_z: Res<ActiveZLayer>,
    world: Res<tile_core::world::World>,
    chunks: Query<&ChunkData>,
    pending_effects: Option<Res<PendingEffects>>,
    wake_requests: Option<Res<WakeRequests>>,
) {
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);

    let frame_ms = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0)
        * 1000.0;

    let entity_count = diagnostics
        .get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT)
        .and_then(|d| d.value())
        .unwrap_or(0.0) as u64;

    // Calculate world stats - only count chunks, skip tile iteration
    let mut total_active = 0;
    let mut active_fluid = 0;
    let mut active_temp = 0;
    let mut active_erosion = 0;
    let mut active_reaction = 0;
    let mut dirty_chunks = 0;

    for chunk in chunks.iter() {
        if !chunk.active.is_empty() {
            total_active += 1;
            if chunk.active.contains(SystemMask::FLUID) {
                active_fluid += 1;
            }
            if chunk.active.contains(SystemMask::TEMPERATURE) {
                active_temp += 1;
            }
            if chunk.active.contains(SystemMask::EROSION) {
                active_erosion += 1;
            }
            if chunk.active.contains(SystemMask::REACTION) {
                active_reaction += 1;
            }
        }
        if chunk.dirty {
            dirty_chunks += 1;
        }
    }

    egui::Window::new("Debug")
        .resizable(false)
        .show(contexts.ctx_mut(), |ui| {
            ui.heading("Renderer");
            ui.label(format!("Z Layer:    {} (Q / E)", active_z.0));
            ui.separator();

            ui.heading("Performance");
            ui.label(format!("FPS:         {fps:.1}"));
            ui.label(format!("Frame:       {frame_ms:.2} ms"));
            ui.separator();

            ui.heading("World");
            ui.label(format!("Tick:        {}", world.current_tick));
            ui.label(format!("Chunks:      {}", world.chunks.len()));
            ui.label(format!("  Dirty:     {dirty_chunks}"));
            ui.label(format!("  Active:    {total_active}"));
            if total_active > 0 {
                ui.label(format!("    Fluid:   {active_fluid}"));
                ui.label(format!("    Temp:    {active_temp}"));
                ui.label(format!("    Erosion: {active_erosion}"));
                ui.label(format!("    React:   {active_reaction}"));
            }
            ui.label(format!("Entities:    {entity_count}"));
            ui.separator();

            ui.heading("Sim Internals");
            let pending_react = pending_effects.map(|p| p.len()).unwrap_or(0);
            let pending_wake = wake_requests.map(|w| w.pending.len()).unwrap_or(0);
            ui.label(format!("Pending React: {pending_react}"));
            ui.label(format!("Pending Wake:  {pending_wake}"));
        });
}
