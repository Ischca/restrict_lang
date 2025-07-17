# Async/Concurrency Design for Restrict Language

## Overview

This document explores async/concurrency models suitable for Restrict Language, considering its unique features: affine types, OSV syntax, zero-GC, and context-based resource management.

## Language Constraints & Opportunities

### Constraints:
- **Affine types**: Values can be used at most once
- **No GC**: Manual memory management via arenas
- **WASM target**: Limited threading support (SharedArrayBuffer/Atomics)
- **No runtime**: Lightweight execution model

### Opportunities:
- **Affine types prevent data races**: No shared mutable state by default
- **Context blocks**: Natural scope for async operations
- **OSV syntax**: Could make async chains more readable
- **Explicit resource management**: Perfect for async resource handling

## Proposed Models

### 1. **Affine Futures** (Inspired by Rust, adapted for affine types)

```restrict
// Future is consumed when awaited (affine!)
type Future<T> = {
    poll: Self -> PollResult<T>
}

type PollResult<T> = 
    | Ready(T)
    | Pending(Future<T>)  // Returns new future for next poll

// Usage with OSV syntax
fun fetchUser = userId: Int32 {
    val future = (userId) http.get("/users/{id}");
    future  // Future is returned, not yet executed
}

fun main = {
    val userFuture = (123) fetchUser;
    val user = userFuture await;  // Future consumed here
    user print;
}
```

### 2. **Linear Channels** (CSP-style, leveraging affine types)

```restrict
// Channel endpoints are affine - ensures proper cleanup
record Channel<T> {
    sender: Sender<T>
    receiver: Receiver<T>
}

// Creating a channel splits it into affine parts
fun createChannel<T> = {
    // Implementation returns (Sender<T>, Receiver<T>)
}

// Usage
fun worker = receiver: Receiver<Int32> {
    receiver receive match {
        Some(value) => {
            value process;
            receiver worker;  // Recursive call with receiver
        }
        None => { unit }  // Channel closed
    }
}

fun main = {
    val (sender, receiver) = createChannel();
    
    // Spawn worker (consumes receiver)
    (receiver) spawn(worker);
    
    // Send values (sender is affine, must be used linearly)
    (42) sender.send;
    (84) sender.send;
    sender.close;  // Sender consumed
}
```

### 3. **Effect Handlers** (Novel approach for Restrict)

```restrict
// Effects are declared as contexts
context Async {
    await: Future<T> -> T
    spawn: (fn() -> T) -> Future<T>
    parallel: List<Future<T>> -> Future<List<T>>
}

// Handlers provide implementation
handler AsyncHandler for Async {
    // Implementation details
}

// Usage combines with existing context system
fun fetchData = {
    with Async {
        val user = ("/users/123") http.get |> await;
        val posts = ("/posts?user=123") http.get |> await;
        (user, posts)
    }
}
```

### 4. **Session Types** (Leveraging affine types for protocol safety)

```restrict
// Session types ensure protocol compliance
type ClientSession = 
    | SendRequest(Request) -> AwaitResponse
    | Close

type AwaitResponse = 
    | ReceiveResponse(Response) -> ClientSession

// Usage ensures protocol is followed
fun httpClient = session: ClientSession {
    session match {
        SendRequest(cont) => {
            val request = Request { method: "GET", path: "/" };
            val awaitSession = (request) cont;
            awaitSession handleResponse
        }
        Close => { unit }
    }
}
```

### 5. **Coroutine Contexts** (Kotlin-inspired, Restrict-adapted)

```restrict
// Coroutine context manages execution
context Coroutine {
    suspend: fn() -> Unit
    resume: Unit -> Unit
    yield: T -> Unit
}

// Structured concurrency via context nesting
fun processItems = items: List<Item> {
    with Coroutine {
        items |> forEach(|item| {
            with Coroutine {  // Child coroutine
                item process;
                yield;  // Cooperative scheduling
            }
        })
    }
}
```

### 6. **Actor Model** (Affine actors)

