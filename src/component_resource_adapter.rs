//! Standard32 adapters for Restrict-owned Component resources.
//!
//! This layer is deliberately separate from the ordinary value adapter. A
//! resource representation and a closure environment must survive the
//! invocation arena that produced it, while Canonical ABI own handles are
//! allocated and generation-checked by the Component runtime. Factories run
//! with the compiler-reserved resource arena selected, then wrap the returned
//! representation with the exact `resource-new` intrinsic resolved from WIT.
//!
//! The persistent arena is shared by all live resources and once-callables.
//! It is reset only after the last generated destructor runs. This is safe for
//! nested synchronous calls and bounds sequential create/drop stress without
//! exposing the arena layout in WIT. Borrow methods receive the persistent
//! representation directly and use it only for one synchronous call; the
//! generated adapter never transfers, drops, stores, or returns that borrow.

use crate::component::{
    ComponentCallableIr, ComponentFunctionIr, ComponentInterfaceArtifact, ComponentInterfaceDeclIr,
    ComponentResourceIr, ComponentResourceReceiverIr, ComponentValueType,
};
use crate::component_resource_plan::{ComponentResourcePlan, PlannedComponentBoundaryType};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use wit_parser::abi::{AbiVariant, WasmSignature, WasmType};
use wit_parser::{
    Function, FunctionKind, ManglingAndAbi, Resolve, ResourceIntrinsic, TypeDefKind, WasmExport,
    WasmExportKind, WasmImport, WorldId, WorldItem, WorldKey,
};

/// Arena locations owned by the compiler's Component build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentResourceAdapterLayout {
    /// Reset-after-call arena already used by the ordinary Canonical adapter.
    pub invocation_arena_addr: u32,
    /// Instance-lifetime arena reserved exclusively for resources/callables.
    pub resource_arena_addr: u32,
}

/// Typed failures from resource adapter generation.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ComponentResourceAdapterError {
    #[error("generated WIT could not be resolved for resource adapter generation: {0}")]
    WitResolution(String),
    #[error("selected WIT world '{0}' was not found for resource adapter generation")]
    MissingWorld(String),
    #[error("world export '{0}' is not a named interface")]
    UnsupportedWorldExport(String),
    #[error("Component resource plan and interface IR disagree at '{0}'")]
    PlanInterfaceMismatch(String),
    #[error("WIT interface '{0}' was not found in the selected world")]
    MissingWitInterface(String),
    #[error("WIT resource '{interface}.{resource}' was not found")]
    MissingWitResource { interface: String, resource: String },
    #[error("WIT function '{interface}.{function}' was not found")]
    MissingWitFunction { interface: String, function: String },
    #[error("unsupported resource adapter boundary '{boundary}': {reason}")]
    UnsupportedBoundary { boundary: String, reason: String },
    #[error("unsafe Core WAT source symbol '{0}'")]
    UnsafeCoreSymbol(String),
    #[error("generated Core WAT has no module declaration")]
    MissingModuleDeclaration,
    #[error("generated Core WAT has no final module delimiter")]
    MissingModuleDelimiter,
}

#[derive(Debug)]
struct ResolvedInterface<'a> {
    key: &'a WorldKey,
    ir: &'a ComponentInterfaceDeclIr,
    id: wit_parser::InterfaceId,
}

#[derive(Debug, Clone)]
struct ResourceNames {
    index: usize,
    dtor_export: String,
}

#[derive(Debug, Clone, Copy)]
struct FactoryEmission {
    adapter_index: usize,
    resource_arena_addr: u32,
}

