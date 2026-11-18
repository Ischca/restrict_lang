use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn cli_check_wasm_core_rejects_all_host_io_builtins() {
    let temp = TempDir::new().unwrap();

    let cases = [
        ("println", r#"fun main: () -> () = { "hi" |> println }"#),
        ("print", r#"fun main: () -> () = { "hi" |> print }"#),
        ("eprint", r#"fun main: () -> () = { "err" |> eprint }"#),
        ("eprintln", r#"fun main: () -> () = { "err" |> eprintln }"#),
        ("print_int", r#"fun main: () -> () = { 42 |> print_int }"#),
        ("print_float", r#"fun main: () -> () = { 3.14 |> print_float }"#),
    ];

    for (name, source) in cases {
        let file = temp.path().join(format!("{name}.rl"));
        fs::write(&file, source).unwrap();

        let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
            .args(["--check", "--target", "wasm-core"])
            .arg(&file)
            .output()
            .unwrap();

        assert!(
            !output.status.success(),
            "--check --target wasm-core must reject {name}, stdout={}",
            String::from_utf8_lossy(&output.stdout)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stdout.contains("OK"), "must not print OK for {name}");
        assert!(
            stderr.contains(name) && stderr.contains("wasip1") && stderr.contains("host I/O"),
            "unexpected stderr for {name}: {stderr}"
        );
    }
}

#[test]
fn cli_check_wasm_core_accepts_host_neutral_code_and_display() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("neutral.rl");
    fs::write(
        &file,
        r#"
pub fun compute: (x: Int32) -> Int32 = { x * 2 + 1 }
fun text: () -> String = { 7 |> display }
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .args(["--check", "--target", "wasm-core"])
        .arg(&file)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("OK"));
}

#[test]
fn cli_allows_project_module_root_for_same_src_directory() {
    let temp = TempDir::new().unwrap();
    let src = temp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(
        src.join("math.rl"),
        "pub fun double: (x: Int32) -> Int32 = { x * 2 }\n",
    )
    .unwrap();
    fs::write(
        src.join("helper.rl"),
        "pub fun offset: () -> Int32 = { 1 }\n",
    )
    .unwrap();
    let main_file = src.join("main.rl");
    fs::write(
        &main_file,
        r#"
import project.math.{double}
import helper.{offset}

pub fun answer: () -> Int32 = {
    val doubled = 21 |> double;
    doubled + (() offset)
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--check")
        .arg("--module-root")
        .arg(format!("project={}", src.display()))
        .arg(&main_file)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "project=<src_dir> must support both `import project.<mod>` and `import <mod>`: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("OK"));
}

#[test]
fn cli_parse_error_displays_untruncated_message_with_line_and_column() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("bad_ok.rl");
    fs::write(
        &file,
        r#"fun demo: () -> Result<Int32, String> = {
    Ok(42)
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--check")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("write `value Result::Ok` or `error Result::Err` (pipe is optional)"),
        "CLI diagnostic must not truncate custom parser error: {stderr}"
    );
    assert!(stderr.contains("at line 2, column 5"), "{stderr}");
    assert!(stderr.contains("    Ok(42)"), "{stderr}");
}

#[test]
fn cli_fmt_formats_and_checks_files_preserving_tokens() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("messy.rl");
    fs::write(
        &file,
        "// header\nfun main: () -> Int32 = {\nval x=1+2;\nx\n}\n",
    )
    .unwrap();

    let check_before = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .args(["--fmt", "--check"])
        .arg(&file)
        .output()
        .unwrap();
    assert!(!check_before.status.success());
    assert!(
        String::from_utf8_lossy(&check_before.stderr).contains("Would reformat"),
        "{}",
        String::from_utf8_lossy(&check_before.stderr)
    );

    let format_run = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--fmt")
        .arg(&file)
        .output()
        .unwrap();
    assert!(
        format_run.status.success(),
        "{}",
        String::from_utf8_lossy(&format_run.stderr)
    );
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "// header\nfun main: () -> Int32 = {\n    val x = 1 + 2;\n    x\n}\n"
    );

    let check_after = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .args(["--fmt", "--check"])
        .arg(&file)
        .output()
        .unwrap();
    assert!(check_after.status.success());
}

#[test]
fn cli_reports_located_affine_and_release_surface_errors() {
    let temp = TempDir::new().unwrap();
    let affine_file = temp.path().join("affine.rl");
    fs::write(
        &affine_file,
        "fun main: () -> String = {\n    val text = \"hi\";\n    val a = text;\n    text\n}\n",
    )
    .unwrap();

    let affine_out = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--check")
        .arg(&affine_file)
        .output()
        .unwrap();
    assert!(!affine_out.status.success());
    let affine_err = String::from_utf8_lossy(&affine_out.stderr);
    assert!(affine_err.contains("at line 4, column 5 (second use)"), "{affine_err}");
    assert!(affine_err.contains("first used at line 3, column 13"), "{affine_err}");

    let release_file = temp.path().join("release.rl");
    fs::write(
        &release_file,
        "pub fun greet: () -> String = {\n    \"hi\"\n}\n",
    )
    .unwrap();
    let release_out = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--check")
        .arg(&release_file)
        .output()
        .unwrap();
    assert!(!release_out.status.success());
    let release_err = String::from_utf8_lossy(&release_out.stderr);
    assert!(release_err.contains("Release surface error:"), "{release_err}");
    assert!(release_err.contains("at line 1, column 9"), "{release_err}");
}
