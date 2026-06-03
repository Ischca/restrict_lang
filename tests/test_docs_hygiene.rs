use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const CURRENT_CORE_DOCS: &[&str] = &[
    "README.md",
    "docs/public/en/introduction.md",
    "docs/public/en/getting-started/hello-world.md",
    "docs/public/en/getting-started/quick-start.md",
    "docs/public/en/guide/README.md",
    "docs/public/en/guide/osv-order.md",
    "docs/public/en/guide/patterns.md",
    "docs/public/en/guide/affine-types.md",
    "docs/public/en/guide/syntax.md",
    "docs/public/en/guide/types.md",
    "docs/public/en/guide/type-inference.md",
    "docs/public/en/guide/functions.md",
    "docs/public/en/guide/records.md",
    "docs/public/en/guide/variables.md",
];

const CURRENT_JAPANESE_DOCS: &[&str] = &[
    "docs/public/ja/introduction.md",
    "docs/public/ja/getting-started/quick-start.md",
    "docs/public/ja/getting-started/hello-world.md",
    "docs/public/ja/guide/syntax.md",
    "docs/public/ja/guide/osv-order.md",
    "docs/public/ja/guide/variables.md",
    "docs/public/ja/guide/types.md",
    "docs/public/ja/guide/functions.md",
    "docs/public/ja/guide/patterns.md",
    "docs/public/ja/guide/affine-types.md",
    "docs/public/ja/guide/records.md",
    "docs/public/ja/guide/warder.md",
    "docs/public/ja/reference/stdlib.md",
];

const AGENT_FACING_INSTRUCTION_DOCS: &[&str] = &["AGENTS.md", ".claude/agent-instructions.md"];

const PUBLIC_RANGE_SURFACE_DOCS: &[&str] = &[
    "LANGUAGE_SPECIFICATION.md",
    "RESTRICT_LANG_EBNF.md",
    "docs/public/en/guide/types.md",
    "docs/public/ja/guide/types.md",
    "docs/public/ja/guide/syntax.md",
];

const PUBLIC_STDLIB_SURFACE_DOCS: &[&str] = &[
    "LANGUAGE_SPECIFICATION.md",
    "README.md",
    "docs/public/en/getting-started/hello-world.md",
];

const PUBLIC_GLOBAL_EXPORT_DOCS: &[&str] = &[
    "docs/public/en/guide/syntax.md",
    "docs/public/en/guide/variables.md",
    "docs/public/en/reference/stdlib.md",
];

const WARDER_PUBLIC_DOCS: &[&str] = &[
    "docs/public/en/getting-started/quick-start.md",
    "docs/public/en/guide/warder.md",
    "docs/public/ja/getting-started/quick-start.md",
    "docs/public/ja/guide/warder.md",
];

const SUPPORTED_WARDER_SUBCOMMANDS: &[&str] = &[
    "new", "init", "add", "remove", "build", "run", "test", "publish", "wrap", "unwrap", "doctor",
];

const KNOWN_EXPERIMENTAL_OR_STALE_EXAMPLES: &[&str] = &[
    "examples/simple_test.rl",
    "examples/spread_destructuring_demo.rl",
    "examples/std_lib.rl",
    "examples/field_access_patterns.rl",
    "examples/tat_cleanup_demo.rl",
    "examples/temporal_file_io.rl",
    "examples/temporal_simple.rl",
    "examples/test_context_temporal.rl",
];

struct RestrictBlock {
    start_line: usize,
    source: String,
}

struct TomlCodeExample {
    name: String,
    start_line: usize,
    source: String,
}

fn restrict_code_blocks(markdown: &str) -> Vec<RestrictBlock> {
    fenced_code_blocks(markdown, |info| info.starts_with("restrict"))
}

fn stdlib_current_reference_code_blocks(markdown: &str) -> Vec<RestrictBlock> {
    fenced_code_blocks(markdown, |info| matches!(info, "restrict" | "text"))
}

fn fenced_code_blocks(markdown: &str, include_info: impl Fn(&str) -> bool) -> Vec<RestrictBlock> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    let mut start_line = 0;
    let mut in_included_block = false;

    for (index, line) in markdown.lines().enumerate() {
        if let Some(info) = line.trim_start().strip_prefix("```") {
            if in_included_block {
                blocks.push(RestrictBlock {
                    start_line,
                    source: current.join("\n"),
                });
                in_included_block = false;
                continue;
            }

            if include_info(info.trim()) {
                in_included_block = true;
                start_line = index + 2;
                current.clear();
            }
            continue;
        }

        if in_included_block {
            current.push(line);
        }
    }

    blocks
}

#[test]
fn v001_core_guides_do_not_reintroduce_stale_syntax() {
    for path in CURRENT_CORE_DOCS {
        let markdown = read_fixture(path);
        for block in restrict_code_blocks(&markdown) {
            assert_no_removed_v001_syntax_patterns(
                &format!("{path}:{}", block.start_line),
                &block.source,
            );
            assert_no_removed_binding_pipe_or_record_initializer(
                &format!("{path}:{}", block.start_line),
                &block.source,
            );
            assert_no_record_shaped_builtin_variants(
                &format!("{path}:{}", block.start_line),
                &block.source,
            );
            assert_current_import_surface(&format!("{path}:{}", block.start_line), &block.source);
            assert_no_traditional_call_syntax(
                &format!("{path}:{}", block.start_line),
                &block.source,
            );
        }
    }
}

