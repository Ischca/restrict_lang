#![cfg(feature = "tat")]

use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

/// Comprehensive tests for Temporal Affine Types (TAT) cleanup code generation
/// These tests verify that the enhanced TAT implementation properly:
/// 1. Generates resource tracking and cleanup WASM code
/// 2. Handles nested temporal scopes correctly
/// 3. Calls appropriate cleanup functions for different resource types
/// 4. Manages cleanup order (LIFO)

fn compile_and_get_wat(input: &str) -> Result<String, String> {
    let (_, program) = parse_program(input).map_err(|e| format!("Parse error: {:?}", e))?;

    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .map_err(|e| format!("Type error: {:?}", e))?;

    let mut codegen = WasmCodeGen::new();
    codegen
        .generate(&program)
        .map_err(|e| format!("Codegen error: {:?}", e))
}

#[test]
fn test_tat_cleanup_infrastructure_generation() {
    // Test that basic infrastructure is generated
    let input = r#"
    fun main: () -> Unit = {
        Unit
    }"#;

    let wat = compile_and_get_wat(input).unwrap();

    // Verify cleanup infrastructure is present
    assert!(
        wat.contains("$resource_list_head"),
        "Resource list global not found"
    );
    assert!(
        wat.contains("$register_resource"),
        "Register resource function not found"
    );
    assert!(
        wat.contains("$cleanup_resources"),
        "Cleanup resources function not found"
    );
    assert!(
        wat.contains("$cleanup_file"),
        "File cleanup function not found"
    );
    assert!(
        wat.contains("$cleanup_database"),
        "Database cleanup function not found"
    );
    assert!(
        wat.contains("$cleanup_transaction"),
        "Transaction cleanup function not found"
    );
    assert!(
        wat.contains("$temp_resource"),
        "Temp resource local not found"
    );
}

#[test]
fn test_tat_temporal_scope_with_file_cleanup() {
    // Test automatic cleanup for File resources
    let input = r#"
    record File<~f> {
        handle: Int32,
        path: String
    }
    
    fun main: () -> Unit = {
        with lifetime<~io> {
            val file = File { handle: 42, path: "test.txt" };
            file.handle;
            Unit
        }
        // File should be automatically cleaned up here
    }"#;

    let wat = compile_and_get_wat(input).unwrap();

    // Verify temporal scope setup
    assert!(
        wat.contains("Initialize temporal scope arena for io"),
        "Temporal scope initialization not found"
    );

    // Verify resource registration
    assert!(
        wat.contains("Auto-register File for temporal cleanup"),
        "Automatic File registration not found"
    );
    assert!(
        wat.contains("i32.const 1"),
        "File cleanup type (1) not found"
    );
    assert!(
        wat.contains("call $register_resource"),
        "Register resource call not found"
    );

    // Verify cleanup happens
    assert!(
        wat.contains("Clean up all resources for temporal scope io"),
        "Cleanup call not found"
    );
    assert!(
        wat.contains("call $cleanup_resources"),
        "Cleanup resources call not found"
    );
    assert!(
        wat.contains("Reset temporal scope arena for io"),
        "Arena reset not found"
    );
}

#[test]
fn test_tat_database_transaction_cleanup_order() {
    // Test LIFO cleanup order with nested resources
    let input = r#"
    record Database<~db> {
        connection: Int32
    }
    
    record Transaction<~tx, ~db> where ~tx within ~db {
        db: Database<~db>,
        txId: Int32
    }
    
    fun main: () -> Unit = {
        with lifetime<~db> {
            val database = Database { connection: 1 };
            
            with lifetime<~tx> {
                val transaction = Transaction { 
                    db: database, 
                    txId: 100 
                };
                transaction.txId;
                Unit
            }
            // Transaction cleaned up first (LIFO)
            
            database.connection;
            Unit
        }
        // Database cleaned up last
    }"#;

    let wat = compile_and_get_wat(input).unwrap();

    // Verify nested scope setup
    assert!(
        wat.contains("Initialize temporal scope arena for db"),
        "Database scope not found"
    );
    assert!(
        wat.contains("Initialize temporal scope arena for tx"),
        "Transaction scope not found"
    );

    // Verify both resource types are registered
    assert!(
        wat.contains("Auto-register Database for temporal cleanup"),
        "Database registration not found"
    );
    assert!(
        wat.contains("Auto-register Transaction for temporal cleanup"),
        "Transaction registration not found"
    );

    // Verify cleanup function mapping
    assert!(
        wat.contains("i32.const 2"),
        "Database cleanup type (2) not found"
    );
    assert!(
        wat.contains("i32.const 3"),
        "Transaction cleanup type (3) not found"
    );

    // Verify cleanup order (tx scope cleaned first, then db scope)
    let tx_cleanup_pos = wat
        .find("Clean up all resources for temporal scope tx")
        .unwrap();
    let db_cleanup_pos = wat
        .find("Clean up all resources for temporal scope db")
        .unwrap();
    assert!(
        tx_cleanup_pos < db_cleanup_pos,
        "Cleanup order incorrect: transaction should be cleaned up before database"
    );
}

