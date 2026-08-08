//! Comprehensive tests for pattern matching exhaustiveness checking.
//!
//! These tests intentionally use the current Restrict syntax:
//! `value match { pattern => { body } }`.

use restrict_lang::parser::parse_program;
use restrict_lang::type_checker::{TypeChecker, TypeError};

fn create_match_program(param_type: &str, return_type: &str, arms: &str) -> String {
    format!(
        r#"
fun test: (x: {}) -> {} = {{
    x match {{
{}
    }}
}}
"#,
        param_type, return_type, arms
    )
}

fn expect_non_exhaustive_error(source: &str, expected_missing: &str) {
    let (remaining, program) = parse_program(source).expect("parse should succeed");
    assert!(
        remaining.trim().is_empty(),
        "parser should consume all input, remaining: {:?}",
        remaining
    );

    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Err(TypeError::NonExhaustivePatterns { missing, .. }) => {
            assert!(
                missing.contains(expected_missing),
                "expected missing pattern '{}' but got '{}'",
                expected_missing,
                missing
            );
        }
        Err(other_error) => {
            panic!(
                "expected NonExhaustivePatterns error but got {:?}",
                other_error
            );
        }
        Ok(_) => {
            panic!("expected exhaustiveness error but type checking succeeded");
        }
    }
}

fn expect_exhaustive(source: &str) {
    let (remaining, program) = parse_program(source).expect("parse should succeed");
    assert!(
        remaining.trim().is_empty(),
        "parser should consume all input, remaining: {:?}",
        remaining
    );

    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("type checking should succeed for exhaustive patterns");
}

#[test]
fn boolean_exhaustive_both_cases() {
    let source = create_match_program(
        "Boolean",
        "Int32",
        r#"
        true => { 1 }
        false => { 0 }
"#,
    );
    expect_exhaustive(&source);
}

#[test]
fn boolean_non_exhaustive_missing_false() {
    let source = create_match_program(
        "Boolean",
        "Int32",
        r#"
        true => { 1 }
"#,
    );
    expect_non_exhaustive_error(&source, "false");
}

#[test]
fn boolean_non_exhaustive_missing_true() {
    let source = create_match_program(
        "Boolean",
        "Int32",
        r#"
        false => { 0 }
"#,
    );
    expect_non_exhaustive_error(&source, "true");
}

#[test]
fn boolean_exhaustive_with_wildcard() {
    let source = create_match_program(
        "Boolean",
        "Int32",
        r#"
        true => { 1 }
        _ => { 0 }
"#,
    );
    expect_exhaustive(&source);
}

#[test]
fn option_exhaustive_both_cases() {
    let source = create_match_program(
        "Option<Int32>",
        "Int32",
        r#"
        Some(value) => { value }
        None => { 0 }
"#,
    );
    expect_exhaustive(&source);
}

#[test]
fn option_non_exhaustive_missing_none() {
    let source = create_match_program(
        "Option<Int32>",
        "Int32",
        r#"
        Some(value) => { value }
"#,
    );
    expect_non_exhaustive_error(&source, "None");
}

#[test]
fn option_non_exhaustive_missing_some() {
    let source = create_match_program(
        "Option<Int32>",
        "Int32",
        r#"
        None => { 0 }
"#,
    );
    expect_non_exhaustive_error(&source, "Some(_)");
}

#[test]
fn nested_option_exhaustive() {
    let source = create_match_program(
        "Option<Option<Int32>>",
        "Int32",
        r#"
        Some(Some(value)) => { value }
        Some(None) => { 0 }
        None => { -1 }
"#,
    );
    expect_exhaustive(&source);
}

#[test]
fn nested_option_non_exhaustive_missing_inner_none() {
    let source = create_match_program(
        "Option<Option<Int32>>",
        "Int32",
        r#"
        Some(Some(value)) => { value }
        None => { -1 }
"#,
    );
    expect_non_exhaustive_error(&source, "Some(None)");
}

#[test]
fn nested_option_boolean_exhaustive() {
    let source = create_match_program(
        "Option<Boolean>",
        "Int32",
        r#"
        Some(true) => { 1 }
        Some(false) => { 0 }
        None => { -1 }
"#,
    );
    expect_exhaustive(&source);
}

#[test]
fn nested_option_boolean_non_exhaustive() {
    let source = create_match_program(
        "Option<Boolean>",
        "Int32",
        r#"
        Some(true) => { 1 }
        None => { -1 }
"#,
    );
    expect_non_exhaustive_error(&source, "Some(false)");
}

#[test]
fn list_exhaustive_empty_and_cons() {
    let source = create_match_program(
        "List<Int32>",
        "Int32",
        r#"
        [] => { 0 }
        [head | tail] => { head }
"#,
    );
    expect_exhaustive(&source);
}

