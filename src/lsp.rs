use crate::ast::{Literal, Pattern};
use crate::diagnostics::{format_lex_error, format_parse_error};
use crate::module::resolve_program_imports_for_file;
use crate::release_surface::check_v001_release_surface;
use crate::type_checker::TypeError;
use crate::{lex, parse_program, TypeChecker};
use std::collections::HashMap;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Debug)]
pub struct RestrictLanguageServer {
    client: Client,
    documents: std::sync::RwLock<HashMap<Url, String>>,
}

#[allow(deprecated)]
fn document_symbol(
    name: String,
    detail: Option<String>,
    kind: SymbolKind,
    range: Range,
    selection_range: Range,
) -> DocumentSymbol {
    DocumentSymbol {
        name,
        detail,
        kind,
        tags: None,
        deprecated: None,
        range,
        selection_range,
        children: None,
    }
}

fn lsp_type_error_message(error: &TypeError) -> String {
    let message = sanitize_lsp_diagnostic_text(&error.to_string());

    format!("Type error: {message}")
}

fn diagnostic_for_message(source: &str, message: String) -> Diagnostic {
    Diagnostic::new_simple(diagnostic_range_for_message(source, &message), message)
}

fn diagnostic_for_type_error(source: &str, error: &TypeError) -> Diagnostic {
    diagnostic_for_message(source, lsp_type_error_message(error))
}

fn diagnostic_range_for_message(source: &str, message: &str) -> Range {
    if let Some(position) = line_column_from_message(message) {
        return single_character_range(source, position);
    }

    if let Some(binding_name) = binding_name_from_message(message) {
        if let Some(range) = binding_name_range(source, binding_name) {
            return range;
        }
    }

    if let Some(function_name) = quoted_name_from_message(message, "function '") {
        if let Some(range) = declaration_name_range(source, "fun", function_name) {
            return range;
        }
    }

    if let Some(record_name) = quoted_name_from_message(message, "record '") {
        if let Some(range) = declaration_name_range(source, "record", record_name) {
            return range;
        }
    }

    Range::new(Position::new(0, 0), Position::new(0, 1))
}

fn line_column_from_message(message: &str) -> Option<Position> {
    let line_start = message.find("line ")? + "line ".len();
    let line_end = line_start
        + message[line_start..]
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .map(char::len_utf8)
            .sum::<usize>();
    let line = message[line_start..line_end].parse::<u32>().ok()?;

    let column_marker = message[line_end..].find("column ")? + line_end + "column ".len();
    let column_end = column_marker
        + message[column_marker..]
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .map(char::len_utf8)
            .sum::<usize>();
    let column = message[column_marker..column_end].parse::<u32>().ok()?;

    Some(Position::new(
        line.saturating_sub(1),
        column.saturating_sub(1),
    ))
}

fn binding_name_from_message(message: &str) -> Option<&str> {
    quoted_name_from_message(message, "binding '")
}

fn quoted_name_from_message<'a>(message: &'a str, marker: &str) -> Option<&'a str> {
    let start = message.find(marker)? + marker.len();
    let end = message[start..].find('\'')? + start;
    Some(&message[start..end])
}

fn declaration_name_range(source: &str, keyword: &str, name: &str) -> Option<Range> {
    for (line_index, line) in source.lines().enumerate() {
        let Some(keyword_start) = find_keyword(line, keyword) else {
            continue;
        };
        let after_keyword = keyword_start + keyword.len();
        let rest = &line[after_keyword..];
        let leading_space = rest.len() - rest.trim_start().len();
        let name_start = after_keyword + leading_space;
        let after_name = name_start + name.len();

        if line[name_start..].starts_with(name)
            && line[after_name..]
                .chars()
                .next()
                .is_none_or(|ch| !is_identifier_continue(ch))
        {
            return Some(Range::new(
                Position::new(
                    line_index as u32,
                    byte_to_character(line, name_start) as u32,
                ),
                Position::new(
                    line_index as u32,
                    byte_to_character(line, after_name) as u32,
                ),
            ));
        }
    }

    None
}

fn find_keyword(line: &str, keyword: &str) -> Option<usize> {
    line.match_indices(keyword).find_map(|(idx, _)| {
        let before_ok = line[..idx]
            .chars()
            .next_back()
            .is_none_or(|ch| !is_identifier_continue(ch));
        let after_idx = idx + keyword.len();
        let after_ok = line[after_idx..]
            .chars()
            .next()
            .is_some_and(char::is_whitespace);
        (before_ok && after_ok).then_some(idx)
    })
}

fn is_identifier_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn binding_name_range(source: &str, binding_name: &str) -> Option<Range> {
    for (line_index, line) in source.lines().enumerate() {
        let Some((pattern_start, pattern_end)) = binding_pattern_range(line) else {
            continue;
        };
        let pattern = &line[pattern_start..pattern_end];
        let Some(relative_start) = pattern.find(binding_name) else {
            continue;
        };
        let start = pattern_start + relative_start;
        let end = start + binding_name.len();

        return Some(Range::new(
            Position::new(line_index as u32, byte_to_character(line, start) as u32),
            Position::new(line_index as u32, byte_to_character(line, end) as u32),
        ));
    }

    None
}

fn single_character_range(source: &str, position: Position) -> Range {
    let line = source.lines().nth(position.line as usize).unwrap_or("");
    let line_len = line.chars().count() as u32;
    let start_character = position.character.min(line_len);
    let end_character = start_character.saturating_add(1).min(line_len.max(1));

    Range::new(
        Position::new(position.line, start_character),
        Position::new(position.line, end_character),
    )
}

fn byte_to_character(line: &str, byte_index: usize) -> usize {
    line[..byte_index.min(line.len())].chars().count()
}

fn collect_diagnostics_for_source(uri: &Url, text: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // Lexing
    match lex(text) {
        Ok((remaining, _tokens)) => {
            // Only report unparsed input if it contains non-whitespace characters
            if !remaining.trim().is_empty() {
                diagnostics.push(diagnostic_for_message(
                    text,
                    format!("Lexer: unparsed input remaining: '{}'", remaining.trim()),
                ));
            }
        }
        Err(e) => {
            diagnostics.push(diagnostic_for_message(text, format_lex_error(text, e)));
            return diagnostics;
        }
    }

    let ast = match parse_and_resolve_program_for_lsp(uri, text) {
        Ok(ast) => ast,
        Err(message) => {
            diagnostics.push(diagnostic_for_message(text, message));
            return diagnostics;
        }
    };

    // Type checking
    let mut type_checker = TypeChecker::new();
    if let Err(e) = type_checker.check_program(&ast) {
        diagnostics.push(diagnostic_for_type_error(text, &e));
        return diagnostics;
    }

    if let Err(e) = check_v001_release_surface(&ast, &type_checker) {
        diagnostics.push(diagnostic_for_message(
            text,
            format!("Release surface error: {}", e),
        ));
    }

    diagnostics
}

