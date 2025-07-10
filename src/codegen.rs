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
    // String constants pool
    strings: Vec<String>,
    // Current function context
    current_function: Option<String>,
    // Generated code
    output: String,
}

#[derive(Debug, Clone)]
struct FunctionSig {
    params: Vec<WasmType>,
    result: Option<WasmType>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
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
            strings: Vec::new(),
            current_function: None,
            output: String::new(),
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
        
        // First pass: collect function signatures
        for decl in &program.declarations {
            if let TopDecl::Function(func) = decl {
                self.collect_function_signature(func)?;
            }
        }
        
        // Second pass: generate functions
        self.output.push_str("\n  ;; Functions\n");
        for decl in &program.declarations {
            match decl {
                TopDecl::Function(func) => {
                    self.generate_function(func)?;
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
        
        self.functions.insert(func.name.clone(), FunctionSig { params, result });
        Ok(())
    }
    
    fn convert_type(&self, ty: &Type) -> Result<WasmType, CodeGenError> {
        match ty {
            Type::Named(name) => match name.as_str() {
                "Int" | "Int32" => Ok(WasmType::I32),
                "Float" | "Float64" => Ok(WasmType::F64),
                "Boolean" | "Bool" => Ok(WasmType::I32), // 0 = false, 1 = true
                _ => Err(CodeGenError::UnsupportedType(name.clone())),
            },
            Type::Generic(_, _) => Err(CodeGenError::UnsupportedType("generic types".to_string())),
        }
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
            Expr::RecordLit(_record_lit) => {
                // For now, records are not directly supported in WASM
                // We'd need to implement them as memory structures
                return Err(CodeGenError::NotImplemented("record literals".to_string()));
            }
            Expr::FieldAccess(_obj_expr, _field_name) => {
                // Field access would require memory layout implementation
                return Err(CodeGenError::NotImplemented("field access".to_string()));
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
            if self.functions.contains_key(func_name) {
                self.output.push_str(&format!("    call ${}\n", func_name));
            } else {
                return Err(CodeGenError::UndefinedFunction(func_name.clone()));
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
                // This is a binding: expr |> name
                // The value is already on the stack, just store it
                self.output.push_str(&format!("    local.set ${}\n", name));
                // And leave it on the stack as the result
                self.output.push_str(&format!("    local.get ${}\n", name));
            }
            PipeTarget::Expr(target_expr) => {
                // This is a function call: expr |> func
                // The argument is already on the stack
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
        let then_result = self.generate_block_contents(&then.then_block)?;
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
    
    fn generate_wat(input: &str) -> Result<String, Box<dyn std::error::Error>> {
        let (_, ast) = parse_program(input)?;
        let mut codegen = WasmCodeGen::new();
        Ok(codegen.generate(&ast)?)
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