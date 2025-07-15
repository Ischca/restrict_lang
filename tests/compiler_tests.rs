use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

fn compile_to_wat(source: &str) -> Result<String, String> {
    // Parse
    let (_, ast) = parse_program(source)
        .map_err(|e| format!("Parse error: {:?}", e))?;
    
    // Type check
    let mut type_checker = TypeChecker::new();
    type_checker.check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))?;
    
    // Generate WASM
    let mut codegen = WasmCodeGen::new();
    codegen.generate(&ast)
        .map_err(|e| format!("Codegen error: {}", e))
}

#[test]
fn test_arithmetic_wat_generation() {
    let source = r#"
        fun main = {
            10 + 20
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Verify WAT contains expected instructions
    assert!(wat.contains("(func $main")); // main doesn't return a value
    assert!(wat.contains("i32.const 10"));
    assert!(wat.contains("i32.const 20"));
    assert!(wat.contains("i32.add"));
    assert!(wat.contains("(export \"_start\" (func $main))"));
}

#[test]
fn test_function_call_wat_generation() {
    let source = r#"
        fun double = x: Int {
            x * 2
        }
        
        fun main = {
            21 |> double
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Verify function definitions
    assert!(wat.contains("(func $double (param $x i32) (result i32)"));
    assert!(wat.contains("(func $main"));
    
    // Verify double function body
    assert!(wat.contains("local.get $x"));
    assert!(wat.contains("i32.const 2"));
    assert!(wat.contains("i32.mul"));
    
    // Verify main calls double
    assert!(wat.contains("i32.const 21"));
    assert!(wat.contains("call $double"));
}

#[test]
fn test_local_variables_wat_generation() {
    let source = r#"
        fun main = {
            val a = 100;
            val b = 50;
            val result = a - b;
            result
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Verify local declarations
    assert!(wat.contains("(local $a i32)"));
    assert!(wat.contains("(local $b i32)"));
    assert!(wat.contains("(local $result i32)"));
    
    // Verify assignments
    assert!(wat.contains("i32.const 100"));
    assert!(wat.contains("local.set $a"));
    assert!(wat.contains("i32.const 50"));
    assert!(wat.contains("local.set $b"));
    
    // Verify subtraction
    assert!(wat.contains("local.get $a"));
    assert!(wat.contains("local.get $b"));
    assert!(wat.contains("i32.sub"));
    assert!(wat.contains("local.set $result"));
    assert!(wat.contains("local.get $result"));
}

#[test]
fn test_conditional_wat_generation() {
    let source = r#"
        fun is_positive = x: Int {
            x > 0 then {
                1
            } else {
                0
            }
        }
        
        fun main = {
            42 |> is_positive
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Verify conditional structure
    assert!(wat.contains("(if (result i32)"));
    assert!(wat.contains("(then"));
    assert!(wat.contains("(else"));
    
    // Verify comparison
    assert!(wat.contains("i32.gt_s"));
    
    // Verify then/else return values
    assert!(wat.contains("i32.const 1"));
    assert!(wat.contains("i32.const 0"));
}

#[test]
fn test_pipe_operator_wat_generation() {
    let source = r#"
        fun inc = x: Int {
            x + 1
        }
        
        fun main = {
            42 |> inc
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Verify pipe translates to function call
    assert!(wat.contains("i32.const 42"));
    assert!(wat.contains("call $inc"));
}

#[test]
fn test_all_binary_operators() {
    let source = r#"
        fun test_ops = {
            val add = 10 + 3;
            val sub = 10 - 3;
            val mul = 10 * 3;
            val div = 10 / 3;
            val mod = 10 % 3;
            val eq = 10 == 3;
            val ne = 10 != 3;
            val lt = 10 < 3;
            val le = 10 <= 3;
            val gt = 10 > 3;
            val ge = 10 >= 3;
            42
        }
        
        fun main = {
            test_ops
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Verify all operators
    assert!(wat.contains("i32.add"));
    assert!(wat.contains("i32.sub"));
    assert!(wat.contains("i32.mul"));
    assert!(wat.contains("i32.div_s"));
    assert!(wat.contains("i32.rem_s"));
    assert!(wat.contains("i32.eq"));
    assert!(wat.contains("i32.ne"));
    assert!(wat.contains("i32.lt_s"));
    assert!(wat.contains("i32.le_s"));
    assert!(wat.contains("i32.gt_s"));
    assert!(wat.contains("i32.ge_s"));
}

#[test]
fn test_multiple_locals() {
    let source = r#"
        fun main = {
            val a = 10;
            val b = 5;
            val sum = a + b;
            val result = sum * 2;
            result
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Verify multiple locals
    assert!(wat.contains("(local $a i32)"));
    assert!(wat.contains("(local $b i32)"));
    assert!(wat.contains("(local $sum i32)"));
}

#[test]
fn test_multiple_parameters() {
    let source = r#"
        fun add3 = a: Int b: Int c: Int {
            a + b + c
        }
        
        fun main = {
            val result = (10, 20, 30) add3
            result
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Verify function with 3 parameters
    assert!(wat.contains("(func $add3 (param $a i32) (param $b i32) (param $c i32) (result i32)"));
    
    // Verify call with 3 arguments
    assert!(wat.contains("i32.const 10"));
    assert!(wat.contains("i32.const 20"));
    assert!(wat.contains("i32.const 30"));
    assert!(wat.contains("call $add3"));
}