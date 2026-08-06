//! # Type Checker Module
//!
//! Implements a hybrid affine + copy type system for Restrict Language, ensuring memory safety
//! without garbage collection. The type checker enforces that each value is used
//! at most once for resource types, while allowing multiple uses of copyable primitive types.
//!
//! ## Key Features
//!
//! - **Hybrid Type System**: Combines affine types for resources with copy semantics for primitives
//! - **Copy Semantics**: Primitive types (Int32, Int64, Boolean, Float64, Char, Unit) can be used multiple times
//! - **Affine Types**: Resource types (String, Records, Functions) can be used at most once
//! - **Type Inference**: Bidirectional type checking with inference
//! - **Generics**: Monomorphization of generic functions
//! - **Prototype Checking**: Validates clone/freeze operations
//!
//! ## Copy vs Affine Types
//!
//! **Copyable Types** (implement Copy trait):
//! - Int32, Int64, Boolean, Float64, Char, Unit
//! - Can be used multiple times without consuming the original binding
//! - Enables recursive functions like factorial to work naturally
//!
//! **Affine Types** (do not implement Copy):
//! - String (heap-allocated)
//! - Record types (complex data structures)
//! - Function types
//! - Can only be used at most once, preventing resource leaks
//! ```rust
//! use restrict_lang::type_checker::TypeChecker;
//! use restrict_lang::parser::parse_program;
//!
//! let source = "fun main: () -> Int32 = { 42 }";
//! let (remaining, program) = parse_program(source).unwrap();
//! assert!(remaining.trim().is_empty());
//!
//! let mut checker = TypeChecker::new();
//! checker.check_program(&program).unwrap();
//! ```

use crate::ast::*;
use crate::lifetime_inference::LifetimeInference;
use crate::type_constraints::{
    finalize_type, fresh_type_param_map, solve_constraints_partial_with_forms_and_initial,
    solve_constraints_with_forms_and_initial, substitute_type_params, unify as unify_constraint,
    Constraint, ConstraintKind, ConstraintOrigin, FormEnvironment,
    Substitution as ConstraintSubstitution, TypeVarGenerator, TypeVarId,
};
use std::collections::{HashMap, HashSet};
use std::fmt;

/// Type checking errors.
///
/// These errors are designed to provide clear, actionable feedback
/// about type system violations.
#[derive(Debug, PartialEq)]
pub enum TypeError {
    /// Variable not found in scope
    UndefinedVariable(String),

    /// Type mismatch between expected and actual
    TypeMismatch {
        expected: String,
        found: String,
    },

    /// Attempt to use a value that has already been consumed
    AffineViolation(String),

    /// Attempt to mutate an immutable binding
    ImmutableReassignment(String),

    /// Type name not found
    UnknownType(String),

    /// Field not found in record
    UnknownField {
        record: String,
        field: String,
    },

    /// Required field missing from record literal
    MissingField {
        record: String,
        field: String,
    },

    /// Attempt to clone a frozen (immutable) record
    CloneFrozenRecord,

    /// Attempt to freeze an already frozen record
    FreezeAlreadyFrozen,

    /// Record type not found
    UndefinedRecord(String),

    /// Function not found
    UndefinedFunction(String),

    /// Method not found for record type
    UndefinedMethod {
        method: String,
        record_type: String,
    },

    /// Wrong number of function arguments
    ArityMismatch {
        expected: usize,
        found: usize,
    },

    /// Context not available in current scope
    UnavailableContext(String),

    /// Heap-backed value escapes an arena scope
    ArenaEscape(String),

    /// Feature not yet implemented
    UnsupportedFeature(String),

    /// Type derivation constraint not satisfied
    NotDerivedFrom(String, String),

    /// Attempt to clone a sealed prototype
    CannotCloneSealed(String),

    DerivationTooDeep(usize),

    /// Temporal constraint violation
    TemporalConstraintViolation(String),

    /// Temporal variable escapes its scope
    TemporalEscape {
        temporal: String,
        message: String,
    },

    /// Invalid temporal constraint
    InvalidTemporalConstraint(String, String),

    /// Non-exhaustive patterns in match expression
    NonExhaustivePatterns {
        missing: String,
        suggestion: String,
    },

    /// Type could not be inferred without an expected type
    CannotInferType(String),

    /// Associated type projection remains unresolved after type inference
    UnresolvedProjection(String),

    /// A source-level form name was not declared.
    UndefinedForm(String),

    /// A source-level form was declared more than once, including prelude forms.
    DuplicateForm(String),

    /// A type adopted the same form more than once.
    DuplicateAdoption {
        target: String,
        form: String,
    },

    /// A required form method was omitted by an adoption.
    MissingFormMethod {
        form: String,
        method: String,
    },

    /// An adoption provided a method that is not part of its form.
    UnexpectedFormMethod {
        form: String,
        method: String,
    },

    /// A form method selector is available through more than one bound.
    AmbiguousFormMethod {
        method: String,
        forms: Vec<String>,
    },
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeError::UndefinedVariable(name) => write!(f, "Undefined variable: {name}"),
            TypeError::TypeMismatch { expected, found } => write!(
                f,
                "Type mismatch: expected {}, found {}",
                sanitize_diagnostic_text(expected),
                sanitize_diagnostic_text(found)
            ),
            TypeError::AffineViolation(name) => {
                write!(
                    f,
                    "Variable {name} has already been used (affine type violation)"
                )
            }
            TypeError::ImmutableReassignment(name) => {
                write!(f, "Cannot reassign to immutable variable {name}")
            }
            TypeError::UnknownType(name) => write!(f, "Unknown type: {name}"),
            TypeError::UnknownField { record, field } => {
                write!(f, "Unknown field {field} in record {record}")
            }
            TypeError::MissingField { record, field } => {
                write!(f, "Missing field {field} in record {record}")
            }
            TypeError::CloneFrozenRecord => write!(f, "Cannot clone a frozen record"),
            TypeError::FreezeAlreadyFrozen => write!(f, "Cannot freeze an already frozen record"),
            TypeError::UndefinedRecord(name) => write!(f, "Record {name} is not defined"),
            TypeError::UndefinedFunction(name) => write!(f, "Function {name} is not defined"),
            TypeError::UndefinedMethod {
                method,
                record_type,
            } => write!(f, "Method {method} not found for record type {record_type}"),
            TypeError::ArityMismatch { expected, found } => {
                write!(
                    f,
                    "Wrong number of arguments: expected {expected}, found {found}"
                )
            }
            TypeError::UnavailableContext(name) => {
                write!(f, "Context {name} is not available in this scope")
            }
            TypeError::ArenaEscape(ty) => write!(
                f,
                "Arena result cannot escape with heap-backed type: {}",
                sanitize_diagnostic_text(ty)
            ),
            TypeError::UnsupportedFeature(message) => {
                write!(
                    f,
                    "Unsupported feature: {}",
                    sanitize_diagnostic_text(message)
                )
            }
            TypeError::NotDerivedFrom(ty, parent) => write!(
                f,
                "Type {} is not derived from {}",
                sanitize_diagnostic_text(ty),
                sanitize_diagnostic_text(parent)
            ),
            TypeError::CannotCloneSealed(name) => write!(f, "Cannot clone sealed prototype {name}"),
            TypeError::DerivationTooDeep(depth) => {
                write!(f, "Derivation depth too deep: {depth} > 3")
            }
            TypeError::TemporalConstraintViolation(message) => write!(
                f,
                "Temporal constraint violation: {}",
                sanitize_diagnostic_text(message)
            ),
            TypeError::TemporalEscape { message, .. } => {
                write!(f, "{}", sanitize_diagnostic_text(message))
            }
            TypeError::InvalidTemporalConstraint(inner, outer) => write!(
                f,
                "Invalid temporal constraint: {} within {}",
                sanitize_diagnostic_text(inner),
                sanitize_diagnostic_text(outer)
            ),
            TypeError::NonExhaustivePatterns {
                missing,
                suggestion,
            } => write!(
                f,
                "Non-exhaustive patterns: missing {}. {}",
                sanitize_diagnostic_text(missing),
                sanitize_diagnostic_text(suggestion)
            ),
            TypeError::CannotInferType(message) => {
                let detail = sanitize_diagnostic_text(message);
                if detail.contains("recursive type") {
                    write!(f, "Cannot infer type: recursive type")
                } else if is_internal_inference_detail(&detail) {
                    if let Some(binding_name) = unresolved_binding_name(&detail) {
                        if unresolved_collection_type(&detail) {
                            write!(
                                f,
                                "Cannot infer type for binding '{binding_name}': empty list requires an expected List type or Array type. Add a collection annotation or use the binding where a concrete collection type is expected"
                            )
                        } else if unresolved_option_type(&detail) {
                            write!(
                                f,
                                "Cannot infer type for binding '{binding_name}': Option::None requires an expected Option type. Add an Option annotation or use the binding where a concrete Option type is expected"
                            )
                        } else if unresolved_result_type(&detail) {
                            write!(
                                f,
                                "Cannot infer type for binding '{binding_name}': Result::Ok/Result::Err requires an expected Result type. Add a Result annotation or use the binding where a concrete Result type is expected"
                            )
                        } else {
                            write!(
                                f,
                                "Cannot infer type for binding '{binding_name}'. Add a type annotation or use the binding where a concrete type is expected"
                            )
                        }
                    } else if let Some(context) = parenthesized_diagnostic_context(&detail) {
                        write!(
                            f,
                            "Cannot infer type at {context}. Add a type annotation or use the expression where a concrete type is expected"
                        )
                    } else {
                        write!(
                            f,
                            "Cannot infer type. Add a type annotation or use the expression where a concrete type is expected"
                        )
                    }
                } else {
                    write!(f, "Cannot infer type: {detail}")
                }
            }
            TypeError::UnresolvedProjection(message) => {
                let detail = sanitize_diagnostic_text(message);
                let base = "Cannot resolve generic collection result type. Add a concrete List/Option annotation or use the generic call in a typed context";
                if let Some(context) = parenthesized_diagnostic_context(&detail) {
                    write!(f, "{base} ({context})")
                } else {
                    write!(f, "{base}")
                }
            }
            TypeError::UndefinedForm(name) => write!(f, "Form {name} is not defined"),
            TypeError::DuplicateForm(name) => write!(f, "Form {name} is already defined"),
            TypeError::DuplicateAdoption { target, form } => {
                write!(f, "Type {target} already takes form {form}")
            }
            TypeError::MissingFormMethod { form, method } => {
                write!(f, "Missing method {method} required by form {form}")
            }
            TypeError::UnexpectedFormMethod { form, method } => {
                write!(f, "Method {method} is not declared by form {form}")
            }
            TypeError::AmbiguousFormMethod { method, forms } => write!(
                f,
                "Form method {method} is ambiguous across {}",
                forms.join(", ")
            ),
        }
    }
}

impl std::error::Error for TypeError {}

fn sanitize_diagnostic_text(message: &str) -> String {
    let message = message
        .replace("InferVar", "inference variable")
        .replace("TypeVarId", "inference variable")
        .replace("Projection", "associated type")
        .replace(" as Container.", " associated type Container::")
        .replace("as Container.", "associated type Container::");
    replace_raw_infer_ids(&message)
}

fn is_internal_inference_detail(message: &str) -> bool {
    message.contains("unknown type")
        || message.contains("inference variable")
        || message.contains("associated type")
}

fn unresolved_binding_name(message: &str) -> Option<&str> {
    let rest = message.strip_prefix("binding '")?;
    let (name, _) = rest.split_once('\'')?;
    Some(name)
}

fn unresolved_collection_type(message: &str) -> bool {
    message.contains("List<unknown type") || message.contains("Array<unknown type")
}

fn unresolved_option_type(message: &str) -> bool {
    message.contains("Option<unknown type")
}

fn unresolved_result_type(message: &str) -> bool {
    message.contains("Result<") && message.contains("unknown type")
}

fn parenthesized_diagnostic_context(message: &str) -> Option<&str> {
    let context_start = message.rfind(" (")?;
    let context = message.get(context_start + 2..message.len().checked_sub(1)?)?;
    message.ends_with(')').then_some(context)
}

fn replace_raw_infer_ids(message: &str) -> String {
    let mut sanitized = String::with_capacity(message.len());
    let mut chars = message.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '?' && chars.peek().is_some_and(|next| next.is_ascii_digit()) {
            while chars.peek().is_some_and(|next| next.is_ascii_digit()) {
                chars.next();
            }
            sanitized.push_str("unknown type");
        } else {
            sanitized.push(ch);
        }
    }

    sanitized
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArrayLength {
    Known(usize),
    AnyInternal,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedType {
    Int32,
    Int64,
    Float64,
    Boolean,
    String,
    Char,
    Unit,
    Record {
        name: String,
        type_args: Vec<TypedType>,
        frozen: bool,
        hash: Option<String>,
        parent_hash: Option<String>,
    },
    Enum {
        name: String,
    },
    Function {
        params: Vec<TypedType>,
        return_type: Box<TypedType>,
    },
    Option(Box<TypedType>),
    Result(Box<TypedType>, Box<TypedType>),
    List(Box<TypedType>),
    Array(Box<TypedType>, ArrayLength),
    TypeParam(String),   // Generic type parameter
    InferVar(TypeVarId), // Inference meta-variable for A-layer and provisional signatures
    Projection {
        base: Box<TypedType>,
        form_name: String,
        assoc_name: String,
        args: Vec<TypedType>,
    }, // Associated type projection, valid only inside A-layer inference
    Temporal {
        base_type: Box<TypedType>,
        temporals: Vec<String>,
    }, // Type with temporal parameters
}

pub fn format_typed_type(ty: &TypedType) -> String {
    match ty {
        TypedType::Int32 => "Int32".to_string(),
        TypedType::Int64 => "Int64".to_string(),
        TypedType::Float64 => "Float64".to_string(),
        TypedType::Boolean => "Boolean".to_string(),
        TypedType::String => "String".to_string(),
        TypedType::Char => "Char".to_string(),
        TypedType::Unit => "()".to_string(),
        TypedType::Record {
            name, type_args, ..
        } => {
            if type_args.is_empty() {
                name.clone()
            } else {
                let args = type_args
                    .iter()
                    .map(format_typed_type)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}<{}>", name, args)
            }
        }
        TypedType::Enum { name } => name.clone(),
        TypedType::Function {
            params,
            return_type,
        } => {
            let params = params
                .iter()
                .map(format_typed_type)
                .collect::<Vec<_>>()
                .join(", ");
            format!("({}) -> {}", params, format_typed_type(return_type))
        }
        TypedType::Option(inner) => format!("Option<{}>", format_typed_type(inner)),
        TypedType::Result(ok, err) => format!(
            "Result<{}, {}>",
            format_typed_type(ok),
            format_typed_type(err)
        ),
        TypedType::List(inner) => format!("List<{}>", format_typed_type(inner)),
        TypedType::Array(inner, size) => {
            let size = match size {
                ArrayLength::Known(size) => size.to_string(),
                ArrayLength::AnyInternal => "_".to_string(),
            };
            format!("Array<{}, {}>", format_typed_type(inner), size)
        }
        TypedType::TypeParam(name) => name.clone(),
        TypedType::InferVar(id) => id.to_string(),
        TypedType::Projection {
            base,
            form_name,
            assoc_name,
            args,
        } => {
            let args = args
                .iter()
                .map(format_typed_type)
                .collect::<Vec<_>>()
                .join(", ");
            if args.is_empty() {
                format!(
                    "{} as {}.{}",
                    format_typed_type(base),
                    form_name,
                    assoc_name
                )
            } else {
                format!(
                    "{} as {}.{}<{}>",
                    format_typed_type(base),
                    form_name,
                    assoc_name,
                    args
                )
            }
        }
        TypedType::Temporal {
            base_type,
            temporals,
        } => {
            let temporals = temporals.join(", ");
            format!("{}<{}>", format_typed_type(base_type), temporals)
        }
    }
}

fn typed_type_mismatch(expected: &TypedType, found: &TypedType) -> TypeError {
    TypeError::TypeMismatch {
        expected: format_typed_type(expected),
        found: format_typed_type(found),
    }
}

fn expected_type_mismatch(expected: impl Into<String>, found: &TypedType) -> TypeError {
    TypeError::TypeMismatch {
        expected: expected.into(),
        found: format_typed_type(found),
    }
}

fn lowercase_option_constructor_error(name: &str) -> TypeError {
    let replacement = match name {
        "some" => "qualified OSV `value Option::Some`",
        "none" => "qualified OSV `() Option::None` with an expected `Option<T>` type",
        _ => "qualified `Option::Some` or `Option::None` OSV syntax",
    };

    TypeError::UnsupportedFeature(format!(
        "lowercase `{name}` is not an Option constructor in Restrict; use {replacement}"
    ))
}

#[derive(Debug, Clone, Default)]
pub struct TypeSubstitution {
    // Maps type parameter names to concrete types
    pub substitutions: HashMap<String, TypedType>,
}

#[derive(Debug, Clone)]
pub struct TemporalConstraint {
    pub inner: String, // ~tx
    pub outer: String, // ~db (where ~tx within ~db)
}

#[derive(Debug, Clone, Default)]
pub struct TemporalContext {
    // Active temporal variables in current scope
    pub active_temporals: HashSet<String>,
    // Temporal constraints (inner within outer)
    pub constraints: Vec<TemporalConstraint>,
    // Parent scope's temporals (for nested scopes)
    pub parent_temporals: Option<Box<TemporalContext>>,
}

impl TypeSubstitution {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, type_param: String, concrete_type: TypedType) {
        self.substitutions.insert(type_param, concrete_type);
    }

    pub fn apply(&self, ty: &TypedType) -> TypedType {
        match ty {
            TypedType::TypeParam(name) => self.substitutions.get(name).unwrap_or(ty).clone(),
            TypedType::List(inner) => TypedType::List(Box::new(self.apply(inner))),
            TypedType::Array(inner, size) => TypedType::Array(Box::new(self.apply(inner)), *size),
            TypedType::Option(inner) => TypedType::Option(Box::new(self.apply(inner))),
            TypedType::Result(ok, err) => {
                TypedType::Result(Box::new(self.apply(ok)), Box::new(self.apply(err)))
            }
            TypedType::Function {
                params,
                return_type,
            } => TypedType::Function {
                params: params.iter().map(|p| self.apply(p)).collect(),
                return_type: Box::new(self.apply(return_type)),
            },
            TypedType::Record {
                name,
                type_args,
                frozen,
                hash,
                parent_hash,
            } => TypedType::Record {
                name: name.clone(),
                type_args: type_args.iter().map(|arg| self.apply(arg)).collect(),
                frozen: *frozen,
                hash: hash.clone(),
                parent_hash: parent_hash.clone(),
            },
            TypedType::Temporal {
                base_type,
                temporals,
            } => TypedType::Temporal {
                base_type: Box::new(self.apply(base_type)),
                temporals: temporals.clone(),
            },
            TypedType::Projection {
                base,
                form_name,
                assoc_name,
                args,
            } => TypedType::Projection {
                base: Box::new(self.apply(base)),
                form_name: form_name.clone(),
                assoc_name: assoc_name.clone(),
                args: args.iter().map(|arg| self.apply(arg)).collect(),
            },
            _ => ty.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct Variable {
    ty: TypedType,
    mutable: bool,
    used: bool, // For affine type checking
    pending_inference_uses: usize,
    deferred: Option<DeferredBinding>,
    deferred_alias_group_id: Option<u64>,
    was_deferred_callable: bool,
    flexible_collection_literal: bool,
}

#[derive(Debug, Clone)]
enum DeferredBinding {
    Lambda(LambdaExpr),
    BranchCallable(DeferredBranchCallable),
    GenericFunction {
        name: String,
        definition: FunctionDef,
    },
}

#[derive(Debug, Clone)]
struct DeferredGenericBranchAliasGroup {
    group_id: u64,
    placeholder: TypedType,
    function_name: String,
    definition: FunctionDef,
    members: Vec<(usize, String)>,
}

type DeferredGenericBranchAliasResolution = (Vec<(usize, String)>, TypedType);

#[derive(Debug, Clone)]
struct DeferredBranchCallable {
    candidates: Vec<DeferredCallableCandidate>,
}

#[derive(Debug, Clone)]
enum DeferredCallableCandidate {
    Lambda(DeferredLambdaCandidate),
    Typed(TypedType),
}

#[derive(Debug, Clone)]
struct DeferredLambdaCandidate {
    lambda: LambdaExpr,
    captures: Vec<(String, TypedType)>,
}

#[derive(Debug, Clone)]
struct RecordDef {
    fields: HashMap<String, TypedType>,
    field_order: Vec<String>,
    type_params: Vec<TypeParam>,
    temporal_constraints: Vec<TemporalConstraint>,
    hash: Option<String>,
    parent_hash: Option<String>,
}

#[derive(Debug, Clone)]
struct EnumDef {
    variants: HashMap<String, Option<TypedType>>,
    variant_order: Vec<String>,
}

type RecordDefSnapshot = (
    HashMap<String, TypedType>,
    Vec<TypeParam>,
    Vec<TemporalConstraint>,
    Option<String>,
    Option<String>,
);

#[derive(Debug, Clone)]
struct FunctionDef {
    params: Vec<(String, TypedType)>,
    return_type: TypedType,
    type_params: Vec<TypeParam>, // Store generic type parameters
    #[allow(dead_code)]
    temporal_constraints: Vec<TemporalConstraint>,
}

#[derive(Debug, Clone, Default)]
struct FormDef {
    methods: HashMap<String, FunctionDef>,
}

#[derive(Debug, Clone)]
pub struct CheckedFunctionSignature {
    pub params: Vec<(String, TypedType)>,
    pub return_type: TypedType,
    pub type_params: Vec<TypeParam>,
    pub temporal_constraints: Vec<TemporalConstraint>,
}

struct VariantPayloadExpectedContext<'a> {
    field_template: &'a TypedType,
    expected: Option<&'a TypedType>,
    substitution: &'a mut ConstraintSubstitution,
}

#[derive(Clone)]
pub struct TypeChecker {
    // Variable environment (stack of scopes)
    var_env: Vec<HashMap<String, Variable>>,
    // Type parameter environment (stack of scopes for generic types)
    type_param_env: Vec<HashSet<String>>,
    // Type bounds environment: type_param -> required_traits
    type_bounds_env: Vec<HashMap<String, Vec<String>>>,
    // Form bounds are behavioral evidence, not legacy trait evidence. Keeping
    // them separate prevents a source form named `Copy` from granting affine
    // Copy semantics to `<T of Copy>`.
    form_bounds_env: Vec<HashMap<String, Vec<String>>>,
    // Trait implementations: type_name -> trait_names
    trait_impls: HashMap<String, HashSet<String>>,
    // Source-visible forms, including compiler-provided prelude forms.
    forms: HashMap<String, FormDef>,
    // Closed-world source form adoptions: target type -> adopted form names.
    form_adoptions: HashMap<String, HashSet<String>>,
    // Record definitions
    records: HashMap<String, RecordDef>,
    // Closed user-defined enum definitions
    enums: HashMap<String, EnumDef>,
    // Function definitions
    functions: HashMap<String, FunctionDef>,
    // Checked expression types, keyed by stable AST node id. Built-in
    // constructor facts may start with internal inference variables and are
    // updated alongside the variable environment whenever constraints resolve.
    checked_expr_types: HashMap<NodeId, TypedType>,
    // Method implementations: record_name -> method_name -> function_def
    methods: HashMap<String, HashMap<String, FunctionDef>>,
    // Functions whose signatures were registered with a provisional return type.
    provisional_function_returns: HashSet<String>,
    // Methods whose signatures were registered with a provisional return type.
    provisional_method_returns: HashSet<(String, String)>,
    // Prototype metadata: record_name -> (hash, parent_hash, sealed)
    prototypes: HashMap<String, (String, Option<String>, bool)>,
    // Available contexts
    _contexts: Vec<String>,
    // Temporal context for tracking temporal variables and constraints
    temporal_context: TemporalContext,
    // AsyncRuntime context stack for tracking async scopes
    async_runtime_stack: Vec<String>, // Stack of async lifetime names
    // Shared A-layer inference variable generator.
    type_var_generator: TypeVarGenerator,
    // Stable provenance for deferred callable moves. Type equality cannot
    // identify aliases after inference unifies independent placeholders.
    next_deferred_alias_group_id: u64,
    // Prevent recursive finalization while a stored callable is being
    // rechecked against the concrete type inferred for its alias group.
    finalizing_deferred_alias_group_ids: HashSet<u64>,
    // Built-in form/adoption environment used by A-layer constraint solving.
    form_environment: FormEnvironment,
    // Record shapes are registered before source `takes` declarations. Bound
    // use validation therefore starts only once all adoptions are known.
    form_bound_validation_ready: bool,
    // At most one implicit `it` focus may be active. Nested scoped clauses
    // must name at least one binder explicitly so affine uses stay legible.
    implicit_focus_depth: usize,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    fn is_display_intrinsic_name(name: &str) -> bool {
        matches!(name, "display" | "print" | "println")
    }

    fn validate_unique_parameter_names(owner: &str, params: &[Param]) -> Result<(), TypeError> {
        let mut names = HashSet::new();
        for param in params {
            if !names.insert(&param.name) {
                return Err(TypeError::UnsupportedFeature(format!(
                    "duplicate parameter '{}' in {}",
                    param.name, owner
                )));
            }
        }
        Ok(())
    }

    fn display_type_param() -> TypeParam {
        TypeParam {
            name: "T".to_string(),
            bounds: vec![],
            derivation_bound: None,
            of_forms: vec!["Display".to_string()],
            is_temporal: false,
        }
    }

    pub fn new() -> Self {
        let mut checker = Self {
            var_env: vec![HashMap::new()],
            type_param_env: vec![HashSet::new()],
            type_bounds_env: vec![HashMap::new()],
            form_bounds_env: vec![HashMap::new()],
            trait_impls: HashMap::new(),
            forms: HashMap::new(),
            form_adoptions: HashMap::new(),
            records: HashMap::new(),
            enums: HashMap::new(),
            functions: HashMap::new(),
            checked_expr_types: HashMap::new(),
            methods: HashMap::new(),
            provisional_function_returns: HashSet::new(),
            provisional_method_returns: HashSet::new(),
            prototypes: HashMap::new(),
            _contexts: Vec::new(),
            temporal_context: TemporalContext::default(),
            async_runtime_stack: Vec::new(),
            type_var_generator: TypeVarGenerator::new(),
            next_deferred_alias_group_id: 0,
            finalizing_deferred_alias_group_ids: HashSet::new(),
            form_environment: FormEnvironment::new(),
            form_bound_validation_ready: false,
            implicit_focus_depth: 0,
        };

        // Register built-in functions and traits
        checker.register_builtins();
        checker.register_builtin_traits();
        checker.register_async_runtime_builtins();

        checker
    }

    pub fn checked_function_return_type(&self, name: &str) -> Option<TypedType> {
        self.functions
            .get(name)
            .map(|function| function.return_type.clone())
    }

    pub fn checked_function_signature(&self, name: &str) -> Option<CheckedFunctionSignature> {
        self.functions
            .get(name)
            .map(|function| CheckedFunctionSignature {
                params: function.params.clone(),
                return_type: function.return_type.clone(),
                type_params: function.type_params.clone(),
                temporal_constraints: function.temporal_constraints.clone(),
            })
    }

    pub fn checked_record_fields_for_type(
        &self,
        ty: &TypedType,
    ) -> Option<Vec<(String, TypedType)>> {
        let base_ty = match ty {
            TypedType::Temporal { base_type, .. } => base_type.as_ref(),
            _ => ty,
        };
        let TypedType::Record {
            name, type_args, ..
        } = base_ty
        else {
            return None;
        };

        let record_def = self.records.get(name)?;
        let bindings = Self::type_arg_bindings(&record_def.type_params, type_args);
        record_def
            .field_order
            .iter()
            .map(|field_name| {
                record_def.fields.get(field_name).map(|field_ty| {
                    (
                        field_name.clone(),
                        Self::apply_type_arg_bindings(field_ty, &bindings),
                    )
                })
            })
            .collect()
    }

    /// Return a closed enum's variants in declaration order.
    ///
    /// Backends may assign internal numeric tags from this order. Those tag
    /// values are an implementation detail and are not part of the source ABI.
    pub fn checked_enum_variants_for_type(
        &self,
        ty: &TypedType,
    ) -> Option<Vec<(String, Option<TypedType>)>> {
        let TypedType::Enum { name } = ty else {
            return None;
        };
        let enum_def = self.enums.get(name)?;
        enum_def
            .variant_order
            .iter()
            .map(|variant_name| {
                enum_def
                    .variants
                    .get(variant_name)
                    .cloned()
                    .map(|payload| (variant_name.clone(), payload))
            })
            .collect()
    }

    /// Return the finalized type recorded while checking the node with
    /// this stable id.
    ///
    /// Facts are keyed by `NodeId`, so they remain valid for any AST that
    /// carries the same numbering as the checked program: the instance
    /// itself and clones of it. A plain re-parse of the root source is NOT
    /// sufficient when the program had imports, because import resolution
    /// splices imported declarations and renumbers the whole program;
    /// querying with differently-numbered nodes silently returns facts for
    /// other nodes.
    pub fn checked_expr_type(&self, expr: &Expr) -> Option<TypedType> {
        self.checked_expr_types.get(&expr.id).cloned()
    }

    pub fn checked_expr_type_count(&self) -> usize {
        self.checked_expr_types.len()
    }

    pub fn checked_variable_type(&self, name: &str) -> Option<TypedType> {
        self.peek_var_type(name)
    }

    fn record_checked_expr_type(&mut self, expr: &Expr, ty: &TypedType) {
        if expr.id == NodeId::DUMMY {
            // Synthesized desugar nodes have no stable identity. Source
            // subexpressions cloned into desugared forms keep their original
            // ids, so their facts are recorded under the source node.
            return;
        }
        if Self::contains_inference_internal_type(ty)
            && Self::builtin_constructor_expr(expr).is_none()
        {
            return;
        }
        self.record_checked_expr_type_allow_inference(expr, ty);
    }

    fn record_checked_expr_type_allow_inference(&mut self, expr: &Expr, ty: &TypedType) {
        if expr.id == NodeId::DUMMY {
            return;
        }
        self.checked_expr_types.insert(expr.id, ty.clone());
    }

    fn range_int32_type() -> TypedType {
        TypedType::Record {
            name: "Range".to_string(),
            type_args: vec![TypedType::Int32],
            frozen: false,
            hash: None,
            parent_hash: None,
        }
    }

    fn register_builtin_traits(&mut self) {
        // Register trait implementations for built-in types

        // Int32 implements Display, Clone, Copy, Debug
        let mut int32_traits = HashSet::new();
        int32_traits.insert("Display".to_string());
        int32_traits.insert("Clone".to_string());
        int32_traits.insert("Copy".to_string());
        int32_traits.insert("Debug".to_string());
        self.trait_impls.insert("Int32".to_string(), int32_traits);

        // Int64 implements Display, Clone, Copy, Debug
        let mut int64_traits = HashSet::new();
        int64_traits.insert("Display".to_string());
        int64_traits.insert("Clone".to_string());
        int64_traits.insert("Copy".to_string());
        int64_traits.insert("Debug".to_string());
        self.trait_impls.insert("Int64".to_string(), int64_traits);

        // String implements Display, Clone, Debug (NOT Copy - strings are heap allocated)
        let mut string_traits = HashSet::new();
        string_traits.insert("Display".to_string());
        string_traits.insert("Clone".to_string());
        string_traits.insert("Debug".to_string());
        self.trait_impls.insert("String".to_string(), string_traits);

        // Boolean implements Display, Clone, Copy, Debug
        let mut bool_traits = HashSet::new();
        bool_traits.insert("Display".to_string());
        bool_traits.insert("Clone".to_string());
        bool_traits.insert("Copy".to_string());
        bool_traits.insert("Debug".to_string());
        self.trait_impls.insert("Boolean".to_string(), bool_traits);

        // Float64 implements Display, Clone, Copy, Debug
        let mut float_traits = HashSet::new();
        float_traits.insert("Display".to_string());
        float_traits.insert("Clone".to_string());
        float_traits.insert("Copy".to_string());
        float_traits.insert("Debug".to_string());
        self.trait_impls.insert("Float64".to_string(), float_traits);

        // Char implements Display, Clone, Copy, Debug
        let mut char_traits = HashSet::new();
        char_traits.insert("Display".to_string());
        char_traits.insert("Clone".to_string());
        char_traits.insert("Copy".to_string());
        char_traits.insert("Debug".to_string());
        self.trait_impls.insert("Char".to_string(), char_traits);

        // Unit implements Display, Clone, Copy, Debug
        let mut unit_traits = HashSet::new();
        unit_traits.insert("Display".to_string());
        unit_traits.insert("Clone".to_string());
        unit_traits.insert("Copy".to_string());
        unit_traits.insert("Debug".to_string());
        self.trait_impls.insert("Unit".to_string(), unit_traits);
    }

    // AsyncRuntime context management methods

    /// Enter a new AsyncRuntime context with the given lifetime
    fn enter_async_runtime(&mut self, lifetime: &str) -> Result<(), TypeError> {
        // Verify that the lifetime is in the current temporal scope
        if !self.temporal_context.active_temporals.contains(lifetime) {
            return Err(TypeError::UndefinedVariable(format!(
                "Lifetime ~{} not in scope",
                lifetime
            )));
        }

        // Push the async runtime onto the stack
        self.async_runtime_stack.push(lifetime.to_string());
        Ok(())
    }

    /// Exit the current AsyncRuntime context
    fn exit_async_runtime(&mut self) -> Result<String, TypeError> {
        self.async_runtime_stack.pop().ok_or_else(|| {
            TypeError::UnsupportedFeature("No AsyncRuntime context to exit".to_string())
        })
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
        self.functions.insert(
            "spawn".to_string(),
            FunctionDef {
                params: vec![(
                    "task".to_string(),
                    TypedType::Function {
                        params: vec![],
                        return_type: Box::new(TypedType::TypeParam("T".to_string())),
                    },
                )],
                return_type: TypedType::Temporal {
                    base_type: Box::new(TypedType::Record {
                        name: "Task".to_string(),
                        type_args: Vec::new(),
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
                    of_forms: vec![],
                    is_temporal: false,
                }],
                temporal_constraints: vec![],
            },
        );

        // await operation: Task<T, ~async> -> T
        self.functions.insert(
            "await".to_string(),
            FunctionDef {
                params: vec![(
                    "task".to_string(),
                    TypedType::Temporal {
                        base_type: Box::new(TypedType::Record {
                            name: "Task".to_string(),
                            type_args: Vec::new(),
                            frozen: false,
                            hash: None,
                            parent_hash: None,
                        }),
                        temporals: vec!["async".to_string()],
                    },
                )],
                return_type: TypedType::TypeParam("T".to_string()),
                type_params: vec![TypeParam {
                    name: "T".to_string(),
                    bounds: vec![],
                    derivation_bound: None,
                    of_forms: vec![],
                    is_temporal: false,
                }],
                temporal_constraints: vec![],
            },
        );

        // channel operation: () -> (Sender<T, ~async>, Receiver<T, ~async>)
        self.functions.insert(
            "channel".to_string(),
            FunctionDef {
                params: vec![],
                return_type: TypedType::Record {
                    name: "Channel".to_string(),
                    type_args: Vec::new(),
                    frozen: false,
                    hash: None,
                    parent_hash: None,
                },
                type_params: vec![TypeParam {
                    name: "T".to_string(),
                    bounds: vec![],
                    derivation_bound: None,
                    of_forms: vec![],
                    is_temporal: false,
                }],
                temporal_constraints: vec![],
            },
        );
    }

    fn register_builtins(&mut self) {
        // Display-polymorphic println function. Code generation resolves the
        // concrete adoption statically after generic specialization.
        self.functions.insert(
            "println".to_string(),
            FunctionDef {
                params: vec![("value".to_string(), TypedType::TypeParam("T".to_string()))],
                return_type: TypedType::Unit,
                type_params: vec![Self::display_type_param()],
                temporal_constraints: vec![],
            },
        );

        let element_type_param = TypeParam {
            name: "T".to_string(),
            bounds: vec![],
            derivation_bound: None,
            of_forms: vec![],
            is_temporal: false,
        };

        // list_length function
        self.functions.insert(
            "list_length".to_string(),
            FunctionDef {
                params: vec![(
                    "list".to_string(),
                    TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
                )],
                return_type: TypedType::Int32,
                type_params: vec![element_type_param.clone()],
                temporal_constraints: vec![],
            },
        );

        // list_get function
        self.functions.insert(
            "list_get".to_string(),
            FunctionDef {
                params: vec![
                    (
                        "list".to_string(),
                        TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
                    ),
                    ("index".to_string(), TypedType::Int32),
                ],
                return_type: TypedType::TypeParam("T".to_string()),
                type_params: vec![element_type_param.clone()],
                temporal_constraints: vec![],
            },
        );

        // array_get function
        self.functions.insert(
            "array_get".to_string(),
            FunctionDef {
                params: vec![
                    (
                        "array".to_string(),
                        TypedType::Array(
                            Box::new(TypedType::TypeParam("T".to_string())),
                            ArrayLength::AnyInternal,
                        ),
                    ),
                    ("index".to_string(), TypedType::Int32),
                ],
                return_type: TypedType::TypeParam("T".to_string()),
                type_params: vec![element_type_param.clone()],
                temporal_constraints: vec![],
            },
        );

        // array_set function
        self.functions.insert(
            "array_set".to_string(),
            FunctionDef {
                params: vec![
                    (
                        "array".to_string(),
                        TypedType::Array(
                            Box::new(TypedType::TypeParam("T".to_string())),
                            ArrayLength::AnyInternal,
                        ),
                    ),
                    ("index".to_string(), TypedType::Int32),
                    ("value".to_string(), TypedType::TypeParam("T".to_string())),
                ],
                return_type: TypedType::Unit,
                type_params: vec![element_type_param],
                temporal_constraints: vec![],
            },
        );

        // tail<T>
        let tail_type_param = TypeParam {
            name: "T".to_string(),
            bounds: vec![],
            derivation_bound: None,
            of_forms: vec![],
            is_temporal: false,
        };
        self.functions.insert(
            "tail".to_string(),
            FunctionDef {
                params: vec![(
                    "list".to_string(),
                    TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
                )],
                return_type: TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
                type_params: vec![tail_type_param],
                temporal_constraints: vec![],
            },
        );

        // Standard library functions
        self.register_std_math();
        self.register_std_list();
        self.register_std_option();
        self.register_std_io();
        self.register_std_forms();
        self.register_std_prelude();

        // Note: Arena is a built-in context but not added to _contexts by default
        // It only becomes available inside a "with Arena" block
    }

    fn register_std_math(&mut self) {
        // abs function
        self.functions.insert(
            "abs".to_string(),
            FunctionDef {
                params: vec![("x".to_string(), TypedType::Int32)],
                return_type: TypedType::Int32,
                type_params: vec![],
                temporal_constraints: vec![],
            },
        );

        // max function
        self.functions.insert(
            "max".to_string(),
            FunctionDef {
                params: vec![
                    ("a".to_string(), TypedType::Int32),
                    ("b".to_string(), TypedType::Int32),
                ],
                return_type: TypedType::Int32,
                type_params: vec![],
                temporal_constraints: vec![],
            },
        );

        // min function
        self.functions.insert(
            "min".to_string(),
            FunctionDef {
                params: vec![
                    ("a".to_string(), TypedType::Int32),
                    ("b".to_string(), TypedType::Int32),
                ],
                return_type: TypedType::Int32,
                type_params: vec![],
                temporal_constraints: vec![],
            },
        );

        // pow function
        self.functions.insert(
            "pow".to_string(),
            FunctionDef {
                params: vec![
                    ("base".to_string(), TypedType::Int32),
                    ("exp".to_string(), TypedType::Int32),
                ],
                return_type: TypedType::Int32,
                type_params: vec![],
                temporal_constraints: vec![],
            },
        );

        // factorial function
        self.functions.insert(
            "factorial".to_string(),
            FunctionDef {
                params: vec![("n".to_string(), TypedType::Int32)],
                return_type: TypedType::Int32,
                type_params: vec![],
                temporal_constraints: vec![],
            },
        );

        // Float versions
        self.functions.insert(
            "abs_f".to_string(),
            FunctionDef {
                params: vec![("x".to_string(), TypedType::Float64)],
                return_type: TypedType::Float64,
                type_params: vec![],
                temporal_constraints: vec![],
            },
        );

        self.functions.insert(
            "max_f".to_string(),
            FunctionDef {
                params: vec![
                    ("a".to_string(), TypedType::Float64),
                    ("b".to_string(), TypedType::Float64),
                ],
                return_type: TypedType::Float64,
                type_params: vec![],
                temporal_constraints: vec![],
            },
        );

        self.functions.insert(
            "min_f".to_string(),
            FunctionDef {
                params: vec![
                    ("a".to_string(), TypedType::Float64),
                    ("b".to_string(), TypedType::Float64),
                ],
                return_type: TypedType::Float64,
                type_params: vec![],
                temporal_constraints: vec![],
            },
        );
    }

    fn register_std_list(&mut self) {
        // Generic list functions
        let t_param = TypeParam {
            name: "T".to_string(),
            bounds: vec![],
            derivation_bound: None,
            of_forms: vec![],
            is_temporal: false,
        };
        // list_is_empty<T>
        self.functions.insert(
            "list_is_empty".to_string(),
            FunctionDef {
                params: vec![(
                    "list".to_string(),
                    TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
                )],
                return_type: TypedType::Boolean,
                type_params: vec![t_param.clone()],
                temporal_constraints: vec![],
            },
        );

        // list_head<T>
        self.functions.insert(
            "list_head".to_string(),
            FunctionDef {
                params: vec![(
                    "list".to_string(),
                    TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
                )],
                return_type: TypedType::Option(Box::new(TypedType::TypeParam("T".to_string()))),
                type_params: vec![t_param.clone()],
                temporal_constraints: vec![],
            },
        );

        // list_tail<T>
        self.functions.insert(
            "list_tail".to_string(),
            FunctionDef {
                params: vec![(
                    "list".to_string(),
                    TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
                )],
                return_type: TypedType::Option(Box::new(TypedType::List(Box::new(
                    TypedType::TypeParam("T".to_string()),
                )))),
                type_params: vec![t_param.clone()],
                temporal_constraints: vec![],
            },
        );

        // list_reverse<T>
        self.functions.insert(
            "list_reverse".to_string(),
            FunctionDef {
                params: vec![(
                    "list".to_string(),
                    TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
                )],
                return_type: TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
                type_params: vec![t_param.clone()],
                temporal_constraints: vec![],
            },
        );

        // list_prepend<T>
        self.functions.insert(
            "list_prepend".to_string(),
            FunctionDef {
                params: vec![
                    ("item".to_string(), TypedType::TypeParam("T".to_string())),
                    (
                        "list".to_string(),
                        TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
                    ),
                ],
                return_type: TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
                type_params: vec![t_param.clone()],
                temporal_constraints: vec![],
            },
        );

        // list_append<T>
        self.functions.insert(
            "list_append".to_string(),
            FunctionDef {
                params: vec![
                    (
                        "list".to_string(),
                        TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
                    ),
                    ("item".to_string(), TypedType::TypeParam("T".to_string())),
                ],
                return_type: TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
                type_params: vec![t_param.clone()],
                temporal_constraints: vec![],
            },
        );

        // list_concat<T>
        self.functions.insert(
            "list_concat".to_string(),
            FunctionDef {
                params: vec![
                    (
                        "a".to_string(),
                        TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
                    ),
                    (
                        "b".to_string(),
                        TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
                    ),
                ],
                return_type: TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
                type_params: vec![t_param.clone()],
                temporal_constraints: vec![],
            },
        );

        // list_count<T>
        self.functions.insert(
            "list_count".to_string(),
            FunctionDef {
                params: vec![(
                    "list".to_string(),
                    TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
                )],
                return_type: TypedType::Int32,
                type_params: vec![t_param.clone()],
                temporal_constraints: vec![],
            },
        );
    }

    fn register_std_option(&mut self) {
        let t_param = TypeParam {
            name: "T".to_string(),
            bounds: vec![],
            derivation_bound: None,
            of_forms: vec![],
            is_temporal: false,
        };

        // option_is_some<T>
        self.functions.insert(
            "option_is_some".to_string(),
            FunctionDef {
                params: vec![(
                    "opt".to_string(),
                    TypedType::Option(Box::new(TypedType::TypeParam("T".to_string()))),
                )],
                return_type: TypedType::Boolean,
                type_params: vec![t_param.clone()],
                temporal_constraints: vec![],
            },
        );

        // option_is_none<T>
        self.functions.insert(
            "option_is_none".to_string(),
            FunctionDef {
                params: vec![(
                    "opt".to_string(),
                    TypedType::Option(Box::new(TypedType::TypeParam("T".to_string()))),
                )],
                return_type: TypedType::Boolean,
                type_params: vec![t_param.clone()],
                temporal_constraints: vec![],
            },
        );

        // option_unwrap_or<T>
        self.functions.insert(
            "option_unwrap_or".to_string(),
            FunctionDef {
                params: vec![
                    (
                        "opt".to_string(),
                        TypedType::Option(Box::new(TypedType::TypeParam("T".to_string()))),
                    ),
                    ("default".to_string(), TypedType::TypeParam("T".to_string())),
                ],
                return_type: TypedType::TypeParam("T".to_string()),
                type_params: vec![t_param.clone()],
                temporal_constraints: vec![],
            },
        );
    }

    fn register_std_io(&mut self) {
        // Display-polymorphic print function.
        self.functions.insert(
            "print".to_string(),
            FunctionDef {
                params: vec![("value".to_string(), TypedType::TypeParam("T".to_string()))],
                return_type: TypedType::Unit,
                type_params: vec![Self::display_type_param()],
                temporal_constraints: vec![],
            },
        );

        // Convert a Display value to its source-level String representation.
        self.functions.insert(
            "display".to_string(),
            FunctionDef {
                params: vec![("value".to_string(), TypedType::TypeParam("T".to_string()))],
                return_type: TypedType::String,
                type_params: vec![Self::display_type_param()],
                temporal_constraints: vec![],
            },
        );

        // print_int function
        self.functions.insert(
            "print_int".to_string(),
            FunctionDef {
                params: vec![("n".to_string(), TypedType::Int32)],
                return_type: TypedType::Unit,
                type_params: vec![],
                temporal_constraints: vec![],
            },
        );

        // print_float function
        self.functions.insert(
            "print_float".to_string(),
            FunctionDef {
                params: vec![("f".to_string(), TypedType::Float64)],
                return_type: TypedType::Unit,
                type_params: vec![],
                temporal_constraints: vec![],
            },
        );

        // eprint function
        self.functions.insert(
            "eprint".to_string(),
            FunctionDef {
                params: vec![("s".to_string(), TypedType::String)],
                return_type: TypedType::Unit,
                type_params: vec![],
                temporal_constraints: vec![],
            },
        );

        // eprintln function
        self.functions.insert(
            "eprintln".to_string(),
            FunctionDef {
                params: vec![("s".to_string(), TypedType::String)],
                return_type: TypedType::Unit,
                type_params: vec![],
                temporal_constraints: vec![],
            },
        );
    }

    fn register_std_forms(&mut self) {
        let display_method = FunctionDef {
            params: vec![("self".to_string(), TypedType::TypeParam("Self".to_string()))],
            return_type: TypedType::String,
            type_params: vec![],
            temporal_constraints: vec![],
        };
        self.forms.insert(
            "Display".to_string(),
            FormDef {
                methods: HashMap::from([("display".to_string(), display_method)]),
            },
        );
        self.forms
            .insert("Container".to_string(), FormDef::default());

        self.form_environment
            .register_builtin_container_adoptions()
            .expect("standard Container form adoptions should be valid");
        for type_name in [
            "String", "Int32", "Int64", "Float64", "Boolean", "Char", "Unit",
        ] {
            self.form_environment
                .register_direct_adoption(type_name, "Display")
                .expect("standard Display adoptions should be unique");
            self.form_adoptions
                .entry(type_name.to_string())
                .or_default()
                .insert("Display".to_string());
        }
    }

    fn register_std_prelude(&mut self) {
        let c_param = TypeParam {
            name: "C".to_string(),
            bounds: vec![],
            derivation_bound: None,
            of_forms: vec!["Container".to_string()],
            is_temporal: false,
        };
        let t_param = TypeParam {
            name: "T".to_string(),
            bounds: vec![],
            derivation_bound: None,
            of_forms: vec![],
            is_temporal: false,
        };
        let u_param = TypeParam {
            name: "U".to_string(),
            bounds: vec![],
            derivation_bound: None,
            of_forms: vec![],
            is_temporal: false,
        };

        // identity<T>
        self.functions.insert(
            "identity".to_string(),
            FunctionDef {
                params: vec![("x".to_string(), TypedType::TypeParam("T".to_string()))],
                return_type: TypedType::TypeParam("T".to_string()),
                type_params: vec![t_param.clone()],
                temporal_constraints: vec![],
            },
        );

        let container_ty = TypedType::TypeParam("C".to_string());
        let container_item_ty = TypedType::Projection {
            base: Box::new(container_ty.clone()),
            form_name: "Container".to_string(),
            assoc_name: "Item".to_string(),
            args: vec![],
        };
        let mapped_container_ty = TypedType::Projection {
            base: Box::new(container_ty.clone()),
            form_name: "Container".to_string(),
            assoc_name: "Mapped".to_string(),
            args: vec![TypedType::TypeParam("U".to_string())],
        };

        // map<C: Container, U>: C, (C.Item -> U) -> C.Mapped<U>
        self.functions.insert(
            "map".to_string(),
            FunctionDef {
                params: vec![
                    ("container".to_string(), container_ty.clone()),
                    (
                        "mapper".to_string(),
                        TypedType::Function {
                            params: vec![container_item_ty.clone()],
                            return_type: Box::new(TypedType::TypeParam("U".to_string())),
                        },
                    ),
                ],
                return_type: mapped_container_ty,
                type_params: vec![c_param.clone(), u_param.clone()],
                temporal_constraints: vec![],
            },
        );

        // not function
        self.functions.insert(
            "not".to_string(),
            FunctionDef {
                params: vec![("b".to_string(), TypedType::Boolean)],
                return_type: TypedType::Boolean,
                type_params: vec![],
                temporal_constraints: vec![],
            },
        );

        // and function
        self.functions.insert(
            "and".to_string(),
            FunctionDef {
                params: vec![
                    ("a".to_string(), TypedType::Boolean),
                    ("b".to_string(), TypedType::Boolean),
                ],
                return_type: TypedType::Boolean,
                type_params: vec![],
                temporal_constraints: vec![],
            },
        );

        // or function
        self.functions.insert(
            "or".to_string(),
            FunctionDef {
                params: vec![
                    ("a".to_string(), TypedType::Boolean),
                    ("b".to_string(), TypedType::Boolean),
                ],
                return_type: TypedType::Boolean,
                type_params: vec![],
                temporal_constraints: vec![],
            },
        );

        // panic function
        self.functions.insert(
            "panic".to_string(),
            FunctionDef {
                params: vec![("message".to_string(), TypedType::String)],
                return_type: TypedType::Unit,
                type_params: vec![],
                temporal_constraints: vec![],
            },
        );

        // assert function
        self.functions.insert(
            "assert".to_string(),
            FunctionDef {
                params: vec![
                    ("condition".to_string(), TypedType::Boolean),
                    ("message".to_string(), TypedType::String),
                ],
                return_type: TypedType::Unit,
                type_params: vec![],
                temporal_constraints: vec![],
            },
        );

        // filter<C: Container>: C, (C.Item -> Boolean) -> C
        self.functions.insert(
            "filter".to_string(),
            FunctionDef {
                params: vec![
                    ("container".to_string(), container_ty.clone()),
                    (
                        "predicate".to_string(),
                        TypedType::Function {
                            params: vec![container_item_ty],
                            return_type: Box::new(TypedType::Boolean),
                        },
                    ),
                ],
                return_type: container_ty,
                type_params: vec![c_param],
                temporal_constraints: vec![],
            },
        );

        // fold<T, U>: List<T>, U, ((U, T) -> U) -> U
        self.functions.insert(
            "fold".to_string(),
            FunctionDef {
                params: vec![
                    (
                        "list".to_string(),
                        TypedType::List(Box::new(TypedType::TypeParam("T".to_string()))),
                    ),
                    ("initial".to_string(), TypedType::TypeParam("U".to_string())),
                    (
                        "reducer".to_string(),
                        TypedType::Function {
                            params: vec![
                                TypedType::TypeParam("U".to_string()),
                                TypedType::TypeParam("T".to_string()),
                            ],
                            return_type: Box::new(TypedType::TypeParam("U".to_string())),
                        },
                    ),
                ],
                return_type: TypedType::TypeParam("U".to_string()),
                type_params: vec![t_param, u_param],
                temporal_constraints: vec![],
            },
        );
    }

    fn push_scope(&mut self) {
        self.var_env.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.var_env.pop();
    }

    fn reject_unresolved_inference_in_current_scope(&self) -> Result<(), TypeError> {
        let Some(scope) = self.var_env.last() else {
            return Ok(());
        };

        let mut unresolved = scope
            .iter()
            .filter(|(_, var)| Self::contains_inference_internal_type(&var.ty))
            .collect::<Vec<_>>();
        unresolved.sort_by_key(|(left, _)| *left);

        if let Some((name, var)) = unresolved.first() {
            return Err(TypeError::CannotInferType(format!(
                "binding '{}' has unresolved type {}",
                name,
                format_typed_type(&var.ty)
            )));
        }

        let mut concrete_deferred = scope
            .iter()
            .filter_map(|(name, var)| var.deferred.as_ref().map(|deferred| (name, var, deferred)))
            .collect::<Vec<_>>();
        concrete_deferred.sort_by_key(|(name, _, _)| *name);
        for (name, var, deferred) in concrete_deferred {
            let group_id = var.deferred_alias_group_id.ok_or_else(|| {
                TypeError::CannotInferType(format!(
                    "deferred callable binding '{name}' is missing alias provenance"
                ))
            })?;
            self.validate_concrete_deferred_callable(&self.var_env, group_id, deferred, &var.ty)?;
        }

        Ok(())
    }

    fn check_branch_from_env<T, F>(
        &mut self,
        base_env: &[HashMap<String, Variable>],
        check: F,
    ) -> Result<(T, Vec<HashMap<String, Variable>>), TypeError>
    where
        F: FnOnce(&mut Self) -> Result<T, TypeError>,
    {
        self.var_env = base_env.to_vec();
        let result = check(self);
        let branch_env = self.var_env.clone();
        self.var_env = base_env.to_vec();
        result.map(|value| (value, branch_env))
    }

    fn resolve_deferred_generic_branch_alias_groups(
        &mut self,
        base_env: &[HashMap<String, Variable>],
        branch_envs: &[Vec<HashMap<String, Variable>>],
    ) -> Result<Vec<DeferredGenericBranchAliasResolution>, TypeError> {
        let mut groups = Vec::<DeferredGenericBranchAliasGroup>::new();

        for scope in base_env {
            for (binding_name, var) in scope {
                let Some(DeferredBinding::GenericFunction { name, definition }) = &var.deferred
                else {
                    continue;
                };
                let group_id = var.deferred_alias_group_id.ok_or_else(|| {
                    TypeError::CannotInferType(format!(
                        "deferred generic binding '{binding_name}' is missing alias provenance"
                    ))
                })?;

                if let Some(group) = groups.iter().find(|group| group.group_id == group_id) {
                    if group.function_name != *name {
                        return Err(TypeError::CannotInferType(
                            "generic alias group mixes distinct function definitions".to_string(),
                        ));
                    }
                    continue;
                }

                let mut members = base_env
                    .iter()
                    .enumerate()
                    .flat_map(|(member_scope_idx, member_scope)| {
                        member_scope
                            .iter()
                            .filter(move |(_, member)| {
                                member.deferred_alias_group_id == Some(group_id)
                            })
                            .map(move |(member_name, _)| (member_scope_idx, member_name.clone()))
                    })
                    .collect::<Vec<_>>();
                members.sort();

                for (member_scope_idx, member_name) in &members {
                    let member = base_env
                        .get(*member_scope_idx)
                        .and_then(|member_scope| member_scope.get(member_name))
                        .ok_or_else(|| TypeError::UndefinedVariable(member_name.clone()))?;
                    if member.ty != var.ty {
                        return Err(TypeError::CannotInferType(format!(
                            "generic alias group member '{member_name}' has inconsistent type state"
                        )));
                    }
                    if let Some(DeferredBinding::GenericFunction {
                        name: member_origin,
                        ..
                    }) = &member.deferred
                    {
                        if member_origin != name {
                            return Err(TypeError::CannotInferType(
                                "generic alias group mixes distinct function definitions"
                                    .to_string(),
                            ));
                        }
                    }
                }

                groups.push(DeferredGenericBranchAliasGroup {
                    group_id,
                    placeholder: var.ty.clone(),
                    function_name: name.clone(),
                    definition: definition.clone(),
                    members,
                });
            }
        }
        groups.sort_by_key(|group| group.group_id);

        let mut resolutions = Vec::new();
        for group in groups {
            let mut anchors = Vec::new();

            for branch_env in branch_envs {
                let mut branch_anchor: Option<TypedType> = None;
                for (scope_idx, member_name) in &group.members {
                    let branch_var = branch_env
                        .get(*scope_idx)
                        .and_then(|scope| scope.get(member_name))
                        .ok_or_else(|| {
                            TypeError::CannotInferType(format!(
                                "generic alias branch lost binding '{member_name}'"
                            ))
                        })?;
                    if Self::contains_inference_internal_type(&branch_var.ty) {
                        if branch_var.deferred_alias_group_id == Some(group.group_id)
                            && branch_var.ty == group.placeholder
                        {
                            continue;
                        }
                        return Err(TypeError::CannotInferType(format!(
                            "generic alias '{member_name}' was only partially resolved in one branch"
                        )));
                    }
                    if branch_var.deferred_alias_group_id.is_some()
                        && branch_var.deferred_alias_group_id != Some(group.group_id)
                    {
                        return Err(TypeError::CannotInferType(format!(
                            "generic alias '{member_name}' changed provenance in one branch"
                        )));
                    }

                    if let Some(previous) = &branch_anchor {
                        let mut substitution = ConstraintSubstitution::new();
                        unify_constraint(previous, &branch_var.ty, &mut substitution)?;
                        branch_anchor = Some(substitution.apply(previous)?);
                    } else {
                        branch_anchor = Some(branch_var.ty.clone());
                    }
                }

                if let Some(anchor) = branch_anchor {
                    let validated = self.instantiate_generic_function_value_from_expected(
                        &group.function_name,
                        &group.definition,
                        Some(&anchor),
                    )?;
                    let mut validation_substitution = ConstraintSubstitution::new();
                    unify_constraint(&anchor, &validated, &mut validation_substitution)?;
                    anchors.push(validation_substitution.apply(&anchor)?);
                }
            }

            let Some(mut merged_anchor) = anchors.first().cloned() else {
                continue;
            };
            for anchor in anchors.iter().skip(1) {
                let mut substitution = ConstraintSubstitution::new();
                unify_constraint(&merged_anchor, anchor, &mut substitution)?;
                merged_anchor = substitution.apply(&merged_anchor)?;
            }

            resolutions.push((group.members, merged_anchor));
        }

        Ok(resolutions)
    }

    fn validate_concrete_deferred_callable(
        &self,
        base_env: &[HashMap<String, Variable>],
        group_id: u64,
        deferred: &DeferredBinding,
        merged_ty: &TypedType,
    ) -> Result<(), TypeError> {
        let mut probe = self.clone();
        probe.var_env = base_env.to_vec();
        probe.finalizing_deferred_alias_group_ids.insert(group_id);
        let mut substitution = ConstraintSubstitution::new();
        probe.check_deferred_callable_payload_against_expected(
            deferred,
            merged_ty,
            &mut substitution,
        )?;
        Ok(())
    }

    fn merge_branch_var_usage(
        &mut self,
        base_env: Vec<HashMap<String, Variable>>,
        branch_envs: &[Vec<HashMap<String, Variable>>],
    ) -> Result<(), TypeError> {
        let generic_alias_resolutions =
            self.resolve_deferred_generic_branch_alias_groups(&base_env, branch_envs)?;
        let mut updates = Vec::new();

        for (scope_idx, scope) in base_env.iter().enumerate() {
            for (name, var) in scope {
                let branch_vars = branch_envs
                    .iter()
                    .filter_map(|env| {
                        env.get(scope_idx)
                            .and_then(|branch_scope| branch_scope.get(name))
                    })
                    .collect::<Vec<_>>();

                let branch_used = branch_vars.iter().any(|branch_var| branch_var.used);
                let mut used = var.used || branch_used;
                let mut pending_inference_uses = var.pending_inference_uses;

                if let Some(max_pending_inference_uses) = branch_vars
                    .iter()
                    .map(|branch_var| branch_var.pending_inference_uses)
                    .max()
                {
                    pending_inference_uses = pending_inference_uses.max(max_pending_inference_uses);
                }

                let generic_alias_resolution = generic_alias_resolutions
                    .iter()
                    .find(|(members, _)| {
                        members.iter().any(|(member_scope_idx, member_name)| {
                            *member_scope_idx == scope_idx && member_name == name
                        })
                    })
                    .map(|(_, resolved)| resolved.clone());
                let mut merged_ty = generic_alias_resolution
                    .clone()
                    .unwrap_or_else(|| var.ty.clone());
                if generic_alias_resolution.is_none()
                    && Self::contains_inference_internal_type(&var.ty)
                {
                    let branch_types: Vec<TypedType> = branch_vars
                        .iter()
                        .map(|branch_var| branch_var.ty.clone())
                        .collect();

                    if let Some(resolved) =
                        Self::merge_inference_branch_type(&var.ty, &branch_types)
                    {
                        merged_ty = resolved;
                    }
                }

                if Self::contains_inference_internal_type(&var.ty)
                    && !Self::contains_inference_internal_type(&merged_ty)
                {
                    if var.mutable || self.is_copyable(&merged_ty) {
                        used = false;
                        pending_inference_uses = 0;
                    } else {
                        for branch_var in &branch_vars {
                            if branch_var.pending_inference_uses > 1
                                || (branch_var.used && branch_var.pending_inference_uses > 0)
                            {
                                return Err(TypeError::AffineViolation(name.clone()));
                            }
                        }
                        if used || pending_inference_uses > 0 {
                            used = true;
                        }
                        pending_inference_uses = 0;
                    }
                }

                let clear_deferred = !Self::contains_inference_internal_type(&merged_ty)
                    && match &var.deferred {
                        Some(DeferredBinding::GenericFunction { .. }) => {
                            generic_alias_resolution.is_some()
                        }
                        Some(deferred) => {
                            let group_id = var.deferred_alias_group_id.ok_or_else(|| {
                                TypeError::CannotInferType(format!(
                                    "deferred callable binding '{name}' is missing alias provenance"
                                ))
                            })?;
                            self.validate_concrete_deferred_callable(
                                &base_env, group_id, deferred, &merged_ty,
                            )?;
                            true
                        }
                        None => false,
                    };
                let clear_alias_group = !Self::contains_inference_internal_type(&merged_ty)
                    && var.deferred_alias_group_id.is_some();
                updates.push((
                    scope_idx,
                    name.clone(),
                    merged_ty,
                    used,
                    pending_inference_uses,
                    clear_deferred,
                    clear_alias_group,
                ));
            }
        }

        self.var_env = base_env;
        for (
            scope_idx,
            name,
            merged_ty,
            used,
            pending_inference_uses,
            clear_deferred,
            clear_alias_group,
        ) in updates
        {
            if let Some(var) = self
                .var_env
                .get_mut(scope_idx)
                .and_then(|scope| scope.get_mut(&name))
            {
                var.ty = merged_ty;
                var.used = used;
                var.pending_inference_uses = pending_inference_uses;
                if clear_deferred {
                    var.deferred = None;
                }
                if clear_alias_group {
                    var.deferred_alias_group_id = None;
                }
            }
        }

        Ok(())
    }

    fn merge_inference_branch_type(
        base_ty: &TypedType,
        branch_types: &[TypedType],
    ) -> Option<TypedType> {
        let mut substitution = ConstraintSubstitution::new();

        for branch_ty in branch_types {
            unify_constraint(base_ty, branch_ty, &mut substitution).ok()?;
        }

        substitution.apply(base_ty).ok()
    }

    fn resolve_branch_result_type(
        branch_expected: &TypedType,
        branch_types: &[TypedType],
        finalize_result: bool,
    ) -> Result<(TypedType, ConstraintSubstitution), TypeError> {
        let mut substitution = ConstraintSubstitution::new();

        for branch_type in branch_types {
            unify_constraint(branch_expected, branch_type, &mut substitution)?;
        }

        let result = if finalize_result {
            finalize_type(branch_expected, &substitution)?
        } else {
            substitution.apply(branch_expected)?
        };

        Ok((result, substitution))
    }

    fn push_type_param_scope(&mut self, type_params: &[TypeParam]) {
        let mut type_param_scope = HashSet::new();
        let mut type_bounds_scope = HashMap::new();
        let mut form_bounds_scope = HashMap::new();

        for param in type_params {
            type_param_scope.insert(param.name.clone());

            // Keep legacy trait bounds separate from source form evidence.
            // A form may deliberately share a name with a trait (notably
            // `Copy`), but `T of Copy` must never make T affine-Copy.
            let bounds = param
                .bounds
                .iter()
                .map(|bound| bound.trait_name.clone())
                .collect::<Vec<_>>();
            let form_bounds = param.of_forms.clone();

            if !bounds.is_empty() {
                type_bounds_scope.insert(param.name.clone(), bounds);
            }
            if !form_bounds.is_empty() {
                form_bounds_scope.insert(param.name.clone(), form_bounds);
            }

            // Store derivation bound for later checking
            if let Some(ref parent_type) = param.derivation_bound {
                // Add derivation bound as a special constraint
                let derivation_bounds = type_bounds_scope.entry(param.name.clone()).or_default();
                derivation_bounds.push(format!("__derivation_from:{}", parent_type));
            }
        }

        self.type_param_env.push(type_param_scope);
        self.type_bounds_env.push(type_bounds_scope);
        self.form_bounds_env.push(form_bounds_scope);
    }

    fn pop_type_param_scope(&mut self) {
        self.type_param_env.pop();
        self.type_bounds_env.pop();
        self.form_bounds_env.pop();
    }

    fn is_type_param(&self, name: &str) -> bool {
        for scope in self.type_param_env.iter().rev() {
            if scope.contains(name) {
                return true;
            }
        }
        false
    }

    fn regular_type_param_names(type_params: &[TypeParam]) -> Vec<String> {
        type_params
            .iter()
            .filter(|param| !param.is_temporal)
            .map(|param| param.name.clone())
            .collect()
    }

    fn type_arg_bindings(
        type_params: &[TypeParam],
        type_args: &[TypedType],
    ) -> HashMap<String, TypedType> {
        Self::regular_type_param_names(type_params)
            .into_iter()
            .zip(type_args.iter().cloned())
            .collect()
    }

    fn apply_type_arg_bindings(ty: &TypedType, bindings: &HashMap<String, TypedType>) -> TypedType {
        match ty {
            TypedType::TypeParam(name) => bindings.get(name).cloned().unwrap_or_else(|| ty.clone()),
            TypedType::List(inner) => {
                TypedType::List(Box::new(Self::apply_type_arg_bindings(inner, bindings)))
            }
            TypedType::Array(inner, size) => TypedType::Array(
                Box::new(Self::apply_type_arg_bindings(inner, bindings)),
                *size,
            ),
            TypedType::Option(inner) => {
                TypedType::Option(Box::new(Self::apply_type_arg_bindings(inner, bindings)))
            }
            TypedType::Result(ok, err) => TypedType::Result(
                Box::new(Self::apply_type_arg_bindings(ok, bindings)),
                Box::new(Self::apply_type_arg_bindings(err, bindings)),
            ),
            TypedType::Function {
                params,
                return_type,
            } => TypedType::Function {
                params: params
                    .iter()
                    .map(|param| Self::apply_type_arg_bindings(param, bindings))
                    .collect(),
                return_type: Box::new(Self::apply_type_arg_bindings(return_type, bindings)),
            },
            TypedType::Record {
                name,
                type_args,
                frozen,
                hash,
                parent_hash,
            } => TypedType::Record {
                name: name.clone(),
                type_args: type_args
                    .iter()
                    .map(|arg| Self::apply_type_arg_bindings(arg, bindings))
                    .collect(),
                frozen: *frozen,
                hash: hash.clone(),
                parent_hash: parent_hash.clone(),
            },
            TypedType::Temporal {
                base_type,
                temporals,
            } => TypedType::Temporal {
                base_type: Box::new(Self::apply_type_arg_bindings(base_type, bindings)),
                temporals: temporals.clone(),
            },
            TypedType::Projection {
                base,
                form_name,
                assoc_name,
                args,
            } => TypedType::Projection {
                base: Box::new(Self::apply_type_arg_bindings(base, bindings)),
                form_name: form_name.clone(),
                assoc_name: assoc_name.clone(),
                args: args
                    .iter()
                    .map(|arg| Self::apply_type_arg_bindings(arg, bindings))
                    .collect(),
            },
            _ => ty.clone(),
        }
    }

    fn get_type_bounds(&self, type_param: &str) -> Vec<String> {
        for scope in self.type_bounds_env.iter().rev() {
            if let Some(bounds) = scope.get(type_param) {
                return bounds.clone();
            }
        }
        Vec::new()
    }

    fn get_form_bounds(&self, type_param: &str) -> Vec<String> {
        for scope in self.form_bounds_env.iter().rev() {
            if let Some(bounds) = scope.get(type_param) {
                return bounds.clone();
            }
        }
        Vec::new()
    }

    fn type_implements_trait(&self, ty: &TypedType, trait_name: &str) -> bool {
        match ty {
            TypedType::Int32 => self
                .trait_impls
                .get("Int32")
                .is_some_and(|traits| traits.contains(trait_name)),
            TypedType::Int64 => self
                .trait_impls
                .get("Int64")
                .is_some_and(|traits| traits.contains(trait_name)),
            TypedType::String => self
                .trait_impls
                .get("String")
                .is_some_and(|traits| traits.contains(trait_name)),
            TypedType::Boolean => self
                .trait_impls
                .get("Boolean")
                .is_some_and(|traits| traits.contains(trait_name)),
            TypedType::Float64 => self
                .trait_impls
                .get("Float64")
                .is_some_and(|traits| traits.contains(trait_name)),
            TypedType::Char => self
                .trait_impls
                .get("Char")
                .is_some_and(|traits| traits.contains(trait_name)),
            TypedType::Unit => self
                .trait_impls
                .get("Unit")
                .is_some_and(|traits| traits.contains(trait_name)),
            TypedType::TypeParam(param_name) => {
                // Check if the type parameter has the required trait bound
                self.get_type_bounds(param_name)
                    .contains(&trait_name.to_string())
            }
            TypedType::Record { name, .. } | TypedType::Enum { name } => self
                .trait_impls
                .get(name)
                .is_some_and(|traits| traits.contains(trait_name)),
            _ => false, // Other types don't implement traits for now
        }
    }

    /// Check if a type is copyable (implements the Copy trait)
    /// Copyable types can be used multiple times without consuming the original binding
    fn is_copyable(&self, ty: &TypedType) -> bool {
        match ty {
            // Base types that explicitly implement Copy
            TypedType::Int32
            | TypedType::Int64
            | TypedType::Boolean
            | TypedType::Float64
            | TypedType::Char
            | TypedType::Unit => true,
            // Composite types are copy only if all their components are copy
            TypedType::Option(inner) => self.is_copyable(inner),
            TypedType::Result(ok, err) => self.is_copyable(ok) && self.is_copyable(err),
            TypedType::Array(inner, _) => self.is_copyable(inner),
            TypedType::Enum { name } => self.enums.get(name).is_some_and(|enum_def| {
                enum_def
                    .variants
                    .values()
                    .all(|payload| payload.as_ref().is_none_or(|ty| self.is_copyable(ty)))
            }),
            // Lists are always heap-allocated, so not copyable
            TypedType::List(_) => false,
            // Strings are heap-allocated, so not copyable
            TypedType::String => false,
            // Records and functions are not copyable by default
            TypedType::Record { .. } | TypedType::Function { .. } => false,
            // Type parameters are copyable only if they have a Copy bound
            TypedType::TypeParam(param_name) => self
                .get_type_bounds(param_name)
                .contains(&"Copy".to_string()),
            // Inference-internal types should normally be finalized before this
            // query, but affine checking must never turn an incomplete inference
            // state into a compiler panic. Treat them conservatively as moves.
            TypedType::InferVar(_) | TypedType::Projection { .. } => false,
            // Temporal types are copyable if their base type is copyable
            TypedType::Temporal { base_type, .. } => self.is_copyable(base_type),
        }
    }

    fn lookup_var(&mut self, name: &str) -> Result<TypedType, TypeError> {
        // First, find the variable and extract needed info
        let mut found_var = None;
        for (scope_idx, scope) in self.var_env.iter().enumerate().rev() {
            if let Some(var) = scope.get(name) {
                found_var = Some((scope_idx, var.clone()));
                break;
            }
        }

        if let Some((scope_idx, var)) = found_var {
            // Mutable variables can be used multiple times
            if var.mutable {
                return Ok(var.ty.clone());
            }

            if Self::contains_inference_internal_type(&var.ty) {
                self.mark_var_pending_inference_use(scope_idx, name)?;
                return Ok(var.ty.clone());
            }

            // Copyable types can be used multiple times without being consumed
            if self.is_copyable(&var.ty) {
                return Ok(var.ty.clone());
            }

            // For non-copyable, immutable types: enforce affine constraint
            if var.used || var.pending_inference_uses > 0 {
                return Err(TypeError::AffineViolation(name.to_string()));
            }

            // Mark as used for affine types
            self.mark_var_used(scope_idx, name)?;
            return Ok(var.ty.clone());
        }

        Err(TypeError::UndefinedVariable(name.to_string()))
    }

    fn mark_var_used(&mut self, scope_idx: usize, name: &str) -> Result<(), TypeError> {
        let scope = self.var_env.get_mut(scope_idx).ok_or_else(|| {
            TypeError::UnsupportedFeature(
                "internal type checker scope missing while marking variable use".to_string(),
            )
        })?;
        let var = scope
            .get_mut(name)
            .ok_or_else(|| TypeError::UndefinedVariable(name.to_string()))?;
        var.used = true;
        Ok(())
    }

    fn mark_var_pending_inference_use(
        &mut self,
        scope_idx: usize,
        name: &str,
    ) -> Result<(), TypeError> {
        let scope = self.var_env.get_mut(scope_idx).ok_or_else(|| {
            TypeError::UnsupportedFeature(
                "internal type checker scope missing while marking pending inference use"
                    .to_string(),
            )
        })?;
        let var = scope
            .get_mut(name)
            .ok_or_else(|| TypeError::UndefinedVariable(name.to_string()))?;
        var.pending_inference_uses += 1;
        Ok(())
    }

    fn fresh_deferred_alias_group_id(&mut self) -> u64 {
        let id = self.next_deferred_alias_group_id;
        self.next_deferred_alias_group_id += 1;
        id
    }

    fn peek_deferred_alias_group_id(&self, name: &str) -> Option<u64> {
        self.var_env
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .and_then(|var| var.deferred_alias_group_id)
    }

    fn peek_was_deferred_callable(&self, name: &str) -> bool {
        self.var_env
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .is_some_and(|var| var.was_deferred_callable)
    }

    fn mark_var_as_deferred_origin(&mut self, name: &str) -> Result<(), TypeError> {
        for scope in self.var_env.iter_mut().rev() {
            if let Some(var) = scope.get_mut(name) {
                var.was_deferred_callable = true;
                return Ok(());
            }
        }
        Err(TypeError::UndefinedVariable(name.to_string()))
    }

    fn transfer_deferred_callable(&mut self, name: &str) -> Result<(), TypeError> {
        for scope in self.var_env.iter_mut().rev() {
            if let Some(var) = scope.get_mut(name) {
                var.deferred = None;
                return Ok(());
            }
        }
        Err(TypeError::UndefinedVariable(name.to_string()))
    }

    fn contains_inference_internal_type(ty: &TypedType) -> bool {
        match ty {
            TypedType::InferVar(_) | TypedType::Projection { .. } => true,
            TypedType::Option(inner) | TypedType::List(inner) | TypedType::Array(inner, _) => {
                Self::contains_inference_internal_type(inner)
            }
            TypedType::Result(ok, err) => {
                Self::contains_inference_internal_type(ok)
                    || Self::contains_inference_internal_type(err)
            }
            TypedType::Function {
                params,
                return_type,
            } => {
                params.iter().any(Self::contains_inference_internal_type)
                    || Self::contains_inference_internal_type(return_type)
            }
            TypedType::Temporal { base_type, .. } => {
                Self::contains_inference_internal_type(base_type)
            }
            TypedType::Record { type_args, .. } => {
                type_args.iter().any(Self::contains_inference_internal_type)
            }
            _ => false,
        }
    }

    fn reject_unresolved_return_type(
        owner_kind: &str,
        owner_name: &str,
        return_type: &TypedType,
    ) -> Result<(), TypeError> {
        if Self::contains_inference_internal_type(return_type) {
            return Err(TypeError::CannotInferType(format!(
                "{owner_kind} '{owner_name}' return type is unresolved; add an explicit return annotation"
            )));
        }

        Ok(())
    }

    fn constrain_inference_binding_from_expected(
        &mut self,
        name: &str,
        actual: &TypedType,
        expected: &TypedType,
    ) -> Result<Option<TypedType>, TypeError> {
        if let (TypedType::List(actual_elem), TypedType::Array(expected_elem, _)) =
            (actual, expected)
        {
            if Self::contains_inference_internal_type(actual_elem)
                || self.is_flexible_collection_literal(name)
            {
                let mut substitution = ConstraintSubstitution::new();
                unify_constraint(expected_elem, actual_elem, &mut substitution)?;
                let constrained_elem = substitution.apply(actual_elem)?;
                let constrained_ty =
                    TypedType::Array(Box::new(constrained_elem), ArrayLength::AnyInternal);
                self.apply_substitution_to_var_env(&substitution)?;
                self.update_var_type(name, constrained_ty.clone());
                return Ok(Some(constrained_ty));
            }
        }

        if !Self::contains_inference_internal_type(actual)
            || Self::contains_inference_internal_type(expected)
        {
            return Ok(None);
        }

        let mut substitution = ConstraintSubstitution::new();
        unify_constraint(expected, actual, &mut substitution)?;
        let constrained_ty = substitution.apply(actual)?;
        self.apply_substitution_to_var_env(&substitution)?;
        self.update_var_type(name, constrained_ty.clone());
        Ok(Some(constrained_ty))
    }

    fn apply_substitution_to_var_env(
        &mut self,
        substitution: &ConstraintSubstitution,
    ) -> Result<(), TypeError> {
        let mut updates = Vec::new();
        let mut checked_expr_updates = Vec::new();

        for (node_id, ty) in &self.checked_expr_types {
            if Self::contains_inference_internal_type(ty) {
                checked_expr_updates.push((*node_id, substitution.apply(ty)?));
            }
        }

        for (scope_idx, scope) in self.var_env.iter().enumerate() {
            for (name, var) in scope {
                if !Self::contains_inference_internal_type(&var.ty) {
                    continue;
                }

                let resolved = substitution.apply(&var.ty)?;
                let mut mark_used = false;
                let mut pending_inference_uses = var.pending_inference_uses;

                if !Self::contains_inference_internal_type(&resolved) {
                    if var.mutable || self.is_copyable(&resolved) {
                        pending_inference_uses = 0;
                    } else if var.pending_inference_uses > 0 {
                        if var.pending_inference_uses > 1 || var.used {
                            return Err(TypeError::AffineViolation(name.clone()));
                        }
                        mark_used = true;
                        pending_inference_uses = 0;
                    }
                }

                updates.push((
                    scope_idx,
                    name.clone(),
                    resolved,
                    mark_used,
                    pending_inference_uses,
                ));
            }
        }

        let mut prospective_env = self.var_env.clone();
        for (scope_idx, name, resolved, mark_used, pending_inference_uses) in &updates {
            let var = prospective_env
                .get_mut(*scope_idx)
                .and_then(|scope| scope.get_mut(name))
                .ok_or_else(|| TypeError::UndefinedVariable(name.clone()))?;
            var.ty = resolved.clone();
            var.pending_inference_uses = *pending_inference_uses;
            if *mark_used {
                var.used = true;
            }
        }

        let mut alias_groups = HashMap::<u64, Vec<(usize, String)>>::new();
        for (scope_idx, scope) in prospective_env.iter().enumerate() {
            for (name, var) in scope {
                if let Some(group_id) = var.deferred_alias_group_id {
                    alias_groups
                        .entry(group_id)
                        .or_default()
                        .push((scope_idx, name.clone()));
                }
            }
        }

        let mut group_ids = alias_groups.keys().copied().collect::<Vec<_>>();
        group_ids.sort_unstable();
        let mut finalized_group_ids = HashSet::new();
        for group_id in group_ids {
            if self.finalizing_deferred_alias_group_ids.contains(&group_id) {
                continue;
            }

            let members = alias_groups.get(&group_id).ok_or_else(|| {
                TypeError::CannotInferType(
                    "deferred callable alias group disappeared during substitution".to_string(),
                )
            })?;
            let (_, first_name) = members.first().ok_or_else(|| {
                TypeError::CannotInferType(
                    "deferred callable alias group has no live members".to_string(),
                )
            })?;
            let anchor_ty = prospective_env
                .get(members[0].0)
                .and_then(|scope| scope.get(first_name))
                .map(|var| var.ty.clone())
                .ok_or_else(|| TypeError::UndefinedVariable(first_name.clone()))?;

            for (scope_idx, member_name) in members.iter().skip(1) {
                let member_ty = prospective_env
                    .get(*scope_idx)
                    .and_then(|scope| scope.get(member_name))
                    .map(|var| &var.ty)
                    .ok_or_else(|| TypeError::UndefinedVariable(member_name.clone()))?;
                if member_ty != &anchor_ty {
                    return Err(TypeError::CannotInferType(format!(
                        "deferred callable alias group member '{member_name}' has inconsistent type state"
                    )));
                }
            }

            if Self::contains_inference_internal_type(&anchor_ty) {
                continue;
            }

            let owners = members
                .iter()
                .filter_map(|(scope_idx, member_name)| {
                    let var = prospective_env.get(*scope_idx)?.get(member_name)?;
                    var.deferred
                        .as_ref()
                        .map(|deferred| (member_name.clone(), deferred.clone()))
                })
                .collect::<Vec<_>>();
            let [(_, deferred)] = owners.as_slice() else {
                return Err(TypeError::CannotInferType(
                    "concrete deferred callable alias group must have exactly one owner"
                        .to_string(),
                ));
            };

            self.validate_concrete_deferred_callable(
                &prospective_env,
                group_id,
                deferred,
                &anchor_ty,
            )?;
            finalized_group_ids.insert(group_id);
        }

        for (scope_idx, name, resolved, mark_used, pending_inference_uses) in updates {
            if let Some(var) = self
                .var_env
                .get_mut(scope_idx)
                .and_then(|scope| scope.get_mut(&name))
            {
                var.ty = resolved;
                var.pending_inference_uses = pending_inference_uses;
                if mark_used {
                    var.used = true;
                }
            }
        }

        for (node_id, resolved) in checked_expr_updates {
            self.checked_expr_types.insert(node_id, resolved);
        }

        if !finalized_group_ids.is_empty() {
            for scope in &mut self.var_env {
                for var in scope.values_mut() {
                    if var
                        .deferred_alias_group_id
                        .is_some_and(|group_id| finalized_group_ids.contains(&group_id))
                    {
                        var.deferred = None;
                        var.deferred_alias_group_id = None;
                    }
                }
            }
        }

        Ok(())
    }

    fn update_var_type(&mut self, name: &str, ty: TypedType) -> bool {
        for scope in self.var_env.iter_mut().rev() {
            if let Some(var) = scope.get_mut(name) {
                var.ty = ty;
                var.flexible_collection_literal = false;
                return true;
            }
        }

        false
    }

    fn resolve_deferred_callable_alias_group(
        &mut self,
        name: &str,
        resolved: &TypedType,
    ) -> Result<(), TypeError> {
        let alias_group_id = self
            .var_env
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .ok_or_else(|| TypeError::UndefinedVariable(name.to_string()))?
            .deferred_alias_group_id
            .ok_or_else(|| {
                TypeError::CannotInferType(format!(
                    "deferred callable binding '{name}' is missing alias provenance"
                ))
            })?;

        let members = self
            .var_env
            .iter()
            .enumerate()
            .flat_map(|(scope_idx, scope)| {
                scope
                    .iter()
                    .filter(move |(_, var)| var.deferred_alias_group_id == Some(alias_group_id))
                    .map(move |(member_name, _)| (scope_idx, member_name.clone()))
            })
            .collect::<Vec<_>>();

        if members.is_empty() {
            return Err(TypeError::CannotInferType(format!(
                "deferred callable binding '{name}' has an empty alias group"
            )));
        }

        let resolved_is_copyable = self.is_copyable(resolved);
        for (scope_idx, member_name) in &members {
            let var = self
                .var_env
                .get(*scope_idx)
                .and_then(|scope| scope.get(member_name))
                .ok_or_else(|| TypeError::UndefinedVariable(member_name.clone()))?;
            if !var.mutable
                && !resolved_is_copyable
                && (var.pending_inference_uses > 1 || (var.used && var.pending_inference_uses > 0))
            {
                return Err(TypeError::AffineViolation(member_name.clone()));
            }
        }

        for (scope_idx, member_name) in members {
            let var = self
                .var_env
                .get_mut(scope_idx)
                .and_then(|scope| scope.get_mut(&member_name))
                .ok_or_else(|| TypeError::UndefinedVariable(member_name.clone()))?;
            if var.mutable || resolved_is_copyable {
                var.pending_inference_uses = 0;
            } else if var.pending_inference_uses == 1 {
                var.used = true;
                var.pending_inference_uses = 0;
            }
            var.ty = resolved.clone();
            var.deferred = None;
            var.deferred_alias_group_id = None;
            var.flexible_collection_literal = false;
        }

        Ok(())
    }

    fn is_flexible_collection_literal(&self, name: &str) -> bool {
        self.var_env.iter().rev().any(|scope| {
            scope
                .get(name)
                .is_some_and(|var| var.flexible_collection_literal)
        })
    }

    fn mark_flexible_collection_literal(&mut self, name: &str) {
        for scope in self.var_env.iter_mut().rev() {
            if let Some(var) = scope.get_mut(name) {
                var.flexible_collection_literal = true;
                return;
            }
        }
    }

    fn peek_deferred_callable(&self, name: &str) -> Option<DeferredBinding> {
        self.var_env
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .and_then(|var| var.deferred.clone())
    }

    fn update_direct_ident_from_substitution(
        &mut self,
        expr: &Expr,
        actual: &TypedType,
        substitution: &ConstraintSubstitution,
    ) -> Result<TypedType, TypeError> {
        let resolved = substitution.apply(actual)?;
        if let ExprKind::Ident(name) = &expr.kind {
            if Self::contains_inference_internal_type(actual) {
                self.update_var_type(name, resolved.clone());
            }
        }

        Ok(resolved)
    }

    fn check_deferred_callable_against_expected(
        &mut self,
        name: &str,
        deferred: &DeferredBinding,
        expected: &TypedType,
        substitution: &mut ConstraintSubstitution,
    ) -> Result<TypedType, TypeError> {
        let resolved = self.check_deferred_callable_payload_against_expected(
            deferred,
            expected,
            substitution,
        )?;
        self.resolve_deferred_callable_alias_group(name, &resolved)?;
        Ok(resolved)
    }

    fn check_deferred_callable_payload_against_expected(
        &mut self,
        deferred: &DeferredBinding,
        expected: &TypedType,
        substitution: &mut ConstraintSubstitution,
    ) -> Result<TypedType, TypeError> {
        let resolved = match deferred {
            DeferredBinding::Lambda(lambda) => {
                self.check_generic_lambda_arg(lambda, expected, substitution)?
            }
            DeferredBinding::BranchCallable(callable) => self
                .check_deferred_branch_callable_against_expected(
                    callable,
                    expected,
                    substitution,
                )?,
            DeferredBinding::GenericFunction {
                name, definition, ..
            } => {
                let instantiated = self.instantiate_generic_function_value_from_expected(
                    name,
                    definition,
                    Some(expected),
                )?;
                unify_constraint(expected, &instantiated, substitution)?;
                instantiated
            }
        };
        substitution.apply(&resolved)
    }

    fn check_deferred_branch_callable_against_expected(
        &mut self,
        callable: &DeferredBranchCallable,
        expected: &TypedType,
        substitution: &mut ConstraintSubstitution,
    ) -> Result<TypedType, TypeError> {
        let mut resolved = None;

        for candidate in &callable.candidates {
            let candidate_ty = self.check_deferred_callable_candidate_against_expected(
                candidate,
                expected,
                substitution,
            )?;
            if let Some(previous) = &resolved {
                unify_constraint(previous, &candidate_ty, substitution)?;
                resolved = Some(substitution.apply(previous)?);
            } else {
                resolved = Some(candidate_ty);
            }
        }

        resolved.ok_or_else(|| {
            TypeError::CannotInferType(
                "deferred branch callable has no callable candidates".to_string(),
            )
        })
    }

    fn check_deferred_callable_candidate_against_expected(
        &mut self,
        candidate: &DeferredCallableCandidate,
        expected: &TypedType,
        substitution: &mut ConstraintSubstitution,
    ) -> Result<TypedType, TypeError> {
        match candidate {
            DeferredCallableCandidate::Lambda(lambda) => self
                .check_deferred_lambda_candidate_against_expected(lambda, expected, substitution),
            DeferredCallableCandidate::Typed(ty) => {
                unify_constraint(expected, ty, substitution)?;
                substitution.apply(ty)
            }
        }
    }

    fn check_deferred_lambda_candidate_against_expected(
        &mut self,
        candidate: &DeferredLambdaCandidate,
        expected: &TypedType,
        substitution: &mut ConstraintSubstitution,
    ) -> Result<TypedType, TypeError> {
        self.push_scope();
        let result = {
            for (name, ty) in &candidate.captures {
                if let Err(err) = self.bind_var(name.clone(), ty.clone(), false) {
                    self.pop_scope();
                    return Err(err);
                }
            }
            self.check_generic_lambda_arg(&candidate.lambda, expected, substitution)
        };
        self.pop_scope();
        result
    }

    fn named_function_value_type(
        &mut self,
        name: &str,
        func_def: &FunctionDef,
        expected: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        if Self::is_display_intrinsic_name(name) {
            return Err(TypeError::UnsupportedFeature(format!(
                "compiler intrinsic '{name}' cannot be used as a first-class function value in the initial Display slice; call it directly with OSV syntax"
            )));
        }
        if !func_def.type_params.is_empty() {
            return self.instantiate_generic_function_value_from_expected(name, func_def, expected);
        }

        Ok(TypedType::Function {
            params: func_def.params.iter().map(|(_, ty)| ty.clone()).collect(),
            return_type: Box::new(func_def.return_type.clone()),
        })
    }

    fn instantiate_generic_function_value_from_expected(
        &mut self,
        name: &str,
        func_def: &FunctionDef,
        expected: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        let expected_func = match expected {
            Some(TypedType::Function {
                params,
                return_type,
            }) => Some((params, return_type)),
            Some(other) => {
                return Err(expected_type_mismatch("function", other));
            }
            None => None,
        };

        if let Some((expected_params, _)) = expected_func {
            if expected_params.len() != func_def.params.len() {
                return Err(TypeError::ArityMismatch {
                    expected: func_def.params.len(),
                    found: expected_params.len(),
                });
            }
        }

        let type_param_names: Vec<String> = func_def
            .type_params
            .iter()
            .map(|param| param.name.clone())
            .collect();
        let type_vars = fresh_type_param_map(&type_param_names, &mut self.type_var_generator);
        let instantiated_params: Vec<TypedType> = func_def
            .params
            .iter()
            .map(|(_, ty)| substitute_type_params(ty, &type_vars))
            .collect();
        let instantiated_return = substitute_type_params(&func_def.return_type, &type_vars);

        let Some((expected_params, expected_return)) = expected_func else {
            return Ok(TypedType::Function {
                params: instantiated_params,
                return_type: Box::new(instantiated_return),
            });
        };

        if expected_params.len() != instantiated_params.len() {
            return Err(TypeError::ArityMismatch {
                expected: instantiated_params.len(),
                found: expected_params.len(),
            });
        }

        let mut substitution = ConstraintSubstitution::new();
        let mut constraints = Vec::new();
        for type_param in &func_def.type_params {
            for form_name in self.form_bound_names(type_param) {
                if let Some(ty) = type_vars.get(&type_param.name) {
                    constraints.push(Constraint::HasForm {
                        ty: ty.clone(),
                        form_name,
                        origin: Self::constraint_origin(ConstraintKind::FormBound {
                            type_param: type_param.name.clone(),
                        }),
                    });
                }
            }
        }

        let instantiated_params: Vec<TypedType> = instantiated_params
            .into_iter()
            .enumerate()
            .map(|(idx, ty)| {
                self.lower_associated_type_projections(
                    ty,
                    &mut constraints,
                    Self::constraint_origin(ConstraintKind::Argument {
                        func_name: name.to_string(),
                        arg_index: idx,
                    }),
                )
            })
            .collect();
        let instantiated_return = self.lower_associated_type_projections(
            instantiated_return,
            &mut constraints,
            Self::constraint_origin(ConstraintKind::ReturnAnnotation {
                var_name: name.to_string(),
            }),
        );

        for (idx, (instantiated, expected)) in instantiated_params
            .iter()
            .zip(expected_params.iter())
            .enumerate()
        {
            self.solve_type_constraint(
                &mut constraints,
                &mut substitution,
                instantiated.clone(),
                expected.clone(),
                Self::constraint_origin(ConstraintKind::Argument {
                    func_name: name.to_string(),
                    arg_index: idx,
                }),
            )?;
        }

        self.solve_type_constraint(
            &mut constraints,
            &mut substitution,
            instantiated_return.clone(),
            expected_return.as_ref().clone(),
            Self::constraint_origin(ConstraintKind::ReturnAnnotation {
                var_name: name.to_string(),
            }),
        )?;

        substitution = self.solve_constraints_with_current_forms(&constraints, &substitution)?;
        let params = instantiated_params
            .iter()
            .map(|param| finalize_type(param, &substitution))
            .collect::<Result<Vec<_>, _>>()?;
        let return_type = finalize_type(&instantiated_return, &substitution)?;

        Ok(TypedType::Function {
            params,
            return_type: Box::new(return_type),
        })
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

    fn ensure_residual_record_type(
        &mut self,
        record_name: &str,
        fields: &[(String, Pattern)],
        source_ty: &TypedType,
    ) -> Result<TypedType, TypeError> {
        let extracted: HashSet<String> = fields
            .iter()
            .map(|(field_name, _)| field_name.clone())
            .collect();
        let (actual_name, instantiated_fields) = self.instantiated_record_fields(source_ty)?;
        if actual_name != record_name {
            return Err(TypeError::TypeMismatch {
                expected: record_name.to_string(),
                found: actual_name,
            });
        }

        let mut remaining_fields = HashMap::new();
        for (field_name, field_ty) in instantiated_fields {
            if !extracted.contains(&field_name) {
                if !self.is_copyable(&field_ty) {
                    return Err(TypeError::UnsupportedFeature(format!(
                        "record rest would implicitly copy non-copy field {record_name}.{field_name} of type {}; extract the field explicitly",
                        format_typed_type(&field_ty)
                    )));
                }
                remaining_fields.insert(field_name.clone(), field_ty.clone());
            }
        }

        let mut remaining_names: Vec<String> = remaining_fields.keys().cloned().collect();
        remaining_names.sort();
        let residual_name = Self::residual_record_name(record_name, &remaining_names);

        if !self.records.contains_key(&residual_name) {
            self.records.insert(
                residual_name.clone(),
                RecordDef {
                    fields: remaining_fields,
                    field_order: remaining_names,
                    type_params: vec![],
                    temporal_constraints: vec![],
                    hash: None,
                    parent_hash: None,
                },
            );
        }

        Ok(TypedType::Record {
            name: residual_name,
            type_args: Vec::new(),
            frozen: false,
            hash: None,
            parent_hash: None,
        })
    }

    fn peek_var_type(&self, name: &str) -> Option<TypedType> {
        self.var_env
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).map(|var| var.ty.clone()))
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

    fn instantiated_record_fields(
        &self,
        ty: &TypedType,
    ) -> Result<(String, HashMap<String, TypedType>), TypeError> {
        let base_ty = match ty {
            TypedType::Temporal { base_type, .. } => base_type.as_ref(),
            _ => ty,
        };

        let TypedType::Record {
            name, type_args, ..
        } = base_ty
        else {
            return Err(expected_type_mismatch("record", ty));
        };

        let record_def = self
            .records
            .get(name)
            .ok_or_else(|| TypeError::UndefinedRecord(name.clone()))?;
        let bindings = Self::type_arg_bindings(&record_def.type_params, type_args);
        let fields = record_def
            .fields
            .iter()
            .map(|(field_name, field_ty)| {
                (
                    field_name.clone(),
                    Self::apply_type_arg_bindings(field_ty, &bindings),
                )
            })
            .collect();

        Ok((name.clone(), fields))
    }

    fn bind_var(&mut self, name: String, ty: TypedType, mutable: bool) -> Result<(), TypeError> {
        self.bind_var_with_deferred(name, ty, mutable, None, None)
    }

    fn bind_var_with_deferred(
        &mut self,
        name: String,
        ty: TypedType,
        mutable: bool,
        deferred: Option<DeferredBinding>,
        deferred_alias_group_id: Option<u64>,
    ) -> Result<(), TypeError> {
        if deferred.is_some() != deferred_alias_group_id.is_some() {
            return Err(TypeError::UnsupportedFeature(
                "internal deferred callable binding is missing alias provenance".to_string(),
            ));
        }
        let was_deferred_callable = deferred.is_some();
        let current_scope = self.var_env.last_mut().ok_or_else(|| {
            TypeError::UnsupportedFeature(
                "internal type checker scope stack is empty while binding variable".to_string(),
            )
        })?;
        current_scope.insert(
            name,
            Variable {
                ty,
                mutable,
                used: false,
                pending_inference_uses: 0,
                deferred,
                deferred_alias_group_id,
                was_deferred_callable,
                flexible_collection_literal: false,
            },
        );
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

    fn reject_deferred_alias_reassignment(&self, name: &str) -> Result<(), TypeError> {
        let var = self
            .var_env
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .ok_or_else(|| TypeError::UndefinedVariable(name.to_string()))?;
        if var.was_deferred_callable {
            return Err(TypeError::UnsupportedFeature(format!(
                "reassignment of deferred-origin callable binding '{name}' is not supported in this release"
            )));
        }
        Ok(())
    }

    fn reassign_var(&mut self, name: &str, ty: &TypedType) -> Result<(), TypeError> {
        self.reject_deferred_alias_reassignment(name)?;
        // Find the variable and check if it's mutable
        for scope in self.var_env.iter_mut().rev() {
            if let Some(var) = scope.get_mut(name) {
                if !var.mutable {
                    return Err(TypeError::ImmutableReassignment(name.to_string()));
                }
                if &var.ty != ty {
                    return Err(typed_type_mismatch(&var.ty, ty));
                }
                // Concrete assignment replaces any non-deferred compile-time
                // alias metadata left by earlier lowering phases.
                var.deferred = None;
                var.deferred_alias_group_id = None;
                // Don't mark as used for reassignment
                return Ok(());
            }
        }
        Err(TypeError::UndefinedVariable(name.to_string()))
    }

    /// Helper method to find a record by its hash in the prototype chain
    fn find_record_by_hash(&self, hash: &str) -> Option<String> {
        // Look through all records to find one with the matching hash
        // This is O(n) but works for the current implementation
        for (record_name, record_def) in &self.records {
            if let Some(record_hash) = &record_def.hash {
                if record_hash == hash {
                    return Some(record_name.clone());
                }
            }
        }
        None
    }

    /// Resolve a method call with arguments (for OSV syntax: obj.method(args))
    /// This is different from resolve_method as it also validates the arguments
    fn resolve_method_call(
        &mut self,
        obj_ty: &TypedType,
        method_name: &str,
        args: &[Box<Expr>],
    ) -> Result<TypedType, TypeError> {
        match obj_ty {
            TypedType::Record {
                name,
                hash: _,
                parent_hash,
                ..
            } => {
                // First, try to find the method in this record's methods
                // Clone the method info to avoid borrow issues
                let method_info = if let Some(method_map) = self.methods.get(name) {
                    method_map.get(method_name).cloned()
                } else {
                    None
                };

                if let Some(method_info) = method_info {
                    // Method signature includes 'self' parameter, so we need to skip it
                    let method_params =
                        if !method_info.params.is_empty() && method_info.params[0].0 == "self" {
                            &method_info.params[1..] // Skip self parameter
                        } else {
                            &method_info.params[..] // No self parameter (shouldn't happen for methods)
                        };

                    // Check arity (excluding self)
                    if args.len() != method_params.len() {
                        return Err(TypeError::ArityMismatch {
                            expected: method_params.len(),
                            found: args.len(),
                        });
                    }

                    // Check argument types
                    for (i, arg) in args.iter().enumerate() {
                        let expected_ty = &method_params[i].1;
                        let actual_ty = self.check_expr_with_expected(arg, Some(expected_ty))?;
                        if &actual_ty != expected_ty {
                            return Err(typed_type_mismatch(expected_ty, &actual_ty));
                        }
                    }

                    return Ok(method_info.return_type.clone());
                }

                // If not found and this record has a parent, try the prototype chain
                if let Some(parent_hash) = parent_hash {
                    if let Some(parent_record) = self.find_record_by_hash(parent_hash) {
                        let parent_ty = TypedType::Record {
                            name: parent_record.clone(),
                            type_args: Vec::new(),
                            frozen: false,
                            hash: None,
                            parent_hash: None,
                        };
                        return self.resolve_method_call(&parent_ty, method_name, args);
                    }
                }

                Err(TypeError::UndefinedMethod {
                    method: method_name.to_string(),
                    record_type: name.clone(),
                })
            }
            _ => Err(expected_type_mismatch("record type", obj_ty)),
        }
    }

    fn peek_method_receiver_type(&self, expr: &Expr) -> Option<TypedType> {
        match &expr.kind {
            ExprKind::Ident(name) => self.peek_var_type(name),
            ExprKind::RecordLit(record_lit) if self.records.contains_key(&record_lit.name) => {
                Some(TypedType::Record {
                    name: record_lit.name.clone(),
                    type_args: Vec::new(),
                    frozen: false,
                    hash: None,
                    parent_hash: None,
                })
            }
            ExprKind::Call(call) => self.peek_named_call_return_type(call),
            ExprKind::Pipe(pipe) => self.peek_pipe_return_type(pipe),
            _ => None,
        }
    }

    /// Resolve a receiver type without changing affine-use state in the real
    /// checker. Simple receiver shapes use the cheap syntactic path above;
    /// richer expressions are checked against an isolated snapshot. If the
    /// expression is a method receiver, the ordinary call path checks it once
    /// against the selected method signature and records the real state.
    fn infer_method_receiver_type_non_consuming(&self, expr: &Expr) -> Option<TypedType> {
        self.peek_method_receiver_type(expr).or_else(|| {
            let mut probe = self.clone();
            probe.check_expr(expr).ok()
        })
    }

    fn peek_named_call_return_type(&self, call: &CallExpr) -> Option<TypedType> {
        let ExprKind::Ident(name) = &call.function.kind else {
            return None;
        };

        if let Some(TypedType::Function {
            params,
            return_type,
        }) = self.peek_var_type(name)
        {
            if params.len() == call.args.len() {
                return Some(*return_type);
            }
        }

        let mut method_probe = self.clone();
        if let Ok(Some(return_type)) = method_probe.check_osv_method_call(name, &call.args) {
            return Some(return_type);
        }

        let func_info = self.functions.get(name)?;
        if self.provisional_function_returns.contains(name) {
            return None;
        }
        if func_info.params.len() != call.args.len() {
            return None;
        }

        Some(func_info.return_type.clone())
    }

    fn peek_pipe_return_type(&self, pipe: &PipeExpr) -> Option<TypedType> {
        match &pipe.target {
            PipeTarget::Ident(name) => self.peek_pipe_ident_return_type(name, &pipe.expr),
            PipeTarget::Expr(target) => match &target.kind {
                ExprKind::Ident(name) => self.peek_pipe_ident_return_type(name, &pipe.expr),
                _ => None,
            },
        }
    }

    fn peek_pipe_ident_return_type(&self, name: &str, receiver: &Expr) -> Option<TypedType> {
        let call = CallExpr {
            function: Box::new(Expr::new(ExprKind::Ident(name.to_string()))),
            args: vec![Box::new(receiver.clone())],
        };

        match self.peek_var_type(name) {
            Some(TypedType::Function { .. }) => return self.peek_named_call_return_type(&call),
            Some(_) => return None,
            None => {}
        }

        if self.receiver_has_form_method_selector(receiver, name) {
            return self.peek_named_call_return_type(&call);
        }

        let function = self.functions.get(name)?;
        if self.provisional_function_returns.contains(name) || function.params.len() != 1 {
            return None;
        }
        Some(function.return_type.clone())
    }

    fn check_osv_method_call(
        &mut self,
        method_name: &str,
        args: &[Box<Expr>],
    ) -> Result<Option<TypedType>, TypeError> {
        let Some(receiver) = args.first() else {
            return Ok(None);
        };

        let Some(receiver_ty) = self.infer_method_receiver_type_non_consuming(receiver) else {
            return Ok(None);
        };

        if let TypedType::TypeParam(param_name) = &receiver_ty {
            let mut candidates = self
                .get_form_bounds(param_name)
                .into_iter()
                .filter_map(|form_name| {
                    self.forms
                        .get(&form_name)
                        .and_then(|form| form.methods.get(method_name))
                        .cloned()
                        .map(|method| (form_name, method))
                })
                .collect::<Vec<_>>();
            candidates.sort_by(|left, right| left.0.cmp(&right.0));
            candidates.dedup_by(|left, right| left.0 == right.0);

            if candidates.len() > 1 {
                return Err(TypeError::AmbiguousFormMethod {
                    method: method_name.to_string(),
                    forms: candidates.into_iter().map(|(name, _)| name).collect(),
                });
            }
            if let Some((_, mut method)) = candidates.pop() {
                method.params = method
                    .params
                    .into_iter()
                    .map(|(name, ty)| (name, Self::substitute_form_self(&ty, &receiver_ty)))
                    .collect();
                method.return_type = Self::substitute_form_self(&method.return_type, &receiver_ty);
                return self
                    .check_resolved_method_call(method_name, &method, args)
                    .map(Some);
            }
            return Ok(None);
        }

        let record_name = match &receiver_ty {
            TypedType::Record { name, .. } => name.clone(),
            TypedType::Temporal { base_type, .. } => match base_type.as_ref() {
                TypedType::Record { name, .. } => name.clone(),
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };

        let Some(method_info) = self
            .methods
            .get(&record_name)
            .and_then(|method_map| method_map.get(method_name))
            .cloned()
        else {
            return Ok(None);
        };

        if self
            .provisional_method_returns
            .contains(&(record_name.clone(), method_name.to_string()))
        {
            return Err(TypeError::CannotInferType(format!(
                "method '{}' for record '{}' is used before its return type has been inferred; add an explicit return annotation",
                method_name, record_name
            )));
        }

        self.check_resolved_method_call(method_name, &method_info, args)
            .map(Some)
    }

    fn check_resolved_method_call(
        &mut self,
        method_name: &str,
        method_info: &FunctionDef,
        args: &[Box<Expr>],
    ) -> Result<TypedType, TypeError> {
        if args.len() != method_info.params.len() {
            return Err(TypeError::ArityMismatch {
                expected: method_info.params.len(),
                found: args.len(),
            });
        }

        if !method_info.type_params.is_empty() {
            let call = CallExpr {
                function: Box::new(Expr::new(ExprKind::Ident(method_name.to_string()))),
                args: args.to_vec(),
            };
            return self.check_function_call_with_inference(method_info, &call, None);
        }

        for (i, arg) in args.iter().enumerate() {
            let expected_ty = &method_info.params[i].1;
            let actual_ty = self.check_expr_with_expected(arg, Some(expected_ty))?;
            if !self.type_matches_expected(expected_ty, &actual_ty) {
                return Err(typed_type_mismatch(expected_ty, &actual_ty));
            }
        }

        Ok(method_info.return_type.clone())
    }

    fn receiver_has_method_selector(&self, receiver: &Expr, method_name: &str) -> bool {
        let Some(receiver_ty) = self.infer_method_receiver_type_non_consuming(receiver) else {
            return false;
        };
        match receiver_ty {
            TypedType::Record { name, .. } => self
                .methods
                .get(&name)
                .is_some_and(|methods| methods.contains_key(method_name)),
            TypedType::Temporal { base_type, .. } => match *base_type {
                TypedType::Record { name, .. } => self
                    .methods
                    .get(&name)
                    .is_some_and(|methods| methods.contains_key(method_name)),
                _ => false,
            },
            TypedType::TypeParam(name) => {
                self.get_form_bounds(&name).into_iter().any(|form_name| {
                    self.forms
                        .get(&form_name)
                        .is_some_and(|form| form.methods.contains_key(method_name))
                })
            }
            _ => false,
        }
    }

    fn receiver_has_form_method_selector(&self, receiver: &Expr, method_name: &str) -> bool {
        let Some(receiver_ty) = self.infer_method_receiver_type_non_consuming(receiver) else {
            return false;
        };

        let adopted_form_has_method = |record_name: &str| {
            self.form_adoptions
                .get(record_name)
                .into_iter()
                .flatten()
                .any(|form_name| {
                    self.forms
                        .get(form_name)
                        .is_some_and(|form| form.methods.contains_key(method_name))
                })
        };

        match receiver_ty {
            TypedType::Record { name, .. } => adopted_form_has_method(&name),
            TypedType::Temporal { base_type, .. } => match *base_type {
                TypedType::Record { name, .. } => adopted_form_has_method(&name),
                _ => false,
            },
            TypedType::TypeParam(name) => {
                self.get_form_bounds(&name).into_iter().any(|form_name| {
                    self.forms
                        .get(&form_name)
                        .is_some_and(|form| form.methods.contains_key(method_name))
                })
            }
            _ => false,
        }
    }

    fn convert_type(&mut self, ty: &Type) -> Result<TypedType, TypeError> {
        match ty {
            Type::Named(name) => match name.as_str() {
                "Int32" => Ok(TypedType::Int32),
                "Int64" => Ok(TypedType::Int64),
                "Float64" => Ok(TypedType::Float64),
                "Boolean" => Ok(TypedType::Boolean),
                "String" => Ok(TypedType::String),
                "Char" => Ok(TypedType::Char),
                "Unit" => Ok(TypedType::Unit),
                "Int" => Err(TypeError::UnknownType("`Int`; use `Int32`".to_string())),
                "Float" => Err(TypeError::UnknownType("`Float`; use `Float64`".to_string())),
                "Bool" => Err(TypeError::UnknownType("`Bool`; use `Boolean`".to_string())),
                _ => {
                    // Check if it's a type parameter
                    if self.is_type_param(name) {
                        // For now, represent type parameters as a special TypedType
                        // In a full implementation, we'd need a TypeParam variant
                        Ok(TypedType::TypeParam(name.clone()))
                    }
                    // Check if it's a record type
                    else if self.records.contains_key(name) {
                        let ty = TypedType::Record {
                            name: name.clone(),
                            type_args: Vec::new(),
                            frozen: false,
                            hash: None,
                            parent_hash: None,
                        };
                        if self.form_bound_validation_ready {
                            self.validate_record_type_form_bounds(&ty)?;
                        }
                        Ok(ty)
                    } else if self.enums.contains_key(name) {
                        Ok(TypedType::Enum { name: name.clone() })
                    } else {
                        Err(TypeError::UnknownType(name.clone()))
                    }
                }
            },
            Type::Generic(name, params) => match name.as_str() {
                "Option" if params.len() == 1 => {
                    Ok(TypedType::Option(Box::new(self.convert_type(&params[0])?)))
                }
                "Result" if params.len() == 2 => Ok(TypedType::Result(
                    Box::new(self.convert_type(&params[0])?),
                    Box::new(self.convert_type(&params[1])?),
                )),
                "List" if params.len() == 1 => {
                    Ok(TypedType::List(Box::new(self.convert_type(&params[0])?)))
                }
                "Range" if params.len() == 1 => {
                    let elem_type = self.convert_type(&params[0])?;
                    if elem_type == TypedType::Int32 {
                        Ok(Self::range_int32_type())
                    } else {
                        Err(TypeError::UnsupportedFeature(
                            "Range<T> currently supports Int32 endpoints only".to_string(),
                        ))
                    }
                }
                "Array" if params.len() == 1 => Err(TypeError::UnknownType(
                    "Array type requires explicit length: use Array<T, N>".to_string(),
                )),
                "Array" if params.len() == 2 => {
                    let elem_type = self.convert_type(&params[0])?;
                    let size = match &params[1] {
                        Type::Named(size) => size.parse::<usize>().map_err(|_| {
                            TypeError::UnknownType(format!(
                                "Array length must be a non-negative integer literal, got {}",
                                size
                            ))
                        })?,
                        _ => {
                            return Err(TypeError::UnknownType(
                                "Array length must be a non-negative integer literal".to_string(),
                            ));
                        }
                    };
                    Ok(TypedType::Array(
                        Box::new(elem_type),
                        ArrayLength::Known(size),
                    ))
                }
                _ if self.records.contains_key(name) => {
                    let record_def = self
                        .records
                        .get(name)
                        .ok_or_else(|| TypeError::UnknownType(name.clone()))?;
                    let regular_param_count = record_def
                        .type_params
                        .iter()
                        .filter(|param| !param.is_temporal)
                        .count();
                    if params.len() != regular_param_count {
                        return Err(TypeError::UnknownType(format!(
                            "{}<{}>",
                            name,
                            params.len()
                        )));
                    }

                    let ty = TypedType::Record {
                        name: name.clone(),
                        type_args: params
                            .iter()
                            .map(|param| self.convert_type(param))
                            .collect::<Result<Vec<_>, _>>()?,
                        frozen: false,
                        hash: None,
                        parent_hash: None,
                    };
                    if self.form_bound_validation_ready {
                        self.validate_record_type_form_bounds(&ty)?;
                    }
                    Ok(ty)
                }
                _ if self.enums.contains_key(name) => Err(TypeError::UnsupportedFeature(format!(
                    "generic enum types are not supported: {}<{}>",
                    name,
                    params.len()
                ))),
                _ => Err(TypeError::UnknownType(format!(
                    "{}<{}>",
                    name,
                    params.len()
                ))),
            },
            Type::Function(params, return_type) => Ok(TypedType::Function {
                params: params
                    .iter()
                    .map(|param| self.convert_type(param))
                    .collect::<Result<Vec<_>, _>>()?,
                return_type: Box::new(self.convert_type(return_type)?),
            }),
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

    fn register_enum_header(&mut self, enum_decl: &EnumDecl) -> Result<(), TypeError> {
        const BUILTIN_TYPES: &[&str] = &[
            "Int32", "Int64", "Float64", "Boolean", "String", "Char", "Unit", "Option", "Result",
            "List", "Range", "Array",
        ];

        if enum_decl.variants.is_empty() {
            return Err(TypeError::UnsupportedFeature(format!(
                "enum '{}' must declare at least one variant",
                enum_decl.name
            )));
        }
        if BUILTIN_TYPES.contains(&enum_decl.name.as_str())
            || self.enums.contains_key(&enum_decl.name)
            || self.records.contains_key(&enum_decl.name)
            || self.forms.contains_key(&enum_decl.name)
        {
            return Err(TypeError::UnsupportedFeature(format!(
                "duplicate type declaration '{}'",
                enum_decl.name
            )));
        }

        let mut variant_names = HashSet::new();
        let mut variants = HashMap::new();
        let mut variant_order = Vec::with_capacity(enum_decl.variants.len());
        for variant in &enum_decl.variants {
            if !variant_names.insert(variant.name.clone()) {
                return Err(TypeError::UnsupportedFeature(format!(
                    "duplicate variant '{}::{}'",
                    enum_decl.name, variant.name
                )));
            }
            variants.insert(variant.name.clone(), None);
            variant_order.push(variant.name.clone());
        }

        self.enums.insert(
            enum_decl.name.clone(),
            EnumDef {
                variants,
                variant_order,
            },
        );
        Ok(())
    }

    fn validate_enum_payload_source_type(
        &self,
        enum_name: &str,
        variant_name: &str,
        ty: &Type,
    ) -> Result<(), TypeError> {
        match ty {
            Type::Function(_, _) => Err(TypeError::UnsupportedFeature(format!(
                "enum variant '{}::{}' cannot carry a function payload",
                enum_name, variant_name
            ))),
            Type::Temporal(_, _) => Err(TypeError::UnsupportedFeature(format!(
                "enum variant '{}::{}' cannot carry a temporal payload",
                enum_name, variant_name
            ))),
            Type::Generic(name, args) => {
                if self.enums.contains_key(name) {
                    return Err(TypeError::UnsupportedFeature(format!(
                        "generic enum types are not supported: {}<{}>",
                        name,
                        args.len()
                    )));
                }
                if self
                    .records
                    .get(name)
                    .is_some_and(|record| record.type_params.iter().any(|param| param.is_temporal))
                {
                    return Err(TypeError::UnsupportedFeature(format!(
                        "enum variant '{}::{}' cannot carry a temporal record payload '{}'; enum payloads must be concrete and non-temporal",
                        enum_name, variant_name, name
                    )));
                }
                for arg in args {
                    self.validate_enum_payload_source_type(enum_name, variant_name, arg)?;
                }
                Ok(())
            }
            Type::Named(name) => {
                if let Some(record) = self.records.get(name) {
                    if record.type_params.iter().any(|param| param.is_temporal) {
                        return Err(TypeError::UnsupportedFeature(format!(
                            "enum variant '{}::{}' cannot carry a temporal record payload '{}'; enum payloads must be concrete and non-temporal",
                            enum_name, variant_name, name
                        )));
                    }
                    if record.type_params.iter().any(|param| !param.is_temporal) {
                        return Err(TypeError::UnsupportedFeature(format!(
                            "enum variant '{}::{}' payload type '{}' is not concrete; supply all generic record type arguments",
                            enum_name, variant_name, name
                        )));
                    }
                }
                Ok(())
            }
        }
    }

    fn collect_source_type_dependencies(
        ty: &Type,
        type_names: &HashSet<String>,
        dependencies: &mut HashSet<String>,
    ) {
        match ty {
            Type::Named(name) | Type::Temporal(name, _) => {
                if type_names.contains(name) {
                    dependencies.insert(name.clone());
                }
            }
            Type::Generic(name, args) => {
                if type_names.contains(name) {
                    dependencies.insert(name.clone());
                }
                for arg in args {
                    Self::collect_source_type_dependencies(arg, type_names, dependencies);
                }
            }
            Type::Function(params, return_type) => {
                for param in params {
                    Self::collect_source_type_dependencies(param, type_names, dependencies);
                }
                Self::collect_source_type_dependencies(return_type, type_names, dependencies);
            }
        }
    }

    fn collect_typed_type_dependencies(
        ty: &TypedType,
        type_names: &HashSet<String>,
        dependencies: &mut HashSet<String>,
    ) {
        match ty {
            TypedType::Enum { name } => {
                if type_names.contains(name) {
                    dependencies.insert(name.clone());
                }
            }
            TypedType::Record {
                name, type_args, ..
            } => {
                if type_names.contains(name) {
                    dependencies.insert(name.clone());
                }
                for type_arg in type_args {
                    Self::collect_typed_type_dependencies(type_arg, type_names, dependencies);
                }
            }
            TypedType::Option(inner)
            | TypedType::List(inner)
            | TypedType::Array(inner, _)
            | TypedType::Temporal {
                base_type: inner, ..
            } => Self::collect_typed_type_dependencies(inner, type_names, dependencies),
            TypedType::Result(ok, err) => {
                Self::collect_typed_type_dependencies(ok, type_names, dependencies);
                Self::collect_typed_type_dependencies(err, type_names, dependencies);
            }
            TypedType::Function {
                params,
                return_type,
            } => {
                for param in params {
                    Self::collect_typed_type_dependencies(param, type_names, dependencies);
                }
                Self::collect_typed_type_dependencies(return_type, type_names, dependencies);
            }
            TypedType::Projection { base, args, .. } => {
                Self::collect_typed_type_dependencies(base, type_names, dependencies);
                for arg in args {
                    Self::collect_typed_type_dependencies(arg, type_names, dependencies);
                }
            }
            TypedType::Int32
            | TypedType::Int64
            | TypedType::Float64
            | TypedType::Boolean
            | TypedType::String
            | TypedType::Char
            | TypedType::Unit
            | TypedType::TypeParam(_)
            | TypedType::InferVar(_) => {}
        }
    }

    fn dependency_reaches(
        current: &str,
        target: &str,
        graph: &HashMap<String, HashSet<String>>,
        visited: &mut HashSet<String>,
    ) -> bool {
        let Some(dependencies) = graph.get(current) else {
            return false;
        };
        for dependency in dependencies {
            if dependency == target {
                return true;
            }
            if visited.insert(dependency.clone())
                && Self::dependency_reaches(dependency, target, graph, visited)
            {
                return true;
            }
        }
        false
    }

    fn validate_enum_recursion(&self, program: &Program) -> Result<(), TypeError> {
        let enum_names = self.enums.keys().cloned().collect::<HashSet<_>>();
        let type_names = enum_names
            .iter()
            .cloned()
            .chain(self.records.keys().cloned())
            .collect::<HashSet<_>>();
        let mut graph = HashMap::<String, HashSet<String>>::new();
        for decl in &program.declarations {
            let TopDecl::Enum(enum_decl) = Self::decl_registration_item(decl) else {
                continue;
            };
            let dependencies = graph.entry(enum_decl.name.clone()).or_default();
            for variant in &enum_decl.variants {
                if let Some(payload) = &variant.payload {
                    Self::collect_source_type_dependencies(payload, &type_names, dependencies);
                }
            }
        }
        for (record_name, record_def) in &self.records {
            let dependencies = graph.entry(record_name.clone()).or_default();
            for field_type in record_def.fields.values() {
                Self::collect_typed_type_dependencies(field_type, &type_names, dependencies);
            }
        }

        for name in &enum_names {
            let mut visited = HashSet::from([name.clone()]);
            if Self::dependency_reaches(name, name, &graph, &mut visited) {
                return Err(TypeError::UnsupportedFeature(format!(
                    "recursive enum definitions are not supported (cycle includes '{}')",
                    name
                )));
            }
        }
        Ok(())
    }

    fn register_enum_payloads(&mut self, enum_decl: &EnumDecl) -> Result<(), TypeError> {
        let mut payloads = Vec::with_capacity(enum_decl.variants.len());
        for variant in &enum_decl.variants {
            let payload = if let Some(payload) = &variant.payload {
                self.validate_enum_payload_source_type(&enum_decl.name, &variant.name, payload)?;
                Some(self.convert_type(payload)?)
            } else {
                None
            };
            payloads.push((variant.name.clone(), payload));
        }

        let enum_def = self.enums.get_mut(&enum_decl.name).ok_or_else(|| {
            TypeError::UnknownType(format!(
                "enum '{}' was not registered before its payloads",
                enum_decl.name
            ))
        })?;
        for (variant_name, payload) in payloads {
            enum_def.variants.insert(variant_name, payload);
        }
        Ok(())
    }

    pub fn check_program(&mut self, program: &Program) -> Result<(), TypeError> {
        self.checked_expr_types.clear();
        self.form_bound_validation_ready = false;
        self.reject_unresolved_imports(&program.imports)?;

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

        // First pass: register source form names before signatures and bounds
        // are resolved. Prelude forms were installed when the checker was
        // created, so a source declaration cannot silently shadow them.
        for decl in &program.declarations {
            if let TopDecl::Form(form) = Self::decl_registration_item(decl) {
                self.register_form_header(form)?;
            }
        }

        // Second pass: register enum names and variant names. Payloads are
        // resolved only after record shapes exist, while records may already
        // refer to these enum headers.
        for decl in &program.declarations {
            if let TopDecl::Enum(enum_decl) = Self::decl_registration_item(decl) {
                self.register_enum_header(enum_decl)?;
            }
        }

        // Third pass: register record/context shapes before any signature that
        // may mention them, regardless of source order.
        for decl in &program.declarations {
            match Self::decl_registration_item(decl) {
                TopDecl::Record(record) => {
                    self.check_record_decl(record)?;
                }
                TopDecl::Context(context) => {
                    self.check_context_decl(context)?;
                }
                _ => {}
            }
        }

        // Fourth pass: reject recursive enum graphs, then resolve each payload.
        self.validate_enum_recursion(program)?;
        for decl in &program.declarations {
            if let TopDecl::Enum(enum_decl) = Self::decl_registration_item(decl) {
                self.register_enum_payloads(enum_decl)?;
            }
        }

        // Fifth pass: resolve source form method signatures now that all
        // nominal type names are available.
        for decl in &program.declarations {
            if let TopDecl::Form(form) = Self::decl_registration_item(decl) {
                self.register_form_methods(form)?;
            }
        }

        // Sixth pass: register function signatures for forward references.
        for decl in &program.declarations {
            if let TopDecl::Function(func) = Self::decl_registration_item(decl) {
                self.register_function_signature(func)?;
            }
        }

        // Seventh pass: register impl method signatures before checking bodies,
        // so OSV method calls can refer to impl blocks declared later.
        for decl in &program.declarations {
            if let TopDecl::Impl(impl_block) = Self::decl_registration_item(decl) {
                self.register_impl_method_signatures(impl_block)?;
            }
        }

        // Eighth pass: validate explicit form adoptions and register their
        // method signatures. This happens after ordinary impl signatures so a
        // selector collision is deterministic and diagnosed before bodies.
        for decl in &program.declarations {
            if let TopDecl::Takes(takes) = Self::decl_registration_item(decl) {
                self.register_takes_method_signatures(takes)?;
            }
        }

        // Record and callable signatures are registered before `takes`
        // declarations so nominal references can be resolved independent of
        // source order. Now that all adoption edges are known, validate every
        // previously registered bounded-record use and enable eager validation
        // for body-local annotations and record construction.
        self.form_bound_validation_ready = true;
        self.validate_registered_record_form_bound_uses()?;

        // Ninth pass: check impl and takes bodies before ordinary functions. This turns
        // unannotated method returns from provisional signatures into inferred
        // concrete method signatures before function bodies call them.
        for decl in &program.declarations {
            if let TopDecl::Impl(impl_block) = Self::decl_registration_item(decl) {
                self.check_impl_block(impl_block)?;
            }
        }
        for decl in &program.declarations {
            if let TopDecl::Takes(takes) = Self::decl_registration_item(decl) {
                self.check_takes_decl(takes)?;
            }
        }

        // Tenth pass: infer unannotated ordinary function returns before
        // annotated functions and top-level bindings use those functions.
        self.infer_unannotated_function_returns(program)?;

        // Final pass: check all remaining declarations
        for decl in &program.declarations {
            match Self::decl_registration_item(decl) {
                TopDecl::Record(_) => {
                    // Already processed in first pass
                }
                TopDecl::Enum(_) => {
                    // Header and payload were already processed before signatures.
                }
                TopDecl::Context(_) => {
                    // Already processed in first pass
                }
                TopDecl::Impl(_) => {
                    // Already processed before function bodies
                }
                TopDecl::Form(_) | TopDecl::Takes(_) => {
                    // Already registered and checked in dedicated passes.
                }
                TopDecl::Function(func) if func.return_type.is_none() => {
                    // Already processed before annotated function bodies
                }
                _ => {
                    self.check_top_decl(decl)?;
                }
            }
        }
        self.reject_unresolved_inference_in_current_scope()?;
        Ok(())
    }

    fn infer_unannotated_function_returns(&mut self, program: &Program) -> Result<(), TypeError> {
        let mut pending = program
            .declarations
            .iter()
            .filter_map(|decl| match Self::decl_registration_item(decl) {
                TopDecl::Function(func) if func.return_type.is_none() => Some(func),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut pending_names = pending
            .iter()
            .map(|func| func.name.clone())
            .collect::<HashSet<_>>();
        let unannotated_names = pending_names.clone();

        while !pending.is_empty() {
            let next_idx = pending.iter().position(|func| {
                let deps =
                    self.collect_unannotated_function_deps_for_function(func, &unannotated_names);
                deps.iter().all(|dep| !pending_names.contains(dep))
            });

            if let Some(idx) = next_idx {
                let func = pending.remove(idx);
                self.check_function_decl(func)?;
                pending_names.remove(&func.name);
            } else {
                // Preserve the existing diagnostic for recursive or mutually
                // recursive unannotated functions.
                return self.check_function_decl(pending[0]);
            }
        }

        Ok(())
    }

    fn collect_unannotated_function_deps_for_function(
        &self,
        func: &FunDecl,
        unannotated_names: &HashSet<String>,
    ) -> HashSet<String> {
        let mut probe = self.clone();
        probe.push_type_param_scope(&func.type_params);
        probe.push_scope();

        let mut bound_vars = HashSet::new();
        if let Some(signature) = self.functions.get(&func.name) {
            for (param, (_, ty)) in func.params.iter().zip(signature.params.iter()) {
                bound_vars.insert(param.name.clone());
                let _ = probe.bind_var(param.name.clone(), ty.clone(), false);
            }
        }

        probe.collect_unannotated_function_deps_in_block(&func.body, &bound_vars, unannotated_names)
    }

    fn collect_unannotated_function_deps_in_block(
        &mut self,
        block: &BlockExpr,
        bound_vars: &HashSet<String>,
        unannotated_names: &HashSet<String>,
    ) -> HashSet<String> {
        self.push_scope();
        let mut deps = HashSet::new();
        let mut block_bound = bound_vars.clone();

        for stmt in &block.statements {
            match stmt {
                Stmt::Binding(bind) => {
                    let annotated_binding_type = bind.type_annotation.as_ref().and_then(|ty| {
                        let mut type_probe = self.clone();
                        type_probe.convert_type(ty).ok()
                    });
                    if let (ExprKind::Lambda(lambda), Some(TypedType::Function { params, .. })) =
                        (&bind.value.kind, annotated_binding_type.as_ref())
                    {
                        deps.extend(self.collect_unannotated_lambda_deps(
                            lambda,
                            &block_bound,
                            unannotated_names,
                            Some(params),
                        ));
                    } else {
                        deps.extend(self.collect_unannotated_function_deps_in_expr(
                            &bind.value,
                            &block_bound,
                            unannotated_names,
                        ));
                    }
                    let binding_type = annotated_binding_type.or_else(|| {
                        let mut type_probe = self.clone();
                        if bind.type_annotation.is_none() {
                            type_probe.check_expr(&bind.value).ok()
                        } else {
                            None
                        }
                    });
                    if let Some(binding_type) = binding_type {
                        let _ = self.bind_pattern(&bind.pattern, &binding_type, bind.mutable);
                    }
                    let mut pattern_vars = HashSet::new();
                    self.collect_pattern_bindings(&bind.pattern, &mut pattern_vars);
                    block_bound.extend(pattern_vars);
                }
                Stmt::Assignment(assign) => {
                    deps.extend(self.collect_unannotated_function_deps_in_expr(
                        &assign.value,
                        &block_bound,
                        unannotated_names,
                    ));
                }
                Stmt::Expr(expr) => {
                    deps.extend(self.collect_unannotated_function_deps_in_expr(
                        expr,
                        &block_bound,
                        unannotated_names,
                    ));
                }
            }
        }

        if let Some(expr) = &block.expr {
            deps.extend(self.collect_unannotated_function_deps_in_expr(
                expr,
                &block_bound,
                unannotated_names,
            ));
        }

        self.pop_scope();
        deps
    }

    fn collect_unannotated_function_deps_in_expr(
        &mut self,
        expr: &Expr,
        bound_vars: &HashSet<String>,
        unannotated_names: &HashSet<String>,
    ) -> HashSet<String> {
        let mut deps = HashSet::new();

        match &expr.kind {
            ExprKind::Ident(name) => {
                if !bound_vars.contains(name) && unannotated_names.contains(name) {
                    deps.insert(name.clone());
                }
            }
            ExprKind::Binary(binary) => {
                deps.extend(self.collect_unannotated_function_deps_in_expr(
                    &binary.left,
                    bound_vars,
                    unannotated_names,
                ));
                deps.extend(self.collect_unannotated_function_deps_in_expr(
                    &binary.right,
                    bound_vars,
                    unannotated_names,
                ));
            }
            ExprKind::Unary(unary) => {
                deps.extend(self.collect_unannotated_function_deps_in_expr(
                    &unary.expr,
                    bound_vars,
                    unannotated_names,
                ));
            }
            ExprKind::Cast(cast) => {
                deps.extend(self.collect_unannotated_function_deps_in_expr(
                    &cast.expr,
                    bound_vars,
                    unannotated_names,
                ));
            }
            ExprKind::Call(call) => {
                let callable_name = match &call.function.kind {
                    ExprKind::Ident(name) => Some(name.as_str()),
                    _ => None,
                };
                let selected_method = match callable_name {
                    Some(name) => call
                        .args
                        .first()
                        .is_some_and(|receiver| self.receiver_has_method_selector(receiver, name)),
                    None => false,
                };
                if !selected_method {
                    if let ExprKind::Lambda(lambda) = &call.function.kind {
                        let expected_params = call
                            .args
                            .iter()
                            .map(|arg| {
                                let mut type_probe = self.clone();
                                type_probe.check_expr(arg).ok()
                            })
                            .collect::<Option<Vec<_>>>();
                        deps.extend(self.collect_unannotated_lambda_deps(
                            lambda,
                            bound_vars,
                            unannotated_names,
                            expected_params.as_deref(),
                        ));
                    } else {
                        deps.extend(self.collect_unannotated_function_deps_in_expr(
                            &call.function,
                            bound_vars,
                            unannotated_names,
                        ));
                    }
                }
                for (index, arg) in call.args.iter().enumerate() {
                    let expected_arg = callable_name.and_then(|name| {
                        self.expected_function_param_type_for_call(name, index, &call.args)
                            .ok()
                            .flatten()
                    });
                    if let (ExprKind::Lambda(lambda), Some(TypedType::Function { params, .. })) =
                        (&arg.kind, expected_arg.as_ref())
                    {
                        deps.extend(self.collect_unannotated_lambda_deps(
                            lambda,
                            bound_vars,
                            unannotated_names,
                            Some(params),
                        ));
                    } else {
                        deps.extend(self.collect_unannotated_function_deps_in_expr(
                            arg,
                            bound_vars,
                            unannotated_names,
                        ));
                    }
                }
            }
            ExprKind::Pipe(pipe) => {
                let expected_receiver = self
                    .expected_type_for_pipe_target_first_arg(&pipe.target, &pipe.expr)
                    .ok()
                    .flatten();
                if let (ExprKind::Lambda(lambda), Some(TypedType::Function { params, .. })) =
                    (&pipe.expr.kind, expected_receiver.as_ref())
                {
                    deps.extend(self.collect_unannotated_lambda_deps(
                        lambda,
                        bound_vars,
                        unannotated_names,
                        Some(params),
                    ));
                } else {
                    deps.extend(self.collect_unannotated_function_deps_in_expr(
                        &pipe.expr,
                        bound_vars,
                        unannotated_names,
                    ));
                }
                match &pipe.target {
                    PipeTarget::Ident(name) => {
                        if !self.receiver_has_form_method_selector(&pipe.expr, name)
                            && !bound_vars.contains(name)
                            && unannotated_names.contains(name)
                        {
                            deps.insert(name.clone());
                        }
                    }
                    PipeTarget::Expr(target) => {
                        if let ExprKind::Lambda(lambda) = &target.kind {
                            let expected_param = {
                                let mut type_probe = self.clone();
                                type_probe.check_expr(&pipe.expr).ok()
                            };
                            let expected_params = expected_param.map(|ty| vec![ty]);
                            deps.extend(self.collect_unannotated_lambda_deps(
                                lambda,
                                bound_vars,
                                unannotated_names,
                                expected_params.as_deref(),
                            ));
                        } else {
                            let selected_method = match &target.kind {
                                ExprKind::Ident(name) => {
                                    self.receiver_has_form_method_selector(&pipe.expr, name)
                                }
                                _ => false,
                            };
                            if !selected_method {
                                deps.extend(self.collect_unannotated_function_deps_in_expr(
                                    target,
                                    bound_vars,
                                    unannotated_names,
                                ));
                            }
                        }
                    }
                }
            }
            ExprKind::FieldAccess(object, _) => {
                deps.extend(self.collect_unannotated_function_deps_in_expr(
                    object,
                    bound_vars,
                    unannotated_names,
                ));
            }
            ExprKind::RecordLit(record_lit) => {
                for field in &record_lit.fields {
                    match field {
                        FieldInit::Field { value, .. } | FieldInit::Spread(value) => {
                            deps.extend(self.collect_unannotated_function_deps_in_expr(
                                value,
                                bound_vars,
                                unannotated_names,
                            ));
                        }
                    }
                }
            }
            ExprKind::Clone(clone_expr) => {
                deps.extend(self.collect_unannotated_function_deps_in_expr(
                    &clone_expr.base,
                    bound_vars,
                    unannotated_names,
                ));
                for field in &clone_expr.updates.fields {
                    match field {
                        FieldInit::Field { value, .. } | FieldInit::Spread(value) => {
                            deps.extend(self.collect_unannotated_function_deps_in_expr(
                                value,
                                bound_vars,
                                unannotated_names,
                            ));
                        }
                    }
                }
            }
            ExprKind::PrototypeClone(proto_clone) => {
                for field in &proto_clone.updates.fields {
                    match field {
                        FieldInit::Field { value, .. } | FieldInit::Spread(value) => {
                            deps.extend(self.collect_unannotated_function_deps_in_expr(
                                value,
                                bound_vars,
                                unannotated_names,
                            ));
                        }
                    }
                }
            }
            ExprKind::Freeze(inner) | ExprKind::Await(inner) | ExprKind::Spawn(inner) => {
                deps.extend(self.collect_unannotated_function_deps_in_expr(
                    inner,
                    bound_vars,
                    unannotated_names,
                ));
            }
            ExprKind::ListLit(elements) | ExprKind::ArrayLit(elements) => {
                for element in elements {
                    deps.extend(self.collect_unannotated_function_deps_in_expr(
                        element,
                        bound_vars,
                        unannotated_names,
                    ));
                }
            }
            ExprKind::RangeLit(range) => {
                deps.extend(self.collect_unannotated_function_deps_in_expr(
                    &range.start,
                    bound_vars,
                    unannotated_names,
                ));
                deps.extend(self.collect_unannotated_function_deps_in_expr(
                    &range.end,
                    bound_vars,
                    unannotated_names,
                ));
            }
            ExprKind::Match(match_expr) => {
                let matched_type = {
                    let mut type_probe = self.clone();
                    type_probe.check_expr(&match_expr.expr).ok()
                };
                deps.extend(self.collect_unannotated_function_deps_in_expr(
                    &match_expr.expr,
                    bound_vars,
                    unannotated_names,
                ));
                for arm in &match_expr.arms {
                    let mut arm_bound = bound_vars.clone();
                    self.collect_pattern_bindings(&arm.pattern, &mut arm_bound);
                    self.push_scope();
                    if let Some(matched_type) = &matched_type {
                        let _ = self.bind_pattern_vars(&arm.pattern, matched_type);
                    }
                    deps.extend(self.collect_unannotated_function_deps_in_block(
                        &arm.body,
                        &arm_bound,
                        unannotated_names,
                    ));
                    self.pop_scope();
                }
            }
            ExprKind::Then(then_expr) => {
                deps.extend(self.collect_unannotated_function_deps_in_expr(
                    &then_expr.condition,
                    bound_vars,
                    unannotated_names,
                ));
                deps.extend(self.collect_unannotated_function_deps_in_block(
                    &then_expr.then_block,
                    bound_vars,
                    unannotated_names,
                ));
                for (condition, block) in &then_expr.else_ifs {
                    deps.extend(self.collect_unannotated_function_deps_in_expr(
                        condition,
                        bound_vars,
                        unannotated_names,
                    ));
                    deps.extend(self.collect_unannotated_function_deps_in_block(
                        block,
                        bound_vars,
                        unannotated_names,
                    ));
                }
                if let Some(else_block) = &then_expr.else_block {
                    deps.extend(self.collect_unannotated_function_deps_in_block(
                        else_block,
                        bound_vars,
                        unannotated_names,
                    ));
                }
            }
            ExprKind::While(while_expr) => {
                deps.extend(self.collect_unannotated_function_deps_in_expr(
                    &while_expr.condition,
                    bound_vars,
                    unannotated_names,
                ));
                deps.extend(self.collect_unannotated_function_deps_in_block(
                    &while_expr.body,
                    bound_vars,
                    unannotated_names,
                ));
            }
            ExprKind::Block(block) => {
                deps.extend(self.collect_unannotated_function_deps_in_block(
                    block,
                    bound_vars,
                    unannotated_names,
                ));
            }
            ExprKind::Lambda(lambda) => {
                deps.extend(self.collect_unannotated_lambda_deps(
                    lambda,
                    bound_vars,
                    unannotated_names,
                    None,
                ));
            }
            ExprKind::With(with_expr) => {
                let mut body_bound = bound_vars.clone();
                self.push_scope();
                for binding in &with_expr.bindings {
                    match binding {
                        FieldInit::Field { name, value } => {
                            deps.extend(self.collect_unannotated_function_deps_in_expr(
                                value,
                                bound_vars,
                                unannotated_names,
                            ));
                            body_bound.insert(name.clone());
                            let binding_type = {
                                let mut type_probe = self.clone();
                                type_probe.check_expr(value).ok()
                            };
                            if let Some(binding_type) = binding_type {
                                let _ = self.bind_var(name.clone(), binding_type, false);
                            }
                        }
                        FieldInit::Spread(value) => {
                            deps.extend(self.collect_unannotated_function_deps_in_expr(
                                value,
                                bound_vars,
                                unannotated_names,
                            ));
                        }
                    }
                }
                deps.extend(self.collect_unannotated_function_deps_in_block(
                    &with_expr.body,
                    &body_bound,
                    unannotated_names,
                ));
                self.pop_scope();
            }
            ExprKind::WithLifetime(with_lifetime) => {
                deps.extend(self.collect_unannotated_function_deps_in_block(
                    &with_lifetime.body,
                    bound_vars,
                    unannotated_names,
                ));
            }
            ExprKind::VariantRef(_)
            | ExprKind::IntLit(_)
            | ExprKind::FloatLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::CharLit(_)
            | ExprKind::BoolLit(_)
            | ExprKind::Unit => {}
        }

        deps
    }

    fn collect_unannotated_lambda_deps(
        &mut self,
        lambda: &LambdaExpr,
        bound_vars: &HashSet<String>,
        unannotated_names: &HashSet<String>,
        expected_params: Option<&[TypedType]>,
    ) -> HashSet<String> {
        let mut lambda_bound = bound_vars.clone();
        self.push_scope();
        for (index, param) in lambda.params.iter().enumerate() {
            lambda_bound.insert(param.name.clone());
            let param_type = if let Some(annotation) = &param.type_annotation {
                self.convert_type(annotation).ok()
            } else {
                expected_params.and_then(|params| params.get(index).cloned())
            };
            if let Some(param_type) = param_type {
                let _ = self.bind_var(param.name.clone(), param_type, false);
            }
        }
        let deps = self.collect_unannotated_function_deps_in_expr(
            &lambda.body,
            &lambda_bound,
            unannotated_names,
        );
        self.pop_scope();
        deps
    }

    fn decl_registration_item(decl: &TopDecl) -> &TopDecl {
        match decl {
            TopDecl::Export(export_decl) => export_decl.item.as_ref(),
            _ => decl,
        }
    }

    fn reject_unresolved_imports(&self, imports: &[ImportDecl]) -> Result<(), TypeError> {
        if let Some(import) = imports.first() {
            return Err(TypeError::UnsupportedFeature(format!(
                "source-level imports must be resolved before type checking; unresolved import {} remains",
                Self::format_import(import)
            )));
        }

        Ok(())
    }

    fn format_import(import: &ImportDecl) -> String {
        let module_name = import.module_path.join(".");

        match &import.items {
            ImportItems::All => format!("{}.*", module_name),
            ImportItems::Named(items) => format!("{}.{{{}}}", module_name, items.join(", ")),
        }
    }

    fn validate_declared_form_bounds(&self, type_params: &[TypeParam]) -> Result<(), TypeError> {
        for type_param in type_params {
            if type_param
                .bounds
                .iter()
                .any(|bound| bound.trait_name == "Container")
            {
                return Err(TypeError::UnsupportedFeature(
                    "form 'Container' is compiler-internal and cannot appear in source bounds"
                        .to_string(),
                ));
            }
            let mut seen = HashSet::new();
            let mut selector_forms = HashMap::<String, String>::new();
            for form_name in self.form_bound_names(type_param) {
                if form_name == "Container" {
                    return Err(TypeError::UnsupportedFeature(
                        "form 'Container' is compiler-internal and cannot appear in source bounds"
                            .to_string(),
                    ));
                }
                let form = self
                    .forms
                    .get(&form_name)
                    .ok_or_else(|| TypeError::UndefinedForm(form_name.clone()))?;
                if !seen.insert(form_name.clone()) {
                    return Err(TypeError::UnsupportedFeature(format!(
                        "duplicate form bound '{} of {}'",
                        type_param.name, form_name
                    )));
                }

                let mut selectors = form.methods.keys().cloned().collect::<Vec<_>>();
                selectors.sort();
                for selector in selectors {
                    if let Some(previous_form) =
                        selector_forms.insert(selector.clone(), form_name.clone())
                    {
                        return Err(TypeError::AmbiguousFormMethod {
                            method: selector,
                            forms: vec![previous_form, form_name.clone()],
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_record_type_form_bounds(&self, ty: &TypedType) -> Result<(), TypeError> {
        match ty {
            TypedType::Record {
                name, type_args, ..
            } => {
                // Some compiler-provided callables use opaque internal record
                // names (for example Task) that are not source declarations.
                // They have no source `of` contract to validate here.
                let Some(record) = self.records.get(name) else {
                    for arg in type_args {
                        self.validate_record_type_form_bounds(arg)?;
                    }
                    return Ok(());
                };
                let regular_params = record
                    .type_params
                    .iter()
                    .filter(|param| !param.is_temporal)
                    .collect::<Vec<_>>();
                let has_form_bounds = regular_params
                    .iter()
                    .any(|param| !self.form_bound_names(param).is_empty());

                if has_form_bounds && type_args.len() != regular_params.len() {
                    return Err(TypeError::CannotInferType(format!(
                        "bounded record '{}' requires {} concrete type argument(s), found {}",
                        name,
                        regular_params.len(),
                        type_args.len()
                    )));
                }

                let mut constraints = Vec::new();
                for (param, arg) in regular_params.iter().zip(type_args.iter()) {
                    for form_name in self.form_bound_names(param) {
                        constraints.push(Constraint::HasForm {
                            ty: arg.clone(),
                            form_name,
                            origin: Self::constraint_origin(ConstraintKind::FormBound {
                                type_param: format!("{}::{}", name, param.name),
                            }),
                        });
                    }
                }
                if !constraints.is_empty() {
                    self.solve_constraints_with_current_forms(
                        &constraints,
                        &ConstraintSubstitution::new(),
                    )?;
                }

                for arg in type_args {
                    self.validate_record_type_form_bounds(arg)?;
                }
                Ok(())
            }
            TypedType::Option(inner) | TypedType::List(inner) | TypedType::Array(inner, _) => {
                self.validate_record_type_form_bounds(inner)
            }
            TypedType::Result(ok, err) => {
                self.validate_record_type_form_bounds(ok)?;
                self.validate_record_type_form_bounds(err)
            }
            TypedType::Function {
                params,
                return_type,
            } => {
                for param in params {
                    self.validate_record_type_form_bounds(param)?;
                }
                self.validate_record_type_form_bounds(return_type)
            }
            TypedType::Temporal { base_type, .. } => {
                self.validate_record_type_form_bounds(base_type)
            }
            TypedType::Projection { base, args, .. } => {
                self.validate_record_type_form_bounds(base)?;
                for arg in args {
                    self.validate_record_type_form_bounds(arg)?;
                }
                Ok(())
            }
            TypedType::Int32
            | TypedType::Int64
            | TypedType::Float64
            | TypedType::Boolean
            | TypedType::String
            | TypedType::Char
            | TypedType::Unit
            | TypedType::Enum { .. }
            | TypedType::TypeParam(_)
            | TypedType::InferVar(_) => Ok(()),
        }
    }

    fn validate_registered_record_form_bound_uses(&mut self) -> Result<(), TypeError> {
        let record_types = self
            .records
            .values()
            .map(|record| {
                (
                    record.type_params.clone(),
                    record.fields.values().cloned().collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        for (type_params, fields) in record_types {
            self.push_type_param_scope(&type_params);
            let result = fields
                .iter()
                .try_for_each(|field| self.validate_record_type_form_bounds(field));
            self.pop_type_param_scope();
            result?;
        }

        let enum_payloads = self
            .enums
            .values()
            .flat_map(|definition| definition.variants.values().flatten().cloned())
            .collect::<Vec<_>>();
        for payload in &enum_payloads {
            self.validate_record_type_form_bounds(payload)?;
        }

        let function_types = self.functions.values().cloned().collect::<Vec<_>>();
        for function in function_types {
            self.push_type_param_scope(&function.type_params);
            let result = function
                .params
                .iter()
                .map(|(_, ty)| ty)
                .chain(std::iter::once(&function.return_type))
                .try_for_each(|ty| self.validate_record_type_form_bounds(ty));
            self.pop_type_param_scope();
            result?;
        }

        let method_types = self
            .methods
            .values()
            .flat_map(|methods| methods.values().cloned())
            .collect::<Vec<_>>();
        for method in method_types {
            self.push_type_param_scope(&method.type_params);
            let result = method
                .params
                .iter()
                .map(|(_, ty)| ty)
                .chain(std::iter::once(&method.return_type))
                .try_for_each(|ty| self.validate_record_type_form_bounds(ty));
            self.pop_type_param_scope();
            result?;
        }

        let form_methods = self
            .forms
            .values()
            .flat_map(|form| form.methods.values().cloned())
            .collect::<Vec<_>>();
        let self_param = TypeParam {
            name: "Self".to_string(),
            bounds: vec![],
            derivation_bound: None,
            of_forms: vec![],
            is_temporal: false,
        };
        for method in form_methods {
            self.push_type_param_scope(std::slice::from_ref(&self_param));
            let result = method
                .params
                .iter()
                .map(|(_, ty)| ty)
                .chain(std::iter::once(&method.return_type))
                .try_for_each(|ty| self.validate_record_type_form_bounds(ty));
            self.pop_type_param_scope();
            result?;
        }

        Ok(())
    }

    fn register_form_header(&mut self, form: &FormDecl) -> Result<(), TypeError> {
        if self.forms.contains_key(&form.name) {
            return Err(TypeError::DuplicateForm(form.name.clone()));
        }
        self.forms.insert(form.name.clone(), FormDef::default());
        Ok(())
    }

    fn register_form_methods(&mut self, form: &FormDecl) -> Result<(), TypeError> {
        if form.methods.is_empty() {
            return Err(TypeError::UnsupportedFeature(format!(
                "form '{}' must declare at least one method",
                form.name
            )));
        }

        let self_param = TypeParam {
            name: "Self".to_string(),
            bounds: vec![],
            derivation_bound: None,
            of_forms: vec![],
            is_temporal: false,
        };
        self.push_type_param_scope(std::slice::from_ref(&self_param));
        let result = (|| {
            let mut methods = HashMap::new();
            for method in &form.methods {
                Self::validate_unique_parameter_names(
                    &format!("form method '{}::{}'", form.name, method.name),
                    &method.params,
                )?;
                if Self::is_display_intrinsic_name(&method.name) {
                    return Err(TypeError::UnsupportedFeature(format!(
                        "form method selector '{}' is compiler-reserved; only the prelude Display.display contract may declare it",
                        method.name
                    )));
                }
                let Some(receiver) = method.params.first() else {
                    return Err(TypeError::UnsupportedFeature(format!(
                        "form method '{}::{}' must declare first parameter as self: Self",
                        form.name, method.name
                    )));
                };
                if receiver.name != "self"
                    || receiver.ty != crate::ast::Type::Named("Self".to_string())
                {
                    return Err(TypeError::UnsupportedFeature(format!(
                        "form method '{}::{}' must declare first parameter as self: Self",
                        form.name, method.name
                    )));
                }
                if methods.contains_key(&method.name) {
                    return Err(TypeError::UnsupportedFeature(format!(
                        "duplicate method '{}' in form '{}'",
                        method.name, form.name
                    )));
                }

                let params = method
                    .params
                    .iter()
                    .map(|param| Ok((param.name.clone(), self.convert_type(&param.ty)?)))
                    .collect::<Result<Vec<_>, TypeError>>()?;
                let return_type = self.convert_type(&method.return_type)?;
                methods.insert(
                    method.name.clone(),
                    FunctionDef {
                        params,
                        return_type,
                        type_params: vec![],
                        temporal_constraints: vec![],
                    },
                );
            }
            Ok(methods)
        })();
        self.pop_type_param_scope();

        let methods = result?;
        self.forms
            .get_mut(&form.name)
            .ok_or_else(|| TypeError::UndefinedForm(form.name.clone()))?
            .methods = methods;
        Ok(())
    }

    fn substitute_form_self(ty: &TypedType, target: &TypedType) -> TypedType {
        match ty {
            TypedType::TypeParam(name) if name == "Self" => target.clone(),
            TypedType::List(inner) => {
                TypedType::List(Box::new(Self::substitute_form_self(inner, target)))
            }
            TypedType::Option(inner) => {
                TypedType::Option(Box::new(Self::substitute_form_self(inner, target)))
            }
            TypedType::Result(ok, err) => TypedType::Result(
                Box::new(Self::substitute_form_self(ok, target)),
                Box::new(Self::substitute_form_self(err, target)),
            ),
            TypedType::Array(inner, size) => {
                TypedType::Array(Box::new(Self::substitute_form_self(inner, target)), *size)
            }
            TypedType::Function {
                params,
                return_type,
            } => TypedType::Function {
                params: params
                    .iter()
                    .map(|param| Self::substitute_form_self(param, target))
                    .collect(),
                return_type: Box::new(Self::substitute_form_self(return_type, target)),
            },
            TypedType::Record {
                name,
                type_args,
                frozen,
                hash,
                parent_hash,
            } => TypedType::Record {
                name: name.clone(),
                type_args: type_args
                    .iter()
                    .map(|arg| Self::substitute_form_self(arg, target))
                    .collect(),
                frozen: *frozen,
                hash: hash.clone(),
                parent_hash: parent_hash.clone(),
            },
            TypedType::Temporal {
                base_type,
                temporals,
            } => TypedType::Temporal {
                base_type: Box::new(Self::substitute_form_self(base_type, target)),
                temporals: temporals.clone(),
            },
            TypedType::Projection {
                base,
                form_name,
                assoc_name,
                args,
            } => TypedType::Projection {
                base: Box::new(Self::substitute_form_self(base, target)),
                form_name: form_name.clone(),
                assoc_name: assoc_name.clone(),
                args: args
                    .iter()
                    .map(|arg| Self::substitute_form_self(arg, target))
                    .collect(),
            },
            _ => ty.clone(),
        }
    }

    fn register_takes_method_signatures(&mut self, takes: &TakesDecl) -> Result<(), TypeError> {
        if takes.form_name == "Container" {
            return Err(TypeError::UnsupportedFeature(
                "form 'Container' is compiler-internal and cannot be adopted by source declarations"
                    .to_string(),
            ));
        }
        if self._contexts.contains(&takes.target) {
            return Err(TypeError::UnsupportedFeature(format!(
                "takes target '{}' must be a concrete record; contexts cannot adopt forms",
                takes.target
            )));
        }
        let record = self.records.get(&takes.target).ok_or_else(|| {
            TypeError::UnsupportedFeature(format!(
                "takes target '{}' must be a concrete record declared in this program",
                takes.target
            ))
        })?;
        if !record.type_params.is_empty() {
            return Err(TypeError::UnsupportedFeature(format!(
                "generic takes target '{}' is not supported in the initial form slice",
                takes.target
            )));
        }
        let form = self
            .forms
            .get(&takes.form_name)
            .cloned()
            .ok_or_else(|| TypeError::UndefinedForm(takes.form_name.clone()))?;
        if self
            .form_adoptions
            .get(&takes.target)
            .is_some_and(|forms| forms.contains(&takes.form_name))
        {
            return Err(TypeError::DuplicateAdoption {
                target: takes.target.clone(),
                form: takes.form_name.clone(),
            });
        }

        let mut provided = HashSet::new();
        for function in &takes.functions {
            if !provided.insert(function.name.clone()) {
                return Err(TypeError::UnsupportedFeature(format!(
                    "duplicate method '{}' in {} takes {}",
                    function.name, takes.target, takes.form_name
                )));
            }
            if !form.methods.contains_key(&function.name) {
                return Err(TypeError::UnexpectedFormMethod {
                    form: takes.form_name.clone(),
                    method: function.name.clone(),
                });
            }
        }
        for required in form.methods.keys() {
            if !provided.contains(required) {
                return Err(TypeError::MissingFormMethod {
                    form: takes.form_name.clone(),
                    method: required.clone(),
                });
            }
        }

        let target_type = TypedType::Record {
            name: takes.target.clone(),
            type_args: vec![],
            frozen: false,
            hash: None,
            parent_hash: None,
        };
        let mut staged = Vec::new();
        for function in &takes.functions {
            Self::validate_unique_parameter_names(
                &format!(
                    "form adoption method '{} takes {}.{}'",
                    takes.target, takes.form_name, function.name
                ),
                &function.params,
            )?;
            if Self::is_display_intrinsic_name(&function.name)
                && !(takes.form_name == "Display" && function.name == "display")
            {
                return Err(TypeError::UnsupportedFeature(format!(
                    "form adoption method selector '{}' is compiler-reserved; only Display.display may implement a Display intrinsic",
                    function.name
                )));
            }
            if function.is_async
                || !function.type_params.is_empty()
                || !function.temporal_constraints.is_empty()
            {
                return Err(TypeError::UnsupportedFeature(format!(
                    "form adoption method '{}::{}' must be non-generic and non-temporal",
                    takes.target, function.name
                )));
            }
            let Some(return_annotation) = function.return_type.as_ref() else {
                return Err(TypeError::UnsupportedFeature(format!(
                    "form adoption method '{}::{}' requires an explicit return type",
                    takes.target, function.name
                )));
            };
            self.validate_impl_receiver_param(&takes.target, function)?;

            let actual = FunctionDef {
                params: self.impl_method_param_types(&takes.target, function)?,
                return_type: self.convert_type(return_annotation)?,
                type_params: vec![],
                temporal_constraints: vec![],
            };
            let required = form
                .methods
                .get(&function.name)
                .expect("provided form method was checked above");
            let expected_params = required
                .params
                .iter()
                .map(|(name, ty)| (name.clone(), Self::substitute_form_self(ty, &target_type)))
                .collect::<Vec<_>>();
            let expected_return = Self::substitute_form_self(&required.return_type, &target_type);
            let actual_param_types = actual.params.iter().map(|(_, ty)| ty).collect::<Vec<_>>();
            let expected_param_types = expected_params.iter().map(|(_, ty)| ty).collect::<Vec<_>>();
            if actual_param_types != expected_param_types || actual.return_type != expected_return {
                return Err(TypeError::UnsupportedFeature(format!(
                    "form method signature mismatch for '{} takes {}.{}': expected ({}) -> {}, found ({}) -> {}",
                    takes.target,
                    takes.form_name,
                    function.name,
                    expected_params
                        .iter()
                        .map(|(name, ty)| format!("{}: {}", name, format_typed_type(ty)))
                        .collect::<Vec<_>>()
                        .join(", "),
                    format_typed_type(&expected_return),
                    actual
                        .params
                        .iter()
                        .map(|(name, ty)| format!("{}: {}", name, format_typed_type(ty)))
                        .collect::<Vec<_>>()
                        .join(", "),
                    format_typed_type(&actual.return_type)
                )));
            }
            if self
                .methods
                .get(&takes.target)
                .and_then(|methods| methods.get(&function.name))
                .is_some()
            {
                return Err(TypeError::UnsupportedFeature(format!(
                    "method selector '{}' for '{}' conflicts with an existing impl or form adoption",
                    function.name, takes.target
                )));
            }
            staged.push((function.name.clone(), actual));
        }

        for (name, signature) in staged {
            self.methods
                .entry(takes.target.clone())
                .or_default()
                .insert(name, signature);
        }
        self.form_environment
            .register_direct_adoption(takes.target.clone(), takes.form_name.clone())?;
        self.form_adoptions
            .entry(takes.target.clone())
            .or_default()
            .insert(takes.form_name.clone());
        Ok(())
    }

    fn check_takes_decl(&mut self, takes: &TakesDecl) -> Result<(), TypeError> {
        self.check_impl_block(&ImplBlock {
            target: takes.target.clone(),
            functions: takes.functions.clone(),
        })
    }

    /// Check if the program needs lifetime inference
    fn needs_lifetime_inference(&self, program: &Program) -> bool {
        // Check if any declaration uses temporal types without explicit lifetimes
        for decl in &program.declarations {
            match decl {
                TopDecl::Record(record) if record.type_params.iter().any(|p| p.is_temporal) => {
                    return true;
                }
                TopDecl::Function(func) if func.type_params.iter().any(|p| p.is_temporal) => {
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    fn register_function_signature(&mut self, func: &FunDecl) -> Result<(), TypeError> {
        Self::validate_unique_parameter_names(&format!("function '{}'", func.name), &func.params)?;
        if Self::is_display_intrinsic_name(&func.name) {
            return Err(TypeError::UnsupportedFeature(format!(
                "top-level function name '{}' is compiler-reserved by the Display output intrinsics",
                func.name
            )));
        }
        self.validate_declared_form_bounds(&func.type_params)?;
        // Push type parameter scope for generics
        self.push_type_param_scope(&func.type_params);

        let mut param_types = Vec::new();
        for param in &func.params {
            let ty = self.convert_type(&param.ty)?;
            param_types.push((param.name.clone(), ty));
        }

        // Annotated return types are part of the public signature and are
        // available to forward references. Unannotated functions get an
        // explicit provisional inference variable until the body is checked.
        let return_type = if let Some(return_type) = &func.return_type {
            self.convert_type(return_type)?
        } else {
            self.type_var_generator.fresh_var()
        };

        self.functions.insert(
            func.name.clone(),
            FunctionDef {
                params: param_types,
                return_type,
                type_params: func.type_params.clone(),
                temporal_constraints: func
                    .temporal_constraints
                    .iter()
                    .map(|c| TemporalConstraint {
                        inner: c.inner.clone(),
                        outer: c.outer.clone(),
                    })
                    .collect(),
            },
        );

        if func.return_type.is_none() {
            self.provisional_function_returns.insert(func.name.clone());
        }

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
            TopDecl::Enum(_) => Ok(()),
            TopDecl::Form(_) | TopDecl::Takes(_) => Ok(()),
            TopDecl::Function(func) => self.check_function_decl(func),
            TopDecl::Binding(bind) => self.check_bind_decl(bind),
            TopDecl::Impl(impl_block) => self.check_impl_block(impl_block),
            TopDecl::Context(context) => self.check_context_decl(context),
            TopDecl::Export(export_decl) => self.check_top_decl(&export_decl.item),
        }
    }

    fn check_record_decl(&mut self, record: &RecordDecl) -> Result<(), TypeError> {
        if self.enums.contains_key(&record.name) || self.forms.contains_key(&record.name) {
            return Err(TypeError::UnsupportedFeature(format!(
                "duplicate type declaration '{}'",
                record.name
            )));
        }

        self.validate_declared_form_bounds(&record.type_params)?;

        // Register temporal type parameters
        for type_param in &record.type_params {
            if type_param.is_temporal {
                self.temporal_context
                    .active_temporals
                    .insert(type_param.name.clone());
            }
        }

        // Register temporal constraints
        for constraint in &record.temporal_constraints {
            self.temporal_context.constraints.push(TemporalConstraint {
                inner: constraint.inner.clone(),
                outer: constraint.outer.clone(),
            });
            // Validate constraint: both temporals should be defined
            if !self
                .temporal_context
                .active_temporals
                .contains(&constraint.inner)
                || !self
                    .temporal_context
                    .active_temporals
                    .contains(&constraint.outer)
            {
                return Err(TypeError::InvalidTemporalConstraint(
                    constraint.inner.clone(),
                    constraint.outer.clone(),
                ));
            }
        }

        self.push_type_param_scope(&record.type_params);
        let field_order = record
            .fields
            .iter()
            .map(|field| field.name.clone())
            .collect::<Vec<_>>();
        let fields = record
            .fields
            .iter()
            .map(|field| {
                let ty = self.convert_type(&field.ty)?;
                Ok((field.name.clone(), ty))
            })
            .collect::<Result<HashMap<_, _>, TypeError>>();
        self.pop_type_param_scope();
        let fields = fields?;

        self.records.insert(
            record.name.clone(),
            RecordDef {
                fields,
                field_order,
                type_params: record.type_params.clone(),
                temporal_constraints: record
                    .temporal_constraints
                    .iter()
                    .map(|c| TemporalConstraint {
                        inner: c.inner.clone(),
                        outer: c.outer.clone(),
                    })
                    .collect(),
                hash: None, // Records don't have their own hash at compile time
                parent_hash: record.parent_hash.clone(),
            },
        );

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
                self.temporal_context
                    .active_temporals
                    .insert(type_param.name.clone());
            }
        }

        // Register temporal constraints
        for constraint in &func.temporal_constraints {
            self.temporal_context.constraints.push(TemporalConstraint {
                inner: constraint.inner.clone(),
                outer: constraint.outer.clone(),
            });
            // Validate constraint
            if !self
                .temporal_context
                .active_temporals
                .contains(&constraint.inner)
                || !self
                    .temporal_context
                    .active_temporals
                    .contains(&constraint.outer)
            {
                return Err(TypeError::InvalidTemporalConstraint(
                    constraint.inner.clone(),
                    constraint.outer.clone(),
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

        let expected_return_type = func
            .return_type
            .as_ref()
            .map(|return_type| self.convert_type(return_type))
            .transpose()?;
        let body_return_type =
            self.check_block_expr_with_expected(&func.body, expected_return_type.as_ref())?;

        if let Some(expected_return_type) = &expected_return_type {
            if !self.type_matches_expected(expected_return_type, &body_return_type) {
                return Err(typed_type_mismatch(expected_return_type, &body_return_type));
            }
        }

        let return_type = expected_return_type.unwrap_or(body_return_type);
        Self::reject_unresolved_return_type("function", &func.name, &return_type)?;
        self.provisional_function_returns.remove(&func.name);

        // Check for temporal escape in return type
        if let TypedType::Temporal { temporals, .. } = &return_type {
            for temporal in temporals {
                if self.temporal_context.active_temporals.contains(temporal) {
                    // Temporal variable from function scope escaping
                    return Err(TypeError::TemporalEscape {
                        temporal: temporal.clone(),
                        message: format!("Temporal parameter {} escapes function scope", temporal),
                    });
                }
            }
        }

        self.functions.insert(
            func.name.clone(),
            FunctionDef {
                params: param_types,
                return_type,
                type_params: func.type_params.clone(),
                temporal_constraints: func
                    .temporal_constraints
                    .iter()
                    .map(|c| TemporalConstraint {
                        inner: c.inner.clone(),
                        outer: c.outer.clone(),
                    })
                    .collect(),
            },
        );

        self.pop_scope();
        self.pop_type_param_scope();

        // Clear temporal context
        self.temporal_context.active_temporals.clear();
        self.temporal_context.constraints.clear();

        Ok(())
    }

    fn check_bind_decl(&mut self, bind: &BindDecl) -> Result<(), TypeError> {
        self.check_bind_decl_with_expected(bind, None)
    }

    fn check_bind_decl_with_expected(
        &mut self,
        bind: &BindDecl,
        contextual_expected_ty: Option<&TypedType>,
    ) -> Result<(), TypeError> {
        let annotated_ty = bind
            .type_annotation
            .as_ref()
            .map(|annotation| self.convert_type(annotation))
            .transpose()?;
        let inherits_deferred_origin = annotated_ty.is_none()
            && matches!(&bind.pattern, Pattern::Ident(_))
            && matches!(
                &bind.value.kind,
                ExprKind::Ident(source) if self.peek_was_deferred_callable(source)
            );
        let inferred_expected_ty = if annotated_ty.is_none()
            && contextual_expected_ty.is_none()
            && matches!(bind.pattern, Pattern::Ident(_))
            && Self::expr_requires_expected_type(&bind.value)
        {
            Some(self.type_var_generator.fresh_var())
        } else {
            None
        };
        let expected_ty = annotated_ty
            .as_ref()
            .or(contextual_expected_ty)
            .or(inferred_expected_ty.as_ref());
        let mut deferred_binding = None;
        let mut deferred_alias_group_id = None;
        let can_defer_unannotated_callable = annotated_ty.is_none()
            && contextual_expected_ty.is_none()
            && matches!(bind.pattern, Pattern::Ident(_))
            && self.can_defer_callable_expr(&bind.value);
        let ty = if can_defer_unannotated_callable {
            let (ty, deferred, alias_group_id) =
                self.check_deferred_callable_binding(&bind.value)?;
            deferred_binding = deferred;
            deferred_alias_group_id = alias_group_id;
            ty
        } else if annotated_ty.is_none()
            && contextual_expected_ty.is_none()
            && matches!(bind.pattern, Pattern::Ident(_))
            && self.branch_expr_has_terminal_lambda(&bind.value)
        {
            match self.check_deferred_callable_binding(&bind.value) {
                Err(err) => return Err(err),
                Ok(_) => return Err(TypeError::CannotInferType(
                    "lambda-producing branch bindings require replay-safe conditions and prefixes"
                        .to_string(),
                )),
            }
        } else {
            self.check_expr_with_expected(&bind.value, expected_ty)?
        };

        if let Some(expected_ty) = expected_ty {
            if !Self::contains_inference_internal_type(expected_ty)
                && !self.type_matches_expected(expected_ty, &ty)
            {
                return Err(typed_type_mismatch(expected_ty, &ty));
            }
        }

        self.check_pattern(&bind.pattern, &ty)?;

        // Handle pattern binding
        if let (Pattern::Ident(name), Some(deferred)) = (&bind.pattern, deferred_binding) {
            self.bind_var_with_deferred(
                name.clone(),
                ty,
                bind.mutable,
                Some(deferred),
                deferred_alias_group_id,
            )?;
        } else {
            self.bind_pattern(&bind.pattern, &ty, bind.mutable)?;
        }
        if let (true, Pattern::Ident(name)) = (inherits_deferred_origin, &bind.pattern) {
            self.mark_var_as_deferred_origin(name)?;
        }

        if annotated_ty.is_none()
            && contextual_expected_ty.is_none()
            && matches!(
                (&bind.pattern, &bind.value.kind),
                (Pattern::Ident(_), ExprKind::ListLit(_))
            )
        {
            if let Pattern::Ident(name) = &bind.pattern {
                self.mark_flexible_collection_literal(name);
            }
        }
        Ok(())
    }

    fn check_deferred_callable_binding(
        &mut self,
        expr: &Expr,
    ) -> Result<(TypedType, Option<DeferredBinding>, Option<u64>), TypeError> {
        if let ExprKind::Lambda(lambda) = &expr.kind {
            let (ty, deferred) = self.check_deferred_lambda_binding(lambda)?;
            let alias_group_id = deferred
                .as_ref()
                .map(|_| self.fresh_deferred_alias_group_id());
            return Ok((ty, deferred, alias_group_id));
        }

        match &expr.kind {
            ExprKind::Ident(name) => {
                if let Some(deferred) = self.peek_deferred_callable(name) {
                    let alias_group_id =
                        self.peek_deferred_alias_group_id(name).ok_or_else(|| {
                            TypeError::CannotInferType(format!(
                                "deferred callable binding '{name}' is missing alias provenance"
                            ))
                        })?;
                    let ty = self.lookup_var(name)?;
                    self.transfer_deferred_callable(name)?;
                    return Ok((ty, Some(deferred), Some(alias_group_id)));
                }

                if let Some(definition) = self
                    .functions
                    .get(name)
                    .filter(|definition| !definition.type_params.is_empty())
                    .cloned()
                {
                    let ty = self.named_function_value_type(name, &definition, None)?;
                    let alias_group_id = self.fresh_deferred_alias_group_id();
                    return Ok((
                        ty,
                        Some(DeferredBinding::GenericFunction {
                            name: name.clone(),
                            definition,
                        }),
                        Some(alias_group_id),
                    ));
                }

                Err(TypeError::CannotInferType(format!(
                    "deferred callable binding '{name}' is not generic"
                )))
            }
            ExprKind::Then(then) => {
                let (ty, deferred) = self.check_then_expr_as_deferred_callable(then)?;
                let alias_group_id = deferred
                    .as_ref()
                    .map(|_| self.fresh_deferred_alias_group_id());
                Ok((ty, deferred, alias_group_id))
            }
            ExprKind::Match(match_expr) => {
                let (ty, deferred) = self.check_match_expr_as_deferred_callable(match_expr)?;
                let alias_group_id = deferred
                    .as_ref()
                    .map(|_| self.fresh_deferred_alias_group_id());
                Ok((ty, deferred, alias_group_id))
            }
            _ => Err(TypeError::CannotInferType(
                "deferred callable binding requires a lambda-producing expression".to_string(),
            )),
        }
    }

    fn check_deferred_lambda_binding(
        &mut self,
        lambda: &LambdaExpr,
    ) -> Result<(TypedType, Option<DeferredBinding>), TypeError> {
        let bound_vars = HashSet::new();
        let free_vars = self.collect_free_variables(&lambda.body, &bound_vars);
        let allowed_temporals = self.temporal_context.active_temporals.clone();

        for var_name in &free_vars {
            if let Some(var_type) = self.peek_var_type(var_name) {
                self.check_temporal_escape(&var_type, &allowed_temporals)?;
            }
        }

        if free_vars.is_empty() && Self::expr_requires_expected_type(&lambda.body) {
            return self.deferred_lambda_placeholder(lambda);
        }

        self.push_scope();

        for param in &lambda.params {
            let param_type = if let Some(type_annotation) = &param.type_annotation {
                self.convert_type(type_annotation)?
            } else {
                self.type_var_generator.fresh_var()
            };
            self.bind_var(param.name.clone(), param_type, false)?;
        }

        let inferred_return_type = if Self::expr_requires_expected_type(&lambda.body) {
            Some(self.type_var_generator.fresh_var())
        } else {
            None
        };
        let body_result =
            self.check_expr_with_expected(&lambda.body, inferred_return_type.as_ref());
        let param_types = lambda
            .params
            .iter()
            .map(|param| self.peek_var_type(&param.name))
            .collect::<Option<Vec<_>>>()
            .unwrap_or_else(|| {
                lambda
                    .params
                    .iter()
                    .map(|_| self.type_var_generator.fresh_var())
                    .collect()
            });
        self.pop_scope();

        let body_type = match body_result {
            Ok(body_type) => body_type,
            Err(err) if free_vars.is_empty() && Self::can_defer_contextless_lambda_error(&err) => {
                return self.deferred_lambda_placeholder(lambda);
            }
            Err(err) => return Err(err),
        };
        let func_type = TypedType::Function {
            params: param_types,
            return_type: Box::new(body_type),
        };

        self.check_temporal_escape(&func_type, &allowed_temporals)?;

        Ok((func_type, None))
    }

    fn can_defer_contextless_lambda_error(err: &TypeError) -> bool {
        match err {
            TypeError::CannotInferType(_) => true,
            TypeError::TypeMismatch { expected, found } => {
                [expected.as_str(), found.as_str()].iter().any(|message| {
                    message.contains("InferVar")
                        || message.contains("Projection")
                        || message.contains('?')
                })
            }
            TypeError::UnresolvedProjection(_) => true,
            _ => false,
        }
    }

    fn format_type_pair(left: &TypedType, right: &TypedType) -> String {
        format!(
            "{} and {}",
            format_typed_type(left),
            format_typed_type(right)
        )
    }

    fn deferred_lambda_placeholder(
        &mut self,
        lambda: &LambdaExpr,
    ) -> Result<(TypedType, Option<DeferredBinding>), TypeError> {
        let params = lambda
            .params
            .iter()
            .map(|param| {
                param
                    .type_annotation
                    .as_ref()
                    .map(|annotation| self.convert_type(annotation))
                    .transpose()
                    .map(|ty| ty.unwrap_or_else(|| self.type_var_generator.fresh_var()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let func_type = TypedType::Function {
            params,
            return_type: Box::new(self.type_var_generator.fresh_var()),
        };
        // The stored clone is re-checked per use site under use-site expected
        // types; strip ids so those instantiation-specific re-checks cannot
        // record facts under the source lambda body's node ids.
        let mut deferred_lambda = lambda.clone();
        strip_expr_ids(&mut deferred_lambda.body);
        Ok((func_type, Some(DeferredBinding::Lambda(deferred_lambda))))
    }

    fn check_then_expr_as_deferred_callable(
        &mut self,
        then: &ThenExpr,
    ) -> Result<(TypedType, Option<DeferredBinding>), TypeError> {
        let cond_ty = self.check_expr(&then.condition)?;
        if cond_ty != TypedType::Boolean {
            return Err(expected_type_mismatch("Boolean", &cond_ty));
        }

        let branch_base = self.var_env.clone();
        let mut branch_envs = Vec::new();
        let mut candidates = Vec::new();

        let (then_candidate, then_env) = self.check_branch_from_env(&branch_base, |checker| {
            checker.push_scope();
            let result = checker.check_block_as_deferred_callable_result(&then.then_block);
            checker.pop_scope();
            result
        })?;
        branch_envs.push(then_env);
        candidates.push(then_candidate);

        for (else_cond, else_block) in &then.else_ifs {
            let (else_if_candidate, else_if_env) =
                self.check_branch_from_env(&branch_base, |checker| {
                    let else_cond_ty = checker.check_expr(else_cond)?;
                    if else_cond_ty != TypedType::Boolean {
                        return Err(expected_type_mismatch("Boolean", &else_cond_ty));
                    }

                    checker.push_scope();
                    let result = checker.check_block_as_deferred_callable_result(else_block);
                    checker.pop_scope();
                    result
                })?;
            branch_envs.push(else_if_env);
            candidates.push(else_if_candidate);
        }

        let else_block = then.else_block.as_ref().ok_or_else(|| {
            TypeError::CannotInferType(
                "lambda-producing then expressions require an else branch".to_string(),
            )
        })?;
        let (else_candidate, else_env) = self.check_branch_from_env(&branch_base, |checker| {
            checker.push_scope();
            let result = checker.check_block_as_deferred_callable_result(else_block);
            checker.pop_scope();
            result
        })?;
        branch_envs.push(else_env);
        candidates.push(else_candidate);

        let ty = self.placeholder_for_deferred_candidates(&candidates)?;
        self.merge_branch_var_usage(branch_base, &branch_envs)?;

        if let Some(anchored_ty) = self.resolve_anchored_deferred_candidates(&candidates)? {
            return Ok((anchored_ty, None));
        }

        Ok((
            ty,
            Some(DeferredBinding::BranchCallable(DeferredBranchCallable {
                candidates,
            })),
        ))
    }

    fn check_match_expr_as_deferred_callable(
        &mut self,
        match_expr: &MatchExpr,
    ) -> Result<(TypedType, Option<DeferredBinding>), TypeError> {
        let scrutinee_type = self.check_expr(&match_expr.expr)?;

        if match_expr.arms.is_empty() {
            return Err(TypeError::TypeMismatch {
                expected: "at least one match arm".to_string(),
                found: "no match arms".to_string(),
            });
        }

        let branch_base = self.var_env.clone();
        let mut branch_envs = Vec::new();
        let mut candidates = Vec::new();

        for arm in &match_expr.arms {
            self.check_pattern(&arm.pattern, &scrutinee_type)?;

            let (candidate, arm_env) = self.check_branch_from_env(&branch_base, |checker| {
                checker.push_scope();
                let result = (|| {
                    checker.bind_pattern_vars(&arm.pattern, &scrutinee_type)?;
                    checker.check_block_as_deferred_callable_result(&arm.body)
                })();
                checker.pop_scope();
                result
            })?;
            branch_envs.push(arm_env);
            candidates.push(candidate);
        }

        if !self.is_pattern_exhaustive(&match_expr.arms, &scrutinee_type) {
            if let Err(missing_patterns) =
                self.check_exhaustiveness_coverage(&match_expr.arms, &scrutinee_type)
            {
                return Err(TypeError::NonExhaustivePatterns {
                    missing: missing_patterns.join(", "),
                    suggestion: "Add the missing patterns or use a wildcard pattern (_)"
                        .to_string(),
                });
            }

            return Err(TypeError::TypeMismatch {
                expected: "exhaustive patterns".to_string(),
                found: "non-exhaustive patterns".to_string(),
            });
        }

        let ty = self.placeholder_for_deferred_candidates(&candidates)?;
        self.merge_branch_var_usage(branch_base, &branch_envs)?;

        if let Some(anchored_ty) = self.resolve_anchored_deferred_candidates(&candidates)? {
            return Ok((anchored_ty, None));
        }

        Ok((
            ty,
            Some(DeferredBinding::BranchCallable(DeferredBranchCallable {
                candidates,
            })),
        ))
    }

    fn check_block_as_deferred_callable_result(
        &mut self,
        block: &BlockExpr,
    ) -> Result<DeferredCallableCandidate, TypeError> {
        for stmt in self.deferred_callable_prefix_statements(block) {
            match stmt {
                Stmt::Binding(bind) => {
                    let Pattern::Ident(name) = &bind.pattern else {
                        return Err(TypeError::CannotInferType(
                            "deferred callable branch prefix bindings must use simple identifiers"
                                .to_string(),
                        ));
                    };
                    if bind.mutable {
                        return Err(TypeError::CannotInferType(
                            "deferred callable branch prefix bindings cannot be mutable"
                                .to_string(),
                        ));
                    }
                    if !self.expr_is_replay_safe_for_deferred_callable(&bind.value) {
                        return Err(TypeError::CannotInferType(
                            "deferred callable branch prefix bindings must be replay-safe"
                                .to_string(),
                        ));
                    }

                    self.check_bind_decl_with_expected(bind, None)?;
                    let Some(bound_ty) = self.peek_var_type(name) else {
                        return Err(TypeError::UndefinedVariable(name.clone()));
                    };
                    if !self.is_copyable(&bound_ty) {
                        return Err(TypeError::CannotInferType(format!(
                            "deferred callable branch prefix binding '{}' must have a Copy type",
                            name
                        )));
                    }
                }
                Stmt::Assignment(_) | Stmt::Expr(_) => {
                    return Err(TypeError::CannotInferType(
                        "deferred callable branch prefixes support only replay-safe val bindings"
                            .to_string(),
                    ));
                }
            }
        }

        if let Some(lambda) = self.block_terminal_lambda(block).cloned() {
            self.reject_unresolved_inference_in_current_scope()?;
            return self
                .deferred_lambda_candidate_from_current_scope(&lambda)
                .map(DeferredCallableCandidate::Lambda);
        }

        let Some(expr) = self.block_terminal_expr(block) else {
            return Err(TypeError::CannotInferType(
                "deferred callable branch blocks require a callable result".to_string(),
            ));
        };

        let ty = self.check_expr(expr)?;
        let TypedType::Function { .. } = ty else {
            return Err(TypeError::CannotInferType(
                "deferred callable branch blocks require a function-typed result".to_string(),
            ));
        };
        if Self::contains_inference_internal_type(&ty) {
            return Err(TypeError::CannotInferType(
                "function-typed branch result still has unresolved inference variables".to_string(),
            ));
        }
        self.reject_unresolved_inference_in_current_scope()?;

        Ok(DeferredCallableCandidate::Typed(ty))
    }

    fn deferred_lambda_candidate_from_current_scope(
        &mut self,
        lambda: &LambdaExpr,
    ) -> Result<DeferredLambdaCandidate, TypeError> {
        let mut bound_vars = HashSet::new();
        for param in &lambda.params {
            bound_vars.insert(param.name.clone());
        }

        let free_vars = self.collect_free_variables(&lambda.body, &bound_vars);
        let allowed_temporals = self.temporal_context.active_temporals.clone();
        let mut captures = Vec::new();

        for var_name in free_vars {
            let Some(var_type) = self.peek_var_type(&var_name) else {
                continue;
            };
            self.check_temporal_escape(&var_type, &allowed_temporals)?;

            if self.current_scope_contains_var(&var_name) {
                captures.push((var_name, var_type));
            } else {
                return Err(TypeError::CannotInferType(format!(
                    "deferred lambda branch outer binding capture '{}' is not supported; bind a Copy prefix inside each branch or add an explicit function type annotation",
                    var_name
                )));
            }
        }

        // Same id discipline as DeferredBinding::Lambda: per-use-site
        // re-checks of this clone must not write source-node facts.
        let mut deferred_lambda = lambda.clone();
        strip_expr_ids(&mut deferred_lambda.body);
        Ok(DeferredLambdaCandidate {
            lambda: deferred_lambda,
            captures,
        })
    }

    fn placeholder_for_deferred_candidates(
        &mut self,
        candidates: &[DeferredCallableCandidate],
    ) -> Result<TypedType, TypeError> {
        let first = candidates.first().ok_or_else(|| {
            TypeError::CannotInferType(
                "deferred branch callable has no callable candidates".to_string(),
            )
        })?;

        let placeholder = match first {
            DeferredCallableCandidate::Lambda(candidate) => {
                self.deferred_lambda_placeholder(&candidate.lambda)?.0
            }
            DeferredCallableCandidate::Typed(ty) => ty.clone(),
        };

        if let TypedType::Function { params, .. } = &placeholder {
            for candidate in candidates.iter().skip(1) {
                if let Some(candidate_arity) = Self::deferred_callable_candidate_arity(candidate) {
                    if candidate_arity != params.len() {
                        return Err(TypeError::ArityMismatch {
                            expected: params.len(),
                            found: candidate_arity,
                        });
                    }
                } else {
                    return Err(TypeError::ArityMismatch {
                        expected: params.len(),
                        found: 0,
                    });
                }
            }
        }

        Ok(placeholder)
    }

    fn resolve_anchored_deferred_candidates(
        &mut self,
        candidates: &[DeferredCallableCandidate],
    ) -> Result<Option<TypedType>, TypeError> {
        let Some(anchor) = candidates.iter().find_map(|candidate| match candidate {
            DeferredCallableCandidate::Typed(ty) => Some(ty.clone()),
            DeferredCallableCandidate::Lambda(_) => None,
        }) else {
            return Ok(None);
        };

        let mut substitution = ConstraintSubstitution::new();
        let mut resolved = anchor;
        for candidate in candidates {
            let candidate_ty = self.check_deferred_callable_candidate_against_expected(
                candidate,
                &resolved,
                &mut substitution,
            )?;
            unify_constraint(&resolved, &candidate_ty, &mut substitution)?;
            resolved = substitution.apply(&resolved)?;
        }

        Ok(Some(resolved))
    }

    fn deferred_callable_candidate_arity(candidate: &DeferredCallableCandidate) -> Option<usize> {
        match candidate {
            DeferredCallableCandidate::Lambda(candidate) => Some(candidate.lambda.params.len()),
            DeferredCallableCandidate::Typed(TypedType::Function { params, .. }) => {
                Some(params.len())
            }
            DeferredCallableCandidate::Typed(_) => None,
        }
    }

    fn current_scope_contains_var(&self, name: &str) -> bool {
        self.var_env
            .last()
            .is_some_and(|scope| scope.contains_key(name))
    }

    fn can_defer_callable_expr(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Lambda(_) => true,
            ExprKind::Ident(name) => {
                self.peek_deferred_callable(name).is_some()
                    || self
                        .functions
                        .get(name)
                        .is_some_and(|definition| !definition.type_params.is_empty())
            }
            ExprKind::Then(then) => {
                self.branch_expr_has_terminal_lambda(expr)
                    && self.expr_is_replay_safe_for_deferred_callable(&then.condition)
                    && then.else_ifs.iter().all(|(condition, block)| {
                        self.expr_is_replay_safe_for_deferred_callable(condition)
                            && self.block_result_is_deferred_callable(block)
                    })
                    && self.block_result_is_deferred_callable(&then.then_block)
                    && then
                        .else_block
                        .as_ref()
                        .is_some_and(|block| self.block_result_is_deferred_callable(block))
            }
            ExprKind::Match(match_expr) => {
                self.branch_expr_has_terminal_lambda(expr)
                    && !match_expr.arms.is_empty()
                    && match_expr
                        .arms
                        .iter()
                        .all(|arm| self.block_result_is_deferred_callable(&arm.body))
            }
            _ => false,
        }
    }

    fn block_result_is_deferred_callable(&self, block: &BlockExpr) -> bool {
        self.block_terminal_expr(block).is_some()
            && self
                .deferred_callable_prefix_statements(block)
                .iter()
                .all(|stmt| self.stmt_is_deferred_callable_prefix(stmt))
    }

    fn branch_expr_has_terminal_lambda(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Then(then) => {
                self.block_terminal_lambda(&then.then_block).is_some()
                    || then
                        .else_ifs
                        .iter()
                        .any(|(_, block)| self.block_terminal_lambda(block).is_some())
                    || then
                        .else_block
                        .as_ref()
                        .is_some_and(|block| self.block_terminal_lambda(block).is_some())
            }
            ExprKind::Match(match_expr) => match_expr
                .arms
                .iter()
                .any(|arm| self.block_terminal_lambda(&arm.body).is_some()),
            _ => false,
        }
    }

    fn stmt_is_deferred_callable_prefix(&self, stmt: &Stmt) -> bool {
        matches!(
            stmt,
            Stmt::Binding(bind) if !bind.mutable && matches!(bind.pattern, Pattern::Ident(_))
        )
    }

    fn block_terminal_lambda<'a>(&self, block: &'a BlockExpr) -> Option<&'a LambdaExpr> {
        match self.block_terminal_expr(block).map(|expr| &expr.kind) {
            Some(ExprKind::Lambda(lambda)) => Some(lambda),
            _ => None,
        }
    }

    fn block_terminal_expr<'a>(&self, block: &'a BlockExpr) -> Option<&'a Expr> {
        if let Some(expr) = block.expr.as_deref() {
            return Some(expr);
        }

        match block.statements.last() {
            Some(Stmt::Expr(expr)) => Some(expr.as_ref()),
            _ => None,
        }
    }

    fn deferred_callable_prefix_statements<'a>(&self, block: &'a BlockExpr) -> &'a [Stmt] {
        if block.expr.is_some() {
            &block.statements
        } else if matches!(block.statements.last(), Some(Stmt::Expr(_))) {
            &block.statements[..block.statements.len() - 1]
        } else {
            &block.statements
        }
    }

    fn expr_is_replay_safe_for_deferred_callable(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::VariantRef(_) => false,
            ExprKind::IntLit(_)
            | ExprKind::FloatLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::CharLit(_)
            | ExprKind::BoolLit(_)
            | ExprKind::Unit => true,
            ExprKind::Ident(name) => self.ident_is_replay_safe_for_deferred_callable(name),
            ExprKind::Binary(binary) => {
                self.expr_is_replay_safe_for_deferred_callable(&binary.left)
                    && self.expr_is_replay_safe_for_deferred_callable(&binary.right)
            }
            ExprKind::Unary(unary) => self.expr_is_replay_safe_for_deferred_callable(&unary.expr),
            ExprKind::Cast(cast) => self.expr_is_replay_safe_for_deferred_callable(&cast.expr),
            ExprKind::ListLit(elements) | ExprKind::ArrayLit(elements) => elements
                .iter()
                .all(|element| self.expr_is_replay_safe_for_deferred_callable(element)),
            ExprKind::RangeLit(range) => {
                self.expr_is_replay_safe_for_deferred_callable(&range.start)
                    && self.expr_is_replay_safe_for_deferred_callable(&range.end)
            }
            _ => false,
        }
    }

    fn ident_is_replay_safe_for_deferred_callable(&self, name: &str) -> bool {
        self.var_env
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .is_some_and(|var| !var.mutable && self.is_copyable(&var.ty))
    }

    fn type_matches_expected(&self, expected: &TypedType, actual: &TypedType) -> bool {
        match (expected, actual) {
            (
                TypedType::Array(expected_elem, expected_size),
                TypedType::Array(actual_elem, actual_size),
            ) if self.array_lengths_match_for_expected(*expected_size, *actual_size) => {
                self.type_matches_expected(expected_elem, actual_elem)
            }
            (TypedType::List(expected_elem), TypedType::List(actual_elem)) => {
                self.type_matches_expected(expected_elem, actual_elem)
            }
            (TypedType::Option(expected_inner), TypedType::Option(actual_inner)) => {
                self.type_matches_expected(expected_inner, actual_inner)
            }
            (
                TypedType::Result(expected_ok, expected_err),
                TypedType::Result(actual_ok, actual_err),
            ) => {
                self.type_matches_expected(expected_ok, actual_ok)
                    && self.type_matches_expected(expected_err, actual_err)
            }
            (
                TypedType::Function {
                    params: expected_params,
                    return_type: expected_return,
                },
                TypedType::Function {
                    params: actual_params,
                    return_type: actual_return,
                },
            ) => {
                expected_params.len() == actual_params.len()
                    && expected_params
                        .iter()
                        .zip(actual_params.iter())
                        .all(|(expected, actual)| self.type_matches_expected(expected, actual))
                    && self.type_matches_expected(expected_return, actual_return)
            }
            (
                TypedType::Record {
                    name: expected_name,
                    type_args: expected_args,
                    frozen: expected_frozen,
                    hash: expected_hash,
                    parent_hash: expected_parent_hash,
                },
                TypedType::Record {
                    name: actual_name,
                    type_args: actual_args,
                    frozen: actual_frozen,
                    hash: actual_hash,
                    parent_hash: actual_parent_hash,
                },
            ) => {
                expected_name == actual_name
                    && expected_frozen == actual_frozen
                    && expected_hash == actual_hash
                    && expected_parent_hash == actual_parent_hash
                    && expected_args.len() == actual_args.len()
                    && expected_args
                        .iter()
                        .zip(actual_args.iter())
                        .all(|(expected, actual)| self.type_matches_expected(expected, actual))
            }
            (
                TypedType::Temporal {
                    base_type: expected_base,
                    temporals: expected_temporals,
                },
                TypedType::Temporal {
                    base_type: actual_base,
                    temporals: actual_temporals,
                },
            ) => {
                expected_temporals == actual_temporals
                    && self.type_matches_expected(expected_base, actual_base)
            }
            _ => expected == actual,
        }
    }

    fn array_lengths_match_for_expected(&self, expected: ArrayLength, actual: ArrayLength) -> bool {
        match (expected, actual) {
            (ArrayLength::AnyInternal, _) => true,
            (ArrayLength::Known(expected), ArrayLength::Known(actual)) => expected == actual,
            (ArrayLength::Known(_), ArrayLength::AnyInternal) => false,
        }
    }

    fn bind_pattern(
        &mut self,
        pattern: &Pattern,
        ty: &TypedType,
        mutable: bool,
    ) -> Result<(), TypeError> {
        match pattern {
            Pattern::Ident(name) => {
                // Simple variable binding
                if let Ok((_existing_ty, _is_mutable)) = self.lookup_var_for_assignment(name) {
                    // This is a reassignment
                    self.reassign_var(name, ty)?;
                } else {
                    // This is a new binding
                    self.bind_var(name.clone(), ty.clone(), mutable)?;
                }
            }
            Pattern::RecordDestruct {
                type_name,
                fields,
                rest,
            } => {
                // Record destructuring with spread
                match ty {
                    TypedType::Record { name: rec_name, .. } if rec_name == type_name => {
                        let (_, instantiated_fields) = self.instantiated_record_fields(ty)?;
                        let field_types: Vec<(String, TypedType)> = fields
                            .iter()
                            .map(|(field_name, _)| {
                                if let Some(field_ty) = instantiated_fields.get(field_name) {
                                    Ok((field_name.clone(), field_ty.clone()))
                                } else {
                                    Err(TypeError::UnknownField {
                                        record: rec_name.clone(),
                                        field: field_name.clone(),
                                    })
                                }
                            })
                            .collect::<Result<Vec<_>, _>>()?;

                        // Handle rest binding first to avoid borrow issues.
                        let rest_type = if let Some(rest_name) = rest {
                            if rest_name == "_" {
                                None
                            } else {
                                Some(self.ensure_residual_record_type(type_name, fields, ty)?)
                            }
                        } else {
                            None
                        };

                        // Bind each extracted field
                        for ((_, field_pattern), (_, field_ty)) in
                            fields.iter().zip(field_types.iter())
                        {
                            self.bind_pattern(field_pattern, field_ty, mutable)?;
                        }

                        // If there's a rest binding, bind it
                        if let (Some(rest_name), Some(rest_ty)) = (rest, rest_type) {
                            self.bind_var(rest_name.clone(), rest_ty, false)?;
                        }

                        // Consume the original record value (affine semantics)
                        // This happens automatically through the check_expr call
                    }
                    _ => return Err(expected_type_mismatch(type_name.clone(), ty)),
                }
            }
            Pattern::Record(rec_name, fields) => {
                // Old-style record pattern
                match ty {
                    TypedType::Record { name: ty_name, .. } if ty_name == rec_name => {
                        let (_, instantiated_fields) = self.instantiated_record_fields(ty)?;
                        let field_types: Vec<(String, TypedType)> = fields
                            .iter()
                            .map(|(field_name, _)| {
                                if let Some(field_ty) = instantiated_fields.get(field_name) {
                                    Ok((field_name.clone(), field_ty.clone()))
                                } else {
                                    Err(TypeError::UnknownField {
                                        record: ty_name.clone(),
                                        field: field_name.clone(),
                                    })
                                }
                            })
                            .collect::<Result<Vec<_>, _>>()?;

                        for ((_, field_pattern), (_, field_ty)) in
                            fields.iter().zip(field_types.iter())
                        {
                            self.bind_pattern(field_pattern, field_ty, mutable)?;
                        }
                    }
                    _ => return Err(expected_type_mismatch(rec_name.clone(), ty)),
                }
            }
            Pattern::ListCons(head, tail) => {
                // List cons pattern [head | tail]
                match ty {
                    TypedType::List(elem_ty) => {
                        self.bind_pattern(head, elem_ty, mutable)?;
                        self.bind_pattern(tail, ty, mutable)?;
                    }
                    _ => return Err(expected_type_mismatch("List", ty)),
                }
            }
            Pattern::ListExact(patterns) => {
                // Exact list pattern [a, b, c]
                match ty {
                    TypedType::List(elem_ty) => {
                        for pattern in patterns {
                            self.bind_pattern(pattern, elem_ty, mutable)?;
                        }
                    }
                    _ => return Err(expected_type_mismatch("List", ty)),
                }
            }
            Pattern::Some(inner) => {
                // Option::Some pattern
                match ty {
                    TypedType::Option(inner_ty) => {
                        self.bind_pattern(inner, inner_ty, mutable)?;
                    }
                    _ => return Err(expected_type_mismatch("Option", ty)),
                }
            }
            Pattern::Ok(inner) => match ty {
                TypedType::Result(ok_ty, _) => {
                    self.bind_pattern(inner, ok_ty, mutable)?;
                }
                _ => return Err(expected_type_mismatch("Result", ty)),
            },
            Pattern::Err(inner) => match ty {
                TypedType::Result(_, err_ty) => {
                    self.bind_pattern(inner, err_ty, mutable)?;
                }
                _ => return Err(expected_type_mismatch("Result", ty)),
            },
            Pattern::EnumVariant {
                enum_name,
                variant_name,
                ..
            } => {
                return Err(TypeError::UnsupportedFeature(format!(
                    "refutable enum pattern '{}::{}' is not allowed in a val binding; use match",
                    enum_name, variant_name
                )));
            }
            Pattern::None | Pattern::EmptyList | Pattern::Wildcard | Pattern::Literal(_) => {
                // These patterns don't bind variables
            }
        }
        Ok(())
    }

    fn check_assignment(&mut self, assign: &AssignStmt) -> Result<(), TypeError> {
        let (target_ty, mutable) = self.lookup_var_for_assignment(&assign.name)?;
        if !mutable {
            return Err(TypeError::ImmutableReassignment(assign.name.clone()));
        }
        self.reject_deferred_alias_reassignment(&assign.name)?;

        let value_ty = self.check_expr_with_expected(&assign.value, Some(&target_ty))?;
        let resolved_target_ty = if Self::contains_inference_internal_type(&target_ty)
            || Self::contains_inference_internal_type(&value_ty)
        {
            let mut substitution = ConstraintSubstitution::new();
            unify_constraint(&target_ty, &value_ty, &mut substitution)?;
            let resolved = substitution.apply(&target_ty)?;
            self.apply_substitution_to_var_env(&substitution)?;
            resolved
        } else {
            if !self.type_matches_expected(&target_ty, &value_ty) {
                return Err(typed_type_mismatch(&target_ty, &value_ty));
            }
            target_ty
        };

        self.reassign_var(&assign.name, &resolved_target_ty)
    }

    fn impl_method_param_types(
        &mut self,
        target: &str,
        func: &FunDecl,
    ) -> Result<Vec<(String, TypedType)>, TypeError> {
        self.validate_impl_receiver_param(target, func)?;

        let mut param_types = Vec::new();
        for (i, param) in func.params.iter().enumerate() {
            let ty = if i == 0 && param.name == "self" {
                TypedType::Record {
                    name: target.to_string(),
                    type_args: Vec::new(),
                    frozen: false,
                    hash: None,
                    parent_hash: None,
                }
            } else {
                self.convert_type(&param.ty)?
            };
            param_types.push((param.name.clone(), ty));
        }
        Ok(param_types)
    }

    fn validate_impl_receiver_param(&self, target: &str, func: &FunDecl) -> Result<(), TypeError> {
        let Some(param) = func.params.first() else {
            return Err(TypeError::UnsupportedFeature(format!(
                "Impl method '{}' for record '{}' must declare first parameter as self: {}",
                func.name, target, target
            )));
        };

        if param.name != "self" || param.ty != crate::ast::Type::Named(target.to_string()) {
            return Err(TypeError::UnsupportedFeature(format!(
                "Impl method '{}' for record '{}' must declare first parameter as self: {}",
                func.name, target, target
            )));
        }

        Ok(())
    }

    fn register_impl_method_signatures(&mut self, impl_block: &ImplBlock) -> Result<(), TypeError> {
        if !self.records.contains_key(&impl_block.target) {
            return Err(TypeError::UndefinedRecord(impl_block.target.clone()));
        }

        let target = impl_block.target.clone();
        for func in &impl_block.functions {
            self.validate_declared_form_bounds(&func.type_params)?;
            Self::validate_unique_parameter_names(
                &format!("impl method '{}::{}'", target, func.name),
                &func.params,
            )?;
            if Self::is_display_intrinsic_name(&func.name) {
                return Err(TypeError::UnsupportedFeature(format!(
                    "impl method selector '{}' is compiler-reserved by the Display output intrinsics",
                    func.name
                )));
            }
            if self
                .methods
                .get(&target)
                .and_then(|method_map| method_map.get(&func.name))
                .is_some()
            {
                return Err(TypeError::UnsupportedFeature(format!(
                    "Duplicate method '{}' for record '{}'",
                    func.name, target
                )));
            }

            self.push_type_param_scope(&func.type_params);
            let signature_result = (|| {
                let param_types = self.impl_method_param_types(&target, func)?;
                let return_type = if let Some(return_type) = &func.return_type {
                    self.convert_type(return_type)?
                } else {
                    self.type_var_generator.fresh_var()
                };

                Ok(FunctionDef {
                    params: param_types,
                    return_type,
                    type_params: func.type_params.clone(),
                    temporal_constraints: func
                        .temporal_constraints
                        .iter()
                        .map(|c| TemporalConstraint {
                            inner: c.inner.clone(),
                            outer: c.outer.clone(),
                        })
                        .collect(),
                })
            })();
            self.pop_type_param_scope();

            let method_def = signature_result?;
            if func.return_type.is_none() {
                self.provisional_method_returns
                    .insert((target.clone(), func.name.clone()));
            }
            self.methods
                .entry(target.clone())
                .or_default()
                .insert(func.name.clone(), method_def);
        }

        Ok(())
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
            self.push_type_param_scope(&func.type_params);
            self.push_scope();

            let param_types = self.impl_method_param_types(&target, func)?;
            for (param_name, ty) in &param_types {
                self.bind_var(param_name.clone(), ty.clone(), false)?;
            }

            let expected_return_type = func
                .return_type
                .as_ref()
                .map(|return_type| self.convert_type(return_type))
                .transpose()?;
            let body_return_type =
                self.check_block_expr_with_expected(&func.body, expected_return_type.as_ref())?;

            if let Some(expected_return_type) = &expected_return_type {
                if !self.type_matches_expected(expected_return_type, &body_return_type) {
                    return Err(typed_type_mismatch(expected_return_type, &body_return_type));
                }
            }

            let return_type = expected_return_type.unwrap_or(body_return_type);
            Self::reject_unresolved_return_type("method", &func.name, &return_type)?;
            self.provisional_method_returns
                .remove(&(target.clone(), func.name.clone()));

            let method_map = self.methods.entry(target.clone()).or_default();
            method_map.insert(
                func.name.clone(),
                FunctionDef {
                    params: param_types,
                    return_type,
                    type_params: func.type_params.clone(),
                    temporal_constraints: func
                        .temporal_constraints
                        .iter()
                        .map(|c| TemporalConstraint {
                            inner: c.inner.clone(),
                            outer: c.outer.clone(),
                        })
                        .collect(),
                },
            );

            self.pop_scope();
            self.pop_type_param_scope();
        }
        Ok(())
    }

    fn check_context_decl(&mut self, context: &ContextDecl) -> Result<(), TypeError> {
        if self.enums.contains_key(&context.name) {
            return Err(TypeError::UnsupportedFeature(format!(
                "duplicate type declaration '{}'",
                context.name
            )));
        }

        // Store context definition
        let mut fields = HashMap::new();
        let mut field_order = Vec::new();
        for field in &context.fields {
            let ty = self.convert_type(&field.ty)?;
            field_order.push(field.name.clone());
            fields.insert(field.name.clone(), ty);
        }

        // Add to available contexts
        self._contexts.push(context.name.clone());

        // Store as a special record type for field access
        self.records.insert(
            context.name.clone(),
            RecordDef {
                fields,
                field_order,
                type_params: vec![],
                temporal_constraints: vec![],
                hash: None,
                parent_hash: None,
            },
        );

        Ok(())
    }

    fn check_int_lit(
        &self,
        value: i64,
        expected: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        match expected {
            Some(TypedType::Int64) => Ok(TypedType::Int64),
            Some(TypedType::Int32) => {
                if i32::try_from(value).is_ok() {
                    Ok(TypedType::Int32)
                } else {
                    Err(TypeError::TypeMismatch {
                        expected: "Int32 literal range".to_string(),
                        found: value.to_string(),
                    })
                }
            }
            _ if i32::try_from(value).is_ok() => Ok(TypedType::Int32),
            _ => Ok(TypedType::Int64),
        }
    }

    fn check_expr(&mut self, expr: &Expr) -> Result<TypedType, TypeError> {
        self.check_expr_with_expected(expr, None)
    }

    fn check_expr_with_expected(
        &mut self,
        expr: &Expr,
        expected: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        let result = (|| -> Result<TypedType, TypeError> {
            match &expr.kind {
                ExprKind::IntLit(value) => self.check_int_lit(*value, expected),
                ExprKind::FloatLit(_) => Ok(TypedType::Float64),
                ExprKind::StringLit(_) => Ok(TypedType::String),
                ExprKind::CharLit(_) => Ok(TypedType::Char),
                ExprKind::BoolLit(_) => Ok(TypedType::Boolean),
                ExprKind::Unit => Ok(TypedType::Unit),
                ExprKind::Ident(name) => {
                    // First try as a variable
                    match self.lookup_var(name) {
                        Ok(ty) => {
                            if let Some(expected_ty) = expected {
                                if let Some(deferred) = self.peek_deferred_callable(name) {
                                    if matches!(expected_ty, TypedType::Function { .. }) {
                                        let mut substitution = ConstraintSubstitution::new();
                                        return self.check_deferred_callable_against_expected(
                                            name,
                                            &deferred,
                                            expected_ty,
                                            &mut substitution,
                                        );
                                    }
                                }

                                if let Some(constrained_ty) = self
                                    .constrain_inference_binding_from_expected(
                                        name,
                                        &ty,
                                        expected_ty,
                                    )?
                                {
                                    return Ok(constrained_ty);
                                }
                            }

                            Ok(ty)
                        }
                        Err(e) => {
                            // If not a variable, check if it's a function. In expression
                            // position a function identifier is a function value. A
                            // zero-argument function must still be invoked with the
                            // OSV unit form `() function` when a value is expected.
                            if let Some(func_def) = self.functions.get(name).cloned() {
                                if self.provisional_function_returns.contains(name) {
                                    return Err(TypeError::CannotInferType(format!(
                                    "function '{}' is used before its return type has been inferred; add an explicit return annotation",
                                    name
                                )));
                                }

                                if func_def.params.is_empty()
                                    && expected.is_some()
                                    && !matches!(expected, Some(TypedType::Function { .. }))
                                {
                                    Err(TypeError::UnsupportedFeature(format!(
                                    "zero-argument function '{name}' must be called with OSV unit syntax `() {name}` or used where a `() -> ...` function type is expected"
                                )))
                                } else {
                                    self.named_function_value_type(name, &func_def, expected)
                                }
                            } else {
                                if matches!(name.as_str(), "some" | "none") {
                                    return Err(lowercase_option_constructor_error(name));
                                }
                                Err(e) // Return the original error
                            }
                        }
                    }
                }
                ExprKind::VariantRef(path) => Err(TypeError::UnsupportedFeature(format!(
                    "enum constructor '{}::{}' must be called directly with OSV syntax",
                    path.enum_name, path.variant_name
                ))),
                ExprKind::RecordLit(record_lit) => {
                    self.check_record_lit_with_expected(record_lit, expected)
                }
                ExprKind::Clone(clone_expr) => self.check_clone_expr(clone_expr),
                ExprKind::Freeze(expr) => self.check_freeze_expr(expr),
                ExprKind::FieldAccess(expr, field) => self.check_field_access(expr, field),
                ExprKind::Call(call) => self.check_call_expr_with_expected(call, expected),
                ExprKind::Block(block) => self.check_block_expr_with_expected(block, expected),
                ExprKind::Binary(binary) => self.check_binary_expr(binary, expected),
                ExprKind::Unary(unary) => self.check_unary_expr(unary, expected),
                ExprKind::Cast(cast) => self.check_cast_expr(cast),
                ExprKind::Pipe(pipe) => self.check_pipe_expr_with_expected(pipe, expected),
                ExprKind::With(with) => self.check_with_expr_with_expected(with, expected),
                ExprKind::WithLifetime(with_lifetime) => {
                    self.check_with_lifetime_expr(with_lifetime)
                }
                ExprKind::Then(then) => self.check_then_expr_with_expected(then, expected),
                ExprKind::While(while_expr) => self.check_while_expr(while_expr),
                ExprKind::Match(match_expr) => {
                    self.check_match_expr_with_expected(match_expr, expected)
                }
                ExprKind::ListLit(elements) if matches!(expected, Some(TypedType::Array(_, _))) => {
                    self.check_array_lit(elements, expected)
                }
                ExprKind::ListLit(elements) => self.check_list_lit(elements, expected),
                ExprKind::RangeLit(range) => self.check_range_lit(range, expected),
                ExprKind::ArrayLit(elements) => self.check_array_lit(elements, expected),
                ExprKind::Lambda(lambda) => self.check_lambda_expr(lambda, expected),
                ExprKind::PrototypeClone(proto_clone) => {
                    self.check_prototype_clone_expr(proto_clone)
                }
                ExprKind::Await(expr) => self.check_await_expr(expr),
                ExprKind::Spawn(expr) => self.check_spawn_expr(expr),
            }
        })();

        if let Ok(ty) = &result {
            self.record_checked_expr_type(expr, ty);
        }

        result
    }

    fn expected_record_type_args(
        expected: Option<&TypedType>,
        record_name: &str,
    ) -> Option<Vec<TypedType>> {
        match expected {
            Some(TypedType::Record {
                name, type_args, ..
            }) if name == record_name => Some(type_args.clone()),
            Some(TypedType::Temporal { base_type, .. }) => {
                Self::expected_record_type_args(Some(base_type), record_name)
            }
            _ => None,
        }
    }

    fn check_record_lit_with_expected(
        &mut self,
        record_lit: &RecordLit,
        expected: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        // First check if record exists and collect field types
        let (
            field_types,
            type_params,
            temporal_constraints,
            record_hash,
            parent_hash,
        ): RecordDefSnapshot = {
            let record_def = self
                .records
                .get(&record_lit.name)
                .ok_or_else(|| TypeError::UndefinedRecord(record_lit.name.clone()))?;
            (
                record_def.fields.clone(),
                record_def.type_params.clone(),
                record_def.temporal_constraints.clone(),
                record_def.hash.clone(),
                record_def.parent_hash.clone(),
            )
        };

        let regular_type_params = Self::regular_type_param_names(&type_params);
        let mut type_args =
            if let Some(type_args) = Self::expected_record_type_args(expected, &record_lit.name) {
                type_args
            } else if regular_type_params.is_empty() {
                Vec::new()
            } else {
                regular_type_params
                    .iter()
                    .map(|_| self.type_var_generator.fresh_var())
                    .collect()
            };

        if type_args.len() != regular_type_params.len() {
            return Err(TypeError::TypeMismatch {
                expected: format!("{} generic arguments", regular_type_params.len()),
                found: type_args.len().to_string(),
            });
        }

        let type_arg_bindings = Self::type_arg_bindings(&type_params, &type_args);
        let instantiated_field_types = field_types
            .iter()
            .map(|(name, ty)| {
                (
                    name.clone(),
                    Self::apply_type_arg_bindings(ty, &type_arg_bindings),
                )
            })
            .collect::<HashMap<_, _>>();

        let mut provided_fields = HashSet::new();
        let mut has_spread = false;
        let mut final_field_sources: HashMap<String, bool> = HashMap::new();
        let mut field_substitution = ConstraintSubstitution::new();

        // Check that all provided fields exist and have correct types.
        for field_init in &record_lit.fields {
            match field_init {
                FieldInit::Field { name, value } => {
                    provided_fields.insert(name.clone());
                    final_field_sources.insert(name.clone(), true);
                    let expected_ty = instantiated_field_types.get(name).ok_or_else(|| {
                        TypeError::UnknownField {
                            record: record_lit.name.clone(),
                            field: name.clone(),
                        }
                    })?;
                    let expected_ty = field_substitution.apply(expected_ty)?;

                    let actual_ty = self.check_expr_with_expected(value, Some(&expected_ty))?;
                    if Self::contains_inference_internal_type(&expected_ty) {
                        unify_constraint(&expected_ty, &actual_ty, &mut field_substitution)?;
                    } else if !self.type_matches_expected(&expected_ty, &actual_ty) {
                        return Err(typed_type_mismatch(&expected_ty, &actual_ty));
                    }
                }
                FieldInit::Spread(expr) => {
                    has_spread = true;
                    let expr_ty = self.check_expr(expr)?;
                    let expected_record_ty = field_substitution.apply(&TypedType::Record {
                        name: record_lit.name.clone(),
                        type_args: type_args.clone(),
                        frozen: false,
                        hash: record_hash.clone(),
                        parent_hash: parent_hash.clone(),
                    })?;

                    if let TypedType::Record { name, .. } = &expr_ty {
                        if name == &record_lit.name {
                            if Self::contains_inference_internal_type(&expected_record_ty) {
                                unify_constraint(
                                    &expected_record_ty,
                                    &expr_ty,
                                    &mut field_substitution,
                                )?;
                            } else if !self.type_matches_expected(&expected_record_ty, &expr_ty) {
                                return Err(typed_type_mismatch(&expected_record_ty, &expr_ty));
                            }
                            let (_, spread_fields) = self.instantiated_record_fields(&expr_ty)?;
                            for field_name in spread_fields.keys() {
                                final_field_sources.insert(field_name.clone(), false);
                            }
                            continue;
                        }
                    }

                    return Err(expected_type_mismatch(
                        format!("record {}", record_lit.name),
                        &expr_ty,
                    ));
                }
            }
        }
        let allow_unresolved_type_args =
            expected.is_none_or(Self::contains_inference_internal_type);
        type_args = type_args
            .iter()
            .map(|arg| {
                if allow_unresolved_type_args {
                    field_substitution.apply(arg)
                } else {
                    finalize_type(arg, &field_substitution)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let final_type_arg_bindings = Self::type_arg_bindings(&type_params, &type_args);
        let final_instantiated_field_types = field_types
            .iter()
            .map(|(name, ty)| {
                (
                    name.clone(),
                    Self::apply_type_arg_bindings(ty, &final_type_arg_bindings),
                )
            })
            .collect::<HashMap<_, _>>();

        if !has_spread {
            for field_name in instantiated_field_types.keys() {
                if !provided_fields.contains(field_name) {
                    return Err(TypeError::MissingField {
                        record: record_lit.name.clone(),
                        field: field_name.clone(),
                    });
                }
            }
        } else {
            self.reject_implicit_non_copy_record_fields(
                &record_lit.name,
                &final_instantiated_field_types,
                &final_field_sources,
                "record spread",
            )?;
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
                let active_temporals: Vec<String> = self
                    .temporal_context
                    .active_temporals
                    .iter()
                    .cloned()
                    .collect();
                let record_temporals: Vec<String> = type_params
                    .iter()
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
                return Err(TypeError::InvalidTemporalConstraint(
                    mapped_inner,
                    mapped_outer,
                ));
            }
        }

        // Create the base record type
        let base_type = TypedType::Record {
            name: record_lit.name.clone(),
            type_args,
            frozen: false,
            hash: record_hash,
            parent_hash,
        };
        self.validate_record_type_form_bounds(&base_type)?;

        // If the record has temporal parameters, wrap it in a Temporal type
        let temporal_params: Vec<String> = type_params
            .iter()
            .filter(|p| p.is_temporal)
            .map(|p| {
                // If we're in a function/context with active temporal parameters,
                // map the record's temporal to the current scope's temporal
                if !self.temporal_context.active_temporals.is_empty() {
                    // For now, use the first active temporal parameter
                    // In a full implementation, we'd have proper mapping/inference
                    if let Some(active_temporal) =
                        self.temporal_context.active_temporals.iter().next()
                    {
                        active_temporal.clone()
                    } else {
                        p.name.clone()
                    }
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

    fn reject_implicit_non_copy_record_fields(
        &self,
        record_name: &str,
        field_types: &HashMap<String, TypedType>,
        final_field_sources: &HashMap<String, bool>,
        operation: &str,
    ) -> Result<(), TypeError> {
        let mut field_names = field_types.keys().cloned().collect::<Vec<_>>();
        field_names.sort();

        for field_name in field_names {
            let Some(field_ty) = field_types.get(&field_name) else {
                continue;
            };
            let final_source_is_explicit = final_field_sources
                .get(&field_name)
                .copied()
                .unwrap_or(false);
            if !final_source_is_explicit && !self.is_copyable(field_ty) {
                return Err(TypeError::UnsupportedFeature(format!(
                    "{operation} would implicitly copy non-copy field {record_name}.{field_name} of type {}; replace the field explicitly",
                    format_typed_type(field_ty)
                )));
            }
        }

        Ok(())
    }

    fn check_clone_expr(&mut self, clone_expr: &CloneExpr) -> Result<TypedType, TypeError> {
        let base_ty = self.check_expr(&clone_expr.base)?;

        match &base_ty {
            TypedType::Record {
                name,
                type_args,
                frozen,
                ..
            } => {
                if *frozen {
                    return Err(TypeError::CloneFrozenRecord);
                }
                // Check field updates
                let field_types: HashMap<String, TypedType> = {
                    let record_def = self
                        .records
                        .get(name)
                        .ok_or_else(|| TypeError::UndefinedRecord(name.clone()))?;
                    let bindings = Self::type_arg_bindings(&record_def.type_params, type_args);
                    record_def
                        .fields
                        .iter()
                        .map(|(field_name, field_ty)| {
                            (
                                field_name.clone(),
                                Self::apply_type_arg_bindings(field_ty, &bindings),
                            )
                        })
                        .collect()
                };
                let mut final_field_sources = field_types
                    .keys()
                    .map(|field_name| (field_name.clone(), false))
                    .collect::<HashMap<_, _>>();

                for field_init in &clone_expr.updates.fields {
                    match field_init {
                        FieldInit::Field {
                            name: field_name,
                            value,
                        } => {
                            // Verify field exists and type matches
                            let expected_ty = field_types.get(field_name).ok_or_else(|| {
                                TypeError::UnknownField {
                                    record: name.clone(),
                                    field: field_name.clone(),
                                }
                            })?;

                            let actual_ty =
                                self.check_expr_with_expected(value, Some(expected_ty))?;
                            if !self.type_matches_expected(expected_ty, &actual_ty) {
                                return Err(typed_type_mismatch(expected_ty, &actual_ty));
                            }
                            final_field_sources.insert(field_name.clone(), true);
                        }
                        FieldInit::Spread(expr) => {
                            let expr_ty = self.check_expr(expr)?;

                            match &expr_ty {
                                TypedType::Record {
                                    name: spread_name,
                                    type_args: spread_args,
                                    ..
                                } if spread_name == name && spread_args == type_args => {
                                    // A same-type spread may overwrite the cloned base.
                                    let (_, spread_fields) =
                                        self.instantiated_record_fields(&expr_ty)?;
                                    for field_name in spread_fields.keys() {
                                        final_field_sources.insert(field_name.clone(), false);
                                    }
                                }
                                _ => {
                                    return Err(expected_type_mismatch(
                                        format!("record {}", name),
                                        &expr_ty,
                                    ));
                                }
                            }
                        }
                    }
                }
                self.reject_implicit_non_copy_record_fields(
                    name,
                    &field_types,
                    &final_field_sources,
                    "record clone",
                )?;
                Ok(TypedType::Record {
                    name: name.clone(),
                    type_args: type_args.clone(),
                    frozen: false,
                    hash: None,
                    parent_hash: None,
                })
            }
            other => Err(expected_type_mismatch("record", other)),
        }
    }

    fn check_freeze_expr(&mut self, expr: &Expr) -> Result<TypedType, TypeError> {
        let ty = self.check_expr(expr)?;

        match ty {
            TypedType::Record {
                name,
                type_args,
                frozen,
                hash,
                parent_hash,
            } => {
                if frozen {
                    return Err(TypeError::FreezeAlreadyFrozen);
                }
                Ok(TypedType::Record {
                    name,
                    type_args,
                    frozen: true,
                    hash,
                    parent_hash,
                })
            }
            other => Err(expected_type_mismatch("record", &other)),
        }
    }

    fn constraint_origin(kind: ConstraintKind) -> ConstraintOrigin {
        ConstraintOrigin { span: None, kind }
    }

    fn call_constraint_name(call: &CallExpr) -> String {
        Self::expr_constraint_name(&call.function).unwrap_or_else(|| "call".to_string())
    }

    fn expr_constraint_name(expr: &Expr) -> Option<String> {
        match &expr.kind {
            ExprKind::Ident(name) => Some(name.clone()),
            ExprKind::FieldAccess(_, field) => Some(field.clone()),
            _ => None,
        }
    }

    fn solve_type_constraint(
        &self,
        constraints: &mut Vec<Constraint>,
        substitution: &mut ConstraintSubstitution,
        expected: TypedType,
        actual: TypedType,
        origin: ConstraintOrigin,
    ) -> Result<(), TypeError> {
        constraints.push(Constraint::TypeEquals {
            expected,
            actual,
            origin,
        });
        *substitution =
            self.solve_constraints_partial_with_current_forms(constraints, substitution)?;
        Ok(())
    }

    fn solve_constraints_with_current_forms(
        &self,
        constraints: &[Constraint],
        initial: &ConstraintSubstitution,
    ) -> Result<ConstraintSubstitution, TypeError> {
        let forms = self.scoped_form_environment();
        solve_constraints_with_forms_and_initial(constraints, &forms, initial)
    }

    fn solve_constraints_partial_with_current_forms(
        &self,
        constraints: &[Constraint],
        initial: &ConstraintSubstitution,
    ) -> Result<ConstraintSubstitution, TypeError> {
        let forms = self.scoped_form_environment();
        solve_constraints_partial_with_forms_and_initial(constraints, &forms, initial)
    }

    fn scoped_form_environment(&self) -> FormEnvironment {
        let mut forms = self.form_environment.clone();
        let mut visible_type_params = HashSet::new();
        for (type_params, bounds) in self
            .type_param_env
            .iter()
            .zip(self.form_bounds_env.iter())
            .rev()
        {
            for type_param in type_params {
                if !visible_type_params.insert(type_param) {
                    continue;
                }
                let Some(bounds) = bounds.get(type_param) else {
                    continue;
                };
                for form_name in bounds {
                    if self.forms.contains_key(form_name) {
                        forms.assume_abstract_adoption(type_param.clone(), form_name.clone());
                    }
                }
            }
        }
        forms
    }

    fn lower_associated_type_projections(
        &mut self,
        ty: TypedType,
        constraints: &mut Vec<Constraint>,
        origin: ConstraintOrigin,
    ) -> TypedType {
        match ty {
            TypedType::Projection {
                base,
                form_name,
                assoc_name,
                args,
            } => {
                let base_type =
                    self.lower_associated_type_projections(*base, constraints, origin.clone());
                let type_args = args
                    .into_iter()
                    .map(|arg| {
                        self.lower_associated_type_projections(arg, constraints, origin.clone())
                    })
                    .collect();
                let result = self.type_var_generator.fresh_var();
                constraints.push(Constraint::AssociatedTypeResolution {
                    base_type,
                    form_name,
                    assoc_name,
                    type_args,
                    result: result.clone(),
                    origin,
                });
                result
            }
            TypedType::List(inner) => TypedType::List(Box::new(
                self.lower_associated_type_projections(*inner, constraints, origin),
            )),
            TypedType::Array(inner, size) => TypedType::Array(
                Box::new(self.lower_associated_type_projections(*inner, constraints, origin)),
                size,
            ),
            TypedType::Option(inner) => TypedType::Option(Box::new(
                self.lower_associated_type_projections(*inner, constraints, origin),
            )),
            TypedType::Result(ok, err) => TypedType::Result(
                Box::new(self.lower_associated_type_projections(*ok, constraints, origin.clone())),
                Box::new(self.lower_associated_type_projections(*err, constraints, origin)),
            ),
            TypedType::Function {
                params,
                return_type,
            } => TypedType::Function {
                params: params
                    .into_iter()
                    .map(|param| {
                        self.lower_associated_type_projections(param, constraints, origin.clone())
                    })
                    .collect(),
                return_type: Box::new(self.lower_associated_type_projections(
                    *return_type,
                    constraints,
                    origin,
                )),
            },
            TypedType::Temporal {
                base_type,
                temporals,
            } => TypedType::Temporal {
                base_type: Box::new(self.lower_associated_type_projections(
                    *base_type,
                    constraints,
                    origin,
                )),
                temporals,
            },
            other => other,
        }
    }

    fn form_bound_names(&self, type_param: &TypeParam) -> Vec<String> {
        type_param.of_forms.clone()
    }

    // Check function call with generic type inference
    fn check_function_call_with_inference(
        &mut self,
        func_info: &FunctionDef,
        call: &CallExpr,
        expected_return: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
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
            let param_types: Vec<TypedType> =
                func_info.params.iter().map(|(_, ty)| ty.clone()).collect();
            self.check_monomorphic_apply_arguments(&call.args, &param_types)?;

            return Ok(func_info.return_type.clone());
        }

        // For generic functions, instantiate declared type parameters as
        // A-layer inference variables and solve equational constraints.
        //
        // B-layer affine effects still follow the source evaluation order:
        // non-lambda arguments are checked once, lambdas are checked once after
        // any concrete context from ordinary arguments and return annotations
        // has been propagated into the substitution.
        let type_param_names: Vec<String> = func_info
            .type_params
            .iter()
            .map(|param| param.name.clone())
            .collect();
        let type_vars = fresh_type_param_map(&type_param_names, &mut self.type_var_generator);
        let raw_param_types: Vec<TypedType> = func_info
            .params
            .iter()
            .map(|(_, ty)| substitute_type_params(ty, &type_vars))
            .collect();
        let raw_return_type = substitute_type_params(&func_info.return_type, &type_vars);

        let mut substitution = ConstraintSubstitution::new();
        let mut constraints = Vec::new();
        let func_name = Self::call_constraint_name(call);

        for type_param in &func_info.type_params {
            for form_name in self.form_bound_names(type_param) {
                if let Some(ty) = type_vars.get(&type_param.name) {
                    constraints.push(Constraint::HasForm {
                        ty: ty.clone(),
                        form_name,
                        origin: Self::constraint_origin(ConstraintKind::FormBound {
                            type_param: type_param.name.clone(),
                        }),
                    });
                }
            }
        }

        let param_types: Vec<TypedType> = raw_param_types
            .into_iter()
            .map(|ty| {
                self.lower_associated_type_projections(
                    ty,
                    &mut constraints,
                    Self::constraint_origin(ConstraintKind::AssocTypeProjection {
                        assoc_name: func_name.clone(),
                    }),
                )
            })
            .collect();
        let return_type = self.lower_associated_type_projections(
            raw_return_type,
            &mut constraints,
            Self::constraint_origin(ConstraintKind::ReturnAnnotation {
                var_name: func_name.clone(),
            }),
        );

        self.seed_constrained_apply_return(
            &mut constraints,
            &mut substitution,
            &return_type,
            expected_return,
            &func_name,
        )?;

        let checked_arg_types = self.check_apply_arguments_with_constraints(
            &call.args,
            &param_types,
            &mut constraints,
            &mut substitution,
            &func_name,
        )?;

        // Check type bounds for inferred types
        for type_param in &func_info.type_params {
            if type_param.bounds.is_empty() && type_param.derivation_bound.is_none() {
                continue;
            }

            let concrete_type = match type_vars.get(&type_param.name) {
                Some(ty) => finalize_type(ty, &substitution)?,
                None => continue,
            };

            {
                // Check trait bounds
                for bound in &type_param.bounds {
                    if !self.type_implements_trait(&concrete_type, &bound.trait_name) {
                        return Err(TypeError::UnsupportedFeature(format!(
                            "Type {} does not implement trait {}",
                            format_typed_type(&concrete_type),
                            bound.trait_name
                        )));
                    }
                }

                // Check derivation bounds (T from ParentType)
                if let Some(required_parent) = &type_param.derivation_bound {
                    self.check_derivation_bound(&concrete_type, required_parent)?;
                }
            }
        }

        self.finish_constrained_apply_return(
            &mut constraints,
            &mut substitution,
            &return_type,
            expected_return,
            &func_name,
        )?;
        self.apply_substitution_to_var_env(&substitution)?;
        for (arg, actual_ty) in call.args.iter().zip(checked_arg_types.iter()) {
            let resolved =
                self.update_direct_ident_from_substitution(arg, actual_ty, &substitution)?;
            self.record_checked_expr_type(arg, &resolved);
        }
        finalize_type(&return_type, &substitution)
    }

    fn seed_constrained_apply_return(
        &self,
        constraints: &mut Vec<Constraint>,
        substitution: &mut ConstraintSubstitution,
        return_type: &TypedType,
        expected_return: Option<&TypedType>,
        func_name: &str,
    ) -> Result<(), TypeError> {
        if let Some(expected_return) = expected_return {
            self.solve_type_constraint(
                constraints,
                substitution,
                return_type.clone(),
                expected_return.clone(),
                Self::constraint_origin(ConstraintKind::ReturnAnnotation {
                    var_name: func_name.to_string(),
                }),
            )?;
        }

        Ok(())
    }

    fn finish_constrained_apply_return(
        &self,
        constraints: &mut Vec<Constraint>,
        substitution: &mut ConstraintSubstitution,
        return_type: &TypedType,
        expected_return: Option<&TypedType>,
        func_name: &str,
    ) -> Result<(), TypeError> {
        if let Some(expected_return) = expected_return {
            let instantiated_return_type = substitution.apply(return_type)?;
            self.solve_type_constraint(
                constraints,
                substitution,
                instantiated_return_type,
                expected_return.clone(),
                Self::constraint_origin(ConstraintKind::ReturnAnnotation {
                    var_name: func_name.to_string(),
                }),
            )?;
        }

        *substitution = self.solve_constraints_with_current_forms(constraints, substitution)?;
        Ok(())
    }

    fn check_monomorphic_apply_arguments(
        &mut self,
        args: &[Box<Expr>],
        param_types: &[TypedType],
    ) -> Result<Vec<TypedType>, TypeError> {
        let mut checked_arg_types = Vec::with_capacity(args.len());

        for (arg, expected_ty) in args.iter().zip(param_types.iter()) {
            let actual_ty = self.check_expr_with_expected(arg, Some(expected_ty))?;
            if !self.type_matches_expected(expected_ty, &actual_ty) {
                return Err(typed_type_mismatch(expected_ty, &actual_ty));
            }
            checked_arg_types.push(actual_ty);
        }

        Ok(checked_arg_types)
    }

    fn check_apply_arguments_with_constraints(
        &mut self,
        args: &[Box<Expr>],
        param_types: &[TypedType],
        constraints: &mut Vec<Constraint>,
        substitution: &mut ConstraintSubstitution,
        func_name: &str,
    ) -> Result<Vec<TypedType>, TypeError> {
        let mut checked_arg_types: Vec<Option<TypedType>> = vec![None; args.len()];

        // Non-lambda arguments are checked first in source order. This preserves
        // B-layer affine effects while letting their concrete types feed A-layer
        // constraints before lambda bodies are checked.
        for (i, arg) in args.iter().enumerate() {
            if matches!(&arg.kind, ExprKind::Lambda(_)) {
                continue;
            }

            let param_type = substitution.apply(&param_types[i])?;
            let actual_ty = self.check_expr_with_expected(arg, Some(&param_type))?;
            self.solve_apply_argument_constraint(
                constraints,
                substitution,
                param_type,
                actual_ty.clone(),
                func_name,
                i,
            )?;
            checked_arg_types[i] = Some(actual_ty);
        }

        // Lambdas are checked after ordinary arguments so parameter and return
        // context collected from sibling arguments can flow into their bodies.
        for (i, arg) in args.iter().enumerate() {
            let param_type = substitution.apply(&param_types[i])?;
            let actual_ty = if let Some(actual_ty) = checked_arg_types[i].clone() {
                actual_ty
            } else if let ExprKind::Lambda(lambda) = &arg.kind {
                self.check_generic_lambda_arg(lambda, &param_type, substitution)?
            } else {
                self.check_expr_with_expected(arg, Some(&param_type))?
            };

            self.solve_apply_argument_constraint(
                constraints,
                substitution,
                param_type,
                actual_ty.clone(),
                func_name,
                i,
            )?;
            checked_arg_types[i] = Some(actual_ty);
        }

        checked_arg_types
            .into_iter()
            .enumerate()
            .map(|(i, ty)| {
                ty.ok_or_else(|| {
                    TypeError::UnsupportedFeature(format!(
                        "internal error: argument {} of {} was not checked",
                        i + 1,
                        func_name
                    ))
                })
            })
            .collect()
    }

    fn solve_apply_argument_constraint(
        &self,
        constraints: &mut Vec<Constraint>,
        substitution: &mut ConstraintSubstitution,
        expected: TypedType,
        actual: TypedType,
        func_name: &str,
        arg_index: usize,
    ) -> Result<(), TypeError> {
        self.solve_type_constraint(
            constraints,
            substitution,
            expected,
            actual,
            Self::constraint_origin(ConstraintKind::Argument {
                func_name: func_name.to_string(),
                arg_index,
            }),
        )
    }

    fn check_generic_lambda_arg(
        &mut self,
        lambda: &LambdaExpr,
        expected: &TypedType,
        substitution: &mut ConstraintSubstitution,
    ) -> Result<TypedType, TypeError> {
        if lambda.implicit_focus && self.implicit_focus_depth > 0 {
            return Err(TypeError::UnsupportedFeature(
                "nested implicit focus scopes are ambiguous; name at least one scope binder with `{ |value| ... }`"
                    .to_string(),
            ));
        }

        if lambda.implicit_focus {
            self.implicit_focus_depth += 1;
        }
        let result = self.check_generic_lambda_arg_inner(lambda, expected, substitution);
        if lambda.implicit_focus {
            self.implicit_focus_depth -= 1;
        }
        result
    }

    fn check_generic_lambda_arg_inner(
        &mut self,
        lambda: &LambdaExpr,
        expected: &TypedType,
        substitution: &mut ConstraintSubstitution,
    ) -> Result<TypedType, TypeError> {
        let expected = substitution.apply(expected)?;
        let expected = if matches!(expected, TypedType::InferVar(_)) {
            let shaped = self.fresh_lambda_function_type(lambda.params.len());
            unify_constraint(&expected, &shaped, substitution)?;
            substitution.apply(&shaped)?
        } else {
            expected
        };
        let (params, return_type) = match &expected {
            TypedType::Function {
                params,
                return_type,
            } => (params, return_type),
            other => {
                return Err(expected_type_mismatch("function", other));
            }
        };

        if lambda.implicit_focus && params.len() != 1 {
            return Err(TypeError::UnsupportedFeature(format!(
                "implicit focus scopes require a unary function parameter, but this callable expects {} binders; name them with `{{ |left, right| ... }}`",
                params.len()
            )));
        }
        if params.len() != lambda.params.len() {
            return Err(TypeError::ArityMismatch {
                expected: params.len(),
                found: lambda.params.len(),
            });
        }

        let bound_vars = HashSet::new();
        let free_vars = self.collect_free_variables(&lambda.body, &bound_vars);
        let allowed_temporals = self.temporal_context.active_temporals.clone();

        for var_name in &free_vars {
            if let Some(var_type) = self.peek_var_type(var_name) {
                self.check_temporal_escape(&var_type, &allowed_temporals)?;
            }
        }

        self.push_scope();
        for (param, param_type) in lambda.params.iter().zip(params.iter()) {
            let param_type = self.resolve_lambda_param_type(param, param_type, substitution)?;
            self.bind_var(param.name.clone(), param_type, false)?;
        }

        let body_result = self.check_expr_with_expected(&lambda.body, Some(return_type.as_ref()));
        let observed_param_types = lambda
            .params
            .iter()
            .map(|param| self.peek_var_type(&param.name))
            .collect::<Option<Vec<_>>>()
            .unwrap_or_else(|| params.to_vec());
        self.pop_scope();

        let body_type = body_result?;
        for (param_type, observed_type) in params.iter().zip(observed_param_types.iter()) {
            unify_constraint(param_type, observed_type, substitution)?;
        }
        unify_constraint(return_type, &body_type, substitution)?;

        let finalized_params = params
            .iter()
            .map(|param| substitution.apply(param))
            .collect::<Result<Vec<_>, _>>()?;
        let finalized_return = substitution.apply(&body_type)?;

        let func_type = TypedType::Function {
            params: finalized_params,
            return_type: Box::new(finalized_return),
        };

        self.check_temporal_escape(&func_type, &allowed_temporals)?;

        Ok(func_type)
    }

    fn fresh_lambda_function_type(&mut self, arity: usize) -> TypedType {
        TypedType::Function {
            params: (0..arity)
                .map(|_| self.type_var_generator.fresh_var())
                .collect(),
            return_type: Box::new(self.type_var_generator.fresh_var()),
        }
    }

    fn resolve_lambda_param_type(
        &mut self,
        param: &LambdaParam,
        expected: &TypedType,
        substitution: &mut ConstraintSubstitution,
    ) -> Result<TypedType, TypeError> {
        if let Some(type_annotation) = &param.type_annotation {
            let annotated_ty = self.convert_type(type_annotation)?;
            unify_constraint(expected, &annotated_ty, substitution)?;
        }

        substitution.apply(expected)
    }

    fn check_immediate_lambda_call(
        &mut self,
        lambda: &LambdaExpr,
        args: &[Box<Expr>],
        expected_return: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        if args.len() != lambda.params.len() {
            return Err(TypeError::ArityMismatch {
                expected: lambda.params.len(),
                found: args.len(),
            });
        }

        if args
            .iter()
            .any(|arg| Self::expr_requires_call_parameter_context(arg))
        {
            return self.check_contextual_immediate_lambda_call(lambda, args, expected_return);
        }

        let mut arg_types = Vec::with_capacity(args.len());
        for arg in args {
            arg_types.push(self.check_expr(arg)?);
        }

        let return_type = expected_return
            .cloned()
            .unwrap_or_else(|| self.type_var_generator.fresh_var());
        let expected_func = TypedType::Function {
            params: arg_types,
            return_type: Box::new(return_type.clone()),
        };

        let mut substitution = ConstraintSubstitution::new();
        self.check_generic_lambda_arg(lambda, &expected_func, &mut substitution)?;
        finalize_type(&return_type, &substitution)
    }

    fn check_contextual_immediate_lambda_call(
        &mut self,
        lambda: &LambdaExpr,
        args: &[Box<Expr>],
        expected_return: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        let mut substitution = ConstraintSubstitution::new();
        let param_types: Vec<TypedType> = args
            .iter()
            .map(|_| self.type_var_generator.fresh_var())
            .collect();
        let return_type = expected_return
            .cloned()
            .unwrap_or_else(|| self.type_var_generator.fresh_var());
        let expected_func = TypedType::Function {
            params: param_types.clone(),
            return_type: Box::new(return_type.clone()),
        };

        self.check_generic_lambda_arg(lambda, &expected_func, &mut substitution)?;

        for (arg, param_type) in args.iter().zip(param_types.iter()) {
            let expected_param = substitution.apply(param_type)?;
            let actual_ty = self.check_expr_with_expected(arg, Some(&expected_param))?;
            unify_constraint(&expected_param, &actual_ty, &mut substitution)?;
        }

        for param_type in &param_types {
            finalize_type(param_type, &substitution)?;
        }

        finalize_type(&return_type, &substitution)
    }

    fn expr_requires_expected_type(expr: &Expr) -> bool {
        if let Some((variant, payload)) = Self::builtin_constructor_expr(expr) {
            return match variant {
                BuiltinVariant::OptionSome => {
                    payload.is_some_and(Self::expr_requires_expected_type)
                }
                BuiltinVariant::OptionNone
                | BuiltinVariant::ResultOk
                | BuiltinVariant::ResultErr => true,
            };
        }

        match &expr.kind {
            ExprKind::ListLit(elements) | ExprKind::ArrayLit(elements) => {
                elements.is_empty()
                    || elements
                        .iter()
                        .any(|element| Self::expr_requires_expected_type(element))
            }
            ExprKind::RangeLit(range) => {
                Self::expr_requires_expected_type(&range.start)
                    || Self::expr_requires_expected_type(&range.end)
            }
            ExprKind::Unary(unary) => Self::expr_requires_expected_type(&unary.expr),
            ExprKind::Cast(cast) => Self::expr_requires_expected_type(&cast.expr),
            ExprKind::RecordLit(record_lit) => record_lit.fields.iter().any(|field| match field {
                FieldInit::Field { value, .. } => Self::expr_requires_expected_type(value),
                FieldInit::Spread(expr) => Self::expr_requires_expected_type(expr),
            }),
            ExprKind::Clone(clone_expr) => {
                clone_expr.updates.fields.iter().any(|field| match field {
                    FieldInit::Field { value, .. } => Self::expr_requires_expected_type(value),
                    FieldInit::Spread(expr) => Self::expr_requires_expected_type(expr),
                })
            }
            ExprKind::Then(then_expr) => {
                Self::expr_requires_expected_type(&then_expr.condition)
                    || then_expr
                        .then_block
                        .expr
                        .as_deref()
                        .is_some_and(Self::expr_requires_expected_type)
                    || then_expr.else_ifs.iter().any(|(condition, block)| {
                        Self::expr_requires_expected_type(condition)
                            || block
                                .expr
                                .as_deref()
                                .is_some_and(Self::expr_requires_expected_type)
                    })
                    || then_expr
                        .else_block
                        .as_ref()
                        .and_then(|block| block.expr.as_deref())
                        .is_some_and(Self::expr_requires_expected_type)
            }
            ExprKind::Match(match_expr) => {
                Self::expr_requires_expected_type(&match_expr.expr)
                    || match_expr.arms.iter().any(|arm| {
                        arm.body
                            .expr
                            .as_deref()
                            .is_some_and(Self::expr_requires_expected_type)
                    })
            }
            _ => false,
        }
    }

    fn expr_requires_call_parameter_context(expr: &Expr) -> bool {
        matches!(&expr.kind, ExprKind::Lambda(_)) || Self::expr_requires_expected_type(expr)
    }

    fn check_function_value_call_with_expected(
        &mut self,
        func_var_name: Option<&str>,
        func_ty: TypedType,
        args: &[Box<Expr>],
        expected_return: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        let (params, return_type) = match func_ty {
            TypedType::Function {
                params,
                return_type,
            } => (params, *return_type),
            other => {
                return Err(expected_type_mismatch("function", &other));
            }
        };

        if args.len() != params.len() {
            return Err(TypeError::ArityMismatch {
                expected: params.len(),
                found: args.len(),
            });
        }

        let mut substitution = ConstraintSubstitution::new();
        let mut constraints = Vec::new();
        let func_name = "function value".to_string();
        let params: Vec<TypedType> = params
            .into_iter()
            .map(|param| {
                self.lower_associated_type_projections(
                    param,
                    &mut constraints,
                    Self::constraint_origin(ConstraintKind::AssocTypeProjection {
                        assoc_name: func_name.clone(),
                    }),
                )
            })
            .collect();
        let return_type = self.lower_associated_type_projections(
            return_type,
            &mut constraints,
            Self::constraint_origin(ConstraintKind::ReturnAnnotation {
                var_name: func_name.clone(),
            }),
        );
        let lowered_func_ty = TypedType::Function {
            params: params.clone(),
            return_type: Box::new(return_type.clone()),
        };

        self.seed_constrained_apply_return(
            &mut constraints,
            &mut substitution,
            &return_type,
            expected_return,
            &func_name,
        )?;

        let checked_arg_types = self.check_apply_arguments_with_constraints(
            args,
            &params,
            &mut constraints,
            &mut substitution,
            &func_name,
        )?;

        self.finish_constrained_apply_return(
            &mut constraints,
            &mut substitution,
            &return_type,
            expected_return,
            &func_name,
        )?;
        if let Some(func_var_name) = func_var_name {
            if let Some(deferred) = self.peek_deferred_callable(func_var_name) {
                let expected_func_ty = substitution.apply(&lowered_func_ty)?;
                self.check_deferred_callable_against_expected(
                    func_var_name,
                    &deferred,
                    &expected_func_ty,
                    &mut substitution,
                )?;
            }
        }
        self.apply_substitution_to_var_env(&substitution)?;
        for (arg, actual_ty) in args.iter().zip(checked_arg_types.iter()) {
            let resolved =
                self.update_direct_ident_from_substitution(arg, actual_ty, &substitution)?;
            self.record_checked_expr_type(arg, &resolved);
        }
        if let Some(func_var_name) = func_var_name {
            if Self::contains_inference_internal_type(&lowered_func_ty) {
                let resolved_func_ty = substitution.apply(&lowered_func_ty)?;
                self.update_var_type(func_var_name, resolved_func_ty);
            }
        }
        finalize_type(&return_type, &substitution)
    }

    fn check_field_access(&mut self, expr: &Expr, field: &str) -> Result<TypedType, TypeError> {
        if let ExprKind::Ident(name) = &expr.kind {
            let var = self._peek_var(name)?.clone();
            let field_ty = self.record_field_type(&var.ty, field)?;

            if var.mutable {
                return Ok(field_ty);
            }

            if var.used {
                return Err(TypeError::AffineViolation(name.clone()));
            }

            if self.is_copyable(&field_ty) {
                return Ok(field_ty);
            }

            // Accessing a non-copyable field moves ownership out of the record,
            // so the parent record is consumed under affine semantics.
            self.lookup_var(name)?;
            return Ok(field_ty);
        }

        let ty = self.check_expr(expr)?;
        self.record_field_type(&ty, field)
    }

    fn record_field_type(&self, ty: &TypedType, field: &str) -> Result<TypedType, TypeError> {
        // Handle temporal types by unwrapping to the base type
        let base_ty = match ty {
            TypedType::Temporal { base_type, .. } => base_type.as_ref(),
            _ => ty,
        };

        match base_ty {
            TypedType::Record {
                name, type_args, ..
            } => {
                let record_def = self
                    .records
                    .get(name)
                    .ok_or_else(|| TypeError::UndefinedRecord(name.clone()))?;
                let bindings = Self::type_arg_bindings(&record_def.type_params, type_args);
                let field_ty =
                    record_def
                        .fields
                        .get(field)
                        .ok_or_else(|| TypeError::UnknownField {
                            record: name.clone(),
                            field: field.to_string(),
                        })?;
                Ok(Self::apply_type_arg_bindings(field_ty, &bindings))
            }
            _ => Err(expected_type_mismatch("record", ty)),
        }
    }

    fn check_call_expr_with_expected(
        &mut self,
        call: &CallExpr,
        expected_return: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        self.check_call_expr_with_expected_mode(call, expected_return, true)
    }

    fn check_call_expr_with_expected_mode(
        &mut self,
        call: &CallExpr,
        expected_return: Option<&TypedType>,
        allow_receiver_methods: bool,
    ) -> Result<TypedType, TypeError> {
        // First check the function expression type
        match &call.function.kind {
            ExprKind::VariantRef(path) => {
                self.check_enum_constructor_call(path, &call.args, expected_return)
            }
            ExprKind::Ident(name) => {
                // First check if it's a variable that holds a function
                match self.lookup_var(name) {
                    Ok(var_ty) => {
                        return self.check_function_value_call_with_expected(
                            Some(name),
                            var_ty,
                            &call.args,
                            expected_return,
                        );
                    }
                    Err(TypeError::UndefinedVariable(_)) => {}
                    Err(err) => return Err(err),
                }

                // Applicable receiver methods are type-directed OSV calls and
                // take precedence over same-spelled global or builtin
                // functions. Lexical callable bindings above still win.
                if allow_receiver_methods {
                    if let Some(return_type) = self.check_osv_method_call(name, &call.args)? {
                        return Ok(return_type);
                    }
                }

                // Handle spawn operation - requires AsyncRuntime context
                if name == "spawn" {
                    if !self.is_in_async_runtime() {
                        return Err(TypeError::UnsupportedFeature(
                            "spawn can only be used within an AsyncRuntime context".to_string(),
                        ));
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
                        return Err(TypeError::UnsupportedFeature(
                            "await can only be used within an AsyncRuntime context".to_string(),
                        ));
                    }

                    if call.args.len() != 1 {
                        return Err(TypeError::ArityMismatch {
                            expected: 1,
                            found: call.args.len(),
                        });
                    }

                    return self.check_await_expr(&call.args[0]);
                }

                // Otherwise try to find a regular function
                if let Some(func_info) = self.functions.get(name).cloned() {
                    if self.provisional_function_returns.contains(name) {
                        return Err(TypeError::CannotInferType(format!(
                            "function '{}' is used before its return type has been inferred; add an explicit return annotation",
                            name
                        )));
                    }

                    // For spawn and await, we need to check AsyncRuntime context even if they're registered builtins
                    if name == "spawn" || name == "await" {
                        // These were already handled above, so this shouldn't happen
                        return Err(TypeError::UnsupportedFeature(
                            "Internal error: spawn/await should be handled earlier".to_string(),
                        ));
                    }
                    self.check_function_call_with_inference(&func_info, call, expected_return)
                } else {
                    if matches!(name.as_str(), "some" | "none") {
                        return Err(lowercase_option_constructor_error(name));
                    }

                    Err(TypeError::UndefinedFunction(name.clone()))
                }
            }
            ExprKind::FieldAccess(obj_expr, method_name) => {
                // Parser-level field access can still appear in callable position.
                // Function-typed fields are first-class callable values; otherwise
                // the public method form remains OSV: `(receiver, args...) method`.
                let obj_ty = self.check_expr(obj_expr)?;
                let field_ty = self.record_field_type(&obj_ty, method_name);
                if matches!(field_ty, Ok(TypedType::Function { .. })) {
                    return self.check_function_value_call_with_expected(
                        None,
                        field_ty?,
                        &call.args,
                        expected_return,
                    );
                }

                self.resolve_method_call(&obj_ty, method_name, &call.args)
            }
            _ => {
                // For other function expressions (including lambdas)
                if let ExprKind::Lambda(lambda) = &call.function.kind {
                    return self.check_immediate_lambda_call(lambda, &call.args, expected_return);
                }

                let func_ty = self.check_expr(&call.function)?;
                self.check_function_value_call_with_expected(
                    None,
                    func_ty,
                    &call.args,
                    expected_return,
                )
            }
        }
    }

    fn enum_variant_payload(&self, path: &VariantPath) -> Result<Option<TypedType>, TypeError> {
        let enum_def = self
            .enums
            .get(&path.enum_name)
            .ok_or_else(|| TypeError::UnknownType(path.enum_name.clone()))?;
        enum_def
            .variants
            .get(&path.variant_name)
            .cloned()
            .ok_or_else(|| {
                TypeError::UnsupportedFeature(format!(
                    "unknown enum variant '{}::{}'",
                    path.enum_name, path.variant_name
                ))
            })
    }

    fn check_enum_constructor_call(
        &mut self,
        path: &VariantPath,
        args: &[Box<Expr>],
        expected_return: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        if let Some(variant) = path.builtin() {
            return self.check_builtin_constructor_call(variant, args, expected_return);
        }

        let payload = self.enum_variant_payload(path)?;
        let enum_ty = TypedType::Enum {
            name: path.enum_name.clone(),
        };

        if let Some(expected) = expected_return {
            if !matches!(expected, TypedType::InferVar(_))
                && !self.type_matches_expected(expected, &enum_ty)
            {
                return Err(typed_type_mismatch(expected, &enum_ty));
            }
        }

        let expected_arity = usize::from(payload.is_some());
        if args.len() != expected_arity {
            return Err(TypeError::ArityMismatch {
                expected: expected_arity,
                found: args.len(),
            });
        }

        if let (Some(payload_ty), Some(arg)) = (payload.as_ref(), args.first()) {
            let actual = self.check_expr_with_expected(arg, Some(payload_ty))?;
            if !self.type_matches_expected(payload_ty, &actual) {
                return Err(typed_type_mismatch(payload_ty, &actual));
            }
        }

        Ok(enum_ty)
    }

    fn check_builtin_constructor_call(
        &mut self,
        variant: BuiltinVariant,
        args: &[Box<Expr>],
        expected: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        let expected_arity = match variant {
            BuiltinVariant::OptionNone => 0,
            BuiltinVariant::OptionSome | BuiltinVariant::ResultOk | BuiltinVariant::ResultErr => 1,
        };
        if args.len() != expected_arity {
            return Err(TypeError::ArityMismatch {
                expected: expected_arity,
                found: args.len(),
            });
        }

        match variant {
            BuiltinVariant::OptionSome => {
                let inferred_inner;
                let expected_inner = if let Some(TypedType::Option(inner)) = expected {
                    Some(inner.as_ref())
                } else if matches!(expected, Some(TypedType::InferVar(_))) {
                    inferred_inner = self.type_var_generator.fresh_var();
                    Some(&inferred_inner)
                } else {
                    None
                };
                let inner_type = self.check_expr_with_expected(&args[0], expected_inner)?;
                // Constructor payloads can remain polymorphic until a later
                // use constrains the enclosing Option. Keep the payload fact
                // alongside the constructor fact so Checked IR can recover
                // the argument value after substitutions are applied.
                self.record_checked_expr_type_allow_inference(&args[0], &inner_type);
                Ok(TypedType::Option(Box::new(inner_type)))
            }
            BuiltinVariant::OptionNone => {
                if let Some(TypedType::Option(inner)) = expected {
                    Ok(TypedType::Option(inner.clone()))
                } else if matches!(expected, Some(TypedType::InferVar(_))) {
                    Ok(TypedType::Option(Box::new(
                        self.type_var_generator.fresh_var(),
                    )))
                } else {
                    Err(TypeError::CannotInferType(
                        "Option::None requires an expected Option type".to_string(),
                    ))
                }
            }
            BuiltinVariant::ResultOk => match expected {
                Some(TypedType::Result(ok_ty, err_ty)) => {
                    let actual_ok = self.check_expr_with_expected(&args[0], Some(ok_ty))?;
                    self.record_checked_expr_type_allow_inference(&args[0], &actual_ok);
                    Ok(TypedType::Result(Box::new(actual_ok), err_ty.clone()))
                }
                Some(TypedType::InferVar(_)) => {
                    let inferred_ok = self.type_var_generator.fresh_var();
                    let inferred_err = self.type_var_generator.fresh_var();
                    let actual_ok = self.check_expr_with_expected(&args[0], Some(&inferred_ok))?;
                    self.record_checked_expr_type_allow_inference(&args[0], &actual_ok);
                    Ok(TypedType::Result(
                        Box::new(actual_ok),
                        Box::new(inferred_err),
                    ))
                }
                _ => Err(TypeError::CannotInferType(
                    "Result::Ok requires an expected Result type".to_string(),
                )),
            },
            BuiltinVariant::ResultErr => match expected {
                Some(TypedType::Result(ok_ty, err_ty)) => {
                    let actual_err = self.check_expr_with_expected(&args[0], Some(err_ty))?;
                    self.record_checked_expr_type_allow_inference(&args[0], &actual_err);
                    Ok(TypedType::Result(ok_ty.clone(), Box::new(actual_err)))
                }
                Some(TypedType::InferVar(_)) => {
                    let inferred_ok = self.type_var_generator.fresh_var();
                    let inferred_err = self.type_var_generator.fresh_var();
                    let actual_err =
                        self.check_expr_with_expected(&args[0], Some(&inferred_err))?;
                    self.record_checked_expr_type_allow_inference(&args[0], &actual_err);
                    Ok(TypedType::Result(
                        Box::new(inferred_ok),
                        Box::new(actual_err),
                    ))
                }
                _ => Err(TypeError::CannotInferType(
                    "Result::Err requires an expected Result type".to_string(),
                )),
            },
        }
    }

    fn builtin_constructor_expr(expr: &Expr) -> Option<(BuiltinVariant, Option<&Expr>)> {
        match &expr.kind {
            ExprKind::Call(call) => {
                let ExprKind::VariantRef(path) = &call.function.kind else {
                    return None;
                };
                let variant = path.builtin()?;
                let payload = match call.args.as_slice() {
                    [] => None,
                    [payload] => Some(payload.as_ref()),
                    _ => None,
                };
                Some((variant, payload))
            }
            ExprKind::Pipe(pipe) => {
                let PipeTarget::Expr(target) = &pipe.target else {
                    return None;
                };
                let ExprKind::VariantRef(path) = &target.kind else {
                    return None;
                };
                Some((path.builtin()?, Some(pipe.expr.as_ref())))
            }
            _ => None,
        }
    }

    fn check_block_expr(&mut self, block: &BlockExpr) -> Result<TypedType, TypeError> {
        self.check_block_expr_with_expected(block, None)
    }

    fn check_block_expr_with_expected(
        &mut self,
        block: &BlockExpr,
        expected: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        self.push_scope();

        let mut last_expr_type = None;

        for (i, stmt) in block.statements.iter().enumerate() {
            match stmt {
                Stmt::Binding(bind) => {
                    let inferred_later_expected = if bind.type_annotation.is_none() {
                        match &bind.pattern {
                            Pattern::Ident(bind_name) => self
                                .infer_unannotated_binding_expected_type_from_later_context(
                                    bind_name,
                                    &bind.value,
                                    &block.statements[i + 1..],
                                    block.expr.as_deref(),
                                    expected,
                                )?,
                            _ => None,
                        }
                    } else {
                        None
                    };
                    let expected_for_binding = match (
                        &bind.pattern,
                        block.expr.as_deref().map(|expr| &expr.kind),
                        expected,
                    ) {
                        (
                            Pattern::Ident(bind_name),
                            Some(ExprKind::Ident(return_name)),
                            Some(ty),
                        ) if bind.type_annotation.is_none() && bind_name == return_name => Some(ty),
                        _ => inferred_later_expected.as_ref(),
                    };
                    self.check_bind_decl_with_expected(bind, expected_for_binding)?
                }
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

        let unresolved_result = self.reject_unresolved_inference_in_current_scope();
        self.pop_scope();
        unresolved_result?;
        Ok(result)
    }

    fn infer_unannotated_binding_expected_type_from_later_context(
        &mut self,
        name: &str,
        value: &Expr,
        later_statements: &[Stmt],
        final_expr: Option<&Expr>,
        expected: Option<&TypedType>,
    ) -> Result<Option<TypedType>, TypeError> {
        if let Some(ty) = self.expected_type_for_returned_binding(name, final_expr, expected) {
            return Ok(Some(ty));
        }

        if let Some(ty) = self.infer_unannotated_variant_binding_expected_type_from_later_context(
            name,
            value,
            later_statements,
            final_expr,
            expected,
        )? {
            return Ok(Some(ty));
        }

        self.infer_unannotated_record_binding_expected_type_from_later_context(
            name,
            value,
            later_statements,
            final_expr,
            expected,
        )
    }

    fn expected_type_for_returned_binding(
        &self,
        name: &str,
        final_expr: Option<&Expr>,
        expected: Option<&TypedType>,
    ) -> Option<TypedType> {
        match (final_expr.map(|expr| &expr.kind), expected) {
            (Some(ExprKind::Ident(returned_name)), Some(ty)) if returned_name == name => {
                Some(ty.clone())
            }
            _ => None,
        }
    }

    fn infer_unannotated_variant_binding_expected_type_from_later_context(
        &mut self,
        name: &str,
        value: &Expr,
        later_statements: &[Stmt],
        final_expr: Option<&Expr>,
        expected: Option<&TypedType>,
    ) -> Result<Option<TypedType>, TypeError> {
        if Self::builtin_constructor_expr(value).is_none() {
            return Ok(None);
        }

        for stmt in later_statements {
            if let Stmt::Expr(expr) = stmt {
                if let Some(ty) =
                    self.infer_variant_binding_expected_type_from_expr(name, value, expr, expected)?
                {
                    return Ok(Some(ty));
                }
            }
        }

        if let Some(expr) = final_expr {
            return self.infer_variant_binding_expected_type_from_expr(name, value, expr, expected);
        }

        Ok(None)
    }

    fn infer_variant_binding_expected_type_from_expr(
        &mut self,
        binding_name: &str,
        value: &Expr,
        expr: &Expr,
        expected: Option<&TypedType>,
    ) -> Result<Option<TypedType>, TypeError> {
        let ExprKind::Match(match_expr) = &expr.kind else {
            return Ok(None);
        };
        if !Self::expr_is_ident(&match_expr.expr, binding_name) {
            return Ok(None);
        }

        let mut option_payload = None;
        let mut result_ok = None;
        let mut result_err = None;

        for arm in &match_expr.arms {
            match &arm.pattern {
                Pattern::Some(inner) => {
                    if let Some(ty) =
                        self.expected_type_for_payload_pattern(inner, &arm.body, expected)?
                    {
                        option_payload = Some(ty);
                    }
                }
                Pattern::Ok(inner) => {
                    if let Some(ty) =
                        self.expected_type_for_payload_pattern(inner, &arm.body, expected)?
                    {
                        result_ok = Some(ty);
                    }
                }
                Pattern::Err(inner) => {
                    if let Some(ty) =
                        self.expected_type_for_payload_pattern(inner, &arm.body, expected)?
                    {
                        result_err = Some(ty);
                    }
                }
                _ => {}
            }
        }

        Ok(
            match Self::builtin_constructor_expr(value).map(|(variant, _)| variant) {
                Some(BuiltinVariant::OptionSome | BuiltinVariant::OptionNone) => {
                    option_payload.map(|payload| TypedType::Option(Box::new(payload)))
                }
                Some(BuiltinVariant::ResultOk | BuiltinVariant::ResultErr) => {
                    match (result_ok, result_err) {
                        (Some(ok), Some(err)) => {
                            Some(TypedType::Result(Box::new(ok), Box::new(err)))
                        }
                        _ => None,
                    }
                }
                _ => None,
            },
        )
    }

    fn expected_type_for_payload_pattern(
        &mut self,
        pattern: &Pattern,
        body: &BlockExpr,
        expected: Option<&TypedType>,
    ) -> Result<Option<TypedType>, TypeError> {
        let Pattern::Ident(name) = pattern else {
            return Ok(None);
        };
        self.expected_type_for_ident_in_block(name, body, expected)
    }

    fn infer_unannotated_record_binding_expected_type_from_later_context(
        &mut self,
        name: &str,
        value: &Expr,
        later_statements: &[Stmt],
        final_expr: Option<&Expr>,
        expected: Option<&TypedType>,
    ) -> Result<Option<TypedType>, TypeError> {
        let ExprKind::RecordLit(record_lit) = &value.kind else {
            return Ok(None);
        };

        let Some(record_def) = self.records.get(&record_lit.name) else {
            return Ok(None);
        };
        let type_params = record_def.type_params.clone();
        let hash = record_def.hash.clone();
        let parent_hash = record_def.parent_hash.clone();
        let regular_type_params = Self::regular_type_param_names(&type_params);
        if regular_type_params.is_empty() {
            return Ok(Some(TypedType::Record {
                name: record_lit.name.clone(),
                type_args: Vec::new(),
                frozen: false,
                hash,
                parent_hash,
            }));
        }

        let type_args = regular_type_params
            .iter()
            .map(|_| self.type_var_generator.fresh_var())
            .collect::<Vec<_>>();
        let type_arg_bindings = Self::type_arg_bindings(&type_params, &type_args);
        let mut substitution = ConstraintSubstitution::new();

        for stmt in later_statements {
            if let Stmt::Expr(expr) = stmt {
                self.bind_record_binding_expected_type_params_from_expr(
                    name,
                    &record_lit.name,
                    expr,
                    expected,
                    &type_arg_bindings,
                    &mut substitution,
                )?;
            }
        }

        if let Some(expr) = final_expr {
            self.bind_record_binding_expected_type_params_from_expr(
                name,
                &record_lit.name,
                expr,
                expected,
                &type_arg_bindings,
                &mut substitution,
            )?;
        }

        let resolved_args = type_args
            .iter()
            .map(|arg| substitution.apply(arg))
            .collect::<Result<Vec<_>, _>>()?;
        if resolved_args
            .iter()
            .any(Self::contains_inference_internal_type)
        {
            return Ok(None);
        }

        Ok(Some(TypedType::Record {
            name: record_lit.name.clone(),
            type_args: resolved_args,
            frozen: false,
            hash,
            parent_hash,
        }))
    }

    fn bind_record_binding_expected_type_params_from_expr(
        &mut self,
        binding_name: &str,
        record_name: &str,
        expr: &Expr,
        expected: Option<&TypedType>,
        type_arg_bindings: &HashMap<String, TypedType>,
        substitution: &mut ConstraintSubstitution,
    ) -> Result<(), TypeError> {
        match &expr.kind {
            ExprKind::FieldAccess(object, field) => {
                if Self::expr_is_ident(object, binding_name) {
                    if let Some(field_ty) = expected {
                        self.bind_record_binding_field_expected_type(
                            record_name,
                            field,
                            field_ty,
                            type_arg_bindings,
                            substitution,
                        )?;
                    }
                }
            }
            ExprKind::Match(match_expr) => {
                if let ExprKind::FieldAccess(object, field) = &match_expr.expr.kind {
                    if Self::expr_is_ident(object, binding_name) {
                        self.bind_record_binding_field_match_expected_type(
                            record_name,
                            field,
                            match_expr,
                            expected,
                            type_arg_bindings,
                            substitution,
                        )?;
                    }
                }
            }
            ExprKind::Then(then) => {
                self.bind_record_binding_expected_type_params_from_block(
                    binding_name,
                    record_name,
                    &then.then_block,
                    expected,
                    type_arg_bindings,
                    substitution,
                )?;
                for (_, block) in &then.else_ifs {
                    self.bind_record_binding_expected_type_params_from_block(
                        binding_name,
                        record_name,
                        block,
                        expected,
                        type_arg_bindings,
                        substitution,
                    )?;
                }
                if let Some(block) = &then.else_block {
                    self.bind_record_binding_expected_type_params_from_block(
                        binding_name,
                        record_name,
                        block,
                        expected,
                        type_arg_bindings,
                        substitution,
                    )?;
                }
            }
            ExprKind::Block(block) => {
                self.bind_record_binding_expected_type_params_from_block(
                    binding_name,
                    record_name,
                    block,
                    expected,
                    type_arg_bindings,
                    substitution,
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    fn bind_record_binding_expected_type_params_from_block(
        &mut self,
        binding_name: &str,
        record_name: &str,
        block: &BlockExpr,
        expected: Option<&TypedType>,
        type_arg_bindings: &HashMap<String, TypedType>,
        substitution: &mut ConstraintSubstitution,
    ) -> Result<(), TypeError> {
        if let Some(expr) = &block.expr {
            self.bind_record_binding_expected_type_params_from_expr(
                binding_name,
                record_name,
                expr,
                expected,
                type_arg_bindings,
                substitution,
            )?;
        } else if let Some(Stmt::Expr(expr)) = block.statements.last() {
            self.bind_record_binding_expected_type_params_from_expr(
                binding_name,
                record_name,
                expr,
                expected,
                type_arg_bindings,
                substitution,
            )?;
        }
        Ok(())
    }

    fn bind_record_binding_field_expected_type(
        &self,
        record_name: &str,
        field: &str,
        field_ty: &TypedType,
        type_arg_bindings: &HashMap<String, TypedType>,
        substitution: &mut ConstraintSubstitution,
    ) -> Result<(), TypeError> {
        let Some(record_def) = self.records.get(record_name) else {
            return Ok(());
        };
        let Some(field_template) = record_def.fields.get(field) else {
            return Ok(());
        };
        let field_template = Self::apply_type_arg_bindings(field_template, type_arg_bindings);
        unify_constraint(&field_template, field_ty, substitution).or(Ok(()))
    }

    fn bind_record_binding_field_match_expected_type(
        &mut self,
        record_name: &str,
        field: &str,
        match_expr: &MatchExpr,
        expected: Option<&TypedType>,
        type_arg_bindings: &HashMap<String, TypedType>,
        substitution: &mut ConstraintSubstitution,
    ) -> Result<(), TypeError> {
        let Some(record_def) = self.records.get(record_name) else {
            return Ok(());
        };
        let Some(field_template) = record_def.fields.get(field) else {
            return Ok(());
        };
        let field_template = Self::apply_type_arg_bindings(field_template, type_arg_bindings);
        let mut context = VariantPayloadExpectedContext {
            field_template: &field_template,
            expected,
            substitution,
        };

        for arm in &match_expr.arms {
            match &arm.pattern {
                Pattern::Some(inner) => self.bind_variant_payload_expected_type_from_match_arm(
                    &mut context,
                    "Option",
                    0,
                    inner,
                    &arm.body,
                )?,
                Pattern::Ok(inner) => self.bind_variant_payload_expected_type_from_match_arm(
                    &mut context,
                    "Result",
                    0,
                    inner,
                    &arm.body,
                )?,
                Pattern::Err(inner) => self.bind_variant_payload_expected_type_from_match_arm(
                    &mut context,
                    "Result",
                    1,
                    inner,
                    &arm.body,
                )?,
                _ => {}
            }
        }
        Ok(())
    }

    fn bind_variant_payload_expected_type_from_match_arm(
        &mut self,
        context: &mut VariantPayloadExpectedContext<'_>,
        variant_type_name: &str,
        payload_index: usize,
        payload_pattern: &Pattern,
        body: &BlockExpr,
    ) -> Result<(), TypeError> {
        let Pattern::Ident(name) = payload_pattern else {
            return Ok(());
        };
        let Some(payload_expected) =
            self.expected_type_for_ident_in_block(name, body, context.expected)?
        else {
            return Ok(());
        };
        let Some(payload_template) = Self::variant_payload_template(
            context.field_template,
            variant_type_name,
            payload_index,
        ) else {
            return Ok(());
        };
        unify_constraint(payload_template, &payload_expected, context.substitution).or(Ok(()))
    }

    fn variant_payload_template<'a>(
        field_template: &'a TypedType,
        variant_type_name: &str,
        payload_index: usize,
    ) -> Option<&'a TypedType> {
        match (field_template, variant_type_name, payload_index) {
            (TypedType::Option(inner), "Option", 0) => Some(inner),
            (TypedType::Result(ok, _), "Result", 0) => Some(ok),
            (TypedType::Result(_, err), "Result", 1) => Some(err),
            _ => None,
        }
    }

    fn expected_type_for_ident_in_block(
        &mut self,
        name: &str,
        block: &BlockExpr,
        expected: Option<&TypedType>,
    ) -> Result<Option<TypedType>, TypeError> {
        if let Some(expr) = &block.expr {
            return self.expected_type_for_ident_in_expr(name, expr, expected);
        }

        if let Some(Stmt::Expr(expr)) = block.statements.last() {
            return self.expected_type_for_ident_in_expr(name, expr, expected);
        }

        Ok(None)
    }

    fn expected_type_for_ident_in_expr(
        &mut self,
        name: &str,
        expr: &Expr,
        expected: Option<&TypedType>,
    ) -> Result<Option<TypedType>, TypeError> {
        if Self::expr_is_ident(expr, name) {
            return Ok(expected.cloned());
        }

        match &expr.kind {
            ExprKind::Pipe(pipe) => {
                if Self::expr_is_ident(&pipe.expr, name) {
                    return self.expected_type_for_pipe_target_first_arg(&pipe.target, &pipe.expr);
                }
            }
            ExprKind::Call(call) => {
                if let ExprKind::Ident(func_name) = &call.function.kind {
                    for (index, arg) in call.args.iter().enumerate() {
                        if Self::expr_is_ident(arg, name) {
                            return self.expected_function_param_type_for_call(
                                func_name, index, &call.args,
                            );
                        }
                    }
                }
            }
            ExprKind::Block(block) => {
                return self.expected_type_for_ident_in_block(name, block, expected);
            }
            ExprKind::Then(then) => {
                if let Some(ty) =
                    self.expected_type_for_ident_in_block(name, &then.then_block, expected)?
                {
                    return Ok(Some(ty));
                }
                for (_, block) in &then.else_ifs {
                    if let Some(ty) =
                        self.expected_type_for_ident_in_block(name, block, expected)?
                    {
                        return Ok(Some(ty));
                    }
                }
                if let Some(block) = &then.else_block {
                    return self.expected_type_for_ident_in_block(name, block, expected);
                }
            }
            _ => {}
        }

        Ok(None)
    }

    fn expected_type_for_pipe_target_first_arg(
        &mut self,
        target: &PipeTarget,
        receiver: &Expr,
    ) -> Result<Option<TypedType>, TypeError> {
        let func_name = match target {
            PipeTarget::Ident(func_name) => func_name,
            PipeTarget::Expr(expr) => match &expr.kind {
                ExprKind::Ident(func_name) => func_name,
                _ => return Ok(None),
            },
        };

        match self.peek_var_type(func_name) {
            Some(TypedType::Function { .. }) => {
                return self.expected_function_param_type_for_call_mode(
                    func_name,
                    0,
                    &[Box::new(receiver.clone())],
                    false,
                );
            }
            Some(_) => return Ok(None),
            None => {}
        }

        if self.receiver_has_form_method_selector(receiver, func_name) {
            return Ok(self.expected_osv_method_param_type(
                func_name,
                0,
                &[Box::new(receiver.clone())],
            ));
        }

        self.expected_function_param_type_for_call_mode(
            func_name,
            0,
            &[Box::new(receiver.clone())],
            false,
        )
    }

    fn expected_function_param_type_for_call(
        &mut self,
        func_name: &str,
        target_index: usize,
        args: &[Box<Expr>],
    ) -> Result<Option<TypedType>, TypeError> {
        self.expected_function_param_type_for_call_mode(func_name, target_index, args, true)
    }

    fn expected_function_param_type_for_call_mode(
        &mut self,
        func_name: &str,
        target_index: usize,
        args: &[Box<Expr>],
        allow_receiver_methods: bool,
    ) -> Result<Option<TypedType>, TypeError> {
        if let Some(TypedType::Function { params, .. }) = self.peek_var_type(func_name) {
            if params.len() != args.len() {
                return Ok(None);
            }
            return Ok(params.get(target_index).cloned());
        }

        if allow_receiver_methods
            && args
                .first()
                .is_some_and(|receiver| self.receiver_has_method_selector(receiver, func_name))
        {
            return Ok(self.expected_osv_method_param_type(func_name, target_index, args));
        }

        let Some(function) = self.functions.get(func_name).cloned() else {
            return Ok(None);
        };
        if function.params.len() != args.len() {
            return Ok(None);
        }
        if function.type_params.is_empty() {
            return Ok(function.params.get(target_index).map(|(_, ty)| ty.clone()));
        }

        let type_param_names = function
            .type_params
            .iter()
            .map(|param| param.name.clone())
            .collect::<Vec<_>>();
        let type_vars = fresh_type_param_map(&type_param_names, &mut self.type_var_generator);
        let mut substitution = ConstraintSubstitution::new();

        for (index, (arg, (_, param_ty))) in args.iter().zip(function.params.iter()).enumerate() {
            if index == target_index {
                continue;
            }
            let Some(arg_ty) = self.non_consuming_expected_context_expr_type(arg) else {
                continue;
            };
            let instantiated_param = substitute_type_params(param_ty, &type_vars);
            if unify_constraint(&instantiated_param, &arg_ty, &mut substitution).is_err() {
                return Ok(None);
            }
        }

        let Some((_, target_param_ty)) = function.params.get(target_index) else {
            return Ok(None);
        };
        let instantiated_target = substitute_type_params(target_param_ty, &type_vars);
        let resolved_target = substitution.apply(&instantiated_target)?;
        if Self::contains_inference_internal_type(&resolved_target) {
            return Ok(None);
        }

        Ok(Some(resolved_target))
    }

    fn expected_osv_method_param_type(
        &self,
        method_name: &str,
        target_index: usize,
        args: &[Box<Expr>],
    ) -> Option<TypedType> {
        let receiver_ty = self.infer_method_receiver_type_non_consuming(args.first()?)?;
        let method = match &receiver_ty {
            TypedType::TypeParam(param_name) => {
                let mut candidates = self
                    .get_form_bounds(param_name)
                    .into_iter()
                    .filter_map(|form_name| {
                        self.forms
                            .get(&form_name)
                            .and_then(|form| form.methods.get(method_name))
                            .cloned()
                            .map(|method| (form_name, method))
                    })
                    .collect::<Vec<_>>();
                candidates.sort_by(|left, right| left.0.cmp(&right.0));
                candidates.dedup_by(|left, right| left.0 == right.0);
                if candidates.len() != 1 {
                    return None;
                }
                let (_, mut method) = candidates.pop()?;
                method.params = method
                    .params
                    .into_iter()
                    .map(|(name, ty)| (name, Self::substitute_form_self(&ty, &receiver_ty)))
                    .collect();
                method.return_type = Self::substitute_form_self(&method.return_type, &receiver_ty);
                method
            }
            TypedType::Record { name, .. } => self
                .methods
                .get(name)
                .and_then(|methods| methods.get(method_name))
                .cloned()?,
            TypedType::Temporal { base_type, .. } => match base_type.as_ref() {
                TypedType::Record { name, .. } => self
                    .methods
                    .get(name)
                    .and_then(|methods| methods.get(method_name))
                    .cloned()?,
                _ => return None,
            },
            _ => return None,
        };

        if !method.type_params.is_empty() || method.params.len() != args.len() {
            return None;
        }
        method.params.get(target_index).map(|(_, ty)| ty.clone())
    }

    fn non_consuming_expected_context_expr_type(&self, expr: &Expr) -> Option<TypedType> {
        if let Some((BuiltinVariant::OptionSome, Some(inner))) =
            Self::builtin_constructor_expr(expr)
        {
            return self
                .non_consuming_expected_context_expr_type(inner)
                .map(|ty| TypedType::Option(Box::new(ty)));
        }

        match &expr.kind {
            ExprKind::IntLit(value) => Some(Self::int_literal_type(*value)),
            ExprKind::FloatLit(_) => Some(TypedType::Float64),
            ExprKind::StringLit(_) => Some(TypedType::String),
            ExprKind::CharLit(_) => Some(TypedType::Char),
            ExprKind::BoolLit(_) => Some(TypedType::Boolean),
            ExprKind::Unit => Some(TypedType::Unit),
            ExprKind::Ident(name) => self
                .peek_var_type(name)
                .and_then(|ty| (!Self::contains_inference_internal_type(&ty)).then_some(ty)),
            ExprKind::ListLit(elements) => {
                let mut element_ty = None;
                for element in elements {
                    let ty = self.non_consuming_expected_context_expr_type(element)?;
                    if let Some(previous) = &element_ty {
                        if !self.type_matches_expected(previous, &ty) {
                            return None;
                        }
                    } else {
                        element_ty = Some(ty);
                    }
                }
                element_ty.map(|ty| TypedType::List(Box::new(ty)))
            }
            ExprKind::ArrayLit(elements) => {
                let mut element_ty = None;
                for element in elements {
                    let ty = self.non_consuming_expected_context_expr_type(element)?;
                    if let Some(previous) = &element_ty {
                        if !self.type_matches_expected(previous, &ty) {
                            return None;
                        }
                    } else {
                        element_ty = Some(ty);
                    }
                }
                element_ty.map(|ty| TypedType::Array(Box::new(ty), ArrayLength::AnyInternal))
            }
            _ => None,
        }
    }

    fn int_literal_type(value: i64) -> TypedType {
        if i32::try_from(value).is_ok() {
            TypedType::Int32
        } else {
            TypedType::Int64
        }
    }

    fn expr_is_ident(expr: &Expr, name: &str) -> bool {
        matches!(&expr.kind, ExprKind::Ident(candidate) if candidate == name)
    }

    fn check_binary_expr(
        &mut self,
        binary: &BinaryExpr,
        expected: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        // For arithmetic ops, if we expect a certain numeric type,
        // propagate that expectation to both operands
        let (expected_left, expected_right) = match binary.op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                // These return the same type as their operands
                // So if we expect Int32 or Float64, both operands should be that type
                match (&binary.op, expected) {
                    (&BinaryOp::Add, Some(TypedType::String)) => {
                        (Some(&TypedType::String), Some(&TypedType::String))
                    }
                    (_, Some(TypedType::Int32)) => {
                        (Some(&TypedType::Int32), Some(&TypedType::Int32))
                    }
                    (_, Some(TypedType::Int64)) => {
                        (Some(&TypedType::Int64), Some(&TypedType::Int64))
                    }
                    (_, Some(TypedType::Float64)) => {
                        (Some(&TypedType::Float64), Some(&TypedType::Float64))
                    }
                    _ => (None, None),
                }
            }
            _ => (None, None),
        };

        let (mut left_ty, mut right_ty) = if expected_left.is_none()
            && expected_right.is_none()
            && Self::is_int_literal_expr(&binary.left)
            && !Self::is_int_literal_expr(&binary.right)
        {
            let right_ty = self.check_expr_with_expected(&binary.right, None)?;
            let left_expected = Self::contextual_binary_operand_type(&binary.op, &right_ty);
            let left_ty = self.check_expr_with_expected(&binary.left, left_expected)?;
            (left_ty, right_ty)
        } else {
            let left_ty = self.check_expr_with_expected(&binary.left, expected_left)?;
            let right_expected = expected_right
                .or_else(|| Self::contextual_binary_operand_type(&binary.op, &left_ty));
            let right_ty = self.check_expr_with_expected(&binary.right, right_expected)?;
            (left_ty, right_ty)
        };

        if Self::contains_inference_internal_type(&left_ty)
            || Self::contains_inference_internal_type(&right_ty)
        {
            let mut substitution = ConstraintSubstitution::new();
            unify_constraint(&left_ty, &right_ty, &mut substitution)?;
            left_ty =
                self.update_direct_ident_from_substitution(&binary.left, &left_ty, &substitution)?;
            right_ty = self.update_direct_ident_from_substitution(
                &binary.right,
                &right_ty,
                &substitution,
            )?;
        }

        // Type check based on operator
        match binary.op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                // Arithmetic operators require numeric types
                match (&left_ty, &right_ty) {
                    (TypedType::Int32, TypedType::Int32) => Ok(TypedType::Int32),
                    (TypedType::Int64, TypedType::Int64) => Ok(TypedType::Int64),
                    (TypedType::Float64, TypedType::Float64) => Ok(TypedType::Float64),
                    (TypedType::String, TypedType::String) if binary.op == BinaryOp::Add => {
                        Ok(TypedType::String)
                    }
                    _ => Err(TypeError::TypeMismatch {
                        expected: if binary.op == BinaryOp::Add {
                            "numeric types or String operands".to_string()
                        } else {
                            "numeric types".to_string()
                        },
                        found: Self::format_type_pair(&left_ty, &right_ty),
                    }),
                }
            }
            BinaryOp::Eq | BinaryOp::Ne => {
                // Enum values use an internal pointer representation. Until
                // structural tag/payload equality is specified and lowered,
                // accepting equality here would silently compare allocation
                // addresses rather than enum values.
                if matches!(
                    (&left_ty, &right_ty),
                    (
                        TypedType::Enum { name: left_name },
                        TypedType::Enum { name: right_name }
                    ) if left_name == right_name
                ) {
                    Err(TypeError::UnsupportedFeature(
                        "user-defined enum equality is not supported; use match on qualified variants"
                            .to_string(),
                    ))
                } else if left_ty == right_ty {
                    Ok(TypedType::Boolean)
                } else {
                    Err(TypeError::TypeMismatch {
                        expected: format_typed_type(&left_ty),
                        found: format_typed_type(&right_ty),
                    })
                }
            }
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                // Comparison operators require numeric types
                match (&left_ty, &right_ty) {
                    (TypedType::Int32, TypedType::Int32) => Ok(TypedType::Boolean),
                    (TypedType::Int64, TypedType::Int64) => Ok(TypedType::Boolean),
                    (TypedType::Float64, TypedType::Float64) => Ok(TypedType::Boolean),
                    _ => Err(TypeError::TypeMismatch {
                        expected: "numeric types".to_string(),
                        found: Self::format_type_pair(&left_ty, &right_ty),
                    }),
                }
            }
            BinaryOp::And | BinaryOp::Or => match (&left_ty, &right_ty) {
                (TypedType::Boolean, TypedType::Boolean) => Ok(TypedType::Boolean),
                _ => Err(TypeError::TypeMismatch {
                    expected: "Boolean operands".to_string(),
                    found: Self::format_type_pair(&left_ty, &right_ty),
                }),
            },
        }
    }

    fn is_int_literal_expr(expr: &Expr) -> bool {
        matches!(&expr.kind, ExprKind::IntLit(_))
            || matches!(
                &expr.kind,
                ExprKind::Unary(UnaryExpr {
                    op: UnaryOp::Neg,
                    expr,
                }) if matches!(&expr.kind, ExprKind::IntLit(_))
            )
    }

    fn contextual_binary_operand_type<'a>(
        op: &BinaryOp,
        ty: &'a TypedType,
    ) -> Option<&'a TypedType> {
        match op {
            BinaryOp::Add => match ty {
                TypedType::Int32 | TypedType::Int64 | TypedType::Float64 | TypedType::String => {
                    Some(ty)
                }
                _ => None,
            },
            BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => match ty {
                TypedType::Int32 | TypedType::Int64 | TypedType::Float64 => Some(ty),
                _ => None,
            },
            BinaryOp::Eq | BinaryOp::Ne => match ty {
                TypedType::Int32
                | TypedType::Int64
                | TypedType::Float64
                | TypedType::Boolean
                | TypedType::Char => Some(ty),
                _ => None,
            },
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => match ty {
                TypedType::Int32 | TypedType::Int64 | TypedType::Float64 => Some(ty),
                _ => None,
            },
            BinaryOp::And | BinaryOp::Or => match ty {
                TypedType::Boolean => Some(ty),
                _ => None,
            },
        }
    }

    fn check_unary_expr(
        &mut self,
        unary: &UnaryExpr,
        expected: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        match unary.op {
            UnaryOp::Neg => {
                let expected_operand = match expected {
                    Some(TypedType::Int32) => Some(&TypedType::Int32),
                    Some(TypedType::Int64) => Some(&TypedType::Int64),
                    Some(TypedType::Float64) => Some(&TypedType::Float64),
                    _ => None,
                };
                let operand_ty = self.check_expr_with_expected(&unary.expr, expected_operand)?;
                match operand_ty {
                    TypedType::Int32 => Ok(TypedType::Int32),
                    TypedType::Int64 => Ok(TypedType::Int64),
                    TypedType::Float64 => Ok(TypedType::Float64),
                    other => Err(expected_type_mismatch("numeric type", &other)),
                }
            }
            UnaryOp::Not => {
                let operand_ty =
                    self.check_expr_with_expected(&unary.expr, Some(&TypedType::Boolean))?;
                match operand_ty {
                    TypedType::Boolean => Ok(TypedType::Boolean),
                    other => Err(expected_type_mismatch("Boolean operand", &other)),
                }
            }
        }
    }

    fn check_cast_expr(&mut self, cast: &CastExpr) -> Result<TypedType, TypeError> {
        let source_ty = self.check_expr(&cast.expr)?;
        let target_ty = self.convert_type(&cast.target)?;

        if Self::is_numeric_cast_type(&source_ty) && Self::is_numeric_cast_type(&target_ty) {
            Ok(target_ty)
        } else {
            Err(TypeError::TypeMismatch {
                expected: "numeric cast between Int32, Int64, or Float64".to_string(),
                found: format!(
                    "{} as {}",
                    format_typed_type(&source_ty),
                    format_typed_type(&target_ty)
                ),
            })
        }
    }

    fn is_numeric_cast_type(ty: &TypedType) -> bool {
        matches!(ty, TypedType::Int32 | TypedType::Int64 | TypedType::Float64)
    }

    fn check_pipe_expr_with_expected(
        &mut self,
        pipe: &PipeExpr,
        expected: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        match &pipe.target {
            PipeTarget::Ident(name) => {
                let function_variable =
                    matches!(self.peek_var_type(name), Some(TypedType::Function { .. }));
                if function_variable {
                    // Pipe to function: expr |> function. The desugared call
                    // checks an id-preserving clone of the pipe object, so its
                    // facts are recorded directly under the source node ids.
                    let call = CallExpr {
                        function: Box::new(Expr::new(ExprKind::Ident(name.clone()))),
                        args: vec![pipe.expr.clone()],
                    };
                    self.check_call_expr_with_expected_mode(&call, expected, false)
                } else if self.receiver_has_form_method_selector(&pipe.expr, name) {
                    let call = CallExpr {
                        function: Box::new(Expr::new(ExprKind::Ident(name.clone()))),
                        args: vec![pipe.expr.clone()],
                    };
                    self.check_call_expr_with_expected(&call, expected)
                } else if self.functions.contains_key(name) {
                    let call = CallExpr {
                        function: Box::new(Expr::new(ExprKind::Ident(name.clone()))),
                        args: vec![pipe.expr.clone()],
                    };
                    self.check_call_expr_with_expected_mode(&call, expected, false)
                } else if matches!(name.as_str(), "some" | "none") {
                    Err(lowercase_option_constructor_error(name))
                } else {
                    // Pipe to binding: expr |> name
                    let expr_ty = self.check_expr(&pipe.expr)?;
                    self.bind_var(name.clone(), expr_ty.clone(), false)?;
                    Ok(expr_ty)
                }
            }
            PipeTarget::Expr(target_expr) => {
                // Pipe to expression: expr |> (func_expr). As above, the
                // cloned object and target keep their ids, so checking the
                // desugared call records facts under the source nodes.
                let call = CallExpr {
                    function: target_expr.clone(),
                    args: vec![pipe.expr.clone()],
                };
                let allow_form_method = match &target_expr.kind {
                    ExprKind::Ident(name) => {
                        !matches!(self.peek_var_type(name), Some(TypedType::Function { .. }))
                            && self.receiver_has_form_method_selector(&pipe.expr, name)
                    }
                    _ => false,
                };
                self.check_call_expr_with_expected_mode(&call, expected, allow_form_method)
            }
        }
    }

    fn check_with_expr_with_expected(
        &mut self,
        with: &WithExpr,
        expected: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        // Push context onto the stack
        let original_len = self._contexts.len();
        let mut entered_async_runtime = false;
        let mut context_bindings: Vec<(String, TypedType)> = Vec::new();

        // Check if it's a built-in context or a user-defined context
        let ctx_name = &with.context_name;
        let is_arena_context = ctx_name == "Arena";
        if is_arena_context {
            // Arena is a built-in context
            self._contexts.push(ctx_name.clone());
        } else if ctx_name.starts_with("AsyncRuntime") {
            // AsyncRuntime context with lifetime parameter
            // Extract lifetime from AsyncRuntime<~async>
            if let Some(lifetime) = self.extract_async_runtime_lifetime(ctx_name) {
                self.enter_async_runtime(&lifetime)?;
                entered_async_runtime = true;
            } else {
                return Err(TypeError::UnavailableContext(format!(
                    "Invalid AsyncRuntime syntax: {}",
                    ctx_name
                )));
            }
            self._contexts.push(ctx_name.clone());
        } else if self.records.contains_key(ctx_name) {
            // User-defined context - add to context stack
            self._contexts.push(ctx_name.clone());
            let field_types = self
                .records
                .get(ctx_name)
                .map(|record| record.fields.clone())
                .ok_or_else(|| TypeError::UnavailableContext(ctx_name.clone()))?;

            // Type check field bindings if any
            for binding in &with.bindings {
                match binding {
                    FieldInit::Field { name, value } => {
                        let expected_ty = match field_types.get(name) {
                            Some(ty) => ty,
                            None => {
                                self._contexts.truncate(original_len);
                                return Err(TypeError::UnknownField {
                                    record: ctx_name.clone(),
                                    field: name.clone(),
                                });
                            }
                        };
                        let actual_ty =
                            match self.check_expr_with_expected(value, Some(expected_ty)) {
                                Ok(ty) => ty,
                                Err(err) => {
                                    self._contexts.truncate(original_len);
                                    return Err(err);
                                }
                            };
                        if !self.type_matches_expected(expected_ty, &actual_ty) {
                            self._contexts.truncate(original_len);
                            return Err(typed_type_mismatch(expected_ty, &actual_ty));
                        }
                        context_bindings.push((name.clone(), expected_ty.clone()));
                    }
                    FieldInit::Spread(_expr) => {
                        // Spread operations not currently supported in context bindings
                        self._contexts.truncate(original_len);
                        return Err(TypeError::UnavailableContext(
                            "Spread operations not supported in context bindings".to_string(),
                        ));
                    }
                }
            }
        } else {
            return Err(TypeError::UnavailableContext(ctx_name.clone()));
        }

        // Check the body with context available
        let has_binding_scope = !context_bindings.is_empty();
        if has_binding_scope {
            self.push_scope();
            for (name, ty) in &context_bindings {
                if let Err(err) = self.bind_var(name.clone(), ty.clone(), false) {
                    self.pop_scope();
                    self._contexts.truncate(original_len);
                    return Err(err);
                }
            }
        }
        let result = self.check_block_expr_with_expected(&with.body, expected);
        if has_binding_scope {
            self.pop_scope();
        }

        // Pop context and exit AsyncRuntime if needed
        let cleanup_result = if entered_async_runtime {
            self.exit_async_runtime().map(|_| ())
        } else {
            Ok(())
        };
        self._contexts.truncate(original_len);

        cleanup_result?;
        let result_ty = result?;
        if is_arena_context {
            self.check_arena_result_escape(&result_ty)?;
        }
        Ok(result_ty)
    }

    fn check_arena_result_escape(&self, ty: &TypedType) -> Result<(), TypeError> {
        if Self::is_arena_scalar_result(ty) {
            Ok(())
        } else {
            Err(TypeError::ArenaEscape(format_typed_type(ty)))
        }
    }

    fn is_arena_scalar_result(ty: &TypedType) -> bool {
        match ty {
            TypedType::Int32
            | TypedType::Int64
            | TypedType::Float64
            | TypedType::Boolean
            | TypedType::Char
            | TypedType::Unit => true,
            TypedType::Temporal { base_type, .. } => Self::is_arena_scalar_result(base_type),
            _ => false,
        }
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
                return Err(TypeError::TemporalConstraintViolation(format!(
                    "Temporal variable {} is not in scope",
                    temporal
                )));
            }
        }

        // Validate constraint transitivity
        // If we have constraints A within B and B within C, then A must be within C
        let constraints = &self.temporal_context.constraints;

        // Build a map of direct constraints
        let mut within_map: HashMap<String, HashSet<String>> = HashMap::new();
        for constraint in constraints {
            within_map
                .entry(constraint.inner.clone())
                .or_default()
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
                within_map.entry(inner).or_default().insert(outer);
            }
        }

        // Check for cycles
        for (temporal, within_set) in &within_map {
            if within_set.contains(temporal) {
                return Err(TypeError::TemporalConstraintViolation(format!(
                    "Cyclic temporal constraint detected: {} within itself",
                    temporal
                )));
            }
        }

        Ok(())
    }

    /// Check await expression.
    /// For now, await is treated as a built-in function.
    fn check_await_expr(&mut self, expr: &Expr) -> Result<TypedType, TypeError> {
        // Verify we're in an AsyncRuntime context
        if !self.is_in_async_runtime() {
            return Err(TypeError::UnsupportedFeature(
                "await can only be used within an AsyncRuntime context".to_string(),
            ));
        }

        // Check the expression being awaited
        let task_type = self.check_expr(expr)?;

        // Get the current async runtime lifetime
        let async_lifetime = self
            .current_async_runtime()
            .ok_or_else(|| {
                TypeError::UnsupportedFeature("No AsyncRuntime context available".to_string())
            })?
            .clone();

        // Verify that we have a Task<T, ~async> type
        match &task_type {
            TypedType::Temporal {
                base_type,
                temporals,
            } => {
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
                                found: format!("Task with temporals: {}", temporals.join(", ")),
                            })
                        }
                    } else {
                        Err(expected_type_mismatch(
                            format!("Task<T, ~{}>", async_lifetime),
                            &task_type,
                        ))
                    }
                } else {
                    Err(expected_type_mismatch(
                        format!("Task<T, ~{}>", async_lifetime),
                        &task_type,
                    ))
                }
            }
            TypedType::Record { name, .. } if name == "Task" => {
                // Handle non-temporal Task for backwards compatibility
                // In a full implementation, this would be an error
                let result_type = self.get_task_result_type(&task_type)?;
                Ok(result_type)
            }
            _ => Err(expected_type_mismatch(
                format!("Task<T, ~{}>", async_lifetime),
                &task_type,
            )),
        }
    }

    /// Check spawn expression.
    /// For now, spawn is treated as a built-in function.
    fn check_spawn_expr(&mut self, expr: &Expr) -> Result<TypedType, TypeError> {
        // Verify we're in an AsyncRuntime context
        if !self.is_in_async_runtime() {
            return Err(TypeError::UnsupportedFeature(
                "spawn can only be used within an AsyncRuntime context".to_string(),
            ));
        }

        // Check the expression being spawned (should be a lambda or async function)
        let func_type = self.check_expr(expr)?;

        // Extract the return type from the function being spawned
        let _return_type = match &func_type {
            TypedType::Function { return_type, .. } => return_type.as_ref().clone(),
            _ => {
                return Err(expected_type_mismatch("function", &func_type));
            }
        };

        // Get the current async runtime lifetime
        let async_lifetime = self
            .current_async_runtime()
            .ok_or_else(|| {
                TypeError::UnsupportedFeature("No AsyncRuntime context available".to_string())
            })?
            .clone();

        // Return Task<T, ~async> where T is the return type of the spawned function
        Ok(TypedType::Temporal {
            base_type: Box::new(TypedType::Record {
                name: "Task".to_string(),
                type_args: Vec::new(),
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
            _ => Err(expected_type_mismatch("Task", task_type)),
        }
    }

    /// Check a with lifetime expression.
    ///
    /// Creates a new temporal scope for the lifetime of the block.
    fn check_with_lifetime_expr(
        &mut self,
        with_lifetime: &WithLifetimeExpr,
    ) -> Result<TypedType, TypeError> {
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
                        constraint.outer.clone(),
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

    fn check_then_expr_with_expected(
        &mut self,
        then: &ThenExpr,
        expected: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        // Check condition is boolean
        let cond_ty = self.check_expr(&then.condition)?;
        if cond_ty != TypedType::Boolean {
            return Err(expected_type_mismatch("Boolean", &cond_ty));
        }

        let branch_base = self.var_env.clone();
        let mut branch_envs = Vec::new();
        let mut branch_types = Vec::new();
        let inferred_result_type = if expected.is_none() {
            Some(self.type_var_generator.fresh_var())
        } else {
            None
        };
        let branch_expected = expected.or(inferred_result_type.as_ref());
        let finalize_result = expected.is_none_or(|ty| !Self::contains_inference_internal_type(ty));

        // Check then branch from the post-condition environment. Branches are
        // mutually exclusive, so usage in one branch must not pre-consume the
        // same affine value for the next branch during checking.
        let (then_ty, then_env) = self.check_branch_from_env(&branch_base, |checker| {
            checker.push_scope();
            let result = checker.check_block_expr_with_expected(&then.then_block, branch_expected);
            checker.pop_scope();
            result
        })?;
        branch_envs.push(then_env);
        branch_types.push(then_ty);

        // Check else-if branches
        for (else_cond, else_block) in &then.else_ifs {
            let (else_if_ty, else_if_env) =
                self.check_branch_from_env(&branch_base, |checker| {
                    let else_cond_ty = checker.check_expr(else_cond)?;
                    if else_cond_ty != TypedType::Boolean {
                        return Err(expected_type_mismatch("Boolean", &else_cond_ty));
                    }

                    checker.push_scope();
                    let result =
                        checker.check_block_expr_with_expected(else_block, branch_expected);
                    checker.pop_scope();
                    result
                })?;
            branch_envs.push(else_if_env);
            branch_types.push(else_if_ty);
        }

        // Check else branch
        if let Some(else_block) = &then.else_block {
            let (else_ty, else_env) = self.check_branch_from_env(&branch_base, |checker| {
                checker.push_scope();
                let result = checker.check_block_expr_with_expected(else_block, branch_expected);
                checker.pop_scope();
                result
            })?;
            branch_envs.push(else_env);
            branch_types.push(else_ty);
        } else {
            branch_types.push(TypedType::Unit);
            branch_envs.push(branch_base.clone());
        }

        let (result_ty, branch_substitution) = Self::resolve_branch_result_type(
            branch_expected.expect("branch result expected type is always initialized"),
            &branch_types,
            finalize_result,
        )?;

        self.merge_branch_var_usage(branch_base, &branch_envs)?;
        self.apply_substitution_to_var_env(&branch_substitution)?;
        Ok(result_ty)
    }

    fn check_while_expr(&mut self, while_expr: &WhileExpr) -> Result<TypedType, TypeError> {
        // Check condition is boolean
        let cond_type = self.check_expr(&while_expr.condition)?;
        if cond_type != TypedType::Boolean {
            return Err(expected_type_mismatch("Boolean", &cond_type));
        }

        // Check body in new scope
        self.push_scope();
        self.check_block_expr(&while_expr.body)?;
        self.pop_scope();

        // While loops always return Unit
        Ok(TypedType::Unit)
    }

    fn check_match_expr_with_expected(
        &mut self,
        match_expr: &MatchExpr,
        expected: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        // Check the scrutinee expression
        let scrutinee_type = self.check_expr(&match_expr.expr)?;

        // Check that we have at least one arm
        if match_expr.arms.is_empty() {
            return Err(TypeError::TypeMismatch {
                expected: "at least one match arm".to_string(),
                found: "no match arms".to_string(),
            });
        }

        // Check each arm and solve all sibling result types together. This lets
        // an early `[]` or `None` arm use concrete type information from a
        // later sibling without re-checking branch bodies or disturbing affine
        // usage snapshots.
        let branch_base = self.var_env.clone();
        let mut branch_envs = Vec::new();
        let mut branch_types = Vec::new();
        let inferred_result_type = if expected.is_none() {
            Some(self.type_var_generator.fresh_var())
        } else {
            None
        };
        let branch_expected = expected.or(inferred_result_type.as_ref());
        let finalize_result = expected.is_none_or(|ty| !Self::contains_inference_internal_type(ty));

        for arm in &match_expr.arms {
            // Check pattern compatibility with scrutinee
            self.check_pattern(&arm.pattern, &scrutinee_type)?;

            let (arm_type, arm_env) = self.check_branch_from_env(&branch_base, |checker| {
                checker.push_scope();
                checker.bind_pattern_vars(&arm.pattern, &scrutinee_type)?;

                let result = checker.check_block_expr_with_expected(&arm.body, branch_expected);

                checker.pop_scope();
                result
            })?;
            branch_envs.push(arm_env);
            branch_types.push(arm_type);
        }

        // Check exhaustiveness with detailed error reporting
        if !self.is_pattern_exhaustive(&match_expr.arms, &scrutinee_type) {
            // Get specific missing patterns for better error message
            if let Err(missing_patterns) =
                self.check_exhaustiveness_coverage(&match_expr.arms, &scrutinee_type)
            {
                let missing_list = missing_patterns.join(", ");
                return Err(TypeError::NonExhaustivePatterns {
                    missing: missing_list,
                    suggestion: "Add the missing patterns or use a wildcard pattern (_)"
                        .to_string(),
                });
            } else {
                // Fallback to generic message
                return Err(TypeError::TypeMismatch {
                    expected: "exhaustive patterns".to_string(),
                    found: "non-exhaustive patterns".to_string(),
                });
            }
        }

        let (result_type, branch_substitution) = Self::resolve_branch_result_type(
            branch_expected.expect("branch result expected type is always initialized"),
            &branch_types,
            finalize_result,
        )?;

        self.merge_branch_var_usage(branch_base, &branch_envs)?;
        self.apply_substitution_to_var_env(&branch_substitution)?;
        Ok(result_type)
    }

    fn check_pattern(&self, pattern: &Pattern, expected_type: &TypedType) -> Result<(), TypeError> {
        match pattern {
            Pattern::Wildcard => Ok(()),
            Pattern::Ident(_) => Ok(()), // Binds to any type
            Pattern::Literal(lit) => {
                let lit_type = match lit {
                    Literal::Int(value) => self.check_int_lit(*value, Some(expected_type))?,
                    Literal::Float(_) => TypedType::Float64,
                    Literal::String(_) => TypedType::String,
                    Literal::Char(_) => TypedType::Char,
                    Literal::Bool(_) => TypedType::Boolean,
                    Literal::Unit => TypedType::Unit,
                };

                if &lit_type != expected_type {
                    return Err(typed_type_mismatch(expected_type, &lit_type));
                }
                Ok(())
            }
            Pattern::Record(name, fields) => {
                if matches!(
                    expected_type,
                    TypedType::Record { .. } | TypedType::Temporal { .. }
                ) {
                    let (record_name, instantiated_fields) =
                        self.instantiated_record_fields(expected_type)?;
                    if name != &record_name {
                        return Err(TypeError::TypeMismatch {
                            expected: record_name.clone(),
                            found: name.clone(),
                        });
                    }

                    for (field_name, field_pattern) in fields {
                        let field_type = instantiated_fields.get(field_name).ok_or_else(|| {
                            TypeError::UnknownField {
                                record: name.clone(),
                                field: field_name.clone(),
                            }
                        })?;

                        self.check_pattern(field_pattern, field_type)?;
                    }
                    Ok(())
                } else {
                    Err(expected_type_mismatch("record type", expected_type))
                }
            }
            Pattern::Some(inner_pattern) => {
                if let TypedType::Option(inner_type) = expected_type {
                    self.check_pattern(inner_pattern, inner_type)
                } else {
                    Err(expected_type_mismatch("Option type", expected_type))
                }
            }
            Pattern::None => {
                if matches!(expected_type, TypedType::Option(_)) {
                    Ok(())
                } else {
                    Err(expected_type_mismatch("Option type", expected_type))
                }
            }
            Pattern::Ok(inner_pattern) => {
                if let TypedType::Result(ok_type, _) = expected_type {
                    self.check_pattern(inner_pattern, ok_type)
                } else {
                    Err(expected_type_mismatch("Result type", expected_type))
                }
            }
            Pattern::Err(inner_pattern) => {
                if let TypedType::Result(_, err_type) = expected_type {
                    self.check_pattern(inner_pattern, err_type)
                } else {
                    Err(expected_type_mismatch("Result type", expected_type))
                }
            }
            Pattern::EnumVariant {
                enum_name,
                variant_name,
                payload,
            } => {
                let TypedType::Enum {
                    name: expected_enum,
                } = expected_type
                else {
                    return Err(expected_type_mismatch(
                        format!("enum type '{}'", enum_name),
                        expected_type,
                    ));
                };
                if expected_enum != enum_name {
                    return Err(TypeError::TypeMismatch {
                        expected: expected_enum.clone(),
                        found: enum_name.clone(),
                    });
                }

                let path = VariantPath {
                    enum_name: enum_name.clone(),
                    variant_name: variant_name.clone(),
                };
                let declared_payload = self.enum_variant_payload(&path)?;
                match (declared_payload.as_ref(), payload.as_deref()) {
                    (None, None) => Ok(()),
                    (Some(payload_ty), Some(payload_pattern)) => {
                        self.check_pattern(payload_pattern, payload_ty)
                    }
                    (None, Some(_)) => Err(TypeError::ArityMismatch {
                        expected: 0,
                        found: 1,
                    }),
                    (Some(_), None) => Err(TypeError::ArityMismatch {
                        expected: 1,
                        found: 0,
                    }),
                }
            }
            Pattern::EmptyList => {
                if matches!(expected_type, TypedType::List(_)) {
                    Ok(())
                } else {
                    Err(expected_type_mismatch("List type", expected_type))
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
                    Err(expected_type_mismatch("List type", expected_type))
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
                    Err(expected_type_mismatch("List type", expected_type))
                }
            }
            Pattern::RecordDestruct {
                type_name,
                fields,
                rest: _,
            } => {
                // Record destructuring with spread syntax
                if matches!(
                    expected_type,
                    TypedType::Record { .. } | TypedType::Temporal { .. }
                ) {
                    let (record_name, instantiated_fields) =
                        self.instantiated_record_fields(expected_type)?;
                    if type_name != &record_name {
                        return Err(TypeError::TypeMismatch {
                            expected: record_name.clone(),
                            found: type_name.clone(),
                        });
                    }

                    for (field_name, field_pattern) in fields {
                        let field_type = instantiated_fields.get(field_name).ok_or_else(|| {
                            TypeError::UnknownField {
                                record: type_name.clone(),
                                field: field_name.clone(),
                            }
                        })?;

                        self.check_pattern(field_pattern, field_type)?;
                    }
                    Ok(())
                } else {
                    Err(expected_type_mismatch("record type", expected_type))
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
                if matches!(ty, TypedType::Record { .. } | TypedType::Temporal { .. }) {
                    let (record_name, instantiated_fields) = self.instantiated_record_fields(ty)?;
                    let field_types: Vec<(String, TypedType)> = fields
                        .iter()
                        .map(|(field_name, _)| {
                            let field_type =
                                instantiated_fields.get(field_name).ok_or_else(|| {
                                    TypeError::UnknownField {
                                        record: record_name.clone(),
                                        field: field_name.clone(),
                                    }
                                })?;
                            Ok((field_name.clone(), field_type.clone()))
                        })
                        .collect::<Result<Vec<_>, TypeError>>()?;

                    for ((_, field_pattern), (_, field_type)) in
                        fields.iter().zip(field_types.iter())
                    {
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
            Pattern::Ok(inner_pattern) => {
                if let TypedType::Result(ok_type, _) = ty {
                    self.bind_pattern_vars(inner_pattern, ok_type)
                } else {
                    Ok(())
                }
            }
            Pattern::Err(inner_pattern) => {
                if let TypedType::Result(_, err_type) = ty {
                    self.bind_pattern_vars(inner_pattern, err_type)
                } else {
                    Ok(())
                }
            }
            Pattern::EnumVariant {
                enum_name,
                variant_name,
                payload,
            } => {
                let TypedType::Enum { name } = ty else {
                    return Ok(());
                };
                if name != enum_name {
                    return Ok(());
                }
                let path = VariantPath {
                    enum_name: enum_name.clone(),
                    variant_name: variant_name.clone(),
                };
                if let (Some(payload_ty), Some(payload_pattern)) =
                    (self.enum_variant_payload(&path)?, payload.as_deref())
                {
                    self.bind_pattern_vars(payload_pattern, &payload_ty)?;
                }
                Ok(())
            }
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
            Pattern::RecordDestruct {
                type_name,
                fields,
                rest,
            } => {
                // Record destructuring with spread
                if matches!(ty, TypedType::Record { .. } | TypedType::Temporal { .. }) {
                    let (name, instantiated_fields) = self.instantiated_record_fields(ty)?;
                    if type_name != &name {
                        return Err(TypeError::TypeMismatch {
                            expected: name.clone(),
                            found: type_name.clone(),
                        });
                    }

                    let field_types: Vec<(String, TypedType)> = fields
                        .iter()
                        .map(|(field_name, _)| {
                            let field_type =
                                instantiated_fields.get(field_name).ok_or_else(|| {
                                    TypeError::UnknownField {
                                        record: name.clone(),
                                        field: field_name.clone(),
                                    }
                                })?;
                            Ok((field_name.clone(), field_type.clone()))
                        })
                        .collect::<Result<Vec<_>, TypeError>>()?;

                    // Bind each extracted field pattern
                    for ((_, field_pattern), (_, field_type)) in
                        fields.iter().zip(field_types.iter())
                    {
                        self.bind_pattern_vars(field_pattern, field_type)?;
                    }

                    // Bind rest variable if present
                    if let Some(rest_name) = rest {
                        if rest_name != "_" {
                            let rest_type =
                                self.ensure_residual_record_type(type_name, fields, ty)?;
                            self.bind_var(rest_name.clone(), rest_type, false)?;
                        }
                    }
                }
                Ok(())
            }
        }
    }

    fn is_pattern_exhaustive(&self, arms: &[MatchArm], ty: &TypedType) -> bool {
        // Check for wildcard or identifier patterns first
        let has_catch_all = arms
            .iter()
            .any(|arm| matches!(arm.pattern, Pattern::Wildcard | Pattern::Ident(_)));

        if has_catch_all {
            return true;
        }

        self.check_exhaustiveness_coverage(arms, ty).is_ok()
    }

    /// Advanced exhaustiveness checking using pattern space analysis
    fn check_exhaustiveness_coverage(
        &self,
        arms: &[MatchArm],
        ty: &TypedType,
    ) -> Result<(), Vec<String>> {
        // Build the pattern matrix from all arms
        let pattern_matrix: Vec<&Pattern> = arms.iter().map(|arm| &arm.pattern).collect();

        // Check if the pattern matrix covers the entire type space
        let uncovered = self.find_uncovered_patterns(&pattern_matrix, ty);

        if uncovered.is_empty() {
            Ok(())
        } else {
            Err(uncovered)
        }
    }

    /// Find patterns that are not covered by the given pattern matrix
    fn find_uncovered_patterns(&self, patterns: &[&Pattern], ty: &TypedType) -> Vec<String> {
        if patterns
            .iter()
            .any(|pattern| matches!(pattern, Pattern::Wildcard | Pattern::Ident(_)))
        {
            return Vec::new();
        }

        match ty {
            TypedType::Boolean => self.find_uncovered_boolean_patterns(patterns),
            TypedType::Option(inner_ty) => self.find_uncovered_option_patterns(patterns, inner_ty),
            TypedType::Result(ok_ty, err_ty) => {
                self.find_uncovered_result_patterns(patterns, ok_ty, err_ty)
            }
            TypedType::Enum { name } => self.find_uncovered_enum_patterns(patterns, name),
            TypedType::Unit => self.find_uncovered_unit_patterns(patterns),
            TypedType::List(elem_ty) => self.find_uncovered_list_patterns(patterns, elem_ty),
            TypedType::Record { name, .. } => self.find_uncovered_record_patterns(patterns, name),
            TypedType::Int32
            | TypedType::Int64
            | TypedType::Float64
            | TypedType::String
            | TypedType::Char => {
                // Infinite types - always require wildcard unless all possible values are covered
                self.find_uncovered_infinite_patterns(patterns, ty)
            }
            _ => {
                // For other types, conservatively require wildcard
                vec!["_ (wildcard pattern required for this type)".to_string()]
            }
        }
    }

    fn find_uncovered_boolean_patterns(&self, patterns: &[&Pattern]) -> Vec<String> {
        let mut uncovered = Vec::new();

        let has_true = patterns
            .iter()
            .any(|p| matches!(p, Pattern::Literal(Literal::Bool(true))));
        let has_false = patterns
            .iter()
            .any(|p| matches!(p, Pattern::Literal(Literal::Bool(false))));

        if !has_true {
            uncovered.push("true".to_string());
        }
        if !has_false {
            uncovered.push("false".to_string());
        }

        uncovered
    }

    fn find_uncovered_option_patterns(
        &self,
        patterns: &[&Pattern],
        inner_ty: &TypedType,
    ) -> Vec<String> {
        let mut uncovered = Vec::new();

        // Find all Some patterns and extract their inner patterns
        let some_patterns: Vec<&Pattern> = patterns
            .iter()
            .filter_map(|p| {
                if let Pattern::Some(inner) = p {
                    Some(inner.as_ref())
                } else {
                    None
                }
            })
            .collect();

        let has_none = patterns.iter().any(|p| matches!(p, Pattern::None));

        // Check if Some patterns are exhaustive for the inner type
        if some_patterns.is_empty() {
            uncovered.push("Some(_)".to_string());
        } else {
            // Recursively check if the inner patterns are exhaustive
            let inner_uncovered = self.find_uncovered_patterns(&some_patterns, inner_ty);
            if !inner_uncovered.is_empty() {
                // If inner patterns aren't exhaustive, we need more Some patterns
                for inner_pattern in inner_uncovered {
                    uncovered.push(format!("Some({})", inner_pattern));
                }
            }
        }

        if !has_none {
            uncovered.push("None".to_string());
        }

        uncovered
    }

    fn find_uncovered_result_patterns(
        &self,
        patterns: &[&Pattern],
        ok_ty: &TypedType,
        err_ty: &TypedType,
    ) -> Vec<String> {
        let mut uncovered = Vec::new();

        let ok_patterns: Vec<&Pattern> = patterns
            .iter()
            .filter_map(|p| {
                if let Pattern::Ok(inner) = p {
                    Some(inner.as_ref())
                } else {
                    None
                }
            })
            .collect();
        let err_patterns: Vec<&Pattern> = patterns
            .iter()
            .filter_map(|p| {
                if let Pattern::Err(inner) = p {
                    Some(inner.as_ref())
                } else {
                    None
                }
            })
            .collect();

        if ok_patterns.is_empty() {
            uncovered.push("Ok(_)".to_string());
        } else {
            for ok_pattern in self.find_uncovered_patterns(&ok_patterns, ok_ty) {
                uncovered.push(format!("Ok({})", ok_pattern));
            }
        }

        if err_patterns.is_empty() {
            uncovered.push("Err(_)".to_string());
        } else {
            for err_pattern in self.find_uncovered_patterns(&err_patterns, err_ty) {
                uncovered.push(format!("Err({})", err_pattern));
            }
        }

        uncovered
    }

    fn find_uncovered_enum_patterns(&self, patterns: &[&Pattern], enum_name: &str) -> Vec<String> {
        let Some(enum_def) = self.enums.get(enum_name) else {
            return vec![format!("_ (unknown enum type '{}')", enum_name)];
        };
        let mut uncovered = Vec::new();

        for variant_name in &enum_def.variant_order {
            let declared_payload = enum_def
                .variants
                .get(variant_name)
                .expect("enum variant order and payload table stay synchronized");
            let matching_payload_patterns = patterns
                .iter()
                .filter_map(|pattern| match pattern {
                    Pattern::EnumVariant {
                        enum_name: pattern_enum,
                        variant_name: pattern_variant,
                        payload,
                    } if pattern_enum == enum_name && pattern_variant == variant_name => {
                        Some(payload.as_deref())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();

            match declared_payload {
                None => {
                    if matching_payload_patterns.is_empty() {
                        uncovered.push(format!("{}::{}", enum_name, variant_name));
                    }
                }
                Some(payload_ty) => {
                    let nested_patterns = matching_payload_patterns
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>();
                    if nested_patterns.is_empty() {
                        uncovered.push(format!("{}::{}(_)", enum_name, variant_name));
                    } else {
                        for missing in self.find_uncovered_patterns(&nested_patterns, payload_ty) {
                            uncovered.push(format!("{}::{}({})", enum_name, variant_name, missing));
                        }
                    }
                }
            }
        }

        uncovered
    }

    fn find_uncovered_unit_patterns(&self, patterns: &[&Pattern]) -> Vec<String> {
        let has_unit = patterns
            .iter()
            .any(|p| matches!(p, Pattern::Literal(Literal::Unit)));

        if has_unit {
            Vec::new()
        } else {
            vec!["()".to_string()]
        }
    }

    fn find_uncovered_list_patterns(
        &self,
        patterns: &[&Pattern],
        elem_ty: &TypedType,
    ) -> Vec<String> {
        let mut uncovered = Vec::new();

        // Analyze what kinds of list patterns we have
        let has_empty = patterns.iter().any(|p| matches!(p, Pattern::EmptyList));
        let has_cons = patterns
            .iter()
            .any(|p| matches!(p, Pattern::ListCons(_, _)));

        // Collect all exact list pattern lengths
        let mut exact_lengths = HashSet::new();
        for pattern in patterns {
            if let Pattern::ListExact(elems) = pattern {
                exact_lengths.insert(elems.len());
            }
        }

        // Check coverage for empty list
        if !has_empty && !exact_lengths.contains(&0) {
            uncovered.push("[]".to_string());
        }

        // For non-empty lists, analyze cons patterns more carefully
        if has_cons {
            // Extract head patterns from all cons patterns
            let head_patterns: Vec<&Pattern> = patterns
                .iter()
                .filter_map(|p| {
                    if let Pattern::ListCons(head, _) = p {
                        Some(head.as_ref())
                    } else {
                        None
                    }
                })
                .collect();

            // Check if head patterns are exhaustive for the element type
            if !head_patterns.is_empty() {
                let head_uncovered = self.find_uncovered_patterns(&head_patterns, elem_ty);
                if !head_uncovered.is_empty() {
                    // If head patterns aren't exhaustive, we need more cons patterns
                    for head_pattern in head_uncovered {
                        uncovered.push(format!("[{}|_]", head_pattern));
                    }
                }
            }
            // Note: We don't recursively check tail patterns as they're the same list type
            // and would lead to infinite recursion. Tail exhaustiveness is assumed if
            // we have proper cons patterns.
        } else {
            // If we don't have cons patterns, check if exact patterns cover enough cases
            let has_any_non_empty =
                !exact_lengths.is_empty() && exact_lengths.iter().any(|&len| len > 0);

            if !has_any_non_empty {
                // No non-empty patterns at all
                uncovered.push("[_|_]".to_string());
            } else {
                // We have some exact patterns, but without cons patterns,
                // we can't prove exhaustiveness for arbitrary-length lists
                uncovered
                    .push("[_|_] (cons pattern needed for exhaustive list matching)".to_string());
            }
        }

        uncovered
    }

    fn find_uncovered_record_patterns(
        &self,
        patterns: &[&Pattern],
        record_name: &str,
    ) -> Vec<String> {
        let Some(record_def) = self.records.get(record_name) else {
            // If record doesn't exist, this is a different error
            return Vec::new();
        };

        // Check if we have any record patterns for this type
        let record_patterns: Vec<&Vec<(String, Pattern)>> = patterns
            .iter()
            .filter_map(|p| match p {
                Pattern::Record(name, fields) if name == record_name => Some(fields),
                Pattern::RecordDestruct {
                    type_name, fields, ..
                } if type_name == record_name => Some(fields),
                _ => None,
            })
            .collect();

        if record_patterns.is_empty() {
            return vec![format!("{}{{ .. }}", record_name)];
        }

        if record_patterns
            .iter()
            .any(|fields| fields.iter().all(|(_, p)| self.is_irrefutable_pattern(p)))
        {
            return Vec::new();
        }

        for (field_name, field_ty) in &record_def.fields {
            let field_patterns: Vec<&Pattern> = record_patterns
                .iter()
                .filter_map(|fields| {
                    if self.record_pattern_other_fields_irrefutable(fields, field_name) {
                        self.pattern_for_record_field(fields, field_name)
                    } else {
                        None
                    }
                })
                .collect();

            if !field_patterns.is_empty()
                && self
                    .find_uncovered_patterns(&field_patterns, field_ty)
                    .is_empty()
            {
                return Vec::new();
            }
        }

        vec![format!("{}{{ .. }}", record_name)]
    }

    fn is_irrefutable_pattern(&self, pattern: &Pattern) -> bool {
        match pattern {
            Pattern::Wildcard | Pattern::Ident(_) | Pattern::Literal(Literal::Unit) => true,
            Pattern::Record(_, fields) => fields
                .iter()
                .all(|(_, field_pattern)| self.is_irrefutable_pattern(field_pattern)),
            Pattern::RecordDestruct { fields, .. } => fields
                .iter()
                .all(|(_, field_pattern)| self.is_irrefutable_pattern(field_pattern)),
            Pattern::Literal(_)
            | Pattern::Some(_)
            | Pattern::None
            | Pattern::Ok(_)
            | Pattern::Err(_)
            | Pattern::EnumVariant { .. }
            | Pattern::EmptyList
            | Pattern::ListCons(_, _)
            | Pattern::ListExact(_) => false,
        }
    }

    fn pattern_for_record_field<'a>(
        &self,
        fields: &'a [(String, Pattern)],
        field_name: &str,
    ) -> Option<&'a Pattern> {
        fields
            .iter()
            .find_map(|(name, pattern)| (name == field_name).then_some(pattern))
    }

    fn record_pattern_other_fields_irrefutable(
        &self,
        fields: &[(String, Pattern)],
        field_name: &str,
    ) -> bool {
        fields
            .iter()
            .filter(|(name, _)| name != field_name)
            .all(|(_, pattern)| self.is_irrefutable_pattern(pattern))
    }

    fn find_uncovered_infinite_patterns(
        &self,
        _patterns: &[&Pattern],
        ty: &TypedType,
    ) -> Vec<String> {
        // For infinite types like Int32, String, etc., we can't enumerate all possibilities
        // so we always require a wildcard unless the user has explicitly covered
        // a reasonable set of cases (which we don't check for now)
        vec![format!(
            "_ (pattern required for infinite type {})",
            format_typed_type(ty)
        )]
    }

    fn check_list_lit(
        &mut self,
        elements: &[Box<Expr>],
        expected: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        if elements.is_empty() {
            // Empty list - infer from expected type if available
            if let Some(TypedType::List(elem_type)) = expected {
                return Ok(TypedType::List(elem_type.clone()));
            } else if matches!(expected, Some(TypedType::InferVar(_))) {
                return Ok(TypedType::List(Box::new(
                    self.type_var_generator.fresh_var(),
                )));
            } else {
                return Err(TypeError::CannotInferType(
                    "empty list requires an expected List type".to_string(),
                ));
            }
        }

        let expected_elem = match expected {
            Some(TypedType::List(elem_type)) => Some(elem_type.as_ref()),
            _ => None,
        };
        let elem_type = self.check_collection_elements(elements, expected_elem, "list")?;

        Ok(TypedType::List(Box::new(elem_type)))
    }

    fn check_range_lit(
        &mut self,
        range: &RangeLit,
        expected: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        let range_type = Self::range_int32_type();

        match expected {
            Some(TypedType::Record {
                name, type_args, ..
            }) if name == "Range" => {
                if !matches!(type_args.as_slice(), [TypedType::Int32]) {
                    return Err(TypeError::UnsupportedFeature(
                        "range literals currently support Range<Int32> only".to_string(),
                    ));
                }
            }
            Some(TypedType::InferVar(_)) | None => {}
            Some(expected_ty) => {
                return Err(TypeError::TypeMismatch {
                    expected: format_typed_type(expected_ty),
                    found: format_typed_type(&range_type),
                });
            }
        }

        let start_type = self.check_expr_with_expected(&range.start, Some(&TypedType::Int32))?;
        if !self.type_matches_expected(&TypedType::Int32, &start_type) {
            return Err(TypeError::TypeMismatch {
                expected: "Int32 range start".to_string(),
                found: format_typed_type(&start_type),
            });
        }

        let end_type = self.check_expr_with_expected(&range.end, Some(&TypedType::Int32))?;
        if !self.type_matches_expected(&TypedType::Int32, &end_type) {
            return Err(TypeError::TypeMismatch {
                expected: "Int32 range end".to_string(),
                found: format_typed_type(&end_type),
            });
        }

        Ok(range_type)
    }

    fn check_array_lit(
        &mut self,
        elements: &[Box<Expr>],
        expected: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        if elements.is_empty() {
            // Empty array - infer from expected type if available
            if let Some(TypedType::Array(elem_type, _)) = expected {
                return Ok(TypedType::Array(elem_type.clone(), ArrayLength::Known(0)));
            } else {
                return Err(TypeError::CannotInferType(
                    "empty array requires an expected Array type".to_string(),
                ));
            }
        }

        let expected_elem = match expected {
            Some(TypedType::Array(elem_type, _)) => Some(elem_type.as_ref()),
            _ => None,
        };
        let elem_type = self.check_collection_elements(elements, expected_elem, "array")?;
        let size = elements.len();
        Ok(TypedType::Array(
            Box::new(elem_type),
            ArrayLength::Known(size),
        ))
    }

    fn check_collection_elements(
        &mut self,
        elements: &[Box<Expr>],
        expected_elem: Option<&TypedType>,
        collection_name: &str,
    ) -> Result<TypedType, TypeError> {
        let element_type = expected_elem
            .cloned()
            .unwrap_or_else(|| self.type_var_generator.fresh_var());
        let mut substitution = ConstraintSubstitution::new();
        let mut constraints = Vec::new();

        for (index, element) in elements.iter().enumerate() {
            let expected_for_element = substitution.apply(&element_type)?;
            let actual_type =
                self.check_expr_with_expected(element, Some(&expected_for_element))?;
            self.solve_type_constraint(
                &mut constraints,
                &mut substitution,
                expected_for_element,
                actual_type,
                Self::constraint_origin(ConstraintKind::Argument {
                    func_name: format!("{} literal", collection_name),
                    arg_index: index,
                }),
            )?;
        }

        finalize_type(&element_type, &substitution)
    }

    fn check_lambda_expr(
        &mut self,
        lambda: &LambdaExpr,
        expected: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        if lambda.implicit_focus && self.implicit_focus_depth > 0 {
            return Err(TypeError::UnsupportedFeature(
                "nested implicit focus scopes are ambiguous; name at least one scope binder with `{ |value| ... }`"
                    .to_string(),
            ));
        }

        if lambda.implicit_focus {
            self.implicit_focus_depth += 1;
        }
        let result = self.check_lambda_expr_inner(lambda, expected);
        if lambda.implicit_focus {
            self.implicit_focus_depth -= 1;
        }
        result
    }

    fn check_lambda_expr_inner(
        &mut self,
        lambda: &LambdaExpr,
        expected: Option<&TypedType>,
    ) -> Result<TypedType, TypeError> {
        // Collect free variables before creating lambda scope
        let bound_vars = HashSet::new();
        let free_vars = self.collect_free_variables(&lambda.body, &bound_vars);

        // Get current temporal context to determine allowed temporals
        let allowed_temporals = self.temporal_context.active_temporals.clone();

        // Check if any free variables have temporal types that would escape
        for var_name in &free_vars {
            if let Some(var_type) = self.peek_var_type(var_name) {
                // Check if this type contains temporals that would escape without
                // consuming affine captures during the pre-check.
                self.check_temporal_escape(&var_type, &allowed_temporals)?;
            }
        }

        // Create a new scope for lambda parameters
        self.push_scope();

        let mut param_types = Vec::new();
        let mut substitution = ConstraintSubstitution::new();
        let shaped_expected = match expected {
            Some(TypedType::InferVar(_)) => {
                Some(self.fresh_lambda_function_type(lambda.params.len()))
            }
            other => other.cloned(),
        };
        let expected_return_type = if let Some(TypedType::Function {
            params,
            return_type,
        }) = shaped_expected.as_ref()
        {
            // Use expected parameter types if available
            if lambda.implicit_focus && params.len() != 1 {
                self.pop_scope();
                return Err(TypeError::UnsupportedFeature(format!(
                    "implicit focus scopes require a unary function parameter, but this callable expects {} binders; name them with `{{ |left, right| ... }}`",
                    params.len()
                )));
            }
            if params.len() != lambda.params.len() {
                self.pop_scope();
                return Err(TypeError::ArityMismatch {
                    expected: params.len(),
                    found: lambda.params.len(),
                });
            }

            for (i, param) in lambda.params.iter().enumerate() {
                let param_type =
                    self.resolve_lambda_param_type(param, &params[i], &mut substitution)?;
                param_types.push(param_type.clone());
                self.bind_var(param.name.clone(), param_type, false)?;
            }

            Some(return_type.as_ref())
        } else if shaped_expected.is_none() {
            let mut annotated_param_types = Vec::with_capacity(lambda.params.len());
            for param in &lambda.params {
                let Some(type_annotation) = &param.type_annotation else {
                    self.pop_scope();
                    return Err(TypeError::CannotInferType(
                        "lambda parameter types require annotations or an expected function type"
                            .to_string(),
                    ));
                };
                let param_type = self.convert_type(type_annotation)?;
                annotated_param_types.push(param_type.clone());
                self.bind_var(param.name.clone(), param_type, false)?;
            }
            param_types = annotated_param_types;
            None
        } else if let Some(other) = shaped_expected.as_ref() {
            self.pop_scope();
            return Err(expected_type_mismatch("function", other));
        } else {
            unreachable!("expected lambda context should be handled above")
        };

        // Type check the body with inferred parameter types
        let body_result = self.check_expr_with_expected(&lambda.body, expected_return_type);
        let observed_param_types = lambda
            .params
            .iter()
            .map(|param| self.peek_var_type(&param.name))
            .collect::<Option<Vec<_>>>()
            .unwrap_or_else(|| param_types.clone());

        // Pop the lambda scope before reporting any body error.
        self.pop_scope();

        let body_type = body_result?;
        for (param_type, observed_type) in param_types.iter().zip(observed_param_types.iter()) {
            unify_constraint(param_type, observed_type, &mut substitution)?;
        }

        let return_type = if let Some(expected_ret) = expected_return_type {
            unify_constraint(expected_ret, &body_type, &mut substitution)?;
            if Self::contains_inference_internal_type(expected_ret) {
                substitution.apply(expected_ret)?
            } else {
                expected_ret.clone()
            }
        } else {
            body_type
        };

        let param_types = param_types
            .iter()
            .map(|param_type| substitution.apply(param_type))
            .collect::<Result<Vec<_>, _>>()?;

        // Create the function type
        let func_type = TypedType::Function {
            params: param_types,
            return_type: Box::new(return_type),
        };

        // Check if the function type itself contains escaping temporals
        self.check_temporal_escape(&func_type, &allowed_temporals)?;

        Ok(func_type)
    }
}

// Standalone type_check function for public API
pub fn type_check(program: &Program) -> Result<(), TypeError> {
    let mut checker = TypeChecker::new();
    checker.check_program(program)
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::parser::parse_program;

    fn check_program_str(input: &str) -> Result<(), TypeError> {
        let (_, program) = parse_program(input).unwrap();
        let mut checker = TypeChecker::new();
        checker.check_program(&program)
    }

    #[test]
    fn deferred_lambda_resolution_does_not_overwrite_source_facts() {
        // Deferred use-site re-checks happen on stored clones. With ids
        // stripped from the clones, branch specialization must not record
        // instantiation-specific facts under the source lambda body's node ids.
        let source = r#"
fun main: () -> Int32 = {
    mut val emit = |x| [];
    val r = true then {
        val a: Int32 -> List<Int32> = emit;
        val xs: List<Int32> = 1 |> a;
        0
    } else {
        0
    };
    val b: Int32 -> List<Int32> = emit;
    42
}
"#;
        let (_, program) = parse_program(source).unwrap();
        let mut checker = TypeChecker::new();
        checker
            .check_program(&program)
            .expect("later uses at the merged concrete specialization are accepted");

        let TopDecl::Function(func) = &program.declarations[0] else {
            panic!("first declaration should be main");
        };
        let Stmt::Binding(binding) = &func.body.statements[0] else {
            panic!("first statement should bind the lambda");
        };
        let ExprKind::Lambda(lambda) = &binding.value.kind else {
            panic!("binding value should be a lambda");
        };

        assert_eq!(
            checker.checked_expr_type(&lambda.body),
            None,
            "use-site instantiations must not write facts for source lambda body nodes"
        );
    }

    #[test]
    fn branch_merge_rechecks_non_generic_deferred_before_clearing() {
        let source = r#"
fun main: () -> () = {
    val mapper = |value| value + 1;
    ()
}
"#;
        let (_, program) = parse_program(source).unwrap();
        let TopDecl::Function(func) = &program.declarations[0] else {
            panic!("first declaration should be main");
        };
        let Stmt::Binding(binding) = &func.body.statements[0] else {
            panic!("first statement should bind the lambda");
        };
        let ExprKind::Lambda(lambda) = &binding.value.kind else {
            panic!("binding value should be a lambda");
        };

        let mut checker = TypeChecker::new();
        let placeholder = checker.fresh_lambda_function_type(1);
        let alias_group_id = checker.fresh_deferred_alias_group_id();
        checker
            .bind_var_with_deferred(
                "mapper".to_string(),
                placeholder,
                false,
                Some(DeferredBinding::Lambda(lambda.clone())),
                Some(alias_group_id),
            )
            .unwrap();
        let base_env = checker.var_env.clone();
        let merged_ty = TypedType::Function {
            params: vec![TypedType::String],
            return_type: Box::new(TypedType::String),
        };
        let mut unchecked_branch = base_env.clone();
        let mapper = unchecked_branch
            .last_mut()
            .and_then(|scope| scope.get_mut("mapper"))
            .unwrap();
        mapper.ty = merged_ty;
        // A transfer clears deferred ownership without checking the lambda
        // body. Branch merge must not treat this marker alone as evidence.
        mapper.deferred = None;
        mapper.used = true;

        let err = checker
            .merge_branch_var_usage(base_env.clone(), &[unchecked_branch, base_env])
            .expect_err("the merged String signature must recheck the stored lambda body");
        assert!(matches!(err, TypeError::TypeMismatch { .. }));
    }

    #[test]
    fn branch_merge_validates_concrete_deferred_generic_before_clearing() {
        let mut checker = TypeChecker::new();
        let definition = checker.functions.get("map").unwrap().clone();
        let invalid_map_type = TypedType::Function {
            params: vec![TypedType::Int32],
            return_type: Box::new(TypedType::Int32),
        };
        let alias_group_id = checker.fresh_deferred_alias_group_id();
        checker
            .bind_var_with_deferred(
                "apply_map".to_string(),
                invalid_map_type,
                false,
                Some(DeferredBinding::GenericFunction {
                    name: "map".to_string(),
                    definition,
                }),
                Some(alias_group_id),
            )
            .unwrap();
        let base_env = checker.var_env.clone();

        let err = checker
            .merge_branch_var_usage(base_env.clone(), &[base_env.clone(), base_env])
            .expect_err("a concrete deferred generic must still match its stored definition");
        assert!(matches!(
            err,
            TypeError::ArityMismatch {
                expected: 2,
                found: 1
            }
        ));
    }

    fn test_record_type(name: &str) -> TypedType {
        TypedType::Record {
            name: name.to_string(),
            type_args: Vec::new(),
            frozen: false,
            hash: None,
            parent_hash: None,
        }
    }

    #[test]
    fn checked_record_fields_preserve_order_and_apply_type_args() {
        let (_, program) = parse_program(
            r#"
record Box<T> {
    label: String,
    value: T
}

fun main: (item: Box<Int64>) -> Box<Int64> = {
    item
}
"#,
        )
        .unwrap();
        let mut checker = TypeChecker::new();
        checker
            .check_program(&program)
            .expect("program should type-check");

        let fields = checker
            .checked_record_fields_for_type(&TypedType::Record {
                name: "Box".to_string(),
                type_args: vec![TypedType::Int64],
                frozen: false,
                hash: None,
                parent_hash: None,
            })
            .expect("checked record fields should be available");

        assert_eq!(
            fields,
            vec![
                ("label".to_string(), TypedType::String),
                ("value".to_string(), TypedType::Int64),
            ]
        );
    }

    #[test]
    fn unresolved_inference_types_are_affine_conservative() {
        let checker = TypeChecker::new();

        assert!(!checker.is_copyable(&TypedType::InferVar(TypeVarId(0))));
        assert!(
            !checker.is_copyable(&TypedType::Option(Box::new(TypedType::InferVar(
                TypeVarId(1)
            ))))
        );
        assert!(!checker.is_copyable(&TypedType::Projection {
            base: Box::new(TypedType::InferVar(TypeVarId(2))),
            form_name: "Container".to_string(),
            assoc_name: "Item".to_string(),
            args: vec![TypedType::Int32],
        }));
    }

    #[test]
    fn checker_owned_form_environment_controls_container_projection() {
        let input = r#"
            fun main: () -> Option<String> = {
                val maybe: Option<Int32> = (1) Option::Some;
                (maybe, |value| "x") map
            }
        "#;
        let (_, program) = parse_program(input).unwrap();

        let mut checker = TypeChecker::new();
        checker
            .check_program(&program)
            .expect("standard form environment should solve Container projection");

        let mut checker_without_forms = TypeChecker::new();
        checker_without_forms.form_environment = FormEnvironment::new();
        let err = checker_without_forms
            .check_program(&program)
            .expect_err("missing Container adoption should reject map projection");
        let message = err.to_string();
        assert!(
            message.contains("Container") || message.contains("associated type"),
            "error should mention missing Container/projection support, got: {message}"
        );
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
    fn test_simple_copy_semantics() {
        // Test that basic copyable types can be used multiple times
        let input = r#"
            val x = 42
            val y = x
            val z = x
        "#;
        assert!(check_program_str(input).is_ok());
    }

    #[test]
    fn test_simple_affine_violations() {
        // Test that non-copyable types (like strings) trigger affine violations
        let input = r#"
            val s = "hello"
            val t = s
            val u = s
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::AffineViolation("s".to_string()))
        );
    }

    #[test]
    fn test_simple_affine_in_blocks() {
        // Test affine violations across block boundaries using basic syntax
        let input = r#"
            val s = "hello"
            val y = { val z = s }
            val w = s
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::AffineViolation("s".to_string()))
        );
    }

    #[test]
    fn test_comprehensive_type_system_fixes() {
        // Test that demonstrates all our fixes working together
        let input1 = r#"
            // Copyable types can be used multiple times
            val x = 42
            val y = x
            val z = x
        "#;
        assert!(check_program_str(input1).is_ok());

        // Non-copyable types trigger affine violations
        let input2 = r#"
            val s = "hello"
            val t = s
            val u = s
        "#;
        assert_eq!(
            check_program_str(input2),
            Err(TypeError::AffineViolation("s".to_string()))
        );

        // Affine violations work across block boundaries
        let input3 = r#"
            val s = "hello"
            val block_result = { val temp = s }
            val w = s
        "#;
        assert_eq!(
            check_program_str(input3),
            Err(TypeError::AffineViolation("s".to_string()))
        );
    }

    #[test]
    fn test_copy_semantics_primitives() {
        // Test that primitive types (Int32, Boolean, etc.) can be used multiple times
        let input = r#"
            val x = 42
            val y = x    // Should work - Int32 is copyable
            val z = x    // Should work - can use x again
        "#;
        assert!(check_program_str(input).is_ok());

        // Test factorial function works with copy semantics
        let factorial_input = r#"
            fun factorial: (n: Int32) -> Int32 = {
                n <= 1 then { 1 } else { n * (n - 1) factorial }  // Uses 'n' twice - should work with copy semantics
            }
        "#;
        assert!(check_program_str(factorial_input).is_ok());

        // Test boolean copy semantics
        let bool_input = r#"
            val flag = true
            val a = flag
            val b = flag  // Should work - Boolean is copyable
        "#;
        assert!(check_program_str(bool_input).is_ok());
    }

    #[test]
    fn test_affine_violation_for_strings() {
        // Test that String types still enforce affine constraints
        let input = r#"
            val s = "hello"
            val y = s   // Should work - first use
            val z = s   // Should fail - String is NOT copyable (affine)
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::AffineViolation("s".to_string()))
        );
    }

    #[test]
    fn test_affine_violation_for_records() {
        // Test that record types maintain affine constraints (not copyable by default)
        let input = r#"
            record Point { x: Int32, y: Int32 }
            val p = Point { x: 10, y: 20 }
            val a = p   // Should work - first use
            val b = p   // Should fail - Record is NOT copyable (affine)
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::AffineViolation("p".to_string()))
        );
    }

    #[test]
    fn test_copy_semantics_composite_types() {
        // Test basic copyable types (Int32 is copyable)
        let copyable_input = r#"
            val x = 42
            val a = x
            val b = x  // Should work - Int32 is copyable
        "#;
        assert!(check_program_str(copyable_input).is_ok());

        // Test records are NOT copyable by default (affine)
        let record_input = r#"
            record Point { x: Int32, y: Int32 }
            val p = Point { x: 10, y: 20 }
            val a = p
            val b = p  // Should fail - records are affine by default
        "#;
        assert_eq!(
            check_program_str(record_input),
            Err(TypeError::AffineViolation("p".to_string()))
        );
    }

    #[test]
    fn test_record_types() {
        let input = r#"
            record Point { x: Int32, y: Int32 }
            val p = Point { x: 10, y: 20 }
        "#;
        assert!(check_program_str(input).is_ok());
    }

    #[test]
    fn test_undefined_record() {
        let input = r#"
            val p = Point { x: 10, y: 20 }
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::UndefinedRecord("Point".to_string()))
        );
    }

    #[test]
    fn test_field_access() {
        let input = r#"
            record Point { x: Int32, y: Int32 }
            val p = Point { x: 10, y: 20 }
            val x = p.x
        "#;
        assert!(check_program_str(input).is_ok());
    }

    #[test]
    fn test_unknown_field() {
        let input = r#"
            record Point { x: Int32, y: Int32 }
            val p = Point { x: 10, y: 20 }
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
    fn test_missing_record_metadata_field_access_returns_error() {
        let checker = TypeChecker::new();

        assert_eq!(
            checker.record_field_type(&test_record_type("Ghost"), "x"),
            Err(TypeError::UndefinedRecord("Ghost".to_string()))
        );
    }

    #[test]
    fn test_missing_record_metadata_clone_returns_error() {
        let mut checker = TypeChecker::new();
        checker
            .bind_var("ghost".to_string(), test_record_type("Ghost"), false)
            .unwrap();

        let clone_expr = CloneExpr {
            base: Box::new(Expr::new(ExprKind::Ident("ghost".to_string()))),
            updates: RecordLit {
                name: String::new(),
                fields: vec![],
            },
        };

        assert_eq!(
            checker.check_clone_expr(&clone_expr),
            Err(TypeError::UndefinedRecord("Ghost".to_string()))
        );
    }

    #[test]
    fn test_pattern_binding_unknown_field_returns_error() {
        let mut checker = TypeChecker::new();
        checker.records.insert(
            "Point".to_string(),
            RecordDef {
                fields: HashMap::from([("x".to_string(), TypedType::Int32)]),
                field_order: vec!["x".to_string()],
                type_params: vec![],
                temporal_constraints: vec![],
                hash: None,
                parent_hash: None,
            },
        );

        let pattern = Pattern::Record(
            "Point".to_string(),
            vec![("z".to_string(), Pattern::Ident("z".to_string()))],
        );

        assert_eq!(
            checker.bind_pattern_vars(&pattern, &test_record_type("Point")),
            Err(TypeError::UnknownField {
                record: "Point".to_string(),
                field: "z".to_string()
            })
        );
    }

    #[test]
    fn test_clone_freeze() {
        let input = r#"
            record Point { x: Int32, y: Int32 }
            val p1 = Point { x: 10, y: 20 }
            val p2 = p1.clone { x: 30 }
            val p3 = p2 freeze
        "#;
        assert!(check_program_str(input).is_ok());
    }

    #[test]
    fn test_clone_frozen_error() {
        let input = r#"
            record Point { x: Int32, y: Int32 }
            val p1 = Point { x: 10, y: 20 } freeze
            val p2 = p1.clone { x: 30 }
        "#;
        assert_eq!(check_program_str(input), Err(TypeError::CloneFrozenRecord));
    }

    #[test]
    fn test_affine_field_access() {
        let input = r#"
            record Point { x: Int32, y: Int32 }
            val p = Point { x: 10, y: 20 }
            val x = p.x
            val y = p.y
        "#;
        assert!(check_program_str(input).is_ok());

        let non_copyable_input = r#"
            record User { id: Int32, name: String }
            val user = User { id: 1, name: "Ada" }
            val id = user.id
            val name = user.name
            val second_id = user.id
        "#;
        assert_eq!(
            check_program_str(non_copyable_input),
            Err(TypeError::AffineViolation("user".to_string()))
        );
    }

    #[test]
    fn test_affine_in_blocks() {
        // Test with affine record type - should fail
        let input = r#"
            record Point { x: Int32, y: Int32 }
            val p = Point { x: 10, y: 20 }
            val y = { val z = p }
            val w = p
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::AffineViolation("p".to_string()))
        );

        // Test with copyable integer - should succeed
        let copyable_input = r#"
            val x = 42
            val y = { val z = x }
            val w = x
        "#;
        assert!(check_program_str(copyable_input).is_ok());
    }

    #[test]
    fn test_function_params_affine() {
        let input = r#"
            record User { id: Int32, name: String }
            fun use_twice: (user: User) -> Int32 = {
                val id = user.id;
                val name = user.name;
                val second_id = user.id;
                second_id
            }
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::AffineViolation("user".to_string()))
        );
    }

    #[test]
    fn test_clone_field_type_mismatch() {
        let input = r#"
            record Point { x: Int32, y: Int32 }
            val p1 = Point { x: 10, y: 20 }
            val p2 = p1.clone { x: "hello" }
        "#;
        assert!(matches!(
            check_program_str(input),
            Err(TypeError::TypeMismatch { .. })
        ));
    }

    #[test]
    fn test_clone_unknown_field() {
        let input = r#"
            record Point { x: Int32, y: Int32 }
            val p1 = Point { x: 10, y: 20 }
            val p2 = p1.clone { z: 30 }
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
            fun add: (a: Int32, b: Int32) -> Int32 = { a }
            val result = (10, 20) add
        "#;
        assert!(check_program_str(input).is_ok());
    }

    #[test]
    fn test_function_arity_mismatch() {
        let input = r#"
            fun add: (a: Int32, b: Int32) -> Int32 = { a }
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
            fun inc: (x: Int32) -> Int32 = { x }
            val result = 42 |> inc
        "#;
        assert!(check_program_str(input).is_ok());
    }

    #[test]
    fn test_context_basic() {
        let input = "context DB { host: String, port: Int32 }

val result = with DB {
    val x = 42
    x
}";
        match parse_program(input) {
            Ok((_, program)) => {
                let mut checker = TypeChecker::new();
                match checker.check_program(&program) {
                    Ok(_) => {}
                    Err(e) => {
                        panic!("Type checking failed: {:?}", e);
                    }
                }
            }
            Err(e) => {
                panic!("Parsing failed: {:?}", e);
            }
        }
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
        // Test individual context declarations work
        let db_input = r#"
            context DB { host: String }

            val result = with DB {
                val x = 42
                x
            }
        "#;
        assert!(check_program_str(db_input).is_ok());

        // Test separate context works too
        let cache_input = r#"
            context Cache { size: Int32 }

            val result = with Cache {
                val y = 100
                y
            }
        "#;
        assert!(check_program_str(cache_input).is_ok());
    }
}

impl TypeChecker {
    // Prototype + Derivation-Bound implementation
    fn check_derivation_bound(
        &self,
        concrete_type: &TypedType,
        required_parent: &str,
    ) -> Result<(), TypeError> {
        match concrete_type {
            TypedType::Record {
                name,
                hash,
                parent_hash,
                ..
            } => {
                // Check if this record derives from the required parent
                if self.is_derived_from(
                    name,
                    hash.as_ref(),
                    parent_hash.as_ref(),
                    required_parent,
                )? {
                    Ok(())
                } else {
                    Err(TypeError::NotDerivedFrom(
                        name.clone(),
                        required_parent.to_string(),
                    ))
                }
            }
            _ => {
                // Non-record types cannot have derivation bounds
                Err(TypeError::NotDerivedFrom(
                    format_typed_type(concrete_type),
                    required_parent.to_string(),
                ))
            }
        }
    }

    fn is_derived_from(
        &self,
        type_name: &str,
        _current_hash: Option<&String>,
        parent_hash: Option<&String>,
        target_parent: &str,
    ) -> Result<bool, TypeError> {
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
                        return self.is_derived_from(
                            parent_name,
                            Some(parent_current_hash),
                            prototype_parent_hash.as_ref(),
                            target_parent,
                        );
                    }
                }
            }
        }

        // Also check using the hash/parent_hash from the type itself
        if let Some(parent_hash_val) = parent_hash {
            for (parent_name, (parent_current_hash, _, _)) in &self.prototypes {
                if parent_current_hash == parent_hash_val {
                    return self.is_derived_from(
                        parent_name,
                        Some(parent_current_hash),
                        None,
                        target_parent,
                    );
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

            if let Some((_, Some(parent_hash_val), _)) = self.prototypes.get(current_type) {
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
        }

        Ok(depth)
    }

    fn check_prototype_clone_expr(
        &mut self,
        proto_clone: &PrototypeCloneExpr,
    ) -> Result<TypedType, TypeError> {
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
            type_args: Vec::new(),
            frozen: proto_clone.freeze_immediately,
            hash: Some(new_hash.clone()),
            parent_hash,
        })
    }

    /// Collect free variables in an expression
    fn collect_free_variables(&self, expr: &Expr, bound_vars: &HashSet<String>) -> HashSet<String> {
        let mut free_vars = HashSet::new();

        match &expr.kind {
            ExprKind::Ident(name) => {
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
            ExprKind::Binary(bin) => {
                free_vars.extend(self.collect_free_variables(&bin.left, bound_vars));
                free_vars.extend(self.collect_free_variables(&bin.right, bound_vars));
            }
            ExprKind::Unary(unary) => {
                free_vars.extend(self.collect_free_variables(&unary.expr, bound_vars));
            }
            ExprKind::Cast(cast) => {
                free_vars.extend(self.collect_free_variables(&cast.expr, bound_vars));
            }
            ExprKind::Call(call) => {
                free_vars.extend(self.collect_free_variables(&call.function, bound_vars));
                for arg in &call.args {
                    free_vars.extend(self.collect_free_variables(arg, bound_vars));
                }
            }
            ExprKind::FieldAccess(object, _field) => {
                free_vars.extend(self.collect_free_variables(object, bound_vars));
            }
            ExprKind::RecordLit(record_lit) => {
                for field in &record_lit.fields {
                    match field {
                        FieldInit::Field { value, .. } => {
                            free_vars.extend(self.collect_free_variables(value, bound_vars));
                        }
                        FieldInit::Spread(expr) => {
                            free_vars.extend(self.collect_free_variables(expr, bound_vars));
                        }
                    }
                }
            }
            ExprKind::Clone(clone_expr) => {
                free_vars.extend(self.collect_free_variables(&clone_expr.base, bound_vars));
                for field in &clone_expr.updates.fields {
                    match field {
                        FieldInit::Field { value, .. } => {
                            free_vars.extend(self.collect_free_variables(value, bound_vars));
                        }
                        FieldInit::Spread(expr) => {
                            free_vars.extend(self.collect_free_variables(expr, bound_vars));
                        }
                    }
                }
            }
            ExprKind::Freeze(expr) => {
                free_vars.extend(self.collect_free_variables(expr, bound_vars));
            }
            ExprKind::PrototypeClone(proto_clone) => {
                // Base is just a name, not an expression, so no free vars from it
                for field in &proto_clone.updates.fields {
                    match field {
                        FieldInit::Field { value, .. } => {
                            free_vars.extend(self.collect_free_variables(value, bound_vars));
                        }
                        FieldInit::Spread(expr) => {
                            free_vars.extend(self.collect_free_variables(expr, bound_vars));
                        }
                    }
                }
            }
            ExprKind::ListLit(elements) => {
                for elem in elements {
                    free_vars.extend(self.collect_free_variables(elem, bound_vars));
                }
            }
            ExprKind::ArrayLit(elements) => {
                for elem in elements {
                    free_vars.extend(self.collect_free_variables(elem, bound_vars));
                }
            }
            ExprKind::RangeLit(range) => {
                free_vars.extend(self.collect_free_variables(&range.start, bound_vars));
                free_vars.extend(self.collect_free_variables(&range.end, bound_vars));
            }
            ExprKind::Match(match_expr) => {
                free_vars.extend(self.collect_free_variables(&match_expr.expr, bound_vars));
                for arm in &match_expr.arms {
                    // Pattern bindings create new bound variables
                    let mut arm_bound = bound_vars.clone();
                    self.collect_pattern_bindings(&arm.pattern, &mut arm_bound);
                    // The body is a BlockExpr, so we need to handle it specially
                    free_vars.extend(self.collect_free_variables_in_block(&arm.body, &arm_bound));
                }
            }
            ExprKind::Then(then_expr) => {
                free_vars.extend(self.collect_free_variables(&then_expr.condition, bound_vars));
                free_vars.extend(
                    self.collect_free_variables_in_block(&then_expr.then_block, bound_vars),
                );
                for (cond, block) in &then_expr.else_ifs {
                    free_vars.extend(self.collect_free_variables(cond, bound_vars));
                    free_vars.extend(self.collect_free_variables_in_block(block, bound_vars));
                }
                if let Some(else_block) = &then_expr.else_block {
                    free_vars.extend(self.collect_free_variables_in_block(else_block, bound_vars));
                }
            }
            ExprKind::While(while_expr) => {
                free_vars.extend(self.collect_free_variables(&while_expr.condition, bound_vars));
                free_vars
                    .extend(self.collect_free_variables_in_block(&while_expr.body, bound_vars));
            }
            ExprKind::Block(block) => {
                free_vars.extend(self.collect_free_variables_in_block(block, bound_vars));
            }
            ExprKind::Lambda(lambda) => {
                let mut lambda_bound = bound_vars.clone();
                for param in &lambda.params {
                    lambda_bound.insert(param.name.clone());
                }
                free_vars.extend(self.collect_free_variables(&lambda.body, &lambda_bound));
            }
            ExprKind::WithLifetime(wl) => {
                free_vars.extend(self.collect_free_variables_in_block(&wl.body, bound_vars));
            }
            ExprKind::With(with_expr) => {
                let mut body_bound = bound_vars.clone();
                for binding in &with_expr.bindings {
                    match binding {
                        FieldInit::Field { name, value } => {
                            free_vars.extend(self.collect_free_variables(value, bound_vars));
                            body_bound.insert(name.clone());
                        }
                        FieldInit::Spread(expr) => {
                            free_vars.extend(self.collect_free_variables(expr, bound_vars));
                        }
                    }
                }
                free_vars
                    .extend(self.collect_free_variables_in_block(&with_expr.body, &body_bound));
            }
            ExprKind::Pipe(pipe_expr) => {
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
            ExprKind::Await(expr) => {
                free_vars.extend(self.collect_free_variables(expr, bound_vars));
            }
            ExprKind::Spawn(expr) => {
                free_vars.extend(self.collect_free_variables(expr, bound_vars));
            }
            ExprKind::VariantRef(_) => {}
            // Literals have no free variables
            ExprKind::IntLit(_)
            | ExprKind::FloatLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::CharLit(_)
            | ExprKind::BoolLit(_)
            | ExprKind::Unit => {}
        }

        free_vars
    }

    /// Helper function to collect free variables in a BlockExpr
    fn collect_free_variables_in_block(
        &self,
        block: &BlockExpr,
        bound_vars: &HashSet<String>,
    ) -> HashSet<String> {
        let mut free_vars = HashSet::new();
        let mut block_bound = bound_vars.clone();

        for stmt in &block.statements {
            match stmt {
                Stmt::Binding(bind_decl) => {
                    free_vars.extend(self.collect_free_variables(&bind_decl.value, &block_bound));
                    // Extract variable names from the pattern
                    let mut pattern_vars = HashSet::new();
                    self.collect_pattern_bindings(&bind_decl.pattern, &mut pattern_vars);
                    block_bound.extend(pattern_vars);
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
            Pattern::Ok(p) | Pattern::Err(p) => {
                self.collect_pattern_bindings(p, bindings);
            }
            Pattern::EnumVariant { payload, .. } => {
                if let Some(payload) = payload {
                    self.collect_pattern_bindings(payload, bindings);
                }
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
            Pattern::RecordDestruct { fields, rest, .. } => {
                // Collect bindings from fields
                for (_, p) in fields {
                    self.collect_pattern_bindings(p, bindings);
                }
                // Collect rest binding if present
                if let Some(rest_name) = rest {
                    if rest_name != "_" {
                        bindings.insert(rest_name.clone());
                    }
                }
            }
        }
    }

    /// Check if a type contains temporal parameters that are not in the allowed set
    fn check_temporal_escape(
        &self,
        ty: &TypedType,
        allowed_temporals: &HashSet<String>,
    ) -> Result<(), TypeError> {
        match ty {
            TypedType::Temporal {
                base_type,
                temporals,
            } => {
                for temporal in temporals {
                    if !allowed_temporals.contains(temporal) {
                        return Err(TypeError::TemporalEscape {
                            temporal: temporal.clone(),
                            message: format!("Temporal parameter {} escapes its scope", temporal),
                        });
                    }
                }
                self.check_temporal_escape(base_type, allowed_temporals)?;
            }
            TypedType::Function {
                params,
                return_type,
            } => {
                for param in params {
                    self.check_temporal_escape(param, allowed_temporals)?;
                }
                self.check_temporal_escape(return_type, allowed_temporals)?;
            }
            TypedType::Option(ty) => {
                self.check_temporal_escape(ty, allowed_temporals)?;
            }
            TypedType::Result(ok_ty, err_ty) => {
                self.check_temporal_escape(ok_ty, allowed_temporals)?;
                self.check_temporal_escape(err_ty, allowed_temporals)?;
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