fn parse_and_resolve_program_for_lsp(
    uri: &Url,
    text: &str,
) -> std::result::Result<crate::ast::Program, String> {
    let (remaining, ast) = parse_program(text).map_err(|e| format_parse_error(text, e))?;

    if !remaining.trim().is_empty() {
        return Err(format!(
            "Parser: unparsed input remaining: '{}'",
            remaining.trim()
        ));
    }

    if ast.imports.is_empty() {
        return Ok(ast);
    }

    let source_path = uri.to_file_path().map_err(|_| {
        "Import resolution error: source-level imports require a file URI".to_string()
    })?;

    resolve_program_imports_for_file(ast, &source_path)
        .map_err(|e| format!("Import resolution error: {}", e))
}

fn sanitize_lsp_diagnostic_text(message: &str) -> String {
    let message = collapse_internal_type_wrappers(message)
        .replace("InferVar", "inference variable")
        .replace("TypeVarId", "inference variable")
        .replace("Projection", "associated type")
        .replace(" as Container.", " associated type Container::")
        .replace("as Container.", "associated type Container::");
    mask_inference_vars(&message)
}

fn collapse_internal_type_wrappers(message: &str) -> String {
    let mut output = String::with_capacity(message.len());
    let mut index = 0;

    while index < message.len() {
        let replacement = [
            ("InferVar(", "an inferred type"),
            ("TypeVarId(", "an inferred type"),
            ("Projection(", "associated type"),
        ]
        .into_iter()
        .find_map(|(prefix, replacement)| {
            if !message[index..].starts_with(prefix) {
                return None;
            }

            let open_index = index + prefix.len() - 1;
            matching_paren_index(message, open_index).map(|close_index| (replacement, close_index))
        });

        if let Some((replacement, close_index)) = replacement {
            output.push_str(replacement);
            index = close_index + 1;
        } else {
            let ch = message[index..]
                .chars()
                .next()
                .expect("index should point to a char boundary");
            output.push(ch);
            index += ch.len_utf8();
        }
    }

    output
}

fn matching_paren_index(message: &str, open_index: usize) -> Option<usize> {
    let mut depth = 0usize;

    for (offset, ch) in message[open_index..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open_index + offset);
                }
            }
            _ => {}
        }
    }

    None
}

fn mask_inference_vars(message: &str) -> String {
    let mut output = String::with_capacity(message.len());
    let mut chars = message.chars().peekable();

    while let Some(ch) = chars.next() {
        let next_is_digit = chars
            .peek()
            .map(|next| next.is_ascii_digit())
            .unwrap_or(false);
        if ch == '?' && next_is_digit {
            while chars
                .peek()
                .map(|next| next.is_ascii_digit())
                .unwrap_or(false)
            {
                chars.next();
            }
            output.push_str("an inferred type");
        } else {
            output.push(ch);
        }
    }

    output
}

fn builtin_completion_items() -> Vec<CompletionItem> {
    vec![
        CompletionItem::new_simple("val".to_string(), "Immutable variable binding".to_string()),
        CompletionItem::new_simple(
            "mut val".to_string(),
            "Mutable variable binding".to_string(),
        ),
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
        CompletionItem::new_simple(
            "println".to_string(),
            "fun println: (s: String) -> ()".to_string(),
        ),
        CompletionItem::new_simple(
            "print".to_string(),
            "fun print: (s: String) -> ()".to_string(),
        ),
        CompletionItem::new_simple(
            "print_int".to_string(),
            "fun print_int: (n: Int32) -> ()".to_string(),
        ),
        CompletionItem::new_simple(
            "abs".to_string(),
            "fun abs: (x: Int32) -> Int32".to_string(),
        ),
        CompletionItem::new_simple(
            "max".to_string(),
            "fun max: (a: Int32, b: Int32) -> Int32".to_string(),
        ),
        CompletionItem::new_simple(
            "min".to_string(),
            "fun min: (a: Int32, b: Int32) -> Int32".to_string(),
        ),
        CompletionItem::new_simple(
            "pow".to_string(),
            "fun pow: (base: Int32, exp: Int32) -> Int32".to_string(),
        ),
        CompletionItem::new_simple(
            "factorial".to_string(),
            "fun factorial: (n: Int32) -> Int32".to_string(),
        ),
        CompletionItem::new_simple(
            "list_head".to_string(),
            "fun list_head: <T>(list: List<T>) -> Option<T>".to_string(),
        ),
        CompletionItem::new_simple(
            "list_tail".to_string(),
            "fun list_tail: <T>(list: List<T>) -> Option<List<T>>".to_string(),
        ),
        CompletionItem::new_simple(
            "list_reverse".to_string(),
            "fun list_reverse: <T>(list: List<T>) -> List<T>".to_string(),
        ),
        CompletionItem::new_simple(
            "list_append".to_string(),
            "fun list_append: <T>(list: List<T>, item: T) -> List<T>".to_string(),
        ),
        CompletionItem::new_simple(
            "list_concat".to_string(),
            "fun list_concat: <T>(a: List<T>, b: List<T>) -> List<T>".to_string(),
        ),
        CompletionItem::new_simple(
            "list_count".to_string(),
            "fun list_count: <T>(list: List<T>) -> Int32".to_string(),
        ),
        CompletionItem::new_simple(
            "list_is_empty".to_string(),
            "fun list_is_empty: <T>(list: List<T>) -> Boolean".to_string(),
        ),
        CompletionItem::new_simple(
            "option_is_some".to_string(),
            "fun option_is_some: <T>(opt: Option<T>) -> Boolean".to_string(),
        ),
        CompletionItem::new_simple(
            "option_is_none".to_string(),
            "fun option_is_none: <T>(opt: Option<T>) -> Boolean".to_string(),
        ),
        CompletionItem::new_simple(
            "option_unwrap_or".to_string(),
            "fun option_unwrap_or: <T>(opt: Option<T>, default: T) -> T".to_string(),
        ),
        CompletionItem::new_simple(
            "not".to_string(),
            "fun not: (b: Boolean) -> Boolean".to_string(),
        ),
        CompletionItem::new_simple(
            "and".to_string(),
            "fun and: (a: Boolean, b: Boolean) -> Boolean".to_string(),
        ),
        CompletionItem::new_simple(
            "or".to_string(),
            "fun or: (a: Boolean, b: Boolean) -> Boolean".to_string(),
        ),
        CompletionItem::new_simple(
            "identity".to_string(),
            "fun identity: <T>(x: T) -> T".to_string(),
        ),
        CompletionItem::new_simple(
            "assert".to_string(),
            "fun assert: (condition: Boolean, message: String) -> ()".to_string(),
        ),
        CompletionItem::new_simple(
            "panic".to_string(),
            "fun panic: (message: String) -> ()".to_string(),
        ),
    ]
}

