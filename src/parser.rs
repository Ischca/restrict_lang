//! # Parser Module
//!
//! The parser transforms a stream of tokens into an Abstract Syntax Tree (AST).
//! It implements Restrict Language's unique OSV (Object-Subject-Verb) syntax
//! and handles affine type constraints during parsing.
//!
//! ## Key Features
//!
//! - **OSV Syntax**: Natural handling of the pipe operator (`|>`)
//! - **Pattern Matching**: Comprehensive pattern support including list patterns
//! - **Generic Functions**: Type parameters with bounds and derivation constraints
//! - **Prototype System**: Parsing of `clone` and `freeze` operations
//!
//! ## Example
//!
//! ```rust
//! use restrict_lang::parser::parse_program;
//!
//! let input = r#"fun main: () -> String = { "Hello, World!" }"#;
//!
//! let (remaining, ast) = parse_program(input).unwrap();
//! assert!(remaining.trim().is_empty());
//! assert_eq!(ast.declarations.len(), 1);
//! ```

use crate::ast::*;
use crate::lexer::{lex_token, skip, Token};
use nom::{
    branch::alt,
    combinator::{map, opt, value},
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{delimited, preceded, tuple},
    IResult,
};

/// Type alias for parser results.
type ParseResult<'a, T> = IResult<&'a str, T>;

const GENERIC_FORM_ERROR: &str = "generic forms are not supported yet; declare a non-generic form";
const GENERIC_TAKES_ERROR: &str =
    "generic or temporal takes targets are not supported yet; adopt a form for a concrete nominal type";
const ASSOCIATED_FORM_TYPE_ERROR: &str =
    "source-level associated form types are not supported yet; forms may only require typed methods";
const DEFAULT_FORM_METHOD_ERROR: &str =
    "default form method bodies are not supported yet; implement the method in a takes declaration";
const CONDITIONAL_TAKES_ERROR: &str = "conditional takes declarations are not supported yet";
const EXPORTED_TAKES_ERROR: &str =
    "takes declarations cannot be public; export the form and nominal type instead";
const UNSUPPORTED_IMPORT_ALIAS_ERROR: &str =
    "string import paths and import aliases are unsupported in v0.0.1; use dotted source imports such as import module.{item}";
const UNSUPPORTED_RE_EXPORT_ERROR: &str =
    "re-exports are unsupported in v0.0.1; import declarations must stay at the source module boundary";
const STALE_LET_ERROR: &str =
    "stale syntax `let` is not valid Restrict; use `val` for immutable bindings";
const STALE_IF_ERROR: &str =
    "stale syntax `if (...)` is not valid Restrict; use condition-first `then` expressions";
const STALE_VAL_MUT_ERROR: &str =
    "stale syntax `val mut` is not valid Restrict; write mutable bindings as `mut val`";
const TRADITIONAL_CALL_ERROR: &str =
    "traditional calls like `add(1, 2)` are not valid Restrict; use OSV syntax such as `(1, 2) add` or `value |> add`";
const STALE_OPTION_VALUE_ERROR: &str =
    "unqualified Option value construction is not valid Restrict; write `value Option::Some` or `() Option::None` (pipe is optional for `Option::Some`)";
const STALE_RESULT_VALUE_ERROR: &str =
    "traditional Result value construction is not valid Restrict; write `value Result::Ok` or `error Result::Err` (pipe is optional)";
const BUILTIN_CONSTRUCTOR_TYPE_ARGUMENT_ERROR: &str =
    "write qualified OSV such as `() Option::None`; built-in constructors for Option or Result do not take source type arguments, so provide the full type through an annotation or typed context";
const STALE_UNIT_ERROR: &str =
    "stale syntax `Unit` is not valid Restrict; use `()` for the unit value or unit type";
const STALE_FUNCTION_DECL_ERROR: &str =
    "stale function declaration syntax is not valid Restrict; write `fun name: (param: Type, ...) = { ... }` and use `()` for no parameters";

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
            Err(nom::Err::Error(nom::error::Error::new(
                original_input,
                nom::error::ErrorKind::Tag,
            )))
        }
    }
}

fn user_syntax_failure<'a, T>(message: &'static str) -> ParseResult<'a, T> {
    Err(nom::Err::Failure(nom::error::Error::new(
        message,
        nom::error::ErrorKind::Fail,
    )))
}

fn starts_with_word(input: &str, word: &str) -> bool {
    let trimmed = input.trim_start();
    let Some(rest) = trimmed.strip_prefix(word) else {
        return false;
    };
    rest.chars()
        .next()
        .is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_')
}

fn starts_with_let_binding(input: &str) -> bool {
    starts_with_word(input, "let")
}

fn starts_with_if_parens(input: &str) -> bool {
    let trimmed = input.trim_start();
    let Some(rest) = trimmed.strip_prefix("if") else {
        return false;
    };
    rest.trim_start().starts_with('(')
}

fn starts_with_val_mut(input: &str) -> bool {
    let trimmed = input.trim_start();
    let Some(rest) = trimmed.strip_prefix("val") else {
        return false;
    };
    starts_with_word(rest, "mut")
}

/// Parses an identifier.
///
/// # Example
///
/// ```
/// // Parses: myVariable, userName, _private
/// ```
fn ident(input: &str) -> ParseResult<'_, String> {
    let original_input = input;
    let (input, token) = lex_token(input)?;
    match token {
        Token::Ident(name) => Ok((input, name)),
        _ => Err(nom::Err::Error(nom::error::Error::new(
            original_input,
            nom::error::ErrorKind::Tag,
        ))),
    }
}

/// Parses a type expression.
///
/// Handles both simple types, generic types, and temporal types.
///
/// # Examples
///
/// ```
/// // Simple types: Int32, String, Point
/// // Generic types: List<Int32>, Result<String, Error>
/// // Temporal types: File<~f>, Transaction<~tx, ~db>
/// ```
fn parse_type(input: &str) -> ParseResult<'_, Type> {
    parse_function_type(input)
}

enum TypeArg {
    Type(Type),
    Temporal(String),
}

fn parse_type_arg(input: &str) -> ParseResult<'_, TypeArg> {
    alt((
        map(
            preceded(expect_token(Token::Tilde), ident),
            TypeArg::Temporal,
        ),
        |input| {
            let original_input = input;
            let (input, token) = lex_token(input)?;
            match token {
                Token::IntLit(value) if value >= 0 => {
                    Ok((input, TypeArg::Type(Type::Named(value.to_string()))))
                }
                _ => Err(nom::Err::Error(nom::error::Error::new(
                    original_input,
                    nom::error::ErrorKind::Tag,
                ))),
            }
        },
        map(parse_type, TypeArg::Type),
    ))(input)
}

fn parse_function_type(input: &str) -> ParseResult<'_, Type> {
    if let Ok((after_params, params)) = delimited(
        expect_token(Token::LParen),
        separated_list0(expect_token(Token::Comma), parse_type),
        expect_token(Token::RParen),
    )(input)
    {
        if let Ok((after_arrow, _)) = expect_token::<'_>(Token::ThinArrow)(after_params) {
            let (input, return_type) = parse_type(after_arrow)?;
            return Ok((input, Type::Function(params, Box::new(return_type))));
        }

        if let [single] = params.as_slice() {
            return Ok((after_params, single.clone()));
        }

        if params.is_empty() {
            return Ok((after_params, Type::Named("Unit".to_string())));
        }
    }

    let (input, param_type) = parse_type_atom(input)?;
    if let Ok((input, _)) = expect_token::<'_>(Token::ThinArrow)(input) {
        let (input, return_type) = parse_type(input)?;
        Ok((
            input,
            Type::Function(vec![param_type], Box::new(return_type)),
        ))
    } else {
        Ok((input, param_type))
    }
}

fn parse_type_atom(input: &str) -> ParseResult<'_, Type> {
    if let Ok((_, Token::Unit)) = lex_token(input) {
        return user_syntax_failure(STALE_UNIT_ERROR);
    }

    let (input, name) = ident(input)?;
    let (input, type_params) = opt(delimited(
        expect_token(Token::Lt),
        separated_list0(expect_token(Token::Comma), parse_type_arg),
        expect_token(Token::Gt),
    ))(input)?;

    match type_params {
        Some(params) => {
            // Check if all are temporal
            let all_temporal = params
                .iter()
                .all(|param| matches!(param, TypeArg::Temporal(_)));
            let all_regular = params.iter().all(|param| matches!(param, TypeArg::Type(_)));

            if all_temporal {
                // All temporal: File<~f>
                Ok((
                    input,
                    Type::Temporal(
                        name,
                        params
                            .into_iter()
                            .map(|param| match param {
                                TypeArg::Temporal(name) => name,
                                TypeArg::Type(_) => unreachable!(),
                            })
                            .collect(),
                    ),
                ))
            } else if all_regular {
                // All regular types: Vec<String>
                let types = params
                    .into_iter()
                    .map(|param| match param {
                        TypeArg::Type(ty) => ty,
                        TypeArg::Temporal(_) => unreachable!(),
                    })
                    .collect();
                Ok((input, Type::Generic(name, types)))
            } else {
                // Mixed not supported yet
                Err(nom::Err::Error(nom::error::Error::new(
                    input,
                    nom::error::ErrorKind::Tag,
                )))
            }
        }
        None => Ok((input, Type::Named(name))),
    }
}

fn field_decl(input: &str) -> ParseResult<'_, FieldDecl> {
    let (input, name) = ident(input)?;
    let (input, _) = expect_token(Token::Colon)(input)?;
    let (input, ty) = parse_type(input)?;
    Ok((input, FieldDecl { name, ty }))
}

fn field_decls(input: &str) -> ParseResult<'_, Vec<FieldDecl>> {
    let mut fields = Vec::new();
    let mut remaining = input;

    loop {
        let (input, _) = skip(remaining)?;

        if expect_token::<'_>(Token::RBrace)(input).is_ok() {
            return Ok((input, fields));
        }

        let (input, field) = field_decl(input)?;
        fields.push(field);

        let (input, _) = opt(expect_token(Token::Comma))(input)?;
        remaining = input;
    }
}

