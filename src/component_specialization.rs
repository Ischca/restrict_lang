//! Compiler-owned explicit generic specialization planning for Component ABI.
//!
//! This module deliberately stops before body code generation. It validates a
//! finite concrete instance against the checked source declaration, assigns an
//! injective stable identity and Core symbol, and deduplicates identical
//! requests. Later Component lowering can consume this plan without treating
//! incidental call-site monomorphization as an ABI declaration.

use crate::type_checker::{
    CheckedExplicitFunctionSpecialization, ExplicitFunctionSpecializationError, TypeChecker,
    TypedType,
};
use std::collections::BTreeMap;
use thiserror::Error;

pub const COMPONENT_SPECIALIZATION_IDENTITY_SCHEMA: u32 = 1;
const COMPONENT_SPECIALIZATION_SYMBOL_PREFIX: &str = "__restrict_component_specialization$sid$";

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentFunctionSpecializationRequest {
    /// Stable package/module/declaration identity supplied by source resolution.
    pub source_identity: String,
    /// Current root-function lookup key used by the checked program.
    pub source_function: String,
    /// Concrete arguments keyed by source type-parameter name.
    pub type_arguments: BTreeMap<String, TypedType>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentFunctionSpecialization {
    pub source_identity: String,
    pub source_function: String,
    /// Concrete arguments in declared type-parameter order.
    pub type_arguments: Vec<(String, TypedType)>,
    pub params: Vec<(String, TypedType)>,
    pub return_type: TypedType,
    /// Injective, versioned semantic identity independent of request ordering.
    pub identity: String,
    /// Injective WAT-safe internal symbol derived from the complete identity.
    pub core_symbol: String,
}

#[derive(Debug, Error, PartialEq)]
pub enum ComponentSpecializationError {
    #[error("Component specialization source identity must not be empty")]
    EmptySourceIdentity,
    #[error("Component specialization source function must not be empty")]
    EmptySourceFunction,
    #[error(transparent)]
    Validation(#[from] ExplicitFunctionSpecializationError),
    #[error("Component specialization identity cannot encode {position} type '{ty}'")]
    UnsupportedIdentityType { position: String, ty: String },
    #[error("two non-identical Component specializations produced identity '{0}'")]
    IdentityCollision(String),
}

/// Deterministic per-compilation set of explicitly requested instances.
pub struct ComponentSpecializationPlanner<'a> {
    checker: &'a TypeChecker,
    instances: BTreeMap<String, ComponentFunctionSpecialization>,
}

impl<'a> ComponentSpecializationPlanner<'a> {
    pub fn new(checker: &'a TypeChecker) -> Self {
        Self {
            checker,
            instances: BTreeMap::new(),
        }
    }

    pub fn request_function(
        &mut self,
        request: ComponentFunctionSpecializationRequest,
    ) -> Result<ComponentFunctionSpecialization, ComponentSpecializationError> {
        if request.source_identity.is_empty() {
            return Err(ComponentSpecializationError::EmptySourceIdentity);
        }
        if request.source_function.is_empty() {
            return Err(ComponentSpecializationError::EmptySourceFunction);
        }

        let checked = self.checker.checked_explicit_function_specialization(
            &request.source_function,
            &request.type_arguments,
        )?;
        let identity = specialization_identity(&request.source_identity, &checked)?;
        let instance = ComponentFunctionSpecialization {
            source_identity: request.source_identity,
            source_function: checked.source_function,
            type_arguments: checked.type_arguments,
            params: checked.params,
            return_type: checked.return_type,
            core_symbol: specialization_core_symbol(&identity),
            identity: identity.clone(),
        };

        if let Some(existing) = self.instances.get(&identity) {
            if existing != &instance {
                return Err(ComponentSpecializationError::IdentityCollision(identity));
            }
            return Ok(existing.clone());
        }
        self.instances.insert(identity, instance.clone());
        Ok(instance)
    }

    pub fn instances(&self) -> impl Iterator<Item = &ComponentFunctionSpecialization> {
        self.instances.values()
    }

    pub fn len(&self) -> usize {
        self.instances.len()
    }

    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }
}

fn specialization_identity(
    source_identity: &str,
    checked: &CheckedExplicitFunctionSpecialization,
) -> Result<String, ComponentSpecializationError> {
    let mut identity = format!(
        "restrict-component-specialization-v{}",
        COMPONENT_SPECIALIZATION_IDENTITY_SCHEMA
    );
    append_identity_part(&mut identity, "decl", source_identity);
    identity.push_str("args");
    identity.push_str(&checked.type_arguments.len().to_string());
    identity.push('x');
    for (parameter, ty) in &checked.type_arguments {
        append_identity_part(&mut identity, "param", parameter);
        let encoded = canonical_type_identity(ty).map_err(|ty| {
            ComponentSpecializationError::UnsupportedIdentityType {
                position: format!("type argument '{parameter}'"),
                ty,
            }
        })?;
        append_identity_part(&mut identity, "type", &encoded);
    }
    Ok(identity)
}

