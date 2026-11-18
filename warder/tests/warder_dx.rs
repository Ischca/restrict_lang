use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn write_basic_app(root: &Path, build_extra: &str) {
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(
        root.join("warder.toml"),
        format!(
            "manifest-version = 1\n\n[package]\nname = \"app\"\nversion = \"0.1.0\"\nentry = \"src/main.rl\"\nedition = \"2025\"\n\n[dependencies]\n\n[build]\ntarget = \"wasm32\"\noptimize = true\noutput = \"dist/\"\n{build_extra}"
        ),
    )
    .unwrap();
    fs::write(
        root.join("src/main.rl"),
        "pub fun answer: () -> Int32 = { 42 }\n\nfun main: () -> () = { () }\n",
    )
    .unwrap();
    fs::write(
        root.join("tests/main_test.rl"),
        "import project.main.{answer}\n\npub fun test_answer: () -> Boolean = { () answer == 42 }\n",
    )
    .unwrap();
}

fn compile_wat_emitter(temp: &TempDir) -> (PathBuf, PathBuf) {
    let source = temp.path().join("wat-emitter.rs");
    let compiler = temp
        .path()
        .join(format!("wat-emitter{}", std::env::consts::EXE_SUFFIX));
    let captured_args = temp.path().join("emitter-args.txt");
    fs::write(
        &source,
        r#"use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(capture) = std::env::var_os("WARDER_CAPTURE_ARGS") {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(capture)
            .unwrap();
        for arg in &args {
            writeln!(f, "{arg}").unwrap();
        }
        writeln!(f, "---").unwrap();
    }
    if let Some(fail_if_contains) = std::env::var_os("WARDER_FAIL_ON_ARG") {
        let needle = fail_if_contains.to_string_lossy();
        if args.iter().any(|arg| arg.contains(needle.as_ref())) {
            eprintln!("simulated failure for {needle}");
            std::process::exit(1);
        }
    }
    if args.iter().any(|arg| arg == "--check") {
        return;
    }
    let output = args.last().expect("output path required");
    let wat = std::env::var("WARDER_TEST_WAT").expect("WARDER_TEST_WAT required");
    std::fs::write(output, wat).unwrap();
}
"#,
    )
    .unwrap();
    let compiled = Command::new("rustc")
        .arg(&source)
        .arg("-o")
        .arg(&compiler)
        .output()
        .expect("rustc should compile helper");
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    (compiler, captured_args)
}

