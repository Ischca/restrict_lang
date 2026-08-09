//! Semantic compatibility classification for WebAssembly Component interfaces.
//!
//! Compatibility is derived from canonical Component metadata, not from Core
//! Wasm bytes or interface-hash equality. Stable rich type declarations and
//! fixed-array lengths participate even when their normalized WIT spelling is
//! otherwise identical.

#[cfg(not(target_arch = "wasm32"))]
use crate::component::ComponentBuildArtifact;
use crate::component::{
    ComponentCallableMultiplicityIr, ComponentInterfaceArtifact, ComponentNamedTypeMetadataKind,
    ComponentParamMetadata, ComponentResourceReceiverIr, ComponentSemanticMetadata,
    ComponentValueType, COMPONENT_RICH_INTERFACE_SUBSET, COMPONENT_SCALAR_IMPLEMENTATION_SUBSET,
};
use std::collections::{BTreeMap, BTreeSet};

/// Overall semantic compatibility from an older contract to a newer contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentCompatibility {
    /// The two canonical semantic contracts are equivalent.
    Identical,
    /// The newer contract only adds exported interfaces or functions.
    NonBreaking,
    /// At least one existing consumer-facing contract was changed or removed.
    Breaking,
}

/// One deterministic semantic change between Component contracts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentCompatibilityChange {
    MetadataSchemaVersionChanged {
        previous: u32,
        current: u32,
    },
    ImplementationSubsetChanged {
        previous: String,
        current: String,
    },
    AbiProfileChanged {
        previous: String,
        current: String,
    },
    PackageChanged {
        previous: String,
        current: String,
    },
    WorldChanged {
        previous: String,
        current: String,
    },
    InterfaceAdded {
        interface: String,
    },
    InterfaceRemoved {
        interface: String,
    },
    TypeAdded {
        interface: String,
        ty: String,
    },
    TypeRemoved {
        interface: String,
        ty: String,
    },
    TypeChanged {
        interface: String,
        ty: String,
        previous: ComponentNamedTypeMetadataKind,
        current: ComponentNamedTypeMetadataKind,
    },
    ResourceAdded {
        interface: String,
        resource: String,
    },
    ResourceRemoved {
        interface: String,
        resource: String,
    },
    ResourceMethodAdded {
        interface: String,
        resource: String,
        method: String,
    },
    ResourceMethodRemoved {
        interface: String,
        resource: String,
        method: String,
    },
    ResourceMethodReceiverChanged {
        interface: String,
        resource: String,
        method: String,
        previous: ComponentResourceReceiverIr,
        current: ComponentResourceReceiverIr,
    },
    ResourceMethodSignatureChanged {
        interface: String,
        resource: String,
        method: String,
        previous_params: Vec<ComponentParamMetadata>,
        current_params: Vec<ComponentParamMetadata>,
        previous_result: ComponentValueType,
        current_result: ComponentValueType,
    },
    CallableAdded {
        interface: String,
        callable: String,
    },
    CallableRemoved {
        interface: String,
        callable: String,
    },
    CallableSignatureChanged {
        interface: String,
        callable: String,
        previous_params: Vec<ComponentParamMetadata>,
        current_params: Vec<ComponentParamMetadata>,
        previous_result: ComponentValueType,
        current_result: ComponentValueType,
    },
    CallableMultiplicityChanged {
        interface: String,
        callable: String,
        previous: ComponentCallableMultiplicityIr,
        current: ComponentCallableMultiplicityIr,
    },
    FunctionAdded {
        interface: String,
        function: String,
    },
    FunctionRemoved {
        interface: String,
        function: String,
    },
    ParameterAdded {
        interface: String,
        function: String,
        position: usize,
        name: String,
        ty: ComponentValueType,
    },
    ParameterRemoved {
        interface: String,
        function: String,
        position: usize,
        name: String,
        ty: ComponentValueType,
    },
    ParameterNameChanged {
        interface: String,
        function: String,
        position: usize,
        previous: String,
        current: String,
    },
    ParameterTypeChanged {
        interface: String,
        function: String,
        position: usize,
        previous: ComponentValueType,
        current: ComponentValueType,
    },
    ResultTypeChanged {
        interface: String,
        function: String,
        previous: ComponentValueType,
        current: ComponentValueType,
    },
}

impl ComponentCompatibilityChange {
    /// Whether this change can invalidate an existing consumer.
    pub fn is_breaking(&self) -> bool {
        match self {
            Self::InterfaceAdded { .. }
            | Self::TypeAdded { .. }
            | Self::ResourceAdded { .. }
            | Self::ResourceMethodAdded { .. }
            | Self::CallableAdded { .. }
            | Self::FunctionAdded { .. } => false,
            Self::ImplementationSubsetChanged { previous, current }
                if previous == COMPONENT_SCALAR_IMPLEMENTATION_SUBSET
                    && current == COMPONENT_RICH_INTERFACE_SUBSET =>
            {
                // Rich lowering is a strict capability superset under the same
                // Canonical ABI profile. The actual interface shape changes
                // are classified independently below.
                false
            }
            _ => true,
        }
    }
}

/// Deterministically ordered compatibility result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentCompatibilityDiff {
    pub compatibility: ComponentCompatibility,
    pub changes: Vec<ComponentCompatibilityChange>,
}