/// Inject resource/callable imports, lifecycle runtime, and Canonical exports
/// into an existing Restrict Core WAT module.
///
/// The ordinary value adapter must skip functions whose result is
/// `OwnResource`/`OwnCallable` and every resource/callable method: this function
/// owns those Canonical export names. At present factory/method/call signatures
/// are limited to Core scalar values and unit. Rich factory arguments must be
/// lifted while the resource arena is current so captured pointers cannot
/// dangle; until that shared lift seam is available they fail closed here.
pub fn generate_component_resource_adapter_core_wat(
    core_wat: &str,
    interface: &ComponentInterfaceArtifact,
    plan: &ComponentResourcePlan,
    layout: ComponentResourceAdapterLayout,
) -> Result<String, ComponentResourceAdapterError> {
    let (resolve, world) = resolve_interface(interface)?;
    let resolved_interfaces = exported_interfaces(&resolve, world, interface)?;
    validate_plan(interface, plan)?;

    let mut imports = String::new();
    let mut adapters = String::new();
    adapters.push_str("\n  ;; Component resource persistent runtime\n");
    emit_resource_runtime(&mut adapters, layout.resource_arena_addr);

    let mut resource_names = BTreeMap::<(String, String), ResourceNames>::new();
    let mut import_index = 0usize;
    for resolved in &resolved_interfaces {
        for resource in &resolved.ir.resources {
            let names = resolve_resource_names(
                &resolve,
                resolved,
                &resource.name,
                import_index,
                &mut imports,
            )?;
            resource_names.insert((resolved.ir.name.clone(), resource.name.clone()), names);
            import_index += 1;
        }
        for callable in &resolved.ir.callables {
            let names = resolve_resource_names(
                &resolve,
                resolved,
                &callable.name,
                import_index,
                &mut imports,
            )?;
            resource_names.insert((resolved.ir.name.clone(), callable.name.clone()), names);
            import_index += 1;
        }
    }

    adapters.push_str("\n  ;; Component resource factories\n");
    let mut adapter_index = 0usize;
    for resolved in &resolved_interfaces {
        for function_ir in &resolved.ir.functions {
            let target = match &function_ir.result {
                ComponentValueType::OwnResource(name) => Some((name, false)),
                ComponentValueType::OwnCallable(name) => Some((name, true)),
                _ => None,
            };
            let Some((target_name, callable)) = target else {
                continue;
            };
            ensure_safe_core_symbol(&function_ir.source_function)?;
            let wit_function = find_function(&resolve, resolved, &function_ir.name, |kind| {
                matches!(kind, FunctionKind::Freestanding)
            })?;
            let names = resource_names
                .get(&(resolved.ir.name.clone(), target_name.clone()))
                .ok_or_else(|| {
                    ComponentResourceAdapterError::PlanInterfaceMismatch(format!(
                        "{}.{} returns undeclared handle '{}'",
                        resolved.ir.name, function_ir.name, target_name
                    ))
                })?;
            validate_factory_plan(
                plan,
                resolved.ir,
                function_ir.name.as_str(),
                target_name,
                callable,
            )?;
            emit_factory(
                &mut adapters,
                &resolve,
                resolved,
                wit_function,
                function_ir,
                names,
                FactoryEmission {
                    adapter_index,
                    resource_arena_addr: layout.resource_arena_addr,
                },
            )?;
            adapter_index += 1;
        }
    }

    adapters.push_str("\n  ;; Component resource methods and destructors\n");
    for resolved in &resolved_interfaces {
        for resource in &resolved.ir.resources {
            ensure_safe_core_symbol(&resource.destructor_source_function)?;
            let names = &resource_names[&(resolved.ir.name.clone(), resource.name.clone())];
            emit_resource_methods(
                &mut adapters,
                &resolve,
                resolved,
                resource,
                names,
                &mut adapter_index,
                layout.invocation_arena_addr,
            )?;
            emit_resource_destructor(
                &mut adapters,
                resource,
                names,
                adapter_index,
                layout.invocation_arena_addr,
            );
            adapter_index += 1;
        }
        for callable in &resolved.ir.callables {
            let names = &resource_names[&(resolved.ir.name.clone(), callable.name.clone())];
            emit_callable(
                &mut adapters,
                &resolve,
                resolved,
                callable,
                names,
                &mut adapter_index,
                layout.invocation_arena_addr,
            )?;
        }
    }

    inject_wat(core_wat, &imports, &adapters)
}

fn resolve_interface(
    artifact: &ComponentInterfaceArtifact,
) -> Result<(Resolve, WorldId), ComponentResourceAdapterError> {
    let mut resolve = Resolve::default();
    let package = resolve
        .push_str(
            "restrict-generated-component-resource-interface.wit",
            &artifact.normalized_wit,
        )
        .map_err(|error| ComponentResourceAdapterError::WitResolution(error.to_string()))?;
    let world = resolve
        .select_world(&[package], Some(&artifact.ir.world.name))
        .map_err(|_| ComponentResourceAdapterError::MissingWorld(artifact.ir.world.name.clone()))?;
    Ok((resolve, world))
}

fn exported_interfaces<'a>(
    resolve: &'a Resolve,
    world: WorldId,
    artifact: &'a ComponentInterfaceArtifact,
) -> Result<Vec<ResolvedInterface<'a>>, ComponentResourceAdapterError> {
    let ir_by_name = artifact
        .ir
        .interfaces
        .iter()
        .map(|interface| (interface.name.as_str(), interface))
        .collect::<BTreeMap<_, _>>();
    let mut result = Vec::new();
    for (key, item) in &resolve.worlds[world].exports {
        let WorldItem::Interface { id, .. } = item else {
            return Err(ComponentResourceAdapterError::UnsupportedWorldExport(
                key_name(resolve, key),
            ));
        };
        let name = resolve.interfaces[*id].name.as_deref().ok_or_else(|| {
            ComponentResourceAdapterError::UnsupportedWorldExport(key_name(resolve, key))
        })?;
        let ir = ir_by_name
            .get(name)
            .copied()
            .ok_or_else(|| ComponentResourceAdapterError::MissingWitInterface(name.to_string()))?;
        result.push(ResolvedInterface { key, ir, id: *id });
    }
    result.sort_by(|left, right| left.ir.name.cmp(&right.ir.name));
    Ok(result)
}

