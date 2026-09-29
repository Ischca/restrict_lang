# B2 Async Component Implementation Plan

**Status:** Implementation planning; no B2 runtime support is claimed.
**Baseline:** Published v0.1.0, with the synchronous Next.js Component demo.
**Audit date:** 2026-09-29

This plan breaks the B2 / v0.2.0 milestone in [ROADMAP.md](../ROADMAP.md)
into reviewable increments. [LANGUAGE_SPECIFICATION.md](../LANGUAGE_SPECIFICATION.md)
remains authoritative. The older async design documents are exploratory;
their example syntax is not an implementation contract.

## Verified starting point

- `FunDecl::is_async` and `ExprKind::Await` / `Spawn` already exist in
  `src/ast.rs`. Parsing an async declaration does not establish runtime support.
- `src/type_checker.rs` contains experimental `AsyncRuntime` handling.
  `check_spawn_expr` computes but discards the callable's return type when
  constructing a `Task`; `get_task_result_type` returns an `Int32` placeholder.
  `check_await_expr` also accepts a non-temporal task as a compatibility path.
  These are gaps to resolve before promoting async to the release surface.
- `src/codegen.rs` rejects `Await` / `Spawn`. There is no working suspension
  implementation behind the Next.js demo's asynchronous Server Function.
  That function invokes a synchronous Restrict Component through its adapter.
- B1 Component selection and adapters reject async functions, futures, streams,
  and unsupported ABI profiles. Keep those synchronous checks intact.
- `tests/cases/test_async_basic.rs` and `test_async_comprehensive.rs` are
  gated by the experimental `tat` feature and contain historical syntax.
  Their existence is not evidence of passing, current async behavior.

## First implementation increment: types and ownership

Specify the contract in `LANGUAGE_SPECIFICATION.md` before implementing it:

1. Distinguish an async computation, a scheduled child task, and its eventual
   result. Decide which concepts require distinct source types. Never infer
   an async result from a record name or a fixed `Int32` fallback.
2. Preserve the result type through creation and awaiting, including `String`,
   a record, and `Result`, so scalar-only fixtures cannot hide lost type data.
3. State how task values are bound to their owning scope, how awaiting consumes
   them, and which captures may survive suspension. Remove the non-temporal
   compatibility path once the replacement contract is implemented.
4. Test using current OSV calls and statement boundaries. Migrate relevant
   historical fixtures by their intended behavior, not by mechanically
   preserving their old syntax.
5. Keep runtime and Component use rejected until the corresponding lowering
   and ownership tests exist. A type-checking-only increment is experimental.

Before committing to a public ABI profile, also run a small, independent
toolchain probe: pin a native Component host and JS/TS binding tool, then test
one genuinely suspended import/export. Record supported features and gaps.
WIT parsing alone is not a successful probe, and the demo's current Jco pin
is not evidence of async compatibility.

## Decisions to settle in that increment

| Decision | Tradeoff to resolve |
| --- | --- |
| When an async computation starts | Lazy creation gives explicit execution control; eager execution may feel familiar to Promise users but couples creation to scheduling. |
| Unawaited children at normal scope exit | Joining preserves child completion but may delay exit; cancellation bounds ownership but needs visible cancellation semantics. |
| Cancellation result | Keep cancellation distinct from a successful result and specify whether it is a typed outcome or control flow. |
| Storage across suspension | Ordinary call-scoped arena storage may expire before resumption; define ownership and release of suspended frames and captured values. |
| Trap cleanup | Do not promise to resume guest cleanup after a trap. Define which host-owned resources can be released and how the failed instance or task is invalidated. |

The single-thread executor and structured task ownership are already B2
requirements. These decisions refine that scope; they do not add threads,
distributed actors, an HTTP framework, or a JavaScript source backend.

## Following increments and their completion evidence

1. **Suspend and resume one computation.** Lower to an explicit state machine
   with storage for live values. A controllable host fixture must keep a call
   pending, resume it, and validate the result. Test immediate completion,
   multiple suspension points, double-resume rejection, and disposal while
   pending. A blocking call wrapped in a Promise does not satisfy this gate.
2. **Structured task lifecycle.** Add the single-thread executor, child-task
   ownership, cancellation, and deadlines under the chosen scope-exit rules.
   Use deterministic wakeups and a controllable clock. Count releases across
   normal completion, cancellation, errors, and traps according to the
   ownership contract; reject tasks escaping their scope.
3. **Async Component calls and futures.** Extend checked Component metadata,
   Canonical ABI lowering, and packaging under an explicitly versioned
   profile. Cover callbacks, re-entry, waitables, and concurrent calls.
   Run both independent-producer directions in a native host and the JS/TS
   adapter. Keep the existing `canonical-v1-sync` conformance gate passing.
4. **Bounded channels and streams.** Specify buffer ownership, closure, and
   cancellation. Tests must show the producer stops when capacity is reached
   and buffered values are released when the consumer stops. Add the stream
   ABI and AsyncIterator binding only with equivalent host conformance cases.
5. **Application evidence.** Extend a demo with a controllable async capability
   behind WIT. Keep Next.js-specific adaptation outside Restrict and Warder.
   Complete filesystem, HTTP, database, and edge deployment profiles remain
   separately scoped integrations.

## Test and documentation boundaries

Follow [TESTING_GUIDELINES.md](TESTING_GUIDELINES.md): reuse existing integration
harnesses, isolate temporary projects, and serialize tests that touch shared
state. Add tests for result typing, ownership, lifecycle transitions, and
observable host behavior rather than generated implementation text. Confirm
focused filters execute tests; zero selected tests do not establish a gate.

The next source change requires its focused tests and the normal compiler
preflight. This planning document alone does not require a Cargo build or test
run. Update public support claims only when a complete corresponding execution
path passes its gate. None of the B2 implementation checkboxes is completed by
this audit or by the synchronous Next.js demo.
