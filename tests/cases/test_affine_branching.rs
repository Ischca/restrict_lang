use restrict_lang::{parse_program, TypeChecker};

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
fn then_else_allows_same_affine_move_in_exclusive_branches() {
    let source = r#"
fun choose: (flag: Boolean, value: String) -> String = {
    flag then {
        value
    } else {
        value
    }
}
"#;

    type_check(source).expect("exclusive then/else branches may each move the same affine value");
}

#[test]
fn then_else_marks_affine_value_used_after_partial_branch_move() {
    let source = r#"
fun bad: (flag: Boolean, value: String) -> String = {
    val _selected = flag then {
        value
    } else {
        "fallback"
    };
    value
}
"#;

    let err = type_check(source).expect_err("value may have been moved by the then branch");
    assert!(
        err.contains("value") && err.contains("already been used"),
        "error should report the maybe-moved affine value, got: {}",
        err
    );
}

#[test]
fn match_allows_same_affine_move_in_exclusive_arms() {
    let source = r#"
fun choose: (flag: Boolean, value: String) -> String = {
    flag match {
        true => {
            value
        }
        false => {
            value
        }
    }
}
"#;

    type_check(source).expect("exclusive match arms may each move the same affine value");
}

#[test]
fn match_marks_affine_value_used_after_partial_arm_move() {
    let source = r#"
fun bad: (flag: Boolean, value: String) -> String = {
    val _selected = flag match {
        true => {
            value
        }
        false => {
            "fallback"
        }
    };
    value
}
"#;

    let err = type_check(source).expect_err("value may have been moved by one match arm");
    assert!(
        err.contains("value") && err.contains("already been used"),
        "error should report the maybe-moved affine value, got: {}",
        err
    );
}

#[test]
fn tuple_call_rejects_repeated_affine_argument() {
    let source = r#"
fun join: (left: String, right: String) -> String = {
    left + right
}

fun bad: (value: String) -> String = {
    (value, value) join
}
"#;

    let err = type_check(source).expect_err("tuple OSV calls should not duplicate affine values");
    assert!(
        err.contains("value") && err.contains("already been used"),
        "error should report the duplicated affine argument, got: {}",
        err
    );
}

#[test]
fn list_literal_rejects_repeated_affine_element() {
    let source = r#"
fun bad: (value: String) -> List<String> = {
    [value, value]
}
"#;

    let err = type_check(source).expect_err("list literals should not duplicate affine values");
    assert!(
        err.contains("value") && err.contains("already been used"),
        "error should report the duplicated affine element, got: {}",
        err
    );
}

#[test]
fn string_field_access_rejects_multiple_affine_field_moves() {
    let source = r#"
record User {
    name: String,
    email: String
}

fun bad: (user: User) -> String = {
    user.name + user.email
}
"#;

    let err = type_check(source).expect_err("moving one String field should consume the record");
    assert!(
        err.contains("user") && err.contains("already been used"),
        "error should report the consumed record after moving an affine field, got: {}",
        err
    );
}

#[test]
fn copyable_field_access_can_read_record_multiple_times() {
    let source = r#"
record Point {
    x: Int32,
    y: Int32
}

fun sum: (point: Point) -> Int32 = {
    point.x + point.y + point.x
}
"#;

    type_check(source).expect("copyable fields should not consume the parent record");
}
