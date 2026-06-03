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

fn assert_valid_wat(source: &str) {
    let wat = compile_to_wat(source).expect("task queue dogfood example should compile to WAT");

    assert!(
        wat.contains("(func $plan_queue"),
        "WAT should contain the queue planner:\n{wat}"
    );
    assert!(
        wat.contains("(func $empty_plan"),
        "WAT should contain the empty collection inference helper:\n{wat}"
    );
    assert!(
        wat.contains(";; filter(list, predicate)"),
        "WAT should include list filter lowering:\n{wat}"
    );
    assert!(
        wat.contains(";; map(list, mapper)"),
        "WAT should include list map lowering:\n{wat}"
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
        panic!("task queue dogfood example generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("task queue dogfood example generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn dogfood_inference_task_queue_example_type_checks() {
    let source = include_str!("../examples/dogfood_inference_task_queue.rl");

    type_check_source(source).expect("task queue dogfood example should parse and type-check");
}

#[test]
fn dogfood_inference_task_queue_example_generates_valid_wat() {
    let source = include_str!("../examples/dogfood_inference_task_queue.rl");

    assert_valid_wat(source);
}

#[test]
fn dogfood_inference_task_queue_example_executes() {
    let dogfood_source = include_str!("../examples/dogfood_inference_task_queue.rl").replace(
        "fun main: () -> QueuePlan",
        "fun build_task_queue_plan: () -> QueuePlan",
    );
    let source = format!(
        "{}\n{}",
        dogfood_source,
        r#"
export fun task_queue_score: () -> Int32 = {
    val plan = () build_task_queue_plan;
    val QueuePlan {
        total_score,
        ready_count,
        blocked_count,
        first_unowned,
        cards,
        alert_codes,
        deferred_ids
    } = plan;
    val owner_score = first_unowned match {
        Some(owner) => { owner }
        None => { 0 }
    };
    val card_count = cards |> list_count;
    val alert_count = alert_codes |> list_count;
    val deferred_count = deferred_ids match {
        Some(ids) => { ids |> list_count }
        None => { 0 }
    };

    total_score + ready_count + blocked_count + owner_score + card_count + alert_count + deferred_count
}
"#
    );
    let wat =
        compile_to_wat(&source).expect("task queue dogfood runtime wrapper should compile to WAT");

    let (mut store, instance) = instantiate_wat("task queue dogfood runtime", &wat);
    let task_queue_score = instance
        .get_typed_func::<(), i32>(&store, "task_queue_score")
        .expect("runtime wrapper should be host-callable");

    assert_eq!(
        task_queue_score
            .call(&mut store, ())
            .expect("runtime wrapper should execute"),
        191
    );
}
