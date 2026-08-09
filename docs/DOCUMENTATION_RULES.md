# Documentation Maintenance Rules

Documentation has two audiences and two locations:

- public language documentation: `docs/public/`
- internal design and implementation documentation: `docs/`

Do not mix these. Internal design notes are not automatically public release
behavior.

## Public Documentation Policy

Public docs are the files published through mdBook and GitHub Pages:

```text
docs/public/SUMMARY.md
docs/public/en/
docs/public/ja/
docs/public/theme/
```

English is the primary public documentation language. Update English first, then
update Japanese when the public-facing behavior changes.

When changing user-facing language behavior:

1. Update `docs/public/en/`.
2. Update `docs/public/SUMMARY.md` if navigation changes.
3. Update `docs/public/ja/` when the Japanese page exists.
4. Update `README.md` if the feature is important for first-time users.
5. Run docs hygiene tests.

Public examples must stay inside the v0.0.1 release surface and distinguish
implemented features from reserved or experimental design space:

- `val`, not `let`
- `mut val`, not `val mut`
- OSV calls, not function-first calls
- line breaks are whitespace; non-callable values naturally start a new
  expression, while `;` separates a callable-shaped value from the preceding
  maximal OSV expression
- `:` field initializers
- ordinary v0.0.1 Core host exports are scalar-only; the explicit B1 Component
  path may document its supported rich values separately
- closed, non-generic, non-recursive user enums may use qualified
  `Type::Variant` names and exhaustive matches
- method-only forms, concrete non-generic record adoptions, and explicit `of`
  bounds may be documented as current post-v0.0.1 behavior
- WebAssembly is the sole code-generation target; Web, cloud, WASI, and
  container integrations are host profiles or adapters, not additional
  language backends
- generated JavaScript glue may be described as a current host requirement,
  but not as a JavaScript backend or direct Wasm access to browser APIs
- generic or recursive user enums, TAT, associated types,
  generic/conditional/enum adoptions, and dynamic dispatch marked as future
  work; do not describe the implemented explicit Component ABI as a Core ABI

Public interoperability sections may describe the B1 synchronous Component
implementation while stating that `0.1.0-rc.1` is a release candidate, stable
v0.1.0 is not yet published, and the ordinary Core ABI remains scalar-only.
They must remain aligned with `docs/V010_SYNCHRONOUS_COMPONENT_ABI.md`, keep
v0.2 async separate, avoid links outside the public book, and not publish
unsupported syntax examples.

Every B1 summary must preserve these guardrails:

- the ordinary pipeline is `.rl -> WAT -> Core Wasm`; the explicit schema-v1/
  schema-v2 path produces checked `ComponentInterfaceIr`, normalized WIT,
  Canonical ABI adapters, and validated Component bytes without changing the
  default Core ABI;
- Warder `build --component` transactionally publishes `.component.wasm`,
  `.component.wit`, and `.component.toml`; `wrap --wit` only attaches
  caller-supplied text to a Core module cage and does not validate or
  componentize it;
- interface authority follows API ownership rather than import/export
  direction: Restrict-owned explicit component exports generate normalized WIT,
  while externally owned WIT remains authoritative for both consumer/importer
  and provider/implementer roles;
- WIT output alone is not completion: Canonical ABI lift/lower, bindings,
  packaging/validation, composition/runtime execution, resource lifecycle, and
  semantic compatibility diffing are part of the B1 gate;
- the implemented external-WIT slice is rich guest-import/consumer plus scalar
  implementation/export conformance; rich external implementation/export and
  external-WIT resources fail closed;
- Rust/Restrict interoperability is tested in both directions through WIT and
  the Canonical ABI without a shared allocator or internal layout; MoonBit is a
  design precedent or optional external comparison, not a required repository
  E2E toolchain; and
- JS/TS bindings, Worker loaders, full WASI 0.2 command/network/HTTP coverage,
  async/future/stream, `many` callables, rich resource signatures,
  resource/callable-bearing captures, generic nominal boundary types, and
  durable cross-module schema-v2 identity are not claimed by B1.

Every Warder project/configuration summary must also preserve these guardrails:

- `warder.toml` is the single authored Warder configuration at the repository
  root; package/member leaf manifests and a separate workspace configuration
  file are not part of the design;
- the current implementation has one publishable root package plus direct
  local source packages selected centrally from `[dependencies]`; each local
  source package needs only `src/lib.rl`;
- `warder.lock` is generated once at the repository root and is reserved for
  nondeterministic external resolution. Registry, Git, foreign-Wasm, and
  transitive resolution currently fail closed, so local-only builds have no
  resolved package entries. The generated lock is committed when present and
  is not hand-edited;
- future multi-publish workspace support extends the same root file and does
  not derive public package or ABI identity from directory placement;
- `dist/*.component.toml` and Cage-internal `manifest.toml` are generated
  artifact metadata, not additional project configuration; and
- the current Component surface remains source-public plus explicitly selected
  by `[component]` in `warder.toml`. A future edition may distinguish
  package-visible `pub` from Component-intent `export`, but public docs must not
  describe that source-first surface as implemented.

## Internal Documentation Policy

Internal docs stay outside `docs/public/`:

```text
docs/TYPE_INFERENCE_DESIGN.md
docs/*_DESIGN.md
docs/*_IMPLEMENTATION.md
docs/*_ROADMAP.md
docs/*_STATUS.md
```

Use internal docs for design options, implementation plans, experiments, and
status tracking. If internal material should become public, rewrite it into a
public guide/reference page instead of linking the internal file directly.

## Translation Requirements

Japanese public docs should preserve the same code examples as English docs.
Translate prose, not Restrict source code.

If immediate Japanese updates are not possible, keep the English page correct
and create a follow-up item rather than blocking implementation.

## Validation

Run:

```bash
mise exec -- cargo test --test quality_gates test_docs_hygiene::
mise exec -- cargo test --test quality_gates test_web_hygiene::
mdbook build docs
```

For the complete Pages artifact:

```bash
mise run docs-pages
```
