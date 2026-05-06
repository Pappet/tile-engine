use bevy::diagnostic::{
    DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, egui};
use render_bevy::ActiveZLayer;
use sim_reaction::resolver::PendingEffects;
use tile_core::activity::WakeRequests;

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
    pending_effects: Option<Res<PendingEffects>>,
    wake_requests: Option<Res<WakeRequests>>,
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
        .default_width(200.0)
        .show(contexts.ctx_mut(), |ui| {
            ui.collapsing("Performance", |ui| {
                ui.label(format!("FPS:         {fps:.1}"));
                ui.label(format!("Frame:       {frame_ms:.2} ms"));
            });

            ui.collapsing("World", |ui| {
                ui.label(format!("Tick:        {}", world.current_tick));
                ui.label(format!("Z Layer:     {}", active_z.0));
                ui.label(format!("Chunks:      {}", world.chunks.len()));
                ui.label(format!("Entities:    {entity_count}"));
            });

            ui.collapsing("Sim Internals", |ui| {
                let pending_react = pending_effects.map(|p| p.len()).unwrap_or(0);
                let pending_wake = wake_requests.map(|w| w.pending.len()).unwrap_or(0);
                ui.label(format!("Pending React: {pending_react}"));
                ui.label(format!("Pending Wake:  {pending_wake}"));
            });
        });
}
