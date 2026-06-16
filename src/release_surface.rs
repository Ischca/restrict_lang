//! v0.0.1 public release-surface validation.
//!
//! The type checker accepts some experimental or source-module-only shapes so
//! existing development tests can continue to exercise them. This pass is the
//! narrower release gate used by the CLI before reporting `--check` success or
//! generating host-visible WebAssembly exports.

use crate::ast::*;
use crate::type_checker::{format_typed_type, TypeChecker, TypedType};
use std::error::Error;
use std::fmt;

const SCALAR_EXPORT_TYPES: &str = "Int32, Int64, Float64, Boolean, Char, and ()";
const TAT_RELEASE_GATE_MESSAGE: &str =
    "Temporal Affine Types (TAT) are outside the default v0.0.1 release gate";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseSurfaceError {
    message: String,
}

impl ReleaseSurfaceError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ReleaseSurfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for ReleaseSurfaceError {}

pub fn check_v001_release_surface(
    program: &Program,
    checker: &TypeChecker,
) -> Result<(), ReleaseSurfaceError> {
    for decl in &program.declarations {
        reject_tat_top_decl(decl)?;
        check_export_decl(decl, checker)?;
    }

    Ok(())
}

fn check_export_decl(decl: &TopDecl, checker: &TypeChecker) -> Result<(), ReleaseSurfaceError> {
    let TopDecl::Export(export_decl) = decl else {
        return Ok(());
    };

    match export_decl.item.as_ref() {
        TopDecl::Function(func) => check_exported_function(func, checker),
        TopDecl::Record(_) => Ok(()),
        TopDecl::Binding(binding) => check_exported_binding(binding, checker),
        TopDecl::Export(_) => Err(ReleaseSurfaceError::new(
            "Nested exports are unsupported in v0.0.1",
        )),
        TopDecl::Impl(_) | TopDecl::Context(_) => Err(ReleaseSurfaceError::new(
            "Only concrete function exports, source-level record exports, and constant global exports are supported in v0.0.1",
        )),
    }
}

fn check_exported_function(
    func: &FunDecl,
    checker: &TypeChecker,
) -> Result<(), ReleaseSurfaceError> {
    if !func.type_params.is_empty() {
        return Err(ReleaseSurfaceError::new(format!(
            "Exported generic function '{}' requires a concrete ABI and is not supported in v0.0.1",
            func.name
        )));
    }

    for param in &func.params {
        ensure_scalar_source_type(
            &func.name,
            &format!("parameter '{}'", param.name),
            &param.ty,
        )?;
    }

    if let Some(return_type) = &func.return_type {
        ensure_scalar_source_type(&func.name, "return", return_type)?;
    } else {
        let inferred = checker
            .checked_function_return_type(&func.name)
            .ok_or_else(|| {
                ReleaseSurfaceError::new(format!(
                    "Exported function '{}' has no checked return type for v0.0.1 ABI validation",
                    func.name
                ))
            })?;
        ensure_scalar_checked_type(&func.name, "return", &inferred)?;
    }

    Ok(())
}

fn check_exported_binding(
    binding: &BindDecl,
    checker: &TypeChecker,
) -> Result<(), ReleaseSurfaceError> {
    if binding.mutable {
        return Err(ReleaseSurfaceError::new(
            "Exported top-level bindings must be immutable scalar constants in v0.0.1",
        ));
    }

    let Pattern::Ident(name) = &binding.pattern else {
        return Err(ReleaseSurfaceError::new(
            "Complex top-level binding exports are unsupported in v0.0.1",
        ));
    };

    if let Some(annotation) = &binding.type_annotation {
        ensure_scalar_global_source_type(name, annotation)?;
    } else {
        let inferred = checker.checked_variable_type(name).ok_or_else(|| {
            ReleaseSurfaceError::new(format!(
                "Exported top-level binding '{}' has no checked type for v0.0.1 ABI validation",
                name
            ))
        })?;
        ensure_scalar_global_checked_type(name, &inferred)?;
    }

    if !is_scalar_literal_constant(&binding.value) {
        return Err(ReleaseSurfaceError::new(format!(
            "Exported top-level binding '{}' must be a scalar literal constant in v0.0.1",
            name
        )));
    }

    Ok(())
}

fn ensure_scalar_source_type(
    export_name: &str,
    position: &str,
    ty: &Type,
) -> Result<(), ReleaseSurfaceError> {
    if is_scalar_source_type(ty) {
        return Ok(());
    }

    Err(ReleaseSurfaceError::new(format!(
        "Exported function '{}' {} type {} requires a composite host ABI; v0.0.1 exports support only scalar {}",
        export_name,
        position,
        format_source_type(ty),
        SCALAR_EXPORT_TYPES
    )))
}

fn ensure_scalar_checked_type(
    export_name: &str,
    position: &str,
    ty: &TypedType,
) -> Result<(), ReleaseSurfaceError> {
    if is_scalar_checked_type(ty) {
        return Ok(());
    }

    Err(ReleaseSurfaceError::new(format!(
        "Exported function '{}' {} type {} requires a composite host ABI; v0.0.1 exports support only scalar {}",
        export_name,
        position,
        format_typed_type(ty),
        SCALAR_EXPORT_TYPES
    )))
}

