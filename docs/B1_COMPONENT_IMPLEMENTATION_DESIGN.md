# B1 Component Implementation Design

**Status**: B1 synchronous implementation complete on
`reset-v001-release`; repository package version remains `0.0.1`
**Target**: B1 / v0.1.0 synchronous Component ABI
**Decision date**: 2026-08-09
**Normative scope**:
[v0.1 Synchronous Component ABI Scope](V010_SYNCHRONOUS_COMPONENT_ABI.md)

This document records the implemented B1 path and its remaining limits. The
ordinary compiler path remains `.rl -> WAT -> Core Wasm`; explicit Component
selection adds checked `ComponentInterfaceIr`, normalized WIT and stable
metadata, Canonical ABI adapters, validated Component bytes, compatibility
diffing, composition, Rust bindings, and runtime execution.

Warder `build --component` is wired to schema v1 and v2 and transactionally
publishes `.component.wasm`, `.component.wit`, and `.component.toml`. Independent
Rust consumers call Restrict-produced Components, and a Rust-produced rich
Component is consumed through Restrict guest imports. `warder wrap --wit`
remains separate caller-provided WIT attachment to a Core module cage.

## Decision Summary

1. The first selection surface is the repository-root `warder.toml`. No new
   Restrict keyword, modifier, or attribute is added for the first slice.
2. A Restrict-owned Component export passes two independent gates:
   - the declaration is source-public under the existing `pub` / `export`
     surface; and
   - the versioned `[component]` configuration selects it explicitly.
3. Neither `pub` nor `export` alone publishes a Component ABI. Their existing
   Core Wasm and source-module behavior remains unchanged.
4. Selected declarations are resolved to stable canonical declaration
   identities, type checked, and genuinely specialized before they enter
   `ComponentInterfaceIr`.
5. `ComponentInterfaceIr` is separate from the current Core Wasm `HostAbi`,
   `ValueRepr`, and layout tables. It describes the public logical interface,
   not the arena implementation.
6. WIT-only output is not a Component claim. B1 includes packaging,
   validation, compatibility diffing, composition, and independent runtime
   execution.
7. Repository cross-language conformance uses Rust in both required
   directions. MoonBit may inform design or external comparison work, but it
   is not a B1 repository toolchain or release gate.

## Why Root Configuration First

The existing parser accepts both `pub` and `export` as the same public
top-level declaration shape. Reinterpreting either spelling for Component
publication would silently change the current Core Wasm surface and would not
provide the package, world, interface, external-name, resource, or concrete
generic-instance information that B1 eventually needs.

The repository-root `warder.toml` already owns build and explicit package
identity. It is therefore the least disruptive place to select a Component
world and the implemented resource, callable, and generic function entries.
A future edition may distinguish package-visible `pub` from Component-intent
`export` and infer ordinary concrete exports from typed source. That
source-first surface is not implemented in B1. It must lower to the same
versioned selection model and must not create a second interface authority.

The double gate also prevents accidental ABI publication:

| Source-public | Configuration-selected | Result |
| --- | --- | --- |
| No | No | Private implementation detail. |
| Yes | No | Available according to current source/Core rules, but absent from the Component ABI. |
| No | Yes | Build error at the manifest entry with the declaration lookup context. |
| Yes | Yes | Candidate for Component type checking, specialization, and interface lowering. |

## Manifest Schema v1

Warder implements parsing, shape validation, compiler translation, and
transactional publication for this selection surface in the root
`warder.toml`. The compiler enforces the source-public plus
configuration-selected gate and lowers it through Component
Interface IR, WIT, Canonical ABI adapters, packaging, validation, and Rust
runtime conformance. Durable cross-module identity remains follow-on work.

```toml
[package]
name = "checkout"
version = "0.1.0"
entry = "src/main.rl"
edition = "2025"

[component]
schema = 1
package = "acme:checkout@0.1.0"
world = "checkout"
abi = "canonical-v1-sync"

[[component.exports]]
interface = "api"
source = "quote"
name = "quote"
```

The selected root declaration can use either existing public spelling:

```restrict
pub fun quote: (subtotal: Int32) -> Int32 = {
    subtotal
}
```

For schema v1:

- `schema` is required and must equal `1`; an unknown version is rejected
  rather than interpreted approximately;
- `package` is the complete WIT package identity in
  `namespace:name@semver` form;
