## 2024-05-22 - [Optimize Liquid Snapshot Caching]
**Learning:** Checking if a chunk has any non-zero liquid amounts using `.iter().any(|&v| v > 0)` is expensive when called repeatedly for the same chunk across multiple passes and neighbor checks.
**Action:** Pre-compute and cache derived chunk-level aggregations (like `chunk_has_liquid`) into `LiquidSnapshot` during the PreTick phase to avoid recalculating it multiple times inside multi-pass cellular automata systems.
