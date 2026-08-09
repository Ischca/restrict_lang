#![cfg(not(target_arch = "wasm32"))]

use restrict_lang::component_external::{
    compile_external_wit_component, ExternalWitAdapterMetadata, ExternalWitArrayLengthMetadata,
    ExternalWitCompileRequest, ExternalWitContract, ExternalWitExportImplementation,
};
use restrict_lang::WasmOptimizationLevel;
use sha2::{Digest, Sha256};
use std::path::Path;
use wasm_encoder::{ComponentBuilder, ComponentExportKind};
use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store, Trap};
use wit_component::ComponentEncoder;

const FIXTURE_ROOT: &str = "tests/fixtures/external_wit_rust_producer";
const RUST_PRODUCER_CORE: &[u8] =
    include_bytes!("fixtures/external_wit_rust_producer/rust-producer.core.wasm");
const RESTRICT_CONSUMER_SOURCE: &str =
    include_str!("fixtures/external_wit_rust_producer/consumer.rl");

#[test]
fn rust_produced_component_calls_through_restrict_consumer_via_wit_only() {
    let wit_path = Path::new(FIXTURE_ROOT).join("wit");

    // This Core module is produced by the pinned wit-bindgen Rust fixture in
    // `rust-producer/`. ComponentEncoder consumes its Rust-generated
    // component-type metadata; no Restrict metadata or layout participates.
    assert!(!wasmparser::Parser::is_component(RUST_PRODUCER_CORE));
    assert_eq!(
        hex::encode(Sha256::digest(RUST_PRODUCER_CORE)),
        "c0e3595df95a996169c4914c0a14242cd0bc5480fa2552fde5caa97f9b74ebb1"
    );
    let has_rust_producer_metadata = wasmparser::Parser::new(0)
        .parse_all(RUST_PRODUCER_CORE)
        .filter_map(Result::ok)
        .any(|payload| match payload {
            wasmparser::Payload::CustomSection(section) if section.name() == "producers" => {
                section.data().windows(4).any(|bytes| bytes == b"Rust")
            }
            _ => false,
        });
    assert!(
        has_rust_producer_metadata,
        "checked-in guest must retain rustc producer provenance"
    );
    wasmparser::Validator::new()
        .validate_all(RUST_PRODUCER_CORE)
        .expect("checked-in Rust guest Core Wasm should validate");
    let rust_producer = ComponentEncoder::default()
        .module(RUST_PRODUCER_CORE)
        .expect("Rust guest metadata should be accepted")
        .validate(true)
        .encode()
        .expect("Rust guest should componentize independently");
    let producer_contract = ExternalWitContract::from_path(&wit_path, "producer")
        .expect("external producer world should resolve");
    producer_contract
        .validate_component(&rust_producer)
        .expect("Rust-produced Component should target the checked-in producer world");

    let unrefined_consumer = ExternalWitContract::from_path(&wit_path, "restrict-consumer")
        .expect("external consumer world should resolve before Array refinement");
    let fixed_array_site = unrefined_consumer
        .binding_ir()
        .list_sites
        .iter()
        .find(|site| site.canonical_path.ends_with("/type/fixed-values"))
        .expect("fixture should expose one stable fixed-values list site")
        .canonical_path
        .clone();
    let consumer_contract = ExternalWitContract::from_path_with_metadata(
        &wit_path,
        "restrict-consumer",
        ExternalWitAdapterMetadata {
            array_lengths: vec![ExternalWitArrayLengthMetadata::new(fixed_array_site, 3)],
        },
    )
    .expect("external consumer world should resolve with exact Array metadata");
    let plan = consumer_contract.restrict_binding_plan();
    assert_eq!(plan.imports.len(), 23);
    let increment_import = plan
        .imports
        .iter()
        .find(|binding| binding.name == "increment")
        .expect("fixture should import increment");
    assert_eq!(
        increment_import.local_function,
        "wit_import_increment_36267bdf9047e8973de0cb9e"
    );
    let interface_name = increment_import
        .interface
        .as_deref()
        .expect("fixture import should be an interface")
        .to_string();
    assert!(plan
        .imports
        .iter()
        .all(|binding| { binding.interface.as_deref() == Some(interface_name.as_str()) }));
    let implementations = plan
        .exports
        .iter()
        .map(|export| {
            let source_function = match export.name.as_str() {
                "run-bool" => "run_bool",
                "run-consume" => "run_consume",
                "run-long" => "run_long",
                "run-marker" => "run_marker",
                "run-ratio" => "run_ratio",
                "run-s32" => "run_s32",
                "run-string" => "run_string",
                "run-list" => "run_list",
                "run-record" => "run_record",
                "run-option" => "run_option",
                "run-result" => "run_result",
                "run-enum" => "run_enum",
                "run-variant" => "run_variant",
                "run-array" => "run_array",
                "run-array-mismatch" => "run_array_mismatch",
                unexpected => panic!("unexpected fixture export `{unexpected}`"),
            };
            ExternalWitExportImplementation::new(export.canonical_identity.clone(), source_function)
        })
        .collect::<Vec<_>>();
    let local_import = |name: &str| {
        plan.imports
            .iter()
            .find(|binding| binding.name == name)
            .unwrap_or_else(|| panic!("fixture should import {name}"))
            .local_function
            .as_str()
    };
    let restrict_consumer_source = format!(
        "{RESTRICT_CONSUMER_SOURCE}\n\
         pub fun run_string: () -> Int32 = {{ (() {}) |> {} }}\n\
         pub fun run_list: () -> Int32 = {{ (() {}) |> {} }}\n\
         pub fun run_record: () -> Int32 = {{ (() {}) |> {} }}\n\
         pub fun run_option: () -> Int32 = {{ (() {}) |> {} }}\n\
         pub fun run_result: () -> Int32 = {{ (() {}) |> {} }}\n\
         pub fun run_enum: () -> Int32 = {{ (() {}) |> {} }}\n\
         pub fun run_variant: () -> Int32 = {{ (() {}) |> {} }}\n\
         pub fun run_array: () -> Int32 = {{ (() {}) |> {} }}\n\
         pub fun run_array_mismatch: () -> Int32 = {{ (() {}) |> {} }}\n",
        local_import("make-string"),
        local_import("check-string"),
        local_import("make-list"),
        local_import("check-list"),
        local_import("make-packet"),
        local_import("check-packet"),
        local_import("make-option"),
        local_import("check-option"),
        local_import("make-result"),
        local_import("check-result"),
        local_import("make-state"),
        local_import("check-state"),
        local_import("make-event"),
        local_import("check-event"),
        local_import("make-array"),
        local_import("check-array"),
        local_import("make-bad-array"),
        local_import("check-array"),
    );
    let restrict_consumer = compile_external_wit_component(ExternalWitCompileRequest {
        source: &restrict_consumer_source,
        entry_path: Path::new(FIXTURE_ROOT).join("consumer.rl").as_path(),
        package_roots: &[],
        contract: &consumer_contract,
        export_implementations: &implementations,
        optimization_level: WasmOptimizationLevel::Release,
        arena_size_bytes: 0x1000,
    })
    .expect("Restrict consumer should compile against the external WIT world");

    // Static Component Model composition aliases the producer's typed WIT
    // instance directly into the consumer. The two nested Components retain
    // independent Core memories and allocators.
    let composed = compose_components(
        &rust_producer,
        &restrict_consumer.component_wasm,
        &interface_name,
        &[
            "run-bool",
            "run-consume",
            "run-long",
            "run-marker",
            "run-ratio",
            "run-s32",
            "run-string",
            "run-list",
            "run-record",
            "run-option",
            "run-result",
            "run-enum",
            "run-variant",
            "run-array",
            "run-array-mismatch",
        ],
    );
    wasmparser::Validator::new_with_features(
        wasmparser::WasmFeatures::default() | wasmparser::WasmFeatures::COMPONENT_MODEL,
    )
    .validate_all(&composed)
    .expect("statically composed Rust-to-Restrict Component should validate");

    let engine = Engine::default();
    let component =
        Component::new(&engine, &composed).expect("Wasmtime should compile the composed Component");
    let linker = Linker::new(&engine);
    let mut store = Store::new(&engine, ());
    let instance = linker
        .instantiate(&mut store, &component)
        .expect("the composed Component should have no unresolved imports");
    let run_consume = instance
        .get_typed_func::<(i32,), ()>(&mut store, "run-consume")
        .expect("composed Component should export run-consume");
    run_consume
        .call(&mut store, (7,))
        .expect("Rust producer should consume through Restrict");

    let run_long = instance
        .get_typed_func::<(i64,), (i64,)>(&mut store, "run-long")
        .expect("composed Component should export run-long");
    let long = i64::MIN + 17;
    assert_eq!(
        run_long
            .call(&mut store, (long,))
            .expect("Rust producer should return s64 through Restrict"),
        (long,)
    );

    let run_marker = instance
        .get_typed_func::<(char,), (char,)>(&mut store, "run-marker")
        .expect("composed Component should export run-marker");
    assert_eq!(
        run_marker
            .call(&mut store, ('界',))
            .expect("Rust producer should return char through Restrict"),
        ('界',)
    );

    let run_ratio = instance
        .get_typed_func::<(f64,), (f64,)>(&mut store, "run-ratio")
        .expect("composed Component should export run-ratio");
    assert_eq!(
        run_ratio
            .call(&mut store, (-12.5,))
            .expect("Rust producer should return f64 through Restrict"),
        (-12.5,)
    );

    let run_s32 = instance
        .get_typed_func::<(i32,), (i32,)>(&mut store, "run-s32")
        .expect("composed Component should export run-s32");
    assert_eq!(
        run_s32
            .call(&mut store, (1,))
            .expect("Rust producer should execute through Restrict"),
        (42,)
    );

    let run_bool = instance
        .get_typed_func::<(bool,), (bool,)>(&mut store, "run-bool")
        .expect("composed Component should export run-bool");
    assert_eq!(
        run_bool
            .call(&mut store, (true,))
            .expect("Rust producer should return bool through Restrict"),
        (false,)
    );
    assert_eq!(
        run_bool
            .call(&mut store, (false,))
            .expect("Rust producer should return bool through Restrict"),
        (true,)
    );

    for (name, expected) in [
        ("run-string", 101),
        ("run-list", 102),
        ("run-record", 103),
        ("run-option", 104),
        ("run-result", 105),
        ("run-enum", 106),
        ("run-variant", 107),
        ("run-array", 108),
    ] {
        let function = instance
            .get_typed_func::<(), (i32,)>(&mut store, name)
            .unwrap_or_else(|error| panic!("composed Component should export {name}: {error:#}"));
        assert_eq!(
            function.call(&mut store, ()).unwrap_or_else(|error| panic!(
                "{name} should cross the Rust/Restrict WIT boundary: {error:#}"
            )),
            (expected,),
            "{name} should preserve its rich value"
        );
    }

    let mismatch = instance
        .get_typed_func::<(), (i32,)>(&mut store, "run-array-mismatch")
        .expect("composed Component should export run-array-mismatch")
        .call(&mut store, ())
        .expect_err("Rust producer returning two items must violate Array<Int32, 3>");
    assert!(
        mismatch.downcast_ref::<Trap>().is_some(),
        "Array length mismatch should surface as a Wasm trap: {mismatch:#}"
    );
}

fn compose_components(
    producer: &[u8],
    consumer: &[u8],
    interface_name: &str,
    consumer_exports: &[&str],
) -> Vec<u8> {
    let mut builder = ComponentBuilder::default();
    let producer_component = builder.component_raw(Some("rust-producer"), producer);
    let producer_instance = builder.instantiate(
        Some("rust-producer-instance"),
        producer_component,
        std::iter::empty::<(&str, ComponentExportKind, u32)>(),
    );
    let producer_interface = builder.alias_export(
        producer_instance,
        interface_name,
        ComponentExportKind::Instance,
    );

    let consumer_component = builder.component_raw(Some("restrict-consumer"), consumer);
    let consumer_instance = builder.instantiate(
        Some("restrict-consumer-instance"),
        consumer_component,
        [(
            interface_name,
            ComponentExportKind::Instance,
            producer_interface,
        )],
    );
    for export_name in consumer_exports {
        let function =
            builder.alias_export(consumer_instance, export_name, ComponentExportKind::Func);
        builder.export(*export_name, ComponentExportKind::Func, function, None);
    }
    builder.finish()
}
