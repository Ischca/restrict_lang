#![cfg(not(target_arch = "wasm32"))]

use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store};
use wit_component::{ComponentEncoder, StringEncoding};
use wit_parser::{
    ManglingAndAbi, ResourceIntrinsic, WasmExport, WasmExportKind, WasmImport, WorldItem,
};

wasmtime::component::bindgen!({
    path: "tests/fixtures/resource_component_probe/wit",
    world: "resource-probe",
});

const WIT: &str = include_str!("fixtures/resource_component_probe/wit/resource-probe.wit");

/// Minimal Standard32 guest-export resource module.
///
/// The resource representation is the initial counter value itself. This keeps
/// the probe focused on Component Model ownership rather than a guest heap.
/// Production Restrict resources still need persistent guest storage for rich
/// representations and captured environments.
fn core_wat(names: &Standard32ResourceNames) -> String {
    format!(
        r#"
(module
  (import "{}" "{}"
    (func $resource_new_counter (param i32) (result i32)))
  (import "{}" "{}"
    (func $resource_rep_counter (param i32) (result i32)))
  (import "{}" "{}"
    (func $resource_drop_counter (param i32)))

  (global $drop_count (mut i32) (i32.const 0))

  (func (export "{}")
    (param $init i32) (result i32)
    local.get $init
    call $resource_new_counter)

  ;; A WIT resource method receives its implicit borrow receiver as the guest
  ;; representation, not as an own handle.
  (func (export "{}")
    (param $rep i32) (result i32)
    local.get $rep)

  ;; A static function with an explicit own<counter> receives an own handle.
  ;; The generated guest binding obtains its representation and drops the
  ;; consumed handle before returning.
  (func (export "{}")
    (param $handle i32) (result i32)
    (local $rep i32)
    local.get $handle
    call $resource_rep_counter
    local.set $rep
    local.get $handle
    call $resource_drop_counter
    local.get $rep)

  (func (export "{}")
    (param $rep i32)
    global.get $drop_count
    i32.const 1
    i32.add
    global.set $drop_count)

  (func (export "{}") (result i32)
    global.get $drop_count)
)
"#,
        names.new_module,
        names.new_name,
        names.rep_module,
        names.rep_name,
        names.drop_module,
        names.drop_name,
        names.constructor_export,
        names.get_export,
        names.consume_export,
        names.destructor_export,
        names.drop_count_export,
    )
}

#[derive(Debug)]
struct Standard32ResourceNames {
    new_module: String,
    new_name: String,
    rep_module: String,
    rep_name: String,
    drop_module: String,
    drop_name: String,
    constructor_export: String,
    get_export: String,
    consume_export: String,
    destructor_export: String,
    drop_count_export: String,
}

fn standard32_resource_names(
    resolve: &wit_parser::Resolve,
    world: wit_parser::WorldId,
) -> Standard32ResourceNames {
    let (interface_key, interface_id) = resolve.worlds[world]
        .exports
        .iter()
        .find_map(|(key, item)| match item {
            WorldItem::Interface { id, .. } => Some((key, *id)),
            _ => None,
        })
        .expect("resource probe world should export its interface");
    let interface = &resolve.interfaces[interface_id];
    let resource = interface.types["counter"];
    let function = |name: &str| {
        interface
            .functions
            .get(name)
            .unwrap_or_else(|| panic!("resource probe should contain WIT function '{name}'"))
    };
    let import = |intrinsic| {
        resolve.wasm_import_name(
            ManglingAndAbi::Standard32,
            WasmImport::ResourceIntrinsic {
                interface: Some(interface_key),
                resource,
                intrinsic,
            },
        )
    };
    let export = |function| {
        resolve.wasm_export_name(
            ManglingAndAbi::Standard32,
            WasmExport::Func {
                interface: Some(interface_key),
                func: function,
                kind: WasmExportKind::Normal,
            },
        )
    };
    let (new_module, new_name) = import(ResourceIntrinsic::ExportedNew);
    let (rep_module, rep_name) = import(ResourceIntrinsic::ExportedRep);
    let (drop_module, drop_name) = import(ResourceIntrinsic::ExportedDrop);
    Standard32ResourceNames {
        new_module,
        new_name,
        rep_module,
        rep_name,
        drop_module,
        drop_name,
        constructor_export: export(function("[constructor]counter")),
        get_export: export(function("[method]counter.get")),
        consume_export: export(function("[static]counter.consume")),
        destructor_export: resolve.wasm_export_name(
            ManglingAndAbi::Standard32,
            WasmExport::ResourceDtor {
                interface: interface_key,
                resource,
            },
        ),
        drop_count_export: export(function("drop-count")),
    }
}

