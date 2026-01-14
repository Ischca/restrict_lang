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

    /// Type inference failed
    #[error("Cannot infer type: {0}")]
    CannotInferType(String),
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
    /// Key is pointer address as usize, value is type name string
    pub expr_types: HashMap<usize, String>,
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
    /// Record definitions: record_name -> fields
    records: HashMap<String, Vec<(String, Type)>>,
    /// Record field offsets: record_name -> field_name -> offset
    record_field_offsets: HashMap<String, HashMap<String, u32>>,
    /// Variable types: var_name -> type_name (e.g., "Point", "Buffer")
    var_types: HashMap<String, String>,
    /// Imported functions to be inlined
    imported_functions: Vec<FunDecl>,
    /// Imported records to be included
    imported_records: Vec<RecordDecl>,
    /// Generic function definitions for monomorphization
    generic_functions: HashMap<String, FunDecl>,
    /// Tracked instantiations: function_name -> Vec<(type_args, mangled_name)>
    instantiations: HashMap<String, Vec<(Vec<String>, String)>>,
    /// Function return types: function_name -> type_name (e.g., "String", "Int32")
    function_return_types: HashMap<String, String>,
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
            records: HashMap::new(),
            record_field_offsets: HashMap::new(),
            var_types: HashMap::new(),
            imported_functions: Vec::new(),
            imported_records: Vec::new(),
            generic_functions: HashMap::new(),
            instantiations: HashMap::new(),
            function_return_types: HashMap::new(),
        }
    }
    
    /// Register imported declarations to be included in the generated WASM.
    /// These declarations will be generated as part of the module instead of
    /// being imported from external modules.
    pub fn register_imported_decl(&mut self, decl: &TopDecl) -> Result<(), CodeGenError> {
        match decl {
            TopDecl::Function(func) => {
                // Register the function signature so codegen knows about it
                let params: Vec<WasmType> = func.params.iter().map(|_| WasmType::I32).collect();
                let result = Some(WasmType::I32); // Default to I32 for now
                self.functions.insert(func.name.clone(), FunctionSig {
                    _params: params,
                    result,
                });
                // Track return type for println dispatch
                // Use explicit annotation or infer from body
                let return_type = if let Some(ref ty) = func.return_type {
                    self.type_to_string(ty)
                } else if let Some(ref expr) = func.body.expr {
                    self.infer_return_type_from_expr(expr)?
                } else {
                    "Unit".to_string()
                };
                self.function_return_types.insert(func.name.clone(), return_type);
                // Store the function for later generation
                self.imported_functions.push(func.clone());
            }
            TopDecl::Record(record) => {
                // Store record for later registration
                self.imported_records.push(record.clone());
            }
            _ => {
                // Other declaration types not yet supported for import
            }
        }
        Ok(())
    }

    /// Set the expression types from the type checker.
    /// This enables proper dispatch of generic functions like println.
    pub fn set_expr_types(&mut self, types: &std::collections::HashMap<usize, crate::type_checker::TypedType>) {
        for (ptr, ty) in types {
            let type_name = crate::type_checker::format_typed_type(ty);
            self.expr_types.insert(*ptr, type_name);
        }
    }

    pub fn generate(&mut self, program: &Program) -> Result<String, CodeGenError> {
        self.output.push_str("(module\n");

        // Skip WASM imports for functions we have inlined declarations for
        // This prevents generating "import" statements for functions we'll define locally
        // (No-op if we have the functions inlined)
        
        // Import WASI functions for I/O
        self.output.push_str("  ;; WASI imports\n");
        self.output.push_str("  (import \"wasi_snapshot_preview1\" \"fd_write\" (func $fd_write (param i32 i32 i32 i32) (result i32)))\n");
        self.output.push_str("  (import \"wasi_snapshot_preview1\" \"fd_read\" (func $fd_read (param i32 i32 i32 i32) (result i32)))\n");
        self.output.push_str("  (import \"wasi_snapshot_preview1\" \"fd_close\" (func $fd_close (param i32) (result i32)))\n");
        self.output.push_str("  (import \"wasi_snapshot_preview1\" \"fd_seek\" (func $fd_seek (param i32 i64 i32 i32) (result i32)))\n");
        self.output.push_str("  (import \"wasi_snapshot_preview1\" \"path_open\" (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))\n");
        self.output.push_str("  (import \"wasi_snapshot_preview1\" \"fd_prestat_get\" (func $fd_prestat_get (param i32 i32) (result i32)))\n");
        self.output.push_str("  (import \"wasi_snapshot_preview1\" \"fd_prestat_dir_name\" (func $fd_prestat_dir_name (param i32 i32 i32) (result i32)))\n");
        self.output.push_str("  (import \"wasi_snapshot_preview1\" \"proc_exit\" (func $proc_exit (param i32)))\n");
        
        // Memory
        self.output.push_str("\n  ;; Memory\n");
        self.output.push_str("  (memory 1)\n");
        self.output.push_str("  (export \"memory\" (memory 0))\n");
        
        // Collect string constants first
        self.collect_strings(program)?;

        // Collect strings from imported functions
        for func in &self.imported_functions.clone() {
            self.collect_strings_from_block(&func.body)?;
        }

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

        // Generate prelude functions
        self.generate_prelude_functions()?;

        // Generate arena allocator functions
        self.generate_arena_functions()?;
        
        // Generate list operation functions
        self.generate_list_functions()?;
        
        // Generate array operation functions
        self.generate_array_functions()?;
        
        // Collect record definitions first
        for decl in &program.declarations {
            match decl {
                TopDecl::Record(record) => {
                    self.register_record_definition(record)?;
                }
                TopDecl::Export(export) => {
                    if let TopDecl::Record(record) = export.item.as_ref() {
                        self.register_record_definition(record)?;
                    }
                }
                _ => {}
            }
        }

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
                TopDecl::Export(export) => {
                    match export.item.as_ref() {
                        TopDecl::Function(func) => {
                            self.register_function_signature(func)?;
                        }
                        TopDecl::Record(record) => {
                            self.register_record_methods(record)?;
                        }
                        _ => {}
                    }
                }
                TopDecl::Impl(_) | TopDecl::Context(_) => {
                    // Not yet implemented
                }
            }
        }
        
        // Generate imported functions first
        if !self.imported_functions.is_empty() {
            self.output.push_str("\n  ;; Imported functions (inlined)\n");
            let imported_funcs: Vec<_> = self.imported_functions.clone();
            for func in &imported_funcs {
                self.generate_function(func)?;
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
                TopDecl::Export(export) => {
                    match export.item.as_ref() {
                        TopDecl::Function(func) => {
                            self.generate_function(func)?;
                        }
                        TopDecl::Record(record) => {
                            self.generate_record_methods(record)?;
                        }
                        _ => {}
                    }
                }
                TopDecl::Impl(_) | TopDecl::Context(_) => {
                    // Not yet implemented
                }
            }
        }

        // Generate monomorphized versions of generic functions
        self.generate_monomorphized_functions()?;

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

        // read_line function - reads a line from stdin
        self.output.push_str("\n  ;; Read a line from stdin\n");
        self.output.push_str("  (func $read_line (result i32)\n");
        self.output.push_str("    (local $buffer i32)\n");
        self.output.push_str("    (local $nread i32)\n");
        self.output.push_str("    (local $result i32)\n");
        self.output.push_str("    (local $i i32)\n");
        self.output.push_str("    (local $char i32)\n");
        self.output.push_str("    (local $len i32)\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Use memory at 4096 for read buffer (1024 bytes)\n");
        self.output.push_str("    i32.const 4096\n");
        self.output.push_str("    local.set $buffer\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Setup iovec at address 4000\n");
        self.output.push_str("    ;; iov_base = buffer address\n");
        self.output.push_str("    i32.const 4000\n");
        self.output.push_str("    local.get $buffer\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    ;; iov_len = 1024 (max read size)\n");
        self.output.push_str("    i32.const 4004\n");
        self.output.push_str("    i32.const 1024\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Call fd_read(stdin=0, iovs, iovs_len=1, nread)\n");
        self.output.push_str("    i32.const 0    ;; stdin\n");
        self.output.push_str("    i32.const 4000 ;; iovs\n");
        self.output.push_str("    i32.const 1    ;; iovs_len\n");
        self.output.push_str("    i32.const 4008 ;; nread ptr\n");
        self.output.push_str("    call $fd_read\n");
        self.output.push_str("    drop\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Get number of bytes read\n");
        self.output.push_str("    i32.const 4008\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $nread\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Find newline and calculate actual length\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    local.set $i\n");
        self.output.push_str("    local.get $nread\n");
        self.output.push_str("    local.set $len\n");
        self.output.push_str("    (block $found\n");
        self.output.push_str("      (loop $search\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        local.get $nread\n");
        self.output.push_str("        i32.ge_u\n");
        self.output.push_str("        br_if $found\n");
        self.output.push_str("        \n");
        self.output.push_str("        local.get $buffer\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        i32.load8_u\n");
        self.output.push_str("        local.tee $char\n");
        self.output.push_str("        i32.const 10  ;; newline\n");
        self.output.push_str("        i32.eq\n");
        self.output.push_str("        (if\n");
        self.output.push_str("          (then\n");
        self.output.push_str("            local.get $i\n");
        self.output.push_str("            local.set $len\n");
        self.output.push_str("            br $found\n");
        self.output.push_str("          )\n");
        self.output.push_str("        )\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.set $i\n");
        self.output.push_str("        br $search\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Allocate result string at 5120 (length prefix + data)\n");
        self.output.push_str("    i32.const 5120\n");
        self.output.push_str("    local.set $result\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Store length\n");
        self.output.push_str("    local.get $result\n");
        self.output.push_str("    local.get $len\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Copy data\n");
        self.output.push_str("    local.get $result\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add  ;; destination\n");
        self.output.push_str("    local.get $buffer  ;; source\n");
        self.output.push_str("    local.get $len  ;; size\n");
        self.output.push_str("    memory.copy\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Return pointer to string\n");
        self.output.push_str("    local.get $result\n");
        self.output.push_str("  )\n");

        // eprint function - print to stderr
        self.output.push_str("\n  ;; Print to stderr\n");
        self.output.push_str("  (func $eprint (param $str i32)\n");
        self.output.push_str("    (local $len i32)\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Read string length\n");
        self.output.push_str("    local.get $str\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $len\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Setup iovec at address 0\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    local.get $str\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    local.get $len\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; fd_write to stderr (fd=2)\n");
        self.output.push_str("    i32.const 2   ;; stderr\n");
        self.output.push_str("    i32.const 0   ;; iovs\n");
        self.output.push_str("    i32.const 1   ;; iovs_len\n");
        self.output.push_str("    i32.const 20  ;; nwritten\n");
        self.output.push_str("    call $fd_write\n");
        self.output.push_str("    drop\n");
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

        // Add read_line to function signatures
        self.functions.insert("read_line".to_string(), FunctionSig {
            _params: vec![],
            result: Some(WasmType::I32),
        });

        // Add eprint to function signatures
        self.functions.insert("eprint".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: None,
        });

        // File I/O functions
        self.generate_file_io_functions()?;

        // Generate string runtime functions
        self.generate_string_functions()?;

        Ok(())
    }

    fn generate_file_io_functions(&mut self) -> Result<(), CodeGenError> {
        self.output.push_str("\n  ;; File I/O functions\n");

        // file_open: Opens a file and returns a file descriptor
        // Parameters: path (string pointer), flags (i32)
        // Returns: fd (i32) or -1 on error
        // Flags: 0 = read, 1 = write, 2 = read+write
        self.output.push_str("  (func $file_open (param $path i32) (param $flags i32) (result i32)\n");
        self.output.push_str("    (local $path_len i32)\n");
        self.output.push_str("    (local $path_ptr i32)\n");
        self.output.push_str("    (local $fd_out i32)\n");
        self.output.push_str("    (local $oflags i32)\n");
        self.output.push_str("    (local $rights i64)\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Get path length and pointer\n");
        self.output.push_str("    local.get $path\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $path_len\n");
        self.output.push_str("    local.get $path\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.set $path_ptr\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Set oflags based on flags parameter\n");
        self.output.push_str("    ;; flags: 0=read, 1=write(create), 2=read+write\n");
        self.output.push_str("    local.get $flags\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.and\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then i32.const 1)  ;; O_CREAT\n");
        self.output.push_str("      (else i32.const 0)  ;; No flags\n");
        self.output.push_str("    )\n");
        self.output.push_str("    local.set $oflags\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Set rights - allow read and write\n");
        self.output.push_str("    i64.const 0x1FFFFFFF  ;; All rights\n");
        self.output.push_str("    local.set $rights\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; fd_out pointer at address 6200\n");
        self.output.push_str("    i32.const 6200\n");
        self.output.push_str("    local.set $fd_out\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Call path_open(dirfd=3, dirflags=0, path, path_len, oflags, rights_base, rights_inheriting, fdflags, fd_out)\n");
        self.output.push_str("    i32.const 3           ;; dirfd (preopened dir)\n");
        self.output.push_str("    i32.const 0           ;; dirflags\n");
        self.output.push_str("    local.get $path_ptr   ;; path\n");
        self.output.push_str("    local.get $path_len   ;; path_len\n");
        self.output.push_str("    local.get $oflags     ;; oflags\n");
        self.output.push_str("    local.get $rights     ;; rights_base\n");
        self.output.push_str("    local.get $rights     ;; rights_inheriting\n");
        self.output.push_str("    i32.const 0           ;; fdflags\n");
        self.output.push_str("    local.get $fd_out     ;; fd out pointer\n");
        self.output.push_str("    call $path_open\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Check result\n");
        self.output.push_str("    i32.eqz\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        ;; Success - return fd\n");
        self.output.push_str("        local.get $fd_out\n");
        self.output.push_str("        i32.load\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        ;; Error - return -1\n");
        self.output.push_str("        i32.const -1\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n\n");

        self.functions.insert("file_open".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });

        // file_read: Read from a file descriptor into a buffer
        // Returns number of bytes read or -1 on error
        self.output.push_str("  (func $file_read (param $fd i32) (param $len i32) (result i32)\n");
        self.output.push_str("    (local $buffer i32)\n");
        self.output.push_str("    (local $nread i32)\n");
        self.output.push_str("    (local $result i32)\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Use buffer at 6300\n");
        self.output.push_str("    i32.const 6300\n");
        self.output.push_str("    local.set $buffer\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Setup iovec at 6204\n");
        self.output.push_str("    i32.const 6204\n");
        self.output.push_str("    local.get $buffer\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    i32.const 6208\n");
        self.output.push_str("    local.get $len\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Call fd_read\n");
        self.output.push_str("    local.get $fd\n");
        self.output.push_str("    i32.const 6204  ;; iovs\n");
        self.output.push_str("    i32.const 1     ;; iovs_len\n");
        self.output.push_str("    i32.const 6212  ;; nread out\n");
        self.output.push_str("    call $fd_read\n");
        self.output.push_str("    local.set $result\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Check result and return string pointer\n");
        self.output.push_str("    local.get $result\n");
        self.output.push_str("    i32.eqz\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        ;; Store length at result string location (7300)\n");
        self.output.push_str("        i32.const 7300\n");
        self.output.push_str("        i32.const 6212\n");
        self.output.push_str("        i32.load\n");
        self.output.push_str("        i32.store\n");
        self.output.push_str("        ;; Copy data\n");
        self.output.push_str("        i32.const 7304\n");
        self.output.push_str("        local.get $buffer\n");
        self.output.push_str("        i32.const 6212\n");
        self.output.push_str("        i32.load\n");
        self.output.push_str("        memory.copy\n");
        self.output.push_str("        ;; Return string pointer\n");
        self.output.push_str("        i32.const 7300\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        ;; Error - return empty string at 7300 with len 0\n");
        self.output.push_str("        i32.const 7300\n");
        self.output.push_str("        i32.const 0\n");
        self.output.push_str("        i32.store\n");
        self.output.push_str("        i32.const 7300\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n\n");

        self.functions.insert("file_read".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });

        // file_write: Write a string to a file descriptor
        // Returns number of bytes written or -1 on error
        self.output.push_str("  (func $file_write (param $fd i32) (param $str i32) (result i32)\n");
        self.output.push_str("    (local $len i32)\n");
        self.output.push_str("    (local $result i32)\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Get string length\n");
        self.output.push_str("    local.get $str\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $len\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Setup iovec at 6220\n");
        self.output.push_str("    i32.const 6220\n");
        self.output.push_str("    local.get $str\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    i32.const 6224\n");
        self.output.push_str("    local.get $len\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Call fd_write\n");
        self.output.push_str("    local.get $fd\n");
        self.output.push_str("    i32.const 6220  ;; iovs\n");
        self.output.push_str("    i32.const 1     ;; iovs_len\n");
        self.output.push_str("    i32.const 6228  ;; nwritten out\n");
        self.output.push_str("    call $fd_write\n");
        self.output.push_str("    local.set $result\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Return nwritten or -1 on error\n");
        self.output.push_str("    local.get $result\n");
        self.output.push_str("    i32.eqz\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        i32.const 6228\n");
        self.output.push_str("        i32.load\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        i32.const -1\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n\n");

        self.functions.insert("file_write".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });

        // file_close: Close a file descriptor
        self.output.push_str("  (func $file_close (param $fd i32) (result i32)\n");
        self.output.push_str("    local.get $fd\n");
        self.output.push_str("    call $fd_close\n");
        self.output.push_str("    i32.eqz\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then i32.const 0)   ;; Success\n");
        self.output.push_str("      (else i32.const -1)  ;; Error\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n\n");

        self.functions.insert("file_close".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: Some(WasmType::I32),
        });

        Ok(())
    }

    fn generate_string_functions(&mut self) -> Result<(), CodeGenError> {
        self.output.push_str("\n  ;; String runtime functions\n");

        // string_length: Get the length of a string (reads 4-byte length prefix)
        self.output.push_str("  (func $string_length (param $str i32) (result i32)\n");
        self.output.push_str("    ;; Read the 4-byte length prefix\n");
        self.output.push_str("    local.get $str\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("  )\n\n");

        self.functions.insert("string_length".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: Some(WasmType::I32),
        });

        // string_equals: Compare two strings for equality
        self.output.push_str("  (func $string_equals (param $a i32) (param $b i32) (result i32)\n");
        self.output.push_str("    (local $len_a i32)\n");
        self.output.push_str("    (local $len_b i32)\n");
        self.output.push_str("    (local $i i32)\n");
        self.output.push_str("    (local $ptr_a i32)\n");
        self.output.push_str("    (local $ptr_b i32)\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Get lengths\n");
        self.output.push_str("    local.get $a\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $len_a\n");
        self.output.push_str("    local.get $b\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $len_b\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; If lengths differ, strings are not equal\n");
        self.output.push_str("    local.get $len_a\n");
        self.output.push_str("    local.get $len_b\n");
        self.output.push_str("    i32.ne\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        i32.const 0  ;; false\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        ;; Compare byte by byte\n");
        self.output.push_str("        local.get $a\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.set $ptr_a\n");
        self.output.push_str("        local.get $b\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.set $ptr_b\n");
        self.output.push_str("        i32.const 0\n");
        self.output.push_str("        local.set $i\n");
        self.output.push_str("        \n");
        self.output.push_str("        (block $done (result i32)\n");
        self.output.push_str("          (loop $cmp\n");
        self.output.push_str("            ;; Check if we've compared all bytes\n");
        self.output.push_str("            local.get $i\n");
        self.output.push_str("            local.get $len_a\n");
        self.output.push_str("            i32.ge_u\n");
        self.output.push_str("            (if\n");
        self.output.push_str("              (then\n");
        self.output.push_str("                i32.const 1  ;; true - all bytes match\n");
        self.output.push_str("                br $done\n");
        self.output.push_str("              )\n");
        self.output.push_str("            )\n");
        self.output.push_str("            \n");
        self.output.push_str("            ;; Compare current bytes\n");
        self.output.push_str("            local.get $ptr_a\n");
        self.output.push_str("            local.get $i\n");
        self.output.push_str("            i32.add\n");
        self.output.push_str("            i32.load8_u\n");
        self.output.push_str("            local.get $ptr_b\n");
        self.output.push_str("            local.get $i\n");
        self.output.push_str("            i32.add\n");
        self.output.push_str("            i32.load8_u\n");
        self.output.push_str("            i32.ne\n");
        self.output.push_str("            (if\n");
        self.output.push_str("              (then\n");
        self.output.push_str("                i32.const 0  ;; false - bytes differ\n");
        self.output.push_str("                br $done\n");
        self.output.push_str("              )\n");
        self.output.push_str("            )\n");
        self.output.push_str("            \n");
        self.output.push_str("            ;; Increment index\n");
        self.output.push_str("            local.get $i\n");
        self.output.push_str("            i32.const 1\n");
        self.output.push_str("            i32.add\n");
        self.output.push_str("            local.set $i\n");
        self.output.push_str("            br $cmp\n");
        self.output.push_str("          )\n");
        self.output.push_str("          i32.const 1  ;; unreachable, but needed for type\n");
        self.output.push_str("        )\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n\n");

        self.functions.insert("string_equals".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });

        // string_concat: Concatenate two strings
        self.output.push_str("  (func $string_concat (param $a i32) (param $b i32) (result i32)\n");
        self.output.push_str("    (local $len_a i32)\n");
        self.output.push_str("    (local $len_b i32)\n");
        self.output.push_str("    (local $new_len i32)\n");
        self.output.push_str("    (local $new_str i32)\n");
        self.output.push_str("    (local $i i32)\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Get lengths\n");
        self.output.push_str("    local.get $a\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $len_a\n");
        self.output.push_str("    local.get $b\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $len_b\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Calculate new length\n");
        self.output.push_str("    local.get $len_a\n");
        self.output.push_str("    local.get $len_b\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.set $new_len\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Allocate memory for new string: 4 bytes length + data\n");
        self.output.push_str("    local.get $new_len\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $new_str\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Write length prefix\n");
        self.output.push_str("    local.get $new_str\n");
        self.output.push_str("    local.get $new_len\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Copy first string\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    local.set $i\n");
        self.output.push_str("    (block $copy1_done\n");
        self.output.push_str("      (loop $copy1\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        local.get $len_a\n");
        self.output.push_str("        i32.ge_u\n");
        self.output.push_str("        br_if $copy1_done\n");
        self.output.push_str("        \n");
        self.output.push_str("        ;; Copy byte from a to new_str\n");
        self.output.push_str("        local.get $new_str\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $a\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        i32.load8_u\n");
        self.output.push_str("        i32.store8\n");
        self.output.push_str("        \n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.set $i\n");
        self.output.push_str("        br $copy1\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Copy second string\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    local.set $i\n");
        self.output.push_str("    (block $copy2_done\n");
        self.output.push_str("      (loop $copy2\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        local.get $len_b\n");
        self.output.push_str("        i32.ge_u\n");
        self.output.push_str("        br_if $copy2_done\n");
        self.output.push_str("        \n");
        self.output.push_str("        ;; Copy byte from b to new_str (offset by len_a)\n");
        self.output.push_str("        local.get $new_str\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $len_a\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $b\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        i32.load8_u\n");
        self.output.push_str("        i32.store8\n");
        self.output.push_str("        \n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.set $i\n");
        self.output.push_str("        br $copy2\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Return new string pointer\n");
        self.output.push_str("    local.get $new_str\n");
        self.output.push_str("  )\n\n");

        self.functions.insert("string_concat".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });

        // char_at: Get character at index (returns -1 if out of bounds)
        self.output.push_str("  (func $char_at (param $str i32) (param $index i32) (result i32)\n");
        self.output.push_str("    (local $len i32)\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Get string length\n");
        self.output.push_str("    local.get $str\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $len\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Check bounds\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    i32.lt_s\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    local.get $len\n");
        self.output.push_str("    i32.ge_s\n");
        self.output.push_str("    i32.or\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        i32.const -1  ;; Out of bounds\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        ;; Return character at index\n");
        self.output.push_str("        local.get $str\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $index\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        i32.load8_u\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n\n");

        self.functions.insert("char_at".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });

        // substring: Extract portion of string (start inclusive, end exclusive)
        self.output.push_str("  (func $substring (param $str i32) (param $start i32) (param $end i32) (result i32)\n");
        self.output.push_str("    (local $len i32)\n");
        self.output.push_str("    (local $new_len i32)\n");
        self.output.push_str("    (local $new_str i32)\n");
        self.output.push_str("    (local $i i32)\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Get string length\n");
        self.output.push_str("    local.get $str\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $len\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Clamp start to [0, len]\n");
        self.output.push_str("    local.get $start\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    i32.lt_s\n");
        self.output.push_str("    (if\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        i32.const 0\n");
        self.output.push_str("        local.set $start\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    local.get $start\n");
        self.output.push_str("    local.get $len\n");
        self.output.push_str("    i32.gt_s\n");
        self.output.push_str("    (if\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        local.get $len\n");
        self.output.push_str("        local.set $start\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Clamp end to [start, len]\n");
        self.output.push_str("    local.get $end\n");
        self.output.push_str("    local.get $start\n");
        self.output.push_str("    i32.lt_s\n");
        self.output.push_str("    (if\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        local.get $start\n");
        self.output.push_str("        local.set $end\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    local.get $end\n");
        self.output.push_str("    local.get $len\n");
        self.output.push_str("    i32.gt_s\n");
        self.output.push_str("    (if\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        local.get $len\n");
        self.output.push_str("        local.set $end\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Calculate new length\n");
        self.output.push_str("    local.get $end\n");
        self.output.push_str("    local.get $start\n");
        self.output.push_str("    i32.sub\n");
        self.output.push_str("    local.set $new_len\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Allocate new string\n");
        self.output.push_str("    local.get $new_len\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $new_str\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Write length prefix\n");
        self.output.push_str("    local.get $new_str\n");
        self.output.push_str("    local.get $new_len\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Copy bytes\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    local.set $i\n");
        self.output.push_str("    (block $copy_done\n");
        self.output.push_str("      (loop $copy\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        local.get $new_len\n");
        self.output.push_str("        i32.ge_u\n");
        self.output.push_str("        br_if $copy_done\n");
        self.output.push_str("        \n");
        self.output.push_str("        ;; Copy byte\n");
        self.output.push_str("        local.get $new_str\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $str\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $start\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        i32.load8_u\n");
        self.output.push_str("        i32.store8\n");
        self.output.push_str("        \n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.set $i\n");
        self.output.push_str("        br $copy\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    \n");
        self.output.push_str("    local.get $new_str\n");
        self.output.push_str("  )\n\n");

        self.functions.insert("substring".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });

        // string_to_int: Parse integer from string (returns 0 on invalid input)
        self.output.push_str("  (func $string_to_int (param $str i32) (result i32)\n");
        self.output.push_str("    (local $len i32)\n");
        self.output.push_str("    (local $i i32)\n");
        self.output.push_str("    (local $result i32)\n");
        self.output.push_str("    (local $char i32)\n");
        self.output.push_str("    (local $is_negative i32)\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Get string length\n");
        self.output.push_str("    local.get $str\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $len\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Empty string returns 0\n");
        self.output.push_str("    local.get $len\n");
        self.output.push_str("    i32.eqz\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        i32.const 0\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        i32.const 0\n");
        self.output.push_str("        local.set $result\n");
        self.output.push_str("        i32.const 0\n");
        self.output.push_str("        local.set $i\n");
        self.output.push_str("        i32.const 0\n");
        self.output.push_str("        local.set $is_negative\n");
        self.output.push_str("        \n");
        self.output.push_str("        ;; Check for negative sign\n");
        self.output.push_str("        local.get $str\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        i32.load8_u\n");
        self.output.push_str("        i32.const 45  ;; '-'\n");
        self.output.push_str("        i32.eq\n");
        self.output.push_str("        (if\n");
        self.output.push_str("          (then\n");
        self.output.push_str("            i32.const 1\n");
        self.output.push_str("            local.set $is_negative\n");
        self.output.push_str("            i32.const 1\n");
        self.output.push_str("            local.set $i\n");
        self.output.push_str("          )\n");
        self.output.push_str("        )\n");
        self.output.push_str("        \n");
        self.output.push_str("        ;; Parse digits\n");
        self.output.push_str("        (block $parse_done\n");
        self.output.push_str("          (loop $parse\n");
        self.output.push_str("            local.get $i\n");
        self.output.push_str("            local.get $len\n");
        self.output.push_str("            i32.ge_u\n");
        self.output.push_str("            br_if $parse_done\n");
        self.output.push_str("            \n");
        self.output.push_str("            ;; Get character\n");
        self.output.push_str("            local.get $str\n");
        self.output.push_str("            i32.const 4\n");
        self.output.push_str("            i32.add\n");
        self.output.push_str("            local.get $i\n");
        self.output.push_str("            i32.add\n");
        self.output.push_str("            i32.load8_u\n");
        self.output.push_str("            local.set $char\n");
        self.output.push_str("            \n");
        self.output.push_str("            ;; Check if digit (48-57 = '0'-'9')\n");
        self.output.push_str("            local.get $char\n");
        self.output.push_str("            i32.const 48\n");
        self.output.push_str("            i32.lt_u\n");
        self.output.push_str("            local.get $char\n");
        self.output.push_str("            i32.const 57\n");
        self.output.push_str("            i32.gt_u\n");
        self.output.push_str("            i32.or\n");
        self.output.push_str("            br_if $parse_done  ;; Stop on non-digit\n");
        self.output.push_str("            \n");
        self.output.push_str("            ;; result = result * 10 + (char - '0')\n");
        self.output.push_str("            local.get $result\n");
        self.output.push_str("            i32.const 10\n");
        self.output.push_str("            i32.mul\n");
        self.output.push_str("            local.get $char\n");
        self.output.push_str("            i32.const 48\n");
        self.output.push_str("            i32.sub\n");
        self.output.push_str("            i32.add\n");
        self.output.push_str("            local.set $result\n");
        self.output.push_str("            \n");
        self.output.push_str("            local.get $i\n");
        self.output.push_str("            i32.const 1\n");
        self.output.push_str("            i32.add\n");
        self.output.push_str("            local.set $i\n");
        self.output.push_str("            br $parse\n");
        self.output.push_str("          )\n");
        self.output.push_str("        )\n");
        self.output.push_str("        \n");
        self.output.push_str("        ;; Apply negative sign if needed\n");
        self.output.push_str("        local.get $is_negative\n");
        self.output.push_str("        (if (result i32)\n");
        self.output.push_str("          (then\n");
        self.output.push_str("            i32.const 0\n");
        self.output.push_str("            local.get $result\n");
        self.output.push_str("            i32.sub\n");
        self.output.push_str("          )\n");
        self.output.push_str("          (else\n");
        self.output.push_str("            local.get $result\n");
        self.output.push_str("          )\n");
        self.output.push_str("        )\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n\n");

        self.functions.insert("string_to_int".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: Some(WasmType::I32),
        });

        // int_to_string: Format integer as string
        self.output.push_str("  (func $int_to_string (param $value i32) (result i32)\n");
        self.output.push_str("    (local $num i32)\n");
        self.output.push_str("    (local $digit i32)\n");
        self.output.push_str("    (local $buffer_start i32)\n");
        self.output.push_str("    (local $buffer_end i32)\n");
        self.output.push_str("    (local $is_negative i32)\n");
        self.output.push_str("    (local $len i32)\n");
        self.output.push_str("    (local $new_str i32)\n");
        self.output.push_str("    (local $i i32)\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Use memory starting at address 500 for temp buffer\n");
        self.output.push_str("    i32.const 520  ;; Start from end and work backwards\n");
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
        self.output.push_str("            \n");
        self.output.push_str("            ;; Get last digit\n");
        self.output.push_str("            local.get $num\n");
        self.output.push_str("            i32.const 10\n");
        self.output.push_str("            i32.rem_u\n");
        self.output.push_str("            local.set $digit\n");
        self.output.push_str("            \n");
        self.output.push_str("            ;; Store digit character\n");
        self.output.push_str("            local.get $buffer_start\n");
        self.output.push_str("            i32.const 1\n");
        self.output.push_str("            i32.sub\n");
        self.output.push_str("            local.tee $buffer_start\n");
        self.output.push_str("            local.get $digit\n");
        self.output.push_str("            i32.const 48  ;; '0'\n");
        self.output.push_str("            i32.add\n");
        self.output.push_str("            i32.store8\n");
        self.output.push_str("            \n");
        self.output.push_str("            ;; Divide by 10\n");
        self.output.push_str("            local.get $num\n");
        self.output.push_str("            i32.const 10\n");
        self.output.push_str("            i32.div_u\n");
        self.output.push_str("            local.set $num\n");
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
        self.output.push_str("    ;; Calculate length\n");
        self.output.push_str("    local.get $buffer_end\n");
        self.output.push_str("    local.get $buffer_start\n");
        self.output.push_str("    i32.sub\n");
        self.output.push_str("    local.set $len\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Allocate new string\n");
        self.output.push_str("    local.get $len\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $new_str\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Write length prefix\n");
        self.output.push_str("    local.get $new_str\n");
        self.output.push_str("    local.get $len\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Copy from buffer to new string\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    local.set $i\n");
        self.output.push_str("    (block $copy_done\n");
        self.output.push_str("      (loop $copy\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        local.get $len\n");
        self.output.push_str("        i32.ge_u\n");
        self.output.push_str("        br_if $copy_done\n");
        self.output.push_str("        \n");
        self.output.push_str("        local.get $new_str\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $buffer_start\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        i32.load8_u\n");
        self.output.push_str("        i32.store8\n");
        self.output.push_str("        \n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.set $i\n");
        self.output.push_str("        br $copy\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    \n");
        self.output.push_str("    local.get $new_str\n");
        self.output.push_str("  )\n\n");

        self.functions.insert("int_to_string".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: Some(WasmType::I32),
        });
        self.function_return_types.insert("int_to_string".to_string(), "String".to_string());

        Ok(())
    }

    fn generate_prelude_functions(&mut self) -> Result<(), CodeGenError> {
        // ============================================================
        // Prelude functions (matching std/prelude.rl)
        // ============================================================
        self.output.push_str("\n  ;; Prelude functions\n");

        // not: (Bool) -> Bool
        self.output.push_str("  (func $not (param $b i32) (result i32)\n");
        self.output.push_str("    local.get $b\n");
        self.output.push_str("    i32.eqz\n");
        self.output.push_str("  )\n");
        self.functions.insert("not".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: Some(WasmType::I32),
        });

        // identity_int: (Int) -> Int
        self.output.push_str("  (func $identity_int (param $x i32) (result i32)\n");
        self.output.push_str("    local.get $x\n");
        self.output.push_str("  )\n");
        self.functions.insert("identity_int".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: Some(WasmType::I32),
        });

        // identity_bool: (Bool) -> Bool
        self.output.push_str("  (func $identity_bool (param $x i32) (result i32)\n");
        self.output.push_str("    local.get $x\n");
        self.output.push_str("  )\n");
        self.functions.insert("identity_bool".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: Some(WasmType::I32),
        });

        // eq_int: (Int, Int) -> Bool
        self.output.push_str("  (func $eq_int (param $a i32) (param $b i32) (result i32)\n");
        self.output.push_str("    local.get $a\n");
        self.output.push_str("    local.get $b\n");
        self.output.push_str("    i32.eq\n");
        self.output.push_str("  )\n");
        self.functions.insert("eq_int".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });

        // ne_int: (Int, Int) -> Bool
        self.output.push_str("  (func $ne_int (param $a i32) (param $b i32) (result i32)\n");
        self.output.push_str("    local.get $a\n");
        self.output.push_str("    local.get $b\n");
        self.output.push_str("    i32.ne\n");
        self.output.push_str("  )\n");
        self.functions.insert("ne_int".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });

        // lt_int: (Int, Int) -> Bool
        self.output.push_str("  (func $lt_int (param $a i32) (param $b i32) (result i32)\n");
        self.output.push_str("    local.get $a\n");
        self.output.push_str("    local.get $b\n");
        self.output.push_str("    i32.lt_s\n");
        self.output.push_str("  )\n");
        self.functions.insert("lt_int".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });

        // le_int: (Int, Int) -> Bool
        self.output.push_str("  (func $le_int (param $a i32) (param $b i32) (result i32)\n");
        self.output.push_str("    local.get $a\n");
        self.output.push_str("    local.get $b\n");
        self.output.push_str("    i32.le_s\n");
        self.output.push_str("  )\n");
        self.functions.insert("le_int".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });

        // gt_int: (Int, Int) -> Bool
        self.output.push_str("  (func $gt_int (param $a i32) (param $b i32) (result i32)\n");
        self.output.push_str("    local.get $a\n");
        self.output.push_str("    local.get $b\n");
        self.output.push_str("    i32.gt_s\n");
        self.output.push_str("  )\n");
        self.functions.insert("gt_int".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });

        // ge_int: (Int, Int) -> Bool
        self.output.push_str("  (func $ge_int (param $a i32) (param $b i32) (result i32)\n");
        self.output.push_str("    local.get $a\n");
        self.output.push_str("    local.get $b\n");
        self.output.push_str("    i32.ge_s\n");
        self.output.push_str("  )\n");
        self.functions.insert("ge_int".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });

        // add: (Int, Int) -> Int
        self.output.push_str("  (func $add (param $a i32) (param $b i32) (result i32)\n");
        self.output.push_str("    local.get $a\n");
        self.output.push_str("    local.get $b\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("  )\n");
        self.functions.insert("add".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });

        // sub: (Int, Int) -> Int
        self.output.push_str("  (func $sub (param $a i32) (param $b i32) (result i32)\n");
        self.output.push_str("    local.get $a\n");
        self.output.push_str("    local.get $b\n");
        self.output.push_str("    i32.sub\n");
        self.output.push_str("  )\n");
        self.functions.insert("sub".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });

        // mul: (Int, Int) -> Int
        self.output.push_str("  (func $mul (param $a i32) (param $b i32) (result i32)\n");
        self.output.push_str("    local.get $a\n");
        self.output.push_str("    local.get $b\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("  )\n");
        self.functions.insert("mul".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });

        // div: (Int, Int) -> Int
        self.output.push_str("  (func $div (param $a i32) (param $b i32) (result i32)\n");
        self.output.push_str("    local.get $a\n");
        self.output.push_str("    local.get $b\n");
        self.output.push_str("    i32.div_s\n");
        self.output.push_str("  )\n");
        self.functions.insert("div".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });

        // mod: (Int, Int) -> Int
        self.output.push_str("  (func $mod (param $a i32) (param $b i32) (result i32)\n");
        self.output.push_str("    local.get $a\n");
        self.output.push_str("    local.get $b\n");
        self.output.push_str("    i32.rem_s\n");
        self.output.push_str("  )\n");
        self.functions.insert("mod".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });

        // neg: (Int) -> Int
        self.output.push_str("  (func $neg (param $x i32) (result i32)\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    local.get $x\n");
        self.output.push_str("    i32.sub\n");
        self.output.push_str("  )\n");
        self.functions.insert("neg".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: Some(WasmType::I32),
        });

        // unit: () -> Unit (returns nothing meaningful)
        self.output.push_str("  (func $unit\n");
        self.output.push_str("    ;; Does nothing, represents unit value\n");
        self.output.push_str("  )\n");
        self.functions.insert("unit".to_string(), FunctionSig {
            _params: vec![],
            result: None,
        });

        // panic: (String) -> Unit
        self.output.push_str("  (func $panic (param $msg i32)\n");
        self.output.push_str("    local.get $msg\n");
        self.output.push_str("    call $println\n");
        self.output.push_str("    unreachable\n");
        self.output.push_str("  )\n");
        self.functions.insert("panic".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: None,
        });

        // assert: (Bool, String) -> Unit
        self.output.push_str("  (func $assert (param $cond i32) (param $msg i32)\n");
        self.output.push_str("    local.get $cond\n");
        self.output.push_str("    i32.eqz\n");
        self.output.push_str("    (if\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        local.get $msg\n");
        self.output.push_str("        call $panic\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n");
        self.functions.insert("assert".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
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
                TopDecl::Export(export) => {
                    match export.item.as_ref() {
                        TopDecl::Function(func) => {
                            self.collect_strings_from_block(&func.body)?;
                        }
                        TopDecl::Binding(val) => {
                            self.collect_strings_from_expr(&val.value)?;
                        }
                        _ => {}
                    }
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
            Expr::WithLifetime(with_lifetime) => {
                self.collect_strings_from_block(&with_lifetime.body)?;
            }
            Expr::Await(_) | Expr::Spawn(_) => {
                // No strings in await/spawn expressions themselves
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
    
    /// Convert an AST Type to a string for type tracking
    fn type_to_string(&self, ty: &crate::ast::Type) -> String {
        match ty {
            crate::ast::Type::Named(name) => name.clone(),
            crate::ast::Type::Generic(base, params) => {
                let params_str: Vec<String> = params.iter()
                    .map(|p| self.type_to_string(p))
                    .collect();
                format!("{}<{}>", base, params_str.join(", "))
            }
            crate::ast::Type::Function(params, return_type) => {
                let params_str: Vec<String> = params.iter()
                    .map(|p| self.type_to_string(p))
                    .collect();
                format!("({}) -> {}", params_str.join(", "), self.type_to_string(return_type))
            }
            crate::ast::Type::Temporal(name, temporals) => {
                // Temporal types like File<~f>
                format!("{}<{}>", name, temporals.iter()
                    .map(|t| format!("~{}", t))
                    .collect::<Vec<_>>()
                    .join(", "))
            }
        }
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

        // Determine return type from function body analysis
        // main function never returns a value in WASM
        // Other functions: infer from final expression or last statement
        let result = if func.name == "main" {
            None
        } else if let Some(ref return_ty) = func.return_type {
            // Explicit return type - check if it's Unit
            let type_name = self.type_to_string(return_ty);
            if type_name == "Unit" || type_name == "()" {
                None
            } else {
                Some(WasmType::I32)
            }
        } else if let Some(ref expr) = func.body.expr {
            // Infer from final expression
            let inferred = self.infer_return_type_from_expr(expr)?;
            if inferred == "Unit" {
                None
            } else {
                Some(WasmType::I32)
            }
        } else if let Some(Stmt::Expr(last_expr)) = func.body.statements.last() {
            // Infer from last statement if it's an expression
            let inferred = self.infer_return_type_from_expr(last_expr)?;
            if inferred == "Unit" {
                None
            } else {
                Some(WasmType::I32)
            }
        } else {
            // Last statement is Binding or Assignment, or block is empty
            // These do not produce values, so the function returns Unit
            match func.body.statements.last() {
                None => None, // Empty block returns Unit
                Some(Stmt::Binding(_)) => None, // Bindings don't produce values
                Some(Stmt::Assignment(_)) => None, // Assignments don't produce values
                Some(Stmt::Expr(_)) => unreachable!(), // Already handled above
            }
        };

        self.functions.insert(func.name.clone(), FunctionSig {
            _params: params,
            result,
        });

        // Track return type for println dispatch
        // If explicit return type is provided, use it
        // Otherwise, infer from the function body (final expression or last statement)
        let return_type = if let Some(ref ty) = func.return_type {
            self.type_to_string(ty)
        } else if let Some(ref expr) = func.body.expr {
            // Infer return type from the body expression
            self.infer_return_type_from_expr(expr)?
        } else if let Some(Stmt::Expr(last_expr)) = func.body.statements.last() {
            // Infer from last statement if it's an expression
            self.infer_return_type_from_expr(last_expr)?
        } else {
            // Last statement is Binding or Assignment, or block is empty
            // These do not produce values, so the function returns Unit
            match func.body.statements.last() {
                None => "Unit".to_string(), // Empty block returns Unit
                Some(Stmt::Binding(_)) => "Unit".to_string(), // Bindings don't produce values
                Some(Stmt::Assignment(_)) => "Unit".to_string(), // Assignments don't produce values
                Some(Stmt::Expr(_)) => unreachable!(), // Already handled above
            }
        };
        self.function_return_types.insert(func.name.clone(), return_type);

        Ok(())
    }

    /// Infer return type from an expression without depending on function_return_types
    /// This is used during function registration before all functions are registered
    fn infer_return_type_from_expr(&self, expr: &Expr) -> Result<String, CodeGenError> {
        match expr {
            Expr::IntLit(_) => Ok("Int".to_string()),
            Expr::FloatLit(_) => Ok("Float".to_string()),
            Expr::StringLit(_) => Ok("String".to_string()),
            Expr::BoolLit(_) => Ok("Bool".to_string()),
            Expr::Unit => Ok("Unit".to_string()),
            Expr::Block(block) => {
                if let Some(ref final_expr) = block.expr {
                    self.infer_return_type_from_expr(final_expr)
                } else {
                    Ok("Unit".to_string())
                }
            }
            Expr::Then(then_expr) => {
                // Infer from the then block
                if let Some(ref final_expr) = then_expr.then_block.expr {
                    self.infer_return_type_from_expr(final_expr)
                } else if let Some(ref else_block) = then_expr.else_block {
                    if let Some(ref final_expr) = else_block.expr {
                        self.infer_return_type_from_expr(final_expr)
                    } else {
                        Ok("Unit".to_string())
                    }
                } else {
                    Ok("Unit".to_string())
                }
            }
            Expr::Call(call) => {
                // Check built-in functions
                if let Expr::Ident(func_name) = call.function.as_ref() {
                    match func_name.as_str() {
                        // String conversion functions
                        "int_to_string" | "float_to_string" | "bool_to_string" => Ok("String".to_string()),
                        "string_to_int" | "string_length" | "char_to_int" => Ok("Int".to_string()),
                        "string_to_float" => Ok("Float".to_string()),
                        "int_to_char" => Ok("Char".to_string()),
                        // Array/List functions
                        "array_get" | "list_get" | "array_length" | "list_length" => Ok("Int".to_string()),
                        "array_set" | "list_push" | "list_pop" => Ok("Unit".to_string()),
                        "new_list" | "new_array" => Ok("List".to_string()),
                        // I/O functions
                        "println" | "print" | "print_int" | "print_float" => Ok("Unit".to_string()),
                        "read_line" => Ok("String".to_string()),
                        // Allocation
                        "allocate" => Ok("Int".to_string()),
                        // Option/Result constructors
                        "some" | "Some" => Ok("Option".to_string()),
                        "none" | "None" => Ok("Option".to_string()),
                        "ok" | "Ok" => Ok("Result".to_string()),
                        "err" | "Err" => Ok("Result".to_string()),
                        "unwrap" | "unwrap_or" => Ok("Int".to_string()), // Generic, but default to Int
                        _ => {
                            // Check if we already have this function registered
                            if let Some(return_type) = self.function_return_types.get(func_name) {
                                Ok(return_type.clone())
                            } else {
                                Err(CodeGenError::CannotInferType(
                                    format!("unknown return type for function '{}'", func_name)
                                ))
                            }
                        }
                    }
                } else {
                    Err(CodeGenError::CannotInferType(
                        "cannot infer return type of non-identifier function call".to_string()
                    ))
                }
            }
            Expr::Binary(_) => Ok("Int".to_string()), // Arithmetic/comparison ops return Int
            Expr::Ident(name) => {
                if let Some(type_name) = self.var_types.get(name) {
                    Ok(type_name.clone())
                } else {
                    Err(CodeGenError::CannotInferType(
                        format!("unknown type for variable '{}'", name)
                    ))
                }
            }
            Expr::RecordLit(rl) => Ok(rl.name.clone()),
            Expr::While(_) => Ok("Unit".to_string()),
            Expr::With(with) => {
                // Infer from the body block
                if let Some(ref final_expr) = with.body.expr {
                    self.infer_return_type_from_expr(final_expr)
                } else {
                    Ok("Unit".to_string())
                }
            }
            Expr::WithLifetime(with_lifetime) => {
                // Infer from the body block
                if let Some(ref final_expr) = with_lifetime.body.expr {
                    self.infer_return_type_from_expr(final_expr)
                } else {
                    Ok("Unit".to_string())
                }
            }
            Expr::ListLit(_) => Ok("List".to_string()),
            Expr::ArrayLit(_) => Ok("Array".to_string()),
            Expr::Pipe(pipe) => {
                // Pipe expression type depends on the target
                match &pipe.target {
                    crate::ast::PipeTarget::Ident(name) => {
                        if let Some(return_type) = self.function_return_types.get(name) {
                            Ok(return_type.clone())
                        } else if name == "println" {
                            Ok("Unit".to_string())
                        } else {
                            // Pipe to a binding returns the value type
                            self.infer_return_type_from_expr(&pipe.expr)
                        }
                    }
                    crate::ast::PipeTarget::Expr(target_expr) => {
                        if let Expr::Ident(func_name) = target_expr.as_ref() {
                            if let Some(return_type) = self.function_return_types.get(func_name) {
                                return Ok(return_type.clone());
                            }
                        }
                        Err(CodeGenError::CannotInferType(
                            "cannot infer type of complex pipe target".to_string()
                        ))
                    }
                }
            }
            Expr::Match(match_expr) => {
                // Infer from the first arm
                if let Some(first_arm) = match_expr.arms.first() {
                    if let Some(ref final_expr) = first_arm.body.expr {
                        self.infer_return_type_from_expr(final_expr)
                    } else {
                        Ok("Unit".to_string())
                    }
                } else {
                    Ok("Unit".to_string())
                }
            }
            // Option type constructors
            Expr::Some(_) => Ok("Option".to_string()),
            Expr::None => Ok("Option".to_string()),
            Expr::NoneTyped(_) => Ok("Option".to_string()),
            // Result type constructors
            Expr::Ok(_) => Ok("Result".to_string()),
            Expr::Err(_) => Ok("Result".to_string()),
            // Field access - need to look up the field type
            Expr::FieldAccess(obj, field) => {
                let var_name = self.expr_to_var_name(obj);
                if let Some(record_type) = self.var_types.get(&var_name) {
                    if let Some(fields) = self.records.get(record_type) {
                        for (field_name, field_type) in fields {
                            if field_name == field {
                                return Ok(self.type_to_string(field_type));
                            }
                        }
                    }
                }
                Err(CodeGenError::CannotInferType(
                    format!("cannot infer type of field access '{}.{}'", var_name, field)
                ))
            }
            // Clone/Freeze return the same type as the input
            Expr::Clone(clone_expr) => self.infer_return_type_from_expr(&clone_expr.base),
            Expr::Freeze(inner) => self.infer_return_type_from_expr(inner),
            Expr::PrototypeClone(proto) => {
                // The result type is the same as the prototype being cloned
                if let Some(type_name) = self.var_types.get(&proto.base) {
                    Ok(type_name.clone())
                } else {
                    // The prototype name itself might be a record type
                    Ok(proto.base.clone())
                }
            }
            // Lambda returns a function type
            Expr::Lambda(_) => Ok("Function".to_string()),
            // It is typically an Int
            Expr::It => Ok("Int".to_string()),
            _ => Err(CodeGenError::CannotInferType(
                format!("cannot infer type of expression: {:?}", std::mem::discriminant(expr))
            ))
        }
    }

    fn register_record_methods(&mut self, _record: &RecordDecl) -> Result<(), CodeGenError> {
        // Records don't have methods in the current AST
        Ok(())
    }
    
    fn register_record_definition(&mut self, record: &RecordDecl) -> Result<(), CodeGenError> {
        let mut fields = Vec::new();
        let mut field_offsets = HashMap::new();
        let mut offset = 0u32;
        
        for field in &record.fields {
            fields.push((field.name.clone(), field.ty.clone()));
            field_offsets.insert(field.name.clone(), offset);
            
            // Calculate field size based on type
            let field_size = match &field.ty {
                Type::Named(name) => match name.as_str() {
                    "Int32" | "Boolean" | "Char" => 4,
                    "Float64" => 8,
                    _ => 4, // Pointers are 4 bytes
                },
                _ => 4, // Default to pointer size
            };
            
            offset += field_size;
        }
        
        self.records.insert(record.name.clone(), fields);
        self.record_field_offsets.insert(record.name.clone(), field_offsets);
        
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
                    "Int" | "Int32" | "Int64" | "Boolean" | "Bool" | "Char" => Ok(WasmType::I32),
                    "Float" | "Float64" => Ok(WasmType::F64),
                    "String" => Ok(WasmType::I32), // String is a pointer
                    "Unit" => Ok(WasmType::I32),
                    _ => {
                        // Check if it's a known record type
                        if self.records.contains_key(name) {
                            Ok(WasmType::I32) // Records are pointers
                        } else if self.is_type_parameter(name) {
                            // Type parameters are represented as I32 (pointer/value)
                            Ok(WasmType::I32)
                        } else {
                            Err(CodeGenError::UnsupportedType(
                                format!("unknown type '{}' cannot be converted to WASM type", name)
                            ))
                        }
                    }
                }
            }
            Type::Generic(name, _params) => {
                match name.as_str() {
                    "List" | "Option" | "Result" | "Array" => Ok(WasmType::I32), // All are pointers
                    _ => {
                        // Check if it's a known record type (user-defined generic record)
                        if self.records.contains_key(name) {
                            Ok(WasmType::I32) // Records are pointers
                        } else {
                            Err(CodeGenError::UnsupportedType(
                                format!("unknown generic type '{}' cannot be converted to WASM type", name)
                            ))
                        }
                    }
                }
            }
            Type::Function(_, _) => Ok(WasmType::I32), // Function pointers
            Type::Temporal(name, _temporals) => {
                // Temporal types are treated like their base type
                self.convert_type(&Type::Named(name.clone()))
            }
        }
    }
    
    // Generate specialized versions of generic functions
    fn generate_generic_function(&mut self, func: &FunDecl) -> Result<(), CodeGenError> {
        // Store all generic functions for later monomorphization
        // Built-in functions (println, new_list, etc.) will be handled
        // specially in generate_monomorphized_function based on type
        self.generic_functions.insert(func.name.clone(), func.clone());
        Ok(())
    }

    /// Record that a generic function is being instantiated with specific types
    fn record_instantiation(&mut self, func_name: &str, type_args: Vec<String>) -> String {
        // Generate mangled name: identity_Int, swap_Int_String, etc.
        let mangled_name = if type_args.is_empty() {
            func_name.to_string()
        } else {
            format!("{}_{}", func_name, type_args.join("_"))
        };

        // Check if this instantiation already exists
        let instantiations = self.instantiations.entry(func_name.to_string()).or_insert_with(Vec::new);
        if !instantiations.iter().any(|(args, _)| args == &type_args) {
            instantiations.push((type_args, mangled_name.clone()));
        }

        mangled_name
    }

    /// Generate all monomorphized versions of generic functions
    fn generate_monomorphized_functions(&mut self) -> Result<(), CodeGenError> {
        // Clone to avoid borrow issues
        let generic_functions = self.generic_functions.clone();
        let instantiations = self.instantiations.clone();

        for (func_name, type_instantiations) in instantiations {
            if let Some(generic_func) = generic_functions.get(&func_name) {
                for (type_args, mangled_name) in type_instantiations {
                    self.generate_monomorphized_function(generic_func, &type_args, &mangled_name)?;
                }
            }
        }

        Ok(())
    }

    /// Generate a monomorphized version of a generic function
    fn generate_monomorphized_function(
        &mut self,
        func: &FunDecl,
        type_args: &[String],
        mangled_name: &str
    ) -> Result<(), CodeGenError> {
        // Handle built-in generic functions specially
        if self.generate_builtin_monomorphization(&func.name, type_args, mangled_name)? {
            return Ok(());
        }

        // Build type substitution map
        let mut type_subst: HashMap<String, String> = HashMap::new();
        for (i, type_param) in func.type_params.iter().enumerate() {
            if i < type_args.len() {
                type_subst.insert(type_param.name.clone(), type_args[i].clone());
            }
        }

        self.current_function = Some(mangled_name.to_string());
        self.push_scope();

        // Collect locals
        let mut locals: Vec<(String, WasmType)> = Vec::new();
        self.collect_locals_from_block(&func.body, &mut locals)?;

        // Function header with mangled name
        self.output.push_str(&format!("  (func ${}", mangled_name));

        // Parameters with substituted types
        let mut next_idx = 0u32;
        for param in func.params.iter() {
            let substituted_type = self.substitute_type(&param.ty, &type_subst);
            let wasm_type = self.convert_type(&substituted_type)?;
            self.output.push_str(&format!(" (param ${} {})", param.name, self.wasm_type_str(wasm_type)));
            self.add_local(&param.name, next_idx);

            // Track parameter type
            let type_name = self.type_to_string(&substituted_type);
            self.var_types.insert(param.name.clone(), type_name);

            next_idx += 1;
        }

        // Result type with substitution
        if let Some(ref ret_type) = func.return_type {
            let substituted_ret = self.substitute_type(ret_type, &type_subst);
            let wasm_ret = self.convert_type(&substituted_ret)?;
            self.output.push_str(&format!(" (result {})", self.wasm_type_str(wasm_ret)));
        }
        self.output.push_str("\n");

        // Declare locals
        for (i, (name, wasm_type)) in locals.iter().enumerate() {
            self.output.push_str(&format!("    (local ${} {})\n", name, self.wasm_type_str(*wasm_type)));
            self.add_local(name, next_idx + i as u32);
        }

        // Generate body
        self.generate_block(&func.body)?;

        self.output.push_str("  )\n\n");
        self.pop_scope();
        self.current_function = None;

        Ok(())
    }

    /// Generate built-in function specializations (println, new_list, etc.)
    /// Returns true if handled, false if should use normal monomorphization
    fn generate_builtin_monomorphization(
        &mut self,
        func_name: &str,
        type_args: &[String],
        mangled_name: &str
    ) -> Result<bool, CodeGenError> {
        match func_name {
            "println" => {
                self.generate_println_mono(type_args, mangled_name)?;
                Ok(true)
            }
            "new_list" => {
                self.generate_new_list_mono(type_args, mangled_name)?;
                Ok(true)
            }
            "list_add" => {
                self.generate_list_add_mono(type_args, mangled_name)?;
                Ok(true)
            }
            "some" => {
                self.generate_some_mono(type_args, mangled_name)?;
                Ok(true)
            }
            "none" => {
                self.generate_none_mono(type_args, mangled_name)?;
                Ok(true)
            }
            _ => Ok(false)
        }
    }

    /// Generate println specialization for a specific type
    fn generate_println_mono(&mut self, type_args: &[String], mangled_name: &str) -> Result<(), CodeGenError> {
        if type_args.is_empty() {
            return Ok(());
        }

        let ty = &type_args[0];
        self.output.push_str(&format!("  (func ${} (param $x i32)\n", mangled_name));

        match ty.as_str() {
            "String" => {
                self.output.push_str("    local.get $x\n");
                self.output.push_str("    call $println\n");
            }
            "Int" | "Int32" => {
                self.output.push_str("    local.get $x\n");
                self.output.push_str("    call $print_int\n");
            }
            "Float" | "Float64" => {
                self.output.push_str("    local.get $x\n");
                self.output.push_str("    call $print_float\n");
            }
            "Bool" => {
                self.output.push_str("    local.get $x\n");
                self.output.push_str("    call $print_bool\n");
            }
            _ => {
                // For other types, try to print as int (fallback)
                self.output.push_str("    local.get $x\n");
                self.output.push_str("    call $print_int\n");
            }
        }

        self.output.push_str("  )\n\n");
        Ok(())
    }

    /// Generate new_list specialization for a specific type
    fn generate_new_list_mono(&mut self, type_args: &[String], mangled_name: &str) -> Result<(), CodeGenError> {
        // new_list<T> creates an empty list
        // Returns a pointer to [length=0, capacity=4, ...data...]
        self.output.push_str(&format!("  (func ${} (result i32)\n", mangled_name));
        self.output.push_str("    (local $ptr i32)\n");
        self.output.push_str("    ;; Allocate list header + initial capacity (4 elements)\n");
        self.output.push_str("    global.get $heap_ptr\n");
        self.output.push_str("    local.set $ptr\n");
        self.output.push_str("    ;; Store length = 0\n");
        self.output.push_str("    local.get $ptr\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    ;; Store capacity = 4\n");
        self.output.push_str("    local.get $ptr\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    ;; Advance heap: header (8) + capacity * 4\n");
        self.output.push_str("    global.get $heap_ptr\n");
        self.output.push_str("    i32.const 24\n");  // 8 + 4*4
        self.output.push_str("    i32.add\n");
        self.output.push_str("    global.set $heap_ptr\n");
        self.output.push_str("    local.get $ptr\n");
        self.output.push_str("  )\n\n");

        let _ = type_args; // Type doesn't affect allocation
        Ok(())
    }

    /// Generate list_add specialization for a specific type
    fn generate_list_add_mono(&mut self, type_args: &[String], mangled_name: &str) -> Result<(), CodeGenError> {
        // list_add<T> adds an element to a list
        self.output.push_str(&format!("  (func ${} (param $list i32) (param $item i32) (result i32)\n", mangled_name));
        self.output.push_str("    (local $len i32)\n");
        self.output.push_str("    (local $cap i32)\n");
        self.output.push_str("    (local $data_ptr i32)\n");
        self.output.push_str("    ;; Get current length\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $len\n");
        self.output.push_str("    ;; Calculate data pointer: list + 8 + len * 4\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $len\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.set $data_ptr\n");
        self.output.push_str("    ;; Store item\n");
        self.output.push_str("    local.get $data_ptr\n");
        self.output.push_str("    local.get $item\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    ;; Increment length\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    local.get $len\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    ;; Return list\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("  )\n\n");

        let _ = type_args;
        Ok(())
    }

    /// Generate some specialization for a specific type
    fn generate_some_mono(&mut self, type_args: &[String], mangled_name: &str) -> Result<(), CodeGenError> {
        // some<T> wraps a value in Option::Some
        // Option layout: [tag=1, value]
        self.output.push_str(&format!("  (func ${} (param $value i32) (result i32)\n", mangled_name));
        self.output.push_str("    (local $ptr i32)\n");
        self.output.push_str("    global.get $heap_ptr\n");
        self.output.push_str("    local.set $ptr\n");
        self.output.push_str("    ;; Store tag = 1 (Some)\n");
        self.output.push_str("    local.get $ptr\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    ;; Store value\n");
        self.output.push_str("    local.get $ptr\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $value\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    ;; Advance heap\n");
        self.output.push_str("    global.get $heap_ptr\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    global.set $heap_ptr\n");
        self.output.push_str("    local.get $ptr\n");
        self.output.push_str("  )\n\n");

        let _ = type_args;
        Ok(())
    }

    /// Generate none specialization for a specific type
    fn generate_none_mono(&mut self, type_args: &[String], mangled_name: &str) -> Result<(), CodeGenError> {
        // none<T> creates Option::None
        // Option layout: [tag=0]
        self.output.push_str(&format!("  (func ${} (result i32)\n", mangled_name));
        self.output.push_str("    (local $ptr i32)\n");
        self.output.push_str("    global.get $heap_ptr\n");
        self.output.push_str("    local.set $ptr\n");
        self.output.push_str("    ;; Store tag = 0 (None)\n");
        self.output.push_str("    local.get $ptr\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    ;; Advance heap\n");
        self.output.push_str("    global.get $heap_ptr\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    global.set $heap_ptr\n");
        self.output.push_str("    local.get $ptr\n");
        self.output.push_str("  )\n\n");

        let _ = type_args;
        Ok(())
    }

    /// Substitute type parameters with concrete types
    fn substitute_type(&self, ty: &Type, subst: &HashMap<String, String>) -> Type {
        match ty {
            Type::Named(name) => {
                if let Some(concrete) = subst.get(name) {
                    Type::Named(concrete.clone())
                } else {
                    ty.clone()
                }
            }
            Type::Generic(name, params) => {
                let new_params: Vec<Type> = params.iter()
                    .map(|p| self.substitute_type(p, subst))
                    .collect();
                Type::Generic(name.clone(), new_params)
            }
            Type::Function(param_types, ret_type) => {
                let new_params: Vec<Type> = param_types.iter()
                    .map(|p| self.substitute_type(p, subst))
                    .collect();
                let new_ret = Box::new(self.substitute_type(ret_type, subst));
                Type::Function(new_params, new_ret)
            }
            _ => ty.clone()
        }
    }

    /// Infer type arguments from a generic function call
    fn infer_type_args_from_call(&self, func_name: &str, args: &[Box<Expr>]) -> Result<Vec<String>, CodeGenError> {
        let generic_func = self.generic_functions.get(func_name)
            .ok_or_else(|| CodeGenError::UndefinedFunction(func_name.to_string()))?;

        let mut type_args = Vec::new();

        // Match each argument with the corresponding parameter
        for (i, param) in generic_func.params.iter().enumerate() {
            if i >= args.len() {
                break;
            }

            // Check if parameter type is a type parameter
            if let Type::Named(param_type_name) = &param.ty {
                // Check if this is one of the type parameters
                let is_type_param = generic_func.type_params.iter()
                    .any(|tp| &tp.name == param_type_name);

                if is_type_param {
                    // Infer the concrete type from the argument
                    let concrete_type = self.infer_expr_type_name(&args[i])?;

                    // Find the index of this type parameter
                    let param_idx = generic_func.type_params.iter()
                        .position(|tp| &tp.name == param_type_name);

                    if let Some(idx) = param_idx {
                        // Ensure we have space in type_args
                        while type_args.len() <= idx {
                            type_args.push(String::new());
                        }
                        type_args[idx] = concrete_type;
                    }
                }
            }
        }

        // Filter out empty strings
        type_args.retain(|s| !s.is_empty());

        Ok(type_args)
    }

    /// Infer the type name from an expression (for monomorphization)
    fn infer_expr_type_name(&self, expr: &Expr) -> Result<String, CodeGenError> {
        match expr {
            Expr::IntLit(_) => Ok("Int".to_string()),
            Expr::FloatLit(_) => Ok("Float".to_string()),
            Expr::StringLit(_) => Ok("String".to_string()),
            Expr::BoolLit(_) => Ok("Bool".to_string()),
            Expr::Unit => Ok("Unit".to_string()),
            Expr::Ident(name) => {
                // Check if we know the type of this variable
                if let Some(type_name) = self.var_types.get(name) {
                    Ok(type_name.clone())
                } else {
                    Err(CodeGenError::CannotInferType(
                        format!("unknown type for variable '{}'", name)
                    ))
                }
            }
            Expr::FieldAccess(object, field) => {
                // Get type of the record and the field
                let var_name = self.expr_to_var_name(object);
                if let Some(record_type) = self.var_types.get(&var_name) {
                    if let Some(fields) = self.records.get(record_type) {
                        for (field_name, field_type) in fields {
                            if field_name == field {
                                return Ok(self.type_to_string(field_type));
                            }
                        }
                        return Err(CodeGenError::CannotInferType(
                            format!("field '{}' not found in record type '{}'", field, record_type)
                        ));
                    }
                    return Err(CodeGenError::CannotInferType(
                        format!("'{}' is not a known record type", record_type)
                    ));
                }
                Err(CodeGenError::CannotInferType(
                    format!("cannot determine type of field access on '{}'", var_name)
                ))
            }
            Expr::RecordLit(rl) => Ok(rl.name.clone()),
            Expr::Block(block) => {
                // Get type from the block's final expression
                if let Some(ref final_expr) = block.expr {
                    self.infer_expr_type_name(final_expr)
                } else {
                    Ok("Unit".to_string())
                }
            }
            Expr::Then(then_expr) => {
                // Infer type from the then block (both branches should have same type)
                if let Some(ref final_expr) = then_expr.then_block.expr {
                    self.infer_expr_type_name(final_expr)
                } else if let Some(ref else_block) = then_expr.else_block {
                    if let Some(ref final_expr) = else_block.expr {
                        self.infer_expr_type_name(final_expr)
                    } else {
                        Ok("Unit".to_string())
                    }
                } else {
                    Ok("Unit".to_string())
                }
            }
            Expr::Call(call) => {
                // Look up the function's return type
                if let Expr::Ident(func_name) = call.function.as_ref() {
                    // Check built-in functions first
                    match func_name.as_str() {
                        // String conversion functions
                        "int_to_string" | "float_to_string" | "bool_to_string" => {
                            return Ok("String".to_string());
                        }
                        "string_to_int" | "string_length" | "char_to_int" => {
                            return Ok("Int".to_string());
                        }
                        "string_to_float" => return Ok("Float".to_string()),
                        "int_to_char" => return Ok("Char".to_string()),
                        // Array/List functions
                        "array_get" | "list_get" | "array_length" | "list_length" => {
                            return Ok("Int".to_string());
                        }
                        "array_set" | "list_push" | "list_pop" => {
                            return Ok("Unit".to_string());
                        }
                        "new_list" | "new_array" => return Ok("List".to_string()),
                        // I/O functions
                        "println" | "print" | "print_int" | "print_float" => {
                            return Ok("Unit".to_string());
                        }
                        "read_line" => return Ok("String".to_string()),
                        // Allocation
                        "allocate" => return Ok("Int".to_string()),
                        // Option/Result constructors
                        "some" | "Some" => return Ok("Option".to_string()),
                        "none" | "None" => return Ok("Option".to_string()),
                        "ok" | "Ok" => return Ok("Result".to_string()),
                        "err" | "Err" => return Ok("Result".to_string()),
                        "unwrap" | "unwrap_or" => return Ok("Int".to_string()),
                        _ => {}
                    }
                    // Check registered function return types
                    if let Some(return_type) = self.function_return_types.get(func_name) {
                        return Ok(return_type.clone());
                    }
                    return Err(CodeGenError::CannotInferType(
                        format!("unknown return type for function '{}'", func_name)
                    ));
                }
                Err(CodeGenError::CannotInferType(
                    "cannot infer return type of non-identifier function call".to_string()
                ))
            }
            Expr::Binary(_) => Ok("Int".to_string()), // Arithmetic/comparison ops return Int
            Expr::While(_) => Ok("Unit".to_string()),
            _ => Err(CodeGenError::CannotInferType(
                format!("cannot infer type of expression: {:?}", std::mem::discriminant(expr))
            ))
        }
    }

    /// Helper to extract variable name from expression
    fn expr_to_var_name(&self, expr: &Expr) -> String {
        match expr {
            Expr::Ident(name) => name.clone(),
            _ => String::new()
        }
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

            // Track parameter type for field access
            let type_name = self.type_to_string(&param.ty);
            self.var_types.insert(param.name.clone(), type_name);

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
        
        // Drop return value for functions that return Unit if the body leaves a value
        // This handles both final expressions and last statements that leave values
        let function_returns_unit = if let Some(sig) = self.functions.get(&func.name) {
            sig.result.is_none()
        } else {
            // If not registered, assume main returns Unit
            func.name == "main"
        };

        if function_returns_unit {
            let needs_drop = if let Some(expr) = &func.body.expr {
                // Has final expression - check if it leaves a value
                self.expr_leaves_value(expr)
            } else if let Some(Stmt::Expr(last_expr)) = func.body.statements.last() {
                // No final expression, but last statement might leave a value
                self.expr_leaves_value(last_expr)
            } else {
                false
            };

            if needs_drop {
                self.output.push_str("    drop\n");
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
        // If this is a lazy block, generate it as a closure
        if block.is_lazy {
            self.generate_lazy_block(block)
        } else {
            self.generate_block_internal(block, false)
        }
    }

    fn generate_block_as_expression(&mut self, block: &BlockExpr) -> Result<(), CodeGenError> {
        if block.is_lazy {
            self.generate_lazy_block(block)
        } else {
            self.generate_block_internal(block, true)
        }
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

    /// Generate a lazy block as a lambda/closure (with optional implicit 'it' parameter)
    fn generate_lazy_block(&mut self, block: &BlockExpr) -> Result<(), CodeGenError> {
        // A lazy block is generated as a lambda
        // If has_implicit_it is true, it has one parameter named 'it'
        // Otherwise, it has no parameters

        let lambda_idx = self.lambda_counter;
        self.lambda_counter += 1;
        let func_name = format!("$lazy_block_{}", lambda_idx);

        // Collect free variables from the block (for closure support)
        // For now, we'll generate a simple lambda without captures
        let free_vars: Vec<String> = vec![];  // TODO: implement free variable collection

        // Add to function table
        self.function_table.push(func_name.clone());

        // Generate the lambda function definition
        // Add 'it' parameter if block uses implicit 'it'
        if block.has_implicit_it {
            self.output.push_str(&format!("\n  (func {} (param $it i32) (result i32)\n", func_name));
        } else {
            self.output.push_str(&format!("\n  (func {} (result i32)\n", func_name));
        }

        // Create a new scope for the lambda
        self.push_scope();

        // Add 'it' to local scope if needed
        if block.has_implicit_it {
            self.add_local("it", 0);  // 'it' is parameter 0
        }

        // Generate block body (as eager block now, since we're inside the lambda function)
        // Create a modified block that is eager
        let mut eager_block = block.clone();
        eager_block.is_lazy = false;
        self.generate_block_internal(&eager_block, true)?;

        self.pop_scope();
        self.output.push_str("  )\n");

        // Return the function index to the stack
        let table_index = self.function_table.len() - 1;
        self.output.push_str(&format!("    i32.const {}\n", table_index));

        Ok(())
    }

    fn generate_binding(&mut self, bind: &BindDecl) -> Result<(), CodeGenError> {
        // First, check if there's an explicit type annotation
        if let Some(ref ty) = bind.ty {
            let type_name = self.type_to_string(ty);
            self.var_types.insert(bind.name.clone(), type_name);
        } else {
            // Infer type of the value for variable tracking
            let inferred_type = self.infer_return_type_from_expr(&bind.value)?;
            self.var_types.insert(bind.name.clone(), inferred_type);
        }

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
    
    fn generate_temporal_scope(&mut self, lifetime: &str, body: &BlockExpr) -> Result<(), CodeGenError> {
        // Create a new arena for this temporal scope
        let arena_addr = self.next_arena_addr;
        self.next_arena_addr += 0x1000; // Reserve 4KB for each arena
        
        // Push arena onto stack
        self.arena_stack.push(arena_addr);
        
        // Generate arena initialization
        self.output.push_str(&format!("    ;; Initialize temporal scope arena for {} at address 0x{:x}\n", lifetime, arena_addr));
        self.output.push_str(&format!("    i32.const {}\n", arena_addr));
        self.output.push_str("    call $arena_init\n");
        self.output.push_str("    drop\n"); // Drop arena address as we track it internally
        
        // Set this arena as current
        self.output.push_str(&format!("    i32.const {}\n", arena_addr));
        self.output.push_str("    global.set $current_arena\n");
        
        // Generate the body expressions
        self.generate_block_as_expression(body)?;
        
        // Clean up arena at scope end
        self.output.push_str(&format!("    ;; Clean up temporal scope arena for {}\n", lifetime));
        self.output.push_str(&format!("    i32.const {}\n", arena_addr));
        self.output.push_str("    call $arena_reset\n");
        
        // Restore previous arena if any
        self.arena_stack.pop();
        if let Some(prev_arena) = self.arena_stack.last() {
            self.output.push_str(&format!("    ;; Restore previous arena\n"));
            self.output.push_str(&format!("    i32.const {}\n", prev_arena));
            self.output.push_str("    global.set $current_arena\n");
        } else if let Some(default_arena) = self.default_arena {
            self.output.push_str(&format!("    ;; Restore default arena\n"));
            self.output.push_str(&format!("    i32.const {}\n", default_arena));
            self.output.push_str("    global.set $current_arena\n");
        } else {
            // No arena to restore
            self.output.push_str("    i32.const 0\n");
            self.output.push_str("    global.set $current_arena\n");
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
            Expr::It => {
                // 'it' is treated like a local variable named "it"
                let it_name = "it".to_string();
                if self.in_lambda_with_captures && self.captured_vars.contains(&it_name) {
                    self.output.push_str("    local.get $it_captured\n");
                } else if let Some(_idx) = self.lookup_local("it") {
                    self.output.push_str("    local.get $it\n");
                } else {
                    return Err(CodeGenError::UndefinedVariable(it_name));
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
                // Allocate memory for the record
                let field_count = record_lit.fields.len();
                let record_size = field_count * 4; // Simplified: 4 bytes per field
                
                self.output.push_str(&format!("    i32.const {}\n", record_size));
                self.output.push_str("    call $allocate\n");
                self.output.push_str("    local.tee $list_tmp\n"); // Save address
                
                // Get field offset map for this record type
                let offsets_map = self.record_field_offsets.get(&record_lit.name).cloned();
                
                // Store each field value
                for (idx, field) in record_lit.fields.iter().enumerate() {
                    self.output.push_str("    local.get $list_tmp\n");

                    // Use the registered field offset if available, otherwise calculate from position
                    let offset = offsets_map
                        .as_ref()
                        .and_then(|offsets| offsets.get(&field.name))
                        .copied()
                        .unwrap_or_else(|| idx as u32 * 4);

                    self.output.push_str(&format!("    i32.const {}\n", offset));
                    self.output.push_str("    i32.add\n");
                    self.generate_expr(&field.value)?;
                    self.output.push_str("    i32.store\n");
                }
                
                // Return the base address
                self.output.push_str("    local.get $list_tmp\n");
            }
            Expr::FieldAccess(obj_expr, field) => {
                // Generate object expression
                self.generate_expr(obj_expr)?;
                
                // Get the type of the object expression
                let record_name = if let Expr::Ident(var_name) = obj_expr.as_ref() {
                    // For identifiers, look up the type from var_types
                    let var_type = self.var_types.get(var_name)
                        .ok_or_else(|| CodeGenError::NotImplemented(
                            format!("field access on unknown variable: {}", var_name)
                        ))?
                        .clone();
                    // Strip generic type parameters if present (e.g., "Box<Int>" -> "Box")
                    if let Some(idx) = var_type.find('<') {
                        var_type[..idx].to_string()
                    } else {
                        var_type
                    }
                } else if let Some(obj_type) = self.expr_types.get(&(obj_expr.as_ref() as *const Expr as usize)) {
                    // Fallback to expr_types if available
                    if let Some(name) = obj_type.strip_suffix(&format!("<~{}>", field)) {
                        name.to_string()
                    } else if obj_type.contains('<') {
                        // Handle generic types like Point<~p>
                        if let Some(idx) = obj_type.find('<') {
                            obj_type[..idx].to_string()
                        } else {
                            obj_type.clone()
                        }
                    } else {
                        obj_type.clone()
                    }
                } else if let Expr::RecordLit(record_lit) = obj_expr.as_ref() {
                    // Direct record literal
                    record_lit.name.clone()
                } else {
                    return Err(CodeGenError::NotImplemented(
                        format!("field access for {}", field)
                    ));
                };
                
                // Look up the field offset
                let field_offset = self.record_field_offsets
                    .get(&record_name)
                    .and_then(|fields| fields.get(field))
                    .ok_or_else(|| CodeGenError::NotImplemented(
                        format!("field access for {} in record {}", field, record_name)
                    ))?;
                
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
            Expr::With(with) => {
                // Phase 5: Generate 'with' as a lazy block (function)
                // This makes it compatible with scope composition
                self.generate_with_as_scope(with)?;
            }
            Expr::WithLifetime(with_lifetime) => {
                self.generate_temporal_scope(&with_lifetime.lifetime, &with_lifetime.body)?;
            }
            Expr::Await(_) | Expr::Spawn(_) => {
                return Err(CodeGenError::NotImplemented("async operations".to_string()));
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
            Expr::ScopeCompose(sc) => {
                self.generate_scope_compose_expr(sc)?;
            }
            Expr::ScopeConcat(sc) => {
                self.generate_scope_concat_expr(sc)?;
            }
            Expr::Ok(expr) => {
                // Result::Ok - allocate tagged union: [tag=1, value]
                self.output.push_str("    ;; Ok(expr) literal\n");
                self.output.push_str("    i32.const 8\n");
                self.output.push_str("    call $allocate\n");
                self.output.push_str("    local.tee $match_tmp\n");

                // Store tag (1 for Ok)
                self.output.push_str("    i32.const 1\n");
                self.output.push_str("    i32.store\n");

                // Generate inner value
                self.generate_expr(expr)?;

                // Store value at offset 4
                self.output.push_str("    local.get $match_tmp\n");
                self.output.push_str("    i32.const 4\n");
                self.output.push_str("    i32.add\n");
                self.output.push_str("    i32.store\n");

                // Leave pointer on stack
                self.output.push_str("    local.get $match_tmp\n");
            }
            Expr::Err(expr) => {
                // Result::Err - allocate tagged union: [tag=0, value]
                self.output.push_str("    ;; Err(expr) literal\n");
                self.output.push_str("    i32.const 8\n");
                self.output.push_str("    call $allocate\n");
                self.output.push_str("    local.tee $match_tmp\n");

                // Store tag (0 for Err)
                self.output.push_str("    i32.const 0\n");
                self.output.push_str("    i32.store\n");

                // Generate inner value
                self.generate_expr(expr)?;

                // Store value at offset 4
                self.output.push_str("    local.get $match_tmp\n");
                self.output.push_str("    i32.const 4\n");
                self.output.push_str("    i32.add\n");
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

    fn generate_scope_compose_expr(&mut self, sc: &ScopeComposeExpr) -> Result<(), CodeGenError> {
        // Generate a new lambda that composes both scopes
        // For scope composition, we create a new function that:
        // 1. Executes the left scope
        // 2. Executes the right scope
        // 3. Returns a merged result

        let compose_idx = self.lambda_counter;
        self.lambda_counter += 1;
        let func_name = format!("$scope_compose_{}", compose_idx);

        self.function_table.push(func_name.clone());

        // Generate the composed function
        self.output.push_str(&format!("\n  (func {} (result i32)\n", func_name));

        // For now, just execute both scopes and return the right one
        // TODO: Implement proper scope merging with binding composition
        self.output.push_str("    ;; Execute left scope\n");
        self.generate_expr(&sc.left)?;
        self.output.push_str("    drop ;; discard left result for now\n");

        self.output.push_str("    ;; Execute right scope\n");
        self.generate_expr(&sc.right)?;
        self.output.push_str("    ;; Return right result\n");

        self.output.push_str("  )\n");

        // Return function table index
        let table_index = self.function_table.len() - 1;
        self.output.push_str(&format!("    i32.const {}\n", table_index));

        Ok(())
    }

    fn generate_scope_concat_expr(&mut self, sc: &ScopeConcatExpr) -> Result<(), CodeGenError> {
        // Generate a new lambda that concatenates both scopes
        // For scope concatenation, we create a new function that:
        // 1. Executes the left scope
        // 2. Executes the right scope with access to left's result
        // 3. Returns the right scope's result

        let concat_idx = self.lambda_counter;
        self.lambda_counter += 1;
        let func_name = format!("$scope_concat_{}", concat_idx);

        self.function_table.push(func_name.clone());

        // Generate the concatenated function
        self.output.push_str(&format!("\n  (func {} (result i32)\n", func_name));

        // Execute left scope and discard result for now
        // TODO: Make left's result available to right scope
        self.output.push_str("    ;; Execute left scope\n");
        self.generate_expr(&sc.left)?;
        self.output.push_str("    drop ;; discard left result for now\n");

        // Execute right scope
        self.output.push_str("    ;; Execute right scope\n");
        self.generate_expr(&sc.right)?;
        self.output.push_str("    ;; Return right result\n");

        self.output.push_str("  )\n");

        // Return function table index
        let table_index = self.function_table.len() - 1;
        self.output.push_str(&format!("    i32.const {}\n", table_index));

        Ok(())
    }

    fn generate_with_as_scope(&mut self, with: &WithExpr) -> Result<(), CodeGenError> {
        // Phase 5: Generate 'with' expression as a lazy block (function)
        // This allows it to be composed with other scopes

        let with_idx = self.lambda_counter;
        self.lambda_counter += 1;
        let func_name = format!("$with_scope_{}", with_idx);

        self.function_table.push(func_name.clone());

        // Generate the with function
        self.output.push_str(&format!("\n  (func {} (result i32)\n", func_name));

        // For now, contexts don't generate actual code - they're compile-time constructs
        // Just execute the body block
        // TODO: Generate proper context setup/teardown for Arena and other contexts
        self.output.push_str("    ;; Execute body with contexts: ");
        for (i, ctx) in with.contexts.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.output.push_str(ctx);
        }
        self.output.push_str("\n");

        // Generate the body
        self.generate_block_internal(&with.body, false)?;

        self.output.push_str("  )\n");

        // Return function table index
        let table_index = self.function_table.len() - 1;
        self.output.push_str(&format!("    i32.const {}\n", table_index));

        Ok(())
    }

    fn generate_binary_expr(&mut self, binary: &BinaryExpr) -> Result<(), CodeGenError> {
        // Special case: detect scope composition (+ with lazy blocks/lambdas)
        if binary.op == BinaryOp::Add {
            let is_left_scope = matches!(&*binary.left,
                Expr::Block(b) if b.is_lazy) || matches!(&*binary.left, Expr::Lambda(_));
            let is_right_scope = matches!(&*binary.right,
                Expr::Block(b) if b.is_lazy) || matches!(&*binary.right, Expr::Lambda(_));

            if is_left_scope && is_right_scope {
                // This is scope composition, delegate to scope compose
                let sc = ScopeComposeExpr {
                    left: binary.left.clone(),
                    right: binary.right.clone(),
                };
                return self.generate_scope_compose_expr(&sc);
            }
        }

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
        // Special case: Detect scope concatenation
        // If function is a lazy block and single arg is also a lazy block, treat as scope concatenation
        if call.args.len() == 1 {
            let is_func_lazy = matches!(&*call.function,
                Expr::Block(b) if b.is_lazy) || matches!(&*call.function, Expr::Lambda(_));
            let is_arg_lazy = matches!(&*call.args[0],
                Expr::Block(b) if b.is_lazy) || matches!(&*call.args[0], Expr::Lambda(_));

            if is_func_lazy && is_arg_lazy {
                // This is scope concatenation: { a } { b }
                let sc = ScopeConcatExpr {
                    left: call.function.clone(),
                    right: call.args[0].clone(),
                };
                return self.generate_scope_concat_expr(&sc);
            }
        }

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
            
            // Handle polymorphic print/println functions
            if (func_name == "print" || func_name == "println") && !call.args.is_empty() {
                let resolved_name = self.resolve_generic_function_call(func_name, &call.args[0])?;
                self.output.push_str(&format!("    call ${}\n", resolved_name));
                return Ok(());
            }

            if self.functions.contains_key(func_name) {
                self.output.push_str(&format!("    call ${}\n", func_name));
            } else if self.generic_functions.contains_key(func_name) {
                // Generic function call - infer type arguments and use monomorphized version
                let type_args = self.infer_type_args_from_call(func_name, &call.args)?;
                let mangled_name = self.record_instantiation(func_name, type_args);
                self.output.push_str(&format!("    call ${}\n", mangled_name));
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
            Expr::StringLit(_) => Ok(WasmType::I32), // String pointer
            Expr::CharLit(_) => Ok(WasmType::I32),
            Expr::RecordLit(_) => Ok(WasmType::I32), // Record pointer
            Expr::ListLit(_) => Ok(WasmType::I32), // List pointer
            Expr::ArrayLit(_) => Ok(WasmType::I32), // Array pointer
            Expr::Ident(name) => {
                // Look up variable type
                if let Some(type_name) = self.var_types.get(name) {
                    match type_name.as_str() {
                        "Float" | "Float64" => Ok(WasmType::F64),
                        _ => Ok(WasmType::I32),
                    }
                } else {
                    Err(CodeGenError::CannotInferType(
                        format!("unknown type for variable '{}' in expression", name)
                    ))
                }
            }
            Expr::Binary(_) => Ok(WasmType::I32), // Binary ops return i32
            Expr::Block(block) => {
                if let Some(ref final_expr) = block.expr {
                    self.infer_expr_type(final_expr)
                } else {
                    Ok(WasmType::I32) // Unit
                }
            }
            Expr::Then(then_expr) => {
                if let Some(ref final_expr) = then_expr.then_block.expr {
                    self.infer_expr_type(final_expr)
                } else {
                    Ok(WasmType::I32) // Unit
                }
            }
            Expr::Call(call) => {
                if let Expr::Ident(func_name) = call.function.as_ref() {
                    if let Some(return_type) = self.function_return_types.get(func_name) {
                        match return_type.as_str() {
                            "Float" | "Float64" => Ok(WasmType::F64),
                            _ => Ok(WasmType::I32),
                        }
                    } else {
                        // Check built-in functions
                        match func_name.as_str() {
                            "string_to_float" => Ok(WasmType::F64),
                            _ => Ok(WasmType::I32), // Most builtins return i32
                        }
                    }
                } else {
                    Err(CodeGenError::CannotInferType(
                        "cannot infer type of non-identifier function call".to_string()
                    ))
                }
            }
            Expr::With(with) => {
                if let Some(ref final_expr) = with.body.expr {
                    self.infer_expr_type(final_expr)
                } else {
                    Ok(WasmType::I32) // Unit
                }
            }
            Expr::WithLifetime(with_lifetime) => {
                if let Some(ref final_expr) = with_lifetime.body.expr {
                    self.infer_expr_type(final_expr)
                } else {
                    Ok(WasmType::I32) // Unit
                }
            }
            Expr::While(_) => Ok(WasmType::I32), // Unit
            Expr::Pipe(pipe) => {
                // Pipe target determines the type
                match &pipe.target {
                    crate::ast::PipeTarget::Ident(name) => {
                        if let Some(return_type) = self.function_return_types.get(name) {
                            match return_type.as_str() {
                                "Float" | "Float64" => Ok(WasmType::F64),
                                _ => Ok(WasmType::I32),
                            }
                        } else {
                            // Pipe to binding or println
                            self.infer_expr_type(&pipe.expr)
                        }
                    }
                    crate::ast::PipeTarget::Expr(target_expr) => {
                        if let Expr::Ident(func_name) = target_expr.as_ref() {
                            if let Some(return_type) = self.function_return_types.get(func_name) {
                                match return_type.as_str() {
                                    "Float" | "Float64" => Ok(WasmType::F64),
                                    _ => Ok(WasmType::I32),
                                }
                            } else {
                                Ok(WasmType::I32)
                            }
                        } else {
                            Ok(WasmType::I32)
                        }
                    }
                }
            }
            Expr::Match(match_expr) => {
                if let Some(first_arm) = match_expr.arms.first() {
                    if let Some(ref final_expr) = first_arm.body.expr {
                        self.infer_expr_type(final_expr)
                    } else {
                        Ok(WasmType::I32) // Unit
                    }
                } else {
                    Ok(WasmType::I32) // Unit
                }
            }
            // Option and Result constructors are pointers
            Expr::Some(_) => Ok(WasmType::I32),
            Expr::None => Ok(WasmType::I32),
            Expr::NoneTyped(_) => Ok(WasmType::I32),
            Expr::Ok(_) => Ok(WasmType::I32),
            Expr::Err(_) => Ok(WasmType::I32),
            // Field access
            Expr::FieldAccess(_, _) => Ok(WasmType::I32), // Could be any type, default to I32
            // Clone/Freeze
            Expr::Clone(_) => Ok(WasmType::I32),
            Expr::Freeze(_) => Ok(WasmType::I32),
            Expr::PrototypeClone(_) => Ok(WasmType::I32),
            // Lambda
            Expr::Lambda(_) => Ok(WasmType::I32), // Function pointer
            // It (implicit parameter)
            Expr::It => Ok(WasmType::I32),
            // Scope operations
            Expr::ScopeCompose(_) => Ok(WasmType::I32),
            Expr::ScopeConcat(_) => Ok(WasmType::I32),
            _ => Err(CodeGenError::CannotInferType(
                format!("cannot infer WASM type of expression: {:?}", std::mem::discriminant(expr))
            ))
        }
    }

    /// Check if a name looks like a type parameter (single uppercase letter or common patterns)
    fn is_type_parameter(&self, name: &str) -> bool {
        // Single uppercase letters are typically type parameters
        if name.len() == 1 {
            let c = name.chars().next().unwrap();
            return c.is_ascii_uppercase();
        }
        // Common type parameter patterns
        matches!(name, "T" | "U" | "V" | "K" | "E" | "A" | "B" | "Item" | "Key" | "Value")
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
            Expr::WithLifetime(with_lifetime) => {
                self.collect_locals_from_block(&with_lifetime.body, locals)?;
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
        // For built-in print/println functions, dispatch based on argument type
        if name == "print" || name == "println" {
            match arg_expr {
                Expr::StringLit(_) => Ok("println".to_string()),  // String always uses println
                Expr::IntLit(_) => Ok("print_int".to_string()),
                Expr::FloatLit(_) => Ok("print_float".to_string()),
                Expr::BoolLit(_) => Ok("print_int".to_string()),  // Boolean as 0/1
                Expr::Ident(var_name) => {
                    // Look up variable type from var_types
                    if let Some(type_name) = self.var_types.get(var_name) {
                        if type_name == "String" {
                            Ok("println".to_string())
                        } else if type_name == "Int32" || type_name == "Int" || type_name == "Int64" {
                            Ok("print_int".to_string())
                        } else if type_name == "Float64" || type_name == "Float" {
                            Ok("print_float".to_string())
                        } else if type_name == "Boolean" || type_name == "Bool" {
                            Ok("print_int".to_string())  // Boolean as 0/1
                        } else {
                            Err(CodeGenError::UnsupportedType(
                                format!("println does not support type '{}' for variable '{}'", type_name, var_name)
                            ))
                        }
                    } else {
                        Err(CodeGenError::UndefinedVariable(
                            format!("cannot determine type of '{}' for println", var_name)
                        ))
                    }
                }
                Expr::Call(call) => {
                    // Look up function return type by extracting function name
                    if let Expr::Ident(func_name) = call.function.as_ref() {
                        if let Some(return_type) = self.function_return_types.get(func_name) {
                            if return_type == "String" {
                                return Ok("println".to_string());
                            } else if return_type == "Int32" || return_type == "Int" || return_type == "Int64" {
                                return Ok("print_int".to_string());
                            } else if return_type == "Float64" || return_type == "Float" {
                                return Ok("print_float".to_string());
                            } else if return_type == "Boolean" || return_type == "Bool" {
                                return Ok("print_int".to_string());
                            } else {
                                return Err(CodeGenError::UnsupportedType(
                                    format!("println does not support return type '{}' from function '{}'", return_type, func_name)
                                ));
                            }
                        }
                        Err(CodeGenError::UndefinedFunction(
                            format!("cannot determine return type of '{}' for println", func_name)
                        ))
                    } else {
                        Err(CodeGenError::NotImplemented(
                            "println with non-identifier function call".to_string()
                        ))
                    }
                }
                _ => {
                    // Try to get type from expr_types map
                    if let Some(type_name) = self.expr_types.get(&(arg_expr as *const Expr as usize)) {
                        if type_name == "String" {
                            Ok("println".to_string())
                        } else if type_name == "Int32" || type_name == "Int" {
                            Ok("print_int".to_string())
                        } else if type_name == "Float64" || type_name == "Float" {
                            Ok("print_float".to_string())
                        } else if type_name == "Boolean" || type_name == "Bool" {
                            Ok("print_int".to_string())
                        } else {
                            Err(CodeGenError::UnsupportedType(
                                format!("println does not support type '{}'", type_name)
                            ))
                        }
                    } else {
                        Err(CodeGenError::NotImplemented(
                            "cannot determine type of expression for println".to_string()
                        ))
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
                    if let Some(type_name) = self.expr_types.get(&(arg_expr as *const Expr as usize)) {
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

        self.output.push_str(&format!("    ;; List literal with {} elements\n", items.len()));

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

        self.output.push_str(&format!("    ;; Array literal with {} elements\n", items.len()));

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
            Pattern::Ok(inner_pattern) => {
                // Check if tag is 1 (Ok) for Result
                self.output.push_str("    local.tee $match_tmp ;; save for value extraction\n");
                self.output.push_str("    i32.load ;; load tag\n");
                self.output.push_str("    i32.const 1 ;; Ok tag\n");
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
            Pattern::Err(inner_pattern) => {
                // Check if tag is 0 (Err) for Result
                self.output.push_str("    local.tee $match_tmp ;; save for value extraction\n");
                self.output.push_str("    i32.load ;; load tag\n");
                self.output.push_str("    i32.const 0 ;; Err tag\n");
                self.output.push_str("    i32.eq\n");

                // If tag matches, match the inner pattern
                self.output.push_str("    (if (result i32)\n");
                self.output.push_str("      (then\n");
                self.output.push_str("        local.get $match_tmp\n");
                self.output.push_str("        i32.const 4\n");
                self.output.push_str("        i32.add\n");
                self.output.push_str("        i32.load ;; load error value from offset 4\n");

                let inner_bindings = self.generate_pattern_match(inner_pattern)?;
                bindings.extend(inner_bindings);

                self.output.push_str("      )\n");
                self.output.push_str("      (else\n");
                self.output.push_str("        i32.const 0 ;; tag mismatch\n");
                self.output.push_str("      )\n");
                self.output.push_str("    )\n");
            }
        }

        Ok(bindings)
    }

    fn generate_then_expr(&mut self, then: &ThenExpr) -> Result<(), CodeGenError> {
        // Generate condition
        self.generate_expr(&then.condition)?;

        // All if expressions produce a value (result i32)
        self.output.push_str("    (if (result i32)\n");
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
                self.output.push_str("        (if (result i32)\n");
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
    
    // Old specialization functions removed - now handled by generate_builtin_monomorphization
    
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
        if let Some(ty) = self.expr_types.get(&(expr as *const Expr as usize)) {
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