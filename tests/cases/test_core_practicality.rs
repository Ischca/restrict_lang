use restrict_lang::{parse_program, TypeChecker, TypeError, WasmCodeGen};
use wasmi::{Caller, Engine, Instance, Linker, Memory, Module, Store};

#[derive(Default)]
struct CapturedIo {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn check_source(source: &str) -> Result<(), TypeError> {
    let (remaining, ast) = parse_program(source).expect("source should parse");
    assert!(
        remaining.trim().is_empty(),
        "unparsed input remaining: {remaining:?}"
    );
    let mut checker = TypeChecker::new();
    checker.check_program(&ast)
}

fn compile_to_wasm(source: &str) -> Result<(String, Vec<u8>), String> {
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

    let wasm = wat::parse_str(&wat).map_err(|e| format!("Invalid WAT: {e}\n\n{wat}"))?;
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .map_err(|e| format!("Invalid Wasm binary: {e}\n\n{wat}"))?;

    Ok((wat, wasm))
}

fn read_i32(memory: Memory, caller: &Caller<'_, CapturedIo>, offset: i32) -> Result<i32, i32> {
    let mut bytes = [0; 4];
    memory
        .read(caller, offset as usize, &mut bytes)
        .map_err(|_| 1)?;
    Ok(i32::from_le_bytes(bytes))
}

fn capture_fd_write(
    mut caller: Caller<'_, CapturedIo>,
    fd: i32,
    iovs: i32,
    iovs_len: i32,
    nwritten: i32,
) -> i32 {
    let Some(memory) = caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
    else {
        return 1;
    };

    let mut written = 0usize;
    let mut captured = Vec::new();
    for i in 0..iovs_len {
        let iov = iovs + (i * 8);
        let base = match read_i32(memory, &caller, iov) {
            Ok(base) => base,
            Err(errno) => return errno,
        };
        let len = match read_i32(memory, &caller, iov + 4) {
            Ok(len) => len,
            Err(errno) => return errno,
        };

        let mut bytes = vec![0; len as usize];
        if memory.read(&caller, base as usize, &mut bytes).is_err() {
            return 1;
        }
        written += bytes.len();
        captured.extend(bytes);
    }

    match fd {
        1 => caller.data_mut().stdout.extend(captured),
        2 => caller.data_mut().stderr.extend(captured),
        _ => return 8,
    }

    if nwritten != 0 {
        let bytes = (written as i32).to_le_bytes();
        if memory
            .write(&mut caller, nwritten as usize, &bytes)
            .is_err()
        {
            return 1;
        }
    }

    0
}

fn instantiate(label: &str, source: &str) -> (Store<CapturedIo>, Instance) {
    let (wat, wasm) =
        compile_to_wasm(source).unwrap_or_else(|err| panic!("{label} should compile: {err}"));
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm[..])
        .unwrap_or_else(|err| panic!("{label} could not load in wasmi: {err}\n\n{wat}"));
    let mut store = Store::new(&engine, CapturedIo::default());
    let mut linker = Linker::new(&engine);

    linker
        .func_wrap("wasi_snapshot_preview1", "fd_write", capture_fd_write)
        .expect("fd_write stub should register");
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "proc_exit",
            |_caller: Caller<'_, CapturedIo>, _code: i32| {},
        )
        .expect("proc_exit stub should register");

    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .unwrap_or_else(|err| panic!("{label} failed to instantiate: {err}\n\n{wat}"));
    (store, instance)
}

fn run_export_i32(label: &str, source: &str, export_name: &str) -> i32 {
    let (mut store, instance) = instantiate(label, source);
    let func = instance
        .get_typed_func::<(), i32>(&store, export_name)
        .unwrap_or_else(|err| panic!("{label} missing export {export_name}: {err}"));
    func.call(&mut store, ())
        .unwrap_or_else(|err| panic!("{label} trapped during {export_name}: {err}"))
}

fn run_export_stdout(label: &str, source: &str, export_name: &str) -> String {
    let (mut store, instance) = instantiate(label, source);
    let func = instance
        .get_typed_func::<(), ()>(&store, export_name)
        .unwrap_or_else(|err| panic!("{label} missing export {export_name}: {err}"));
    func.call(&mut store, ())
        .unwrap_or_else(|err| panic!("{label} trapped during {export_name}: {err}"));
    String::from_utf8(store.into_data().stdout).expect("stdout should be valid utf8")
}

