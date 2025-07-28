// Restrict Language Native Test DSL
// Embracing OSV syntax, pipes, and temporal types

import "std_lib.rl"

// Core types with minimal boilerplate
record Test { 
    name: String, 
    run: () -> TestResult 
}

record TestResult {
    passed: Bool,
    message: String
}

// Custom operators for assertions
infix =? 8  // equality check
infix >? 8  // greater than check
infix <? 8  // less than check
infix ~? 8  // pattern match check
infix |? 8  // contains check

// Assertion operators implementation
fun =? = |actual, expected| {
    if actual == expected {
        TestResult { passed = true, message = "✓" }
    } else {
        TestResult { 
            passed = false, 
            message = actual.toString ++ " ≠ " ++ expected.toString 
        }
    }
}

fun >? = |actual, expected| {
    if actual > expected {
        TestResult { passed = true, message = "✓" }
    } else {
        TestResult { 
            passed = false, 
            message = actual.toString ++ " ≤ " ++ expected.toString 
        }
    }
}

fun <? = |actual, expected| {
    if actual < expected {
        TestResult { passed = true, message = "✓" }
    } else {
        TestResult { 
            passed = false, 
            message = actual.toString ++ " ≥ " ++ expected.toString 
        }
    }
}

// Pattern match operator
fun ~? = |actual, pattern| {
    pattern(actual)
}

// Contains operator for lists
fun |? = |list: List<T>, element: T| {
    list.contains(element) true "element found".whenTrue
}

// Pipe-friendly test builder
fun test = |name: String| -> |(() -> TestResult) -> Test| {
    |assertion| Test { name = name, run = assertion }
}

// Test combinator for multiple assertions
fun && = |result1: TestResult, result2: TestResult| -> TestResult {
    if result1.passed && result2.passed {
        TestResult { passed = true, message = "✓" }
    } else {
        TestResult { 
            passed = false, 
            message = result1.message ++ " && " ++ result2.message 
        }
    }
}

// Pattern builders for common assertions
fun whenTrue = |condition: Bool, message: String| -> TestResult {
    if condition {
        TestResult { passed = true, message = "✓" }
    } else {
        TestResult { passed = false, message = message }
    }
}

fun whenSome = |opt: Option<T>, check: T -> TestResult| -> TestResult {
    match opt {
        Some(value) -> value.check,
        None -> TestResult { passed = false, message = "Expected Some, got None" }
    }
}

fun whenNone = |opt: Option<T>| -> TestResult {
    match opt {
        None -> TestResult { passed = true, message = "✓" },
        Some(_) -> TestResult { passed = false, message = "Expected None, got Some" }
    }
}

// Temporal test isolation
fun isolated = |<~test> name: String, testFn: () -> TestResult| -> Test {
    Test {
        name = name,
        run = || {
            with lifetime<~test> {
                // All test resources are cleaned up automatically
                testFn()
            }
        }
    }
}

// Property testing with pipes
fun forAll = |gen: () -> T| -> |property: T -> Bool| -> |times: Int32| -> TestResult {
    var passed = 0;
    var failed = None;
    
    (1..times).forEach |i| {
        val input = gen();
        if input.property {
            passed = passed + 1;
        } else if failed.isNone {
            failed = Some(input);
        }
    };
    
    match failed {
        None -> TestResult { passed = true, message = "✓ " ++ passed.toString ++ " cases" },
        Some(counterexample) -> TestResult { 
            passed = false, 
            message = "✗ Failed at: " ++ counterexample.toString 
        }
    }
}

// Table-driven tests with pipes
fun testCases = |cases: List<(I, O)>| -> |f: I -> O| -> List<TestResult> {
    cases.map |(input, expected)| {
        (input.f) expected.=?
    }
}

// Test suite runner using pipes
fun runTests = |tests: List<Test>| {
    "\n⚡ Restrict Test Runner\n".println;
    
    tests
        |> (|test| {
            val result = test.run();
            val status = if result.passed { "✓" } else { "✗" };
            (status ++ " " ++ test.name ++ " " ++ result.message).println;
            result
        }).map
        |> (|results| {
            val passed = results.filter(|r| r.passed).length;
            val total = results.length;
            "\n━━━━━━━━━━━━━━━━━━━━━".println;
            ("Passed: " ++ passed.toString ++ "/" ++ total.toString).println;
            if passed == total { "✨ All tests passed!" } else { "💥 Some tests failed" }
        })
        |> println
}

// DSL for building test suites with pipes
fun suite = |tests: List<Test>| -> List<Test> { tests }

// Example: Natural test flow
fun examples = {
    []
        |> ("basic math" || {
            (1 + 1) 2.=? && (2 * 3) 6.=?
        }).test.cons
        
        |> ("comparisons" || {
            5 3.>? && 3 5.<? && 10 10.=?
        }).test.cons
        
        |> ("lists" || {
            [1, 2, 3] 2.|? && 
            [].length 0.=? &&
            [1, 2] ++ [3, 4] [1, 2, 3, 4].=?
        }).test.cons
        
        |> ("options" || {
            Some(42).whenSome(|x| x 42.=?) &&
            None.whenNone
        }).test.cons
        
        |> ("pattern matching" ~? |value| {
            match value {
                42 -> TestResult { passed = true, message = "✓" },
                _ -> TestResult { passed = false, message = "Not 42" }
            }
        } |> (|check| 42.check)).test.cons
        
        |> ("property: reverse twice" |> 
            (|| [1, 2, 3, 4, 5].shuffle) |>
            (|list| list.reverse.reverse == list) |>
            100.forAll
        ).test.cons
        
        |> ("isolated resource test" |<~test>| {
            record Resource<~r> { id: Int32 }
            with lifetime<~r> {
                val res = Resource { id = 123 };
                res.id 123.=?
            }
        }).isolated.cons
        
        |> ("table test: fibonacci" |> {
            [(0, 0), (1, 1), (2, 1), (5, 5), (10, 55)]
                .testCases
                .fib
                .all(|r| r.passed)
                .whenTrue("All fibonacci tests passed")
        }).test.cons
}

// Advanced: Test specification using temporal constraints
fun spec = |<~s> name: String| -> |setup: () -> Context<~s>| -> |tests: Context<~s> -> List<Test>| -> List<Test> {
    with lifetime<~s> {
        val ctx = setup();
        ctx.tests
    }
}

record Context<~c> {
    data: Map<String, Any>
}

// Async test support with temporal types
fun eventually = |<~async> assertion: () -> TestResult, timeout: Duration| -> Test {
    "async test" || {
        with lifetime<~async> {
            // Would retry assertion until timeout
            assertion()
        }
    }.test
}

// Main runner
fun main = {
    examples.runTests
}

// Helper functions
fun cons = |elem: T, list: List<T>| -> List<T> { [elem] ++ list }
fun all = |list: List<T>, pred: T -> Bool| -> Bool {
    list.filter(pred.not).length == 0
}

// Fibonacci for testing
fun fib = |n: Int32| -> Int32 {
    match n {
        0 -> 0,
        1 -> 1,
        n -> (n - 1).fib + (n - 2).fib
    }
}