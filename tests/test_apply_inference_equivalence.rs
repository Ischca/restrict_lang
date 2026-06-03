use restrict_lang::{parse_program, TypeChecker};

fn type_check(input: &str) -> Result<(), String> {
    let (remaining, program) = parse_program(input).map_err(|e| format!("Parse error: {e:?}"))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {remaining:?}"));
    }

    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .map_err(|e| format!("Type error: {e}"))
}

fn assert_type_checks(cases: &[(&str, &str)]) {
    for (name, input) in cases {
        type_check(input).unwrap_or_else(|err| {
            panic!("{name} should type check, got {err}\n\nSource:\n{input}")
        });
    }
}

#[test]
fn generic_identity_and_choose_infer_equivalent_int_results() {
    assert_type_checks(&[
        (
            "OSV tuple call to generic choose",
            r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> Int32 = {
    (41, 0) choose_first
}
"#,
        ),
        (
            "pipe call to generic identity",
            r#"
fun identity_local: <T>(value: T) -> T = {
    value
}

fun main: () -> Int32 = {
    41 |> identity_local
}
"#,
        ),
        (
            "named generic identity function value call",
            r#"
fun identity_local: <T>(value: T) -> T = {
    value
}

fun main: () -> Int32 = {
    val keep = identity_local;
    41 |> keep
}
"#,
        ),
        (
            "parenthesized generic function value pipe target",
            r#"
fun identity_local: <T>(value: T) -> T = {
    value
}

fun main: () -> Int32 = {
    val keep = identity_local;
    41 |> (keep)
}
"#,
        ),
        (
            "named generic choose function value call",
            r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> Int32 = {
    val choose = choose_first;
    (41, 0) choose
}
"#,
        ),
        (
            "immediate lambda call",
            r#"
fun main: () -> Int32 = {
    41 |> (|value| value)
}
"#,
        ),
        (
            "local deferred lambda use",
            r#"
fun main: () -> Int32 = {
    val keep = |value| value;
    41 |> keep
}
"#,
        ),
        (
            "parenthesized deferred lambda pipe target",
            r#"
fun main: () -> Int32 = {
    val keep = |value| value + 1;
    41 |> (keep)
}
"#,
        ),
        (
            "local deferred lambda choose use",
            r#"
fun main: () -> Int32 = {
    val choose = |value, fallback| value;
    (41, 0) choose
}
"#,
        ),
    ]);
}

#[test]
fn map_infers_equivalent_list_and_option_results() {
    assert_type_checks(&[
        (
            "List map with immediate lambda mapper",
            r#"
fun main: () -> List<Int32> = {
    val numbers: List<Int32> = [1, 2, 3];
    (numbers, |value| value + 1) map
}
"#,
        ),
        (
            "List map with named function value mapper",
            r#"
fun keep_int: (value: Int32) -> Int32 = {
    value
}

fun main: () -> List<Int32> = {
    val numbers: List<Int32> = [1, 2, 3];
    val mapper = keep_int;
    (numbers, mapper) map
}
"#,
        ),
        (
            "List map with local deferred lambda mapper",
            r#"
fun main: () -> List<Int32> = {
    val numbers: List<Int32> = [1, 2, 3];
    val mapper = |value| value + 1;
    (numbers, mapper) map
}
"#,
        ),
        (
            "Option map with immediate lambda mapper",
            r#"
fun main: () -> Option<Int32> = {
    val maybe: Option<Int32> = Some(1);
    (maybe, |value| value + 1) map
}
"#,
        ),
        (
            "Option map with named function value mapper",
            r#"
fun keep_int: (value: Int32) -> Int32 = {
    value
}

fun main: () -> Option<Int32> = {
    val maybe: Option<Int32> = Some(1);
    val mapper = keep_int;
    (maybe, mapper) map
}
"#,
        ),
        (
            "Option map with local deferred lambda mapper",
            r#"
fun main: () -> Option<Int32> = {
    val maybe: Option<Int32> = Some(1);
    val mapper = |value| value;
    (maybe, mapper) map
}
"#,
        ),
    ]);
}