#[test]
fn stdlib_reference_code_blocks_do_not_reintroduce_stale_syntax() {
    let path = "docs/public/en/reference/stdlib.md";
    let markdown = read_fixture(path);

    for block in stdlib_current_reference_code_blocks(&markdown) {
        assert_no_removed_v001_syntax_patterns(
            &format!("{path}:{}", block.start_line),
            &block.source,
        );
        assert_no_removed_binding_pipe_or_record_initializer(
            &format!("{path}:{}", block.start_line),
            &block.source,
        );
        assert_no_record_shaped_builtin_variants(
            &format!("{path}:{}", block.start_line),
            &block.source,
        );
        assert_current_import_surface(&format!("{path}:{}", block.start_line), &block.source);
    }
}

#[test]
fn public_range_surface_docs_use_int32_only() {
    for path in PUBLIC_RANGE_SURFACE_DOCS {
        let markdown = read_fixture(path);
        assert!(
            !markdown.contains("Range<T>"),
            "{path} should document v0.0.1 ranges as Range<Int32>, not as a generic range type"
        );
        assert!(
            markdown.contains("Range<Int32>"),
            "{path} should explicitly document the v0.0.1 Range<Int32> surface"
        );
    }
}

#[test]
fn public_docs_do_not_advertise_unsupported_string_helpers() {
    for path in PUBLIC_STDLIB_SURFACE_DOCS {
        let markdown = read_fixture(path);

        for helper in ["to_uppercase", "reverse", "concat"] {
            assert!(
                !contains_word(&markdown, helper),
                "{path} should not advertise unsupported std helper `{helper}` as current v0.0.1 syntax"
            );
        }
    }
}

#[test]
fn public_docs_do_not_advertise_exported_string_globals() {
    for path in PUBLIC_GLOBAL_EXPORT_DOCS {
        let markdown = read_fixture(path);
        let normalized = normalize_markdown_whitespace(&markdown);

        for stale in [
            "pub val release_label: String",
            "`Char`, `String`",
            "`String`, and `()`",
        ] {
            assert!(
                !markdown.contains(stale),
                "{path} should not advertise exported String globals as v0.0.1 scalar ABI surface"
            );
        }
        assert!(
            normalized.contains("`Int32`, `Int64`, `Float64`, `Boolean`, `Char`, and `()`"),
            "{path} should list the scalar top-level export ABI without String"
        );
    }
}

#[test]
fn public_impl_docs_match_current_grouped_osv_surface() {
    let spec = read_fixture("LANGUAGE_SPECIFICATION.md");
    let readme = read_fixture("README.md");

    assert!(
        spec.contains("(receiver) method") && spec.contains("(receiver, args...) method"),
        "LANGUAGE_SPECIFICATION.md should document impl dispatch as grouped OSV calls"
    );
    assert!(
        spec.contains("must be `self: Target`"),
        "LANGUAGE_SPECIFICATION.md should make the impl receiver contract normative"
    );
    assert!(
        !spec.contains("receiver |> method"),
        "LANGUAGE_SPECIFICATION.md should not advertise pipe dispatch for impl methods"
    );
    assert!(
        readme.contains("(bob) is_adult") && !readme.contains("|> is_adult"),
        "README.md should show impl method calls through grouped OSV, not pipe dispatch"
    );
}

#[test]
fn ebnf_documents_impl_as_reachable_top_level_syntax() {
    let ebnf = read_fixture("RESTRICT_LANG_EBNF.md");

    assert!(
        ebnf.contains("\"impl\""),
        "RESTRICT_LANG_EBNF.md should include impl in the keyword list"
    );
    assert!(
        ebnf.contains("| impl_decl"),
        "RESTRICT_LANG_EBNF.md should make impl_decl reachable from top_decl"
    );
    assert!(
        ebnf.contains("impl_decl") && ebnf.contains("\"impl\" identifier"),
        "RESTRICT_LANG_EBNF.md should define impl block grammar"
    );
}

#[test]
fn ebnf_marks_user_defined_enum_as_reserved_not_reachable() {
    let ebnf = read_fixture("RESTRICT_LANG_EBNF.md");
    let top_decl = ebnf
        .split("top_decl")
        .nth(1)
        .and_then(|after| after.split("(* Function Declaration *)").next())
        .expect("RESTRICT_LANG_EBNF.md should contain top_decl before function declarations");

    assert!(
        !top_decl.contains("| enum_decl"),
        "RESTRICT_LANG_EBNF.md should not expose user-defined enum declarations as current top_decl syntax"
    );
    assert!(
        ebnf.contains("reserved: enum_decl")
            && ebnf.contains("Reserved Enum Declaration: post-v0.0.1"),
        "RESTRICT_LANG_EBNF.md should keep enum_decl visible as reserved post-v0.0.1 syntax"
    );
}

#[test]
fn type_inference_docs_keep_post_v001_features_outside_default_gate() {
    let markdown = read_fixture("docs/public/en/guide/type-inference.md");

    for required_claim in [
        "User-defined `enum`/ADT syntax",
        "Temporal Affine Types (TAT) remain outside the default v0.0.1 gate",
        "User-defined `form`, `takes`, `of`, and associated-type declarations are\nfuture design work, not current source syntax",
    ] {
        assert!(
            markdown.contains(required_claim),
            "docs/public/en/guide/type-inference.md should keep `{required_claim}` clearly outside default v0.0.1"
        );
    }
}

#[test]
fn readme_places_generic_export_boundary_at_release_surface_validation() {
    let readme = read_fixture("README.md");

    assert!(
        !readme.contains("rejected by codegen. Exported records"),
        "README.md should not describe exported generic functions as reaching codegen before rejection"
    );
    assert!(
        readme.contains(
            "rejected by v0.0.1 release-surface validation before `--check` success or\n  code generation"
        ),
        "README.md should say exported generic/composite ABI gaps are rejected by release-surface validation before codegen"
    );
}

