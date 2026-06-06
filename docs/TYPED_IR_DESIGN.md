# Typed IR and Optimization Pipeline Design

This document defines the internal pipeline direction for Restrict. It is not a
public language reference. The language specification remains the source of
truth for syntax and v0.0.1 semantics.

## Pipeline

The target pipeline is staged:

```text
AST
  -> Checked IR
  -> Layout IR
  -> Wasm MIR
  -> Optimized Wasm MIR
  -> WAT/Wasm
```

The current compiler still lowers mostly from AST and type-checker side tables.
The IR work should be introduced as a foundation first, then made authoritative
one boundary at a time.

The current IR foundation includes a read-only Checked IR builder. It constructs
function signature IR, normalized Apply sites, and a flat `TypedExpr` skeleton
from checked expression facts after type checking while the existing code
generator remains AST-driven.

## Why Restrict Needs Its Own IR Shape

Restrict should not treat OSV as a parser-only surface. OSV gives the source
language a natural value-flow order:

```restrict
value |> transform |> summarize
```

For A-layer type inference, this order is not the solving order. The constraint
solver remains order independent. For B-layer ownership, the order matters:
values are evaluated and consumed from left to right.

The IR should therefore preserve two facts at once:

- finalized type facts from A-layer inference
- affine/resource flow facts from B-layer validation

## Checked IR

Checked IR records source-level semantics after type inference has completed.
Every expression node should have:

```text
ExprId
FinalType
ValueRepr
FlowSummary
TypedExprKind
```

`FinalType` must not contain `InferVar` or `Projection`. Those are A-layer
implementation details and must not pass into codegen or layout selection.

The current builder creates a flat shadow list of `TypedExpr` entries for AST
expressions that the type checker has already checked. This list is not yet an
authoritative typed tree. It is the first bridge from finalized type facts into
the IR layer, allowing layout selection and later local optimization work to
stop depending on ad hoc AST/type-checker queries one feature family at a time.
Facts that still contain `InferVar` or `Projection` are deliberately not
materialized as `TypedExpr` entries.

The `ApplyIr` values inside that shadow list are shared with normalized apply
site metadata through builder-local `ExprId`s, so source-level call surfaces can
be validated without regenerating placeholder value IDs. The `ValueId`s remain
placeholders for validation and migration tests. They must not be treated as the
final flow graph until stable `BindingId` assignment and the shadow affine
verifier are in place.

During the read-only shadow stage, builder-local `ValueId`s must still be
internally coherent. An `ApplyIr` result must be the value produced by its
matching `TypedExpr`, and apply argument IDs should refer to values already
produced by child shadow expressions. The callee `ValueId` may still be a
synthetic placeholder, but top-level function callees now carry builder-local
`callee_provenance` with the source name, declared type parameters, finalized
declaration signature, return representation, and monomorphic status. Function
values, immediate lambdas, and method-resolution calls remain value-based until
their symbol and receiver identities are represented explicitly. These IDs and
callee facts are provenance links for builder validation and migration tests;
they are not stable `BindingId`s, ownership authorities, region capabilities,
ABI handles, or cross-build identities.

The builder also assigns builder-local `BindingId`s for parameters and simple
identifier bindings such as `val alias = value` or `mut val alias = value`.
Identifier expressions that resolve through this shadow scope are emitted as
`TypedExprKind::Binding`. Complex pattern bindings are not decomposed at this
stage; names introduced by those patterns are installed only as shadow barriers
so later identifier expressions do not accidentally resolve to an outer binding
with the same name. These IDs are useful provenance for diagnostics and
optimization reports, but they are not yet authoritative symbol identities or
rewrite handles.

A read-only shadow invariant validator runs after Checked IR construction. It
only checks builder-local provenance: each `NormalizedApplySite.expr_id` must
point to a `TypedExprKind::Apply` with the same `ApplyIr`, `TypedExpr.value`
must be that `ApplyIr.result`, and the expression's `FlowSummary` must record
that result as produced. For each Apply expression, `FlowSummary.uses()` must
cover `ApplyIr.args` one-for-one in OSV/evaluation order with events at the
Apply `ExprId`; top-level callee placeholders are not treated as ownership uses.
Passing this validator does not make Checked IR the codegen source of truth, a
stable `BindingId` graph, or an ownership authority.

### Read-Only Function Lowering Readiness

Checked IR also reports a read-only lowering-readiness summary for each
function: whether the source declaration was exported, which type parameters and
temporal constraints were declared, parameter and return `HostAbi` values, the
current body result `ValueId`, required layout descriptors, and separated
readiness for internal lowering versus v0.0.1 host ABI eligibility.

