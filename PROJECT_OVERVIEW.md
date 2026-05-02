# Project Overview

## What it is?
A Bevy-based 3D tile/voxel simulation engine emphasizing fluid dynamics, thermodynamics, and reactions.

## Project Stats
- Language: Rust
- Framework: Bevy

## Architectural Decisions
- 4 Layers: Core -> Worldgen -> Sim -> Renderer.
- No float in Sim state.
- Data-driven behaviors (RON registries).

## Detailed Architecture
Check `ARCHITEKTUR.md` for full truth.
`crates/` contains layer crates (`core`, `sim-*`, `worldgen-*`, `render-bevy`, `app`).

## Source Files Description
- `crates/core`: Base coordinates, definitions.
- `crates/app`: Binary entry point.
- `PLAN.md`: Implementation phases.

## Dependencies
- Bevy: Core ECS / rendering.
- Serde/Bincode: Save/Load.

## Additional References
- ARCHITEKTUR.md
- PLAN.md
- CLAUDE.md
