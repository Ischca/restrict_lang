use restrict_lang::{parse_program, TypeChecker};

/// Differential prototype edge-case probes.
///
/// These examples intentionally preserve older proposal syntax such as
/// `clone value with`, mutable rebinding, and traditional calls. Current
/// Restrict uses spec-defined clone syntax and OSV calls, so these probes
/// should be rejected before code generation.
fn assert_rejected_before_codegen(name: &str, input: &str) {
    let (remaining, program) = match parse_program(input) {
        Ok(parsed) => parsed,
        Err(_) => return,
    };

    if !remaining.trim().is_empty() {
        return;
    }

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program).is_err(),
        "{name} should be rejected by parser or type checker before codegen"
    );
}

#[test]
fn test_affine_field_double_inheritance() {
    // Bug Hunt 1: Can we double-consume affine fields through prototype chains?
    let input = r#"
    record Resource {
        token: String  // Affine by default
    }

    record Parent {
        resource: Resource
    }

    fun main = {
        val parent = Parent { resource = Resource { token = "secret" } };

        // Clone parent twice - do both children get the affine resource?
        val child1 = clone parent with { };
        val child2 = clone parent with { };

        // If this works, we've duplicated an affine resource!
        child1.resource.token |> println;
        child2.resource.token |> println;  // Double use!
    }"#;

    assert_rejected_before_codegen("affine field double-inheritance probe", input);
}

#[test]
fn test_prototype_field_shadowing_chaos() {
    // Bug Hunt 2: Field shadowing in prototype chains with different types
    let input = r#"
    record Base {
        value: Int32
    }

    record StringBase {
        value: String  // Same field name, different type!
    }

    fun main = {
        val base = Base { value = 42 };

        // What happens when we try to shadow with wrong type?
        val confused = clone base with {
            value = "not an int"  // Type confusion!
        };

        // Which type is value now?
        confused.value + 1  // Should fail
    }"#;

    assert_rejected_before_codegen("prototype field-shadowing probe", input);
}

#[test]
fn test_cyclic_prototype_inheritance() {
    // Bug Hunt 3: Creating inheritance cycles
    let input = r#"
    record Proto {
        name: String,
        parent: Option<Proto>
    }

    fun create_cycle() -> Proto {
        val a = Proto { name = "A", parent = None };
        val b = clone a with { name = "B", parent = Some(a) };
        val c = clone b with { name = "C", parent = Some(b) };

        // Now create the cycle by updating A
        a.parent = Some(c);  // A -> C -> B -> A

        // Walk the chain - infinite loop?
        val current = a;
        while true {
            match current.parent {
                Some(p) => current = p,
                None => break
            }
        };
        current
    }

    fun main = {
        create_cycle().name
    }"#;

    assert_rejected_before_codegen("cyclic prototype probe", input);
}

#[test]
fn test_differential_update_type_escape() {
    // Bug Hunt 4: Type parameter escape through differential updates
    let input = r#"
    record Container<T> {
        value: T,
        tag: String
    }

    fun smuggle_type<T, U>(cont: Container<T>, new_val: U) -> Container<U> {
        // Attempting to change generic type through differential update
        clone cont with { value = new_val }  // Type error!
    }

    fun main = {
        val int_cont = Container { value = 42, tag = "number" };
        val string_cont = smuggle_type(int_cont, "not a number");

        // We've changed the type parameter!
        string_cont.value.len()  // Should fail if T is still Int32
    }"#;

    assert_rejected_before_codegen("differential update type-escape probe", input);
}

#[test]
fn test_frozen_prototype_mutation_backdoor() {
    // Bug Hunt 5: Can we mutate frozen prototypes through clever cloning?
    let input = r#"
    frozen record Config {
        host: String,
        port: Int32
    }

    record Wrapper {
        config: Config
    }

    fun main = {
        val frozen_config = Config { host = "localhost", port = 8080 };
        val wrapper = Wrapper { config = frozen_config };

        // Try to mutate through wrapper cloning
        val evil_wrapper = clone wrapper with {
            config = Config { host = "evil.com", port = 666 }
        };

        // Did we just mutate a frozen value's semantics?
        evil_wrapper.config.host
    }"#;

    assert_rejected_before_codegen("frozen prototype mutation probe", input);
}

