// Simplified test framework for quick testing
// Demonstrates Restrict Language's expressiveness

record Test {
    name: String,
    run: () -> Bool
}

// Simple assertion
fun assert = |condition: Bool| {
    if condition.not {
        panic("Assertion failed")
    }
}

// Test runner
fun runTests = |tests: List<Test>| {
    var passed = 0;
    var failed = 0;
    
    tests.forEach |test| {
        ("Running: " ++ test.name).println;
        try {
            test.run();
            passed = passed + 1;
            "  ✓ Passed".println;
        } catch e {
            failed = failed + 1;
            ("  ✗ Failed: " ++ e.message).println;
        }
    };
    
    "\nResults:".println;
    ("Passed: " ++ passed.toString).println;
    ("Failed: " ++ failed.toString).println;
}

// Example tests
fun main = {
    [
        Test { 
            name = "Basic arithmetic",
            run = || {
                (1 + 1 == 2).assert;
                (10 - 5 == 5).assert;
                (3 * 4 == 12).assert;
                true
            }
        },
        
        Test {
            name = "String operations",
            run = || {
                ("hello" ++ " world" == "hello world").assert;
                "test".length == 4.assert;
                true
            }
        },
        
        Test {
            name = "List operations",
            run = || {
                [1, 2, 3].length == 3.assert;
                [1, 2] ++ [3, 4] == [1, 2, 3, 4].assert;
                [1, 2, 3].head == Some(1).assert;
                [].head == None.assert;
                true
            }
        },
        
        Test {
            name = "Pattern matching",
            run = || {
                val result = Some(42) match {
                    Some(x) -> x * 2,
                    None -> 0
                };
                (result == 84).assert;
                true
            }
        },
        
        Test {
            name = "Affine types (conceptual)",
            run = || {
                // Test that mutable values can be used multiple times
                var x = 100;
                val y = x;
                val z = x;  // OK because x is mutable
                (y == z).assert;
                true
            }
        }
    ].runTests
}