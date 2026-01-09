//! # Parser Module
//!
//! The parser transforms a stream of tokens into an Abstract Syntax Tree (AST).
//! It implements Restrict Language's unique OSV (Object-Subject-Verb) syntax
//! and handles affine type constraints during parsing.
//!
//! ## Key Features
//!
//! - **OSV Syntax**: Natural handling of pipe operators (`|>`, `|>>`)
//! - **Pattern Matching**: Comprehensive pattern support including list patterns
//! - **Generic Functions**: Type parameters with bounds and derivation constraints
//! - **Prototype System**: Parsing of `clone` and `freeze` operations
//!
//! ## Example
//!
//! ```rust
//! use restrict_lang::parser::parse_program;
//!
//! let input = r#"
//!     fn main() {
//!         "Hello, World!" |> println;
//!     }
//! "#;
//! 
//! let ast = parse_program(input).unwrap();
//! ```

use nom::{
    IResult,
    branch::alt,
    combinator::{map, opt, value},
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{preceded, tuple, delimited},
};
use crate::lexer::{Token, lex_token, skip, Span};
use crate::ast::*;

/// Context for tracking source positions during parsing.
pub struct ParseContext<'a> {
    /// The original source string
    pub source: &'a str,
}

impl<'a> ParseContext<'a> {
    /// Creates a new parse context.
    pub fn new(source: &'a str) -> Self {
        Self { source }
    }

    /// Calculates the byte offset from remaining input.
    pub fn offset(&self, remaining: &str) -> usize {
        self.source.len() - remaining.len()
    }

    /// Creates a span from start offset to current position.
    pub fn span_from(&self, start: usize, remaining: &str) -> Span {
        Span::new(start, self.offset(remaining))
    }
}

/// Helper to calculate byte offset from remaining input.
fn calc_offset(original: &str, remaining: &str) -> usize {
    original.len() - remaining.len()
}

/// Helper to create a span from original source and remaining input.
fn make_span(original: &str, start_remaining: &str, end_remaining: &str) -> Span {
    let start = calc_offset(original, start_remaining);
    let end = calc_offset(original, end_remaining);
    Span::new(start, end)
}

/// Type alias for parser results.
type ParseResult<'a, T> = IResult<&'a str, T>;

/// Expects a specific token and consumes it.
/// 
/// Returns an error if the next token doesn't match.
fn expect_token<'a>(expected: Token) -> impl Fn(&'a str) -> ParseResult<'a, ()> {
    move |input| {
        let original_input = input;
        let (input, token) = lex_token(input)?;
        if token == expected {
            let (input, _) = skip(input)?; // Skip trailing whitespace after token
            Ok((input, ()))
        } else {
            // Return error with the original input to allow backtracking
            Err(nom::Err::Error(nom::error::Error::new(original_input, nom::error::ErrorKind::Tag)))
        }
    }
}

/// Parses an identifier.
/// 
/// # Example
/// 
/// ```
/// // Parses: myVariable, userName, _private
/// ```
fn ident(input: &str) -> ParseResult<String> {
    let original_input = input;
    let (input, token) = lex_token(input)?;
    match token {
        Token::Ident(name) => Ok((input, name)),
        Token::It => Ok((input, "it".to_string())),  // Allow 'it' as an identifier in binding contexts
        _ => Err(nom::Err::Error(nom::error::Error::new(original_input, nom::error::ErrorKind::Tag)))
    }
}

/// Helper to parse a type name (identifier or Unit keyword)
fn type_name(input: &str) -> ParseResult<String> {
    let (input, token) = lex_token(input)?;
    match token {
        Token::Unit => Ok((input, "Unit".to_string())),
        Token::Ident(name) => Ok((input, name)),
        _ => Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
    }
}

/// Parses a type expression.
///
/// Handles both simple types, generic types, and temporal types.
///
/// # Examples
///
/// ```
/// // Simple types: i32, String, Point
/// // Generic types: Vec<i32>, Map<String, Value>
/// // Temporal types: File<~f>, Transaction<~tx, ~db>
/// ```
fn parse_type(input: &str) -> ParseResult<Type> {
    // Try to parse function type first: |Type1, Type2| -> ReturnType
    if let Ok((input_after_bar, _)) = expect_token(Token::Bar)(input) {
        // Parse parameter types
        let (input, param_types) = delimited(
            expect_token(Token::Bar),
            separated_list0(expect_token(Token::Comma), parse_type),
            expect_token(Token::Bar)
        )(input)?;

        // Parse arrow and return type
        let (input, _) = expect_token(Token::ThinArrow)(input)?;
        let (input, return_type) = parse_type(input)?;

        return Ok((input, Type::Function(param_types, Box::new(return_type))));
    }

    // Parse type name - handle both identifiers and the Unit keyword
    let (input, name) = type_name(input)?;
    let (input, type_params) = opt(
        delimited(
            expect_token(Token::Lt),
            separated_list0(
                expect_token(Token::Comma),
                alt((
                    // Parse temporal parameter (~f)
                    map(
                        preceded(expect_token(Token::Tilde), ident),
                        |name| (name, true)  // (name, is_temporal)
                    ),
                    // Parse regular type parameter
                    map(parse_type, |ty| {
                        match ty {
                            Type::Named(n) => (n, false),
                            _ => panic!("Complex types not supported as parameters yet")
                        }
                    })
                ))
            ),
            expect_token(Token::Gt)
        )
    )(input)?;

    match type_params {
        Some(params) => {
            // Check if all are temporal
            let all_temporal = params.iter().all(|(_, is_temporal)| *is_temporal);
            let all_regular = params.iter().all(|(_, is_temporal)| !*is_temporal);

            if all_temporal {
                // All temporal: File<~f>
                Ok((input, Type::Temporal(name, params.into_iter().map(|(n, _)| n).collect())))
            } else if all_regular {
                // All regular types: Vec<String>
                let types = params.into_iter().map(|(n, _)| Type::Named(n)).collect();
                Ok((input, Type::Generic(name, types)))
            } else {
                // Mixed not supported yet
                Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
            }
        }
        None => Ok((input, Type::Named(name)))
    }
}

fn field_decl(input: &str) -> ParseResult<FieldDecl> {
    let (input, name) = ident(input)?;
    let (input, _) = expect_token(Token::Colon)(input)?;
    let (input, ty) = parse_type(input)?;
    Ok((input, FieldDecl { name, ty }))
}

