use restrict_lang::{parse_program, TypeChecker};
use restrict_lang::type_checker::TypeError;

/// 🔍 Test Alchemist's Treasure Hunt: Three-Layer Scope Edge Cases
/// 
/// These tests expose fundamental design flaws in the proposed three-layer scope system
/// (temporal/spatial/capability). Each test is designed to break the language in a specific way.

#[test]
fn test_scope_layer_ordering_chaos() {
    // Edge Case 1: What happens when scope layers are declared out of "natural" order?
    // This tests if the language enforces any ordering between temporal/spatial/capability
    let input = r#"
    record DataProcessor<~t, @space, %cap> {
        data: String
    }
    
    fun main = {
        // Intentionally chaotic ordering
        with capability<%process> {
            with spatial<@heap> {
                with lifetime<~long> {
                    // Now reverse the order in inner scope
                    with lifetime<~short> where ~short within ~long {
                        with spatial<@stack> where @stack within @heap {
                            with capability<%read> where %read within %process {
                                // What happens here? Can we construct DataProcessor?
                                val proc = DataProcessor { data = "chaos" };
                                proc.data
                            }
                        }
                    }
                }
            }
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    // This should expose ordering constraints or lack thereof
    let _ = checker.check_program(&program);
}

#[test]
fn test_scope_escape_through_layers() {
    // Edge Case 2: Can values escape through different scope layers?
    let input = r#"
    record Escapist<~t, @space> {
        secret: String
    }
    
    fun smuggle_through_spatial<~t>(item: Escapist<~t, @stack>) -> Escapist<~t, @heap> {
        // Attempting to smuggle stack-allocated data to heap
        item  // Type system should catch this
    }
    
    fun smuggle_through_temporal<@space>(item: Escapist<~short, @space>) -> Escapist<~long, @space> {
        // Attempting to extend temporal lifetime
        item  // This must fail
    }
    
    fun main = {
        with lifetime<~short> {
            with spatial<@stack> {
                val escapist = Escapist { secret = "classified" };
                // Try both smuggling routes
                escapist |> smuggle_through_spatial |> smuggle_through_temporal
            }
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => panic!("Expected scope escape to be caught!"),
        Err(e) => assert!(e.to_string().contains("escape") || e.to_string().contains("lifetime")),
    }
}

#[test]
fn test_deeply_nested_scope_explosion() {
    // Edge Case 3: Stack overflow through deep nesting?
    let input = r#"
    record DeepResource<~t1, ~t2, ~t3, ~t4, ~t5, @s1, @s2, @s3, %c1, %c2> {
        level: Int32
    }
    
    fun main = {
        // 10 levels deep, 3 scope types each = 30 nested scopes
        with lifetime<~t1> {
        with spatial<@s1> {
        with capability<%c1> {
            with lifetime<~t2> where ~t2 within ~t1 {
            with spatial<@s2> where @s2 within @s1 {
            with capability<%c2> where %c2 within %c1 {
                with lifetime<~t3> where ~t3 within ~t2 {
                with spatial<@s3> where @s3 within @s2 {
                    with lifetime<~t4> where ~t4 within ~t3 {
                    with lifetime<~t5> where ~t5 within ~t4 {
                        // Can the type checker handle this complexity?
                        val deep = DeepResource { level = 10 };
                        deep.level
                    }}
                }}
            }}
        }}}
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    // This tests implementation limits and performance
    let _ = checker.check_program(&program);
}

#[test]
fn test_scope_diamond_dependency() {
    // Edge Case 4: Diamond dependency in scope constraints
    let input = r#"
    record DiamondResource<~base, ~left, ~right, ~merged> 
    where ~left within ~base, ~right within ~base, ~merged within ~left, ~merged within ~right {
        data: String
    }
    
    fun main = {
        with lifetime<~base> {
            with lifetime<~left> where ~left within ~base {
                with lifetime<~right> where ~right within ~base {
                    // Diamond problem: ~merged must be within both ~left and ~right
                    with lifetime<~merged> where ~merged within ~left, ~merged within ~right {
                        // This creates a diamond dependency graph
                        val diamond = DiamondResource { data = "paradox" };
                        diamond.data
                    }
                }
            }
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    // Diamond dependencies often expose constraint solver weaknesses
    let _ = checker.check_program(&program);
}

#[test]
fn test_scope_cycle_detection() {
    // Edge Case 5: Circular scope dependencies
    let input = r#"
    // Attempt 1: Direct cycle
    record CyclicResource<~a, ~b> where ~a within ~b, ~b within ~a {
        data: String
    }
    
    // Attempt 2: Indirect cycle through three scopes
    record TriangleCycle<~x, ~y, ~z> where ~x within ~y, ~y within ~z, ~z within ~x {
        data: String
    }
    
    fun main = {
        // This should be caught at type definition time
        42
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => panic!("Circular dependencies should be detected!"),
        Err(_) => {}, // Good, cycles were caught
    }
}

#[test]
fn test_scope_variance_violations() {
    // Edge Case 6: Variance issues with nested scopes
    let input = r#"
    record Invariant<~t, @s> {
        data: String
    }
    
    record Covariant<~t, @s> {
        producer: fn() -> Invariant<~t, @s>
    }
    
    record Contravariant<~t, @s> {
        consumer: fn(Invariant<~t, @s>) -> ()
    }
    
    fun break_variance<~t1, ~t2, @s1, @s2>
    (cov: Covariant<~t1, @s1>) -> Covariant<~t2, @s2>
    where ~t1 within ~t2, @s1 within @s2 {
        // This should fail: covariant position requires ~t2 within ~t1
        cov
    }
    
    fun main = {
        with lifetime<~outer> {
            with lifetime<~inner> where ~inner within ~outer {
                with spatial<@heap> {
                    with spatial<@stack> where @stack within @heap {
                        val cov = Covariant { 
                            producer = || Invariant { data = "variance test" }
                        };
                        // Try to widen the scope (should fail)
                        break_variance(cov)
                    }
                }
            }
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => panic!("Variance violation should be caught!"),
        Err(_) => {}, // Good
    }
}

#[test]
fn test_cross_layer_interference() {
    // Edge Case 7: Different scope layers interfering with each other
    let input = r#"
    record CrossLayer<~t, @s, %c> {
        // What if temporal affects spatial allocation?
        data: if ~t == ~static then @heap else @stack
    }
    
    fun allocate_confusion<~t, @s, %c>() -> CrossLayer<~t, @s, %c> {
        // The allocation strategy depends on temporal scope?!
        CrossLayer { data = "confused" }
    }
    
    fun main = {
        with lifetime<~static> {
            with spatial<@stack> {
                // Contradiction: ~static suggests heap, but we're in stack scope
                allocate_confusion()
            }
        }
    }"#;
    
    // This input is intentionally malformed to test parser robustness
    let result = parse_program(input);
    assert!(result.is_err(), "Cross-layer type dependencies should not parse");
}

#[test]
fn test_scope_bound_affinity_confusion() {
    // Edge Case 8: Scope-bounded affinity edge cases
    let input = r#"
    record AffineBound<~t, @s> affine_within<~t, @s> {
        // This value is affine only within specific scope combination
        precious: String
    }
    
    fun consume_in_scope<~t, @s>(item: AffineBound<~t, @s>) {
        // Use once within scope
        item.precious |> drop
    }
    
    fun try_double_use<~t, @s>(item: AffineBound<~t, @s>) {
        consume_in_scope(item);
        consume_in_scope(item);  // Should fail: affine violation
    }
    
    fun escape_affinity<~t1, ~t2, @s>(item: AffineBound<~t1, @s>) -> String
    where ~t2 within ~t1 {
        with lifetime<~t2> {
            // Does affinity bound change with scope narrowing?
            item.precious  // Escaping to outer scope
        }
    }
    
    fun main = {
        with lifetime<~session> {
            with spatial<@request_heap> {
                val bounded = AffineBound { precious = "one-time-token" };
                escape_affinity(bounded)  // Token escapes its affinity bound
            }
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => panic!("Affine bound violation should be caught!"),
        Err(e) => assert!(e.to_string().contains("affine") || e.to_string().contains("used")),
    }
}

#[test]
fn test_phantom_scope_parameters() {
    // Edge Case 9: Unused scope parameters that affect type equality
    let input = r#"
    record Phantom<~t, @s, %c> {
        // None of the parameters are used in fields
        data: String
    }
    
    fun type_equality_test<~t1, ~t2, @s1, @s2>
    (p1: Phantom<~t1, @s1, %read>, p2: Phantom<~t2, @s2, %write>) -> Bool {
        // Are these the same type? Parameters are phantom...
        p1 == p2  // Type error or runtime error?
    }
    
    fun main = {
        with lifetime<~a> {
        with lifetime<~b> {
            with spatial<@here> {
            with spatial<@there> {
                val phantom1 = Phantom { data = "ghost" };
                val phantom2 = Phantom { data = "ghost" };
                type_equality_test(phantom1, phantom2)
            }}
        }}
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    // Phantom parameters often reveal type system inconsistencies
    let _ = checker.check_program(&program);
}

#[test] 
fn test_scope_inference_ambiguity() {
    // Edge Case 10: Ambiguous scope inference
    let input = r#"
    record Ambiguous<~t, @s> {
        data: String
    }
    
    fun infer_me(x: Ambiguous) -> Ambiguous {
        // Which scopes should be inferred?
        x
    }
    
    fun main = {
        with lifetime<~t1> {
        with lifetime<~t2> {
            with spatial<@s1> {
            with spatial<@s2> {
                // Multiple valid scope instantiations
                val amb = Ambiguous { data = "which scope?" };
                infer_me(amb)  // ~t1,@s1 or ~t2,@s2 or something else?
            }}
        }}
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    // Inference ambiguity often indicates incomplete type system design
    let _ = checker.check_program(&program);
}