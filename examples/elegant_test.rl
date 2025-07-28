// Elegant Test DSL for Restrict Language
// Natural and expressive test syntax

import "std_lib.rl"

// Core test types
record Spec {
    description: String,
    assertions: List<Assertion>
}

record Assertion {
    check: () -> Bool,
    message: String
}

// Test context for building specs
record Context {
    specs: List<Spec>,
    currentSpec: Option<Spec>
}

// DSL entry point
fun describe = |what: String, spec: Context -> Context| -> List<Spec> {
    val ctx = Context { specs = [], currentSpec = None };
    val finalCtx = ctx.spec;
    finalCtx.specs
}

// Spec builder
fun it = |ctx: Context, behavior: String, test: () -> Unit| -> Context {
    val spec = Spec { 
        description = behavior, 
        assertions = [] 
    };
    
    // Run the test to collect assertions
    val assertions = collectAssertions(test);
    
    ctx.clone { 
        specs = ctx.specs ++ [spec.clone { assertions = assertions }],
        currentSpec = None
    }
}

// Assertion collectors (would need compiler magic in real implementation)
fun collectAssertions = |test: () -> Unit| -> List<Assertion> {
    // In a real implementation, this would intercept assertion calls
    []
}

// Natural assertion syntax
impl<T> T {
    fun should = |self: T| -> Expectation<T> {
        Expectation { actual = self }
    }
    
    fun shouldNot = |self: T| -> NegativeExpectation<T> {
        NegativeExpectation { actual = self }
    }
}

record Expectation<T> {
    actual: T
}

record NegativeExpectation<T> {
    actual: T
}

// Matchers
impl<T> Expectation<T> {
    fun equal = |self: Expectation<T>, expected: T| -> Assertion {
        Assertion {
            check = || self.actual == expected,
            message = self.actual.toString ++ " should equal " ++ expected.toString
        }
    }
    
    fun beGreaterThan = |self: Expectation<T>, value: T| -> Assertion {
        Assertion {
            check = || self.actual > value,
            message = self.actual.toString ++ " should be greater than " ++ value.toString
        }
    }
    
    fun beLessThan = |self: Expectation<T>, value: T| -> Assertion {
        Assertion {
            check = || self.actual < value,
            message = self.actual.toString ++ " should be less than " ++ value.toString
        }
    }
}

impl Expectation<Bool> {
    fun beTrue = |self: Expectation<Bool>| -> Assertion {
        Assertion {
            check = || self.actual,
            message = "Expected true but got " ++ self.actual.toString
        }
    }
    
    fun beFalse = |self: Expectation<Bool>| -> Assertion {
        Assertion {
            check = || self.actual.not,
            message = "Expected false but got " ++ self.actual.toString
        }
    }
}

impl<T> Expectation<List<T>> {
    fun contain = |self: Expectation<List<T>>, element: T| -> Assertion {
        Assertion {
            check = || self.actual.contains(element),
            message = "List should contain " ++ element.toString
        }
    }
    
    fun haveLength = |self: Expectation<List<T>>, length: Int32| -> Assertion {
        Assertion {
            check = || self.actual.length == length,
            message = "List should have length " ++ length.toString ++ " but has " ++ self.actual.length.toString
        }
    }
    
    fun beEmpty = |self: Expectation<List<T>>| -> Assertion {
        Assertion {
            check = || self.actual.length == 0,
            message = "List should be empty but has " ++ self.actual.length.toString ++ " elements"
        }
    }
}

impl<T> Expectation<Option<T>> {
    fun beSome = |self: Expectation<Option<T>>| -> Assertion {
        Assertion {
            check = || match self.actual {
                Some(_) -> true,
                None -> false
            },
            message = "Expected Some but got None"
        }
    }
    
    fun beNone = |self: Expectation<Option<T>>| -> Assertion {
        Assertion {
            check = || match self.actual {
                Some(_) -> false,
                None -> true
            },
            message = "Expected None but got Some"
        }
    }
    
    fun haveSomeValue = |self: Expectation<Option<T>>, value: T| -> Assertion {
        Assertion {
            check = || match self.actual {
                Some(x) -> x == value,
                None -> false
            },
            message = "Expected Some(" ++ value.toString ++ ") but got " ++ self.actual.toString
        }
    }
}

// Negative matchers
impl<T> NegativeExpectation<T> {
    fun equal = |self: NegativeExpectation<T>, value: T| -> Assertion {
        Assertion {
            check = || self.actual != value,
            message = self.actual.toString ++ " should not equal " ++ value.toString
        }
    }
}

// Custom matchers
fun satisfy = |actual: T, predicate: T -> Bool, description: String| -> Assertion {
    Assertion {
        check = || actual.predicate,
        message = actual.toString ++ " should " ++ description
    }
}

// Table-driven tests
record TestCase<Input, Output> {
    input: Input,
    expected: Output,
    description: Option<String>
}

