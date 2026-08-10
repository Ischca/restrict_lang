# Restrict Language Specification v1.0

**THE DEFINITIVE SOURCE OF TRUTH FOR RESTRICT LANGUAGE SYNTAX**

This document is the **single authoritative specification** for Restrict Language. All other documentation is superseded by this specification. Any conflicts with other documentation should be resolved by referring to this document.

## Language Philosophy

- **OSV (Object-Subject-Verb)**: Natural function composition: `value |> function`
- **Affine Types**: Each variable can be used at most once (unless marked mutable)
- **Temporal Affine Types (TAT)**: Planned/experimental automatic resource cleanup with temporal scopes; outside the default v0.0.1 gate
- **No Side Effects**: Expression statements must be pure
- **Arena Memory**: Deterministic memory management without garbage collection

## Compilation and Host Model

Restrict has one code-generation family: WebAssembly. JavaScript, native
machine code, individual cloud platforms, and container runtimes are not
separate Restrict language backends. A host profile may select imports,
exports, ABI adapters, and packaging without changing Restrict source semantics.

Core language behavior must remain independent of JavaScript objects, DOM APIs,
specific cloud bindings, Docker, and individual WebAssembly runtimes. External
operations are provided by explicit host imports or capabilities. This keeps a
Restrict program portable between native WASI runtimes, component hosts,
browsers, and edge environments when those hosts provide compatible
interfaces.

The current v0.0.1 program output is a Core WebAssembly module. It imports the
WASI Preview 1 `fd_write` and `proc_exit` operations for basic program I/O, and
a zero-argument `main` receives the `_start` wrapper specified below. The
browser playground supplies equivalent behavior through a JavaScript bridge.
That generated or handwritten bridge is host glue, not a JavaScript code-
generation backend.

The ordinary v0.0.1 compiler pipeline remains `.rl -> WAT -> Core Wasm`. This
branch also implements an explicit B1 Component path. In a Warder project, a
schema-v1 or schema-v2 selection from the repository-root `warder.toml` is the
default artifact for `warder build`; `--component` asserts that selection and
`--core` explicitly requests the Core Wasm/Cage artifact. The selection is
checked and monomorphized
into `ComponentInterfaceIr`,
normalized WIT and stable ABI metadata, Canonical ABI adapters, and a validated
WebAssembly Component. Warder publishes the resulting `.component.wasm`,
`.component.wit`, and `.component.toml` files as one transactional artifact
set. `warder run` and the current publish preflight are Core-only and report
that they force the Core profile when `[component]` is present. This project
default does not change the compiler's default Core Wasm ABI, and the
repository is versioned and published as stable `0.1.0`.

External standards and third-party contracts use the complementary WIT-first
path. Rich synchronous guest imports are implemented and exercised by the
Rust-produced Component -> Restrict consumer gate. Restrict implementation of
an externally owned interface is currently implemented for the scalar slice;
rich-value implementation/export and external-WIT resources fail closed with
typed diagnostics. Warder `wrap --wit` remains a different operation: it
stores caller-supplied WIT text in a Core module cage and does not generate or
validate a Component.

A Warder-managed repository has one authored configuration file,
`warder.toml`, at its root. The current implementation describes one
publishable root package plus any direct local source packages selected by the
root `[dependencies]` table. Those local packages need `src/lib.rl`; they do
not carry leaf manifests or leaf lock files. Warder generates one
repository-root `warder.lock` for dependency resolutions that cannot be
reconstructed deterministically from the repository sources. Registry, Git,
foreign-Wasm, and transitive dependency resolution currently fail closed, so a
local-only build has no resolved package entries in that lock.
`warder.lock` is generated rather than hand-authored and is committed when it
exists.

All Warder commands accept `--project DIR`, which selects a project without
requiring the caller to change its shell working directory. `warder build
--watch` performs an initial build, then rebuilds when the root
`warder.toml`, root `.rl` sources, or `.rl` sources in a configured direct
local dependency change. Watch mode does not add transitive dependency
resolution, an incremental compiler, or a JavaScript package/bundler graph.

`warder test` compiles test files under `tests/` to WASI Preview 1 Core Wasm and executes
source-public, synchronous, non-generic, zero-argument Boolean functions whose
names begin with `test_` or end with `_test`. A return value of `true` passes;
`false`, a trap, an invalid Boolean representation, a compilation error, or a
missing export fails. The optional filter matches function names. This is a
Warder convention over ordinary Restrict functions, not a separate language
declaration syntax. During test compilation, the reserved package namespace
`project` maps to the directory containing `package.entry`, so a default
project can import its implementation with `import project.main.{name}`.
Dependency aliases may not use `project`.

The generated `dist/*.component.toml` files are Component artifact metadata,
not additional Warder configuration. Likewise, Cage-internal `manifest.toml`
and temporary Warder build/recovery state are not project manifests. A future
multi-publish workspace graph will extend the same root `warder.toml`; it must
not introduce member or package leaf manifests. Directory placement may help
Warder discover local sources, but public package identity and Component ABI
identity remain explicit, path-independent contract data.

The compiler exposes two artifact target profiles. `wasip1` is the default and
retains the v0.0.1 program-I/O imports. `wasm-core` emits an import-free Core
WebAssembly module for host-neutral workloads and rejects `print`, `println`,
and other host-I/O calls. Both profiles use the same Restrict source semantics.
The native CLI can encode and validate binary output directly with `--emit
wasm`; text output remains available with `--emit wat`.

Compiler output is unoptimized by default so the complete lowered module stays
available for debugging. `--release` applies the compiler's deterministic
reachability pass after WAT lowering: host-visible exports and the start entry
are roots, direct calls are followed transitively, and unused functions,
function imports, named types, globals, tables, and element segments are
removed when they cannot affect those roots. This release pass does not yet
rewrite instructions or invoke an external `wasm-opt` binary.

Compiler-managed arenas default to 4096 bytes for compatibility. A build may
select a larger multiple-of-four capacity with `--arena-bytes`; this changes
reserved linear-memory capacity, not source semantics or host ABI. Arena
exhaustion deliberately traps and must not be treated as a recoverable
source-level error. Before trapping, generated modules expose a compiler-owned
diagnostic through the `__restrict_arena_error_code`,
`__restrict_arena_error_requested_bytes`, and
`__restrict_arena_capacity_bytes` `i32` globals. Error code `0` means no arena
error, `1` means exhaustion, and `2` means allocation was attempted without an
active arena. Initializing a new arena clears the error code and requested-byte
value. A host may inspect these values after a trap to classify the failure,
but must not resume the trapped call.

`--instrument-memory` additionally exports compiler-owned peak-live-byte,
current-live-byte, allocation-count, and reset-count globals plus
`__restrict_memory_metrics_reset`. This instrumentation is for benchmarks and
diagnostic hosts; it changes generated bookkeeping and therefore must not be
used as the timed artifact in a performance comparison. Peak and live byte
counts exclude the arena header. The current counters describe the B0
single-host-entry arena path and do not claim aggregate accounting for nested
arenas.

The version boundary is:

