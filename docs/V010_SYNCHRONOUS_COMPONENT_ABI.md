# v0.1 Synchronous Component ABI Scope

**Status**: B1 implementation complete; `0.1.0-rc.1` release candidate
**Target release**: v0.1.0
**Successor**: v0.2.0 async and concurrency
**Scope amendment date**: 2026-08-09

`LANGUAGE_SPECIFICATION.md` remains authoritative. The ordinary v0.0.1
compatibility boundary remains the default scalar Core host ABI. This document
defines the explicit synchronous Component path implemented for B1 and keeps
the release candidate distinct from stable v0.1.0.

The implementation architecture, first Component selection schema, and phased acceptance
gates are defined in the
[B1 Component Implementation Design](B1_COMPONENT_IMPLEMENTATION_DESIGN.md).

## Current Core and Component Paths

The ordinary path remains `.rl -> WAT -> Core Wasm`. The explicit B1 path uses
schema-v1 or schema-v2 `[component]` selection from the repository-root
`warder.toml` to produce checked `ComponentInterfaceIr`, normalized WIT and
stable ABI metadata, Canonical ABI adapters, and validated Component bytes.

`warder build --component` publishes
`<name>-<version>.component.wasm`, `.component.wit`, and `.component.toml` as a
recoverable transaction. `warder wrap --wit` is deliberately different: it
attaches caller-supplied WIT to a Core module cage without generating,
type-checking, or validating a Component.

## Decision

The B1 path implements a synchronous, versioned host ABI for these boundary
categories:

- `String`, `List<T>`, `Array<T, N>`, records, `Option<T>`,
  `Result<T, E>`, and user-defined enums;
- explicitly declared host resources with affine `own`, temporary `borrow`,
  exactly-once drop, and scalar-signature factories/methods;
- concrete value-only capture closure and function-value signatures represented
  by generated once-callable resources; and
- generic functions exposed only through explicit, deterministic concrete
  instantiations.

The compiler and Warder generate WIT, a validated WebAssembly Component, and
machine-readable interface metadata. Independent Rust bindings are generated
from WIT in the repository E2E gates.
Raw arena pointers, layout identifiers, function-table indexes, and open type
parameters must never become public ABI.

WIT file output alone is not the goal. v0.1.0 requires the complete
interoperability path: owner-selected WIT contracts, the WebAssembly Component
Model, Canonical ABI adapters, generated language bindings, component
composition, and execution in conforming runtimes.

Every v0.1.0 component function is synchronous. WIT `async func`, `future<T>`,
`stream<T>`, Restrict `await` and `spawn`, cancellation, backpressure, and
cleanup across suspension points belong to v0.2.0.

## Interface Authority and Generation Direction

Interface authority follows API ownership, not import/export direction:

- **Restrict-owned API**: an explicitly selected Restrict component export is
  checked and monomorphized into `ComponentInterfaceIr` (Component Interface
  IR), from which the compiler emits normalized WIT and adapters. Source-level
  `pub` alone never publishes a Component ABI.
- **Externally owned API**: WASI, a standard world, or a third-party component
  supplies WIT first. That WIT remains authoritative both when Restrict
  consumes/imports the API and when Restrict implements/exports the API. The
  implemented rich guest-import path generates typed bindings and adapters.
  External implementation/export conformance is currently scalar; rich
  implementation/export and external-WIT resources fail closed.
- **Final compatibility contract**: normalized WIT plus stable ABI-profile
  metadata is authoritative for both ownership paths.

Source-level `pub` does not automatically become a component export. The
existing `pub` / `export` declaration surface remains the source-public
eligibility gate, while a separate `[component]` entry in the root
`warder.toml` must explicitly select the component world and exports. This
avoids accidentally publishing internal types without reinterpreting current
Core Wasm export syntax.

This repository configuration change does not implement source-first
Component publication. The current parser still gives `pub` and `export` the
same source-public meaning. A future edition may distinguish package-visible
`pub` from Component-intent `export` and infer ordinary concrete exports, but
that surface must lower to the same checked Component selection contract.

Generated host code, Canonical ABI adapters, and Component packaging are
derived artifacts. WIT-first toolchains such as MoonBit remain design
references, while the repository's executable interoperability gates use Rust
only. Restrict generates WIT for interfaces it owns.

