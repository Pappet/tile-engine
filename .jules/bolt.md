
## 2024-05-11 - Fast zero-checks on large arrays
**Learning:** Checking for zeroes across large fixed-size arrays (like `[u8; CHUNK_AREA]`) is significantly faster using direct array comparison (`*arr != [0u8; SIZE]`) rather than `iter().any(|&x| x > 0)` due to LLVM SIMD/memcmp auto-vectorization.
**Action:** Always prefer direct array equality/inequality checks for fixed-size continuous chunks of memory when validating empty/zero states instead of manual iterators or scalar loops.