- **v0.1.0 / B1**: the stable synchronous Component Model implementation:
  scalar and rich values (`String`, `List`, `Array`,
  records, `Option`, `Result`, and user enums/variants), synchronous resources,
  value-capturing once-callable resources, and explicitly selected concrete
  generic function instances; generated and consumed WIT; Canonical ABI
  lift/lower adapters; validated packaging, compatibility diffing,
  composition, Rust bindings, and pinned-runtime execution;
- **v0.2.0**: async functions, `await`, structured `spawn`, futures, streams,
  cancellation, backpressure, generic async capability adapters, and
  deterministic resource cleanup across suspension; and
- later generated Web/cloud adapters and direct browser host or DOM interfaces
  if and when portable standards expose them to WebAssembly.

The normative implemented contract and its intentional limits are recorded in
`docs/V010_SYNCHRONOUS_COMPONENT_ABI.md`. B1 implementation completion on this
branch is distinct from publishing a versioned v0.1 release.

Docker, containerd, and similar systems may execute or package a WASI artifact,
but do not define a Restrict ABI. Likewise, a browser or cloud host may require
JavaScript glue today without requiring Restrict to compile source code to
JavaScript.

## 1. Lexical Elements

### 1.1 Keywords (Reserved)
```
fun val mut record context enum match then else while
temporal within where clone freeze pub import export
impl as fatal true false Some None with lifetime await spawn
form takes of
```

Some reserved words are for planned or experimental features. A word being
reserved does not imply that every related syntax form is part of the current
v0.0.1 implementation.

The v0.0.1 release exposes the deliberately small, method-only `form` /
`takes` / `of` slice specified below. Generic forms, associated types, default
methods, generic or conditional adoptions, enum adoptions, and dynamic dispatch
remain future work.

The v0.0.1 release includes compiler-provided `Option<T>` and `Result<T, E>`
sum types plus the closed, non-generic user-defined `enum` slice specified below.
Generic enums, recursive enums, and variants with more than one direct payload
remain future work.
Host-visible WebAssembly exports that would require an exported
generic/composite host ABI, including exported generic functions or direct
exported record values, remain outside default v0.0.1 until that ABI is
implemented and passes the v0.1.0 Component ABI gate.

### 1.2 Operators
```
|>      // Pipe operator (primary)
=       // Assignment
=>      // Match arrow
->      // Function return type arrow
+  -    // Arithmetic
*  /  % // Arithmetic
== !=   // Equality
<  <=   // Comparison
>  >=   // Comparison
&&  ||  // Logical
!       // Logical not
~       // Temporal marker
::      // Type/variant namespace separator
```

### 1.3 Delimiters
```
{ }     // Block/Record delimiters
( )     // Expression/Parameter grouping
[ ]     // List/Array literals
, ;     // Separators
: .     // Type annotation, field access
::      // Type/variant namespace separator
```

### 1.4 Literals
- **Integers**: `42`, `0xFF`, `1_000_000`
- **Floats**: `3.14`, `1.5e10`, `3.14E-2`
- **Strings**: `"hello"`, with escapes `\n \t \\ \" \'`
- **Characters**: `'a'`, `'\n'`
- **Booleans**: `true`, `false`
- **Unit**: `()`

### 1.5 Comments
- **Single-line**: `// comment`
- **Multi-line**: `/* comment */` (no nesting)

### 1.6 Whitespace and Expression Boundaries

Spaces, tabs, comments, and line breaks are ordinary whitespace. A line break
does not terminate an expression. The parser reads the maximal expression
allowed by the grammar, including a direct OSV call split across lines:

```rust
val answer = 41
    increment
// Equivalent to: val answer = 41 increment
```

A direct OSV chain continues only while the following expression can serve as
a verb (a function value). A literal, record, or collection value cannot be a
verb, so it naturally begins a new expression even without a semicolon:

```rust
"first" println
"second" println
```

A semicolon explicitly terminates the current statement when the following
source could instead be interpreted as another verb in the same OSV chain:

```rust
val message = "ready";
message println
```

Reserved declaration keywords, delimiters, operators, and values that cannot
act as verbs still establish the boundaries required by their grammar, so
semicolons are not required after every declaration or unambiguous expression.
A block's final expression is not followed by a semicolon. Top-level
declarations are never separated by semicolons.

An explicit semicolon is appropriate when a local `val` deliberately stages a
named value before an identifier-started expression. When the name exists only
inside a higher-order transformation, a scoped verb clause can keep the same
flow semicolon-free by naming the callback input instead:

```rust
values map { |value|
    value + 1
}
```

## 2. Variable Declarations

### 2.1 Immutable Variables
```rust
val x = 42              // Immutable binding
val name = "Alice"      // Type inferred
val pi: Float64 = 3.14  // Explicit type
```

### 2.2 Mutable Variables
```rust
mut val counter = 0     // Mutable binding (mut before val)
mut val items: List<String> = []  // With type annotation
```

**CRITICAL**: The syntax is `mut val`, NOT `val mut`. This is enforced by the parser.

## 3. Function Declarations

### 3.1 Standard Function Syntax
```rust
fun name: (param: Type, ...) -> ReturnType = {
    // body
}

// Examples:
fun add: (x: Int32, y: Int32) -> Int32 = {
    x + y
}

fun greet: (name: String) = {  // Return type inferred
    "Hello, " + name
}

fun main: () = {  // No parameters
    42
}
```

Parameter binders within one function or method signature must be unique.
The same rule applies to form contracts and their `takes` implementations.

### 3.2 Generic Functions
```rust
fun identity: <T>(value: T) -> T = {
    value
}

fun map: <T, U>(list: List<T>, f: T -> U) -> List<U> = {
    // implementation
}

// `of` requires explicit form adoptions. Multiple forms use `+`.
fun render: <T of Display>(value: T) -> String = {
    value |> display
}

fun compare_rendered: <T of Display + Comparable>(left: T, right: T) -> Int32 = {
    (left, right) compare
}
```

`of` appears on a declared type parameter, not on an individual value
parameter. A concrete argument must have a matching adoption for every listed
form. Prelude adoptions may be compiler-provided; source records adopt forms
explicitly. Form bounds do not introduce subtyping or implicit structural
conformance.

### 3.3 Temporal Functions (Experimental / Outside v0.0.1 Default Gate)
```rust
fun process: <~t>(data: Data<~t>) -> Result<Data<~t>, Error> = {
    data |> validate |> transform
}
```

## 4. Types

### 4.1 Basic Types
- `Int32`, `Int64`, `Float64`
- `String`, `Char`, `Boolean`
- `()` (Unit type)

### 4.2 Generic and Built-in Collection Types
```rust
List<T>           // Dynamic list
Array<T, N>       // Fixed-size array
Option<T>         // Maybe value
Result<T, E>      // Success or error
Range<Int32>      // v0.0.1 range values with Int32 endpoints
```

`Array<T, N>` is a fixed-length public type. The compiler may use an internal
wildcard length for built-in array operations, but that internal representation
is not a source-level `Array<T, 0>` contract.

Range literals are intentionally concrete in v0.0.1. Ranges over non-Int32
endpoint types are outside the current public support surface.

### 4.3 Temporal Types (Experimental / Outside v0.0.1 Default Gate)
```rust
File<~f>          // File with temporal scope ~f
Connection<~db>   // Database connection with scope ~db
```

### 4.4 Function Types
```rust
Int32 -> String         // Function type
(Int32, String) -> Boolean // Multi-parameter function
```

