use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

fn compile_to_wat(source: &str) -> Result<String, String> {
    let (_, ast) = parse_program(source)
        .map_err(|e| format!("Parse error: {:?}", e))?;
    let mut type_checker = TypeChecker::new();
    type_checker.check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))?;
    let mut codegen = WasmCodeGen::new();
    codegen.generate(&ast)
        .map_err(|e| format!("Codegen error: {}", e))
}

#[test]
fn test_tuple_literal_parsing() {
    let source = r#"
        fun add: (a: Int, b: Int) -> Int = {
            a + b
        }

        fun main: () -> Int = {
            (5, 3) |> add
        }
    "#;

    let wat = compile_to_wat(source).unwrap();

    // Tuple should be auto-expanded: elements pushed directly, not as heap allocation
    assert!(wat.contains("i32.const 5"));
    assert!(wat.contains("i32.const 3"));
    assert!(wat.contains("call $add"));
    // Should NOT contain tuple allocation for this pipe
    assert!(!wat.contains("Tuple literal"));
}

#[test]
fn test_single_arg_pipe_still_works() {
    let source = r#"
        fun double: (n: Int) -> Int = {
            n + n
        }

        fun main: () -> Int = {
            42 |> double
        }
    "#;

    let wat = compile_to_wat(source).unwrap();
    assert!(wat.contains("i32.const 42"));
    assert!(wat.contains("call $double"));
}

#[test]
fn test_tuple_in_binding() {
    // Tuple as a binding value should allocate on heap
    let source = r#"
        fun add: (a: Int, b: Int) -> Int = {
            a + b
        }

        fun main: () -> Int = {
            val result = (10, 20) |> add
            result
        }
    "#;

    let wat = compile_to_wat(source).unwrap();
    // Auto-expanded, so elements should be pushed directly
    assert!(wat.contains("i32.const 10"));
    assert!(wat.contains("i32.const 20"));
    assert!(wat.contains("call $add"));
}

#[test]
fn test_pipe_chain() {
    let source = r#"
        fun double: (n: Int) -> Int = {
            n + n
        }

        fun main: () -> Int = {
            5 |> double |> double
        }
    "#;

    let wat = compile_to_wat(source).unwrap();
    // Should call double twice
    let double_count = wat.matches("call $double").count();
    // At least 2 calls to $double in the main function
    assert!(double_count >= 2, "Expected at least 2 calls to $double, found {}", double_count);
}
