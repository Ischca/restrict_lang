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
fn annotated_return_type_is_used_for_forward_reference() {
    let input = r#"
fun main: () -> Boolean = {
    41 |> is_positive
}

fun is_positive: (value: Int32) -> Boolean = {
    value > 0
}
"#;

    type_check(input).expect("forward reference should use annotated return type");
}

#[test]
fn unannotated_function_can_call_later_annotated_function() {
    let input = r#"
fun wraps_positive: (value: Int32) = {
    value |> is_positive
}

fun is_positive: (value: Int32) -> Boolean = {
    value > 0
}

fun main: () -> Boolean = {
    41 |> wraps_positive
}
"#;

    type_check(input).expect("unannotated caller should infer from annotated forward callee");
}

#[test]
fn unannotated_return_is_inferred_before_annotated_forward_reference() {
    let input = r#"
fun main: () -> Float64 = {
    41.5 |> adjust
}

fun adjust: (value: Float64) = {
    value + 0.5
}
"#;

    type_check(input).expect("unannotated return should be inferred before annotated callers");
}

#[test]
fn unannotated_forward_function_call_orders_return_inference() {
    let input = r#"
fun adjusted: (value: Float64) = {
    value |> risk
}

fun risk: (value: Float64) = {
    value + 0.5
}

fun main: () -> Float64 = {
    41.5 |> adjusted
}
"#;

    type_check(input)
        .expect("unannotated forward calls should infer callee return before caller return");
}

#[test]
fn unannotated_recursive_function_requires_return_annotation() {
    let input = r#"
fun countdown: (value: Int32) = {
    value > 0 then {
        value - 1 |> countdown
    } else {
        0
    }
}

fun main: () -> Int32 = {
    3 |> countdown
}
"#;

    let err = type_check(input).expect_err("unannotated recursion should require a return type");
    assert!(
        err.contains("function 'countdown' is used before its return type has been inferred"),
        "error should explain the required return annotation, got: {}",
        err
    );
}

#[test]
fn unannotated_non_int32_cycle_diagnostic_does_not_expose_int32_placeholder() {
    let input = r#"
fun normalize: (value: Float64) = {
    value > 0.0 then {
        value |> score
    } else {
        value + 0.5
    }
}

fun score: (value: Float64) = {
    value |> normalize
}

fun main: () -> Float64 = {
    1.5 |> normalize
}
"#;

    let err = type_check(input).expect_err("unannotated cycle should require a return annotation");
    assert!(
        err.contains("return type has been inferred"),
        "error should explain the required return annotation, got: {}",
        err
    );
    assert!(
        !err.contains("Int32"),
        "provisional return diagnostics must not expose an Int32 placeholder, got: {}",
        err
    );
}

#[test]
fn unannotated_unconstrained_return_requires_annotation() {
    let input = r#"
fun empty_values: () = {
    []
}
"#;

    let err = type_check(input).expect_err("unconstrained empty list return should need context");
    assert!(
        err.contains("empty list requires an expected List type"),
        "error should explain the missing return annotation context, got: {}",
        err
    );
}

#[test]
fn unannotated_generic_function_value_return_requires_annotation() {
    let input = r#"
fun id_local: <T>(value: T) -> T = {
    value
}

fun main: () = {
    id_local
}
"#;

    let err = type_check(input)
        .expect_err("unannotated generic function value return should need context");
    assert!(
        err.contains("return type") && err.contains("explicit return annotation"),
        "error should explain the missing return annotation context, got: {}",
        err
    );
    assert!(
        !err.contains("InferVar") && !err.contains("TypeVarId") && !err.contains("?"),
        "error should not expose inference internals, got: {}",
        err
    );
}

#[test]
fn annotated_return_mismatch_is_rejected() {
    let input = r#"
fun bad: () -> Boolean = {
    1
}
"#;

    let err = type_check(input).expect_err("return annotation should be enforced");
    assert!(
        err.contains("Type mismatch"),
        "error should explain return mismatch, got: {}",
        err
    );
}

#[test]
fn generic_return_annotation_is_a_contract() {
    let input = r#"
fun bad_identity: <T>(value: T) -> T = {
    1
}
"#;

    let err = type_check(input).expect_err("generic return annotation should not be rebound");
    assert!(
        err.contains("Type mismatch"),
        "error should explain generic return mismatch, got: {}",
        err
    );
}

#[test]
fn generic_return_annotation_accepts_matching_type_param() {
    let input = r#"
fun identity: <T>(value: T) -> T = {
    value
}

fun main: () -> Int32 = {
    41 |> identity
}
"#;

    type_check(input).expect("generic return annotation should accept matching type parameter");
}