#[test]
fn list_non_exhaustive_missing_empty() {
    let source = create_match_program(
        "List<Int32>",
        "Int32",
        r#"
        [head | tail] => { head }
"#,
    );
    expect_non_exhaustive_error(&source, "[]");
}

#[test]
fn list_non_exhaustive_missing_cons() {
    let source = create_match_program(
        "List<Int32>",
        "Int32",
        r#"
        [] => { 0 }
"#,
    );
    expect_non_exhaustive_error(&source, "[_|_]");
}

#[test]
fn list_exact_patterns_need_cons() {
    let source = create_match_program(
        "List<Int32>",
        "Int32",
        r#"
        [] => { 0 }
        [a] => { a }
        [a, b] => { a + b }
"#,
    );
    expect_non_exhaustive_error(&source, "cons pattern needed");
}

#[test]
fn unit_exhaustive() {
    let source = create_match_program(
        "()",
        "Int32",
        r#"
        () => { 42 }
"#,
    );
    expect_exhaustive(&source);
}

#[test]
fn record_patterns_basic() {
    let source = r#"
record Point {
    x: Int32,
    y: Int32
}

fun test: (p: Point) -> Int32 = {
    p match {
        Point { x, y } => { x + y }
    }
}
"#;
    expect_exhaustive(source);
}

#[test]
fn record_literal_field_pattern_is_not_exhaustive() {
    let source = r#"
record Feature {
    enabled: Boolean,
    score: Int32
}

fun test: (feature: Feature) -> Int32 = {
    feature match {
        Feature { enabled: true, ..._ } => { 1 }
    }
}
"#;
    expect_non_exhaustive_error(source, "Feature{ .. }");
}

#[test]
fn record_boolean_field_patterns_can_be_exhaustive() {
    let source = r#"
record Feature {
    enabled: Boolean,
    score: Int32
}

fun test: (feature: Feature) -> Int32 = {
    feature match {
        Feature { enabled: true, ..._ } => { 1 }
        Feature { enabled: false, ..._ } => { 0 }
    }
}
"#;
    expect_exhaustive(source);
}

#[test]
fn record_restricted_int_field_requires_catch_all() {
    let source = r#"
record Route {
    status: Int32,
    open: Boolean
}

fun test: (route: Route) -> Int32 = {
    route match {
        Route { status: 200, ..._ } => { 1 }
    }
}
"#;
    expect_non_exhaustive_error(source, "Route{ .. }");
}

#[test]
fn infinite_types_require_wildcard() {
    let source = create_match_program(
        "Int32",
        "String",
        r#"
        42 => { "answer" }
        0 => { "zero" }
"#,
    );
    expect_non_exhaustive_error(&source, "pattern required for infinite type");
}

#[test]
fn infinite_types_with_wildcard() {
    let source = create_match_program(
        "Int32",
        "String",
        r#"
        42 => { "answer" }
        0 => { "zero" }
        _ => { "other" }
"#,
    );
    expect_exhaustive(&source);
}

#[test]
fn wildcard_makes_everything_exhaustive() {
    let source = create_match_program(
        "String",
        "String",
        r#"
        _ => { "anything" }
"#,
    );
    expect_exhaustive(&source);
}

#[test]
fn identifier_pattern_makes_everything_exhaustive() {
    let source = create_match_program(
        "Int32",
        "Int32",
        r#"
        value => { value }
"#,
    );
    expect_exhaustive(&source);
}

#[test]
fn complex_nested_option_list() {
    let source = create_match_program(
        "Option<List<Int32>>",
        "Int32",
        r#"
        Some([]) => { 0 }
        Some([head | tail]) => { head }
        None => { -1 }
"#,
    );
    expect_exhaustive(&source);
}

#[test]
fn complex_nested_option_list_non_exhaustive() {
    let source = create_match_program(
        "Option<List<Int32>>",
        "Int32",
        r#"
        Some([head | tail]) => { head }
        None => { -1 }
"#,
    );
    expect_non_exhaustive_error(&source, "Some([])");
}

#[test]
fn error_message_includes_suggestion() {
    let source = create_match_program(
        "Boolean",
        "Int32",
        r#"
        true => { 1 }
"#,
    );

    let (remaining, program) = parse_program(&source).expect("parse should succeed");
    assert!(
        remaining.trim().is_empty(),
        "parser should consume all input, remaining: {:?}",
        remaining
    );

    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Err(TypeError::NonExhaustivePatterns { suggestion, .. }) => {
            assert!(
                suggestion.contains("Add the missing patterns or use a wildcard pattern"),
                "suggestion should be helpful: '{}'",
                suggestion
            );
        }
        other => {
            panic!(
                "expected NonExhaustivePatterns error with suggestion but got {:?}",
                other
            );
        }
    }
}
