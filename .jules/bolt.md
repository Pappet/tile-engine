
## 2024-05-10 - [Fluid CA Performance]
**Learning:** Checking for zeros across large fixed-size arrays (like `[u8; CHUNK_AREA]`) using an iterator `.iter().any(|&x| x > 0)` is slower than using direct array comparison `*arr != [0u8; SIZE]` because the latter takes advantage of LLVM SIMD/memcmp auto-vectorization.
**Action:** When determining if a large chunk array is completely empty/zero, use direct array equivalence `*array != [0; SIZE]` rather than `.iter().any()`.
