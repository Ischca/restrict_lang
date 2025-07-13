use anyhow::{Result, bail};
use std::fs;
use crate::manifest::Manifest;
use super::{print_success, print_info, print_warning};

pub fn init_project() -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let manifest_path = current_dir.join("package.rl.toml");
    
    if manifest_path.exists() {
        bail!("Project already initialized (package.rl.toml exists)");
    }
    
    // Get project name from directory
    let project_name = current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("my-project")
        .to_string();
    
    // Create manifest
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

fun main = {
    "Hello from Restrict Language!" |> println
}
"#;
            fs::write(main_path, main_content)?;
        }
    } else {
        // Check if there's already a main.rl
        let main_path = src_dir.join("main.rl");
        if !main_path.exists() {
            print_warning("src/ directory exists but no main.rl found. You may need to update the 'entry' field in package.rl.toml");
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
restrict-lock.toml

# Editor
.vscode/
.idea/
*.swp
"#;
        fs::write(gitignore_path, gitignore_content)?;
    }
    
    print_success(&format!("Initialized Restrict Language project '{}'", project_name));
    print_info("Next steps:");
    println!("  warder build");
    println!("  warder run");
    
    Ok(())
}