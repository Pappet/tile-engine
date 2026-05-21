## 2024-05-21 - Pre-filtering active registry entries
**Learning:** In systems that iterate over many chunks or tiles, performing registry lookups and condition checks (like tick intervals) for every reaction inside the hot loop causes massive redundant work.
**Action:** Always pre-filter and pre-fetch active registry entries into a local Vec *before* the spatial loops. This shifts complexity from O(Chunks * TotalReactions) to O(TotalReactions + Chunks * ActiveReactions).
