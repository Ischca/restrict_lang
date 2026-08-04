use restrict_lang::ir::builder::build_checked_ir;
use restrict_lang::module::resolve_program_imports_with_module_source_map;
use restrict_lang::{
    check_release_surface, parse_program, HostAbiProfile, TypeChecker, WasmCodeGen,
};
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use wasmi::{Caller, Engine, Instance, Linker, Module, Store};

fn compile_flat_record_v1(source: &str) -> Result<(String, Vec<u8>), String> {
    let (remaining, program) =
        parse_program(source).map_err(|error| format!("Parse error: {error:?}"))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {remaining:?}"));
    }

    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .map_err(|error| format!("Type error: {error}"))?;
    check_release_surface(&program, &checker, HostAbiProfile::FlatRecordV1)
        .map_err(|error| format!("Release surface error: {error}"))?;
    let checked_ir =
        build_checked_ir(&program, &checker).map_err(|error| format!("IR error: {error}"))?;
    let wat = WasmCodeGen::with_host_abi_profile(HostAbiProfile::FlatRecordV1)
        .generate_checked(&program, &checked_ir)
        .map_err(|error| format!("Codegen error: {error}"))?;
    let wasm = wat::parse_str(&wat).map_err(|error| format!("Invalid WAT: {error}\n\n{wat}"))?;
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .map_err(|error| format!("Invalid Wasm: {error}\n\n{wat}"))?;

    Ok((wat, wasm))
}

fn instantiate(wasm: &[u8]) -> Result<(Store<()>, Instance), Box<dyn std::error::Error>> {
    let engine = Engine::default();
    let module = Module::new(&engine, wasm)?;
    let mut store = Store::new(&engine, ());
    let mut linker = Linker::new(&engine);

    linker.func_wrap(
        "wasi_snapshot_preview1",
        "fd_write",
        |_caller: Caller<'_, ()>, _fd: i32, _iovs: i32, _iovs_len: i32, _nwritten: i32| -> i32 {
            0
        },
    )?;
    linker.func_wrap(
        "wasi_snapshot_preview1",
        "proc_exit",
        |_caller: Caller<'_, ()>, _code: i32| {},
    )?;

    let instance = linker.instantiate_and_start(&mut store, &module)?;
    Ok((store, instance))
}

#[test]
fn flat_record_v1_round_trips_mixed_scalar_fields() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
pub record Reading {
    count: Int32,
    total: Int64,
    ratio: Float64,
    active: Boolean,
    marker: Char
}

pub fun keep_reading: (reading: Reading) -> Reading = {
    reading
}
"#;

    let (wat, wasm) = compile_flat_record_v1(source)?;
    assert!(wat.contains("(func $__restrict_flat_record_v1_keep_reading"));
    assert!(
        wat.contains("(export \"keep_reading\" (func $__restrict_flat_record_v1_keep_reading))")
    );
    assert!(!wat.contains("(export \"keep_reading\" (func $keep_reading))"));

    let (mut store, instance) = instantiate(&wasm)?;
    let keep_reading = instance
        .get_typed_func::<(i32, i64, f64, i32, i32), (i32, i64, f64, i32, i32)>(
            &store,
            "keep_reading",
        )?;

    let first = (7, 9_000_000_001, 3.25, 1, ':' as i32);
    let second = (-2, -4_000_000_005, -1.5, 0, 'Z' as i32);
    assert_eq!(keep_reading.call(&mut store, first)?, first);
    assert_eq!(keep_reading.call(&mut store, second)?, second);
    Ok(())
}

