use restrict_lang::{parse_program, Program, TypeChecker, WasmCodeGen};
use wasmi::{Caller, Engine, Instance, Linker, Module, Store};

fn parse_source(source: &str) -> Result<Program, String> {
    let (remaining, program) = parse_program(source).map_err(|e| format!("Parse error: {e:?}"))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {remaining:?}"));
    }

    Ok(program)
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
fn release_readiness_example_executes_through_exported_wrapper() {
    let source = include_str!("../examples/release_readiness.rl").replace(
        "fun main: () -> ReleaseDecision",
        "fun release_readiness_decision: () -> ReleaseDecision",
    ) + r#"

export fun release_readiness_score: () -> Int32 = {
    val decision = () release_readiness_decision;
    val ReleaseDecision {
        approved,
        risk_score,
        uncovered_count,
        missing_owner,
        blocker_codes,
        review_scores
    } = decision;
    val approved_score = approved then {
        1000
    } else {
        0
    };
    val missing_owner_score = missing_owner match {
        Some(owner) => { owner }
        None => { 0 }
    };
    val blocker_count = blocker_codes |> list_count;
    val review_count = review_scores |> list_count;

    approved_score
        + risk_score
        + uncovered_count
        + missing_owner_score
        + blocker_count
        + review_count
}
"#;

    let wat =
        compile_to_wat(&source).expect("release readiness runtime wrapper should compile to WAT");
    let (mut store, instance) = instantiate_wat("release readiness runtime", &wat);
    let release_readiness_score = instance
        .get_typed_func::<(), i32>(&store, "release_readiness_score")
        .expect("runtime wrapper should be host-callable");

    assert_eq!(
        release_readiness_score
            .call(&mut store, ())
            .expect("runtime wrapper should execute"),
        105
    );
}
