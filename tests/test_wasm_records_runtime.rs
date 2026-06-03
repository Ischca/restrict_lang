use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};
use wasmi::{Caller, Engine, Instance, Linker, Module, Store};

fn compile_to_wasm(source: &str) -> Result<Vec<u8>, String> {
    let (remaining, ast) = parse_program(source).map_err(|e| format!("Parse error: {e:?}"))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {remaining:?}"));
    }

    let mut type_checker = TypeChecker::new();
    type_checker
        .check_program(&ast)
        .map_err(|e| format!("Type error: {e}"))?;

    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&ast)
        .map_err(|e| format!("Codegen error: {e}"))?;

    wat::parse_str(&wat).map_err(|e| format!("Invalid generated WAT: {e}\n\n{wat}"))
}

fn instantiate(source: &str) -> Result<(Store<()>, Instance), Box<dyn std::error::Error>> {
    let wasm = compile_to_wasm(source)?;
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
fn record_field_destructuring_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
record Reading {
    measured: Float64,
    expected: Float64,
    verified: Boolean
}

fun delta: (reading: Reading) -> Float64 = {
    val Reading { measured, expected, verified } = reading;
    verified then {
        measured - expected
    } else {
        expected - measured
    }
}

export fun reading_delta: (measured: Float64, expected: Float64, verified: Boolean) -> Float64 = {
    val reading = Reading {
        measured: measured,
        expected: expected,
        verified: verified
    };
    reading |> delta
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let reading_delta = instance.get_typed_func::<(f64, f64, i32), f64>(&store, "reading_delta")?;

    assert_eq!(reading_delta.call(&mut store, (73.5, 70.0, 1))?, 3.5);
    assert_eq!(reading_delta.call(&mut store, (73.5, 70.0, 0))?, -3.5);
    Ok(())
}

#[test]
fn clone_update_preserves_existing_fields_at_runtime() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
record Profile {
    id: Int32,
    score: Int32,
    active: Boolean
}

fun patch_profile: (base: Profile, replacement: Int32) -> Profile = {
    base.clone {
        score: replacement,
        active: true
    }
}

export fun patched_profile_code: (base_id: Int32, replacement: Int32) -> Int32 = {
    val base = Profile {
        id: base_id,
        score: 10,
        active: false
    };
    val updated = (base, replacement) patch_profile;
    val Profile { id, score, active } = updated;
    active then {
        id * 100 + score
    } else {
        0
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let patched_profile_code =
        instance.get_typed_func::<(i32, i32), i32>(&store, "patched_profile_code")?;

    assert_eq!(patched_profile_code.call(&mut store, (7, 42))?, 742);
    assert_eq!(patched_profile_code.call(&mut store, (3, 5))?, 305);
    Ok(())
}

#[test]
fn exported_wrapper_can_destructure_record_main_result() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
record PipelineSummary {
    total: Int32,
    kept: Int32
}

fun main: () -> PipelineSummary = {
    PipelineSummary {
        total: 94,
        kept: 3
    }
}

export fun public_pipeline_score: () -> Int32 = {
    val summary = () main;
    val PipelineSummary { total, kept } = summary;

    total + kept
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let public_pipeline_score =
        instance.get_typed_func::<(), i32>(&store, "public_pipeline_score")?;

    assert_eq!(public_pipeline_score.call(&mut store, ())?, 97);
    Ok(())
}

#[test]
fn generic_record_clone_freeze_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
record Box<T> {
    value: T,
    backup: T
}

export fun refreshed_box_total: (replacement: Int32) -> Int32 = {
    val base: Box<Int32> = Box {
        value: 1,
        backup: 40
    };
    val updated = base.clone {
        value: replacement
    };
    val refreshed = updated freeze;
    val Box { value, backup } = refreshed;
    value + backup
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let refreshed_box_total = instance.get_typed_func::<i32, i32>(&store, "refreshed_box_total")?;

    assert_eq!(refreshed_box_total.call(&mut store, 2)?, 42);
    assert_eq!(refreshed_box_total.call(&mut store, 7)?, 47);
    Ok(())
}

#[test]
fn unannotated_local_generic_record_literal_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
record Bundle<T> {
    selected: Option<T>,
    history: List<T>
}

fun make_bundle: () -> Bundle<Int32> = {
    val bundle = Bundle {
        selected: None,
        history: []
    };
    bundle
}

export fun local_generic_bundle_score: () -> Int32 = {
    val bundle = () make_bundle;
    val Bundle { selected, history } = bundle;
    selected match {
        Some(value) => { value }
        None => { 42 }
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let local_generic_bundle_score =
        instance.get_typed_func::<(), i32>(&store, "local_generic_bundle_score")?;

    assert_eq!(local_generic_bundle_score.call(&mut store, ())?, 42);
    Ok(())
}

#[test]
fn unannotated_local_generic_record_literal_field_context_executes(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
record Box<T> {
    value: Option<T>
}

export fun local_generic_field_score: () -> Int32 = {
    val box = Box {
        value: None
    };
    box.value match {
        Some(value) => { value }
        None => { 42 }
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let local_generic_field_score =
        instance.get_typed_func::<(), i32>(&store, "local_generic_field_score")?;

    assert_eq!(local_generic_field_score.call(&mut store, ())?, 42);
    Ok(())
}

#[test]
fn unannotated_local_generic_record_result_field_context_executes(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
record Box<T> {
    result: Result<T, Int32>
}

export fun local_generic_result_field_score: () -> Int32 = {
    val box = Box {
        result: Err(7)
    };
    box.result match {
        Ok(value) => { value }
        Err(code) => { 42 }
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let local_generic_result_field_score =
        instance.get_typed_func::<(), i32>(&store, "local_generic_result_field_score")?;

    assert_eq!(local_generic_result_field_score.call(&mut store, ())?, 42);
    Ok(())
}

#[test]
fn unannotated_local_generic_record_result_list_payload_executes(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
record Box<T> {
    result: Result<List<T>, String>
}

fun first_i64: (values: List<Int64>) -> Int64 = {
    (values, 0) list_get
}

export fun local_generic_result_list_score: () -> Int64 = {
    val box = Box {
        result: Ok([1])
    };
    box.result match {
        Ok(values) => { values |> first_i64 }
        Err(message) => { 0 }
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let local_generic_result_list_score =
        instance.get_typed_func::<(), i64>(&store, "local_generic_result_list_score")?;

    assert_eq!(local_generic_result_list_score.call(&mut store, ())?, 1);
    Ok(())
}

#[test]
fn generic_record_wide_field_offsets_execute() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
record BoxPair<T> {
    first: T,
    second: Int32
}

export fun generic_record_offset_score: () -> Int32 = {
    val pair: BoxPair<Float64> = BoxPair {
        first: 1.5,
        second: 41
    };
    pair.second + 1
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let generic_record_offset_score =
        instance.get_typed_func::<(), i32>(&store, "generic_record_offset_score")?;

    assert_eq!(generic_record_offset_score.call(&mut store, ())?, 42);
    Ok(())
}

#[test]
fn generic_record_pair_float_first_and_int_second_execute() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r#"
record Pair<T> {
    first: T,
    second: Int32
}

export fun generic_pair_destructure_score: () -> Int32 = {
    val pair: Pair<Float64> = Pair {
        first: 1.5,
        second: 41
    };
    val Pair { first, second } = pair;
    first == 1.5 then {
        second + 1
    } else {
        0
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let generic_pair_destructure_score =
        instance.get_typed_func::<(), i32>(&store, "generic_pair_destructure_score")?;

    assert_eq!(generic_pair_destructure_score.call(&mut store, ())?, 42);
    Ok(())
}

#[test]
fn generic_record_pair_rest_keeps_instantiated_offsets() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
record Pair<T> {
    first: T,
    second: Int32
}

export fun generic_pair_rest_score: () -> Int32 = {
    val pair: Pair<Float64> = Pair {
        first: 1.5,
        second: 41
    };
    val Pair { first, ...rest } = pair;
    first == 1.5 then {
        rest.second + 1
    } else {
        0
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let generic_pair_rest_score =
        instance.get_typed_func::<(), i32>(&store, "generic_pair_rest_score")?;

    assert_eq!(generic_pair_rest_score.call(&mut store, ())?, 42);
    Ok(())
}

#[test]
fn record_rest_match_with_wide_field_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
record Incident {
    owner: Int32,
    score: Float64,
    code: Int32
}

export fun rest_match_score: () -> Int32 = {
    val incident = Incident {
        owner: 7,
        score: 1.5,
        code: 35
    };

    incident match {
        Incident { owner, ...rest } => {
            owner + rest.code
        }
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let rest_match_score = instance.get_typed_func::<(), i32>(&store, "rest_match_score")?;

    assert_eq!(rest_match_score.call(&mut store, ())?, 42);
    Ok(())
}

#[test]
fn impl_method_dispatch_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
record Score {
    value: Float64
}

impl Score {
    fun risk: (self: Score) = {
        self.value + 0.5
    }
}

export fun method_score: () -> Float64 = {
    val score = Score { value: 41.5 };
    (score) risk
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let method_score = instance.get_typed_func::<(), f64>(&store, "method_score")?;

    assert_eq!(method_score.call(&mut store, ())?, 42.0);
    Ok(())
}

#[test]
fn generic_impl_method_dispatch_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
record ScoreBox {
    seed: Int32
}

impl ScoreBox {
    fun keep: <T>(self: ScoreBox, value: T) -> T = {
        value
    }
}

export fun generic_method_score: (base: Int32) -> Int32 = {
    val box = ScoreBox { seed: 1 };
    (box, base + 1) keep
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let generic_method_score =
        instance.get_typed_func::<i32, i32>(&store, "generic_method_score")?;

    assert_eq!(generic_method_score.call(&mut store, 41)?, 42);
    Ok(())
}

#[test]
fn top_level_constant_export_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
val release_bias = 3

export fun biased: (score: Int32) -> Int32 = {
    score + release_bias
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let biased = instance.get_typed_func::<i32, i32>(&store, "biased")?;

    assert_eq!(biased.call(&mut store, 39)?, 42);
    Ok(())
}
