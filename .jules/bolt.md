## 2024-05-24 - [Avoid unnecessary deep copying of large structs]
**Learning:** For performance optimization, avoid doing `let mut array = [0u8; CHUNK_AREA]; array.copy_from_slice(...)` if the array is only read and not mutated. It is significantly faster to just use a slice reference `&*chunk.liquid_amount_read`. Unnecessary large stack array allocations in tight loops significantly slow down execution.
**Action:** Replace `copy_from_slice` arrays with direct references when read-only.
