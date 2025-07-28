// Pure Restrict Language Test Framework
// Minimal dots, maximum OSV

import "std_lib.rl"

// Test is just a function that returns a message
type Test = () -> String

// Core testing functions - no dots needed
fun test = |name: String, check: Bool| -> Test {
    || if check {
        "✓ " ++ name
    } else {
        "✗ " ++ name
    }
}

fun testEq = |name: String, actual, expected| -> Test {
    || if actual == expected {
        "✓ " ++ name
    } else {
        "✗ " ++ name ++ ": " ++ actual toString ++ " ≠ " ++ expected toString
    }
}

fun testWith = |name: String, value, check: Any -> Bool| -> Test {
    || if value check {
        "✓ " ++ name
    } else {
        "✗ " ++ name ++ ": check failed for " ++ value toString
    }
}

// Test runners - data flows naturally
fun run = |tests: List<Test>| {
    tests map |t| t() forEach println;
    
    val results = tests map |t| {
        val msg = t();
        msg startsWith "✓"
    };
    
    val passed = results filter |x| x length;
    val total = results length;
    
    "";
    passed toString ++ "/" ++ total toString ++ " passed" println
}

// Test combinators using OSV
fun all = |checks: List<Bool>| -> Bool {
    checks filter |x| x not length == 0
}

fun any = |checks: List<Bool>| -> Bool {
    checks filter |x| x length > 0
}

// Property testing without dots
fun forAll = |gen: () -> T, prop: T -> Bool, count: Int32| -> Bool {
    val inputs = (1 to count) map |_| gen();
    inputs map prop all
}

// Table-driven tests
fun cases = |data: List<(I, O)>, f: I -> O| -> List<Bool> {
    data map |(input, expected)| {
        input f == expected
    }
}

// Pattern matching helper
fun matches = |value, pattern: Any -> Bool| -> Bool {
    value pattern
}

// Test suite builder using pipes
fun suite = |name: String| -> |tests: List<Test>| -> List<Test> {
    ("=== " ++ name ++ " ===") println;
    tests
}

// Temporal test isolation
fun isolated = |<~t> testFn: () -> Test| -> Test {
    || with lifetime<~t> {
        testFn()()
    }
}

// Example: Natural Restrict Language testing
fun examples = {
    "Core Tests" suite [
        "addition" test (1 + 1 == 2),
        
        "equality" testEq (2 * 3) 6,
        
        "greater" testWith 10 |x| x > 5,
        
        "list length" test ([1, 2, 3] length == 3),
        
        "list concat" testEq ([1, 2] ++ [3, 4]) [1, 2, 3, 4],
        
        "option some" testWith Some(42) |opt| 
            match opt {
                Some(x) -> x == 42,
                None -> false
            },
        
        "pattern match" test (
            [1, 2, 3] matches |list|
                match list {
                    [] -> false,
                    [x, ...rest] -> x == 1 && rest length == 2
                }
        ),
        
        "all true" test ([true, true, true] all),
        
        "any true" test ([false, true, false] any),
        
        "property" test (
            (|| (1 to 10) shuffle)
            |list| list reverse reverse == list
            50
            forAll
        ),
        
        "table test" test (
            [(0, 0), (1, 1), (2, 1), (3, 2), (5, 5)]
            fib
            cases
            all
        ),
        
        "isolated resource" isolated |<~r>| {
            || {
                record Resource<~r> { id: Int32 }
                val res = Resource { id = 123 };
                "resource test" test (res id == 123)()
            }
        }
    ] run
}

// Even more natural with custom operators
infix => 8  // expectation operator

fun => = |actual, expected| -> Bool {
    actual == expected
}

// Ultra-clean test syntax
fun clean = {
    [
        "math" test (1 + 1 => 2),
        "compare" test (5 > 3),
        "list" test ([1,2,3] length => 3),
        "pipe" test (5 |> double |> inc => 11)
    ] run
}

// Minimal assertion library
fun expect = |value| -> Expectation {
    Expectation { value = value }
}

record Expectation<T> { value: T }

// OSV-style expectations
fun toBe = |exp: Expectation<T>, expected: T| -> Bool {
    exp value == expected
}

fun toEqual = toBe  // alias

fun toContain = |exp: Expectation<List<T>>, elem: T| -> Bool {
    exp value contains elem
}

fun toBeEmpty = |exp: Expectation<List<T>>| -> Bool {
    exp value length == 0
}

// Usage with expectations
fun expectationStyle = {
    [
        "expect equal" test (5 expect toBe 5),
        "expect list" test ([1,2,3] expect toContain 2),
        "expect empty" test ([] expect toBeEmpty)
    ] run
}

// Main
fun main = {
    examples;
    "";
    clean;
    "";
    expectationStyle
}

// Helper functions
fun toString = |x| -> String {
    // Built-in conversion
    __builtin_to_string(x)
}

fun println = |s: String| {
    __builtin_println(s)
}

fun startsWith = |s: String, prefix: String| -> Bool {
    // String prefix check
    __builtin_string_starts_with(s, prefix)
}

fun length = |list: List<T>| -> Int32 {
    __builtin_list_length(list)
}

fun id = |x| x
fun not = |b: Bool| -> Bool { if b { false } else { true } }
fun double = |x| x * 2
fun inc = |x| x + 1
fun fib = |n| match n { 0 -> 0, 1 -> 1, n -> (n-1) fib + (n-2) fib }
fun contains = |list, elem| __builtin_list_contains(list, elem)
fun to = |start, end| __builtin_range(start, end)
fun shuffle = |list| __builtin_list_shuffle(list)
fun reverse = |list| __builtin_list_reverse(list)
fun map = |list, f| __builtin_list_map(list, f)
fun filter = |list, pred| __builtin_list_filter(list, pred)
fun forEach = |list, f| __builtin_list_foreach(list, f)