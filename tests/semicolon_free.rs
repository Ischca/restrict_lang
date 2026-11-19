use restrict_lang::{
    check_v001_release_surface, parse_program, TypeChecker, WasmCodeGen, WasmTargetProfile,
};
use wasmi::{Caller, Engine, Linker, Module, Store};

fn compile_to_wasm(source: &str) -> Vec<u8> {
    let (remaining, program) =
        parse_program(source).unwrap_or_else(|err| panic!("parse failed: {err:?}\n{source}"));
    assert!(
        remaining.trim().is_empty(),
        "unparsed trailing input: {remaining:?}"
    );

    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .unwrap_or_else(|err| panic!("type check failed: {err}\n{source}"));

    check_v001_release_surface(&program, &checker)
        .unwrap_or_else(|err| panic!("release surface failed: {err}\n{source}"));

    let mut codegen = WasmCodeGen::new().with_target_profile(WasmTargetProfile::WasiP1);
    let wat = codegen
        .generate(&program)
        .unwrap_or_else(|err| panic!("codegen failed: {err}\n{source}"));

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| panic!("WAT parse failed: {err}\n{wat}"));
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| panic!("Wasm validation failed: {err}\n{wat}"));
    wasm
}

fn call_export_i32(source: &str, export_name: &str) -> i32 {
    call_export::<i32>(source, export_name)
}

fn call_export<R: wasmi::WasmResults>(source: &str, export_name: &str) -> R {
    let wasm = compile_to_wasm(source);
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm).expect("valid Wasm module");
    let mut store = Store::new(&engine, ());
    let mut linker = Linker::new(&engine);

    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "fd_write",
            |_caller: Caller<'_, ()>, _fd: i32, _iovs: i32, _len: i32, _nwritten: i32| -> i32 { 0 },
        )
        .expect("define fd_write");
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "proc_exit",
            |_caller: Caller<'_, ()>, _code: i32| {},
        )
        .expect("define proc_exit");

    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .expect("instantiate module");
    let func = instance
        .get_typed_func::<(), R>(&store, export_name)
        .expect("get exported function");
    func.call(&mut store, ()).expect("execute function")
}

#[test]
fn semicolon_free_val_to_match_executes_in_wasm() {
    let source = r#"
enum Mode {
    Fast,
    Safe(Int32)
}

export fun run_val_to_match: () -> Int32 = {
    val base = 10
    val mode = (base + 5) Mode::Safe
    mode match {
        Mode::Fast => { 1 }
        Mode::Safe(bonus) => { bonus * 2 }
    }
}
"#;
    assert_eq!(call_export_i32(source, "run_val_to_match"), 30);
}

#[test]
fn semicolon_free_then_to_assignment_and_while_break_continue_execute_in_wasm() {
    let source = r#"
export fun run_loop: () -> Int32 = {
    mut val idx = 0
    mut val total = 0
    (idx < 10) while {
        idx = idx + 1
        (idx == 2) then {
            continue
        }
        (idx == 6) then {
            break
        }
        total = total + idx
    }
    total
}
"#;
    // Sums 1 + 3 + 4 + 5 = 13 (skips 2, breaks at 6)
    assert_eq!(call_export_i32(source, "run_loop"), 13);
}

#[test]
fn semicolon_free_bare_return_and_multiline_pipe_execute_in_wasm() {
    let source = r#"
fun inc: (n: Int32) -> Int32 = {
    n + 1
}

fun triple: (n: Int32) -> Int32 = {
    n * 3
}

fun side_effect_and_bare_return: (flag: Boolean) -> () = {
    (flag) then {
        return
        "unreachable" |> println
    }
    "reached" |> println
}

export fun run_pipe_and_return: () -> Int32 = {
    val unused = (true) side_effect_and_bare_return
    val seed = 4
    seed
        |> inc
        |> triple
}
"#;
    assert_eq!(call_export_i32(source, "run_pipe_and_return"), 15);
}

