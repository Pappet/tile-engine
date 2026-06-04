## 2024-06-04 - Mark mix_hash function #[inline] to eliminate function call overhead across boundaries
**Learning:** Small, frequently called math functions like `mix_hash` that are heavily utilized across multiple crates in simulation hot paths suffer from function call overhead because Rust does not cross-crate inline them automatically by default.
**Action:** Always explicitly add the `#[inline]` attribute to frequently used utility functions shared between crates in hot paths.
