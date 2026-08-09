//! Bridge from schema-v2 Component selections to checked generic instances.
//!
//! This module consumes the manifest-neutral selection contract, resolves its
//! closed type expressions, and delegates all generic validation to the type
//! checker's explicit-specialization seam. It deliberately stops before CIR
//! construction and body code generation.

use crate::component_selection_v2::{
    ClosedComponentType, ComponentFunctionExportContract, ComponentSelectionContract,
};
use crate::component_specialization::{
    ComponentFunctionSpecialization, ComponentFunctionSpecializationRequest,
    ComponentSpecializationError, ComponentSpecializationPlanner,
};
use crate::type_checker::{ArrayLength, TypeChecker, TypedType};
use std::collections::BTreeMap;
use thiserror::Error;

/// Resolves root-nominal names after source/module resolution has established
/// their canonical declaration identity and arity.
///
/// Keeping this authority outside the manifest parser prevents a syntactically
/// valid name from being mistaken for an existing record or enum. The returned
/// type is still checked for closure, temporals, and generic bounds by the
/// explicit-specialization pass.
pub trait ComponentNamedTypeResolver {
    fn resolve_named_type(
        &self,
        name: &str,
        arguments: Vec<TypedType>,
    ) -> Result<TypedType, ComponentNamedTypeResolutionError>;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ComponentNamedTypeResolutionError {
    #[error("named type '{name}' was not found")]
    NotFound { name: String },
    #[error("named type '{name}' expects {expected} type argument(s), found {found}")]
    Arity {
        name: String,
        expected: usize,
        found: usize,
    },
    #[error("named type '{name}' cannot be used here: {reason}")]
    Unsupported { name: String, reason: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentGenericExportPlan {
    pub interface: String,
    pub external_name: String,
    pub source_function: String,
    pub specialization_identity: String,
    pub core_symbol: String,
}

/// Closed, deterministic set of compiler-owned generic instances plus their
/// external Component bindings.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ComponentGenericPlan {
    pub exports: Vec<ComponentGenericExportPlan>,
    pub specializations: Vec<ComponentFunctionSpecialization>,
}

#[derive(Debug, Error, PartialEq)]
pub enum ComponentGenericPlanError {
    #[error("{path}: selected function '{function}' was not found in the checked program")]
    FunctionNotFound { path: String, function: String },
    #[error("{path}.type-args.{parameter}: {error}")]
    NamedTypeResolution {
        path: String,
        parameter: String,
        #[source]
        error: ComponentNamedTypeResolutionError,
    },
    #[error(
        "{path}.type-args.{parameter}: Array length {length} does not fit this compiler target"
    )]
    ArrayLengthOutOfRange {
        path: String,
        parameter: String,
        length: u64,
    },
    #[error("{path}: {error}")]
    Specialization {
        path: String,
        #[source]
        error: ComponentSpecializationError,
    },
}

/// Plan every explicitly selected generic export in a normalized Component
/// contract. Ordinary non-generic exports are intentionally left for CIR
/// selection validation, while a generic export with no `type-args` reaches
/// the checked seam and receives its typed missing-argument diagnostic.
pub fn plan_component_generic_exports(
    selection: &ComponentSelectionContract,
    checker: &TypeChecker,
    named_types: &impl ComponentNamedTypeResolver,
) -> Result<ComponentGenericPlan, ComponentGenericPlanError> {
    let mut selected = selection.exports.iter().collect::<Vec<_>>();
    selected.sort_by(|left, right| export_sort_key(left).cmp(&export_sort_key(right)));

    let mut planner = ComponentSpecializationPlanner::new(checker);
    let mut exports = BTreeMap::new();

    for (index, export) in selected.into_iter().enumerate() {
        let path = format!("component.exports[{index}]");
        let signature = checker
            .checked_function_signature(&export.source_function)
            .ok_or_else(|| ComponentGenericPlanError::FunctionNotFound {
                path: path.clone(),
                function: export.source_function.clone(),
            })?;

        // This pass owns generic publication only. Supplying type arguments to
        // a non-generic function is still an error and is deliberately sent to
        // the checked specialization seam below.
        if signature.type_params.is_empty() && export.type_arguments.is_empty() {
            continue;
        }

        let mut type_arguments = BTreeMap::new();
        for argument in &export.type_arguments {
            let ty = resolve_closed_component_type(&argument.ty, named_types).map_err(|error| {
                match error {
                    ResolveClosedTypeError::Named(error) => {
                        ComponentGenericPlanError::NamedTypeResolution {
                            path: path.clone(),
                            parameter: argument.parameter.clone(),
                            error,
                        }
                    }
                    ResolveClosedTypeError::ArrayLengthOutOfRange(length) => {
                        ComponentGenericPlanError::ArrayLengthOutOfRange {
                            path: path.clone(),
                            parameter: argument.parameter.clone(),
                            length,
                        }
                    }
                }
            })?;
            type_arguments.insert(argument.parameter.clone(), ty);
        }

        let specialization = planner
            .request_function(ComponentFunctionSpecializationRequest {
                source_identity: root_source_identity(&selection.package, &export.source_function),
                source_function: export.source_function.clone(),
                type_arguments,
            })
            .map_err(|error| ComponentGenericPlanError::Specialization {
                path: path.clone(),
                error,
            })?;

        let binding = ComponentGenericExportPlan {
            interface: export.interface.clone(),
            external_name: export.external_name.clone(),
            source_function: export.source_function.clone(),
            specialization_identity: specialization.identity.clone(),
            core_symbol: specialization.core_symbol.clone(),
        };
        exports.insert(
            (
                binding.interface.clone(),
                binding.external_name.clone(),
                binding.specialization_identity.clone(),
            ),
            binding,
        );
    }

    Ok(ComponentGenericPlan {
        exports: exports.into_values().collect(),
        specializations: planner.instances().cloned().collect(),
    })
}