#[test]
fn semicolon_free_next_line_unary_minus_and_parens_execute_in_wasm() {
    let source = r#"
fun add_pair: (a: Int32, b: Int32) -> Int32 = {
    a + b
}

fun is_positive: (n: Int32) -> Boolean = {
    n > 0
}

export fun run_unary_and_parens: () -> Int32 = {
    val first = 7
    val ready = (first) is_positive
    (ready) then {
        val x = 100
        -first
    } else {
        (1, 2) add_pair
    }
}
"#;
    assert_eq!(call_export_i32(source, "run_unary_and_parens"), -7);
}

#[test]
fn same_line_semicolon_separation_and_optional_semicolons_remain_valid() {
    let source = r#"
fun inc: (n: Int32) -> Int32 = {
    n + 1
}

export fun run_mixed_semicolons: () -> Int32 = {
    val a = 10; val b = 20;
    mut val acc = 0;
    acc = a |> inc; acc = acc + b;
    acc
}
"#;
    assert_eq!(call_export_i32(source, "run_mixed_semicolons"), 31);
}

#[test]
fn pipe_continues_scoped_verbs_anonymous_scopes_and_argument_tuples_in_wasm() {
    let source = r#"
fun add: (a: Int32, b: Int32) -> Int32 = {
    a + b
}

export fun run_pipe_scopes: () -> Int32 = {
    // The second argument of `fold` appears mid-pipeline; the scope opens there
    // instead of wrapping the whole pipeline in parentheses from its first line.
    val shifted_total = [1, 2, 3]
        |> map { it + 1 }
        |> { (it, 0) |> fold { |sum, value| sum + value } }
    val kept = [1, 2, 3]
        |> map { it * 2 } filter { it > 2 }
        |> list_count
    val seeded_total = ([1, 2, 3], 10)
        |> fold { |sum, value| sum + value }
    val combined = shifted_total
        |> { |total| (total, kept) |> add }
    (combined, seeded_total)
        |> add
}
"#;
    // shifted_total = 2 + 3 + 4 = 9, kept = 2, seeded_total = 16, combined = 11
    assert_eq!(call_export_i32(source, "run_pipe_scopes"), 27);
}

#[test]
fn pipe_scope_forms_execute_like_their_same_line_spellings() {
    let piped = r#"
export fun run: () -> Int32 = {
    [3, 4, 5]
        |> filter { it > 3 }
        |> { (it, 100) |> fold { |sum, value| sum + value } }
}
"#;
    let same_line = r#"
export fun run: () -> Int32 = {
    [3, 4, 5] filter { it > 3 } { |kept| (kept, 100) fold { |sum, value| sum + value } }
}
"#;
    assert_eq!(call_export_i32(piped, "run"), 109);
    assert_eq!(call_export_i32(same_line, "run"), 109);
}

#[test]
fn fallback_operator_takes_some_and_none_paths_in_wasm() {
    let source = r#"
fun first_even: (a: Int32, b: Int32) -> Option<Int32> = {
    (a % 2 == 0) then {
        a Option::Some
    } else {
        (b % 2 == 0) then {
            b Option::Some
        } else {
            () Option::None
        }
    }
}

export fun run_fallback: () -> Int32 = {
    val kept = (3, 4) first_even ?? 100
    val used = (3, 5) first_even ?? 7
    kept * 10 + used
}
"#;
    assert_eq!(call_export_i32(source, "run_fallback"), 47);
}

#[test]
fn fallback_operator_matches_the_explicit_option_match_in_wasm() {
    let with_fallback = r#"
fun find: (n: Int32) -> Option<Int32> = {
    (n > 2) then { (n * 3) Option::Some } else { () Option::None }
}

export fun run: () -> Int32 = {
    val a = 5 find ?? -1
    val b = 1 find ?? -1
    a * 100 + b
}
"#;
    let explicit = r#"
fun find: (n: Int32) -> Option<Int32> = {
    (n > 2) then { (n * 3) Option::Some } else { () Option::None }
}

export fun run: () -> Int32 = {
    val a = 5 find match { Some(value) => { value } None => { -1 } }
    val b = 1 find match { Some(value) => { value } None => { -1 } }
    a * 100 + b
}
"#;
    assert_eq!(call_export_i32(explicit, "run"), 1499);
    assert_eq!(call_export_i32(with_fallback, "run"), 1499);
}

