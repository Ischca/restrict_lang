use restrict_lang::{parse_program, TypeChecker, TypeError, TypedType};

fn checker_for_source(source: &str) -> Result<TypeChecker, TypeError> {
    let (remaining, program) =
        parse_program(source).unwrap_or_else(|error| panic!("source should parse: {error:?}"));
    assert!(
        remaining.trim().is_empty(),
        "parser should consume all input, remaining: {remaining:?}"
    );

    let mut checker = TypeChecker::new();
    checker.check_program(&program)?;
    Ok(checker)
}

fn check_source(source: &str) -> Result<(), TypeError> {
    checker_for_source(source).map(drop)
}

#[test]
fn qualified_enum_constructors_and_match_type_check() {
    let source = r#"
enum ParseError {
    Empty
    Message(String)
}

fun empty_error: () -> ParseError = {
    () ParseError::Empty
}

fun message_error: (message: String) -> ParseError = {
    message |> ParseError::Message
}

fun score: (error: ParseError) -> Int32 = {
    error match {
        ParseError::Empty => { 0 }
        ParseError::Message(message) => { 1 }
    }
}
"#;

    check_source(source).expect("qualified enum construction and matching should type-check");
}

#[test]
fn checked_enum_metadata_preserves_declaration_order_and_payloads() {
    let checker = checker_for_source(
        r#"
enum Status {
    Idle
    Progress(Int64)
    Failed(String)
}
"#,
    )
    .expect("enum declaration should type-check");

    assert_eq!(
        checker.checked_enum_variants_for_type(&TypedType::Enum {
            name: "Status".to_string(),
        }),
        Some(vec![
            ("Idle".to_string(), None),
            ("Progress".to_string(), Some(TypedType::Int64)),
            ("Failed".to_string(), Some(TypedType::String)),
        ])
    );
}

#[test]
fn custom_enum_can_be_a_result_error_type() {
    let source = r#"
enum ParseError {
    Empty
    Message(String)
}

fun fail_empty: () -> Result<Int32, ParseError> = {
    Err(() ParseError::Empty)
}

fun recover: (result: Result<Int32, ParseError>) -> Int32 = {
    result match {
        Ok(value) => { value }
        Err(error) => {
            error match {
                ParseError::Empty => { 0 }
                ParseError::Message(message) => { 1 }
            }
        }
    }
}
"#;

    check_source(source).expect("Result should accept a closed user enum error type");
}

#[test]
fn enum_match_reports_the_missing_qualified_variant() {
    let error = check_source(
        r#"
enum ParseError {
    Empty
    Message(String)
}

fun score: (error: ParseError) -> Int32 = {
    error match {
        ParseError::Empty => { 0 }
    }
}
"#,
    )
    .expect_err("match should be rejected as non-exhaustive");

    match error {
        TypeError::NonExhaustivePatterns { missing, .. } => {
            assert!(missing.contains("ParseError::Message(_)"), "{missing}");
        }
        other => panic!("expected an exhaustiveness error, got {other:?}"),
    }
}

#[test]
fn enum_payload_patterns_are_checked_recursively_for_exhaustiveness() {
    let source = r#"
enum Toggle {
    Disabled
    Enabled(Boolean)
}

fun score: (toggle: Toggle) -> Int32 = {
    toggle match {
        Toggle::Disabled => { 0 }
        Toggle::Enabled(true) => { 1 }
        Toggle::Enabled(false) => { 2 }
    }
}
"#;

    check_source(source).expect("nested Boolean payload patterns cover the enum");
}

#[test]
fn enum_constructor_validates_variant_arity_and_payload_type() {
    let missing_payload = check_source(
        r#"
enum ParseError { Message(String) }
fun bad: () -> ParseError = { () ParseError::Message }
"#,
    )
    .expect_err("unary constructor should require one payload");
    assert_eq!(
        missing_payload,
        TypeError::ArityMismatch {
            expected: 1,
            found: 0,
        }
    );

    let extra_payload = check_source(
        r#"
enum ParseError { Empty }
fun bad: () -> ParseError = { (1) ParseError::Empty }
"#,
    )
    .expect_err("nullary constructor should reject a payload");
    assert_eq!(
        extra_payload,
        TypeError::ArityMismatch {
            expected: 0,
            found: 1,
        }
    );

    let wrong_payload = check_source(
        r#"
enum ParseError { Message(String) }
fun bad: () -> ParseError = { 1 |> ParseError::Message }
"#,
    )
    .expect_err("constructor payload should match its declaration");
    assert!(matches!(wrong_payload, TypeError::TypeMismatch { .. }));
}

#[test]
fn enum_variant_reference_is_not_a_first_class_value() {
    let error = check_source(
        r#"
enum ParseError { Empty }
fun bad: () -> ParseError = { ParseError::Empty }
"#,
    )
    .expect_err("a constructor path outside direct OSV call position should be rejected");

    assert!(
        matches!(error, TypeError::UnsupportedFeature(message) if message.contains("must be called directly with OSV syntax"))
    );
}