fn record_decl(input: &str) -> ParseResult<RecordDecl> {
    let (input, _) = expect_token(Token::Record)(input)?;
    let (input, name) = ident(input)?;
    
    // Parse optional type parameters: <T, ~f>
    let (input, type_params) = opt(|input| {
        let (input, _) = expect_token(Token::Lt)(input)?;
        let (input, params) = separated_list1(
            expect_token(Token::Comma),
            type_param
        )(input)?;
        let (input, _) = expect_token(Token::Gt)(input)?;
        Ok((input, params))
    })(input)?;
    let type_params = type_params.unwrap_or_default();
    
    // Parse optional temporal constraints: where ~tx within ~db
    let (input, temporal_constraints) = opt(|input| {
        let (input, _) = expect_token(Token::Where)(input)?;
        separated_list1(
            expect_token(Token::Comma),
            temporal_constraint
        )(input)
    })(input)?;
    let temporal_constraints = temporal_constraints.unwrap_or_default();
    
    let (input, _) = expect_token(Token::LBrace)(input)?;
    // Parse fields - they should be space-separated, not comma-separated
    let (input, fields) = many0(field_decl)(input)?;
    let (input, _) = expect_token(Token::RBrace)(input)?;
    
    // For now, skip freeze/sealed checks to debug parsing issue
    let frozen = false;
    let sealed = false;
    
    Ok((input, RecordDecl {
        name,
        type_params,
        temporal_constraints,
        fields,
        frozen,
        sealed,
        parent_hash: None,
        span: None,
    }))
}

// Parse a temporal constraint: ~tx within ~db
fn temporal_constraint(input: &str) -> ParseResult<TemporalConstraint> {
    let (input, _) = expect_token(Token::Tilde)(input)?;
    let (input, inner) = ident(input)?;
    let (input, _) = expect_token(Token::Within)(input)?;
    let (input, _) = expect_token(Token::Tilde)(input)?;
    let (input, outer) = ident(input)?;
    Ok((input, TemporalConstraint { inner, outer }))
}

fn param(input: &str) -> ParseResult<Param> {
    // For now, skip context bounds since we don't have @ token
    let context_bound = None;
    
    let (input, name) = ident(input)?;
    let (input, _) = expect_token(Token::Colon)(input)?;
    let (input, ty) = parse_type(input)?;
    Ok((input, Param { name, ty, context_bound }))
}

/// Parse a block expression with context-dependent evaluation mode.
///
/// # Parameters
/// - `input`: The input string to parse
/// - `is_lazy`: Whether this block should be lazy (true) or eager (false)
///   - Lazy: block is a closure, can be stored and called later
///   - Eager: block executes immediately
fn block_expr_with_mode(input: &str, is_lazy: bool) -> ParseResult<BlockExpr> {
    let (input, _) = expect_token(Token::LBrace)(input)?;

    // Parse statements and expressions carefully
    let mut statements = Vec::new();
    let mut remaining = input;
    let mut final_expr = None;
    
    loop {
        // Skip whitespace and comments before each statement/expression
        let (rest, _) = skip(remaining)?;
        remaining = rest;
        
        // Check if we've reached the closing brace
        if let Ok((after_brace, _)) = expect_token::<'_>(Token::RBrace)(remaining) {
            remaining = after_brace;
            break;
        }
        
        // Try to parse a binding first
        if let Ok((after_bind, bind_decl)) = bind_decl(remaining) {
            statements.push(Stmt::Binding(bind_decl));
            // Consume optional semicolon
            let (after_semi, _) = opt(expect_token(Token::Semicolon))(after_bind)?;
            remaining = after_semi;
            continue;
        }
        
        // Try to parse an assignment
        if let Ok((after_assign, assign_stmt)) = assignment_stmt(remaining) {
            statements.push(assign_stmt);
            // Consume optional semicolon
            let (after_semi, _) = opt(expect_token(Token::Semicolon))(after_assign)?;
            remaining = after_semi;
            continue;
        }
        
        // Otherwise, parse an expression with statement context
        let (after_expr, expr) = expression_in_statement(remaining)?;
        
        // Peek ahead to see if this is the final expression
        if let Ok((_, _)) = expect_token::<'_>(Token::RBrace)(after_expr) {
            // This is the final expression
            final_expr = Some(Box::new(expr));
            remaining = after_expr;
        } else {
            // This is a statement expression
            statements.push(Stmt::Expr(Box::new(expr)));
            // Consume optional semicolon
            let (after_semi, _) = opt(expect_token(Token::Semicolon))(after_expr)?;
            remaining = after_semi;
        }
    }
    
    // Detect if the block uses 'it' anywhere
    let has_implicit_it = block_uses_it(&statements, &final_expr);

    Ok((remaining, BlockExpr {
        statements,
        expr: final_expr,
        is_lazy,
        has_implicit_it,
        span: None,
    }))
}

/// Parse an eager block (default for function bodies, control flow, etc.)
fn block_expr(input: &str) -> ParseResult<BlockExpr> {
    block_expr_with_mode(input, false)  // eager by default
}

/// Parse a lazy block (for expression contexts)
fn lazy_block_expr(input: &str) -> ParseResult<BlockExpr> {
    block_expr_with_mode(input, true)  // lazy for closures
}

fn fun_decl(input: &str) -> ParseResult<FunDecl> {
    // Skip leading whitespace
    let (input, _) = skip(input)?;
    
    // Check for optional async keyword
    let (input, is_async) = opt(expect_token(Token::Async))(input)?;
    let is_async = is_async.is_some();
    
    let (input, _) = expect_token(Token::Fun)(input)?;
    let (input, name) = ident(input)?;
    let (input, _) = expect_token(Token::Colon)(input)?;
    
    // Parse optional generic type parameters: <T: Display, U: Clone + Debug, ~t>
    let (input, type_params) = opt(|input| {
        let (input, _) = expect_token(Token::Lt)(input)?;
        let (input, params) = separated_list1(
            expect_token(Token::Comma),
            type_param
        )(input)?;
        let (input, _) = expect_token(Token::Gt)(input)?;
        Ok((input, params))
    })(input)?;
    let type_params = type_params.unwrap_or_default();
    
    // Parse parameter list: (x: Int32, y: Int32)
    let (input, _) = expect_token(Token::LParen)(input)?;
    let (input, params) = separated_list0(
        expect_token(Token::Comma),
        param
    )(input)?;
    let (input, _) = expect_token(Token::RParen)(input)?;
    
    // Parse optional return type: -> ReturnType
    let (input, _return_type) = opt(|input| {
        let (input, _) = expect_token(Token::ThinArrow)(input)?;
        parse_type(input)
    })(input)?;
    
    // Parse optional temporal constraints: where ~tx within ~db
    let (input, temporal_constraints) = opt(|input| {
        let (input, _) = expect_token(Token::Where)(input)?;
        separated_list1(
            expect_token(Token::Comma),
            temporal_constraint
        )(input)
    })(input)?;
    let temporal_constraints = temporal_constraints.unwrap_or_default();
    
    let (input, _) = expect_token(Token::Assign)(input)?;
    let (input, body) = block_expr(input)?;
    
    Ok((input, FunDecl {
        name,
        is_async,
        type_params,
        temporal_constraints,
        params,
        body,
        span: None,
    }))
}

