use std::fs;
use std::process::{Command, Output};

use wasmi::{Caller, Engine, Instance, Linker, Module, Store};

fn instantiate_wat(label: &str, wat: &str) -> (Store<()>, Instance) {
    let wasm = wat::parse_str(wat).unwrap_or_else(|err| {
        panic!("{label} generated invalid WAT: {err}\n\n{wat}");
    });

    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("{label} generated WAT that is not valid wasm: {err}\n\n{wat}");
        });

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm[..]).unwrap_or_else(|err| {
        panic!("{label} generated wasm that wasmi cannot load: {err}\n\n{wat}");
    });
    let mut store = Store::new(&engine, ());
    let mut linker = Linker::new(&engine);

    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "fd_write",
            |_caller: Caller<'_, ()>,
             _fd: i32,
             _iovs: i32,
             _iovs_len: i32,
             _nwritten: i32|
             -> i32 { 0 },
        )
        .expect("fd_write stub should be registered");
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "proc_exit",
            |_caller: Caller<'_, ()>, _code: i32| {},
        )
        .expect("proc_exit stub should be registered");

    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .unwrap_or_else(|err| {
            panic!("{label} generated wasm that wasmi cannot instantiate: {err}\n\n{wat}");
        });

    (store, instance)
}

fn assert_instantiable_wat(label: &str, wat: &str) {
    let _ = instantiate_wat(label, wat);
}

fn assert_success_streams(label: &str, output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.trim().is_empty(),
        "{label} should not write stderr on success: {stderr}"
    );
    assert!(
        stdout.contains("Successfully compiled to"),
        "{label} should report the output path, stdout: {stdout}"
    );
    assert!(
        !stdout.contains("Tokens:") && !stdout.contains("AST:") && !stdout.contains("==="),
        "{label} should keep default success output concise, stdout: {stdout}"
    );
}

fn assert_check_success_streams(label: &str, output: &Output, source_path: &std::path::Path) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.trim().is_empty(),
        "{label} should not write stderr on --check success: {stderr}"
    );
    assert!(
        stdout.trim() == format!("OK {}", source_path.display()),
        "{label} should report concise --check success, stdout: {stdout}"
    );
}

fn run_check_temp_source(stem: &str, source: &str) -> (std::path::PathBuf, Output) {
    let source_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_{}_{}.rl",
        stem,
        std::process::id()
    ));
    fs::write(&source_path, source).expect("check source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--check")
        .arg(&source_path)
        .output()
        .expect("restrict_lang binary should run");

    (source_path, output)
}

fn run_compile_temp_source(
    stem: &str,
    source: &str,
) -> (std::path::PathBuf, std::path::PathBuf, Output) {
    let source_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_compile_{}_{}.rl",
        stem,
        std::process::id()
    ));
    let output_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_compile_{}_{}.wat",
        stem,
        std::process::id()
    ));
    let _ = fs::remove_file(&output_path);
    fs::write(&source_path, source).expect("compile source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg(&source_path)
        .arg(&output_path)
        .output()
        .expect("restrict_lang binary should run");

    (source_path, output_path, output)
}

fn assert_no_inference_internals(label: &str, stderr: &str) {
    for internal in [
        "?0",
        "InferVar",
        "TypeVarId",
        "Projection",
        "Feature not implemented",
    ] {
        assert!(
            !stderr.contains(internal),
            "{label} should not expose type inference internals ({internal}), got: {stderr}"
        );
    }
}

fn assert_no_parser_internals(label: &str, stderr: &str) {
    for internal in ["Error(", "ErrorKind", "nom::", "Tag", "Alt"] {
        assert!(
            !stderr.contains(internal),
            "{label} should not expose parser internals ({internal}), got: {stderr}"
        );
    }
}

#[test]
fn cli_help_reports_release_usage_on_stdout() {
    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--help")
        .output()
        .expect("restrict_lang binary should run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "--help should exit successfully, stderr: {stderr}"
    );
    assert!(
        stderr.trim().is_empty(),
        "--help should not write normal help text to stderr: {stderr}"
    );
    let expected_usage = format!(
        "Usage: {} [OPTIONS] <source_file> [output_file]",
        env!("CARGO_PKG_NAME")
    );
    assert_eq!(stdout.lines().next(), Some(expected_usage.as_str()));
    for option in [
        "--version",
        "--check",
        "--ast",
        "--verbose",
        "--lsp",
        "--help",
    ] {
        assert!(
            stdout.contains(option),
            "--help should list {option}, stdout: {stdout}"
        );
    }
    assert!(
        !stdout.contains("target/debug"),
        "--help should show stable release usage, stdout: {stdout}"
    );
}