### 4.5 User-Defined Enum Types

```rust
pub enum ParseError {
    Empty
    Message(String)
}
```

An enum is a closed tagged sum. Variant tags follow declaration order starting
at zero, but tag values and payload offsets are compiler-internal details and
are not a source or host ABI.

The current slice has these constraints:

- an enum has at least one variant;
- a variant has either no payload or exactly one payload;
- multiple logical payload values must be wrapped in a record;
- enum declarations are non-generic and non-recursive;
- payload types are concrete, monomorphic, non-temporal, non-function types
  supported by the compiler's internal WebAssembly representation;
- variants are scoped under their enum name and are never injected as bare
  names; and
- `impl`, `clone`, `freeze`, match guards, first-class constructor values, and
  `==`/`!=` structural equality for enums are outside this slice. Use `match`
  on qualified variants instead of comparing enum allocation identities.

An enum value is Copy only when every payload type is Copy. A payload-free enum
is therefore Copy. Constructing a payload variant moves an affine payload, and
matching an enum consumes the scrutinee once under the ordinary affine rules.

## 5. Expressions

### 5.1 Literals and Variables
```rust
42              // Integer
3.14            // Float
"hello"         // String
'x'             // Character
true            // Boolean
()              // Unit
x               // Variable reference
```

### 5.2 Function Calls

#### OSV Style (Object-Subject-Verb) - ONLY SUPPORTED SYNTAX

```rust
// ✅ CORRECT: OSV syntax (Object-Subject-Verb)
value |> function           // Single argument via pipe
value function              // Single argument via direct OSV
(arg1, arg2) function       // Multiple arguments via tuple
() function                 // No arguments via unit

// ❌ COMPILE ERROR: Traditional function calls NOT supported
function(args)              // ERROR: Traditional syntax forbidden
function()                  // ERROR: Traditional syntax forbidden
object.method(args)         // ERROR: Traditional syntax forbidden
```

**CRITICAL RULE**: Restrict Language **exclusively** uses OSV syntax.
Ordinary value arguments always come BEFORE the function name. Traditional
parenthetical function calls `function(args)` will cause compilation errors.
The scoped verb clause in Section 5.9 is a separate OSV form: its trailing
block is a scope opened by the verb, not an ordinary value argument.
Whitespace, including line breaks, does not end a direct call. A following
value that cannot be a verb begins a new expression naturally. Use `;` when a
following callable-shaped expression must begin a new statement instead of
extending the OSV call, as specified in Section 1.6.

**OSV Pattern Examples:**
```rust
// Data flows left-to-right naturally
("hello, " + "Restrict") |> println

// Multiple arguments use tuple syntax
(10, 20) add                    // Invalid traditional form: add(10, 20)
(1, 2, 3, 4) sum_all           // Invalid traditional form: sum_all(1, 2, 3, 4)

// Complex expressions maintain clarity
val result = user_data
    |> validate_input
    |> transform_data
    |> save_to_database
    |> generate_response

// Method-like calls still use OSV
user.profile get_name          // Invalid traditional form: user.profile.get_name()
database.connection close      // Invalid traditional form: database.connection.close()
```

### 5.3 Binary Operations
```rust
x + y           // Addition
x - y           // Subtraction
x * y           // Multiplication
x / y           // Division
x % y           // Modulo
x == y          // Equality
x != y          // Inequality
x < y           // Less than
x <= y          // Less than or equal
x > y           // Greater than
x >= y          // Greater than or equal
x && y          // Logical and
x || y          // Logical or
```

### 5.4 Conditional Expressions
```rust
condition then {
    // true branch
} else {
    // false branch
}

// Example:
age >= 18 then { "adult" } else { "minor" }
```

### 5.5 Match Expressions
```rust
value match {
    pattern => { result }
    pattern => { result }
    _ => { default }
}

// Example:
x match {
    Some(v) => { v * 2 }
    None => { 0 }
}
```

### 5.6 List/Array Literals
```rust
[1, 2, 3]           // List literal
[1..10]             // Range (creates Range<Int32>)
[]                  // Empty list
```

**DEPRECATED**: `[|1, 2, 3|]` syntax is no longer supported.

### 5.7 Record Literals
```rust
Person { name: "Alice", age: 30 }
Point { x: 0, y: 0 }
```

### 5.8 Lambda Expressions
```rust
|x| x * 2           // Single parameter
|x, y| x + y        // Multiple parameters
|x: Int32| x + 1    // With type annotations
```

### 5.9 Scoped Verb Clauses

A function whose final remaining parameter is a function may open that
function's body as a trailing scope. The ordinary value arguments remain
before the verb:

```rust
val shifted = values map {
    it + 1
}

val total = (values, 0) fold { |sum, value|
    sum + value
}
```

This is a **scoped verb clause**, not a general trailing-argument rule. In the
first example, `values` is the ordinary object of `map`; `map` then opens a
scope focused on each element. The complete clause produces a value that can
become the object of the next clause:

```rust
values map {
    it + 1
} filter {
    it > 2
} |> list_count
```

Scoped verb clauses associate left-to-right. The example above applies
`filter` to the complete result of the `map` clause, then pipes the complete
filtered result to `list_count`.

The compiler elaborates scoped clauses through ordinary lambdas:

```rust
values map { it + 1 }
// elaborates as:
(values, |it| { it + 1 }) map

values map { |value| value + 1 }
// elaborates as:
(values, |value| { value + 1 }) map
```

Rules:

- An unheaded scope introduces the contextual focus binding `it` and therefore
  supplies exactly one lambda parameter.
- `it` is a contextual binding, not a globally reserved name. It exists only
  inside the implicit focus scope that introduces it.
- Explicit binders reuse the lambda parameter syntax at the start of the
  scope: `{ |value| ... }` or `{ |left, right| ... }`.
- A zero-parameter scope uses an explicit empty lambda header: `{ || ... }`.
- The clause supplies only the callable's final remaining parameter, and that
  parameter must have a function type. All earlier parameters use ordinary
  OSV object or tuple syntax.
- Nested implicit focus scopes are rejected because two active `it` bindings
  make affine use and capture intent unclear. Name at least one scope binder.
- Scope statements, the final result expression, captures, affine consumption,
  temporal escape checks, and type inference follow the ordinary block and
  lambda rules. The syntax does not grant captures additional uses.
- A scoped clause may execute its body zero, one, or many times according to
  the receiving function's contract. The braces express a lexical scope, not
  a guarantee of immediate or single execution.
- For the compiler-provided containers, `Option` callbacks execute at most
  once, while `List` callbacks used by `map`, `filter`, and `fold` may execute
  many times. A many-execution inline callback may capture only immutable Copy
  bindings. The checker rejects mutable or affine captures and retains that
  rejection when the lambda is first stored in a local callable binding. A
  callable parameter whose captures are opaque is also rejected in a repeated
  callback position until its replay safety can be proven.
- A repeated `while` condition or body likewise cannot consume an immutable
  affine binding from outside the loop. Mutable loop state and immutable Copy
  values remain reusable under their ordinary rules.

This form extends clause-level OSV rather than weakening it: a value precedes
the scope-opening verb, and the resulting complete clause precedes its next
verb. It follows the same value-then-verb-then-scope shape as `value match {
... }` and `condition then { ... }`.