fn export_sort_key(
    export: &ComponentFunctionExportContract,
) -> (
    &str,
    &str,
    &str,
    &Vec<crate::component_selection_v2::ComponentTypeArgumentContract>,
) {
    (
        &export.interface,
        &export.external_name,
        &export.source_function,
        &export.type_arguments,
    )
}

fn root_source_identity(package: &str, source_function: &str) -> String {
    format!(
        "restrict-root-decl-v1$package{}x{}$name{}x{}",
        package.len(),
        package,
        source_function.len(),
        source_function
    )
}

enum ResolveClosedTypeError {
    Named(ComponentNamedTypeResolutionError),
    ArrayLengthOutOfRange(u64),
}

fn resolve_closed_component_type(
    ty: &ClosedComponentType,
    named_types: &impl ComponentNamedTypeResolver,
) -> Result<TypedType, ResolveClosedTypeError> {
    Ok(match ty {
        ClosedComponentType::Unit => TypedType::Unit,
        ClosedComponentType::Int32 => TypedType::Int32,
        ClosedComponentType::Int64 => TypedType::Int64,
        ClosedComponentType::Float64 => TypedType::Float64,
        ClosedComponentType::Boolean => TypedType::Boolean,
        ClosedComponentType::String => TypedType::String,
        ClosedComponentType::Char => TypedType::Char,
        ClosedComponentType::Named { name, arguments } => {
            let arguments = arguments
                .iter()
                .map(|argument| resolve_closed_component_type(argument, named_types))
                .collect::<Result<Vec<_>, _>>()?;
            named_types
                .resolve_named_type(name, arguments)
                .map_err(ResolveClosedTypeError::Named)?
        }
        ClosedComponentType::List(inner) => {
            TypedType::List(Box::new(resolve_closed_component_type(inner, named_types)?))
        }
        ClosedComponentType::Array { element, length } => {
            let length = usize::try_from(*length)
                .map_err(|_| ResolveClosedTypeError::ArrayLengthOutOfRange(*length))?;
            TypedType::Array(
                Box::new(resolve_closed_component_type(element, named_types)?),
                ArrayLength::Known(length),
            )
        }
        ClosedComponentType::Option(inner) => {
            TypedType::Option(Box::new(resolve_closed_component_type(inner, named_types)?))
        }
        ClosedComponentType::Result { ok, err } => TypedType::Result(
            Box::new(resolve_closed_component_type(ok, named_types)?),
            Box::new(resolve_closed_component_type(err, named_types)?),
        ),
        ClosedComponentType::Function { params, result } => TypedType::Function {
            params: params
                .iter()
                .map(|param| resolve_closed_component_type(param, named_types))
                .collect::<Result<Vec<_>, _>>()?,
            return_type: Box::new(resolve_closed_component_type(result, named_types)?),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_selection_v2::{
        ComponentTypeArgumentContract, COMPONENT_SELECTION_ABI_V2, COMPONENT_SELECTION_SCHEMA_V2,
    };
    use crate::parser::parse_program;
    use crate::type_checker::ExplicitFunctionSpecializationError;

    struct TestNamedTypes;

    impl ComponentNamedTypeResolver for TestNamedTypes {
        fn resolve_named_type(
            &self,
            name: &str,
            arguments: Vec<TypedType>,
        ) -> Result<TypedType, ComponentNamedTypeResolutionError> {
            match name {
                "Pair" if arguments.len() == 2 => Ok(TypedType::Record {
                    name: name.to_string(),
                    type_args: arguments,
                    frozen: false,
                    hash: None,
                    parent_hash: None,
                }),
                "Pair" => Err(ComponentNamedTypeResolutionError::Arity {
                    name: name.to_string(),
                    expected: 2,
                    found: arguments.len(),
                }),
                "Status" if arguments.is_empty() => Ok(TypedType::Enum {
                    name: name.to_string(),
                }),
                "Open" if arguments.is_empty() => {
                    Ok(TypedType::TypeParam("Unresolved".to_string()))
                }
                "Timed" if arguments.is_empty() => Ok(TypedType::Temporal {
                    base_type: Box::new(TypedType::Int32),
                    temporals: vec!["scope".to_string()],
                }),
                _ => Err(ComponentNamedTypeResolutionError::NotFound {
                    name: name.to_string(),
                }),
            }
        }
    }

    fn checked(source: &str) -> TypeChecker {
        let (remaining, program) = parse_program(source).expect("source should parse");
        assert!(remaining.trim().is_empty());
        let mut checker = TypeChecker::new();
        checker
            .check_program(&program)
            .expect("source should type check");
        checker
    }

    fn export(
        source_function: &str,
        external_name: &str,
        type_arguments: impl IntoIterator<Item = (&'static str, ClosedComponentType)>,
    ) -> ComponentFunctionExportContract {
        ComponentFunctionExportContract {
            interface: "api".to_string(),
            source_function: source_function.to_string(),
            external_name: external_name.to_string(),
            type_arguments: type_arguments
                .into_iter()
                .map(|(parameter, ty)| ComponentTypeArgumentContract {
                    parameter: parameter.to_string(),
                    ty,
                })
                .collect(),
        }
    }

    fn contract(exports: Vec<ComponentFunctionExportContract>) -> ComponentSelectionContract {
        ComponentSelectionContract {
            schema_version: COMPONENT_SELECTION_SCHEMA_V2,
            package: "test:generic@0.1.0".to_string(),
            world: "generic".to_string(),
            abi_profile: COMPONENT_SELECTION_ABI_V2.to_string(),
            exports,
            resources: Vec::new(),
            callables: Vec::new(),
        }
    }

    #[test]
    fn plans_closed_instance_with_deterministic_symbol() {
        let checker = checked("fun identity: <T>(value: T) -> T = { value }");
        let selection = contract(vec![export(
            "identity",
            "identity-string",
            [("T", ClosedComponentType::String)],
        )]);

        let first = plan_component_generic_exports(&selection, &checker, &TestNamedTypes).unwrap();
        let second = plan_component_generic_exports(&selection, &checker, &TestNamedTypes).unwrap();

        assert_eq!(first, second);
        assert_eq!(first.exports.len(), 1);
        assert_eq!(first.specializations.len(), 1);
        assert_eq!(first.specializations[0].params[0].1, TypedType::String);
        assert_eq!(first.specializations[0].return_type, TypedType::String);
        assert_eq!(
            first.exports[0].core_symbol,
            first.specializations[0].core_symbol
        );
    }

    #[test]
    fn skips_plain_exports_and_deduplicates_identical_generic_selections() {
        let checker = checked(
            "fun plain: (value: Int32) -> Int32 = { value }\nfun identity: <T>(value: T) -> T = { value }",
        );
        let generic = export(
            "identity",
            "identity-string",
            [("T", ClosedComponentType::String)],
        );
        let selection = contract(vec![generic.clone(), export("plain", "plain", []), generic]);

        let plan = plan_component_generic_exports(&selection, &checker, &TestNamedTypes).unwrap();

        assert_eq!(plan.exports.len(), 1);
        assert_eq!(plan.specializations.len(), 1);
    }

    #[test]
    fn restores_declaration_order_and_resolves_nested_closed_types() {
        let checker =
            checked("fun choose: <T, U>(left: T, right: U) -> Result<T, U> = { left Result::Ok }");
        let nested = ClosedComponentType::Option(Box::new(ClosedComponentType::Array {
            element: Box::new(ClosedComponentType::String),
            length: 4,
        }));
        let pair = ClosedComponentType::Named {
            name: "Pair".to_string(),
            arguments: vec![ClosedComponentType::Int32, nested.clone()],
        };
        let selection = contract(vec![export(
            "choose",
            "choose-pair",
            [("U", nested), ("T", pair)],
        )]);

        let plan = plan_component_generic_exports(&selection, &checker, &TestNamedTypes).unwrap();

        assert_eq!(plan.specializations[0].type_arguments[0].0, "T");
        assert_eq!(plan.specializations[0].type_arguments[1].0, "U");
        assert!(matches!(
            plan.specializations[0].type_arguments[0].1,
            TypedType::Record { ref name, .. } if name == "Pair"
        ));
    }

    #[test]
    fn reports_missing_and_extra_arguments_from_checked_seam() {
        let checker = checked("fun identity: <T>(value: T) -> T = { value }");
        let missing = contract(vec![export("identity", "missing", [])]);
        assert!(matches!(
            plan_component_generic_exports(&missing, &checker, &TestNamedTypes),
            Err(ComponentGenericPlanError::Specialization {
                error: ComponentSpecializationError::Validation(
                    ExplicitFunctionSpecializationError::MissingTypeArguments { .. }
                ),
                ..
            })
        ));

        let extra = contract(vec![export(
            "identity",
            "extra",
            [
                ("T", ClosedComponentType::Int32),
                ("U", ClosedComponentType::String),
            ],
        )]);
        assert!(matches!(
            plan_component_generic_exports(&extra, &checker, &TestNamedTypes),
            Err(ComponentGenericPlanError::Specialization {
                error: ComponentSpecializationError::Validation(
                    ExplicitFunctionSpecializationError::ExtraTypeArguments { .. }
                ),
                ..
            })
        ));

        let checker = checked("fun plain: (value: Int32) -> Int32 = { value }");
        let non_generic = contract(vec![export(
            "plain",
            "plain-string",
            [("T", ClosedComponentType::String)],
        )]);
        assert!(matches!(
            plan_component_generic_exports(&non_generic, &checker, &TestNamedTypes),
            Err(ComponentGenericPlanError::Specialization {
                error: ComponentSpecializationError::Validation(
                    ExplicitFunctionSpecializationError::FunctionIsNotGeneric(_)
                ),
                ..
            })
        ));
    }

    #[test]
    fn reports_open_and_temporal_arguments_from_checked_seam() {
        let checker = checked("fun identity: <T>(value: T) -> T = { value }");
        for (name, expected_temporal) in [("Open", false), ("Timed", true)] {
            let selection = contract(vec![export(
                "identity",
                "invalid",
                [(
                    "T",
                    ClosedComponentType::Named {
                        name: name.to_string(),
                        arguments: Vec::new(),
                    },
                )],
            )]);
            let error = plan_component_generic_exports(&selection, &checker, &TestNamedTypes)
                .expect_err("argument should be rejected");
            assert!(match error {
                ComponentGenericPlanError::Specialization {
                    error:
                        ComponentSpecializationError::Validation(
                            ExplicitFunctionSpecializationError::TemporalTypeArgument { .. },
                        ),
                    ..
                } => expected_temporal,
                ComponentGenericPlanError::Specialization {
                    error:
                        ComponentSpecializationError::Validation(
                            ExplicitFunctionSpecializationError::OpenTypeArgument { .. },
                        ),
                    ..
                } => !expected_temporal,
                _ => false,
            });
        }
    }

    #[test]
    fn reports_nominal_resolution_before_specialization() {
        let checker = checked("fun identity: <T>(value: T) -> T = { value }");
        let selection = contract(vec![export(
            "identity",
            "unknown",
            [(
                "T",
                ClosedComponentType::Named {
                    name: "Missing".to_string(),
                    arguments: Vec::new(),
                },
            )],
        )]);

        assert!(matches!(
            plan_component_generic_exports(&selection, &checker, &TestNamedTypes),
            Err(ComponentGenericPlanError::NamedTypeResolution {
                error: ComponentNamedTypeResolutionError::NotFound { .. },
                ..
            })
        ));
    }
}
