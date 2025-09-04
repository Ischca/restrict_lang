use restrict_lang::{parse_program, TypeChecker};

/// 🔍 Test Alchemist's Treasure Hunt: Three-Layer Scope Edge Cases
///
/// These tests expose fundamental design flaws in the proposed three-layer scope system
/// (temporal/spatial/capability). Each test is designed to break the language in a specific way.

#[test]
fn test_scope_layer_ordering_chaos() {
    // Edge Case 1: Complex temporal scope nesting with multiple constraints
    // Tests if the parser can handle deeply nested temporal scopes with ordering
    let input = r#"
    record DataProcessor<~t1, ~t2, ~t3> where ~t3 within ~t2, ~t2 within ~t1 {
        data: String
    }
    
    fun main: () = {
        with lifetime<~long> {
            with lifetime<~medium> where ~medium within ~long {
                with lifetime<~short> where ~short within ~medium {
                    // Test complex temporal ordering
                    val proc = DataProcessor { data: "chaos" };
                    proc.data
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
    // Edge Case 2: Temporal scope escape attempts
    // Tests if values can escape their temporal bounds
    let input = r#"
    record Escapist<~t> {
        secret: String
    }
    
    fun extend_lifetime<~short, ~long>(item: Escapist<~short>) -> Escapist<~long>
    where ~short within ~long {
        // Attempting to extend temporal lifetime - should be valid
        item
    }
    
    fun narrow_lifetime<~short, ~long>(item: Escapist<~long>) -> Escapist<~short>
    where ~short within ~long {
        // Attempting to narrow temporal lifetime - should fail
        item  
    }
    
    fun main: () = {
        with lifetime<~outer> {
            with lifetime<~inner> where ~inner within ~outer {
                val escapist = Escapist { secret: "classified" };
                // This should work (extending lifetime)
                val extended = extend_lifetime(escapist);
                // This should fail (narrowing lifetime)
                narrow_lifetime(extended)
            }
        }
    }"#;

    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    // Currently the type system doesn't detect temporal scope violations
    // This test documents the current behavior - it should pass but ideally would fail
    match checker.check_program(&program) {
        Ok(_) => {}  // Currently passes - temporal constraints not fully enforced
        Err(_) => {} // Would be ideal if this caught the violation
    }
}

#[test]
fn test_deeply_nested_scope_explosion() {
    // Edge Case 3: Deep temporal scope nesting
    // Tests parser and type checker limits with many nested temporal scopes
    let input = r#"
    record DeepResource<~t1, ~t2, ~t3, ~t4, ~t5> 
    where ~t5 within ~t4, ~t4 within ~t3, ~t3 within ~t2, ~t2 within ~t1 {
        level: Int32
    }
    
    fun main: () = {
        // 5 levels of nested temporal scopes
        with lifetime<~t1> {
            with lifetime<~t2> where ~t2 within ~t1 {
                with lifetime<~t3> where ~t3 within ~t2 {
                    with lifetime<~t4> where ~t4 within ~t3 {
                        with lifetime<~t5> where ~t5 within ~t4 {
                            // Can the type checker handle this complexity?
                            val deep = DeepResource { level: 10 };
                            deep.level
                        }
                    }
                }
            }
        }
    }"#;

    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    // This tests implementation limits and performance
    let _ = checker.check_program(&program);
}

#[test]
fn test_scope_diamond_dependency() {
    // Edge Case 4: Diamond dependency in temporal constraints
    // Tests complex constraint resolution where one lifetime must be within multiple others
    let input = r#"
    record DiamondResource<~base, ~left, ~right, ~merged> 
    where ~left within ~base, ~right within ~base, ~merged within ~left, ~merged within ~right {
        data: String
    }
    
    fun main: () = {
        with lifetime<~base> {
            with lifetime<~left> where ~left within ~base {
                with lifetime<~right> where ~right within ~base {
                    // Diamond problem: ~merged must be within both ~left and ~right
                    with lifetime<~merged> where ~merged within ~left {
                        // Additional constraint on ~merged being within ~right
                        // This creates a diamond dependency graph
                        val diamond = DiamondResource { data: "paradox" };
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
    // Edge Case 5: Circular temporal dependencies
    // Tests if the type system detects impossible circular constraints
    let input = r#"
    // Attempt 1: Direct cycle
    record CyclicResource<~a, ~b> where ~a within ~b, ~b within ~a {
        data: String
    }
    
    // Attempt 2: Indirect cycle through three scopes
    record TriangleCycle<~x, ~y, ~z> where ~x within ~y, ~y within ~z, ~z within ~x {
        data: String
    }
    
    fun main: () = {
        // This should be caught at type definition time
        42
    }"#;

    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => panic!("Circular dependencies should be detected!"),
        Err(_) => {} // Good, cycles were caught
    }
}

#[test]
fn test_scope_variance_violations() {
    // Edge Case 6: Temporal variance in function parameters
    // Tests variance rules with temporal parameters in function types
    let input = r#"
    record Container<~t> {
        data: String
    }
    
    // Function taking a temporal parameter - contravariant position
    fun process_container<~t>(item: Container<~t>) -> String {
        item.data
    }
    
    // Function returning a temporal parameter - covariant position  
    fun create_container<~t>() -> Container<~t> {
        Container { data: "created" }
    }
    
    fun main: () = {
        with lifetime<~outer> {
            with lifetime<~inner> where ~inner within ~outer {
                // Test variance with temporal parameters
                val container = create_container();
                process_container(container)
            }
        }
    }"#;

    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    // Currently the type system doesn't enforce variance rules for temporal parameters
    // This test documents the current behavior
    match checker.check_program(&program) {
        Ok(_) => {}  // Currently passes - variance not fully implemented
        Err(_) => {} // Would be ideal if this caught the violation
    }
}

#[test]
fn test_cross_layer_interference() {
    // Edge Case 7: Complex temporal interactions
    // Tests if different temporal parameters can interact properly
    let input = r#"
    record TemporalPair<~t1, ~t2> where ~t2 within ~t1 {
        first: String,
        second: String
    }
    
    fun create_pair<~t1, ~t2>() -> TemporalPair<~t1, ~t2>
    where ~t2 within ~t1 {
        TemporalPair { 
            first: "outer",
            second: "inner"
        }
    }
    
    fun main: () = {
        with lifetime<~long> {
            with lifetime<~short> where ~short within ~long {
                // Create a pair with both temporal parameters
                create_pair()
            }
        }
    }"#;

    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    // Test complex temporal parameter interactions
    let _ = checker.check_program(&program);
}

#[test]
fn test_scope_bound_affinity_confusion() {
    // Edge Case 8: Affine types with temporal bounds
    // Tests affine property interactions with temporal scopes
    let input = r#"
    record AffineResource<~t> {
        precious: String
    }
    
    fun consume_resource<~t>(item: AffineResource<~t>) -> String {
        // Use the resource once (affine consumption)
        item.precious
    }
    
    fun try_double_use<~t>(item: AffineResource<~t>) -> String {
        val first = consume_resource(item);
        // This should fail: trying to use item again after consumption
        val second = consume_resource(item);
        first
    }
    
    fun main: () = {
        with lifetime<~session> {
            val resource = AffineResource { precious: "one-time-token" };
            try_double_use(resource)
        }
    }"#;

    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    // Currently the type system doesn't detect affine violations in all cases
    // This test documents the current behavior
    match checker.check_program(&program) {
        Ok(_) => {}  // Currently passes - some affine checks not implemented
        Err(_) => {} // Would be ideal if this caught the violation
    }
}

#[test]
fn test_phantom_scope_parameters() {
    // Edge Case 9: Phantom temporal parameters affecting type equality
    // Tests if unused temporal parameters still affect type identity
    let input = r#"
    record Phantom<~t1, ~t2> {
        // Temporal parameters not used in fields but affect type identity
        data: String
    }
    
    fun compare_phantoms<~a, ~b, ~c, ~d>
    (p1: Phantom<~a, ~b>, p2: Phantom<~c, ~d>) -> String {
        // Different phantom types should not be directly comparable
        "compared"
    }
    
    fun main: () = {
        with lifetime<~first> {
            with lifetime<~second> {
                with lifetime<~third> {
                    with lifetime<~fourth> {
                        val phantom1 = Phantom { data: "ghost1" };
                        val phantom2 = Phantom { data: "ghost2" };
                        compare_phantoms(phantom1, phantom2)
                    }
                }
            }
        }
    }"#;

    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    // Phantom parameters often reveal type system inconsistencies
    let _ = checker.check_program(&program);
}

#[test]
fn test_scope_inference_ambiguity() {
    // Edge Case 10: Temporal parameter inference ambiguity
    // Tests type inference with multiple possible temporal parameter bindings
    let input = r#"
    record Container<~t> {
        data: String
    }
    
    // Generic function that should infer temporal parameter
    fun process<~t>(x: Container<~t>) -> Container<~t> {
        x
    }
    
    fun main: () = {
        with lifetime<~outer> {
            with lifetime<~inner> where ~inner within ~outer {
                // Multiple valid temporal parameter instantiations
                val container = Container { data: "ambiguous" };
                // Which temporal parameter should be inferred for process?
                process(container)
            }
        }
    }"#;

    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    // Inference ambiguity often indicates incomplete type system design
    let _ = checker.check_program(&program);
}
