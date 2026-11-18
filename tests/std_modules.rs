use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use restrict_lang::module::{
    resolve_program_imports_with_base_dir, resolve_program_imports_with_module_source_map,
    ModuleResolver,
};
use restrict_lang::{
    check_v001_release_surface, parse_program, Program, TypeChecker, WasmCodeGen,
    WasmOptimizationLevel, WasmTargetProfile,
};
use wasmi::{Caller, Engine, Instance, Linker, Memory, Module, Store};

#[derive(Default)]
struct CapturedIo {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn parse_complete(source: &str) -> Program {
    let (remaining, program) = parse_program(source)
        .unwrap_or_else(|err| panic!("source should parse: {err:?}\n{source}"));
    assert!(
        remaining.trim().is_empty(),
        "source should parse completely, remaining: {remaining:?}"
    );
    program
}

fn compile_with_std_to_wasm_profile(
    source: &str,
    base_dir: Option<&std::path::Path>,
    profile: WasmTargetProfile,
) -> Result<Vec<u8>, String> {
    compile_with_std_to_wasm_options(source, base_dir, profile, WasmOptimizationLevel::None)
}

fn compile_with_std_to_wasm_options(
    source: &str,
    base_dir: Option<&std::path::Path>,
    profile: WasmTargetProfile,
    opt_level: WasmOptimizationLevel,
) -> Result<Vec<u8>, String> {
    let program = parse_complete(source);
    let resolved = resolve_program_imports_with_base_dir(program, base_dir)
        .map_err(|err| format!("Module resolution error: {err}"))?;

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .map_err(|err| format!("Type error: {err}"))?;

    check_v001_release_surface(&resolved, &checker)
        .map_err(|err| format!("Release surface error: {err}"))?;

    let mut codegen = WasmCodeGen::new()
        .with_target_profile(profile)
        .with_optimization_level(opt_level);
    let wat = codegen
        .generate(&resolved)
        .map_err(|err| format!("Codegen error: {err}"))?;

    let wasm = wat::parse_str(&wat).map_err(|err| format!("WAT parse error: {err}\n\n{wat}"))?;
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .map_err(|err| format!("Wasm validation error: {err}\n\n{wat}"))?;
    Ok(wasm)
}

fn compile_with_std_to_wasm(source: &str) -> Vec<u8> {
    let outside_repo = std::env::temp_dir();
    compile_with_std_to_wasm_profile(source, Some(&outside_repo), WasmTargetProfile::WasiP1)
        .unwrap_or_else(|err| panic!("compilation failed: {err}\n\nsource:\n{source}"))
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

fn instantiate_wasm(wasm: &[u8]) -> (Store<CapturedIo>, Instance) {
    let engine = Engine::default();
    let module = Module::new(&engine, wasm).expect("wasmi should load validated wasm");
    let mut store = Store::new(&engine, CapturedIo::default());
    let mut linker = Linker::new(&engine);

    linker
        .func_wrap("wasi_snapshot_preview1", "fd_write", capture_fd_write)
        .expect("fd_write should wrap");
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "proc_exit",
            |_caller: Caller<'_, CapturedIo>, _code: i32| {},
        )
        .expect("proc_exit should wrap");

    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .expect("wasmi should instantiate module");
    (store, instance)
}

fn run_main_i32(source: &str) -> i32 {
    let wasm = compile_with_std_to_wasm(source);
    let (mut store, instance) = instantiate_wasm(&wasm);
    let main_fn = instance
        .get_typed_func::<(), i32>(&store, "main")
        .expect("main() -> Int32 should be exported");
    main_fn.call(&mut store, ()).expect("main should execute")
}

fn run_main_i64(source: &str) -> i64 {
    let wasm = compile_with_std_to_wasm(source);
    let (mut store, instance) = instantiate_wasm(&wasm);
    let main_fn = instance
        .get_typed_func::<(), i64>(&store, "main")
        .expect("main() -> Int64 should be exported");
    main_fn.call(&mut store, ()).expect("main should execute")
}

fn run_main_f64(source: &str) -> f64 {
    let wasm = compile_with_std_to_wasm(source);
    let (mut store, instance) = instantiate_wasm(&wasm);
    let main_fn = instance
        .get_typed_func::<(), f64>(&store, "main")
        .expect("main() -> Float64 should be exported");
    main_fn.call(&mut store, ()).expect("main should execute")
}

#[test]
fn std_math_module_executes_in_wasm() {
    let result = run_main_i32(
        r#"
import std.math.*

export fun main: () -> Int32 = {
    val c1 = (15, 0, 10) clamp;
    val c2 = (-5, 0, 10) clamp;
    val c3 = (4, 0, 10) clamp;
    val s1 = -9 |> signum;
    val s2 = 0 |> signum;
    val s3 = 7 |> signum;
    val g = (48, 18) gcd;
    val l = (12, 18) lcm;
    val parity = (4 |> is_even) && (7 |> is_odd);
    val bonus = parity then { 100 } else { 0 };
    c1 + c2 + c3 + s1 + s2 + s3 + g + l + bonus
}
"#,
    );
    // 10 + 0 + 4 + (-1) + 0 + 1 + 6 + 36 + 100 = 156
    assert_eq!(result, 156);
}

#[test]
fn std_math_i64_and_f64_execute_in_wasm() {
    let i64_result = run_main_i64(
        r#"
import std.math.{abs_i64, min_i64, max_i64, clamp_i64, signum_i64}

export fun main: () -> Int64 = {
    val a = -5_000_000_000 |> abs_i64;
    val b = (10_000_000_000, 3_000_000_000) min_i64;
    val c = (10_000_000_000, 3_000_000_000) max_i64;
    val d = (20_000_000_000, 0, 5_000_000_000) clamp_i64;
    val s = -42 |> signum_i64;
    a + b + c + d + (s as Int64)
}
"#,
    );
    // 5e9 + 3e9 + 10e9 + 5e9 - 1 = 22_999_999_999
    assert_eq!(i64_result, 22_999_999_999);

    let f64_result = run_main_f64(
        r#"
import std.math.{clamp_f, lerp_f, approx_eq_f}

export fun main: () -> Float64 = {
    val c = (1.5, 0.0, 1.0) clamp_f;
    val l = (10.0, 20.0, 0.25) lerp_f;
    val ok = (l, 12.5, 0.001) approx_eq_f;
    ok then { c + l } else { -1.0 }
}
"#,
    );
    assert!((f64_result - 13.5).abs() < 1e-9);
}

#[test]
fn std_prelude_module_executes_in_wasm() {
    let result = run_main_i32(
        r#"
import std.prelude.*

export fun main: () -> Int32 = {
    val id = 21 |> identity_val;
    val x1 = (true, false) bool_xor;
    val x2 = (true, true) bool_xor;
    val imp = (false, false) bool_implies;
    val c1 = (3, 7) cmp_int;
    val c2 = (7, 7) cmp_int;
    val c3 = (9, 7) cmp_int;
    val eq = (5, 5) eq_int;
    val ne = (5, 6) ne_int;
    id + (x1 |> bool_to_int) + (x2 |> bool_to_int) + (imp |> bool_to_int) + c1 + c2 + c3 + (eq |> bool_to_int) + (ne |> bool_to_int)
}
"#,
    );
    // 21 + 1 + 0 + 1 + (-1) + 0 + 1 + 1 + 1 = 25
    assert_eq!(result, 25);
}

#[test]
fn std_option_module_executes_in_wasm() {
    let result = run_main_i32(
        r#"
import std.option.*

fun none_int_err: () -> Option<Int32> = {
    () Option::None
}

export fun main: () -> Int32 = {
    val some_ten: Option<Int32> = (10) Option::Some;
    val none_int: Option<Int32> = () Option::None;

    val o1 = ((none_int, some_ten) option_or, 0) option_unwrap_or;
    val o2 = (((20) Option::Some, () Option::None) option_or, 0) option_unwrap_or;

    val none_for_else: Option<Int32> = () Option::None;
    val u1 = (none_for_else, || 7) option_unwrap_or_else;

    val mapper: Int32 -> Option<Int32> = |x| (x * 3) Option::Some;
    val mapped: Option<Int32> = ((4) Option::Some, mapper) option_and_then;
    val m1 = (mapped, 0) option_unwrap_or;

    val nested: Option<Option<Int32>> = ((8) Option::Some) Option::Some;
    val f1 = (nested |> option_flatten, 0) option_unwrap_or;

    val combine_add: (Int32, Int32) -> Int32 = |a, b| a + b;
    val zipped: Option<Int32> = ((3) Option::Some, (5) Option::Some, combine_add) option_zip_with;
    val z1 = (zipped, 0) option_unwrap_or;

    val as_res: Result<Int32, Int32> = (() none_int_err, 99) option_ok_or;
    val r1 = as_res match {
        Ok(v) => { v }
        Err(e) => { e }
    };

    o1 + o2 + u1 + m1 + f1 + z1 + r1
}
"#,
    );
    // 10 + 20 + 7 + 12 + 8 + 8 + 99 = 164
    assert_eq!(result, 164);
}

#[test]
fn std_result_module_executes_in_wasm() {
    let result = run_main_i32(
        r#"
import std.result.*

export fun main: () -> Int32 = {
    val ok1: Result<Int32, Int32> = (10) Result::Ok;
    val err1: Result<Int32, Int32> = (5) Result::Err;

    val flags = (ok1 |> result_is_ok) && (err1 |> result_is_err);
    val base = flags then { 1 } else { 0 };

    val ok2: Result<Int32, Int32> = (15) Result::Ok;
    val err2: Result<Int32, Int32> = (4) Result::Err;
    val u1 = (ok2, 0) result_unwrap_or;
    val u2 = (err2, |e| e * 3) result_unwrap_or_else;

    val ok3: Result<Int32, Int32> = (7) Result::Ok;
    val mapped: Result<Int32, Int32> = (ok3, |x| x + 5) result_map;
    val m1 = (mapped, 0) result_unwrap_or;

    val err3: Result<Int32, Int32> = (2) Result::Err;
    val mapped_err: Result<Int32, Int32> = (err3, |e| e + 10) result_map_err;
    val m2 = (mapped_err, |e| e) result_unwrap_or_else;

    val ok4: Result<Int32, Int32> = (6) Result::Ok;
    val next_fn: Int32 -> Result<Int32, Int32> = |x| (x * 4) Result::Ok;
    val chained: Result<Int32, Int32> = (ok4, next_fn) result_and_then;
    val c1 = (chained, 0) result_unwrap_or;

    val ok5: Result<Int32, Int32> = (9) Result::Ok;
    val err4: Result<Int32, Int32> = (11) Result::Err;
    val opt_ok = (ok5 |> result_ok, 0) option_unwrap_or;
    val opt_err = (err4 |> result_err, 0) option_unwrap_or;

    base + u1 + u2 + m1 + m2 + c1 + opt_ok + opt_err
}
"#,
    );
    // 1 + 15 + 12 + 12 + 12 + 24 + 9 + 11 = 96
    assert_eq!(result, 96);
}

#[test]
fn std_list_module_executes_in_wasm() {
    let result = run_main_i32(
        r#"
import std.list.*

export fun main: () -> Int32 = {
    mut val rng = (1, 6) list_range;
    val sum_all = rng |> list_sum;
    val prod_first3 = (rng, 3) list_take |> list_product;
    val dropped_sum = (rng, 3) list_drop |> list_sum;

    val has_four = ([1, 2, 3, 4], |x| x == 4) list_any;
    val all_pos = ([1, 2, 3, 4], |x| x > 0) list_all;
    val found = ([3, 8, 12], |x| x > 5) list_find;
    val found_val = (found, 0) option_unwrap_or;
    val not_found = ([1, 2, 3], |x| x > 10) list_find;
    val not_found_val = (not_found, 7) option_unwrap_or;

    val add_pair: (Int32, Int32) -> Int32 = |a, b| a + b;
    val zipped: List<Int32> = ([1, 2, 3], [10, 20], add_pair) list_zip_with;
    val zipped_sum = zipped |> list_sum;

    val enumerated = ([100, 200], 5) list_enumerate_from;
    val second_entry = (enumerated, 1) list_get;
    val enum_score = second_entry.index + second_entry.value;

    val flags = (has_four && all_pos) then { 1 } else { 0 };
    sum_all + prod_first3 + dropped_sum + found_val + not_found_val + zipped_sum + enum_score + flags
}
"#,
    );
    // 15 + 6 + 9 + 8 + 7 + 33 + 206 + 1 = 285
    assert_eq!(result, 285);
}

#[test]
fn std_string_and_io_modules_execute_in_wasm() {
    let wasm = compile_with_std_to_wasm(
        r#"
import std.string.*
import std.io.*

export fun main: () -> Int32 = {
    val empty_ok = "" |> string_is_empty;
    val non_empty_ok = "ok" |> string_non_empty;
    val rep = ("ab", 3) string_repeat;
    mut val padded = ("x", "0", 3) string_pad_left;
    mut val joined = (["a", "b", "c"], "-") string_join_simple;

    ("joined", joined) println_kv;
    ("padded", padded) eprintln_labeled;

    val ok = empty_ok && non_empty_ok && (rep == "ababab") && (padded == "000x") && (joined == "a-b-c");
    ok then { 42 } else { 0 }
}
"#,
    );

    let (mut store, instance) = instantiate_wasm(&wasm);
    let main_fn = instance
        .get_typed_func::<(), i32>(&store, "main")
        .expect("main() -> Int32 should be exported");
    let result = main_fn.call(&mut store, ()).expect("main should execute");
    assert_eq!(result, 42);

    let io = store.data();
    assert_eq!(String::from_utf8_lossy(&io.stdout), "joined: a-b-c\n");
    assert_eq!(String::from_utf8_lossy(&io.stderr), "padded: 000x\n");
}

#[test]
fn std_modules_deduplicate_across_diamond_and_split_imports_and_ignore_local_std_dir() {
    let temp_dir = std::env::temp_dir().join(format!(
        "restrict_std_diamond_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let fake_std_dir = temp_dir.join("std");
    fs::create_dir_all(&fake_std_dir).expect("temp std dir should be created");
    // Even if a project has a local ./std/math.rl on disk, compiler-embedded std.math wins.
    fs::write(
        fake_std_dir.join("math.rl"),
        "pub fun clamp: (x: Int32, lo: Int32, hi: Int32) -> Int32 = { 9999 }\n",
    )
    .expect("decoy file should be written");
    fs::write(
        temp_dir.join("helper.rl"),
        r#"
import std.math.{clamp, gcd}

export fun helper_score: (n: Int32) -> Int32 = {
    ((n, 0, 10) clamp) + ((12, 8) gcd)
}
"#,
    )
    .expect("helper module should be written");

    let source = r#"
import helper.{helper_score}
import std.math.{clamp}
import std.math.{lcm}
import std.lib.{std_version}

export fun main: () -> Int32 = {
    val v = () std_version;
    val v_ok = v == "0.1.0";
    val a = 25 |> helper_score;
    val b = (-3, 0, 10) clamp;
    val c = (6, 8) lcm;
    v_ok then { a + b + c } else { -1 }
}
"#;

    let wasm =
        compile_with_std_to_wasm_profile(source, Some(&temp_dir), WasmTargetProfile::WasmCore)
            .expect("diamond/split std imports should compile in wasm-core");
    let (mut store, instance) = instantiate_wasm(&wasm);
    let main_fn = instance
        .get_typed_func::<(), i32>(&store, "main")
        .expect("main should export");
    // helper_score(25) = 10 + 4 = 14, clamp(-3,0,10)=0, lcm(6,8)=24 => 38
    assert_eq!(main_fn.call(&mut store, ()).expect("main should run"), 38);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn std_module_diagnostics_reject_overrides_unknowns_and_planned_profiles() {
    let mut resolver = ModuleResolver::new();
    let temp_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("std");
    let root_err = resolver
        .add_package_root("std".to_string(), temp_dir)
        .expect_err("--module-root std=... must remain rejected");
    assert!(
        root_err.to_string().contains("reserved"),
        "unexpected error for std package root: {root_err}"
    );

    let mut virtual_sources = HashMap::new();
    virtual_sources.insert(
        "std.math".to_string(),
        "pub fun clamp: (a: Int32, b: Int32, c: Int32) -> Int32 = { 0 }".to_string(),
    );
    let root = parse_complete(
        r#"
import std.math.*
fun main: () -> Int32 = { 0 }
"#,
    );
    let virtual_err = resolve_program_imports_with_module_source_map(root, virtual_sources)
        .expect_err("virtual std override must be rejected");
    assert!(
        virtual_err.to_string().contains("reserved"),
        "unexpected error for virtual std override: {virtual_err}"
    );

    for (import_stmt, expected_fragment) in [
        ("import std.unknown.*", "Unknown standard library module"),
        ("import std.http.*", "planned capability profile"),
        ("import std.database.*", "planned capability profile"),
        ("import std.filesystem.*", "planned capability profile"),
        ("import std", "cannot be imported directly"),
    ] {
        let program = parse_complete(&format!("{import_stmt}\nfun main: () -> Int32 = {{ 0 }}\n"));
        let mut r = ModuleResolver::new();
        let err = r
            .resolve_program_imports(program)
            .expect_err(&format!("{import_stmt} should fail"));
        assert!(
            err.to_string().contains(expected_fragment),
            "{import_stmt} error should contain {expected_fragment:?}, got: {err}"
        );
    }
}

#[test]
fn std_edge_cases_and_release_mode_execute_in_wasm() {
    let i64_sum = run_main_i64(
        r#"
import std.list.{list_sum_i64}
import std.prelude.{cmp_int64}

export fun main: () -> Int64 = {
    val nums: List<Int64> = [1_000_000_000, 2_000_000_000, 3_000_000_000];
    val sum = nums |> list_sum_i64;
    val cmp = (sum, 6_000_000_000) cmp_int64;
    sum + (cmp as Int64)
}
"#,
    );
    assert_eq!(i64_sum, 6_000_000_000);

    let f64_sum = run_main_f64(
        r#"
import std.list.{list_sum_f}
import std.prelude.{cmp_float}

export fun main: () -> Float64 = {
    val nums: List<Float64> = [1.5, 2.25, 0.25];
    val sum = nums |> list_sum_f;
    val cmp = (sum, 3.0) cmp_float;
    sum + (cmp as Float64)
}
"#,
    );
    assert!((f64_sum - 5.0).abs() < 1e-9);

    let edge_source = r#"
import std.math
import std.option.{option_or, option_unwrap_or_else, option_and_then, option_ok_or, option_flatten, option_zip_with}
import std.result.{result_is_ok, result_is_err, result_unwrap_or, result_unwrap_or_else, result_map, result_map_err, result_and_then, result_ok, result_err}
import std.list.{list_any, list_all, list_find, list_take, list_drop, list_range, list_zip_with}
import std.string.{string_repeat, string_join_simple}

export fun main: () -> Int32 = {
    mut val empty_ints: List<Int32> = [];
    val empty_any = (empty_ints, |x| x > 0) list_any;
    val empty_all = (empty_ints, |x| x > 0) list_all;
    val empty_find = ((empty_ints, |x| x > 0) list_find, 11) option_unwrap_or;
    val empty_take = ([1, 2], 0) list_take |> list_length;
    val neg_drop = ([1, 2], -3) list_drop |> list_length;
    val empty_range = (5, 5) list_range |> list_length;
    val add_pair: (Int32, Int32) -> Int32 = |a, b| a + b;
    val empty_zip: List<Int32> = (empty_ints, [1, 2], add_pair) list_zip_with;
    val empty_zip_len = empty_zip |> list_length;

    val zero_rep = ("x", 0) string_repeat;
    val empty_parts: List<String> = [];
    val empty_join = (empty_parts, ",") string_join_simple;
    val single_join = (["solo"], ",") string_join_simple;
    val str_ok = (zero_rep == "") && (empty_join == "") && (single_join == "solo");

    val none_i: Option<Int32> = () Option::None;
    val opt_none_and = ((none_i, |x| (x + 1) Option::Some) option_and_then, 13) option_unwrap_or;
    val opt_some_else = ((17) Option::Some, || 99) option_unwrap_or_else;
    val opt_some_ok = ((19) Option::Some, 0) option_ok_or;
    val opt_ok_val = (opt_some_ok, 0) result_unwrap_or;
    val opt_flat_none: Option<Option<Int32>> = (() Option::None) Option::Some;
    val flat_val = (opt_flat_none |> option_flatten, 23) option_unwrap_or;
    val add_pair_opt: (Int32, Int32) -> Int32 = |a, b| a + b;
    val zip_none: Option<Int32> = ((1) Option::Some, () Option::None, add_pair_opt) option_zip_with;
    val zip_none_val = (zip_none, 29) option_unwrap_or;

    val err_res: Result<Int32, Int32> = (31) Result::Err;
    val err_unwrap = (err_res, 37) result_unwrap_or;
    val ok_res: Result<Int32, Int32> = (41) Result::Ok;
    val ok_else = (ok_res, |e| e) result_unwrap_or_else;
    val err_map_val = ((err_res, |x| x + 1) result_map, 43) result_unwrap_or;
    val ok_map_err: Result<Int32, Int32> = (ok_res |> result_is_ok) then { (47) Result::Ok } else { (0) Result::Err };
    val mapped_ok_err = (ok_map_err, |e| e + 1) result_map_err;
    val ok_map_err_val = (mapped_ok_err, 0) result_unwrap_or;
    val chain_fn: Int32 -> Result<Int32, Int32> = |x| (x + 1) Result::Ok;
    val err_chain: Result<Int32, Int32> = (err_res, chain_fn) result_and_then;
    val err_chain_val = (err_chain |> result_err, 0) option_unwrap_or;
    val err_ok_none = (err_res |> result_ok, 53) option_unwrap_or;
    val ok_err_none = (ok_res |> result_err, 59) option_unwrap_or;

    val bool_sum = ((empty_any |> not) && empty_all && str_ok && (err_res |> result_is_err)) then { 1 } else { 0 };
    val bare_import_clamp = (12, 0, 5) clamp;

    bool_sum
        + bare_import_clamp
        + empty_find
        + empty_take
        + neg_drop
        + empty_range
        + empty_zip_len
        + opt_none_and
        + opt_some_else
        + opt_ok_val
        + flat_val
        + zip_none_val
        + err_unwrap
        + ok_else
        + err_map_val
        + ok_map_err_val
        + err_chain_val
        + err_ok_none
        + ok_err_none
}
"#;

    let release_wasm = compile_with_std_to_wasm_options(
        edge_source,
        None,
        WasmTargetProfile::WasmCore,
        WasmOptimizationLevel::Release,
    )
    .expect("release-mode std edge cases should compile");
    let (mut store, instance) = instantiate_wasm(&release_wasm);
    let main_fn = instance
        .get_typed_func::<(), i32>(&store, "main")
        .expect("main should be exported");
    // 1 + 5 + 11 + 0 + 2 + 0 + 0 + 13 + 17 + 19 + 23 + 29 + 37 + 41 + 43 + 47 + 31 + 53 + 59 = 431
    assert_eq!(main_fn.call(&mut store, ()).expect("main should run"), 431);
}
