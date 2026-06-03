use std::fs;
use std::path::Path;

use restrict_lang::parse_program;

#[test]
fn std_reference_sources_are_explicit_parseable_indexes() {
    for path in std_reference_sources() {
        let source = read_workspace_file(&path);

        assert!(
            source.contains("not the runtime implementation"),
            "{path} should say that runtime std behavior is compiler-registered"
        );
        assert!(
            source.contains("compiler"),
            "{path} should point readers to the compiler-owned implementation"
        );

        let (remaining, _program) =
            parse_program(&source).unwrap_or_else(|err| panic!("{path} should parse: {err:?}"));
        assert!(
            remaining.trim().is_empty(),
            "{path} should parse completely, remaining: {remaining:?}"
        );

        for (line_number, line) in source.lines().enumerate() {
            assert!(
                line.trim().is_empty() || line.trim_start().starts_with("//"),
                "{path}:{} should stay a comment-only reference index",
                line_number + 1
            );
        }
    }
}

#[test]
fn std_reference_sources_do_not_reintroduce_stale_syntax() {
    for path in std_reference_sources() {
        let source = read_workspace_file(&path);

        assert_no_stale_release_words(&path, &source);

        assert!(
            !source.contains("fun<"),
            "{path} should not use legacy generic function syntax"
        );
        assert!(
            !source.contains("|>>"),
            "{path} should not use removed mutable pipe syntax"
        );
        for io_name in compiler_registered_io_names() {
            assert!(
                !contains_function_first_call(&source, io_name),
                "{path} should not document function-first IO call `{io_name}(...)`"
            );
        }
        assert!(
            !source.contains("fun ") && !source.contains("fun<"),
            "{path} should stay a source-comment reference index"
        );
    }
}

#[test]
fn legacy_stdlib_sources_do_not_advertise_non_v001_io() {
    for path in std_reference_sources() {
        let source = read_workspace_file(&path);

        for phrase in ["read_line", "stdin", "standard input"] {
            assert!(
                !contains_case_insensitive(&source, phrase),
                "{path} should not advertise non-v0.0.1 IO surface `{phrase}`"
            );
        }
    }
}

#[test]
fn function_first_io_detection_covers_registered_io_names() {
    for io_name in compiler_registered_io_names() {
        assert!(
            contains_function_first_call(&format!("{io_name}(value)"), io_name),
            "direct function-first call should be detected for {io_name}"
        );
        assert!(
            contains_function_first_call(&format!("{io_name} (value)"), io_name),
            "whitespace-separated function-first call should be detected for {io_name}"
        );
        assert!(
            !contains_function_first_call(&format!("value |> {io_name}"), io_name),
            "OSV call should be allowed for {io_name}"
        );
    }

    assert!(
        !contains_function_first_call("print_int(value)", "print"),
        "shorter IO names should not match inside longer identifiers"
    );
}

#[test]
fn std_reference_docs_do_not_use_backlog_language() {
    for path in ["std/README.md", "docs/public/en/reference/stdlib.md"] {
        let source = read_workspace_file(path);
        assert_no_stale_release_words(path, &source);
    }
}

fn compiler_registered_io_names() -> &'static [&'static str] {
    &[
        "println",
        "print",
        "print_int",
        "print_float",
        "eprint",
        "eprintln",
    ]
}

#[test]
fn std_readme_uses_current_v001_call_and_import_surface() {
    let source = read_workspace_file("std/README.md");

    assert!(
        !source.contains("import std."),
        "std/README.md should not imply package-level std aggregators"
    );
    assert!(
        !source.contains(" as "),
        "std/README.md should not imply import aliases"
    );

    for inline_code in inline_code_spans(&source) {
        assert!(
            !starts_function_first_call(inline_code),
            "std/README.md inline code should use OSV call notation: `{inline_code}`"
        );
    }
}

#[test]
fn every_std_source_file_has_hygiene_coverage() {
    let covered = std_reference_sources();
    assert!(
        !covered.is_empty(),
        "std source hygiene should cover checked-in std/*.rl and stdlib/**/*.rl files"
    );

    for path in checked_in_reference_sources() {
        assert!(
            covered.contains(&path),
            "{path} should be covered by std source hygiene"
        );
    }
}

fn std_reference_sources() -> Vec<String> {
    checked_in_reference_sources()
}