#[test]
fn test_prototype_method_dispatch_ambiguity() {
    // Bug Hunt 6: Method resolution order in deep prototype chains
    let input = r#"
    record Animal {
        sound: String
    }

    impl Animal {
        fun speak(self) -> String {
            self.sound
        }
    }

    record Dog from Animal {
        breed: String
    }

    impl Dog {
        fun speak(self) -> String {
            self.sound ++ " woof!"
        }
    }

    fun test_dispatch() -> String {
        val animal = Animal { sound = "generic" };
        val dog1 = clone animal with { breed = "labrador" };
        val dog2 = clone dog1 with { sound = "loud" };

        // Which speak() method is called?
        // Animal::speak or Dog::speak?
        // Does cloning preserve method bindings?
        dog2.speak()
    }

    fun main = {
        test_dispatch()
    }"#;

    assert_rejected_before_codegen("prototype method-dispatch probe", input);
}

#[test]
fn test_differential_field_deletion() {
    // Bug Hunt 7: Can we "delete" fields through differential updates?
    let input = r#"
    record Full {
        a: String,
        b: Int32,
        c: Boolean
    }

    record Partial {
        a: String
        // Missing b and c
    }

    fun shrink_record(full: Full) -> Partial {
        // Can we narrow a type through cloning?
        clone full with { }  // What happens to fields b and c?
    }

    fun main = {
        val full = Full { a = "test", b = 42, c = true };
        val partial = shrink_record(full);

        // This should fail - field doesn't exist
        partial.b
    }"#;

    assert_rejected_before_codegen("differential field-deletion probe", input);
}

#[test]
fn test_prototype_hash_collision_attack() {
    // Bug Hunt 8: Hash collision in prototype identity
    let input = r#"
    record Identifiable {
        // Assuming prototype hash is computed from fields
        data: String
    }

    fun create_collision() -> Boolean {
        // Create two different objects that might hash the same
        val proto1 = Identifiable { data = "ABC" };
        val proto2 = Identifiable { data = "ACB" };  // Permutation

        // Clone both and compare prototype hashes
        val child1 = clone proto1 with { };
        val child2 = clone proto2 with { };

        // If hashes collide, type system might confuse them
        child1 === child2  // Prototype identity comparison
    }

    fun main = {
        create_collision()
    }"#;

    assert_rejected_before_codegen("prototype hash-collision probe", input);
}

#[test]
fn test_deep_prototype_chain_performance() {
    // Bug Hunt 9: Performance degradation with deep chains
    let input = r#"
    record Node {
        value: Int32,
        depth: Int32
    }

    fun create_deep_chain(depth: Int32) -> Node {
        val base = Node { value = 0, depth = 0 };
        var current = base;
        var i = 1;

        while i <= depth {
            current = clone current with {
                value = i,
                depth = i
            };
            i = i + 1;
        };

        current
    }

    fun walk_chain(node: Node) -> Int32 {
        // Walking up the prototype chain
        // With depth 1000, this might be very slow
        node.value
    }

    fun main = {
        val deep = create_deep_chain(1000);
        walk_chain(deep)
    }"#;

    assert_rejected_before_codegen("deep prototype-chain probe", input);
}

#[test]
fn test_affine_prototype_field_tracking() {
    // Bug Hunt 10: Complex affine field tracking through differential updates
    let input = r#"
    record MultiResource {
        token1: String,  // Affine
        token2: String,  // Affine
        shared: Int32    // Not affine
    }

    fun partial_consume(mr: MultiResource) -> String {
        // Consume only token1
        val t1 = mr.token1;

        // Clone with partial state - what happens to token2?
        val partial = clone mr with {
            token1 = "already consumed"
        };

        // Can we still use token2 from partial?
        partial.token2  // Should this be allowed?
    }

    fun main = {
        val mr = MultiResource {
            token1 = "secret1",
            token2 = "secret2",
            shared = 42
        };

        partial_consume(mr)
    }"#;

    assert_rejected_before_codegen("affine prototype field-tracking probe", input);
}