This is migration evidence only. It does not make Checked IR the source of WAT
generation, replace `WasmCodeGen::generate`, or authorize a new host-visible ABI
surface. `HostAbi::Unit` and `HostAbi::Scalar` remain the only v0.0.1
host-exportable shapes. Composite, generic, closure/function-value, temporal, or
unfinalized types stay internal-only unless a future host adapter explicitly
defines otherwise.

## Apply Normalization

The following source surfaces should converge to one IR shape:

- `value |> function`
- `(a, b) function`
- `() function`
- named function value call
- parenthesized function value call
- immediate lambda call
- method-resolution call

The IR node is:

```text
Apply {
  flavor,
  callee,
  callee_provenance,
  args,
  result
}
```

`flavor` is retained for diagnostics and source reconstruction.
`callee_provenance` separates top-level function symbols from value callees
without changing ownership flow: top-level symbols are not recorded as argument
uses, and value callees do not become direct calls until a later authoritative
symbol graph and closure representation exist. Neither field should create
separate type-inference or codegen semantics unless the source form actually
requires it.

## A-Layer Boundary

A-layer inference may use:

- `InferVar`
- associated type `Projection`
- `Constraint`
- `Substitution`
- delayed lambda replay
- built-in `Container` adoption

The IR builder boundary must finalize these into ordinary types. If an
unresolved inference value reaches IR construction, this is an internal compiler
error or a user-facing inference error before codegen.

## B-Layer Flow

B-layer flow is not a constraint bag. It is environment threading:

```text
FlowEnv -> expr -> FlowEnv
```

Initial IR support can mirror the existing affine behavior through use events:

```text
UseEvent {
  value,
  kind: ReadCopy | Move | BorrowShared | BorrowMut | Drop,
  at: ExprId
}
```

The current shadow builder records Apply argument uses in OSV/evaluation order.
For example, `(a, b) f` records uses for `a` then `b`, `value |> f` records the
left-side object as the single argument use, and `() f` records no argument
uses. Scalar and unit arguments are `ReadCopy`; composite reference and closure
arguments are `Move` until a later borrow/copy analysis proves a narrower
effect.

Later, this becomes the authoritative place for:

- branch and match residual environment merge
- record field move tracking
- freeze and clone effects
- context availability
- arena escape validation
- gated temporal cleanup

## Layout IR

Layout IR chooses internal representation without exposing host ABI. It attaches:

```text
ValueRepr
LayoutId
LayoutDescriptor
HostAbi
Region
```

The key rule is separation: `HostAbi` is the export contract, while
`LayoutDescriptor` is compiler-owned internal machinery.

Within one checked IR build, `LayoutTable` canonicalizes identical lowerable
descriptor shapes so repeated values of the same internal shape share a
`LayoutId`. This keeps later optimization facts attached to one handle instead
of scattering them across duplicate descriptors. Opaque unlowered generic
descriptors are intentionally not canonicalized until they carry enough
provenance to preserve diagnostics. The IDs are still build-local compiler
metadata, not source-visible names or host ABI handles.

`Range<Int32>` remains a source-level record-shaped type fact after checking,
but Layout IR gives it a dedicated internal descriptor with `start` and `end`
`i32` endpoints at fixed offsets. This preserves the current v0.0.1 range
surface while giving later lowering and optimization passes the concrete
two-endpoint shape instead of an empty generic record descriptor.

Sum descriptors for `Option` and `Result` retain logical tags and continue to
use `TaggedPayload` as the concrete layout strategy. They may also carry
advisory optimization candidates such as null niches and scalar tag-payload
pairs. Those candidates are facts for future lowering decisions, not permission
for the current shadow IR builder or WAT generator to change representation.

## Wasm MIR

Wasm MIR is deliberately lower level than Checked IR. It is where semantic
metadata is either erased or lowered into concrete Wasm operations.

The first optimization-stage foundation should support:

- hygiene cleanup such as removing `nop`
- local constant folding
- later copy/move elimination
- later direct-call conversion for non-capturing closures
- later scalar replacement of aggregates
- later list pipeline fusion

The current `src/ir/optimize.rs` foundation is intentionally small. It proves
that the pipeline has a distinct optimization stage without committing codegen
to a full rewrite.

Implemented Wasm MIR optimization levels are ordered as validation barriers:

- `None`: preserve the MIR exactly.
- `Hygiene`: remove semantically empty instructions such as `nop`.
- `Local`: run hygiene first, then local stack rewrites to a fixpoint.

