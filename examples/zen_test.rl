// Zen of Restrict Language Testing
// Where tests are just data transformations

import "std_lib.rl"

// Tests are morphisms from expectations to reality
type Test = String × (() -> Bool)

// The test operator - just function application
infix ⟷ 10
fun ⟷ = |desc, check| (desc, check)

// Reality check operators
infix ≟ 9   // semantic equality
infix ∈ 9   // membership
infix ↦ 9   // maps to

fun ≟ = |a, b| || a == b
fun ∈ = |elem, list| || list.contains(elem)
fun ↦ = |input, output| |f| || input.f == output

// Test flow is just data flow
fun flow = |tests: List<Test>| {
    tests 
    |> |t| t.1() t.0    // execute and pair with name
    |> |name, pass| (if pass "✓" else "✗") ++ " " ++ name
    |> println.forEach
    |> tests.count(|t| t.1()) tests.length  // count results
    |> |p, t| "\n" ++ p.toString ++ "/" ++ t.toString
    |> println
}

// Temporal test contexts are just scoped computations
fun within = |<~τ> compute| {
    with lifetime<~τ> { compute() }
}

// Property tests are just filtered generations
fun holds = |gen, invariant, n| || {
    (1..n).generate(gen).all(invariant)
}

// Table tests are just mapped validations
fun validates = |table, morphism| || {
    table.all |(i, o)| i morphism.↦ o
}

// The beauty of simplicity
fun demonstrate = {
    [
        "identity" ⟷ 42 42.≟,
        "composition" ⟷ 5 |> (*2) |> (+1) 11.≟,
        "membership" ⟷ 3 [1,2,3,4].∈,
        "morphism" ⟷ 5 10.↦ (*2),
        
        "property" ⟷ (|| 1 to 100).holds(|n| n + 0 n.≟, 100),
        
        "table" ⟷ [(0,1), (1,1), (2,2), (3,6)].validates(factorial),
        
        "temporal" ⟷ within |<~t>| {
            record Box<~t> { value: Int32 }
            Box { value = 7 }.value 7.≟()
        },
        
        "pipeline" ⟷ "hello"
            |> uppercase
            |> (++ " WORLD")
            |> length
            11.≟,
            
        "pattern" ⟷ [1,2,3] match {
            [] -> false,
            [_] -> false,
            [_,_,_] -> true,
            _ -> false
        }
    ].flow
}

// Helpers exist in the flow
fun count = |list, pred| list.filter(pred).length
fun all = |list, pred| list.filter(pred.not).isEmpty
fun generate = |range, gen| range.map |_| gen()
fun factorial = |n| match n { 0->1, n->n*(n-1).factorial }
fun uppercase = |s: String| s  // would use string method

// Tests as pure expressions
fun pure = {
    // A test suite is just an expression that evaluates to unit
    val results = [
        1 + 1 == 2,
        [].isEmpty,
        Some(5).isSome,
        "test".length == 4
    ];
    
    val passed = results.count(|x| x);
    (passed.toString ++ "/" ++ results.length.toString).println
}

// The ultimate: tests as types
type Proven<P> = private Proven(P)

fun prove = |<P> evidence: P, proof: P -> Bool| -> Option<Proven<P>> {
    if evidence.proof { Some(Proven(evidence)) } else { None }
}

// Usage: type-safe tested values
fun typeLevel = {
    val maybePrime = 17.prove(isPrime);
    match maybePrime {
        Some(Proven(p)) -> (p.toString ++ " is proven prime").println,
        None -> "Not prime".println
    }
}

fun main = {
    "⚡ Zen Test Runner".println;
    "═══════════════════".println;
    demonstrate
}

// Utilities
fun to = |start, end| (start..end).toList
fun isEmpty = |list| list.length == 0
fun isSome = |opt| match opt { Some(_) -> true, None -> false }
fun isPrime = |n| n > 1 && (2 to n.sqrt).all(|i| n % i != 0)
fun sqrt = |n: Int32| -> Int32 { 
    // Simple integer square root
    var i = 1;
    while i * i <= n { i = i + 1 };
    i - 1
}