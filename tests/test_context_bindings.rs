use restrict_lang::{parse_program, TypeChecker};

fn type_check(input: &str) -> Result<(), String> {
    let (remaining, program) = parse_program(input).map_err(|e| format!("Parse error: {:?}", e))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {:?}", remaining));
    }

    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .map_err(|e| format!("Type error: {}", e))
}

#[test]
fn context_binding_is_available_in_body() {
    let input = r#"
context Config {
    limit: Int32
}

fun main: () -> Int32 = {
    val source = 41;
    with Config { limit: source } {
        limit + 1
    }
}
"#;

    type_check(input).expect("context binding should be scoped into the with body");
}

#[test]
fn context_binding_supplies_expected_type_to_empty_collection() {
    let input = r#"
context Bucket {
    items: List<Int32>
}

fun main: () -> List<Int32> = {
    with Bucket { items: [] } {
        items
    }
}
"#;

    type_check(input).expect("context field type should infer empty collection bindings");
}

#[test]
fn context_binding_rejects_unknown_field() {
    let input = r#"
context Config {
    limit: Int32
}

fun main: () -> Int32 = {
    with Config { missing: 1 } {
        1
    }
}
"#;

    let err = type_check(input).expect_err("unknown context fields should be rejected");
    assert!(
        err.contains("Unknown field missing in record Config"),
        "unexpected error: {err}"
    );
}

#[test]
fn context_binding_rejects_type_mismatch() {
    let input = r#"
context Config {
    limit: Int32
}

fun main: () -> Int32 = {
    with Config { limit: "slow" } {
        limit
    }
}
"#;

    let err = type_check(input).expect_err("context field bindings should be type checked");
    assert!(err.contains("Type mismatch"), "unexpected error: {err}");
}
