# Value Representation and ABI Design

This document is an internal design note for Restrict compiler development. It
does not expand the published v0.0.1 public host ABI. The default v0.0.1
release surface still exports only scalar monomorphic functions and scalar
literal globals. Post-v0.0.1 experiments must be selected explicitly; the
first such profile is the narrow core-Wasm adapter `flat-record-v1` documented
below.

## Goals

1. Keep Restrict's internal value model explicit enough for optimization.
2. Avoid publishing raw linear-memory layouts as a stable host ABI.
3. Preserve affine ownership and arena region information across lowering.
4. Leave room for later composite host adapters without blocking v0.0.1.
5. Define experimental adapters independently from mutable internal layouts.

## Non-Goals

- No source-level `form` / `takes` support in v0.0.1.
- No user-defined ADT layout in v0.0.1.
- No composite host ABI in the default v0.0.1 release surface.
- No Temporal Affine Type runtime contract in the default v0.0.1 gate.
- No promise that internal byte offsets are externally stable.

## ABI Facets

Restrict values must be described through two separate ABI facets.

| Facet | Audience | Contract |
| --- | --- | --- |
| Internal ABI | Compiler, optimizer, Wasm lowering | May use typed references, layout descriptors, arena regions, and specialized layouts. |
| Host ABI | External caller | v0.0.1 supports only `Int32`, `Int64`, `Float64`, `Boolean`, `Char`, and `()`. |
| Experimental host profile | Explicitly opted-in external caller | `flat-record-v1` adds generated scalar-flattening wrappers for a restricted class of records without publishing their internal layout. |

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

`LayoutTable` interns lowerable descriptor shapes within one checked IR build.
Repeated uses of the same finalized internal shape should therefore reuse the
same `LayoutId`, giving later lowering and optimization passes one canonical
handle for facts such as element size, alignment, sum tags, and closure ABI
metadata. Opaque unlowered generic descriptors are not interned until they carry
enough provenance to preserve diagnostics. All layout IDs remain compiler-local
metadata and are not stable across builds or observable through the host ABI.

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

Checked record metadata preserves source field order, and Layout IR records
internal field names, offsets, and element layouts for concrete monomorphic
source record instantiations. Open generic record layouts remain opaque until
monomorphization supplies stable field sizes. These offsets are compiler
metadata for lowering and optimization; they are not published as a v0.0.1 host
ABI and the legacy codegen path remains authoritative until Layout IR lowering
is adopted feature by feature.

The opt-in `flat-record-v1` profile does not make these offsets or descriptors
public. Its wrapper translates between source-ordered scalar values and the
current internal representation each time the module is generated.

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

These candidates are advisory metadata on the descriptor. The current concrete
layout strategy remains `TaggedPayload`; candidates must not become source-level
semantics, host ABI promises, or implicit layout changes until a later lowering
pass explicitly makes one authoritative.

### Range<Int32>

`Range<Int32>` is a current v0.0.1 source type. The existing codegen treats it
as a pointer-shaped internal value. The IR layout table now keeps that
two-endpoint model as a dedicated internal descriptor:

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

## Experimental Core-Wasm Profile: `flat-record-v1`

### Selection and compatibility boundary

The profile is selected only with:

```text
--host-abi flat-record-v1
```

It is experimental and opt-in. Without this option, v0.0.1 release-surface
validation remains authoritative and rejects record-valued host function
boundaries. The profile affects eligible function exports only; it does not add
record globals, direct type exports, WIT, or Component Model output.

### Eligible boundary records

An eligible record:

1. is a concrete, non-generic source record;
2. is declared with `pub record` in the source module;
3. has no temporal type parameters or constraints;
4. declares from 1 through 16 fields; and
5. has only direct scalar fields.

The direct field mapping is:

| Restrict field | Core-Wasm value |
| --- | --- |
| `Int32` | `i32` |
| `Boolean` | `i32`, retaining the existing `0` or `1` scalar contract |
| `Char` | `i32`, retaining the existing Unicode scalar-value contract |
| `Int64` | `i64` |
| `Float64` | `f64` |

Unit fields and reference-shaped fields are ineligible. In particular, the
profile does not recursively flatten nested records and does not admit
`String`, `List`, `Array`, `Option`, `Result`, range, closure, function, opaque,
generic, or temporal fields.