#[test]
fn flat_record_v1_copies_record_results_before_resetting_its_arena() {
    let source = r#"
pub record Measurement {
    value: Float64,
    samples: Int64,
    valid: Boolean
}

pub fun measurement: (value: Float64, samples: Int64) -> Measurement = {
    Measurement {
        value: value,
        samples: samples,
        valid: true
    }
}
"#;

    let (wat, wasm) = compile_flat_record_v1(source).expect("preview codegen should succeed");
    let result_load = wat
        .find("local.set $host_result_2")
        .expect("adapter should copy all result fields into scalar locals");
    let arena_reset = wat[result_load..]
        .find("call $arena_reset")
        .map(|offset| result_load + offset)
        .expect("adapter should reset its arena");
    let returned_scalar = wat[arena_reset..]
        .find("local.get $host_result_0")
        .map(|offset| arena_reset + offset)
        .expect("adapter should return copied scalar values");
    assert!(result_load < arena_reset && arena_reset < returned_scalar);

    let (mut store, instance) = instantiate(&wasm).expect("preview Wasm should instantiate");
    let measurement = instance
        .get_typed_func::<(f64, i64), (f64, i64, i32)>(&store, "measurement")
        .expect("flattened multi-value export should have the documented type");
    assert_eq!(
        measurement
            .call(&mut store, (12.5, 42))
            .expect("flattened export should execute"),
        (12.5, 42, 1)
    );
}

#[test]
fn default_codegen_keeps_the_v001_scalar_only_contract() {
    let source = r#"
pub record Point {
    x: Int32,
    y: Int32
}

pub fun identity: (point: Point) -> Point = {
    point
}
"#;
    let (remaining, program) = parse_program(source).expect("source should parse");
    assert!(remaining.trim().is_empty());
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("source should type check");
    let checked_ir = build_checked_ir(&program, &checker).expect("Checked IR should build");

    let error = WasmCodeGen::new()
        .generate_checked(&program, &checked_ir)
        .expect_err("default ABI must continue rejecting record exports");
    assert!(error
        .to_string()
        .contains("v0.0.1 exports support only scalar"));
}

#[test]
fn cli_flat_record_v1_emits_the_host_adapter() {
    let unique = format!("{}_flat_record_codegen", std::process::id());
    let source_path = std::env::temp_dir().join(format!("restrict_lang_{unique}.rl"));
    let wat_path = std::env::temp_dir().join(format!("restrict_lang_{unique}.wat"));
    fs::write(
        &source_path,
        r#"
pub record Pair {
    left: Int32,
    right: Int64
}

pub fun keep_pair: (pair: Pair) -> Pair = {
    pair
}
"#,
    )
    .expect("temporary source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .args(["--host-abi", "flat-record-v1"])
        .arg(&source_path)
        .arg(&wat_path)
        .output()
        .expect("compiler should run");
    assert!(
        output.status.success(),
        "opt-in compilation should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let wat = fs::read_to_string(&wat_path).expect("compiler should write WAT");
    assert!(wat.contains("(export \"keep_pair\" (func $__restrict_flat_record_v1_keep_pair))"));
    let wasm = wat::parse_str(&wat).expect("CLI output should assemble");
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .expect("CLI output should validate");

    let _ = fs::remove_file(source_path);
    let _ = fs::remove_file(wat_path);
}

#[test]
fn flat_record_v1_sizes_memory_for_nine_adapter_arenas() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = String::from(
        r#"
pub record Pair {
    left: Int32,
    right: Int32
}
"#,
    );
    for index in 0..9 {
        source.push_str(&format!(
            r#"
pub fun keep_{index}: (pair: Pair) -> Pair = {{
    pair
}}
"#
        ));
    }

    let (wat, wasm) = compile_flat_record_v1(&source)?;
    assert!(
        wat.contains("  (memory 2)"),
        "nine 4-KiB adapter arenas require a second Wasm page:\n{wat}"
    );

    let (mut store, instance) = instantiate(&wasm)?;
    let keep_ninth = instance.get_typed_func::<(i32, i32), (i32, i32)>(&store, "keep_8")?;
    assert_eq!(keep_ninth.call(&mut store, (81, 82))?, (81, 82));
    Ok(())
}

#[test]
fn flat_record_v1_places_adapter_arenas_after_large_static_data(
) -> Result<(), Box<dyn std::error::Error>> {
    let literal = "a".repeat(40_000);
    let source = format!(
        r#"
fun large_literal: () -> String = {{
    "{literal}"
}}

pub record Pair {{
    left: Int32,
    right: Int32
}}

pub fun keep_pair: (pair: Pair) -> Pair = {{
    pair
}}
"#
    );

    let (wat, wasm) = compile_flat_record_v1(&source)?;
    assert!(
        wat.contains("i32.const 45056"),
        "the adapter arena should start at the next 4-KiB boundary after static data"
    );

    let (mut store, instance) = instantiate(&wasm)?;
    let memory = instance
        .get_export(&store, "memory")
        .and_then(|export| export.into_memory())
        .expect("generated module should export memory");
    let mut before = vec![0; literal.len() + 4];
    memory.read(&store, 1024, &mut before)?;
    assert_eq!(u32::from_le_bytes(before[..4].try_into()?), 40_000);
    assert!(before[4..].iter().all(|byte| *byte == b'a'));

    let keep_pair = instance.get_typed_func::<(i32, i32), (i32, i32)>(&store, "keep_pair")?;
    assert_eq!(keep_pair.call(&mut store, (7, 8))?, (7, 8));

    let mut after = vec![0; before.len()];
    memory.read(&store, 1024, &mut after)?;
    assert_eq!(after, before, "adapter entry must not corrupt static data");
    Ok(())
}

#[test]
fn flat_record_v1_reserves_generated_adapter_names() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
pub record Pair {
    left: Int32,
    right: Int32
}

fun __restrict_flat_record_v1_foo: () -> Int32 = {
    0
}

val __restrict_flat_record_v1_foo__depth = 1

pub fun foo: (pair: Pair) -> Pair = {
    pair
}

pub fun foo_: (pair: Pair) -> Pair = {
    pair
}
"#;

    let (wat, wasm) = compile_flat_record_v1(source)?;
    assert!(wat.contains("(export \"foo\" (func $__restrict_flat_record_v1_foo_))"));
    assert!(wat.contains("(export \"foo_\" (func $__restrict_flat_record_v1_foo__))"));
    assert!(wat.contains("(global $__restrict_flat_record_v1_foo__depth_ (mut i32)"));

    let (mut store, instance) = instantiate(&wasm)?;
    for export_name in ["foo", "foo_"] {
        let function = instance.get_typed_func::<(i32, i32), (i32, i32)>(&store, export_name)?;
        assert_eq!(function.call(&mut store, (3, 4))?, (3, 4));
    }
    Ok(())
}

