//! Public release-surface validation for the default and opt-in host ABIs.
//!
//! The type checker accepts some experimental or source-module-only shapes so
//! existing development tests can continue to exercise them. This pass is the
//! narrower release gate used by the CLI before reporting `--check` success or
//! generating host-visible WebAssembly exports.

use crate::ast::*;
use crate::type_checker::{format_typed_type, TypeChecker, TypedType};
use std::error::Error;
use std::fmt;
use std::str::FromStr;

const SCALAR_EXPORT_TYPES: &str = "Int32, Int64, Float64, Boolean, Char, and ()";
const FLAT_RECORD_FIELD_TYPES: &str = "Int32, Int64, Float64, Boolean, or Char";
const FLAT_RECORD_MAX_SLOTS: usize = 16;
const TAT_RELEASE_GATE_MESSAGE: &str =
    "Temporal Affine Types (TAT) are outside the default v0.0.1 release gate";

/// Selects the host-visible ABI contract enforced by release validation.
///
/// `V001Scalar` remains the default and preserves the published v0.0.1
/// contract. `FlatRecordV1` is an explicit opt-in profile for exported
/// functions that flatten small, source-exported scalar records at the host
/// boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HostAbiProfile {
    #[default]
    V001Scalar,
    FlatRecordV1,
}

impl HostAbiProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V001Scalar => "v0.0.1",
            Self::FlatRecordV1 => "flat-record-v1",
        }
    }
}

impl fmt::Display for HostAbiProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for HostAbiProfile {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "v0.0.1" => Ok(Self::V001Scalar),
            "flat-record-v1" => Ok(Self::FlatRecordV1),
            _ => Err(format!(
                "unknown host ABI profile '{}'; expected 'v0.0.1' or 'flat-record-v1'",
                value
            )),
        }
    }
}

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
    check_release_surface(program, checker, HostAbiProfile::V001Scalar)
}

/// Validates a program against the selected host ABI release profile.
pub fn check_release_surface(
    program: &Program,
    checker: &TypeChecker,
    profile: HostAbiProfile,
) -> Result<(), ReleaseSurfaceError> {
    for decl in &program.declarations {
        reject_tat_top_decl(decl)?;
        check_export_decl(program, decl, checker, profile)?;
    }

    Ok(())
}

fn check_export_decl(
    program: &Program,
    decl: &TopDecl,
    checker: &TypeChecker,
    profile: HostAbiProfile,
) -> Result<(), ReleaseSurfaceError> {
    let TopDecl::Export(export_decl) = decl else {
        return Ok(());
    };

    match export_decl.item.as_ref() {
        TopDecl::Function(func) => check_exported_function(program, func, checker, profile),
        TopDecl::Record(_) => Ok(()),
        TopDecl::Binding(binding) => check_exported_binding(binding, checker, profile),
        TopDecl::Export(_) => Err(ReleaseSurfaceError::new(match profile {
            HostAbiProfile::V001Scalar => "Nested exports are unsupported in v0.0.1".to_string(),
            HostAbiProfile::FlatRecordV1 => {
                "Nested exports are unsupported by host ABI profile flat-record-v1".to_string()
            }
        })),
        TopDecl::Impl(_) | TopDecl::Context(_) => {
            Err(ReleaseSurfaceError::new(match profile {
                HostAbiProfile::V001Scalar => "Only concrete function exports, source-level record exports, and constant global exports are supported in v0.0.1".to_string(),
                HostAbiProfile::FlatRecordV1 => "Only concrete function exports, source-level record exports, and constant scalar global exports are supported by host ABI profile flat-record-v1".to_string(),
            }))
        }
    }
}

fn check_exported_function(
    program: &Program,
    func: &FunDecl,
    checker: &TypeChecker,
    profile: HostAbiProfile,
) -> Result<(), ReleaseSurfaceError> {
    match profile {
        HostAbiProfile::V001Scalar => check_v001_exported_function(func, checker),
        HostAbiProfile::FlatRecordV1 => {
            check_flat_record_v1_exported_function(program, func, checker)
        }
    }
}

