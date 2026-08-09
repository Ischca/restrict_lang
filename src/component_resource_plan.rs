//! Checked schema-v2 planning for synchronous Component resources and callables.
//!
//! This module is the compiler-owned seam between manifest-neutral declarations
//! and Component Interface IR/adapter generation. It deliberately carries only
//! source identities, logical checked types, ownership facts, and stable WIT
//! identities. Core Wasm addresses, table indexes, arena offsets, and allocator
//! details are not part of this plan.

use crate::ast::{
    BlockExpr, EnumDecl, Expr, ExprKind, FieldInit, FunDecl, PipeTarget, Program, RecordDecl, Stmt,
    TopDecl,
};
use crate::component_selection_v2::{
    ClosedComponentType, ComponentCallableMultiplicity, ComponentResourceReceiver,
    ComponentSelectionContract, ComponentSourceVisibility, COMPONENT_SELECTION_ABI_V2,
    COMPONENT_SELECTION_SCHEMA_V2,
};
use crate::ir::builder::{CheckedFunctionIr, CheckedProgramIr};
use crate::type_checker::{format_typed_type, ArrayLength, TypedType};
use std::collections::BTreeMap;
use thiserror::Error;

/// Root declarations captured before module resolution flattens imports.
///
/// Resource declarations are root-only in schema v2. Retaining this catalog
/// prevents an imported public record or function from satisfying a local
/// resource contract by name.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentResourceRootCatalog {
    records: BTreeMap<String, RootRecord>,
    enums: BTreeMap<String, RootEnum>,
    functions: BTreeMap<String, RootFunction>,
}

#[derive(Debug, Clone, PartialEq)]
struct RootRecord {
    declaration: RecordDecl,
    visibility: ComponentSourceVisibility,
}

#[derive(Debug, Clone, PartialEq)]
struct RootEnum {
    declaration: EnumDecl,
    visibility: ComponentSourceVisibility,
}

#[derive(Debug, Clone, PartialEq)]
struct RootFunction {
    declaration: FunDecl,
    visibility: ComponentSourceVisibility,
}

impl ComponentResourceRootCatalog {
    pub fn from_unresolved_root(program: &Program) -> Result<Self, ComponentResourcePlanError> {
        let mut catalog = Self {
            records: BTreeMap::new(),
            enums: BTreeMap::new(),
            functions: BTreeMap::new(),
        };
        for declaration in &program.declarations {
            catalog.insert_declaration(declaration, ComponentSourceVisibility::Private)?;
        }
        Ok(catalog)
    }

    fn insert_declaration(
        &mut self,
        declaration: &TopDecl,
        visibility: ComponentSourceVisibility,
    ) -> Result<(), ComponentResourcePlanError> {
        if let TopDecl::Export(export) = declaration {
            return self.insert_declaration(&export.item, ComponentSourceVisibility::Public);
        }
        match declaration {
            TopDecl::Record(record) => {
                if self
                    .records
                    .insert(
                        record.name.clone(),
                        RootRecord {
                            declaration: record.clone(),
                            visibility,
                        },
                    )
                    .is_some()
                {
                    return Err(ComponentResourcePlanError::DuplicateRootDeclaration {
                        kind: "record",
                        name: record.name.clone(),
                    });
                }
            }
            TopDecl::Enum(enumeration) => {
                if self
                    .enums
                    .insert(
                        enumeration.name.clone(),
                        RootEnum {
                            declaration: enumeration.clone(),
                            visibility,
                        },
                    )
                    .is_some()
                {
                    return Err(ComponentResourcePlanError::DuplicateRootDeclaration {
                        kind: "enum",
                        name: enumeration.name.clone(),
                    });
                }
            }
            TopDecl::Function(function) => {
                if self
                    .functions
                    .insert(
                        function.name.clone(),
                        RootFunction {
                            declaration: function.clone(),
                            visibility,
                        },
                    )
                    .is_some()
                {
                    return Err(ComponentResourcePlanError::DuplicateRootDeclaration {
                        kind: "function",
                        name: function.name.clone(),
                    });
                }
            }
            TopDecl::Export(_) => unreachable!("exports are unwrapped above"),
            TopDecl::Form(_)
            | TopDecl::Takes(_)
            | TopDecl::Impl(_)
            | TopDecl::Context(_)
            | TopDecl::Binding(_) => {}
        }
        Ok(())
    }

