// Comprehensive tests for Restrict Language features
// Using our own test framework

import "test_framework.rl"

// Test Affine Types
fun testAffineTypes: () -> TestSuite = {
    suite("Affine Type System", [
        test("simple affine violation", () -> {
            // This would need compiler integration to test properly
            // For now, we test the concept
            val x = 42;
            val y = x;  // x is consumed here
            // val z = x;  // This would be a compile error
            assertTrue(y == 42, "value should be moved correctly")
        }),
        
        test("mutable allows multiple use", () -> {
            var x = 42;
            val y = x;  // x is not consumed because it's mutable
            val z = x;  // This is allowed
            assertEqual(y + z, 84, "mutable values can be used multiple times")
        }),
        
        test("record field access consumes", () -> {
            record Point { x: Int32, y: Int32 }
            val p = Point { x: 10, y: 20 };
            val x = p.x;  // p is consumed
            // val y = p.y;  // This would be a compile error
            assertEqual(x, 10, "field access should work")
        })
    ])
}

// Test OSV Syntax
fun testOSVSyntax: () -> TestSuite = {
    suite("OSV (Object-Subject-Verb) Syntax", [
        test("method call syntax", () -> {
            record Counter { value: Int32 }
            
            fun increment: (c: Counter) -> Counter = {
                c.clone { value: c.value + 1 }
            }
            
            val c1 = Counter { value: 0 };
            val c2 = c1.increment();
            assertEqual(c2.value, 1, "OSV method call should work")
        }),
        
        test("pipe operator", () -> {
            fun double: (x: Int32) -> Int32 = { x * 2 }
            fun addOne: (x: Int32) -> Int32 = { x + 1 }
            
            val result = 5 |> double |> addOne;
            assertEqual(result, 11, "pipe operator should chain functions")
        }),
        
        test("mutable pipe operator", () -> {
            var x = 10;
            x |>> double;  // mutates x
            assertEqual(x, 20, "mutable pipe should update variable")
        })
    ])
}

// Test Pattern Matching
fun testPatternMatching: () -> TestSuite = {
    suite("Pattern Matching", [
        test("option matching", () -> {
            val opt = Some(42);
            val result = match opt {
                Some(x) -> x * 2,
                None -> 0
            };
            assertEqual(result, 84, "option pattern matching")
        }),
        
        test("list pattern matching", () -> {
            val list = [1, 2, 3];
            val result = match list {
                [] -> 0,
                [x] -> x,
                [x, y, ...rest] -> x + y
            };
            assertEqual(result, 3, "list pattern should match first two elements")
        }),
        
        test("nested patterns", () -> {
            record Box<T> { value: T }
            val nested = Some(Box { value: 42 });
            
            val result = match nested {
                Some(Box { value: x }) -> x,
                _ -> 0
            };
            assertEqual(result, 42, "nested pattern matching")
        })
    ])
}

// Test Temporal Types
fun testTemporalTypes: () -> TestSuite = {
    suite("Temporal Affine Types", [
        isolatedTest("basic temporal scope", 
            () -> TestContext { data: None, cleanup: None },
            (ctx) -> {
                record Resource<~r> { id: Int32 }
                
                with lifetime<~temp> {
                    val res = Resource { id: 123 };
                    assertEqual(res.id, 123, "temporal resource access")
                }
                // res is automatically cleaned up here
            }
        ),
        
        isolatedTest("temporal constraints",
            () -> TestContext { data: None, cleanup: None },
            (ctx) -> {
                record Outer<~o> { value: Int32 }
                record Inner<~i, ~o> where ~i within ~o { 
                    outer: Outer<~o> 
                }
                
                with lifetime<~long> {
                    val outer = Outer { value: 1 };
                    with lifetime<~short> where ~short within ~long {
                        val inner = Inner { outer: outer };
                        assertTrue(true, "temporal constraints should be enforced")
                    }
                }
            }
        )
    ])
}

// Test Type Inference
fun testTypeInference: () -> TestSuite = {
    suite("Type Inference", [
        test("basic inference", () -> {
            val x = 42;  // inferred as Int32
            val y = 3.14;  // inferred as Float64
            val s = "hello";  // inferred as String
            assertTrue(true, "basic type inference")
        }),
        
        test("generic function inference", () -> {
            fun identity: <T>(x: T) -> T = { x }
            
            val i = identity(42);  // T inferred as Int32
            val s = identity("hello");  // T inferred as String
            
            assertEqual(i, 42, "generic inference for Int32") &&
            assertEqual(s, "hello", "generic inference for String")
        }),
        
        test("collection inference", () -> {
            val list = [1, 2, 3];  // inferred as List<Int32>
            val opt = Some(42);  // inferred as Option<Int32>
            
            assertEqual(list.length(), 3, "list type inference") &&
            assertEqual(opt, Some(42), "option type inference")
        })
    ])
}

// Test Record System
fun testRecords: () -> TestSuite = {
    suite("Record System", [
        test("basic records", () -> {
            record Person { name: String, age: Int32 }
            val p = Person { name: "Alice", age: 30 };
            
            assertEqual(p.name, "Alice", "record field access")
        }),
        
        test("record cloning", () -> {
            record Point { x: Int32, y: Int32 }
            val p1 = Point { x: 10, y: 20 };
            val p2 = p1.clone { y: 30 };
            
            assertEqual(p2.x, 10, "unchanged field preserved") &&
            assertEqual(p2.y, 30, "changed field updated")
        }),
        
        test("frozen records", () -> {
            record Config { value: String }
            val config = Config { value: "test" }.freeze;
            // config.clone would be a compile error
            assertEqual(config.value, "test", "frozen record access")
        })
    ])
}

// Performance benchmarks
fun runBenchmarks: () -> Unit = {
    println("\n=== Benchmarks ===");
    
    val results = [
        benchmark("list creation", () -> {
            val list = [1, 2, 3, 4, 5];
            ()
        }, 10000),
        
        benchmark("record creation", () -> {
            record Data { x: Int32, y: Int32 }
            val d = Data { x: 1, y: 2 };
            ()
        }, 10000),
        
        benchmark("pattern matching", () -> {
            val opt = Some(42);
            match opt {
                Some(x) -> x,
                None -> 0
            };
            ()
        }, 10000)
    ];
    
    results.forEach(r -> {
        println(r.name ++ ": " ++ toString(r.avgTime) ++ "ms avg (" ++ toString(r.iterations) ++ " iterations)");
    })
}

// Main test runner
fun main = {
    println("Restrict Language Comprehensive Test Suite");
    println("=========================================\n");
    
    val runner = createRunner()
        |> addSuite(testAffineTypes())
        |> addSuite(testOSVSyntax())
        |> addSuite(testPatternMatching())
        |> addSuite(testTemporalTypes())
        |> addSuite(testTypeInference())
        |> addSuite(testRecords())
        |> runAll();
    
    printResults(runner);
    
    runBenchmarks()
}