fn validate_plan(
    artifact: &ComponentInterfaceArtifact,
    plan: &ComponentResourcePlan,
) -> Result<(), ComponentResourceAdapterError> {
    let ir_resources = artifact
        .ir
        .interfaces
        .iter()
        .flat_map(|interface| {
            interface.resources.iter().map(move |resource| {
                (
                    interface.name.as_str(),
                    resource.name.as_str(),
                    resource.source_record.as_str(),
                )
            })
        })
        .collect::<BTreeSet<_>>();
    let planned_resources = plan
        .resources
        .iter()
        .map(|resource| {
            (
                resource.identity.interface.as_str(),
                resource.identity.external_name.as_str(),
                resource.identity.source_record.as_str(),
            )
        })
        .collect::<BTreeSet<_>>();
    if ir_resources != planned_resources {
        return Err(ComponentResourceAdapterError::PlanInterfaceMismatch(
            "resource declaration set".to_string(),
        ));
    }
    let ir_callables = artifact
        .ir
        .interfaces
        .iter()
        .flat_map(|interface| {
            interface
                .callables
                .iter()
                .map(move |callable| (interface.name.as_str(), callable.name.as_str()))
        })
        .collect::<BTreeSet<_>>();
    let planned_callables = plan
        .callables
        .iter()
        .map(|callable| {
            (
                callable.identity.interface.as_str(),
                callable.identity.external_name.as_str(),
            )
        })
        .collect::<BTreeSet<_>>();
    if ir_callables != planned_callables {
        return Err(ComponentResourceAdapterError::PlanInterfaceMismatch(
            "callable declaration set".to_string(),
        ));
    }

    for interface in &artifact.ir.interfaces {
        for resource in &interface.resources {
            let planned = plan
                .resources
                .iter()
                .find(|planned| {
                    planned.identity.interface == interface.name
                        && planned.identity.external_name == resource.name
                })
                .ok_or_else(|| {
                    ComponentResourceAdapterError::PlanInterfaceMismatch(format!(
                        "resource {}.{}",
                        interface.name, resource.name
                    ))
                })?;
            if resource.destructor_source_function != planned.destructor.source_function
                || resource.methods.len() != planned.methods.len()
            {
                return Err(ComponentResourceAdapterError::PlanInterfaceMismatch(
                    format!("resource lifecycle {}.{}", interface.name, resource.name),
                ));
            }
            for method in &resource.methods {
                let planned_method = planned
                    .methods
                    .iter()
                    .find(|planned| planned.external_name == method.name)
                    .ok_or_else(|| {
                        ComponentResourceAdapterError::PlanInterfaceMismatch(format!(
                            "resource method {}.{}.{}",
                            interface.name, resource.name, method.name
                        ))
                    })?;
                let receiver_matches = matches!(
                    (method.receiver, planned_method.receiver),
                    (
                        ComponentResourceReceiverIr::Borrow,
                        crate::component_selection_v2::ComponentResourceReceiver::Borrow
                    ) | (
                        ComponentResourceReceiverIr::Own,
                        crate::component_selection_v2::ComponentResourceReceiver::Own
                    )
                );
                if method.source_function != planned_method.source_function
                    || method.receiver_name != planned_method.receiver_source_name
                    || !receiver_matches
                    || !param_provenance_matches(&method.params, &planned_method.params)
                {
                    return Err(ComponentResourceAdapterError::PlanInterfaceMismatch(
                        format!(
                            "resource method contract {}.{}.{}",
                            interface.name, resource.name, method.name
                        ),
                    ));
                }
            }
        }
        for callable in &interface.callables {
            let planned = plan
                .callables
                .iter()
                .find(|planned| {
                    planned.identity.interface == interface.name
                        && planned.identity.external_name == callable.name
                })
                .ok_or_else(|| {
                    ComponentResourceAdapterError::PlanInterfaceMismatch(format!(
                        "callable {}.{}",
                        interface.name, callable.name
                    ))
                })?;
            if callable.multiplicity != crate::component::ComponentCallableMultiplicityIr::Once
                || planned.multiplicity
                    != crate::component_selection_v2::ComponentCallableMultiplicity::Once
                || !param_provenance_matches(&callable.params, &planned.params)
            {
                return Err(ComponentResourceAdapterError::PlanInterfaceMismatch(
                    format!("callable contract {}.{}", interface.name, callable.name),
                ));
            }
        }
    }
    Ok(())
}

fn param_provenance_matches(
    ir: &[crate::component::ComponentParamIr],
    planned: &[crate::component_resource_plan::PlannedComponentParam],
) -> bool {
    ir.len() == planned.len()
        && ir.iter().zip(planned).all(|(ir, planned)| {
            ir.source_index == planned.source_index && ir.source_name == planned.source_name
        })
}

fn validate_factory_plan(
    plan: &ComponentResourcePlan,
    interface: &ComponentInterfaceDeclIr,
    function: &str,
    target: &str,
    callable: bool,
) -> Result<(), ComponentResourceAdapterError> {
    let export = plan.export(&interface.name, function).ok_or_else(|| {
        ComponentResourceAdapterError::PlanInterfaceMismatch(format!(
            "factory {}.{}",
            interface.name, function
        ))
    })?;
    let matches = match &export.result {
        PlannedComponentBoundaryType::OwnResource(identity) if !callable => {
            identity.interface == interface.name && identity.external_name == target
        }
        PlannedComponentBoundaryType::OwnCallable(identity) if callable => {
            identity.interface == interface.name && identity.external_name == target
        }
        _ => false,
    };
    if !matches {
        return Err(ComponentResourceAdapterError::PlanInterfaceMismatch(
            format!("factory {}.{} result", interface.name, function),
        ));
    }
    Ok(())
}

