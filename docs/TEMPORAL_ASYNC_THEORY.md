# Temporal Types with Async: Theoretical Foundation

## Core Challenge

How do temporal constraints work across asynchronous boundaries?

## Key Insights

### 1. **Futures Capture Temporals**
A `Future` must capture the temporal constraints of its result:
```
Future<T<~t>, ~completion>
```
- First parameter: The value type with its temporal
- Second parameter: When the future itself completes

### 2. **Await Transfers Temporals**
```
await : Future<T<~t>, ~now> → T<~t>
```
The temporal `~t` is preserved through await, but must still be within its original scope.

## Extended Rules for Async

### Rule A1: Future Creation (T-Async)
```
Γ, ~now ⊢ e : T<~t>    ~t within ~now
─────────────────────────────────────── (T-Async)
Γ ⊢ async { e } : Future<T<~t>, ~now>
```
An async block captures temporals from its creation context.

### Rule A2: Await (T-Await)
```
Γ ⊢ e : Future<T<~t>, ~fut>    ~fut within ~current
──────────────────────────────────────────────────── (T-Await)
Γ ⊢ await(e) : T<~t>
```
Await extracts the value, preserving its temporal constraints.

### Rule A3: Spawn with Temporal (T-Spawn)
```
Γ, ~task ⊢ e : T    ~task within ~parent
────────────────────────────────────────── (T-Spawn)
Γ, ~parent ⊢ spawn { e } : Future<T, ~task>
```
Spawned tasks have their own temporal scope within the parent.

### Rule A4: Temporal Cancellation (T-Cancel)
```
future : Future<T<~t>, ~fut>    drop(future)
─────────────────────────────────────────── (T-Cancel)
cleanup(~t) if future was holding last reference
```
Dropping a future triggers cleanup of captured resources.

## Async Temporal Patterns

### Pattern 1: Borrowed Async
```restrict
// Resource borrowed across async
fun processAsync<~res> = resource: Resource<~res> {
    async {
        resource.process();  // OK: ~res still valid
        delay(100) |> await;
        resource.finish()    // OK: ~res still valid
    }
}
```

### Pattern 2: Owned Async
```restrict
// Resource moved into async
fun processOwned = {
    with Resources {
        val res = acquire();  // res: Resource<~ctx>
        spawn {
            res.process();    // res moved into spawn
            res.cleanup()     // spawn owns cleanup
        }
    }  // Parent doesn't cleanup res (moved)
}
```

### Pattern 3: Structured Concurrency
```restrict
// Parent scope owns all child temporals
fun parallel<~batch> = items: List<Item> {
    with Async<~batch> {
        val futures = items.map(|item| {
            spawn<~task> {    // ~task within ~batch
                item.process()
            }
        });
        
        futures |> Future.all |> await
    }  // All ~task cleaned up with ~batch
}
```

## Temporal Flow Analysis

### 1. **Temporal Escape in Async**
```restrict
// ERROR: Temporal escape
fun leak<~io> = {
    val file = fs.open("test");  // file: File<~io>
    async {
        file.read()  // ERROR: ~io may not be valid when future runs
    }
}

// OK: Temporal captured properly
fun capture<~io> = file: File<~io> {
    async {
        file.read()  // OK: ~io in signature
    }
}
```

### 2. **Temporal Joins**
```restrict
// Multiple temporals must be compatible
fun join<~a, ~b> = 
    f1: Future<T<~a>, ~x>
    f2: Future<U<~b>, ~y>
where ~x within ~a, ~y within ~b {
    async {
        val t = f1 |> await;  // t: T<~a>
        val u = f2 |> await;  // u: U<~b>
        process(t, u)         // OK if ~a and ~b still valid
    }
}
```

## Safety Properties for Async

### Property A1: No Async Use-After-Free
**Theorem**: If `await future` returns `T<~t>`, then `~t` is still valid.

**Proof sketch**: 
- By T-Await, `~fut within ~current`
- By T-Async, captured temporals must be valid at creation
- Cleanup deferred until all futures complete

### Property A2: Cancellation Safety
**Theorem**: Dropping a future cleans up owned resources but not borrowed ones.

**Proof sketch**:
- Owned resources: Reference count drops to 0, cleanup triggered
- Borrowed resources: Other references exist, no cleanup

### Property A3: Structured Concurrency
**Theorem**: If `~child within ~parent`, all child tasks complete before parent cleanup.

**Proof sketch**:
- By temporal containment rules
- Parent scope waits for all ~child completions
- Cleanup order preserved

## Implementation Strategy

### 1. Future Representation
```rust
struct Future<T> {
    value_type: Type,
    value_temporal: TemporalVar,
    completion_temporal: TemporalVar,
    state: FutureState<T>,
}
```

### 2. Await Transformation
```
await(future) =>
    match future.poll() {
        Ready(value) => value,
        Pending => {
            suspend_with_temporal(future.temporals);
            retry
        }
    }
```

### 3. Cleanup Scheduling
```
on_scope_exit(~t):
    wait_for_futures_with_temporal(~t);
    cleanup_resources_with_temporal(~t);
```

## Open Research Questions

1. **Temporal Inference in Async**: How much can be inferred vs explicit?
2. **Deadlock Prevention**: Can temporal ordering prevent async deadlocks?
3. **Temporal Optimizations**: Can we optimize cleanup based on temporal analysis?
4. **Distributed Temporals**: How do temporals work across network boundaries?

## Next Steps

1. Implement Future type with temporal parameters
2. Add await transformation to compiler
3. Create async temporal test suite
4. Prove safety properties formally

This theoretical foundation ensures that Restrict's temporal types provide the same safety guarantees in async code as in sync code!