# Restrict Distributed Concurrency Vision

## Status

This is a non-normative design note for future exploration. None of the task,
actor, mailbox, distributed delivery, or supervision semantics described here
are part of the current v0.0.1 release surface.

The language specification remains authoritative. In particular, `await` and
`spawn` are currently reserved or experimental, and the v0.0.1 code generator
does not provide a concurrency runtime.

## Goal

Restrict should not aim to reproduce BEAM processes unchanged. The intended
competitive axis is a WebAssembly-first concurrency model that combines:

- statically typed, stackless tasks
- affine ownership transfer for local messages
- arena-oriented allocation and deterministic task cleanup
- structured cancellation through temporal scopes
- explicit remote communication with schema-specialized encoding
- bounded mailboxes, backpressure, and observable failure states

The performance goal must be measured by several independent dimensions rather
than a single claim that Restrict is "faster than BEAM":

- task creation and completion throughput
- bytes of runtime overhead per parked task
- local and remote message throughput
- p50, p99, and p999 scheduling and delivery latency
- scaling across CPU cores and cluster nodes
- behavior during overload, cancellation, node loss, and restart storms

## Where Restrict May Have an Advantage

### Local concurrency

The strongest local opportunity is to transfer ownership of a message arena
instead of copying or tracing the complete message graph. An affine send would
consume the sender's message value. The receiver would become its sole owner,
and the runtime could reclaim the arena in one operation after processing.

This is a proposed extension of the current arena model, not current behavior.
It would require at least three memory domains:

1. a task-local arena for temporary values
2. a transferable message arena with one logical owner
3. an explicitly shared immutable area for frozen values

Affine ownership alone is not a complete data-race proof. The compiler and
runtime must also prevent mutable aliases, define the semantics of frozen
values, and restrict access to shared linear memory.

### Distributed concurrency

A pointer cannot be transferred across machines. Remote delivery must encode
and copy bytes through a network transport. Affine typing is still useful
because it can establish that the sender no longer owns the logical message,
avoid unintended duplicate sends, bound the lifetime of encoding buffers, and
allow a type-specialized encoder to be generated for each concrete message
schema.

Restrict should use a canonical wire format. It must not expose or transmit the
compiler's current in-memory record layout directly. A wire schema should have
an explicit identity, version, compatibility policy, size limits, and canonical
representation independent of a particular WebAssembly engine.

## Proposed Runtime Model

### Tasks and scheduling

- Lower async functions into stackless state machines.
- Run many Restrict tasks on a fixed pool of host-created OS threads.
- Use per-core ready queues with work stealing.
- Insert compiler-managed scheduling safe points at loop backedges and selected
  calls so a CPU-bound task cannot monopolize a worker.
- Apply an execution budget between safe points to provide preemption-like
  fairness without requiring a stackful process per task.
- Keep allocation out of the steady-state poll and wake paths where practical.

Core WebAssembly shared memory and atomics can support the worker pool, but the
host is responsible for creating threads. In a browser, workers should be
created ahead of demand. In a native environment, Warder or another host
runtime would own the worker and I/O pools.

### Structured tasks and long-lived actors

Restrict should distinguish two abstractions:

- A structured task belongs to a temporal parent scope. Leaving the scope waits
  for or cancels all children before releasing their resources.
- A long-lived actor owns state and a bounded mailbox. Detaching it from the
  caller is allowed only under an explicit supervisor or service scope.

This separation keeps ordinary `spawn` operations structurally bounded while
still allowing servers and background services to live longer than one request.

### Mailboxes and backpressure

- Mailboxes are bounded by default.
- A send operation reports accepted, rejected, or waiting-for-capacity state.
- Producers cannot silently create an unbounded queue in another task.
- Per-message and per-mailbox byte limits are part of the runtime contract.
- Message selection should avoid an unbounded linear scan unless explicitly
  requested by a higher-level protocol abstraction.

## Distributed Topology

Remote communication must remain visible in the type and API surface. A remote
send has latency, partial failure, authentication, serialization, and versioning
concerns that a local send does not have.

The default cluster topology should not automatically form a full mesh. A
possible first architecture is:

```text
Restrict task
    -> node-local router
    -> multiplexed authenticated connection
    -> remote router
    -> bounded mailbox
    -> remote actor
```

The router layer would provide:

- a logical address containing cluster, node, actor, and actor generation
- a small number of multiplexed connections rather than one connection per task
- batching for small messages and streaming for large payloads
- per-peer and per-actor flow control
- schema negotiation and explicit rejection of incompatible messages
- authentication based on TLS and scoped capabilities
- metrics for queue depth, encode time, delivery latency, and dropped messages

Location transparency is not a goal. Code should be able to see whether an
address is local or remote and select an appropriate failure and latency policy.

## Delivery Semantics

Network delivery cannot generally distinguish "not delivered" from "delivered
but acknowledgement lost." A distributed affine send must therefore model the
ambiguous case instead of returning the original message unconditionally.

A conceptual outcome has three states:

```text
NotSent(Message)  -- definitely not delivered; ownership can be returned
Delivered(Ack)    -- accepted according to the selected durability policy
Uncertain(Id)     -- delivery may have occurred; the original cannot be reused
```

The default transport should provide at-most-once delivery with ordering only
within a documented sender/receiver stream. Retrying an uncertain operation
requires an explicit idempotency key or an operation type declared idempotent.
End-to-end exactly-once effects are not a transport promise; they require
deduplication and durable transactional state at the application boundary.

## Failure, Supervision, and Ownership

- A local supervisor handles task traps and restart policy.
- Remote monitoring uses leases, heartbeats, and actor generations; it does not
  pretend that node failure can be detected instantaneously.
- Cancellation propagates down a temporal task tree and runs deterministic
  cleanup for owned resources.
- Actor migration freezes intake at a safe point, transfers a canonical state
  snapshot, increments the actor generation, and resumes only after ownership
  has been committed to one destination.
- The type system can prevent ordinary duplicate ownership, but it cannot by
  itself prevent split-brain during a network partition. Migration and leader
  election require a coordinator, consensus protocol, or an application-defined
  conflict policy.

Supervisor APIs, distributed discovery, persistence, rolling upgrades, and
observability are part of the runtime product, not syntax-only language work.

## WebAssembly and WASI Boundary

WebAssembly supplies a portable execution target, shared memory, and atomic
operations, but not an Erlang-style process model or distributed runtime.
WASI 0.3 supplies native async component functions, streams, futures, and async
socket operations that can serve as a portable I/O boundary. Restrict must
still define and implement tasks, scheduling, mailboxes, routing, delivery,
supervision, and cluster membership.

The first implementation should target a native Wasmtime/WASI host. Browser
support should reuse the source semantics but may use a pre-created Web Worker
pool and accept different performance characteristics.

## Implementation Order

### Phase 0: Semantics and benchmarks

1. Define structured task, cancellation, mailbox, and affine send semantics.
2. Define measurable comparison workloads and avoid relying on a sequential
   ring benchmark alone.
3. Record baseline results for Erlang/OTP and at least one Rust async runtime on
   the same hardware.

### Phase 1: Single-thread executor

1. Lower one async function into a stackless state machine.
2. Implement wake, timer, cancellation, and deterministic arena cleanup.
3. Add bounded local channels and task-leak tests.

### Phase 2: Multicore runtime

1. Add a fixed worker pool and per-core queues.
2. Implement work stealing and compiler-inserted execution budgets.
3. Add transferable message arenas and immutable frozen sharing.

### Phase 3: Two-node transport

1. Define the canonical message schema and compatibility rules.
2. Add authenticated TCP or QUIC transport through the host runtime.
3. Implement multiplexing, batching, bounded queues, and backpressure.
4. Expose `NotSent`, `Delivered`, and `Uncertain` outcomes.

### Phase 4: Distributed actors

1. Add actor identities, generations, monitors, and local supervisors.
2. Add discovery and non-full-mesh routing.
3. Implement migration only after a durable ownership protocol is specified.

### Phase 5: Operational maturity

1. Add tracing, profiling, queue inspection, and overload diagnostics.
2. Add rolling protocol upgrades and mixed-version compatibility tests.
3. Add restart-storm, partition, packet-loss, and recovery benchmarks.

## Benchmark Matrix

The project should publish results only for reproducible workloads with fixed
hardware, runtime versions, message sizes, and durability settings.

| Workload | Primary measurements |
| --- | --- |
| Spawn and join storm | tasks/s, bytes/task, p99 completion |
| One million parked tasks | RSS, wake latency, scheduler CPU |
| Local ping-pong and fan-out | messages/s, p99/p999 latency |
| Owned large-message pipeline | copies, encode time, bandwidth |
| Bounded-mailbox overload | producer latency, drops, recovery time |
| Two-node small-message exchange | messages/s, CPU/message, p99 latency |
| Large streamed transfer | throughput, peak RSS, copies |
| Node loss and reconnect | detection time, uncertain deliveries |
| Supervisor restart storm | availability, queue recovery, CPU saturation |
| Multi-node scale-out | connection count, routing cost, throughput/node |

Early success means outperforming BEAM on selected statically typed message and
arena-lifetime workloads while remaining explicit about areas where BEAM/OTP is
still stronger, especially fault-tolerant supervision, distribution tooling,
and operational history.

## Background References

- [Restrict Language Specification](../LANGUAGE_SPECIFICATION.md)
- [Existing async/concurrency exploration](./ASYNC_CONCURRENCY_DESIGN.md)
- [Temporal async theory](./TEMPORAL_ASYNC_THEORY.md)
- [Erlang process efficiency guide](https://www.erlang.org/doc/system/eff_guide_processes.html)
- [Distributed Erlang](https://www.erlang.org/docs/27/system/distributed.html)
- [Erlang distribution protocol](https://www.erlang.org/docs/29/apps/erts/erl_dist_protocol.html)
- [WASI 0.3](https://wasi.dev/releases/wasi-p3)
- [WebAssembly threads draft](https://webassembly.github.io/threads/core/)
