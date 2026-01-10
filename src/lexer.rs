//! # Lexer Module
//!
//! The lexer tokenizes Restrict Language source code using the nom parser combinator library.
//! It handles the unique OSV syntax, including the pipe operator `|>`, and supports
//! affine type annotations and prototype-based features.
//!
//! ## Token Categories
//!
//! - **Keywords**: Language reserved words (`fn`, `let`, `clone`, `freeze`, etc.)
//! - **Operators**: Including the distinctive pipe operators (`|>`, `|>>`)
//! - **Literals**: Numbers, strings, characters, booleans
//! - **Identifiers**: Variable and function names
//! - **Delimiters**: Parentheses, braces, brackets
//!
//! ## Example
//!
//! ```rust
//! use restrict_lang::lexer::tokenize;
//!
//! let input = r#""Hello, World!" |> println;"#;
//! let tokens = tokenize(input).unwrap();
//! ```

use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, take_while1, take_while, take_until},
    character::complete::{char, digit1, one_of},
    combinator::{recognize, map, value},
    multi::many0,
    sequence::{pair, preceded, delimited},
};
use std::fmt;

/// Represents a position in source code as a byte offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    /// Start byte offset (inclusive)
    pub start: usize,
    /// End byte offset (exclusive)
    pub end: usize,
}

impl Span {
    /// Creates a new span from start and end byte offsets.
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Creates a span covering a single point.
    pub fn point(offset: usize) -> Self {
        Self { start: offset, end: offset }
    }

