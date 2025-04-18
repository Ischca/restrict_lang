use nom::{
    branch::alt,
    bytes::complete::{escaped_transform, tag, take_until},
    character::complete::{alpha1, alphanumeric1, char, multispace0, multispace1, none_of, one_of},
    combinator::{all_consuming, map, opt, recognize, value},
    multi::{fold_many0, many0, many1, separated_list0},
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult,
};
use std::collections::HashMap;

// ASTの定義
#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    Number(i64),
    Identifier(String),
    FunctionCall(Box<Expr>, Vec<Expr>),
    FunctionDef(String, Vec<String>, Box<Expr>),
    Let(String, Box<Expr>),
    Block(Vec<Expr>),
    If(Box<Expr>, Box<Expr>, Option<Box<Expr>>),
    Pipe(Box<Expr>, Box<Expr>),
    String(String),
    Boolean(bool),
}

#[derive(Clone, Debug)]
pub enum Value {
    Number(i64),
    String(String),
    Boolean(bool),
    Function(Vec<String>, Box<Expr>, Env),
    Unit,
}

type Env = HashMap<String, Value>;

// パーサーの実装

fn parse_number(input: &str) -> IResult<&str, Expr> {
    map(
        recognize(many1(terminated(one_of("0123456789"), many0(char('_'))))),
        |s: &str| Expr::Number(s.replace("_", "").parse().unwrap()),
    )(input)
}

fn parse_identifier(input: &str) -> IResult<&str, Expr> {
    map(
        recognize(pair(
            alt((alpha1, tag("_"))),
            many0(alt((alphanumeric1, tag("_")))),
        )),
        |s: &str| Expr::Identifier(s.to_string()),
    )(input)
}

fn parse_boolean(input: &str) -> IResult<&str, Expr> {
    alt((
        map(tag("true"), |_| Expr::Boolean(true)),
        map(tag("false"), |_| Expr::Boolean(false)),
    ))(input)
}

fn parse_string(input: &str) -> IResult<&str, Expr> {
    let (input, content) = delimited(
        char('"'),
        escaped_transform(
            none_of("\\\""),
            '\\',
            alt((
                value("\\", char('\\')),
                value("\"", char('"')),
                value("\n", char('n')),
                value("\r", char('r')),
                value("\t", char('t')),
            )),
        ),
        char('"'),
    )(input)?;
    Ok((input, Expr::String(content)))
}

fn parse_atom(input: &str) -> IResult<&str, Expr> {
    preceded(
        ws,
        alt((
            parse_number,
            parse_string,
            parse_boolean, // 追加
            parse_identifier,
            parse_paren_expr,
            parse_block,
        )),
    )(input)
}

fn parse_postfix_expr(input: &str) -> IResult<&str, Expr> {
    let (mut input, mut left) = parse_atom(input)?;
    loop {
        let (new_input, opt_right) = opt(preceded(
            multispace1,
            parse_atom, // 関数名を後置
        ))(input)?;
        match opt_right {
            Some(right) => {
                // 関数呼び出しの順序を修正：関数が right、引数が left
                left = Expr::FunctionCall(Box::new(right), vec![left]);
                input = new_input;
            }
            None => break Ok((input, left)),
        }
    }
}

fn parse_operator(input: &str) -> IResult<&str, &str> {
    let operators = alt((
        tag("=="),
        tag("!="),
        tag(">="),
        tag("<="),
        tag(">"),
        tag("<"),
        tag("+"),
        tag("-"),
        tag("*"),
        tag("/"),
        tag("%"),
    ));
    delimited(ws, operators, ws)(input)
}

fn parse_infix_expr(input: &str) -> IResult<&str, Expr> {
    let (input, left) = parse_postfix_expr(input)?;
    let x = fold_many0(
        pair(parse_operator, parse_postfix_expr),
        || left.clone(),
        |acc, (op, rhs)| {
            Expr::FunctionCall(Box::new(Expr::Identifier(op.to_string())), vec![acc, rhs])
        },
    )(input);
    x
}

fn parse_pipe_expr(input: &str) -> IResult<&str, Expr> {
    let (input, left) = parse_infix_expr(input)?;
    let x = fold_many0(
        preceded(
            delimited(multispace0, tag("|>"), multispace0),
            parse_infix_expr,
        ),
        || left.clone(),
        |acc, next| Expr::Pipe(Box::new(acc), Box::new(next)),
    )(input);
    x
}

