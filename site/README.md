# Restrict Pages Site

This directory contains the static GitHub Pages shell that hosts the public
landing page, blog, mdBook documentation, and the browser compiler together.

## Layout

- `/` is the landing page from `site/index.html`.
- `/blog/` is copied from `site/blog/`.
- `/docs/` is copied from `docs/book/` after `mdbook build`; the book source is
  `docs/public/`, not the internal design documents under `docs/`.
- `/compiler/` is copied from `web/` after the WebAssembly compiler build.
- Restrict code snippets use normal `<pre><code class="language-restrict">`
  blocks. `site/restrict-code-blocks.js` applies the shared highlighter on the
  landing page and blog.
- `/tools/highlight-theme-lab.html` is a noindex local/public utility for
  choosing token colors and exporting CSS for the shared `hljs-*` classes.

## Local Build

```sh
mdbook build docs
wasm-pack build --target web --out-dir web/pkg
bash scripts/build-pages.sh
```

The assembler fails fast if `docs/book/` or `web/pkg/` is missing, then writes
the finished artifact to `site/dist/`. Set `PAGES_DIST_DIR=/path/to/output`
when validating into a temporary directory.