#### Completed values and anonymous scopes

Application does not stop merely because its object was produced by a block,
`match`, `then`, `while`, or another completed clause. A completed value may
continue through direct OSV, a pipe, a named scoped verb, or an anonymous
scope:

```rust
positive match {
    true => { incremented }
    false => { 0 }
} print

positive match {
    true => { incremented }
    false => { 0 }
} |> print

positive match {
    true => { incremented }
    false => { 0 }
} { |value|
    value print
}
```

An anonymous scope is the nameless, exact-once counterpart of a scoped verb
clause. It elaborates to immediate application of a unary lambda:

```rust
value { it * it }
// elaborates as:
value |> (|it| { it * it })

value { |number| number * number }
// elaborates as:
value |> (|number| { number * number })
```

Rules:

- The object expression is evaluated exactly once, before the scope body.
- The scope body is evaluated exactly once and its result is the result of the
  complete expression. This form is result-producing, not an implicit `tap`.
- An unheaded anonymous scope binds the object as `it`. An explicit anonymous
  scope must declare exactly one binder. `{ || ... }` and
  `{ |left, right| ... }` are rejected with an anonymous-scope arity
  diagnostic in this position.
- The scope is an ordinary lexical lambda scope. It does not extend a temporal,
  arena, resource, or host-capability lifetime.
- Exact-once application permits the same affine captures as an ordinary
  immediate unary lambda call. The usual affine and temporal checks still
  apply inside the body.
- Application continuations associate left-to-right. For example,
  `values map { it + 1 } { |mapped| mapped print }` first completes `map`, then
  applies the anonymous scope to the mapped value.
- Newlines remain ordinary whitespace. Use `;` when a following callable-shaped
  expression must begin a separate expression rather than continue the current
  application chain.

The braces after a completed value denote an anonymous scope. Nominal record
construction remains `UpperCamelName { fields... }`; it is parsed as a record
literal before it can be considered a continuation target.

### 5.10 User-Defined Enum Construction

Enum constructors are qualified direct OSV call targets:

```rust
() ParseError::Empty
"invalid input" |> ParseError::Message
("invalid input") ParseError::Message
```

The payload-free constructor takes unit. A payload constructor takes exactly
one value of its declared payload type. A qualified constructor is not a
first-class value in this slice and must appear as the direct target of a call
or pipe. Traditional call order remains invalid:

```rust
ParseError::Message("invalid input") // ERROR: traditional call syntax
```

### 5.11 Built-in Option and Result Construction

`Option` and `Result` use the same qualified OSV constructor form as
user-defined enums. The pipe is optional, and a single direct object does not
need parentheses:

```rust
42 Option::Some
42 |> Option::Some
() Option::None
42 Result::Ok
error Result::Err
error |> Result::Err
```

The built-in namespaces are required in value expressions. The older
unqualified or traditional forms `Some(42)`, `None`, `Ok(42)`, and
`Err(error)` are invalid. Match patterns remain unqualified because they
decompose an already typed built-in sum value:

```rust
value match {
    Some(inner) => { inner }
    None => { 0 }
}
```

## 6. Patterns (for match expressions)

### 6.1 Basic Patterns
```rust
_               // Wildcard
x               // Variable binding
42              // Literal
true            // Boolean literal
"hello"         // String literal
```

### 6.2 Option Patterns
```rust
Some(x)         // Extract value from Some
None            // Match None
```

### 6.3 User-Defined Enum Patterns

```rust
error match {
    ParseError::Empty => { 0 }
    ParseError::Message(message) => { 1 }
}
```

Variant patterns are always qualified. A payload-free variant has no payload
pattern; a payload variant has exactly one nested pattern. Match exhaustiveness
is checked across all variants of the enum. Payload bindings obey the ordinary
affine-use rules, and an enum variant pattern is refutable, so it cannot be
used as a standalone `val` binding pattern.

### 6.4 List Patterns
```rust
[]              // Empty list
[x]             // Single element
[x, y]          // Exact elements
[head | tail]   // Head and tail (cons pattern)
```

### 6.5 Record Patterns
```rust
Person { name, age }                    // Extract all fields
Person { name: "Alice", age }          // Partial match with literal
Point { x: 0, y: 0 }                   // Exact match
```

### 6.6 Spread Destructuring Patterns

Spread destructuring allows extraction of specific fields while capturing remaining fields in a rest binding:

```rust
// Basic spread pattern
Person { name, email, ...rest }         // Extract name and email, rest gets remaining fields

// Spread with explicit field patterns
User { id: userId, name, ...userMeta }  // Extract id as userId, name as name, rest as userMeta

// Spread in match expressions
value match {
    User { role: "admin", ..._ } => { "Administrator access" }
    User { department: "IT", name, ..._ } => { "IT user: " + name }
    User { name, ...profile } => { (name, profile) process_user }
}

// Future/planned: nested spread patterns in backend codegen
// Shown as a planned match-arm shape, not a current v0.0.1 guarantee.
Company {
    name: companyName,
    contact: Contact { email, ...contactInfo },
    ...companyDetails
} => {
    // Extract company name, contact email, and group remaining fields
    (companyName, email, contactInfo, companyDetails) process_company
}

// Wildcard spread (ignore remaining fields)
Point { x, y, ..._ } => { (x, y) calculate_distance }
```

**Spread Pattern Rules:**
- Spread pattern `...rest` must be the last element in a record pattern
- Rest binding captures all unmatched fields as a new record
- Use `..._` to ignore remaining fields
- Rest binding maintains the original record type but only with remaining fields

## 7. Statements

### 7.1 Variable Declarations
```rust
val x = 42              // Immutable
mut val counter = 0     // Mutable
```

### 7.2 Assignments
```rust
counter = counter + 1   // Only for mutable variables
```

### 7.3 Expression Statements
```rust
42                      // Expression as statement
"hello" |> println      // Function call
x + y                   // Must be pure (no side effects)
```

Expression statements that could be parsed as one whitespace-adjacent OSV
expression must be separated with `;`. A newline alone is not a statement
terminator.

## 8. Record Types

### 8.1 Basic Records
```rust
record Person {
    name: String
    age: Int32
}

record Point<T> {
    x: T
    y: T
}
```

### 8.2 Temporal Records
```rust
record File<~t> {
    path: String
    handle: FileHandle<~t>
}

record Connection<~db> where ~tx within ~db {
    url: String
    session: Session<~tx>
}
```

### 8.3 Implementation Blocks

Implementation blocks attach type-directed functions to a record name without
introducing object-style call syntax.

```rust
record Score {
    value: Int32
}

impl Score {
    fun bump: (self: Score, amount: Int32) -> Int32 = {
        self.value + amount
    }
}

fun main: () -> Int32 = {
    val score = Score { value: 40 }
    (score, 2) bump
}
```

Rules:

- The `impl` target must be a known record declaration name.
- An impl function is still a function. Calls remain grouped OSV:
  `(receiver) method` or `(receiver, args...) method`.
- Traditional object calls such as `score.bump(2)` are invalid.
- The first parameter of an impl method must be `self: Target`, where `Target`
  is the impl block's record name.
- Impl methods may be generic, and unannotated method returns may be inferred
  when the body supplies a concrete type.