fn check_v001_exported_function(
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

fn check_flat_record_v1_exported_function(
    program: &Program,
    func: &FunDecl,
    checker: &TypeChecker,
) -> Result<(), ReleaseSurfaceError> {
    if !func.type_params.is_empty() {
        return Err(ReleaseSurfaceError::new(format!(
            "Exported generic function '{}' requires a concrete ABI and is not supported by host ABI profile flat-record-v1",
            func.name
        )));
    }

    let mut parameter_slots = 0usize;
    for param in &func.params {
        let slots = flat_record_v1_source_slots(
            program,
            &func.name,
            &format!("parameter '{}'", param.name),
            FlatRecordSlotPosition::Parameter,
            &param.ty,
        )?;
        parameter_slots = parameter_slots.checked_add(slots).ok_or_else(|| {
            ReleaseSurfaceError::new(format!(
                "Exported function '{}' parameter ABI exceeds the flat-record-v1 slot limit",
                func.name
            ))
        })?;
    }
    if parameter_slots > FLAT_RECORD_MAX_SLOTS {
        return Err(ReleaseSurfaceError::new(format!(
            "Exported function '{}' flattens to {} parameter slots; host ABI profile flat-record-v1 supports at most {}",
            func.name, parameter_slots, FLAT_RECORD_MAX_SLOTS
        )));
    }

    let return_slots = if let Some(return_type) = &func.return_type {
        flat_record_v1_source_slots(
            program,
            &func.name,
            "return",
            FlatRecordSlotPosition::Return,
            return_type,
        )?
    } else {
        let inferred = checker
            .checked_function_return_type(&func.name)
            .ok_or_else(|| {
                ReleaseSurfaceError::new(format!(
                    "Exported function '{}' has no checked return type for flat-record-v1 ABI validation",
                    func.name
                ))
            })?;
        flat_record_v1_checked_slots(
            program,
            &func.name,
            "return",
            FlatRecordSlotPosition::Return,
            &inferred,
        )?
    };
    if return_slots > FLAT_RECORD_MAX_SLOTS {
        return Err(ReleaseSurfaceError::new(format!(
            "Exported function '{}' flattens to {} return slots; host ABI profile flat-record-v1 supports at most {}",
            func.name, return_slots, FLAT_RECORD_MAX_SLOTS
        )));
    }

    Ok(())
}

#[derive(Clone, Copy)]
enum FlatRecordSlotPosition {
    Parameter,
    Return,
}

impl FlatRecordSlotPosition {
    const fn unit_slots(self) -> usize {
        match self {
            // Preserve the existing internal convention where a Unit
            // parameter crosses the host boundary as one dummy i32.
            Self::Parameter => 1,
            Self::Return => 0,
        }
    }
}

fn flat_record_v1_source_slots(
    program: &Program,
    export_name: &str,
    position: &str,
    slot_position: FlatRecordSlotPosition,
    ty: &Type,
) -> Result<usize, ReleaseSurfaceError> {
    match ty {
        Type::Named(name) if name == "Unit" => Ok(slot_position.unit_slots()),
        Type::Named(name) if is_direct_record_scalar_name(name) => Ok(1),
        Type::Named(name) => flat_record_v1_record_slots(program, export_name, position, name),
        Type::Generic(_, _) => Err(flat_record_v1_unsupported_type(
            export_name,
            position,
            &format_source_type(ty),
            "generic records and built-in composite types are not supported",
        )),
        Type::Function(_, _) => Err(flat_record_v1_unsupported_type(
            export_name,
            position,
            &format_source_type(ty),
            "function and closure values are not supported",
        )),
        Type::Temporal(_, _) => Err(flat_record_v1_unsupported_type(
            export_name,
            position,
            &format_source_type(ty),
            "temporal values are not supported",
        )),
    }
}

fn flat_record_v1_checked_slots(
    program: &Program,
    export_name: &str,
    position: &str,
    slot_position: FlatRecordSlotPosition,
    ty: &TypedType,
) -> Result<usize, ReleaseSurfaceError> {
    match ty {
        TypedType::Int32
        | TypedType::Int64
        | TypedType::Float64
        | TypedType::Boolean
        | TypedType::Char => Ok(1),
        TypedType::Unit => Ok(slot_position.unit_slots()),
        TypedType::Record {
            name, type_args, ..
        } if type_args.is_empty() => {
            flat_record_v1_record_slots(program, export_name, position, name)
        }
        TypedType::Record { .. } => Err(flat_record_v1_unsupported_type(
            export_name,
            position,
            &format_typed_type(ty),
            "generic records are not supported",
        )),
        TypedType::String
        | TypedType::Option(_)
        | TypedType::Result(_, _)
        | TypedType::List(_)
        | TypedType::Array(_, _) => Err(flat_record_v1_unsupported_type(
            export_name,
            position,
            &format_typed_type(ty),
            "String, List, Array, Option, and Result values are not supported",
        )),
        TypedType::Function { .. } => Err(flat_record_v1_unsupported_type(
            export_name,
            position,
            &format_typed_type(ty),
            "function and closure values are not supported",
        )),
        TypedType::TypeParam(_) => Err(flat_record_v1_unsupported_type(
            export_name,
            position,
            &format_typed_type(ty),
            "generic type parameters are not supported",
        )),
        TypedType::InferVar(_) | TypedType::Projection { .. } => {
            Err(flat_record_v1_unsupported_type(
                export_name,
                position,
                &format_typed_type(ty),
                "the checked type is not concrete",
            ))
        }
        TypedType::Temporal { .. } => Err(flat_record_v1_unsupported_type(
            export_name,
            position,
            &format_typed_type(ty),
            "temporal values are not supported",
        )),
    }
}

fn flat_record_v1_record_slots(
    program: &Program,
    export_name: &str,
    position: &str,
    record_name: &str,
) -> Result<usize, ReleaseSurfaceError> {
    let (record, source_exported) =
        find_record_declaration(program, record_name).ok_or_else(|| {
            flat_record_v1_unsupported_type(
                export_name,
                position,
                record_name,
                "the record declaration is unavailable",
            )
        })?;

    if !source_exported {
        return Err(ReleaseSurfaceError::new(format!(
            "Exported function '{}' {} record '{}' must be source-exported for host ABI profile flat-record-v1",
            export_name, position, record_name
        )));
    }
    if !record.type_params.is_empty() {
        return Err(ReleaseSurfaceError::new(format!(
            "Exported function '{}' {} record '{}' is generic; host ABI profile flat-record-v1 supports only non-generic records",
            export_name, position, record_name
        )));
    }
    if record.fields.is_empty() {
        return Err(ReleaseSurfaceError::new(format!(
            "Exported function '{}' {} record '{}' is empty; host ABI profile flat-record-v1 requires 1..={} direct scalar fields",
            export_name, position, record_name, FLAT_RECORD_MAX_SLOTS
        )));
    }
    if record.fields.len() > FLAT_RECORD_MAX_SLOTS {
        return Err(ReleaseSurfaceError::new(format!(
            "Exported function '{}' {} record '{}' has {} fields; host ABI profile flat-record-v1 supports at most {}",
            export_name,
            position,
            record_name,
            record.fields.len(),
            FLAT_RECORD_MAX_SLOTS
        )));
    }

    for field in &record.fields {
        if !is_flat_record_field_type(&field.ty) {
            return Err(ReleaseSurfaceError::new(format!(
                "Exported function '{}' {} record '{}' field '{}' type {} is unsupported by host ABI profile flat-record-v1; record fields must be direct {} scalars",
                export_name,
                position,
                record_name,
                field.name,
                format_source_type(&field.ty),
                FLAT_RECORD_FIELD_TYPES
            )));
        }
    }

    Ok(record.fields.len())
}

fn flat_record_v1_unsupported_type(
    export_name: &str,
    position: &str,
    ty: &str,
    reason: &str,
) -> ReleaseSurfaceError {
    ReleaseSurfaceError::new(format!(
        "Exported function '{}' {} type {} is unsupported by host ABI profile flat-record-v1: {}",
        export_name, position, ty, reason
    ))
}

fn find_record_declaration<'a>(
    program: &'a Program,
    record_name: &str,
) -> Option<(&'a RecordDecl, bool)> {
    program.declarations.iter().find_map(|decl| match decl {
        TopDecl::Record(record) if record.name == record_name => Some((record, false)),
        TopDecl::Export(export)
            if matches!(export.item.as_ref(), TopDecl::Record(record) if record.name == record_name) =>
        {
            let TopDecl::Record(record) = export.item.as_ref() else {
                unreachable!("record export match should preserve the record declaration")
            };
            Some((record, true))
        }
        _ => None,
    })
}

