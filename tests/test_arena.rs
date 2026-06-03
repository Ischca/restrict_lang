use restrict_lang::{generate, parse_program, TypeChecker};

fn compile(source: &str) -> Result<String, String> {
    // Parse
    let (_, ast) = parse_program(source).map_err(|e| format!("Parse error: {:?}", e))?;

    // Type check
    let mut type_checker = TypeChecker::new();
    type_checker
        .check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))?;

    // Generate code
    generate(&ast).map_err(|e| format!("Codegen error: {}", e))
}

fn type_check(source: &str) -> Result<(), String> {
    let (remaining, ast) = parse_program(source).map_err(|e| format!("Parse error: {:?}", e))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {:?}", remaining));
    }

    let mut type_checker = TypeChecker::new();
    type_checker
        .check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))
}

fn assert_arena_escape_rejected(source: &str) {
    let err = type_check(source).expect_err("arena heap-backed result should be rejected");
    assert!(
        err.contains("Arena result cannot escape"),
        "error should explain arena result escape, got: {}",
        err
    );
}

#[test]
fn test_basic_arena() {
    let source = r#"
        fun main = {
            with Arena {
                // For now, just test that arena block compiles
                42
            }
        }
    "#;

    let result = compile(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
    let wat = result.unwrap();

    // Check that arena functions are generated
    assert!(wat.contains("$arena_init"));
    assert!(wat.contains("$arena_alloc"));
    assert!(wat.contains("$arena_reset"));

    // Check that arena is initialized and reset
    assert!(wat.contains("call $arena_init"));
    assert!(wat.contains("call $arena_reset"));
}

#[test]
fn test_nested_arena() {
    let source = r#"
        fun main = {
            with Arena {
                val x = 1;
                with Arena {
                    val y = 2;
                    x + y
                }
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok());
}

#[test]
fn test_arena_with_other_context_error() {
    let source = r#"
        fun main = {
            with NonExistentContext {
                42
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_err());
}

#[test]
fn test_arena_list_allocation() {
    let source = r#"
        fun main: () -> Int32 = {
            with Arena {
                val nums = [1, 2, 3, 4, 5];
                val doubled = (nums, |n| n * 2) map;
                doubled |> list_count
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok());
}

#[test]
fn test_arena_value_escape() {
    // Values created in arena should not escape the block
    // This is a semantic test - the type system should catch this
    let source = r#"
        fun main = {
            val x = with Arena {
                42  // This is fine - integers are copied
            };
            x
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok());
}

#[test]
fn test_arena_scalar_result_escape_allowed() {
    let cases = [
        ("Int32", "42"),
        ("Int64", "10000000000"),
        ("Float64", "3.5"),
        ("Boolean", "true"),
        ("Char", "'x'"),
        ("()", "()"),
    ];

    for (return_type, expr) in cases {
        let source = format!(
            r#"
                fun main: () -> {} = {{
                    val x = with Arena {{
                        {}
                    }};
                    x
                }}
            "#,
            return_type, expr
        );

        let result = type_check(&source);
        assert!(
            result.is_ok(),
            "arena scalar result {} should be allowed, got: {:?}",
            return_type,
            result
        );
    }
}

#[test]
fn test_arena_list_result_escape_rejected() {
    let source = r#"
        fun main: () -> List<Int32> = {
            with Arena {
                [1, 2, 3]
            }
        }
    "#;

    assert_arena_escape_rejected(source);
}

#[test]
fn test_arena_array_result_escape_rejected() {
    let source = r#"
        fun main: () -> Array<Int32, 3> = {
            with Arena {
                val values: Array<Int32, 3> = [1, 2, 3];
                values
            }
        }
    "#;

    assert_arena_escape_rejected(source);
}

#[test]
fn test_arena_string_result_escape_rejected() {
    let source = r#"
        fun main: () -> String = {
            with Arena {
                "arena text"
            }
        }
    "#;

    assert_arena_escape_rejected(source);
}
