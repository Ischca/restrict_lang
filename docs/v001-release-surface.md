# v0.0.1 Release Surface Matrix

This matrix defines the compact public surface for the v0.0.1 release. The
language specification remains the source of truth; this document records what
the release intentionally presents as stable, explicitly rejected, or reserved.

## Supported

| Surface | v0.0.1 status |
| --- | --- |
| OSV-only calls | Supported. Calls use `value |> function`, `(arg1, arg2) function`, or `() function`; traditional `function(args)` calls are outside the surface. |
| `val` / `mut val` bindings | Supported. Immutable bindings use `val`; mutable bindings use `mut val`. |
| Built-in generic values | Supported for `List<T>`, `Option<T>`, `Result<T, E>`, and concrete `Range<Int32>`. |
| Fixed-length arrays | Supported as `Array<T, N>`. Any internal wildcard length used by built-in array operations is compiler machinery, not a source-level `Array<T, 0>` release contract. |
| Built-in container behavior | `map`, `filter`, and `fold` use compiler-supported behavior for the current built-in containers. The internal `Container` associated-type machinery is not source-visible. |
| Closed user-defined enums | Supported for non-generic, non-recursive enums whose variants have zero or one payload. Constructors use qualified `Type::Variant` names in OSV order, patterns are qualified, and matches are exhaustive. `pub enum` crosses source-module boundaries only and creates no host-visible enum ABI. |
| Source-level forms / Method-only forms | `form Name` declares fully typed method signatures. Concrete record `takes` declarations each target one concrete, non-generic record. Generic `of` bounds use `<T of Form>` or `<T of First + Second>`. Dispatch is static and monomorphized to direct method calls. |
| Standard `Display` | `display`, `print`, and `println` accept `<T of Display>`. `String`, `Int32`, `Int64`, `Float64`, `Boolean`, `Char`, and `Unit` have compiler-provided adoptions; records adopt Display explicitly. stderr remains String-only and `print_int` / `print_float` remain compatibility helpers. |
| Source imports without aliases/re-exports | Supported for dotted module imports, named imports, wildcard imports, and whole-module imports. |
| Source-level record exports no host ABI | Supported as source module metadata. Exported records can be imported by Restrict source modules, but do not create direct host-visible WebAssembly exports. |
| Scalar monomorphic `pub fun` / `export fun` host ABI | Supported for concrete, non-generic public function exports whose parameters and result are scalar host ABI values: `Int32`, `Int64`, `Float64`, `Boolean`, `Char`, or `()`. Other public function exports stay covered by the rejected generic/composite host ABI row. |
| Scalar constant `pub val` / `export val` host globals | Supported for immutable top-level literal constants whose host ABI is scalar: `Int32`, `Int64`, `Float64`, `Boolean`, `Char`, or `()`. Composite constants such as `String`, records, lists, `Option`, and `Result` remain outside the host-visible global export ABI. |
| Program entry `main` emitted as `_start` | Supported for program execution when `main` takes no parameters. `main` is the source entry point for executable programs and keeps its source result type in generated Wasm. The host `_start` export is a no-result wrapper that initializes the default arena, calls `$main`, drops any returned value, and resets the arena. A parameterized function named `main` remains a normal function and does not emit `_start`; expose host-callable scalar results through a separate scalar monomorphic `pub fun` or `export fun` wrapper. |

## Rejected With Explicit Diagnostics

| Surface | v0.0.1 diagnostic contract |
| --- | --- |
| Traditional calls | Rejected with diagnostic "traditional calls like `add(1, 2)` are not valid Restrict; use OSV syntax such as `(1, 2) add` or `value |> add`" because OSV-only calls are the public call surface. |
| Import aliases and string imports | Rejected with `string import paths and import aliases are unsupported in v0.0.1; use dotted source imports such as import module.{item}`. |
| Re-exports | Rejected with `re-exports are unsupported in v0.0.1; import declarations must stay at the source module boundary`. |
| Exported generic/composite host ABI as design gap | Rejected by v0.0.1 release-surface validation before `--check` success or code generation when a public export would require a generic or composite host ABI that v0.0.1 has not designed. |
| Exported composite top-level global ABI as design gap | Rejected by v0.0.1 release-surface validation before `--check` success or code generation when an exported top-level binding would require a composite host ABI. |
| Computed or mutable exported globals | Rejected by v0.0.1 release-surface validation. Exported top-level bindings must be immutable scalar constants in v0.0.1, and exported top-level bindings must be scalar literal constants in v0.0.1 rather than computed expressions. |

## Experimental/Future

| Surface | Reason |
| --- | --- |
| TAT outside default gate | Temporal Affine Types are planned/experimental and remain outside the v0.0.1 default release gate. |
| Generic/recursive enums and `?` | Remain future work. Custom closed enums can be used as `Result<T, CustomError>` errors, but ergonomic `?` propagation is not implemented. |
| Advanced form features | Associated types, generic forms, default methods, generic or conditional adoptions, enum adoptions, and dynamic dispatch remain future work. |
| Generic export ABI | Host-visible WebAssembly ABI rules for exported generic and composite values are still design work, not a supported release contract. |
