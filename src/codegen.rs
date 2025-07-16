//! # Code Generation Module
//!
//! Generates WebAssembly (WASM) code from the typed AST. The code generator
//! produces WebAssembly Text Format (WAT) which can be compiled to binary WASM.
//!
//! ## Key Features
//!
//! - **Zero GC**: Direct memory management without garbage collection
//! - **Affine Types**: Compile-time guarantees translate to efficient runtime
//! - **Monomorphization**: Generic functions are specialized at compile time
//! - **Arena Allocation**: Efficient memory management for temporary values
//! - **Lambda Support**: First-class functions with closure capture
//!
//! ## Memory Layout
//!
//! The generated WASM uses a custom memory layout:
//! - String constants are stored in a dedicated section
//! - Arena allocation for temporary values
//! - Stack-based local variables
//!
//! ## Example
//!
//! ```rust
//! use restrict_lang::codegen::WasmCodeGen;
//! use restrict_lang::parser::parse_program;
//!
//! let program = parse_program(source).unwrap();
//! let mut codegen = WasmCodeGen::new();
//! let wat = codegen.generate(&program)?;
//! ```

use crate::ast::*;
use std::collections::HashMap;
use thiserror::Error;

/// Code generation errors.
#[derive(Debug, Error)]
pub enum CodeGenError {
    /// Variable not found in current scope
    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),
    
    /// Function not found
    #[error("Undefined function: {0}")]
    UndefinedFunction(String),
    
    /// Type cannot be represented in WASM
    #[error("Type not supported in WASM: {0}")]
    UnsupportedType(String),
    
    /// Feature not yet implemented
    #[error("Feature not implemented: {0}")]
    NotImplemented(String),
}

/// WebAssembly Text Format (WAT) code generator.
/// 
/// Transforms a typed AST into executable WebAssembly code.
/// The generator handles memory management, function calls,
/// and the unique OSV syntax of Restrict Language.
pub struct WasmCodeGen {
    /// Variable to local index mapping (scoped)
    locals: Vec<HashMap<String, u32>>,
    /// Function signatures for type checking
    functions: HashMap<String, FunctionSig>,
    /// Method signatures: record_name -> method_name -> function_sig
    methods: HashMap<String, HashMap<String, FunctionSig>>,
    /// String constants pool for deduplication
    strings: Vec<String>,
    /// String constant offsets in linear memory
    string_offsets: HashMap<String, u32>,
    /// Next available memory offset for allocation
    next_mem_offset: u32,
    /// Current function being generated
    current_function: Option<String>,
    /// Generated WAT code output
    output: String,
    /// Type information for expressions (from type checker)
    pub expr_types: HashMap<*const Expr, String>,
    /// Arena management for temporary allocations
    arena_stack: Vec<u32>,
    /// Next available arena address
    next_arena_addr: u32,
    /// Default arena for global allocations
    default_arena: Option<u32>,
    /// Counter for generating unique lambda names
    lambda_counter: u32,
    /// Generated lambda function definitions
    lambda_functions: Vec<String>,
    /// Function table entries for indirect calls
    function_table: Vec<String>,
    /// Whether we're inside a lambda with captures
    in_lambda_with_captures: bool,
    /// List of captured variable names in current lambda
    captured_vars: Vec<String>,
}

#[derive(Debug, Clone)]
struct FunctionSig {
    _params: Vec<WasmType>,
    result: Option<WasmType>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum WasmType {
    I32,
    I64,
    F32,
    F64,
}

impl WasmCodeGen {
    pub fn new() -> Self {
        Self {
            locals: vec![HashMap::new()],
            functions: HashMap::new(),
            methods: HashMap::new(),
            strings: Vec::new(),
            string_offsets: HashMap::new(),
            next_mem_offset: 1024, // Start at 1024 to leave room for other data
            current_function: None,
            output: String::new(),
            expr_types: HashMap::new(),
            arena_stack: Vec::new(),
            next_arena_addr: 0x8000, // Arena starts at 32KB
            default_arena: None,
            lambda_counter: 0,
            lambda_functions: Vec::new(),
            function_table: Vec::new(),
            in_lambda_with_captures: false,
            captured_vars: Vec::new(),
        }
    }
    
    pub fn generate(&mut self, program: &Program) -> Result<String, CodeGenError> {
        self.output.push_str("(module\n");
        
        // Process module imports first
        self.generate_imports(&program.imports)?;
        
        // Import WASI functions for I/O
        self.output.push_str("  ;; WASI imports\n");
        self.output.push_str("  (import \"wasi_snapshot_preview1\" \"fd_write\" (func $fd_write (param i32 i32 i32 i32) (result i32)))\n");
        self.output.push_str("  (import \"wasi_snapshot_preview1\" \"proc_exit\" (func $proc_exit (param i32)))\n");
        
        // Memory
        self.output.push_str("\n  ;; Memory\n");
        self.output.push_str("  (memory 1)\n");
        self.output.push_str("  (export \"memory\" (memory 0))\n");
        
        // Collect string constants first
        self.collect_strings(program)?;
        
        // Generate string data section
        if !self.strings.is_empty() {
            self.output.push_str("\n  ;; String constants\n");
            for (s, offset) in &self.string_offsets {
                let bytes = s.as_bytes();
                let len = bytes.len() as u32;
                
                // Format: 4 bytes length + string data
                self.output.push_str(&format!("  (data (i32.const {}) \"", offset));
                
                // Write length as little-endian
                self.output.push_str(&format!("\\{:02x}\\{:02x}\\{:02x}\\{:02x}", 
                    len & 0xff, 
                    (len >> 8) & 0xff,
                    (len >> 16) & 0xff,
                    (len >> 24) & 0xff
                ));
                
                // Write string data
                for byte in bytes {
                    if *byte == b'"' {
                        self.output.push_str("\\\"");
                    } else if *byte == b'\\' {
                        self.output.push_str("\\\\");
                    } else if *byte >= 32 && *byte <= 126 {
                        self.output.push(*byte as char);
                    } else {
                        self.output.push_str(&format!("\\{:02x}", byte));
                    }
                }
                
                self.output.push_str("\")\n");
            }
        }
        
        // Generate built-in functions
        self.generate_builtin_functions()?;
        
        // Generate arena allocator functions
        self.generate_arena_functions()?;
        
        // Generate list operation functions
        self.generate_list_functions()?;
        
        // Generate array operation functions
        self.generate_array_functions()?;
        
        // Collect all function signatures first
        for decl in &program.declarations {
            match decl {
                TopDecl::Function(func) => {
                    self.register_function_signature(func)?;
                }
                TopDecl::Binding(_) => {
                    // Global values not yet supported
                }
                TopDecl::Record(record) => {
                    self.register_record_methods(record)?;
                }
                TopDecl::Export(_) => {
                    // Not yet implemented
                }
                TopDecl::Impl(_) | TopDecl::Context(_) => {
                    // Not yet implemented
                }
            }
        }
        
        // Generate functions
        self.output.push_str("\n  ;; Functions\n");
        for decl in &program.declarations {
            match decl {
                TopDecl::Function(func) => {
                    self.generate_function(func)?;
                }
                TopDecl::Binding(_) => {
                    // Global values not yet supported
                }
                TopDecl::Record(record) => {
                    self.generate_record_methods(record)?;
                }
                TopDecl::Export(_) => {
                    // Not yet implemented
                }
                TopDecl::Impl(_) | TopDecl::Context(_) => {
                    // Not yet implemented
                }
            }
        }
        
        // Generate lambda functions
        for lambda_func in &self.lambda_functions {
            self.output.push_str(lambda_func);
        }
        
        // Generate function table if we have indirect calls
        if !self.function_table.is_empty() {
            self.output.push_str("\n  ;; Function table for indirect calls\n");
            self.output.push_str("  (table ");
            self.output.push_str(&self.function_table.len().to_string());
            self.output.push_str(" funcref)\n");
            
            // Initialize table elements
            for (i, func_name) in self.function_table.iter().enumerate() {
                self.output.push_str(&format!("  (elem (i32.const {}) func ${})\n", i, func_name));
            }
        }
        
        // Generate module exports
        self.generate_exports(program)?;
        
        // Export main function if it exists
        if self.functions.contains_key("main") {
            self.output.push_str("\n  ;; Export main\n");
            self.output.push_str("  (export \"_start\" (func $main))\n");
        }
        
        self.output.push_str(")\n");
        
        Ok(self.output.clone())
    }
    