#[test]
fn test_tat_resource_list_state_management() {
    // Test that nested scopes properly save and restore resource list state
    let input = r#"
    record File<~f> {
        handle: Int32,
        path: String
    }
    
    fun main: () -> Unit = {
        with lifetime<~outer> {
            val file1 = File { handle: 1, path: "outer.txt" };
            
            with lifetime<~inner> {
                val file2 = File { handle: 2, path: "inner.txt" };
                file2.handle;
                Unit
            }
            
            // After inner scope, should return to outer scope resource list
            file1.handle;
            Unit
        }
    }"#;

    let wat = compile_and_get_wat(input).unwrap();

    // Verify resource list state management
    assert!(
        wat.contains("Save resource list state for temporal scope outer"),
        "Outer scope state save not found"
    );
    assert!(
        wat.contains("Save resource list state for temporal scope inner"),
        "Inner scope state save not found"
    );

    // Verify restoration
    assert!(
        wat.contains("Restore previous resource list state"),
        "Resource list restoration not found"
    );
    assert!(
        wat.contains("local.get $temp_resource"),
        "Temp resource usage not found"
    );
    assert!(
        wat.contains("global.set $resource_list_head"),
        "Resource list head restoration not found"
    );
}

#[test]
fn test_tat_arena_restoration_after_cleanup() {
    // Test that arena context is properly restored after cleanup
    let input = r#"
    record Database<~db> {
        connection: Int32
    }
    
    fun main: () -> Unit = {
        with lifetime<~first> {
            val db1 = Database { connection: 1 };
            
            with lifetime<~second> {
                val db2 = Database { connection: 2 };
                db2.connection;
                Unit
            }
            
            // Should be back in first arena context
            val db3 = Database { connection: 3 };
            db1.connection + db3.connection;
            Unit
        }
    }"#;

    let wat = compile_and_get_wat(input).unwrap();

    // Verify arena addresses are different
    assert!(
        wat.contains("i32.const 32768"),
        "First arena address not found"
    ); // 0x8000
    assert!(
        wat.contains("i32.const 36864"),
        "Second arena address not found"
    ); // 0x9000

    // Verify arena restoration
    assert!(
        wat.contains("Restore previous arena"),
        "Arena restoration not found"
    );
    assert!(
        wat.contains("global.set $current_arena"),
        "Current arena restoration not found"
    );
}

