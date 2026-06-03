# Restrict Documentation

This directory intentionally separates internal project documentation from the
language documentation published through GitHub Pages.

## Public Pages Documentation

The public mdBook source lives under:

```text
docs/public/
├── SUMMARY.md
├── en/
├── ja/
└── theme/
```

`docs/book.toml` uses `src = "public"`, so `mdbook build docs` publishes only
the public book source into `docs/book/`. The Pages assembler then copies
`docs/book/` to `/docs/` in the site artifact.

Public docs should be written for language users. They should explain the
current v0.0.1 surface clearly and avoid internal implementation plans unless
the page is explicitly describing a public release boundary.

## Internal Documentation

Internal design notes, implementation plans, experiments, and status documents
stay under `docs/` outside `docs/public/`.

Examples:

```text
docs/TYPE_INFERENCE_DESIGN.md
docs/STDLIB_ARCHITECTURE.md
docs/TEMPORAL_*.md
docs/*_DESIGN.md
docs/*_IMPLEMENTATION.md
```

These files are for compiler development and design discussion. Do not link
them from `docs/public/SUMMARY.md` as user-facing documentation. If an internal
design needs to become public documentation, rewrite it into a user-facing page
under `docs/public/` instead of linking the internal document directly.

## Build Commands

Build the public mdBook:

```bash
mdbook build docs
```

Build the full Pages artifact:

```bash
mise run docs-pages
```

That task builds:

```text
docs/book/      public mdBook output
web/pkg/        wasm-pack browser compiler bundle
site/dist/      final Pages artifact
```

## Editing Rules

When editing public docs:

- edit `docs/public/en/` first
- update `docs/public/SUMMARY.md` for visible navigation changes
- keep examples on the v0.0.1 release surface
- use `val`, `mut val`, OSV calls, and `:` record fields
- avoid TAT, user-defined ADTs, `form`/`takes`, and composite host ABI examples
  unless they are clearly marked as future work

When editing internal docs:

- keep design notes outside `docs/public/`
- prefer explicit status labels such as supported, rejected, experimental, or
  future
- do not assume an internal design document is user-facing release behavior

## Validation

Run focused docs checks after changing public docs:

```bash
mise exec -- cargo test --test test_docs_hygiene
mise exec -- cargo test --test test_web_hygiene
```

Run `mdbook build docs` before assembling Pages.
