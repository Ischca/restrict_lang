//! # Lexer Module
//!
//! The lexer tokenizes Restrict Language source code using the nom parser combinator library.
//! It handles the unique OSV syntax, including the pipe operator `|>`, and supports
//! affine type annotations and prototype-based features.
//!
//! ## Token Categories
//!
//! - **Keywords**: Language reserved words (`fun`, `val`, `mut`, `clone`, `freeze`, etc.)
//! - **Operators**: Including the distinctive pipe operator (`|>`)
//! - **Literals**: Numbers, strings, characters, booleans
//! - **Identifiers**: Variable and function names
//! - **Delimiters**: Parentheses, braces, brackets
//!
//! ## Example
//!
//! ```rust
//! assert!(!restrict_lang::lexer::lex_tokens(r#""Hello, World!" |> println"#)
//!     .unwrap()
//!     .is_empty());
//! ```

use crate::diagnostics::format_lex_error;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_until, take_while, take_while1},
    character::complete::char,
    combinator::{recognize, value},
    multi::many0,
    sequence::{delimited, pair, preceded},
    IResult,
};
use std::fmt;

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
    /// `enum` keyword reserved for future enum declarations
    Enum,
    /// `form` keyword reserved for future source-level form declarations
    Form,
    /// `takes` keyword reserved for future source-level form adoptions
    Takes,
    /// `with` keyword for resource management
    With,
    /// `fun` keyword for function declarations
    Fun,
    /// `val` keyword for bindings
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
    /// `()` literal token
    Unit,
    /// `Some` variant constructor
    Some,
    /// `None` variant constructor
    None,
    /// `import` keyword
    Import,
    /// `export` keyword
    Export,
    /// `pub` keyword for public exports
    Pub,
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
    /// `as` keyword for explicit casts
    As,

    // Identifiers and Literals
    /// Identifier (variable/function name)
    Ident(String),
    /// Integer literal
    IntLit(i64),
    /// Floating-point literal
    FloatLit(f64),
    /// String literal
    StringLit(String),
    /// Character literal
    CharLit(char),

    // Operators
    /// Pipe operator `|>` for OSV syntax
    Pipe,
    /// Vertical bar `|` for patterns
    Bar,
    Assign,    // =
    Arrow,     // =>
    ThinArrow, // ->
    Plus,      // +
    Minus,     // -
    Star,      // *
    Asterisk,  // * (for import *)
    Slash,     // /
    Percent,   // %
    Eq,        // ==
    Ne,        // !=
    Not,       // !
    Lt,        // <
    Le,        // <=
    Gt,        // >
    Ge,        // >=
    And,       // &&
    Or,        // ||

    // Temporal
    Tilde, // ~ (for temporal type variables)

    // Delimiters
    LBrace,    // {
    RBrace,    // }
    LParen,    // (
    RParen,    // )
    LBracket,  // [
    RBracket,  // ]
    Comma,     // ,
    Colon,     // :
    Dot,       // .
    DotDot,    // .. (range literal separator)
    DotDotDot, // ... (for spread destructuring)
    Semicolon, // ;

    // Special
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
            Token::Enum => write!(f, "enum"),
            Token::Form => write!(f, "form"),
            Token::Takes => write!(f, "takes"),
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
            Token::Unit => write!(f, "()"),
            Token::Some => write!(f, "Some"),
            Token::None => write!(f, "None"),
            Token::Import => write!(f, "import"),
            Token::Export => write!(f, "export"),
            Token::Pub => write!(f, "pub"),
            Token::Sealed => write!(f, "sealed"),
            Token::From => write!(f, "from"),
            Token::Within => write!(f, "within"),
            Token::Where => write!(f, "where"),
            Token::Lifetime => write!(f, "lifetime"),
            Token::Await => write!(f, "await"),
            Token::Spawn => write!(f, "spawn"),
            Token::As => write!(f, "as"),
            Token::Ident(s) => write!(f, "{}", s),
            Token::IntLit(n) => write!(f, "{}", n),
            Token::FloatLit(n) => write!(f, "{}", n),
            Token::StringLit(s) => write!(f, "\"{}\"", s),
            Token::CharLit(c) => write!(f, "'{}'", c),
            Token::Pipe => write!(f, "|>"),
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
            Token::Not => write!(f, "!"),
            Token::Lt => write!(f, "<"),
            Token::Le => write!(f, "<="),
            Token::Gt => write!(f, ">"),
            Token::Ge => write!(f, ">="),
            Token::And => write!(f, "&&"),
            Token::Or => write!(f, "||"),
            Token::Tilde => write!(f, "~"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::Comma => write!(f, ","),
            Token::Colon => write!(f, ":"),
            Token::Dot => write!(f, "."),
            Token::DotDot => write!(f, ".."),
            Token::DotDotDot => write!(f, "..."),
            Token::Semicolon => write!(f, ";"),
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
    recognize(pair(
        take_while1(|c: char| is_ident_start(c)),
        take_while(|c: char| is_ident_continue(c)),
    ))(input)
}

