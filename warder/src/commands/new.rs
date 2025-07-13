use anyhow::{Result, bail};
use std::fs;
use std::path::Path;
use crate::manifest::Manifest;
use super::{print_success, print_info};

pub fn new_project(name: &str) -> Result<()> {
    // Validate project name
    if !is_valid_project_name(name) {
        bail!("Invalid project name '{}'. Must start with a letter and contain only letters, numbers, hyphens, and underscores.", name);
    }
    
    let project_path = Path::new(name);
    
    if project_path.exists() {
        bail!("Directory '{}' already exists", name);
    }
    
    // Create project structure
    fs::create_dir_all(project_path)?;
    fs::create_dir_all(project_path.join("src"))?;
    fs::create_dir_all(project_path.join("tests"))?;
    
    // Create manifest
    let manifest = Manifest::new(name);
    manifest.save(&project_path.join("package.rl.toml"))?;
    
    // Create main.rl
    let main_content = r#"// Welcome to Restrict Language!

fun main = {
    "Hello, World!" |> println
}
"#;
    fs::write(project_path.join("src/main.rl"), main_content)?;
    
    // Create test file
    let test_content = r#"// Example test file

import std.test.{assert, assert_eq}

fun test_example = {
    val result = 1 + 1;
    assert_eq(result, 2)
}
"#;
    fs::write(project_path.join("tests/main_test.rl"), test_content)?;
    
    // Create .gitignore
    let gitignore_content = r#"# Restrict Language
/dist/
/.restrict-cache/
restrict-lock.toml

# Editor
.vscode/
.idea/
*.swp
"#;
    fs::write(project_path.join(".gitignore"), gitignore_content)?;
    
    // Create README.md
    let readme_content = format!(r#"# {}

A new Restrict Language project.

## Getting Started

```bash
warder build
warder run
```

## Testing

```bash
warder test
```
"#, name);
    fs::write(project_path.join("README.md"), readme_content)?;
    
    print_success(&format!("Created project '{}'", name));
    print_info(&format!("Next steps:"));
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
    if !first_char.is_alphabetic() {
        return false;
    }
    
    name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}