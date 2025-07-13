// build.rs - Custom build script for Restrict Language

use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=std/");
    println!("cargo:rerun-if-changed=examples/");
    
    // Generate build information
    generate_build_info();
    
    // Setup standard library path
    setup_std_lib_path();
    
    // Copy standard library files if needed
    copy_std_lib_files();
    
    // Generate version information
    generate_version_info();
    
    // Check for required tools
    check_dependencies();
}

fn generate_build_info() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("build_info.rs");
    
    let git_hash = get_git_hash().unwrap_or_else(|| "unknown".to_string());
    let build_time = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    let rustc_version = get_rustc_version().unwrap_or_else(|| "unknown".to_string());
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    let profile = env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());
    
    let build_info = format!(
        r#"
pub const GIT_HASH: &str = "{}";
pub const BUILD_TIME: &str = "{}";
pub const RUSTC_VERSION: &str = "{}";
pub const TARGET: &str = "{}";
pub const PROFILE: &str = "{}";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PKG_NAME: &str = env!("CARGO_PKG_NAME");
"#,
        git_hash, build_time, rustc_version, target, profile
    );
    
    fs::write(dest_path, build_info).unwrap();
}

fn setup_std_lib_path() {
    let std_path = if let Ok(path) = env::var("RESTRICT_LANG_STD_PATH") {
        path
    } else {
        // Default to std/ directory relative to project root
        env::var("CARGO_MANIFEST_DIR")
            .map(|dir| format!("{}/std", dir))
            .unwrap_or_else(|_| "std".to_string())
    };
    
    println!("cargo:rustc-env=RESTRICT_LANG_STD_PATH={}", std_path);
}

fn copy_std_lib_files() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let std_dest = Path::new(&out_dir).join("std");
    
    if let Ok(std_src) = env::var("CARGO_MANIFEST_DIR") {
        let std_src = Path::new(&std_src).join("std");
        if std_src.exists() {
            // Copy standard library files to build output
            if std_dest.exists() {
                fs::remove_dir_all(&std_dest).ok();
            }
            copy_dir_all(&std_src, &std_dest).ok();
        }
    }
}

fn generate_version_info() {
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let major_minor = version.split('.').take(2).collect::<Vec<_>>().join(".");
    
    println!("cargo:rustc-env=RESTRICT_LANG_VERSION={}", version);
    println!("cargo:rustc-env=RESTRICT_LANG_VERSION_MAJOR_MINOR={}", major_minor);
}

fn check_dependencies() {
    // Check if wasmtime is available (optional)
    if Command::new("wasmtime").arg("--version").output().is_ok() {
        println!("cargo:rustc-cfg=feature=\"wasmtime_available\"");
    }
    
    // Check if Node.js is available (for VS Code extension)
    if Command::new("node").arg("--version").output().is_ok() {
        println!("cargo:rustc-cfg=feature=\"nodejs_available\"");
    }
    
    // Check if wasm-pack is available
    if Command::new("wasm-pack").arg("--version").output().is_ok() {
        println!("cargo:rustc-cfg=feature=\"wasm_pack_available\"");
    }
}

fn get_git_hash() -> Option<String> {
    Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
}

fn get_rustc_version() -> Option<String> {
    Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}