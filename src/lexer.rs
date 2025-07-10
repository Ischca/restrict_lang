use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, take_while1, take_while},
    character::complete::{char, digit1, one_of, multispace0},
    combinator::{recognize, map, value},
    multi::many0,
    sequence::{pair, preceded, delimited},
};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Record,
    Clone,
    Freeze,
    Impl,
    Context,
    With,
    Fun,
    Val,
    Mut,
    Then,
    Else,
    While,
    Match,
    Async,
    Return,
    True,
    False,
    Unit,
    
    // Identifiers and Literals
    Ident(String),
    IntLit(i32),
    FloatLit(f64),
    StringLit(String),
    CharLit(char),
    
    // Operators
    Pipe,           // |>
    PipeMut,        // |>>
    Bar,            // |
    Assign,         // =
    Arrow,          // =>
    Plus,           // +
    Minus,          // -
    Star,           // *
    Slash,          // /
    Percent,        // %
    Eq,             // ==
    Ne,             // !=
    Lt,             // <
    Le,             // <=
    Gt,             // >
    Ge,             // >=
    
    // Delimiters
    LBrace,         // {
    RBrace,         // }
    LParen,         // (
    RParen,         // )
    Comma,          // ,
    Colon,          // :
    Dot,            // .
    
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
            Token::Slash => write!(f, "/"),
            Token::Percent => write!(f, "%"),
            Token::Eq => write!(f, "=="),
            Token::Ne => write!(f, "!="),
            Token::Lt => write!(f, "<"),
            Token::Le => write!(f, "<="),
            Token::Gt => write!(f, ">"),
            Token::Ge => write!(f, ">="),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::Comma => write!(f, ","),
            Token::Colon => write!(f, ":"),
            Token::Dot => write!(f, "."),
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
        value(Token::Bar, tag("|")),
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
    ))(input)
}

fn delimiter(input: &str) -> IResult<&str, Token> {
    alt((
        value(Token::LBrace, char('{')),
        value(Token::RBrace, char('}')),
        value(Token::LParen, char('(')),
        value(Token::RParen, char(')')),
        value(Token::Comma, char(',')),
        value(Token::Colon, char(':')),
        value(Token::Dot, char('.')),
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
    multispace0(input)
}

// TODO: implement comment parsing
// fn comment(input: &str) -> IResult<&str, &str> {
//     alt((
//         // Single line comment
//         recognize(pair(tag("//"), take_while(|c| c != '\n'))),
//         // Multi-line comment
//         ...
//     ))(input)
// }

fn skip(input: &str) -> IResult<&str, ()> {
    map(
        whitespace,  // Just use whitespace for now
        |_| ()
    )(input)
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
}