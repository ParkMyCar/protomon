# Performance Optimization Notes for Protomon Decode Path

This document catalogs performance optimization techniques applicable to the protomon decode path, based on systems-level performance engineering research from sources including [Algorithms for Modern Hardware](https://en.algorithmica.org/hpc/).

## Table of Contents

1. [Branchless Programming](#1-branchless-programming)
2. [SIMD Varint Decoding](#2-simd-varint-decoding)
3. [Cache Line Optimization](#3-cache-line-optimization)
4. [Function Inlining and Calling Convention](#4-function-inlining-and-calling-convention)
5. [Memory Access Pattern Optimization](#5-memory-access-pattern-optimization)
6. [Lookup Tables and Precomputation](#6-lookup-tables-and-precomputation)
7. [Loop Unrolling](#7-loop-unrolling)
8. [Error Path Optimization](#8-error-path-optimization)
9. [Zero-Copy and Allocation Reduction](#9-zero-copy-and-allocation-reduction)
10. [Architecture-Specific Intrinsics](#10-architecture-specific-intrinsics)

---

## 1. Branchless Programming

### Theory

Branch mispredictions are expensive on modern CPUs, costing 10-20 cycles to flush the pipeline. Branchless programming replaces conditional branches with predicated operations (e.g., `cmov` instruction).

**Key insight from [Algorithmica](https://en.algorithmica.org/hpc/pipelining/branchless/):**
> "Using predication eliminates a control hazard but introduces a data hazard. There is still a pipeline stall, but it is a cheaper one: you only need to wait for cmov to be resolved and not flush the entire pipeline in case of a mispredict."

### When to Use
- When branch prediction is unreliable (<75% accuracy)
- Hot loops with data-dependent branches
- SIMD code (no branching in SIMD)

### When NOT to Use
- When branch is highly predictable (>75%)
- When both branches have significant computation cost

### Current State in Protomon

**leb128.rs (lines 86-168):** The current LEB128 decode uses byte-by-byte branching:
```rust
if b < 0x80 {
    return Ok((value, 1));
};
```

An experimental branchless implementation exists (`decode_u64_impl_a`) but is marked as dead code.

**wire.rs (lines 44-54):** Uses `likely()/unlikely()` hints for branch prediction but still branches.

### Potential Targets
1. LEB128 decoding - replace early-exit branches with branchless accumulation
2. Wire type dispatch - consider jump tables vs conditional chains
3. Tag matching in generated code

---

## 2. SIMD Varint Decoding

### Theory

SIMD (Single Instruction Multiple Data) can decode multiple bytes in parallel. The [varint-simd](https://github.com/as-com/varint-simd) crate demonstrates gigabytes/second throughput for LEB128 decoding using:

- SSSE3 byte shuffling (`pshufb`)
- BMI2 bit extraction (`pext`)
- AVX2 for wider operations

### Key Techniques

1. **Load 16 bytes at once** - Single memory operation
2. **Parallel MSB detection** - Identify continuation bytes with SIMD mask
3. **Bit extraction with PEXT** - Extract 7-bit payloads from each byte
4. **Branchless length calculation** - Use `tzcnt` (trailing zero count)

### Current State in Protomon

**leb128.rs (lines 436-530):** Contains `decode_u64_impl_a` which:
- Loads 16 bytes as u128
- Uses trailing zeros to find length
- Has optional BMI2 PEXT path

**Known Issue:** The comment states this performs worse in microbenchmarks, but this may be due to:
- Benchmark overhead dominating small inputs
- AMD Zen pre-5 having slow PEXT
- Missing warm-up for instruction cache

### Potential Targets
1. Batch varint decoding for packed fields
2. Wire key + varint decoding combined
3. Runtime detection of BMI2/AVX2 support

---

## 3. Cache Line Optimization

### Theory

From [Algorithmica CPU Cache](https://en.algorithmica.org/hpc/cpu-cache/):
> "The basic units of data transfer in the CPU cache system are not individual bits and bytes, but cache lines."

Modern CPUs use 64-byte cache lines. Accessing one byte fetches the entire 64-byte block.

### Key Principles

1. **Spatial locality** - Access contiguous memory
2. **Temporal locality** - Reuse recently accessed data
3. **Alignment** - Align hot data to cache line boundaries
4. **Avoid false sharing** - Pad data to prevent cache line conflicts

### Current State in Protomon

- `Repeated` struct holds offsets in `SmallVec<[u32; 8]>` - 32 bytes inline, fits in half a cache line
- `ProtoPacked` uses `SmallVec<[Bytes; 1]>` - single Bytes avoids heap allocation
- Buffer slices are processed sequentially (good locality)

### Potential Targets
1. Struct field ordering for hot path data
2. Prefetching for large message decoding
3. Alignment of decode buffers

---

## 4. Function Inlining and Calling Convention

### Theory

The x86-64 System V ABI passes the first 6 integer arguments in registers (RDI, RSI, RDX, RCX, R8, R9). Exceeding this causes stack spilling.

**Key insight:** Keep hot functions small with few arguments to maximize register usage.

### Inlining Guidelines

- `#[inline(always)]` - Force inline for very hot, small functions
- `#[inline]` - Suggest inlining, let compiler decide
- `#[cold]` - Mark error paths to prevent inlining
- `#[inline(never)]` - Prevent bloat from large functions

### Current State in Protomon

**Good practices:**
- `decode_leb128` is `#[inline(always)]`
- `likely()`/`unlikely()` use `#[cold]` function calls
- Most decode functions are `#[inline]`

**Potential Issues:**
- Generic functions may not inline across crate boundaries
- Large decode functions may cause instruction cache pressure

### Potential Targets
1. Audit inline attributes on hot paths
2. Reduce argument count where possible
3. Consider `#[target_feature]` for SIMD paths

---

## 5. Memory Access Pattern Optimization

### Theory

Sequential memory access enables hardware prefetching. Pointer chasing (following pointers to random locations) defeats prefetching.

### Key Patterns

1. **Sequential iteration** - Process arrays linearly
2. **Software prefetching** - `core::arch::x86_64::_mm_prefetch`
3. **Batch operations** - Decode multiple values before processing
4. **Avoid pointer chasing** - Keep data contiguous

### Current State in Protomon

**Good patterns:**
- Wire format is sequential
- `Repeated` stores offsets in contiguous SmallVec
- Packed fields decode sequentially

**Potential Issues:**
- `Repeated::iter()` jumps to non-contiguous offsets
- `LazyMessage` defers decoding, breaking locality

### Potential Targets
1. Prefetch hints for large messages
2. Batch decode strategies for repeated fields
3. Consider eager decode for small messages

---

## 6. Lookup Tables and Precomputation

### Theory

Replace computation with table lookups when:
- Computation is expensive
- Input domain is small
- Table fits in cache

### Current State in Protomon

**leb128.rs (lines 265-296):** Uses range matching for `encoded_leb128_len()`:
```rust
match self {
    u64::MIN..=BYTE_1_END => 1,
    BYTE_2_STR..=BYTE_2_END => 2,
    // ...
}
```

This could use `leading_zeros()` for faster computation.

### Potential Targets
1. Wire type dispatch table
2. LEB128 length calculation via bit manipulation
3. UTF-8 validation tables (for ProtoString)

---

## 7. Loop Unrolling

### Theory

Unrolling reduces:
- Branch overhead per iteration
- Loop counter updates
- Potential for better instruction scheduling

### Current State in Protomon

**leb128.rs:** Already manually unrolled (each byte handled separately)

**packed.rs (lines 376-395):** Unrolled 4x for fixed32 decoding:
```rust
for _ in 0..chunks {
    dst.push(T::read_le(ptr));
    dst.push(T::read_le(ptr.add(4)));
    dst.push(T::read_le(ptr.add(8)));
    dst.push(T::read_le(ptr.add(12)));
    ptr = ptr.add(16);
}
```

### Potential Targets
1. Wire key decoding loop
2. UTF-8 validation
3. Message field iteration

---

## 8. Error Path Optimization

### Theory

Move error handling out of the hot path:
- Use `#[cold]` on error construction
- Avoid error formatting until needed
- Keep error types small

### Current State in Protomon

**error.rs:** Uses enum with varied sizes
**util.rs (lines 23-25):** Has `cold_path()` function for unlikely branches

### Potential Targets
1. Audit error construction in hot paths
2. Consider `Result<T, ()>` internally, convert at boundary
3. Use `Option` where error details aren't needed

---

## 9. Zero-Copy and Allocation Reduction

### Theory

Allocations are expensive (~50-100ns). Zero-copy techniques:
- Slice existing buffers instead of copying
- Use `bytes::Bytes` for reference-counted sharing
- Avoid temporary `Vec` allocations

### Current State in Protomon

**Excellent:**
- `ProtoString`/`ProtoBytes` wrap `bytes::Bytes` (zero-copy)
- `LazyMessage` defers parsing
- `Repeated` stores offsets, not values

**Potential Issues:**
- `String::decode_into` allocates and copies
- `Vec<u8>::decode_into` allocates and copies
- Iterator-based decoding creates intermediate values

### Potential Targets
1. Reduce cloning in `Repeated::iter()`
2. Pool allocations for temporary buffers
3. In-place decoding for fixed-size arrays

---

## 10. Architecture-Specific Intrinsics

### Theory

Use CPU-specific instructions when available:
- BMI2: `pext`, `pdep` (bit manipulation)
- AVX2: 256-bit SIMD
- AVX-512: 512-bit SIMD (limited availability)

### Detection Strategies

```rust
// Compile-time feature detection
#[cfg(target_feature = "bmi2")]

// Runtime detection
if is_x86_feature_detected!("bmi2") { ... }
```

### Current State in Protomon

**leb128.rs (lines 476-490):** Has BMI2 PEXT code path:
```rust
#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
{
    let part_a = core::arch::x86_64::_pext_u64(leb_part_a, 0x7f7f7f7f7f7f7f7f);
    // ...
}
```

**Limitation:** Compile-time only, no runtime dispatch.

### Potential Targets
1. Runtime feature detection with fallback
2. AVX2 for batch packed decoding
3. Consider `multiversion` crate for automatic dispatch

---

## Summary of Optimization Priorities

| Priority | Technique | Expected Impact | Complexity |
|----------|-----------|-----------------|------------|
| HIGH | Branchless LEB128 | 10-30% decode speedup | Medium |
| HIGH | SIMD packed decoding | 2-5x for large arrays | High |
| HIGH | Error path cold marking | 5-15% hot path speedup | Low |
| MEDIUM | Cache prefetching | 5-20% for large msgs | Medium |
| MEDIUM | Lookup tables | 5-10% for specific ops | Low |
| MEDIUM | Inline attribute audit | 5-15% depending on code | Low |
| LOW | Loop unrolling | 5-10% in specific loops | Low |
| LOW | Struct field ordering | 1-5% improvement | Low |

---

## Benchmark Strategy

Each optimization should be validated with:

1. **Microbenchmarks** - Isolate the specific operation
2. **Integration benchmarks** - Full message decode/encode
3. **Profile-guided analysis** - Use `perf` to verify hotspots
4. **Multiple architectures** - Test on Intel and AMD

Use the existing benchmarks in `benches/`:
- `codec.rs` - Full message encode/decode
- `leb128.rs` - Varint performance
- `packed.rs` - Packed field decoding

---

## References

- [Algorithms for Modern Hardware - Algorithmica](https://en.algorithmica.org/hpc/)
- [varint-simd - SIMD LEB128 decoding](https://github.com/as-com/varint-simd)
- [x86 Calling Conventions](https://en.wikipedia.org/wiki/X86_calling_conventions)
- [Rust Inline Assembly](https://doc.rust-lang.org/reference/inline-assembly.html)
- [Rust core::arch Intrinsics](https://doc.rust-lang.org/core/arch/index.html)