impl ComponentCompatibilityDiff {
    pub fn is_compatible(&self) -> bool {
        self.compatibility != ComponentCompatibility::Breaking
    }
}

/// Compare two validated compiler artifacts by their semantic contracts.
///
/// Interface hashes are deliberately not used as the classifier: hashes are
/// fast identity keys, while compatibility requires a directional semantic
/// comparison from `previous` to `current`.
pub fn diff_component_interfaces(
    previous: &ComponentInterfaceArtifact,
    current: &ComponentInterfaceArtifact,
) -> ComponentCompatibilityDiff {
    diff_component_metadata(&previous.metadata, &current.metadata)
}

/// Compare two fully packaged compiler outputs without reconstructing an
/// interface contract from WIT or Core Wasm.
#[cfg(not(target_arch = "wasm32"))]
pub fn diff_component_build_artifacts(
    previous: &ComponentBuildArtifact,
    current: &ComponentBuildArtifact,
) -> ComponentCompatibilityDiff {
    diff_component_metadata(&previous.semantic_metadata, &current.semantic_metadata)
}

/// Compare canonical Component semantic metadata from older to newer.
///
/// The metadata produced by the compiler is already normalized. This function
/// nevertheless indexes interfaces and functions by stable external name so
/// declaration-vector order cannot make a diff nondeterministic.
pub fn diff_component_metadata(
    previous: &ComponentSemanticMetadata,
    current: &ComponentSemanticMetadata,
) -> ComponentCompatibilityDiff {
    let mut changes = Vec::new();

    if previous.schema_version != current.schema_version {
        changes.push(ComponentCompatibilityChange::MetadataSchemaVersionChanged {
            previous: previous.schema_version,
            current: current.schema_version,
        });
    }
    if previous.implementation_subset != current.implementation_subset {
        changes.push(ComponentCompatibilityChange::ImplementationSubsetChanged {
            previous: previous.implementation_subset.clone(),
            current: current.implementation_subset.clone(),
        });
    }
    if previous.abi_profile != current.abi_profile {
        changes.push(ComponentCompatibilityChange::AbiProfileChanged {
            previous: previous.abi_profile.clone(),
            current: current.abi_profile.clone(),
        });
    }
    if previous.package != current.package {
        changes.push(ComponentCompatibilityChange::PackageChanged {
            previous: previous.package.clone(),
            current: current.package.clone(),
        });
    }
    if previous.world != current.world {
        changes.push(ComponentCompatibilityChange::WorldChanged {
            previous: previous.world.clone(),
            current: current.world.clone(),
        });
    }

    let previous_interfaces = previous
        .interfaces
        .iter()
        .map(|interface| (interface.name.as_str(), interface))
        .collect::<BTreeMap<_, _>>();
    let current_interfaces = current
        .interfaces
        .iter()
        .map(|interface| (interface.name.as_str(), interface))
        .collect::<BTreeMap<_, _>>();
    let interface_names = previous_interfaces
        .keys()
        .chain(current_interfaces.keys())
        .copied()
        .collect::<BTreeSet<_>>();

    for interface_name in interface_names {
        match (
            previous_interfaces.get(interface_name),
            current_interfaces.get(interface_name),
        ) {
            (None, Some(_)) => changes.push(ComponentCompatibilityChange::InterfaceAdded {
                interface: interface_name.to_string(),
            }),
            (Some(_), None) => changes.push(ComponentCompatibilityChange::InterfaceRemoved {
                interface: interface_name.to_string(),
            }),
            (Some(previous_interface), Some(current_interface)) => {
                diff_interface(
                    interface_name,
                    previous_interface,
                    current_interface,
                    &mut changes,
                );
            }
            (None, None) => unreachable!("interface name came from one of the metadata maps"),
        }
    }

    let compatibility = if changes.is_empty() {
        ComponentCompatibility::Identical
    } else if changes
        .iter()
        .any(ComponentCompatibilityChange::is_breaking)
    {
        ComponentCompatibility::Breaking
    } else {
        ComponentCompatibility::NonBreaking
    };

    ComponentCompatibilityDiff {
        compatibility,
        changes,
    }
}