fn keyword(input: &str) -> IResult<&str, Token> {
    let ident = identifier(input)?;
    let token = match ident.1 {
        "record" => Token::Record,
        "clone" => Token::Clone,
        "freeze" => Token::Freeze,
        "impl" => Token::Impl,
        "context" => Token::Context,
        "enum" => Token::Enum,
        "form" => Token::Form,
        "takes" => Token::Takes,
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
        "import" => Token::Import,
        "export" => Token::Export,
        "pub" => Token::Pub,
        "sealed" => Token::Sealed,
        "from" => Token::From,
        "within" => Token::Within,
        "where" => Token::Where,
        "lifetime" => Token::Lifetime,
        "await" => Token::Await,
        "spawn" => Token::Spawn,
        "as" => Token::As,
        _ => return Ok((ident.0, Token::Ident(ident.1.to_string()))),
    };
    Ok((ident.0, token))
}

fn lexer_error(input: &str, kind: nom::error::ErrorKind) -> nom::Err<nom::error::Error<&str>> {
    nom::Err::Error(nom::error::Error::new(input, kind))
}

fn integer(input: &str) -> IResult<&str, Token> {
    if let Some(rest) = input
        .strip_prefix("0x")
        .or_else(|| input.strip_prefix("0X"))
    {
        let len = rest
            .char_indices()
            .take_while(|(_, ch)| ch.is_ascii_hexdigit() || *ch == '_')
            .last()
            .map(|(idx, ch)| idx + ch.len_utf8())
            .unwrap_or(0);
        if len == 0 {
            return Err(lexer_error(input, nom::error::ErrorKind::HexDigit));
        }

        let literal = &rest[..len];
        let digits = literal.replace('_', "");
        let value = i64::from_str_radix(&digits, 16)
            .map_err(|_| lexer_error(input, nom::error::ErrorKind::MapRes))?;
        return Ok((&rest[len..], Token::IntLit(value)));
    }

    let len = input
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_digit() || *ch == '_')
        .last()
        .map(|(idx, ch)| idx + ch.len_utf8())
        .unwrap_or(0);
    if len == 0 {
        return Err(lexer_error(input, nom::error::ErrorKind::Digit));
    }

    let literal = &input[..len];
    let digits = literal.replace('_', "");
    let value = digits
        .parse::<i64>()
        .map_err(|_| lexer_error(input, nom::error::ErrorKind::MapRes))?;
    Ok((&input[len..], Token::IntLit(value)))
}

fn scan_decimal_digits(input: &str, mut idx: usize) -> usize {
    while let Some(ch) = input[idx..].chars().next() {
        if ch.is_ascii_digit() || ch == '_' {
            idx += ch.len_utf8();
        } else {
            break;
        }
    }
    idx
}