fn parse_param(input: &str) -> IResult<&str, (String, Option<String>)> {
    map(
        tuple((
            parse_identifier,
            opt(preceded(
                delimited(multispace0, char(':'), multispace0),
                parse_identifier,
            )),
        )),
        |(name_expr, type_expr)| {
            let name = match name_expr {
                Expr::Identifier(s) => s,
                _ => unreachable!(),
            };
            let type_name = match type_expr {
                Some(Expr::Identifier(s)) => Some(s),
                _ => None,
            };
            (name, type_name)
        },
    )(input)
}

fn parse_function_def(input: &str) -> IResult<&str, Expr> {
    map(
        tuple((
            tag("fun"),
            multispace1,
            parse_identifier,
            opt(preceded(
                delimited(multispace0, char(':'), multispace0),
                parse_identifier, // 型注釈を解析
            )),
            multispace0,
            char('='),
            multispace0,
            separated_list0(delimited(multispace0, char(','), multispace0), parse_param),
            multispace0,
            parse_block,
        )),
        |(_, _, name_expr, _type_annotation, _, _, _, params, _, body)| {
            let name = match name_expr {
                Expr::Identifier(s) => s,
                _ => unreachable!(),
            };
            let param_names = params
                .into_iter()
                .map(|(param_name, _type_name)| param_name)
                .collect();
            Expr::FunctionDef(name, param_names, Box::new(body))
        },
    )(input)
}

fn parse_let(input: &str) -> IResult<&str, Expr> {
    map(
        tuple((
            tag("let"),
            multispace1,
            parse_identifier,
            multispace0,
            char('='),
            multispace0,
            parse_expr,
        )),
        |(_, _, name, _, _, _, value)| {
            Expr::Let(
                match name {
                    Expr::Identifier(s) => s,
                    _ => unreachable!(),
                },
                Box::new(value),
            )
        },
    )(input)
}

fn parse_if(input: &str) -> IResult<&str, Expr> {
    map(
        tuple((
            tag("if"),
            multispace1,
            parse_expr,
            multispace1,
            tag("then"),
            multispace1,
            parse_expr,
            opt(preceded(
                tuple((multispace1, tag("else"), multispace1)),
                parse_expr,
            )),
        )),
        |(_, _, condition, _, _, _, then_branch, else_branch)| {
            Expr::If(
                Box::new(condition),
                Box::new(then_branch),
                else_branch.map(Box::new),
            )
        },
    )(input)
}

fn parse_block(input: &str) -> IResult<&str, Expr> {
    map(
        delimited(
            char('{'),
            many0(delimited(multispace0, parse_expr, multispace0)),
            char('}'),
        ),
        Expr::Block,
    )(input)
}

fn skip_comment(input: &str) -> IResult<&str, ()> {
    value((), tuple((tag("//"), take_until("\n"), opt(char('\n')))))(input)
}

fn ws(input: &str) -> IResult<&str, ()> {
    value((), many0(alt((value((), multispace1), skip_comment))))(input)
}

fn parse_paren_expr(input: &str) -> IResult<&str, Expr> {
    delimited(
        delimited(multispace0, char('('), multispace0),
        parse_expr,
        delimited(multispace0, char(')'), multispace0),
    )(input)
}

fn parse_expr(input: &str) -> IResult<&str, Expr> {
    preceded(
        ws,
        alt((
            parse_function_def,
            parse_let,
            parse_if,
            parse_pipe_expr,
            parse_infix_expr, // 追加
        )),
    )(input)
}

pub fn parse(input: &str) -> Result<Expr, String> {
    match all_consuming(delimited(ws, parse_expr, ws))(input) {
        Ok((_, expr)) => Ok(expr),
        Err(e) => Err(format!("Parse error: {:?}", e)),
    }
}

// 評価器の実装

impl Expr {
    fn from_value(val: Value) -> Expr {
        match val {
            Value::Number(n) => Expr::Number(n),
            Value::String(s) => Expr::String(s),
            Value::Boolean(b) => Expr::Boolean(b),
            Value::Unit => Expr::Block(vec![]),
            _ => panic!("Cannot convert complex Value to Expr"),
        }
    }
}