fn diff_interface(
    interface_name: &str,
    previous: &crate::component::ComponentInterfaceMetadata,
    current: &crate::component::ComponentInterfaceMetadata,
    changes: &mut Vec<ComponentCompatibilityChange>,
) {
    let previous_types = previous
        .types
        .iter()
        .map(|ty| (ty.name.as_str(), ty))
        .collect::<BTreeMap<_, _>>();
    let current_types = current
        .types
        .iter()
        .map(|ty| (ty.name.as_str(), ty))
        .collect::<BTreeMap<_, _>>();
    let type_names = previous_types
        .keys()
        .chain(current_types.keys())
        .copied()
        .collect::<BTreeSet<_>>();

    for type_name in type_names {
        match (previous_types.get(type_name), current_types.get(type_name)) {
            (None, Some(_)) => changes.push(ComponentCompatibilityChange::TypeAdded {
                interface: interface_name.to_string(),
                ty: type_name.to_string(),
            }),
            (Some(_), None) => changes.push(ComponentCompatibilityChange::TypeRemoved {
                interface: interface_name.to_string(),
                ty: type_name.to_string(),
            }),
            (Some(previous_type), Some(current_type))
                if previous_type.kind != current_type.kind =>
            {
                changes.push(ComponentCompatibilityChange::TypeChanged {
                    interface: interface_name.to_string(),
                    ty: type_name.to_string(),
                    previous: previous_type.kind.clone(),
                    current: current_type.kind.clone(),
                });
            }
            (Some(_), Some(_)) => {}
            (None, None) => unreachable!("type name came from one of the metadata maps"),
        }
    }

    diff_resources(
        interface_name,
        previous.resources.as_slice(),
        current.resources.as_slice(),
        changes,
    );
    diff_callables(
        interface_name,
        previous.callables.as_slice(),
        current.callables.as_slice(),
        changes,
    );

    let previous_functions = previous
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let current_functions = current
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let function_names = previous_functions
        .keys()
        .chain(current_functions.keys())
        .copied()
        .collect::<BTreeSet<_>>();

    for function_name in function_names {
        match (
            previous_functions.get(function_name),
            current_functions.get(function_name),
        ) {
            (None, Some(_)) => changes.push(ComponentCompatibilityChange::FunctionAdded {
                interface: interface_name.to_string(),
                function: function_name.to_string(),
            }),
            (Some(_), None) => changes.push(ComponentCompatibilityChange::FunctionRemoved {
                interface: interface_name.to_string(),
                function: function_name.to_string(),
            }),
            (Some(previous_function), Some(current_function)) => diff_function(
                interface_name,
                function_name,
                previous_function,
                current_function,
                changes,
            ),
            (None, None) => unreachable!("function name came from one of the metadata maps"),
        }
    }
}

fn diff_resources(
    interface_name: &str,
    previous: &[crate::component::ComponentResourceMetadata],
    current: &[crate::component::ComponentResourceMetadata],
    changes: &mut Vec<ComponentCompatibilityChange>,
) {
    let previous_resources = previous
        .iter()
        .map(|resource| (resource.name.as_str(), resource))
        .collect::<BTreeMap<_, _>>();
    let current_resources = current
        .iter()
        .map(|resource| (resource.name.as_str(), resource))
        .collect::<BTreeMap<_, _>>();
    let resource_names = previous_resources
        .keys()
        .chain(current_resources.keys())
        .copied()
        .collect::<BTreeSet<_>>();

    for resource_name in resource_names {
        match (
            previous_resources.get(resource_name),
            current_resources.get(resource_name),
        ) {
            (None, Some(_)) => changes.push(ComponentCompatibilityChange::ResourceAdded {
                interface: interface_name.to_string(),
                resource: resource_name.to_string(),
            }),
            (Some(_), None) => {
                changes.push(ComponentCompatibilityChange::ResourceRemoved {
                    interface: interface_name.to_string(),
                    resource: resource_name.to_string(),
                });
            }
            (Some(previous_resource), Some(current_resource)) => diff_resource_methods(
                interface_name,
                resource_name,
                previous_resource.methods.as_slice(),
                current_resource.methods.as_slice(),
                changes,
            ),
            (None, None) => unreachable!("resource name came from one of the metadata maps"),
        }
    }
}

fn diff_resource_methods(
    interface_name: &str,
    resource_name: &str,
    previous: &[crate::component::ComponentResourceMethodMetadata],
    current: &[crate::component::ComponentResourceMethodMetadata],
    changes: &mut Vec<ComponentCompatibilityChange>,
) {
    let previous_methods = previous
        .iter()
        .map(|method| (method.name.as_str(), method))
        .collect::<BTreeMap<_, _>>();
    let current_methods = current
        .iter()
        .map(|method| (method.name.as_str(), method))
        .collect::<BTreeMap<_, _>>();
    let method_names = previous_methods
        .keys()
        .chain(current_methods.keys())
        .copied()
        .collect::<BTreeSet<_>>();

    for method_name in method_names {
        match (
            previous_methods.get(method_name),
            current_methods.get(method_name),
        ) {
            (None, Some(_)) => {
                changes.push(ComponentCompatibilityChange::ResourceMethodAdded {
                    interface: interface_name.to_string(),
                    resource: resource_name.to_string(),
                    method: method_name.to_string(),
                });
            }
            (Some(_), None) => {
                changes.push(ComponentCompatibilityChange::ResourceMethodRemoved {
                    interface: interface_name.to_string(),
                    resource: resource_name.to_string(),
                    method: method_name.to_string(),
                });
            }
            (Some(previous_method), Some(current_method)) => {
                if previous_method.receiver != current_method.receiver {
                    changes.push(
                        ComponentCompatibilityChange::ResourceMethodReceiverChanged {
                            interface: interface_name.to_string(),
                            resource: resource_name.to_string(),
                            method: method_name.to_string(),
                            previous: previous_method.receiver,
                            current: current_method.receiver,
                        },
                    );
                }
                if previous_method.params != current_method.params
                    || previous_method.result != current_method.result
                {
                    changes.push(
                        ComponentCompatibilityChange::ResourceMethodSignatureChanged {
                            interface: interface_name.to_string(),
                            resource: resource_name.to_string(),
                            method: method_name.to_string(),
                            previous_params: previous_method.params.clone(),
                            current_params: current_method.params.clone(),
                            previous_result: previous_method.result.clone(),
                            current_result: current_method.result.clone(),
                        },
                    );
                }
            }
            (None, None) => unreachable!("method name came from one of the metadata maps"),
        }
    }
}