    /// Creates a span that combines two spans (from start of first to end of second).
    pub fn merge(self, other: Span) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Returns the length of this span in bytes.
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Returns true if this span has zero length.
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    /// Converts byte offset to line and column (0-indexed).
    pub fn to_line_col(&self, source: &str) -> (usize, usize) {
        let mut line = 0;
        let mut col = 0;
        for (i, ch) in source.char_indices() {
            if i >= self.start {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    /// Converts span to line/column range for LSP.
    pub fn to_line_col_range(&self, source: &str) -> ((usize, usize), (usize, usize)) {
        let start_pos = Self { start: self.start, end: self.start }.to_line_col(source);
        let end_pos = Self { start: self.end, end: self.end }.to_line_col(source);
        (start_pos, end_pos)
    }
}

/// A token with its span information.
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    /// The token value
    pub token: Token,
    /// The span in source code
    pub span: Span,
}

impl SpannedToken {
    /// Creates a new spanned token.
    pub fn new(token: Token, span: Span) -> Self {
        Self { token, span }
    }
}

/// Token types in Restrict Language.
/// 
/// Each token represents a lexical unit in the source code.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    /// `record` keyword for type declarations
    Record,
    /// `clone` keyword for cloning values
    Clone,
    /// `freeze` keyword for creating immutable prototypes
    Freeze,
    /// `impl` keyword for implementation blocks
    Impl,
    /// `context` keyword for context declarations
    Context,
    /// `with` keyword for resource management
    With,
    /// `fn` keyword for function declarations
    Fun,
    /// `let` keyword for bindings
    Val,
    /// `mut` keyword for mutable bindings
    Mut,
    /// `if` keyword (mapped from `then`)
    Then,
    /// `else` keyword
    Else,
    /// `while` keyword for loops
    While,
    /// `match` keyword for pattern matching
    Match,
    /// `async` keyword for asynchronous functions
    Async,
    /// `return` keyword
    Return,
    /// `true` boolean literal
    True,
    /// `false` boolean literal
    False,
    /// Unit type `()`
    Unit,
    /// `Some` variant constructor
    Some,
    /// `None` variant constructor
    None,
    /// `import` keyword
    Import,
    /// `export` keyword
    Export,
    /// `sealed` modifier for records
    Sealed,
    /// `from` keyword for derivation bounds
    From,
    /// `within` keyword for temporal constraints
    Within,
    /// `where` keyword for constraints
    Where,
    /// `lifetime` keyword for temporal scope
    Lifetime,
    /// `await` keyword for async operations
    Await,
    /// `spawn` keyword for spawning tasks
    Spawn,
    /// `it` keyword for implicit lambda parameter
    It,

    // Identifiers and Literals
    /// Identifier (variable/function name)
    Ident(String),
    /// Integer literal
    IntLit(i32),
    /// Floating-point literal
    FloatLit(f64),
    /// String literal
    StringLit(String),
    /// Character literal
    CharLit(char),
    
    // Operators
    /// Pipe operator `|>` for OSV syntax
    Pipe,
    /// Mutable pipe operator `|>>`
    PipeMut,
    /// Vertical bar `|` for patterns
    Bar,
    Assign,         // =
    Arrow,          // =>
    ThinArrow,      // ->
    Plus,           // +
    Minus,          // -
    Star,           // *
    Asterisk,       // * (for import *)
    Slash,          // /
    Percent,        // %
    Eq,             // ==
    Ne,             // !=
    Lt,             // <
    Le,             // <=
    Gt,             // >
    Ge,             // >=
    
    // Temporal
    Tilde,          // ~ (for temporal type variables)
    
    // Delimiters
    LBrace,         // {
    RBrace,         // }
    LParen,         // (
    RParen,         // )
    LBracket,       // [
    RBracket,       // ]
    LArrayBracket,  // [|
    RArrayBracket,  // |]
    Comma,          // ,
    Colon,          // :
    Dot,            // .
    Semicolon,      // ;
    
    // Special
    /// Newline token for statement termination
    Newline,
    Eof,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Record => write!(f, "record"),
            Token::Clone => write!(f, "clone"),
            Token::Freeze => write!(f, "freeze"),
            Token::Impl => write!(f, "impl"),
            Token::Context => write!(f, "context"),
            Token::With => write!(f, "with"),
            Token::Fun => write!(f, "fun"),
            Token::Val => write!(f, "val"),
            Token::Mut => write!(f, "mut"),
            Token::Then => write!(f, "then"),
            Token::Else => write!(f, "else"),
            Token::While => write!(f, "while"),
            Token::Match => write!(f, "match"),
            Token::Async => write!(f, "async"),
            Token::Return => write!(f, "return"),
            Token::True => write!(f, "true"),
            Token::False => write!(f, "false"),
            Token::Unit => write!(f, "Unit"),
            Token::Some => write!(f, "Some"),
            Token::None => write!(f, "None"),
            Token::Import => write!(f, "import"),
            Token::Export => write!(f, "export"),
            Token::Sealed => write!(f, "sealed"),
            Token::From => write!(f, "from"),
            Token::Within => write!(f, "within"),
            Token::Where => write!(f, "where"),
            Token::Lifetime => write!(f, "lifetime"),
            Token::Await => write!(f, "await"),
            Token::Spawn => write!(f, "spawn"),
            Token::It => write!(f, "it"),
            Token::Ident(s) => write!(f, "{}", s),
            Token::IntLit(n) => write!(f, "{}", n),
            Token::FloatLit(n) => write!(f, "{}", n),
            Token::StringLit(s) => write!(f, "\"{}\"", s),
            Token::CharLit(c) => write!(f, "'{}'", c),
            Token::Pipe => write!(f, "|>"),
            Token::PipeMut => write!(f, "|>>"),
            Token::Bar => write!(f, "|"),
            Token::Assign => write!(f, "="),
            Token::Arrow => write!(f, "=>"),
            Token::ThinArrow => write!(f, "->"),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Star => write!(f, "*"),
            Token::Asterisk => write!(f, "*"),
            Token::Slash => write!(f, "/"),
            Token::Percent => write!(f, "%"),
            Token::Eq => write!(f, "=="),
            Token::Ne => write!(f, "!="),
            Token::Lt => write!(f, "<"),
            Token::Le => write!(f, "<="),
            Token::Gt => write!(f, ">"),
            Token::Ge => write!(f, ">="),
            Token::Tilde => write!(f, "~"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::LArrayBracket => write!(f, "[|"),
            Token::RArrayBracket => write!(f, "|]"),
            Token::Comma => write!(f, ","),
            Token::Colon => write!(f, ":"),
            Token::Dot => write!(f, "."),
            Token::Semicolon => write!(f, ";"),
            Token::Newline => write!(f, "<newline>"),
            Token::Eof => write!(f, "EOF"),
        }
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn identifier(input: &str) -> IResult<&str, &str> {
    recognize(
        pair(
            take_while1(|c: char| is_ident_start(c)),
            take_while(|c: char| is_ident_continue(c))
        )
    )(input)
}

fn keyword(input: &str) -> IResult<&str, Token> {
    let ident = identifier(input)?;
    let token = match ident.1 {
        "record" => Token::Record,
        "clone" => Token::Clone,
        "freeze" => Token::Freeze,
        "impl" => Token::Impl,
        "context" => Token::Context,
        "with" => Token::With,
        "fun" => Token::Fun,
        "val" => Token::Val,
        "mut" => Token::Mut,
        "then" => Token::Then,
        "else" => Token::Else,
        "while" => Token::While,
        "match" => Token::Match,
        "async" => Token::Async,
        "return" => Token::Return,
        "true" => Token::True,
        "false" => Token::False,
        "Unit" => Token::Unit,
        "Some" => Token::Some,
        "None" => Token::None,
        "none" => Token::None,  // Allow lowercase for inference
        "import" => Token::Import,
        "export" => Token::Export,
        "sealed" => Token::Sealed,
        "from" => Token::From,
        "within" => Token::Within,
        "where" => Token::Where,
        "lifetime" => Token::Lifetime,
        "await" => Token::Await,
        "spawn" => Token::Spawn,
        "it" => Token::It,
        _ => return Ok((ident.0, Token::Ident(ident.1.to_string()))),
    };
    Ok((ident.0, token))
}

fn integer(input: &str) -> IResult<&str, Token> {
    map(
        digit1,
        |s: &str| Token::IntLit(s.parse().unwrap())
    )(input)
}

fn float(input: &str) -> IResult<&str, Token> {
    map(
        recognize(
            pair(
                digit1,
                pair(
                    char('.'),
                    digit1
                )
            )
        ),
        |s: &str| Token::FloatLit(s.parse().unwrap())
    )(input)
}

fn string_lit(input: &str) -> IResult<&str, Token> {
    map(
        delimited(
            char('"'),
            take_while(|c| c != '"'),  // Simplified for now
            char('"')
        ),
        |s: &str| Token::StringLit(s.to_string())
    )(input)
}

fn char_lit(input: &str) -> IResult<&str, Token> {
    map(
        delimited(
            char('\''),
            alt((
                preceded(char('\\'), one_of("'\\nrt")),
                one_of("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 !@#$%^&*()_+-=[]{}|;:,.<>?/~`")
            )),
            char('\'')
        ),
        Token::CharLit
    )(input)
}

fn operator(input: &str) -> IResult<&str, Token> {
    alt((
        value(Token::PipeMut, tag("|>>")),
        value(Token::Pipe, tag("|>")),
        value(Token::RArrayBracket, tag("|]")),  // Check |] before |
        value(Token::Bar, tag("|")),
        value(Token::LArrayBracket, tag("[|")),  // Check [| 
        value(Token::ThinArrow, tag("->")),
        value(Token::Arrow, tag("=>")),
        value(Token::Eq, tag("==")),
        value(Token::Ne, tag("!=")),
        value(Token::Le, tag("<=")),
        value(Token::Ge, tag(">=")),
        value(Token::Assign, tag("=")),
        value(Token::Plus, tag("+")),
        value(Token::Minus, tag("-")),
        value(Token::Star, tag("*")),
        value(Token::Slash, tag("/")),
        value(Token::Percent, tag("%")),
        value(Token::Lt, tag("<")),
        value(Token::Gt, tag(">")),
        value(Token::Tilde, tag("~")),
    ))(input)
}

fn delimiter(input: &str) -> IResult<&str, Token> {
    alt((
        value(Token::LBrace, char('{')),
        value(Token::RBrace, char('}')),
        value(Token::LParen, char('(')),
        value(Token::RParen, char(')')),
        value(Token::LBracket, char('[')),
        value(Token::RBracket, char(']')),
        value(Token::Comma, char(',')),
        value(Token::Colon, char(':')),
        value(Token::Dot, char('.')),
        value(Token::Semicolon, char(';')),
    ))(input)
}

fn token(input: &str) -> IResult<&str, Token> {
    alt((
        float,
        integer,
        keyword,
        string_lit,
        char_lit,
        operator,
        delimiter,
    ))(input)
}

fn whitespace(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_whitespace())(input)
}

/// Parse only non-newline whitespace (spaces, tabs)
fn non_newline_whitespace(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_whitespace() && c != '\n' && c != '\r')(input)
}