pub fn eval(expr: &Expr, env: &mut Env) -> Result<Value, String> {
    let mut current_expr = expr.clone();
    let mut current_env = env.clone();

    loop {
        // デバッグ用出力（必要に応じてコメントアウト）
        // println!("Evaluating: {:?}", current_expr);
        // println!("Current Env: {:?}", current_env);

        match &current_expr {
            Expr::Number(n) => return Ok(Value::Number(*n)),
            Expr::String(s) => return Ok(Value::String(s.clone())),
            Expr::Boolean(b) => return Ok(Value::Boolean(*b)),
            Expr::Identifier(name) => {
                return current_env
                    .get(name)
                    .cloned()
                    .ok_or_else(|| format!("Undefined variable: {}", name))
            }
            Expr::FunctionCall(func, args) => {
                let func_val = eval(func, &mut current_env)?;
                match func_val {
                    Value::Function(params, body, closure_env) => {
                        if params.len() != args.len() {
                            return Err(format!(
                                "Expected {} arguments, got {}",
                                params.len(),
                                args.len()
                            ));
                        }
                        let mut new_env = closure_env.clone();
                        for (param, arg) in params.iter().zip(args) {
                            let arg_val = eval(arg, &mut current_env)?;
                            new_env.insert(param.clone(), arg_val);
                        }
                        current_expr = (*body).clone();
                        current_env = new_env;
                        continue;
                    }
                    _ => {
                        match func.as_ref() {
                            Expr::Identifier(name) => match name.as_str() {
                                "print" => {
                                    let arg = args.get(0).ok_or("Missing argument for print")?;
                                    let value = eval(arg, &mut current_env)?;
                                    print!("{:?}", value);
                                    return Ok(Value::Unit); // return を追加
                                }
                                "println" => {
                                    let arg = args.get(0).ok_or("Missing argument for println")?;
                                    let value = eval(arg, &mut current_env)?;
                                    println!("{:?}", value);
                                    return Ok(Value::Unit); // return を追加
                                }
                                "+" => {
                                    if args.len() != 2 {
                                        return Err(
                                            "Addition requires exactly 2 arguments".to_string()
                                        );
                                    }
                                    let left = eval(&args[0], &mut current_env)?;
                                    let right = eval(&args[1], &mut current_env)?;
                                    match (left, right) {
                                        (Value::Number(l), Value::Number(r)) => {
                                            return Ok(Value::Number(l + r)); // return を追加
                                        }
                                        _ => return Err("Type mismatch in addition".to_string()),
                                    }
                                }
                                "-" => {
                                    if args.len() != 2 {
                                        return Err(
                                            "Subtraction requires exactly 2 arguments".to_string()
                                        );
                                    }
                                    let left = eval(&args[0], &mut current_env)?;
                                    let right = eval(&args[1], &mut current_env)?;
                                    match (left, right) {
                                        (Value::Number(l), Value::Number(r)) => {
                                            return Ok(Value::Number(l - r)); // return を追加
                                        }
                                        _ => return Err("Type mismatch in subtraction".to_string()),
                                    }
                                }
                                "*" => {
                                    if args.len() != 2 {
                                        return Err("Multiplication requires exactly 2 arguments"
                                            .to_string());
                                    }
                                    let left = eval(&args[0], &mut current_env)?;
                                    let right = eval(&args[1], &mut current_env)?;
                                    match (left, right) {
                                        (Value::Number(l), Value::Number(r)) => {
                                            return Ok(Value::Number(l * r)); // return を追加
                                        }
                                        _ => {
                                            return Err(
                                                "Type mismatch in multiplication".to_string()
                                            )
                                        }
                                    }
                                }
                                "/" => {
                                    if args.len() != 2 {
                                        return Err(
                                            "Division requires exactly 2 arguments".to_string()
                                        );
                                    }
                                    let left = eval(&args[0], &mut current_env)?;
                                    let right = eval(&args[1], &mut current_env)?;
                                    match (left, right) {
                                        (Value::Number(l), Value::Number(r)) if r != 0 => {
                                            return Ok(Value::Number(l / r)); // return を追加
                                        }
                                        (Value::Number(_), Value::Number(0)) => {
                                            return Err("Division by zero".to_string());
                                            // return を追加
                                        }
                                        _ => return Err("Type mismatch in division".to_string()),
                                    }
                                }
                                "%" => {
                                    if args.len() != 2 {
                                        return Err(
                                            "Modulo requires exactly 2 arguments".to_string()
                                        );
                                    }
                                    let left = eval(&args[0], &mut current_env)?;
                                    let right = eval(&args[1], &mut current_env)?;
                                    match (left, right) {
                                        (Value::Number(l), Value::Number(r)) if r != 0 => {
                                            return Ok(Value::Number(l % r)); // return を追加
                                        }
                                        (Value::Number(_), Value::Number(0)) => {
                                            return Err("Modulo by zero".to_string());
                                            // return を追加
                                        }
                                        _ => {
                                            return Err(
                                                "Type mismatch in modulo operation".to_string()
                                            )
                                        }
                                    }
                                }
                                ">" => {
                                    if args.len() != 2 {
                                        return Err(
                                            "Greater than comparison requires exactly 2 arguments"
                                                .to_string(),
                                        );
                                    }
                                    let left = eval(&args[0], &mut current_env)?;
                                    let right = eval(&args[1], &mut current_env)?;
                                    match (left, right) {
                                        (Value::Number(l), Value::Number(r)) => {
                                            return Ok(Value::Boolean(l > r)); // return を追加
                                        }
                                        _ => {
                                            return Err("Type mismatch in greater than comparison"
                                                .to_string());
                                        }
                                    }
                                }
                                "<" => {
                                    if args.len() != 2 {
                                        return Err(
                                            "Less than comparison requires exactly 2 arguments"
                                                .to_string(),
                                        );
                                    }
                                    let left = eval(&args[0], &mut current_env)?;
                                    let right = eval(&args[1], &mut current_env)?;
                                    match (left, right) {
                                        (Value::Number(l), Value::Number(r)) => {
                                            return Ok(Value::Boolean(l < r)); // return を追加
                                        }
                                        _ => {
                                            return Err(
                                                "Type mismatch in less than comparison".to_string()
                                            );
                                        }
                                    }
                                }
                                "==" => {
                                    if args.len() != 2 {
                                        return Err(
                                            "Equality comparison requires exactly 2 arguments"
                                                .to_string(),
                                        );
                                    }
                                    let left = eval(&args[0], &mut current_env)?;
                                    let right = eval(&args[1], &mut current_env)?;
                                    match (left, right) {
                                        (Value::Number(l), Value::Number(r)) => {
                                            return Ok(Value::Boolean(l == r)); // return を追加
                                        }
                                        (Value::Boolean(l), Value::Boolean(r)) => {
                                            return Ok(Value::Boolean(l == r)); // return を追加
                                        }
                                        (Value::String(l), Value::String(r)) => {
                                            return Ok(Value::Boolean(l == r)); // return を追加
                                        }
                                        _ => {
                                            return Err(
                                                "Type mismatch in equality comparison".to_string()
                                            );
                                        }
                                    }
                                }
                                _ => return Err(format!("Unknown function: {}", name)),
                            },
                            _ => return Err("Not a function".to_string()),
                        }
                    }
                }
            }
            Expr::FunctionDef(name, params, body) => {
                let func = Value::Function(params.clone(), body.clone(), current_env.clone());
                current_env.insert(name.clone(), func.clone());
                continue;
            }
            Expr::Let(name, value) => {
                let val = eval(value, &mut current_env)?;
                current_env.insert(name.clone(), val.clone());
                continue;
            }
            Expr::Block(exprs) => {
                let mut result = Value::Unit;
                for expr in exprs {
                    result = eval(&expr, &mut current_env)?;
                }
                return Ok(result);
            }
            Expr::If(condition, then_branch, else_branch) => {
                let cond_val = eval(condition, &mut current_env)?;
                match cond_val {
                    Value::Boolean(true) => {
                        current_expr = *then_branch.clone();
                    }
                    Value::Boolean(false) => {
                        if let Some(else_expr) = else_branch {
                            current_expr = *else_expr.clone();
                        } else {
                            return Ok(Value::Unit);
                        }
                    }
                    _ => return Err(format!("Condition must be a boolean: {:?}", cond_val)),
                }
            }
            Expr::Pipe(left, right) => {
                let left_val = eval(left, &mut current_env)?;
                match right.as_ref() {
                    Expr::FunctionCall(func, args) => {
                        // パイプの左側の値を引数として右側の関数に渡す
                        let mut new_args = vec![Expr::from_value(left_val)];
                        new_args.extend(args.clone());
                        current_expr = Expr::FunctionCall(func.clone(), new_args);
                    }
                    _ => return Err("Right side of pipe must be a function call".to_string()),
                }
            }
        }
    }
}

pub fn interpret(expr: &Expr) -> Result<Value, String> {
    let mut env = HashMap::new();
    // 組み込み関数の定義
    env.insert(
        "print".to_string(),
        Value::Function(
            vec!["x".to_string()],
            Box::new(Expr::FunctionCall(
                Box::new(Expr::Identifier("print".to_string())),
                vec![Expr::Identifier("x".to_string())],
            )),
            HashMap::new(),
        ),
    );

    env.insert(
        "println".to_string(),
        Value::Function(
            vec!["x".to_string()],
            Box::new(Expr::FunctionCall(
                Box::new(Expr::Identifier("println".to_string())),
                vec![Expr::Identifier("x".to_string())],
            )),
            HashMap::new(),
        ),
    );
    // 実際の評価
    eval(expr, &mut env)
}
