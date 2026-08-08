use restrict_lang::ir::builder::build_checked_ir;
use restrict_lang::{
    check_release_surface, parse_program, HostAbiProfile, TypeChecker, WasmCodeGen,
    WasmOptimizationLevel, WasmTargetProfile,
};
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use wasmi::{Engine, Instance, Linker, Module, Store, Val};
use wasmparser::{Parser, Payload, Validator};

fn compile_wat(
    source: &str,
    target: WasmTargetProfile,
    arena_bytes: u32,
) -> anyhow::Result<String> {
    compile_wat_with_options(
        source,
        target,
        arena_bytes,
        WasmOptimizationLevel::None,
        false,
    )
}

fn compile_wat_with_options(
    source: &str,
    target: WasmTargetProfile,
    arena_bytes: u32,
    optimization: WasmOptimizationLevel,
    instrument_memory: bool,
) -> anyhow::Result<String> {
    let (remaining, program) =
        parse_program(source).map_err(|error| anyhow::anyhow!("{error:?}"))?;
    anyhow::ensure!(remaining.trim().is_empty(), "source was not fully parsed");

    let mut type_checker = TypeChecker::new();
    type_checker.check_program(&program)?;
    check_release_surface(&program, &type_checker, HostAbiProfile::V001Scalar)?;
    let checked_ir = build_checked_ir(&program, &type_checker)?;
    let mut codegen = WasmCodeGen::with_host_abi_profile(HostAbiProfile::V001Scalar)
        .with_target_profile(target)
        .with_optimization_level(optimization)
        .with_memory_instrumentation(instrument_memory)
        .with_arena_size_bytes(arena_bytes)?;
    Ok(codegen.generate_checked(&program, &checked_ir)?)
}

fn exported_i32_global(instance: &Instance, store: &Store<()>, name: &str) -> anyhow::Result<i32> {
    let global = instance
        .get_global(store, name)
        .ok_or_else(|| anyhow::anyhow!("missing exported global '{name}'"))?;
    match global.get(store) {
        Val::I32(value) => Ok(value),
        other => anyhow::bail!("exported global '{name}' is not i32: {other:?}"),
    }
}

fn import_names(wasm: &[u8]) -> anyhow::Result<Vec<(String, String)>> {
    let mut imports = Vec::new();
    for payload in Parser::new(0).parse_all(wasm) {
        if let Payload::ImportSection(section) = payload? {
            for import in section.into_imports() {
                let import = import?;
                imports.push((import.module.to_string(), import.name.to_string()));
            }
        }
    }
    Ok(imports)
}

#[test]
fn wasm_core_is_import_free_and_executes_scalar_export() -> anyhow::Result<()> {
    let source = r#"
pub fun benchmark: (value: Int32) -> Int32 = {
    value * 3 + 1
}
"#;
    let wat = compile_wat(source, WasmTargetProfile::WasmCore, 4096)?;
    assert!(!wat.contains("wasi_snapshot_preview1"));

    let wasm = wat::parse_str(&wat)?;
    Validator::new().validate_all(&wasm)?;
    assert!(import_names(&wasm)?.is_empty());

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate_and_start(&mut store, &module)?;
    let benchmark = instance.get_typed_func::<i32, i32>(&store, "benchmark")?;
    assert_eq!(benchmark.call(&mut store, 14)?, 43);
    Ok(())
}

#[test]
fn wasm_core_rejects_host_output() -> anyhow::Result<()> {
    let source = r#"
fun main: () -> () = {
    "not host neutral" println
}
"#;
    let error = compile_wat(source, WasmTargetProfile::WasmCore, 4096)
        .expect_err("wasm-core must reject host output");
    let message = error.to_string();
    assert!(
        message.contains("wasip1") && message.contains("host I/O"),
        "{message}"
    );
    Ok(())
}

#[test]
fn wasip1_keeps_program_io_imports() -> anyhow::Result<()> {
    let source = r#"
fun main: () -> () = {
    "hello" println
}
"#;
    let wat = compile_wat(source, WasmTargetProfile::WasiP1, 4096)?;
    let wasm = wat::parse_str(&wat)?;
    let imports = import_names(&wasm)?;
    assert!(imports.contains(&("wasi_snapshot_preview1".to_string(), "fd_write".to_string())));
    Ok(())
}

