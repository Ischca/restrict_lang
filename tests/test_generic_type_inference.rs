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
fn generic_identity_infers_from_argument() {
    let input = r#"
fun identity_local: <T>(value: T) -> T = {
    value
}

fun main: () -> Int32 = {
    42 |> identity_local
}
"#;

    type_check(input).expect("generic identity should infer T from the piped argument");
}

#[test]
fn generic_function_infers_multiple_parameters() {
    let input = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> String = {
    ("primary", "fallback") choose_first
}
"#;

    type_check(input).expect("generic call should infer a shared type parameter");
}

#[test]
fn generic_function_infers_annotated_lambda_from_bare_type_parameter() {
    let input = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun apply_once: (f: Int32 -> Int32) -> Int32 = {
    41 |> f
}

fun main: () -> Int32 = {
    val chosen = (|x: Int32| x + 1, |y: Int32| y) choose_first
    chosen |> apply_once
}
"#;

    type_check(input)
        .expect("annotated lambdas should infer a shared function type through generic arguments");
}

#[test]
fn generic_function_shapes_unannotated_lambda_from_bare_type_parameter() {
    let input = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> Int32 = {
    val chosen = (|x| x + 1, |y| y) choose_first
    41 |> chosen
}
"#;

    type_check(input).expect("bare generic T should shape unannotated lambdas as function values");
}

#[test]
fn generic_function_shapes_unannotated_lambda_inside_option() {
    let input = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> Int32 = {
    val chosen = (Some(|x| x + 1), Some(|y| y)) choose_first
    chosen match {
        Some(mapper) => { 41 |> mapper }
        None => { 0 }
    }
}
"#;

    type_check(input).expect("bare generic Option<T> should shape lambda payloads");
}

#[test]
fn generic_function_shapes_unannotated_lambda_inside_list() {
    let input = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> Int32 = {
    val chosen = ([|x| x + 1], [|y| y]) choose_first
    val mapper = (chosen, 0) list_get
    41 |> mapper
}
"#;

    type_check(input).expect("bare generic List<T> should shape lambda elements");
}

#[test]
fn prelude_map_infers_lambda_return_type() {
    let input = r#"
fun main: () -> List<Int32> = {
    val numbers: List<Int32> = [1, 2, 3]
    (numbers, |n| n * 2) map
}
"#;

    type_check(input).expect("map should infer U from the lambda body");
}

#[test]
fn prelude_map_uses_expected_return_type() {
    let input = r#"
fun main: () -> List<String> = {
    val names: List<String> = ["a", "b"]
    (names, |name| name) map
}
"#;

    type_check(input).expect("map should preserve the expected List element type");
}

#[test]
fn prelude_map_uses_container_mapped_projection_for_option() {
    let input = r#"
fun main: () -> Option<String> = {
    val maybe_score: Option<Int32> = Some(7)
    (maybe_score, |score| score > 0 then { "positive" } else { "zero" }) map
}
"#;

    type_check(input).expect("map should infer Option<Int32>.Mapped<String> as Option<String>");
}

#[test]
fn prelude_map_accepts_monomorphic_named_function_mapper() {
    let input = r#"
fun score: (value: Int32) -> Float64 = {
    value > 0 then {
        1.5
    } else {
        0.5
    }
}

fun main: () -> List<Float64> = {
    val numbers = [1, 2, 3]
    (numbers, score) map
}
"#;

    type_check(input).expect("map should accept a named monomorphic function as mapper");
}

#[test]
fn prelude_identity_can_be_used_as_expected_function_value() {
    let input = r#"
fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3]
    (numbers, identity) map
}
"#;

    type_check(input).expect("identity should instantiate from the mapper expected type");
}

#[test]
fn user_generic_function_value_instantiates_from_expected_mapper() {
    let input = r#"
fun id_local: <T>(value: T) -> T = {
    value
}

fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3]
    (numbers, id_local) map
}
"#;

    type_check(input).expect("user generic mapper should instantiate from expected function type");
}

#[test]
fn user_generic_function_value_still_requires_expected_function_type() {
    let input = r#"
fun id_local: <T>(value: T) -> T = {
    value
}

fun main: () -> Int32 = {
    val mapper = id_local;
    0
}
"#;

    let err = type_check(input).expect_err("unresolved generic function value should be rejected");
    assert!(
        err.contains("Cannot infer type") && err.contains("mapper"),
        "error should identify the unresolved generic function value, got: {}",
        err
    );
}

