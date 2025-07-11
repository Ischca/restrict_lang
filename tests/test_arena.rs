use restrict_lang::{parse_program, TypeChecker, generate};

fn compile(source: &str) -> Result<String, String> {
    // Parse
    let (_, ast) = parse_program(source)
        .map_err(|e| format!("Parse error: {:?}", e))?;
    
    // Type check
    let mut type_checker = TypeChecker::new();
    type_checker.check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))?;
    
    // Generate code
    generate(&ast)
        .map_err(|e| format!("Codegen error: {}", e))
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

// This test will be enabled once we implement list allocation
#[test]
#[ignore]
fn test_arena_list_allocation() {
    let source = r#"
        fun main = {
            with Arena {
                val nums = [1, 2, 3, 4, 5];
                val doubled = nums map (* 2);
                doubled
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