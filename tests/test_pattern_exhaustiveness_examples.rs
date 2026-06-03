//! Real-world examples of pattern matching exhaustiveness.
//!
//! These tests use current Restrict syntax:
//! `value match { pattern => { body } }`.

use restrict_lang::parser::parse_program;
use restrict_lang::type_checker::{TypeChecker, TypeError};

fn type_check(source: &str) -> Result<(), TypeError> {
    let (remaining, program) = parse_program(source).expect("parse should succeed");
    assert!(
        remaining.trim().is_empty(),
        "parser should consume all input, remaining: {remaining:?}"
    );

    let mut checker = TypeChecker::new();
    checker.check_program(&program)
}

fn expect_non_exhaustive(source: &str, expected_missing: &str) {
    match type_check(source) {
        Err(TypeError::NonExhaustivePatterns { missing, .. }) => {
            assert!(
                missing.contains(expected_missing),
                "expected missing pattern {expected_missing:?}, got {missing:?}"
            );
        }
        Err(other) => panic!("expected NonExhaustivePatterns, got {other:?}"),
        Ok(_) => panic!("expected non-exhaustive pattern error"),
    }
}

#[test]
fn example_result_type_handling() {
    let source = r#"
fun process_result: (result: Result<Int32, Int32>) -> Int32 = {
    result match {
        Ok(value) => { value * 2 }
    }
}
"#;

    expect_non_exhaustive(source, "Err");
}

#[test]
fn example_state_machine_completeness() {
    let source = r#"
fun handle_state: (state: Option<Boolean>) -> Int32 = {
    state match {
        Some(true) => { 2 }
        Some(false) => { 1 }
        None => { 0 }
    }
}
"#;

    type_check(source).expect("state handling should cover every state");
}

#[test]
fn example_option_chain_safety() {
    let source = r#"
fun use_result: (result: Option<Int32>) -> Int32 = {
    result match {
        Some(value) => { value }
        None => { 0 }
    }
}
"#;

    type_check(source).expect("option handling should cover Some and None");
}

#[test]
fn example_list_processing_completeness() {
    let source = r#"
fun list_length: (items: List<Int32>) -> Int32 = {
    items match {
        [] => { 0 }
    }
}
"#;

    expect_non_exhaustive(source, "[_|_]");
}

#[test]
fn example_nested_pattern_safety() {
    let source = r#"
fun incomplete_nested: (value: Option<Option<Int32>>) -> Int32 = {
    value match {
        Some(Some(inner)) => { inner }
        None => { 0 }
    }
}
"#;

    expect_non_exhaustive(source, "Some(None)");
}

#[test]
fn example_helpful_error_messages() {
    let source = r#"
fun process_bool: (flag: Boolean) -> Int32 = {
    flag match {
        true => { 1 }
    }
}
"#;

    match type_check(source) {
        Err(TypeError::NonExhaustivePatterns {
            missing,
            suggestion,
        }) => {
            assert!(missing.contains("false"));
            assert!(suggestion.contains("wildcard"));
        }
        Err(other) => panic!("expected NonExhaustivePatterns, got {other:?}"),
        Ok(_) => panic!("expected missing false branch"),
    }
}

#[test]
fn example_performance_conscious_exhaustiveness() {
    let source = r#"
fun process_large_space: (value: Int32) -> Int32 = {
    value match {
        1 => { 10 }
        2 => { 20 }
        3 => { 30 }
        _ => { 0 }
    }
}
"#;

    type_check(source).expect("wildcard should cover the remaining Int32 space");
}