- Impl blocks do not introduce class inheritance or open-ended OOP dispatch.
  They are a scoped, type-directed function namespace that preserves Restrict's
  value-flow-first OSV model.

### 8.4 Forms, Adoptions, and Form Bounds

A `form` is an explicit, compile-time behavioral contract. The initial source
surface is intentionally method-only: forms are non-generic, every method has
a fully typed signature, and form declarations do not contain method bodies.

```rust
pub form Labelled {
    fun label: (self: Self) -> String
}

record Badge {
    text: String
}

Badge takes Labelled {
    fun label: (self: Badge) -> String = {
        self.text
    }
}

fun read_label: <T of Labelled>(value: T) -> String = {
    value |> label
}
```

Rules:

- A form method must declare `self: Self` as its first parameter and must have
  an explicit return type. `Self` is available only in the form contract.
- A `takes` declaration targets one concrete, non-generic record. It provides
  exactly the methods required by the form using fully typed `fun`
  declarations with bodies. Missing, extra, duplicate, generic, temporal, or
  signature-incompatible methods are errors.
- `takes` declarations cannot be marked `pub`. Export the form and nominal
  record instead. A `pub form` is source-module metadata and can be imported by
  other Restrict modules; it does not create a host-visible WebAssembly export.
- A type parameter may require one or more forms with `<T of A + B>`. The
  concrete argument must have one compiler-provided or explicit adoption for
  every bound. An ordinary `impl` method with a matching name does not satisfy
  a form.
- Calls remain OSV: `receiver |> method` for one argument and
  `(receiver, args...) method` for multiple arguments. Traditional object calls
  such as `receiver.method()` remain invalid.
- Form resolution is closed-world and static. A canonical type may adopt a
  canonical form at most once. In the initial slice, ordinary `impl` methods
  and all form adoptions for one concrete type share one selector namespace:
  the type cannot expose the same method name through two declarations. A
  generic parameter with multiple bounds that expose the same selector is
  likewise ambiguous. Compilation fails rather than choosing by priority.
- The compiler monomorphizes form-bounded generic calls and emits a direct call
  to the selected adoption. Forms do not use vtables, runtime dictionaries,
  type-tag dispatch, or dynamic dispatch.
- Form calls use ordinary affine function semantics. Passing a non-Copy value
  as `self` consumes it; compiler-provided Copy scalars remain reusable. Form
  lookup itself is type inference and must not consume or replay an expression.

The initial form slice does not include associated types, generic forms,
generic or conditional `takes`, form default bodies, negative bounds,
first-class existential form values, selective adoption imports, or enum
adoptions. Form method compatibility is positional: parameter names may differ,
but arity and parameter and return types must match after replacing `Self` with
the target record. The compiler-internal `Container.Item` /
`Container.Mapped<U>` machinery used by current collection builtins does not
make `Container` or associated-type syntax source-visible.

The compiler prelude provides this method-only form:

```rust
pub form Display {
    fun display: (self: Self) -> String
}
```

`String`, `Int32`, `Int64`, `Float64`, `Boolean`, `Char`, and `()` have
compiler-provided `Display` adoptions. A user record adopts the form explicitly
with a `RecordName takes Display` declaration. Its `display`
method consumes a non-Copy `self` in the same way
as any other affine call.

`display`, `print`, and `println` are compiler-reserved call targets in this
initial slice. They cannot be redeclared as source functions or ordinary method
selectors; `display` inside `RecordName takes Display` is the sole method
exception. These three polymorphic builtins must be called directly and cannot
yet be captured as first-class function values.

## 9. Context Declarations

### 9.1 Basic Context
```rust
context Database {
    connection: Connection
    timeout: Int32
}
```

### 9.2 Context-Bound Functions (Future / Planned)

Context declarations and `with Context { ... } { ... }` expressions are current
syntax. Function-level context annotations are planned and are shown here only
as future syntax.

```rust
@Database
fun query: (sql: String) -> Result<Data, Error> = {
    // Can access connection and timeout implicitly
    (connection, sql) execute
}
```

## 10. Temporal Resource Management (Experimental / Outside v0.0.1 Default Gate)

### 10.1 Temporal Scopes
```rust
temporal ~t {
    val resource = Resource<~t> { ... }
    // resource automatically cleaned up when ~t ends
}
```

### 10.2 With Expressions
```rust
with Database { connection: conn } {
    "SELECT * FROM users" |> query
}

with lifetime<~f> {
    val file = File<~f> { path: "/tmp/data" }
    file |> read
}
```

### 10.3 Temporal Constraints
```rust
where ~inner within ~outer     // inner lifetime contained in outer
```

## 11. Prototype Operations

### 11.1 Clone
```rust
val newObj = obj.clone { field: newValue }
```

### 11.2 Freeze
```rust
val frozen = obj freeze         // Make immutable
val cloneAndFreeze = obj.clone { field: value } freeze
```

## 12. Import/Export

### 12.1 Imports
```rust
import release.policy.{score}
import release.policy.*
import release.policy
```

Imports are source-level declarations. The current v0.0.1 implementation
supports dotted module paths with named imports, wildcard imports, or whole
module imports. String paths, source-level import aliases, re-exports, and
package-level standard-library aggregators are reserved for a later
module-design pass. A build tool may bind the first dotted path segment to a
package source root; that root-configuration/compiler binding is not an `import ... as`
source alias.

Within one compilation, a source declaration has one canonical identity: its
dotted module path plus its declaration name. Splitting named imports across
multiple statements, or reaching the same declaration through both direct and
transitive imports, must not create distinct nominal types or duplicate module
bodies. Duplicate exports, duplicate virtual sources that normalize to the
same module path, ambiguous configured search roots, and cyclic imports are
resolver errors. A failed resolution is not cached as a complete module and
may be retried after the missing source becomes available. When compiling a
source file, its parent directory is searched before the process working
directory fallback.

The native compiler accepts repeatable `--module-root ALIAS=DIR` bindings for
package builds. `ALIAS` must be one complete, non-keyword Restrict identifier;
`std` is reserved. A configured namespace has these mappings:

- `import ALIAS.{item}` loads `DIR/lib.rl`;
- `import ALIAS.foo.bar.{item}` loads `DIR/foo/bar.rl`;
- an unqualified import inside a mounted package is resolved inside the same
  `ALIAS` namespace;
- `ALIAS.lib` is rejected because it would give `lib.rl` two canonical module
  identities;
- a missing file under a configured namespace does not fall back to an
  application source directory.

Explicit application search roots and package source roots are canonicalized
and must be pairwise disjoint: equal, ancestor, and descendant roots are
rejected. This keeps one physical source file from acquiring both an
application identity and a package-qualified identity.

Warder uses this binding for direct local path dependencies selected centrally
from the repository-root `warder.toml`. A local source package needs only
`src/lib.rl`; its `[dependencies]` key is the source namespace. It does not
have a leaf manifest or leaf lock. The current implementation has one
publishable root package plus these direct local source packages. Registry,
Git, foreign-Wasm, transitive dependency resolution, and a multi-publish
workspace graph remain outside this buildable slice and must fail explicitly
rather than producing unresolved `warder.lock` entries.

