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

fn instantiate_wat(label: &str, wat: &str) -> (Store<()>, Instance) {
    let wasm = wat::parse_str(wat).unwrap_or_else(|err| {
        panic!("{label} generated invalid WAT: {err}\n\n{wat}");
    });

    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("{label} generated invalid Wasm binary: {err}\n\n{wat}");
        });

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm[..]).unwrap_or_else(|err| {
        panic!("{label} generated Wasm that wasmi cannot load: {err}\n\n{wat}");
    });
    let mut store = Store::new(&engine, ());
    let mut linker = Linker::new(&engine);

    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "fd_write",
            |_caller: Caller<'_, ()>,
             _fd: i32,
             _iovs: i32,
             _iovs_len: i32,
             _nwritten: i32|
             -> i32 { 0 },
        )
        .expect("fd_write stub should be registered");
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "proc_exit",
            |_caller: Caller<'_, ()>, _code: i32| {},
        )
        .expect("proc_exit stub should be registered");

    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .unwrap_or_else(|err| {
            panic!("{label} generated Wasm that wasmi cannot instantiate: {err}\n\n{wat}");
        });

    (store, instance)
}

#[test]
fn dogfood_list_tail_inference_example_type_checks() {
    let source = include_str!("../examples/dogfood_list_tail_inference.rl");

    type_check_source(source).expect("Int64 list-tail dogfood example should parse and type-check");
}

#[test]
fn dogfood_list_tail_inference_example_generates_valid_wat() {
    let source = include_str!("../examples/dogfood_list_tail_inference.rl");
    let wat = compile_to_wat(source).expect("Int64 list-tail dogfood should compile to WAT");

    assert!(
        wat.contains("(func $dogfood_list_tail_inference_score"),
        "WAT should contain the exported Int64 entry point:\n{wat}"
    );
    assert!(
        wat.contains("call $tail_i64"),
        "Int64 cons pattern should preserve the 8-byte tail layout:\n{wat}"
    );
    assert!(
        wat.contains(";; fold(list, initial, reducer)"),
        "WAT should include list fold lowering:\n{wat}"
    );
    assert!(
        wat.contains(";; Ok literal") && wat.contains(";; Err literal"),
        "WAT should include Result constructor lowering:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("Int64 list-tail dogfood generated invalid WAT: {err}\n\n{wat}")
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("Int64 list-tail dogfood generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn dogfood_list_tail_inference_example_executes() {
    let source = include_str!("../examples/dogfood_list_tail_inference.rl");
    let wat = compile_to_wat(source)
        .expect("Int64 list-tail dogfood runtime export should compile to WAT");

    let (mut store, instance) = instantiate_wat("Int64 list-tail dogfood runtime", &wat);
    let score = instance
        .get_typed_func::<(), i64>(&store, "dogfood_list_tail_inference_score")
        .expect("runtime export should be host-callable");

    assert_eq!(
        score
            .call(&mut store, ())
            .expect("runtime export should execute"),
        23_000_000_000
    );
}
