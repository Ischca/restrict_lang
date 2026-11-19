use super::build::resolve_project_file;
use super::{find_project_root, load_manifest, print_error, print_info, print_success};
use anyhow::{bail, Context, Result};
use restrict_lang::formatter::format_source;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub async fn fmt_project(check: bool) -> Result<()> {
    let root = find_project_root()?;
    let root = std::fs::canonicalize(&root)
        .with_context(|| format!("Failed to canonicalize project root {}", root.display()))?;
    let manifest = load_manifest()?;
    let entry_path =
        resolve_project_file(&root, Path::new(&manifest.package.entry), "package.entry")?;
    let source_dir = entry_path
        .parent()
        .context("package.entry must have a source directory")?;

    let mut files = Vec::new();
    collect_rl_files(source_dir, &mut files)?;
    let tests_dir = root.join("tests");
    if tests_dir.is_dir() {
        collect_rl_files(&tests_dir, &mut files)?;
    }
    files.sort();
    files.dedup();

    if files.is_empty() {
        print_info("No Restrict source files found");
        return Ok(());
    }

    let mut unformatted = Vec::new();
    let mut formatted_count = 0usize;

    for file in &files {
        let display = file.strip_prefix(&root).unwrap_or(file).display();
        let original = std::fs::read_to_string(file)
            .with_context(|| format!("Failed to read {}", file.display()))?;
        let formatted = format_source(&original)
            .map_err(|err| anyhow::anyhow!("Formatting failed for {}: {}", display, err))?;
        if formatted != original {
            if check {
                print_error(&format!("Needs formatting: {display}"));
                unformatted.push(display.to_string());
            } else {
                std::fs::write(file, formatted).with_context(|| {
                    format!("Failed to write formatted file {}", file.display())
                })?;
                print_info(&format!("Formatted {display}"));
                formatted_count += 1;
            }
        }
    }

    if check {
        if unformatted.is_empty() {
            print_success(&format!("All {} file(s) are formatted", files.len()));
            Ok(())
        } else {
            bail!("{} file(s) need formatting", unformatted.len());
        }
    } else {
        print_success(&format!(
            "Formatted {formatted_count} of {} file(s)",
            files.len()
        ));
        Ok(())
    }
}

fn collect_rl_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in WalkDir::new(dir).follow_links(false) {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type().is_symlink() {
            continue;
        }
        if entry.file_type().is_file()
            && path.extension().and_then(|ext| ext.to_str()) == Some("rl")
        {
            out.push(path.to_path_buf());
        }
    }
    Ok(())
}