#[test]
fn cli_version_reports_package_version_on_stdout() {
    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--version")
        .output()
        .expect("restrict_lang binary should run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "--version should exit successfully, stderr: {stderr}"
    );
    assert!(
        stderr.trim().is_empty(),
        "--version should not write to stderr: {stderr}"
    );
    assert_eq!(
        stdout.trim(),
        format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn cli_compiles_release_example_to_valid_wat() {
    let output_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_release_readiness_{}.wat",
        std::process::id()
    ));
    let _ = fs::remove_file(&output_path);

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("examples/release_readiness.rl")
        .arg(&output_path)
        .output()
        .expect("restrict_lang binary should run");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "release readiness example should compile through the CLI, stderr: {}",
        stderr
    );
    assert_success_streams("release readiness CLI", &output);

    let wat = fs::read_to_string(&output_path).expect("compiled WAT should be readable");
    assert!(
        wat.trim_start().starts_with("(module"),
        "CLI output should be WAT text, got: {}",
        wat
    );
    assert!(wat.contains("(func $assess_release"));
    assert!(wat.contains("call_indirect"));

    assert_instantiable_wat("release readiness CLI", &wat);

    let _ = fs::remove_file(output_path);
}

#[test]
fn cli_compiles_current_sample_syntax() {
    let source_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_sample_{}.rl",
        std::process::id()
    ));
    let output_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_sample_{}.wat",
        std::process::id()
    ));
    let _ = fs::remove_file(&output_path);

    fs::write(
        &source_path,
        r#"fun main: () -> () = {
    val message = "Hello, Restrict Language!"
    message |> println
}
"#,
    )
    .expect("sample source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg(&source_path)
        .arg(&output_path)
        .output()
        .expect("restrict_lang binary should run");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "current sample syntax should compile, stderr: {}",
        stderr
    );
    assert_success_streams("current sample CLI", &output);

    let wat = fs::read_to_string(&output_path).expect("compiled sample WAT should be readable");
    assert_instantiable_wat("current sample CLI", &wat);

    let _ = fs::remove_file(source_path);
    let _ = fs::remove_file(output_path);
}

#[test]
fn cli_check_reports_concise_success() {
    let source_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_check_success_{}.rl",
        std::process::id()
    ));

    fs::write(
        &source_path,
        r#"fun main: () -> Int32 = {
    42
}
"#,
    )
    .expect("check source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--check")
        .arg(&source_path)
        .output()
        .expect("restrict_lang binary should run");

    assert!(
        output.status.success(),
        "--check source should pass, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_check_success_streams("check CLI", &output, &source_path);

    let _ = fs::remove_file(source_path);
}

#[test]
fn cli_type_mismatch_uses_user_facing_type_names() {
    let source_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_type_mismatch_{}.rl",
        std::process::id()
    ));

    fs::write(
        &source_path,
        r#"fun main: () -> List<Int32> = {
    1
}
"#,
    )
    .expect("type mismatch source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--check")
        .arg(&source_path)
        .output()
        .expect("restrict_lang binary should run");

    assert!(
        !output.status.success(),
        "type mismatch CLI check should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Type mismatch: expected List<Int32>, found Int32"),
        "stderr should use source-facing type names, got: {stderr}"
    );
    for internal in ["List(", "Record {", "InferVar", "Projection", "TypeVarId"] {
        assert!(
            !stderr.contains(internal),
            "stderr should not expose internal type formatting ({internal}), got: {stderr}"
        );
    }

    let _ = fs::remove_file(source_path);
}

#[test]
fn cli_parse_error_hides_nom_debug_details() {
    let source_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_parse_error_{}.rl",
        std::process::id()
    ));

    fs::write(
        &source_path,
        r#"fun main: () -> Int32 = {
    val answer =
}
"#,
    )
    .expect("parse error source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--check")
        .arg(&source_path)
        .output()
        .expect("restrict_lang binary should run");

    assert!(!output.status.success(), "parse error check should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Parsing error at line"),
        "stderr should include a concise parse location, got: {stderr}"
    );
    for internal in ["Error(", "ErrorKind", "nom::", "Tag", "Alt"] {
        assert!(
            !stderr.contains(internal),
            "stderr should not expose parser internals ({internal}), got: {stderr}"
        );
    }

    let _ = fs::remove_file(source_path);
}

