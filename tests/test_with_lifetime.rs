use restrict_lang::{parse_program, TypeChecker};

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_with_lifetime_basic() {
    // Basic with lifetime block
    let input = r#"
    record File<~f> {
        handle: Int32
    }
    
    fun main: () -> Int = {
        with lifetime<~f> {
            val file = File { handle = 1 };
            file.handle
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_with_lifetime_anonymous() {
    // Anonymous lifetime
    let input = r#"
    record Resource<~r> {
        id: Int32
    }
    
    fun main: () -> Int = {
        with lifetime {
            val res = Resource { id = 42 };
            res.id
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_with_lifetime_escape_error() {
    // Should error when trying to return temporal value
    let input = r#"
    record File<~f> {
        handle: Int32
    }
    
    fun main: () -> Int = {
        with lifetime<~f> {
            val file = File { handle = 1 };
            file  // ERROR: Cannot return File<~f> outside lifetime scope
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => panic!("Expected temporal escape error"),
        Err(e) => {
            assert!(matches!(e, restrict_lang::TypeError::TemporalEscape { .. }));
            println!("Got expected error: {:?}", e);
        }
    }
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_nested_with_lifetime() {
    // Nested lifetime scopes
    let input = r#"
    record Outer<~out> {
        id: Int32
    }
    
    record Inner<~in, ~out> where ~in within ~out {
        outer: Outer<~out>
        data: Int32
    }
    
    fun main: () -> Int = {
        with lifetime<~out> {
            val outer = Outer { id = 1 };
            with lifetime<~in> {
                val inner = Inner { outer = outer, data = 42 };
                inner.data
            }
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}