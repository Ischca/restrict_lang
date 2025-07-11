use crate::ast::*;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CodeGenError {
    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),
    
    #[error("Undefined function: {0}")]
    UndefinedFunction(String),
    
    #[error("Type not supported in WASM: {0}")]
    UnsupportedType(String),
    
    #[error("Feature not implemented: {0}")]
    NotImplemented(String),
}

/// WebAssembly Text Format (WAT) code generator
pub struct WasmCodeGen {
    // Variable to local index mapping
    locals: Vec<HashMap<String, u32>>,
    // Function signatures
    functions: HashMap<String, FunctionSig>,
    // Method signatures: record_name -> method_name -> function_sig
    methods: HashMap<String, HashMap<String, FunctionSig>>,
    // String constants pool
    strings: Vec<String>,
    // String constant offsets in memory
    string_offsets: HashMap<String, u32>,
    // Next available memory offset
    next_mem_offset: u32,
    // Current function context
    current_function: Option<String>,
    // Generated code
    output: String,
    // Type information for expressions (filled by external type checker)
    pub expr_types: HashMap<*const Expr, String>,
    // Arena management
    arena_stack: Vec<u32>,  // Stack of arena start addresses
    next_arena_addr: u32,   // Next available arena address
    default_arena: Option<u32>, // Default arena address
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
        }
    }
    
    pub fn generate(&mut self, program: &Program) -> Result<String, CodeGenError> {
        self.output.push_str("(module\n");
        
        // Import WASI functions for I/O
        self.output.push_str("  ;; WASI imports\n");
        self.output.push_str("  (import \"wasi_snapshot_preview1\" \"fd_write\" (func $fd_write (param i32 i32 i32 i32) (result i32)))\n");
        self.output.push_str("  (import \"wasi_snapshot_preview1\" \"proc_exit\" (func $proc_exit (param i32)))\n");
        
        // Memory
        self.output.push_str("\n  ;; Memory\n");
        self.output.push_str("  (memory 1)\n");
        self.output.push_str("  (export \"memory\" (memory 0))\n");
        
        // Generate string constants
        self.generate_string_constants(program)?;
        
        // Generate built-in functions
        self.generate_builtin_functions()?;
        
        // Generate arena allocator functions
        self.generate_arena_functions()?;
        
        // First pass: collect function and method signatures
        for decl in &program.declarations {
            match decl {
                TopDecl::Function(func) => {
                    self.collect_function_signature(func)?;
                }
                TopDecl::Impl(impl_block) => {
                    self.collect_impl_signatures(impl_block)?;
                }
                _ => {}
            }
        }
        
        // Second pass: generate functions
        self.output.push_str("\n  ;; Functions\n");
        for decl in &program.declarations {
            match decl {
                TopDecl::Function(func) => {
                    self.generate_function(func)?;
                }
                TopDecl::Impl(impl_block) => {
                    self.generate_impl_methods(impl_block)?;
                }
                TopDecl::Binding(_bind) => {
                    // Global bindings are not supported yet
                    return Err(CodeGenError::NotImplemented("global bindings".to_string()));
                }
                _ => {
                    // Records, contexts, etc. are compile-time only
                }
            }
        }
        
        // Export main function if it exists
        if self.functions.contains_key("main") {
            self.output.push_str("\n  ;; Export main\n");
            self.output.push_str("  (export \"_start\" (func $main))\n");
        }
        
        self.output.push_str(")\n");
        Ok(self.output.clone())
    }
    
    fn generate_string_constants(&mut self, program: &Program) -> Result<(), CodeGenError> {
        // First, collect all string literals from the program
        self.collect_string_literals(program);
        
        // Generate data section for string constants
        self.output.push_str("\n  ;; String constants\n");
        
        for (_idx, string) in self.strings.iter().enumerate() {
            let offset = self.next_mem_offset;
            self.string_offsets.insert(string.clone(), offset);
            
            // Store length at offset
            let len = string.len() as u32;
            
            // Generate data segment with length prefix
            self.output.push_str(&format!(
                "  (data (i32.const {}) \"\\{:02x}\\{:02x}\\{:02x}\\{:02x}{}\")\n",
                offset,
                (len & 0xff) as u8,
                ((len >> 8) & 0xff) as u8,
                ((len >> 16) & 0xff) as u8,
                ((len >> 24) & 0xff) as u8,
                string
            ));
            
            // Update offset: 4 bytes for length + string length
            self.next_mem_offset = offset + 4 + len;
            
            // Align to 4-byte boundary
            self.next_mem_offset = (self.next_mem_offset + 3) & !3;
        }
        
        Ok(())
    }
    
    fn generate_builtin_functions(&mut self) -> Result<(), CodeGenError> {
        self.output.push_str("\n  ;; Built-in functions\n");
        
        // println function for strings
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
        
        // Add println to function signatures
        self.functions.insert("println".to_string(), FunctionSig {
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
        
        // Allocate function using current arena
        self.output.push_str("  (func $allocate (param $size i32) (result i32)\n");
        self.output.push_str("    ;; Use current arena or fail if none\n");
        self.output.push_str("    global.get $current_arena\n");
        self.output.push_str("    local.get $size\n");
        self.output.push_str("    call $arena_alloc\n");
        self.output.push_str("  )\n");
        
        // Add arena functions to function signatures
        self.functions.insert("arena_init".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: Some(WasmType::I32),
        });
        self.functions.insert("arena_alloc".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });
        self.functions.insert("arena_reset".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: None,
        });
        self.functions.insert("allocate".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: Some(WasmType::I32),
        });
        
        // Generate list operation functions
        self.generate_list_functions()?;
        
        Ok(())
    }
    
    fn generate_list_functions(&mut self) -> Result<(), CodeGenError> {
        self.output.push_str("\n  ;; List operation functions\n");
        
        // list_length function
        self.output.push_str("  (func $list_length (param $list i32) (result i32)\n");
        self.output.push_str("    ;; Load length from list header (offset 0)\n");
        self.output.push_str("    local.get $list\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("  )\n");
        
        // list_get function (with bounds checking)
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
        
        // Add list functions to function signatures
        self.functions.insert("list_length".to_string(), FunctionSig {
            _params: vec![WasmType::I32],
            result: Some(WasmType::I32),
        });
        self.functions.insert("list_get".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });
        
        // Generate array operation functions
        self.generate_array_functions()?;
        
        Ok(())
    }
    
    fn generate_array_functions(&mut self) -> Result<(), CodeGenError> {
        self.output.push_str("\n  ;; Array operation functions\n");
        
        // array_get function (simpler than list_get - no header)
        self.output.push_str("  (func $array_get (param $array i32) (param $index i32) (result i32)\n");
        self.output.push_str("    ;; Calculate element address: array + (index * 4)\n");
        self.output.push_str("    local.get $array\n");
        self.output.push_str("    local.get $index\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.mul\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str("    i32.load\n");
        self.output.push_str("  )\n");
        
        // array_set function (for mutable arrays)
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
        
        // Add array functions to function signatures
        self.functions.insert("array_get".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
        });
        self.functions.insert("array_set".to_string(), FunctionSig {
            _params: vec![WasmType::I32, WasmType::I32, WasmType::I32],
            result: None,
        });
        
        Ok(())
    }
    
    fn collect_string_literals(&mut self, program: &Program) {
        // Walk through the entire AST to find string literals
        for decl in &program.declarations {
            match decl {
                TopDecl::Function(func) => {
                    self.collect_strings_from_block(&func.body);
                }
                TopDecl::Impl(impl_block) => {
                    for func in &impl_block.functions {
                        self.collect_strings_from_block(&func.body);
                    }
                }
                _ => {}
            }
        }
    }
    
    fn collect_strings_from_block(&mut self, block: &BlockExpr) {
        for stmt in &block.statements {
            match stmt {
                Stmt::Binding(bind) => {
                    self.collect_strings_from_expr(&bind.value);
                }
                Stmt::Assignment(assign) => {
                    self.collect_strings_from_expr(&assign.value);
                }
                Stmt::Expr(expr) => {
                    self.collect_strings_from_expr(expr);
                }
            }
        }
        if let Some(expr) = &block.expr {
            self.collect_strings_from_expr(expr);
        }
    }
    
    fn collect_strings_from_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::StringLit(s) => {
                if !self.strings.contains(s) {
                    self.strings.push(s.clone());
                }
            }
            Expr::Binary(binary) => {
                self.collect_strings_from_expr(&binary.left);
                self.collect_strings_from_expr(&binary.right);
            }
            Expr::Call(call) => {
                self.collect_strings_from_expr(&call.function);
                for arg in &call.args {
                    self.collect_strings_from_expr(arg);
                }
            }
            Expr::Block(block) => {
                self.collect_strings_from_block(block);
            }
            Expr::RecordLit(record) => {
                for field in &record.fields {
                    self.collect_strings_from_expr(&field.value);
                }
            }
            Expr::Pipe(pipe) => {
                self.collect_strings_from_expr(&pipe.expr);
                match &pipe.target {
                    PipeTarget::Expr(expr) => self.collect_strings_from_expr(expr),
                    PipeTarget::Ident(_) => {} // Identifiers don't contain strings
                }
            }
            Expr::Then(then) => {
                self.collect_strings_from_expr(&then.condition);
                self.collect_strings_from_block(&then.then_block);
                for (condition, block) in &then.else_ifs {
                    self.collect_strings_from_expr(condition);
                    self.collect_strings_from_block(block);
                }
                if let Some(else_block) = &then.else_block {
                    self.collect_strings_from_block(else_block);
                }
            }
            Expr::While(while_expr) => {
                self.collect_strings_from_expr(&while_expr.condition);
                self.collect_strings_from_block(&while_expr.body);
            }
            _ => {}
        }
    }
    
    fn collect_function_signature(&mut self, func: &FunDecl) -> Result<(), CodeGenError> {
        let params: Vec<WasmType> = func.params.iter()
            .map(|p| self.convert_type(&p.ty))
            .collect::<Result<Vec<_>, _>>()?;
        
        // For now, assume all functions return i32
        let result = Some(WasmType::I32);
        
        self.functions.insert(func.name.clone(), FunctionSig { _params: params, result });
        Ok(())
    }
    
    fn convert_type(&self, ty: &Type) -> Result<WasmType, CodeGenError> {
        match ty {
            Type::Named(name) => match name.as_str() {
                "Int" | "Int32" => Ok(WasmType::I32),
                "Float" | "Float64" => Ok(WasmType::F64),
                "Boolean" | "Bool" => Ok(WasmType::I32), // 0 = false, 1 = true
                _ => {
                    // Records are passed as i32 references for now
                    Ok(WasmType::I32)
                }
            },
            Type::Generic(_, _) => Err(CodeGenError::UnsupportedType("generic types".to_string())),
        }
    }
    
    fn collect_impl_signatures(&mut self, impl_block: &ImplBlock) -> Result<(), CodeGenError> {
        let record_name = impl_block.target.clone();
        
        let mut method_sigs = Vec::new();
        let mut function_sigs = Vec::new();
        
        for func in &impl_block.functions {
            let params: Vec<WasmType> = func.params.iter()
                .map(|p| self.convert_type(&p.ty))
                .collect::<Result<Vec<_>, _>>()?;
            
            // For now, assume all methods return i32
            let result = Some(WasmType::I32);
            
            // Generate a mangled name for the method
            let mangled_name = format!("{}_{}", record_name, func.name);
            
            // Collect signatures to add later
            method_sigs.push((func.name.clone(), FunctionSig { _params: params.clone(), result }));
            function_sigs.push((mangled_name, FunctionSig { _params: params, result }));
        }
        
        // Now add them to the maps
        let method_map = self.methods.entry(record_name).or_insert_with(HashMap::new);
        for (name, sig) in method_sigs {
            method_map.insert(name, sig);
        }
        for (name, sig) in function_sigs {
            self.functions.insert(name, sig);
        }
        
        Ok(())
    }
    
    fn generate_impl_methods(&mut self, impl_block: &ImplBlock) -> Result<(), CodeGenError> {
        let record_name = impl_block.target.clone();
        
        for func in &impl_block.functions {
            // Generate the method with a mangled name
            let mangled_func = FunDecl {
                name: format!("{}_{}", record_name, func.name),
                params: func.params.clone(),
                body: func.body.clone(),
            };
            self.generate_function(&mangled_func)?;
        }
        Ok(())
    }
    
    fn generate_function(&mut self, func: &FunDecl) -> Result<(), CodeGenError> {
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
        for (name, ty) in locals {
            self.output.push_str(&format!("    (local ${} {})\n", name, self.wasm_type_str(ty)));
            self.add_local(&name, next_idx);
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
        
        // Hack: Add common pattern variable names
        // TODO: Collect these properly from the AST
        let common_names = vec!["n", "x", "y", "z", "a", "b", "c", "head", "tail", "rest"];
        for name in common_names {
            if !self.locals.last().unwrap().contains_key(name) {
                self.output.push_str(&format!("    (local ${} i32)\n", name));
                self.add_local(name, next_idx);
                next_idx += 1;
            }
        }
        
        // Initialize default arena for main function
        if func.name == "main" && self.default_arena.is_none() {
            let arena_addr = self.next_arena_addr;
            self.default_arena = Some(arena_addr);
            self.next_arena_addr += 0x10000; // 64KB for default arena
            
            self.output.push_str(&format!("    ;; Initialize default arena\n"));
            self.output.push_str(&format!("    i32.const {}\n", arena_addr));
            self.output.push_str("    call $arena_init\n");
            self.output.push_str("    global.set $current_arena\n\n");
        }
        
        // Generate function body
        self.generate_block(&func.body)?;
        
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
        // Generate statements
        for stmt in &block.statements {
            match stmt {
                Stmt::Binding(bind) => self.generate_binding(bind)?,
                Stmt::Assignment(assign) => self.generate_assignment(assign)?,
                Stmt::Expr(expr) => {
                    self.generate_expr(expr)?;
                    // Pop the result if it's not the last expression
                    if block.expr.is_some() || stmt != block.statements.last().unwrap() {
                        self.output.push_str("    drop\n");
                    }
                }
            }
        }
        
        // Generate return expression
        if let Some(expr) = &block.expr {
            self.generate_expr(expr)?;
        } else if block.statements.is_empty() {
            // Empty block returns 0 (Unit)
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
                if let Some(_idx) = self.lookup_local(name) {
                    self.output.push_str(&format!("    local.get ${}\n", name));
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
            Expr::FieldAccess(obj_expr, field_name) => {
                // For now, we'll simulate field access with a simple offset
                // In a real implementation, we'd need proper memory layout
                self.generate_expr(obj_expr)?;
                
                // Assume records are stored as pointers, and fields are at fixed offsets
                // For simplicity, assume each field is 4 bytes (i32)
                match field_name.as_str() {
                    "hp" | "x" | "value" => self.output.push_str("    i32.const 0\n    i32.add\n    i32.load\n"), // offset 0
                    "atk" | "y" | "mp" => self.output.push_str("    i32.const 4\n    i32.add\n    i32.load\n"), // offset 4
                    _ => return Err(CodeGenError::NotImplemented(format!("field access: {}", field_name))),
                }
            }
            Expr::Clone(_) => {
                return Err(CodeGenError::NotImplemented("clone expressions".to_string()));
            }
            Expr::Freeze(_) => {
                return Err(CodeGenError::NotImplemented("freeze expressions".to_string()));
            }
            Expr::Pipe(pipe) => {
                self.generate_pipe_expr(pipe)?;
            }
            Expr::With(with_expr) => {
                self.generate_with_expr(with_expr)?;
            }
            Expr::Then(then) => {
                self.generate_then_expr(then)?;
            }
            Expr::While(while_expr) => {
                self.generate_while_expr(while_expr)?;
            }
            Expr::Match(match_expr) => {
                self.generate_match_expr(match_expr)?;
            }
            Expr::StringLit(s) => {
                // Generate pointer to string constant
                if let Some(&offset) = self.string_offsets.get(s) {
                    // Push string pointer (offset in memory)
                    self.output.push_str(&format!("    i32.const {}\n", offset));
                } else {
                    return Err(CodeGenError::NotImplemented("string literal not found in constant pool".to_string()));
                }
            }
            Expr::CharLit(_) => {
                return Err(CodeGenError::NotImplemented("char literals".to_string()));
            }
            Expr::ListLit(elements) => {
                self.generate_list_literal(elements)?;
            }
            Expr::ArrayLit(elements) => {
                self.generate_array_literal(elements)?;
            }
            Expr::Some(expr) => {
                // Option<T> is represented as a tagged union:
                // - First i32: discriminant (0 = None, 1 = Some)
                // - Second i32: value (only present if Some)
                // For now, we'll just push the values on the stack
                self.output.push_str("    i32.const 1\n"); // Some tag
                self.generate_expr(expr)?; // The value
            }
            Expr::None => {
                // None is represented as discriminant 0
                self.output.push_str("    i32.const 0\n"); // None tag
                self.output.push_str("    i32.const 0\n"); // Dummy value
            }
        }
        Ok(())
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
            BinaryOp::Eq => self.output.push_str("    i32.eq\n"),
            BinaryOp::Ne => self.output.push_str("    i32.ne\n"),
            BinaryOp::Lt => self.output.push_str("    i32.lt_s\n"),
            BinaryOp::Le => self.output.push_str("    i32.le_s\n"),
            BinaryOp::Gt => self.output.push_str("    i32.gt_s\n"),
            BinaryOp::Ge => self.output.push_str("    i32.ge_s\n"),
        }
        Ok(())
    }
    
    fn generate_call_expr(&mut self, call: &CallExpr) -> Result<(), CodeGenError> {
        // Generate arguments
        for arg in &call.args {
            self.generate_expr(arg)?;
        }
        
        // Generate function call
        if let Expr::Ident(func_name) = &*call.function {
            // First check if it's a regular function
            if self.functions.contains_key(func_name) {
                self.output.push_str(&format!("    call ${}\n", func_name));
            } else {
                // Try to find it as a method
                // For methods, we need to determine the record type from the first argument
                if let Some(first_arg) = call.args.first() {
                    // Try to get type information from the expr_types map
                    let record_type = if let Some(ty) = self.expr_types.get(&(&**first_arg as *const Expr)) {
                        // Extract record name from type string (e.g., "Enemy" from "Enemy")
                        Some(ty.clone())
                    } else {
                        None
                    };
                    
                    if let Some(record_name) = record_type {
                        // We have type information - use it for precise method resolution
                        if let Some(method_map) = self.methods.get(&record_name) {
                            if method_map.contains_key(func_name) {
                                let mangled_name = format!("{}_{}", record_name, func_name);
                                self.output.push_str(&format!("    call ${}\n", mangled_name));
                            } else {
                                return Err(CodeGenError::UndefinedFunction(
                                    format!("Method '{}' not found in record '{}'", func_name, record_name)
                                ));
                            }
                        } else {
                            return Err(CodeGenError::UndefinedFunction(
                                format!("No methods defined for record '{}'", record_name)
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
            return Err(CodeGenError::NotImplemented("indirect calls".to_string()));
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
            Expr::Then(then_expr) => {
                self.collect_locals_from_block(&then_expr.then_block, locals)?;
                for (_, else_if_block) in &then_expr.else_ifs {
                    self.collect_locals_from_block(else_if_block, locals)?;
                }
                if let Some(else_block) = &then_expr.else_block {
                    self.collect_locals_from_block(else_block, locals)?;
                }
            }
            Expr::While(while_expr) => {
                self.collect_locals_from_block(&while_expr.body, locals)?;
            }
            Expr::With(with_expr) => {
                self.collect_locals_from_block(&with_expr.body, locals)?;
            }
            _ => {}
        }
        Ok(())
    }
    
    fn collect_locals_from_pattern(&self, pattern: &Pattern, locals: &mut Vec<(String, WasmType)>) -> Result<(), CodeGenError> {
        match pattern {
            Pattern::Ident(name) => {
                if name != "_" {
                    locals.push((name.clone(), WasmType::I32)); // TODO: proper type inference
                }
            }
            Pattern::ListCons(head, tail) => {
                self.collect_locals_from_pattern(head, locals)?;
                self.collect_locals_from_pattern(tail, locals)?;
            }
            Pattern::ListExact(patterns) => {
                for pattern in patterns {
                    self.collect_locals_from_pattern(pattern, locals)?;
                }
            }
            Pattern::Some(inner) => {
                self.collect_locals_from_pattern(inner, locals)?;
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
                if self.functions.contains_key(name) {
                    // It's a function call: expr |> func
                    self.output.push_str(&format!("    call ${}\n", name));
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
                        } else {
                            return Err(CodeGenError::UndefinedFunction(func_name.clone()));
                        }
                    }
                    _ => {
                        return Err(CodeGenError::NotImplemented("complex pipe targets".to_string()));
                    }
                }
            }
        }
        
        Ok(())
    }
    
    fn generate_then_expr(&mut self, then: &ThenExpr) -> Result<(), CodeGenError> {
        // Generate condition
        self.generate_expr(&then.condition)?;
        
        // WASM if/then/else
        self.output.push_str("    i32.const 0\n");
        self.output.push_str("    i32.ne\n"); // Convert to boolean (0 or 1)
        
        // Determine result type
        let result_type = self.infer_block_type(&then.then_block)?;
        let type_str = self.wasm_type_str(result_type);
        
        self.output.push_str(&format!("    (if (result {})\n", type_str));
        self.output.push_str("      (then\n");
        
        // Generate then branch
        self.push_scope();
        let _then_result = self.generate_block_contents(&then.then_block)?;
        self.pop_scope();
        
        self.output.push_str("      )\n");
        
        // Generate else branches and final else
        for else_if in &then.else_ifs {
            self.output.push_str("      (else\n");
            
            // Generate else-if condition
            self.generate_expr(&else_if.0)?;
            self.output.push_str("        i32.const 0\n");
            self.output.push_str("        i32.ne\n");
            
            self.output.push_str(&format!("        (if (result {})\n", type_str));
            self.output.push_str("          (then\n");
            
            self.push_scope();
            let _else_if_result = self.generate_block_contents(&else_if.1)?;
            self.pop_scope();
            
            self.output.push_str("          )\n");
        }
        
        // Final else
        if let Some(else_block) = &then.else_block {
            self.output.push_str("          (else\n");
            
            self.push_scope();
            let _else_result = self.generate_block_contents(else_block)?;
            self.pop_scope();
            
            self.output.push_str("          )\n");
        } else {
            // No else branch - return default value
            self.output.push_str("          (else\n");
            self.output.push_str(&format!("            {} {}\n", 
                if result_type == WasmType::F64 { "f64.const" } else { "i32.const" },
                "0"));
            self.output.push_str("          )\n");
        }
        
        // Close all the nested ifs
        for _ in &then.else_ifs {
            self.output.push_str("        )\n");
            self.output.push_str("      )\n");
        }
        
        self.output.push_str("    )\n");
        
        Ok(())
    }
    
    fn generate_while_expr(&mut self, while_expr: &WhileExpr) -> Result<(), CodeGenError> {
        // WASM loop structure:
        // (loop $label
        //   condition
        //   (if
        //     (then
        //       body
        //       (br $label)  ; continue loop
        //     )
        //   )
        // )
        
        self.output.push_str("    (loop $while_loop\n");
        
        // Generate condition
        self.generate_expr(&while_expr.condition)?;
        
        // If condition is true, execute body and continue loop
        self.output.push_str("      (if\n");
        self.output.push_str("        (then\n");
        
        // Generate body
        self.push_scope();
        self.generate_block(&while_expr.body)?;
        self.output.push_str("          drop\n"); // Drop the body result
        self.pop_scope();
        
        // Continue loop
        self.output.push_str("          (br $while_loop)\n");
        self.output.push_str("        )\n");
        self.output.push_str("      )\n");
        self.output.push_str("    )\n");
        
        // While loops return unit (i32.const 0)
        self.output.push_str("    i32.const 0\n");
        
        Ok(())
    }
    
    fn generate_with_expr(&mut self, with_expr: &WithExpr) -> Result<(), CodeGenError> {
        // For now, only support Arena context
        if with_expr.contexts.len() != 1 || with_expr.contexts[0] != "Arena" {
            return Err(CodeGenError::NotImplemented("Only 'with Arena' is currently supported".to_string()));
        }
        
        // Initialize a new arena
        let arena_start = self.next_arena_addr;
        self.arena_stack.push(arena_start);
        self.next_arena_addr += 0x8000; // 32KB per arena
        
        // Call arena_init
        self.output.push_str(&format!("    i32.const {}\n", arena_start));
        self.output.push_str("    call $arena_init\n");
        self.output.push_str("    drop\n"); // We don't need the return value for now
        
        // Generate the body
        self.push_scope();
        let result = self.generate_block(&with_expr.body)?;
        self.pop_scope();
        
        // Reset the arena
        self.output.push_str(&format!("    i32.const {}\n", arena_start));
        self.output.push_str("    call $arena_reset\n");
        
        // Pop arena from stack
        self.arena_stack.pop();
        
        // If block returns a value, it should still be on the stack
        Ok(())
    }
    
    fn generate_match_expr(&mut self, match_expr: &MatchExpr) -> Result<(), CodeGenError> {
        // We need to evaluate the scrutinee once and store it
        // For now, we'll use a simple if-else chain approach
        
        // Generate the scrutinee expression
        self.generate_expr(&match_expr.expr)?;
        
        // For now, we'll keep the value on the stack and duplicate as needed
        // In a real implementation, we'd allocate a temporary local
        
        // Simplified match generation using cascading if-else
        let _has_else = false;
        
        for (i, arm) in match_expr.arms.iter().enumerate() {
            if i > 0 {
                // Duplicate scrutinee for next comparison
                self.output.push_str("    local.get $match_tmp\n");
            } else {
                // First arm - save scrutinee to temporary
                self.output.push_str("    local.tee $match_tmp\n");
            }
            
            // Generate pattern test if needed
            match &arm.pattern {
                Pattern::Wildcard => {
                    // Wildcard always matches - generate the body directly
                    self.output.push_str("    drop\n"); // Drop the scrutinee
                    
                    if i > 0 {
                        self.output.push_str("      )\n      (else\n");
                    }
                    
                    self.push_scope();
                    self.generate_block(&arm.body)?;
                    self.pop_scope();
                    
                    // This is the last arm we'll generate
                    break;
                }
                Pattern::Ident(name) => {
                    // Identifier pattern - bind and generate body
                    if i > 0 {
                        self.output.push_str("      )\n      (else\n");
                    }
                    
                    self.push_scope();
                    
                    // Pattern variable should already be declared as a local
                    
                    // Store the value in the local variable
                    self.output.push_str(&format!("        local.get $match_tmp\n"));
                    self.output.push_str(&format!("        local.set ${}\n", name));
                    
                    self.generate_block(&arm.body)?;
                    self.pop_scope();
                    
                    // This is the last arm we'll generate
                    break;
                }
                Pattern::Literal(lit) => {
                    // Generate comparison
                    match lit {
                        Literal::Int(n) => {
                            self.output.push_str(&format!("    i32.const {}\n", n));
                            self.output.push_str("    i32.eq\n");
                        }
                        Literal::Bool(b) => {
                            self.output.push_str(&format!("    i32.const {}\n", if *b { 1 } else { 0 }));
                            self.output.push_str("    i32.eq\n");
                        }
                        _ => return Err(CodeGenError::NotImplemented(
                            format!("pattern matching for literal {:?}", lit)
                        )),
                    }
                    
                    // Generate if-then-else
                    self.output.push_str("    (if (result i32)\n");
                    self.output.push_str("      (then\n");
                    
                    self.push_scope();
                    self.generate_block(&arm.body)?;
                    self.pop_scope();
                    
                    self.output.push_str("      )\n");
                    
                    if i == match_expr.arms.len() - 1 {
                        // Last arm - no else
                        self.output.push_str("      (else\n");
                        self.output.push_str("        unreachable\n");
                        self.output.push_str("      )\n");
                        self.output.push_str("    )\n");
                    }
                }
                Pattern::Record(_, _) => {
                    return Err(CodeGenError::NotImplemented("record pattern matching".to_string()));
                }
                Pattern::Some(_) => {
                    // Check if discriminant is 1 (Some)
                    // Assuming Option is represented as two values on stack: discriminant, value
                    // We need to duplicate both and check discriminant
                    self.output.push_str("    drop\n"); // Drop the value part for now
                    self.output.push_str("    local.get $match_tmp\n"); // Get discriminant
                    self.output.push_str("    i32.const 1\n");
                    self.output.push_str("    i32.eq\n");
                    
                    // Generate if-then-else
                    self.output.push_str("    (if (result i32)\n");
                    self.output.push_str("      (then\n");
                    
                    self.push_scope();
                    // TODO: Bind inner pattern variable
                    self.generate_block(&arm.body)?;
                    self.pop_scope();
                    
                    self.output.push_str("      )\n");
                    
                    if i == match_expr.arms.len() - 1 {
                        self.output.push_str("      (else\n");
                        self.output.push_str("        unreachable\n");
                        self.output.push_str("      )\n");
                        self.output.push_str("    )\n");
                    }
                }
                Pattern::None => {
                    // Check if discriminant is 0 (None)
                    self.output.push_str("    drop\n"); // Drop the value part
                    self.output.push_str("    local.get $match_tmp\n"); // Get discriminant
                    self.output.push_str("    i32.const 0\n");
                    self.output.push_str("    i32.eq\n");
                    
                    // Generate if-then-else
                    self.output.push_str("    (if (result i32)\n");
                    self.output.push_str("      (then\n");
                    
                    self.push_scope();
                    self.generate_block(&arm.body)?;
                    self.pop_scope();
                    
                    self.output.push_str("      )\n");
                    
                    if i == match_expr.arms.len() - 1 {
                        self.output.push_str("      (else\n");
                        self.output.push_str("        unreachable\n");
                        self.output.push_str("      )\n");
                        self.output.push_str("    )\n");
                    }
                }
                Pattern::EmptyList => {
                    // Check if list length is 0
                    self.output.push_str("    i32.load\n"); // Load length from list header
                    self.output.push_str("    i32.const 0\n");
                    self.output.push_str("    i32.eq\n");
                    
                    // Generate if-then-else
                    self.output.push_str("    (if (result i32)\n");
                    self.output.push_str("      (then\n");
                    
                    self.push_scope();
                    self.generate_block(&arm.body)?;
                    self.pop_scope();
                    
                    self.output.push_str("      )\n");
                    
                    if i == match_expr.arms.len() - 1 {
                        self.output.push_str("      (else\n");
                        self.output.push_str("        unreachable\n");
                        self.output.push_str("      )\n");
                        self.output.push_str("    )\n");
                    }
                }
                Pattern::ListExact(patterns) => {
                    // Check if list length matches exactly
                    self.output.push_str("    i32.load\n"); // Load length from list header
                    self.output.push_str(&format!("    i32.const {}\n", patterns.len()));
                    self.output.push_str("    i32.eq\n");
                    
                    // Generate if-then-else
                    self.output.push_str("    (if (result i32)\n");
                    self.output.push_str("      (then\n");
                    
                    self.push_scope();
                    
                    // Bind each element to pattern variables
                    for (idx, pattern) in patterns.iter().enumerate() {
                        if let Pattern::Ident(name) = &**pattern {
                            // Load element from list
                            self.output.push_str("        local.get $match_tmp\n");
                            self.output.push_str(&format!("        i32.const {}\n", 8 + idx * 4)); // Skip header
                            self.output.push_str("        i32.add\n");
                            self.output.push_str("        i32.load\n");
                            self.output.push_str(&format!("        local.set ${}\n", name));
                        }
                    }
                    
                    self.generate_block(&arm.body)?;
                    self.pop_scope();
                    
                    self.output.push_str("      )\n");
                    
                    if i == match_expr.arms.len() - 1 {
                        self.output.push_str("      (else\n");
                        self.output.push_str("        unreachable\n");
                        self.output.push_str("      )\n");
                        self.output.push_str("    )\n");
                    }
                }
                Pattern::ListCons(head_pattern, tail_pattern) => {
                    // Check if list is non-empty
                    self.output.push_str("    i32.load\n"); // Load length from list header
                    self.output.push_str("    i32.const 0\n");
                    self.output.push_str("    i32.gt_s\n"); // length > 0
                    
                    // Generate if-then-else
                    self.output.push_str("    (if (result i32)\n");
                    self.output.push_str("      (then\n");
                    
                    self.push_scope();
                    
                    // Bind head
                    if let Pattern::Ident(head_name) = &**head_pattern {
                        self.output.push_str("        local.get $match_tmp\n");
                        self.output.push_str("        i32.const 8\n"); // Skip header to first element
                        self.output.push_str("        i32.add\n");
                        self.output.push_str("        i32.load\n");
                        self.output.push_str(&format!("        local.set ${}\n", head_name));
                    }
                    
                    // Create tail list (for now, we'll create a new list)
                    if let Pattern::Ident(tail_name) = &**tail_pattern {
                        // Calculate tail length
                        self.output.push_str("        local.get $match_tmp\n");
                        self.output.push_str("        i32.load\n"); // Get original length
                        self.output.push_str("        i32.const 1\n");
                        self.output.push_str("        i32.sub\n"); // tail_length = length - 1
                        self.output.push_str("        local.tee $tail_len\n");
                        
                        // Allocate new list for tail
                        self.output.push_str("        i32.const 4\n");
                        self.output.push_str("        i32.mul\n"); // tail_len * 4
                        self.output.push_str("        i32.const 8\n");
                        self.output.push_str("        i32.add\n"); // + header size
                        self.output.push_str("        call $allocate\n");
                        self.output.push_str("        local.tee $tail_tmp\n");
                        
                        // Write tail length
                        self.output.push_str("        local.get $tail_tmp\n");
                        self.output.push_str("        local.get $tail_len\n");
                        self.output.push_str("        i32.store\n");
                        
                        // Write tail capacity
                        self.output.push_str("        local.get $tail_tmp\n");
                        self.output.push_str("        i32.const 4\n");
                        self.output.push_str("        i32.add\n");
                        self.output.push_str("        local.get $tail_len\n");
                        self.output.push_str("        i32.store\n");
                        
                        // Copy tail elements
                        self.output.push_str("        local.get $tail_tmp\n");
                        self.output.push_str("        i32.const 8\n");
                        self.output.push_str("        i32.add\n"); // Destination: tail + 8
                        self.output.push_str("        local.get $match_tmp\n");
                        self.output.push_str("        i32.const 12\n");
                        self.output.push_str("        i32.add\n"); // Source: original + 12 (skip header and first element)
                        self.output.push_str("        local.get $tail_len\n");
                        self.output.push_str("        i32.const 4\n");
                        self.output.push_str("        i32.mul\n"); // Size in bytes
                        self.output.push_str("        memory.copy\n");
                        
                        // Store tail in variable
                        self.output.push_str("        local.get $tail_tmp\n");
                        self.output.push_str(&format!("        local.set ${}\n", tail_name));
                    }
                    
                    self.generate_block(&arm.body)?;
                    self.pop_scope();
                    
                    self.output.push_str("      )\n");
                    
                    if i == match_expr.arms.len() - 1 {
                        self.output.push_str("      (else\n");
                        self.output.push_str("        unreachable\n");
                        self.output.push_str("      )\n");
                        self.output.push_str("    )\n");
                    }
                }
            }
        }
        
        // Close any remaining if blocks
        for _ in 1..match_expr.arms.len() {
            if match_expr.arms.last().map(|a| !matches!(&a.pattern, Pattern::Wildcard | Pattern::Ident(_))).unwrap_or(false) {
                self.output.push_str("    )\n");
            }
        }
        
        Ok(())
    }
    
    fn generate_list_literal(&mut self, elements: &[Box<Expr>]) -> Result<(), CodeGenError> {
        // Calculate the size needed: header (8 bytes) + elements (4 bytes each)
        let element_count = elements.len() as i32;
        let header_size = 8; // length (4) + capacity (4)
        let element_size = 4; // i32 for now
        let total_size = header_size + (element_count * element_size);
        
        self.output.push_str(&format!("    ;; List literal with {} elements\n", element_count));
        
        // Allocate memory for the list
        self.output.push_str(&format!("    i32.const {}\n", total_size));
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.tee $list_tmp\n"); // Save list pointer
        
        // Write length field
        self.output.push_str("    local.get $list_tmp\n");
        self.output.push_str(&format!("    i32.const {}\n", element_count));
        self.output.push_str("    i32.store\n");
        
        // Write capacity field
        self.output.push_str("    local.get $list_tmp\n");
        self.output.push_str("    i32.const 4\n");
        self.output.push_str("    i32.add\n");
        self.output.push_str(&format!("    i32.const {}\n", element_count));
        self.output.push_str("    i32.store\n");
        
        // Write elements
        for (i, element) in elements.iter().enumerate() {
            self.output.push_str(&format!("    ;; Element {}\n", i));
            self.output.push_str("    local.get $list_tmp\n");
            self.output.push_str(&format!("    i32.const {}\n", header_size + (i as i32 * element_size)));
            self.output.push_str("    i32.add\n");
            
            // Generate the element value
            self.generate_expr(element)?;
            
            self.output.push_str("    i32.store\n");
        }
        
        // Leave list pointer on stack
        self.output.push_str("    local.get $list_tmp\n");
        
        Ok(())
    }
    
    fn generate_array_literal(&mut self, elements: &[Box<Expr>]) -> Result<(), CodeGenError> {
        // Arrays don't have a header - just elements
        let element_count = elements.len() as i32;
        let element_size = 4; // i32 for now
        let total_size = element_count * element_size;
        
        self.output.push_str(&format!("    ;; Array literal with {} elements\n", element_count));
        
        if element_count == 0 {
            // Empty array - return null pointer
            self.output.push_str("    i32.const 0\n");
            return Ok(());
        }
        
        // Allocate memory for the array
        self.output.push_str(&format!("    i32.const {}\n", total_size));
        self.output.push_str("    call $allocate\n");
        self.output.push_str("    local.tee $list_tmp\n"); // Reuse list_tmp for arrays
        
        // Write elements directly (no header)
        for (i, element) in elements.iter().enumerate() {
            self.output.push_str(&format!("    ;; Element {}\n", i));
            self.output.push_str("    local.get $list_tmp\n");
            self.output.push_str(&format!("    i32.const {}\n", i as i32 * element_size));
            self.output.push_str("    i32.add\n");
            
            // Generate the element value
            self.generate_expr(element)?;
            
            self.output.push_str("    i32.store\n");
        }
        
        // Leave array pointer on stack
        self.output.push_str("    local.get $list_tmp\n");
        
        Ok(())
    }
    
    
    fn generate_block_contents(&mut self, block: &BlockExpr) -> Result<WasmType, CodeGenError> {
        // Generate statements
        for (i, stmt) in block.statements.iter().enumerate() {
            match stmt {
                Stmt::Binding(bind) => self.generate_binding(bind)?,
                Stmt::Assignment(assign) => self.generate_assignment(assign)?,
                Stmt::Expr(expr) => {
                    self.generate_expr(expr)?;
                    // Pop the result if it's not the last expression
                    let is_last = i == block.statements.len() - 1 && block.expr.is_none();
                    if !is_last {
                        self.output.push_str("        drop\n");
                    }
                }
            }
        }
        
        // Generate return expression
        if let Some(expr) = &block.expr {
            self.generate_expr(expr)?;
            self.infer_expr_type(expr)
        } else if let Some(Stmt::Expr(last_expr)) = block.statements.last() {
            // Last statement was an expression, its type is the block type
            self.infer_expr_type(last_expr)
        } else {
            // Empty block or ends with binding
            self.output.push_str("        i32.const 0\n");
            Ok(WasmType::I32)
        }
    }
    
    fn infer_block_type(&self, block: &BlockExpr) -> Result<WasmType, CodeGenError> {
        if let Some(expr) = &block.expr {
            self.infer_expr_type(expr)
        } else if let Some(Stmt::Expr(last_expr)) = block.statements.last() {
            self.infer_expr_type(last_expr)
        } else {
            Ok(WasmType::I32) // Default to i32 (Unit)
        }
    }
}

// Standalone generate function for public API
pub fn generate(program: &Program) -> Result<String, CodeGenError> {
    let mut codegen = WasmCodeGen::new();
    codegen.generate(program)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_program;
    
    fn generate_wat(input: &str) -> Result<String, String> {
        let (_, ast) = parse_program(input)
            .map_err(|e| format!("Parse error: {:?}", e))?;
        let mut codegen = WasmCodeGen::new();
        codegen.generate(&ast)
            .map_err(|e| format!("Codegen error: {}", e))
    }
    
    #[test]
    fn test_simple_arithmetic() {
        let input = r#"
            fun add = a: Int b: Int {
                a + b
            }
        "#;
        let wat = generate_wat(input).unwrap();
        assert!(wat.contains("i32.add"));
    }
    
    #[test]
    fn test_main_function() {
        let input = r#"
            fun main = {
                42
            }
        "#;
        let wat = generate_wat(input).unwrap();
        assert!(wat.contains("(export \"_start\" (func $main))"));
        assert!(wat.contains("i32.const 42"));
    }
}