```restrict
// Actors own their state (affine)
record Actor<State, Msg> {
    state: State
    handler: (State, Msg) -> (State, Response)
    mailbox: Receiver<Msg>
}

// Creating an actor consumes the initial state
fun createActor<State, Msg> = 
    initialState: State 
    handler: (State, Msg) -> (State, Response) {
    val (sender, receiver) = createChannel();
    val actor = Actor {
        state: initialState
        handler: handler
        mailbox: receiver
    };
    (actor) spawn(runActor);
    sender  // Return only the sender
}

// Usage
fun counter = {
    val counterActor = (0, |state, msg| {
        msg match {
            Increment => (state + 1, Ok)
            Get => (state, Value(state))
        }
    }) createActor;
    
    Increment counterActor.send;
    Get counterActor.send;
}
```

## Recommended Approach: Hybrid Model

Combine the best aspects:

### 1. **Core: Affine Futures + Linear Channels**
- Futures for single async values
- Channels for streams and communication
- Both leverage affine types for safety

### 2. **High-level: Effect Handlers**
- Use context system for structured concurrency
- Provides clean syntax via `with` blocks
- Allows different async strategies

### 3. **OSV-Optimized Syntax**

```restrict
// Sequential async with OSV
fun fetchUserWithPosts = userId: Int32 {
    with Async {
        val user = ("/users/{userId}") http.get |> await;
        val posts = ("/posts?user={userId}") http.get |> await;
        UserWithPosts { user: user, posts: posts }
    }
}

// Parallel async with OSV
fun fetchParallel = urls: List<String> {
    with Async {
        urls 
        |> map(|url| (url) http.get)  // Create futures
        |> parallel                    // Wait for all
        |> await
    }
}

// Actor-style with OSV
fun startServer = port: Int32 {
    val server = (port) tcp.listen;
    
    server.accept 
    |> spawn(|conn| {
        conn handleConnection
    })
    |> supervise(RestartOnError)
}
```

## Implementation Strategy

### Phase 1: Foundation
1. Implement basic Future type
2. Add compiler support for async/await transformation
3. Create simple event loop for WASI

### Phase 2: Channels
1. Implement linear channels
2. Add `spawn` for green threads
3. Create channel-based primitives

### Phase 3: Effect System
1. Extend context system for effects
2. Implement Async effect handler
3. Add structured concurrency

### Phase 4: Advanced Features
1. Session types for protocols
2. Actor model implementation
3. Supervision trees

## Novel Ideas for Restrict

### 1. **Temporal Affine Types**
```restrict
// Values have temporal bounds
temporal<'t> record Connection {
    socket: Socket
}

fun handleRequest = conn: Connection<'t> {
    with lifetime<'t> {
        // Connection only valid within this scope
        conn.read |> process |> conn.write
    }  // Connection automatically closed
}
```

### 2. **Async Resource Contexts**
```restrict
context AsyncResource<T> extends Async {
    resource: T
    cleanup: T -> Unit
}

// Automatic cleanup after async operations
with AsyncResource<Database> {
    val result = query("SELECT * FROM users") |> await;
    // Database cleaned up even if await fails
}
```

### 3. **Dataflow Variables** (Oz-inspired)
```restrict
// Single-assignment async variables
type Flow<T> = {
    bind: T -> Unit      // Can only be called once (affine)
    wait: Unit -> T      // Blocks until bound
}

fun dataflow = {
    val (flow, binder) = createFlow();
    
    spawn(|| {
        val result = expensiveComputation();
        (result) binder.bind;  // Binder consumed
    });
    
    // Can be read multiple times before binding
    val value = flow.wait();
}
```

## Advantages of This Design

1. **Memory Safety**: Affine types prevent data races
2. **Resource Safety**: Automatic cleanup via affine types
3. **Protocol Safety**: Session types ensure correct communication
4. **Composability**: Effect handlers allow mixing strategies
5. **Ergonomics**: OSV syntax makes async chains readable

## Open Questions

1. How to handle cancellation with affine types?
2. Should we support work-stealing schedulers?
3. How to integrate with WASM's future threading proposal?
4. Can we make debugging async code easier with affine types?
5. Should async be a language feature or library feature?