#[test]
fn cli_check_reports_actionable_stale_syntax_diagnostics() {
    let cases = [
        (
            "let_binding",
            r#"fun main: () -> Int32 = {
    let count = 1
    count
}
"#,
            ["`let`", "`val`", "immutable bindings"],
        ),
        (
            "if_parens",
            r#"fun main: () -> Int32 = {
    if (true) { 1 } else { 0 }
}
"#,
            ["`if (...)`", "`then`", "condition-first"],
        ),
        (
            "val_mut",
            r#"fun main: () -> Int32 = {
    val mut count = 0
    count
}
"#,
            ["`val mut`", "`mut val`", "mutable bindings"],
        ),
        (
            "traditional_call",
            r#"fun add: (x: Int32, y: Int32) -> Int32 = {
    x + y
}

fun main: () -> Int32 = {
    add(1, 2)
}
"#,
            ["add(1, 2)", "OSV", "(1, 2) add"],
        ),
        (
            "legacy_int_type",
            r#"fun main: () -> Int = {
    1
}
"#,
            ["`Int`", "`Int32`", "Unknown type"],
        ),
        (
            "legacy_float_type",
            r#"fun main: () -> Float = {
    1.0
}
"#,
            ["`Float`", "`Float64`", "Unknown type"],
        ),
        (
            "legacy_bool_type",
            r#"fun main: () -> Bool = {
    true
}
"#,
            ["`Bool`", "`Boolean`", "Unknown type"],
        ),
        (
            "unit_type_name",
            r#"fun main: () -> Unit = {
    ()
}
"#,
            ["`Unit`", "`()`", "unit type"],
        ),
        (
            "unit_value_name",
            r#"fun main: () -> () = {
    Unit
}
"#,
            ["`Unit`", "`()`", "unit value"],
        ),
        (
            "none_type_argument",
            r#"fun main: () -> Option<Int32> = {
    None<Int32>
}
"#,
            ["`None<T>`", "`None`", "Option<T>"],
        ),
        (
            "lowercase_some",
            r#"fun main: () -> Option<Int32> = {
    42 |> some
}
"#,
            ["lowercase `some`", "Option constructor", "Some(value)"],
        ),
        (
            "lowercase_some_direct_call",
            r#"fun main: () -> Option<Int32> = {
    42 some
}
"#,
            ["lowercase `some`", "Option constructor", "Some(value)"],
        ),
        (
            "lowercase_none",
            r#"fun main: () -> Option<Int32> = {
    () none
}
"#,
            ["lowercase `none`", "Option constructor", "`None`"],
        ),
        (
            "lowercase_none_bare",
            r#"fun main: () -> Option<Int32> = {
    none
}
"#,
            ["lowercase `none`", "Option constructor", "`None`"],
        ),
    ];

    for (stem, source, expected_parts) in cases {
        let (source_path, output) = run_check_temp_source(stem, source);

        assert!(
            !output.status.success(),
            "{stem} stale syntax check should fail"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        for expected in expected_parts {
            assert!(
                stderr.contains(expected),
                "{stem} diagnostic should mention {expected}, got: {stderr}"
            );
        }
        assert_no_parser_internals(stem, &stderr);

        let _ = fs::remove_file(source_path);
    }
}

#[test]
fn cli_inference_error_preserves_binding_context_without_internal_ids() {
    let source_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_inference_error_{}.rl",
        std::process::id()
    ));

    fs::write(
        &source_path,
        r#"fun main: () -> Int32 = {
    val items = [];
    0
}
"#,
    )
    .expect("inference error source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--check")
        .arg(&source_path)
        .output()
        .expect("restrict_lang binary should run");

    assert!(!output.status.success(), "inference check should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Cannot infer type for binding 'items'"),
        "stderr should identify the unresolved binding, got: {stderr}"
    );
    assert!(
        stderr.contains("empty list requires an expected List type"),
        "stderr should explain the empty-list context requirement, got: {stderr}"
    );
    for internal in ["?0", "InferVar", "TypeVarId", "Projection"] {
        assert!(
            !stderr.contains(internal),
            "stderr should not expose type inference internals ({internal}), got: {stderr}"
        );
    }

    let _ = fs::remove_file(source_path);
}

#[test]
fn cli_unresolved_builtin_projection_error_hides_internal_ids() {
    let source_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_unresolved_builtin_projection_{}.rl",
        std::process::id()
    ));

    fs::write(
        &source_path,
        r#"fun main: () -> Int32 = {
    val apply_map = map;
    0
}
"#,
    )
    .expect("unresolved builtin projection source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--check")
        .arg(&source_path)
        .output()
        .expect("restrict_lang binary should run");

    assert!(
        !output.status.success(),
        "unresolved builtin projection check should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Cannot infer type for binding 'apply_map'"),
        "stderr should identify the unresolved builtin binding, got: {stderr}"
    );
    for internal in ["?0", "InferVar", "TypeVarId", "Projection"] {
        assert!(
            !stderr.contains(internal),
            "stderr should not expose type inference internals ({internal}), got: {stderr}"
        );
    }

    let _ = fs::remove_file(source_path);
}