fn record_decl(input: &str) -> ParseResult<'_, RecordDecl> {
    let (input, _) = expect_token(Token::Record)(input)?;
    let (input, name) = ident(input)?;

    // Parse optional type parameters: <T, ~f>
    let (input, type_params) = opt(|input| {
        let (input, _) = expect_token(Token::Lt)(input)?;
        let (input, params) = separated_list1(expect_token(Token::Comma), type_param)(input)?;
        let (input, _) = expect_token(Token::Gt)(input)?;
        Ok((input, params))
    })(input)?;
    let type_params = type_params.unwrap_or_default();

    // Parse optional temporal constraints: where ~tx within ~db
    let (input, temporal_constraints) = opt(|input| {
        let (input, _) = expect_token(Token::Where)(input)?;
        separated_list1(expect_token(Token::Comma), temporal_constraint)(input)
    })(input)?;
    let temporal_constraints = temporal_constraints.unwrap_or_default();

    let (input, _) = expect_token(Token::LBrace)(input)?;
    let (input, fields) = field_decls(input)?;
    let (input, _) = expect_token(Token::RBrace)(input)?;

    // For now, skip freeze/sealed checks to debug parsing issue
    let frozen = false;
    let sealed = false;

    Ok((
        input,
        RecordDecl {
            name,
            type_params,
            temporal_constraints,
            fields,
            frozen,
            sealed,
            parent_hash: None,
        },
    ))
}

fn enum_variant_decl(input: &str) -> ParseResult<'_, EnumVariantDecl> {
    let (input, name) = ident(input)?;
    let (input, payload) = opt(delimited(
        expect_token(Token::LParen),
        parse_type,
        expect_token(Token::RParen),
    ))(input)?;
    Ok((input, EnumVariantDecl { name, payload }))
}

fn enum_decl(input: &str) -> ParseResult<'_, EnumDecl> {
    let (input, _) = expect_token(Token::Enum)(input)?;
    let (input, name) = ident(input)?;
    let (input, _) = expect_token(Token::LBrace)(input)?;

    let (mut input, first) = enum_variant_decl(input)?;
    let mut variants = vec![first];

    loop {
        let (after_comma, _) = opt(expect_token(Token::Comma))(input)?;
        input = after_comma;

        if let Ok((after_brace, _)) = expect_token::<'_>(Token::RBrace)(input) {
            return Ok((after_brace, EnumDecl { name, variants }));
        }

        // Whitespace and newlines are already skipped by token parsing, so a
        // comma is optional between variants while a trailing comma is also
        // accepted.
        let (after_variant, variant) = enum_variant_decl(input)?;
        variants.push(variant);
        input = after_variant;
    }
}

// Parse a temporal constraint: ~tx within ~db
fn temporal_constraint(input: &str) -> ParseResult<'_, TemporalConstraint> {
    let (input, _) = expect_token(Token::Tilde)(input)?;
    let (input, inner) = ident(input)?;
    let (input, _) = expect_token(Token::Within)(input)?;
    let (input, _) = expect_token(Token::Tilde)(input)?;
    let (input, outer) = ident(input)?;
    Ok((input, TemporalConstraint { inner, outer }))
}

fn param(input: &str) -> ParseResult<'_, Param> {
    // For now, skip context bounds since we don't have @ token
    let context_bound = None;

    let (input, name) = ident(input)?;
    let (input, _) = expect_token(Token::Colon)(input)?;
    let (input, ty) = parse_type(input)?;
    Ok((
        input,
        Param {
            name,
            ty,
            context_bound,
        },
    ))
}

fn block_expr(input: &str) -> ParseResult<'_, BlockExpr> {
    let (input, _) = expect_token(Token::LBrace)(input)?;
    block_expr_body(input)
}

/// Parse a block after its opening brace has already been consumed.
///
/// Scoped verb clauses reuse the ordinary block grammar after an optional
/// lambda-style binder header, so keeping the body parser separate prevents
/// the two block forms from drifting apart.
fn block_expr_body(input: &str) -> ParseResult<'_, BlockExpr> {
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

        if starts_with_let_binding(remaining) {
            return user_syntax_failure(STALE_LET_ERROR);
        }
        if starts_with_if_parens(remaining) {
            return user_syntax_failure(STALE_IF_ERROR);
        }
        if starts_with_val_mut(remaining) {
            return user_syntax_failure(STALE_VAL_MUT_ERROR);
        }

        // Try to parse a binding first
        if let Ok((after_bind, bind_decl)) = bind_decl_in_statement(remaining) {
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

    Ok((
        remaining,
        BlockExpr {
            statements,
            expr: final_expr,
        },
    ))
}

fn fun_decl(input: &str) -> ParseResult<'_, FunDecl> {
    // Skip leading whitespace
    let (input, _) = skip(input)?;

    // Check for optional async keyword
    let (input, is_async) = opt(expect_token(Token::Async))(input)?;
    let is_async = is_async.is_some();

    let (input, _) = expect_token(Token::Fun)(input)?;
    let (input, name) = ident(input)?;

    if matches!(lex_token(input), Ok((_, Token::Assign))) {
        return user_syntax_failure(STALE_FUNCTION_DECL_ERROR);
    }
    let (input, _) = expect_token(Token::Colon)(input)?;

    let (input, type_params) = opt(|input| {
        let (input, _) = expect_token(Token::Lt)(input)?;
        let (input, params) = separated_list1(expect_token(Token::Comma), type_param)(input)?;
        let (input, _) = expect_token(Token::Gt)(input)?;
        Ok((input, params))
    })(input)?;
    let type_params = type_params.unwrap_or_default();

    if !matches!(lex_token(input), Ok((_, Token::LParen))) {
        return user_syntax_failure(STALE_FUNCTION_DECL_ERROR);
    }
    let (input, _) = expect_token(Token::LParen)(input)?;
    let (input, params) = separated_list0(expect_token(Token::Comma), param)(input)?;
    let (input, _) = expect_token(Token::RParen)(input)?;

    let (input, return_type) = opt(|input| {
        let (input, _) = expect_token(Token::ThinArrow)(input)?;
        parse_type(input)
    })(input)?;

    let (input, temporal_constraints) = opt(|input| {
        let (input, _) = expect_token(Token::Where)(input)?;
        separated_list1(expect_token(Token::Comma), temporal_constraint)(input)
    })(input)?;
    let temporal_constraints = temporal_constraints.unwrap_or_default();

    let (input, _) = expect_token(Token::Assign)(input)?;

    let (input, body) = block_expr(input)?;

    Ok((
        input,
        FunDecl {
            name,
            is_async,
            type_params,
            temporal_constraints,
            params,
            return_type,
            body,
        },
    ))
}

// Parse a type parameter with optional bounds: T: Display + Clone and derivation bound: T from ParentType
// Also supports temporal type parameters: ~t
fn type_param(input: &str) -> ParseResult<'_, TypeParam> {
    // Check if this is a temporal type parameter
    let (input, is_temporal) = opt(expect_token(Token::Tilde))(input)?;
    let is_temporal = is_temporal.is_some();

    let (input, name) = ident(input)?;

    // Temporal parameters don't have trait bounds or derivation bounds
    if is_temporal {
        return Ok((
            input,
            TypeParam {
                name,
                bounds: vec![],
                derivation_bound: None,
                of_forms: vec![],
                is_temporal: true,
            },
        ));
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
            },
        )(input)
    })(input)?;

    let bounds = bounds.unwrap_or_default();

    // Parse form constraints: T of Showable + Comparable
    let (input, of_forms) = opt(|input| {
        let (input, _) = expect_token(Token::Of)(input)?;
        separated_list1(expect_token(Token::Plus), ident)(input)
    })(input)?;
    let of_forms = of_forms.unwrap_or_default();

    Ok((
        input,
        TypeParam {
            name,
            bounds,
            derivation_bound,
            of_forms,
            is_temporal: false,
        },
    ))
}

#[allow(dead_code)]
fn impl_block(input: &str) -> ParseResult<'_, ImplBlock> {
    let (input, _) = expect_token(Token::Impl)(input)?;
    let (input, target) = ident(input)?;
    let (input, _) = expect_token(Token::LBrace)(input)?;
    let (input, functions) = many0(fun_decl)(input)?;
    let (input, _) = expect_token(Token::RBrace)(input)?;
    Ok((input, ImplBlock { target, functions }))
}

fn context_decl(input: &str) -> ParseResult<'_, ContextDecl> {
    let (input, _) = expect_token(Token::Context)(input)?;
    let (input, name) = ident(input)?;

    // Parse optional type parameters (including temporal): <~fs>
    let (input, _type_params) = opt(|input| {
        let (input, _) = expect_token(Token::Lt)(input)?;
        let (input, params) = separated_list1(expect_token(Token::Comma), type_param)(input)?;
        let (input, _) = expect_token(Token::Gt)(input)?;
        Ok((input, params))
    })(input)?;

    // TODO: Handle type parameters in context declaration
    // For now, we'll ignore them until we update the ContextDecl struct

    let (input, _) = expect_token(Token::LBrace)(input)?;
    let (input, fields) = field_decls(input)?;
    let (input, _) = expect_token(Token::RBrace)(input)?;
    Ok((input, ContextDecl { name, fields }))
}

fn bind_decl_in_statement(input: &str) -> ParseResult<'_, BindDecl> {
    let (input, mutable) = opt(expect_token(Token::Mut))(input)?;
    let (input, _) = expect_token(Token::Val)(input)?;

    let (input, bind_pattern) = pattern(input)?;

    let (input, type_annotation) = opt(preceded(expect_token(Token::Colon), parse_type))(input)?;
    let (input, _) = expect_token(Token::Assign)(input)?;
    let (input, value) = expression_in_statement(input)?; // Use statement-aware expression parsing
    Ok((
        input,
        BindDecl {
            mutable: mutable.is_some(),
            pattern: bind_pattern,
            type_annotation,
            value: Box::new(value),
        },
    ))
}

