use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

fn write_app(root: &Path, dependencies: &str) {
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(
        root.join("package.rl.toml"),
        format!(
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\nentry = \"src/main.rl\"\nedition = \"2025\"\n\n[dependencies]\n{dependencies}\n\n[build]\ntarget = \"wasm32\"\noptimize = true\noutput = \"dist/\"\n"
        ),
    )
    .unwrap();
    let (main_source, test_source) = if dependencies.trim().is_empty() {
        (
            "pub fun app_score: () -> Int32 = { 42 }\n",
            "fun dependency_test: () -> Int32 = { 42 }\n",
        )
    } else {
        (
            "import local_utils.{score}\n\npub fun app_score: () -> Int32 = { () score }\n",
            "import local_utils.{score}\n\nfun dependency_test: () -> Int32 = { () score }\n",
        )
    };
    fs::write(root.join("src/main.rl"), main_source).unwrap();
    fs::write(root.join("tests/dependency_test.rl"), test_source).unwrap();
}

fn write_dependency(root: &Path, version: &str, score: i32) {
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("package.rl.toml"),
        format!(
            "[package]\nname = \"local-utils\"\nversion = \"{version}\"\nentry = \"src/main.rl\"\nedition = \"2025\"\n\n[dependencies]\n"
        ),
    )
    .unwrap();
    fs::write(
        root.join("src/lib.rl"),
        format!("pub fun score: () -> Int32 = {{ {score} }}\n"),
    )
    .unwrap();
}

