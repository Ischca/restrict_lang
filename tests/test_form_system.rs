use restrict_lang::parse_program;

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
fn form_declarations_are_outside_v001_surface() {
    let err = parse_error_message(
        r#"
form Showable {
    show: (x: Int32) -> String
}
"#,
    );

    assert!(err.contains("form"));
    assert!(err.contains("unsupported in v0.0.1"));
}

#[test]
fn takes_declarations_are_outside_v001_surface() {
    let err = parse_error_message(
        r#"
Widget takes Showable {
    show = |x| { "widget" }
}
"#,
    );

    assert!(err.contains("takes"));
    assert!(err.contains("unparsed input"));
}

#[test]
fn of_form_constraints_are_not_public_syntax() {
    let err = parse_error_message(
        r#"
fun display: <T of Showable>(value: T) -> String = {
    "value"
}
"#,
    );

    assert!(err.contains("of"));
    assert!(err.contains("Tag"));
}
