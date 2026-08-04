use crate::diagnostics::{format_lex_error, format_parse_error};
use crate::ir::builder::build_checked_ir;
use crate::module::resolve_program_imports_with_module_source_map;
use crate::{check_v001_release_surface, lex, parse_program, TypeChecker, WasmCodeGen};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

#[derive(Serialize, Deserialize)]
pub struct CompilationResult {
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub tokens: Option<String>,
    pub ast: Option<String>,
}

#[wasm_bindgen]
pub fn compile_restrict_lang(source: &str) -> JsValue {
    let result = compile_internal(source, None);
    serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
}

/// Convert compiler-generated WebAssembly text into an executable binary.
///
/// Keeping this conversion in the compiler bundle lets the playground run
/// programs locally without loading a third-party WAT parser from a CDN.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn wat_to_wasm(wat_source: &str) -> Result<Vec<u8>, JsValue> {
    wat_to_wasm_internal(wat_source).map_err(|error| JsValue::from_str(&error))
}

#[cfg(any(test, target_arch = "wasm32"))]
fn wat_to_wasm_internal(wat_source: &str) -> Result<Vec<u8>, String> {
    wat::parse_str(wat_source).map_err(|error| format!("WebAssembly assembly error: {error}"))
}

#[wasm_bindgen]
pub fn compile_restrict_lang_with_modules(source: &str, modules: JsValue) -> JsValue {
    let module_sources = match serde_wasm_bindgen::from_value::<HashMap<String, String>>(modules) {
        Ok(module_sources) => module_sources,
        Err(e) => {
            let result = CompilationResult {
                success: false,
                output: None,
                error: Some(format!("Module source map error: {}", e)),
                tokens: None,
                ast: None,
            };
            return serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL);
        }
    };

    let result = compile_internal(source, Some(module_sources));
    serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
}

fn compile_internal(
    source: &str,
    module_sources: Option<HashMap<String, String>>,
) -> CompilationResult {
    // Step 1: Lexical analysis
    let tokens = match lex(source) {
        Ok((remaining, tokens)) => {
            if !remaining.is_empty() {
                return CompilationResult {
                    success: false,
                    output: None,
                    error: Some(format_lex_unparsed_input(source, remaining)),
                    tokens: None,
                    ast: None,
                };
            }
            tokens
        }
        Err(e) => {
            return CompilationResult {
                success: false,
                output: None,
                error: Some(format_lex_error(source, e)),
                tokens: None,
                ast: None,
            };
        }
    };

    let tokens_debug = format!("{:#?}", tokens);

    // Step 2: Parsing
    let ast = match parse_program(source) {
        Ok((remaining, ast)) => {
            if !remaining.is_empty() {
                return CompilationResult {
                    success: false,
                    output: None,
                    error: Some(format_parse_unparsed_input(source, remaining)),
                    tokens: Some(tokens_debug),
                    ast: None,
                };
            }
            ast
        }
        Err(e) => {
            return CompilationResult {
                success: false,
                output: None,
                error: Some(format_parse_error(source, e)),
                tokens: Some(tokens_debug),
                ast: None,
            };
        }
    };

    let ast = if ast.imports.is_empty() {
        ast
    } else {
        match module_sources {
            Some(module_sources) => {
                match resolve_program_imports_with_module_source_map(ast, module_sources) {
                    Ok(resolved) => resolved,
                    Err(e) => {
                        return CompilationResult {
                            success: false,
                            output: None,
                            error: Some(format!("Import resolution error: {}", e)),
                            tokens: Some(tokens_debug),
                            ast: None,
                        };
                    }
                }
            }
            None => {
                return CompilationResult {
                    success: false,
                    output: None,
                    error: Some(
                        "Import resolution error: source-level imports require module sources in the browser compiler".to_string(),
                    ),
                    tokens: Some(tokens_debug),
                    ast: None,
                };
            }
        }
    };

    let ast_debug = format!("{:#?}", ast);

    // Step 3: Type checking
    let mut type_checker = TypeChecker::new();
    if let Err(e) = type_checker.check_program(&ast) {
        return CompilationResult {
            success: false,
            output: None,
            error: Some(format!("Type error: {}", e)),
            tokens: Some(tokens_debug),
            ast: Some(ast_debug),
        };
    }

    if let Err(e) = check_v001_release_surface(&ast, &type_checker) {
        return CompilationResult {
            success: false,
            output: None,
            error: Some(format!("Release surface error: {}", e)),
            tokens: Some(tokens_debug),
            ast: Some(ast_debug),
        };
    }

    let checked_ir = match build_checked_ir(&ast, &type_checker) {
        Ok(checked_ir) => checked_ir,
        Err(_) => {
            return CompilationResult {
                success: false,
                output: None,
                error: Some(
                    "Code generation error: Invalid checked compiler state: construction failed"
                        .to_string(),
                ),
                tokens: Some(tokens_debug),
                ast: Some(ast_debug),
            };
        }
    };

    // Step 4: Code generation
    let mut codegen = WasmCodeGen::new();
    let wat = match codegen.generate_checked(&ast, &checked_ir) {
        Ok(wat) => wat,
        Err(e) => {
            return CompilationResult {
                success: false,
                output: None,
                error: Some(format!("Code generation error: {}", e)),
                tokens: Some(tokens_debug),
                ast: Some(ast_debug),
            };
        }
    };

    CompilationResult {
        success: true,
        output: Some(wat),
        error: None,
        tokens: Some(tokens_debug),
        ast: Some(ast_debug),
    }
}