### 12.2 Exports
```rust
pub fun publicFunction: () = { ... }
pub record PublicType { ... }
pub enum PublicError { Missing Invalid(String) }
pub form PublicContract { fun render: (self: Self) -> String }
pub val release_bias: Int32 = 3
```

For the v0.0.1 implementation, exported records are source-level module
metadata. Exported enums have the same source-module-only meaning.
Records, enums, and forms can be imported and used by other Restrict source modules,
but they do not emit direct host-visible WebAssembly exports. Importing an enum
imports its type namespace; callers continue to spell constructors and patterns
as `EnumName::Variant`. Importing a form makes its canonical contract available
to `of` bounds and concrete record adoptions. Exported generic functions also remain outside the
current concrete WebAssembly ABI surface.
Host-visible exported top-level bindings are limited to scalar literal
constants with `Int32`, `Int64`, `Float64`, `Boolean`, `Char`, or `()` ABI.
Composite constants such as `String`, records, lists, `Option`, and `Result`
or user-defined enums remain source-level values unless a concrete host ABI is
designed. Host-visible function parameters and results involving user-defined
enums are rejected by both the default ABI and `flat-record-v1`.

### 12.3 Experimental Flat Record Host ABI (Opt-in)

The default v0.0.1 host ABI remains the scalar-only surface described above. A
compiler invocation may explicitly select the experimental core-WebAssembly
profile `--host-abi flat-record-v1`. This opt-in does not change what an
invocation without the flag accepts, and it is not part of the published
v0.0.1 compatibility boundary.

`flat-record-v1` extends function exports with a deliberately narrow record
adapter. A record used in an exported function parameter or result is eligible
only when all of the following hold:

- the function and record are concrete and non-generic;
- the record declaration itself is source-exported with `pub record`;
- neither the function nor the record has temporal parameters or constraints;
- the record declares between 1 and 16 fields, inclusive; and
- every field is directly `Int32`, `Int64`, `Float64`, `Boolean`, or `Char`.

`()` fields, nested records, and every other composite field type are not part
of this profile. A generic declaration does not become eligible merely because
one call site could be monomorphized.

The generated host wrapper flattens parameters in function parameter order.
Each record parameter contributes its fields in source declaration order, and
each ordinary scalar parameter contributes one value. A `()` parameter keeps
the existing one-dummy-`i32` convention; a `()` result contributes no value.
The complete flattened
parameter vector must contain at most 16 core-WebAssembly value slots. `i64`
and `f64` each count as one slot, not two 32-bit words.

A record result is returned as core-WebAssembly multi-value results in source
field order. The complete flattened result vector must also contain at most 16
slots. Scalar and unit results retain their existing one-slot and zero-slot
representations.

Only the generated wrapper is exported under the source export name. The
Restrict implementation body and its internal calling convention remain
private. In particular, the wrapper must not expose an arena pointer,
`LayoutId`, record byte offset, or any other internal representation detail.
The external field order therefore remains source-defined even if the internal
record layout changes.

Unsupported signatures are rejected rather than silently falling back to an
internal pointer ABI. This initial profile does not cover `String`, `List`,
`Array`, `Option`, `Result`, range, nested-record, function, generic, temporal,
or global composite values. It does not generate WIT or a WebAssembly
Component Model interface. Source-level `pub record` declarations continue to
be module metadata and do not by themselves create a host-visible Wasm export.

A core-WebAssembly trap that escapes a `flat-record-v1` wrapper bypasses its
normal arena restoration. The host must treat that module instance as invalid
for subsequent `flat-record-v1` calls and instantiate a fresh module. A nested
same-export call that traps may be caught by the host only when the outer call
then returns normally, allowing the outer wrapper to restore its entry state.

### 12.4 B1 v0.1.0 Synchronous Component ABI

This section specifies the explicit B1 Component path implemented on this
branch. It does not change the ordinary v0.0.1 Core Wasm output: the default
ABI remains the scalar profile and `flat-record-v1` remains experimental.
B1 implementation completion is not a claim that a v0.1 package has already
been released.

The public Component interface is not the compiler's arena representation.
Interface authority follows API ownership, not import/export direction:

- for a Restrict-owned API, an explicitly selected component export is type
  checked and monomorphized into `ComponentInterfaceIr` (Component Interface
  IR), from which the compiler generates normalized WIT and Canonical ABI
  adapters. Ordinary source-level `pub` continues to mean module visibility
  and is not an implicit component export;
- for WASI, standard worlds, third-party APIs, and other externally owned
  contracts, external WIT is authoritative whether Restrict consumes/imports
  the API or implements/exports it. The toolchain generates typed Restrict
  bindings or implementation obligations and verifies the resulting component
  against that WIT; and
- normalized WIT plus stable ABI-profile metadata is the final compatibility
  authority for both ownership paths.

The implemented Restrict-owned selection surface is versioned `[component]`
metadata in the repository-root `warder.toml`, not a new language keyword. A
declaration enters the Component ABI only when both conditions hold:

1. it is source-public under the existing `pub` / `export` declaration
   surface; and
2. a schema-versioned `[component]` entry selects it explicitly.

Neither `pub` nor `export` alone publishes a Component ABI, and this explicit
selection rule does not change their current Core Wasm or source-module
behavior. Schema v1 uses this manifest shape:

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

`package` is the complete WIT package identity in
`namespace:name@semver` form. In schema v1 its name and version must equal
`[package].name` and `[package].version`; `abi` must be
`canonical-v1-sync`. `world`, `interface`, and `name` are validated WIT
identifiers. For schema v1, `source` names one source-public
function in the package root module. An absent selection exports nothing to a
Component; an unknown schema, ABI profile, or field, a private or missing
declaration, duplicate selection, or external-name collision is an error.
Cross-module selectors are not accepted until module resolution preserves a
stable canonical declaration identity independent of its internal mangled
name.

Warder parses and validates the root configuration, resolves the selection
through the compiler library, and transactionally publishes normalized WIT,
validated Component bytes, and metadata. A future language edition may
distinguish `pub` package visibility from `export` Component publication and
derive ordinary concrete exports from typed source. That source-first surface
is not implemented: today the explicit source-public plus `[component]`
double gate remains required. Any future source sugar must lower to the same
checked selection model and cannot become a second interface authority. The
detailed invariants are specified in the
[B1 Component Implementation Design](docs/B1_COMPONENT_IMPLEMENTATION_DESIGN.md).

Restrict-owned APIs do not require a second handwritten WIT declaration. That
would duplicate Restrict types, invite drift, and discard affine ownership and
capture facts plus source-span diagnostics before interface checking. External
standards and third-party contracts remain WIT-first because Restrict does not
own those interfaces.

WIT file emission alone does not describe the implemented B1 stack. The
synchronous
interoperability stack includes normalized WIT, Component Model packaging,
Canonical ABI lift/lower and cleanup, Rust language bindings, component composition,
compatibility diffing, and execution in pinned Component runtimes. The
repository proves both Rust interoperability directions through
the versioned WIT world; WIT-only output is not reported as Component support.

The v0.1.0 synchronous type mapping is:

- `Int32`, `Int64`, `Float64`, `Boolean`, `Char`, and `()` map to the
  corresponding WIT scalar or empty result;
