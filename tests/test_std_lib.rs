use restrict_lang::*;

fn check_program_str(input: &str) -> Result<(), String> {
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
fn std_math_functions_use_osv_calls() {
    let input = r#"
fun test_math: () -> Int32 = {
    val a = -5 |> abs;
    val b = (10, 20) max;
    val c = (3, 7) min;
    val d = (2, 3) pow;
    val e = 4 |> factorial;
    a + b + c + d + e
}
"#;

    check_program_str(input).expect("math functions should type check");
}

#[test]
fn std_list_functions_use_affine_aware_bindings() {
    let input = r#"
fun test_list: () -> Int32 = {
    mut val numbers = [1, 2, 3, 4, 5];
    val empty = numbers |> list_is_empty;
    val head = numbers |> list_head;
    val tail = numbers |> list_tail;
    val reversed = numbers |> list_reverse;
    val count = numbers |> list_count;
    count
}
"#;

    check_program_str(input).expect("list functions should type check");
}

#[test]
fn std_option_functions_use_osv_calls() {
    let input = r#"
fun test_option: () -> Int32 = {
    val opt = Some(42);
    val has_value = opt |> option_is_some;
    val is_empty = opt |> option_is_none;
    val value = (opt, 0) option_unwrap_or;
    value
}
"#;

    check_program_str(input).expect("option functions should type check");
}

#[test]
fn std_io_functions_use_osv_calls() {
    let input = r#"
fun test_io: () -> () = {
    "Hello" |> print;
    42 |> print_int;
    3.14 |> print_float;
    "Error" |> eprint;
    "Error with newline" |> eprintln
}
"#;

    check_program_str(input).expect("I/O functions should type check");
}

#[test]
fn std_prelude_functions_use_osv_calls() {
    let input = r#"
fun test_prelude: () -> Boolean = {
    val x = 42 |> identity;
    val bool_not = true |> not;
    val bool_and = (true, false) and;
    val bool_or = (bool_and, false) or;
    (true, "This should pass") assert;
    bool_or
}
"#;

    check_program_str(input).expect("prelude functions should type check");
}

#[test]
fn generic_list_functions_infer_element_types() {
    let input = r#"
fun test_generic_lists: () -> Int32 = {
    mut val strings = ["hello", "world"];
    val string_count = strings |> list_count;
    val string_head = strings |> list_head;

    mut val numbers = [1, 2, 3];
    val number_count = numbers |> list_count;
    val number_head = numbers |> list_head;
    string_count + number_count
}
"#;

    check_program_str(input).expect("generic list functions should infer element types");
}

#[test]
fn generic_option_functions_infer_payload_types() {
    let input = r#"
fun test_generic_options: () -> Int32 = {
    val int_opt = Some(42);
    val int_value = (int_opt, 0) option_unwrap_or;

    val string_opt = Some("hello");
    val string_value = (string_opt, "default") option_unwrap_or;

    val none_opt: Option<Int32> = None;
    val default_value = (none_opt, 999) option_unwrap_or;
    int_value + default_value
}
"#;

    check_program_str(input).expect("generic option functions should infer payload types");
}

#[test]
fn float_math_functions_type_check() {
    let input = r#"
fun test_float_math: () -> Float64 = {
    val a = -3.14 |> abs_f;
    val b = (1.5, 2.7) max_f;
    val c = (0.5, 1.0) min_f;
    a + b + c
}
"#;

    check_program_str(input).expect("float math functions should type check");
}

#[test]
fn list_operations_compose_with_osv_calls() {
    let input = r#"
fun test_list_chaining: () -> Int32 = {
    val extended = ([1, 2, 3, 4], 5) list_append;
    val combined = ([1, 2, 3, 4], [6, 7, 8]) list_concat;
    val first = (0, [1, 2, 3, 4]) list_prepend;
    val a = extended |> list_count;
    val b = combined |> list_count;
    val c = first |> list_count;
    a + b + c
}
"#;

    check_program_str(input).expect("list operations should type check");
}

#[test]
fn standard_library_comprehensive_osv_flow() {
    let input = r#"
fun comprehensive_test: () -> Boolean = {
    val abs_result = -42 |> abs;
    val max_result = (abs_result, 50) max;

    mut val my_list = [1, 2, 3, 4, 5];
    val list_size = my_list |> list_count;
    val maybe_first = my_list |> list_head;
    val rest_elements = my_list |> list_tail;

    val first_or_zero = (maybe_first, 0) option_unwrap_or;
    val has_value = maybe_first |> option_is_some;

    val above_threshold = max_result > 40;
    val condition = (has_value, above_threshold) and;
    val result = (condition, false) or;

    first_or_zero |> print_int;
    list_size |> print_int;
    (result, "Test should pass") assert;
    result
}
"#;

    check_program_str(input).expect("comprehensive stdlib flow should type check");
}