#[test]
fn nested_match_patterns_execute_across_options_records_and_lists() {
    let source = r#"
record User {
    id: Int32,
    tags: List<String>
}

record Pair {
    a: Option<Int32>,
    b: Option<Int32>
}

fun classify_user: (entry: Option<User>, pair: Pair, matrix: List<List<Int32>>) -> Int32 = {
    val user_score = entry match {
        Some(User { id, tags: ["VIP" | _] }) => { id + 100 }
        Some(User { id, ..._ }) => { id }
        None => { 0 }
    };

    val pair_score = pair match {
        Pair { a: Some(x), b: Some(y) } => { x * 10 + y }
        Pair { a: Some(x), b: None } => { x }
        _ => { -1 }
    };

    val matrix_score = matrix match {
        [[first | second] | _] => { first + (second |> list_count) }
        [] => { 0 }
        _ => { -5 }
    };

    user_score + pair_score + matrix_score
}

export fun run_nested_match: () -> Int32 = {
    val vip = User { id: 7, tags: ["VIP", "core"] };
    val regular = User { id: 3, tags: ["guest"] };
    val s1 = ((vip) Option::Some, Pair { a: (4) Option::Some, b: (5) Option::Some }, [[10, 20, 30], [40]]) classify_user;
    val s2 = ((regular) Option::Some, Pair { a: (2) Option::Some, b: () Option::None }, []) classify_user;
    s1 * 100 + s2
}
"#;

    // s1 = (7 + 100) + (4 * 10 + 5) + (10 + 2) = 107 + 45 + 12 = 164
    // s2 = 3 + 2 + 0 = 5
    // total = 16405
    assert_eq!(
        run_export_i32("nested_match_patterns", source, "run_nested_match"),
        16405
    );
}

#[test]
fn nested_clone_expressions_preserve_outer_and_update_inner_fields() {
    let source = r#"
record Inner {
    x: Int32,
    y: Int32
}

record Outer {
    tag: Int32,
    inner: Inner,
    weight: Int32
}

export fun run_nested_clone: () -> Int32 = {
    mut val base = Outer {
        tag: 7,
        inner: Inner { x: 10, y: 20 },
        weight: 3
    };
    val updated = base.clone {
        inner: base.inner.clone { x: 99 },
        weight: 5
    };
    updated.tag * 1000 + updated.inner.x * 10 + updated.inner.y + updated.weight
}
"#;

    // 7 * 1000 + 99 * 10 + 20 + 5 = 7000 + 990 + 25 = 8015
    assert_eq!(
        run_export_i32("nested_clone", source, "run_nested_clone"),
        8015
    );
}

#[test]
fn closure_shadowing_keeps_outer_binding_intact_and_rejects_outer_mut_write() {
    let valid = r#"
export fun run_shadow: () -> Int32 = {
    val x = 10;
    val bump = |x: Int32| {
        val y = x + 1;
        y * 2
    };
    val inner = 20 |> bump;
    x + inner
}
"#;
    // bump(20) => (20 + 1) * 2 = 42; outer x = 10 => 52
    assert_eq!(run_export_i32("closure_shadowing", valid, "run_shadow"), 52);

    let bad_direct_mut_capture = r#"
fun bad: () -> Int32 = {
    mut val count = 0;
    val inc = |delta: Int32| {
        count = count + delta;
        count
    };
    1 |> inc
}
"#;
    assert!(
        check_source(bad_direct_mut_capture).is_err(),
        "mutating an outer mut val inside an annotated lambda must be rejected"
    );

    let bad_deferred_mut_capture = r#"
fun bad: () -> Int32 = {
    mut val count = 0;
    val inc = || {
        count = count + 1;
        count
    };
    () inc
}
"#;
    assert!(
        check_source(bad_deferred_mut_capture).is_err(),
        "mutating an outer mut val inside a deferred lambda must be rejected"
    );

    let bad_generic_mut_capture = r#"
fun bad: () -> Int32 = {
    mut val total = 0;
    val mapped = ([0..3], |x| {
        total = total + x;
        x + 1
    }) map;
    mapped |> list_count
}
"#;
    assert!(
        check_source(bad_generic_mut_capture).is_err(),
        "mutating an outer mut val inside a generic callback lambda must be rejected"
    );
}

