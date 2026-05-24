## 2024-05-18 - Cache derived chunk aggregates in LiquidSnapshot
**Learning:** Derived chunk-level aggregates (like checking if a chunk has any liquid) should be cached in the `LiquidSnapshot` during the PreTick phase, rather than recalculating them repeatedly by iterating over chunk data inside multi-pass CA CA loops.
**Action:** When implementing new queries over chunks that are used in hot CA loops, identify whether the aggregate can be pre-calculated and cached per-tick in the respective snapshot system.
