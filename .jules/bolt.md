## 2024-05-20 - Cache Chunk-Level Aggregates in Snapshot
**Learning:** Checking if a chunk has any liquid by iterating over all tiles repeatedly (`iter().any()`) inside multi-pass cellular automata systems (`pressure_propagation`, `fluid_step_local`, `liquid_vertical_flow`) is inefficient, as chunk data is frozen per tick.
**Action:** Derived chunk-level aggregates (like `chunk_has_liquid`) should be cached in the `LiquidSnapshot` during the PreTick phase rather than being re-calculated repeatedly on the fly.
