use anyhow::{Result, bail};
use colored::*;
use std::path::PathBuf;

mod new;
mod init;
mod add;
mod build;
mod run;
mod test;
mod publish;
mod wrap;
mod doctor;

pub use new::new_project;
pub use init::init_project;
pub use add::{add_dependency, remove_dependency};
pub use build::build_project;
pub use run::run_project;
pub use test::test_project;
pub use publish::publish_package;
pub use wrap::{wrap_wasm, unwrap_cage};
pub use doctor::doctor_check;

use crate::manifest::Manifest;

pub fn find_project_root() -> Result<PathBuf> {
    let current = std::env::current_dir()?;
    let mut path = current.as_path();
    
    loop {
        let manifest_path = path.join("package.rl.toml");
        if manifest_path.exists() {
            return Ok(path.to_path_buf());
        }
        
        match path.parent() {
            Some(parent) => path = parent,
            None => bail!("Not in a Restrict Language project (no package.rl.toml found)"),
        }
    }
}

pub fn load_manifest() -> Result<Manifest> {
    let root = find_project_root()?;
    let manifest_path = root.join("package.rl.toml");
    Manifest::load(&manifest_path)
}

pub fn save_manifest(manifest: &Manifest) -> Result<()> {
    let root = find_project_root()?;
    let manifest_path = root.join("package.rl.toml");
    manifest.save(&manifest_path)
}

pub fn print_success(message: &str) {
    println!("{} {}", "✓".green().bold(), message);
}

pub fn print_error(message: &str) {
    eprintln!("{} {}", "✗".red().bold(), message);
}

pub fn print_info(message: &str) {
    println!("{} {}", "ℹ".blue().bold(), message);
}

pub fn print_warning(message: &str) {
    println!("{} {}", "⚠".yellow().bold(), message);
}