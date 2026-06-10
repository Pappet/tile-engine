## 2026-06-10 - Cross-Crate Inlining
**Learning:** Small, frequently called math functions (such as `mix_hash`) used across crate boundaries in simulation hot paths are not automatically inlined by the compiler across crates, which can lead to unnecessary function call overhead.
**Action:** Always explicitly mark such functions with `#[inline]` to guarantee cross-crate inlining and avoid function call overhead.
