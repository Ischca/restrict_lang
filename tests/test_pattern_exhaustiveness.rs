//! Comprehensive test suite for pattern matching exhaustiveness checking
//! 
//! Tests various scenarios including:
//! - Boolean pattern exhaustiveness
//! - Option pattern exhaustiveness
//! - List pattern exhaustiveness
//! - Record pattern exhaustiveness
//! - Nested pattern combinations
//! - Error message quality

#[cfg(test)]
mod tests {
    use restrict_lang::ast::*;
    use restrict_lang::parser::parse_program;
    use restrict_lang::type_checker::{TypeChecker, TypeError};

    /// Helper function to create a simple program with a match expression
    fn create_match_program(match_expr: &str) -> String {
        format!(
            r#"
            fun test(x) {{
                {}
            }}
            "#,
            match_expr
        )
    }

    /// Helper function to check if type checking fails with non-exhaustive pattern error
    fn expect_non_exhaustive_error(source: &str, expected_missing: &str) {
        let (_, program) = parse_program(source).expect("Parse should succeed");
        let mut checker = TypeChecker::new();
        
        match checker.check_program(&program) {
            Err(TypeError::NonExhaustivePatterns { missing, .. }) => {
                assert!(
                    missing.contains(expected_missing),
                    "Expected missing pattern '{}' but got: '{}'",
                    expected_missing,
                    missing
                );
            }
            Err(other_error) => {
                panic!("Expected NonExhaustivePatterns error but got: {:?}", other_error);
            }
            Ok(_) => {
                panic!("Expected exhaustiveness error but type checking succeeded");
            }
        }
    }

    /// Helper function to check if type checking succeeds (patterns are exhaustive)
    fn expect_exhaustive(source: &str) {
        let (_, program) = parse_program(source).expect("Parse should succeed");
        let mut checker = TypeChecker::new();
        
        match checker.check_program(&program) {
            Ok(_) => {
                // Success - patterns are exhaustive
            }
            Err(TypeError::NonExhaustivePatterns { missing, .. }) => {
                panic!("Expected exhaustive patterns but got missing: {}", missing);
            }
            Err(other_error) => {
                // Other errors are fine - we're only testing exhaustiveness
            }
        }
    }

    #[test]
    fn test_boolean_exhaustive_both_cases() {
        let source = create_match_program(
            r#"
            x |> match {
                true => 1,
                false => 0
            }
            "#
        );
        expect_exhaustive(&source);
    }

    #[test]
    fn test_boolean_non_exhaustive_missing_false() {
        let source = create_match_program(
            r#"
            x |> match {
                true => 1
            }
            "#
        );
        expect_non_exhaustive_error(&source, "false");
    }

    #[test]
    fn test_boolean_non_exhaustive_missing_true() {
        let source = create_match_program(
            r#"
            x |> match {
                false => 0
            }
            "#
        );
        expect_non_exhaustive_error(&source, "true");
    }

    #[test]
    fn test_boolean_exhaustive_with_wildcard() {
        let source = create_match_program(
            r#"
            x |> match {
                true => 1,
                _ => 0
            }
            "#
        );
        expect_exhaustive(&source);
    }

    #[test]
    fn test_option_exhaustive_both_cases() {
        let source = create_match_program(
            r#"
            x |> match {
                Some(value) => value,
                None => 0
            }
            "#
        );
        expect_exhaustive(&source);
    }

    #[test]
    fn test_option_non_exhaustive_missing_none() {
        let source = create_match_program(
            r#"
            x |> match {
                Some(value) => value
            }
            "#
        );
        expect_non_exhaustive_error(&source, "None");
    }

    #[test]
    fn test_option_non_exhaustive_missing_some() {
        let source = create_match_program(
            r#"
            x |> match {
                None => 0
            }
            "#
        );
        expect_non_exhaustive_error(&source, "Some(_)");
    }

    #[test]
    fn test_nested_option_exhaustive() {
        let source = create_match_program(
            r#"
            x |> match {
                Some(Some(value)) => value,
                Some(None) => 0,
                None => -1
            }
            "#
        );
        expect_exhaustive(&source);
    }

    #[test]
    fn test_nested_option_non_exhaustive_missing_inner_none() {
        let source = create_match_program(
            r#"
            x |> match {
                Some(Some(value)) => value,
                None => -1
            }
            "#
        );
        expect_non_exhaustive_error(&source, "Some(None)");
    }

    #[test]
    fn test_nested_option_boolean_exhaustive() {
        let source = create_match_program(
            r#"
            x |> match {
                Some(true) => 1,
                Some(false) => 0,
                None => -1
            }
            "#
        );
        expect_exhaustive(&source);
    }