#[test]
fn enum_takes_display_and_payload_free_equality_and_copy_execute() {
    let source = r#"
enum Status {
    Ready
    Busy(Int32)
}

Status takes Display {
    fun display: (self: Status) -> String = {
        self match {
            Status::Ready => { "ready" }
            Status::Busy(code) => {
                "busy:" + (code display)
            }
        }
    }
}

enum Mode {
    Fast
    Slow
}

export fun emit_statuses: () -> () = {
    () Status::Ready |> println;
    42 |> Status::Busy |> println
}

export fun check_mode_equality: () -> Int32 = {
    val m = () Mode::Fast;
    val first = m == () Mode::Fast;
    val second = m != () Mode::Slow;
    val copy_a = m;
    val copy_b = m;
    val third = copy_a == copy_b;
    (first && second && third) then { 1 } else { 0 }
}
"#;

    let stdout = run_export_stdout("enum_takes_display", source, "emit_statuses");
    assert_eq!(stdout, "ready\nbusy:42\n");
    assert_eq!(
        run_export_i32("enum_mode_eq", source, "check_mode_equality"),
        1
    );

    let unknown_form_enum_takes = r#"
enum Tag { Alpha }
Tag takes NonExistentForm {
    fun render: (self: Tag) -> String = { "alpha" }
}
"#;
    assert!(
        check_source(unknown_form_enum_takes).is_err(),
        "adopting an undefined form on an enum must be rejected"
    );

    let payload_enum_eq = r#"
enum Item { One, Two(Int32) }
fun eq_check: () -> Boolean = {
    val a = 1 |> Item::Two;
    val b = 1 |> Item::Two;
    a == b
}
"#;
    assert!(
        check_source(payload_enum_eq).is_err(),
        "payload-carrying enum equality must remain rejected"
    );
}

#[test]
fn chained_copy_field_access_preserves_outer_and_clone_consumes_val_base() {
    let source = r#"
record Inner {
    x: Int32,
    y: Int32
}

record Outer {
    inner: Inner,
    label: String
}

export fun run_chained_copy_and_clone: () -> Int32 = {
    val first = Outer {
        inner: Inner { x: 4, y: 9 },
        label: "alpha"
    };
    val sum = first.inner.x + first.inner.y + first.inner.x;
    val cloned = first.clone {
        inner: Inner { x: 10, y: 20 }
    };
    val inner_sum = cloned.inner.x + cloned.inner.y;
    val label_bonus = (cloned.label == "alpha") then { 5 } else { 0 };
    sum * 100 + inner_sum + label_bonus
}
"#;
    // sum = 4 + 9 + 4 = 17; cloned = 10 + 20 + 5 = 35 => 1735
    assert_eq!(
        run_export_i32(
            "chained_copy_and_clone",
            source,
            "run_chained_copy_and_clone"
        ),
        1735
    );

    let noncopy_chained = r#"
record Inner { label: String }
record Outer { inner: Inner }
fun bad: (o: Outer) -> Boolean = {
    val a = o.inner.label;
    val b = o.inner.label;
    a == b
}
"#;
    assert!(
        matches!(check_source(noncopy_chained), Err(TypeError::AffineViolation(ref name)) if name == "o"),
        "chained access ending in non-Copy field must still consume outer"
    );

    let consumed_val_clone = r#"
record Entry { name: String, count: Int32 }
fun bad: () -> Int32 = {
    val base = Entry { name: "x", count: 1 };
    val next = base.clone { count: 2 };
    val reused = base.count;
    next.count + reused
}
"#;
    assert!(
        matches!(
            check_source(consumed_val_clone),
            Err(TypeError::AffineViolation(_))
        ),
        "using a val base after .clone must be rejected"
    );

    let mut_val_unconsumed_clone = r#"
record Entry { name: String, count: Int32 }
fun bad: () -> Int32 = {
    mut val base = Entry { name: "x", count: 1 };
    val next = base.clone { count: 2 };
    base = Entry { name: "y", count: 3 };
    next.count + base.count
}
"#;
    assert!(
        matches!(
            check_source(mut_val_unconsumed_clone),
            Err(TypeError::UnsupportedFeature(ref msg))
                if msg.contains("implicitly copy non-copy field")
        ),
        "cloning an unconsumed mut val base while preserving non-Copy field must be rejected"
    );
}

