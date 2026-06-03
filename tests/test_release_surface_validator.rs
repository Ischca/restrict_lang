use std::fs;
use std::process::Command;

use restrict_lang::{check_v001_release_surface, parse_program, TypeChecker};

fn check_release_surface(source: &str) -> Result<(), String> {
    let (remaining, program) = parse_program(source).map_err(|err| format!("parse: {err:?}"))?;
    if !remaining.trim().is_empty() {
        return Err(format!("unparsed input remaining: {remaining:?}"));
    }

    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .map_err(|err| format!("type: {err}"))?;
    check_v001_release_surface(&program, &checker).map_err(|err| err.to_string())
}

fn assert_release_error(source: &str, expected: &str) {
    let err = check_release_surface(source).expect_err("source should fail release validation");
    assert!(
        err.contains(expected),
        "release error should mention {expected}, got: {err}"
    );
}

#[test]
fn release_surface_accepts_scalar_exports_and_source_record_exports() {
    check_release_surface(
        r#"
pub record ReleaseSlice {
    score: Int32
}

pub val release_bias: Int32 = 3

pub fun public_score: (value: Int32) = {
    value + 1
}

fun main: () -> Int32 = {
    41 |> public_score
}
"#,
    )
    .expect("scalar exports and source-level record exports should be v0.0.1 surface");
}

#[test]
fn release_surface_rejects_exported_generic_functions() {
    let err = check_release_surface(
        r#"
pub fun keep: <T>(value: T) -> T = {
    value
}

fun main: () -> Int32 = {
    1 |> keep
}
"#,
    )
    .expect_err("exported generic functions should fail release validation");

    assert_eq!(
        err,
        "Exported generic function 'keep' requires a concrete ABI and is not supported in v0.0.1"
    );
}

#[test]
fn release_surface_accepts_all_scalar_global_exports() {
    check_release_surface(
        r#"
pub val release_bias: Int32 = 3
pub val large_budget: Int64 = 9000000000
pub val confidence_floor: Float64 = 1.5
pub val feature_enabled: Boolean = true
pub val delimiter: Char = ':'
pub val no_payload: () = ()

fun main: () -> Int32 = {
    release_bias
}
"#,
    )
    .expect("all scalar top-level exports should be in the v0.0.1 global ABI surface");
}

#[test]
fn release_surface_rejects_exported_composite_function_abi() {
    assert_release_error(
        r#"
record ReleaseSlice {
    score: Int32
}

pub fun public_score: (slice: ReleaseSlice) -> Int32 = {
    slice.score
}

fun main: () -> Int32 = {
    1
}
"#,
        "Exported function 'public_score' parameter 'slice' type ReleaseSlice requires a composite host ABI",
    );

    assert_release_error(
        r#"
pub fun release_scores: () = {
    [1, 2, 3]
}

fun main: () -> Int32 = {
    1
}
"#,
        "Exported function 'release_scores' return type List<Int32> requires a composite host ABI",
    );
}

#[test]
fn release_surface_rejects_exported_composite_globals() {
    assert_release_error(
        r#"
pub val release_label = "stable"

fun main: () -> Int32 = {
    1
}
"#,
        "Exported top-level binding 'release_label' has type String which requires a composite host ABI",
    );

    assert_release_error(
        r#"
pub val release_scores: List<Int32> = [1, 2]

fun main: () -> Int32 = {
    1
}
"#,
        "Exported top-level binding 'release_scores' has type List<Int32> which requires a composite host ABI",
    );
}

#[test]
fn release_surface_rejects_computed_and_mutable_exported_globals() {
    assert_release_error(
        r#"
pub val release_score: Int32 = 40 + 2

fun main: () -> Int32 = {
    1
}
"#,
        "Exported top-level binding 'release_score' must be a scalar literal constant",
    );

    assert_release_error(
        r#"
pub mut val release_score: Int32 = 42

fun main: () -> Int32 = {
    1
}
"#,
        "Exported top-level bindings must be immutable scalar constants in v0.0.1",
    );
}

#[test]
fn release_surface_rejects_tat_in_default_gate() {
    let err = check_release_surface(
        r#"
record File<~f> {
    handle: Int32
}

fun main: () -> Int32 = {
    1
}
"#,
    )
    .expect_err("TAT record parameters should fail v0.0.1 release validation");

    for expected in [
        "record 'File' uses temporal type parameters",
        "Temporal Affine Types (TAT) are outside the default v0.0.1 release gate",
    ] {
        assert!(
            err.contains(expected),
            "release error should mention {expected}, got: {err}"
        );
    }
}

#[test]
fn cli_check_runs_release_surface_before_reporting_ok() {
    let source_path = std::env::temp_dir().join(format!(
        "restrict_lang_release_surface_check_{}.rl",
        std::process::id()
    ));
    fs::write(
        &source_path,
        r#"
pub fun release_label: () = {
    "stable"
}

fun main: () -> Int32 = {
    1
}
"#,
    )
    .expect("temp source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--check")
        .arg(&source_path)
        .output()
        .expect("restrict_lang binary should run");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success(),
        "--check should reject composite public exports"
    );
    assert!(
        stdout.trim().is_empty(),
        "--check failure should not print OK, stdout: {stdout}"
    );
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

    let _ = fs::remove_file(source_path);
}
