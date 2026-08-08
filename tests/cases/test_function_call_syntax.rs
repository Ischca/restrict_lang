//! Test that traditional function call syntax is NOT supported
//! Only OSV (Object-Subject-Verb) syntax should be accepted

#[cfg(test)]
mod tests {
    use restrict_lang::{parse_program, TypeChecker};

    fn parse_complete(source: &str) -> Result<(), String> {
        match parse_program(source) {
            Ok((remaining, _)) if remaining.trim().is_empty() => Ok(()),
            Ok((remaining, _)) => Err(format!("unparsed input remaining: {remaining:?}")),
            Err(err) => Err(format!("parse error: {err:?}")),
        }
    }

    fn assert_parse_rejected(source: &str, message: &str) {
        assert!(parse_complete(source).is_err(), "{message}");
    }

    fn assert_parse_accepted(source: &str, message: &str) {
        parse_complete(source).expect(message);
    }

    fn type_check_complete(source: &str) -> Result<(), String> {
        let (remaining, program) =
            parse_program(source).map_err(|err| format!("parse error: {err:?}"))?;
        if !remaining.trim().is_empty() {
            return Err(format!("unparsed input remaining: {remaining:?}"));
        }

        let mut checker = TypeChecker::new();
        checker
            .check_program(&program)
            .map_err(|err| format!("type error: {err}"))
    }

    #[test]
    fn test_traditional_function_call_rejected() {
        // Traditional function call syntax should NOT parse
        let traditional_syntax = r#"
            fun add: (x: Int32, y: Int32) -> Int32 = {
                x + y
            }

            fun main: () = {
                val result = add(5, 10);  // This should FAIL
                result
            }
        "#;

        assert_parse_rejected(
            traditional_syntax,
            "Traditional function call syntax should be rejected",
        );
    }

    #[test]
    fn test_osv_function_call_accepted() {
        // OSV function call syntax should parse correctly
        let osv_syntax = r#"
            fun add: (x: Int32, y: Int32) -> Int32 = {
                x + y
            }

            fun main: () = {
                val result = (5, 10) add;  // OSV syntax - should work
                result
            }
        "#;

        assert_parse_accepted(osv_syntax, "OSV function call syntax should be accepted");
    }

    #[test]
    fn test_pipe_operator_accepted() {
        // Pipe operator should work
        let pipe_syntax = r#"
            fun double: (x: Int32) -> Int32 = {
                x * 2
            }

            fun main: () = {
                val result = 5 |> double;  // Pipe operator - should work
                result
            }
        "#;

        assert_parse_accepted(pipe_syntax, "Pipe operator syntax should be accepted");
    }

    #[test]
    fn test_no_args_function_call() {
        // No-argument function calls
        let no_args_osv = r#"
            fun get_value: () -> Int32 = {
                42
            }

            fun main: () = {
                val result = () get_value;  // OSV with no args
                result
            }
        "#;

        assert_parse_accepted(
            no_args_osv,
            "No-argument OSV function call should be accepted",
        );

        // Traditional no-args syntax should fail
        let no_args_traditional = r#"
            fun get_value: () -> Int32 = {
                42
            }

            fun main: () = {
                val result = get_value();  // Traditional - should FAIL
                result
            }
        "#;

        assert_parse_rejected(
            no_args_traditional,
            "Traditional no-args function call should be rejected",
        );
    }

    #[test]
    fn bare_zero_arg_function_name_is_not_an_implicit_call() {
        let bare_zero_arg = r#"
            fun get_value: () -> Int32 = {
                42
            }

            fun main: () -> Int32 = {
                get_value
            }
        "#;

        let err = type_check_complete(bare_zero_arg)
            .expect_err("bare zero-arg function names should not implicitly call");
        assert!(
            err.contains("zero-argument function 'get_value'") && err.contains("() get_value"),
            "error should point users to the OSV unit call form, got: {err}"
        );

        let osv_zero_arg = r#"
            fun get_value: () -> Int32 = {
                42
            }

            fun main: () -> Int32 = {
                () get_value
            }
        "#;

        type_check_complete(osv_zero_arg).expect("explicit OSV unit call should remain accepted");
    }

    #[test]
    fn test_traditional_calls_with_whitespace_rejected() {
        let with_spaced_args = r#"
            fun add: (x: Int32, y: Int32) -> Int32 = {
                x + y
            }

            fun main: () -> Int32 = {
                val result = add (5, 10)
                result
            }
        "#;

        assert_parse_rejected(
            with_spaced_args,
            "Traditional calls with whitespace before arguments should be rejected",
        );

        let with_spaced_unit_args = r#"
            fun get_value: () -> Int32 = {
                42
            }

            fun main: () -> Int32 = {
                val result = get_value ()
                result
            }
        "#;

        assert_parse_rejected(
            with_spaced_unit_args,
            "Traditional unit calls with whitespace should be rejected",
        );

        let with_spaced_field_call = r#"
            fun main: () -> Int32 = {
                val data = 1
                val result = data.get_value ()
                result
            }
        "#;

        assert_parse_rejected(
            with_spaced_field_call,
            "Traditional field calls with whitespace should be rejected",
        );

        let nested_spaced_calls = r#"
            fun add: (x: Int32, y: Int32) -> Int32 = { x + y }
            fun double: (x: Int32) -> Int32 = { x * 2 }

            fun main: () -> Int32 = {
                val result = double (add (5, 10))
                result
            }
        "#;

        assert_parse_rejected(
            nested_spaced_calls,
            "Nested traditional calls with whitespace should be rejected",
        );
    }

    #[test]
    fn test_print_function_calls() {
        // Test print functions use OSV syntax
        let print_osv = r#"
            fun main: () = {
                "Hello World" |> println;         // Pipe - should work
                ("Message: ", 42) print;          // OSV - should work
                42 |> print_int;                  // Pipe - should work
            }
        "#;

        assert_parse_accepted(print_osv, "Print functions with OSV syntax should work");

        // Traditional print calls should fail
        let print_traditional = r#"
            fun main: () = {
                println("Hello World");  // Traditional - should FAIL
            }
        "#;

        assert_parse_rejected(
            print_traditional,
            "Traditional print function call should be rejected",
        );
    }

    #[test]
    fn test_nested_function_calls() {
        // Nested OSV calls
        let nested_osv = r#"
            fun add: (x: Int32, y: Int32) -> Int32 = { x + y }
            fun double: (x: Int32) -> Int32 = { x * 2 }

            fun main: () = {
                val result = (5, 10) add |> double;  // Chained with pipe
                val result2 = ((2, 3) add) double;   // Nested OSV
                result + result2
            }
        "#;

        assert_parse_accepted(nested_osv, "Nested OSV function calls should work");

        // Nested traditional calls should fail
        let nested_traditional = r#"
            fun add: (x: Int32, y: Int32) -> Int32 = { x + y }
            fun double: (x: Int32) -> Int32 = { x * 2 }

            fun main: () = {
                val result = double(add(5, 10));  // Traditional nested - should FAIL
                result
            }
        "#;

        assert_parse_rejected(
            nested_traditional,
            "Nested traditional function calls should be rejected",
        );
    }
}
