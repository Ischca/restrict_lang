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
fn dogfood_generic_context_inference_example_type_checks() {
    let source = include_str!("../examples/dogfood_generic_context_inference.rl");

    type_check_source(source)
        .expect("generic context inference dogfood example should parse and type-check");
}

#[test]
fn dogfood_generic_context_inference_example_generates_valid_wat() {
    let source = include_str!("../examples/dogfood_generic_context_inference.rl");
    let wat =
        compile_to_wat(source).expect("generic context inference dogfood should compile to WAT");

    assert!(
        wat.contains("(func $choose_primary"),
        "WAT should contain the user generic helper:\n{wat}"
    );
    assert!(
        wat.contains(";; Ok literal"),
        "WAT should include Result constructor lowering:\n{wat}"
    );
    assert!(
        wat.contains(";; None literal"),
        "WAT should include Option None lowering:\n{wat}"
    );
    assert!(
        wat.contains(";; fold(list, initial, reducer)"),
        "WAT should include List fold lowering:\n{wat}"
    );
    assert!(
        wat.contains(";; map(option, mapper)"),
        "WAT should include Option map lowering:\n{wat}"
    );
    assert!(
        wat.contains(";; filter(option, predicate)"),
        "WAT should include Option filter lowering:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("generic context inference dogfood generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!(
                "generic context inference dogfood generated invalid Wasm binary: {err}\n\n{wat}"
            );
        });
}

#[test]
fn dogfood_generic_context_inference_example_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = include_str!("../examples/dogfood_generic_context_inference.rl");
    let (mut store, instance) = instantiate(source)?;
    let score = instance.get_typed_func::<(), i32>(&store, "dogfood_generic_context_score")?;

    assert_eq!(score.call(&mut store, ())?, 3);
    Ok(())
}