#[test]
fn flat_record_v1_accepts_an_imported_public_record() -> Result<(), Box<dyn std::error::Error>> {
    let (_, root) = parse_program(
        r#"
import schema.{Pair}

pub fun keep_pair: (pair: Pair) -> Pair = {
    pair
}
"#,
    )
    .map_err(|error| format!("Parse error: {error:?}"))?;
    let mut module_sources = HashMap::new();
    module_sources.insert(
        "schema".to_string(),
        r#"
pub record Pair {
    left: Int32,
    right: Int32
}
"#
        .to_string(),
    );
    let program = resolve_program_imports_with_module_source_map(root, module_sources)?;

    let mut checker = TypeChecker::new();
    checker.check_program(&program)?;
    check_release_surface(&program, &checker, HostAbiProfile::FlatRecordV1)?;
    let checked_ir = build_checked_ir(&program, &checker)?;
    let wat = WasmCodeGen::with_host_abi_profile(HostAbiProfile::FlatRecordV1)
        .generate_checked(&program, &checked_ir)?;
    let wasm = wat::parse_str(&wat)?;
    wasmparser::Validator::new().validate_all(&wasm)?;

    let (mut store, instance) = instantiate(&wasm)?;
    let keep_pair = instance.get_typed_func::<(i32, i32), (i32, i32)>(&store, "keep_pair")?;
    assert_eq!(keep_pair.call(&mut store, (11, 12))?, (11, 12));
    Ok(())
}

#[derive(Default)]
struct ReentryState {
    entered: bool,
    nested_result: Option<(i32, i32)>,
}

