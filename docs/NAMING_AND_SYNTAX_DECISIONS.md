# Naming and Syntax Decisions

**Status**: Accepted direction
**Decision date**: 2026-11-18
**Scope**: Official identifier naming conventions (`UpperCamelCase` / `snake_case`), line-bounded direct OSV / semicolon-free statement boundaries, pipe stages for scopes and argument tuples (Decision 3, 2026-11-19), and the `Option` fallback operator `??` (Decision 4, 2026-11-19)

This document records the language-architecture rationale behind Restrict's
identifier naming conventions and line-bounded statement boundaries.
`LANGUAGE_SPECIFICATION.md` remains authoritative for implemented language
behavior.

## Decision 1: Official Naming Conventions (`UpperCamelCase` + `snake_case`)

Restrict standardizes on two complementary casing conventions:

- **`UpperCamelCase`** for nominal types and type-level constructs: `record`,
  `enum`, `form`, `context`, generic type parameters (such as `T`, `U`, `Item`),
  and enum variant constructors (`EnumName::Variant`, including `Option::Some`,
  `Option::None`, `Result::Ok`, and `Result::Err`).
- **`snake_case`** for value-level, executable, and module-level identifiers:
  functions (`fun`), `impl` / `form` / `takes` methods, `val` / `mut val`
  bindings, parameters, record/context fields, module names, source file names
  (`my_module.rl`), and temporal lifetimes (`~file`, `~tx`).

### Rationale: Why `snake_case` Instead of `lowerCamelCase`

1. **Lossless, reversible 1:1 mapping with WebAssembly Component Model (WIT) `kebab-case` identifiers**

   WIT identifiers use lowercase `kebab-case` (for example, `parse-http-url`).
   In Restrict's Component Interface IR emitter (`src/component.rs`,
   `normalize_component_identifier`), `snake_case` identifiers map 1:1 to WIT
   (`parse_http_url` <-> `parse-http-url`). By contrast, `lowerCamelCase`
   introduces acronym-splitting and round-trip collision hazards: splitting on
   every uppercase ASCII letter maps `parseHTTP` to `parse-h-t-t-p`, while
   `parseHttp` maps to `parse-http`, neither of which preserves a clean,
   reversible word boundary without brittle heuristics. Standardizing on
   `snake_case` keeps Component export generation, external WIT binding
   generation, and compatibility diffing deterministic and ambiguity-free.

2. **Visual separation of Nominal Types/Constructors vs. Values/Verbs in parenthesis-free OSV expressions**

   Restrict uses Object-Subject-Verb (OSV) syntax where arguments and verbs
   appear side by side without call parentheses:

   ```restrict
   42 Option::Some
   Notice { text: "ready" } println
   user_input parse_int
   ```

   Using `UpperCamelCase` exclusively for types and constructors and `snake_case`
   for values and verbs makes the boundary between data/types and operations
   immediately readable at a glance. It also aligns directly with the parser's
   disambiguation rule in `src/parser.rs`: `UpperCamelName { ... }` commits to a
   nominal record literal, whereas `lowercase_verb { ... }` retains backtracking
   so higher-order scoped verb clauses such as `values map { it + 1 }` never
   collide with record construction.

3. **Readability of namespace-prefixed free functions in a value-flow language**

   Restrict deliberately avoids `object.method()` dot-call syntax. Standard
   library operations are free functions composed in OSV order and often carry a
   module or type prefix (`string_parse_int`, `option_unwrap_or_else`,
   `result_and_then`, `list_enumerate_from`). Multi-segment prefixes remain easy
   to scan in `snake_case`, whereas `lowerCamelCase` (`optionUnwrapOrElse`)
   blurs prefix and verb boundaries in long pipelines.

4. **Ecosystem consistency with WASI, Rust host bindings, and Warder conventions**

   The convention matches WASI Preview 1/2 identifiers (`fd_write`, `proc_exit`,
   `args_get`), generated Rust host bindings, compiler builtins
   (`string_length`, `char_from_code`, `array_length`), and Warder's test
   discovery convention (`test_*` / `*_test`).

