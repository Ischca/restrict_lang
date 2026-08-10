use super::{find_enclosing_project_root, print_info, print_success};
use crate::manifest::{Manifest, WARDER_MANIFEST_FILENAME};
use anyhow::{bail, Result};
use std::fs;
use std::path::Path;

pub fn new_project(name: &str) -> Result<()> {
    // Validate project name
    if !is_valid_project_name(name) {
        bail!("Invalid project name '{}'. Must start with a letter and contain only letters, numbers, hyphens, and underscores.", name);
    }

    let current_dir = std::env::current_dir()?;
    if let Some(existing_root) = find_enclosing_project_root(&current_dir)? {
        bail!(
            "Cannot create a nested Warder project inside {}. Add package configuration to the root {} instead",
            existing_root.display(),
            WARDER_MANIFEST_FILENAME
        );
    }

    let project_path = Path::new(name);

    if project_path.exists() {
        bail!("Directory '{}' already exists", name);
    }

    // Create project structure
    fs::create_dir_all(project_path)?;
    fs::create_dir_all(project_path.join("src"))?;
    fs::create_dir_all(project_path.join("tests"))?;

    // Create the repository-root warder.toml.
    let manifest = Manifest::new(name);
    manifest.save(&project_path.join(WARDER_MANIFEST_FILENAME))?;

    // Create main.rl
    let main_content = r#"// Welcome to Restrict Language!

pub fun answer: () -> Int32 = {
    42
}

fun main: () -> () = {
    "Hello, World!" |> println
}
"#;
    fs::write(project_path.join("src/main.rl"), main_content)?;

    // Create an executable Boolean test exported to Warder's Wasm runner.
    let test_content = r#"// Example executable test

import project.main.{answer}

pub fun test_example: () -> Boolean = {
    () answer == 42
}
"#;
    fs::write(project_path.join("tests/main_test.rl"), test_content)?;

    // Create .gitignore
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
    fs::write(project_path.join(".gitignore"), gitignore_content)?;

    // Create README.md
    let readme_content = format!(
        r#"# {}

A new Restrict Language project.

## Getting Started

```bash
warder build
warder run
```

## Testing

`warder test` compiles and executes public `() -> Boolean` test functions under `tests/`.

```bash
warder test
```
"#,
        name
    );
    fs::write(project_path.join("README.md"), readme_content)?;

    print_success(&format!("Created project '{}'", name));
    print_info("Next steps:");
    println!("  cd {}", name);
    println!("  warder build");
    println!("  warder run");

    Ok(())
}

fn is_valid_project_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    let first_char = name.chars().next().unwrap();
    if !first_char.is_ascii_alphabetic() {
        return false;
    }

    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}