    fn generate_builtin_functions(&mut self) -> Result<(), CodeGenError> {
        // Built-in println function for strings
        self.output.push_str("\n  ;; Built-in functions\n");
        self.output.push_str("  (func $println (param $str i32)\n");
        self.output.push_str("    (local $len i32)\n");
        self.output.push_str("    (local $iov_base i32)\n");
        self.output.push_str("    (local $iov_len i32)\n");
        self.output.push_str("    (local $nwritten i32)\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Read string length from memory (first 4 bytes)\n");
        self.output.push_str("    local.get $str\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $len\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Prepare iovec structure at memory address 0\n");
        self.output.push_str("    ;; iov_base = str + 4 (skip length prefix)\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    local.get $str\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; iov_len = string length\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    local.get $len\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Add newline to iovec\n");
        self.output.push_str("    ;; Store newline at address 16\n");
        self.output.push_str("    i32.const 16\n");
        self.output.push_str("    i32.const 10  ;; '\\n'\n");
        self.output.push_str("    i32.store8\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Second iovec for newline\n");
        self.output.push_str("    i32.const 8   ;; second iovec base\n");
        self.output.push_str("    i32.const 16  ;; address of newline\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    i32.const 12  ;; second iovec len\n");
        self.output.push_str("    i32.const 1   ;; length of newline\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Call fd_write\n");
        self.output.push_str("    i32.const 1   ;; stdout\n");
        self.output.push_str("    i32.const 0   ;; iovs\n");
        self.output.push_str("    i32.const 2   ;; iovs_len (2 iovecs)\n");
        self.output.push_str("    i32.const 20  ;; nwritten (output param)\n");
        self.output.push_str("    call $fd_write\n");
        self.output.push_str("    drop\n");
        self.output.push_str("  )\n");
        
        // print_int function with proper integer to string conversion
        self.output.push_str("\n  (func $print_int (param $value i32)\n");
        self.output.push_str("    (local $num i32)\n");
        self.output.push_str("    (local $digit i32)\n");
        self.output.push_str("    (local $buffer_start i32)\n");
        self.output.push_str("    (local $buffer_end i32)\n");
        self.output.push_str("    (local $is_negative i32)\n");
        self.output.push_str("    (local $len i32)\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Use memory starting at address 400 for the buffer\n");
        self.output.push_str("    i32.const 420  ;; Start from the end of buffer and work backwards\n");
        self.output.push_str("    local.set $buffer_end\n");
        self.output.push_str("    local.get $buffer_end\n");
        self.output.push_str("    local.set $buffer_start\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Check if negative\n");
        self.output.push_str("    local.get $value\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    i32.lt_s\n");
        self.output.push_str("    local.set $is_negative\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Get absolute value\n");
        self.output.push_str("    local.get $is_negative\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        i32.const 0\n");
        self.output.push_str("        local.get $value\n");
        self.output.push_str("        i32.sub\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        local.get $value\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    local.set $num\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Handle zero special case\n");
        self.output.push_str("    local.get $num\n");
        self.output.push_str("    i32.eqz\n");
        self.output.push_str("    (if\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        local.get $buffer_start\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.sub\n");
        self.output.push_str("        local.tee $buffer_start\n");
        self.output.push_str("        i32.const 48  ;; '0'\n");
        self.output.push_str("        i32.store8\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        ;; Convert digits\n");
        self.output.push_str("        (block $break\n");
        self.output.push_str("          (loop $digit_loop\n");
        self.output.push_str("            local.get $num\n");
        self.output.push_str("            i32.eqz\n");
        self.output.push_str("            br_if $break\n");
        self.output.push_str("          \n");
        self.output.push_str("          ;; Get last digit\n");
        self.output.push_str("          local.get $num\n");
        self.output.push_str("          i32.const 10\n");
        self.output.push_str("          i32.rem_u\n");
        self.output.push_str("          local.set $digit\n");
        self.output.push_str("          \n");
        self.output.push_str("          ;; Store digit character\n");
        self.output.push_str("          local.get $buffer_start\n");
        self.output.push_str("          i32.const 1\n");
        self.output.push_str("          i32.sub\n");
        self.output.push_str("          local.tee $buffer_start\n");
        self.output.push_str("          local.get $digit\n");
        self.output.push_str("          i32.const 48  ;; '0'\n");
        self.output.push_str("          i32.add\n");
        self.output.push_str("          i32.store8\n");
        self.output.push_str("          \n");
        self.output.push_str("          ;; Divide by 10\n");
        self.output.push_str("          local.get $num\n");
        self.output.push_str("          i32.const 10\n");
        self.output.push_str("          i32.div_u\n");
        self.output.push_str("          local.set $num\n");
        self.output.push_str("          \n");
        self.output.push_str("            br $digit_loop\n");
        self.output.push_str("          )\n");
        self.output.push_str("        )\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Add negative sign if needed\n");
        self.output.push_str("    local.get $is_negative\n");
        self.output.push_str("    (if\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        local.get $buffer_start\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.sub\n");
        self.output.push_str("        local.tee $buffer_start\n");
        self.output.push_str("        i32.const 45  ;; '-'\n");
        self.output.push_str("        i32.store8\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Add newline\n");
        self.output.push_str("    local.get $buffer_end\n");
        self.output.push_str("    i32.const 10  ;; '\\n'\n");
        self.output.push_str("    i32.store8\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Calculate length\n");
        self.output.push_str("    local.get $buffer_end\n");
        self.output.push_str("    local.get $buffer_start\n");
        self.output.push_str("    i32.sub\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.add  ;; +1 for newline\n");
        self.output.push_str("    local.set $len\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Setup iovec\n");
        self.output.push_str("    i32.const 200\n");
        self.output.push_str("    local.get $buffer_start  ;; iov_base\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    i32.const 204\n");
        self.output.push_str("    local.get $len          ;; iov_len\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Call fd_write\n");
        self.output.push_str("    i32.const 1   ;; stdout\n");
        self.output.push_str("    i32.const 200 ;; iovec\n");
        self.output.push_str("    i32.const 1   ;; iovec count\n");
        self.output.push_str("    i32.const 300 ;; nwritten\n");
        self.output.push_str("    call $fd_write\n");
        self.output.push_str("    drop\n");
        self.output.push_str("  )\n");
        
        // println function with generic dispatch
        self.output.push_str("\n  ;; Generic println function\n");
        self.output.push_str("  (func $println_generic (param $value i32) (param $type_tag i32)\n");
        self.output.push_str("    ;; Dispatch based on type tag\n");
        self.output.push_str("    ;; 0 = String, 1 = Int32\n");
        self.output.push_str("    local.get $type_tag\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    i32.eq\n");
        self.output.push_str("    (if\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        ;; String case\n");
        self.output.push_str("        local.get $value\n");
        self.output.push_str("        call $println\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        ;; Int32 case\n");
        self.output.push_str("        local.get $value\n");
        self.output.push_str("        call $print_int\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n");

        // Add println to function signatures  
        self.functions.insert("println".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: None,
        });
        
        // Add print_int to function signatures
        self.functions.insert("print_int".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: None,
        });
        
