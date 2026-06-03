use restrict_lang::{parse_program, TypeChecker};

/// Arena allocation edge-case probes.
///
/// These examples intentionally use old proposal syntax and unsafe operations
/// that are outside the current language specification. They are kept as
/// regression probes to ensure such programs are rejected before code
/// generation instead of being treated as valid Restrict programs.
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
fn test_arena_use_after_free() {
    // Memory Bug 1: Classic use-after-free with arena references
    let input = r#"
    record ArenaString<@arena> {
        data: &str,
        arena: @arena
    }
    
    fun create_dangling_reference() -> &str {
        with arena<@temp> {
            val s = ArenaString {
                data = "allocated in temp arena",
                arena: @temp
            };
            
            s.data  // Returns reference to arena that's about to die!
        }  // @temp arena is freed here
    }
    
    fun main = {
        val dangling = create_dangling_reference();
        dangling.len()  // Use after free!
    }"#;

    assert_rejected_before_codegen("arena use-after-free probe", input);
}

#[test]
fn test_arena_double_free() {
    // Memory Bug 2: Double-free through multiple arena scopes
    let input = r#"
    record SharedBuffer<@a1, @a2> {
        data: Vec<u8>,
        arena1: @a1,
        arena2: @a2
    }
    
    fun double_arena_trouble() {
        with arena<@first> {
            with arena<@second> {
                val buffer = SharedBuffer {
                    data = vec![1, 2, 3],
                    arena1: @first,
                    arena2: @second
                };
                
                // Which arena owns the data?
                drop(buffer);  // Explicit drop
            }  // @second freed - partial free?
        }  // @first freed - double free?
    }
    
    fun main = {
        double_arena_trouble()
    }"#;

    assert_rejected_before_codegen("arena double-free probe", input);
}

#[test]
fn test_arena_stack_overflow() {
    // Memory Bug 3: Stack overflow through recursive arena allocation
    let input = r#"
    record RecursiveNode<@arena> {
        value: Int32,
        children: List<RecursiveNode<@arena>>,
        arena_ref: @arena
    }
    
    fun create_deep_tree<@parent>(depth: Int32) -> RecursiveNode<@parent> {
        if depth <= 0 {
            RecursiveNode {
                value: 0,
                children: [],
                arena_ref: @parent
            }
        } else {
            // Each recursive call creates new arena!
            with arena<@child> where @child within @parent {
                val children = [
                    create_deep_tree(depth - 1),
                    create_deep_tree(depth - 1)
                ];
                
                RecursiveNode {
                    value: depth,
                    children: children,
                    arena_ref: @child  // Wrong arena!
                }
            }
        }
    }
    
    fun main = {
        with arena<@root> {
            create_deep_tree(1000)  // Stack overflow from nested arenas
        }
    }"#;

    assert_rejected_before_codegen("arena stack-overflow probe", input);
}

#[test]
fn test_arena_cross_contamination() {
    // Memory Bug 4: Data from one arena contaminating another
    let input = r#"
    record Sensitive<@arena> {
        password: String,
        arena: @arena
    }
    
    fun leak_between_arenas() -> String {
        var leaked = "";
        
        with arena<@secure> {
            val secret = Sensitive {
                password: "super_secret_123",
                arena: @secure
            };
            
            // Try to copy out of arena
            leaked = secret.password;  // Deep copy or reference?
        }
        
        with arena<@public> {
            // Can we access the leaked data?
            leaked  // Contains data from freed @secure arena
        }
    }
    
    fun main = {
        leak_between_arenas()
    }"#;

    assert_rejected_before_codegen("arena cross-contamination probe", input);
}

#[test]
fn test_arena_allocation_overflow() {
    // Memory Bug 5: Integer overflow in arena size calculation
    let input = r#"
    record BigChunk<@arena> {
        data: Array<u8, 1_000_000>,
        arena: @arena
    }
    
    fun overflow_arena_size() {
        with arena<@huge> size = 100_000_000 {
            var chunks = [];
            var i = 0;
            
            // Allocate until we overflow arena size
            while i < 1000 {
                chunks.push(BigChunk {
                    data: [0; 1_000_000],
                    arena: @huge
                });
                i = i + 1;
            }
            
            // 1000 * 1_000_000 = 1_000_000_000 > arena size
            // Integer overflow or out-of-memory?
        }
    }
    
    fun main = {
        overflow_arena_size()
    }"#;

    assert_rejected_before_codegen("arena allocation-overflow probe", input);
}