#[test]
fn warder_public_docs_do_not_advertise_unsupported_v001_surface() {
    for path in WARDER_PUBLIC_DOCS {
        let markdown = read_fixture(path);
        assert_no_unsupported_warder_public_doc_claims(path, &markdown);
    }
}

#[test]
fn english_quick_start_restrict_blocks_do_not_advertise_host_network_io() {
    let path = "docs/public/en/getting-started/quick-start.md";
    let markdown = read_fixture(path);

    for block in restrict_code_blocks(&markdown) {
        let label = format!("{path}:{}", block.start_line);
        assert_no_host_network_io_symbols(&label, &block.source);
    }
}

#[test]
fn agent_instruction_restrict_examples_do_not_reintroduce_stale_syntax() {
    for path in AGENT_FACING_INSTRUCTION_DOCS {
        let markdown = read_fixture(path);
        for block in restrict_code_blocks(&markdown) {
            let label = format!("{path}:{}", block.start_line);
            assert_no_removed_v001_syntax_patterns(&label, &block.source);
            assert_no_removed_binding_pipe_or_record_initializer(&label, &block.source);
            assert_no_traditional_call_syntax(&label, &block.source);
        }
    }
}

fn assert_no_unsupported_warder_public_doc_claims(label: &str, markdown: &str) {
    let failures = unsupported_warder_public_doc_claims(markdown);

    assert!(
        failures.is_empty(),
        "{label} advertises unsupported Warder v0.0.1 surface:\n{}",
        failures.join("\n")
    );
}

fn unsupported_warder_public_doc_claims(markdown: &str) -> Vec<String> {
    let mut failures = Vec::new();

    for (line_index, line) in markdown.lines().enumerate() {
        let line_number = line_index + 1;
        let negates_unsupported_claim = line_negates_warder_command_claim(line);

        for invocation in warder_invocations(line) {
            let Some(subcommand) = invocation.tokens.first() else {
                continue;
            };

            if is_supported_warder_global_flag(subcommand) {
                continue;
            }

            if negates_unsupported_claim {
                continue;
            }

            if !SUPPORTED_WARDER_SUBCOMMANDS.contains(&subcommand.as_str()) {
                failures.push(format!(
                    "line {line_number}: unsupported Warder subcommand `{subcommand}` in `{}`; supported subcommands are {}",
                    line.trim(),
                    SUPPORTED_WARDER_SUBCOMMANDS.join(", ")
                ));
                continue;
            }

            if subcommand == "build"
                && invocation
                    .tokens
                    .iter()
                    .any(|token| token == "--target" || token.starts_with("--target="))
            {
                failures.push(format!(
                    "line {line_number}: `warder build --target` is not implemented by warder/src/main.rs: {}",
                    line.trim()
                ));
            }
        }

        if line_claims_target_warder_artifact(line) {
            failures.push(format!(
                "line {line_number}: Warder public docs should not describe build/package artifacts under `target/...`: {}",
                line.trim()
            ));
        }
    }

    failures
}

struct WarderInvocation {
    tokens: Vec<String>,
}

fn warder_invocations(line: &str) -> Vec<WarderInvocation> {
    let mut invocations = Vec::new();

    for (index, _) in line.match_indices("warder") {
        if !is_standalone_word_at(line, index, "warder") || !is_warder_command_context(line, index)
        {
            continue;
        }
        if matches!(
            line[index + "warder".len()..].chars().next(),
            Some('/' | '.')
        ) {
            continue;
        }

        invocations.push(WarderInvocation {
            tokens: shellish_tokens_after(line, index + "warder".len()),
        });
    }

    invocations
}

fn shellish_tokens_after(line: &str, start: usize) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cursor = start;

    while cursor < line.len() {
        cursor = skip_shellish_separators(line, cursor);
        if cursor >= line.len() {
            break;
        }

        let bytes = line.as_bytes();
        if bytes[cursor] == b'#' || bytes[cursor] == b'`' {
            break;
        }

        let Some(token) = shellish_token_at(line, cursor) else {
            break;
        };

        cursor += token.len();
        tokens.push(token.to_string());
    }

    tokens
}

fn shellish_token_at(line: &str, start: usize) -> Option<&str> {
    let mut end = start;

    for (offset, char) in line[start..].char_indices() {
        if offset == 0 {
            if !(is_identifier_start(char) || char == '-' || char == '<') {
                return None;
            }
        } else if !(is_identifier_continue(char)
            || matches!(char, '-' | '.' | '/' | ':' | '<' | '>'))
        {
            break;
        }

        end = start + offset + char.len_utf8();
    }

    Some(&line[start..end])
}

fn skip_shellish_separators(line: &str, mut cursor: usize) -> usize {
    while cursor < line.len() {
        let char = line[cursor..]
            .chars()
            .next()
            .expect("cursor should be at a char boundary");

        if char.is_ascii_whitespace()
            || matches!(char, '"' | '\'' | ',' | '[' | ']' | '(' | ')' | '|')
        {
            cursor += char.len_utf8();
            continue;
        }

        break;
    }

    cursor
}

fn is_warder_command_context(line: &str, index: usize) -> bool {
    line[..index].chars().next_back().is_none_or(|char| {
        char.is_ascii_whitespace() || matches!(char, '`' | '"' | '\'' | '(' | '[' | '{' | '|')
    })
}