#[test]
fn fallback_operator_is_short_circuit_in_wasm() {
    // A `Some` left side must not evaluate its fallback: the fallback here
    // returns from the function, so it would change the observed result.
    let source = r#"
fun lookup: (n: Int32) -> Option<Int32> = {
    (n > 0) then { n Option::Some } else { () Option::None }
}

fun guarded: (n: Int32) -> Int32 = {
    val value = n lookup ?? return -1
    value + 100
}

export fun run_short_circuit: () -> Int32 = {
    val present = 5 guarded
    val missing = 0 guarded
    present * 1000 + (missing + 1)
}
"#;
    // present = 105, missing = -1
    assert_eq!(call_export_i32(source, "run_short_circuit"), 105_000);
}

#[test]
fn fallback_operator_diverges_with_break_and_continue_in_wasm() {
    let source = r#"
fun pick: (n: Int32) -> Option<Int32> = {
    (n == 2) then {
        () Option::None
    } else {
        (n == 5) then { () Option::None } else { (n * 10) Option::Some }
    }
}

export fun run_loop_controls: () -> Int32 = {
    mut val idx = 0
    mut val total = 0
    (idx < 10) while {
        idx = idx + 1
        // idx 2 skips, idx 5 stops; the others add idx * 10
        val picked = (idx pick) ?? ((idx == 2) then { continue } else { break })
        total = total + picked
    }
    total
}
"#;
    // 10 added, 2 skipped, 30 + 40 added, 5 breaks: 80
    assert_eq!(call_export_i32(source, "run_loop_controls"), 80);
}

#[test]
fn fallback_operator_accepts_break_and_continue_directly_in_wasm() {
    let source = r#"
fun pick: (n: Int32) -> Option<Int32> = {
    (n == 2) then { () Option::None } else { n Option::Some }
}

export fun run_continue: () -> Int32 = {
    mut val idx = 0
    mut val total = 0
    (idx < 4) while {
        idx = idx + 1
        val picked = idx pick ?? continue
        total = total + picked
    }
    total
}

export fun run_break: () -> Int32 = {
    mut val idx = 0
    mut val total = 0
    (idx < 4) while {
        idx = idx + 1
        val picked = idx pick ?? break
        total = total + picked
    }
    total
}
"#;
    // continue skips 2: 1 + 3 + 4; break stops at 2: 1
    assert_eq!(call_export_i32(source, "run_continue"), 8);
    assert_eq!(call_export_i32(source, "run_break"), 1);
}

#[test]
fn fallback_operator_handles_string_int64_and_float64_payloads_in_wasm() {
    let strings = r#"
fun maybe_name: (present: Boolean) -> Option<String> = {
    present then { "alice" Option::Some } else { () Option::None }
}

export fun run_strings: () -> Int32 = {
    val named = true maybe_name ?? "nobody"
    val anonymous = false maybe_name ?? "nobody"
    (named string_length) * 100 + (anonymous string_length)
}
"#;
    assert_eq!(call_export_i32(strings, "run_strings"), 506);

    let int64 = r#"
export fun run_int64: () -> Int64 = {
    val parsed = "40" string_parse_int64 ?? 0
    val missing = "x" string_parse_int64 ?? 2
    parsed + missing
}
"#;
    assert_eq!(call_export::<i64>(int64, "run_int64"), 42);

    let float64 = r#"
export fun run_float64: () -> Float64 = {
    val parsed = "1.5" string_parse_float ?? 0.0
    val missing = "x" string_parse_float ?? 0.25
    parsed + missing
}
"#;
    assert_eq!(call_export::<f64>(float64, "run_float64"), 1.75);
}

