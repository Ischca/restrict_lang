use restrict_lang::{parse_program, ast::*};

#[test]
fn test_simple_lambda() {
    let input = "val f = |x| x + 1";
    let (_, program) = parse_program(input).unwrap();
    
    assert_eq!(program.declarations.len(), 1);
    match &program.declarations[0] {
        TopDecl::Binding(bind) => {
            assert_eq!(bind.name, "f");
            assert!(!bind.mutable);
            match &*bind.value {
                Expr::Lambda(lambda) => {
                    assert_eq!(lambda.params, vec!["x"]);
                    // Check body is x + 1
                    match &*lambda.body {
                        Expr::Binary(bin) => {
                            assert!(matches!(&*bin.left, Expr::Ident(s) if s == "x"));
                            assert!(matches!(bin.op, BinaryOp::Add));
                            assert!(matches!(&*bin.right, Expr::IntLit(1)));
                        }
                        _ => panic!("Expected binary expression in lambda body"),
                    }
                }
                _ => panic!("Expected lambda expression"),
            }
        }
        _ => panic!("Expected binding declaration"),
    }
}

#[test]
fn test_lambda_multiple_params() {
    let input = "val add = |x, y| x + y";
    let (_, program) = parse_program(input).unwrap();
    
    match &program.declarations[0] {
        TopDecl::Binding(bind) => {
            match &*bind.value {
                Expr::Lambda(lambda) => {
                    assert_eq!(lambda.params, vec!["x", "y"]);
                }
                _ => panic!("Expected lambda expression"),
            }
        }
        _ => panic!("Expected binding declaration"),
    }
}

#[test]
fn test_lambda_no_params() {
    let input = "val unit = || 42";
    let (_, program) = parse_program(input).unwrap();
    
    match &program.declarations[0] {
        TopDecl::Binding(bind) => {
            match &*bind.value {
                Expr::Lambda(lambda) => {
                    assert_eq!(lambda.params.len(), 0);
                    assert!(matches!(&*lambda.body, Expr::IntLit(42)));
                }
                _ => panic!("Expected lambda expression"),
            }
        }
        _ => panic!("Expected binding declaration"),
    }
}

#[test]
fn test_nested_lambda() {
    let input = "val curry_add = |x| |y| x + y";
    let (_, program) = parse_program(input).unwrap();
    
    match &program.declarations[0] {
        TopDecl::Binding(bind) => {
            match &*bind.value {
                Expr::Lambda(outer) => {
                    assert_eq!(outer.params, vec!["x"]);
                    match &*outer.body {
                        Expr::Lambda(inner) => {
                            assert_eq!(inner.params, vec!["y"]);
                        }
                        _ => panic!("Expected nested lambda"),
                    }
                }
                _ => panic!("Expected lambda expression"),
            }
        }
        _ => panic!("Expected binding declaration"),
    }
}

#[test]
fn test_lambda_with_block_body() {
    let input = r#"val compute = |x| {
        val y = x * 2;
        val z = y + 1;
        z
    }"#;
    let (_, program) = parse_program(input).unwrap();
    
    match &program.declarations[0] {
        TopDecl::Binding(bind) => {
            match &*bind.value {
                Expr::Lambda(lambda) => {
                    assert_eq!(lambda.params, vec!["x"]);
                    assert!(matches!(&*lambda.body, Expr::Block(_)));
                }
                _ => panic!("Expected lambda expression"),
            }
        }
        _ => panic!("Expected binding declaration"),
    }
}

#[test]
fn test_lambda_in_match_arm() {
    let input = r#"fun test = {
        val opt = Some(5);
        opt match {
            Some(x) => { |y| x + y }
            None => { |y| y }
        }
    }"#;
    let (_, program) = parse_program(input).unwrap();
    
    // Basic validation that it parses
    assert_eq!(program.declarations.len(), 1);
}

#[test]
fn test_lambda_not_confused_with_list_pattern() {
    // Ensure |x| syntax doesn't interfere with [head | tail] pattern
    let input = r#"fun test = {
        val lst = [1, 2, 3];
        val f = |x| x + 1;
        lst match {
            [head | tail] => { f(head) }
            [] => { 0 }
        }
    }"#;
    let (_, program) = parse_program(input).unwrap();
    
    // Basic validation that it parses correctly
    assert_eq!(program.declarations.len(), 1);
}

#[test]
fn test_lambda_call_immediately() {
    // Due to OSV syntax, (|x| x * 2)(21) is parsed as 21 applied to the lambda
    // which is: (|x| x * 2) 21 => 21 (|x| x * 2)
    let input = "val result = (|x| x * 2)(21)";
    let (_, program) = parse_program(input).unwrap();
    
    match &program.declarations[0] {
        TopDecl::Binding(bind) => {
            match &*bind.value {
                Expr::Call(call) => {
                    // Due to OSV, the function is 21 and the lambda is the argument
                    assert!(matches!(&*call.function, Expr::IntLit(21)));
                    assert_eq!(call.args.len(), 1);
                    assert!(matches!(&*call.args[0], Expr::Lambda(_)));
                }
                _ => panic!("Expected call expression"),
            }
        }
        _ => panic!("Expected binding declaration"),
    }
}