#[test]
fn configurable_arena_changes_bounds_and_initial_memory() -> anyhow::Result<()> {
    let source = r#"
pub fun benchmark: (value: Int32) -> Int32 = {
    value + 1
}
"#;
    let wat = compile_wat(source, WasmTargetProfile::WasmCore, 131_072)?;
    assert!(wat.contains("i32.const 131072"));

    let wasm = wat::parse_str(&wat)?;
    Validator::new().validate_all(&wasm)?;
    Ok(())
}

#[test]
fn arena_exhaustion_records_a_machine_readable_diagnostic() -> anyhow::Result<()> {
    let source = r#"
record Pair {
    left: Int32
    right: Int32
}

pub fun benchmark: (value: Int32) -> Int32 = {
    val pair = Pair { left: value, right: value + 1 };
    pair.left
}
"#;
    let wat = compile_wat_with_options(
        source,
        WasmTargetProfile::WasmCore,
        8,
        WasmOptimizationLevel::Release,
        false,
    )?;
    let wasm = wat::parse_str(&wat)?;
    Validator::new().validate_all(&wasm)?;

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate_and_start(&mut store, &module)?;
    let benchmark = instance.get_typed_func::<i32, i32>(&store, "benchmark")?;
    benchmark
        .call(&mut store, 42)
        .expect_err("an eight-byte arena has no payload capacity");

    assert_eq!(
        exported_i32_global(&instance, &store, "__restrict_arena_error_code")?,
        1
    );
    assert!(exported_i32_global(&instance, &store, "__restrict_arena_error_requested_bytes")? > 0);
    assert_eq!(
        exported_i32_global(&instance, &store, "__restrict_arena_capacity_bytes")?,
        8
    );
    Ok(())
}

#[test]
fn memory_instrumentation_reports_peak_and_confirms_entry_reset() -> anyhow::Result<()> {
    let source = r#"
record Pair {
    left: Int32
    right: Int32
}

pub fun benchmark: (value: Int32) -> Int32 = {
    val pair = Pair { left: value, right: value + 1 };
    pair.left
}
"#;
    let wat = compile_wat_with_options(
        source,
        WasmTargetProfile::WasmCore,
        4096,
        WasmOptimizationLevel::Release,
        true,
    )?;
    let wasm = wat::parse_str(&wat)?;
    Validator::new().validate_all(&wasm)?;

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate_and_start(&mut store, &module)?;
    let reset = instance.get_typed_func::<(), ()>(&store, "__restrict_memory_metrics_reset")?;
    let benchmark = instance.get_typed_func::<i32, i32>(&store, "benchmark")?;
    reset.call(&mut store, ())?;
    assert_eq!(benchmark.call(&mut store, 42)?, 42);

    assert!(exported_i32_global(&instance, &store, "__restrict_arena_peak_bytes")? > 0);
    assert_eq!(
        exported_i32_global(&instance, &store, "__restrict_arena_live_bytes")?,
        0
    );
    assert_eq!(
        exported_i32_global(&instance, &store, "__restrict_arena_reset_count")?,
        1
    );
    assert!(exported_i32_global(&instance, &store, "__restrict_arena_allocation_count")? > 0);
    assert_eq!(
        exported_i32_global(&instance, &store, "__restrict_arena_error_code")?,
        0
    );

    reset.call(&mut store, ())?;
    assert_eq!(
        exported_i32_global(&instance, &store, "__restrict_arena_peak_bytes")?,
        0
    );
    assert_eq!(
        exported_i32_global(&instance, &store, "__restrict_arena_reset_count")?,
        0
    );
    Ok(())
}

#[test]
fn cli_emits_valid_binary_wasm_for_core_target() -> anyhow::Result<()> {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let source_path = std::env::temp_dir().join(format!("restrict_core_{unique}.rl"));
    let wasm_path = std::env::temp_dir().join(format!("restrict_core_{unique}.wasm"));
    fs::write(
        &source_path,
        r#"pub fun benchmark: (value: Int32) -> Int32 = { value * 2 }"#,
    )?;

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .args([
            "--target",
            "wasm-core",
            "--emit",
            "wasm",
            "--arena-bytes",
            "65536",
        ])
        .arg(&source_path)
        .arg(&wasm_path)
        .output()?;

    let _ = fs::remove_file(&source_path);
    anyhow::ensure!(
        output.status.success(),
        "compiler failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let wasm = fs::read(&wasm_path)?;
    let _ = fs::remove_file(&wasm_path);
    Validator::new().validate_all(&wasm)?;
    assert!(import_names(&wasm)?.is_empty());
    Ok(())
}