#[test]
fn control_flow_range_char_and_escapes_execute_end_to_end() {
    let source = r#"
fun find_sum_with_while: (limit: Int32) -> Int32 = {
    mut val i = 0;
    mut val acc = 0;
    (i < 10) while {
        i = i + 1;
        (i == 2) then {
            continue
        };
        (i > limit) then {
            break
        };
        with Arena {
            val tag = "step";
            (tag == "step") then {
                acc = acc + i
            };
            (acc > 100) then {
                break
            }
        }
    };
    acc
}

fun early_return_test: (flag: Boolean) -> Int32 = {
    with Arena {
        val guard = "arena";
        flag then {
            (guard == "arena") then {
                return 55
            }
        }
    };
    9
}

fun early_unit_return: (skip: Boolean) -> () = {
    skip then {
        return ()
    };
    "ran" |> println
}

export fun run_control_flow_and_ranges: () -> Int32 = {
    val while_res = 5 |> find_sum_with_while;
    val ret_t = true |> early_return_test;
    val ret_f = false |> early_return_test;

    val mapped = ([0..5], |x| x * 2) map |> list_count;
    val filtered = ([-3..3], |x| x >= 0) filter |> list_count;
    val folded = ([1..5], 0, |acc, x| acc + x) fold;
    val single = ([4..5], 0, |acc, x| acc + x) fold;
    val empty = ([5..2], 7, |acc, x| acc + x) fold;

    val c_a = '\x41';
    val c_nul = '\0';
    val c_hex_nul = '\x00';
    val char_cmp = (('a' < 'b') && ('b' <= 'b') && ('z' > 'a') && ('z' >= 'z')) then { 1 } else { 0 };
    val char_vals = (c_a as Int32) + (c_nul as Int32) + (c_hex_nul as Int32) + char_cmp;

    // while_res: 1 + 3 + 4 + 5 = 13
    // ret_t: 55, ret_f: 9
    // mapped: 5, filtered: 3 (0,1,2), folded: 1+2+3+4 = 10, single: 4, empty: 7
    // char_vals: 65 + 0 + 0 + 1 = 66
    while_res + ret_t + ret_f + mapped + filtered + folded + single + empty + char_vals
}

export fun run_unit_return: () -> () = {
    true |> early_unit_return;
    false |> early_unit_return
}
"#;

    // 13 + 55 + 9 + 5 + 3 + 10 + 4 + 7 + 66 = 172
    assert_eq!(
        run_export_i32(
            "control_flow_and_ranges",
            source,
            "run_control_flow_and_ranges"
        ),
        172
    );
    assert_eq!(
        run_export_stdout("early_unit_return", source, "run_unit_return"),
        "ran\n"
    );
}

#[test]
fn anonymous_scope_results_keep_their_type_when_bound_and_reused() {
    // The scope body is typed before its code is generated, so names bound
    // inside it (binders and earlier `val`s) must take part in that inference.
    let source = r#"
fun add: (a: Int32, b: Int32) -> Int32 = {
    a + b
}

fun pick_first: <T>(left: T, right: T) -> T = {
    left
}

export fun fold_seed_from_scope_local: () -> Int32 = {
    val total = [1, 2, 3] map { it + 1 } { |shifted|
        val seed = 0
        (shifted, seed) fold { |sum, value| sum + value }
    }
    val pair = (total, 1) add
    pair
}

export fun map_result_feeds_a_later_fold: () -> Int32 = {
    val shifted = [1, 2, 3] { |items|
        val offset = 10
        items map { |item| item + offset }
    }
    val total = (shifted, 0) fold { |sum, value| sum + value }
    (total, 0) add
}

export fun filter_result_feeds_a_later_fold: () -> Int32 = {
    val kept = [1, 2, 3] { |items|
        val floor = 1
        items filter { |item| item > floor }
    }
    val total = (kept, 0) fold { |sum, value| sum + value }
    (total, 0) add
}

export fun nested_scope_result_instantiates_a_generic: () -> Int32 = {
    val nested = 4 { |base|
        val bumped = base { |inner| inner + 1 }
        (bumped, base) pick_first
    }
    (nested, 0) add
}
"#;

    let label = "anonymous scope result inference";
    // 2 + 3 + 4 = 9, then + 1
    assert_eq!(
        run_export_i32(label, source, "fold_seed_from_scope_local"),
        10
    );
    // 11 + 12 + 13
    assert_eq!(
        run_export_i32(label, source, "map_result_feeds_a_later_fold"),
        36
    );
    // 2 + 3
    assert_eq!(
        run_export_i32(label, source, "filter_result_feeds_a_later_fold"),
        5
    );
    assert_eq!(
        run_export_i32(label, source, "nested_scope_result_instantiates_a_generic"),
        5
    );
}