pub fn bind_decl(input: &str) -> ParseResult<'_, BindDecl> {
    let (input, mutable) = opt(expect_token(Token::Mut))(input)?;
    let (input, _) = expect_token(Token::Val)(input)?;

    let (input, bind_pattern) = pattern(input)?;

    let (input, type_annotation) = opt(preceded(expect_token(Token::Colon), parse_type))(input)?;
    let (input, _) = expect_token(Token::Assign)(input)?;
    let (input, value) = expression(input)?; // Use normal expression parsing for binding values
    Ok((
        input,
        BindDecl {
            mutable: mutable.is_some(),
            pattern: bind_pattern,
            type_annotation,
            value: Box::new(value),
        },
    ))
}

fn literal(input: &str) -> ParseResult<'_, Expr> {
    let (input, token) = lex_token(input)?;
    match token {
        Token::IntLit(n) => Ok((input, Expr::new(ExprKind::IntLit(n)))),
        Token::FloatLit(f) => Ok((input, Expr::new(ExprKind::FloatLit(f)))),
        Token::StringLit(s) => Ok((input, Expr::new(ExprKind::StringLit(s)))),
        Token::CharLit(c) => Ok((input, Expr::new(ExprKind::CharLit(c)))),
        Token::True => Ok((input, Expr::new(ExprKind::BoolLit(true)))),
        Token::False => Ok((input, Expr::new(ExprKind::BoolLit(false)))),
        Token::Unit => user_syntax_failure(STALE_UNIT_ERROR),
        _ => Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        ))),
    }
}

fn field_init(input: &str) -> ParseResult<'_, FieldInit> {
    alt((
        // Parse spread expression: ...expr
        |input| {
            let (input, _) = expect_token(Token::DotDotDot)(input)?;
            let (input, expr) = expression(input)?;
            Ok((input, FieldInit::Spread(Box::new(expr))))
        },
        // Parse regular field assignment: name: value
        |input| {
            let (input, name) = ident(input)?;
            let (input, _) = expect_token(Token::Colon)(input)?;
            let (input, value) = expression(input)?;
            Ok((
                input,
                FieldInit::Field {
                    name,
                    value: Box::new(value),
                },
            ))
        },
    ))(input)
}

fn record_lit(input: &str) -> ParseResult<'_, RecordLit> {
    alt((
        // Try to parse named record literal first: TypeName { ... }
        |input| {
            let (input, name) = ident(input)?;
            let (input, _) = expect_token(Token::LBrace)(input)?;
            let (input, fields) = separated_list0(expect_token(Token::Comma), field_init)(input)?;
            let (input, _) = expect_token(Token::RBrace)(input)?;
            Ok((input, RecordLit { name, fields }))
        },
        // Parse anonymous record literal: { ... }
        |input| {
            let (input, _) = expect_token(Token::LBrace)(input)?;
            let (input, fields) = separated_list0(expect_token(Token::Comma), field_init)(input)?;
            let (input, _) = expect_token(Token::RBrace)(input)?;
            Ok((
                input,
                RecordLit {
                    name: String::new(),
                    fields,
                },
            ))
        },
    ))(input)
}

#[allow(dead_code)]
fn unary_expr(input: &str) -> ParseResult<'_, Expr> {
    alt((
        |input| {
            let (input, _) = expect_token(Token::Minus)(input)?;
            let (input, expr) = unary_expr(input)?;
            Ok((
                input,
                Expr::new(ExprKind::Unary(UnaryExpr {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                })),
            ))
        },
        |input| {
            let (input, _) = expect_token(Token::Not)(input)?;
            let (input, expr) = unary_expr(input)?;
            Ok((
                input,
                Expr::new(ExprKind::Unary(UnaryExpr {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                })),
            ))
        },
        postfix_expr,
    ))(input)
}

fn atom_expr(input: &str) -> ParseResult<'_, Expr> {
    alt((
        literal,
        unit_expr,
        lambda_expr, // Try lambda before other expressions that use |
        stale_option_value_expr,
        list_lit, // Try list literal before record
        map(record_lit, |r| Expr::new(ExprKind::RecordLit(r))), // Try record_lit before ident
        variant_ref_expr, // Try qualified variants before identifiers
        map(ident, |i| Expr::new(ExprKind::Ident(i))),
        delimited(
            expect_token(Token::LParen),
            expression,
            expect_token(Token::RParen),
        ),
        with_expr,
        map(block_expr, |b| Expr::new(ExprKind::Block(b))),
    ))(input)
}

fn variant_path(input: &str) -> ParseResult<'_, VariantPath> {
    let (input, enum_name) = ident(input)?;
    let (input, _) = expect_token(Token::ColonColon)(input)?;
    let (input, variant_name) = alt((
        ident,
        value("Some".to_string(), expect_token(Token::Some)),
        value("None".to_string(), expect_token(Token::None)),
    ))(input)?;
    let path = VariantPath {
        enum_name,
        variant_name,
    };
    if path.builtin().is_some() && expect_token::<'_>(Token::Lt)(input).is_ok() {
        return user_syntax_failure(BUILTIN_CONSTRUCTOR_TYPE_ARGUMENT_ERROR);
    }
    Ok((input, path))
}

fn variant_ref_expr(input: &str) -> ParseResult<'_, Expr> {
    map(variant_path, |path| Expr::new(ExprKind::VariantRef(path)))(input)
}

fn unit_expr(input: &str) -> ParseResult<'_, Expr> {
    let (input, _) = expect_token(Token::LParen)(input)?;
    let (input, _) = expect_token(Token::RParen)(input)?;
    Ok((input, Expr::new(ExprKind::Unit)))
}

fn stale_option_value_expr(input: &str) -> ParseResult<'_, Expr> {
    match lex_token(input) {
        Ok((_, Token::Some | Token::None)) => user_syntax_failure(STALE_OPTION_VALUE_ERROR),
        _ => Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        ))),
    }
}

fn list_lit(input: &str) -> ParseResult<'_, Expr> {
    let (input, _) = expect_token(Token::LBracket)(input)?;

    if let Ok((input, _)) = expect_token::<'_>(Token::RBracket)(input) {
        return Ok((input, Expr::new(ExprKind::ListLit(Vec::new()))));
    }

    let (input, first) = expression(input)?;
    if let Ok((input, _)) = expect_token::<'_>(Token::DotDot)(input) {
        let (input, end) = expression(input)?;
        let (input, _) = expect_token(Token::RBracket)(input)?;
        return Ok((
            input,
            Expr::new(ExprKind::RangeLit(RangeLit {
                start: Box::new(first),
                end: Box::new(end),
            })),
        ));
    }

    let (input, rest) = many0(preceded(
        expect_token(Token::Comma),
        map(expression, Box::new),
    ))(input)?;
    let (input, _) = expect_token(Token::RBracket)(input)?;
    let mut elements = vec![Box::new(first)];
    elements.extend(rest);
    Ok((input, Expr::new(ExprKind::ListLit(elements))))
}

fn lambda_expr(input: &str) -> ParseResult<'_, Expr> {
    if let Ok((input, _)) = expect_token::<'_>(Token::Or)(input) {
        let (input, body) = expression(input)?;
        return Ok((
            input,
            Expr::new(ExprKind::Lambda(LambdaExpr {
                params: Vec::new(),
                body: Box::new(body),
                implicit_focus: false,
            })),
        ));
    }

    let (input, _) = expect_token(Token::Bar)(input)?;
    let (input, params) = separated_list0(expect_token(Token::Comma), lambda_param)(input)?;
    let (input, _) = expect_token(Token::Bar)(input)?;
    let (input, body) = expression(input)?;
    Ok((
        input,
        Expr::new(ExprKind::Lambda(LambdaExpr {
            params,
            body: Box::new(body),
            implicit_focus: false,
        })),
    ))
}

fn lambda_param(input: &str) -> ParseResult<'_, LambdaParam> {
    let (input, name) = ident(input)?;
    let (input, type_annotation) = opt(preceded(expect_token(Token::Colon), parse_type))(input)?;

    Ok((
        input,
        LambdaParam {
            name,
            type_annotation,
        },
    ))
}

/// Parse the scope attached to a scoped OSV verb clause.
///
/// An implicit scope introduces the contextual unary focus binding `it`:
///
/// ```restrict
/// values map { it + 1 }
/// ```
///
/// An explicit scope reuses Restrict's existing lambda binders as its header:
///
/// ```restrict
/// values map { |value| value + 1 }
/// (values, 0) fold { |total, value| total + value }
/// ```
///
/// Both forms elaborate to an ordinary lambda whose body is an ordinary block.
fn scoped_lambda_block(input: &str) -> ParseResult<'_, Expr> {
    let (input, _) = expect_token(Token::LBrace)(input)?;

    let (input, params, implicit_focus) =
        if let Ok((input, _)) = expect_token::<'_>(Token::Or)(input) {
            (input, Vec::new(), false)
        } else if let Ok((input, _)) = expect_token::<'_>(Token::Bar)(input) {
            let (input, params) = separated_list1(expect_token(Token::Comma), lambda_param)(input)?;
            let (input, _) = expect_token(Token::Bar)(input)?;
            (input, params, false)
        } else {
            (
                input,
                vec![LambdaParam {
                    name: "it".to_string(),
                    type_annotation: None,
                }],
                true,
            )
        };

    let (input, body) = block_expr_body(input)?;
    Ok((
        input,
        Expr::new(ExprKind::Lambda(LambdaExpr {
            params,
            body: Box::new(Expr::new(ExprKind::Block(body))),
            implicit_focus,
        })),
    ))
}