#[test]
fn top_level_builtin_function_value_still_requires_expected_function_type() {
    let input = r#"
val apply_map = map
"#;

    let err =
        type_check(input).expect_err("top-level unresolved builtin function value should reject");
    assert!(
        err.contains("Cannot infer type") && err.contains("apply_map"),
        "error should identify the unresolved top-level function value, got: {}",
        err
    );
    for internal in ["?0", "InferVar", "TypeVarId", "Projection"] {
        assert!(
            !err.contains(internal),
            "error should not expose inference internals ({internal}), got: {err}"
        );
    }
}

#[test]
fn local_generic_function_value_infers_from_later_map_use() {
    let input = r#"
fun id_local: <T>(value: T) -> T = {
    value
}

fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3];
    val mapper = id_local;
    (numbers, mapper) map
}
"#;

    type_check(input).expect("local generic function value should infer from later map use");
}

#[test]
fn local_generic_function_alias_chain_infers_from_later_pipe_use() {
    let input = r#"
fun id_local: <T>(value: T) -> T = {
    value
}

fun main: () -> Int32 = {
    val keep = id_local;
    val keep_again = keep;
    41 |> keep_again
}
"#;

    type_check(input).expect("generic function alias chains should infer from later pipe use");
}

#[test]
fn resolved_generic_function_alias_chain_rejects_double_use() {
    let input = r#"
fun id_local: <T>(value: T) -> T = {
    value
}

fun main: () -> Int32 = {
    val keep = id_local;
    val keep_again = keep;
    val first = 41 |> keep_again;
    val second = 42 |> keep;
    first + second
}
"#;

    let err = type_check(input)
        .expect_err("resolved non-copy function aliases should preserve affine double-use checks");
    assert!(
        err.contains("affine type violation"),
        "error should explain the affine double use, got: {err}"
    );
}

#[test]
fn then_produced_lambda_infers_from_later_pipe_use() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag then {
        |score| score + 1
    } else {
        |score| score * 2
    };
    41 |> adjust
}
"#;

    type_check(input).expect("then-produced lambda should infer from later pipe use");
}

#[test]
fn then_produced_lambda_with_prefix_binding_infers_from_later_pipe_use() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag then {
        val bonus = 1;
        |score| score + bonus
    } else {
        val factor = 2;
        |score| score * factor
    };
    41 |> adjust
}
"#;

    type_check(input).expect("then-produced lambda should infer with replay-safe prefix bindings");
}

#[test]
fn match_produced_lambda_infers_from_later_pipe_use() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag match {
        true => { |score| score + 1 }
        false => { |score| score * 2 }
    };
    41 |> adjust
}
"#;

    type_check(input).expect("match-produced lambda should infer from later pipe use");
}

#[test]
fn match_produced_lambda_can_capture_pattern_binding() {
    let input = r#"
fun main: () -> Int32 = {
    val maybe_bonus: Option<Int32> = Some(2);
    val adjust = maybe_bonus match {
        Some(bonus) => { |score| score + bonus }
        None => { |score| score }
    };
    40 |> adjust
}
"#;

    type_check(input).expect("match-produced lambda should capture pattern bindings during replay");
}

#[test]
fn match_produced_lambda_with_prefix_binding_captures_pattern_binding() {
    let input = r#"
fun main: () -> Int32 = {
    val maybe_bonus: Option<Int32> = Some(2);
    val adjust = maybe_bonus match {
        Some(bonus) => {
            val doubled = bonus * 2;
            |score| score + doubled
        }
        None => {
            val doubled = 0;
            |score| score + doubled
        }
    };
    38 |> adjust
}
"#;

    type_check(input).expect(
        "match-produced lambda should allow replay-safe prefix bindings from pattern captures",
    );
}

#[test]
fn then_produced_mapper_infers_from_later_map_use() {
    let input = r#"
fun main: (flag: Boolean) -> List<Int32> = {
    val numbers = [1, 2, 3];
    val mapper = flag then {
        |score| score + 1
    } else {
        |score| score * 2
    };
    (numbers, mapper) map
}
"#;

    type_check(input).expect("then-produced mapper should infer from later map use");
}