- `String` maps to WIT `string` with validated UTF-8 conversion;
- `List<T>` maps recursively to `list<T>`;
- `Array<T, N>` maps to `list<T>` plus stable exact-length metadata and a
  generated length check at every boundary;
- records map recursively to WIT `record` in source declaration order;
- `Option<T>` and `Result<T, E>` map recursively to WIT `option` and `result`;
- payload-free user enums map to WIT `enum`, while payload-carrying user enums
  map to WIT `variant` by source case identity rather than internal tag layout;
- explicitly declared synchronous host resources map to WIT `resource`, where
  `own` transfers affine ownership, `borrow` is call-scoped, and destruction
  occurs exactly once;
- each exported closure/function-value signature maps to a generated once-
  callable resource, which owns value-only captured state and exposes a
  synchronous call operation; raw table indexes and arena pointers never cross
  the boundary. Captures containing another resource or callable fail closed;
  and
- a generic function can cross the boundary only through a finite, explicit
  set of concrete monomorphized instances with deterministic external names.
  WIT never contains an unresolved Restrict type parameter. Generic nominal
  boundary types are not part of this B1 implementation.

Manifest schema v2 implements the declaration contract for synchronous
resources, once-callable resources, and concrete generic function instances.
Warder parses and normalizes it into a manifest-neutral compiler model that is
connected to Component Interface IR, code generation, adapters, validation,
runtime execution, and transactional artifact publication. Schema v1 remains
valid and continues to reject every v2-only field.

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

Every entry is explicit. A resource `source` must resolve to one named public
root record. Its destructor identity is explicit even when the implementation
is private; `destructor-visibility` records whether source resolution must find
a `private` or `public` declaration. Each method records an external name,
source identity, and receiver fact: `own` consumes the affine handle, while
`borrow` is valid only for the dynamic extent of one synchronous call. These
facts are stable contract input; they do not infer a resource from record shape
or method spelling. The destructor source cannot also be listed as an ordinary
resource method.

A callable declares a concrete parameter/result signature and capture-call
multiplicity. The field is mandatory and accepts the vocabulary `once` and
`many`, but the initial synchronous v0.1 contract accepts only `once`; `many`
is rejected with a typed unsupported-multiplicity diagnostic until its affine
capture semantics and adapters are implemented.

`type-args` maps declared source type-parameter names to closed Restrict type
expressions. The accepted expression grammar covers `()`, scalar types,
root-nominal types with concrete arguments, `List`, `Array<T, N>`, `Option`,
`Result`, and concrete function types. Temporal types, `Self`, wildcards,
malformed arities, symbolic array lengths, and trailing input are rejected
during manifest normalization. Nominal names are still resolved and bounds are
still checked by the compiler; parsing a name does not prove that the type
exists. Map and manifest order are non-semantic, and the specialization pass
restores declaration order before computing its stable identity.

Schema v2 remains root-only until durable `CanonicalDeclId` resolution lands.
Types not listed above, including `Range<Int32>`, do not acquire an ABI by
implication. A source byte type and direct `list<u8>` mapping are separate
follow-on design work; `Char` is not a byte substitute.

WIT alone cannot represent Restrict open generics, first-class closures,
affine capture multiplicity, or a stable `Array<T, N>` length constraint.
Explicit concrete instantiation, callable resources, ownership metadata and
drop adapters, and `list<T>` length validation are therefore normative v0.1.0
adapter rules rather than optional implementation conventions.

Lifted arguments cannot expose host-retained pointers into a Restrict arena.
Composite results must be copied or transferred to Canonical ABI-owned storage
before invocation-arena reset. Generated post-return and drop paths must cover
normal return, source `Result::Err`, host error, allocation failure, and trap.
After an uncaught component trap the instance is terminal. Synchronous callback
re-entry must preserve the outer arena and all resource/callable ownership.

The B1 conformance gate covers rich external-WIT consumer bindings, scalar
external implementation/export conformance, Canonical ABI lift/lower and
post-return or drop cleanup, component packaging/validation/composition/runtime
execution, resource-lifecycle negative tests, and breaking/non-breaking
compatibility diffs over normalized WIT plus stable ABI metadata. Rich-value
external implementation/export and external-WIT resources currently fail
closed and remain follow-on work. Repository release gates use two
independently compiled end-to-end paths:

- Rust-produced Component -> Restrict consumer; and
- Restrict-produced Component -> Rust consumer.

These paths may share only the versioned WIT package/world; they must not share
an allocator, linear memory, arena layout, runtime representation, or a bridge
that bypasses the Canonical ABI. MoonBit remains an external design precedent
and optional comparison target, not a required repository E2E toolchain.

v0.1.0 retains the current Core Wasm and WASI Preview 1 outputs. It consumes
synchronous WIT capability imports and runs its conformance world in a pinned
Component host, adapting the current basic output/exit behavior where needed.
Full WASI 0.2 command-interface coverage is a separate host-profile milestone,
not part of this ABI gate. Async functions, `future`, `stream`, cancellation,
backpressure, and ownership across suspension are excluded from v0.1.0 and
form the v0.2.0 boundary. Complete network/HTTP profiles are follow-on host
integration rather than implied core-language semantics.

## 13. Operator Precedence (Highest to Lowest)

1. Field access, qualified variant names, grouped direct OSV calls, and scoped
   verb clauses: `.field`, `.clone`, `Type::Variant`, `freeze`, `(value) f`,
   `() f`, `values map { ... }`
2. Unary: `!`, `-`
3. Multiplicative: `*`, `/`, `%`
4. Additive: `+`, `-`
5. Relational: `<`, `<=`, `>`, `>=`
6. Equality: `==`, `!=`
7. Logical AND: `&&`
8. Logical OR: `||`
9. Pipe: `|>` (left associative)
10. Completed-value application continuation (left associative): direct OSV,
    a following pipe, a named scoped verb, or an anonymous scope after a
    complete `match`, `then`, `while`, block, or preceding clause

Single-argument calls may use pipe form (`value |> f`) or direct OSV form
(`value f`). Parentheses are optional for a single simple direct object, as in
`42 Option::Some`. Unit and tuple objects stay grouped (`() now`, `(left,
right) max`), and compound objects should be grouped when precedence would
otherwise change their meaning (`(1 + 2) double`). Pipe starts from a complete
expression, so `1 + 2 |> double` is parsed as `(1 + 2) |> double`.
Likewise, a scoped verb clause is complete before a following pipe:
`values map { it + 1 } |> list_count` pipes the mapped collection.
An anonymous scope also receives the complete expression to its left, so
`1 + 2 { |value| value * value }` applies the scope to `1 + 2`, not only to
`2`.

## 14. Standard Library Types

### 14.1 Collections
- `List<T>` - Dynamic list
- `Array<T, N>` - Fixed-size array
- `Range<Int32>` - Range type from `[start..end]` with Int32 endpoints

### 14.2 Error Handling
- `Option<T>` - May contain `value Option::Some` or `() Option::None`
- `Result<T, E>` - Success via `value Result::Ok` or error via `error Result::Err`

A user-defined enum may be used as the error parameter of `Result<T, E>`:

```rust
enum DecodeError {
    Empty
    Invalid(String)
}

fun fail: (message: String) -> Result<Int32, DecodeError> = {
    (message |> DecodeError::Invalid) Result::Err
}
```