fn checked_in_reference_sources() -> Vec<String> {
    let mut paths = checked_in_std_sources();
    paths.extend(checked_in_stdlib_sources());
    paths.sort();
    paths
}

fn checked_in_std_sources() -> Vec<String> {
    checked_in_sources_under("std")
}

fn checked_in_stdlib_sources() -> Vec<String> {
    checked_in_sources_under("stdlib")
}

fn checked_in_sources_under(relative_root: &str) -> Vec<String> {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths = Vec::new();
    collect_restrict_sources(workspace_root, Path::new(relative_root), &mut paths);
    paths.sort();
    paths
}

fn collect_restrict_sources(workspace_root: &Path, relative_dir: &Path, sources: &mut Vec<String>) {
    let absolute_dir = workspace_root.join(relative_dir);
    if !absolute_dir.exists() {
        return;
    }

    let mut entries = fs::read_dir(&absolute_dir)
        .unwrap_or_else(|err| panic!("{} should be readable: {err}", absolute_dir.display()))
        .map(|entry| entry.unwrap_or_else(|err| panic!("std entry should be readable: {err}")))
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let relative_path = relative_dir.join(entry.file_name());

        if path.is_dir() {
            collect_restrict_sources(workspace_root, &relative_path, sources);
        } else if path.extension().is_some_and(|extension| extension == "rl") {
            sources.push(relative_path.to_string_lossy().to_string());
        }
    }
}

fn read_workspace_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{} should be readable: {err}", path.display()))
}

fn assert_no_stale_release_words(path: &str, source: &str) {
    for word in [
        "TODO",
        "FIXME",
        "let",
        "fn",
        "Unit",
        "Bool",
        "Int",
        "Float",
        "future",
        "planned",
        "unimplemented",
        "placeholder",
        "stub",
    ] {
        assert!(
            !contains_word_case_insensitive(source, word),
            "{path} should not contain stale release word `{word}`"
        );
    }

    for phrase in [
        "not yet",
        "coming soon",
        "work in progress",
        "今後",
        "予定",
        "未実装",
        "未対応",
    ] {
        assert!(
            !contains_case_insensitive(source, phrase),
            "{path} should not contain backlog phrase `{phrase}`"
        );
    }
}

fn contains_word(value: &str, word: &str) -> bool {
    value.match_indices(word).any(|(index, _)| {
        let before = value[..index].chars().next_back();
        let after = value[index + word.len()..].chars().next();

        before.is_none_or(|char| !is_identifier_continue(char))
            && after.is_none_or(|char| !is_identifier_continue(char))
    })
}

fn contains_word_case_insensitive(value: &str, word: &str) -> bool {
    contains_word(&value.to_lowercase(), &word.to_lowercase())
}

fn contains_function_first_call(source: &str, function_name: &str) -> bool {
    source.match_indices(function_name).any(|(index, _)| {
        let before = source[..index].chars().next_back();
        if before.is_some_and(is_identifier_continue) {
            return false;
        }

        let after_name = &source[index + function_name.len()..];
        if after_name
            .chars()
            .next()
            .is_some_and(is_identifier_continue)
        {
            return false;
        }

        after_name.chars().find(|char| !char.is_whitespace()) == Some('(')
    })
}

fn contains_case_insensitive(value: &str, needle: &str) -> bool {
    value.to_lowercase().contains(&needle.to_lowercase())
}

fn inline_code_spans(markdown: &str) -> Vec<&str> {
    let mut spans = Vec::new();

    for line in markdown.lines() {
        let mut rest = line;
        while let Some(start) = rest.find('`') {
            let after_start = &rest[start + 1..];
            let Some(end) = after_start.find('`') else {
                break;
            };

            spans.push(&after_start[..end]);
            rest = &after_start[end + 1..];
        }
    }

    spans
}

fn starts_function_first_call(value: &str) -> bool {
    let Some(open_paren) = value.find('(') else {
        return false;
    };
    let name = value[..open_paren].trim();

    !name.is_empty()
        && name
            .chars()
            .all(|char| char == '_' || char.is_ascii_alphanumeric())
        && name
            .chars()
            .next()
            .is_some_and(|char| char.is_ascii_lowercase() || char == '_')
}

fn is_identifier_continue(char: char) -> bool {
    char == '_' || char.is_ascii_alphanumeric()
}
