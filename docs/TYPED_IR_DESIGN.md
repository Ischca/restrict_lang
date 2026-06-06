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

The `ValueId` and `ApplyIr` values inside that shadow list are placeholders for
validation and migration tests. They must not be treated as the final flow graph
until stable `ExprId` / `BindingId` assignment and the shadow affine verifier
are in place.

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
  args,
  result
}
```

`flavor` is retained for diagnostics and source reconstruction. It should not
create separate type-inference or codegen semantics unless the source form
actually requires it.

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
    keys are acceptable only at this read-only shadow stage; stable `ExprId` and
    `BindingId` assignment must replace them before IR becomes authoritative.
11. `TypedExpr.final_type` must be derived from post-check facts, not from
    fallback codegen inference or by re-checking expressions inside the builder.
12. Shadow `TypedExpr` `ValueId`s are not a control-flow or ownership authority
    yet; they become meaningful only after the flow verifier owns the graph.

## Migration Plan

1. Add IR foundations and unit tests.
2. Capture checked expression type facts from `TypeChecker` without mutating
   affine state during IR construction.
3. Introduce stable `ExprId` / `BindingId` assignment and replace temporary
   AST-instance pointer keys.
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
ABI.
