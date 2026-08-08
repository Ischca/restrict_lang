use restrict_lang::{generate, parse_program, ExprKind, TopDecl, TypeChecker, TypeError};

fn compile(source: &str) -> Result<String, String> {
    let (remaining, ast) = parse_program(source).map_err(|e| format!("Parse error: {:?}", e))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {:?}", remaining));
    }

    let mut type_checker = TypeChecker::new();
    type_checker
        .check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))?;

    generate(&ast).map_err(|e| format!("Codegen error: {}", e))
}

#[test]
fn parses_range_literal() {
    let source = r#"
        fun main: () -> Range<Int32> = {
            [1..10]
        }
    "#;

    let (remaining, ast) = parse_program(source).expect("range literal should parse");
    assert!(remaining.trim().is_empty());

    let TopDecl::Function(function) = &ast.declarations[0] else {
        panic!("expected function declaration");
    };
    assert!(matches!(
        function.body.expr.as_deref().map(|e| &e.kind),
        Some(ExprKind::RangeLit(range))
            if matches!(&range.start.kind, ExprKind::IntLit(1))
                && matches!(&range.end.kind, ExprKind::IntLit(10))
    ));
}

#[test]
fn typechecks_range_int32_literal() {
    let source = r#"
        fun main: () -> Range<Int32> = {
            [1..10]
        }
    "#;

    let (remaining, ast) = parse_program(source).expect("range literal should parse");
    assert!(remaining.trim().is_empty());

    let mut type_checker = TypeChecker::new();
    type_checker
        .check_program(&ast)
        .expect("Range<Int32> literal should typecheck");
}

#[test]
fn emits_wat_for_range_literal() {
    let source = r#"
        fun main: () -> Range<Int32> = {
            [1..10]
        }
    "#;

    let wat = compile(source).expect("Range<Int32> literal should compile");
    assert!(wat.contains("i32.const 8 ;; range size"));
    assert!(wat.contains("call $allocate"));
    assert!(wat.contains("i32.const 1"));
    assert!(wat.contains("i32.const 4 ;; range end offset"));
    assert!(wat.contains("i32.const 10"));
}

#[test]
fn rejects_non_int32_range_endpoint() {
    let source = r#"
        fun main: () -> Range<Int32> = {
            [1.5..10]
        }
    "#;

    let (remaining, ast) = parse_program(source).expect("range literal should parse");
    assert!(remaining.trim().is_empty());

    let mut type_checker = TypeChecker::new();
    let err = type_checker
        .check_program(&ast)
        .expect_err("Float range endpoint should be rejected");
    assert!(
        matches!(&err, TypeError::TypeMismatch { .. }),
        "expected type mismatch, got {err}"
    );
}

#[test]
fn rejects_non_int32_range_type() {
    let source = r#"
        fun main: () -> Range<Int64> = {
            [1..10]
        }
    "#;

    let (remaining, ast) = parse_program(source).expect("range literal should parse");
    assert!(remaining.trim().is_empty());

    let mut type_checker = TypeChecker::new();
    let err = type_checker
        .check_program(&ast)
        .expect_err("Range<Int64> should be rejected");
    assert!(
        matches!(&err, TypeError::UnsupportedFeature(message) if message.contains("Range<T> currently supports Int32")),
        "expected explicit Range<Int32> support error, got {err}"
    );
}