#[wasm_bindgen]
pub fn lex_only(source: &str) -> JsValue {
    let result = lex_only_internal(source);
    serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
}

fn lex_only_internal(source: &str) -> CompilationResult {
    let result = match lex(source) {
        Ok((remaining, tokens)) => {
            if !remaining.is_empty() {
                CompilationResult {
                    success: false,
                    output: None,
                    error: Some(format_lex_unparsed_input(source, remaining)),
                    tokens: None,
                    ast: None,
                }
            } else {
                CompilationResult {
                    success: true,
                    output: None,
                    error: None,
                    tokens: Some(format!("{:#?}", tokens)),
                    ast: None,
                }
            }
        }
        Err(e) => CompilationResult {
            success: false,
            output: None,
            error: Some(format_lex_error(source, e)),
            tokens: None,
            ast: None,
        },
    };

    result
}

#[wasm_bindgen]
pub fn parse_only(source: &str) -> JsValue {
    let result = parse_only_internal(source);
    serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
}

fn parse_only_internal(source: &str) -> CompilationResult {
    let result = match parse_program(source) {
        Ok((remaining, ast)) => {
            if !remaining.is_empty() {
                CompilationResult {
                    success: false,
                    output: None,
                    error: Some(format_parse_unparsed_input(source, remaining)),
                    tokens: None,
                    ast: None,
                }
            } else {
                CompilationResult {
                    success: true,
                    output: None,
                    error: None,
                    tokens: None,
                    ast: Some(format!("{:#?}", ast)),
                }
            }
        }
        Err(e) => CompilationResult {
            success: false,
            output: None,
            error: Some(format_parse_error(source, e)),
            tokens: None,
            ast: None,
        },
    };

    result
}

fn format_lex_unparsed_input(source: &str, remaining: &str) -> String {
    format_lex_error(source, nom_error_at(remaining))
}

fn format_parse_unparsed_input(source: &str, remaining: &str) -> String {
    format_parse_error(source, nom_error_at(remaining))
}

fn nom_error_at(input: &str) -> nom::Err<nom::error::Error<&str>> {
    nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag))
}