// Parse a type parameter with optional bounds: T: Display + Clone and derivation bound: T from ParentType
// Also supports temporal type parameters: ~t
fn type_param(input: &str) -> ParseResult<TypeParam> {
    // Check if this is a temporal type parameter
    let (input, is_temporal) = opt(expect_token(Token::Tilde))(input)?;
    let is_temporal = is_temporal.is_some();
    
    let (input, name) = ident(input)?;
    
    // Temporal parameters don't have trait bounds or derivation bounds
    if is_temporal {
        return Ok((input, TypeParam { 
            name, 
            bounds: vec![], 
            derivation_bound: None, 
            is_temporal: true 
        }));
    }
    
    // Parse optional derivation bound: from ParentType
    let (input, derivation_bound) = opt(|input| {
        let (input, _) = expect_token(Token::From)(input)?;
        let (input, parent_name) = ident(input)?;
        Ok((input, parent_name))
    })(input)?;
    
    // Parse optional trait bounds: : Display + Clone + Debug
    let (input, bounds) = opt(|input| {
        let (input, _) = expect_token(Token::Colon)(input)?;
        separated_list1(
            expect_token(Token::Plus), // + for trait bounds
            |input| {
                let (input, trait_name) = ident(input)?;
                Ok((input, TypeBound { trait_name }))
            }
        )(input)
    })(input)?;
    
    let bounds = bounds.unwrap_or_default();
    Ok((input, TypeParam { name, bounds, derivation_bound, is_temporal: false }))
}

#[allow(dead_code)]
fn impl_block(input: &str) -> ParseResult<ImplBlock> {
    let (input, _) = expect_token(Token::Impl)(input)?;
    let (input, target) = ident(input)?;
    let (input, _) = expect_token(Token::LBrace)(input)?;
    let (input, functions) = many0(fun_decl)(input)?;
    let (input, _) = expect_token(Token::RBrace)(input)?;
    Ok((input, ImplBlock { target, functions }))
}

fn context_decl(input: &str) -> ParseResult<ContextDecl> {
    let (input, _) = expect_token(Token::Context)(input)?;
    let (input, name) = ident(input)?;
    
    // Parse optional type parameters (including temporal): <~fs>
    let (input, _type_params) = opt(|input| {
        let (input, _) = expect_token(Token::Lt)(input)?;
        let (input, params) = separated_list1(
            expect_token(Token::Comma),
            type_param
        )(input)?;
        let (input, _) = expect_token(Token::Gt)(input)?;
        Ok((input, params))
    })(input)?;
    
    // TODO: Handle type parameters in context declaration
    // For now, we'll ignore them until we update the ContextDecl struct
    
    let (input, _) = expect_token(Token::LBrace)(input)?;
    let (input, fields) = many0(field_decl)(input)?;
    let (input, _) = expect_token(Token::RBrace)(input)?;
    Ok((input, ContextDecl { name, fields }))
}

pub fn bind_decl(input: &str) -> ParseResult<BindDecl> {
    let (input, mutable) = opt(expect_token(Token::Mut))(input)?;
    let (input, _) = expect_token(Token::Val)(input)?;
    let (input, name) = ident(input)?;
    let (input, _) = expect_token(Token::Assign)(input)?;
    let (input, value) = expression_in_statement(input)?;  // Use statement-aware parsing to avoid consuming across statement boundaries
    Ok((input, BindDecl {
        mutable: mutable.is_some(),
        name,
        value: Box::new(value),
        span: None,
    }))
}

fn literal(input: &str) -> ParseResult<Expr> {
    let (input, token) = lex_token(input)?;
    match token {
        Token::IntLit(n) => Ok((input, Expr::IntLit(n))),
        Token::FloatLit(f) => Ok((input, Expr::FloatLit(f))),
        Token::StringLit(s) => Ok((input, Expr::StringLit(s))),
        Token::CharLit(c) => Ok((input, Expr::CharLit(c))),
        Token::True => Ok((input, Expr::BoolLit(true))),
        Token::False => Ok((input, Expr::BoolLit(false))),
        Token::Unit => Ok((input, Expr::Unit)),
        _ => Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
    }
}

fn field_init(input: &str) -> ParseResult<FieldInit> {
    let (input, name) = ident(input)?;
    let (input, _) = expect_token(Token::Assign)(input)?;
    let (input, value) = expression(input)?;
    Ok((input, FieldInit { name, value: Box::new(value) }))
}

fn record_lit(input: &str) -> ParseResult<RecordLit> {
    let (input, name) = ident(input)?;
    let (input, _) = expect_token(Token::LBrace)(input)?;
    let (input, fields) = separated_list0(
        expect_token(Token::Comma),
        field_init
    )(input)?;
    let (input, _) = expect_token(Token::RBrace)(input)?;
    Ok((input, RecordLit { name, fields }))
}

#[allow(dead_code)]
fn unary_expr(input: &str) -> ParseResult<Expr> {
    alt((
        // Handle unary minus
        |input| {
            let (input, _) = expect_token(Token::Minus)(input)?;
            let (input, expr) = atom_expr(input)?;
            // Convert to negative literal if it's an integer literal
            match expr {
                Expr::IntLit(n) => Ok((input, Expr::IntLit(-n))),
                _ => Ok((input, Expr::Binary(BinaryExpr {
                    left: Box::new(Expr::IntLit(0)),
                    op: BinaryOp::Sub,
                    right: Box::new(expr),
                })))
            }
        },
        atom_expr
    ))(input)
}

fn atom_expr(input: &str) -> ParseResult<Expr> {
    alt((
        literal,
        lambda_expr,  // Try lambda before other expressions that use |
        some_expr,  // Try Some before ident
        none_expr,  // Try None before ident
        array_lit,  // Try array literal before list
        list_lit,  // Try list literal before record
        map(record_lit, Expr::RecordLit),  // Try record_lit before ident
        value(Expr::It, expect_token(Token::It)),  // 'it' keyword
        map(ident, Expr::Ident),
        // Unit literal () - must come before general parenthesized expressions
        value(
            Expr::Unit,
            tuple((expect_token(Token::LParen), expect_token(Token::RParen)))
        ),
        delimited(
            expect_token(Token::LParen),
            expression,
            expect_token(Token::RParen)
        ),
        with_expr,
        map(lazy_block_expr, Expr::Block)  // Blocks in expression position are lazy
    ))(input)
}

fn none_expr(input: &str) -> ParseResult<Expr> {
    let (input, _) = expect_token(Token::None)(input)?;
    
    // Check if we have a type parameter
    if let Ok((input, _)) = expect_token::<'_>(Token::Lt)(input) {
        let (input, ty) = parse_type(input)?;
        let (input, _) = expect_token(Token::Gt)(input)?;
        Ok((input, Expr::NoneTyped(ty)))
    } else {
        // Bare None - will use tagged union  
        Ok((input, Expr::None))
    }
}

fn some_expr(input: &str) -> ParseResult<Expr> {
    let (input, _) = expect_token(Token::Some)(input)?;
    let (input, _) = expect_token(Token::LParen)(input)?;
    let (input, expr) = expression(input)?;
    let (input, _) = expect_token(Token::RParen)(input)?;
    Ok((input, Expr::Some(Box::new(expr))))
}

fn list_lit(input: &str) -> ParseResult<Expr> {
    let (input, _) = expect_token(Token::LBracket)(input)?;
    let (input, elements) = separated_list0(
        expect_token(Token::Comma),
        map(expression, Box::new)
    )(input)?;
    let (input, _) = expect_token(Token::RBracket)(input)?;
    Ok((input, Expr::ListLit(elements)))
}