The current local pass folds adjacent `i32.const`, `i32.const`, `i32.add`
patterns using WebAssembly wrapping integer semantics. It intentionally remains
below Layout IR: it does not change `HostAbi`, layout descriptors, regions, or
ownership facts.

Planned passes such as copy/move elimination, closure direct-call conversion,
scalar replacement, and list pipeline fusion require stronger flow and layout
metadata before they can become authoritative.

The current optimization foundation also includes a read-only Checked IR
value-use summary. It classifies produced `ValueId`s as body results,
copy-only scalar flows, single affine moves, unused pure values, or apply
results that must not be rewritten while effect information is unknown. This is
an analysis bridge for later move/copy elimination; it does not rewrite Checked
IR, does not change WAT generation, and does not authorize removing Apply nodes.

The first affine forwarding report is similarly conservative. It may flag a
runtime-reference binding value that is moved exactly once into one Apply
argument, preserving the binding id/name, Apply flavor, and argument index for
diagnostics and future lowering. It deliberately excludes direct literal moves
and scalar copy reads, and every candidate remains blocked on stable
authoritative `BindingId` / expression provenance before any rewrite can happen.
This keeps the optimization path tied to Restrict's affine value flow without
introducing hidden clone/copy behavior.

## Invariants

1. IR and codegen never accept `InferVar` or `Projection`.
2. v0.0.1 host exports remain scalar-only.
3. Internal composite representation is not host ABI.
4. OSV order is preserved for ownership flow and diagnostics.
5. A-layer inference remains order-independent.
6. B-layer resource flow remains evaluation-order aware.
7. Arena and future temporal regions are represented as capabilities, not just
   allocator calls.
8. Optimizations may erase metadata, but must not introduce hidden clone or
   implicit copy.
9. The read-only Checked IR builder must not re-run inference, mutate
   `TypeChecker` affine state, or become the codegen source of truth until the
   Layout IR migration begins.
10. Checked expression facts are snapshots for the current AST instance. Pointer
    keys are acceptable only at this read-only shadow stage; authoritative
    stable `ExprId` and `BindingId` assignment must replace them before IR
    becomes authoritative.
11. `TypedExpr.final_type` must be derived from post-check facts, not from
    fallback codegen inference or by re-checking expressions inside the builder.
12. Shadow `TypedExpr` `ValueId`s are not a control-flow or ownership authority
    yet; they become meaningful only after the flow verifier owns the graph.
13. Shadow `ApplyIr` value flow must be provenance-coherent within one builder
    run, but remains builder-local until stable `ExprId` / `BindingId` and the
    affine flow verifier own the graph.
14. Shadow Apply `FlowSummary` use events must cover `ApplyIr.args` in
    OSV/evaluation order without adding callee ownership semantics for top-level
    function symbols.
15. Builder-local `BindingId`s must identify parameter and simple identifier
    binding provenance only. Complex pattern names must block outer provenance
    but not create partial binding identities. They are not stable symbol
    identities until the authoritative binding graph replaces the shadow scope.
16. Top-level `ApplyIr.callee_provenance` must agree with the normalized callee
    hint and its monomorphic flag must match the finalized declaration
    signature. Value callees must not be promoted to top-level provenance by
    name alone.

## Migration Plan

1. Add IR foundations and unit tests.
2. Capture checked expression type facts from `TypeChecker` without mutating
   affine state during IR construction.
3. Introduce authoritative stable `ExprId` / `BindingId` assignment and replace
   temporary AST-instance pointer keys. Builder-local `ExprId`s already link
   normalized apply metadata to matching shadow `TypedExpr` entries.
4. Build read-only Checked IR from AST while existing codegen remains active.
   The current builder covers function signatures, normalized Apply sites, and
   a flat `TypedExpr` skeleton from checked facts.
5. Extend Apply normalization with checked function-value and method metadata.
6. Move layout selection behind `LayoutTable`.
7. Add a shadow flow verifier matching current affine behavior.
8. Make codegen consume Layout IR for one feature family at a time.
9. Add Wasm MIR lowering and make the optimizer authoritative.

## Current Scope

This design stage adds the skeleton, invariants, and a read-only builder for
function signatures, Apply sites, and checked expression type facts. It does not
yet replace `WasmCodeGen::generate`, change generated WAT, or broaden the host
ABI. The lowering-readiness summary is advisory metadata for the migration plan;
the release-surface validator and existing codegen remain authoritative.
