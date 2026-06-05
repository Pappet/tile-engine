## 2024-06-05 - Cross-crate inlining for small functions
**Learning:** Small, frequently called math functions (such as `mix_hash`) used across crate boundaries in simulation hot paths are not automatically inlined by the compiler across crates without LTO, which can introduce significant function call overhead.
**Action:** Always explicitly mark such small helper functions with `#[inline]` to guarantee cross-crate inlining and avoid performance bottlenecks in hot paths.
