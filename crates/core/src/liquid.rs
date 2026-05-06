use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::coords::ChunkCoord;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LiquidId(pub u16);

pub const LIQ_NONE: LiquidId = LiquidId(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GasId(pub u16);

/// Emitted by the fluid system when two different liquid kinds are adjacent
/// (Bibel §9.4). Resolved by `liquid_collision_reaction_system` before flow.
#[derive(Event, Debug, Clone, Copy)]
pub struct LiquidCollisionEvent {
    pub coord: ChunkCoord,
    pub idx: usize,
    /// Liquid already in the tile.
    pub existing: LiquidId,
    /// Liquid in the adjacent tile pressing in.
    pub incoming: LiquidId,
}
