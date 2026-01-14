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
#[ignore = "Type inference for local variables in return type deduction needs work"]
fn test_simple_mutable_binding() {
    let input = "fun test = { mut val x = 5; val result = x; result }";
    let wat = compile(input).unwrap();
    assert!(wat.contains("local.get"));
}

#[test]
#[ignore = "Type inference for local variables in return type deduction needs work"]
fn test_mutable_reassignment() {
    let input = r#"fun test = {
        mut val x = 5
        x = 10
        x
    }"#;
    let wat = compile(input).unwrap();
    assert!(wat.contains("local.set"));
}

#[test]
#[ignore = "Uses non-EBNF v1.0 syntax"]
fn test_immutable_reassignment_error() {
    let input = r#"fun test = {
    val x = 5;
    x = 10;
    x
}"#;
    let result = compile(input);
    if let Ok(wat) = &result {
        panic!("Expected error but got success. WAT: {}", wat);
    }
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("ImmutableReassignment") || err.contains("Cannot reassign to immutable"), 
            "Expected immutable reassignment error but got: {}", err);
}

#[test]
#[ignore = "Type inference for local variables in return type deduction needs work"]
fn test_mutable_with_arithmetic() {
    let input = r#"fun test = {
        mut val x = 5
        x = x + 1
        x
    }"#;
    let wat = compile(input).unwrap();
    assert!(wat.contains("i32.add"));
}

#[test]
#[ignore = "While loop code generation not implemented"]
fn test_while_with_mutable() {
    let input = r#"fun test = {
    mut val x = 0
    mut val sum = 0
    (x < 10) while {
        sum = sum + x
        x = x + 1
    }
    sum
}"#;
    let wat = compile(input).unwrap();
    // Check for loop structure (might have different label format)
    assert!(wat.contains("loop") || wat.contains("(loop"), 
            "Expected loop in WAT but got:\n{}", wat);
}

#[test]
fn test_mutable_parameter_reassignment() {
    let input = r#"fun add_one = x:Int {
        mut val y = x
        y = y + 1
        y
    }"#;
    let wat = compile(input).unwrap();
    assert!(wat.contains("local.get"));
}

#[test]
#[ignore = "Type inference for local variables in return type deduction needs work"]
fn test_multiple_reassignments() {
    let input = r#"fun test = {
        mut val x = 1
        x = 2
        x = 3
        x = 4
        x
    }"#;
    let wat = compile(input).unwrap();
    assert!(wat.contains("local.set"));
}

#[test]
#[ignore = "Type inference for local variables in return type deduction needs work"]
fn test_affine_with_mutable() {
    let input = r#"fun test = {
        val y = 5
        mut val x = y
        x = x + 1
        x
    }"#;
    let wat = compile(input).unwrap();
    // Should compile successfully
}

#[test]
#[ignore = "Type inference for local variables in return type deduction needs work"]
fn test_mutable_record_field() {
    let input = r#"
    record Point { x: Int32, y: Int32 }

    fun test = {
        with Arena {
            val p = Point { x = 10, y = 20 }
            mut val x = p.x
            x = x + 1
            x
        }
    }"#;
    let wat = compile(input).unwrap();
    // Should compile successfully
}