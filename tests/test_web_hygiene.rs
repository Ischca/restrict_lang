use restrict_lang::{parse_program, TypeChecker};
use std::fs;
use std::path::Path;

#[derive(Debug)]
struct EmbeddedExample {
    label: String,
    source: String,
}

#[test]
fn embedded_web_examples_use_v001_public_syntax() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let examples = embedded_web_examples(root);

    assert!(
        !examples.is_empty(),
        "web UI should expose at least one embedded Restrict example"
    );

    for example in examples {
        assert_current_web_example_syntax(&example.label, &example.source);
    }
}

#[test]
fn web_readme_does_not_advertise_removed_or_complete_language_support() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = read_fixture(root, "web/README.md");
    let lower_readme = readme.to_lowercase();

    assert!(
        !lower_readme.contains("all features"),
        "web/README.md should not claim that every Restrict feature is supported"
    );

    for removed_or_overstated in ["|>>", "if/else", "while loops"] {
        assert!(
            !readme.contains(removed_or_overstated),
            "web/README.md should not advertise `{removed_or_overstated}` as part of the web demo"
        );
    }
}

#[test]
fn playground_runs_generated_wasm_and_surfaces_program_output() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let html = read_fixture(root, "web/index.html");
    let app = read_fixture(root, "web/app.js");
    let readme = read_fixture(root, "web/README.md");

    for required in [
        r#"id="runBtn""#,
        r#"id="tab-output""#,
        r#"id="outputOutput""#,
        "Program output",
    ] {
        assert!(
            html.contains(required),
            "playground UI should expose `{required}`"
        );
    }

    for required in [
        "wat_to_wasm",
        "WebAssembly.instantiate",
        "wasi_snapshot_preview1",
        "fd_write",
        "programInstance.exports._start",
        "exampleGroups",
        "examplesById",
        "updateExampleGuide",
    ] {
        assert!(
            app.contains(required),
            "playground runtime should include `{required}`"
        );
    }

    assert!(
        readme.contains("## Browser Runtime") && readme.contains("Output tab"),
        "web README should explain how browser execution and output work"
    );
}

#[test]
fn playground_editor_overlay_keeps_caret_and_highlight_metrics_aligned() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let html = read_fixture(root, "web/index.html");

    for required in [
        "overflow-wrap: normal",
        "font-variant-ligatures: none",
        r#"font-feature-settings: "liga" 0, "calt" 0"#,
        "letter-spacing: 0",
        ".source-highlight .hljs-operator { font-weight: inherit; }",
        ".source-highlight .hljs-comment { font-style: inherit; }",
    ] {
        assert!(
            html.contains(required),
            "playground editor layers should share the text metric rule `{required}`"
        );
    }
}

#[test]
fn playground_separates_generic_forms_from_display_output() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let html = read_fixture(root, "web/index.html");
    let app = read_fixture(root, "web/app.js");
    let manifest = read_fixture(root, "samples/playground/manifest.json");
    let form = read_fixture(root, "samples/playground/form_contract.rl");
    let display = read_fixture(root, "samples/playground/display_types.rl");

    for required in ["form Labelled", "Badge takes Labelled", "<T of Labelled>"] {
        assert!(
            form.contains(required),
            "generic form example should include `{required}`"
        );
    }

    for required in [
        "Notice takes Display",
        "fun display: (self: Notice) -> String",
        "42 println",
        "Notice { text: \"record adoption\" } println",
    ] {
        assert!(
            display.contains(required),
            "Display example should include `{required}`"
        );
    }

    for required in ["formContract", "displayTypes", "affineDiagnostic"] {
        assert!(
            manifest.contains(required),
            "playground manifest should include `{required}`"
        );
    }

    for required in [
        r#"id="sampleGuide""#,
        r#"id="sampleDescription""#,
        r#"id="sampleExpectation""#,
    ] {
        assert!(
            html.contains(required),
            "playground should expose `{required}`"
        );
    }

    assert!(
        app.contains("populateExampleSelect") && app.contains("optionGroup.label = group.title"),
        "playground should build grouped example options from the generated catalog"
    );
}

#[test]
fn shell_installer_uses_portable_checksum_flags() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let installer = read_fixture(root, "install.sh");

    assert!(
        installer.contains("sha256sum -c -") && installer.contains("shasum -a 256 -c -"),
        "install.sh should explicitly read checksum lists from stdin on macOS and GNU implementations"
    );
    assert!(
        !installer.contains("sha256sum --check --status")
            && !installer.contains("shasum --algorithm 256 --check --status"),
        "install.sh should not rely on GNU-style long checksum options"
    );
}

