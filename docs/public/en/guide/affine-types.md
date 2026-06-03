# Affine Types

Affine ownership is the rule that a value may be used at most once unless the
type is copyable or the binding is mutable. Restrict uses this rule to make
resource flow explicit without requiring a garbage collector.

## Copyable And Affine Values

The current copyable primitives are `Int32`, `Int64`, `Float64`, `Boolean`,
`Char`, and `()`. These values can be reused because copying them is cheap and
does not duplicate heap ownership.

```restrict
fun main: () -> Int32 = {
    val score: Int32 = 40
    score + score
}
```

Heap-backed values are affine by default. This includes `String`, `List<T>`,
records, function values, and resource-like values.

```restrict
fun consume_message: (message: String) -> Int32 = {
    1
}

fun main: () -> Int32 = {
    val message = "ready"
    message |> consume_message
}
```

After an affine value is passed to a consuming function, that binding is no
longer available.

## Mutable Bindings

Use `mut val` when a local binding needs repeated assignment or repeated use.

```restrict
fun main: () -> Int32 = {
    mut val counter = 0
    counter = counter + 1
    counter = counter + 2
    counter
}
```

`mut val` is a local choice. It should not replace API designs that naturally
move values through a pipeline.

## Functions Consume Arguments

Function arguments are values. Passing an affine value transfers ownership to
the function.

```restrict
record Payload {
    code: Int32,
    label: String
}

fun payload_code: (payload: Payload) -> Int32 = {
    payload.code
}

fun main: () -> Int32 = {
    val payload = Payload { code: 7, label: "release" }
    payload |> payload_code
}
```

Copyable fields such as `Int32` can be returned freely, but the record itself is
still consumed by `payload_code`.

## Return Values

Functions that should preserve a heap-backed value return the next value in the
chain.

```restrict
record Ticket {
    severity: Int32,
    owner: String
}

fun lower_severity: (ticket: Ticket) -> Ticket = {
    ticket.clone { severity: 1 }
}

fun read_severity: (ticket: Ticket) -> Int32 = {
    ticket.severity
}

fun main: () -> Int32 = {
    val ticket = Ticket { severity: 5, owner: "ops" }
    val lowered = ticket |> lower_severity
    lowered |> read_severity
}
```

Record updates use `.clone { field: value }`. Clone updates are postfix record
updates, not function calls.

## Type-Directed Impl Functions

`impl` groups functions under a receiver type for type-directed dispatch. These
functions are still called with OSV syntax.

```restrict
record Credit {
    amount: Int32
}

record Debit {
    amount: Int32
}

impl Credit {
    fun signed_amount: (self: Credit) -> Int32 = {
        self.amount
    }
}

impl Debit {
    fun signed_amount: (self: Debit) -> Int32 = {
        0 - self.amount
    }
}

fun main: () -> Int32 = {
    val entry = Debit { amount: 12 }
    (entry) signed_amount
}
```

The first argument supplies the receiver type; selection is type-directed and
the call remains OSV.

## Affine Resources

Affine values are useful for one-shot resources, tokens, and state transitions.

```restrict
record Token {
    id: Int32
}

fun use_token: (token: Token) -> Int32 = {
    token.id
}

fun main: () -> Int32 = {
    val token = Token { id: 99 }
    token |> use_token
}
```

The token cannot be spent again after it moves into `use_token`.

## State Machines

Different record types can encode valid states, and functions move ownership
from one state to the next.

```restrict
record ClosedConnection {
    id: Int32
}

record OpenConnection {
    handle: Int32
}

fun connect: (closed: ClosedConnection) -> OpenConnection = {
    OpenConnection { handle: closed.id }
}

fun send: (open: OpenConnection, bytes: Int32) -> OpenConnection = {
    OpenConnection { handle: open.handle + bytes }
}

fun close: (open: OpenConnection) -> ClosedConnection = {
    ClosedConnection { id: open.handle }
}

fun main: () -> ClosedConnection = {
    val closed = ClosedConnection { id: 1 }
    val opened = closed |> connect
    val sent = (opened, 5) send
    sent |> close
}
```

There is no value left that can be sent after the connection is closed.

## Best Practices

1. Prefer left-to-right OSV pipelines for ownership transfer.
2. Use copyable primitives freely, but treat heap-backed values as affine.
3. Use `mut val` for local mutation only when it clarifies the code.
4. Return the next value when a function transforms an affine resource.
5. Use `.clone { ... }` deliberately when a record update needs a fresh value.

## See Also

- [Variables and Mutability](variables.md) - How affine types affect bindings
- [Type System](types.md) - Copyable primitives and heap-backed affine values
- [OSV Word Order](osv-order.md) - How calls express ownership flow
