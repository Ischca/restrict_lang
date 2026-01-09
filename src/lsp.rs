use std::collections::HashMap;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use crate::{lex, parse_program, TypeChecker};
use crate::lexer::Span;
use crate::parser::{parse_program_with_errors, parse_program_recovering};

#[derive(Debug)]
pub struct RestrictLanguageServer {
    client: Client,
    documents: std::sync::RwLock<HashMap<Url, String>>,
}

impl RestrictLanguageServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: std::sync::RwLock::new(HashMap::new()),
        }
    }

    fn get_diagnostics(&self, _uri: &Url, text: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Lexing with span information
        match crate::lexer::lex_spanned_tokens(text) {
            Ok(_tokens) => {
                // Lexing succeeded
            }
            Err((msg, span)) => {
                let range = self.span_to_range(text, &span);
                diagnostics.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: None,
                    code_description: None,
                    source: Some("restrict-lang".to_string()),
                    message: format!("Lexer error: {}", msg),
                    related_information: None,
                    tags: None,
                    data: None,
                });
                return diagnostics;
            }
        }

        // Use error-recovering parser to get all syntax errors
        let parse_result = parse_program_recovering(text);

        // Add all parse errors as diagnostics
        for error in &parse_result.errors {
            let range = self.span_to_range(text, &error.span);
            diagnostics.push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                code: None,
                code_description: None,
                source: Some("restrict-lang".to_string()),
                message: error.message.clone(),
                related_information: None,
                tags: None,
                data: None,
            });
        }

        // Type check the (possibly partial) AST and collect all errors with spans
        let mut type_checker = TypeChecker::new();
        let type_errors = type_checker.check_program_collecting(&parse_result.program);

        for type_error in type_errors {
            let range = if let Some(span) = type_error.span {
                self.span_to_range(text, &span)
            } else {
                // Fallback to beginning of file if no span available
                Range::new(Position::new(0, 0), Position::new(0, 10))
            };

            diagnostics.push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                code: None,
                code_description: None,
                source: Some("restrict-lang".to_string()),
                message: format!("Type error: {}", type_error.error),
                related_information: None,
                tags: None,
                data: None,
            });
        }

        diagnostics
    }

    /// Converts a byte-offset Span to an LSP Range (line/column).
    fn span_to_range(&self, text: &str, span: &Span) -> Range {
        let (start_line, start_col) = span.to_line_col(text);
        let end_span = Span::new(span.end, span.end);
        let (end_line, end_col) = end_span.to_line_col(text);

        Range::new(
            Position::new(start_line as u32, start_col as u32),
            Position::new(end_line as u32, end_col as u32),
        )
    }

    fn find_definition_at_position(&self, ast: &crate::ast::Program, text: &str, position: &Position) -> Option<Location> {
        // Simple implementation: find function definitions
        let lines: Vec<&str> = text.lines().collect();
        let target_line_idx = position.line as usize;
        
        if target_line_idx >= lines.len() {
            return None;
        }
        
        let target_line = lines[target_line_idx];
        let char_idx = position.character as usize;
        
        if char_idx >= target_line.len() {
            return None;
        }
        
        // Extract word at cursor position
        let (start, end) = self.extract_word_at_position(target_line, char_idx)?;
        let word = &target_line[start..end];
        
        // Search for function definitions
        for (line_idx, line) in lines.iter().enumerate() {
            if line.contains("fun ") && line.contains(word) {
                // Simple heuristic: if the line contains "fun" and our word
                if let Some(pos) = line.find(word) {
                    return Some(Location::new(
                        Url::parse("file://current").unwrap(), // This should be the actual URI
                        Range::new(
                            Position::new(line_idx as u32, pos as u32),
                            Position::new(line_idx as u32, (pos + word.len()) as u32),
                        ),
                    ));
                }
            }
        }
        
        None
    }

    fn get_hover_info(&self, _ast: &crate::ast::Program, text: &str, position: &Position) -> Option<String> {
        let lines: Vec<&str> = text.lines().collect();
        let target_line_idx = position.line as usize;
        
        if target_line_idx >= lines.len() {
            return None;
        }
        
        let target_line = lines[target_line_idx];
        let char_idx = position.character as usize;
        
        if char_idx >= target_line.len() {
            return None;
        }
        
        // Extract word at cursor position
        if let Some((start, end)) = self.extract_word_at_position(target_line, char_idx) {
            let word = &target_line[start..end];
            
            // Provide hover information based on word
            match word {
                "fun" => Some("**fun** - Function definition keyword\n\nDefines a new function.".to_string()),
                "val" => Some("**val** - Immutable variable binding\n\nCreates an immutable binding.".to_string()),
                "mut" => Some("**mut** - Mutable variable modifier\n\nMakes a variable mutable.".to_string()),
                "then" => Some("**then** - Conditional expression\n\nConditional branch keyword.".to_string()),
                "else" => Some("**else** - Alternative branch\n\nAlternative branch in conditional.".to_string()),
                "while" => Some("**while** - Loop expression\n\nLoop construct.".to_string()),
                "with" => Some("**with** - Context binding\n\nBinds context variables.".to_string()),
                "clone" => Some("**clone** - Object cloning\n\nCreates a copy of an object.".to_string()),
                "freeze" => Some("**freeze** - Object freezing\n\nMakes an object immutable.".to_string()),
                "true" | "false" => Some(format!("**{}** - Boolean literal\n\nType: Bool", word)),
                _ => {
                    // Check if it's a standard library function
                    match word {
                        "println" => Some("**println(s: String)** -> Unit\n\nPrints a string followed by a newline.".to_string()),
                        "abs" => Some("**abs(x: Int)** -> Int\n\nReturns the absolute value of an integer.".to_string()),
                        "max" => Some("**max(a: Int, b: Int)** -> Int\n\nReturns the maximum of two integers.".to_string()),
                        "min" => Some("**min(a: Int, b: Int)** -> Int\n\nReturns the minimum of two integers.".to_string()),
                        "list_head" => Some("**list_head<T>(list: List<T>)** -> Option<T>\n\nReturns the first element of a list.".to_string()),
                        "list_tail" => Some("**list_tail<T>(list: List<T>)** -> Option<List<T>>\n\nReturns the tail of a list.".to_string()),
                        "option_unwrap_or" => Some("**option_unwrap_or<T>(opt: Option<T>, default: T)** -> T\n\nUnwraps an Option or returns a default value.".to_string()),
                        _ => Some(format!("Symbol: **{}**", word)),
                    }
                }
            }
        } else {
            None
        }
    }

    fn extract_document_symbols(&self, ast: &crate::ast::Program, text: &str) -> Vec<DocumentSymbol> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = text.lines().collect();
        
        // Extract symbols from AST
        for decl in &ast.declarations {
            match decl {
                crate::ast::TopDecl::Function(func) => {
                    // Find the function in the text to get its position
                    for (line_idx, line) in lines.iter().enumerate() {
                        if line.contains(&format!("fun {}", func.name)) {
                            let start_pos = line.find(&func.name).unwrap_or(0);
                            symbols.push(DocumentSymbol {
                                name: func.name.clone(),
                                detail: Some(format!("Function with {} parameters", func.params.len())),
                                kind: SymbolKind::FUNCTION,
                                tags: None,
                                deprecated: None,
                                range: Range::new(
                                    Position::new(line_idx as u32, 0),
                                    Position::new(line_idx as u32, line.len() as u32),
                                ),
                                selection_range: Range::new(
                                    Position::new(line_idx as u32, start_pos as u32),
                                    Position::new(line_idx as u32, (start_pos + func.name.len()) as u32),
                                ),
                                children: None,
                            });
                            break;
                        }
                    }
                }
                crate::ast::TopDecl::Record(record) => {
                    for (line_idx, line) in lines.iter().enumerate() {
                        if line.contains(&format!("record {}", record.name)) {
                            let start_pos = line.find(&record.name).unwrap_or(0);
                            symbols.push(DocumentSymbol {
                                name: record.name.clone(),
                                detail: Some(format!("Record with {} fields", record.fields.len())),
                                kind: SymbolKind::STRUCT,
                                tags: None,
                                deprecated: None,
                                range: Range::new(
                                    Position::new(line_idx as u32, 0),
                                    Position::new(line_idx as u32, line.len() as u32),
                                ),
                                selection_range: Range::new(
                                    Position::new(line_idx as u32, start_pos as u32),
                                    Position::new(line_idx as u32, (start_pos + record.name.len()) as u32),
                                ),
                                children: None,
                            });
                            break;
                        }
                    }
                }
                crate::ast::TopDecl::Binding(binding) => {
                    for (line_idx, line) in lines.iter().enumerate() {
                        if line.contains(&format!("val {}", binding.name)) || line.contains(&format!("mut val {}", binding.name)) {
                            let start_pos = line.find(&binding.name).unwrap_or(0);
                            symbols.push(DocumentSymbol {
                                name: binding.name.clone(),
                                detail: Some(if binding.mutable { "Mutable variable".to_string() } else { "Immutable variable".to_string() }),
                                kind: SymbolKind::VARIABLE,
                                tags: None,
                                deprecated: None,
                                range: Range::new(
                                    Position::new(line_idx as u32, 0),
                                    Position::new(line_idx as u32, line.len() as u32),
                                ),
                                selection_range: Range::new(
                                    Position::new(line_idx as u32, start_pos as u32),
                                    Position::new(line_idx as u32, (start_pos + binding.name.len()) as u32),
                                ),
                                children: None,
                            });
                            break;
                        }
                    }
                }
                _ => {} // Handle other declaration types
            }
        }
        
        symbols
    }

    fn extract_word_at_position(&self, line: &str, char_idx: usize) -> Option<(usize, usize)> {
        let chars: Vec<char> = line.chars().collect();
        
        if char_idx >= chars.len() {
            return None;
        }
        
        // Find word boundaries
        let mut start = char_idx;
        let mut end = char_idx;
        
        // Expand backwards
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }
        
        // Expand forwards
        while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
            end += 1;
        }
        
        if start == end {
            return None;
        }
        
        Some((start, end))
    }

    fn find_references_at_position(&self, ast: &crate::ast::Program, text: &str, position: &Position, include_declaration: bool) -> Vec<Location> {
        let mut references = Vec::new();
        let lines: Vec<&str> = text.lines().collect();
        let target_line_idx = position.line as usize;
        
        if target_line_idx >= lines.len() {
            return references;
        }
        
        let target_line = lines[target_line_idx];
        let char_idx = position.character as usize;
        
        if char_idx >= target_line.len() {
            return references;
        }
        
        // Extract word at cursor position
        if let Some((start, end)) = self.extract_word_at_position(target_line, char_idx) {
            let word = &target_line[start..end];
            
            // Search for all occurrences of this word
            for (line_idx, line) in lines.iter().enumerate() {
                let mut search_start = 0;
                while let Some(pos) = line[search_start..].find(word) {
                    let actual_pos = search_start + pos;
                    
                    // Check if this is a whole word match
                    let is_word_boundary = (actual_pos == 0 || !line.chars().nth(actual_pos - 1).unwrap_or(' ').is_alphanumeric()) &&
                        (actual_pos + word.len() >= line.len() || !line.chars().nth(actual_pos + word.len()).unwrap_or(' ').is_alphanumeric());
                    
                    if is_word_boundary {
                        // Skip the original position if not including declaration
                        if !include_declaration && line_idx == target_line_idx && actual_pos == start {
                            search_start = actual_pos + word.len();
                            continue;
                        }
                        
                        references.push(Location::new(
                            uri_from_string("file://current"),
                            Range::new(
                                Position::new(line_idx as u32, actual_pos as u32),
                                Position::new(line_idx as u32, (actual_pos + word.len()) as u32),
                            ),
                        ));
                    }
                    
                    search_start = actual_pos + word.len();
                }
            }
        }
        
        references
    }

    fn generate_semantic_tokens(&self, _ast: &crate::ast::Program, text: &str) -> Vec<SemanticToken> {
        let mut tokens = Vec::new();
        let lines: Vec<&str> = text.lines().collect();
        
        for (line_idx, line) in lines.iter().enumerate() {
            let mut char_idx = 0;
            let chars: Vec<char> = line.chars().collect();
            
            while char_idx < chars.len() {
                // Skip whitespace
                if chars[char_idx].is_whitespace() {
                    char_idx += 1;
                    continue;
                }
                
                // Check for keywords
                if let Some((token_length, token_type)) = self.classify_token_at_position(line, char_idx) {
                    tokens.push(SemanticToken {
                        delta_line: if tokens.is_empty() { line_idx as u32 } else { 0 },
                        delta_start: char_idx as u32,
                        length: token_length as u32,
                        token_type: token_type as u32,
                        token_modifiers_bitset: 0,
                    });
                    char_idx += token_length;
                } else {
                    char_idx += 1;
                }
            }
        }
        
        tokens
    }

    fn classify_token_at_position(&self, line: &str, char_idx: usize) -> Option<(usize, usize)> {
        let remaining = &line[char_idx..];
        
        // Keywords
        let keywords = [
            ("fun", 14), // KEYWORD
            ("val", 14),
            ("mut", 14),
            ("then", 14),
            ("else", 14),
            ("while", 14),
            ("match", 14),
            ("with", 14),
            ("clone", 14),
            ("freeze", 14),
            ("record", 14),
            ("true", 14),
            ("false", 14),
            ("Some", 14),
            ("None", 14),
        ];
        
        for (keyword, token_type) in keywords {
            if remaining.starts_with(keyword) {
                // Check word boundary
                let end_idx = char_idx + keyword.len();
                if end_idx >= line.len() || !line.chars().nth(end_idx).unwrap_or(' ').is_alphanumeric() {
                    return Some((keyword.len(), token_type));
                }
            }
        }
        
        // Numbers
        if remaining.chars().next().unwrap_or(' ').is_ascii_digit() {
            let mut length = 0;
            for ch in remaining.chars() {
                if ch.is_ascii_digit() || ch == '.' {
                    length += 1;
                } else {
                    break;
                }
            }
            return Some((length, 18)); // NUMBER
        }
        
        // Strings
        if remaining.starts_with('"') {
            let mut length = 1;
            let mut escaped = false;
            for ch in remaining[1..].chars() {
                length += 1;
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    break;
                }
            }
            return Some((length, 17)); // STRING
        }
        
        None
    }
}

