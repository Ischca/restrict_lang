use super::{find_project_root, load_manifest, print_error, print_info, print_success};
use crate::dependencies::{
    resolve_local_dependencies, ResolvedLocalDependency, PROJECT_SOURCE_NAMESPACE,
};
use anyhow::{bail, Context, Result};
use restrict_lang::ast::{FunDecl, TopDecl, Type};
use restrict_lang::diagnostics::format_parse_error;
use restrict_lang::parser::parse_program;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;
use wasmi::{Caller, Engine, Linker, Memory, Module, Store};

#[derive(Debug, Clone, PartialEq, Eq)]
struct TestFunction {
    name: String,
}

#[derive(Debug)]
struct TestOutcome {
    name: String,
    failure: Option<String>,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Default)]
struct TestIo {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

pub async fn test_project(filter: Option<String>) -> Result<()> {
    let root = find_project_root()?;
    let root = std::fs::canonicalize(&root)
        .with_context(|| format!("Failed to canonicalize project root {}", root.display()))?;
    root.to_str()
        .with_context(|| format!("Project root is not valid UTF-8: {:?}", root))?;
    let manifest = load_manifest()?;
    let (resolved_dependencies, _) = resolve_local_dependencies(&root, &manifest)?;
    let entry_path = root.join(&manifest.package.entry);
    let project_source_root = entry_path
        .parent()
        .context("package.entry must have a source directory")?;
    let project_source_root = std::fs::canonicalize(project_source_root).with_context(|| {
        format!(
            "Failed to canonicalize project source root {}",
            project_source_root.display()
        )
    })?;
    let tests_dir = root.join("tests");

    if !tests_dir.exists() {
        print_info("No tests directory found");
        return Ok(());
    }

    let mut test_files = Vec::new();
    for entry in WalkDir::new(&tests_dir) {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|part| part.to_str()) == Some("rl") {
            test_files.push(path.to_path_buf());
        }
    }
    test_files.sort();

    if test_files.is_empty() {
        print_info("No test files found");
        return Ok(());
    }

    print_info(&format!("Running tests from {} file(s)", test_files.len()));
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut matched = 0usize;

    for test_file in test_files {
        let display = test_file
            .strip_prefix(&root)
            .unwrap_or(&test_file)
            .display();
        match run_test_file(
            &root,
            &project_source_root,
            &test_file,
            &resolved_dependencies,
            filter.as_deref(),
        )
        .await
        {
            Ok(outcomes) => {
                if outcomes.is_empty() {
                    continue;
                }
                println!("{}", display);
                matched += outcomes.len();
                for outcome in outcomes {
                    match outcome.failure {
                        None => {
                            println!("  {} ... PASSED", outcome.name);
                            passed += 1;
                        }
                        Some(failure) => {
                            println!("  {} ... FAILED", outcome.name);
                            print_error(&format!("  {failure}"));
                            failed += 1;
                        }
                    }
                    print_test_output("stdout", &outcome.stdout);
                    print_test_output("stderr", &outcome.stderr);
                }
            }
            Err(error) => {
                // A filter narrows executable function names, not compilation.
                // Never hide a parse or compile failure just because the file
                // name differs from the filter.
                println!("{} ... FAILED", display);
                print_error(&format!("  {error:#}"));
                matched += 1;
                failed += 1;
            }
        }
    }

    println!();
    if matched == 0 {
        if let Some(filter) = filter {
            bail!("No executable tests matched filter '{filter}'");
        }
        bail!(
            "No executable tests found; declare `pub fun test_name: () -> Boolean = {{ ... }}` under tests/"
        );
    }
    if failed == 0 {
        print_success(&format!("All tests passed! ({passed} total)"));
        Ok(())
    } else {
        bail!("{passed} passed, {failed} failed")
    }
}

async fn run_test_file(
    project_root: &Path,
    project_source_root: &Path,
    test_file: &PathBuf,
    resolved_dependencies: &[ResolvedLocalDependency],
    filter: Option<&str>,
) -> Result<Vec<TestOutcome>> {
    let source = std::fs::read_to_string(test_file)
        .with_context(|| format!("Failed to read test source {}", test_file.display()))?;
    let (remaining, program) = parse_program(&source)
        .map_err(|error| anyhow::anyhow!(format_parse_error(&source, error)))?;
    if !remaining.trim().is_empty() {
        bail!(
            "Test parser left unconsumed source near `{}`",
            remaining.trim()
        );
    }
    let tests = discover_test_functions(&program.declarations)?
        .into_iter()
        .filter(|test| filter.is_none_or(|needle| test.name.contains(needle)))
        .collect::<Vec<_>>();
    if tests.is_empty() {
        return Ok(Vec::new());
    }

    let staging = tempfile::Builder::new()
        .prefix("warder-test-")
        .tempdir()
        .context("Failed to create test compilation directory")?;
    let wat_output = staging.path().join("test.wat");
    let compiler =
        std::env::var("RESTRICT_LANG_BIN").unwrap_or_else(|_| "restrict_lang".to_string());
    let mut command = Command::new(compiler);
    command
        .current_dir(project_root)
        .args(["--target", "wasip1"]);
    command.arg("--module-root").arg(format!(
        "{}={}",
        PROJECT_SOURCE_NAMESPACE,
        project_source_root
            .to_str()
            .context("Project source root is not valid UTF-8")?
    ));
    for dependency in resolved_dependencies {
        command
            .arg("--module-root")
            .arg(dependency.module_root_arg());
    }
    let output = command
        .arg(test_file)
        .arg(&wat_output)
        .output()
        .context("Failed to compile test file")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Test compilation failed:\n{}", stderr.trim());
    }

    let wasm = wat::parse_file(&wat_output).with_context(|| {
        format!(
            "Test compiler emitted invalid WAT at {}",
            wat_output.display()
        )
    })?;
    execute_test_functions(&wasm, &tests)
}

