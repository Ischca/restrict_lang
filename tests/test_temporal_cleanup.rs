use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

/// Tests for temporal scope cleanup and memory management
/// These tests verify that temporal scopes properly clean up resources

fn compile(input: &str) -> Result<String, String> {
    let (_, program) = parse_program(input)
        .map_err(|e| format!("Parse error: {:?}", e))?;
    
    let mut checker = TypeChecker::new();
    checker.check_program(&program)
        .map_err(|e| format!("Type error: {:?}", e))?;
    
    let mut codegen = WasmCodeGen::new();
    codegen.generate(&program)
        .map_err(|e| format!("Codegen error: {:?}", e))
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_cleanup_order() {
    // Test that temporal scopes are cleaned up in reverse order
    let input = r#"
    record Logger<~l> {
        name: String
    }
    
    fun log: <~l>(logger: Logger<~l>, msg: String) -> Unit = {
        // In real implementation, this would log
        Unit
    }
    
    fun main: () -> Unit = {
        with lifetime<~app> {
            val appLogger = Logger { name: "app" };
            log(appLogger, "Starting app");
            
            with lifetime<~request> {
                val reqLogger = Logger { name: "request" };
                log(reqLogger, "Processing request");
                
                with lifetime<~query> {
                    val queryLogger = Logger { name: "query" };
                    log(queryLogger, "Executing query");
                    Unit
                }
                // ~query cleaned up here
            }
            // ~request cleaned up here
        }
        // ~app cleaned up here
    }"#;
    
    let wat = compile(input).unwrap();
    
    // Verify cleanup order (LIFO)
    assert!(wat.contains("Clean up temporal scope arena for query"));
    assert!(wat.contains("Clean up temporal scope arena for request"));
    assert!(wat.contains("Clean up temporal scope arena for app"));
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_cleanup_with_early_return() {
    // Test that cleanup happens even with early returns
    let input = r#"
    record Resource<~r> {
        id: Int32,
        allocated: Bool
    }
    
    fun process: () -> Int32 = {
        with lifetime<~temp> {
            val res = Resource { id: 42, allocated: true };
            
            if res.allocated {
                return res.id;  // Early return should still trigger cleanup
            }
            
            0
        }
    }
    
    fun main: () -> Unit = {
        val result = process();
        Unit
    }"#;
    
    let wat = compile(input).unwrap();
    
    // Verify cleanup happens before return
    assert!(wat.contains("Clean up temporal scope arena for temp"));
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_cleanup_exception_safety() {
    // Test cleanup in presence of panics/exceptions
    let input = r#"
    record CriticalResource<~c> {
        handle: Int32
    }
    
    fun riskyOperation: () -> Unit = {
        with lifetime<~critical> {
            val resource = CriticalResource { handle: 1 };
            
            // Simulate potential panic point
            if resource.handle == 1 {
                panic("Something went wrong");
            }
            
            Unit
        }
    }
    
    fun main: () -> Unit = {
        // Even if riskyOperation panics, cleanup should happen
        riskyOperation();
        Unit
    }"#;
    
    match compile(input) {
        Ok(wat) => {
            // Should have cleanup code even with panic
            assert!(wat.contains("Clean up temporal scope arena for critical"));
        }
        Err(e) => {
            // Panic might not be implemented yet
            assert!(e.contains("panic") || e.contains("Unsupported"));
        }
    }
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_cleanup_with_loops() {
    // Test cleanup in loops
    let input = r#"
    record Counter<~c> {
        value: Int32
    }
    
    fun main: () -> Unit = {
        mut i = 0;
        while i < 3 {
            with lifetime<~iter> {
                val counter = Counter { value: i };
                i = i + 1;
                Unit
            }
            // ~iter should be cleaned up after each iteration
        };
        Unit
    }"#;
    
    let wat = compile(input).unwrap();
    
    // Verify cleanup happens inside loop
    assert!(wat.contains("Clean up temporal scope arena for iter"));
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_cleanup_nested_functions() {
    // Test cleanup across function calls
    let input = r#"
    record Handle<~h> {
        id: Int32
    }
    
    fun inner: () -> Int32 = {
        with lifetime<~inner_scope> {
            val handle = Handle { id: 100 };
            handle.id
        }
    }
    
    fun outer: () -> Int32 = {
        with lifetime<~outer_scope> {
            val handle = Handle { id: 200 };
            val innerResult = inner();
            handle.id + innerResult
        }
    }
    
    fun main: () -> Unit = {
        val result = outer();
        Unit
    }"#;
    
    let wat = compile(input).unwrap();
    
    // Verify both scopes are cleaned up
    assert!(wat.contains("Clean up temporal scope arena for inner_scope"));
    assert!(wat.contains("Clean up temporal scope arena for outer_scope"));
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_cleanup_with_match() {
    // Test cleanup in pattern matching
    let input = r#"
    record Token<~t> {
        value: String
    }
    
    fun main: () -> Unit = {
        val option = Some(42);
        
        match option {
            Some(n) => {
                with lifetime<~some_branch> {
                    val token = Token { value: "matched" };
                    Unit
                }
            },
            None => {
                with lifetime<~none_branch> {
                    val token = Token { value: "no match" };
                    Unit
                }
            }
        };
        Unit
    }"#;
    
    let wat = compile(input).unwrap();
    
    // Verify cleanup in both branches
    assert!(wat.contains("Clean up temporal scope arena for some_branch") ||
            wat.contains("Clean up temporal scope arena for none_branch"));
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_cleanup_memory_layout() {
    // Test that memory is properly laid out for cleanup
    let input = r#"
    record Large<~l> {
        data1: Int32,
        data2: Int32,
        data3: Int32,
        data4: Int32,
        data5: Int32
    }
    
    fun main: () -> Unit = {
        with lifetime<~scope1> {
            val large1 = Large { 
                data1: 1, data2: 2, data3: 3, 
                data4: 4, data5: 5 
            };
            
            with lifetime<~scope2> {
                val large2 = Large { 
                    data1: 6, data2: 7, data3: 8, 
                    data4: 9, data5: 10 
                };
                
                large1.data1 + large2.data1;
                Unit
            }
        }
    }"#;
    
    let wat = compile(input).unwrap();
    
    // Verify arenas are at different addresses
    assert!(wat.contains("i32.const 32768")); // First arena
    assert!(wat.contains("i32.const 36864")); // Second arena
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_cleanup_with_recursion() {
    // Test cleanup in recursive functions
    let input = r#"
    record Node<~n> {
        value: Int32,
        hasNext: Bool
    }
    
    fun traverse: (depth: Int32) -> Int32 = {
        if depth == 0 {
            0
        } else {
            with lifetime<~node> {
                val node = Node { value: depth, hasNext: true };
                node.value + traverse(depth - 1)
            }
        }
    }
    
    fun main: () -> Unit = {
        val result = traverse(3);
        Unit
    }"#;
    
    let wat = compile(input).unwrap();
    
    // Verify cleanup happens at each recursion level
    assert!(wat.contains("Clean up temporal scope arena for node"));
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_cleanup_interleaved() {
    // Test interleaved temporal scopes
    let input = r#"
    record A<~a> {
        id: Int32
    }
    
    record B<~b> {
        id: Int32
    }
    
    fun main: () -> Unit = {
        with lifetime<~scope_a1> {
            val a1 = A { id: 1 };
            
            with lifetime<~scope_b> {
                val b = B { id: 2 };
                
                with lifetime<~scope_a2> {
                    val a2 = A { id: 3 };
                    
                    a1.id + b.id + a2.id;
                    Unit
                }
            }
        }
    }"#;
    
    let wat = compile(input).unwrap();
    
    // Verify all scopes are cleaned up
    assert!(wat.contains("Clean up temporal scope arena for scope_a2"));
    assert!(wat.contains("Clean up temporal scope arena for scope_b"));
    assert!(wat.contains("Clean up temporal scope arena for scope_a1"));
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_cleanup_restore_arena() {
    // Test that previous arena is restored after cleanup
    let input = r#"
    record Item<~i> {
        value: Int32
    }
    
    fun main: () -> Unit = {
        with lifetime<~outer> {
            val item1 = Item { value: 1 };
            
            with lifetime<~inner> {
                val item2 = Item { value: 2 };
                item2.value;
                Unit
            };
            
            // After inner cleanup, should be back in outer arena
            val item3 = Item { value: 3 };
            item1.value + item3.value;
            Unit
        }
    }"#;
    
    let wat = compile(input).unwrap();
    
    // Verify arena restoration
    assert!(wat.contains("Restore previous arena"));
}