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
fn empty_list_infers_from_binding_annotation() {
    let input = r#"
fun main: () -> List<Float64> = {
    val numbers: List<Float64> = [];
    numbers
}
"#;

    type_check(input).expect("empty list should use the binding annotation");
}

#[test]
fn empty_list_binding_uses_block_return_context() {
    let input = r#"
fun main: () -> List<Int32> = {
    val numbers = [];
    numbers
}
"#;

    type_check(input).expect("empty list binding should use the block return context");
}

#[test]
fn empty_list_infers_from_function_parameter() {
    let input = r#"
fun process_floats: (lst: List<Float64>) -> List<Float64> = {
    lst
}

fun main: () -> List<Float64> = {
    [] |> process_floats
}
"#;

    type_check(input).expect("empty list should use the called function parameter type");
}

#[test]
fn empty_list_in_match_arm_uses_result_context() {
    let input = r#"
fun keep_strings: (items: List<String>) -> List<String> = {
    items match {
        [head | tail] => { tail }
        [] => { [] }
    }
}
"#;

    type_check(input).expect("empty match arm should use the annotated function return type");
}

#[test]
fn then_empty_list_infers_from_later_sibling_branch() {
    let input = r#"
fun consume_ints: (items: List<Int32>) -> Int32 = {
    1
}

fun main: (flag: Boolean) -> Int32 = {
    val values = flag then {
        []
    } else {
        [1, 2]
    };
    values |> consume_ints
}
"#;

    type_check(input).expect("then branch [] should infer from a later concrete sibling");
}

#[test]
fn inferred_empty_list_moved_from_then_branch_rejects_reuse() {
    let input = r#"
fun consume_ints: (items: List<Int32>) -> Int32 = {
    1
}

fun main: (flag: Boolean) -> Int32 = {
    val items = [];
    val selected = flag then {
        items
    } else {
        [1, 2]
    };
    (selected |> consume_ints) + (items |> consume_ints)
}
"#;

    let err = type_check(input).expect_err("List<Int32> inferred after branch move is affine");
    assert!(
        err.contains("affine") || err.contains("already been used"),
        "expected affine rejection for branch-moved empty list, got: {err}"
    );
}

#[test]
fn then_none_infers_from_later_sibling_branch() {
    let input = r#"
fun unwrap_or_zero: (maybe: Option<Int32>) -> Int32 = {
    maybe match {
        Some(value) => { value }
        None => { 0 }
    }
}

fun main: (flag: Boolean) -> Int32 = {
    val maybe = flag then {
        None
    } else {
        Some(7)
    };
    maybe |> unwrap_or_zero
}
"#;

    type_check(input).expect("then branch None should infer from a later Some sibling");
}

#[test]
fn match_empty_list_infers_from_later_sibling_arm() {
    let input = r#"
fun consume_ints: (items: List<Int32>) -> Int32 = {
    1
}

fun main: (flag: Boolean) -> Int32 = {
    val values = flag match {
        true => { [] }
        false => { [1, 2] }
    };
    values |> consume_ints
}
"#;

    type_check(input).expect("match arm [] should infer from a later concrete sibling");
}

#[test]
fn inferred_empty_list_moved_from_match_arm_rejects_reuse() {
    let input = r#"
fun consume_ints: (items: List<Int32>) -> Int32 = {
    1
}

fun main: (flag: Boolean) -> Int32 = {
    val items = [];
    val selected = flag match {
        true => { items }
        false => { [1, 2] }
    };
    (selected |> consume_ints) + (items |> consume_ints)
}
"#;

    let err = type_check(input).expect_err("List<Int32> inferred after match move is affine");
    assert!(
        err.contains("affine") || err.contains("already been used"),
        "expected affine rejection for match-moved empty list, got: {err}"
    );
}

