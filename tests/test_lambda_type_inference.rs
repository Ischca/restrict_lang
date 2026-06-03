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
fn lambda_uses_function_argument_context() {
    let input = r#"
fun apply_to_int: (f: Int32 -> Int32, x: Int32) -> Int32 = {
    x |> f
}

fun main: () -> Int32 = {
    (|x| x + 1, 41) apply_to_int
}
"#;

    type_check(input).expect("lambda should infer parameter type from function argument context");
}

#[test]
fn lambda_uses_local_type_annotation_context() {
    let input = r#"
fun main: () -> Int32 = {
    val increment: Int32 -> Int32 = |x| x + 1;
    41 |> increment
}
"#;

    type_check(input).expect("lambda should infer parameter type from annotated binding");
}

#[test]
fn annotated_contextless_lambda_infers_function_type() {
    let input = r#"
fun main: () -> Int32 = {
    val increment = |x: Int32| x + 1;
    41 |> increment
}
"#;

    type_check(input).expect("fully annotated lambda should infer without external context");
}

#[test]
fn contextless_lambda_body_constraints_infer_function_type() {
    let input = r#"
fun main: () -> Int32 = {
    val add = |left, right| left + right;
    (20, 22) add
}
"#;

    type_check(input).expect("lambda body constraints should infer a concrete function type");
}

#[test]
fn local_deferred_lambda_infers_from_later_direct_use() {
    let input = r#"
fun main: () -> Int32 = {
    val choose_left = |left, right| left;
    (41, 0) choose_left
}
"#;

    type_check(input).expect("later direct use should resolve a deferred local lambda");
}

#[test]
fn annotated_lambda_param_mismatch_is_rejected() {
    let input = r#"
fun main: () -> Int32 = {
    val bad: Boolean -> Int32 = |x: Int32| x + 1;
    true |> bad
}
"#;

    let err = type_check(input).expect_err("lambda parameter annotation should match context");
    assert!(
        err.contains("Type mismatch"),
        "error should explain type mismatch, got: {}",
        err
    );
}

#[test]
fn partial_lambda_annotation_uses_expected_context() {
    let input = r#"
fun main: () -> Int32 = {
    val add: (Int32, Int32) -> Int32 = |x: Int32, y| x + y;
    (10, 20) add
}
"#;

    type_check(input).expect("expected function type should fill unannotated lambda params");
}

#[test]
fn immediate_lambda_pipe_infers_from_argument() {
    let input = r#"
fun main: () -> Int32 = {
    41 |> (|x| x + 1)
}
"#;

    type_check(input).expect("pipe to an immediate lambda should infer from the piped argument");
}

#[test]
fn immediate_lambda_pipe_uses_expected_return() {
    let input = r#"
fun main: () -> String = {
    41 |> (|x| "ready")
}
"#;

    type_check(input).expect("pipe to an immediate lambda should use the expected return type");
}

#[test]
fn immediate_lambda_pipe_infers_empty_list_from_expected_return() {
    let input = r#"
fun main: () -> List<Int32> = {
    [] |> (|xs| xs)
}
"#;

    type_check(input).expect("empty list should infer through immediate lambda return context");
}

#[test]
fn immediate_lambda_pipe_infers_none_from_expected_return() {
    let input = r#"
fun main: () -> Option<Int32> = {
    None |> (|value| value)
}
"#;

    type_check(input).expect("None should infer through immediate lambda return context");
}

#[test]
fn immediate_lambda_pipe_infers_nested_empty_list_from_expected_return() {
    let input = r#"
fun main: () -> Option<List<Int32>> = {
    Some([]) |> (|value| value)
}
"#;

    type_check(input).expect("nested empty list should infer through immediate lambda context");
}

#[test]
fn immediate_lambda_pipe_uses_generic_body_to_constrain_empty_list_argument() {
    let input = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> List<Int32> = {
    val fallback = [1, 2];
    [] |> (|empty| (empty, fallback) choose_first)
}
"#;

    type_check(input).expect("generic lambda body should constrain the empty piped argument");
}

#[test]
fn immediate_lambda_pipe_uses_generic_body_to_constrain_none_argument() {
    let input = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> Option<Int32> = {
    val fallback = Some(1);
    None |> (|empty| (empty, fallback) choose_first)
}
"#;

    type_check(input).expect("generic lambda body should constrain the None piped argument");
}

#[test]
fn immediate_lambda_pipe_rejects_unconstrained_empty_argument() {
    let input = r#"
fun main: () -> Int32 = {
    [] |> (|xs| 1)
}
"#;

    let err = type_check(input).expect_err("ignored empty argument should remain ambiguous");
    assert!(
        err.contains("Cannot infer type"),
        "error should explain inference failure, got: {}",
        err
    );
}

