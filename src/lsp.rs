use std::collections::HashMap;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use crate::{lex, parse_program, TypeChecker};

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

        // Lexing
        match lex(text) {
            Ok((remaining, _tokens)) => {
                // Only report unparsed input if it contains non-whitespace characters
                if !remaining.trim().is_empty() {
                    diagnostics.push(Diagnostic::new_simple(
                        Range::new(Position::new(0, 0), Position::new(0, 1)),
                        format!("Lexer: unparsed input remaining: '{}'", remaining.trim()),
                    ));
                }
            }
            Err(e) => {
                diagnostics.push(Diagnostic::new_simple(
                    Range::new(Position::new(0, 0), Position::new(0, 1)),
                    format!("Lexing error: {:?}", e),
                ));
                return diagnostics;
            }
        }

        // Parsing
        match parse_program(text) {
            Ok((remaining, ast)) => {
                // Only report unparsed input if it contains non-whitespace characters
                if !remaining.trim().is_empty() {
                    diagnostics.push(Diagnostic::new_simple(
                        Range::new(Position::new(0, 0), Position::new(0, 1)),
                        format!("Parser: unparsed input remaining: '{}'", remaining.trim()),
                    ));
                }

                // Type checking
                let mut type_checker = TypeChecker::new();
                if let Err(e) = type_checker.check_program(&ast) {
                    diagnostics.push(Diagnostic::new_simple(
                        Range::new(Position::new(0, 0), Position::new(0, 1)),
                        format!("Type error: {}", e),
                    ));
                }
            }
            Err(e) => {
                diagnostics.push(Diagnostic::new_simple(
                    Range::new(Position::new(0, 0), Position::new(0, 1)),
                    format!("Parsing error: {:?}", e),
                ));
            }
        }

        diagnostics
    }
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
        
        {
            let documents = self.documents.read().unwrap();
            if let Some(_text) = documents.get(uri) {
                return Ok(Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(
                        "Restrict Language - Hover information".to_string(),
                    )),
                    range: None,
                }));
            }
        }
        
        Ok(None)
    }

    async fn completion(&self, _: CompletionParams) -> Result<Option<CompletionResponse>> {
        Ok(Some(CompletionResponse::Array(vec![
            CompletionItem::new_simple("val".to_string(), "Immutable variable binding".to_string()),
            CompletionItem::new_simple("mut val".to_string(), "Mutable variable binding".to_string()),
            CompletionItem::new_simple("fun".to_string(), "Function definition".to_string()),
            CompletionItem::new_simple("if".to_string(), "Conditional expression".to_string()),
            CompletionItem::new_simple("then".to_string(), "Then clause".to_string()),
            CompletionItem::new_simple("else".to_string(), "Else clause".to_string()),
            CompletionItem::new_simple("with".to_string(), "Context binding".to_string()),
            CompletionItem::new_simple("clone".to_string(), "Clone object".to_string()),
            CompletionItem::new_simple("freeze".to_string(), "Freeze object".to_string()),
            CompletionItem::new_simple("true".to_string(), "Boolean true".to_string()),
            CompletionItem::new_simple("false".to_string(), "Boolean false".to_string()),
        ])))
    }

    async fn goto_definition(
        &self,
        _: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        Ok(None)
    }

    async fn references(&self, _: ReferenceParams) -> Result<Option<Vec<Location>>> {
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
        _: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
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
        _: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
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