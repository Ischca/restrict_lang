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
fn local_empty_list_infers_item_type_from_fold_use() {
    let input = r#"
fun main: () -> Int32 = {
    val xs = [];
    (xs, 0, |total, item| total + item) fold
}
"#;

    type_check(input)
        .expect("local [] should infer List<Int32> from fold item and accumulator use");
}

#[test]
fn local_none_infers_payload_type_from_unwrap_or_use() {
    let input = r#"
fun unwrap_or: <T>(maybe: Option<T>, fallback: T) -> T = {
    maybe match {
        Some(value) => { value }
        None => { fallback }
    }
}

fun main: () -> Int32 = {
    val maybe = None;
    (maybe, 0) unwrap_or
}
"#;

    type_check(input).expect("local None should infer Option<Int32> from unwrap_or fallback");
}

#[test]
fn mutable_local_none_infers_payload_type_from_later_use() {
    let input = r#"
fun unwrap_or: <T>(maybe: Option<T>, fallback: T) -> T = {
    maybe match {
        Some(value) => { value }
        None => { fallback }
    }
}

fun main: () -> Int32 = {
    mut val maybe = None;
    (maybe, 7) unwrap_or
}
"#;

    type_check(input).expect("mutable local None should infer Option<Int32> from later use");
}

#[test]
fn mutable_int64_assignment_uses_target_type() {
    let input = r#"
fun main: () -> Int64 = {
    mut val score: Int64 = 0;
    score = 1;
    score
}
"#;

    type_check(input).expect("assignment RHS should use the mutable target Int64 type");
}

#[test]
fn mutable_option_assignment_uses_target_type_for_none() {
    let input = r#"
fun unwrap_or: <T>(maybe: Option<T>, fallback: T) -> T = {
    maybe match {
        Some(value) => { value }
        None => { fallback }
    }
}

fun main: () -> Int32 = {
    mut val maybe: Option<Int32> = Some(1);
    maybe = None;
    (maybe, 0) unwrap_or
}
"#;

    type_check(input).expect("None assignment should use the mutable target Option type");
}

#[test]
fn mutable_empty_list_assignment_uses_target_type() {
    let input = r#"
fun main: () -> List<Int32> = {
    mut val reviewers: List<Int32> = [1, 2, 3];
    reviewers = [];
    reviewers
}
"#;

    type_check(input).expect("[] assignment should use the mutable target List type");
}

#[test]
fn local_range_literal_uses_return_context() {
    let input = r#"
fun main: () -> Range<Int32> = {
    val launch_window = [1..4];
    launch_window
}
"#;

    type_check(input).expect("local range literal should use the Range<Int32> return context");
}

#[test]
fn local_range_literal_uses_later_function_context() {
    let input = r#"
fun accept_range: (window: Range<Int32>) -> Int32 = {
    1
}

fun main: () -> Int32 = {
    val window = [1..4];
    window |> accept_range
}
"#;

    type_check(input).expect("local range literal should use later Range<Int32> call context");
}

#[test]
fn local_array_literal_uses_expected_return_context() {
    let input = r#"
fun main: () -> Array<Option<Int32>, 2> = {
    val reviewers = [None, Some(42)];
    reviewers
}
"#;

    type_check(input).expect("local array literal should use expected Array<Option<Int32>, 2>");
}

#[test]
fn local_empty_array_infers_from_later_array_get_use() {
    let input = r#"
fun main: () -> Option<Int32> = {
    val reviewers = [];
    (reviewers, 0) array_get
}
"#;

    type_check(input)
        .expect("local [] should infer an Option<Int32> array from array_get and return context");
}

#[test]
fn local_non_empty_array_infers_from_later_array_get_use() {
    let input = r#"
fun main: () -> Int32 = {
    val reviewers = [41, 42];
    (reviewers, 0) array_get
}
"#;

    type_check(input)
        .expect("unannotated local list literal should become an array from array_get");
}

#[test]
fn local_empty_array_infers_from_later_array_set_value() {
    let input = r#"
fun main: () -> () = {
    val reviewers = [];
    (reviewers, 0, Some(42)) array_set
}
"#;

    type_check(input)
        .expect("local [] should infer its Array payload type from a later array_set value");
}