#[test]
fn pages_shell_hosts_docs_blog_and_compiler_routes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    for path in [
        "site/index.html",
        "site/styles.css",
        "site/logo.svg",
        "site/favicon.svg",
        "site/404.html",
        "site/robots.txt",
        "site/sitemap.xml",
        "site/blog/index.html",
        "site/blog/introducing-restrict-v001.html",
        "site/tools/highlight-theme-lab.html",
        "site/build-pages.sh",
        "scripts/build-pages.sh",
        "docs/public/theme/index.hbs",
        "docs/public/theme/favicon.svg",
        "docs/public/theme/restrict-highlight.js",
        "site/restrict-highlight.js",
        "site/restrict-code-blocks.js",
        "web/restrict-highlight.js",
        "web/examples.js",
    ] {
        assert!(
            root.join(path).is_file(),
            "Pages source should include {path}"
        );
    }

    for removed_post in [
        "site/blog/type-inference-v001.html",
        "site/blog/runtime-dogfood.html",
        "site/blog/shipping-v001-preview.html",
    ] {
        assert!(
            !root.join(removed_post).exists(),
            "the v0.0.1 starting line should not republish historical post {removed_post}"
        );
    }

    let landing = read_fixture(root, "site/index.html");
    for link in [r#"href="docs/""#, r#"href="compiler/""#, r#"href="blog/""#] {
        assert!(
            landing.contains(link),
            "landing page should link to the co-hosted route {link}"
        );
    }

    let readme_logo = read_fixture(root, "assets/logo.svg");
    let site_logo = read_fixture(root, "site/logo.svg");
    assert_eq!(
        site_logo.split_whitespace().collect::<String>(),
        readme_logo.split_whitespace().collect::<String>(),
        "the Pages header should use the same logo artwork as README.md"
    );

    let compact_logo = read_fixture(root, "assets/logo-small.svg");
    for path in ["site/favicon.svg", "docs/public/theme/favicon.svg"] {
        assert_eq!(
            read_fixture(root, path)
                .split_whitespace()
                .collect::<String>(),
            compact_logo.split_whitespace().collect::<String>(),
            "{path} should use the compact Restrict logo artwork"
        );
    }

    for (path, logo_src) in [
        ("site/index.html", "logo.svg"),
        ("site/blog/index.html", "../logo.svg"),
        ("site/blog/introducing-restrict-v001.html", "../logo.svg"),
        ("site/404.html", "/restrict_lang/logo.svg"),
    ] {
        let html = read_fixture(root, path);
        assert!(
            html.contains(&format!(r#"class="brand-mark" src="{logo_src}""#)),
            "{path} should display the README logo in its header"
        );
    }

    let workflow = read_fixture(root, ".github/workflows/deploy-docs.yml");
    assert!(
        workflow.contains("actions/configure-pages@v6"),
        "Pages workflow should configure GitHub Pages before artifact upload"
    );
    assert!(
        workflow.contains("mdbook build docs"),
        "Pages workflow should build mdBook into docs/book"
    );
    assert!(
        workflow.contains("wasm-pack build --target web --out-dir web/pkg"),
        "Pages workflow should build the browser compiler bundle"
    );
    assert!(
        workflow.contains("node scripts/smoke-web-runtime.mjs"),
        "Pages workflow should execute the generated compiler and capture its output before deployment"
    );
    assert!(
        workflow.contains("bash scripts/sync_samples.sh --check"),
        "Pages workflow should reject a stale generated playground catalog"
    );
    assert!(
        workflow.contains("bash scripts/build-pages.sh") && workflow.contains("path: ./site/dist"),
        "Pages workflow should upload the assembled LP/docs/blog/compiler artifact"
    );
    assert!(
        workflow.contains("test -f site/dist/docs/index.html")
            && workflow.contains("test -f site/dist/compiler/pkg/restrict_lang.js")
            && workflow.contains("test -f site/dist/compiler/examples.js")
            && workflow.contains("test -f site/dist/compiler/restrict-highlight.js")
            && workflow.contains("test -f site/dist/favicon.svg")
            && workflow.contains("find site/dist/compiler/pkg -maxdepth 1 -type f -name '*.wasm'"),
        "Pages workflow should validate docs and compiler files before upload"
    );

    let book_config = read_fixture(root, "docs/book.toml");
    assert!(
        book_config.contains(r#"src = "public""#),
        "mdBook should use docs/public so internal design docs are not published"
    );
    assert!(
        book_config.contains(r#"theme = "public/theme""#),
        "mdBook should use the public theme under docs/public/theme"
    );
    assert!(
        book_config.contains(r#"site-url = "/restrict_lang/docs/""#),
        "mdBook site-url should reflect the /docs/ subdirectory"
    );
}

#[test]
fn release_blog_explains_the_language_and_establishes_the_v001_baseline() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let article = read_fixture(root, "site/blog/introducing-restrict-v001.html");

    for section in [
        "Why put the value first?",
        "Scope is a typed capability boundary",
        "Ownership is part of the expression",
        "Inference should remove repetition, not intent",
        "Records describe products; enums describe choices",
        "Forms express behavior without runtime objects",
        "The compiler is inspectable from source to Wasm",
        "A language also needs a working path around the compiler",
        "The starting line",
    ] {
        assert!(
            article.contains(section),
            "v0.0.1 article should explain the language through the `{section}` section"
        );
    }

    for current_syntax in [
        "42 Option::Some",
        "() Option::None",
        "42 Result::Ok",
        "DecodeError::Invalid",
        "context Logging",
        "with RequestScope",
        "with Arena { }",
        "&lt;T of Labelled&gt;",
        "value |&gt; label",
    ] {
        assert!(
            article.contains(current_syntax),
            "v0.0.1 article should demonstrate current syntax `{current_syntax}`"
        );
    }

    for stale_framing in ["post-v0.0.1", "v0.0.1 preview", "pre-release"] {
        assert!(
            !article.to_lowercase().contains(stale_framing),
            "v0.0.1 article should not retain stale framing `{stale_framing}`"
        );
    }

    let scope_index = article
        .find("Scope is a typed capability boundary")
        .expect("v0.0.1 article should explain typed scopes");
    let ownership_index = article
        .find("Ownership is part of the expression")
        .expect("v0.0.1 article should explain affine ownership");
    assert!(
        scope_index < ownership_index,
        "the distinctive scope model should be introduced before ownership details"
    );

    assert!(
        !article.contains("Restrict is a small")
            && !article.contains("small, statically typed language"),
        "the article should not characterize the whole language as small"
    );
}

#[test]
fn release_blog_restrict_examples_parse_and_type_check() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let article = read_fixture(root, "site/blog/introducing-restrict-v001.html");
    let examples = extract_restrict_code_blocks(&article);

    assert_eq!(
        examples.len(),
        6,
        "v0.0.1 article should keep its six language examples under test"
    );

    for (index, source) in examples.iter().enumerate() {
        let label = format!("v0.0.1 article example {}", index + 1);
        assert_current_web_example_syntax(&label, source);

        let (remaining, program) = parse_program(source)
            .unwrap_or_else(|error| panic!("{label} should parse: {error:?}\n{source}"));
        assert!(
            remaining.trim().is_empty(),
            "{label} should parse all input, remaining: {remaining:?}\n{source}"
        );

        TypeChecker::new()
            .check_program(&program)
            .unwrap_or_else(|error| panic!("{label} should type-check: {error}\n{source}"));
    }
}

#[test]
fn tagged_releases_are_stable_and_use_curated_notes_when_available() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow = read_fixture(root, ".github/workflows/release.yml");
    let notes = read_fixture(root, "docs/releases/v0.0.1.md");

    for required in [
        "name: Publish GitHub release",
        "notes_file=\"docs/releases/${{ github.ref_name }}.md\"",
        "--notes-file \"$notes_file\"",
        "--generate-notes",
    ] {
        assert!(
            workflow.contains(required),
            "release workflow should include `{required}`"
        );
    }

    assert!(
        !workflow.contains("--prerelease") && !workflow.contains("pre-release"),
        "the first tagged release should be published as a stable GitHub release"
    );

    for required in [
        "first public release",
        "## Language highlights",
        "## Compiler and tools",
        "## Deliberate boundaries",
        "## Start here",
    ] {
        assert!(
            notes.contains(required),
            "v0.0.1 release notes should include `{required}`"
        );
    }
}

#[test]
fn restrict_highlighting_is_shared_by_docs_and_compiler() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let docs_highlighter = read_fixture(root, "docs/public/theme/restrict-highlight.js");
    let compiler_highlighter = read_fixture(root, "web/restrict-highlight.js");
    let site_highlighter = read_fixture(root, "site/restrict-highlight.js");
    let site_initializer = read_fixture(root, "site/restrict-code-blocks.js");
    let theme_lab = read_fixture(root, "site/tools/highlight-theme-lab.html");
    let site_styles = read_fixture(root, "site/styles.css");
    let docs_theme = read_fixture(root, "docs/public/theme/restrict-lang.css");
    let docs_theme_script = read_fixture(root, "docs/public/theme/restrict-lang.js");
    let docs_rustdoc_theme = read_fixture(root, "docs/public/theme/rustdoc-restrict.css");
    let docs_template = read_fixture(root, "docs/public/theme/index.hbs");
    let landing_html = read_fixture(root, "site/index.html");
    let release_post = read_fixture(root, "site/blog/introducing-restrict-v001.html");
    let compiler_html = read_fixture(root, "web/index.html");
    let compiler_app = read_fixture(root, "web/app.js");
    let build_script = read_fixture(root, "site/build-pages.sh");

    assert_eq!(
        docs_highlighter, compiler_highlighter,
        "docs and compiler should use the same Restrict highlighter rules"
    );
    assert_eq!(
        docs_highlighter, site_highlighter,
        "LP and blog should use the same Restrict highlighter rules as docs"
    );
    assert!(
        docs_template.contains(r#"<script src="{{ resource "highlight.js" }}"></script>"#)
            && docs_template.contains(
                r#"<script src="{{ resource "theme/restrict-highlight.js" }}"></script>"#
            )
            && docs_template.contains(r#"<script src="{{ resource "book.js" }}"></script>"#),
        "mdBook should register Restrict highlighting between highlight.js and book.js"
    );
    assert!(
        site_initializer.contains("function highlightRestrictBlocks")
            && site_initializer.contains("pre code.language-restrict")
            && site_initializer.contains("highlighter.highlightRestrict(block.textContent)")
            && site_initializer.contains("global.RestrictCodeBlocks"),
        "static Pages shell should expose a reusable Restrict code block highlighter"
    );
    assert!(
        !docs_theme.contains("language-restrict::before")
            && !docs_theme.contains(r#"content: "OSV""#)
            && !docs_theme_script.contains(r#"content: "OSV""#)
            && !docs_theme_script.contains(".osv-line::after")
            && !docs_rustdoc_theme.contains(r#"content: "OSV""#)
            && !docs_rustdoc_theme.contains(".osv-example::before"),
        "docs Restrict code blocks should not render an OSV pseudo-label over source code"
    );
    for (path, content) in [
        ("site/styles.css", &site_styles),
        ("docs/public/theme/restrict-lang.css", &docs_theme),
        ("web/index.html", &compiler_html),
    ] {
        assert!(
            content.contains("#ff6b35")
                && content.contains("#f7931e")
                && content.contains("#c1440e")
                && content.contains("#ffaa55"),
            "{path} should derive LP and syntax colors from the logo palette"
        );

        for legacy_color in [
            "#176b87", "#0f4f66", "#4CAF50", "#45a049", "#7dd3fc", "#c4b5fd", "#7c3aed",
        ] {
            assert!(
                !content.contains(legacy_color),
                "{path} should not keep the pre-logo highlight/primary color {legacy_color}"
            );
        }
    }
    assert!(
        theme_lab.contains(r#"<meta name="robots" content="noindex">"#)
            && theme_lab.contains(r#"src="../restrict-highlight.js""#)
            && theme_lab.contains("window.RestrictHighlight.highlightRestrict(sampleSource)")
            && theme_lab.contains("--rl-syntax-keyword")
            && theme_lab.contains(".language-restrict .hljs-operator")
            && theme_lab.contains("navigator.clipboard.writeText(css)"),
        "theme lab should preview Restrict tokens and export CSS for the shared hljs classes"
    );
    for (path, html, highlighter_src, initializer_src) in [
        (
            "site/index.html",
            &landing_html,
            r#"src="restrict-highlight.js""#,
            r#"src="restrict-code-blocks.js""#,
        ),
        (
            "site/blog/introducing-restrict-v001.html",
            &release_post,
            r#"src="../restrict-highlight.js""#,
            r#"src="../restrict-code-blocks.js""#,
        ),
    ] {
        assert!(
            html.contains(r#"<code class="language-restrict">"#)
                && html.contains(highlighter_src)
                && html.contains(initializer_src),
            "{path} should use language-restrict code blocks and load the shared static highlighter"
        );
    }
    assert!(
        compiler_html.contains(r#"id="sourceHighlight""#)
            && compiler_html.contains(r#"src="./restrict-highlight.js""#),
        "online compiler should load and render the Restrict source highlighter"
    );
    assert!(
        compiler_app.contains("function syncSourceHighlight()")
            && compiler_app.contains("highlighter.highlightRestrict(source.value)")
            && compiler_app.contains("source.addEventListener('input', syncSourceHighlight)"),
        "online compiler should keep the highlight layer synced with textarea input"
    );
    assert!(
        build_script.contains("require_file \"$ROOT_DIR/web/restrict-highlight.js\"")
            && build_script.contains("cp \"$ROOT_DIR/web/restrict-highlight.js\" \"$TMP_DIR/compiler/restrict-highlight.js\"")
            && build_script.contains("require_file \"$SITE_DIR/restrict-highlight.js\"")
            && build_script.contains("require_file \"$SITE_DIR/restrict-code-blocks.js\"")
            && build_script.contains("require_file \"$SITE_DIR/tools/highlight-theme-lab.html\"")
            && build_script.contains("cp \"$SITE_DIR/restrict-highlight.js\" \"$TMP_DIR/restrict-highlight.js\"")
            && build_script.contains("cp \"$SITE_DIR/restrict-code-blocks.js\" \"$TMP_DIR/restrict-code-blocks.js\"")
            && build_script.contains("cp \"$SITE_DIR/tools/\"*.html \"$TMP_DIR/tools/\"")
            && build_script.contains("require_file \"$TMP_DIR/tools/highlight-theme-lab.html\""),
        "Pages assembler should publish static-site and compiler highlighter assets"
    );
}

#[test]
fn pages_static_html_has_public_metadata() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    for (path, canonical_url, og_type) in [
        (
            "site/index.html",
            "https://ischca.github.io/restrict_lang/",
            "website",
        ),
        (
            "site/blog/index.html",
            "https://ischca.github.io/restrict_lang/blog/",
            "website",
        ),
        (
            "site/blog/introducing-restrict-v001.html",
            "https://ischca.github.io/restrict_lang/blog/introducing-restrict-v001.html",
            "article",
        ),
    ] {
        let html = read_fixture(root, path);

        for required in [
            r#"<meta name="description""#,
            r#"<meta name="theme-color""#,
            r#"<meta property="og:site_name" content="Restrict Language">"#,
            r#"<meta property="og:title""#,
            r#"<meta property="og:description""#,
            r#"<meta name="twitter:card" content="summary">"#,
            r#"<link rel="icon""#,
        ] {
            assert!(html.contains(required), "{path} should include {required}");
        }

        assert!(
            html.contains(&format!(r#"<meta property="og:type" content="{og_type}">"#)),
            "{path} should expose a stable Open Graph type"
        );
        assert!(
            html.contains(&format!(
                r#"<meta property="og:url" content="{canonical_url}">"#
            )),
            "{path} should expose its public Pages URL"
        );
        assert!(
            html.contains(&format!(r#"<link rel="canonical" href="{canonical_url}">"#)),
            "{path} should expose a canonical URL"
        );
    }
}

#[test]
fn pages_auxiliary_routes_are_publishable() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let not_found = read_fixture(root, "site/404.html");
    let robots = read_fixture(root, "site/robots.txt");
    let sitemap = read_fixture(root, "site/sitemap.xml");

    assert!(
        not_found.contains(r#"<meta name="robots" content="noindex">"#)
            && not_found.contains(r#"href="/restrict_lang/docs/""#)
            && not_found.contains(r#"href="/restrict_lang/compiler/""#)
            && not_found.contains(r#"href="/restrict_lang/styles.css""#),
        "404 page should be non-indexed and route visitors back to key Pages sections"
    );
    assert!(
        robots.contains("Sitemap: https://ischca.github.io/restrict_lang/sitemap.xml"),
        "robots.txt should point crawlers at the Pages sitemap"
    );

    for public_url in [
        "https://ischca.github.io/restrict_lang/",
        "https://ischca.github.io/restrict_lang/docs/",
        "https://ischca.github.io/restrict_lang/compiler/",
        "https://ischca.github.io/restrict_lang/blog/",
        "https://ischca.github.io/restrict_lang/blog/introducing-restrict-v001.html",
    ] {
        assert!(
            sitemap.contains(&format!("<loc>{public_url}</loc>")),
            "sitemap.xml should include {public_url}"
        );
    }
}

#[test]
fn pages_public_route_hrefs_resolve_from_expected_bases() {
    assert_eq!(
        resolve_public_path("/restrict_lang/", "docs/"),
        "/restrict_lang/docs/"
    );
    assert_eq!(
        resolve_public_path("/restrict_lang/", "compiler/"),
        "/restrict_lang/compiler/"
    );
    assert_eq!(
        resolve_public_path("/restrict_lang/blog/", "../docs/"),
        "/restrict_lang/docs/"
    );
    assert_eq!(
        resolve_public_path(
            "/restrict_lang/docs/en/guide/syntax.html",
            "/restrict_lang/compiler/"
        ),
        "/restrict_lang/compiler/"
    );
    assert_eq!(
        resolve_public_path(
            "/restrict_lang/docs/missing/deep/page",
            "/restrict_lang/styles.css"
        ),
        "/restrict_lang/styles.css"
    );
}

#[test]
fn mdbook_theme_opens_playground_from_pages_root() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let theme = read_fixture(root, "docs/public/theme/restrict-lang.js");

    assert!(
        theme.contains("function pagesSiteRoot()")
            && theme.contains("pathname.indexOf('/docs/')")
            && theme.contains("`${pagesSiteRoot()}compiler/`"),
        "mdBook theme should compute the compiler URL from the Pages root"
    );
    assert!(
        !theme.contains("const playgroundUrl = '../compiler/'"),
        "mdBook theme should not use a fixed relative compiler URL from nested docs pages"
    );
}

#[test]
fn pages_build_script_fails_before_partial_artifacts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = read_fixture(root, "site/build-pages.sh");

    for required in [
        "require_file \"$ROOT_DIR/docs/book/index.html\"",
        "require_dir \"$ROOT_DIR/web/pkg\"",
        "require_file \"$ROOT_DIR/web/pkg/restrict_lang.js\"",
        "require_file \"$ROOT_DIR/web/examples.js\"",
        "require_file \"$SITE_DIR/tools/highlight-theme-lab.html\"",
        "require_file \"$SITE_DIR/blog/introducing-restrict-v001.html\"",
        "require_file \"$SITE_DIR/logo.svg\"",
        "cp \"$SITE_DIR/logo.svg\" \"$TMP_DIR/logo.svg\"",
        "require_file \"$TMP_DIR/logo.svg\"",
        "cp \"$SITE_DIR/tools/\"*.html \"$TMP_DIR/tools/\"",
        "require_file \"$TMP_DIR/tools/highlight-theme-lab.html\"",
        "require_file \"$TMP_DIR/blog/introducing-restrict-v001.html\"",
        "require_file \"$TMP_DIR/compiler/examples.js\"",
        "does not contain a .wasm bundle",
        "mktemp -d",
        "mv \"$TMP_DIR\" \"$DIST_DIR\"",
    ] {
        assert!(
            script.contains(required),
            "site/build-pages.sh should include `{required}`"
        );
    }

    let preflight_index = script
        .find("require_file \"$ROOT_DIR/docs/book/index.html\"")
        .expect("build script should preflight docs/book/index.html");
    let replace_index = script
        .find("rm -rf \"$DIST_DIR\"")
        .expect("build script should replace site/dist only after staging");
    assert!(
        preflight_index < replace_index,
        "site/build-pages.sh should validate required inputs before removing site/dist"
    );
}

fn embedded_web_examples(root: &Path) -> Vec<EmbeddedExample> {
    let mut examples = Vec::new();
    let index = read_fixture(root, "web/index.html");

    if let Some(source) = extract_textarea(&index, "sourceCode") {
        examples.push(EmbeddedExample {
            label: "web/index.html textarea#sourceCode".to_string(),
            source,
        });
    }

    for (index, source) in extract_example_code_divs(&index).into_iter().enumerate() {
        examples.push(EmbeddedExample {
            label: format!("web/index.html .example-code[{}]", index + 1),
            source,
        });
    }

    let samples_dir = root.join("samples/playground");
    let mut sample_paths = fs::read_dir(&samples_dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", samples_dir.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rl"))
        .collect::<Vec<_>>();
    sample_paths.sort();

    for path in sample_paths {
        examples.push(EmbeddedExample {
            label: path.display().to_string(),
            source: fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display())),
        });
    }

    examples
}

fn extract_textarea(markup: &str, id: &str) -> Option<String> {
    let marker = format!(r#"<textarea id="{id}""#);
    let start = markup.find(&marker)?;
    let content_start = start + markup[start..].find('>')? + 1;
    let content_end = content_start + markup[content_start..].find("</textarea>")?;

    Some(decode_html_text(&markup[content_start..content_end]))
}

fn extract_example_code_divs(markup: &str) -> Vec<String> {
    let marker = r#"<div class="example-code">"#;
    let mut snippets = Vec::new();
    let mut remaining = markup;

    while let Some(start) = remaining.find(marker) {
        let content_start = start + marker.len();
        let Some(end_offset) = remaining[content_start..].find("</div>") else {
            break;
        };
        let content_end = content_start + end_offset;
        snippets.push(decode_html_text(&remaining[content_start..content_end]));
        remaining = &remaining[content_end + "</div>".len()..];
    }

    snippets
}

fn extract_restrict_code_blocks(markup: &str) -> Vec<String> {
    let marker = r#"<pre><code class="language-restrict">"#;
    let closing = "</code></pre>";
    let mut snippets = Vec::new();
    let mut remaining = markup;

    while let Some(start) = remaining.find(marker) {
        let content_start = start + marker.len();
        let Some(end_offset) = remaining[content_start..].find(closing) else {
            break;
        };
        let content_end = content_start + end_offset;
        snippets.push(decode_html_text(&remaining[content_start..content_end]));
        remaining = &remaining[content_end + closing.len()..];
    }

    snippets
}

fn assert_current_web_example_syntax(label: &str, source: &str) {
    let code_only = source
        .lines()
        .map(strip_line_comment)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        !code_only.contains("|>>"),
        "{label} should not use the removed mutable pipe operator:\n{source}"
    );
    assert!(
        !code_only.contains("[|"),
        "{label} should use list or array literals with `[`:\n{source}"
    );

    for (stale, replacement) in [
        ("let", "val"),
        ("fn", "fun"),
        ("if", "then/else"),
        ("Int", "Int32"),
        ("Float", "Float64"),
        ("Bool", "Boolean"),
        ("Unit", "()"),
    ] {
        assert!(
            !contains_word(&code_only, stale),
            "{label} should use {replacement} instead of stale `{stale}` syntax:\n{source}"
        );
    }

    assert_no_record_field_assignments(label, &code_only, source);

    let mut in_form_contract = false;

    for (line_index, line) in code_only.lines().enumerate() {
        let line_number = line_index + 1;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        // Semicolons are part of the current surface when an
        // identifier-started expression must not extend the preceding OSV
        // chain. Parsing and sample compilation enforce valid placement; this
        // hygiene pass should not reject the separator itself.

        if trimmed.starts_with("form ") || trimmed.starts_with("pub form ") {
            in_form_contract = true;
        }

        assert_current_function_declaration(label, line_number, line, source, in_form_contract);
        assert_no_traditional_call_syntax(label, line_number, line, source);

        if in_form_contract && trimmed == "}" {
            in_form_contract = false;
        }
    }
}

fn assert_current_function_declaration(
    label: &str,
    line_number: usize,
    line: &str,
    source: &str,
    in_form_contract: bool,
) {
    let trimmed = line.trim_start();
    let Some(after_fun) = trimmed.strip_prefix("fun ") else {
        return;
    };
    let before_body = after_fun.split('=').next().unwrap_or(after_fun);

    let has_parameter_list =
        before_body.contains(": (") || (before_body.contains(": <") && before_body.contains(">("));
    assert!(
        has_parameter_list,
        "{label}:{line_number} should use `fun name: (...) -> Type =` or generic `fun name: <T>(...) -> Type =` syntax:\n{source}"
    );
    let is_inferred_main = before_body.trim() == "main: ()";
    assert!(
        before_body.contains(" -> ") || is_inferred_main,
        "{label}:{line_number} should include an explicit return type, except for the canonical `fun main: () =` entry point:\n{source}"
    );

    if in_form_contract {
        assert!(
            !trimmed.contains(" =") && !trimmed.ends_with('{'),
            "{label}:{line_number} form contracts should declare signatures without bodies:\n{source}"
        );
        return;
    }

    assert!(
        trimmed.contains(" ="),
        "{label}:{line_number} should include `=` before the function body:\n{source}"
    );
}

fn assert_no_record_field_assignments(label: &str, code_only: &str, source: &str) {
    let mut in_record_context = false;

    for (line_index, line) in code_only.lines().enumerate() {
        let line_number = line_index + 1;

        if in_record_context && starts_with_field_assignment(line) {
            panic!(
                "{label}:{line_number} should use `field: value` or `field: Type`, not `field = ...`:\n{source}"
            );
        }

        for open in record_context_open_positions(line) {
            let close = line[open + 1..].find('}').map(|offset| open + 1 + offset);
            let segment_end = close.unwrap_or(line.len());
            let segment = &line[open + 1..segment_end];

            if segment_has_field_assignment(segment) {
                panic!("{label}:{line_number} should use colon-delimited record fields:\n{source}");
            }

            if close.is_none() {
                in_record_context = true;
            }
        }

        if in_record_context && line.contains('}') {
            in_record_context = false;
        }
    }
}

fn record_context_open_positions(line: &str) -> Vec<usize> {
    line.match_indices('{')
        .filter_map(|(open, _)| {
            let prefix = line[..open].trim_end();
            if prefix.ends_with('=') {
                return None;
            }

            let word = last_identifier_before(line, open)?;
            let is_record_keyword = prefix.trim_start().starts_with("record ");
            let starts_with_uppercase = word
                .chars()
                .next()
                .is_some_and(|char_| char_.is_ascii_uppercase());

            (is_record_keyword || starts_with_uppercase).then_some(open)
        })
        .collect()
}

fn starts_with_field_assignment(line: &str) -> bool {
    let trimmed = line.trim_start();
    let chars: Vec<_> = trimmed.chars().collect();

    if chars.first().is_none_or(|char_| !is_ident_start(*char_)) {
        return false;
    }

    let ident_end = ident_end(&chars, 0);
    let after_space = skip_space(&chars, ident_end);
    chars.get(after_space) == Some(&'=')
        && chars.get(after_space + 1) != Some(&'=')
        && chars.get(after_space + 1) != Some(&'>')
}

fn segment_has_field_assignment(segment: &str) -> bool {
    let chars: Vec<_> = segment.chars().collect();
    let mut index = 0;

    while index < chars.len() {
        if !is_ident_start(chars[index]) {
            index += 1;
            continue;
        }

        let ident_end = ident_end(&chars, index);
        let after_space = skip_space(&chars, ident_end);
        if chars.get(after_space) == Some(&'=')
            && chars.get(after_space + 1) != Some(&'=')
            && chars.get(after_space + 1) != Some(&'>')
        {
            return true;
        }
        index = ident_end;
    }

    false
}

fn assert_no_traditional_call_syntax(label: &str, line_number: usize, line: &str, source: &str) {
    let trimmed = line.trim_start();
    if trimmed.starts_with("fun ") {
        return;
    }

    let chars: Vec<_> = line.chars().collect();
    let mut index = 0;

    while index < chars.len() {
        if chars[index] == '.' {
            let method_start = skip_space(&chars, index + 1);
            if chars
                .get(method_start)
                .is_some_and(|char_| is_ident_start(*char_))
            {
                let method_end = ident_end(&chars, method_start);
                let after_space = skip_space(&chars, method_end);
                if chars.get(after_space) == Some(&'(') {
                    panic!(
                        "{label}:{line_number} should use OSV helper calls instead of object-style method calls:\n{source}"
                    );
                }
            }
        }

        if !is_ident_start(chars[index]) {
            index += 1;
            continue;
        }

        let ident_start = index;
        let ident_end = ident_end(&chars, ident_start);
        let ident: String = chars[ident_start..ident_end].iter().collect();
        let after_space = skip_space(&chars, ident_end);

        if chars.get(after_space) == Some(&'(')
            && !ident
                .chars()
                .next()
                .is_some_and(|char_| char_.is_ascii_uppercase())
            && !matches!(
                ident.as_str(),
                "fun" | "record" | "val" | "mut" | "match" | "then" | "else"
            )
        {
            panic!(
                "{label}:{line_number} should use OSV calls instead of `{ident}(...)`:\n{source}"
            );
        }

        index = ident_end;
    }
}

fn read_fixture(root: &Path, relative_path: &str) -> String {
    fs::read_to_string(root.join(relative_path))
        .unwrap_or_else(|err| panic!("failed to read {relative_path}: {err}"))
}

fn resolve_public_path(base_path: &str, href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }

    if href.starts_with('/') {
        return normalize_public_path(href);
    }

    let base_dir = if base_path.ends_with('/') {
        base_path.to_string()
    } else {
        base_path
            .rsplit_once('/')
            .map(|(prefix, _)| format!("{prefix}/"))
            .unwrap_or_else(|| "/".to_string())
    };

    normalize_public_path(&format!("{base_dir}{href}"))
}

fn normalize_public_path(path: &str) -> String {
    let keep_trailing_slash = path.ends_with('/');
    let mut parts = Vec::new();

    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }

    let mut normalized = format!("/{}", parts.join("/"));
    if keep_trailing_slash && !normalized.ends_with('/') {
        normalized.push('/');
    }
    normalized
}

fn decode_html_text(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
        .trim()
        .to_string()
}

fn strip_line_comment(line: &str) -> &str {
    line.split("//").next().unwrap_or(line)
}

fn contains_word(line: &str, word: &str) -> bool {
    line.match_indices(word).any(|(index, _)| {
        let before = line[..index].chars().next_back();
        let after = line[index + word.len()..].chars().next();

        is_boundary(before) && is_boundary(after)
    })
}

fn last_identifier_before(line: &str, offset: usize) -> Option<&str> {
    let prefix = &line[..offset];
    let end = prefix
        .char_indices()
        .rev()
        .find_map(|(index, char_)| is_ident_continue(char_).then_some(index + char_.len_utf8()))?;
    let start = prefix[..end]
        .char_indices()
        .rev()
        .find_map(|(index, char_)| (!is_ident_continue(char_)).then_some(index + char_.len_utf8()))
        .unwrap_or(0);

    Some(&prefix[start..end])
}

fn ident_end(chars: &[char], start: usize) -> usize {
    let mut end = start + 1;
    while chars
        .get(end)
        .is_some_and(|char_| is_ident_continue(*char_))
    {
        end += 1;
    }
    end
}

fn skip_space(chars: &[char], start: usize) -> usize {
    let mut index = start;
    while chars.get(index).is_some_and(|char_| char_.is_whitespace()) {
        index += 1;
    }
    index
}

fn is_boundary(char_: Option<char>) -> bool {
    char_.is_none_or(|char_| !is_ident_continue(char_))
}

fn is_ident_start(char_: char) -> bool {
    char_ == '_' || char_.is_ascii_alphabetic()
}

fn is_ident_continue(char_: char) -> bool {
    char_ == '_' || char_.is_ascii_alphanumeric()
}