Requiring handwritten WIT for every Restrict-owned API would duplicate the
Restrict type declaration, create drift between two sources, and discard
useful affine/capture information and source diagnostics before the boundary is
checked. The hybrid ownership rule avoids that duplication without weakening
WIT-first conformance for external standards and third-party APIs.

### Toolchain precedent snapshot (2026-08-09)

- MoonBit's official Component Model tutorial is WIT-first: define WIT, run
  `wit-bindgen moonbit`, compile generated MoonBit to Core Wasm, then run
  `wasm-tools component embed` and `component new`.
- Grain's official CLI documents direct Core Wasm output and the project
  describes itself as WASI-first. No official Grain WIT/Component generation
  workflow was documented in the sources reviewed on this date; that is a
  documentation finding, not proof that no external experiment exists.
- Kotlin/Native targets LLVM-based native binaries and is not the corresponding
  Wasm path. Kotlin/Wasm is relevant: its official `wasi:http` sample calls a
  Kotlin fork of `wit-bindgen` on a checked-in WIT directory, compiles Core
  Wasm, and componentizes it with `wasm-tools`. The sample identifies itself as
  an early prototype rather than a final Kotlin-to-WIT exporter.

These precedents support WIT-first imports and dependency contracts. They do
not require Restrict to make authors duplicate an explicit export interface
that the Restrict compiler can derive and validate deterministically.

### Implemented selection surface

The Restrict-owned Component selection surface is versioned `[component]`
metadata in the repository-root `warder.toml`. Schema v1 selects ordinary
public functions; schema v2 additionally selects resources, once-callables,
and explicit generic function instances.

```toml
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

The declaration must also be source-public using the existing `pub` or
`export` spelling. Both gates are mandatory: a public declaration omitted from
the manifest is absent from the Component ABI, while a manifest entry naming a
private, missing, ambiguous, or duplicate declaration is an error. Neither
source-public spelling alone publishes a Component ABI.

Schema v1 requires a complete `namespace:name@semver` WIT identity in
`component.package`; its name and version must match the existing `[package]`
name and version. It requires `abi = "canonical-v1-sync"` and validates
`world`, `interface`, and external `name` as WIT identifiers. Schema v1 limits
`source` to a public function in the package root module. Schema v2 is also
root-only. Cross-module selection waits for a stable canonical declaration
identity and must never use an internal resolver-mangled or WAT symbol.

Unknown schema versions, ABI profiles, fields, and normalization collisions are
rejected. Configuration entry order has no interface meaning; normalized WIT and
stable ABI metadata remain the compatibility authority. Future source syntax
may be added only as sugar that lowers to this same selection model.

The current Warder project model has one publishable root package. Direct local
source packages are selected centrally from its `[dependencies]` table and
need only `src/lib.rl`; they do not own manifests or locks. Warder generates a
single root `warder.lock` for nondeterministic external resolutions. Those
resolvers remain fail-closed in the current local-only slice, so the lock has
no resolved package entries. Multi-publish workspace support is future work
and must extend the same root configuration without leaf manifests.

## Layer Boundary

```text
Restrict-owned explicit component exports
    -> checked and monomorphized Component Interface IR
    -> generated normalized WIT

Externally owned WIT (consume/import or implement/export)
    -> generated Restrict bindings and conformance obligations

Both ownership paths
    -> compiler-owned Core Wasm + Canonical ABI lift/lower adapters
    -> validated WebAssembly component
    -> generated language bindings
    -> component composition and conforming runtime execution