#[test]
fn local_non_empty_array_infers_from_later_array_set_value() {
    let input = r#"
fun main: () -> () = {
    val reviewers = [1, 2];
    (reviewers, 0, 42) array_set
}
"#;

    type_check(input)
        .expect("unannotated local list literal should become an array from array_set");
}

#[test]
fn local_generic_record_result_list_infers_from_later_match_use() {
    let input = r#"
record Box<T> {
    result: Result<List<T>, String>
}

fun use_i64s: (values: List<Int64>) -> Int64 = {
    0
}

fun main: () -> Int64 = {
    val box = Box {
        result: Ok([1])
    };
    box.result match {
        Ok(values) => { values |> use_i64s }
        Err(message) => { 0 }
    }
}
"#;

    type_check(input)
        .expect("generic record Result<List<T>> should infer T from later match payload use");
}

#[test]
fn local_generic_record_infers_from_generic_call_sibling_argument() {
    let input = r#"
record Box<T> {
    value: T
}

fun unwrap_box: <T>(box: Box<T>, fallback: T) -> T = {
    box.value
}

fun main: () -> List<Int32> = {
    val box = Box { value: [] };
    (box, [1, 2]) unwrap_box
}
"#;

    type_check(input)
        .expect("generic record local should infer from sibling generic call arguments");
}

#[test]
fn annotated_list_does_not_become_array_from_later_use() {
    let input = r#"
fun main: () -> Int32 = {
    val reviewers: List<Int32> = [41, 42];
    (reviewers, 0) array_get
}
"#;

    let err = type_check(input).expect_err("explicit List annotation should not be retargeted");
    assert!(
        err.contains("Type mismatch") && err.contains("Array"),
        "error should preserve the explicit List boundary, got: {err}"
    );
}

#[test]
fn local_empty_array_annotation_rejects_nonzero_public_length() {
    let input = r#"
fun main: () -> Int32 = {
    val reviewers: Array<Option<Int32>, 2> = [];
    0
}
"#;

    let err = type_check(input)
        .expect_err("empty array literal should not satisfy a nonzero public Array length");
    assert!(
        err.contains("Array<Option<Int32>, 2>") && err.contains("Array<Option<Int32>, 0>"),
        "error should compare concrete public array lengths, got: {err}"
    );
}

#[test]
fn local_empty_array_can_be_explicit_zero_length() {
    let input = r#"
fun main: () -> Array<Option<Int32>, 0> = {
    val reviewers: Array<Option<Int32>, 0> = [];
    reviewers
}
"#;

    type_check(input).expect("explicit public Array<T, 0> should type-check as zero-length");
}

#[test]
fn local_builtin_array_wildcard_cannot_escape_as_public_array_zero() {
    let input = r#"
fun main: () -> Array<Option<Int32>, 1> = {
    val reviewers = [];
    val first = (reviewers, 0) array_get;
    val selected = reviewers;
    selected
}
"#;

    let err = type_check(input)
        .expect_err("internal array wildcard should not escape as a public array length");
    assert!(
        !err.contains("Array<Option<Int32>, 0>"),
        "error should not expose the internal wildcard as public Array<T, 0>, got: {err}"
    );
}

#[test]
fn local_ok_and_err_infer_result_type_from_later_use() {
    let input = r#"
fun result_value_or_zero: (result: Result<Int32, String>) -> Int32 = {
    result match {
        Ok(value) => {
            value
        }
        Err(error) => {
            0
        }
    }
}

fun main: () -> Int32 = {
    val ok_result = Ok(41);
    val err_result = Err("missing");
    (ok_result |> result_value_or_zero) + (err_result |> result_value_or_zero)
}
"#;

    type_check(input).expect("local Ok/Err should infer full Result type from later use");
}

#[test]
fn local_result_list_payload_infers_from_later_match_use() {
    let input = r#"
fun first_i64: (values: List<Int64>) -> Int64 = {
    (values, 0) list_get
}

fun error_score: (message: String) -> Int64 = {
    0
}

fun main: () -> Int64 = {
    val result = Ok([1]);
    result match {
        Ok(values) => { values |> first_i64 }
        Err(message) => { message |> error_score }
    }
}
"#;

    type_check(input)
        .expect("local Result<List<T>, E> should infer List<Int64> from match payload use");
}

