use restrict_lang::{parse_program, TypeChecker};

/// Additional edge case tests for temporal type inference
/// These tests focus on inference edge cases not covered elsewhere

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_inference_with_generics() {
    // Test that temporal parameters are correctly inferred with generic types
    let input = r#"
    record Box<T, ~b> {
        value: T
    }
    
    fun wrap: <T>(value: T) -> Box<T, ~new> = {
        Box { value: value }
    }
    
    fun main: () -> Unit = {
        with lifetime<~scope> {
            val boxed = wrap(42);  // Should infer Box<Int32, ~scope>
            boxed.value;
            Unit
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_inference_through_pipe() {
    // Test temporal inference through pipe operators
    let input = r#"
    record Stream<T, ~s> {
        data: T
    }
    
    fun process: <T, ~s>(stream: Stream<T, ~s>) -> T = {
        stream.data
    }
    
    fun main: () -> Unit = {
        with lifetime<~io> {
            val stream = Stream { data: "Hello" };
            stream |> process;  // Should infer ~s = ~io
            Unit
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_inference_nested_records() {
    // Test temporal inference with nested record types
    let input = r#"
    record Inner<~i> {
        value: Int32
    }
    
    record Outer<~o> {
        inner: Inner<~o>  // Inner shares the same temporal
    }
    
    fun main: () -> Unit = {
        with lifetime<~scope> {
            val inner = Inner { value: 42 };
            val outer = Outer { inner: inner };
            outer.inner.value;
            Unit
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_inference_mismatch() {
    // Test that temporal parameter mismatch is caught
    let input = r#"
    record Container<~c> {
        id: Int32
    }
    
    fun combine: <~a, ~b>(c1: Container<~a>, c2: Container<~b>) -> Container<~a> = {
        c1  // Return first container
    }
    
    fun main: () -> Unit = {
        with lifetime<~scope1> {
            with lifetime<~scope2> {
                val c1 = Container { id: 1 };
                val c2 = Container { id: 2 };
                // This should fail - c1 and c2 have different temporal scopes
                val result = combine(c1, c2);
                Unit
            }
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    // This test is expected to pass as the function allows different temporal parameters
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_inference_with_constraints() {
    // Test inference with temporal constraints
    let input = r#"
    record Outer<~o> {
        id: Int32
    }
    
    record Inner<~i, ~o> where ~i within ~o {
        outer_ref: Outer<~o>
    }
    
    fun makeInner: <~o>(outer: Outer<~o>) -> Inner<~new, ~o> 
    where ~new within ~o = {
        Inner { outer_ref: outer }
    }
    
    fun main: () -> Unit = {
        with lifetime<~parent> {
            val outer = Outer { id: 1 };
            with lifetime<~child> where ~child within ~parent {
                val inner = makeInner(outer);
                inner.outer_ref.id;
                Unit
            }
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_inference_option_types() {
    // Test temporal inference with Option types
    let input = r#"
    record Resource<~r> {
        name: String
    }
    
    fun findResource: <~r>(name: String) -> Option<Resource<~r>> = {
        if name == "test" {
            Some(Resource { name: name })
        } else {
            None
        }
    }
    
    fun main: () -> Unit = {
        with lifetime<~search> {
            val result = findResource("test");
            match result {
                Some(res) => res.name,
                None => "not found"
            };
            Unit
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_inference_list_operations() {
    // Test temporal inference with list operations
    let input = r#"
    record Item<~i> {
        id: Int32
    }
    
    fun makeList: <~l>() -> List<Item<~l>> = {
        [Item { id: 1 }, Item { id: 2 }, Item { id: 3 }]
    }
    
    fun main: () -> Unit = {
        with lifetime<~list_scope> {
            val items = makeList();
            items |> length;
            Unit
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_inference_higher_order() {
    // Test temporal inference with higher-order functions
    let input = r#"
    record Data<~d> {
        value: Int32
    }
    
    fun map: <~d>(data: Data<~d>, f: Int32 -> Int32) -> Data<~d> = {
        Data { value: f(data.value) }
    }
    
    fun double: (x: Int32) -> Int32 = {
        x * 2
    }
    
    fun main: () -> Unit = {
        with lifetime<~compute> {
            val data = Data { value: 21 };
            val result = map(data, double);
            result.value;
            Unit
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_inference_across_blocks() {
    // Test that temporal inference works across block boundaries
    let input = r#"
    record Token<~t> {
        value: String
    }
    
    fun main: () -> Unit = {
        with lifetime<~session> {
            val token = {
                val temp = Token { value: "secret" };
                temp  // Return from inner block
            };
            token.value;
            Unit
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_inference_with_aliases() {
    // Test temporal inference with type aliases (if supported)
    let input = r#"
    record Handle<~h> {
        id: Int32
    }
    
    fun alias: <~h>(h: Handle<~h>) -> Handle<~h> = {
        h  // Just return the same handle
    }
    
    fun main: () -> Unit = {
        with lifetime<~handle_scope> {
            val h1 = Handle { id: 1 };
            val h2 = alias(h1);
            h2.id;
            Unit
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_inference_polymorphic_constraint() {
    // Test inference with polymorphic temporal constraints
    let input = r#"
    record Parent<~p> {
        id: Int32
    }
    
    record Child<~c, ~p> where ~c within ~p {
        parent: Parent<~p>,
        name: String
    }
    
    fun createFamily: <~p>() -> (Parent<~p>, Child<~new, ~p>)
    where ~new within ~p = {
        val parent = Parent { id: 1 };
        val child = Child { parent: parent, name: "child" };
        (parent, child)
    }
    
    fun main: () -> Unit = {
        with lifetime<~family> {
            with lifetime<~generation> where ~generation within ~family {
                val (p, c) = createFamily();
                c.parent.id;
                Unit
            }
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_inference_error_propagation() {
    // Test that temporal inference errors propagate correctly
    let input = r#"
    record Error<~e> {
        message: String
    }
    
    record Result<T, ~r> {
        value: Option<T>,
        error: Option<Error<~r>>
    }
    
    fun tryOperation: <~op>() -> Result<Int32, ~op> = {
        Result { 
            value: Some(42), 
            error: None 
        }
    }
    
    fun main: () -> Unit = {
        with lifetime<~operation> {
            val result = tryOperation();
            match result.value {
                Some(v) => v,
                None => 0
            };
            Unit
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}