fn array_lit(input: &str) -> ParseResult<Expr> {
    let (input, _) = expect_token(Token::LArrayBracket)(input)?;
    let (input, elements) = separated_list0(
        expect_token(Token::Comma),
        map(expression, Box::new)
    )(input)?;
    let (input, _) = expect_token(Token::RArrayBracket)(input)?;
    Ok((input, Expr::ArrayLit(elements)))
}

fn lambda_expr(input: &str) -> ParseResult<Expr> {
    let (input, _) = expect_token(Token::Bar)(input)?;
    let (input, params) = separated_list0(
        expect_token(Token::Comma),
        ident
    )(input)?;
    let (input, _) = expect_token(Token::Bar)(input)?;
    let (input, body) = expression(input)?;
    Ok((input, Expr::Lambda(LambdaExpr {
        params,
        body: Box::new(body),
        has_implicit_param: false,  // TODO: detect 'it' usage
    })))
}

fn with_expr(input: &str) -> ParseResult<Expr> {
    let (input, _) = expect_token(Token::With)(input)?;
    
    // Check if this is a lifetime expression
    if let Ok((remaining, _)) = expect_token(Token::Lifetime)(input) {
        return with_lifetime_expr(input);
    }
    
    // Otherwise, parse as context expression
    let (input, contexts) = alt((
        delimited(
            expect_token(Token::LParen),
            separated_list0(expect_token(Token::Comma), ident),
            expect_token(Token::RParen)
        ),
        map(ident, |name| vec![name])
    ))(input)?;
    let (input, body) = block_expr(input)?;
    Ok((input, Expr::With(WithExpr { contexts, body })))
}

/// Parses a with lifetime expression.
/// 
/// # Examples
/// 
/// ```
/// with lifetime<~f> { ... }
/// with lifetime { ... }  // anonymous lifetime
/// ```
fn with_lifetime_expr(input: &str) -> ParseResult<Expr> {
    let (input, _) = expect_token(Token::Lifetime)(input)?;
    
    // Parse optional lifetime parameter
    let (input, lifetime_opt) = opt(
        delimited(
            expect_token(Token::Lt),
            preceded(expect_token(Token::Tilde), ident),
            expect_token(Token::Gt)
        )
    )(input)?;
    
    let (lifetime, anonymous) = match lifetime_opt {
        Some(name) => (name, false),
        None => {
            // For now, use a simple placeholder. In practice, this would be 
            // handled by the type checker's lifetime inference
            ("_anon".to_string(), true)
        },
    };
    
    // Parse optional where clause with temporal constraints
    let (input, constraints) = opt(preceded(
        expect_token(Token::Where),
        separated_list1(
            expect_token(Token::Comma),
            temporal_constraint
        )
    ))(input)?;
    
    let constraints = constraints.unwrap_or_default();
    
    let (input, body) = block_expr(input)?;
    
    Ok((input, Expr::WithLifetime(WithLifetimeExpr {
        lifetime,
        anonymous,
        constraints,
        body,
    })))
}

fn pattern(input: &str) -> ParseResult<Pattern> {
    alt((
        // Check for wildcard pattern
        |input| {
            let (input, token) = lex_token(input)?;
            match token {
                Token::Ident(s) if s == "_" => Ok((input, Pattern::Wildcard)),
                _ => Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
            }
        },
        some_pattern,
        none_pattern,
        record_pattern,  // Try record patterns before identifiers
        list_pattern,  // Try list patterns before literals
        map(literal, |expr| match expr {
            Expr::IntLit(n) => Pattern::Literal(Literal::Int(n)),
            Expr::FloatLit(f) => Pattern::Literal(Literal::Float(f)),
            Expr::StringLit(s) => Pattern::Literal(Literal::String(s)),
            Expr::CharLit(c) => Pattern::Literal(Literal::Char(c)),
            Expr::BoolLit(b) => Pattern::Literal(Literal::Bool(b)),
            Expr::Unit => Pattern::Literal(Literal::Unit),
            _ => unreachable!()
        }),
        map(ident, Pattern::Ident)
    ))(input)
}

fn some_pattern(input: &str) -> ParseResult<Pattern> {
    let (input, _) = expect_token(Token::Some)(input)?;
    let (input, _) = expect_token(Token::LParen)(input)?;
    let (input, pattern) = pattern(input)?;
    let (input, _) = expect_token(Token::RParen)(input)?;
    Ok((input, Pattern::Some(Box::new(pattern))))
}

fn none_pattern(input: &str) -> ParseResult<Pattern> {
    let (input, _) = expect_token(Token::None)(input)?;
    Ok((input, Pattern::None))
}

fn record_pattern(input: &str) -> ParseResult<Pattern> {
    // Try to parse an identifier followed by {
    let (input, name) = ident(input)?;
    let (input, _) = expect_token(Token::LBrace)(input)?;
    
    // Parse fields
    let (input, fields) = many0(
        |input| {
            let (input, field_name) = ident(input)?;
            // Check if there's a colon for an explicit pattern
            if let Ok((input, _)) = expect_token::<'_>(Token::Colon)(input) {
                let (input, pattern) = pattern(input)?;
                Ok((input, (field_name, pattern)))
            } else {
                // Shorthand: just field name binds to a variable
                Ok((input, (field_name.clone(), Pattern::Ident(field_name))))
            }
        }
    )(input)?;
    
    let (input, _) = expect_token(Token::RBrace)(input)?;
    Ok((input, Pattern::Record(name, fields)))
}

fn list_pattern(input: &str) -> ParseResult<Pattern> {
    let (input, _) = expect_token(Token::LBracket)(input)?;
    
    // Check for empty list pattern
    if let Ok((input, _)) = expect_token::<'_>(Token::RBracket)(input) {
        return Ok((input, Pattern::EmptyList));
    }
    
    // Parse first element
    let (input, first) = pattern(input)?;
    
    // Check if it's a cons pattern [head | tail]
    if let Ok((input, _)) = expect_token::<'_>(Token::Bar)(input) {
        let (input, tail) = pattern(input)?;
        let (input, _) = expect_token(Token::RBracket)(input)?;
        return Ok((input, Pattern::ListCons(Box::new(first), Box::new(tail))));
    }
    
    // Otherwise it's an exact pattern [a, b, c]
    let mut patterns = vec![Box::new(first)];
    
    // Parse remaining elements
    let (input, mut rest) = separated_list0(
        expect_token(Token::Comma),
        map(pattern, Box::new)
    )(input)?;
    patterns.append(&mut rest);
    
    let (input, _) = expect_token(Token::RBracket)(input)?;
    Ok((input, Pattern::ListExact(patterns)))
}

fn match_arm(input: &str) -> ParseResult<MatchArm> {
    let (input, pattern) = pattern(input)?;
    let (input, _) = expect_token(Token::Arrow)(input)?;
    let (input, body) = block_expr(input)?;
    Ok((input, MatchArm { pattern, body }))
}