#[test]
fn fallback_operator_chains_to_the_right_and_continues_into_pipes_in_wasm() {
    let source = r#"
fun parse: (text: String) -> Option<Int32> = {
    text string_parse_int
}

fun double: (n: Int32) -> Int32 = {
    n * 2
}

export fun run_chain: () -> Int32 = {
    // The first two sides are absent, so the final constant is used.
    val last = ("x" parse) ?? ("y" parse) ?? 9
    // The middle side is present.
    val middle = ("x" parse) ?? ("21" parse) ?? 9
    // `??` is a pipe stage: the pipe after it applies to the whole fallback result.
    val piped = "x" parse ?? 4 |> double
    val kept = "5" parse ?? 4 |> double
    last + middle + piped + kept
}
"#;
    // 9 + 21 + 8 + 10
    assert_eq!(call_export_i32(source, "run_chain"), 48);
}

#[test]
fn fallback_operator_continues_a_pipeline_across_lines_in_wasm() {
    let source = r#"
fun clamp_port: (port: Int32) -> Int32 = {
    (port > 1000) then { 1000 } else { port }
}

fun port_for: (raw: String) -> Int32 = {
    raw
    |> string_trim
    |> string_parse_int
    ?? 8080
    |> clamp_port
}

export fun run_port: () -> Int32 = {
    val parsed = "  300 " port_for
    val missing = "none" port_for
    parsed + missing
}
"#;
    // 300 stays; the 8080 default is clamped to 1000
    assert_eq!(call_export_i32(source, "run_port"), 1300);
}

#[test]
fn fallback_operators_with_different_payload_types_share_a_function_in_wasm() {
    let source = r#"
fun opt_int: (present: Boolean) -> Option<Int32> = {
    present then { 7 Option::Some } else { () Option::None }
}

fun opt_text: (present: Boolean) -> Option<String> = {
    present then { "seven" Option::Some } else { () Option::None }
}

fun opt_flag: (present: Boolean) -> Option<Boolean> = {
    present then { true Option::Some } else { () Option::None }
}

export fun run_many_types: () -> Int32 = {
    val a = true opt_int ?? 1
    val b = true opt_text ?? "none"
    val c = false opt_int ?? 2
    val d = false opt_text ?? "four"
    val e = true opt_flag ?? false
    val f = false opt_flag ?? true
    val g = false opt_text ?? "x"
    val h = true opt_int ?? 0
    val flags = e then { 1 } else { 0 }
    val other = f then { 10 } else { 0 }
    a + (b string_length) + c + (d string_length) + flags + other + (g string_length) + h
}
"#;
    // 7 + 5 + 2 + 4 + 1 + 10 + 1 + 7
    assert_eq!(call_export_i32(source, "run_many_types"), 37);
}

#[test]
fn fallback_operator_consumes_its_left_and_the_used_fallback_arm_in_wasm() {
    // The fallback may use a binding: it is consumed only on the `None` path,
    // like a binding used in one arm of a `match`.
    let source = r#"
fun lookup: (n: Int32) -> Option<String> = {
    (n > 0) then { "found" Option::Some } else { () Option::None }
}

export fun run_affine: () -> Int32 = {
    val default_name = "default"
    val name = 1 lookup ?? default_name
    name string_length
}
"#;
    assert_eq!(call_export_i32(source, "run_affine"), 5);
}

#[test]
fn fallback_operator_binder_never_captures_a_user_variable_in_wasm() {
    // A user variable named like the expansion's old binder must keep its value.
    let source = r#"
fun pick: (opt: Option<Int32>) -> Int32 = {
    val fallback_payload = 5
    val chosen = opt ?? 0
    fallback_payload + chosen
}

export fun main: () -> Int32 = {
    (7 Option::Some) pick
}
"#;
    assert_eq!(call_export_i32(source, "main"), 12);
}