#[test]
fn branch_produced_lambda_with_prefix_return_mismatch_is_rejected() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag then {
        val bonus = 1;
        |score| score + bonus
    } else {
        val positive = true;
        |score| score > 0
    };
    41 |> adjust
}
"#;

    let err = type_check(input)
        .expect_err("branch lambda return mismatch with prefix bindings should reject");
    assert!(
        err.contains("Type mismatch"),
        "error should explain the return mismatch, got: {err}"
    );
}

#[test]
fn branch_produced_lambda_return_mismatch_is_rejected() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag then {
        |score| score + 1
    } else {
        |score| score > 0
    };
    41 |> adjust
}
"#;

    let err = type_check(input).expect_err("branch lambda return mismatch should reject");
    assert!(
        err.contains("Type mismatch"),
        "error should explain the return mismatch, got: {err}"
    );
}

#[test]
fn branch_produced_lambda_rejects_non_copy_prefix_binding() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag then {
        val label = "release";
        |score| score
    } else {
        |score| score
    };
    41 |> adjust
}
"#;

    let err = type_check(input)
        .expect_err("deferred callable branch prefix should reject non-Copy bindings");
    assert!(
        err.contains("Copy type"),
        "error should explain the Copy boundary, got: {err}"
    );
}

#[test]
fn branch_produced_lambda_rejects_mutable_prefix_binding() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag then {
        mut val bonus = 1;
        |score| score + bonus
    } else {
        |score| score
    };
    41 |> adjust
}
"#;

    let err = type_check(input)
        .expect_err("deferred callable branch prefix should reject mutable bindings");
    assert!(
        err.contains("cannot be mutable"),
        "error should explain the mutable prefix boundary, got: {err}"
    );
}

#[test]
fn branch_produced_lambda_rejects_complex_pattern_prefix_binding() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag then {
        val Some(bonus): Option<Int32> = Some(1);
        |score| score + bonus
    } else {
        |score| score
    };
    41 |> adjust
}
"#;

    let err = type_check(input)
        .expect_err("deferred callable branch prefix should reject complex patterns");
    assert!(
        err.contains("simple identifiers"),
        "error should explain the simple identifier prefix boundary, got: {err}"
    );
}

#[test]
fn branch_produced_lambda_arity_mismatch_is_rejected() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag then {
        |score| score
    } else {
        |score, fallback| score + fallback
    };
    41 |> adjust
}
"#;

    let err = type_check(input).expect_err("branch lambda arity mismatch should reject");
    assert!(
        err.contains("Arity mismatch") || err.contains("Wrong number of arguments"),
        "error should explain the arity mismatch, got: {err}"
    );
}

#[test]
fn unused_branch_produced_lambda_still_requires_expected_type() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag then {
        |score| score
    } else {
        |score| score
    };
    0
}
"#;

    let err = type_check(input)
        .expect_err("unresolved branch-produced lambda should reject at scope exit");
    assert!(
        err.contains("Cannot infer type") && err.contains("adjust"),
        "error should identify the unresolved deferred binding, got: {err}"
    );
}

#[test]
fn tail_infers_list_element_type_generically() {
    let input = r#"
fun main: () -> List<String> = {
    val names: List<String> = ["Ada", "Grace", "Edsger"];
    names |> tail
}
"#;

    type_check(input).expect("tail should preserve the concrete List element type");
}

#[test]
fn prelude_map_rejects_non_container_input() {
    let input = r#"
fun main: () -> String = {
    ("Ada", |name| name) map
}
"#;

    let err = type_check(input).expect_err("map should require a Container input");
    assert!(
        err.contains("Container"),
        "error should explain the missing Container form, got: {}",
        err
    );
}

#[test]
fn prelude_map_rejects_result_as_non_container_input() {
    let input = r#"
fun main: () -> Result<Int32, Int32> = {
    val result: Result<Int32, Int32> = Ok(1);
    (result, |value| value + 1) map
}
"#;

    let err = type_check(input).expect_err("Result is not a v0.0.1 Container");
    assert!(
        err.contains("Container"),
        "error should explain the closed-world Container boundary, got: {err}"
    );
}

#[test]
fn prelude_map_rejects_array_as_non_container_input() {
    let input = r#"
fun main: () -> Array<Int32, 2> = {
    val values: Array<Int32, 2> = [1, 2];
    (values, |value| value + 1) map
}
"#;

    let err = type_check(input).expect_err("Array is not a v0.0.1 Container");
    assert!(
        err.contains("Container"),
        "error should explain the closed-world Container boundary, got: {err}"
    );
}

