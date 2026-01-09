use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use crate::{lex, parse_program, TypeChecker, WasmCodeGen};
use crate::parser::parse_program_recovering;
use crate::diagnostic::{Diagnostic, DiagnosticBag, RenderConfig};
use crate::type_checker::{SymbolInfo, SymbolKind, format_typed_type, TypedType};
use crate::lexer::Span;

#[derive(Serialize, Deserialize)]
pub struct CompilationResult {
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub tokens: Option<String>,
    pub ast: Option<String>,
}

/// Rich error information for editor integration
#[derive(Serialize, Deserialize)]
pub struct RichError {
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub message: String,
    pub code: Option<String>,
    pub severity: String,  // "error", "warning", "info", "hint"
    pub notes: Vec<String>,
    pub help: Vec<String>,
}

/// Compilation result with rich error information
#[derive(Serialize, Deserialize)]
pub struct RichCompilationResult {
    pub success: bool,
    pub output: Option<String>,
    pub errors: Vec<RichError>,
    pub tokens: Option<String>,
    pub ast: Option<String>,
}

/// Inlay hint for editor display
#[derive(Serialize, Deserialize)]
pub struct WebInlayHint {
    pub line: u32,
    pub column: u32,
    pub label: String,
    pub kind: String,  // "type", "parameter", "affine"
    pub tooltip: Option<String>,
}

/// Symbol information for hover/go-to-definition
#[derive(Serialize, Deserialize)]
pub struct WebSymbol {
    pub name: String,
    pub kind: String,  // "variable", "function", "parameter", "record"
    pub type_str: String,
    pub line: u32,
    pub column: u32,
    pub mutable: bool,
    pub used: bool,
}