- for schema v1, the name and version in `component.package` must equal
  `[package].name` and `[package].version`;
- `abi` is required and must be `canonical-v1-sync`;
- unknown fields are rejected rather than ignored;
- `world`, `interface`, and `name` are explicit WIT identifiers and must pass
  WIT identifier validation;
- `exports` contains at least one explicit entry;
- `source` is one non-keyword root-module Restrict identifier and must name a
  source-public function;
- each `(interface, name)` pair is unique;
- each manifest entry selects exactly one declaration and unresolved,
  ambiguous, private, or duplicate selections are errors; and
- manifest array order is not semantic. Normalization imposes deterministic
  ordering before WIT generation and hashing.

Schema v1 deliberately limits `source` to a root-module function.
Cross-module selectors require a durable `CanonicalDeclId` produced by module
resolution. Resolver-mangled WAT names such as internal module aliases are not
stable source identities and must never appear in WIT or interface metadata.

## Manifest Schema v2 Contract

Schema v2 is the manifest-neutral declaration contract for resources,
once-callable resources, and concrete generic function instances. Warder
translates it to a deterministic compiler model consumed by
`ComponentInterfaceIr`, specialization, Canonical ABI code generation,
artifact publication, validation, and the runtime tests.

```toml
[component]
schema = 2
package = "acme:checkout@0.1.0"
world = "checkout"
abi = "canonical-v1-sync"

[[component.exports]]
interface = "api"
source = "identity"
name = "identity-string"
type-args = { T = "String" }

[[component.resources]]
interface = "api"
source = "FileHandle"
name = "file-handle"
destructor = "close_file"
destructor-visibility = "private"
methods = [
  { source = "read_file", name = "read", receiver = "borrow" },
  { source = "finish_file", name = "finish", receiver = "own" },
]

[[component.callables]]
interface = "api"
name = "transform"
params = [{ name = "value", type = "List<String>" }]
result = "Result<String, TransformError>"
multiplicity = "once"
```

Schema v1 remains accepted unchanged and rejects `type-args`, `resources`, and
`callables`. An unknown schema still fails closed. Schema v2 requires at least
one explicit function export, resource, or callable and applies these rules:

- a function `source` is one root Restrict identifier; the same generic
  function may be selected more than once only when its complete concrete
  `type-args` map differs, and every instance has a distinct external name;
- type-argument keys are source type-parameter names. Values are parsed closed
  type expressions rather than opaque strings. Parsing accepts `()`, scalars,
  concrete root-nominal applications, `List`, `Array<T, N>`, `Option`,
  `Result`, and function types. It rejects temporal types, `Self`, wildcards,
  malformed built-in arities, symbolic array lengths, and trailing input;
- parsing a nominal name is not name resolution. The later compiler pass must
  resolve the name, validate bounds, restore declared parameter order, and
  produce a genuine specialization. Incidental call-site monomorphization is
  never a selection;
- a resource `source` must resolve to one named public record. `destructor`
  records one root function identity, and `destructor-visibility` (`private`
  or `public`) states which source visibility must be verified. Each method
  explicitly records its source identity, external name, and receiver mode.
  The destructor source cannot also be selected as an ordinary method;
- the implemented resource adapter accepts scalar-signature factories and
  methods. Rich resource signatures and external-WIT resources fail closed;
- `borrow` is call-scoped and cannot escape the synchronous method call. `own`
  transfers and consumes the affine resource handle. The adapter ensures one
  destructor call on every owning path;
- a callable declares concrete named parameters, a concrete result, and an
  explicit `once` or `many` multiplicity. The initial v0.1 slice accepts only
  `once`; `many` is parsed as a known value but rejected with a typed
  unsupported-multiplicity error. Once-callables may capture ordinary values;
  captures containing resources or other callables fail closed; and
- manifest array/map order is non-semantic. The compiler-neutral model sorts
  declarations, methods, and type-argument keys and rejects duplicate external
  identities before any CIR lowering.

The v2 model deliberately contains source and ownership facts that WIT cannot
carry by itself. It contains no arena address, layout hash, WAT symbol, table
index, or trusted raw WIT. Cross-module selectors remain rejected until a
durable `CanonicalDeclId` exists. Byte-buffer source syntax is not introduced
by this schema.

## Selection and Identity Model