fn resolve_resource_names(
    resolve: &Resolve,
    interface: &ResolvedInterface<'_>,
    resource_name: &str,
    index: usize,
    imports: &mut String,
) -> Result<ResourceNames, ComponentResourceAdapterError> {
    let resource = resolve.interfaces[interface.id]
        .types
        .get(resource_name)
        .copied()
        .filter(|id| matches!(resolve.types[*id].kind, TypeDefKind::Resource))
        .ok_or_else(|| ComponentResourceAdapterError::MissingWitResource {
            interface: interface.ir.name.clone(),
            resource: resource_name.to_string(),
        })?;
    let import_name = |intrinsic| {
        resolve.wasm_import_name(
            ManglingAndAbi::Standard32,
            WasmImport::ResourceIntrinsic {
                interface: Some(interface.key),
                resource,
                intrinsic,
            },
        )
    };
    let (new_module, new_name) = import_name(ResourceIntrinsic::ExportedNew);
    let (rep_module, rep_name) = import_name(ResourceIntrinsic::ExportedRep);
    let (drop_module, drop_name) = import_name(ResourceIntrinsic::ExportedDrop);
    for (suffix, module, name, signature) in [
        ("new", &new_module, &new_name, "(param i32) (result i32)"),
        ("rep", &rep_module, &rep_name, "(param i32) (result i32)"),
        ("drop", &drop_module, &drop_name, "(param i32)"),
    ] {
        imports.push_str("\n  (import \"");
        append_wat_string_contents(imports, module);
        imports.push_str("\" \"");
        append_wat_string_contents(imports, name);
        imports.push_str(&format!(
            "\" (func $__restrict_component_resource_{index}_{suffix} {signature}))\n"
        ));
    }
    let dtor_export = resolve.wasm_export_name(
        ManglingAndAbi::Standard32,
        WasmExport::ResourceDtor {
            interface: interface.key,
            resource,
        },
    );
    Ok(ResourceNames { index, dtor_export })
}

fn find_function<'a>(
    resolve: &'a Resolve,
    interface: &ResolvedInterface<'_>,
    item_name: &str,
    kind: impl Fn(&FunctionKind) -> bool,
) -> Result<&'a Function, ComponentResourceAdapterError> {
    resolve.interfaces[interface.id]
        .functions
        .values()
        .find(|function| function.item_name() == item_name && kind(&function.kind))
        .ok_or_else(|| ComponentResourceAdapterError::MissingWitFunction {
            interface: interface.ir.name.clone(),
            function: item_name.to_string(),
        })
}

fn emit_resource_runtime(output: &mut String, resource_arena_addr: u32) {
    output.push_str(
        "  (global $__restrict_component_resource_arena_ready (mut i32) (i32.const 0))\n",
    );
    output
        .push_str("  (global $__restrict_component_resource_live_count (mut i32) (i32.const 0))\n");
    output.push_str("  (func $__restrict_component_resource_ensure_arena\n");
    output.push_str("    global.get $__restrict_component_resource_arena_ready\n");
    output.push_str("    i32.eqz\n");
    output.push_str("    (if (then\n");
    output.push_str(&format!("      i32.const {resource_arena_addr}\n"));
    output.push_str("      call $arena_init\n");
    output.push_str("      drop\n");
    output.push_str("      i32.const 1\n");
    output.push_str("      global.set $__restrict_component_resource_arena_ready\n");
    output.push_str("    ))\n");
    output.push_str("  )\n");
    output.push_str("  (func $__restrict_component_resource_retain\n");
    output.push_str("    global.get $__restrict_component_resource_live_count\n");
    output.push_str("    i32.const -1\n");
    output.push_str("    i32.eq\n");
    output.push_str("    (if (then unreachable))\n");
    output.push_str("    global.get $__restrict_component_resource_live_count\n");
    output.push_str("    i32.const 1\n");
    output.push_str("    i32.add\n");
    output.push_str("    global.set $__restrict_component_resource_live_count\n");
    output.push_str("  )\n");
    output.push_str("  (func $__restrict_component_resource_release\n");
    output.push_str("    global.get $__restrict_component_resource_live_count\n");
    output.push_str("    i32.eqz\n");
    output.push_str("    (if (then unreachable))\n");
    output.push_str("    global.get $__restrict_component_resource_live_count\n");
    output.push_str("    i32.const 1\n");
    output.push_str("    i32.sub\n");
    output.push_str("    global.set $__restrict_component_resource_live_count\n");
    output.push_str("    global.get $__restrict_component_resource_live_count\n");
    output.push_str("    i32.eqz\n");
    output.push_str("    (if (then\n");
    output.push_str(&format!("      i32.const {resource_arena_addr}\n"));
    output.push_str("      call $arena_reset\n");
    output.push_str("      i32.const 0\n");
    output.push_str("      global.set $__restrict_component_resource_arena_ready\n");
    output.push_str("    ))\n");
    output.push_str("  )\n");
}

fn emit_factory(
    output: &mut String,
    resolve: &Resolve,
    interface: &ResolvedInterface<'_>,
    wit_function: &Function,
    function: &ComponentFunctionIr,
    names: &ResourceNames,
    emission: FactoryEmission,
) -> Result<(), ComponentResourceAdapterError> {
    let boundary = format!("{}.{}", interface.ir.name, function.name);
    ensure_scalar_params(
        &boundary,
        &function
            .params
            .iter()
            .map(|param| &param.ty)
            .collect::<Vec<_>>(),
    )?;
    let signature = resolve.wasm_signature(AbiVariant::GuestExport, wit_function);
    validate_scalar_signature(&boundary, &signature, &function.params, None, true)?;
    let canonical_name = resolve.wasm_export_name(
        ManglingAndAbi::Standard32,
        WasmExport::Func {
            interface: Some(interface.key),
            func: wit_function,
            kind: WasmExportKind::Normal,
        },
    );
    let adapter_name = format!(
        "__restrict_component_resource_factory_{}",
        emission.adapter_index
    );
    emit_function_header(output, &adapter_name, &signature);
    output.push_str("    (local $resource_prev_arena i32)\n");
    output.push_str("    (local $resource_rep i32)\n");
    output.push_str("    (local $resource_own i32)\n");
    output.push_str("    call $__restrict_component_resource_ensure_arena\n");
    output.push_str("    global.get $current_arena\n");
    output.push_str("    local.set $resource_prev_arena\n");
    output.push_str(&format!("    i32.const {}\n", emission.resource_arena_addr));
    output.push_str("    global.set $current_arena\n");
    emit_source_args(output, &function.params, 0);
    output.push_str(&format!("    call ${}\n", function.source_function));
    output.push_str("    local.set $resource_rep\n");
    output.push_str("    local.get $resource_prev_arena\n");
    output.push_str("    global.set $current_arena\n");
    output.push_str("    local.get $resource_rep\n");
    output.push_str(&format!(
        "    call $__restrict_component_resource_{}_new\n",
        names.index
    ));
    output.push_str("    local.set $resource_own\n");
    output.push_str("    call $__restrict_component_resource_retain\n");
    output.push_str("    local.get $resource_own\n");
    output.push_str("  )\n");
    emit_export(output, &canonical_name, &adapter_name);
    Ok(())
}

