use restrict_lang::{parse_program, TypeChecker};

/// Edge case tests for Temporal Affine Types (TAT)
/// These tests cover corner cases and error scenarios not covered in other test files

#[test]
fn test_temporal_escape_through_closure() {
    // Test that temporal values cannot escape through closures
    let input = r#"record File<~f> {
    handle: Int32,
    path: String
}

fun leakThroughClosure: () -> (File<~io> -> Int32) = {
    with lifetime<~io> {
        val file = File { handle = 1, path: "test.txt" };
        // This should fail - returning a closure that captures temporal value
        fun(f: File<~io>) -> Int32 = { f.handle }
    }
}

fun main: () -> Unit = {
    Unit
}"#;
    
    let result = parse_program(input);
    match result {
        Ok((remaining, program)) => {
            if !remaining.trim().is_empty() {
                panic!("Parse error: unparsed input remains: {}", remaining);
            }
            let mut checker = TypeChecker::new();
            match checker.check_program(&program) {
                Ok(_) => panic!("Expected type error for temporal escape through closure"),
                Err(e) => {
                    println!("Got expected error: {:?}", e);
                }
            }
        }
        Err(e) => panic!("Parse error: {:?}", e)
    }
}

#[test]
fn test_temporal_in_recursive_types() {
    // Test temporal types in recursive data structures
    let input = r#"
    record Node<T, ~n> {
        value: T,
        next: Option<Node<T, ~n>>
    }
    
    fun main: () -> Unit = {
        with lifetime<~list> {
            val node3 = Node { value: 3, next: None };
            val node2 = Node { value: 2, next: Some(node3) };
            val node1 = Node { value: 1, next: Some(node2) };
            match node1.next {
                Some(n) => n.value,
                None => 0
            };
            Unit
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
fn test_temporal_constraint_transitivity_violation() {
    // Test that transitive constraints are properly enforced
    let input = r#"
    record A<~a> {
        id: Int32
    }
    
    record B<~b, ~a> where ~b within ~a {
        a: A<~a>
    }
    
    record C<~c, ~b, ~a> where ~c within ~b, ~b within ~a {
        b: B<~b, ~a>
    }
    
    fun main: () -> Unit = {
        with lifetime<~x> {
            with lifetime<~y> where ~y within ~x {
                with lifetime<~z> {
                    // This should fail - ~z is not within ~y
                    val a = A { id: 1 };
                    val b = B { a: a };
                    val c = C { b: b };
                    Unit
                }
            }
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => panic!("Expected type error for constraint transitivity violation"),
        Err(e) => {
            println!("Got expected error: {:?}", e);
        }
    }
}

#[test]
fn test_temporal_multiple_constraints() {
    // Test records with multiple temporal constraints
    let input = r#"
    record MultiConstraint<~a, ~b, ~c> 
    where ~a within ~b, ~a within ~c {
        dataB: Int32,
        dataC: Int32
    }
    
    fun main: () -> Unit = {
        with lifetime<~parent1> {
            with lifetime<~parent2> {
                with lifetime<~child> where ~child within ~parent1, ~child within ~parent2 {
                    val mc = MultiConstraint { dataB: 1, dataC: 2 };
                    mc.dataB + mc.dataC;
                    Unit
                }
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

#[test]
fn test_temporal_in_match_patterns() {
    // Test temporal types in pattern matching
    let input = r#"
    record Result<T, E, ~r> {
        variant: Int32,  // 0 for Ok, 1 for Err
        ok: Option<T>,
        err: Option<E>
    }
    
    fun processResult: <T, ~r>(result: Result<T, String, ~r>) -> T = {
        match result.variant {
            0 => match result.ok {
                Some(val) => val,
                None => panic("Invalid Result state")
            },
            1 => panic("Error occurred"),
            _ => panic("Invalid variant")
        }
    }
    
    fun main: () -> Unit = {
        with lifetime<~op> {
            val result = Result { 
                variant: 0, 
                ok: Some(42), 
                err: None 
            };
            val value = processResult(result);
            Unit
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
fn test_temporal_function_parameter_inference() {
    // Test that temporal parameters are correctly inferred in function calls
    let input = r#"
    record Channel<T, ~ch> {
        id: Int32,
        buffer: T
    }
    
    fun send: <T, ~ch>(channel: Channel<T, ~ch>, msg: T) -> Unit = {
        // In real implementation, this would send the message
        Unit
    }
    
    fun receive: <T, ~ch>(channel: Channel<T, ~ch>) -> T = {
        channel.buffer
    }
    
    fun main: () -> Unit = {
        with lifetime<~comm> {
            val ch = Channel { id: 1, buffer: "Hello" };
            send(ch, "World");
            val msg = receive(ch);
            Unit
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
fn test_temporal_affine_double_use() {
    // Test that temporal values still follow affine rules
    let input = r#"
    record Secret<~s> {
        value: String,
        consumed: Bool
    }
    
    fun consume: <~s>(secret: Secret<~s>) -> String = {
        secret.value
    }
    
    fun main: () -> Unit = {
        with lifetime<~secure> {
            val secret = Secret { value: "password123", consumed: false };
            val v1 = consume(secret);
            // This should fail - secret already consumed
            val v2 = consume(secret);
            Unit
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => panic!("Expected affine violation for double use of temporal value"),
        Err(e) => {
            assert!(e.to_string().contains("already been used") || 
                    e.to_string().contains("affine"));
            println!("Got expected error: {:?}", e);
        }
    }
}

#[test]
fn test_temporal_partial_application() {
    // Test temporal types with partial function application
    let input = r#"
    record Logger<~log> {
        name: String,
        level: Int32
    }
    
    fun logMessage: <~log>(logger: Logger<~log>, level: Int32, msg: String) -> Unit = {
        Unit
    }
    
    fun main: () -> Unit = {
        with lifetime<~logging> {
            val logger = Logger { name: "app", level: 1 };
            // Partial application should preserve temporal constraints
            val logInfo = logMessage(logger, 2);
            logInfo("Starting application");
            Unit
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
fn test_temporal_cyclic_constraint() {
    // Test that cyclic temporal constraints are rejected
    let input = r#"
    record Cycle<~a, ~b> where ~a within ~b, ~b within ~a {
        data: Int32
    }
    
    fun main: () -> Unit = {
        Unit
    }"#;
    
    let result = parse_program(input);
    match result {
        Ok((_, program)) => {
            let mut checker = TypeChecker::new();
            match checker.check_program(&program) {
                Ok(_) => panic!("Expected error for cyclic temporal constraint"),
                Err(e) => {
                    println!("Got expected error: {:?}", e);
                }
            }
        }
        Err(e) => {
            // Could also fail at parse time
            println!("Parse error (expected): {:?}", e);
        }
    }
}

#[test]
fn test_temporal_with_context_interaction() {
    // Test interaction between temporal scopes and context
    let input = r#"
    context Database<~db> {
        query: String -> String
    }
    
    record Connection<~conn, ~db> where ~conn within ~db {
        id: Int32
    }
    
    fun main: () -> Unit = {
        with lifetime<~db> {
            with Database {
                with lifetime<~conn> where ~conn within ~db {
                    val conn = Connection { id: 1 };
                    val result = "SELECT * FROM users" |> query;
                    Unit
                }
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

#[test]
fn test_temporal_empty_scope() {
    // Test empty temporal scope behavior
    let input = r#"
    fun main: () -> Unit = {
        with lifetime<~empty> {
            // Empty scope should still work
        };
        Unit
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}

#[test]
fn test_temporal_shadowing() {
    // Test temporal variable shadowing
    let input = r#"
    record Item<~i> {
        id: Int32
    }
    
    fun main: () -> Unit = {
        with lifetime<~scope> {
            val item = Item { id: 1 };
            {
                // Inner scope with same temporal variable
                with lifetime<~scope> {
                    val item = Item { id: 2 };
                    item.id;
                    Unit
                };
                Unit
            };
            item.id;
            Unit
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}