fn ensure_scalar_global_source_type(name: &str, ty: &Type) -> Result<(), ReleaseSurfaceError> {
    if is_scalar_source_type(ty) {
        return Ok(());
    }

    Err(ReleaseSurfaceError::new(format!(
        "Exported top-level binding '{}' has type {} which requires a composite host ABI; v0.0.1 global exports support only scalar {}",
        name,
        format_source_type(ty),
        SCALAR_EXPORT_TYPES
    )))
}

fn ensure_scalar_global_checked_type(
    name: &str,
    ty: &TypedType,
) -> Result<(), ReleaseSurfaceError> {
    if is_scalar_checked_type(ty) {
        return Ok(());
    }

    Err(ReleaseSurfaceError::new(format!(
        "Exported top-level binding '{}' has type {} which requires a composite host ABI; v0.0.1 global exports support only scalar {}",
        name,
        format_typed_type(ty),
        SCALAR_EXPORT_TYPES
    )))
}

fn is_scalar_source_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Named(name)
            if matches!(
                name.as_str(),
                "Int32" | "Int64" | "Float64" | "Boolean" | "Char" | "Unit"
            )
    )
}

fn is_scalar_checked_type(ty: &TypedType) -> bool {
    matches!(
        ty,
        TypedType::Int32
            | TypedType::Int64
            | TypedType::Float64
            | TypedType::Boolean
            | TypedType::Char
            | TypedType::Unit
    )
}

fn is_scalar_literal_constant(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::IntLit(_)
        | ExprKind::FloatLit(_)
        | ExprKind::BoolLit(_)
        | ExprKind::CharLit(_)
        | ExprKind::Unit => true,
        ExprKind::Unary(unary) if matches!(unary.op, UnaryOp::Neg) => {
            matches!(
                &unary.expr.kind,
                ExprKind::IntLit(_) | ExprKind::FloatLit(_)
            )
        }
        _ => false,
    }
}

fn format_source_type(ty: &Type) -> String {
    match ty {
        Type::Named(name) if name == "Unit" => "()".to_string(),
        _ => ty.to_string(),
    }
}

fn reject_tat_top_decl(decl: &TopDecl) -> Result<(), ReleaseSurfaceError> {
    match decl {
        TopDecl::Record(record) => {
            reject_tat_type_params("record", &record.name, &record.type_params)?;
            reject_tat_constraints("record", &record.name, &record.temporal_constraints)?;
            for field in &record.fields {
                reject_tat_type(&format!("record '{}'", record.name), &field.ty)?;
            }
        }
        TopDecl::Function(func) => reject_tat_function(func)?,
        TopDecl::Binding(binding) => reject_tat_binding(binding)?,
        TopDecl::Impl(impl_block) => {
            for func in &impl_block.functions {
                reject_tat_function(func)?;
            }
        }
        TopDecl::Context(context) => {
            for field in &context.fields {
                reject_tat_type(&format!("context '{}'", context.name), &field.ty)?;
            }
        }
        TopDecl::Export(export_decl) => reject_tat_top_decl(export_decl.item.as_ref())?,
    }

    Ok(())
}

fn reject_tat_function(func: &FunDecl) -> Result<(), ReleaseSurfaceError> {
    reject_tat_type_params("function", &func.name, &func.type_params)?;
    reject_tat_constraints("function", &func.name, &func.temporal_constraints)?;
    for param in &func.params {
        reject_tat_type(&format!("function '{}'", func.name), &param.ty)?;
    }
    if let Some(return_type) = &func.return_type {
        reject_tat_type(&format!("function '{}'", func.name), return_type)?;
    }
    reject_tat_block(&func.body)
}

fn reject_tat_binding(binding: &BindDecl) -> Result<(), ReleaseSurfaceError> {
    if let Some(annotation) = &binding.type_annotation {
        reject_tat_type("binding annotation", annotation)?;
    }
    reject_tat_expr(&binding.value)
}

fn reject_tat_type_params(
    kind: &str,
    name: &str,
    type_params: &[TypeParam],
) -> Result<(), ReleaseSurfaceError> {
    if type_params.iter().any(|param| param.is_temporal) {
        return Err(ReleaseSurfaceError::new(format!(
            "{} '{}' uses temporal type parameters; {}",
            kind, name, TAT_RELEASE_GATE_MESSAGE
        )));
    }
    Ok(())
}

fn reject_tat_constraints(
    kind: &str,
    name: &str,
    constraints: &[TemporalConstraint],
) -> Result<(), ReleaseSurfaceError> {
    if !constraints.is_empty() {
        return Err(ReleaseSurfaceError::new(format!(
            "{} '{}' uses temporal constraints; {}",
            kind, name, TAT_RELEASE_GATE_MESSAGE
        )));
    }
    Ok(())
}

