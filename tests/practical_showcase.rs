use restrict_lang::formatter::format_source;
use restrict_lang::module::resolve_program_imports_with_base_dir;
use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};
use std::fs;
use std::process::Command;
use tempfile::TempDir;
use wasmi::{Caller, Engine, Instance, Linker, Memory, Module, Store};

#[derive(Default)]
struct CapturedIo {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
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

fn instantiate_wat(label: &str, wat: &str) -> (Store<CapturedIo>, Instance) {
    let wasm = wat::parse_str(wat)
        .unwrap_or_else(|err| panic!("{label} generated invalid WAT: {err}\n\n{wat}"));
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| panic!("{label} generated invalid Wasm binary: {err}\n\n{wat}"));

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm[..])
        .unwrap_or_else(|err| panic!("{label} failed to load in wasmi: {err}"));
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
        .unwrap_or_else(|err| panic!("{label} failed to instantiate: {err}"));
    (store, instance)
}

#[test]
fn practical_log_analyzer_formats_compiles_and_executes_via_wasmi() {
    let compiler = env!("CARGO_BIN_EXE_restrict_lang");
    let example_path = "examples/practical_log_analyzer.rl";

    let fmt_check = Command::new(compiler)
        .args(["--fmt", "--check", example_path])
        .output()
        .expect("restrict_lang --fmt --check should run");
    assert!(
        fmt_check.status.success(),
        "{example_path} should pass --fmt --check\nstdout:{}\nstderr:{}",
        String::from_utf8_lossy(&fmt_check.stdout),
        String::from_utf8_lossy(&fmt_check.stderr)
    );

    let type_check = Command::new(compiler)
        .args(["--check", example_path])
        .output()
        .expect("restrict_lang --check should run");
    assert!(
        type_check.status.success(),
        "{example_path} should pass --check\nstdout:{}\nstderr:{}",
        String::from_utf8_lossy(&type_check.stdout),
        String::from_utf8_lossy(&type_check.stderr)
    );

    let temp = TempDir::new().unwrap();
    let wat_path = temp.path().join("practical_log_analyzer.wat");
    let compile = Command::new(compiler)
        .args(["--arena-bytes", "65536"])
        .arg(example_path)
        .arg(&wat_path)
        .output()
        .expect("restrict_lang compile should run");
    assert!(
        compile.status.success(),
        "{example_path} should compile\nstdout:{}\nstderr:{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr)
    );

    let wat = fs::read_to_string(&wat_path).expect("compiled WAT should be readable");
    let (mut store, instance) = instantiate_wat("practical_log_analyzer", &wat);

    let summary_fn = instance
        .get_typed_func::<(), i32>(&store, "analyze_logs_summary")
        .expect("analyze_logs_summary export should exist");
    let summary = summary_fn
        .call(&mut store, ())
        .expect("analyze_logs_summary should succeed");
    assert_eq!(summary, 4430);

    let start_fn = instance
        .get_typed_func::<(), ()>(&store, "_start")
        .expect("_start export should exist");
    start_fn
        .call(&mut store, ())
        .expect("_start should succeed");

    let stdout = String::from_utf8(store.into_data().stdout).expect("stdout should be valid utf8");
    assert_eq!(stdout, "level=ERROR summary=4430\n");
}

#[test]
fn formatter_preserves_tokens_comments_and_idempotency_across_new_syntax() {
    let raw = r#"import std.option.*

enum Flag {
    Ok,
    Err
}

Flag takes Display {
    fun display: (self: Flag) -> String = {
        self match {
            Flag::Ok => { "ok\0" }
            Flag::Err => { "err\x21" }
        }
    }
}

fun demo: (xs: List<Int32>) -> Int32 = {
    val nul = '\0';
    val bang = '\x21';
    mut val acc = (nul as Int32) + (bang as Int32);
    // Walk range with break and continue
    mut val i = 0;
    (i < 5) while {
        i = i + 1;
        (i == 2) then { continue };
        (i == 4) then { break };
        acc = acc + i
    };
    val picked = xs list_find { it > 10 };
    picked match {
        Some(v) => { return v + acc }
        None => { return -1 }
    }
}

fun unit_helper: (done: Boolean) -> () = {
    done then { return () };
    (() Flag::Ok) |> println;
    return
}
"#;

    let formatted_once = format_source(raw).expect("format_source should succeed on new syntax");
    let formatted_twice = format_source(&formatted_once)
        .expect("format_source should succeed on already-formatted code");
    assert_eq!(
        formatted_once, formatted_twice,
        "formatting new syntax constructs must be idempotent"
    );
    assert!(formatted_once.contains("Flag takes Display"));
    assert!(formatted_once.contains("\"ok\\0\""));
    assert!(formatted_once.contains("'\\x21'"));
    assert!(formatted_once.contains("return v + acc"));
    assert!(formatted_once.contains("return -1"));
    assert!(formatted_once.contains("return ()"));
    assert!(formatted_once.contains("{ it > 10 }"));
    assert!(formatted_once.contains("// Walk range with break and continue"));
}

#[test]
fn scoped_verb_clauses_compose_cleanly_with_std_combinators() {
    let source = r#"
import std.option.*
import std.result.*
import std.list.*

export fun run_scoped_std_combinators: () -> Int32 = {
    val opt_val = (10) Option::Some option_and_then { |v|
        (v > 5) then { (v * 2) Option::Some } else { () Option::None }
    } option_unwrap_or_else { || 0 };

    val res_ok: Result<Int32, Int32> = (7) Result::Ok;
    val mapped_step: Result<Int32, Int32> = res_ok result_map { |v| v + 3 };
    val chained_step = mapped_step result_and_then { |v|
        val ok: Result<Int32, Int32> = (v * 2) Result::Ok;
        ok
    };
    val mapped_res = chained_step result_unwrap_or_else { |err| err };

    val res_err: Result<Int32, Int32> = (4) Result::Err;
    val err_step: Result<Int32, Int32> = res_err result_map_err { |e| e + 1 };
    val mapped_err = err_step result_unwrap_or_else { |e| e * 10 };

    mut val xs = [3, 8, 15, 22];
    val any_big = xs list_any { it > 10 };
    val all_pos = xs list_all { it > 0 };
    val first_big = xs list_find { it > 10 } option_unwrap_or_else { || 0 };

    val flags = (any_big && all_pos) then { 1 } else { 0 };
    opt_val + mapped_res + mapped_err + first_big + flags
}
"#;

    let (remaining, program) = parse_program(source).expect("scoped std test should parse");
    assert!(remaining.trim().is_empty(), "remaining: {remaining:?}");
    let resolved = resolve_program_imports_with_base_dir(program, None)
        .expect("std imports should resolve without explicit base_dir");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("scoped std combinators should type check");
    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&resolved)
        .expect("scoped std combinators should compile");

    let (mut store, instance) = instantiate_wat("scoped_std_combinators", &wat);
    let func = instance
        .get_typed_func::<(), i32>(&store, "run_scoped_std_combinators")
        .expect("run_scoped_std_combinators export should exist");
    // opt_val = 20, mapped_res = 20, mapped_err = 50, first_big = 15, flags = 1 => 106
    assert_eq!(func.call(&mut store, ()).unwrap(), 106);
}
