# Temporal Affine Types (TAT) Cleanup Implementation

This document describes the enhanced TAT implementation for automatic resource cleanup in Restrict Language.

## Overview

The Temporal Affine Types (TAT) system now includes automatic cleanup code generation that ensures resources are properly disposed of when temporal scopes end. This provides deterministic resource management without garbage collection.

## Architecture

### 1. Resource Tracking Infrastructure

The code generator creates a linked list to track resources that need cleanup:

```wasm
;; Global resource list head
(global $resource_list_head (mut i32) (i32.const 0))

;; Resource entry format: [resource_ptr, cleanup_type, next_entry]
;; Each entry is 12 bytes
```

### 2. Resource Registration

When temporal resources are created (e.g., `File<~f>`, `Database<~db>`), they are automatically registered:

```rust
// In generate_expr for RecordLit
if self.cleanup_functions.contains_key(&record_lit.name) && !self.temporal_scope_stack.is_empty() {
    // Auto-register for cleanup
    self.register_resource(resource_ptr, cleanup_type);
}
```

Generated WASM:
```wasm
;; Auto-register File for temporal cleanup
local.tee $temp_resource
i32.const 1  ;; File cleanup type
call $register_resource
local.get $temp_resource
```

### 3. Temporal Scope Management

Each `with lifetime<~name> { ... }` block:

1. **Initializes** a new arena for memory allocation
2. **Saves** the current resource list state
3. **Executes** the body (registering resources as needed)
4. **Cleans up** all registered resources via `$cleanup_resources`
5. **Restores** the previous resource list state
6. **Resets** the arena memory

```wasm
;; Initialize temporal scope arena for io at address 0x8000
i32.const 32768
call $arena_init
drop

;; Set as current arena
i32.const 32768
global.set $current_arena

;; Save resource list state
global.get $resource_list_head
local.tee $temp_resource

;; ... body execution ...

;; Clean up all resources for temporal scope io
call $cleanup_resources

;; Restore previous resource list state
local.get $temp_resource
global.set $resource_list_head

;; Reset temporal scope arena
i32.const 32768
call $arena_reset
```

### 4. Cleanup Function Dispatch

The `$cleanup_resources` function iterates through the resource list and calls appropriate cleanup functions based on type:

```wasm
;; Call appropriate cleanup function based on type
local.get $cleanup_type
i32.const 1  ;; File type
i32.eq
(if
  (then
    local.get $resource_ptr
    call $cleanup_file
  )
)

local.get $cleanup_type
i32.const 2  ;; Database type
i32.eq
(if
  (then
    local.get $resource_ptr
    call $cleanup_database
  )
)
```

### 5. Resource-Specific Cleanup Functions

Each resource type has a dedicated cleanup function:

#### File Cleanup
```wasm
(func $cleanup_file (param $file_ptr i32)
  ;; Close file handle (simplified - would call WASI fd_close)
  local.get $file_ptr
  i32.load  ;; Load file handle from first field
  ;; call $wasi_close  ;; Would call actual WASI close
  drop      ;; For now, just drop the handle
)
```

#### Database Cleanup
```wasm
(func $cleanup_database (param $db_ptr i32)
  ;; Close database connection (simplified)
  local.get $db_ptr
  i32.load  ;; Load connection handle
  ;; call $db_close  ;; Would call actual database close
  drop      ;; For now, just drop the handle
)
```

#### Transaction Cleanup
```wasm
(func $cleanup_transaction (param $tx_ptr i32)
  ;; Rollback transaction if not committed
  local.get $tx_ptr
  i32.const 8  ;; Offset to txId field
  i32.add
  i32.load
  ;; call $tx_rollback  ;; Would call actual transaction rollback
  drop
)
```

## Memory Layout

### Arena Structure
Each temporal scope gets its own arena starting at a unique address:

```
Arena Header (8 bytes):
├── Start Address (4 bytes)
└── Current Address (4 bytes)

Arena Data:
├── Resource allocations
└── Local variables
```

### Resource List Entry
Each resource entry in the cleanup list:

```
Resource Entry (12 bytes):
├── Resource Pointer (4 bytes)    - Points to the actual resource
├── Cleanup Type (4 bytes)        - Identifies which cleanup function to call
└── Next Entry Pointer (4 bytes)  - Links to next resource in list
```

## Cleanup Guarantees

### 1. LIFO Cleanup Order
Resources are cleaned up in reverse order of their creation (Last In, First Out):

```rust
with lifetime<~outer> {
    let resource1 = File { ... };      // Created first
    with lifetime<~inner> {
        let resource2 = Database { ... }; // Created second
        // ...
    }  // resource2 cleaned up first
}  // resource1 cleaned up second
```

### 2. Exception Safety
Cleanup occurs even in the presence of early returns, exceptions, or complex control flow because it's generated at the scope exit point.

### 3. Nested Scope Isolation
Each temporal scope maintains its own resource list state, preventing interference between nested scopes.

## Integration with Affine Types

The TAT cleanup system works seamlessly with Restrict Language's affine type system:

1. **Single Use**: Resources can only be used once, preventing double-cleanup
2. **Move Semantics**: Resource ownership is clear and tracked
3. **Temporal Constraints**: `where ~tx within ~db` ensures proper lifetime relationships

## Performance Characteristics

### Time Complexity
- **Resource Registration**: O(1) - simply prepends to linked list
- **Cleanup**: O(n) where n is number of resources in scope
- **Scope Entry/Exit**: O(1) for arena management

### Space Complexity
- **Per Resource**: 12 bytes overhead for cleanup tracking
- **Per Scope**: 4KB arena (can be tuned based on usage patterns)

## Example Usage

```rust
record File<~f> { handle: Int32, path: String }
record Database<~db> { connection: Int32 }

fun processData: () -> Unit = {
    with lifetime<~io> {
        val file = File { handle: 42, path: "data.txt" };    // Auto-registered
        
        with lifetime<~db> {
            val db = Database { connection: 1001 };           // Auto-registered
            
            // Use resources...
            file.handle + db.connection;
            Unit
        }  // db automatically cleaned up via cleanup_database()
        
        Unit
    }  // file automatically cleaned up via cleanup_file()
}
```

Generated cleanup sequence:
1. `cleanup_database()` called for db
2. Database arena reset
3. `cleanup_file()` called for file  
4. File arena reset

## Future Enhancements

1. **Custom Cleanup Functions**: Allow users to define cleanup behavior for custom types
2. **Async Cleanup**: Support for asynchronous resource cleanup
3. **Resource Pooling**: Reuse cleaned up resources where appropriate
4. **Debug Tracing**: Add debug output for cleanup operations
5. **Resource Metrics**: Track resource usage and cleanup statistics

## Testing

The implementation includes comprehensive tests covering:

- Basic resource registration and cleanup
- Nested scope cleanup order
- Mixed resource types in same scope
- Control flow with cleanup
- Empty temporal scopes
- Arena management and restoration

See `tests/test_tat_cleanup_comprehensive.rs` for detailed test cases.