#[allow(dead_code)]
fn match_expr(input: &str) -> ParseResult<Expr> {
    match_expr_with_context(input, false)
}

fn match_expr_with_context(input: &str, in_statement: bool) -> ParseResult<Expr> {
    let (input, expr) = pipe_expr_with_context(input, in_statement)?;
    let (input, arms) = opt(
        preceded(
            expect_token(Token::Match),
            delimited(
                expect_token(Token::LBrace),
                many1(match_arm),
                expect_token(Token::RBrace)
            )
        )
    )(input)?;
    
    match arms {
        Some(arms) => Ok((input, Expr::Match(MatchExpr { 
            expr: Box::new(expr), 
            arms 
        }))),
        None => Ok((input, expr))
    }
}

#[allow(dead_code)]
fn while_expr(input: &str) -> ParseResult<Expr> {
    while_expr_with_context(input, false)
}

fn while_expr_with_context(input: &str, in_statement: bool) -> ParseResult<Expr> {
    let (input, expr) = match_expr_with_context(input, in_statement)?;
    let (input, body) = opt(
        preceded(
            expect_token(Token::While),
            block_expr
        )
    )(input)?;
    
    match body {
        Some(body) => Ok((input, Expr::While(WhileExpr { 
            condition: Box::new(expr), 
            body 
        }))),
        None => Ok((input, expr))
    }
}

fn then_expr(input: &str) -> ParseResult<Expr> {
    then_expr_with_context(input, false)
}

fn then_expr_with_context(input: &str, in_statement: bool) -> ParseResult<Expr> {
    let (input, first_cond) = while_expr_with_context(input, in_statement)?;
    let (input, then_part) = opt(
        preceded(
            expect_token(Token::Then),
            tuple((
                block_expr,
                many0(tuple((
                    expect_token(Token::Else),
                    |i| while_expr_with_context(i, in_statement),
                    expect_token(Token::Then),
                    block_expr
                ))),
                opt(preceded(
                    expect_token(Token::Else),
                    block_expr
                ))
            ))
        )
    )(input)?;
    
    match then_part {
        Some((then_block, else_ifs, else_block)) => {
            let else_ifs = else_ifs.into_iter()
                .map(|(_, cond, _, block)| (Box::new(cond), block))
                .collect();
            Ok((input, Expr::Then(ThenExpr {
                condition: Box::new(first_cond),
                then_block,
                else_ifs,
                else_block
            })))
        },
        None => Ok((input, first_cond))
    }
}

fn binary_op(input: &str) -> ParseResult<BinaryOp> {
    let (input, token) = lex_token(input)?;
    match token {
        Token::Plus => Ok((input, BinaryOp::Add)),
        Token::Minus => Ok((input, BinaryOp::Sub)),
        Token::Star => Ok((input, BinaryOp::Mul)),
        Token::Slash => Ok((input, BinaryOp::Div)),
        Token::Percent => Ok((input, BinaryOp::Mod)),
        Token::Eq => Ok((input, BinaryOp::Eq)),
        Token::Ne => Ok((input, BinaryOp::Ne)),
        Token::Lt => Ok((input, BinaryOp::Lt)),
        Token::Le => Ok((input, BinaryOp::Le)),
        Token::Gt => Ok((input, BinaryOp::Gt)),
        Token::Ge => Ok((input, BinaryOp::Ge)),
        _ => Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
    }
}

fn pipe_op(input: &str) -> ParseResult<PipeOp> {
    let (input, token) = lex_token(input)?;
    match token {
        Token::Pipe => Ok((input, PipeOp::Pipe)),
        Token::PipeMut => Ok((input, PipeOp::PipeMut)),
        Token::Bar => Ok((input, PipeOp::Bar)),
        _ => Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
    }
}

#[allow(dead_code)]
fn binary_expr(input: &str) -> ParseResult<Expr> {
    binary_expr_with_context(input, false)
}

fn binary_expr_with_context(input: &str, in_statement: bool) -> ParseResult<Expr> {
    let (input, first) = call_expr_with_context(input, in_statement)?;
    
    // Try to parse binary operator and right operand
    let (input, rest) = many0(
        tuple((
            binary_op,
            |i| call_expr_with_context(i, in_statement)
        ))
    )(input)?;
    
    // Left-associative fold
    let expr = rest.into_iter().fold(first, |left, (op, right)| {
        Expr::Binary(BinaryExpr {
            left: Box::new(left),
            op,
            right: Box::new(right),
        })
    });
    
    Ok((input, expr))
}

#[allow(dead_code)]
fn pipe_expr(input: &str) -> ParseResult<Expr> {
    pipe_expr_with_context(input, false)
}

fn pipe_expr_with_context(input: &str, in_statement: bool) -> ParseResult<Expr> {
    let (input, first) = binary_expr_with_context(input, in_statement)?;
    let (input, pipes) = many0(
        tuple((
            pipe_op,
            alt((
                map(ident, PipeTarget::Ident),
                map(|i| binary_expr_with_context(i, in_statement), |e| PipeTarget::Expr(Box::new(e)))
            ))
        ))
    )(input)?;
    
    let expr = pipes.into_iter().fold(first, |acc, (op, target)| {
        Expr::Pipe(PipeExpr {
            expr: Box::new(acc),
            op,
            target
        })
    });
    Ok((input, expr))
}

#[allow(dead_code)]
fn call_expr(input: &str) -> ParseResult<Expr> {
    call_expr_with_context(input, false)
}