Error propagation is explicit through `match` in the current compiler. A
postfix `?` operator remains future work until the language specifies a real
early-exit operation together with affine branch merging and deterministic
cleanup of arena and temporal resources.

### 14.3 Basic Functions
```rust
println: <T of Display>(T) -> ()
print: <T of Display>(T) -> ()
display: <T of Display>(T) -> String
print_int: (Int32) -> ()
print_float: (Float64) -> ()
eprint: (String) -> ()
eprintln: (String) -> ()
```

`print` and `println` statically select the argument type's `Display` adoption.
`print_int` and `print_float` remain available as compatibility helpers;
`eprint` and `eprintln` remain String-only. No runtime type tag or dynamic
dispatch is used.

## 15. DEPRECATED AND REMOVED SYNTAX

The following syntax is **NO LONGER SUPPORTED** and will cause compilation errors:

### 15.1 Removed Keywords
- `let` (use `val` instead)
- `fn` (use `fun` instead)
- `if` (use `then/else` instead)
- `Unit` as type name (use `()`)

### 15.2 Removed Operators
- `|>>` mutable pipe operator (removed)

### 15.3 Removed Syntax Patterns
- `val mut x = 5` (use `mut val x = 5`)
- `fun name = ...` (use `fun name: (...) = ...`)
- `fun name: param: Type ...` (function parameter lists require parentheses)
- `[|1, 2, 3|]` array literals (use `[1, 2, 3]`)
- `if condition { ... }` (use `condition then { ... }`)
- `while condition { ... }` (use `condition while { ... }`)
- `Some(value)` / `None` in value expressions (use qualified `Option::Some` /
  `Option::None` OSV calls; unqualified forms remain match patterns)
- `Ok(value)` / `Err(error)` in value expressions (use qualified
  `Result::Ok` / `Result::Err` OSV calls)

## 16. EXAMPLES

### 16.1 Hello World
```rust
fun main: () = {
    "Hello, Restrict Language!" |> println
}
```

### 16.2 Basic Arithmetic
```rust
fun add: (x: Int32, y: Int32) -> Int32 = {
    x + y
}

fun main: () = {
    val result = (10, 20) add
    result |> println
}
```

### 16.3 Forms and Display
```rust
record Notice {
    text: String
}

Notice takes Display {
    fun display: (self: Notice) -> String = {
        self.text
    }
}

fun main: () -> () = {
    42 |> print
    " · " |> print
    Notice { text: "records too" } |> println
}
```

### 16.4 Pattern Matching
```rust
fun describe: (x: Option<Int32>) -> String = {
    x match {
        Some(n) => { "Got number" }
        None => { "No number" }
    }
}
```

### 16.5 Comprehensive Pattern Matching Examples
```rust
// Advanced Option pattern matching
fun process_maybe: (data: Option<User>) -> String = {
    data match {
        Some(User { name, role: "admin", ..._ }) => { "Admin: " + name }
        Some(User { name, department: "IT", ..._ }) => { "IT user: " + name }
        Some(User { name, ..._ }) => { "Regular user: " + name }
        None => { "No user data" }
    }
}

// List pattern matching with spread
fun analyze_list: (numbers: List<Int32>) -> Int32 = {
    numbers match {
        [] => { 0 }
        [single] => { single }
        [first, second] => { first + second }
        [head | tail] => { head + (tail |> list_length) }
    }
}

// Complex nested pattern matching
record Address { street: String, city: String, zipcode: String }
record Person { name: String, age: Int32, address: Address, tags: List<String> }

fun categorize_person: (person: Person) -> String = {
    person match {
        // Pattern with nested record destructuring
        Person {
            age,
            address: Address { city: "Tokyo", ..._ },
            tags,
            ..._
        } => { "Tokyo resident" }

        // Pattern with list matching
        Person { name, tags: ["VIP" | _], ..._ } => { "VIP member: " + name }
        Person { name, tags: [], ..._ } => { "Untagged user: " + name }

        // Catch-all with spread
        Person { name, age, ...profile } => {
            "Regular user: " + name
        }
    }
}
```

### 16.5 User-Defined Errors

```rust
enum CheckoutError {
    InvalidSku
    PaymentDeclined(String)
}

fun classify: (error: CheckoutError) -> Int32 = {
    error match {
        CheckoutError::InvalidSku => { 1 }
        CheckoutError::PaymentDeclined(message) => { 2 }
    }
}

fun reject: (message: String) -> Result<Int32, CheckoutError> = {
    (message |> CheckoutError::PaymentDeclined) Result::Err
}
```

### 16.6 Records and Methods
```rust
record Point {
    x: Int32
    y: Int32
}

impl Point {
    fun distance: (self: Point, other: Point) -> Float64 = {
        val dx = self.x - other.x
        val dy = self.y - other.y
        ((dx * dx + dy * dy) as Float64) |> sqrt
    }
}

fun main: () -> Float64 = {
    val start = Point { x: 0, y: 0 }
    val end = Point { x: 3, y: 4 }
    (start, end) distance
}
```

### 16.7 Scoped Collection Flow

```rust
fun main: () -> Int32 = {
    val values = [1, 2, 3]
    val shifted = values map {
        it + 1
    }
    (shifted, 0) fold { |total, value|
        total + value
    }
}
```

### 16.8 Temporal Resource Management
```rust
fun processFile: (path: String) -> Result<String, Error> = {
    temporal ~file {
        val file = File<~file> { path: path }
        file |> read
        // file automatically closed when ~file scope ends
    }
}
```

## 17. MIGRATION GUIDE

If you have existing code using deprecated syntax:

### 17.1 Variable Declarations
```rust
// OLD (incorrect)
val mut x = 5
let x = 5

// NEW (correct)
mut val x = 5
val x = 5
```

### 17.2 Function Declarations
```rust
// OLD (some docs show this)
fun add = x:Int y:Int { x + y }

// NEW (correct)
fun add: (x: Int32, y: Int32) -> Int32 = { x + y }
```

### 17.3 Array Literals
```rust
// OLD (deprecated)
[|1, 2, 3|]

// NEW (correct)
[1, 2, 3]
```

### 17.4 Conditionals
```rust
// OLD (not supported)
if condition { ... } else { ... }

// NEW (correct)
condition then { ... } else { ... }
```

### 17.5 Built-in Sum Constructors

```rust
// OLD (not supported in value expressions)
Some(42)
None
Ok(42)
Err("missing")

// NEW (qualified OSV; pipe and direct forms are both valid)
42 Option::Some
42 |> Option::Some
() Option::None
42 Result::Ok
"missing" Result::Err
```

---

## COMPLIANCE

This specification defines Restrict Language specification v1.0. Its document
version is independent of compiler product releases such as v0.0.1, v0.1.0,
and v0.2.0. All implementations, documentation, tutorials, and examples MUST
conform to the implemented sections of this specification and must not present
planned sections as current behavior.

**Parser Implementation**: The official parser in `src/parser.rs` is the
authority for the current implementation. Sections marked future, planned, or
experimental are intentionally outside the default v0.0.1 gate unless tests
explicitly include them.

**Documentation**: All other documentation files are superseded by this specification.

**Last Updated**: 2026-08-09
**Version**: 1.0.0
**Status**: CANONICAL SOURCE OF TRUTH
