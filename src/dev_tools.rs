use std::fs;
use std::path::Path;
use colored::*;
use crate::parser::parse_program;
use crate::type_checker::type_check;
use crate::codegen::WasmCodeGen;

pub struct DevTools;

impl DevTools {
    /// Watch mode: automatically recompile on file changes
    pub fn watch(path: &str) -> Result<(), Box<dyn std::error::Error>> {
        use notify::{Watcher, RecursiveMode, Event, EventKind};
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
            eprintln!("{}", format!("Warning: Unparsed input: '{}'", remaining).yellow());
        }
        
        // Type check
        match type_check(&ast) {
            Ok(()) => {},
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
                eprintln!("{}", format!("Code generation error: {:?}", e).red());
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
        
        match error {
            nom::Err::Error(e) | nom::Err::Failure(e) => {
                if let Some(pos) = source.find(e.input) {
                    let line_num = source[..pos].lines().count();
                    let line_start = source[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
                    let col = pos - line_start + 1;
                    
                    eprintln!("  {} at line {}, column {}", "→".red(), line_num, col);
                    
                    // Show the problematic line
                    if let Some(line) = source.lines().nth(line_num - 1) {
                        eprintln!("  {}", line);
                        eprintln!("  {}{}", " ".repeat(col - 1), "^".red());
                    }
                }
            }
            _ => {}
        }
    }
    
    fn report_type_error(_source: &str, error: crate::type_checker::TypeError) {
        eprintln!("{}", "Type Error:".red().bold());
        eprintln!("  {} {:?}", "→".red(), error);
    }
    
    /// Generate language server protocol responses
    pub fn lsp_diagnostics(content: &str) -> Vec<LspDiagnostic> {
        let mut diagnostics = Vec::new();
        
        // Try to parse
        match parse_program(content) {
            Ok((remaining, ast)) => {
                if !remaining.trim().is_empty() {
                    diagnostics.push(LspDiagnostic {
                        severity: DiagnosticSeverity::Warning,
                        message: format!("Unparsed input: '{}'", remaining),
                        line: content.lines().count() as u32,
                        column: 0,
                    });
                }
                
                // Try to type check
                if let Err(e) = type_check(&ast) {
                    diagnostics.push(LspDiagnostic {
                        severity: DiagnosticSeverity::Error,
                        message: format!("Type error: {:?}", e),
                        line: 0, // Would need better position tracking
                        column: 0,
                    });
                }
            }
            Err(e) => {
                diagnostics.push(LspDiagnostic {
                    severity: DiagnosticSeverity::Error,
                    message: format!("Parse error: {:?}", e),
                    line: 0,
                    column: 0,
                });
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