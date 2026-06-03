use restrict_lang::{parse_program, TypeChecker};

/// Prototype cycle and recursion edge-case probes.
///
/// These examples preserve older proposal syntax and features that are not in
/// the current language specification. The release gate should ensure they are
/// rejected before code generation.
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
fn test_simple_prototype_cycle() {
    // Cycle Bug 1: Direct self-reference
    let input = r#"
    record SelfRef {
        name: String,
        parent: Option<SelfRef>
    }
    
    fun create_ouroboros() -> SelfRef {
        val snake = SelfRef {
            name = "ouroboros",
            parent = None
        };
        
        // Make it reference itself
        snake.parent = Some(snake);  // Cycle created!
        
        snake
    }
    
    fun walk_parents(obj: SelfRef, depth: Int32) -> Int32 {
        if depth > 100 {
            depth  // Recursion limit
        } else {
            match obj.parent {
                Some(p) => walk_parents(p, depth + 1),  // Infinite loop
                None => depth
            }
        }
    }
    
    fun main = {
        val cycle = create_ouroboros();
        walk_parents(cycle, 0)
    }"#;

    assert_rejected_before_codegen("simple prototype cycle probe", input);
}

#[test]
fn test_mutual_prototype_cycle() {
    // Cycle Bug 2: Mutual recursion between prototypes
    let input = r#"
    record Yin {
        name: String,
        yang: Option<Yang>
    }
    
    record Yang {
        name: String,
        yin: Option<Yin>
    }
    
    fun create_cycle() -> Yin {
        val yin = Yin { name = "yin", yang = None };
        val yang = Yang { name = "yang", yin = Some(yin) };
        
        // Complete the cycle
        yin.yang = Some(yang);
        
        yin
    }
    
    fun trace_cycle(start: Yin, count: Int32) -> String {
        if count > 10 {
            "cycle detected"
        } else {
            match start.yang {
                Some(y) => match y.yin {
                    Some(yi) => trace_cycle(yi, count + 1),
                    None => "broken"
                },
                None => "no cycle"
            }
        }
    }
    
    fun main = {
        val cycle = create_cycle();
        trace_cycle(cycle, 0)
    }"#;

    assert_rejected_before_codegen("mutual prototype cycle probe", input);
}

#[test]
fn test_clone_cycle_amplification() {
    // Cycle Bug 3: Cycles amplified through cloning
    let input = r#"
    record Node {
        id: Int32,
        next: Option<Node>,
        prev: Option<Node>
    }
    
    fun create_circular_list() -> Node {
        val n1 = Node { id = 1, next = None, prev = None };
        val n2 = clone n1 with { id = 2 };
        val n3 = clone n2 with { id = 3 };
        
        // Create circular linked list
        n1.next = Some(n2);
        n2.next = Some(n3);
        n3.next = Some(n1);  // Back to start
        
        n1.prev = Some(n3);
        n2.prev = Some(n1);
        n3.prev = Some(n2);
        
        // Now clone the whole cycle
        clone n1 with { id = 4 }  // What happens to next/prev?
    }
    
    fun count_nodes(start: Node, limit: Int32) -> Int32 {
        var current = start;
        var count = 0;
        
        while count < limit {
            match current.next {
                Some(n) => {
                    current = n;
                    count = count + 1;
                },
                None => break
            }
        };
        
        count  // Will hit limit due to cycle
    }
    
    fun main = {
        val list = create_circular_list();
        count_nodes(list, 1000)
    }"#;

    assert_rejected_before_codegen("clone cycle amplification probe", input);
}

#[test]
fn test_type_level_recursion() {
    // Cycle Bug 4: Infinite recursion at type level
    let input = r#"
    // Attempt 1: Direct recursive type
    record Infinite {
        child: Infinite  // No base case!
    }
    
    // Attempt 2: Mutual recursive types without base
    record A {
        b: B
    }
    
    record B {
        a: A
    }
    
    fun impossible() -> Infinite {
        // Can't construct without infinite recursion
        Infinite {
            child = Infinite {
                child = Infinite {
                    child = // ... forever
                }
            }
        }
    }
    
    fun main = {
        42  // Can't even call impossible()
    }"#;

    assert_rejected_before_codegen("type-level recursion probe", input);
}

#[test]
fn test_prototype_method_recursion() {
    // Cycle Bug 5: Method calls creating infinite recursion
    let input = r#"
    record Base {
        value: Int32
    }
    
    impl Base {
        fun process(self) -> Int32 {
            self.value
        }
    }
    
    record Derived from Base {
        multiplier: Int32  
    }
    
    impl Derived {
        fun process(self) -> Int32 {
            // Calls parent method which might call this again
            self.super().process() * self.multiplier
        }
    }
    
    record DoubleDerived from Derived {
        extra: Int32
    }
    
    impl DoubleDerived {
        fun process(self) -> Int32 {
            // Complex dispatch chain
            self.super().process() + self.extra
        }
    }
    
    fun create_dispatch_loop() -> Int32 {
        val obj = DoubleDerived {
            value = 10,
            multiplier = 2,
            extra = 5
        };
        
        // Method dispatch through prototype chain
        obj.process()  // Which process() is called?
    }
    
    fun main = {
        create_dispatch_loop()
    }"#;

    assert_rejected_before_codegen("prototype method recursion probe", input);
}