The Warder parser produces its manifest `ComponentConfig`. Schema v1 translates
to `ComponentSelection`; schema v2 translates to the normalized
`ComponentSelectionContract`. Neither compiler model nor
`ComponentInterfaceIr` depends on Warder's TOML structs. Both contracts are
validated against resolved, checked root declarations before CIR lowering.

The source resolver must provide a stable identity for every selected
declaration:

```text
CanonicalDeclId = package identity + canonical module path + declaration name
```

The identity is independent of filesystem spelling after canonical module
resolution, import aliases, WAT symbol mangling, declaration traversal order,
and optimizer output. Source spans and manifest locations remain attached for
diagnostics but are excluded from interface hashing.

In the current root-only implementation, the compiler records the exact unresolved
root-public declaration and verifies that the resolved and checked program
still contains that declaration. A durable cross-module `CanonicalDeclId`
remains to be implemented. The restriction must be diagnosed explicitly; it
must not fall back to a flattened or mangled imported name.

## Component Interface IR

`ComponentInterfaceIr` is built after source resolution, type checking, and
explicit specialization, and before Canonical ABI or Core Wasm adapter
generation:

```text
resolved and checked Restrict program
          +
versioned ComponentSelection
          |
          v
ComponentInterfaceIr
```

Its logical shape is:

```rust
ComponentInterfaceIr {
    package,
    world,
    abi_profile,
    interfaces,
    stable_metadata,
}

ComponentInterface {
    name,
    types,
    functions,
    resources,
}

ComponentFunction {
    source_id,
    source_span,
    wit_name,
    params,
    result,
    specialization,
}
```

The exact Rust field spelling is internal. These invariants are normative:

- the graph contains finalized Component boundary types and no unresolved
  inference variable or open Restrict type parameter;
- `Array<T, N>` remains distinct from `List<T>` in the IR so exact length can
  remain stable ABI metadata even though WIT lowering uses `list<T>`;
- record field order and enum/variant case order preserve their source
  declaration identity;
- resources retain `own`, `borrow`, destructor, and call-scope facts;
- callable resources retain signature, capture ownership, and multiplicity
  facts that WIT alone cannot express;
- concrete generic instances retain their canonical source identity, concrete
  type arguments, and stable external name;
- Restrict-owned and external-WIT-owned interfaces record their authority so
  generated-WIT and conformance flows cannot be confused; and
- source spans are retained for diagnostics but are not compatibility data.

The IR must not contain:

- arena pointers or offsets;
- `LayoutId`, internal padding, or `ValueRepr`;
- WAT identifiers or function-table indexes;
- the current Core-only `HostAbi` as its type model;
- caller-provided raw WIT text as a trusted type fact; or
- an unresolved generic, temporal lifetime, or unsupported boundary type.

`HostAbi` remains the contract for current Core Wasm profiles. Component
mapping belongs to a separate `ComponentType` graph so extending B1 cannot
silently widen the v0.0.1 scalar ABI or `flat-record-v1`.

## Normalized WIT and Stable Metadata

The WIT emitter is a deterministic projection of `ComponentInterfaceIr`, not
a second type checker and not a string template over the source AST.

Normalization must:

1. validate every external identifier and reject normalization collisions;
2. sort packages, interfaces, functions, and world items by stable external
   identity;
3. preserve record-field and variant-case declaration order;
4. order reachable named types deterministically with a stable tie-breaker;
5. emit one canonical whitespace and newline form; and
6. parse and resolve the generated package again with a pinned WIT parser.

The current Cage `abi_hash` identifies exact packaged Wasm and attached WIT
content. It is not a semantic interface hash and must not be reused as one.
B1 keeps two distinct concepts:

- an artifact content hash, which may change when optimized Wasm bytes change;
  and
- an interface hash derived from the schema version, normalized WIT, and
  canonical ABI-profile metadata.

An optimizer-only internal change must leave the interface hash unchanged. A
meaningful WIT or stable adapter-metadata change must update it. Compatibility
classification is a semantic diff over the normalized interface, not equality
of either hash by itself.

## Compiler and Warder Pipeline

The implemented production pipeline is:

```text
warder.toml
    -> validate schema and explicit selections

.rl sources
    -> resolve modules and CanonicalDeclId values
    -> type check and preserve source diagnostics
    -> perform requested concrete specialization

selection + checked specialized symbols
    -> build and validate ComponentInterfaceIr
    -> emit normalized WIT and stable ABI metadata
    -> reparse and resolve generated WIT
    -> generate Core Wasm and Canonical ABI adapters
    -> embed component type information and package Component Wasm
    -> validate the Component with pinned in-process tooling
    -> execute independent conformance consumers
```