fn with_expr(input: &str) -> ParseResult<'_, Expr> {
    let (input, _) = expect_token(Token::With)(input)?;

    // Check if this is a lifetime expression
    if let Ok((_remaining, _)) = expect_token(Token::Lifetime)(input) {
        return with_lifetime_expr(input);
    }

    // Parse context name
    let (input, context_name) = ident(input)?;

    // Prefer `with Context { bindings } { body }` when a second block follows.
    if let Ok((after_bindings, bindings)) = context_bindings_block(input) {
        if let Ok((after_body, body)) = block_expr(after_bindings) {
            return Ok((
                after_body,
                Expr::new(ExprKind::With(WithExpr {
                    context_name,
                    bindings,
                    body,
                })),
            ));
        }
    }

    // Fall back to the existing `with Context { body }` form. If another block
    // follows, the first block was intended as a binding block and must parse as
    // colon-delimited bindings rather than a regular body block.
    let (input, body) = block_expr(input)?;
    if let Ok((_, Token::LBrace)) = lex_token(input) {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )));
    }

    Ok((
        input,
        Expr::new(ExprKind::With(WithExpr {
            context_name,
            bindings: Vec::new(),
            body,
        })),
    ))
}

fn context_bindings_block(input: &str) -> ParseResult<'_, Vec<FieldInit>> {
    let (input, _) = expect_token(Token::LBrace)(input)?;
    let (input, bindings) = separated_list0(expect_token(Token::Comma), field_init)(input)?;
    let (input, _) = expect_token(Token::RBrace)(input)?;
    Ok((input, bindings))
}

/// Parses a with lifetime expression.
///
/// # Examples
///
/// ```text
/// with lifetime<~f> { ... }
/// with lifetime { ... }  // anonymous lifetime
/// ```
fn with_lifetime_expr(input: &str) -> ParseResult<'_, Expr> {
    let (input, _) = expect_token(Token::Lifetime)(input)?;

    // Parse optional lifetime parameter
    let (input, lifetime_opt) = opt(delimited(
        expect_token(Token::Lt),
        preceded(expect_token(Token::Tilde), ident),
        expect_token(Token::Gt),
    ))(input)?;

    let (lifetime, anonymous) = match lifetime_opt {
        Some(name) => (name, false),
        None => {
            // For now, use a simple placeholder. In practice, this would be
            // handled by the type checker's lifetime inference
            ("_anon".to_string(), true)
        }
    };

    // Parse optional where clause with temporal constraints
    let (input, constraints) = opt(preceded(
        expect_token(Token::Where),
        separated_list1(expect_token(Token::Comma), temporal_constraint),
    ))(input)?;

    let constraints = constraints.unwrap_or_default();

    let (input, body) = block_expr(input)?;

    Ok((
        input,
        Expr::new(ExprKind::WithLifetime(WithLifetimeExpr {
            lifetime,
            anonymous,
            constraints,
            body,
        })),
    ))
}

fn pattern(input: &str) -> ParseResult<'_, Pattern> {
    alt((
        // Check for wildcard pattern
        |input| {
            let (input, token) = lex_token(input)?;
            match token {
                Token::Ident(s) if s == "_" => Ok((input, Pattern::Wildcard)),
                _ => Err(nom::Err::Error(nom::error::Error::new(
                    input,
                    nom::error::ErrorKind::Tag,
                ))),
            }
        },
        some_pattern,
        none_pattern,
        ok_pattern,
        err_pattern,
        enum_variant_pattern,
        record_pattern, // Try record patterns before identifiers
        list_pattern,   // Try list patterns before literals
        unit_pattern,
        map(literal, |expr| match expr.kind {
            ExprKind::IntLit(n) => Pattern::Literal(Literal::Int(n)),
            ExprKind::FloatLit(f) => Pattern::Literal(Literal::Float(f)),
            ExprKind::StringLit(s) => Pattern::Literal(Literal::String(s)),
            ExprKind::CharLit(c) => Pattern::Literal(Literal::Char(c)),
            ExprKind::BoolLit(b) => Pattern::Literal(Literal::Bool(b)),
            ExprKind::Unit => Pattern::Literal(Literal::Unit),
            _ => unreachable!(),
        }),
        map(ident, Pattern::Ident),
    ))(input)
}

fn enum_variant_pattern(input: &str) -> ParseResult<'_, Pattern> {
    let (input, path) = variant_path(input)?;
    let (input, payload) = opt(delimited(
        expect_token(Token::LParen),
        map(pattern, Box::new),
        expect_token(Token::RParen),
    ))(input)?;
    Ok((
        input,
        Pattern::EnumVariant {
            enum_name: path.enum_name,
            variant_name: path.variant_name,
            payload,
        },
    ))
}

fn unit_pattern(input: &str) -> ParseResult<'_, Pattern> {
    let (input, _) = expect_token(Token::LParen)(input)?;
    let (input, _) = expect_token(Token::RParen)(input)?;
    Ok((input, Pattern::Literal(Literal::Unit)))
}

fn some_pattern(input: &str) -> ParseResult<'_, Pattern> {
    let (input, _) = expect_token(Token::Some)(input)?;
    let (input, _) = expect_token(Token::LParen)(input)?;
    let (input, pattern) = pattern(input)?;
    let (input, _) = expect_token(Token::RParen)(input)?;
    Ok((input, Pattern::Some(Box::new(pattern))))
}

fn none_pattern(input: &str) -> ParseResult<'_, Pattern> {
    let (input, _) = expect_token(Token::None)(input)?;
    Ok((input, Pattern::None))
}

fn result_constructor_pattern<'a>(
    input: &'a str,
    expected: &'static str,
) -> ParseResult<'a, Pattern> {
    let (input, name) = ident(input)?;
    if name != expected {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )));
    }

    let (input, _) = expect_token(Token::LParen)(input)?;
    let (input, pattern) = pattern(input)?;
    let (input, _) = expect_token(Token::RParen)(input)?;

    match expected {
        "Ok" => Ok((input, Pattern::Ok(Box::new(pattern)))),
        "Err" => Ok((input, Pattern::Err(Box::new(pattern)))),
        _ => unreachable!(),
    }
}

fn ok_pattern(input: &str) -> ParseResult<'_, Pattern> {
    result_constructor_pattern(input, "Ok")
}

fn err_pattern(input: &str) -> ParseResult<'_, Pattern> {
    result_constructor_pattern(input, "Err")
}

fn record_pattern(input: &str) -> ParseResult<'_, Pattern> {
    // Try to parse an identifier followed by {
    let (input, name) = ident(input)?;
    let (input, _) = expect_token(Token::LBrace)(input)?;

    // Parse fields and check for spread
    let mut fields = Vec::new();
    let mut rest = None;
    let mut input = input;

    loop {
        // Check for closing brace
        if let Ok((new_input, _)) = expect_token::<'_>(Token::RBrace)(input) {
            input = new_input;
            break;
        }

        // Check for spread pattern ...rest
        if let Ok((new_input, _)) = expect_token::<'_>(Token::DotDotDot)(input) {
            let (new_input, rest_name) = ident(new_input)?;
            rest = Some(rest_name);
            input = new_input;

            // After spread, only closing brace is allowed
            let (new_input, _) = expect_token(Token::RBrace)(input)?;
            input = new_input;
            break;
        }

        // Parse regular field
        let (new_input, field_name) = ident(input)?;

        // Check if there's a colon for an explicit pattern
        let (new_input, field_pattern) =
            if let Ok((new_input, _)) = expect_token::<'_>(Token::Colon)(new_input) {
                let (new_input, pat) = pattern(new_input)?;
                (new_input, pat)
            } else {
                // Shorthand: just field name binds to a variable
                (new_input, Pattern::Ident(field_name.clone()))
            };

        fields.push((field_name, field_pattern));
        input = new_input;

        // Check for comma
        if let Ok((new_input, _)) = expect_token::<'_>(Token::Comma)(input) {
            input = new_input;
        } else {
            // No comma, expect closing brace
            let (new_input, _) = expect_token(Token::RBrace)(input)?;
            input = new_input;
            break;
        }
    }

    // Return appropriate pattern type based on whether we have spread
    if rest.is_some() || fields.iter().any(|(_, p)| !matches!(p, Pattern::Ident(_))) {
        Ok((
            input,
            Pattern::RecordDestruct {
                type_name: name,
                fields,
                rest,
            },
        ))
    } else {
        // Use simpler Record pattern if no spread and all fields are simple bindings
        Ok((input, Pattern::Record(name, fields)))
    }
}

fn list_pattern(input: &str) -> ParseResult<'_, Pattern> {
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
    let (input, mut rest) =
        many0(preceded(expect_token(Token::Comma), map(pattern, Box::new)))(input)?;
    patterns.append(&mut rest);

    let (input, _) = expect_token(Token::RBracket)(input)?;
    Ok((input, Pattern::ListExact(patterns)))
}

fn match_arm(input: &str) -> ParseResult<'_, MatchArm> {
    let (input, pattern) = pattern(input)?;
    let (input, _) = expect_token(Token::Arrow)(input)?;
    let (input, body) = block_expr(input)?;
    Ok((input, MatchArm { pattern, body }))
}

#[allow(dead_code)]
fn match_expr(input: &str) -> ParseResult<'_, Expr> {
    match_expr_with_context(input, false)
}

fn match_expr_with_context(input: &str, in_statement: bool) -> ParseResult<'_, Expr> {
    let (input, expr) = pipe_expr_with_context(input, in_statement)?;
    let (input, arms) = opt(preceded(
        expect_token(Token::Match),
        delimited(
            expect_token(Token::LBrace),
            many1(match_arm),
            expect_token(Token::RBrace),
        ),
    ))(input)?;

    match arms {
        Some(arms) => Ok((
            input,
            Expr::new(ExprKind::Match(MatchExpr {
                expr: Box::new(expr),
                arms,
            })),
        )),
        None => Ok((input, expr)),
    }
}

