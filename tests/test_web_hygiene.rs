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
fn pages_shell_hosts_docs_blog_and_compiler_routes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    for path in [
        "site/index.html",
        "site/styles.css",
        "site/favicon.svg",
        "site/404.html",
        "site/robots.txt",
        "site/sitemap.xml",
        "site/blog/index.html",
        "site/blog/type-inference-v001.html",
        "site/blog/runtime-dogfood.html",
        "site/tools/highlight-theme-lab.html",
        "site/build-pages.sh",
        "scripts/build-pages.sh",
        "docs/public/theme/index.hbs",
        "docs/public/theme/restrict-highlight.js",
        "site/restrict-highlight.js",
        "site/restrict-code-blocks.js",
        "web/restrict-highlight.js",
    ] {
        assert!(
            root.join(path).is_file(),
            "Pages source should include {path}"
        );
    }

    let landing = read_fixture(root, "site/index.html");
    for link in [r#"href="docs/""#, r#"href="compiler/""#, r#"href="blog/""#] {
        assert!(
            landing.contains(link),
            "landing page should link to the co-hosted route {link}"
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
        workflow.contains("bash scripts/build-pages.sh") && workflow.contains("path: ./site/dist"),
        "Pages workflow should upload the assembled LP/docs/blog/compiler artifact"
    );
    assert!(
        workflow.contains("test -f site/dist/docs/index.html")
            && workflow.contains("test -f site/dist/compiler/pkg/restrict_lang.js")
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
    let inference_post = read_fixture(root, "site/blog/type-inference-v001.html");
    let dogfood_post = read_fixture(root, "site/blog/runtime-dogfood.html");
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
            "site/blog/type-inference-v001.html",
            &inference_post,
            r#"src="../restrict-highlight.js""#,
            r#"src="../restrict-code-blocks.js""#,
        ),
        (
            "site/blog/runtime-dogfood.html",
            &dogfood_post,
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
            "https://restrict-lang.github.io/restrict_lang/",
            "website",
        ),
        (
            "site/blog/index.html",
            "https://restrict-lang.github.io/restrict_lang/blog/",
            "website",
        ),
        (
            "site/blog/type-inference-v001.html",
            "https://restrict-lang.github.io/restrict_lang/blog/type-inference-v001.html",
            "article",
        ),
        (
            "site/blog/runtime-dogfood.html",
            "https://restrict-lang.github.io/restrict_lang/blog/runtime-dogfood.html",
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
        robots.contains("Sitemap: https://restrict-lang.github.io/restrict_lang/sitemap.xml"),
        "robots.txt should point crawlers at the Pages sitemap"
    );

    for public_url in [
        "https://restrict-lang.github.io/restrict_lang/",
        "https://restrict-lang.github.io/restrict_lang/docs/",
        "https://restrict-lang.github.io/restrict_lang/compiler/",
        "https://restrict-lang.github.io/restrict_lang/blog/",
        "https://restrict-lang.github.io/restrict_lang/blog/type-inference-v001.html",
        "https://restrict-lang.github.io/restrict_lang/blog/runtime-dogfood.html",
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
        "require_file \"$SITE_DIR/tools/highlight-theme-lab.html\"",
        "cp \"$SITE_DIR/tools/\"*.html \"$TMP_DIR/tools/\"",
        "require_file \"$TMP_DIR/tools/highlight-theme-lab.html\"",
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

    let app = read_fixture(root, "web/app.js");
    for (index, source) in extract_restrict_template_literals(&app)
        .into_iter()
        .enumerate()
    {
        examples.push(EmbeddedExample {
            label: format!("web/app.js example template[{}]", index + 1),
            source,
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

fn extract_restrict_template_literals(javascript: &str) -> Vec<String> {
    javascript_template_literals(javascript)
        .into_iter()
        .filter(|literal| literal.contains("fun ") || literal.contains("record "))
        .collect()
}

fn javascript_template_literals(javascript: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let bytes = javascript.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] != b'`' {
            index += 1;
            continue;
        }

        let literal_start = index + 1;
        index += 1;
        let mut escaped = false;

        while index < bytes.len() {
            match bytes[index] {
                b'\\' if !escaped => {
                    escaped = true;
                    index += 1;
                }
                b'`' if !escaped => {
                    literals.push(javascript[literal_start..index].to_string());
                    index += 1;
                    break;
                }
                _ => {
                    escaped = false;
                    index += 1;
                }
            }
        }
    }

    literals
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

    for (line_index, line) in code_only.lines().enumerate() {
        let line_number = line_index + 1;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        assert!(
            !trimmed.contains(';'),
            "{label}:{line_number} should not use semicolons in embedded examples:\n{source}"
        );
        assert_current_function_declaration(label, line_number, line, source);
        assert_no_traditional_call_syntax(label, line_number, line, source);
    }
}

fn assert_current_function_declaration(label: &str, line_number: usize, line: &str, source: &str) {
    let trimmed = line.trim_start();
    let Some(after_fun) = trimmed.strip_prefix("fun ") else {
        return;
    };
    let before_body = after_fun.split('=').next().unwrap_or(after_fun);

    assert!(
        before_body.contains(": ("),
        "{label}:{line_number} should use `fun name: (...) -> Type =` syntax:\n{source}"
    );
    assert!(
        before_body.contains(" -> "),
        "{label}:{line_number} should include an explicit return type:\n{source}"
    );
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
