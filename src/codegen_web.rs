use crate::ast::*;
use crate::codegen::{CodeGenError, WasmCodeGen};

/// Web-specific code generator that uses JavaScript imports instead of WASI
pub struct WebWasmCodeGen {
    inner: WasmCodeGen,
}

impl WebWasmCodeGen {
    pub fn new() -> Self {
        Self {
            inner: WasmCodeGen::new(),
        }
    }
    
    pub fn generate(&mut self, program: &Program) -> Result<String, CodeGenError> {
        // Use the inner generator but replace WASI imports with JS imports
        let mut wat = self.inner.generate(program)?;
        
        // Replace WASI imports with JavaScript imports
        wat = wat.replace(
            r#"  (import "wasi_snapshot_preview1" "fd_write" (func $fd_write (param i32 i32 i32 i32) (result i32)))"#,
            r#"  (import "env" "js_print" (func $js_print (param i32 i32)))"#
        );
        
        // Remove proc_exit import (not needed for web)
        wat = wat.replace(
            r#"  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))"#,
            ""
        );
        
        // Replace fd_write calls with js_print calls
        wat = wat.replace(
            "call $fd_write",
            "call $js_print"
        );
        
        // Simplify println to just pass pointer and length
        let println_old = r#"  (func $println (param $str i32)
    (local $len i32)
    (local $iov_base i32)
    (local $iov_len i32)
    (local $nwritten i32)
    
    ;; Read string length from memory (first 4 bytes)
    local.get $str
    i32.load
    local.set $len
    
    ;; Prepare iovec structure at memory address 0
    ;; iov_base = str + 4 (skip length prefix)
    i32.const 0
    local.get $str
    i32.const 4
    i32.add
    i32.store
    
    ;; iov_len = string length
    i32.const 4
    local.get $len
    i32.store
    
    ;; Add newline to iovec
    ;; Store newline at address 16
    i32.const 16
    i32.const 10  ;; '\n'
    i32.store8
    
    ;; Second iovec for newline
    i32.const 8   ;; second iovec base
    i32.const 16  ;; address of newline
    i32.store
    
    i32.const 12  ;; second iovec len
    i32.const 1   ;; length of newline
    i32.store
    
    ;; Call fd_write
    i32.const 1   ;; stdout
    i32.const 0   ;; iovs
    i32.const 2   ;; iovs_len (2 iovecs)
    i32.const 20  ;; nwritten (output param)
    call $js_print
    drop
  )"#;
        
        let println_new = r#"  (func $println (param $str i32)
    (local $len i32)
    
    ;; Read string length from memory (first 4 bytes)
    local.get $str
    i32.load
    local.set $len
    
    ;; Call JavaScript print function with pointer to string data and length
    local.get $str
    i32.const 4
    i32.add    ;; Skip length prefix
    local.get $len
    call $js_print
  )"#;
        
        wat = wat.replace(println_old, println_new);
        
        Ok(wat)
    }
}