fun testTable = |cases: List<TestCase<I, O>>, test: I -> O| -> List<Assertion> {
    cases.map |case| {
        val actual = case.input.test;
        Assertion {
            check = || actual == case.expected,
            message = match case.description {
                Some(desc) -> desc ++ ": expected " ++ case.expected.toString ++ " but got " ++ actual.toString,
                None -> "For input " ++ case.input.toString ++ ": expected " ++ case.expected.toString ++ " but got " ++ actual.toString
            }
        }
    }
}

// Example usage
fun exampleSpecs = {
    "List operations" describe |ctx| {
        ctx
        |> it "should support basic operations" || {
            [1, 2, 3].should.haveLength(3);
            [].should.beEmpty;
            [1, 2, 3].should.contain(2);
        }
        |> it "should support functional operations" || {
            [1, 2, 3].map(|x| x * 2).should.equal([2, 4, 6]);
            [1, 2, 3, 4].filter(|x| x > 2).should.equal([3, 4]);
        }
    }
}

// Property-based testing with elegant syntax
fun property = |description: String, gen: () -> T, predicate: T -> Bool| -> Spec {
    Spec {
        description = "Property: " ++ description,
        assertions = (1..100).map |_| {
            val input = gen();
            Assertion {
                check = || input.predicate,
                message = "Property failed for: " ++ input.toString
            }
        }
    }
}

// Async test support
fun eventually = |assertion: () -> Assertion, timeout: Int32| -> Assertion {
    // Would retry assertion until timeout
    assertion()
}

// Test runner with pretty output
fun runSpecs = |specs: List<Spec>| {
    "\n🧪 Running tests...\n".println;
    
    var totalAssertions = 0;
    var passedAssertions = 0;
    
    specs.forEach |spec| {
        ("  " ++ spec.description).println;
        
        spec.assertions.forEach |assertion| {
            totalAssertions = totalAssertions + 1;
            if assertion.check() {
                passedAssertions = passedAssertions + 1;
                "    ✓ pass".println;
            } else {
                ("    ✗ " ++ assertion.message).println;
            }
        }
    };
    
    "\n📊 Summary:".println;
    ("  Total assertions: " ++ totalAssertions.toString).println;
    ("  Passed: " ++ passedAssertions.toString).println;
    ("  Failed: " ++ (totalAssertions - passedAssertions).toString).println;
    
    if passedAssertions == totalAssertions {
        "\n✨ All tests passed!".println;
    } else {
        "\n❌ Some tests failed.".println;
    }
}

// BDD-style syntax
fun given = |context: String, setup: () -> T| -> Scenario<T> {
    Scenario {
        context = context,
        setup = setup,
        actions = [],
        expectations = []
    }
}

record Scenario<T> {
    context: String,
    setup: () -> T,
    actions: List<T -> T>,
    expectations: List<T -> Assertion>
}

impl<T> Scenario<T> {
    fun when = |self: Scenario<T>, action: String, f: T -> T| -> Scenario<T> {
        self.clone { actions = self.actions ++ [f] }
    }
    
    fun then = |self: Scenario<T>, expectation: String, check: T -> Assertion| -> Scenario<T> {
        self.clone { expectations = self.expectations ++ [check] }
    }
    
    fun run = |self: Scenario<T>| -> Spec {
        val initial = self.setup();
        val final = self.actions.foldLeft(initial, |acc, action| acc.action);
        
        Spec {
            description = self.context,
            assertions = self.expectations.map |exp| final.exp
        }
    }
}

// Main example
fun main = {
    val specs = [
        "Arithmetic" describe |ctx| {
            ctx
            |> it "should handle addition" || {
                (1 + 1).should.equal(2);
                (5 + 5).should.equal(10);
            }
            |> it "should handle comparison" || {
                5.should.beGreaterThan(3);
                3.should.beLessThan(5);
                true.should.beTrue;
            }
        },
        
        "Pattern matching" describe |ctx| {
            ctx
            |> it "should work with options" || {
                Some(42).should.beSome;
                None.should.beNone;
                Some(42).should.haveSomeValue(42);
            }
        },
        
        // Property-based test
        property("list reverse twice equals original", 
            || [1, 2, 3], // generator
            |list| list.reverse.reverse == list
        ),
        
        // BDD-style test
        given("a counter starting at 0", || 0)
            .when("incremented twice", |x| x + 1)
            .when("incremented again", |x| x + 1)
            .then("should equal 2", |x| x.should.equal(2))
            .run,
        
        // Table-driven test
        "Fibonacci" describe |ctx| {
            ctx
            |> it "should calculate correctly" || {
                [
                    TestCase { input = 0, expected = 0, description = Some("fib(0)") },
                    TestCase { input = 1, expected = 1, description = Some("fib(1)") },
                    TestCase { input = 5, expected = 5, description = Some("fib(5)") },
                    TestCase { input = 10, expected = 55, description = Some("fib(10)") }
                ].testTable(fib)
            }
        }
    ].flatten;
    
    specs.runSpecs
}