#[test]
fn warder_run_executes_in_process_with_wasmi_without_external_runtimes() {
    let temp = TempDir::new().unwrap();
    let app = temp.path().join("app");
    write_basic_app(&app, "");
    let (compiler, _) = compile_wat_emitter(&temp);

    let wat = r#"(module
        (import "wasi_snapshot_preview1" "fd_write"
            (func $fd_write (param i32 i32 i32 i32) (result i32)))
        (import "wasi_snapshot_preview1" "fd_read"
            (func $fd_read (param i32 i32 i32 i32) (result i32)))
        (import "wasi_snapshot_preview1" "args_sizes_get"
            (func $args_sizes_get (param i32 i32) (result i32)))
        (import "wasi_snapshot_preview1" "args_get"
            (func $args_get (param i32 i32) (result i32)))
        (import "wasi_snapshot_preview1" "environ_sizes_get"
            (func $environ_sizes_get (param i32 i32) (result i32)))
        (import "wasi_snapshot_preview1" "environ_get"
            (func $environ_get (param i32 i32) (result i32)))
        (import "wasi_snapshot_preview1" "proc_exit"
            (func $proc_exit (param i32)))
        (memory (export "memory") 1)
        (data (i32.const 4096) "err\0a")
        (func (export "_start")
            i32.const 0
            i32.const 4
            call $args_sizes_get
            drop
            i32.const 64
            i32.const 128
            call $args_get
            drop
            i32.const 8
            i32.const 12
            call $environ_sizes_get
            drop
            i32.const 256
            i32.const 512
            call $environ_get
            drop
            ;; Read stdin up to 8 bytes into 4120
            i32.const 4200
            i32.const 4120
            i32.store
            i32.const 4204
            i32.const 8
            i32.store
            i32.const 0
            i32.const 4200
            i32.const 1
            i32.const 4208
            call $fd_read
            drop
            ;; Write arg1 (len 6: "--flag"), env0 (len 7: "FOO=bar"), and stdin to stdout
            i32.const 4300
            i32.const 68
            i32.load
            i32.store
            i32.const 4304
            i32.const 6
            i32.store
            i32.const 4308
            i32.const 256
            i32.load
            i32.store
            i32.const 4312
            i32.const 7
            i32.store
            i32.const 4316
            i32.const 4120
            i32.store
            i32.const 4320
            i32.const 4208
            i32.load
            i32.store
            i32.const 1
            i32.const 4300
            i32.const 3
            i32.const 4340
            call $fd_write
            drop
            ;; Write "err\n" to stderr
            i32.const 4352
            i32.const 4096
            i32.store
            i32.const 4356
            i32.const 4
            i32.store
            i32.const 2
            i32.const 4352
            i32.const 1
            i32.const 4360
            call $fd_write
            drop
            i32.const 0
            call $proc_exit)
    )"#;

    let wat_file = temp.path().join("test_run.wat");
    fs::write(&wat_file, wat).unwrap();
    // Keep WAT out of env vars so env0 in `std::env::vars()` is deterministic:
    // put WAT in a file and patch compiler source or pass minimal env.
    let mut child = Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(&app)
        .env_clear()
        .env("FOO", "bar")
        .env("RESTRICT_LANG_BIN", &compiler)
        .env("WARDER_TEST_WAT", wat)
        .args(["run", "--", "--flag"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(b"!in").unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(
        output.status.success(),
        "proc_exit(0) must succeed; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("--flag"), "stdout: {stdout}");
    assert!(stdout.contains("!in"), "stdout: {stdout}");
    assert!(
        stdout.contains("FOO=bar") || stdout.contains("RESTRIC"),
        "stdout should contain forwarded env: {stdout}"
    );
    assert!(stderr.contains("err\n"), "stderr: {stderr}");
}

#[test]
fn warder_run_propagates_non_zero_proc_exit_and_supports_host_target() {
    let temp = TempDir::new().unwrap();
    let app = temp.path().join("app");
    write_basic_app(&app, "");
    let manifest_path = app.join("warder.toml");
    let updated = fs::read_to_string(&manifest_path)
        .unwrap()
        .replace("target = \"wasm32\"", "target = \"host\"");
    fs::write(&manifest_path, updated).unwrap();
    let (compiler, _) = compile_wat_emitter(&temp);

    let wat = r#"(module
        (import "wasi_snapshot_preview1" "proc_exit"
            (func $proc_exit (param i32)))
        (func (export "_start")
            i32.const 7
            call $proc_exit)
    )"#;

    let output = Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(&app)
        .env("PATH", "")
        .env("RESTRICT_LANG_BIN", &compiler)
        .env("WARDER_TEST_WAT", wat)
        .arg("run")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(7));
}

#[test]
fn warder_build_passes_project_module_root_and_arena_bytes() {
    let temp = TempDir::new().unwrap();
    let app = temp.path().join("app");
    write_basic_app(&app, "arena-bytes = 65536\n");
    let (compiler, captured_args) = compile_wat_emitter(&temp);

    let output = Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(&app)
        .env("RESTRICT_LANG_BIN", &compiler)
        .env("WARDER_CAPTURE_ARGS", &captured_args)
        .env("WARDER_TEST_WAT", "(module (func (export \"main\")))")
        .arg("build")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let args = fs::read_to_string(&captured_args)
        .unwrap()
        .replace('\\', "/");
    assert!(
        args.lines()
            .any(|line| line.starts_with("project=") && line.ends_with("/application/src")),
        "warder build must pass --module-root project=<src_dir>: {args}"
    );
    assert!(
        args.lines().any(|line| line == "--arena-bytes")
            && args.lines().any(|line| line == "65536"),
        "warder build must pass --arena-bytes 65536: {args}"
    );
}

