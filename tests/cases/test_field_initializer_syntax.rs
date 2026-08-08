use restrict_lang::ast::{Expr, ExprKind, TopDecl};
use restrict_lang::parse_program;

fn fully_parses(source: &str) -> bool {
    match parse_program(source) {
        Ok((remaining, _)) => remaining.trim().is_empty(),
        Err(_) => false,
    }
}

fn binding_expr(source: &str) -> Expr {
    let (remaining, program) = parse_program(source).expect("source should parse");
    assert!(
        remaining.trim().is_empty(),
        "parser left unconsumed input: {remaining:?}"
    );

    match program.declarations.into_iter().next().unwrap() {
        TopDecl::Binding(binding) => *binding.value,
        other => panic!("expected binding declaration, got {other:?}"),
    }
}

#[test]
fn record_declarations_accept_comma_separated_fields() {
    let (remaining, program) = parse_program(
        r#"
record Point {
    x: Int32,
    y: Int32
}
"#,
    )
    .expect("comma-separated record fields should parse");

    assert!(remaining.trim().is_empty());
    match &program.declarations[0] {
        TopDecl::Record(record) => assert_eq!(record.fields.len(), 2),
        other => panic!("expected record declaration, got {other:?}"),
    }
}

#[test]
fn record_declarations_accept_newline_separated_fields() {
    let (remaining, program) = parse_program(
        r#"
record Point {
    x: Int32
    y: Int32
}
"#,
    )
    .expect("newline-separated record fields should parse");

    assert!(remaining.trim().is_empty());
    match &program.declarations[0] {
        TopDecl::Record(record) => assert_eq!(record.fields.len(), 2),
        other => panic!("expected record declaration, got {other:?}"),
    }
}

#[test]
fn context_declarations_accept_newline_separated_fields() {
    let (remaining, program) = parse_program(
        r#"
context Database {
    connection: Connection
    timeout: Int32
}
"#,
    )
    .expect("newline-separated context fields should parse");

    assert!(remaining.trim().is_empty());
    match &program.declarations[0] {
        TopDecl::Context(context) => assert_eq!(context.fields.len(), 2),
        other => panic!("expected context declaration, got {other:?}"),
    }
}

#[test]
fn record_literals_still_require_commas_between_fields() {
    assert!(fully_parses(
        r#"
record Point {
    x: Int32
    y: Int32
}

val point = Point { x: 1, y: 2 }
"#
    ));

    assert!(!fully_parses(
        r#"
record Point {
    x: Int32
    y: Int32
}

val point = Point { x: 1 y: 2 }
"#
    ));
}

#[test]
fn record_literal_fields_require_colon() {
    assert!(fully_parses(
        r#"
record Point {
    x: Int32
}

val point = Point { x: 1 }
"#
    ));

    assert!(!fully_parses(
        r#"
record Point {
    x: Int32
}

val point = Point { x = 1 }
"#
    ));
}

#[test]
fn anonymous_record_fields_require_colon() {
    assert!(matches!(
        binding_expr(
            r#"
val point = { x: 1 }
"#
        )
        .kind,
        ExprKind::RecordLit(_)
    ));

    assert!(!matches!(
        binding_expr(
            r#"
val point = { x = 1 }
"#
        )
        .kind,
        ExprKind::RecordLit(_)
    ));
}

#[test]
fn clone_update_fields_require_colon() {
    assert!(fully_parses(
        r#"
val point = base.clone { x: 1 }
"#
    ));

    assert!(!fully_parses(
        r#"
val point = base.clone { x = 1 }
"#
    ));
}

#[test]
fn context_binding_fields_require_colon() {
    assert!(fully_parses(
        r#"
context Config {
    limit: Int32
}

fun main: () -> Int32 = {
    with Config { limit: 1 } {
        limit
    }
}
"#
    ));

    assert!(!fully_parses(
        r#"
context Config {
    limit: Int32
}

fun main: () -> Int32 = {
    with Config { limit = 1 } {
        limit
    }
}
"#
    ));
}