### Historical Context

Early documentation, design sketches, and examples mixed Kotlin/Scala-inspired
`lowerCamelCase` (`newObj`, `processFile`, `userId`) with `snake_case`. All new
and maintained Restrict code, standard library modules (`std.*`), examples,
specifications, and guides standardize strictly on `UpperCamelCase` for types
and constructors and `snake_case` for values, functions, fields, modules, and
lifetimes.

---

## Decision 2: Line-Bounded Direct OSV & Principle of Semicolon-Free Code

### Problem Statement

Previously, Restrict treated newlines as ordinary whitespace everywhere,
including bare direct OSV juxtaposition (`value verb`). Because any identifier or
parenthesized expression on the next line could act as a verb, writing:

```restrict
val answer = 41
increment
```

parsed as `val answer = (41 increment)` rather than a `val` binding followed by a
standalone expression. Similarly, a `val` binding, `mut val` reassignment, or
`then` / `match` / `while` block followed by an identifier-started expression
attempted to extend the preceding expression as an OSV call chain unless the
author inserted a defensive `;`:

```restrict
val message = "ready";
message println
```

Requiring defensive semicolons after almost every `val` binding and block
contradicted Restrict's design goal of clean, low-noise, semicolon-free code.

### Considered Alternatives

- **Option A (Chosen): Line-bounded direct OSV and anonymous scopes with explicit multi-line continuations**
  - Direct bare OSV juxtaposition (`"hello" println`, `(10, 20) add`,
    `42 Option::Some`), trailing scoped verb clauses (`values map { ... } filter
    { ... }`), and trailing anonymous scopes (`value { |v| ... }`) remain
    concise and whitespace-driven on the same line (with spaces, tabs, or
    single-line inline comments).
  - Multi-line composition uses explicit continuation syntax that already
    signals continuation unambiguously: the pipe operator (`|>`), infix binary
    operators (`+`, `-`, `*`, `&&`, `||`, etc.), postfix `.field` / `.clone`, or
    control-flow keywords (`then`, `else`, `match`, `while`).
  - Bare direct OSV verbs and anonymous `{ ... }` scopes do not cross line
    breaks. A bare `return` followed by a newline returns `()`.
  - Semicolons `;` remain valid and optional — useful when placing multiple
    statements on one line or when explicitly discarding a block's final
    expression as `()` — while idiomatic multi-line Restrict code is written
    semicolon-free.
- **Option B (Rejected): Statement-start lookahead only**
  - Keeping newlines as pure whitespace in OSV calls while stopping only when
    the next line begins with `val`, `mut`, `return`, or `ident =`.
  - Rejected because it still breaks whenever the next line is a plain
    expression statement or block result starting with an identifier (such as
    `val x = 1\n x println`), forcing semicolons on the very bindings most
    common in Restrict blocks.
- **Option C (Rejected): Formatter-only semicolon insertion**
  - Retaining whitespace-across-newlines OSV parsing and teaching the formatter
    to insert defensive `;` automatically.
  - Rejected because it normalizes noisy semicolons across every multi-line
    block and fails users writing code before formatting runs.

### Normative Rules and Examples

1. **Same-line direct OSV and scoped clauses**

   Direct OSV calls, tuple calls, and chained same-line scoped clauses stay
   unchanged on a single line (and multi-line bodies inside `{ ... }` keep
   working because `{` opens on the same line as the verb):

   ```restrict
   "hello" println
   (10, 20) add
   42 Option::Some

   values map {
       it + 1
   } filter {
       it > 2
   } |> list_count
   ```

2. **Line breaks terminate direct OSV and anonymous scopes**

   A newline after a binding, assignment, expression, or completed control/block
   expression prevents bare OSV verb or anonymous-scope continuation on the
   following line:

   ```restrict
   val message = "ready"
   message println

   positive match {
       true => { 1 }
       false => { 0 }
   }
   summary println
   ```

3. **Explicit multi-line continuation**

   Multi-line pipelines continue with `|>` (or infix operators, `.field` /
   `.clone`, and control keywords):

   ```restrict
   val result = user_data
       |> validate_input
       |> transform_data
       |> save_to_database
   ```