fn fake_compiler(temp: &TempDir) -> (PathBuf, PathBuf) {
    let source = temp.path().join("fake-restrict-lang.rs");
    let compiler = temp.path().join(format!(
        "fake-restrict-lang{}",
        std::env::consts::EXE_SUFFIX
    ));
    let captured_args = temp.path().join("compiler-args.txt");
    fs::write(
        &source,
        r#"use std::io::Write;

fn main() {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    let capture = std::env::var_os("WARDER_CAPTURE_ARGS")
        .expect("WARDER_CAPTURE_ARGS must be set");
    let mut capture_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(capture)
        .expect("capture file should open");
    for argument in &arguments {
        writeln!(capture_file, "{}", argument.to_string_lossy()).unwrap();
    }
    writeln!(
        capture_file,
        "cwd={}",
        std::env::current_dir().unwrap().display()
    )
    .unwrap();
    if let Some(path) = std::env::var_os("WARDER_MUTATE_SOURCE") {
        let content = std::env::var("WARDER_MUTATE_SOURCE_CONTENT")
            .unwrap_or_else(|_| "pub fun changed: () -> Int32 = { 99 }\n".to_string());
        std::fs::write(path, content).expect("fake compiler should mutate the requested source");
    }
    if let Some(ready) = std::env::var_os("WARDER_FAKE_READY") {
        std::fs::write(&ready, b"ready").unwrap();
        let continue_path = std::env::var_os("WARDER_FAKE_CONTINUE")
            .expect("WARDER_FAKE_CONTINUE must accompany WARDER_FAKE_READY");
        for _ in 0..1_000 {
            if std::path::Path::new(&continue_path).exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            std::path::Path::new(&continue_path).exists(),
            "fake compiler timed out waiting for its barrier"
        );
    }
    if arguments.iter().any(|argument| argument == "--check") {
        return;
    }
    let output = arguments.last().expect("compiler output path is required");
    let wat = std::env::var("WARDER_FAKE_WAT")
        .unwrap_or_else(|_| "(module (func (export \"main\")))\n".to_string());
    std::fs::write(output, wat)
        .expect("fake WAT should be written");
}
"#,
    )
    .unwrap();
    let output = Command::new("rustc")
        .arg(&source)
        .arg("-o")
        .arg(&compiler)
        .output()
        .expect("rustc should compile the fake compiler");
    assert!(
        output.status.success(),
        "fake compiler should compile: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    (compiler, captured_args)
}

fn failing_compiler(temp: &TempDir) -> PathBuf {
    let source = temp.path().join("failing-restrict-lang.rs");
    let compiler = temp.path().join(format!(
        "failing-restrict-lang{}",
        std::env::consts::EXE_SUFFIX
    ));
    fs::write(&source, "fn main() { std::process::exit(42); }\n").unwrap();
    let output = Command::new("rustc")
        .arg(&source)
        .arg("-o")
        .arg(&compiler)
        .output()
        .expect("rustc should compile the failing compiler");
    assert!(
        output.status.success(),
        "failing compiler should compile: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    compiler
}

fn run_warder(app: &Path, compiler: &Path, captured_args: &Path, command: &str) -> Output {
    run_warder_from(app, compiler, captured_args, command)
}

fn run_warder_from(
    current_dir: &Path,
    compiler: &Path,
    captured_args: &Path,
    command: &str,
) -> Output {
    Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(current_dir)
        .env("RESTRICT_LANG_BIN", compiler)
        .env("WARDER_CAPTURE_ARGS", captured_args)
        .arg(command)
        .output()
        .expect("warder should run")
}

#[test]
fn build_and_test_mount_direct_local_dependency_and_write_real_lock_metadata() {
    let temp = TempDir::new().unwrap();
    let app = temp.path().join("app");
    let dependency = temp.path().join("local-utils");
    write_app(&app, "local_utils = { path = \"../local-utils\" }");
    write_dependency(&dependency, "1.2.3", 42);
    let (compiler, captured_args) = fake_compiler(&temp);

    let build = run_warder(&app, &compiler, &captured_args, "build");
    assert!(
        build.status.success(),
        "warder build should succeed: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    for extension in ["wat", "wasm", "rgc"] {
        assert!(
            app.join(format!("dist/app-0.1.0.{extension}")).is_file(),
            "warder build should write the {extension} artifact"
        );
    }

    let lock = fs::read_to_string(app.join("restrict-lock.toml")).unwrap();
    assert!(lock.contains("[packages.local_utils]"), "{lock}");
    assert!(lock.contains("version = \"1.2.3\""), "{lock}");
    let sha = lock
        .lines()
        .find_map(|line| line.strip_prefix("sha256 = \"")?.strip_suffix('"'))
        .expect("local lock should contain a source hash");
    assert_eq!(sha.len(), 64);
    assert!(
        lock.contains("abi_hash = \"\""),
        "v1 source-only locks retain an empty ABI hash for older Warder readers: {lock}"
    );

    let build_args = fs::read_to_string(&captured_args).unwrap();
    let portable_build_args = build_args.replace('\\', "/");
    assert!(build_args.contains("--module-root"), "{build_args}");
    assert!(
        portable_build_args.lines().any(|argument| {
            argument.starts_with("local_utils=")
                && argument.contains("/.warder-build-")
                && argument.ends_with("/dependencies/0000/src")
        }),
        "the compiler must receive an immutable dependency snapshot: {build_args}"
    );
    assert!(
        portable_build_args
            .lines()
            .any(|argument| argument.ends_with("/application/src/main.rl")),
        "the compiler must receive an immutable application snapshot: {build_args}"
    );

    let test = run_warder_from(&app.join("tests"), &compiler, &captured_args, "test");
    assert!(
        test.status.success(),
        "warder test should use the same package roots: {}",
        String::from_utf8_lossy(&test.stderr)
    );
    let all_args = fs::read_to_string(&captured_args).unwrap();
    assert!(all_args.contains("--check"), "{all_args}");
    assert!(
        all_args.matches("--module-root").count() >= 2,
        "build and test should each mount the dependency: {all_args}"
    );
    let canonical_app = app.canonicalize().unwrap();
    let expected_test_cwd = format!("cwd={}", canonical_app.display()).replace('\\', "/");
    assert!(
        all_args.replace('\\', "/").contains(&expected_test_cwd),
        "warder test must anchor compiler fallback resolution at the project root: {all_args}"
    );
}

#[test]
fn rebuild_refreshes_version_and_source_hash_then_prunes_removed_dependency() {
    let temp = TempDir::new().unwrap();
    let app = temp.path().join("app");
    let dependency = temp.path().join("local-utils");
    write_app(&app, "local_utils = { path = \"../local-utils\" }");
    write_dependency(&dependency, "1.2.3", 41);
    let (compiler, captured_args) = fake_compiler(&temp);

    assert!(run_warder(&app, &compiler, &captured_args, "build")
        .status
        .success());
    let first_lock = fs::read_to_string(app.join("restrict-lock.toml")).unwrap();

    write_dependency(&dependency, "1.3.0", 42);
    assert!(run_warder(&app, &compiler, &captured_args, "build")
        .status
        .success());
    let second_lock = fs::read_to_string(app.join("restrict-lock.toml")).unwrap();
    assert!(second_lock.contains("version = \"1.3.0\""), "{second_lock}");
    assert_ne!(
        first_lock, second_lock,
        "source changes must refresh the lock"
    );

    write_app(&app, "");
    assert!(run_warder(&app, &compiler, &captured_args, "build")
        .status
        .success());
    let pruned_lock = fs::read_to_string(app.join("restrict-lock.toml")).unwrap();
    assert!(!pruned_lock.contains("local_utils"), "{pruned_lock}");
}

#[test]
fn unsupported_dependency_does_not_mutate_lock_or_create_build_directory() {
    let temp = TempDir::new().unwrap();
    let app = temp.path().join("app");
    write_app(&app, "registry_pkg = \"1.0.0\"");
    let sentinel = "version = 1\n# keep-existing-lock\n";
    fs::write(app.join("restrict-lock.toml"), sentinel).unwrap();
    let (compiler, captured_args) = fake_compiler(&temp);

    let output = run_warder(&app, &compiler, &captured_args, "build");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Registry dependency 'registry_pkg'"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(app.join("restrict-lock.toml")).unwrap(),
        sentinel
    );
    assert!(!app.join("dist").exists());
    assert!(
        !captured_args.exists(),
        "compiler must not run after rejection"
    );
}

#[test]
fn add_rejects_an_overlapping_dependency_without_mutating_the_manifest() {
    let temp = TempDir::new().unwrap();
    let app = temp.path().join("app");
    let dependency = app.join("src/vendor");
    write_app(&app, "");
    write_dependency(&dependency, "1.2.3", 42);
    let manifest_path = app.join("package.rl.toml");
    let previous_manifest = fs::read(&manifest_path).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(&app)
        .args(["add", "vendor", "--path", "src/vendor"])
        .output()
        .expect("warder add should run");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("package source roots must be disjoint"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(manifest_path).unwrap(), previous_manifest);
}

#[test]
fn build_rejects_manifest_paths_and_output_names_that_escape_the_project() {
    let temp = TempDir::new().unwrap();
    let (compiler, captured_args) = fake_compiler(&temp);

    let output_escape_app = temp.path().join("output-escape-app");
    write_app(&output_escape_app, "");
    let manifest_path = output_escape_app.join("package.rl.toml");
    let manifest = fs::read_to_string(&manifest_path)
        .unwrap()
        .replace("output = \"dist/\"", "output = \"../escape\"");
    fs::write(&manifest_path, manifest).unwrap();
    let output = run_warder(&output_escape_app, &compiler, &captured_args, "build");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("build.output must be"));
    assert!(!temp.path().join("escape").exists());

    let name_escape_app = temp.path().join("name-escape-app");
    write_app(&name_escape_app, "");
    let manifest_path = name_escape_app.join("package.rl.toml");
    let manifest = fs::read_to_string(&manifest_path)
        .unwrap()
        .replace("name = \"app\"", "name = \"../escape\"");
    fs::write(&manifest_path, manifest).unwrap();
    let output = run_warder(&name_escape_app, &compiler, &captured_args, "build");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Invalid package.name"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let symlink_app = temp.path().join("symlink-output-app");
        let outside = temp.path().join("outside-output");
        write_app(&symlink_app, "");
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, symlink_app.join("dist")).unwrap();
        let output = run_warder(&symlink_app, &compiler, &captured_args, "build");
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains("build.output must contain only real directories"));
        assert!(fs::read_dir(&outside).unwrap().next().is_none());
    }

    for (index, invalid_name) in ["foo:bar", "foo*bar", "foo\\\\bar"].iter().enumerate() {
        let invalid_name_app = temp.path().join(format!("invalid-name-{index}"));
        write_app(&invalid_name_app, "");
        let manifest_path = invalid_name_app.join("package.rl.toml");
        let manifest = fs::read_to_string(&manifest_path)
            .unwrap()
            .replace("name = \"app\"", &format!("name = \"{invalid_name}\""));
        fs::write(&manifest_path, manifest).unwrap();
        let output = run_warder(&invalid_name_app, &compiler, &captured_args, "build");
        assert!(
            !output.status.success(),
            "{invalid_name} should be rejected"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Invalid package.name"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn compiler_failure_preserves_the_previous_lock_and_artifact_set() {
    let temp = TempDir::new().unwrap();
    let app = temp.path().join("app");
    let dependency = temp.path().join("local-utils");
    write_app(&app, "local_utils = { path = \"../local-utils\" }");
    write_dependency(&dependency, "1.2.3", 41);
    let (compiler, captured_args) = fake_compiler(&temp);
    assert!(run_warder(&app, &compiler, &captured_args, "build")
        .status
        .success());

    let tracked_paths = [
        app.join("restrict-lock.toml"),
        app.join("dist/app-0.1.0.wat"),
        app.join("dist/app-0.1.0.wasm"),
        app.join("dist/app-0.1.0.rgc"),
    ];
    let previous = tracked_paths
        .iter()
        .map(|path| fs::read(path).unwrap())
        .collect::<Vec<_>>();

    write_dependency(&dependency, "1.2.4", 99);
    let failed = run_warder(&app, &failing_compiler(&temp), &captured_args, "build");

    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("Compilation failed"));
    for (path, expected) in tracked_paths.iter().zip(previous) {
        assert_eq!(
            fs::read(path).unwrap(),
            expected,
            "{} changed",
            path.display()
        );
    }
}