#[test]
fn result_copyable_after_inference_can_be_copied_before_first_constraint() {
    let input = r#"
fun result_value_or_code: (result: Result<Int32, Int32>) -> Int32 = {
    result match {
        Ok(value) => {
            value
        }
        Err(code) => {
            code
        }
    }
}

fun main: () -> Int32 = {
    val result = Ok(41);
    val first = result;
    val second = result;
    (first |> result_value_or_code) + (second |> result_value_or_code)
}
"#;

    type_check(input).expect("Result<Int32, Int32> should become copyable after inference");
}

#[test]
fn result_noncopy_after_inference_rejects_multiple_pending_uses() {
    let input = r#"
fun result_value_or_code: (result: Result<String, Int32>) -> Int32 = {
    result match {
        Ok(value) => {
            1
        }
        Err(code) => {
            code
        }
    }
}

fun main: () -> Int32 = {
    val result = Ok("ready");
    val first = result;
    val second = result;
    (first |> result_value_or_code) + (second |> result_value_or_code)
}
"#;

    let err = type_check(input).expect_err("Result<String, Int32> should remain affine");
    assert!(
        err.contains("affine") || err.contains("already been used"),
        "expected affine rejection for non-copy inferred Result, got: {err}"
    );
}

#[test]
fn result_noncopy_due_err_side_rejects_multiple_pending_uses() {
    let input = r#"
fun result_value_or_zero: (result: Result<Int32, String>) -> Int32 = {
    result match {
        Ok(value) => {
            value
        }
        Err(error) => {
            0
        }
    }
}

fun main: () -> Int32 = {
    val result = Ok(41);
    val first = result;
    val second = result;
    (first |> result_value_or_zero) + (second |> result_value_or_zero)
}
"#;

    let err = type_check(input).expect_err("Result<Int32, String> should remain affine");
    assert!(
        err.contains("affine") || err.contains("already been used"),
        "expected affine rejection for non-copy inferred Result error side, got: {err}"
    );
}

#[test]
fn err_result_noncopy_due_ok_side_rejects_multiple_pending_uses() {
    let input = r#"
fun result_value_or_code: (result: Result<String, Int32>) -> Int32 = {
    result match {
        Ok(value) => {
            1
        }
        Err(code) => {
            code
        }
    }
}

fun main: () -> Int32 = {
    val result = Err(7);
    val first = result;
    val second = result;
    (first |> result_value_or_code) + (second |> result_value_or_code)
}
"#;

    let err = type_check(input).expect_err("Result<String, Int32> should remain affine");
    assert!(
        err.contains("affine") || err.contains("already been used"),
        "expected affine rejection for non-copy inferred Result ok side, got: {err}"
    );
}

#[test]
fn result_copyable_after_branch_constraint_can_be_reused() {
    let input = r#"
fun result_value_or_code: (result: Result<Int32, Int32>) -> Int32 = {
    result match {
        Ok(value) => {
            value
        }
        Err(code) => {
            code
        }
    }
}

fun main: () -> Int32 = {
    val flag: Boolean = true;
    val result = Ok(41);
    val chosen = flag then {
        result |> result_value_or_code
    } else {
        0
    };
    chosen + (result |> result_value_or_code)
}
"#;

    type_check(input).expect("branch-constrained Result<Int32, Int32> should become copyable");
}

#[test]
fn result_noncopy_after_branch_constraint_rejects_reuse() {
    let input = r#"
fun result_value_or_code: (result: Result<String, Int32>) -> Int32 = {
    result match {
        Ok(value) => {
            1
        }
        Err(code) => {
            code
        }
    }
}

fun main: () -> Int32 = {
    val flag: Boolean = true;
    val result = Ok("ready");
    val chosen = flag then {
        result |> result_value_or_code
    } else {
        0
    };
    chosen + (result |> result_value_or_code)
}
"#;

    let err = type_check(input).expect_err("branch-constrained Result<String, Int32> is affine");
    assert!(
        err.contains("affine") || err.contains("already been used"),
        "expected affine rejection for branch-constrained non-copy Result, got: {err}"
    );
}

