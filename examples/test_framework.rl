// v0.0.1 test-framework smoke example.
// Keeps the old file's intent through small result aggregation without legacy DSL.

record CheckResult {
    passed: Boolean,
    failures: Int32
}

fun check_equal_int: (actual: Int32, expected: Int32) -> CheckResult = {
    actual == expected then {
        CheckResult { passed: true, failures: 0 }
    } else {
        CheckResult { passed: false, failures: 1 }
    }
}

fun check_equal_bool: (actual: Boolean, expected: Boolean) -> CheckResult = {
    actual == expected then {
        CheckResult { passed: true, failures: 0 }
    } else {
        CheckResult { passed: false, failures: 1 }
    }
}

fun combine_results: (left: CheckResult, right: CheckResult) -> CheckResult = {
    val CheckResult { passed: left_passed, failures: left_failures } = left;
    val CheckResult { passed: right_passed, failures: right_failures } = right;

    CheckResult {
        passed: left_passed && right_passed,
        failures: left_failures + right_failures
    }
}

fun suite_result: (first: Int32, second: Int32) -> CheckResult = {
    val math_result = (first + second, 7) check_equal_int;
    val order_result = (first < second, true) check_equal_bool;

    (math_result, order_result) combine_results
}

fun main: () -> Int32 = {
    val result = (3, 4) suite_result;
    result.failures
}
