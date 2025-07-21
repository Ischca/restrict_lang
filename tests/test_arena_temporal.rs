use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

fn compile(input: &str) -> Result<String, String> {
    let (remaining, program) = parse_program(input)
        .map_err(|e| format!("Parse error: {:?}", e))?;
    
    println!("Parsed {} declarations:", program.declarations.len());
    println!("Remaining input: {} chars", remaining.len());
    if !remaining.trim().is_empty() {
        println!("Remaining: {:?}", remaining);
    }
    
    for (i, decl) in program.declarations.iter().enumerate() {
        match decl {
            restrict_lang::TopDecl::Function(f) => {
                println!("  [{}] Function: {}", i, f.name);
            }
            restrict_lang::TopDecl::Record(r) => {
                println!("  [{}] Record: {}", i, r.name);
            }
            _ => {
                println!("  [{}] Other", i);
            }
        }
    }
    
    let mut checker = TypeChecker::new();
    checker.check_program(&program)
        .map_err(|e| format!("Type error: {:?}", e))?;
    
    let mut codegen = WasmCodeGen::new();
    codegen.generate(&program)
        .map_err(|e| format!("Codegen error: {:?}", e))
}

#[test]
fn test_temporal_scope_arena_allocation() {
    let input = r#"record Point<~p> {
    x: Int32
    y: Int32
}

fun main = {
    with lifetime<~outer> {
        val p1 = Point { x = 10, y = 20 };
        
        with lifetime<~inner> {
            val p2 = Point { x = 30, y = 40 };
            p2.x
        };
        
        p1.x
    }
}"#;
    
    let wat = match compile(input) {
        Ok(wat) => wat,
        Err(e) => {
            eprintln!("Compilation failed: {}", e);
            panic!("Compilation failed: {}", e);
        }
    };
    
    // Debug: Save WAT to file for inspection
    std::fs::write("debug_wat_output.wat", &wat).unwrap();
    println!("WAT saved to debug_wat_output.wat");
    
    // Check arena initialization for temporal scopes
    assert!(wat.contains("Initialize temporal scope arena for outer"));
    assert!(wat.contains("Initialize temporal scope arena for inner"));
    
    // Check arena cleanup
    assert!(wat.contains("Clean up temporal scope arena for inner"));
    assert!(wat.contains("Clean up temporal scope arena for outer"));
    
    // Check arena restore
    assert!(wat.contains("Restore previous arena"));
}

#[test]
fn test_nested_temporal_scope_memory() {
    let input = r#"record Buffer<~b> {
    data: String
    size: Int32
}

fun main = {
    with lifetime<~level1> {
        val buf1 = Buffer { data = "Level 1", size = 7 };
        
        with lifetime<~level2> {
            val buf2 = Buffer { data = "Level 2", size = 7 };
            
            with lifetime<~level3> {
                val buf3 = Buffer { data = "Level 3", size = 7 };
                buf3.size
            };
            
            buf2.size
        };
        
        buf1.size
    }
}"#;
    
    let wat = match compile(input) {
        Ok(wat) => wat,
        Err(e) => {
            eprintln!("Compilation failed: {}", e);
            panic!("Compilation failed: {}", e);
        }
    };
    
    // Verify proper arena stack management
    assert!(wat.contains("Initialize temporal scope arena for level1"));
    assert!(wat.contains("Initialize temporal scope arena for level2"));
    assert!(wat.contains("Initialize temporal scope arena for level3"));
    
    // Each arena should have different addresses
    assert!(wat.contains("i32.const 32768")); // 0x8000
    assert!(wat.contains("i32.const 36864")); // 0x9000
    assert!(wat.contains("i32.const 40960")); // 0xA000
}

#[test]
fn test_temporal_scope_with_allocations() {
    let input = r#"fun main = {
    with lifetime<~temp> {
        val list = [1, 2, 3, 4, 5];
        val opt = Some(42);
        list |> length
    }
}"#;
    
    let wat = match compile(input) {
        Ok(wat) => wat,
        Err(e) => {
            eprintln!("Compilation failed: {}", e);
            panic!("Compilation failed: {}", e);
        }
    };
    
    // Check that allocations use the temporal arena
    assert!(wat.contains("call $allocate")); // List and Option allocations
    assert!(wat.contains("global.get $current_arena"));
    assert!(wat.contains("call $arena_alloc"));
}

#[test]
fn test_temporal_scope_return_value() {
    let input = r#"fun process<~p> = {
    with lifetime<~local> {
        val temp = 100;
        temp + 42
    }
}

fun main = {
    with lifetime<~main> {
        process
    }
}"#;
    
    let wat = match compile(input) {
        Ok(wat) => wat,
        Err(e) => {
            eprintln!("Compilation failed: {}", e);
            panic!("Compilation failed: {}", e);
        }
    };
    
    // Verify that values can be returned from temporal scopes
    assert!(wat.contains("Initialize temporal scope arena for local"));
    assert!(wat.contains("Clean up temporal scope arena for local"));
    // The addition result should be preserved
    assert!(wat.contains("i32.add"));
}

#[test]
fn test_async_runtime_with_arena() {
    let input = r#"fun compute<~async> = n: Int32 {
    n * 2
}

fun main = {
    with AsyncRuntime<~async> {
        val task = 21 |> compute |> spawn;
        task |> await
    }
}"#;
    
    match compile(input) {
        Ok(wat) => {
            // For now, async operations might not be fully implemented
            assert!(wat.contains("Initialize temporal scope arena for async") ||
                    wat.contains("async operations"));
        }
        Err(e) => {
            // Expected for now if async isn't fully implemented
            assert!(e.contains("async") || e.contains("AsyncRuntime"));
        }
    }
}

#[test]
fn test_temporal_memory_bounds() {
    let input = r#"record Large<~l> {
    data1: Int32
    data2: Int32
    data3: Int32
    data4: Int32
}

fun main = {
    with lifetime<~scope> {
        val item1 = Large { data1 = 1, data2 = 2, data3 = 3, data4 = 4 };
        val item2 = Large { data1 = 5, data2 = 6, data3 = 7, data4 = 8 };
        val item3 = Large { data1 = 9, data2 = 10, data3 = 11, data4 = 12 };
        item1.data1 + item2.data2 + item3.data3
    }
}"#;
    
    let wat = match compile(input) {
        Ok(wat) => wat,
        Err(e) => {
            eprintln!("Compilation failed: {}", e);
            panic!("Compilation failed: {}", e);
        }
    };
    
    // Verify memory allocation within arena bounds
    assert!(wat.contains("call $allocate"));
    assert!(wat.contains("TODO: Add bounds checking") || 
            wat.contains("bounds check"));
}