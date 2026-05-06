use std::collections::{HashMap, VecDeque};

use bevy::diagnostic::{
    DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, egui};
use render_bevy::ActiveZLayer;
use sim_fluid::LiquidRegistry;
use sim_reaction::resolver::PendingEffects;
use tile_core::activity::WakeRequests;
use tile_core::chunk::ChunkData;
use tile_core::liquid::{LIQ_NONE, LiquidId};

const HISTORY_LEN: usize = 60;

/// Rolling reaction-rate history (one entry per tick, last 60).
#[derive(Resource, Default)]
struct ReactionHistory(VecDeque<usize>);

pub struct DebugUiPlugin;

impl Plugin for DebugUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin)
            .add_plugins(FrameTimeDiagnosticsPlugin)
            .add_plugins(EntityCountDiagnosticsPlugin)
            .init_resource::<ReactionHistory>()
            .add_systems(Update, update_reaction_history)
            .add_systems(Update, debug_window.after(update_reaction_history));
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
        });
}