#[test]
fn test_arena_pointer_smuggling() {
    // Memory Bug 6: Smuggling pointers between arenas
    let input = r#"
    record Pointer<T, @arena> {
        ptr: *T,
        arena: @arena
    }
    
    fun smuggle_pointer<@src, @dst>() -> Pointer<Int32, @dst> {
        with arena<@src> {
            val value = 42;
            val src_ptr = Pointer {
                ptr: &value as *Int32,
                arena: @src
            };
            
            // Type-level pointer laundering
            Pointer {
                ptr: src_ptr.ptr,  // Same pointer
                arena: @dst        // Different arena!
            }
        }
    }
    
    fun main = {
        with arena<@destination> {
            val smuggled = smuggle_pointer();
            unsafe { *smuggled.ptr }  // Dereferencing freed memory
        }
    }"#;

    assert_rejected_before_codegen("arena pointer-smuggling probe", input);
}

#[test]
fn test_arena_fragmentation_attack() {
    // Memory Bug 7: Deliberate fragmentation to cause allocation failure
    let input = r#"
    record Fragment<@arena> {
        size: usize,
        data: Vec<u8>,
        arena: @arena
    }
    
    fun fragment_arena<@victim>() {
        // Allocate and free in pattern to maximize fragmentation
        var fragments = [];
        var i = 0;
        
        // Phase 1: Allocate alternating sizes
        while i < 1000 {
            let size = if i % 2 == 0 { 1024 } else { 1 };
            fragments.push(Fragment {
                size: size,
                data: vec![0; size],
                arena: @victim
            });
            i = i + 1;
        }
        
        // Phase 2: Free every other allocation
        i = 0;
        while i < fragments.len() {
            if i % 2 == 0 {
                drop(fragments[i]);
            }
            i = i + 2;
        }
        
        // Phase 3: Try to allocate large contiguous block
        val large = Fragment {
            size: 500_000,
            data: vec![0; 500_000],  // Fails due to fragmentation
            arena: @victim
        };
    }
    
    fun main = {
        with arena<@target> size = 1_000_000 {
            fragment_arena()
        }
    }"#;

    assert_rejected_before_codegen("arena fragmentation probe", input);
}

#[test]
fn test_arena_type_confusion() {
    // Memory Bug 8: Type confusion through arena reinterpretation
    let input = r#"
    record IntHolder<@arena> {
        value: Int32,
        arena: @arena
    }
    
    record FloatHolder<@arena> {
        value: Float32,
        arena: @arena
    }
    
    fun type_pun<@arena>() -> Float32 {
        // Allocate as integer
        val int_data = IntHolder {
            value: 0x41414141,  // Bit pattern
            arena: @arena
        };
        
        // Get raw pointer
        val ptr = &int_data as *u8;
        
        // Reinterpret as float (type punning)
        val float_data = unsafe {
            *(ptr as *FloatHolder)
        };
        
        float_data.value  // Type confusion!
    }
    
    fun main = {
        with arena<@pun> {
            type_pun()
        }
    }"#;

    assert_rejected_before_codegen("arena type-confusion probe", input);
}

#[test]
fn test_arena_reset_invalidation() {
    // Memory Bug 9: References invalidated by arena reset
    let input = r#"
    record Cached<T, @arena> {
        value: T,
        cache_ptr: *T,
        arena: @arena
    }
    
    fun arena_reset_bug<@arena>() -> Int32 {
        val cached = Cached {
            value: 42,
            cache_ptr: null,
            arena: @arena
        };
        
        // Store pointer to value
        cached.cache_ptr = &cached.value;
        
        // Imagine arena supports reset operation
        arena_reset(@arena);  // Clears all allocations
        
        // Use cached pointer - points to freed memory!
        unsafe { *cached.cache_ptr }
    }
    
    fun main = {
        with arena<@resetable> {
            arena_reset_bug()
        }
    }"#;

    assert_rejected_before_codegen("arena reset-invalidation probe", input);
}

#[test]
fn test_arena_alignment_chaos() {
    // Memory Bug 10: Alignment issues causing undefined behavior
    let input = r#"
    #[repr(align(64))]
    record CacheAligned<@arena> {
        data: [u8; 64],
        arena: @arena
    }
    
    #[repr(packed)]
    record Packed<@arena> {
        a: u8,
        b: u64,  // Unaligned!
        arena: @arena
    }
    
    fun alignment_chaos<@arena>() {
        // Allocate with strict alignment
        val aligned = CacheAligned {
            data: [0; 64],
            arena: @arena
        };
        
        // Followed by packed struct
        val packed = Packed {
            a: 1,
            b: 0xDEADBEEF,
            arena: @arena
        };
        
        // Does arena respect alignment requirements?
        // Unaligned access to packed.b could crash
        val sum = aligned.data[0] as u64 + packed.b;
        
        // Cast shenanigans
        val ptr = &packed as *u8;
        val misaligned = unsafe {
            *((ptr + 1) as *u64)  // Definitely unaligned!
        };
    }
    
    fun main = {
        with arena<@chaotic> {
            alignment_chaos()
        }
    }"#;

    assert_rejected_before_codegen("arena alignment probe", input);
}
