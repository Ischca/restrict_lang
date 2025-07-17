# Async Design with Temporal Affine Types (TAT)

## Overview

This document explores async/concurrent programming models for Restrict Language, leveraging the newly implemented Temporal Affine Types (TAT) for safe and expressive async operations.

## Design Options

### Option 1: Explicit Future with TAT

```restrict
// Future is temporal - bound to async context lifetime
record Future<T, ~async> {
    poll: () -> PollResult<T>
}

enum PollResult<T> {
    Ready(T),
    Pending
}

fun fetchUser<~async> = userId: Int32 -> Future<User, ~async> {
    // Create future bound to ~async lifetime
    http.get("/users/{userId}")
}

fun main = {
    with lifetime<~async> {
        val userFuture = (123) fetchUser;
        
        // Poll until ready (simplified)
        loop {
            userFuture.poll() match {
                Ready(user) => break user,
                Pending => yield()
            }
        }
    }
}
```

**Pros:**
- Explicit control over async operations
- Clear lifetime boundaries
- Works well with OSV syntax

**Cons:**
- Manual polling is verbose
- Need runtime/executor support

### Option 2: async/await with TAT

```restrict
// async functions automatically get temporal parameter
async fun fetchUser<~async> = userId: Int32 -> User {
    val response = http.get("/users/{userId}") await;
    response.parseUser()
}

fun main = {
    with lifetime<~async> {
        // await consumes the async computation
        val user = (123) fetchUser |> await;
        user.name |> println
    }
}
```

**Pros:**
- Familiar syntax from other languages
- Cleaner than manual polling
- Natural with pipe operator

**Cons:**
- Requires special syntax support
- May hide complexity

### Option 3: Effect Handlers with TAT

```restrict
// Async as an effect with temporal bounds
context Async<~async> {
    spawn: (() -> T) -> Future<T, ~async>
    await: Future<T, ~async> -> T
    parallel: List<Future<T, ~async>> -> Future<List<T>, ~async>
}

fun fetchData<~async> = {
    with Async<~async> {
        val user = spawn(|| http.get("/users/123")) |> await;
        val posts = spawn(|| http.get("/posts?user=123")) |> await;
        (user, posts)
    }
}
```

**Pros:**
- Composable and extensible
- Clear separation of concerns
- Works with existing context system

**Cons:**
- More complex to implement
- May be unfamiliar to users

### Option 4: Temporal Channels (CSP-style)

```restrict
// Channels bound to communication lifetime
record Channel<T, ~ch> {
    send: T -> ()
    receive: () -> Option<T>
}

fun worker<~ch> = input: Channel<Task, ~ch>, output: Channel<Result, ~ch> {
    loop {
        input.receive() match {
            Some(task) => {
                val result = task.process();
                output.send(result)
            },
            None => break
        }
    }
}

fun main = {
    with lifetime<~work> {
        val (taskSend, taskRecv) = Channel.create();
        val (resultSend, resultRecv) = Channel.create();
        
        // Spawn worker with channels
        spawn(|| worker(taskRecv, resultSend));
        
        // Send tasks
        taskSend.send(Task { id = 1 });
        
        // Receive results
        resultRecv.receive()
    }
}
```

**Pros:**
- Natural concurrency model
- No shared state
- Clear communication patterns

**Cons:**
- Different from async/await
- Requires goroutine-like runtime

## Recommendation: Hybrid Approach

Combine the best aspects:

1. **Core Model**: Effect handlers with TAT for flexibility
2. **Sugar Syntax**: async/await for common cases
3. **Channels**: For communication between concurrent tasks

```restrict
// Core effect handler
context AsyncRuntime<~async> {
    spawn: (() -> T) -> Task<T, ~async>
    await: Task<T, ~async> -> T
    channel: () -> (Sender<T, ~async>, Receiver<T, ~async>)
}

// Sugar: async function desugars to effect
async fun fetchUser<~async> = userId: Int32 -> User {
    // Desugars to: with AsyncRuntime<~async> { ... }
    http.get("/users/{userId}") await
}

// Usage combines both
fun processUsers<~async> = userIds: List<Int32> {
    with AsyncRuntime<~async> {
        val (sender, receiver) = channel();
        
        // Spawn workers
        userIds |> forEach(|id| {
            spawn(|| {
                val user = (id) fetchUser await;
                sender.send(user)
            })
        });
        
        // Collect results
        userIds |> map(|_| receiver.receive())
    }
}
```

## Integration with TAT

Key benefits of using TAT for async:

1. **Automatic Cleanup**: Tasks cancelled when lifetime ends
2. **No Leaks**: Futures can't escape their async context  
3. **Structured Concurrency**: Natural parent-child relationships
4. **Type Safety**: Can't mix tasks from different async contexts

## Next Steps

1. Implement core `AsyncRuntime` context
2. Add `Task<T, ~async>` type (similar to Future)
3. Implement `spawn` and `await` operations
4. Add syntactic sugar for `async`/`await`
5. Build channel abstractions on top