        Ok(())
    }
    
    fn generate_arena_functions(&mut self) -> Result<(), CodeGenError> {
        self.output.push_str("\n  ;; Arena allocator functions\n");
        
        // Global variable to track current arena
        self.output.push_str("  (global $current_arena (mut i32) (i32.const 0))\n\n");
        
        // Arena init function
        self.output.push_str("  (func $arena_init (param $start i32) (result i32)\n");
        self.output.push_str("    ;; Initialize arena header\n");
        self.output.push_str("    ;; Store start address at offset 0\n");
        self.output.push_str("    local.get $start\n");
        self.output.push_str("    local.get $start\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    ;; Store current address at offset 4 (start + 8 for header)\n");
        self.output.push_str("    local.get $start\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $start\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    ;; Return arena header address\n");
        self.output.push_str("    local.get $start\n");
        self.output.push_str("  )\n");
        
        // Add function signatures
        self.functions.insert("arena_init".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: Some(WasmType::I32),
        });
        
        // Arena alloc function
        self.output.push_str("  (func $arena_alloc (param $arena i32) (param $size i32) (result i32)\n");
        self.output.push_str("    (local $current i32)\n");
        self.output.push_str("    (local $aligned_size i32)\n");
        self.output.push_str("    (local $new_current i32)\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Load current pointer\n");
        self.output.push_str("    local.get $arena\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $current\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Align size to 4 bytes\n");
        self.output.push_str("    local.get $size\n");
        self.output.push_str("    i32.const 3\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.const -4\n");
        self.output.push_str("    i32.and\n");
        self.output.push_str("    local.set $aligned_size\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Calculate new current\n");
        self.output.push_str("    local.get $current\n");
        self.output.push_str("    local.get $aligned_size\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.set $new_current\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; TODO: Add bounds checking\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Update current pointer\n");
        self.output.push_str("    local.get $arena\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $new_current\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Return allocated address\n");
        self.output.push_str("    local.get $current\n");
        self.output.push_str("  )\n");
        
        self.functions.insert("arena_alloc".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });
        
        // Arena reset function
        self.output.push_str("  (func $arena_reset (param $arena i32)\n");
        self.output.push_str("    ;; Reset current to start + 8 (after header)\n");
        self.output.push_str("    local.get $arena\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $arena\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("  )\n");
        
        self.functions.insert("arena_reset".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: None,
        });
        
        // Allocate function (uses current arena)
        self.output.push_str("  (func $allocate (param $size i32) (result i32)\n");
        self.output.push_str("    ;; Use current arena or fail if none\n");
        self.output.push_str("    global.get $current_arena\n");
        self.output.push_str("    local.get $size\n");
        self.output.push_str("    call $arena_alloc\n");
        self.output.push_str("  )\n");
        
        self.functions.insert("allocate".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: Some(WasmType::I32),
        });
        
        Ok(())
    }
    
    fn generate_list_functions(&mut self) -> Result<(), CodeGenError> {
        self.output.push_str("\n  ;; List operation functions\n");
        
        // List length function
        self.output.push_str("  (func $list_length (param $list i32) (result i32)\n");
        self.output.push_str("    ;; Load length from list header (offset 0)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("  )\n");
        
        self.functions.insert("list_length".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: Some(WasmType::I32),
        });
        
        // List get function
        self.output.push_str("  (func $list_get (param $list i32) (param $index i32) (result i32)\n");
        self.output.push_str("    (local $length i32)\n");
        self.output.push_str("    ;; Load length for bounds check\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $length\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Bounds check\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.ge_u\n");
        self.output.push_str("    (if\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        ;; Index out of bounds - trap\n");
        self.output.push_str("        unreachable\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Calculate element address: list + 8 + (index * 4)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("  )\n");
        
        self.functions.insert("list_get".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });
        
        // Tail function
        self.output.push_str("  (func $tail (param $list i32) (result i32)\n");
        self.output.push_str("    (local $length i32)\n");
        self.output.push_str("    (local $new_list i32)\n");
        self.output.push_str("    (local $new_length i32)\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Load original length\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $length\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Check if list is empty\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.eqz\n");
        self.output.push_str("    (if\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        ;; Return the same empty list\n");
        self.output.push_str("        local.get $list\n");
        self.output.push_str("        return\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Calculate new length\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.sub\n");
        self.output.push_str("    local.set $new_length\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Allocate new list: 8 bytes header + (new_length * 4) bytes data\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $new_list\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Write new length\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Write new capacity (same as length)\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Copy elements from original list (skip first element)\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    ;; destination\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.const 12\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    ;; source\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    ;; size\n");
        self.output.push_str("    memory.copy\n");
        self.output.push_str("    \n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("  )\n");
        
        self.functions.insert("tail".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: Some(WasmType::I32),
        });
        
        Ok(())
    }
    
    fn generate_array_functions(&mut self) -> Result<(), CodeGenError> {
        self.output.push_str("\n  ;; Array operation functions\n");
        
        // Array get function
        self.output.push_str("  (func $array_get (param $array i32) (param $index i32) (result i32)\n");
        self.output.push_str("    ;; Calculate element address: array + (index * 4)\n");
        self.output.push_str("    local.get $array\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("  )\n");
        
        self.functions.insert("array_get".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });
        
        // Array set function
        self.output.push_str("  (func $array_set (param $array i32) (param $index i32) (param $value i32)\n");
        self.output.push_str("    ;; Calculate element address: array + (index * 4)\n");
        self.output.push_str("    local.get $array\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $value\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("  )\n");
        
        self.functions.insert("array_set".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32, WasmType::I32],
            result: None,
        });
        
        Ok(())
    }
    
    fn collect_strings(&mut self, program: &Program) -> Result<(), CodeGenError> {
        for decl in &program.declarations {
            match decl {
                TopDecl::Function(func) => {
                    self.collect_strings_from_block(&func.body)?;
                }
                TopDecl::Binding(val) => {
                    self.collect_strings_from_expr(&val.value)?;
                }
                TopDecl::Record(_record) => {
                    // Records don't have methods in the current AST
                }
                TopDecl::Export(_) => {
                    // Not yet implemented
                }
                TopDecl::Impl(_) | TopDecl::Context(_) => {
                    // Not yet implemented
                }
            }
        }
        Ok(())
    }
    
    fn collect_strings_from_block(&mut self, block: &BlockExpr) -> Result<(), CodeGenError> {
        for stmt in &block.statements {
            match stmt {
                Stmt::Binding(bind) => {
                    self.collect_strings_from_expr(&bind.value)?;
                }
                Stmt::Assignment(assign) => {
                    self.collect_strings_from_expr(&assign.value)?;
                }
                Stmt::Expr(expr) => {
                    self.collect_strings_from_expr(expr)?;
                }
            }
        }
        
        if let Some(expr) = &block.expr {
            self.collect_strings_from_expr(expr)?;
        }
        
        Ok(())
    }
    
    fn collect_strings_from_expr(&mut self, expr: &Expr) -> Result<(), CodeGenError> {
        match expr {
            Expr::StringLit(s) => {
                if !self.string_offsets.contains_key(s) {
                    let offset = self.next_mem_offset;
                    self.string_offsets.insert(s.clone(), offset);
                    self.strings.push(s.clone());
                    // Account for length prefix (4 bytes) + string data
                    self.next_mem_offset += 4 + s.len() as u32;
                    // Align to 4 bytes
                    self.next_mem_offset = (self.next_mem_offset + 3) & !3;
                }
            }
            Expr::Block(block) => {
                self.collect_strings_from_block(block)?;
            }
            Expr::Call(call) => {
                self.collect_strings_from_expr(&call.function)?;
                for arg in &call.args {
                    self.collect_strings_from_expr(arg)?;
                }
            }
            Expr::Binary(binary) => {
                self.collect_strings_from_expr(&binary.left)?;
                self.collect_strings_from_expr(&binary.right)?;
            }
            Expr::Pipe(pipe) => {
                self.collect_strings_from_expr(&pipe.expr)?;
                if let PipeTarget::Expr(target) = &pipe.target {
                    self.collect_strings_from_expr(target)?;
                }
            }
            Expr::RecordLit(record) => {
                for field in &record.fields {
                    self.collect_strings_from_expr(&field.value)?;
                }
            }
            Expr::FieldAccess(expr, _) => {
                self.collect_strings_from_expr(expr)?;
            }
            Expr::ListLit(items) => {
                for item in items {
                    self.collect_strings_from_expr(item)?;
                }
            }
            Expr::ArrayLit(items) => {
                for item in items {
                    self.collect_strings_from_expr(item)?;
                }
            }
            Expr::Match(match_expr) => {
                self.collect_strings_from_expr(&match_expr.expr)?;
                for arm in &match_expr.arms {
                    self.collect_strings_from_block(&arm.body)?;
                }
            }
            Expr::Then(then) => {
                self.collect_strings_from_expr(&then.condition)?;
                self.collect_strings_from_block(&then.then_block)?;
                for (cond, block) in &then.else_ifs {
                    self.collect_strings_from_expr(cond)?;
                    self.collect_strings_from_block(block)?;
                }
                if let Some(block) = &then.else_block {
                    self.collect_strings_from_block(block)?;
                }
            }
            Expr::While(while_expr) => {
                self.collect_strings_from_expr(&while_expr.condition)?;
                self.collect_strings_from_block(&while_expr.body)?;
            }
            Expr::With(with) => {
                self.collect_strings_from_block(&with.body)?;
            }
            Expr::Clone(clone) => {
                self.collect_strings_from_expr(&clone.base)?;
                for field in &clone.updates.fields {
                    self.collect_strings_from_expr(&field.value)?;
                }
            }
            Expr::Freeze(expr) => {
                self.collect_strings_from_expr(expr)?;
            }
            Expr::Lambda(lambda) => {
                self.collect_strings_from_expr(&lambda.body)?;
            }
            _ => {}
        }
        Ok(())
    }
    
    fn register_function_signature(&mut self, func: &FunDecl) -> Result<(), CodeGenError> {
        // Handle generic functions
        if !func.type_params.is_empty() {
            // For now, we'll generate specialized versions when called
            return Ok(());
        }
        
        let params: Vec<WasmType> = func.params.iter()
            .map(|_| WasmType::I32)  // All types are i32 for now
            .collect();
        
        // TODO: Determine return type from function body analysis
        // For now, assume functions with expressions in their body return i32
        // Exception: main function never returns a value
        let result = if func.name == "main" {
            None
        } else if func.body.expr.is_some() {
            Some(WasmType::I32)
        } else {
            None
        };
        
        self.functions.insert(func.name.clone(), FunctionSig {
            _params: params,
            result,
        });
        
        Ok(())
    }
    
    fn register_record_methods(&mut self, _record: &RecordDecl) -> Result<(), CodeGenError> {
        // Records don't have methods in the current AST
        Ok(())
    }
    
    fn generate_record_methods(&mut self, _record: &RecordDecl) -> Result<(), CodeGenError> {
        // Records don't have methods in the current AST
        Ok(())
    }
    
    fn convert_type(&self, ty: &Type) -> Result<WasmType, CodeGenError> {
        match ty {
            Type::Named(name) => {
                match name.as_str() {
                    "Int32" | "Boolean" | "Char" => Ok(WasmType::I32),
                    "Float64" => Ok(WasmType::F64),
                    "String" => Ok(WasmType::I32), // String is a pointer
                    _ => Ok(WasmType::I32), // Records and other types are pointers
                }
            }
            Type::Generic(name, _params) => {
                match name.as_str() {
                    "List" | "Option" | "Array" => Ok(WasmType::I32), // All are pointers
                    _ => Ok(WasmType::I32), // Default to pointer
                }
            }
            Type::Function(_, _) => Ok(WasmType::I32), // Function pointers
        }
    }
    
    // Generate specialized versions of generic functions
    fn generate_generic_function(&mut self, func: &FunDecl) -> Result<(), CodeGenError> {
        // Handle special generic functions
        match func.name.as_str() {
            "println" => self.generate_println_specializations(func),
            "new_list" => self.generate_new_list_specializations(func),
            "list_add" => self.generate_list_add_specializations(func),
            "some" => self.generate_some_specializations(func),
            "none" => self.generate_none_specializations(func),
            _ => {
                // For other generic functions, we'll need to generate on demand
                // This is a placeholder for future monomorphization
                Ok(())
            }
        }
    }
    
    fn generate_println_specializations(&mut self, func: &FunDecl) -> Result<(), CodeGenError> {
        // Generate println_String specialization
        let string_func = FunDecl {
            name: "println_String".to_string(),
            type_params: vec![],
            params: vec![Param {
                name: func.params[0].name.clone(),
                ty: Type::Named("String".to_string()),
                context_bound: None,
            }],
            body: BlockExpr {
                statements: vec![],
                expr: Some(Box::new(Expr::Call(CallExpr {
                    function: Box::new(Expr::Ident("println".to_string())), // Call built-in println
                    args: vec![Box::new(Expr::Ident(func.params[0].name.clone()))],
                }))),
            },
        };
        
        // Generate println_Int32 specialization
        let int_func = FunDecl {
            name: "println_Int32".to_string(),
            type_params: vec![],
            params: vec![Param {
                name: func.params[0].name.clone(),
                ty: Type::Named("Int32".to_string()),
                context_bound: None,
            }],
            body: BlockExpr {
                statements: vec![],
                expr: Some(Box::new(Expr::Call(CallExpr {
                    function: Box::new(Expr::Ident("print_int".to_string())), // Call built-in print_int
                    args: vec![Box::new(Expr::Ident(func.params[0].name.clone()))],
                }))),
            },
        };
        
        // Generate the specialized functions
        self.generate_function(&string_func)?;
        self.generate_function(&int_func)?;
        
        Ok(())
    }
    
    fn generate_function(&mut self, func: &FunDecl) -> Result<(), CodeGenError> {
        // If this is a generic function, generate specialized versions
        if !func.type_params.is_empty() {
            return self.generate_generic_function(func);
        }
        
        self.current_function = Some(func.name.clone());
        self.push_scope();
        
        // First, collect all local variables by analyzing the function body
        let mut locals: Vec<(String, WasmType)> = Vec::new();
        self.collect_locals_from_block(&func.body, &mut locals)?;
        
        // Function header
        self.output.push_str(&format!("  (func ${}", func.name));
        
        // Parameters
        let mut next_idx = 0u32;
        for param in func.params.iter() {
            let wasm_type = self.convert_type(&param.ty)?;
            self.output.push_str(&format!(" (param ${} {})", param.name, self.wasm_type_str(wasm_type)));
            self.add_local(&param.name, next_idx);
            next_idx += 1;
        }
        
        // Result type
        if let Some(sig) = self.functions.get(&func.name) {
            if let Some(result_type) = sig.result {
                self.output.push_str(&format!(" (result {})", self.wasm_type_str(result_type)));
            }
        }
        
        self.output.push_str("\n");
        
        // Declare all local variables
        for (name, ty) in &locals {
            self.output.push_str(&format!("    (local ${} {})\n", name, self.wasm_type_str(*ty)));
            self.add_local(name, next_idx);
            next_idx += 1;
        }
        
        // Add temporary variable for match expressions
        // TODO: Only add when needed
        // Add temporary variable for list literals
        self.output.push_str("    (local $list_tmp i32)\n");
        self.add_local("list_tmp", next_idx);
        next_idx += 1;
        
        // Add temporary variables for match expressions
        self.output.push_str("    (local $match_tmp i32)\n");
        self.add_local("match_tmp", next_idx);
        next_idx += 1;
        
        self.output.push_str("    (local $tail_len i32)\n");
        self.add_local("tail_len", next_idx);
        next_idx += 1;
        
        self.output.push_str("    (local $tail_tmp i32)\n");
        self.add_local("tail_tmp", next_idx);
        next_idx += 1;
        
        // Add temporary variable for closures
        self.output.push_str("    (local $closure_tmp i32)\n");
        self.add_local("closure_tmp", next_idx);
        next_idx += 1;
        
        // Add temporary variables for clone expressions
        self.output.push_str("    (local $clone_tmp i32)\n");
        self.add_local("clone_tmp", next_idx);
        next_idx += 1;
        
        self.output.push_str("    (local $base_tmp i32)\n");
        self.add_local("base_tmp", next_idx);
        next_idx += 1;
        
        // Add temporary variable for freeze expressions
        self.output.push_str("    (local $freeze_tmp i32)\n");
        self.add_local("freeze_tmp", next_idx);
        next_idx += 1;
        
        // Hack: Add common pattern variable names (only if not already declared)
        for var_name in ["n", "x", "y", "z", "a", "b", "c", "head", "tail", "rest"] {
            // Check if this variable is already declared as a parameter or local
            let already_parameter = func.params.iter().any(|p| p.name == var_name);
            let already_local = locals.iter().any(|(name, _)| name == var_name);
            if !already_parameter && !already_local {
                self.output.push_str(&format!("    (local ${} i32)\n", var_name));
                self.add_local(var_name, next_idx);
                next_idx += 1;
            }
        }
        
        // Initialize default arena for main function
        if func.name == "main" {
            self.default_arena = Some(self.next_arena_addr);
            self.output.push_str(&format!("    ;; Initialize default arena\n"));
            self.output.push_str(&format!("    i32.const {}\n", self.next_arena_addr));
            self.output.push_str("    call $arena_init\n");
            self.output.push_str("    global.set $current_arena\n\n");
        }
        
        // Generate function body
        self.generate_block(&func.body)?;
        
        // Drop return value for main function if it leaves a value
        if func.name == "main" && func.body.expr.is_some() {
            if let Some(expr) = &func.body.expr {
                if self.expr_leaves_value(expr) {
                    self.output.push_str("    drop\n");
                }
            }
        }
        
        // Reset default arena at the end of main
        if func.name == "main" && self.default_arena.is_some() {
            self.output.push_str(&format!("\n    ;; Reset default arena\n"));
            self.output.push_str(&format!("    i32.const {}\n", self.default_arena.unwrap()));
            self.output.push_str("    call $arena_reset\n");
        }
        
        self.output.push_str("  )\n");
        
        self.pop_scope();
        self.current_function = None;
        Ok(())
    }
    
    fn generate_block(&mut self, block: &BlockExpr) -> Result<(), CodeGenError> {
        self.generate_block_internal(block, false)
    }
    
    fn generate_block_as_expression(&mut self, block: &BlockExpr) -> Result<(), CodeGenError> {
        self.generate_block_internal(block, true)
    }
    
    fn generate_block_internal(&mut self, block: &BlockExpr, as_expression: bool) -> Result<(), CodeGenError> {
        // Generate statements
        for (i, stmt) in block.statements.iter().enumerate() {
            let is_last_stmt = i == block.statements.len() - 1;
            match stmt {
                Stmt::Binding(bind) => self.generate_binding(bind)?,
                Stmt::Assignment(assign) => self.generate_assignment(assign)?,
                Stmt::Expr(expr) => {
                    self.generate_expr(expr)?;
                    // Pop the result if it's not the last expression and the expression leaves a value
                    let should_drop = if as_expression {
                        // In expression context, only drop if not the final value
                        !is_last_stmt || block.expr.is_some()
                    } else {
                        // In statement context, drop unless it's the final expression
                        (block.expr.is_some() || !is_last_stmt) && self.expr_leaves_value(expr)
                    };
                    if should_drop && self.expr_leaves_value(expr) {
                        self.output.push_str("    drop\n");
                    }
                }
            }
        }
        
        // Generate return expression
        if let Some(expr) = &block.expr {
            self.generate_expr(expr)?;
        } else if block.statements.is_empty() && !as_expression {
            // Empty block returns 0 (Unit) only in statement context
            self.output.push_str("    i32.const 0\n");
        }
        
        Ok(())
    }
    
    fn generate_binding(&mut self, bind: &BindDecl) -> Result<(), CodeGenError> {
        // Generate the value expression
        self.generate_expr(&bind.value)?;
        
        // Store in local (it should already be declared and registered)
        if self.lookup_local(&bind.name).is_none() {
            return Err(CodeGenError::UndefinedVariable(bind.name.clone()));
        }
        
        self.output.push_str(&format!("    local.set ${}\n", bind.name));
        
        Ok(())
    }
    
    fn generate_assignment(&mut self, assign: &AssignStmt) -> Result<(), CodeGenError> {
        // Generate the value expression
        self.generate_expr(&assign.value)?;
        
        // Store in local
        if self.lookup_local(&assign.name).is_some() {
            self.output.push_str(&format!("    local.set ${}\n", assign.name));
        } else {
            return Err(CodeGenError::UndefinedVariable(assign.name.clone()));
        }
        
        Ok(())
    }
    
    fn generate_expr(&mut self, expr: &Expr) -> Result<(), CodeGenError> {
        match expr {
            Expr::IntLit(n) => {
                self.output.push_str(&format!("    i32.const {}\n", n));
            }
            Expr::FloatLit(f) => {
                self.output.push_str(&format!("    f64.const {}\n", f));
            }
            Expr::BoolLit(b) => {
                self.output.push_str(&format!("    i32.const {}\n", if *b { 1 } else { 0 }));
            }
            Expr::Unit => {
                self.output.push_str("    i32.const 0\n");
            }
            Expr::Ident(name) => {
                // Check if it's a captured variable in a lambda
                if self.in_lambda_with_captures && self.captured_vars.contains(name) {
                    self.output.push_str(&format!("    local.get ${}_captured\n", name));
                } else if let Some(_idx) = self.lookup_local(name) {
                    self.output.push_str(&format!("    local.get ${}\n", name));
                } else if self.functions.contains_key(name) {
                    // It's a zero-argument function call
                    self.output.push_str(&format!("    call ${}\n", name));
                } else {
                    return Err(CodeGenError::UndefinedVariable(name.clone()));
                }
            }
            Expr::Binary(binary) => {
                self.generate_binary_expr(binary)?;
            }
            Expr::Call(call) => {
                self.generate_call_expr(call)?;
            }
            Expr::Block(block) => {
                self.generate_block(block)?;
            }
            Expr::RecordLit(record_lit) => {
                // For now, we'll use a simple implementation
                // Allocate memory and store fields
                // In a real implementation, we'd have a proper memory allocator
                
                // For simplicity, use a fixed address (this is not production-ready!)
                self.output.push_str("    i32.const 1024\n"); // Base address
                
                // Store each field value
                let mut offset = 0;
                for field in &record_lit.fields {
                    self.output.push_str("    i32.const 1024\n");
                    self.output.push_str(&format!("    i32.const {}\n", offset));
                    self.output.push_str("    i32.add\n");
                    self.generate_expr(&field.value)?;
                    self.output.push_str("    i32.store\n");
                    offset += 4; // Assume all fields are i32
                }
                
                // Return the base address
                self.output.push_str("    i32.const 1024\n");
            }
            Expr::FieldAccess(obj_expr, field) => {
                // Generate object expression
                self.generate_expr(obj_expr)?;
                
                // For now, assume simple field offset calculation
                // In a real implementation, we'd need type information
                let field_offset = match field.as_str() {
                    "x" => 0,
                    "y" => 4,
                    _ => return Err(CodeGenError::NotImplemented(format!("field access for {}", field))),
                };
                
                self.output.push_str(&format!("    i32.const {}\n", field_offset));
                self.output.push_str("    i32.add\n");
                self.output.push_str("    i32.load\n");
            }
            Expr::StringLit(s) => {
                if let Some(offset) = self.string_offsets.get(s) {
                    self.output.push_str(&format!("    i32.const {}\n", offset));
                } else {
                    return Err(CodeGenError::NotImplemented("string literal not in pool".to_string()));
                }
            }
            Expr::CharLit(_c) => {
                return Err(CodeGenError::NotImplemented("char literals".to_string()));
            }
            Expr::Pipe(pipe) => {
                self.generate_pipe_expr(pipe)?;
            }
            Expr::ListLit(items) => {
                self.generate_list_literal(items)?;
            }
            Expr::ArrayLit(items) => {
                self.generate_array_literal(items)?;
            }
            Expr::Match(match_expr) => {
                self.generate_match_expr(match_expr)?;
            }
            Expr::Then(then) => {
                self.generate_then_expr(then)?;
            }
            Expr::While(while_expr) => {
                self.generate_while_expr(while_expr)?;
            }
            Expr::With(_) => {
                return Err(CodeGenError::NotImplemented("with expressions".to_string()));
            }
            Expr::Clone(clone) => {
                self.generate_clone_expr(clone)?;
            }
            Expr::Freeze(expr) => {
                self.generate_freeze_expr(expr)?;
            }
            Expr::None => {
                // Tagged union: allocate 8 bytes (4 for tag, 4 for padding)
                self.output.push_str("    ;; None literal\n");
                self.output.push_str("    i32.const 8\n");
                self.output.push_str("    call $allocate\n");
                self.output.push_str("    local.tee $match_tmp\n");
                
                // Store tag (0 for None)
                self.output.push_str("    i32.const 0\n");
                self.output.push_str("    i32.store\n");
                
                // Leave pointer on stack
                self.output.push_str("    local.get $match_tmp\n");
            }
            Expr::Some(inner) => {
                // Generate the inner value first
                self.generate_expr(inner)?;
                
                // Tagged union: allocate 8 bytes (4 for tag, 4 for value)
                self.output.push_str("    ;; Some literal\n");
                self.output.push_str("    i32.const 8\n");
                self.output.push_str("    call $allocate\n");
                self.output.push_str("    local.tee $match_tmp\n");
                
                // Store tag (1 for Some)
                self.output.push_str("    i32.const 1\n");
                self.output.push_str("    i32.store\n");
                
                // Store value at offset 4
                self.output.push_str("    local.get $match_tmp\n");
                self.output.push_str("    i32.const 4\n");
                self.output.push_str("    i32.add\n");
                // The value is already on the stack
                self.output.push_str("    i32.store\n");
                
                // Leave pointer on stack
                self.output.push_str("    local.get $match_tmp\n");
            }
            Expr::Lambda(lambda) => {
                self.generate_lambda_expr(lambda)?;
            }
            Expr::PrototypeClone(proto_clone) => {
                self.generate_prototype_clone_expr(proto_clone)?;
            }
            Expr::NoneTyped(_ty) => {
                // Tagged union: allocate 8 bytes (4 for tag, 4 for padding)
                self.output.push_str("    ;; None<T> literal\n");
                self.output.push_str("    i32.const 8\n");
                self.output.push_str("    call $allocate\n");
                self.output.push_str("    local.tee $match_tmp\n");
                
                // Store tag (0 for None)
                self.output.push_str("    i32.const 0\n");
                self.output.push_str("    i32.store\n");
                
                // Leave pointer on stack
                self.output.push_str("    local.get $match_tmp\n");
            }
        }
        Ok(())
    }
    
    fn generate_lambda_expr(&mut self, lambda: &LambdaExpr) -> Result<(), CodeGenError> {
        // Generate a unique name for this lambda
        let lambda_name = format!("lambda_{}", self.lambda_counter);
        self.lambda_counter += 1;
        
        // Add to function table for indirect calls
        let table_index = self.function_table.len();
        self.function_table.push(lambda_name.clone());
        
        // Analyze free variables (variables captured from outer scope)
        let free_vars = self.analyze_free_variables(lambda)?;
        
        if free_vars.is_empty() {
            // Simple case: no captured variables, just return function index
            self.output.push_str(&format!("    i32.const {} ;; function table index for {}\n", table_index, lambda_name));
        } else {
            // Complex case: need to create closure with captured variables
            // Closure layout: [function_index, captured_var1, captured_var2, ...]
            let closure_size = 4 + (free_vars.len() * 4); // 4 bytes per captured variable
            
            // Allocate closure
            self.output.push_str(&format!("    i32.const {} ;; closure size\n", closure_size));
            self.output.push_str("    call $allocate\n");
            self.output.push_str("    local.set $closure_tmp\n");
            
            // Store function index
            self.output.push_str("    local.get $closure_tmp\n");
            self.output.push_str(&format!("    i32.const {} ;; function table index\n", table_index));
            self.output.push_str("    i32.store\n");
            
            // Store captured variables
            let mut offset = 4;
            for (i, (var_name, _)) in free_vars.iter().enumerate() {
                self.output.push_str("    local.get $closure_tmp\n");
                self.output.push_str(&format!("    i32.const {} ;; offset for captured var {}\n", offset, i));
                self.output.push_str("    i32.add\n");
                self.output.push_str(&format!("    local.get ${}\n", var_name));
                self.output.push_str("    i32.store\n");
                offset += 4;
            }
            
            // Return closure pointer
            self.output.push_str("    local.get $closure_tmp\n");
        }
        
        // Generate the lambda function separately
        let mut lambda_code = String::new();
        lambda_code.push_str(&format!("  (func ${}", lambda_name));
        
        // Parameters
        for param in lambda.params.iter() {
            lambda_code.push_str(&format!(" (param ${} i32)", param));
        }
        
        // If we have captured variables, add them as additional parameters
        if !free_vars.is_empty() {
            lambda_code.push_str(" (param $closure i32)");
        }
        
        // Result type (for now, always i32)
        lambda_code.push_str(" (result i32)\n");
        
        // Generate local declarations for captured variables
        if !free_vars.is_empty() {
            for (var_name, _) in &free_vars {
                lambda_code.push_str(&format!("    (local ${}_captured i32)\n", var_name));
            }
            
            // Load captured variables from closure
            let mut offset = 4;
            for (var_name, _) in &free_vars {
                lambda_code.push_str("    local.get $closure\n");
                lambda_code.push_str(&format!("    i32.const {}\n", offset));
                lambda_code.push_str("    i32.add\n");
                lambda_code.push_str("    i32.load\n");
                lambda_code.push_str(&format!("    local.set ${}_captured\n", var_name));
                offset += 4;
            }
        }
        
        // Generate lambda body with captured variable context
        let old_in_lambda = self.in_lambda_with_captures;
        let old_captured_vars = self.captured_vars.clone();
        
        self.in_lambda_with_captures = !free_vars.is_empty();
        self.captured_vars = free_vars.iter().map(|(name, _)| name.clone()).collect();
        
        // Save current output and switch to lambda code
        let saved_output = std::mem::replace(&mut self.output, lambda_code);
        
        // Set up local scope for lambda
        self.push_scope();
        for (i, param) in lambda.params.iter().enumerate() {
            self.add_local(param, i as u32);
        }
        
        // Generate lambda body
        self.generate_expr(&lambda.body)?;
        
        self.pop_scope();
        
        // Restore output and save lambda code
        lambda_code = std::mem::replace(&mut self.output, saved_output);
        lambda_code.push_str("  )\n");
        
        self.in_lambda_with_captures = old_in_lambda;
        self.captured_vars = old_captured_vars;
        
        // Add lambda function to the list
        self.lambda_functions.push(lambda_code);
        
        Ok(())
    }
    
    fn analyze_free_variables(&self, _lambda: &LambdaExpr) -> Result<Vec<(String, WasmType)>, CodeGenError> {
        // TODO: Implement proper free variable analysis
        // For now, return empty list
        Ok(Vec::new())
    }
    
    fn generate_binary_expr(&mut self, binary: &BinaryExpr) -> Result<(), CodeGenError> {
        // Generate left operand
        self.generate_expr(&binary.left)?;
        
        // Generate right operand
        self.generate_expr(&binary.right)?;
        
        // Generate operation
        match binary.op {
            BinaryOp::Add => self.output.push_str("    i32.add\n"),
            BinaryOp::Sub => self.output.push_str("    i32.sub\n"),
            BinaryOp::Mul => self.output.push_str("    i32.mul\n"),
            BinaryOp::Div => self.output.push_str("    i32.div_s\n"),
            BinaryOp::Mod => self.output.push_str("    i32.rem_s\n"),
            BinaryOp::Eq => {
                self.output.push_str("    i32.eq\n");
            }
            BinaryOp::Ne => {
                self.output.push_str("    i32.ne\n");
            }
            BinaryOp::Lt => {
                self.output.push_str("    i32.lt_s\n");
            }
            BinaryOp::Gt => {
                self.output.push_str("    i32.gt_s\n");
            }
            BinaryOp::Le => {
                self.output.push_str("    i32.le_s\n");
            }
            BinaryOp::Ge => {
                self.output.push_str("    i32.ge_s\n");
            }
        }
        
        Ok(())
    }
    
    fn generate_call_expr(&mut self, call: &CallExpr) -> Result<(), CodeGenError> {
        // Generate arguments first
        for arg in &call.args {
            self.generate_expr(arg)?;
        }
        
        // Handle function call
        if let Expr::Ident(func_name) = &*call.function {
            // Handle special built-in function 'some'
            if func_name == "some" {
                // Tagged union: allocate 8 bytes (4 for tag, 4 for value)
                self.output.push_str("    ;; Some constructor\n");
                self.output.push_str("    i32.const 8\n");
                self.output.push_str("    call $allocate\n");
                self.output.push_str("    local.tee $match_tmp\n");
                
                // Store tag (1 for Some)
                self.output.push_str("    i32.const 1\n");
                self.output.push_str("    i32.store\n");
                
                // Store value at offset 4
                self.output.push_str("    local.get $match_tmp\n");
                self.output.push_str("    i32.const 4\n");
                self.output.push_str("    i32.add\n");
                // The value is already on the stack from the argument
                self.output.push_str("    i32.store\n");
                
                // Return pointer to the Option
                self.output.push_str("    local.get $match_tmp\n");
                return Ok(());
            }
            
            // Handle special built-in function 'none'
            if func_name == "none" {
                // Tagged union: allocate 8 bytes (4 for tag, 4 for padding)
                self.output.push_str("    ;; None constructor\n");
                self.output.push_str("    i32.const 8\n");
                self.output.push_str("    call $allocate\n");
                self.output.push_str("    local.tee $match_tmp\n");
                
                // Store tag (0 for None)
                self.output.push_str("    i32.const 0\n");
                self.output.push_str("    i32.store\n");
                
                // Return pointer to the Option
                self.output.push_str("    local.get $match_tmp\n");
                return Ok(());
            }
            
            if self.functions.contains_key(func_name) {
                self.output.push_str(&format!("    call ${}\n", func_name));
            } else {
                // Check if it's a method call
                if let Some(obj_expr) = call.args.first() {
                    // Try to determine the record type from the expression
                    if let Some(record_type) = self.get_expr_type(obj_expr) {
                        if let Some(methods) = self.methods.get(&record_type) {
                            if methods.contains_key(func_name) {
                                let mangled_name = format!("{}_{}", record_type, func_name);
                                self.output.push_str(&format!("    call ${}\n", mangled_name));
                                return Ok(());
                            } else {
                                return Err(CodeGenError::UndefinedFunction(
                                    format!("Method '{}' not found in record '{}'", func_name, record_type)
                                ));
                            }
                        } else {
                            return Err(CodeGenError::UndefinedFunction(
                                format!("No methods defined for record '{}'", record_type)
                            ));
                        }
                    } else {
                        // No type information - fall back to checking uniqueness
                        let mut found_records = Vec::new();
                        for (record_name, method_map) in &self.methods {
                            if method_map.contains_key(func_name) {
                                found_records.push(record_name.clone());
                            }
                        }
                        
                        if found_records.is_empty() {
                            return Err(CodeGenError::UndefinedFunction(func_name.clone()));
                        } else if found_records.len() > 1 {
                            // Method exists in multiple records - ambiguous without type info
                            return Err(CodeGenError::NotImplemented(
                                format!("Ambiguous method '{}' found in records: {:?}. Type-directed method resolution not yet implemented", 
                                    func_name, found_records)
                            ));
                        } else {
                            // Unique method - safe to call
                            let record_name = &found_records[0];
                            let mangled_name = format!("{}_{}", record_name, func_name);
                            self.output.push_str(&format!("    call ${}\n", mangled_name));
                        }
                    }
                } else {
                    return Err(CodeGenError::UndefinedFunction(func_name.clone()));
                }
            }
        } else {
            // Non-identifier function expression (e.g., field access, or complex expression)
            // Generate the function expression to get the table index
            self.generate_expr(&call.function)?;
            
            // Generate type signature for call_indirect
            let param_count = call.args.len();
            self.output.push_str("    call_indirect (type ");
            
            // Generate type index - for simplicity, we'll inline the type
            self.output.push_str("(func");
            for _ in 0..param_count {
                self.output.push_str(" (param i32)");
            }
            self.output.push_str(" (result i32)))\n");
        }
        
        Ok(())
    }
    
    fn infer_expr_type(&self, expr: &Expr) -> Result<WasmType, CodeGenError> {
        match expr {
            Expr::IntLit(_) => Ok(WasmType::I32),
            Expr::FloatLit(_) => Ok(WasmType::F64),
            Expr::BoolLit(_) => Ok(WasmType::I32),
            Expr::Unit => Ok(WasmType::I32),
            _ => Ok(WasmType::I32), // Default to i32 for now
        }
    }
    
    fn wasm_type_str(&self, ty: WasmType) -> &'static str {
        match ty {
            WasmType::I32 => "i32",
            WasmType::I64 => "i64",
            WasmType::F32 => "f32",
            WasmType::F64 => "f64",
        }
    }
    
    fn push_scope(&mut self) {
        self.locals.push(HashMap::new());
    }
    
    fn pop_scope(&mut self) {
        self.locals.pop();
    }
    
    fn add_local(&mut self, name: &str, idx: u32) {
        if let Some(scope) = self.locals.last_mut() {
            scope.insert(name.to_string(), idx);
        }
    }
    
    fn lookup_local(&self, name: &str) -> Option<u32> {
        for scope in self.locals.iter().rev() {
            if let Some(idx) = scope.get(name) {
                return Some(*idx);
            }
        }
        None
    }
    
    fn bind_local(&mut self, name: &str, idx: u32) {
        self.add_local(name, idx);
    }
    
    #[allow(dead_code)]
    fn next_local_index(&self) -> u32 {
        let mut max_idx = 0;
        for scope in &self.locals {
            for (_, idx) in scope {
                max_idx = max_idx.max(*idx);
            }
        }
        max_idx + 1
    }
    
    fn collect_locals_from_block(&self, block: &BlockExpr, locals: &mut Vec<(String, WasmType)>) -> Result<(), CodeGenError> {
        for stmt in &block.statements {
            match stmt {
                Stmt::Binding(bind) => {
                    let ty = self.infer_expr_type(&bind.value)?;
                    locals.push((bind.name.clone(), ty));
                    // Also collect locals from the value expression
                    self.collect_locals_from_expr(&bind.value, locals)?;
                }
                Stmt::Assignment(_) => {
                    // Assignments don't create new locals
                }
                Stmt::Expr(expr) => {
                    // Check for nested blocks and match expressions
                    self.collect_locals_from_expr(expr, locals)?;
                }
            }
        }
        
        // Check the return expression for nested blocks
        if let Some(expr) = &block.expr {
            self.collect_locals_from_expr(expr, locals)?;
        }
        
        Ok(())
    }
    
    fn expr_leaves_value(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Pipe(pipe) => {
                match &pipe.target {
                    PipeTarget::Ident(name) => {
                        if name == "println" && self.current_function == Some("main".to_string()) {
                            // println in main doesn't leave value
                            false
                        } else if let Some(sig) = self.functions.get(name) {
                            // Function leaves value if it has a return type
                            sig.result.is_some()
                        } else {
                            // Binding leaves value
                            true
                        }
                    }
                    PipeTarget::Expr(target_expr) => {
                        if let Expr::Ident(func_name) = &**target_expr {
                            if let Some(sig) = self.functions.get(func_name) {
                                sig.result.is_some()
                            } else {
                                true
                            }
                        } else {
                            true
                        }
                    }
                }
            }
            Expr::Call(call) => {
                if let Expr::Ident(func_name) = &*call.function {
                    if let Some(sig) = self.functions.get(func_name) {
                        sig.result.is_some()
                    } else {
                        true
                    }
                } else {
                    true
                }
            }
            // Most other expressions leave values
            _ => true,
        }
    }

    fn collect_locals_from_expr(&self, expr: &Expr, locals: &mut Vec<(String, WasmType)>) -> Result<(), CodeGenError> {
        match expr {
            Expr::Block(block) => {
                self.collect_locals_from_block(block, locals)?;
            }
            Expr::Match(match_expr) => {
                // Collect locals from match arms
                for arm in &match_expr.arms {
                    self.collect_locals_from_pattern(&arm.pattern, locals)?;
                    self.collect_locals_from_block(&arm.body, locals)?;
                }
            }
            Expr::Then(then) => {
                self.collect_locals_from_block(&then.then_block, locals)?;
                for (_, block) in &then.else_ifs {
                    self.collect_locals_from_block(block, locals)?;
                }
                if let Some(block) = &then.else_block {
                    self.collect_locals_from_block(block, locals)?;
                }
            }
            Expr::While(while_expr) => {
                self.collect_locals_from_block(&while_expr.body, locals)?;
            }
            Expr::With(with) => {
                self.collect_locals_from_block(&with.body, locals)?;
            }
            Expr::Lambda(lambda) => {
                // Lambda parameters are locals within the lambda, not in outer scope
                // But we might need to collect locals from the lambda body later
                self.collect_locals_from_expr(&lambda.body, locals)?;
            }
            _ => {}
        }
        Ok(())
    }
    
    fn collect_locals_from_pattern(&self, pattern: &Pattern, locals: &mut Vec<(String, WasmType)>) -> Result<(), CodeGenError> {
        match pattern {
            Pattern::Ident(name) => {
                locals.push((name.clone(), WasmType::I32));
            }
            Pattern::ListExact(patterns) => {
                for pattern in patterns {
                    self.collect_locals_from_pattern(pattern, locals)?;
                }
            }
            Pattern::ListCons(head, tail) => {
                self.collect_locals_from_pattern(head, locals)?;
                self.collect_locals_from_pattern(tail, locals)?;
            }
            Pattern::EmptyList => {
                // No locals to collect
            }
            Pattern::Record(_, field_patterns) => {
                for (_, pattern) in field_patterns {
                    self.collect_locals_from_pattern(pattern, locals)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    
    fn generate_pipe_expr(&mut self, pipe: &PipeExpr) -> Result<(), CodeGenError> {
        // Generate the source expression
        self.generate_expr(&pipe.expr)?;
        
        match &pipe.target {
            PipeTarget::Ident(name) => {
                // Check if this is a function or a binding
                if name == "println" {
                    // Special handling for generic println - determine type at runtime
                    let specialized_name = self.resolve_generic_function_call(name, &pipe.expr)?;
                    self.output.push_str(&format!("    call ${}\n", specialized_name));
                    // These functions return nothing, so we need to push unit value for pipe result
                    // But only if we're not in main function (which returns nothing)
                    if self.current_function != Some("main".to_string()) {
                        self.output.push_str("    i32.const 0\n");
                    }
                } else if self.functions.contains_key(name) {
                    // It's a function call: expr |> func
                    self.output.push_str(&format!("    call ${}\n", name));
                    // If function returns nothing, push unit value for pipe result
                    // But only if we're not in main function (which returns nothing)
                    if let Some(sig) = self.functions.get(name) {
                        if sig.result.is_none() && self.current_function != Some("main".to_string()) {
                            self.output.push_str("    i32.const 0\n");
                        }
                    }
                } else if self.lookup_local(name).is_some() {
                    // It's an existing local variable, error
                    return Err(CodeGenError::NotImplemented("pipe to existing variable".to_string()));
                } else {
                    // It's a new binding: expr |> name
                    // This should have been handled by the type checker to add the local
                    self.output.push_str(&format!("    local.set ${}\n", name));
                    // And leave it on the stack as the result
                    self.output.push_str(&format!("    local.get ${}\n", name));
                }
            }
            PipeTarget::Expr(target_expr) => {
                // This is a complex expression
                match &**target_expr {
                    Expr::Ident(func_name) => {
                        // Single argument function call
                        if self.functions.contains_key(func_name) {
                            self.output.push_str(&format!("    call ${}\n", func_name));
                            // If function returns nothing, push unit value for pipe result
                            // But only if we're not in main function (which returns nothing)
                            if let Some(sig) = self.functions.get(func_name) {
                                if sig.result.is_none() && self.current_function != Some("main".to_string()) {
                                    self.output.push_str("    i32.const 0\n");
                                }
                            }
                        } else {
                            return Err(CodeGenError::UndefinedFunction(func_name.clone()));
                        }
                    }
                    _ => return Err(CodeGenError::NotImplemented("complex pipe target".to_string())),
                }
            }
        }
        
        Ok(())
    }
    
    fn resolve_generic_function_call(&self, name: &str, arg_expr: &Expr) -> Result<String, CodeGenError> {
        // For built-in functions, use the original name for string types
        if name == "println" {
            match arg_expr {
                Expr::StringLit(_) => Ok("println".to_string()),
                Expr::IntLit(_) => Ok("print_int".to_string()),
                _ => {
                    // Try to get type from expr_types map
                    if let Some(type_name) = self.expr_types.get(&(arg_expr as *const Expr)) {
                        if type_name == "String" {
                            Ok("println".to_string())
                        } else if type_name == "Int32" || type_name == "Int" {
                            Ok("print_int".to_string())
                        } else {
                            Ok("println".to_string())
                        }
                    } else {
                        // Default to println
                        Ok("println".to_string())
                    }
                }
            }
        } else {
            // For user-defined generic functions, use specialized names
            match arg_expr {
                Expr::StringLit(_) => Ok(format!("{}_String", name)),
                Expr::IntLit(_) => Ok(format!("{}_Int32", name)),
                _ => {
                    // Try to get type from expr_types map
                    if let Some(type_name) = self.expr_types.get(&(arg_expr as *const Expr)) {
                        Ok(format!("{}_{}", name, type_name))
                    } else {
                        // Default to Int32 for now
                        Ok(format!("{}_Int32", name))
                    }
                }
            }
        }
    }
    
    fn generate_list_literal(&mut self, items: &[Box<Expr>]) -> Result<(), CodeGenError> {
        let list_size = 8 + (items.len() * 4); // Header (length + capacity) + elements
        
        // Allocate memory for the list
        self.output.push_str(&format!("    i32.const {} ;; list size\n", list_size));
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $list_tmp\n");
        
        // Write length
        self.output.push_str("    local.get $list_tmp\n");
        self.output.push_str(&format!("    i32.const {} ;; length\n", items.len()));
        self.output.push_str("    i32.store\n");
        
        // Write capacity (same as length for literals)
        self.output.push_str("    local.get $list_tmp\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str(&format!("    i32.const {} ;; capacity\n", items.len()));
        self.output.push_str("    i32.store\n");
        
        // Write elements
        for (i, item) in items.iter().enumerate() {
            self.output.push_str("    local.get $list_tmp\n");
            self.output.push_str(&format!("    i32.const {} ;; offset to element {}\n", 8 + (i * 4), i));
            self.output.push_str("    i32.add\n");
            self.generate_expr(item)?;
            self.output.push_str("    i32.store\n");
        }
        
        // Return the list pointer
        self.output.push_str("    local.get $list_tmp\n");
        
        Ok(())
    }
    
    fn generate_array_literal(&mut self, items: &[Box<Expr>]) -> Result<(), CodeGenError> {
        let array_size = items.len() * 4; // No header, just elements
        
        // Allocate memory for the array
        self.output.push_str(&format!("    i32.const {} ;; array size\n", array_size));
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.tee $list_tmp\n"); // Save and leave on stack
        
        // Write elements
        for (i, item) in items.iter().enumerate() {
            self.output.push_str("    local.get $list_tmp\n");
            self.output.push_str(&format!("    i32.const {} ;; offset to element {}\n", i * 4, i));
            self.output.push_str("    i32.add\n");
            self.generate_expr(item)?;
            self.output.push_str("    i32.store\n");
        }
        
        // Array pointer is already on stack from local.tee
        
        Ok(())
    }
    
    fn generate_match_expr(&mut self, match_expr: &MatchExpr) -> Result<(), CodeGenError> {
        // First evaluate the expression being matched
        self.generate_expr(&match_expr.expr)?;
        self.output.push_str("    local.set $match_tmp\n");
        
        // Generate a series of if-else blocks for each pattern
        for (i, arm) in match_expr.arms.iter().enumerate() {
            if i > 0 {
                self.output.push_str("      (else\n");
            }
            
            // Generate pattern matching code
            self.output.push_str("    local.get $match_tmp\n");
            let bindings = self.generate_pattern_match(&arm.pattern)?;
            
            self.output.push_str("    (if (result i32)\n");
            self.output.push_str("      (then\n");
            
            // Apply bindings
            for (name, load_code) in bindings {
                self.output.push_str(&load_code);
                self.output.push_str(&format!("        local.set ${}\n", name));
            }
            
            // Generate arm body as expression (match arms should produce values)
            self.push_scope();
            self.generate_block_as_expression(&arm.body)?;
            self.pop_scope();
            
            self.output.push_str("      )\n");
        }
        
        // Close all the else blocks
        for _ in 1..match_expr.arms.len() {
            self.output.push_str("      )\n");
        }
        self.output.push_str("    )\n");
        
        Ok(())
    }
    
    fn generate_pattern_match(&mut self, pattern: &Pattern) -> Result<Vec<(String, String)>, CodeGenError> {
        let mut bindings = Vec::new();
        
        match pattern {
            Pattern::Wildcard => {
                // Always matches, no bindings
                self.output.push_str("    i32.const 1 ;; wildcard always matches\n");
            }
            Pattern::Ident(name) => {
                // Always matches, bind the value
                bindings.push((name.clone(), "    local.get $match_tmp\n".to_string()));
                self.output.push_str("    i32.const 1 ;; var always matches\n");
            }
            Pattern::Literal(lit) => {
                match lit {
                    Literal::Int(n) => {
                        // Check if equal to the integer
                        self.output.push_str(&format!("    i32.const {}\n", n));
                        self.output.push_str("    i32.eq\n");
                    }
                    Literal::String(_) => {
                        return Err(CodeGenError::NotImplemented("string patterns".to_string()));
                    }
                    Literal::Float(_) => {
                        return Err(CodeGenError::NotImplemented("float patterns".to_string()));
                    }
                    Literal::Char(_) => {
                        return Err(CodeGenError::NotImplemented("char patterns".to_string()));
                    }
                    Literal::Bool(b) => {
                        self.output.push_str(&format!("    i32.const {}\n", if *b { 1 } else { 0 }));
                        self.output.push_str("    i32.eq\n");
                    }
                    Literal::Unit => {
                        self.output.push_str("    i32.const 0\n");
                        self.output.push_str("    i32.eq\n");
                    }
                }
            }
            Pattern::EmptyList => {
                // Check if list is empty
                self.output.push_str("    call $list_length\n");
                self.output.push_str("    i32.const 0\n");
                self.output.push_str("    i32.eq\n");
            }
            Pattern::ListExact(patterns) => {
                // Check length first
                self.output.push_str("    call $list_length\n");
                self.output.push_str(&format!("    i32.const {}\n", patterns.len()));
                self.output.push_str("    i32.eq\n");
                
                // For each element pattern
                for (i, pattern) in patterns.iter().enumerate() {
                    self.output.push_str("    (if (result i32)\n");
                    self.output.push_str("      (then\n");
                    self.output.push_str("        local.get $match_tmp\n");
                    self.output.push_str(&format!("        i32.const {}\n", i));
                    self.output.push_str("        call $list_get\n");
                    
                    let sub_bindings = self.generate_pattern_match(pattern)?;
                    bindings.extend(sub_bindings);
                    
                    self.output.push_str("      )\n");
                    self.output.push_str("      (else\n");
                    self.output.push_str("        i32.const 0 ;; pattern failed\n");
                    self.output.push_str("      )\n");
                    self.output.push_str("    )\n");
                    self.output.push_str("    i32.and ;; all patterns must match\n");
                }
            }
            Pattern::ListCons(head_pattern, tail_pattern) => {
                // Check that list is not empty
                self.output.push_str("    local.tee $tail_tmp ;; save list for tail\n");
                self.output.push_str("    call $list_length\n");
                self.output.push_str("    local.tee $tail_len ;; save length\n");
                self.output.push_str("    i32.const 0\n");
                self.output.push_str("    i32.gt_u ;; length > 0\n");
                
                // Match head
                self.output.push_str("    (if (result i32)\n");
                self.output.push_str("      (then\n");
                self.output.push_str("        local.get $match_tmp\n");
                self.output.push_str("        i32.const 0\n");
                self.output.push_str("        call $list_get\n");
                
                let head_bindings = self.generate_pattern_match(head_pattern)?;
                bindings.extend(head_bindings);
                
                // Get tail
                self.output.push_str("        (if (result i32)\n");
                self.output.push_str("          (then\n");
                self.output.push_str("            local.get $tail_tmp\n");
                self.output.push_str("            call $tail\n");
                
                let tail_bindings = self.generate_pattern_match(tail_pattern)?;
                bindings.extend(tail_bindings);
                
                self.output.push_str("          )\n");
                self.output.push_str("          (else\n");
                self.output.push_str("            i32.const 0\n");
                self.output.push_str("          )\n");
                self.output.push_str("        )\n");
                self.output.push_str("      )\n");
                self.output.push_str("      (else\n");
                self.output.push_str("        i32.const 0 ;; empty list\n");
                self.output.push_str("      )\n");
                self.output.push_str("    )\n");
            }
            Pattern::Record(_record_name, field_patterns) => {
                // For record patterns, we need to:
                // 1. Save the record pointer
                // 2. Extract and match each field pattern
                // 3. Combine the match results
                
                // Save the record pointer
                self.output.push_str("    local.set $match_tmp\n");
                
                // We need to match all fields and AND the results
                let mut all_bindings = Vec::new();
                let mut field_offset = 0;
                let mut first_field = true;
                
                for (_field_name, field_pattern) in field_patterns {
                    // Load the field value
                    self.output.push_str("    local.get $match_tmp\n");
                    self.output.push_str(&format!("    i32.const {}\n", field_offset));
                    self.output.push_str("    i32.add\n");
                    self.output.push_str("    i32.load\n");
                    
                    // Match the field pattern
                    let field_bindings = self.generate_pattern_match(field_pattern)?;
                    all_bindings.extend(field_bindings);
                    
                    // If not the first field, AND with previous result
                    if !first_field {
                        self.output.push_str("    i32.and\n");
                    }
                    first_field = false;
                    
                    field_offset += 4; // Assume all fields are i32 for now
                }
                
                // If there were no fields, just push 1
                if first_field {
                    self.output.push_str("    i32.const 1 ;; empty record pattern always matches\n");
                }
                
                return Ok(all_bindings);
            }
            Pattern::Some(inner_pattern) => {
                // Check if tag is 1 (Some)
                self.output.push_str("    local.tee $match_tmp ;; save for value extraction\n");
                self.output.push_str("    i32.load ;; load tag\n");
                self.output.push_str("    i32.const 1 ;; Some tag\n");
                self.output.push_str("    i32.eq\n");
                
                // If tag matches, match the inner pattern
                self.output.push_str("    (if (result i32)\n");
                self.output.push_str("      (then\n");
                self.output.push_str("        local.get $match_tmp\n");
                self.output.push_str("        i32.const 4\n");
                self.output.push_str("        i32.add\n");
                self.output.push_str("        i32.load ;; load value from offset 4\n");
                
                let inner_bindings = self.generate_pattern_match(inner_pattern)?;
                bindings.extend(inner_bindings);
                
                self.output.push_str("      )\n");
                self.output.push_str("      (else\n");
                self.output.push_str("        i32.const 0 ;; tag mismatch\n");
                self.output.push_str("      )\n");
                self.output.push_str("    )\n");
            }
            Pattern::None => {
                // Check if tag is 0 (None)
                self.output.push_str("    i32.load ;; load tag\n");
                self.output.push_str("    i32.const 0 ;; None tag\n");
                self.output.push_str("    i32.eq\n");
            }
        }
        
        Ok(bindings)
    }
    
    fn generate_then_expr(&mut self, then: &ThenExpr) -> Result<(), CodeGenError> {
        // Generate condition
        self.generate_expr(&then.condition)?;
        
        self.output.push_str("    (if\n");
        self.output.push_str("      (then\n");
        self.generate_block(&then.then_block)?;
        self.output.push_str("      )\n");
        
        if !then.else_ifs.is_empty() || then.else_block.is_some() {
            self.output.push_str("      (else\n");
            
            // Generate else-if chain
            for (i, (cond, block)) in then.else_ifs.iter().enumerate() {
                if i > 0 {
                    self.output.push_str("        (else\n");
                }
                self.generate_expr(cond)?;
                self.output.push_str("        (if\n");
                self.output.push_str("          (then\n");
                self.generate_block(block)?;
                self.output.push_str("          )\n");
            }
            
            // Final else block
            if let Some(else_block) = &then.else_block {
                if !then.else_ifs.is_empty() {
                    self.output.push_str("          (else\n");
                }
                self.generate_block(else_block)?;
                if !then.else_ifs.is_empty() {
                    self.output.push_str("          )\n");
                }
            } else if !then.else_ifs.is_empty() {
                // Need to provide unit value if no else block
                self.output.push_str("          (else\n");
                self.output.push_str("            i32.const 0 ;; unit\n");
                self.output.push_str("          )\n");
            }
            
            // Close all the nested ifs
            for _ in 0..then.else_ifs.len() {
                self.output.push_str("        )\n");
            }
            
            self.output.push_str("      )\n");
        } else {
            // No else block, provide unit value
            self.output.push_str("      (else\n");
            self.output.push_str("        i32.const 0 ;; unit\n");
            self.output.push_str("      )\n");
        }
        
        self.output.push_str("    )\n");
        
        Ok(())
    }
    
    fn generate_while_expr(&mut self, while_expr: &WhileExpr) -> Result<(), CodeGenError> {
        self.output.push_str("    (loop $while_loop\n");
        
        // Generate condition
        self.generate_expr(&while_expr.condition)?;
        
        self.output.push_str("      (if\n");
        self.output.push_str("        (then\n");
        
        // Generate body
        self.generate_block(&while_expr.body)?;
        
        // Loop back
        self.output.push_str("          br $while_loop\n");
        self.output.push_str("        )\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        
        // While loops return unit
        self.output.push_str("    i32.const 0 ;; unit\n");
        
        Ok(())
    }
    
    fn generate_clone_expr(&mut self, clone: &CloneExpr) -> Result<(), CodeGenError> {
        // Clone expressions create a new record by copying the base record
        // and updating specified fields with new values
        
        // First, get the record type from the base expression
        let _record_type = match self.get_expr_type(&clone.base) {
            Some(ty) => ty,
            None => return Err(CodeGenError::NotImplemented("clone with unknown base type".to_string())),
        };
        
        // Calculate record size (this is simplified - should use actual field info)
        // For now, assume each field is 4 bytes (i32)
        let field_count = clone.updates.fields.len() + 2; // Estimate base fields + updates
        let record_size = field_count * 4;
        
        // Allocate memory for the new record
        self.output.push_str(&format!("    i32.const {} ;; record size\n", record_size));
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $clone_tmp\n");
        
        // Generate base expression to get the original record
        self.generate_expr(&clone.base)?;
        self.output.push_str("    local.set $base_tmp\n");
        
        // Copy all fields from base record to new record
        // For simplicity, we'll copy the entire record memory
        self.output.push_str("    local.get $clone_tmp ;; destination\n");
        self.output.push_str("    local.get $base_tmp ;; source\n");
        self.output.push_str(&format!("    i32.const {} ;; size\n", record_size));
        self.output.push_str("    memory.copy\n");
        
        // Now update the specified fields with new values
        for (field_index, field_init) in clone.updates.fields.iter().enumerate() {
            // Calculate field offset (simplified - assumes 4 bytes per field)
            let field_offset = field_index * 4;
            
            // Store the target address first
            self.output.push_str("    local.get $clone_tmp\n");
            self.output.push_str(&format!("    i32.const {} ;; field offset for {}\n", field_offset, field_init.name));
            self.output.push_str("    i32.add\n");
            
            // Generate the new value for this field
            self.generate_expr(&field_init.value)?;
            
            // Store the new value at the correct field offset
            self.output.push_str("    i32.store\n");
        }
        
        // Return pointer to the new cloned record
        self.output.push_str("    local.get $clone_tmp\n");
        
        Ok(())
    }
    
    fn generate_freeze_expr(&mut self, expr: &Expr) -> Result<(), CodeGenError> {
        // Freeze expressions create a frozen copy of a record
        // In our implementation, we'll add a "frozen" flag at the beginning of the record
        
        // Generate the expression to get the record to freeze
        self.generate_expr(expr)?;
        self.output.push_str("    local.set $freeze_tmp\n");
        
        // For simplicity, we'll assume the record layout is:
        // [frozen_flag: i32][field1: i32][field2: i32]...
        // If the record is already frozen, just return it
        
        // Check if already frozen (first 4 bytes should be 1 if frozen)
        self.output.push_str("    local.get $freeze_tmp\n");
        self.output.push_str("    i32.load ;; load frozen flag\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        ;; Already frozen, return as-is\n");
        self.output.push_str("        local.get $freeze_tmp\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        ;; Not frozen, create frozen copy\n");
        
        // Estimate record size (simplified)
        let record_size = 20; // Assume 5 fields * 4 bytes each
        
        // Allocate new record
        self.output.push_str(&format!("        i32.const {} ;; record size\n", record_size));
        self.output.push_str("        call $allocate\n");
        self.output.push_str("        local.tee $clone_tmp\n");
        
        // Copy the entire record
        self.output.push_str("        local.get $freeze_tmp ;; source\n");
        self.output.push_str(&format!("        i32.const {} ;; size\n", record_size));
        self.output.push_str("        memory.copy\n");
        
        // Set frozen flag to 1
        self.output.push_str("        local.get $clone_tmp\n");
        self.output.push_str("        i32.const 1 ;; frozen flag\n");
        self.output.push_str("        i32.store\n");
        
        // Return the frozen record
        self.output.push_str("        local.get $clone_tmp\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        
        Ok(())
    }
    
    // Generate specialized versions for generic list functions
    fn generate_new_list_specializations(&mut self, _func: &FunDecl) -> Result<(), CodeGenError> {
        // Generate new_list_Int32 specialization
        self.output.push_str("  (func $new_list_Int32 (result i32)\n");
        self.output.push_str("    ;; Allocate empty list: 8 bytes header\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.tee $list_tmp\n");
        self.output.push_str("    ;; Set length to 0\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    ;; Set capacity to 0\n");
        self.output.push_str("    local.get $list_tmp\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $list_tmp\n");
        self.output.push_str("  )\n");
        
        self.functions.insert("new_list_Int32".to_string(), FunctionSig {
            _params: vec![],
            result: Some(WasmType::I32),
        });
        
        Ok(())
    }
    
    fn generate_list_add_specializations(&mut self, _func: &FunDecl) -> Result<(), CodeGenError> {
        // Generate list_add_Int32 specialization
        self.output.push_str("  (func $list_add_Int32 (param $list i32) (param $value i32) (result i32)\n");
        self.output.push_str("    (local $length i32)\n");
        self.output.push_str("    (local $capacity i32)\n");
        self.output.push_str("    (local $new_list i32)\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Load current length and capacity\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $length\n");
        self.output.push_str("    \n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $capacity\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Calculate new size: header + (length + 1) * 4\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $new_list\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Set new length\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Set new capacity\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Copy existing elements\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    memory.copy\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Add new element\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $value\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("  )\n");
        
        self.functions.insert("list_add_Int32".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });
        
        Ok(())
    }
    
    fn generate_some_specializations(&mut self, _func: &FunDecl) -> Result<(), CodeGenError> {
        // Generate some_Int32 specialization
        self.output.push_str("  (func $some_Int32 (param $value i32) (result i32)\n");
        self.output.push_str("    ;; Allocate Option: tag (1 for Some) + value\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.tee $list_tmp\n");
        self.output.push_str("    ;; Set tag to 1 (Some)\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    ;; Set value\n");
        self.output.push_str("    local.get $list_tmp\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $value\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $list_tmp\n");
        self.output.push_str("  )\n");
        
        self.functions.insert("some_Int32".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: Some(WasmType::I32),
        });
        
        Ok(())
    }
    
    fn generate_none_specializations(&mut self, _func: &FunDecl) -> Result<(), CodeGenError> {
        // Generate none_Int32 specialization
        self.output.push_str("  (func $none_Int32 (result i32)\n");
        self.output.push_str("    ;; Return NULL pointer for None\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("  )\n");
        
        self.functions.insert("none_Int32".to_string(), FunctionSig {
            _params: vec![],
            result: Some(WasmType::I32),
        });
        
        Ok(())
    }
    
    fn generate_imports(&mut self, imports: &[ImportDecl]) -> Result<(), CodeGenError> {
        if imports.is_empty() {
            return Ok(());
        }
        
        self.output.push_str("\n  ;; Module imports\n");
        
        for import in imports {
            let module_name = import.module_path.join(".");
            
            match &import.items {
                ImportItems::All => {
                    // Import all items from module (simplified implementation)
                    self.output.push_str(&format!("  ;; Import all from {}\n", module_name));
                    // In a real implementation, we'd need to resolve what "all" means
                    // For now, we'll generate a placeholder comment
                }
                ImportItems::Named(items) => {
                    for item in items {
                        // Generate WebAssembly import for each named item
                        // Assume functions for now
                        self.output.push_str(&format!(
                            "  (import \"{}\" \"{}\" (func ${} (param i32) (result i32)))\n",
                            module_name, item, item
                        ));
                        
                        // Register the imported function
                        self.functions.insert(item.clone(), FunctionSig {
                            _params: vec![WasmType::I32],
                            result: Some(WasmType::I32),
                        });
                    }
                }
            }
        }
        
        Ok(())
    }
    
    fn generate_exports(&mut self, program: &Program) -> Result<(), CodeGenError> {
        let mut has_exports = false;
        
        for decl in &program.declarations {
            if let TopDecl::Export(export_decl) = decl {
                if !has_exports {
                    self.output.push_str("\n  ;; Module exports\n");
                    has_exports = true;
                }
                
                match &*export_decl.item {
                    TopDecl::Function(func) => {
                        // Export function
                        self.output.push_str(&format!(
                            "  (export \"{}\" (func ${}))\n",
                            func.name, func.name
                        ));
                    }
                    TopDecl::Record(record) => {
                        // Export record constructor (simplified)
                        self.output.push_str(&format!(
                            "  ;; Export record type: {}\n",
                            record.name
                        ));
                        // In a real implementation, we'd export record-related functions
                    }
                    TopDecl::Binding(binding) => {
                        // Export global binding
                        self.output.push_str(&format!(
                            "  ;; Export binding: {}\n",
                            binding.name
                        ));
                        // In a real implementation, we'd export as global or memory location
                    }
                    _ => {
                        // Other export types not implemented yet
                        self.output.push_str("  ;; Unsupported export type\n");
                    }
                }
            }
        }
        
        Ok(())
    }
    
    fn generate_prototype_clone_expr(&mut self, proto_clone: &PrototypeCloneExpr) -> Result<(), CodeGenError> {
        // Generate prototype clone with hash metadata
        // This is similar to regular clone but includes prototype metadata
        
        // For now, generate similar to regular clone
        // In a full implementation, this would include hash generation and parent tracking
        
        // Calculate record size (simplified)
        let field_count = proto_clone.updates.fields.len() + 3; // Base fields + metadata
        let record_size = field_count * 4;
        
        // Allocate memory for the new prototype instance
        self.output.push_str(&format!("    i32.const {} ;; prototype instance size\n", record_size));
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $clone_tmp\n");
        
        // Set prototype metadata (hash, parent_hash, sealed flag)
        // In a real implementation, these would be computed values
        self.output.push_str("    ;; Set prototype hash metadata\n");
        self.output.push_str("    local.get $clone_tmp\n");
        self.output.push_str("    i32.const 0x12345678 ;; placeholder hash\n");
        self.output.push_str("    i32.store\n");
        
        // Set parent hash
        self.output.push_str("    local.get $clone_tmp\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.const 0x87654321 ;; placeholder parent hash\n");
        self.output.push_str("    i32.store\n");
        
        // Set sealed flag
        self.output.push_str("    local.get $clone_tmp\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        if proto_clone.sealed {
            self.output.push_str("    i32.const 1 ;; sealed\n");
        } else {
            self.output.push_str("    i32.const 0 ;; not sealed\n");
        }
        self.output.push_str("    i32.store\n");
        
        // Handle field updates (starting from offset 12)
        for (field_index, field_init) in proto_clone.updates.fields.iter().enumerate() {
            let field_offset = 12 + (field_index * 4); // After metadata
            
            // Store the target address first
            self.output.push_str("    local.get $clone_tmp\n");
            self.output.push_str(&format!("    i32.const {} ;; field offset for {}\n", field_offset, field_init.name));
            self.output.push_str("    i32.add\n");
            
            // Generate the new value for this field
            self.generate_expr(&field_init.value)?;
            
            // Store the new value at the correct field offset
            self.output.push_str("    i32.store\n");
        }
        
        // Return pointer to the new prototype instance
        self.output.push_str("    local.get $clone_tmp\n");
        
        Ok(())
    }
    
    fn get_expr_type(&self, expr: &Expr) -> Option<String> {
        // First check the expr_types map (filled by type checker)
        if let Some(ty) = self.expr_types.get(&(expr as *const Expr)) {
            return Some(ty.clone());
        }
        
        // Fall back to simple inference
        match expr {
            Expr::RecordLit(record) => {
                // Try to infer from record name if available
                Some(record.name.clone())
            }
            Expr::Ident(_name) => {
                // Check if it's a known variable
                // This would require more context, so return None for now
                None
            }
            _ => None,
        }
    }
}