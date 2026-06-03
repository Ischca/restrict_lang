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
fn dogfood_callable_codegen_handoff_example_type_checks() {
    let source = include_str!("../examples/dogfood_callable_codegen_handoff.rl");

    type_check_source(source).expect("callable codegen handoff dogfood should type-check");
}

#[test]
fn dogfood_callable_codegen_handoff_example_generates_valid_wat() {
    let source = include_str!("../examples/dogfood_callable_codegen_handoff.rl");
    let wat = compile_to_wat(source).expect("callable codegen handoff should compile to WAT");

    assert!(
        wat.contains("(func $immediate_then_score"),
        "WAT should contain the immediate then callable pipe case:\n{wat}"
    );
    assert!(
        wat.contains("(func $immediate_match_score"),
        "WAT should contain the immediate match callable pipe case:\n{wat}"
    );
    assert!(
        wat.contains("(func $option_handoff_score"),
        "WAT should contain the Option callable binding case:\n{wat}"
    );
    assert!(
        wat.contains("(func $lambda_"),
        "WAT should contain generated lambdas from inferred callable arms:\n{wat}"
    );
    assert!(
        wat.contains("fnref_launch_bonus_"),
        "WAT should lower a named function value arm through a function reference:\n{wat}"
    );
    assert!(
        wat.contains("mapper"),
        "WAT should declare or reference the Option match-bound callable:\n{wat}"
    );
    assert!(
        wat.contains("call_indirect"),
        "WAT should invoke inferred callable pipe targets indirectly:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("callable codegen handoff dogfood generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!(
                "callable codegen handoff dogfood generated invalid Wasm binary: {err}\n\n{wat}"
            );
        });
}

#[test]
fn dogfood_callable_codegen_handoff_example_executes() {
    let source = include_str!("../examples/dogfood_callable_codegen_handoff.rl").replace(
        "fun main: () -> Int32",
        "export fun callable_codegen_handoff_score: () -> Int32",
    );
    let wat = compile_to_wat(&source)
        .expect("callable codegen handoff dogfood runtime wrapper should compile to WAT");

    let (mut store, instance) = instantiate_wat("callable codegen handoff dogfood runtime", &wat);
    let score = instance
        .get_typed_func::<(), i32>(&store, "callable_codegen_handoff_score")
        .expect("runtime wrapper should be host-callable");

    assert_eq!(
        score
            .call(&mut store, ())
            .expect("runtime wrapper should execute"),
        58
    );
}