/// Parse a newline (LF or CRLF)
fn newline(input: &str) -> IResult<&str, &str> {
    alt((
        tag("\r\n"),
        tag("\n"),
    ))(input)
}

fn comment(input: &str) -> IResult<&str, &str> {
    alt((
        // Single line comment
        recognize(pair(tag("//"), take_while(|c| c != '\n'))),
        // Multi-line comment - simplified version (no nested comments)
        recognize(
            delimited(
                tag("/*"),
                take_until("*/"),
                tag("*/")
            )
        ),
    ))(input)
}

pub fn skip(input: &str) -> IResult<&str, ()> {
    let mut input = input;
    loop {
        if let Ok((rest, _)) = alt((whitespace, comment))(input) {
            input = rest;
        } else {
            break;
        }
    }
    Ok((input, ()))
}

/// Skip whitespace and comments, but NOT newlines.
/// Used for newline-sensitive parsing.
pub fn skip_non_newline(input: &str) -> IResult<&str, ()> {
    let mut input = input;
    loop {
        // Skip non-newline whitespace
        if let Ok((rest, _)) = non_newline_whitespace(input) {
            input = rest;
            continue;
        }
        // Skip comments (but not the newline at the end of single-line comments)
        if let Ok((rest, _)) = comment(input) {
            input = rest;
            continue;
        }
        break;
    }
    Ok((input, ()))
}

