use std::fs;
use std::process::Command;

use restrict_lang::{
    check_release_surface as validate_release_surface, check_v001_release_surface, parse_program,
    HostAbiProfile, TypeChecker,
};

fn check_release_surface(source: &str) -> Result<(), String> {
    let (program, checker) = checked_program(source)?;
    check_v001_release_surface(&program, &checker).map_err(|err| err.to_string())
}

fn check_release_surface_with_profile(source: &str, profile: HostAbiProfile) -> Result<(), String> {
    let (program, checker) = checked_program(source)?;
    validate_release_surface(&program, &checker, profile).map_err(|err| err.to_string())
}

fn checked_program(source: &str) -> Result<(restrict_lang::Program, TypeChecker), String> {
    let (remaining, program) = parse_program(source).map_err(|err| format!("parse: {err:?}"))?;
    if !remaining.trim().is_empty() {
        return Err(format!("unparsed input remaining: {remaining:?}"));
    }

    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .map_err(|err| format!("type: {err}"))?;
    Ok((program, checker))
}

fn assert_release_error(source: &str, expected: &str) {
    let err = check_release_surface(source).expect_err("source should fail release validation");
    assert!(
        err.contains(expected),
        "release error should mention {expected}, got: {err}"
    );
}

fn assert_profile_error(source: &str, profile: HostAbiProfile, expected: &str) {
    let err = check_release_surface_with_profile(source, profile)
        .expect_err("source should fail profile validation");
    assert!(
        err.contains(expected),
        "profile error should mention {expected}, got: {err}"
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
fn v001_wrapper_matches_explicit_v001_profile() {
    let source = r#"
record ReleaseSlice {
    score: Int32
}

pub fun public_score: (slice: ReleaseSlice) -> Int32 = {
    slice.score
}
"#;

    assert_eq!(
        check_release_surface(source),
        check_release_surface_with_profile(source, HostAbiProfile::V001Scalar)
    );
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
fn flat_record_v1_accepts_source_exported_direct_scalar_records() {
    check_release_surface_with_profile(
        r#"
pub record Reading {
    count: Int32,
    total: Int64,
    ratio: Float64,
    active: Boolean,
    marker: Char
}

pub fun keep_reading: (reading: Reading, unused_bias: Int32) -> Reading = {
    reading
}

pub fun default_reading: () = {
    Reading {
        count: 1,
        total: 2,
        ratio: 3.0,
        active: true,
        marker: ':'
    }
}

fun main: () -> Int32 = {
    1
}
"#,
        HostAbiProfile::FlatRecordV1,
    )
    .expect("flat-record-v1 should accept explicit and inferred flat scalar records");
}

#[test]
fn flat_record_v1_requires_record_declarations_to_be_source_exported() {
    assert_profile_error(
        r#"
record PrivateReading {
    count: Int32
}

pub fun keep_private: (reading: PrivateReading) -> PrivateReading = {
    reading
}
"#,
        HostAbiProfile::FlatRecordV1,
        "record 'PrivateReading' must be source-exported",
    );
}

#[test]
fn flat_record_v1_rejects_unsupported_record_shapes() {
    let cases = [
        (
            "empty record",
            r#"
pub record Empty {
}

pub fun inspect: (value: Empty) -> Int32 = {
    1
}
"#,
            "record 'Empty' is empty",
        ),
        (
            "Unit field",
            r#"
pub record UnitField {
    marker: ()
}

pub fun inspect: (value: UnitField) -> Int32 = {
    1
}
"#,
            "field 'marker' type () is unsupported",
        ),
        (
            "String field",
            r#"
pub record StringField {
    label: String
}

pub fun inspect: (value: StringField) -> Int32 = {
    1
}
"#,
            "field 'label' type String is unsupported",
        ),
        (
            "List field",
            r#"
pub record ListField {
    values: List<Int32>
}

pub fun inspect: (value: ListField) -> Int32 = {
    1
}
"#,
            "field 'values' type List<Int32> is unsupported",
        ),
        (
            "Array field",
            r#"
pub record ArrayField {
    values: Array<Int32, 2>
}

pub fun inspect: (value: ArrayField) -> Int32 = {
    1
}
"#,
            "field 'values' type Array<Int32, 2> is unsupported",
        ),
        (
            "Option field",
            r#"
pub record OptionField {
    value: Option<Int32>
}

pub fun inspect: (value: OptionField) -> Int32 = {
    1
}
"#,
            "field 'value' type Option<Int32> is unsupported",
        ),
        (
            "Result field",
            r#"
pub record ResultField {
    value: Result<Int32, Int32>
}

pub fun inspect: (value: ResultField) -> Int32 = {
    1
}
"#,
            "field 'value' type Result<Int32, Int32> is unsupported",
        ),
        (
            "nested record",
            r#"
pub record Inner {
    value: Int32
}

pub record Outer {
    inner: Inner
}

pub fun inspect: (value: Outer) -> Int32 = {
    1
}
"#,
            "field 'inner' type Inner is unsupported",
        ),
        (
            "generic record",
            r#"
pub record Box<T> {
    value: T
}

pub fun inspect: (value: Box<Int32>) -> Int32 = {
    1
}
"#,
            "generic records and built-in composite types are not supported",
        ),
    ];

    for (label, source, expected) in cases {
        let err = check_release_surface_with_profile(source, HostAbiProfile::FlatRecordV1)
            .expect_err("unsupported record shape should fail profile validation");
        assert!(
            err.contains(expected),
            "{label} error should mention {expected}, got: {err}"
        );
    }
}

#[test]
fn flat_record_v1_rejects_direct_dynamic_composites_and_generic_functions() {
    let cases = [
        (
            "String",
            r#"pub fun inspect: (value: String) -> Int32 = { 1 }"#,
        ),
        (
            "List<Int32>",
            r#"pub fun inspect: (value: List<Int32>) -> Int32 = { 1 }"#,
        ),
        (
            "Array<Int32, 2>",
            r#"pub fun inspect: (value: Array<Int32, 2>) -> Int32 = { 1 }"#,
        ),
        (
            "Option<Int32>",
            r#"pub fun inspect: (value: Option<Int32>) -> Int32 = { 1 }"#,
        ),
        (
            "Result<Int32, Int32>",
            r#"pub fun inspect: (value: Result<Int32, Int32>) -> Int32 = { 1 }"#,
        ),
    ];

    for (expected_type, source) in cases {
        assert_profile_error(
            source,
            HostAbiProfile::FlatRecordV1,
            &format!("type {expected_type} is unsupported"),
        );
    }

    assert_profile_error(
        r#"
pub fun keep: <T>(value: T) -> T = {
    value
}
"#,
        HostAbiProfile::FlatRecordV1,
        "Exported generic function 'keep' requires a concrete ABI",
    );
}

#[test]
fn flat_record_v1_rejects_parameter_slot_overflow() {
    assert_profile_error(
        r#"
pub record SixteenSlots {
    f01: Int32, f02: Int32, f03: Int32, f04: Int32,
    f05: Int32, f06: Int32, f07: Int32, f08: Int32,
    f09: Int32, f10: Int32, f11: Int32, f12: Int32,
    f13: Int32, f14: Int32, f15: Int32, f16: Int32
}

pub fun too_wide: (value: SixteenSlots, extra: Int32) -> Int32 = {
    1
}
"#,
        HostAbiProfile::FlatRecordV1,
        "flattens to 17 parameter slots",
    );
}

#[test]
fn flat_record_v1_rejects_record_field_count_overflow() {
    assert_profile_error(
        r#"
pub record SeventeenSlots {
    f01: Int32, f02: Int32, f03: Int32, f04: Int32,
    f05: Int32, f06: Int32, f07: Int32, f08: Int32,
    f09: Int32, f10: Int32, f11: Int32, f12: Int32,
    f13: Int32, f14: Int32, f15: Int32, f16: Int32,
    f17: Int32
}

pub fun too_wide: () -> SeventeenSlots = {
    SeventeenSlots {
        f01: 1, f02: 2, f03: 3, f04: 4,
        f05: 5, f06: 6, f07: 7, f08: 8,
        f09: 9, f10: 10, f11: 11, f12: 12,
        f13: 13, f14: 14, f15: 15, f16: 16,
        f17: 17
    }
}
"#,
        HostAbiProfile::FlatRecordV1,
        "record 'SeventeenSlots' has 17 fields",
    );
}

#[test]
fn flat_record_v1_rejects_record_and_other_composite_globals() {
    assert_profile_error(
        r#"
pub val release_label = "stable"

fun main: () -> Int32 = {
    1
}
"#,
        HostAbiProfile::FlatRecordV1,
        "flat-record-v1 global exports support only scalar",
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

#[test]
fn cli_flat_record_v1_check_is_opt_in() {
    let source_path = std::env::temp_dir().join(format!(
        "restrict_lang_flat_record_v1_check_{}.rl",
        std::process::id()
    ));
    fs::write(
        &source_path,
        r#"
pub record ReleaseSlice {
    score: Int32,
    ratio: Float64
}

pub fun keep_slice: (slice: ReleaseSlice) -> ReleaseSlice = {
    slice
}
"#,
    )
    .expect("temp source should be writable");

    let default_output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--check")
        .arg(&source_path)
        .output()
        .expect("restrict_lang binary should run with its default profile");
    assert!(
        !default_output.status.success(),
        "default v0.0.1 should keep rejecting record-valued host exports"
    );
    assert!(
        String::from_utf8_lossy(&default_output.stderr).contains("requires a composite host ABI"),
        "default stderr should preserve the v0.0.1 boundary: {}",
        String::from_utf8_lossy(&default_output.stderr)
    );

    let opt_in_output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .args(["--host-abi", "flat-record-v1", "--check"])
        .arg(&source_path)
        .output()
        .expect("restrict_lang binary should run with flat-record-v1");
    assert!(
        opt_in_output.status.success(),
        "flat-record-v1 --check should accept the exported record: {}",
        String::from_utf8_lossy(&opt_in_output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&opt_in_output.stdout).starts_with("OK "),
        "successful opt-in check should report OK: {}",
        String::from_utf8_lossy(&opt_in_output.stdout)
    );

    let _ = fs::remove_file(source_path);
}

#[test]
fn cli_host_abi_reports_missing_and_unknown_profiles() {
    let missing = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--host-abi")
        .output()
        .expect("restrict_lang binary should report a missing profile");
    assert!(!missing.status.success());
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("--host-abi requires a value"),
        "missing profile diagnostic should be explicit: {}",
        String::from_utf8_lossy(&missing.stderr)
    );

    let missing_before_option = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .args(["--host-abi", "--check"])
        .output()
        .expect("restrict_lang binary should not consume another option as a profile");
    assert!(!missing_before_option.status.success());
    assert!(
        String::from_utf8_lossy(&missing_before_option.stderr)
            .contains("--host-abi requires a value"),
        "option-like missing profile diagnostic should be explicit: {}",
        String::from_utf8_lossy(&missing_before_option.stderr)
    );

    let unknown = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .args(["--host-abi", "opaque-v9"])
        .output()
        .expect("restrict_lang binary should report an unknown profile");
    assert!(!unknown.status.success());
    assert!(
        String::from_utf8_lossy(&unknown.stderr).contains("unknown host ABI profile 'opaque-v9'"),
        "unknown profile diagnostic should name the invalid value: {}",
        String::from_utf8_lossy(&unknown.stderr)
    );
}