fn discover_test_functions(declarations: &[TopDecl]) -> Result<Vec<TestFunction>> {
    let mut tests = Vec::new();
    for declaration in declarations {
        match declaration {
            TopDecl::Export(export) => {
                if let TopDecl::Function(function) = export.item.as_ref() {
                    if is_test_name(&function.name) {
                        validate_test_signature(function)?;
                        tests.push(TestFunction {
                            name: function.name.clone(),
                        });
                    }
                }
            }
            TopDecl::Function(function) if is_test_name(&function.name) => {
                bail!(
                    "Test function '{}' must be public so Warder can execute its Wasm export; write `pub fun {}: () -> Boolean = {{ ... }}`",
                    function.name,
                    function.name
                );
            }
            _ => {}
        }
    }
    Ok(tests)
}

fn is_test_name(name: &str) -> bool {
    name.starts_with("test_") || name.ends_with("_test")
}

fn validate_test_signature(function: &FunDecl) -> Result<()> {
    if function.is_async {
        bail!("Test function '{}' must be synchronous", function.name);
    }
    if !function.type_params.is_empty() {
        bail!("Test function '{}' must not be generic", function.name);
    }
    if !function.params.is_empty() {
        bail!(
            "Test function '{}' must take no parameters; expected `() -> Boolean`",
            function.name
        );
    }
    if function.return_type != Some(Type::Named("Boolean".to_string())) {
        bail!(
            "Test function '{}' must declare `-> Boolean` so false can fail the test",
            function.name
        );
    }
    Ok(())
}

fn execute_test_functions(wasm: &[u8], tests: &[TestFunction]) -> Result<Vec<TestOutcome>> {
    wasmparser::Validator::new()
        .validate_all(wasm)
        .context("Compiled test module is invalid WebAssembly")?;
    let engine = Engine::default();
    let module = Module::new(&engine, wasm).context("Failed to load compiled test module")?;
    let mut linker = Linker::new(&engine);
    linker
        .func_wrap("wasi_snapshot_preview1", "fd_write", capture_fd_write)
        .context("Failed to register the test stdout bridge")?;
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "proc_exit",
            |_caller: Caller<'_, TestIo>, _code: i32| {},
        )
        .context("Failed to register the test process-exit bridge")?;

    let mut outcomes = Vec::with_capacity(tests.len());
    for test in tests {
        // Give every test a fresh instance so one test cannot leak mutable
        // globals or linear-memory state into the next one.
        let mut store = Store::new(&engine, TestIo::default());
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .context("Failed to instantiate compiled test module")?;
        let failure = match instance.get_typed_func::<(), i32>(&store, &test.name) {
            Ok(function) => match function.call(&mut store, ()) {
                Ok(0) => Some("returned false".to_string()),
                Ok(1) => None,
                Ok(value) => Some(format!(
                    "returned invalid Boolean representation {value}; expected 0 or 1"
                )),
                Err(error) => Some(format!("trapped: {error}")),
            },
            Err(error) => Some(format!("missing executable Boolean export: {error}")),
        };
        let stdout = String::from_utf8_lossy(&store.data().stdout).into_owned();
        let stderr = String::from_utf8_lossy(&store.data().stderr).into_owned();
        outcomes.push(TestOutcome {
            name: test.name.clone(),
            failure,
            stdout,
            stderr,
        });
    }
    Ok(outcomes)
}

fn read_i32(memory: Memory, caller: &Caller<'_, TestIo>, offset: i32) -> Result<i32, i32> {
    let offset = usize::try_from(offset).map_err(|_| 1)?;
    let mut bytes = [0; 4];
    memory.read(caller, offset, &mut bytes).map_err(|_| 1)?;
    Ok(i32::from_le_bytes(bytes))
}