pub fn lex_token(input: &str) -> IResult<&str, Token> {
    preceded(skip, token)(input)
}

/// Lex a token without skipping newlines.
/// Returns either a regular token or a Newline token.
pub fn lex_token_newline_aware(input: &str) -> IResult<&str, Token> {
    // First skip non-newline whitespace and comments
    let (input, _) = skip_non_newline(input)?;

    // Check if we're at a newline
    if let Ok((rest, _)) = newline(input) {
        return Ok((rest, Token::Newline));
    }

    // Otherwise, lex a normal token
    token(input)
}

/// Checks if a token should suppress the following newline.
/// These are tokens after which a newline should be ignored (continuation).
fn suppresses_following_newline(token: &Token) -> bool {
    matches!(token,
        // Binary operators
        Token::Plus | Token::Minus | Token::Star | Token::Slash | Token::Percent |
        // Comparison operators
        Token::Eq | Token::Ne | Token::Lt | Token::Le | Token::Gt | Token::Ge |
        // Assignment and arrow operators
        Token::Assign | Token::Arrow | Token::ThinArrow |
        // Pipe operators
        Token::Pipe | Token::PipeMut | Token::Bar |
        // Opening delimiters
        Token::LBrace | Token::LParen | Token::LBracket | Token::LArrayBracket |
        // Comma and colon (continuation in lists, type annotations)
        Token::Comma | Token::Colon |
        // Keywords that expect something to follow
        Token::Fun | Token::Val | Token::Mut | Token::Record | Token::Context |
        Token::Impl | Token::Import | Token::Export | Token::With | Token::Clone |
        Token::Match | Token::Then | Token::Else | Token::While | Token::Where |
        Token::From | Token::Within | Token::Async | Token::Return |
        // Tilde (for temporal types)
        Token::Tilde
    )
}

/// Checks if a token should suppress the preceding newline.
/// These are tokens before which a newline should be ignored.
fn suppresses_preceding_newline(token: &Token) -> bool {
    matches!(token,
        // Closing delimiters
        Token::RBrace | Token::RParen | Token::RBracket | Token::RArrayBracket |
        // These can continue a previous line
        Token::Else | Token::Match | Token::Then | Token::While |
        // Operators that can appear at start of continuation line
        Token::Pipe | Token::PipeMut |
        // Dot for method chaining
        Token::Dot
    )
}