fn emit_resource_methods(
    output: &mut String,
    resolve: &Resolve,
    interface: &ResolvedInterface<'_>,
    resource: &ComponentResourceIr,
    names: &ResourceNames,
    adapter_index: &mut usize,
    invocation_arena_addr: u32,
) -> Result<(), ComponentResourceAdapterError> {
    let resource_id = resource_id(resolve, interface, &resource.name)?;
    for method in &resource.methods {
        ensure_safe_core_symbol(&method.source_function)?;
        let expected_kind = |kind: &FunctionKind| match (method.receiver, kind) {
            (ComponentResourceReceiverIr::Borrow, FunctionKind::Method(id)) => *id == resource_id,
            (ComponentResourceReceiverIr::Own, FunctionKind::Static(id)) => *id == resource_id,
            _ => false,
        };
        let wit_function = find_function(resolve, interface, &method.name, expected_kind)?;
        let boundary = format!("{}.{}.{}", interface.ir.name, resource.name, method.name);
        ensure_scalar_params(
            &boundary,
            &method
                .params
                .iter()
                .map(|param| &param.ty)
                .collect::<Vec<_>>(),
        )?;
        ensure_scalar_result(&boundary, &method.result)?;
        let signature = resolve.wasm_signature(AbiVariant::GuestExport, wit_function);
        validate_scalar_signature(
            &boundary,
            &signature,
            &method.params,
            Some(&method.result),
            false,
        )?;
        let canonical_name = resolve.wasm_export_name(
            ManglingAndAbi::Standard32,
            WasmExport::Func {
                interface: Some(interface.key),
                func: wit_function,
                kind: WasmExportKind::Normal,
            },
        );
        let adapter_name = format!("__restrict_component_resource_method_{}", *adapter_index);
        emit_function_header(output, &adapter_name, &signature);
        emit_invocation_locals(output);
        output.push_str("    (local $resource_rep i32)\n");
        if scalar_wasm_type(&method.result)?.is_some() {
            output.push_str(&format!(
                "    (local $resource_result {})\n",
                scalar_wasm_type(&method.result)?.unwrap()
            ));
        }
        match method.receiver {
            ComponentResourceReceiverIr::Borrow => {
                output.push_str("    local.get $canonical_param_0\n");
                output.push_str("    local.set $resource_rep\n");
            }
            ComponentResourceReceiverIr::Own => {
                output.push_str("    local.get $canonical_param_0\n");
                output.push_str(&format!(
                    "    call $__restrict_component_resource_{}_rep\n",
                    names.index
                ));
                output.push_str("    local.set $resource_rep\n");
            }
        }
        emit_invocation_enter(output, invocation_arena_addr);
        output.push_str("    local.get $resource_rep\n");
        emit_source_args(output, &method.params, 1);
        output.push_str(&format!("    call ${}\n", method.source_function));
        if scalar_wasm_type(&method.result)?.is_some() {
            output.push_str("    local.set $resource_result\n");
        }
        emit_invocation_exit(output, invocation_arena_addr);
        if method.receiver == ComponentResourceReceiverIr::Own {
            output.push_str("    local.get $canonical_param_0\n");
            output.push_str(&format!(
                "    call $__restrict_component_resource_{}_drop\n",
                names.index
            ));
        }
        if scalar_wasm_type(&method.result)?.is_some() {
            output.push_str("    local.get $resource_result\n");
        }
        output.push_str("  )\n");
        emit_export(output, &canonical_name, &adapter_name);
        *adapter_index += 1;
    }
    Ok(())
}

fn emit_resource_destructor(
    output: &mut String,
    resource: &ComponentResourceIr,
    names: &ResourceNames,
    adapter_index: usize,
    invocation_arena_addr: u32,
) {
    let adapter_name = format!("__restrict_component_resource_dtor_{adapter_index}");
    output.push_str(&format!(
        "  (func ${adapter_name} (param $resource_rep i32)\n"
    ));
    emit_invocation_locals(output);
    emit_invocation_enter(output, invocation_arena_addr);
    output.push_str("    local.get $resource_rep\n");
    output.push_str(&format!(
        "    call ${}\n",
        resource.destructor_source_function
    ));
    emit_invocation_exit(output, invocation_arena_addr);
    output.push_str("    call $__restrict_component_resource_release\n");
    output.push_str("  )\n");
    emit_export(output, &names.dtor_export, &adapter_name);
}

