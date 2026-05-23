## 2026-05-23 - Pre-filter active periodic reactions
**Learning:** Optimizing simulation systems by pre-filtering and pre-fetching active registry entries into a `Vec` outside of hot chunk/tile loops significantly reduces redundant work from O(Chunks * TotalReactions) to O(TotalReactions + Chunks * ActiveReactions).
**Action:** Pre-filter active reactions outside of chunk loops in simulation systems.