#[test]
fn match_none_infers_from_later_sibling_arm() {
    let input = r#"
fun unwrap_or_zero: (maybe: Option<Int32>) -> Int32 = {
    maybe match {
        Some(value) => { value }
        None => { 0 }
    }
}

fun main: (flag: Boolean) -> Int32 = {
    val maybe = flag match {
        true => { None }
        false => { Some(7) }
    };
    maybe |> unwrap_or_zero
}
"#;

    type_check(input).expect("match arm None should infer from a later Some sibling");
}

#[test]
fn empty_list_in_some_uses_nested_option_context() {
    let input = r#"
fun main: () -> Option<List<Boolean>> = {
    val maybe: Option<List<Boolean>> = Some([]);
    maybe
}
"#;

    type_check(input).expect("empty list should infer inside Option context");
}

#[test]
fn empty_list_inside_local_ok_infers_from_later_result_use() {
    let input = r#"
fun add_int: (total: Int32, value: Int32) -> Int32 = {
    total + value
}

fun result_list_total: (result: Result<List<Int32>, String>) -> Int32 = {
    result match {
        Ok(values) => {
            (values, 0, add_int) fold
        }
        Err(error) => {
            0
        }
    }
}

fun main: () -> Int32 = {
    val result = Ok([]);
    result |> result_list_total
}
"#;

    type_check(input).expect("empty list inside local Ok should infer from later Result use");
}

#[test]
fn list_elements_use_expected_option_context() {
    let input = r#"
fun main: () -> List<Option<Int32>> = {
    [None, Some(1)]
}
"#;

    type_check(input).expect("list elements should use the expected element type");
}

#[test]
fn list_elements_infer_none_from_sibling_some() {
    let input = r#"
fun main: () -> List<Option<Int32>> = {
    val values = [None, Some(1)];
    values
}
"#;

    type_check(input).expect("None should infer from sibling Some in a list literal");
}

#[test]
fn empty_list_element_infers_from_sibling_list() {
    let input = r#"
fun main: () -> List<List<Int32>> = {
    [[], [1, 2]]
}
"#;

    type_check(input).expect("empty list element should infer from sibling list element");
}

#[test]
fn singleton_none_list_still_requires_context() {
    let input = r#"
fun main: () -> Int32 = {
    val values = [None];
    1
}
"#;

    let err = type_check(input).expect_err("singleton None list should still need context");
    assert!(
        err.contains("Cannot infer type"),
        "error should explain inference failure, got: {}",
        err
    );
}

#[test]
fn contextless_empty_list_is_rejected() {
    let input = r#"
fun main: () -> Int32 = {
    val values = [];
    1
}
"#;

    let err = type_check(input).expect_err("empty list without context should fail");
    assert!(
        err.contains("Cannot infer type"),
        "error should explain inference failure, got: {}",
        err
    );
}

#[test]
fn none_infers_from_binding_annotation() {
    let input = r#"
fun main: () -> Option<Int32> = {
    val maybe: Option<Int32> = None;
    maybe
}
"#;

    type_check(input).expect("None should use the binding annotation");
}

#[test]
fn none_in_nested_block_uses_return_context() {
    let input = r#"
fun main: () -> Option<Int32> = {
    {
        None
    }
}
"#;

    type_check(input).expect("nested block should preserve the expected return type");
}

#[test]
fn empty_list_in_lambda_block_uses_expected_return_context() {
    let input = r#"
fun apply_builder: (builder: Int32 -> List<Int32>, seed: Int32) -> List<Int32> = {
    seed |> builder
}

fun main: () -> List<Int32> = {
    (|seed| {
        []
    }, 1) apply_builder
}
"#;

    type_check(input).expect("lambda block body should use the expected function return type");
}

#[test]
fn contextless_none_is_rejected() {
    let input = r#"
fun main: () -> Int32 = {
    val maybe = None;
    1
}
"#;

    let err = type_check(input).expect_err("None without context should fail");
    assert!(
        err.contains("Cannot infer type"),
        "error should explain inference failure, got: {}",
        err
    );
}