#[allow(dead_code)]
fn while_expr(input: &str) -> ParseResult<'_, Expr> {
    while_expr_with_context(input, false)
}

fn while_expr_with_context(input: &str, in_statement: bool) -> ParseResult<'_, Expr> {
    let (input, expr) = match_expr_with_context(input, in_statement)?;
    let (input, body) = opt(preceded(expect_token(Token::While), block_expr))(input)?;

    match body {
        Some(body) => Ok((
            input,
            Expr::new(ExprKind::While(WhileExpr {
                condition: Box::new(expr),
                body,
            })),
        )),
        None => Ok((input, expr)),
    }
}

fn then_expr(input: &str) -> ParseResult<'_, Expr> {
    then_expr_with_context(input, false)
}

fn then_expr_with_context(input: &str, in_statement: bool) -> ParseResult<'_, Expr> {
    let (input, first_cond) = while_expr_with_context(input, in_statement)?;
    let (input, then_part) = opt(preceded(
        expect_token(Token::Then),
        tuple((
            block_expr,
            many0(tuple((
                expect_token(Token::Else),
                |i| while_expr_with_context(i, in_statement),
                expect_token(Token::Then),
                block_expr,
            ))),
            opt(preceded(expect_token(Token::Else), block_expr)),
        )),
    ))(input)?;

    match then_part {
        Some((then_block, else_ifs, else_block)) => {
            let else_ifs = else_ifs
                .into_iter()
                .map(|(_, cond, _, block)| (Box::new(cond), block))
                .collect();
            Ok((
                input,
                Expr::new(ExprKind::Then(ThenExpr {
                    condition: Box::new(first_cond),
                    then_block,
                    else_ifs,
                    else_block,
                })),
            ))
        }
        None => Ok((input, first_cond)),
    }
}

fn binary_op(input: &str) -> ParseResult<'_, BinaryOp> {
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
        Token::And => Ok((input, BinaryOp::And)),
        Token::Or => Ok((input, BinaryOp::Or)),
        _ => Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        ))),
    }
}

fn binary_precedence(op: &BinaryOp) -> u8 {
    match op {
        BinaryOp::Or => 1,
        BinaryOp::And => 2,
        BinaryOp::Eq | BinaryOp::Ne => 3,
        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => 4,
        BinaryOp::Add | BinaryOp::Sub => 5,
        BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => 6,
    }
}

fn pipe_op(input: &str) -> ParseResult<'_, PipeOp> {
    let (input, token) = lex_token(input)?;
    match token {
        Token::Pipe => Ok((input, PipeOp::Pipe)),
        Token::Bar => Ok((input, PipeOp::Bar)),
        _ => Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        ))),
    }
}

fn starts_infix_or_pipe(tok: &Token) -> bool {
    matches!(
        tok,
        Token::Plus
            | Token::Minus
            | Token::Star
            | Token::Slash
            | Token::Percent
            | Token::Eq
            | Token::Ne
            | Token::Lt
            | Token::Le
            | Token::Gt
            | Token::Ge
            | Token::And
            | Token::Or
            | Token::Pipe
            | Token::Bar
    )
}

#[allow(dead_code)]
fn binary_expr(input: &str) -> ParseResult<'_, Expr> {
    binary_expr_with_context(input, false)
}

fn binary_expr_with_context(input: &str, in_statement: bool) -> ParseResult<'_, Expr> {
    binary_expr_min_precedence(input, in_statement, 1)
}

fn binary_expr_min_precedence(
    input: &str,
    in_statement: bool,
    min_precedence: u8,
) -> ParseResult<'_, Expr> {
    let (mut input, mut left) = call_expr_with_context(input, in_statement)?;

    while let Ok((after_op, op)) = binary_op(input) {
        let precedence = binary_precedence(&op);
        if precedence < min_precedence {
            break;
        }

        let (after_right, right) =
            binary_expr_min_precedence(after_op, in_statement, precedence + 1)?;
        left = Expr::new(ExprKind::Binary(BinaryExpr {
            left: Box::new(left),
            op,
            right: Box::new(right),
        }));
        input = after_right;
    }

    Ok((input, left))
}

#[allow(dead_code)]
fn pipe_expr(input: &str) -> ParseResult<'_, Expr> {
    pipe_expr_with_context(input, false)
}

fn pipe_expr_with_context(input: &str, in_statement: bool) -> ParseResult<'_, Expr> {
    let (input, first) = binary_expr_with_context(input, in_statement)?;
    let (input, pipes) = many0(tuple((
        pipe_op,
        alt((
            map(variant_ref_expr, |expr| PipeTarget::Expr(Box::new(expr))),
            map(ident, PipeTarget::Ident),
            map(
                |i| binary_expr_with_context(i, in_statement),
                |e| PipeTarget::Expr(Box::new(e)),
            ),
        )),
    )))(input)?;

    let expr = pipes.into_iter().fold(first, |acc, (op, target)| {
        Expr::new(ExprKind::Pipe(PipeExpr {
            expr: Box::new(acc),
            op,
            target,
        }))
    });
    Ok((input, expr))
}

#[allow(dead_code)]
fn call_expr(input: &str) -> ParseResult<'_, Expr> {
    call_expr_with_context(input, false)
}

fn is_scoped_verb_target(expr: &Expr) -> bool {
    matches!(&expr.kind, ExprKind::Ident(_) | ExprKind::FieldAccess(_, _))
}

/// Attach one or more verb-controlled scopes to a completed OSV call.
///
/// The result of each complete clause becomes the object of the next clause,
/// so `values map { ... } filter { ... }` is left associative.
fn scoped_clause_suffixes<'a>(mut input: &'a str, mut expr: Expr) -> ParseResult<'a, Expr> {
    if !matches!(&expr.kind, ExprKind::Call(_))
        || !matches!(lex_token(input), Ok((_, Token::LBrace)))
    {
        return Ok((input, expr));
    }

    let (after_scope, scope) = scoped_lambda_block(input)?;
    let ExprKind::Call(mut call) = expr.kind else {
        unreachable!("scoped clause suffix requires a completed call")
    };
    call.args.push(Box::new(scope));
    expr = Expr::new(ExprKind::Call(call));
    input = after_scope;

    while let Ok((after_target, target)) = simple_expr(input) {
        if !is_scoped_verb_target(&target)
            || !matches!(lex_token(after_target), Ok((_, Token::LBrace)))
        {
            break;
        }

        let (after_scope, scope) = scoped_lambda_block(after_target)?;
        expr = Expr::new(ExprKind::Call(CallExpr {
            function: Box::new(target),
            args: vec![Box::new(expr), Box::new(scope)],
        }));
        input = after_scope;
    }

    Ok((input, expr))
}

fn call_expr_with_context(input: &str, in_statement: bool) -> ParseResult<'_, Expr> {
    alt((
        // Multiple arguments with parentheses: (a,b,c) func - OSV syntax
        |input| {
            let (input, args) = delimited(
                expect_token(Token::LParen),
                separated_list0(expect_token(Token::Comma), expression),
                expect_token(Token::RParen),
            )(input)?;

            if let Ok((_, tok)) = lex_token(input) {
                if starts_infix_or_pipe(&tok) || matches!(tok, Token::Not) {
                    return Err(nom::Err::Error(nom::error::Error::new(
                        input,
                        nom::error::ErrorKind::Tag,
                    )));
                }
            }

            let (input, func) = simple_expr(input)?;
            scoped_clause_suffixes(
                input,
                Expr::new(ExprKind::Call(CallExpr {
                    function: Box::new(func),
                    args: args.into_iter().map(Box::new).collect(),
                })),
            )
        },
        // Single expression or OSV style
        move |input| {
            let (input, first) = simple_expr(input)?;

            // CRITICAL: Reject traditional function call syntax
            // The Restrict Language enforces OSV (Object-Subject-Verb) word order.
            // Traditional syntax like func(args) or obj.method(args) is FORBIDDEN.
            // Only OSV syntax is allowed: (args) func, args func, args |> func
            // Check for a following parenthesized argument list after optional
            // whitespace. Traditional calls like `func(args)`, `func (args)`,
            // and `obj.method (args)` are rejected; calls must use OSV order.
            match &first.kind {
                ExprKind::Ident(_) | ExprKind::FieldAccess(_, _) | ExprKind::VariantRef(_)
                    if input.trim_start().starts_with('(') =>
                {
                    if matches!(&first.kind, ExprKind::Ident(name) if name == "Ok" || name == "Err")
                    {
                        return user_syntax_failure(STALE_RESULT_VALUE_ERROR);
                    }
                    // This is traditional syntax like func() or obj.method() - REJECT IT
                    return user_syntax_failure(TRADITIONAL_CALL_ERROR);
                }
                _ => {}
            }

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
                }
                // Check for closing brace
                if let Ok((_, Token::RBrace)) = lex_token(input) {
                    return Ok((input, first));
                }
                // Don't consume more expressions if we see a binary operator coming
                if let Ok((_, tok)) = lex_token(input) {
                    if starts_infix_or_pipe(&tok) {
                        return Ok((input, first));
                    }
                }

                // CRITICAL FIX: In statement context, don't consume lone identifiers
                // as they might be standalone variable references or start of new statements
                if let Ok((_, Token::Ident(_))) = lex_token(input) {
                    // Look ahead after the identifier to see if it's followed by something
                    // that would indicate it's part of a function call
                    let (after_ident, _) = lex_token(input)?;
                    if !matches!(
                        lex_token(after_ident),
                        Ok((
                            _,
                            Token::IntLit(_)
                                | Token::FloatLit(_)
                                | Token::StringLit(_)
                                | Token::CharLit(_)
                                | Token::True
                                | Token::False
                                | Token::LParen
                                | Token::LBracket
                                | Token::LBrace
                                | Token::ColonColon
                        ))
                    ) {
                        // No token after identifier - definitely standalone
                        return Ok((input, first));
                    }
                }
            }

            // Otherwise, try to parse more expressions for OSV
            // But don't parse expressions that start with binary operators
            let (input, rest) = many0(|input| {
                // Peek at the next token
                if let Ok((_, tok)) = lex_token(input) {
                    // Don't parse if it starts with an infix operator.
                    if starts_infix_or_pipe(&tok) || matches!(tok, Token::LBrace) {
                        return Err(nom::Err::Error(nom::error::Error::new(
                            input,
                            nom::error::ErrorKind::Tag,
                        )));
                    }
                }

                let (after_expr, expr) = simple_expr(input)?;
                if in_statement {
                    if let Ok((_, Token::While)) = lex_token(after_expr) {
                        return Err(nom::Err::Error(nom::error::Error::new(
                            input,
                            nom::error::ErrorKind::Tag,
                        )));
                    }
                }

                Ok((after_expr, expr))
            })(input)?;

            if rest.is_empty() {
                Ok((input, first))
            } else {
                // OSV: obj subj.verb => subj.verb(obj)
                let result = rest.into_iter().fold(first, |arg, func| {
                    Expr::new(ExprKind::Call(CallExpr {
                        function: Box::new(func),
                        args: vec![Box::new(arg)],
                    }))
                });
                scoped_clause_suffixes(input, result)
            }
        },
    ))(input)
}

