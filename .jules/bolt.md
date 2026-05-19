
## 2024-05-19 - Cache Derived Aggregate Statistics per Chunk Early
**Learning:** Derived chunk-level aggregates (like determining if a chunk `has_liquid` by scanning O(Area) tiles) should be calculated and cached during the initial `PreTick` snapshot phase rather than repeatedly during multi-pass cellular automata systems, avoiding expensive repeated scans.
**Action:** When a simulation requires aggregates or checks across an entire chunk's data in the inner loop, cache it into `LiquidSnapshot` or a similar struct at snapshotting time for O(1) reads later.
