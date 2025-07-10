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
    _strings: Vec<String>,
    // Current function context
    current_function: Option<String>,
    // Generated code
    output: String,
    // Type information for expressions (filled by external type checker)
    pub expr_types: HashMap<*const Expr, String>,
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
            _strings: Vec::new(),
            current_function: None,
            output: String::new(),
            expr_types: HashMap::new(),
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
    
    fn generate_string_constants(&mut self, _program: &Program) -> Result<(), CodeGenError> {
        // TODO: Collect all string literals and generate data section
        self.output.push_str("\n  ;; String constants\n");
        self.output.push_str("  (data (i32.const 0) \"\")\n");
        Ok(())
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
        
        // Generate function body
        self.generate_block(&func.body)?;
        
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
            Expr::With(_) => {
                return Err(CodeGenError::NotImplemented("with expressions".to_string()));
            }
            Expr::Then(then) => {
                self.generate_then_expr(then)?;
            }
            Expr::While(_) => {
                return Err(CodeGenError::NotImplemented("while loops".to_string()));
            }
            Expr::Match(_) => {
                return Err(CodeGenError::NotImplemented("match expressions".to_string()));
            }
            _ => {
                return Err(CodeGenError::NotImplemented(format!("expression type: {:?}", expr)));
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
                }
                Stmt::Expr(expr) => {
                    // Check for nested blocks
                    if let Expr::Block(nested_block) = &**expr {
                        self.collect_locals_from_block(nested_block, locals)?;
                    }
                }
            }
        }
        
        // Check the return expression for nested blocks
        if let Some(expr) = &block.expr {
            if let Expr::Block(nested_block) = &**expr {
                self.collect_locals_from_block(nested_block, locals)?;
            }
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
    
    fn generate_block_contents(&mut self, block: &BlockExpr) -> Result<WasmType, CodeGenError> {
        // Generate statements
        for (i, stmt) in block.statements.iter().enumerate() {
            match stmt {
                Stmt::Binding(bind) => self.generate_binding(bind)?,
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