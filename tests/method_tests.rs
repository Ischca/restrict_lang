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
fn test_method_declaration() {
    let source = r#"
        record Point { x: Int y: Int }
        
        impl Point {
            fun distance = self: Point other: Point {
                0 + 0
            }
        }
        
        fun main = {
            42
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Verify method is generated with mangled name
    assert!(wat.contains("(func $Point_distance"));
    assert!(wat.contains("(param $self i32) (param $other i32)"));
}

#[test]
fn test_method_call() {
    let source = r#"
        record Counter { value: Int }
        
        impl Counter {
            fun increment = self: Counter amount: Int {
                self.value + amount
            }
        }
        
        fun main = {
            val result = (Counter { value = 10 }, 5) increment
            result
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Verify method is called with mangled name
    assert!(wat.contains("call $Counter_increment"));
}

#[test]
fn test_method_field_access() {
    let source = r#"
        record Player { hp: Int mp: Int }
        
        impl Player {
            fun is_alive = self: Player {
                self.hp > 0 then { 1 } else { 0 }
            }
        }
        
        fun main = {
            val alive = (Player { hp = 100, mp = 50 }) is_alive
            alive
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Verify field access in method
    assert!(wat.contains("i32.const 0")); // hp offset
    assert!(wat.contains("i32.load"));
    assert!(wat.contains("i32.gt_s"));
}

#[test]
fn test_multiple_methods() {
    let source = r#"
        record Vec2 { x: Int y: Int }
        
        impl Vec2 {
            fun add = self: Vec2 other: Vec2 {
                42
            }
            
            fun sub = self: Vec2 other: Vec2 {
                24
            }
        }
        
        fun main = {
            val sum = (Vec2 { x = 10, y = 20 }, Vec2 { x = 5, y = 10 }) add
            sum
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Verify both methods are generated
    assert!(wat.contains("(func $Vec2_add"));
    assert!(wat.contains("(func $Vec2_sub"));
}

#[test]
fn test_method_resolution() {
    let source = r#"
        record A { value: Int }
        record B { value: Int }
        
        impl A {
            fun process = self: A { 100 }
        }
        
        impl B {
            fun process = self: B { 200 }
        }
        
        fun main = {
            (A { value = 1 }) process + (B { value = 2 }) process
        }
    "#;
    
    // This should fail because we don't have type-directed method resolution yet
    let result = compile_to_wat(source);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Ambiguous method 'process'"));
}

#[test]
fn test_unique_method_resolution() {
    let source = r#"
        record A { value: Int }
        record B { value: Int }
        
        impl A {
            fun process_a = self: A { 100 }
        }
        
        impl B {
            fun process_b = self: B { 200 }
        }
        
        fun main = {
            (A { value = 1 }) process_a + (B { value = 2 }) process_b
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Verify correct method resolution when methods have unique names
    assert!(wat.contains("(func $A_process_a"));
    assert!(wat.contains("(func $B_process_b"));
    assert!(wat.contains("call $A_process_a"));
    assert!(wat.contains("call $B_process_b"));
}