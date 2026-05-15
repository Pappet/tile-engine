## 2024-05-15 - Optimize Periodic Reaction Lookups
**Learning:** In highly-iterative ECS environments like `sim-reaction`, looking up conditions that don't depend on ECS iteration inside a loop over components multiplies overhead. Here we observed `O(Chunks * TotalReactions)` which performed unnecessary `is_multiple_of` checks repeatedly.
**Action:** Always inspect inner loops for values that remain constant across iterations. Hoist computations and lookups to `O(TotalReactions + Chunks * ActiveReactions)` when it evaluates purely external/non-entity criteria like system time/ticks.
