use restrict_lang::{parse_program, Program, TypeChecker, WasmCodeGen};
use wasmi::{Caller, Engine, Instance, Linker, Module, Store};

fn parse_source(source: &str) -> Result<Program, String> {
    let (remaining, program) = parse_program(source).map_err(|e| format!("Parse error: {e:?}"))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {remaining:?}"));
    }

    Ok(program)
}

fn type_check_source(source: &str) -> Result<(), String> {
    let program = parse_source(source)?;
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .map_err(|e| format!("Type error: {e}"))
}

fn compile_to_wat(source: &str) -> Result<String, String> {
    let program = parse_source(source)?;
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .map_err(|e| format!("Type error: {e}"))?;

    let mut codegen = WasmCodeGen::new();
    codegen
        .generate(&program)
        .map_err(|e| format!("Codegen error: {e}"))
}

fn instantiate(source: &str) -> Result<(Store<()>, Instance), Box<dyn std::error::Error>> {
    let wat = compile_to_wat(source)?;
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
fn dogfood_array_range_window_inference_example_type_checks() {
    let source = include_str!("../examples/dogfood_array_range_window_inference.rl");

    type_check_source(source).expect("Array/Range dogfood example should parse and type-check");
}

#[test]
fn dogfood_array_range_window_inference_example_generates_valid_wat() {
    let source = include_str!("../examples/dogfood_array_range_window_inference.rl");
    let wat = compile_to_wat(source).expect("Array/Range dogfood example should compile to WAT");

    assert!(
        wat.contains("(func $make_window"),
        "WAT should contain the window builder:\n{wat}"
    );
    assert!(
        wat.contains("(func $score_release_window"),
        "WAT should contain the exported scorer:\n{wat}"
    );
    assert!(
        wat.contains("i32.const 8 ;; range size"),
        "WAT should include Range<Int32> lowering:\n{wat}"
    );
    assert!(
        wat.contains("array size"),
        "WAT should include Array lowering:\n{wat}"
    );
    assert!(
        wat.contains("call $array_set"),
        "WAT should include mutable array update lowering:\n{wat}"
    );
    assert!(
        wat.contains("call $array_get"),
        "WAT should include array access lowering:\n{wat}"
    );
    assert!(
        wat.contains(";; Some literal") && wat.contains(";; None literal"),
        "WAT should include Option literal lowering:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("Array/Range dogfood example generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("Array/Range dogfood example generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn dogfood_array_range_window_inference_example_executes() -> Result<(), Box<dyn std::error::Error>>
{
    let source = include_str!("../examples/dogfood_array_range_window_inference.rl");
    let (mut store, instance) = instantiate(source)?;
    let score_release_window =
        instance.get_typed_func::<i32, i32>(&store, "score_release_window")?;

    assert_eq!(score_release_window.call(&mut store, 5)?, 17);
    assert_eq!(score_release_window.call(&mut store, 9)?, 25);
    Ok(())
}
