#![cfg(not(target_arch = "wasm32"))]

use restrict_lang::component::{
    compile_component_contract, ComponentCompileContractRequest, COMPONENT_ABI_PROFILE,
};
use restrict_lang::component_selection_v2::{
    ClosedComponentType, ComponentCallableContract, ComponentCallableMultiplicity,
    ComponentCallableParamContract, ComponentDestructorContract, ComponentFunctionExportContract,
    ComponentResourceContract, ComponentResourceMethodContract, ComponentResourceReceiver,
    ComponentSelectionContract, ComponentSourceVisibility, COMPONENT_SELECTION_SCHEMA_V2,
};
use restrict_lang::parser::parse_program;
use restrict_lang::type_checker::TypeChecker;
use restrict_lang::WasmOptimizationLevel;
use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store};

wasmtime::component::bindgen!({
    path: "tests/fixtures/component_resource_consumer/wit",
    world: "component-resource-consumer",
});

const SOURCE: &str = include_str!("fixtures/component_resource_consumer/source/main.rl");

fn selection() -> ComponentSelectionContract {
    ComponentSelectionContract::try_new(
        COMPONENT_SELECTION_SCHEMA_V2,
        "test:component-resource-consumer@0.1.0",
        "component-resource-consumer",
        COMPONENT_ABI_PROFILE,
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
    .expect("schema-v2 resource selection should normalize")
}

#[test]
fn resource_fixture_source_parses_and_typechecks() {
    let (remaining, program) = parse_program(SOURCE).expect("fixture source should parse");
    assert!(
        remaining.trim().is_empty(),
        "fixture source has trailing input"
    );
    TypeChecker::new()
        .check_program(&program)
        .expect("fixture source should typecheck");
}

#[test]
fn independent_rust_bindings_enforce_resource_and_callable_ownership() {
    let snapshot = tempfile::tempdir().expect("source snapshot should be creatable");
    let entry_path = snapshot.path().join("main.rl");
    let selection = selection();
    let artifact = compile_component_contract(ComponentCompileContractRequest {
        source: SOURCE,
        entry_path: &entry_path,
        package_roots: &[],
        selection: &selection,
        optimization_level: WasmOptimizationLevel::Release,
        arena_size_bytes: 0x20_000,
    })
    .expect("production Component compiler pipeline should support resources and callables");
    assert_eq!(
        artifact.normalized_wit,
        include_str!("fixtures/component_resource_consumer/wit/component-resource-consumer.wit"),
        "Rust bindings and Restrict must use the exact normalized WIT contract"
    );

    let engine = Engine::default();
    let component = Component::new(&engine, &artifact.component_wasm)
        .expect("Wasmtime should compile the Restrict-produced Component");
    let linker = Linker::new(&engine);
    let mut store = Store::new(&engine, ());
    let bindings = ComponentResourceConsumer::instantiate(&mut store, &component, &linker)
        .expect("Wasmtime should instantiate the import-free Component");
    let api = bindings.test_component_resource_consumer_api();
    let counter_api = api.counter();
    let transform_api = api.transform();

    let counter = api.call_make_counter(&mut store, 41).unwrap();
    assert!(counter.owned());
    assert_eq!(counter_api.call_read(&mut store, counter).unwrap(), 41);
    assert_eq!(counter_api.call_read(&mut store, counter).unwrap(), 41);

    let stale_counter = counter;
    assert_eq!(counter_api.call_finish(&mut store, counter).unwrap(), 41);
    counter_api
        .call_read(&mut store, stale_counter)
        .expect_err("terminal own method must invalidate every stale handle copy");
    stale_counter
        .resource_drop(&mut store)
        .expect_err("terminal own method must run the resource destructor exactly once");

    let explicitly_dropped = api.call_make_counter(&mut store, -7).unwrap();
    let stale_after_drop = explicitly_dropped;
    explicitly_dropped
        .resource_drop(&mut store)
        .expect("an unconsumed own resource must support explicit drop");
    stale_after_drop
        .resource_drop(&mut store)
        .expect_err("an explicitly dropped handle cannot be dropped twice");

    // Resource backing storage is shared but may only be reset after the last
    // live handle. Dropping one of two resources must not invalidate the
    // other's representation.
    let first_live = api.call_make_counter(&mut store, 10).unwrap();
    let second_live = api.call_make_counter(&mut store, 20).unwrap();
    first_live
        .resource_drop(&mut store)
        .expect("dropping one live resource should succeed");
    assert_eq!(counter_api.call_read(&mut store, second_live).unwrap(), 20);
    second_live
        .resource_drop(&mut store)
        .expect("the final live resource should release the persistent arena");

    let transform = api.call_make_transform(&mut store, 9).unwrap();
    let stale_transform = transform;
    assert_eq!(
        transform_api.call_call(&mut store, transform, 33).unwrap(),
        42
    );
    transform_api
        .call_call(&mut store, stale_transform, 1)
        .expect_err("once callable must invalidate stale copies after its first call");
    stale_transform
        .resource_drop(&mut store)
        .expect_err("called closure captures must be cleaned exactly once");

    // Dropping without calling exercises the other capture-cleanup path. A
    // stale copy then proves that the generated callable destructor completed
    // and the dynamic resource generation was invalidated.
    let uncalled = api.call_make_transform(&mut store, 100).unwrap();
    let stale_uncalled = uncalled;
    uncalled
        .resource_drop(&mut store)
        .expect("dropping an uncalled once closure must clean its captures");
    stale_uncalled
        .resource_drop(&mut store)
        .expect_err("uncalled closure captures cannot be cleaned twice");
}