fn emit_callable(
    output: &mut String,
    resolve: &Resolve,
    interface: &ResolvedInterface<'_>,
    callable: &ComponentCallableIr,
    names: &ResourceNames,
    adapter_index: &mut usize,
    invocation_arena_addr: u32,
) -> Result<(), ComponentResourceAdapterError> {
    let callable_id = resource_id(resolve, interface, &callable.name)?;
    let wit_function = find_function(
        resolve,
        interface,
        "call",
        |kind| matches!(kind, FunctionKind::Static(id) if *id == callable_id),
    )?;
    let boundary = format!("{}.{}.call", interface.ir.name, callable.name);
    ensure_scalar_params(
        &boundary,
        &callable
            .params
            .iter()
            .map(|param| &param.ty)
            .collect::<Vec<_>>(),
    )?;
    ensure_scalar_result(&boundary, &callable.result)?;
    let signature = resolve.wasm_signature(AbiVariant::GuestExport, wit_function);
    validate_scalar_signature(
        &boundary,
        &signature,
        &callable.params,
        Some(&callable.result),
        false,
    )?;
    let canonical_name = resolve.wasm_export_name(
        ManglingAndAbi::Standard32,
        WasmExport::Func {
            interface: Some(interface.key),
            func: wit_function,
            kind: WasmExportKind::Normal,
        },
    );
    let adapter_name = format!("__restrict_component_callable_call_{}", *adapter_index);
    let closure_type_name = format!("__restrict_component_callable_type_{}", *adapter_index);
    emit_callable_closure_type(output, callable, &closure_type_name)?;
    emit_function_header(output, &adapter_name, &signature);
    emit_invocation_locals(output);
    output.push_str("    (local $resource_rep i32)\n");
    let source_result = scalar_wasm_type(&callable.result)?.unwrap_or("i32");
    if callable.result != ComponentValueType::Unit {
        output.push_str(&format!("    (local $resource_result {source_result})\n"));
    }
    output.push_str("    local.get $canonical_param_0\n");
    output.push_str(&format!(
        "    call $__restrict_component_resource_{}_rep\n",
        names.index
    ));
    output.push_str("    local.set $resource_rep\n");
    emit_invocation_enter(output, invocation_arena_addr);
    emit_source_args(output, &callable.params, 1);
    output.push_str("    local.get $resource_rep\n");
    output.push_str("    local.get $resource_rep\n");
    output.push_str("    i32.load\n");
    output.push_str(&format!("    call_indirect (type ${closure_type_name})\n"));
    if callable.result == ComponentValueType::Unit {
        output.push_str("    drop\n");
    } else {
        output.push_str("    local.set $resource_result\n");
    }
    emit_invocation_exit(output, invocation_arena_addr);
    output.push_str("    local.get $canonical_param_0\n");
    output.push_str(&format!(
        "    call $__restrict_component_resource_{}_drop\n",
        names.index
    ));
    if callable.result != ComponentValueType::Unit {
        output.push_str("    local.get $resource_result\n");
    }
    output.push_str("  )\n");
    emit_export(output, &canonical_name, &adapter_name);
    *adapter_index += 1;

    let dtor_name = format!("__restrict_component_callable_dtor_{}", *adapter_index);
    output.push_str(&format!("  (func ${dtor_name} (param $resource_rep i32)\n"));
    output.push_str("    (local $capture_size i32)\n");
    output.push_str("    local.get $resource_rep\n");
    output.push_str("    i32.load offset=4\n");
    output.push_str("    local.set $capture_size\n");
    output.push_str("    local.get $resource_rep\n");
    output.push_str("    i32.const 0\n");
    output.push_str("    local.get $capture_size\n");
    output.push_str("    memory.fill\n");
    output.push_str("    call $__restrict_component_resource_release\n");
    output.push_str("  )\n");
    emit_export(output, &names.dtor_export, &dtor_name);
    *adapter_index += 1;
    Ok(())
}

fn resource_id(
    resolve: &Resolve,
    interface: &ResolvedInterface<'_>,
    resource_name: &str,
) -> Result<wit_parser::TypeId, ComponentResourceAdapterError> {
    resolve.interfaces[interface.id]
        .types
        .get(resource_name)
        .copied()
        .filter(|id| matches!(resolve.types[*id].kind, TypeDefKind::Resource))
        .ok_or_else(|| ComponentResourceAdapterError::MissingWitResource {
            interface: interface.ir.name.clone(),
            resource: resource_name.to_string(),
        })
}

fn validate_scalar_signature(
    boundary: &str,
    signature: &WasmSignature,
    params: &[crate::component::ComponentParamIr],
    result: Option<&ComponentValueType>,
    factory: bool,
) -> Result<(), ComponentResourceAdapterError> {
    let mut expected = Vec::new();
    if !factory {
        expected.push(WasmType::I32);
    }
    for param in params {
        if let Some(ty) = component_wasm_type(&param.ty)? {
            expected.push(ty);
        }
    }
    if !same_core_signature(&signature.params, &expected)
        || signature.indirect_params
        || signature.retptr
    {
        return Err(ComponentResourceAdapterError::UnsupportedBoundary {
            boundary: boundary.to_string(),
            reason: format!(
                "unexpected Standard32 signature {:?}; expected direct parameters {:?}",
                signature.params, expected
            ),
        });
    }
    let expected_results = if factory {
        vec![WasmType::I32]
    } else {
        component_wasm_type(result.expect("non-factory signature has a logical result"))?
            .into_iter()
            .collect::<Vec<_>>()
    };
    if !same_core_signature(&signature.results, &expected_results) {
        return Err(ComponentResourceAdapterError::UnsupportedBoundary {
            boundary: boundary.to_string(),
            reason: format!(
                "unexpected Standard32 results {:?}; expected direct results {:?}",
                signature.results, expected_results
            ),
        });
    }
    Ok(())
}

