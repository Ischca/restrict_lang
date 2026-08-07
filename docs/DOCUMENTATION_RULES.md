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
- scalar host exports only
- closed, non-generic, non-recursive user enums may use qualified
  `Type::Variant` names and exhaustive matches
- method-only forms, concrete non-generic record adoptions, and explicit `of`
  bounds may be documented as current post-v0.0.1 behavior
- WebAssembly is the sole code-generation target; Web, cloud, WASI, and
  container integrations are host profiles or adapters, not additional
  language backends
- generated JavaScript glue may be described as a current host requirement,
  but not as a JavaScript backend or direct Wasm access to browser APIs
- generic or recursive user enums, enum host ABI, TAT, associated types,
  generic/conditional/enum adoptions, dynamic dispatch, and composite host ABI
  marked as future work

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
mise exec -- cargo test --test test_docs_hygiene
mise exec -- cargo test --test test_web_hygiene
mdbook build docs
```

For the complete Pages artifact:

```bash
mise run docs-pages
```