// Initialize the web module
#[wasm_bindgen(start)]
pub fn init() {
    // Set up panic hook for better error reporting
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wat_to_wasm_internal_emits_executable_binary() {
        let wasm = wat_to_wasm_internal("(module (func (export \"main\")))")
            .expect("valid WAT should assemble");

        assert_eq!(&wasm[..4], b"\0asm");
    }

    #[test]
    fn wat_to_wasm_internal_reports_assembly_errors() {
        let message = wat_to_wasm_internal("(module (func")
            .expect_err("invalid WAT should report an assembly error");

        assert!(message.starts_with("WebAssembly assembly error:"));
    }

    #[test]
    fn compile_internal_uses_checked_forward_float64_signature() {
        let source = r#"
fun adjusted: (value: Float64) = {
    value |> risk
}

fun risk: (value: Float64) = {
    value + 0.5
}

fun main: () -> Float64 = {
    41.5 |> adjusted
}
"#;
        let result = compile_internal(source, None);
        assert!(result.success, "browser compile failed: {:?}", result.error);
        let wat = result.output.expect("successful compile should return WAT");
        assert!(wat.contains("(func $adjusted (param $value f64) (result f64)"));
        wat_to_wasm_internal(&wat).expect("browser WAT should assemble");
    }

    #[test]
    fn compile_internal_uses_checked_ir_after_module_resolution() {
        let source = r#"
import release.{adjusted}

fun main: () -> Float64 = {
    41.5 |> adjusted
}
"#;
        let mut modules = HashMap::new();
        modules.insert(
            "release".to_string(),
            r#"
export fun adjusted: (value: Float64) = {
    value + 0.5
}
"#
            .to_string(),
        );

        let result = compile_internal(source, Some(modules));
        assert!(result.success, "browser compile failed: {:?}", result.error);
        let wat = result.output.expect("successful compile should return WAT");
        assert!(wat.contains("(func $adjusted (param $value f64) (result f64)"));
        wat_to_wasm_internal(&wat).expect("resolved browser WAT should assemble");
    }

    #[test]
    fn compile_internal_applies_release_surface_before_checked_codegen() {
        let source = r#"
export fun identity: <T>(value: T) -> T = {
    value
}
"#;
        let result = compile_internal(source, None);
        let message = result.error.expect("generic export should fail");

        assert!(!result.success);
        assert!(message.starts_with("Release surface error:"), "{message}");
        assert!(
            !message.contains("Invalid checked compiler state"),
            "{message}"
        );
    }

    #[test]
    fn compile_internal_formats_lex_errors_without_nom_debug() {
        let result = compile_internal("val x = |>>", None);
        let message = result.error.expect("invalid source should fail");

        assert!(!result.success);
        assert!(message.contains("Lexing error at line 1, column 9"));
        assert!(message.contains("unexpected input near `|>>`"));
        assert_no_raw_nom_debug(&message);
    }

    #[test]
    fn compile_internal_formats_parse_errors_without_nom_debug() {
        let source = "fun main: () -> Int32 = {\n    val answer =\n}\n";
        let result = compile_internal(source, None);
        let message = result.error.expect("invalid source should fail");

        assert!(!result.success);
        assert!(message.contains("Parsing error at line"));
        assert!(message.contains("column"));
        assert_no_raw_nom_debug(&message);
    }

    #[test]
    fn lex_only_formats_leftover_input_without_debug_remaining() {
        let result = lex_only_internal("val x = 1\nval y = @");
        let message = result.error.expect("unknown token should fail");

        assert!(!result.success);
        assert!(message.contains("Lexing error at line 2, column 9"));
        assert!(message.contains("unexpected input near `@`"));
        assert_no_raw_nom_debug(&message);
    }

    #[test]
    fn parse_only_formats_errors_without_nom_debug() {
        let result = parse_only_internal("fun main: () -> Int32 = {\n    val answer =\n}\n");
        let message = result.error.expect("invalid source should fail");

        assert!(!result.success);
        assert!(message.contains("Parsing error at line"));
        assert!(message.contains("column"));
        assert_no_raw_nom_debug(&message);
    }

    fn assert_no_raw_nom_debug(message: &str) {
        for internal in ["Error(", "Failure(", "ErrorKind", "nom"] {
            assert!(
                !message.contains(internal),
                "diagnostic should not expose parser internals ({internal}): {message}"
            );
        }
        assert!(
            !message.contains("Unparsed input remaining"),
            "diagnostic should not expose debug-style remaining input text: {message}"
        );
    }
}