fn same_core_signature(actual: &[WasmType], expected: &[WasmType]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| wat_type(*actual) == wat_type(*expected))
}

fn ensure_scalar_params(
    boundary: &str,
    params: &[&ComponentValueType],
) -> Result<(), ComponentResourceAdapterError> {
    for param in params {
        component_wasm_type(param).map_err(|_| {
            ComponentResourceAdapterError::UnsupportedBoundary {
                boundary: boundary.to_string(),
                reason:
                    "rich resource/callable arguments require the shared persistent-arena lift seam"
                        .to_string(),
            }
        })?;
    }
    Ok(())
}

fn ensure_scalar_result(
    boundary: &str,
    result: &ComponentValueType,
) -> Result<(), ComponentResourceAdapterError> {
    component_wasm_type(result).map(|_| ()).map_err(|_| {
        ComponentResourceAdapterError::UnsupportedBoundary {
            boundary: boundary.to_string(),
            reason:
                "rich resource/callable results require the shared boundary-arena lowering seam"
                    .to_string(),
        }
    })
}

fn component_wasm_type(
    ty: &ComponentValueType,
) -> Result<Option<WasmType>, ComponentResourceAdapterError> {
    Ok(match ty {
        ComponentValueType::Unit => None,
        ComponentValueType::S32 | ComponentValueType::Bool | ComponentValueType::Char => {
            Some(WasmType::I32)
        }
        ComponentValueType::S64 => Some(WasmType::I64),
        ComponentValueType::F64 => Some(WasmType::F64),
        _ => {
            return Err(ComponentResourceAdapterError::UnsupportedBoundary {
                boundary: "value".to_string(),
                reason: format!("non-scalar Component value {ty:?}"),
            })
        }
    })
}

fn scalar_wasm_type(
    ty: &ComponentValueType,
) -> Result<Option<&'static str>, ComponentResourceAdapterError> {
    Ok(component_wasm_type(ty)?.map(wat_type))
}

fn emit_callable_closure_type(
    output: &mut String,
    callable: &ComponentCallableIr,
    callable_type_name: &str,
) -> Result<(), ComponentResourceAdapterError> {
    let params = callable
        .params
        .iter()
        .map(|param| Ok(component_wasm_type(&param.ty)?.unwrap_or(WasmType::I32)))
        .collect::<Result<Vec<_>, ComponentResourceAdapterError>>()?;
    let result = component_wasm_type(&callable.result)?.unwrap_or(WasmType::I32);
    output.push_str(&format!("  (type ${callable_type_name} (func"));
    for param in params {
        output.push_str(&format!(" (param {})", wat_type(param)));
    }
    output.push_str(" (param i32)");
    output.push_str(&format!(" (result {})))\n", wat_type(result)));
    Ok(())
}

fn emit_function_header(output: &mut String, name: &str, signature: &WasmSignature) {
    output.push_str(&format!("  (func ${name}"));
    for (index, ty) in signature.params.iter().enumerate() {
        output.push_str(&format!(
            " (param $canonical_param_{index} {})",
            wat_type(*ty)
        ));
    }
    for ty in &signature.results {
        output.push_str(&format!(" (result {})", wat_type(*ty)));
    }
    output.push('\n');
}

fn emit_source_args(
    output: &mut String,
    params: &[crate::component::ComponentParamIr],
    canonical_start: usize,
) {
    let mut canonical_index = canonical_start;
    for param in params {
        if param.ty == ComponentValueType::Unit {
            output.push_str("    i32.const 0\n");
        } else {
            output.push_str(&format!(
                "    local.get $canonical_param_{canonical_index}\n"
            ));
            canonical_index += 1;
        }
    }
}

fn emit_invocation_locals(output: &mut String) {
    output.push_str("    (local $adapter_prev_arena i32)\n");
    output.push_str("    (local $adapter_entry_depth i32)\n");
    output.push_str("    (local $adapter_arena_mark i32)\n");
}

fn emit_invocation_enter(output: &mut String, arena_addr: u32) {
    output.push_str("    global.get $current_arena\n");
    output.push_str("    local.set $adapter_prev_arena\n");
    output.push_str("    global.get $__restrict_component_adapter_depth\n");
    output.push_str("    local.tee $adapter_entry_depth\n");
    output.push_str("    i32.eqz\n");
    output.push_str("    (if\n");
    output.push_str("      (then\n");
    output.push_str(&format!("        i32.const {arena_addr}\n"));
    output.push_str("        call $arena_init\n");
    output.push_str("        drop\n");
    output.push_str("      )\n");
    output.push_str("      (else\n");
    output.push_str(&format!("        i32.const {}\n", arena_addr + 4));
    output.push_str("        i32.load\n");
    output.push_str("        local.set $adapter_arena_mark\n");
    output.push_str("      )\n");
    output.push_str("    )\n");
    output.push_str("    local.get $adapter_entry_depth\n");
    output.push_str("    i32.const 1\n");
    output.push_str("    i32.add\n");
    output.push_str("    global.set $__restrict_component_adapter_depth\n");
    output.push_str(&format!("    i32.const {arena_addr}\n"));
    output.push_str("    global.set $current_arena\n");
}