fn fallback_error(body: &str) -> TypeError {
    let source = format!(
        r#"
fun maybe_text: (n: Int32) -> Option<String> = {{
    (n > 0) then {{ "text" Option::Some }} else {{ () Option::None }}
}}

fun maybe_int: (n: Int32) -> Option<Int32> = {{
    (n > 0) then {{ n Option::Some }} else {{ () Option::None }}
}}

fun checked: (n: Int32) -> Result<Int32, String> = {{
    (n > 0) then {{ n Result::Ok }} else {{ "bad" Result::Err }}
}}

export fun probe: () -> Int32 = {{
{body}
}}
"#
    );
    check_source(&source).expect_err("the fallback program must be rejected")
}

#[test]
fn fallback_rejects_a_left_side_that_is_not_an_option() {
    let err = fallback_error("    val n = 5\n    n ?? 1");
    assert!(
        matches!(&err, TypeError::FallbackLeftNotOption { found, is_result: false } if found == "Int32"),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "`??` requires an `Option` value on its left, found `Int32`"
    );
}

#[test]
fn fallback_rejects_result_and_points_at_result_ok() {
    let err = fallback_error("    1 checked ?? 0");
    assert!(
        matches!(
            &err,
            TypeError::FallbackLeftNotOption {
                is_result: true,
                ..
            }
        ),
        "{err:?}"
    );
    let message = err.to_string();
    assert!(
        message.contains("`??` requires an `Option` value on its left"),
        "{message}"
    );
    assert!(
        message.contains("value |> result_ok ?? fallback"),
        "{message}"
    );
}

#[test]
fn fallback_rejects_a_fallback_of_the_wrong_type() {
    let err = fallback_error("    1 maybe_int ?? \"zero\"");
    assert_eq!(
        err.to_string(),
        "the fallback of `??` must be `Int32`, the payload of the left `Option<Int32>`, found `String`"
    );

    // An annotation on the result does not change which side is blamed.
    let err = fallback_error("    val n: Int32 = 1 maybe_int ?? \"zero\"\n    n");
    assert!(
        matches!(&err, TypeError::FallbackTypeMismatch { payload, found } if payload == "Int32" && found == "String"),
        "{err:?}"
    );
}

#[test]
fn fallback_blames_the_annotation_when_the_payload_does_not_match_it() {
    let err = fallback_error("    val chosen: Int32 = 1 maybe_text ?? 1\n    chosen");
    assert_eq!(
        err.to_string(),
        "`??` yields `String`, the payload of the left `Option<String>`, but `Int32` is expected here"
    );
    // Same context, but a fallback that is itself of the wrong type.
    let err = fallback_error("    val chosen: Int32 = 1 maybe_text ?? false\n    chosen");
    assert!(
        matches!(err, TypeError::FallbackResultMismatch { .. }),
        "{err:?}"
    );
}

#[test]
fn fallback_reports_errors_inside_the_fallback_as_they_are() {
    let err = fallback_error("    1 maybe_int ?? (1 + \"a\")");
    assert!(
        !matches!(err, TypeError::FallbackTypeMismatch { .. }),
        "an error inside the fallback expression is not a payload mismatch: {err:?}"
    );
}

#[test]
fn fallback_binding_used_again_after_the_fallback_is_an_affine_violation() {
    let err = fallback_error(
        "    val default_name = \"default\"\n    val name = 1 maybe_text ?? default_name\n    val again = default_name\n    0",
    );
    assert!(
        matches!(&err, TypeError::AffineViolation(name) if name == "default_name"),
        "{err:?}"
    );
}

#[test]
fn fallback_binding_used_on_both_sides_is_an_affine_violation() {
    let err = fallback_error(
        "    val shared = \"shared\"\n    val name = (shared Option::Some) ?? shared\n    0",
    );
    assert!(
        matches!(&err, TypeError::AffineViolation(name) if name == "shared"),
        "{err:?}"
    );
}

#[test]
fn fallback_accepts_a_binding_used_only_on_the_fallback_side() {
    let source = r#"
fun maybe_text: (n: Int32) -> Option<String> = {
    (n > 0) then { "text" Option::Some } else { () Option::None }
}

export fun probe: () -> Int32 = {
    val default_name = "default"
    val name = 1 maybe_text ?? default_name
    name string_length
}
"#;
    check_source(source).expect("a single use in the fallback is fine");
}
