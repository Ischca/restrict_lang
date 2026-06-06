# Value Representation and ABI Design

This document is an internal design note for Restrict compiler development. It
does not expand the v0.0.1 public host ABI. The v0.0.1 release surface still
exports only scalar monomorphic functions and scalar literal globals.

## Goals

1. Keep Restrict's internal value model explicit enough for optimization.
2. Avoid publishing raw linear-memory layouts as a stable host ABI.
3. Preserve affine ownership and arena region information across lowering.
4. Leave room for later composite host adapters without blocking v0.0.1.

## Non-Goals

- No source-level `form` / `takes` support in v0.0.1.
- No user-defined ADT layout in v0.0.1.
- No composite host ABI in v0.0.1.
- No Temporal Affine Type runtime contract in the default v0.0.1 gate.
- No promise that internal byte offsets are externally stable.

## ABI Facets

Restrict values must be described through two separate ABI facets.

| Facet | Audience | Contract |
| --- | --- | --- |
| Internal ABI | Compiler, optimizer, Wasm lowering | May use typed references, layout descriptors, arena regions, and specialized layouts. |
| Host ABI | External caller | v0.0.1 supports only `Int32`, `Int64`, `Float64`, `Boolean`, `Char`, and `()`. |

The compiler must never treat internal layout as host-visible contract. Later
composite host support should be generated through adapters that use layout
descriptors to copy, view, or handle values safely.

## ValueRepr

The IR foundation uses a compact value representation:

```text
Unit
Scalar(I32 | I64 | F64)
Ref(LayoutId)
Closure { layout: LayoutId, abi: AbiId }
```

This is intentionally not identical to raw Wasm locals. `Ref(LayoutId)` means
"an internal typed pointer whose layout is known to the compiler", not "a stable
host pointer".

## Scalar Values

| Restrict type | Internal repr | v0.0.1 host ABI |
| --- | --- | --- |
| `Int32` | `i32` | `i32` |
| `Boolean` | `i32` with `0` or `1` | `i32` |
| `Char` | Unicode scalar value as `i32` | `i32` |
| `Int64` | `i64` | `i64` |
| `Float64` | `f64` | `f64` |
| `()` | no value | no value or bridge unit |

## Composite Values

Composite values are internal-only in v0.0.1 host exports.

### String

Initial internal descriptor:

```text
StringRef -> [len:u32][flags_or_cap:u32][utf8 bytes...]
```

The descriptor must distinguish interned constants from arena-allocated strings
through flags or storage-class metadata. A future host adapter may expose
`data,len` views or copy-out buffers, but the raw internal pointer is not the
ABI.

### List<T>

Initial internal descriptor:

```text
ListRef -> [len:u32][cap:u32][elem_size:u32][elem_layout_id:u32][elements...]
```

This extends the current list shape toward generic helper reuse and layout-aware
optimization. The hot path should use compile-time known element size and avoid
loading descriptor fields when specialization can erase them.

### Array<T, N>

Initial internal descriptor:

```text
ArrayRef -> [len:u32][elem_size:u32][elem_layout_id:u32][elements...]
```

Fixed arrays are good candidates for later inline storage, SROA, and stack/local
lowering when `T` is scalar or copyable.

### Record

Records use monomorphized layout descriptors. Internal record layout may reorder,
pad, or split fields later, but source field order remains the canonical surface
for diagnostics and future host adapters.

Two strategies are expected:

- `DescriptorManaged`: layout is centralized and may evolve.
- `FieldsOnly`: a concrete lowerable layout with known offsets.

The current implementation should move toward centralizing record offset
calculation in the layout table instead of recomputing it in codegen paths.

### Option<T> and Result<T, E>

Initial internal descriptor:

```text
Option<T>      -> [tag:u32][payload aligned]
Result<T, E>   -> [tag:u32][payload aligned]
```

Logical tags:

- `None = 0`, `Some = 1`
- `Err = 0`, `Ok = 1`

The descriptor should retain optimization candidates:

- null niche for `Option<Ref>`
- scalar pair for small copy payloads
- unboxed tag and payload for scalar-only local flows

These are internal optimizations, not source-level semantics.

### Range<Int32>

`Range<Int32>` is a current v0.0.1 source type. The existing codegen treats it
as a pointer-shaped internal value. The initial descriptor should keep the
current two-endpoint model:

```text
RangeRef -> [start:i32][end:i32]
```

Ranges over non-`Int32` endpoints remain outside the v0.0.1 public support
surface.

### Function Values

Initial internal descriptor:

```text
ClosureRef -> [table_index:u32][abi_id:u32][capture_bytes:u32][captures...]
```

Non-capturing closures and named function values can later be optimized to direct
calls or thin callable references. Host callers should not observe function
table indexes directly.

## Region and Arena Ownership

Arena allocation is not only an allocator detail. The IR should treat it as a
region capability:

```text
ValueId -> RegionId -> RegionKind
```

Current region kinds:

- `DefaultArena`
- `ArenaScope`
- `HostBoundary`
- `TemporalScope` for future gated work

The verifier should reject heap-backed values that escape an arena scope unless
the escape is explicitly represented by a supported host adapter or region
transfer.

## Optimization Contract

The representation is designed so later passes can erase overhead:

- scalar `Option` / `Result` can become tag-payload locals
- small records can be split into scalar locals
- list pipelines can fuse when no observable boundary exists
- non-capturing closures can become direct calls
- layout descriptor reads can become constants after specialization
- arena allocations can be grouped and reset by region

The semantic IR may carry rich ownership and region metadata, but hot lowering
must not keep that metadata as runtime cost unless required.

## Migration Notes

1. Keep v0.0.1 release-surface validation as the host ABI authority.
2. Introduce `ValueRepr` and `LayoutTable` as compile-time metadata first.
3. Move codegen layout choices behind descriptor queries incrementally.
4. Treat `Range<Int32>` as a source-type migration item because the finalized
   typed representation does not yet expose a dedicated range variant.
5. Generate composite host adapters only after internal descriptors are stable.