#[test]
fn warder_component_build_honors_manifest_arena_bytes() {
    let temp = TempDir::new().unwrap();
    let app = temp.path().join("app");
    fs::create_dir_all(app.join("src")).unwrap();
    fs::write(
        app.join("warder.toml"),
        r#"manifest-version = 1

[package]
name = "app"
version = "0.1.0"
entry = "src/main.rl"
edition = "2025"

[dependencies]

[build]
target = "wasm32"
optimize = true
output = "dist/"
arena-bytes = 65536

[component]
schema = 1
package = "example:app@0.1.0"
world = "app"
abi = "canonical-v1-sync"

[[component.exports]]
interface = "api"
source = "increment"
name = "increment"
"#,
    )
    .unwrap();
    fs::write(
        app.join("src/helper.rl"),
        "pub fun step: () -> Int32 = { 1 }\n",
    )
    .unwrap();
    fs::write(
        app.join("src/main.rl"),
        "import project.helper.{step}\n\npub fun increment: (value: Int32) -> Int32 = { value + (() step) }\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(&app)
        .args(["build", "--component"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Component build with project.* import and arena-bytes should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(app.join("dist/app-0.1.0.component.wasm").is_file());
}

#[test]
fn warder_check_validates_sources_and_tests_without_creating_dist() {
    let temp = TempDir::new().unwrap();
    let app = temp.path().join("app");
    write_basic_app(&app, "arena-bytes = 16384\n");
    let (compiler, captured_args) = compile_wat_emitter(&temp);

    let ok_output = Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(&app)
        .env("RESTRICT_LANG_BIN", &compiler)
        .env("WARDER_CAPTURE_ARGS", &captured_args)
        .arg("check")
        .output()
        .unwrap();

    assert!(
        ok_output.status.success(),
        "{}",
        String::from_utf8_lossy(&ok_output.stderr)
    );
    assert!(!app.join("dist").exists());
    assert!(!app.join("warder.lock").exists());

    let args = fs::read_to_string(&captured_args)
        .unwrap()
        .replace('\\', "/");
    assert!(args.contains("--check"), "{args}");
    assert!(args.contains("--arena-bytes"), "{args}");
    assert!(args.contains("16384"), "{args}");
    assert!(
        args.lines()
            .any(|line| line.starts_with("project=") && line.ends_with("/app/src")),
        "{args}"
    );
    assert!(
        args.lines().any(|line| line.ends_with("/src/main.rl")),
        "{args}"
    );
    assert!(
        args.lines()
            .any(|line| line.ends_with("/tests/main_test.rl")),
        "{args}"
    );

    let fail_output = Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(&app)
        .env("RESTRICT_LANG_BIN", &compiler)
        .env("WARDER_FAIL_ON_ARG", "main_test.rl")
        .arg("check")
        .output()
        .unwrap();
    assert!(!fail_output.status.success());
    assert!(String::from_utf8_lossy(&fail_output.stderr).contains("Check failed"));
}

#[test]
fn warder_clean_removes_dist_and_staging_files_while_preserving_sources_and_lock() {
    let temp = TempDir::new().unwrap();
    let app = temp.path().join("app");
    write_basic_app(&app, "");
    fs::create_dir_all(app.join("dist")).unwrap();
    fs::write(app.join("dist/app-0.1.0.wasm"), b"wasm").unwrap();
    fs::create_dir_all(app.join(".restrict-cache")).unwrap();
    fs::write(app.join(".restrict-cache/entry"), b"cache").unwrap();
    fs::create_dir_all(app.join(".warder-build-leftover")).unwrap();
    fs::write(app.join(".warder-build-leftover/tmp"), b"tmp").unwrap();
    fs::write(app.join(".warder-build.lock"), b"").unwrap();
    fs::write(app.join(".warder-build-transaction.toml"), b"").unwrap();
    fs::write(app.join("warder.lock"), "version = 1\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(&app)
        .arg("clean")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!app.join("dist").exists());
    assert!(!app.join(".restrict-cache").exists());
    assert!(!app.join(".warder-build-leftover").exists());
    assert!(!app.join(".warder-build.lock").exists());
    assert!(!app.join(".warder-build-transaction.toml").exists());
    assert!(app.join("warder.toml").is_file());
    assert!(app.join("warder.lock").is_file());
    assert!(app.join("src/main.rl").is_file());
    assert!(app.join("tests/main_test.rl").is_file());

    let repeat = Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(&app)
        .arg("clean")
        .output()
        .unwrap();
    assert!(repeat.status.success());
    assert!(String::from_utf8_lossy(&repeat.stdout).contains("Nothing to clean"));
}

#[test]
fn warder_doctor_reports_no_warnings_on_clean_project_with_discovered_compiler() {
    let temp = TempDir::new().unwrap();
    let app = temp.path().join("app");
    write_basic_app(&app, "");
    let (compiler, _) = compile_wat_emitter(&temp);

    let output = Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(&app)
        .env("PATH", "")
        .env("RESTRICT_LANG_BIN", &compiler)
        .arg("doctor")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No issues found!"), "{stdout}");
    assert!(!stdout.contains("skipped"), "{stdout}");
    assert!(!stdout.contains("No WASM runtime found"), "{stdout}");
    assert!(!stdout.contains("Warnings"), "{stdout}");
}

#[test]
fn warder_fmt_formats_src_and_tests_and_supports_check_mode() {
    let temp = TempDir::new().unwrap();
    let app = temp.path().join("app");
    write_basic_app(&app, "");
    fs::write(
        app.join("src/main.rl"),
        "pub fun answer: () -> Int32 = {\n42\n}\n\nfun main: () -> () = {\n\"hi\"|>println\n}\n",
    )
    .unwrap();
    fs::write(
        app.join("tests/main_test.rl"),
        "import project.main.{ answer }\n\npub fun test_answer: () -> Boolean = {\n() answer==42\n}\n",
    )
    .unwrap();

    let check_before = Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(&app)
        .args(["fmt", "--check"])
        .output()
        .unwrap();
    assert!(!check_before.status.success());
    let check_stderr = String::from_utf8_lossy(&check_before.stderr);
    assert!(check_stderr.contains("Needs formatting"), "{check_stderr}");

    let fmt_run = Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(&app)
        .arg("fmt")
        .output()
        .unwrap();
    assert!(
        fmt_run.status.success(),
        "{}",
        String::from_utf8_lossy(&fmt_run.stderr)
    );
    assert_eq!(
        fs::read_to_string(app.join("src/main.rl")).unwrap(),
        "pub fun answer: () -> Int32 = {\n    42\n}\n\nfun main: () -> () = {\n    \"hi\" |> println\n}\n"
    );
    assert_eq!(
        fs::read_to_string(app.join("tests/main_test.rl")).unwrap(),
        "import project.main.{answer}\n\npub fun test_answer: () -> Boolean = {\n    () answer == 42\n}\n"
    );

    let check_after = Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(&app)
        .args(["fmt", "--check"])
        .output()
        .unwrap();
    assert!(
        check_after.status.success(),
        "{}",
        String::from_utf8_lossy(&check_after.stderr)
    );
}

#[test]
fn warder_practical_log_analyzer_end_to_end() {
    let warder_bin = Path::new(env!("CARGO_BIN_EXE_warder"));
    let restrict_bin =
        warder_bin.with_file_name(format!("restrict_lang{}", std::env::consts::EXE_SUFFIX));
    if !restrict_bin.is_file() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("warder should live under workspace root");
        let built = Command::new("cargo")
            .current_dir(workspace_root)
            .args(["build", "--offline", "--locked", "--bin", "restrict_lang"])
            .output()
            .expect("cargo build restrict_lang should run");
        assert!(
            built.status.success(),
            "failed to build restrict_lang binary: {}",
            String::from_utf8_lossy(&built.stderr)
        );
    }

    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("warder should live under workspace root");
    let showcase_source =
        fs::read_to_string(workspace_root.join("examples/practical_log_analyzer.rl"))
            .expect("examples/practical_log_analyzer.rl should be readable");

    let temp = TempDir::new().unwrap();
    let app = temp.path().join("log_analyzer_app");
    fs::create_dir_all(app.join("src")).unwrap();
    fs::create_dir_all(app.join("tests")).unwrap();
    fs::write(
        app.join("warder.toml"),
        "manifest-version = 1\n\n[package]\nname = \"log_analyzer_app\"\nversion = \"0.1.0\"\nentry = \"src/main.rl\"\nedition = \"2025\"\n\n[dependencies]\n\n[build]\ntarget = \"wasm32\"\noptimize = true\noutput = \"dist/\"\narena-bytes = 65536\n",
    )
    .unwrap();
    fs::write(app.join("src/main.rl"), &showcase_source).unwrap();
    fs::write(
        app.join("tests/analyzer_test.rl"),
        "import project.main.{analyze_logs_summary}\n\npub fun test_summary: () -> Boolean = {\n    () analyze_logs_summary == 4430\n}\n",
    )
    .unwrap();

    let fmt_check = Command::new(warder_bin)
        .current_dir(&app)
        .env("RESTRICT_LANG_BIN", &restrict_bin)
        .args(["fmt", "--check"])
        .output()
        .unwrap();
    assert!(
        fmt_check.status.success(),
        "warder fmt --check failed\nstdout:{}\nstderr:{}",
        String::from_utf8_lossy(&fmt_check.stdout),
        String::from_utf8_lossy(&fmt_check.stderr)
    );

    let check = Command::new(warder_bin)
        .current_dir(&app)
        .env("RESTRICT_LANG_BIN", &restrict_bin)
        .arg("check")
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "warder check failed\nstdout:{}\nstderr:{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );

    let test = Command::new(warder_bin)
        .current_dir(&app)
        .env("RESTRICT_LANG_BIN", &restrict_bin)
        .arg("test")
        .output()
        .unwrap();
    assert!(
        test.status.success(),
        "warder test failed\nstdout:{}\nstderr:{}",
        String::from_utf8_lossy(&test.stdout),
        String::from_utf8_lossy(&test.stderr)
    );

    let run = Command::new(warder_bin)
        .current_dir(&app)
        .env("RESTRICT_LANG_BIN", &restrict_bin)
        .arg("run")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "warder run failed\nstdout:{}\nstderr:{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let run_stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run_stdout.contains("level=ERROR summary=4430"),
        "unexpected warder run stdout: {run_stdout}"
    );
}