```

The Core Wasm representation remains an implementation detail. Component
adapters own all copying, validation, handle-table operations, and cleanup
needed to cross the boundary.

## Synchronous Type Mapping

| Restrict boundary type | v0.1.0 component contract |
| --- | --- |
| `Int32`, `Int64`, `Float64`, `Boolean`, `Char`, `()` | WIT `s32`, `s64`, `f64`, `bool`, `char`, and no payload. |
| `String` | WIT `string`, with UTF-8 selected and tested as the canonical encoding. No internal string pointer escapes. |
| `List<T>` | WIT `list<T>` when `T` is recursively interface-lowerable. Lift and lower copy values across the invocation boundary. |
| `Array<T, N>` | WIT `list<T>` plus a generated exact-length check and `N` in interface metadata. v0.1.0 does not depend on proposal-only fixed-length list syntax. |
| Record | A generated WIT `record` whose fields preserve source declaration order and recursively use this table. Internal padding and offsets stay private. |
| `Option<T>` | WIT `option<T>`. |
| `Result<T, E>` | WIT `result<T, E>`, including unit success or error payloads. |
| Payload-free user enum | WIT `enum`. |
| Payload-carrying user enum | WIT `variant`, preserving source case order and recursively lowering each payload. |
| Host resource | WIT `resource`, using owning handles for transfer and borrowed handles for call-scoped scalar-signature methods. Destruction occurs exactly once. |
| Closure or function value | A generated once-callable WIT resource for one concrete signature. Value-only captures are owned and dropped deterministically; resource/callable-bearing captures fail closed. |
| Generic function | No open generic appears in WIT. Each boundary function is explicitly instantiated with concrete arguments, monomorphized, assigned a stable external name, and included in compatibility metadata. |

Ordinary records must never become resources through a layout heuristic. The
source or package interface must classify a host resource explicitly before
code generation. Likewise, generic function instantiations are explicit rather
than inferred from incidental internal call sites. A distinct source byte
type, generic nominal boundary types, rich resource signatures, `many`
callables, and resource/callable-bearing captures remain follow-on work.
`Range<Int32>` and other types not listed in the table are not silently
ABI-enabled; they require a separate mapping decision.

WIT has no first-class function type or open generic declaration. Callable
resources and concrete monomorphization are therefore the v0.1.0 interoperability
contract, not temporary pointer or table-index conventions.

More generally, WIT alone cannot represent Restrict open generics,
first-class closures, affine capture multiplicity, or a stable `Array<T, N>`
length constraint. Explicit concrete instantiation, callable resources,
ownership metadata and drop adapters, and `list<T>` length validation are
therefore normative adapter rules.

## Ownership and Failure Contract

- A lifted composite argument is valid for the duration required by the
  generated adapter; the host cannot retain a pointer into a Restrict arena.
- A lowered composite result is copied or transferred into Canonical ABI-owned
  storage before its Restrict invocation arena is reset.
- `own<resource>` transfers affine ownership. `borrow<resource>` cannot escape
  the call in which it was received.
- Callable resources own captured environments. Dropping the handle releases
  captures once, and calling a dropped handle is rejected by the component
  runtime.
- A trap cannot expose partially lowered values. The generated binding reports
  a structured trap distinct from a source-level `Result::Err`.
- Re-entry through a synchronous imported callback must preserve the outer
  invocation's arena, resource, and callable state.

## Implemented B1 Gate

Schema v1 and v2 manifest parsing, recursive `ComponentInterfaceIr`, WIT
round-tripping, semantic hashing, Canonical ABI adapters, Component validation,
Warder publication, compatibility diffing, and Rust runtime execution are one
implemented path. Cage content hashes remain distinct from the semantic
interface hash.

The independently built Rust consumer shares only the versioned WIT
package/world and Component Model tooling. It does not share Restrict source,
linear memory, an allocator, arena layout, runtime representation, or a
handwritten bridge. The reverse Rust-produced Component -> Restrict consumer
gate follows the same rule.

The detailed pipeline, diagnostics contract, phases, and kill criteria are in
the [B1 Component Implementation Design](B1_COMPONENT_IMPLEMENTATION_DESIGN.md).

## Required v0.1.0 Artifacts

Warder publishes a deterministic transactional artifact set with stable paths:

1. `<name>-<version>.component.wasm`, a validated WebAssembly Component;
2. `<name>-<version>.component.wit`, the normalized generated WIT package and
   selected world; and
3. `<name>-<version>.component.toml`, containing package/world/ABI identity,
   implementation subset, semantic interface hash, and exact Component/WIT
   content hashes.

The lock update is committed in the same recoverable transaction. A failed
rebuild preserves the previous complete set. JS/TS bindings and a Worker loader
are optional later host adapters, not B1 artifacts or repository exit gates.

The existing cage content hash may continue to identify exact bytes. A separate
interface hash must be derived from normalized WIT and ABI-profile metadata so
an optimizer-only Wasm change does not masquerade as an interface break.

## B1 Conformance Evidence

Generating a `.wit` file is not completion. The implemented repository gate
automates:

- rich-value round trips for `String`, `List`, `Array`, records, `Option`,
  `Result`, enums, and variants, including Array exact-length traps;
- resource construction, repeated borrow, terminal ownership, stale-handle,
  and exactly-once destructor/drop behavior;
- value-only capture once-callable invocation and cleanup;
- explicit generic function instance naming, specialization, and runtime calls;
- generated WIT parsing and WebAssembly Component validation with pinned Rust
  tools;
- rich external-WIT guest-import binding generation plus scalar external
  implementation/export conformance;
- Component composition and execution in a pinned native runtime;
- compatibility tests proving that internal layout changes do not alter the
  interface hash and that the compatibility diff classifies both breaking and
  non-breaking changes;
- independently compiled Rust-produced Component -> Restrict consumer
  interoperability; and
- independently compiled Restrict-produced Component -> Rust consumer
  interoperability.

The two cross-language gates must share only the versioned WIT package/world.
They must not share an allocator, linear memory, Restrict arena layout,
language runtime representation, or handwritten bridge that bypasses the
Canonical ABI.

MoonBit remains a design precedent and optional external comparison target; it
is not a required repository E2E toolchain or v0.1.0 release dependency.

Intentional follow-ons are rich external implementation/export,
external-WIT resources, rich resource method/factory signatures,
resource/callable-bearing captures, `many` callables, generic nominal boundary
types, source byte syntax, and durable cross-module schema-v2 identity. Each
unsupported form fails closed with a typed diagnostic rather than exposing an
internal representation.

The experimental `flat-record-v1` profile is useful implementation evidence,
but it does not satisfy this gate and does not become the component ABI.

## WASI Relationship

v0.1.0 retains the existing `wasm-core` and WASI Preview 1 paths. Its Component
path consumes synchronous WIT capability imports, and a pinned Component host
runs the conformance world, including the existing
basic command output/exit behavior through an adapter where needed.

Full WASI 0.2 command coverage for arguments, environment, filesystem, clocks,
randomness, and every standard interface is a separate host-profile milestone;
it is not silently added to the v0.1.0 ABI release gate. Likewise, v0.2.0
proves the language and Component async boundary, but complete network/HTTP
profiles can ship as follow-on v0.2.x work. This keeps standardized capability
libraries independently testable from the language ABI.

## v0.2.0 Async Boundary

v0.2.0 owns the suspension and concurrency semantics deliberately excluded
from v0.1.0:

- Restrict async functions plus `await` and structured `spawn`;
- stackless state-machine lowering and temporal/affine checks across suspension;
- WIT `async func`, `future<T>`, and `stream<T>` adapters;
- cancellation, deadlines, backpressure, and deterministic async resource
  cleanup;
- safe re-entry and concurrent component calls; and
- generic async capability adapters, with network and HTTP profiles tracked as
  follow-on host integration rather than implied core-language features.

Shared-memory threads, distributed actors, supervision trees, and multi-node
delivery require separate acceptance criteria. They may build on v0.2.0 but are
not implied merely by completing the synchronous v0.1.0 ABI.

## Standards References

- [WebAssembly Component Model WIT design](https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md)
- [WebAssembly Component Model concurrency design](https://github.com/WebAssembly/component-model/blob/main/design/mvp/Concurrency.md)
- [MoonBit Component Model tutorial](https://docs.moonbitlang.com/en/latest/toolchain/wasm/component-model-tutorial.html)
- [Grain CLI](https://grain-lang.org/docs/tooling/grain_cli)
- [Grain WASI-first direction](https://grain-lang.org/blog/2024/03/04/announcing-grain-v0.6-emmer/)
- [Kotlin/Native overview](https://kotlinlang.org/docs/native-overview.html)
- [Kotlin/Wasm and WASI](https://kotlinlang.org/docs/wasm-wasi.html)
- [Kotlin Component Model prototype](https://github.com/Kotlin/sample-wasi-http-kotlin)
