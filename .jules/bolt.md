
## 2024-05-27 - Cache aggregate chunk properties in pre-tick snapshots
**Learning:** Checking chunk-level aggregates like `chunk.liquid_amount_read.iter().any(|&a| a > 0)` is expensive in O(N) multi-pass cellular automata systems, especially when chunks are mostly empty.
**Action:** Always pre-calculate and cache chunk-level aggregates inside snapshot structures like `LiquidSnapshot` during the PreTick phase, and use fast HashSets or HashMaps instead of iterating `ChunkData` repeatedly.
