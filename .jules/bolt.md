## 2024-05-18 - Caching `has_liquid` per chunk in LiquidSnapshot
**Learning:** Checking whether a chunk has liquid by iterating over its array is an O(N) check per step, per chunk, per simulation system. By pre-computing and storing a simple `has_liquid` boolean in `LiquidSnapshot` during the `PreTick` phase, we can significantly reduce duplicated calculations during liquid CA and vertical flow steps.
**Action:** Always look for O(N) array calculations inside nested loops that remain static during a tick and move them into the snapshotting phase where they are only evaluated once.
