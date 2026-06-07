## 2026-06-07 - Inline Random Number Generation
**Learning:** `mix_hash` in `crates/core/src/rng.rs` is highly utilized across crates for procedural randomness. Not marking it `#[inline]` prevents the compiler from inlining it across crates, leading to significant function call overhead in the hot loop.
**Action:** Always verify `#[inline]` on short mathematical functions used globally and primarily in tight simulation loops.