/// Lexes input into tokens with newline-sensitive tokenization.
///
/// This function implements Kotlin-style newline handling:
/// - Newlines are emitted as tokens
/// - Newlines after operators, open brackets, etc. are suppressed
/// - Newlines before closing brackets, else, etc. are suppressed
///
/// The resulting token stream can be used by the parser to determine
/// statement boundaries without requiring semicolons.
pub fn lex_newline_aware(input: &str) -> Result<Vec<Token>, String> {
    let mut remaining = input;
    let mut tokens: Vec<Token> = Vec::new();

    loop {
        // Skip non-newline whitespace
        if let Ok((rest, _)) = skip_non_newline(remaining) {
            remaining = rest;
        }

        if remaining.is_empty() {
            break;
        }

        // Try to lex a token (including newline)
        match lex_token_newline_aware(remaining) {
            Ok((rest, Token::Newline)) => {
                // Check if we should suppress this newline based on previous token
                let suppress_after_prev = tokens.last()
                    .map(suppresses_following_newline)
                    .unwrap_or(true); // Suppress at start of file

                // Skip any additional newlines and whitespace to find the next actual token
                // This collapses multiple consecutive newlines into one
                let mut peek_rest = rest;
                loop {
                    if let Ok((r, _)) = skip_non_newline(peek_rest) {
                        peek_rest = r;
                    }
                    if let Ok((r, _)) = newline(peek_rest) {
                        peek_rest = r;
                    } else {
                        break;
                    }
                }

                // Check if we should suppress this newline based on next token
                let suppress_before_next = if !peek_rest.is_empty() {
                    if let Ok((_, next_tok)) = token(peek_rest) {
                        suppresses_preceding_newline(&next_tok)
                    } else {
                        false
                    }
                } else {
                    true // Suppress at end of file
                };

                // Only emit newline if not suppressed
                if !suppress_after_prev && !suppress_before_next {
                    tokens.push(Token::Newline);
                }

                // Skip past all the newlines we just processed (collapse multiple into one)
                remaining = peek_rest;
            }
            Ok((rest, tok)) => {
                tokens.push(tok);
                remaining = rest;
            }
            Err(e) => {
                return Err(format!("Lexing error at: {}", &remaining[..remaining.len().min(20)]));
            }
        }
    }

    Ok(tokens)
}

/// Lexes input into spanned tokens with newline awareness.
pub fn lex_spanned_newline_aware(input: &str) -> Result<Vec<SpannedToken>, (String, Span)> {
    let original_len = input.len();
    let mut remaining = input;
    let mut tokens: Vec<SpannedToken> = Vec::new();

    loop {
        // Skip non-newline whitespace
        if let Ok((rest, _)) = skip_non_newline(remaining) {
            remaining = rest;
        }

        if remaining.is_empty() {
            break;
        }

        let start = original_len - remaining.len();

        // Try to lex a token (including newline)
        match lex_token_newline_aware(remaining) {
            Ok((rest, Token::Newline)) => {
                let end = original_len - rest.len();

                // Check if we should suppress this newline based on previous token
                let suppress_after_prev = tokens.last()
                    .map(|st| suppresses_following_newline(&st.token))
                    .unwrap_or(true);

                // Skip any additional newlines and whitespace to find the next actual token
                // This collapses multiple consecutive newlines into one
                let mut peek_rest = rest;
                loop {
                    if let Ok((r, _)) = skip_non_newline(peek_rest) {
                        peek_rest = r;
                    }
                    if let Ok((r, _)) = newline(peek_rest) {
                        peek_rest = r;
                    } else {
                        break;
                    }
                }

                let suppress_before_next = if !peek_rest.is_empty() {
                    if let Ok((_, next_tok)) = token(peek_rest) {
                        suppresses_preceding_newline(&next_tok)
                    } else {
                        false
                    }
                } else {
                    true
                };

                if !suppress_after_prev && !suppress_before_next {
                    tokens.push(SpannedToken::new(Token::Newline, Span::new(start, end)));
                }

                // Skip past all the newlines we just processed (collapse multiple into one)
                remaining = peek_rest;
            }
            Ok((rest, tok)) => {
                let end = original_len - rest.len();
                tokens.push(SpannedToken::new(tok, Span::new(start, end)));
                remaining = rest;
            }
            Err(_) => {
                return Err((
                    format!("Unexpected input: '{}'", remaining.chars().take(20).collect::<String>()),
                    Span::new(start, start + remaining.len().min(20))
                ));
            }
        }
    }

    Ok(tokens)
}