#[test]
fn flat_record_v1_preserves_outer_values_during_same_export_reentry(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
pub record Pair {
    left: Int32,
    right: Int32
}

pub fun echo_after_print: (pair: Pair) -> Pair = {
    "reenter" |> println;
    pair
}
"#;
    let (_, wasm) = compile_flat_record_v1(source)?;

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm)?;
    let mut store = Store::new(&engine, ReentryState::default());
    let mut linker = Linker::new(&engine);
    linker.func_wrap(
        "wasi_snapshot_preview1",
        "fd_write",
        |mut caller: Caller<'_, ReentryState>,
         _fd: i32,
         _iovs: i32,
         _iovs_len: i32,
         _nwritten: i32|
         -> i32 {
            if !caller.data().entered {
                caller.data_mut().entered = true;
                let function = caller
                    .get_export("echo_after_print")
                    .and_then(|export| export.into_func())
                    .expect("reentrant export should be visible");
                let typed = function
                    .typed::<(i32, i32), (i32, i32)>(&caller)
                    .expect("reentrant export should have the flat record ABI");
                let nested_result = typed
                    .call(&mut caller, (91, 92))
                    .expect("same-export reentry should execute");
                caller.data_mut().nested_result = Some(nested_result);
            }
            0
        },
    )?;
    linker.func_wrap(
        "wasi_snapshot_preview1",
        "proc_exit",
        |_caller: Caller<'_, ReentryState>, _code: i32| {},
    )?;
    let instance = linker.instantiate_and_start(&mut store, &module)?;

    let function = instance.get_typed_func::<(i32, i32), (i32, i32)>(&store, "echo_after_print")?;
    assert_eq!(function.call(&mut store, (1, 2))?, (1, 2));
    assert_eq!(store.data().nested_result, Some((91, 92)));
    Ok(())
}

#[derive(Default)]
struct TrappingReentryState {
    attempted: bool,
    nested_trapped: bool,
}

#[test]
fn flat_record_v1_repairs_adapter_state_after_a_caught_reentrant_trap(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
pub record Boxed {
    value: Int32
}

pub fun divide_after_print: (boxed: Boxed) -> Boxed = {
    "reenter" |> println;
    val value = 100 / boxed.value;
    Boxed { value: value }
}
"#;
    let (_, wasm) = compile_flat_record_v1(source)?;

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm)?;
    let mut store = Store::new(&engine, TrappingReentryState::default());
    let mut linker = Linker::new(&engine);
    linker.func_wrap(
        "wasi_snapshot_preview1",
        "fd_write",
        |mut caller: Caller<'_, TrappingReentryState>,
         _fd: i32,
         _iovs: i32,
         _iovs_len: i32,
         _nwritten: i32|
         -> i32 {
            if !caller.data().attempted {
                caller.data_mut().attempted = true;
                let function = caller
                    .get_export("divide_after_print")
                    .and_then(|export| export.into_func())
                    .expect("reentrant export should be visible");
                let typed = function
                    .typed::<i32, i32>(&caller)
                    .expect("single-field record should flatten to one i32");
                let nested_trapped = typed.call(&mut caller, 0).is_err();
                caller.data_mut().nested_trapped = nested_trapped;
            }
            0
        },
    )?;
    linker.func_wrap(
        "wasi_snapshot_preview1",
        "proc_exit",
        |_caller: Caller<'_, TrappingReentryState>, _code: i32| {},
    )?;
    let instance = linker.instantiate_and_start(&mut store, &module)?;

    let function = instance.get_typed_func::<i32, i32>(&store, "divide_after_print")?;
    assert_eq!(function.call(&mut store, 2)?, 50);
    assert!(store.data().nested_trapped);
    assert_eq!(
        function.call(&mut store, 4)?,
        25,
        "a caught nested trap must not poison later adapter calls"
    );
    Ok(())
}

#[test]
fn flat_record_v1_recovers_from_an_escaping_trap_with_a_fresh_instance(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
pub record Boxed {
    value: Int32
}

pub fun divide: (boxed: Boxed) -> Boxed = {
    val value = 100 / boxed.value;
    Boxed { value: value }
}
"#;
    let (_, wasm) = compile_flat_record_v1(source)?;

    {
        let (mut trapped_store, trapped_instance) = instantiate(&wasm)?;
        let trapped_divide =
            trapped_instance.get_typed_func::<i32, i32>(&trapped_store, "divide")?;
        assert!(
            trapped_divide.call(&mut trapped_store, 0).is_err(),
            "division by zero should escape the adapter as a Wasm trap"
        );
    }

    let (mut fresh_store, fresh_instance) = instantiate(&wasm)?;
    let fresh_divide = fresh_instance.get_typed_func::<i32, i32>(&fresh_store, "divide")?;
    assert_eq!(fresh_divide.call(&mut fresh_store, 4)?, 25);
    Ok(())
}