4. **Bare `return` across a newline**

   `return` followed by a newline returns `()` rather than consuming the next
   line's expression as its return value:

   ```restrict
   done then {
       return
   } else {
       ()
   }
   ```

5. **Optional `;` semantics**

   Semicolons remain supported for two explicit purposes:
   - separating multiple statements on a single line (`val a = 1; a println`);
   - discarding a block's final expression value so the block evaluates to `()`
     (`do_work;`).

   Idiomatic multi-line Restrict code, standard library modules, and examples
   omit trailing semicolons.

### Design Consequences

- Multi-line Restrict code is semicolon-free by default without introducing
  ambiguous lookahead rules.
- Pipe chains (`|>`) remain the canonical multi-line composition style, while
  same-line direct OSV (`42 Option::Some`, `"hello" println`) stays compact.
- The token- and comment-preserving formatter (`src/formatter.rs`) retains
  explicit `;` where authors intentionally write them on single-line sequences
  or unit-discard statements, without inserting defensive semicolons across
  lines.

---

## Decision 3: Pipe Stages Continue Scopes and Argument Tuples

**Decision date**: 2026-11-19

### Problem Statement

Decision 2 made direct OSV, scoped verb clauses, and anonymous scopes
line-bounded, with `|>` as the multi-line continuation. The parser, however,
accepted only a bare verb after `|>`. Three same-line forms therefore had no
multi-line spelling:

```restrict
(a, b) |> add              // syntax error
values |> map { it + 1 }   // the scope did not attach to `map`
value |> { it + 1 }        // parsed as a plain block; `it` was undefined
```

The practical cost showed up when an additional argument became relevant in
the middle of a flow. Because multi-argument calls group their arguments before
the verb, the grouping had to be carried back to the first line of the
expression:

```restrict
val range_bonus = (([0..entry_count], |i| i + 1) map, 0, |acc, x| acc + x) fold
```

### Considered Alternatives

- **Option A (Chosen): `|>` continues the existing scope forms**
  - `value |> verb { ... }`, `value |> { ... }`, and `(a, b) |> verb` elaborate
    to exactly the same calls as `value verb { ... }`, `value { ... }`, and
    `(a, b) verb`.
  - A pipeline opens an anonymous scope at the stage where a further argument
    appears, and ordinary tuple-call syntax is used inside that scope:
    `|> { (it, 0) |> fold { |acc, x| acc + x } }`.
  - No new operator, keyword, or binding rule is introduced. `it` still exists
    only inside braces.
- **Option B (Rejected): extra value arguments after the pipe (`|> arg verb`)**
  - Rejected because `|` also delimits lambda binders. An identifier directly
    after `|>` reads like a binder name, so the position after `|>` is reserved
    for a verb or an opening `{`.
- **Option C (Rejected): configuration arguments on the verb (`|> verb(arg)`)**
  - Rejected because it reintroduces the `verb(args)` shape that Restrict
    forbids everywhere else, even if only after a pipe.
- **Option D (Rejected): a keyword particle (`|> verb with arg`)**
  - Rejected because it adds a second meaning to `with`, which already
    introduces context blocks, and it is longer than opening a scope.
- **Option E (Deferred): focus tuple shorthand (`|> (it, 0) fold { ... }`)**
  - Would save one pair of braces for single-expression stages.
  - Deferred because `it` would appear outside any braces, which contradicts
    the rule that `it` exists only inside the implicit focus scope that
    introduces it, and because `value |> (expr)` already pipes into a
    parenthesized callable expression. Revisit after the chosen form has been
    used in real programs.

### Normative Rules

1. The token after `|>` is a verb or an opening `{`, never a value argument.
2. `value |> verb { ... }` is a scoped verb clause when `{` opens on the same
   line as the verb.
3. `value |> { ... }` is an anonymous exact-once scope with exactly one binder.
4. `(a, b, ...) |> verb` supplies the verb of an argument tuple. The tuple is
   not a value, so the call binds at grouped-call precedence and the tuple
   cannot open an anonymous scope.