fn capture_fd_write(
    mut caller: Caller<'_, TestIo>,
    fd: i32,
    iovs: i32,
    iovs_len: i32,
    nwritten: i32,
) -> i32 {
    const MAX_IOVECS: i32 = 1024;
    const MAX_CAPTURE_BYTES: usize = 16 * 1024 * 1024;

    if !(0..=MAX_IOVECS).contains(&iovs_len) {
        return 1;
    }
    let Some(memory) = caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
    else {
        return 1;
    };

    let mut captured = Vec::new();
    for index in 0..iovs_len {
        let Some(iov) = index
            .checked_mul(8)
            .and_then(|offset| iovs.checked_add(offset))
        else {
            return 1;
        };
        let base = match read_i32(memory, &caller, iov) {
            Ok(base) => base,
            Err(errno) => return errno,
        };
        let Some(len_offset) = iov.checked_add(4) else {
            return 1;
        };
        let len = match read_i32(memory, &caller, len_offset) {
            Ok(len) => len,
            Err(errno) => return errno,
        };
        let Ok(base) = usize::try_from(base) else {
            return 1;
        };
        let Ok(len) = usize::try_from(len) else {
            return 1;
        };
        if captured
            .len()
            .checked_add(len)
            .is_none_or(|total| total > MAX_CAPTURE_BYTES)
        {
            return 1;
        }

        let start = captured.len();
        captured.resize(start + len, 0);
        if memory.read(&caller, base, &mut captured[start..]).is_err() {
            return 1;
        }
    }

    let existing_len = match fd {
        1 => caller.data().stdout.len(),
        2 => caller.data().stderr.len(),
        _ => return 8,
    };
    if existing_len
        .checked_add(captured.len())
        .is_none_or(|total| total > MAX_CAPTURE_BYTES)
    {
        return 1;
    }
    match fd {
        1 => caller.data_mut().stdout.extend_from_slice(&captured),
        2 => caller.data_mut().stderr.extend_from_slice(&captured),
        _ => unreachable!("file descriptor was validated above"),
    }

    let Ok(nwritten) = usize::try_from(nwritten) else {
        return 1;
    };
    if memory
        .write(
            &mut caller,
            nwritten,
            &(captured.len() as i32).to_le_bytes(),
        )
        .is_err()
    {
        return 1;
    }
    0
}

fn print_test_output(label: &str, output: &str) {
    for line in output.lines() {
        println!("    {label}: {line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_only_public_boolean_test_functions() {
        let source = r#"
pub fun test_passes: () -> Boolean = { true }
pub fun helper: () -> Boolean = { false }
"#;
        let (_, program) = parse_program(source).unwrap();
        assert_eq!(
            discover_test_functions(&program.declarations).unwrap(),
            vec![TestFunction {
                name: "test_passes".to_string()
            }]
        );
    }

    #[test]
    fn rejects_private_or_non_boolean_tests() {
        let (_, private) = parse_program("fun test_private: () -> Boolean = { true }").unwrap();
        assert!(discover_test_functions(&private.declarations)
            .unwrap_err()
            .to_string()
            .contains("must be public"));

        let (_, scalar) = parse_program("pub fun test_scalar: () -> Int32 = { 1 }").unwrap();
        assert!(discover_test_functions(&scalar.declarations)
            .unwrap_err()
            .to_string()
            .contains("-> Boolean"));
    }

    #[test]
    fn executes_true_and_reports_false() {
        let wasm = wat::parse_str(
            r#"(module
                (func (export "test_true") (result i32) i32.const 1)
                (func (export "test_false") (result i32) i32.const 0)
            )"#,
        )
        .unwrap();
        let tests = vec![
            TestFunction {
                name: "test_true".to_string(),
            },
            TestFunction {
                name: "test_false".to_string(),
            },
        ];
        let outcomes = execute_test_functions(&wasm, &tests).unwrap();
        assert!(outcomes[0].failure.is_none());
        assert_eq!(outcomes[1].failure.as_deref(), Some("returned false"));
    }

    #[test]
    fn supplies_the_standard_program_io_imports() {
        let wasm = wat::parse_str(
            r#"(module
                (import "wasi_snapshot_preview1" "fd_write"
                    (func $fd_write (param i32 i32 i32 i32) (result i32)))
                (import "wasi_snapshot_preview1" "proc_exit"
                    (func $proc_exit (param i32)))
                (memory (export "memory") 1)
                (data (i32.const 0) "\10\00\00\00\06\00\00\00")
                (data (i32.const 16) "hello\0a")
                (func (export "test_output") (result i32)
                    i32.const 1
                    i32.const 0
                    i32.const 1
                    i32.const 8
                    call $fd_write
                    drop
                    i32.const 1)
            )"#,
        )
        .unwrap();
        let outcomes = execute_test_functions(
            &wasm,
            &[TestFunction {
                name: "test_output".to_string(),
            }],
        )
        .unwrap();

        assert!(outcomes[0].failure.is_none());
        assert_eq!(outcomes[0].stdout, "hello\n");
        assert!(outcomes[0].stderr.is_empty());
    }
}
