# Naming and Syntax Decisions

**Status**: Accepted direction
**Decision date**: 2026-11-18
**Scope**: Official identifier naming conventions (`UpperCamelCase` / `snake_case`) and line-bounded direct OSV / semicolon-free statement boundaries

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
