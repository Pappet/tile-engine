use bevy::diagnostic::{DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};

use crate::ActiveZLayer;

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

    egui::Window::new("Debug")
        .resizable(false)
        .show(contexts.ctx_mut(), |ui| {
            ui.heading("Renderer");
            ui.label(format!("Z Layer:  {} (Q / E)", active_z.0));
            ui.separator();

            ui.heading("Performance");
            ui.label(format!("FPS:       {fps:.1}"));
            ui.label(format!("Frame:     {frame_ms:.2} ms"));
            ui.separator();

            ui.heading("World");
            ui.label(format!("Chunks:    {}", world.chunks.len()));
            ui.label(format!("Entities:  {entity_count}"));
            ui.label(format!("Tick:      {}", world.current_tick));
        });
}