    fn resolve_closed_type(
        &self,
        ty: &ClosedComponentType,
        path: &str,
    ) -> Result<TypedType, ComponentResourcePlanError> {
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
                    .enumerate()
                    .map(|(index, argument)| {
                        self.resolve_closed_type(argument, &format!("{path}.arguments[{index}]"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if let Some(record) = self.records.get(name) {
                    let expected = record.declaration.type_params.len();
                    if arguments.len() != expected {
                        return Err(ComponentResourcePlanError::NamedTypeArity {
                            path: path.to_string(),
                            name: name.clone(),
                            expected,
                            found: arguments.len(),
                        });
                    }
                    TypedType::Record {
                        name: name.clone(),
                        type_args: arguments,
                        frozen: record.declaration.frozen,
                        hash: None,
                        parent_hash: record.declaration.parent_hash.clone(),
                    }
                } else if self.enums.contains_key(name) {
                    if !arguments.is_empty() {
                        return Err(ComponentResourcePlanError::NamedTypeArity {
                            path: path.to_string(),
                            name: name.clone(),
                            expected: 0,
                            found: arguments.len(),
                        });
                    }
                    TypedType::Enum { name: name.clone() }
                } else {
                    return Err(ComponentResourcePlanError::NamedTypeNotFound {
                        path: path.to_string(),
                        name: name.clone(),
                    });
                }
            }
            ClosedComponentType::List(inner) => TypedType::List(Box::new(
                self.resolve_closed_type(inner, &format!("{path}.element"))?,
            )),
            ClosedComponentType::Array { element, length } => {
                let length = usize::try_from(*length).map_err(|_| {
                    ComponentResourcePlanError::ArrayLengthOutOfRange {
                        path: path.to_string(),
                        length: *length,
                    }
                })?;
                TypedType::Array(
                    Box::new(self.resolve_closed_type(element, &format!("{path}.element"))?),
                    ArrayLength::Known(length),
                )
            }
            ClosedComponentType::Option(inner) => TypedType::Option(Box::new(
                self.resolve_closed_type(inner, &format!("{path}.some"))?,
            )),
            ClosedComponentType::Result { ok, err } => TypedType::Result(
                Box::new(self.resolve_closed_type(ok, &format!("{path}.ok"))?),
                Box::new(self.resolve_closed_type(err, &format!("{path}.err"))?),
            ),
            ClosedComponentType::Function { params, result } => TypedType::Function {
                params: params
                    .iter()
                    .enumerate()
                    .map(|(index, param)| {
                        self.resolve_closed_type(param, &format!("{path}.params[{index}]"))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                return_type: Box::new(self.resolve_closed_type(result, &format!("{path}.result"))?),
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlannedComponentResourceIdentity {
    pub interface: String,
    pub external_name: String,
    pub source_record: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlannedComponentCallableIdentity {
    pub interface: String,
    pub external_name: String,
}

/// Logical type at one generated WIT boundary position.
///
/// A direct resource or callable value is always an owned handle outside an
/// implicit borrowed resource receiver. Nested handles are rejected because
/// v0.1 does not define ownership transfer through aggregate value shapes.
#[derive(Debug, Clone, PartialEq)]
pub enum PlannedComponentBoundaryType {
    Value(TypedType),
    OwnResource(PlannedComponentResourceIdentity),
    OwnCallable(PlannedComponentCallableIdentity),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedComponentParam {
    pub source_index: usize,
    pub source_name: String,
    pub ty: PlannedComponentBoundaryType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedComponentExport {
    pub interface: String,
    pub external_name: String,
    pub source_function: String,
    pub params: Vec<PlannedComponentParam>,
    pub result: PlannedComponentBoundaryType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedComponentDestructor {
    pub source_function: String,
    pub visibility: ComponentSourceVisibility,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedComponentResourceMethod {
    pub source_function: String,
    pub visibility: ComponentSourceVisibility,
    pub external_name: String,
    pub receiver: ComponentResourceReceiver,
    pub receiver_source_name: String,
    /// Checked parameters after the implicit/explicit receiver.
    pub params: Vec<PlannedComponentParam>,
    pub result: PlannedComponentBoundaryType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedComponentResource {
    pub identity: PlannedComponentResourceIdentity,
    pub source_type: TypedType,
    pub destructor: PlannedComponentDestructor,
    pub methods: Vec<PlannedComponentResourceMethod>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedComponentCallable {
    pub identity: PlannedComponentCallableIdentity,
    pub source_type: TypedType,
    pub params: Vec<PlannedComponentParam>,
    pub result: PlannedComponentBoundaryType,
    pub multiplicity: ComponentCallableMultiplicity,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ComponentResourcePlan {
    pub resources: Vec<PlannedComponentResource>,
    pub callables: Vec<PlannedComponentCallable>,
    pub exports: Vec<PlannedComponentExport>,
}

impl ComponentResourcePlan {
    pub fn resource_for_source_type(&self, ty: &TypedType) -> Option<&PlannedComponentResource> {
        self.resources
            .iter()
            .find(|resource| same_component_type(&resource.source_type, ty))
    }

    pub fn callable_for_source_type(&self, ty: &TypedType) -> Option<&PlannedComponentCallable> {
        self.callables
            .iter()
            .find(|callable| same_component_type(&callable.source_type, ty))
    }

    pub fn export(&self, interface: &str, external_name: &str) -> Option<&PlannedComponentExport> {
        self.exports
            .iter()
            .find(|export| export.interface == interface && export.external_name == external_name)
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ComponentResourcePlanError {
    #[error("schema-v2 resource planning requires schema {expected}, found {found}")]
    UnsupportedSchema { expected: u32, found: u32 },
    #[error("resource planning requires ABI '{expected}', found '{found}'")]
    UnsupportedAbi {
        expected: &'static str,
        found: String,
    },
    #[error("checked Restrict IR was modified or is internally invalid")]
    InvalidCheckedIr,
    #[error("checked Restrict IR does not belong to the resolved source program")]
    CheckedSourceProgramMismatch,
    #[error("duplicate root {kind} declaration '{name}'")]
    DuplicateRootDeclaration { kind: &'static str, name: String },
    #[error("resource '{path}' source record '{source_record}' is not a root declaration")]
    ResourceRecordNotFound { path: String, source_record: String },
    #[error("resource '{path}' source record '{source_record}' must be public")]
    ResourceRecordNotPublic { path: String, source_record: String },
    #[error(
        "resource '{path}' source record '{source_record}' must be non-generic and non-temporal"
    )]
    OpenResourceRecord { path: String, source_record: String },
    #[error("{path}: root function '{function}' was not found")]
    FunctionNotFound { path: String, function: String },
    #[error("{path}: function '{function}' visibility is {actual:?}, expected {expected:?}")]
    FunctionVisibility {
        path: String,
        function: String,
        expected: ComponentSourceVisibility,
        actual: ComponentSourceVisibility,
    },
    #[error("{path}: root declaration '{name}' no longer matches the resolved source program")]
    ResolvedRootDeclarationMismatch { path: String, name: String },
    #[error("{path}: checked function metadata is missing for '{function}'")]
    CheckedFunctionNotFound { path: String, function: String },
    #[error("{path}: checked visibility for function '{function}' is stale")]
    CheckedFunctionVisibility { path: String, function: String },
    #[error("{path}: async function '{function}' is outside canonical-v1-sync")]
    AsyncFunctionUnsupported { path: String, function: String },
    #[error("{path}: function '{function}' must be a concrete non-temporal root function")]
    OpenFunctionUnsupported { path: String, function: String },
    #[error("{path}: destructor '{function}' must have signature ({resource}) -> ()")]
    InvalidDestructorSignature {
        path: String,
        function: String,
        resource: String,
    },
    #[error("{path}: method '{function}' must take {resource} as its first receiver parameter")]
    InvalidMethodReceiver {
        path: String,
        function: String,
        resource: String,
    },
    #[error(
        "{path}: borrow receiver '{receiver}' escapes a field-read position in method '{function}'"
    )]
    BorrowReceiverEscapes {
        path: String,
        function: String,
        receiver: String,
    },
    #[error(
        "{path}: terminal own method '{function}' cannot return its consumed resource '{resource}'"
    )]
    OwnMethodReturnsConsumedResource {
        path: String,
        function: String,
        resource: String,
    },
    #[error("{path}: resource method '{function}' cannot return an owned resource/callable in the initial synchronous adapter")]
    ResourceMethodReturnsHandle { path: String, function: String },
    #[error("{path}: once callable cannot return an owned resource/callable in the initial synchronous adapter")]
    CallableReturnsHandle { path: String },
    #[error("{path}: a resource/callable-producing export cannot consume another owned handle in the initial synchronous adapter")]
    HandleProducerConsumesHandle { path: String },
    #[error("{path}: free Component exports cannot consume an owned resource/callable in the initial synchronous adapter; expose a resource method instead")]
    FreeExportConsumesHandle { path: String },
    #[error("{path}: resource method parameters after the receiver cannot consume an owned resource/callable in the initial synchronous adapter")]
    ResourceMethodHandleParam { path: String },
    #[error("{path}: once callable parameters cannot consume an owned resource/callable in the initial synchronous adapter")]
    CallableHandleParam { path: String },
    #[error("{path}: callable multiplicity 'many' is outside synchronous v0.1")]
    UnsupportedCallableMultiplicity { path: String },
    #[error("{path}: named type '{name}' was not found in the root program")]
    NamedTypeNotFound { path: String, name: String },
    #[error("{path}: named type '{name}' expects {expected} argument(s), found {found}")]
    NamedTypeArity {
        path: String,
        name: String,
        expected: usize,
        found: usize,
    },
    #[error("{path}: Array length {length} does not fit this compiler target")]
    ArrayLengthOutOfRange { path: String, length: u64 },
    #[error(
        "{path}: callable signature '{signature}' is declared by more than one callable resource"
    )]
    AmbiguousCallableSignature { path: String, signature: String },
    #[error("{path}: function value '{signature}' has no matching schema-v2 callable declaration")]
    UndeclaredCallableSignature { path: String, signature: String },
    #[error("{path}: resource/callable handles nested inside '{ty}' are outside the v0.1 ownership contract")]
    NestedHandleUnsupported { path: String, ty: String },
    #[error("{path}: type '{ty}' is outside the synchronous Component boundary contract")]
    UnsupportedBoundaryType { path: String, ty: String },
}

/// Validate and deterministically plan every schema-v2 resource/callable fact.
pub fn plan_component_resources(
    root: &ComponentResourceRootCatalog,
    resolved_program: &Program,
    checked_program: &CheckedProgramIr,
    selection: &ComponentSelectionContract,
) -> Result<ComponentResourcePlan, ComponentResourcePlanError> {
    if selection.schema_version != COMPONENT_SELECTION_SCHEMA_V2 {
        return Err(ComponentResourcePlanError::UnsupportedSchema {
            expected: COMPONENT_SELECTION_SCHEMA_V2,
            found: selection.schema_version,
        });
    }
    if selection.abi_profile != COMPONENT_SELECTION_ABI_V2 {
        return Err(ComponentResourcePlanError::UnsupportedAbi {
            expected: COMPONENT_SELECTION_ABI_V2,
            found: selection.abi_profile.clone(),
        });
    }
    if !checked_program.is_unmodified() || checked_program.validate_lowering_summaries().is_err() {
        return Err(ComponentResourcePlanError::InvalidCheckedIr);
    }
    if !checked_program.matches_source_program(resolved_program) {
        return Err(ComponentResourcePlanError::CheckedSourceProgramMismatch);
    }

    let checked_by_name = checked_program
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect::<BTreeMap<_, _>>();

    let mut resources = Vec::with_capacity(selection.resources.len());
    for (index, contract) in selection.resources.iter().enumerate() {
        let path = format!("component.resources[{index}]");
        let record = root.records.get(&contract.source_record).ok_or_else(|| {
            ComponentResourcePlanError::ResourceRecordNotFound {
                path: path.clone(),
                source_record: contract.source_record.clone(),
            }
        })?;
        if record.visibility != ComponentSourceVisibility::Public {
            return Err(ComponentResourcePlanError::ResourceRecordNotPublic {
                path,
                source_record: contract.source_record.clone(),
            });
        }
        if !record.declaration.type_params.is_empty()
            || !record.declaration.temporal_constraints.is_empty()
        {
            return Err(ComponentResourcePlanError::OpenResourceRecord {
                path,
                source_record: contract.source_record.clone(),
            });
        }
        ensure_resolved_record(root, resolved_program, &contract.source_record, &path)?;
        let source_type = TypedType::Record {
            name: contract.source_record.clone(),
            type_args: Vec::new(),
            frozen: record.declaration.frozen,
            hash: None,
            parent_hash: record.declaration.parent_hash.clone(),
        };

        let destructor_path = format!("{path}.destructor");
        let (destructor_source, destructor_checked) = resolve_root_function(
            root,
            resolved_program,
            &checked_by_name,
            &destructor_path,
            &contract.destructor.source_function,
            Some(contract.destructor.visibility),
        )?;
        ensure_sync_concrete_function(destructor_source, destructor_checked, &destructor_path)?;
        if destructor_checked.params.len() != 1
            || !same_component_type(
                destructor_checked.params[0].final_type.as_typed_type(),
                &source_type,
            )
            || destructor_checked.return_type.as_typed_type() != &TypedType::Unit
        {
            return Err(ComponentResourcePlanError::InvalidDestructorSignature {
                path: destructor_path,
                function: contract.destructor.source_function.clone(),
                resource: contract.source_record.clone(),
            });
        }

        resources.push(PlannedComponentResource {
            identity: PlannedComponentResourceIdentity {
                interface: contract.interface.clone(),
                external_name: contract.external_name.clone(),
                source_record: contract.source_record.clone(),
            },
            source_type,
            destructor: PlannedComponentDestructor {
                source_function: contract.destructor.source_function.clone(),
                visibility: contract.destructor.visibility,
            },
            methods: Vec::with_capacity(contract.methods.len()),
        });
    }

    let mut callables = Vec::with_capacity(selection.callables.len());
    for (index, contract) in selection.callables.iter().enumerate() {
        let path = format!("component.callables[{index}]");
        if contract.multiplicity != ComponentCallableMultiplicity::Once {
            return Err(ComponentResourcePlanError::UnsupportedCallableMultiplicity { path });
        }
        let params = contract
            .params
            .iter()
            .enumerate()
            .map(|(param_index, param)| {
                root.resolve_closed_type(&param.ty, &format!("{path}.params[{param_index}].type"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let result = root.resolve_closed_type(&contract.result, &format!("{path}.result"))?;
        let source_type = TypedType::Function {
            params,
            return_type: Box::new(result),
        };
        if let Some(existing) = callables
            .iter()
            .find(|existing: &&PlannedComponentCallable| {
                same_component_type(&existing.source_type, &source_type)
            })
        {
            return Err(ComponentResourcePlanError::AmbiguousCallableSignature {
                path,
                signature: format!(
                    "{} (also declared as {}.{})",
                    format_typed_type(&source_type),
                    existing.identity.interface,
                    existing.identity.external_name
                ),
            });
        }
        callables.push(PlannedComponentCallable {
            identity: PlannedComponentCallableIdentity {
                interface: contract.interface.clone(),
                external_name: contract.external_name.clone(),
            },
            source_type,
            params: Vec::with_capacity(contract.params.len()),
            result: PlannedComponentBoundaryType::Value(TypedType::Unit),
            multiplicity: contract.multiplicity,
        });
    }

    // Once all handle identities exist, classify callable call signatures.
    for (index, callable) in callables.clone().iter().enumerate() {
        let contract = &selection.callables[index];
        let TypedType::Function {
            params,
            return_type,
        } = &callable.source_type
        else {
            unreachable!("callable source type is constructed above")
        };
        let planned_params = params
            .iter()
            .zip(&contract.params)
            .enumerate()
            .map(|(param_index, (ty, param))| {
                Ok(PlannedComponentParam {
                    source_index: param_index,
                    source_name: param.name.clone(),
                    ty: classify_boundary_type(
                        ty,
                        &resources,
                        &callables,
                        &format!("component.callables[{index}].params[{param_index}]"),
                    )?,
                })
            })
            .collect::<Result<Vec<_>, ComponentResourcePlanError>>()?;
        if planned_params
            .iter()
            .any(|param| is_owned_handle(&param.ty))
        {
            return Err(ComponentResourcePlanError::CallableHandleParam {
                path: format!("component.callables[{index}].params"),
            });
        }
        let result = classify_boundary_type(
            return_type,
            &resources,
            &callables,
            &format!("component.callables[{index}].result"),
        )?;
        if is_owned_handle(&result) {
            return Err(ComponentResourcePlanError::CallableReturnsHandle {
                path: format!("component.callables[{index}].result"),
            });
        }
        callables[index].params = planned_params;
        callables[index].result = result;
    }

    // Methods are classified after callable identities so every nested or
    // additional handle parameter can be rejected with its precise identity.
    for (resource_index, contract) in selection.resources.iter().enumerate() {
        let resource_type = resources[resource_index].source_type.clone();
        let mut methods = Vec::with_capacity(contract.methods.len());
        for (method_index, method) in contract.methods.iter().enumerate() {
            let path = format!("component.resources[{resource_index}].methods[{method_index}]");
            let (source, checked) = resolve_root_function(
                root,
                resolved_program,
                &checked_by_name,
                &path,
                &method.source_function,
                None,
            )?;
            ensure_sync_concrete_function(source, checked, &path)?;
            let Some(receiver) = checked.params.first() else {
                return Err(ComponentResourcePlanError::InvalidMethodReceiver {
                    path,
                    function: method.source_function.clone(),
                    resource: contract.source_record.clone(),
                });
            };
            if !same_component_type(receiver.final_type.as_typed_type(), &resource_type) {
                return Err(ComponentResourcePlanError::InvalidMethodReceiver {
                    path,
                    function: method.source_function.clone(),
                    resource: contract.source_record.clone(),
                });
            }
            if method.receiver == ComponentResourceReceiver::Borrow
                && !borrow_receiver_is_field_read_only(&source.body, &source.params[0].name)
            {
                return Err(ComponentResourcePlanError::BorrowReceiverEscapes {
                    path,
                    function: method.source_function.clone(),
                    receiver: source.params[0].name.clone(),
                });
            }
            if method.receiver == ComponentResourceReceiver::Own
                && contains_resource_type(
                    checked.return_type.as_typed_type(),
                    &resources[resource_index].identity.source_record,
                )
            {
                return Err(
                    ComponentResourcePlanError::OwnMethodReturnsConsumedResource {
                        path,
                        function: method.source_function.clone(),
                        resource: contract.source_record.clone(),
                    },
                );
            }
            let params = checked
                .params
                .iter()
                .enumerate()
                .skip(1)
                .map(|(source_index, param)| {
                    Ok(PlannedComponentParam {
                        source_index,
                        source_name: param.name.clone(),
                        ty: classify_boundary_type(
                            param.final_type.as_typed_type(),
                            &resources,
                            &callables,
                            &format!("{path}.params[{source_index}]"),
                        )?,
                    })
                })
                .collect::<Result<Vec<_>, ComponentResourcePlanError>>()?;
            if params.iter().any(|param| is_owned_handle(&param.ty)) {
                return Err(ComponentResourcePlanError::ResourceMethodHandleParam {
                    path: format!("{path}.params"),
                });
            }
            let result = classify_boundary_type(
                checked.return_type.as_typed_type(),
                &resources,
                &callables,
                &format!("{path}.result"),
            )?;
            if is_owned_handle(&result) {
                return Err(ComponentResourcePlanError::ResourceMethodReturnsHandle {
                    path,
                    function: method.source_function.clone(),
                });
            }
            methods.push(PlannedComponentResourceMethod {
                source_function: method.source_function.clone(),
                visibility: root
                    .functions
                    .get(&method.source_function)
                    .expect("resolved root method must remain catalogued")
                    .visibility,
                external_name: method.external_name.clone(),
                receiver: method.receiver,
                receiver_source_name: receiver.name.clone(),
                params,
                result,
            });
        }
        resources[resource_index].methods = methods;
    }

    let mut exports = Vec::new();
    for (index, export) in selection.exports.iter().enumerate() {
        // Explicit generic instances are classified from their specialization
        // plan by the integration layer. This planner never substitutes an
        // open checked signature or guesses a call-site instantiation.
        if !export.type_arguments.is_empty() {
            continue;
        }
        let path = format!("component.exports[{index}]");
        let (_, checked) = resolve_root_function(
            root,
            resolved_program,
            &checked_by_name,
            &path,
            &export.source_function,
            Some(ComponentSourceVisibility::Public),
        )?;
        if !checked.monomorphic {
            continue;
        }
        let params = checked
            .params
            .iter()
            .enumerate()
            .map(|(source_index, param)| {
                Ok(PlannedComponentParam {
                    source_index,
                    source_name: param.name.clone(),
                    ty: classify_boundary_type(
                        param.final_type.as_typed_type(),
                        &resources,
                        &callables,
                        &format!("{path}.params[{source_index}]"),
                    )?,
                })
            })
            .collect::<Result<Vec<_>, ComponentResourcePlanError>>()?;
        let result = classify_boundary_type(
            checked.return_type.as_typed_type(),
            &resources,
            &callables,
            &format!("{path}.result"),
        )?;
        if params.iter().any(|param| is_owned_handle(&param.ty)) {
            if is_owned_handle(&result) {
                return Err(ComponentResourcePlanError::HandleProducerConsumesHandle { path });
            }
            return Err(ComponentResourcePlanError::FreeExportConsumesHandle { path });
        }
        exports.push(PlannedComponentExport {
            interface: export.interface.clone(),
            external_name: export.external_name.clone(),
            source_function: export.source_function.clone(),
            params,
            result,
        });
    }

    resources.sort_by(|left, right| left.identity.cmp(&right.identity));
    callables.sort_by(|left, right| left.identity.cmp(&right.identity));
    exports.sort_by(|left, right| {
        (&left.interface, &left.external_name, &left.source_function).cmp(&(
            &right.interface,
            &right.external_name,
            &right.source_function,
        ))
    });
    Ok(ComponentResourcePlan {
        resources,
        callables,
        exports,
    })
}

fn is_owned_handle(ty: &PlannedComponentBoundaryType) -> bool {
    matches!(
        ty,
        PlannedComponentBoundaryType::OwnResource(_) | PlannedComponentBoundaryType::OwnCallable(_)
    )
}

/// Schema-v2 `borrow` is a compiler-owned loan until Restrict gains a source
/// borrow type. The selected source function may inspect fields, but it may not
/// pass, return, clone, freeze, match, capture, or otherwise move the receiver
/// itself. This conservative source check prevents a manifest from relabeling
/// an arbitrary affine-consuming function as a reusable WIT borrow method.
fn borrow_receiver_is_field_read_only(block: &BlockExpr, receiver: &str) -> bool {
    block.statements.iter().all(|statement| match statement {
        Stmt::Binding(binding) => borrow_receiver_expr_is_safe(&binding.value, receiver, false),
        Stmt::Assignment(assignment) => {
            borrow_receiver_expr_is_safe(&assignment.value, receiver, false)
        }
        Stmt::Expr(expr) => borrow_receiver_expr_is_safe(expr, receiver, false),
    }) && block
        .expr
        .as_deref()
        .is_none_or(|expr| borrow_receiver_expr_is_safe(expr, receiver, false))
}

fn borrow_receiver_fields_are_safe(fields: &[FieldInit], receiver: &str) -> bool {
    fields.iter().all(|field| match field {
        FieldInit::Field { value, .. } => borrow_receiver_expr_is_safe(value, receiver, false),
        FieldInit::Spread(expr) => borrow_receiver_expr_is_safe(expr, receiver, false),
    })
}

fn borrow_receiver_expr_is_safe(expr: &Expr, receiver: &str, field_base: bool) -> bool {
    match &expr.kind {
        ExprKind::Ident(name) => name != receiver || field_base,
        ExprKind::RecordLit(record) => borrow_receiver_fields_are_safe(&record.fields, receiver),
        ExprKind::Clone(clone) => {
            borrow_receiver_expr_is_safe(&clone.base, receiver, false)
                && borrow_receiver_fields_are_safe(&clone.updates.fields, receiver)
        }
        ExprKind::PrototypeClone(clone) => {
            borrow_receiver_fields_are_safe(&clone.updates.fields, receiver)
        }
        ExprKind::Then(then) => {
            borrow_receiver_expr_is_safe(&then.condition, receiver, false)
                && borrow_receiver_is_field_read_only(&then.then_block, receiver)
                && then.else_ifs.iter().all(|(condition, block)| {
                    borrow_receiver_expr_is_safe(condition, receiver, false)
                        && borrow_receiver_is_field_read_only(block, receiver)
                })
                && then
                    .else_block
                    .as_ref()
                    .is_none_or(|block| borrow_receiver_is_field_read_only(block, receiver))
        }
        ExprKind::While(while_expr) => {
            borrow_receiver_expr_is_safe(&while_expr.condition, receiver, false)
                && borrow_receiver_is_field_read_only(&while_expr.body, receiver)
        }
        ExprKind::Match(match_expr) => {
            borrow_receiver_expr_is_safe(&match_expr.expr, receiver, false)
                && match_expr
                    .arms
                    .iter()
                    .all(|arm| borrow_receiver_is_field_read_only(&arm.body, receiver))
        }
        ExprKind::Call(call) => {
            borrow_receiver_expr_is_safe(&call.function, receiver, false)
                && call
                    .args
                    .iter()
                    .all(|arg| borrow_receiver_expr_is_safe(arg, receiver, false))
        }
        ExprKind::Binary(binary) => {
            borrow_receiver_expr_is_safe(&binary.left, receiver, false)
                && borrow_receiver_expr_is_safe(&binary.right, receiver, false)
        }
        ExprKind::Unary(unary) => borrow_receiver_expr_is_safe(&unary.expr, receiver, false),
        ExprKind::Cast(cast) => borrow_receiver_expr_is_safe(&cast.expr, receiver, false),
        ExprKind::Pipe(pipe) => {
            borrow_receiver_expr_is_safe(&pipe.expr, receiver, false)
                && match &pipe.target {
                    PipeTarget::Ident(name) => name != receiver,
                    PipeTarget::Expr(target) => {
                        borrow_receiver_expr_is_safe(target, receiver, false)
                    }
                }
        }
        ExprKind::With(with) => {
            borrow_receiver_fields_are_safe(&with.bindings, receiver)
                && borrow_receiver_is_field_read_only(&with.body, receiver)
        }
        ExprKind::WithLifetime(with) => borrow_receiver_is_field_read_only(&with.body, receiver),
        ExprKind::Block(block) => borrow_receiver_is_field_read_only(block, receiver),
        ExprKind::FieldAccess(inner, _) => borrow_receiver_expr_is_safe(inner, receiver, true),
        ExprKind::ListLit(items) | ExprKind::ArrayLit(items) => items
            .iter()
            .all(|item| borrow_receiver_expr_is_safe(item, receiver, false)),
        ExprKind::RangeLit(range) => {
            borrow_receiver_expr_is_safe(&range.start, receiver, false)
                && borrow_receiver_expr_is_safe(&range.end, receiver, false)
        }
        ExprKind::Freeze(inner) | ExprKind::Await(inner) | ExprKind::Spawn(inner) => {
            borrow_receiver_expr_is_safe(inner, receiver, false)
        }
        // Until checked capture effects are attached to this plan, lambdas in
        // a borrow method are rejected conservatively: an immediate-looking
        // lambda can still be returned through a branch or alias.
        ExprKind::Lambda(_) => false,
        ExprKind::IntLit(_)
        | ExprKind::FloatLit(_)
        | ExprKind::StringLit(_)
        | ExprKind::CharLit(_)
        | ExprKind::BoolLit(_)
        | ExprKind::Unit
        | ExprKind::VariantRef(_) => true,
    }
}

fn ensure_resolved_record(
    root: &ComponentResourceRootCatalog,
    resolved: &Program,
    name: &str,
    path: &str,
) -> Result<(), ComponentResourcePlanError> {
    let expected = root
        .records
        .get(name)
        .expect("resource source was resolved before this check");
    if resolved.declarations.iter().any(|declaration| {
        declaration_record(declaration).is_some_and(|(actual, visibility)| {
            actual == &expected.declaration && visibility == expected.visibility
        })
    }) {
        Ok(())
    } else {
        Err(
            ComponentResourcePlanError::ResolvedRootDeclarationMismatch {
                path: path.to_string(),
                name: name.to_string(),
            },
        )
    }
}

fn resolve_root_function<'a>(
    root: &'a ComponentResourceRootCatalog,
    resolved: &Program,
    checked_by_name: &BTreeMap<&str, &'a CheckedFunctionIr>,
    path: &str,
    name: &str,
    expected_visibility: Option<ComponentSourceVisibility>,
) -> Result<(&'a FunDecl, &'a CheckedFunctionIr), ComponentResourcePlanError> {
    let source =
        root.functions
            .get(name)
            .ok_or_else(|| ComponentResourcePlanError::FunctionNotFound {
                path: path.to_string(),
                function: name.to_string(),
            })?;
    if let Some(expected) = expected_visibility {
        if source.visibility != expected {
            return Err(ComponentResourcePlanError::FunctionVisibility {
                path: path.to_string(),
                function: name.to_string(),
                expected,
                actual: source.visibility,
            });
        }
    }
    if !resolved.declarations.iter().any(|declaration| {
        declaration_function(declaration).is_some_and(|(actual, visibility)| {
            actual == &source.declaration && visibility == source.visibility
        })
    }) {
        return Err(
            ComponentResourcePlanError::ResolvedRootDeclarationMismatch {
                path: path.to_string(),
                name: name.to_string(),
            },
        );
    }
    let checked = checked_by_name.get(name).copied().ok_or_else(|| {
        ComponentResourcePlanError::CheckedFunctionNotFound {
            path: path.to_string(),
            function: name.to_string(),
        }
    })?;
    if checked.lowering.source_exported != (source.visibility == ComponentSourceVisibility::Public)
    {
        return Err(ComponentResourcePlanError::CheckedFunctionVisibility {
            path: path.to_string(),
            function: name.to_string(),
        });
    }
    Ok((&source.declaration, checked))
}

fn ensure_sync_concrete_function(
    source: &FunDecl,
    checked: &CheckedFunctionIr,
    path: &str,
) -> Result<(), ComponentResourcePlanError> {
    if source.is_async {
        return Err(ComponentResourcePlanError::AsyncFunctionUnsupported {
            path: path.to_string(),
            function: source.name.clone(),
        });
    }
    if !source.type_params.is_empty()
        || !source.temporal_constraints.is_empty()
        || !checked.monomorphic
    {
        return Err(ComponentResourcePlanError::OpenFunctionUnsupported {
            path: path.to_string(),
            function: source.name.clone(),
        });
    }
    Ok(())
}

fn classify_boundary_type(
    ty: &TypedType,
    resources: &[PlannedComponentResource],
    callables: &[PlannedComponentCallable],
    path: &str,
) -> Result<PlannedComponentBoundaryType, ComponentResourcePlanError> {
    if let Some(resource) = resources
        .iter()
        .find(|resource| same_component_type(&resource.source_type, ty))
    {
        return Ok(PlannedComponentBoundaryType::OwnResource(
            resource.identity.clone(),
        ));
    }
    if matches!(ty, TypedType::Function { .. }) {
        let matches = callables
            .iter()
            .filter(|callable| same_component_type(&callable.source_type, ty))
            .collect::<Vec<_>>();
        return match matches.as_slice() {
            [callable] => Ok(PlannedComponentBoundaryType::OwnCallable(
                callable.identity.clone(),
            )),
            [] => Err(ComponentResourcePlanError::UndeclaredCallableSignature {
                path: path.to_string(),
                signature: format_typed_type(ty),
            }),
            _ => Err(ComponentResourcePlanError::AmbiguousCallableSignature {
                path: path.to_string(),
                signature: format_typed_type(ty),
            }),
        };
    }
    if contains_any_handle(ty, resources) {
        return Err(ComponentResourcePlanError::NestedHandleUnsupported {
            path: path.to_string(),
            ty: format_typed_type(ty),
        });
    }
    if !is_synchronous_closed_value(ty) {
        return Err(ComponentResourcePlanError::UnsupportedBoundaryType {
            path: path.to_string(),
            ty: format_typed_type(ty),
        });
    }
    Ok(PlannedComponentBoundaryType::Value(ty.clone()))
}

fn contains_any_handle(ty: &TypedType, resources: &[PlannedComponentResource]) -> bool {
    if matches!(ty, TypedType::Function { .. })
        || resources
            .iter()
            .any(|resource| same_component_type(&resource.source_type, ty))
    {
        return true;
    }
    match ty {
        TypedType::Record { type_args, .. } => type_args
            .iter()
            .any(|argument| contains_any_handle(argument, resources)),
        TypedType::Option(inner)
        | TypedType::List(inner)
        | TypedType::Array(inner, _)
        | TypedType::Temporal {
            base_type: inner, ..
        } => contains_any_handle(inner, resources),
        TypedType::Result(ok, err) => {
            contains_any_handle(ok, resources) || contains_any_handle(err, resources)
        }
        TypedType::Projection { base, args, .. } => {
            contains_any_handle(base, resources)
                || args
                    .iter()
                    .any(|argument| contains_any_handle(argument, resources))
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
        | TypedType::InferVar(_) => false,
        TypedType::Function { .. } => true,
    }
}

fn contains_resource_type(ty: &TypedType, source_record: &str) -> bool {
    match ty {
        TypedType::Record {
            name, type_args, ..
        } => {
            name == source_record
                || type_args
                    .iter()
                    .any(|argument| contains_resource_type(argument, source_record))
        }
        TypedType::Function {
            params,
            return_type,
        } => {
            params
                .iter()
                .any(|param| contains_resource_type(param, source_record))
                || contains_resource_type(return_type, source_record)
        }
        TypedType::Option(inner)
        | TypedType::List(inner)
        | TypedType::Array(inner, _)
        | TypedType::Temporal {
            base_type: inner, ..
        } => contains_resource_type(inner, source_record),
        TypedType::Result(ok, err) => {
            contains_resource_type(ok, source_record) || contains_resource_type(err, source_record)
        }
        TypedType::Projection { base, args, .. } => {
            contains_resource_type(base, source_record)
                || args
                    .iter()
                    .any(|argument| contains_resource_type(argument, source_record))
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
        | TypedType::InferVar(_) => false,
    }
}

fn is_synchronous_closed_value(ty: &TypedType) -> bool {
    match ty {
        TypedType::Int32
        | TypedType::Int64
        | TypedType::Float64
        | TypedType::Boolean
        | TypedType::String
        | TypedType::Char
        | TypedType::Unit
        | TypedType::Enum { .. } => true,
        TypedType::Record { type_args, .. } => type_args.iter().all(is_synchronous_closed_value),
        TypedType::Option(inner) | TypedType::List(inner) => is_synchronous_closed_value(inner),
        TypedType::Array(inner, ArrayLength::Known(_)) => is_synchronous_closed_value(inner),
        TypedType::Result(ok, err) => {
            is_synchronous_closed_value(ok) && is_synchronous_closed_value(err)
        }
        TypedType::Function { .. }
        | TypedType::Array(_, ArrayLength::AnyInternal)
        | TypedType::TypeParam(_)
        | TypedType::InferVar(_)
        | TypedType::Projection { .. }
        | TypedType::Temporal { .. } => false,
    }
}

fn same_component_type(left: &TypedType, right: &TypedType) -> bool {
    match (left, right) {
        (TypedType::Int32, TypedType::Int32)
        | (TypedType::Int64, TypedType::Int64)
        | (TypedType::Float64, TypedType::Float64)
        | (TypedType::Boolean, TypedType::Boolean)
        | (TypedType::String, TypedType::String)
        | (TypedType::Char, TypedType::Char)
        | (TypedType::Unit, TypedType::Unit) => true,
        (
            TypedType::Record {
                name: left_name,
                type_args: left_args,
                ..
            },
            TypedType::Record {
                name: right_name,
                type_args: right_args,
                ..
            },
        ) => {
            left_name == right_name
                && left_args.len() == right_args.len()
                && left_args
                    .iter()
                    .zip(right_args)
                    .all(|(left, right)| same_component_type(left, right))
        }
        (TypedType::Enum { name: left }, TypedType::Enum { name: right }) => left == right,
        (
            TypedType::Function {
                params: left_params,
                return_type: left_result,
            },
            TypedType::Function {
                params: right_params,
                return_type: right_result,
            },
        ) => {
            left_params.len() == right_params.len()
                && left_params
                    .iter()
                    .zip(right_params)
                    .all(|(left, right)| same_component_type(left, right))
                && same_component_type(left_result, right_result)
        }
        (TypedType::Option(left), TypedType::Option(right))
        | (TypedType::List(left), TypedType::List(right)) => same_component_type(left, right),
        (TypedType::Result(left_ok, left_err), TypedType::Result(right_ok, right_err)) => {
            same_component_type(left_ok, right_ok) && same_component_type(left_err, right_err)
        }
        (TypedType::Array(left, left_len), TypedType::Array(right, right_len)) => {
            left_len == right_len && same_component_type(left, right)
        }
        (TypedType::TypeParam(left), TypedType::TypeParam(right)) => left == right,
        _ => false,
    }
}

fn declaration_record(declaration: &TopDecl) -> Option<(&RecordDecl, ComponentSourceVisibility)> {
    match declaration {
        TopDecl::Record(record) => Some((record, ComponentSourceVisibility::Private)),
        TopDecl::Export(export) => match export.item.as_ref() {
            TopDecl::Record(record) => Some((record, ComponentSourceVisibility::Public)),
            _ => None,
        },
        _ => None,
    }
}

fn declaration_function(declaration: &TopDecl) -> Option<(&FunDecl, ComponentSourceVisibility)> {
    match declaration {
        TopDecl::Function(function) => Some((function, ComponentSourceVisibility::Private)),
        TopDecl::Export(export) => match export.item.as_ref() {
            TopDecl::Function(function) => Some((function, ComponentSourceVisibility::Public)),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_selection_v2::{
        ComponentCallableContract, ComponentCallableParamContract, ComponentDestructorContract,
        ComponentFunctionExportContract, ComponentResourceContract,
        ComponentResourceMethodContract,
    };
    use crate::ir::builder::build_checked_ir;
    use crate::parser::parse_program;
    use crate::type_checker::TypeChecker;

    const SOURCE: &str = r#"
pub record Counter { value: Int32 }

fun close_counter: (counter: Counter) -> () = { () }
fun read_counter: (counter: Counter) -> Int32 = { counter.value }
fun finish_counter: (counter: Counter) -> Int32 = { counter.value }

pub fun make_counter: (value: Int32) -> Counter = {
    Counter { value: value }
}

pub fun make_transform: (offset: Int32) -> Int32 -> Int32 = {
    |value: Int32| value + offset
}
"#;

    fn checked(source: &str) -> (Program, CheckedProgramIr) {
        let (remaining, program) = parse_program(source).expect("source should parse");
        assert!(remaining.trim().is_empty());
        let mut checker = TypeChecker::new();
        checker
            .check_program(&program)
            .expect("source should type check");
        let checked = build_checked_ir(&program, &checker).expect("checked IR should build");
        (program, checked)
    }

    fn selection() -> ComponentSelectionContract {
        ComponentSelectionContract::try_new(
            COMPONENT_SELECTION_SCHEMA_V2,
            "test:resources@0.1.0",
            "resources",
            COMPONENT_SELECTION_ABI_V2,
            vec![
                ComponentFunctionExportContract {
                    interface: "api".to_string(),
                    source_function: "make_counter".to_string(),
                    external_name: "make-counter".to_string(),
                    type_arguments: vec![],
                },
                ComponentFunctionExportContract {
                    interface: "api".to_string(),
                    source_function: "make_transform".to_string(),
                    external_name: "make-transform".to_string(),
                    type_arguments: vec![],
                },
            ],
            vec![ComponentResourceContract {
                interface: "api".to_string(),
                source_record: "Counter".to_string(),
                external_name: "counter".to_string(),
                destructor: ComponentDestructorContract {
                    source_function: "close_counter".to_string(),
                    visibility: ComponentSourceVisibility::Private,
                },
                methods: vec![
                    ComponentResourceMethodContract {
                        source_function: "read_counter".to_string(),
                        external_name: "read".to_string(),
                        receiver: ComponentResourceReceiver::Borrow,
                    },
                    ComponentResourceMethodContract {
                        source_function: "finish_counter".to_string(),
                        external_name: "finish".to_string(),
                        receiver: ComponentResourceReceiver::Own,
                    },
                ],
            }],
            vec![ComponentCallableContract {
                interface: "api".to_string(),
                external_name: "transform".to_string(),
                params: vec![ComponentCallableParamContract {
                    name: "value".to_string(),
                    ty: ClosedComponentType::Int32,
                }],
                result: ClosedComponentType::Int32,
                multiplicity: ComponentCallableMultiplicity::Once,
            }],
        )
        .expect("selection should normalize")
    }

    #[test]
    fn plans_resource_ownership_and_callable_once_from_checked_ir() {
        let (program, checked) = checked(SOURCE);
        let root = ComponentResourceRootCatalog::from_unresolved_root(&program).unwrap();
        let plan = plan_component_resources(&root, &program, &checked, &selection()).unwrap();

        assert_eq!(plan.resources.len(), 1);
        assert_eq!(plan.callables.len(), 1);
        assert_eq!(plan.exports.len(), 2);
        assert_eq!(
            plan.resources[0]
                .methods
                .iter()
                .map(|method| (method.external_name.as_str(), method.receiver))
                .collect::<Vec<_>>(),
            vec![
                ("finish", ComponentResourceReceiver::Own),
                ("read", ComponentResourceReceiver::Borrow),
            ]
        );
        assert_eq!(
            plan.callables[0].multiplicity,
            ComponentCallableMultiplicity::Once
        );
        assert!(matches!(
            plan.export("api", "make-counter").unwrap().result,
            PlannedComponentBoundaryType::OwnResource(_)
        ));
        assert!(matches!(
            plan.export("api", "make-transform").unwrap().result,
            PlannedComponentBoundaryType::OwnCallable(_)
        ));
    }

    #[test]
    fn rejects_private_resource_source_record() {
        let source = SOURCE.replacen("pub record Counter", "record Counter", 1);
        let (program, checked) = checked(&source);
        let root = ComponentResourceRootCatalog::from_unresolved_root(&program).unwrap();
        let error = plan_component_resources(&root, &program, &checked, &selection())
            .expect_err("private resource record must fail closed");
        assert!(matches!(
            error,
            ComponentResourcePlanError::ResourceRecordNotPublic { .. }
        ));
    }

    #[test]
    fn rejects_destructor_visibility_mismatch() {
        let (program, checked) = checked(SOURCE);
        let root = ComponentResourceRootCatalog::from_unresolved_root(&program).unwrap();
        let mut selection = selection();
        selection.resources[0].destructor.visibility = ComponentSourceVisibility::Public;
        let error = plan_component_resources(&root, &program, &checked, &selection)
            .expect_err("stale destructor visibility must fail closed");
        assert!(matches!(
            error,
            ComponentResourcePlanError::FunctionVisibility { .. }
        ));
    }

    #[test]
    fn rejects_async_resource_method() {
        let source = SOURCE.replacen("fun read_counter", "async fun read_counter", 1);
        let (program, checked) = checked(&source);
        let root = ComponentResourceRootCatalog::from_unresolved_root(&program).unwrap();
        let error = plan_component_resources(&root, &program, &checked, &selection())
            .expect_err("async resource method must fail closed");
        assert!(matches!(
            error,
            ComponentResourcePlanError::AsyncFunctionUnsupported { .. }
        ));
    }

    #[test]
    fn rejects_borrow_method_that_moves_receiver_to_destructor() {
        let source = SOURCE.replace(
            "fun read_counter: (counter: Counter) -> Int32 = { counter.value }",
            "fun read_counter: (counter: Counter) -> Int32 = { counter |> close_counter; 0 }",
        );
        let (program, checked) = checked(&source);
        let root = ComponentResourceRootCatalog::from_unresolved_root(&program).unwrap();
        let error = plan_component_resources(&root, &program, &checked, &selection())
            .expect_err("manifest borrow cannot relabel an affine-consuming function");
        assert!(matches!(
            error,
            ComponentResourcePlanError::BorrowReceiverEscapes { .. }
        ));
    }

    #[test]
    fn rejects_many_even_if_contract_bypasses_normalization() {
        let (program, checked) = checked(SOURCE);
        let root = ComponentResourceRootCatalog::from_unresolved_root(&program).unwrap();
        let mut selection = selection();
        selection.callables[0].multiplicity = ComponentCallableMultiplicity::Many;
        let error = plan_component_resources(&root, &program, &checked, &selection)
            .expect_err("many must fail closed at the checked seam too");
        assert!(matches!(
            error,
            ComponentResourcePlanError::UnsupportedCallableMultiplicity { .. }
        ));
    }

    #[test]
    fn rejects_undeclared_callable_factory_result() {
        let (program, checked) = checked(SOURCE);
        let root = ComponentResourceRootCatalog::from_unresolved_root(&program).unwrap();
        let mut selection = selection();
        selection.callables.clear();
        let error = plan_component_resources(&root, &program, &checked, &selection)
            .expect_err("function values cannot leak as raw table indexes");
        assert!(matches!(
            error,
            ComponentResourcePlanError::UndeclaredCallableSignature { .. }
        ));
    }

    #[test]
    fn rejects_own_method_that_returns_consumed_resource() {
        let source = SOURCE.replace(
            "fun finish_counter: (counter: Counter) -> Int32 = { counter.value }",
            "fun finish_counter: (counter: Counter) -> Counter = { counter }",
        );
        let (program, checked) = checked(&source);
        let root = ComponentResourceRootCatalog::from_unresolved_root(&program).unwrap();
        let error = plan_component_resources(&root, &program, &checked, &selection())
            .expect_err("terminal own method cannot re-export the destroyed receiver");
        assert!(matches!(
            error,
            ComponentResourcePlanError::OwnMethodReturnsConsumedResource { .. }
        ));
    }

    #[test]
    fn rejects_once_callable_that_returns_an_owned_handle() {
        let (program, checked) = checked(SOURCE);
        let root = ComponentResourceRootCatalog::from_unresolved_root(&program).unwrap();
        let mut selection = selection();
        selection.callables[0].result = ClosedComponentType::Named {
            name: "Counter".to_string(),
            arguments: vec![],
        };
        let error = plan_component_resources(&root, &program, &checked, &selection)
            .expect_err("callable resource result transfer is not in the initial adapter");
        assert!(matches!(
            error,
            ComponentResourcePlanError::CallableReturnsHandle { .. }
        ));
    }

    #[test]
    fn rejects_handle_producer_that_consumes_another_handle() {
        let source = format!(
            "{SOURCE}\npub fun replace_counter: (counter: Counter) -> Counter = {{ counter }}\n"
        );
        let (program, checked) = checked(&source);
        let root = ComponentResourceRootCatalog::from_unresolved_root(&program).unwrap();
        let mut selection = selection();
        selection.exports.push(ComponentFunctionExportContract {
            interface: "api".to_string(),
            source_function: "replace_counter".to_string(),
            external_name: "replace-counter".to_string(),
            type_arguments: vec![],
        });
        let error = plan_component_resources(&root, &program, &checked, &selection)
            .expect_err("handle transfer into a new exported handle must fail closed");
        assert!(matches!(
            error,
            ComponentResourcePlanError::HandleProducerConsumesHandle { .. }
        ));
    }

    #[test]
    fn rejects_free_export_that_consumes_a_handle() {
        let source = format!(
            "{SOURCE}\npub fun inspect_counter: (counter: Counter) -> Int32 = {{ counter.value }}\n"
        );
        let (program, checked) = checked(&source);
        let root = ComponentResourceRootCatalog::from_unresolved_root(&program).unwrap();
        let mut selection = selection();
        selection.exports.push(ComponentFunctionExportContract {
            interface: "api".to_string(),
            source_function: "inspect_counter".to_string(),
            external_name: "inspect-counter".to_string(),
            type_arguments: vec![],
        });
        let error = plan_component_resources(&root, &program, &checked, &selection)
            .expect_err("free own-handle parameters require an implemented drop path");
        assert!(matches!(
            error,
            ComponentResourcePlanError::FreeExportConsumesHandle { .. }
        ));
    }

    #[test]
    fn rejects_resource_method_with_an_additional_handle_parameter() {
        let source = format!(
            "{SOURCE}\nfun compare_counter: (counter: Counter, other: Counter) -> Int32 = {{ counter.value }}\n"
        );
        let (program, checked) = checked(&source);
        let root = ComponentResourceRootCatalog::from_unresolved_root(&program).unwrap();
        let mut selection = selection();
        selection.resources[0]
            .methods
            .push(ComponentResourceMethodContract {
                source_function: "compare_counter".to_string(),
                external_name: "compare".to_string(),
                receiver: ComponentResourceReceiver::Borrow,
            });
        let error = plan_component_resources(&root, &program, &checked, &selection)
            .expect_err("additional method handles require an implemented drop path");
        assert!(matches!(
            error,
            ComponentResourcePlanError::ResourceMethodHandleParam { .. }
        ));
    }

    #[test]
    fn rejects_once_callable_with_a_handle_parameter() {
        let (program, checked) = checked(SOURCE);
        let root = ComponentResourceRootCatalog::from_unresolved_root(&program).unwrap();
        let mut selection = selection();
        selection.callables[0].params[0].ty = ClosedComponentType::Named {
            name: "Counter".to_string(),
            arguments: vec![],
        };
        let error = plan_component_resources(&root, &program, &checked, &selection)
            .expect_err("callable handles require an implemented drop path");
        assert!(matches!(
            error,
            ComponentResourcePlanError::CallableHandleParam { .. }
        ));
    }
}