#[test]
fn result_copyable_after_pending_then_branch_result_can_be_reused() {
    let input = r#"
fun result_value_or_code: (result: Result<Int32, Int32>) -> Int32 = {
    result match {
        Ok(value) => {
            value
        }
        Err(code) => {
            code
        }
    }
}

fun main: () -> Int32 = {
    val flag: Boolean = true;
    val result = Ok(41);
    val selected = flag then {
        result
    } else {
        Err(7)
    };
    val first = selected |> result_value_or_code;
    first + (result |> result_value_or_code)
}
"#;

    type_check(input).expect("branch-pending Result<Int32, Int32> should become copyable");
}

#[test]
fn result_noncopy_after_pending_then_branch_result_rejects_reuse() {
    let input = r#"
fun result_value_or_code: (result: Result<String, Int32>) -> Int32 = {
    result match {
        Ok(value) => {
            1
        }
        Err(code) => {
            code
        }
    }
}

fun main: () -> Int32 = {
    val flag: Boolean = true;
    val result = Ok("ready");
    val selected = flag then {
        result
    } else {
        Err(7)
    };
    val first = selected |> result_value_or_code;
    first + (result |> result_value_or_code)
}
"#;

    let err = type_check(input).expect_err("branch-pending Result<String, Int32> is affine");
    assert!(
        err.contains("affine") || err.contains("already been used"),
        "expected affine rejection for branch-pending non-copy Result, got: {err}"
    );
}

#[test]
fn result_copyable_after_pending_match_arm_result_can_be_reused() {
    let input = r#"
fun result_value_or_code: (result: Result<Int32, Int32>) -> Int32 = {
    result match {
        Ok(value) => {
            value
        }
        Err(code) => {
            code
        }
    }
}

fun main: () -> Int32 = {
    val flag: Boolean = true;
    val result = Ok(41);
    val selected = flag match {
        true => { result }
        false => { Err(7) }
    };
    val first = selected |> result_value_or_code;
    first + (result |> result_value_or_code)
}
"#;

    type_check(input).expect("match-pending Result<Int32, Int32> should become copyable");
}

#[test]
fn result_noncopy_after_pending_match_arm_result_rejects_reuse() {
    let input = r#"
fun result_value_or_code: (result: Result<String, Int32>) -> Int32 = {
    result match {
        Ok(value) => {
            1
        }
        Err(code) => {
            code
        }
    }
}

fun main: () -> Int32 = {
    val flag: Boolean = true;
    val result = Ok("ready");
    val selected = flag match {
        true => { result }
        false => { Err(7) }
    };
    val first = selected |> result_value_or_code;
    first + (result |> result_value_or_code)
}
"#;

    let err = type_check(input).expect_err("match-pending Result<String, Int32> is affine");
    assert!(
        err.contains("affine") || err.contains("already been used"),
        "expected affine rejection for match-pending non-copy Result, got: {err}"
    );
}

#[test]
fn result_then_sibling_constructor_constrains_pending_ok_binding_error_side() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val result = Ok(41);
    val selected = flag then {
        result
    } else {
        Err("missing")
    };
    selected match {
        Ok(value) => {
            value
        }
        Err(error) => {
            0
        }
    }
}
"#;

    type_check(input).expect("sibling Err branch should constrain pending Ok binding error type");
}

#[test]
fn result_match_sibling_constructor_constrains_pending_ok_binding_error_side() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val result = Ok(41);
    val selected = flag match {
        true => {
            result
        }
        false => {
            Err("missing")
        }
    };
    selected match {
        Ok(value) => {
            value
        }
        Err(error) => {
            0
        }
    }
}
"#;

    type_check(input)
        .expect("sibling Err match arm should constrain pending Ok binding error type");
}

#[test]
fn none_then_direct_match_constrains_pending_option_payload() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val maybe = None;
    val selected = flag then {
        maybe
    } else {
        Some(1)
    };
    selected match {
        Some(value) => {
            value
        }
        None => {
            1
        }
    }
}
"#;

    type_check(input).expect("sibling Some branch should constrain pending None payload");
}

#[test]
fn none_match_direct_match_constrains_pending_option_payload() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val maybe = None;
    val selected = flag match {
        true => {
            maybe
        }
        false => {
            Some(1)
        }
    };
    selected match {
        Some(value) => {
            value
        }
        None => {
            1
        }
    }
}
"#;

    type_check(input).expect("sibling Some match arm should constrain pending None payload");
}

