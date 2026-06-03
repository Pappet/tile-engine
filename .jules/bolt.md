## 2024-05-18 - Cheap Probability Short-circuiting and Inlining Cross-Crate Hot Math

**Learning:** In hot simulation loops (like voxel cellular automata), evaluating dynamic conditions (iterating vectors and evaluating enum variants) can be relatively expensive. Moreover, small math functions called millions of times per tick across crate boundaries suffer from call overhead if not explicitly marked `#[inline]`.
**Action:** Always place stateless, cheap probability rolls (like `mix_hash`) *before* expensive state reads/evaluations to short-circuit the execution for low-probability events. Explicitly mark high-frequency math functions like `mix_hash` with `#[inline]` to allow LLVM to inline them across crate boundaries.
