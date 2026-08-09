#![cfg(not(target_arch = "wasm32"))]

use restrict_lang::component::{
    compile_component, ComponentCompileRequest, ComponentExportSelection, ComponentSelection,
    COMPONENT_ABI_PROFILE,
};
use restrict_lang::parser::parse_program;
use restrict_lang::type_checker::TypeChecker;
use restrict_lang::WasmOptimizationLevel;
use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store, Trap};

wasmtime::component::bindgen!({
    path: "tests/fixtures/component_rich_consumer/wit",
    world: "component-rich-consumer",
});

use exports::test::component_rich_consumer::api::{Event, Packet, Status};

const SOURCE: &str = include_str!("fixtures/component_rich_consumer/source/main.rl");

fn selection() -> ComponentSelection {
    let exports = [
        ("echo_array", "echo-array"),
        ("echo_event", "echo-event"),
        ("echo_list", "echo-list"),
        ("echo_option", "echo-option"),
        ("echo_packet", "echo-packet"),
        ("echo_result", "echo-result"),
        ("echo_status", "echo-status"),
        ("echo_string", "echo-string"),
    ]
    .into_iter()
    .map(|(source, external)| ComponentExportSelection::new("api", source, external))
    .collect();
    ComponentSelection::v1(
        "test:component-rich-consumer@0.1.0",
        "component-rich-consumer",
        COMPONENT_ABI_PROFILE,
        exports,
    )
}

#[test]
fn rich_consumer_fixture_source_parses_and_typechecks() {
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
fn independently_generated_rust_bindings_call_restrict_rich_component() {
    let snapshot = tempfile::tempdir().expect("source snapshot should be creatable");
    let entry_path = snapshot.path().join("main.rl");
    let selection = selection();
    let artifact = compile_component(ComponentCompileRequest {
        source: SOURCE,
        entry_path: &entry_path,
        package_roots: &[],
        selection: &selection,
        optimization_level: WasmOptimizationLevel::Release,
        arena_size_bytes: 0x20_000,
    })
    .expect("production Component compiler pipeline should support rich values");
    assert_eq!(
        artifact.normalized_wit,
        include_str!("fixtures/component_rich_consumer/wit/component-rich-consumer.wit"),
        "the independent Rust consumer must bind to the exact generated WIT contract"
    );

    let engine = Engine::default();
    let component = Component::new(&engine, &artifact.component_wasm)
        .expect("Wasmtime should compile the independently produced Component");
    let linker = Linker::new(&engine);
    let mut store = Store::new(&engine, ());
    let bindings = ComponentRichConsumer::instantiate(&mut store, &component, &linker)
        .expect("Wasmtime should instantiate the import-free Component");
    let api = bindings.test_component_rich_consumer_api();

    assert_eq!(api.call_echo_string(&mut store, "").unwrap(), "");
    assert_eq!(
        api.call_echo_string(&mut store, "Restrict 🦀界").unwrap(),
        "Restrict 🦀界"
    );
    assert_eq!(
        api.call_echo_list(&mut store, &[]).unwrap(),
        Vec::<i32>::new()
    );
    assert_eq!(
        api.call_echo_list(&mut store, &[i32::MIN, 0, i32::MAX])
            .unwrap(),
        [i32::MIN, 0, i32::MAX]
    );
    assert_eq!(
        api.call_echo_array(&mut store, &[7, -8, 9]).unwrap(),
        [7, -8, 9]
    );

    assert_eq!(api.call_echo_option(&mut store, None).unwrap(), None);
    assert_eq!(
        api.call_echo_option(&mut store, Some("選択肢"))
            .unwrap()
            .as_deref(),
        Some("選択肢")
    );
    assert_eq!(
        api.call_echo_result(&mut store, Ok(&[])).unwrap(),
        Ok(Vec::<i32>::new())
    );
    assert_eq!(
        api.call_echo_result(&mut store, Ok(&[-2, 5, 8])).unwrap(),
        Ok(vec![-2, 5, 8])
    );
    let failure = "失敗".to_string();
    assert_eq!(
        api.call_echo_result(&mut store, Err(&failure))
            .unwrap()
            .as_deref()
            .map_err(String::as_str),
        Err("失敗")
    );

    assert!(matches!(
        api.call_echo_status(&mut store, Status::Ready).unwrap(),
        Status::Ready
    ));
    assert!(matches!(
        api.call_echo_status(&mut store, Status::Paused).unwrap(),
        Status::Paused
    ));
    assert!(matches!(
        api.call_echo_event(&mut store, &Event::Idle).unwrap(),
        Event::Idle
    ));
    match api
        .call_echo_event(&mut store, &Event::Message(String::new()))
        .unwrap()
    {
        Event::Message(message) => assert!(message.is_empty()),
        other => panic!("expected message event, got {other:?}"),
    }
    match api
        .call_echo_event(&mut store, &Event::Values(Vec::new()))
        .unwrap()
    {
        Event::Values(values) => assert!(values.is_empty()),
        other => panic!("expected values event, got {other:?}"),
    }

    let packet = Packet {
        label: "nested 📦".to_string(),
        values: vec![10, 20, 30],
        fixed: vec![-1, 0, 1],
        maybe: Some(String::new()),
        outcome: Ok(vec![]),
        status: Status::Paused,
        event: Event::Values(vec![3, 1, 4]),
    };
    let echoed = api.call_echo_packet(&mut store, &packet).unwrap();
    assert_eq!(echoed.label, "nested 📦");
    assert_eq!(echoed.values, [10, 20, 30]);
    assert_eq!(echoed.fixed, [-1, 0, 1]);
    assert_eq!(echoed.maybe.as_deref(), Some(""));
    assert_eq!(echoed.outcome, Ok(vec![]));
    assert!(matches!(echoed.status, Status::Paused));
    match echoed.event {
        Event::Values(values) => assert_eq!(values, [3, 1, 4]),
        other => panic!("expected nested values event, got {other:?}"),
    }

    let empty_packet = Packet {
        label: String::new(),
        values: vec![],
        fixed: vec![0, 0, 0],
        maybe: None,
        outcome: Err(String::new()),
        status: Status::Ready,
        event: Event::Idle,
    };
    for iteration in 0..32 {
        assert_eq!(
            api.call_echo_string(&mut store, "repeat").unwrap(),
            "repeat",
            "string repeat call {iteration}"
        );
        let echoed = api.call_echo_packet(&mut store, &empty_packet).unwrap();
        assert!(echoed.label.is_empty(), "packet repeat call {iteration}");
        assert!(echoed.values.is_empty(), "packet repeat call {iteration}");
        assert_eq!(echoed.fixed, [0, 0, 0], "packet repeat call {iteration}");
        assert_eq!(echoed.maybe, None, "packet repeat call {iteration}");
        assert_eq!(
            echoed.outcome,
            Err(String::new()),
            "packet repeat call {iteration}"
        );
        assert!(matches!(echoed.status, Status::Ready));
        assert!(matches!(echoed.event, Event::Idle));
    }

    let mut mismatch_store = Store::new(&engine, ());
    let mismatch_bindings =
        ComponentRichConsumer::instantiate(&mut mismatch_store, &component, &linker)
            .expect("a fresh instance should be available for the trapping call");
    let mismatch_api = mismatch_bindings.test_component_rich_consumer_api();
    let error = mismatch_api
        .call_echo_array(&mut mismatch_store, &[1, 2])
        .expect_err("Array<Int32, 3> must trap when the canonical list length is not three");
    assert!(
        error.downcast_ref::<Trap>().is_some(),
        "array length mismatch should surface as a Wasm trap: {error:#}"
    );
}
