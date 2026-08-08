use restrict_lang::ast::{collect_node_ids, NodeId, TopDecl, Type};
use restrict_lang::{parse_program, WasmCodeGen};

fn parse_complete(input: &str) -> restrict_lang::ast::Program {
    let (remaining, program) = parse_program(input).expect("source should parse");
    assert!(
        remaining.trim().is_empty(),
        "source should parse completely, remaining: {remaining:?}"
    );
    program
}

fn parse_error_message(input: &str) -> String {
    match parse_program(input) {
        Ok((remaining, program)) if remaining.trim().is_empty() => {
            panic!("expected parse failure, got program={program:?}")
        }
        Ok((remaining, _)) => format!("unparsed input: {remaining}"),
        Err(err) => format!("{err:?}"),
    }
}

#[test]
fn form_takes_and_of_are_source_syntax() {
    let program = parse_complete(
        r#"
pub form Showable {
    fun show: (self: Self) -> String
}

form Comparable {
    fun compare: (self: Self, other: Self) -> Int32
}

record Widget {
    label: String
}

Widget takes Showable {
    fun show: (self: Widget) -> String = {
        self.label
    }
}

fun render: <T of Showable + Comparable>(value: T) -> String = {
    (value) show
}
"#,
    );

    let TopDecl::Export(export) = &program.declarations[0] else {
        panic!("expected a public form");
    };
    let TopDecl::Form(showable) = export.item.as_ref() else {
        panic!("expected a form export");
    };
    assert_eq!(showable.name, "Showable");
    assert_eq!(showable.methods.len(), 1);
    assert_eq!(showable.methods[0].name, "show");
    assert_eq!(
        showable.methods[0].return_type,
        Type::Named("String".to_string())
    );

    let TopDecl::Takes(takes) = &program.declarations[3] else {
        panic!("expected a takes declaration");
    };
    assert_eq!(takes.target, "Widget");
    assert_eq!(takes.form_name, "Showable");
    assert_eq!(takes.functions.len(), 1);
    assert_eq!(takes.functions[0].name, "show");

    let TopDecl::Function(render) = &program.declarations[4] else {
        panic!("expected the constrained function");
    };
    assert_eq!(
        render.type_params[0].of_forms,
        vec!["Showable".to_string(), "Comparable".to_string()]
    );

    let ids = collect_node_ids(&program);
    assert_eq!(ids, (0..ids.len() as u32).map(NodeId).collect::<Vec<_>>());
}

#[test]
fn form_methods_require_typed_signatures_without_defaults() {
    let missing_signature = parse_error_message(
        r#"
form Showable {
    fun show: () = { "default" }
}
"#,
    );
    assert!(missing_signature.contains("Tag"));

    let default_body = parse_error_message(
        r#"
form Showable {
    fun show: (self: Self) -> String = { "default" }
}
"#,
    );
    assert!(default_body.contains("default form method bodies"));
}

#[test]
fn deferred_form_features_fail_explicitly() {
    let generic_form = parse_error_message(
        r#"
form Showable<T> {
    fun show: (self: Self) -> String
}
"#,
    );
    assert!(generic_form.contains("generic forms are not supported yet"));

    let associated_type = parse_error_message(
        r#"
form Iterable {
    type Item
}
"#,
    );
    assert!(associated_type.contains("associated form types are not supported yet"));

    let generic_takes = parse_error_message(
        r#"
Widget<T> takes Showable {
    fun show: (self: Widget) -> String = { "widget" }
}
"#,
    );
    assert!(generic_takes.contains("generic or temporal takes targets"));

    let conditional_takes = parse_error_message(
        r#"
Widget takes Showable where T of Other {
    fun show: (self: Widget) -> String = { "widget" }
}
"#,
    );
    assert!(conditional_takes.contains("conditional takes declarations"));

    let public_takes = parse_error_message(
        r#"
pub Widget takes Showable {
    fun show: (self: Widget) -> String = { "widget" }
}
"#,
    );
    assert!(public_takes.contains("takes declarations cannot be public"));
}

#[test]
fn codegen_rejects_context_as_a_takes_target_without_typechecking() {
    let program = parse_complete(
        r#"
form Showable {
    fun show: (self: Self) -> String
}

context Request {
    label: String
}

Request takes Showable {
    fun show: (self: Request) -> String = { self.label }
}
"#,
    );

    let error = WasmCodeGen::new()
        .generate(&program)
        .expect_err("codegen must independently reject a context takes target")
        .to_string();
    assert!(error.contains("takes target 'Request' is not a concrete, non-generic record"));
}
