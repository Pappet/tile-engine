## 2024-05-25 - Pre-filter active reactions outside hot chunk loop
**Learning:** In the voxel simulation, checking `every_ticks` for periodic reactions inside the nested `chunks.iter()` -> `reactions` loop causes redundant registry lookups and trigger evaluation for every chunk. Since periodic triggers are uniform across all chunks for a given tick, this can be pre-calculated.
**Action:** Pre-filter active registry entries into a `Vec` outside of hot chunk/tile loops to reduce redundant work from O(Chunks * TotalReactions) to O(TotalReactions + Chunks * ActiveReactions).