fn is_standalone_word_at(value: &str, index: usize, word: &str) -> bool {
    let before = value[..index].chars().next_back();
    let after = value[index + word.len()..].chars().next();

    before.is_none_or(|char| !is_identifier_continue(char))
        && after.is_none_or(|char| !is_identifier_continue(char))
}

fn is_supported_warder_global_flag(token: &str) -> bool {
    matches!(token, "--version" | "-V" | "--help" | "-h")
}

fn line_negates_warder_command_claim(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();

    lower.contains("there is no")
        || lower.contains("no separate")
        || lower.contains("not implemented")
        || lower.contains("not supported")
        || lower.contains("unsupported")
        || line.contains("ありません")
        || line.contains("存在しません")
        || line.contains("未実装")
        || line.contains("サポートされていません")
}

fn line_claims_target_warder_artifact(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();

    lower.contains("target/")
        && (lower.contains("output:")
            || lower.contains("artifact")
            || lower.contains(".rgc")
            || line.contains("出力")
            || line.contains("成果物"))
}

fn assert_no_host_network_io_symbols(label: &str, source: &str) {
    let code_only = source
        .lines()
        .map(strip_line_comment)
        .collect::<Vec<_>>()
        .join("\n");

    for module in ["host.net", "host.io"] {
        assert!(
            !code_only.contains(module),
            "{label} advertises host networking/I/O module `{module}` in the quick-start runnable path:\n{source}"
        );
    }

    for symbol in [
        "TcpListener",
        "TcpStream",
        "bind",
        "accept",
        "read",
        "write",
    ] {
        assert!(
            !contains_word(&code_only, symbol),
            "{label} advertises host networking/I/O symbol `{symbol}` in the quick-start runnable path:\n{source}"
        );
    }
}

fn assert_no_removed_v001_syntax_patterns(label: &str, source: &str) {
    let code_only = source
        .lines()
        .map(strip_line_comment)
        .collect::<Vec<_>>()
        .join("\n");
    let stale_substrings = [
        ("rust fn", "fn "),
        ("struct declaration", "struct "),
        ("if expression", "if "),
        ("for loop", "for "),
        ("use import", "use "),
        ("unsupported test declaration", "test \""),
        ("lowercase i32", "i32"),
        ("lowercase f64", "f64"),
        ("lowercase bool", "bool"),
        ("user enum declaration", "enum "),
        ("bracket-bar array literal", "[|"),
    ];

    for (description, pattern) in stale_substrings {
        assert!(
            !code_only.contains(pattern),
            "{label} has stale {description} syntax:\n{source}"
        );
    }

    for (description, word) in [
        ("legacy Unit spelling", "Unit"),
        ("legacy Bool spelling", "Bool"),
        ("legacy Int spelling", "Int"),
        ("legacy Float spelling", "Float"),
    ] {
        assert!(
            !contains_word(&code_only, word),
            "{label} has stale {description} syntax:\n{source}"
        );
    }
}

#[test]
fn docs_en_restrict_blocks_do_not_use_removed_binding_pipe_or_record_initializers() {
    let root = workspace_root().join("docs/public/en");

    for path in files_with_extension(&root, "md") {
        let relative_path = relative_workspace_path(&path);

        let markdown = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("{} should be readable: {err}", path.display()));
        for block in restrict_code_blocks(&markdown) {
            assert_no_removed_binding_pipe_or_record_initializer(
                &format!("{relative_path}:{}", block.start_line),
                &block.source,
            );
            assert_no_record_shaped_builtin_variants(
                &format!("{relative_path}:{}", block.start_line),
                &block.source,
            );
            assert_current_import_surface(
                &format!("{relative_path}:{}", block.start_line),
                &block.source,
            );
        }
    }
}

#[test]
fn current_japanese_docs_do_not_reintroduce_stale_syntax() {
    for path in CURRENT_JAPANESE_DOCS {
        let markdown = read_fixture(path);
        for block in restrict_code_blocks(&markdown) {
            let label = format!("{path}:{}", block.start_line);
            assert_no_removed_v001_syntax_patterns(&label, &block.source);
            assert_no_removed_binding_pipe_or_record_initializer(&label, &block.source);
            assert_no_record_shaped_builtin_variants(&label, &block.source);
            assert_no_traditional_call_syntax(&label, &block.source);
            assert_current_import_surface(&label, &block.source);
        }
    }
}

#[test]
fn public_doc_snippets_do_not_reintroduce_stale_syntax() {
    let markdown_dirs = ["docs/public/includes"];
    let source_dirs = ["docs/code-examples"];

    for relative_dir in markdown_dirs {
        let root = workspace_root().join(relative_dir);
        for path in files_with_extension(&root, "md") {
            let relative_path = relative_workspace_path(&path);
            let markdown = fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("{} should be readable: {err}", path.display()));

            for block in restrict_code_blocks(&markdown) {
                let label = format!("{relative_path}:{}", block.start_line);
                assert_no_removed_v001_syntax_patterns(&label, &block.source);
                assert_no_removed_binding_pipe_or_record_initializer(&label, &block.source);
                assert_no_record_shaped_builtin_variants(&label, &block.source);
                assert_no_traditional_call_syntax(&label, &block.source);
                assert_current_import_surface(&label, &block.source);
            }
        }
    }

    for relative_dir in source_dirs {
        let root = workspace_root().join(relative_dir);
        for path in files_with_extension(&root, "rl") {
            let relative_path = relative_workspace_path(&path);
            let source = fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("{} should be readable: {err}", path.display()));

            assert_no_removed_v001_syntax_patterns(&relative_path, &source);
            assert_no_removed_binding_pipe_or_record_initializer(&relative_path, &source);
            assert_no_record_shaped_builtin_variants(&relative_path, &source);
            assert_no_traditional_call_syntax(&relative_path, &source);
            assert_current_import_surface(&relative_path, &source);
        }
    }
}