#[test]
fn none_copyable_after_inference_can_be_copied_before_first_constraint() {
    let input = r#"
fun unwrap_or: <T>(maybe: Option<T>, fallback: T) -> T = {
    maybe match {
        Some(value) => { value }
        None => { fallback }
    }
}

fun main: () -> Int32 = {
    val maybe = None;
    val first = maybe;
    val second = maybe;
    val a = (first, 1) unwrap_or;
    a + ((second, 2) unwrap_or)
}
"#;

    type_check(input).expect("Option<Int32> should become copyable after inference resolves it");
}

#[test]
fn none_copyable_after_branch_inference_can_be_reused() {
    let input = r#"
fun unwrap_or: <T>(maybe: Option<T>, fallback: T) -> T = {
    maybe match {
        Some(value) => { value }
        None => { fallback }
    }
}

fun main: () -> Int32 = {
    val flag: Boolean = true;
    val maybe = None;
    val chosen = flag then {
        (maybe, 1) unwrap_or
    } else {
        0
    };
    chosen + ((maybe, 2) unwrap_or)
}
"#;

    type_check(input).expect("branch-local Option<Int32> inference should not force a move");
}

#[test]
fn none_copyable_after_pending_then_branch_result_can_be_reused() {
    let input = r#"
fun unwrap_or: <T>(maybe: Option<T>, fallback: T) -> T = {
    maybe match {
        Some(value) => { value }
        None => { fallback }
    }
}

fun main: () -> Int32 = {
    val flag: Boolean = true;
    val maybe = None;
    val selected = flag then {
        maybe
    } else {
        None
    };
    val first = (selected, 1) unwrap_or;
    first + ((maybe, 2) unwrap_or)
}
"#;

    type_check(input).expect("pending branch use should clear when Option<Int32> is copyable");
}

#[test]
fn none_noncopy_after_pending_then_branch_result_rejects_reuse() {
    let input = r#"
fun unwrap_or: <T>(maybe: Option<T>, fallback: T) -> T = {
    maybe match {
        Some(value) => { value }
        None => { fallback }
    }
}

fun main: () -> String = {
    val flag: Boolean = true;
    val maybe = None;
    val selected = flag then {
        maybe
    } else {
        None
    };
    val first = (selected, "first") unwrap_or;
    first + ((maybe, "second") unwrap_or)
}
"#;

    let err =
        type_check(input).expect_err("pending branch use should move inferred Option<String>");
    assert!(
        err.contains("affine") || err.contains("already been used"),
        "expected affine rejection for branch-pending non-copy option, got: {err}"
    );
}

#[test]
fn none_noncopy_after_pending_match_arm_result_rejects_reuse() {
    let input = r#"
fun unwrap_or: <T>(maybe: Option<T>, fallback: T) -> T = {
    maybe match {
        Some(value) => { value }
        None => { fallback }
    }
}

fun main: () -> String = {
    val flag: Boolean = true;
    val maybe = None;
    val selected = flag match {
        true => { maybe }
        false => { None }
    };
    val first = (selected, "first") unwrap_or;
    first + ((maybe, "second") unwrap_or)
}
"#;

    let err = type_check(input).expect_err("pending match use should move inferred Option<String>");
    assert!(
        err.contains("affine") || err.contains("already been used"),
        "expected affine rejection for match-pending non-copy option, got: {err}"
    );
}

#[test]
fn none_inferred_as_noncopy_option_still_rejects_multiple_pending_uses() {
    let input = r#"
fun unwrap_or: <T>(maybe: Option<T>, fallback: T) -> T = {
    maybe match {
        Some(value) => { value }
        None => { fallback }
    }
}

fun main: () -> String = {
    val maybe = None;
    val first = maybe;
    val second = maybe;
    val a = (first, "a") unwrap_or;
    a + ((second, "b") unwrap_or)
}
"#;

    let err = type_check(input).expect_err("Option<String> should remain affine after inference");
    assert!(
        err.contains("affine") || err.contains("already been used"),
        "expected affine rejection for non-copy inferred option, got: {err}"
    );
}

#[test]
fn local_lambda_mapper_infers_argument_type_from_map_use() {
    let input = r#"
fun main: () -> List<Int32> = {
    val xs: List<Int32> = [1, 2, 3];
    val mapper = |x| x + 1;
    (xs, mapper) map
}
"#;

    type_check(input).expect("local lambda mapper should infer Int32 argument type from map use");
}

