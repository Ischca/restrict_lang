use std::collections::HashMap;
use crate::ast::*;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum TypeError {
    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),
    
    #[error("Type mismatch: expected {expected}, found {found}")]
    TypeMismatch { expected: String, found: String },
    
    #[error("Variable {0} has already been used (affine type violation)")]
    AffineViolation(String),
    
    #[error("Cannot reassign to immutable variable {0}")]
    ImmutableReassignment(String),
    
    #[error("Unknown type: {0}")]
    UnknownType(String),
    
    #[error("Unknown field {field} in record {record}")]
    UnknownField { record: String, field: String },
    
    #[error("Cannot clone a frozen record")]
    CloneFrozenRecord,
    
    #[error("Cannot freeze an already frozen record")]
    FreezeAlreadyFrozen,
    
    #[error("Record {0} is not defined")]
    UndefinedRecord(String),
    
    #[error("Function {0} is not defined")]
    UndefinedFunction(String),
    
    #[error("Wrong number of arguments: expected {expected}, found {found}")]
    ArityMismatch { expected: usize, found: usize },
    
    #[error("Context {0} is not available in this scope")]
    UnavailableContext(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedType {
    Int32,
    Float64,
    Boolean,
    String,
    Char,
    Unit,
    Record { name: String, frozen: bool },
    Function { params: Vec<TypedType>, return_type: Box<TypedType> },
    Option(Box<TypedType>),
    List(Box<TypedType>),
    Array(Box<TypedType>, usize),
}

#[derive(Debug, Clone)]
struct Variable {
    ty: TypedType,
    _mutable: bool,
    used: bool,  // For affine type checking
}

#[derive(Debug)]
struct RecordDef {
    fields: HashMap<String, TypedType>,
}

#[derive(Debug)]
struct FunctionDef {
    params: Vec<(String, TypedType)>,
    return_type: TypedType,
}

pub struct TypeChecker {
    // Variable environment (stack of scopes)
    var_env: Vec<HashMap<String, Variable>>,
    // Record definitions
    records: HashMap<String, RecordDef>,
    // Function definitions
    functions: HashMap<String, FunctionDef>,
    // Available contexts
    _contexts: Vec<String>,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            var_env: vec![HashMap::new()],
            records: HashMap::new(),
            functions: HashMap::new(),
            _contexts: Vec::new(),
        }
    }
    
    fn push_scope(&mut self) {
        self.var_env.push(HashMap::new());
    }
    
    fn pop_scope(&mut self) {
        self.var_env.pop();
    }
    
    fn lookup_var(&mut self, name: &str) -> Result<TypedType, TypeError> {
        // Search from innermost to outermost scope
        for scope in self.var_env.iter_mut().rev() {
            if let Some(var) = scope.get_mut(name) {
                if var.used {
                    return Err(TypeError::AffineViolation(name.to_string()));
                }
                var.used = true;
                return Ok(var.ty.clone());
            }
        }
        Err(TypeError::UndefinedVariable(name.to_string()))
    }
    
    // Look up variable without marking it as used (for checking only)
    fn _peek_var(&self, name: &str) -> Result<&Variable, TypeError> {
        for scope in self.var_env.iter().rev() {
            if let Some(var) = scope.get(name) {
                return Ok(var);
            }
        }
        Err(TypeError::UndefinedVariable(name.to_string()))
    }
    
    fn bind_var(&mut self, name: String, ty: TypedType, mutable: bool) -> Result<(), TypeError> {
        let current_scope = self.var_env.last_mut().unwrap();
        current_scope.insert(name, Variable { ty, _mutable: mutable, used: false });
        Ok(())
    }
    
    fn convert_type(&self, ty: &Type) -> Result<TypedType, TypeError> {
        match ty {
            Type::Named(name) => match name.as_str() {
                "Int" | "Int32" => Ok(TypedType::Int32),
                "Float" | "Float64" => Ok(TypedType::Float64),
                "Boolean" | "Bool" => Ok(TypedType::Boolean),
                "String" => Ok(TypedType::String),
                "Char" => Ok(TypedType::Char),
                "Unit" => Ok(TypedType::Unit),
                _ => {
                    // Check if it's a record type
                    if self.records.contains_key(name) {
                        Ok(TypedType::Record { name: name.clone(), frozen: false })
                    } else {
                        Err(TypeError::UnknownType(name.clone()))
                    }
                }
            },
            Type::Generic(name, params) => {
                match name.as_str() {
                    "Option" if params.len() == 1 => {
                        Ok(TypedType::Option(Box::new(self.convert_type(&params[0])?)))
                    },
                    "List" if params.len() == 1 => {
                        Ok(TypedType::List(Box::new(self.convert_type(&params[0])?)))
                    },
                    _ => Err(TypeError::UnknownType(format!("{}<{}>", name, params.len())))
                }
            }
        }
    }
    
    pub fn check_program(&mut self, program: &Program) -> Result<(), TypeError> {
        for decl in &program.declarations {
            self.check_top_decl(decl)?;
        }
        Ok(())
    }
    
    fn check_top_decl(&mut self, decl: &TopDecl) -> Result<(), TypeError> {
        match decl {
            TopDecl::Record(record) => self.check_record_decl(record),
            TopDecl::Function(func) => self.check_function_decl(func),
            TopDecl::Binding(bind) => self.check_bind_decl(bind),
            TopDecl::Impl(impl_block) => self.check_impl_block(impl_block),
            TopDecl::Context(context) => self.check_context_decl(context),
        }
    }
    
    fn check_record_decl(&mut self, record: &RecordDecl) -> Result<(), TypeError> {
        let mut fields = HashMap::new();
        for field in &record.fields {
            let ty = self.convert_type(&field.ty)?;
            fields.insert(field.name.clone(), ty);
        }
        self.records.insert(record.name.clone(), RecordDef { fields });
        Ok(())
    }
    
    fn check_function_decl(&mut self, func: &FunDecl) -> Result<(), TypeError> {
        self.push_scope();
        
        let mut param_types = Vec::new();
        for param in &func.params {
            let ty = self.convert_type(&param.ty)?;
            param_types.push((param.name.clone(), ty.clone()));
            self.bind_var(param.name.clone(), ty, false)?;
        }
        
        let return_type = self.check_block_expr(&func.body)?;
        
        self.functions.insert(func.name.clone(), FunctionDef {
            params: param_types,
            return_type,
        });
        
        self.pop_scope();
        Ok(())
    }
    
    fn check_bind_decl(&mut self, bind: &BindDecl) -> Result<(), TypeError> {
        let ty = self.check_expr(&bind.value)?;
        self.bind_var(bind.name.clone(), ty, bind.mutable)?;
        Ok(())
    }
    
    fn check_impl_block(&mut self, impl_block: &ImplBlock) -> Result<(), TypeError> {
        // Verify the record exists
        if !self.records.contains_key(&impl_block.target) {
            return Err(TypeError::UndefinedRecord(impl_block.target.clone()));
        }
        
        for func in &impl_block.functions {
            self.check_function_decl(func)?;
        }
        Ok(())
    }
    
    fn check_context_decl(&mut self, context: &ContextDecl) -> Result<(), TypeError> {
        // Store context definition
        let mut fields = HashMap::new();
        for field in &context.fields {
            let ty = self.convert_type(&field.ty)?;
            fields.insert(field.name.clone(), ty);
        }
        
        // Add to available contexts
        self._contexts.push(context.name.clone());
        
        // Store as a special record type for field access
        self.records.insert(context.name.clone(), RecordDef { fields });
        
        Ok(())
    }
    
    fn check_expr(&mut self, expr: &Expr) -> Result<TypedType, TypeError> {
        match expr {
            Expr::IntLit(_) => Ok(TypedType::Int32),
            Expr::FloatLit(_) => Ok(TypedType::Float64),
            Expr::StringLit(_) => Ok(TypedType::String),
            Expr::CharLit(_) => Ok(TypedType::Char),
            Expr::BoolLit(_) => Ok(TypedType::Boolean),
            Expr::Unit => Ok(TypedType::Unit),
            Expr::Ident(name) => self.lookup_var(name),
            Expr::RecordLit(record_lit) => self.check_record_lit(record_lit),
            Expr::Clone(clone_expr) => self.check_clone_expr(clone_expr),
            Expr::Freeze(expr) => self.check_freeze_expr(expr),
            Expr::FieldAccess(expr, field) => self.check_field_access(expr, field),
            Expr::Call(call) => self.check_call_expr(call),
            Expr::Block(block) => self.check_block_expr(block),
            Expr::Binary(binary) => self.check_binary_expr(binary),
            Expr::Pipe(pipe) => self.check_pipe_expr(pipe),
            Expr::With(with) => self.check_with_expr(with),
            _ => todo!("Type checking for {:?} not implemented", expr),
        }
    }
    
    fn check_record_lit(&mut self, record_lit: &RecordLit) -> Result<TypedType, TypeError> {
        // First check if record exists and collect field types
        let field_types: HashMap<String, TypedType> = {
            let record_def = self.records.get(&record_lit.name)
                .ok_or_else(|| TypeError::UndefinedRecord(record_lit.name.clone()))?;
            record_def.fields.clone()
        };
        
        // Check that all fields are present and have correct types
        for field_init in &record_lit.fields {
            let expected_ty = field_types.get(&field_init.name)
                .ok_or_else(|| TypeError::UnknownField {
                    record: record_lit.name.clone(),
                    field: field_init.name.clone(),
                })?;
            
            let actual_ty = self.check_expr(&field_init.value)?;
            if &actual_ty != expected_ty {
                return Err(TypeError::TypeMismatch {
                    expected: format!("{:?}", expected_ty),
                    found: format!("{:?}", actual_ty),
                });
            }
        }
        
        Ok(TypedType::Record { name: record_lit.name.clone(), frozen: false })
    }
    
    fn check_clone_expr(&mut self, clone_expr: &CloneExpr) -> Result<TypedType, TypeError> {
        let base_ty = self.check_expr(&clone_expr.base)?;
        
        match &base_ty {
            TypedType::Record { name, frozen } => {
                if *frozen {
                    return Err(TypeError::CloneFrozenRecord);
                }
                // Check field updates
                let field_types: HashMap<String, TypedType> = {
                    let record_def = self.records.get(name).unwrap();
                    record_def.fields.clone()
                };
                
                for field_init in &clone_expr.updates.fields {
                    // Verify field exists and type matches
                    let expected_ty = field_types.get(&field_init.name)
                        .ok_or_else(|| TypeError::UnknownField {
                            record: name.clone(),
                            field: field_init.name.clone(),
                        })?;
                    
                    let actual_ty = self.check_expr(&field_init.value)?;
                    if &actual_ty != expected_ty {
                        return Err(TypeError::TypeMismatch {
                            expected: format!("{:?}", expected_ty),
                            found: format!("{:?}", actual_ty),
                        });
                    }
                }
                Ok(TypedType::Record { name: name.clone(), frozen: false })
            }
            _ => Err(TypeError::TypeMismatch {
                expected: "record".to_string(),
                found: format!("{:?}", base_ty),
            })
        }
    }
    
    fn check_freeze_expr(&mut self, expr: &Expr) -> Result<TypedType, TypeError> {
        let ty = self.check_expr(expr)?;
        
        match ty {
            TypedType::Record { name, frozen } => {
                if frozen {
                    return Err(TypeError::FreezeAlreadyFrozen);
                }
                Ok(TypedType::Record { name, frozen: true })
            }
            _ => Err(TypeError::TypeMismatch {
                expected: "record".to_string(),
                found: format!("{:?}", ty),
            })
        }
    }
    
    fn check_field_access(&mut self, expr: &Expr, field: &str) -> Result<TypedType, TypeError> {
        let ty = self.check_expr(expr)?;
        
        match &ty {
            TypedType::Record { name, .. } => {
                let record_def = self.records.get(name).unwrap();
                record_def.fields.get(field)
                    .cloned()
                    .ok_or_else(|| TypeError::UnknownField {
                        record: name.clone(),
                        field: field.to_string(),
                    })
            }
            _ => Err(TypeError::TypeMismatch {
                expected: "record".to_string(),
                found: format!("{:?}", ty),
            })
        }
    }
    
    fn check_call_expr(&mut self, call: &CallExpr) -> Result<TypedType, TypeError> {
        // First check the function expression type
        match &*call.function {
            Expr::Ident(name) => {
                // Look up function definition
                let func_info = self.functions.get(name)
                    .ok_or_else(|| TypeError::UndefinedFunction(name.clone()))?;
                
                let expected_arity = func_info.params.len();
                let return_type = func_info.return_type.clone();
                let param_types: Vec<TypedType> = func_info.params.iter()
                    .map(|(_, ty)| ty.clone())
                    .collect();
                
                // Check arity
                if call.args.len() != expected_arity {
                    return Err(TypeError::ArityMismatch {
                        expected: expected_arity,
                        found: call.args.len(),
                    });
                }
                
                // Check argument types
                for (i, arg) in call.args.iter().enumerate() {
                    let expected_ty = &param_types[i];
                    let actual_ty = self.check_expr(arg)?;
                    if &actual_ty != expected_ty {
                        return Err(TypeError::TypeMismatch {
                            expected: format!("{:?}", expected_ty),
                            found: format!("{:?}", actual_ty),
                        });
                    }
                }
                
                Ok(return_type)
            }
            Expr::FieldAccess(obj_expr, _method_name) => {
                // Method call on object
                let _obj_ty = self.check_expr(obj_expr)?;
                
                // For now, assume method calls return Unit
                // TODO: Implement proper method resolution
                Ok(TypedType::Unit)
            }
            _ => {
                // For other function expressions, just check they exist
                let _func_ty = self.check_expr(&call.function)?;
                // TODO: Check if it's actually a function type
                Ok(TypedType::Unit)
            }
        }
    }
    
    fn check_block_expr(&mut self, block: &BlockExpr) -> Result<TypedType, TypeError> {
        self.push_scope();
        
        let mut last_expr_type = None;
        
        for (i, stmt) in block.statements.iter().enumerate() {
            match stmt {
                Stmt::Binding(bind) => self.check_bind_decl(bind)?,
                Stmt::Expr(expr) => {
                    let ty = self.check_expr(expr)?;
                    // Keep track of the last expression's type
                    if i == block.statements.len() - 1 {
                        last_expr_type = Some(ty);
                    }
                }
            }
        }
        
        let result = if let Some(expr) = &block.expr {
            self.check_expr(expr)?
        } else if let Some(ty) = last_expr_type {
            // If no explicit return expression but last statement was an expression,
            // use its type as the block's type
            ty
        } else {
            TypedType::Unit
        };
        
        self.pop_scope();
        Ok(result)
    }
    
    fn check_binary_expr(&mut self, binary: &BinaryExpr) -> Result<TypedType, TypeError> {
        let left_ty = self.check_expr(&binary.left)?;
        let right_ty = self.check_expr(&binary.right)?;
        
        // Type check based on operator
        match binary.op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                // Arithmetic operators require numeric types
                match (&left_ty, &right_ty) {
                    (TypedType::Int32, TypedType::Int32) => Ok(TypedType::Int32),
                    (TypedType::Float64, TypedType::Float64) => Ok(TypedType::Float64),
                    _ => Err(TypeError::TypeMismatch {
                        expected: "numeric types".to_string(),
                        found: format!("{:?} and {:?}", left_ty, right_ty),
                    })
                }
            }
            BinaryOp::Eq | BinaryOp::Ne => {
                // Equality operators work on same types
                if left_ty == right_ty {
                    Ok(TypedType::Boolean)
                } else {
                    Err(TypeError::TypeMismatch {
                        expected: format!("{:?}", left_ty),
                        found: format!("{:?}", right_ty),
                    })
                }
            }
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                // Comparison operators require numeric types
                match (&left_ty, &right_ty) {
                    (TypedType::Int32, TypedType::Int32) => Ok(TypedType::Boolean),
                    (TypedType::Float64, TypedType::Float64) => Ok(TypedType::Boolean),
                    _ => Err(TypeError::TypeMismatch {
                        expected: "numeric types".to_string(),
                        found: format!("{:?} and {:?}", left_ty, right_ty),
                    })
                }
            }
        }
    }
    
    fn check_pipe_expr(&mut self, pipe: &PipeExpr) -> Result<TypedType, TypeError> {
        let expr_ty = self.check_expr(&pipe.expr)?;
        
        match &pipe.target {
            PipeTarget::Ident(name) => {
                // Pipe to binding: expr |> name
                // This creates a new binding
                self.bind_var(name.clone(), expr_ty.clone(), false)?;
                Ok(expr_ty)
            }
            PipeTarget::Expr(target_expr) => {
                // Pipe to expression: expr |> func
                // This is like func(expr)
                match &**target_expr {
                    Expr::Ident(func_name) => {
                        // Single argument function call
                        let call = CallExpr {
                            function: Box::new(Expr::Ident(func_name.clone())),
                            args: vec![pipe.expr.clone()],
                        };
                        self.check_call_expr(&call)
                    }
                    _ => {
                        // For now, just return the target expression's type
                        self.check_expr(target_expr)
                    }
                }
            }
        }
    }
    
    fn check_with_expr(&mut self, with: &WithExpr) -> Result<TypedType, TypeError> {
        // Push contexts onto the stack
        let original_len = self._contexts.len();
        
        // Verify all contexts exist and push them
        for ctx_name in &with.contexts {
            if !self.records.contains_key(ctx_name) {
                return Err(TypeError::UnavailableContext(ctx_name.clone()));
            }
            self._contexts.push(ctx_name.clone());
        }
        
        // Check the body with contexts available
        let result = self.check_block_expr(&with.body)?;
        
        // Pop contexts (in reverse order)
        self._contexts.truncate(original_len);
        
        Ok(result)
    }
    
    fn _is_context_available(&self, name: &str) -> bool {
        self._contexts.contains(&name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_program;
    
    fn check_program_str(input: &str) -> Result<(), TypeError> {
        let (_, program) = parse_program(input).unwrap();
        let mut checker = TypeChecker::new();
        checker.check_program(&program)
    }
    
    #[test]
    fn test_basic_types() {
        let input = r#"
            val x = 42
            val y = 3.14
            val z = "hello"
            val w = true
        "#;
        assert!(check_program_str(input).is_ok());
    }
    
    #[test]
    fn test_affine_violation() {
        let input = r#"
            val x = 42
            val y = x
            val z = x
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::AffineViolation("x".to_string()))
        );
    }
    
    #[test]
    fn test_record_types() {
        let input = r#"
            record Point { x: Int y: Int }
            val p = Point { x = 10, y = 20 }
        "#;
        assert!(check_program_str(input).is_ok());
    }
    
    #[test]
    fn test_undefined_record() {
        let input = r#"
            val p = Point { x = 10, y = 20 }
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::UndefinedRecord("Point".to_string()))
        );
    }
    
    #[test]
    fn test_field_access() {
        let input = r#"
            record Point { x: Int y: Int }
            val p = Point { x = 10, y = 20 }
            val x = p.x
        "#;
        assert!(check_program_str(input).is_ok());
    }
    
    #[test]
    fn test_unknown_field() {
        let input = r#"
            record Point { x: Int y: Int }
            val p = Point { x = 10, y = 20 }
            val z = p.z
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::UnknownField {
                record: "Point".to_string(),
                field: "z".to_string()
            })
        );
    }
    
    #[test]
    fn test_clone_freeze() {
        let input = r#"
            record Point { x: Int y: Int }
            val p1 = Point { x = 10, y = 20 }
            val p2 = p1.clone { x = 30 }
            val p3 = p2 freeze
        "#;
        assert!(check_program_str(input).is_ok());
    }
    
    #[test]
    fn test_clone_frozen_error() {
        let input = r#"
            record Point { x: Int y: Int }
            val p1 = Point { x = 10, y = 20 } freeze
            val p2 = p1.clone { x = 30 }
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::CloneFrozenRecord)
        );
    }
    
    #[test]
    fn test_affine_field_access() {
        let input = r#"
            record Point { x: Int y: Int }
            val p = Point { x = 10, y = 20 }
            val x = p.x
            val y = p.y
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::AffineViolation("p".to_string()))
        );
    }
    
    #[test]
    fn test_affine_in_blocks() {
        let input = r#"
            val x = 42
            val y = { val z = x }
            val w = x
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::AffineViolation("x".to_string()))
        );
    }
    
    #[test]
    fn test_function_params_affine() {
        let input = r#"
            record Point { x: Int y: Int }
            fun use_twice = p: Point {
                val x = p.x
                val y = p.x
            }
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::AffineViolation("p".to_string()))
        );
    }
    
    #[test]
    fn test_clone_field_type_mismatch() {
        let input = r#"
            record Point { x: Int y: Int }
            val p1 = Point { x = 10, y = 20 }
            val p2 = p1.clone { x = "hello" }
        "#;
        assert!(matches!(
            check_program_str(input),
            Err(TypeError::TypeMismatch { .. })
        ));
    }
    
    #[test]
    fn test_clone_unknown_field() {
        let input = r#"
            record Point { x: Int y: Int }
            val p1 = Point { x = 10, y = 20 }
            val p2 = p1.clone { z = 30 }
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::UnknownField {
                record: "Point".to_string(),
                field: "z".to_string()
            })
        );
    }
    
    #[test]
    fn test_function_call() {
        let input = r#"
            fun add = a: Int b: Int { a }
            val result = (10, 20) add
        "#;
        assert!(check_program_str(input).is_ok());
    }
    
    #[test]
    fn test_function_arity_mismatch() {
        let input = r#"
            fun add = a: Int b: Int { a }
            val result = (10) add
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::ArityMismatch {
                expected: 2,
                found: 1
            })
        );
    }
    
    #[test]
    fn test_undefined_function() {
        let input = r#"
            val result = (10, 20) add
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::UndefinedFunction("add".to_string()))
        );
    }
    
    #[test]
    fn test_binary_arithmetic() {
        let input = r#"
            val x = 10 + 20
            val y = 30 - 10
            val z = 5 * 6
            val w = 20 / 4
        "#;
        assert!(check_program_str(input).is_ok());
    }
    
    #[test]
    fn test_binary_comparison() {
        let input = r#"
            val a = 10 < 20
            val b = 30 > 10
            val c = 5 == 5
            val d = 10 != 20
        "#;
        assert!(check_program_str(input).is_ok());
    }
    
    #[test]
    fn test_binary_type_mismatch() {
        let input = r#"
            val x = 10 + "hello"
        "#;
        assert!(matches!(
            check_program_str(input),
            Err(TypeError::TypeMismatch { .. })
        ));
    }
    
    #[test]
    fn test_pipe_binding() {
        let input = r#"
            val x = 42 |> doubled
            val y = doubled
        "#;
        assert!(check_program_str(input).is_ok());
    }
    
    #[test]
    fn test_pipe_function() {
        let input = r#"
            fun inc = x: Int { x }
            val result = 42 |> inc
        "#;
        assert!(check_program_str(input).is_ok());
    }
    
    #[test]
    fn test_context_basic() {
        let input = r#"
            context DB { host: String port: Int }
            
            with DB {
                val x = 42
            }
        "#;
        assert!(check_program_str(input).is_ok());
    }
    
    #[test]
    fn test_context_unavailable() {
        let input = r#"
            val y = with DB {
                val x = 42
            }
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::UnavailableContext("DB".to_string()))
        );
    }
    
    #[test]
    fn test_multiple_contexts() {
        let input = r#"
            context DB { host: String }
            context Cache { size: Int }
            
            with (DB, Cache) {
                val x = 42
            }
        "#;
        assert!(check_program_str(input).is_ok());
    }
}