#[test]
fn docs_code_examples_compile_to_valid_wat() {
    let root = workspace_root().join("docs/code-examples");

    for path in files_with_extension(&root, "rl") {
        let relative_path = relative_workspace_path(&path);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("{} should be readable: {err}", path.display()));
        let wat = compile_doc_example_to_wat(&relative_path, &source);

        wat::parse_str(&wat)
            .unwrap_or_else(|err| panic!("{relative_path} generated invalid WAT: {err}\n\n{wat}"));
    }
}

#[test]
fn code_examples_toml_restrict_snippets_do_not_reintroduce_stale_syntax() {
    let path = "docs/code-examples.toml";

    for example in restrict_code_examples_toml(path) {
        let label = format!("{path} [{}]:{}", example.name, example.start_line);
        assert_no_removed_v001_syntax_patterns(&label, &example.source);
        assert_no_removed_binding_pipe_or_record_initializer(&label, &example.source);
        assert_no_record_shaped_builtin_variants(&label, &example.source);
        assert_no_traditional_call_syntax(&label, &example.source);
        assert_current_import_surface(&label, &example.source);
        assert_no_unsupported_codegen_helper_names(&label, &example.source);
    }
}

#[test]
fn root_mdbook_summary_links_resolve_to_existing_pages() {
    assert_summary_links_resolve_to_existing_pages("docs/public/SUMMARY.md");
}