#[test]
fn cli_none_without_expected_type_identifies_binding_context() {
    let (source_path, output) = run_check_temp_source(
        "none_without_expected_type",
        r#"fun main: () -> Int32 = {
    val maybe = None;
    0
}
"#,
    );

    assert!(!output.status.success(), "None inference check should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Cannot infer type for binding 'maybe'"),
        "stderr should identify the unresolved Option binding, got: {stderr}"
    );
    assert!(
        stderr.contains("None requires an expected Option type"),
        "stderr should explain the Option context requirement, got: {stderr}"
    );
    assert_no_inference_internals("None inference diagnostic", &stderr);

    let _ = fs::remove_file(source_path);
}

#[test]
fn cli_ok_without_expected_type_identifies_binding_context() {
    let (source_path, output) = run_check_temp_source(
        "ok_without_expected_type",
        r#"fun main: () -> Int32 = {
    val result = Ok(1);
    0
}
"#,
    );

    assert!(!output.status.success(), "Ok inference check should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Cannot infer type for binding 'result'"),
        "stderr should identify the unresolved Result binding, got: {stderr}"
    );
    assert!(
        stderr.contains("Ok/Err requires an expected Result type"),
        "stderr should explain the Result context requirement, got: {stderr}"
    );
    assert_no_inference_internals("Ok inference diagnostic", &stderr);

    let _ = fs::remove_file(source_path);
}

#[test]
fn cli_err_without_expected_type_identifies_binding_context() {
    let (source_path, output) = run_check_temp_source(
        "err_without_expected_type",
        r#"fun main: () -> Int32 = {
    val result = Err("network");
    0
}
"#,
    );

    assert!(!output.status.success(), "Err inference check should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Cannot infer type for binding 'result'"),
        "stderr should identify the unresolved Result binding, got: {stderr}"
    );
    assert!(
        stderr.contains("Ok/Err requires an expected Result type"),
        "stderr should explain the Result context requirement, got: {stderr}"
    );
    assert_no_inference_internals("Err inference diagnostic", &stderr);

    let _ = fs::remove_file(source_path);
}

#[test]
fn cli_unresolved_contextless_lambda_identifies_expected_function_context() {
    let (source_path, output) = run_check_temp_source(
        "contextless_lambda",
        r#"fun main: () -> Int32 = {
    val identity = |value| value;
    0
}
"#,
    );

    assert!(
        !output.status.success(),
        "contextless lambda inference check should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Cannot infer type for binding 'identity'"),
        "stderr should identify the unresolved lambda binding, got: {stderr}"
    );
    assert!(
        stderr.contains("expected function type") || stderr.contains("type annotation"),
        "stderr should suggest adding function context or annotation, got: {stderr}"
    );
    assert_no_inference_internals("contextless lambda diagnostic", &stderr);

    let _ = fs::remove_file(source_path);
}

#[test]
fn cli_compile_unresolved_inference_stops_before_codegen() {
    let cases = [
        (
            "empty_list",
            r#"fun main: () -> Int32 = {
    val items = [];
    0
}
"#,
            "Cannot infer type for binding 'items'",
        ),
        (
            "builtin_projection",
            r#"fun main: () -> Int32 = {
    val apply_map = map;
    0
}
"#,
            "Cannot infer type for binding 'apply_map'",
        ),
        (
            "none",
            r#"fun main: () -> Int32 = {
    val maybe = None;
    0
}
"#,
            "Cannot infer type for binding 'maybe'",
        ),
        (
            "ok",
            r#"fun main: () -> Int32 = {
    val result = Ok(1);
    0
}
"#,
            "Cannot infer type for binding 'result'",
        ),
        (
            "contextless_lambda",
            r#"fun main: () -> Int32 = {
    val identity = |value| value;
    0
}
"#,
            "Cannot infer type for binding 'identity'",
        ),
    ];

    for (stem, source, expected) in cases {
        let (source_path, output_path, output) = run_compile_temp_source(stem, source);

        assert!(!output.status.success(), "{stem} compile should fail");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("Type error:"),
            "{stem} should fail in type checking before codegen, got: {stderr}"
        );
        assert!(
            stderr.contains(expected),
            "{stem} diagnostic should preserve binding context {expected}, got: {stderr}"
        );
        assert!(
            !stderr.contains("Code generation error") && !stderr.contains("Unsupported feature"),
            "{stem} should not reach codegen fallback, got: {stderr}"
        );
        assert_no_inference_internals(stem, &stderr);
        assert!(
            !output_path.exists(),
            "{stem} failed compile should not leave a WAT output at {}",
            output_path.display()
        );

        let _ = fs::remove_file(source_path);
        let _ = fs::remove_file(output_path);
    }
}