#[test]
fn test_lazy_evaluation_cycle() {
    // Cycle Bug 6: Lazy evaluation creating hidden cycles
    let input = r#"
    record Lazy<T> {
        thunk: fn() -> T,
        cached: Option<T>
    }
    
    record Stream {
        head: Int32,
        tail: Lazy<Stream>
    }
    
    fun create_infinite_stream() -> Stream {
        // Stream that references itself
        val stream = Stream {
            head = 1,
            tail = Lazy {
                thunk = || stream,  // Captures reference to itself!
                cached = None
            }
        };
        
        stream
    }
    
    fun take_n(stream: Stream, n: Int32) -> List<Int32> {
        if n <= 0 {
            []
        } else {
            var result = [stream.head];
            val next = stream.tail.thunk();  // Evaluates to same stream
            result ++ take_n(next, n - 1)    // Infinite recursion
        }
    }
    
    fun main = {
        val infinite = create_infinite_stream();
        take_n(infinite, 10)
    }"#;

    assert_rejected_before_codegen("lazy evaluation cycle probe", input);
}

#[test]
fn test_constraint_solver_cycle() {
    // Cycle Bug 7: Type constraint solver infinite loop
    let input = r#"
    record Constrained<T, U> 
    where T: ConvertTo<U>, U: ConvertTo<T> {
        t_val: T,
        u_val: U
    }
    
    trait ConvertTo<Target> {
        fun convert(self) -> Target
    }
    
    impl String: ConvertTo<Int32> {
        fun convert(self) -> Int32 { 
            self.parse() 
        }
    }
    
    impl Int32: ConvertTo<String> {
        fun convert(self) -> String {
            self.toString()
        }
    }
    
    fun conversion_loop<T, U>(c: Constrained<T, U>) -> T {
        // Convert back and forth forever?
        c.t_val.convert().convert().convert().convert()
    }
    
    fun main = {
        val c = Constrained {
            t_val = 42,
            u_val = "42"
        };
        
        conversion_loop(c)
    }"#;

    assert_rejected_before_codegen("constraint solver cycle probe", input);
}

#[test]
fn test_clone_freeze_cycle() {
    // Cycle Bug 8: Clone/freeze operations creating cycles
    let input = r#"
    record Freezable {
        name: String,
        frozen_parent: Option<Freezable>
    }
    
    fun freeze_cycle() -> Freezable {
        val base = Freezable {
            name = "base",
            frozen_parent = None
        };
        
        val child = clone base with {
            name = "child",
            frozen_parent = Some(freeze base)
        };
        
        val grandchild = clone child with {
            name = "grandchild",
            frozen_parent = Some(freeze child)
        };
        
        // Try to create cycle by updating base
        base.frozen_parent = Some(freeze grandchild);  // Frozen cycle?
        
        grandchild
    }
    
    fun detect_frozen_cycle(obj: Freezable, visited: Set<String>) -> Boolean {
        if visited.contains(obj.name) {
            true  // Cycle detected
        } else {
            match obj.frozen_parent {
                Some(parent) => {
                    visited.insert(obj.name);
                    detect_frozen_cycle(parent, visited)
                },
                None => false
            }
        }
    }
    
    fun main = {
        val cyclic = freeze_cycle();
        detect_frozen_cycle(cyclic, Set::new())
    }"#;

    assert_rejected_before_codegen("clone-freeze cycle probe", input);
}

#[test]
fn test_generic_instantiation_loop() {
    // Cycle Bug 9: Generic type instantiation creating infinite types
    let input = r#"
    record Nested<T> {
        value: T,
        deeper: Option<Nested<Nested<T>>>  // Exponential growth!
    }
    
    fun instantiate_monster() -> Nested<Int32> {
        Nested {
            value = 42,
            deeper = Some(Nested {
                value = Nested {
                    value = 1,
                    deeper = None
                },
                deeper = Some(Nested {
                    value = Nested {
                        value = Nested {
                            value = 2,
                            deeper = None
                        },
                        deeper = None
                    },
                    deeper = None  // Type system explosion
                })
            })
        }
    }
    
    fun main = {
        instantiate_monster()
    }"#;

    assert_rejected_before_codegen("generic instantiation loop probe", input);
}

#[test]
fn test_affine_cycle_paradox() {
    // Cycle Bug 10: Affine types in cycles creating paradoxes
    let input = r#"
    record AffineCycle {
        token: String,  // Affine
        next: Option<AffineCycle>
    }
    
    fun create_affine_cycle() -> AffineCycle {
        val n1 = AffineCycle { token = "one", next = None };
        val n2 = AffineCycle { token = "two", next = Some(n1) };
        val n3 = AffineCycle { token = "three", next = Some(n2) };
        
        // Try to complete the cycle
        n1.next = Some(n3);  // But n1 was moved into n2!
        
        n3
    }
    
    fun consume_cycle(start: AffineCycle) {
        var current = start;
        var count = 0;
        
        // Try to traverse and consume tokens
        while count < 10 {
            current.token |> println;  // Consume token
            
            match current.next {
                Some(n) => {
                    current = n;  // Move to next
                    count = count + 1;
                },
                None => break
            }
        }
        
        // In a cycle, we'd revisit nodes whose tokens were consumed!
    }
    
    fun main = {
        val cycle = create_affine_cycle();
        consume_cycle(cycle)
    }"#;

    assert_rejected_before_codegen("affine cycle paradox probe", input);
}