#[test]
fn public_mdbook_source_is_separate_from_internal_design_docs() {
    let book_config = read_fixture("docs/book.toml");
    assert!(
        book_config.contains(r#"src = "public""#),
        "docs/book.toml should build Pages docs from docs/public, not from internal docs/"
    );
    assert!(
        book_config.contains(r#"theme = "public/theme""#),
        "docs/book.toml should load the public mdBook theme from docs/public/theme"
    );
    assert!(
        !workspace_root().join("docs/theme").exists(),
        "legacy docs/theme should not duplicate the public theme outside docs/public"
    );

    let public_root = workspace_root().join("docs/public");
    let forbidden = files_with_extension(&public_root, "md")
        .into_iter()
        .filter_map(|path| {
            let relative = relative_workspace_path(&path);
            let filename = path.file_name()?.to_string_lossy();
            let uppercase_name = filename.to_ascii_uppercase();
            let is_internal_name = uppercase_name.contains("DESIGN")
                || uppercase_name.contains("IMPLEMENTATION")
                || uppercase_name.contains("ROADMAP")
                || uppercase_name.contains("STATUS")
                || uppercase_name.contains("THEORY");

            is_internal_name.then_some(relative)
        })
        .collect::<Vec<_>>();

    assert!(
        forbidden.is_empty(),
        "docs/public should not contain internal design/status documents:\n{}",
        forbidden.join("\n")
    );
}

#[test]
fn readme_local_markdown_doc_links_resolve() {
    let readme = read_fixture("README.md");
    let missing_links = local_markdown_doc_links_with_lines(&readme)
        .into_iter()
        .filter_map(|(line_number, link)| {
            let path = workspace_root().join(link.trim_start_matches("./").trim_start_matches('/'));
            (!path.is_file()).then(|| format!("line {line_number}: {link}"))
        })
        .collect::<Vec<_>>();

    assert!(
        missing_links.is_empty(),
        "README.md links to missing local docs:\n{}",
        missing_links.join("\n")
    );
}

fn assert_summary_links_resolve_to_existing_pages(summary_path: &str) {
    let summary = read_fixture(summary_path);
    let summary_dir = workspace_root()
        .join(summary_path)
        .parent()
        .expect("summary file should have a parent directory")
        .to_path_buf();

    for link in local_markdown_links(&summary) {
        let path = summary_dir.join(link.trim_start_matches("./"));
        assert!(
            path.is_file(),
            "{summary_path} links to a missing page: {link}"
        );
    }
}

#[test]
fn supported_examples_do_not_use_removed_binding_pipe_or_record_initializers() {
    for path in tracked_files_with_extension_under("examples", "rl") {
        let relative_path = relative_workspace_path(&path);
        if KNOWN_EXPERIMENTAL_OR_STALE_EXAMPLES.contains(&relative_path.as_str()) {
            continue;
        }

        let source = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("{} should be readable: {err}", path.display()));
        assert_no_removed_binding_pipe_or_record_initializer(&relative_path, &source);
        assert_no_record_shaped_builtin_variants(&relative_path, &source);
        assert_current_import_surface(&relative_path, &source);
    }
}

fn assert_no_removed_binding_pipe_or_record_initializer(label: &str, source: &str) {
    let failures = removed_binding_pipe_or_record_initializer_failures(source);

    assert!(
        failures.is_empty(),
        "{label} has stale syntax:\n{}",
        failures.join("\n")
    );
}

fn assert_no_traditional_call_syntax(label: &str, source: &str) {
    let failures = traditional_call_syntax_failures(source);

    assert!(
        failures.is_empty(),
        "{label} has traditional call syntax:\n{}",
        failures.join("\n")
    );
}

fn assert_no_record_shaped_builtin_variants(label: &str, source: &str) {
    let code_only = source
        .lines()
        .map(strip_line_comment)
        .collect::<Vec<_>>()
        .join("\n");

    for variant in ["Some", "None", "Ok", "Err"] {
        let pattern = format!("{variant} {{");
        assert!(
            !code_only.contains(&pattern),
            "{label} has stale record-shaped `{variant}` syntax; use `{variant}(...)` or `{variant}` as appropriate:\n{source}"
        );
    }
}

fn assert_current_import_surface(label: &str, source: &str) {
    let mut failures = Vec::new();

    for (line_index, line) in source.lines().enumerate() {
        let code = strip_line_comment(line).trim_start();
        if code.starts_with("export import") {
            failures.push(format!(
                "line {}: re-exported imports are outside v0.0.1 source imports: {}",
                line_index + 1,
                line.trim()
            ));
        }

        if code.starts_with("import std.") || code.starts_with("export import std.") {
            failures.push(format!(
                "line {}: std aggregator imports are reserved outside the current source import examples: {}",
                line_index + 1,
                line.trim()
            ));
        }

        if !code.starts_with("import ") {
            continue;
        }

        if code.contains('"') {
            failures.push(format!(
                "line {}: string import path is outside v0.0.1 source imports: {}",
                line_index + 1,
                line.trim()
            ));
        }

        if code.contains(" as ") {
            failures.push(format!(
                "line {}: import alias is outside v0.0.1 source imports: {}",
                line_index + 1,
                line.trim()
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{label} has non-v0.0.1 import syntax:\n{}",
        failures.join("\n")
    );
}

fn assert_no_unsupported_codegen_helper_names(label: &str, source: &str) {
    let code_only = source
        .lines()
        .map(strip_line_comment)
        .collect::<Vec<_>>()
        .join("\n");

    for helper in ["split", "join", "capitalize"] {
        assert!(
            !contains_word(&code_only, helper),
            "{label} advertises unsupported helper `{helper}` as current v0.0.1 syntax:\n{source}"
        );
    }

    {
        let stale_type = "Vec";
        assert!(
            !contains_word(&code_only, stale_type),
            "{label} advertises stale type `{stale_type}` as current v0.0.1 syntax:\n{source}"
        );
    }

    assert!(
        !contains_word(&code_only, "fn"),
        "{label} advertises stale `fn` syntax; use `fun` declarations or Restrict function type syntax:\n{source}"
    );
}

fn removed_binding_pipe_or_record_initializer_failures(source: &str) -> Vec<String> {
    let mut failures = Vec::new();

    for (line_index, line) in source.lines().enumerate() {
        let code = strip_line_comment(line);

        if contains_word(code, "let") {
            failures.push(format!(
                "line {}: let binding: {}",
                line_index + 1,
                line.trim()
            ));
        }

        if code.contains("|>>") {
            failures.push(format!(
                "line {}: removed mutable pipe |>>: {}",
                line_index + 1,
                line.trim()
            ));
        }
    }

    failures.extend(
        stale_record_field_initializer_lines(source)
            .into_iter()
            .map(|(line, text)| format!("line {line}: record field initializer uses =: {text}")),
    );

    failures
}

fn restrict_code_examples_toml(path: &str) -> Vec<TomlCodeExample> {
    let source = read_fixture(path);
    let mut examples = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_code: Option<(usize, String)> = None;
    let mut current_language: Option<String> = None;
    let mut in_code = false;
    let mut code_start_line = 0;
    let mut code_lines = Vec::new();

    for (line_index, line) in source.lines().enumerate() {
        let line_number = line_index + 1;

        if in_code {
            if line.trim() == "'''" {
                current_code = Some((code_start_line, code_lines.join("\n")));
                code_lines.clear();
                in_code = false;
            } else {
                code_lines.push(line);
            }
            continue;
        }

        let trimmed = line.trim();

        if let Some(name) = toml_table_name(trimmed) {
            push_restrict_toml_example(
                &mut examples,
                current_name.take(),
                current_code.take(),
                current_language.take(),
            );
            current_name = Some(name.to_string());
            continue;
        }

        if trimmed == "code = '''" {
            in_code = true;
            code_start_line = line_number + 1;
            code_lines.clear();
            continue;
        }

        if let Some(language) = toml_quoted_value(trimmed, "language") {
            current_language = Some(language.to_string());
        }
    }

    push_restrict_toml_example(&mut examples, current_name, current_code, current_language);

    assert!(
        !in_code,
        "{path} has an unterminated TOML multiline code string"
    );
    assert!(
        !examples.is_empty(),
        "{path} should contain at least one restrict code example"
    );

    examples
}

fn push_restrict_toml_example(
    examples: &mut Vec<TomlCodeExample>,
    name: Option<String>,
    code: Option<(usize, String)>,
    language: Option<String>,
) {
    let Some(name) = name else {
        return;
    };

    if language.as_deref() != Some("restrict") {
        return;
    }

    let Some((start_line, source)) = code else {
        panic!("docs/code-examples.toml [{name}] has language restrict but no code block");
    };

    examples.push(TomlCodeExample {
        name,
        start_line,
        source,
    });
}

fn toml_table_name(trimmed_line: &str) -> Option<&str> {
    trimmed_line
        .strip_prefix('[')
        .and_then(|line| line.strip_suffix(']'))
        .filter(|name| !name.starts_with('[') && !name.is_empty())
}

fn toml_quoted_value<'a>(trimmed_line: &'a str, key: &str) -> Option<&'a str> {
    let value = trimmed_line.strip_prefix(key)?.trim_start();
    let value = value.strip_prefix('=')?.trim_start();
    value.strip_prefix('"')?.strip_suffix('"')
}

fn traditional_call_syntax_failures(source: &str) -> Vec<String> {
    let mut failures = Vec::new();

    for (line_index, line) in source.lines().enumerate() {
        let code = strip_line_comment(line);

        if let Some(call) = object_style_method_call(code) {
            failures.push(format!(
                "line {}: object-style method call: {}",
                line_index + 1,
                call
            ));
        }

        if let Some(call) = function_first_call(code) {
            failures.push(format!(
                "line {}: function-first call: {}",
                line_index + 1,
                call
            ));
        }
    }

    failures
}

fn object_style_method_call(line: &str) -> Option<String> {
    let bytes = line.as_bytes();

    for (dot_index, char) in line.char_indices() {
        if char != '.' {
            continue;
        }

        let mut name_start = dot_index + 1;
        while name_start < bytes.len() && bytes[name_start].is_ascii_whitespace() {
            name_start += 1;
        }

        let Some(name) = identifier_at(line, name_start) else {
            continue;
        };
        let after_name = name_start + name.len();
        let mut after_space = after_name;
        while after_space < bytes.len() && bytes[after_space].is_ascii_whitespace() {
            after_space += 1;
        }

        if bytes.get(after_space) == Some(&b'(') {
            return Some(line.trim().to_string());
        }
    }

    None
}

fn function_first_call(line: &str) -> Option<String> {
    let mut search_from = 0;

    while search_from < line.len() {
        let Some((name_start, name)) = next_identifier(line, search_from) else {
            break;
        };

        let after_name = name_start + name.len();
        let bytes = line.as_bytes();
        let mut after_space = after_name;
        while after_space < bytes.len() && bytes[after_space].is_ascii_whitespace() {
            after_space += 1;
        }

        if bytes.get(after_space) == Some(&b'(')
            && starts_function_first_call(line, name_start, name)
        {
            return Some(line.trim().to_string());
        }

        search_from = after_name;
    }

    None
}

fn starts_function_first_call(line: &str, name_start: usize, name: &str) -> bool {
    if matches!(
        name,
        "Some"
            | "Ok"
            | "Err"
            | "fun"
            | "val"
            | "mut"
            | "record"
            | "impl"
            | "context"
            | "match"
            | "then"
            | "else"
            | "with"
            | "pub"
    ) {
        return false;
    }

    if name
        .chars()
        .next()
        .is_some_and(|char| char.is_ascii_uppercase())
    {
        return false;
    }

    line[..name_start]
        .chars()
        .next_back()
        .is_none_or(|char| !matches!(char, '|' | '<' | '\'' | '"'))
}

fn stale_record_field_initializer_lines(source: &str) -> Vec<(usize, String)> {
    let mut failures = Vec::new();
    let mut record_context_stack = Vec::new();

    for (line_index, line) in source.lines().enumerate() {
        let code = strip_line_comment(line);
        let mut reported = false;

        if contains_inline_record_field_assignment(code) {
            failures.push((line_index + 1, line.trim().to_string()));
            reported = true;
        }

        if !reported
            && record_context_stack.last().copied().unwrap_or(false)
            && starts_with_field_assignment(code)
        {
            failures.push((line_index + 1, line.trim().to_string()));
        }

        update_record_context_stack(code, &mut record_context_stack);
    }

    failures
}

fn contains_inline_record_field_assignment(line: &str) -> bool {
    let mut search_from = 0;

    while let Some(open_offset) = line[search_from..].find('{') {
        let open = search_from + open_offset;
        if opens_record_context(&line[..open]) {
            if let Some(close_offset) = line[open + 1..].find('}') {
                let close = open + 1 + close_offset;
                if line[open + 1..close]
                    .split(',')
                    .any(starts_with_field_assignment)
                {
                    return true;
                }
            }
        }

        search_from = open + 1;
    }

    false
}

fn update_record_context_stack(line: &str, stack: &mut Vec<bool>) {
    for (index, char) in line.char_indices() {
        match char {
            '{' => stack.push(opens_record_context(&line[..index])),
            '}' => {
                stack.pop();
            }
            _ => {}
        }
    }
}

fn opens_record_context(prefix: &str) -> bool {
    let trimmed = prefix.trim_end();

    if trimmed.ends_with("=>")
        || trimmed.ends_with("then")
        || trimmed.ends_with("else")
        || trimmed.ends_with("match")
        || trimmed.starts_with("fun ")
        || trimmed.starts_with("record ")
        || trimmed.starts_with("context ")
        || trimmed.starts_with("test ")
    {
        return false;
    }

    if trimmed.ends_with(".clone") || trimmed.ends_with(" clone") {
        return true;
    }

    if trimmed.starts_with("with ") {
        return true;
    }

    if trimmed.starts_with("val ") || trimmed.starts_with("mut val ") {
        return trimmed.ends_with('=');
    }

    last_identifier(trimmed)
        .and_then(|identifier| identifier.chars().next())
        .is_some_and(|first| first.is_ascii_uppercase())
}

fn starts_with_field_assignment(fragment: &str) -> bool {
    let trimmed = fragment.trim_start();
    let mut chars = trimmed.char_indices();
    let Some((_, first)) = chars.next() else {
        return false;
    };

    if !is_identifier_start(first) {
        return false;
    }

    let mut identifier_end = first.len_utf8();
    for (index, char) in chars {
        if is_identifier_continue(char) {
            identifier_end = index + char.len_utf8();
        } else {
            break;
        }
    }

    let rest = trimmed[identifier_end..].trim_start();
    rest.starts_with('=') && !rest.starts_with("==") && !rest.starts_with("=>")
}

fn last_identifier(value: &str) -> Option<&str> {
    value
        .split(|char: char| !is_identifier_continue(char))
        .rfind(|part| !part.is_empty())
}

fn next_identifier(value: &str, search_from: usize) -> Option<(usize, &str)> {
    for (offset, char) in value[search_from..].char_indices() {
        if is_identifier_start(char) {
            let start = search_from + offset;
            return identifier_at(value, start).map(|identifier| (start, identifier));
        }
    }

    None
}

fn identifier_at(value: &str, start: usize) -> Option<&str> {
    let mut chars = value[start..].char_indices();
    let (_, first) = chars.next()?;
    if !is_identifier_start(first) {
        return None;
    }

    let mut end = start + first.len_utf8();
    for (offset, char) in chars {
        if is_identifier_continue(char) {
            end = start + offset + char.len_utf8();
        } else {
            break;
        }
    }

    Some(&value[start..end])
}

fn contains_word(value: &str, word: &str) -> bool {
    value.match_indices(word).any(|(index, _)| {
        let before = value[..index].chars().next_back();
        let after = value[index + word.len()..].chars().next();

        before.is_none_or(|char| !is_identifier_continue(char))
            && after.is_none_or(|char| !is_identifier_continue(char))
    })
}

fn normalize_markdown_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_line_comment(line: &str) -> &str {
    line.split_once("//")
        .map(|(code, _comment)| code)
        .unwrap_or(line)
}

fn is_identifier_start(char: char) -> bool {
    char == '_' || char.is_ascii_alphabetic()
}

fn is_identifier_continue(char: char) -> bool {
    char == '_' || char.is_ascii_alphanumeric()
}

fn read_fixture(relative_path: &str) -> String {
    let path = workspace_root().join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{} should be readable: {err}", path.display()))
}

fn compile_doc_example_to_wat(label: &str, source: &str) -> String {
    let (remaining, program) = parse_program(source)
        .unwrap_or_else(|err| panic!("{label} should parse as Restrict source: {err:?}"));
    assert!(
        remaining.trim().is_empty(),
        "{label} should parse all input, remaining: {remaining:?}"
    );

    let mut type_checker = TypeChecker::new();
    type_checker
        .check_program(&program)
        .unwrap_or_else(|err| panic!("{label} should type-check: {err}"));

    let mut codegen = WasmCodeGen::new();
    codegen
        .generate(&program)
        .unwrap_or_else(|err| panic!("{label} should generate WAT: {err}"))
}

fn local_markdown_links(markdown: &str) -> Vec<&str> {
    let mut links = Vec::new();

    for line in markdown.lines() {
        let mut rest = line;
        while let Some(open) = rest.find("](") {
            let after_open = &rest[open + 2..];
            let Some(close) = after_open.find(')') else {
                break;
            };
            let link = &after_open[..close];
            if link.starts_with("./") {
                links.push(link);
            }
            rest = &after_open[close + 1..];
        }
    }

    links
}

fn local_markdown_doc_links_with_lines(markdown: &str) -> Vec<(usize, String)> {
    let mut links = Vec::new();

    for (line_index, line) in markdown.lines().enumerate() {
        let mut rest = line;
        while let Some(open) = rest.find("](") {
            let after_open = &rest[open + 2..];
            let Some(close) = after_open.find(')') else {
                break;
            };
            let raw_link = &after_open[..close];
            if let Some(link) = local_markdown_doc_path(raw_link) {
                links.push((line_index + 1, link));
            }
            rest = &after_open[close + 1..];
        }
    }

    links
}

fn local_markdown_doc_path(raw_link: &str) -> Option<String> {
    let link = raw_link.trim();
    if link.is_empty()
        || link.starts_with('#')
        || link.contains("://")
        || link.starts_with("mailto:")
        || link.starts_with("data:")
    {
        return None;
    }

    let path = link
        .split('#')
        .next()
        .expect("split always returns at least one segment")
        .split('?')
        .next()
        .expect("split always returns at least one segment")
        .trim();

    (Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        == Some("md"))
    .then(|| path.to_string())
}

fn files_with_extension(root: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files_with_extension(root, extension, &mut files);
    files.sort();
    files
}

fn tracked_files_with_extension_under(relative_dir: &str, extension: &str) -> Vec<PathBuf> {
    let prefix = format!("{}/", relative_dir.trim_end_matches('/'));
    let mut files: Vec<_> = tracked_workspace_files()
        .into_iter()
        .filter(|relative_path| relative_path.starts_with(&prefix))
        .filter(|relative_path| {
            Path::new(relative_path)
                .extension()
                .and_then(|value| value.to_str())
                == Some(extension)
        })
        .map(|relative_path| workspace_root().join(relative_path))
        .collect();
    files.sort();
    files
}

fn tracked_workspace_files() -> Vec<String> {
    let output = Command::new("git")
        .args(["ls-files", "--"])
        .current_dir(workspace_root())
        .output()
        .expect("git ls-files should run for docs hygiene checks");
    let stdout = String::from_utf8(output.stdout).expect("git ls-files output should be UTF-8");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "git ls-files should succeed for docs hygiene checks\nstderr:\n{stderr}"
    );

    stdout.lines().map(str::to_owned).collect()
}

fn collect_files_with_extension(path: &Path, extension: &str, files: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().and_then(|value| value.to_str()) == Some(extension) {
            files.push(path.to_path_buf());
        }
        return;
    }

    let entries = fs::read_dir(path)
        .unwrap_or_else(|err| panic!("{} should be readable: {err}", path.display()));
    for entry in entries {
        let entry = entry.expect("directory entry should be readable");
        collect_files_with_extension(&entry.path(), extension, files);
    }
}

fn relative_workspace_path(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}
