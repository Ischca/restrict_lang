use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use crate::{lex, parse_program, TypeChecker, WasmCodeGen};

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
    let result = compile_internal(source);
    serde_wasm_bindgen::to_value(&result).unwrap_or_else(|_| JsValue::NULL)
}

fn compile_internal(source: &str) -> CompilationResult {
    // Step 1: Lexical analysis
    let tokens = match lex(source) {
        Ok((remaining, tokens)) => {
            if !remaining.is_empty() {
                return CompilationResult {
                    success: false,
                    output: None,
                    error: Some(format!("Lexer error: Unparsed input remaining: {:?}", remaining)),
                    tokens: None,
                    ast: None,
                };
            }
            tokens
        },
        Err(e) => {
            return CompilationResult {
                success: false,
                output: None,
                error: Some(format!("Lexing error: {:?}", e)),
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
                    error: Some(format!("Parser error: Unparsed input remaining: {:?}", remaining)),
                    tokens: Some(tokens_debug),
                    ast: None,
                };
            }
            ast
        },
        Err(e) => {
            return CompilationResult {
                success: false,
                output: None,
                error: Some(format!("Parsing error: {:?}", e)),
                tokens: Some(tokens_debug),
                ast: None,
            };
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

    // Step 4: Code generation
    let mut codegen = WasmCodeGen::new();
    let wat = match codegen.generate(&ast) {
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
    let result = match lex(source) {
        Ok((remaining, tokens)) => {
            if !remaining.is_empty() {
                CompilationResult {
                    success: false,
                    output: None,
                    error: Some(format!("Lexer error: Unparsed input remaining: {:?}", remaining)),
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
        },
        Err(e) => {
            CompilationResult {
                success: false,
                output: None,
                error: Some(format!("Lexing error: {:?}", e)),
                tokens: None,
                ast: None,
            }
        }
    };

    serde_wasm_bindgen::to_value(&result).unwrap_or_else(|_| JsValue::NULL)
}

#[wasm_bindgen]
pub fn parse_only(source: &str) -> JsValue {
    let result = match parse_program(source) {
        Ok((remaining, ast)) => {
            if !remaining.is_empty() {
                CompilationResult {
                    success: false,
                    output: None,
                    error: Some(format!("Parser error: Unparsed input remaining: {:?}", remaining)),
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
        },
        Err(e) => {
            CompilationResult {
                success: false,
                output: None,
                error: Some(format!("Parsing error: {:?}", e)),
                tokens: None,
                ast: None,
            }
        }
    };

    serde_wasm_bindgen::to_value(&result).unwrap_or_else(|_| JsValue::NULL)
}

// Initialize the web module
#[wasm_bindgen(start)]
pub fn init() {
    // Set up panic hook for better error reporting
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}