#[test]
fn application_mutation_during_compile_rejects_the_staged_artifact_set() {
    let temp = TempDir::new().unwrap();
    let app = temp.path().join("app");
    write_app(&app, "");
    let (compiler, captured_args) = fake_compiler(&temp);
    assert!(run_warder(&app, &compiler, &captured_args, "build")
        .status
        .success());

    let tracked_paths = [
        app.join("restrict-lock.toml"),
        app.join("dist/app-0.1.0.wat"),
        app.join("dist/app-0.1.0.wasm"),
        app.join("dist/app-0.1.0.rgc"),
    ];
    let previous = tracked_paths
        .iter()
        .map(|path| fs::read(path).unwrap())
        .collect::<Vec<_>>();
    let live_source = app.join("src/main.rl");
    fs::write(&live_source, "pub fun app_score: () -> Int32 = { 41 }\n").unwrap();

    let failed = Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(&app)
        .env("RESTRICT_LANG_BIN", &compiler)
        .env("WARDER_CAPTURE_ARGS", &captured_args)
        .env("WARDER_MUTATE_SOURCE", &live_source)
        .env(
            "WARDER_MUTATE_SOURCE_CONTENT",
            "pub fun app_score: () -> Int32 = { 99 }\n",
        )
        .arg("build")
        .output()
        .expect("warder should run");

    assert!(!failed.status.success());
    assert!(
        String::from_utf8_lossy(&failed.stderr).contains("Application sources changed"),
        "{}",
        String::from_utf8_lossy(&failed.stderr)
    );
    for (path, expected) in tracked_paths.iter().zip(previous) {
        assert_eq!(
            fs::read(path).unwrap(),
            expected,
            "{} changed after a rejected source race",
            path.display()
        );
    }
}

