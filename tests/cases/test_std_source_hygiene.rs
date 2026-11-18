use std::fs;
use std::path::Path;

use restrict_lang::module::ModuleResolver;
use restrict_lang::{
    check_v001_release_surface, parse_program, TypeChecker, WasmCodeGen, WasmTargetProfile,
};

const SHIPPED_STD_MODULES: &[&str] = &[
    "lib", "prelude", "math", "option", "result", "list", "string", "io",
];

const PLANNED_CAPABILITY_PROFILES: &[&str] =
    &["std/database.rl", "std/filesystem.rl", "std/http.rl"];

const REFERENCE_ONLY_SOURCES: &[&str] = &["std/test_module.rl", "stdlib/src/IO.rl"];

#[test]
fn shipped_std_modules_compile_through_typecheck_release_surface_and_wasm_codegen() {
    for module_name in SHIPPED_STD_MODULES {
        let source_path = format!("std/{module_name}.rl");
        let module_source = read_workspace_file(&source_path);
        let (remaining, _) = parse_program(&module_source)
            .unwrap_or_else(|err| panic!("{source_path} should parse: {err:?}"));
        assert!(
            remaining.trim().is_empty(),
            "{source_path} should parse completely, remaining: {remaining:?}"
        );

        let driver =
            format!("import std.{module_name}.*\n\nexport fun main: () -> Int32 = {{\n    1\n}}\n");
        let (_, program) = parse_program(&driver)
            .unwrap_or_else(|err| panic!("driver for std.{module_name} should parse: {err:?}"));

        let mut resolver = ModuleResolver::new();
        let resolved = resolver
            .resolve_program_imports(program)
            .unwrap_or_else(|err| panic!("std.{module_name} should resolve: {err}"));

        let mut checker = TypeChecker::new();
        checker
            .check_program(&resolved)
            .unwrap_or_else(|err| panic!("std.{module_name} should typecheck: {err}"));

        check_v001_release_surface(&resolved, &checker)
            .unwrap_or_else(|err| panic!("std.{module_name} should pass release surface: {err}"));

        let profile = if *module_name == "io" {
            WasmTargetProfile::WasiP1
        } else {
            WasmTargetProfile::WasmCore
        };
        let mut codegen = WasmCodeGen::new().with_target_profile(profile);
        let wat = codegen
            .generate(&resolved)
            .unwrap_or_else(|err| panic!("std.{module_name} should generate WAT: {err}"));

        let wasm = wat::parse_str(&wat)
            .unwrap_or_else(|err| panic!("std.{module_name} WAT should parse: {err}\n{wat}"));
        wasmparser::Validator::new()
            .validate_all(&wasm)
            .unwrap_or_else(|err| panic!("std.{module_name} Wasm should validate: {err}\n{wat}"));
    }
}

#[test]
fn std_namespace_remains_reserved_and_unknown_or_planned_modules_fail_cleanly() {
    let mut resolver = ModuleResolver::new();
    let std_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("std");
    let root_err = resolver
        .add_package_root("std".to_string(), std_dir)
        .expect_err("--module-root std=... must remain rejected");
    assert!(
        root_err.to_string().contains("reserved"),
        "expected reserved namespace error, got: {root_err}"
    );

    for (module_name, expected_fragment) in [
        ("foo", "Unknown standard library module"),
        ("http", "planned capability profile"),
        ("database", "planned capability profile"),
        ("filesystem", "planned capability profile"),
    ] {
        let driver = format!("import std.{module_name}.*\nfun main: () -> Int32 = {{ 0 }}\n");
        let (_, program) = parse_program(&driver).expect("driver should parse");
        let mut r = ModuleResolver::new();
        let err = r
            .resolve_program_imports(program)
            .expect_err(&format!("import std.{module_name}.* should fail"));
        assert!(
            err.to_string().contains(expected_fragment),
            "import std.{module_name}.* error should contain {expected_fragment:?}, got: {err}"
        );
    }
}

#[test]
fn planned_capability_profiles_and_reference_indexes_stay_comment_only() {
    for path in PLANNED_CAPABILITY_PROFILES
        .iter()
        .chain(REFERENCE_ONLY_SOURCES.iter())
    {
        let source = read_workspace_file(path);
        let (remaining, _program) =
            parse_program(&source).unwrap_or_else(|err| panic!("{path} should parse: {err:?}"));
        assert!(
            remaining.trim().is_empty(),
            "{path} should parse completely, remaining: {remaining:?}"
        );

        for (line_number, line) in source.lines().enumerate() {
            assert!(
                line.trim().is_empty() || line.trim_start().starts_with("//"),
                "{path}:{} should stay a comment-only file",
                line_number + 1
            );
        }
    }

    for path in PLANNED_CAPABILITY_PROFILES {
        let source = read_workspace_file(path);
        assert!(
            source.contains("planned capability-profile"),
            "{path} should clearly document itself as a planned capability profile"
        );
    }

    for path in REFERENCE_ONLY_SOURCES {
        let source = read_workspace_file(path);
        assert!(
            source.contains("not the runtime implementation") && source.contains("compiler"),
            "{path} should point readers to the compiler-owned implementation"
        );
        assert_no_stale_release_words(path, &source);
    }
}

#[test]
fn std_sources_do_not_reintroduce_stale_syntax() {
    let all_sources = checked_in_reference_sources();
    for path in all_sources {
        let source = read_workspace_file(&path);
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
        for stale_kw in ["TODO", "FIXME", "let", "fn"] {
            assert!(
                !contains_word_case_insensitive(&source, stale_kw),
                "{path} should not contain stale keyword `{stale_kw}`"
            );
        }
    }
}

#[test]
fn legacy_stdlib_sources_do_not_advertise_non_v001_io() {
    for path in REFERENCE_ONLY_SOURCES {
        let source = read_workspace_file(path);

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
    let mut covered = SHIPPED_STD_MODULES
        .iter()
        .map(|name| format!("std/{name}.rl"))
        .collect::<Vec<_>>();
    covered.extend(PLANNED_CAPABILITY_PROFILES.iter().map(|p| (*p).to_string()));
    covered.extend(REFERENCE_ONLY_SOURCES.iter().map(|p| (*p).to_string()));

    let checked_in = checked_in_reference_sources();
    assert!(
        !checked_in.is_empty(),
        "std source hygiene should cover checked-in std/*.rl and stdlib/**/*.rl files"
    );

    for path in &checked_in {
        assert!(
            covered.contains(path),
            "{path} should be covered by std source hygiene"
        );
    }
    for path in &covered {
        assert!(
            checked_in.contains(path),
            "{path} is listed in std hygiene coverage but missing from workspace"
        );
    }
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
