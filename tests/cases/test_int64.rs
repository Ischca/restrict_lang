use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};
use wasmi::{Caller, Engine, Instance, Linker, Module, Store};

fn parse_and_check(source: &str) -> restrict_lang::ast::Program {
    let (remaining, ast) = parse_program(source).expect("source should parse");
    assert!(
        remaining.trim().is_empty(),
        "source should parse completely, remaining: {remaining:?}"
    );

    let mut type_checker = TypeChecker::new();
    type_checker
        .check_program(&ast)
        .expect("source should type check");

    ast
}

fn compile_to_wat(source: &str) -> String {
    let ast = parse_and_check(source);
    let mut codegen = WasmCodeGen::new();
    codegen.generate(&ast).expect("source should generate WAT")
}

fn instantiate(source: &str) -> Result<(Store<()>, Instance), Box<dyn std::error::Error>> {
    let wat = compile_to_wat(source);
    let wasm = wat::parse_str(&wat)?;
    wasmparser::Validator::new().validate_all(&wasm)?;

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm[..])?;
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
fn int64_annotations_and_literals_type_check() {
    parse_and_check(
        r#"
fun widen: (value: Int64) -> Int64 = {
    value + 2
}

fun main: () -> Int64 = {
    5_000_000_000
}
"#,
    );
}

#[test]
fn int64_codegen_uses_i64_and_validates() {
    let wat = compile_to_wat(
        r#"
export fun add_big: (value: Int64) -> Int64 = {
    value + 5_000_000_000
}
"#,
    );

    assert!(wat.contains("(param $value i64)"));
    assert!(wat.contains("(result i64)"));
    assert!(wat.contains("i64.const 5000000000"));
    assert!(wat.contains("i64.add"));

    let wasm = wat::parse_str(&wat).expect("Int64 WAT should parse");
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .expect("Int64 Wasm should validate");
}

#[test]
fn exported_int64_arithmetic_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun adjusted: (value: Int64) -> Int64 = {
    (value * 2) - 3
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let adjusted = instance.get_typed_func::<i64, i64>(&store, "adjusted")?;

    assert_eq!(adjusted.call(&mut store, 5_000_000_000)?, 9_999_999_997);
    Ok(())
}

#[test]
fn exported_int64_comparison_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun over_limit: (value: Int64) -> Boolean = {
    value > 5_000_000_000
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let over_limit = instance.get_typed_func::<i64, i32>(&store, "over_limit")?;

    assert_eq!(over_limit.call(&mut store, 5_000_000_001)?, 1);
    assert_eq!(over_limit.call(&mut store, 5_000_000_000)?, 0);
    Ok(())
}
