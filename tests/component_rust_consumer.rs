#![cfg(not(target_arch = "wasm32"))]

use restrict_lang::component::{
    compile_component, ComponentCompileRequest, ComponentExportSelection, ComponentSelection,
    COMPONENT_ABI_PROFILE,
};
use restrict_lang::WasmOptimizationLevel;
use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store};

wasmtime::component::bindgen!({
    path: "tests/fixtures/component_scalar_consumer/wit",
    world: "component-consumer",
});

const SOURCE: &str = r#"
pub fun answer: () -> Int32 = {
    42
}

pub fun consume_value: (value: Int32) -> () = {
    ()
}

pub fun consume_unit: (value: ()) -> Int32 = {
    42
}

pub fun echo_long: (value: Int64) -> Int64 = {
    value
}

pub fun echo_marker: (value: Char) -> Char = {
    value
}

pub fun echo_ratio: (value: Float64) -> Float64 = {
    value
}

pub fun increment: (value: Int32) -> Int32 = {
    value + 1
}

pub fun keep_active: (
    left: Int32,
    total: Int64,
    ratio: Float64,
    active: Boolean,
    marker: Char
) -> Boolean = {
    active
}
"#;

#[test]
fn independently_generated_rust_bindings_call_restrict_scalar_component() {
    let exports = [
        ("answer", "answer"),
        ("consume_unit", "consume-unit"),
        ("consume_value", "consume-value"),
        ("echo_long", "echo-long"),
        ("echo_marker", "echo-marker"),
        ("echo_ratio", "echo-ratio"),
        ("increment", "increment"),
        ("keep_active", "keep-active"),
    ]
    .into_iter()
    .map(|(source, external)| ComponentExportSelection::new("api", source, external))
    .collect();
    let selection = ComponentSelection::v1(
        "test:component-consumer@0.1.0",
        "component-consumer",
        COMPONENT_ABI_PROFILE,
        exports,
    );
    let snapshot = tempfile::tempdir().expect("source snapshot should be creatable");
    let entry_path = snapshot.path().join("main.rl");
    let artifact = compile_component(ComponentCompileRequest {
        source: SOURCE,
        entry_path: &entry_path,
        package_roots: &[],
        selection: &selection,
        optimization_level: WasmOptimizationLevel::Release,
        arena_size_bytes: 0x1000,
    })
    .expect("production Component compiler pipeline should succeed");
    assert_eq!(
        artifact.normalized_wit,
        include_str!("fixtures/component_scalar_consumer/wit/component-consumer.wit"),
        "the independent Rust consumer must bind to the exact generated WIT contract"
    );

    let engine = Engine::default();
    let component = Component::new(&engine, &artifact.component_wasm)
        .expect("Wasmtime should compile the independently produced Component");
    let linker = Linker::new(&engine);
    let mut store = Store::new(&engine, ());
    let bindings = ComponentConsumer::instantiate(&mut store, &component, &linker)
        .expect("Wasmtime should instantiate the import-free Component");
    let api = bindings.test_component_consumer_api();

    assert_eq!(api.call_answer(&mut store).unwrap(), 42);
    assert_eq!(api.call_consume_unit(&mut store).unwrap(), 42);
    api.call_consume_value(&mut store, 7).unwrap();
    assert_eq!(
        api.call_echo_long(&mut store, i64::MIN + 17).unwrap(),
        i64::MIN + 17
    );
    assert_eq!(api.call_echo_marker(&mut store, '🦀').unwrap(), '🦀');
    assert_eq!(api.call_echo_ratio(&mut store, -12.5).unwrap(), -12.5);
    assert_eq!(api.call_increment(&mut store, 41).unwrap(), 42);
    assert!(api
        .call_keep_active(&mut store, -3, 9_000_000_000, 0.25, true, '界')
        .unwrap());
    assert!(!api
        .call_keep_active(&mut store, 8, -5, -0.0, false, '\0')
        .unwrap());
}
