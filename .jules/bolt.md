## 2024-05-18 - Periodic Reaction Loop Optimization
**Learning:** Optimizing simulation systems (like periodic_reaction_system) by pre-filtering and pre-fetching active registry entries into a Vec outside of hot chunk/tile loops significantly reduces redundant work from O(Chunks * TotalReactions) to O(TotalReactions + Chunks * ActiveReactions).
**Action:** Extract the active reactions into a `Vec` *before* iterating over chunks.
