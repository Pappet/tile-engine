## 2024-05-24 - Array Comparison Optimization
**Learning:** In Rust fixed-size array checks, checking for non-zero elements using `iter().any(|&x| x > 0)` is significantly slower than `*arr != [0u8; SIZE]` because LLVM can easily auto-vectorize the latter into SIMD memcmp instructions.
**Action:** Replace iterator chains over large fixed-size byte arrays with direct comparisons to zeroed arrays where performance is critical.