#[test]
fn cli_unresolved_generic_function_value_identifies_expected_function_context() {
    let (source_path, output) = run_check_temp_source(
        "generic_function_value",
        r#"fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> Int32 = {
    val chooser = choose_first;
    0
}
"#,
    );

    assert!(
        !output.status.success(),
        "generic function value inference check should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Cannot infer type for binding 'chooser'"),
        "stderr should identify the unresolved generic function binding, got: {stderr}"
    );
    assert!(
        stderr.contains("expected function type") || stderr.contains("type annotation"),
        "stderr should suggest adding function context or annotation, got: {stderr}"
    );
    assert_no_inference_internals("generic function value diagnostic", &stderr);

    let _ = fs::remove_file(source_path);
}

#[test]
fn cli_array_type_boundary_reports_public_length_syntax() {
    let (source_path, output) = run_check_temp_source(
        "array_type_boundary",
        r#"fun main: () -> Int32 = {
    val scores: Array<Int32> = [1, 2, 3];
    0
}
"#,
    );

    assert!(
        !output.status.success(),
        "source Array<T> boundary check should fail"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().is_empty(),
        "--check failure should not print OK, stdout: {stdout}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Array type requires explicit length"),
        "stderr should explain the public array length requirement, got: {stderr}"
    );
    assert!(
        stderr.contains("Array<T, N>"),
        "stderr should show the supported v0.0.1 array syntax, got: {stderr}"
    );
    assert_no_inference_internals("Array<T> boundary diagnostic", &stderr);

    let _ = fs::remove_file(source_path);
}

#[test]
fn cli_v001_release_surface_error_hides_internal_diagnostic_terms() {
    let (source_path, output) = run_check_temp_source(
        "v001_release_surface_composite_export",
        r#"pub fun release_label: () = {
    "stable"
}

fun main: () -> Int32 = {
    1
}
"#,
    );

    assert!(
        !output.status.success(),
        "release surface CLI check should fail"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().is_empty(),
        "--check failure should not print OK, stdout: {stdout}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Release surface error"),
        "stderr should identify the release surface gate, got: {stderr}"
    );
    assert!(
        stderr.contains(
            "Exported function 'release_label' return type String requires a composite host ABI"
        ),
        "stderr should explain the unsupported export ABI, got: {stderr}"
    );
    for internal in [
        "InferVar",
        "TypeVarId",
        "Projection",
        "Feature not implemented",
    ] {
        assert!(
            !stderr.contains(internal),
            "stderr should not expose internal diagnostic text ({internal}), got: {stderr}"
        );
    }

    let _ = fs::remove_file(source_path);
}

#[test]
fn cli_enum_gap_reports_unsupported_feature_cleanly() {
    let source_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_enum_gap_{}.rl",
        std::process::id()
    ));

    fs::write(
        &source_path,
        r#"enum ReviewState { Ready }

fun main: () -> Int32 = {
    0
}
"#,
    )
    .expect("enum gap source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--check")
        .arg(&source_path)
        .output()
        .expect("restrict_lang binary should run");

    assert!(!output.status.success(), "enum gap check should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("enum declarations are unsupported in v0.0.1"),
        "stderr should preserve the unsupported enum message, got: {stderr}"
    );
    assert!(
        stderr.contains("user-defined enum declarations are not implemented"),
        "stderr should identify the enum declaration gap, got: {stderr}"
    );
    for internal in [
        "unexpected input near",
        "Error(",
        "ErrorKind",
        "nom::",
        "Tag",
        "Alt",
    ] {
        assert!(
            !stderr.contains(internal),
            "stderr should not expose parser internals ({internal}), got: {stderr}"
        );
    }

    let _ = fs::remove_file(source_path);
}

#[test]
fn cli_form_takes_gap_reports_unsupported_container_boundary_cleanly() {
    let cases = [
        (
            "form_gap",
            r#"form Container<T> {
    Item
}

fun main: () -> Int32 = {
    0
}
"#,
        ),
        (
            "takes_gap",
            r#"takes List<T> Container {
    Item = T
}

fun main: () -> Int32 = {
    0
}
"#,
        ),
    ];

    for (stem, source) in cases {
        let (source_path, output) = run_check_temp_source(stem, source);

        assert!(!output.status.success(), "{stem} check should fail");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("source-level `form` / `takes` syntax is unsupported in v0.0.1"),
            "{stem} diagnostic should explain the v0.0.1 form/takes boundary, got: {stderr}"
        );
        assert!(
            stderr.contains("compiler-internal Container behavior"),
            "{stem} diagnostic should identify the Container-only internal behavior, got: {stderr}"
        );
        assert!(
            !stderr.contains("unexpected input near"),
            "{stem} diagnostic should preserve the unsupported-feature message, got: {stderr}"
        );
        assert_no_parser_internals(stem, &stderr);

        let _ = fs::remove_file(source_path);
    }
}