pub fn lex(input: &str) -> IResult<&str, Vec<Token>> {
    let (input, _) = skip(input)?;  // Skip initial whitespace
    let (input, tokens) = many0(lex_token)(input)?;
    let (input, _) = skip(input)?;  // Skip trailing whitespace
    Ok((input, tokens))
}

// Wrapper function that tokenizes the entire input or returns an error
pub fn lex_tokens(input: &str) -> Result<Vec<Token>, String> {
    match lex(input) {
        Ok((remaining, tokens)) => {
            if !remaining.is_empty() {
                Err(format!("Unexpected input at: {}", remaining))
            } else {
                Ok(tokens)
            }
        }
        Err(e) => Err(format!("Lexing error: {:?}", e)),
    }
}

/// Lexes input and returns tokens with span information.
///
/// This is the preferred function for LSP and error reporting as it
/// preserves source location information.
pub fn lex_spanned(input: &str) -> IResult<&str, Vec<SpannedToken>> {
    let original = input;
    let original_len = input.len();

    let mut remaining = input;
    let mut tokens = Vec::new();

    loop {
        // Skip whitespace and comments
        let (after_skip, _) = skip(remaining)?;
        remaining = after_skip;

        if remaining.is_empty() {
            break;
        }

        // Calculate current position
        let start = original_len - remaining.len();

        // Try to lex a token
        match token(remaining) {
            Ok((rest, tok)) => {
                let end = original_len - rest.len();
                tokens.push(SpannedToken::new(tok, Span::new(start, end)));
                remaining = rest;
            }
            Err(_) => {
                // Return what we have so far
                break;
            }
        }
    }

    Ok((remaining, tokens))
}