fn literal_symbol_label(literal: &Literal) -> String {
    match literal {
        Literal::Int(value) => value.to_string(),
        Literal::Float(value) => value.to_string(),
        Literal::String(value) => format!("\"{value}\""),
        Literal::Char(value) => format!("'{value}'"),
        Literal::Bool(value) => value.to_string(),
        Literal::Unit => "()".to_string(),
    }
}

fn pattern_symbol_label(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Wildcard => "_".to_string(),
        Pattern::Literal(literal) => literal_symbol_label(literal),
        Pattern::Ident(name) => name.clone(),
        Pattern::Record(name, fields) => format!(
            "{} {{ {} }}",
            name,
            fields
                .iter()
                .map(|(field, pattern)| {
                    if matches!(pattern, Pattern::Ident(binding) if binding == field) {
                        field.clone()
                    } else {
                        format!("{}: {}", field, pattern_symbol_label(pattern))
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Pattern::RecordDestruct {
            type_name,
            fields,
            rest,
        } => {
            let mut parts = fields
                .iter()
                .map(|(field, pattern)| {
                    if matches!(pattern, Pattern::Ident(binding) if binding == field) {
                        field.clone()
                    } else {
                        format!("{}: {}", field, pattern_symbol_label(pattern))
                    }
                })
                .collect::<Vec<_>>();

            if let Some(rest) = rest {
                parts.push(format!("...{rest}"));
            }

            format!("{} {{ {} }}", type_name, parts.join(", "))
        }
        Pattern::Some(inner) => format!("Some({})", pattern_symbol_label(inner)),
        Pattern::None => "None".to_string(),
        Pattern::Ok(inner) => format!("Ok({})", pattern_symbol_label(inner)),
        Pattern::Err(inner) => format!("Err({})", pattern_symbol_label(inner)),
        Pattern::EmptyList => "[]".to_string(),
        Pattern::ListCons(head, tail) => {
            format!(
                "[{} | {}]",
                pattern_symbol_label(head),
                pattern_symbol_label(tail)
            )
        }
        Pattern::ListExact(patterns) => format!(
            "[{}]",
            patterns
                .iter()
                .map(|pattern| pattern_symbol_label(pattern))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn binding_pattern_range(line: &str) -> Option<(usize, usize)> {
    let pattern_start = line
        .find("mut val ")
        .map(|index| index + "mut val ".len())
        .or_else(|| line.find("val ").map(|index| index + "val ".len()))?;

    let mut paren_depth: u32 = 0;
    let mut brace_depth: u32 = 0;
    let mut bracket_depth: u32 = 0;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut pattern_end = line.len();

    for (offset, char) in line[pattern_start..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        if in_string {
            if char == '\\' {
                escaped = true;
            } else if char == '"' {
                in_string = false;
            }
            continue;
        }

        if in_char {
            if char == '\\' {
                escaped = true;
            } else if char == '\'' {
                in_char = false;
            }
            continue;
        }

        match char {
            '"' => in_string = true,
            '\'' => in_char = true,
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '=' | ':' if paren_depth == 0 && brace_depth == 0 && bracket_depth == 0 => {
                pattern_end = pattern_start + offset;
                break;
            }
            _ => {}
        }
    }

    while pattern_end > pattern_start {
        let Some((index, char)) = line[..pattern_end].char_indices().next_back() else {
            break;
        };
        if !char.is_whitespace() {
            break;
        }
        pattern_end = index;
    }

    (pattern_start < pattern_end).then_some((pattern_start, pattern_end))
}

const SEMANTIC_TOKEN_KEYWORD: u32 = 15;
const SEMANTIC_TOKEN_STRING: u32 = 18;
const SEMANTIC_TOKEN_NUMBER: u32 = 19;
const SEMANTIC_TOKEN_OPERATOR: u32 = 21;

fn restrict_server_capabilities() -> ServerCapabilities {
    let token_types = vec![
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
    ];
    debug_assert_eq!(
        token_types[SEMANTIC_TOKEN_KEYWORD as usize],
        SemanticTokenType::KEYWORD
    );
    debug_assert_eq!(
        token_types[SEMANTIC_TOKEN_STRING as usize],
        SemanticTokenType::STRING
    );
    debug_assert_eq!(
        token_types[SEMANTIC_TOKEN_NUMBER as usize],
        SemanticTokenType::NUMBER
    );
    debug_assert_eq!(
        token_types[SEMANTIC_TOKEN_OPERATOR as usize],
        SemanticTokenType::OPERATOR
    );

    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
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
        document_highlight_provider: None,
        document_symbol_provider: Some(OneOf::Left(true)),
        workspace_symbol_provider: None,
        code_action_provider: None,
        code_lens_provider: None,
        document_formatting_provider: None,
        document_range_formatting_provider: None,
        document_on_type_formatting_provider: None,
        rename_provider: None,
        document_link_provider: None,
        color_provider: None,
        folding_range_provider: None,
        declaration_provider: None,
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
                    text_document_registration_options: TextDocumentRegistrationOptions {
                        document_selector: Some(vec![DocumentFilter {
                            language: Some("restrict".to_string()),
                            scheme: Some("file".to_string()),
                            pattern: None,
                        }]),
                    },
                    semantic_tokens_options: SemanticTokensOptions {
                        work_done_progress_options: WorkDoneProgressOptions::default(),
                        legend: SemanticTokensLegend {
                            token_types,
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
                        range: None,
                        full: Some(SemanticTokensFullOptions::Bool(true)),
                    },
                    static_registration_options: StaticRegistrationOptions::default(),
                },
            ),
        ),
        ..ServerCapabilities::default()
    }
}

impl RestrictLanguageServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: std::sync::RwLock::new(HashMap::new()),
        }
    }

    fn get_diagnostics(&self, uri: &Url, text: &str) -> Vec<Diagnostic> {
        collect_diagnostics_for_source(uri, text)
    }

    fn parse_and_resolve_program(
        &self,
        uri: &Url,
        text: &str,
    ) -> std::result::Result<crate::ast::Program, String> {
        parse_and_resolve_program_for_lsp(uri, text)
    }

    fn find_definition_at_position(
        uri: &Url,
        _ast: &crate::ast::Program,
        text: &str,
        position: &Position,
    ) -> Option<Location> {
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
        let (start, end) = Self::extract_word_at_position(target_line, char_idx)?;
        let word = &target_line[start..end];

        // Search for function definitions
        for (line_idx, line) in lines.iter().enumerate() {
            if line.contains("fun ") && line.contains(word) {
                // Simple heuristic: if the line contains "fun" and our word
                if let Some(pos) = line.find(word) {
                    return Some(Location::new(
                        uri.clone(),
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

    fn get_hover_info(
        &self,
        _ast: &crate::ast::Program,
        text: &str,
        position: &Position,
    ) -> Option<String> {
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
        if let Some((start, end)) = Self::extract_word_at_position(target_line, char_idx) {
            let word = &target_line[start..end];

            // Provide hover information based on word
            match word {
                "fun" => Some(
                    "**fun** - Function definition keyword\n\nDefines a new function.".to_string(),
                ),
                "val" => Some(
                    "**val** - Immutable variable binding\n\nCreates an immutable binding."
                        .to_string(),
                ),
                "mut" => Some(
                    "**mut** - Mutable variable modifier\n\nMakes a variable mutable.".to_string(),
                ),
                "then" => Some(
                    "**then** - Conditional expression\n\nConditional branch keyword.".to_string(),
                ),
                "else" => Some(
                    "**else** - Alternative branch\n\nAlternative branch in conditional."
                        .to_string(),
                ),
                "while" => Some("**while** - Loop expression\n\nLoop construct.".to_string()),
                "with" => {
                    Some("**with** - Context binding\n\nBinds context variables.".to_string())
                }
                "clone" => {
                    Some("**clone** - Object cloning\n\nCreates a copy of an object.".to_string())
                }
                "freeze" => {
                    Some("**freeze** - Object freezing\n\nMakes an object immutable.".to_string())
                }
                "true" | "false" => {
                    Some(format!("**{}** - Boolean literal\n\nType: Boolean", word))
                }
                _ => {
                    // Check if it's a standard library function
                    match word {
                        "println" => Some("**fun println: (s: String) -> ()**\n\nPrints a string followed by a newline.".to_string()),
                        "abs" => Some("**fun abs: (x: Int32) -> Int32**\n\nReturns the absolute value of an integer.".to_string()),
                        "max" => Some("**fun max: (a: Int32, b: Int32) -> Int32**\n\nReturns the maximum of two integers.".to_string()),
                        "min" => Some("**fun min: (a: Int32, b: Int32) -> Int32**\n\nReturns the minimum of two integers.".to_string()),
                        "list_head" => Some("**fun list_head: <T>(list: List<T>) -> Option<T>**\n\nReturns the first element of a list.".to_string()),
                        "list_tail" => Some("**fun list_tail: <T>(list: List<T>) -> Option<List<T>>**\n\nReturns the tail of a list.".to_string()),
                        "option_unwrap_or" => Some("**fun option_unwrap_or: <T>(opt: Option<T>, default: T) -> T**\n\nUnwraps an Option or returns a default value.".to_string()),
                        _ => Some(format!("Symbol: **{}**", word)),
                    }
                }
            }
        } else {
            None
        }
    }

    fn extract_document_symbols(ast: &crate::ast::Program, text: &str) -> Vec<DocumentSymbol> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = text.lines().collect();
        let mut next_binding_line = 0;

        // Extract symbols from AST
        for decl in &ast.declarations {
            match decl {
                crate::ast::TopDecl::Function(func) => {
                    // Find the function in the text to get its position
                    for (line_idx, line) in lines.iter().enumerate() {
                        if line.contains(&format!("fun {}", func.name)) {
                            let start_pos = line.find(&func.name).unwrap_or(0);
                            symbols.push(document_symbol(
                                func.name.clone(),
                                Some(format!("Function with {} parameters", func.params.len())),
                                SymbolKind::FUNCTION,
                                Range::new(
                                    Position::new(line_idx as u32, 0),
                                    Position::new(line_idx as u32, line.len() as u32),
                                ),
                                Range::new(
                                    Position::new(line_idx as u32, start_pos as u32),
                                    Position::new(
                                        line_idx as u32,
                                        (start_pos + func.name.len()) as u32,
                                    ),
                                ),
                            ));
                            break;
                        }
                    }
                }
                crate::ast::TopDecl::Record(record) => {
                    for (line_idx, line) in lines.iter().enumerate() {
                        if line.contains(&format!("record {}", record.name)) {
                            let start_pos = line.find(&record.name).unwrap_or(0);
                            symbols.push(document_symbol(
                                record.name.clone(),
                                Some(format!("Record with {} fields", record.fields.len())),
                                SymbolKind::STRUCT,
                                Range::new(
                                    Position::new(line_idx as u32, 0),
                                    Position::new(line_idx as u32, line.len() as u32),
                                ),
                                Range::new(
                                    Position::new(line_idx as u32, start_pos as u32),
                                    Position::new(
                                        line_idx as u32,
                                        (start_pos + record.name.len()) as u32,
                                    ),
                                ),
                            ));
                            break;
                        }
                    }
                }
                crate::ast::TopDecl::Binding(binding) => {
                    let binding_name = pattern_symbol_label(&binding.pattern);
                    for (line_idx, line) in lines.iter().enumerate().skip(next_binding_line) {
                        if let Some((start_pos, end_pos)) = binding_pattern_range(line) {
                            next_binding_line = line_idx + 1;
                            symbols.push(document_symbol(
                                binding_name.clone(),
                                Some(if binding.mutable {
                                    "Mutable variable".to_string()
                                } else {
                                    "Immutable variable".to_string()
                                }),
                                SymbolKind::VARIABLE,
                                Range::new(
                                    Position::new(line_idx as u32, 0),
                                    Position::new(line_idx as u32, line.len() as u32),
                                ),
                                Range::new(
                                    Position::new(line_idx as u32, start_pos as u32),
                                    Position::new(line_idx as u32, end_pos as u32),
                                ),
                            ));
                            break;
                        }
                    }
                }
                _ => {} // Handle other declaration types
            }
        }

        symbols
    }

    fn extract_word_at_position(line: &str, char_idx: usize) -> Option<(usize, usize)> {
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

    fn find_references_at_position(
        uri: &Url,
        _ast: &crate::ast::Program,
        text: &str,
        position: &Position,
        include_declaration: bool,
    ) -> Vec<Location> {
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
        if let Some((start, end)) = Self::extract_word_at_position(target_line, char_idx) {
            let word = &target_line[start..end];

            // Search for all occurrences of this word
            for (line_idx, line) in lines.iter().enumerate() {
                let mut search_start = 0;
                while let Some(pos) = line[search_start..].find(word) {
                    let actual_pos = search_start + pos;

                    // Check if this is a whole word match
                    let is_word_boundary = (actual_pos == 0
                        || !line
                            .chars()
                            .nth(actual_pos - 1)
                            .unwrap_or(' ')
                            .is_alphanumeric())
                        && (actual_pos + word.len() >= line.len()
                            || !line
                                .chars()
                                .nth(actual_pos + word.len())
                                .unwrap_or(' ')
                                .is_alphanumeric());

                    if is_word_boundary {
                        // Skip the original position if not including declaration
                        if !include_declaration
                            && line_idx == target_line_idx
                            && actual_pos == start
                        {
                            search_start = actual_pos + word.len();
                            continue;
                        }

                        references.push(Location::new(
                            uri.clone(),
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

    fn generate_semantic_tokens(_ast: &crate::ast::Program, text: &str) -> Vec<SemanticToken> {
        let mut tokens = Vec::new();
        let lines: Vec<&str> = text.lines().collect();
        let mut previous_position: Option<(usize, usize)> = None;

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
                if let Some((token_length, token_type)) =
                    Self::classify_token_at_position(line, char_idx)
                {
                    let (delta_line, delta_start) = match previous_position {
                        Some((previous_line, previous_start)) if previous_line == line_idx => {
                            (0, char_idx.saturating_sub(previous_start) as u32)
                        }
                        Some((previous_line, _)) => {
                            ((line_idx - previous_line) as u32, char_idx as u32)
                        }
                        None => (line_idx as u32, char_idx as u32),
                    };

                    tokens.push(SemanticToken {
                        delta_line,
                        delta_start,
                        length: token_length as u32,
                        token_type,
                        token_modifiers_bitset: 0,
                    });
                    previous_position = Some((line_idx, char_idx));
                    char_idx += token_length;
                } else {
                    char_idx += 1;
                }
            }
        }

        tokens
    }

    fn classify_token_at_position(line: &str, char_idx: usize) -> Option<(usize, u32)> {
        let remaining = &line[char_idx..];

        // Keywords
        let keywords = [
            ("fun", SEMANTIC_TOKEN_KEYWORD),
            ("val", SEMANTIC_TOKEN_KEYWORD),
            ("mut", SEMANTIC_TOKEN_KEYWORD),
            ("then", SEMANTIC_TOKEN_KEYWORD),
            ("else", SEMANTIC_TOKEN_KEYWORD),
            ("while", SEMANTIC_TOKEN_KEYWORD),
            ("match", SEMANTIC_TOKEN_KEYWORD),
            ("with", SEMANTIC_TOKEN_KEYWORD),
            ("clone", SEMANTIC_TOKEN_KEYWORD),
            ("freeze", SEMANTIC_TOKEN_KEYWORD),
            ("record", SEMANTIC_TOKEN_KEYWORD),
            ("true", SEMANTIC_TOKEN_KEYWORD),
            ("false", SEMANTIC_TOKEN_KEYWORD),
            ("Some", SEMANTIC_TOKEN_KEYWORD),
            ("None", SEMANTIC_TOKEN_KEYWORD),
        ];

        for (keyword, token_type) in keywords {
            if remaining.starts_with(keyword) {
                // Check word boundary
                let end_idx = char_idx + keyword.len();
                if end_idx >= line.len()
                    || !line.chars().nth(end_idx).unwrap_or(' ').is_alphanumeric()
                {
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
            return Some((length, SEMANTIC_TOKEN_NUMBER));
        }

        // Strings
        if let Some(stripped) = remaining.strip_prefix('"') {
            let mut length = 1;
            let mut escaped = false;
            for ch in stripped.chars() {
                length += 1;
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    break;
                }
            }
            return Some((length, SEMANTIC_TOKEN_STRING));
        }

        None
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    #[test]
    fn type_error_diagnostic_hides_inference_variable_ids() {
        let message = lsp_type_error_message(&TypeError::CannotInferType("?0".to_string()));

        assert!(message.contains("Cannot infer type"));
        assert!(!message.contains("?0"));
        assert!(!message.contains("InferVar"));
    }

    #[test]
    fn type_error_diagnostic_hides_nested_inference_placeholder_detail() {
        let message =
            lsp_type_error_message(&TypeError::CannotInferType("Option<?12>".to_string()));

        assert!(message.contains("Cannot infer type"));
        assert!(message.contains("type annotation") || message.contains("concrete type"));
        assert!(!message.contains("?12"));
        assert!(!message.contains("an inferred type"));
        assert!(!message.contains("unknown type"));
    }

    #[test]
    fn type_error_diagnostic_preserves_actionable_inference_detail() {
        let none_message = lsp_type_error_message(&TypeError::CannotInferType(
            "None requires an expected Option type".to_string(),
        ));
        let empty_list_message = lsp_type_error_message(&TypeError::CannotInferType(
            "empty list requires an expected List type".to_string(),
        ));
        let lambda_message = lsp_type_error_message(&TypeError::CannotInferType(
            "lambda parameter types require annotations or an expected function type".to_string(),
        ));

        assert!(none_message.contains("None requires an expected Option type"));
        assert!(empty_list_message.contains("empty list requires an expected List type"));
        assert!(lambda_message
            .contains("lambda parameter types require annotations or an expected function type"));
        assert!(!none_message.contains("?0"));
        assert!(!empty_list_message.contains("InferVar"));
        assert!(!lambda_message.contains("Projection"));
    }

    #[test]
    fn type_error_diagnostic_preserves_binding_context_for_unresolved_collection() {
        let message = lsp_type_error_message(&TypeError::CannotInferType(
            "binding 'items' has unresolved type List<?0>".to_string(),
        ));

        assert!(message.contains("binding 'items'"));
        assert!(message.contains("empty list requires an expected List type"));
        assert!(!message.contains("?0"));
        assert!(!message.contains("an inferred type"));
        assert!(!message.contains("InferVar"));
    }

    #[test]
    fn type_error_diagnostic_points_to_unresolved_binding() {
        let source = "fun main: () -> Int32 = {\n    val items = [];\n    0\n}\n";
        let diagnostic = diagnostic_for_type_error(
            source,
            &TypeError::CannotInferType("binding 'items' has unresolved type List<?0>".to_string()),
        );

        assert!(diagnostic.message.contains("binding 'items'"));
        assert_eq!(diagnostic.range.start, Position::new(1, 8));
        assert_eq!(diagnostic.range.end, Position::new(1, 13));
    }

    #[test]
    fn published_diagnostics_preserve_inference_context_and_binding_range() {
        let source = "fun main: () -> Int32 = {\n    val items = [];\n    0\n}\n";
        let uri = Url::parse("file:///tmp/unresolved-list.rl").expect("valid file uri");
        let diagnostics = collect_diagnostics_for_source(&uri, source);

        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert!(diagnostic
            .message
            .contains("Cannot infer type for binding 'items'"));
        assert!(
            diagnostic
                .message
                .contains("empty list requires an expected List type"),
            "diagnostic should preserve the actionable empty-list context: {}",
            diagnostic.message
        );
        assert!(!diagnostic.message.contains("?0"));
        assert_eq!(diagnostic.range.start, Position::new(1, 8));
        assert_eq!(diagnostic.range.end, Position::new(1, 13));
    }

    #[test]
    fn published_diagnostics_include_release_surface_errors() {
        let source = r#"
pub fun release_label: () = {
    "stable"
}

fun main: () -> Int32 = {
    1
}
"#;
        let uri = Url::parse("file:///tmp/release-surface.rl").expect("valid file uri");
        let diagnostics = collect_diagnostics_for_source(&uri, source);

        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert!(
            diagnostic
                .message
                .contains("Release surface error: Exported function 'release_label'"),
            "diagnostic should identify the release surface gate: {}",
            diagnostic.message
        );
        assert!(
            diagnostic
                .message
                .contains("return type String requires a composite host ABI"),
            "diagnostic should explain the unsupported export ABI: {}",
            diagnostic.message
        );
        assert!(
            !diagnostic.message.contains("Type error"),
            "release surface failures should not be mislabeled as type errors: {}",
            diagnostic.message
        );
        assert_eq!(diagnostic.range.start, Position::new(1, 8));
        assert_eq!(diagnostic.range.end, Position::new(1, 21));
    }

    #[tokio::test]
    async fn compile_command_rejects_exported_generic_before_codegen() {
        let source = r#"
pub fun public_identity: <T>(value: T) -> T = {
    value
}

fun main: () -> Int32 = {
    1
}
"#;
        let uri =
            Url::parse("file:///tmp/restrict-lsp-exported-generic.rl").expect("valid file uri");
        let result = execute_lsp_test_command("restrict.lsp.compile", uri, source).await;

        assert_eq!(
            result.get("success").and_then(serde_json::Value::as_bool),
            Some(false)
        );
        let message = result
            .get("message")
            .and_then(serde_json::Value::as_str)
            .expect("command response should include a message");

        assert!(
            message.contains("Release surface error: Exported generic function 'public_identity'"),
            "compile command should report release validation wording: {message}"
        );
        assert!(
            message.contains("requires a concrete ABI"),
            "compile command should explain the unsupported generic ABI: {message}"
        );
        assert!(
            !message.contains("Code generation error"),
            "compile command should fail before codegen: {message}"
        );
        assert!(
            result.get("output").is_none(),
            "release validation failures should not report codegen output: {result}"
        );
    }

    #[tokio::test]
    async fn type_check_command_reports_release_validation_failure() {
        let source = r#"
pub fun release_label: () = {
    "stable"
}

fun main: () -> Int32 = {
    1
}
"#;
        let uri =
            Url::parse("file:///tmp/restrict-lsp-composite-export.rl").expect("valid file uri");
        let result = execute_lsp_test_command("restrict.lsp.typeCheck", uri, source).await;

        assert_eq!(
            result.get("success").and_then(serde_json::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            result
                .get("diagnostics")
                .and_then(serde_json::Value::as_u64),
            Some(1)
        );
        let message = result
            .get("message")
            .and_then(serde_json::Value::as_str)
            .expect("command response should include a message");

        assert!(
            message.contains("release validation failed"),
            "typeCheck command should mention release validation: {message}"
        );
    }

    #[test]
    fn parser_diagnostic_uses_reported_line_and_column_range() {
        let source = "fun main: () -> Int32 = {\n    val answer =\n}\n";
        let diagnostic = diagnostic_for_message(
            source,
            "Parsing error at line 2, column 5: unexpected input near `val answer =`".to_string(),
        );

        assert!(diagnostic.message.contains("Parsing error at line"));
        assert_ne!(
            diagnostic.range,
            Range::new(Position::new(0, 0), Position::new(0, 1))
        );
        assert_eq!(diagnostic.range.start.line, 1);
    }

    #[test]
    fn type_error_diagnostic_hides_projection_details() {
        let message = lsp_type_error_message(&TypeError::UnresolvedProjection(
            "?0 as Container.Mapped<String>".to_string(),
        ));

        assert!(message.contains("Cannot resolve generic collection result type"));
        assert!(!message.contains("?0"));
        assert!(!message.contains("Projection"));
        assert!(!message.contains("Container.Mapped"));
    }

    #[test]
    fn fallback_diagnostic_sanitizer_masks_internal_terms() {
        let message = sanitize_lsp_diagnostic_text(
            "expected InferVar(TypeVarId(?12)), found Projection(?13 as Container.Item)",
        );

        assert_eq!(message, "expected an inferred type, found associated type");
    }

    #[test]
    fn builtin_completion_signatures_use_current_restrict_types() {
        let items = builtin_completion_items();
        let details = items
            .iter()
            .filter_map(|item| item.detail.as_deref())
            .collect::<Vec<_>>();
        let joined_details = details.join("\n");

        assert!(joined_details.contains("fun print_int: (n: Int32) -> ()"));
        assert!(joined_details.contains("fun list_is_empty: <T>(list: List<T>) -> Boolean"));
        assert!(joined_details.contains("fun assert: (condition: Boolean, message: String) -> ()"));

        for item in &items {
            assert_ne!(item.label, "let");
            assert_ne!(item.label, "fn");
        }

        for detail in details {
            assert_no_legacy_public_syntax(detail);
            assert_no_function_first_signature(detail);
        }
    }

    #[test]
    fn pattern_symbol_labels_use_current_restrict_syntax() {
        let record = Pattern::Record(
            "Point".to_string(),
            vec![
                ("x".to_string(), Pattern::Ident("x".to_string())),
                ("y".to_string(), Pattern::Ident("y".to_string())),
            ],
        );
        let spread = Pattern::RecordDestruct {
            type_name: "User".to_string(),
            fields: vec![
                (
                    "role".to_string(),
                    Pattern::Literal(Literal::String("admin".to_string())),
                ),
                ("name".to_string(), Pattern::Ident("name".to_string())),
            ],
            rest: Some("profile".to_string()),
        };
        let list_cons = Pattern::ListCons(
            Box::new(Pattern::Ident("head".to_string())),
            Box::new(Pattern::Ident("tail".to_string())),
        );

        assert_eq!(pattern_symbol_label(&record), "Point { x, y }");
        assert_eq!(
            pattern_symbol_label(&spread),
            "User { role: \"admin\", name, ...profile }"
        );
        assert_eq!(pattern_symbol_label(&list_cons), "[head | tail]");

        for label in [
            pattern_symbol_label(&record),
            pattern_symbol_label(&spread),
            pattern_symbol_label(&list_cons),
        ] {
            assert!(!label.contains("complex_pattern"));
            assert!(!label.contains("tuple"));
            assert!(!label.contains("guard"));
        }
    }

    #[test]
    fn document_symbols_show_binding_patterns_without_placeholders() {
        let text = "val Point { x, y } = point\nval [head | tail] = items\n";
        let (_, ast) = parse_program(text).expect("test program should parse");

        let symbols = RestrictLanguageServer::extract_document_symbols(&ast, text);
        let symbol_names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert!(symbol_names.contains(&"Point { x, y }"));
        assert!(symbol_names.contains(&"[head | tail]"));
        assert!(!symbol_names.contains(&"complex_pattern"));

        for symbol in symbols {
            let line = text
                .lines()
                .nth(symbol.selection_range.start.line as usize)
                .expect("selection range should point at a source line");
            let selected = &line[symbol.selection_range.start.character as usize
                ..symbol.selection_range.end.character as usize];
            assert_eq!(selected, symbol.name);
        }
    }

    #[test]
    fn definition_locations_use_document_uri() {
        let text = "fun double: (x: Int32) -> Int32 = {\n    x\n}\n\nval result = 21 |> double\n";
        let ast = parse_lsp_test_program(text);
        let uri = Url::parse("file:///tmp/restrict/example.rl").unwrap();
        let position = position_of_word(text, "double", 1);

        let location =
            RestrictLanguageServer::find_definition_at_position(&uri, &ast, text, &position)
                .expect("definition should be found");

        assert_eq!(location.uri, uri);
        assert_ne!(location.uri.as_str(), "file://current");
    }

    #[test]
    fn reference_locations_use_document_uri() {
        let text = "fun double: (x: Int32) -> Int32 = {\n    x\n}\n\nval result = 21 |> double\n";
        let ast = parse_lsp_test_program(text);
        let uri = Url::parse("file:///tmp/restrict/example.rl").unwrap();
        let position = position_of_word(text, "double", 1);

        let references =
            RestrictLanguageServer::find_references_at_position(&uri, &ast, text, &position, true);

        assert_eq!(references.len(), 2);
        for reference in references {
            assert_eq!(reference.uri, uri);
            assert_ne!(reference.uri.as_str(), "file://current");
        }
    }

    #[test]
    fn v001_capabilities_do_not_advertise_noop_release_features() {
        let capabilities = restrict_server_capabilities();

        assert!(capabilities.definition_provider.is_some());
        assert!(capabilities.references_provider.is_some());
        assert!(capabilities.hover_provider.is_some());
        assert!(capabilities.completion_provider.is_some());
        assert!(capabilities.semantic_tokens_provider.is_some());
        assert!(capabilities.document_symbol_provider.is_some());

        assert!(capabilities.document_highlight_provider.is_none());
        assert!(capabilities.workspace_symbol_provider.is_none());
        assert!(capabilities.code_action_provider.is_none());
        assert!(capabilities.code_lens_provider.is_none());
        assert!(capabilities.document_formatting_provider.is_none());
        assert!(capabilities.document_range_formatting_provider.is_none());
        assert!(capabilities.document_on_type_formatting_provider.is_none());
        assert!(capabilities.rename_provider.is_none());
        assert!(capabilities.declaration_provider.is_none());
        assert!(capabilities.signature_help_provider.is_none());

        let semantic_options = semantic_token_options(&capabilities);
        assert!(
            semantic_options.range.is_none(),
            "semantic token range requests are not implemented in v0.0.1"
        );
        assert_eq!(
            semantic_options.legend.token_types[SEMANTIC_TOKEN_KEYWORD as usize],
            SemanticTokenType::KEYWORD
        );
    }

    #[test]
    fn semantic_token_type_constants_match_server_legend() {
        let capabilities = restrict_server_capabilities();
        let legend = semantic_token_legend(&capabilities);

        assert_eq!(
            legend.token_types[SEMANTIC_TOKEN_KEYWORD as usize],
            SemanticTokenType::KEYWORD
        );
        assert_eq!(
            legend.token_types[SEMANTIC_TOKEN_STRING as usize],
            SemanticTokenType::STRING
        );
        assert_eq!(
            legend.token_types[SEMANTIC_TOKEN_NUMBER as usize],
            SemanticTokenType::NUMBER
        );
        assert_eq!(
            legend.token_types[SEMANTIC_TOKEN_OPERATOR as usize],
            SemanticTokenType::OPERATOR
        );
    }

    #[test]
    fn semantic_tokens_decode_to_absolute_positions_across_lines() {
        let text =
            "fun id: (x: Int32) -> Int32 = {\n    x\n}\nval answer = 42\nval message = \"ok\"\n";
        let ast = parse_lsp_test_program(text);

        let tokens = RestrictLanguageServer::generate_semantic_tokens(&ast, text);
        let decoded = decode_semantic_tokens(&tokens);

        assert!(decoded.contains(&(0, 0, 3, SEMANTIC_TOKEN_KEYWORD)));
        assert!(decoded.contains(&(3, 0, 3, SEMANTIC_TOKEN_KEYWORD)));
        assert!(decoded.contains(&(3, 13, 2, SEMANTIC_TOKEN_NUMBER)));
        assert!(decoded.contains(&(4, 0, 3, SEMANTIC_TOKEN_KEYWORD)));
        assert!(decoded.contains(&(4, 14, 4, SEMANTIC_TOKEN_STRING)));
    }

    fn assert_no_legacy_public_syntax(text: &str) {
        for word in ["let", "fn", "Unit", "Bool"] {
            assert!(
                !contains_word(text, word),
                "legacy public syntax `{word}` in LSP text: {text}"
            );
        }
    }

    fn assert_no_function_first_signature(detail: &str) {
        let Some((name, _signature)) = detail
            .strip_prefix("fun ")
            .and_then(|rest| rest.split_once(':'))
        else {
            return;
        };

        assert!(
            !detail.contains(&format!("{}(", name.trim())),
            "function-first signature in LSP detail: {detail}"
        );
    }

    fn contains_word(text: &str, word: &str) -> bool {
        text.match_indices(word).any(|(index, _)| {
            let before = text[..index].chars().next_back();
            let after = text[index + word.len()..].chars().next();

            before.is_none_or(|char| !is_word_char(char))
                && after.is_none_or(|char| !is_word_char(char))
        })
    }

    fn is_word_char(char: char) -> bool {
        char == '_' || char.is_ascii_alphanumeric()
    }

    fn parse_lsp_test_program(text: &str) -> crate::ast::Program {
        let (remaining, ast) = parse_program(text).expect("test program should parse");
        assert!(
            remaining.trim().is_empty(),
            "test program left unparsed input: {remaining:?}"
        );
        ast
    }

    async fn execute_lsp_test_command(command: &str, uri: Url, source: &str) -> serde_json::Value {
        let (service, _socket) = LspService::new(RestrictLanguageServer::new);
        service
            .inner()
            .documents
            .write()
            .unwrap()
            .insert(uri.clone(), source.to_string());

        service
            .inner()
            .execute_command(ExecuteCommandParams {
                command: command.to_string(),
                arguments: vec![serde_json::Value::String(uri.to_string())],
                work_done_progress_params: WorkDoneProgressParams::default(),
            })
            .await
            .expect("execute command should not return a JSON-RPC error")
            .expect("execute command should return a JSON value")
    }

    fn position_of_word(text: &str, word: &str, occurrence: usize) -> Position {
        let mut seen = 0;
        for (line_index, line) in text.lines().enumerate() {
            let mut search_start = 0;
            while let Some(offset) = line[search_start..].find(word) {
                if seen == occurrence {
                    return Position::new(line_index as u32, (search_start + offset) as u32);
                }
                seen += 1;
                search_start += offset + word.len();
            }
        }

        panic!("word `{word}` occurrence {occurrence} not found in test source");
    }

    fn semantic_token_legend(capabilities: &ServerCapabilities) -> &SemanticTokensLegend {
        &semantic_token_options(capabilities).legend
    }

    fn semantic_token_options(capabilities: &ServerCapabilities) -> &SemanticTokensOptions {
        match capabilities
            .semantic_tokens_provider
            .as_ref()
            .expect("semantic tokens should be advertised")
        {
            SemanticTokensServerCapabilities::SemanticTokensOptions(options) => options,
            SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(options) => {
                &options.semantic_tokens_options
            }
        }
    }

    fn decode_semantic_tokens(tokens: &[SemanticToken]) -> Vec<(u32, u32, u32, u32)> {
        let mut decoded = Vec::new();
        let mut line = 0;
        let mut start = 0;

        for token in tokens {
            line += token.delta_line;
            if token.delta_line == 0 {
                start += token.delta_start;
            } else {
                start = token.delta_start;
            }
            decoded.push((line, start, token.length, token.token_type));
        }

        decoded
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for RestrictLanguageServer {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: restrict_server_capabilities(),
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
            .log_message(
                MessageType::INFO,
                format!("File saved: {}", params.text_document.uri),
            )
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
            if let Ok(ast) = self.parse_and_resolve_program(uri, text) {
                let mut type_checker = TypeChecker::new();
                if type_checker.check_program(&ast).is_ok() {
                    if let Some(hover_info) = self.get_hover_info(&ast, text, position) {
                        return Ok(Some(Hover {
                            contents: HoverContents::Scalar(MarkedString::LanguageString(
                                LanguageString {
                                    language: "restrict".to_string(),
                                    value: hover_info,
                                },
                            )),
                            range: None,
                        }));
                    }
                }
            }
        }

        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let mut completions = builtin_completion_items();

        // Context-sensitive completions
        let uri = &params.text_document_position.text_document.uri;
        let documents = self.documents.read().unwrap();
        if let Some(text) = documents.get(uri) {
            if let Ok(ast) = self.parse_and_resolve_program(uri, text) {
                // Add symbols from current document
                for decl in &ast.declarations {
                    match decl {
                        crate::ast::TopDecl::Function(func) => {
                            if func.name.starts_with("__rl_mod_") {
                                continue;
                            }
                            completions.push(CompletionItem::new_simple(
                                func.name.clone(),
                                format!("User function with {} parameters", func.params.len()),
                            ));
                        }
                        crate::ast::TopDecl::Record(record) => {
                            if record.name.starts_with("__rl_mod_") {
                                continue;
                            }
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
            if let Ok(ast) = self.parse_and_resolve_program(uri, text) {
                if let Some(location) = Self::find_definition_at_position(uri, &ast, text, position)
                {
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
            if let Ok(ast) = self.parse_and_resolve_program(uri, text) {
                let references = Self::find_references_at_position(
                    uri,
                    &ast,
                    text,
                    position,
                    params.context.include_declaration,
                );
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
            if let Ok(ast) = self.parse_and_resolve_program(uri, text) {
                let symbols = Self::extract_document_symbols(&ast, text);
                if !symbols.is_empty() {
                    return Ok(Some(DocumentSymbolResponse::Nested(symbols)));
                }
            }
        }

        Ok(None)
    }

    async fn symbol(&self, _: WorkspaceSymbolParams) -> Result<Option<Vec<SymbolInformation>>> {
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
            if let Ok(ast) = self.parse_and_resolve_program(uri, text) {
                let tokens = Self::generate_semantic_tokens(&ast, text);
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

    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> Result<Option<serde_json::Value>> {
        match params.command.as_str() {
            "restrict.lsp.compile" => {
                // Get the active document URI from arguments
                if let Some(args) = params.arguments.first() {
                    if let Some(uri_str) = args.as_str() {
                        if let Ok(uri) = Url::parse(uri_str) {
                            let text = {
                                let documents = self.documents.read().unwrap();
                                documents.get(&uri).cloned()
                            };

                            if let Some(text) = text {
                                // Parse and resolve imports before type checking/codegen.
                                match self.parse_and_resolve_program(&uri, &text) {
                                    Ok(ast) => {
                                        // Type check
                                        let mut type_checker = crate::TypeChecker::new();
                                        if let Err(e) = type_checker.check_program(&ast) {
                                            return Ok(Some(serde_json::json!({
                                                "success": false,
                                                "message": lsp_type_error_message(&e)
                                            })));
                                        }
                                        if let Err(e) =
                                            check_v001_release_surface(&ast, &type_checker)
                                        {
                                            return Ok(Some(serde_json::json!({
                                                "success": false,
                                                "message": format!("Release surface error: {}", e)
                                            })));
                                        }

                                        // Generate WASM
                                        let (wasm_result, has_globals) = {
                                            let mut codegen = crate::WasmCodeGen::new();
                                            let result = codegen.generate(&ast);
                                            let has_globals = ast.declarations.iter().any(|d| {
                                                matches!(d, crate::ast::TopDecl::Binding(_))
                                            });
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
                                                                format!("Compilation successful with global bindings emitted. Output: {}", wat_path.display())
                                                            } else {
                                                                format!("Compilation successful. Output: {}", wat_path.display())
                                                            };

                                                            self.client
                                                                .log_message(
                                                                    MessageType::INFO,
                                                                    format!(
                                                                        "Saved WAT to: {}",
                                                                        wat_path.display()
                                                                    ),
                                                                )
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
                                    Err(message) => {
                                        return Ok(Some(serde_json::json!({
                                            "success": false,
                                            "message": message
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
                if let Some(args) = params.arguments.first() {
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
                                    "message": if success { "Check and release validation passed" } else { "Check or release validation failed" },
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
                if let Some(args) = params.arguments.first() {
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
                                            "message": format_parse_error(&text, e)
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

    let (service, socket) = LspService::new(RestrictLanguageServer::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
