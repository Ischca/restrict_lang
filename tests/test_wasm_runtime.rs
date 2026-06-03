use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};
use wasmi::{Caller, Engine, Instance, Linker, Module, Store, TrapCode};

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
fn exported_int_function_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun public_score: (value: Int32) -> Int32 = {
    value + 1
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let public_score = instance.get_typed_func::<i32, i32>(&store, "public_score")?;

    assert_eq!(public_score.call(&mut store, 41)?, 42);
    Ok(())
}

#[test]
fn exported_char_function_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun echo_delimiter: (value: Char) -> Char = {
    value
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let echo_delimiter = instance.get_typed_func::<i32, i32>(&store, "echo_delimiter")?;

    assert_eq!(echo_delimiter.call(&mut store, ':' as i32)?, ':' as i32);
    Ok(())
}

#[test]
fn exported_float_function_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun adjusted_score: (value: Float64) -> Float64 = {
    value + 0.5
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let adjusted_score = instance.get_typed_func::<f64, f64>(&store, "adjusted_score")?;

    assert_eq!(adjusted_score.call(&mut store, 41.5)?, 42.0);
    Ok(())
}

#[test]
fn annotated_float_lambda_preserves_runtime_abi() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun adjusted_pipeline: (value: Float64) -> Float64 = {
    val adjust = |x: Float64| x + 0.5;
    value |> adjust
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let adjusted_pipeline = instance.get_typed_func::<f64, f64>(&store, "adjusted_pipeline")?;

    assert_eq!(adjusted_pipeline.call(&mut store, 41.5)?, 42.0);
    Ok(())
}

#[test]
fn exported_boolean_function_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun should_launch: (risk: Int32) -> Boolean = {
    risk < 10
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let should_launch = instance.get_typed_func::<i32, i32>(&store, "should_launch")?;

    assert_eq!(should_launch.call(&mut store, 9)?, 1);
    assert_eq!(should_launch.call(&mut store, 10)?, 0);
    Ok(())
}

#[test]
fn exported_function_calls_another_restrict_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
fun risk_bonus: (risk: Int32) -> Int32 = {
    risk * 2
}

export fun final_score: (base: Int32, risk: Int32) -> Int32 = {
    base + (risk |> risk_bonus)
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let final_score = instance.get_typed_func::<(i32, i32), i32>(&store, "final_score")?;

    assert_eq!(final_score.call(&mut store, (10, 4))?, 18);
    Ok(())
}

#[test]
fn exported_wrapper_can_call_scalar_main() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
fun increment: (value: Int32) -> Int32 = {
    value + 1
}

fun main: () -> Int32 = {
    41 |> increment
}

export fun public_main_score: () -> Int32 = {
    val score = () main;
    score + 1
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let public_main_score = instance.get_typed_func::<(), i32>(&store, "public_main_score")?;

    assert_eq!(public_main_score.call(&mut store, ())?, 43);
    Ok(())
}

