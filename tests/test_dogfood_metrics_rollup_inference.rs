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
fn dogfood_metrics_rollup_example_type_checks() {
    let source = include_str!("../examples/dogfood_metrics_rollup_inference.rl");

    type_check_source(source).expect("metrics rollup dogfood example should parse and type-check");
}

#[test]
fn dogfood_metrics_rollup_example_generates_valid_wat() {
    let source = include_str!("../examples/dogfood_metrics_rollup_inference.rl");
    let wat = compile_to_wat(source).expect("metrics rollup dogfood example should compile to WAT");

    assert!(
        wat.contains("(func $rollup_metrics"),
        "WAT should contain the metrics rollup entry point:\n{wat}"
    );
    assert!(
        wat.contains("(func $blank_report"),
        "WAT should contain the expected-type empty collection helper:\n{wat}"
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
        panic!("metrics rollup dogfood example generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("metrics rollup dogfood example generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn dogfood_metrics_rollup_example_executes() {
    let dogfood_source = include_str!("../examples/dogfood_metrics_rollup_inference.rl").replace(
        "fun main: () -> MetricReport",
        "fun build_metrics_report: () -> MetricReport",
    );
    let source = format!(
        "{}\n{}",
        dogfood_source,
        r#"
export fun metrics_rollup_score: () -> Int32 = {
    val report = () build_metrics_report;
    val MetricReport {
        total_weighted,
        warning_count,
        critical_count,
        first_missing_previous,
        scored,
        warning_keys,
        sampled_keys
    } = report;
    val missing_score = first_missing_previous match {
        Some(key) => { key }
        None => { 0 }
    };
    val scored_count = scored |> list_count;
    val warning_key_count = warning_keys |> list_count;
    val sampled_count = sampled_keys match {
        Some(keys) => { keys |> list_count }
        None => { 0 }
    };

    total_weighted + warning_count + critical_count + missing_score + scored_count + warning_key_count + sampled_count
}
"#
    );
    let wat = compile_to_wat(&source)
        .expect("metrics rollup dogfood runtime wrapper should compile to WAT");

    let (mut store, instance) = instantiate_wat("metrics rollup dogfood runtime", &wat);
    let metrics_rollup_score = instance
        .get_typed_func::<(), i32>(&store, "metrics_rollup_score")
        .expect("runtime wrapper should be host-callable");

    assert_eq!(
        metrics_rollup_score
            .call(&mut store, ())
            .expect("runtime wrapper should execute"),
        450
    );
}

#[test]
fn generic_record_literal_infers_type_arg_from_fields() {
    let source = r#"
record MetricSample<T> {
    key: Int32,
    current: T,
    previous: Option<T>,
    weight: Int32
}

fun main: () -> Int32 = {
    val sample = MetricSample {
        key: 1,
        current: 42,
        previous: Some(41),
        weight: 1
    };

    sample.current
}
"#;

    type_check_source(source).expect("generic record literals should infer type args from fields");
}

#[test]
fn instantiated_generic_record_destructure_substitutes_fields() {
    let source = r#"
record MetricSlot<T> {
    value: T,
    fallback: Option<T>
}

fun read_plus_one: (slot: MetricSlot<Int32>) -> Int32 = {
    val MetricSlot {
        value,
        fallback
    } = slot;

    value + 1
}

fun main: () -> Int32 = {
    val slot: MetricSlot<Int32> = MetricSlot {
        value: 41,
        fallback: None
    };

    slot |> read_plus_one
}
"#;

    type_check_source(source)
        .expect("instantiated generic record destructuring should substitute field types");
}
