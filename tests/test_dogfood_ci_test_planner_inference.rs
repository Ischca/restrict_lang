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
fn dogfood_ci_test_planner_example_type_checks() {
    let source = include_str!("../examples/dogfood_ci_test_planner_inference.rl");

    type_check_source(source).expect("CI test planner dogfood example should parse and type-check");
}

#[test]
fn dogfood_ci_test_planner_wat_generation_is_valid_when_supported() {
    let source = include_str!("../examples/dogfood_ci_test_planner_inference.rl");
    let wat =
        compile_to_wat(source).expect("CI test planner dogfood example should generate valid WAT");

    assert!(
        wat.contains("(func $plan_ci_tests"),
        "WAT should contain the CI planner entry point:\n{wat}"
    );
    assert!(
        wat.contains("(func $blank_plan"),
        "WAT should contain the expected-type empty literal helper:\n{wat}"
    );
    assert!(
        wat.contains(";; filter(list, predicate)") && wat.contains(";; map(list, mapper)"),
        "WAT should include list filter and map lowering:\n{wat}"
    );
    assert!(
        wat.contains(";; fold(list, initial, reducer)"),
        "WAT should include list fold lowering:\n{wat}"
    );
    assert!(
        wat.contains(";; map(option, mapper)") && wat.contains(";; filter(option, predicate)"),
        "WAT should include Option map and filter lowering:\n{wat}"
    );
    assert!(
        wat.contains(";; Ok literal") && wat.contains(";; Err literal"),
        "WAT should include Result constructor lowering:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("CI test planner dogfood example generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("CI test planner dogfood example generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn dogfood_ci_test_planner_example_executes() {
    let dogfood_source = include_str!("../examples/dogfood_ci_test_planner_inference.rl").replace(
        "fun main: () -> CiPlan",
        "fun build_ci_test_plan: () -> CiPlan",
    );
    let source = format!(
        "{}\n{}",
        dogfood_source,
        r#"
export fun ci_test_planner_score: () -> Int32 = {
    val plan = () build_ci_test_plan;
    val CiPlan {
        total_score,
        selected_count,
        flaky_count,
        first_unowned,
        candidates,
        selected_ids,
        quarantined_suites,
        skipped_suites,
        route
    } = plan;
    val owner_score = first_unowned match {
        Some(owner) => { owner }
        None => { 0 }
    };
    val route_score = route match {
        Ok(score) => { score }
        Err(score) => { 0 - score }
    };
    val candidate_count = candidates |> list_count;
    val selected_id_count = selected_ids |> list_count;
    val quarantined_count = quarantined_suites |> list_count;
    val skipped_count = skipped_suites match {
        Some(suites) => { suites |> list_count }
        None => { 0 }
    };

    total_score + selected_count + flaky_count + owner_score + route_score + candidate_count + selected_id_count + quarantined_count + skipped_count
}
"#
    );
    let wat =
        compile_to_wat(&source).expect("CI test planner runtime wrapper should compile to WAT");

    let (mut store, instance) = instantiate_wat("CI test planner dogfood runtime", &wat);
    let ci_test_planner_score = instance
        .get_typed_func::<(), i32>(&store, "ci_test_planner_score")
        .expect("runtime wrapper should be host-callable");

    assert_eq!(
        ci_test_planner_score
            .call(&mut store, ())
            .expect("runtime wrapper should execute"),
        221
    );
}
