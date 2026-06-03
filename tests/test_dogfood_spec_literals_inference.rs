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
fn dogfood_spec_literals_inference_example_type_checks() {
    let source = include_str!("../examples/dogfood_spec_literals_inference.rl");

    type_check_source(source).expect("spec literal dogfood example should parse and type-check");
}

#[test]
fn dogfood_spec_literals_inference_example_generates_valid_wat() {
    let source = include_str!("../examples/dogfood_spec_literals_inference.rl");
    let wat = compile_to_wat(source).expect("spec literal dogfood example should compile to WAT");

    assert!(
        wat.contains("(func $plan_profile"),
        "WAT should contain the profile planner:\n{wat}"
    );
    assert!(
        wat.contains("(export \"exported_bias\" (func $exported_bias))"),
        "pub fun should emit a Wasm export:\n{wat}"
    );
    assert!(
        wat.contains("(export \"score_bias\" (global $score_bias))"),
        "pub val should emit a Wasm global export:\n{wat}"
    );
    assert!(
        wat.contains("i32.const 255"),
        "hex integer literal should lower to decimal i32 constant:\n{wat}"
    );
    assert!(
        wat.contains("i32.const 1000"),
        "underscored integer literal should lower without separators:\n{wat}"
    );
    assert!(
        wat.contains("f64.const 15000000000"),
        "lowercase exponent float literal should lower to f64 constant:\n{wat}"
    );
    assert!(
        wat.contains("f64.const 0.0314"),
        "uppercase signed exponent float literal should lower to f64 constant:\n{wat}"
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
        panic!("spec literal dogfood example generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("spec literal dogfood example generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn dogfood_spec_literals_inference_example_executes() {
    let source = format!(
        "{}\n{}",
        include_str!("../examples/dogfood_spec_literals_inference.rl"),
        r#"
export fun literal_profile_score: () -> Int32 = {
    val profile = LiteralProfile {
        name: "alpha\nbeta\t\\\"\'",
        separator: '\n',
        scale: 1.5e10,
        base: 0xFF,
        samples: [1_000, 2, 3],
        owner: None
    };
    val LiteralPlan {
        score,
        route,
        audit_ids,
        owner_seen
    } = profile |> plan_profile;

    score
}
"#
    );
    let wat = compile_to_wat(&source)
        .expect("spec literal dogfood runtime wrapper should compile to WAT");

    let (mut store, instance) = instantiate_wat("spec literal dogfood runtime", &wat);
    let literal_profile_score = instance
        .get_typed_func::<(), i32>(&store, "literal_profile_score")
        .expect("runtime wrapper should be host-callable");

    assert_eq!(
        literal_profile_score
            .call(&mut store, ())
            .expect("runtime wrapper should execute"),
        1333
    );
}
