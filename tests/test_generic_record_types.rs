use restrict_lang::{parse_program, TypeChecker};

fn parse(source: &str) -> Result<(), String> {
    let (remaining, _program) =
        parse_program(source).map_err(|e| format!("Parse error: {:?}", e))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {:?}", remaining));
    }

    Ok(())
}

fn type_check(source: &str) -> Result<(), String> {
    let (remaining, program) =
        parse_program(source).map_err(|e| format!("Parse error: {:?}", e))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {:?}", remaining));
    }

    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .map_err(|e| format!("Type error: {}", e))
}

#[test]
fn generic_record_declaration_parses() {
    let source = r#"
record Box<T> {
    value: T
}
"#;

    parse(source).expect("generic record declaration should parse");
}

#[test]
fn generic_record_declaration_type_checks() {
    let source = r#"
record Box<T> {
    value: T
}
"#;

    type_check(source).expect("generic record declaration should type-check");
}

#[test]
fn annotated_generic_record_binding_type_checks() {
    let source = r#"
record Box<T> {
    value: T
}

fun main: () -> Int32 = {
    val b: Box<Int32> = Box { value: 1 };
    0
}
"#;

    type_check(source).expect("Box<Int32> annotation should instantiate Box<T>");
}

#[test]
fn generic_record_field_access_yields_instantiated_type() {
    let source = r#"
record Box<T> {
    value: T
}

fun main: () -> Int32 = {
    val b: Box<Int32> = Box { value: 1 };
    b.value
}
"#;

    type_check(source).expect("Box<Int32>.value should type-check as Int32");
}

#[test]
fn generic_record_field_mismatch_rejects() {
    let source = r#"
record Box<T> {
    value: T
}

fun main: () -> Int32 = {
    val b: Box<Int32> = Box { value: "not an int" };
    0
}
"#;

    let err = type_check(source).expect_err("Box<Int32> should reject a String value");
    assert!(
        err.contains("Type mismatch"),
        "error should report the mismatched generic field type, got: {}",
        err
    );
}

#[test]
fn generic_record_option_and_list_fields_use_instantiated_type() {
    let source = r#"
record Bag<T> {
    current: Option<T>,
    history: List<T>
}

fun main: () -> Int32 = {
    val bag: Bag<Int32> = Bag {
        current: Some(1),
        history: [1, 2, 3]
    };

    bag.current match {
        Some(value) => { value }
        None => { 0 }
    }
}
"#;

    type_check(source).expect("Option<T> and List<T> fields should instantiate with Bag<Int32>");
}

#[test]
fn generic_record_destructure_binds_instantiated_field_types() {
    let source = r#"
record Slot<T> {
    value: T,
    fallback: Option<T>
}

fun read_plus_one: (slot: Slot<Int32>) -> Int32 = {
    val Slot {
        value,
        fallback
    } = slot;

    value + 1
}

fun main: () -> Int32 = {
    val slot: Slot<Int32> = Slot {
        value: 41,
        fallback: None
    };

    slot |> read_plus_one
}
"#;

    type_check(source).expect("Slot<Int32> destructuring should bind value as Int32");
}

#[test]
fn generic_record_literal_infers_type_arg_from_fields() {
    let source = r#"
record Slot<T> {
    value: T,
    fallback: Option<T>
}

fun main: () -> Int32 = {
    val slot = Slot {
        value: 41,
        fallback: Some(40)
    };

    slot.value
}
"#;

    type_check(source).expect("Slot<T> literal should infer T from field values");
}

#[test]
fn generic_record_literal_infers_type_arg_from_function_parameter() {
    let source = r#"
record Box<T> {
    value: T
}

fun unwrap: <T>(box: Box<T>) -> T = {
    box.value
}

fun main: () -> Int32 = {
    Box { value: 1 } |> unwrap
}
"#;

    type_check(source).expect("Box<T> should infer T from literal fields through unwrap");
}

#[test]
fn nested_generic_record_literal_infers_type_arg_from_function_parameter() {
    let source = r#"
record Box<T> {
    value: T
}

record Envelope<T> {
    inner: Box<T>
}

fun unwrap_envelope: <T>(envelope: Envelope<T>) -> T = {
    envelope.inner.value
}

fun main: () -> Int32 = {
    Envelope { inner: Box { value: 1 } } |> unwrap_envelope
}
"#;

    type_check(source).expect("nested generic records should infer T through field substitution");
}

#[test]
fn generic_record_literal_rejects_unresolved_type_arg() {
    let source = r#"
record Box<T> {
    value: List<T>
}

fun make: () = {
    Box { value: [] }
}
"#;

    let err = type_check(source).expect_err("Box<T> should not keep unresolved T");
    assert!(
        err.contains("Cannot infer type"),
        "error should report the unresolved generic record argument, got: {err}"
    );
    assert!(
        !err.contains("InferVar") && !err.contains("TypeVarId") && !err.contains("?0"),
        "error should hide inference internals, got: {err}"
    );
}