/// Semantic token for syntax highlighting
#[derive(Serialize, Deserialize)]
pub struct SemanticToken {
    pub line: u32,
    pub column: u32,
    pub length: u32,
    pub token_type: String,  // "keyword", "function", "variable", "type", "number", "string", "operator", "comment"
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

// ============================================================================
// Rich Compilation API (with line numbers and error details)
// ============================================================================

/// Compile with rich error information
#[wasm_bindgen]
pub fn compile_with_diagnostics(source: &str) -> JsValue {
    let result = compile_rich_internal(source);
    serde_wasm_bindgen::to_value(&result).unwrap_or_else(|_| JsValue::NULL)
}

fn compile_rich_internal(source: &str) -> RichCompilationResult {
    let mut errors = Vec::new();

    // Step 1: Lex with span information
    let lex_result = crate::lexer::lex_spanned_tokens(source);
    if let Err((msg, span)) = &lex_result {
        let (line, col) = span.to_line_col(source);
        let end_span = Span::new(span.end, span.end);
        let (end_line, end_col) = end_span.to_line_col(source);
        errors.push(RichError {
            line: line as u32,
            column: col as u32,
            end_line: end_line as u32,
            end_column: end_col as u32,
            message: format!("Lexer error: {}", msg),
            code: Some("L0001".to_string()),
            severity: "error".to_string(),
            notes: vec![],
            help: vec![],
        });
        return RichCompilationResult {
            success: false,
            output: None,
            errors,
            tokens: None,
            ast: None,
        };
    }

    // Step 2: Parse with error recovery
    let parse_result = parse_program_recovering(source);

    // Collect parse errors
    for error in &parse_result.errors {
        let (line, col) = error.span.to_line_col(source);
        let end_span = Span::new(error.span.end, error.span.end);
        let (end_line, end_col) = end_span.to_line_col(source);
        errors.push(RichError {
            line: line as u32,
            column: col as u32,
            end_line: end_line as u32,
            end_column: end_col as u32,
            message: error.message.clone(),
            code: Some("P0001".to_string()),
            severity: "error".to_string(),
            notes: vec![],
            help: vec![],
        });
    }

    let ast_debug = format!("{:#?}", parse_result.program);

    // Step 3: Type check with error collection
    let mut type_checker = TypeChecker::new();
    let type_errors = type_checker.check_program_collecting(&parse_result.program);

    for type_error in type_errors {
        let diag = type_error.to_diagnostic();
        let (line, col, end_line, end_col) = if let Some(span) = type_error.span {
            let (l, c) = span.to_line_col(source);
            let end_span = Span::new(span.end, span.end);
            let (el, ec) = end_span.to_line_col(source);
            (l as u32, c as u32, el as u32, ec as u32)
        } else {
            (0, 0, 0, 1)
        };

        errors.push(RichError {
            line,
            column: col,
            end_line,
            end_column: end_col,
            message: diag.message.clone(),
            code: diag.code.clone(),
            severity: match diag.severity {
                crate::diagnostic::Severity::Error => "error",
                crate::diagnostic::Severity::Warning => "warning",
                crate::diagnostic::Severity::Note => "info",
                crate::diagnostic::Severity::Help => "hint",
            }.to_string(),
            notes: diag.notes.clone(),
            help: diag.help.clone(),
        });
    }

    if !errors.is_empty() {
        return RichCompilationResult {
            success: false,
            output: None,
            errors,
            tokens: None,
            ast: Some(ast_debug),
        };
    }

    // Step 4: Code generation
    let mut codegen = WasmCodeGen::new();
    match codegen.generate(&parse_result.program) {
        Ok(wat) => RichCompilationResult {
            success: true,
            output: Some(wat),
            errors: vec![],
            tokens: None,
            ast: Some(ast_debug),
        },
        Err(e) => {
            errors.push(RichError {
                line: 0,
                column: 0,
                end_line: 0,
                end_column: 1,
                message: format!("Code generation error: {}", e),
                code: Some("G0001".to_string()),
                severity: "error".to_string(),
                notes: vec![],
                help: vec![],
            });
            RichCompilationResult {
                success: false,
                output: None,
                errors,
                tokens: None,
                ast: Some(ast_debug),
            }
        }
    }
}

// ============================================================================
// Inlay Hints API
// ============================================================================

/// Get inlay hints for the source code
#[wasm_bindgen]
pub fn get_inlay_hints(source: &str) -> JsValue {
    let hints = get_inlay_hints_internal(source);
    serde_wasm_bindgen::to_value(&hints).unwrap_or_else(|_| JsValue::NULL)
}

fn get_inlay_hints_internal(source: &str) -> Vec<WebInlayHint> {
    let mut hints = Vec::new();

    let parse_result = parse_program_recovering(source);
    let mut type_checker = TypeChecker::new();
    let _ = type_checker.check_program_collecting(&parse_result.program);

    for symbol in type_checker.symbols() {
        if let Some(span) = symbol.def_span {
            // Only show hints for variables and parameters
            if symbol.kind == SymbolKind::Variable || symbol.kind == SymbolKind::Parameter {
                let (line, col) = span.to_line_col(source);

                // Type hint
                hints.push(WebInlayHint {
                    line: line as u32,
                    column: (col + symbol.name.len()) as u32,
                    label: format!(": {}", symbol.type_display()),
                    kind: "type".to_string(),
                    tooltip: Some(format!("Type: {}", symbol.type_display())),
                });

                // Mutability hint
                if symbol.mutable {
                    hints.push(WebInlayHint {
                        line: line as u32,
                        column: (col + symbol.name.len()) as u32,
                        label: " (mut)".to_string(),
                        kind: "parameter".to_string(),
                        tooltip: Some("This binding is mutable".to_string()),
                    });
                }

                // Affine status hint for non-copy types
                if !is_copy_type(&symbol.ty) {
                    let (label, tooltip) = if symbol.used {
                        (" [consumed]", "This value has been consumed (affine type)")
                    } else {
                        (" [available]", "This value is still available for use")
                    };
                    hints.push(WebInlayHint {
                        line: line as u32,
                        column: (col + symbol.name.len()) as u32,
                        label: label.to_string(),
                        kind: "affine".to_string(),
                        tooltip: Some(tooltip.to_string()),
                    });
                }
            }
        }
    }

    hints
}

fn is_copy_type(ty: &TypedType) -> bool {
    matches!(ty,
        TypedType::Int32 |
        TypedType::Float64 |
        TypedType::Boolean |
        TypedType::Char |
        TypedType::Unit
    )
}

// ============================================================================
// Symbol Information API
// ============================================================================

/// Get all symbols in the source code
#[wasm_bindgen]
pub fn get_symbols(source: &str) -> JsValue {
    let symbols = get_symbols_internal(source);
    serde_wasm_bindgen::to_value(&symbols).unwrap_or_else(|_| JsValue::NULL)
}

fn get_symbols_internal(source: &str) -> Vec<WebSymbol> {
    let parse_result = parse_program_recovering(source);
    let mut type_checker = TypeChecker::new();
    let _ = type_checker.check_program_collecting(&parse_result.program);

    type_checker.symbols().iter().map(|s| {
        let (line, column) = s.def_span
            .map(|span| span.to_line_col(source))
            .unwrap_or((0, 0));

        WebSymbol {
            name: s.name.clone(),
            kind: match s.kind {
                SymbolKind::Variable => "variable",
                SymbolKind::Parameter => "parameter",
                SymbolKind::Function => "function",
                SymbolKind::Record => "record",
                SymbolKind::Field => "field",
            }.to_string(),
            type_str: s.type_display(),
            line: line as u32,
            column: column as u32,
            mutable: s.mutable,
            used: s.used,
        }
    }).collect()
}

// ============================================================================
// Semantic Tokens API (for syntax highlighting)
// ============================================================================

/// Get semantic tokens for syntax highlighting
#[wasm_bindgen]
pub fn get_semantic_tokens(source: &str) -> JsValue {
    let tokens = get_semantic_tokens_internal(source);
    serde_wasm_bindgen::to_value(&tokens).unwrap_or_else(|_| JsValue::NULL)
}

fn get_semantic_tokens_internal(source: &str) -> Vec<SemanticToken> {
    use crate::lexer::{lex_spanned_tokens, Token};

    let mut tokens = Vec::new();

    if let Ok(spanned_tokens) = lex_spanned_tokens(source) {
        for st in spanned_tokens {
            let (line, col) = st.span.to_line_col(source);
            let length = st.span.len() as u32;

            let token_type = match &st.token {
                // Keywords
                Token::Fun | Token::Val | Token::Mut | Token::Record |
                Token::Then | Token::Else | Token::While | Token::Match |
                Token::With | Token::Clone | Token::Freeze | Token::Import |
                Token::From | Token::Context | Token::Impl | Token::Async |
                Token::Await | Token::Return | Token::Sealed | Token::Export |
                Token::Where | Token::Spawn => "keyword",

                // Literals
                Token::IntLit(_) | Token::FloatLit(_) => "number",
                Token::StringLit(_) => "string",
                Token::CharLit(_) => "string",
                Token::True | Token::False => "keyword",

                // Unit type
                Token::Unit => "type",

                // Identifiers - check if starts with uppercase for type
                Token::Ident(name) => {
                    if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                        "type"
                    } else {
                        "variable"
                    }
                },

                // Operators
                Token::Pipe | Token::PipeMut | Token::Arrow | Token::ThinArrow |
                Token::Plus | Token::Minus | Token::Star | Token::Slash | Token::Percent |
                Token::Assign | Token::Eq | Token::Ne | Token::Lt | Token::Gt |
                Token::Le | Token::Ge | Token::Tilde |
                Token::Dot | Token::Colon | Token::Comma | Token::Semicolon => "operator",

                // Option types
                Token::Some | Token::None => "keyword",

                // Lifetime/temporal
                Token::Lifetime | Token::Within => "keyword",

                // Brackets
                Token::LParen | Token::RParen | Token::LBrace | Token::RBrace |
                Token::LBracket | Token::RBracket | Token::LArrayBracket |
                Token::RArrayBracket | Token::Bar => "operator",

                // Special tokens
                Token::It => "keyword",
                Token::Asterisk => "operator",
                Token::Eof => continue,  // Skip EOF token
            };

            tokens.push(SemanticToken {
                line: line as u32,
                column: col as u32,
                length,
                token_type: token_type.to_string(),
            });
        }
    }