fn call_expr_with_context(input: &str, in_statement: bool) -> ParseResult<Expr> {
    alt((
        // Multiple arguments with parentheses: (a,b,c) func
        map(
            tuple((
                delimited(
                    expect_token(Token::LParen),
                    separated_list0(expect_token(Token::Comma), expression),
                    expect_token(Token::RParen)
                ),
                simple_expr
            )),
            |(args, func)| Expr::Call(CallExpr {
                function: Box::new(func),
                args: args.into_iter().map(Box::new).collect()
            })
        ),
        // Single expression or OSV style
        move |input| {
            let (input, first) = simple_expr(input)?;
            
            if in_statement {
                // In statement context, be conservative about consuming more expressions
                // Peek at the next tokens to see if this is a new statement
                if let Ok((_, Token::Val)) = lex_token(input) {
                    return Ok((input, first));
                }
                if let Ok((_, Token::Mut)) = lex_token(input) {
                    return Ok((input, first));
                }
                // Check for assignment pattern: ident =
                if let Ok((after_ident, Token::Ident(_))) = lex_token(input) {
                    if let Ok((_, Token::Assign)) = lex_token(after_ident) {
                        return Ok((input, first));
                    }
                    // Also stop before a bare identifier that might be a final expression
                    // Only continue if there's a clear operator or call pattern
                    // For now, conservatively stop before any identifier to avoid consuming final expressions
                    return Ok((input, first));
                }
                // Check for unit literal () which might be a separate statement/expression
                if let Ok((after_lparen, Token::LParen)) = lex_token(input) {
                    if let Ok((_, Token::RParen)) = lex_token(after_lparen) {
                        return Ok((input, first));
                    }
                }
                // Check for closing brace
                if let Ok((_, Token::RBrace)) = lex_token(input) {
                    return Ok((input, first));
                }
                // Don't consume more expressions if we see a binary operator coming
                if let Ok((_, tok)) = lex_token(input) {
                    match tok {
                        Token::Plus | Token::Minus | Token::Star | Token::Slash | Token::Percent |
                        Token::Eq | Token::Ne | Token::Lt | Token::Le | Token::Gt | Token::Ge |
                        Token::Pipe | Token::PipeMut | Token::Bar => {
                            return Ok((input, first));
                        }
                        _ => {}
                    }
                }
            }
            
            // Otherwise, try to parse more expressions for OSV
            // But don't parse expressions that start with binary operators
            let (input, rest) = many0(|input| {
                // Peek at the next token
                if let Ok((_, tok)) = lex_token(input) {
                    match tok {
                        // Don't parse if it starts with a binary operator
                        Token::Plus | Token::Minus | Token::Star | Token::Slash | Token::Percent |
                        Token::Eq | Token::Ne | Token::Lt | Token::Le | Token::Gt | Token::Ge => {
                            return Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)));
                        }
                        _ => {}
                    }
                }
                simple_expr(input)
            })(input)?;
            
            if rest.is_empty() {
                Ok((input, first))
            } else {
                // OSV: obj subj.verb => subj.verb(obj)
                let result = rest.into_iter().fold(first, |arg, func| {
                    Expr::Call(CallExpr {
                        function: Box::new(func),
                        args: vec![Box::new(arg)]
                    })
                });
                Ok((input, result))
            }
        }
    ))(input)
}

pub fn simple_expr(input: &str) -> ParseResult<Expr> {
    let (mut input, mut expr) = atom_expr(input)?;
    
    // Handle postfix operations
    loop {
        
        // Handle field access and other postfix operations
        let (new_input, op) = opt(alt((
            value(PostfixOp::Dot, expect_token(Token::Dot)),
            value(PostfixOp::Freeze, expect_token(Token::Freeze)),
        )))(input)?;
        
        match op {
            Some(PostfixOp::Dot) => {
                // Check if the next token is Clone keyword for .clone syntax
                if let Ok((_, Token::Clone)) = lex_token(new_input) {
                    // This is a .clone operation, not field access
                    let (new_input, _) = expect_token(Token::Clone)(new_input)?;
                    // Parse field updates for clone: { field1 = value1, field2 = value2, ... }
                    let (new_input, _) = expect_token(Token::LBrace)(new_input)?;
                    let (new_input, fields) = separated_list0(
                        expect_token(Token::Comma),
                        field_init
                    )(new_input)?;
                    let (new_input, _) = expect_token(Token::RBrace)(new_input)?;
                    
                    let mut clone_expr = Expr::Clone(CloneExpr {
                        base: Box::new(expr),
                        updates: RecordLit {
                            name: String::new(),
                            fields
                        }
                    });
                    
                    // Check if freeze follows the clone
                    if let Ok((freeze_input, _)) = expect_token::<'_>(Token::Freeze)(new_input) {
                        clone_expr = Expr::Freeze(Box::new(clone_expr));
                        input = freeze_input;
                    } else {
                        input = new_input;
                    }
                    
                    expr = clone_expr;
                } else {
                    // Regular field access
                    let (new_input, field) = ident(new_input)?;
                    expr = Expr::FieldAccess(Box::new(expr), field);
                    input = new_input;
                }
            }
            Some(PostfixOp::Freeze) => {
                expr = Expr::Freeze(Box::new(expr));
                input = new_input;
            }
            None => break,
        }
    }
    
    Ok((input, expr))
}

#[derive(Clone, Copy)]
enum PostfixOp {
    Dot,
    Freeze,
}

fn expression(input: &str) -> ParseResult<Expr> {
    then_expr(input)
}

fn expression_in_statement(input: &str) -> ParseResult<Expr> {
    then_expr_with_context(input, true)
}

#[allow(dead_code)]
fn statement(input: &str) -> ParseResult<Stmt> {
    alt((
        map(bind_decl, Stmt::Binding),
        assignment_stmt,
        map(expression, |e| Stmt::Expr(Box::new(e)))
    ))(input)
}

fn assignment_stmt(input: &str) -> ParseResult<Stmt> {
    let (input, name) = ident(input)?;
    let (input, _) = expect_token(Token::Assign)(input)?;
    let (input, value) = expression(input)?;  // Use normal expression parsing for assignment values
    Ok((input, Stmt::Assignment(AssignStmt {
        name,
        value: Box::new(value)
    })))
}

// Parse prototype clone expression: ParentType.clone { field: value } [freeze] [sealed]
fn prototype_clone_expr(input: &str) -> ParseResult<Expr> {
    let (input, base_name) = ident(input)?;
    let (input, _) = expect_token(Token::Dot)(input)?;
    let (input, _) = expect_token(Token::Clone)(input)?;
    let (input, _) = expect_token(Token::LBrace)(input)?;
    let (input, fields) = many0(field_init)(input)?;
    let (input, _) = expect_token(Token::RBrace)(input)?;
    
    // Check for freeze keyword
    let (input, freeze_immediately) = opt(|input| {
        expect_token(Token::Freeze)(input)
    })(input)?;
    let freeze_immediately = freeze_immediately.is_some();
    
    // Check for sealed keyword
    let (input, sealed) = opt(|input| {
        expect_token(Token::Sealed)(input)
    })(input)?;
    let sealed = sealed.is_some();
    
    Ok((input, Expr::PrototypeClone(PrototypeCloneExpr {
        base: base_name.clone(),
        updates: RecordLit { name: base_name, fields },
        freeze_immediately,
        sealed,
    })))
}

fn import_decl(input: &str) -> ParseResult<ImportDecl> {
    let (input, _) = expect_token(Token::Import)(input)?;
    let (input, module_path) = separated_list1(
        expect_token(Token::Dot),
        ident
    )(input)?;
    
    // Check for specific imports: .{foo, bar}
    let (input, items) = if let Ok((input, _)) = expect_token(Token::Dot)(input) {
        alt((
            // import module.*
            map(
                expect_token(Token::Star),
                |_| ImportItems::All
            ),
            // import module.{foo, bar}
            map(
                delimited(
                    expect_token(Token::LBrace),
                    separated_list0(expect_token(Token::Comma), ident),
                    expect_token(Token::RBrace)
                ),
                ImportItems::Named
            )
        ))(input)?
    } else {
        // import module (imports all)
        (input, ImportItems::All)
    };
    
    Ok((input, ImportDecl { module_path, items, span: None }))
}

fn export_decl(input: &str) -> ParseResult<TopDecl> {
    let (input, _) = expect_token(Token::Export)(input)?;
    let (input, item) = top_decl_inner(input)?;
    Ok((input, TopDecl::Export(ExportDecl {
        item: Box::new(item)
    })))
}