fn uri_from_string(s: &str) -> Url {
    Url::parse(s).unwrap_or_else(|_| Url::parse("file://unknown").unwrap())
}

#[tower_lsp::async_trait]
impl LanguageServer for RestrictLanguageServer {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string()]),
                    work_done_progress_options: Default::default(),
                    all_commit_characters: None,
                    completion_item: None,
                }),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_highlight_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(false),
                }),
                document_formatting_provider: Some(OneOf::Left(true)),
                document_range_formatting_provider: Some(OneOf::Left(true)),
                document_on_type_formatting_provider: Some(DocumentOnTypeFormattingOptions {
                    first_trigger_character: "\n".to_string(),
                    more_trigger_character: Some(vec![";".to_string()]),
                }),
                rename_provider: Some(OneOf::Left(true)),
                document_link_provider: None,
                color_provider: None,
                folding_range_provider: None,
                declaration_provider: Some(DeclarationCapability::Simple(true)),
                // Enable command execution with LSP-specific command names
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![
                        "restrict.lsp.compile".to_string(),
                        "restrict.lsp.typeCheck".to_string(),
                        "restrict.lsp.showAST".to_string(),
                    ],
                    work_done_progress_options: Default::default(),
                }),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                        SemanticTokensRegistrationOptions {
                            text_document_registration_options: {
                                TextDocumentRegistrationOptions {
                                    document_selector: Some(vec![DocumentFilter {
                                        language: Some("restrict".to_string()),
                                        scheme: Some("file".to_string()),
                                        pattern: None,
                                    }]),
                                }
                            },
                            semantic_tokens_options: SemanticTokensOptions {
                                work_done_progress_options: WorkDoneProgressOptions::default(),
                                legend: SemanticTokensLegend {
                                    token_types: vec![
                                        SemanticTokenType::NAMESPACE,
                                        SemanticTokenType::TYPE,
                                        SemanticTokenType::CLASS,
                                        SemanticTokenType::ENUM,
                                        SemanticTokenType::INTERFACE,
                                        SemanticTokenType::STRUCT,
                                        SemanticTokenType::TYPE_PARAMETER,
                                        SemanticTokenType::PARAMETER,
                                        SemanticTokenType::VARIABLE,
                                        SemanticTokenType::PROPERTY,
                                        SemanticTokenType::ENUM_MEMBER,
                                        SemanticTokenType::EVENT,
                                        SemanticTokenType::FUNCTION,
                                        SemanticTokenType::METHOD,
                                        SemanticTokenType::MACRO,
                                        SemanticTokenType::KEYWORD,
                                        SemanticTokenType::MODIFIER,
                                        SemanticTokenType::COMMENT,
                                        SemanticTokenType::STRING,
                                        SemanticTokenType::NUMBER,
                                        SemanticTokenType::REGEXP,
                                        SemanticTokenType::OPERATOR,
                                    ],
                                    token_modifiers: vec![
                                        SemanticTokenModifier::DECLARATION,
                                        SemanticTokenModifier::DEFINITION,
                                        SemanticTokenModifier::READONLY,
                                        SemanticTokenModifier::STATIC,
                                        SemanticTokenModifier::DEPRECATED,
                                        SemanticTokenModifier::ABSTRACT,
                                        SemanticTokenModifier::ASYNC,
                                        SemanticTokenModifier::MODIFICATION,
                                        SemanticTokenModifier::DOCUMENTATION,
                                        SemanticTokenModifier::DEFAULT_LIBRARY,
                                    ],
                                },
                                range: Some(true),
                                full: Some(SemanticTokensFullOptions::Bool(true)),
                            },
                            static_registration_options: StaticRegistrationOptions::default(),
                        },
                    ),
                ),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "Restrict Language Server".to_string(),
                version: Some("0.1.0".to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Restrict Language Server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text.clone();
        
        {
            let mut documents = self.documents.write().unwrap();
            documents.insert(uri.clone(), text.clone());
        }

        let diagnostics = self.get_diagnostics(&uri, &text);
        
        self.client
            .publish_diagnostics(uri, diagnostics, Some(params.text_document.version))
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        
        if let Some(change) = params.content_changes.into_iter().next() {
            {
                let mut documents = self.documents.write().unwrap();
                documents.insert(uri.clone(), change.text.clone());
            }

            let diagnostics = self.get_diagnostics(&uri, &change.text);
            
            self.client
                .publish_diagnostics(uri, diagnostics, Some(params.text_document.version))
                .await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, format!("File saved: {}", params.text_document.uri))
            .await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        {
            let mut documents = self.documents.write().unwrap();
            documents.remove(&params.text_document.uri);
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = &params.text_document_position_params.position;
        
        let documents = self.documents.read().unwrap();
        if let Some(text) = documents.get(uri) {
            // Parse the document and get type information
            if let Ok((_, ast)) = crate::parse_program(text) {
                let mut type_checker = TypeChecker::new();
                if type_checker.check_program(&ast).is_ok() {
                    if let Some(hover_info) = self.get_hover_info(&ast, text, position) {
                        return Ok(Some(Hover {
                            contents: HoverContents::Scalar(MarkedString::LanguageString(LanguageString {
                                language: "restrict".to_string(),
                                value: hover_info,
                            })),
                            range: None,
                        }));
                    }
                }
            }
        }
        
        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let mut completions = vec![
            // Keywords
            CompletionItem::new_simple("val".to_string(), "Immutable variable binding".to_string()),
            CompletionItem::new_simple("mut val".to_string(), "Mutable variable binding".to_string()),
            CompletionItem::new_simple("fun".to_string(), "Function definition".to_string()),
            CompletionItem::new_simple("record".to_string(), "Record type definition".to_string()),
            CompletionItem::new_simple("then".to_string(), "Conditional expression".to_string()),
            CompletionItem::new_simple("else".to_string(), "Else clause".to_string()),
            CompletionItem::new_simple("while".to_string(), "While loop".to_string()),
            CompletionItem::new_simple("match".to_string(), "Pattern matching".to_string()),
            CompletionItem::new_simple("with".to_string(), "Context binding".to_string()),
            CompletionItem::new_simple("clone".to_string(), "Clone object".to_string()),
            CompletionItem::new_simple("freeze".to_string(), "Freeze object".to_string()),
            CompletionItem::new_simple("true".to_string(), "Boolean true".to_string()),
            CompletionItem::new_simple("false".to_string(), "Boolean false".to_string()),
            CompletionItem::new_simple("Some".to_string(), "Optional value".to_string()),
            CompletionItem::new_simple("None".to_string(), "No value".to_string()),
            
            // Standard library functions
            CompletionItem::new_simple("println".to_string(), "println(s: String) -> Unit".to_string()),
            CompletionItem::new_simple("print".to_string(), "print(s: String) -> Unit".to_string()),
            CompletionItem::new_simple("print_int".to_string(), "print_int(n: Int) -> Unit".to_string()),
            
            // Math functions
            CompletionItem::new_simple("abs".to_string(), "abs(x: Int) -> Int".to_string()),
            CompletionItem::new_simple("max".to_string(), "max(a: Int, b: Int) -> Int".to_string()),
            CompletionItem::new_simple("min".to_string(), "min(a: Int, b: Int) -> Int".to_string()),
            CompletionItem::new_simple("pow".to_string(), "pow(base: Int, exp: Int) -> Int".to_string()),
            CompletionItem::new_simple("factorial".to_string(), "factorial(n: Int) -> Int".to_string()),
            
            // List functions
            CompletionItem::new_simple("list_head".to_string(), "list_head<T>(list: List<T>) -> Option<T>".to_string()),
            CompletionItem::new_simple("list_tail".to_string(), "list_tail<T>(list: List<T>) -> Option<List<T>>".to_string()),
            CompletionItem::new_simple("list_reverse".to_string(), "list_reverse<T>(list: List<T>) -> List<T>".to_string()),
            CompletionItem::new_simple("list_append".to_string(), "list_append<T>(list: List<T>, item: T) -> List<T>".to_string()),
            CompletionItem::new_simple("list_concat".to_string(), "list_concat<T>(a: List<T>, b: List<T>) -> List<T>".to_string()),
            CompletionItem::new_simple("list_count".to_string(), "list_count<T>(list: List<T>) -> Int".to_string()),
            CompletionItem::new_simple("list_is_empty".to_string(), "list_is_empty<T>(list: List<T>) -> Bool".to_string()),
            
            // Option functions
            CompletionItem::new_simple("option_is_some".to_string(), "option_is_some<T>(opt: Option<T>) -> Bool".to_string()),
            CompletionItem::new_simple("option_is_none".to_string(), "option_is_none<T>(opt: Option<T>) -> Bool".to_string()),
            CompletionItem::new_simple("option_unwrap_or".to_string(), "option_unwrap_or<T>(opt: Option<T>, default: T) -> T".to_string()),
            
            // Boolean functions
            CompletionItem::new_simple("not".to_string(), "not(b: Bool) -> Bool".to_string()),
            CompletionItem::new_simple("and".to_string(), "and(a: Bool, b: Bool) -> Bool".to_string()),
            CompletionItem::new_simple("or".to_string(), "or(a: Bool, b: Bool) -> Bool".to_string()),
            
            // Utility functions
            CompletionItem::new_simple("identity".to_string(), "identity<T>(x: T) -> T".to_string()),
            CompletionItem::new_simple("assert".to_string(), "assert(condition: Bool, message: String) -> Unit".to_string()),
            CompletionItem::new_simple("panic".to_string(), "panic(message: String) -> Unit".to_string()),
        ];

        // Context-sensitive completions
        let uri = &params.text_document_position.text_document.uri;
        let documents = self.documents.read().unwrap();
        if let Some(text) = documents.get(uri) {
            if let Ok((_, ast)) = crate::parse_program(text) {
                // Add symbols from current document
                for decl in &ast.declarations {
                    match decl {
                        crate::ast::TopDecl::Function(func) => {
                            completions.push(CompletionItem::new_simple(
                                func.name.clone(),
                                format!("User function with {} parameters", func.params.len()),
                            ));
                        }
                        crate::ast::TopDecl::Record(record) => {
                            completions.push(CompletionItem::new_simple(
                                record.name.clone(),
                                format!("Record type with {} fields", record.fields.len()),
                            ));
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(Some(CompletionResponse::Array(completions)))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = &params.text_document_position_params.position;
        
        let documents = self.documents.read().unwrap();
        if let Some(text) = documents.get(uri) {
            // Parse the document to get AST and find symbol at position
            if let Ok((_, ast)) = crate::parse_program(text) {
                if let Some(location) = self.find_definition_at_position(&ast, text, position) {
                    return Ok(Some(GotoDefinitionResponse::Scalar(location)));
                }
            }
        }
        
        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = &params.text_document_position.position;
        
        let documents = self.documents.read().unwrap();
        if let Some(text) = documents.get(uri) {
            if let Ok((_, ast)) = crate::parse_program(text) {
                let references = self.find_references_at_position(&ast, text, position, params.context.include_declaration);
                if !references.is_empty() {
                    return Ok(Some(references));
                }
            }
        }
        
        Ok(None)
    }

    async fn document_highlight(
        &self,
        _: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        Ok(None)
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;
        
        let documents = self.documents.read().unwrap();
        if let Some(text) = documents.get(uri) {
            if let Ok((_, ast)) = crate::parse_program(text) {
                let symbols = self.extract_document_symbols(&ast, text);
                if !symbols.is_empty() {
                    return Ok(Some(DocumentSymbolResponse::Nested(symbols)));
                }
            }
        }
        
        Ok(None)
    }

    async fn symbol(
        &self,
        _: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        Ok(None)
    }

    async fn code_action(&self, _: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        Ok(None)
    }

    async fn code_lens(&self, _: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        Ok(None)
    }

    async fn formatting(&self, _: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        Ok(None)
    }

    async fn range_formatting(
        &self,
        _: DocumentRangeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        Ok(None)
    }

    async fn on_type_formatting(
        &self,
        _: DocumentOnTypeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        Ok(None)
    }

    async fn rename(&self, _: RenameParams) -> Result<Option<WorkspaceEdit>> {
        Ok(None)
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = &params.text_document.uri;
        
        let documents = self.documents.read().unwrap();
        if let Some(text) = documents.get(uri) {
            if let Ok((_, ast)) = crate::parse_program(text) {
                let tokens = self.generate_semantic_tokens(&ast, text);
                if !tokens.is_empty() {
                    return Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
                        result_id: None,
                        data: tokens,
                    })));
                }
            }
        }
        
        Ok(None)
    }

    async fn semantic_tokens_range(
        &self,
        _: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        Ok(None)
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<serde_json::Value>> {
        match params.command.as_str() {
            "restrict.lsp.compile" => {
                // Get the active document URI from arguments
                if let Some(args) = params.arguments.get(0) {
                    if let Some(uri_str) = args.as_str() {
                        if let Ok(uri) = Url::parse(uri_str) {
                            let text = {
                                let documents = self.documents.read().unwrap();
                                documents.get(&uri).cloned()
                            };
                            
                            if let Some(text) = text {
                                // Parse the program
                                match crate::parse_program(&text) {
                                    Ok((_, ast)) => {
                                        // Type check
                                        let mut type_checker = crate::TypeChecker::new();
                                        if let Err(e) = type_checker.check_program(&ast) {
                                            return Ok(Some(serde_json::json!({
                                                "success": false,
                                                "message": format!("Type error: {}", e)
                                            })));
                                        }
                                        
                                        // Generate WASM
                                        let (wasm_result, has_globals) = {
                                            let mut codegen = crate::WasmCodeGen::new();
                                            let result = codegen.generate(&ast);
                                            let has_globals = ast.declarations.iter().any(|d| matches!(d, crate::ast::TopDecl::Binding(_)));
                                            (result, has_globals)
                                        };
                                        
                                        match wasm_result {
                                            Ok(wasm_text) => {
                                                // Save the WAT file
                                                if let Ok(file_path) = uri.to_file_path() {
                                                    let wat_path = file_path.with_extension("wat");
                                                    match std::fs::write(&wat_path, &wasm_text) {
                                                        Ok(_) => {
                                                            let message = if has_globals {
                                                                format!("Compilation completed with warnings (global bindings skipped). Output: {}", wat_path.display())
                                                            } else {
                                                                format!("Compilation successful. Output: {}", wat_path.display())
                                                            };
                                                            
                                                            self.client
                                                                .log_message(MessageType::INFO, format!("Saved WAT to: {}", wat_path.display()))
                                                                .await;
                                                                
                                                            return Ok(Some(serde_json::json!({
                                                                "success": true,
                                                                "message": message,
                                                                "output": wat_path.to_string_lossy()
                                                            })));
                                                        }
                                                        Err(e) => {
                                                            return Ok(Some(serde_json::json!({
                                                                "success": false,
                                                                "message": format!("Failed to save WAT file: {}", e)
                                                            })));
                                                        }
                                                    }
                                                } else {
                                                    return Ok(Some(serde_json::json!({
                                                        "success": false,
                                                        "message": "Failed to convert URI to file path"
                                                    })));
                                                }
                                            }
                                            Err(e) => {
                                                return Ok(Some(serde_json::json!({
                                                    "success": false,
                                                    "message": format!("Code generation error: {}", e)
                                                })));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        return Ok(Some(serde_json::json!({
                                            "success": false,
                                            "message": format!("Parse error: {:?}", e)
                                        })));
                                    }
                                }
                            } else {
                                return Ok(Some(serde_json::json!({
                                    "success": false,
                                    "message": "File not found in LSP cache"
                                })));
                            }
                        }
                    }
                }
                Ok(Some(serde_json::json!({
                    "success": false,
                    "message": "No file URI provided"
                })))
            }
            "restrict.lsp.typeCheck" => {
                if let Some(args) = params.arguments.get(0) {
                    if let Some(uri_str) = args.as_str() {
                        if let Ok(uri) = Url::parse(uri_str) {
                            let text = {
                                let documents = self.documents.read().unwrap();
                                documents.get(&uri).cloned()
                            };
                            
                            if let Some(text) = text {
                                let diagnostics = self.get_diagnostics(&uri, &text);
                                
                                self.client
                                    .publish_diagnostics(uri.clone(), diagnostics.clone(), None)
                                    .await;
                                
                                let success = diagnostics.is_empty();
                                return Ok(Some(serde_json::json!({
                                    "success": success,
                                    "message": if success { "Type check passed" } else { "Type check failed" },
                                    "diagnostics": diagnostics.len()
                                })));
                            }
                        }
                    }
                }
                Ok(Some(serde_json::json!({
                    "success": false,
                    "message": "No file URI provided"
                })))
            }
            "restrict.lsp.showAST" => {
                if let Some(args) = params.arguments.get(0) {
                    if let Some(uri_str) = args.as_str() {
                        if let Ok(uri) = Url::parse(uri_str) {
                            let text = {
                                let documents = self.documents.read().unwrap();
                                documents.get(&uri).cloned()
                            };
                            
                            if let Some(text) = text {
                                match crate::parse_program(&text) {
                                    Ok((_, ast)) => {
                                        return Ok(Some(serde_json::json!({
                                            "success": true,
                                            "ast": format!("{:#?}", ast)
                                        })));
                                    }
                                    Err(e) => {
                                        return Ok(Some(serde_json::json!({
                                            "success": false,
                                            "message": format!("Parse error: {:?}", e)
                                        })));
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(Some(serde_json::json!({
                    "success": false,
                    "message": "No file URI provided"
                })))
            }
            _ => Ok(None),
        }
    }
}

pub async fn start_lsp_server() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| RestrictLanguageServer::new(client));
    Server::new(stdin, stdout, socket).serve(service).await;
}