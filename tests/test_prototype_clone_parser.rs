use restrict_lang::ast::{Expr, ExprKind, FieldInit, TopDecl};
use restrict_lang::parser::parse_program;

fn parse_binding_expr(source: &str) -> Expr {
    let (remaining, program) = parse_program(source).expect("source should parse");
    assert!(
        remaining.trim().is_empty(),
        "parser left unconsumed input: {remaining:?}"
    );
    assert_eq!(program.declarations.len(), 1);

    match program.declarations.into_iter().next().unwrap() {
        TopDecl::Binding(binding) => *binding.value,
        other => panic!("expected binding declaration, got {other:?}"),
    }
}

fn field_names(expr: &Expr) -> Vec<&str> {
    match &expr.kind {
        ExprKind::Clone(clone) => clone
            .updates
            .fields
            .iter()
            .map(|field| match field {
                FieldInit::Field { name, .. } => name.as_str(),
                FieldInit::Spread(_) => "...",
            })
            .collect(),
        other => panic!("expected clone expression, got {other:?}"),
    }
}

#[test]
fn parses_clone_updates_with_colon_fields() {
    let expr = parse_binding_expr(
        r#"
val child = base.clone { hp: 500, name: "worker" }
"#,
    );

    assert_eq!(field_names(&expr), vec!["hp", "name"]);
}

#[test]
fn rejects_clone_updates_with_equals_fields() {
    let result = parse_program(
        r#"
val child = base.clone { hp = 500 }
"#,
    );

    assert!(result.is_err(), "clone update fields must use colon syntax");
}

#[test]
fn parses_freeze_after_clone_update() {
    let expr = parse_binding_expr(
        r#"
val child = base.clone { hp: 500 } freeze
"#,
    );

    match &expr.kind {
        ExprKind::Freeze(inner) => assert_eq!(field_names(inner), vec!["hp"]),
        other => panic!("expected frozen clone expression, got {other:?}"),
    }
}

#[test]
fn rejects_unbraced_traditional_clone_call_shape() {
    let result = parse_program(
        r#"
val child = base.clone(hp)
"#,
    );

    assert!(result.is_err(), "traditional clone call shape should fail");
}
