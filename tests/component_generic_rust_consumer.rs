#![cfg(not(target_arch = "wasm32"))]

use restrict_lang::component::{
    compile_component_contract, ComponentCompileContractRequest, COMPONENT_ABI_PROFILE,
};
use restrict_lang::component_selection_v2::{
    ClosedComponentType, ComponentFunctionExportContract, ComponentSelectionContract,
    ComponentTypeArgumentContract, COMPONENT_SELECTION_SCHEMA_V2,
};
use restrict_lang::WasmOptimizationLevel;
use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store};

wasmtime::component::bindgen!({
    path: "tests/fixtures/component_generic_consumer/wit",
    world: "generic-consumer",
});

const SOURCE: &str = r#"
pub fun identity: <T>(value: T) -> T = {
    value
}
"#;

fn generic_export(name: &str, ty: ClosedComponentType) -> ComponentFunctionExportContract {
    ComponentFunctionExportContract {
        interface: "api".to_string(),
        source_function: "identity".to_string(),
        external_name: name.to_string(),
        type_arguments: vec![ComponentTypeArgumentContract {
            parameter: "T".to_string(),
            ty,
        }],
    }
}

#[test]
fn explicit_generic_instances_are_real_component_exports() {
    let selection = ComponentSelectionContract::try_new(
        COMPONENT_SELECTION_SCHEMA_V2,
        "test:generic-consumer@0.1.0",
        "generic-consumer",
        COMPONENT_ABI_PROFILE,
        vec![
            generic_export("identity-string", ClosedComponentType::String),
            generic_export("identity-int", ClosedComponentType::Int32),
        ],
        vec![],
        vec![],
    )
    .expect("schema-v2 generic contract should normalize");
    let snapshot = tempfile::tempdir().expect("source snapshot should be creatable");
    let entry_path = snapshot.path().join("main.rl");
    let artifact = compile_component_contract(ComponentCompileContractRequest {
        source: SOURCE,
        entry_path: &entry_path,
        package_roots: &[],
        selection: &selection,
        optimization_level: WasmOptimizationLevel::Release,
        arena_size_bytes: 0x20_000,
    })
    .expect("explicit generic instances should compile through Component packaging");
    assert_eq!(
        artifact.normalized_wit,
        include_str!("fixtures/component_generic_consumer/wit/generic-consumer.wit")
    );

    let engine = Engine::default();
    let component = Component::new(&engine, &artifact.component_wasm)
        .expect("Wasmtime should compile the generic Restrict Component");
    let linker = Linker::new(&engine);
    let mut store = Store::new(&engine, ());
    let bindings = GenericConsumer::instantiate(&mut store, &component, &linker)
        .expect("generic Component should instantiate without private Core imports");
    let api = bindings.test_generic_consumer_api();
    assert_eq!(
        api.call_identity_int(&mut store, i32::MIN).unwrap(),
        i32::MIN
    );
    assert_eq!(
        api.call_identity_string(&mut store, "具体化 🦀").unwrap(),
        "具体化 🦀"
    );
}