    tokens
}

// ============================================================================
// Formatted Error Output (Rust-style)
// ============================================================================

/// Get Rust-style formatted error output
#[wasm_bindgen]
pub fn get_formatted_errors(source: &str) -> String {
    let parse_result = parse_program_recovering(source);
    let mut bag = DiagnosticBag::new();

    // Add parse errors
    for error in &parse_result.errors {
        bag.add(
            Diagnostic::error(&error.message)
                .with_code("P0001")
                .with_label(error.span, "")
        );
    }

    // Type check and add type errors
    let mut type_checker = TypeChecker::new();
    let type_errors = type_checker.check_program_collecting(&parse_result.program);

    for type_error in type_errors {
        bag.add(type_error.to_diagnostic());
    }

    // Render without colors for web
    bag.render(source, RenderConfig::no_colors())
}

// ============================================================================
// Type Check Only (no code generation)
// ============================================================================

/// Type check only, returning symbol information
#[wasm_bindgen]
pub fn type_check_only(source: &str) -> JsValue {
    #[derive(Serialize)]
    struct TypeCheckResult {
        success: bool,
        errors: Vec<RichError>,
        symbols: Vec<WebSymbol>,
    }

    let parse_result = parse_program_recovering(source);
    let mut type_checker = TypeChecker::new();
    let type_errors = type_checker.check_program_collecting(&parse_result.program);

    let mut errors = Vec::new();

    // Collect parse errors
    for error in &parse_result.errors {
        let (line, col) = error.span.to_line_col(source);
        let end_span = Span::new(error.span.end, error.span.end);
        let (end_line, end_col) = end_span.to_line_col(source);
        errors.push(RichError {
            line: line as u32,
            column: col as u32,
            end_line: end_line as u32,
            end_column: end_col as u32,
            message: error.message.clone(),
            code: Some("P0001".to_string()),
            severity: "error".to_string(),
            notes: vec![],
            help: vec![],
        });
    }

    // Collect type errors
    for type_error in type_errors {
        let diag = type_error.to_diagnostic();
        let (line, col, end_line, end_col) = if let Some(span) = type_error.span {
            let (l, c) = span.to_line_col(source);
            let end_span = Span::new(span.end, span.end);
            let (el, ec) = end_span.to_line_col(source);
            (l as u32, c as u32, el as u32, ec as u32)
        } else {
            (0, 0, 0, 1)
        };

        errors.push(RichError {
            line,
            column: col,
            end_line,
            end_column: end_col,
            message: diag.message.clone(),
            code: diag.code.clone(),
            severity: "error".to_string(),
            notes: diag.notes.clone(),
            help: diag.help.clone(),
        });
    }

    let symbols = get_symbols_internal(source);

    let result = TypeCheckResult {
        success: errors.is_empty(),
        errors,
        symbols,
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