pub fn simple_expr(input: &str) -> ParseResult<'_, Expr> {
    cast_expr(input)
}

fn cast_expr(input: &str) -> ParseResult<'_, Expr> {
    let (mut input, mut expr) = unary_expr(input)?;

    while let Ok((after_as, _)) = expect_token::<'_>(Token::As)(input) {
        let (after_type, target) = parse_type(after_as)?;
        expr = Expr::new(ExprKind::Cast(CastExpr {
            expr: Box::new(expr),
            target,
        }));
        input = after_type;
    }

    Ok((input, expr))
}

fn postfix_expr(input: &str) -> ParseResult<'_, Expr> {
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
                    let (new_input, fields) =
                        separated_list0(expect_token(Token::Comma), field_init)(new_input)?;
                    let (new_input, _) = expect_token(Token::RBrace)(new_input)?;

                    let mut clone_expr = Expr::new(ExprKind::Clone(CloneExpr {
                        base: Box::new(expr),
                        updates: RecordLit {
                            name: String::new(),
                            fields,
                        },
                    }));

                    // Check if freeze follows the clone
                    if let Ok((freeze_input, _)) = expect_token::<'_>(Token::Freeze)(new_input) {
                        clone_expr = Expr::new(ExprKind::Freeze(Box::new(clone_expr)));
                        input = freeze_input;
                    } else {
                        input = new_input;
                    }

                    expr = clone_expr;
                } else {
                    // Regular field access
                    let (new_input, field) = ident(new_input)?;
                    expr = Expr::new(ExprKind::FieldAccess(Box::new(expr), field));
                    input = new_input;
                }
            }
            Some(PostfixOp::Freeze) => {
                expr = Expr::new(ExprKind::Freeze(Box::new(expr)));
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

fn expression(input: &str) -> ParseResult<'_, Expr> {
    then_expr(input)
}

fn expression_in_statement(input: &str) -> ParseResult<'_, Expr> {
    then_expr_with_context(input, true)
}

#[allow(dead_code)]
fn statement(input: &str) -> ParseResult<'_, Stmt> {
    alt((
        map(bind_decl_in_statement, Stmt::Binding),
        assignment_stmt,
        map(expression, |e| Stmt::Expr(Box::new(e))),
    ))(input)
}

fn assignment_stmt(input: &str) -> ParseResult<'_, Stmt> {
    let (input, name) = ident(input)?;
    let (input, _) = expect_token(Token::Assign)(input)?;
    let (input, value) = expression_in_statement(input)?;
    Ok((
        input,
        Stmt::Assignment(AssignStmt {
            name,
            value: Box::new(value),
        }),
    ))
}

fn import_decl(input: &str) -> ParseResult<'_, ImportDecl> {
    let (input, _) = expect_token(Token::Import)(input)?;
    if matches!(lex_token(input), Ok((_, Token::StringLit(_)))) {
        return Err(nom::Err::Failure(nom::error::Error::new(
            UNSUPPORTED_IMPORT_ALIAS_ERROR,
            nom::error::ErrorKind::Fail,
        )));
    }

    let (input, module_path) = separated_list1(expect_token(Token::Dot), ident)(input)?;

    // Check for specific imports: .{foo, bar}
    let (input, items) = if let Ok((input, _)) = expect_token(Token::Dot)(input) {
        alt((
            // import module.*
            map(expect_token(Token::Star), |_| ImportItems::All),
            // import module.{foo, bar}
            map(
                delimited(
                    expect_token(Token::LBrace),
                    separated_list0(expect_token(Token::Comma), ident),
                    expect_token(Token::RBrace),
                ),
                ImportItems::Named,
            ),
        ))(input)?
    } else {
        // import module (imports all)
        (input, ImportItems::All)
    };

    if matches!(lex_token(input), Ok((_, Token::As))) {
        return Err(nom::Err::Failure(nom::error::Error::new(
            UNSUPPORTED_IMPORT_ALIAS_ERROR,
            nom::error::ErrorKind::Fail,
        )));
    }

    Ok((input, ImportDecl { module_path, items }))
}

fn export_decl(input: &str) -> ParseResult<'_, TopDecl> {
    let (input, _) = alt((expect_token(Token::Export), expect_token(Token::Pub)))(input)?;
    if matches!(lex_token(input), Ok((_, Token::Import))) {
        return Err(nom::Err::Failure(nom::error::Error::new(
            UNSUPPORTED_RE_EXPORT_ERROR,
            nom::error::ErrorKind::Fail,
        )));
    }
    let (input, item) = top_decl_inner(input)?;
    if matches!(item, TopDecl::Takes(_)) {
        return user_syntax_failure(EXPORTED_TAKES_ERROR);
    }
    Ok((
        input,
        TopDecl::Export(ExportDecl {
            item: Box::new(item),
        }),
    ))
}

fn form_method_decl(input: &str) -> ParseResult<'_, FormMethodDecl> {
    let (input, _) = expect_token(Token::Fun)(input)?;
    let (input, name) = ident(input)?;
    let (input, _) = expect_token(Token::Colon)(input)?;

    if matches!(lex_token(input), Ok((_, Token::Lt))) {
        return user_syntax_failure(GENERIC_FORM_ERROR);
    }

    let (input, params) = delimited(
        expect_token(Token::LParen),
        separated_list0(expect_token(Token::Comma), param),
        expect_token(Token::RParen),
    )(input)?;
    let (input, _) = expect_token(Token::ThinArrow)(input)?;
    let (input, return_type) = parse_type(input)?;

    if matches!(lex_token(input), Ok((_, Token::Assign))) {
        return user_syntax_failure(DEFAULT_FORM_METHOD_ERROR);
    }

    Ok((
        input,
        FormMethodDecl {
            name,
            params,
            return_type,
        },
    ))
}

fn form_decl(input: &str) -> ParseResult<'_, FormDecl> {
    let (input, _) = expect_token(Token::Form)(input)?;
    let (input, name) = ident(input)?;

    if matches!(lex_token(input), Ok((_, Token::Lt))) {
        return user_syntax_failure(GENERIC_FORM_ERROR);
    }

    let (mut input, _) = expect_token(Token::LBrace)(input)?;
    let mut methods = Vec::new();
    loop {
        let (remaining, _) = skip(input)?;
        if let Ok((after_brace, _)) = expect_token::<'_>(Token::RBrace)(remaining) {
            return Ok((after_brace, FormDecl { name, methods }));
        }
        if starts_with_word(remaining, "type") {
            return user_syntax_failure(ASSOCIATED_FORM_TYPE_ERROR);
        }

        let (remaining, method) = form_method_decl(remaining)?;
        methods.push(method);
        let (remaining, _) = opt(expect_token(Token::Comma))(remaining)?;
        input = remaining;
    }
}

fn takes_decl(input: &str) -> ParseResult<'_, TakesDecl> {
    let (input, target) = ident(input)?;

    if matches!(lex_token(input), Ok((_, Token::Lt | Token::Tilde))) {
        return user_syntax_failure(GENERIC_TAKES_ERROR);
    }

    let (input, _) = expect_token(Token::Takes)(input)?;
    let (input, form_name) = ident(input)?;

    if matches!(lex_token(input), Ok((_, Token::Lt | Token::Tilde))) {
        return user_syntax_failure(GENERIC_TAKES_ERROR);
    }
    if matches!(lex_token(input), Ok((_, Token::Where))) {
        return user_syntax_failure(CONDITIONAL_TAKES_ERROR);
    }

    let (mut input, _) = expect_token(Token::LBrace)(input)?;
    let mut functions = Vec::new();
    loop {
        let (remaining, _) = skip(input)?;
        if let Ok((after_brace, _)) = expect_token::<'_>(Token::RBrace)(remaining) {
            return Ok((
                after_brace,
                TakesDecl {
                    target,
                    form_name,
                    functions,
                },
            ));
        }
        if starts_with_word(remaining, "type") {
            return user_syntax_failure(ASSOCIATED_FORM_TYPE_ERROR);
        }

        let (remaining, function) = fun_decl(remaining)?;
        functions.push(function);
        input = remaining;
    }
}

fn top_decl_inner(input: &str) -> ParseResult<'_, TopDecl> {
    alt((
        map(form_decl, TopDecl::Form),
        map(enum_decl, TopDecl::Enum),
        map(fun_decl, TopDecl::Function),
        map(record_decl, TopDecl::Record),
        map(impl_block, TopDecl::Impl),
        map(context_decl, TopDecl::Context),
        map(bind_decl, TopDecl::Binding),
        map(takes_decl, TopDecl::Takes),
    ))(input)
}

