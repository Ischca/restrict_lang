// Ultra-minimal Restrict Language Test DSL
// Maximum elegance, minimum ceremony

import "std_lib.rl"

// Single test result type
type Result = Pass | Fail(String)

// Core test operator
infix !! 9  // test operator: name !! assertion

// Test is just a name and result pair
fun !! = |name: String, assertion: Any| -> (String, Result) {
    val result = match assertion {
        true -> Pass,
        false -> Fail("assertion failed"),
        Result r -> r,
        (Bool, String) (cond, msg) -> if cond { Pass } else { Fail(msg) },
        _ -> Fail("invalid assertion type")
    };
    (name, result)
}

// Assertion helpers as simple functions
fun eq = |a, b| -> Result {
    if a == b { Pass } else { Fail(a.toString ++ " ≠ " ++ b.toString) }
}

fun gt = |a, b| -> Result {
    if a > b { Pass } else { Fail(a.toString ++ " ≤ " ++ b.toString) }
}

fun has = |list, elem| -> Result {
    if list.contains(elem) { Pass } else { Fail("not found") }
}

// Pattern matching helper
fun matches = |value, pattern| -> Result {
    pattern(value)
}

// Chaining assertions
fun and = |r1: Result, r2: Result| -> Result {
    match (r1, r2) {
        (Pass, Pass) -> Pass,
        (Fail(m1), _) -> Fail(m1),
        (_, Fail(m2)) -> Fail(m2)
    }
}

// Test runner - just a list processor
fun run = |tests: List<(String, Result)>| {
    val results = tests.map |(name, result)| {
        match result {
            Pass -> { ("✓ " ++ name).println; 1 },
            Fail(msg) -> { ("✗ " ++ name ++ ": " ++ msg).println; 0 }
        }
    };
    
    val passed = results.sum;
    val total = results.length;
    
    "\n━━━━━━━━━━━━━━━━━━".println;
    (passed.toString ++ "/" ++ total.toString ++ " passed").println;
}

// Property testing in one line
fun prop = |gen, check, n| -> Result {
    (1..n).map(|_| gen().check).find(|r| match r { Fail(_) -> true, _ -> false })
        .getOrElse(Pass)
}

// Table testing
fun table = |cases, f| -> Result {
    cases.map(|(input, expected)| input.f expected.eq)
         .find(|r| match r { Fail(_) -> true, _ -> false })
         .getOrElse(Pass)
}

// Temporal isolation as a simple wrapper
fun isolated = |<~t> test| -> Result {
    with lifetime<~t> { test() }
}

// Usage - pure data flow
fun main = {
    [
        // Simple assertions
        "addition" !! (1 + 1) 2.eq,
        "comparison" !! 5 3.gt,
        "list contains" !! [1, 2, 3] 2.has,
        
        // Chained assertions
        "multiple checks" !! ((2 + 2) 4.eq and 10 5.gt),
        
        // Pattern matching
        "option match" !! Some(42).matches |opt| match opt {
            Some(42) -> Pass,
            _ -> Fail("wrong value")
        },
        
        // Property test
        "reverse property" !! 
            (|| [1,2,3,4,5]) 
            (|l| l.reverse.reverse l.eq) 
            100.prop,
        
        // Table test
        "fibonacci" !! 
            [(0,0), (1,1), (2,1), (5,5)] 
            fib.table,
        
        // Isolated test
        "with resource" !! isolated |<~r>| {
            record Res<~r> { value: Int32 }
            val r = Res { value = 42 };
            r.value 42.eq
        },
        
        // Custom matcher
        "custom" !! 42.matches |n| {
            if n.isPrime { Pass } 
            else { Fail(n.toString ++ " is not prime") }
        }
    ].run
}

// Even more concise with custom syntax
fun test = |name| -> |body| -> (String, Result) {
    name !! body()
}

// Alternative style using pipes
fun testSuite = {
    []
        |> "math" { (1+1) 2.eq }.test.cons
        |> "lists" { [1,2,3].length 3.eq }.test.cons
        |> "option" { Some(5).map(|x| x*2) Some(10).eq }.test.cons
        |> run
}

// One-liner test definition
fun quickTest = |cases: List<(String, Bool)>| {
    cases.map |(name, pass)| name !! pass .run
}

// Usage
fun quickExample = {
    [
        ("true is true", true),
        ("1 < 2", 1 < 2),
        ("list not empty", [1,2,3].length > 0)
    ].quickTest
}

// Helpers
fun cons = |x, xs| [x] ++ xs
fun sum = |list: List<Int32>| -> Int32 {
    list.foldLeft(0, |acc, x| acc + x)
}
fun isPrime = |n: Int32| -> Bool {
    n > 1 && (2..n.sqrt).all(|i| n % i != 0)
}
fun fib = |n| match n { 0->0, 1->1, n->(n-1).fib+(n-2).fib }