fn specialization_core_symbol(identity: &str) -> String {
    let mut symbol = String::from(COMPONENT_SPECIALIZATION_SYMBOL_PREFIX);
    for byte in identity.as_bytes() {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        symbol.push(HEX[usize::from(byte >> 4)] as char);
        symbol.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    symbol
}

fn append_identity_part(output: &mut String, tag: &str, value: &str) {
    output.push_str(tag);
    output.push_str(&value.len().to_string());
    output.push('x');
    output.push_str(value);
}

/// Canonical, injective identity for finalized Restrict types.
fn canonical_type_identity(ty: &TypedType) -> Result<String, String> {
    let mut output = String::new();
    match ty {
        TypedType::Int32 => output.push_str("s32"),
        TypedType::Int64 => output.push_str("s64"),
        TypedType::Float64 => output.push_str("f64"),
        TypedType::Boolean => output.push_str("bool"),
        TypedType::String => output.push_str("string"),
        TypedType::Char => output.push_str("char"),
        TypedType::Unit => output.push_str("unit"),
        TypedType::Record {
            name, type_args, ..
        } => {
            output.push_str("record");
            append_identity_part(&mut output, "name", name);
            // Prototype/layout hashes and local mutability state are compiler
            // details, not part of the source-level nominal type identity.
            append_type_list(&mut output, "args", type_args)?;
        }
        TypedType::Enum { name } => {
            output.push_str("enum");
            append_identity_part(&mut output, "name", name);
        }
        TypedType::Function {
            params,
            return_type,
        } => {
            output.push_str("function");
            append_type_list(&mut output, "params", params)?;
            let result = canonical_type_identity(return_type)?;
            append_identity_part(&mut output, "result", &result);
        }
        TypedType::Option(inner) => {
            output.push_str("option");
            append_nested_type(&mut output, inner)?;
        }
        TypedType::Result(ok, err) => {
            output.push_str("result");
            append_nested_type(&mut output, ok)?;
            append_nested_type(&mut output, err)?;
        }
        TypedType::List(inner) => {
            output.push_str("list");
            append_nested_type(&mut output, inner)?;
        }
        TypedType::Array(inner, crate::type_checker::ArrayLength::Known(length)) => {
            output.push_str("array");
            output.push_str(&length.to_string());
            output.push('x');
            append_nested_type(&mut output, inner)?;
        }
        TypedType::Array(_, crate::type_checker::ArrayLength::AnyInternal)
        | TypedType::TypeParam(_)
        | TypedType::InferVar(_)
        | TypedType::Projection { .. }
        | TypedType::Temporal { .. } => return Err(crate::type_checker::format_typed_type(ty)),
    }
    Ok(output)
}

fn append_nested_type(output: &mut String, ty: &TypedType) -> Result<(), String> {
    let nested = canonical_type_identity(ty)?;
    append_identity_part(output, "type", &nested);
    Ok(())
}

fn append_type_list(output: &mut String, tag: &str, types: &[TypedType]) -> Result<(), String> {
    output.push_str(tag);
    output.push_str(&types.len().to_string());
    output.push('x');
    for ty in types {
        append_nested_type(output, ty)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_program;

    fn checked(source: &str) -> TypeChecker {
        let (remaining, program) = parse_program(source).expect("source should parse");
        assert!(remaining.trim().is_empty());
        let mut checker = TypeChecker::new();
        checker
            .check_program(&program)
            .expect("source should type check");
        checker
    }

    fn request(
        source_identity: &str,
        source_function: &str,
        bindings: impl IntoIterator<Item = (&'static str, TypedType)>,
    ) -> ComponentFunctionSpecializationRequest {
        ComponentFunctionSpecializationRequest {
            source_identity: source_identity.to_string(),
            source_function: source_function.to_string(),
            type_arguments: bindings
                .into_iter()
                .map(|(name, ty)| (name.to_string(), ty))
                .collect(),
        }
    }

    #[test]
    fn explicit_instance_is_finalized_in_declaration_order() {
        let checker =
            checked("fun choose: <T, U>(left: T, right: U) -> Result<T, U> = { left Result::Ok }");
        let mut planner = ComponentSpecializationPlanner::new(&checker);
        let instance = planner
            .request_function(request(
                "test:pkg/root.choose",
                "choose",
                [("U", TypedType::String), ("T", TypedType::Int32)],
            ))
            .unwrap();

        assert_eq!(
            instance.type_arguments,
            vec![
                ("T".to_string(), TypedType::Int32),
                ("U".to_string(), TypedType::String),
            ]
        );
        assert_eq!(
            instance.params,
            vec![
                ("left".to_string(), TypedType::Int32),
                ("right".to_string(), TypedType::String),
            ]
        );
        assert_eq!(
            instance.return_type,
            TypedType::Result(Box::new(TypedType::Int32), Box::new(TypedType::String))
        );
        assert!(instance
            .core_symbol
            .starts_with(COMPONENT_SPECIALIZATION_SYMBOL_PREFIX));
    }

    #[test]
    fn missing_extra_open_and_temporal_arguments_are_typed_errors() {
        let checker = checked("fun identity: <T>(value: T) -> T = { value }");
        let mut planner = ComponentSpecializationPlanner::new(&checker);

        assert!(matches!(
            planner.request_function(request("test:pkg/root.identity", "identity", [])),
            Err(ComponentSpecializationError::Validation(
                ExplicitFunctionSpecializationError::MissingTypeArguments { .. }
            ))
        ));
        assert!(matches!(
            planner.request_function(request(
                "test:pkg/root.identity",
                "identity",
                [("T", TypedType::Int32), ("U", TypedType::String)],
            )),
            Err(ComponentSpecializationError::Validation(
                ExplicitFunctionSpecializationError::ExtraTypeArguments { .. }
            ))
        ));
        assert!(matches!(
            planner.request_function(request(
                "test:pkg/root.identity",
                "identity",
                [("T", TypedType::TypeParam("Open".to_string()))],
            )),
            Err(ComponentSpecializationError::Validation(
                ExplicitFunctionSpecializationError::OpenTypeArgument { .. }
            ))
        ));
        assert!(matches!(
            planner.request_function(request(
                "test:pkg/root.identity",
                "identity",
                [(
                    "T",
                    TypedType::Temporal {
                        base_type: Box::new(TypedType::Int32),
                        temporals: vec!["scope".to_string()],
                    },
                )],
            )),
            Err(ComponentSpecializationError::Validation(
                ExplicitFunctionSpecializationError::TemporalTypeArgument { .. }
            ))
        ));
    }

    #[test]
    fn declared_bounds_are_checked_for_explicit_instances() {
        let checker =
            checked("fun render: <T of Display>(value: T) -> String = { value |> display }");
        let mut planner = ComponentSpecializationPlanner::new(&checker);
        planner
            .request_function(request(
                "test:pkg/root.render",
                "render",
                [("T", TypedType::String)],
            ))
            .unwrap();
        assert!(matches!(
            planner.request_function(request(
                "test:pkg/root.render",
                "render",
                [("T", TypedType::List(Box::new(TypedType::Int32)))],
            )),
            Err(ComponentSpecializationError::Validation(
                ExplicitFunctionSpecializationError::BoundNotSatisfied { .. }
            ))
        ));
    }

    #[test]
    fn associated_types_are_resolved_in_the_finalized_signature() {
        let checker = TypeChecker::new();
        let mut planner = ComponentSpecializationPlanner::new(&checker);
        let instance = planner
            .request_function(request(
                "restrict:std/prelude.map",
                "map",
                [
                    ("C", TypedType::List(Box::new(TypedType::Int32))),
                    ("U", TypedType::String),
                ],
            ))
            .unwrap();

        assert_eq!(
            instance.params,
            vec![
                (
                    "container".to_string(),
                    TypedType::List(Box::new(TypedType::Int32)),
                ),
                (
                    "mapper".to_string(),
                    TypedType::Function {
                        params: vec![TypedType::Int32],
                        return_type: Box::new(TypedType::String),
                    },
                ),
            ]
        );
        assert_eq!(
            instance.return_type,
            TypedType::List(Box::new(TypedType::String))
        );
    }

    #[test]
    fn identity_is_injective_and_identical_requests_dedupe() {
        let checker =
            checked("fun choose: <T, U>(left: T, right: U) -> Result<T, U> = { left Result::Ok }");
        let mut planner = ComponentSpecializationPlanner::new(&checker);
        let first = planner
            .request_function(request(
                "test:pkg/root.choose",
                "choose",
                [("T", TypedType::Int32), ("U", TypedType::String)],
            ))
            .unwrap();
        let duplicate = planner
            .request_function(request(
                "test:pkg/root.choose",
                "choose",
                [("U", TypedType::String), ("T", TypedType::Int32)],
            ))
            .unwrap();
        let reversed = planner
            .request_function(request(
                "test:pkg/root.choose",
                "choose",
                [("T", TypedType::String), ("U", TypedType::Int32)],
            ))
            .unwrap();

        assert_eq!(first, duplicate);
        assert_ne!(first.identity, reversed.identity);
        assert_ne!(first.core_symbol, reversed.core_symbol);
        assert_eq!(planner.len(), 2);
    }
}
