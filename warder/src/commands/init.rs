use super::{
    find_enclosing_project_root, inspect_regular_candidate, print_info, print_success,
    print_warning, reject_legacy_lock,
};
use crate::manifest::{Manifest, WARDER_LOCK_FILENAME, WARDER_MANIFEST_FILENAME};
use anyhow::{bail, Result};
use std::fs;

pub fn init_project() -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let manifest_path = current_dir.join(WARDER_MANIFEST_FILENAME);
    reject_legacy_lock(&current_dir)?;

    if let Some(existing_root) = find_enclosing_project_root(&current_dir)? {
        if existing_root == current_dir {
            bail!(
                "Project already initialized ({} exists)",
                WARDER_MANIFEST_FILENAME
            );
        }
        bail!(
            "Cannot initialize a nested Warder project at {}; repository root is {}. Add package configuration to the root {} instead",
            current_dir.display(),
            existing_root.display(),
            WARDER_MANIFEST_FILENAME
        );
    }

    let orphan_lock = current_dir.join(WARDER_LOCK_FILENAME);
    if inspect_regular_candidate(&orphan_lock, "Dependency lock")? {
        bail!(
            "Cannot initialize with orphaned generated lock {}; remove it before creating {}",
            orphan_lock.display(),
            WARDER_MANIFEST_FILENAME
        );
    }

    // Get project name from directory
    let project_name = current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("my-project")
        .to_string();

    // Create the repository-root warder.toml.
    let manifest = Manifest::new(&project_name);
    manifest.save(&manifest_path)?;

    // Create src directory if it doesn't exist
    let src_dir = current_dir.join("src");
    if !src_dir.exists() {
        fs::create_dir(&src_dir)?;

        // Create main.rl if it doesn't exist
        let main_path = src_dir.join("main.rl");
        if !main_path.exists() {
            let main_content = r#"// Entry point for your Restrict Language project

fun main: () -> () = {
    "Hello from Restrict Language!" |> println
}
"#;
            fs::write(main_path, main_content)?;
        }
    } else {
        // Check if there's already a main.rl
        let main_path = src_dir.join("main.rl");
        if !main_path.exists() {
            print_warning(&format!(
                "src/ directory exists but no main.rl found. You may need to update the 'entry' field in {}",
                WARDER_MANIFEST_FILENAME
            ));
        }
    }

    // Create tests directory if it doesn't exist
    let tests_dir = current_dir.join("tests");
    if !tests_dir.exists() {
        fs::create_dir(&tests_dir)?;
    }

    // Create .gitignore if it doesn't exist
    let gitignore_path = current_dir.join(".gitignore");
    if !gitignore_path.exists() {
        let gitignore_content = r#"# Restrict Language
/dist/
/.restrict-cache/
.warder-build.lock
.warder-build-transaction.toml

# Editor
.vscode/
.idea/
*.swp
"#;
        fs::write(gitignore_path, gitignore_content)?;
    }

    print_success(&format!(
        "Initialized Restrict Language project '{}'",
        project_name
    ));
    print_info("Next steps:");
    println!("  warder build");
    println!("  warder run");

    Ok(())
}