    #[test]
    fn test_nested_option_boolean_non_exhaustive() {
        let source = create_match_program(
            r#"
            x |> match {
                Some(true) => 1,
                None => -1
            }
            "#
        );
        expect_non_exhaustive_error(&source, "Some(false)");
    }

    #[test]
    fn test_list_exhaustive_empty_and_cons() {
        let source = create_match_program(
            r#"
            x |> match {
                [] => 0,
                [head | tail] => head
            }
            "#
        );
        expect_exhaustive(&source);
    }

    #[test]
    fn test_list_non_exhaustive_missing_empty() {
        let source = create_match_program(
            r#"
            x |> match {
                [head | tail] => head
            }
            "#
        );
        expect_non_exhaustive_error(&source, "[]");
    }

    #[test]
    fn test_list_non_exhaustive_missing_cons() {
        let source = create_match_program(
            r#"
            x |> match {
                [] => 0
            }
            "#
        );
        expect_non_exhaustive_error(&source, "[_|_]");
    }

    #[test]
    fn test_list_exact_patterns_need_cons() {
        let source = create_match_program(
            r#"
            x |> match {
                [] => 0,
                [a] => a,
                [a, b] => a + b
            }
            "#
        );
        // Exact patterns can't cover all possible list lengths
        expect_non_exhaustive_error(&source, "cons pattern needed");
    }

    #[test]
    fn test_unit_exhaustive() {
        let source = create_match_program(
            r#"
            x |> match {
                () => 42
            }
            "#
        );
        expect_exhaustive(&source);
    }

    #[test]
    fn test_unit_non_exhaustive() {
        let source = create_match_program(
            r#"
            x |> match {
                // No patterns - should require ()
            }
            "#
        );
        expect_non_exhaustive_error(&source, "()");
    }

    #[test]
    fn test_record_patterns_basic() {
        let source = r#"
            record Point {
                x: Int32,
                y: Int32
            }
            
            fun test(p: Point) {
                p |> match {
                    Point { x, y } => x + y
                }
            }
        "#;
        expect_exhaustive(&source);
    }

    #[test]
    fn test_record_patterns_missing() {
        let source = r#"
            record Point {
                x: Int32,
                y: Int32
            }
            
            fun test(p: Point) {
                p |> match {
                    // No patterns - should require Point pattern
                }
            }
        "#;
        expect_non_exhaustive_error(&source, "Point{ .. }");
    }

    #[test]
    fn test_infinite_types_require_wildcard() {
        let source = create_match_program(
            r#"
            x |> match {
                42 => "answer",
                0 => "zero"
            }
            "#
        );
        expect_non_exhaustive_error(&source, "pattern required for infinite type");
    }

    #[test]
    fn test_infinite_types_with_wildcard() {
        let source = create_match_program(
            r#"
            x |> match {
                42 => "answer",
                0 => "zero",
                _ => "other"
            }
            "#
        );
        expect_exhaustive(&source);
    }

    #[test]
    fn test_wildcard_makes_everything_exhaustive() {
        let source = create_match_program(
            r#"
            x |> match {
                _ => "anything"
            }
            "#
        );
        expect_exhaustive(&source);
    }

    #[test]
    fn test_identifier_pattern_makes_everything_exhaustive() {
        let source = create_match_program(
            r#"
            x |> match {
                value => value
            }
            "#
        );
        expect_exhaustive(&source);
    }

    #[test]
    fn test_complex_nested_option_list() {
        let source = create_match_program(
            r#"
            x |> match {
                Some([]) => 0,
                Some([head | tail]) => head,
                None => -1
            }
            "#
        );
        expect_exhaustive(&source);
    }

    #[test]
    fn test_complex_nested_option_list_non_exhaustive() {
        let source = create_match_program(
            r#"
            x |> match {
                Some([head | tail]) => head,
                None => -1
            }
            "#
        );
        expect_non_exhaustive_error(&source, "Some([])");
    }

    #[test]
    fn test_error_message_includes_suggestion() {
        let source = create_match_program(
            r#"
            x |> match {
                true => 1
            }
            "#
        );
        
        let (_, program) = parse_program(&source).expect("Parse should succeed");
        let mut checker = TypeChecker::new();
        
        match checker.check_program(&program) {
            Err(TypeError::NonExhaustivePatterns { suggestion, .. }) => {
                assert!(
                    suggestion.contains("Add the missing patterns or use a wildcard pattern"),
                    "Suggestion should be helpful: '{}'",
                    suggestion
                );
            }
            other => {
                panic!("Expected NonExhaustivePatterns error with suggestion but got: {:?}", other);
            }
        }
    }
}