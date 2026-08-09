use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn minimal_manifest(name: &str) -> String {
    format!("manifest-version = 1\n\n[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n")
}

#[test]
fn new_creates_one_minimal_root_manifest_and_does_not_ignore_the_lock() {
    let temp = TempDir::new().unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(temp.path())
        .args(["new", "app"])
        .output()
        .expect("warder should run");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let app = temp.path().join("app");
    assert_eq!(
        fs::read_to_string(app.join("warder.toml")).unwrap(),
        minimal_manifest("app")
    );
    assert!(!app.join("warder.lock").exists());
    let gitignore = fs::read_to_string(app.join(".gitignore")).unwrap();
    assert!(!gitignore.contains("warder.lock"));
}

#[test]
fn new_and_init_refuse_to_create_leaf_manifests() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("root");
    let nested = root.join("packages/child");
    fs::create_dir_all(&nested).unwrap();
    fs::write(root.join("warder.toml"), minimal_manifest("root")).unwrap();

    let new_output = Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(&root)
        .args(["new", "leaf"])
        .output()
        .expect("warder should run");
    assert!(!new_output.status.success());
    assert!(
        String::from_utf8_lossy(&new_output.stderr).contains("nested Warder project"),
        "{}",
        String::from_utf8_lossy(&new_output.stderr)
    );
    assert!(!root.join("leaf").exists());

    let init_output = Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(&nested)
        .arg("init")
        .output()
        .expect("warder should run");
    assert!(!init_output.status.success());
    assert!(
        String::from_utf8_lossy(&init_output.stderr).contains("nested Warder project"),
        "{}",
        String::from_utf8_lossy(&init_output.stderr)
    );
    assert!(!nested.join("warder.toml").exists());
}

#[test]
fn init_rejects_an_orphaned_generated_lock() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("project");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("warder.lock"), "version = 1\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_warder"))
        .current_dir(&root)
        .arg("init")
        .output()
        .expect("warder should run");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("orphaned generated lock"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!root.join("warder.toml").exists());
    assert_eq!(
        fs::read_to_string(root.join("warder.lock")).unwrap(),
        "version = 1\n"
    );
}