#[test]
fn cli_import_alias_gap_reports_unsupported_feature_cleanly() {
    let source_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_import_alias_gap_{}.rl",
        std::process::id()
    ));

    fs::write(
        &source_path,
        r#"import "std/io" as io

fun main: () -> Int32 = {
    0
}
"#,
    )
    .expect("import alias gap source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--check")
        .arg(&source_path)
        .output()
        .expect("restrict_lang binary should run");

    assert!(!output.status.success(), "import alias check should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("string import paths and import aliases are unsupported in v0.0.1"),
        "stderr should preserve the unsupported import alias message, got: {stderr}"
    );
    for internal in [
        "unexpected input near",
        "Error(",
        "ErrorKind",
        "nom::",
        "Tag",
        "Alt",
    ] {
        assert!(
            !stderr.contains(internal),
            "stderr should not expose parser internals ({internal}), got: {stderr}"
        );
    }

    let _ = fs::remove_file(source_path);
}

#[test]
fn cli_reexport_gap_reports_unsupported_feature_cleanly() {
    let source_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_reexport_gap_{}.rl",
        std::process::id()
    ));

    fs::write(
        &source_path,
        r#"export import release.policy.{score}

fun main: () -> Int32 = {
    0
}
"#,
    )
    .expect("re-export gap source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--check")
        .arg(&source_path)
        .output()
        .expect("restrict_lang binary should run");

    assert!(!output.status.success(), "re-export check should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("re-exports are unsupported in v0.0.1"),
        "stderr should preserve the unsupported re-export message, got: {stderr}"
    );
    for internal in [
        "unexpected input near",
        "Error(",
        "ErrorKind",
        "nom::",
        "Tag",
        "Alt",
    ] {
        assert!(
            !stderr.contains(internal),
            "stderr should not expose parser internals ({internal}), got: {stderr}"
        );
    }

    let _ = fs::remove_file(source_path);
}

#[test]
fn cli_import_parse_error_hides_nom_debug_details() {
    let dir = std::env::temp_dir().join(format!(
        "restrict_lang_cli_import_parse_error_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("import parse error temp dir should be writable");

    let root_path = dir.join("main.rl");
    let module_path = dir.join("release.rl");

    fs::write(
        &root_path,
        r#"
import release.{public_score}

fun main: () -> Int32 = {
    41 |> public_score
}
"#,
    )
    .expect("root import source should be writable");
    fs::write(
        &module_path,
        r#"
export fun public_score: (value: Int32) -> Int32 = {
    val answer =
}
"#,
    )
    .expect("malformed module source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--check")
        .arg(&root_path)
        .output()
        .expect("restrict_lang binary should run");

    assert!(
        !output.status.success(),
        "malformed import module check should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Import resolution error: Failed to parse module release"),
        "stderr should identify the malformed import module, got: {stderr}"
    );
    assert!(
        stderr.contains("Parsing error at line"),
        "stderr should include a concise parse location, got: {stderr}"
    );
    for internal in ["Error(", "ErrorKind", "nom::", "Tag", "Alt"] {
        assert!(
            !stderr.contains(internal),
            "stderr should not expose parser internals ({internal}), got: {stderr}"
        );
    }

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_verbose_mode_keeps_phase_details_available() {
    let source_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_verbose_{}.rl",
        std::process::id()
    ));
    let output_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_verbose_{}.wat",
        std::process::id()
    ));
    let _ = fs::remove_file(&output_path);

    fs::write(
        &source_path,
        r#"fun main: () -> Int32 = {
    42
}
"#,
    )
    .expect("verbose source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--verbose")
        .arg(&source_path)
        .arg(&output_path)
        .output()
        .expect("restrict_lang binary should run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "verbose compile should pass\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("=== Lexing ===")
            && stdout.contains("Tokens:")
            && stdout.contains("AST:")
            && stdout.contains("=== WASM Code Generation ==="),
        "verbose mode should retain phase details, stdout: {stdout}"
    );

    let _ = fs::remove_file(source_path);
    let _ = fs::remove_file(output_path);
}