#[test]
fn local_identity_lambda_mapper_infers_from_map_use() {
    let input = r#"
fun main: () -> List<Int32> = {
    val xs: List<Int32> = [1, 2, 3];
    val mapper = |x| x;
    (xs, mapper) map
}
"#;

    type_check(input).expect("local identity lambda should infer from map expected mapper type");
}

#[test]
fn local_identity_lambda_infers_from_direct_pipe_use() {
    let input = r#"
fun main: () -> Int32 = {
    val mapper = |x| x;
    41 |> mapper
}
"#;

    type_check(input).expect("local identity lambda should infer from direct function value use");
}

#[test]
fn local_match_callable_arms_mix_function_value_and_lambda() {
    let input = r#"
fun main: () -> Int32 = {
    val maybe_mapper: Option<Int32 -> Int32> = Some(|score| score + 1);
    val mapper = maybe_mapper match {
        Some(f) => {
            f
        }
        None => {
            |score| score
        }
    };
    41 |> mapper
}
"#;

    type_check(input)
        .expect("match-produced callable should mix existing function values and lambdas");
}

#[test]
fn local_then_callable_arms_mix_function_value_and_lambda() {
    let input = r#"
fun boost_score: (score: Int32) -> Int32 = {
    score + 10
}

fun main: () -> Int32 = {
    val use_boost = true;
    val mapper = use_boost then {
        boost_score
    } else {
        |score| score + 1
    };
    32 |> mapper
}
"#;

    type_check(input)
        .expect("then-produced callable should mix existing function values and lambdas");
}

#[test]
fn typed_contextless_lambda_infers_function_type() {
    let input = r#"
fun main: () -> Int32 = {
    val apply_buffer = |score: Int32| score + 5;
    37 |> apply_buffer
}
"#;

    type_check(input).expect("typed lambda should infer its function type without binding context");
}

#[test]
fn local_lambda_body_waits_for_expected_copy_type() {
    let input = r#"
fun main: () -> List<Int32> = {
    val xs: List<Int32> = [1, 2, 3];
    val mapper = |x| x + x;
    (xs, mapper) map
}
"#;

    type_check(input).expect("local lambda body should use later expected copy type");
}

#[test]
fn local_lambda_body_infers_empty_list_return_from_map_result() {
    let input = r#"
fun main: () -> List<List<Int32>> = {
    val xs: List<Int32> = [1, 2, 3];
    val mapper = |x| [];
    (xs, mapper) map
}
"#;

    type_check(input).expect("local lambda empty list return should infer from map result");
}

#[test]
fn immediate_lambda_body_infers_empty_list_return_from_map_result() {
    let input = r#"
fun main: () -> List<List<Int32>> = {
    val xs: List<Int32> = [1, 2, 3];
    (xs, |x| []) map
}
"#;

    type_check(input).expect("immediate lambda empty list return should infer from map result");
}

#[test]
fn local_lambda_body_waits_for_expected_type_in_direct_pipe() {
    let input = r#"
fun main: () -> Int32 = {
    val mapper = |x| x + x;
    21 |> mapper
}
"#;

    type_check(input).expect("direct function value call should resolve deferred lambda body");
}

#[test]
fn local_lambda_double_use_of_affine_input_is_rejected_after_expected_type() {
    let input = r#"
fun main: () -> List<String> = {
    val xs: List<String> = ["a", "b"];
    val mapper = |x| x + x;
    (xs, mapper) map
}
"#;

    let err = type_check(input)
        .expect_err("deferred lambda should still reject affine double-use after replay");
    assert!(
        err.contains("affine") || err.contains("already been used"),
        "error should preserve affine checking after deferred lambda replay, got: {err}"
    );
}

#[test]
fn unresolved_local_lambda_return_is_rejected_at_scope_exit() {
    let input = r#"
fun main: () -> Int32 = {
    val mapper = |x| x;
    0
}
"#;

    let err = type_check(input).expect_err("unused unresolved local lambda should be rejected");
    assert!(
        err.contains("Cannot infer type") && err.contains("mapper"),
        "error should identify the unresolved lambda binding, got: {err}"
    );
}
