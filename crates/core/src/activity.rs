use bitflags::bitflags;
use bevy_ecs::prelude::*;
use bevy_app::prelude::*;
use crate::coords::ChunkCoord;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct SystemMask: u16 {
        const FLUID       = 1 << 0;
        const TEMPERATURE = 1 << 1;
        const EROSION     = 1 << 2;
        const REACTION    = 1 << 3;
        const GAS         = 1 << 4;
    }
}

/// Stub resource for propagating wake events to neighbor chunks
#[derive(Resource, Default)]
pub struct WakeRequests {
    // For Phase 1, just a stub
    pub pending: Vec<ChunkCoord>,
}

const ACTIVITY_DECAY_TICKS: u64 = 100;

fn tick_counter_system(mut world: ResMut<crate::world::World>) {
    world.current_tick += 1;
}

fn activity_decay_system(
    world: Res<crate::world::World>,
    mut chunks: Query<&mut crate::chunk::ChunkData>,
) {
    let current_tick = world.current_tick;
    for mut chunk in chunks.iter_mut() {
        if !chunk.active.is_empty() {
            if current_tick >= chunk.last_active_tick + ACTIVITY_DECAY_TICKS {
                chunk.active = SystemMask::empty();
            }
        }
    }
}

pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WakeRequests>();
        
        // We register the activity systems. In a full app, these would run in the PostTick schedule.
        // For now, we just add them to the main Update schedule.
        app.add_systems(Update, (tick_counter_system, activity_decay_system).chain());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::ChunkData;
    use crate::material::MAT_AIR;

    #[test]
    fn test_activity_decay() {
        let mut app = App::new();
        app.init_resource::<crate::world::World>();
        app.add_plugins(CorePlugin);

        let mut chunk = ChunkData::new_filled(ChunkCoord { cx: 0, cy: 0, cz: 0 }, MAT_AIR);
        chunk.active = SystemMask::FLUID;
        chunk.last_active_tick = 0;
        let entity = app.world_mut().spawn(chunk).id();

        // Run 50 ticks -> should still be active
        for _ in 0..50 {
            app.update();
        }

        let chunk = app.world().get::<ChunkData>(entity).unwrap();
        assert_eq!(chunk.active, SystemMask::FLUID);

        // Run another 50 ticks -> should decay
        for _ in 0..50 {
            app.update();
        }

        let chunk = app.world().get::<ChunkData>(entity).unwrap();
        assert_eq!(chunk.active, SystemMask::empty());
        assert_eq!(app.world().resource::<crate::world::World>().current_tick, 100);
    }
}