fn is_flat_record_field_type(ty: &Type) -> bool {
    matches!(ty, Type::Named(name) if is_direct_record_scalar_name(name))
}

fn is_direct_record_scalar_name(name: &str) -> bool {
    matches!(name, "Int32" | "Int64" | "Float64" | "Boolean" | "Char")
}

fn check_exported_binding(
    binding: &BindDecl,
    checker: &TypeChecker,
    profile: HostAbiProfile,
) -> Result<(), ReleaseSurfaceError> {
    if binding.mutable {
        return Err(ReleaseSurfaceError::new(match profile {
            HostAbiProfile::V001Scalar => {
                "Exported top-level bindings must be immutable scalar constants in v0.0.1"
                    .to_string()
            }
            HostAbiProfile::FlatRecordV1 => "Exported top-level bindings must be immutable scalar constants for host ABI profile flat-record-v1".to_string(),
        }));
    }

    let Pattern::Ident(name) = &binding.pattern else {
        return Err(ReleaseSurfaceError::new(match profile {
            HostAbiProfile::V001Scalar => {
                "Complex top-level binding exports are unsupported in v0.0.1".to_string()
            }
            HostAbiProfile::FlatRecordV1 => "Complex top-level binding exports are unsupported by host ABI profile flat-record-v1".to_string(),
        }));
    };

    if let Some(annotation) = &binding.type_annotation {
        ensure_scalar_global_source_type(name, annotation, profile)?;
    } else {
        let inferred = checker.checked_variable_type(name).ok_or_else(|| {
            ReleaseSurfaceError::new(format!(
                "Exported top-level binding '{}' has no checked type for {} ABI validation",
                name, profile
            ))
        })?;
        ensure_scalar_global_checked_type(name, &inferred, profile)?;
    }

    if !is_scalar_literal_constant(&binding.value) {
        return Err(ReleaseSurfaceError::new(match profile {
            HostAbiProfile::V001Scalar => format!(
                "Exported top-level binding '{}' must be a scalar literal constant in v0.0.1",
                name
            ),
            HostAbiProfile::FlatRecordV1 => format!(
                "Exported top-level binding '{}' must be a scalar literal constant for host ABI profile flat-record-v1",
                name
            ),
        }));
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

fn ensure_scalar_global_source_type(
    name: &str,
    ty: &Type,
    profile: HostAbiProfile,
) -> Result<(), ReleaseSurfaceError> {
    if is_scalar_source_type(ty) {
        return Ok(());
    }

    Err(ReleaseSurfaceError::new(match profile {
        HostAbiProfile::V001Scalar => format!(
            "Exported top-level binding '{}' has type {} which requires a composite host ABI; v0.0.1 global exports support only scalar {}",
            name,
            format_source_type(ty),
            SCALAR_EXPORT_TYPES
        ),
        HostAbiProfile::FlatRecordV1 => format!(
            "Exported top-level binding '{}' has type {} which requires a composite host ABI; flat-record-v1 global exports support only scalar {}",
            name,
            format_source_type(ty),
            SCALAR_EXPORT_TYPES
        ),
    }))
}

fn ensure_scalar_global_checked_type(
    name: &str,
    ty: &TypedType,
    profile: HostAbiProfile,
) -> Result<(), ReleaseSurfaceError> {
    if is_scalar_checked_type(ty) {
        return Ok(());
    }

    Err(ReleaseSurfaceError::new(match profile {
        HostAbiProfile::V001Scalar => format!(
            "Exported top-level binding '{}' has type {} which requires a composite host ABI; v0.0.1 global exports support only scalar {}",
            name,
            format_typed_type(ty),
            SCALAR_EXPORT_TYPES
        ),
        HostAbiProfile::FlatRecordV1 => format!(
            "Exported top-level binding '{}' has type {} which requires a composite host ABI; flat-record-v1 global exports support only scalar {}",
            name,
            format_typed_type(ty),
            SCALAR_EXPORT_TYPES
        ),
    }))
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
