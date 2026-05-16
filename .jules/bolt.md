## 2025-02-12 - Cached chunk liquid status
**Learning:** In the voxel simulation (like `sim-fluid`), derived chunk-level aggregates (like checking if a chunk contains any liquid, which requires iterating over 1024 tiles) are highly redundant when queried repeatedly inside multi-pass cellular automata systems (`pressure_propagation`, `fluid_step_local`, `liquid_vertical_flow`).
**Action:** Caching these boolean results in the `LiquidSnapshot` during the PreTick phase avoids O(Chunks * Tiles) iterations, reducing it to O(Chunks).
