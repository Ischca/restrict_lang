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
    mutable: bool,
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
    // Method implementations: record_name -> method_name -> function_def
    methods: HashMap<String, HashMap<String, FunctionDef>>,
    // Available contexts
    _contexts: Vec<String>,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut checker = Self {
            var_env: vec![HashMap::new()],
            records: HashMap::new(),
            functions: HashMap::new(),
            methods: HashMap::new(),
            _contexts: Vec::new(),
        };
        
        // Register built-in functions
        checker.register_builtins();
        
        checker
    }
    
    fn register_builtins(&mut self) {
        // println function
        self.functions.insert("println".to_string(), FunctionDef {
            params: vec![("s".to_string(), TypedType::String)],
            return_type: TypedType::Unit,
        });
        
        // list_length function
        self.functions.insert("list_length".to_string(), FunctionDef {
            params: vec![("list".to_string(), TypedType::List(Box::new(TypedType::Int32)))],
            return_type: TypedType::Int32,
        });
        
        // list_get function
        self.functions.insert("list_get".to_string(), FunctionDef {
            params: vec![
                ("list".to_string(), TypedType::List(Box::new(TypedType::Int32))),
                ("index".to_string(), TypedType::Int32)
            ],
            return_type: TypedType::Int32,
        });
        
        // array_get function
        self.functions.insert("array_get".to_string(), FunctionDef {
            params: vec![
                ("array".to_string(), TypedType::Array(Box::new(TypedType::Int32), 0)), // Size 0 means any size
                ("index".to_string(), TypedType::Int32)
            ],
            return_type: TypedType::Int32,
        });
        
        // array_set function
        self.functions.insert("array_set".to_string(), FunctionDef {
            params: vec![
                ("array".to_string(), TypedType::Array(Box::new(TypedType::Int32), 0)),
                ("index".to_string(), TypedType::Int32),
                ("value".to_string(), TypedType::Int32)
            ],
            return_type: TypedType::Unit,
        });
        
        // Note: Arena is a built-in context but not added to _contexts by default
        // It only becomes available inside a "with Arena" block
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
                // Mutable variables can be used multiple times
                if var.used && !var.mutable {
                    return Err(TypeError::AffineViolation(name.to_string()));
                }
                if !var.mutable {
                    var.used = true;
                }
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
        current_scope.insert(name, Variable { ty, mutable, used: false });
        Ok(())
    }
    
    fn lookup_var_for_assignment(&mut self, name: &str) -> Result<(TypedType, bool), TypeError> {
        // Look up variable without marking it as used (for assignment target)
        for scope in self.var_env.iter().rev() {
            if let Some(var) = scope.get(name) {
                return Ok((var.ty.clone(), var.mutable));
            }
        }
        Err(TypeError::UndefinedVariable(name.to_string()))
    }
    
    fn reassign_var(&mut self, name: &str, ty: &TypedType) -> Result<(), TypeError> {
        // Find the variable and check if it's mutable
        for scope in self.var_env.iter_mut().rev() {
            if let Some(var) = scope.get_mut(name) {
                if !var.mutable {
                    return Err(TypeError::ImmutableReassignment(name.to_string()));
                }
                if &var.ty != ty {
                    return Err(TypeError::TypeMismatch {
                        expected: format!("{:?}", var.ty),
                        found: format!("{:?}", ty),
                    });
                }
                // Don't mark as used for reassignment
                return Ok(());
            }
        }
        Err(TypeError::UndefinedVariable(name.to_string()))
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
    
    // Wrapper function for compatibility
    pub fn type_check(&mut self, program: &Program) -> Result<(), TypeError> {
        self.check_program(program)
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
        
        // Check if this is a new binding or reassignment
        if let Ok((_existing_ty, _is_mutable)) = self.lookup_var_for_assignment(&bind.name) {
            // This is a reassignment
            self.reassign_var(&bind.name, &ty)?;
        } else {
            // This is a new binding
            self.bind_var(bind.name.clone(), ty, bind.mutable)?;
        }
        Ok(())
    }
    
    fn check_assignment(&mut self, assign: &AssignStmt) -> Result<(), TypeError> {
        let value_ty = self.check_expr(&assign.value)?;
        self.reassign_var(&assign.name, &value_ty)
    }
    
    fn check_impl_block(&mut self, impl_block: &ImplBlock) -> Result<(), TypeError> {
        // Verify the record exists
        if !self.records.contains_key(&impl_block.target) {
            return Err(TypeError::UndefinedRecord(impl_block.target.clone()));
        }
        
        // Clone the target to avoid borrow issues
        let target = impl_block.target.clone();
        
        for func in &impl_block.functions {
            // Check the method, but with special handling for 'self' parameter
            self.push_scope();
            
            let mut param_types = Vec::new();
            for (i, param) in func.params.iter().enumerate() {
                let ty = if i == 0 && param.name == "self" {
                    // First parameter named 'self' should be the record type
                    TypedType::Record { 
                        name: target.clone(), 
                        frozen: false  // Methods can be called on both frozen and unfrozen records
                    }
                } else {
                    self.convert_type(&param.ty)?
                };
                param_types.push((param.name.clone(), ty.clone()));
                self.bind_var(param.name.clone(), ty, false)?;
            }
            
            let return_type = self.check_block_expr(&func.body)?;
            
            // Store the method in the methods map
            let method_map = self.methods.entry(target.clone()).or_insert_with(HashMap::new);
            method_map.insert(func.name.clone(), FunctionDef {
                params: param_types,
                return_type,
            });
            
            self.pop_scope();
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
            Expr::Then(then) => self.check_then_expr(then),
            Expr::While(while_expr) => self.check_while_expr(while_expr),
            Expr::Match(match_expr) => self.check_match_expr(match_expr),
            Expr::ListLit(elements) => self.check_list_lit(elements),
            Expr::ArrayLit(elements) => self.check_array_lit(elements),
            Expr::Some(expr) => {
                let inner_type = self.check_expr(expr)?;
                Ok(TypedType::Option(Box::new(inner_type)))
            },
            Expr::None => {
                // Type will be inferred from context
                // For now, return a generic Option type
                // This should be refined based on usage context
                Ok(TypedType::Option(Box::new(TypedType::Unit)))
            },
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
                // First try to find a regular function
                if let Some(func_info) = self.functions.get(name) {
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
                        
                        // Special handling for array types with size 0 (meaning any size)
                        let types_match = match (expected_ty, &actual_ty) {
                            (TypedType::Array(e_elem, 0), TypedType::Array(a_elem, _)) => {
                                // Size 0 means any size array
                                e_elem == a_elem
                            }
                            (TypedType::List(e_elem), TypedType::List(a_elem)) => {
                                // For lists, just check element type
                                e_elem == a_elem
                            }
                            _ => expected_ty == &actual_ty
                        };
                        
                        if !types_match {
                            return Err(TypeError::TypeMismatch {
                                expected: format!("{:?}", expected_ty),
                                found: format!("{:?}", actual_ty),
                            });
                        }
                    }
                    
                    Ok(return_type)
                } else {
                    // Try to find a method
                    // Check if the first argument is a record type
                    if let Some(first_arg) = call.args.first() {
                        if let Ok(first_arg_ty) = self.check_expr(first_arg) {
                            if let TypedType::Record { name: record_name, .. } = &first_arg_ty {
                                // Look for the method in this record's methods
                                if let Some(method_map) = self.methods.get(record_name) {
                                    if let Some(method_info) = method_map.get(name) {
                                        let expected_arity = method_info.params.len();
                                        let return_type = method_info.return_type.clone();
                                        let param_types: Vec<TypedType> = method_info.params.iter()
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
                                        
                                        return Ok(return_type);
                                    }
                                }
                            }
                        }
                    }
                    
                    Err(TypeError::UndefinedFunction(name.clone()))
                }
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
                Stmt::Assignment(assign) => self.check_assignment(assign)?,
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
            // Check if it's a built-in context or a user-defined context
            if ctx_name == "Arena" {
                // Arena is a built-in context
                self._contexts.push(ctx_name.clone());
            } else if self.records.contains_key(ctx_name) {
                // User-defined context
                self._contexts.push(ctx_name.clone());
            } else {
                return Err(TypeError::UnavailableContext(ctx_name.clone()));
            }
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
    
    fn check_then_expr(&mut self, then: &ThenExpr) -> Result<TypedType, TypeError> {
        // Check condition is boolean
        let cond_ty = self.check_expr(&then.condition)?;
        if cond_ty != TypedType::Boolean {
            return Err(TypeError::TypeMismatch {
                expected: "Boolean".to_string(),
                found: format!("{:?}", cond_ty),
            });
        }
        
        // Check then branch
        self.push_scope();
        let then_ty = self.check_block_expr(&then.then_block)?;
        self.pop_scope();
        
        // Check else-if branches
        let result_ty = then_ty.clone();
        for (else_cond, else_block) in &then.else_ifs {
            let else_cond_ty = self.check_expr(else_cond)?;
            if else_cond_ty != TypedType::Boolean {
                return Err(TypeError::TypeMismatch {
                    expected: "Boolean".to_string(),
                    found: format!("{:?}", else_cond_ty),
                });
            }
            
            self.push_scope();
            let else_if_ty = self.check_block_expr(else_block)?;
            self.pop_scope();
            
            if else_if_ty != result_ty {
                return Err(TypeError::TypeMismatch {
                    expected: format!("{:?}", result_ty),
                    found: format!("{:?}", else_if_ty),
                });
            }
        }
        
        // Check else branch
        if let Some(else_block) = &then.else_block {
            self.push_scope();
            let else_ty = self.check_block_expr(else_block)?;
            self.pop_scope();
            
            if else_ty != result_ty {
                return Err(TypeError::TypeMismatch {
                    expected: format!("{:?}", result_ty),
                    found: format!("{:?}", else_ty),
                });
            }
        } else {
            // No else branch - result must be Unit
            if result_ty != TypedType::Unit {
                return Err(TypeError::TypeMismatch {
                    expected: "Unit (missing else branch)".to_string(),
                    found: format!("{:?}", result_ty),
                });
            }
        }
        
        Ok(result_ty)
    }
    
    fn check_while_expr(&mut self, while_expr: &WhileExpr) -> Result<TypedType, TypeError> {
        // Check condition is boolean
        let cond_type = self.check_expr(&while_expr.condition)?;
        if cond_type != TypedType::Boolean {
            return Err(TypeError::TypeMismatch {
                expected: "Boolean".to_string(),
                found: format!("{:?}", cond_type),
            });
        }
        
        // Check body in new scope
        self.push_scope();
        self.check_block_expr(&while_expr.body)?;
        self.pop_scope();
        
        // While loops always return Unit
        Ok(TypedType::Unit)
    }
    
    fn check_match_expr(&mut self, match_expr: &MatchExpr) -> Result<TypedType, TypeError> {
        // Check the scrutinee expression
        let scrutinee_type = self.check_expr(&match_expr.expr)?;
        
        // Check that we have at least one arm
        if match_expr.arms.is_empty() {
            return Err(TypeError::TypeMismatch {
                expected: "at least one match arm".to_string(),
                found: "no match arms".to_string(),
            });
        }
        
        // Check each arm and ensure all return the same type
        let mut result_type = None;
        
        for arm in &match_expr.arms {
            // Check pattern compatibility with scrutinee
            self.check_pattern(&arm.pattern, &scrutinee_type)?;
            
            // Check the arm body
            self.push_scope();
            
            // Bind pattern variables
            self.bind_pattern_vars(&arm.pattern, &scrutinee_type)?;
            
            let arm_type = self.check_block_expr(&arm.body)?;
            
            self.pop_scope();
            
            // Ensure all arms have the same type
            match &result_type {
                None => result_type = Some(arm_type),
                Some(expected) => {
                    if expected != &arm_type {
                        return Err(TypeError::TypeMismatch {
                            expected: format!("{:?}", expected),
                            found: format!("{:?}", arm_type),
                        });
                    }
                }
            }
        }
        
        // Check exhaustiveness (simple version - just check for wildcard or identifier)
        let has_catch_all = match_expr.arms.iter().any(|arm| {
            matches!(arm.pattern, Pattern::Wildcard | Pattern::Ident(_))
        });
        
        if !has_catch_all && !self.is_pattern_exhaustive(&match_expr.arms, &scrutinee_type) {
            return Err(TypeError::TypeMismatch {
                expected: "exhaustive patterns".to_string(),
                found: "non-exhaustive patterns (add wildcard _ or identifier pattern)".to_string(),
            });
        }
        
        Ok(result_type.unwrap_or(TypedType::Unit))
    }
    
    fn check_pattern(&self, pattern: &Pattern, expected_type: &TypedType) -> Result<(), TypeError> {
        match pattern {
            Pattern::Wildcard => Ok(()),
            Pattern::Ident(_) => Ok(()), // Binds to any type
            Pattern::Literal(lit) => {
                let lit_type = match lit {
                    Literal::Int(_) => TypedType::Int32,
                    Literal::Float(_) => TypedType::Float64,
                    Literal::String(_) => TypedType::String,
                    Literal::Char(_) => TypedType::Char,
                    Literal::Bool(_) => TypedType::Boolean,
                    Literal::Unit => TypedType::Unit,
                };
                
                if &lit_type != expected_type {
                    return Err(TypeError::TypeMismatch {
                        expected: format!("{:?}", expected_type),
                        found: format!("{:?}", lit_type),
                    });
                }
                Ok(())
            }
            Pattern::Record(name, fields) => {
                if let TypedType::Record { name: record_name, .. } = expected_type {
                    if name != record_name {
                        return Err(TypeError::TypeMismatch {
                            expected: record_name.clone(),
                            found: name.clone(),
                        });
                    }
                    
                    // Check fields
                    let record_def = self.records.get(name)
                        .ok_or_else(|| TypeError::UnknownType(name.clone()))?;
                    
                    for (field_name, field_pattern) in fields {
                        let field_type = record_def.fields.get(field_name)
                            .ok_or_else(|| TypeError::UnknownField {
                                record: name.clone(),
                                field: field_name.clone(),
                            })?;
                        
                        self.check_pattern(field_pattern, field_type)?;
                    }
                    Ok(())
                } else {
                    Err(TypeError::TypeMismatch {
                        expected: "record type".to_string(),
                        found: format!("{:?}", expected_type),
                    })
                }
            }
            Pattern::Some(inner_pattern) => {
                if let TypedType::Option(inner_type) = expected_type {
                    self.check_pattern(inner_pattern, inner_type)
                } else {
                    Err(TypeError::TypeMismatch {
                        expected: "Option type".to_string(),
                        found: format!("{:?}", expected_type),
                    })
                }
            }
            Pattern::None => {
                if matches!(expected_type, TypedType::Option(_)) {
                    Ok(())
                } else {
                    Err(TypeError::TypeMismatch {
                        expected: "Option type".to_string(),
                        found: format!("{:?}", expected_type),
                    })
                }
            }
            Pattern::EmptyList => {
                if matches!(expected_type, TypedType::List(_)) {
                    Ok(())
                } else {
                    Err(TypeError::TypeMismatch {
                        expected: "List type".to_string(),
                        found: format!("{:?}", expected_type),
                    })
                }
            }
            Pattern::ListCons(head_pattern, tail_pattern) => {
                if let TypedType::List(element_type) = expected_type {
                    // Check head pattern against element type
                    self.check_pattern(head_pattern, element_type)?;
                    // Check tail pattern against list type
                    self.check_pattern(tail_pattern, expected_type)?;
                    Ok(())
                } else {
                    Err(TypeError::TypeMismatch {
                        expected: "List type".to_string(),
                        found: format!("{:?}", expected_type),
                    })
                }
            }
            Pattern::ListExact(patterns) => {
                if let TypedType::List(element_type) = expected_type {
                    // Check each pattern against element type
                    for pattern in patterns {
                        self.check_pattern(pattern, element_type)?;
                    }
                    Ok(())
                } else {
                    Err(TypeError::TypeMismatch {
                        expected: "List type".to_string(),
                        found: format!("{:?}", expected_type),
                    })
                }
            }
        }
    }
    
    fn bind_pattern_vars(&mut self, pattern: &Pattern, ty: &TypedType) -> Result<(), TypeError> {
        match pattern {
            Pattern::Wildcard => Ok(()),
            Pattern::Ident(name) => {
                self.bind_var(name.clone(), ty.clone(), false)?;
                Ok(())
            }
            Pattern::Literal(_) => Ok(()),
            Pattern::Record(_, fields) => {
                if let TypedType::Record { name, .. } = ty {
                    // Clone to avoid borrow issues
                    let field_types: Vec<(String, TypedType)> = {
                        let record_def = self.records.get(name).unwrap();
                        fields.iter()
                            .map(|(field_name, _)| {
                                (field_name.clone(), record_def.fields.get(field_name).unwrap().clone())
                            })
                            .collect()
                    };
                    
                    for ((_, field_pattern), (_, field_type)) in fields.iter().zip(field_types.iter()) {
                        self.bind_pattern_vars(field_pattern, field_type)?;
                    }
                }
                Ok(())
            }
            Pattern::Some(inner_pattern) => {
                if let TypedType::Option(inner_type) = ty {
                    self.bind_pattern_vars(inner_pattern, inner_type)
                } else {
                    Ok(())
                }
            }
            Pattern::None => Ok(()),
            Pattern::EmptyList => Ok(()),
            Pattern::ListCons(head_pattern, tail_pattern) => {
                if let TypedType::List(element_type) = ty {
                    // Bind head pattern with element type
                    self.bind_pattern_vars(head_pattern, element_type)?;
                    // Bind tail pattern with list type
                    self.bind_pattern_vars(tail_pattern, ty)?;
                    Ok(())
                } else {
                    Ok(())
                }
            }
            Pattern::ListExact(patterns) => {
                if let TypedType::List(element_type) = ty {
                    // Bind each pattern with element type
                    for pattern in patterns {
                        self.bind_pattern_vars(pattern, element_type)?;
                    }
                }
                Ok(())
            }
        }
    }
    
    fn is_pattern_exhaustive(&self, arms: &[MatchArm], ty: &TypedType) -> bool {
        // Simple exhaustiveness check
        match ty {
            TypedType::Boolean => {
                // Check if we have both true and false patterns
                let has_true = arms.iter().any(|arm| {
                    matches!(&arm.pattern, Pattern::Literal(Literal::Bool(true)))
                });
                let has_false = arms.iter().any(|arm| {
                    matches!(&arm.pattern, Pattern::Literal(Literal::Bool(false)))
                });
                has_true && has_false
            }
            TypedType::Option(_) => {
                // Check if we have both Some and None patterns
                let has_some = arms.iter().any(|arm| {
                    matches!(&arm.pattern, Pattern::Some(_))
                });
                let has_none = arms.iter().any(|arm| {
                    matches!(&arm.pattern, Pattern::None)
                });
                has_some && has_none
            }
            TypedType::Unit => {
                // Unit only has one value
                arms.iter().any(|arm| {
                    matches!(&arm.pattern, Pattern::Literal(Literal::Unit))
                })
            }
            _ => false, // For other types, require wildcard
        }
    }
    
    fn check_list_lit(&mut self, elements: &[Box<Expr>]) -> Result<TypedType, TypeError> {
        if elements.is_empty() {
            // Empty list - we can't infer the element type yet
            // For now, we'll use a placeholder
            return Ok(TypedType::List(Box::new(TypedType::Int32)));
        }
        
        // Check all elements and ensure they have the same type
        let first_type = self.check_expr(&elements[0])?;
        
        for element in elements.iter().skip(1) {
            let element_type = self.check_expr(element)?;
            if element_type != first_type {
                return Err(TypeError::TypeMismatch {
                    expected: format!("{:?}", first_type),
                    found: format!("{:?}", element_type),
                });
            }
        }
        
        Ok(TypedType::List(Box::new(first_type)))
    }
    
    fn check_array_lit(&mut self, elements: &[Box<Expr>]) -> Result<TypedType, TypeError> {
        if elements.is_empty() {
            // Empty array - we can't infer the element type yet
            // For now, we'll use a placeholder with size 0
            return Ok(TypedType::Array(Box::new(TypedType::Int32), 0));
        }
        
        // Check all elements and ensure they have the same type
        let first_type = self.check_expr(&elements[0])?;
        
        for element in elements.iter().skip(1) {
            let element_type = self.check_expr(element)?;
            if element_type != first_type {
                return Err(TypeError::TypeMismatch {
                    expected: format!("{:?}", first_type),
                    found: format!("{:?}", element_type),
                });
            }
        }
        
        let size = elements.len();
        Ok(TypedType::Array(Box::new(first_type), size))
    }
}

// Standalone type_check function for public API
pub fn type_check(program: &Program) -> Result<(), TypeError> {
    let mut checker = TypeChecker::new();
    checker.check_program(program)
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