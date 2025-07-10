use nom::{
    IResult,
    branch::alt,
    character::complete::{char, multispace0},
    combinator::{map, opt, value},
    multi::{many0, many1, separated_list0},
    sequence::{preceded, tuple, delimited},
};
use crate::lexer::{Token, lex_token};
use crate::ast::*;

type ParseResult<'a, T> = IResult<&'a str, T>;

fn expect_token<'a>(expected: Token) -> impl Fn(&'a str) -> ParseResult<'a, ()> {
    move |input| {
        let (input, token) = lex_token(input)?;
        if token == expected {
            Ok((input, ()))
        } else {
            Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
        }
    }
}

fn ident(input: &str) -> ParseResult<String> {
    let (input, token) = lex_token(input)?;
    match token {
        Token::Ident(name) => Ok((input, name)),
        _ => Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
    }
}

fn parse_type(input: &str) -> ParseResult<Type> {
    let (input, name) = ident(input)?;
    let (input, generics) = opt(
        delimited(
            expect_token(Token::Lt),
            separated_list0(
                expect_token(Token::Comma),
                parse_type
            ),
            expect_token(Token::Gt)
        )
    )(input)?;
    
    match generics {
        Some(params) => Ok((input, Type::Generic(name, params))),
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
    let (input, _) = expect_token(Token::LBrace)(input)?;
    let (input, fields) = many0(field_decl)(input)?;
    let (input, _) = expect_token(Token::RBrace)(input)?;
    Ok((input, RecordDecl { name, fields }))
}

fn param(input: &str) -> ParseResult<Param> {
    let (input, context_bound) = opt(
        preceded(
            char('@'),
            ident
        )
    )(input)?;
    let (input, name) = ident(input)?;
    let (input, _) = expect_token(Token::Colon)(input)?;
    let (input, ty) = parse_type(input)?;
    Ok((input, Param { name, ty, context_bound }))
}

fn block_expr(input: &str) -> ParseResult<BlockExpr> {
    let (input, _) = expect_token(Token::LBrace)(input)?;
    
    // Parse statements and expressions carefully
    let mut statements = Vec::new();
    let mut remaining = input;
    let mut final_expr = None;
    
    loop {
        // Skip whitespace
        let (next_input, _) = multispace0(remaining)?;
        remaining = next_input;
        
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
        let (next_input2, _) = multispace0(after_expr)?;
        if let Ok((_, _)) = expect_token::<'_>(Token::RBrace)(next_input2) {
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
    
    Ok((remaining, BlockExpr { 
        statements, 
        expr: final_expr 
    }))
}

fn fun_decl(input: &str) -> ParseResult<FunDecl> {
    let (input, _) = expect_token(Token::Fun)(input)?;
    let (input, name) = ident(input)?;
    let (input, _) = expect_token(Token::Assign)(input)?;
    let (input, params) = many0(param)(input)?;  // Changed from many1 to many0
    let (input, body) = block_expr(input)?;
    Ok((input, FunDecl { name, params, body }))
}

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
    let (input, value) = expression(input)?;  // Use normal expression parsing
    Ok((input, BindDecl { 
        mutable: mutable.is_some(), 
        name, 
        value: Box::new(value) 
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

fn atom_expr(input: &str) -> ParseResult<Expr> {
    alt((
        literal,
        map(record_lit, Expr::RecordLit),  // Try record_lit before ident
        map(ident, Expr::Ident),
        delimited(
            expect_token(Token::LParen),
            expression,
            expect_token(Token::RParen)
        ),
        with_expr,
        map(block_expr, Expr::Block)
    ))(input)
}

fn with_expr(input: &str) -> ParseResult<Expr> {
    let (input, _) = expect_token(Token::With)(input)?;
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

fn pattern(input: &str) -> ParseResult<Pattern> {
    alt((
        value(Pattern::Wildcard, char('_')),
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
                // Check if the next token might start a new statement
                let (after_ws, _) = multispace0(input)?;
                
                // Peek at the next tokens to see if this is a new statement
                if let Ok((_, Token::Val)) = lex_token(after_ws) {
                    return Ok((input, first));
                }
                if let Ok((_, Token::Mut)) = lex_token(after_ws) {
                    return Ok((input, first));
                }
                // Check for assignment pattern: ident =
                if let Ok((after_ident, Token::Ident(_))) = lex_token(after_ws) {
                    let (after_ws2, _) = multispace0(after_ident)?;
                    if let Ok((_, Token::Assign)) = lex_token(after_ws2) {
                        return Ok((input, first));
                    }
                }
            }
            
            // Otherwise, try to parse more expressions for OSV
            let (input, rest) = many0(simple_expr)(input)?;
            
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
    
    // Handle field access and postfix operations
    loop {
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
                    
                    expr = Expr::Clone(CloneExpr {
                        base: Box::new(expr),
                        updates: RecordLit {
                            name: String::new(),
                            fields
                        }
                    });
                    input = new_input;
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
    let (input, value) = expression(input)?;  // Use normal expression parsing
    Ok((input, Stmt::Assignment(AssignStmt {
        name,
        value: Box::new(value)
    })))
}

pub fn top_decl(input: &str) -> ParseResult<TopDecl> {
    alt((
        map(record_decl, TopDecl::Record),
        map(impl_block, TopDecl::Impl),
        map(context_decl, TopDecl::Context),
        map(fun_decl, TopDecl::Function),
        map(bind_decl, TopDecl::Binding)
    ))(input)
}

pub fn parse_program(input: &str) -> ParseResult<Program> {
    let (input, _) = multispace0(input)?; // Skip initial whitespace
    let (input, declarations) = many0(preceded(multispace0, top_decl))(input)?;
    let (input, _) = multispace0(input)?; // Skip trailing whitespace
    
    // If we have remaining input but no declarations parsed, try to give a helpful error
    if !input.is_empty() && declarations.is_empty() {
        // Try to parse a single top_decl to get a better error message
        if let Err(e) = top_decl(input) {
            return Err(e);
        }
    }
    
    Ok((input, Program { declarations }))
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
        let input = "fun add = a:Int b:Int { a }";
        let (_, decl) = fun_decl(input).unwrap();
        assert_eq!(decl.name, "add");
        assert_eq!(decl.params.len(), 2);
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
}