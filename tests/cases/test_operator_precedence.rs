use restrict_lang::{ast::*, parse_program};

fn main_expr(source: &str) -> Expr {
    let (_, program) = parse_program(source).expect("source should parse");
    let TopDecl::Function(function) = &program.declarations[0] else {
        panic!("expected function declaration");
    };
    function
        .body
        .expr
        .as_deref()
        .expect("function body should have final expression")
        .clone()
}

#[test]
fn multiplicative_binds_tighter_than_additive() {
    let expr = main_expr(
        r#"
fun main: () -> Int32 = {
    1 + 2 * 3
}
"#,
    );

    let ExprKind::Binary(add) = &expr.kind else {
        panic!("expected top-level addition, got {expr:?}");
    };
    assert!(matches!(add.op, BinaryOp::Add));
    assert!(matches!(&add.left.kind, ExprKind::IntLit(1)));

    let ExprKind::Binary(mul) = &add.right.kind else {
        panic!("expected multiplication on the right, got {:?}", add.right);
    };
    assert!(matches!(mul.op, BinaryOp::Mul));
}

#[test]
fn comparison_binds_tighter_than_logical_ops() {
    let expr = main_expr(
        r#"
fun main: () -> Boolean = {
    1 + 2 > 3 && false || true
}
"#,
    );

    let ExprKind::Binary(or_expr) = &expr.kind else {
        panic!("expected top-level logical or, got {expr:?}");
    };
    assert!(matches!(or_expr.op, BinaryOp::Or));

    let ExprKind::Binary(and_expr) = &or_expr.left.kind else {
        panic!("expected logical and on the left, got {:?}", or_expr.left);
    };
    assert!(matches!(and_expr.op, BinaryOp::And));

    let ExprKind::Binary(gt_expr) = &and_expr.left.kind else {
        panic!("expected comparison on the left, got {:?}", and_expr.left);
    };
    assert!(matches!(gt_expr.op, BinaryOp::Gt));

    let ExprKind::Binary(add_expr) = &gt_expr.left.kind else {
        panic!(
            "expected addition before comparison, got {:?}",
            gt_expr.left
        );
    };
    assert!(matches!(add_expr.op, BinaryOp::Add));
}

#[test]
fn parentheses_override_binary_precedence() {
    let expr = main_expr(
        r#"
fun main: () -> Int32 = {
    (1 + 2) * 3
}
"#,
    );

    let ExprKind::Binary(mul) = &expr.kind else {
        panic!("expected top-level multiplication, got {expr:?}");
    };
    assert!(matches!(mul.op, BinaryOp::Mul));

    let ExprKind::Binary(add) = &mul.left.kind else {
        panic!(
            "expected parenthesized addition on the left, got {:?}",
            mul.left
        );
    };
    assert!(matches!(add.op, BinaryOp::Add));
}

#[test]
fn subtraction_after_parenthesized_expression_stays_binary() {
    let expr = main_expr(
        r#"
fun main: () -> Int32 = {
    10 + (2 * 3) - 4
}
"#,
    );

    let ExprKind::Binary(sub) = &expr.kind else {
        panic!("expected top-level subtraction, got {expr:?}");
    };
    assert!(matches!(sub.op, BinaryOp::Sub));
    assert!(matches!(&sub.right.kind, ExprKind::IntLit(4)));

    let ExprKind::Binary(add) = &sub.left.kind else {
        panic!("expected addition on the left, got {:?}", sub.left);
    };
    assert!(matches!(add.op, BinaryOp::Add));

    let ExprKind::Binary(mul) = &add.right.kind else {
        panic!(
            "expected parenthesized multiplication on the right, got {:?}",
            add.right
        );
    };
    assert!(matches!(mul.op, BinaryOp::Mul));
}

#[test]
fn unary_binds_tighter_than_multiplicative() {
    let expr = main_expr(
        r#"
fun main: () -> Int32 = {
    -1 * 2
}
"#,
    );

    let ExprKind::Binary(mul) = &expr.kind else {
        panic!("expected top-level multiplication, got {expr:?}");
    };
    assert!(matches!(mul.op, BinaryOp::Mul));
    assert!(matches!(
        &mul.left.kind,
        ExprKind::Unary(UnaryExpr {
            op: UnaryOp::Neg,
            ..
        })
    ));
}

#[test]
fn logical_not_binds_tighter_than_logical_and() {
    let expr = main_expr(
        r#"
fun main: () -> Boolean = {
    !false && true
}
"#,
    );

    let ExprKind::Binary(and_expr) = &expr.kind else {
        panic!("expected top-level logical and, got {expr:?}");
    };
    assert!(matches!(and_expr.op, BinaryOp::And));
    assert!(matches!(
        &and_expr.left.kind,
        ExprKind::Unary(UnaryExpr {
            op: UnaryOp::Not,
            ..
        })
    ));
}

#[test]
fn pipe_applies_after_binary_expression() {
    let expr = main_expr(
        r#"
fun main: () -> Int32 = {
    1 + 2 |> double
}
"#,
    );

    let ExprKind::Pipe(pipe) = &expr.kind else {
        panic!("expected top-level pipe, got {expr:?}");
    };
    assert!(matches!(pipe.target, PipeTarget::Ident(ref name) if name == "double"));

    let ExprKind::Binary(add) = &pipe.expr.kind else {
        panic!(
            "expected pipe source to be the full binary expression, got {:?}",
            pipe.expr
        );
    };
    assert!(matches!(add.op, BinaryOp::Add));
}

#[test]
fn parentheses_make_binary_expression_the_osv_object() {
    let expr = main_expr(
        r#"
fun main: () -> Int32 = {
    (1 + 2) double
}
"#,
    );

    let ExprKind::Call(call) = &expr.kind else {
        panic!("expected top-level direct OSV call, got {expr:?}");
    };
    assert!(matches!(&call.function.kind, ExprKind::Ident(name) if name == "double"));

    let [arg] = call.args.as_slice() else {
        panic!("expected one OSV object argument, got {:?}", call.args);
    };
    assert!(matches!(
        &arg.kind,
        ExprKind::Binary(BinaryExpr {
            op: BinaryOp::Add,
            ..
        })
    ));
}
