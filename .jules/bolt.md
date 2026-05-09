## 2024-05-09 - SIMD array comparison vs `iter().any()`
**Learning:** Checking for zeroes across large arrays (like `1024` elements in `CHUNK_AREA`) using `iter().any(|&x| x > 0)` is very slow compared to direct array comparison (`**b_arr != [0u8; 1024]`) which gets auto-vectorized/optimized by LLVM into SIMD instructions (often a `memcmp`).
**Action:** Replace `iter().any(|&x| x > 0)` with `**b_arr != [0u8; CHUNK_AREA]` (where a zero array is defined) across hot loops (e.g. `fluid_ca.rs` and `vertical_flow.rs`) when checking if a chunk contains any liquid.