fn diff_callables(
    interface_name: &str,
    previous: &[crate::component::ComponentCallableMetadata],
    current: &[crate::component::ComponentCallableMetadata],
    changes: &mut Vec<ComponentCompatibilityChange>,
) {
    let previous_callables = previous
        .iter()
        .map(|callable| (callable.name.as_str(), callable))
        .collect::<BTreeMap<_, _>>();
    let current_callables = current
        .iter()
        .map(|callable| (callable.name.as_str(), callable))
        .collect::<BTreeMap<_, _>>();
    let callable_names = previous_callables
        .keys()
        .chain(current_callables.keys())
        .copied()
        .collect::<BTreeSet<_>>();

    for callable_name in callable_names {
        match (
            previous_callables.get(callable_name),
            current_callables.get(callable_name),
        ) {
            (None, Some(_)) => changes.push(ComponentCompatibilityChange::CallableAdded {
                interface: interface_name.to_string(),
                callable: callable_name.to_string(),
            }),
            (Some(_), None) => changes.push(ComponentCompatibilityChange::CallableRemoved {
                interface: interface_name.to_string(),
                callable: callable_name.to_string(),
            }),
            (Some(previous_callable), Some(current_callable)) => {
                if previous_callable.params != current_callable.params
                    || previous_callable.result != current_callable.result
                {
                    changes.push(ComponentCompatibilityChange::CallableSignatureChanged {
                        interface: interface_name.to_string(),
                        callable: callable_name.to_string(),
                        previous_params: previous_callable.params.clone(),
                        current_params: current_callable.params.clone(),
                        previous_result: previous_callable.result.clone(),
                        current_result: current_callable.result.clone(),
                    });
                }
                if previous_callable.multiplicity != current_callable.multiplicity {
                    changes.push(ComponentCompatibilityChange::CallableMultiplicityChanged {
                        interface: interface_name.to_string(),
                        callable: callable_name.to_string(),
                        previous: previous_callable.multiplicity,
                        current: current_callable.multiplicity,
                    });
                }
            }
            (None, None) => unreachable!("callable name came from one of the metadata maps"),
        }
    }
}

