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
//! let source = "fun main: () -> Int32 = { 42 }";
//! let (remaining, program) = parse_program(source).unwrap();
//! assert!(remaining.trim().is_empty());
//!
//! let mut codegen = WasmCodeGen::new();
//! let wat = codegen.generate(&program).unwrap();
//! assert!(wat.contains("(module"));
//! ```

use crate::ast::*;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

const RECORD_TMP_MIN_COUNT: usize = 8;
const ARENA_SIZE_BYTES: u32 = 0x1000;
const WITH_ARENA_TMP_COUNT: usize = 8;

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

    /// Internal marker for codegen paths that are intentionally outside the current release.
    #[error("Unsupported feature: {0}")]
    NotImplemented(String),

    /// Feature not supported
    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),
}

struct VariantPayloadBindContext<'a> {
    field_template: &'a Type,
    expected_source: &'a Type,
    type_params: &'a [String],
    substitution: &'a mut HashMap<String, Type>,
}

/// WebAssembly Text Format (WAT) code generator.
///
/// Transforms a typed AST into executable WebAssembly code.
/// The generator handles memory management, function calls,
/// and the unique OSV syntax of Restrict Language.
pub struct WasmCodeGen {
    /// Variable to local index mapping (scoped)
    locals: Vec<HashMap<String, u32>>,
    /// Variable to Wasm ABI type mapping (scoped)
    local_types: Vec<HashMap<String, WasmType>>,
    /// Variable to declared/source Restrict type mapping (scoped)
    local_source_types: Vec<HashMap<String, Type>>,
    /// Source binding names to emitted Wasm local names (scoped).
    local_aliases: Vec<HashMap<String, String>>,
    /// Specific binding declarations that must use an emitted Wasm local alias.
    binding_local_aliases: HashMap<usize, String>,
    /// First observed Wasm type for a source local name during local collection.
    collected_local_types: HashMap<String, WasmType>,
    /// Counter for generated local aliases.
    local_alias_counter: usize,
    /// Local aliases to generic functions that must be instantiated from use-site ABI.
    generic_function_aliases: Vec<HashMap<String, String>>,
    /// Local aliases to deferred callable expressions whose ABI is learned from their use site.
    deferred_lambda_aliases: Vec<HashMap<String, Expr>>,
    /// Function signatures for type checking
    functions: HashMap<String, FunctionSig>,
    /// Function signatures in source-level Restrict types.
    function_source_sigs: HashMap<String, FunctionSourceSig>,
    /// Source declarations retained for on-demand generic specialization.
    function_decls: HashMap<String, FunDecl>,
    /// Specialized generic function names already emitted.
    specialized_functions: HashSet<String>,
    /// Functions declared through `export fun`.
    exported_functions: HashSet<String>,
    /// Top-level immutable globals and their Wasm ABI types.
    global_types: HashMap<String, WasmType>,
    /// Top-level immutable globals and their source-level Restrict types.
    global_source_types: HashMap<String, Type>,
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
    /// True once any `call_indirect` instruction has been emitted.
    has_indirect_closure_call: bool,
    /// Whether we're inside a lambda with captures
    in_lambda_with_captures: bool,
    /// List of captured variable names in current lambda
    captured_vars: Vec<String>,
    /// Record definitions: record_name -> fields
    records: HashMap<String, Vec<(String, Type)>>,
    /// Record generic type parameter names.
    record_type_params: HashMap<String, Vec<String>>,
    /// Record field offsets: record_name -> field_name -> offset
    record_field_offsets: HashMap<String, HashMap<String, u32>>,
    /// Variable types: var_name -> type_name (e.g., "Point", "Buffer")
    var_types: HashMap<String, String>,
    /// Temporal resource tracking: lifetime -> [(resource_ptr, cleanup_fn)]
    temporal_resources: HashMap<String, Vec<(String, String)>>,
    /// Stack of temporal scopes for nested lifetimes
    temporal_scope_stack: Vec<String>,
    /// Resource cleanup functions: type_name -> cleanup_function_name
    cleanup_functions: HashMap<String, String>,
    /// Current nesting depth for record literal temporaries.
    record_literal_depth: usize,
    /// Current nesting depth for record-pattern scratch temporaries.
    record_pattern_depth: usize,
    /// Number of record scratch temporaries declared in the current function.
    record_tmp_count: usize,
    /// Current nesting depth for `with Arena` restore temporaries.
    with_arena_depth: usize,
    /// Expected ABI for the lambda currently being generated.
    lambda_abi_stack: Vec<LambdaAbiContext>,
}

#[derive(Debug, Clone)]
struct FunctionSig {
    _params: Vec<WasmType>,
    result: Option<WasmType>,
}

#[derive(Debug, Clone)]
struct FunctionSourceSig {
    type_params: Vec<String>,
    params: Vec<Type>,
    result: Option<Type>,
}

#[derive(Debug, Clone)]
struct LambdaAbiContext {
    params: Vec<WasmType>,
    result: WasmType,
    source_params: Vec<Type>,
    source_result: Type,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum IterationInputKind {
    List,
    Option,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum WasmType {
    I32,
    I64,
    F32,
    F64,
}

impl Default for WasmCodeGen {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmCodeGen {
    pub fn new() -> Self {
        Self {
            locals: vec![HashMap::new()],
            local_types: vec![HashMap::new()],
            local_source_types: vec![HashMap::new()],
            local_aliases: vec![HashMap::new()],
            binding_local_aliases: HashMap::new(),
            collected_local_types: HashMap::new(),
            local_alias_counter: 0,
            generic_function_aliases: vec![HashMap::new()],
            deferred_lambda_aliases: vec![HashMap::new()],
            functions: HashMap::new(),
            function_source_sigs: HashMap::new(),
            function_decls: HashMap::new(),
            specialized_functions: HashSet::new(),
            exported_functions: HashSet::new(),
            global_types: HashMap::new(),
            global_source_types: HashMap::new(),
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
            has_indirect_closure_call: false,
            in_lambda_with_captures: false,
            captured_vars: Vec::new(),
            records: HashMap::new(),
            record_type_params: HashMap::new(),
            record_field_offsets: HashMap::new(),
            var_types: HashMap::new(),
            temporal_resources: HashMap::new(),
            temporal_scope_stack: Vec::new(),
            cleanup_functions: HashMap::new(),
            record_literal_depth: 0,
            record_pattern_depth: 0,
            record_tmp_count: RECORD_TMP_MIN_COUNT,
            with_arena_depth: 0,
            lambda_abi_stack: Vec::new(),
        }
    }

    pub fn generate(&mut self, program: &Program) -> Result<String, CodeGenError> {
        self.output.push_str("(module\n");

        // Process module imports first
        self.generate_imports(&program.imports)?;

        // Import WASI functions for I/O
        self.output.push_str("  ;; WASI imports\n");
        self.output.push_str("  (import \"wasi_snapshot_preview1\" \"fd_write\" (func $fd_write (param i32 i32 i32 i32) (result i32)))\n");
        self.output.push_str(
            "  (import \"wasi_snapshot_preview1\" \"proc_exit\" (func $proc_exit (param i32)))\n",
        );

        // Memory
        self.output.push_str("\n  ;; Memory\n");
        self.output.push_str("  (memory 1)\n");
        self.output.push_str("  (export \"memory\" (memory 0))\n");

        self.generate_indirect_call_types();

        // Note: Using direct function calls for cleanup instead of function table

        // Collect string constants first
        self.collect_strings(program)?;

        // Generate string data section
        if !self.strings.is_empty() {
            self.output.push_str("\n  ;; String constants\n");
            for (s, offset) in &self.string_offsets {
                let bytes = s.as_bytes();
                let len = bytes.len() as u32;

                // Format: 4 bytes length + string data
                self.output
                    .push_str(&format!("  (data (i32.const {}) \"", offset));

                // Write length as little-endian
                self.output.push_str(&format!(
                    "\\{:02x}\\{:02x}\\{:02x}\\{:02x}",
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

        // Generate temporal cleanup functions
        self.generate_temporal_cleanup_functions()?;

        // Collect record definitions first
        for decl in &program.declarations {
            if let TopDecl::Record(record) = Self::decl_codegen_item(decl) {
                self.register_record_definition(record)?;
            }
        }
        for decl in &program.declarations {
            if let TopDecl::Context(context) = Self::decl_codegen_item(decl) {
                self.register_context_definition(context)?;
            }
        }

        for decl in &program.declarations {
            if let TopDecl::Binding(binding) = Self::decl_codegen_item(decl) {
                self.register_global_binding(binding)?;
            }
        }
        self.collect_exported_functions(program);

        // Collect all function signatures first
        for decl in &program.declarations {
            match Self::decl_codegen_item(decl) {
                TopDecl::Function(func) => {
                    self.register_function_signature(func)?;
                }
                TopDecl::Binding(_) => {}
                TopDecl::Record(record) => {
                    self.register_record_methods(record)?;
                }
                TopDecl::Impl(impl_block) => {
                    self.register_impl_methods(impl_block)?;
                }
                TopDecl::Export(_) => {
                    // Not yet implemented
                }
                TopDecl::Context(_) => {
                    // Not yet implemented
                }
            }
        }

        self.generate_global_bindings(program)?;

        // Generate functions
        self.output.push_str("\n  ;; Functions\n");
        for decl in &program.declarations {
            match Self::decl_codegen_item(decl) {
                TopDecl::Function(func) => {
                    self.generate_function(func)?;
                }
                TopDecl::Binding(_) => {}
                TopDecl::Record(record) => {
                    self.generate_record_methods(record)?;
                }
                TopDecl::Impl(impl_block) => {
                    self.generate_impl_methods(impl_block)?;
                }
                TopDecl::Export(_) => {
                    // Not yet implemented
                }
                TopDecl::Context(_) => {
                    // Not yet implemented
                }
            }
        }

        // Generate lambda functions
        for lambda_func in &self.lambda_functions {
            self.output.push_str(lambda_func);
        }

        // Generate function table if we have indirect calls
        if self.has_indirect_closure_call || !self.function_table.is_empty() {
            self.output
                .push_str("\n  ;; Function table for indirect calls\n");
            self.output.push_str("  (table ");
            self.output
                .push_str(&self.function_table.len().max(1).to_string());
            self.output.push_str(" funcref)\n");

            // Initialize table elements
            for (i, func_name) in self.function_table.iter().enumerate() {
                self.output
                    .push_str(&format!("  (elem (i32.const {}) func ${})\n", i, func_name));
            }
        }

        // Generate module exports
        self.generate_exports(program)?;

        // Export a no-result program entry wrapper only for zero-argument
        // `main`. A parameterized function named `main` remains an ordinary
        // Restrict function with its declared ABI.
        if self.should_generate_start_wrapper() {
            self.generate_start_wrapper()?;
        }

        self.output.push_str(")\n");

        Ok(self.output.clone())
    }

    fn decl_codegen_item(decl: &TopDecl) -> &TopDecl {
        match decl {
            TopDecl::Export(export_decl) => export_decl.item.as_ref(),
            _ => decl,
        }
    }

    fn collect_exported_functions(&mut self, program: &Program) {
        for decl in &program.declarations {
            if let TopDecl::Export(export_decl) = decl {
                if let TopDecl::Function(func) = export_decl.item.as_ref() {
                    self.exported_functions.insert(func.name.clone());
                }
            }
        }
    }

    fn should_generate_start_wrapper(&self) -> bool {
        self.functions
            .get("main")
            .map(|sig| sig._params.is_empty())
            .unwrap_or(false)
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
        self.output
            .push_str("    ;; Read string length from memory (first 4 bytes)\n");
        self.output.push_str("    local.get $str\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $len\n");
        self.output.push_str("    \n");
        self.output
            .push_str("    ;; Prepare iovec structure at memory address 0\n");
        self.output
            .push_str("    ;; iov_base = str + 4 (skip length prefix)\n");
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
        self.output
            .push_str("    i32.const 8   ;; second iovec base\n");
        self.output
            .push_str("    i32.const 16  ;; address of newline\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output
            .push_str("    i32.const 12  ;; second iovec len\n");
        self.output
            .push_str("    i32.const 1   ;; length of newline\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Call fd_write\n");
        self.output.push_str("    i32.const 1   ;; stdout\n");
        self.output.push_str("    i32.const 0   ;; iovs\n");
        self.output
            .push_str("    i32.const 2   ;; iovs_len (2 iovecs)\n");
        self.output
            .push_str("    i32.const 20  ;; nwritten (output param)\n");
        self.output.push_str("    call $fd_write\n");
        self.output.push_str("    drop\n");
        self.output.push_str("  )\n");

        // print_int function with proper integer to string conversion
        self.output
            .push_str("\n  (func $print_int (param $value i32)\n");
        self.output.push_str("    (local $num i32)\n");
        self.output.push_str("    (local $digit i32)\n");
        self.output.push_str("    (local $buffer_start i32)\n");
        self.output.push_str("    (local $buffer_end i32)\n");
        self.output.push_str("    (local $is_negative i32)\n");
        self.output.push_str("    (local $len i32)\n");
        self.output.push_str("    \n");
        self.output
            .push_str("    ;; Use memory starting at address 400 for the buffer\n");
        self.output
            .push_str("    i32.const 420  ;; Start from the end of buffer and work backwards\n");
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
        self.output.push_str("        local.set $buffer_start\n");
        self.output.push_str("        local.get $buffer_start\n");
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
        self.output.push_str("          local.set $buffer_start\n");
        self.output.push_str("          local.get $buffer_start\n");
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
        self.output.push_str("        local.set $buffer_start\n");
        self.output.push_str("        local.get $buffer_start\n");
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
        self.output
            .push_str("    local.get $buffer_start  ;; iov_base\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    i32.const 204\n");
        self.output
            .push_str("    local.get $len          ;; iov_len\n");
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
        self.output
            .push_str("  (func $println_generic (param $value i32) (param $type_tag i32)\n");
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
        self.functions.insert(
            "println".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: None,
            },
        );
        self.function_source_sigs.insert(
            "println".to_string(),
            FunctionSourceSig {
                type_params: vec![],
                params: vec![Type::Named("String".to_string())],
                result: Some(Type::Named("Unit".to_string())),
            },
        );

        // Add print_int to function signatures
        self.functions.insert(
            "print_int".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: None,
            },
        );

        self.generate_std_io_functions()?;
        self.generate_std_math_functions()?;
        self.generate_std_prelude_functions()?;

        for (name, arity) in [("map", 2), ("filter", 2), ("fold", 3)] {
            self.functions.insert(
                name.to_string(),
                FunctionSig {
                    _params: vec![WasmType::I32; arity],
                    result: Some(WasmType::I32),
                },
            );
        }

        self.generate_std_option_functions();

        Ok(())
    }

    fn generate_std_io_functions(&mut self) -> Result<(), CodeGenError> {
        self.emit_string_write_function("print", 1, false);
        self.emit_string_write_function("eprint", 2, false);
        self.emit_string_write_function("eprintln", 2, true);

        self.output
            .push_str("  (func $print_float (param $value f64)\n");
        self.output.push_str("    (local $num i32)\n");
        self.output.push_str("    (local $frac i32)\n");
        self.output.push_str("    (local $digit i32)\n");
        self.output.push_str("    (local $buffer_start i32)\n");
        self.output.push_str("    (local $buffer_end i32)\n");
        self.output.push_str("    (local $is_negative i32)\n");
        self.output.push_str("    (local $len i32)\n");
        self.output.push_str("    (local $abs_value f64)\n");
        self.output.push_str("    f64.const 0\n");
        self.output.push_str("    local.get $value\n");
        self.output.push_str("    f64.gt\n");
        self.output.push_str("    local.set $is_negative\n");
        self.output.push_str("    local.get $is_negative\n");
        self.output.push_str("    (if (result f64)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        local.get $value\n");
        self.output.push_str("        f64.neg\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        local.get $value\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    local.set $abs_value\n");
        self.output.push_str("    local.get $abs_value\n");
        self.output.push_str("    i32.trunc_f64_s\n");
        self.output.push_str("    local.set $num\n");
        self.output.push_str("    local.get $abs_value\n");
        self.output.push_str("    local.get $num\n");
        self.output.push_str("    f64.convert_i32_s\n");
        self.output.push_str("    f64.sub\n");
        self.output.push_str("    f64.const 100\n");
        self.output.push_str("    f64.mul\n");
        self.output.push_str("    i32.trunc_f64_s\n");
        self.output.push_str("    local.set $frac\n");
        self.output.push_str("    i32.const 700\n");
        self.output.push_str("    local.set $buffer_end\n");
        self.output.push_str("    local.get $buffer_end\n");
        self.output.push_str("    local.set $buffer_start\n");

        for _ in 0..2 {
            self.output.push_str("    local.get $frac\n");
            self.output.push_str("    i32.const 10\n");
            self.output.push_str("    i32.rem_u\n");
            self.output.push_str("    local.set $digit\n");
            self.output.push_str("    local.get $buffer_start\n");
            self.output.push_str("    i32.const 1\n");
            self.output.push_str("    i32.sub\n");
            self.output.push_str("    local.set $buffer_start\n");
            self.output.push_str("    local.get $buffer_start\n");
            self.output.push_str("    local.get $digit\n");
            self.output.push_str("    i32.const 48\n");
            self.output.push_str("    i32.add\n");
            self.output.push_str("    i32.store8\n");
            self.output.push_str("    local.get $frac\n");
            self.output.push_str("    i32.const 10\n");
            self.output.push_str("    i32.div_u\n");
            self.output.push_str("    local.set $frac\n");
        }

        self.output.push_str("    local.get $buffer_start\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.sub\n");
        self.output.push_str("    local.set $buffer_start\n");
        self.output.push_str("    local.get $buffer_start\n");
        self.output.push_str("    i32.const 46\n");
        self.output.push_str("    i32.store8\n");
        self.output.push_str("    local.get $num\n");
        self.output.push_str("    i32.eqz\n");
        self.output.push_str("    (if\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        local.get $buffer_start\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.sub\n");
        self.output.push_str("        local.set $buffer_start\n");
        self.output.push_str("        local.get $buffer_start\n");
        self.output.push_str("        i32.const 48\n");
        self.output.push_str("        i32.store8\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output
            .push_str("        (block $print_float_digits_done\n");
        self.output
            .push_str("          (loop $print_float_digits\n");
        self.output.push_str("            local.get $num\n");
        self.output.push_str("            i32.eqz\n");
        self.output
            .push_str("            br_if $print_float_digits_done\n");
        self.output.push_str("            local.get $num\n");
        self.output.push_str("            i32.const 10\n");
        self.output.push_str("            i32.rem_u\n");
        self.output.push_str("            local.set $digit\n");
        self.output
            .push_str("            local.get $buffer_start\n");
        self.output.push_str("            i32.const 1\n");
        self.output.push_str("            i32.sub\n");
        self.output
            .push_str("            local.set $buffer_start\n");
        self.output
            .push_str("            local.get $buffer_start\n");
        self.output.push_str("            local.get $digit\n");
        self.output.push_str("            i32.const 48\n");
        self.output.push_str("            i32.add\n");
        self.output.push_str("            i32.store8\n");
        self.output.push_str("            local.get $num\n");
        self.output.push_str("            i32.const 10\n");
        self.output.push_str("            i32.div_u\n");
        self.output.push_str("            local.set $num\n");
        self.output.push_str("            br $print_float_digits\n");
        self.output.push_str("          )\n");
        self.output.push_str("        )\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    local.get $is_negative\n");
        self.output.push_str("    (if\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        local.get $buffer_start\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.sub\n");
        self.output.push_str("        local.set $buffer_start\n");
        self.output.push_str("        local.get $buffer_start\n");
        self.output.push_str("        i32.const 45\n");
        self.output.push_str("        i32.store8\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    local.get $buffer_end\n");
        self.output.push_str("    local.get $buffer_start\n");
        self.output.push_str("    i32.sub\n");
        self.output.push_str("    local.set $len\n");
        self.output.push_str("    i32.const 200\n");
        self.output.push_str("    local.get $buffer_start\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    i32.const 204\n");
        self.output.push_str("    local.get $len\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.const 200\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.const 300\n");
        self.output.push_str("    call $fd_write\n");
        self.output.push_str("    drop\n");
        self.output.push_str("  )\n");

        for (name, param_ty) in [
            ("print", Type::Named("String".to_string())),
            ("print_int", Type::Named("Int32".to_string())),
            ("print_float", Type::Named("Float64".to_string())),
            ("eprint", Type::Named("String".to_string())),
            ("eprintln", Type::Named("String".to_string())),
        ] {
            let wasm_param = self.convert_type(&param_ty)?;
            self.functions.insert(
                name.to_string(),
                FunctionSig {
                    _params: vec![wasm_param],
                    result: None,
                },
            );
            self.function_source_sigs.insert(
                name.to_string(),
                FunctionSourceSig {
                    type_params: vec![],
                    params: vec![param_ty],
                    result: Some(Type::Named("Unit".to_string())),
                },
            );
        }

        Ok(())
    }

    fn emit_string_write_function(&mut self, name: &str, fd: i32, newline: bool) {
        self.output
            .push_str(&format!("  (func ${} (param $str i32)\n", name));
        self.output.push_str("    (local $len i32)\n");
        self.output.push_str("    local.get $str\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $len\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    local.get $str\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    local.get $len\n");
        self.output.push_str("    i32.store\n");

        if newline {
            self.output.push_str("    i32.const 16\n");
            self.output.push_str("    i32.const 10\n");
            self.output.push_str("    i32.store8\n");
            self.output.push_str("    i32.const 8\n");
            self.output.push_str("    i32.const 16\n");
            self.output.push_str("    i32.store\n");
            self.output.push_str("    i32.const 12\n");
            self.output.push_str("    i32.const 1\n");
            self.output.push_str("    i32.store\n");
        }

        self.output.push_str(&format!("    i32.const {}\n", fd));
        self.output.push_str("    i32.const 0\n");
        self.output
            .push_str(&format!("    i32.const {}\n", if newline { 2 } else { 1 }));
        self.output.push_str("    i32.const 20\n");
        self.output.push_str("    call $fd_write\n");
        self.output.push_str("    drop\n");
        self.output.push_str("  )\n");
    }

    fn generate_std_math_functions(&mut self) -> Result<(), CodeGenError> {
        self.output.push_str("\n  ;; Math operation functions\n");

        self.output
            .push_str("  (func $abs (param $x i32) (result i32)\n");
        self.output.push_str("    local.get $x\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    i32.lt_s\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        i32.const 0\n");
        self.output.push_str("        local.get $x\n");
        self.output.push_str("        i32.sub\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        local.get $x\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n");

        self.output
            .push_str("  (func $max (param $a i32) (param $b i32) (result i32)\n");
        self.output.push_str("    local.get $a\n");
        self.output.push_str("    local.get $b\n");
        self.output.push_str("    i32.gt_s\n");
        self.output.push_str("    (if (result i32)\n");
        self.output
            .push_str("      (then local.get $a)\n      (else local.get $b)\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n");

        self.output
            .push_str("  (func $min (param $a i32) (param $b i32) (result i32)\n");
        self.output.push_str("    local.get $a\n");
        self.output.push_str("    local.get $b\n");
        self.output.push_str("    i32.lt_s\n");
        self.output.push_str("    (if (result i32)\n");
        self.output
            .push_str("      (then local.get $a)\n      (else local.get $b)\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n");

        self.output
            .push_str("  (func $pow (param $base i32) (param $exp i32) (result i32)\n");
        self.output.push_str("    (local $result i32)\n");
        self.output.push_str("    (local $i i32)\n");
        self.output.push_str("    local.get $exp\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    i32.lt_s\n");
        self.output.push_str("    (if (then unreachable))\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    local.set $result\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    local.set $i\n");
        self.output.push_str("    (block $pow_done\n");
        self.output.push_str("      (loop $pow_loop\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        local.get $exp\n");
        self.output.push_str("        i32.ge_s\n");
        self.output.push_str("        br_if $pow_done\n");
        self.output.push_str("        local.get $result\n");
        self.output.push_str("        local.get $base\n");
        self.output.push_str("        i32.mul\n");
        self.output.push_str("        local.set $result\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.set $i\n");
        self.output.push_str("        br $pow_loop\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    local.get $result\n");
        self.output.push_str("  )\n");

        self.output
            .push_str("  (func $factorial (param $n i32) (result i32)\n");
        self.output.push_str("    (local $result i32)\n");
        self.output.push_str("    (local $i i32)\n");
        self.output.push_str("    local.get $n\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    i32.lt_s\n");
        self.output.push_str("    (if (then unreachable))\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    local.set $result\n");
        self.output.push_str("    i32.const 2\n");
        self.output.push_str("    local.set $i\n");
        self.output.push_str("    (block $factorial_done\n");
        self.output.push_str("      (loop $factorial_loop\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        local.get $n\n");
        self.output.push_str("        i32.gt_s\n");
        self.output.push_str("        br_if $factorial_done\n");
        self.output.push_str("        local.get $result\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.mul\n");
        self.output.push_str("        local.set $result\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.set $i\n");
        self.output.push_str("        br $factorial_loop\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    local.get $result\n");
        self.output.push_str("  )\n");

        self.output
            .push_str("  (func $abs_f (param $x f64) (result f64)\n");
        self.output.push_str("    local.get $x\n");
        self.output.push_str("    f64.abs\n");
        self.output.push_str("  )\n");

        self.output
            .push_str("  (func $max_f (param $a f64) (param $b f64) (result f64)\n");
        self.output.push_str("    local.get $a\n");
        self.output.push_str("    local.get $b\n");
        self.output.push_str("    f64.max\n");
        self.output.push_str("  )\n");

        self.output
            .push_str("  (func $min_f (param $a f64) (param $b f64) (result f64)\n");
        self.output.push_str("    local.get $a\n");
        self.output.push_str("    local.get $b\n");
        self.output.push_str("    f64.min\n");
        self.output.push_str("  )\n");

        for (name, params, result) in [
            (
                "abs",
                vec![Type::Named("Int32".to_string())],
                Type::Named("Int32".to_string()),
            ),
            (
                "max",
                vec![
                    Type::Named("Int32".to_string()),
                    Type::Named("Int32".to_string()),
                ],
                Type::Named("Int32".to_string()),
            ),
            (
                "min",
                vec![
                    Type::Named("Int32".to_string()),
                    Type::Named("Int32".to_string()),
                ],
                Type::Named("Int32".to_string()),
            ),
            (
                "pow",
                vec![
                    Type::Named("Int32".to_string()),
                    Type::Named("Int32".to_string()),
                ],
                Type::Named("Int32".to_string()),
            ),
            (
                "factorial",
                vec![Type::Named("Int32".to_string())],
                Type::Named("Int32".to_string()),
            ),
            (
                "abs_f",
                vec![Type::Named("Float64".to_string())],
                Type::Named("Float64".to_string()),
            ),
            (
                "max_f",
                vec![
                    Type::Named("Float64".to_string()),
                    Type::Named("Float64".to_string()),
                ],
                Type::Named("Float64".to_string()),
            ),
            (
                "min_f",
                vec![
                    Type::Named("Float64".to_string()),
                    Type::Named("Float64".to_string()),
                ],
                Type::Named("Float64".to_string()),
            ),
        ] {
            let wasm_params = params
                .iter()
                .map(|param| self.convert_type(param))
                .collect::<Result<Vec<_>, _>>()?;
            let wasm_result = self.convert_type(&result)?;
            self.functions.insert(
                name.to_string(),
                FunctionSig {
                    _params: wasm_params,
                    result: Some(wasm_result),
                },
            );
            self.function_source_sigs.insert(
                name.to_string(),
                FunctionSourceSig {
                    type_params: vec![],
                    params,
                    result: Some(result),
                },
            );
        }

        Ok(())
    }

    fn generate_std_prelude_functions(&mut self) -> Result<(), CodeGenError> {
        self.output.push_str("\n  ;; Prelude operation functions\n");
        self.output
            .push_str("  (func $not (param $value i32) (result i32)\n");
        self.output.push_str("    local.get $value\n");
        self.output.push_str("    i32.eqz\n");
        self.output.push_str("  )\n");

        self.output
            .push_str("  (func $and (param $a i32) (param $b i32) (result i32)\n");
        self.output.push_str("    local.get $a\n");
        self.output.push_str("    local.get $b\n");
        self.output.push_str("    i32.and\n");
        self.output.push_str("  )\n");

        self.output
            .push_str("  (func $or (param $a i32) (param $b i32) (result i32)\n");
        self.output.push_str("    local.get $a\n");
        self.output.push_str("    local.get $b\n");
        self.output.push_str("    i32.or\n");
        self.output.push_str("  )\n");

        self.output
            .push_str("  (func $assert (param $condition i32) (param $message i32)\n");
        self.output.push_str("    local.get $condition\n");
        self.output.push_str("    i32.eqz\n");
        self.output.push_str("    (if\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        unreachable\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n");

        self.output
            .push_str("  (func $panic (param $message i32)\n");
        self.output.push_str("    unreachable\n");
        self.output.push_str("  )\n");

        for (name, params, result) in [
            (
                "not",
                vec![Type::Named("Boolean".to_string())],
                Some(Type::Named("Boolean".to_string())),
            ),
            (
                "and",
                vec![
                    Type::Named("Boolean".to_string()),
                    Type::Named("Boolean".to_string()),
                ],
                Some(Type::Named("Boolean".to_string())),
            ),
            (
                "or",
                vec![
                    Type::Named("Boolean".to_string()),
                    Type::Named("Boolean".to_string()),
                ],
                Some(Type::Named("Boolean".to_string())),
            ),
            (
                "assert",
                vec![
                    Type::Named("Boolean".to_string()),
                    Type::Named("String".to_string()),
                ],
                None,
            ),
            ("panic", vec![Type::Named("String".to_string())], None),
        ] {
            let wasm_params = params
                .iter()
                .map(|param| self.convert_type(param))
                .collect::<Result<Vec<_>, _>>()?;
            let wasm_result = result
                .as_ref()
                .map(|ty| self.convert_type(ty))
                .transpose()?;
            self.functions.insert(
                name.to_string(),
                FunctionSig {
                    _params: wasm_params,
                    result: wasm_result,
                },
            );
            self.function_source_sigs.insert(
                name.to_string(),
                FunctionSourceSig {
                    type_params: vec![],
                    params,
                    result: result.or_else(|| Some(Type::Named("Unit".to_string()))),
                },
            );
        }

        Ok(())
    }

    fn generate_std_option_functions(&mut self) {
        self.output.push_str("\n  ;; Option operation functions\n");
        self.output
            .push_str("  (func $option_is_some (param $option i32) (result i32)\n");
        self.output.push_str("    local.get $option\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.eq\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "option_is_some".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: Some(WasmType::I32),
            },
        );
        self.function_source_sigs.insert(
            "option_is_some".to_string(),
            FunctionSourceSig {
                type_params: vec!["T".to_string()],
                params: vec![Type::Generic(
                    "Option".to_string(),
                    vec![Type::Named("T".to_string())],
                )],
                result: Some(Type::Named("Boolean".to_string())),
            },
        );

        self.output
            .push_str("  (func $option_is_none (param $option i32) (result i32)\n");
        self.output.push_str("    local.get $option\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    i32.eq\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "option_is_none".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: Some(WasmType::I32),
            },
        );
        self.function_source_sigs.insert(
            "option_is_none".to_string(),
            FunctionSourceSig {
                type_params: vec!["T".to_string()],
                params: vec![Type::Generic(
                    "Option".to_string(),
                    vec![Type::Named("T".to_string())],
                )],
                result: Some(Type::Named("Boolean".to_string())),
            },
        );

        self.output.push_str(
            "  (func $option_unwrap_or (param $option i32) (param $default i32) (result i32)\n",
        );
        self.output.push_str("    local.get $option\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.eq\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        i32.load\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        local.get $default\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "option_unwrap_or".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I32],
                result: Some(WasmType::I32),
            },
        );
        self.function_source_sigs.insert(
            "option_unwrap_or".to_string(),
            FunctionSourceSig {
                type_params: vec!["T".to_string()],
                params: vec![
                    Type::Generic("Option".to_string(), vec![Type::Named("T".to_string())]),
                    Type::Named("T".to_string()),
                ],
                result: Some(Type::Named("T".to_string())),
            },
        );

        self.output.push_str(
            "  (func $option_unwrap_or_f64 (param $option i32) (param $default f64) (result f64)\n",
        );
        self.output.push_str("    local.get $option\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.eq\n");
        self.output.push_str("    (if (result f64)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        f64.load\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        local.get $default\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "option_unwrap_or_f64".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::F64],
                result: Some(WasmType::F64),
            },
        );
    }

    fn generate_indirect_call_types(&mut self) {
        self.output
            .push_str("\n  ;; Indirect closure call signatures\n");
        for arg_count in 0..=4 {
            self.output
                .push_str(&format!("  (type $closure_call_{} (func", arg_count));
            for _ in 0..arg_count {
                self.output.push_str(" (param i32)");
            }
            self.output.push_str(" (param i32) (result i32)))\n");
        }

        for arg_count in 0..=4 {
            self.generate_typed_indirect_call_types(arg_count);
        }
    }

    fn generate_typed_indirect_call_types(&mut self, arg_count: usize) {
        let supported_types = [WasmType::I32, WasmType::F64, WasmType::I64];
        let combinations = supported_types.len().pow(arg_count as u32);
        for mut encoded in 0..combinations {
            let mut arg_types = Vec::with_capacity(arg_count);
            for _ in 0..arg_count {
                let idx = encoded % supported_types.len();
                arg_types.push(supported_types[idx]);
                encoded /= supported_types.len();
            }

            for result in supported_types {
                if arg_types.iter().all(|ty| *ty == WasmType::I32) && result == WasmType::I32 {
                    continue;
                }

                let type_name = self.closure_call_type_name(&arg_types, result);
                self.output
                    .push_str(&format!("  (type ${} (func", type_name));
                for arg_ty in &arg_types {
                    self.output
                        .push_str(&format!(" (param {})", self.wasm_type_str(*arg_ty)));
                }
                self.output.push_str(" (param i32)");
                self.output
                    .push_str(&format!(" (result {})))\n", self.wasm_type_str(result)));
            }
        }
    }

    fn generate_arena_functions(&mut self) -> Result<(), CodeGenError> {
        self.output.push_str("\n  ;; Arena allocator functions\n");

        // Global variable to track current arena
        self.output
            .push_str("  (global $current_arena (mut i32) (i32.const 0))\n\n");

        // Arena init function
        self.output
            .push_str("  (func $arena_init (param $start i32) (result i32)\n");
        self.output.push_str("    ;; Initialize arena header\n");
        self.output
            .push_str("    ;; Store start address at offset 0\n");
        self.output.push_str("    local.get $start\n");
        self.output.push_str("    local.get $start\n");
        self.output.push_str("    i32.store\n");
        self.output
            .push_str("    ;; Store current address at offset 4 (start + 8 for header)\n");
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
        self.functions.insert(
            "arena_init".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: Some(WasmType::I32),
            },
        );

        // Arena alloc function
        self.output
            .push_str("  (func $arena_alloc (param $arena i32) (param $size i32) (result i32)\n");
        self.output.push_str("    (local $current i32)\n");
        self.output.push_str("    (local $aligned_size i32)\n");
        self.output.push_str("    (local $new_current i32)\n");
        self.output.push_str("    (local $arena_end i32)\n");
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
        self.output.push_str("    ;; Arena bounds check\n");
        self.output.push_str("    local.get $arena\n");
        self.output
            .push_str(&format!("    i32.const {}\n", ARENA_SIZE_BYTES));
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.set $arena_end\n");
        self.output.push_str("    local.get $new_current\n");
        self.output.push_str("    local.get $arena_end\n");
        self.output.push_str("    i32.gt_u\n");
        self.output.push_str("    (if\n");
        self.output.push_str("      (then\n");
        self.output
            .push_str("        ;; Arena allocation overflow - trap\n");
        self.output.push_str("        unreachable\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
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

        self.functions.insert(
            "arena_alloc".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I32],
                result: Some(WasmType::I32),
            },
        );

        // Arena reset function
        self.output
            .push_str("  (func $arena_reset (param $arena i32)\n");
        self.output
            .push_str("    ;; Reset current to start + 8 (after header)\n");
        self.output.push_str("    local.get $arena\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $arena\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "arena_reset".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: None,
            },
        );

        // Allocate function (uses current arena)
        self.output
            .push_str("  (func $allocate (param $size i32) (result i32)\n");
        self.output
            .push_str("    ;; Use current arena or fail if none\n");
        self.output.push_str("    global.get $current_arena\n");
        self.output.push_str("    i32.eqz\n");
        self.output.push_str("    (if\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        unreachable\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    global.get $current_arena\n");
        self.output.push_str("    local.get $size\n");
        self.output.push_str("    call $arena_alloc\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "allocate".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: Some(WasmType::I32),
            },
        );

        self.generate_string_concat_function();
        self.generate_string_eq_function();

        Ok(())
    }

    fn generate_string_concat_function(&mut self) {
        self.output
            .push_str("\n  ;; String concatenation function\n");
        self.output
            .push_str("  (func $string_concat (param $left i32) (param $right i32) (result i32)\n");
        self.output.push_str("    (local $left_len i32)\n");
        self.output.push_str("    (local $right_len i32)\n");
        self.output.push_str("    (local $total_len i32)\n");
        self.output.push_str("    (local $out i32)\n");
        self.output.push_str("    (local $i i32)\n");
        self.output.push_str("    local.get $left\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $left_len\n");
        self.output.push_str("    local.get $right\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $right_len\n");
        self.output.push_str("    local.get $left_len\n");
        self.output.push_str("    local.get $right_len\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.set $total_len\n");
        self.output.push_str("    local.get $total_len\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $out\n");
        self.output.push_str("    local.get $out\n");
        self.output.push_str("    local.get $total_len\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    local.set $i\n");
        self.output.push_str("    (block $copy_left_done\n");
        self.output.push_str("      (loop $copy_left\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        local.get $left_len\n");
        self.output.push_str("        i32.ge_u\n");
        self.output.push_str("        br_if $copy_left_done\n");
        self.output.push_str("        local.get $out\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $left\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        i32.load8_u\n");
        self.output.push_str("        i32.store8\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.set $i\n");
        self.output.push_str("        br $copy_left\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    local.set $i\n");
        self.output.push_str("    (block $copy_right_done\n");
        self.output.push_str("      (loop $copy_right\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        local.get $right_len\n");
        self.output.push_str("        i32.ge_u\n");
        self.output.push_str("        br_if $copy_right_done\n");
        self.output.push_str("        local.get $out\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $left_len\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $right\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        i32.load8_u\n");
        self.output.push_str("        i32.store8\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.set $i\n");
        self.output.push_str("        br $copy_right\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    local.get $out\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "string_concat".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I32],
                result: Some(WasmType::I32),
            },
        );
        self.function_source_sigs.insert(
            "string_concat".to_string(),
            FunctionSourceSig {
                type_params: vec![],
                params: vec![
                    Type::Named("String".to_string()),
                    Type::Named("String".to_string()),
                ],
                result: Some(Type::Named("String".to_string())),
            },
        );
    }

    fn generate_string_eq_function(&mut self) {
        self.output.push_str("\n  ;; String equality function\n");
        self.output
            .push_str("  (func $string_eq (param $left i32) (param $right i32) (result i32)\n");
        self.output.push_str("    (local $left_len i32)\n");
        self.output.push_str("    (local $right_len i32)\n");
        self.output.push_str("    (local $i i32)\n");
        self.output.push_str("    (local $matched i32)\n");
        self.output.push_str("    local.get $left\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $left_len\n");
        self.output.push_str("    local.get $right\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $right_len\n");
        self.output.push_str("    local.get $left_len\n");
        self.output.push_str("    local.get $right_len\n");
        self.output.push_str("    i32.ne\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        i32.const 0\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        i32.const 0\n");
        self.output.push_str("        local.set $i\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        local.set $matched\n");
        self.output.push_str("        (block $string_eq_done\n");
        self.output.push_str("          (loop $string_eq_loop\n");
        self.output.push_str("            local.get $i\n");
        self.output.push_str("            local.get $left_len\n");
        self.output.push_str("            i32.ge_u\n");
        self.output.push_str("            br_if $string_eq_done\n");
        self.output.push_str("            local.get $left\n");
        self.output.push_str("            i32.const 4\n");
        self.output.push_str("            i32.add\n");
        self.output.push_str("            local.get $i\n");
        self.output.push_str("            i32.add\n");
        self.output.push_str("            i32.load8_u\n");
        self.output.push_str("            local.get $right\n");
        self.output.push_str("            i32.const 4\n");
        self.output.push_str("            i32.add\n");
        self.output.push_str("            local.get $i\n");
        self.output.push_str("            i32.add\n");
        self.output.push_str("            i32.load8_u\n");
        self.output.push_str("            i32.ne\n");
        self.output.push_str("            (if\n");
        self.output.push_str("              (then\n");
        self.output.push_str("                i32.const 0\n");
        self.output.push_str("                local.set $matched\n");
        self.output.push_str("                br $string_eq_done\n");
        self.output.push_str("              )\n");
        self.output.push_str("            )\n");
        self.output.push_str("            local.get $i\n");
        self.output.push_str("            i32.const 1\n");
        self.output.push_str("            i32.add\n");
        self.output.push_str("            local.set $i\n");
        self.output.push_str("            br $string_eq_loop\n");
        self.output.push_str("          )\n");
        self.output.push_str("        )\n");
        self.output.push_str("        local.get $matched\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "string_eq".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I32],
                result: Some(WasmType::I32),
            },
        );
        self.function_source_sigs.insert(
            "string_eq".to_string(),
            FunctionSourceSig {
                type_params: vec![],
                params: vec![
                    Type::Named("String".to_string()),
                    Type::Named("String".to_string()),
                ],
                result: Some(Type::Named("Boolean".to_string())),
            },
        );
    }

    fn generate_list_functions(&mut self) -> Result<(), CodeGenError> {
        self.output.push_str("\n  ;; List operation functions\n");

        // List length function
        self.output
            .push_str("  (func $list_length (param $list i32) (result i32)\n");
        self.output
            .push_str("    ;; Load length from list header (offset 0)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_length".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: Some(WasmType::I32),
            },
        );
        self.function_source_sigs.insert(
            "list_length".to_string(),
            FunctionSourceSig {
                type_params: vec!["T".to_string()],
                params: vec![Type::Generic(
                    "List".to_string(),
                    vec![Type::Named("T".to_string())],
                )],
                result: Some(Type::Named("Int32".to_string())),
            },
        );

        // List get function
        self.output
            .push_str("  (func $list_get (param $list i32) (param $index i32) (result i32)\n");
        self.output.push_str("    (local $length i32)\n");
        self.output
            .push_str("    ;; Load length for bounds check\n");
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
        self.output
            .push_str("        ;; Index out of bounds - trap\n");
        self.output.push_str("        unreachable\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    \n");
        self.output
            .push_str("    ;; Calculate element address: list + 8 + (index * 4)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_get".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I32],
                result: Some(WasmType::I32),
            },
        );
        self.function_source_sigs.insert(
            "list_get".to_string(),
            FunctionSourceSig {
                type_params: vec!["T".to_string()],
                params: vec![
                    Type::Generic("List".to_string(), vec![Type::Named("T".to_string())]),
                    Type::Named("Int32".to_string()),
                ],
                result: Some(Type::Named("T".to_string())),
            },
        );

        // Float64-specialized list get. The public source function remains
        // `list_get`; codegen selects this ABI helper from the list element type.
        self.output
            .push_str("  (func $list_get_f64 (param $list i32) (param $index i32) (result f64)\n");
        self.output.push_str("    (local $length i32)\n");
        self.output
            .push_str("    ;; Load length for bounds check\n");
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
        self.output
            .push_str("        ;; Index out of bounds - trap\n");
        self.output.push_str("        unreachable\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    \n");
        self.output
            .push_str("    ;; Calculate element address: list + 8 + (index * 8)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    f64.load\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_get_f64".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I32],
                result: Some(WasmType::F64),
            },
        );

        self.output
            .push_str("  (func $list_get_i64 (param $list i32) (param $index i32) (result i64)\n");
        self.output.push_str("    (local $length i32)\n");
        self.output
            .push_str("    ;; Load length for bounds check\n");
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
        self.output
            .push_str("        ;; Index out of bounds - trap\n");
        self.output.push_str("        unreachable\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    \n");
        self.output
            .push_str("    ;; Calculate element address: list + 8 + (index * 8)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i64.load\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_get_i64".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I32],
                result: Some(WasmType::I64),
            },
        );

        // list_count<T> is the public stdlib name for list length.
        self.output
            .push_str("  (func $list_count (param $list i32) (result i32)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    call $list_length\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_count".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: Some(WasmType::I32),
            },
        );
        self.function_source_sigs.insert(
            "list_count".to_string(),
            FunctionSourceSig {
                type_params: vec!["T".to_string()],
                params: vec![Type::Generic(
                    "List".to_string(),
                    vec![Type::Named("T".to_string())],
                )],
                result: Some(Type::Named("Int32".to_string())),
            },
        );

        // list_is_empty<T>
        self.output
            .push_str("  (func $list_is_empty (param $list i32) (result i32)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    call $list_length\n");
        self.output.push_str("    i32.eqz\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_is_empty".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: Some(WasmType::I32),
            },
        );
        self.function_source_sigs.insert(
            "list_is_empty".to_string(),
            FunctionSourceSig {
                type_params: vec!["T".to_string()],
                params: vec![Type::Generic(
                    "List".to_string(),
                    vec![Type::Named("T".to_string())],
                )],
                result: Some(Type::Named("Boolean".to_string())),
            },
        );

        // list_head<T> for 4-byte ABI values.
        self.output
            .push_str("  (func $list_head (param $list i32) (result i32)\n");
        self.output.push_str("    (local $option i32)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    i32.eqz\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        call $allocate\n");
        self.output.push_str("        local.set $option\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("        i32.const 0\n");
        self.output.push_str("        i32.store\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        call $allocate\n");
        self.output.push_str("        local.set $option\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.store\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $list\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        i32.load\n");
        self.output.push_str("        i32.store\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_head".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: Some(WasmType::I32),
            },
        );
        self.function_source_sigs.insert(
            "list_head".to_string(),
            FunctionSourceSig {
                type_params: vec!["T".to_string()],
                params: vec![Type::Generic(
                    "List".to_string(),
                    vec![Type::Named("T".to_string())],
                )],
                result: Some(Type::Generic(
                    "Option".to_string(),
                    vec![Type::Named("T".to_string())],
                )),
            },
        );

        self.output
            .push_str("  (func $list_head_f64 (param $list i32) (result i32)\n");
        self.output.push_str("    (local $option i32)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    i32.eqz\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        call $allocate\n");
        self.output.push_str("        local.set $option\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("        i32.const 0\n");
        self.output.push_str("        i32.store\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        i32.const 12\n");
        self.output.push_str("        call $allocate\n");
        self.output.push_str("        local.set $option\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.store\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $list\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        f64.load\n");
        self.output.push_str("        f64.store\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_head_f64".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: Some(WasmType::I32),
            },
        );

        self.output
            .push_str("  (func $list_head_i64 (param $list i32) (result i32)\n");
        self.output.push_str("    (local $option i32)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    i32.eqz\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        call $allocate\n");
        self.output.push_str("        local.set $option\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("        i32.const 0\n");
        self.output.push_str("        i32.store\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        i32.const 12\n");
        self.output.push_str("        call $allocate\n");
        self.output.push_str("        local.set $option\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.store\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $list\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        i64.load\n");
        self.output.push_str("        i64.store\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_head_i64".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: Some(WasmType::I32),
            },
        );

        // list_tail<T> for 4-byte ABI values.
        self.output
            .push_str("  (func $list_tail (param $list i32) (result i32)\n");
        self.output.push_str("    (local $option i32)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    i32.eqz\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        call $allocate\n");
        self.output.push_str("        local.set $option\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("        i32.const 0\n");
        self.output.push_str("        i32.store\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        call $allocate\n");
        self.output.push_str("        local.set $option\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.store\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $list\n");
        self.output.push_str("        call $tail\n");
        self.output.push_str("        i32.store\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_tail".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: Some(WasmType::I32),
            },
        );
        self.function_source_sigs.insert(
            "list_tail".to_string(),
            FunctionSourceSig {
                type_params: vec!["T".to_string()],
                params: vec![Type::Generic(
                    "List".to_string(),
                    vec![Type::Named("T".to_string())],
                )],
                result: Some(Type::Generic(
                    "Option".to_string(),
                    vec![Type::Generic(
                        "List".to_string(),
                        vec![Type::Named("T".to_string())],
                    )],
                )),
            },
        );

        self.output
            .push_str("  (func $list_tail_f64 (param $list i32) (result i32)\n");
        self.output.push_str("    (local $option i32)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    i32.eqz\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        call $allocate\n");
        self.output.push_str("        local.set $option\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("        i32.const 0\n");
        self.output.push_str("        i32.store\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        call $allocate\n");
        self.output.push_str("        local.set $option\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.store\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $list\n");
        self.output.push_str("        call $tail_f64\n");
        self.output.push_str("        i32.store\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_tail_f64".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: Some(WasmType::I32),
            },
        );

        self.output
            .push_str("  (func $list_tail_i64 (param $list i32) (result i32)\n");
        self.output.push_str("    (local $option i32)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    i32.eqz\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        call $allocate\n");
        self.output.push_str("        local.set $option\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("        i32.const 0\n");
        self.output.push_str("        i32.store\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        call $allocate\n");
        self.output.push_str("        local.set $option\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.store\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $list\n");
        self.output.push_str("        call $tail_i64\n");
        self.output.push_str("        i32.store\n");
        self.output.push_str("        local.get $option\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_tail_i64".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: Some(WasmType::I32),
            },
        );

        // list_reverse<T> for 4-byte ABI values.
        self.output
            .push_str("  (func $list_reverse (param $list i32) (result i32)\n");
        self.output.push_str("    (local $length i32)\n");
        self.output.push_str("    (local $out i32)\n");
        self.output.push_str("    (local $i i32)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $length\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $out\n");
        self.output.push_str("    local.get $out\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $out\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    local.set $i\n");
        self.output.push_str("    (block $list_reverse_done\n");
        self.output.push_str("      (loop $list_reverse_loop\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        local.get $length\n");
        self.output.push_str("        i32.ge_u\n");
        self.output.push_str("        br_if $list_reverse_done\n");
        self.output.push_str("        local.get $out\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.mul\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $list\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $length\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.sub\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.sub\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.mul\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        i32.load\n");
        self.output.push_str("        i32.store\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.set $i\n");
        self.output.push_str("        br $list_reverse_loop\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    local.get $out\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_reverse".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: Some(WasmType::I32),
            },
        );
        self.function_source_sigs.insert(
            "list_reverse".to_string(),
            FunctionSourceSig {
                type_params: vec!["T".to_string()],
                params: vec![Type::Generic(
                    "List".to_string(),
                    vec![Type::Named("T".to_string())],
                )],
                result: Some(Type::Generic(
                    "List".to_string(),
                    vec![Type::Named("T".to_string())],
                )),
            },
        );

        self.output
            .push_str("  (func $list_reverse_f64 (param $list i32) (result i32)\n");
        self.output.push_str("    (local $length i32)\n");
        self.output.push_str("    (local $out i32)\n");
        self.output.push_str("    (local $i i32)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $length\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $out\n");
        self.output.push_str("    local.get $out\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $out\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    local.set $i\n");
        self.output.push_str("    (block $list_reverse_f64_done\n");
        self.output.push_str("      (loop $list_reverse_f64_loop\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        local.get $length\n");
        self.output.push_str("        i32.ge_u\n");
        self.output
            .push_str("        br_if $list_reverse_f64_done\n");
        self.output.push_str("        local.get $out\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        i32.mul\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $list\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $length\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.sub\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.sub\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        i32.mul\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        f64.load\n");
        self.output.push_str("        f64.store\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.set $i\n");
        self.output.push_str("        br $list_reverse_f64_loop\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    local.get $out\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_reverse_f64".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: Some(WasmType::I32),
            },
        );

        self.output
            .push_str("  (func $list_reverse_i64 (param $list i32) (result i32)\n");
        self.output.push_str("    (local $length i32)\n");
        self.output.push_str("    (local $out i32)\n");
        self.output.push_str("    (local $i i32)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $length\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $out\n");
        self.output.push_str("    local.get $out\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $out\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    local.set $i\n");
        self.output.push_str("    (block $list_reverse_i64_done\n");
        self.output.push_str("      (loop $list_reverse_i64_loop\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        local.get $length\n");
        self.output.push_str("        i32.ge_u\n");
        self.output
            .push_str("        br_if $list_reverse_i64_done\n");
        self.output.push_str("        local.get $out\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        i32.mul\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $list\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.get $length\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.sub\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.sub\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        i32.mul\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        i64.load\n");
        self.output.push_str("        i64.store\n");
        self.output.push_str("        local.get $i\n");
        self.output.push_str("        i32.const 1\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str("        local.set $i\n");
        self.output.push_str("        br $list_reverse_i64_loop\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    local.get $out\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_reverse_i64".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: Some(WasmType::I32),
            },
        );

        // list_append<T> for 4-byte ABI values (Int32, Boolean, Char, pointers).
        self.output
            .push_str("  (func $list_append (param $list i32) (param $item i32) (result i32)\n");
        self.output.push_str("    (local $length i32)\n");
        self.output.push_str("    (local $new_length i32)\n");
        self.output.push_str("    (local $new_list i32)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $length\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.set $new_length\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $new_list\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
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
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $item\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_append".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I32],
                result: Some(WasmType::I32),
            },
        );
        self.function_source_sigs.insert(
            "list_append".to_string(),
            FunctionSourceSig {
                type_params: vec!["T".to_string()],
                params: vec![
                    Type::Generic("List".to_string(), vec![Type::Named("T".to_string())]),
                    Type::Named("T".to_string()),
                ],
                result: Some(Type::Generic(
                    "List".to_string(),
                    vec![Type::Named("T".to_string())],
                )),
            },
        );

        // list_prepend<T> for 4-byte ABI values.
        self.output
            .push_str("  (func $list_prepend (param $item i32) (param $list i32) (result i32)\n");
        self.output.push_str("    (local $length i32)\n");
        self.output.push_str("    (local $new_length i32)\n");
        self.output.push_str("    (local $new_list i32)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $length\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.set $new_length\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $new_list\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $item\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 12\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    memory.copy\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_prepend".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I32],
                result: Some(WasmType::I32),
            },
        );
        self.function_source_sigs.insert(
            "list_prepend".to_string(),
            FunctionSourceSig {
                type_params: vec!["T".to_string()],
                params: vec![
                    Type::Named("T".to_string()),
                    Type::Generic("List".to_string(), vec![Type::Named("T".to_string())]),
                ],
                result: Some(Type::Generic(
                    "List".to_string(),
                    vec![Type::Named("T".to_string())],
                )),
            },
        );

        // list_concat<T> for 4-byte ABI values.
        self.output
            .push_str("  (func $list_concat (param $left i32) (param $right i32) (result i32)\n");
        self.output.push_str("    (local $left_length i32)\n");
        self.output.push_str("    (local $right_length i32)\n");
        self.output.push_str("    (local $new_length i32)\n");
        self.output.push_str("    (local $new_list i32)\n");
        self.output.push_str("    local.get $left\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $left_length\n");
        self.output.push_str("    local.get $right\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $right_length\n");
        self.output.push_str("    local.get $left_length\n");
        self.output.push_str("    local.get $right_length\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.set $new_length\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $new_list\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $left\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $left_length\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    memory.copy\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $left_length\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $right\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $right_length\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    memory.copy\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_concat".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I32],
                result: Some(WasmType::I32),
            },
        );
        self.function_source_sigs.insert(
            "list_concat".to_string(),
            FunctionSourceSig {
                type_params: vec!["T".to_string()],
                params: vec![
                    Type::Generic("List".to_string(), vec![Type::Named("T".to_string())]),
                    Type::Generic("List".to_string(), vec![Type::Named("T".to_string())]),
                ],
                result: Some(Type::Generic(
                    "List".to_string(),
                    vec![Type::Named("T".to_string())],
                )),
            },
        );

        // Float64-specialized list update helpers. Source calls still use the
        // generic stdlib names; codegen selects these ABI helpers from the list
        // element type.
        self.output.push_str(
            "  (func $list_append_f64 (param $list i32) (param $item f64) (result i32)\n",
        );
        self.output.push_str("    (local $length i32)\n");
        self.output.push_str("    (local $new_length i32)\n");
        self.output.push_str("    (local $new_list i32)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $length\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.set $new_length\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $new_list\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    memory.copy\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $item\n");
        self.output.push_str("    f64.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_append_f64".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::F64],
                result: Some(WasmType::I32),
            },
        );

        self.output.push_str(
            "  (func $list_append_i64 (param $list i32) (param $item i64) (result i32)\n",
        );
        self.output.push_str("    (local $length i32)\n");
        self.output.push_str("    (local $new_length i32)\n");
        self.output.push_str("    (local $new_list i32)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $length\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.set $new_length\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $new_list\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    memory.copy\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $item\n");
        self.output.push_str("    i64.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_append_i64".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I64],
                result: Some(WasmType::I32),
            },
        );
        self.function_source_sigs.insert(
            "list_append_i64".to_string(),
            FunctionSourceSig {
                type_params: vec![],
                params: vec![
                    Type::Generic("List".to_string(), vec![Type::Named("Int64".to_string())]),
                    Type::Named("Int64".to_string()),
                ],
                result: Some(Type::Generic(
                    "List".to_string(),
                    vec![Type::Named("Int64".to_string())],
                )),
            },
        );

        self.output.push_str(
            "  (func $list_prepend_f64 (param $item f64) (param $list i32) (result i32)\n",
        );
        self.output.push_str("    (local $length i32)\n");
        self.output.push_str("    (local $new_length i32)\n");
        self.output.push_str("    (local $new_list i32)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $length\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.set $new_length\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $new_list\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $item\n");
        self.output.push_str("    f64.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 16\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    memory.copy\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_prepend_f64".to_string(),
            FunctionSig {
                _params: vec![WasmType::F64, WasmType::I32],
                result: Some(WasmType::I32),
            },
        );

        self.output.push_str(
            "  (func $list_prepend_i64 (param $item i64) (param $list i32) (result i32)\n",
        );
        self.output.push_str("    (local $length i32)\n");
        self.output.push_str("    (local $new_length i32)\n");
        self.output.push_str("    (local $new_list i32)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $length\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 1\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.set $new_length\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $new_list\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $item\n");
        self.output.push_str("    i64.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 16\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    memory.copy\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_prepend_i64".to_string(),
            FunctionSig {
                _params: vec![WasmType::I64, WasmType::I32],
                result: Some(WasmType::I32),
            },
        );
        self.function_source_sigs.insert(
            "list_prepend_i64".to_string(),
            FunctionSourceSig {
                type_params: vec![],
                params: vec![
                    Type::Named("Int64".to_string()),
                    Type::Generic("List".to_string(), vec![Type::Named("Int64".to_string())]),
                ],
                result: Some(Type::Generic(
                    "List".to_string(),
                    vec![Type::Named("Int64".to_string())],
                )),
            },
        );

        self.output.push_str(
            "  (func $list_concat_f64 (param $left i32) (param $right i32) (result i32)\n",
        );
        self.output.push_str("    (local $left_length i32)\n");
        self.output.push_str("    (local $right_length i32)\n");
        self.output.push_str("    (local $new_length i32)\n");
        self.output.push_str("    (local $new_list i32)\n");
        self.output.push_str("    local.get $left\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $left_length\n");
        self.output.push_str("    local.get $right\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $right_length\n");
        self.output.push_str("    local.get $left_length\n");
        self.output.push_str("    local.get $right_length\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.set $new_length\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $new_list\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $left\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $left_length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    memory.copy\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $left_length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $right\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $right_length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    memory.copy\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_concat_f64".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I32],
                result: Some(WasmType::I32),
            },
        );

        self.output.push_str(
            "  (func $list_concat_i64 (param $left i32) (param $right i32) (result i32)\n",
        );
        self.output.push_str("    (local $left_length i32)\n");
        self.output.push_str("    (local $right_length i32)\n");
        self.output.push_str("    (local $new_length i32)\n");
        self.output.push_str("    (local $new_list i32)\n");
        self.output.push_str("    local.get $left\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $left_length\n");
        self.output.push_str("    local.get $right\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $right_length\n");
        self.output.push_str("    local.get $left_length\n");
        self.output.push_str("    local.get $right_length\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.set $new_length\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $new_list\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $left\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $left_length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    memory.copy\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $left_length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $right\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $right_length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    memory.copy\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "list_concat_i64".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I32],
                result: Some(WasmType::I32),
            },
        );

        // Tail function
        self.output
            .push_str("  (func $tail (param $list i32) (result i32)\n");
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
        self.output
            .push_str("        ;; Return the same empty list\n");
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
        self.output
            .push_str("    ;; Allocate new list: 8 bytes header + (new_length * 4) bytes data\n");
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
        self.output
            .push_str("    ;; Write new capacity (same as length)\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output
            .push_str("    ;; Copy elements from original list (skip first element)\n");
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

        self.functions.insert(
            "tail".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: Some(WasmType::I32),
            },
        );
        self.function_source_sigs.insert(
            "tail".to_string(),
            FunctionSourceSig {
                type_params: vec!["T".to_string()],
                params: vec![Type::Generic(
                    "List".to_string(),
                    vec![Type::Named("T".to_string())],
                )],
                result: Some(Type::Generic(
                    "List".to_string(),
                    vec![Type::Named("T".to_string())],
                )),
            },
        );

        self.output
            .push_str("  (func $tail_f64 (param $list i32) (result i32)\n");
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
        self.output
            .push_str("        ;; Return the same empty list\n");
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
        self.output
            .push_str("    ;; Allocate new list: 8 bytes header + (new_length * 8) bytes data\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.const 8\n");
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
        self.output
            .push_str("    ;; Write new capacity (same as length)\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output
            .push_str("    ;; Copy elements from original Float64 list (skip first element)\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    ;; destination\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.const 16\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    ;; source\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    ;; size\n");
        self.output.push_str("    memory.copy\n");
        self.output.push_str("    \n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "tail_f64".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: Some(WasmType::I32),
            },
        );

        self.output
            .push_str("  (func $tail_i64 (param $list i32) (result i32)\n");
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
        self.output
            .push_str("        ;; Return the same empty list\n");
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
        self.output
            .push_str("    ;; Allocate new list: 8 bytes header + (new_length * 8) bytes data\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.const 8\n");
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
        self.output
            .push_str("    ;; Write new capacity (same as length)\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output
            .push_str("    ;; Copy elements from original Int64 list (skip first element)\n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    ;; destination\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.const 16\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    ;; source\n");
        self.output.push_str("    local.get $new_length\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    ;; size\n");
        self.output.push_str("    memory.copy\n");
        self.output.push_str("    \n");
        self.output.push_str("    local.get $new_list\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "tail_i64".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: Some(WasmType::I32),
            },
        );

        Ok(())
    }

    fn generate_array_functions(&mut self) -> Result<(), CodeGenError> {
        self.output.push_str("\n  ;; Array operation functions\n");

        self.output
            .push_str("  (func $array_bounds_check (param $array i32) (param $index i32)\n");
        self.output
            .push_str("    ;; Array bounds check: index >= length traps\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    local.get $array\n");
        self.output.push_str("    i32.load ;; length\n");
        self.output.push_str("    i32.ge_u\n");
        self.output.push_str("    (if\n");
        self.output.push_str("      (then\n");
        self.output
            .push_str("        ;; Array index out of bounds - trap\n");
        self.output.push_str("        unreachable\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("  )\n");

        // Array get function
        self.output
            .push_str("  (func $array_get (param $array i32) (param $index i32) (result i32)\n");
        self.output
            .push_str("    ;; Bounds check before reading an Int32-sized element\n");
        self.output.push_str("    local.get $array\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    call $array_bounds_check\n");
        self.output
            .push_str("    ;; Calculate element address: array + 8 + (index * 4)\n");
        self.output.push_str("    local.get $array\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "array_get".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I32],
                result: Some(WasmType::I32),
            },
        );
        self.function_source_sigs.insert(
            "array_get".to_string(),
            FunctionSourceSig {
                type_params: vec!["T".to_string()],
                params: vec![
                    Type::Generic("Array".to_string(), vec![Type::Named("T".to_string())]),
                    Type::Named("Int32".to_string()),
                ],
                result: Some(Type::Named("T".to_string())),
            },
        );

        // Float64-specialized array get. Public source calls still use `array_get`.
        self.output.push_str(
            "  (func $array_get_f64 (param $array i32) (param $index i32) (result f64)\n",
        );
        self.output
            .push_str("    ;; Bounds check before reading a Float64 element\n");
        self.output.push_str("    local.get $array\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    call $array_bounds_check\n");
        self.output
            .push_str("    ;; Calculate element address: array + 8 + (index * 8)\n");
        self.output.push_str("    local.get $array\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    f64.load\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "array_get_f64".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I32],
                result: Some(WasmType::F64),
            },
        );

        // Int64-specialized array get. Public source calls still use `array_get`.
        self.output.push_str(
            "  (func $array_get_i64 (param $array i32) (param $index i32) (result i64)\n",
        );
        self.output
            .push_str("    ;; Bounds check before reading an Int64 element\n");
        self.output.push_str("    local.get $array\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    call $array_bounds_check\n");
        self.output
            .push_str("    ;; Calculate element address: array + 8 + (index * 8)\n");
        self.output.push_str("    local.get $array\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i64.load\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "array_get_i64".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I32],
                result: Some(WasmType::I64),
            },
        );

        // Array set function
        self.output.push_str(
            "  (func $array_set (param $array i32) (param $index i32) (param $value i32)\n",
        );
        self.output
            .push_str("    ;; Bounds check before writing an Int32-sized element\n");
        self.output.push_str("    local.get $array\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    call $array_bounds_check\n");
        self.output
            .push_str("    ;; Calculate element address: array + 8 + (index * 4)\n");
        self.output.push_str("    local.get $array\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $value\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "array_set".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I32, WasmType::I32],
                result: None,
            },
        );
        self.function_source_sigs.insert(
            "array_set".to_string(),
            FunctionSourceSig {
                type_params: vec!["T".to_string()],
                params: vec![
                    Type::Generic("Array".to_string(), vec![Type::Named("T".to_string())]),
                    Type::Named("Int32".to_string()),
                    Type::Named("T".to_string()),
                ],
                result: Some(Type::Named("Unit".to_string())),
            },
        );

        // Float64-specialized array set. Public source calls still use `array_set`.
        self.output.push_str(
            "  (func $array_set_f64 (param $array i32) (param $index i32) (param $value f64)\n",
        );
        self.output
            .push_str("    ;; Bounds check before writing a Float64 element\n");
        self.output.push_str("    local.get $array\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    call $array_bounds_check\n");
        self.output
            .push_str("    ;; Calculate element address: array + 8 + (index * 8)\n");
        self.output.push_str("    local.get $array\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $value\n");
        self.output.push_str("    f64.store\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "array_set_f64".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I32, WasmType::F64],
                result: None,
            },
        );
        self.function_source_sigs.insert(
            "array_set_f64".to_string(),
            FunctionSourceSig {
                type_params: vec![],
                params: vec![
                    Type::Generic(
                        "Array".to_string(),
                        vec![Type::Named("Float64".to_string())],
                    ),
                    Type::Named("Int32".to_string()),
                    Type::Named("Float64".to_string()),
                ],
                result: Some(Type::Named("Unit".to_string())),
            },
        );

        // Int64-specialized array set. Public source calls still use `array_set`.
        self.output.push_str(
            "  (func $array_set_i64 (param $array i32) (param $index i32) (param $value i64)\n",
        );
        self.output
            .push_str("    ;; Bounds check before writing an Int64 element\n");
        self.output.push_str("    local.get $array\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    call $array_bounds_check\n");
        self.output
            .push_str("    ;; Calculate element address: array + 8 + (index * 8)\n");
        self.output.push_str("    local.get $array\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $value\n");
        self.output.push_str("    i64.store\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "array_set_i64".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I32, WasmType::I64],
                result: None,
            },
        );
        self.function_source_sigs.insert(
            "array_set_i64".to_string(),
            FunctionSourceSig {
                type_params: vec![],
                params: vec![
                    Type::Generic("Array".to_string(), vec![Type::Named("Int64".to_string())]),
                    Type::Named("Int32".to_string()),
                    Type::Named("Int64".to_string()),
                ],
                result: Some(Type::Named("Unit".to_string())),
            },
        );

        Ok(())
    }

    /// Generate temporal cleanup functions for resource management
    fn generate_temporal_cleanup_functions(&mut self) -> Result<(), CodeGenError> {
        self.output
            .push_str("\n  ;; Temporal resource cleanup functions\n");

        // Resource tracking table: each entry is [resource_ptr, cleanup_fn_ptr, next_entry]
        self.output
            .push_str("  (global $resource_list_head (mut i32) (i32.const 0))\n");

        // Register resource for cleanup
        self.output.push_str(
            "  (func $register_resource (param $resource_ptr i32) (param $cleanup_fn i32)\n",
        );
        self.output.push_str("    (local $entry i32)\n");
        self.output
            .push_str("    ;; Allocate 12 bytes for entry: [resource_ptr, cleanup_fn, next]\n");
        self.output.push_str("    i32.const 12\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $entry\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Store resource pointer\n");
        self.output.push_str("    local.get $entry\n");
        self.output.push_str("    local.get $resource_ptr\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output
            .push_str("    ;; Store cleanup function pointer\n");
        self.output.push_str("    local.get $entry\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $cleanup_fn\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Link to existing list head\n");
        self.output.push_str("    local.get $entry\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    global.get $resource_list_head\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Update list head\n");
        self.output.push_str("    local.get $entry\n");
        self.output.push_str("    global.set $resource_list_head\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "register_resource".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I32],
                result: None,
            },
        );

        // Clean up all registered resources (simplified)
        self.output.push_str("  (func $cleanup_resources\n");
        self.output.push_str("    (local $current i32)\n");
        self.output.push_str("    (local $resource_ptr i32)\n");
        self.output.push_str("    (local $cleanup_type i32)\n");
        self.output.push_str("    (local $next i32)\n");
        self.output.push_str("    \n");
        self.output.push_str("    global.get $resource_list_head\n");
        self.output.push_str("    local.set $current\n");
        self.output.push_str("    \n");
        self.output
            .push_str("    ;; Iterate through resource list\n");
        self.output.push_str("    (loop $cleanup_loop\n");
        self.output.push_str("      local.get $current\n");
        self.output.push_str("      i32.eqz\n");
        self.output.push_str("      (if\n");
        self.output.push_str("        (then br $cleanup_loop)\n");
        self.output.push_str("      )\n");
        self.output.push_str("      \n");
        self.output.push_str("      ;; Load resource pointer\n");
        self.output.push_str("      local.get $current\n");
        self.output.push_str("      i32.load\n");
        self.output.push_str("      local.set $resource_ptr\n");
        self.output.push_str("      \n");
        self.output
            .push_str("      ;; Load cleanup function type\n");
        self.output.push_str("      local.get $current\n");
        self.output.push_str("      i32.const 4\n");
        self.output.push_str("      i32.add\n");
        self.output.push_str("      i32.load\n");
        self.output.push_str("      local.set $cleanup_type\n");
        self.output.push_str("      \n");
        self.output.push_str("      ;; Load next pointer\n");
        self.output.push_str("      local.get $current\n");
        self.output.push_str("      i32.const 8\n");
        self.output.push_str("      i32.add\n");
        self.output.push_str("      i32.load\n");
        self.output.push_str("      local.set $next\n");
        self.output.push_str("      \n");
        self.output
            .push_str("      ;; Call appropriate cleanup function based on type\n");
        self.output.push_str("      local.get $cleanup_type\n");
        self.output.push_str("      i32.const 1\n");
        self.output.push_str("      i32.eq\n");
        self.output.push_str("      (if\n");
        self.output.push_str("        (then\n");
        self.output.push_str("          local.get $resource_ptr\n");
        self.output.push_str("          call $cleanup_file\n");
        self.output.push_str("        )\n");
        self.output.push_str("      )\n");
        self.output.push_str("      \n");
        self.output.push_str("      local.get $cleanup_type\n");
        self.output.push_str("      i32.const 2\n");
        self.output.push_str("      i32.eq\n");
        self.output.push_str("      (if\n");
        self.output.push_str("        (then\n");
        self.output.push_str("          local.get $resource_ptr\n");
        self.output.push_str("          call $cleanup_database\n");
        self.output.push_str("        )\n");
        self.output.push_str("      )\n");
        self.output.push_str("      \n");
        self.output.push_str("      local.get $cleanup_type\n");
        self.output.push_str("      i32.const 3\n");
        self.output.push_str("      i32.eq\n");
        self.output.push_str("      (if\n");
        self.output.push_str("        (then\n");
        self.output.push_str("          local.get $resource_ptr\n");
        self.output
            .push_str("          call $cleanup_transaction\n");
        self.output.push_str("        )\n");
        self.output.push_str("      )\n");
        self.output.push_str("      \n");
        self.output.push_str("      ;; Move to next entry\n");
        self.output.push_str("      local.get $next\n");
        self.output.push_str("      local.set $current\n");
        self.output.push_str("      br $cleanup_loop\n");
        self.output.push_str("    )\n");
        self.output.push_str("    \n");
        self.output.push_str("    ;; Clear the resource list\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    global.set $resource_list_head\n");
        self.output.push_str("  )\n");

        self.functions.insert(
            "cleanup_resources".to_string(),
            FunctionSig {
                _params: vec![],
                result: None,
            },
        );

        // Generate common resource cleanup functions
        self.generate_common_cleanup_functions()?;
        Ok(())
    }

    /// Generate cleanup functions for common resource types
    fn generate_common_cleanup_functions(&mut self) -> Result<(), CodeGenError> {
        // File handle cleanup
        self.output.push_str("  ;; File handle cleanup function\n");
        self.output
            .push_str("  (func $cleanup_file (param $file_ptr i32)\n");
        self.output
            .push_str("    ;; Close file handle (simplified - would call WASI fd_close)\n");
        self.output.push_str("    local.get $file_ptr\n");
        self.output
            .push_str("    i32.load  ;; Load file handle from first field\n");
        self.output
            .push_str("    ;; call $wasi_close  ;; Would call actual WASI close\n");
        self.output
            .push_str("    drop      ;; For now, just drop the handle\n");
        self.output.push_str("  )\n");

        // Database connection cleanup
        self.output
            .push_str("  ;; Database connection cleanup function\n");
        self.output
            .push_str("  (func $cleanup_database (param $db_ptr i32)\n");
        self.output
            .push_str("    ;; Close database connection (simplified)\n");
        self.output.push_str("    local.get $db_ptr\n");
        self.output
            .push_str("    i32.load  ;; Load connection handle\n");
        self.output
            .push_str("    ;; call $db_close  ;; Would call actual database close\n");
        self.output
            .push_str("    drop      ;; For now, just drop the handle\n");
        self.output.push_str("  )\n");

        // Transaction cleanup
        self.output.push_str("  ;; Transaction cleanup function\n");
        self.output
            .push_str("  (func $cleanup_transaction (param $tx_ptr i32)\n");
        self.output
            .push_str("    ;; Rollback transaction if not committed\n");
        self.output.push_str("    local.get $tx_ptr\n");
        self.output
            .push_str("    i32.const 8  ;; Offset to txId field\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.load\n");
        self.output
            .push_str("    ;; call $tx_rollback  ;; Would call actual transaction rollback\n");
        self.output.push_str("    drop\n");
        self.output.push_str("  )\n");

        // Register cleanup functions in the mapping
        self.cleanup_functions
            .insert("File".to_string(), "cleanup_file".to_string());
        self.cleanup_functions
            .insert("Database".to_string(), "cleanup_database".to_string());
        self.cleanup_functions
            .insert("Transaction".to_string(), "cleanup_transaction".to_string());

        // Add function signatures for cleanup functions
        self.functions.insert(
            "cleanup_file".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: None,
            },
        );

        self.functions.insert(
            "cleanup_database".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: None,
            },
        );

        self.functions.insert(
            "cleanup_transaction".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32],
                result: None,
            },
        );

        Ok(())
    }

    fn collect_strings(&mut self, program: &Program) -> Result<(), CodeGenError> {
        for decl in &program.declarations {
            match Self::decl_codegen_item(decl) {
                TopDecl::Function(func) => {
                    self.collect_strings_from_block(&func.body)?;
                }
                TopDecl::Binding(val) => {
                    self.collect_strings_from_expr(&val.value)?;
                }
                TopDecl::Record(_record) => {
                    // Records don't have methods in the current AST
                }
                TopDecl::Impl(impl_block) => {
                    for func in &impl_block.functions {
                        self.collect_strings_from_block(&func.body)?;
                    }
                }
                TopDecl::Export(_) => {
                    // Not yet implemented
                }
                TopDecl::Context(_) => {
                    // Not yet implemented
                }
            }
        }
        Ok(())
    }

    fn intern_string_literal(&mut self, s: &str) {
        if !self.string_offsets.contains_key(s) {
            let offset = self.next_mem_offset;
            self.string_offsets.insert(s.to_string(), offset);
            self.strings.push(s.to_string());
            // Account for length prefix (4 bytes) + string data.
            self.next_mem_offset += 4 + s.len() as u32;
            // Align to 4 bytes.
            self.next_mem_offset = (self.next_mem_offset + 3) & !3;
        }
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
                self.intern_string_literal(s);
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
            Expr::Unary(unary) => {
                self.collect_strings_from_expr(&unary.expr)?;
            }
            Expr::Cast(cast) => {
                self.collect_strings_from_expr(&cast.expr)?;
            }
            Expr::Pipe(pipe) => {
                self.collect_strings_from_expr(&pipe.expr)?;
                if let PipeTarget::Expr(target) = &pipe.target {
                    self.collect_strings_from_expr(target)?;
                }
            }
            Expr::RecordLit(record) => {
                for field in &record.fields {
                    match field {
                        FieldInit::Field { value, .. } => {
                            self.collect_strings_from_expr(value)?;
                        }
                        FieldInit::Spread(expr) => {
                            self.collect_strings_from_expr(expr)?;
                        }
                    }
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
            Expr::RangeLit(range) => {
                self.collect_strings_from_expr(&range.start)?;
                self.collect_strings_from_expr(&range.end)?;
            }
            Expr::Match(match_expr) => {
                self.collect_strings_from_expr(&match_expr.expr)?;
                for arm in &match_expr.arms {
                    self.collect_strings_from_pattern(&arm.pattern)?;
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
                for binding in &with.bindings {
                    match binding {
                        FieldInit::Field { value, .. } => {
                            self.collect_strings_from_expr(value)?;
                        }
                        FieldInit::Spread(expr) => {
                            self.collect_strings_from_expr(expr)?;
                        }
                    }
                }
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
                    match field {
                        FieldInit::Field { value, .. } => {
                            self.collect_strings_from_expr(value)?;
                        }
                        FieldInit::Spread(expr) => {
                            self.collect_strings_from_expr(expr)?;
                        }
                    }
                }
            }
            Expr::Freeze(expr) => {
                self.collect_strings_from_expr(expr)?;
            }
            Expr::Some(expr) | Expr::Ok(expr) | Expr::Err(expr) => {
                self.collect_strings_from_expr(expr)?;
            }
            Expr::Lambda(lambda) => {
                self.collect_strings_from_expr(&lambda.body)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn collect_strings_from_pattern(&mut self, pattern: &Pattern) -> Result<(), CodeGenError> {
        match pattern {
            Pattern::Literal(Literal::String(s)) => {
                self.intern_string_literal(s);
            }
            Pattern::Record(_, fields) => {
                for (_, pattern) in fields {
                    self.collect_strings_from_pattern(pattern)?;
                }
            }
            Pattern::RecordDestruct { fields, .. } => {
                for (_, pattern) in fields {
                    self.collect_strings_from_pattern(pattern)?;
                }
            }
            Pattern::Some(inner) | Pattern::Ok(inner) | Pattern::Err(inner) => {
                self.collect_strings_from_pattern(inner)?;
            }
            Pattern::ListCons(head, tail) => {
                self.collect_strings_from_pattern(head)?;
                self.collect_strings_from_pattern(tail)?;
            }
            Pattern::ListExact(patterns) => {
                for pattern in patterns {
                    self.collect_strings_from_pattern(pattern)?;
                }
            }
            Pattern::Wildcard
            | Pattern::Ident(_)
            | Pattern::Literal(_)
            | Pattern::None
            | Pattern::EmptyList => {}
        }

        Ok(())
    }

    fn register_function_signature(&mut self, func: &FunDecl) -> Result<(), CodeGenError> {
        let type_params: Vec<String> = func
            .type_params
            .iter()
            .map(|param| param.name.clone())
            .collect();
        let params: Vec<WasmType> = func
            .params
            .iter()
            .map(|param| self.convert_signature_type(&param.ty, &type_params))
            .collect::<Result<Vec<_>, _>>()?;

        let source_result = if let Some(return_type) = &func.return_type {
            Some(return_type.clone())
        } else {
            self.infer_function_body_source_type(func)
        };

        let result = if let Some(return_type) = &func.return_type {
            self.convert_signature_result_type(return_type, &type_params)?
        } else if let Some(source_result) = &source_result {
            self.convert_signature_result_type(source_result, &type_params)?
        } else if func.body.expr.is_some() {
            return Err(CodeGenError::UnsupportedFeature(format!(
                "function '{}' return ABI requires a return type annotation or inferable body source type",
                func.name
            )));
        } else {
            None
        };

        self.functions.insert(
            func.name.clone(),
            FunctionSig {
                _params: params,
                result,
            },
        );

        self.function_source_sigs.insert(
            func.name.clone(),
            FunctionSourceSig {
                type_params,
                params: func.params.iter().map(|param| param.ty.clone()).collect(),
                result: source_result,
            },
        );
        self.function_decls.insert(func.name.clone(), func.clone());

        Ok(())
    }

    fn infer_function_body_source_type(&mut self, func: &FunDecl) -> Option<Type> {
        self.local_source_types.push(HashMap::new());
        for param in &func.params {
            self.set_local_source_type(&param.name, param.ty.clone());
        }

        let result = self.infer_block_source_type_for_signature(&func.body);
        self.local_source_types.pop();
        result
    }

    fn infer_block_source_type_for_signature(&mut self, block: &BlockExpr) -> Option<Type> {
        for stmt in &block.statements {
            if let Stmt::Binding(bind) = stmt {
                let value_ty = bind
                    .type_annotation
                    .clone()
                    .or_else(|| self.infer_expr_source_type_for_signature(&bind.value));
                self.bind_pattern_source_types_for_signature(&bind.pattern, value_ty.as_ref());
            }
        }

        if let Some(expr) = &block.expr {
            return self.infer_expr_source_type_for_signature(expr);
        }

        match block.statements.last() {
            Some(Stmt::Expr(expr)) => self.infer_expr_source_type_for_signature(expr),
            _ => None,
        }
    }

    fn infer_expr_source_type_for_signature(&mut self, expr: &Expr) -> Option<Type> {
        match expr {
            Expr::Block(block) => {
                self.local_source_types.push(HashMap::new());
                let ty = self.infer_block_source_type_for_signature(block);
                self.local_source_types.pop();
                ty
            }
            Expr::Then(then) => {
                let mut ty = None;

                self.local_source_types.push(HashMap::new());
                ty = Self::merge_source_types(
                    ty,
                    self.infer_block_source_type_for_signature(&then.then_block),
                );
                self.local_source_types.pop();

                for (_, block) in &then.else_ifs {
                    self.local_source_types.push(HashMap::new());
                    ty = Self::merge_source_types(
                        ty,
                        self.infer_block_source_type_for_signature(block),
                    );
                    self.local_source_types.pop();
                }

                if let Some(block) = &then.else_block {
                    self.local_source_types.push(HashMap::new());
                    ty = Self::merge_source_types(
                        ty,
                        self.infer_block_source_type_for_signature(block),
                    );
                    self.local_source_types.pop();
                }

                ty
            }
            Expr::Match(match_expr) => {
                let scrutinee_ty = self.infer_expr_source_type_for_signature(&match_expr.expr);
                let mut ty = None;

                for arm in &match_expr.arms {
                    self.local_source_types.push(HashMap::new());
                    self.bind_pattern_source_types_for_signature(
                        &arm.pattern,
                        scrutinee_ty.as_ref(),
                    );
                    ty = Self::merge_source_types(
                        ty,
                        self.infer_block_source_type_for_signature(&arm.body),
                    );
                    self.local_source_types.pop();
                }

                ty
            }
            Expr::With(with) => {
                let bindings = self.context_source_bindings(with, &HashMap::new());
                self.local_source_types.push(bindings);
                let ty = self.infer_block_source_type_for_signature(&with.body);
                self.local_source_types.pop();
                ty
            }
            _ => self.infer_expr_source_type(expr),
        }
    }

    fn bind_pattern_source_types_for_signature(
        &mut self,
        pattern: &Pattern,
        value_ty: Option<&Type>,
    ) {
        match pattern {
            Pattern::Ident(name) => {
                if let Some(ty) = value_ty {
                    self.set_local_source_type(name, ty.clone());
                }
            }
            Pattern::Record(record_name, fields) => {
                self.bind_record_pattern_source_types_for_signature(record_name, fields, None);
            }
            Pattern::RecordDestruct {
                type_name,
                fields,
                rest,
            } => {
                self.bind_record_pattern_source_types_for_signature(
                    type_name,
                    fields,
                    rest.as_ref(),
                );
            }
            Pattern::Some(inner) => {
                let payload_ty = self.variant_payload_type(value_ty, "Some").cloned();
                self.bind_pattern_source_types_for_signature(inner, payload_ty.as_ref());
            }
            Pattern::Ok(inner) => {
                let payload_ty = self.variant_payload_type(value_ty, "Ok").cloned();
                self.bind_pattern_source_types_for_signature(inner, payload_ty.as_ref());
            }
            Pattern::Err(inner) => {
                let payload_ty = self.variant_payload_type(value_ty, "Err").cloned();
                self.bind_pattern_source_types_for_signature(inner, payload_ty.as_ref());
            }
            Pattern::ListCons(head, tail) => {
                let element_ty = match value_ty {
                    Some(Type::Generic(name, params)) if name == "List" => params.first().cloned(),
                    _ => None,
                };
                self.bind_pattern_source_types_for_signature(head, element_ty.as_ref());
                self.bind_pattern_source_types_for_signature(tail, value_ty);
            }
            Pattern::ListExact(patterns) => {
                let element_ty = match value_ty {
                    Some(Type::Generic(name, params)) if name == "List" => params.first().cloned(),
                    _ => None,
                };
                for pattern in patterns {
                    self.bind_pattern_source_types_for_signature(pattern, element_ty.as_ref());
                }
            }
            Pattern::Wildcard | Pattern::Literal(_) | Pattern::None | Pattern::EmptyList => {}
        }
    }

    fn bind_record_pattern_source_types_for_signature(
        &mut self,
        record_name: &str,
        fields: &[(String, Pattern)],
        rest: Option<&String>,
    ) {
        for (field_name, field_pattern) in fields {
            let field_ty = self.record_field_type(record_name, field_name).cloned();
            self.bind_pattern_source_types_for_signature(field_pattern, field_ty.as_ref());
        }

        if let Some(rest_name) = rest {
            if rest_name != "_" {
                if let Ok(residual_name) =
                    self.ensure_residual_record_definition(record_name, fields)
                {
                    self.set_local_source_type(rest_name, Type::Named(residual_name));
                }
            }
        }
    }

    fn register_record_methods(&mut self, _record: &RecordDecl) -> Result<(), CodeGenError> {
        // Records don't have methods in the current AST
        Ok(())
    }

    fn method_function_name(record_name: &str, method_name: &str) -> String {
        format!("{}_{}", record_name, method_name)
    }

    fn method_codegen_decl(&self, target: &str, func: &FunDecl) -> FunDecl {
        let mut method = func.clone();
        method.name = Self::method_function_name(target, &func.name);

        if let Some(first_param) = method.params.first_mut() {
            if first_param.name == "self" {
                first_param.ty = Type::Named(target.to_string());
            }
        }

        method
    }

    fn register_impl_method_signature(
        &mut self,
        target: &str,
        func: &FunDecl,
    ) -> Result<(), CodeGenError> {
        let method = self.method_codegen_decl(target, func);
        if self
            .methods
            .get(target)
            .and_then(|method_map| method_map.get(&func.name))
            .is_some()
        {
            return Err(CodeGenError::UnsupportedFeature(format!(
                "Duplicate method '{}' for record '{}'",
                func.name, target
            )));
        }

        self.register_function_signature(&method)?;

        let sig = self
            .functions
            .get(&method.name)
            .cloned()
            .ok_or_else(|| CodeGenError::UndefinedFunction(method.name.clone()))?;
        self.methods
            .entry(target.to_string())
            .or_default()
            .insert(func.name.clone(), sig);

        Ok(())
    }

    fn register_impl_methods(&mut self, impl_block: &ImplBlock) -> Result<(), CodeGenError> {
        if !self.records.contains_key(&impl_block.target) {
            return Err(CodeGenError::UnsupportedType(format!(
                "impl target '{}' is not a known record",
                impl_block.target
            )));
        }

        for func in &impl_block.functions {
            if func.return_type.is_some() {
                self.register_impl_method_signature(&impl_block.target, func)?;
            }
        }

        for func in &impl_block.functions {
            if func.return_type.is_none() {
                self.register_impl_method_signature(&impl_block.target, func)?;
            }
        }

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
                    "Int64" | "Float64" => 8,
                    _ => 4, // Pointers are 4 bytes
                },
                _ => 4, // Default to pointer size
            };

            offset += field_size;
        }

        self.record_type_params.insert(
            record.name.clone(),
            record
                .type_params
                .iter()
                .filter(|param| !param.is_temporal)
                .map(|param| param.name.clone())
                .collect(),
        );
        self.records.insert(record.name.clone(), fields);
        self.record_field_offsets
            .insert(record.name.clone(), field_offsets);

        Ok(())
    }

    fn size_of_type(&self, ty: &Type) -> u32 {
        match ty {
            Type::Named(name) => match name.as_str() {
                "Int32" | "Boolean" | "Char" => 4,
                "Int64" | "Float64" => 8,
                _ => 4,
            },
            _ => 4,
        }
    }

    fn register_context_definition(&mut self, context: &ContextDecl) -> Result<(), CodeGenError> {
        let mut fields = Vec::new();
        let mut field_offsets = HashMap::new();
        let mut offset = 0u32;

        for field in &context.fields {
            fields.push((field.name.clone(), field.ty.clone()));
            field_offsets.insert(field.name.clone(), offset);
            offset += self.size_of_type(&field.ty);
        }

        self.records.insert(context.name.clone(), fields);
        self.record_field_offsets
            .insert(context.name.clone(), field_offsets);

        Ok(())
    }

    fn global_binding_name<'a>(&self, binding: &'a BindDecl) -> Result<&'a str, CodeGenError> {
        match &binding.pattern {
            Pattern::Ident(name) => Ok(name),
            _ => Err(CodeGenError::UnsupportedFeature(
                "Complex top-level bindings are not supported by codegen yet".to_string(),
            )),
        }
    }

    fn global_binding_source_type(&self, binding: &BindDecl) -> Result<Type, CodeGenError> {
        binding
            .type_annotation
            .clone()
            .or_else(|| self.infer_expr_source_type(&binding.value))
            .ok_or_else(|| {
                CodeGenError::UnsupportedFeature(
                    "Top-level binding requires an inferable constant type".to_string(),
                )
            })
    }

    fn register_global_binding(&mut self, binding: &BindDecl) -> Result<(), CodeGenError> {
        if binding.mutable {
            return Err(CodeGenError::UnsupportedFeature(
                "Top-level mutable bindings are not supported by codegen yet".to_string(),
            ));
        }

        let name = self.global_binding_name(binding)?.to_string();
        let source_ty = self.global_binding_source_type(binding)?;
        let wasm_ty = self.convert_type(&source_ty)?;
        self.global_source_types.insert(name.clone(), source_ty);
        self.global_types.insert(name, wasm_ty);
        Ok(())
    }

    fn generate_global_bindings(&mut self, program: &Program) -> Result<(), CodeGenError> {
        let mut has_globals = false;

        for decl in &program.declarations {
            if let TopDecl::Binding(binding) = Self::decl_codegen_item(decl) {
                if !has_globals {
                    self.output.push_str("\n  ;; Top-level constants\n");
                    has_globals = true;
                }

                let name = self.global_binding_name(binding)?;
                let source_ty = self.global_binding_source_type(binding)?;
                let is_exported_binding = matches!(
                    decl,
                    TopDecl::Export(export_decl)
                        if matches!(export_decl.item.as_ref(), TopDecl::Binding(_))
                );
                if is_exported_binding {
                    Self::ensure_scalar_host_export_global(name, &source_ty)?;
                }
                let wasm_ty = self.convert_type(&source_ty)?;
                let init_expr = self.global_const_expr(&binding.value, &source_ty)?;
                self.output.push_str(&format!(
                    "  (global ${} {} ({}))\n",
                    name,
                    self.wasm_type_str(wasm_ty),
                    init_expr
                ));
            }
        }

        Ok(())
    }

    fn global_const_expr(&self, expr: &Expr, source_ty: &Type) -> Result<String, CodeGenError> {
        match (expr, source_ty) {
            (Expr::IntLit(value), Type::Named(name)) if name == "Int32" => {
                Ok(format!("i32.const {}", value))
            }
            (Expr::IntLit(value), Type::Named(name)) if name == "Int64" => {
                Ok(format!("i64.const {}", value))
            }
            (Expr::FloatLit(value), Type::Named(name)) if name == "Float64" => {
                Ok(format!("f64.const {}", value))
            }
            (Expr::BoolLit(value), Type::Named(name)) if name == "Boolean" => {
                Ok(format!("i32.const {}", if *value { 1 } else { 0 }))
            }
            (Expr::CharLit(value), Type::Named(name)) if name == "Char" => {
                Ok(format!("i32.const {}", *value as u32))
            }
            (Expr::StringLit(value), Type::Named(name)) if name == "String" => {
                let offset = self.string_offsets.get(value).ok_or_else(|| {
                    CodeGenError::NotImplemented("string literal not in pool".to_string())
                })?;
                Ok(format!("i32.const {}", offset))
            }
            (Expr::Unit, Type::Named(name)) if name == "Unit" => Ok("i32.const 0".to_string()),
            (Expr::Unary(unary), Type::Named(name)) if name == "Int32" => {
                if let (UnaryOp::Neg, Expr::IntLit(value)) = (&unary.op, unary.expr.as_ref()) {
                    Ok(format!("i32.const {}", -value))
                } else {
                    Err(CodeGenError::UnsupportedFeature(
                        "Top-level Int32 constants must be literals".to_string(),
                    ))
                }
            }
            (Expr::Unary(unary), Type::Named(name)) if name == "Int64" => {
                if let (UnaryOp::Neg, Expr::IntLit(value)) = (&unary.op, unary.expr.as_ref()) {
                    Ok(format!("i64.const {}", -value))
                } else {
                    Err(CodeGenError::UnsupportedFeature(
                        "Top-level Int64 constants must be literals".to_string(),
                    ))
                }
            }
            (Expr::Unary(unary), Type::Named(name)) if name == "Float64" => {
                if let (UnaryOp::Neg, Expr::FloatLit(value)) = (&unary.op, unary.expr.as_ref()) {
                    Ok(format!("f64.const {}", -value))
                } else {
                    Err(CodeGenError::UnsupportedFeature(
                        "Top-level Float64 constants must be literals".to_string(),
                    ))
                }
            }
            _ => Err(CodeGenError::UnsupportedFeature(format!(
                "Top-level binding of type {:?} requires runtime initialization and is not supported by codegen yet",
                source_ty
            ))),
        }
    }

    fn generate_record_methods(&mut self, _record: &RecordDecl) -> Result<(), CodeGenError> {
        // Records don't have methods in the current AST
        Ok(())
    }

    fn generate_impl_methods(&mut self, impl_block: &ImplBlock) -> Result<(), CodeGenError> {
        for func in &impl_block.functions {
            let method = self.method_codegen_decl(&impl_block.target, func);
            self.generate_function(&method)?;
        }

        Ok(())
    }

    fn convert_type(&self, ty: &Type) -> Result<WasmType, CodeGenError> {
        match ty {
            Type::Named(name) => match name.as_str() {
                "Int32" | "Boolean" | "Char" | "Unit" => Ok(WasmType::I32),
                "Int64" => Ok(WasmType::I64),
                "Float64" => Ok(WasmType::F64),
                "String" => Ok(WasmType::I32), // String is a pointer
                _ if self.records.contains_key(name) => Ok(WasmType::I32),
                _ => Err(CodeGenError::UnsupportedType(format!(
                    "unknown source type '{}' has no Wasm ABI{}",
                    name,
                    self.current_function
                        .as_ref()
                        .map(|function| format!(" while generating '{}'", function))
                        .unwrap_or_default()
                ))),
            },
            Type::Generic(name, _params) => match name.as_str() {
                "List" | "Option" | "Result" | "Array" | "Range" => Ok(WasmType::I32), // All are pointers
                _ if self.records.contains_key(name) => Ok(WasmType::I32),
                _ => Err(CodeGenError::UnsupportedType(format!(
                    "generic source type '{}' has no Wasm ABI{}",
                    ty,
                    self.current_function
                        .as_ref()
                        .map(|function| format!(" while generating '{}'", function))
                        .unwrap_or_default()
                ))),
            },
            Type::Function(_, _) => Ok(WasmType::I32), // Function pointers
            Type::Temporal(name, _temporals) => {
                // Temporal types are treated like their base type
                self.convert_type(&Type::Named(name.clone()))
            }
        }
    }

    fn convert_signature_type(
        &self,
        ty: &Type,
        type_params: &[String],
    ) -> Result<WasmType, CodeGenError> {
        match ty {
            Type::Named(name) if type_params.iter().any(|param| param == name) => {
                // A generic declaration has no concrete Wasm ABI until a call
                // site specializes it. Keep an erased placeholder only for
                // name resolution; specialized functions use `convert_type`.
                Ok(WasmType::I32)
            }
            Type::Temporal(name, _) if type_params.iter().any(|param| param == name) => {
                Ok(WasmType::I32)
            }
            _ => self.convert_type(ty),
        }
    }

    fn convert_signature_result_type(
        &self,
        ty: &Type,
        type_params: &[String],
    ) -> Result<Option<WasmType>, CodeGenError> {
        if type_params.is_empty() {
            return self.convert_result_type(ty);
        }

        match ty {
            Type::Named(name) if name == "Unit" => Ok(None),
            _ => Ok(Some(self.convert_signature_type(ty, type_params)?)),
        }
    }

    fn type_size(&self, ty: &Type) -> usize {
        match ty {
            Type::Named(name) if name == "Int64" || name == "Float64" => 8,
            _ => 4,
        }
    }

    fn wasm_type_size(&self, ty: WasmType) -> usize {
        match ty {
            WasmType::I64 | WasmType::F64 => 8,
            WasmType::I32 | WasmType::F32 => 4,
        }
    }

    fn record_size(&self, record_name: &str, fallback_field_count: usize) -> usize {
        self.records
            .get(record_name)
            .map(|fields| fields.iter().map(|(_, ty)| self.type_size(ty)).sum())
            .unwrap_or(fallback_field_count * 4)
    }

    fn instantiated_record_fields(
        &self,
        record_name: &str,
        record_ty: Option<&Type>,
    ) -> Option<Vec<(String, Type)>> {
        let fields = self.records.get(record_name)?;
        let bindings = record_ty
            .map(|ty| self.record_type_arg_bindings(record_name, ty))
            .unwrap_or_default();

        Some(
            fields
                .iter()
                .map(|(name, ty)| (name.clone(), Self::apply_record_type_args(ty, &bindings)))
                .collect(),
        )
    }

    fn instantiated_record_size(
        &self,
        record_name: &str,
        record_ty: Option<&Type>,
        fallback_field_count: usize,
    ) -> usize {
        self.instantiated_record_fields(record_name, record_ty)
            .map(|fields| fields.iter().map(|(_, ty)| self.type_size(ty)).sum())
            .unwrap_or(fallback_field_count * 4)
    }

    fn instantiated_record_field_offset(
        &self,
        record_name: &str,
        record_ty: Option<&Type>,
        field_name: &str,
    ) -> Result<u32, CodeGenError> {
        let fields = self
            .instantiated_record_fields(record_name, record_ty)
            .ok_or_else(|| Self::invalid_record_layout_error(record_name, field_name))?;
        let mut offset = 0u32;

        for (name, ty) in fields {
            if name == field_name {
                return Ok(offset);
            }
            offset += self.type_size(&ty) as u32;
        }

        Err(Self::invalid_record_layout_error(record_name, field_name))
    }

    fn invalid_record_layout_error(record_name: &str, field_name: &str) -> CodeGenError {
        CodeGenError::UnsupportedFeature(format!(
            "invalid record layout: field '{}' in record '{}' has no registered codegen layout metadata",
            field_name, record_name
        ))
    }

    fn instantiated_record_field_type_by_name(
        &self,
        record_name: &str,
        record_ty: Option<&Type>,
        field_name: &str,
    ) -> Option<Type> {
        if let Some(record_ty) = record_ty {
            self.instantiated_record_field_type(record_ty, field_name)
        } else {
            self.record_field_type(record_name, field_name).cloned()
        }
    }

    fn residual_record_name(record_name: &str, remaining_fields: &[String]) -> String {
        let mut stable_fields = remaining_fields.to_vec();
        stable_fields.sort();
        if stable_fields.is_empty() {
            format!("__RestrictRest_{}_empty", record_name)
        } else {
            format!("__RestrictRest_{}_{}", record_name, stable_fields.join("_"))
        }
    }

    fn ensure_residual_record_definition(
        &mut self,
        record_name: &str,
        fields: &[(String, Pattern)],
    ) -> Result<String, CodeGenError> {
        let extracted: HashSet<String> = fields
            .iter()
            .map(|(field_name, _)| field_name.clone())
            .collect();
        let source_fields = self.records.get(record_name).cloned().ok_or_else(|| {
            CodeGenError::NotImplemented(format!("residual record for {}", record_name))
        })?;
        let residual_fields: Vec<(String, Type)> = source_fields
            .into_iter()
            .filter(|(field_name, _)| !extracted.contains(field_name))
            .collect();
        let remaining_names: Vec<String> = residual_fields
            .iter()
            .map(|(field_name, _)| field_name.clone())
            .collect();
        let residual_name = Self::residual_record_name(record_name, &remaining_names);

        if !self.records.contains_key(&residual_name) {
            let mut field_offsets = HashMap::new();
            let mut offset = 0u32;
            for (field_name, field_ty) in &residual_fields {
                field_offsets.insert(field_name.clone(), offset);
                offset += self.type_size(field_ty) as u32;
            }
            self.records.insert(residual_name.clone(), residual_fields);
            self.record_field_offsets
                .insert(residual_name.clone(), field_offsets);
        }

        Ok(residual_name)
    }

    fn wasm_load_op_for_type(&self, ty: Option<&Type>) -> &'static str {
        match ty {
            Some(Type::Named(name)) if name == "Int64" => "i64.load",
            Some(Type::Named(name)) if name == "Float64" => "f64.load",
            _ => "i32.load",
        }
    }

    fn wasm_store_op_for_type(&self, ty: Option<&Type>) -> &'static str {
        match ty {
            Some(Type::Named(name)) if name == "Int64" => "i64.store",
            Some(Type::Named(name)) if name == "Float64" => "f64.store",
            _ => "i32.store",
        }
    }

    fn wasm_load_op_for_wasm_type(&self, ty: WasmType) -> &'static str {
        match ty {
            WasmType::I32 => "i32.load",
            WasmType::I64 => "i64.load",
            WasmType::F32 => "f32.load",
            WasmType::F64 => "f64.load",
        }
    }

    fn wasm_store_op_for_wasm_type(&self, ty: WasmType) -> &'static str {
        match ty {
            WasmType::I32 => "i32.store",
            WasmType::I64 => "i64.store",
            WasmType::F32 => "f32.store",
            WasmType::F64 => "f64.store",
        }
    }

    fn source_record_name<'a>(&self, ty: &'a Type) -> Option<&'a str> {
        match ty {
            Type::Named(name) | Type::Generic(name, _) if self.records.contains_key(name) => {
                Some(name)
            }
            Type::Temporal(name, _) if self.records.contains_key(name) => Some(name),
            _ => None,
        }
    }

    fn record_type_arg_bindings(&self, record_name: &str, ty: &Type) -> HashMap<String, Type> {
        let Some(type_params) = self.record_type_params.get(record_name) else {
            return HashMap::new();
        };

        let args = match ty {
            Type::Generic(name, args) if name == record_name => args.as_slice(),
            _ => &[],
        };

        type_params
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect()
    }

    fn apply_record_type_args(ty: &Type, bindings: &HashMap<String, Type>) -> Type {
        match ty {
            Type::Named(name) => bindings.get(name).cloned().unwrap_or_else(|| ty.clone()),
            Type::Generic(name, args) => Type::Generic(
                name.clone(),
                args.iter()
                    .map(|arg| Self::apply_record_type_args(arg, bindings))
                    .collect(),
            ),
            Type::Function(params, return_type) => Type::Function(
                params
                    .iter()
                    .map(|param| Self::apply_record_type_args(param, bindings))
                    .collect(),
                Box::new(Self::apply_record_type_args(return_type, bindings)),
            ),
            Type::Temporal(name, temporals) => Type::Temporal(name.clone(), temporals.clone()),
        }
    }

    fn instantiated_record_field_type(&self, record_ty: &Type, field: &str) -> Option<Type> {
        let record_name = self.source_record_name(record_ty)?;
        let field_ty = self.record_field_type(record_name, field)?;
        let bindings = self.record_type_arg_bindings(record_name, record_ty);
        Some(Self::apply_record_type_args(field_ty, &bindings))
    }

    fn infer_record_lit_source_type(&self, record: &RecordLit) -> Option<Type> {
        let type_params = self.record_type_params.get(&record.name).cloned()?;
        if type_params.is_empty() {
            return Some(Type::Named(record.name.clone()));
        }

        let mut substitution = HashMap::new();
        for field in &record.fields {
            let FieldInit::Field { name, value } = field else {
                continue;
            };
            let field_ty = self.record_field_type(&record.name, name)?;
            let Some(value_ty) = self.infer_expr_source_type(value) else {
                continue;
            };
            Self::bind_source_type_params(field_ty, &value_ty, &type_params, &mut substitution);
        }

        let args = type_params
            .iter()
            .map(|param| substitution.get(param).cloned())
            .collect::<Option<Vec<_>>>()?;
        Some(Type::Generic(record.name.clone(), args))
    }

    fn generate_record_literal_with_source_type(
        &mut self,
        record_lit: &RecordLit,
        record_source_ty: &Type,
    ) -> Result<(), CodeGenError> {
        if self.record_literal_depth >= self.record_tmp_count {
            return Err(CodeGenError::UnsupportedFeature(format!(
                "Record literal nesting deeper than {} levels",
                self.record_tmp_count
            )));
        }

        let record_tmp = format!("record_tmp_{}", self.record_literal_depth);
        self.record_literal_depth += 1;

        let result = (|| -> Result<(), CodeGenError> {
            let field_count = record_lit.fields.len();
            let record_size = self.instantiated_record_size(
                &record_lit.name,
                Some(record_source_ty),
                field_count,
            );

            self.output
                .push_str(&format!("    i32.const {}\n", record_size));
            self.output.push_str("    call $allocate\n");
            self.output
                .push_str(&format!("    local.set ${}\n", record_tmp));

            for field in &record_lit.fields {
                match field {
                    FieldInit::Field { name, value } => {
                        self.output
                            .push_str(&format!("    local.get ${}\n", record_tmp));

                        let offset = self.instantiated_record_field_offset(
                            &record_lit.name,
                            Some(record_source_ty),
                            name,
                        )?;

                        self.output.push_str(&format!("    i32.const {}\n", offset));
                        self.output.push_str("    i32.add\n");
                        let field_type =
                            self.instantiated_record_field_type(record_source_ty, name);
                        if let Some(Type::Function(param_types, return_type)) = field_type.as_ref()
                        {
                            let abi = self.source_function_abi(param_types, return_type)?;
                            self.generate_callable_value_with_abi(value, &abi)?;
                        } else {
                            match field_type.as_ref() {
                                Some(field_type) => {
                                    self.generate_expr_with_expected_source(value, field_type)?
                                }
                                None => self.generate_expr(value)?,
                            }
                        }
                        self.output.push_str(&format!(
                            "    {}\n",
                            self.wasm_store_op_for_type(field_type.as_ref())
                        ));
                    }
                    FieldInit::Spread(expr) => {
                        self.generate_record_spread_copy(&record_lit.name, expr, &record_tmp)?;
                    }
                }
            }

            self.output
                .push_str(&format!("    local.get ${}\n", record_tmp));

            if let Some(cleanup_fn) = self
                .cleanup_functions
                .get(&record_lit.name)
                .filter(|_| !self.temporal_scope_stack.is_empty())
            {
                self.output.push_str(&format!(
                    "    ;; Auto-register {} for temporal cleanup\n",
                    record_lit.name
                ));
                self.output.push_str("    local.tee $temp_resource\n");

                let cleanup_index = match cleanup_fn.as_str() {
                    "cleanup_file" => 1,
                    "cleanup_database" => 2,
                    "cleanup_transaction" => 3,
                    _ => 0,
                };

                self.output
                    .push_str(&format!("    i32.const {}\n", cleanup_index));
                self.output.push_str("    call $register_resource\n");
                self.output.push_str("    local.get $temp_resource\n");
            }

            Ok(())
        })();

        self.record_literal_depth -= 1;
        result
    }

    fn variant_payload_type<'a>(&self, ty: Option<&'a Type>, variant: &str) -> Option<&'a Type> {
        match (ty, variant) {
            (Some(Type::Generic(name, params)), "Some") if name == "Option" => params.first(),
            (Some(Type::Generic(name, params)), "Ok") if name == "Result" => params.first(),
            (Some(Type::Generic(name, params)), "Err") if name == "Result" => params.get(1),
            _ => None,
        }
    }

    fn variant_payload_wasm_type(&self, ty: Option<&Type>) -> Result<WasmType, CodeGenError> {
        match ty {
            Some(ty) => self.convert_type(ty),
            None => Ok(WasmType::I32),
        }
    }

    fn payload_temp_local(&self, ty: WasmType) -> &'static str {
        match ty {
            WasmType::I32 => "option_value_tmp",
            WasmType::I64 => "option_value_i64_tmp",
            WasmType::F32 => "option_value_f32_tmp",
            WasmType::F64 => "option_value_f64_tmp",
        }
    }

    fn match_temp_local(&self, ty: WasmType) -> &'static str {
        match ty {
            WasmType::I32 => "match_tmp",
            WasmType::I64 => "match_tmp_i64",
            WasmType::F32 => "match_tmp_f32",
            WasmType::F64 => "match_tmp_f64",
        }
    }

    fn variant_payload_load_code(
        &self,
        source_local: &str,
        payload_ty: Option<&Type>,
        indent: &str,
    ) -> Result<String, CodeGenError> {
        let wasm_ty = self.variant_payload_wasm_type(payload_ty)?;
        Ok(format!(
            "{indent}local.get ${source_local}\n{indent}i32.const 4\n{indent}i32.add\n{indent}{} ;; load variant payload\n",
            self.wasm_load_op_for_wasm_type(wasm_ty)
        ))
    }

    fn emit_variant_payload_load(
        &mut self,
        source_local: &str,
        payload_ty: Option<&Type>,
    ) -> Result<(), CodeGenError> {
        let load_code = self.variant_payload_load_code(source_local, payload_ty, "    ")?;
        self.output.push_str(&load_code);
        Ok(())
    }

    fn register_record_var_type(&mut self, name: &str, ty: &Type) {
        if let Some(type_name) = self.source_record_name(ty) {
            self.var_types
                .insert(name.to_string(), type_name.to_string());
        }
    }

    fn convert_result_type(&self, ty: &Type) -> Result<Option<WasmType>, CodeGenError> {
        match ty {
            Type::Named(name) if name == "Unit" => Ok(None),
            _ => Ok(Some(self.convert_type(ty)?)),
        }
    }

    // Generate specialized versions of generic functions
    fn generate_generic_function(&mut self, func: &FunDecl) -> Result<(), CodeGenError> {
        // Handle special generic functions
        match func.name.as_str() {
            "println" => self.generate_println_specializations(func),
            "new_list" => self.generate_new_list_specializations(func),
            "list_add" => self.generate_list_add_specializations(func),
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
            temporal_constraints: vec![],
            params: vec![Param {
                name: func.params[0].name.clone(),
                ty: Type::Named("String".to_string()),
                context_bound: None,
            }],
            return_type: Some(Type::Named("Unit".to_string())),
            body: BlockExpr {
                statements: vec![],
                expr: Some(Box::new(Expr::Call(CallExpr {
                    function: Box::new(Expr::Ident("println".to_string())), // Call built-in println
                    args: vec![Box::new(Expr::Ident(func.params[0].name.clone()))],
                }))),
            },
            is_async: false,
        };

        // Generate println_Int32 specialization
        let int_func = FunDecl {
            name: "println_Int32".to_string(),
            type_params: vec![],
            temporal_constraints: vec![],
            params: vec![Param {
                name: func.params[0].name.clone(),
                ty: Type::Named("Int32".to_string()),
                context_bound: None,
            }],
            return_type: Some(Type::Named("Unit".to_string())),
            body: BlockExpr {
                statements: vec![],
                expr: Some(Box::new(Expr::Call(CallExpr {
                    function: Box::new(Expr::Ident("print_int".to_string())), // Call built-in print_int
                    args: vec![Box::new(Expr::Ident(func.params[0].name.clone()))],
                }))),
            },
            is_async: false,
        };

        // Generate the specialized functions
        self.generate_function(&string_func)?;
        self.generate_function(&int_func)?;

        Ok(())
    }

    fn generate_start_wrapper(&mut self) -> Result<(), CodeGenError> {
        let main_sig = self
            .functions
            .get("main")
            .ok_or_else(|| CodeGenError::UndefinedFunction("main".to_string()))?;
        if !main_sig._params.is_empty() {
            return Ok(());
        }
        let main_returns_value = main_sig.result.is_some();
        let start_arena = self.next_arena_addr;
        self.next_arena_addr += ARENA_SIZE_BYTES;

        self.output.push_str("\n  ;; Program entry wrapper\n");
        self.output.push_str("  (func $__restrict_start\n");
        self.output.push_str("    (local $entry_prev_arena i32)\n");
        self.output.push_str("    ;; Save caller arena\n");
        self.output.push_str("    global.get $current_arena\n");
        self.output.push_str("    local.set $entry_prev_arena\n");
        self.output.push_str("    ;; Initialize default arena\n");
        self.output
            .push_str(&format!("    i32.const {}\n", start_arena));
        self.output.push_str("    call $arena_init\n");
        self.output.push_str("    global.set $current_arena\n\n");
        self.output.push_str("    call $main\n");
        if main_returns_value {
            self.output.push_str("    drop\n");
        }
        self.output.push_str("\n    ;; Reset default arena\n");
        self.output
            .push_str(&format!("    i32.const {}\n", start_arena));
        self.output.push_str("    call $arena_reset\n");
        self.output.push_str("    local.get $entry_prev_arena\n");
        self.output.push_str("    global.set $current_arena\n");
        self.output.push_str("  )\n");

        self.output.push_str("\n  ;; Export main\n");
        self.output
            .push_str("  (export \"_start\" (func $__restrict_start))\n");

        Ok(())
    }

    fn generate_function(&mut self, func: &FunDecl) -> Result<(), CodeGenError> {
        if !func.type_params.is_empty() {
            if matches!(func.name.as_str(), "println" | "new_list" | "list_add") {
                return self.generate_generic_function(func);
            }

            // User-defined generics are emitted only after a call site supplies
            // concrete source types. Emitting the declaration directly would
            // silently collapse type parameters to the pointer/i32 ABI.
            return Ok(());
        }

        let outer_default_arena = self.default_arena;
        let outer_record_tmp_count = self.record_tmp_count;
        self.binding_local_aliases.clear();
        self.collected_local_types.clear();
        self.local_alias_counter = 0;
        self.record_tmp_count =
            RECORD_TMP_MIN_COUNT.max(Self::max_record_tmp_depth_in_block(&func.body));
        self.current_function = Some(func.name.clone());
        let is_host_entry = self.exported_functions.contains(&func.name);
        self.push_scope();

        for (idx, param) in func.params.iter().enumerate() {
            let wasm_type = self.convert_type(&param.ty)?;
            self.add_local(&param.name, idx as u32);
            self.set_local_type(&param.name, wasm_type);
            self.set_local_source_type(&param.name, param.ty.clone());
            self.register_record_var_type(&param.name, &param.ty);
        }

        let body_expected_source = func.return_type.clone().or_else(|| {
            self.function_source_sigs
                .get(&func.name)
                .and_then(|sig| sig.result.clone())
        });

        // First, collect all local variables by analyzing the function body
        let mut locals: Vec<(String, WasmType)> = Vec::new();
        self.collect_locals_from_block_with_expected(
            &func.body,
            &mut locals,
            body_expected_source.as_ref(),
        )?;
        let locals = Self::dedupe_locals(locals)?;

        // Function header
        self.output.push_str(&format!("  (func ${}", func.name));

        // Parameters
        let mut next_idx = 0u32;
        for param in func.params.iter() {
            let wasm_type = self.convert_type(&param.ty)?;
            self.output.push_str(&format!(
                " (param ${} {})",
                param.name,
                self.wasm_type_str(wasm_type)
            ));
            next_idx += 1;
        }

        // Result type
        if let Some(sig) = self.functions.get(&func.name) {
            if let Some(result_type) = sig.result {
                self.output
                    .push_str(&format!(" (result {})", self.wasm_type_str(result_type)));
            }
        }

        self.output.push('\n');

        // Declare all local variables
        for (name, ty) in &locals {
            self.output.push_str(&format!(
                "    (local ${} {})\n",
                name,
                self.wasm_type_str(*ty)
            ));
            self.add_local(name, next_idx);
            self.set_local_type(name, *ty);
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
        self.set_local_type("match_tmp", WasmType::I32);
        next_idx += 1;

        self.output.push_str("    (local $match_tmp_i64 i64)\n");
        self.add_local("match_tmp_i64", next_idx);
        self.set_local_type("match_tmp_i64", WasmType::I64);
        next_idx += 1;

        self.output.push_str("    (local $match_tmp_f32 f32)\n");
        self.add_local("match_tmp_f32", next_idx);
        self.set_local_type("match_tmp_f32", WasmType::F32);
        next_idx += 1;

        self.output.push_str("    (local $match_tmp_f64 f64)\n");
        self.add_local("match_tmp_f64", next_idx);
        self.set_local_type("match_tmp_f64", WasmType::F64);
        next_idx += 1;

        self.output.push_str("    (local $option_value_tmp i32)\n");
        self.add_local("option_value_tmp", next_idx);
        self.set_local_type("option_value_tmp", WasmType::I32);
        next_idx += 1;

        self.output
            .push_str("    (local $option_value_i64_tmp i64)\n");
        self.add_local("option_value_i64_tmp", next_idx);
        self.set_local_type("option_value_i64_tmp", WasmType::I64);
        next_idx += 1;

        self.output
            .push_str("    (local $option_value_f32_tmp f32)\n");
        self.add_local("option_value_f32_tmp", next_idx);
        self.set_local_type("option_value_f32_tmp", WasmType::F32);
        next_idx += 1;

        self.output
            .push_str("    (local $option_value_f64_tmp f64)\n");
        self.add_local("option_value_f64_tmp", next_idx);
        self.set_local_type("option_value_f64_tmp", WasmType::F64);
        next_idx += 1;

        self.output.push_str("    (local $f64_mod_left f64)\n");
        self.add_local("f64_mod_left", next_idx);
        self.set_local_type("f64_mod_left", WasmType::F64);
        next_idx += 1;

        self.output.push_str("    (local $f64_mod_right f64)\n");
        self.add_local("f64_mod_right", next_idx);
        self.set_local_type("f64_mod_right", WasmType::F64);
        next_idx += 1;

        self.output.push_str("    (local $tail_len i32)\n");
        self.add_local("tail_len", next_idx);
        next_idx += 1;

        self.output.push_str("    (local $tail_tmp i32)\n");
        self.add_local("tail_tmp", next_idx);
        next_idx += 1;

        // Add temporary variable for resource cleanup registration
        self.output.push_str("    (local $temp_resource i32)\n");
        self.add_local("temp_resource", next_idx);
        next_idx += 1;

        // Add temporary variable for closures
        self.output.push_str("    (local $closure_tmp i32)\n");
        self.add_local("closure_tmp", next_idx);
        next_idx += 1;

        for name in [
            "iter_list",
            "iter_func",
            "iter_len",
            "iter_index",
            "iter_out",
            "iter_out_index",
            "iter_value",
            "iter_result",
            "iter_acc",
        ] {
            self.output
                .push_str(&format!("    (local ${} i32)\n", name));
            self.add_local(name, next_idx);
            next_idx += 1;
        }

        for (name, ty) in [
            ("iter_value_i64", WasmType::I64),
            ("iter_acc_i64", WasmType::I64),
            ("iter_value_f64", WasmType::F64),
            ("iter_result_f64", WasmType::F64),
            ("iter_acc_f64", WasmType::F64),
        ] {
            self.output.push_str(&format!(
                "    (local ${} {})\n",
                name,
                self.wasm_type_str(ty)
            ));
            self.add_local(name, next_idx);
            self.set_local_type(name, ty);
            next_idx += 1;
        }

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

        for depth in 0..self.record_tmp_count {
            let name = format!("record_tmp_{}", depth);
            self.output
                .push_str(&format!("    (local ${} i32)\n", name));
            self.add_local(&name, next_idx);
            next_idx += 1;
        }

        for depth in 0..WITH_ARENA_TMP_COUNT {
            let name = format!("with_prev_arena_{}", depth);
            self.output
                .push_str(&format!("    (local ${} i32)\n", name));
            self.add_local(&name, next_idx);
            next_idx += 1;
        }

        if is_host_entry {
            self.output.push_str("    (local $entry_prev_arena i32)\n");
            self.add_local("entry_prev_arena", next_idx);
            next_idx += 1;
        }

        // Hack: Add common pattern variable names (only if not already declared)
        for var_name in ["n", "x", "y", "z", "a", "b", "c", "head", "tail", "rest"] {
            // Check if this variable is already declared as a parameter or local
            let already_parameter = func.params.iter().any(|p| p.name == var_name);
            let already_local = locals.iter().any(|(name, _)| name == var_name);
            if !already_parameter && !already_local {
                self.output
                    .push_str(&format!("    (local ${} i32)\n", var_name));
                self.add_local(var_name, next_idx);
                next_idx += 1;
            }
        }

        // Initialize a default arena for host entry points. Internal helper
        // functions inherit their caller's arena unless they enter `with Arena`.
        let function_default_arena = if is_host_entry {
            let arena_addr = self.next_arena_addr;
            self.next_arena_addr += ARENA_SIZE_BYTES;
            self.default_arena = Some(arena_addr);
            Some(arena_addr)
        } else {
            None
        };
        if let Some(default_arena) = function_default_arena {
            self.output.push_str("    ;; Save caller arena\n");
            self.output.push_str("    global.get $current_arena\n");
            self.output.push_str("    local.set $entry_prev_arena\n");
            self.output.push_str("    ;; Initialize default arena\n");
            self.output
                .push_str(&format!("    i32.const {}\n", default_arena));
            self.output.push_str("    call $arena_init\n");
            self.output.push_str("    global.set $current_arena\n\n");
        }

        // Generate function body
        if let Some(return_type) = body_expected_source.as_ref() {
            self.generate_block_with_expected_source(&func.body, return_type)?;
        } else {
            self.generate_block(&func.body)?;
        }

        // Drop expression values for functions whose Wasm ABI has no result.
        let function_returns_value = self
            .functions
            .get(&func.name)
            .and_then(|sig| sig.result)
            .is_some();
        if !function_returns_value && func.body.expr.is_some() {
            if let Some(expr) = &func.body.expr {
                if self.expr_leaves_value(expr) {
                    self.output.push_str("    drop\n");
                }
            }
        }

        // Reset entry-point default arena before returning to the host.
        if let Some(default_arena) = function_default_arena {
            self.output.push_str("\n    ;; Reset default arena\n");
            self.output
                .push_str(&format!("    i32.const {}\n", default_arena));
            self.output.push_str("    call $arena_reset\n");
            self.output.push_str("    local.get $entry_prev_arena\n");
            self.output.push_str("    global.set $current_arena\n");
        }

        self.output.push_str("  )\n");

        self.pop_scope();
        self.default_arena = outer_default_arena;
        self.record_tmp_count = outer_record_tmp_count;
        self.current_function = None;
        Ok(())
    }

    fn generate_block(&mut self, block: &BlockExpr) -> Result<(), CodeGenError> {
        self.generate_block_internal(block, false, None)
    }

    fn dedupe_locals(
        locals: Vec<(String, WasmType)>,
    ) -> Result<Vec<(String, WasmType)>, CodeGenError> {
        let mut deduped = Vec::new();
        let mut seen = HashMap::new();

        for (name, ty) in locals {
            if let Some(existing_ty) = seen.get(&name) {
                if existing_ty != &ty {
                    return Err(CodeGenError::UnsupportedFeature(format!(
                        "local '{}' is bound with incompatible Wasm types",
                        name
                    )));
                }
                continue;
            }

            seen.insert(name.clone(), ty);
            deduped.push((name, ty));
        }

        Ok(deduped)
    }

    fn generate_block_as_expression(&mut self, block: &BlockExpr) -> Result<(), CodeGenError> {
        self.generate_block_internal(block, true, None)
    }

    fn generate_block_with_expected_source(
        &mut self,
        block: &BlockExpr,
        expected_source: &Type,
    ) -> Result<(), CodeGenError> {
        self.generate_block_internal(block, false, Some(expected_source))
    }

    fn generate_block_internal(
        &mut self,
        block: &BlockExpr,
        as_expression: bool,
        expected_source: Option<&Type>,
    ) -> Result<(), CodeGenError> {
        // Generate statements
        for (i, stmt) in block.statements.iter().enumerate() {
            let is_last_stmt = i == block.statements.len() - 1;
            match stmt {
                Stmt::Binding(bind) => self.generate_binding_with_later_array_context(
                    bind,
                    &block.statements[i + 1..],
                    block.expr.as_deref(),
                    expected_source,
                )?,
                Stmt::Assignment(assign) => self.generate_assignment(assign)?,
                Stmt::Expr(expr) => {
                    if block.expr.is_none() && is_last_stmt {
                        if let Some(expected_source) = expected_source {
                            self.generate_expr_with_expected_source(expr, expected_source)?;
                        } else {
                            self.generate_expr(expr)?;
                        }
                    } else {
                        self.generate_expr(expr)?;
                    }
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
            if let Some(expected_source) = expected_source {
                self.generate_expr_with_expected_source(expr, expected_source)?;
            } else {
                self.generate_expr(expr)?;
            }
        } else if as_expression {
            let final_stmt_leaves_value = matches!(
                block.statements.last(),
                Some(Stmt::Expr(expr)) if self.expr_leaves_value(expr)
            );
            if !final_stmt_leaves_value {
                self.output.push_str("    i32.const 0\n");
            }
        } else if block.statements.is_empty() && !as_expression {
            // Empty block returns 0 (Unit) only in statement context
            self.output.push_str("    i32.const 0\n");
        }

        Ok(())
    }

    fn generate_binding_with_later_array_context(
        &mut self,
        bind: &BindDecl,
        later_statements: &[Stmt],
        final_expr: Option<&Expr>,
        expected_source: Option<&Type>,
    ) -> Result<(), CodeGenError> {
        if bind.type_annotation.is_none() {
            if let Pattern::Ident(name) = &bind.pattern {
                if let Some(source_ty) = self
                    .infer_unannotated_binding_source_type_from_later_context(
                        name,
                        &bind.value,
                        later_statements,
                        final_expr,
                        expected_source,
                    )
                {
                    if let Some(local_name) = self
                        .binding_local_aliases
                        .get(&Self::binding_id(bind))
                        .cloned()
                    {
                        self.set_local_source_type(&local_name, source_ty.clone());
                    }
                    self.set_local_source_type(name, source_ty);
                }
            }
        }

        self.generate_binding(bind)
    }

    fn expected_source_for_returned_binding(
        &self,
        name: &str,
        final_expr: Option<&Expr>,
        expected_source: Option<&Type>,
    ) -> Option<Type> {
        match (final_expr, expected_source) {
            (Some(Expr::Ident(returned_name)), Some(source_ty)) if returned_name == name => {
                Some(source_ty.clone())
            }
            _ => None,
        }
    }

    fn infer_unannotated_binding_source_type_from_later_context(
        &self,
        name: &str,
        value: &Expr,
        later_statements: &[Stmt],
        final_expr: Option<&Expr>,
        expected_source: Option<&Type>,
    ) -> Option<Type> {
        self.expected_source_for_returned_binding(name, final_expr, expected_source)
            .or_else(|| {
                self.infer_unannotated_record_binding_source_type_from_later_context(
                    name,
                    value,
                    later_statements,
                    final_expr,
                    expected_source,
                )
            })
            .or_else(|| {
                self.infer_unannotated_container_binding_source_type_from_later_iteration_use(
                    name,
                    value,
                    later_statements,
                    final_expr,
                )
            })
            .or_else(|| {
                self.infer_unannotated_list_binding_source_type_from_later_array_use(
                    name,
                    value,
                    later_statements,
                    final_expr,
                )
            })
    }

    fn infer_unannotated_container_binding_source_type_from_later_iteration_use(
        &self,
        name: &str,
        value: &Expr,
        later_statements: &[Stmt],
        final_expr: Option<&Expr>,
    ) -> Option<Type> {
        let container_name = match value {
            Expr::ListLit(_) => "List",
            Expr::None | Expr::Some(_) => "Option",
            _ => return None,
        };

        for stmt in later_statements {
            if let Some(item_ty) =
                self.find_iteration_item_context_for_ident_in_stmt(name, container_name, stmt)
            {
                return Some(Type::Generic(container_name.to_string(), vec![item_ty]));
            }
        }

        if let Some(expr) = final_expr {
            if let Some(item_ty) =
                self.find_iteration_item_context_for_ident_in_expr(name, container_name, expr)
            {
                return Some(Type::Generic(container_name.to_string(), vec![item_ty]));
            }
        }

        None
    }

    fn find_iteration_item_context_for_ident_in_stmt(
        &self,
        name: &str,
        container_name: &str,
        stmt: &Stmt,
    ) -> Option<Type> {
        match stmt {
            Stmt::Binding(bind) => {
                let found = self.find_iteration_item_context_for_ident_in_expr(
                    name,
                    container_name,
                    &bind.value,
                );
                if Self::pattern_binds_name(&bind.pattern, name) {
                    None
                } else {
                    found
                }
            }
            Stmt::Expr(expr) => {
                self.find_iteration_item_context_for_ident_in_expr(name, container_name, expr)
            }
            Stmt::Assignment(assign) => self.find_iteration_item_context_for_ident_in_expr(
                name,
                container_name,
                &assign.value,
            ),
        }
    }

    fn find_iteration_item_context_for_ident_in_expr(
        &self,
        name: &str,
        container_name: &str,
        expr: &Expr,
    ) -> Option<Type> {
        match expr {
            Expr::Call(call) => {
                if self.call_first_arg_is_ident(call, name) {
                    if let Expr::Ident(function_name) = call.function.as_ref() {
                        if let Some(item_ty) = self.iteration_item_context_from_call(
                            function_name,
                            container_name,
                            &call.args,
                        ) {
                            return Some(item_ty);
                        }
                    }
                }

                call.args
                    .iter()
                    .find_map(|arg| {
                        self.find_iteration_item_context_for_ident_in_expr(
                            name,
                            container_name,
                            arg,
                        )
                    })
                    .or_else(|| {
                        self.find_iteration_item_context_for_ident_in_expr(
                            name,
                            container_name,
                            &call.function,
                        )
                    })
            }
            Expr::Pipe(pipe) => {
                if Self::expr_is_ident(&pipe.expr, name) {
                    if let Some(item_ty) =
                        self.iteration_item_context_from_pipe_target(container_name, &pipe.target)
                    {
                        return Some(item_ty);
                    }
                }

                self.find_iteration_item_context_for_ident_in_expr(name, container_name, &pipe.expr)
                    .or_else(|| match &pipe.target {
                        PipeTarget::Expr(target) => self
                            .find_iteration_item_context_for_ident_in_expr(
                                name,
                                container_name,
                                target,
                            ),
                        PipeTarget::Ident(_) => None,
                    })
            }
            Expr::Block(block) => {
                self.find_iteration_item_context_for_ident_in_block(name, container_name, block)
            }
            Expr::Then(then) => self
                .find_iteration_item_context_for_ident_in_block(
                    name,
                    container_name,
                    &then.then_block,
                )
                .or_else(|| {
                    then.else_ifs.iter().find_map(|(_, block)| {
                        self.find_iteration_item_context_for_ident_in_block(
                            name,
                            container_name,
                            block,
                        )
                    })
                })
                .or_else(|| {
                    then.else_block.as_ref().and_then(|block| {
                        self.find_iteration_item_context_for_ident_in_block(
                            name,
                            container_name,
                            block,
                        )
                    })
                }),
            Expr::Match(match_expr) => self
                .find_iteration_item_context_for_ident_in_expr(
                    name,
                    container_name,
                    &match_expr.expr,
                )
                .or_else(|| {
                    match_expr.arms.iter().find_map(|arm| {
                        self.find_iteration_item_context_for_ident_in_block(
                            name,
                            container_name,
                            &arm.body,
                        )
                    })
                }),
            Expr::FieldAccess(object, _) => {
                self.find_iteration_item_context_for_ident_in_expr(name, container_name, object)
            }
            Expr::Some(inner) | Expr::Ok(inner) | Expr::Err(inner) | Expr::Freeze(inner) => {
                self.find_iteration_item_context_for_ident_in_expr(name, container_name, inner)
            }
            Expr::Unary(unary) => self.find_iteration_item_context_for_ident_in_expr(
                name,
                container_name,
                &unary.expr,
            ),
            Expr::Binary(binary) => self
                .find_iteration_item_context_for_ident_in_expr(name, container_name, &binary.left)
                .or_else(|| {
                    self.find_iteration_item_context_for_ident_in_expr(
                        name,
                        container_name,
                        &binary.right,
                    )
                }),
            Expr::Cast(cast) => {
                self.find_iteration_item_context_for_ident_in_expr(name, container_name, &cast.expr)
            }
            Expr::ListLit(items) | Expr::ArrayLit(items) => items.iter().find_map(|item| {
                self.find_iteration_item_context_for_ident_in_expr(name, container_name, item)
            }),
            _ => None,
        }
    }

    fn find_iteration_item_context_for_ident_in_block(
        &self,
        name: &str,
        container_name: &str,
        block: &BlockExpr,
    ) -> Option<Type> {
        for stmt in &block.statements {
            if let Some(item_ty) =
                self.find_iteration_item_context_for_ident_in_stmt(name, container_name, stmt)
            {
                return Some(item_ty);
            }
        }

        block.expr.as_ref().and_then(|expr| {
            self.find_iteration_item_context_for_ident_in_expr(name, container_name, expr)
        })
    }

    fn call_first_arg_is_ident(&self, call: &CallExpr, name: &str) -> bool {
        call.args
            .first()
            .is_some_and(|arg| Self::expr_is_ident(arg, name))
    }

    fn iteration_item_context_from_pipe_target(
        &self,
        container_name: &str,
        target: &PipeTarget,
    ) -> Option<Type> {
        match target {
            PipeTarget::Ident(function_name) => {
                self.iteration_item_context_from_call(function_name, container_name, &[])
            }
            PipeTarget::Expr(expr) => match expr.as_ref() {
                Expr::Ident(function_name) => {
                    self.iteration_item_context_from_call(function_name, container_name, &[])
                }
                Expr::FieldAccess(_, _) => self
                    .infer_expr_source_type(expr)
                    .and_then(|source_ty| match source_ty {
                        Type::Function(params, _) => params.first().cloned(),
                        _ => None,
                    })
                    .and_then(|param_ty| {
                        Self::container_item_from_source_type(&param_ty, container_name)
                    }),
                _ => None,
            },
        }
    }

    fn iteration_item_context_from_call(
        &self,
        function_name: &str,
        container_name: &str,
        args: &[Box<Expr>],
    ) -> Option<Type> {
        match function_name {
            "map" | "filter" if args.len() >= 2 => self.callable_param_source_type(&args[1], 0),
            "fold" if container_name == "List" && args.len() >= 3 => {
                self.callable_param_source_type(&args[2], 1)
            }
            _ => None,
        }
    }

    fn callable_param_source_type(&self, callable: &Expr, index: usize) -> Option<Type> {
        match callable {
            Expr::Ident(name) => {
                if let Some(Type::Function(params, _)) = self.lookup_local_source_type(name) {
                    return params.get(index).cloned();
                }

                let function_name = self
                    .lookup_generic_function_alias(name)
                    .unwrap_or_else(|| name.clone());
                self.function_source_sigs
                    .get(&function_name)
                    .and_then(|sig| {
                        if sig.type_params.is_empty() {
                            sig.params.get(index).cloned()
                        } else {
                            None
                        }
                    })
            }
            Expr::Lambda(lambda) => lambda.params.get(index).and_then(|param| {
                param
                    .type_annotation
                    .clone()
                    .or_else(|| self.infer_lambda_param_source_type_from_body(lambda, index))
            }),
            _ => match self.infer_expr_source_type(callable) {
                Some(Type::Function(params, _)) => params.get(index).cloned(),
                _ => None,
            },
        }
    }

    fn infer_lambda_param_source_type_from_body(
        &self,
        lambda: &LambdaExpr,
        index: usize,
    ) -> Option<Type> {
        let param = lambda.params.get(index)?;
        self.expected_source_for_ident_in_expr(&param.name, &lambda.body, None)
            .or_else(|| self.infer_ident_source_type_from_expr_usage(&param.name, &lambda.body))
    }

    fn infer_ident_source_type_from_expr_usage(&self, name: &str, expr: &Expr) -> Option<Type> {
        match expr {
            Expr::Binary(binary) => {
                if Self::expr_is_ident(&binary.left, name) {
                    if let Some(other_ty) = self.infer_expr_source_type(&binary.right) {
                        if let Some(ty) =
                            Self::contextual_binary_operand_source_type(&binary.op, &other_ty)
                        {
                            return Some(ty);
                        }
                    }
                }
                if Self::expr_is_ident(&binary.right, name) {
                    if let Some(other_ty) = self.infer_expr_source_type(&binary.left) {
                        if let Some(ty) =
                            Self::contextual_binary_operand_source_type(&binary.op, &other_ty)
                        {
                            return Some(ty);
                        }
                    }
                }

                self.infer_ident_source_type_from_expr_usage(name, &binary.left)
                    .or_else(|| self.infer_ident_source_type_from_expr_usage(name, &binary.right))
            }
            Expr::Unary(unary) => {
                if Self::expr_is_ident(&unary.expr, name) {
                    return match unary.op {
                        UnaryOp::Not => Some(Type::Named("Boolean".to_string())),
                        UnaryOp::Neg => None,
                    };
                }
                self.infer_ident_source_type_from_expr_usage(name, &unary.expr)
            }
            Expr::Cast(cast) => {
                if Self::expr_is_ident(&cast.expr, name) {
                    Some(cast.target.clone())
                } else {
                    self.infer_ident_source_type_from_expr_usage(name, &cast.expr)
                }
            }
            Expr::Call(call) => self
                .expected_source_for_ident_in_expr(name, expr, None)
                .or_else(|| {
                    call.args
                        .iter()
                        .find_map(|arg| self.infer_ident_source_type_from_expr_usage(name, arg))
                })
                .or_else(|| self.infer_ident_source_type_from_expr_usage(name, &call.function)),
            Expr::Pipe(pipe) => self
                .expected_source_for_ident_in_expr(name, expr, None)
                .or_else(|| self.infer_ident_source_type_from_expr_usage(name, &pipe.expr))
                .or_else(|| match &pipe.target {
                    PipeTarget::Expr(target) => {
                        self.infer_ident_source_type_from_expr_usage(name, target)
                    }
                    PipeTarget::Ident(_) => None,
                }),
            Expr::Block(block) => self.infer_ident_source_type_from_block_usage(name, block),
            Expr::Then(then) => self
                .infer_ident_source_type_from_expr_usage(name, &then.condition)
                .or_else(|| self.infer_ident_source_type_from_block_usage(name, &then.then_block))
                .or_else(|| {
                    then.else_ifs.iter().find_map(|(condition, block)| {
                        self.infer_ident_source_type_from_expr_usage(name, condition)
                            .or_else(|| self.infer_ident_source_type_from_block_usage(name, block))
                    })
                })
                .or_else(|| {
                    then.else_block.as_ref().and_then(|block| {
                        self.infer_ident_source_type_from_block_usage(name, block)
                    })
                }),
            Expr::While(while_expr) => self
                .infer_ident_source_type_from_expr_usage(name, &while_expr.condition)
                .or_else(|| self.infer_ident_source_type_from_block_usage(name, &while_expr.body)),
            Expr::Match(match_expr) => self
                .infer_ident_source_type_from_expr_usage(name, &match_expr.expr)
                .or_else(|| {
                    match_expr.arms.iter().find_map(|arm| {
                        if Self::pattern_binds_name(&arm.pattern, name) {
                            None
                        } else {
                            self.infer_ident_source_type_from_block_usage(name, &arm.body)
                        }
                    })
                }),
            Expr::With(with_expr) => {
                for binding in &with_expr.bindings {
                    match binding {
                        FieldInit::Field {
                            name: field_name,
                            value,
                        } => {
                            if let Some(ty) =
                                self.infer_ident_source_type_from_expr_usage(name, value)
                            {
                                return Some(ty);
                            }
                            if field_name == name {
                                return None;
                            }
                        }
                        FieldInit::Spread(value) => {
                            if let Some(ty) =
                                self.infer_ident_source_type_from_expr_usage(name, value)
                            {
                                return Some(ty);
                            }
                        }
                    }
                }
                self.infer_ident_source_type_from_block_usage(name, &with_expr.body)
            }
            Expr::WithLifetime(with_lifetime) => {
                self.infer_ident_source_type_from_block_usage(name, &with_lifetime.body)
            }
            Expr::FieldAccess(object, _) => {
                self.infer_ident_source_type_from_expr_usage(name, object)
            }
            Expr::Freeze(inner)
            | Expr::Some(inner)
            | Expr::Ok(inner)
            | Expr::Err(inner)
            | Expr::Await(inner)
            | Expr::Spawn(inner) => self.infer_ident_source_type_from_expr_usage(name, inner),
            Expr::ListLit(items) | Expr::ArrayLit(items) => items
                .iter()
                .find_map(|item| self.infer_ident_source_type_from_expr_usage(name, item)),
            Expr::RangeLit(range) => self
                .infer_ident_source_type_from_expr_usage(name, &range.start)
                .or_else(|| self.infer_ident_source_type_from_expr_usage(name, &range.end)),
            Expr::RecordLit(record) => record.fields.iter().find_map(|field| {
                let value = match field {
                    FieldInit::Field { value, .. } | FieldInit::Spread(value) => value,
                };
                self.infer_ident_source_type_from_expr_usage(name, value)
            }),
            Expr::Clone(clone) => self
                .infer_ident_source_type_from_expr_usage(name, &clone.base)
                .or_else(|| {
                    clone.updates.fields.iter().find_map(|field| {
                        let value = match field {
                            FieldInit::Field { value, .. } | FieldInit::Spread(value) => value,
                        };
                        self.infer_ident_source_type_from_expr_usage(name, value)
                    })
                }),
            Expr::PrototypeClone(proto_clone) => {
                proto_clone.updates.fields.iter().find_map(|field| {
                    let value = match field {
                        FieldInit::Field { value, .. } | FieldInit::Spread(value) => value,
                    };
                    self.infer_ident_source_type_from_expr_usage(name, value)
                })
            }
            Expr::Lambda(lambda) => {
                if lambda.params.iter().any(|param| param.name == name) {
                    None
                } else {
                    self.infer_ident_source_type_from_expr_usage(name, &lambda.body)
                }
            }
            Expr::IntLit(_)
            | Expr::FloatLit(_)
            | Expr::StringLit(_)
            | Expr::CharLit(_)
            | Expr::BoolLit(_)
            | Expr::Unit
            | Expr::Ident(_)
            | Expr::None => None,
        }
    }

    fn infer_ident_source_type_from_block_usage(
        &self,
        name: &str,
        block: &BlockExpr,
    ) -> Option<Type> {
        for stmt in &block.statements {
            match stmt {
                Stmt::Binding(bind) => {
                    if let Some(ty) =
                        self.infer_ident_source_type_from_expr_usage(name, &bind.value)
                    {
                        return Some(ty);
                    }
                    if Self::pattern_binds_name(&bind.pattern, name) {
                        return None;
                    }
                }
                Stmt::Assignment(assign) => {
                    if let Some(ty) =
                        self.infer_ident_source_type_from_expr_usage(name, &assign.value)
                    {
                        return Some(ty);
                    }
                }
                Stmt::Expr(expr) => {
                    if let Some(ty) = self.infer_ident_source_type_from_expr_usage(name, expr) {
                        return Some(ty);
                    }
                }
            }
        }

        block
            .expr
            .as_ref()
            .and_then(|expr| self.infer_ident_source_type_from_expr_usage(name, expr))
    }

    fn contextual_binary_operand_source_type(op: &BinaryOp, other: &Type) -> Option<Type> {
        let named = match other {
            Type::Named(name) => name.as_str(),
            _ => return None,
        };

        match op {
            BinaryOp::Add => match named {
                "Int32" | "Int64" | "Float64" | "String" => Some(other.clone()),
                _ => None,
            },
            BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => match named {
                "Int32" | "Int64" | "Float64" => Some(other.clone()),
                _ => None,
            },
            BinaryOp::Eq | BinaryOp::Ne => match named {
                "Int32" | "Int64" | "Float64" | "Boolean" | "Char" => Some(other.clone()),
                _ => None,
            },
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => match named {
                "Int32" | "Int64" | "Float64" => Some(other.clone()),
                _ => None,
            },
            BinaryOp::And | BinaryOp::Or => match named {
                "Boolean" => Some(other.clone()),
                _ => None,
            },
        }
    }

    fn infer_unannotated_record_binding_source_type_from_later_context(
        &self,
        name: &str,
        value: &Expr,
        later_statements: &[Stmt],
        final_expr: Option<&Expr>,
        expected_source: Option<&Type>,
    ) -> Option<Type> {
        let Expr::RecordLit(record_lit) = value else {
            return None;
        };

        let type_params = self.record_type_params.get(&record_lit.name)?;
        if type_params.is_empty() {
            return Some(Type::Named(record_lit.name.clone()));
        }

        let mut substitution = HashMap::new();
        for field in &record_lit.fields {
            let FieldInit::Field {
                name: field_name,
                value,
            } = field
            else {
                continue;
            };
            let Some(value_ty) = self.infer_expr_source_type(value) else {
                continue;
            };
            let Some(field_ty) = self.record_field_type(&record_lit.name, field_name) else {
                continue;
            };
            Self::bind_source_type_params(field_ty, &value_ty, type_params, &mut substitution);
        }

        for stmt in later_statements {
            if let Stmt::Expr(expr) = stmt {
                self.bind_record_binding_type_params_from_expr(
                    name,
                    &record_lit.name,
                    expr,
                    expected_source,
                    type_params,
                    &mut substitution,
                );
            }
        }

        if let Some(expr) = final_expr {
            self.bind_record_binding_type_params_from_expr(
                name,
                &record_lit.name,
                expr,
                expected_source,
                type_params,
                &mut substitution,
            );
        }

        let args = type_params
            .iter()
            .map(|param| substitution.get(param).cloned())
            .collect::<Option<Vec<_>>>()?;
        Some(Type::Generic(record_lit.name.clone(), args))
    }

    fn bind_record_binding_type_params_from_expr(
        &self,
        binding_name: &str,
        record_name: &str,
        expr: &Expr,
        expected_source: Option<&Type>,
        type_params: &[String],
        substitution: &mut HashMap<String, Type>,
    ) {
        match expr {
            Expr::FieldAccess(object, field) => {
                if Self::expr_is_ident(object, binding_name) {
                    if let Some(field_ty) = expected_source {
                        self.bind_record_binding_field_context(
                            record_name,
                            field,
                            field_ty,
                            type_params,
                            substitution,
                        );
                    }
                }
            }
            Expr::Match(match_expr) => {
                if let Expr::FieldAccess(object, field) = match_expr.expr.as_ref() {
                    if Self::expr_is_ident(object, binding_name) {
                        self.bind_record_binding_field_match_context(
                            record_name,
                            field,
                            match_expr,
                            expected_source,
                            type_params,
                            substitution,
                        );
                    }
                }
            }
            Expr::Call(call) => {
                if let Expr::FieldAccess(object, field) = call.function.as_ref() {
                    if Self::expr_is_ident(object, binding_name) {
                        self.bind_record_binding_field_callable_call_context(
                            record_name,
                            field,
                            &call.args,
                            expected_source,
                            type_params,
                            substitution,
                        );
                    }
                }

                for arg in &call.args {
                    self.bind_record_binding_type_params_from_expr(
                        binding_name,
                        record_name,
                        arg,
                        expected_source,
                        type_params,
                        substitution,
                    );
                }

                self.bind_record_binding_type_params_from_expr(
                    binding_name,
                    record_name,
                    &call.function,
                    expected_source,
                    type_params,
                    substitution,
                );
            }
            Expr::Then(then) => {
                self.bind_record_binding_type_params_from_block(
                    binding_name,
                    record_name,
                    &then.then_block,
                    expected_source,
                    type_params,
                    substitution,
                );
                for (_, block) in &then.else_ifs {
                    self.bind_record_binding_type_params_from_block(
                        binding_name,
                        record_name,
                        block,
                        expected_source,
                        type_params,
                        substitution,
                    );
                }
                if let Some(block) = &then.else_block {
                    self.bind_record_binding_type_params_from_block(
                        binding_name,
                        record_name,
                        block,
                        expected_source,
                        type_params,
                        substitution,
                    );
                }
            }
            Expr::Pipe(pipe) => {
                if let PipeTarget::Expr(target) = &pipe.target {
                    if let Expr::FieldAccess(object, field) = target.as_ref() {
                        if Self::expr_is_ident(object, binding_name) {
                            self.bind_record_binding_field_callable_pipe_context(
                                record_name,
                                field,
                                &pipe.expr,
                                expected_source,
                                type_params,
                                substitution,
                            );
                        }
                    }
                }

                self.bind_record_binding_type_params_from_expr(
                    binding_name,
                    record_name,
                    &pipe.expr,
                    expected_source,
                    type_params,
                    substitution,
                );

                if let PipeTarget::Expr(target) = &pipe.target {
                    self.bind_record_binding_type_params_from_expr(
                        binding_name,
                        record_name,
                        target,
                        expected_source,
                        type_params,
                        substitution,
                    );
                }
            }
            Expr::Block(block) => {
                self.bind_record_binding_type_params_from_block(
                    binding_name,
                    record_name,
                    block,
                    expected_source,
                    type_params,
                    substitution,
                );
            }
            _ => {}
        }
    }

    fn bind_record_binding_type_params_from_block(
        &self,
        binding_name: &str,
        record_name: &str,
        block: &BlockExpr,
        expected_source: Option<&Type>,
        type_params: &[String],
        substitution: &mut HashMap<String, Type>,
    ) {
        if let Some(expr) = &block.expr {
            self.bind_record_binding_type_params_from_expr(
                binding_name,
                record_name,
                expr,
                expected_source,
                type_params,
                substitution,
            );
        } else if let Some(Stmt::Expr(expr)) = block.statements.last() {
            self.bind_record_binding_type_params_from_expr(
                binding_name,
                record_name,
                expr,
                expected_source,
                type_params,
                substitution,
            );
        }
    }

    fn bind_record_binding_field_callable_call_context(
        &self,
        record_name: &str,
        field: &str,
        args: &[Box<Expr>],
        expected_result_source: Option<&Type>,
        type_params: &[String],
        substitution: &mut HashMap<String, Type>,
    ) {
        let Some(Type::Function(params, return_ty)) = self.record_field_type(record_name, field)
        else {
            return;
        };

        for (param_ty, arg) in params.iter().zip(args.iter()) {
            if let Some(arg_ty) = self.infer_expr_source_type(arg) {
                Self::bind_source_type_params(param_ty, &arg_ty, type_params, substitution);
            }
        }

        if let Some(expected_result_source) = expected_result_source {
            Self::bind_source_type_params(
                return_ty,
                expected_result_source,
                type_params,
                substitution,
            );
        }
    }

    fn bind_record_binding_field_context(
        &self,
        record_name: &str,
        field: &str,
        field_ty: &Type,
        type_params: &[String],
        substitution: &mut HashMap<String, Type>,
    ) {
        if let Some(field_template) = self.record_field_type(record_name, field) {
            Self::bind_source_type_params(field_template, field_ty, type_params, substitution);
        }
    }

    fn bind_record_binding_field_callable_pipe_context(
        &self,
        record_name: &str,
        field: &str,
        pipe_arg: &Expr,
        expected_result_source: Option<&Type>,
        type_params: &[String],
        substitution: &mut HashMap<String, Type>,
    ) {
        let Some(Type::Function(params, return_ty)) = self.record_field_type(record_name, field)
        else {
            return;
        };

        if let Some((param_ty, arg_ty)) = params
            .first()
            .zip(self.infer_expr_source_type(pipe_arg).as_ref())
        {
            Self::bind_source_type_params(param_ty, arg_ty, type_params, substitution);
        }

        if let Some(expected_result_source) = expected_result_source {
            Self::bind_source_type_params(
                return_ty,
                expected_result_source,
                type_params,
                substitution,
            );
        }
    }

    fn bind_record_binding_field_match_context(
        &self,
        record_name: &str,
        field: &str,
        match_expr: &MatchExpr,
        expected_source: Option<&Type>,
        type_params: &[String],
        substitution: &mut HashMap<String, Type>,
    ) {
        let Some(expected_source) = expected_source else {
            return;
        };
        let Some(field_template) = self.record_field_type(record_name, field) else {
            return;
        };
        let mut context = VariantPayloadBindContext {
            field_template,
            expected_source,
            type_params,
            substitution,
        };

        for arm in &match_expr.arms {
            match &arm.pattern {
                Pattern::Some(inner) => {
                    self.bind_variant_payload_from_match_arm(
                        &mut context,
                        "Option",
                        0,
                        inner,
                        &arm.body,
                    );
                }
                Pattern::Ok(inner) => {
                    self.bind_variant_payload_from_match_arm(
                        &mut context,
                        "Result",
                        0,
                        inner,
                        &arm.body,
                    );
                }
                Pattern::Err(inner) => {
                    self.bind_variant_payload_from_match_arm(
                        &mut context,
                        "Result",
                        1,
                        inner,
                        &arm.body,
                    );
                }
                _ => {}
            }
        }
    }

    fn bind_variant_payload_from_match_arm(
        &self,
        context: &mut VariantPayloadBindContext<'_>,
        variant_type_name: &str,
        payload_index: usize,
        payload_pattern: &Pattern,
        body: &BlockExpr,
    ) {
        let Pattern::Ident(name) = payload_pattern else {
            return;
        };
        let Some(payload_expected_source) =
            self.expected_source_for_ident_in_block(name, body, Some(context.expected_source))
        else {
            return;
        };
        let Type::Generic(name, args) = context.field_template else {
            return;
        };
        if name != variant_type_name {
            return;
        }
        let Some(payload_template) = args.get(payload_index) else {
            return;
        };
        Self::bind_source_type_params(
            payload_template,
            &payload_expected_source,
            context.type_params,
            context.substitution,
        );
    }

    fn expr_is_ident(expr: &Expr, name: &str) -> bool {
        matches!(expr, Expr::Ident(candidate) if candidate == name)
    }

    fn expected_source_for_ident_in_block(
        &self,
        name: &str,
        block: &BlockExpr,
        expected_source: Option<&Type>,
    ) -> Option<Type> {
        if let Some(expr) = block.expr.as_deref() {
            return self.expected_source_for_ident_in_expr(name, expr, expected_source);
        }

        match block.statements.last() {
            Some(Stmt::Expr(expr)) => {
                self.expected_source_for_ident_in_expr(name, expr, expected_source)
            }
            _ => None,
        }
    }

    fn expected_source_for_ident_in_expr(
        &self,
        name: &str,
        expr: &Expr,
        expected_source: Option<&Type>,
    ) -> Option<Type> {
        if Self::expr_is_ident(expr, name) {
            return expected_source.cloned();
        }

        match expr {
            Expr::Pipe(pipe) if Self::expr_is_ident(&pipe.expr, name) => {
                self.expected_source_for_pipe_target_first_arg(&pipe.target)
            }
            Expr::Pipe(pipe) => match &pipe.target {
                PipeTarget::Ident(target_name) if target_name == name => {
                    let arg_ty = self.infer_expr_source_type(&pipe.expr)?;
                    let return_ty = expected_source.cloned().or_else(|| {
                        self.infer_expr_source_type_with_bindings(expr, &HashMap::new())
                    })?;
                    Some(Type::Function(vec![arg_ty], Box::new(return_ty)))
                }
                PipeTarget::Expr(target) if Self::expr_is_ident(target, name) => {
                    let arg_ty = self.infer_expr_source_type(&pipe.expr)?;
                    let return_ty = expected_source.cloned().or_else(|| {
                        self.infer_expr_source_type_with_bindings(expr, &HashMap::new())
                    })?;
                    Some(Type::Function(vec![arg_ty], Box::new(return_ty)))
                }
                PipeTarget::Expr(target) => {
                    self.expected_source_for_ident_in_expr(name, target, expected_source)
                }
                PipeTarget::Ident(_) => None,
            },
            Expr::Call(call) => {
                if let Expr::Ident(func_name) = call.function.as_ref() {
                    if func_name == name {
                        let arg_tys = call
                            .args
                            .iter()
                            .map(|arg| self.infer_expr_source_type(arg))
                            .collect::<Option<Vec<_>>>()?;
                        let return_ty = expected_source.cloned().or_else(|| {
                            self.infer_expr_source_type_with_bindings(expr, &HashMap::new())
                        })?;
                        return Some(Type::Function(arg_tys, Box::new(return_ty)));
                    }

                    for (index, arg) in call.args.iter().enumerate() {
                        if Self::expr_is_ident(arg, name) {
                            return self.expected_function_param_source_type(func_name, index);
                        }
                    }
                }
                None
            }
            Expr::Block(block) => {
                self.expected_source_for_ident_in_block(name, block, expected_source)
            }
            Expr::Then(then) => self
                .expected_source_for_ident_in_block(name, &then.then_block, expected_source)
                .or_else(|| {
                    then.else_ifs.iter().find_map(|(_, block)| {
                        self.expected_source_for_ident_in_block(name, block, expected_source)
                    })
                })
                .or_else(|| {
                    then.else_block.as_ref().and_then(|block| {
                        self.expected_source_for_ident_in_block(name, block, expected_source)
                    })
                }),
            _ => None,
        }
    }

    fn expected_source_for_pipe_target_first_arg(&self, target: &PipeTarget) -> Option<Type> {
        match target {
            PipeTarget::Ident(func_name) => self.expected_function_param_source_type(func_name, 0),
            PipeTarget::Expr(expr) => match expr.as_ref() {
                Expr::Ident(func_name) => self.expected_function_param_source_type(func_name, 0),
                _ => None,
            },
        }
    }

    fn expected_function_param_source_type(&self, func_name: &str, index: usize) -> Option<Type> {
        let sig = self.function_source_sigs.get(func_name)?;
        if !sig.type_params.is_empty() {
            return None;
        }
        sig.params.get(index).cloned()
    }

    fn generate_binding(&mut self, bind: &BindDecl) -> Result<(), CodeGenError> {
        // For now, only handle simple identifier patterns
        // Full pattern support (including destructuring) is TODO
        match &bind.pattern {
            Pattern::Ident(name) => {
                let local_name = self
                    .binding_local_aliases
                    .get(&Self::binding_id(bind))
                    .cloned();
                if let Some(local_name) = &local_name {
                    self.set_local_alias(name, local_name.clone());
                }
                let storage_name = local_name.as_deref().unwrap_or(name);
                let binding_source_ty = bind
                    .type_annotation
                    .clone()
                    .or_else(|| self.lookup_local_source_type(storage_name))
                    .or_else(|| self.lookup_local_source_type(name))
                    .or_else(|| self.infer_expr_source_type(&bind.value));
                let generic_function_alias = if bind.type_annotation.is_none() {
                    self.generic_function_alias_target(&bind.value)
                } else {
                    None
                };
                let deferred_lambda_alias = if bind.type_annotation.is_none()
                    && binding_source_ty.is_none()
                    && generic_function_alias.is_none()
                    && self.is_deferred_callable_expr(&bind.value)
                {
                    Some((*bind.value).clone())
                } else {
                    None
                };

                // Infer type of the value for variable tracking
                if let Some(annotation) = &bind.type_annotation {
                    self.set_local_source_type(storage_name, annotation.clone());
                    self.set_local_source_type(name, annotation.clone());
                    self.register_record_var_type(name, annotation);
                } else if let Some(ty) = &binding_source_ty {
                    self.register_record_var_type(name, ty);
                    self.set_local_source_type(storage_name, ty.clone());
                    self.set_local_source_type(name, ty.clone());
                }

                // Generate the value expression. Function annotations provide
                // the runtime ABI for zero-argument function values, which are
                // otherwise parsed as shorthand calls.
                if let Some(annotation) = &bind.type_annotation {
                    self.clear_generic_function_alias(name);
                    self.clear_deferred_lambda_alias(name);
                    self.generate_expr_with_expected_source(&bind.value, annotation)?;
                } else if let Some(function_name) = generic_function_alias.as_ref() {
                    self.clear_deferred_lambda_alias(name);
                    self.set_generic_function_alias(name, function_name.clone());
                    self.output.push_str(&format!(
                        "    i32.const 0 ;; generic function alias for {}\n",
                        function_name
                    ));
                } else if let Some(callable) = deferred_lambda_alias {
                    self.clear_generic_function_alias(name);
                    self.set_deferred_lambda_alias(name, callable);
                    self.output
                        .push_str("    i32.const 0 ;; deferred callable alias\n");
                } else if let Some(source_ty) = binding_source_ty.as_ref() {
                    self.clear_generic_function_alias(name);
                    self.clear_deferred_lambda_alias(name);
                    if Self::is_unit_source_type(source_ty) {
                        self.generate_expr(&bind.value)?;
                    } else {
                        self.generate_expr_with_expected_source(&bind.value, source_ty)?;
                    }
                } else {
                    self.clear_generic_function_alias(name);
                    self.clear_deferred_lambda_alias(name);
                    self.generate_expr(&bind.value)?;
                }

                // Store in local (it should already be declared and registered)
                if self.lookup_local(name).is_none() {
                    return Err(CodeGenError::UndefinedVariable(name.clone()));
                }

                if binding_source_ty
                    .as_ref()
                    .is_some_and(Self::is_unit_source_type)
                    && !self.expr_leaves_value(&bind.value)
                    && !self.expr_synthesizes_unit_value(&bind.value)
                {
                    self.output.push_str("    i32.const 0\n");
                }
                self.output
                    .push_str(&format!("    local.set ${}\n", storage_name));
            }
            Pattern::Record(_, _)
            | Pattern::RecordDestruct { .. }
            | Pattern::Some(_)
            | Pattern::Ok(_)
            | Pattern::Err(_)
            | Pattern::None
            | Pattern::EmptyList
            | Pattern::ListCons(_, _)
            | Pattern::ListExact(_)
            | Pattern::Literal(_)
            | Pattern::Wildcard => self.generate_pattern_binding(bind)?,
        }

        Ok(())
    }

    fn generate_pattern_binding(&mut self, bind: &BindDecl) -> Result<(), CodeGenError> {
        let source_ty = bind
            .type_annotation
            .clone()
            .or_else(|| self.infer_expr_source_type(&bind.value));
        let wasm_ty = if let Some(source_ty) = &source_ty {
            self.convert_type(source_ty)?
        } else {
            self.infer_expr_type(&bind.value)?
        };
        let match_local = self.match_temp_local(wasm_ty);

        if let Some(source_ty) = &source_ty {
            self.generate_expr_with_expected_source(&bind.value, source_ty)?;
        } else {
            self.generate_expr(&bind.value)?;
        }
        self.output
            .push_str(&format!("    local.set ${}\n", match_local));
        self.output
            .push_str(&format!("    local.get ${}\n", match_local));
        let bindings =
            self.generate_pattern_match(&bind.pattern, source_ty.as_ref(), match_local)?;

        self.output.push_str("    (if\n");
        self.output.push_str("      (then\n");
        for (name, load_code) in bindings {
            self.output.push_str(&load_code);
            self.output
                .push_str(&format!("        local.set ${}\n", name));
        }
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        unreachable\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");

        Ok(())
    }

    #[allow(dead_code)]
    fn bind_record_fields(
        &mut self,
        record_name: &str,
        fields: &[(String, Pattern)],
        source_local: &str,
    ) -> Result<(), CodeGenError> {
        for (field_name, field_pattern) in fields {
            self.load_record_field_from_local(record_name, source_local, field_name)?;
            let field_type = self.record_field_type(record_name, field_name).cloned();
            self.bind_pattern_value(field_pattern, field_type.as_ref())?;
        }

        Ok(())
    }

    #[allow(dead_code)]
    fn bind_pattern_value(
        &mut self,
        pattern: &Pattern,
        value_type: Option<&Type>,
    ) -> Result<(), CodeGenError> {
        match pattern {
            Pattern::Ident(name) => {
                if self.lookup_local(name).is_none() {
                    return Err(CodeGenError::UndefinedVariable(name.clone()));
                }

                self.output.push_str(&format!("    local.set ${}\n", name));
                if let Some(value_type) = value_type {
                    self.set_local_source_type(name, value_type.clone());
                    self.register_record_var_type(name, value_type);
                }
            }
            Pattern::Wildcard => {
                self.output.push_str("    drop\n");
            }
            Pattern::Record(record_name, fields) => {
                self.output.push_str("    local.set $base_tmp\n");
                self.bind_record_fields(record_name, fields, "base_tmp")?;
            }
            Pattern::RecordDestruct {
                type_name,
                fields,
                rest,
            } => {
                self.output.push_str("    local.set $base_tmp\n");
                self.bind_record_fields(type_name, fields, "base_tmp")?;
                self.bind_record_rest(type_name, fields, rest.as_ref(), "base_tmp")?;
            }
            _ => {
                return Err(CodeGenError::UnsupportedFeature(format!(
                    "Nested pattern {:?} in record bindings not yet supported",
                    pattern
                )));
            }
        }

        Ok(())
    }

    fn load_record_field_from_local(
        &mut self,
        record_name: &str,
        source_local: &str,
        field_name: &str,
    ) -> Result<(), CodeGenError> {
        let field_offset = self.record_field_offset(record_name, field_name)?;
        self.output
            .push_str(&format!("    local.get ${}\n", source_local));
        self.output
            .push_str(&format!("    i32.const {}\n", field_offset));
        self.output.push_str("    i32.add\n");
        let field_type = self.record_field_type(record_name, field_name);
        self.output
            .push_str(&format!("    {}\n", self.wasm_load_op_for_type(field_type)));
        Ok(())
    }

    fn record_field_offset(
        &self,
        record_name: &str,
        field_name: &str,
    ) -> Result<u32, CodeGenError> {
        self.record_field_offsets
            .get(record_name)
            .and_then(|fields| fields.get(field_name))
            .copied()
            .ok_or_else(|| Self::invalid_record_layout_error(record_name, field_name))
    }

    fn record_field_type(&self, record_name: &str, field_name: &str) -> Option<&Type> {
        self.records
            .get(record_name)?
            .iter()
            .find_map(|(name, ty)| (name == field_name).then_some(ty))
    }

    fn record_rest_load_code(
        &mut self,
        record_name: &str,
        source_ty: Option<&Type>,
        fields: &[(String, Pattern)],
        source_local: &str,
        indent: &str,
    ) -> Result<(String, String), CodeGenError> {
        let residual_name = self.ensure_residual_record_definition(record_name, fields)?;
        let residual_fields = self.records.get(&residual_name).cloned().ok_or_else(|| {
            CodeGenError::NotImplemented(format!("residual record {}", residual_name))
        })?;
        let residual_offsets = self
            .record_field_offsets
            .get(&residual_name)
            .cloned()
            .ok_or_else(|| {
                CodeGenError::NotImplemented(format!("field offsets for {}", residual_name))
            })?;
        let residual_size = self.record_size(&residual_name, residual_fields.len());
        let mut code = String::new();

        code.push_str(&format!(
            "{indent}i32.const {} ;; rest record size\n",
            residual_size
        ));
        code.push_str(&format!("{indent}call $allocate\n"));
        code.push_str(&format!("{indent}local.set $clone_tmp\n"));

        for (field_name, field_ty) in residual_fields {
            let source_offset =
                self.instantiated_record_field_offset(record_name, source_ty, &field_name)?;
            let residual_offset = residual_offsets.get(&field_name).copied().ok_or_else(|| {
                CodeGenError::NotImplemented(format!(
                    "residual field {} in residual record {}",
                    field_name, residual_name
                ))
            })?;
            code.push_str(&format!("{indent}local.get $clone_tmp\n"));
            code.push_str(&format!("{indent}i32.const {}\n", residual_offset));
            code.push_str(&format!("{indent}i32.add\n"));
            code.push_str(&format!("{indent}local.get ${}\n", source_local));
            code.push_str(&format!("{indent}i32.const {}\n", source_offset));
            code.push_str(&format!("{indent}i32.add\n"));
            code.push_str(&format!(
                "{indent}{} ;; load rest field {}\n",
                self.wasm_load_op_for_type(Some(&field_ty)),
                field_name
            ));
            code.push_str(&format!(
                "{indent}{} ;; store rest field {}\n",
                self.wasm_store_op_for_type(Some(&field_ty)),
                field_name
            ));
        }

        code.push_str(&format!("{indent}local.get $clone_tmp\n"));
        Ok((residual_name, code))
    }

    #[allow(dead_code)]
    fn bind_record_rest(
        &mut self,
        record_name: &str,
        fields: &[(String, Pattern)],
        rest: Option<&String>,
        source_local: &str,
    ) -> Result<(), CodeGenError> {
        let Some(rest_name) = rest else {
            return Ok(());
        };
        if rest_name == "_" {
            return Ok(());
        }

        if self.lookup_local(rest_name).is_none() {
            return Err(CodeGenError::UndefinedVariable(rest_name.clone()));
        }

        let (residual_name, load_code) =
            self.record_rest_load_code(record_name, None, fields, source_local, "    ")?;
        self.output.push_str(&load_code);
        self.output
            .push_str(&format!("    local.set ${}\n", rest_name));
        self.set_local_source_type(rest_name, Type::Named(residual_name.clone()));
        self.var_types.insert(rest_name.clone(), residual_name);
        Ok(())
    }

    fn generate_assignment(&mut self, assign: &AssignStmt) -> Result<(), CodeGenError> {
        let storage_name = self
            .lookup_local_alias(&assign.name)
            .map(str::to_string)
            .unwrap_or_else(|| assign.name.clone());
        let source_ty = self
            .lookup_local_source_type(&assign.name)
            .or_else(|| self.infer_expr_source_type(&assign.value));

        if let Some(source_ty) = source_ty.as_ref() {
            self.set_local_source_type(&storage_name, source_ty.clone());
            self.set_local_source_type(&assign.name, source_ty.clone());
            self.generate_expr_with_expected_source(&assign.value, source_ty)?;
        } else {
            self.generate_expr(&assign.value)?;
        }

        // Store in local
        if self.lookup_local(&assign.name).is_some() {
            self.output
                .push_str(&format!("    local.set ${}\n", storage_name));
        } else {
            return Err(CodeGenError::UndefinedVariable(assign.name.clone()));
        }

        Ok(())
    }

    fn generate_temporal_scope(
        &mut self,
        lifetime: &str,
        body: &BlockExpr,
    ) -> Result<(), CodeGenError> {
        // Create a new arena for this temporal scope
        let arena_addr = self.next_arena_addr;
        self.next_arena_addr += 0x1000; // Reserve 4KB for each arena

        // Push arena onto stack and track temporal scope
        self.arena_stack.push(arena_addr);
        self.temporal_scope_stack.push(lifetime.to_string());
        self.temporal_resources
            .insert(lifetime.to_string(), Vec::new());

        // Generate arena initialization
        self.output.push_str(&format!(
            "    ;; Initialize temporal scope arena for {} at address 0x{:x}\n",
            lifetime, arena_addr
        ));
        self.output
            .push_str(&format!("    i32.const {}\n", arena_addr));
        self.output.push_str("    call $arena_init\n");
        self.output.push_str("    drop\n"); // Drop arena address as we track it internally

        // Set this arena as current
        self.output
            .push_str(&format!("    i32.const {}\n", arena_addr));
        self.output.push_str("    global.set $current_arena\n");

        // Save current resource list state (for nested scopes)
        self.output.push_str(&format!(
            "    ;; Save resource list state for temporal scope {}\n",
            lifetime
        ));
        self.output.push_str("    global.get $resource_list_head\n");
        self.output.push_str("    local.tee $temp_resource\n"); // Reuse temp_resource for saved state

        // Generate the body expressions
        self.generate_block_as_expression(body)?;

        // CRITICAL: Clean up all resources registered in this temporal scope
        self.output.push_str(&format!(
            "    ;; Clean up all resources for temporal scope {}\n",
            lifetime
        ));
        self.output.push_str("    call $cleanup_resources\n");

        // Restore previous resource list state
        self.output
            .push_str("    ;; Restore previous resource list state\n");
        self.output.push_str("    local.get $temp_resource\n");
        self.output.push_str("    global.set $resource_list_head\n");

        // Clean up arena memory
        self.output.push_str(&format!(
            "    ;; Reset temporal scope arena for {}\n",
            lifetime
        ));
        self.output
            .push_str(&format!("    i32.const {}\n", arena_addr));
        self.output.push_str("    call $arena_reset\n");

        // Restore previous arena if any
        self.arena_stack.pop();
        self.temporal_scope_stack.pop();
        self.temporal_resources.remove(lifetime);

        if let Some(prev_arena) = self.arena_stack.last() {
            self.output.push_str("    ;; Restore previous arena\n");
            self.output
                .push_str(&format!("    i32.const {}\n", prev_arena));
            self.output.push_str("    global.set $current_arena\n");
        } else if let Some(default_arena) = self.default_arena {
            self.output.push_str("    ;; Restore default arena\n");
            self.output
                .push_str(&format!("    i32.const {}\n", default_arena));
            self.output.push_str("    global.set $current_arena\n");
        } else {
            // No arena to restore
            self.output.push_str("    i32.const 0\n");
            self.output.push_str("    global.set $current_arena\n");
        }

        Ok(())
    }

    /// Register a resource for cleanup in the current temporal scope
    #[allow(dead_code)]
    fn register_temporal_resource(
        &mut self,
        resource_var: &str,
        resource_type: &str,
    ) -> Result<(), CodeGenError> {
        if let Some(cleanup_fn) = self.cleanup_functions.get(resource_type) {
            if let Some(current_lifetime) = self.temporal_scope_stack.last() {
                // Register the resource for cleanup
                self.output.push_str(&format!(
                    "    ;; Register {} resource '{}' for cleanup\n",
                    resource_type, resource_var
                ));
                self.output
                    .push_str(&format!("    local.get ${}\n", resource_var));

                // Get function index for cleanup function (simplified - would need actual function table)
                let cleanup_index = match cleanup_fn.as_str() {
                    "cleanup_file" => 1,
                    "cleanup_database" => 2,
                    "cleanup_transaction" => 3,
                    _ => 0,
                };

                self.output
                    .push_str(&format!("    i32.const {}\n", cleanup_index));
                self.output.push_str("    call $register_resource\n");

                // Track in our internal structures
                if let Some(resources) = self.temporal_resources.get_mut(current_lifetime) {
                    resources.push((resource_var.to_string(), cleanup_fn.clone()));
                }
            }
        }
        Ok(())
    }

    fn generate_expr(&mut self, expr: &Expr) -> Result<(), CodeGenError> {
        match expr {
            Expr::IntLit(n) => {
                let wasm_ty = Self::int_literal_wasm_type(*n);
                self.output.push_str(&format!(
                    "    {}.const {}\n",
                    self.wasm_type_str(wasm_ty),
                    n
                ));
            }
            Expr::FloatLit(f) => {
                self.output.push_str(&format!("    f64.const {}\n", f));
            }
            Expr::BoolLit(b) => {
                self.output
                    .push_str(&format!("    i32.const {}\n", if *b { 1 } else { 0 }));
            }
            Expr::Unit => {
                self.output.push_str("    i32.const 0\n");
            }
            Expr::Ident(name) => {
                // Check if it's a captured variable in a lambda
                if self.in_lambda_with_captures && self.captured_vars.contains(name) {
                    self.emit_local_get(name);
                } else if let Some(_idx) = self.lookup_local(name) {
                    self.emit_local_get(name);
                } else if self.global_types.contains_key(name) {
                    self.output.push_str(&format!("    global.get ${}\n", name));
                } else if self.functions.contains_key(name) {
                    let is_expected_function_value = self.lambda_abi_stack.last().is_some();
                    let has_runtime_params = self
                        .function_source_sigs
                        .get(name)
                        .is_some_and(|sig| !sig.params.is_empty());

                    if is_expected_function_value || has_runtime_params {
                        self.generate_named_function_reference(name)?;
                    } else {
                        // Existing shorthand for zero-argument functions.
                        self.output.push_str(&format!("    call ${}\n", name));
                    }
                } else {
                    return Err(CodeGenError::UndefinedVariable(name.clone()));
                }
            }
            Expr::Binary(binary) => {
                self.generate_binary_expr(binary)?;
            }
            Expr::Unary(unary) => {
                self.generate_unary_expr(unary)?;
            }
            Expr::Cast(cast) => {
                self.generate_cast_expr(cast)?;
            }
            Expr::Call(call) => {
                self.generate_call_expr(call)?;
            }
            Expr::Block(block) => {
                self.generate_block(block)?;
            }
            Expr::RecordLit(record_lit) => {
                let record_source_ty = self
                    .infer_record_lit_source_type(record_lit)
                    .unwrap_or_else(|| Type::Named(record_lit.name.clone()));
                self.generate_record_literal_with_source_type(record_lit, &record_source_ty)?;
            }
            Expr::FieldAccess(obj_expr, field) => {
                // Generate object expression
                self.generate_expr(obj_expr)?;

                // Get the type of the object expression
                let object_source_ty = self.infer_expr_source_type(obj_expr);
                let record_name = if let Some(source_ty) = &object_source_ty {
                    self.source_record_name(source_ty)
                        .map(str::to_string)
                        .ok_or_else(|| {
                            CodeGenError::NotImplemented(format!("field access for {}", field))
                        })?
                } else if let Expr::Ident(var_name) = obj_expr.as_ref() {
                    // For identifiers, look up the record type from variable tracking.
                    self.var_types.get(var_name).cloned().ok_or_else(|| {
                        CodeGenError::NotImplemented(format!(
                            "field access on unknown variable: {}",
                            var_name
                        ))
                    })?
                } else if let Some(obj_type) =
                    self.expr_types.get(&(obj_expr.as_ref() as *const Expr))
                {
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
                    return Err(CodeGenError::NotImplemented(format!(
                        "field access for {}",
                        field
                    )));
                };

                // Look up the field offset
                let field_offset = self.instantiated_record_field_offset(
                    &record_name,
                    object_source_ty.as_ref(),
                    field,
                )?;

                self.output
                    .push_str(&format!("    i32.const {}\n", field_offset));
                self.output.push_str("    i32.add\n");
                let field_type = object_source_ty
                    .as_ref()
                    .and_then(|source_ty| self.instantiated_record_field_type(source_ty, field))
                    .or_else(|| self.record_field_type(&record_name, field).cloned());
                self.output.push_str(&format!(
                    "    {}\n",
                    self.wasm_load_op_for_type(field_type.as_ref())
                ));
            }
            Expr::StringLit(s) => {
                if let Some(offset) = self.string_offsets.get(s) {
                    self.output.push_str(&format!("    i32.const {}\n", offset));
                } else {
                    return Err(CodeGenError::NotImplemented(
                        "string literal not in pool".to_string(),
                    ));
                }
            }
            Expr::CharLit(c) => {
                self.output
                    .push_str(&format!("    i32.const {}\n", *c as u32));
            }
            Expr::Pipe(pipe) => {
                self.generate_pipe_expr(pipe)?;
            }
            Expr::ListLit(items) => {
                self.generate_list_literal(items)?;
            }
            Expr::RangeLit(range) => {
                self.generate_range_literal(range)?;
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
            Expr::With(with_expr) => {
                self.generate_with_expr(with_expr)?;
            }
            Expr::WithLifetime(with_lifetime) => {
                self.generate_temporal_scope(&with_lifetime.lifetime, &with_lifetime.body)?;
            }
            Expr::Await(_) | Expr::Spawn(_) => {
                return Err(CodeGenError::UnsupportedFeature(
                    "async await/spawn operations are experimental and outside the v0.0.1 codegen surface"
                        .to_string(),
                ));
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
                self.generate_variant_constructor("Some", 1, inner)?;
            }
            Expr::Ok(inner) => {
                self.generate_variant_constructor("Ok", 1, inner)?;
            }
            Expr::Err(inner) => {
                self.generate_variant_constructor("Err", 0, inner)?;
            }
            Expr::Lambda(lambda) => {
                self.generate_lambda_expr(lambda)?;
            }
            Expr::PrototypeClone(proto_clone) => {
                self.generate_prototype_clone_expr(proto_clone)?;
            }
        }
        Ok(())
    }

    fn generate_lambda_expr(&mut self, lambda: &LambdaExpr) -> Result<(), CodeGenError> {
        let abi = self.lambda_abi_for(lambda)?;
        // Generate a unique name for this lambda
        let lambda_name = format!("lambda_{}", self.lambda_counter);
        self.lambda_counter += 1;

        // Add to function table for indirect calls
        let table_index = self.function_table.len();
        self.function_table.push(lambda_name.clone());

        // Analyze free variables (variables captured from outer scope)
        let free_vars = self.analyze_free_variables(lambda)?;
        let free_var_sources: HashMap<String, Type> = free_vars
            .iter()
            .filter_map(|(name, _)| {
                self.lookup_local_source_type(name)
                    .map(|source_ty| (name.clone(), source_ty))
            })
            .collect();

        let lambda_locals = self.collect_lambda_locals(lambda)?;
        let lambda_record_tmp_count =
            RECORD_TMP_MIN_COUNT.max(Self::max_record_tmp_depth_in_expr(&lambda.body));

        // Closure layout: [function_index, captured_var1, captured_var2, ...].
        // Even non-capturing lambdas use this layout so function values have a
        // single runtime representation.
        let closure_size = 4 + free_vars
            .iter()
            .map(|(_, ty)| self.wasm_type_size(*ty))
            .sum::<usize>();

        self.output
            .push_str(&format!("    i32.const {} ;; closure size\n", closure_size));
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $closure_tmp\n");

        self.output.push_str("    local.get $closure_tmp\n");
        self.output.push_str(&format!(
            "    i32.const {} ;; function table index for {}\n",
            table_index, lambda_name
        ));
        self.output.push_str("    i32.store\n");

        let mut offset = 4;
        for (i, (var_name, _)) in free_vars.iter().enumerate() {
            let captured_ty = free_vars[i].1;
            self.output.push_str("    local.get $closure_tmp\n");
            self.output.push_str(&format!(
                "    i32.const {} ;; offset for captured var {}\n",
                offset, i
            ));
            self.output.push_str("    i32.add\n");
            self.emit_local_get(var_name);
            self.output.push_str(&format!(
                "    {}\n",
                self.wasm_store_op_for_wasm_type(captured_ty)
            ));
            offset += self.wasm_type_size(captured_ty);
        }

        self.output.push_str("    local.get $closure_tmp\n");

        // Generate the lambda function separately
        let mut lambda_code = String::new();
        lambda_code.push_str(&format!("  (func ${}", lambda_name));

        // Parameters
        for (param, param_ty) in lambda.params.iter().zip(abi.params.iter()) {
            lambda_code.push_str(&format!(
                " (param ${} {})",
                param.name,
                self.wasm_type_str(*param_ty)
            ));
        }

        lambda_code.push_str(" (param $closure i32)");

        lambda_code.push_str(&format!(" (result {})\n", self.wasm_type_str(abi.result)));

        lambda_code.push_str("    (local $closure_tmp i32)\n");
        lambda_code.push_str("    (local $list_tmp i32)\n");
        lambda_code.push_str("    (local $match_tmp i32)\n");
        lambda_code.push_str("    (local $match_tmp_i64 i64)\n");
        lambda_code.push_str("    (local $match_tmp_f32 f32)\n");
        lambda_code.push_str("    (local $match_tmp_f64 f64)\n");
        lambda_code.push_str("    (local $option_value_tmp i32)\n");
        lambda_code.push_str("    (local $option_value_i64_tmp i64)\n");
        lambda_code.push_str("    (local $option_value_f32_tmp f32)\n");
        lambda_code.push_str("    (local $option_value_f64_tmp f64)\n");
        lambda_code.push_str("    (local $f64_mod_left f64)\n");
        lambda_code.push_str("    (local $f64_mod_right f64)\n");
        lambda_code.push_str("    (local $tail_len i32)\n");
        lambda_code.push_str("    (local $tail_tmp i32)\n");
        for name in [
            "iter_list",
            "iter_func",
            "iter_len",
            "iter_index",
            "iter_out",
            "iter_out_index",
            "iter_value",
            "iter_result",
            "iter_acc",
        ] {
            lambda_code.push_str(&format!("    (local ${} i32)\n", name));
        }
        for (name, ty) in [
            ("iter_value_i64", WasmType::I64),
            ("iter_acc_i64", WasmType::I64),
            ("iter_value_f64", WasmType::F64),
            ("iter_result_f64", WasmType::F64),
            ("iter_acc_f64", WasmType::F64),
        ] {
            lambda_code.push_str(&format!(
                "    (local ${} {})\n",
                name,
                self.wasm_type_str(ty)
            ));
        }
        lambda_code.push_str("    (local $clone_tmp i32)\n");
        lambda_code.push_str("    (local $base_tmp i32)\n");
        lambda_code.push_str("    (local $freeze_tmp i32)\n");
        for depth in 0..lambda_record_tmp_count {
            lambda_code.push_str(&format!("    (local $record_tmp_{} i32)\n", depth));
        }
        for depth in 0..WITH_ARENA_TMP_COUNT {
            lambda_code.push_str(&format!("    (local $with_prev_arena_{} i32)\n", depth));
        }
        for (name, ty) in &lambda_locals {
            lambda_code.push_str(&format!(
                "    (local ${} {})\n",
                name,
                self.wasm_type_str(*ty)
            ));
        }

        // Generate local declarations for captured variables
        if !free_vars.is_empty() {
            for (var_name, ty) in &free_vars {
                lambda_code.push_str(&format!(
                    "    (local ${}_captured {})\n",
                    var_name,
                    self.wasm_type_str(*ty)
                ));
            }

            // Load captured variables from closure
            let mut offset = 4;
            for (var_name, ty) in &free_vars {
                lambda_code.push_str("    local.get $closure\n");
                lambda_code.push_str(&format!("    i32.const {}\n", offset));
                lambda_code.push_str("    i32.add\n");
                lambda_code.push_str(&format!("    {}\n", self.wasm_load_op_for_wasm_type(*ty)));
                lambda_code.push_str(&format!("    local.set ${}_captured\n", var_name));
                offset += self.wasm_type_size(*ty);
            }
        }

        // Generate lambda body with captured variable context
        let old_in_lambda = self.in_lambda_with_captures;
        let old_captured_vars = self.captured_vars.clone();

        self.in_lambda_with_captures = !free_vars.is_empty();
        self.captured_vars = free_vars.iter().map(|(name, _)| name.clone()).collect();
        let old_record_literal_depth = self.record_literal_depth;
        let old_record_tmp_count = self.record_tmp_count;
        let old_with_arena_depth = self.with_arena_depth;
        self.record_literal_depth = 0;
        self.record_tmp_count = lambda_record_tmp_count;
        self.with_arena_depth = 0;

        // Save current output and switch to lambda code
        let saved_output = std::mem::replace(&mut self.output, lambda_code);

        // Set up local scope for lambda
        self.push_scope();
        for (i, (param, param_ty)) in lambda.params.iter().zip(abi.params.iter()).enumerate() {
            self.add_local(&param.name, i as u32);
            self.set_local_type(&param.name, *param_ty);
            if let Some(source_ty) = abi.source_params.get(i) {
                self.set_local_source_type(&param.name, source_ty.clone());
            }
        }
        self.add_local("closure", lambda.params.len() as u32);
        self.set_local_type("closure", WasmType::I32);
        for (name, ty) in &free_vars {
            self.set_local_type(name, *ty);
            if let Some(source_ty) = free_var_sources.get(name) {
                self.set_local_source_type(name, source_ty.clone());
            }
        }

        let mut next_idx = lambda.params.len() as u32 + 1;
        for (name, ty) in &lambda_locals {
            self.add_local(name, next_idx);
            self.set_local_type(name, *ty);
            next_idx += 1;
        }

        // Generate lambda body
        let body_result = self.generate_expr(&lambda.body);

        self.pop_scope();

        // Restore output and save lambda code
        lambda_code = std::mem::replace(&mut self.output, saved_output);

        self.in_lambda_with_captures = old_in_lambda;
        self.captured_vars = old_captured_vars;
        self.record_literal_depth = old_record_literal_depth;
        self.record_tmp_count = old_record_tmp_count;
        self.with_arena_depth = old_with_arena_depth;

        body_result?;

        lambda_code.push_str("  )\n");

        // Add lambda function to the list
        self.lambda_functions.push(lambda_code);

        Ok(())
    }

    fn collect_lambda_locals(
        &mut self,
        lambda: &LambdaExpr,
    ) -> Result<Vec<(String, WasmType)>, CodeGenError> {
        let abi = self.lambda_abi_for(lambda)?;
        self.push_scope();
        for (idx, (param, param_ty)) in lambda.params.iter().zip(abi.params.iter()).enumerate() {
            self.add_local(&param.name, idx as u32);
            self.set_local_type(&param.name, *param_ty);
            if let Some(source_ty) = abi.source_params.get(idx) {
                self.set_local_source_type(&param.name, source_ty.clone());
            }
        }

        let mut locals = Vec::new();
        self.collect_locals_from_expr(&lambda.body, &mut locals)?;
        self.pop_scope();

        let params: HashSet<&str> = lambda
            .params
            .iter()
            .map(|param| param.name.as_str())
            .collect();
        let locals = locals
            .into_iter()
            .filter(|(name, _)| !params.contains(name.as_str()))
            .collect();

        Self::dedupe_locals(locals)
    }

    fn lambda_abi_for(&self, lambda: &LambdaExpr) -> Result<LambdaAbiContext, CodeGenError> {
        if let Some(context) = self.lambda_abi_stack.last() {
            if context.params.len() == lambda.params.len() {
                return Ok(context.clone());
            }
        }

        let source_params = lambda
            .params
            .iter()
            .map(|param| {
                param.type_annotation.clone().ok_or_else(|| {
                    CodeGenError::UnsupportedFeature(format!(
                        "lambda parameter '{}' requires an expected function type or an explicit annotation for code generation",
                        param.name
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let params = source_params
            .iter()
            .map(|param| self.convert_type(param))
            .collect::<Result<Vec<_>, _>>()?;
        let source_bindings = lambda
            .params
            .iter()
            .zip(source_params.iter())
            .map(|(param, source_ty)| (param.name.clone(), source_ty.clone()))
            .collect::<HashMap<_, _>>();
        let result_source =
            self.infer_expr_source_type_with_bindings(&lambda.body, &source_bindings).ok_or_else(
                || {
                    CodeGenError::UnsupportedFeature(
                        "lambda without an expected function result requires an inferable return type for code generation"
                            .to_string(),
                    )
                },
            )?;
        let result = self.convert_type(&result_source)?;

        Ok(LambdaAbiContext {
            params,
            result,
            source_params,
            source_result: result_source,
        })
    }

    fn analyze_free_variables(
        &self,
        lambda: &LambdaExpr,
    ) -> Result<Vec<(String, WasmType)>, CodeGenError> {
        let mut bound: HashSet<String> = lambda
            .params
            .iter()
            .map(|param| param.name.clone())
            .collect();
        let mut seen = HashSet::new();
        let mut free_vars = Vec::new();
        self.collect_free_variables_for_codegen(
            &lambda.body,
            &mut bound,
            &mut seen,
            &mut free_vars,
        )?;
        Ok(free_vars)
    }

    fn collect_free_variables_for_codegen(
        &self,
        expr: &Expr,
        bound: &mut HashSet<String>,
        seen: &mut HashSet<String>,
        free_vars: &mut Vec<(String, WasmType)>,
    ) -> Result<(), CodeGenError> {
        match expr {
            Expr::Ident(name) => self.capture_if_free(name, bound, seen, free_vars)?,
            Expr::Binary(binary) => {
                self.collect_free_variables_for_codegen(&binary.left, bound, seen, free_vars)?;
                self.collect_free_variables_for_codegen(&binary.right, bound, seen, free_vars)?;
            }
            Expr::Unary(unary) => {
                self.collect_free_variables_for_codegen(&unary.expr, bound, seen, free_vars)?;
            }
            Expr::Cast(cast) => {
                self.collect_free_variables_for_codegen(&cast.expr, bound, seen, free_vars)?;
            }
            Expr::Call(call) => {
                self.collect_free_variables_for_codegen(&call.function, bound, seen, free_vars)?;
                for arg in &call.args {
                    self.collect_free_variables_for_codegen(arg, bound, seen, free_vars)?;
                }
            }
            Expr::Pipe(pipe) => {
                self.collect_free_variables_for_codegen(&pipe.expr, bound, seen, free_vars)?;
                match &pipe.target {
                    PipeTarget::Ident(name) => {
                        if !self.functions.contains_key(name) {
                            self.capture_if_free(name, bound, seen, free_vars)?;
                        }
                    }
                    PipeTarget::Expr(target) => {
                        self.collect_free_variables_for_codegen(target, bound, seen, free_vars)?;
                    }
                }
            }
            Expr::FieldAccess(object, _) => {
                self.collect_free_variables_for_codegen(object, bound, seen, free_vars)?;
            }
            Expr::RecordLit(record) => {
                for field in &record.fields {
                    match field {
                        FieldInit::Field { value, .. } => {
                            self.collect_free_variables_for_codegen(value, bound, seen, free_vars)?;
                        }
                        FieldInit::Spread(value) => {
                            self.collect_free_variables_for_codegen(value, bound, seen, free_vars)?;
                        }
                    }
                }
            }
            Expr::Clone(clone) => {
                self.collect_free_variables_for_codegen(&clone.base, bound, seen, free_vars)?;
                for field in &clone.updates.fields {
                    match field {
                        FieldInit::Field { value, .. } => {
                            self.collect_free_variables_for_codegen(value, bound, seen, free_vars)?;
                        }
                        FieldInit::Spread(value) => {
                            self.collect_free_variables_for_codegen(value, bound, seen, free_vars)?;
                        }
                    }
                }
            }
            Expr::Freeze(value) | Expr::Await(value) | Expr::Spawn(value) => {
                self.collect_free_variables_for_codegen(value, bound, seen, free_vars)?;
            }
            Expr::PrototypeClone(proto) => {
                for field in &proto.updates.fields {
                    match field {
                        FieldInit::Field { value, .. } => {
                            self.collect_free_variables_for_codegen(value, bound, seen, free_vars)?;
                        }
                        FieldInit::Spread(value) => {
                            self.collect_free_variables_for_codegen(value, bound, seen, free_vars)?;
                        }
                    }
                }
            }
            Expr::Then(then) => {
                self.collect_free_variables_for_codegen(&then.condition, bound, seen, free_vars)?;
                self.collect_free_variables_in_block_for_codegen(
                    &then.then_block,
                    bound,
                    seen,
                    free_vars,
                )?;
                for (condition, block) in &then.else_ifs {
                    self.collect_free_variables_for_codegen(condition, bound, seen, free_vars)?;
                    self.collect_free_variables_in_block_for_codegen(
                        block, bound, seen, free_vars,
                    )?;
                }
                if let Some(block) = &then.else_block {
                    self.collect_free_variables_in_block_for_codegen(
                        block, bound, seen, free_vars,
                    )?;
                }
            }
            Expr::While(while_expr) => {
                self.collect_free_variables_for_codegen(
                    &while_expr.condition,
                    bound,
                    seen,
                    free_vars,
                )?;
                self.collect_free_variables_in_block_for_codegen(
                    &while_expr.body,
                    bound,
                    seen,
                    free_vars,
                )?;
            }
            Expr::Match(match_expr) => {
                self.collect_free_variables_for_codegen(&match_expr.expr, bound, seen, free_vars)?;
                for arm in &match_expr.arms {
                    let mut arm_bound = bound.clone();
                    self.collect_pattern_bindings_for_codegen(&arm.pattern, &mut arm_bound);
                    self.collect_free_variables_in_block_for_codegen(
                        &arm.body,
                        &mut arm_bound,
                        seen,
                        free_vars,
                    )?;
                }
            }
            Expr::With(with_expr) => {
                let mut body_bound = bound.clone();
                for binding in &with_expr.bindings {
                    match binding {
                        FieldInit::Field { name, value } => {
                            self.collect_free_variables_for_codegen(value, bound, seen, free_vars)?;
                            body_bound.insert(name.clone());
                        }
                        FieldInit::Spread(value) => {
                            self.collect_free_variables_for_codegen(value, bound, seen, free_vars)?;
                        }
                    }
                }
                self.collect_free_variables_in_block_for_codegen(
                    &with_expr.body,
                    &mut body_bound,
                    seen,
                    free_vars,
                )?;
            }
            Expr::WithLifetime(with_lifetime) => {
                self.collect_free_variables_in_block_for_codegen(
                    &with_lifetime.body,
                    bound,
                    seen,
                    free_vars,
                )?;
            }
            Expr::Block(block) => {
                self.collect_free_variables_in_block_for_codegen(block, bound, seen, free_vars)?;
            }
            Expr::ListLit(items) | Expr::ArrayLit(items) => {
                for item in items {
                    self.collect_free_variables_for_codegen(item, bound, seen, free_vars)?;
                }
            }
            Expr::RangeLit(range) => {
                self.collect_free_variables_for_codegen(&range.start, bound, seen, free_vars)?;
                self.collect_free_variables_for_codegen(&range.end, bound, seen, free_vars)?;
            }
            Expr::Some(value) => {
                self.collect_free_variables_for_codegen(value, bound, seen, free_vars)?;
            }
            Expr::Ok(value) | Expr::Err(value) => {
                self.collect_free_variables_for_codegen(value, bound, seen, free_vars)?;
            }
            Expr::Lambda(lambda) => {
                let mut lambda_bound = bound.clone();
                for param in &lambda.params {
                    lambda_bound.insert(param.name.clone());
                }
                self.collect_free_variables_for_codegen(
                    &lambda.body,
                    &mut lambda_bound,
                    seen,
                    free_vars,
                )?;
            }
            Expr::IntLit(_)
            | Expr::FloatLit(_)
            | Expr::StringLit(_)
            | Expr::CharLit(_)
            | Expr::BoolLit(_)
            | Expr::Unit
            | Expr::None => {}
        }

        Ok(())
    }

    fn generate_record_spread_copy(
        &mut self,
        record_name: &str,
        source: &Expr,
        target_local: &str,
    ) -> Result<(), CodeGenError> {
        let fields = self.records.get(record_name).cloned().ok_or_else(|| {
            CodeGenError::NotImplemented(format!("spread for unknown record {}", record_name))
        })?;
        let offsets = self
            .record_field_offsets
            .get(record_name)
            .cloned()
            .ok_or_else(|| {
                CodeGenError::NotImplemented(format!("spread offsets for record {}", record_name))
            })?;

        self.generate_expr(source)?;
        self.output.push_str("    local.set $base_tmp\n");

        for (field_name, field_type) in fields {
            let offset = offsets.get(&field_name).copied().ok_or_else(|| {
                CodeGenError::NotImplemented(format!(
                    "spread field {} in record {}",
                    field_name, record_name
                ))
            })?;
            self.output
                .push_str(&format!("    local.get ${}\n", target_local));
            self.output.push_str(&format!("    i32.const {}\n", offset));
            self.output.push_str("    i32.add\n");
            self.output.push_str("    local.get $base_tmp\n");
            self.output.push_str(&format!("    i32.const {}\n", offset));
            self.output.push_str("    i32.add\n");
            self.output.push_str(&format!(
                "    {}\n",
                self.wasm_load_op_for_type(Some(&field_type))
            ));
            self.output.push_str(&format!(
                "    {}\n",
                self.wasm_store_op_for_type(Some(&field_type))
            ));
        }

        Ok(())
    }

    fn collect_free_variables_in_block_for_codegen(
        &self,
        block: &BlockExpr,
        bound: &mut HashSet<String>,
        seen: &mut HashSet<String>,
        free_vars: &mut Vec<(String, WasmType)>,
    ) -> Result<(), CodeGenError> {
        let mut block_bound = bound.clone();

        for stmt in &block.statements {
            match stmt {
                Stmt::Binding(binding) => {
                    self.collect_free_variables_for_codegen(
                        &binding.value,
                        &mut block_bound,
                        seen,
                        free_vars,
                    )?;
                    self.collect_pattern_bindings_for_codegen(&binding.pattern, &mut block_bound);
                }
                Stmt::Assignment(assign) => {
                    self.collect_free_variables_for_codegen(
                        &assign.value,
                        &mut block_bound,
                        seen,
                        free_vars,
                    )?;
                }
                Stmt::Expr(expr) => {
                    self.collect_free_variables_for_codegen(
                        expr,
                        &mut block_bound,
                        seen,
                        free_vars,
                    )?;
                }
            }
        }

        if let Some(expr) = &block.expr {
            self.collect_free_variables_for_codegen(expr, &mut block_bound, seen, free_vars)?;
        }

        Ok(())
    }

    fn collect_pattern_bindings_for_codegen(&self, pattern: &Pattern, bound: &mut HashSet<String>) {
        match pattern {
            Pattern::Ident(name) => {
                bound.insert(name.clone());
            }
            Pattern::Record(_, fields) => {
                for (_, pattern) in fields {
                    self.collect_pattern_bindings_for_codegen(pattern, bound);
                }
            }
            Pattern::RecordDestruct { fields, rest, .. } => {
                for (_, pattern) in fields {
                    self.collect_pattern_bindings_for_codegen(pattern, bound);
                }
                if let Some(rest) = rest {
                    if rest != "_" {
                        bound.insert(rest.clone());
                    }
                }
            }
            Pattern::Some(inner)
            | Pattern::Ok(inner)
            | Pattern::Err(inner)
            | Pattern::ListCons(inner, _) => {
                self.collect_pattern_bindings_for_codegen(inner, bound);
                if let Pattern::ListCons(_, tail) = pattern {
                    self.collect_pattern_bindings_for_codegen(tail, bound);
                }
            }
            Pattern::ListExact(patterns) => {
                for pattern in patterns {
                    self.collect_pattern_bindings_for_codegen(pattern, bound);
                }
            }
            Pattern::Wildcard | Pattern::Literal(_) | Pattern::None | Pattern::EmptyList => {}
        }
    }

    fn capture_if_free(
        &self,
        name: &str,
        bound: &HashSet<String>,
        seen: &mut HashSet<String>,
        free_vars: &mut Vec<(String, WasmType)>,
    ) -> Result<(), CodeGenError> {
        if !bound.contains(name) && self.lookup_local(name).is_some() && seen.insert(name.into()) {
            let ty = self.lookup_local_abi_type(name)?.ok_or_else(|| {
                CodeGenError::UnsupportedFeature(format!(
                    "missing Wasm ABI metadata for local '{}'",
                    name
                ))
            })?;
            free_vars.push((name.to_string(), ty));
        }

        Ok(())
    }

    fn emit_local_get(&mut self, name: &str) {
        if self.in_lambda_with_captures && self.captured_vars.iter().any(|var| var == name) {
            self.output
                .push_str(&format!("    local.get ${}_captured\n", name));
        } else if let Some(local_name) = self.lookup_local_alias(name) {
            self.output
                .push_str(&format!("    local.get ${}\n", local_name));
        } else {
            self.output.push_str(&format!("    local.get ${}\n", name));
        }
    }

    fn generate_binary_expr(&mut self, binary: &BinaryExpr) -> Result<(), CodeGenError> {
        if binary.op == BinaryOp::Add && self.is_string_concat(binary) {
            self.generate_expr(&binary.left)?;
            self.generate_expr(&binary.right)?;
            self.output.push_str("    call $string_concat\n");
            return Ok(());
        }

        if matches!(binary.op, BinaryOp::Eq | BinaryOp::Ne) && self.is_string_binary(binary) {
            self.generate_expr(&binary.left)?;
            self.generate_expr(&binary.right)?;
            self.output.push_str("    call $string_eq\n");
            if binary.op == BinaryOp::Ne {
                self.output.push_str("    i32.eqz\n");
            }
            return Ok(());
        }

        let operand_type = self.infer_binary_operand_type(binary)?;

        if operand_type == WasmType::F64 && binary.op == BinaryOp::Mod {
            self.generate_f64_mod_expr(binary)?;
            return Ok(());
        }

        // Generate operands with the selected numeric ABI so integer literals
        // in Int64 expressions are emitted as i64 values. Logical operators
        // consume Boolean/i32 subexpressions, so their operands should keep
        // their own expression-level codegen.
        if matches!(binary.op, BinaryOp::And | BinaryOp::Or) {
            self.generate_expr(&binary.left)?;
            self.generate_expr(&binary.right)?;
        } else {
            self.generate_expr_with_wasm_type(&binary.left, operand_type)?;
            self.generate_expr_with_wasm_type(&binary.right, operand_type)?;
        }

        // Generate operation
        let op = match (operand_type, &binary.op) {
            (WasmType::F64, BinaryOp::Add) => "f64.add",
            (WasmType::F64, BinaryOp::Sub) => "f64.sub",
            (WasmType::F64, BinaryOp::Mul) => "f64.mul",
            (WasmType::F64, BinaryOp::Div) => "f64.div",
            (WasmType::F64, BinaryOp::Mod) => unreachable!("handled before generic binary op"),
            (WasmType::F64, BinaryOp::Eq) => "f64.eq",
            (WasmType::F64, BinaryOp::Ne) => "f64.ne",
            (WasmType::F64, BinaryOp::Lt) => "f64.lt",
            (WasmType::F64, BinaryOp::Gt) => "f64.gt",
            (WasmType::F64, BinaryOp::Le) => "f64.le",
            (WasmType::F64, BinaryOp::Ge) => "f64.ge",
            (WasmType::I64, BinaryOp::Add) => "i64.add",
            (WasmType::I64, BinaryOp::Sub) => "i64.sub",
            (WasmType::I64, BinaryOp::Mul) => "i64.mul",
            (WasmType::I64, BinaryOp::Div) => "i64.div_s",
            (WasmType::I64, BinaryOp::Mod) => "i64.rem_s",
            (WasmType::I64, BinaryOp::Eq) => "i64.eq",
            (WasmType::I64, BinaryOp::Ne) => "i64.ne",
            (WasmType::I64, BinaryOp::Lt) => "i64.lt_s",
            (WasmType::I64, BinaryOp::Gt) => "i64.gt_s",
            (WasmType::I64, BinaryOp::Le) => "i64.le_s",
            (WasmType::I64, BinaryOp::Ge) => "i64.ge_s",
            (_, BinaryOp::Add) => "i32.add",
            (_, BinaryOp::Sub) => "i32.sub",
            (_, BinaryOp::Mul) => "i32.mul",
            (_, BinaryOp::Div) => "i32.div_s",
            (_, BinaryOp::Mod) => "i32.rem_s",
            (_, BinaryOp::Eq) => "i32.eq",
            (_, BinaryOp::Ne) => "i32.ne",
            (_, BinaryOp::Lt) => "i32.lt_s",
            (_, BinaryOp::Gt) => "i32.gt_s",
            (_, BinaryOp::Le) => "i32.le_s",
            (_, BinaryOp::Ge) => "i32.ge_s",
            (_, BinaryOp::And) => "i32.and",
            (_, BinaryOp::Or) => "i32.or",
        };

        self.output.push_str(&format!("    {}\n", op));

        Ok(())
    }

    fn generate_binary_expr_with_operand_type(
        &mut self,
        binary: &BinaryExpr,
        operand_type: WasmType,
    ) -> Result<(), CodeGenError> {
        if operand_type == WasmType::F64 && binary.op == BinaryOp::Mod {
            self.generate_f64_mod_expr(binary)?;
            return Ok(());
        }

        self.generate_expr_with_wasm_type(&binary.left, operand_type)?;
        self.generate_expr_with_wasm_type(&binary.right, operand_type)?;

        let op = match (operand_type, &binary.op) {
            (WasmType::F64, BinaryOp::Add) => "f64.add",
            (WasmType::F64, BinaryOp::Sub) => "f64.sub",
            (WasmType::F64, BinaryOp::Mul) => "f64.mul",
            (WasmType::F64, BinaryOp::Div) => "f64.div",
            (WasmType::F64, BinaryOp::Mod) => unreachable!("handled before generic binary op"),
            (WasmType::I64, BinaryOp::Add) => "i64.add",
            (WasmType::I64, BinaryOp::Sub) => "i64.sub",
            (WasmType::I64, BinaryOp::Mul) => "i64.mul",
            (WasmType::I64, BinaryOp::Div) => "i64.div_s",
            (WasmType::I64, BinaryOp::Mod) => "i64.rem_s",
            (_, BinaryOp::Add) => "i32.add",
            (_, BinaryOp::Sub) => "i32.sub",
            (_, BinaryOp::Mul) => "i32.mul",
            (_, BinaryOp::Div) => "i32.div_s",
            (_, BinaryOp::Mod) => "i32.rem_s",
            (_, op) => {
                return Err(CodeGenError::UnsupportedFeature(format!(
                    "expected-value codegen for binary operator '{}'",
                    op
                )))
            }
        };

        self.output.push_str(&format!("    {}\n", op));
        Ok(())
    }

    fn generate_expr_with_wasm_type(
        &mut self,
        expr: &Expr,
        expected_type: WasmType,
    ) -> Result<(), CodeGenError> {
        match (expected_type, expr) {
            (WasmType::I64, Expr::IntLit(value)) => {
                self.output.push_str(&format!("    i64.const {}\n", value));
                Ok(())
            }
            (WasmType::I32, Expr::IntLit(value)) => {
                self.output.push_str(&format!("    i32.const {}\n", value));
                Ok(())
            }
            (WasmType::I64, Expr::Unary(unary)) if unary.op == UnaryOp::Neg => {
                self.generate_expr_with_wasm_type(&unary.expr, WasmType::I64)?;
                self.output.push_str("    i64.const -1\n");
                self.output.push_str("    i64.mul\n");
                Ok(())
            }
            (WasmType::I32, Expr::Unary(unary)) if unary.op == UnaryOp::Neg => {
                self.generate_expr_with_wasm_type(&unary.expr, WasmType::I32)?;
                self.output.push_str("    i32.const -1\n");
                self.output.push_str("    i32.mul\n");
                Ok(())
            }
            (WasmType::I64, Expr::Binary(binary)) if Self::is_arithmetic_op(&binary.op) => {
                self.generate_binary_expr_with_operand_type(binary, WasmType::I64)
            }
            (WasmType::F64, Expr::Binary(binary)) if Self::is_arithmetic_op(&binary.op) => {
                self.generate_binary_expr_with_operand_type(binary, WasmType::F64)
            }
            (WasmType::I32, Expr::Binary(binary)) if Self::is_arithmetic_op(&binary.op) => {
                self.generate_binary_expr_with_operand_type(binary, WasmType::I32)
            }
            _ => self.generate_expr(expr),
        }
    }

    fn is_arithmetic_op(op: &BinaryOp) -> bool {
        matches!(
            op,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod
        )
    }

    fn generate_f64_mod_expr(&mut self, binary: &BinaryExpr) -> Result<(), CodeGenError> {
        self.generate_expr(&binary.left)?;
        self.output.push_str("    local.set $f64_mod_left\n");

        self.generate_expr(&binary.right)?;
        self.output.push_str("    local.set $f64_mod_right\n");

        self.output.push_str("    local.get $f64_mod_left\n");
        self.output.push_str("    local.get $f64_mod_left\n");
        self.output.push_str("    local.get $f64_mod_right\n");
        self.output.push_str("    f64.div\n");
        self.output.push_str("    f64.trunc\n");
        self.output.push_str("    local.get $f64_mod_right\n");
        self.output.push_str("    f64.mul\n");
        self.output.push_str("    f64.sub\n");

        Ok(())
    }

    fn generate_unary_expr(&mut self, unary: &UnaryExpr) -> Result<(), CodeGenError> {
        let operand_type = self.infer_expr_type(&unary.expr)?;
        self.generate_expr(&unary.expr)?;

        match (&unary.op, operand_type) {
            (UnaryOp::Neg, WasmType::F64) => {
                self.output.push_str("    f64.neg\n");
            }
            (UnaryOp::Neg, WasmType::I64) => {
                self.output.push_str("    i64.const -1\n");
                self.output.push_str("    i64.mul\n");
            }
            (UnaryOp::Neg, _) => {
                self.output.push_str("    i32.const -1\n");
                self.output.push_str("    i32.mul\n");
            }
            (UnaryOp::Not, _) => {
                self.output.push_str("    i32.eqz\n");
            }
        }

        Ok(())
    }

    fn generate_cast_expr(&mut self, cast: &CastExpr) -> Result<(), CodeGenError> {
        let source_ty = self.infer_expr_type(&cast.expr)?;
        let target_ty = self.convert_type(&cast.target)?;

        self.generate_expr_with_wasm_type(&cast.expr, source_ty)?;

        match (source_ty, target_ty) {
            (source_ty, target_ty) if source_ty == target_ty => {}
            (WasmType::I32, WasmType::I64) => self.output.push_str("    i64.extend_i32_s\n"),
            (WasmType::I32, WasmType::F64) => self.output.push_str("    f64.convert_i32_s\n"),
            (WasmType::I64, WasmType::I32) => self.output.push_str("    i32.wrap_i64\n"),
            (WasmType::I64, WasmType::F64) => self.output.push_str("    f64.convert_i64_s\n"),
            (WasmType::F64, WasmType::I32) => self.output.push_str("    i32.trunc_f64_s\n"),
            (WasmType::F64, WasmType::I64) => self.output.push_str("    i64.trunc_f64_s\n"),
            _ => {
                return Err(CodeGenError::UnsupportedFeature(format!(
                    "cast from {} to {}",
                    self.wasm_type_str(source_ty),
                    self.wasm_type_str(target_ty)
                )));
            }
        }

        Ok(())
    }

    fn infer_binary_operand_type(&self, binary: &BinaryExpr) -> Result<WasmType, CodeGenError> {
        let left = self.infer_expr_type(&binary.left)?;
        let right = self.infer_expr_type(&binary.right)?;

        if matches!(left, WasmType::F64) || matches!(right, WasmType::F64) {
            Ok(WasmType::F64)
        } else if matches!(left, WasmType::I64) || matches!(right, WasmType::I64) {
            Ok(WasmType::I64)
        } else {
            Ok(WasmType::I32)
        }
    }

    fn generate_variant_constructor(
        &mut self,
        label: &str,
        tag: i32,
        inner: &Expr,
    ) -> Result<(), CodeGenError> {
        let payload_ty = self.infer_expr_type(inner)?;
        self.generate_expr(inner)?;
        self.generate_variant_from_stack(label, tag, payload_ty)
    }

    fn generate_variant_from_stack(
        &mut self,
        label: &str,
        tag: i32,
        payload_ty: WasmType,
    ) -> Result<(), CodeGenError> {
        let payload_tmp = self.payload_temp_local(payload_ty);
        self.output
            .push_str(&format!("    local.set ${}\n", payload_tmp));

        let allocation_size = 4 + self.wasm_type_size(payload_ty);
        self.output.push_str(&format!("    ;; {} literal\n", label));
        self.output
            .push_str(&format!("    i32.const {}\n", allocation_size));
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $match_tmp\n");

        self.output.push_str("    local.get $match_tmp\n");
        self.output
            .push_str(&format!("    i32.const {} ;; {} tag\n", tag, label));
        self.output.push_str("    i32.store\n");

        self.output.push_str("    local.get $match_tmp\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output
            .push_str(&format!("    local.get ${}\n", payload_tmp));
        self.output.push_str(&format!(
            "    {}\n",
            self.wasm_store_op_for_wasm_type(payload_ty)
        ));

        self.output.push_str("    local.get $match_tmp\n");
        Ok(())
    }

    fn generate_call_expr(&mut self, call: &CallExpr) -> Result<(), CodeGenError> {
        if let Expr::Ident(func_name) = &*call.function {
            if let Some(target_name) = self.lookup_generic_function_alias(func_name) {
                match target_name.as_str() {
                    "map" => return self.generate_map_call(call),
                    "filter" => return self.generate_filter_call(call),
                    "fold" => return self.generate_fold_call(call),
                    "identity" => {
                        if call.args.len() != 1 {
                            return Err(CodeGenError::UnsupportedFeature(
                                "identity expects exactly one argument".to_string(),
                            ));
                        }
                        self.generate_expr(&call.args[0])?;
                        return Ok(());
                    }
                    _ => {}
                }
            }

            match func_name.as_str() {
                "map" => return self.generate_map_call(call),
                "filter" => return self.generate_filter_call(call),
                "fold" => return self.generate_fold_call(call),
                _ => {}
            }
        }

        if let Expr::Ident(func_name) = &*call.function {
            if self.functions.contains_key(func_name) {
                let target_name = self.resolve_named_function_call_target(func_name, &call.args)?;
                if let Some(source_params) =
                    self.concrete_source_params_for_call_target(&target_name, &call.args)
                {
                    let has_function_param = source_params
                        .iter()
                        .any(|param| matches!(param, Type::Function(_, _)));
                    self.generate_call_args_with_source_params(&call.args, &source_params)?;
                    self.output
                        .push_str(&format!("    call ${}\n", target_name));
                    if let Some(sig) = self.functions.get(&target_name) {
                        if sig.result.is_none()
                            && has_function_param
                            && self.current_function != Some("main".to_string())
                        {
                            self.output.push_str("    i32.const 0\n");
                        }
                    }
                    return Ok(());
                }
            }
        }

        if let Expr::Ident(func_name) = &*call.function {
            if !self.functions.contains_key(func_name) && self.lookup_local(func_name).is_none() {
                if let Some(target_name) = self.resolve_method_call_target(func_name, &call.args)? {
                    let target_name =
                        self.specialize_method_call_target(target_name, &call.args)?;
                    self.generate_call_args_for_target(&call.args, &target_name)?;
                    self.output
                        .push_str(&format!("    call ${}\n", target_name));
                    if let Some(sig) = self.functions.get(&target_name) {
                        if sig.result.is_none() && self.current_function != Some("main".to_string())
                        {
                            self.output.push_str("    i32.const 0\n");
                        }
                    }
                    return Ok(());
                }
            }
        }

        // Handle function call
        if let Expr::Ident(func_name) = &*call.function {
            if func_name == "identity" {
                if call.args.len() != 1 {
                    return Err(CodeGenError::UnsupportedFeature(
                        "identity expects exactly one argument".to_string(),
                    ));
                }
                self.generate_expr(&call.args[0])?;
                return Ok(());
            }

            if self.functions.contains_key(func_name) {
                let target_name = self.resolve_named_function_call_target(func_name, &call.args)?;
                self.generate_call_args_for_target(&call.args, &target_name)?;
                self.output
                    .push_str(&format!("    call ${}\n", target_name));
            } else if self.lookup_local(func_name).is_some() {
                let abi = self.callable_abi_for_args(&call.function, &call.args)?;
                self.generate_call_args_with_source_params(&call.args, &abi.source_params)?;
                self.generate_callable_value_with_abi(&call.function, &abi)?;
                self.emit_typed_indirect_closure_call(&abi);
                return Ok(());
            } else {
                // Check if it's a method call
                if let Some(obj_expr) = call.args.first() {
                    // Try to determine the record type from the expression
                    if let Some(record_type) = self.get_expr_type(obj_expr) {
                        if let Some(methods) = self.methods.get(&record_type) {
                            if methods.contains_key(func_name) {
                                let mangled_name = format!("{}_{}", record_type, func_name);
                                let mangled_name =
                                    self.specialize_method_call_target(mangled_name, &call.args)?;
                                self.generate_call_args_for_target(&call.args, &mangled_name)?;
                                self.output
                                    .push_str(&format!("    call ${}\n", mangled_name));
                                return Ok(());
                            } else {
                                return Err(CodeGenError::UndefinedFunction(format!(
                                    "Method '{}' not found in record '{}'",
                                    func_name, record_type
                                )));
                            }
                        } else {
                            return Err(CodeGenError::UndefinedFunction(format!(
                                "No methods defined for record '{}'",
                                record_type
                            )));
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
                            return Err(CodeGenError::UnsupportedFeature(format!(
                                "Restrict OSV method call '{}' is ambiguous across records {:?}; receiver type could not be inferred",
                                func_name, found_records
                            )));
                        } else {
                            // Unique method - safe to call
                            let record_name = &found_records[0];
                            let mangled_name = format!("{}_{}", record_name, func_name);
                            let mangled_name =
                                self.specialize_method_call_target(mangled_name, &call.args)?;
                            self.generate_call_args_for_target(&call.args, &mangled_name)?;
                            self.output
                                .push_str(&format!("    call ${}\n", mangled_name));
                        }
                    }
                } else {
                    return Err(CodeGenError::UndefinedFunction(func_name.clone()));
                }
            }
        } else {
            // Non-identifier function expression (e.g., field access, or complex expression)
            // Generate the function expression to get the closure pointer
            let abi = self.callable_abi_for_args(&call.function, &call.args)?;
            self.generate_call_args_with_source_params(&call.args, &abi.source_params)?;
            self.generate_callable_value_with_abi(&call.function, &abi)?;
            self.emit_typed_indirect_closure_call(&abi);
        }

        Ok(())
    }

    fn specialize_method_call_target(
        &mut self,
        target_name: String,
        args: &[Box<Expr>],
    ) -> Result<String, CodeGenError> {
        if self
            .function_source_sigs
            .get(&target_name)
            .is_some_and(|sig| !sig.type_params.is_empty())
            && self.function_decls.contains_key(&target_name)
        {
            self.ensure_generic_function_call_specialization(&target_name, args, None)
        } else {
            Ok(target_name)
        }
    }

    fn generate_call_args_with_source_params(
        &mut self,
        args: &[Box<Expr>],
        source_params: &[Type],
    ) -> Result<(), CodeGenError> {
        for (arg, source_param) in args.iter().zip(source_params.iter()) {
            self.generate_expr_with_expected_source(arg, source_param)?;
        }
        Ok(())
    }

    fn method_receiver_record_name(&self, expr: &Expr) -> Option<String> {
        match self.infer_expr_source_type(expr) {
            Some(Type::Named(name)) if self.records.contains_key(&name) => return Some(name),
            Some(Type::Temporal(name, _)) if self.records.contains_key(&name) => return Some(name),
            _ => {}
        }

        self.get_expr_type(expr)
            .filter(|type_name| self.records.contains_key(type_name))
    }

    fn resolve_method_call_target(
        &self,
        method_name: &str,
        args: &[Box<Expr>],
    ) -> Result<Option<String>, CodeGenError> {
        let Some(receiver) = args.first() else {
            return Ok(None);
        };

        if let Some(record_name) = self.method_receiver_record_name(receiver) {
            if let Some(methods) = self.methods.get(&record_name) {
                if methods.contains_key(method_name) {
                    return Ok(Some(Self::method_function_name(&record_name, method_name)));
                }

                return Err(CodeGenError::UndefinedFunction(format!(
                    "Method '{}' not found in record '{}'",
                    method_name, record_name
                )));
            }

            return Err(CodeGenError::UndefinedFunction(format!(
                "No methods defined for record '{}'",
                record_name
            )));
        }

        let mut found_records = Vec::new();
        for (record_name, method_map) in &self.methods {
            if method_map.contains_key(method_name) {
                found_records.push(record_name.clone());
            }
        }

        match found_records.len() {
            0 => Ok(None),
            1 => Ok(Some(Self::method_function_name(
                &found_records[0],
                method_name,
            ))),
            _ => Err(CodeGenError::UnsupportedFeature(format!(
                "Restrict OSV method call '{}' is ambiguous across records {:?}; receiver type could not be inferred",
                method_name, found_records
            ))),
        }
    }

    fn generate_call_args_for_target(
        &mut self,
        args: &[Box<Expr>],
        target_name: &str,
    ) -> Result<(), CodeGenError> {
        if let Some(source_params) = self.concrete_source_params_for_call_target(target_name, args)
        {
            return self.generate_call_args_with_source_params(args, &source_params);
        }

        for arg in args {
            self.generate_expr(arg)?;
        }

        Ok(())
    }

    fn concrete_source_params_for_call_target(
        &self,
        target_name: &str,
        args: &[Box<Expr>],
    ) -> Option<Vec<Type>> {
        self.concrete_source_params_for_call_target_with_expected(target_name, args, None)
    }

    fn concrete_source_params_for_call_target_with_expected(
        &self,
        target_name: &str,
        args: &[Box<Expr>],
        expected_source: Option<&Type>,
    ) -> Option<Vec<Type>> {
        let sig = self.function_source_sigs.get(target_name)?;
        if sig.type_params.is_empty() {
            return Some(sig.params.clone());
        }

        if sig.params.len() != args.len() {
            return None;
        }

        let mut substitution = HashMap::new();
        for (param_ty, arg) in sig.params.iter().zip(args.iter()) {
            if let Some(arg_ty) = self.infer_expr_source_type(arg) {
                Self::bind_source_type_params(
                    param_ty,
                    &arg_ty,
                    &sig.type_params,
                    &mut substitution,
                );
            }
        }

        for (param_ty, arg) in sig.params.iter().zip(args.iter()) {
            if let Some(arg_ty) = self.infer_contextual_call_argument_source_type(
                param_ty,
                arg,
                &sig.type_params,
                &substitution,
            ) {
                Self::bind_source_type_params(
                    param_ty,
                    &arg_ty,
                    &sig.type_params,
                    &mut substitution,
                );
            }
        }

        if let (Some(result_template), Some(expected_source)) =
            (sig.result.as_ref(), expected_source)
        {
            Self::bind_source_type_params(
                result_template,
                expected_source,
                &sig.type_params,
                &mut substitution,
            );
        }

        sig.params
            .iter()
            .map(|param| {
                Self::substitute_source_type_params(param, &sig.type_params, &substitution)
            })
            .collect()
    }

    fn generate_expr_with_expected_source(
        &mut self,
        expr: &Expr,
        expected_source: &Type,
    ) -> Result<(), CodeGenError> {
        if let Expr::Call(call) = expr {
            if let Expr::Ident(func_name) = call.function.as_ref() {
                if let Some(target_name) = self.lookup_generic_function_alias(func_name) {
                    match target_name.as_str() {
                        "map" => return self.generate_map_call(call),
                        "filter" => return self.generate_filter_call(call),
                        "fold" => return self.generate_fold_call(call),
                        _ => {}
                    }
                }

                if func_name == "identity" {
                    if call.args.len() != 1 {
                        return Err(CodeGenError::UnsupportedFeature(
                            "identity expects exactly one argument".to_string(),
                        ));
                    }
                    return self.generate_expr_with_expected_source(&call.args[0], expected_source);
                }
            }
        }

        if let Expr::Pipe(pipe) = expr {
            let is_identity_target = match &pipe.target {
                PipeTarget::Ident(name) => name == "identity",
                PipeTarget::Expr(target) => {
                    matches!(target.as_ref(), Expr::Ident(name) if name == "identity")
                }
            };
            if is_identity_target {
                return self.generate_expr_with_expected_source(&pipe.expr, expected_source);
            }
        }

        if let Expr::Call(call) = expr {
            if let Expr::Ident(func_name) = call.function.as_ref() {
                if !matches!(func_name.as_str(), "map" | "filter" | "fold")
                    && self.functions.contains_key(func_name)
                {
                    let target_name = self.resolve_named_function_call_target_with_expected(
                        func_name,
                        &call.args,
                        Some(expected_source),
                    )?;
                    let has_function_param = self
                        .function_source_sigs
                        .get(&target_name)
                        .is_some_and(|sig| {
                            sig.params
                                .iter()
                                .any(|param| matches!(param, Type::Function(_, _)))
                        });
                    if target_name != *func_name
                        || has_function_param
                        || self.function_source_sigs.contains_key(&target_name)
                    {
                        if let Some(source_params) = self
                            .concrete_source_params_for_call_target_with_expected(
                                &target_name,
                                &call.args,
                                Some(expected_source),
                            )
                        {
                            self.generate_call_args_with_source_params(&call.args, &source_params)?;
                        } else {
                            self.generate_call_args_for_target(&call.args, &target_name)?;
                        }
                        self.output
                            .push_str(&format!("    call ${}\n", target_name));
                        if let Some(sig) = self.functions.get(&target_name) {
                            if sig.result.is_none()
                                && (target_name != *func_name || has_function_param)
                                && self.current_function != Some("main".to_string())
                            {
                                self.output.push_str("    i32.const 0\n");
                            }
                        }
                        return Ok(());
                    }
                }

                if !self.functions.contains_key(func_name) && self.lookup_local(func_name).is_none()
                {
                    if let Some(target_name) =
                        self.resolve_method_call_target(func_name, &call.args)?
                    {
                        let target_name =
                            self.specialize_method_call_target(target_name, &call.args)?;
                        if let Some(source_params) = self
                            .concrete_source_params_for_call_target_with_expected(
                                &target_name,
                                &call.args,
                                Some(expected_source),
                            )
                        {
                            self.generate_call_args_with_source_params(&call.args, &source_params)?;
                        } else {
                            self.generate_call_args_for_target(&call.args, &target_name)?;
                        }
                        self.output
                            .push_str(&format!("    call ${}\n", target_name));
                        if let Some(sig) = self.functions.get(&target_name) {
                            if sig.result.is_none()
                                && self.current_function != Some("main".to_string())
                            {
                                self.output.push_str("    i32.const 0\n");
                            }
                        }
                        return Ok(());
                    }
                }
            }

            if !matches!(
                call.function.as_ref(),
                Expr::Ident(func_name)
                    if matches!(func_name.as_str(), "map" | "filter" | "fold")
                        || self.functions.contains_key(func_name)
            ) {
                if let Ok(arg_source_tys) = call
                    .args
                    .iter()
                    .map(|arg| self.infer_expr_source_type_for_abi(arg))
                    .collect::<Result<Vec<_>, _>>()
                {
                    let abi = self.callable_abi_for_arg_sources_with_expected(
                        &call.function,
                        &arg_source_tys,
                        Some(expected_source),
                    )?;
                    self.generate_call_args_with_source_params(&call.args, &abi.source_params)?;
                    self.generate_callable_value_with_abi(&call.function, &abi)?;
                    self.emit_typed_indirect_closure_call(&abi);
                    return Ok(());
                }
            }
        }

        match expr {
            Expr::Match(match_expr) => {
                return self
                    .generate_match_expr_with_expected_source(match_expr, Some(expected_source));
            }
            Expr::Then(then) => {
                return self.generate_then_expr_with_expected_source(then, Some(expected_source));
            }
            Expr::With(with_expr) => {
                return self
                    .generate_with_expr_with_expected_source(with_expr, Some(expected_source));
            }
            Expr::RecordLit(record_lit) => {
                if self.source_record_name(expected_source) == Some(record_lit.name.as_str()) {
                    return self
                        .generate_record_literal_with_source_type(record_lit, expected_source);
                }
            }
            Expr::Some(inner) => {
                if let Type::Generic(name, args) = expected_source {
                    if name == "Option" {
                        let payload_source = args.first().ok_or_else(|| {
                            CodeGenError::UnsupportedType(
                                "Option requires one type argument".into(),
                            )
                        })?;
                        let payload_ty = self.convert_type(payload_source)?;
                        self.generate_expr_with_expected_source(inner, payload_source)?;
                        return self.generate_variant_from_stack("Some", 1, payload_ty);
                    }
                }
            }
            Expr::Ok(inner) => {
                if let Type::Generic(name, args) = expected_source {
                    if name == "Result" {
                        let payload_source = args.first().ok_or_else(|| {
                            CodeGenError::UnsupportedType(
                                "Result requires two type arguments".into(),
                            )
                        })?;
                        let payload_ty = self.convert_type(payload_source)?;
                        self.generate_expr_with_expected_source(inner, payload_source)?;
                        return self.generate_variant_from_stack("Ok", 1, payload_ty);
                    }
                }
            }
            Expr::Err(inner) => {
                if let Type::Generic(name, args) = expected_source {
                    if name == "Result" {
                        let payload_source = args.get(1).ok_or_else(|| {
                            CodeGenError::UnsupportedType(
                                "Result requires two type arguments".into(),
                            )
                        })?;
                        let payload_ty = self.convert_type(payload_source)?;
                        self.generate_expr_with_expected_source(inner, payload_source)?;
                        return self.generate_variant_from_stack("Err", 0, payload_ty);
                    }
                }
            }
            Expr::ListLit(items) => {
                if let Type::Generic(name, args) = expected_source {
                    if name == "Array" {
                        return self.generate_array_literal_with_expected(items, args.first());
                    }
                    if name == "List" {
                        return self.generate_list_literal_with_expected(items, args.first());
                    }
                }
            }
            Expr::ArrayLit(items) => {
                if let Type::Generic(name, args) = expected_source {
                    if name == "Array" {
                        return self.generate_array_literal_with_expected(items, args.first());
                    }
                }
            }
            Expr::RangeLit(range) => {
                if let Type::Generic(name, args) = expected_source {
                    if name == "Range"
                        && matches!(args.first(), Some(Type::Named(elem)) if elem == "Int32")
                    {
                        return self.generate_range_literal(range);
                    }
                    if name == "Range" {
                        return Err(CodeGenError::UnsupportedFeature(
                            "range literals currently support Range<Int32> only".to_string(),
                        ));
                    }
                }
            }
            _ => {}
        };

        if let Type::Function(params, return_type) = expected_source {
            let abi = self.source_function_abi(params, return_type)?;
            self.generate_callable_value_with_abi(expr, &abi)
        } else if let Expr::Block(block) = expr {
            self.generate_block_internal(block, true, Some(expected_source))
        } else if let Type::Named(name) = expected_source {
            match name.as_str() {
                "Int64" => self.generate_expr_with_wasm_type(expr, WasmType::I64),
                "Int32" => self.generate_expr_with_wasm_type(expr, WasmType::I32),
                "Float64" => self.generate_expr_with_wasm_type(expr, WasmType::F64),
                _ => self.generate_expr(expr),
            }
        } else {
            self.generate_expr(expr)
        }
    }

    fn resolve_builtin_abi_function(&self, func_name: &str, args: &[Box<Expr>]) -> String {
        match func_name {
            "list_get" => match args
                .first()
                .and_then(|arg| self.indexed_collection_element_source_type(arg, "List"))
            {
                Some(Type::Named(name)) if name == "Float64" => "list_get_f64".to_string(),
                Some(Type::Named(name)) if name == "Int64" => "list_get_i64".to_string(),
                _ => func_name.to_string(),
            },
            "list_head" => match args
                .first()
                .and_then(|arg| self.indexed_collection_element_source_type(arg, "List"))
            {
                Some(Type::Named(name)) if name == "Float64" => "list_head_f64".to_string(),
                Some(Type::Named(name)) if name == "Int64" => "list_head_i64".to_string(),
                _ => func_name.to_string(),
            },
            "list_tail" => match args
                .first()
                .and_then(|arg| self.indexed_collection_element_source_type(arg, "List"))
            {
                Some(Type::Named(name)) if name == "Float64" => "list_tail_f64".to_string(),
                Some(Type::Named(name)) if name == "Int64" => "list_tail_i64".to_string(),
                _ => func_name.to_string(),
            },
            "tail" => match args
                .first()
                .and_then(|arg| self.indexed_collection_element_source_type(arg, "List"))
            {
                Some(Type::Named(name)) if name == "Float64" => "tail_f64".to_string(),
                Some(Type::Named(name)) if name == "Int64" => "tail_i64".to_string(),
                _ => func_name.to_string(),
            },
            "list_reverse" => match args
                .first()
                .and_then(|arg| self.indexed_collection_element_source_type(arg, "List"))
            {
                Some(Type::Named(name)) if name == "Float64" => "list_reverse_f64".to_string(),
                Some(Type::Named(name)) if name == "Int64" => "list_reverse_i64".to_string(),
                _ => func_name.to_string(),
            },
            "list_append" => match args
                .first()
                .and_then(|arg| self.indexed_collection_element_source_type(arg, "List"))
            {
                Some(Type::Named(name)) if name == "Float64" => "list_append_f64".to_string(),
                Some(Type::Named(name)) if name == "Int64" => "list_append_i64".to_string(),
                _ => func_name.to_string(),
            },
            "list_prepend" => match args
                .get(1)
                .and_then(|arg| self.indexed_collection_element_source_type(arg, "List"))
            {
                Some(Type::Named(name)) if name == "Float64" => "list_prepend_f64".to_string(),
                Some(Type::Named(name)) if name == "Int64" => "list_prepend_i64".to_string(),
                _ => func_name.to_string(),
            },
            "list_concat" => match args
                .first()
                .and_then(|arg| self.indexed_collection_element_source_type(arg, "List"))
            {
                Some(Type::Named(name)) if name == "Float64" => "list_concat_f64".to_string(),
                Some(Type::Named(name)) if name == "Int64" => "list_concat_i64".to_string(),
                _ => func_name.to_string(),
            },
            "array_get" => match args
                .first()
                .and_then(|arg| self.indexed_collection_element_source_type(arg, "Array"))
            {
                Some(Type::Named(name)) if name == "Float64" => "array_get_f64".to_string(),
                Some(Type::Named(name)) if name == "Int64" => "array_get_i64".to_string(),
                _ => func_name.to_string(),
            },
            "array_set" => match args
                .first()
                .and_then(|arg| self.indexed_collection_element_source_type(arg, "Array"))
                .or_else(|| args.get(2).and_then(|arg| self.infer_expr_source_type(arg)))
            {
                Some(Type::Named(name)) if name == "Float64" => "array_set_f64".to_string(),
                Some(Type::Named(name)) if name == "Int64" => "array_set_i64".to_string(),
                _ => func_name.to_string(),
            },
            "option_unwrap_or" => {
                let payload_ty = args
                    .get(1)
                    .and_then(|arg| self.infer_expr_source_type(arg))
                    .or_else(|| {
                        args.first()
                            .and_then(|arg| self.option_payload_source_type(arg))
                    });
                match payload_ty {
                    Some(Type::Named(name)) if name == "Float64" => {
                        "option_unwrap_or_f64".to_string()
                    }
                    _ => func_name.to_string(),
                }
            }
            _ => func_name.to_string(),
        }
    }

    fn resolve_named_function_call_target(
        &mut self,
        func_name: &str,
        args: &[Box<Expr>],
    ) -> Result<String, CodeGenError> {
        self.resolve_named_function_call_target_with_expected(func_name, args, None)
    }

    fn resolve_named_function_call_target_with_expected(
        &mut self,
        func_name: &str,
        args: &[Box<Expr>],
        expected_source: Option<&Type>,
    ) -> Result<String, CodeGenError> {
        let builtin_target = self.resolve_builtin_abi_function(func_name, args);
        if builtin_target != func_name {
            return Ok(builtin_target);
        }

        if self
            .function_source_sigs
            .get(func_name)
            .is_some_and(|sig| !sig.type_params.is_empty())
            && self.function_decls.contains_key(func_name)
        {
            return self.ensure_generic_function_call_specialization(
                func_name,
                args,
                expected_source,
            );
        }

        Ok(func_name.to_string())
    }

    fn ensure_generic_function_call_specialization(
        &mut self,
        function_name: &str,
        args: &[Box<Expr>],
        expected_source: Option<&Type>,
    ) -> Result<String, CodeGenError> {
        let source_sig = self
            .function_source_sigs
            .get(function_name)
            .ok_or_else(|| CodeGenError::UndefinedFunction(function_name.to_string()))?
            .clone();

        if source_sig.params.len() != args.len() {
            return Err(CodeGenError::UnsupportedFeature(format!(
                "generic function '{}' specialization expected {} arguments, found {}",
                function_name,
                source_sig.params.len(),
                args.len()
            )));
        }

        let mut substitution = HashMap::new();
        for (param_ty, arg) in source_sig.params.iter().zip(args.iter()) {
            if let Some(arg_ty) = self.infer_expr_source_type(arg) {
                Self::bind_source_type_params(
                    param_ty,
                    &arg_ty,
                    &source_sig.type_params,
                    &mut substitution,
                );
            }
        }

        for (param_ty, arg) in source_sig.params.iter().zip(args.iter()) {
            if let Some(arg_ty) = self.infer_contextual_call_argument_source_type(
                param_ty,
                arg,
                &source_sig.type_params,
                &substitution,
            ) {
                Self::bind_source_type_params(
                    param_ty,
                    &arg_ty,
                    &source_sig.type_params,
                    &mut substitution,
                );
            }
        }

        if let (Some(result_template), Some(expected_source)) =
            (source_sig.result.as_ref(), expected_source)
        {
            Self::bind_source_type_params(
                result_template,
                expected_source,
                &source_sig.type_params,
                &mut substitution,
            );
        }

        self.ensure_generic_function_specialization_with_substitution(
            function_name,
            &source_sig,
            substitution,
        )
    }

    fn infer_contextual_call_argument_source_type(
        &self,
        template: &Type,
        arg: &Expr,
        type_params: &[String],
        substitution: &HashMap<String, Type>,
    ) -> Option<Type> {
        let contextual_template =
            Self::substitute_source_type_params_partial(template, type_params, substitution);
        match contextual_template {
            Type::Function(params, _) => {
                let return_ty = self.infer_callable_return_source_type(arg, &params)?;
                Some(Type::Function(params, Box::new(return_ty)))
            }
            _ => self.infer_expr_source_type(arg),
        }
    }

    fn ensure_generic_function_specialization_for_abi(
        &mut self,
        function_name: &str,
        abi: &LambdaAbiContext,
    ) -> Result<String, CodeGenError> {
        let source_sig = self
            .function_source_sigs
            .get(function_name)
            .ok_or_else(|| CodeGenError::UndefinedFunction(function_name.to_string()))?
            .clone();

        if source_sig.type_params.is_empty() {
            return Ok(function_name.to_string());
        }

        if source_sig.params.len() != abi.source_params.len() {
            return Err(CodeGenError::UnsupportedFeature(format!(
                "generic function '{}' specialization expected {} argument types, found {}",
                function_name,
                source_sig.params.len(),
                abi.source_params.len()
            )));
        }

        let mut substitution = HashMap::new();
        for (param_ty, arg_ty) in source_sig.params.iter().zip(abi.source_params.iter()) {
            Self::bind_source_type_params(
                param_ty,
                arg_ty,
                &source_sig.type_params,
                &mut substitution,
            );
        }
        if let Some(result_template) = source_sig.result.as_ref() {
            Self::bind_source_type_params(
                result_template,
                &abi.source_result,
                &source_sig.type_params,
                &mut substitution,
            );
        }

        self.ensure_generic_function_specialization_with_substitution(
            function_name,
            &source_sig,
            substitution,
        )
    }

    fn ensure_generic_function_specialization_with_substitution(
        &mut self,
        function_name: &str,
        source_sig: &FunctionSourceSig,
        substitution: HashMap<String, Type>,
    ) -> Result<String, CodeGenError> {
        for type_param in &source_sig.type_params {
            if !substitution.contains_key(type_param) {
                return Err(CodeGenError::UnsupportedFeature(format!(
                    "generic function '{}' requires an explicit argument type for '{}'",
                    function_name, type_param
                )));
            }
        }

        let func = self
            .function_decls
            .get(function_name)
            .cloned()
            .ok_or_else(|| {
                CodeGenError::UnsupportedFeature(format!(
                    "generic function '{}' cannot be specialized without a source declaration",
                    function_name
                ))
            })?;

        let specialized_name = Self::generic_specialization_name(
            function_name,
            &source_sig.type_params,
            &substitution,
        );
        if self.specialized_functions.contains(&specialized_name) {
            return Ok(specialized_name);
        }

        let specialized_params = func
            .params
            .iter()
            .map(|param| {
                Ok(Param {
                    name: param.name.clone(),
                    ty: Self::substitute_source_type_params(
                        &param.ty,
                        &source_sig.type_params,
                        &substitution,
                    )
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedFeature(format!(
                            "generic function '{}' has unresolved parameter type '{}'",
                            function_name, param.name
                        ))
                    })?,
                    context_bound: param.context_bound.clone(),
                })
            })
            .collect::<Result<Vec<_>, CodeGenError>>()?;
        let specialized_return_type = func
            .return_type
            .as_ref()
            .map(|return_type| {
                Self::substitute_source_type_params(
                    return_type,
                    &source_sig.type_params,
                    &substitution,
                )
                .ok_or_else(|| {
                    CodeGenError::UnsupportedFeature(format!(
                        "generic function '{}' has an unresolved return type",
                        function_name
                    ))
                })
            })
            .transpose()?;

        let specialized_func = FunDecl {
            name: specialized_name.clone(),
            is_async: func.is_async,
            type_params: vec![],
            temporal_constraints: func.temporal_constraints.clone(),
            params: specialized_params,
            return_type: specialized_return_type,
            body: func.body.clone(),
        };

        self.register_function_signature(&specialized_func)?;
        self.specialized_functions.insert(specialized_name.clone());

        let outer_output = std::mem::take(&mut self.output);
        let outer_current_function = self.current_function.clone();
        let result = self.generate_function(&specialized_func);
        let generated_function = std::mem::take(&mut self.output);
        self.output = outer_output;
        self.current_function = outer_current_function;
        result?;
        self.lambda_functions.push(generated_function);

        Ok(specialized_name)
    }

    fn generic_specialization_name(
        function_name: &str,
        type_params: &[String],
        substitution: &HashMap<String, Type>,
    ) -> String {
        let suffix = type_params
            .iter()
            .filter_map(|param| substitution.get(param))
            .map(Self::source_type_suffix)
            .collect::<Vec<_>>()
            .join("_");
        format!("{}__{}", Self::sanitize_wasm_name(function_name), suffix)
    }

    fn source_type_suffix(ty: &Type) -> String {
        match ty {
            Type::Named(name) => Self::sanitize_wasm_name(name),
            Type::Generic(name, params) => {
                let mut parts = vec![Self::sanitize_wasm_name(name)];
                parts.extend(params.iter().map(Self::source_type_suffix));
                parts.join("_")
            }
            Type::Function(params, return_type) => {
                let mut parts = vec!["fn".to_string()];
                parts.extend(params.iter().map(Self::source_type_suffix));
                parts.push("to".to_string());
                parts.push(Self::source_type_suffix(return_type));
                parts.join("_")
            }
            Type::Temporal(name, temporals) => {
                let mut parts = vec![Self::sanitize_wasm_name(name)];
                parts.extend(
                    temporals
                        .iter()
                        .map(|temporal| Self::sanitize_wasm_name(temporal)),
                );
                parts.join("_")
            }
        }
    }

    fn substitute_source_type_params_partial(
        ty: &Type,
        type_params: &[String],
        substitution: &HashMap<String, Type>,
    ) -> Type {
        match ty {
            Type::Named(name) if type_params.iter().any(|param| param == name) => substitution
                .get(name)
                .cloned()
                .unwrap_or_else(|| ty.clone()),
            Type::Named(_) => ty.clone(),
            Type::Generic(name, params) => Type::Generic(
                name.clone(),
                params
                    .iter()
                    .map(|param| {
                        Self::substitute_source_type_params_partial(
                            param,
                            type_params,
                            substitution,
                        )
                    })
                    .collect(),
            ),
            Type::Function(params, return_type) => Type::Function(
                params
                    .iter()
                    .map(|param| {
                        Self::substitute_source_type_params_partial(
                            param,
                            type_params,
                            substitution,
                        )
                    })
                    .collect(),
                Box::new(Self::substitute_source_type_params_partial(
                    return_type,
                    type_params,
                    substitution,
                )),
            ),
            Type::Temporal(name, temporals) if type_params.iter().any(|param| param == name) => {
                substitution
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| Type::Temporal(name.clone(), temporals.clone()))
            }
            Type::Temporal(name, temporals) => Type::Temporal(name.clone(), temporals.clone()),
        }
    }

    fn sanitize_wasm_name(name: &str) -> String {
        name.chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '_' {
                    ch
                } else {
                    '_'
                }
            })
            .collect()
    }

    fn indexed_collection_element_source_type(
        &self,
        collection: &Expr,
        collection_name: &str,
    ) -> Option<Type> {
        match self.infer_expr_source_type(collection) {
            Some(Type::Generic(name, params)) if name == collection_name => params.first().cloned(),
            _ => None,
        }
    }

    fn option_payload_source_type(&self, option: &Expr) -> Option<Type> {
        match self.infer_expr_source_type(option) {
            Some(Type::Generic(name, params)) if name == "Option" => params.first().cloned(),
            _ => None,
        }
    }

    #[allow(dead_code)]
    fn emit_indirect_closure_call(&mut self, arg_count: usize) {
        self.has_indirect_closure_call = true;
        self.output.push_str("    local.set $closure_tmp\n");
        self.output.push_str("    local.get $closure_tmp\n");
        self.output.push_str("    local.get $closure_tmp\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str(&format!(
            "    call_indirect (type $closure_call_{})\n",
            arg_count
        ));
    }

    fn emit_typed_indirect_closure_call(&mut self, abi: &LambdaAbiContext) {
        self.has_indirect_closure_call = true;
        let type_name = self.closure_call_type_name(&abi.params, abi.result);
        self.output.push_str("    local.set $closure_tmp\n");
        self.output.push_str("    local.get $closure_tmp\n");
        self.output.push_str("    local.get $closure_tmp\n");
        self.output.push_str("    i32.load\n");
        self.output
            .push_str(&format!("    call_indirect (type ${})\n", type_name));
    }

    fn emit_iter_func_call(&mut self, arg_types: &[WasmType], result_type: WasmType, indent: &str) {
        self.has_indirect_closure_call = true;
        let type_name = self.closure_call_type_name(arg_types, result_type);
        self.output
            .push_str(&format!("{indent}local.get $iter_func\n"));
        self.output
            .push_str(&format!("{indent}local.get $iter_func\n"));
        self.output.push_str(&format!("{indent}i32.load\n"));
        self.output
            .push_str(&format!("{indent}call_indirect (type ${})\n", type_name));
    }

    fn source_function_abi(
        &self,
        params: &[Type],
        return_type: &Type,
    ) -> Result<LambdaAbiContext, CodeGenError> {
        Ok(LambdaAbiContext {
            params: params
                .iter()
                .map(|param| self.convert_type(param))
                .collect::<Result<Vec<_>, _>>()?,
            result: self.convert_type(return_type)?,
            source_params: params.to_vec(),
            source_result: return_type.clone(),
        })
    }

    fn callable_abi_for_arg_sources(
        &self,
        callable: &Expr,
        arg_source_tys: &[Type],
    ) -> Result<LambdaAbiContext, CodeGenError> {
        self.callable_abi_for_arg_sources_with_expected(callable, arg_source_tys, None)
    }

    fn callable_abi_for_arg_sources_with_expected(
        &self,
        callable: &Expr,
        arg_source_tys: &[Type],
        expected_result_source: Option<&Type>,
    ) -> Result<LambdaAbiContext, CodeGenError> {
        if let Expr::Ident(name) = callable {
            if name == "identity" && arg_source_tys.len() == 1 {
                return self.source_function_abi(arg_source_tys, &arg_source_tys[0]);
            }

            if let Some(Type::Function(params, return_type)) = self.lookup_local_source_type(name) {
                if params.len() == arg_source_tys.len() {
                    return self.source_function_abi(&params, &return_type);
                }
            }

            if let Some(function_name) = self.lookup_generic_function_alias(name) {
                if function_name == "identity" && arg_source_tys.len() == 1 {
                    return self.source_function_abi(arg_source_tys, &arg_source_tys[0]);
                }
                return self.named_function_abi_for_arg_sources_with_expected(
                    &function_name,
                    arg_source_tys,
                    expected_result_source,
                );
            }

            if let Some(callable) = self.lookup_deferred_lambda_alias(name) {
                let result_source = self
                    .infer_callable_return_source_type(&callable, arg_source_tys)
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedFeature(format!(
                            "deferred callable '{}' requires a concrete return type for function value use",
                            name
                        ))
                    })?;
                return self.source_function_abi(arg_source_tys, &result_source);
            }

            if self.functions.contains_key(name) {
                return if expected_result_source.is_some() {
                    self.named_function_abi_for_arg_sources_with_expected(
                        name,
                        arg_source_tys,
                        expected_result_source,
                    )
                } else {
                    self.named_function_abi_from_source_signature(name)
                };
            }
        }

        if let Some(Type::Function(params, return_type)) = self.infer_expr_source_type(callable) {
            if params.len() == arg_source_tys.len() {
                return self.source_function_abi(&params, &return_type);
            }
        }

        if matches!(callable, Expr::Lambda(_) | Expr::Then(_) | Expr::Match(_)) {
            let result_source = self
                .infer_callable_return_source_type(callable, arg_source_tys)
                .or_else(|| expected_result_source.cloned())
                .ok_or_else(|| {
                    CodeGenError::UnsupportedFeature(
                        "typed callable call requires an inferable return type".to_string(),
                    )
                })?;
            return self.source_function_abi(arg_source_tys, &result_source);
        }

        Err(CodeGenError::UnsupportedFeature(
            "function value call requires a known function type".to_string(),
        ))
    }

    fn callable_abi_for_args(
        &self,
        callable: &Expr,
        args: &[Box<Expr>],
    ) -> Result<LambdaAbiContext, CodeGenError> {
        let arg_source_tys = args
            .iter()
            .map(|arg| self.infer_expr_source_type_for_abi(arg))
            .collect::<Result<Vec<_>, _>>()?;

        self.callable_abi_for_arg_sources(callable, &arg_source_tys)
    }

    fn generate_callable_value_with_abi(
        &mut self,
        callable: &Expr,
        abi: &LambdaAbiContext,
    ) -> Result<(), CodeGenError> {
        if let Expr::Ident(name) = callable {
            if let Some(function_name) = self.lookup_generic_function_alias(name) {
                if function_name == "identity" {
                    return self.generate_identity_function_reference_with_abi(abi);
                }
                return self.generate_named_function_reference_with_abi(&function_name, abi);
            }

            if let Some(lambda) = self.lookup_deferred_lambda_alias(name) {
                return self.generate_lambda_argument(
                    &lambda,
                    abi.params.clone(),
                    abi.result,
                    abi.source_params.clone(),
                    abi.source_result.clone(),
                );
            }

            if self.functions.contains_key(name) {
                return self.generate_named_function_reference_with_abi(name, abi);
            }
        }

        if matches!(callable, Expr::Ident(name) if name == "identity") {
            return self.generate_identity_function_reference_with_abi(abi);
        }

        if matches!(callable, Expr::Lambda(_)) {
            return self.generate_lambda_argument(
                callable,
                abi.params.clone(),
                abi.result,
                abi.source_params.clone(),
                abi.source_result.clone(),
            );
        }

        self.lambda_abi_stack.push(abi.clone());
        let expected_source = Type::Function(
            abi.source_params.clone(),
            Box::new(abi.source_result.clone()),
        );
        let result = if matches!(callable, Expr::Then(_) | Expr::Match(_)) {
            self.generate_expr_with_expected_source(callable, &expected_source)
        } else {
            self.generate_expr(callable)
        };
        self.lambda_abi_stack.pop();
        result
    }

    fn generate_named_function_reference(
        &mut self,
        function_name: &str,
    ) -> Result<(), CodeGenError> {
        let abi = if let Some(context) = self.lambda_abi_stack.last() {
            context.clone()
        } else {
            self.named_function_abi_from_source_signature(function_name)?
        };

        self.generate_named_function_reference_with_abi(function_name, &abi)
    }

    fn generate_identity_function_reference_with_abi(
        &mut self,
        abi: &LambdaAbiContext,
    ) -> Result<(), CodeGenError> {
        if abi.params.len() != 1 || abi.params[0] != abi.result {
            return Err(CodeGenError::UnsupportedFeature(
                "identity function values require matching single-argument input and output types"
                    .to_string(),
            ));
        }

        self.ensure_supported_closure_wasm_type(abi.result, "identity result")?;
        self.ensure_supported_closure_wasm_type(abi.params[0], "identity parameter")?;

        let thunk_name = format!("fnref_identity_{}", self.lambda_counter);
        self.lambda_counter += 1;
        let table_index = self.function_table.len();
        self.function_table.push(thunk_name.clone());

        self.output
            .push_str("    i32.const 4 ;; identity closure size\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $closure_tmp\n");
        self.output.push_str("    local.get $closure_tmp\n");
        self.output.push_str(&format!(
            "    i32.const {} ;; function table index for {}\n",
            table_index, thunk_name
        ));
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $closure_tmp\n");

        let param_ty = abi.params[0];
        let mut thunk_code = String::new();
        thunk_code.push_str(&format!(
            "  (func ${} (param $arg0 {}) (param $closure i32) (result {})\n",
            thunk_name,
            self.wasm_type_str(param_ty),
            self.wasm_type_str(abi.result)
        ));
        thunk_code.push_str("    local.get $arg0\n");
        thunk_code.push_str("  )\n");
        self.lambda_functions.push(thunk_code);

        Ok(())
    }

    fn named_function_abi_from_source_signature(
        &self,
        function_name: &str,
    ) -> Result<LambdaAbiContext, CodeGenError> {
        let source_sig = self
            .function_source_sigs
            .get(function_name)
            .ok_or_else(|| CodeGenError::UndefinedFunction(function_name.to_string()))?;

        if !source_sig.type_params.is_empty() {
            return Err(CodeGenError::UnsupportedFeature(format!(
                "generic function '{}' cannot be used as a runtime function value yet; use an explicit lambda to instantiate it",
                function_name
            )));
        }

        let result_source = source_sig
            .result
            .clone()
            .unwrap_or_else(|| Type::Named("Unit".to_string()));

        Ok(LambdaAbiContext {
            params: source_sig
                .params
                .iter()
                .map(|param| self.convert_type(param))
                .collect::<Result<Vec<_>, _>>()?,
            result: self.convert_type(&result_source)?,
            source_params: source_sig.params.clone(),
            source_result: result_source,
        })
    }

    fn named_function_abi_for_arg_sources_with_expected(
        &self,
        function_name: &str,
        arg_source_tys: &[Type],
        expected_result_source: Option<&Type>,
    ) -> Result<LambdaAbiContext, CodeGenError> {
        let source_sig = self
            .function_source_sigs
            .get(function_name)
            .ok_or_else(|| CodeGenError::UndefinedFunction(function_name.to_string()))?;

        if source_sig.params.len() != arg_source_tys.len() {
            return Err(CodeGenError::UnsupportedFeature(format!(
                "function '{}' expected {} arguments, found {}",
                function_name,
                source_sig.params.len(),
                arg_source_tys.len()
            )));
        }

        let result_source = if source_sig.type_params.is_empty() {
            source_sig
                .result
                .clone()
                .unwrap_or_else(|| Type::Named("Unit".to_string()))
        } else {
            let mut substitution = HashMap::new();
            for (param_ty, arg_ty) in source_sig.params.iter().zip(arg_source_tys.iter()) {
                Self::bind_source_type_params(
                    param_ty,
                    arg_ty,
                    &source_sig.type_params,
                    &mut substitution,
                );
            }

            let result_template = source_sig
                .result
                .as_ref()
                .cloned()
                .unwrap_or_else(|| Type::Named("Unit".to_string()));
            if let Some(expected_result_source) = expected_result_source {
                Self::bind_source_type_params(
                    &result_template,
                    expected_result_source,
                    &source_sig.type_params,
                    &mut substitution,
                );
            }
            Self::substitute_source_type_params(
                &result_template,
                &source_sig.type_params,
                &substitution,
            )
            .ok_or_else(|| {
                CodeGenError::UnsupportedFeature(format!(
                    "generic function '{}' requires a concrete result type for function value use",
                    function_name
                ))
            })?
        };

        self.source_function_abi(arg_source_tys, &result_source)
    }

    fn generate_named_function_reference_with_abi(
        &mut self,
        function_name: &str,
        abi: &LambdaAbiContext,
    ) -> Result<(), CodeGenError> {
        let source_sig = self.function_source_sigs.get(function_name).cloned();
        let target_function_name = if source_sig
            .as_ref()
            .is_some_and(|sig| !sig.type_params.is_empty())
        {
            self.ensure_generic_function_specialization_for_abi(function_name, abi)?
        } else {
            function_name.to_string()
        };
        let target_returns_unit = self
            .functions
            .get(&target_function_name)
            .is_some_and(|sig| sig.result.is_none());

        self.ensure_supported_closure_wasm_type(abi.result, "named function result")?;
        for param_ty in &abi.params {
            self.ensure_supported_closure_wasm_type(*param_ty, "named function parameter")?;
        }

        let thunk_name = format!("fnref_{}_{}", target_function_name, self.lambda_counter);
        self.lambda_counter += 1;
        let table_index = self.function_table.len();
        self.function_table.push(thunk_name.clone());

        self.output
            .push_str("    i32.const 4 ;; named function closure size\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $closure_tmp\n");
        self.output.push_str("    local.get $closure_tmp\n");
        self.output.push_str(&format!(
            "    i32.const {} ;; function table index for {}\n",
            table_index, thunk_name
        ));
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $closure_tmp\n");

        let mut thunk_code = String::new();
        thunk_code.push_str(&format!("  (func ${}", thunk_name));
        for (idx, param_ty) in abi.params.iter().enumerate() {
            thunk_code.push_str(&format!(
                " (param $arg{} {})",
                idx,
                self.wasm_type_str(*param_ty)
            ));
        }
        thunk_code.push_str(" (param $closure i32)");
        thunk_code.push_str(&format!(" (result {})\n", self.wasm_type_str(abi.result)));
        for idx in 0..abi.params.len() {
            thunk_code.push_str(&format!("    local.get $arg{}\n", idx));
        }
        thunk_code.push_str(&format!("    call ${}\n", target_function_name));
        if target_returns_unit {
            thunk_code.push_str("    i32.const 0\n");
        }
        thunk_code.push_str("  )\n");
        self.lambda_functions.push(thunk_code);

        Ok(())
    }

    fn generate_lambda_argument(
        &mut self,
        lambda_expr: &Expr,
        params: Vec<WasmType>,
        result: WasmType,
        source_params: Vec<Type>,
        source_result: Type,
    ) -> Result<(), CodeGenError> {
        if let Expr::Ident(name) = lambda_expr {
            if let Some(function_name) = self.lookup_generic_function_alias(name) {
                if function_name == "identity" {
                    let abi = LambdaAbiContext {
                        params,
                        result,
                        source_params,
                        source_result,
                    };
                    return self.generate_identity_function_reference_with_abi(&abi);
                }
                let abi = LambdaAbiContext {
                    params,
                    result,
                    source_params,
                    source_result,
                };
                return self.generate_named_function_reference_with_abi(&function_name, &abi);
            }

            if let Some(lambda_expr) = self.lookup_deferred_lambda_alias(name) {
                return self.generate_lambda_argument(
                    &lambda_expr,
                    params,
                    result,
                    source_params,
                    source_result,
                );
            }

            if self.functions.contains_key(name) {
                let abi = LambdaAbiContext {
                    params,
                    result,
                    source_params,
                    source_result,
                };
                return self.generate_named_function_reference_with_abi(name, &abi);
            }
        }

        if matches!(lambda_expr, Expr::Ident(name) if name == "identity") {
            let abi = LambdaAbiContext {
                params,
                result,
                source_params,
                source_result,
            };
            return self.generate_identity_function_reference_with_abi(&abi);
        }

        let expected_source =
            Type::Function(source_params.clone(), Box::new(source_result.clone()));
        self.lambda_abi_stack.push(LambdaAbiContext {
            params,
            result,
            source_params,
            source_result,
        });
        let result = if matches!(lambda_expr, Expr::Then(_) | Expr::Match(_)) {
            self.generate_expr_with_expected_source(lambda_expr, &expected_source)
        } else {
            self.generate_expr(lambda_expr)
        };
        self.lambda_abi_stack.pop();
        result
    }

    fn generate_map_call(&mut self, call: &CallExpr) -> Result<(), CodeGenError> {
        if call.args.len() != 2 {
            return Err(CodeGenError::UnsupportedFeature(
                "map expects container and mapper arguments".to_string(),
            ));
        }

        let item_source_ty = self.iteration_item_source_type(call, "map")?;
        let item_ty = self.convert_type(&item_source_ty)?;
        let result_source_ty = self
            .infer_map_result_source_type(call)
            .unwrap_or_else(|| item_source_ty.clone());
        let result_ty = self.convert_type(&result_source_ty)?;
        self.ensure_supported_closure_wasm_type(item_ty, "map input")?;
        self.ensure_supported_closure_wasm_type(result_ty, "map mapper result")?;

        match self.iteration_input_kind(call, "map")? {
            IterationInputKind::Option => self.generate_option_map_call(
                call,
                &item_source_ty,
                item_ty,
                result_source_ty,
                result_ty,
            ),
            IterationInputKind::List | IterationInputKind::Unknown => self.generate_list_map_call(
                call,
                &item_source_ty,
                item_ty,
                result_source_ty,
                result_ty,
            ),
        }
    }

    fn generate_list_map_call(
        &mut self,
        call: &CallExpr,
        item_source_ty: &Type,
        item_ty: WasmType,
        result_source_ty: Type,
        result_ty: WasmType,
    ) -> Result<(), CodeGenError> {
        let result_size = self.wasm_type_size(result_ty);
        self.output.push_str("    ;; map(list, mapper)\n");
        self.generate_expr(&call.args[0])?;
        self.output.push_str("    local.set $iter_list\n");
        self.generate_lambda_argument(
            &call.args[1],
            vec![item_ty],
            result_ty,
            vec![item_source_ty.clone()],
            result_source_ty,
        )?;
        self.output.push_str("    local.set $iter_func\n");

        self.output.push_str("    local.get $iter_list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $iter_len\n");

        self.output.push_str("    local.get $iter_len\n");
        self.output
            .push_str(&format!("    i32.const {}\n", result_size));
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $iter_out\n");

        self.store_iter_out_header("iter_len")?;
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    local.set $iter_index\n");

        self.output.push_str("    (loop $map_loop\n");
        self.output.push_str("      local.get $iter_index\n");
        self.output.push_str("      local.get $iter_len\n");
        self.output.push_str("      i32.lt_u\n");
        self.output.push_str("      (if\n");
        self.output.push_str("        (then\n");
        self.store_current_iter_output_address("iter_index", result_ty)?;
        self.emit_current_iter_value(item_ty)?;
        self.emit_iter_func_call(&[item_ty], result_ty, "          ");
        self.output.push_str(&format!(
            "          {}\n",
            self.wasm_store_op_for_wasm_type(result_ty)
        ));
        self.increment_local("iter_index")?;
        self.output.push_str("          br $map_loop\n");
        self.output.push_str("        )\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output.push_str("    local.get $iter_out\n");

        Ok(())
    }

    fn generate_option_map_call(
        &mut self,
        call: &CallExpr,
        item_source_ty: &Type,
        item_ty: WasmType,
        result_source_ty: Type,
        result_ty: WasmType,
    ) -> Result<(), CodeGenError> {
        self.output.push_str("    ;; map(option, mapper)\n");
        self.generate_expr(&call.args[0])?;
        self.output.push_str("    local.set $match_tmp\n");
        self.generate_lambda_argument(
            &call.args[1],
            vec![item_ty],
            result_ty,
            vec![item_source_ty.clone()],
            result_source_ty,
        )?;
        self.output.push_str("    local.set $iter_func\n");

        self.output.push_str("    local.get $match_tmp\n");
        self.output.push_str("    i32.load ;; load Option tag\n");
        self.output.push_str("    i32.const 1 ;; Some tag\n");
        self.output.push_str("    i32.eq\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        local.get $match_tmp\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str(&format!(
            "        {} ;; load Option payload\n",
            self.wasm_load_op_for_wasm_type(item_ty)
        ));
        self.emit_iter_func_call(&[item_ty], result_ty, "        ");
        self.generate_variant_from_stack("Some", 1, result_ty)?;
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.generate_none_value_with_temp("option_value_tmp");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");

        Ok(())
    }

    fn generate_filter_call(&mut self, call: &CallExpr) -> Result<(), CodeGenError> {
        if call.args.len() != 2 {
            return Err(CodeGenError::UnsupportedFeature(
                "filter expects container and predicate arguments".to_string(),
            ));
        }

        let item_source_ty = self.iteration_item_source_type(call, "filter")?;
        let item_ty = self.convert_type(&item_source_ty)?;
        self.ensure_supported_closure_wasm_type(item_ty, "filter input")?;

        match self.iteration_input_kind(call, "filter")? {
            IterationInputKind::Option => {
                self.generate_option_filter_call(call, &item_source_ty, item_ty)
            }
            IterationInputKind::List | IterationInputKind::Unknown => {
                self.generate_list_filter_call(call, &item_source_ty, item_ty)
            }
        }
    }

    fn generate_list_filter_call(
        &mut self,
        call: &CallExpr,
        item_source_ty: &Type,
        item_ty: WasmType,
    ) -> Result<(), CodeGenError> {
        let item_size = self.wasm_type_size(item_ty);
        let value_local = self.iter_value_local(item_ty);
        self.output.push_str("    ;; filter(list, predicate)\n");
        self.generate_expr(&call.args[0])?;
        self.output.push_str("    local.set $iter_list\n");
        self.generate_lambda_argument(
            &call.args[1],
            vec![item_ty],
            WasmType::I32,
            vec![item_source_ty.clone()],
            Type::Named("Boolean".to_string()),
        )?;
        self.output.push_str("    local.set $iter_func\n");

        self.output.push_str("    local.get $iter_list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $iter_len\n");

        self.output.push_str("    local.get $iter_len\n");
        self.output
            .push_str(&format!("    i32.const {}\n", item_size));
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.const 8\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $iter_out\n");

        self.output.push_str("    local.get $iter_out\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $iter_out\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    local.get $iter_len\n");
        self.output.push_str("    i32.store\n");

        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    local.set $iter_index\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    local.set $iter_out_index\n");

        self.output.push_str("    (loop $filter_loop\n");
        self.output.push_str("      local.get $iter_index\n");
        self.output.push_str("      local.get $iter_len\n");
        self.output.push_str("      i32.lt_u\n");
        self.output.push_str("      (if\n");
        self.output.push_str("        (then\n");
        self.load_current_iter_value(item_ty)?;
        self.output
            .push_str(&format!("          local.get ${}\n", value_local));
        self.emit_iter_func_call(&[item_ty], WasmType::I32, "          ");
        self.output.push_str("          local.set $iter_result\n");
        self.output.push_str("          local.get $iter_result\n");
        self.output.push_str("          (if\n");
        self.output.push_str("            (then\n");
        self.store_current_iter_output_address("iter_out_index", item_ty)?;
        self.output
            .push_str(&format!("              local.get ${}\n", value_local));
        self.output.push_str(&format!(
            "              {}\n",
            self.wasm_store_op_for_wasm_type(item_ty)
        ));
        self.increment_local("iter_out_index")?;
        self.output.push_str("            )\n");
        self.output.push_str("          )\n");
        self.increment_local("iter_index")?;
        self.output.push_str("          br $filter_loop\n");
        self.output.push_str("        )\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");

        self.output.push_str("    local.get $iter_out\n");
        self.output.push_str("    local.get $iter_out_index\n");
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $iter_out\n");

        Ok(())
    }

    fn generate_option_filter_call(
        &mut self,
        call: &CallExpr,
        item_source_ty: &Type,
        item_ty: WasmType,
    ) -> Result<(), CodeGenError> {
        self.output.push_str("    ;; filter(option, predicate)\n");
        self.generate_expr(&call.args[0])?;
        self.output.push_str("    local.set $match_tmp\n");
        self.generate_lambda_argument(
            &call.args[1],
            vec![item_ty],
            WasmType::I32,
            vec![item_source_ty.clone()],
            Type::Named("Boolean".to_string()),
        )?;
        self.output.push_str("    local.set $iter_func\n");

        self.output.push_str("    local.get $match_tmp\n");
        self.output.push_str("    i32.load ;; load Option tag\n");
        self.output.push_str("    i32.const 1 ;; Some tag\n");
        self.output.push_str("    i32.eq\n");
        self.output.push_str("    (if (result i32)\n");
        self.output.push_str("      (then\n");
        self.output.push_str("        local.get $match_tmp\n");
        self.output.push_str("        i32.const 4\n");
        self.output.push_str("        i32.add\n");
        self.output.push_str(&format!(
            "        {} ;; load Option payload\n",
            self.wasm_load_op_for_wasm_type(item_ty)
        ));
        self.emit_iter_func_call(&[item_ty], WasmType::I32, "        ");
        self.output.push_str("        (if (result i32)\n");
        self.output.push_str("          (then\n");
        self.output.push_str("            local.get $match_tmp\n");
        self.output.push_str("          )\n");
        self.output.push_str("          (else\n");
        self.generate_none_value_with_temp("option_value_tmp");
        self.output.push_str("          )\n");
        self.output.push_str("        )\n");
        self.output.push_str("      )\n");
        self.output.push_str("      (else\n");
        self.output.push_str("        local.get $match_tmp\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");

        Ok(())
    }

    fn generate_fold_call(&mut self, call: &CallExpr) -> Result<(), CodeGenError> {
        if call.args.len() != 3 {
            return Err(CodeGenError::UnsupportedFeature(
                "fold expects list, initial, and reducer arguments".to_string(),
            ));
        }
        self.ensure_list_iteration_argument(call, "fold")?;
        let item_source_ty = self.iteration_item_source_type(call, "fold")?;
        let item_ty = self.convert_type(&item_source_ty)?;
        let acc_source_ty = self.infer_expr_source_type(&call.args[1]).ok_or_else(|| {
            CodeGenError::UnsupportedFeature(
                "fold accumulator requires an inferable source type; add an annotation or concrete initial value"
                    .to_string(),
            )
        })?;
        let acc_ty = self.convert_type(&acc_source_ty)?;
        self.ensure_supported_closure_wasm_type(item_ty, "fold List item")?;
        self.ensure_supported_closure_wasm_type(acc_ty, "fold accumulator")?;

        self.output
            .push_str("    ;; fold(list, initial, reducer)\n");
        self.generate_expr(&call.args[0])?;
        self.output.push_str("    local.set $iter_list\n");
        self.generate_expr(&call.args[1])?;
        self.output
            .push_str(&format!("    local.set ${}\n", self.iter_acc_local(acc_ty)));
        self.generate_lambda_argument(
            &call.args[2],
            vec![acc_ty, item_ty],
            acc_ty,
            vec![acc_source_ty.clone(), item_source_ty],
            acc_source_ty,
        )?;
        self.output.push_str("    local.set $iter_func\n");

        self.output.push_str("    local.get $iter_list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("    local.set $iter_len\n");
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    local.set $iter_index\n");

        self.output.push_str("    (loop $fold_loop\n");
        self.output.push_str("      local.get $iter_index\n");
        self.output.push_str("      local.get $iter_len\n");
        self.output.push_str("      i32.lt_u\n");
        self.output.push_str("      (if\n");
        self.output.push_str("        (then\n");
        self.output.push_str(&format!(
            "          local.get ${}\n",
            self.iter_acc_local(acc_ty)
        ));
        self.emit_current_iter_value(item_ty)?;
        self.emit_iter_func_call(&[acc_ty, item_ty], acc_ty, "          ");
        self.output.push_str(&format!(
            "          local.set ${}\n",
            self.iter_acc_local(acc_ty)
        ));
        self.increment_local("iter_index")?;
        self.output.push_str("          br $fold_loop\n");
        self.output.push_str("        )\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        self.output
            .push_str(&format!("    local.get ${}\n", self.iter_acc_local(acc_ty)));

        Ok(())
    }

    fn iteration_input_kind(
        &self,
        call: &CallExpr,
        function_name: &str,
    ) -> Result<IterationInputKind, CodeGenError> {
        if let Some(source_ty) = self.infer_expr_source_type(&call.args[0]) {
            match source_ty {
                Type::Generic(ref name, _) if name == "List" => Ok(IterationInputKind::List),
                Type::Generic(ref name, _) if name == "Option" => Ok(IterationInputKind::Option),
                _ => Err(CodeGenError::UnsupportedFeature(format!(
                    "{} code generation currently supports List and Option inputs, found {}",
                    function_name, source_ty
                ))),
            }
        } else {
            Ok(IterationInputKind::Unknown)
        }
    }

    fn iteration_item_source_type(
        &self,
        call: &CallExpr,
        function_name: &str,
    ) -> Result<Type, CodeGenError> {
        self.container_item_source_type(&call.args[0], "List")
            .or_else(|| self.container_item_source_type(&call.args[0], "Option"))
            .ok_or_else(|| {
                CodeGenError::UnsupportedFeature(format!(
                    "{} code generation requires a known List or Option item type",
                    function_name
                ))
            })
    }

    fn ensure_supported_closure_wasm_type(
        &self,
        ty: WasmType,
        context: &str,
    ) -> Result<(), CodeGenError> {
        if matches!(ty, WasmType::I32 | WasmType::I64 | WasmType::F64) {
            return Ok(());
        }

        Err(CodeGenError::UnsupportedFeature(format!(
            "{} code generation currently supports i32, i64, and f64 closure values, found {}",
            context,
            self.wasm_type_str(ty)
        )))
    }

    fn generate_none_value_with_temp(&mut self, temp_local: &str) {
        self.output.push_str("        ;; None literal\n");
        self.output.push_str("        i32.const 8\n");
        self.output.push_str("        call $allocate\n");
        self.output
            .push_str(&format!("        local.tee ${}\n", temp_local));
        self.output.push_str("        i32.const 0\n");
        self.output.push_str("        i32.store\n");
        self.output
            .push_str(&format!("        local.get ${}\n", temp_local));
    }

    fn ensure_list_iteration_argument(
        &self,
        call: &CallExpr,
        function_name: &str,
    ) -> Result<(), CodeGenError> {
        if let Some(source_ty) = self.infer_expr_source_type(&call.args[0]) {
            if matches!(source_ty, Type::Generic(ref name, _) if name == "List") {
                return Ok(());
            }

            return Err(CodeGenError::UnsupportedFeature(format!(
                "{} code generation currently supports List inputs, found {}",
                function_name, source_ty
            )));
        }

        Ok(())
    }

    fn store_iter_out_header(&mut self, len_local: &str) -> Result<(), CodeGenError> {
        self.output.push_str("    local.get $iter_out\n");
        self.output
            .push_str(&format!("    local.get ${}\n", len_local));
        self.output.push_str("    i32.store\n");
        self.output.push_str("    local.get $iter_out\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output
            .push_str(&format!("    local.get ${}\n", len_local));
        self.output.push_str("    i32.store\n");
        Ok(())
    }

    fn load_current_iter_value(&mut self, ty: WasmType) -> Result<(), CodeGenError> {
        self.emit_current_iter_value(ty)?;
        self.output.push_str(&format!(
            "          local.set ${}\n",
            self.iter_value_local(ty)
        ));
        Ok(())
    }

    fn emit_current_iter_value(&mut self, ty: WasmType) -> Result<(), CodeGenError> {
        let element_size = self.wasm_type_size(ty);
        self.output.push_str("          local.get $iter_list\n");
        self.output.push_str("          i32.const 8\n");
        self.output.push_str("          i32.add\n");
        self.output.push_str("          local.get $iter_index\n");
        self.output
            .push_str(&format!("          i32.const {}\n", element_size));
        self.output.push_str("          i32.mul\n");
        self.output.push_str("          i32.add\n");
        self.output.push_str(&format!(
            "          {}\n",
            self.wasm_load_op_for_wasm_type(ty)
        ));
        Ok(())
    }

    fn store_current_iter_output_address(
        &mut self,
        index_local: &str,
        ty: WasmType,
    ) -> Result<(), CodeGenError> {
        let element_size = self.wasm_type_size(ty);
        self.output.push_str("          local.get $iter_out\n");
        self.output.push_str("          i32.const 8\n");
        self.output.push_str("          i32.add\n");
        self.output
            .push_str(&format!("          local.get ${}\n", index_local));
        self.output
            .push_str(&format!("          i32.const {}\n", element_size));
        self.output.push_str("          i32.mul\n");
        self.output.push_str("          i32.add\n");
        Ok(())
    }

    fn int_literal_source_type(value: i64) -> Type {
        if i32::try_from(value).is_ok() {
            Type::Named("Int32".to_string())
        } else {
            Type::Named("Int64".to_string())
        }
    }

    fn int_literal_wasm_type(value: i64) -> WasmType {
        if i32::try_from(value).is_ok() {
            WasmType::I32
        } else {
            WasmType::I64
        }
    }

    fn iter_value_local(&self, ty: WasmType) -> &'static str {
        match ty {
            WasmType::I64 => "iter_value_i64",
            WasmType::F64 => "iter_value_f64",
            _ => "iter_value",
        }
    }

    fn iter_acc_local(&self, ty: WasmType) -> &'static str {
        match ty {
            WasmType::I64 => "iter_acc_i64",
            WasmType::F64 => "iter_acc_f64",
            _ => "iter_acc",
        }
    }

    fn increment_local(&mut self, name: &str) -> Result<(), CodeGenError> {
        self.output
            .push_str(&format!("          local.get ${}\n", name));
        self.output.push_str("          i32.const 1\n");
        self.output.push_str("          i32.add\n");
        self.output
            .push_str(&format!("          local.set ${}\n", name));
        Ok(())
    }

    fn infer_expr_type(&self, expr: &Expr) -> Result<WasmType, CodeGenError> {
        match expr {
            Expr::IntLit(value) => Ok(Self::int_literal_wasm_type(*value)),
            Expr::FloatLit(_) => Ok(WasmType::F64),
            Expr::BoolLit(_) => Ok(WasmType::I32),
            Expr::Unit => Ok(WasmType::I32),
            Expr::Ident(name) => {
                if let Some(ty) = self.lookup_local_abi_type(name)? {
                    Ok(ty)
                } else if self.lookup_local(name).is_some() {
                    Err(CodeGenError::UnsupportedFeature(format!(
                        "missing Wasm ABI metadata for local '{}'",
                        name
                    )))
                } else {
                    Ok(WasmType::I32)
                }
            }
            Expr::FieldAccess(object, field) => self.infer_field_access_type(object, field),
            Expr::Binary(binary) => self.infer_binary_expr_type(binary),
            Expr::Unary(unary) => self.infer_unary_expr_type(unary),
            Expr::Cast(cast) => self.convert_type(&cast.target),
            Expr::Then(then) => self.infer_then_result_type(then),
            Expr::Match(match_expr) => match match_expr.arms.first() {
                Some(arm) => self.infer_block_result_type(&arm.body),
                None => Ok(WasmType::I32),
            },
            Expr::Block(block) => self.infer_block_result_type(block),
            Expr::With(with) => {
                if let Some(source_ty) = self.infer_expr_source_type(expr) {
                    return self.convert_type(&source_ty);
                }
                self.infer_block_result_type(&with.body)
            }
            Expr::Call(call) => {
                if let Some(source_ty) = self.infer_expr_source_type(expr) {
                    return self.convert_type(&source_ty);
                }

                if let Expr::Ident(name) = call.function.as_ref() {
                    if let Some(sig) = self.functions.get(name) {
                        return Ok(sig.result.unwrap_or(WasmType::I32));
                    }
                }
                Ok(WasmType::I32)
            }
            Expr::Pipe(pipe) => match &pipe.target {
                PipeTarget::Ident(name) => {
                    if let Some(source_ty) = self.infer_expr_source_type(expr) {
                        return self.convert_type(&source_ty);
                    }

                    if let Some(sig) = self.functions.get(name) {
                        Ok(sig.result.unwrap_or(WasmType::I32))
                    } else {
                        self.infer_expr_type(&pipe.expr)
                    }
                }
                PipeTarget::Expr(target) => {
                    if let Some(source_ty) = self.infer_expr_source_type(expr) {
                        return self.convert_type(&source_ty);
                    }

                    if let Expr::Ident(name) = target.as_ref() {
                        if let Some(sig) = self.functions.get(name) {
                            return Ok(sig.result.unwrap_or(WasmType::I32));
                        }
                    }
                    Ok(WasmType::I32)
                }
            },
            _ => Ok(WasmType::I32), // Pointers, records, strings, lists, and unit use i32 ABI.
        }
    }

    fn infer_expr_source_type_for_abi(&self, expr: &Expr) -> Result<Type, CodeGenError> {
        if let Some(source_ty) = self.infer_expr_source_type(expr) {
            return Ok(source_ty);
        }

        match self.infer_expr_type(expr)? {
            WasmType::F64 => Ok(Type::Named("Float64".to_string())),
            WasmType::I64 => Ok(Type::Named("Int64".to_string())),
            WasmType::F32 => Ok(Type::Named("Float32".to_string())),
            WasmType::I32 => Ok(Type::Named("Int32".to_string())),
        }
    }

    fn infer_expr_source_type(&self, expr: &Expr) -> Option<Type> {
        match expr {
            Expr::IntLit(value) => Some(Self::int_literal_source_type(*value)),
            Expr::FloatLit(_) => Some(Type::Named("Float64".to_string())),
            Expr::BoolLit(_) => Some(Type::Named("Boolean".to_string())),
            Expr::CharLit(_) => Some(Type::Named("Char".to_string())),
            Expr::StringLit(_) => Some(Type::Named("String".to_string())),
            Expr::Unit => Some(Type::Named("Unit".to_string())),
            Expr::Ident(name) => self.lookup_local_source_type(name).or_else(|| {
                if self.lookup_local(name).is_none() {
                    self.named_function_source_type(name)
                } else {
                    None
                }
            }),
            Expr::RecordLit(record) => self.infer_record_lit_source_type(record),
            Expr::Clone(clone) => self.infer_expr_source_type(&clone.base),
            Expr::Freeze(inner) => self.infer_expr_source_type(inner),
            Expr::Some(inner) => self
                .infer_expr_source_type(inner)
                .map(|ty| Type::Generic("Option".to_string(), vec![ty])),
            Expr::Binary(binary) => match binary.op {
                BinaryOp::Eq
                | BinaryOp::Ne
                | BinaryOp::Lt
                | BinaryOp::Le
                | BinaryOp::Gt
                | BinaryOp::Ge
                | BinaryOp::And
                | BinaryOp::Or => Some(Type::Named("Boolean".to_string())),
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                    if binary.op == BinaryOp::Add && self.is_string_concat(binary) {
                        return Some(Type::Named("String".to_string()));
                    }

                    match self.infer_binary_expr_type(binary) {
                        Ok(WasmType::F64) => Some(Type::Named("Float64".to_string())),
                        Ok(WasmType::I64) => Some(Type::Named("Int64".to_string())),
                        Ok(_) => Some(Type::Named("Int32".to_string())),
                        Err(_) => None,
                    }
                }
            },
            Expr::Unary(unary) => match unary.op {
                UnaryOp::Not => Some(Type::Named("Boolean".to_string())),
                UnaryOp::Neg => self.infer_expr_source_type(&unary.expr),
            },
            Expr::Cast(cast) => Some(cast.target.clone()),
            Expr::FieldAccess(object, field) => self.infer_field_access_source_type(object, field),
            Expr::ListLit(items) => self
                .infer_collection_element_source_type(items)
                .map(|ty| Type::Generic("List".to_string(), vec![ty])),
            Expr::RangeLit(_) => Some(Type::Generic(
                "Range".to_string(),
                vec![Type::Named("Int32".to_string())],
            )),
            Expr::ArrayLit(items) => self
                .infer_collection_element_source_type(items)
                .map(|ty| Type::Generic("Array".to_string(), vec![ty])),
            Expr::Then(then) => self.infer_then_source_type(then),
            Expr::Match(match_expr) => {
                self.infer_match_source_type_with_bindings(match_expr, &HashMap::new())
            }
            Expr::Block(block) => self.infer_block_source_type(block),
            Expr::With(with) => {
                let bindings = self.context_source_bindings(with, &HashMap::new());
                self.infer_block_source_type_with_bindings(&with.body, &bindings)
            }
            Expr::Lambda(lambda) => {
                let params = lambda
                    .params
                    .iter()
                    .map(|param| param.type_annotation.clone())
                    .collect::<Option<Vec<_>>>()?;
                let bindings = lambda
                    .params
                    .iter()
                    .zip(params.iter())
                    .map(|(param, ty)| (param.name.clone(), ty.clone()))
                    .collect::<HashMap<_, _>>();
                let return_ty =
                    self.infer_expr_source_type_with_bindings(&lambda.body, &bindings)?;
                Some(Type::Function(params, Box::new(return_ty)))
            }
            Expr::Call(call) => {
                if let Expr::Ident(name) = call.function.as_ref() {
                    let arg_exprs = call.args.iter().map(|arg| arg.as_ref()).collect::<Vec<_>>();
                    if self.can_infer_named_function_call_source_type(name, false) {
                        if let Some(return_ty) =
                            self.infer_function_call_source_type(name, &arg_exprs)
                        {
                            return Some(return_ty);
                        }
                    }

                    let arg_tys = call
                        .args
                        .iter()
                        .map(|arg| self.infer_expr_source_type(arg))
                        .collect::<Option<Vec<_>>>()?;
                    if let Some(return_ty) =
                        self.infer_named_callable_return_source_type(name, &arg_tys)
                    {
                        return Some(return_ty);
                    }

                    if let Ok(Some(target_name)) = self.resolve_method_call_target(name, &call.args)
                    {
                        return self
                            .infer_function_return_source_type_for_args(&target_name, &arg_tys);
                    }
                }

                None
            }
            Expr::Pipe(pipe) => match &pipe.target {
                PipeTarget::Ident(name) => {
                    if self.functions.contains_key(name) {
                        let args = [pipe.expr.as_ref()];
                        self.infer_function_call_source_type(name, &args)
                    } else if let Some(Type::Function(params, return_ty)) =
                        self.lookup_local_source_type(name)
                    {
                        if params.len() == 1 {
                            Some(*return_ty)
                        } else {
                            None
                        }
                    } else {
                        self.infer_expr_source_type(&pipe.expr)
                    }
                }
                PipeTarget::Expr(target) => {
                    if let Expr::Ident(name) = target.as_ref() {
                        if self.functions.contains_key(name) {
                            let args = [pipe.expr.as_ref()];
                            self.infer_function_call_source_type(name, &args)
                        } else if let Some(Type::Function(params, return_ty)) =
                            self.lookup_local_source_type(name)
                        {
                            if params.len() == 1 {
                                Some(*return_ty)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else if let Some(arg_ty) = self.infer_expr_source_type(&pipe.expr) {
                        self.infer_callable_return_source_type(target, &[arg_ty])
                    } else {
                        None
                    }
                }
            },
            _ => None,
        }
    }

    fn infer_function_call_source_type(&self, name: &str, args: &[&Expr]) -> Option<Type> {
        if let Some(function_name) = self.lookup_generic_function_alias(name) {
            return self.infer_function_call_source_type(&function_name, args);
        }

        match name {
            "map" if args.len() == 2 => {
                return self.infer_mapped_container_source_type(args[0], args[1])
            }
            "filter" if args.len() == 2 => return self.infer_expr_source_type(args[0]),
            "fold" if args.len() == 3 => return self.infer_expr_source_type(args[1]),
            _ => {}
        }

        let sig = self.function_source_sigs.get(name)?;
        let result = sig.result.as_ref()?;

        if sig.type_params.is_empty() {
            return Some(result.clone());
        }

        if sig.params.len() != args.len() {
            return None;
        }

        let mut substitution = HashMap::new();
        for (param_ty, arg_expr) in sig.params.iter().zip(args.iter()) {
            if let Some(arg_ty) = self.infer_expr_source_type(arg_expr) {
                Self::bind_source_type_params(
                    param_ty,
                    &arg_ty,
                    &sig.type_params,
                    &mut substitution,
                );
            }
        }
        for (param_ty, arg_expr) in sig.params.iter().zip(args.iter()) {
            if let Some(arg_ty) = self.infer_contextual_call_argument_source_type(
                param_ty,
                arg_expr,
                &sig.type_params,
                &substitution,
            ) {
                Self::bind_source_type_params(
                    param_ty,
                    &arg_ty,
                    &sig.type_params,
                    &mut substitution,
                );
            }
        }

        Self::substitute_source_type_params(result, &sig.type_params, &substitution)
    }

    fn is_iteration_function_name(name: &str) -> bool {
        matches!(name, "map" | "filter" | "fold")
    }

    fn can_infer_named_function_call_source_type(&self, name: &str, bound_in_expr: bool) -> bool {
        if bound_in_expr || self.lookup_local_source_type(name).is_some() {
            return false;
        }

        if self.lookup_local(name).is_some() && self.lookup_generic_function_alias(name).is_none() {
            return false;
        }

        Self::is_iteration_function_name(name)
            || self.lookup_generic_function_alias(name).is_some()
            || self.function_source_sigs.contains_key(name)
    }

    fn named_function_source_type(&self, name: &str) -> Option<Type> {
        let sig = self.function_source_sigs.get(name)?;
        if !sig.type_params.is_empty() {
            return None;
        }

        let result = sig
            .result
            .clone()
            .unwrap_or_else(|| Type::Named("Unit".to_string()));
        Some(Type::Function(sig.params.clone(), Box::new(result)))
    }

    fn infer_mapped_container_source_type(
        &self,
        container_expr: &Expr,
        mapper_expr: &Expr,
    ) -> Option<Type> {
        match self.infer_expr_source_type(container_expr)? {
            Type::Generic(name, params) if name == "List" || name == "Option" => {
                let item_ty = params.first()?.clone();
                let mapped_ty = self
                    .infer_callable_return_source_type(mapper_expr, std::slice::from_ref(&item_ty))
                    .unwrap_or(item_ty);
                Some(Type::Generic(name, vec![mapped_ty]))
            }
            _ => None,
        }
    }

    fn infer_map_result_source_type(&self, call: &CallExpr) -> Option<Type> {
        let item_ty = self
            .container_item_source_type(&call.args[0], "List")
            .or_else(|| self.container_item_source_type(&call.args[0], "Option"))?;
        self.infer_callable_return_source_type(&call.args[1], &[item_ty])
    }

    fn container_item_source_type(
        &self,
        container_expr: &Expr,
        container_name: &str,
    ) -> Option<Type> {
        match self.infer_expr_source_type(container_expr) {
            Some(source_ty) => Self::container_item_from_source_type(&source_ty, container_name),
            _ => None,
        }
    }

    fn container_item_from_source_type(source_ty: &Type, container_name: &str) -> Option<Type> {
        match source_ty {
            Type::Generic(name, params) if name == container_name => params.first().cloned(),
            _ => None,
        }
    }

    fn list_element_source_type(&self, source_ty: Option<&Type>) -> Option<Type> {
        match source_ty {
            Some(Type::Generic(name, params)) if name == "List" => params.first().cloned(),
            _ => None,
        }
    }

    fn list_get_function_for_element(&self, element_source_ty: Option<&Type>) -> &'static str {
        match element_source_ty {
            Some(Type::Named(name)) if name == "Float64" => "list_get_f64",
            Some(Type::Named(name)) if name == "Int64" => "list_get_i64",
            _ => "list_get",
        }
    }

    fn list_tail_function_for_element(&self, element_source_ty: Option<&Type>) -> &'static str {
        match element_source_ty {
            Some(Type::Named(name)) if name == "Float64" => "tail_f64",
            Some(Type::Named(name)) if name == "Int64" => "tail_i64",
            _ => "tail",
        }
    }

    fn infer_callable_return_source_type(&self, callable: &Expr, arg_tys: &[Type]) -> Option<Type> {
        self.infer_callable_return_source_type_with_bindings(callable, arg_tys, &HashMap::new())
    }

    fn infer_callable_return_source_type_with_bindings(
        &self,
        callable: &Expr,
        arg_tys: &[Type],
        bindings: &HashMap<String, Type>,
    ) -> Option<Type> {
        match callable {
            Expr::Lambda(lambda) => {
                if lambda.params.len() != arg_tys.len() {
                    return None;
                }

                let mut lambda_bindings = bindings.clone();
                lambda
                    .params
                    .iter()
                    .zip(arg_tys.iter())
                    .for_each(|(param, arg_ty)| {
                        lambda_bindings.insert(
                            param.name.clone(),
                            param
                                .type_annotation
                                .clone()
                                .unwrap_or_else(|| arg_ty.clone()),
                        );
                    });
                self.infer_expr_source_type_with_bindings(&lambda.body, &lambda_bindings)
            }
            Expr::Ident(name) => self.infer_named_callable_return_source_type(name, arg_tys),
            Expr::Then(then) => {
                self.infer_then_callable_return_source_type_with_bindings(then, arg_tys, bindings)
            }
            Expr::Match(match_expr) => self.infer_match_callable_return_source_type_with_bindings(
                match_expr, arg_tys, bindings,
            ),
            _ => match self.infer_expr_source_type_with_bindings(callable, bindings) {
                Some(Type::Function(params, return_ty)) if params.len() == arg_tys.len() => {
                    Some(*return_ty)
                }
                _ => None,
            },
        }
    }

    fn infer_then_callable_return_source_type_with_bindings(
        &self,
        then: &ThenExpr,
        arg_tys: &[Type],
        bindings: &HashMap<String, Type>,
    ) -> Option<Type> {
        let mut result =
            self.infer_block_callable_return_source_type(&then.then_block, arg_tys, bindings);

        for (_, block) in &then.else_ifs {
            result = Self::merge_source_types(
                result,
                self.infer_block_callable_return_source_type(block, arg_tys, bindings),
            );
        }

        if let Some(block) = &then.else_block {
            result = Self::merge_source_types(
                result,
                self.infer_block_callable_return_source_type(block, arg_tys, bindings),
            );
        }

        result
    }

    fn infer_match_callable_return_source_type_with_bindings(
        &self,
        match_expr: &MatchExpr,
        arg_tys: &[Type],
        bindings: &HashMap<String, Type>,
    ) -> Option<Type> {
        let scrutinee_ty = self.infer_expr_source_type_with_bindings(&match_expr.expr, bindings);
        let mut result = None;

        for arm in &match_expr.arms {
            let mut arm_bindings = bindings.clone();
            self.extend_pattern_source_bindings(
                &arm.pattern,
                scrutinee_ty.as_ref(),
                &mut arm_bindings,
            );
            result = Self::merge_source_types(
                result,
                self.infer_block_callable_return_source_type(&arm.body, arg_tys, &arm_bindings),
            );
        }

        result
    }

    fn infer_block_callable_return_source_type(
        &self,
        block: &BlockExpr,
        arg_tys: &[Type],
        bindings: &HashMap<String, Type>,
    ) -> Option<Type> {
        let mut block_bindings = bindings.clone();

        for stmt in &block.statements {
            if let Stmt::Binding(bind) = stmt {
                let value_ty = bind.type_annotation.clone().or_else(|| {
                    self.infer_expr_source_type_with_bindings(&bind.value, &block_bindings)
                });
                self.extend_pattern_source_bindings(
                    &bind.pattern,
                    value_ty.as_ref(),
                    &mut block_bindings,
                );
            }
        }

        let expr = block.expr.as_deref()?;
        self.infer_callable_return_source_type_with_bindings(expr, arg_tys, &block_bindings)
    }

    fn infer_named_callable_return_source_type(
        &self,
        name: &str,
        arg_tys: &[Type],
    ) -> Option<Type> {
        if let Some(Type::Function(params, return_ty)) = self.lookup_local_source_type(name) {
            if params.len() == arg_tys.len() {
                return Some(*return_ty);
            }
        }

        if let Some(function_name) = self.lookup_generic_function_alias(name) {
            return self.infer_function_return_source_type_for_args(&function_name, arg_tys);
        }

        if let Some(callable) = self.lookup_deferred_lambda_alias(name) {
            return self.infer_callable_return_source_type(&callable, arg_tys);
        }

        if let Some(return_ty) = self.infer_method_return_source_type_for_args(name, arg_tys) {
            return Some(return_ty);
        }

        self.infer_function_return_source_type_for_args(name, arg_tys)
    }

    fn infer_method_return_source_type_for_args(
        &self,
        method_name: &str,
        arg_tys: &[Type],
    ) -> Option<Type> {
        let receiver_ty = arg_tys.first()?;
        let record_name = self.source_record_name(receiver_ty)?;
        if !self
            .methods
            .get(record_name)
            .is_some_and(|methods| methods.contains_key(method_name))
        {
            return None;
        }

        let target_name = Self::method_function_name(record_name, method_name);
        self.infer_function_return_source_type_for_args(&target_name, arg_tys)
    }

    fn infer_function_return_source_type_for_args(
        &self,
        name: &str,
        arg_tys: &[Type],
    ) -> Option<Type> {
        let sig = self.function_source_sigs.get(name)?;
        let result = sig
            .result
            .clone()
            .unwrap_or_else(|| Type::Named("Unit".to_string()));
        if sig.params.len() != arg_tys.len() {
            return None;
        }

        let mut substitution = HashMap::new();
        for (param_ty, arg_ty) in sig.params.iter().zip(arg_tys.iter()) {
            Self::bind_source_type_params(param_ty, arg_ty, &sig.type_params, &mut substitution);
        }

        Self::substitute_source_type_params(&result, &sig.type_params, &substitution)
    }

    fn infer_expr_source_type_with_bindings(
        &self,
        expr: &Expr,
        bindings: &HashMap<String, Type>,
    ) -> Option<Type> {
        match expr {
            Expr::Ident(name) => bindings
                .get(name)
                .cloned()
                .or_else(|| self.lookup_local_source_type(name))
                .or_else(|| {
                    if self.lookup_local(name).is_none() {
                        self.named_function_source_type(name)
                    } else {
                        None
                    }
                }),
            Expr::IntLit(value) => Some(Self::int_literal_source_type(*value)),
            Expr::FloatLit(_) => Some(Type::Named("Float64".to_string())),
            Expr::BoolLit(_) => Some(Type::Named("Boolean".to_string())),
            Expr::CharLit(_) => Some(Type::Named("Char".to_string())),
            Expr::StringLit(_) => Some(Type::Named("String".to_string())),
            Expr::Unit => Some(Type::Named("Unit".to_string())),
            Expr::Some(inner) => self
                .infer_expr_source_type_with_bindings(inner, bindings)
                .map(|ty| Type::Generic("Option".to_string(), vec![ty])),
            Expr::Unary(unary) => match unary.op {
                UnaryOp::Not => Some(Type::Named("Boolean".to_string())),
                UnaryOp::Neg => self.infer_expr_source_type_with_bindings(&unary.expr, bindings),
            },
            Expr::Cast(cast) => Some(cast.target.clone()),
            Expr::Binary(binary) => self.infer_binary_source_type_with_bindings(binary, bindings),
            Expr::Call(call) => {
                if let Expr::Ident(name) = call.function.as_ref() {
                    let arg_exprs = call.args.iter().map(|arg| arg.as_ref()).collect::<Vec<_>>();
                    if self.can_infer_named_function_call_source_type(
                        name,
                        bindings.contains_key(name),
                    ) {
                        if let Some(return_ty) =
                            self.infer_function_call_source_type(name, &arg_exprs)
                        {
                            return Some(return_ty);
                        }
                    }

                    let arg_tys = call
                        .args
                        .iter()
                        .map(|arg| self.infer_expr_source_type_with_bindings(arg, bindings))
                        .collect::<Option<Vec<_>>>()?;
                    self.infer_named_callable_return_source_type(name, &arg_tys)
                } else {
                    None
                }
            }
            Expr::Pipe(pipe) => {
                let arg_ty = self.infer_expr_source_type_with_bindings(&pipe.expr, bindings)?;
                match &pipe.target {
                    PipeTarget::Ident(name) => {
                        self.infer_named_callable_return_source_type(name, &[arg_ty])
                    }
                    PipeTarget::Expr(target) => {
                        if let Expr::Ident(name) = target.as_ref() {
                            self.infer_named_callable_return_source_type(name, &[arg_ty])
                        } else {
                            None
                        }
                    }
                }
            }
            Expr::Then(then) => self.infer_then_source_type_with_bindings(then, bindings),
            Expr::Match(match_expr) => {
                self.infer_match_source_type_with_bindings(match_expr, bindings)
            }
            Expr::Block(block) => self.infer_block_source_type_with_bindings(block, bindings),
            Expr::With(with) => {
                let nested_bindings = self.context_source_bindings(with, bindings);
                self.infer_block_source_type_with_bindings(&with.body, &nested_bindings)
            }
            _ => self.infer_expr_source_type(expr),
        }
    }

    fn context_source_bindings(
        &self,
        with: &WithExpr,
        outer: &HashMap<String, Type>,
    ) -> HashMap<String, Type> {
        let mut bindings = outer.clone();
        for binding in &with.bindings {
            if let FieldInit::Field { name, value } = binding {
                if let Some(field_ty) = self
                    .record_field_type(&with.context_name, name)
                    .cloned()
                    .or_else(|| self.infer_expr_source_type_with_bindings(value, outer))
                {
                    bindings.insert(name.clone(), field_ty);
                }
            }
        }
        bindings
    }

    fn infer_match_source_type_with_bindings(
        &self,
        match_expr: &MatchExpr,
        bindings: &HashMap<String, Type>,
    ) -> Option<Type> {
        let scrutinee_ty = self.infer_expr_source_type_with_bindings(&match_expr.expr, bindings);
        let mut result = None;

        for arm in &match_expr.arms {
            let mut arm_bindings = bindings.clone();
            self.extend_pattern_source_bindings(
                &arm.pattern,
                scrutinee_ty.as_ref(),
                &mut arm_bindings,
            );
            if let Some(ty) = self.infer_block_source_type_with_bindings(&arm.body, &arm_bindings) {
                result = Self::merge_source_types(result, Some(ty));
            }
        }

        result
    }

    fn extend_pattern_source_bindings(
        &self,
        pattern: &Pattern,
        value_ty: Option<&Type>,
        bindings: &mut HashMap<String, Type>,
    ) {
        match pattern {
            Pattern::Ident(name) if name != "_" => {
                if let Some(ty) = value_ty {
                    bindings.insert(name.clone(), ty.clone());
                }
            }
            Pattern::Record(record_name, fields) => {
                self.extend_record_pattern_source_bindings(record_name, fields, bindings);
            }
            Pattern::RecordDestruct {
                type_name, fields, ..
            } => {
                self.extend_record_pattern_source_bindings(type_name, fields, bindings);
            }
            Pattern::Some(inner) => {
                self.extend_pattern_source_bindings(
                    inner,
                    self.variant_payload_type(value_ty, "Some"),
                    bindings,
                );
            }
            Pattern::Ok(inner) => {
                self.extend_pattern_source_bindings(
                    inner,
                    self.variant_payload_type(value_ty, "Ok"),
                    bindings,
                );
            }
            Pattern::Err(inner) => {
                self.extend_pattern_source_bindings(
                    inner,
                    self.variant_payload_type(value_ty, "Err"),
                    bindings,
                );
            }
            Pattern::ListCons(head, tail) => {
                let element_ty = self.list_element_source_type(value_ty);
                self.extend_pattern_source_bindings(head, element_ty.as_ref(), bindings);

                let tail_ty = element_ty
                    .clone()
                    .map(|ty| Type::Generic("List".to_string(), vec![ty]));
                self.extend_pattern_source_bindings(tail, tail_ty.as_ref(), bindings);
            }
            Pattern::ListExact(items) => {
                let element_ty = self.list_element_source_type(value_ty);
                for item in items {
                    self.extend_pattern_source_bindings(item, element_ty.as_ref(), bindings);
                }
            }
            Pattern::Wildcard | Pattern::Literal(_) | Pattern::None | Pattern::EmptyList => {}
            Pattern::Ident(_) => {}
        }
    }

    fn extend_record_pattern_source_bindings(
        &self,
        record_name: &str,
        fields: &[(String, Pattern)],
        bindings: &mut HashMap<String, Type>,
    ) {
        for (field_name, field_pattern) in fields {
            if let Some(field_ty) = self.record_field_type(record_name, field_name) {
                self.extend_pattern_source_bindings(field_pattern, Some(field_ty), bindings);
            }
        }
    }

    fn infer_binary_source_type_with_bindings(
        &self,
        binary: &BinaryExpr,
        bindings: &HashMap<String, Type>,
    ) -> Option<Type> {
        match binary.op {
            BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge
            | BinaryOp::And
            | BinaryOp::Or => Some(Type::Named("Boolean".to_string())),
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                let left_ty = self.infer_expr_source_type_with_bindings(&binary.left, bindings)?;
                let right_ty =
                    self.infer_expr_source_type_with_bindings(&binary.right, bindings)?;

                if matches!(left_ty, Type::Named(ref name) if name == "String")
                    || matches!(right_ty, Type::Named(ref name) if name == "String")
                {
                    return Some(Type::Named("String".to_string()));
                }

                if matches!(left_ty, Type::Named(ref name) if name == "Float64")
                    || matches!(right_ty, Type::Named(ref name) if name == "Float64")
                {
                    Some(Type::Named("Float64".to_string()))
                } else if matches!(left_ty, Type::Named(ref name) if name == "Int64")
                    || matches!(right_ty, Type::Named(ref name) if name == "Int64")
                {
                    Some(Type::Named("Int64".to_string()))
                } else {
                    Some(Type::Named("Int32".to_string()))
                }
            }
        }
    }

    fn infer_block_source_type_with_bindings(
        &self,
        block: &BlockExpr,
        bindings: &HashMap<String, Type>,
    ) -> Option<Type> {
        let mut block_bindings = bindings.clone();

        for stmt in &block.statements {
            if let Stmt::Binding(bind) = stmt {
                let value_ty = bind.type_annotation.clone().or_else(|| {
                    self.infer_expr_source_type_with_bindings(&bind.value, &block_bindings)
                });
                self.extend_pattern_source_bindings(
                    &bind.pattern,
                    value_ty.as_ref(),
                    &mut block_bindings,
                );
            }
        }

        if let Some(expr) = &block.expr {
            return self.infer_expr_source_type_with_bindings(expr, &block_bindings);
        }

        match block.statements.last() {
            Some(Stmt::Expr(expr)) => {
                self.infer_expr_source_type_with_bindings(expr, &block_bindings)
            }
            _ => None,
        }
    }

    fn bind_source_type_params(
        template: &Type,
        actual: &Type,
        type_params: &[String],
        substitution: &mut HashMap<String, Type>,
    ) {
        match (template, actual) {
            (Type::Named(name), actual) if type_params.iter().any(|param| param == name) => {
                substitution
                    .entry(name.clone())
                    .or_insert_with(|| actual.clone());
            }
            (
                Type::Generic(template_name, template_args),
                Type::Generic(actual_name, actual_args),
            ) if template_name == actual_name => {
                for (template_arg, actual_arg) in template_args.iter().zip(actual_args.iter()) {
                    Self::bind_source_type_params(
                        template_arg,
                        actual_arg,
                        type_params,
                        substitution,
                    );
                }
            }
            (
                Type::Function(template_params, template_ret),
                Type::Function(actual_params, actual_ret),
            ) => {
                for (template_param, actual_param) in
                    template_params.iter().zip(actual_params.iter())
                {
                    Self::bind_source_type_params(
                        template_param,
                        actual_param,
                        type_params,
                        substitution,
                    );
                }
                Self::bind_source_type_params(template_ret, actual_ret, type_params, substitution);
            }
            (Type::Temporal(template_name, _), actual)
                if type_params.iter().any(|param| param == template_name) =>
            {
                substitution
                    .entry(template_name.clone())
                    .or_insert_with(|| actual.clone());
            }
            _ => {}
        }
    }

    fn substitute_source_type_params(
        ty: &Type,
        type_params: &[String],
        substitution: &HashMap<String, Type>,
    ) -> Option<Type> {
        match ty {
            Type::Named(name) if type_params.iter().any(|param| param == name) => {
                substitution.get(name).cloned()
            }
            Type::Named(_) => Some(ty.clone()),
            Type::Generic(name, params) => {
                let substituted_params = params
                    .iter()
                    .map(|param| {
                        Self::substitute_source_type_params(param, type_params, substitution)
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(Type::Generic(name.clone(), substituted_params))
            }
            Type::Function(params, ret) => {
                let substituted_params = params
                    .iter()
                    .map(|param| {
                        Self::substitute_source_type_params(param, type_params, substitution)
                    })
                    .collect::<Option<Vec<_>>>()?;
                let substituted_ret =
                    Self::substitute_source_type_params(ret, type_params, substitution)?;
                Some(Type::Function(
                    substituted_params,
                    Box::new(substituted_ret),
                ))
            }
            Type::Temporal(name, temporals) if type_params.iter().any(|param| param == name) => {
                substitution.get(name).cloned()
            }
            Type::Temporal(name, temporals) => {
                Some(Type::Temporal(name.clone(), temporals.clone()))
            }
        }
    }

    fn infer_block_source_type(&self, block: &BlockExpr) -> Option<Type> {
        self.infer_block_source_type_with_bindings(block, &HashMap::new())
    }

    fn infer_then_source_type(&self, then: &ThenExpr) -> Option<Type> {
        let mut result = self.infer_block_source_type(&then.then_block);

        for (_, block) in &then.else_ifs {
            result = Self::merge_source_types(result, self.infer_block_source_type(block));
        }

        if let Some(block) = &then.else_block {
            result = Self::merge_source_types(result, self.infer_block_source_type(block));
        }

        result
    }

    fn infer_then_source_type_with_bindings(
        &self,
        then: &ThenExpr,
        bindings: &HashMap<String, Type>,
    ) -> Option<Type> {
        let mut result = self.infer_block_source_type_with_bindings(&then.then_block, bindings);

        for (_, block) in &then.else_ifs {
            result = Self::merge_source_types(
                result,
                self.infer_block_source_type_with_bindings(block, bindings),
            );
        }

        if let Some(block) = &then.else_block {
            result = Self::merge_source_types(
                result,
                self.infer_block_source_type_with_bindings(block, bindings),
            );
        }

        result
    }

    fn merge_source_types(left: Option<Type>, right: Option<Type>) -> Option<Type> {
        match (left, right) {
            (Some(left), Some(right)) if left == right => Some(left),
            (Some(left), Some(right)) => match (&left, &right) {
                (Type::Generic(left_name, left_args), Type::Generic(right_name, right_args))
                    if left_name == right_name && left_args == right_args =>
                {
                    Some(left)
                }
                _ => Some(left),
            },
            (Some(left), None) => Some(left),
            (None, Some(right)) => Some(right),
            (None, None) => None,
        }
    }

    fn infer_field_access_source_type(&self, object: &Expr, field: &str) -> Option<Type> {
        let object_ty = self.infer_expr_source_type(object)?;
        self.instantiated_record_field_type(&object_ty, field)
    }

    fn infer_block_result_type(&self, block: &BlockExpr) -> Result<WasmType, CodeGenError> {
        if let Some(expr) = &block.expr {
            return self.infer_expr_type(expr);
        }

        if let Some(Stmt::Expr(expr)) = block.statements.last() {
            return self.infer_expr_type(expr);
        }

        Ok(WasmType::I32)
    }

    fn infer_then_result_type(&self, then: &ThenExpr) -> Result<WasmType, CodeGenError> {
        self.infer_block_result_type(&then.then_block)
    }

    fn infer_field_access_type(
        &self,
        object: &Expr,
        field: &str,
    ) -> Result<WasmType, CodeGenError> {
        if let Some(source_ty) = self.infer_field_access_source_type(object, field) {
            return self.convert_type(&source_ty);
        }

        let record_name = match object {
            Expr::Ident(name) => self.var_types.get(name),
            Expr::RecordLit(record) => Some(&record.name),
            _ => None,
        };

        if let Some(record_name) = record_name {
            if let Some(field_ty) = self.record_field_type(record_name, field) {
                return self.convert_type(field_ty);
            }
        }

        Ok(WasmType::I32)
    }

    fn infer_binary_expr_type(&self, binary: &BinaryExpr) -> Result<WasmType, CodeGenError> {
        match binary.op {
            BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge
            | BinaryOp::And
            | BinaryOp::Or => Ok(WasmType::I32),
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                let left = self.infer_expr_type(&binary.left)?;
                let right = self.infer_expr_type(&binary.right)?;
                if matches!(left, WasmType::F64) || matches!(right, WasmType::F64) {
                    Ok(WasmType::F64)
                } else if matches!(left, WasmType::I64) || matches!(right, WasmType::I64) {
                    Ok(WasmType::I64)
                } else {
                    Ok(WasmType::I32)
                }
            }
        }
    }

    fn is_string_concat(&self, binary: &BinaryExpr) -> bool {
        binary.op == BinaryOp::Add && self.is_string_binary(binary)
    }

    fn is_string_binary(&self, binary: &BinaryExpr) -> bool {
        matches!(self.infer_expr_source_type(&binary.left), Some(Type::Named(name)) if name == "String")
            && matches!(self.infer_expr_source_type(&binary.right), Some(Type::Named(name)) if name == "String")
    }

    fn infer_unary_expr_type(&self, unary: &UnaryExpr) -> Result<WasmType, CodeGenError> {
        match unary.op {
            UnaryOp::Not => Ok(WasmType::I32),
            UnaryOp::Neg => self.infer_expr_type(&unary.expr),
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

    fn wasm_type_suffix(&self, ty: WasmType) -> &'static str {
        match ty {
            WasmType::I32 => "i32",
            WasmType::I64 => "i64",
            WasmType::F32 => "f32",
            WasmType::F64 => "f64",
        }
    }

    fn closure_call_type_name(&self, arg_types: &[WasmType], result: WasmType) -> String {
        if arg_types.iter().all(|ty| *ty == WasmType::I32) && result == WasmType::I32 {
            return format!("closure_call_{}", arg_types.len());
        }

        let args = arg_types
            .iter()
            .map(|ty| self.wasm_type_suffix(*ty))
            .collect::<Vec<_>>()
            .join("_");
        format!(
            "closure_call_{}_{}_to_{}",
            arg_types.len(),
            args,
            self.wasm_type_suffix(result)
        )
    }

    fn push_scope(&mut self) {
        self.locals.push(HashMap::new());
        self.local_types.push(HashMap::new());
        self.local_source_types.push(HashMap::new());
        self.local_aliases.push(HashMap::new());
        self.generic_function_aliases.push(HashMap::new());
        self.deferred_lambda_aliases.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.locals.pop();
        self.local_types.pop();
        self.local_source_types.pop();
        self.local_aliases.pop();
        self.generic_function_aliases.pop();
        self.deferred_lambda_aliases.pop();
    }

    fn add_local(&mut self, name: &str, idx: u32) {
        if let Some(scope) = self.locals.last_mut() {
            scope.insert(name.to_string(), idx);
        }
    }

    fn binding_id(binding: &BindDecl) -> usize {
        std::ptr::from_ref(binding) as usize
    }

    fn collect_binding_local_name(
        &mut self,
        binding: &BindDecl,
        source_name: &str,
        wasm_ty: WasmType,
    ) -> String {
        match self.collected_local_types.get(source_name).copied() {
            Some(existing_ty) if existing_ty != wasm_ty => {
                let alias = format!("__local_{}_{}", self.local_alias_counter, source_name);
                self.local_alias_counter += 1;
                self.binding_local_aliases
                    .insert(Self::binding_id(binding), alias.clone());
                alias
            }
            Some(_) => source_name.to_string(),
            None => {
                self.collected_local_types
                    .insert(source_name.to_string(), wasm_ty);
                source_name.to_string()
            }
        }
    }

    fn set_local_alias(&mut self, source_name: &str, local_name: String) {
        if let Some(scope) = self.local_aliases.last_mut() {
            scope.insert(source_name.to_string(), local_name);
        }
    }

    fn lookup_local_alias(&self, name: &str) -> Option<&str> {
        for scope in self.local_aliases.iter().rev() {
            if let Some(local_name) = scope.get(name) {
                return Some(local_name);
            }
        }
        None
    }

    fn lookup_local(&self, name: &str) -> Option<u32> {
        if let Some(local_name) = self.lookup_local_alias(name) {
            for scope in self.locals.iter().rev() {
                if let Some(idx) = scope.get(local_name) {
                    return Some(*idx);
                }
            }
        }

        for scope in self.locals.iter().rev() {
            if let Some(idx) = scope.get(name) {
                return Some(*idx);
            }
        }
        None
    }

    fn set_local_type(&mut self, name: &str, ty: WasmType) {
        if let Some(scope) = self.local_types.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    fn lookup_local_type(&self, name: &str) -> Option<WasmType> {
        if let Some(local_name) = self.lookup_local_alias(name) {
            for scope in self.local_types.iter().rev() {
                if let Some(ty) = scope.get(local_name) {
                    return Some(*ty);
                }
            }
        }

        for scope in self.local_types.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(*ty);
            }
        }
        self.global_types.get(name).copied()
    }

    fn lookup_local_abi_type(&self, name: &str) -> Result<Option<WasmType>, CodeGenError> {
        if let Some(source_ty) = self.lookup_local_source_type(name) {
            return self.convert_type(&source_ty).map(Some);
        }

        Ok(self.lookup_local_type(name))
    }

    fn set_local_source_type(&mut self, name: &str, ty: Type) {
        if let Some(scope) = self.local_source_types.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    fn lookup_local_source_type(&self, name: &str) -> Option<Type> {
        if let Some(local_name) = self.lookup_local_alias(name) {
            for scope in self.local_source_types.iter().rev() {
                if let Some(ty) = scope.get(local_name) {
                    return Some(ty.clone());
                }
            }
        }

        for scope in self.local_source_types.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty.clone());
            }
        }
        self.global_source_types.get(name).cloned()
    }

    fn set_generic_function_alias(&mut self, name: &str, function_name: String) {
        if let Some(scope) = self.generic_function_aliases.last_mut() {
            scope.insert(name.to_string(), function_name);
        }
    }

    fn clear_generic_function_alias(&mut self, name: &str) {
        if let Some(scope) = self.generic_function_aliases.last_mut() {
            scope.remove(name);
        }
    }

    fn lookup_generic_function_alias(&self, name: &str) -> Option<String> {
        for idx in (0..self.generic_function_aliases.len()).rev() {
            if let Some(function_name) = self.generic_function_aliases[idx].get(name) {
                return Some(function_name.clone());
            }

            if self
                .locals
                .get(idx)
                .is_some_and(|scope| scope.contains_key(name))
            {
                return None;
            }
        }

        None
    }

    fn set_deferred_lambda_alias(&mut self, name: &str, callable: Expr) {
        if let Some(scope) = self.deferred_lambda_aliases.last_mut() {
            scope.insert(name.to_string(), callable);
        }
    }

    fn clear_deferred_lambda_alias(&mut self, name: &str) {
        if let Some(scope) = self.deferred_lambda_aliases.last_mut() {
            scope.remove(name);
        }
    }

    fn lookup_deferred_lambda_alias(&self, name: &str) -> Option<Expr> {
        for idx in (0..self.deferred_lambda_aliases.len()).rev() {
            if let Some(callable) = self.deferred_lambda_aliases[idx].get(name) {
                return Some(callable.clone());
            }

            if self
                .locals
                .get(idx)
                .is_some_and(|scope| scope.contains_key(name))
            {
                return None;
            }
        }

        None
    }

    fn is_deferred_callable_expr(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Lambda(_) => true,
            Expr::Then(then) => {
                self.expr_is_replay_safe_for_deferred_callable(&then.condition)
                    && then.else_ifs.iter().all(|(condition, block)| {
                        self.expr_is_replay_safe_for_deferred_callable(condition)
                            && self.block_result_is_deferred_callable_with_bindings(
                                block,
                                &HashMap::new(),
                            )
                    })
                    && self.block_result_is_deferred_callable_with_bindings(
                        &then.then_block,
                        &HashMap::new(),
                    )
                    && then.else_block.as_ref().is_some_and(|block| {
                        self.block_result_is_deferred_callable_with_bindings(block, &HashMap::new())
                    })
            }
            Expr::Match(match_expr) => {
                let scrutinee_ty = self.infer_expr_source_type(&match_expr.expr);
                self.expr_is_replay_safe_for_deferred_callable(&match_expr.expr)
                    && !match_expr.arms.is_empty()
                    && match_expr.arms.iter().all(|arm| {
                        let mut arm_bindings = HashMap::new();
                        self.extend_pattern_source_bindings(
                            &arm.pattern,
                            scrutinee_ty.as_ref(),
                            &mut arm_bindings,
                        );
                        self.block_result_is_deferred_callable_with_bindings(
                            &arm.body,
                            &arm_bindings,
                        )
                    })
            }
            _ => false,
        }
    }

    #[allow(dead_code)]
    fn block_result_is_deferred_callable(&self, block: &BlockExpr) -> bool {
        self.block_result_is_deferred_callable_with_bindings(block, &HashMap::new())
    }

    fn block_result_is_deferred_callable_with_bindings(
        &self,
        block: &BlockExpr,
        bindings: &HashMap<String, Type>,
    ) -> bool {
        if self.block_terminal_lambda(block).is_none() {
            return false;
        }

        let mut block_bindings = bindings.clone();
        for stmt in self.deferred_callable_prefix_statements(block) {
            let Stmt::Binding(bind) = stmt else {
                return false;
            };

            if bind.mutable {
                return false;
            }

            let Pattern::Ident(name) = &bind.pattern else {
                return false;
            };

            if name == "_"
                || !self.expr_is_replay_safe_for_deferred_callable_with_bindings(
                    &bind.value,
                    &block_bindings,
                )
            {
                return false;
            }

            let Some(source_ty) = bind.type_annotation.clone().or_else(|| {
                self.infer_expr_source_type_with_bindings(&bind.value, &block_bindings)
            }) else {
                return false;
            };

            if !Self::is_copyable_source_type(&source_ty) {
                return false;
            }

            block_bindings.insert(name.clone(), source_ty);
        }

        true
    }

    fn block_terminal_lambda<'a>(&self, block: &'a BlockExpr) -> Option<&'a LambdaExpr> {
        if let Some(Expr::Lambda(lambda)) = block.expr.as_deref() {
            return Some(lambda);
        }

        match block.statements.last() {
            Some(Stmt::Expr(expr)) => match expr.as_ref() {
                Expr::Lambda(lambda) => Some(lambda),
                _ => None,
            },
            _ => None,
        }
    }

    fn deferred_callable_prefix_statements<'a>(&self, block: &'a BlockExpr) -> &'a [Stmt] {
        if block.expr.is_some() {
            &block.statements
        } else if matches!(block.statements.last(), Some(Stmt::Expr(expr)) if matches!(expr.as_ref(), Expr::Lambda(_)))
        {
            &block.statements[..block.statements.len() - 1]
        } else {
            &block.statements
        }
    }

    fn expr_is_replay_safe_for_deferred_callable(&self, expr: &Expr) -> bool {
        self.expr_is_replay_safe_for_deferred_callable_with_bindings(expr, &HashMap::new())
    }

    fn expr_is_replay_safe_for_deferred_callable_with_bindings(
        &self,
        expr: &Expr,
        bindings: &HashMap<String, Type>,
    ) -> bool {
        match expr {
            Expr::IntLit(_)
            | Expr::FloatLit(_)
            | Expr::StringLit(_)
            | Expr::CharLit(_)
            | Expr::BoolLit(_)
            | Expr::Unit
            | Expr::None => true,
            Expr::Ident(name) => bindings
                .get(name)
                .cloned()
                .or_else(|| self.lookup_local_source_type(name))
                .as_ref()
                .is_some_and(Self::is_copyable_source_type),
            Expr::Binary(binary) => {
                self.expr_is_replay_safe_for_deferred_callable_with_bindings(&binary.left, bindings)
                    && self.expr_is_replay_safe_for_deferred_callable_with_bindings(
                        &binary.right,
                        bindings,
                    )
            }
            Expr::Unary(unary) => {
                self.expr_is_replay_safe_for_deferred_callable_with_bindings(&unary.expr, bindings)
            }
            Expr::Cast(cast) => {
                self.expr_is_replay_safe_for_deferred_callable_with_bindings(&cast.expr, bindings)
            }
            Expr::Some(inner) | Expr::Ok(inner) | Expr::Err(inner) => {
                self.expr_is_replay_safe_for_deferred_callable_with_bindings(inner, bindings)
            }
            Expr::ListLit(elements) | Expr::ArrayLit(elements) => elements.iter().all(|element| {
                self.expr_is_replay_safe_for_deferred_callable_with_bindings(element, bindings)
            }),
            Expr::RangeLit(range) => {
                self.expr_is_replay_safe_for_deferred_callable_with_bindings(&range.start, bindings)
                    && self.expr_is_replay_safe_for_deferred_callable_with_bindings(
                        &range.end, bindings,
                    )
            }
            _ => false,
        }
    }

    fn is_copyable_source_type(ty: &Type) -> bool {
        match ty {
            Type::Named(name) => {
                matches!(
                    name.as_str(),
                    "Int32" | "Int64" | "Float64" | "Boolean" | "Char" | "Unit"
                )
            }
            Type::Generic(name, args) if name == "Option" => {
                args.first().is_some_and(Self::is_copyable_source_type)
            }
            Type::Generic(name, args) if name == "Result" && args.len() == 2 => {
                Self::is_copyable_source_type(&args[0]) && Self::is_copyable_source_type(&args[1])
            }
            Type::Generic(name, args) if name == "Array" => {
                args.first().is_some_and(Self::is_copyable_source_type)
            }
            _ => false,
        }
    }

    fn generic_function_alias_target(&self, expr: &Expr) -> Option<String> {
        let Expr::Ident(name) = expr else {
            return None;
        };

        if let Some(function_name) = self.lookup_generic_function_alias(name) {
            return Some(function_name);
        }

        if matches!(name.as_str(), "identity" | "map" | "filter" | "fold") {
            return Some(name.clone());
        }

        let sig = self.function_source_sigs.get(name)?;
        if sig.type_params.is_empty() || !self.function_decls.contains_key(name) {
            return None;
        }

        Some(name.clone())
    }

    #[allow(dead_code)]
    fn bind_local(&mut self, name: &str, idx: u32) {
        self.add_local(name, idx);
    }

    #[allow(dead_code)]
    fn next_local_index(&self) -> u32 {
        let mut max_idx = 0;
        for scope in &self.locals {
            for idx in scope.values() {
                max_idx = max_idx.max(*idx);
            }
        }
        max_idx + 1
    }

    fn collect_locals_from_pattern(
        &mut self,
        pattern: &Pattern,
        ty: &WasmType,
        source_ty: Option<&Type>,
        locals: &mut Vec<(String, WasmType)>,
    ) -> Result<(), CodeGenError> {
        match pattern {
            Pattern::Ident(name) => {
                locals.push((name.clone(), *ty));
                self.set_local_type(name, *ty);
                if let Some(source_ty) = source_ty {
                    self.set_local_source_type(name, source_ty.clone());
                }
            }
            Pattern::RecordDestruct {
                type_name,
                fields,
                rest,
            } => {
                self.collect_record_pattern_locals(
                    type_name,
                    source_ty,
                    fields,
                    rest.as_ref(),
                    locals,
                )?;
            }
            Pattern::Record(record_name, fields) => {
                self.collect_record_pattern_locals(record_name, source_ty, fields, None, locals)?;
            }
            Pattern::ListCons(head, tail) => {
                let element_source_ty = self.list_element_source_type(source_ty);
                let element_wasm_ty = element_source_ty
                    .as_ref()
                    .map(|ty| self.convert_type(ty))
                    .transpose()?
                    .unwrap_or(*ty);
                self.collect_locals_from_pattern(
                    head,
                    &element_wasm_ty,
                    element_source_ty.as_ref(),
                    locals,
                )?;
                self.collect_locals_from_pattern(tail, ty, source_ty, locals)?;
            }
            Pattern::ListExact(patterns) => {
                let element_source_ty = self.list_element_source_type(source_ty);
                let element_wasm_ty = element_source_ty
                    .as_ref()
                    .map(|ty| self.convert_type(ty))
                    .transpose()?
                    .unwrap_or(*ty);
                for p in patterns {
                    self.collect_locals_from_pattern(
                        p,
                        &element_wasm_ty,
                        element_source_ty.as_ref(),
                        locals,
                    )?;
                }
            }
            Pattern::Some(inner) => {
                let inner_source_ty = self.variant_payload_type(source_ty, "Some");
                let inner_wasm_ty = self.variant_payload_wasm_type(inner_source_ty)?;
                self.collect_locals_from_pattern(inner, &inner_wasm_ty, inner_source_ty, locals)?;
            }
            Pattern::Ok(inner) => {
                let inner_source_ty = self.variant_payload_type(source_ty, "Ok");
                let inner_wasm_ty = self.variant_payload_wasm_type(inner_source_ty)?;
                self.collect_locals_from_pattern(inner, &inner_wasm_ty, inner_source_ty, locals)?;
            }
            Pattern::Err(inner) => {
                let inner_source_ty = self.variant_payload_type(source_ty, "Err");
                let inner_wasm_ty = self.variant_payload_wasm_type(inner_source_ty)?;
                self.collect_locals_from_pattern(inner, &inner_wasm_ty, inner_source_ty, locals)?;
            }
            Pattern::Wildcard | Pattern::None | Pattern::EmptyList | Pattern::Literal(_) => {
                // These patterns don't bind variables
            }
        }
        Ok(())
    }

    fn collect_record_pattern_locals(
        &mut self,
        record_name: &str,
        source_ty: Option<&Type>,
        fields: &[(String, Pattern)],
        rest: Option<&String>,
        locals: &mut Vec<(String, WasmType)>,
    ) -> Result<(), CodeGenError> {
        for (field_name, field_pattern) in fields {
            let field_type = self
                .instantiated_record_field_type_by_name(record_name, source_ty, field_name)
                .ok_or_else(|| {
                    CodeGenError::NotImplemented(format!(
                        "field destructuring for {} in record {}",
                        field_name, record_name
                    ))
                })?;
            let wasm_type = self.convert_type(&field_type)?;
            self.collect_locals_from_pattern(field_pattern, &wasm_type, Some(&field_type), locals)?;
        }

        if let Some(rest_name) = rest {
            if rest_name == "_" {
                return Ok(());
            }
            let residual_name = self.ensure_residual_record_definition(record_name, fields)?;
            locals.push((rest_name.clone(), WasmType::I32));
            self.set_local_type(rest_name, WasmType::I32);
            self.set_local_source_type(rest_name, Type::Named(residual_name));
        }

        Ok(())
    }

    fn infer_unannotated_list_binding_source_type_from_later_array_use(
        &self,
        name: &str,
        value: &Expr,
        later_statements: &[Stmt],
        final_expr: Option<&Expr>,
    ) -> Option<Type> {
        let Expr::ListLit(items) = value else {
            return None;
        };

        let literal_elem_ty = self.infer_collection_element_source_type(items);
        let mut found_array_use = false;
        let mut contextual_elem_ty = None;

        for stmt in later_statements {
            Self::merge_array_use(
                &mut found_array_use,
                &mut contextual_elem_ty,
                self.find_array_use_for_ident_in_stmt(name, stmt),
            );
            if contextual_elem_ty.is_some() {
                break;
            }
        }

        if contextual_elem_ty.is_none() {
            if let Some(expr) = final_expr {
                Self::merge_array_use(
                    &mut found_array_use,
                    &mut contextual_elem_ty,
                    self.find_array_use_for_ident_in_expr(name, expr),
                );
            }
        }

        if !found_array_use {
            return None;
        }

        literal_elem_ty
            .or(contextual_elem_ty)
            .map(|elem| Type::Generic("Array".to_string(), vec![elem]))
    }

    fn merge_array_use(
        found: &mut bool,
        elem_ty: &mut Option<Type>,
        candidate: (bool, Option<Type>),
    ) {
        if candidate.0 {
            *found = true;
            if elem_ty.is_none() {
                *elem_ty = candidate.1;
            }
        }
    }

    fn find_array_use_for_ident_in_stmt(&self, name: &str, stmt: &Stmt) -> (bool, Option<Type>) {
        match stmt {
            Stmt::Binding(bind) => {
                let found = self.find_array_use_for_ident_in_expr(name, &bind.value);
                if Self::pattern_binds_name(&bind.pattern, name) {
                    return found;
                }
                found
            }
            Stmt::Assignment(assign) => self.find_array_use_for_ident_in_expr(name, &assign.value),
            Stmt::Expr(expr) => self.find_array_use_for_ident_in_expr(name, expr),
        }
    }

    fn find_array_use_for_ident_in_block(
        &self,
        name: &str,
        block: &BlockExpr,
    ) -> (bool, Option<Type>) {
        let mut found_array_use = false;
        let mut elem_ty = None;

        for stmt in &block.statements {
            Self::merge_array_use(
                &mut found_array_use,
                &mut elem_ty,
                self.find_array_use_for_ident_in_stmt(name, stmt),
            );
            if matches!(stmt, Stmt::Binding(bind) if Self::pattern_binds_name(&bind.pattern, name))
            {
                return (found_array_use, elem_ty);
            }
        }

        if let Some(expr) = &block.expr {
            Self::merge_array_use(
                &mut found_array_use,
                &mut elem_ty,
                self.find_array_use_for_ident_in_expr(name, expr),
            );
        }

        (found_array_use, elem_ty)
    }

    fn find_array_use_for_ident_in_expr(&self, name: &str, expr: &Expr) -> (bool, Option<Type>) {
        let mut found_array_use = false;
        let mut elem_ty = None;

        if let Expr::Call(call) = expr {
            Self::merge_array_use(
                &mut found_array_use,
                &mut elem_ty,
                self.direct_array_use_for_ident(name, call),
            );
        }

        match expr {
            Expr::Call(call) => {
                Self::merge_array_use(
                    &mut found_array_use,
                    &mut elem_ty,
                    self.find_array_use_for_ident_in_expr(name, &call.function),
                );
                for arg in &call.args {
                    Self::merge_array_use(
                        &mut found_array_use,
                        &mut elem_ty,
                        self.find_array_use_for_ident_in_expr(name, arg),
                    );
                }
            }
            Expr::Binary(binary) => {
                Self::merge_array_use(
                    &mut found_array_use,
                    &mut elem_ty,
                    self.find_array_use_for_ident_in_expr(name, &binary.left),
                );
                Self::merge_array_use(
                    &mut found_array_use,
                    &mut elem_ty,
                    self.find_array_use_for_ident_in_expr(name, &binary.right),
                );
            }
            Expr::Unary(unary) => {
                Self::merge_array_use(
                    &mut found_array_use,
                    &mut elem_ty,
                    self.find_array_use_for_ident_in_expr(name, &unary.expr),
                );
            }
            Expr::Cast(cast) => {
                Self::merge_array_use(
                    &mut found_array_use,
                    &mut elem_ty,
                    self.find_array_use_for_ident_in_expr(name, &cast.expr),
                );
            }
            Expr::Pipe(pipe) => {
                Self::merge_array_use(
                    &mut found_array_use,
                    &mut elem_ty,
                    self.find_array_use_for_ident_in_expr(name, &pipe.expr),
                );
                if let PipeTarget::Expr(target) = &pipe.target {
                    Self::merge_array_use(
                        &mut found_array_use,
                        &mut elem_ty,
                        self.find_array_use_for_ident_in_expr(name, target),
                    );
                }
            }
            Expr::Block(block) => {
                Self::merge_array_use(
                    &mut found_array_use,
                    &mut elem_ty,
                    self.find_array_use_for_ident_in_block(name, block),
                );
            }
            Expr::Then(then) => {
                Self::merge_array_use(
                    &mut found_array_use,
                    &mut elem_ty,
                    self.find_array_use_for_ident_in_expr(name, &then.condition),
                );
                Self::merge_array_use(
                    &mut found_array_use,
                    &mut elem_ty,
                    self.find_array_use_for_ident_in_block(name, &then.then_block),
                );
                for (condition, block) in &then.else_ifs {
                    Self::merge_array_use(
                        &mut found_array_use,
                        &mut elem_ty,
                        self.find_array_use_for_ident_in_expr(name, condition),
                    );
                    Self::merge_array_use(
                        &mut found_array_use,
                        &mut elem_ty,
                        self.find_array_use_for_ident_in_block(name, block),
                    );
                }
                if let Some(block) = &then.else_block {
                    Self::merge_array_use(
                        &mut found_array_use,
                        &mut elem_ty,
                        self.find_array_use_for_ident_in_block(name, block),
                    );
                }
            }
            Expr::While(while_expr) => {
                Self::merge_array_use(
                    &mut found_array_use,
                    &mut elem_ty,
                    self.find_array_use_for_ident_in_expr(name, &while_expr.condition),
                );
                Self::merge_array_use(
                    &mut found_array_use,
                    &mut elem_ty,
                    self.find_array_use_for_ident_in_block(name, &while_expr.body),
                );
            }
            Expr::Match(match_expr) => {
                Self::merge_array_use(
                    &mut found_array_use,
                    &mut elem_ty,
                    self.find_array_use_for_ident_in_expr(name, &match_expr.expr),
                );
                for arm in &match_expr.arms {
                    if !Self::pattern_binds_name(&arm.pattern, name) {
                        Self::merge_array_use(
                            &mut found_array_use,
                            &mut elem_ty,
                            self.find_array_use_for_ident_in_block(name, &arm.body),
                        );
                    }
                }
            }
            Expr::With(with_expr) => {
                let mut shadows_body = false;
                for binding in &with_expr.bindings {
                    match binding {
                        FieldInit::Field {
                            name: field_name,
                            value,
                        } => {
                            Self::merge_array_use(
                                &mut found_array_use,
                                &mut elem_ty,
                                self.find_array_use_for_ident_in_expr(name, value),
                            );
                            shadows_body |= field_name == name;
                        }
                        FieldInit::Spread(value) => {
                            Self::merge_array_use(
                                &mut found_array_use,
                                &mut elem_ty,
                                self.find_array_use_for_ident_in_expr(name, value),
                            );
                        }
                    }
                }
                if !shadows_body {
                    Self::merge_array_use(
                        &mut found_array_use,
                        &mut elem_ty,
                        self.find_array_use_for_ident_in_block(name, &with_expr.body),
                    );
                }
            }
            Expr::WithLifetime(with_lifetime) => {
                Self::merge_array_use(
                    &mut found_array_use,
                    &mut elem_ty,
                    self.find_array_use_for_ident_in_block(name, &with_lifetime.body),
                );
            }
            Expr::Lambda(lambda) => {
                if !lambda.params.iter().any(|param| param.name == name) {
                    Self::merge_array_use(
                        &mut found_array_use,
                        &mut elem_ty,
                        self.find_array_use_for_ident_in_expr(name, &lambda.body),
                    );
                }
            }
            Expr::RecordLit(record) => {
                for field in &record.fields {
                    let value = match field {
                        FieldInit::Field { value, .. } | FieldInit::Spread(value) => value,
                    };
                    Self::merge_array_use(
                        &mut found_array_use,
                        &mut elem_ty,
                        self.find_array_use_for_ident_in_expr(name, value),
                    );
                }
            }
            Expr::Clone(clone) => {
                Self::merge_array_use(
                    &mut found_array_use,
                    &mut elem_ty,
                    self.find_array_use_for_ident_in_expr(name, &clone.base),
                );
                for field in &clone.updates.fields {
                    let value = match field {
                        FieldInit::Field { value, .. } | FieldInit::Spread(value) => value,
                    };
                    Self::merge_array_use(
                        &mut found_array_use,
                        &mut elem_ty,
                        self.find_array_use_for_ident_in_expr(name, value),
                    );
                }
            }
            Expr::PrototypeClone(prototype) => {
                for field in &prototype.updates.fields {
                    let value = match field {
                        FieldInit::Field { value, .. } | FieldInit::Spread(value) => value,
                    };
                    Self::merge_array_use(
                        &mut found_array_use,
                        &mut elem_ty,
                        self.find_array_use_for_ident_in_expr(name, value),
                    );
                }
            }
            Expr::Freeze(inner)
            | Expr::Some(inner)
            | Expr::Ok(inner)
            | Expr::Err(inner)
            | Expr::Await(inner)
            | Expr::Spawn(inner)
            | Expr::FieldAccess(inner, _) => {
                Self::merge_array_use(
                    &mut found_array_use,
                    &mut elem_ty,
                    self.find_array_use_for_ident_in_expr(name, inner),
                );
            }
            Expr::ListLit(elements) | Expr::ArrayLit(elements) => {
                for element in elements {
                    Self::merge_array_use(
                        &mut found_array_use,
                        &mut elem_ty,
                        self.find_array_use_for_ident_in_expr(name, element),
                    );
                }
            }
            Expr::RangeLit(range) => {
                Self::merge_array_use(
                    &mut found_array_use,
                    &mut elem_ty,
                    self.find_array_use_for_ident_in_expr(name, &range.start),
                );
                Self::merge_array_use(
                    &mut found_array_use,
                    &mut elem_ty,
                    self.find_array_use_for_ident_in_expr(name, &range.end),
                );
            }
            Expr::IntLit(_)
            | Expr::FloatLit(_)
            | Expr::StringLit(_)
            | Expr::CharLit(_)
            | Expr::BoolLit(_)
            | Expr::Unit
            | Expr::Ident(_)
            | Expr::None => {}
        }

        (found_array_use, elem_ty)
    }

    fn direct_array_use_for_ident(&self, name: &str, call: &CallExpr) -> (bool, Option<Type>) {
        let Expr::Ident(func_name) = call.function.as_ref() else {
            return (false, None);
        };
        if !matches!(func_name.as_str(), "array_get" | "array_set") {
            return (false, None);
        }
        let Some(Expr::Ident(arg_name)) = call.args.first().map(|arg| arg.as_ref()) else {
            return (false, None);
        };
        if arg_name != name {
            return (false, None);
        }

        let elem_ty = if func_name == "array_set" {
            call.args
                .get(2)
                .and_then(|arg| self.infer_expr_source_type(arg))
        } else {
            None
        };
        (true, elem_ty)
    }

    fn pattern_binds_name(pattern: &Pattern, name: &str) -> bool {
        match pattern {
            Pattern::Ident(binding) => binding == name,
            Pattern::Record(_, fields) => fields
                .iter()
                .any(|(_, field_pattern)| Self::pattern_binds_name(field_pattern, name)),
            Pattern::RecordDestruct { fields, rest, .. } => {
                rest.as_ref().is_some_and(|binding| binding == name)
                    || fields
                        .iter()
                        .any(|(_, field_pattern)| Self::pattern_binds_name(field_pattern, name))
            }
            Pattern::Some(inner) | Pattern::Ok(inner) | Pattern::Err(inner) => {
                Self::pattern_binds_name(inner, name)
            }
            Pattern::ListCons(head, tail) => {
                Self::pattern_binds_name(head, name) || Self::pattern_binds_name(tail, name)
            }
            Pattern::ListExact(patterns) => patterns
                .iter()
                .any(|pattern| Self::pattern_binds_name(pattern, name)),
            Pattern::Wildcard | Pattern::Literal(_) | Pattern::None | Pattern::EmptyList => false,
        }
    }

    fn collect_locals_from_block(
        &mut self,
        block: &BlockExpr,
        locals: &mut Vec<(String, WasmType)>,
    ) -> Result<(), CodeGenError> {
        self.collect_locals_from_block_with_expected(block, locals, None)
    }

    fn collect_locals_from_block_with_expected(
        &mut self,
        block: &BlockExpr,
        locals: &mut Vec<(String, WasmType)>,
        expected_source: Option<&Type>,
    ) -> Result<(), CodeGenError> {
        for (stmt_index, stmt) in block.statements.iter().enumerate() {
            match stmt {
                Stmt::Binding(bind) => {
                    let source_ty = bind
                        .type_annotation
                        .clone()
                        .or_else(|| {
                            if let Pattern::Ident(name) = &bind.pattern {
                                self.infer_unannotated_binding_source_type_from_later_context(
                                    name,
                                    &bind.value,
                                    &block.statements[stmt_index + 1..],
                                    block.expr.as_deref(),
                                    expected_source,
                                )
                            } else {
                                None
                            }
                        })
                        .or_else(|| self.infer_expr_source_type(&bind.value));
                    let ty = if let Some(source_ty) = &source_ty {
                        self.convert_type(source_ty)?
                    } else {
                        self.infer_expr_type(&bind.value)?
                    };
                    // Extract variables from the pattern
                    if let Pattern::Ident(name) = &bind.pattern {
                        let local_name = self.collect_binding_local_name(bind, name, ty);
                        locals.push((local_name.clone(), ty));
                        self.set_local_type(&local_name, ty);
                        self.set_local_type(name, ty);
                        if let Some(source_ty) = &source_ty {
                            self.set_local_source_type(&local_name, source_ty.clone());
                            self.set_local_source_type(name, source_ty.clone());
                            self.register_record_var_type(name, source_ty);
                        }
                        if let Some(annotation) = &bind.type_annotation {
                            self.register_record_var_type(name, annotation);
                        }
                    } else {
                        self.collect_locals_from_pattern(
                            &bind.pattern,
                            &ty,
                            source_ty.as_ref(),
                            locals,
                        )?;
                    }
                    // Also collect locals from the value expression
                    self.collect_locals_from_expr(&bind.value, locals)?;
                }
                Stmt::Assignment(assign) => {
                    // Assignments don't create new locals, but they can finalize the source type
                    // of an ambiguous mutable binding such as `mut val items = []`.
                    if self.lookup_local_source_type(&assign.name).is_none() {
                        if let Some(source_ty) = self.infer_expr_source_type(&assign.value) {
                            let storage_name = self
                                .lookup_local_alias(&assign.name)
                                .map(str::to_string)
                                .unwrap_or_else(|| assign.name.clone());
                            self.set_local_source_type(&storage_name, source_ty.clone());
                            self.set_local_source_type(&assign.name, source_ty);
                        }
                    }
                    self.collect_locals_from_expr(&assign.value, locals)?;
                }
                Stmt::Expr(expr) => {
                    // Check for nested blocks and match expressions
                    self.collect_locals_from_expr(expr, locals)?;
                }
            }
        }

        // Check the return expression for nested blocks
        if let Some(expr) = &block.expr {
            self.collect_locals_from_expr_with_expected(expr, locals, expected_source)?;
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
                            // Void pipe calls synthesize a Unit value outside main.
                            sig.result.is_some()
                                || self.current_function != Some("main".to_string())
                        } else {
                            // Binding leaves value
                            true
                        }
                    }
                    PipeTarget::Expr(target_expr) => {
                        if let Expr::Ident(func_name) = &**target_expr {
                            if let Some(sig) = self.functions.get(func_name) {
                                sig.result.is_some()
                                    || self.current_function != Some("main".to_string())
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

    fn is_unit_source_type(ty: &Type) -> bool {
        matches!(ty, Type::Named(name) if name == "Unit")
    }

    fn expr_synthesizes_unit_value(&self, expr: &Expr) -> bool {
        if self.current_function == Some("main".to_string()) {
            return false;
        }

        match expr {
            Expr::Call(call) => {
                if let Expr::Ident(func_name) = call.function.as_ref() {
                    if self.functions.contains_key(func_name) {
                        let target_name = self.resolve_builtin_abi_function(func_name, &call.args);
                        return self
                            .function_source_sigs
                            .get(&target_name)
                            .is_some_and(|sig| {
                                sig.params
                                    .iter()
                                    .any(|param| matches!(param, Type::Function(_, _)))
                            });
                    }
                }
                false
            }
            Expr::Pipe(pipe) => match &pipe.target {
                PipeTarget::Ident(name) => self
                    .functions
                    .get(name)
                    .is_some_and(|sig| sig.result.is_none()),
                PipeTarget::Expr(target) => {
                    if let Expr::Ident(name) = target.as_ref() {
                        self.functions
                            .get(name)
                            .is_some_and(|sig| sig.result.is_none())
                    } else {
                        false
                    }
                }
            },
            _ => false,
        }
    }

    fn max_record_tmp_depth_in_block(block: &BlockExpr) -> usize {
        let stmt_depth = block
            .statements
            .iter()
            .map(Self::max_record_tmp_depth_in_stmt)
            .max()
            .unwrap_or(0);
        let expr_depth = block
            .expr
            .as_deref()
            .map(Self::max_record_tmp_depth_in_expr)
            .unwrap_or(0);
        stmt_depth.max(expr_depth)
    }

    fn max_record_tmp_depth_in_stmt(stmt: &Stmt) -> usize {
        match stmt {
            Stmt::Binding(bind) => Self::max_record_tmp_depth_in_expr(&bind.value)
                .max(Self::max_record_tmp_depth_in_pattern(&bind.pattern)),
            Stmt::Assignment(assign) => Self::max_record_tmp_depth_in_expr(&assign.value),
            Stmt::Expr(expr) => Self::max_record_tmp_depth_in_expr(expr),
        }
    }

    fn max_record_tmp_depth_in_expr(expr: &Expr) -> usize {
        match expr {
            Expr::RecordLit(record) => {
                1 + record
                    .fields
                    .iter()
                    .map(Self::max_record_tmp_depth_in_field_init)
                    .max()
                    .unwrap_or(0)
            }
            Expr::Clone(clone) => Self::max_record_tmp_depth_in_expr(&clone.base)
                .max(Self::max_record_tmp_depth_in_record_lit(&clone.updates)),
            Expr::Freeze(inner)
            | Expr::Await(inner)
            | Expr::Spawn(inner)
            | Expr::Some(inner)
            | Expr::Ok(inner)
            | Expr::Err(inner) => Self::max_record_tmp_depth_in_expr(inner),
            Expr::Then(then) => {
                let else_if_depth = then
                    .else_ifs
                    .iter()
                    .map(|(condition, block)| {
                        Self::max_record_tmp_depth_in_expr(condition)
                            .max(Self::max_record_tmp_depth_in_block(block))
                    })
                    .max()
                    .unwrap_or(0);
                Self::max_record_tmp_depth_in_expr(&then.condition)
                    .max(Self::max_record_tmp_depth_in_block(&then.then_block))
                    .max(else_if_depth)
                    .max(
                        then.else_block
                            .as_ref()
                            .map(Self::max_record_tmp_depth_in_block)
                            .unwrap_or(0),
                    )
            }
            Expr::While(while_expr) => Self::max_record_tmp_depth_in_expr(&while_expr.condition)
                .max(Self::max_record_tmp_depth_in_block(&while_expr.body)),
            Expr::Match(match_expr) => {
                let arm_depth = match_expr
                    .arms
                    .iter()
                    .map(|arm| {
                        Self::max_record_tmp_depth_in_pattern(&arm.pattern)
                            .max(Self::max_record_tmp_depth_in_block(&arm.body))
                    })
                    .max()
                    .unwrap_or(0);
                Self::max_record_tmp_depth_in_expr(&match_expr.expr).max(arm_depth)
            }
            Expr::Call(call) => {
                let arg_depth = call
                    .args
                    .iter()
                    .map(|arg| Self::max_record_tmp_depth_in_expr(arg))
                    .max()
                    .unwrap_or(0);
                Self::max_record_tmp_depth_in_expr(&call.function).max(arg_depth)
            }
            Expr::Binary(binary) => Self::max_record_tmp_depth_in_expr(&binary.left)
                .max(Self::max_record_tmp_depth_in_expr(&binary.right)),
            Expr::Unary(unary) => Self::max_record_tmp_depth_in_expr(&unary.expr),
            Expr::Cast(cast) => Self::max_record_tmp_depth_in_expr(&cast.expr),
            Expr::Pipe(pipe) => {
                let target_depth = match &pipe.target {
                    PipeTarget::Ident(_) => 0,
                    PipeTarget::Expr(target) => Self::max_record_tmp_depth_in_expr(target),
                };
                Self::max_record_tmp_depth_in_expr(&pipe.expr).max(target_depth)
            }
            Expr::With(with) => {
                let binding_depth = with
                    .bindings
                    .iter()
                    .map(Self::max_record_tmp_depth_in_field_init)
                    .max()
                    .unwrap_or(0);
                binding_depth.max(Self::max_record_tmp_depth_in_block(&with.body))
            }
            Expr::WithLifetime(with_lifetime) => {
                Self::max_record_tmp_depth_in_block(&with_lifetime.body)
            }
            Expr::Block(block) => Self::max_record_tmp_depth_in_block(block),
            Expr::FieldAccess(object, _) => Self::max_record_tmp_depth_in_expr(object),
            Expr::ListLit(items) | Expr::ArrayLit(items) => items
                .iter()
                .map(|item| Self::max_record_tmp_depth_in_expr(item))
                .max()
                .unwrap_or(0),
            Expr::RangeLit(range) => Self::max_record_tmp_depth_in_expr(&range.start)
                .max(Self::max_record_tmp_depth_in_expr(&range.end)),
            Expr::Lambda(lambda) => Self::max_record_tmp_depth_in_expr(&lambda.body),
            Expr::PrototypeClone(proto_clone) => {
                Self::max_record_tmp_depth_in_record_lit(&proto_clone.updates)
            }
            Expr::IntLit(_)
            | Expr::FloatLit(_)
            | Expr::StringLit(_)
            | Expr::CharLit(_)
            | Expr::BoolLit(_)
            | Expr::Unit
            | Expr::Ident(_)
            | Expr::None => 0,
        }
    }

    fn max_record_tmp_depth_in_record_lit(record: &RecordLit) -> usize {
        1 + record
            .fields
            .iter()
            .map(Self::max_record_tmp_depth_in_field_init)
            .max()
            .unwrap_or(0)
    }

    fn max_record_tmp_depth_in_field_init(field: &FieldInit) -> usize {
        match field {
            FieldInit::Field { value, .. } | FieldInit::Spread(value) => {
                Self::max_record_tmp_depth_in_expr(value)
            }
        }
    }

    fn max_record_tmp_depth_in_pattern(pattern: &Pattern) -> usize {
        match pattern {
            Pattern::Record(_, fields) => 1 + Self::max_record_tmp_depth_in_pattern_fields(fields),
            Pattern::RecordDestruct { fields, .. } => {
                1 + Self::max_record_tmp_depth_in_pattern_fields(fields)
            }
            Pattern::Some(inner) | Pattern::Ok(inner) | Pattern::Err(inner) => {
                Self::max_record_tmp_depth_in_pattern(inner)
            }
            Pattern::ListCons(head, tail) => Self::max_record_tmp_depth_in_pattern(head)
                .max(Self::max_record_tmp_depth_in_pattern(tail)),
            Pattern::ListExact(patterns) => patterns
                .iter()
                .map(|pattern| Self::max_record_tmp_depth_in_pattern(pattern))
                .max()
                .unwrap_or(0),
            Pattern::Wildcard
            | Pattern::Ident(_)
            | Pattern::Literal(_)
            | Pattern::None
            | Pattern::EmptyList => 0,
        }
    }

    fn max_record_tmp_depth_in_pattern_fields(fields: &[(String, Pattern)]) -> usize {
        fields
            .iter()
            .map(|(_, pattern)| Self::max_record_tmp_depth_in_pattern(pattern))
            .max()
            .unwrap_or(0)
    }

    fn match_pattern_local_name(
        match_expr: &MatchExpr,
        arm_index: usize,
        binding_index: usize,
        source_name: &str,
    ) -> String {
        let match_id = std::ptr::from_ref(match_expr) as usize;
        format!(
            "__match_{:x}_{}_{}_{}",
            match_id, arm_index, binding_index, source_name
        )
    }

    fn match_pattern_binding_conflicts(
        &self,
        match_expr: &MatchExpr,
        scrutinee_source_ty: Option<&Type>,
        scrutinee_wasm_ty: WasmType,
    ) -> Result<HashSet<String>, CodeGenError> {
        let mut seen = HashMap::new();
        let mut conflicts = HashSet::new();

        for arm in &match_expr.arms {
            let mut bindings = Vec::new();
            self.collect_pattern_binding_types(
                &arm.pattern,
                scrutinee_source_ty,
                scrutinee_wasm_ty,
                &mut bindings,
            )?;

            for (name, ty, _) in bindings {
                if let Some(existing_ty) = seen.insert(name.clone(), ty) {
                    if existing_ty != ty {
                        conflicts.insert(name);
                    }
                }
            }
        }

        Ok(conflicts)
    }

    fn collect_pattern_binding_types(
        &self,
        pattern: &Pattern,
        source_ty: Option<&Type>,
        fallback_wasm_ty: WasmType,
        bindings: &mut Vec<(String, WasmType, Option<Type>)>,
    ) -> Result<(), CodeGenError> {
        match pattern {
            Pattern::Ident(name) => {
                let wasm_ty = source_ty
                    .map(|ty| self.convert_type(ty))
                    .transpose()?
                    .unwrap_or(fallback_wasm_ty);
                bindings.push((name.clone(), wasm_ty, source_ty.cloned()));
            }
            Pattern::Record(record_name, fields) => {
                self.collect_record_pattern_binding_types(
                    record_name,
                    source_ty,
                    fields,
                    None,
                    bindings,
                )?;
            }
            Pattern::RecordDestruct {
                type_name,
                fields,
                rest,
            } => {
                self.collect_record_pattern_binding_types(
                    type_name,
                    source_ty,
                    fields,
                    rest.as_ref(),
                    bindings,
                )?;
            }
            Pattern::ListCons(head, tail) => {
                let element_source_ty = self.list_element_source_type(source_ty);
                let element_wasm_ty = element_source_ty
                    .as_ref()
                    .map(|ty| self.convert_type(ty))
                    .transpose()?
                    .unwrap_or(fallback_wasm_ty);
                self.collect_pattern_binding_types(
                    head,
                    element_source_ty.as_ref(),
                    element_wasm_ty,
                    bindings,
                )?;
                self.collect_pattern_binding_types(tail, source_ty, fallback_wasm_ty, bindings)?;
            }
            Pattern::ListExact(patterns) => {
                let element_source_ty = self.list_element_source_type(source_ty);
                let element_wasm_ty = element_source_ty
                    .as_ref()
                    .map(|ty| self.convert_type(ty))
                    .transpose()?
                    .unwrap_or(fallback_wasm_ty);
                for pattern in patterns {
                    self.collect_pattern_binding_types(
                        pattern,
                        element_source_ty.as_ref(),
                        element_wasm_ty,
                        bindings,
                    )?;
                }
            }
            Pattern::Some(inner) => {
                let payload_ty = self.variant_payload_type(source_ty, "Some");
                let payload_wasm_ty = self.variant_payload_wasm_type(payload_ty)?;
                self.collect_pattern_binding_types(inner, payload_ty, payload_wasm_ty, bindings)?;
            }
            Pattern::Ok(inner) => {
                let payload_ty = self.variant_payload_type(source_ty, "Ok");
                let payload_wasm_ty = self.variant_payload_wasm_type(payload_ty)?;
                self.collect_pattern_binding_types(inner, payload_ty, payload_wasm_ty, bindings)?;
            }
            Pattern::Err(inner) => {
                let payload_ty = self.variant_payload_type(source_ty, "Err");
                let payload_wasm_ty = self.variant_payload_wasm_type(payload_ty)?;
                self.collect_pattern_binding_types(inner, payload_ty, payload_wasm_ty, bindings)?;
            }
            Pattern::Wildcard | Pattern::None | Pattern::EmptyList | Pattern::Literal(_) => {}
        }

        Ok(())
    }

    fn collect_record_pattern_binding_types(
        &self,
        record_name: &str,
        source_ty: Option<&Type>,
        fields: &[(String, Pattern)],
        rest: Option<&String>,
        bindings: &mut Vec<(String, WasmType, Option<Type>)>,
    ) -> Result<(), CodeGenError> {
        if let Some(rest_name) = rest {
            if rest_name != "_" {
                bindings.push((rest_name.clone(), WasmType::I32, None));
            }
        }

        for (field_name, field_pattern) in fields {
            let field_type = self
                .instantiated_record_field_type_by_name(record_name, source_ty, field_name)
                .ok_or_else(|| {
                    CodeGenError::NotImplemented(format!(
                        "field destructuring for {} in record {}",
                        field_name, record_name
                    ))
                })?;
            let wasm_type = self.convert_type(&field_type)?;
            self.collect_pattern_binding_types(
                field_pattern,
                Some(&field_type),
                wasm_type,
                bindings,
            )?;
        }

        Ok(())
    }

    fn collect_locals_from_expr(
        &mut self,
        expr: &Expr,
        locals: &mut Vec<(String, WasmType)>,
    ) -> Result<(), CodeGenError> {
        self.collect_locals_from_expr_with_expected(expr, locals, None)
    }

    fn collect_locals_from_expr_with_expected(
        &mut self,
        expr: &Expr,
        locals: &mut Vec<(String, WasmType)>,
        expected_source: Option<&Type>,
    ) -> Result<(), CodeGenError> {
        match expr {
            Expr::Block(block) => {
                self.collect_locals_from_block_with_expected(block, locals, expected_source)?;
            }
            Expr::Match(match_expr) => {
                let scrutinee_source_ty = self.infer_expr_source_type(&match_expr.expr);
                let scrutinee_wasm_ty = if let Some(source_ty) = &scrutinee_source_ty {
                    self.convert_type(source_ty)?
                } else {
                    self.infer_expr_type(&match_expr.expr)?
                };
                let conflicting_pattern_bindings = self.match_pattern_binding_conflicts(
                    match_expr,
                    scrutinee_source_ty.as_ref(),
                    scrutinee_wasm_ty,
                )?;
                // Collect locals from match arms
                for (arm_index, arm) in match_expr.arms.iter().enumerate() {
                    self.push_scope();
                    let mut pattern_locals = Vec::new();
                    self.collect_locals_from_pattern(
                        &arm.pattern,
                        &scrutinee_wasm_ty,
                        scrutinee_source_ty.as_ref(),
                        &mut pattern_locals,
                    )?;

                    for (binding_index, (name, ty)) in pattern_locals.iter().enumerate() {
                        if conflicting_pattern_bindings.contains(name) {
                            let local_name = Self::match_pattern_local_name(
                                match_expr,
                                arm_index,
                                binding_index,
                                name,
                            );
                            self.set_local_alias(name, local_name.clone());
                            locals.push((local_name, *ty));
                        } else {
                            locals.push((name.clone(), *ty));
                        }
                    }

                    self.collect_locals_from_block_with_expected(
                        &arm.body,
                        locals,
                        expected_source,
                    )?;
                    self.pop_scope();
                }
            }
            Expr::Then(then) => {
                self.collect_locals_from_block_with_expected(
                    &then.then_block,
                    locals,
                    expected_source,
                )?;
                for (_, block) in &then.else_ifs {
                    self.collect_locals_from_block_with_expected(block, locals, expected_source)?;
                }
                if let Some(block) = &then.else_block {
                    self.collect_locals_from_block_with_expected(block, locals, expected_source)?;
                }
            }
            Expr::While(while_expr) => {
                self.collect_locals_from_block(&while_expr.body, locals)?;
            }
            Expr::With(with) => {
                let mut scoped_bindings = Vec::new();
                for binding in &with.bindings {
                    match binding {
                        FieldInit::Field { name, value } => {
                            self.collect_locals_from_expr(value, locals)?;
                            let source_ty = self
                                .record_field_type(&with.context_name, name)
                                .cloned()
                                .or_else(|| self.infer_expr_source_type(value));
                            let wasm_ty = if let Some(source_ty) = &source_ty {
                                self.convert_type(source_ty)?
                            } else {
                                self.infer_expr_type(value)?
                            };
                            locals.push((name.clone(), wasm_ty));
                            scoped_bindings.push((name.clone(), wasm_ty, source_ty));
                        }
                        FieldInit::Spread(expr) => {
                            self.collect_locals_from_expr(expr, locals)?;
                        }
                    }
                }

                self.push_scope();
                for (name, wasm_ty, source_ty) in &scoped_bindings {
                    self.set_local_type(name, *wasm_ty);
                    if let Some(source_ty) = source_ty {
                        self.set_local_source_type(name, source_ty.clone());
                        self.register_record_var_type(name, source_ty);
                    }
                }
                let result = self.collect_locals_from_block_with_expected(
                    &with.body,
                    locals,
                    expected_source,
                );
                self.pop_scope();
                result?;
            }
            Expr::WithLifetime(with_lifetime) => {
                self.collect_locals_from_block(&with_lifetime.body, locals)?;
            }
            Expr::Lambda(_) => {
                // Lambda bodies are emitted as separate functions, so their locals
                // must not leak into the enclosing function's declaration list.
            }
            Expr::Call(call) => {
                for arg in &call.args {
                    self.collect_locals_from_expr(arg, locals)?;
                }

                let expected_function_source = call
                    .args
                    .iter()
                    .map(|arg| self.infer_expr_source_type_for_abi(arg))
                    .collect::<Result<Vec<_>, _>>()
                    .ok()
                    .and_then(|arg_source_tys| {
                        self.callable_abi_for_arg_sources(&call.function, &arg_source_tys)
                            .ok()
                            .map(|abi| {
                                Type::Function(abi.source_params, Box::new(abi.source_result))
                            })
                    });

                self.collect_locals_from_expr_with_expected(
                    &call.function,
                    locals,
                    expected_function_source.as_ref(),
                )?;
            }
            Expr::Pipe(pipe) => match &pipe.target {
                PipeTarget::Ident(name) => {
                    let source_param = if name == "identity" {
                        None
                    } else if self.functions.contains_key(name) {
                        self.resolve_named_function_call_target(
                            name,
                            std::slice::from_ref(&pipe.expr),
                        )
                        .ok()
                        .and_then(|target_name| {
                            self.function_source_sigs
                                .get(&target_name)
                                .and_then(|sig| sig.params.first())
                                .cloned()
                        })
                    } else if self.lookup_local(name).is_some() {
                        let target = Expr::Ident(name.clone());
                        self.infer_expr_source_type_for_abi(&pipe.expr)
                            .ok()
                            .and_then(|arg_source_ty| {
                                self.callable_abi_for_arg_sources(&target, &[arg_source_ty])
                                    .ok()
                                    .and_then(|abi| abi.source_params.first().cloned())
                            })
                    } else {
                        None
                    };

                    self.collect_locals_from_expr_with_expected(
                        &pipe.expr,
                        locals,
                        source_param.as_ref(),
                    )?;
                }
                PipeTarget::Expr(target_expr) => {
                    let callable_context = self
                        .infer_expr_source_type_for_abi(&pipe.expr)
                        .ok()
                        .and_then(|arg_source_ty| {
                            self.callable_abi_for_arg_sources(
                                target_expr,
                                std::slice::from_ref(&arg_source_ty),
                            )
                            .ok()
                        });
                    let source_param = callable_context
                        .as_ref()
                        .and_then(|abi| abi.source_params.first())
                        .cloned();
                    let expected_function_source = callable_context
                        .map(|abi| Type::Function(abi.source_params, Box::new(abi.source_result)));

                    self.collect_locals_from_expr_with_expected(
                        &pipe.expr,
                        locals,
                        source_param.as_ref(),
                    )?;
                    self.collect_locals_from_expr_with_expected(
                        target_expr,
                        locals,
                        expected_function_source.as_ref(),
                    )?;
                }
            },
            Expr::Ok(inner) | Expr::Err(inner) | Expr::Some(inner) => {
                self.collect_locals_from_expr(inner, locals)?;
            }
            Expr::RangeLit(range) => {
                self.collect_locals_from_expr(&range.start, locals)?;
                self.collect_locals_from_expr(&range.end, locals)?;
            }
            Expr::Unary(unary) => {
                self.collect_locals_from_expr(&unary.expr, locals)?;
            }
            Expr::Cast(cast) => {
                self.collect_locals_from_expr(&cast.expr, locals)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn generate_pipe_expr(&mut self, pipe: &PipeExpr) -> Result<(), CodeGenError> {
        match &pipe.target {
            PipeTarget::Ident(name) => {
                // Check if this is a function or a binding
                if name == "identity" {
                    // identity is a no-op in the value pipeline.
                    self.generate_expr(&pipe.expr)?;
                } else if name == "println" {
                    // Special handling for generic println - determine type at runtime
                    let specialized_name = self.resolve_generic_function_call(name, &pipe.expr)?;
                    self.generate_expr(&pipe.expr)?;
                    self.output
                        .push_str(&format!("    call ${}\n", specialized_name));
                    // These functions return nothing, so we need to push unit value for pipe result
                    // But only if we're not in main function (which returns nothing)
                    if self.current_function != Some("main".to_string()) {
                        self.output.push_str("    i32.const 0\n");
                    }
                } else if self.functions.contains_key(name) {
                    // It's a function call: expr |> func
                    let target_name = self.resolve_named_function_call_target(
                        name,
                        std::slice::from_ref(&pipe.expr),
                    )?;
                    if let Some(source_param) = self
                        .function_source_sigs
                        .get(&target_name)
                        .and_then(|sig| sig.params.first())
                        .cloned()
                    {
                        self.generate_expr_with_expected_source(&pipe.expr, &source_param)?;
                    } else {
                        self.generate_expr(&pipe.expr)?;
                    }
                    self.output
                        .push_str(&format!("    call ${}\n", target_name));
                    // If function returns nothing, push unit value for pipe result
                    // But only if we're not in main function (which returns nothing)
                    if let Some(sig) = self.functions.get(name) {
                        if sig.result.is_none() && self.current_function != Some("main".to_string())
                        {
                            self.output.push_str("    i32.const 0\n");
                        }
                    }
                } else if self.lookup_local(name).is_some() {
                    // Function value stored in a local: expr |> f
                    let target = Expr::Ident(name.clone());
                    let arg_source_ty = self.infer_expr_source_type_for_abi(&pipe.expr)?;
                    let abi = self.callable_abi_for_arg_sources(&target, &[arg_source_ty])?;
                    let source_param = abi.source_params.first().ok_or_else(|| {
                        CodeGenError::UnsupportedFeature(
                            "function value pipe requires a single-argument function".to_string(),
                        )
                    })?;
                    self.generate_expr_with_expected_source(&pipe.expr, source_param)?;
                    self.generate_callable_value_with_abi(&target, &abi)?;
                    self.emit_typed_indirect_closure_call(&abi);
                } else {
                    // It's a new binding: expr |> name
                    // This should have been handled by the type checker to add the local
                    self.generate_expr(&pipe.expr)?;
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
                        if func_name == "identity" {
                            // identity is a no-op in the value pipeline.
                            self.generate_expr(&pipe.expr)?;
                        } else if self.functions.contains_key(func_name) {
                            let target_name = self.resolve_named_function_call_target(
                                func_name,
                                std::slice::from_ref(&pipe.expr),
                            )?;
                            if let Some(source_param) = self
                                .function_source_sigs
                                .get(&target_name)
                                .and_then(|sig| sig.params.first())
                                .cloned()
                            {
                                self.generate_expr_with_expected_source(&pipe.expr, &source_param)?;
                            } else {
                                self.generate_expr(&pipe.expr)?;
                            }
                            self.output
                                .push_str(&format!("    call ${}\n", target_name));
                            // If function returns nothing, push unit value for pipe result
                            // But only if we're not in main function (which returns nothing)
                            if let Some(sig) = self.functions.get(func_name) {
                                if sig.result.is_none()
                                    && self.current_function != Some("main".to_string())
                                {
                                    self.output.push_str("    i32.const 0\n");
                                }
                            }
                        } else if self.lookup_local(func_name).is_some() {
                            let target = Expr::Ident(func_name.clone());
                            let arg_source_ty = self.infer_expr_source_type_for_abi(&pipe.expr)?;
                            let abi =
                                self.callable_abi_for_arg_sources(&target, &[arg_source_ty])?;
                            let source_param = abi.source_params.first().ok_or_else(|| {
                                CodeGenError::UnsupportedFeature(
                                    "function value pipe requires a single-argument function"
                                        .to_string(),
                                )
                            })?;
                            self.generate_expr_with_expected_source(&pipe.expr, source_param)?;
                            self.generate_callable_value_with_abi(&target, &abi)?;
                            self.emit_typed_indirect_closure_call(&abi);
                        } else {
                            return Err(CodeGenError::UndefinedFunction(func_name.clone()));
                        }
                    }
                    _ => {
                        let arg_source_ty = self.infer_expr_source_type_for_abi(&pipe.expr)?;
                        let abi =
                            self.callable_abi_for_arg_sources(target_expr, &[arg_source_ty])?;
                        let source_param = abi.source_params.first().ok_or_else(|| {
                            CodeGenError::UnsupportedFeature(
                                "function value pipe requires a single-argument function"
                                    .to_string(),
                            )
                        })?;
                        self.generate_expr_with_expected_source(&pipe.expr, source_param)?;
                        self.generate_callable_value_with_abi(target_expr, &abi)?;
                        self.emit_typed_indirect_closure_call(&abi);
                    }
                }
            }
        }

        Ok(())
    }

    fn resolve_generic_function_call(
        &self,
        name: &str,
        arg_expr: &Expr,
    ) -> Result<String, CodeGenError> {
        if name == "println" {
            let source_ty = self.infer_expr_source_type(arg_expr).ok_or_else(|| {
                CodeGenError::UnsupportedFeature(
                    "println requires an inferable String or Int32 argument; use print_float for Float64"
                        .to_string(),
                )
            })?;

            match source_ty {
                Type::Named(type_name) if type_name == "String" => Ok("println".to_string()),
                Type::Named(type_name) if type_name == "Int32" => Ok("print_int".to_string()),
                other => Err(CodeGenError::UnsupportedFeature(format!(
                    "println does not support argument type {}; use print_int, print_float, or a String value",
                    other
                ))),
            }
        } else {
            match self.infer_expr_source_type(arg_expr) {
                Some(Type::Named(type_name)) => Ok(format!("{}_{}", name, type_name)),
                Some(other) => Err(CodeGenError::UnsupportedFeature(format!(
                    "generic function '{}' requires a concrete named specialization for argument type {}",
                    name, other
                ))),
                None => Err(CodeGenError::UnsupportedFeature(format!(
                    "generic function '{}' requires an inferable argument type for specialization",
                    name
                ))),
            }
        }
    }

    fn generate_list_literal(&mut self, items: &[Box<Expr>]) -> Result<(), CodeGenError> {
        self.generate_list_literal_with_expected(items, None)
    }

    fn generate_range_literal(&mut self, range: &RangeLit) -> Result<(), CodeGenError> {
        self.output.push_str("    i32.const 8 ;; range size\n");
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $list_tmp\n");

        self.output.push_str("    local.get $list_tmp\n");
        self.generate_expr_with_expected_source(&range.start, &Type::Named("Int32".to_string()))?;
        self.output.push_str("    i32.store\n");

        self.output.push_str("    local.get $list_tmp\n");
        self.output
            .push_str("    i32.const 4 ;; range end offset\n");
        self.output.push_str("    i32.add\n");
        self.generate_expr_with_expected_source(&range.end, &Type::Named("Int32".to_string()))?;
        self.output.push_str("    i32.store\n");

        self.output.push_str("    local.get $list_tmp\n");
        Ok(())
    }

    fn generate_list_literal_with_expected(
        &mut self,
        items: &[Box<Expr>],
        element_source_ty: Option<&Type>,
    ) -> Result<(), CodeGenError> {
        let element_type = if let Some(source_ty) = element_source_ty {
            self.convert_type(source_ty)?
        } else {
            self.infer_collection_element_wasm_type(items)?
        };
        let element_size = self.wasm_type_size(element_type);
        let list_size = 8 + (items.len() * element_size); // Header (length + capacity) + elements

        // Allocate memory for the list
        self.output
            .push_str(&format!("    i32.const {} ;; list size\n", list_size));
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $list_tmp\n");

        // Write length
        self.output.push_str("    local.get $list_tmp\n");
        self.output
            .push_str(&format!("    i32.const {} ;; length\n", items.len()));
        self.output.push_str("    i32.store\n");

        // Write capacity (same as length for literals)
        self.output.push_str("    local.get $list_tmp\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output
            .push_str(&format!("    i32.const {} ;; capacity\n", items.len()));
        self.output.push_str("    i32.store\n");

        // Write elements
        for (i, item) in items.iter().enumerate() {
            self.output.push_str("    local.get $list_tmp\n");
            self.output.push_str(&format!(
                "    i32.const {} ;; offset to element {}\n",
                8 + (i * element_size),
                i
            ));
            self.output.push_str("    i32.add\n");
            if let Some(source_ty) = element_source_ty {
                self.generate_expr_with_expected_source(item, source_ty)?;
            } else {
                self.generate_expr(item)?;
            }
            self.output.push_str(&format!(
                "    {}\n",
                self.wasm_store_op_for_wasm_type(element_type)
            ));
        }

        // Return the list pointer
        self.output.push_str("    local.get $list_tmp\n");

        Ok(())
    }

    fn generate_array_literal(&mut self, items: &[Box<Expr>]) -> Result<(), CodeGenError> {
        self.generate_array_literal_with_expected(items, None)
    }

    fn generate_array_literal_with_expected(
        &mut self,
        items: &[Box<Expr>],
        element_source_ty: Option<&Type>,
    ) -> Result<(), CodeGenError> {
        let element_type = if let Some(source_ty) = element_source_ty {
            self.convert_type(source_ty)?
        } else {
            self.infer_collection_element_wasm_type(items)?
        };
        let element_size = self.wasm_type_size(element_type);
        let array_size = 8 + (items.len() * element_size);

        // Allocate memory for the array: length + element-size metadata + elements.
        self.output
            .push_str(&format!("    i32.const {} ;; array size\n", array_size));
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.tee $list_tmp\n"); // Save and leave on stack

        // Write length
        self.output.push_str("    local.get $list_tmp\n");
        self.output
            .push_str(&format!("    i32.const {} ;; array length\n", items.len()));
        self.output.push_str("    i32.store\n");

        // Write element-size metadata. The current runtime only needs length,
        // but the second header word keeps the layout explicit and extensible.
        self.output.push_str("    local.get $list_tmp\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str(&format!(
            "    i32.const {} ;; array element size\n",
            element_size
        ));
        self.output.push_str("    i32.store\n");

        // Write elements
        for (i, item) in items.iter().enumerate() {
            self.output.push_str("    local.get $list_tmp\n");
            self.output.push_str(&format!(
                "    i32.const {} ;; offset to element {}\n",
                8 + (i * element_size),
                i
            ));
            self.output.push_str("    i32.add\n");
            if let Some(source_ty) = element_source_ty {
                self.generate_expr_with_expected_source(item, source_ty)?;
            } else {
                self.generate_expr(item)?;
            }
            self.output.push_str(&format!(
                "    {}\n",
                self.wasm_store_op_for_wasm_type(element_type)
            ));
        }

        // Array pointer is already on stack from local.tee

        Ok(())
    }

    fn infer_collection_element_wasm_type(
        &self,
        items: &[Box<Expr>],
    ) -> Result<WasmType, CodeGenError> {
        if let Some(source_ty) = self.infer_collection_element_source_type(items) {
            return self.convert_type(&source_ty);
        }

        let Some(first) = items.first() else {
            return Ok(WasmType::I32);
        };

        self.infer_expr_type(first)
    }

    fn infer_collection_element_source_type(&self, items: &[Box<Expr>]) -> Option<Type> {
        items
            .first()
            .and_then(|first| self.infer_expr_source_type(first))
    }

    fn generate_match_expr(&mut self, match_expr: &MatchExpr) -> Result<(), CodeGenError> {
        self.generate_match_expr_with_expected_source(match_expr, None)
    }

    fn generate_match_expr_with_expected_source(
        &mut self,
        match_expr: &MatchExpr,
        expected_source: Option<&Type>,
    ) -> Result<(), CodeGenError> {
        if match_expr.arms.is_empty() {
            self.output.push_str("    unreachable\n");
            return Ok(());
        }

        let result_type = if let Some(expected_source) = expected_source {
            self.convert_type(expected_source)?
        } else {
            self.infer_match_result_type(match_expr)?
        };
        let result_type_name = self.wasm_type_str(result_type);
        let scrutinee_source_ty = self.infer_expr_source_type(&match_expr.expr);
        let scrutinee_wasm_ty = if let Some(source_ty) = &scrutinee_source_ty {
            self.convert_type(source_ty)?
        } else {
            self.infer_expr_type(&match_expr.expr)?
        };
        let match_local = self.match_temp_local(scrutinee_wasm_ty);
        let conflicting_pattern_bindings = self.match_pattern_binding_conflicts(
            match_expr,
            scrutinee_source_ty.as_ref(),
            scrutinee_wasm_ty,
        )?;

        // First evaluate the expression being matched
        self.generate_expr(&match_expr.expr)?;
        self.output
            .push_str(&format!("    local.set ${}\n", match_local));

        // Generate a series of if-else blocks for each pattern
        for (i, arm) in match_expr.arms.iter().enumerate() {
            if i > 0 {
                self.output.push_str("      (else\n");
            }

            // Generate pattern matching code
            self.output
                .push_str(&format!("    local.get ${}\n", match_local));
            let bindings = self.generate_pattern_match(
                &arm.pattern,
                scrutinee_source_ty.as_ref(),
                match_local,
            )?;
            let mut binding_infos = Vec::new();
            self.collect_pattern_binding_types(
                &arm.pattern,
                scrutinee_source_ty.as_ref(),
                scrutinee_wasm_ty,
                &mut binding_infos,
            )?;

            self.output
                .push_str(&format!("    (if (result {})\n", result_type_name));
            self.output.push_str("      (then\n");

            self.push_scope();
            // Apply bindings
            for (binding_index, (name, load_code)) in bindings.into_iter().enumerate() {
                let (_, wasm_ty, source_ty) =
                    binding_infos.get(binding_index).cloned().ok_or_else(|| {
                        CodeGenError::UnsupportedFeature(format!(
                            "missing pattern binding metadata for '{}'",
                            name
                        ))
                    })?;
                let local_name = if conflicting_pattern_bindings.contains(&name) {
                    let local_name =
                        Self::match_pattern_local_name(match_expr, i, binding_index, &name);
                    self.set_local_alias(&name, local_name.clone());
                    local_name
                } else {
                    name.clone()
                };
                self.output.push_str(&load_code);
                self.output
                    .push_str(&format!("        local.set ${}\n", local_name));
                self.set_local_type(&name, wasm_ty);
                if let Some(source_ty) = source_ty {
                    self.set_local_source_type(&name, source_ty);
                }
            }

            // Generate arm body as expression (match arms should produce values)
            self.generate_block_internal(&arm.body, true, expected_source)?;
            self.pop_scope();

            self.output.push_str("      )\n");
        }

        // Matches are expected to be exhaustive after type checking. If codegen
        // reaches this path at runtime, trap instead of manufacturing a value.
        self.output.push_str("      (else\n");
        self.output.push_str("        unreachable\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");

        // Close all containing else blocks and their if expressions.
        for _ in 1..match_expr.arms.len() {
            self.output.push_str("      )\n");
            self.output.push_str("    )\n");
        }

        Ok(())
    }

    fn infer_match_result_type(&self, match_expr: &MatchExpr) -> Result<WasmType, CodeGenError> {
        match match_expr.arms.first() {
            Some(arm) => self.infer_block_result_type(&arm.body),
            None => Ok(WasmType::I32),
        }
    }

    fn generate_pattern_match(
        &mut self,
        pattern: &Pattern,
        source_ty: Option<&Type>,
        match_local: &str,
    ) -> Result<Vec<(String, String)>, CodeGenError> {
        let mut bindings = Vec::new();

        match pattern {
            Pattern::Wildcard => {
                // Always matches, no bindings
                self.output.push_str("    drop\n");
                self.output
                    .push_str("    i32.const 1 ;; wildcard always matches\n");
            }
            Pattern::Ident(name) => {
                // Always matches, bind the value
                bindings.push((name.clone(), format!("    local.get ${}\n", match_local)));
                self.output.push_str("    drop\n");
                self.output
                    .push_str("    i32.const 1 ;; var always matches\n");
            }
            Pattern::Literal(lit) => match lit {
                Literal::Int(n) => {
                    if matches!(source_ty, Some(Type::Named(name)) if name == "Int64") {
                        self.output.push_str(&format!("    i64.const {}\n", n));
                        self.output.push_str("    i64.eq\n");
                    } else {
                        self.output.push_str(&format!("    i32.const {}\n", n));
                        self.output.push_str("    i32.eq\n");
                    }
                }
                Literal::String(value) => {
                    let offset = self.string_offsets.get(value).ok_or_else(|| {
                        CodeGenError::NotImplemented("string literal not in pool".to_string())
                    })?;
                    self.output.push_str(&format!("    i32.const {}\n", offset));
                    self.output.push_str("    call $string_eq\n");
                }
                Literal::Float(value) => {
                    self.output.push_str(&format!("    f64.const {}\n", value));
                    self.output.push_str("    f64.eq\n");
                }
                Literal::Char(c) => {
                    self.output
                        .push_str(&format!("    i32.const {}\n", *c as u32));
                    self.output.push_str("    i32.eq\n");
                }
                Literal::Bool(b) => {
                    self.output
                        .push_str(&format!("    i32.const {}\n", if *b { 1 } else { 0 }));
                    self.output.push_str("    i32.eq\n");
                }
                Literal::Unit => {
                    self.output.push_str("    i32.const 0\n");
                    self.output.push_str("    i32.eq\n");
                }
            },
            Pattern::EmptyList => {
                // Check if list is empty
                self.output.push_str("    call $list_length\n");
                self.output.push_str("    i32.const 0\n");
                self.output.push_str("    i32.eq\n");
            }
            Pattern::ListExact(patterns) => {
                let element_source_ty = self.list_element_source_type(source_ty);
                let element_wasm_ty = element_source_ty
                    .as_ref()
                    .map(|ty| self.convert_type(ty))
                    .transpose()?
                    .unwrap_or(WasmType::I32);
                let element_local = self.payload_temp_local(element_wasm_ty);
                let list_get_fn = self.list_get_function_for_element(element_source_ty.as_ref());

                // Check length first
                self.output.push_str("    call $list_length\n");
                self.output
                    .push_str(&format!("    i32.const {}\n", patterns.len()));
                self.output.push_str("    i32.eq\n");

                // For each element pattern
                for (i, pattern) in patterns.iter().enumerate() {
                    self.output.push_str("    (if (result i32)\n");
                    self.output.push_str("      (then\n");
                    self.output
                        .push_str(&format!("        local.get ${}\n", match_local));
                    self.output.push_str(&format!("        i32.const {}\n", i));
                    self.output
                        .push_str(&format!("        call ${}\n", list_get_fn));
                    self.output
                        .push_str(&format!("        local.set ${}\n", element_local));
                    self.output
                        .push_str(&format!("        local.get ${}\n", element_local));

                    let sub_bindings = self.generate_pattern_match(
                        pattern,
                        element_source_ty.as_ref(),
                        element_local,
                    )?;
                    self.output.push_str("        (if (result i32)\n");
                    self.output.push_str("          (then\n");
                    for (name, load_code) in sub_bindings {
                        self.output.push_str(&load_code);
                        self.output
                            .push_str(&format!("            local.set ${}\n", name));
                    }
                    self.output
                        .push_str("            i32.const 1 ;; element pattern matched\n");
                    self.output.push_str("          )\n");
                    self.output.push_str("          (else\n");
                    self.output
                        .push_str("            i32.const 0 ;; element pattern failed\n");
                    self.output.push_str("          )\n");
                    self.output.push_str("        )\n");

                    self.output.push_str("      )\n");
                    self.output.push_str("      (else\n");
                    self.output
                        .push_str("        i32.const 0 ;; pattern failed\n");
                    self.output.push_str("      )\n");
                    self.output.push_str("    )\n");
                }
            }
            Pattern::ListCons(head_pattern, tail_pattern) => {
                let element_source_ty = self.list_element_source_type(source_ty);
                let element_wasm_ty = element_source_ty
                    .as_ref()
                    .map(|ty| self.convert_type(ty))
                    .transpose()?
                    .unwrap_or(WasmType::I32);
                let element_local = self.payload_temp_local(element_wasm_ty);
                let list_get_fn = self.list_get_function_for_element(element_source_ty.as_ref());
                let tail_fn = self.list_tail_function_for_element(element_source_ty.as_ref());

                // Check that list is not empty
                self.output
                    .push_str("    local.tee $tail_tmp ;; save list for tail\n");
                self.output.push_str("    call $list_length\n");
                self.output
                    .push_str("    local.tee $tail_len ;; save length\n");
                self.output.push_str("    i32.const 0\n");
                self.output.push_str("    i32.gt_u ;; length > 0\n");

                // Match head
                self.output.push_str("    (if (result i32)\n");
                self.output.push_str("      (then\n");
                self.output.push_str("        local.get $tail_tmp\n");
                self.output.push_str("        i32.const 0\n");
                self.output
                    .push_str(&format!("        call ${}\n", list_get_fn));
                self.output
                    .push_str(&format!("        local.set ${}\n", element_local));
                self.output
                    .push_str(&format!("        local.get ${}\n", element_local));

                let head_bindings = self.generate_pattern_match(
                    head_pattern,
                    element_source_ty.as_ref(),
                    element_local,
                )?;

                // Get tail
                self.output.push_str("        (if (result i32)\n");
                self.output.push_str("          (then\n");
                for (name, load_code) in head_bindings {
                    self.output.push_str(&load_code);
                    self.output
                        .push_str(&format!("            local.set ${}\n", name));
                }
                self.output.push_str("            local.get $tail_tmp\n");
                self.output
                    .push_str(&format!("            call ${}\n", tail_fn));
                self.output.push_str("            local.set $tail_tmp\n");
                self.output.push_str("            local.get $tail_tmp\n");

                let tail_bindings =
                    self.generate_pattern_match(tail_pattern, source_ty, "tail_tmp")?;
                self.output.push_str("            (if (result i32)\n");
                self.output.push_str("              (then\n");
                for (name, load_code) in tail_bindings {
                    self.output.push_str(&load_code);
                    self.output
                        .push_str(&format!("                local.set ${}\n", name));
                }
                self.output
                    .push_str("                i32.const 1 ;; tail pattern matched\n");
                self.output.push_str("              )\n");
                self.output.push_str("              (else\n");
                self.output
                    .push_str("                i32.const 0 ;; tail pattern failed\n");
                self.output.push_str("              )\n");
                self.output.push_str("            )\n");

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
            Pattern::Record(record_name, field_patterns) => {
                return self.generate_record_pattern_match(
                    record_name,
                    source_ty,
                    field_patterns,
                    None,
                );
            }
            Pattern::Some(inner_pattern) => {
                let payload_ty = self.variant_payload_type(source_ty, "Some");
                // Check if tag is 1 (Some)
                self.output
                    .push_str("    local.tee $option_value_tmp ;; save for value extraction\n");
                self.output.push_str("    i32.load ;; load tag\n");
                self.output.push_str("    i32.const 1 ;; Some tag\n");
                self.output.push_str("    i32.eq\n");

                match inner_pattern.as_ref() {
                    Pattern::Ident(name) => {
                        bindings.push((
                            name.clone(),
                            self.variant_payload_load_code(
                                "option_value_tmp",
                                payload_ty,
                                "        ",
                            )?,
                        ));
                    }
                    Pattern::Wildcard => {}
                    _ => {
                        // If tag matches, match the inner pattern.
                        self.output.push_str("    (if (result i32)\n");
                        self.output.push_str("      (then\n");
                        self.emit_variant_payload_load("option_value_tmp", payload_ty)?;

                        let inner_bindings =
                            self.generate_pattern_match(inner_pattern, payload_ty, "match_tmp")?;
                        bindings.extend(inner_bindings);

                        self.output.push_str("      )\n");
                        self.output.push_str("      (else\n");
                        self.output
                            .push_str("        i32.const 0 ;; tag mismatch\n");
                        self.output.push_str("      )\n");
                        self.output.push_str("    )\n");
                    }
                }
            }
            Pattern::None => {
                // Check if tag is 0 (None)
                self.output.push_str("    i32.load ;; load tag\n");
                self.output.push_str("    i32.const 0 ;; None tag\n");
                self.output.push_str("    i32.eq\n");
            }
            Pattern::Ok(inner_pattern) => {
                let payload_ty = self.variant_payload_type(source_ty, "Ok");
                self.output
                    .push_str("    local.tee $option_value_tmp ;; save for Ok extraction\n");
                self.output.push_str("    i32.load ;; load tag\n");
                self.output.push_str("    i32.const 1 ;; Ok tag\n");
                self.output.push_str("    i32.eq\n");

                match inner_pattern.as_ref() {
                    Pattern::Ident(name) => {
                        bindings.push((
                            name.clone(),
                            self.variant_payload_load_code(
                                "option_value_tmp",
                                payload_ty,
                                "        ",
                            )?,
                        ));
                    }
                    Pattern::Wildcard => {}
                    _ => {
                        self.output.push_str("    (if (result i32)\n");
                        self.output.push_str("      (then\n");
                        self.emit_variant_payload_load("option_value_tmp", payload_ty)?;

                        let inner_bindings =
                            self.generate_pattern_match(inner_pattern, payload_ty, "match_tmp")?;
                        bindings.extend(inner_bindings);

                        self.output.push_str("      )\n");
                        self.output.push_str("      (else\n");
                        self.output
                            .push_str("        i32.const 0 ;; tag mismatch\n");
                        self.output.push_str("      )\n");
                        self.output.push_str("    )\n");
                    }
                }
            }
            Pattern::Err(inner_pattern) => {
                let payload_ty = self.variant_payload_type(source_ty, "Err");
                self.output
                    .push_str("    local.tee $option_value_tmp ;; save for Err extraction\n");
                self.output.push_str("    i32.load ;; load tag\n");
                self.output.push_str("    i32.const 0 ;; Err tag\n");
                self.output.push_str("    i32.eq\n");

                match inner_pattern.as_ref() {
                    Pattern::Ident(name) => {
                        bindings.push((
                            name.clone(),
                            self.variant_payload_load_code(
                                "option_value_tmp",
                                payload_ty,
                                "        ",
                            )?,
                        ));
                    }
                    Pattern::Wildcard => {}
                    _ => {
                        self.output.push_str("    (if (result i32)\n");
                        self.output.push_str("      (then\n");
                        self.emit_variant_payload_load("option_value_tmp", payload_ty)?;

                        let inner_bindings =
                            self.generate_pattern_match(inner_pattern, payload_ty, "match_tmp")?;
                        bindings.extend(inner_bindings);

                        self.output.push_str("      )\n");
                        self.output.push_str("      (else\n");
                        self.output
                            .push_str("        i32.const 0 ;; tag mismatch\n");
                        self.output.push_str("      )\n");
                        self.output.push_str("    )\n");
                    }
                }
            }
            Pattern::RecordDestruct {
                type_name,
                fields,
                rest,
            } => {
                return self.generate_record_pattern_match(
                    type_name,
                    source_ty,
                    fields,
                    rest.as_ref(),
                );
            }
        }

        Ok(bindings)
    }

    fn generate_record_pattern_match(
        &mut self,
        record_name: &str,
        source_ty: Option<&Type>,
        field_patterns: &[(String, Pattern)],
        rest: Option<&String>,
    ) -> Result<Vec<(String, String)>, CodeGenError> {
        let mut bindings = Vec::new();

        // Type checking has already proven the scrutinee is this record type.
        self.output.push_str("    local.set $match_tmp\n");

        if let Some(rest_name) = rest {
            if rest_name != "_" {
                let (residual_name, load_code) = self.record_rest_load_code(
                    record_name,
                    source_ty,
                    field_patterns,
                    "match_tmp",
                    "        ",
                )?;
                bindings.push((rest_name.clone(), load_code));
                self.set_local_source_type(rest_name, Type::Named(residual_name.clone()));
                self.var_types.insert(rest_name.clone(), residual_name);
            }
        }

        if field_patterns.is_empty() {
            self.output
                .push_str("    i32.const 1 ;; empty record pattern always matches\n");
            return Ok(bindings);
        }

        for (index, (field_name, field_pattern)) in field_patterns.iter().enumerate() {
            self.generate_record_field_pattern_condition(
                record_name,
                source_ty,
                field_name,
                field_pattern,
                &mut bindings,
            )?;

            if index > 0 {
                self.output.push_str("    i32.and\n");
            }
        }

        Ok(bindings)
    }

    fn generate_record_field_pattern_condition(
        &mut self,
        record_name: &str,
        source_ty: Option<&Type>,
        field_name: &str,
        field_pattern: &Pattern,
        bindings: &mut Vec<(String, String)>,
    ) -> Result<(), CodeGenError> {
        match field_pattern {
            Pattern::Ident(name) => {
                let load_code = self.record_field_load_code(
                    record_name,
                    source_ty,
                    "match_tmp",
                    field_name,
                    "        ",
                )?;
                bindings.push((name.clone(), load_code));
                self.output
                    .push_str("    i32.const 1 ;; field binding always matches\n");
            }
            Pattern::Wildcard => {
                self.output
                    .push_str("    i32.const 1 ;; wildcard field always matches\n");
            }
            Pattern::Literal(lit) => {
                self.load_record_field_from_local(record_name, "match_tmp", field_name)?;
                match lit {
                    Literal::Int(n) => {
                        if matches!(
                            self.record_field_type(record_name, field_name),
                            Some(Type::Named(name)) if name == "Int64"
                        ) {
                            self.output.push_str(&format!("    i64.const {}\n", n));
                            self.output.push_str("    i64.eq\n");
                        } else {
                            self.output.push_str(&format!("    i32.const {}\n", n));
                            self.output.push_str("    i32.eq\n");
                        }
                    }
                    Literal::Bool(value) => {
                        self.output
                            .push_str(&format!("    i32.const {}\n", if *value { 1 } else { 0 }));
                        self.output.push_str("    i32.eq\n");
                    }
                    Literal::Float(value) => {
                        self.output.push_str(&format!("    f64.const {}\n", value));
                        self.output.push_str("    f64.eq\n");
                    }
                    Literal::Unit => {
                        self.output.push_str("    i32.const 0\n");
                        self.output.push_str("    i32.eq\n");
                    }
                    Literal::String(value) => {
                        let offset = self.string_offsets.get(value).ok_or_else(|| {
                            CodeGenError::NotImplemented("string literal not in pool".to_string())
                        })?;
                        self.output.push_str(&format!("    i32.const {}\n", offset));
                        self.output.push_str("    call $string_eq\n");
                    }
                    Literal::Char(c) => {
                        self.output
                            .push_str(&format!("    i32.const {}\n", *c as u32));
                        self.output.push_str("    i32.eq\n");
                    }
                }
            }
            nested_pattern => {
                let field_ty =
                    self.instantiated_record_field_type_by_name(record_name, source_ty, field_name);
                if field_ty.is_none() {
                    return Err(CodeGenError::NotImplemented(format!(
                        "field pattern type for {} in record {}",
                        field_name, record_name
                    )));
                }

                if self.record_pattern_depth >= self.record_tmp_count {
                    return Err(CodeGenError::NotImplemented(format!(
                        "record pattern nesting deeper than {} levels",
                        self.record_tmp_count
                    )));
                }
                let parent_tmp = format!("record_tmp_{}", self.record_pattern_depth);

                self.output.push_str("    local.get $match_tmp\n");
                self.output
                    .push_str(&format!("    local.set ${}\n", parent_tmp));
                let load_code = self.record_field_load_code(
                    record_name,
                    source_ty,
                    &parent_tmp,
                    field_name,
                    "    ",
                )?;
                self.output.push_str(&load_code);
                self.output.push_str("    local.set $match_tmp\n");
                self.output.push_str("    local.get $match_tmp\n");

                self.record_pattern_depth += 1;
                let nested_bindings_result =
                    self.generate_pattern_match(nested_pattern, field_ty.as_ref(), "match_tmp");
                self.record_pattern_depth -= 1;
                let nested_bindings = nested_bindings_result?;

                self.output.push_str("    (if (result i32)\n");
                self.output.push_str("      (then\n");
                for (name, load_code) in nested_bindings {
                    self.output.push_str(&load_code);
                    self.output
                        .push_str(&format!("        local.set ${}\n", name));
                }
                self.output
                    .push_str("        i32.const 1 ;; nested field pattern matched\n");
                self.output.push_str("      )\n");
                self.output.push_str("      (else\n");
                self.output
                    .push_str("        i32.const 0 ;; nested field pattern failed\n");
                self.output.push_str("      )\n");
                self.output.push_str("    )\n");
                self.output
                    .push_str(&format!("    local.get ${}\n", parent_tmp));
                self.output.push_str("    local.set $match_tmp\n");
            }
        }

        Ok(())
    }

    fn record_field_load_code(
        &self,
        record_name: &str,
        source_ty: Option<&Type>,
        source_local: &str,
        field_name: &str,
        indent: &str,
    ) -> Result<String, CodeGenError> {
        let field_offset =
            self.instantiated_record_field_offset(record_name, source_ty, field_name)?;
        let field_type =
            self.instantiated_record_field_type_by_name(record_name, source_ty, field_name);
        Ok(format!(
            "{indent}local.get ${source_local}\n{indent}i32.const {field_offset}\n{indent}i32.add\n{indent}{}\n",
            self.wasm_load_op_for_type(field_type.as_ref())
        ))
    }

    fn generate_then_expr(&mut self, then: &ThenExpr) -> Result<(), CodeGenError> {
        self.generate_then_expr_with_expected_source(then, None)
    }

    fn generate_then_expr_with_expected_source(
        &mut self,
        then: &ThenExpr,
        expected_source: Option<&Type>,
    ) -> Result<(), CodeGenError> {
        let result_type = if let Some(expected_source) = expected_source {
            self.convert_type(expected_source)?
        } else {
            self.infer_then_result_type(then)?
        };

        // Generate condition
        self.generate_expr(&then.condition)?;

        self.output.push_str(&format!(
            "    (if (result {})\n",
            self.wasm_type_str(result_type)
        ));
        self.output.push_str("      (then\n");
        self.push_scope();
        let then_result = self.generate_block_internal(&then.then_block, true, expected_source);
        self.pop_scope();
        then_result?;
        self.output.push_str("      )\n");

        self.output.push_str("      (else\n");
        self.generate_then_else_chain(
            &then.else_ifs,
            then.else_block.as_ref(),
            0,
            result_type,
            expected_source,
        )?;
        self.output.push_str("      )\n");

        self.output.push_str("    )\n");

        Ok(())
    }

    fn generate_then_else_chain(
        &mut self,
        else_ifs: &[(Box<Expr>, BlockExpr)],
        else_block: Option<&BlockExpr>,
        index: usize,
        result_type: WasmType,
        expected_source: Option<&Type>,
    ) -> Result<(), CodeGenError> {
        if let Some((condition, block)) = else_ifs.get(index) {
            self.generate_expr(condition)?;
            self.output.push_str(&format!(
                "        (if (result {})\n",
                self.wasm_type_str(result_type)
            ));
            self.output.push_str("          (then\n");
            self.push_scope();
            let then_result = self.generate_block_internal(block, true, expected_source);
            self.pop_scope();
            then_result?;
            self.output.push_str("          )\n");
            self.output.push_str("          (else\n");
            self.generate_then_else_chain(
                else_ifs,
                else_block,
                index + 1,
                result_type,
                expected_source,
            )?;
            self.output.push_str("          )\n");
            self.output.push_str("        )\n");
        } else if let Some(block) = else_block {
            self.push_scope();
            let else_result = self.generate_block_internal(block, true, expected_source);
            self.pop_scope();
            else_result?;
        } else {
            self.emit_zero_value(result_type, "unit");
        }

        Ok(())
    }

    fn emit_zero_value(&mut self, ty: WasmType, comment: &str) {
        match ty {
            WasmType::I32 => self
                .output
                .push_str(&format!("        i32.const 0 ;; {}\n", comment)),
            WasmType::I64 => self
                .output
                .push_str(&format!("        i64.const 0 ;; {}\n", comment)),
            WasmType::F32 => self
                .output
                .push_str(&format!("        f32.const 0 ;; {}\n", comment)),
            WasmType::F64 => self
                .output
                .push_str(&format!("        f64.const 0 ;; {}\n", comment)),
        }
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

    fn generate_with_expr(&mut self, with_expr: &WithExpr) -> Result<(), CodeGenError> {
        self.generate_with_expr_with_expected_source(with_expr, None)
    }

    fn generate_with_expr_with_expected_source(
        &mut self,
        with_expr: &WithExpr,
        expected_source: Option<&Type>,
    ) -> Result<(), CodeGenError> {
        // Generate with context expression:
        // 1. Establish context scope
        // 2. Bind context field values to local variables (if provided)
        // 3. Generate body with context available
        // 4. Clean up context scope

        self.output.push_str(&format!(
            "    ;; With context: {}\n",
            with_expr.context_name
        ));

        self.push_scope();
        let result = self.generate_with_expr_inner(with_expr, expected_source);
        self.pop_scope();
        result
    }

    fn generate_with_expr_inner(
        &mut self,
        with_expr: &WithExpr,
        expected_source: Option<&Type>,
    ) -> Result<(), CodeGenError> {
        let arena_scope = if with_expr.context_name == "Arena" {
            Some(self.begin_with_arena_scope()?)
        } else {
            None
        };

        if !with_expr.bindings.is_empty() {
            self.output.push_str("    ;; Context field bindings:\n");
            for binding in &with_expr.bindings {
                match binding {
                    FieldInit::Field { name, value } => {
                        self.output
                            .push_str(&format!("    ;; Bind context field '{}'\n", name));

                        let source_ty = self
                            .record_field_type(&with_expr.context_name, name)
                            .cloned()
                            .or_else(|| self.infer_expr_source_type(value));
                        let wasm_ty = if let Some(source_ty) = &source_ty {
                            self.generate_expr_with_expected_source(value, source_ty)?;
                            self.convert_type(source_ty)?
                        } else {
                            self.generate_expr(value)?;
                            self.infer_expr_type(value)?
                        };

                        if self.lookup_local(name).is_none() {
                            return Err(CodeGenError::UndefinedVariable(name.clone()));
                        }

                        self.output.push_str(&format!("    local.set ${}\n", name));
                        self.set_local_type(name, wasm_ty);
                        if let Some(source_ty) = source_ty {
                            self.set_local_source_type(name, source_ty.clone());
                            self.register_record_var_type(name, &source_ty);
                        }
                    }
                    FieldInit::Spread(_) => {
                        return Err(CodeGenError::NotImplemented(
                            "spread operations in context bindings".to_string(),
                        ));
                    }
                }
            }
        }

        // Generate the body block
        self.output.push_str("    ;; Context body:\n");
        self.generate_block_internal(&with_expr.body, true, expected_source)?;

        if let Some((depth, arena_addr)) = arena_scope {
            self.end_with_arena_scope(depth, arena_addr)?;
        }

        self.output.push_str("    ;; End with context\n");

        Ok(())
    }

    fn begin_with_arena_scope(&mut self) -> Result<(usize, u32), CodeGenError> {
        if self.with_arena_depth >= WITH_ARENA_TMP_COUNT {
            return Err(CodeGenError::UnsupportedFeature(format!(
                "`with Arena` nesting deeper than {} scopes",
                WITH_ARENA_TMP_COUNT
            )));
        }

        let depth = self.with_arena_depth;
        self.with_arena_depth += 1;

        let arena_addr = self.next_arena_addr;
        self.next_arena_addr += ARENA_SIZE_BYTES;
        self.arena_stack.push(arena_addr);

        self.output.push_str("    ;; Enter with Arena scope\n");
        self.output.push_str("    global.get $current_arena\n");
        self.output
            .push_str(&format!("    local.set $with_prev_arena_{}\n", depth));
        self.output
            .push_str(&format!("    i32.const {}\n", arena_addr));
        self.output.push_str("    call $arena_init\n");
        self.output.push_str("    global.set $current_arena\n");

        Ok((depth, arena_addr))
    }

    fn end_with_arena_scope(&mut self, depth: usize, arena_addr: u32) -> Result<(), CodeGenError> {
        self.output.push_str("    ;; Exit with Arena scope\n");
        self.output
            .push_str(&format!("    i32.const {}\n", arena_addr));
        self.output.push_str("    call $arena_reset\n");
        self.output
            .push_str(&format!("    local.get $with_prev_arena_{}\n", depth));
        self.output.push_str("    global.set $current_arena\n");

        self.arena_stack.pop();
        self.with_arena_depth -= 1;
        Ok(())
    }

    fn generate_clone_expr(&mut self, clone: &CloneExpr) -> Result<(), CodeGenError> {
        // Clone expressions create a new record by copying the base record
        // and updating specified fields with new values

        let base_source_ty = self.infer_expr_source_type(&clone.base).ok_or_else(|| {
            CodeGenError::UnsupportedFeature(
                "record clone requires a base expression with a known record type".to_string(),
            )
        })?;
        let record_name = self
            .source_record_name(&base_source_ty)
            .ok_or_else(|| {
                CodeGenError::UnsupportedFeature(format!(
                    "record clone requires a record base, found {}",
                    base_source_ty
                ))
            })?
            .to_string();
        let record_size = self.instantiated_record_size(
            &record_name,
            Some(&base_source_ty),
            clone.updates.fields.len(),
        );

        // Allocate memory for the new record
        self.output
            .push_str(&format!("    i32.const {} ;; record size\n", record_size));
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $clone_tmp\n");

        // Generate base expression to get the original record
        self.generate_expr(&clone.base)?;
        self.output.push_str("    local.set $base_tmp\n");

        // Copy all fields from base record to new record
        // For simplicity, we'll copy the entire record memory
        self.output
            .push_str("    local.get $clone_tmp ;; destination\n");
        self.output.push_str("    local.get $base_tmp ;; source\n");
        self.output
            .push_str(&format!("    i32.const {} ;; size\n", record_size));
        self.output.push_str("    memory.copy\n");

        // Now update the specified fields with new values
        for field_init in &clone.updates.fields {
            match field_init {
                FieldInit::Field { name, value } => {
                    let field_offset = self.instantiated_record_field_offset(
                        &record_name,
                        Some(&base_source_ty),
                        name,
                    )?;
                    let field_type = self.instantiated_record_field_type(&base_source_ty, name);
                    let store_op = self.wasm_store_op_for_type(field_type.as_ref()).to_string();

                    // Store the target address first
                    self.output.push_str("    local.get $clone_tmp\n");
                    self.output.push_str(&format!(
                        "    i32.const {} ;; field offset for {}\n",
                        field_offset, name
                    ));
                    self.output.push_str("    i32.add\n");

                    // Generate the new value for this field
                    if let Some(Type::Function(param_types, return_type)) = field_type.as_ref() {
                        let abi = self.source_function_abi(param_types, return_type)?;
                        self.generate_callable_value_with_abi(value, &abi)?;
                    } else if let Some(field_type) = field_type.as_ref() {
                        self.generate_expr_with_expected_source(value, field_type)?;
                    } else {
                        self.generate_expr(value)?;
                    }

                    // Store the new value at the correct field offset
                    self.output.push_str(&format!("    {}\n", store_op));
                }
                FieldInit::Spread(expr) => {
                    self.generate_record_spread_copy(&record_name, expr, "clone_tmp")?;
                }
            }
        }

        // Return pointer to the new cloned record
        self.output.push_str("    local.get $clone_tmp\n");

        Ok(())
    }

    fn generate_freeze_expr(&mut self, expr: &Expr) -> Result<(), CodeGenError> {
        // Freeze expressions create an immutable copy at the type level. Runtime
        // records currently have no metadata header, so codegen must preserve the
        // registered field layout exactly instead of inventing a frozen flag slot.
        let source_ty = self.infer_expr_source_type(expr).ok_or_else(|| {
            CodeGenError::UnsupportedFeature(
                "freeze requires an expression with a known record type".to_string(),
            )
        })?;
        let record_name = self
            .source_record_name(&source_ty)
            .ok_or_else(|| {
                CodeGenError::UnsupportedFeature(
                    "freeze requires an expression with a known record type".to_string(),
                )
            })?
            .to_string();
        let record_size = self.instantiated_record_size(&record_name, Some(&source_ty), 0);

        // Generate the expression to get the record to freeze
        self.generate_expr(expr)?;
        self.output.push_str("    local.set $freeze_tmp\n");

        self.output.push_str(&format!(
            "    ;; Freeze {} by copying record layout\n",
            record_name
        ));

        // Allocate new record
        self.output.push_str(&format!(
            "    i32.const {} ;; frozen record size\n",
            record_size
        ));
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.set $clone_tmp\n");

        // Copy the entire record
        self.output
            .push_str("    local.get $clone_tmp ;; destination\n");
        self.output
            .push_str("    local.get $freeze_tmp ;; source\n");
        self.output
            .push_str(&format!("    i32.const {} ;; size\n", record_size));
        self.output.push_str("    memory.copy\n");

        // Return the frozen record
        self.output.push_str("    local.get $clone_tmp\n");

        Ok(())
    }

    // Generate specialized versions for generic list functions
    fn generate_new_list_specializations(&mut self, _func: &FunDecl) -> Result<(), CodeGenError> {
        // Generate new_list_Int32 specialization
        self.output
            .push_str("  (func $new_list_Int32 (result i32)\n");
        self.output
            .push_str("    ;; Allocate empty list: 8 bytes header\n");
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

        self.functions.insert(
            "new_list_Int32".to_string(),
            FunctionSig {
                _params: vec![],
                result: Some(WasmType::I32),
            },
        );

        Ok(())
    }

    fn generate_list_add_specializations(&mut self, _func: &FunDecl) -> Result<(), CodeGenError> {
        // Generate list_add_Int32 specialization
        self.output.push_str(
            "  (func $list_add_Int32 (param $list i32) (param $value i32) (result i32)\n",
        );
        self.output.push_str("    (local $length i32)\n");
        self.output.push_str("    (local $capacity i32)\n");
        self.output.push_str("    (local $new_list i32)\n");
        self.output.push_str("    \n");
        self.output
            .push_str("    ;; Load current length and capacity\n");
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
        self.output
            .push_str("    ;; Calculate new size: header + (length + 1) * 4\n");
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

        self.functions.insert(
            "list_add_Int32".to_string(),
            FunctionSig {
                _params: vec![WasmType::I32, WasmType::I32],
                result: Some(WasmType::I32),
            },
        );

        Ok(())
    }

    fn generate_imports(&mut self, imports: &[ImportDecl]) -> Result<(), CodeGenError> {
        if imports.is_empty() {
            return Ok(());
        }

        let import = &imports[0];
        Err(CodeGenError::UnsupportedFeature(format!(
            "source-level imports must be resolved before code generation; unresolved import {} remains",
            Self::format_import(import)
        )))
    }

    fn format_import(import: &ImportDecl) -> String {
        let module_name = import.module_path.join(".");

        match &import.items {
            ImportItems::All => format!("{}.*", module_name),
            ImportItems::Named(items) => format!("{}.{{{}}}", module_name, items.join(", ")),
        }
    }

    fn is_scalar_host_export_type(ty: &Type) -> bool {
        matches!(
            ty,
            Type::Named(name)
                if matches!(
                    name.as_str(),
                    "Int32" | "Int64" | "Float64" | "Boolean" | "Char" | "Unit"
                )
        )
    }

    fn ensure_scalar_host_export_type(
        export_name: &str,
        position: &str,
        ty: &Type,
    ) -> Result<(), CodeGenError> {
        if Self::is_scalar_host_export_type(ty) {
            return Ok(());
        }

        Err(CodeGenError::UnsupportedFeature(format!(
            "Exported function '{}' {} type {} requires a composite host ABI; v0.0.1 exports support only scalar Int32, Int64, Float64, Boolean, Char, and ()",
            export_name, position, ty
        )))
    }

    fn ensure_scalar_host_export_function(
        func: &FunDecl,
        source_sig: Option<&FunctionSourceSig>,
    ) -> Result<(), CodeGenError> {
        for param in &func.params {
            Self::ensure_scalar_host_export_type(
                &func.name,
                &format!("parameter '{}'", param.name),
                &param.ty,
            )?;
        }

        if let Some(return_type) = source_sig
            .and_then(|sig| sig.result.as_ref())
            .or(func.return_type.as_ref())
        {
            Self::ensure_scalar_host_export_type(&func.name, "return", return_type)?;
        }

        Ok(())
    }

    fn ensure_scalar_host_export_global(name: &str, ty: &Type) -> Result<(), CodeGenError> {
        if Self::is_scalar_host_export_type(ty) {
            return Ok(());
        }

        Err(CodeGenError::UnsupportedFeature(format!(
            "Exported top-level binding '{}' has type {} which requires a composite host ABI; v0.0.1 global exports support only scalar Int32, Int64, Float64, Boolean, Char, and ()",
            name, ty
        )))
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
                        if !func.type_params.is_empty() {
                            return Err(CodeGenError::UnsupportedFeature(format!(
                                "Exported generic function '{}' requires a concrete ABI and is not supported yet",
                                func.name
                            )));
                        }
                        Self::ensure_scalar_host_export_function(
                            func,
                            self.function_source_sigs.get(&func.name),
                        )?;

                        // Export function
                        self.output.push_str(&format!(
                            "  (export \"{}\" (func ${}))\n",
                            func.name, func.name
                        ));
                    }
                    TopDecl::Record(record) => {
                        self.output.push_str(&format!(
                            "  ;; source export record {} has no direct Wasm export\n",
                            record.name
                        ));
                    }
                    TopDecl::Binding(binding) => {
                        let export_name = match &binding.pattern {
                            Pattern::Ident(name) => name.clone(),
                            _ => {
                                return Err(CodeGenError::UnsupportedFeature(
                                    "Complex top-level binding exports are not supported yet"
                                        .to_string(),
                                ));
                            }
                        };
                        if !self.global_types.contains_key(&export_name) {
                            return Err(CodeGenError::UnsupportedFeature(format!(
                                "Exported top-level binding '{}' is not a supported constant global",
                                export_name
                            )));
                        }
                        if let Some(source_ty) = self.global_source_types.get(&export_name) {
                            Self::ensure_scalar_host_export_global(&export_name, source_ty)?;
                        }
                        self.output.push_str(&format!(
                            "  (export \"{}\" (global ${}))\n",
                            export_name, export_name
                        ));
                    }
                    _ => {
                        return Err(CodeGenError::UnsupportedFeature(
                            "Only concrete function exports, source-level record exports, and constant global exports are supported by codegen".to_string(),
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    fn generate_prototype_clone_expr(
        &mut self,
        proto_clone: &PrototypeCloneExpr,
    ) -> Result<(), CodeGenError> {
        Err(CodeGenError::UnsupportedFeature(format!(
            "prototype clone for '{}' requires real prototype identity metadata and is not supported by codegen yet",
            proto_clone.base
        )))
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
            Expr::Ident(name) => {
                if let Some(Type::Named(type_name)) = self.lookup_local_source_type(name) {
                    if self.records.contains_key(&type_name) {
                        return Some(type_name);
                    }
                }

                self.var_types.get(name).cloned()
            }
            _ => None,
        }
    }
}
