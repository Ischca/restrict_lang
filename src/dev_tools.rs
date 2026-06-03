use crate::codegen::WasmCodeGen;
use crate::diagnostics::format_parse_error;
use crate::module::resolve_program_imports_for_file;
use crate::parser::parse_program;
use crate::type_checker::type_check;
use colored::*;
use std::fs;
use std::path::Path;

pub struct DevTools;

impl DevTools {
    /// Watch mode: automatically recompile on file changes
    pub fn watch(path: &str) -> Result<(), Box<dyn std::error::Error>> {
        use notify::{Event, EventKind, RecursiveMode, Watcher};
        use std::sync::mpsc::channel;

        let (tx, rx) = channel();
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                tx.send(event).unwrap();
            }
        })?;
        watcher.watch(Path::new(path), RecursiveMode::Recursive)?;

        println!("{}", "Watching for changes...".green());

        loop {
            match rx.recv() {
                Ok(event) => {
                    // Only recompile on modify events
                    if matches!(event.kind, EventKind::Modify(_)) {
                        println!("{}", "File changed, recompiling...".yellow());
                        Self::compile_file(path);
                    }
                }
                Err(e) => eprintln!("Watch error: {:?}", e),
            }
        }
    }

    /// Compile with detailed error reporting
    pub fn compile_file(path: &str) {
        println!("{}", format!("Compiling {}...", path).blue());

        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("{}", format!("Failed to read file: {}", e).red());
                return;
            }
        };

        // Parse
        let (remaining, ast) = match parse_program(&content) {
            Ok(result) => result,
            Err(e) => {
                Self::report_parse_error(&content, e);
                return;
            }
        };

        if !remaining.trim().is_empty() {
            eprintln!("{}", format!("Unparsed input: '{}'", remaining).red());
            return;
        }

        let source_path = Path::new(path);
        let ast = match resolve_program_imports_for_file(ast, source_path) {
            Ok(ast) => ast,
            Err(e) => {
                eprintln!("{}", format!("Import resolution error: {}", e).red());
                return;
            }
        };

        // Type check
        match type_check(&ast) {
            Ok(()) => {}
            Err(e) => {
                Self::report_type_error(&content, e);
                return;
            }
        };

        // Generate code
        let mut codegen = WasmCodeGen::new();
        let wat = match codegen.generate(&ast) {
            Ok(wat) => wat,
            Err(e) => {
                eprintln!("{}", format!("Code generation error: {}", e).red());
                return;
            }
        };

        // Write output
        let out_path = path.replace(".rl", ".wat");
        match fs::write(&out_path, &wat) {
            Ok(_) => println!("{}", format!("✓ Compiled to {}", out_path).green()),
            Err(e) => eprintln!("{}", format!("Failed to write output: {}", e).red()),
        }
    }

    fn report_parse_error(source: &str, error: nom::Err<nom::error::Error<&str>>) {
        eprintln!("{}", "Parse Error:".red().bold());
        eprintln!("  {} {}", "→".red(), format_parse_error(source, error));
    }

    fn report_type_error(_source: &str, error: crate::type_checker::TypeError) {
        eprintln!("{}", "Type Error:".red().bold());
        eprintln!("  {} {}", "→".red(), error);
    }

    /// Generate language server protocol responses
    pub fn lsp_diagnostics(content: &str) -> Vec<LspDiagnostic> {
        Self::lsp_diagnostics_with_path(content, None)
    }

    pub fn lsp_diagnostics_for_path(content: &str, path: &Path) -> Vec<LspDiagnostic> {
        Self::lsp_diagnostics_with_path(content, Some(path))
    }

    fn lsp_diagnostics_with_path(content: &str, path: Option<&Path>) -> Vec<LspDiagnostic> {
        let mut diagnostics = Vec::new();

        // Try to parse
        match parse_program(content) {
            Ok((remaining, ast)) => {
                if !remaining.trim().is_empty() {
                    diagnostics.push(lsp_error(
                        content,
                        format!("Unparsed input: '{}'", remaining),
                    ));
                    return diagnostics;
                }

                let ast = match path {
                    Some(path) => match resolve_program_imports_for_file(ast, path) {
                        Ok(ast) => ast,
                        Err(e) => {
                            diagnostics.push(lsp_error(
                                content,
                                format!("Import resolution error: {}", e),
                            ));
                            return diagnostics;
                        }
                    },
                    None if !ast.imports.is_empty() => {
                        diagnostics.push(lsp_error(
                            content,
                            "Import resolution error: source-level imports require a file path for diagnostics".to_string(),
                        ));
                        return diagnostics;
                    }
                    None => ast,
                };

                // Try to type check
                if let Err(e) = type_check(&ast) {
                    diagnostics.push(lsp_error(content, format!("Type error: {}", e)));
                }
            }
            Err(e) => {
                diagnostics.push(lsp_error(content, format_parse_error(content, e)));
            }
        }

        diagnostics
    }
}

#[derive(Debug)]
pub struct LspDiagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

fn lsp_error(content: &str, message: String) -> LspDiagnostic {
    let (line, column) = diagnostic_position_for_message(content, &message).unwrap_or((0, 0));

    LspDiagnostic {
        severity: DiagnosticSeverity::Error,
        message,
        line,
        column,
    }
}

fn diagnostic_position_for_message(content: &str, message: &str) -> Option<(u32, u32)> {
    if let Some(position) = line_column_from_message(message) {
        return Some(position);
    }

    let binding_name = binding_name_from_message(message)?;
    binding_name_position(content, binding_name)
}

fn line_column_from_message(message: &str) -> Option<(u32, u32)> {
    let line_start = message.find("line ")? + "line ".len();
    let line_end = line_start
        + message[line_start..]
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .map(char::len_utf8)
            .sum::<usize>();
    let line = message[line_start..line_end].parse::<u32>().ok()?;

    let column_start = message[line_end..].find("column ")? + line_end + "column ".len();
    let column_end = column_start
        + message[column_start..]
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .map(char::len_utf8)
            .sum::<usize>();
    let column = message[column_start..column_end].parse::<u32>().ok()?;

    Some((line.saturating_sub(1), column.saturating_sub(1)))
}

fn binding_name_from_message(message: &str) -> Option<&str> {
    let marker = "binding '";
    let start = message.find(marker)? + marker.len();
    let end = message[start..].find('\'')? + start;
    Some(&message[start..end])
}

fn binding_name_position(content: &str, binding_name: &str) -> Option<(u32, u32)> {
    for (line_index, line) in content.lines().enumerate() {
        let Some(pattern_start) = line
            .find("mut val ")
            .map(|index| index + "mut val ".len())
            .or_else(|| line.find("val ").map(|index| index + "val ".len()))
        else {
            continue;
        };
        let Some(relative_start) = line[pattern_start..].find(binding_name) else {
            continue;
        };
        let byte_index = pattern_start + relative_start;
        let column = line[..byte_index].chars().count();

        return Some((line_index as u32, column as u32));
    }

    None
}