pub fn top_decl(input: &str) -> ParseResult<'_, TopDecl> {
    // Skip whitespace/comments before parsing a top-level declaration
    let (input, _) = skip(input)?;

    // Try export_decl first, but if it fails, make sure we have the right input
    match export_decl(input) {
        Ok(result) => Ok(result),
        Err(e @ nom::Err::Failure(_)) => Err(e),
        Err(_) => {
            // export_decl failed, try top_decl_inner with the original input
            top_decl_inner(input)
        }
    }
}

pub fn parse_program(input: &str) -> ParseResult<'_, Program> {
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

        if starts_with_let_binding(rest) {
            return user_syntax_failure(STALE_LET_ERROR);
        }
        if starts_with_if_parens(rest) {
            return user_syntax_failure(STALE_IF_ERROR);
        }
        if starts_with_val_mut(rest) {
            return user_syntax_failure(STALE_VAL_MUT_ERROR);
        }

        // Try to parse a declaration
        match top_decl(rest) {
            Ok((rest2, decl)) => {
                declarations.push(decl);
                remaining = rest2;
            }
            Err(e @ nom::Err::Failure(_)) => return Err(e),
            Err(e) => {
                // If we've made some progress but failed to parse a declaration,
                // and there's still non-whitespace content, this is a parse error
                // (e.g., traditional function call syntax in a function body)
                if !declarations.is_empty() && !rest.trim().is_empty() {
                    return Err(e);
                }

                // Special case: if we have non-whitespace content that looks like
                // it should be a declaration but fails to parse, propagate the error
                // This catches cases like functions with traditional syntax
                let trimmed = rest.trim();
                if !trimmed.is_empty() {
                    // Check if it starts with declaration keywords
                    if trimmed.starts_with("fun ")
                        || trimmed.starts_with("record ")
                        || trimmed.starts_with("enum ")
                        || trimmed.starts_with("form ")
                        || trimmed.starts_with("takes ")
                        || trimmed.starts_with("impl ")
                        || trimmed.starts_with("context ")
                        || trimmed.starts_with("val ")
                        || trimmed.starts_with("import ")
                        || trimmed.starts_with("export ")
                        || trimmed.starts_with("pub ")
                    {
                        return Err(e);
                    }
                }

                // If we haven't parsed anything yet, we can safely stop
                remaining = rest;
                break;
            }
        }
    }

    let mut program = Program {
        imports,
        declarations,
    };
    // Number expression nodes once the full program structure is known, so
    // downstream stages can key per-node facts by stable NodeId.
    assign_node_ids(&mut program);

    Ok((remaining, program))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_decl() {
        let input = "record Enemy { hp: Int32, atk: Int32 }";
        let (_, decl) = record_decl(input).unwrap();
        assert_eq!(decl.name, "Enemy");
        assert_eq!(decl.fields.len(), 2);
    }

    #[test]
    fn test_enum_decl_accepts_nullary_and_payload_variants() {
        let input = r#"enum CheckoutError {
            InvalidSku
            PaymentDeclined(String),
            RetryAfter(Result<Int32, String>),
        }"#;
        let (remaining, decl) = enum_decl(input).unwrap();

        assert!(remaining.trim().is_empty());
        assert_eq!(decl.name, "CheckoutError");
        assert_eq!(
            decl.variants,
            vec![
                EnumVariantDecl {
                    name: "InvalidSku".to_string(),
                    payload: None,
                },
                EnumVariantDecl {
                    name: "PaymentDeclined".to_string(),
                    payload: Some(Type::Named("String".to_string())),
                },
                EnumVariantDecl {
                    name: "RetryAfter".to_string(),
                    payload: Some(Type::Generic(
                        "Result".to_string(),
                        vec![
                            Type::Named("Int32".to_string()),
                            Type::Named("String".to_string()),
                        ],
                    )),
                },
            ]
        );
    }

    #[test]
    fn test_enum_decl_requires_at_least_one_variant() {
        assert!(enum_decl("enum Empty {}").is_err());
    }

    #[test]
    fn test_enum_decl_rejects_generics_and_multiple_payloads() {
        assert!(enum_decl("enum Generic<T> { Value(T) }").is_err());
        assert!(enum_decl("enum PairLike { Pair(Int32, String) }").is_err());
    }

    #[test]
    fn test_pub_enum_is_an_exported_top_decl() {
        let (remaining, program) =
            parse_program("pub enum Status { Ready, Failed(String) }").unwrap();
        assert!(remaining.trim().is_empty());

        let TopDecl::Export(export) = &program.declarations[0] else {
            panic!("expected exported enum");
        };
        let TopDecl::Enum(enum_decl) = export.item.as_ref() else {
            panic!("expected enum declaration");
        };
        assert_eq!(enum_decl.name, "Status");
        assert_eq!(enum_decl.variants.len(), 2);
    }

    #[test]
    fn test_form_takes_and_of_constraints_parse() {
        let source = r#"
            pub form Showable {
                fun show: (self: Self) -> String
            }

            form Comparable {
                fun compare: (self: Self, other: Self) -> Int32
            }

            record Widget { label: String }

            Widget takes Showable {
                fun show: (self: Widget) -> String = { self.label }
            }

            fun render: <T of Showable + Comparable>(value: T) -> String = {
                (value) show
            }
        "#;
        let (remaining, program) = parse_program(source).unwrap();
        assert!(remaining.trim().is_empty());

        let TopDecl::Export(export) = &program.declarations[0] else {
            panic!("expected public form");
        };
        let TopDecl::Form(showable) = export.item.as_ref() else {
            panic!("expected form declaration");
        };
        assert_eq!(showable.name, "Showable");
        assert_eq!(showable.methods.len(), 1);
        assert_eq!(showable.methods[0].name, "show");
        assert_eq!(showable.methods[0].params[0].name, "self");
        assert_eq!(
            showable.methods[0].return_type,
            Type::Named("String".to_string())
        );

        let TopDecl::Takes(takes) = &program.declarations[3] else {
            panic!("expected takes declaration");
        };
        assert_eq!(takes.target, "Widget");
        assert_eq!(takes.form_name, "Showable");
        assert_eq!(takes.functions.len(), 1);
        assert_eq!(takes.functions[0].name, "show");

        let TopDecl::Function(render) = &program.declarations[4] else {
            panic!("expected constrained function");
        };
        assert_eq!(
            render.type_params[0].of_forms,
            vec!["Showable".to_string(), "Comparable".to_string()]
        );
    }

    #[test]
    fn test_takes_bodies_receive_dense_node_ids() {
        let (_, program) = parse_program(
            r#"
                form Showable { fun show: (self: Self) -> String }
                record Widget { label: String }
                Widget takes Showable {
                    fun show: (self: Widget) -> String = { self.label }
                }
            "#,
        )
        .unwrap();

        let ids = collect_node_ids(&program);
        assert!(!ids.is_empty());
        assert_eq!(ids, (0..ids.len() as u32).map(NodeId).collect::<Vec<_>>());
    }

    #[test]
    fn test_initial_form_slice_rejects_deferred_features() {
        let generic_form =
            parse_program("form Showable<T> { fun show: (self: Self) -> String }").unwrap_err();
        assert!(matches!(
            generic_form,
            nom::Err::Failure(error) if error.input == GENERIC_FORM_ERROR
        ));

        let associated = parse_program("form Iterable { type Item }").unwrap_err();
        assert!(matches!(
            associated,
            nom::Err::Failure(error) if error.input == ASSOCIATED_FORM_TYPE_ERROR
        ));

        let default_method =
            parse_program("form Showable { fun show: (self: Self) -> String = { \"default\" } }")
                .unwrap_err();
        assert!(matches!(
            default_method,
            nom::Err::Failure(error) if error.input == DEFAULT_FORM_METHOD_ERROR
        ));

        let generic_takes = parse_program(
            "Widget<T> takes Showable { fun show: (self: Widget) -> String = { \"x\" } }",
        )
        .unwrap_err();
        assert!(matches!(
            generic_takes,
            nom::Err::Failure(error) if error.input == GENERIC_TAKES_ERROR
        ));

        let conditional = parse_program(
            "Widget takes Showable where T of Other { fun show: (self: Widget) -> String = { \"x\" } }",
        )
        .unwrap_err();
        assert!(matches!(
            conditional,
            nom::Err::Failure(error) if error.input == CONDITIONAL_TAKES_ERROR
        ));

        let public_takes = parse_program(
            "pub Widget takes Showable { fun show: (self: Widget) -> String = { \"x\" } }",
        )
        .unwrap_err();
        assert!(matches!(
            public_takes,
            nom::Err::Failure(error) if error.input == EXPORTED_TAKES_ERROR
        ));
    }

    #[test]
    fn test_qualified_variant_expression_ref() {
        let (remaining, expr) = simple_expr("CheckoutError::InvalidSku").unwrap();
        assert!(remaining.trim().is_empty());
        assert_eq!(
            expr.kind,
            ExprKind::VariantRef(VariantPath {
                enum_name: "CheckoutError".to_string(),
                variant_name: "InvalidSku".to_string(),
            })
        );
    }

    #[test]
    fn test_osv_variant_constructors() {
        let (remaining, nullary) = call_expr("() CheckoutError::InvalidSku").unwrap();
        assert!(remaining.trim().is_empty());
        let ExprKind::Call(nullary) = nullary.kind else {
            panic!("expected nullary OSV constructor call");
        };
        assert!(nullary.args.is_empty());
        assert!(matches!(
            nullary.function.kind,
            ExprKind::VariantRef(VariantPath { ref enum_name, ref variant_name })
                if enum_name == "CheckoutError" && variant_name == "InvalidSku"
        ));

        let (remaining, payload) =
            call_expr(r#"("declined") CheckoutError::PaymentDeclined"#).unwrap();
        assert!(remaining.trim().is_empty());
        let ExprKind::Call(payload) = payload.kind else {
            panic!("expected payload OSV constructor call");
        };
        assert_eq!(payload.args.len(), 1);
        assert!(matches!(
            payload.function.kind,
            ExprKind::VariantRef(VariantPath { ref enum_name, ref variant_name })
                if enum_name == "CheckoutError" && variant_name == "PaymentDeclined"
        ));

        let (remaining, piped) =
            pipe_expr(r#""declined" |> CheckoutError::PaymentDeclined"#).unwrap();
        assert!(remaining.trim().is_empty());
        let ExprKind::Pipe(piped) = piped.kind else {
            panic!("expected pipe constructor call");
        };
        assert!(matches!(
            piped.target,
            PipeTarget::Expr(target)
                if matches!(target.kind,
                    ExprKind::VariantRef(VariantPath { ref enum_name, ref variant_name })
                        if enum_name == "CheckoutError" && variant_name == "PaymentDeclined")
        ));
    }

    #[test]
    fn test_scoped_osv_clause_desugars_implicit_focus() {
        let (remaining, expr) = call_expr("values map { it + 1 }").unwrap();
        assert!(remaining.trim().is_empty());

        let ExprKind::Call(call) = expr.kind else {
            panic!("expected scoped map clause to elaborate to a call");
        };
        assert_eq!(call.args.len(), 2);
        assert!(matches!(call.function.kind, ExprKind::Ident(ref name) if name == "map"));
        assert!(matches!(call.args[0].kind, ExprKind::Ident(ref name) if name == "values"));

        let ExprKind::Lambda(lambda) = &call.args[1].kind else {
            panic!("expected scoped map body to elaborate to a lambda");
        };
        assert!(lambda.implicit_focus);
        assert_eq!(lambda.params.len(), 1);
        assert_eq!(lambda.params[0].name, "it");
        assert!(matches!(lambda.body.kind, ExprKind::Block(_)));
    }

    #[test]
    fn test_scoped_osv_clause_reuses_explicit_lambda_binders() {
        let source = "(values, 0) fold { |total, value| total + value }";
        let (remaining, expr) = call_expr(source).unwrap();
        assert!(remaining.trim().is_empty());

        let ExprKind::Call(call) = expr.kind else {
            panic!("expected scoped fold clause to elaborate to a call");
        };
        assert_eq!(call.args.len(), 3);
        let ExprKind::Lambda(lambda) = &call.args[2].kind else {
            panic!("expected scoped fold body to elaborate to a lambda");
        };
        assert!(!lambda.implicit_focus);
        assert_eq!(
            lambda
                .params
                .iter()
                .map(|param| param.name.as_str())
                .collect::<Vec<_>>(),
            vec!["total", "value"]
        );
        assert!(matches!(lambda.body.kind, ExprKind::Block(_)));
    }

    #[test]
    fn test_scoped_osv_clauses_chain_left_associatively() {
        let source = "values map { it + 1 } filter { it > 2 }";
        let (remaining, expr) = call_expr(source).unwrap();
        assert!(remaining.trim().is_empty());

        let ExprKind::Call(filter_call) = expr.kind else {
            panic!("expected outer filter call");
        };
        assert!(matches!(filter_call.function.kind, ExprKind::Ident(ref name) if name == "filter"));
        assert_eq!(filter_call.args.len(), 2);
        assert!(
            matches!(filter_call.args[0].kind, ExprKind::Call(ref map_call)
            if matches!(map_call.function.kind, ExprKind::Ident(ref name) if name == "map"))
        );
        assert!(matches!(filter_call.args[1].kind, ExprKind::Lambda(_)));
    }

    #[test]
    fn test_qualified_builtin_value_constructors() {
        let (remaining, direct_some) = call_expr("42 Option::Some").unwrap();
        assert!(remaining.trim().is_empty());
        let ExprKind::Call(direct_some) = direct_some.kind else {
            panic!("expected direct Option::Some call");
        };
        assert_eq!(direct_some.args.len(), 1);
        assert!(matches!(
            direct_some.function.kind,
            ExprKind::VariantRef(VariantPath { ref enum_name, ref variant_name })
                if enum_name == "Option" && variant_name == "Some"
        ));

        let (remaining, direct_none) = call_expr("() Option::None").unwrap();
        assert!(remaining.trim().is_empty());
        let ExprKind::Call(direct_none) = direct_none.kind else {
            panic!("expected direct Option::None call");
        };
        assert!(direct_none.args.is_empty());
        assert!(matches!(
            direct_none.function.kind,
            ExprKind::VariantRef(VariantPath { ref enum_name, ref variant_name })
                if enum_name == "Option" && variant_name == "None"
        ));

        let (remaining, piped_ok) = pipe_expr("42 |> Result::Ok").unwrap();
        assert!(remaining.trim().is_empty());
        let ExprKind::Pipe(piped_ok) = piped_ok.kind else {
            panic!("expected piped Result::Ok call");
        };
        assert!(matches!(
            piped_ok.target,
            PipeTarget::Expr(target)
                if matches!(target.kind,
                    ExprKind::VariantRef(VariantPath { ref enum_name, ref variant_name })
                        if enum_name == "Result" && variant_name == "Ok")
        ));

        let (remaining, direct_err) = call_expr(r#""missing" Result::Err"#).unwrap();
        assert!(remaining.trim().is_empty());
        let ExprKind::Call(direct_err) = direct_err.kind else {
            panic!("expected direct Result::Err call");
        };
        assert_eq!(direct_err.args.len(), 1);
        assert!(matches!(
            direct_err.function.kind,
            ExprKind::VariantRef(VariantPath { ref enum_name, ref variant_name })
                if enum_name == "Result" && variant_name == "Err"
        ));

        let source = r#"
            fun main: () -> Option<Int32> = {
                42 Option::Some
            }
        "#;
        let (remaining, program) = parse_program(source).unwrap();
        assert!(remaining.trim().is_empty());
        let TopDecl::Function(main) = &program.declarations[0] else {
            panic!("expected main function");
        };
        assert!(matches!(
            main.body.expr.as_deref().map(|expr| &expr.kind),
            Some(ExprKind::Call(call))
                if matches!(call.function.kind,
                    ExprKind::VariantRef(VariantPath { ref enum_name, ref variant_name })
                        if enum_name == "Option" && variant_name == "Some")
        ));
    }

    #[test]
    fn test_legacy_builtin_value_constructors_are_rejected() {
        for source in ["Some(42)", "None"] {
            let err = expression(source).unwrap_err();
            assert!(matches!(
                err,
                nom::Err::Failure(error) if error.input == STALE_OPTION_VALUE_ERROR
            ));
        }

        for source in ["Ok(42)", "Err(42)"] {
            let err = expression(source).unwrap_err();
            assert!(matches!(
                err,
                nom::Err::Failure(error) if error.input == STALE_RESULT_VALUE_ERROR
            ));
        }

        assert_eq!(
            pattern("Some(value)").unwrap().1,
            Pattern::Some(Box::new(Pattern::Ident("value".to_string())))
        );
        assert_eq!(pattern("None").unwrap().1, Pattern::None);
        assert_eq!(
            pattern("Ok(value)").unwrap().1,
            Pattern::Ok(Box::new(Pattern::Ident("value".to_string())))
        );
        assert_eq!(
            pattern("Err(error)").unwrap().1,
            Pattern::Err(Box::new(Pattern::Ident("error".to_string())))
        );
    }

    #[test]
    fn test_traditional_variant_constructor_call_is_rejected() {
        let err = expression(r#"CheckoutError::PaymentDeclined("declined")"#).unwrap_err();
        assert!(matches!(err, nom::Err::Failure(_)));
    }

    #[test]
    fn test_qualified_variant_patterns() {
        assert_eq!(
            pattern("CheckoutError::InvalidSku").unwrap().1,
            Pattern::EnumVariant {
                enum_name: "CheckoutError".to_string(),
                variant_name: "InvalidSku".to_string(),
                payload: None,
            }
        );
        assert_eq!(
            pattern("CheckoutError::PaymentDeclined(message)")
                .unwrap()
                .1,
            Pattern::EnumVariant {
                enum_name: "CheckoutError".to_string(),
                variant_name: "PaymentDeclined".to_string(),
                payload: Some(Box::new(Pattern::Ident("message".to_string()))),
            }
        );
    }

    #[test]
    fn test_fun_decl() {
        let input = "fun add: (a: Int32, b: Int32) -> Int32 = { a }";
        let (_, decl) = fun_decl(input).unwrap();
        assert_eq!(decl.name, "add");
        assert_eq!(decl.params.len(), 2);
        assert_eq!(decl.return_type, Some(Type::Named("Int32".to_string())));
    }

    #[test]
    fn test_pipe_expr() {
        let input = "42 |> add 10";
        let (_, expr) = pipe_expr(input).unwrap();
        assert!(matches!(&expr.kind, ExprKind::Pipe(_)));
    }

    #[test]
    fn test_mutable_pipe_rejected() {
        assert!(pipe_expr("42 |>> add").is_err());
    }

    #[test]
    fn test_clone_freeze() {
        let input = "base.clone { hp: 500 } freeze";
        let (_, expr) = simple_expr(input).unwrap();
        assert!(matches!(&expr.kind, ExprKind::Freeze(_)));
    }

    #[test]
    fn test_field_access() {
        let input = "obj.field";
        let (_, expr) = simple_expr(input).unwrap();
        assert!(matches!(&expr.kind, ExprKind::FieldAccess(_, _)));
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
        if let ExprKind::WithLifetime(ref wl) = expr.kind {
            assert_eq!(wl.lifetime, "f");
            assert!(!wl.anonymous);
            assert!(wl.constraints.is_empty());
        } else {
            panic!("Expected WithLifetime expression");
        }
    }
}