#[test]
fn duplicate_enum_types_and_variants_are_rejected() {
    let duplicate_type = check_source(
        r#"
enum State { Ready }
record State { value: Int32 }
"#,
    )
    .expect_err("record and enum names share the type namespace");
    assert!(
        matches!(duplicate_type, TypeError::UnsupportedFeature(message) if message.contains("duplicate type declaration 'State'"))
    );

    let duplicate_variant = check_source(
        r#"
enum State { Ready Ready }
"#,
    )
    .expect_err("variants must be unique within an enum");
    assert!(
        matches!(duplicate_variant, TypeError::UnsupportedFeature(message) if message.contains("duplicate variant 'State::Ready'"))
    );
}

#[test]
fn recursive_enum_payloads_are_rejected_directly_and_through_records() {
    let direct = check_source(
        r#"
enum Loop { Next(Loop) }
"#,
    )
    .expect_err("directly recursive enum should be rejected");
    assert!(
        matches!(direct, TypeError::UnsupportedFeature(message) if message.contains("recursive enum definitions"))
    );

    let through_record = check_source(
        r#"
enum Loop { Next(Node) }
record Node { next: Loop }
"#,
    )
    .expect_err("enum/record cycles should be rejected before layout construction");
    assert!(
        matches!(through_record, TypeError::UnsupportedFeature(message) if message.contains("recursive enum definitions"))
    );
}

#[test]
fn temporal_and_function_enum_payloads_are_rejected() {
    let function_payload = check_source(
        r#"
enum Callback { Ready((Int32) -> Int32) }
"#,
    )
    .expect_err("function payloads are outside the initial enum slice");
    assert!(
        matches!(function_payload, TypeError::UnsupportedFeature(message) if message.contains("function payload"))
    );

    let temporal_payload = check_source(
        r#"
enum Borrowed { Value(String<~request>) }
"#,
    )
    .expect_err("temporal payloads are outside the initial enum slice");
    assert!(
        matches!(temporal_payload, TypeError::UnsupportedFeature(message) if message.contains("temporal payload"))
    );
}

#[test]
fn enum_payload_records_must_be_concrete_and_non_temporal() {
    let open_generic = check_source(
        r#"
record Box<T> { value: T }
enum Bad { Payload(Box) }
"#,
    )
    .expect_err("a bare generic record is not a concrete enum payload");
    assert!(
        matches!(open_generic, TypeError::UnsupportedFeature(message) if message.contains("payload type 'Box' is not concrete"))
    );

    check_source(
        r#"
record Box<T> { value: T }
enum Good { Payload(Box<Int32>) }
"#,
    )
    .expect("a fully instantiated non-temporal record is a concrete enum payload");
}

#[test]
fn enum_equality_is_rejected_until_structural_semantics_exist() {
    let error = check_source(
        r#"
enum State { Ready }

fun same: () -> Boolean = {
    val left = () State::Ready;
    val right = () State::Ready;
    left == right
}
"#,
    )
    .expect_err("enum equality must not fall back to pointer equality");

    assert!(
        matches!(error, TypeError::UnsupportedFeature(message) if message.contains("user-defined enum equality is not supported"))
    );
}

#[test]
fn moving_a_string_into_an_enum_constructor_consumes_it() {
    let error = check_source(
        r#"
enum Message { Text(String) }

fun bad: (text: String) -> Message = {
    val message = text |> Message::Text;
    val reused = text;
    message
}
"#,
    )
    .expect_err("String payload construction should move the source binding");

    assert!(matches!(error, TypeError::AffineViolation(name) if name == "text"));
}

#[test]
fn enum_is_copy_only_when_every_payload_is_copy() {
    let copy_source = r#"
enum Code {
    Empty
    Value(Int32)
}

fun score: (code: Code) -> Int32 = {
    val first = code match {
        Code::Empty => { 0 }
        Code::Value(value) => { value }
    };
    val second = code match {
        Code::Empty => { 0 }
        Code::Value(value) => { value }
    };
    first + second
}
"#;
    check_source(copy_source).expect("an enum with only scalar payloads should be Copy");

    let affine_error = check_source(
        r#"
enum Message { Text(String) }

fun bad: (message: Message) -> Int32 = {
    val first = message match {
        Message::Text(_) => { 0 }
    };
    message match {
        Message::Text(_) => { first }
    }
}
"#,
    )
    .expect_err("an enum containing String should remain affine");
    assert!(matches!(affine_error, TypeError::AffineViolation(name) if name == "message"));
}

#[test]
fn refutable_enum_patterns_are_rejected_in_val_bindings() {
    let error = check_source(
        r#"
enum ParseError { Empty Message(String) }

fun bad: () -> Int32 = {
    val ParseError::Empty = () ParseError::Empty;
    0
}
"#,
    )
    .expect_err("refutable enum patterns belong in match expressions");

    assert!(
        matches!(error, TypeError::UnsupportedFeature(message) if message.contains("not allowed in a val binding"))
    );
}