#[test]
fn concurrent_builds_are_serialized_and_publish_one_consistent_artifact_set() {
    use std::io::Read;

    let temp = TempDir::new().unwrap();
    let app = temp.path().join("app");
    write_app(&app, "");
    let (compiler, captured_args) = fake_compiler(&temp);
    let ready = temp.path().join("first-ready");
    let continue_path = temp.path().join("continue-first");

    let mut first = Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(&app)
        .env("RESTRICT_LANG_BIN", &compiler)
        .env("WARDER_CAPTURE_ARGS", &captured_args)
        .env("WARDER_FAKE_READY", &ready)
        .env("WARDER_FAKE_CONTINUE", &continue_path)
        .env("WARDER_FAKE_WAT", "(module (func (export \"first\")))\n")
        .arg("build")
        .spawn()
        .expect("first warder build should start");

    for _ in 0..1_000 {
        if ready.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        ready.exists(),
        "first build did not reach the compiler barrier"
    );

    let mut second = Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(&app)
        .env("RESTRICT_LANG_BIN", &compiler)
        .env("WARDER_CAPTURE_ARGS", &captured_args)
        .env("WARDER_FAKE_WAT", "(module (func (export \"second\")))\n")
        .arg("build")
        .spawn()
        .expect("second warder build should start");
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(
        second.try_wait().unwrap().is_none(),
        "second build should wait for the project lock"
    );

    fs::write(&continue_path, "continue").unwrap();
    assert!(
        first.wait().unwrap().success(),
        "first build should succeed"
    );
    assert!(
        second.wait().unwrap().success(),
        "second build should succeed"
    );

    let wat_path = app.join("dist/app-0.1.0.wat");
    let wasm_path = app.join("dist/app-0.1.0.wasm");
    let cage_path = app.join("dist/app-0.1.0.rgc");
    let final_wat = fs::read_to_string(&wat_path).unwrap();
    assert!(final_wat.contains("second"), "{final_wat}");
    let expected_wasm = wat::parse_str(&final_wat).unwrap();
    let final_wasm = fs::read(&wasm_path).unwrap();
    assert_eq!(final_wasm, expected_wasm);

    let cage_file = fs::File::open(cage_path).unwrap();
    let mut archive = zip::ZipArchive::new(cage_file).unwrap();
    let mut cage_wasm = Vec::new();
    archive
        .by_name("module.wasm")
        .unwrap()
        .read_to_end(&mut cage_wasm)
        .unwrap();
    assert_eq!(cage_wasm, final_wasm);
}