fn emit_invocation_exit(output: &mut String, arena_addr: u32) {
    output.push_str("    local.get $adapter_entry_depth\n");
    output.push_str("    global.set $__restrict_component_adapter_depth\n");
    output.push_str("    local.get $adapter_entry_depth\n");
    output.push_str("    i32.eqz\n");
    output.push_str("    (if\n");
    output.push_str("      (then\n");
    output.push_str(&format!("        i32.const {arena_addr}\n"));
    output.push_str("        call $arena_reset\n");
    output.push_str("      )\n");
    output.push_str("      (else\n");
    output.push_str(&format!("        i32.const {}\n", arena_addr + 4));
    output.push_str("        local.get $adapter_arena_mark\n");
    output.push_str("        i32.store\n");
    output.push_str("      )\n");
    output.push_str("    )\n");
    output.push_str("    local.get $adapter_prev_arena\n");
    output.push_str("    global.set $current_arena\n");
}

fn emit_export(output: &mut String, export: &str, function: &str) {
    output.push_str("  (export \"");
    append_wat_string_contents(output, export);
    output.push_str(&format!("\" (func ${function}))\n"));
}

fn wat_type(ty: WasmType) -> &'static str {
    match ty {
        WasmType::I32 | WasmType::Pointer | WasmType::Length => "i32",
        WasmType::I64 | WasmType::PointerOrI64 => "i64",
        WasmType::F32 => "f32",
        WasmType::F64 => "f64",
    }
}

fn ensure_safe_core_symbol(symbol: &str) -> Result<(), ComponentResourceAdapterError> {
    let safe = !symbol.is_empty()
        && symbol
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'));
    if safe {
        Ok(())
    } else {
        Err(ComponentResourceAdapterError::UnsafeCoreSymbol(
            symbol.to_string(),
        ))
    }
}

fn inject_wat(
    core_wat: &str,
    imports: &str,
    adapters: &str,
) -> Result<String, ComponentResourceAdapterError> {
    let module_start = core_wat
        .find("(module")
        .ok_or(ComponentResourceAdapterError::MissingModuleDeclaration)?
        + "(module".len();
    let module_end = core_wat
        .rfind(')')
        .filter(|end| *end >= module_start)
        .ok_or(ComponentResourceAdapterError::MissingModuleDelimiter)?;
    let mut output = String::with_capacity(core_wat.len() + imports.len() + adapters.len());
    output.push_str(&core_wat[..module_start]);
    output.push_str(imports);
    output.push_str(&core_wat[module_start..module_end]);
    output.push_str(adapters);
    output.push_str(&core_wat[module_end..]);
    Ok(output)
}

fn append_wat_string_contents(output: &mut String, value: &str) {
    for byte in value.bytes() {
        match byte {
            b'"' => output.push_str("\\\""),
            b'\\' => output.push_str("\\\\"),
            0x20..=0x7e => output.push(char::from(byte)),
            _ => output.push_str(&format!("\\{byte:02x}")),
        }
    }
}

fn key_name(resolve: &Resolve, key: &WorldKey) -> String {
    match key {
        WorldKey::Name(name) => name.clone(),
        WorldKey::Interface(id) => resolve.interfaces[*id]
            .name
            .clone()
            .unwrap_or_else(|| format!("interface-{id:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{ComponentCallableMultiplicityIr, ComponentParamIr};

    #[test]
    fn standard32_pointer_kinds_match_their_core_scalar_widths() {
        assert!(same_core_signature(
            &[WasmType::Pointer, WasmType::Length, WasmType::PointerOrI64],
            &[WasmType::I32, WasmType::I32, WasmType::I64],
        ));
        assert!(!same_core_signature(&[WasmType::Pointer], &[WasmType::I64]));
    }

    #[test]
    fn callable_adapter_owns_its_indirect_signature() {
        let callable = ComponentCallableIr {
            name: "transform".to_string(),
            params: vec![
                ComponentParamIr {
                    source_index: 0,
                    source_name: "value".to_string(),
                    name: "value".to_string(),
                    ty: ComponentValueType::S32,
                },
                ComponentParamIr {
                    source_index: 1,
                    source_name: "scale".to_string(),
                    name: "scale".to_string(),
                    ty: ComponentValueType::F64,
                },
                ComponentParamIr {
                    source_index: 2,
                    source_name: "marker".to_string(),
                    name: "marker".to_string(),
                    ty: ComponentValueType::Unit,
                },
            ],
            result: ComponentValueType::S64,
            multiplicity: ComponentCallableMultiplicityIr::Once,
        };
        let mut wat = String::new();
        emit_callable_closure_type(&mut wat, &callable, "owned-callable-type").unwrap();
        assert_eq!(
            wat,
            "  (type $owned-callable-type (func (param i32) (param f64) (param i32) (param i32) (result i64)))\n"
        );
    }

    #[test]
    fn persistent_resource_runtime_resets_only_after_the_last_release() {
        let mut wat = String::new();
        emit_resource_runtime(&mut wat, 0x60_000);
        assert!(wat.contains("global $__restrict_component_resource_live_count"));
        assert!(wat.contains("global.set $__restrict_component_resource_live_count"));
        assert!(wat.contains("i32.const 393216\n      call $arena_reset"));
        assert!(wat.contains("global.get $__restrict_component_resource_live_count\n    i32.eqz"));
    }
}
