// Example demonstrating improved lambda type inference

// 1. Basic inference from body usage
fun test_basic_inference = {
    // Parameter type inferred from + operator with Int32 literal
    val add_one = |x| x + 1;
    val result = (41) add_one;
    result
}

// 2. Inference from function application
fun apply_int_function = f:Int->Int, x:Int {
    val result = (x) f;
    result
}

fun test_application_inference = {
    // Parameter type inferred from being passed to a typed function
    val double = |x| x * 2;
    val result = apply_int_function(double, 21);
    result
}

// 3. Nested lambda inference
fun test_nested_inference = {
    // Both x and y inferred as Int32 from usage
    val curry_add = |x| |y| x + y;
    val add5 = (5) curry_add;
    val result = (10) add5;
    result
}

// 4. Inference with comparison operators
fun test_comparison_inference = {
    // x inferred as Int32 from comparison with 0
    val is_positive = |x| x > 0;
    val check1 = (42) is_positive;
    val check2 = (-5) is_positive;
    check1
}

// 5. Inference in Option context
fun test_option_inference = {
    // Lambda type preserved in Option
    val maybe_increment = Some(|x| x + 1);
    maybe_increment
}

// 6. Multiple parameter inference
fun test_multi_param_inference = {
    // Both parameters inferred from usage
    val max = |x, y| {
        val cond = x > y;
        cond match {
            true => { x }
            false => { y }
        }
    };
    val result = (10, 20) max;
    result
}

// Main function to test all examples
fun main = {
    val test1 = test_basic_inference();
    val test2 = test_application_inference();
    val test3 = test_nested_inference();
    val test4 = test_comparison_inference();
    val test5 = test_option_inference();
    val test6 = test_multi_param_inference();
    
    test1 + test2 + test3 + test6
}