fn top_decl_inner(input: &str) -> ParseResult<TopDecl> {
    alt((
        map(fun_decl, TopDecl::Function),
        map(record_decl, TopDecl::Record),
        map(impl_block, TopDecl::Impl),
        map(context_decl, TopDecl::Context),
        map(bind_decl, TopDecl::Binding)
    ))(input)
}

pub fn top_decl(input: &str) -> ParseResult<TopDecl> {
    // Skip whitespace/comments before parsing a top-level declaration
    let (input, _) = skip(input)?;
    
    // Try export_decl first, but if it fails, make sure we have the right input
    match export_decl(input) {
        Ok(result) => Ok(result),
        Err(_) => {
            // export_decl failed, try top_decl_inner with the original input
            top_decl_inner(input)
        }
    }
}

pub fn parse_program(input: &str) -> ParseResult<Program> {
    let original = input;

    // Skip leading whitespace/comments first
    let (input, _) = skip(input)?;
    let (input, imports) = many0(import_decl)(input)?;

    // Parse declarations
    let mut remaining = input;
    let mut declarations = Vec::new();

    loop {
        // Skip any whitespace/comments first
        let (rest, _) = skip(remaining)?;

        // If nothing left after skipping whitespace, we're done
        if rest.is_empty() {
            remaining = rest;
            break;
        }

        // Try to parse a declaration
        match top_decl(rest) {
            Ok((rest2, decl)) => {
                declarations.push(decl);
                remaining = rest2;
            }
            Err(_) => {
                // If we can't parse more declarations, leave remaining as is
                remaining = rest;
                break;
            }
        }
    }

    let span = Some(make_span(original, original, remaining));
    Ok((remaining, Program { imports, declarations, span }))
}

/// Parse result with detailed error information for LSP.
#[derive(Debug, Clone)]
pub struct ParseError {
    /// Error message
    pub message: String,
    /// Location of the error
    pub span: Span,
}

/// Result of parsing with error recovery.
#[derive(Debug)]
pub struct ParseResult2 {
    /// The (possibly partial) AST
    pub program: Program,
    /// Collected errors during parsing
    pub errors: Vec<ParseError>,
}

/// Parses a program and returns detailed error information on failure.
pub fn parse_program_with_errors(input: &str) -> Result<Program, ParseError> {
    match parse_program(input) {
        Ok((remaining, mut program)) => {
            if !remaining.trim().is_empty() {
                let pos = input.len() - remaining.len();
                Err(ParseError {
                    message: format!("Unexpected input: '{}'", remaining.chars().take(30).collect::<String>()),
                    span: Span::new(pos, pos + remaining.len().min(30)),
                })
            } else {
                // Set program span to cover entire input
                program.span = Some(Span::new(0, input.len()));
                Ok(program)
            }
        }
        Err(e) => {
            // Try to extract position from nom error
            let (msg, span) = match e {
                nom::Err::Error(ref err) | nom::Err::Failure(ref err) => {
                    let pos = input.len() - err.input.len();
                    (format!("Parse error: {:?}", err.code), Span::new(pos, pos + 1))
                }
                nom::Err::Incomplete(_) => {
                    ("Incomplete input".to_string(), Span::new(input.len(), input.len()))
                }
            };
            Err(ParseError { message: msg, span })
        }
    }
}

/// Parses a program with error recovery, returning a partial AST and all errors.
///
/// This is useful for IDE integration where we want to provide diagnostics
/// even when the code has syntax errors.
pub fn parse_program_recovering(input: &str) -> ParseResult2 {
    let mut errors = Vec::new();
    let mut declarations = Vec::new();
    let mut imports = Vec::new();
    let mut remaining = input;

    // Skip leading whitespace
    if let Ok((rest, _)) = skip(remaining) {
        remaining = rest;
    }

    // Try to parse imports
    loop {
        let (rest, _) = skip(remaining).unwrap_or((remaining, ()));
        remaining = rest;

        if remaining.is_empty() {
            break;
        }

        // Check if this looks like an import
        if let Ok((_, Token::Import)) = lex_token(remaining) {
            match import_decl(remaining) {
                Ok((rest, import)) => {
                    imports.push(import);
                    remaining = rest;
                }
                Err(_) => {
                    // Skip to next line or declaration
                    remaining = skip_to_next_decl(remaining, &mut errors);
                    break;
                }
            }
        } else {
            break;
        }
    }

    // Parse declarations with recovery
    loop {
        let (rest, _) = skip(remaining).unwrap_or((remaining, ()));
        remaining = rest;

        if remaining.is_empty() {
            break;
        }

        match top_decl(remaining) {
            Ok((rest, decl)) => {
                declarations.push(decl);
                remaining = rest;
            }
            Err(e) => {
                // Record the error
                let pos = input.len() - remaining.len();
                let error_msg = match &e {
                    nom::Err::Error(err) | nom::Err::Failure(err) => {
                        format!("Syntax error: {:?}", err.code)
                    }
                    nom::Err::Incomplete(_) => "Incomplete input".to_string(),
                };

                // Try to determine error span
                let error_end = remaining
                    .find('\n')
                    .map(|n| pos + n)
                    .unwrap_or(input.len());

                errors.push(ParseError {
                    message: error_msg,
                    span: Span::new(pos, error_end.min(pos + 50)),
                });

                // Skip to next declaration
                remaining = skip_to_next_decl(remaining, &mut errors);

                if remaining.is_empty() {
                    break;
                }
            }
        }
    }

    ParseResult2 {
        program: Program {
            imports,
            declarations,
            span: Some(Span::new(0, input.len())),
        },
        errors,
    }
}

/// Skips input until the next likely declaration start.
fn skip_to_next_decl<'a>(input: &'a str, _errors: &mut Vec<ParseError>) -> &'a str {
    let mut remaining = input;

    // Skip to next line first
    if let Some(newline_pos) = remaining.find('\n') {
        remaining = &remaining[newline_pos + 1..];
    } else {
        return "";
    }

    // Look for declaration keywords
    let decl_keywords = ["fun", "record", "context", "impl", "val", "export", "import"];

    loop {
        // Skip whitespace
        let trimmed = remaining.trim_start();
        if trimmed.is_empty() {
            return "";
        }

        // Check if we're at a declaration keyword
        for keyword in &decl_keywords {
            if trimmed.starts_with(keyword) {
                // Make sure it's a word boundary
                let after_keyword = &trimmed[keyword.len()..];
                if after_keyword.is_empty() ||
                   !after_keyword.chars().next().unwrap().is_alphanumeric() {
                    return trimmed;
                }
            }
        }

        // Skip to next line
        if let Some(newline_pos) = remaining.find('\n') {
            remaining = &remaining[newline_pos + 1..];
        } else {
            return "";
        }
    }
}

/// Helper function to detect if a block uses the 'it' keyword
fn block_uses_it(statements: &[Stmt], final_expr: &Option<Box<Expr>>) -> bool {
    // Check all statements
    for stmt in statements {
        if stmt_uses_it(stmt) {
            return true;
        }
    }

    // Check final expression
    if let Some(expr) = final_expr {
        if expr_uses_it(expr) {
            return true;
        }
    }

    false
}