#[test]
fn test_tat_cleanup_with_mixed_resource_types() {
    // Test cleanup with multiple different resource types in same scope
    let input = r#"
    record File<~f> {
        handle: Int32,
        path: String
    }
    
    record Database<~db> {
        connection: Int32
    }
    
    record Transaction<~tx, ~db> where ~tx within ~db {
        db: Database<~db>,
        txId: Int32
    }
    
    fun main: () -> Unit = {
        with lifetime<~mixed> {
            val file = File { handle: 42, path: "data.txt" };
            val db = Database { connection: 1 };
            val tx = Transaction { db: db, txId: 100 };
            
            file.handle + db.connection + tx.txId;
            Unit
        }
        // All three should be cleaned up
    }"#;

    let wat = compile_and_get_wat(input).unwrap();

    // Verify all resource types are registered
    assert!(
        wat.contains("Auto-register File for temporal cleanup"),
        "File registration not found"
    );
    assert!(
        wat.contains("Auto-register Database for temporal cleanup"),
        "Database registration not found"
    );
    assert!(
        wat.contains("Auto-register Transaction for temporal cleanup"),
        "Transaction registration not found"
    );

    // Verify cleanup dispatch logic handles all types
    assert!(
        wat.contains("i32.const 1"),
        "File cleanup type dispatch not found"
    );
    assert!(
        wat.contains("i32.const 2"),
        "Database cleanup type dispatch not found"
    );
    assert!(
        wat.contains("i32.const 3"),
        "Transaction cleanup type dispatch not found"
    );

    assert!(
        wat.contains("call $cleanup_file"),
        "File cleanup call not found"
    );
    assert!(
        wat.contains("call $cleanup_database"),
        "Database cleanup call not found"
    );
    assert!(
        wat.contains("call $cleanup_transaction"),
        "Transaction cleanup call not found"
    );
}

#[test]
fn test_tat_cleanup_with_control_flow() {
    // Test that cleanup happens even with complex control flow
    let input = r#"
    record File<~f> {
        handle: Int32,
        path: String
    }
    
    fun main: () -> Unit = {
        with lifetime<~flow> {
            val file = File { handle: 42, path: "test.txt" };
            
            if file.handle > 0 {
                val result = file.handle * 2;
                if result > 50 {
                    result + 10;
                    Unit
                } else {
                    Unit
                }
            } else {
                Unit
            };
            
            Unit
        }
        // File should still be cleaned up regardless of control flow path
    }"#;

    let wat = compile_and_get_wat(input).unwrap();

    // Verify resource registration happens before control flow
    let register_pos = wat.find("Auto-register File for temporal cleanup").unwrap();
    let if_pos = wat.find("if (result i32)").unwrap_or(wat.len()); // Find first if
    assert!(
        register_pos < if_pos,
        "Resource registration should happen before control flow"
    );

    // Verify cleanup happens after all control flow
    assert!(
        wat.contains("Clean up all resources for temporal scope flow"),
        "Cleanup should happen after control flow"
    );
}

#[test]
fn test_tat_cleanup_function_implementations() {
    // Test that cleanup functions have proper implementations
    let input = r#"
    fun main: () -> Unit = {
        Unit
    }"#;

    let wat = compile_and_get_wat(input).unwrap();

    // Verify cleanup function implementations exist and are well-formed

    // File cleanup
    assert!(
        wat.contains("(func $cleanup_file (param $file_ptr i32)"),
        "File cleanup function signature not found"
    );
    assert!(
        wat.contains("local.get $file_ptr"),
        "File cleanup doesn't access resource"
    );
    assert!(
        wat.contains("i32.load  ;; Load file handle"),
        "File cleanup doesn't load handle"
    );

    // Database cleanup
    assert!(
        wat.contains("(func $cleanup_database (param $db_ptr i32)"),
        "Database cleanup function signature not found"
    );
    assert!(
        wat.contains("i32.load  ;; Load connection handle"),
        "Database cleanup doesn't load connection"
    );

    // Transaction cleanup
    assert!(
        wat.contains("(func $cleanup_transaction (param $tx_ptr i32)"),
        "Transaction cleanup function signature not found"
    );
    assert!(
        wat.contains("i32.const 8  ;; Offset to txId field"),
        "Transaction cleanup doesn't access txId"
    );
}

#[test]
fn test_tat_empty_temporal_scope() {
    // Test that temporal scopes work correctly even with no resources
    let input = r#"
    fun main: () -> Unit = {
        with lifetime<~empty> {
            val x = 42;
            x + 1;
            Unit
        }
    }"#;

    let wat = compile_and_get_wat(input).unwrap();

    // Should still have temporal scope infrastructure
    assert!(
        wat.contains("Initialize temporal scope arena for empty"),
        "Empty scope initialization not found"
    );
    assert!(
        wat.contains("Clean up all resources for temporal scope empty"),
        "Empty scope cleanup not found"
    );

    // But no resource registration
    assert!(
        !wat.contains("Auto-register"),
        "No auto-registration should occur in empty scope"
    );
}