Production builds should use pinned in-process WIT, componentization, and
validation libraries recorded in the repository toolchain lockfiles. The
generated `warder.lock` is reserved for nondeterministic package resolution;
the current local-only dependency slice gives it no resolved package entries.
A pinned `wasm-tools` CLI may
serve as a CI oracle, but correctness must not depend on an unpinned executable
found on the user's `PATH`.

Warder owns artifact orchestration and transactional publication. The compiler
library owns parsing, resolution, type checking, specialization, Component
Interface IR, WIT generation, and adapter code generation. Warder must not
reconstruct compiler types from emitted WAT or source text.

## Implemented B1 Gate

The B1 implementation includes:

- versioned manifest parsing and structural validation;
- root-module selection and exact source-declaration identity tests;
- recursive `ComponentInterfaceIr` construction for the supported B1 values,
  resources, once-callables, and explicit generic function instances;
- deterministic normalized WIT generation;
- WIT parser round-trip and golden tests;
- interface-hash fixtures and compatibility diffing over normalized WIT and
  stable metadata;
- canonical adapters, Component encoding and validation;
- transactional Warder publication; and
- both independent Rust interoperability directions through WIT only.

The ordinary compiler still emits Core Wasm unless the explicit Component path
is selected. A Cage WIT attachment is never treated as a verified interface.
Generated WIT is always paired with Component encoding, validation, and runtime
tests before this document calls the path implemented.

## Producer Component Slice

Every Restrict-owned export follows the same producer path, first proven for a
scalar function and extended by the rich/resource/generic tests:

1. a source-public root function is explicitly selected by Component selection schema v1;
2. selection resolves to one stable canonical declaration identity;
3. its finalized scalar signature enters `ComponentInterfaceIr`;
4. the compiler generates normalized WIT and matching Component type data;
5. the compiler emits the Core function and the required Canonical ABI-facing
   Component structure without exposing an internal pointer or layout;
6. Warder packages deterministic Component Wasm and validates it with pinned
   Component Model tooling;
7. an independently compiled Rust consumer generates or checks its binding
   from the versioned WIT world, instantiates the Component in a pinned native
   runtime, calls the function, and verifies its result; and
8. determinism and hash tests distinguish artifact bytes from the semantic
   interface contract.

The Rust consumer may share only the versioned WIT package/world and ordinary
Component Model tooling. It must not share Restrict source, WAT symbols,
linear memory, an allocator, arena layout, runtime representation, or a
handwritten bridge that bypasses the Component boundary.

`warder build --component` now publishes only after this gate succeeds on the
exact staged bytes. A failed build preserves the prior Component artifact set.

## Required Repository E2E Direction

The required repository interoperability language is Rust:

- the first accepted producer slice is
  `Restrict-produced Component -> Rust consumer`; and
- the complete v0.1.0 release gate additionally requires
  `Rust-produced Component -> Restrict consumer`.

Both directions build independently and connect only through WIT, Component
types, and the Canonical ABI. MoonBit does not appear in the required E2E
matrix. It remains an optional external precedent or comparison target.

## Implementation Phases

### Phase 1: Selection and Stable Identity

- Implemented: optional versioned `[component]` data, schema/profile/package/
  identifier/duplicate validation, neutral `ComponentSelection`, root-public
  selection, and the public-plus-selected double gate.
- Implemented: preserve the existing parser and current `pub` / `export`
  behavior.
- Remaining: introduce durable `CanonicalDeclId` values before cross-module
  selectors are accepted.

### Phase 2: Component IR and WIT

- Implemented: independent recursive Component type graph, deterministic WIT
  and semantic metadata/hash, pinned WIT reparse, ordering, and negative tests.

### Phase 3: Producer and Publication

- Implemented: Standard32 export aliases, Component type metadata, deterministic
  in-process Component encoding/validation, semantic/content hash separation,
  transactional Warder publication, and independent Rust consumption.

### Phase 4: External WIT Consumer Slice

- Implemented: parse rich externally owned WIT, generate Restrict guest-import
  bindings and adapters, compose independently, and run
  `Rust-produced Component -> Restrict consumer`.
- Implemented: scalar external implementation/export conformance.
- Follow-on: rich external implementation/export and external-WIT resources;
  both currently fail closed with typed diagnostics.

