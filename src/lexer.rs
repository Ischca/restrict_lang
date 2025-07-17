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

pub fn lex_token(input: &str) -> IResult<&str, Token> {
    preceded(skip, token)(input)
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
}