fn float(input: &str) -> IResult<&str, Token> {
    let first = input
        .chars()
        .next()
        .ok_or_else(|| lexer_error(input, nom::error::ErrorKind::Digit))?;
    if !first.is_ascii_digit() {
        return Err(lexer_error(input, nom::error::ErrorKind::Digit));
    }

    let mut idx = scan_decimal_digits(input, 0);
    let mut has_dot = false;
    let mut has_exp = false;

    if input[idx..].starts_with('.') {
        let frac_start = idx + 1;
        let frac_end = scan_decimal_digits(input, frac_start);
        if frac_end == frac_start {
            return Err(lexer_error(input, nom::error::ErrorKind::Float));
        }
        has_dot = true;
        idx = frac_end;
    }

    if matches!(input[idx..].chars().next(), Some('e' | 'E')) {
        let exp_marker = idx;
        idx += 1;
        if matches!(input[idx..].chars().next(), Some('+' | '-')) {
            idx += 1;
        }
        let exp_start = idx;
        idx = scan_decimal_digits(input, idx);
        if idx == exp_start {
            return Err(lexer_error(
                &input[exp_marker..],
                nom::error::ErrorKind::Float,
            ));
        }
        has_exp = true;
    }

    if !has_dot && !has_exp {
        return Err(lexer_error(input, nom::error::ErrorKind::Float));
    }

    let literal = input[..idx].replace('_', "");
    let value = literal
        .parse::<f64>()
        .map_err(|_| lexer_error(input, nom::error::ErrorKind::MapRes))?;
    Ok((&input[idx..], Token::FloatLit(value)))
}

fn string_lit(input: &str) -> IResult<&str, Token> {
    let Some(rest) = input.strip_prefix('"') else {
        return Err(lexer_error(input, nom::error::ErrorKind::Char));
    };

    let mut value = String::new();
    let mut escaped = false;
    for (idx, ch) in rest.char_indices() {
        if escaped {
            value.push(
                unescape_char(ch)
                    .ok_or_else(|| lexer_error(&rest[idx..], nom::error::ErrorKind::Escaped))?,
            );
            escaped = false;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '"' => return Ok((&rest[idx + ch.len_utf8()..], Token::StringLit(value))),
            _ => value.push(ch),
        }
    }

    Err(lexer_error(input, nom::error::ErrorKind::TakeUntil))
}

fn char_lit(input: &str) -> IResult<&str, Token> {
    let Some(rest) = input.strip_prefix('\'') else {
        return Err(lexer_error(input, nom::error::ErrorKind::Char));
    };

    let mut chars = rest.char_indices();
    let Some((_, first)) = chars.next() else {
        return Err(lexer_error(input, nom::error::ErrorKind::Char));
    };

    let (value, consumed) = if first == '\\' {
        let Some((escape_idx, escape)) = chars.next() else {
            return Err(lexer_error(input, nom::error::ErrorKind::Escaped));
        };
        let value = unescape_char(escape)
            .ok_or_else(|| lexer_error(&rest[escape_idx..], nom::error::ErrorKind::Escaped))?;
        (value, escape_idx + escape.len_utf8())
    } else {
        (first, first.len_utf8())
    };

    let after_value = &rest[consumed..];
    let Some(after_quote) = after_value.strip_prefix('\'') else {
        return Err(lexer_error(after_value, nom::error::ErrorKind::Char));
    };

    Ok((after_quote, Token::CharLit(value)))
}

fn unescape_char(ch: char) -> Option<char> {
    match ch {
        'n' => Some('\n'),
        't' => Some('\t'),
        'r' => Some('\r'),
        '\\' => Some('\\'),
        '"' => Some('"'),
        '\'' => Some('\''),
        _ => None,
    }
}

fn operator(input: &str) -> IResult<&str, Token> {
    if input.starts_with("|>>") {
        return Err(nom::Err::Failure(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )));
    }

    alt((
        alt((
            value(Token::Pipe, tag("|>")),
            value(Token::Or, tag("||")),
            value(Token::DotDotDot, tag("...")), // Check ... before .
            value(Token::DotDot, tag("..")),
            value(Token::ThinArrow, tag("->")),
            value(Token::Arrow, tag("=>")),
            value(Token::Eq, tag("==")),
            value(Token::Ne, tag("!=")),
            value(Token::And, tag("&&")),
            value(Token::Le, tag("<=")),
            value(Token::Ge, tag(">=")),
        )),
        alt((
            value(Token::Bar, tag("|")),
            value(Token::Assign, tag("=")),
            value(Token::Plus, tag("+")),
            value(Token::Minus, tag("-")),
            value(Token::Star, tag("*")),
            value(Token::Slash, tag("/")),
            value(Token::Percent, tag("%")),
            value(Token::Not, tag("!")),
            value(Token::Lt, tag("<")),
            value(Token::Gt, tag(">")),
            value(Token::Tilde, tag("~")),
        )),
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
        float, integer, keyword, string_lit, char_lit, operator, delimiter,
    ))(input)
}

