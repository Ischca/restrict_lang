// Test Framework written in Restrict Language
// This demonstrates the language's capabilities while providing a testing solution

import "std_lib.rl"

record TestCase {
    name: String,
    description: String,
    testFn: () -> TestResult
}

record TestResult {
    passed: Bool,
    message: String
}

record TestSuite {
    name: String,
    tests: List<TestCase>
}

record TestRunner {
    suites: List<TestSuite>,
    results: List<(String, TestResult)>
}

// Assert functions
fun assertTrue = |condition: Bool, message: String| {
    if condition {
        TestResult { passed = true, message = "OK" }
    } else {
        TestResult { passed = false, message = message }
    }
}

fun assertFalse = |condition: Bool, message: String| {
    condition.not message.assertTrue
}

fun assertEqual = |actual, expected, message: String| {
    if actual == expected {
        TestResult { passed = true, message = "OK" }
    } else {
        TestResult { 
            passed = false, 
            message = message ++ " - Expected: " ++ expected.toString ++ ", Got: " ++ actual.toString
        }
    }
}

// Test runner functions
fun createRunner = {
    TestRunner { 
        suites = [], 
        results = [] 
    }
}

fun addSuite = |runner: TestRunner, suite: TestSuite| {
    runner.clone { suites = runner.suites ++ [suite] }
}

fun runTest = |test: TestCase| {
    val result = test.testFn();
    (test.name, result)
}

fun runSuite = |suite: TestSuite| {
    suite.tests.map |test| test.runTest
}

fun runAll = |runner: TestRunner| {
    val allResults = runner.suites.flatMap |suite| suite.runSuite;
    runner.clone { results = allResults }
}

// Result reporting
fun countPassed = |results: List<(String, TestResult)>| {
    results.filter |r| r.1.passed .length
}

fun countFailed = |results: List<(String, TestResult)>| {
    results.filter |r| r.1.passed.not .length
}

fun printResults = |runner: TestRunner| {
    "=== Test Results ===".println;
    
    runner.results.forEach |result| {
        val (name, res) = result;
        if res.passed {
            ("✓ " ++ name).println;
        } else {
            ("✗ " ++ name ++ ": " ++ res.message).println;
        }
    };
    
    val passed = runner.results.countPassed;
    val failed = runner.results.countFailed;
    val total = passed + failed;
    
    "".println;
    ("Total:  " ++ total.toString).println;
    ("Passed: " ++ passed.toString).println;
    ("Failed: " ++ failed.toString).println;
}

// DSL for creating tests
fun test = |name: String, testFn: () -> TestResult| {
    TestCase {
        name = name,
        description = "",
        testFn = testFn
    }
}

fun suite = |name: String, tests: List<TestCase>| {
    TestSuite {
        name = name,
        tests = tests
    }
}

// Example usage
fun exampleTests = {
    val runner = createRunner
        |> ("Basic Tests" [
            "addition" (|| (1 + 1) 2 "1 + 1 should equal 2".assertEqual).test,
            "boolean" (|| true "true should be true".assertTrue).test,
            "strings" (|| "hello" "hello" "strings should match".assertEqual).test
        ].suite).addSuite
        |> ("List Tests" [
            "empty list" (|| [].length 0 "empty list length".assertEqual).test,
            "list append" (|| ([1] ++ [2]) [1, 2] "list concatenation".assertEqual).test,
            "list head" (|| [1, 2, 3].head Some(1) "list head".assertEqual).test
        ].suite).addSuite
        |> runAll;
    
    runner.printResults
}

// Test macros using temporal types for test isolation
fun isolatedTest = |<~t> name: String, setup: () -> TestContext<~t>, test: TestContext<~t> -> TestResult| {
    TestCase {
        name = name,
        description = "",
        testFn = || {
            with lifetime<~test> {
                val ctx = setup();
                ctx.test
            }
        }
    }
}

record TestContext<~t> {
    data: Option<Any>,
    cleanup: Option<() -> Unit>
}

// Property-based testing helpers
fun forAll = |<T> gen: () -> T, property: T -> Bool, count: Int32| {
    var i = 0;
    var failed = false;
    var failureMessage = "";
    
    while i < count && failed.not {
        val input = gen();
        if input.property.not {
            failed = true;
            failureMessage = "Property failed for input: " ++ input.toString;
        }
        i = i + 1;
    }
    
    if failed {
        TestResult { passed = false, message = failureMessage }
    } else {
        TestResult { passed = true, message = "Property held for " ++ count.toString ++ " inputs" }
    }
}

// Benchmark support
record BenchmarkResult {
    name: String,
    iterations: Int32,
    totalTime: Float64,
    avgTime: Float64
}

fun benchmark = |name: String, f: () -> Unit, iterations: Int32| {
    val startTime = currentTimeMillis;
    
    var i = 0;
    while i < iterations {
        f();
        i = i + 1;
    }
    
    val endTime = currentTimeMillis;
    val totalTime = endTime - startTime;
    val avgTime = totalTime / iterations.toFloat;
    
    BenchmarkResult {
        name = name,
        iterations = iterations,
        totalTime = totalTime,
        avgTime = avgTime
    }
}

// Main entry point
fun main = {
    "Restrict Language Test Framework".println;
    "================================".println;
    exampleTests
}