/// Lexes input and returns spanned tokens or an error with position.
pub fn lex_spanned_tokens(input: &str) -> Result<Vec<SpannedToken>, (String, Span)> {
    let original_len = input.len();

    match lex_spanned(input) {
        Ok((remaining, tokens)) => {
            if !remaining.trim().is_empty() {
                let pos = original_len - remaining.len();
                Err((
                    format!("Unexpected input: '{}'", remaining.chars().take(20).collect::<String>()),
                    Span::new(pos, pos + remaining.len().min(20))
                ))
            } else {
                Ok(tokens)
            }
        }
        Err(e) => Err((format!("Lexing error: {:?}", e), Span::new(0, 0))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keywords() {
        assert_eq!(lex("record").unwrap().1, vec![Token::Record]);
        assert_eq!(lex("fun").unwrap().1, vec![Token::Fun]);
        assert_eq!(lex("val").unwrap().1, vec![Token::Val]);
    }

    #[test]
    fn test_spanned_tokens() {
        let input = "val x = 42";
        let tokens = lex_spanned_tokens(input).unwrap();

        assert_eq!(tokens.len(), 4);

        // "val" at position 0-3
        assert_eq!(tokens[0].token, Token::Val);
        assert_eq!(tokens[0].span, Span::new(0, 3));

        // "x" at position 4-5
        assert_eq!(tokens[1].token, Token::Ident("x".to_string()));
        assert_eq!(tokens[1].span, Span::new(4, 5));

        // "=" at position 6-7
        assert_eq!(tokens[2].token, Token::Assign);
        assert_eq!(tokens[2].span, Span::new(6, 7));

        // "42" at position 8-10
        assert_eq!(tokens[3].token, Token::IntLit(42));
        assert_eq!(tokens[3].span, Span::new(8, 10));
    }

    #[test]
    fn test_span_line_col() {
        let source = "val x = 42\nval y = 10";
        let span1 = Span::new(0, 3);  // "val" on line 0
        let span2 = Span::new(11, 14); // "val" on line 1

        assert_eq!(span1.to_line_col(source), (0, 0));
        assert_eq!(span2.to_line_col(source), (1, 0));
    }

    #[test]
    fn test_span_merge() {
        let span1 = Span::new(0, 5);
        let span2 = Span::new(10, 15);
        let merged = span1.merge(span2);
        assert_eq!(merged, Span::new(0, 15));
    }

    #[test]
    fn test_identifiers() {
        assert_eq!(lex("hello").unwrap().1, vec![Token::Ident("hello".to_string())]);
        assert_eq!(lex("_test123").unwrap().1, vec![Token::Ident("_test123".to_string())]);
    }

    #[test]
    fn test_operators() {
        assert_eq!(lex("|>").unwrap().1, vec![Token::Pipe]);
        assert_eq!(lex("|>>").unwrap().1, vec![Token::PipeMut]);
        assert_eq!(lex("=>").unwrap().1, vec![Token::Arrow]);
    }

    #[test]
    fn test_complex_expression() {
        let tokens = lex("val x = 42 |> add 10").unwrap().1;
        assert_eq!(tokens, vec![
            Token::Val,
            Token::Ident("x".to_string()),
            Token::Assign,
            Token::IntLit(42),
            Token::Pipe,
            Token::Ident("add".to_string()),
            Token::IntLit(10),
        ]);
    }
    
    #[test]
    fn test_array_brackets() {
        let tokens = lex("[|1, 2, 3|]").unwrap().1;
        assert_eq!(tokens, vec![
            Token::LArrayBracket,
            Token::IntLit(1),
            Token::Comma,
            Token::IntLit(2),
            Token::Comma,
            Token::IntLit(3),
            Token::RArrayBracket,
        ]);
    }
    
    #[test]
    fn test_temporal_tilde() {
        let tokens = lex("record File<~f> { }").unwrap().1;
        assert_eq!(tokens, vec![
            Token::Record,
            Token::Ident("File".to_string()),
            Token::Lt,
            Token::Tilde,
            Token::Ident("f".to_string()),
            Token::Gt,
            Token::LBrace,
            Token::RBrace,
        ]);
    }
    
    #[test]
    fn test_temporal_constraints() {
        let tokens = lex("where ~tx within ~db").unwrap().1;
        assert_eq!(tokens, vec![
            Token::Where,
            Token::Tilde,
            Token::Ident("tx".to_string()),
            Token::Within,
            Token::Tilde,
            Token::Ident("db".to_string()),
        ]);
    }
    
    #[test]
    fn test_comments() {
        // Test single line comment
        let input = "val x = 42 // this is a comment\nval y = 10";
        let result = lex(input).unwrap().1;
        assert_eq!(result, vec![
            Token::Val,
            Token::Ident("x".to_string()),
            Token::Assign,
            Token::IntLit(42),
            Token::Val,
            Token::Ident("y".to_string()),
            Token::Assign,
            Token::IntLit(10),
        ]);
        
        // Test multi-line comment
        let input = "val x = /* this is a\nmulti-line comment */ 42";
        let result = lex(input).unwrap().1;
        assert_eq!(result, vec![
            Token::Val,
            Token::Ident("x".to_string()),
            Token::Assign,
            Token::IntLit(42),
        ]);
        
        // Test multiple comments
        let input = "// start comment\nval x = 42 /* inline */ + 10 // end comment";
        let result = lex(input).unwrap().1;
        assert_eq!(result, vec![
            Token::Val,
            Token::Ident("x".to_string()),
            Token::Assign,
            Token::IntLit(42),
            Token::Plus,
            Token::IntLit(10),
        ]);
    }

    // ========== Newline-aware lexer tests ==========

    #[test]
    fn test_newline_aware_basic() {
        // Two statements on separate lines should have a newline token between them
        let input = "val x = 42\nval y = 10";
        let tokens = lex_newline_aware(input).unwrap();
        assert_eq!(tokens, vec![
            Token::Val,
            Token::Ident("x".to_string()),
            Token::Assign,
            Token::IntLit(42),
            Token::Newline,
            Token::Val,
            Token::Ident("y".to_string()),
            Token::Assign,
            Token::IntLit(10),
        ]);
    }

    #[test]
    fn test_newline_suppressed_after_operator() {
        // Newline after operator should be suppressed
        let input = "val x = 42 +\n10";
        let tokens = lex_newline_aware(input).unwrap();
        assert_eq!(tokens, vec![
            Token::Val,
            Token::Ident("x".to_string()),
            Token::Assign,
            Token::IntLit(42),
            Token::Plus,
            Token::IntLit(10),
        ]);
    }

    #[test]
    fn test_newline_suppressed_after_pipe() {
        // Newline after pipe operator should be suppressed
        let input = "42 |>\nprintln";
        let tokens = lex_newline_aware(input).unwrap();
        assert_eq!(tokens, vec![
            Token::IntLit(42),
            Token::Pipe,
            Token::Ident("println".to_string()),
        ]);
    }

    #[test]
    fn test_newline_suppressed_in_braces() {
        // Newlines inside braces should be suppressed
        let input = "{\n42\n}";
        let tokens = lex_newline_aware(input).unwrap();
        assert_eq!(tokens, vec![
            Token::LBrace,
            Token::IntLit(42),
            Token::RBrace,
        ]);
    }

    #[test]
    fn test_newline_suppressed_before_else() {
        // Newline before else should be suppressed
        let input = "x then { 1 }\nelse { 2 }";
        let tokens = lex_newline_aware(input).unwrap();
        assert_eq!(tokens, vec![
            Token::Ident("x".to_string()),
            Token::Then,
            Token::LBrace,
            Token::IntLit(1),
            Token::RBrace,
            Token::Else,
            Token::LBrace,
            Token::IntLit(2),
            Token::RBrace,
        ]);
    }

    #[test]
    fn test_newline_suppressed_after_comma() {
        // Newline after comma should be suppressed
        let input = "[\n1,\n2,\n3\n]";
        let tokens = lex_newline_aware(input).unwrap();
        assert_eq!(tokens, vec![
            Token::LBracket,
            Token::IntLit(1),
            Token::Comma,
            Token::IntLit(2),
            Token::Comma,
            Token::IntLit(3),
            Token::RBracket,
        ]);
    }

    #[test]
    fn test_newline_in_function_body() {
        // Statements inside function body should be separated by newlines
        let input = "fun main: () -> Int = {\n    val x = 1\n    val y = 2\n    x\n}";
        let tokens = lex_newline_aware(input).unwrap();
        // Newlines after `=` (after fun decl) and `{` are suppressed
        // Newlines before `}` are suppressed
        // But newline between statements should be preserved
        assert!(tokens.contains(&Token::Newline), "Expected newline tokens in function body");

        // Count newlines - should have 2 (after x = 1, after y = 2)
        let newline_count = tokens.iter().filter(|t| **t == Token::Newline).count();
        assert_eq!(newline_count, 2, "Expected 2 newlines between statements, got {}", newline_count);
    }

    #[test]
    fn test_newline_suppressed_after_assign() {
        // Newline after = should be suppressed (multiline values)
        let input = "val x =\n42";
        let tokens = lex_newline_aware(input).unwrap();
        assert_eq!(tokens, vec![
            Token::Val,
            Token::Ident("x".to_string()),
            Token::Assign,
            Token::IntLit(42),
        ]);
    }

    #[test]
    fn test_method_chaining_on_newline() {
        // Dot at start of line should suppress preceding newline (method chaining)
        let input = "obj\n.method";
        let tokens = lex_newline_aware(input).unwrap();
        assert_eq!(tokens, vec![
            Token::Ident("obj".to_string()),
            Token::Dot,
            Token::Ident("method".to_string()),
        ]);
    }

    #[test]
    fn test_pipe_at_start_of_line() {
        // Pipe at start of line should suppress preceding newline
        let input = "42\n|> println";
        let tokens = lex_newline_aware(input).unwrap();
        assert_eq!(tokens, vec![
            Token::IntLit(42),
            Token::Pipe,
            Token::Ident("println".to_string()),
        ]);
    }

    #[test]
    fn test_multiple_newlines_collapsed() {
        // Multiple newlines should be treated as one
        let input = "val x = 42\n\n\nval y = 10";
        let tokens = lex_newline_aware(input).unwrap();
        let newline_count = tokens.iter().filter(|t| **t == Token::Newline).count();
        assert_eq!(newline_count, 1, "Multiple newlines should collapse to one");
    }
}