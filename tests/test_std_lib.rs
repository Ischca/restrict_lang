use restrict_lang::*;

fn check_program_str(input: &str) -> Result<(), TypeError> {
    match parse_program(input) {
        Ok((_, program)) => {
            let mut checker = TypeChecker::new();
            checker.check_program(&program)
        }
        Err(e) => panic!("Parse error: {:?}", e),
    }
}

#[test]
fn test_std_math_functions() {
    let input = r#"
        fun test_math() {
            val a = abs(-5)
            val b = max(10, 20)
            val c = min(3, 7)
            val d = pow(2, 3)
            val e = factorial(4)
        }
    "#;
    assert!(check_program_str(input).is_ok());
}

#[test]
fn test_std_list_functions() {
    let input = r#"
        fun test_list() {
            val numbers = [1, 2, 3, 4, 5]
            val empty = list_is_empty(numbers)
            val head = list_head(numbers)
            val tail = list_tail(numbers)
            val reversed = list_reverse(numbers)
            val count = list_count(numbers)
        }
    "#;
    assert!(check_program_str(input).is_ok());
}

#[test]
fn test_std_option_functions() {
    let input = r#"
        fun test_option() {
            val opt = Some(42)
            val has_value = option_is_some(opt)
            val is_empty = option_is_none(opt)
            val value = option_unwrap_or(opt, 0)
        }
    "#;
    assert!(check_program_str(input).is_ok());
}

#[test]
fn test_std_io_functions() {
    let input = r#"
        fun test_io() {
            print("Hello")
            print_int(42)
            print_float(3.14)
            eprint("Error")
            eprintln("Error with newline")
        }
    "#;
    assert!(check_program_str(input).is_ok());
}

#[test]
fn test_std_prelude_functions() {
    let input = r#"
        fun test_prelude() {
            val x = identity(42)
            val bool_not = not(true)
            val bool_and = and(true, false)
            val bool_or = or(true, false)
            assert(true, "This should pass")
        }
    "#;
    assert!(check_program_str(input).is_ok());
}

#[test]
fn test_generic_list_functions_with_type_inference() {
    let input = r#"
        fun test_generic_lists() {
            val strings = ["hello", "world"]
            val string_count = list_count(strings)
            val string_head = list_head(strings)
            
            val numbers = [1, 2, 3]
            val number_count = list_count(numbers)
            val number_head = list_head(numbers)
        }
    "#;
    assert!(check_program_str(input).is_ok());
}

#[test]
fn test_generic_option_functions() {
    let input = r#"
        fun test_generic_options() {
            val int_opt = Some(42)
            val string_opt = Some("hello")
            val none_opt = None
            
            val int_value = option_unwrap_or(int_opt, 0)
            val string_value = option_unwrap_or(string_opt, "default")
            val default_value = option_unwrap_or(none_opt, 999)
        }
    "#;
    assert!(check_program_str(input).is_ok());
}

#[test]
fn test_math_with_float() {
    let input = r#"
        fun test_float_math() {
            val a = abs_f(-3.14)
            val b = max_f(1.5, 2.7)
            val c = min_f(0.5, 1.0)
        }
    "#;
    assert!(check_program_str(input).is_ok());
}

#[test]
fn test_list_operations_chaining() {
    let input = r#"
        fun test_list_chaining() {
            val numbers = [1, 2, 3, 4]
            val item = 5
            val extended = list_append(numbers, item)
            val combined = list_concat(numbers, [6, 7, 8])
            val first = list_prepend(0, numbers)
        }
    "#;
    assert!(check_program_str(input).is_ok());
}

#[test]
fn test_standard_library_comprehensive() {
    let input = r#"
        fun comprehensive_test() {
            // Math operations
            val abs_result = abs(-42)
            val max_result = max(abs_result, 50)
            
            // List operations
            val my_list = [1, 2, 3, 4, 5]
            val list_size = list_count(my_list)
            val first_element = list_head(my_list)
            val rest_elements = list_tail(my_list)
            
            // Option handling
            val maybe_first = list_head(my_list)
            val first_or_zero = option_unwrap_or(maybe_first, 0)
            val has_value = option_is_some(maybe_first)
            
            // Boolean operations
            val condition = and(has_value, max_result > 40)
            val result = or(condition, false)
            
            // Output
            print_int(first_or_zero)
            print_int(list_size)
            
            // Assertions
            assert(result, "Test should pass")
        }
    "#;
    assert!(check_program_str(input).is_ok());
}