5. Nested implicit focus scopes stay rejected. Inside an unheaded scope stage,
   an inner scoped clause names its binder, or the stage names its object.

### Design Consequences

- Every same-line application form has a multi-line spelling, so Decision 2 no
  longer forces long flows onto one line.
- The parser elaborates the piped forms to the existing call nodes; the type
  checker and code generator need no new cases.
- Index access (`container[key]`) and the remaining brevity proposals are
  separate decisions and are not part of this one. The fallback for `Option`
  (`??`) is recorded in Decision 4; its extension to `Result` stays open.

---

## Decision 4: Fallback Operator `??`

**Decision date**: 2026-11-19

### Problem Statement

Taking the payload of an `Option` or a default was only possible through
`option_unwrap_or` or a `match`. Inside a pipeline, the function needs its
default as a second argument, and a second argument must be grouped before the
verb. After Decision 3, a pipeline reaches it only through an anonymous scope:

```restrict
val auth_hits = (counts, "auth")
|> smap_get
|> { (it, 0) |> option_unwrap_or }
```

The default is the point of the line, but it sits inside a scope that exists
only to supply a second argument.

### Considered Alternatives

- **Option A (Chosen): a stage `??` at the same level as `|>`**
  - `value ?? fallback` takes the complete expression on its left, associates
    to the left together with `|>`, and continues across lines like `|>`:
    `raw |> string_parse_int ?? 8080 |> clamp_port` is
    `((raw |> string_parse_int) ?? 8080) |> clamp_port`.
  - The right side is a binary-operator-level expression or a diverging
    `return`, `break`, or `continue`. A pipe in the right side is written with
    parentheses (`a ?? (key |> load)`).
  - A directly following `??` nests to the right (`a ?? b ?? 0` is
    `a ?? (b ?? 0)`); a pipe ends the right side.
  - Applies to `Option<T>` only. The result has type `T` and the fallback is
    evaluated only for `None`.
  - The parser expands it to `value match { Some(v) => { v } None =>
    { fallback } }`, so existing typing, affine merging, and code generation
    apply unchanged. The expansion records its origin so that diagnostics speak
    of `??`.
- **Option B (Rejected): one precedence level below `|>`**
  - Rejected because the stage after `?? 8080` would apply only to the default:
    in `raw |> string_parse_int ?? 8080 |> clamp_port`, `clamp_port` would
    receive `8080` and the parsed value would bypass it. When the types agree,
    the program is accepted with a meaning the author did not intend.
- **Option C (Deferred): `Result` on the left**
  - `Result` has an error value that a fallback would silently discard. Whether
    and how to support it is decided together with a postfix `?`, which has the
    same question. Until then, `value |> result_ok ?? fallback` states the
    discard explicitly.

### Normative Rules

1. The left side of `??` is `Option<T>`; the expression has type `T`. Any other
   left side is a type error, and a `Result` gets a hint to use `result_ok`.
2. The fallback is evaluated only when the left side is `None`, and it must
   have type `T`.
3. `??` shares a level with `|>`, associates to the left with it, and may begin
   a line to continue the previous line's expression.
4. The right side is a `binary_expr`, `return [value]`, `break`, or `continue`.
   A directly following `??` nests to the right.
5. The left side is consumed. A binding used in the right side is treated like a
   binding used in one arm of a `match`.
6. `option_unwrap_or` and the other `std.option` functions are kept and are not
   deprecated.

### Design Consequences

- The default value stays in the line where it matters, and no scope is needed
  to supply a second argument.
- A diverging right side makes an early exit read as the `None` path:
  `val name = opt ?? return "missing" Result::Err`.
- Because `??` and `|>` share one level, a stage written after `??` applies to
  the combined value. Authors who want a stage to apply to the default alone
  write it in parentheses.
- The expansion binds the payload under a name containing `$`, which no source
  program can write. A name an author could write would hide an outer variable
  of the same name inside the generated `match` and could change its value.
- No new typing or code generation rule exists; any future change to `match`
  applies to `??` as well.