fn build_probe_component() -> Vec<u8> {
    let mut resolve = wit_parser::Resolve::default();
    let package = resolve
        .push_str("resource-probe.wit", WIT)
        .expect("resource probe WIT should resolve");
    let world = resolve
        .select_world(&[package], Some("resource-probe"))
        .expect("resource probe world should resolve");
    let names = standard32_resource_names(&resolve, world);
    assert_eq!(names.new_module, "cm32p2|_ex_test:resource-probe/api");
    assert_eq!(names.new_name, "counter_new");
    assert_eq!(names.rep_module, names.new_module);
    assert_eq!(names.rep_name, "counter_rep");
    assert_eq!(names.drop_module, names.new_module);
    assert_eq!(names.drop_name, "counter_drop");
    assert_eq!(
        names.constructor_export,
        "cm32p2|test:resource-probe/api|[constructor]counter"
    );
    assert_eq!(
        names.get_export,
        "cm32p2|test:resource-probe/api|[method]counter.get"
    );
    assert_eq!(
        names.consume_export,
        "cm32p2|test:resource-probe/api|[static]counter.consume"
    );
    assert_eq!(
        names.destructor_export,
        "cm32p2|test:resource-probe/api|counter_dtor"
    );
    assert_eq!(
        names.drop_count_export,
        "cm32p2|test:resource-probe/api|drop-count"
    );

    let mut core_wasm =
        wat::parse_str(core_wat(&names)).expect("resource probe Core WAT should parse");
    wasmparser::Validator::new()
        .validate_all(&core_wasm)
        .expect("resource probe Core Wasm should validate");
    wit_component::embed_component_metadata(&mut core_wasm, &resolve, world, StringEncoding::UTF8)
        .expect("resource probe metadata should embed");

    let component = ComponentEncoder::default()
        .module(&core_wasm)
        .expect("resource probe module should match its WIT metadata")
        .reject_legacy_names(true)
        .validate(true)
        .encode()
        .expect("resource probe should componentize");
    wit_component::targets(&resolve, world, &component)
        .expect("resource probe Component should target the selected world");
    component
}

#[test]
fn rust_bindgen_calls_borrows_consumes_and_drops_guest_resource() {
    let engine = Engine::default();
    let component_wasm = build_probe_component();
    let component = Component::new(&engine, &component_wasm)
        .expect("Wasmtime should compile the resource probe Component");
    let linker = Linker::new(&engine);
    let mut store = Store::new(&engine, ());
    let bindings = ResourceProbe::instantiate(&mut store, &component, &linker)
        .expect("Wasmtime should instantiate the import-free resource probe");
    let api = bindings.test_resource_probe_api();
    let counter_api = api.counter();

    let counter = counter_api
        .call_constructor(&mut store, 41)
        .expect("constructor should return own<counter>");
    assert!(counter.owned());
    assert_eq!(api.call_drop_count(&mut store).unwrap(), 0);

    // The implicit borrow does not consume the own handle, so it is reusable.
    assert_eq!(counter_api.call_get(&mut store, counter).unwrap(), 41);
    assert_eq!(counter_api.call_get(&mut store, counter).unwrap(), 41);

    // ResourceAny is Copy, but Wasmtime's generation-tracked table makes a
    // stale copy unusable after ownership is transferred.
    let stale_after_consume = counter;
    assert_eq!(counter_api.call_consume(&mut store, counter).unwrap(), 41);
    assert_eq!(api.call_drop_count(&mut store).unwrap(), 1);
    let reuse_error = counter_api
        .call_get(&mut store, stale_after_consume)
        .expect_err("a consumed ResourceAny copy must not be reusable");
    assert!(
        reuse_error.to_string().contains("unknown handle index"),
        "unexpected stale-handle error: {reuse_error:#}"
    );
    stale_after_consume
        .resource_drop(&mut store)
        .expect_err("a consumed ResourceAny copy must not be dropped twice");
    assert_eq!(api.call_drop_count(&mut store).unwrap(), 1);

    let explicitly_dropped = counter_api
        .call_constructor(&mut store, -7)
        .expect("second constructor should return own<counter>");
    let stale_after_drop = explicitly_dropped;
    explicitly_dropped
        .resource_drop(&mut store)
        .expect("an unconsumed own resource must support explicit drop");
    assert_eq!(api.call_drop_count(&mut store).unwrap(), 2);
    stale_after_drop
        .resource_drop(&mut store)
        .expect_err("an explicitly dropped ResourceAny copy must stay stale");
    assert_eq!(api.call_drop_count(&mut store).unwrap(), 2);
}