### Phase 5: Recursive Value Types

- Implemented: `String`, `List`, `Array`, records, `Option`, `Result`, and user
  enums/variants through the same recursive IR and Canonical ABI adapter.
- Implemented: array-length traps and nested rich-value runtime coverage.
- Follow-on: a distinct source byte type before exposing WIT `list<u8>`.

### Phase 6: Resources, Callables, and Concrete Generics

- Implemented: schema-v2 resource `own`/`borrow`/drop, scalar-signature
  factories and methods, value-only capture once-callables, and explicit finite
  generic function instances with stable external names.
- Implemented: source resolution, checked specialization, Component IR,
  Canonical ABI adapters, validation, Warder publication, and Rust runtime
  tests for these entries.
- Follow-on: rich resource signatures, resource/callable-bearing captures,
  `many`, generic nominal boundary types, and cross-module schema-v2 identity.

### Phase 7: Packaging and Compatibility

- Implemented: Component composition, pinned Rust runtime gates, transactional
  artifact publication, and breaking/non-breaking compatibility diffing.
- Optional follow-on: typed JS/TS bindings and a framework-neutral Worker
  loader. They are not repository B1 completion gates.
- Release management remains separate: the package version is still `0.0.1`.

## Diagnostics Contract

Every failure must identify its stage and retain the best available source:

- configuration errors identify `warder.toml`, the component entry index, and
  the invalid field;
- lookup errors show the requested source name and valid source-public root
  candidates without exposing internal mangled names;
- visibility errors point to both the configuration selection and source
  declaration and explain the two-gate rule;
- unsupported-type errors name the boundary path, such as
  `api.quote parameter subtotal`, and the current implementation phase;
- generic errors distinguish missing explicit type arguments from failed
  specialization or an unresolved inference variable;
- WIT name collisions report every source declaration that normalizes to the
  same external identity;
- generated-WIT failures identify whether emission, parse, resolution, or
  source-to-WIT conformance failed;
- component failures distinguish adapter generation, type embedding,
  componentization, validation, instantiation, and call execution; and
- external-WIT errors point into the WIT contract and the mismatching Restrict
  implementation or binding obligation.

Unsupported boundary types and features are rejected. They never fall back to
an arena pointer, a raw Core Wasm signature, or an unverified WIT attachment.

## Kill Criteria

The implementation must fail closed if any of these conditions would be
required to make the slice appear complete:

- all source-level `pub` or `export` declarations are published implicitly;
- an unknown Component selection schema or ABI profile is accepted or ignored;
- duplicate or colliding external names are resolved by traversal order;
- cross-module selection depends on an internal resolver-mangled or WAT name;
- `ComponentInterfaceIr` contains `HostAbi`, `ValueRepr`, `LayoutId`, an arena
  address, padding, or a function-table index as public type information;
- `is_monomorphic()` is treated as if it generated a concrete generic
  specialization;
- an unsupported type silently falls back to a pointer or Core-only ABI;
- a generated WIT file or attached WIT text is reported as Component support
  without matching componentization, validation, and execution;
- the current Cage `abi_hash` is used as semantic compatibility authority;
- an optimizer-only change alters the interface hash, or a meaningful WIT or
  stable ABI-metadata change does not;
- two identical clean builds produce different normalized WIT, Component
  interface identity, or semantic interface hash;
- the Rust consumer needs Restrict internal symbols, shared linear memory, a
  shared allocator, an internal layout, or a handwritten bridge; or
- Warder publishes any member of the Component artifact set before all staged
  bytes and hashes validate.

If pinned Component tooling stops packaging, validating, or running the
generated artifact, the implementation must fail closed. The project must not
weaken the gate to WIT-only output.

## Test Evidence Required

The gate requires unit, golden, negative, and runtime tests for manifest
selection, root identity, IR invariants, normalized WIT, interface hashing,
rich values, resources, callables, explicit generics, and external WIT. It also
requires:

- in-process Component validation;
- a second validation oracle in CI where practical;
- an independent Rust consumer build and runtime call;
- repeated clean-build determinism checks;
- regression tests proving unselected public declarations stay absent; and
- docs guardrails that keep ordinary Core output, the explicit B1 path, and a
  future published v0.1 release distinct.

Follow-on slices extend the same pipeline and do not create parallel ABI paths
for composite values, resources, callables, or generic instances.