fn reject_tat_type(context: &str, ty: &Type) -> Result<(), ReleaseSurfaceError> {
    match ty {
        Type::Temporal(_, _) => Err(ReleaseSurfaceError::new(format!(
            "{} uses temporal types; {}",
            context, TAT_RELEASE_GATE_MESSAGE
        ))),
        Type::Generic(_, params) => {
            for param in params {
                reject_tat_type(context, param)?;
            }
            Ok(())
        }
        Type::Function(params, return_type) => {
            for param in params {
                reject_tat_type(context, param)?;
            }
            reject_tat_type(context, return_type)
        }
        Type::Named(_) => Ok(()),
    }
}

fn reject_tat_block(block: &BlockExpr) -> Result<(), ReleaseSurfaceError> {
    for stmt in &block.statements {
        match stmt {
            Stmt::Binding(binding) => reject_tat_binding(binding)?,
            Stmt::Assignment(assign) => reject_tat_expr(&assign.value)?,
            Stmt::Expr(expr) => reject_tat_expr(expr)?,
        }
    }
    if let Some(expr) = &block.expr {
        reject_tat_expr(expr)?;
    }
    Ok(())
}

fn reject_tat_expr(expr: &Expr) -> Result<(), ReleaseSurfaceError> {
    match &expr.kind {
        ExprKind::Cast(cast) => {
            reject_tat_expr(&cast.expr)?;
            reject_tat_type("cast target", &cast.target)
        }
        ExprKind::Binary(binary) => {
            reject_tat_expr(&binary.left)?;
            reject_tat_expr(&binary.right)
        }
        ExprKind::Unary(unary) => reject_tat_expr(&unary.expr),
        ExprKind::Call(call) => {
            reject_tat_expr(&call.function)?;
            for arg in &call.args {
                reject_tat_expr(arg)?;
            }
            Ok(())
        }
        ExprKind::Pipe(pipe) => {
            reject_tat_expr(&pipe.expr)?;
            if let PipeTarget::Expr(target) = &pipe.target {
                reject_tat_expr(target)?;
            }
            Ok(())
        }
        ExprKind::FieldAccess(base, _) => reject_tat_expr(base),
        ExprKind::RecordLit(record) => reject_tat_record_lit(record),
        ExprKind::Clone(clone_expr) => {
            reject_tat_expr(&clone_expr.base)?;
            reject_tat_record_lit(&clone_expr.updates)
        }
        ExprKind::PrototypeClone(proto_clone) => reject_tat_record_lit(&proto_clone.updates),
        ExprKind::Freeze(inner)
        | ExprKind::Some(inner)
        | ExprKind::Ok(inner)
        | ExprKind::Err(inner)
        | ExprKind::Await(inner)
        | ExprKind::Spawn(inner) => reject_tat_expr(inner),
        ExprKind::ListLit(elements) | ExprKind::ArrayLit(elements) => {
            for element in elements {
                reject_tat_expr(element)?;
            }
            Ok(())
        }
        ExprKind::RangeLit(range) => {
            reject_tat_expr(&range.start)?;
            reject_tat_expr(&range.end)
        }
        ExprKind::Match(match_expr) => {
            reject_tat_expr(&match_expr.expr)?;
            for arm in &match_expr.arms {
                reject_tat_block(&arm.body)?;
            }
            Ok(())
        }
        ExprKind::Then(then_expr) => {
            reject_tat_expr(&then_expr.condition)?;
            reject_tat_block(&then_expr.then_block)?;
            for (condition, block) in &then_expr.else_ifs {
                reject_tat_expr(condition)?;
                reject_tat_block(block)?;
            }
            if let Some(block) = &then_expr.else_block {
                reject_tat_block(block)?;
            }
            Ok(())
        }
        ExprKind::While(while_expr) => {
            reject_tat_expr(&while_expr.condition)?;
            reject_tat_block(&while_expr.body)
        }
        ExprKind::Block(block) => reject_tat_block(block),
        ExprKind::Lambda(lambda) => {
            for param in &lambda.params {
                if let Some(annotation) = &param.type_annotation {
                    reject_tat_type("lambda parameter", annotation)?;
                }
            }
            reject_tat_expr(&lambda.body)
        }
        ExprKind::With(with_expr) => {
            for binding in &with_expr.bindings {
                reject_tat_field_init(binding)?;
            }
            reject_tat_block(&with_expr.body)
        }
        ExprKind::WithLifetime(_) => Err(ReleaseSurfaceError::new(format!(
            "with lifetime blocks are unsupported in v0.0.1; {}",
            TAT_RELEASE_GATE_MESSAGE
        ))),
        ExprKind::IntLit(_)
        | ExprKind::FloatLit(_)
        | ExprKind::StringLit(_)
        | ExprKind::CharLit(_)
        | ExprKind::BoolLit(_)
        | ExprKind::Unit
        | ExprKind::Ident(_)
        | ExprKind::None => Ok(()),
    }
}

fn reject_tat_record_lit(record: &RecordLit) -> Result<(), ReleaseSurfaceError> {
    for field in &record.fields {
        reject_tat_field_init(field)?;
    }
    Ok(())
}

fn reject_tat_field_init(field: &FieldInit) -> Result<(), ReleaseSurfaceError> {
    match field {
        FieldInit::Field { value, .. } | FieldInit::Spread(value) => reject_tat_expr(value),
    }
}