#[test]
fn first_class_builtin_apply_preserves_container_projections() {
    assert_type_checks(&[
        (
            "first-class List map preserves mapped container projection",
            r#"
fun main: () -> List<String> = {
    val numbers: List<Int32> = [1, 2, 3];
    val apply_map = map;
    (numbers, |value| "x") apply_map
}
"#,
        ),
        (
            "annotated first-class List map resolves Container projections",
            r#"
fun main: () -> List<String> = {
    val apply_map: (List<Int32>, Int32 -> String) -> List<String> = map;
    val numbers: List<Int32> = [1, 2, 3];
    (numbers, |value| "x") apply_map
}
"#,
        ),
        (
            "first-class Option map preserves value-changing Container projection",
            r#"
fun main: () -> Option<String> = {
    val maybe: Option<Int32> = None;
    val apply_map = map;
    (maybe, |value| "x") apply_map
}
"#,
        ),
        (
            "first-class List filter lowers item projection",
            r#"
fun main: () -> List<Int32> = {
    val numbers: List<Int32> = [1, 2, 3];
    val apply_filter = filter;
    (numbers, |value| value > 1) apply_filter
}
"#,
        ),
        (
            "first-class Option filter lowers item projection",
            r#"
fun main: () -> Option<Int32> = {
    val maybe: Option<Int32> = Some(3);
    val apply_filter = filter;
    (maybe, |value| value > 1) apply_filter
}
"#,
        ),
        (
            "first-class fold preserves accumulator and item inference",
            r#"
fun main: () -> Int32 = {
    val numbers: List<Int32> = [1, 2, 3];
    val apply_fold = fold;
    (numbers, 0, |total, value| total + value) apply_fold
}
"#,
        ),
    ]);
}

#[test]
fn apply_forms_contextualize_lambda_arguments_order_independently() {
    assert_type_checks(&[
        (
            "immediate lambda callee contextualizes lambda argument",
            r#"
fun main: () -> Int32 = {
    (|value| value + 1, 41) (|f: Int32 -> Int32, input: Int32| input |> f)
}
"#,
        ),
        (
            "pipe lambda object uses target parameter context",
            r#"
fun run: <T>(f: Int32 -> T) -> T = {
    41 |> f
}

fun main: () -> String = {
    (|value| "ok") |> run
}
"#,
        ),
        (
            "generic function value keeps lambda-before-source order independent",
            r#"
fun apply_first: <T, U>(f: T -> U, value: T) -> U = {
    value |> f
}

fun main: () -> Int32 = {
    val apply = apply_first;
    (|value| value + 1, 41) apply
}
"#,
        ),
    ]);
}

#[test]
fn empty_list_and_none_use_expected_return_across_apply_forms() {
    assert_type_checks(&[
        (
            "empty list through generic choose OSV tuple call",
            r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> List<Int32> = {
    ([], [1, 2]) choose_first
}
"#,
        ),
        (
            "None through generic choose OSV tuple call",
            r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> Option<Int32> = {
    (None, Some(1)) choose_first
}
"#,
        ),
        (
            "empty list through generic identity pipe call",
            r#"
fun identity_local: <T>(value: T) -> T = {
    value
}

fun main: () -> List<Int32> = {
    [] |> identity_local
}
"#,
        ),
        (
            "None through generic identity pipe call",
            r#"
fun identity_local: <T>(value: T) -> T = {
    value
}

fun main: () -> Option<Int32> = {
    None |> identity_local
}
"#,
        ),
        (
            "empty list through named generic function value call",
            r#"
fun identity_local: <T>(value: T) -> T = {
    value
}

fun main: () -> List<Int32> = {
    val keep = identity_local;
    [] |> keep
}
"#,
        ),
        (
            "None through named generic function value call",
            r#"
fun identity_local: <T>(value: T) -> T = {
    value
}

fun main: () -> Option<Int32> = {
    val keep = identity_local;
    None |> keep
}
"#,
        ),
        (
            "empty list through immediate lambda call",
            r#"
fun main: () -> List<Int32> = {
    [] |> (|items| items)
}
"#,
        ),
        (
            "None through immediate lambda call",
            r#"
fun main: () -> Option<Int32> = {
    None |> (|maybe| maybe)
}
"#,
        ),
        (
            "empty list through local deferred lambda use",
            r#"
fun main: () -> List<Int32> = {
    val keep = |items| items;
    [] |> keep
}
"#,
        ),
        (
            "None through local deferred lambda use",
            r#"
fun main: () -> Option<Int32> = {
    val keep = |maybe| maybe;
    None |> keep
}
"#,
        ),
    ]);
}

#[test]
fn affine_double_use_is_rejected_across_apply_forms() {
    let cases = [
        (
            "pipe call to immediate lambda",
            r#"
fun main: () -> String = {
    "a" |> (|value| value + value)
}
"#,
        ),
        (
            "OSV tuple call to immediate lambda",
            r#"
fun main: () -> String = {
    ("a", "unused") (|value, fallback| value + value)
}
"#,
        ),
        (
            "local deferred lambda use",
            r#"
fun main: () -> String = {
    val duplicate = |value| value + value;
    "a" |> duplicate
}
"#,
        ),
        (
            "map with local deferred lambda mapper",
            r#"
fun main: () -> List<String> = {
    val words: List<String> = ["a", "b"];
    val duplicate = |word| word + word;
    (words, duplicate) map
}
"#,
        ),
    ];

    for (name, input) in cases {
        let err = type_check(input).unwrap_err();
        assert!(
            err.contains("affine") || err.contains("already been used"),
            "{name} should reject affine double use, got {err}"
        );
    }
}