#[test]
fn cli_compiled_export_executes() {
    let source_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_export_{}.rl",
        std::process::id()
    ));
    let output_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_export_{}.wat",
        std::process::id()
    ));
    let _ = fs::remove_file(&output_path);

    fs::write(
        &source_path,
        r#"export fun score_release: (base: Int32, risk: Int32) -> Int32 = {
    base + (risk * 2)
}
"#,
    )
    .expect("export source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg(&source_path)
        .arg(&output_path)
        .output()
        .expect("restrict_lang binary should run");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "export source should compile through the CLI, stderr: {}",
        stderr
    );
    assert_success_streams("export CLI", &output);

    let wat = fs::read_to_string(&output_path).expect("compiled export WAT should be readable");
    let (mut store, instance) = instantiate_wat("export CLI", &wat);
    let score_release = instance
        .get_typed_func::<(i32, i32), i32>(&store, "score_release")
        .expect("CLI-generated export should be host-callable");

    assert_eq!(
        score_release
            .call(&mut store, (10, 4))
            .expect("CLI-generated export should execute"),
        18
    );

    let _ = fs::remove_file(source_path);
    let _ = fs::remove_file(output_path);
}

#[test]
fn cli_compiles_exported_example_to_callable_wasm() {
    let output_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_dogfood_spec_literals_{}.wat",
        std::process::id()
    ));
    let _ = fs::remove_file(&output_path);

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("examples/dogfood_spec_literals_inference.rl")
        .arg(&output_path)
        .output()
        .expect("restrict_lang binary should run");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "dogfood spec literals example should compile through the CLI, stderr: {}",
        stderr
    );
    assert_success_streams("dogfood spec literals CLI", &output);

    let wat = fs::read_to_string(&output_path).expect("compiled dogfood WAT should be readable");
    let (mut store, instance) = instantiate_wat("dogfood spec literals CLI", &wat);
    let exported_bias = instance
        .get_typed_func::<(), i32>(&store, "exported_bias")
        .expect("dogfood exported_bias should be host-callable");

    assert_eq!(
        exported_bias
            .call(&mut store, ())
            .expect("dogfood exported_bias should execute"),
        16
    );

    let _ = fs::remove_file(output_path);
}

#[test]
fn cli_compiles_dogfood_type_inference_to_semantic_wat() {
    let output_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_dogfood_type_inference_{}.wat",
        std::process::id()
    ));
    let _ = fs::remove_file(&output_path);

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("examples/dogfood_type_inference.rl")
        .arg(&output_path)
        .output()
        .expect("restrict_lang binary should run");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "dogfood type inference example should compile through the CLI, stderr: {}",
        stderr
    );
    assert_success_streams("dogfood type inference CLI", &output);

    let wat = fs::read_to_string(&output_path).expect("compiled dogfood WAT should be readable");
    assert!(wat.contains("(func $summarize_review (param $batch i32) (result i32)"));
    assert!(
        wat.contains("local.get $owner\n    i32.store"),
        "summarize_review should store the inferred owner field in ReviewSummary:\n{wat}"
    );
    assert!(
        wat.contains("local.get $total_risk\n    i32.store"),
        "summarize_review should store the folded risk field in ReviewSummary:\n{wat}"
    );
    assert!(
        wat.contains("local.get $approved\n    i32.store"),
        "summarize_review should store the inferred Boolean approval field:\n{wat}"
    );
    assert!(
        wat.contains("local.get $escalation\n    i32.store"),
        "summarize_review should store the inferred Result escalation field:\n{wat}"
    );
    assert!(
        wat.contains("(func $main (result i32)\n"),
        "dogfood main should keep its source-level result ABI so Restrict wrappers can call it:\n{wat}"
    );
    assert!(
        wat.contains("call $summarize_review\n  )"),
        "dogfood main should return the summarized score through its source-level ABI:\n{wat}"
    );
    assert!(
        wat.contains("(func $__restrict_start")
            && wat.contains("call $main\n    drop")
            && wat.contains("(export \"_start\" (func $__restrict_start))"),
        "_start should be a no-result wrapper that calls main and drops its result:\n{wat}"
    );
    assert_instantiable_wat("dogfood type inference CLI", &wat);

    let _ = fs::remove_file(output_path);
}

#[test]
fn cli_compiles_v001_release_identity_regression() {
    let source_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_release_identity_{}.rl",
        std::process::id()
    ));
    let output_path = std::env::temp_dir().join(format!(
        "restrict_lang_cli_release_identity_{}.wat",
        std::process::id()
    ));
    let _ = fs::remove_file(&output_path);

    fs::write(
        &source_path,
        r#"record ReleaseCandidate {
    id: Int32,
    risk: Int32,
    owner: Option<Int32>
}

record ReleaseQueue {
    audit_candidates: List<ReleaseCandidate>,
    ship_candidates: List<ReleaseCandidate>,
    manual_owner: Option<Int32>,
    fallback_owner: Int32,
    risk_limit: Int32
}