#[test]
fn immediate_lambda_pipe_rejects_return_mismatch() {
    let input = r#"
fun main: () -> String = {
    41 |> (|x| x + 1)
}
"#;

    let err = type_check(input).expect_err("expected return should constrain immediate lambda");
    assert!(
        err.contains("Type mismatch"),
        "error should explain type mismatch, got: {}",
        err
    );
}

#[test]
fn lambda_uses_boolean_return_context() {
    let input = r#"
fun apply_predicate: (f: Int32 -> Boolean, x: Int32) -> Boolean = {
    x |> f
}

fun main: () -> Boolean = {
    (|x| x > 0, 42) apply_predicate
}
"#;

    type_check(input).expect("lambda should infer Boolean result from expected function type");
}

#[test]
fn lambda_uses_float_context() {
    let input = r#"
fun apply_float: (f: Float64 -> Float64, x: Float64) -> Float64 = {
    x |> f
}

fun main: () -> Float64 = {
    (|x| x + 3.14, 1.0) apply_float
}
"#;

    type_check(input).expect("lambda should infer Float64 parameter from expected function type");
}

#[test]
fn lambda_uses_multi_parameter_context() {
    let input = r#"
fun apply_two: (f: (Int32, Int32) -> Int32, a: Int32, b: Int32) -> Int32 = {
    (a, b) f
}

fun main: () -> Int32 = {
    (|x, y| x + y, 10, 20) apply_two
}
"#;

    type_check(input)
        .expect("lambda should infer multiple parameter types from expected function type");
}

#[test]
fn lambda_capture_precheck_does_not_consume_affine_value() {
    let input = r#"
fun apply_to_int_string: (f: Int32 -> String, x: Int32) -> String = {
    x |> f
}

fun main: () -> String = {
    val greeting = "hello";
    (|x| greeting, 1) apply_to_int_string
}
"#;

    type_check(input).expect("lambda pre-check should not consume captured affine values");
}

#[test]
fn contextless_lambda_is_rejected() {
    let input = r#"
fun main: () -> Int32 = {
    val identity = |x| x;
    41
}
"#;

    let err = type_check(input).expect_err("unresolved lambda type should fail at scope exit");
    assert!(
        err.contains("Cannot infer type") && err.contains("identity"),
        "error should explain inference failure, got: {}",
        err
    );
}

#[test]
fn contextless_lambda_reports_body_type_errors_immediately() {
    let input = r#"
fun main: () -> Int32 = {
    val bad = |x| x + true;
    41
}
"#;

    let err = type_check(input).expect_err("body type error should not be hidden by deferral");
    assert!(
        err.contains("Type mismatch"),
        "error should report the body mismatch, got: {}",
        err
    );
    assert!(
        !err.contains("unresolved deferred type"),
        "body mismatch should not be replaced by a later deferred-type error, got: {}",
        err
    );
    assert!(
        !err.contains("InferVar") && !err.contains("TypeVarId") && !err.contains("Projection"),
        "error should not expose inference internals, got: {}",
        err
    );
}

#[test]
fn contextless_fully_annotated_lambda_succeeds() {
    let input = r#"
fun main: () -> Int32 = {
    val add_one = |x: Int32| x + 1;
    41 |> add_one
}
"#;

    type_check(input).expect("fully annotated lambda should not require expected context");
}

#[test]
fn partially_annotated_lambda_uses_expected_function_type() {
    let input = r#"
fun apply_two: (f: (Int32, Int32) -> Int32, a: Int32, b: Int32) -> Int32 = {
    (a, b) f
}

fun main: () -> Int32 = {
    (|x: Int32, y| x + y, 10, 20) apply_two
}
"#;

    type_check(input).expect("expected function type should fill unannotated lambda params");
}

#[test]
fn annotated_lambda_expected_function_type_mismatch_is_rejected() {
    let input = r#"
fun apply_string: (f: String -> Int32, value: String) -> Int32 = {
    value |> f
}

fun main: () -> Int32 = {
    (|x: Int32| x + 1, "hello") apply_string
}
"#;

    let err = type_check(input).expect_err("lambda annotation should match expected function type");
    assert!(
        err.contains("Type mismatch"),
        "error should explain type mismatch, got: {}",
        err
    );
}

#[test]
fn annotated_lambda_return_mismatch_is_rejected() {
    let input = r#"
fun main: () -> Int32 = {
    val predicate: Int32 -> Int32 = |x| x > 0;
    41 |> predicate
}
"#;

    let err = type_check(input).expect_err("annotated lambda return type should be checked");
    assert!(
        err.contains("Type mismatch"),
        "error should explain type mismatch, got: {}",
        err
    );
}