fn diff_function(
    interface_name: &str,
    function_name: &str,
    previous: &crate::component::ComponentFunctionMetadata,
    current: &crate::component::ComponentFunctionMetadata,
    changes: &mut Vec<ComponentCompatibilityChange>,
) {
    for (position, (previous_param, current_param)) in
        previous.params.iter().zip(&current.params).enumerate()
    {
        if previous_param.name != current_param.name {
            changes.push(ComponentCompatibilityChange::ParameterNameChanged {
                interface: interface_name.to_string(),
                function: function_name.to_string(),
                position,
                previous: previous_param.name.clone(),
                current: current_param.name.clone(),
            });
        }
        if previous_param.ty != current_param.ty {
            changes.push(ComponentCompatibilityChange::ParameterTypeChanged {
                interface: interface_name.to_string(),
                function: function_name.to_string(),
                position,
                previous: previous_param.ty.clone(),
                current: current_param.ty.clone(),
            });
        }
    }

    for (position, param) in previous
        .params
        .iter()
        .enumerate()
        .skip(current.params.len())
    {
        changes.push(ComponentCompatibilityChange::ParameterRemoved {
            interface: interface_name.to_string(),
            function: function_name.to_string(),
            position,
            name: param.name.clone(),
            ty: param.ty.clone(),
        });
    }
    for (position, param) in current
        .params
        .iter()
        .enumerate()
        .skip(previous.params.len())
    {
        changes.push(ComponentCompatibilityChange::ParameterAdded {
            interface: interface_name.to_string(),
            function: function_name.to_string(),
            position,
            name: param.name.clone(),
            ty: param.ty.clone(),
        });
    }

    if previous.result != current.result {
        changes.push(ComponentCompatibilityChange::ResultTypeChanged {
            interface: interface_name.to_string(),
            function: function_name.to_string(),
            previous: previous.result.clone(),
            current: current.result.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{
        build_component_interface, component_interface_hash, component_semantic_metadata,
        ComponentCallableMetadata, ComponentExportSelection, ComponentFunctionMetadata,
        ComponentInterfaceMetadata, ComponentResourceMetadata, ComponentResourceMethodMetadata,
        ComponentSelection, RootPublicFunctionCatalog, COMPONENT_ABI_PROFILE,
        COMPONENT_SEMANTIC_METADATA_SCHEMA_VERSION,
    };
    use crate::ir::builder::build_checked_ir;
    use crate::{parse_program, Program, TypeChecker};

    fn parse(source: &str) -> Program {
        let (remaining, program) = parse_program(source).expect("source should parse");
        assert!(remaining.trim().is_empty());
        program
    }

    fn artifact(
        source: &str,
        exports: Vec<ComponentExportSelection>,
    ) -> ComponentInterfaceArtifact {
        let root = parse(source);
        let catalog = RootPublicFunctionCatalog::from_unresolved_root(&root).unwrap();
        let mut checker = TypeChecker::new();
        checker
            .check_program(&root)
            .expect("source should type check");
        let checked = build_checked_ir(&root, &checker).expect("checked IR should build");
        build_component_interface(
            &catalog,
            &root,
            &checked,
            &ComponentSelection::v1(
                "test:compat@0.1.0",
                "compat",
                COMPONENT_ABI_PROFILE,
                exports,
            ),
        )
        .expect("component interface should build")
    }

    fn param(name: &str, ty: ComponentValueType) -> ComponentParamMetadata {
        ComponentParamMetadata {
            name: name.to_string(),
            ty,
        }
    }

    fn method(
        name: &str,
        receiver: ComponentResourceReceiverIr,
        params: Vec<ComponentParamMetadata>,
        result: ComponentValueType,
    ) -> ComponentResourceMethodMetadata {
        ComponentResourceMethodMetadata {
            name: name.to_string(),
            receiver,
            params,
            result,
        }
    }

    fn resource(
        name: &str,
        methods: Vec<ComponentResourceMethodMetadata>,
    ) -> ComponentResourceMetadata {
        ComponentResourceMetadata {
            name: name.to_string(),
            methods,
        }
    }

    fn callable(
        name: &str,
        params: Vec<ComponentParamMetadata>,
        result: ComponentValueType,
        multiplicity: ComponentCallableMultiplicityIr,
    ) -> ComponentCallableMetadata {
        ComponentCallableMetadata {
            name: name.to_string(),
            params,
            result,
            multiplicity,
        }
    }

    fn metadata(
        resources: Vec<ComponentResourceMetadata>,
        callables: Vec<ComponentCallableMetadata>,
        functions: Vec<ComponentFunctionMetadata>,
    ) -> ComponentSemanticMetadata {
        ComponentSemanticMetadata {
            schema_version: COMPONENT_SEMANTIC_METADATA_SCHEMA_VERSION,
            implementation_subset: COMPONENT_RICH_INTERFACE_SUBSET.to_string(),
            abi_profile: COMPONENT_ABI_PROFILE.to_string(),
            package: "test:compat@0.1.0".to_string(),
            world: "compat".to_string(),
            interfaces: vec![ComponentInterfaceMetadata {
                name: "api".to_string(),
                types: Vec::new(),
                resources,
                callables,
                functions,
            }],
        }
    }

    fn packaged(metadata: ComponentSemanticMetadata) -> ComponentBuildArtifact {
        ComponentBuildArtifact {
            component_wasm: Vec::new(),
            normalized_wit: String::new(),
            semantic_metadata: metadata.clone(),
            interface_hash: [0; 32],
            implementation_subset: metadata.implementation_subset.clone(),
            package: metadata.package.clone(),
            world: metadata.world.clone(),
            abi_profile: metadata.abi_profile.clone(),
        }
    }

    #[test]
    fn packaged_build_artifacts_expose_the_semantic_compatibility_diff() {
        let previous = packaged(metadata(Vec::new(), Vec::new(), Vec::new()));
        let current = packaged(metadata(
            Vec::new(),
            vec![callable(
                "transform",
                vec![param("value", ComponentValueType::S32)],
                ComponentValueType::S32,
                ComponentCallableMultiplicityIr::Once,
            )],
            Vec::new(),
        ));

        let diff = diff_component_build_artifacts(&previous, &current);
        assert_eq!(diff.compatibility, ComponentCompatibility::NonBreaking);
        assert!(matches!(
            diff.changes.as_slice(),
            [ComponentCompatibilityChange::CallableAdded { interface, callable }]
                if interface == "api" && callable == "transform"
        ));
    }

    #[test]
    fn body_and_internal_lowering_provenance_do_not_change_interface_identity() {
        let previous = artifact(
            "pub fun echo: (value: Int32) -> Int32 = { value }",
            vec![ComponentExportSelection::new("api", "echo", "echo")],
        );
        let body_changed = artifact(
            "pub fun echo: (value: Int32) -> Int32 = { value + 1 }",
            vec![ComponentExportSelection::new("api", "echo", "echo")],
        );

        assert_eq!(previous.interface_hash, body_changed.interface_hash);
        assert_eq!(
            diff_component_interfaces(&previous, &body_changed).compatibility,
            ComponentCompatibility::Identical
        );

        let mut internal_ir_changed = previous.ir.clone();
        let function = &mut internal_ir_changed.interfaces[0].functions[0];
        function.source_function = "different-internal-symbol".to_string();
        function.params[0].source_index = 99;
        function.params[0].source_name = "different_internal_binder".to_string();
        let metadata = component_semantic_metadata(&internal_ir_changed);

        assert_eq!(metadata, previous.metadata);
        assert_eq!(
            component_interface_hash(&previous.normalized_wit, &metadata),
            previous.interface_hash,
            "compiler-owned lowering/layout provenance must not enter the interface hash"
        );
    }

    #[test]
    fn exported_interface_and_function_additions_are_non_breaking() {
        let previous = artifact(
            "pub fun first: (value: Int32) -> Int32 = { value }",
            vec![ComponentExportSelection::new("api", "first", "first")],
        );
        let current = artifact(
            r#"
pub fun first: (value: Int32) -> Int32 = { value }
pub fun second: (value: Int64) -> Int64 = { value }
pub fun inspect: () -> Boolean = { true }
"#,
            vec![
                ComponentExportSelection::new("api", "first", "first"),
                ComponentExportSelection::new("api", "second", "second"),
                ComponentExportSelection::new("admin", "inspect", "inspect"),
            ],
        );

        let diff = diff_component_interfaces(&previous, &current);
        assert_eq!(diff.compatibility, ComponentCompatibility::NonBreaking);
        assert!(diff.is_compatible());
        assert_eq!(
            diff.changes,
            vec![
                ComponentCompatibilityChange::InterfaceAdded {
                    interface: "admin".to_string(),
                },
                ComponentCompatibilityChange::FunctionAdded {
                    interface: "api".to_string(),
                    function: "second".to_string(),
                },
            ]
        );
    }

    #[test]
    fn removals_are_breaking_and_reported_in_stable_name_order() {
        let previous = artifact(
            r#"
pub fun first: (value: Int32) -> Int32 = { value }
pub fun second: (value: Int64) -> Int64 = { value }
pub fun inspect: () -> Boolean = { true }
"#,
            vec![
                ComponentExportSelection::new("api", "second", "second"),
                ComponentExportSelection::new("admin", "inspect", "inspect"),
                ComponentExportSelection::new("api", "first", "first"),
            ],
        );
        let current = artifact(
            "pub fun first: (value: Int32) -> Int32 = { value }",
            vec![ComponentExportSelection::new("api", "first", "first")],
        );

        let diff = diff_component_interfaces(&previous, &current);
        assert_eq!(diff.compatibility, ComponentCompatibility::Breaking);
        assert_eq!(
            diff.changes,
            vec![
                ComponentCompatibilityChange::InterfaceRemoved {
                    interface: "admin".to_string(),
                },
                ComponentCompatibilityChange::FunctionRemoved {
                    interface: "api".to_string(),
                    function: "second".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parameter_name_type_and_result_changes_are_breaking() {
        let previous = artifact(
            "pub fun convert: (value: Int32) -> Int32 = { value }",
            vec![ComponentExportSelection::new("api", "convert", "convert")],
        );
        let current = artifact(
            "pub fun convert: (item: Int64) -> Int64 = { item }",
            vec![ComponentExportSelection::new("api", "convert", "convert")],
        );

        let diff = diff_component_interfaces(&previous, &current);
        assert_eq!(diff.compatibility, ComponentCompatibility::Breaking);
        assert_eq!(diff.changes.len(), 3);
        assert!(matches!(
            diff.changes[0],
            ComponentCompatibilityChange::ParameterNameChanged { position: 0, .. }
        ));
        assert!(matches!(
            diff.changes[1],
            ComponentCompatibilityChange::ParameterTypeChanged { position: 0, .. }
        ));
        assert!(matches!(
            diff.changes[2],
            ComponentCompatibilityChange::ResultTypeChanged { .. }
        ));
    }

    #[test]
    fn parameter_arity_and_abi_profile_changes_are_breaking() {
        let previous = artifact(
            "pub fun convert: (value: Int32) -> Int32 = { value }",
            vec![ComponentExportSelection::new("api", "convert", "convert")],
        );
        let current = artifact(
            "pub fun convert: (value: Int32, mode: Int32) -> Int32 = { value + mode }",
            vec![ComponentExportSelection::new("api", "convert", "convert")],
        );
        let arity_diff = diff_component_interfaces(&previous, &current);
        assert_eq!(arity_diff.compatibility, ComponentCompatibility::Breaking);
        assert!(matches!(
            arity_diff.changes.as_slice(),
            [ComponentCompatibilityChange::ParameterAdded { position: 1, .. }]
        ));

        let mut changed_profile = previous.metadata.clone();
        changed_profile.abi_profile = "canonical-v2-sync".to_string();
        let profile_diff = diff_component_metadata(&previous.metadata, &changed_profile);
        assert_eq!(profile_diff.compatibility, ComponentCompatibility::Breaking);
        assert!(matches!(
            profile_diff.changes.as_slice(),
            [ComponentCompatibilityChange::AbiProfileChanged { .. }]
        ));
    }

    #[test]
    fn record_enum_and_variant_shape_changes_are_breaking() {
        let record_previous = artifact(
            r#"
pub record Profile {
    id: Int32,
    label: String
}
pub fun keep: (value: Profile) -> Profile = { value }
"#,
            vec![ComponentExportSelection::new("api", "keep", "keep")],
        );
        let record_current = artifact(
            r#"
pub record Profile {
    id: Int64,
    label: String
}
pub fun keep: (value: Profile) -> Profile = { value }
"#,
            vec![ComponentExportSelection::new("api", "keep", "keep")],
        );
        let record_diff = diff_component_interfaces(&record_previous, &record_current);
        assert_eq!(record_diff.compatibility, ComponentCompatibility::Breaking);
        assert!(matches!(
            record_diff.changes.as_slice(),
            [ComponentCompatibilityChange::TypeChanged { ty, .. }] if ty == "profile"
        ));

        let enum_previous = artifact(
            r#"
pub enum Status { Ready Failed }
pub fun keep: (value: Status) -> Status = { value }
"#,
            vec![ComponentExportSelection::new("api", "keep", "keep")],
        );
        let enum_current = artifact(
            r#"
pub enum Status { Ready Failed Pending }
pub fun keep: (value: Status) -> Status = { value }
"#,
            vec![ComponentExportSelection::new("api", "keep", "keep")],
        );
        let enum_diff = diff_component_interfaces(&enum_previous, &enum_current);
        assert_eq!(enum_diff.compatibility, ComponentCompatibility::Breaking);
        assert!(matches!(
            enum_diff.changes.as_slice(),
            [ComponentCompatibilityChange::TypeChanged { ty, .. }] if ty == "status"
        ));

        let variant_previous = artifact(
            r#"
pub enum Event { Idle Message(String) }
pub fun keep: (value: Event) -> Event = { value }
"#,
            vec![ComponentExportSelection::new("api", "keep", "keep")],
        );
        let variant_current = artifact(
            r#"
pub enum Event { Idle Message(Int32) }
pub fun keep: (value: Event) -> Event = { value }
"#,
            vec![ComponentExportSelection::new("api", "keep", "keep")],
        );
        let variant_diff = diff_component_interfaces(&variant_previous, &variant_current);
        assert_eq!(variant_diff.compatibility, ComponentCompatibility::Breaking);
        assert!(matches!(
            variant_diff.changes.as_slice(),
            [ComponentCompatibilityChange::TypeChanged { ty, .. }] if ty == "event"
        ));
    }

    #[test]
    fn first_resource_and_callable_additions_allow_only_the_scalar_to_rich_upgrade() {
        let mut previous = metadata(Vec::new(), Vec::new(), Vec::new());
        previous.implementation_subset = COMPONENT_SCALAR_IMPLEMENTATION_SUBSET.to_string();
        let current = metadata(
            vec![resource("counter", Vec::new())],
            vec![callable(
                "transform",
                vec![param("value", ComponentValueType::S32)],
                ComponentValueType::S32,
                ComponentCallableMultiplicityIr::Once,
            )],
            Vec::new(),
        );

        let upgrade = diff_component_metadata(&previous, &current);
        assert_eq!(upgrade.compatibility, ComponentCompatibility::NonBreaking);
        assert_eq!(
            upgrade.changes,
            vec![
                ComponentCompatibilityChange::ImplementationSubsetChanged {
                    previous: COMPONENT_SCALAR_IMPLEMENTATION_SUBSET.to_string(),
                    current: COMPONENT_RICH_INTERFACE_SUBSET.to_string(),
                },
                ComponentCompatibilityChange::ResourceAdded {
                    interface: "api".to_string(),
                    resource: "counter".to_string(),
                },
                ComponentCompatibilityChange::CallableAdded {
                    interface: "api".to_string(),
                    callable: "transform".to_string(),
                },
            ]
        );

        let downgrade = diff_component_metadata(&current, &previous);
        assert_eq!(downgrade.compatibility, ComponentCompatibility::Breaking);
        assert!(matches!(
            downgrade.changes.first(),
            Some(ComponentCompatibilityChange::ImplementationSubsetChanged {
                previous,
                current,
            }) if previous == COMPONENT_RICH_INTERFACE_SUBSET
                && current == COMPONENT_SCALAR_IMPLEMENTATION_SUBSET
        ));

        let mut unknown = current.clone();
        unknown.implementation_subset = "restrict-component-interface-future-v9".to_string();
        let unknown_diff = diff_component_metadata(&previous, &unknown);
        assert_eq!(
            unknown_diff.compatibility,
            ComponentCompatibility::Breaking,
            "unknown subset transitions must fail closed even when the WIT changes are additive"
        );
    }

    #[test]
    fn resource_callable_and_method_additions_are_non_breaking_in_stable_path_order() {
        let read = method(
            "read",
            ComponentResourceReceiverIr::Borrow,
            Vec::new(),
            ComponentValueType::S32,
        );
        let previous = metadata(
            vec![resource("counter", vec![read.clone()])],
            Vec::new(),
            Vec::new(),
        );
        let current = metadata(
            vec![
                resource("session", Vec::new()),
                resource(
                    "counter",
                    vec![
                        read,
                        method(
                            "finish",
                            ComponentResourceReceiverIr::Own,
                            Vec::new(),
                            ComponentValueType::S32,
                        ),
                    ],
                ),
            ],
            vec![callable(
                "transform",
                vec![param("value", ComponentValueType::S32)],
                ComponentValueType::S32,
                ComponentCallableMultiplicityIr::Once,
            )],
            Vec::new(),
        );

        let diff = diff_component_metadata(&previous, &current);
        assert_eq!(diff.compatibility, ComponentCompatibility::NonBreaking);
        assert_eq!(
            diff.changes,
            vec![
                ComponentCompatibilityChange::ResourceMethodAdded {
                    interface: "api".to_string(),
                    resource: "counter".to_string(),
                    method: "finish".to_string(),
                },
                ComponentCompatibilityChange::ResourceAdded {
                    interface: "api".to_string(),
                    resource: "session".to_string(),
                },
                ComponentCompatibilityChange::CallableAdded {
                    interface: "api".to_string(),
                    callable: "transform".to_string(),
                },
            ]
        );
    }

    #[test]
    fn resource_callable_and_method_removals_are_breaking_in_stable_path_order() {
        let finish = method(
            "finish",
            ComponentResourceReceiverIr::Own,
            Vec::new(),
            ComponentValueType::S32,
        );
        let previous = metadata(
            vec![
                resource("session", Vec::new()),
                resource(
                    "counter",
                    vec![
                        method(
                            "read",
                            ComponentResourceReceiverIr::Borrow,
                            Vec::new(),
                            ComponentValueType::S32,
                        ),
                        finish.clone(),
                    ],
                ),
            ],
            vec![callable(
                "transform",
                vec![param("value", ComponentValueType::S32)],
                ComponentValueType::S32,
                ComponentCallableMultiplicityIr::Once,
            )],
            Vec::new(),
        );
        let current = metadata(
            vec![resource("counter", vec![finish])],
            Vec::new(),
            Vec::new(),
        );

        let diff = diff_component_metadata(&previous, &current);
        assert_eq!(diff.compatibility, ComponentCompatibility::Breaking);
        assert_eq!(
            diff.changes,
            vec![
                ComponentCompatibilityChange::ResourceMethodRemoved {
                    interface: "api".to_string(),
                    resource: "counter".to_string(),
                    method: "read".to_string(),
                },
                ComponentCompatibilityChange::ResourceRemoved {
                    interface: "api".to_string(),
                    resource: "session".to_string(),
                },
                ComponentCompatibilityChange::CallableRemoved {
                    interface: "api".to_string(),
                    callable: "transform".to_string(),
                },
            ]
        );
    }

    #[test]
    fn resource_receiver_signature_and_owned_handle_target_changes_are_breaking() {
        let previous_method = method(
            "replace",
            ComponentResourceReceiverIr::Borrow,
            vec![param(
                "target",
                ComponentValueType::OwnResource("session".to_string()),
            )],
            ComponentValueType::OwnCallable("transform".to_string()),
        );
        let current_method = method(
            "replace",
            ComponentResourceReceiverIr::Own,
            vec![param(
                "target",
                ComponentValueType::OwnResource("database".to_string()),
            )],
            ComponentValueType::OwnCallable("mapper".to_string()),
        );
        let previous = metadata(
            vec![resource("counter", vec![previous_method.clone()])],
            Vec::new(),
            Vec::new(),
        );
        let current = metadata(
            vec![resource("counter", vec![current_method.clone()])],
            Vec::new(),
            Vec::new(),
        );

        let diff = diff_component_metadata(&previous, &current);
        assert_eq!(diff.compatibility, ComponentCompatibility::Breaking);
        assert_eq!(
            diff.changes,
            vec![
                ComponentCompatibilityChange::ResourceMethodReceiverChanged {
                    interface: "api".to_string(),
                    resource: "counter".to_string(),
                    method: "replace".to_string(),
                    previous: ComponentResourceReceiverIr::Borrow,
                    current: ComponentResourceReceiverIr::Own,
                },
                ComponentCompatibilityChange::ResourceMethodSignatureChanged {
                    interface: "api".to_string(),
                    resource: "counter".to_string(),
                    method: "replace".to_string(),
                    previous_params: previous_method.params,
                    current_params: current_method.params,
                    previous_result: previous_method.result,
                    current_result: current_method.result,
                },
            ]
        );
    }

    #[test]
    fn callable_signature_owned_handle_target_and_multiplicity_changes_are_breaking() {
        let previous_callable = callable(
            "transform",
            vec![param(
                "target",
                ComponentValueType::OwnResource("session".to_string()),
            )],
            ComponentValueType::OwnCallable("continuation".to_string()),
            ComponentCallableMultiplicityIr::Once,
        );
        let current_callable = callable(
            "transform",
            vec![param(
                "target",
                ComponentValueType::OwnResource("database".to_string()),
            )],
            ComponentValueType::OwnCallable("mapper".to_string()),
            ComponentCallableMultiplicityIr::Many,
        );
        let previous = metadata(Vec::new(), vec![previous_callable.clone()], Vec::new());
        let current = metadata(Vec::new(), vec![current_callable.clone()], Vec::new());

        let diff = diff_component_metadata(&previous, &current);
        assert_eq!(diff.compatibility, ComponentCompatibility::Breaking);
        assert_eq!(
            diff.changes,
            vec![
                ComponentCompatibilityChange::CallableSignatureChanged {
                    interface: "api".to_string(),
                    callable: "transform".to_string(),
                    previous_params: previous_callable.params,
                    current_params: current_callable.params,
                    previous_result: previous_callable.result,
                    current_result: current_callable.result,
                },
                ComponentCompatibilityChange::CallableMultiplicityChanged {
                    interface: "api".to_string(),
                    callable: "transform".to_string(),
                    previous: ComponentCallableMultiplicityIr::Once,
                    current: ComponentCallableMultiplicityIr::Many,
                },
            ]
        );
    }

    #[test]
    fn fixed_array_length_change_is_breaking_despite_identical_wit() {
        let previous = artifact(
            "pub fun keep: (value: Array<Int32, 2>) -> Array<Int32, 2> = { value }",
            vec![ComponentExportSelection::new("api", "keep", "keep")],
        );
        let current = artifact(
            "pub fun keep: (value: Array<Int32, 3>) -> Array<Int32, 3> = { value }",
            vec![ComponentExportSelection::new("api", "keep", "keep")],
        );

        assert_eq!(previous.normalized_wit, current.normalized_wit);
        let diff = diff_component_interfaces(&previous, &current);
        assert_eq!(diff.compatibility, ComponentCompatibility::Breaking);
        assert!(matches!(
            diff.changes.as_slice(),
            [
                ComponentCompatibilityChange::ParameterTypeChanged { .. },
                ComponentCompatibilityChange::ResultTypeChanged { .. }
            ]
        ));
    }
}
