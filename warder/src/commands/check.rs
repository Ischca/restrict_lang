use super::build::resolve_project_file;
use super::{find_project_root, load_manifest, print_info, print_success, resolve_restrict_lang_bin};
use crate::dependencies::{
    resolve_local_dependencies, validate_local_dependency_layout, PROJECT_SOURCE_NAMESPACE,
};
use crate::manifest::{COMPONENT_SCHEMA_VERSION, COMPONENT_SCHEMA_VERSION_V2};
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;
use walkdir::WalkDir;

const COMPONENT_ARENA_SIZE_BYTES: u32 = 4096;

pub async fn check_project() -> Result<()> {
    let root = find_project_root()?;
    let root = std::fs::canonicalize(&root)
        .with_context(|| format!("Failed to canonicalize project root {}", root.display()))?;
    let manifest = load_manifest()?;
    let component_contract = manifest.compiler_component_contract()?;
    let entry_path =
        resolve_project_file(&root, Path::new(&manifest.package.entry), "package.entry")?;
    let project_source_root = entry_path
        .parent()
        .context("package.entry must have a source directory")?;

    let (resolved_dependencies, _) = resolve_local_dependencies(&root, &manifest)?;
    validate_local_dependency_layout(&root, &manifest, &resolved_dependencies)?;

    print_info(&format!(
        "Checking {} v{}...",
        manifest.package.name, manifest.package.version
    ));

    if let Some(contract) = component_contract {
        let source = std::fs::read_to_string(&entry_path)
            .with_context(|| format!("Failed to read entry source {}", entry_path.display()))?;
        let uses_project_namespace = restrict_lang::parser::parse_program(&source)
            .is_ok_and(|(_, program)| {
                program.imports.iter().any(|import| {
                    import
                        .module_path
                        .first()
                        .is_some_and(|root| root == PROJECT_SOURCE_NAMESPACE)
                })
            });
        let mut package_roots = resolved_dependencies
            .iter()
            .map(|dependency| (dependency.alias.clone(), dependency.source_dir.clone()))
            .collect::<Vec<_>>();
        let staging_dir = if uses_project_namespace {
            let dir = tempfile::Builder::new()
                .prefix("warder-check-")
                .tempdir()
                .context("Failed to create check staging directory")?;
            package_roots.push((
                PROJECT_SOURCE_NAMESPACE.to_string(),
                project_source_root.to_path_buf(),
            ));
            Some(dir)
        } else {
            None
        };
        let component_entry_path = match &staging_dir {
            Some(dir) => dir.path().join("main.rl"),
            None => entry_path.clone(),
        };
        let arena_size_bytes = manifest
            .build
            .arena_bytes
            .unwrap_or(COMPONENT_ARENA_SIZE_BYTES);
        let optimization_level = if manifest.build.optimize {
            restrict_lang::WasmOptimizationLevel::Release
        } else {
            restrict_lang::WasmOptimizationLevel::None
        };
        match contract.schema_version {
            COMPONENT_SCHEMA_VERSION => {
                let selection = manifest
                    .compiler_component_selection()?
                    .context("validated schema-v1 Component selection disappeared")?;
                restrict_lang::component::compile_component(
                    restrict_lang::component::ComponentCompileRequest {
                        source: &source,
                        entry_path: &component_entry_path,
                        package_roots: &package_roots,
                        selection: &selection,
                        optimization_level,
                        arena_size_bytes,
                    },
                )
            }
            COMPONENT_SCHEMA_VERSION_V2 => restrict_lang::component::compile_component_contract(
                restrict_lang::component::ComponentCompileContractRequest {
                    source: &source,
                    entry_path: &component_entry_path,
                    package_roots: &package_roots,
                    selection: &contract,
                    optimization_level,
                    arena_size_bytes,
                },
            ),
            schema => bail!("Unsupported component schema {schema}"),
        }
        .context("Component check failed")?;
    } else {
        let target_profile = match manifest.build.target.as_str() {
            "wasm-core" => "wasm-core",
            _ => "wasip1",
        };
        run_compiler_check(
            &root,
            project_source_root,
            &entry_path,
            &resolved_dependencies,
            target_profile,
            manifest.build.arena_bytes,
        )?;
    }

    let tests_dir = root.join("tests");
    if tests_dir.is_dir() {
        let mut test_files = Vec::new();
        for entry in WalkDir::new(&tests_dir) {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|part| part.to_str()) == Some("rl") {
                test_files.push(path.to_path_buf());
            }
        }
        test_files.sort();
        for test_file in test_files {
            run_compiler_check(
                &root,
                project_source_root,
                &test_file,
                &resolved_dependencies,
                "wasip1",
                manifest.build.arena_bytes,
            )?;
        }
    }

    print_success(&format!(
        "Checked {} v{}",
        manifest.package.name, manifest.package.version
    ));
    Ok(())
}

fn run_compiler_check(
    project_root: &Path,
    project_source_root: &Path,
    source_file: &Path,
    resolved_dependencies: &[crate::dependencies::ResolvedLocalDependency],
    target_profile: &str,
    arena_bytes: Option<u32>,
) -> Result<()> {
    let compiler = resolve_restrict_lang_bin(Some(project_root));
    let mut cmd = Command::new(&compiler);
    cmd.current_dir(project_root)
        .args(["--check", "--target", target_profile]);
    if let Some(arena_bytes) = arena_bytes {
        cmd.arg("--arena-bytes").arg(arena_bytes.to_string());
    }
    let mut project_root_arg = std::ffi::OsString::from(PROJECT_SOURCE_NAMESPACE);
    project_root_arg.push("=");
    project_root_arg.push(project_source_root);
    cmd.arg("--module-root").arg(project_root_arg);
    for dependency in resolved_dependencies {
        cmd.arg("--module-root").arg(dependency.module_root_arg());
    }
    cmd.arg(source_file);

    let output = cmd
        .output()
        .context("Failed to run restrict_lang compiler. Is it installed and in PATH?")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "Check failed for {}:\n{}",
            source_file
                .strip_prefix(project_root)
                .unwrap_or(source_file)
                .display(),
            stderr.trim()
        );
    }
    Ok(())
}