record ReleasePlan {
    selected_ids: List<Int32>,
    owner: Int32,
    decision: Result<Int32, Int32>,
    sampled_ids: Option<List<Int32>>
}

fun choose_value: <T>(preferred: Option<T>, fallback: T) -> T = {
    preferred match {
        Some(value) => {
            value
        }
        None => {
            fallback
        }
    }
}

fun owner_bonus: (owner: Option<Int32>) -> Int32 = {
    owner match {
        Some(value) => {
            5
        }
        None => {
            0
        }
    }
}

fun is_ready: (candidate: ReleaseCandidate) -> Boolean = {
    val ReleaseCandidate {
        id,
        risk,
        owner
    } = candidate;
    val score = risk - (owner |> owner_bonus);

    score < 75
}

fun candidate_id: (candidate: ReleaseCandidate) -> Int32 = {
    val ReleaseCandidate {
        id,
        risk,
        owner
    } = candidate;

    id
}

fun add_risk: (total: Int32, candidate: ReleaseCandidate) -> Int32 = {
    val ReleaseCandidate {
        id,
        risk,
        owner
    } = candidate;

    total + risk
}

fun decide_release: (risk_total: Int32, risk_limit: Int32, owner: Int32) -> Result<Int32, Int32> = {
    risk_total <= risk_limit then {
        Ok(owner)
    } else {
        Err(risk_total - risk_limit)
    }
}

fun empty_plan: () -> ReleasePlan = {
    ReleasePlan {
        selected_ids: [],
        owner: 0,
        decision: Err(0),
        sampled_ids: Some([])
    }
}

fun plan_release: (queue: ReleaseQueue) -> ReleasePlan = {
    val ReleaseQueue {
        audit_candidates,
        ship_candidates,
        manual_owner,
        fallback_owner,
        risk_limit
    } = queue;
    val ready_candidates = (ship_candidates, |candidate| candidate |> is_ready) filter;
    val selected_ids = (ready_candidates, candidate_id) map;
    val risk_total = (audit_candidates, 0, add_risk) fold;
    val owner = (manual_owner, fallback_owner) choose_value;
    val decision = (risk_total, risk_limit, owner) decide_release;

    ReleasePlan {
        selected_ids: selected_ids,
        owner: owner,
        decision: decision,
        sampled_ids: None
    }
}

fun build_release_identity_plan: () -> ReleasePlan = {
    val audit_candidates: List<ReleaseCandidate> = [
        ReleaseCandidate {
            id: 101,
            risk: 42,
            owner: Some(7)
        },
        ReleaseCandidate {
            id: 102,
            risk: 31,
            owner: None
        }
    ];
    val ship_candidates: List<ReleaseCandidate> = [
        ReleaseCandidate {
            id: 201,
            risk: 21,
            owner: Some(9)
        },
        ReleaseCandidate {
            id: 202,
            risk: 91,
            owner: None
        }
    ];
    val queue = ReleaseQueue {
        audit_candidates: audit_candidates,
        ship_candidates: ship_candidates,
        manual_owner: None,
        fallback_owner: 44,
        risk_limit: 90
    };

    queue |> plan_release
}

export fun release_identity_score: () -> Int32 = {
    val plan = () build_release_identity_plan;
    val ReleasePlan {
        selected_ids,
        owner,
        decision,
        sampled_ids
    } = plan;
    val selected_count = selected_ids |> list_count;
    val decision_score = decision match {
        Ok(value) => {
            value
        }
        Err(error) => {
            0 - error
        }
    };
    val sampled_count = sampled_ids match {
        Some(ids) => {
            ids |> list_count
        }
        None => {
            0
        }
    };

    owner + selected_count + decision_score + sampled_count
}
"#,
    )
    .expect("release identity source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg(&source_path)
        .arg(&output_path)
        .output()
        .expect("restrict_lang binary should run");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "release identity regression should compile through the CLI, stderr: {}",
        stderr
    );
    assert_success_streams("release identity CLI", &output);

    let wat =
        fs::read_to_string(&output_path).expect("compiled release identity WAT should be readable");
    assert!(wat.trim_start().starts_with("(module"));
    assert!(wat.contains("(func $plan_release"));
    assert!(wat.contains("(func $empty_plan"));

    let (mut store, instance) = instantiate_wat("release identity CLI", &wat);
    let release_identity_score = instance
        .get_typed_func::<(), i32>(&store, "release_identity_score")
        .expect("release identity wrapper should be host-callable");
    assert_eq!(
        release_identity_score
            .call(&mut store, ())
            .expect("release identity wrapper should execute"),
        89
    );

    let _ = fs::remove_file(source_path);
    let _ = fs::remove_file(output_path);
}