/// Check if a statement uses 'it'
fn stmt_uses_it(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Binding(bind) => expr_uses_it(&bind.value),
        Stmt::Assignment(assign) => expr_uses_it(&assign.value),
        Stmt::Expr(expr) => expr_uses_it(expr),
    }
}

/// Recursively check if an expression uses 'it'
fn expr_uses_it(expr: &Expr) -> bool {
    match expr {
        Expr::It => true,
        Expr::Binary(bin) => expr_uses_it(&bin.left) || expr_uses_it(&bin.right),
        Expr::Call(call) => {
            expr_uses_it(&call.function) || call.args.iter().any(|arg| expr_uses_it(arg))
        }
        Expr::Block(block) => block_uses_it(&block.statements, &block.expr),
        Expr::Pipe(pipe) => {
            expr_uses_it(&pipe.expr) ||
            match &pipe.target {
                PipeTarget::Expr(e) => expr_uses_it(e),
                _ => false,
            }
        }
        Expr::Then(then_expr) => {
            expr_uses_it(&then_expr.condition) ||
            block_uses_it(&then_expr.then_block.statements, &then_expr.then_block.expr) ||
            then_expr.else_block.as_ref().map_or(false, |b| block_uses_it(&b.statements, &b.expr))
        }
        Expr::While(while_expr) => {
            expr_uses_it(&while_expr.condition) ||
            block_uses_it(&while_expr.body.statements, &while_expr.body.expr)
        }
        Expr::Match(match_expr) => {
            expr_uses_it(&match_expr.expr) ||
            match_expr.arms.iter().any(|arm| block_uses_it(&arm.body.statements, &arm.body.expr))
        }
        Expr::Lambda(lambda) => expr_uses_it(&lambda.body),
        Expr::FieldAccess(obj, _) => expr_uses_it(obj),
        Expr::ListLit(items) | Expr::ArrayLit(items) => items.iter().any(|item| expr_uses_it(item)),
        Expr::Some(inner) => expr_uses_it(inner),
        Expr::RecordLit(rec) => rec.fields.iter().any(|f| expr_uses_it(&f.value)),
        Expr::Clone(clone) => {
            expr_uses_it(&clone.base) ||
            clone.updates.fields.iter().any(|f| expr_uses_it(&f.value))
        }
        Expr::Freeze(inner) => expr_uses_it(inner),
        Expr::PrototypeClone(proto) => proto.updates.fields.iter().any(|f| expr_uses_it(&f.value)),
        Expr::With(with) => block_uses_it(&with.body.statements, &with.body.expr),
        Expr::ScopeCompose(sc) => expr_uses_it(&sc.left) || expr_uses_it(&sc.right),
        Expr::ScopeConcat(sc) => expr_uses_it(&sc.left) || expr_uses_it(&sc.right),
        Expr::WithLifetime(wl) => block_uses_it(&wl.body.statements, &wl.body.expr),
        Expr::Await(inner) | Expr::Spawn(inner) => expr_uses_it(inner),
        // Literals and identifiers don't use 'it'
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::StringLit(_) |
        Expr::CharLit(_) | Expr::BoolLit(_) | Expr::Unit |
        Expr::Ident(_) | Expr::None | Expr::NoneTyped(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_decl() {
        let input = "record Enemy { hp: Int atk: Int }";
        let (_, decl) = record_decl(input).unwrap();
        assert_eq!(decl.name, "Enemy");
        assert_eq!(decl.fields.len(), 2);
    }

    #[test]
    fn test_fun_decl() {
        let input = "fun add: (a:Int, b:Int) -> Int = { a }";
        let (_, decl) = fun_decl(input).unwrap();
        assert_eq!(decl.name, "add");
        assert_eq!(decl.params.len(), 2);
    }

    #[test]
    fn test_fun_decl_unit_return() {
        let input = "fun simple: (x: Int) -> Unit = { () }";
        let result = fun_decl(input);
        assert!(result.is_ok(), "Failed to parse function with Unit return type: {:?}", result.err());
        let (_, decl) = result.unwrap();
        assert_eq!(decl.name, "simple");
    }

    #[test]
    fn test_pipe_expr() {
        let input = "42 |> add 10";
        let (_, expr) = pipe_expr(input).unwrap();
        assert!(matches!(expr, Expr::Pipe(_)));
    }
    
    #[test]
    fn test_clone_freeze() {
        let input = "base.clone { hp = 500 } freeze";
        let (_, expr) = simple_expr(input).unwrap();
        assert!(matches!(expr, Expr::Freeze(_)));
    }
    
    #[test]
    fn test_field_access() {
        let input = "obj.field";
        let (_, expr) = simple_expr(input).unwrap();
        assert!(matches!(expr, Expr::FieldAccess(_, _)));
    }
    
    #[test]
    fn test_temporal_constraint() {
        let input = "~tx within ~db";
        let (_, constraint) = temporal_constraint(input).unwrap();
        assert_eq!(constraint.inner, "tx");
        assert_eq!(constraint.outer, "db");
    }
    
    #[test]
    fn test_with_lifetime() {
        let input = "with lifetime<~f> { 42 }";
        let (_, expr) = with_expr(input).unwrap();
        if let Expr::WithLifetime(ref wl) = expr {
            assert_eq!(wl.lifetime, "f");
            assert!(!wl.anonymous);
            assert!(wl.constraints.is_empty());
        } else {
            panic!("Expected WithLifetime expression");
        }
    }

    #[test]
    fn test_parse_program_with_errors() {
        // Use correct syntax: fun name: (params) -> ReturnType = { body }
        let input = "fun main: () -> Int = { 42 }";
        let result = parse_program_with_errors(input);
        match &result {
            Ok(program) => {
                assert_eq!(program.declarations.len(), 1);
                assert!(program.span.is_some());
            }
            Err(e) => {
                panic!("Parse failed: {} at {:?}", e.message, e.span);
            }
        }
    }

    #[test]
    fn test_parse_program_recovering_valid() {
        let input = "fun main: () -> Int = { 42 }\nfun second: () -> Int = { 10 }";
        let result = parse_program_recovering(input);
        assert!(result.errors.is_empty(), "Unexpected errors: {:?}", result.errors);
        assert_eq!(result.program.declarations.len(), 2);
    }

    #[test]
    fn test_parse_program_recovering_with_errors() {
        // Invalid syntax in the middle - should recover and parse the second function
        let input = "fun first: () -> Int = { 42 }\n@#$invalid\nfun second: () -> Int = { 10 }";
        let result = parse_program_recovering(input);
        // Should have at least one error
        assert!(!result.errors.is_empty(), "Expected at least one error");
        // Should have parsed the first function at minimum
        assert!(!result.program.declarations.is_empty(), "Expected at least one declaration");
    }

    #[test]
    fn test_parse_error_span() {
        let input = "fun main: () -> Unit = { () }";
        let result = parse_program_with_errors(input);
        // This should succeed with unit return
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }
}