#[test]
fn prelude_filter_uses_boolean_predicate_context() {
    let input = r#"
fun main: () -> List<Int32> = {
    val numbers: List<Int32> = [1, 2, 3]
    (numbers, |n| n > 1) filter
}
"#;

    type_check(input).expect("filter should infer T and require a Boolean predicate");
}

#[test]
fn prelude_filter_uses_container_item_projection_for_option() {
    let input = r#"
fun main: () -> Option<Int32> = {
    val maybe_value: Option<Int32> = Some(42)
    (maybe_value, |value| value > 10) filter
}
"#;

    type_check(input).expect("filter should infer Option.Item through Container projection");
}

#[test]
fn prelude_filter_rejects_non_boolean_predicate() {
    let input = r#"
fun main: () -> List<Int32> = {
    val numbers: List<Int32> = [1, 2, 3]
    (numbers, |n| n + 1) filter
}
"#;

    let err = type_check(input).expect_err("filter predicate should return Boolean");
    assert!(
        err.contains("Type mismatch"),
        "error should explain predicate return mismatch, got: {}",
        err
    );
}

#[test]
fn prelude_fold_infers_accumulator_and_item_types() {
    let input = r#"
fun main: () -> Int32 = {
    val numbers: List<Int32> = [1, 2, 3]
    (numbers, 0, |acc, n| acc + n) fold
}
"#;

    type_check(input).expect("fold should infer both accumulator and item types");
}

#[test]
fn prelude_fold_rejects_reducer_return_mismatch() {
    let input = r#"
fun main: () -> Int32 = {
    val numbers: List<Int32> = [1, 2, 3]
    (numbers, 0, |acc, n| n > acc) fold
}
"#;

    let err = type_check(input).expect_err("fold reducer should return the accumulator type");
    assert!(
        err.contains("Type mismatch"),
        "error should explain reducer return mismatch, got: {}",
        err
    );
}

#[test]
fn generic_lambda_capture_precheck_does_not_consume_affine_value() {
    let input = r#"
fun apply_generic: <T, U>(value: T, f: T -> U) -> U = {
    value |> f
}

fun main: () -> String = {
    val greeting = "hello";
    (1, |n| greeting) apply_generic
}
"#;

    type_check(input).expect("generic lambda pre-check should not consume captured affine values");
}

#[test]
fn generic_lambda_can_appear_before_inference_source() {
    let input = r#"
fun apply_first: <T, U>(f: T -> U, value: T) -> U = {
    value |> f
}

fun main: () -> Int32 = {
    (|n| n + 1, 41) apply_first
}
"#;

    type_check(input).expect("generic inference should not depend on lambda argument order");
}

#[test]
fn generic_empty_list_infers_from_sibling_argument() {
    let input = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> List<Int32> = {
    ([], [1, 2, 3]) choose_first
}
"#;

    type_check(input).expect("empty list should infer through generic sibling constraints");
}

#[test]
fn generic_none_infers_from_sibling_argument() {
    let input = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> Option<Int32> = {
    (None, Some(1)) choose_first
}
"#;

    type_check(input).expect("None should infer through generic sibling constraints");
}

#[test]
fn generic_list_elements_infer_from_sibling_constructors() {
    let input = r#"
fun keep_list: <T>(items: List<T>) -> List<T> = {
    items
}

fun main: () -> List<Option<Int32>> = {
    [None, Some(1)] |> keep_list
}
"#;

    type_check(input).expect("generic list element inference should use sibling constructors");
}

#[test]
fn array_get_accepts_any_length_array_parameter() {
    let input = r#"
fun main: () -> Option<Int32> = {
    ([None, Some(1)], 0) array_get
}
"#;

    type_check(input)
        .expect("internal array wildcard parameters should accept any concrete array length");
}

#[test]
fn generic_lambda_return_mismatch_is_rejected() {
    let input = r#"
fun main: () -> List<String> = {
    val numbers: List<Int32> = [1, 2, 3]
    (numbers, |n| n * 2) map
}
"#;

    let err = type_check(input).expect_err("expected return type should constrain map result");
    assert!(
        err.contains("Type mismatch"),
        "error should explain type mismatch, got: {}",
        err
    );
}