#[test]
fn exported_branch_arithmetic_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun launch_score: (risk: Int32, limit: Int32) -> Int32 = {
    (risk < limit) then {
        limit - risk
    } else {
        0
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let launch_score = instance.get_typed_func::<(i32, i32), i32>(&store, "launch_score")?;

    assert_eq!(launch_score.call(&mut store, (7, 10))?, 3);
    assert_eq!(launch_score.call(&mut store, (12, 10))?, 0);
    Ok(())
}

#[test]
fn exported_chained_and_nested_conditionals_execute() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun risk_bucket: (score: Int32) -> Int32 = {
    score < 0 then {
        0
    } else score < 10 then {
        1
    } else {
        2
    }
}

export fun nested_gate: (enabled: Boolean, score: Int32) -> Int32 = {
    enabled then {
        score >= 10 then {
            2
        } else {
            1
        }
    } else {
        0
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let risk_bucket = instance.get_typed_func::<i32, i32>(&store, "risk_bucket")?;
    let nested_gate = instance.get_typed_func::<(i32, i32), i32>(&store, "nested_gate")?;

    assert_eq!(risk_bucket.call(&mut store, -1)?, 0);
    assert_eq!(risk_bucket.call(&mut store, 5)?, 1);
    assert_eq!(risk_bucket.call(&mut store, 10)?, 2);
    assert_eq!(nested_gate.call(&mut store, (0, 20))?, 0);
    assert_eq!(nested_gate.call(&mut store, (1, 9))?, 1);
    assert_eq!(nested_gate.call(&mut store, (1, 10))?, 2);
    Ok(())
}

#[test]
fn exported_unit_conditional_without_else_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun maybe_do_unit: (enabled: Boolean) -> () = {
    enabled then {
        ()
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let maybe_do_unit = instance.get_typed_func::<i32, ()>(&store, "maybe_do_unit")?;

    maybe_do_unit.call(&mut store, 0)?;
    maybe_do_unit.call(&mut store, 1)?;
    Ok(())
}

#[test]
fn exported_recursive_factorial_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
fun factorial_inner: (value: Int32) -> Int32 = {
    value <= 1 then {
        1
    } else {
        val next = value - 1;
        value * (next factorial_inner)
    }
}

export fun recursive_factorial: (value: Int32) -> Int32 = {
    value |> factorial_inner
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let recursive_factorial = instance.get_typed_func::<i32, i32>(&store, "recursive_factorial")?;

    assert_eq!(recursive_factorial.call(&mut store, 0)?, 1);
    assert_eq!(recursive_factorial.call(&mut store, 1)?, 1);
    assert_eq!(recursive_factorial.call(&mut store, 5)?, 120);
    Ok(())
}

#[test]
fn exported_mutual_recursion_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
fun is_even: (value: Int32) -> Boolean = {
    value == 0 then {
        true
    } else {
        val next = value - 1;
        next |> is_odd
    }
}

fun is_odd: (value: Int32) -> Boolean = {
    value == 0 then {
        false
    } else {
        val next = value - 1;
        next |> is_even
    }
}

export fun even_flag: (value: Int32) -> Boolean = {
    value |> is_even
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let even_flag = instance.get_typed_func::<i32, i32>(&store, "even_flag")?;

    assert_eq!(even_flag.call(&mut store, 0)?, 1);
    assert_eq!(even_flag.call(&mut store, 7)?, 0);
    assert_eq!(even_flag.call(&mut store, 8)?, 1);
    Ok(())
}

#[test]
fn exported_expected_type_lambda_pipeline_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
fun apply_int: (f: Int32 -> Int32, value: Int32) -> Int32 = {
    value |> f
}

export fun scored_pipeline: (base: Int32) -> Int32 = {
    val add_release_buffer: Int32 -> Int32 = |score| score + 3;
    val buffered = base |> add_release_buffer;
    (|score| score * 2, buffered) apply_int
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let scored_pipeline = instance.get_typed_func::<i32, i32>(&store, "scored_pipeline")?;

    assert_eq!(scored_pipeline.call(&mut store, 7)?, 20);
    Ok(())
}

#[test]
fn exported_generic_choice_of_annotated_lambda_executes() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun apply_int: (f: Int32 -> Int32, value: Int32) -> Int32 = {
    value |> f
}

export fun chosen_lambda_score: (base: Int32) -> Int32 = {
    val chosen = (|score: Int32| score + 1, |fallback: Int32| fallback) choose_first;
    (chosen, base) apply_int
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let chosen_lambda_score = instance.get_typed_func::<i32, i32>(&store, "chosen_lambda_score")?;

    assert_eq!(chosen_lambda_score.call(&mut store, 41)?, 42);
    Ok(())
}

#[test]
fn exported_then_produced_lambda_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun branch_lambda_score: (flag: Boolean, value: Int32) -> Int32 = {
    val adjust = flag then {
        |score| score + 1
    } else {
        |score| score * 2
    };
    value |> adjust
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let branch_lambda_score =
        instance.get_typed_func::<(i32, i32), i32>(&store, "branch_lambda_score")?;

    assert_eq!(branch_lambda_score.call(&mut store, (1, 41))?, 42);
    assert_eq!(branch_lambda_score.call(&mut store, (0, 21))?, 42);
    Ok(())
}

#[test]
fn exported_then_produced_lambda_with_prefix_bindings_executes(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun branch_lambda_score_with_prefix: (flag: Boolean, value: Int32) -> Int32 = {
    val adjust = flag then {
        val bonus = 2;
        |score| score + bonus
    } else {
        val factor = 2;
        |score| score * factor
    };
    value |> adjust
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let branch_lambda_score_with_prefix =
        instance.get_typed_func::<(i32, i32), i32>(&store, "branch_lambda_score_with_prefix")?;

    assert_eq!(
        branch_lambda_score_with_prefix.call(&mut store, (1, 40))?,
        42
    );
    assert_eq!(
        branch_lambda_score_with_prefix.call(&mut store, (0, 21))?,
        42
    );
    Ok(())
}

#[test]
fn exported_match_produced_lambda_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun match_lambda_score: (flag: Boolean, value: Int32) -> Int32 = {
    val maybe_bonus = flag then {
        Some(2)
    } else {
        None
    };
    val adjust = maybe_bonus match {
        Some(bonus) => { |score| score + bonus }
        None => { |score| score }
    };
    value |> adjust
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let match_lambda_score =
        instance.get_typed_func::<(i32, i32), i32>(&store, "match_lambda_score")?;

    assert_eq!(match_lambda_score.call(&mut store, (1, 40))?, 42);
    assert_eq!(match_lambda_score.call(&mut store, (0, 42))?, 42);
    Ok(())
}

#[test]
fn exported_generic_function_value_call_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
fun identity_local: <T>(value: T) -> T = {
    value
}

fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

export fun generic_function_value_score: (base: Int32) -> Int32 = {
    val keep = identity_local;
    val choose = choose_first;
    val kept = base |> keep;
    (kept, 1) choose
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let generic_function_value_score =
        instance.get_typed_func::<i32, i32>(&store, "generic_function_value_score")?;

    assert_eq!(generic_function_value_score.call(&mut store, 41)?, 41);
    Ok(())
}

#[test]
fn exported_apply_surface_equivalence_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
fun identity_local: <T>(value: T) -> T = {
    value
}

export fun apply_surface_score: (base: Int32) -> Int32 = {
    val generic_value = identity_local;
    val parenthesized_generic_value = identity_local;
    val deferred_lambda = |value| value;
    val direct = base |> identity_local;
    val named_value = base |> generic_value;
    val parenthesized_value = base |> (parenthesized_generic_value);
    val immediate_lambda = base |> (|value| value);
    val local_lambda = base |> deferred_lambda;
    direct + named_value + parenthesized_value + immediate_lambda + local_lambda
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let apply_surface_score = instance.get_typed_func::<i32, i32>(&store, "apply_surface_score")?;

    assert_eq!(apply_surface_score.call(&mut store, 8)?, 40);
    Ok(())
}

#[test]
fn user_defined_context_binding_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
context RuntimeScale {
    factor: Int32,
    offset: Int32
}

export fun context_score: (input: Int32, adjustment: Int32) -> Int32 = {
    with RuntimeScale { factor: input, offset: adjustment } {
        factor * 10 + offset
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let context_score = instance.get_typed_func::<(i32, i32), i32>(&store, "context_score")?;

    assert_eq!(context_score.call(&mut store, (4, 2))?, 42);
    assert_eq!(context_score.call(&mut store, (7, 5))?, 75);
    Ok(())
}

#[test]
fn exported_list_map_filter_fold_pipeline_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun scored_iterator_pipeline: () -> Int32 = {
    with Arena {
        val values = [1, 2, 3, 4];
        val shifted = (values, |value| value + 1) map;
        val kept = (shifted, |value| value > 3) filter;
        (kept, 0, |total, value| total + value) fold
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let scored_iterator_pipeline =
        instance.get_typed_func::<(), i32>(&store, "scored_iterator_pipeline")?;

    assert_eq!(scored_iterator_pipeline.call(&mut store, ())?, 9);
    Ok(())
}

#[test]
fn exported_named_function_iterators_execute() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
fun add_one: (value: Int32) -> Int32 = {
    value + 1
}

fun keep_large: (value: Int32) -> Boolean = {
    value > 11
}

fun add_total: (total: Int32, value: Int32) -> Int32 = {
    total + value
}

export fun named_iterator_pipeline: () -> Int32 = {
    with Arena {
        val values = [10, 11, 12];
        val shifted = (values, add_one) map;
        val kept = (shifted, keep_large) filter;
        (kept, 0, add_total) fold
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let named_iterator_pipeline =
        instance.get_typed_func::<(), i32>(&store, "named_iterator_pipeline")?;

    assert_eq!(named_iterator_pipeline.call(&mut store, ())?, 25);
    Ok(())
}

#[test]
fn exported_option_map_filter_pipeline_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun option_pipeline_score: (input: Int32) -> Int32 = {
    val maybe_value: Option<Int32> = Some(input);
    val shifted = (maybe_value, |value| value + 1) map;
    val kept = (shifted, |value| value > 10) filter;

    kept match {
        Some(value) => { value }
        None => { 0 }
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let option_pipeline_score =
        instance.get_typed_func::<i32, i32>(&store, "option_pipeline_score")?;

    assert_eq!(option_pipeline_score.call(&mut store, 9)?, 0);
    assert_eq!(option_pipeline_score.call(&mut store, 10)?, 11);
    Ok(())
}

#[test]
fn exported_result_match_executes_for_ok_and_err() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
fun decide_result: (ok: Boolean, score: Int32) -> Result<Int32, Int32> = {
    ok then {
        Ok(score + 1)
    } else {
        Err(score + 10)
    }
}

export fun result_pipeline_score: (ok: Boolean, score: Int32) -> Int32 = {
    val decision = (ok, score) decide_result;

    decision match {
        Ok(value) => { value }
        Err(error) => { error * 2 }
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let result_pipeline_score =
        instance.get_typed_func::<(i32, i32), i32>(&store, "result_pipeline_score")?;

    assert_eq!(result_pipeline_score.call(&mut store, (1, 4))?, 5);
    assert_eq!(result_pipeline_score.call(&mut store, (0, 4))?, 28);
    Ok(())
}

#[test]
fn exported_int64_iterator_pipeline_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun int64_iterator_pipeline: () -> Int64 = {
    with Arena {
        val readings: List<Int64> = [5000000000, 6000000000];
        val shifted = (readings, |value| value + 10000000000) map;
        val kept = (shifted, |value| value > 15000000000) filter;
        val initial: Int64 = 0;
        (kept, initial, |total, value| total + value) fold
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let int64_iterator_pipeline =
        instance.get_typed_func::<(), i64>(&store, "int64_iterator_pipeline")?;

    assert_eq!(int64_iterator_pipeline.call(&mut store, ())?, 16000000000);
    Ok(())
}

#[test]
fn exported_float_option_result_payload_matches_execute() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r#"
fun choose_offset: (enabled: Boolean) -> Option<Float64> = {
    enabled then {
        Some(1.5)
    } else {
        None
    }
}

fun decode_ratio: (ok: Boolean) -> Result<Float64, Int32> = {
    ok then {
        Ok(2.5)
    } else {
        Err(4)
    }
}

export fun float_option_score: (enabled: Boolean) -> Float64 = {
    val offset = enabled |> choose_offset;

    offset match {
        Some(value) => {
            value + 0.25
        }
        None => {
            0.0
        }
    }
}

export fun float_result_score: (ok: Boolean) -> Float64 = {
    val decoded = ok |> decode_ratio;

    decoded match {
        Ok(value) => {
            value + 0.5
        }
        Err(code) => {
            0.0
        }
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let float_option_score = instance.get_typed_func::<i32, f64>(&store, "float_option_score")?;
    let float_result_score = instance.get_typed_func::<i32, f64>(&store, "float_result_score")?;

    assert_eq!(float_option_score.call(&mut store, 1)?, 1.75);
    assert_eq!(float_option_score.call(&mut store, 0)?, 0.0);
    assert_eq!(float_result_score.call(&mut store, 1)?, 3.0);
    assert_eq!(float_result_score.call(&mut store, 0)?, 0.0);
    Ok(())
}

#[test]
fn exported_unannotated_branch_option_float_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
fun choose_offset: (missing: Boolean) = {
    missing then {
        None
    } else {
        Some(1.5)
    }
}

export fun inferred_float_option_score: (missing: Boolean) -> Float64 = {
    val offset = missing |> choose_offset;

    offset match {
        Some(value) => {
            value + 0.25
        }
        None => {
            0.0
        }
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let inferred_float_option_score =
        instance.get_typed_func::<i32, f64>(&store, "inferred_float_option_score")?;

    assert_eq!(inferred_float_option_score.call(&mut store, 0)?, 1.75);
    assert_eq!(inferred_float_option_score.call(&mut store, 1)?, 0.0);
    Ok(())
}

#[test]
fn exported_string_char_float_literal_patterns_execute() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
record Route {
    status: String,
    load: Float64
}

fun status_points: (status: String) -> Int32 = {
    status match {
        "ok" => {
            0
        }
        "page" => {
            10
        }
        _ => {
            1
        }
    }
}

fun load_points: (load: Float64) -> Int32 = {
    load match {
        0.0 => {
            0
        }
        1.0 => {
            5
        }
        _ => {
            2
        }
    }
}

fun code_points: (code: Char) -> Int32 = {
    code match {
        'A' => {
            7
        }
        _ => {
            0
        }
    }
}

export fun literal_pattern_score: () -> Int32 = {
    val route = Route { status: "page", load: 1.0 };
    val by_record = route match {
        Route { status: "page", load: 1.0 } => {
            3
        }
        _ => {
            0
        }
    };
    val status_score = "page" |> status_points;
    val load_score = 1.0 |> load_points;
    val code_score = 'A' |> code_points;

    by_record + status_score + load_score + code_score
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let literal_pattern_score =
        instance.get_typed_func::<(), i32>(&store, "literal_pattern_score")?;

    assert_eq!(literal_pattern_score.call(&mut store, ())?, 25);
    Ok(())
}

#[test]
fn exported_stdlib_value_functions_execute() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun std_math_score: () -> Int32 = {
    val a = -5 |> abs;
    val b = (10, 20) max;
    val c = (3, 7) min;
    val d = (2, 3) pow;
    val e = 4 |> factorial;

    a + b + c + d + e
}

export fun std_prelude_score: () -> Boolean = {
    val bool_not = false |> not;
    val bool_and = (bool_not, true) and;
    val bool_or = (bool_and, false) or;
    (bool_or, "prelude boolean flow") assert;

    bool_or
}

export fun std_option_score: () -> Int32 = {
    mut val some: Option<Int32> = Some(42);
    mut val none: Option<Int32> = None;
    val has_value = some |> option_is_some;
    val is_missing = none |> option_is_none;
    val value = (some, 0) option_unwrap_or;
    val fallback = (none, 7) option_unwrap_or;

    (has_value && is_missing) then {
        value + fallback
    } else {
        0
    }
}

export fun std_list_access_score: () -> Int32 = {
    mut val numbers = [1, 2, 3];
    val empty = numbers |> list_is_empty;
    val first = numbers |> list_head;
    val rest = numbers |> list_tail;
    val reversed = numbers |> list_reverse;
    val first_value = first match {
        Some(value) => { value }
        None => { 0 }
    };
    val rest_count = rest match {
        Some(items) => { items |> list_count }
        None => { 0 }
    };
    val reversed_first = (reversed, 0) list_get;

    empty then {
        0
    } else {
        first_value + rest_count + reversed_first
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let std_math_score = instance.get_typed_func::<(), i32>(&store, "std_math_score")?;
    let std_prelude_score = instance.get_typed_func::<(), i32>(&store, "std_prelude_score")?;
    let std_option_score = instance.get_typed_func::<(), i32>(&store, "std_option_score")?;
    let std_list_access_score =
        instance.get_typed_func::<(), i32>(&store, "std_list_access_score")?;

    assert_eq!(std_math_score.call(&mut store, ())?, 60);
    assert_eq!(std_prelude_score.call(&mut store, ())?, 1);
    assert_eq!(std_option_score.call(&mut store, ())?, 49);
    assert_eq!(std_list_access_score.call(&mut store, ())?, 6);
    Ok(())
}

#[test]
fn exported_float_stdlib_helpers_execute() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun float_stdlib_score: () -> Float64 = {
    mut val readings = [1.5, 2.5, 3.5];
    mut val some: Option<Float64> = Some(1.5);
    mut val none: Option<Float64> = None;
    val maybe_head = readings |> list_head;
    val maybe_tail = readings |> list_tail;
    val reversed = readings |> list_reverse;
    val head_value = maybe_head match {
        Some(value) => { value }
        None => { 0.0 }
    };
    val tail_count = maybe_tail match {
        Some(rest) => { rest |> list_count }
        None => { 0 }
    };
    val first_reversed = (reversed, 0) list_get;
    val option_value = (some, 0.0) option_unwrap_or;
    val option_fallback = (none, 2.5) option_unwrap_or;

    tail_count > 0 then {
        head_value + first_reversed + option_value + option_fallback
    } else {
        head_value
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let float_stdlib_score = instance.get_typed_func::<(), f64>(&store, "float_stdlib_score")?;

    assert_eq!(float_stdlib_score.call(&mut store, ())?, 9.0);
    Ok(())
}

#[test]
fn specialized_array_set_runtime_abi_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun update_float64_array: () -> Float64 = {
    with Arena {
        mut val readings: Array<Float64, 2> = [1.5, 2.5];
        (readings, 0, 3.5) array_set;
        (readings, 0) array_get
    }
}

export fun update_int64_array: () -> Int64 = {
    with Arena {
        mut val counters: Array<Int64, 2> = [10000000000, 20000000000];
        (counters, 1, 30000000000) array_set;
        (counters, 1) array_get
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let update_float64_array =
        instance.get_typed_func::<(), f64>(&store, "update_float64_array")?;
    let update_int64_array = instance.get_typed_func::<(), i64>(&store, "update_int64_array")?;

    assert_eq!(update_float64_array.call(&mut store, ())?, 3.5);
    assert_eq!(update_int64_array.call(&mut store, ())?, 30000000000);
    assert_eq!(update_float64_array.call(&mut store, ())?, 3.5);
    Ok(())
}

#[test]
fn array_get_and_set_trap_on_out_of_bounds_indexes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun read_index_at_length: () -> Int32 = {
    with Arena {
        val readings: Array<Int32, 2> = [10, 20];
        (readings, 2) array_get
    }
}

export fun read_negative_index: () -> Int32 = {
    with Arena {
        val readings: Array<Int32, 2> = [10, 20];
        val index = 0 - 1;
        (readings, index) array_get
    }
}

export fun write_index_at_length: () -> Int32 = {
    with Arena {
        mut val readings: Array<Int32, 2> = [10, 20];
        (readings, 2, 30) array_set;
        0
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let read_index_at_length =
        instance.get_typed_func::<(), i32>(&store, "read_index_at_length")?;
    let read_negative_index = instance.get_typed_func::<(), i32>(&store, "read_negative_index")?;
    let write_index_at_length =
        instance.get_typed_func::<(), i32>(&store, "write_index_at_length")?;

    let err = read_index_at_length
        .call(&mut store, ())
        .expect_err("array_get should trap when index == length");
    assert_eq!(err.as_trap_code(), Some(TrapCode::UnreachableCodeReached));

    let err = read_negative_index
        .call(&mut store, ())
        .expect_err("array_get should treat negative Int32 indexes as out of bounds");
    assert_eq!(err.as_trap_code(), Some(TrapCode::UnreachableCodeReached));

    let err = write_index_at_length
        .call(&mut store, ())
        .expect_err("array_set should trap when index == length");
    assert_eq!(err.as_trap_code(), Some(TrapCode::UnreachableCodeReached));
    Ok(())
}

#[test]
fn specialized_int64_list_runtime_abi_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun update_int64_list: () -> Int64 = {
    with Arena {
        val base: List<Int64> = [10000000000, 20000000000];
        val appended = (base, 30000000000) list_append;
        val prepended = (40000000000, appended) list_prepend;
        val reversed = prepended |> list_reverse;
        (reversed, 0) list_get
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let update_int64_list = instance.get_typed_func::<(), i64>(&store, "update_int64_list")?;

    assert_eq!(update_int64_list.call(&mut store, ())?, 30000000000);
    Ok(())
}

#[test]
fn small_int64_literals_runtime_abi_executes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun update_small_int64_array: () -> Int64 = {
    with Arena {
        mut val counters: Array<Int64, 2> = [1, 2];
        val done = (counters, 1, 3) array_set;
        (counters, 1) array_get
    }
}

export fun update_small_int64_list: () -> Int64 = {
    with Arena {
        val base: List<Int64> = [1, 2];
        val appended = (base, 3) list_append;
        (appended, 2) list_get
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let update_small_int64_array =
        instance.get_typed_func::<(), i64>(&store, "update_small_int64_array")?;
    let update_small_int64_list =
        instance.get_typed_func::<(), i64>(&store, "update_small_int64_list")?;

    assert_eq!(update_small_int64_array.call(&mut store, ())?, 3);
    assert_eq!(update_small_int64_list.call(&mut store, ())?, 3);
    Ok(())
}

#[test]
fn exported_allocation_without_explicit_arena_uses_export_arena(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun unscoped_allocating_export: () -> Int32 = {
    val values = [1, 2];
    values |> list_count
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let unscoped_allocating_export =
        instance.get_typed_func::<(), i32>(&store, "unscoped_allocating_export")?;

    assert_eq!(unscoped_allocating_export.call(&mut store, ())?, 2);
    Ok(())
}

#[test]
fn exported_function_preserves_caller_arena_after_calling_exported_allocator(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun inner_allocating_export: () -> Int32 = {
    val inner_values = [1, 2];
    inner_values |> list_count
}

export fun outer_allocating_export: () -> Int32 = {
    val inner_count = () inner_allocating_export;
    val outer_values = [3, 4];
    inner_count + (outer_values |> list_count)
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let outer_allocating_export =
        instance.get_typed_func::<(), i32>(&store, "outer_allocating_export")?;

    assert_eq!(outer_allocating_export.call(&mut store, ())?, 4);
    Ok(())
}