The exported function must itself be non-generic and non-temporal. Generic
source signatures remain ineligible even if internal monomorphization can
produce a concrete call-site specialization.

### Flattening and slot limits

Parameters are flattened from left to right in source function parameter order.
An ordinary scalar parameter contributes one core-Wasm value. A record
parameter contributes one value per field in the record's source declaration
order. A Unit parameter keeps the current dummy-`i32` parameter convention,
while a Unit result contributes zero result values.

The flattened parameter vector is limited to 16 core-Wasm value slots in total.
The flattened result vector is independently limited to 16 slots. Each `i32`,
`i64`, and `f64` value counts as one slot. The limits count values rather than
32-bit storage words.

A scalar result retains the existing scalar ABI, and unit has no result. An
eligible record result becomes a core-Wasm multi-value result whose values are
ordered by the record's source field declarations. Hosts using this profile
must therefore support core-Wasm multi-value function results.

For example, a parameter list consisting of an `Int32`, a record whose declared
fields are `active: Boolean` followed by `total: Int64`, and a `Float64` maps
to:

```text
(param i32 i32 i64 f64)
```

A record result whose declared fields are `code: Int32` followed by
`elapsed: Float64` maps to:

```text
(result i32 f64)
```

Source field order is the external contract. Internal field offsets, padding,
specialization, or descriptor ordering cannot change the flattened order.

### Generated wrapper and lifetime invariants

An export that uses this profile is split into an internal Restrict body and a
generated host wrapper. Only the wrapper is exported, and it uses the source
export name. The internal symbol name is intentionally unspecified and is not
part of the ABI.

For a record parameter, the wrapper materializes the internal record from the
incoming scalar values before calling the Restrict body. For a record result,
the wrapper must read every result field into scalar locals before resetting or
restoring the invocation arena. It then returns those locals as multi-value
results. Internal Restrict calls continue to use the internal body and internal
calling convention rather than routing through the host wrapper.

Adapter arenas are compiler-private regions placed after static data, and the
module's initial memory is sized from the completed static-data and arena plan.
Synchronous same-export re-entry saves the outer bump mark and allocates nested
values after it. On normal completion, each wrapper restores the depth and mark
captured at its own entry; this also repairs the outer state when a host catches
a trap from a nested same-export call and then returns to the outer invocation.
If a trap escapes the top-level wrapper, core Wasm does not run that cleanup.
The embedding must discard the instance and instantiate a fresh module before
making another `flat-record-v1` call; continued use of the trapped instance is
outside this preview profile's contract.

These rules establish the following invariants:

- no internal record pointer crosses the host boundary;
- no `LayoutId`, byte offset, arena address, or function-table index is host
  observable;
- all returned values remain valid after the invocation arena is reset because
  every result is already a scalar local; and
- no host allocator, ownership handle, copy-out buffer, or post-return function
  is introduced by this profile.

The compiler must reject an unsupported record boundary instead of exporting
the internal pointer-shaped calling convention. General composite ownership,
nested-value copying, pointer-length values, WIT canonical lowering, and
Component Model integration require a later, separately versioned profile.

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
   typed representation does not yet expose a dedicated range variant even
   though the IR layout table has a dedicated descriptor for its internal shape.
5. Keep general composite host adapters deferred until their descriptors and
   ownership rules are stable. `flat-record-v1` is a narrower exception because
   its external contract contains only source-ordered scalar values and never
   publishes a descriptor or internal pointer.

## Read-Only ABI Summary

The IR ABI summary is advisory compiler metadata. A function can be marked as a
v0.0.1 host-ABI candidate only when it has no declared generic or temporal
signature surface, is monomorphic after checking, and every host-visible
parameter and return maps to `HostAbi::Unit` or `HostAbi::Scalar`.

All `Ref(LayoutId)`, closure, descriptor, region, and composite layouts remain
internal representation details. The default summary must not export
additional functions, expose raw linear-memory pointers, expose function table
indexes, or generate composite adapters. The release-surface validator and
existing codegen remain authoritative for invocations that do not explicitly
select an experimental host ABI profile. `flat-record-v1` may generate only the
scalar-flattening wrapper defined above; it does not change the meaning or
visibility of `Ref(LayoutId)`.
