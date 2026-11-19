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
        .get_typed_func::<(), i32>(&store, export_name)
        .expect("get exported i32 function");
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
