# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Status

Pre-implementation. Only `ARCHITEKTUR.md` (architecture spec / "Bibel") and `PLAN.md` (work packages) exist. The workspace and all crates are yet to be created starting from P0.1.

## Build / Test Commands

Once the workspace exists (P0.1+):

```sh
cargo build --workspace
cargo test --workspace
cargo test -p core                     # single crate
cargo test -p core coords              # single test module
cargo run -p app
cargo run -p app --features profile    # with puffin/tracy profiling
```

## Planned Workspace Layout

```
tile-engine/
├── crates/
│   ├── core/            # data model, coords, World-API
│   ├── sim-fluid/       # fluid CA, LiquidRegistry
│   ├── sim-reaction/    # ReactionRegistry, resolver
│   ├── sim-thermal/     # heat diffusion (later)
│   ├── worldgen-api/    # Stage traits and resources
│   ├── worldgen-earthlike/
│   ├── render-bevy/     # renderer plugin
│   ├── persistence/     # save/load
│   └── app/             # binary, assembles plugins
└── data/                # RON/TOML definitions loaded at runtime
    ├── materials.ron
    ├── liquids.ron
    └── reactions.ron
```

## Architecture (4 Layers)

From bottom to top — each layer only knows the layer directly below it:

1. **Core** (`crates/core`) — `ChunkData`, `MaterialId`, `LiquidId`, `ReactionId`, coordinate types. The only layer all others may depend on.
2. **Worldgen** — writes `ChunkData` at startup. Replaceable as a Bevy plugin. Sim never sees where chunks come from.
3. **Sim** — reads/writes `ChunkData`. Sub-plugins: `FluidPlugin`, reaction system, erosion. Knows nothing about Worldgen or Renderer.
4. **Renderer** (`crates/render-bevy`) — reads `ChunkData`, `MaterialRegistry`, `LiquidRegistry`. Never touches Sim internals.

## Core Data Model

- **Tile** = one voxel (x, y, z). Not an ECS Entity. Lives as an index inside a `ChunkData` array.
- **Chunk** = 32×32 tiles at a single Z-level (`CHUNK_SIZE = 32`, `CHUNK_AREA = 1024`). One ECS Entity with `ChunkData` component. SoA layout inside (`Box<[T; 1024]>` per field).
- **Entity** = anything with individual state: creatures, pumps, `LiquidSource`, `LiquidDrain`.
- `World` resource holds `HashMap<ChunkCoord, Entity>` + `current_tick: u64`.
- `WorldAccess` / `WorldAccessMut` are Bevy `SystemParam`s for ergonomic tile access.

Coordinate types: `WorldPos`, `ChunkCoord`, `LocalPos`. Split via bit-shift (`>> 5`, `& 31`). Negative coordinates must work — use the bit-shift approach, not division. Indexing: `ly * CHUNK_SIZE + lx`.

## Key Invariants

These must never be violated:

1. **`ARCHITEKTUR.md` is truth.** On conflict between plan and Bibel, Bibel wins.
2. **No `thread_rng()` in Sim or Worldgen.** Use seeded deterministic RNG derived via `mix_hash`.
3. **No `f32` in Sim state.** Use `i16`/`i32`/fixed-point. Floats are only allowed in Worldgen maps (which are saved, not regenerated).
4. **All new fields are immediately classified:** authoritative (serialize) or derived (`#[serde(skip)]`). Write-buffers (double-buffering) and activity bits are always `#[serde(skip)]`.
5. **Behaviours as data, not code.** Materials, liquids, reactions are registry entries. No hardcoded special-cases per type.
6. **Save only between Sim ticks** (PostTick / before next PreTick). Never mid-`Simulate`-phase.
7. **One liquid per tile.** Collisions go through the Reaction system, not special-cased fluid logic.

## Sim Architecture

`SimSet` ordering: `PreTick → Simulate → PostTick`. Runs in Bevy `FixedUpdate`.

Cross-chunk writes use double-buffering (`_read` / `_write` per field, swap in PostTick) + `LiquidSnapshot` resource (read-only cross-chunk view). The `flow_between` formula is symmetric — both sides call it with the same args and only touch their own write-buffer.

Multi-rate ticking: Fluid every tick, reactions individually, heat every 4 ticks, erosion every 100 ticks.

## Save/Load

Binary format: `bincode` + `serde`. Save is a directory of files (`header.bin`, `world.bin`, `chunks/cx_cy_cz.bin` for dirty chunks only, `entities.bin`, etc.). Dirty chunks tracked via `chunk.dirty: bool`. Clean chunks are regenerated from deterministic Worldgen on load.

Entities use the `Persistent` trait with `TYPE_ID: u32`. Renderer components (`Sprite`, `Transform`) are never persisted.

## Implementation Phases

See `PLAN.md` for detailed work packages. Current entry point: **P0.1** (workspace setup). Each package has explicit acceptance criteria — these are mandatory. Do not exceed the defined scope; file an issue instead.

Parallelizable after P1.4: activity system (P1.6) and deferred-spawn handling (P1.5) are independent. Worldgen API (P3.1) is independent of the renderer (Phase 2).

<!-- code-review-graph MCP tools -->
## MCP Tools: code-review-graph

**IMPORTANT: This project has a knowledge graph. ALWAYS use the
code-review-graph MCP tools BEFORE using Grep/Glob/Read to explore
the codebase.** The graph is faster, cheaper (fewer tokens), and gives
you structural context (callers, dependents, test coverage) that file
scanning cannot.

### When to use graph tools FIRST

- **Exploring code**: `semantic_search_nodes` or `query_graph` instead of Grep
- **Understanding impact**: `get_impact_radius` instead of manually tracing imports
- **Code review**: `detect_changes` + `get_review_context` instead of reading entire files
- **Finding relationships**: `query_graph` with callers_of/callees_of/imports_of/tests_for
- **Architecture questions**: `get_architecture_overview` + `list_communities`

Fall back to Grep/Glob/Read **only** when the graph doesn't cover what you need.

### Key Tools

| Tool | Use when |
|------|----------|
| `detect_changes` | Reviewing code changes — gives risk-scored analysis |
| `get_review_context` | Need source snippets for review — token-efficient |
| `get_impact_radius` | Understanding blast radius of a change |
| `get_affected_flows` | Finding which execution paths are impacted |
| `query_graph` | Tracing callers, callees, imports, tests, dependencies |
| `semantic_search_nodes` | Finding functions/classes by name or keyword |
| `get_architecture_overview` | Understanding high-level codebase structure |
| `refactor_tool` | Planning renames, finding dead code |

### Workflow

1. The graph auto-updates on file changes (via hooks).
2. Use `detect_changes` for code review.
3. Use `get_affected_flows` to understand impact.
4. Use `query_graph` pattern="tests_for" to check coverage.
