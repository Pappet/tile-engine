## 2024-06-06 - [Cross-crate function inlining]
**Learning:** Small, frequently called math functions (such as `mix_hash`) used across crate boundaries in simulation hot paths must be explicitly marked `#[inline]` to guarantee cross-crate inlining and avoid function call overhead.
**Action:** Always verify cross-crate function usage and apply `#[inline]` where it's called frequently in performance-critical paths.
