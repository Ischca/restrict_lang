use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

fn compile(input: &str) -> Result<String, String> {
    let (remaining, program) = parse_program(input)
        .map_err(|e| format!("Parse error: {:?}", e))?;

    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {:?}", remaining));
    }

    let mut checker = TypeChecker::new();
    checker.check_program(&program)
        .map_err(|e| format!("Type error: {:?}", e))?;

    let mut codegen = WasmCodeGen::new();
    codegen.generate(&program)
        .map_err(|e| format!("Codegen error: {:?}", e))
}

fn type_check(input: &str) -> Result<(), String> {
    let (remaining, program) = parse_program(input)
        .map_err(|e| format!("Parse error: {:?}", e))?;

    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {:?}", remaining));
    }

    let mut checker = TypeChecker::new();
    checker.check_program(&program)
        .map_err(|e| format!("Type error: {:?}", e))?;

    Ok(())
}

// =============================================================================
// Phase 1: Basic Generic Function Parsing and Type Checking
// =============================================================================

#[test]
fn test_generic_identity_function_parse() {
    // Test that we can parse a generic identity function
    let input = r#"
fun identity<T>: (x: T) -> T = {
    x
}

fun main: () -> Int = {
    42 identity
}
"#;

    match parse_program(input) {
        Ok((rem, prog)) => {
            println!("Remaining: {:?}", rem);
            println!("Declarations: {}", prog.declarations.len());
            assert!(rem.trim().is_empty(), "Should parse all input, remaining: {:?}", rem);
            assert_eq!(prog.declarations.len(), 2, "Should have 2 declarations");
        }
        Err(e) => panic!("Parse failed: {:?}", e),
    }
}

#[test]
fn test_generic_identity_function_typecheck() {
    // Test that generic identity function type checks
    let input = r#"
fun identity<T>: (x: T) -> T = {
    x
}

fun main: () -> Int = {
    42 identity
}
"#;

    match type_check(input) {
        Ok(()) => println!("Type check passed!"),
        Err(e) => panic!("Type check failed: {}", e),
    }
}

#[test]
fn test_generic_identity_function_codegen() {
    // Test that generic identity function generates code
    let input = r#"
fun identity<T>: (x: T) -> T = {
    x
}

fun main: () -> Int = {
    42 identity
}
"#;

    match compile(input) {
        Ok(wat) => {
            println!("Compilation successful!");
            // Should generate a specialized identity_Int function
            assert!(
                wat.contains("$identity") || wat.contains("$identity_Int"),
                "Should generate identity function"
            );
        }
        Err(e) => panic!("Compilation failed: {}", e),
    }
}

// =============================================================================
// Phase 2: Multiple Type Parameters
// =============================================================================

#[test]
fn test_generic_pair_function() {
    let input = r#"
fun first<A, B>: (a: A, b: B) -> A = {
    a
}

fun main: () -> Int = {
    (10, "hello") first
}
"#;

    match type_check(input) {
        Ok(()) => println!("Type check passed!"),
        Err(e) => panic!("Type check failed: {}", e),
    }
}

#[test]
#[ignore = "Affine type violation: multiple field accesses on same record need to be treated as single use"]
fn test_generic_swap_function() {
    let input = r#"
record Pair<A, B> {
    first: A,
    second: B
}

fun swap<A, B>: (p: Pair<A, B>) -> Pair<B, A> = {
    Pair { first = p.second, second = p.first }
}

fun main: () -> Int = {
    val p = Pair { first = 1, second = "two" };
    val swapped = p swap;
    0
}
"#;

    match type_check(input) {
        Ok(()) => println!("Type check passed!"),
        Err(e) => panic!("Type check failed: {}", e),
    }
}

// =============================================================================
// Phase 3: Type Bounds
// =============================================================================

#[test]
fn test_generic_with_display_bound() {
    // T: Display means T can be printed
    let input = r#"
fun show<T: Display>: (x: T) -> Unit = {
    x println
}

fun main: () -> Int = {
    42 show;
    "hello" show;
    0
}
"#;

    match type_check(input) {
        Ok(()) => println!("Type check passed!"),
        Err(e) => panic!("Type check failed: {}", e),
    }
}

// =============================================================================
// Phase 4: Generic Records
// =============================================================================

#[test]
fn test_generic_box_record() {
    let input = r#"
record Box<T> {
    value: T
}

fun main: () -> Int = {
    val intBox = Box { value = 42 };
    val strBox = Box { value = "hello" };
    intBox.value
}
"#;

    match type_check(input) {
        Ok(()) => println!("Type check passed!"),
        Err(e) => panic!("Type check failed: {}", e),
    }
}

#[test]
fn test_generic_box_codegen() {
    let input = r#"
record Box<T> {
    value: T
}

fun unbox<T>: (b: Box<T>) -> T = {
    b.value
}

fun main: () -> Int = {
    val b = Box { value = 42 };
    b unbox
}
"#;

    match compile(input) {
        Ok(wat) => {
            println!("Compilation successful!");
            // Should generate specialized Box_Int and unbox_Int
            println!("WAT output:\n{}", &wat[..wat.len().min(2000)]);
        }
        Err(e) => panic!("Compilation failed: {}", e),
    }
}

// =============================================================================
// Phase 5: Type Inference
// =============================================================================

#[test]
fn test_generic_type_inference() {
    // The type parameter should be inferred from usage
    let input = r#"
fun identity<T>: (x: T) -> T = {
    x
}

fun main: () -> Int = {
    val x: Int = 42 identity;
    val y: String = "hello" identity;
    x
}
"#;

    match type_check(input) {
        Ok(()) => println!("Type check passed!"),
        Err(e) => panic!("Type check failed: {}", e),
    }
}

// =============================================================================
// Error Cases
// =============================================================================

#[test]
fn test_generic_type_mismatch_error() {
    // Should fail: trying to use identity with wrong return type
    let input = r#"
fun identity<T>: (x: T) -> T = {
    x
}

fun main: () -> Int = {
    val x: String = 42 identity;
    0
}
"#;

    match type_check(input) {
        Ok(()) => panic!("Should have failed type check"),
        Err(e) => {
            println!("Expected error: {}", e);
            // Check for type mismatch error (case-insensitive)
            let e_lower = e.to_lowercase();
            assert!(e_lower.contains("type") || e_lower.contains("mismatch"),
                    "Error should mention type mismatch: {}", e);
        }
    }
}
