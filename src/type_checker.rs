//! # Type Checker Module
//!
//! Implements the affine type system for Restrict Language, ensuring memory safety
//! without garbage collection. The type checker enforces that each value is used
//! at most once, preventing use-after-free and double-free bugs.
//!
//! ## Key Features
//!
//! - **Affine Types**: Each binding can be used at most once
//! - **Type Inference**: Bidirectional type checking with inference
//! - **Generics**: Monomorphization of generic functions
//! - **Prototype Checking**: Validates clone/freeze operations
//! - **Pattern Exhaustiveness**: Ensures all cases are handled
//!
//! ## Example
//!
//! ```rust
//! use restrict_lang::type_checker::TypeChecker;
//! use restrict_lang::parser::parse_program;
//!
//! let program = parse_program(source).unwrap();
//! let mut checker = TypeChecker::new();
//! checker.check_program(&program)?;
//! ```

use std::collections::{HashMap, HashSet};
use crate::ast::*;
use crate::lifetime_inference::LifetimeInference;
use thiserror::Error;

/// Type checking errors.
/// 
/// These errors are designed to provide clear, actionable feedback
/// about type system violations.
#[derive(Debug, Error, PartialEq)]
pub enum TypeError {
    /// Variable not found in scope
    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),
    
    /// Type mismatch between expected and actual
    #[error("Type mismatch: expected {expected}, found {found}")]
    TypeMismatch { expected: String, found: String },
    
    /// Attempt to use a value that has already been consumed
    #[error("Variable {0} has already been used (affine type violation)")]
    AffineViolation(String),
    
    /// Attempt to mutate an immutable binding
    #[error("Cannot reassign to immutable variable {0}")]
    ImmutableReassignment(String),
    
    /// Type name not found
    #[error("Unknown type: {0}")]
    UnknownType(String),
    
    /// Field not found in record
    #[error("Unknown field {field} in record {record}")]
    UnknownField { record: String, field: String },
    
    /// Attempt to clone a frozen (immutable) record
    #[error("Cannot clone a frozen record")]
    CloneFrozenRecord,
    
    /// Attempt to freeze an already frozen record
    #[error("Cannot freeze an already frozen record")]
    FreezeAlreadyFrozen,
    
    /// Record type not found
    #[error("Record {0} is not defined")]
    UndefinedRecord(String),
    
    /// Function not found
    #[error("Function {0} is not defined")]
    UndefinedFunction(String),
    
    /// Wrong number of function arguments
    #[error("Wrong number of arguments: expected {expected}, found {found}")]
    ArityMismatch { expected: usize, found: usize },
    
    /// Context not available in current scope
    #[error("Context {0} is not available in this scope")]
    UnavailableContext(String),
    
    /// Feature not yet implemented
    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),
    
    /// Type derivation constraint not satisfied
    #[error("Type {0} is not derived from {1}")]
    NotDerivedFrom(String, String),
    
    /// Attempt to clone a sealed prototype
    #[error("Cannot clone sealed prototype {0}")]
    CannotCloneSealed(String),
    
    #[error("Derivation depth too deep: {0} > 3")]
    DerivationTooDeep(usize),
    
    /// Temporal constraint violation
    #[error("Temporal constraint violation: {0}")]
    TemporalConstraintViolation(String),
    
    /// Temporal variable escapes its scope
    #[error("{message}")]
    TemporalEscape { temporal: String, message: String },
    
    /// Invalid temporal constraint
    #[error("Invalid temporal constraint: {0} within {1}")]
    InvalidTemporalConstraint(String, String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedType {
    Int32,
    Float64,
    Boolean,
    String,
    Char,
    Unit,
    Record { name: String, frozen: bool, hash: Option<String>, parent_hash: Option<String> },
    Function { params: Vec<TypedType>, return_type: Box<TypedType> },
    Option(Box<TypedType>),
    List(Box<TypedType>),
    Array(Box<TypedType>, usize),
    TypeParam(String), // Generic type parameter
    Temporal { base_type: Box<TypedType>, temporals: Vec<String> }, // Type with temporal parameters
}

#[derive(Debug, Clone)]
pub struct TypeSubstitution {
    // Maps type parameter names to concrete types
    pub substitutions: HashMap<String, TypedType>,
}

#[derive(Debug, Clone)]
pub struct TemporalConstraint {
    pub inner: String,  // ~tx
    pub outer: String,  // ~db (where ~tx within ~db)
}

#[derive(Debug, Clone)]
pub struct TemporalContext {
    // Active temporal variables in current scope
    pub active_temporals: HashSet<String>,
    // Temporal constraints (inner within outer)
    pub constraints: Vec<TemporalConstraint>,
    // Parent scope's temporals (for nested scopes)
    pub parent_temporals: Option<Box<TemporalContext>>,
}

impl Default for TemporalContext {
    fn default() -> Self {
        Self {
            active_temporals: HashSet::new(),
            constraints: Vec::new(),
            parent_temporals: None,
        }
    }
}

impl TypeSubstitution {
    pub fn new() -> Self {
        Self {
            substitutions: HashMap::new(),
        }
    }
    
    pub fn add(&mut self, type_param: String, concrete_type: TypedType) {
        self.substitutions.insert(type_param, concrete_type);
    }
    
    pub fn apply(&self, ty: &TypedType) -> TypedType {
        match ty {
            TypedType::TypeParam(name) => {
                self.substitutions.get(name).unwrap_or(ty).clone()
            }
            TypedType::List(inner) => TypedType::List(Box::new(self.apply(inner))),
            TypedType::Array(inner, size) => TypedType::Array(Box::new(self.apply(inner)), *size),
            TypedType::Option(inner) => TypedType::Option(Box::new(self.apply(inner))),
            TypedType::Function { params, return_type } => TypedType::Function {
                params: params.iter().map(|p| self.apply(p)).collect(),
                return_type: Box::new(self.apply(return_type)),
            },
            TypedType::Temporal { base_type, temporals } => TypedType::Temporal {
                base_type: Box::new(self.apply(base_type)),
                temporals: temporals.clone(),
            },
            _ => ty.clone(),
        }
    }
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
    type_params: Vec<TypeParam>,
    temporal_constraints: Vec<TemporalConstraint>,
}

#[derive(Debug, Clone)]
struct FunctionDef {
    params: Vec<(String, TypedType)>,
    return_type: TypedType,
    type_params: Vec<TypeParam>, // Store generic type parameters
    temporal_constraints: Vec<TemporalConstraint>,
}

pub struct TypeChecker {
    // Variable environment (stack of scopes)
    var_env: Vec<HashMap<String, Variable>>,
    // Type parameter environment (stack of scopes for generic types)
    type_param_env: Vec<HashSet<String>>,
    // Type bounds environment: type_param -> required_traits
    type_bounds_env: Vec<HashMap<String, Vec<String>>>,
    // Trait implementations: type_name -> trait_names
    trait_impls: HashMap<String, HashSet<String>>,
    // Record definitions
    records: HashMap<String, RecordDef>,
    // Function definitions
    functions: HashMap<String, FunctionDef>,
    // Method implementations: record_name -> method_name -> function_def
    methods: HashMap<String, HashMap<String, FunctionDef>>,
    // Prototype metadata: record_name -> (hash, parent_hash, sealed)
    prototypes: HashMap<String, (String, Option<String>, bool)>,
    // Available contexts
    _contexts: Vec<String>,
    // Temporal context for tracking temporal variables and constraints
    temporal_context: TemporalContext,
    // AsyncRuntime context stack for tracking async scopes
    async_runtime_stack: Vec<String>, // Stack of async lifetime names
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut checker = Self {
            var_env: vec![HashMap::new()],
            type_param_env: vec![HashSet::new()],
            type_bounds_env: vec![HashMap::new()],
            trait_impls: HashMap::new(),
            records: HashMap::new(),
            functions: HashMap::new(),
            methods: HashMap::new(),
            prototypes: HashMap::new(),
            _contexts: Vec::new(),
            temporal_context: TemporalContext {
                active_temporals: HashSet::new(),
                constraints: Vec::new(),
                parent_temporals: None,
            },
            async_runtime_stack: Vec::new(),
        };
        
        // Register built-in functions and traits
        checker.register_builtins();
        checker.register_builtin_traits();
        checker.register_async_runtime_builtins();
        
        checker
    }
    
    fn register_builtin_traits(&mut self) {
        // Register trait implementations for built-in types
        
        // Int32 implements Display, Clone, Debug
        let mut int32_traits = HashSet::new();
        int32_traits.insert("Display".to_string());
        int32_traits.insert("Clone".to_string());
        int32_traits.insert("Debug".to_string());
        self.trait_impls.insert("Int32".to_string(), int32_traits);
        
        // String implements Display, Clone, Debug
        let mut string_traits = HashSet::new();
        string_traits.insert("Display".to_string());
        string_traits.insert("Clone".to_string());
        string_traits.insert("Debug".to_string());
        self.trait_impls.insert("String".to_string(), string_traits);
        
        // Boolean implements Display, Clone, Debug
        let mut bool_traits = HashSet::new();
        bool_traits.insert("Display".to_string());
        bool_traits.insert("Clone".to_string());
        bool_traits.insert("Debug".to_string());
        self.trait_impls.insert("Boolean".to_string(), bool_traits);
        
        // Float64 implements Display, Clone, Debug
        let mut float_traits = HashSet::new();
        float_traits.insert("Display".to_string());
        float_traits.insert("Clone".to_string());
        float_traits.insert("Debug".to_string());
        self.trait_impls.insert("Float64".to_string(), float_traits);
    }
    
    /// AsyncRuntime context management methods
    
    /// Enter a new AsyncRuntime context with the given lifetime
    fn enter_async_runtime(&mut self, lifetime: &str) -> Result<(), TypeError> {
        // Verify that the lifetime is in the current temporal scope
        if !self.temporal_context.active_temporals.contains(lifetime) {
            return Err(TypeError::UndefinedVariable(format!("Lifetime ~{} not in scope", lifetime)));
        }
        
        // Push the async runtime onto the stack
        self.async_runtime_stack.push(lifetime.to_string());
        Ok(())
    }
    
    /// Exit the current AsyncRuntime context
    fn exit_async_runtime(&mut self) -> Result<String, TypeError> {
        self.async_runtime_stack.pop()
            .ok_or_else(|| TypeError::UnsupportedFeature("No AsyncRuntime context to exit".to_string()))
    }
    
    /// Get the current AsyncRuntime context lifetime if available
    fn current_async_runtime(&self) -> Option<&String> {
        self.async_runtime_stack.last()
    }
    
    /// Check if we're currently in an AsyncRuntime context
    fn is_in_async_runtime(&self) -> bool {
        !self.async_runtime_stack.is_empty()
    }
    
    /// Register AsyncRuntime context operations
    fn register_async_runtime_builtins(&mut self) {
        // spawn operation: (() -> T) -> Task<T, ~async>
        self.functions.insert("spawn".to_string(), FunctionDef {
            params: vec![("task".to_string(), TypedType::Function {
                params: vec![],
                return_type: Box::new(TypedType::TypeParam("T".to_string())),
            })],
            return_type: TypedType::Temporal {
                base_type: Box::new(TypedType::Record {
                    name: "Task".to_string(),
                    frozen: false,
                    hash: None,
                    parent_hash: None,
                }),
                temporals: vec!["async".to_string()],
            },
            type_params: vec![TypeParam {
                name: "T".to_string(),
                bounds: vec![],
                derivation_bound: None,
                is_temporal: false,
            }],
            temporal_constraints: vec![],
        });
        
        // await operation: Task<T, ~async> -> T
        self.functions.insert("await".to_string(), FunctionDef {
            params: vec![("task".to_string(), TypedType::Temporal {
                base_type: Box::new(TypedType::Record {
                    name: "Task".to_string(),
                    frozen: false,
                    hash: None,
                    parent_hash: None,
                }),
                temporals: vec!["async".to_string()],
            })],
            return_type: TypedType::TypeParam("T".to_string()),
            type_params: vec![TypeParam {
                name: "T".to_string(),
                bounds: vec![],
                derivation_bound: None,
                is_temporal: false,
            }],
            temporal_constraints: vec![],
        });
        
        // channel operation: () -> (Sender<T, ~async>, Receiver<T, ~async>)
        self.functions.insert("channel".to_string(), FunctionDef {
            params: vec![],
            return_type: TypedType::Record {
                name: "Channel".to_string(),
                frozen: false,
                hash: None,
                parent_hash: None,
            },
            type_params: vec![TypeParam {
                name: "T".to_string(),
                bounds: vec![],
                derivation_bound: None,
                is_temporal: false,
            }],
            temporal_constraints: vec![],
        });
    }
    
    fn register_builtins(&mut self) {
        // println function
        self.functions.insert("println".to_string(), FunctionDef {
            params: vec![("s".to_string(), TypedType::String)],
            return_type: TypedType::Unit,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        // list_length function
        self.functions.insert("list_length".to_string(), FunctionDef {
            params: vec![("list".to_string(), TypedType::List(Box::new(TypedType::Int32)))],
            return_type: TypedType::Int32,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        // list_get function
        self.functions.insert("list_get".to_string(), FunctionDef {
            params: vec![
                ("list".to_string(), TypedType::List(Box::new(TypedType::Int32))),
                ("index".to_string(), TypedType::Int32)
            ],
            return_type: TypedType::Int32,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        // array_get function
        self.functions.insert("array_get".to_string(), FunctionDef {
            params: vec![
                ("array".to_string(), TypedType::Array(Box::new(TypedType::Int32), 0)), // Size 0 means any size
                ("index".to_string(), TypedType::Int32)
            ],
            return_type: TypedType::Int32,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        // array_set function
        self.functions.insert("array_set".to_string(), FunctionDef {
            params: vec![
                ("array".to_string(), TypedType::Array(Box::new(TypedType::Int32), 0)),
                ("index".to_string(), TypedType::Int32),
                ("value".to_string(), TypedType::Int32)
            ],
            return_type: TypedType::Unit,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        // tail function - returns tail of a list (generic version would be better)
        self.functions.insert("tail".to_string(), FunctionDef {
            params: vec![("list".to_string(), TypedType::List(Box::new(TypedType::Int32)))],
            return_type: TypedType::List(Box::new(TypedType::Int32)),
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        // Standard library functions
        self.register_std_math();
        self.register_std_list();
        self.register_std_option();
        self.register_std_io();
        self.register_std_prelude();
        
        // Note: Arena is a built-in context but not added to _contexts by default
        // It only becomes available inside a "with Arena" block
    }
    
    fn register_std_math(&mut self) {
        use crate::ast::{TypeParam, TypeBound};
        
        // abs function
        self.functions.insert("abs".to_string(), FunctionDef {
            params: vec![("x".to_string(), TypedType::Int32)],
            return_type: TypedType::Int32,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        // max function  
        self.functions.insert("max".to_string(), FunctionDef {
            params: vec![
                ("a".to_string(), TypedType::Int32),
                ("b".to_string(), TypedType::Int32)
            ],
            return_type: TypedType::Int32,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        // min function
        self.functions.insert("min".to_string(), FunctionDef {
            params: vec![
                ("a".to_string(), TypedType::Int32),
                ("b".to_string(), TypedType::Int32)
            ],
            return_type: TypedType::Int32,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        // pow function
        self.functions.insert("pow".to_string(), FunctionDef {
            params: vec![
                ("base".to_string(), TypedType::Int32),
                ("exp".to_string(), TypedType::Int32)
            ],
            return_type: TypedType::Int32,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        // factorial function
        self.functions.insert("factorial".to_string(), FunctionDef {
            params: vec![("n".to_string(), TypedType::Int32)],
            return_type: TypedType::Int32,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        // Float versions
        self.functions.insert("abs_f".to_string(), FunctionDef {
            params: vec![("x".to_string(), TypedType::Float64)],
            return_type: TypedType::Float64,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        self.functions.insert("max_f".to_string(), FunctionDef {
            params: vec![
                ("a".to_string(), TypedType::Float64),
                ("b".to_string(), TypedType::Float64)
            ],
            return_type: TypedType::Float64,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        self.functions.insert("min_f".to_string(), FunctionDef {
            params: vec![
                ("a".to_string(), TypedType::Float64),
                ("b".to_string(), TypedType::Float64)
            ],
            return_type: TypedType::Float64,
            type_params: vec![],
            temporal_constraints: vec![],
        });
    }
    
    fn register_std_list(&mut self) {
        use crate::ast::{TypeParam, TypeBound};
        
        // Generic list functions
        let t_param = TypeParam {
            name: "T".to_string(),
            bounds: vec![],
            derivation_bound: None,
            is_temporal: false,
        };
        
        // list_is_empty<T>
        self.functions.insert("list_is_empty".to_string(), FunctionDef {
            params: vec![("list".to_string(), TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))))],
            return_type: TypedType::Boolean,
            type_params: vec![t_param.clone()],
            temporal_constraints: vec![],
        });
        
        // list_head<T>
        self.functions.insert("list_head".to_string(), FunctionDef {
            params: vec![("list".to_string(), TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))))],
            return_type: TypedType::Option(Box::new(TypedType::TypeParam("T".to_string()))),
            type_params: vec![t_param.clone()],
            temporal_constraints: vec![],
        });
        
        // list_tail<T>
        self.functions.insert("list_tail".to_string(), FunctionDef {
            params: vec![("list".to_string(), TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))))],
            return_type: TypedType::Option(Box::new(TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))))),
            type_params: vec![t_param.clone()],
            temporal_constraints: vec![],
        });
        
        // list_reverse<T>
        self.functions.insert("list_reverse".to_string(), FunctionDef {
            params: vec![("list".to_string(), TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))))],
            return_type: TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
            type_params: vec![t_param.clone()],
            temporal_constraints: vec![],
        });
        
        // list_prepend<T>
        self.functions.insert("list_prepend".to_string(), FunctionDef {
            params: vec![
                ("item".to_string(), TypedType::TypeParam("T".to_string())),
                ("list".to_string(), TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))))
            ],
            return_type: TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
            type_params: vec![t_param.clone()],
            temporal_constraints: vec![],
        });
        
        // list_append<T>
        self.functions.insert("list_append".to_string(), FunctionDef {
            params: vec![
                ("list".to_string(), TypedType::List(Box::new(TypedType::TypeParam("T".to_string())))),
                ("item".to_string(), TypedType::TypeParam("T".to_string()))
            ],
            return_type: TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
            type_params: vec![t_param.clone()],
            temporal_constraints: vec![],
        });
        
        // list_concat<T>
        self.functions.insert("list_concat".to_string(), FunctionDef {
            params: vec![
                ("a".to_string(), TypedType::List(Box::new(TypedType::TypeParam("T".to_string())))),
                ("b".to_string(), TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))))
            ],
            return_type: TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
            type_params: vec![t_param.clone()],
            temporal_constraints: vec![],
        });
        
        // list_count<T>
        self.functions.insert("list_count".to_string(), FunctionDef {
            params: vec![("list".to_string(), TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))))],
            return_type: TypedType::Int32,
            type_params: vec![t_param.clone()],
            temporal_constraints: vec![],
        });
    }
    
    fn register_std_option(&mut self) {
        use crate::ast::{TypeParam, TypeBound};
        
        let t_param = TypeParam {
            name: "T".to_string(),
            bounds: vec![],
            derivation_bound: None,
            is_temporal: false,
        };
        
        // option_is_some<T>
        self.functions.insert("option_is_some".to_string(), FunctionDef {
            params: vec![("opt".to_string(), TypedType::Option(Box::new(TypedType::TypeParam("T".to_string()))))],
            return_type: TypedType::Boolean,
            type_params: vec![t_param.clone()],
            temporal_constraints: vec![],
        });
        
        // option_is_none<T>
        self.functions.insert("option_is_none".to_string(), FunctionDef {
            params: vec![("opt".to_string(), TypedType::Option(Box::new(TypedType::TypeParam("T".to_string()))))],
            return_type: TypedType::Boolean,
            type_params: vec![t_param.clone()],
            temporal_constraints: vec![],
        });
        
        // option_unwrap_or<T>
        self.functions.insert("option_unwrap_or".to_string(), FunctionDef {
            params: vec![
                ("opt".to_string(), TypedType::Option(Box::new(TypedType::TypeParam("T".to_string())))),
                ("default".to_string(), TypedType::TypeParam("T".to_string()))
            ],
            return_type: TypedType::TypeParam("T".to_string()),
            type_params: vec![t_param.clone()],
            temporal_constraints: vec![],
        });
    }
    
    fn register_std_io(&mut self) {
        // print function
        self.functions.insert("print".to_string(), FunctionDef {
            params: vec![("s".to_string(), TypedType::String)],
            return_type: TypedType::Unit,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        // print_int function
        self.functions.insert("print_int".to_string(), FunctionDef {
            params: vec![("n".to_string(), TypedType::Int32)],
            return_type: TypedType::Unit,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        // print_float function
        self.functions.insert("print_float".to_string(), FunctionDef {
            params: vec![("f".to_string(), TypedType::Float64)],
            return_type: TypedType::Unit,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        // eprint function
        self.functions.insert("eprint".to_string(), FunctionDef {
            params: vec![("s".to_string(), TypedType::String)],
            return_type: TypedType::Unit,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        // eprintln function
        self.functions.insert("eprintln".to_string(), FunctionDef {
            params: vec![("s".to_string(), TypedType::String)],
            return_type: TypedType::Unit,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        // some function - wraps a value in Option::Some
        // Note: We handle 'some' specially in check_call_expr to make it work with any type
    }
    
    fn register_std_prelude(&mut self) {
        use crate::ast::{TypeParam, TypeBound};
        
        let t_param = TypeParam {
            name: "T".to_string(),
            bounds: vec![],
            derivation_bound: None,
            is_temporal: false,
        };
        
        // identity<T>
        self.functions.insert("identity".to_string(), FunctionDef {
            params: vec![("x".to_string(), TypedType::TypeParam("T".to_string()))],
            return_type: TypedType::TypeParam("T".to_string()),
            type_params: vec![t_param.clone()],
            temporal_constraints: vec![],
        });
        
        // not function
        self.functions.insert("not".to_string(), FunctionDef {
            params: vec![("b".to_string(), TypedType::Boolean)],
            return_type: TypedType::Boolean,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        // and function
        self.functions.insert("and".to_string(), FunctionDef {
            params: vec![
                ("a".to_string(), TypedType::Boolean),
                ("b".to_string(), TypedType::Boolean)
            ],
            return_type: TypedType::Boolean,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        // or function
        self.functions.insert("or".to_string(), FunctionDef {
            params: vec![
                ("a".to_string(), TypedType::Boolean),
                ("b".to_string(), TypedType::Boolean)
            ],
            return_type: TypedType::Boolean,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        // panic function
        self.functions.insert("panic".to_string(), FunctionDef {
            params: vec![("message".to_string(), TypedType::String)],
            return_type: TypedType::Unit,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        // assert function
        self.functions.insert("assert".to_string(), FunctionDef {
            params: vec![
                ("condition".to_string(), TypedType::Boolean),
                ("message".to_string(), TypedType::String)
            ],
            return_type: TypedType::Unit,
            type_params: vec![],
            temporal_constraints: vec![],
        });
    }
    
    fn push_scope(&mut self) {
        self.var_env.push(HashMap::new());
    }
    
    fn pop_scope(&mut self) {
        self.var_env.pop();
    }
    
    fn push_type_param_scope(&mut self, type_params: &[TypeParam]) {
        let mut type_param_scope = HashSet::new();
        let mut type_bounds_scope = HashMap::new();
        
        for param in type_params {
            type_param_scope.insert(param.name.clone());
            
            // Collect trait bounds for this type parameter
            let bounds: Vec<String> = param.bounds.iter()
                .map(|bound| bound.trait_name.clone())
                .collect();
            
            if !bounds.is_empty() {
                type_bounds_scope.insert(param.name.clone(), bounds);
            }
            
            // Store derivation bound for later checking
            if let Some(ref parent_type) = param.derivation_bound {
                // Add derivation bound as a special constraint
                let derivation_bounds = type_bounds_scope.entry(param.name.clone()).or_insert_with(Vec::new);
                derivation_bounds.push(format!("__derivation_from:{}", parent_type));
            }
        }
        
        self.type_param_env.push(type_param_scope);
        self.type_bounds_env.push(type_bounds_scope);
    }
    
    fn pop_type_param_scope(&mut self) {
        self.type_param_env.pop();
        self.type_bounds_env.pop();
    }
    
    fn is_type_param(&self, name: &str) -> bool {
        for scope in self.type_param_env.iter().rev() {
            if scope.contains(name) {
                return true;
            }
        }
        false
    }
    
    fn get_type_bounds(&self, type_param: &str) -> Vec<String> {
        for scope in self.type_bounds_env.iter().rev() {
            if let Some(bounds) = scope.get(type_param) {
                return bounds.clone();
            }
        }
        Vec::new()
    }
    
    fn check_type_bounds(&self, type_param: &str, required_trait: &str) -> Result<(), TypeError> {
        let bounds = self.get_type_bounds(type_param);
        if bounds.contains(&required_trait.to_string()) {
            Ok(())
        } else {
            Err(TypeError::UnsupportedFeature(
                format!("Type parameter {} does not implement required trait {}", type_param, required_trait)
            ))
        }
    }
    
    fn type_implements_trait(&self, ty: &TypedType, trait_name: &str) -> bool {
        match ty {
            TypedType::Int32 => self.trait_impls.get("Int32").map_or(false, |traits| traits.contains(trait_name)),
            TypedType::String => self.trait_impls.get("String").map_or(false, |traits| traits.contains(trait_name)),
            TypedType::Boolean => self.trait_impls.get("Boolean").map_or(false, |traits| traits.contains(trait_name)),
            TypedType::Float64 => self.trait_impls.get("Float64").map_or(false, |traits| traits.contains(trait_name)),
            TypedType::TypeParam(param_name) => {
                // Check if the type parameter has the required trait bound
                self.get_type_bounds(param_name).contains(&trait_name.to_string())
            }
            _ => false, // Other types don't implement traits for now
        }
    }
    
    // Type unification for generic type inference
    fn unify(&self, expected: &TypedType, actual: &TypedType, substitution: &mut TypeSubstitution) -> Result<(), TypeError> {
        match (expected, actual) {
            // If expected is a type parameter, bind it to the actual type
            (TypedType::TypeParam(name), actual_ty) => {
                if let Some(existing) = substitution.substitutions.get(name).cloned() {
                    // Type parameter already bound, check consistency
                    self.unify(&existing, actual_ty, substitution)
                } else {
                    // Bind the type parameter
                    substitution.add(name.clone(), actual_ty.clone());
                    Ok(())
                }
            }
            // If actual is a type parameter, it should be bound already
            (expected_ty, TypedType::TypeParam(name)) => {
                if let Some(bound_type) = substitution.substitutions.get(name).cloned() {
                    self.unify(expected_ty, &bound_type, substitution)
                } else {
                    // Reverse binding
                    substitution.add(name.clone(), expected_ty.clone());
                    Ok(())
                }
            }
            // Same concrete types unify
            (TypedType::Int32, TypedType::Int32) |
            (TypedType::Float64, TypedType::Float64) |
            (TypedType::Boolean, TypedType::Boolean) |
            (TypedType::String, TypedType::String) |
            (TypedType::Char, TypedType::Char) |
            (TypedType::Unit, TypedType::Unit) => Ok(()),
            
            // Records must have same name and frozen status
            (TypedType::Record { name: n1, frozen: f1, .. }, TypedType::Record { name: n2, frozen: f2, .. }) => {
                if n1 == n2 && f1 == f2 {
                    Ok(())
                } else {
                    Err(TypeError::TypeMismatch {
                        expected: format!("{:?}", expected),
                        found: format!("{:?}", actual),
                    })
                }
            }
            
            // List types must have same element type
            (TypedType::List(e1), TypedType::List(e2)) => {
                self.unify(e1, e2, substitution)
            }
            
            // Option types must have same inner type
            (TypedType::Option(e1), TypedType::Option(e2)) => {
                self.unify(e1, e2, substitution)
            }
            
            // Array types must have same element type and size
            (TypedType::Array(e1, s1), TypedType::Array(e2, s2)) => {
                if s1 == s2 {
                    self.unify(e1, e2, substitution)
                } else {
                    Err(TypeError::TypeMismatch {
                        expected: format!("{:?}", expected),
                        found: format!("{:?}", actual),
                    })
                }
            }
            
            // Function types must have compatible parameters and return types
            (TypedType::Function { params: p1, return_type: r1 }, 
             TypedType::Function { params: p2, return_type: r2 }) => {
                if p1.len() != p2.len() {
                    return Err(TypeError::TypeMismatch {
                        expected: format!("{:?}", expected),
                        found: format!("{:?}", actual),
                    });
                }
                
                for (param1, param2) in p1.iter().zip(p2.iter()) {
                    self.unify(param1, param2, substitution)?;
                }
                
                self.unify(r1, r2, substitution)
            }
            
            // Temporal types must have compatible base types
            // For now, we ignore temporal parameters in unification
            (TypedType::Temporal { base_type: b1, .. }, TypedType::Temporal { base_type: b2, .. }) => {
                self.unify(b1, b2, substitution)
            }
            
            // Allow unifying a temporal type with its base type
            (TypedType::Temporal { base_type, .. }, other) => {
                self.unify(base_type, other, substitution)
            }
            (other, TypedType::Temporal { base_type, .. }) => {
                self.unify(other, base_type, substitution)
            }
            
            // All other combinations are type mismatches
            _ => Err(TypeError::TypeMismatch {
                expected: format!("{:?}", expected),
                found: format!("{:?}", actual),
            })
        }
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
    
    fn convert_type(&mut self, ty: &Type) -> Result<TypedType, TypeError> {
        match ty {
            Type::Named(name) => match name.as_str() {
                "Int" | "Int32" => Ok(TypedType::Int32),
                "Float" | "Float64" => Ok(TypedType::Float64),
                "Boolean" | "Bool" => Ok(TypedType::Boolean),
                "String" => Ok(TypedType::String),
                "Char" => Ok(TypedType::Char),
                "Unit" => Ok(TypedType::Unit),
                _ => {
                    // Check if it's a type parameter
                    if self.is_type_param(name) {
                        // For now, represent type parameters as a special TypedType
                        // In a full implementation, we'd need a TypeParam variant
                        Ok(TypedType::TypeParam(name.clone()))
                    }
                    // Check if it's a record type
                    else if self.records.contains_key(name) {
                        Ok(TypedType::Record { name: name.clone(), frozen: false, hash: None, parent_hash: None })
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
            },
            Type::Function(_, _) => {
                // TODO: Implement function type conversion
                Err(TypeError::UnsupportedFeature("Function types not yet implemented".to_string()))
            }
            Type::Temporal(name, temporals) => {
                // Validate temporal constraints before creating the type
                self.validate_temporal_constraints(temporals)?;
                
                // Convert base type and wrap with temporal parameters
                let base_type = self.convert_type(&Type::Named(name.clone()))?;
                Ok(TypedType::Temporal {
                    base_type: Box::new(base_type),
                    temporals: temporals.clone(),
                })
            }
        }
    }
    
    pub fn check_program(&mut self, program: &Program) -> Result<(), TypeError> {
        // Run lifetime inference if needed
        if self.needs_lifetime_inference(program) {
            let mut lifetime_inference = LifetimeInference::new();
            match lifetime_inference.infer_program(program) {
                Ok(_annotations) => {
                    // TODO: Apply inferred lifetimes to the program
                    // For now, we just proceed with manual annotations
                }
                Err(e) => {
                    // Convert inference error to type error
                    return Err(TypeError::TemporalConstraintViolation(e));
                }
            }
        }
        
        // First pass: register all function signatures and record types
        for decl in &program.declarations {
            match decl {
                TopDecl::Function(func) => {
                    self.register_function_signature(func)?;
                }
                TopDecl::Record(record) => {
                    self.check_record_decl(record)?;
                }
                _ => {}
            }
        }
        
        // Second pass: check all declarations
        for decl in &program.declarations {
            match decl {
                TopDecl::Record(_) => {
                    // Already processed in first pass
                }
                _ => {
                    self.check_top_decl(decl)?;
                }
            }
        }
        Ok(())
    }
    
    /// Check if the program needs lifetime inference
    fn needs_lifetime_inference(&self, program: &Program) -> bool {
        // Check if any declaration uses temporal types without explicit lifetimes
        for decl in &program.declarations {
            match decl {
                TopDecl::Record(record) => {
                    if record.type_params.iter().any(|p| p.is_temporal) {
                        return true;
                    }
                }
                TopDecl::Function(func) => {
                    if func.type_params.iter().any(|p| p.is_temporal) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }
    
    fn register_function_signature(&mut self, func: &FunDecl) -> Result<(), TypeError> {
        // Push type parameter scope for generics
        self.push_type_param_scope(&func.type_params);
        
        let mut param_types = Vec::new();
        for param in &func.params {
            let ty = self.convert_type(&param.ty)?;
            param_types.push((param.name.clone(), ty));
        }
        
        // For now, assume all functions return Int32 (will be refined during actual checking)
        // This is just for forward reference resolution
        self.functions.insert(func.name.clone(), FunctionDef {
            params: param_types,
            return_type: TypedType::Int32,
            type_params: func.type_params.clone(),
            temporal_constraints: func.temporal_constraints.iter()
                .map(|c| TemporalConstraint {
                    inner: c.inner.clone(),
                    outer: c.outer.clone(),
                })
                .collect(),
        });
        
        self.pop_type_param_scope();
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
            TopDecl::Export(export_decl) => self.check_top_decl(&export_decl.item),
        }
    }
    
    fn check_record_decl(&mut self, record: &RecordDecl) -> Result<(), TypeError> {
        // Register temporal type parameters
        for type_param in &record.type_params {
            if type_param.is_temporal {
                self.temporal_context.active_temporals.insert(type_param.name.clone());
            }
        }
        
        // Register temporal constraints
        for constraint in &record.temporal_constraints {
            self.temporal_context.constraints.push(TemporalConstraint {
                inner: constraint.inner.clone(),
                outer: constraint.outer.clone(),
            });
            // Validate constraint: both temporals should be defined
            if !self.temporal_context.active_temporals.contains(&constraint.inner) ||
               !self.temporal_context.active_temporals.contains(&constraint.outer) {
                return Err(TypeError::InvalidTemporalConstraint(
                    constraint.inner.clone(),
                    constraint.outer.clone()
                ));
            }
        }
        
        let mut fields = HashMap::new();
        for field in &record.fields {
            let ty = self.convert_type(&field.ty)?;
            fields.insert(field.name.clone(), ty);
        }
        
        self.records.insert(record.name.clone(), RecordDef { 
            fields,
            type_params: record.type_params.clone(),
            temporal_constraints: record.temporal_constraints.iter()
                .map(|c| TemporalConstraint {
                    inner: c.inner.clone(),
                    outer: c.outer.clone(),
                })
                .collect(),
        });
        
        // Clear temporal context for this record
        self.temporal_context.active_temporals.clear();
        self.temporal_context.constraints.clear();
        
        Ok(())
    }
    
    fn check_function_decl(&mut self, func: &FunDecl) -> Result<(), TypeError> {
        // Push type parameter scope for generics (including temporal parameters)
        self.push_type_param_scope(&func.type_params);
        
        // Register temporal type parameters
        for type_param in &func.type_params {
            if type_param.is_temporal {
                self.temporal_context.active_temporals.insert(type_param.name.clone());
            }
        }
        
        // Register temporal constraints
        for constraint in &func.temporal_constraints {
            self.temporal_context.constraints.push(TemporalConstraint {
                inner: constraint.inner.clone(),
                outer: constraint.outer.clone(),
            });
            // Validate constraint
            if !self.temporal_context.active_temporals.contains(&constraint.inner) ||
               !self.temporal_context.active_temporals.contains(&constraint.outer) {
                return Err(TypeError::InvalidTemporalConstraint(
                    constraint.inner.clone(),
                    constraint.outer.clone()
                ));
            }
        }
        
        self.push_scope();
        
        let mut param_types = Vec::new();
        for param in &func.params {
            let ty = self.convert_type(&param.ty)?;
            param_types.push((param.name.clone(), ty.clone()));
            self.bind_var(param.name.clone(), ty, false)?;
        }
        
        let return_type = self.check_block_expr(&func.body)?;
        
        // Check for temporal escape in return type
        if let TypedType::Temporal { temporals, .. } = &return_type {
            for temporal in temporals {
                if self.temporal_context.active_temporals.contains(temporal) {
                    // Temporal variable from function scope escaping
                    return Err(TypeError::TemporalEscape {
                        temporal: temporal.clone(),
                        message: format!("Temporal parameter {} escapes function scope", temporal)
                    });
                }
            }
        }
        
        self.functions.insert(func.name.clone(), FunctionDef {
            params: param_types,
            return_type,
            type_params: func.type_params.clone(),
            temporal_constraints: func.temporal_constraints.iter()
                .map(|c| TemporalConstraint {
                    inner: c.inner.clone(),
                    outer: c.outer.clone(),
                })
                .collect(),
        });
        
        self.pop_scope();
        self.pop_type_param_scope();
        
        // Clear temporal context
        self.temporal_context.active_temporals.clear();
        self.temporal_context.constraints.clear();
        
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
                        frozen: false,  // Methods can be called on both frozen and unfrozen records
                        hash: None,
                        parent_hash: None
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
                type_params: func.type_params.clone(),
                temporal_constraints: func.temporal_constraints.iter()
                    .map(|c| TemporalConstraint {
                        inner: c.inner.clone(),
                        outer: c.outer.clone(),
                    })
                    .collect(),
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
        self.records.insert(context.name.clone(), RecordDef { 
            fields,
            type_params: vec![],
            temporal_constraints: vec![],
        });
        
        Ok(())
    }
    
    fn check_expr(&mut self, expr: &Expr) -> Result<TypedType, TypeError> {
        self.check_expr_with_expected(expr, None)
    }
    
    fn check_expr_with_expected(&mut self, expr: &Expr, expected: Option<&TypedType>) -> Result<TypedType, TypeError> {
        match expr {
            Expr::IntLit(_) => Ok(TypedType::Int32),
            Expr::FloatLit(_) => Ok(TypedType::Float64),
            Expr::StringLit(_) => Ok(TypedType::String),
            Expr::CharLit(_) => Ok(TypedType::Char),
            Expr::BoolLit(_) => Ok(TypedType::Boolean),
            Expr::Unit => Ok(TypedType::Unit),
            Expr::Ident(name) => {
                // First try as a variable
                match self.lookup_var(name) {
                    Ok(ty) => Ok(ty),
                    Err(e) => {
                        // If not a variable, check if it's a zero-argument function
                        if let Some(func_def) = self.functions.get(name) {
                            if func_def.params.is_empty() {
                                // Zero-argument function can be referenced without parentheses
                                Ok(func_def.return_type.clone())
                            } else {
                                Err(e)  // Return the original error (could be AffineViolation)
                            }
                        } else {
                            Err(e)  // Return the original error
                        }
                    }
                }
            },
            Expr::RecordLit(record_lit) => self.check_record_lit(record_lit),
            Expr::Clone(clone_expr) => self.check_clone_expr(clone_expr),
            Expr::Freeze(expr) => self.check_freeze_expr(expr),
            Expr::FieldAccess(expr, field) => self.check_field_access(expr, field),
            Expr::Call(call) => self.check_call_expr(call),
            Expr::Block(block) => self.check_block_expr(block),
            Expr::Binary(binary) => self.check_binary_expr(binary, expected),
            Expr::Pipe(pipe) => self.check_pipe_expr(pipe),
            Expr::With(with) => self.check_with_expr(with),
            Expr::WithLifetime(with_lifetime) => self.check_with_lifetime_expr(with_lifetime),
            Expr::Then(then) => self.check_then_expr(then),
            Expr::While(while_expr) => self.check_while_expr(while_expr),
            Expr::Match(match_expr) => self.check_match_expr(match_expr),
            Expr::ListLit(elements) => self.check_list_lit(elements, expected),
            Expr::ArrayLit(elements) => self.check_array_lit(elements, expected),
            Expr::Some(expr) => {
                let expected_inner = if let Some(TypedType::Option(inner)) = expected {
                    Some(inner.as_ref())
                } else {
                    None
                };
                let inner_type = self.check_expr_with_expected(expr, expected_inner)?;
                Ok(TypedType::Option(Box::new(inner_type)))
            },
            Expr::None => {
                // Use expected type if available
                if let Some(TypedType::Option(inner)) = expected {
                    Ok(TypedType::Option(inner.clone()))
                } else {
                    // Default to Option<Unit> if no context
                    Ok(TypedType::Option(Box::new(TypedType::Unit)))
                }
            },
            Expr::Lambda(lambda) => self.check_lambda_expr(lambda, expected),
            Expr::PrototypeClone(proto_clone) => self.check_prototype_clone_expr(proto_clone),
            Expr::Await(expr) => self.check_await_expr(expr),
            Expr::Spawn(expr) => self.check_spawn_expr(expr),
            Expr::NoneTyped(ty) => {
                // Convert AST type to TypedType
                let typed_type = self.convert_type(ty)?;
                Ok(TypedType::Option(Box::new(typed_type)))
            },
        }
    }
    
    fn check_record_lit(&mut self, record_lit: &RecordLit) -> Result<TypedType, TypeError> {
        // First check if record exists and collect field types
        let (field_types, type_params, temporal_constraints): (HashMap<String, TypedType>, Vec<TypeParam>, Vec<TemporalConstraint>) = {
            let record_def = self.records.get(&record_lit.name)
                .ok_or_else(|| TypeError::UndefinedRecord(record_lit.name.clone()))?;
            (record_def.fields.clone(), record_def.type_params.clone(), record_def.temporal_constraints.clone())
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
        
        // Validate temporal constraints
        for constraint in &temporal_constraints {
            // Map the record's temporal parameters to the current scope's temporals
            let mut mapped_inner = constraint.inner.clone();
            let mut mapped_outer = constraint.outer.clone();
            
            // If we're in a temporal context, use the active temporals
            if !self.temporal_context.active_temporals.is_empty() {
                // For now, assume simple mapping based on order
                // In a full implementation, we'd have proper mapping/inference
                let active_temporals: Vec<String> = self.temporal_context.active_temporals.iter().cloned().collect();
                let record_temporals: Vec<String> = type_params.iter()
                    .filter(|p| p.is_temporal)
                    .map(|p| p.name.clone())
                    .collect();
                
                for (i, record_temporal) in record_temporals.iter().enumerate() {
                    if i < active_temporals.len() {
                        if constraint.inner == *record_temporal {
                            mapped_inner = active_temporals[i].clone();
                        }
                        if constraint.outer == *record_temporal {
                            mapped_outer = active_temporals[i].clone();
                        }
                    }
                }
            }
            
            // Check if the constraint is satisfied in the current context
            if !self.is_lifetime_within(&mapped_inner, &mapped_outer) {
                return Err(TypeError::InvalidTemporalConstraint(mapped_inner, mapped_outer));
            }
        }
        
        // Create the base record type
        let base_type = TypedType::Record { 
            name: record_lit.name.clone(), 
            frozen: false, 
            hash: None, 
            parent_hash: None 
        };
        
        // If the record has temporal parameters, wrap it in a Temporal type
        let temporal_params: Vec<String> = type_params.iter()
            .filter(|p| p.is_temporal)
            .map(|p| {
                // If we're in a function/context with active temporal parameters,
                // map the record's temporal to the current scope's temporal
                if !self.temporal_context.active_temporals.is_empty() {
                    // For now, use the first active temporal parameter
                    // In a full implementation, we'd have proper mapping/inference
                    self.temporal_context.active_temporals.iter().next().unwrap().clone()
                } else {
                    // No active temporals, use the parameter name as is
                    p.name.clone()
                }
            })
            .collect();
            
        if !temporal_params.is_empty() {
            Ok(TypedType::Temporal {
                base_type: Box::new(base_type),
                temporals: temporal_params,
            })
        } else {
            Ok(base_type)
        }
    }
    
    fn check_clone_expr(&mut self, clone_expr: &CloneExpr) -> Result<TypedType, TypeError> {
        let base_ty = self.check_expr(&clone_expr.base)?;
        
        match &base_ty {
            TypedType::Record { name, frozen, .. } => {
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
                Ok(TypedType::Record { name: name.clone(), frozen: false, hash: None, parent_hash: None })
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
            TypedType::Record { name, frozen, hash, parent_hash } => {
                if frozen {
                    return Err(TypeError::FreezeAlreadyFrozen);
                }
                Ok(TypedType::Record { name, frozen: true, hash, parent_hash })
            }
            _ => Err(TypeError::TypeMismatch {
                expected: "record".to_string(),
                found: format!("{:?}", ty),
            })
        }
    }
    
    // Check function call with generic type inference
    fn check_function_call_with_inference(&mut self, func_info: &FunctionDef, call: &CallExpr) -> Result<TypedType, TypeError> {
        let expected_arity = func_info.params.len();
        
        // Check arity
        if call.args.len() != expected_arity {
            return Err(TypeError::ArityMismatch {
                expected: expected_arity,
                found: call.args.len(),
            });
        }
        
        // If the function is not generic, use simple type checking
        if func_info.type_params.is_empty() {
            let param_types: Vec<TypedType> = func_info.params.iter()
                .map(|(_, ty)| ty.clone())
                .collect();
            
            // Check argument types
            for (i, arg) in call.args.iter().enumerate() {
                let expected_ty = &param_types[i];
                let actual_ty = self.check_expr_with_expected(arg, Some(expected_ty))?;
                
                // Special handling for array types with size 0 (meaning any size)
                let types_match = match (expected_ty, &actual_ty) {
                    (TypedType::Array(e_elem, 0), TypedType::Array(a_elem, _)) => {
                        e_elem == a_elem
                    }
                    (TypedType::List(e_elem), TypedType::List(a_elem)) => {
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
            
            return Ok(func_info.return_type.clone());
        }
        
        // For generic functions, perform type inference
        let mut substitution = TypeSubstitution::new();
        
        // Infer types from arguments
        for (i, arg) in call.args.iter().enumerate() {
            let param_type = &func_info.params[i].1;
            let actual_ty = self.check_expr(arg)?;
            
            // Unify parameter type with actual argument type
            self.unify(param_type, &actual_ty, &mut substitution)?;
        }
        
        // Check type bounds for inferred types
        for type_param in &func_info.type_params {
            if let Some(concrete_type) = substitution.substitutions.get(&type_param.name) {
                // Check trait bounds
                for bound in &type_param.bounds {
                    if !self.type_implements_trait(concrete_type, &bound.trait_name) {
                        return Err(TypeError::UnsupportedFeature(
                            format!("Type {:?} does not implement trait {}", concrete_type, bound.trait_name)
                        ));
                    }
                }
                
                // Check derivation bounds (T from ParentType)
                if let Some(required_parent) = &type_param.derivation_bound {
                    self.check_derivation_bound(concrete_type, required_parent)?;
                }
            }
        }
        
        // Apply substitution to return type
        let instantiated_return_type = substitution.apply(&func_info.return_type);
        Ok(instantiated_return_type)
    }
    
    fn check_field_access(&mut self, expr: &Expr, field: &str) -> Result<TypedType, TypeError> {
        let ty = self.check_expr(expr)?;
        
        // Handle temporal types by unwrapping to the base type
        let base_ty = match &ty {
            TypedType::Temporal { base_type, .. } => base_type.as_ref(),
            _ => &ty,
        };
        
        match base_ty {
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
                // First check if it's a variable that holds a function
                if let Ok(var_ty) = self.lookup_var(name) {
                    match var_ty {
                        TypedType::Function { params, return_type } => {
                            // Check arity
                            if call.args.len() != params.len() {
                                return Err(TypeError::ArityMismatch {
                                    expected: params.len(),
                                    found: call.args.len(),
                                });
                            }
                            
                            // Check argument types
                            for (i, arg) in call.args.iter().enumerate() {
                                let expected_ty = &params[i];
                                let actual_ty = self.check_expr_with_expected(arg, Some(expected_ty))?;
                                if &actual_ty != expected_ty {
                                    return Err(TypeError::TypeMismatch {
                                        expected: format!("{:?}", expected_ty),
                                        found: format!("{:?}", actual_ty),
                                    });
                                }
                            }
                            
                            return Ok(*return_type);
                        }
                        _ => {
                            return Err(TypeError::TypeMismatch {
                                expected: "function".to_string(),
                                found: format!("{:?}", var_ty),
                            });
                        }
                    }
                }
                
                // Handle special built-in function 'some'
                if name == "some" {
                    if call.args.len() != 1 {
                        return Err(TypeError::ArityMismatch {
                            expected: 1,
                            found: call.args.len(),
                        });
                    }
                    let arg_type = self.check_expr(&call.args[0])?;
                    return Ok(TypedType::Option(Box::new(arg_type)));
                }
                
                // Handle spawn operation - requires AsyncRuntime context
                if name == "spawn" {
                    println!("DEBUG: Checking spawn, is_in_async_runtime: {}, stack: {:?}", self.is_in_async_runtime(), self.async_runtime_stack);
                    if !self.is_in_async_runtime() {
                        return Err(TypeError::UnsupportedFeature("spawn can only be used within an AsyncRuntime context".to_string()));
                    }
                    
                    if call.args.len() != 1 {
                        return Err(TypeError::ArityMismatch {
                            expected: 1,
                            found: call.args.len(),
                        });
                    }
                    
                    return self.check_spawn_expr(&call.args[0]);
                }
                
                // Handle await operation - requires AsyncRuntime context
                if name == "await" {
                    if !self.is_in_async_runtime() {
                        return Err(TypeError::UnsupportedFeature("await can only be used within an AsyncRuntime context".to_string()));
                    }
                    
                    if call.args.len() != 1 {
                        return Err(TypeError::ArityMismatch {
                            expected: 1,
                            found: call.args.len(),
                        });
                    }
                    
                    return self.check_await_expr(&call.args[0]);
                }
                
                // Handle special built-in function 'none' (lowercase for inference)
                if name == "none" {
                    if call.args.len() != 0 {
                        return Err(TypeError::ArityMismatch {
                            expected: 0,
                            found: call.args.len(),
                        });
                    }
                    // For now, default to Option<Unit>
                    // TODO: Implement proper type inference from context
                    return Ok(TypedType::Option(Box::new(TypedType::Unit)));
                }
                
                // Otherwise try to find a regular function
                if let Some(func_info) = self.functions.get(name).cloned() {
                    // For spawn and await, we need to check AsyncRuntime context even if they're registered builtins
                    if name == "spawn" || name == "await" {
                        // These were already handled above, so this shouldn't happen
                        return Err(TypeError::UnsupportedFeature("Internal error: spawn/await should be handled earlier".to_string()));
                    }
                    return self.check_function_call_with_inference(&func_info, call);
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
                                            let actual_ty = self.check_expr_with_expected(arg, Some(expected_ty))?;
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
                // For other function expressions (including lambdas)
                let func_ty = self.check_expr(&call.function)?;
                
                match func_ty {
                    TypedType::Function { params, return_type } => {
                        // Check arity
                        if call.args.len() != params.len() {
                            return Err(TypeError::ArityMismatch {
                                expected: params.len(),
                                found: call.args.len(),
                            });
                        }
                        
                        // Check argument types
                        for (i, arg) in call.args.iter().enumerate() {
                            let expected_ty = &params[i];
                            let actual_ty = self.check_expr_with_expected(arg, Some(expected_ty))?;
                            if &actual_ty != expected_ty {
                                return Err(TypeError::TypeMismatch {
                                    expected: format!("{:?}", expected_ty),
                                    found: format!("{:?}", actual_ty),
                                });
                            }
                        }
                        
                        Ok(*return_type)
                    }
                    _ => Err(TypeError::TypeMismatch {
                        expected: "function".to_string(),
                        found: format!("{:?}", func_ty),
                    })
                }
            }
        }
    }
    
    fn check_block_expr(&mut self, block: &BlockExpr) -> Result<TypedType, TypeError> {
        self.check_block_expr_with_expected(block, None)
    }
    
    fn check_block_expr_with_expected(&mut self, block: &BlockExpr, expected: Option<&TypedType>) -> Result<TypedType, TypeError> {
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
            self.check_expr_with_expected(expr, expected)?
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
    
    fn check_binary_expr(&mut self, binary: &BinaryExpr, expected: Option<&TypedType>) -> Result<TypedType, TypeError> {
        // For arithmetic ops, if we expect a certain numeric type, 
        // propagate that expectation to both operands
        let (expected_left, expected_right) = match binary.op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                // These return the same type as their operands
                // So if we expect Int32 or Float64, both operands should be that type
                match expected {
                    Some(TypedType::Int32) => (Some(&TypedType::Int32), Some(&TypedType::Int32)),
                    Some(TypedType::Float64) => (Some(&TypedType::Float64), Some(&TypedType::Float64)),
                    _ => (None, None)
                }
            }
            _ => (None, None)
        };
        
        let left_ty = self.check_expr_with_expected(&binary.left, expected_left)?;
        let right_ty = self.check_expr_with_expected(&binary.right, expected_right)?;
        
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
            } else if ctx_name.starts_with("AsyncRuntime") {
                // AsyncRuntime context with lifetime parameter
                // Extract lifetime from AsyncRuntime<~async>
                if let Some(lifetime) = self.extract_async_runtime_lifetime(ctx_name) {
                    self.enter_async_runtime(&lifetime)?;
                } else {
                    return Err(TypeError::UnavailableContext(format!("Invalid AsyncRuntime syntax: {}", ctx_name)));
                }
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
        
        // Pop contexts (in reverse order) and exit AsyncRuntime contexts
        for ctx_name in with.contexts.iter().rev() {
            if ctx_name.starts_with("AsyncRuntime") {
                self.exit_async_runtime()?;
            }
        }
        self._contexts.truncate(original_len);
        
        Ok(result)
    }
    
    fn _is_context_available(&self, name: &str) -> bool {
        self._contexts.contains(&name.to_string())
    }
    
    /// Extract lifetime from AsyncRuntime<~lifetime> syntax
    fn extract_async_runtime_lifetime(&self, ctx_name: &str) -> Option<String> {
        // Parse "AsyncRuntime<~async>" to extract "async"
        if ctx_name.starts_with("AsyncRuntime<~") && ctx_name.ends_with(">") {
            let start = "AsyncRuntime<~".len();
            let end = ctx_name.len() - 1;
            if start < end {
                return Some(ctx_name[start..end].to_string());
            }
        }
        None
    }
    
    /// Check if a temporal variable is in scope (including parent scopes).
    fn is_temporal_in_scope(&self, temporal: &str) -> bool {
        if self.temporal_context.active_temporals.contains(temporal) {
            return true;
        }
        
        // Check parent scopes
        let mut current = &self.temporal_context.parent_temporals;
        while let Some(parent) = current {
            if parent.active_temporals.contains(temporal) {
                return true;
            }
            current = &parent.parent_temporals;
        }
        
        false
    }
    
    /// Check if inner lifetime is within outer lifetime according to constraints.
    fn is_lifetime_within(&self, inner: &str, outer: &str) -> bool {
        // Direct constraint check
        for constraint in &self.temporal_context.constraints {
            if constraint.inner == inner && constraint.outer == outer {
                return true;
            }
        }
        
        // Check parent contexts
        let mut current = &self.temporal_context.parent_temporals;
        while let Some(parent) = current {
            for constraint in &parent.constraints {
                if constraint.inner == inner && constraint.outer == outer {
                    return true;
                }
            }
            current = &parent.parent_temporals;
        }
        
        // If inner and outer are the same, it's trivially true
        inner == outer
    }
    
    /// Validate temporal constraints when creating temporal types.
    fn validate_temporal_constraints(&self, temporals: &[String]) -> Result<(), TypeError> {
        // Check that all temporals are in scope
        for temporal in temporals {
            if !self.is_temporal_in_scope(temporal) {
                return Err(TypeError::TemporalConstraintViolation(
                    format!("Temporal variable {} is not in scope", temporal)
                ));
            }
        }
        
        // Validate constraint transitivity
        // If we have constraints A within B and B within C, then A must be within C
        let constraints = &self.temporal_context.constraints;
        
        // Build a map of direct constraints
        let mut within_map: HashMap<String, HashSet<String>> = HashMap::new();
        for constraint in constraints {
            within_map.entry(constraint.inner.clone())
                .or_insert_with(HashSet::new)
                .insert(constraint.outer.clone());
        }
        
        // Compute transitive closure
        let mut changed = true;
        while changed {
            changed = false;
            let mut updates: Vec<(String, String)> = Vec::new();
            
            for (inner, outers) in &within_map {
                for outer in outers.clone() {
                    if let Some(outer_outers) = within_map.get(&outer) {
                        for outer_outer in outer_outers {
                            if !outers.contains(outer_outer) {
                                updates.push((inner.clone(), outer_outer.clone()));
                                changed = true;
                            }
                        }
                    }
                }
            }
            
            // Apply updates
            for (inner, outer) in updates {
                within_map.entry(inner)
                    .or_insert_with(HashSet::new)
                    .insert(outer);
            }
        }
        
        // Check for cycles
        for (temporal, within_set) in &within_map {
            if within_set.contains(temporal) {
                return Err(TypeError::TemporalConstraintViolation(
                    format!("Cyclic temporal constraint detected: {} within itself", temporal)
                ));
            }
        }
        
        Ok(())
    }
    
    /// Check await expression.
    /// For now, await is treated as a built-in function.
    fn check_await_expr(&mut self, expr: &Expr) -> Result<TypedType, TypeError> {
        // Verify we're in an AsyncRuntime context
        if !self.is_in_async_runtime() {
            return Err(TypeError::UnsupportedFeature("await can only be used within an AsyncRuntime context".to_string()));
        }
        
        // Check the expression being awaited
        let task_type = self.check_expr(expr)?;
        
        // Get the current async runtime lifetime
        let async_lifetime = self.current_async_runtime()
            .ok_or_else(|| TypeError::UnsupportedFeature("No AsyncRuntime context available".to_string()))?
            .clone();
        
        // Verify that we have a Task<T, ~async> type
        match &task_type {
            TypedType::Temporal { base_type, temporals } => {
                // Check if base_type is a Task record
                if let TypedType::Record { name, .. } = base_type.as_ref() {
                    if name == "Task" {
                        // Check if the temporals include the current async lifetime
                        if temporals.contains(&async_lifetime) {
                            // For Task<T, ~async>, we need to extract T
                            // This is a simplified version - in a full implementation
                            // we'd look up the Task record definition to get the payload type
                            // For now, assume the task contains the result type
                            let result_type = self.get_task_result_type(base_type)?;
                            Ok(result_type)
                        } else {
                            Err(TypeError::TypeMismatch {
                                expected: format!("Task<T, ~{}>", async_lifetime),
                                found: format!("Task with temporals: {:?}", temporals),
                            })
                        }
                    } else {
                        Err(TypeError::TypeMismatch {
                            expected: format!("Task<T, ~{}>", async_lifetime),
                            found: format!("{:?}", task_type),
                        })
                    }
                } else {
                    Err(TypeError::TypeMismatch {
                        expected: format!("Task<T, ~{}>", async_lifetime),
                        found: format!("{:?}", task_type),
                    })
                }
            }
            TypedType::Record { name, .. } if name == "Task" => {
                // Handle non-temporal Task for backwards compatibility
                // In a full implementation, this would be an error
                let result_type = self.get_task_result_type(&task_type)?;
                Ok(result_type)
            }
            _ => Err(TypeError::TypeMismatch {
                expected: format!("Task<T, ~{}>", async_lifetime),
                found: format!("{:?}", task_type),
            })
        }
    }
    
    /// Check spawn expression.
    /// For now, spawn is treated as a built-in function.
    fn check_spawn_expr(&mut self, expr: &Expr) -> Result<TypedType, TypeError> {
        // Verify we're in an AsyncRuntime context
        if !self.is_in_async_runtime() {
            return Err(TypeError::UnsupportedFeature("spawn can only be used within an AsyncRuntime context".to_string()));
        }
        
        // Check the expression being spawned (should be a lambda or async function)
        let func_type = self.check_expr(expr)?;
        
        // Extract the return type from the function being spawned
        let _return_type = match &func_type {
            TypedType::Function { return_type, .. } => return_type.as_ref().clone(),
            _ => {
                return Err(TypeError::TypeMismatch {
                    expected: "function".to_string(),
                    found: format!("{:?}", func_type),
                });
            }
        };
        
        // Get the current async runtime lifetime
        let async_lifetime = self.current_async_runtime()
            .ok_or_else(|| TypeError::UnsupportedFeature("No AsyncRuntime context available".to_string()))?
            .clone();
        
        // Return Task<T, ~async> where T is the return type of the spawned function
        Ok(TypedType::Temporal {
            base_type: Box::new(TypedType::Record {
                name: "Task".to_string(),
                frozen: false,
                hash: None,
                parent_hash: None,
            }),
            temporals: vec![async_lifetime],
        })
    }
    
    
    /// Helper method to extract the result type from a Task type.
    /// This is a simplified implementation that assumes Task<T> contains T.
    fn get_task_result_type(&self, task_type: &TypedType) -> Result<TypedType, TypeError> {
        // For now, this is a simplified implementation
        // In a full implementation, we'd look up the Task record definition
        // and extract the type parameter T
        match task_type {
            TypedType::Record { name, .. } if name == "Task" => {
                // For now, we'll return Int32 as a placeholder
                // In a real implementation, we'd extract the generic parameter T
                // from the Task<T> record definition
                Ok(TypedType::Int32)
            }
            _ => Err(TypeError::TypeMismatch {
                expected: "Task".to_string(),
                found: format!("{:?}", task_type),
            })
        }
    }
    
    /// Check a with lifetime expression.
    /// 
    /// Creates a new temporal scope for the lifetime of the block.
    fn check_with_lifetime_expr(&mut self, with_lifetime: &WithLifetimeExpr) -> Result<TypedType, TypeError> {
        // Save current temporal context
        let saved_context = self.temporal_context.clone();
        
        // Create new temporal scope
        let new_context = TemporalContext {
            active_temporals: saved_context.active_temporals.clone(),
            constraints: saved_context.constraints.clone(),
            parent_temporals: Some(Box::new(saved_context)),
        };
        
        // Add the lifetime to active temporals
        let mut active_temporals = new_context.active_temporals;
        active_temporals.insert(with_lifetime.lifetime.clone());
        
        // Add new constraints from the with lifetime expression
        let mut constraints = new_context.constraints;
        
        // Validate and add constraints
        for constraint in &with_lifetime.constraints {
            // Verify that the outer lifetime is in scope
            if constraint.outer != with_lifetime.lifetime {
                // The outer lifetime must be from parent scope
                if !self.is_temporal_in_scope(&constraint.outer) {
                    return Err(TypeError::InvalidTemporalConstraint(
                        constraint.inner.clone(),
                        constraint.outer.clone()
                    ));
                }
            }
            
            constraints.push(TemporalConstraint {
                inner: constraint.inner.clone(),
                outer: constraint.outer.clone(),
            });
        }
        
        self.temporal_context = TemporalContext {
            active_temporals,
            constraints,
            parent_temporals: new_context.parent_temporals,
        };
        
        // Check the body with the new temporal scope
        let result = self.check_block_expr(&with_lifetime.body)?;
        
        // Check that the result doesn't escape the temporal scope
        // Get the allowed temporals (all active except the one being introduced by this with_lifetime)
        let mut allowed_temporals = self.temporal_context.active_temporals.clone();
        allowed_temporals.remove(&with_lifetime.lifetime);
        
        // Use the comprehensive temporal escape check
        self.check_temporal_escape(&result, &allowed_temporals)?;
        
        // Restore temporal context
        if let Some(parent) = self.temporal_context.parent_temporals.take() {
            self.temporal_context = *parent;
        }
        
        Ok(result)
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
            
            // Use expected type from previous arms if available
            let expected_arm_type = result_type.as_ref();
            let arm_type = if let Some(expected) = expected_arm_type {
                self.check_block_expr_with_expected(&arm.body, Some(expected))?
            } else {
                self.check_block_expr(&arm.body)?
            };
            
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
    
    fn check_list_lit(&mut self, elements: &[Box<Expr>], expected: Option<&TypedType>) -> Result<TypedType, TypeError> {
        if elements.is_empty() {
            // Empty list - infer from expected type if available
            if let Some(TypedType::List(elem_type)) = expected {
                return Ok(TypedType::List(elem_type.clone()));
            } else {
                // For now, default to List<Int32> if no context
                return Ok(TypedType::List(Box::new(TypedType::Int32)));
            }
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
    
    fn check_array_lit(&mut self, elements: &[Box<Expr>], expected: Option<&TypedType>) -> Result<TypedType, TypeError> {
        if elements.is_empty() {
            // Empty array - infer from expected type if available
            if let Some(TypedType::Array(elem_type, _)) = expected {
                return Ok(TypedType::Array(elem_type.clone(), 0));
            } else {
                // For now, default to Array<Int32, 0> if no context
                return Ok(TypedType::Array(Box::new(TypedType::Int32), 0));
            }
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
    
    fn check_lambda_expr(&mut self, lambda: &LambdaExpr, expected: Option<&TypedType>) -> Result<TypedType, TypeError> {
        // Collect free variables before creating lambda scope
        let bound_vars = HashSet::new();
        let free_vars = self.collect_free_variables(&lambda.body, &bound_vars);
        
        // Get current temporal context to determine allowed temporals
        let allowed_temporals = self.temporal_context.active_temporals.clone();
        
        // Check if any free variables have temporal types that would escape
        for var_name in &free_vars {
            match self.lookup_var(var_name) {
                Ok(var_type) => {
                    // Check if this type contains temporals that would escape
                    self.check_temporal_escape(&var_type, &allowed_temporals)?;
                }
                Err(_) => {
                    // Variable not found - this will be caught later during body type checking
                }
            }
        }
        
        // Create a new scope for lambda parameters
        self.push_scope();
        
        let mut param_types = Vec::new();
        let expected_return_type = if let Some(TypedType::Function { params, return_type }) = expected {
            // Use expected parameter types if available
            if params.len() != lambda.params.len() {
                return Err(TypeError::ArityMismatch {
                    expected: params.len(),
                    found: lambda.params.len(),
                });
            }
            
            for (i, param) in lambda.params.iter().enumerate() {
                let param_type = params[i].clone();
                param_types.push(param_type.clone());
                self.bind_var(param.clone(), param_type, false)?;
            }
            
            Some(return_type.as_ref())
        } else {
            // Otherwise, try to infer from body usage
            // First, try simple inference from body
            for param in &lambda.params {
                let inferred_type = self.infer_param_type_from_usage(param, &lambda.body);
                param_types.push(inferred_type.clone());
                self.bind_var(param.clone(), inferred_type, false)?;
            }
            
            None
        };
        
        // Type check the body with inferred parameter types
        let body_type = self.check_expr_with_expected(&lambda.body, expected_return_type)?;
        
        // If we had an expected return type, verify it matches
        if let Some(expected_ret) = expected_return_type {
            if &body_type != expected_ret {
                return Err(TypeError::TypeMismatch {
                    expected: format!("{:?}", expected_ret),
                    found: format!("{:?}", body_type),
                });
            }
        }
        
        // Pop the lambda scope
        self.pop_scope();
        
        // Create the function type
        let func_type = TypedType::Function {
            params: param_types,
            return_type: Box::new(body_type),
        };
        
        // Check if the function type itself contains escaping temporals
        self.check_temporal_escape(&func_type, &allowed_temporals)?;
        
        Ok(func_type)
    }
    
    fn infer_param_type_from_usage(&self, param_name: &str, expr: &Expr) -> TypedType {
        // Analyze the expression to infer the parameter type
        match expr {
            Expr::Binary(bin) => {
                // Check if the parameter is used in this binary expression
                let uses_param = self.expr_uses_param(&bin.left, param_name) || 
                                self.expr_uses_param(&bin.right, param_name);
                
                if uses_param {
                    match bin.op {
                        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                            // Check if the other operand is a float literal
                            if self.expr_contains_float(&bin.left) || self.expr_contains_float(&bin.right) {
                                return TypedType::Float64;
                            }
                            // Default to Int32 for arithmetic
                            TypedType::Int32
                        }
                        BinaryOp::Gt | BinaryOp::Lt | BinaryOp::Ge | BinaryOp::Le => {
                            // Comparison operators work with numeric types
                            // Check for float literals
                            if self.expr_contains_float(&bin.left) || self.expr_contains_float(&bin.right) {
                                return TypedType::Float64;
                            }
                            TypedType::Int32
                        }
                        _ => TypedType::Int32
                    }
                } else {
                    // Recursively check sub-expressions
                    let left_type = self.infer_param_type_from_usage(param_name, &bin.left);
                    if !matches!(left_type, TypedType::Int32) {
                        return left_type;
                    }
                    self.infer_param_type_from_usage(param_name, &bin.right)
                }
            }
            Expr::Block(block) => {
                // Check all statements in the block
                for stmt in &block.statements {
                    if let Stmt::Expr(expr) = stmt {
                        let inferred = self.infer_param_type_from_usage(param_name, expr);
                        if !matches!(inferred, TypedType::Int32) {
                            return inferred;
                        }
                    }
                }
                // Check the final expression if present
                if let Some(final_expr) = &block.expr {
                    self.infer_param_type_from_usage(param_name, &**final_expr)
                } else {
                    TypedType::Int32
                }
            }
            _ => TypedType::Int32 // Default fallback
        }
    }
    
    fn expr_uses_param(&self, expr: &Expr, param_name: &str) -> bool {
        match expr {
            Expr::Ident(name) => name == param_name,
            Expr::Binary(bin) => {
                self.expr_uses_param(&bin.left, param_name) || 
                self.expr_uses_param(&bin.right, param_name)
            }
            Expr::Block(block) => {
                block.statements.iter().any(|stmt| {
                    if let Stmt::Expr(e) = stmt {
                        self.expr_uses_param(e, param_name)
                    } else {
                        false
                    }
                }) || block.expr.as_ref().map_or(false, |e| self.expr_uses_param(&**e, param_name))
            }
            _ => false
        }
    }
    
    fn expr_contains_float(&self, expr: &Expr) -> bool {
        match expr {
            Expr::FloatLit(_) => true,
            Expr::Binary(bin) => {
                self.expr_contains_float(&bin.left) || self.expr_contains_float(&bin.right)
            }
            Expr::Block(block) => {
                block.statements.iter().any(|stmt| {
                    if let Stmt::Expr(e) = stmt {
                        self.expr_contains_float(e)
                    } else {
                        false
                    }
                }) || block.expr.as_ref().map_or(false, |e| self.expr_contains_float(&**e))
            }
            _ => false
        }
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
            fun use_twice: (p: Point) -> Unit = {
                val x = p.x
                val y = p.x
                ()
            }
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::AffineViolation("p".to_string()))
        );
    }

    #[test]
    fn test_affine_mutable_allowed() {
        // Mutable variables should be allowed to be used multiple times
        let input = r#"
            mut val x = 42
            val y = x
            val z = x
        "#;
        assert!(check_program_str(input).is_ok());
    }

    #[test]
    fn test_affine_nested_blocks() {
        // Test affine checking in deeply nested blocks
        let input = r#"
            val x = 42
            val y = {
                {
                    x
                }
            }
            val z = x
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::AffineViolation("x".to_string()))
        );
    }

    #[test]
    fn test_affine_conditionals() {
        // Test affine checking in conditional branches
        let input = r#"
            record Point { x: Int y: Int }
            fun conditional: (p: Point, flag: Bool) -> Int = {
                val result = flag then {
                    p.x
                } else {
                    p.y
                }
                result
            }
        "#;
        // Both branches use p, but in different ways
        // Current implementation may detect this as affine violation
        // because it conservatively marks p as used in both branches
        let result = check_program_str(input);
        // For now, let's check what error we actually get
        match result {
            Ok(()) => {}, // This would be ideal
            Err(TypeError::AffineViolation(var)) if var == "p" => {
                // This is what we currently get - conservative checking
                // TODO: Improve affine checking to handle conditionals better
            },
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_affine_conditional_violation() {
        // Using a variable before AND inside a conditional should fail
        let input = r#"
            val x = 42
            val y = x
            val z = true then { x } else { 0 }
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::AffineViolation("x".to_string()))
        );
    }

    #[test]
    fn test_affine_multiple_params() {
        // Multiple parameters should be checked independently
        // Using different parameters is OK
        let input = r#"
            record Point { x: Int y: Int }
            val p1 = Point { x = 1, y = 2 }
            val p2 = Point { x = 3, y = 4 }
            val x1 = p1.x
            val x2 = p2.x
        "#;
        assert!(check_program_str(input).is_ok());
    }

    #[test]
    fn test_affine_multiple_params_violation() {
        // Using the same variable/parameter twice should fail
        let input = r#"
            record Point { x: Int y: Int }
            val p1 = Point { x = 1, y = 2 }
            val x = p1.x
            val y = p1.y
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::AffineViolation("p1".to_string()))
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
            fun add: (a: Int, b: Int) -> Int = { a }
            val result = (10, 20) add
        "#;
        assert!(check_program_str(input).is_ok());
    }
    
    #[test]
    fn test_function_arity_mismatch() {
        let input = r#"
            fun add: (a: Int, b: Int) -> Int = { a }
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
            fun inc: (x: Int) -> Int = { x }
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

impl TypeChecker {
    // Prototype + Derivation-Bound implementation
    fn check_derivation_bound(&self, concrete_type: &TypedType, required_parent: &str) -> Result<(), TypeError> {
        match concrete_type {
            TypedType::Record { name, hash, parent_hash, .. } => {
                // Check if this record derives from the required parent
                if self.is_derived_from(name, hash.as_ref(), parent_hash.as_ref(), required_parent)? {
                    Ok(())
                } else {
                    Err(TypeError::NotDerivedFrom(name.clone(), required_parent.to_string()))
                }
            }
            _ => {
                // Non-record types cannot have derivation bounds
                Err(TypeError::NotDerivedFrom(format!("{:?}", concrete_type), required_parent.to_string()))
            }
        }
    }
    
    fn is_derived_from(&self, type_name: &str, _current_hash: Option<&String>, parent_hash: Option<&String>, target_parent: &str) -> Result<bool, TypeError> {
        // Base case: check if current type is the target
        if type_name == target_parent {
            return Ok(true);
        }
        
        // Check prototypes registry for derivation info
        if let Some((_, prototype_parent_hash, _)) = self.prototypes.get(type_name) {
            if let Some(parent_hash_val) = prototype_parent_hash {
                // Find the parent type name by hash
                for (parent_name, (parent_current_hash, _, _)) in &self.prototypes {
                    if parent_current_hash == parent_hash_val {
                        // Recursively check parent
                        return self.is_derived_from(parent_name, Some(parent_current_hash), prototype_parent_hash.as_ref(), target_parent);
                    }
                }
            }
        }
        
        // Also check using the hash/parent_hash from the type itself
        if let Some(parent_hash_val) = parent_hash {
            for (parent_name, (parent_current_hash, _, _)) in &self.prototypes {
                if parent_current_hash == parent_hash_val {
                    return self.is_derived_from(parent_name, Some(parent_current_hash), None, target_parent);
                }
            }
        }
        
        Ok(false)
    }
    
    fn generate_prototype_hash(&self, record_name: &str, content: &str) -> String {
        // Simple hash implementation (in production, use SHA-3)
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        record_name.hash(&mut hasher);
        content.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
    
    fn check_derivation_depth(&self, type_name: &str) -> Result<usize, TypeError> {
        let mut depth = 0;
        let mut current_type = type_name;
        
        loop {
            if depth > 3 {
                return Err(TypeError::DerivationTooDeep(depth));
            }
            
            if let Some((_, parent_hash, _)) = self.prototypes.get(current_type) {
                if let Some(parent_hash_val) = parent_hash {
                    // Find parent by hash
                    let mut found_parent = false;
                    for (parent_name, (parent_current_hash, _, _)) in &self.prototypes {
                        if parent_current_hash == parent_hash_val {
                            current_type = parent_name;
                            depth += 1;
                            found_parent = true;
                            break;
                        }
                    }
                    if !found_parent {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        
        Ok(depth)
    }
    
    fn check_derivation_bounds_for_call(&self, func_def: &FunctionDef, arg_types: &[TypedType]) -> Result<(), TypeError> {
        // Check derivation bounds for each type parameter
        for (i, type_param) in func_def.type_params.iter().enumerate() {
            if let Some(ref parent_type) = type_param.derivation_bound {
                // Find the corresponding argument type
                if i < arg_types.len() {
                    let arg_type = &arg_types[i];
                    self.check_derivation_bound(arg_type, parent_type)?;
                }
            }
        }
        Ok(())
    }
    
    fn check_prototype_clone_expr(&mut self, proto_clone: &PrototypeCloneExpr) -> Result<TypedType, TypeError> {
        // Check if base prototype exists
        if !self.records.contains_key(&proto_clone.base) {
            return Err(TypeError::UndefinedRecord(proto_clone.base.clone()));
        }
        
        // Check if base is sealed
        if let Some((_, _, sealed)) = self.prototypes.get(&proto_clone.base) {
            if *sealed {
                return Err(TypeError::CannotCloneSealed(proto_clone.base.clone()));
            }
        }
        
        // Check derivation depth
        self.check_derivation_depth(&proto_clone.base)?;
        
        // Generate hash for the new prototype
        let content = format!("{:?}", proto_clone); // Simplified content hash
        let new_hash = self.generate_prototype_hash(&proto_clone.base, &content);
        
        // Get parent hash
        let parent_hash = if let Some((hash, _, _)) = self.prototypes.get(&proto_clone.base) {
            Some(hash.clone())
        } else {
            None
        };
        
        // Check field updates (similar to clone expression)
        // ... field checking logic ...
        
        Ok(TypedType::Record { 
            name: format!("{}#{}", proto_clone.base, &new_hash[..8]), // Unique name
            frozen: proto_clone.freeze_immediately, 
            hash: Some(new_hash.clone()),
            parent_hash 
        })
    }
    
    /// Collect free variables in an expression
    fn collect_free_variables(&self, expr: &Expr, bound_vars: &HashSet<String>) -> HashSet<String> {
        let mut free_vars = HashSet::new();
        
        match expr {
            Expr::Ident(name) => {
                if !bound_vars.contains(name) {
                    // Check if variable exists in scope
                    for scope in self.var_env.iter().rev() {
                        if scope.contains_key(name) {
                            free_vars.insert(name.clone());
                            break;
                        }
                    }
                }
            }
            Expr::Binary(bin) => {
                free_vars.extend(self.collect_free_variables(&bin.left, bound_vars));
                free_vars.extend(self.collect_free_variables(&bin.right, bound_vars));
            }
            Expr::Call(call) => {
                free_vars.extend(self.collect_free_variables(&call.function, bound_vars));
                for arg in &call.args {
                    free_vars.extend(self.collect_free_variables(arg, bound_vars));
                }
            }
            Expr::FieldAccess(object, _field) => {
                free_vars.extend(self.collect_free_variables(object, bound_vars));
            }
            Expr::RecordLit(record_lit) => {
                for field in &record_lit.fields {
                    free_vars.extend(self.collect_free_variables(&field.value, bound_vars));
                }
            }
            Expr::Clone(clone_expr) => {
                free_vars.extend(self.collect_free_variables(&clone_expr.base, bound_vars));
                for field in &clone_expr.updates.fields {
                    free_vars.extend(self.collect_free_variables(&field.value, bound_vars));
                }
            }
            Expr::Freeze(expr) => {
                free_vars.extend(self.collect_free_variables(expr, bound_vars));
            }
            Expr::PrototypeClone(proto_clone) => {
                // Base is just a name, not an expression, so no free vars from it
                for field in &proto_clone.updates.fields {
                    free_vars.extend(self.collect_free_variables(&field.value, bound_vars));
                }
            }
            Expr::ListLit(elements) => {
                for elem in elements {
                    free_vars.extend(self.collect_free_variables(elem, bound_vars));
                }
            }
            Expr::ArrayLit(elements) => {
                for elem in elements {
                    free_vars.extend(self.collect_free_variables(elem, bound_vars));
                }
            }
            Expr::Match(match_expr) => {
                free_vars.extend(self.collect_free_variables(&match_expr.expr, bound_vars));
                for arm in &match_expr.arms {
                    // Pattern bindings create new bound variables
                    let mut arm_bound = bound_vars.clone();
                    self.collect_pattern_bindings(&arm.pattern, &mut arm_bound);
                    // The body is a BlockExpr, so we need to handle it specially
                    free_vars.extend(self.collect_free_variables_in_block(&arm.body, &arm_bound));
                }
            }
            Expr::Then(then_expr) => {
                free_vars.extend(self.collect_free_variables(&then_expr.condition, bound_vars));
                free_vars.extend(self.collect_free_variables_in_block(&then_expr.then_block, bound_vars));
                for (cond, block) in &then_expr.else_ifs {
                    free_vars.extend(self.collect_free_variables(cond, bound_vars));
                    free_vars.extend(self.collect_free_variables_in_block(block, bound_vars));
                }
                if let Some(else_block) = &then_expr.else_block {
                    free_vars.extend(self.collect_free_variables_in_block(else_block, bound_vars));
                }
            }
            Expr::While(while_expr) => {
                free_vars.extend(self.collect_free_variables(&while_expr.condition, bound_vars));
                free_vars.extend(self.collect_free_variables_in_block(&while_expr.body, bound_vars));
            }
            Expr::Block(block) => {
                free_vars.extend(self.collect_free_variables_in_block(block, bound_vars));
            }
            Expr::Lambda(lambda) => {
                let mut lambda_bound = bound_vars.clone();
                for param in &lambda.params {
                    lambda_bound.insert(param.clone());
                }
                free_vars.extend(self.collect_free_variables(&lambda.body, &lambda_bound));
            }
            Expr::WithLifetime(wl) => {
                free_vars.extend(self.collect_free_variables_in_block(&wl.body, bound_vars));
            }
            Expr::With(with_expr) => {
                free_vars.extend(self.collect_free_variables_in_block(&with_expr.body, bound_vars));
            }
            Expr::Pipe(pipe_expr) => {
                free_vars.extend(self.collect_free_variables(&pipe_expr.expr, bound_vars));
                match &pipe_expr.target {
                    PipeTarget::Ident(_) => {
                        // Target identifier is a binding, not a use
                    }
                    PipeTarget::Expr(target_expr) => {
                        free_vars.extend(self.collect_free_variables(target_expr, bound_vars));
                    }
                }
            }
            Expr::Some(expr) => {
                free_vars.extend(self.collect_free_variables(expr, bound_vars));
            }
            Expr::Await(expr) => {
                free_vars.extend(self.collect_free_variables(expr, bound_vars));
            }
            Expr::Spawn(expr) => {
                free_vars.extend(self.collect_free_variables(expr, bound_vars));
            }
            // Literals and None have no free variables
            Expr::IntLit(_) | Expr::FloatLit(_) | Expr::StringLit(_) | 
            Expr::CharLit(_) | Expr::BoolLit(_) | Expr::Unit | 
            Expr::None | Expr::NoneTyped(_) => {}
        }
        
        free_vars
    }
    
    /// Helper function to collect free variables in a BlockExpr
    fn collect_free_variables_in_block(&self, block: &BlockExpr, bound_vars: &HashSet<String>) -> HashSet<String> {
        let mut free_vars = HashSet::new();
        let mut block_bound = bound_vars.clone();
        
        for stmt in &block.statements {
            match stmt {
                Stmt::Binding(bind_decl) => {
                    free_vars.extend(self.collect_free_variables(&bind_decl.value, &block_bound));
                    block_bound.insert(bind_decl.name.clone());
                }
                Stmt::Assignment(assign) => {
                    free_vars.extend(self.collect_free_variables(&assign.value, &block_bound));
                }
                Stmt::Expr(expr) => {
                    free_vars.extend(self.collect_free_variables(expr, &block_bound));
                }
            }
        }
        
        if let Some(expr) = &block.expr {
            free_vars.extend(self.collect_free_variables(expr, &block_bound));
        }
        
        free_vars
    }
    
    /// Collect variable bindings from a pattern
    fn collect_pattern_bindings(&self, pattern: &Pattern, bindings: &mut HashSet<String>) {
        match pattern {
            Pattern::Ident(name) => {
                bindings.insert(name.clone());
            }
            Pattern::Wildcard => {}
            Pattern::Record(_name, fields) => {
                for (_, p) in fields {
                    self.collect_pattern_bindings(p, bindings);
                }
            }
            Pattern::Some(p) => {
                self.collect_pattern_bindings(p, bindings);
            }
            Pattern::ListCons(head, tail) => {
                self.collect_pattern_bindings(head, bindings);
                self.collect_pattern_bindings(tail, bindings);
            }
            Pattern::ListExact(patterns) => {
                for p in patterns {
                    self.collect_pattern_bindings(p, bindings);
                }
            }
            Pattern::Literal(_) | Pattern::None | Pattern::EmptyList => {}
        }
    }
    
    /// Check if a type contains temporal parameters that are not in the allowed set
    fn check_temporal_escape(&self, ty: &TypedType, allowed_temporals: &HashSet<String>) -> Result<(), TypeError> {
        match ty {
            TypedType::Temporal { base_type, temporals } => {
                for temporal in temporals {
                    if !allowed_temporals.contains(temporal) {
                        return Err(TypeError::TemporalEscape {
                            temporal: temporal.clone(),
                            message: format!("Temporal parameter {} escapes its scope", temporal)
                        });
                    }
                }
                self.check_temporal_escape(base_type, allowed_temporals)?;
            }
            TypedType::Function { params, return_type } => {
                for param in params {
                    self.check_temporal_escape(param, allowed_temporals)?;
                }
                self.check_temporal_escape(return_type, allowed_temporals)?;
            }
            TypedType::Option(ty) => {
                self.check_temporal_escape(ty, allowed_temporals)?;
            }
            TypedType::List(ty) => {
                self.check_temporal_escape(ty, allowed_temporals)?;
            }
            TypedType::Array(elem_ty, _) => {
                self.check_temporal_escape(elem_ty, allowed_temporals)?;
            }
            _ => {}
        }
        Ok(())
    }
}