fn whitespace(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_whitespace())(input)
}

fn comment(input: &str) -> IResult<&str, &str> {
    alt((
        // Single line comment
        recognize(pair(tag("//"), take_while(|c| c != '\n'))),
        // Multi-line comment - simplified version (no nested comments)
        recognize(delimited(tag("/*"), take_until("*/"), tag("*/"))),
    ))(input)
}

pub fn skip(input: &str) -> IResult<&str, ()> {
    let mut input = input;
    while let Ok((rest, _)) = alt((whitespace, comment))(input) {
        input = rest;
    }
    Ok((input, ()))
}

pub fn lex_token(input: &str) -> IResult<&str, Token> {
    preceded(skip, token)(input)
}

pub fn lex(input: &str) -> IResult<&str, Vec<Token>> {
    let (input, _) = skip(input)?; // Skip initial whitespace
    let (input, tokens) = many0(lex_token)(input)?;
    let (input, _) = skip(input)?; // Skip trailing whitespace
    Ok((input, tokens))
}

// Wrapper function that tokenizes the entire input or returns an error
pub fn lex_tokens(input: &str) -> Result<Vec<Token>, String> {
    match lex(input) {
        Ok((remaining, tokens)) => {
            if !remaining.is_empty() {
                let error = nom::Err::Error(nom::error::Error::new(
                    remaining,
                    nom::error::ErrorKind::Tag,
                ));
                Err(format_lex_error(input, error))
            } else {
                Ok(tokens)
            }
        }
        Err(e) => Err(format_lex_error(input, e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keywords() {
        assert_eq!(lex("record").unwrap().1, vec![Token::Record]);
        assert_eq!(lex("enum").unwrap().1, vec![Token::Enum]);
        assert_eq!(lex("form").unwrap().1, vec![Token::Form]);
        assert_eq!(lex("takes").unwrap().1, vec![Token::Takes]);
        assert_eq!(lex("fun").unwrap().1, vec![Token::Fun]);
        assert_eq!(lex("val").unwrap().1, vec![Token::Val]);
        assert_eq!(lex("pub").unwrap().1, vec![Token::Pub]);
        assert_eq!(lex("as").unwrap().1, vec![Token::As]);
    }

    #[test]
    fn test_identifiers() {
        assert_eq!(
            lex("hello").unwrap().1,
            vec![Token::Ident("hello".to_string())]
        );
        assert_eq!(
            lex("_test123").unwrap().1,
            vec![Token::Ident("_test123".to_string())]
        );
    }

    #[test]
    fn test_operators() {
        assert_eq!(lex("|>").unwrap().1, vec![Token::Pipe]);
        assert!(lex("|>>").is_err());
        assert_eq!(lex("&&").unwrap().1, vec![Token::And]);
        assert_eq!(lex("||").unwrap().1, vec![Token::Or]);
        assert_eq!(lex("!").unwrap().1, vec![Token::Not]);
        assert_eq!(lex("=>").unwrap().1, vec![Token::Arrow]);
    }

    #[test]
    fn test_complex_expression() {
        let tokens = lex("val x = 42 |> add 10").unwrap().1;
        assert_eq!(
            tokens,
            vec![
                Token::Val,
                Token::Ident("x".to_string()),
                Token::Assign,
                Token::IntLit(42),
                Token::Pipe,
                Token::Ident("add".to_string()),
                Token::IntLit(10),
            ]
        );
    }

    #[test]
    fn test_spec_number_literals() {
        let tokens = lex("0xFF 1_000_000 1.5e10 3.14E-2").unwrap().1;
        assert_eq!(
            tokens,
            vec![
                Token::IntLit(255),
                Token::IntLit(1_000_000),
                Token::FloatLit(1.5e10),
                Token::FloatLit(3.14E-2),
            ]
        );
    }

    #[test]
    fn test_spec_string_and_char_escapes() {
        let tokens = lex(r#""a\nb\t\\\"\'" '\n' '\t' '\\' '\''"#).unwrap().1;
        assert_eq!(
            tokens,
            vec![
                Token::StringLit("a\nb\t\\\"'".to_string()),
                Token::CharLit('\n'),
                Token::CharLit('\t'),
                Token::CharLit('\\'),
                Token::CharLit('\''),
            ]
        );
    }

    #[test]
    fn test_temporal_tilde() {
        let tokens = lex("record File<~f> { }").unwrap().1;
        assert_eq!(
            tokens,
            vec![
                Token::Record,
                Token::Ident("File".to_string()),
                Token::Lt,
                Token::Tilde,
                Token::Ident("f".to_string()),
                Token::Gt,
                Token::LBrace,
                Token::RBrace,
            ]
        );
    }

    #[test]
    fn test_temporal_constraints() {
        let tokens = lex("where ~tx within ~db").unwrap().1;
        assert_eq!(
            tokens,
            vec![
                Token::Where,
                Token::Tilde,
                Token::Ident("tx".to_string()),
                Token::Within,
                Token::Tilde,
                Token::Ident("db".to_string()),
            ]
        );
    }

    #[test]
    fn test_comments() {
        // Test single line comment
        let input = "val x = 42 // this is a comment\nval y = 10";
        let result = lex(input).unwrap().1;
        assert_eq!(
            result,
            vec![
                Token::Val,
                Token::Ident("x".to_string()),
                Token::Assign,
                Token::IntLit(42),
                Token::Val,
                Token::Ident("y".to_string()),
                Token::Assign,
                Token::IntLit(10),
            ]
        );

        // Test multi-line comment
        let input = "val x = /* this is a\nmulti-line comment */ 42";
        let result = lex(input).unwrap().1;
        assert_eq!(
            result,
            vec![
                Token::Val,
                Token::Ident("x".to_string()),
                Token::Assign,
                Token::IntLit(42),
            ]
        );

        // Test multiple comments
        let input = "// start comment\nval x = 42 /* inline */ + 10 // end comment";
        let result = lex(input).unwrap().1;
        assert_eq!(
            result,
            vec![
                Token::Val,
                Token::Ident("x".to_string()),
                Token::Assign,
                Token::IntLit(42),
                Token::Plus,
                Token::IntLit(10),
            ]
        );
    }

    #[test]
    fn lex_tokens_formats_leftover_input_as_user_diagnostic() {
        let message = lex_tokens("val x = 1\nval y = @").expect_err("unknown token should fail");

        assert!(message.contains("Lexing error at line 2, column 9"));
        assert!(message.contains("unexpected input near `@`"));
        assert_no_raw_nom_debug(&message);
    }

    #[test]
    fn lex_tokens_formats_nom_errors_as_user_diagnostics() {
        let message = lex_tokens("val x = |>>").expect_err("invalid pipe token should fail");

        assert!(message.contains("Lexing error at line 1, column 9"));
        assert!(message.contains("unexpected input near `|>>`"));
        assert_no_raw_nom_debug(&message);
    }

    fn assert_no_raw_nom_debug(message: &str) {
        assert!(
            !message.contains("Error("),
            "diagnostic should not expose raw nom error debug output: {message}"
        );
        assert!(
            !message.contains("Failure("),
            "diagnostic should not expose raw nom failure debug output: {message}"
        );
        assert!(
            !message.contains("ErrorKind"),
            "diagnostic should not expose nom error kinds: {message}"
        );
        assert!(
            !message.contains("nom"),
            "diagnostic should not name parser internals: {message}"
        );
    }
}
