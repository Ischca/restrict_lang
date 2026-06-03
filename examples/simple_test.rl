// Minimal smoke test sketch in current Restrict syntax.
// This is not the full test framework. It keeps only syntax that matches the
// language specification.

record TestResult {
    name: String
    passed: Int32
}

fun assert_equal: (actual: Int32, expected: Int32) -> Int32 = {
    actual == expected then {
        1
    } else {
        0
    }
}

fun arithmetic_test: () -> TestResult = {
    val passed = (1 + 1, 2) assert_equal
    TestResult { name: "Basic arithmetic", passed: passed }
}

fun option_test: () -> TestResult = {
    val result = Some(42) match {
        Some(x) => { x }
        None => { 0 }
    }
    val passed = (result, 42) assert_equal
    TestResult { name: "Option pattern", passed: passed }
}

fun main: () = {
    val arithmetic = () arithmetic_test
    val option = () option_test
    arithmetic.passed + option.passed
}
